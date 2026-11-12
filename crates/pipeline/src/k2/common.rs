use std::borrow::Cow;
use std::collections::HashMap;

use super::bcp47_grandfathered::GRANDFATHERED;
use super::iso4217_generated::ISO4217;
use gtfs_core::{EntityType, Notice};
use gtfs_rules::get_rule;
use smol_str::SmolStr;
use url::Url;

pub type HeaderIndex = HashMap<String, usize>;
pub type RowMap = HashMap<String, String>;

/// Shared validation context for K2 modules.
#[derive(Debug, Clone)]
pub struct K2Context<'a> {
    pub file: &'a str,
    pub headers: &'a [String],
    pub header_index: HeaderIndex,
}

impl<'a> K2Context<'a> {
    pub fn new(file: &'a str, headers: &'a [String]) -> Self {
        Self {
            file,
            headers,
            header_index: build_header_index(headers),
        }
    }
}

/// Convert the header array to an index map for O(1) lookup.
pub fn build_header_index(headers: &[String]) -> HeaderIndex {
    headers
        .iter()
        .enumerate()
        .map(|(idx, name)| (name.clone(), idx))
        .collect()
}

/// Convert a row to a map aligned with the header names.
///
/// EVERY header enters the map: if the row is shorter than that column, its value is empty.
/// Thus `get_trimmed_field` returning `None` means only "the column is absent from the header"
/// (a short row is `Some("")`). This distinction lets required-field rules stay silent when
/// the column is absent and defer to ARC_025.
pub fn build_row_map(headers: &[String], row: &[SmolStr]) -> RowMap {
    headers
        .iter()
        .enumerate()
        .map(|(i, header)| {
            let value = row.get(i).map(|v| v.to_string()).unwrap_or_default();
            (header.clone(), value)
        })
        .collect()
}

pub fn get_field<'a>(row: &'a RowMap, field: &str) -> Option<&'a str> {
    row.get(field).map(String::as_str)
}

pub fn get_trimmed_field<'a>(row: &'a RowMap, field: &str) -> Option<&'a str> {
    row.get(field).map(String::as_str).map(str::trim)
}

pub fn has_nonempty_field(row: &RowMap, field: &str) -> bool {
    get_trimmed_field(row, field).is_some_and(|v| !v.is_empty())
}

// ── Streaming (stream_mode) path helpers ─────────────────────────────────
//
// The `RowMap`-based readers above first convert the row to a map. In the streaming path,
// the row arrives as a raw `Cow` slice and is accessed by column index (no map is built —
// allocation cost is unacceptable at stop_times/shapes scale). The two layers are deliberately
// separate; the following helpers are distinguished by the `_col` suffix.

/// Column value from a Cow slice: empty string if the index is absent or the row is short.
///
/// `#[inline]` is deliberate: at stop_times scale (6M+ rows), this is called several times
/// per row — even within the crate, the call cost is measurable.
#[inline]
/// RAW value for IDENTITY fields — do NOT `trim` (issue #85).
///
/// 🔴 `get_trimmed_field`/`get_col` trimmed every value and IDs were built from the trimmed value.
/// The result was silent normalization of identity semantics:
///   · `stops.stop_id=" A "` + `stop_times.stop_id=A` → FK MATCHED (measured)
///   · `A` and `" A "` in the same file → FABRICATED duplicate (`STP_001`, measured)
/// `" A "` and `A` are not the same lexical value; PK/FK semantics are strict and cannot be
/// normalized. Extra whitespace is a separate QUALITY signal (`DQ_016` already reports it).
///
/// ⚠️ `trim` is still used for WHITESPACE DIAGNOSIS — without changing the identity:
/// `get_raw_field(..).filter(|v| !v.trim().is_empty())` says "a whitespace-only value DOES NOT
/// EXIST" while leaving the identity raw.
pub fn get_raw_field<'a>(row: &'a RowMap, field: &str) -> Option<&'a str> {
    row.get(field).map(|s| s.as_str())
}

/// Raw streaming value for IDENTITY columns — the non-trimming counterpart of `get_col` (issue #85).
pub fn get_col_raw<'a>(row: &'a [Cow<'_, str>], col: Option<usize>) -> &'a str {
    col.and_then(|i| row.get(i))
        .map(|s| s.as_ref())
        .unwrap_or("")
}

pub fn get_col<'a>(row: &'a [Cow<'_, str>], col: Option<usize>) -> &'a str {
    col.and_then(|i| row.get(i))
        .map(|s| s.as_ref().trim())
        .unwrap_or("")
}

/// u32 from a raw value. Empty → `Ok(None)`, invalid → `Err(())`.
/// Does NOT produce an error message — the caller builds one if needed
/// (see `parse_u32`, whose `RowMap` version returns its own error text).
#[allow(clippy::result_unit_err)]
pub fn parse_u32_col(raw: &str) -> Result<Option<u32>, ()> {
    if raw.is_empty() {
        return Ok(None);
    }
    raw.parse::<u32>().map(Some).map_err(|_| ())
}

/// f64 from a raw value. Empty → `Ok(None)`, invalid → `Err(())`.
#[allow(clippy::result_unit_err)]
/// ⚠️ NON-FINITE VALUES ARE REJECTED (issue #82). Rust's `f64` parser ACCEPTS `NaN`, `inf`,
/// and `-Infinity`; GTFS `Float`/`Latitude` types do not include them, and a `NaN` coordinate
/// silently turns every geometry rule below into "comparison false" (distance, ordering, threshold).
pub fn parse_f64_col(raw: &str) -> Result<Option<f64>, ()> {
    if raw.is_empty() {
        return Ok(None);
    }
    match raw.parse::<f64>() {
        Ok(v) if v.is_finite() => Ok(Some(v)),
        _ => Err(()),
    }
}

/// Field reading for STRICT type predicates — **RAW value** (issue #92).
///
/// 🔴 `get_trimmed_field` trimmed every value, so the type predicate saw the trimmed value:
/// `" https://example.com "` has unescaped whitespace raw, but the strict URL rule stayed silent;
/// only the `DQ_016` quality signal remained. Thus a Spec claim (`P5f72fb5a` PROVEN) was based
/// on a normalized proxy value. This is the type-predicate counterpart of `#85`'s identity decision.
///
/// ⚠️ A **WHITESPACE-ONLY value counts as ABSENT** — trimming is used only for PRESENCE
/// diagnosis, without changing the value. Otherwise an optional field containing `"   "` would
/// become an "invalid type", reversing the behavior deliberately preserved in `#85`.
pub fn get_lexical_field<'a>(row: &'a RowMap, field: &str) -> Option<&'a str> {
    let raw = get_raw_field(row, field)?;
    if raw.trim().is_empty() {
        None
    } else {
        Some(raw)
    }
}

/// Returns whether a well-formed BCP-47 tag has Japanese (`ja`) as its
/// primary language subtag. Prefix matching is intentionally avoided: `jaa`
/// is Jamamadí, not Japanese.
pub fn is_japanese_language_tag(value: &str) -> bool {
    looks_like_bcp47(value)
        && value
            .split('-')
            .next()
            .is_some_and(|primary| primary.eq_ignore_ascii_case("ja"))
}

/// Returns whether a user-facing value contains Japanese kana.
///
/// Kanji alone is deliberately not enough here: the same CJK characters are
/// used by Chinese and Taiwanese feeds, which would make a text-only JP gate
/// noisy. Kana (Hiragana, Katakana, or half-width Katakana) is the independent
/// text signal used to recover Japanese feeds whose language metadata is wrong.
pub fn has_japanese_kana(value: &str) -> bool {
    value.chars().any(|c| {
        matches!(c,
            '\u{3040}'..='\u{309F}'   // Hiragana
            | '\u{30A0}'..='\u{30FF}' // Katakana
            | '\u{FF66}'..='\u{FF9F}' // Half-width Katakana
        )
    })
}

pub fn parse_f64(row: &RowMap, field: &str) -> Result<Option<f64>, String> {
    let Some(raw) = get_lexical_field(row, field) else {
        return Ok(None);
    };
    if raw.is_empty() {
        return Ok(None);
    }
    // Shared helper: streaming and non-streaming paths must make the SAME decision (issue #82).
    parse_f64_col(raw)
        .map_err(|_| format!("'{field}' için sonlu bir f64 bekleniyor, alınan: {raw}"))
}

pub fn parse_u32(row: &RowMap, field: &str) -> Result<Option<u32>, String> {
    let Some(raw) = get_lexical_field(row, field) else {
        return Ok(None);
    };
    if raw.is_empty() {
        return Ok(None);
    }
    raw.parse::<u32>()
        .map(Some)
        .map_err(|_| format!("'{field}' için u32 bekleniyor, alınan: {raw}"))
}

pub fn parse_i32(row: &RowMap, field: &str) -> Result<Option<i32>, String> {
    let Some(raw) = get_lexical_field(row, field) else {
        return Ok(None);
    };
    if raw.is_empty() {
        return Ok(None);
    }
    raw.parse::<i32>()
        .map(Some)
        .map_err(|_| format!("'{field}' için i32 bekleniyor, alınan: {raw}"))
}

/// GTFS Schedule date format: YYYYMMDD
pub fn parse_service_date(row: &RowMap, field: &str) -> Result<Option<(u32, u32, u32)>, String> {
    let Some(raw) = get_lexical_field(row, field) else {
        return Ok(None);
    };
    if raw.is_empty() {
        return Ok(None);
    }
    if raw.len() != 8 || !raw.chars().all(|c| c.is_ascii_digit()) {
        return Err(format!("'{field}' için YYYYMMDD bekleniyor, alınan: {raw}"));
    }

    let year = raw[0..4]
        .parse::<u32>()
        .map_err(|_| format!("'{field}' yıl bölümü geçersiz: {raw}"))?;
    let month = raw[4..6]
        .parse::<u32>()
        .map_err(|_| format!("'{field}' ay bölümü geçersiz: {raw}"))?;
    let day = raw[6..8]
        .parse::<u32>()
        .map_err(|_| format!("'{field}' gün bölümü geçersiz: {raw}"))?;

    // ⚠️ CALENDAR VALIDITY (issue #82). Old code checked only FORMAT: `20261340` was parsed
    // and RETURNED as `(2026, 13, 40)`, so the service-day calculations below used a nonexistent
    // date. The streaming check went only as far as `month ≤ 12, day ≤ 31` — `20260231` still passed.
    // Both now call the SAME helper.
    if !is_valid_calendar_date(year, month, day) {
        return Err(format!("'{field}' takvimde olmayan tarih: {raw}"));
    }

    Ok(Some((year, month, day)))
}

/// Return the trimmed date if the raw value was rejected only because of surrounding whitespace.
///
/// The decision uses the raw value (`parse_service_date`, #92) and the finding is suppressed
/// under `DQ_016`. This helper is for semantic records ONLY: without it, a service containing
/// `"20271231    "` is treated as having "no active day" and every trip produces `TRP_026`.
/// An actually invalid value still returns `None` after trimming — a real format error does not
/// enter the service range.
pub fn whitespace_trimmed_service_date(row: &RowMap, field: &str) -> Option<(u32, u32, u32)> {
    let raw = get_raw_field(row, field)?;
    let trimmed = raw.trim();
    if trimmed.is_empty() || raw == trimmed {
        return None;
    }
    let row = RowMap::from([(field.to_string(), trimmed.to_string())]);
    parse_service_date(&row, field).ok().flatten()
}

/// Is this a real calendar day — including leap years (issue #82).
pub fn is_valid_calendar_date(year: u32, month: u32, day: u32) -> bool {
    if !(1..=12).contains(&month) || day == 0 {
        return false;
    }
    // ⚠️ SAME calculation as `gtfs_config::is_leap_year` in another crate (it uses it for
    // config range validation). This is a two-line universal calendar fact; use the same
    // expression instead of adding a dependency so the two implementations match when read.
    let leap = (year.is_multiple_of(4) && !year.is_multiple_of(100)) || year.is_multiple_of(400);
    let max = match month {
        1 | 3 | 5 | 7 | 8 | 10 | 12 => 31,
        4 | 6 | 9 | 11 => 30,
        2 if leap => 29,
        _ => 28,
    };
    day <= max
}

/// GTFS time format: HH:MM:SS (HH may exceed 24).
pub fn parse_gtfs_time(row: &RowMap, field: &str) -> Result<Option<(u32, u32, u32)>, String> {
    let Some(raw) = get_lexical_field(row, field) else {
        return Ok(None);
    };
    if raw.is_empty() {
        return Ok(None);
    }

    let parts: Vec<&str> = raw.split(':').collect();
    if parts.len() != 3 {
        return Err(format!("'{field}' için HH:MM:SS bekleniyor, alınan: {raw}"));
    }
    // ⚠️ LEXICAL WIDTH is also part of the type (issue #82): the spec says `H:MM:SS` or
    // `HH:MM:SS`; minutes and seconds are TWO DIGITS. Old code only parsed numerically and
    // accepted `1:2:3`. Hours may be 1–2 digits (`25:00:00` service-day notation).
    if !gtfs_time_widths_ok(&parts) {
        return Err(format!(
            "'{field}' için [H]H:MM:SS bekleniyor (dakika/saniye iki basamaklı), alınan: {raw}"
        ));
    }

    let hour = parts[0]
        .parse::<u32>()
        .map_err(|_| format!("'{field}' saat bölümü geçersiz: {raw}"))?;
    let minute = parts[1]
        .parse::<u32>()
        .map_err(|_| format!("'{field}' dakika bölümü geçersiz: {raw}"))?;
    let second = parts[2]
        .parse::<u32>()
        .map_err(|_| format!("'{field}' saniye bölümü geçersiz: {raw}"))?;

    if minute > 59 || second > 59 {
        return Err(format!(
            "'{field}' için dakika/saniye aralığı geçersiz: {raw}"
        ));
    }

    Ok(Some((hour, minute, second)))
}

/// LEXICAL validation of the GTFS time format — `parse_gtfs_time` and streaming-side
/// `parse_gtfs_time_raw` must make the SAME decision (otherwise the same feed differs in
/// streaming and full modes).
pub fn gtfs_time_widths_ok(parts: &[&str]) -> bool {
    // ⚠️ NO DIGIT LIMIT ON HOURS — deliberate. The first draft limited `HH` to 1–2 digits and
    // failed `tfr_006_does_not_overflow_on_huge_hour`. The issue is not the test: service-day
    // notation exceeds 24 (30:37, 38:10 measured in rail feeds), and multi-day train trips can
    // reach three digits. Limiting hours would reject VALID data; issue #82 reported `1:2:3`,
    // the minute/second width. Overflow protection remains separately in `TFR_006`.
    parts.len() == 3
        && !parts[0].is_empty()
        && parts[1].len() == 2
        && parts[2].len() == 2
        && parts.iter().all(|p| p.bytes().all(|b| b.is_ascii_digit()))
}

/// Is this an ISO 4217 ALPHABETIC code (issue #82).
///
/// ⚠️ Previously it only said "three uppercase ASCII letters": `ZZZ` passed and
/// `iso4217_minor_unit` silently assigned 2 decimal places to unknown codes — a fabricated
/// currency was both accepted and given fabricated amount formatting.
/// The list contains ACTIVE ISO 4217 alphabetic codes; historical codes (e.g. `TRL`, `DEM`)
/// are deliberately EXCLUDED — a feed must report a current currency.
pub fn is_iso4217(code: &str) -> bool {
    ISO4217.binary_search_by_key(&code, |(k, _)| *k).is_ok()
}

pub fn validate_enum(value: &str, allowed: &[&str]) -> bool {
    allowed.contains(&value)
}

/// GTFS `URL` type: *"A fully qualified URL that includes http:// or https://."*
///
/// `Url::parse` alone is NOT ENOUGH — it accepts any scheme and returns `Ok` for `mailto:`,
/// `ftp://`, `file:///`, `javascript:alert(1)`, and even `foo:bar`. Without scheme validation,
/// these values were accepted in passenger-visible fields (2026-08-03 measurement, T5 gap #1).
///
/// Scheme names are case-INSENSITIVE under RFC 3986 (`HTTP://` is valid), so comparison is done
/// that way. ⚠️ Compare BYTES, not a slice (`&s[..7]`): slicing `&str` at the middle of a
/// multibyte character **panics**. Since fields can contain free text, a value such as `Ünivers…`
/// would trigger that path. The prefix is pure ASCII, so byte comparison is safe and correct.
pub fn looks_like_url(value: &str) -> bool {
    fn starts_with_ci(s: &str, prefix: &str) -> bool {
        s.len() >= prefix.len()
            && s.as_bytes()[..prefix.len()].eq_ignore_ascii_case(prefix.as_bytes())
    }
    // ⚠️ INNER `trim` REMOVED (issue #92): if the predicate trimmed itself, the measurement
    // would remain normalized even when the caller supplied the raw value. Whitespace is already
    // one of the characters rejected by `url_strict_ok` — leading/trailing whitespace is therefore
    // NOT ESCAPED.
    let trimmed = value;
    let has_web_scheme = starts_with_ci(trimmed, "http://") || starts_with_ci(trimmed, "https://");
    if !(has_web_scheme && url_strict_ok(trimmed)) {
        return false;
    }
    let Ok(parsed) = Url::parse(trimmed) else {
        return false;
    };
    // DOUBLE SCHEME: `https://https://example.com/...` parses with the literal host `"https"`.
    // A single-label host equal to the scheme name cannot be a real address; this pattern only
    // arises when the scheme is written twice at the start of the value.
    //
    // ⚠️ This is NOT a GENERAL host-shape check, deliberately. `AGN_003` is Kritik+Spec, an R1
    // publication blocker: a false positive here REJECTS A VALID FEED. General host validation
    // would also risk IDNs, and `url_strict_ok`'s `is_ascii()` gate was removed for exactly that
    // reason in #144. This check is anchored to one certain pattern; neighboring cases such as
    // underscored hosts, `localhost`, and numeric hosts are DELIBERATELY excluded (#162).
    !matches!(parsed.host_str(), Some(h) if h.eq_ignore_ascii_case("http") || h.eq_ignore_ascii_case("https"))
}

/// LEXICAL half of the GTFS `URL` type — **STRICT**, by the spec's own measure (issue #80).
///
/// Full wording (from the pinned catalogue, `P5f72fb5a`):
/// *"A fully qualified URL that includes http:// or https://, and any special characters
/// in the URL must be correctly escaped."* — the referenced document is W3C's URI guidance.
/// There (and in RFC 3986), **a non-ASCII character cannot appear raw in a URI**; it must be
/// percent-encoded.
///
/// 🔴 **`is_ascii()` GATE REMOVED (#144, 2026-08-16).** Short history, because this decision
/// changed twice and its basis must be written down so it does not change a third time:
///
/// - On 2026-08-07 morning, raw Unicode was accepted because "IRI use is common".
/// - Reverted that same day: it was a PRODUCT tolerance, not a spec measure.
/// - It is now removed again, but for a **different reason** — not because it is a tolerance,
///   but because `is_ascii()` read the rule WRONG.
///
/// The rule says *"any **special characters** in the URL must be correctly escaped"*. `é`, `ü`,
/// Cyrillic, and full-width `ｗ` are NOT in RFC 3986's delimiter set; `url_escaping_ok`'s own
/// docstring already said so. The two functions conflicted: one said "non-ASCII is not rejected;
/// rejecting it would break international feeds", while the other added `is_ascii()` and did exactly that.
///
/// 🔑 **Distinguishing criterion: does the value have a deterministic canonical ASCII equivalent?**
/// ```text
///   "https://ｗｗｗ.city.chikuma.lg.jp" → https://www.city.chikuma.lg.jp/   (NFKC)
///   "https://thyon4vallées.ch"        → https://xn--thyon4valles-keb.ch/  (punycode)
///   "https://a.example/ürün"          → https://a.example/%C3%BCr%C3%BCn  (percent encoding)
/// ```
/// In all three, the producer's intent is UNAMBIGUOUS. For real defects, the parser invents an
/// interpretation (`a b` → `a%20b`) or passes the malformed value through (`a%zzb`);
/// `url_escaping_ok` catches both, and the caller catches a missing scheme (`wemapp.eu`).
/// Thus all three real error modes are caught WITHOUT `is_ascii()` — the gate only rejected values
/// that had a canonical equivalent.
///
/// ⚠️ **The basis of the old measurement is INVALIDATED.** The sentence justifying the gate was
/// "20 feeds · **0** non-ASCII in 12,897 URLs, so the strict measure affects no feed". The full
/// catalogue run (4,259 feeds) found real cases — the 20-feed sample simply had no international URLs.
/// The cost is asymmetric: calling a valid IDN address KRİTİK·Spec REJECTS a valid feed
/// (same class as `PTH_017`/`ARC_031`); the reverse only under-warns.
///
/// `P5f72fb5a` remains PROVEN: `url_escaping_ok` still STRICTLY measures the second half of the
/// rule (delimiter set + validity of percent escapes). The measured scope did not shrink; only
/// a constraint outside the wording was removed.
///
/// ⚠️ No separate "tolerant" predicate was added: nobody would call it, and an unused relaxation
/// would make it unclear later who uses which measure.
pub fn url_strict_ok(value: &str) -> bool {
    url_escaping_ok(value)
}

/// SECOND half of the GTFS `URL` type: *"…any special characters … correctly escaped."*
///
/// 🔴 `Url::parse` does NOT PROVE this (issue #80). The parser is deliberately tolerant
/// and NORMALIZES its input; measured on 2026-08-07:
/// ```text
///   "https://a.example/a b"    → Ok, a%20b   (unescaped whitespace silently encoded)
///   "https://a.example/a"b"   → Ok, a%22b
///   "https://a.example/a<b>"   → Ok, a%3Cb%3E
///   "https://a.example/a%zzb"  → Ok, UNCHANGED (malformed percent escape accepted)
///   "https://a.example/a\b"   → Ok, a/b      (backslash converted to slash)
/// ```
/// Thus "it parses" and "the producer escaped it correctly" were NOT THE SAME; the second half
/// of the rule was being treated as unproven.
///
/// ⚠️ **Non-ASCII characters are NOT REJECTED** — deliberate remainder. The spec says "special
/// character"; `ü`, `ş`, Cyrillic, etc. are not in RFC 3986's delimiter set and RFC 3987 (IRI)
/// use is common. Rejecting them would break international feeds. Measurement: no non-ASCII
/// appeared in 12,897 URLs across 20 feeds, so there is no evidence in that direction either.
pub fn url_escaping_ok(value: &str) -> bool {
    // Characters that cannot appear RAW in a URI under RFC 3986 (which the parser silently
    // encodes) + control characters.
    const MUST_ESCAPE: &[char] = &[' ', '"', '<', '>', '\\', '^', '`', '{', '}', '|'];
    if value
        .chars()
        .any(|c| MUST_ESCAPE.contains(&c) || c.is_control())
    {
        return false;
    }
    // A percent escape requires TWO hexadecimal digits; `%zz` and a trailing `%` are malformed.
    let b = value.as_bytes();
    let mut i = 0;
    while i < b.len() {
        if b[i] == b'%' {
            if i + 2 >= b.len() || !b[i + 1].is_ascii_hexdigit() || !b[i + 2].is_ascii_hexdigit() {
                return false;
            }
            i += 3;
        } else {
            i += 1;
        }
    }
    true
}

/// ISO 4217 minor unit — **SEPARATE from code VALIDITY** (issue #82).
///
/// `None` means two DIFFERENT things, and the caller cannot handle both the same way:
///   · code NOT IN THE TABLE → invalid code; `FAR_003`/`FPD_003` reports it, not this helper
///   · decimal UNDEFINED (`255`, `N.A.` in the source) → funds/precious metals (`XAU`, `XDR`)
///
/// 🔴 The old version ASSUMED "no value other than 0/2/3" and silently assigned 2 decimals
/// to every unknown code. Measured: `CLF` and `UYW` have **4** decimals, so the assumption was
/// wrong and a fabricated code (`ZZZ`) also received fabricated formatting.
pub fn iso4217_minor_unit(code: &str) -> Option<u8> {
    let c = code.trim();
    ISO4217
        .binary_search_by_key(&c, |(k, _)| *k)
        .ok()
        .map(|i| ISO4217[i].1)
        .filter(|&u| u != 255)
}

/// Does the amount carry the number of decimal places required by its currency?
///
/// Returns `true` for values that cannot be parsed as numbers — that violation belongs to
/// FPD_002/FAR_002 and must not be reported twice here.
pub fn amount_has_iso4217_decimals(amount: &str, currency: &str) -> bool {
    let a = amount.trim();
    if a.is_empty() || currency.trim().len() != 3 || a.parse::<f64>().is_err() {
        return true;
    }
    let dec = a.split_once('.').map(|(_, f)| f.len()).unwrap_or(0);
    // If the code is invalid or its decimal precision is undefined (fund/precious metal),
    // report nothing here: `FAR_003`/`FPD_003` reports invalid codes, so reporting twice is wrong.
    match iso4217_minor_unit(currency.trim()) {
        Some(units) => dec as u8 == units,
        None => true,
    }
}

/// GTFS `Email` type — **SYNTAX** validation (issue #86).
///
/// 🔴 The old predicate used `split_once('@')` and treated everything after the first `@` as
/// the domain: for `a@@b.com`, `local="a"`, `domain="@b.com"`, so all four checks passed.
/// A second `@` was never seen.
///
/// ## Which grammar level (cards MUST NOT claim more than this)
///
/// RFC 5322 `addr-spec` **dot-atom** form. SUPPORTED: `atext` characters
/// (`A-Za-z0-9` and ``!#$%&'*+-/=?^_`{|}~``), dot-separated atoms, and a domain with at least
/// two labels. UNSUPPORTED and deliberately INVALID: quoted local-part (`"a b"@x.com`),
/// comments/`CFWS`, IP-literal domain (`a@[192.0.2.1]`), source routing.
/// These are valid in RFC but unrealistic in a GTFS contact field; accepting them would expand
/// the predicate indefensibly.
///
/// ⚠️ **DELIVERABILITY IS NOT MEASURED** — there is no DNS/MX lookup and there cannot be one:
/// the validator does not access the network. "Syntax is valid" and "the address works" differ.
///
/// ⚠️ Non-ASCII characters are accepted (EAI / SMTPUTF8, RFC 6531): `info@şehir.example` can
/// be a real address and rejecting it would break international feeds. Measurement: no non-ASCII
/// email appeared in 20 corpus feeds, so there is no contrary evidence in that direction either.
pub fn looks_like_email(value: &str) -> bool {
    // ⚠️ No inner `trim` (issue #92): whitespace is rejected below and the raw value is measured.
    let trimmed = value;
    // There must be EXACTLY ONE separator. This is exactly the class the old code missed.
    let mut parts = trimmed.split('@');
    let (Some(local), Some(domain), None) = (parts.next(), parts.next(), parts.next()) else {
        return false;
    };
    if local.is_empty() || local.len() > 64 || domain.is_empty() || domain.len() > 255 {
        return false;
    }
    if trimmed.chars().any(|c| c.is_whitespace() || c.is_control()) {
        return false;
    }
    // local-part: dot-separated atoms; no empty atom, leading dot, or trailing dot.
    let atext = |c: char| {
        c.is_alphanumeric()
            || "!#$%&'*+-/=?^_`{|}~".contains(c)
            || (!c.is_ascii() && !c.is_whitespace() && !c.is_control())
    };
    if !local
        .split('.')
        .all(|atom| !atom.is_empty() && atom.chars().all(atext))
    {
        return false;
    }
    // domain: at least two labels; each starts and ends with a letter/digit and may contain hyphens.
    let labels: Vec<&str> = domain.split('.').collect();
    if labels.len() < 2 {
        return false;
    }
    let label_ok = |l: &str| {
        !l.is_empty()
            && l.len() <= 63
            && !l.starts_with('-')
            && !l.ends_with('-')
            && l.chars().all(|c| c.is_alphanumeric() || c == '-')
    };
    if !labels.iter().all(|l| label_ok(l)) {
        return false;
    }
    // The final label (TLD) cannot be numeric — `a@b.1` is not an address.
    labels
        .last()
        .is_some_and(|tld| tld.chars().all(|c| c.is_alphabetic()) && tld.len() >= 2)
}

/// GTFS `Language` type — **RFC 5646 (BCP 47) well-formed** syntax (issue #82).
///
/// 🔴 The old predicate only required "alphanumeric, ≤8" and accepted these three (measured):
/// `en-a` (singleton with no extension) · `en-a-b-c` (empty extension sequence) ·
/// `en-a-bbb-a-ccc` (REPEATED singleton). These are NOT well-formed in RFC 5646.
///
/// Grammar measured (`langtag`):
/// ```text
/// language ["-" script] ["-" region] *("-" variant) *("-" extension) ["-" privateuse]
///   language   2-3 alpha [3 times 3-alpha extlang] | 4 alpha (reserved) | 5-8 alpha
///   script     4 alpha            region   2 alpha | 3 digit
///   variant    5-8 alphanum | digit + 3 alphanum
///   extension  singleton (alphanum except x) + 1..* (2-8 alphanum)   ← singleton UNIQUE
///   privateuse "x" + 1..* (1-8 alphanum)
/// ```
/// ⚠️ **WELL-FORMED ≠ VALID.** No IANA registry lookup: `zz-ZZ` is syntactically correct
/// but is not a registered language. Registry validation requires a network/embedded registry,
/// and the card does NOT claim it.
/// Entirely private-use `x-…` tags and legacy (grandfathered) `i-…` forms are accepted — RFC accepts them.
pub fn looks_like_bcp47(value: &str) -> bool {
    // ⚠️ No inner `trim` (issue #92); whitespace is not a subtag character and is rejected below.
    let t = value;
    if t.is_empty() || t.len() > 100 {
        return false;
    }
    let parts: Vec<&str> = t.split('-').collect();
    if parts
        .iter()
        .any(|p| p.is_empty() || p.len() > 8 || !p.chars().all(|c| c.is_ascii_alphanumeric()))
    {
        return false;
    }
    let alpha = |p: &str| p.chars().all(|c| c.is_ascii_alphabetic());
    let digit = |p: &str| p.chars().all(|c| c.is_ascii_digit());

    // Entirely private use: x-abc
    if parts[0].eq_ignore_ascii_case("x") {
        return parts.len() >= 2;
    }
    // 🔴 GRANDFATHERED TAGS ARE A FIXED REGISTRY (issue #82, reopened). The old code treated
    // the `i-` prefix as an OPEN namespace: `i-` + any letter sequence passed, so `i-foo` and
    // `i-whatever` were considered well-formed. In RFC 5646 these forms come from the IANA
    // registry; a fabricated `i-foo` is neither a grandfathered tag nor a normal `langtag`.
    //
    // ⚠️ Do not confuse this with the `zz-ZZ` REMAINDER in the card: `zz-ZZ` is syntactically
    // well-formed but unregistered (deliberately accepted). `i-foo` is syntactically INVALID —
    // it was an error, not a remainder.
    //
    // Comparison is normalized to lowercase (RFC 5646 §2.1.1: tags are case-insensitive).
    // The table is generated; its source and date are at the top of the file.
    let lowered = t.to_ascii_lowercase();
    if GRANDFATHERED.binary_search(&lowered.as_str()).is_ok() {
        return true;
    }
    // `i-…` is valid only if present in the registry; the lookup above already catches it.
    if parts[0].eq_ignore_ascii_case("i") {
        return false;
    }

    let mut idx = 0usize;
    // language
    let lang = parts[idx];
    if !alpha(lang) || !(2..=8).contains(&lang.len()) {
        return false;
    }
    idx += 1;
    if (2..=3).contains(&lang.len()) {
        // at most 3 extlang (3 alpha)
        let mut ext = 0;
        while ext < 3 && idx < parts.len() && parts[idx].len() == 3 && alpha(parts[idx]) {
            // 3-alpha is neither script (script is 4) nor region (2 alpha/3 digit)
            idx += 1;
            ext += 1;
        }
    }
    // script (4 alpha)
    if idx < parts.len() && parts[idx].len() == 4 && alpha(parts[idx]) {
        idx += 1;
    }
    // region (2 alpha | 3 digit)
    if idx < parts.len()
        && ((parts[idx].len() == 2 && alpha(parts[idx]))
            || (parts[idx].len() == 3 && digit(parts[idx])))
    {
        idx += 1;
    }
    // variant* (5-8 alphanum | digit + 3 alphanum)
    while idx < parts.len() {
        let p = parts[idx];
        let is_variant = (5..=8).contains(&p.len())
            || (p.len() == 4 && p.starts_with(|c: char| c.is_ascii_digit()));
        if !is_variant {
            break;
        }
        idx += 1;
    }
    // extension* — each singleton ONCE, followed by at least one 2-8 subtag
    let mut seen_singletons: Vec<char> = Vec::new();
    while idx < parts.len() && parts[idx].len() == 1 && !parts[idx].eq_ignore_ascii_case("x") {
        let sing = parts[idx].chars().next().unwrap().to_ascii_lowercase();
        if !sing.is_ascii_alphanumeric() || seen_singletons.contains(&sing) {
            return false;
        }
        seen_singletons.push(sing);
        idx += 1;
        let mut n = 0;
        while idx < parts.len() && (2..=8).contains(&parts[idx].len()) {
            idx += 1;
            n += 1;
        }
        if n == 0 {
            return false; // singleton present, extension MISSING
        }
    }
    // privateuse
    if idx < parts.len() && parts[idx].eq_ignore_ascii_case("x") {
        idx += 1;
        if idx >= parts.len() {
            return false;
        }
        idx = parts.len(); // after x: 1-8 alphanum; the initial check already enforced this
    }
    idx == parts.len()
}

/// GTFS `Phone number` type. Spec (`agency_phone`): *"Dialable text (for example, TriMet's
/// **"503-238-RIDE"**) is permitted, but the field must not contain any other descriptive
/// text."* The same type applies to `booking_rules.phone_number`.
///
/// Until 2026-08-03, the check rejected EVERY value containing letters, including the spec's own
/// example. This `PTH_017` format was a bug: treating explicitly permitted data as a violation.
///
/// **Vanity versus descriptive text:** letter groups must form a contiguous TAIL at the END
/// of the value — `503-238-RIDE`, `+1 800 FLOWERS`, `1-800-GO-FEDEX` are valid;
/// `Call 503-238-1234` ("Call" at the start), `555-1234 ext 99` ("ext" in the middle) are invalid.
/// In vanity numbers, letters form the final blocks; descriptive text appears at the start or middle.
///
/// **Multiple letter groups require an additional condition (#95):** all must be UPPERCASE.
/// Vanity spelling universally uses uppercase to mark keypad letters (`1-800-GO-FEDEX`);
/// prose such as `1234 call us now` cannot pass. This condition is NOT required for one letter
/// group — the change only EXPANDS the accepted set and newly rejects no value.
///
/// ⚠️ **This is a QUALITY heuristic, NOT a phone-number grammar.** GTFS `Phone number` does not
/// define a full grammar; this measures the intuitive distinction between "translatable text"
/// and "descriptive text". Known boundary: all-uppercase prose (`1234 CALL US NOW`) is accepted.
/// The rule is reported only under the `Quality` authority via `AGN_007`/`BKR_022`; it makes no hard Spec claim.
///
/// ⚠️ The heuristic has NO MEASURED BENEFIT: not one vanity number appears in the 239-feed corpus.
/// It exists for spec alignment; it is preserved because the distinction continues to reject two
/// values actually present in the corpus (`80000078 (Liepājā); …`) — descriptive text forbidden by
/// the spec, on which the rule is CORRECT.
pub fn looks_like_phone(value: &str) -> bool {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return false;
    }
    // Threshold uses TRANSLATABLE DIGITS: in vanity numbers, letters replace digits
    // (`+1 800 FLOWERS` has only four digits but is an eleven-digit number). Still require
    // at least one digit, or `FLOWERS` alone would count as a phone number.
    let digit_count = trimmed.chars().filter(|c| c.is_ascii_digit()).count();
    let dialable_count = trimmed
        .chars()
        .filter(|c| c.is_ascii_alphanumeric())
        .count();
    if dialable_count < 5 || digit_count == 0 {
        return false;
    }
    if !trimmed.chars().all(|c| {
        c.is_ascii_digit()
            || c.is_ascii_alphabetic()
            || matches!(c, '+' | '-' | '(' | ')' | ' ' | '.')
    }) {
        return false;
    }
    // Letter groups: parts consisting entirely of letters after splitting on separators.
    let parts: Vec<&str> = trimmed
        .split(|c: char| !c.is_ascii_alphanumeric())
        .filter(|p| !p.is_empty())
        .collect();
    let letter_group_indices: Vec<usize> = parts
        .iter()
        .enumerate()
        .filter(|(_, p)| p.chars().all(|c| c.is_ascii_alphabetic()))
        .map(|(i, _)| i)
        .collect();
    if letter_group_indices.is_empty() {
        return true; // saf numara
    }
    // Letter groups must form a contiguous TAIL: all of the final n parts are letters.
    let n = letter_group_indices.len();
    let tail_is_contiguous = letter_group_indices
        .iter()
        .enumerate()
        .all(|(k, &i)| i == parts.len() - n + k);
    if !tail_is_contiguous {
        return false; // letters at the start or in the middle → descriptive text
    }
    // One group: old behavior (case-insensitive). Multiple groups: uppercase vanity-writing
    // convention — the only signal separating prose (#95).
    n == 1
        || letter_group_indices
            .iter()
            .all(|&i| parts[i].chars().all(|c| c.is_ascii_uppercase()))
}

pub fn looks_like_iana_timezone(value: &str) -> bool {
    value.parse::<chrono_tz::Tz>().is_ok()
}

pub fn is_hex_color_6(value: &str) -> bool {
    value.len() == 6 && value.chars().all(|c| c.is_ascii_hexdigit())
}

pub fn wcag_contrast_ratio(fg_hex: &str, bg_hex: &str) -> Option<f64> {
    fn channel_to_linear(c: u8) -> f64 {
        let srgb = c as f64 / 255.0;
        if srgb <= 0.03928 {
            srgb / 12.92
        } else {
            ((srgb + 0.055) / 1.055).powf(2.4)
        }
    }

    fn luminance(hex: &str) -> Option<f64> {
        if !is_hex_color_6(hex) {
            return None;
        }
        let r = u8::from_str_radix(&hex[0..2], 16).ok()?;
        let g = u8::from_str_radix(&hex[2..4], 16).ok()?;
        let b = u8::from_str_radix(&hex[4..6], 16).ok()?;
        Some(
            0.2126 * channel_to_linear(r)
                + 0.7152 * channel_to_linear(g)
                + 0.0722 * channel_to_linear(b),
        )
    }

    let fg = luminance(fg_hex)?;
    let bg = luminance(bg_hex)?;
    let (lighter, darker) = if fg >= bg { (fg, bg) } else { (bg, fg) };
    Some((lighter + 0.05) / (darker + 0.05))
}

/// Produce scope_key from a row according to the RuleMeta.scope_key_field definition.
///
/// For a single field, return its trimmed value.
/// For pipe-separated fields, return `None` if a component is missing.
pub fn derive_scope_key(row: &RowMap, scope_key_field: Option<&str>) -> Option<String> {
    let spec = scope_key_field?;
    if spec.contains('|') {
        let mut parts = Vec::new();
        for key in spec.split('|') {
            let value = get_trimmed_field(row, key)?;
            if value.is_empty() {
                return None;
            }
            parts.push(value.to_string());
        }
        Some(parts.join("|"))
    } else {
        let value = get_trimmed_field(row, spec)?;
        if value.is_empty() {
            None
        } else {
            Some(value.to_string())
        }
    }
}

/// Produce a canonical notice for K2 modules.
/// Shared emit for the `presence:required` gap — 12 callers.
///
/// Why a separate helper: anchor-granularity triage (2026-08-05) found the SAME gap in 12 fields —
/// rules titled "X duplicates" and "X not found" never saw empty values. Duplicate checks skip
/// empty keys (correct: empty values are not duplicates), FK resolution treats empty as "no reference"
/// and passes; the intermediate "this field is required" judgment was missing.
///
/// Stay silent if the field is ABSENT: a missing column belongs to `ARC_025`, not these rules.
#[allow(clippy::too_many_arguments)]
pub fn require_nonempty(
    row: &RowMap,
    field: &str,
    rule_id: &'static str,
    entity_type: EntityType,
    entity_id: Option<String>,
    file: &str,
    line: u64,
    message: String,
    remediation: &'static str,
    notices: &mut Vec<Notice>,
    counter: &mut u32,
) {
    // `None` = column absent from header → ARC_025; `Some("")` = column exists, value empty → this rule.
    if get_trimmed_field(row, field).is_some_and(str::is_empty) {
        notices.push(make_k2_notice(
            counter,
            rule_id,
            entity_type,
            entity_id,
            Some(row),
            file,
            Some(line),
            Some(field),
            Some(String::new()),
            Some("(dolu)".to_string()),
            message,
            remediation,
        ));
    }
}

/// `ARC_033` summary — ONE notice per FILE. All four stream files call this;
/// copying the check to four places would repeat the `ARC_022` (#75) mistake.
pub(crate) fn arc033_summary(
    acc: &crate::k1_parse::Rfc4180Acc,
    file: &str,
    counter: &mut u32,
) -> Option<gtfs_core::Notice> {
    if acc.rows == 0 {
        return None;
    }
    let kind = acc.kind.unwrap_or(crate::k1_parse::RFC4180_BARE_QUOTE);
    let example = acc.example.clone().unwrap_or_default();
    let mut summary = make_k2_notice(
        counter,
        "ARC_033",
        EntityType::File,
        Some(file.to_string()),
        None,
        file,
        acc.first_line,
        None,
        Some(format!(
            "rows={}; issue={}",
            acc.rows,
            crate::k1_parse::rfc4180_issue_code(kind)
        )),
        None,
        format!(
            "'{file}' RFC 4180'e uymuyor: {kind} ({} satırda; ilk örnek: '{example}').",
            acc.rows
        ),
        "Quote any field value that contains a quotation mark or a comma, and double every quotation mark inside it (\"12\"\" Street\").",
    );
    summary.details = Some(acc.details());
    Some(summary)
}

/// `ARC_013` — unclosed quote in the STREAM FILE BODY (issue #84).
///
/// 🔴 This finding was produced NOWHERE until 2026-08-07. K1 never opens the bodies of these
/// four files (`is_zip_stream`), the branch leading to K1's body scanner was DEAD and removed,
/// and K2 readers tolerated an unclosed quote "best-effort". A single stray quote in
/// `stop_times.txt` could therefore swallow the rest of the file and the validator said nothing.
///
/// ⚠️ **NOT FATAL — deliberate and DIFFERENT from K1.** In a non-streamed required file the same
/// defect becomes `FatalError`; K2 has NO fatal channel (the pipeline cannot stop after K1), and
/// rescanning the body in K1 would undo `#38`'s memory design. The finding is the same rule and
/// remains CRITICAL; what differs is whether the pipeline can stop.
/// Corpus measurement (2026-08-07): 20 feeds · 75 stream files · **0** unclosed quotes —
/// this decision affects no feed in current data.
pub fn arc013_unclosed_stream(file: &str, counter: &mut u32) -> gtfs_core::Notice {
    let msg = "Kapanmamış tırnak işareti (unclosed quote)";
    make_k2_notice(
        counter,
        "ARC_013",
        EntityType::File,
        Some(file.to_string()),
        None,
        file,
        None,
        None,
        Some("issue=unclosed_quote".to_string()),
        None,
        format!("'{file}' CSV tokenization hatası: {msg} — kaydın kalanı okunamadı."),
        "Check the CSV format; ensure all quoted fields are properly closed.",
    )
}

// K2 notices are called as a canonical emit table; collecting independent fields in a
// parameter struct would disrupt the existing table layout in this hot path.
#[allow(clippy::too_many_arguments)]
pub fn make_k2_notice(
    counter: &mut u32,
    rule_id: &str,
    entity_type: EntityType,
    entity_id: Option<String>,
    row: Option<&RowMap>,
    file: &str,
    line: Option<u64>,
    field: Option<&str>,
    observed_value: Option<String>,
    expected_value: Option<String>,
    message: String,
    remediation: &str,
) -> Notice {
    // K2-specific: derive scope_key from the row according to the rule's scope_key_field.
    // The `entity_id` fallback is ONLY for rules that declare an identity field: without one,
    // the notice is feed-level. Previously ARC_013/ARC_035 carried the filename, and the FTR/GGL
    // family carried an undeclared identity as scope; `blocks` treated these as broad roots.
    let meta = get_rule(rule_id).unwrap_or_else(|| panic!("K2: bilinmeyen rule_id {rule_id}"));
    let scope_key = meta.scope_key_field.and_then(|field| {
        row.and_then(|r| derive_scope_key(r, Some(field)))
            .or_else(|| entity_id.clone())
    });

    let whitespace_derived = row.zip(field).is_some_and(|(r, f)| {
        whitespace_parser_derivative(rule_id, r, f, observed_value.as_deref(), &message)
    });
    let mut notice = crate::notice_factory::build(
        "K2",
        Some("k2"),
        counter,
        rule_id,
        entity_type,
        entity_id,
        scope_key,
        Some(file.to_string()),
        line,
        field.map(str::to_string),
        observed_value,
        expected_value,
        message,
        remediation,
    );
    if whitespace_derived {
        notice.whitespace_derived = true;
    }
    notice
}

/// Recognizes when a K2 type/enum notice failed to parse only because of whitespace around
/// the raw value. Content errors (such as "abc") do not pass this gate, preserving independent
/// semantic violations on the same row. Supports compound field keys (a|b).
fn whitespace_parser_derivative(
    rule_id: &str,
    row: &RowMap,
    field: &str,
    observed: Option<&str>,
    message: &str,
) -> bool {
    let parser_error = message.contains("bekleniyor, alınan:")
        || message.contains("sayı olarak okunamıyor")
        || message.contains("ondalık sayı olarak okunamıyor")
        || message.contains("geçersiz.");
    if !parser_error {
        return false;
    }
    whitespace_only_parser_failure(rule_id, row, field, observed)
}

/// Once the caller has proven a parse error, test whether it arose only from surrounding whitespace
/// without constructing the expensive error message and `Notice`.
pub(crate) fn whitespace_only_parser_failure(
    rule_id: &str,
    row: &RowMap,
    field: &str,
    observed: Option<&str>,
) -> bool {
    field.split('|').any(|name| {
        let Some(raw) = row.get(name).map(String::as_str) else {
            return false;
        };
        let trimmed = raw.trim();
        !trimmed.is_empty()
            && raw != trimmed
            && observed.is_some_and(|value| value == raw || value == trimmed)
            && trimmed_value_is_semantically_valid(rule_id, name, trimmed)
    })
}

/// Did a FORMAT validator such as URL / email / language tag reject only because of surrounding
/// whitespace? The root is `DQ_016`; K7 suppresses the derivative — but only when a `DQ_016`
/// root exists in the same file, otherwise preserve the finding so it does not disappear without a trace (`k7_reporting.rs`).
///
/// ⚠️ The numeric path's `trimmed_value_is_semantically_valid` table is NOT USED HERE: it
/// returns `true` for unknown fields and would suppress real format violations such as
/// `' bozuk-url '`. Instead, the caller supplies its own validator and the trimmed value
/// MUST PASS that validator.
///
/// Measurement (corpus 16): `AGN_008` 10 feeds, `AGN_009` 9 feeds, `FIN_004` 1 feed — in all
/// 20 cases the only defect was leading whitespace (`' https://metrarail.com/tickets'`,
/// `' kotu@city.sakata.lg.jp'`, `' bg'`), and `DQ_016` already reported the same row in 9 of 10 feeds.
pub(crate) fn whitespace_only_format_failure(raw: &str, valid: impl Fn(&str) -> bool) -> bool {
    let trimmed = raw.trim();
    !trimmed.is_empty() && raw != trimmed && valid(trimmed)
}

/// If the trimmed raw value also carries an independent range/enum violation, it cannot be reduced
/// to a whitespace root alone. This small field table preserves values such as 91.0; for unknown
/// fields, parse success is sufficient root evidence.
fn trimmed_value_is_semantically_valid(rule_id: &str, field: &str, value: &str) -> bool {
    if field.ends_with("_lat") {
        return value
            .parse::<f64>()
            .is_ok_and(|v| v.is_finite() && (-90.0..=90.0).contains(&v));
    }
    if field.ends_with("_lon") {
        return value
            .parse::<f64>()
            .is_ok_and(|v| v.is_finite() && (-180.0..=180.0).contains(&v));
    }
    if matches!(
        field,
        "price" | "amount" | "length" | "max_slope" | "level_index"
    ) {
        return value
            .parse::<f64>()
            .is_ok_and(|v| v.is_finite() && v >= 0.0);
    }
    if field == "min_width" {
        return value.parse::<f64>().is_ok_and(|v| v.is_finite() && v > 0.0);
    }
    if field == "traversal_time" {
        return value.parse::<u32>().is_ok_and(|v| v > 0);
    }
    if field == "pathway_mode" {
        return value.parse::<u32>().is_ok_and(|v| (1..=7).contains(&v));
    }
    if field == "is_bidirectional" {
        return value.parse::<u32>().is_ok_and(|v| v <= 1);
    }
    if field == "stair_count" {
        return value.parse::<i32>().is_ok_and(|v| v != 0);
    }
    if field == "route_type" {
        return value
            .parse::<u32>()
            .is_ok_and(|v| matches!(v, 0..=7 | 11 | 12));
    }
    if field == "location_type" {
        return value.parse::<u32>().is_ok_and(|v| v <= 4);
    }
    if field == "wheelchair_boarding"
        || field == "wheelchair_accessible"
        || field == "bikes_allowed"
        || field == "cars_allowed"
    {
        return value.parse::<u32>().is_ok_and(|v| v <= 2);
    }
    if field == "payment_method" {
        return value.parse::<u32>().is_ok_and(|v| v <= 1);
    }
    if field == "transfers" {
        return value.parse::<u32>().is_ok_and(|v| v <= 2);
    }
    if field == "transfer_type" {
        return value.parse::<u32>().is_ok_and(|v| v <= 5);
    }
    if matches!(
        field,
        "monday" | "tuesday" | "wednesday" | "thursday" | "friday" | "saturday" | "sunday"
    ) {
        return value.parse::<u32>().is_ok_and(|v| v <= 1);
    }
    if field == "stop_access" {
        return value.parse::<u32>().is_ok_and(|v| v <= 2);
    }
    // Dates were absent from the table and fell through to the `true` below: `" 2027-12-31 "`
    // was suppressed as a whitespace derivative even though it remained invalid after trimming,
    // so Kritik·Spec `CAL_004` was hidden (17 September, `whitespace_padded_calendar_dates_keep_the_service_active`).
    if matches!(
        field,
        "start_date" | "end_date" | "date" | "feed_start_date" | "feed_end_date"
    ) {
        let row = RowMap::from([(field.to_string(), value.to_string())]);
        return matches!(parse_service_date(&row, field), Ok(Some(_)));
    }
    // Rule-specific custom parser messages still need a conservative guard.
    if rule_id == "RTS_004" {
        return value
            .parse::<u32>()
            .is_ok_and(|v| matches!(v, 0..=7 | 11 | 12));
    }
    true
}

#[cfg(test)]
mod tests {

    #[test]
    fn looks_like_url_rejects_a_doubled_scheme() {
        // tdg-83744: 'https://https://www.mairie-courchevel.com/...'
        assert!(!looks_like_url(
            "https://https://www.mairie-courchevel.com/a"
        ));
        assert!(!looks_like_url("http://http://example.com"));
        assert!(!looks_like_url("https://HTTPS://example.com"));
    }

    #[test]
    fn looks_like_url_still_accepts_the_neighbours_we_chose_not_to_reject() {
        // Cases proven in #162 but deliberately NOT rejected. AGN_003 is Kritik+Spec,
        // a publication blocker; a false positive here would reject a valid feed.
        assert!(looks_like_url("https://www.keolis_littoral.com")); // underscored host
        assert!(looks_like_url("http://localhost")); // single label
        assert!(looks_like_url("http://167")); // parses as 0.0.0.167
        assert!(looks_like_url("http://www.takethehop.coml/")); // misspelled TLD
    }

    #[test]
    fn looks_like_url_accepts_an_idn_host() {
        // The is_ascii() gate was removed in #144; the double-scheme check must not bring it back.
        assert!(looks_like_url("https://é.example.com"));
        assert!(looks_like_url("https://www.example.com"));
    }

    use super::*;

    #[test]
    fn build_row_map_aligns_headers() {
        let headers = vec!["a".to_string(), "b".to_string()];
        let row = vec![SmolStr::from("1"), SmolStr::from("2")];
        let map = build_row_map(&headers, &row);
        assert_eq!(get_field(&map, "a"), Some("1"));
        assert_eq!(get_field(&map, "b"), Some("2"));
    }

    #[test]
    fn derive_scope_key_supports_pipe_separated_keys() {
        let row = HashMap::from([
            ("from_stop_id".to_string(), "S1".to_string()),
            ("to_stop_id".to_string(), "S2".to_string()),
        ]);
        let key = derive_scope_key(&row, Some("from_stop_id|to_stop_id"));
        assert_eq!(key.as_deref(), Some("S1|S2"));
    }

    #[test]
    fn parse_gtfs_time_accepts_25h_clock() {
        let row = HashMap::from([("arrival_time".to_string(), "25:10:05".to_string())]);
        let parsed = parse_gtfs_time(&row, "arrival_time").expect("geçerli GTFS saati");
        assert_eq!(parsed, Some((25, 10, 5)));
    }

    #[test]
    fn parse_service_date_rejects_bad_format() {
        let row = HashMap::from([("start_date".to_string(), "2026-05-14".to_string())]);
        assert!(parse_service_date(&row, "start_date").is_err());
    }

    #[test]
    fn timezone_validation_works() {
        assert!(looks_like_iana_timezone("Europe/Istanbul"));
        assert!(!looks_like_iana_timezone("Mars/Base"));
    }

    #[test]
    fn contrast_ratio_black_white_is_high() {
        let ratio = wcag_contrast_ratio("000000", "FFFFFF").expect("hex geçerli");
        assert!(ratio > 20.0);
    }

    #[test]
    fn make_k2_notice_derives_scope_key_from_registry() {
        let row = HashMap::from([("trip_id".to_string(), "T1".to_string())]);
        let mut counter = 0;
        let notice = make_k2_notice(
            &mut counter,
            "STM_001",
            EntityType::Trip,
            Some("T1".to_string()),
            Some(&row),
            "stop_times.txt",
            Some(2),
            Some("trip_id"),
            Some("".to_string()),
            None,
            "trip_id zorunlu".to_string(),
            "Alanı doldurun.",
        );
        assert_eq!(notice.scope_key.as_deref(), Some("T1"));
        assert_eq!(notice.id, "k2/STM_001#1");
    }

    /// issue #82 (reopened) — `i-*` is NOT an ARBITRARY namespace.
    #[test]
    fn bcp47_rejects_invented_grandfathered_tags() {
        assert!(
            !super::looks_like_bcp47("i-foo"),
            "uydurma i-* well-formed DEĞİLDİR"
        );
        assert!(!super::looks_like_bcp47("i-whatever"));
        assert!(
            super::looks_like_bcp47("i-klingon"),
            "kayıtlı grandfathered etiket"
        );
        assert!(super::looks_like_bcp47("i-navajo"));
        assert!(
            super::looks_like_bcp47("en-GB-oed"),
            "düzensiz grandfathered etiket"
        );
        assert!(
            super::looks_like_bcp47("I-KLINGON"),
            "etiketler büyük/küçük harfe DUYARSIZ"
        );
        assert!(
            super::looks_like_bcp47("zh-min-nan"),
            "düzenli grandfathered etiket"
        );
        // ⚠️ Do not confuse with `zz-ZZ`: it is well-formed but unregistered (deliberate remainder).
        assert!(
            super::looks_like_bcp47("zz-ZZ"),
            "kayıt defteri DENETLENMEZ — belgelenmiş kalıntı"
        );
        // The normal langtag path must work independently of the grandfathered list.
        assert!(super::looks_like_bcp47("tr") && super::looks_like_bcp47("en-US"));
    }

    #[test]
    fn japanese_language_detection_uses_the_primary_subtag() {
        assert!(super::is_japanese_language_tag("ja"));
        assert!(super::is_japanese_language_tag("ja-JP"));
        assert!(super::is_japanese_language_tag("JA-Hrkt"));
        assert!(!super::is_japanese_language_tag("jaa"));
        assert!(!super::is_japanese_language_tag("en-ja"));
        assert!(!super::is_japanese_language_tag("ja--JP"));
    }

    #[test]
    fn japanese_text_detection_requires_kana_not_kanji() {
        assert!(super::has_japanese_kana("東京バス"));
        assert!(super::has_japanese_kana("とうきょう駅"));
        assert!(super::has_japanese_kana("ｼﾌﾞﾔ"));
        assert!(!super::has_japanese_kana("東京駅"));
        assert!(!super::has_japanese_kana("臺北車站"));
        assert!(!super::has_japanese_kana("Tokyo Station"));
    }

    /// issue #86 — GTFS `Email` type SYNTAX validation.
    /// Counterexample `a@@b.com` was ACCEPTED on current `main`: the old predicate
    /// used `split_once('@')` and treated everything after the first separator as the domain.
    #[test]
    fn email_rejects_invalid_addr_spec_shapes() {
        // Valid — supported dot-atom form.
        for ok in [
            "info@example.com",
            "a.b-c@sub.example.co.uk",
            "ticket+iyi_haber@example.org",
            "x!#$%&'*+-/=?^_`{|}~@example.com",
            "info@şehir.example",
        ] {
            assert!(
                super::looks_like_email(ok),
                "geçerli adres reddedildi: {ok}"
            );
        }
        // Invalid — each is a SEPARATE defect class.
        for bad in [
            "a@@b.com",         // reported counterexample: two separators
            "a@b@c.com",        // two separators, both in separate positions
            "@example.com",     // local part missing
            "a@",               // domain missing
            "a@b",              // single label
            "a@.example.com",   // empty label
            "a@example..com",   // empty label (in the middle)
            "a@-example.com",   // label starts with a hyphen
            "a@example-.com",   // label ends with a hyphen
            "a@example.1",      // numeric TLD
            "a@example.c",      // one-letter TLD
            ".a@example.com",   // leading dot
            "a.@example.com",   // trailing dot
            "a..b@example.com", // empty atom
            "a b@example.com",  // whitespace
            "a\t@example.com",  // tab
            "noatsign.example.com",
            "",
        ] {
            assert!(
                !super::looks_like_email(bad),
                "geçersiz adres kabul edildi: {bad:?}"
            );
        }
    }

    /// issue #82 — shared type helpers were BROADER than the standards they represent.
    /// For each: the reported INVALID example + a valid boundary case.
    #[test]
    fn type_helpers_reject_values_outside_the_declared_types() {
        // 1) BCP 47: the primary subtag must be ALPHABETIC.
        // VALID (RFC 5646 well-formed)
        for ok in [
            "tr",
            "en-US",
            "zh-Hant-TW",
            "es-419",
            "x-private",
            "de-CH-1901",
            "en-a-bbb",
            "en-a-bbb-x-priv",
            "i-klingon",
            "sl-rozaj-biske",
        ] {
            assert!(
                super::looks_like_bcp47(ok),
                "geçerli dil etiketi reddedildi: {ok}"
            );
        }
        // INVALID — each is a separate syntax rule
        for bad in [
            "123",
            "1-tr",
            "en-a",
            "en-a-b-c",
            "en-a-bbb-a-ccc",
            "en--US",
            "en-",
            "x-",
            "-en",
            "en-toolongsubtag",
            "en-US-",
        ] {
            assert!(
                !super::looks_like_bcp47(bad),
                "geçersiz dil etiketi kabul edildi: {bad:?}"
            );
        }

        // 5) f64: non-finite values are outside the type.
        assert!(super::parse_f64_col("NaN").is_err(), "NaN koordinat olamaz");
        assert!(super::parse_f64_col("inf").is_err());
        assert!(super::parse_f64_col("-Infinity").is_err());
        assert_eq!(super::parse_f64_col("41.5").unwrap(), Some(41.5));
        assert_eq!(super::parse_f64_col("").unwrap(), None);

        // 6) Time: minute/second TWO digits; NO limit on hours (service day).
        assert!(
            !super::gtfs_time_widths_ok(&["1", "2", "3"]),
            "1:2:3 tip dışı"
        );
        assert!(!super::gtfs_time_widths_ok(&["08", "5", "00"]));
        assert!(
            super::gtfs_time_widths_ok(&["8", "05", "00"]),
            "H:MM:SS geçerli"
        );
        assert!(super::gtfs_time_widths_ok(&["08", "05", "00"]));
        assert!(
            super::gtfs_time_widths_ok(&["145", "00", "00"]),
            "çok günlü tren seferi — saat basamağı SINIRLANMAZ"
        );

        // 4) ISO 4217: a fabricated code must not pass.
        assert!(!super::is_iso4217("ZZZ"), "ZZZ ISO 4217 kodu değildir");
        assert!(
            !super::is_iso4217("TRL"),
            "tedavülden kalkmış kod bilinçli olarak dışarıda"
        );
        assert!(!super::is_iso4217("usd"), "kodlar BÜYÜK harftir");
        assert!(super::is_iso4217("TRY") && super::is_iso4217("EUR") && super::is_iso4217("USD"));

        // 2/3) Calendar: format is correct but the DAY does not exist.
        assert!(
            !super::is_valid_calendar_date(2026, 13, 40),
            "13. ay yoktur"
        );
        assert!(
            !super::is_valid_calendar_date(2026, 2, 31),
            "31 Şubat yoktur"
        );
        assert!(
            !super::is_valid_calendar_date(2026, 2, 29),
            "2026 artık yıl DEĞİL"
        );
        assert!(super::is_valid_calendar_date(2024, 2, 29), "2024 artık yıl");
        assert!(super::is_valid_calendar_date(2026, 12, 31));
    }

    #[test]
    fn looks_like_url_requires_a_web_scheme() {
        // Spec: "A fully qualified URL that includes http:// or https://."
        assert!(super::looks_like_url("http://a.com"));
        assert!(super::looks_like_url("https://a.com/x?y=1#z"));
        assert!(
            super::looks_like_url("HTTPS://A.COM"),
            "şema adı büyük/küçük harfe duyarsız"
        );
        // issue #92: leading/trailing whitespace is NO LONGER TRIMMED. This test preserved old
        // tolerance; the spec says "special characters must be correctly escaped", and whitespace
        // is an unescaped character. PRESENCE diagnosis still trims — `"   "` is a MISSING field,
        // not an invalid URL; the caller preserves that distinction with two readings.
        assert!(
            !super::looks_like_url("  https://a.com  "),
            "ham değerde çıplak boşluk"
        );
        assert!(super::looks_like_url("https://a.com"));

        // All of these are VALID for `Url::parse` and passed validation before scheme checking
        // was added (T5 gap #1, 2026-08-03 measurement).
        assert!(!super::looks_like_url("mailto:info@a.com"));
        assert!(!super::looks_like_url("ftp://a.com/x"));
        assert!(!super::looks_like_url("file:///etc/passwd"));
        assert!(!super::looks_like_url("javascript:alert(1)"));

        // issue #80 — the SECOND half of the rule: special characters must be escaped CORRECTLY.
        // `Url::parse` returned Ok for all of these (after normalization).
        assert!(
            super::looks_like_url("https://a.example/a%20b"),
            "doğru yüzde kodlaması"
        );
        assert!(
            !super::looks_like_url("https://a.example/a b"),
            "kaçırılmamış boşluk"
        );
        assert!(
            !super::looks_like_url("https://a.example/a\"b"),
            "kaçırılmamış tırnak"
        );
        assert!(
            !super::looks_like_url("https://a.example/a<b>"),
            "kaçırılmamış açılı ayraç"
        );
        assert!(
            !super::looks_like_url("https://a.example/a%zzb"),
            "BOZUK yüzde kodlaması"
        );
        assert!(
            !super::looks_like_url("https://a.example/a%2"),
            "eksik yüzde kodlaması"
        );
        assert!(
            !super::looks_like_url("https://a.example/a\\b"),
            "ters bölü çıplak geçemez"
        );
        // #144: non-ASCII is NO LONGER REJECTED. The rule says "special characters"; `ü`, `ş`,
        // and full-width `ｗ` are not in RFC 3986's delimiter set. The distinguishing criterion
        // is a deterministic canonical ASCII equivalent: NFKC, punycode, and percent encoding
        // are all unambiguous — the parser does not INVENT an interpretation.
        assert!(
            super::looks_like_url("https://example.com/güzergah"),
            "ASCII dışı yol: kanonik karşılığı var"
        );
        assert!(
            super::looks_like_url("https://example.com/g%C3%BCzergah"),
            "yüzde kodlanmış eşdeğeri de GEÇERLİ"
        );
        assert!(
            super::looks_like_url("https://şehir.example/yol"),
            "IDN alan adı GEÇERLİ"
        );
        assert!(
            super::looks_like_url("https://xn--ehir-jua.example/yol"),
            "punycode eşdeğeri GEÇERLİ"
        );
        // Two real corpus cases (#144, full catalogue run).
        assert!(
            super::looks_like_url("https://ｗｗｗ.city.chikuma.lg.jp"),
            "jbda-chikumacity: tam genişlikli www, NFKC ile www.city.chikuma.lg.jp"
        );
        assert!(
            super::looks_like_url("https://thyon4vallées.ch"),
            "mdb-2898: punycode ile xn--thyon4valles-keb.ch"
        );
        // 🔑 LIMIT OF THE RELAXATION: non-ASCII is free, but no unescaped DELIMITER remains.
        // These three must be rejected even inside a non-ASCII value, or the relaxation is invalid.
        assert!(
            !super::looks_like_url("https://şehir.example/a b"),
            "ASCII dışı host çıplak boşluğu MEŞRULAŞTIRMAZ"
        );
        assert!(
            !super::looks_like_url("https://şehir.example/a%zz"),
            "ASCII dışı host bozuk yüzde kodlamasını MEŞRULAŞTIRMAZ"
        );
        assert!(
            !super::looks_like_url("şehir.example/yol"),
            "şema yokluğu ASCII dışı olmaktan bağımsız"
        );
        assert!(!super::looks_like_url("foo:bar"));
        assert!(
            !super::looks_like_url("jrutil://invalid"),
            "korpusta görülen yer tutucu"
        );

        // Scheme-less values were already rejected; preserve the behavior.
        assert!(!super::looks_like_url("www.example.com"));
        assert!(!super::looks_like_url("a.com"));
        assert!(!super::looks_like_url(""));
        // A value with an `http://` prefix that cannot parse as a URL is also rejected.
        assert!(!super::looks_like_url("http://"));

        // Value beginning with a multibyte character: slicing (`&s[..7]`) would split a byte
        // boundary and PANIC. Fields can contain free text.
        assert!(!super::looks_like_url("Üniversite Kampüsü"));
        assert!(!super::looks_like_url("東京駅"));
        assert!(!super::looks_like_url("ü"));
    }

    #[test]
    fn looks_like_phone_accepts_dialable_vanity_text() {
        // The spec's OWN example — rejected until 2026-08-03 (PTH_017 format bug).
        assert!(super::looks_like_phone("503-238-RIDE"));
        assert!(super::looks_like_phone("+1 800 FLOWERS"));
        assert!(super::looks_like_phone("1-800-COLLECT"));
        // #95: MULTIPLE vanity letter groups — all uppercase and in the tail.
        assert!(
            super::looks_like_phone("1-800-GO-FEDEX"),
            "iki vanity grubu kabul edilmeli"
        );
        assert!(
            super::looks_like_phone("1 800 GO FEDEX"),
            "ayırıcı boşluk da olabilir"
        );
        assert!(super::looks_like_phone("+1-800-NEW-CARS"));
        // NO uppercase requirement for one group — preserve old behavior exactly (the accepted
        // set only expanded; no value was newly rejected).
        assert!(super::looks_like_phone("503-238-ride"));
        // Numeric-only values: preserve behavior.
        assert!(super::looks_like_phone("+90 212 555 12 34"));
        assert!(super::looks_like_phone("(503) 238-7433"));
        assert!(super::looks_like_phone("555.12.34"));
    }

    #[test]
    fn looks_like_phone_rejects_descriptive_text() {
        // Spec: "must not contain any other descriptive text."
        assert!(
            !super::looks_like_phone("Call 503-238-1234"),
            "harf grubu BAŞTA"
        );
        assert!(
            !super::looks_like_phone("555-1234 ext 99"),
            "harf grubu ORTADA"
        );
        assert!(
            !super::looks_like_phone("Call us at 503 238 1234"),
            "birden çok harf grubu"
        );
        // #95 boundary: a multi-group TAIL counts as vanity only when UPPERCASE; prose cannot pass.
        assert!(
            !super::looks_like_phone("1234 call us now"),
            "küçük harf kuyruk düzyazıdır"
        );
        assert!(
            !super::looks_like_phone("555 1234 Call Us"),
            "baş harfi büyük düzyazı vanity değil"
        );
        assert!(
            !super::looks_like_phone("GO FEDEX 1 800"),
            "harf kuyrukta değil, BAŞTA"
        );
        assert!(
            !super::looks_like_phone("1-800-GO-FEDEX-now"),
            "kuyruğun son grubu küçük harf"
        );
        // Two values ACTUALLY present in the corpus (mdb-2337, mdb-992) — the rule fires
        // CORRECTLY on them and must continue to do so after the fix.
        assert!(!super::looks_like_phone(
            "80000078 (Liepājā); 80000079 (Pierīgā)"
        ));
        // Too few digits / no digits / empty: preserve behavior.
        assert!(!super::looks_like_phone("RIDE"), "beş haneden kısa");
        assert!(
            !super::looks_like_phone("FLOWERS"),
            "hiç rakam yok — telefon değil"
        );
        assert!(!super::looks_like_phone("12"));
        assert!(!super::looks_like_phone(""));
    }

    #[test]
    fn iso4217_decimals_follow_the_currency() {
        use super::{amount_has_iso4217_decimals as ok, iso4217_minor_unit as mu};
        assert_eq!(mu("EUR"), Some(2));
        assert_eq!(mu("JPY"), Some(0));
        assert_eq!(mu("KWD"), Some(3));
        // issue #82: the assumption "no value other than 0/2/3" was WRONG — measured.
        assert_eq!(mu("CLF"), Some(4), "CLF dört ondalıklıdır");
        assert_eq!(mu("UYW"), Some(4), "UYW dört ondalıklıdır");
        // Code validity and decimal precision are SEPARATE concepts: both return `None`, but for different reasons.
        assert_eq!(mu("ZZZ"), None, "bilinmeyen kod — 2 VARSAYILMAZ");
        assert_eq!(
            mu("XAU"),
            None,
            "kıymetli maden: ondalık TANIMSIZ (kaynakta N.A.)"
        );
        assert!(super::is_iso4217("XAU"), "…ama XAU GEÇERLİ bir koddur");
        // 🔴 The hand-written list was wrong: BGN is not ACTIVE in the 2026-01-01 list
        // (Bulgaria adopted the euro). The generated table takes this from the source.
        assert!(!super::is_iso4217("BGN"), "BGN artık aktif kod değil");

        assert!(ok("2.50", "EUR"));
        assert!(ok("150", "JPY"), "yen ondalık taşımaz");
        assert!(ok("1.500", "KWD"));

        // Real violations measured in the corpus.
        assert!(!ok("0.9", "EUR"), "EUR iki basamak ister");
        assert!(!ok("6.7", "HKD"));
        assert!(!ok("8", "EUR"));
        assert!(!ok("100.00", "JPY"), "yen ondalık TAŞIMAMALI");
        // Decimal validation stays SILENT for an invalid code — FAR_003/FPD_003 reports it.
        assert!(ok("1.23456", "ZZZ"), "geçersiz kod iki kez raporlanmaz");
        assert!(ok("1.2", "XAU"), "ondalığı tanımsız kodda denetim yapılmaz");
        assert!(!ok("0.0000", "HKD"), "fazla basamak da ihlal");

        // Non-numeric / empty values belong to FPD_002 — silent here.
        assert!(ok("", "EUR"));
        assert!(ok("abc", "EUR"));
        assert!(ok("2.50", ""), "para birimi yoksa karar verilemez");
    }
}
