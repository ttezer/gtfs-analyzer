use std::collections::{HashMap, HashSet};
use std::io::Read;

use crate::decompress_guard::{DecompressionLimits, GuardedReader, DEFAULT_DECOMPRESSION_LIMITS};
use gtfs_config::ValidatorConfig;
use gtfs_core::{EntityType, FatalCode, FatalError, Notice, PartialReport, Severity};
use smol_str::SmolStr;

/// Debug-only K1 tracing. It is fully compiled out in release WASM builds
/// (with debug_assertions disabled) and native builds — zero runtime cost and no effect on
/// Notices or control flow. (It was disabled because WASM stack symbolisation with DevTools
/// open made every console call cripplingly slow during profiling.)
macro_rules! k1dbg {
    ($($arg:tt)*) => {{
        #[cfg(all(target_family = "wasm", debug_assertions))]
        web_sys::console::log_1(&format!($($arg)*).into());
    }};
}

// ── GTFS file lists ───────────────────────────────────────────────────────────

const REQUIRED_FILES: &[&str] = &[
    "agency.txt",
    "stops.txt",
    "routes.txt",
    "trips.txt",
    "stop_times.txt",
];

const CALENDAR_FILES: &[&str] = &["calendar.txt", "calendar_dates.txt"];

/// An input such as `agency.csv` that carries a GTFS file body but does not have a `.txt`
/// extension. It is independent of location; the caller handles root/subdirectory layout.
fn gtfs_stem_with_wrong_extension(path: &str) -> bool {
    let base = path
        .rsplit(['/', '\\'])
        .next()
        .unwrap_or(path)
        .to_ascii_lowercase();
    let Some((stem, _ext)) = base.rsplit_once('.') else {
        return false;
    };
    KNOWN_FILES
        .iter()
        .any(|k| k.strip_suffix(".txt") == Some(stem))
}

pub(crate) const KNOWN_FILES: &[&str] = &[
    "agency.txt",
    "stops.txt",
    "routes.txt",
    "trips.txt",
    "stop_times.txt",
    "calendar.txt",
    "calendar_dates.txt",
    "shapes.txt",
    "frequencies.txt",
    "transfers.txt",
    "fare_attributes.txt",
    "fare_rules.txt",
    "pathways.txt",
    "levels.txt",
    "feed_info.txt",
    "translations.txt",
    "attributions.txt",
    "route_networks.txt",
    // Fares v2
    "areas.txt",
    "stop_areas.txt",
    "networks.txt",
    "rider_categories.txt",
    "fare_media.txt",
    "fare_products.txt",
    "fare_leg_rules.txt",
    "fare_leg_join_rules.txt",
    "fare_transfer_rules.txt",
    "timeframes.txt",
    // Flex
    "booking_rules.txt",
    "location_groups.txt",
    "location_group_stops.txt",
    // GTFS-JP extensions (Japanese standard)
    "agency_jp.txt",
    "routes_jp.txt",
    "office_jp.txt",
    "pattern_jp.txt",
];

// ── Types ─────────────────────────────────────────────────────────────────────

/// Raw CSV data: headers + rows. Type/enum checks run in K2.
#[derive(Debug, Default)]
pub struct RawFile {
    pub name: String,
    pub headers: Vec<String>,
    /// Each row contains raw string values aligned with the headers by index.
    /// SmolStr: fields ≤22 bytes are stored inline (no heap allocation); longer ones use Arc<str>.
    ///
    /// NOTE: for very large files (stop_times.txt), this field is left EMPTY to save memory;
    /// the raw body is kept in `raw_text` and parsed by K2 as a stream (OOM fix Plan A).
    pub rows: Vec<Vec<SmolStr>>,
    /// Uncompressed file size (bytes). Large files may be truncated by READ_LIMIT.
    pub bytes: u32,
    /// BOM-free, UTF-8-validated raw text (including the header) for files parsed as streams
    /// (stop_times.txt). K2 processes it line by line when `rows` is empty.
    /// `None` for all other files.
    pub raw_text: Option<String>,
    /// The REAL entry name inside the ZIP — populated only when it differs from `name`.
    ///
    /// In a wrapped feed (`TestFlex1/stop_times.txt`), K1 drops the root prefix and processes
    /// the file as `stop_times.txt`. K2 reopens the ZIP to stream large files; `by_name("stop_times.txt")`
    /// finds NOTHING in a wrapped archive, so the file was silently treated as empty. Measured
    /// in the 13th corpus run: in **all 44 of the 44** feeds triggering ARC_024, not one `STM_*`
    /// rule fired.
    pub zip_entry_name: Option<String>,
}

pub type RawFiles = HashMap<String, RawFile>;

#[derive(Debug)]
pub struct K1Result {
    pub files: RawFiles,
    pub notices: Vec<Notice>,
    pub partial: PartialReport,
    /// Files discovered by K1; the dependency-aware recovery plan for broken/missing optional
    /// files is built from this set.
    pub present_files: HashSet<String>,
    /// Temporarily accept the conditional absence of `stops.txt` in K1 only when structurally
    /// valid Flex locations justify it; the file name or a merely readable polygon is not enough.
    /// After K2, the full pipeline also verifies that at least one ID in this set is used by
    /// `stop_times.location_id`.
    pub has_valid_locations_geojson: bool,
    /// Unique string `id` values whose required Feature members and geometry are complete.
    /// While `geojson_location_ids` retains every written ID for XFL_025, this narrower set is
    /// only evidence for the `stops.txt` Flex exemption.
    pub valid_geojson_location_ids: std::collections::HashSet<String>,
    /// Set of locations.geojson feature 'id' values (XFL_025: stop_times.location_id cross-ref).
    pub geojson_location_ids: std::collections::HashSet<String>,
    /// locations.geojson feature 'id' → GEOMETRY (moved to K6 for Pa1fdaa0d).
    ///
    /// ⚠️ 2026-08-06 external audit: because geometry was not retained, the rule that "a geojson
    /// region + time window + pickup/drop_off behavior cannot overlap in the same trip" was
    /// **mistakenly marked OUT OF SCOPE**. The architecture was incomplete, not the evidence —
    /// every input to the rule is inside the feed. This field closes that gap.
    pub geojson_geometries: std::collections::HashMap<String, LocationGeometry>,
    /// Unknown root-level `.txt` files and their parsed headers. K2 cannot retain
    /// unknown files because it only materialises typed GTFS records, but K4 needs
    /// their names/headers for the profile-aware GTFS-JP namespace check.
    pub custom_file_headers: std::collections::HashMap<String, Vec<String>>,
}

/// Geometry of a `locations.geojson` feature: outer rings + bounding box.
///
/// Holes are deliberately not retained: the outer boundary is enough for the overlap test, and
/// ignoring holes makes an "overlap" decision more INCLUSIVE, not merely more prone to FALSE
/// POSITIVES. Because the rule is a PROHIBITION, that direction is unsafe, so the overlap
/// decision is also verified by intersection/containment (see `polygons_overlap`).
#[derive(Debug, Clone, Default)]
pub struct LocationGeometry {
    /// The OUTER ring of each Polygon (there may be several in a MultiPolygon).
    pub outer_rings: Vec<Vec<(f64, f64)>>,
    /// (min_lon, min_lat, max_lon, max_lat) — cheap pre-filter.
    pub bbox: (f64, f64, f64, f64),
}

// ── Notice helper ──────────────────────────────────────────────────────────────

// Notice fields stay in the same order as the canonical emit table; the parameter struct keeps
// calls to this low-level helper readable.
#[allow(clippy::too_many_arguments)]
fn make_notice(
    counter: &mut u32,
    rule_id: &str,
    entity_type: EntityType,
    entity_id: Option<String>,
    file: Option<&str>,
    line: Option<u64>,
    field: Option<&str>,
    observed_value: Option<String>,
    message: String,
    remediation: &str,
) -> Notice {
    // K1 does not produce scope_key or expected_value — both are always None.
    crate::notice_factory::build(
        "K1",
        Some("k1"),
        counter,
        rule_id,
        entity_type,
        entity_id,
        None,
        file.map(str::to_string),
        line,
        field.map(str::to_string),
        observed_value,
        None,
        message,
        remediation,
    )
}

// ── UTF-8 BOM ──────────────────────────────────────────────────────────────────

const UTF8_BOM: &[u8] = &[0xEF, 0xBB, 0xBF];

/// Maximum buffer to preallocate from the decompressed size declared in the ZIP central
/// directory (attacker-controlled). Because `zf.size()` is untrusted, an archive could force
/// a large Vec allocation before the first byte is read simply by *declaring* an enormous size.
/// We cap the declaration here; the Vec still grows as real bytes arrive, so legitimate large
/// files behave the same and only the initial capacity is limited.
const MAX_PREALLOC: usize = 64 * 1024 * 1024; // 64 MiB

/// Fixed safety budgets for the central directory and expensive optional entries.
/// These do not narrow the scope of normal GTFS feeds; they only prevent an attacker from
/// producing unbounded metadata/JSON and geometric comparisons during validation.
const MAX_ZIP_ENTRIES: usize = 100_000;
const MAX_ZIP_METADATA_BYTES: usize = 16 * 1024 * 1024;
const MAX_GEOJSON_BYTES: u64 = 64 * 1024 * 1024;

/// Initial capacity for a K1 raw-file buffer: cap the declared size at [`MAX_PREALLOC`] and
/// return at least 1 (a zero-capacity allocation is pointless).
#[inline]
fn prealloc_capacity(uncompressed_hint: usize) -> usize {
    uncompressed_hint.clamp(1, MAX_PREALLOC)
}

/// Local file header signature and fixed length.
const LFH_SIGNATURE: u32 = 0x0403_4b50;
const LFH_LEN: usize = 30;
/// Central directory header signature and fixed length.
const CDH_SIGNATURE: u32 = 0x0201_4b50;
const CDH_LEN: usize = 46;
/// End-of-central-directory signature and fixed length.
const EOCD_SIGNATURE: u32 = 0x0605_4b50;
const EOCD_LEN: usize = 22;
/// EOCD can be at most the comment length (64 KB) before the end of the file.
const EOCD_SEARCH_WINDOW: usize = 65_557;
/// Data descriptor signature. According to APPNOTE, this signature is OPTIONAL; both forms are read.
const DATA_DESCRIPTOR_SIGNATURE: u32 = 0x0807_4b50;
/// Data descriptor flag (general purpose bit flag, bit 3).
const FLAG_DATA_DESCRIPTOR: u16 = 0x0008;
/// zip64 sentinel — the real size is carried in the extra field; the 32-bit field cannot be read.
const ZIP64_SENTINEL: u32 = 0xFFFF_FFFF;

/// Identifies the layer of corruption when `ZipArchive::new` fails.
///
/// 🔴 Why this is needed: the `zip` crate (2.4.2) also returns "Could not find EOCD" for a
/// corrupt entry, and passing that message through sent users in the WRONG direction. In the
/// measurement (`tld-5895`, the corpus's only `ZipUnreadable` feed), the EOCD at offset 855.231
/// was INTACT and all 9 central-directory entries were intact; only the local header of
/// `stop_times.txt` was corrupt (offset 570.618, zero bytes instead of the signature) — the
/// previous entry's compressed data had overrun it. `unzip -t` reports the same offset; we used
/// to say "the end of the archive was truncated."
///
/// `None` → nothing more can be determined from the raw bytes; the caller only returns the
/// library message. zip64 archives are SKIPPED: 32-bit EOCD fields contain sentinel values, the
/// real offset is in the zip64 record, and it cannot be read here; calling the unreadable part
/// "corrupt" would be a false diagnosis. The `diagnose_unreadable_zip` result contains Turkish
/// text (the pipeline message) plus the type and parameters for translation. The type selects
/// the fatal-template variant (such as `archive.local_header`).
#[derive(Debug)]
struct ZipDiagnosis {
    kind: &'static str,
    params: Vec<(&'static str, String)>,
    text: String,
}

fn diagnose_unreadable_zip(zip_bytes: &[u8]) -> Option<ZipDiagnosis> {
    let read_u16 = |at: usize| -> Option<u16> {
        zip_bytes
            .get(at..at + 2)
            .map(|b| u16::from_le_bytes([b[0], b[1]]))
    };
    let read_u32 = |at: usize| -> Option<u32> {
        zip_bytes
            .get(at..at + 4)
            .map(|b| u32::from_le_bytes([b[0], b[1], b[2], b[3]]))
    };

    // 1) EOCD: search backwards from the end of the file through the comment window.
    let start = zip_bytes.len().saturating_sub(EOCD_SEARCH_WINDOW);
    let eocd = (start..zip_bytes.len().saturating_sub(EOCD_LEN - 1))
        .rev()
        .find(|&i| read_u32(i) == Some(EOCD_SIGNATURE))?;

    let cd_offset = read_u32(eocd + 16)?;
    let entry_count = read_u16(eocd + 10)?;
    if cd_offset == ZIP64_SENTINEL || entry_count == u16::MAX {
        return None; // zip64 → the real values are in another record
    }
    let cd_offset = cd_offset as usize;

    // 2) Is the central directory where EOCD points?
    if read_u32(cd_offset) != Some(CDH_SIGNATURE) {
        return Some(ZipDiagnosis {
            kind: "cd_signature",
            params: vec![("cd_offset", cd_offset.to_string()), ("eocd", eocd.to_string())],
            text: format!(
                "merkez dizin {cd_offset}. bayta işaret ediyor ama orada merkez dizin imzası yok (EOCD {eocd}. baytta okunabildi)"
            ),
        });
    }

    // 3) Walk the central-directory entries: is each local header in place?
    let mut cursor = cd_offset;
    for _ in 0..entry_count {
        if read_u32(cursor) != Some(CDH_SIGNATURE) {
            return Some(ZipDiagnosis {
                kind: "cd_truncated",
                params: vec![
                    ("cursor", cursor.to_string()),
                    ("entry_count", entry_count.to_string()),
                ],
                text: format!(
                    "merkez dizin {cursor}. bayttan sonra okunamıyor (beklenen girdi sayısı: {entry_count})"
                ),
            });
        }
        let name_len = read_u16(cursor + 28)? as usize;
        let extra_len = read_u16(cursor + 30)? as usize;
        let comment_len = read_u16(cursor + 32)? as usize;
        let local_offset = read_u32(cursor + 42)?;
        let name = zip_bytes
            .get(cursor + CDH_LEN..cursor + CDH_LEN + name_len)
            .map_or_else(
                || "?".to_string(),
                |b| String::from_utf8_lossy(b).into_owned(),
            );

        if local_offset != ZIP64_SENTINEL && read_u32(local_offset as usize) != Some(LFH_SIGNATURE)
        {
            return Some(ZipDiagnosis {
                kind: "local_header",
                text: format!(
                    "'{name}' yerel başlığı {local_offset}. bayta göre bozuk (imza yok); EOCD ve merkez dizin sağlam, bir önceki girdinin sıkıştırılmış verisi taşmış olabilir"
                ),
                params: vec![("name", name), ("local_offset", local_offset.to_string())],
            });
        }
        cursor += CDH_LEN + name_len + extra_len + comment_len;
    }

    // EOCD, the central directory, and all local headers are in place: the corruption is in
    // the entry DATA.
    Some(ZipDiagnosis {
        kind: "entry_data",
        params: Vec::new(),
        text: "EOCD, merkez dizin ve tüm yerel başlıklar yerinde; bozukluk girdilerin sıkıştırılmış verisinde olabilir"
            .to_string(),
    })
}

/// ARC_036: does an entry's STREAM view (local header + data descriptor) contradict the
/// central directory?
///
/// `entries`, merkez dizinden okunan `(header_start, compressed_size, uncompressed_size, crc32)`
/// quadruples read from the central directory. Only entries with the data-descriptor flag set
/// are inspected; an entry is inconsistent if the descriptor is absent or its triple does not
/// match the central directory.
///
/// 🔑 Setting the flag is NOT A DEFECT BY ITSELF — 40% of the corpus sample uses descriptors
/// correctly (local sizes are 0, and the descriptor is present and correct). Firing without a
/// comparison would produce a false finding on ~1,700 feeds.
///
/// zip64 entries are SKIPPED: 32-bit fields contain the `0xFFFFFFFF` sentinel, the real value
/// is in the extra field and cannot be read here — treating an unreadable value as
/// "inconsistent" would produce a false positive.
fn count_inconsistent_stream_headers(zip_bytes: &[u8], entries: &[(u64, u64, u64, u32)]) -> usize {
    let mut inconsistent = 0usize;
    for &(header_start, csize, usize_declared, crc) in entries {
        let Ok(off) = usize::try_from(header_start) else {
            continue;
        };
        let Some(lfh) = zip_bytes.get(off..off.saturating_add(LFH_LEN)) else {
            continue;
        };
        if u32::from_le_bytes([lfh[0], lfh[1], lfh[2], lfh[3]]) != LFH_SIGNATURE {
            continue;
        }
        let flag = u16::from_le_bytes([lfh[6], lfh[7]]);
        if flag & FLAG_DATA_DESCRIPTOR == 0 {
            continue;
        }
        let local_csize = u32::from_le_bytes([lfh[18], lfh[19], lfh[20], lfh[21]]);
        let local_usize = u32::from_le_bytes([lfh[22], lfh[23], lfh[24], lfh[25]]);
        let name_len = u16::from_le_bytes([lfh[26], lfh[27]]) as usize;
        let extra_len = u16::from_le_bytes([lfh[28], lfh[29]]) as usize;
        if local_csize == ZIP64_SENTINEL
            || local_usize == ZIP64_SENTINEL
            || csize >= u64::from(ZIP64_SENTINEL)
            || usize_declared >= u64::from(ZIP64_SENTINEL)
        {
            continue;
        }
        // The end of the data is found from the central-directory size; the local field is zero
        // in a compliant streaming ZIP and cannot be used to calculate the position.
        let Ok(csize_usize) = usize::try_from(csize) else {
            continue;
        };
        let data_end = off
            .saturating_add(LFH_LEN)
            .saturating_add(name_len)
            .saturating_add(extra_len)
            .saturating_add(csize_usize);
        let Some(head) = zip_bytes.get(data_end..data_end.saturating_add(4)) else {
            inconsistent += 1; // there is not even room for the descriptor
            continue;
        };
        let body_off = if u32::from_le_bytes([head[0], head[1], head[2], head[3]])
            == DATA_DESCRIPTOR_SIGNATURE
        {
            data_end.saturating_add(4)
        } else {
            data_end
        };
        let Some(body) = zip_bytes.get(body_off..body_off.saturating_add(12)) else {
            inconsistent += 1;
            continue;
        };
        let d_crc = u32::from_le_bytes([body[0], body[1], body[2], body[3]]);
        let d_csize = u32::from_le_bytes([body[4], body[5], body[6], body[7]]);
        let d_usize = u32::from_le_bytes([body[8], body[9], body[10], body[11]]);
        if u64::from(d_csize) != csize || u64::from(d_usize) != usize_declared || d_crc != crc {
            inconsistent += 1;
        }
    }
    inconsistent
}

/// Converts a GuardedReader read error to a Fatal: ARC_029 (DecompressionLimit) if the guard
/// tripped, otherwise the raw ZIP read error (ARC_001 ZipUnreadable).
fn read_fatal<R: Read>(
    guarded: &GuardedReader<'_, R>,
    raw_name: &str,
    e: &std::io::Error,
) -> FatalError {
    match guarded.tripped() {
        Some(trip) => {
            let (variant, values) = trip.fatal_params();
            values.into_iter().fold(
                FatalError::new(
                    FatalCode::DecompressionLimit,
                    format!(
                        "'{raw_name}' sıkıştırma koruması sınırını aştı: {}",
                        trip.describe()
                    ),
                )
                .variant(variant)
                .param("file", raw_name),
                |err, (key, value)| err.param(key, value),
            )
        }
        None => FatalError::new(
            FatalCode::ZipUnreadable,
            format!("'{raw_name}' okunamadı: {e}"),
        )
        .variant("entry_read")
        .param("file", raw_name)
        .param("detail", e),
    }
}

/// Truncates a partially read buffer to the last COMPLETE UTF-8 character boundary.
///
/// Called ONLY for `is_zip_stream` (the 8 KB header read): the incomplete tail there is caused
/// by OUR cut-off point, not by a defect in the file.
///
/// **Does not HIDE genuinely invalid bytes.** `Utf8Error` distinguishes two cases:
/// - `error_len() == None` → "input ended unexpectedly" = an incomplete character
///   (it can occur only at the END of the buffer) → truncate; no problem.
/// - `error_len() == Some(n)` → invalid byte sequence in the body → DO NOT TOUCH; ARC_002 /
///   Utf8Critical still fires as before.
///
/// Because `from_utf8` reports the FIRST error, a genuine body error returns `Some` and is not
/// truncated — genuine violations always win.
///
/// Regression: mdb-3176 (JP), mdb-1834 (LT), mdb-1831 (TH) — all had valid UTF-8 in
/// trips.txt but were made fatal by a cut at bytes 8190–8191.
fn truncate_to_utf8_boundary(buf: &mut Vec<u8>) {
    if let Err(e) = std::str::from_utf8(buf) {
        if e.error_len().is_none() {
            buf.truncate(e.valid_up_to());
        }
    }
}

fn strip_bom(bytes: &[u8]) -> (&[u8], bool) {
    if bytes.starts_with(UTF8_BOM) {
        (&bytes[3..], true)
    } else {
        (bytes, false)
    }
}

fn has_malformed_eol(bytes: &[u8]) -> bool {
    bytes
        .iter()
        .enumerate()
        .any(|(i, b)| *b == b'\r' && bytes.get(i + 1) != Some(&b'\n'))
}

// ── CSV tokenizer ──────────────────────────────────────────────────────────────

/// RFC 4180 violation accumulator — `ARC_033`. ONE notice per FILE (`Arc032Acc` pattern).
///
/// 🔴 **Why here, NOT in the parsed value:** detection needs to know the field BOUNDARIES.
/// After parsing, `"12"" Street"` (VALID) and `12" Street` (VIOLATION) produce the SAME value:
/// `12" Street`. A value-based check cannot distinguish them and would mark valid data as a
/// violation. Therefore the decision is made in the tokenizer, where it is known whether the
/// field was quoted or unquoted.
#[derive(Default, Clone)]
pub(crate) struct Rfc4180Acc {
    pub rows: u64,
    pub first_line: Option<u64>,
    pub example: Option<String>,
    /// Violation type — two separate spec sentences (see the `ARC_033` card).
    pub kind: Option<&'static str>,
    last_line: u64,
}

/// GTFS File Requirements uses two separate sentences, and both are in scope for this rule:
/// *"Field values that contain quotation marks or commas must be enclosed within quotation
/// marks."* and *"In addition, each quotation mark in the field value must be preceded with
/// a quotation mark."*
const ARC033_REMEDIATION: &str =
    "Quote any field value that contains a quotation mark or a comma, and double every quotation mark inside it (\"12\"\" Street\").";

pub(crate) const RFC4180_BARE_QUOTE: &str = "tırnaksız alan tırnak işareti içeriyor";
pub(crate) const RFC4180_AFTER_CLOSE: &str = "kapanış tırnağından sonra karakter var";

pub(crate) fn rfc4180_issue_code(kind: &str) -> &'static str {
    match kind {
        RFC4180_AFTER_CLOSE => "characters_after_closing_quote",
        _ => "bare_quote",
    }
}

impl Rfc4180Acc {
    pub(crate) fn observe(&mut self, line: u64, kind: &'static str, example: &str) {
        if self.last_line != line || self.rows == 0 {
            self.last_line = line;
            self.rows += 1;
            if self.first_line.is_none() {
                self.first_line = Some(line);
            }
        }
        if self.kind.is_none() {
            self.kind = Some(kind);
        }
        if self.example.is_none() {
            let e: String = example.chars().take(40).collect();
            self.example = Some(e);
        }
    }

    #[cfg(feature = "parallel")]
    pub(crate) fn merge(&mut self, other: &Rfc4180Acc) {
        if other.rows == 0 {
            return;
        }
        self.rows += other.rows;
        self.first_line = match (self.first_line, other.first_line) {
            (Some(a), Some(b)) => Some(a.min(b)),
            (a, b) => a.or(b),
        };
        self.kind = self.kind.or(other.kind);
        if self.example.is_none() {
            self.example = other.example.clone();
        }
    }
}

/// RFC 4180-compatible CSV tokenizer. Byte-level scanner; fast on large files.
/// `max_data_rows`: maximum number of rows excluding the header (None = unlimited).
/// When the limit is exceeded, remaining bytes are not processed and the returned vec is truncated.
///
/// The third return value accumulates RFC 4180 violations (`ARC_033`); the tokenizer does not
/// return an ERROR and continues parsing as before. Deliberately: making a malformed quote make
/// data unreadable and connecting it to `ARC_013` (FATAL for a required file) would reject
/// near-valid feeds — the `PTH_017` asymmetry.
fn tokenize_csv(
    text: &str,
    max_data_rows: Option<usize>,
) -> Result<(Vec<Vec<SmolStr>>, bool, Rfc4180Acc), String> {
    let bytes = text.as_bytes();
    let n = bytes.len();
    let cap = max_data_rows
        .map(|m| m + 1)
        .unwrap_or_else(|| (n / 50).clamp(1, 5_000_000));
    let mut records: Vec<Vec<SmolStr>> = Vec::with_capacity(cap.min(5_000_000));
    let mut pos = 0;
    let mut col_hint: usize = 8;
    let mut truncated = false;
    let mut rfc = Rfc4180Acc::default();

    while pos < n {
        // `records.len()` is the index of the record being built; header 0 → row 1, first data
        // row 1 → row 2. The caller's loop uses the same convention (`row_idx + 2`).
        let line_no = (records.len() + 1) as u64;
        let mut record: Vec<SmolStr> = Vec::with_capacity(col_hint);

        loop {
            if bytes[pos] == b'"' {
                // Quoted field — converting to SmolStr requires a temporary String.
                pos += 1;
                let mut buf = String::new();
                let mut closed = false;
                while pos < n {
                    let b = bytes[pos];
                    if b == b'"' {
                        pos += 1;
                        if pos < n && bytes[pos] == b'"' {
                            buf.push('"');
                            pos += 1;
                        } else {
                            closed = true;
                            break;
                        }
                    } else if b < 0x80 {
                        buf.push(b as char);
                        pos += 1;
                    } else {
                        let ch = text[pos..].chars().next().unwrap();
                        buf.push(ch);
                        pos += ch.len_utf8();
                    }
                }
                if !closed {
                    return Err("Kapanmamış tırnak işareti (unclosed quote)".to_string());
                }
                // RFC 4180: a delimiter must follow a closing quote. If it does not, a quote
                // inside the field was NOT ESCAPED (`"Broadway"Ave`). Until now the tokenizer
                // silently split the record here — and data was lost.
                if pos < n && !matches!(bytes[pos], b',' | b'\n' | b'\r') {
                    rfc.observe(line_no, RFC4180_AFTER_CLOSE, &buf);
                    // 🔴 The remaining text is appended to the SAME field; no NEW field is opened.
                    //
                    // The old behavior closed the field here and split a cell such as
                    // `"Thonon Les Arts" A compter du 01/09/2026` into TWO fields. The result
                    // was column drift: 7 rows in `tdg-80973/routes.txt` were written this way,
                    // and `route_id` shifted into the next column, producing a Spec·CRITICAL
                    // finding that "route_id is required" for `RTS_031` — exactly the 7 rows
                    // counted by ARC_033.
                    //
                    // The violation is still reported (by the `observe` above); what changes is
                    // that the violation does NOT CORRUPT THE DATA. Python's `csv` module uses
                    // the same tolerance.
                    while pos < n && !matches!(bytes[pos], b',' | b'\n' | b'\r') {
                        let b = bytes[pos];
                        if b < 0x80 {
                            buf.push(b as char);
                            pos += 1;
                        } else {
                            let ch = text[pos..].chars().next().unwrap();
                            buf.push(ch);
                            pos += ch.len_utf8();
                        }
                    }
                }
                record.push(SmolStr::new(&buf));
            } else {
                // Unquoted field — SmolStr directly from the slice (≤22 bytes → inline, no heap allocation).
                let start = pos;
                let mut bare_quote = false;
                while pos < n {
                    let b = bytes[pos];
                    if b == b',' || b == b'\n' || b == b'\r' {
                        break;
                    }
                    // RFC 4180 §2.5: a quote MUST NOT occur in an unquoted field. The old scanner
                    // treated it as an ordinary character, so the violation was NEVER reported.
                    bare_quote |= b == b'"';
                    pos += 1;
                }
                if bare_quote {
                    rfc.observe(line_no, RFC4180_BARE_QUOTE, &text[start..pos]);
                }
                record.push(SmolStr::new(&text[start..pos]));
            }

            if pos >= n {
                break;
            }
            match bytes[pos] {
                b',' => {
                    pos += 1;
                    if pos >= n {
                        // Trailing comma with no newline: push an empty field.
                        record.push(SmolStr::new(""));
                        break;
                    }
                    continue;
                }
                b'\r' => {
                    pos += 1;
                    if pos < n && bytes[pos] == b'\n' {
                        pos += 1;
                    }
                    break;
                }
                b'\n' => {
                    pos += 1;
                    break;
                }
                _ => break,
            }
        }

        if !(record.is_empty() || record.len() == 1 && record[0].is_empty()) {
            col_hint = record.len();
            records.push(record);
            if let Some(max) = max_data_rows {
                // records[0] is the header; the number of data rows is records.len()-1.
                if records.len().saturating_sub(1) > max {
                    records.pop();
                    truncated = true;
                    break;
                }
            }
        }
    }

    Ok((records, truncated, rfc))
}

// ── Schema helpers ────────────────────────────────────────────────────────────

/// Required column set per file. A missing header produces ARC_025; an empty value is handled by
/// the relevant K2 group rule.
///
/// ⚠️ **MANUALLY MAINTAINED and ARC_025 Critical·Spec** — adding a CONDITIONAL field here blocks
/// a valid feed from publication. That was exactly the case for
/// `transfers.from_stop_id`/`to_stop_id` on 2026-08-02 (spec: "Optional if transfer_type is 4 or 5").
/// The list is now locked against the spec by `required_columns_match_the_specification`.
///
/// `pub`: this file is the only production caller, but scope measurement (`emit_proof.rs`) uses
/// this list as the **declared anchor source** — ARC_025 emits the missing column as
/// `field=<sütun>`, and the mechanism is one loop, so membership proves that the field's
/// `presence:required` requirement is checked. (Triggering every column one by one with fixtures
/// would require ~100 fixtures; what was done for CAL_002 does not scale here.)
pub fn required_fields(filename: &str) -> &'static [&'static str] {
    match filename {
        "agency.txt" => &["agency_name", "agency_url", "agency_timezone"],
        "stops.txt" => &["stop_id"],
        "routes.txt" => &["route_id", "route_type"],
        "trips.txt" => &["route_id", "service_id", "trip_id"],
        // stop_id is CONDITIONALLY required (spec): "Required if stop_times.location_group_id AND
        // stop_times.location_id are NOT defined. Forbidden if ... are defined." The condition
        // is applied in `required_fields_for` based on the headers.
        "stop_times.txt" => &["trip_id", "stop_sequence"],
        "calendar.txt" => &[
            "service_id",
            "monday",
            "tuesday",
            "wednesday",
            "thursday",
            "friday",
            "saturday",
            "sunday",
            "start_date",
            "end_date",
        ],
        "calendar_dates.txt" => &["service_id", "date", "exception_type"],
        "shapes.txt" => &[
            "shape_id",
            "shape_pt_lat",
            "shape_pt_lon",
            "shape_pt_sequence",
        ],
        "frequencies.txt" => &["trip_id", "start_time", "end_time", "headway_secs"],
        // ⚠️ `from_stop_id`/`to_stop_id` MUST NOT BE HERE — the spec makes both conditional:
        // "Required if transfer_type is empty, 0, 1, 2, or 3. Optional if transfer_type is 4 or 5."
        // 4/5 are onboard transfers and use `from_trip_id`/`to_trip_id`. A VALID feed with only
        // type-4/5 transfers received ARC_025 (Critical·Spec) and failed R1 because it omitted
        // these columns — a false positive that blocked a valid feed from publication.
        // The only truly unconditional Required field is `transfer_type`, which was missing.
        // (2026-08-02, the ARC_025 list was compared with the spec.)
        "transfers.txt" => &["transfer_type"],
        // Empty transfers = unlimited transfers (a valid GTFS value) — the empty-value rule must
        // not fire.
        "fare_attributes.txt" => &[
            "fare_id",
            "price",
            "currency_type",
            "payment_method",
            "transfers",
        ],
        "fare_rules.txt" => &["fare_id"],
        "pathways.txt" => &[
            "pathway_id",
            "from_stop_id",
            "to_stop_id",
            "pathway_mode",
            "is_bidirectional",
        ],
        "levels.txt" => &["level_id", "level_index"],
        "feed_info.txt" => &["feed_publisher_name", "feed_publisher_url", "feed_lang"],
        "translations.txt" => &["table_name", "field_name", "language", "translation"],
        "rider_categories.txt" => &[
            "rider_category_id",
            "rider_category_name",
            "is_default_fare_category",
        ],
        "fare_media.txt" => &["fare_media_id", "fare_media_type"],
        "fare_products.txt" => &["fare_product_id", "amount", "currency"],
        "fare_leg_rules.txt" => &["fare_product_id"],
        // Spec: all four fields are part of the composite primary key; network fields are
        // Required, while stop fields are mutually conditional (measured by FLJ_003/004) → only
        // the two unconditional Required fields belong in required_fields.
        "fare_leg_join_rules.txt" => &["from_network_id", "to_network_id"],
        "fare_transfer_rules.txt" => &["fare_transfer_type"],
        "areas.txt" => &["area_id"],
        "stop_areas.txt" => &["area_id", "stop_id"],
        "networks.txt" => &["network_id"],
        "timeframes.txt" => &["timeframe_group_id", "service_id"],
        "booking_rules.txt" => &["booking_rule_id", "booking_type"],
        "location_groups.txt" => &["location_group_id"],
        "location_group_stops.txt" => &["location_group_id", "stop_id"],
        // organization_name is Required in the spec; it was missing from the list (added on 2026-08-02).
        "attributions.txt" => &["organization_name"],
        // Both fields in route_networks.txt are Required in the spec (the file was introduced on 2026-08-02).
        "route_networks.txt" => &["network_id", "route_id"],
        _ => &[],
    }
}

/// ARC_009: is an empty (header-only) file CRITICAL? If required files (agency/stops/routes/trips/
/// stop_times) plus service-defining calendar files (calendar/calendar_dates) are empty, data is
/// absent where it is expected → CRITICAL. Empty optional files (transfers, fare_*, shapes,
/// frequencies, translations, Fares v2/Flex/JP …) are harmless → downgraded to INFO.
fn arc009_critical(filename: &str) -> bool {
    REQUIRED_FILES.contains(&filename) || CALENDAR_FILES.contains(&filename)
}

/// Would being unable to READ this file invalidate the publication decision (issue #133)?
///
/// Required files plus service-defining calendar files. If one cannot be read, none of its
/// dependent rules run, and "no findings" does not mean "clean".
/// Optional files (`attributions`, `fare_*`, `shapes`, …) are deliberately excluded: losing one
/// makes the run PARTIAL but must not fail the gate.
pub fn is_certification_critical(filename: &str) -> bool {
    arc009_critical(filename)
}

/// DQ_016: leading/trailing whitespace is a **raw lexical violation**. This applies not only to
/// string/id fields but also to numeric, enum, date, and time fields: the parser tolerating
/// whitespace does not change the producer's input. Typed checks continue to see the raw value;
/// K7 suppresses only downstream findings caused by this root cause.
/// DQ_016 file-level accumulator: count of rows with whitespace + affected column set + raw sample.
///
/// **Why aggregate instead of emitting per row:** leading/trailing whitespace is one PRODUCER
/// HABIT (most often writing `, ` instead of `,`) → it appears on EVERY row in the file.
/// Per-row emission produced **2,361,873** notices in `mdb-992` (98.6% of the feed total;
/// MobilityData produced 12 for the same feed) — the same case where STM_050 exhausted the
/// browser. Detection is correct (under RFC 4180 the whitespace is part of the field); the
/// problem is presentation.
///
/// All three emit sites (general K1, K2 stop_times, K2 shapes) use this.
#[derive(Default)]
pub(crate) struct Dq016Acc {
    pub rows: u64,
    pub cols: std::collections::BTreeSet<String>,
    pub samples: std::collections::BTreeMap<String, String>,
    /// Planner context per sampled column: the sample's line and the record it belongs to (the
    /// value of the file's key column on that row, raw), so the card can name and map it.
    sample_records: std::collections::BTreeMap<String, (u64, String)>,
    /// The key column (name, index), chosen once from the headers; `Some(None)` when none.
    key: Option<Option<(String, usize)>>,
    pub first_line: Option<u64>,
    last_line: u64,
}

/// DQ_016 record column: the column naming the row's record, in this order (stop_times and
/// frequencies by trip, trips by trip, stops by stop, routes by route…); otherwise the first
/// `*_id` column.
const DQ016_KEY_COLUMNS: [&str; 9] = [
    "trip_id",
    "stop_id",
    "route_id",
    "shape_id",
    "service_id",
    "agency_id",
    "fare_id",
    "pathway_id",
    "level_id",
];

impl Dq016Acc {
    /// Processes one row. `values`/`headers` must have the same order.
    pub fn observe<'a>(
        &mut self,
        line: u64,
        values: impl Iterator<Item = &'a str> + Clone,
        headers: &[String],
    ) {
        let record_values = values.clone();
        for (i, s) in values.enumerate() {
            if s == s.trim() {
                continue;
            }
            let Some(name) = headers.get(i).map(|h| h.as_str()) else {
                continue;
            };
            if self.last_line != line || self.rows == 0 {
                self.last_line = line;
                self.rows += 1;
                if self.first_line.is_none() {
                    self.first_line = Some(line);
                }
            }
            self.cols.insert(name.to_string());
            if !self.samples.contains_key(name) {
                self.samples.insert(name.to_string(), s.to_string());
                // Once per column: the record this first sample sits on.
                let key = self.key.get_or_insert_with(|| {
                    DQ016_KEY_COLUMNS
                        .iter()
                        .find_map(|k| headers.iter().position(|h| h == k))
                        .or_else(|| headers.iter().position(|h| h.ends_with("_id")))
                        .map(|idx| (headers[idx].clone(), idx))
                });
                if let Some((_, idx)) = key {
                    let record = record_values.clone().nth(*idx).unwrap_or("").to_string();
                    self.sample_records.insert(name.to_string(), (line, record));
                }
            }
        }
    }

    /// Summary text: (observed, message, column list). `None` when empty.
    /// Merges counters accumulated by another chunk (stop_times chunk merging).
    /// `other` is the chunk that comes LATER in the file; keep the smallest `first_line`,
    /// largest `last_line`, sum `rows`, and merge `cols`.
    #[cfg(feature = "parallel")]
    pub(crate) fn merge(&mut self, other: Dq016Acc) {
        self.rows += other.rows;
        self.cols.extend(other.cols);
        for (field, sample) in other.samples {
            self.samples.entry(field).or_insert(sample);
        }
        for (field, record) in other.sample_records {
            self.sample_records.entry(field).or_insert(record);
        }
        if self.key.is_none() {
            self.key = other.key;
        }
        self.first_line = match (self.first_line, other.first_line) {
            (Some(a), Some(b)) => Some(a.min(b)),
            (a, b) => a.or(b),
        };
        self.last_line = self.last_line.max(other.last_line);
    }

    pub fn summary(&self, file_name: &str) -> Option<(String, String, String)> {
        if self.rows == 0 {
            return None;
        }
        let cols = self
            .cols
            .iter()
            .map(String::as_str)
            .collect::<Vec<_>>()
            .join(", ");
        Some((
            format!("{} rows", self.rows),
            format!(
                "'{file_name}' dosyasında {} satırda baştaki/sondaki boşluk var; etkilenen sütunlar: {cols}.",
                self.rows
            ),
            cols,
        ))
    }

    /// Adds raw-value evidence to the DQ_016 root finding. Whitespace remains visible in JSON;
    /// no value is trimmed or reused as an identity.
    ///
    /// `typed_whitespace_fields` (#126): fields that can be read as NUMBERS after whitespace is
    /// removed. These are cells that would enter a type/range rule without the whitespace — the
    /// actual location of the "hidden because of whitespace" phenomenon.
    ///
    /// ⚠️ WHY A SEPARATE KEY WAS NEEDED: K7 writes `suppressed_derivative_*` on the root finding,
    /// but that counter exists only when a derivative NOTICE is created. `stop_times`, `shapes`,
    /// `trips`, and `calendar_dates` read columns with `get_col` and apply `.trim()`, so no
    /// derivative is created in those four files and the key is never written. The missing key
    /// read as "nothing is hidden here", although it actually described the reader's regime.
    /// This field is produced in both regimes so the numbers are comparable: both are populated
    /// in a file that creates derivatives, while only this one is populated in a trimming file.
    pub(crate) fn evidence_details(&self) -> Option<std::collections::BTreeMap<String, String>> {
        if self.samples.is_empty() {
            return None;
        }
        let samples = self
            .samples
            .iter()
            .map(|(field, value)| format!("{field}={value:?}"))
            .collect::<Vec<_>>()
            .join("; ");
        let mut details: std::collections::BTreeMap<String, String> = [
            ("raw_samples".to_string(), samples),
            ("affected_rows".to_string(), self.rows.to_string()),
        ]
        .into_iter()
        .collect();
        // Planner context: the same samples one per column (sorted), value kept raw so the card
        // can show where the whitespace sits.
        // Each sample also carries its line and the record it sits on (the key column's value).
        for (i, (field, value)) in self.samples.iter().enumerate() {
            details.insert(format!("sample_{i}_field"), field.clone());
            details.insert(format!("sample_{i}_value"), value.clone());
            if let Some((line, record)) = self.sample_records.get(field) {
                details.insert(format!("sample_{i}_line"), line.to_string());
                if !record.is_empty() {
                    details.insert(format!("sample_{i}_record"), record.clone());
                }
            }
        }
        if let Some(Some((column, _))) = &self.key {
            details.insert("record_field".to_string(), column.clone());
        }

        // If the raw form cannot be read but the trimmed form can, whitespace alone hides a typed
        // value. Text fields (" Durak Adı ") are not numbers in either form, so they are omitted.
        let typed: Vec<&str> = self
            .samples
            .iter()
            .filter(|(_, v)| v.parse::<f64>().is_err() && v.trim().parse::<f64>().is_ok())
            .map(|(field, _)| field.as_str())
            .collect();
        if !typed.is_empty() {
            details.insert("typed_whitespace_fields".to_string(), typed.join(", "));
        }

        // 🔴 WHITESPACE IN AN IDENTITY FIELD IS NOT ORDINARY WHITESPACE.
        //
        // `" Durak Adı "` in a description field is merely ugly. `"10T0001C1  "` in a KEY
        // field silently makes every cross-file lookup miss: `routes.txt` writes the key with
        // whitespace, `trips.txt` without it, and the raw bytes do not match. This follows
        // directly from the decision to compare identities RAW (#85), and that decision is
        // PRESERVED — the producer must fix this.
        //
        // But `suppress_whitespace_derivatives` can declare only rules that PRODUCED A FINDING.
        // An analysis that produces no finding because a lookup missed would disappear without
        // a trace — the exact opposite of k7_reporting's #134 invariant ("every suppressed
        // finding must be declared somewhere"). `mdb-2653` showed this: `trips.txt` declared
        // 304,117 suppressed findings, while the 1,502 block inconsistencies missed by
        // `TRP_024` appeared nowhere because that rule had never spoken (#169).
        let identity: Vec<&str> = self
            .cols
            .iter()
            .map(String::as_str)
            .filter(|c| is_identity_column(c))
            .collect();
        if !identity.is_empty() {
            details.insert(
                "identity_whitespace_fields".to_string(),
                identity.join(", "),
            );
            details.insert(
                "cross_file_analysis".to_string(),
                "unreliable — lookups keyed on these values cannot match across files".to_string(),
            );
        }
        Some(details)
    }
}

/// Columns used as KEYS for cross-file lookups in GTFS.
///
/// The list is intentionally open-ended: not everything ending in `*_id` is a key (such as
/// `shape_dist_traveled`), but every GTFS key column either ends in `_id` or is `parent_station`.
/// The cost of a false positive is low here — this is only a DECLARATION, not a finding.
pub(crate) fn is_identity_column(col: &str) -> bool {
    const EXTRA: &[&str] = &["parent_station"];
    let c = col.trim();
    c.ends_with("_id") || EXTRA.contains(&c)
}

pub(crate) const DQ016_REMEDIATION: &str =
    "Trim leading and trailing whitespace from all field values.";

/// In zip 9, `name()` returns a `Result`: a non-UTF-8 name is decoded as cp437 and errors only
/// on an inaccessible branch. It is still not swallowed silently; it is reported as "ZIP could
/// not be read" in the same way as the neighboring `by_index` error.
fn entry_name_error(index: usize, e: &zip::result::ZipError) -> FatalError {
    FatalError::new(
        FatalCode::ZipUnreadable,
        format!("ZIP girdisinin adı çözülemedi (index {index}): {e}"),
    )
    .variant("entry_index")
    .param("index", index)
    .param("detail", e)
}

// ── ARC_012 / ARC_021: shared K1 ↔ K2-stream logic ───────────────────────────
//
// In stream_mode these checks moved from K1 to K2, but were COPIED separately for two targets
// (stop_times, shapes) — the exact same predicate and user message in three places. Changing
// the threshold/range list meant editing three files; forgetting one would silently diverge.
// Same rationale as DQ016_ACC: one definition, three callers.

/// ARC_012: does the row have a different number of columns from the header?
///
/// `None` → no problem. `Some((message, recommendation, observed, bilgi_mi))`; when `bilgi_mi=true`,
/// the row is SHORT (trailing optional fields omitted — valid CSV practice, severity is lowered
/// to Info); `false` means there are EXTRA fields (an unescaped comma → nominal severity).
pub(crate) fn arc012_check(
    file_name: &str,
    line: u64,
    row_len: usize,
    header_count: usize,
) -> Option<(String, &'static str, String, bool)> {
    if row_len == header_count {
        return None;
    }
    let short = row_len < header_count;
    let (msg, tip) = if short {
        let missing = header_count - row_len;
        (
            format!("'{file_name}' {line}. satırda sondaki {missing} isteğe bağlı alan atlanmış ({row_len} sütun, başlık: {header_count})."),
            "CSV'de sondaki boş alanlar atlanabilir; zorunlu alanlar boş bırakılmamalıdır.",
        )
    } else {
        (
            format!("'{file_name}' {line}. satırda fazla alan: {row_len} (beklenen {header_count}) — kaçmamış virgül veya format hatası."),
            "Her satırın başlık sayısı kadar virgülle ayrılmış değer içerdiğinden emin olun.",
        )
    };
    Some((
        msg,
        tip,
        format!("{row_len} columns (expected {header_count})"),
        short,
    ))
}

/// ARC_021: searches the row for an unprintable/problematic character and returns the first
/// code point found.
///
/// Valid Unicode letters/digits/whitespace (Japanese, Turkish, etc.) are NOT a problem — only
/// control characters, DEL, surrogate code points, and private-use characters are flagged.
/// ARC_034: is the row an exact repeat of the header row?
///
/// 🔴 BOTH K1 and K2 must perform this check. `stop_times.txt`, `shapes.txt`, `trips.txt`, and
/// `calendar_dates.txt` read ONLY the header in K1 via `stream_mode`; data rows go to K2. If the
/// check stays only in K1, a repeated header row in those four files is processed as DATA — row
/// 16873 of `mdb-1004/trips.txt` is exactly this case, and it reported the literal `"route_id"`
/// as an FK violation for `TRP_002` (#181).
///
/// ⚠️ False-positive gate: EVERY field in the row must equal its own column name.
/// It is legitimate for a single column's `route_id` value to be "route_id".
pub(crate) fn arc034_is_header_repeat<S: AsRef<str>>(row: &[S], headers: &[String]) -> bool {
    row.len() == headers.len()
        && !headers.is_empty()
        && row
            .iter()
            .zip(headers.iter())
            .all(|(v, h)| v.as_ref().trim() == h.trim())
}

pub(crate) fn arc021_bad_char<'a>(values: impl Iterator<Item = &'a str>) -> Option<u32> {
    for val in values {
        for ch in val.chars() {
            if ch.is_alphanumeric() || ch.is_whitespace() {
                continue;
            }
            let cp = ch as u32;
            let is_bad = (cp < 32 && cp != 9)
                || cp == 127
                || (0xD800..=0xDFFF).contains(&cp)
                || (0xE000..=0xF8FF).contains(&cp)
                || (0xFFF0..=0xFFFF).contains(&cp);
            if is_bad {
                return Some(cp);
            }
        }
    }
    None
}

/// ARC_030: searches a field value for a tab, line feed, or carriage return and returns the first
/// one found.
///
/// GTFS spec, Dataset Files: "Field values must not contain tabs, carriage returns or new
/// lines." ARC_021 deliberately misses these three (`is_whitespace()` exemption, plus `cp != 9`):
/// that rule targets malformed/unprintable characters and exempts valid text. These three
/// characters can occur in otherwise valid text, but the spec explicitly prohibits them. The
/// two rules measure different phenomena.
///
/// Byte scanning is UTF-8 safe: TAB/LF/CR are ASCII, and continuation bytes of multibyte
/// characters are 0x80 or above. Using `bytes()` instead of `chars()` is deliberate — this is
/// a K1 hot path.
pub(crate) fn arc030_bad_whitespace<'a>(values: impl Iterator<Item = &'a str>) -> Option<u32> {
    for val in values {
        for b in val.bytes() {
            if b == b'\t' || b == b'\n' || b == b'\r' {
                return Some(b as u32);
            }
        }
    }
    None
}

pub(crate) fn arc030_message(file_name: &str, cp: u32) -> String {
    let kind = match cp {
        9 => "sekme (TAB)",
        10 => "satır sonu (LF)",
        _ => "satır başı (CR)",
    };
    format!("'{file_name}' dosyasında bir alan değeri {kind} karakteri içeriyor (U+{cp:04X}).")
}

pub(crate) const ARC030_REMEDIATION: &str =
    "Remove tabs and line breaks from field values; collapse multi-line text onto a single line.";

// ── ARC_032: HTML tag / comment / escape sequence ─────────────────────────────
//
// GTFS spec, File Requirements: "Field values must not contain HTML tags, comments or
// escape sequences." ARC_021 and ARC_030 miss this — `<` (U+003C) is in none of ARC_021's
// malformed-character ranges, is not alphanumeric, and still does not satisfy `is_bad`.
//
// ⚠️ CLOSED LIST, NOT A PATTERN. During measurement, a loose pattern (`<\s*/?[a-zA-Z]+...>` +
// any `&kelime;`) produced 1,991 matches in 239 feeds, and examples showed that most were
// LEGITIMATE text: `K.A.Wheel&Tire;` is not an entity, and `St Sauvant >< St Césaire` is not a
// tag. The closed list left 1,955 genuine matches (4 feeds). Expand the pattern only after
// reviewing examples.
/// HTML element names — **GENERATED** (issue #79, `k1_html_elements.rs`).
///
/// 🔴 This list used to be handwritten and the card claimed "ALL OF HTML5". Audit 6 disproved
/// that claim: `svg` and `math` were missing. Keeping the tag list manually while generating the
/// entity table from the source was inconsistent; both are now generated.
///
/// Set = WHATWG element index ∪ foreign-content roots (`svg`, `math`) ∪ obsolete elements
/// (`font`, `center`, …). The rationale for the two additional sets is in the generator's
/// docstring: `svg`/`math` are NOT in the HTML element index (they are SVG/MathML elements) but
/// are RECOGNISED starting tags in HTML; obsolete elements were removed from the index but occur
/// in real feeds (`<BR>` and `<font>` were measured in the corpus).
///
/// ⚠️ Keep the list CLOSED — treating every word inside angle brackets as a tag would flag real
/// text such as `<TBD>`, `<Bilinmiyor>`, and `<未定>`.
use crate::k1_html_elements::HTML_ELEMENTS as HTML_TAGS;
/// Named character references — **GENERATED** (issue #79, `k1_html_entities.rs`).
///
/// 🔴 This used to contain six names (`nbsp amp lt gt quot apos`), while `&copy;`, `&reg;`,
/// `&euro;`, and `&mdash;` escaped the rule. The list is now all of HTML5 (2,125 names); its
/// source and SHA-256 are in the generated file, and drift is checked in CI.
///
/// ⚠️ Keep the list CLOSED — allowing the `&` + word + `;` pattern would catch real text
/// (`K.A.Wheel&Tire;`, measured in the corpus). The distinction is that the list is the
/// standard itself, not a SUBSET of the standard.
use crate::k1_html_entities::{HTML5_ENTITIES as HTML_ENTITIES, HTML5_ENTITIES_NO_SEMI};

/// Searches a value for an HTML tag, comment start, or character escape.
///
/// Returns the text of the marker found (such as `<BR>` or `&nbsp;`) for display to the user.
fn arc032_markup(value: &str) -> Option<String> {
    let bytes = value.as_bytes();
    for (i, &b) in bytes.iter().enumerate() {
        if b == b'<' {
            if value[i..].starts_with("<!--") {
                return Some("<!--".to_string());
            }
            // `</` closing tags count too. Preserve the slash in the OUTPUT: the example shown
            // to the user must match the text in the file exactly.
            let after = &value[i + 1..];
            let (slash, rest) = match after.strip_prefix('/') {
                Some(r) => ("/", r),
                None => ("", after),
            };
            let name_len = rest
                .find(|c: char| !c.is_ascii_alphanumeric())
                .unwrap_or(rest.len());
            if name_len == 0 {
                continue;
            }
            let name = &rest[..name_len];
            // 🔴 CHECK THE NAME BOUNDARY (found by the issue #79 test): name extraction stopped
            // at the first non-alphanumeric character, so `<svg-yok>` looked like "svg". Hyphenated
            // names are CUSTOM ELEMENTS in HTML (`<my-widget>`), not `svg`. A real tag name ends
            // only at whitespace, `/`, or `>`.
            match rest[name_len..].chars().next() {
                Some(c) if c.is_whitespace() || c == '/' || c == '>' => {}
                _ => continue,
            }
            // ⚠️ Tag names are CASE-INSENSITIVE (`<BR>` = `<br>`), unlike entity names. The table
            // is lowercase and sorted; lowercase before binary search (a linear scan of the
            // 130-name list would run for every `<`).
            let lower = name.to_ascii_lowercase();
            if HTML_TAGS.binary_search(&lower.as_str()).is_err() {
                continue;
            }
            // A tag is a tag only when it closes with `>`: not "a < b" or "3 <br".
            let Some(close) = rest[name_len..].find('>') else {
                continue;
            };
            // Another `<` between `<` and `>` means this is plain text, not a tag.
            let inner = &rest[name_len..name_len + close];
            if inner.contains('<') {
                continue;
            }
            return Some(format!("<{slash}{}{}>", &rest[..name_len], inner));
        }
        if b == b'&' {
            let rest = &value[i + 1..];
            // Name = the alphanumeric sequence after `&` (`#` numeric forms are handled separately).
            let name_len = rest
                .find(|c: char| !c.is_ascii_alphanumeric())
                .unwrap_or(rest.len());
            let name = &rest[..name_len];
            // ⚠️ LEGACY FORM WITHOUT A SEMICOLON (issue #79): HTML5 recognises some names without
            // `;` (`&copy`). The exception does NOT apply to ALL names; the separate table comes
            // from the source. Measurement: 20 feeds · 54,076,644 cells · 0 findings in this
            // form → the false-positive risk is measurably zero.
            if !name.is_empty()
                && !rest[name_len..].starts_with(';')
                && HTML5_ENTITIES_NO_SEMI.binary_search(&name).is_ok()
            {
                return Some(format!("&{name}"));
            }
            let Some(semi) = rest.find(';') else { continue };
            if semi == 0 || semi > 8 {
                continue;
            }
            let ent = &rest[..semi];
            // ⚠️ Entity names are CASE-SENSITIVE and the list carries both forms separately
            // (`&copy;` and `&COPY;` are different entries). `eq_ignore_ascii_case` would mean
            // both false matches and an O(n) scan of the 2,125-name list.
            let known = HTML_ENTITIES.binary_search(&ent).is_ok()
                || (ent.starts_with('#')
                    && ent.len() >= 2
                    && (ent[1..].chars().all(|c| c.is_ascii_digit())
                        || (ent[1..].starts_with(['x', 'X'])
                            && ent.len() >= 3
                            && ent[2..].chars().all(|c| c.is_ascii_hexdigit()))));
            if known {
                return Some(format!("&{ent};"));
            }
        }
    }
    None
}

/// ARC_032 accumulator — ONE notice per FILE (same rationale as `Dq016Acc`: one feed had
/// 1,176 stop names marked, and per-row emission overwhelmed the browser).
#[derive(Default)]
pub(crate) struct Arc032Acc {
    pub rows: u64,
    pub cols: std::collections::BTreeSet<String>,
    pub first_line: Option<u64>,
    pub example: Option<String>,
    last_line: u64,
}

impl Arc032Acc {
    pub fn observe<'a>(
        &mut self,
        line: u64,
        values: impl Iterator<Item = &'a str>,
        headers: &[String],
    ) {
        for (i, s) in values.enumerate() {
            // Fast path: a marker starts only with `<` or `&`. In streaming files (stop_times),
            // almost all values return here; skip the header-name comparison.
            if !s.bytes().any(|b| b == b'<' || b == b'&') {
                continue;
            }
            let Some(name) = headers.get(i).map(|h| h.as_str()) else {
                continue;
            };
            // `tts_stop_name` is STP_023's field: that rule already reports ANY `<`/`>` marker
            // (including SSML) and is broader. Emitting both would report the same phenomenon twice.
            if name == "tts_stop_name" {
                continue;
            }
            let Some(found) = arc032_markup(s) else {
                continue;
            };
            if self.last_line != line || self.rows == 0 {
                self.last_line = line;
                self.rows += 1;
                if self.first_line.is_none() {
                    self.first_line = Some(line);
                }
            }
            self.cols.insert(name.to_string());
            if self.example.is_none() {
                self.example = Some(found);
            }
        }
    }

    /// stop_times chunk merge: `self` is the earlier chunk, so preserve its example.
    #[cfg(feature = "parallel")]
    pub(crate) fn merge(&mut self, other: Arc032Acc) {
        self.rows += other.rows;
        self.cols.extend(other.cols);
        if self.example.is_none() {
            self.example = other.example;
        }
        self.first_line = match (self.first_line, other.first_line) {
            (Some(a), Some(b)) => Some(a.min(b)),
            (a, b) => a.or(b),
        };
        self.last_line = self.last_line.max(other.last_line);
    }

    /// Summary: (observed, message, column list). `None` when empty.
    pub fn summary(&self, file_name: &str) -> Option<(String, String, String)> {
        if self.rows == 0 {
            return None;
        }
        let cols = self
            .cols
            .iter()
            .map(String::as_str)
            .collect::<Vec<_>>()
            .join(", ");
        let example = self.example.as_deref().unwrap_or("");
        Some((
            example.to_string(),
            format!(
                "'{file_name}' dosyasında {} satırda HTML işareti var (ör. '{example}'); \
                 etkilenen sütunlar: {cols}.",
                self.rows
            ),
            cols,
        ))
    }
}

/// Columns whose type in the spec is `ID` / `Foreign ID` (GTFS Reference "Field Types").
/// `direction_id` is an Enum and is NOT in the list; `jp_*` extension columns are outside the spec.
pub(crate) const GTFS_ID_FIELDS: &[&str] = &[
    "agency_id",
    "stop_id",
    "parent_station",
    "zone_id",
    "level_id",
    "route_id",
    "network_id",
    "trip_id",
    "service_id",
    "block_id",
    "shape_id",
    "fare_id",
    "origin_id",
    "destination_id",
    "contains_id",
    "from_stop_id",
    "to_stop_id",
    "from_route_id",
    "to_route_id",
    "from_trip_id",
    "to_trip_id",
    "pathway_id",
    "area_id",
    "from_area_id",
    "to_area_id",
    "fare_media_id",
    "fare_product_id",
    "leg_group_id",
    "from_leg_group_id",
    "to_leg_group_id",
    "rider_category_id",
    "timeframe_group_id",
    "from_timeframe_group_id",
    "to_timeframe_group_id",
    "location_group_id",
    "location_id",
    "booking_rule_id",
    "pickup_booking_rule_id",
    "drop_off_booking_rule_id",
    "prior_notice_service_id",
    "attribution_id",
    "record_id",
    "record_sub_id",
];

/// ARC_037 accumulator — a character outside printable ASCII (0x20–0x7E) in an ID field.
/// Spec: *"Using only printable ASCII characters is recommended."* Because this is a
/// recommendation, produce ONE summary per FILE (`Arc032Acc` pattern); MD counts per row
/// (58K in one feed). Unprintable characters belong to ARC_021; this rule counts only values
/// above 0x7E.
#[derive(Default)]
pub(crate) struct Arc037Acc {
    pub rows: u64,
    pub cols: std::collections::BTreeSet<String>,
    pub first_line: Option<u64>,
    pub example: Option<(String, String)>,
    last_line: u64,
}

impl Arc037Acc {
    pub fn observe<'a>(
        &mut self,
        line: u64,
        values: impl Iterator<Item = &'a str>,
        headers: &[String],
    ) {
        for (i, s) in values.enumerate() {
            if s.is_ascii() {
                continue;
            }
            let Some(name) = headers.get(i).map(|h| h.as_str()) else {
                continue;
            };
            if !GTFS_ID_FIELDS.contains(&name) {
                continue;
            }
            if self.last_line != line || self.rows == 0 {
                self.last_line = line;
                self.rows += 1;
                if self.first_line.is_none() {
                    self.first_line = Some(line);
                }
            }
            self.cols.insert(name.to_string());
            if self.example.is_none() {
                self.example = Some((name.to_string(), s.to_string()));
            }
        }
    }

    /// stop_times chunk merge: `self` is the earlier chunk, so preserve its example.
    #[cfg(feature = "parallel")]
    pub(crate) fn merge(&mut self, other: Arc037Acc) {
        self.rows += other.rows;
        self.cols.extend(other.cols);
        if self.example.is_none() {
            self.example = other.example;
        }
        self.first_line = match (self.first_line, other.first_line) {
            (Some(a), Some(b)) => Some(a.min(b)),
            (a, b) => a.or(b),
        };
        self.last_line = self.last_line.max(other.last_line);
    }

    /// Summary: (observed, message, column list). `None` when empty.
    pub fn summary(&self, file_name: &str) -> Option<(String, String, String)> {
        if self.rows == 0 {
            return None;
        }
        let cols = self
            .cols
            .iter()
            .map(String::as_str)
            .collect::<Vec<_>>()
            .join(", ");
        let (col, value) = self.example.clone().unwrap_or_default();
        Some((
            value.clone(),
            format!(
                "'{file_name}' dosyasında {} satırda ID alanı ASCII dışı karakter içeriyor \
                 (ör. {col}='{value}'); etkilenen sütunlar: {cols}.",
                self.rows
            ),
            cols,
        ))
    }
}

pub(crate) const ARC037_REMEDIATION: &str =
    "Use only printable ASCII characters in ID values; IDs are not shown to riders, the name and description fields carry the display text.";

pub(crate) const ARC032_REMEDIATION: &str =
    "Remove HTML tags and character escapes from field values; GTFS fields carry plain text and consumers display this markup literally.";

pub(crate) fn arc021_message(file_name: &str, cp: u32) -> String {
    // Do not say "non-ASCII": the rule deliberately exempts valid Unicode letters (ü, ö, 漢字…);
    // only control/DEL/surrogate/private-use characters are flagged. The message must not claim
    // more than the rule does (VBB German-text discussion, 2026-07-24).
    format!("'{file_name}' dosyasında yazdırılamaz veya sorunlu karakter içeren değer var (U+{cp:04X}).")
}

pub(crate) const ARC021_REMEDIATION: &str =
    "Remove control/non-printable characters from field values; valid Unicode letters (e.g. ü, ö, 漢字) are fine.";

/// Unknown columns listed individually per file before ARC_017 switches to one summary notice.
pub const ARC_017_MAX_PER_FILE: usize = 100;

/// Column names defined in the GTFS specification (ARC_017).
/// `translations.txt::table_name` carries a table name without its extension; the column list
/// is keyed by file name. This arm carries the legacy and profile-specific column lists for
/// `translations.txt`; for all official GTFS tables, the translation-field catalogue is also
/// resolved from the generated `spec_fields.json` source.
///
/// 🔑 Constructing a name (`format!("{table}.txt")`) allocates once per row;
/// `mdb-2126`'s `translations.txt` contains 158,531 rows.
pub(crate) fn known_columns_for_table(table: &str) -> &'static [&'static str] {
    match table {
        "agency" => known_columns("agency.txt"),
        "stops" => known_columns("stops.txt"),
        "routes" => known_columns("routes.txt"),
        "trips" => known_columns("trips.txt"),
        "stop_times" => known_columns("stop_times.txt"),
        "feed_info" => known_columns("feed_info.txt"),
        "attributions" => known_columns("attributions.txt"),
        "pathways" => known_columns("pathways.txt"),
        "levels" => known_columns("levels.txt"),
        _ => &[],
    }
}

/// Returns the complete known-column set for a canonical GTFS/GTFS-JP file.
/// K4 uses this to distinguish official `jp_` extensions from a custom field
/// that collides with the GTFS-JP reserved namespace.
pub(crate) fn known_columns_for_file(filename: &str) -> &'static [&'static str] {
    known_columns(filename)
}

fn known_columns(filename: &str) -> &'static [&'static str] {
    match filename {
        "agency.txt" => &[
            "agency_id",
            "agency_name",
            "agency_url",
            "agency_timezone",
            "agency_lang",
            "agency_phone",
            "agency_fare_url",
            "agency_email",
            "cemv_support",
        ],
        "stops.txt" => &[
            "stop_id",
            "stop_code",
            "stop_name",
            "stop_desc",
            "stop_lat",
            "stop_lon",
            "zone_id",
            "stop_url",
            "location_type",
            "parent_station",
            "stop_timezone",
            "wheelchair_boarding",
            "level_id",
            "platform_code",
            "stop_access",
            "tts_stop_name",
        ],
        "routes.txt" => &[
            "route_id",
            "agency_id",
            "route_short_name",
            "route_long_name",
            "route_desc",
            "route_type",
            "route_url",
            "route_color",
            "route_text_color",
            "route_sort_order",
            "continuous_pickup",
            "continuous_drop_off",
            "network_id",
            "cemv_support",
            // GTFS-JP
            "jp_parent_route_id",
            "jp_office_id",
        ],
        "trips.txt" => &[
            "route_id",
            "service_id",
            "trip_id",
            "trip_headsign",
            "trip_short_name",
            "direction_id",
            "block_id",
            "shape_id",
            "wheelchair_accessible",
            "bikes_allowed",
            "cars_allowed",
            "safe_duration_factor",
            "safe_duration_offset",
            // GTFS-JP
            "jp_trip_desc",
            "jp_trip_desc_symbol",
            "jp_pattern_id",
            // Tokyo Toei and other GTFS-JP feeds may carry the official
            // jp_office_id extension on trips.txt as well as routes.txt.
            "jp_office_id",
        ],
        "stop_times.txt" => &[
            "trip_id",
            "arrival_time",
            "departure_time",
            "stop_id",
            "stop_sequence",
            "stop_headsign",
            "pickup_type",
            "drop_off_type",
            "continuous_pickup",
            "continuous_drop_off",
            "shape_dist_traveled",
            "timepoint",
            // GTFS-Flex (official Schedule Reference): demand-responsive service fields.
            // The K2 parser ALREADY read these; omitting them here produced an ARC_017 "unknown
            // column" false positive in a valid Flex feed.
            "location_id",
            "location_group_id",
            "start_pickup_drop_off_window",
            "end_pickup_drop_off_window",
            "pickup_booking_rule_id",
            "drop_off_booking_rule_id",
        ],
        "calendar.txt" => &[
            "service_id",
            "monday",
            "tuesday",
            "wednesday",
            "thursday",
            "friday",
            "saturday",
            "sunday",
            "start_date",
            "end_date",
        ],
        "calendar_dates.txt" => &["service_id", "date", "exception_type"],
        "shapes.txt" => &[
            "shape_id",
            "shape_pt_lat",
            "shape_pt_lon",
            "shape_pt_sequence",
            "shape_dist_traveled",
        ],
        "frequencies.txt" => &[
            "trip_id",
            "start_time",
            "end_time",
            "headway_secs",
            "exact_times",
        ],
        "transfers.txt" => &[
            "from_stop_id",
            "to_stop_id",
            "transfer_type",
            "min_transfer_time",
            "from_route_id",
            "to_route_id",
            "from_trip_id",
            "to_trip_id",
        ],
        "fare_attributes.txt" => &[
            "fare_id",
            "price",
            "currency_type",
            "payment_method",
            "transfers",
            "agency_id",
            "transfer_duration",
        ],
        "fare_rules.txt" => &[
            "fare_id",
            "route_id",
            "origin_id",
            "destination_id",
            "contains_id",
        ],
        "pathways.txt" => &[
            "pathway_id",
            "from_stop_id",
            "to_stop_id",
            "pathway_mode",
            "is_bidirectional",
            "length",
            "traversal_time",
            "stair_count",
            "max_slope",
            "min_width",
            "signposted_as",
            "reversed_signposted_as",
        ],
        "levels.txt" => &["level_id", "level_index", "level_name"],
        "feed_info.txt" => &[
            "feed_publisher_name",
            "feed_publisher_url",
            "feed_lang",
            "default_lang",
            "feed_start_date",
            "feed_end_date",
            "feed_version",
            "feed_contact_email",
            "feed_contact_url",
        ],
        "translations.txt" => &[
            "table_name",
            "field_name",
            "language",
            "translation",
            "record_id",
            "record_sub_id",
            "field_value",
        ],
        "attributions.txt" => &[
            "attribution_id",
            "agency_id",
            "route_id",
            "trip_id",
            "organization_name",
            "is_producer",
            "is_operator",
            "is_authority",
            "attribution_url",
            "attribution_email",
            "attribution_phone",
        ],
        // ── Fares v2 ── (column names verified against the GTFS spec + repository K2 parsers)
        "rider_categories.txt" => &[
            "rider_category_id",
            "rider_category_name",
            "is_default_fare_category",
            "eligibility_url",
        ],
        "fare_media.txt" => &["fare_media_id", "fare_media_name", "fare_media_type"],
        "fare_products.txt" => &[
            "fare_product_id",
            "fare_product_name",
            "rider_category_id",
            "fare_media_id",
            "amount",
            "currency",
        ],
        "fare_leg_rules.txt" => &[
            "leg_group_id",
            "network_id",
            "from_area_id",
            "to_area_id",
            "from_timeframe_group_id",
            "to_timeframe_group_id",
            "fare_product_id",
            "rule_priority",
        ],
        // `rule_priority` is NOT HERE: in the 27 April 2026 Schedule Reference this field is
        // defined only in `fare_leg_rules.txt` (FLG_007 checks it there). The official field list
        // for fare_transfer_rules is the seven entries below.
        "fare_transfer_rules.txt" => &[
            "from_leg_group_id",
            "to_leg_group_id",
            "transfer_count",
            "duration_limit",
            "duration_limit_type",
            "fare_transfer_type",
            "fare_product_id",
        ],
        "timeframes.txt" => &["timeframe_group_id", "start_time", "end_time", "service_id"],
        "areas.txt" => &["area_id", "area_name"],
        "stop_areas.txt" => &["area_id", "stop_id"],
        "networks.txt" => &["network_id", "network_name"],
        "route_networks.txt" => &["network_id", "route_id"],
        "fare_leg_join_rules.txt" => &[
            "from_network_id",
            "to_network_id",
            "from_stop_id",
            "to_stop_id",
        ],
        // ── Flex ──
        "booking_rules.txt" => &[
            "booking_rule_id",
            "booking_type",
            "prior_notice_duration_min",
            "prior_notice_duration_max",
            "prior_notice_last_day",
            "prior_notice_last_time",
            "prior_notice_start_day",
            "prior_notice_start_time",
            "prior_notice_service_id",
            "message",
            "pickup_message",
            "drop_off_message",
            "phone_number",
            "info_url",
            "booking_url",
        ],
        "location_groups.txt" => &["location_group_id", "location_group_name"],
        "location_group_stops.txt" => &["location_group_id", "stop_id"],
        // ── GTFS-JP (Japanese profile) — columns verified against the official format reference
        // and a Tokyo Toei feed. Note: columns prefixed with jp_ are already skipped by ARC_017
        // (an additional safety margin).
        "agency_jp.txt" => &[
            "agency_id",
            "agency_official_name",
            "agency_zip_number",
            "agency_address",
            "agency_president_pos",
            "agency_president_name",
        ],
        "office_jp.txt" => &["office_id", "office_name", "office_url", "office_phone"],
        "routes_jp.txt" => &[
            "route_id",
            "route_update_date",
            "origin_stop",
            "via_stop",
            "destination_stop",
        ],
        "pattern_jp.txt" => &[
            "jp_pattern_id",
            "route_update_date",
            "origin_stop",
            "via_stop",
            "destination_stop",
        ],
        _ => &[],
    }
}

/// Is this an AppleDouble sidecar file added to the ZIP by macOS?
///
/// When Finder compresses a folder, it writes a resource-fork entry for every file at
/// `__MACOSX/…/._<ad>`. Because `._agency.txt` ends in `.txt`, the old code treated it as a
/// "GTFS file in a subdirectory"; it is actually a binary metadata sidecar, not a GTFS file,
/// and no parser reads it.
///
/// In the full-catalog run (#144), **all 42 of 42** feeds receiving ARC_024 did so only for
/// this reason. The more serious effect was in `detect_wrapped_root`: `__MACOSX/` looked like a
/// second top-level folder, so wrapped-feed tolerance was dropped and valid feeds became fatal
/// with ARC_004 — zero analysis.
fn is_macos_metadata_entry(normalized_name: &str) -> bool {
    normalized_name == "__MACOSX" || normalized_name.starts_with("__MACOSX/")
}

/// Returns the root prefix of a feed wrapped in one top-level folder (e.g. `"TEST GTFS/"`).
///
/// Some publishers package the ZIP in one folder: `TEST GTFS/agency.txt`. The spec requires
/// files at the root, so these receive ARC_024 — but strict behavior gives the user ARC_004
/// fatal and **zero analysis**, even though the feed itself is valid.
///
/// Tolerance is deliberately narrow; all three conditions must hold:
/// 1. no `.txt` exists at the root (if one does, the feed is not wrapped; leave it alone),
/// 2. all `.txt` entries are directly under ONE AND THE SAME top-level folder (no second level),
/// 3. that folder contains all required GTFS files.
///
/// Otherwise `None` → the old behavior. A "non-GTFS repository ZIP" (e.g. `.csv` files in one
/// folder) remains fatal; tolerance rescues only a genuinely valid feed.
fn detect_wrapped_root(entry_names: &[String]) -> Option<String> {
    let normalized: Vec<String> = entry_names.iter().map(|n| n.replace('\\', "/")).collect();
    // `__MACOSX/…/._x.txt` entries do NOT participate in root discovery; otherwise they would
    // look like a second top-level folder (or a second level) and disable tolerance.
    let txt = || {
        normalized
            .iter()
            .filter(|n| n.ends_with(".txt") && !is_macos_metadata_entry(n))
    };

    if txt().any(|n| !n.contains('/')) {
        return None; // `.txt` at root → not wrapped
    }

    let mut prefix: Option<&str> = None;
    for name in txt() {
        let (dir, rest) = name.split_once('/')?;
        if rest.contains('/') {
            return None; // second-level subdirectory → no tolerance
        }
        match prefix {
            None => prefix = Some(dir),
            Some(p) if p == dir => {}
            Some(_) => return None, // more than one top-level folder
        }
    }

    let dir = prefix?;
    REQUIRED_FILES
        .iter()
        .all(|f| normalized.iter().any(|n| n == &format!("{dir}/{f}")))
        .then(|| format!("{dir}/"))
}

// ── Main parse function ──────────────────────────────────────────────────────

/// K1 parse layer. Produces a raw file map and ARC_* notices from ZIP bytes.
///
/// Fatal conditions (the pipeline stops):
/// - ARC_001: if the ZIP cannot be opened
/// - ARC_002: if a required file cannot be read as UTF-8
/// - ARC_004: if required files are missing
/// - ARC_013: if CSV tokenization fails for a required file
///
/// ARC_008: produces a Critical notice and lets the pipeline continue (non-Fatal).
///
/// `cfg` is read only for the ARC_022 threshold (`max_file_rows`). K1 was historically
/// unconfigured; the signature changed on 2026-08-07 (issue #71) because the rule could not be
/// fixture-tested while the threshold was hard-coded (a 1M-row ZIP). The same field is passed
/// to K2's streaming paths — making the knob work ONLY in K1 would silently leave it ineffective
/// for two files that actually exceed 1M rows (stop_times/shapes).
pub fn parse(zip_bytes: &[u8], cfg: &ValidatorConfig) -> Result<K1Result, FatalError> {
    parse_with_limits(zip_bytes, cfg, DEFAULT_DECOMPRESSION_LIMITS, None)
}

/// Shared body for K1's native and SDK callers. Because the SDK has less WASM memory, it applies
/// a lower limit to individual files that are not streamed; the native CLI's large-feed behavior
/// is preserved.
pub fn parse_with_limits(
    zip_bytes: &[u8],
    cfg: &ValidatorConfig,
    limits: DecompressionLimits,
    buffered_entry_limit: Option<u64>,
) -> Result<K1Result, FatalError> {
    k1dbg!("[K1] parse başladı, {} bayt", zip_bytes.len());
    let cursor = std::io::Cursor::new(zip_bytes);
    k1dbg!("[K1] ZipArchive::new çağrılıyor...");
    // The library message can identify the WRONG layer of corruption (see
    // `diagnose_unreadable_zip`), so raw-byte diagnosis takes PRIORITY and the library text is
    // appended afterward.
    let mut archive = zip::ZipArchive::new(cursor).map_err(|e| {
        match diagnose_unreadable_zip(zip_bytes) {
            Some(diagnosis) => diagnosis.params.into_iter().fold(
                FatalError::new(
                    FatalCode::ZipUnreadable,
                    format!(
                        "ZIP arşivi açılamadı: {}. Kütüphane hatası: {e}",
                        diagnosis.text
                    ),
                )
                .variant(&format!("archive.{}", diagnosis.kind)),
                |err, (key, value)| err.param(key, value),
            ),
            None => FatalError::new(
                FatalCode::ZipUnreadable,
                format!("ZIP arşivi açılamadı: {e}"),
            )
            .variant("archive"),
        }
        .param("detail", &e)
    })?;
    k1dbg!("[K1] archive açıldı: {} entry", archive.len());
    if archive.len() > MAX_ZIP_ENTRIES {
        return Err(FatalError::new(
            FatalCode::ResourceLimit,
            format!(
                "ZIP merkezi dizini {} girdilik güvenlik sınırını aşıyor.",
                MAX_ZIP_ENTRIES
            ),
        )
        .variant("zip_entries")
        .param("limit", MAX_ZIP_ENTRIES));
    }

    // ARC_009 false-positive guard: even if calendar_dates.txt is empty (or header-only), it is
    // not CRITICAL when calendar.txt defines service (calendar_dates is conditionally required
    // and optional when calendar.txt is present). Compute this before the loop so it is
    // independent of entry order. If calendar.txt is ABSENT, empty calendar_dates means "no
    // service is defined" → ARC_009 remains. A feed wrapped in one top-level folder uses that
    // folder as its root (see detect_wrapped_root).
    let mut entry_names = Vec::with_capacity(archive.len());
    let mut metadata_bytes = 0usize;
    for (i, name) in archive.file_names().enumerate() {
        let name = name.map_err(|e| entry_name_error(i, &e))?;
        metadata_bytes = metadata_bytes.saturating_add(name.len());
        if metadata_bytes > MAX_ZIP_METADATA_BYTES {
            return Err(FatalError::new(
                FatalCode::ResourceLimit,
                format!(
                    "ZIP merkezi dizini {MAX_ZIP_METADATA_BYTES} bayt metadata sınırını aşıyor."
                ),
            )
            .variant("zip_metadata")
            .param("limit", MAX_ZIP_METADATA_BYTES));
        }
        entry_names.push(name.to_string());
    }
    let mut partial = PartialReport::default();
    let root_prefix = detect_wrapped_root(&entry_names);
    if let Some(prefix) = &root_prefix {
        partial.mark_root_error(format!(
            "GTFS files are not at the ZIP root; root prefix is '{prefix}'"
        ));
    }
    if root_prefix.is_none()
        && entry_names.iter().any(|name| {
            let normalized = name.replace('\\', "/");
            normalized.ends_with(".txt")
                && normalized.contains('/')
                && !is_macos_metadata_entry(&normalized)
        })
    {
        partial.mark_root_error(
            "GTFS .txt files were found in multiple/deep subdirectories; root discovery is unsafe",
        );
    }
    // 🔴 "FILE EXISTS" and "FILE CARRIES DATA" are separate questions, and suppression must
    // inspect the SECOND. A zero-byte `calendar.txt` does not EXCUSE an empty `calendar_dates.txt`:
    // in that case the feed has no service definition.
    let mut file_has_data = |name: &str| -> bool {
        archive
            .index_for_name(name)
            .and_then(|i| archive.by_index_raw(i).ok())
            .map(|f| f.size() > 0)
            .unwrap_or(false)
    };
    let calendar_entry = format!("{}calendar.txt", root_prefix.as_deref().unwrap_or(""));
    let has_calendar_txt = entry_names
        .iter()
        .any(|n| n.replace('\\', "/") == calendar_entry)
        && file_has_data(&calendar_entry);
    // 🔑 THE SAME TOLERANCE ALSO APPLIES IN THE OPPOSITE DIRECTION (#166). Spec: "calendar.txt —
    // Required UNLESS all dates of service are defined in calendar_dates.txt."
    // Therefore an empty calendar.txt is VALID when service data is defined in calendar_dates.txt.
    // Suppression had been written in only one direction: empty `calendar_dates.txt` was
    // tolerated, while empty `calendar.txt` was reported as CRITICAL. In the full catalog run,
    // 59 of `ARC_009`'s 104 CRITICAL feeds were exactly this case, and MobilityData reported none
    // of them — its `empty_file` fires only for REQUIRED files, not conditionally required ones.
    //
    // 🔴 "If both are empty, DQ_005 owns it" HAD BEEN WRITTEN BUT NOT MEASURED. In run 7,
    // both files were 0 BYTES in `ntd-60089` and five sibling feeds; `DQ_005` did not fire and
    // no rule said that the feed had no service. MobilityData emitted `empty_file` for seven
    // files while we emitted five — these two files were exactly the difference.
    // Suppression now requires the other file to CARRY DATA, not merely to exist.
    //
    // ⚠️ LIMIT: the check uses uncompressed SIZE (`size() > 0`), not the parse result. A zero-byte
    // file is caught; a "header-only" file is not, because suppression flags are computed BEFORE
    // the file loop and no file has been parsed yet. All six measured corpus cases were 0 bytes.
    let calendar_dates_entry =
        format!("{}calendar_dates.txt", root_prefix.as_deref().unwrap_or(""));
    let has_calendar_dates_txt = entry_names
        .iter()
        .any(|n| n.replace('\\', "/") == calendar_dates_entry)
        && file_has_data(&calendar_dates_entry);

    // ARC_036: stream-view consistency. Central-directory metadata is collected here; raw-byte
    // comparison is performed by `count_inconsistent_stream_headers`. Use `by_index_raw` because
    // the decision does not require DECOMPRESSION, only reading header fields.
    let stream_entries: Vec<(u64, u64, u64, u32)> = (0..archive.len())
        .filter_map(|i| {
            archive.by_index_raw(i).ok().map(|zf| {
                (
                    zf.header_start(),
                    zf.compressed_size(),
                    zf.size(),
                    zf.crc32(),
                )
            })
        })
        .collect();
    let inconsistent_stream_entries = count_inconsistent_stream_headers(zip_bytes, &stream_entries);

    let mut notices: Vec<Notice> = Vec::new();
    let mut counter: u32 = 0;
    let mut geojson_location_ids: std::collections::HashSet<String> =
        std::collections::HashSet::new();
    let mut valid_geojson_location_ids: std::collections::HashSet<String> =
        std::collections::HashSet::new();
    let mut geojson_geometries: std::collections::HashMap<String, LocationGeometry> =
        std::collections::HashMap::new();
    let mut raw_files: RawFiles = HashMap::new();
    let mut present_files: HashSet<String> = HashSet::new();
    let mut custom_file_headers: std::collections::HashMap<String, Vec<String>> =
        std::collections::HashMap::new();
    // ARC_035: files that exist but carry no data rows. Emit the finding at the end of traversal,
    // next to ARC_004 — the Flex exemption (`has_valid_locations_geojson`) is reliable only
    // there because ZIP entry order may place locations.geojson later.
    let mut empty_files: HashSet<String> = HashSet::new();
    // locations.geojson is handled specially, so it is added to present_files in the branch below.
    let mut has_valid_locations_geojson = false;
    // Per-file primary-key column index for DQ_016 — the first "*_id" column.
    let _dq016_dummy = ();

    // #46: zip-bomb protection — total bytes decompressed across all entries (the whole traversal).
    let mut total_decompressed: u64 = 0;
    for i in 0..archive.len() {
        k1dbg!("[K1] by_index({i})...");
        let mut zf = archive.by_index(i).map_err(|e| {
            FatalError::new(
                FatalCode::ZipUnreadable,
                format!("ZIP dosyası okunamadı (index {i}): {e}"),
            )
            .variant("entry_index")
            .param("index", i)
            .param("detail", &e)
        })?;

        let entry_name = zf.name().map_err(|e| entry_name_error(i, &e))?.into_owned();
        // For a wrapped feed, remove the root prefix; all subsequent logic sees the file as if it
        // were at the root. Finding messages still show the REAL ZIP path (entry_name).
        let raw_name = match &root_prefix {
            Some(prefix) => entry_name
                .replace('\\', "/")
                .strip_prefix(prefix.as_str())
                .map_or_else(|| entry_name.clone(), str::to_string),
            None => entry_name.clone(),
        };
        if raw_name.is_empty() {
            continue; // the wrapper folder entry itself
        }
        // macOS resource fork: not GTFS content; no rule measures it (#144).
        // Drop it BEFORE ARC_024, otherwise `._agency.txt` would count as a "subdirectory .txt".
        if is_macos_metadata_entry(&entry_name.replace('\\', "/"))
            || is_macos_metadata_entry(&raw_name.replace('\\', "/"))
        {
            continue;
        }
        // Tolerance applied: process the file, but still report that it is in a subdirectory.
        if raw_name != entry_name && raw_name.ends_with(".txt") {
            notices.push(make_notice(
                &mut counter, "ARC_024",
                EntityType::File, Some(entry_name.clone()),
                Some(&entry_name), None, None,
                Some(entry_name.clone()),
                format!("'{entry_name}' alt dizinde bulunuyor — GTFS dosyaları ZIP kök dizininde düz olmalıdır."),
                "Move the file to the root of the ZIP archive.",
            ));
        }
        if raw_name.ends_with(".txt") && zf.unix_mode().is_some_and(|mode| mode & 0o400 == 0) {
            notices.push(make_notice(
                &mut counter,
                "ARC_027",
                EntityType::File,
                Some(raw_name.clone()),
                Some(&raw_name),
                None,
                None,
                zf.unix_mode().map(|m| format!("{m:o}")),
                format!("'{raw_name}' ZIP girdisinde kullanıcı okuma izni bulunmuyor."),
                "Add the user-read (0400) bit to the ZIP entry's Unix permissions.",
            ));
        }

        // ARC_023: nested ZIP file inside the ZIP (root OR subdirectory).
        // 🔴 Previously only a .zip at the ROOT was seen; a nested ZIP in a subdirectory was
        // skipped without any finding (ARC_007 also filtered it with `contains('/')`). Five corpus
        // feeds — tdg-83019 · tld-768_1 · mdb-3340 · mdb-657 · tld-752 — carry all their GTFS in
        // a nested ZIP in a subdirectory; users saw only "required file missing" and could not
        // learn WHY. Location does not change the rule: GTFS does not support nested ZIPs.
        if raw_name.ends_with(".zip") {
            notices.push(make_notice(
                &mut counter, "ARC_023",
                EntityType::File, Some(raw_name.clone()),
                Some(&raw_name), None, None,
                Some(raw_name.clone()),
                format!("'{raw_name}' GTFS ZIP içinde başka bir ZIP dosyası — GTFS bu formatı desteklemiyor."),
                "Remove the nested ZIP file; GTFS files must be directly inside a single flat ZIP archive.",
            ));
            continue;
        }

        // locations.geojson: Flex GeoJSON location file (special handling)
        if raw_name == "locations.geojson" {
            present_files.insert(raw_name.clone());
            let declared_size = zf.size();
            let compressed_size = zf.compressed_size();
            let mut geo_limits = limits;
            geo_limits.max_entry_decompressed =
                geo_limits.max_entry_decompressed.min(MAX_GEOJSON_BYTES);
            let mut buf = Vec::with_capacity(prealloc_capacity(
                declared_size.min(MAX_GEOJSON_BYTES) as usize,
            ));
            let mut guarded = GuardedReader::new(
                &mut zf,
                geo_limits,
                &mut total_decompressed,
                compressed_size,
            );
            match guarded.read_to_end(&mut buf) {
                Ok(_) => {
                    has_valid_locations_geojson = validate_locations_geojson(
                        &buf,
                        &raw_name,
                        &mut notices,
                        &mut counter,
                        &mut geojson_location_ids,
                        &mut valid_geojson_location_ids,
                        &mut geojson_geometries,
                    );
                    if !has_valid_locations_geojson {
                        partial.mark_unavailable(raw_name.clone());
                    }
                }
                Err(e) => {
                    // locations.geojson is optional; an unreadable entry must not make
                    // stops.txt optional, and must be visible in PARTIAL metadata.
                    notices.push(make_notice(
                        &mut counter,
                        "LOC_012",
                        EntityType::File,
                        Some(raw_name.clone()),
                        Some(&raw_name),
                        None,
                        None,
                        Some(e.to_string()),
                        format!("'{raw_name}' okunamadı: {e}"),
                        "Regenerate locations.geojson as valid UTF-8 JSON.",
                    ));
                    partial.mark_unavailable(raw_name.clone());
                }
            }
            continue;
        }

        // ARC_024: .txt file in a subdirectory — standard parsers skip these files.
        if raw_name.ends_with(".txt") && (raw_name.contains('/') || raw_name.contains('\\')) {
            notices.push(make_notice(
                &mut counter, "ARC_024",
                EntityType::File, Some(raw_name.clone()),
                Some(&raw_name), None, None,
                Some(raw_name.clone()),
                format!("'{raw_name}' alt dizinde bulunuyor — GTFS dosyaları ZIP kök dizininde düz olmalıdır."),
                "Move the file to the root of the ZIP archive.",
            ));
            continue;
        }

        // ARC_007: unrecognised (non-.txt) files at the root are also marked as unknown files
        // (e.g. .pdf/.out/.csv). .zip → ARC_023, locations.geojson, and subdirectory .txt →
        // ARC_024 were handled above; they continued before reaching here.
        if !raw_name.ends_with(".txt") {
            // Subdirectory files are normally silent (a GitHub repository ZIP may carry dozens
            // of LICENSE/README/source files; reporting all of them is noise).
            // ONE exception: an entry whose name is a GTFS file stem but whose extension is NOT
            // `.txt` — such as `agency.csv`. In corpus 13, two feeds (mdb-3434 · mdb-3135) carried
            // all GTFS in a subdirectory with `.csv` extensions, and the only output was "required
            // file missing"; no finding showed that the files were THERE.
            if !raw_name.contains('/') && !raw_name.contains('\\')
                || gtfs_stem_with_wrong_extension(&raw_name)
            {
                notices.push(make_notice(
                    &mut counter,
                    "ARC_007",
                    EntityType::File,
                    Some(raw_name.clone()),
                    Some(&raw_name),
                    None,
                    None,
                    Some(raw_name.clone()),
                    format!("'{raw_name}' GTFS spesifikasyonunda tanımlı değil."),
                    "Remove non-GTFS files from the ZIP.",
                ));
            }
            continue;
        }
        // Process only root-level .txt files (subdirectory .txt already received ARC_024 above).
        if raw_name.contains('/') || raw_name.contains('\\') {
            continue;
        }

        present_files.insert(raw_name.clone());
        let is_required = REQUIRED_FILES.contains(&raw_name.as_str());
        let is_known = KNOWN_FILES.contains(&raw_name.as_str());

        // Read bytes (before the is_known check for ARC_011).
        k1dbg!(
            "[K1] okuma başladı: {raw_name} (compressed: {} b)",
            zf.compressed_size()
        );
        let uncompressed_hint = zf.size().min(usize::MAX as u64) as usize;
        // #38: K2 streams large files from the ZIP; K1 reads only the header row.
        // 8 KB partial read for stop_times, trips, and calendar_dates; raw_text=None.
        let is_zip_stream = matches!(
            raw_name.as_str(),
            "stop_times.txt" | "shapes.txt" | "trips.txt" | "calendar_dates.txt"
        );
        let _is_stop_times = raw_name == "stop_times.txt"; // legacy: is_zip_stream now only for ARC_011 bytes
                                                           // Log BEFORE allocation: if a crash occurs, the last line shows which file OOMed.
        crate::timing::mem_note(&format!(
            "K1::alloc::{} hint={}MB{}",
            raw_name,
            uncompressed_hint / 1_048_576,
            if is_zip_stream { " (partial)" } else { "" }
        ));
        crate::timing::mem_log(&format!("K1::pre-alloc::{raw_name}"));
        let mut raw_vec = if is_zip_stream {
            Vec::with_capacity(8192)
        } else {
            Vec::with_capacity(prealloc_capacity(uncompressed_hint))
        };
        {
            let _t = crate::timing::Timer::start(format!("K1::decompress::{raw_name}"));
            // #46: every entry is read through GuardedReader, which counts decompressed bytes.
            // If the bomb trips, read() returns an error; distinguish it from a plain ZIP error
            // with tripped() and produce an ARC_029 (DecompressionLimit) Fatal.
            let comp_size = zf.compressed_size();
            let mut entry_limits = limits;
            if !is_zip_stream {
                if let Some(limit) = buffered_entry_limit {
                    entry_limits.max_entry_decompressed =
                        entry_limits.max_entry_decompressed.min(limit);
                }
            }
            let mut guarded =
                GuardedReader::new(&mut zf, entry_limits, &mut total_decompressed, comp_size);
            if is_zip_stream {
                let mut chunk = [0u8; 64 * 1024];
                let mut malformed_eol = false;
                let mut previous_was_cr = false;
                let mut scanned = 0usize;
                let mut reached_eof = false;
                // K1 needs only a small sample for schema/encoding checks, but it must
                // still give GuardedReader enough input to detect a compression bomb.
                // Probe up to the ratio floor, then leave the bounded full pass to K2.
                // This avoids decompressing multi-gigabyte stream entries twice while
                // preserving ARC_029 for highly-compressible payloads.
                let probe_limit = usize::try_from(entry_limits.ratio_floor)
                    .unwrap_or(usize::MAX)
                    .saturating_add(1)
                    .max(8192);
                loop {
                    let n = match guarded.read(&mut chunk) {
                        Ok(n) => n,
                        Err(e) => return Err(read_fatal(&guarded, &raw_name, &e)),
                    };
                    if n == 0 {
                        reached_eof = true;
                        break;
                    }
                    scanned = scanned.saturating_add(n);
                    if previous_was_cr && chunk[0] != b'\n' {
                        malformed_eol = true;
                    }
                    malformed_eol |= chunk[..n]
                        .windows(2)
                        .any(|p| p[0] == b'\r' && p[1] != b'\n');
                    previous_was_cr = chunk[n - 1] == b'\r';
                    let keep = (8192usize.saturating_sub(raw_vec.len())).min(n);
                    raw_vec.extend_from_slice(&chunk[..keep]);
                    if scanned >= probe_limit {
                        break;
                    }
                }
                if reached_eof {
                    malformed_eol |= previous_was_cr;
                }
                if malformed_eol {
                    notices.push(make_notice(
                        &mut counter,
                        "ARC_026",
                        EntityType::File,
                        Some(raw_name.clone()),
                        Some(&raw_name),
                        None,
                        None,
                        Some("CR not followed by LF".to_string()),
                        format!("'{raw_name}' CRLF veya LF dışında satır sonu içeriyor."),
                        "Rewrite the line endings as LF or CRLF.",
                    ));
                }
                // The 8192-byte cut can split a multibyte UTF-8 character.
                // Without truncation, the from_utf8 below would treat a VALID file as corrupt
                // and Utf8Critical on a required file would reject the entire feed.
                truncate_to_utf8_boundary(&mut raw_vec);
            } else if let Err(e) = guarded.read_to_end(&mut raw_vec) {
                return Err(read_fatal(&guarded, &raw_name, &e));
            }
        }
        if !is_zip_stream && has_malformed_eol(&raw_vec) {
            notices.push(make_notice(
                &mut counter,
                "ARC_026",
                EntityType::File,
                Some(raw_name.clone()),
                Some(&raw_name),
                None,
                None,
                Some("CR not followed by LF".to_string()),
                format!("'{raw_name}' CRLF veya LF dışında satır sonu içeriyor."),
                "Rewrite the line endings as LF or CRLF.",
            ));
        }

        // ARC_011: file size — for every root-level .txt file (including unknown files).
        // In zip_stream partial_read, raw_vec contains only the header (~8 KB); the real size
        // comes from ZIP metadata.
        let file_size_bytes = if is_zip_stream {
            uncompressed_hint
        } else {
            raw_vec.len()
        };
        notices.push(make_notice(
            &mut counter,
            "ARC_011",
            EntityType::File,
            Some(raw_name.clone()),
            Some(&raw_name),
            None,
            None,
            Some(format!("{file_size_bytes} bytes")),
            format!("'{raw_name}' boyutu: {file_size_bytes} bayt."),
            "Informational notice; no action required.",
        ));

        // ARC_007: unknown file — skip after recording its size.
        if !is_known {
            // Unknown files are intentionally not retained in `raw_files`; keep only
            // their header names so K4 can enforce GTFS-JP's reserved namespace rule
            // without retaining arbitrary custom-file bodies in memory.
            let (unknown_bytes, _) = strip_bom(&raw_vec);
            if let Ok(text) = std::str::from_utf8(unknown_bytes) {
                if let Ok((mut header_rows, _, _)) = tokenize_csv(text, Some(1)) {
                    if let Some(row) = header_rows.pop() {
                        custom_file_headers.insert(
                            raw_name.clone(),
                            row.into_iter().map(|h| h.trim().to_string()).collect(),
                        );
                    }
                }
            }
            notices.push(make_notice(
                &mut counter,
                "ARC_007",
                EntityType::File,
                Some(raw_name.clone()),
                Some(&raw_name),
                None,
                None,
                Some(raw_name.clone()),
                format!("'{raw_name}' GTFS spesifikasyonunda tanımlı değil."),
                "Remove non-GTFS files from the ZIP.",
            ));
            continue;
        }

        // ARC_006: optional file present (INFO).
        if !is_required {
            notices.push(make_notice(
                &mut counter,
                "ARC_006",
                EntityType::File,
                Some(raw_name.clone()),
                Some(&raw_name),
                None,
                None,
                None,
                format!("İsteğe bağlı dosya mevcut: '{raw_name}'."),
                "Informational notice; no action required.",
            ));
        }

        // BOM check.
        let (bytes, has_bom) = strip_bom(&raw_vec);
        if has_bom {
            // ARC_010 and DQ_014 have the same condition — do not fire both separately;
            // DQ_014 is referenced only in the ARC_002 suppressor list, and ARC_010 covers it.
            notices.push(make_notice(
                &mut counter, "ARC_010",
                EntityType::File, Some(raw_name.clone()),
                Some(&raw_name), None, None,
                Some("UTF-8 BOM (EF BB BF)".to_string()),
                format!("'{raw_name}' dosyasında UTF-8 BOM var — bazı parser'lar BOM'u ilk alan adının parçası olarak okur."),
                "Re-save the file as UTF-8 without BOM.",
            ));
        }

        k1dbg!("[K1] okundu: {raw_name} ({} bayt)", bytes.len());
        // UTF-8 validation — from_utf8 returns a zero-cost &str (no copy).
        // Invalid UTF-8 files are skipped for safe typed validation; no lossy record is produced.
        let text: &str = match std::str::from_utf8(bytes) {
            Ok(s) => s,
            Err(_) => {
                // ARC_002: Critical UTF-8 violation.
                notices.push(make_notice(
                    &mut counter,
                    "ARC_002",
                    EntityType::File,
                    Some(raw_name.clone()),
                    Some(&raw_name),
                    None,
                    None,
                    None,
                    format!("'{raw_name}' UTF-8 kodlamasıyla okunamıyor."),
                    "Re-save the file with UTF-8 encoding.",
                ));
                partial.mark_unavailable(raw_name.clone());
                if !is_required {
                    notices.push(make_notice(
                        &mut counter,
                        "ARC_003",
                        EntityType::File,
                        Some(raw_name.clone()),
                        Some(&raw_name),
                        None,
                        None,
                        None,
                        format!("'{raw_name}' UTF-8 dışı olduğu için dosya atlandı."),
                        "Save all GTFS files with UTF-8 encoding.",
                    ));
                }
                continue;
            }
        };

        // OOM fix Plan A + #15 W3: stop_times.txt and shapes.txt can be very large; fully
        // expanding them into Vec<Vec<SmolStr>> in K1 uses hundreds of MB (~920 MB measured in a
        // shape-heavy feed). Tokenize ONLY the headers of these files; pass the body to K2 as raw
        // text (`raw_text`) for streaming. Per-row notices (ARC_012/016/018/021, DQ_016, ARC_022,
        // empty-data ARC_009) moved to K2's stream pass.
        // #38: trips.txt + calendar_dates.txt are streamed in K1.
        // trips: ~3M trips × 8 fields in Vec<Vec<SmolStr>> (~576 MB) overlaps the TripRecord Vec.
        // calendar_dates: a Swiss feed with 8M+ rows uses 700MB+ in rows Vec<Vec<SmolStr>>, leaving
        // no room to decompress stop_times.txt.
        let stream_mode = raw_name == "stop_times.txt"
            || raw_name == "shapes.txt"
            || raw_name == "trips.txt"
            || raw_name == "calendar_dates.txt";

        k1dbg!("[K1] tokenizing: {raw_name}");
        // CSV tokenization — only the header (Some(0)) in stream_mode, UNLIMITED everywhere else.
        // #185: the SDK path applied a `cfg.max_file_rows` row budget; files over 1M rows were
        // silently cut off and the dangling references produced by the cut were blamed on the
        // feed. Measurement showed the limit was unnecessary (VBB took 18.9 s in the browser
        // whole and 117 s truncated: the cost was not reading rows but producing spurious
        // notices). `max_file_rows` is only the ARC_022 threshold; `buffered_entry_limit` remains
        // the byte limit.
        let _t = crate::timing::Timer::start(format!("K1::tokenize::{raw_name}"));
        let (mut records, _, rfc4180) = match tokenize_csv(
            text,
            if stream_mode { Some(0) } else { None },
        ) {
            Ok(r) => r,
            Err(msg) => {
                // ARC_013.
                notices.push(make_notice(
                    &mut counter, "ARC_013",
                    EntityType::File, Some(raw_name.clone()),
                    Some(&raw_name), None, None,
                    Some("issue=unclosed_quote".to_string()),
                    format!("'{raw_name}' CSV tokenization hatası: {msg}"),
                    "Check the CSV format; ensure all quoted fields are properly closed.",
                ));
                partial.mark_unavailable(raw_name.clone());
                continue;
            }
        };

        // stream_mode: K1 tokenizes only the header, but still detects an unclosed quote in the
        // body (ARC_013) — preserve Fatal behavior for required files (NO allocation).
        // zip_stream files have no body; checking for an unclosed quote would be a false positive.
        // 🔴 THERE USED TO BE A DEAD BRANCH HERE (measured and removed on 2026-08-07).
        // Its condition was `stream_mode && !is_zip_stream`; both lists named the SAME four files
        // (`stop_times`/`shapes`/`trips`/`calendar_dates`), so the condition could NEVER be true.
        // When `is_zip_stream` expanded from stop_times to four files in the `#38` streaming pass,
        // the branch silently died. A panic was added and the full test suite was run: it never ran.
        //
        // ⚠️ WHAT WAS REMOVED WAS A CLAIM, NOT BEHAVIOR: an unclosed quote in a stream-file body
        // (`ARC_013`) is **NOT CHECKED ANYWHERE** — K2 readers tolerate it "best-effort". This gap
        // was hidden behind dead code; it is now documented. RFC 4180 quote-escaping violations
        // (`ARC_033`), however, are checked in K2 where the bytes are actually read.

        // ARC_009: empty file.
        if records.is_empty() {
            // An empty calendar_dates.txt is a false positive when calendar.txt defines service → suppress.
            let conditional_pair_covers_it = (raw_name == "calendar_dates.txt" && has_calendar_txt)
                || (raw_name == "calendar.txt" && has_calendar_dates_txt);
            if !conditional_pair_covers_it {
                empty_files.insert(raw_name.clone());
                // An empty required file belongs to ARC_035 (Critical·Spec); ARC_009 reports only
                // optional files and is always Info.
                if !REQUIRED_FILES.contains(&raw_name.as_str()) {
                    notices.push(make_notice(
                        &mut counter,
                        "ARC_009",
                        EntityType::File,
                        Some(raw_name.clone()),
                        Some(&raw_name),
                        None,
                        None,
                        None,
                        format!("'{raw_name}' dosyası boş veya yalnızca başlık satırı içeriyor."),
                        "Add at least one data row to the file (or remove the file if it is not needed).",
                    ));
                }
            }
            continue;
        }

        // Header row (take ownership; no clone).
        let raw_headers = records.remove(0);

        // ARC_014: whitespace in header.
        for (col_i, hdr) in raw_headers.iter().enumerate() {
            if hdr != hdr.trim() {
                notices.push(make_notice(
                    &mut counter,
                    "ARC_014",
                    EntityType::File,
                    Some(raw_name.clone()),
                    Some(&raw_name),
                    Some(1),
                    Some(hdr.as_str()),
                    Some(format!("{hdr:?}")),
                    format!("'{raw_name}' başlığında '{hdr}' sütununda gereksiz boşluk var."),
                    "Remove leading and trailing whitespace from CSV header field names.",
                ));
                let _ = col_i;
            }
        }

        // Clean headers (trim).
        let headers: Vec<String> = raw_headers.iter().map(|h| h.trim().to_string()).collect();

        // ARC_019: empty column name in header.
        for hdr in &headers {
            if hdr.is_empty() {
                notices.push(make_notice(
                    &mut counter,
                    "ARC_019",
                    EntityType::File,
                    Some(raw_name.clone()),
                    Some(&raw_name),
                    Some(1),
                    None,
                    Some("\"\"".to_string()),
                    format!("'{raw_name}' başlığında boş sütun adı var."),
                    "Ensure all header column names are non-empty.",
                ));
                break; // Once per file is enough.
            }
        }

        // ARC_015: duplicate column.
        let mut seen_hdrs: HashSet<&str> = HashSet::new();
        for hdr in &headers {
            if !seen_hdrs.insert(hdr.as_str()) {
                notices.push(make_notice(
                    &mut counter,
                    "ARC_015",
                    EntityType::File,
                    Some(raw_name.clone()),
                    Some(&raw_name),
                    Some(1),
                    Some(hdr.as_str()),
                    Some(hdr.clone()),
                    format!("'{raw_name}' dosyasında '{hdr}' sütunu tekrarlanıyor."),
                    "Remove the duplicate column from the header row.",
                ));
            }
        }

        // ARC_017: unknown column (jp_ prefix = GTFS-JP extension; skip). At most
        // ARC_017_MAX_PER_FILE columns are listed one by one; the rest become one summary
        // notice, so a header with 200,000 junk columns cannot inflate the report (2026-11
        // security review: 119 MB of JSON). The corpus maximum is 81 per FEED, so real
        // feeds never reach the cap.
        let known_cols = known_columns(&raw_name);
        if !known_cols.is_empty() {
            let mut unknown = 0usize;
            for hdr in &headers {
                if !known_cols.contains(&hdr.as_str()) && !hdr.starts_with("jp_") {
                    unknown += 1;
                    if unknown > ARC_017_MAX_PER_FILE {
                        continue;
                    }
                    notices.push(make_notice(
                        &mut counter,
                        "ARC_017",
                        EntityType::File,
                        Some(raw_name.clone()),
                        Some(&raw_name),
                        Some(1),
                        Some(hdr.as_str()),
                        Some(hdr.clone()),
                        format!(
                            "'{raw_name}' dosyasında '{hdr}' GTFS spesifikasyonunda tanımlı değil."
                        ),
                        "Remove columns not defined in the GTFS specification.",
                    ));
                }
            }
            if unknown > ARC_017_MAX_PER_FILE {
                let omitted = unknown - ARC_017_MAX_PER_FILE;
                let mut notice = make_notice(
                    &mut counter,
                    "ARC_017",
                    EntityType::File,
                    Some(raw_name.clone()),
                    Some(&raw_name),
                    Some(1),
                    None,
                    Some(omitted.to_string()),
                    format!(
                        "'{raw_name}' dosyasında {omitted} bilinmeyen sütun daha var; yalnızca ilk {ARC_017_MAX_PER_FILE} tanesi ayrı ayrı raporlandı."
                    ),
                    "Remove columns not defined in the GTFS specification.",
                );
                let details = notice.details.get_or_insert_with(Default::default);
                details.insert("message_variant", "truncated".to_string());
                details.insert("omitted_columns", omitted.to_string());
                details.insert("listed_columns", ARC_017_MAX_PER_FILE.to_string());
                notices.push(notice);
            }
        }

        let header_count = headers.len();
        let req_flds = required_fields(&raw_name);
        // stop_times.stop_id is conditionally required (spec): "Required if stop_times.location_group_id
        // AND stop_times.location_id are NOT defined." If either appears in the header, the file
        // uses Flex and stop_id is not expected → do not produce ARC_025.
        let stop_id_required = raw_name == "stop_times.txt"
            && !headers
                .iter()
                .any(|h| h == "location_group_id" || h == "location_id");
        let extra_req: &[&str] = if stop_id_required { &["stop_id"] } else { &[] };

        // ARC_025: required column present in req_flds but ABSENT from the header (header-level,
        // once per file). Equivalent to MD's missing_required_column. An empty value (column
        // present) belongs to the relevant K2 group rule.
        for &f in req_flds.iter().chain(extra_req) {
            if !headers.iter().any(|h| h == f) {
                notices.push(make_notice(
                    &mut counter,
                    "ARC_025",
                    EntityType::File,
                    Some(raw_name.clone()),
                    Some(&raw_name),
                    Some(1),
                    Some(f),
                    Some(String::new()),
                    format!("'{raw_name}' dosyasında zorunlu '{f}' sütunu başlıkta yok."),
                    "Add the required column to the header.",
                ));
            }
        }

        // DQ_016: the primary-key column index is loop-invariant — compute it once outside the loop.
        let dq016_pk_idx = headers.iter().position(|h| h.ends_with("_id"));
        let _ = dq016_pk_idx; // file-level summary → no row identity is needed
        let mut dq016 = Dq016Acc::default();
        let mut arc032 = Arc032Acc::default();
        let mut arc037 = Arc037Acc::default();
        let mut rows: Vec<Vec<SmolStr>> = Vec::new();
        let mut arc021_fired = false;
        let mut arc030_fired = false;
        for (row_idx, row) in records.into_iter().enumerate() {
            let line_num = (row_idx + 2) as u64;

            // ARC_012: inconsistent column count.
            // row < header: trailing empty optional fields omitted — valid CSV practice → INFO.
            // row > header: extra fields — unescaped comma or format error → CRITICAL.
            if let Some((msg, tip, observed, is_info)) =
                arc012_check(&raw_name, line_num, row.len(), header_count)
            {
                let mut n = make_notice(
                    &mut counter,
                    "ARC_012",
                    EntityType::File,
                    Some(raw_name.clone()),
                    Some(&raw_name),
                    Some(line_num),
                    None,
                    Some(observed),
                    msg,
                    tip,
                );
                if is_info {
                    n.severity = Severity::Bilgi;
                }
                notices.push(n);
            }

            // ARC_018: empty data row (all fields empty).
            if row.iter().all(|v| v.trim().is_empty()) {
                notices.push(make_notice(
                    &mut counter,
                    "ARC_018",
                    EntityType::File,
                    Some(raw_name.clone()),
                    Some(&raw_name),
                    Some(line_num),
                    None,
                    None,
                    format!("'{raw_name}' {line_num}. satırı tamamen boş."),
                    "Remove empty rows from the file.",
                ));
                continue; // Do not store the empty row.
            }

            // ARC_034: HEADER ROW REPEATED AS A DATA ROW.
            //
            // If the header is written again in the middle of a file (usually because two files
            // were concatenated), the parser treats it as data and every field takes its own
            // column name as the "value". One structural defect then spreads into dozens of
            // field errors: in `mdb-1003`/`mdb-1004`, a phantom agency's `agency_timezone` becomes
            // the literal "agency_timezone" and produces `AGN_004` + `AGN_005`; in `mdb-1004`,
            // the same row in `trips.txt` triggers `TRP_006`. None tells the user the root problem.
            //
            // This follows the precedent set by `ARC_018` (an entirely empty row = ONE error,
            // not thirteen missing fields).
            //
            // ⚠️ FALSE-POSITIVE GATE: EVERY field in the row must equal its own column name. It is
            // legitimate for one column's `route_id` value to be "route_id" and it must not fire;
            // all fields matching cannot arise in another legitimate case.
            if arc034_is_header_repeat(&row[..], &headers[..]) {
                notices.push(make_notice(
                    &mut counter, "ARC_034",
                    EntityType::File, Some(raw_name.clone()),
                    Some(&raw_name), Some(line_num), None,
                    None,
                    format!("'{raw_name}' {line_num}. satırı başlık satırının tekrarı — veri olarak okunuyor."),
                    "Remove the repeated header row in the middle of the file; it usually comes from two files being concatenated.",
                ));
                continue; // Do not STORE the repeated header as data.
            }

            // ARC_021: unprintable or problematic character — excluding valid Unicode text (Japanese, etc.).
            if !arc021_fired {
                if let Some(cp) = arc021_bad_char(row.iter().map(|v| v.as_str())) {
                    arc021_fired = true;
                    notices.push(make_notice(
                        &mut counter,
                        "ARC_021",
                        EntityType::File,
                        Some(raw_name.clone()),
                        Some(&raw_name),
                        Some(line_num),
                        None,
                        Some(format!("U+{cp:04X}")),
                        arc021_message(&raw_name, cp),
                        ARC021_REMEDIATION,
                    ));
                }
            }

            // ARC_030: tab/line ending in a field value — the spec explicitly forbids these.
            if !arc030_fired {
                if let Some(cp) = arc030_bad_whitespace(row.iter().map(|v| v.as_str())) {
                    arc030_fired = true;
                    notices.push(make_notice(
                        &mut counter,
                        "ARC_030",
                        EntityType::File,
                        Some(raw_name.clone()),
                        Some(&raw_name),
                        Some(line_num),
                        None,
                        Some(format!("U+{cp:04X}")),
                        arc030_message(&raw_name, cp),
                        ARC030_REMEDIATION,
                    ));
                }
            }

            // DQ_016: extra whitespace in values — field and row level.
            // DQ_016: file-level accumulation; ONE summary after the emit loop.
            dq016.observe(line_num, row.iter().map(|v| v.as_str()), &headers);

            // ARC_032: HTML tag/escape sequence — the same accumulation pattern.
            arc032.observe(line_num, row.iter().map(|v| v.as_str()), &headers);
            arc037.observe(line_num, row.iter().map(|v| v.as_str()), &headers);

            rows.push(row);
        }

        // DQ_016: ONE summary per FILE (NOT per row — burst prevention, STM_050 pattern).
        if let Some((observed, msg, cols)) = dq016.summary(&raw_name) {
            let mut n = make_notice(
                &mut counter,
                "DQ_016",
                EntityType::File,
                Some(raw_name.clone()),
                Some(raw_name.as_str()),
                dq016.first_line,
                Some(cols.as_str()),
                Some(observed),
                msg,
                DQ016_REMEDIATION,
            );
            n.details = dq016.evidence_details().map(Into::into);
            notices.push(n);
        }

        // ARC_032: ONE summary per FILE (same burst prevention as DQ_016).
        if let Some((observed, msg, cols)) = arc032.summary(&raw_name) {
            notices.push(make_notice(
                &mut counter,
                "ARC_032",
                EntityType::File,
                Some(raw_name.clone()),
                Some(raw_name.as_str()),
                arc032.first_line,
                Some(cols.as_str()),
                Some(observed),
                msg,
                ARC032_REMEDIATION,
            ));
        }

        // ARC_037: non-ASCII character in an ID field — ONE summary per FILE (ARC_032 pattern).
        if let Some((observed, msg, cols)) = arc037.summary(&raw_name) {
            notices.push(make_notice(
                &mut counter,
                "ARC_037",
                EntityType::File,
                Some(raw_name.clone()),
                Some(raw_name.as_str()),
                arc037.first_line,
                Some(cols.as_str()),
                Some(observed),
                msg,
                ARC037_REMEDIATION,
            ));
        }

        // ARC_033: RFC 4180 quote violation — ONE summary per FILE (ARC_032 pattern).
        // Emit here, NOT in K2: the decision is made by the tokenizer, and the parsed value seen
        // by K2 CANNOT SHOW the violation (see the `Rfc4180Acc` doc).
        if rfc4180.rows > 0 {
            let kind = rfc4180.kind.unwrap_or(RFC4180_BARE_QUOTE);
            let example = rfc4180.example.clone().unwrap_or_default();
            notices.push(make_notice(
                &mut counter,
                "ARC_033",
                EntityType::File,
                Some(raw_name.clone()),
                Some(raw_name.as_str()),
                rfc4180.first_line,
                None,
                Some(format!(
                    "rows={}; issue={}",
                    rfc4180.rows,
                    rfc4180_issue_code(kind)
                )),
                format!(
                    "'{raw_name}' RFC 4180'e uymuyor: {kind} ({} satırda; ilk örnek: '{example}').",
                    rfc4180.rows
                ),
                ARC033_REMEDIATION,
            ));
        }

        // ARC_022: file row-count information threshold (K2 produces it in stream_mode — rows is
        // empty here). Non-stream files are tokenized without a limit (#185), and `rows.len()` is
        // the real count.
        let max_rows = cfg.max_file_rows as usize;
        if !stream_mode && rows.len() > max_rows {
            notices.push(make_notice(
                &mut counter,
                "ARC_022",
                EntityType::File,
                Some(raw_name.clone()),
                Some(&raw_name),
                None,
                None,
                Some(format!("{}", rows.len())),
                format!(
                    "'{raw_name}' dosyasında {} satır var; {} satırlık bilgi eşiğini aşıyor. Dosyanın tamamı doğrulandı.",
                    rows.len(),
                    max_rows
                ),
                "Informational; no fix required. Very large files can increase processing time and memory use for consumers.",
            ));
        }

        // ARC_009 check for a header-only record (header exists, no data).
        // rows is empty in stream_mode; K2 produces it for stop_times/shapes (total_rows exists).
        // Check calendar_dates.txt stream_mode here too (K2 has no total_rows).
        let stream_empty = stream_mode
            && raw_name == "calendar_dates.txt"
            && text.lines().filter(|l| !l.trim().is_empty()).count() <= 1;
        if (!stream_mode && rows.is_empty() || stream_empty)
            && !(raw_name == "calendar_dates.txt" && has_calendar_txt)
            && !(raw_name == "calendar.txt" && has_calendar_dates_txt)
        {
            empty_files.insert(raw_name.clone());
            // Required file → ARC_035 (at traversal end, with the Flex exemption).
            if !REQUIRED_FILES.contains(&raw_name.as_str()) {
                notices.push(make_notice(
                    &mut counter,
                    "ARC_009",
                    EntityType::File,
                    Some(raw_name.clone()),
                    Some(&raw_name),
                    None,
                    None,
                    None,
                    format!("'{raw_name}' dosyasında başlık satırı var ama veri satırı yok."),
                    "Add at least one data row to the file (or remove the file if it is not needed).",
                ));
            }
        }

        // In zip_stream partial_read, bytes = ~8 KB snippet; the real size is in ZIP metadata.
        // NLL: the borrow of bytes ends here (last use), so raw_vec can move afterward.
        let bytes_len = if is_zip_stream {
            uncompressed_hint as u32
        } else {
            bytes.len() as u32
        };
        // For zip_stream files (stop_times, trips, calendar_dates): raw_text=None.
        // K2 receives zip_bytes, reopens the ZIP, and streams the body.
        // shapes.txt: stream through raw_text (not zip_stream, raw_text=Some).
        let raw_text = if is_zip_stream {
            None
        } else if stream_mode {
            // shapes.txt — is_required=false, so the lossy path is acceptable.
            Some(text.to_string())
        } else {
            None
        };

        k1dbg!("[K1] bitti: {raw_name} ({} satır)", rows.len());
        let zip_entry_name = (raw_name != entry_name).then(|| entry_name.clone());
        raw_files.insert(
            raw_name.clone(),
            RawFile {
                name: raw_name,
                headers,
                rows,
                bytes: bytes_len,
                raw_text,
                zip_entry_name,
            },
        );
    }

    // ── File presence checks ──────────────────────────────────────────────────

    // ARC_004: required file missing → Fatal.
    //
    // `stops.txt` is CONDITIONALLY required. Spec (Schedule Reference, file table):
    //   "stops.txt — Conditionally Required — Optional if demand-responsive zones are
    //    defined in locations.geojson. Required otherwise."
    // A Flex-only feed defines service through `locations.geojson` regions and may carry no
    // stops.txt. Keeping it in the unconditional list made a VALID feed completely unreadable
    // with ARC_004 — the user could not see a single finding. If both are absent, service has no
    // defined location → PARTIAL.
    let missing: Vec<&str> = REQUIRED_FILES
        .iter()
        .filter(|&&f| !present_files.contains(f))
        .filter(|&&f| !(f == "stops.txt" && has_valid_locations_geojson))
        .copied()
        .collect();

    if !missing.is_empty() {
        for &f in &missing {
            partial.mark_unavailable(f);
            notices.push(make_notice(
                &mut counter,
                "ARC_004",
                // We have the file name; leaving this empty produced the en/ja/fr message
                // "Required file '' is missing.".
                EntityType::Feed,
                Some(f.to_string()),
                // One finding per missing file: the registry's `File` dedup distinguishes by this field.
                Some(f),
                None,
                None,
                Some(f.to_string()),
                format!("Zorunlu GTFS dosyası eksik: '{f}'."),
                "Eksik dosyayı feed ZIP arşivine ekleyin.",
            ));
        }
    }

    // ARC_035: required file EXISTS but has no data rows.
    // Same axis and exemption as ARC_004 — if the "file exists" check does not inspect content,
    // a zero-byte `stop_times.txt` passes the publication gate (in corpus run 13, five feeds with
    // empty stop_times received a publication score of 100.0).
    let empty_required: Vec<&str> = REQUIRED_FILES
        .iter()
        .filter(|&&f| empty_files.contains(f))
        .filter(|&&f| !(f == "stops.txt" && has_valid_locations_geojson))
        .copied()
        .collect();
    for &f in &empty_required {
        notices.push(make_notice(
            &mut counter,
            "ARC_035",
            EntityType::File,
            Some(f.to_string()),
            Some(f),
            None,
            None,
            Some(f.to_string()),
            format!("Zorunlu GTFS dosyası '{f}' hiç veri satırı içermiyor."),
            "Dosyaya gerçek verileri ekleyin; boş bırakmak dosyayı eklememekle aynıdır.",
        ));
    }

    // ARC_036: ZIP stream view contradicts the central directory. ONE feed-level finding —
    // emitting per entry repeats the publisher's one mistake once per entry.
    if inconsistent_stream_entries > 0 {
        notices.push(make_notice(
            &mut counter, "ARC_036",
            EntityType::Feed, None,
            None, None, None,
            Some(inconsistent_stream_entries.to_string()),
            format!(
                "ZIP arşivinde {inconsistent_stream_entries} girdinin akış başlığı merkez diziniyle çelişiyor; \
                 arşivi akıştan okuyan araçlar dosyayı açamaz."
            ),
            "Repack the archive with a standard ZIP tool; the data-descriptor flag and the sizes in the local header must be consistent.",
        ));
    }

    // ARC_008: both calendar.txt AND calendar_dates.txt are missing (Critical, non-Fatal).
    let has_any_calendar = CALENDAR_FILES.iter().any(|&f| present_files.contains(f));
    if !has_any_calendar {
        notices.push(make_notice(
            &mut counter,
            "ARC_008",
            EntityType::Feed,
            None,
            None,
            None,
            None,
            None,
            "calendar.txt ve calendar_dates.txt dosyalarının ikisi de eksik.".to_string(),
            "Include at least one of calendar.txt or calendar_dates.txt in the ZIP.",
        ));
    }

    // ARC_031: spec file-level requirement — feed_info.txt is REQUIRED when translations.txt exists.
    // When the condition is not met, the file is only RECOMMENDED; that case is ARC_020 (Quality).
    if present_files.contains("translations.txt") && !present_files.contains("feed_info.txt") {
        notices.push(make_notice(
            &mut counter, "ARC_031",
            EntityType::Feed, None,
            None, None, None,
            None,
            "translations.txt var ama feed_info.txt eksik; spec bu durumda feed_info.txt'i zorunlu kılar.".to_string(),
            "Add feed_info.txt and fill in feed_lang — it says which language the translations are relative to.",
        ));
    }

    Ok(K1Result {
        files: raw_files,
        notices,
        partial,
        present_files,
        has_valid_locations_geojson,
        geojson_location_ids,
        valid_geojson_location_ids,
        geojson_geometries,
        custom_file_headers,
    })
}

// ── locations.geojson validation ─────────────────────────────────────────────

fn validate_locations_geojson(
    bytes: &[u8],
    fname: &str,
    notices: &mut Vec<Notice>,
    counter: &mut u32,
    geojson_ids: &mut std::collections::HashSet<String>,
    valid_geojson_ids: &mut std::collections::HashSet<String>,
    geoms: &mut std::collections::HashMap<String, LocationGeometry>,
) -> bool {
    let text = match std::str::from_utf8(bytes) {
        Ok(s) => s,
        Err(_) => {
            notices.push(make_notice(
                counter,
                "ARC_002",
                EntityType::File,
                Some(fname.to_string()),
                Some(fname),
                None,
                None,
                None,
                format!("'{fname}' UTF-8 kodlamasıyla okunamıyor."),
                "Re-save the file with UTF-8 encoding.",
            ));
            return false;
        }
    };

    let json: serde_json::Value =
        match serde_json::from_str(text) {
            Ok(v) => v,
            Err(e) => {
                notices.push(make_notice(
                counter, "LOC_012", EntityType::File, Some(fname.to_string()),
                Some(fname), None, None, Some(e.to_string()),
                format!("'{fname}' geçerli bir GeoJSON belgesi değil: {e}"),
                "Make sure locations.geojson is a valid GeoJSON FeatureCollection.",
            ));
                return false;
            }
        };

    let root_type = json.get("type").and_then(|t| t.as_str());
    if root_type != Some("FeatureCollection") {
        notices.push(make_notice(
            counter,
            "LOC_012",
            EntityType::File,
            Some(fname.to_string()),
            Some(fname),
            None,
            Some("type"),
            root_type.map(str::to_string),
            format!(
                "'{fname}' kök tipi 'FeatureCollection' olmalıdır; bulundu: '{}'.",
                root_type.unwrap_or("yok")
            ),
            "The root type of locations.geojson must be 'FeatureCollection'.",
        ));
        return false;
    }

    let Some(features) = json.get("features").and_then(|f| f.as_array()) else {
        notices.push(make_notice(
            counter,
            "LOC_012",
            EntityType::File,
            Some(fname.to_string()),
            Some(fname),
            None,
            Some("features"),
            None,
            format!("'{fname}' için 'features' dizisi eksik veya geçersiz."),
            "Add a valid 'features' array to locations.geojson.",
        ));
        return false;
    };

    // LOC_005: empty FeatureCollection.
    if features.is_empty() {
        notices.push(make_notice(
            counter,
            "LOC_005",
            EntityType::File,
            Some(fname.to_string()),
            Some(fname),
            None,
            Some("features"),
            None,
            format!("'{fname}' FeatureCollection'ı boş — hiç feature yok."),
            "Add at least one Polygon or MultiPolygon feature to locations.geojson.",
        ));
        return false;
    }

    // LOC_007: duplicate feature 'id' values. The counter also ensures that the Flex exemption
    // relies only on genuinely unique string IDs.
    let mut id_counts: std::collections::HashMap<String, usize> = std::collections::HashMap::new();
    {
        let mut seen_ids: std::collections::HashMap<String, usize> =
            std::collections::HashMap::new();
        for (i, feature) in features.iter().enumerate() {
            if let Some(id_val) = feature.get("id") {
                let id_str = id_val.to_string();
                // XFL_025: unquoted raw ID so it matches the raw CSV value of stop_times.location_id.
                geojson_ids.insert(
                    id_val
                        .as_str()
                        .map(str::to_string)
                        .unwrap_or_else(|| id_str.clone()),
                );
                if let Some(id) = id_val.as_str() {
                    *id_counts.entry(id.to_string()).or_insert(0) += 1;
                }
                if let Some(first_idx) = seen_ids.get(&id_str) {
                    notices.push(make_notice(
                        counter, "LOC_007", EntityType::File, Some(fname.to_string()),
                        Some(fname), Some((i + 1) as u64), Some("id"), Some(id_str.clone()),
                        format!("'{fname}' özellik {} ve {} aynı 'id' değerine sahip ({id_str}) — GTFS Flex referansı belirsizleşir.", first_idx + 1, i + 1),
                        "Assign a unique \"id\" to each feature in locations.geojson.",
                    ));
                } else {
                    seen_ids.insert(id_str, i);
                }
            }
        }
    }

    for (i, feature) in features.iter().enumerate() {
        let feat_num = (i + 1) as u64;
        let valid_type = feature.get("type").and_then(|v| v.as_str()) == Some("Feature");
        let valid_properties = feature.get("properties").is_some_and(|v| v.is_object());
        let valid_id = feature
            .get("id")
            .and_then(|v| v.as_str())
            .filter(|id| !id.is_empty() && id_counts.get(*id) == Some(&1));

        // LOC_008: every Feature's "type" field is Required and must be "Feature" (spec table).
        match feature.get("type").and_then(|v| v.as_str()) {
            Some("Feature") => {}
            other => {
                notices.push(make_notice(
                    counter,
                    "LOC_008",
                    EntityType::File,
                    Some(fname.to_string()),
                    Some(fname),
                    Some(feat_num),
                    Some("type"),
                    other.map(str::to_string),
                    format!(
                        "'{fname}' özellik {feat_num} için 'type' eksik veya \"Feature\" değil."
                    ),
                    "Add \"type\": \"Feature\" to every feature object.",
                ));
            }
        }

        // LOC_009: the "properties" object is Required (spec table). An empty object is VALID —
        // stop_name/stop_desc are optional; missing or non-object values are violations.
        if !feature.get("properties").is_some_and(|v| v.is_object()) {
            notices.push(make_notice(
                counter,
                "LOC_009",
                EntityType::File,
                Some(fname.to_string()),
                Some(fname),
                Some(feat_num),
                Some("properties"),
                None,
                format!("'{fname}' özellik {feat_num} için 'properties' nesnesi eksik."),
                "Add a 'properties' object to every feature; an empty object is valid.",
            ));
        }

        // LOC_003: 'id' property missing from Feature.
        if feature.get("id").is_none() {
            notices.push(make_notice(
                counter, "LOC_003", EntityType::File, Some(fname.to_string()),
                Some(fname), Some(feat_num), Some("id"), None,
                format!("'{fname}' özellik {feat_num} için 'id' property eksik — stop_times çapraz referansı için zorunlu."),
                "Add a unique \"id\" property to each feature that matches the location_id referenced in stop_times.txt.",
            ));
        }

        // Pa1fdaa0d: raw ID used to bind geometry to the feature's 'id' (the SAME normalization
        // as XFL_025 — it must match the raw CSV value of `stop_times.location_id`).
        let feature_id: Option<String> = feature.get("id").map(|v| {
            v.as_str()
                .map(str::to_string)
                .unwrap_or_else(|| v.to_string())
        });
        let mut geom = LocationGeometry {
            outer_rings: Vec::new(),
            bbox: (f64::MAX, f64::MAX, f64::MIN, f64::MIN),
        };
        let geometry = feature.get("geometry");

        // LOC_002: geometry null or missing from Feature.
        if geometry.is_none_or(|g| g.is_null()) {
            notices.push(make_notice(
                counter, "LOC_002", EntityType::File, Some(fname.to_string()),
                Some(fname), Some(feat_num), Some("geometry"), None,
                format!("'{fname}' özellik {feat_num} için geometry null veya eksik — GTFS Flex gerektiriyor."),
                "Add a valid Polygon or MultiPolygon geometry to each feature in locations.geojson.",
            ));
            continue;
        }
        let geometry = geometry.unwrap();

        let geom_type = geometry.get("type").and_then(|t| t.as_str());
        let valid_geometry = match geom_type {
            Some("Polygon") => {
                // LOC_010: "coordinates" is Required. A missing/non-array value used to be
                // silently skipped by `if let Some(..)` → geometry passed without validation.
                match geometry.get("coordinates").and_then(|c| c.as_array()) {
                    Some(rings) => check_polygon_rings(
                        counter,
                        fname,
                        feat_num,
                        rings,
                        notices,
                        Some(&mut geom),
                    ),
                    None => {
                        missing_coordinates(counter, fname, feat_num, notices);
                        false
                    }
                }
            }
            Some("MultiPolygon") => match geometry.get("coordinates").and_then(|c| c.as_array()) {
                None => {
                    missing_coordinates(counter, fname, feat_num, notices);
                    false
                }
                Some(polygons) => {
                    let mut valid = !polygons.is_empty();
                    for poly in polygons {
                        if let Some(rings) = poly.as_array() {
                            valid &= check_polygon_rings(
                                counter,
                                fname,
                                feat_num,
                                rings,
                                notices,
                                Some(&mut geom),
                            );
                        } else {
                            valid = false;
                        }
                    }
                    valid
                }
            },
            Some(t) => {
                notices.push(make_notice(
                    counter, "LOC_001", EntityType::File, Some(fname.to_string()),
                    Some(fname), Some(feat_num), Some("geometry.type"), Some(t.to_string()),
                    format!("'{fname}' özellik {feat_num} geçersiz geometri tipi: '{t}'. GTFS Flex yalnızca Polygon ve MultiPolygon destekler."),
                    "Use only Polygon and MultiPolygon geometries in locations.geojson.",
                ));
                false
            }
            None => {
                notices.push(make_notice(
                    counter,
                    "LOC_001",
                    EntityType::File,
                    Some(fname.to_string()),
                    Some(fname),
                    Some(feat_num),
                    Some("geometry.type"),
                    None,
                    format!("'{fname}' özellik {feat_num} için geometri tipi eksik."),
                    "Define a Polygon or MultiPolygon geometry for every feature.",
                ));
                false
            }
        };

        // Pa1fdaa0d: geometry is moved to K6 keyed by the feature's 'id'.
        if let Some(id) = feature_id {
            if !geom.outer_rings.is_empty() {
                geoms.insert(id.clone(), geom);
            }
            if valid_type && valid_properties && valid_geometry && valid_id == Some(id.as_str()) {
                valid_geojson_ids.insert(id);
            }
        }
    }

    !valid_geojson_ids.is_empty()
}

/// Checks LOC_004 (not closed) and LOC_006 (area > 500 km²) for polygon rings.
/// LOC_010: geometry.coordinates missing or not an array. The spec marks this field Required;
/// without it, the region geometry cannot be resolved at all, so the finding is CRITICAL.
fn missing_coordinates(counter: &mut u32, fname: &str, feat_num: u64, notices: &mut Vec<Notice>) {
    notices.push(make_notice(
        counter,
        "LOC_010",
        EntityType::File,
        Some(fname.to_string()),
        Some(fname),
        Some(feat_num),
        Some("coordinates"),
        None,
        format!("'{fname}' özellik {feat_num} için geometry 'coordinates' eksik veya dizi değil."),
        "Add a valid 'coordinates' array to the geometry.",
    ));
}

fn check_polygon_rings(
    counter: &mut u32,
    fname: &str,
    feat_num: u64,
    rings: &[serde_json::Value],
    notices: &mut Vec<Notice>,
    // Pa1fdaa0d: move the outer ring to K6. With `None`, do not collect geometry
    // (validation only) — for tests and legacy call paths.
    collect: Option<&mut LocationGeometry>,
) -> bool {
    let mut all_lats = Vec::new();
    let mut all_lons = Vec::new();
    let mut already_reported_closure = false;
    // Signed area per ring for LOC_006: the first ring is the outer boundary; the rest are holes.
    let mut all_rings: Vec<Vec<(f64, f64)>> = Vec::new();
    let mut ring_areas: Vec<f64> = Vec::new();
    let mut structurally_valid = !rings.is_empty();

    for ring in rings {
        let Some(pts) = ring.as_array() else {
            structurally_valid = false;
            continue;
        };
        // RFC 7946 LinearRing: at least four positions, with the first and last equal.
        if pts.len() < 4 {
            structurally_valid = false;
            continue;
        }

        // LOC_004: the ring is open when the first and last points differ.
        if !already_reported_closure {
            let first = &pts[0];
            let last = &pts[pts.len() - 1];
            let first_lon = first.get(0).and_then(|v| v.as_f64());
            let first_lat = first.get(1).and_then(|v| v.as_f64());
            let last_lon = last.get(0).and_then(|v| v.as_f64());
            let last_lat = last.get(1).and_then(|v| v.as_f64());
            if let (Some(fl), Some(fa), Some(ll), Some(la)) =
                (first_lon, first_lat, last_lon, last_lat)
            {
                if (fl - ll).abs() > 1e-8 || (fa - la).abs() > 1e-8 {
                    notices.push(make_notice(
                        counter, "LOC_004", EntityType::File, Some(fname.to_string()),
                        Some(fname), Some(feat_num), Some("coordinates"), None,
                        format!("'{fname}' özellik {feat_num} Polygon ring'i kapalı değil — ilk nokta [{fl},{fa}] son nokta [{ll},{la}] ile eşleşmiyor."),
                        "Close the Polygon ring by making the last coordinate equal to the first coordinate.",
                    ));
                    already_reported_closure = true;
                    structurally_valid = false;
                }
            } else {
                structurally_valid = false;
            }
        }

        // Collect ring coordinates for area calculation and latitude scaling.
        let mut ring_pts: Vec<(f64, f64)> = Vec::with_capacity(pts.len());
        for pt in pts {
            if let (Some(lon), Some(lat)) = (
                pt.get(0).and_then(|v| v.as_f64()),
                pt.get(1).and_then(|v| v.as_f64()),
            ) {
                all_lats.push(lat);
                all_lons.push(lon);
                ring_pts.push((lon, lat));
            } else {
                structurally_valid = false;
            }
        }
        ring_areas.push(signed_ring_area_km2(&ring_pts));
        all_rings.push(ring_pts);
    }

    // LOC_011: OpenGIS 6.1.11 — rings must be SIMPLE and holes must remain INSIDE the outer ring.
    // LOC_004 checks closure only; these are the actual 6.1.11 requirements.
    if let Some(outer) = all_rings.first() {
        // ── OpenGIS SFS 6.1.11 polygon-validity requirements ────────────────────
        // (1) rings are simple · (3) no two rings CROSS (they may touch at one point)
        // (4) no cut lines/spikes/punctures · (5) connected interior · holes INSIDE the shell.
        //
        // ⚠️ RING ORIENTATION IS NOT IN THIS LIST. The 2026-08-06 external audit also counted
        // "ring orientation" among the items `LOC_011` did not measure; **6.1.11, which the
        // spec cites, does not require orientation** — the clockwise rule belongs to the
        // SERIALIZATION rule in GeoJSON RFC 7946 §3.1.6, and the RFC also says parsers should
        // not reject non-conforming geometry. Treating orientation as a violation would repeat
        // the `PTH_017` trap → IT IS DELIBERATELY NOT MEASURED.
        let mut bad: Option<Loc011Failure> = None;
        if !ring_is_simple(outer) {
            bad = Some(Loc011Failure::new(
                "issue=outer_ring_self_intersection",
                "dış ring kendini kesiyor",
            ));
        } else if has_spike(outer) {
            bad = Some(Loc011Failure::new(
                "issue=outer_ring_zero_area_spike",
                "zero-area spike (spike/cut line) on the outer ring",
            ));
        } else if ring_touches_itself(outer) {
            bad = Some(Loc011Failure::new(
                "issue=outer_ring_self_touch",
                "dış ring kendine dokunuyor — iç kısım bölünüyor",
            ));
        }
        for (i, hole) in all_rings.iter().enumerate().skip(1) {
            if bad.is_some() {
                break;
            }
            if !ring_is_simple(hole) {
                bad = Some(Loc011Failure::new(
                    format!("issue=hole_self_intersection; ring={i}"),
                    format!("{i}. delik kendini kesiyor"),
                ));
            } else if has_spike(hole) {
                bad = Some(Loc011Failure::new(
                    format!("issue=hole_zero_area_spike; ring={i}"),
                    format!("zero-area spike (spike/cut line) in hole {i}"),
                ));
            } else if ring_touches_itself(hole) {
                bad = Some(Loc011Failure::new(
                    format!("issue=hole_self_touch; ring={i}"),
                    format!("{i}. delik kendine dokunuyor — iç kısım bölünüyor"),
                ));
            } else if rings_cross(outer, hole) {
                // 6.1.11 (3): rings must not cross. This was not measured before.
                bad = Some(Loc011Failure::new(
                    format!("issue=hole_crosses_outer_ring; ring={i}"),
                    format!("{i}. delik dış ring'i kesiyor"),
                ));
            } else if hole.iter().any(|&p| !point_in_ring(p, outer)) {
                // Previously only the FIRST point was sampled → a hole partly outside the ring
                // went unnoticed. Now ALL vertices are checked.
                bad = Some(Loc011Failure::new(
                    format!("issue=hole_outside_outer_ring; ring={i}"),
                    format!("hole {i} lies partly or wholly outside the outer ring"),
                ));
            }
        }
        // Hole-hole relationship: crossing is forbidden (6.1.11/3); nested holes are also
        // invalid (the inner "hole" would actually be filled area, making the interior undefined).
        'outer: for i in 1..all_rings.len() {
            if bad.is_some() {
                break;
            }
            for j in (i + 1)..all_rings.len() {
                if rings_cross(&all_rings[i], &all_rings[j]) {
                    bad = Some(Loc011Failure::new(
                        format!("issue=holes_cross; rings={i},{j}"),
                        format!("{i}. ve {j}. delikler birbirini kesiyor"),
                    ));
                    break 'outer;
                }
                if all_rings[j]
                    .first()
                    .is_some_and(|&p| point_in_ring(p, &all_rings[i]))
                    || all_rings[i]
                        .first()
                        .is_some_and(|&p| point_in_ring(p, &all_rings[j]))
                {
                    bad = Some(Loc011Failure::new(
                        format!("issue=nested_holes; rings={i},{j}"),
                        format!("{i}. ve {j}. delikler iç içe"),
                    ));
                    break 'outer;
                }
            }
        }
        // 6.1.11 item 5 — the interior must be CONNECTED. ALL contacts between rings are
        // evaluated with a single cycle criterion; "two rings touch at two points" was only
        // one special case of that criterion (see `interior_disconnection`).
        if bad.is_none() {
            bad = interior_disconnection(&all_rings);
        }
        if let Some(failure) = bad {
            structurally_valid = false;
            notices.push(make_notice(
                counter, "LOC_011", EntityType::File, Some(fname.to_string()),
                Some(fname), Some(feat_num), Some("coordinates"), Some(failure.observed),
                format!("'{fname}' özellik {feat_num} poligonu OpenGIS Simple Features 6.1.11'e göre geçersiz: {}.", failure.explanation),
                "Fix the polygon rings so they do not self-intersect, and keep holes inside the outer ring.",
            ));
        }
    }

    // LOC_006: TRUE polygon area > 500 km²
    //
    // Until 2026-08-01 this measured the bounding-box area. Measurements on eight real
    // GTFS-Flex feeds showed that the bbox inflates the true area by 1.4×–5.6×, and 3 of
    // the rule's 7 firings (Columbia County ×2, Pueblo) came ONLY from that inflation — their
    // true areas were below the threshold. Polygons are not convex (service zones follow
    // administrative boundaries), so the bbox systematically skews upward. Shoelace measures
    // the true area; holes are subtracted.
    if all_lats.len() >= 3 && !ring_areas.is_empty() {
        let outer = ring_areas[0].abs();
        let holes: f64 = ring_areas[1..].iter().map(|a| a.abs()).sum();
        let area_km2 = (outer - holes).max(0.0);
        if area_km2 > 500.0 {
            notices.push(make_notice(
                counter, "LOC_006", EntityType::File, Some(fname.to_string()),
                Some(fname), Some(feat_num), Some("coordinates"), Some(format!("{area_km2:.0}km²")),
                format!("'{fname}' özellik {feat_num} Polygon alanı çok büyük (~{area_km2:.0}km²) — GTFS Flex bölgesi için gerçekçi değil."),
                "Reduce the polygon extent to a realistic service zone; large polygons are likely coordinate errors.",
            ));
        }
    }

    // Pa1fdaa0d: accumulate the OUTER ring. Holes are not retained — the outer boundary is
    // sufficient for the overlap decision, and ignoring holes would make the decision merely
    // "more inclusive"; because this is a PROHIBITION, that direction is NOT SAFE, so K6 also
    // verifies the decision by intersection/containment.
    if let (Some(g), Some(outer)) = (collect, all_rings.first()) {
        for &(x, y) in outer {
            g.bbox.0 = g.bbox.0.min(x);
            g.bbox.1 = g.bbox.1.min(y);
            g.bbox.2 = g.bbox.2.max(x);
            g.bbox.3 = g.bbox.3.max(y);
        }
        g.outer_rings.push(outer.clone());
    }
    structurally_valid && all_rings.first().is_some_and(|ring| ring.len() >= 4)
}

/// Do two line segments CROSS (excluding shared endpoints and tangency)?
///
/// ⚠️ `robust::orient2d` is REQUIRED. A 2026-08-06 measurement found that the naive `f64`
/// determinant varied from 5↔17 on the same 21 polygons depending on epsilon, and was not even
/// monotonic.
///
/// ⚠️ All four orientations must be NONZERO, so **collinear overlap does NOT COUNT as a CROSS**.
/// This is deliberate: OpenGIS 6.1.11 permits rings to be tangent at ONE POINT. The cost is that
/// "spike/cut line" violations caused by collinear overlap pass here → `has_spike` catches them
/// separately.
pub(crate) fn segments_cross(a: (f64, f64), b: (f64, f64), c: (f64, f64), d: (f64, f64)) -> bool {
    let o = |p: (f64, f64), q: (f64, f64), r: (f64, f64)| {
        let v = robust::orient2d(
            robust::Coord { x: p.0, y: p.1 },
            robust::Coord { x: q.0, y: q.1 },
            robust::Coord { x: r.0, y: r.1 },
        );
        if v > 0.0 {
            1i8
        } else if v < 0.0 {
            -1
        } else {
            0
        }
    };
    let (o1, o2, o3, o4) = (o(a, b, c), o(a, b, d), o(c, d, a), o(c, d, b));
    o1 != o2 && o3 != o4 && o1 != 0 && o2 != 0 && o3 != 0 && o4 != 0
}

/// Ring bounding box — cheap pre-filter before comparing ring pairs.
/// Avoid starting the O(n·m) intersection scan for pairs with disjoint boxes.
fn ring_bbox(pts: &[(f64, f64)]) -> (f64, f64, f64, f64) {
    let mut b = (f64::MAX, f64::MAX, f64::MIN, f64::MIN);
    for &(x, y) in pts {
        b.0 = b.0.min(x);
        b.1 = b.1.min(y);
        b.2 = b.2.max(x);
        b.3 = b.3.max(y);
    }
    b
}

fn bboxes_disjoint(a: (f64, f64, f64, f64), b: (f64, f64, f64, f64)) -> bool {
    a.2 < b.0 || b.2 < a.0 || a.3 < b.1 || b.3 < a.1
}

/// Do TWO RINGS CROSS (OpenGIS 6.1.11: *"No two Rings in the boundary cross"*)?
/// The 2026-08-06 audit showed this was not measured — `LOC_011` checked only each ring's OWN
/// simplicity and containment of the hole's FIRST POINT.
fn rings_cross(r1: &[(f64, f64)], r2: &[(f64, f64)]) -> bool {
    if r1.len() < 2 || r2.len() < 2 {
        return false;
    }
    if bboxes_disjoint(ring_bbox(r1), ring_bbox(r2)) {
        return false;
    }
    for i in 0..r1.len() - 1 {
        for j in 0..r2.len() - 1 {
            if segments_cross(r1[i], r1[i + 1], r2[j], r2[j + 1]) {
                return true;
            }
        }
    }
    false
}

/// At HOW MANY POINTS do two rings touch (number of contact points)?
///
/// OpenGIS SFS 6.1.11 item 3 permits rings to be tangent at **one point**; item 5 requires the
/// interior to be CONNECTED. The count does not decide by itself — [`interior_disconnection`]
/// applies the cycle criterion.
fn ring_contact_points(a: &[(f64, f64)], b: &[(f64, f64)]) -> usize {
    if bboxes_disjoint(ring_bbox(a), ring_bbox(b)) {
        return 0;
    }
    // There are two forms of contact and BOTH must count:
    //   (1) shared VERTEX — both rings write the same coordinate
    //   (2) a vertex of one ring lies on the EDGE of the other — no shared vertex but there is
    //       geometric contact. My first implementation counted only (1) and its own test failed:
    //       the hole touched the shell at (0,3)/(0,7), neither of which was a shell vertex.
    //       Counting only vertices would be a narrower check than claimed.
    //
    // ⚠️ Compare coordinates by EXACT equality; point-on-edge uses `robust::orient2d` with ZERO
    // orientation plus a range check. Epsilon tolerance would count nearby but distinct vertices
    // as touching and produce false positives.
    let touches = |p: (f64, f64), ring: &[(f64, f64)]| -> bool {
        if ring.contains(&p) {
            return true;
        }
        ring.windows(2).any(|w| point_on_segment(p, w[0], w[1]))
    };
    let mut seen: Vec<(f64, f64)> = Vec::new();
    for &p in a {
        if touches(p, b) && !seen.contains(&p) {
            seen.push(p);
        }
    }
    for &p in b {
        if touches(p, a) && !seen.contains(&p) {
            seen.push(p);
        }
    }
    seen.len()
}

/// Is point `p` ON segment `s`–`e`?
///
/// ⚠️ Decide using ZERO orientation from `robust::orient2d`, NOT epsilon tolerance: tolerance
/// counts nearby but DISTINCT vertices as touching and produces false positives.
fn point_on_segment(p: (f64, f64), s: (f64, f64), e: (f64, f64)) -> bool {
    if robust::orient2d(
        robust::Coord { x: s.0, y: s.1 },
        robust::Coord { x: e.0, y: e.1 },
        robust::Coord { x: p.0, y: p.1 },
    ) != 0.0
    {
        return false;
    }
    p.0 >= s.0.min(e.0) && p.0 <= s.0.max(e.0) && p.1 >= s.1.min(e.1) && p.1 <= s.1.max(e.1)
}

/// Does the ring touch ITSELF — a SELF-LOOP in the contact graph?
///
/// `ring_is_simple` checks only CROSSING and deliberately excludes tangency
/// (`segments_cross` requires all four orientations to be nonzero). Thus a figure-eight ring —
/// one that visits the same vertex twice or has a vertex on its own distant edge — passed through.
/// Such a ring's INTERIOR splits into two lobes, violating item 5 of 6.1.11.
///
/// ⚠️ ADJACENT indices are deliberately skipped: consecutive duplicate coordinates (zero-length
/// edges) and intermediate vertices on straight edges are common in real data and are NOT
/// self-contacts; counting them would produce false positives. Zero-area reversals are
/// `has_spike`'s job.
///
/// 🔴 MEASUREMENT LESSON (2026-08-07): the first version skipped only |i−j| ≤ 1 and produced
/// false positives in THREE of the 6 corpus feeds carrying `locations.geojson` — the same
/// coordinate was written THREE times consecutively (84 vertices in mdb-2045), so 189 and 191
/// were considered "not adjacent". The ring is now CLEANED of consecutive repeats first; adjacency
/// is decided on the cleaned sequence. The protection written by guess failed on real data.
fn ring_touches_itself(pts: &[(f64, f64)]) -> bool {
    // Remove consecutive duplicate vertices (cyclically, including the closing point).
    let mut pts: Vec<(f64, f64)> = {
        let body = &pts[..pts.len().saturating_sub(1)]; // closing copy
        let mut out: Vec<(f64, f64)> = Vec::with_capacity(body.len());
        for &p in body {
            if out.last() != Some(&p) {
                out.push(p);
            }
        }
        while out.len() > 1 && out.first() == out.last() {
            out.pop();
        }
        out
    };
    pts.push(*pts.first().unwrap_or(&(0.0, 0.0))); // close the ring again
    let pts = &pts[..];
    let n = pts.len().saturating_sub(1);
    if n < 4 {
        return false;
    }
    let adjacent = |a: usize, b: usize| {
        let d = (a + n - b) % n;
        d <= 1 || d >= n - 1
    };
    // (1) the same coordinate at two NON-ADJACENT vertices
    for i in 0..n {
        for j in (i + 1)..n {
            if !adjacent(i, j) && pts[i] == pts[j] {
                return true;
            }
        }
    }
    // (2) a vertex on a NON-ADJACENT edge (T-contact; no shared vertex)
    for i in 0..n {
        for j in 0..n {
            if adjacent(i, j) || adjacent(i, (j + 1) % n) {
                continue;
            }
            if point_on_segment(pts[i], pts[j], pts[(j + 1) % n]) {
                return true;
            }
        }
    }
    false
}

/// OpenGIS SFS 6.1.11 item 5 — *"the interior of every Polygon is a connected point set"*.
///
/// CRITERION: the multigraph whose nodes are rings and whose edges are CONTACT POINTS must be
/// ACYCLIC (a forest). One contact does not split the interior — it can be walked around; as soon
/// as contacts close a cycle, a closed curve forms and splits the interior into two components.
/// (JTS `ConnectedInteriorTester` uses the same criterion.)
///
/// 🔴 Until 2026-08-07 (issue #74), only the SPECIAL CASE was measured: "two rings touch at two
/// points" — two parallel edges in graph terms, the shortest cycle. The TRIANGLE cycle closed by
/// three rings touching pairwise at one point escaped; the card and ledger recorded it as a
/// remnant. Converting the criterion to cycles covered the special case automatically.
///
/// Self-loops (`ring_touches_itself`) are checked separately per ring by the caller.
struct Loc011Failure {
    observed: String,
    explanation: String,
}

impl Loc011Failure {
    fn new(observed: impl Into<String>, explanation: impl Into<String>) -> Self {
        Self {
            observed: observed.into(),
            explanation: explanation.into(),
        }
    }
}

fn interior_disconnection(rings: &[Vec<(f64, f64)>]) -> Option<Loc011Failure> {
    let label = |i: usize| {
        if i == 0 {
            "dış ring".to_string()
        } else {
            format!("{i}. delik")
        }
    };
    let mut parent: Vec<usize> = (0..rings.len()).collect();
    fn find(parent: &mut [usize], x: usize) -> usize {
        let mut root = x;
        while parent[root] != root {
            root = parent[root];
        }
        let mut cur = x;
        while parent[cur] != cur {
            let next = parent[cur];
            parent[cur] = root;
            cur = next;
        }
        root
    }
    for i in 0..rings.len() {
        for j in (i + 1)..rings.len() {
            let contacts = ring_contact_points(&rings[i], &rings[j]);
            if contacts == 0 {
                continue;
            }
            if contacts > 1 {
                return Some(Loc011Failure::new(
                    format!("issue=multiple_ring_contacts; rings={i},{j}; contacts={contacts}"),
                    format!(
                        "{} ile {} birden fazla noktada ({contacts}) dokunuyor — iç kısım bölünüyor",
                        label(i),
                        label(j)
                    ),
                ));
            }
            let (ri, rj) = (find(&mut parent, i), find(&mut parent, j));
            if ri == rj {
                return Some(Loc011Failure::new(
                    format!("issue=ring_contact_cycle; rings={i},{j}"),
                    format!(
                        "ring temasları çevrim oluşturuyor ({} ile {} teması zinciri kapatıyor) — iç kısım bölünüyor",
                        label(i), label(j)
                    ),
                ));
            }
            parent[ri] = rj;
        }
    }
    None
}

/// Does the ring contain a SPIKE / CUT LINE (OpenGIS 6.1.11: *"may not have cut lines, spikes or
/// punctures"*)? A spike means the ring follows a line and returns along the SAME line, forming
/// a zero-area protrusion. `segments_cross` cannot see this because orientations are zero for
/// collinear overlap and it does not count as a crossing. Here we look for a 180° turn in three
/// consecutive points.
fn has_spike(pts: &[(f64, f64)]) -> bool {
    let n = pts.len().saturating_sub(1);
    if n < 3 {
        return false;
    }
    for i in 0..n {
        let (a, b, c) = (pts[i], pts[(i + 1) % n], pts[(i + 2) % n]);
        let v = robust::orient2d(
            robust::Coord { x: a.0, y: a.1 },
            robust::Coord { x: b.0, y: b.1 },
            robust::Coord { x: c.0, y: c.1 },
        );
        if v != 0.0 {
            continue; // not collinear → cannot be a spike
        }
        // Collinear: a reversal exists when the vectors from b to a and b to c point in OPPOSITE
        // directions.
        let (ux, uy) = (a.0 - b.0, a.1 - b.1);
        let (vx, vy) = (c.0 - b.0, c.1 - b.1);
        if ux * vx + uy * vy > 0.0 {
            return true; // they point in the same direction → b has returned from one end
        }
    }
    false
}

/// Is the ring SIMPLE — that is, does it not cross itself (OpenGIS 6.1.11)?
///
/// ⚠️ `robust::orient2d` is REQUIRED, not the naive `f64` determinant. A 2026-08-06
/// measurement varied between 5 and 17 on the same 21 polygons depending on epsilon, and was
/// not even monotonic. The adaptive-precision predicate removes this instability.
///
/// Adjacent segments and the ring's closing pair are deliberately skipped: they SHARE endpoints
/// and do not count as crossings.
fn ring_is_simple(pts: &[(f64, f64)]) -> bool {
    let n = pts.len().saturating_sub(1); // last point = first point
    if n < 4 {
        return true;
    }
    for i in 0..n {
        for j in (i + 2)..n {
            if i == 0 && j == n - 1 {
                continue; // closing pair is adjacent
            }
            if segments_cross(pts[i], pts[i + 1], pts[j], pts[j + 1]) {
                return false;
            }
        }
    }
    true
}

/// Is a point inside the ring (ray casting)? Used for hole-containment checks.
pub(crate) fn point_in_ring(pt: (f64, f64), ring: &[(f64, f64)]) -> bool {
    let mut inside = false;
    let n = ring.len();
    if n < 3 {
        return false;
    }
    let mut j = n - 1;
    for i in 0..n {
        let (xi, yi) = ring[i];
        let (xj, yj) = ring[j];
        if (yi > pt.1) != (yj > pt.1) && pt.0 < (xj - xi) * (pt.1 - yi) / (yj - yi) + xi {
            inside = !inside;
        }
        j = i;
    }
    inside
}

/// Signed area (km²) of a GeoJSON ring, using shoelace on an equirectangular projection.
/// Because service regions are small, the ring's own mean latitude is sufficient for scaling;
/// the sign carries ring orientation, and the caller takes the absolute value.
fn signed_ring_area_km2(pts: &[(f64, f64)]) -> f64 {
    if pts.len() < 3 {
        return 0.0;
    }
    let lat0 = pts.iter().map(|p| p.1).sum::<f64>() / pts.len() as f64;
    let kx = 111.320 * lat0.to_radians().cos(); // 1° longitude → km
    let ky = 110.574; // 1° latitude → km
    let mut sum = 0.0;
    for i in 0..pts.len() {
        let (x1, y1) = (pts[i].0 * kx, pts[i].1 * ky);
        let (x2, y2) = (
            pts[(i + 1) % pts.len()].0 * kx,
            pts[(i + 1) % pts.len()].1 * ky,
        );
        sum += x1 * y2 - x2 * y1;
    }
    sum / 2.0
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    // After Notice creation moved to notice_factory, get_rule is used only here
    // (all_notices_have_valid_rule_ids).
    use gtfs_rules::get_rule;
    use std::io::Write;
    use zip::write::SimpleFileOptions;

    /// Tests do not concern themselves with the ARC_022 threshold; call with the DEFAULT config.
    /// This glob import shadows it, so the signature change in #71 did not touch 40+ calls.
    fn parse(zip_bytes: &[u8]) -> Result<K1Result, FatalError> {
        super::parse(zip_bytes, &ValidatorConfig::default())
    }

    fn minimal_gtfs_zip() -> Vec<u8> {
        let buf = std::io::Cursor::new(Vec::new());
        let mut zw = zip::ZipWriter::new(buf);
        let opts = SimpleFileOptions::default();

        let files: &[(&str, &str)] = &[
            ("agency.txt",     "agency_id,agency_name,agency_url,agency_timezone\n1,Test,http://x.com,UTC\n"),
            ("stops.txt",      "stop_id,stop_name,stop_lat,stop_lon\nS1,Stop1,41.0,29.0\n"),
            ("routes.txt",     "route_id,agency_id,route_short_name,route_type\nR1,1,101,3\n"),
            ("trips.txt",      "route_id,service_id,trip_id\nR1,SVC1,T1\n"),
            ("stop_times.txt", "trip_id,arrival_time,departure_time,stop_id,stop_sequence\nT1,08:00:00,08:00:00,S1,1\n"),
            ("calendar.txt",   "service_id,monday,tuesday,wednesday,thursday,friday,saturday,sunday,start_date,end_date\nSVC1,1,1,1,1,1,0,0,20240101,20241231\n"),
        ];
        for (name, content) in files {
            zw.start_file(*name, opts).unwrap();
            zw.write_all(content.as_bytes()).unwrap();
        }
        zw.finish().unwrap().into_inner()
    }

    /// The `zip` crate (2.4.2) also returns "Could not find EOCD" for a corrupt LOCAL HEADER;
    /// passing that message through made users look at the END of the archive. Measurement
    /// (`tld-5895`, the corpus's only `ZipUnreadable` feed): EOCD and central directory were
    /// intact, and the corrupt part was the local header of `stop_times.txt` (offset 570.618).
    /// Diagnosis must distinguish the LAYER of corruption.
    #[test]
    fn zip_diagnosis_points_at_the_broken_local_header() {
        let files: &[(&str, &[u8])] = &[
            ("agency.txt", b"agency_name\nA\n"),
            ("stops.txt", b"stop_id\nS1\n"),
        ];
        let mut bytes = zip_with_files(files);

        // No layer is blamed for a sound archive.
        let clean = diagnose_unreadable_zip(&bytes)
            .expect("sağlam arşivde de teşhis dönmeli")
            .text;
        assert!(
            clean.contains("tüm yerel başlıklar yerinde"),
            "sağlam arşiv yanlış katmanı suçluyor: {clean}"
        );

        // Corrupt the local-header signature of the second entry; leave the central directory
        // and EOCD untouched.
        let second = bytes
            .windows(4)
            .enumerate()
            .filter(|(_, w)| *w == LFH_SIGNATURE.to_le_bytes())
            .nth(1)
            .map(|(i, _)| i)
            .expect("ikinci yerel başlık bulunmalı");
        bytes[second] = 0;

        let diagnosis =
            diagnose_unreadable_zip(&bytes).expect("bozuk yerel başlık teşhis edilmeli");
        assert_eq!(diagnosis.kind, "local_header");
        let detail = diagnosis.text;
        assert!(
            detail.contains("yerel başlığı") && detail.contains("merkez dizin sağlam"),
            "yerel başlık bozukluğu doğru anlatılmalı: {detail}"
        );
    }

    /// If EOCD is genuinely absent, the library message is already correct; do not INVENT extra
    /// diagnosis.
    #[test]
    fn zip_diagnosis_stays_quiet_without_an_eocd() {
        let files: &[(&str, &[u8])] = &[("agency.txt", b"agency_name\nA\n")];
        let bytes = zip_with_files(files);
        let truncated = &bytes[..bytes.len() / 2];
        assert!(
            diagnose_unreadable_zip(truncated).is_none(),
            "EOCD yokken teşhis uydurulmamalı"
        );
    }

    fn zip_with_files(files: &[(&str, &[u8])]) -> Vec<u8> {
        let buf = std::io::Cursor::new(Vec::new());
        let mut zw = zip::ZipWriter::new(buf);
        let opts = SimpleFileOptions::default();
        for (name, content) in files {
            zw.start_file(*name, opts).unwrap();
            zw.write_all(content).unwrap();
        }
        zw.finish().unwrap().into_inner()
    }

    #[test]
    fn minimal_valid_feed_produces_no_error() {
        let zip = minimal_gtfs_zip();
        let result = parse(&zip);
        assert!(
            result.is_ok(),
            "Geçerli feed Ok dönmeli: {:?}",
            result.err()
        );
    }

    #[test]
    fn minimal_valid_feed_has_six_files() {
        let zip = minimal_gtfs_zip();
        let k1 = parse(&zip).unwrap();
        assert_eq!(k1.files.len(), 6);
        assert!(k1.files.contains_key("agency.txt"));
        assert!(k1.files.contains_key("stop_times.txt"));
    }

    #[test]
    fn prealloc_capacity_caps_declared_size() {
        // An enormous declaration is capped at MAX_PREALLOC (lying-header DoS protection).
        assert_eq!(prealloc_capacity(usize::MAX), MAX_PREALLOC);
        assert_eq!(prealloc_capacity(MAX_PREALLOC + 1), MAX_PREALLOC);
        // A zero declaration → at least 1 (avoid a pointless zero-capacity allocation).
        assert_eq!(prealloc_capacity(0), 1);
        // Real sizes below the cap pass unchanged (behavior is unchanged).
        assert_eq!(prealloc_capacity(1000), 1000);
        assert_eq!(prealloc_capacity(MAX_PREALLOC), MAX_PREALLOC);
    }

    #[test]
    fn neither_native_nor_sdk_parse_truncates_buffered_files() {
        let mut fare_rules = String::from("fare_id,price,currency\n");
        for i in 0..12 {
            fare_rules.push_str(&format!("F{i},1.00,EUR\n"));
        }
        let zip = zip_with_files(&[
            ("agency.txt", b"agency_id,agency_name,agency_url,agency_timezone\nA1,Test,http://x.test,UTC\n"),
            ("stops.txt", b"stop_id,stop_name,stop_lat,stop_lon\nS1,Stop,41,29\n"),
            ("routes.txt", b"route_id,agency_id,route_short_name,route_type\nR1,A1,1,3\n"),
            ("trips.txt", b"route_id,service_id,trip_id\nR1,S1,T1\n"),
            ("stop_times.txt", b"trip_id,arrival_time,departure_time,stop_id,stop_sequence\nT1,08:00:00,08:00:00,S1,1\n"),
            ("calendar.txt", b"service_id,monday,tuesday,wednesday,thursday,friday,saturday,sunday,start_date,end_date\nS1,1,1,1,1,1,0,0,20260101,20261231\n"),
            ("fare_rules.txt", fare_rules.as_bytes()),
        ]);
        let cfg = ValidatorConfig {
            max_file_rows: 10,
            ..ValidatorConfig::default()
        };

        let native =
            super::parse_with_limits(&zip, &cfg, super::DEFAULT_DECOMPRESSION_LIMITS, None)
                .expect("native parse başarılı olmalı");
        let sdk = super::parse_with_limits(
            &zip,
            &cfg,
            crate::decompress_guard::SDK_DECOMPRESSION_LIMITS,
            Some(crate::decompress_guard::SDK_MAX_BUFFERED_FILE_BYTES),
        )
        .expect("SDK parse başarılı olmalı");

        // #185: `max_file_rows` is a REPORTING threshold (ARC_022), not a truncation limit.
        // Both paths must read the entire file; if the SDK path starts truncating, the browser
        // validates the truncated feed as complete and reports dangling references as the user's
        // fault.
        assert_eq!(native.files["fare_rules.txt"].rows.len(), 12);
        assert_eq!(
            sdk.files["fare_rules.txt"].rows.len(),
            12,
            "SDK parse yolu buffered dosyayı kesmemeli (#185)"
        );
    }

    #[test]
    fn invalid_zip_returns_fatal_zip_unreadable() {
        let err = parse(b"not a zip").expect_err("geçersiz ZIP Fatal olmalı");
        assert_eq!(err.code, FatalCode::ZipUnreadable);
    }

    #[test]
    fn missing_required_file_returns_partial_with_arc004() {
        // ZIP without agency.txt.
        let zip = zip_with_files(&[
            ("stops.txt",      b"stop_id,stop_name,stop_lat,stop_lon\nS1,Stop1,41.0,29.0\n"),
            ("routes.txt",     b"route_id,agency_id,route_short_name,route_type\nR1,1,101,3\n"),
            ("trips.txt",      b"route_id,service_id,trip_id\nR1,SVC1,T1\n"),
            ("stop_times.txt", b"trip_id,arrival_time,departure_time,stop_id,stop_sequence\nT1,08:00:00,08:00:00,S1,1\n"),
            ("calendar.txt",   b"service_id,monday,tuesday,wednesday,thursday,friday,saturday,sunday,start_date,end_date\nSVC1,1,1,1,1,1,0,0,20240101,20241231\n"),
        ]);
        let result = parse(&zip).expect("agency.txt eksikliği kurtarılabilir olmalı");
        assert!(result
            .partial
            .unavailable_files
            .iter()
            .any(|f| f == "agency.txt"));
        assert!(result.notices.iter().any(|n| {
            n.rule_id == "ARC_004" && n.observed_value.as_deref() == Some("agency.txt")
        }));
    }

    // ── Feed wrapped in one top-level folder (detect_wrapped_root) ─────────────
    // Real case mdb-1814: `TEST GTFS/agency.txt` … a valid feed wrapped in one folder.
    // Strict behavior gave the user zero analysis. Tolerance is NARROW; the tests below lock in
    // both recovery and fatal behavior for counterexamples.

    /// Minimal but COMPLETE feed; when `prefix` is supplied, every file is placed below that folder.
    fn wrapped_feed_zip(prefix: &str) -> Vec<u8> {
        let p = |name: &str| format!("{prefix}{name}");
        let files: Vec<(String, &[u8])> = vec![
            (p("agency.txt"),     b"agency_id,agency_name,agency_url,agency_timezone\n1,A,https://e.org,Europe/Istanbul\n" as &[u8]),
            (p("stops.txt"),      b"stop_id,stop_name,stop_lat,stop_lon\nS1,Stop1,41.0,29.0\n"),
            (p("routes.txt"),     b"route_id,agency_id,route_short_name,route_type\nR1,1,101,3\n"),
            (p("trips.txt"),      b"route_id,service_id,trip_id\nR1,SVC1,T1\n"),
            (p("stop_times.txt"), b"trip_id,arrival_time,departure_time,stop_id,stop_sequence\nT1,08:00:00,08:00:00,S1,1\n"),
            (p("calendar.txt"),   b"service_id,monday,tuesday,wednesday,thursday,friday,saturday,sunday,start_date,end_date\nSVC1,1,1,1,1,1,0,0,20240101,20241231\n"),
        ];
        zip_with_files(
            &files
                .iter()
                .map(|(n, c)| (n.as_str(), *c))
                .collect::<Vec<_>>(),
        )
    }

    #[test]
    fn feed_wrapped_in_single_folder_is_parsed_and_reported() {
        let k1 =
            parse(&wrapped_feed_zip("TEST GTFS/")).expect("sarılı geçerli feed fatal OLMAMALI");
        assert!(!k1.partial.root_structural_errors.is_empty());
        // Files appear to be at the root → required-file checks pass.
        for required in REQUIRED_FILES {
            assert!(
                k1.files.contains_key(*required),
                "'{required}' kök adıyla görünmeli"
            );
        }
        // Their subdirectory location is still a finding and shows the REAL path.
        let arc024: Vec<_> = k1
            .notices
            .iter()
            .filter(|n| n.rule_id == "ARC_024")
            .collect();
        assert_eq!(arc024.len(), 6, "her .txt için bir ARC_024 beklenir");
        assert!(
            arc024.iter().all(|n| n
                .file
                .as_deref()
                .is_some_and(|f| f.starts_with("TEST GTFS/"))),
            "ARC_024 ZIP'teki gerçek yolu göstermeli"
        );
    }

    #[test]
    fn wrapped_root_without_gtfs_files_is_partial_and_auditable() {
        // GitHub repository ZIP (real case mdb-3135): one folder, but not GTFS.
        let zip = zip_with_files(&[
            ("honduras-transit-main/agency.csv", b"a,b\n1,2\n" as &[u8]),
            ("honduras-transit-main/README.md", b"# repo\n"),
        ]);
        let result = parse(&zip).expect("okunabilir ama GTFS olmayan ZIP PARTIAL olmalı");
        assert!(!result.partial.unavailable_files.is_empty());
        assert!(result.notices.iter().any(|n| n.rule_id == "ARC_004"));
    }

    #[test]
    fn wrapped_root_is_rejected_when_more_than_one_folder_holds_txt() {
        let mut files: Vec<(String, &[u8])> = Vec::new();
        for (i, name) in REQUIRED_FILES.iter().enumerate() {
            // Required files are split across TWO folders → the root is ambiguous, so no tolerance.
            files.push((format!("dir{}/{name}", i % 2), b"x\n" as &[u8]));
        }
        let zip = zip_with_files(
            &files
                .iter()
                .map(|(n, c)| (n.as_str(), *c))
                .collect::<Vec<_>>(),
        );
        let result = parse(&zip).expect("birden fazla üst klasör PARTIAL olmalı");
        assert!(result
            .partial
            .root_structural_errors
            .iter()
            .any(|e| e.contains("multiple/deep subdirectories")));
        assert_eq!(result.partial.unavailable_files.len(), REQUIRED_FILES.len());
    }

    // ── macOS AppleDouble artifacts (`__MACOSX/`) ─────────────────────────────
    // In the full catalog run, all 42 feeds receiving ARC_024 did so because of
    // `__MACOSX/._agency.txt` (#144). `._agency.txt` is not a GTFS file but a Finder resource-fork
    // sidecar added to the ZIP; "move it to the root" is the wrong recommendation. The more
    // serious consequence appears in the second test: it disables wrapped-feed tolerance.

    #[test]
    fn macosx_resource_fork_entries_do_not_produce_arc_024() {
        let zip = zip_with_files(&[
            ("agency.txt",     b"agency_id,agency_name,agency_url,agency_timezone\n1,A,https://e.org,Europe/Istanbul\n" as &[u8]),
            ("stops.txt",      b"stop_id,stop_name,stop_lat,stop_lon\nS1,Stop1,41.0,29.0\n"),
            ("routes.txt",     b"route_id,agency_id,route_short_name,route_type\nR1,1,101,3\n"),
            ("trips.txt",      b"route_id,service_id,trip_id\nR1,SVC1,T1\n"),
            ("stop_times.txt", b"trip_id,arrival_time,departure_time,stop_id,stop_sequence\nT1,08:00:00,08:00:00,S1,1\n"),
            ("calendar.txt",   b"service_id,monday,tuesday,wednesday,thursday,friday,saturday,sunday,start_date,end_date\nSVC1,1,1,1,1,1,0,0,20240101,20241231\n"),
            ("__MACOSX/._agency.txt", b"\x00\x05\x16\x07"),
            ("__MACOSX/._stops.txt",  b"\x00\x05\x16\x07"),
        ]);
        let k1 = parse(&zip).expect("kökteki geçerli feed fatal OLMAMALI");
        assert!(
            k1.notices.iter().all(|n| n.rule_id != "ARC_024"),
            "__MACOSX kaynak-çatalı ARC_024 ÜRETMEMELİ, üretilen: {:?}",
            k1.notices
                .iter()
                .filter(|n| n.rule_id == "ARC_024")
                .map(|n| n.file.clone())
                .collect::<Vec<_>>()
        );
    }

    /// `__MACOSX` must not disable wrapped-feed tolerance.
    ///
    /// `detect_wrapped_root` walks every `.txt` entry. Finder's `__MACOSX/…` entries looked like
    /// a second top-level folder (or a second level), so tolerance returned `None` → ARC_004 fatal
    /// → ZERO analysis. This is a valid-feed rejection class, much more serious than ARC_024 noise.
    #[test]
    fn macosx_sidecar_does_not_break_the_wrapper_tolerance() {
        let mut files: Vec<(String, &[u8])> = vec![
            ("TEST GTFS/agency.txt".into(),     b"agency_id,agency_name,agency_url,agency_timezone\n1,A,https://e.org,Europe/Istanbul\n" as &[u8]),
            ("TEST GTFS/stops.txt".into(),      b"stop_id,stop_name,stop_lat,stop_lon\nS1,Stop1,41.0,29.0\n"),
            ("TEST GTFS/routes.txt".into(),     b"route_id,agency_id,route_short_name,route_type\nR1,1,101,3\n"),
            ("TEST GTFS/trips.txt".into(),      b"route_id,service_id,trip_id\nR1,SVC1,T1\n"),
            ("TEST GTFS/stop_times.txt".into(), b"trip_id,arrival_time,departure_time,stop_id,stop_sequence\nT1,08:00:00,08:00:00,S1,1\n"),
            ("TEST GTFS/calendar.txt".into(),   b"service_id,monday,tuesday,wednesday,thursday,friday,saturday,sunday,start_date,end_date\nSVC1,1,1,1,1,1,0,0,20240101,20241231\n"),
        ];
        // The two forms Finder actually writes: flat at the root and mirroring the wrapped folder.
        files.push(("__MACOSX/._agency.txt".into(), b"\x00\x05\x16\x07" as &[u8]));
        files.push(("__MACOSX/TEST GTFS/._stops.txt".into(), b"\x00\x05\x16\x07"));

        let zip = zip_with_files(
            &files
                .iter()
                .map(|(n, c)| (n.as_str(), *c))
                .collect::<Vec<_>>(),
        );
        let k1 = parse(&zip).expect("__MACOSX taşıyan sarılı feed fatal OLMAMALI");

        for required in REQUIRED_FILES {
            assert!(
                k1.files.contains_key(*required),
                "'{required}' kök adıyla görünmeli — __MACOSX hoşgörüyü düşürmemeli"
            );
        }
        assert!(
            k1.notices.iter().all(|n| n.rule_id != "ARC_004"),
            "__MACOSX yüzünden ARC_004 ÜRETİLMEMELİ"
        );
        // Genuine wrapped files still receive ARC_024; only __MACOSX entries are excluded.
        let arc024: Vec<_> = k1
            .notices
            .iter()
            .filter(|n| n.rule_id == "ARC_024")
            .collect();
        assert_eq!(
            arc024.len(),
            6,
            "yalnız 6 gerçek .txt için ARC_024 beklenir"
        );
        assert!(
            arc024.iter().all(|n| n
                .file
                .as_deref()
                .is_some_and(|f| !f.starts_with("__MACOSX/"))),
            "hiçbir ARC_024 __MACOSX girdisini göstermemeli"
        );
    }

    #[test]
    fn root_level_feed_is_untouched_by_the_wrapper_tolerance() {
        let k1 = parse(&wrapped_feed_zip("")).expect("normal feed geçerli");
        assert!(
            k1.notices.iter().all(|n| n.rule_id != "ARC_024"),
            "kökteki feed ARC_024 ALMAMALI"
        );
    }

    /// A multibyte character straddling byte 8192 must not make a VALID feed fatal.
    ///
    /// Real case: in a 250-feed corpus run, 3 feeds (JP/LT/TH) were rejected for this reason
    /// alone; MD reported 0 errors on the same feeds. `trips.txt` is read partially (8 KB) at
    /// #38 → when the boundary split a character, the file was considered corrupt.
    /// It affects only feeds with non-ASCII `trips.txt` (the ASCII corpus never catches it).
    #[test]
    fn multibyte_char_across_partial_read_boundary_is_not_fatal() {
        // K1 cuts at exactly byte 8192. If the three-byte 'あ' starts at 8190, its bytes
        // land at 8190/8191/8192 → the buffer takes the first two and cuts the third.
        let mut trips = String::from("route_id,service_id,trip_id,trip_headsign\n");
        let mut i = 0;
        while trips.len() + 24 < 8_190 {
            trips.push_str(&format!("R1,SVC1,T{i},AAA\n"));
            i += 1;
        }
        while trips.len() < 8_190 {
            trips.push('A'); // pad exactly to 8190
        }
        assert_eq!(trips.len(), 8_190, "test kurgusu: hizalama");
        trips.push('あ'); // bytes 8190,8191,8192 → the cut at 8192 splits in the MIDDLE
        trips.push('\n');
        let trips_bytes = trips.as_bytes().to_vec();
        assert!(
            trips_bytes.len() > 8_192,
            "test kurgusu: dosya 8KB'ı aşmalı"
        );
        assert!(
            std::str::from_utf8(&trips_bytes[..8_192]).is_err(),
            "test kurgusu: ilk 8192 bayt çok baytlı karakteri bölmeli (yoksa test bugu kanıtlamaz)"
        );
        assert!(
            std::str::from_utf8(&trips_bytes).is_ok(),
            "test kurgusu: dosya BÜTÜN olarak geçerli UTF-8"
        );

        let zip = zip_with_files(&[
            ("agency.txt",     b"agency_id,agency_name,agency_url,agency_timezone\n1,Test,http://x.com,UTC\n"),
            ("stops.txt",      b"stop_id,stop_name,stop_lat,stop_lon\nS1,Stop1,41.0,29.0\n"),
            ("routes.txt",     b"route_id,agency_id,route_short_name,route_type\nR1,1,101,3\n"),
            ("trips.txt",      &trips_bytes),
            ("stop_times.txt", b"trip_id,arrival_time,departure_time,stop_id,stop_sequence\nT0,08:00:00,08:00:00,S1,1\n"),
            ("calendar.txt",   b"service_id,monday,tuesday,wednesday,thursday,friday,saturday,sunday,start_date,end_date\nSVC1,1,1,1,1,1,0,0,20240101,20241231\n"),
        ]);
        let res = parse(&zip);
        assert!(
            res.is_ok(),
            "geçerli UTF-8 feed fatal olmamalı: {:?}",
            res.err()
        );
        let notices = &res.unwrap().notices;
        assert!(
            !notices.iter().any(|n| n.rule_id == "ARC_002"),
            "geçerli dosyada ARC_002 (UTF-8 ihlali) üretilmemeli"
        );
    }

    /// Truncation must not hide genuinely invalid bytes in the BODY — even in a partially read file.
    #[test]
    fn genuinely_invalid_utf8_in_streamed_file_is_partial_without_typed_records() {
        let mut trips: Vec<u8> = b"route_id,service_id,trip_id\n".to_vec();
        trips.extend_from_slice(b"R1,SVC1,\xFF\xFE_bozuk\n"); // invalid sequence in the body
        while trips.len() < 9_000 {
            trips.extend_from_slice(b"R1,SVC1,T\n");
        }
        let zip = zip_with_files(&[
            ("agency.txt",     b"agency_id,agency_name,agency_url,agency_timezone\n1,Test,http://x.com,UTC\n"),
            ("stops.txt",      b"stop_id,stop_name,stop_lat,stop_lon\nS1,Stop1,41.0,29.0\n"),
            ("routes.txt",     b"route_id,agency_id,route_short_name,route_type\nR1,1,101,3\n"),
            ("trips.txt",      &trips),
            ("stop_times.txt", b"trip_id,arrival_time,departure_time,stop_id,stop_sequence\nT0,08:00:00,08:00:00,S1,1\n"),
            ("calendar.txt",   b"service_id,monday,tuesday,wednesday,thursday,friday,saturday,sunday,start_date,end_date\nSVC1,1,1,1,1,1,0,0,20240101,20241231\n"),
        ]);
        let result = parse(&zip).expect("bozuk UTF-8 dosya PARTIAL olmalı");
        assert!(result
            .partial
            .unavailable_files
            .iter()
            .any(|f| f == "trips.txt"));
        assert!(result.notices.iter().any(|n| n.rule_id == "ARC_002"));
    }

    #[test]
    fn utf8_failure_on_required_file_is_partial_without_typed_records() {
        // Invalid UTF-8 bytes in stops.txt
        let bad_bytes: &[u8] = b"stop_id,stop_name\nS1,\xFF\xFE invalid\n";
        let zip = zip_with_files(&[
            ("agency.txt",     b"agency_id,agency_name,agency_url,agency_timezone\n1,Test,http://x.com,UTC\n"),
            ("stops.txt",      bad_bytes),
            ("routes.txt",     b"route_id,agency_id,route_short_name,route_type\nR1,1,101,3\n"),
            ("trips.txt",      b"route_id,service_id,trip_id\nR1,SVC1,T1\n"),
            ("stop_times.txt", b"trip_id,arrival_time,departure_time,stop_id,stop_sequence\nT1,08:00:00,08:00:00,S1,1\n"),
            ("calendar.txt",   b"service_id,monday,tuesday,wednesday,thursday,friday,saturday,sunday,start_date,end_date\nSVC1,1,1,1,1,1,0,0,20240101,20241231\n"),
        ]);
        let result = parse(&zip).expect("UTF-8 hatası PARTIAL olmalı");
        assert!(result
            .partial
            .unavailable_files
            .iter()
            .any(|f| f == "stops.txt"));
        assert!(result.notices.iter().any(|n| n.rule_id == "ARC_002"));
    }

    #[test]
    fn csv_malformed_on_required_file_is_partial_without_typed_records() {
        // #38: stop_times/trips/calendar_dates are read partially in K1 (8 KB header) → body in K2.
        // routes.txt is used for the malformed CSV test (it is still read in full in K1).
        let bad_routes = b"route_id,agency_id,route_short_name,route_type\n\"unclosed quote\n";
        let zip = zip_with_files(&[
            ("agency.txt",     b"agency_id,agency_name,agency_url,agency_timezone\n1,Test,http://x.com,UTC\n"),
            ("stops.txt",      b"stop_id,stop_name,stop_lat,stop_lon\nS1,Stop1,41.0,29.0\n"),
            ("routes.txt",     bad_routes),
            ("trips.txt",      b"route_id,service_id,trip_id\nR1,SVC1,T1\n"),
            ("stop_times.txt", b"trip_id,arrival_time,departure_time,stop_id,stop_sequence\nT1,08:00:00,08:00:00,S1,1\n"),
            ("calendar.txt",   b"service_id,monday,tuesday,wednesday,thursday,friday,saturday,sunday,start_date,end_date\nSVC1,1,1,1,1,1,0,0,20240101,20241231\n"),
        ]);
        let result = parse(&zip).expect("bozuk CSV PARTIAL olmalı");
        assert!(result
            .partial
            .unavailable_files
            .iter()
            .any(|f| f == "routes.txt"));
        assert!(result.notices.iter().any(|n| n.rule_id == "ARC_013"));
    }

    #[test]
    fn unknown_file_produces_arc007_notice() {
        let zip = zip_with_files(&[
            ("agency.txt",     b"agency_id,agency_name,agency_url,agency_timezone\n1,Test,http://x.com,UTC\n"),
            ("stops.txt",      b"stop_id,stop_name,stop_lat,stop_lon\nS1,Stop1,41.0,29.0\n"),
            ("routes.txt",     b"route_id,agency_id,route_short_name,route_type\nR1,1,101,3\n"),
            ("trips.txt",      b"route_id,service_id,trip_id\nR1,SVC1,T1\n"),
            ("stop_times.txt", b"trip_id,arrival_time,departure_time,stop_id,stop_sequence\nT1,08:00:00,08:00:00,S1,1\n"),
            ("calendar.txt",   b"service_id,monday,tuesday,wednesday,thursday,friday,saturday,sunday,start_date,end_date\nSVC1,1,1,1,1,1,0,0,20240101,20241231\n"),
            ("custom_data.txt",b"foo,bar\n1,2\n"),
        ]);
        let k1 = parse(&zip).unwrap();
        assert!(
            k1.notices.iter().any(|n| n.rule_id == "ARC_007"),
            "ARC_007 notice bekleniyor"
        );
    }

    #[test]
    fn non_txt_unknown_file_produces_arc007() {
        // MobilityData unknown_file parity: non-.txt files in the ZIP
        // (e.g. .pdf/.out) must also produce ARC_007 — the DELFI feed had .pdf + .out.
        let zip = zip_with_files(&[
            ("agency.txt",     b"agency_id,agency_name,agency_url,agency_timezone\n1,Test,http://x.com,UTC\n"),
            ("stops.txt",      b"stop_id,stop_name,stop_lat,stop_lon\nS1,Stop1,41.0,29.0\n"),
            ("routes.txt",     b"route_id,agency_id,route_short_name,route_type\nR1,1,101,3\n"),
            ("trips.txt",      b"route_id,service_id,trip_id\nR1,SVC1,T1\n"),
            ("stop_times.txt", b"trip_id,arrival_time,departure_time,stop_id,stop_sequence\nT1,08:00:00,08:00:00,S1,1\n"),
            ("calendar.txt",   b"service_id,monday,tuesday,wednesday,thursday,friday,saturday,sunday,start_date,end_date\nSVC1,1,1,1,1,1,0,0,20240101,20241231\n"),
            ("readme.pdf",     b"%PDF-1.4 fake\n"),
            ("export.out",     b"legacy dump\n"),
        ]);
        let k1 = parse(&zip).unwrap();
        for f in ["readme.pdf", "export.out"] {
            assert!(
                k1.notices
                    .iter()
                    .any(|n| n.rule_id == "ARC_007" && n.file.as_deref() == Some(f)),
                "ARC_007 notice bekleniyor: {f}"
            );
        }
    }

    #[test]
    fn unknown_column_in_fares_v2_produces_arc017() {
        // Regression: because known_columns did not include Fares v2 files, ARC_017 skipped
        // these files entirely. In rider_categories.txt, eligibility_url is VALID (spec), while
        // zzz_custom must be UNKNOWN.
        let zip = zip_with_files(&[
            ("agency.txt",     b"agency_id,agency_name,agency_url,agency_timezone\n1,Test,http://x.com,UTC\n"),
            ("stops.txt",      b"stop_id,stop_name,stop_lat,stop_lon\nS1,Stop1,41.0,29.0\n"),
            ("routes.txt",     b"route_id,agency_id,route_short_name,route_type\nR1,1,101,3\n"),
            ("trips.txt",      b"route_id,service_id,trip_id\nR1,SVC1,T1\n"),
            ("stop_times.txt", b"trip_id,arrival_time,departure_time,stop_id,stop_sequence\nT1,08:00:00,08:00:00,S1,1\n"),
            ("calendar.txt",   b"service_id,monday,tuesday,wednesday,thursday,friday,saturday,sunday,start_date,end_date\nSVC1,1,1,1,1,1,0,0,20240101,20241231\n"),
            ("rider_categories.txt", b"rider_category_id,rider_category_name,eligibility_url,zzz_custom\nRC1,Adult,http://x.com,foo\n"),
        ]);
        let k1 = parse(&zip).unwrap();
        let arc017_cols: Vec<&str> = k1
            .notices
            .iter()
            .filter(|n| n.rule_id == "ARC_017" && n.file.as_deref() == Some("rider_categories.txt"))
            .filter_map(|n| n.field.as_deref())
            .collect();
        assert!(
            arc017_cols.contains(&"zzz_custom"),
            "bilinmeyen sütun zzz_custom ARC_017 üretmeli: {arc017_cols:?}"
        );
        assert!(
            !arc017_cols.contains(&"eligibility_url"),
            "geçerli sütun eligibility_url ARC_017 üretmemeli"
        );
        assert!(
            !arc017_cols.contains(&"rider_category_id"),
            "geçerli sütun rider_category_id ARC_017 üretmemeli"
        );
    }

    #[test]
    fn unknown_column_in_gtfs_jp_produces_arc017() {
        // office_jp.txt: office_id/office_name are VALID; jp_extra is skipped by the jp_ prefix;
        // bad_col is UNKNOWN.
        let zip = zip_with_files(&[
            ("agency.txt",     b"agency_id,agency_name,agency_url,agency_timezone\n1,Test,http://x.com,UTC\n"),
            ("stops.txt",      b"stop_id,stop_name,stop_lat,stop_lon\nS1,Stop1,41.0,29.0\n"),
            ("routes.txt",     b"route_id,agency_id,route_short_name,route_type\nR1,1,101,3\n"),
            ("trips.txt",      b"route_id,service_id,trip_id\nR1,SVC1,T1\n"),
            ("stop_times.txt", b"trip_id,arrival_time,departure_time,stop_id,stop_sequence\nT1,08:00:00,08:00:00,S1,1\n"),
            ("calendar.txt",   b"service_id,monday,tuesday,wednesday,thursday,friday,saturday,sunday,start_date,end_date\nSVC1,1,1,1,1,1,0,0,20240101,20241231\n"),
            ("office_jp.txt",  b"office_id,office_name,jp_extra,bad_col\nO1,Ofis,x,y\n"),
        ]);
        let k1 = parse(&zip).unwrap();
        let cols: Vec<&str> = k1
            .notices
            .iter()
            .filter(|n| n.rule_id == "ARC_017" && n.file.as_deref() == Some("office_jp.txt"))
            .filter_map(|n| n.field.as_deref())
            .collect();
        assert!(
            cols.contains(&"bad_col"),
            "bad_col ARC_017 üretmeli: {cols:?}"
        );
        assert!(
            !cols.contains(&"office_name"),
            "office_name geçerli, ARC_017 üretmemeli"
        );
        assert!(!cols.contains(&"jp_extra"), "jp_ önekli sütun atlanmalı");
    }

    #[test]
    fn every_known_file_has_column_list_for_arc017() {
        // ARC_017 (K1: `if !known_cols.is_empty()`) skips a file entirely when
        // known_columns() returns empty. Every file in KNOWN_FILES must have a column list;
        // otherwise unknown columns in that file silently escape (the root cause of this bug).
        let missing: Vec<&str> = KNOWN_FILES
            .iter()
            .copied()
            .filter(|&f| known_columns(f).is_empty())
            .collect();
        assert!(
            missing.is_empty(),
            "known_columns() boş dönen KNOWN_FILES: {missing:?}"
        );
    }

    #[test]
    fn bom_produces_arc010_notice() {
        let bom_csv = b"\xEF\xBB\xBFstop_id,stop_name,stop_lat,stop_lon\nS1,Stop1,41.0,29.0\n";
        let zip = zip_with_files(&[
            ("agency.txt",     b"agency_id,agency_name,agency_url,agency_timezone\n1,Test,http://x.com,UTC\n"),
            ("stops.txt",      bom_csv),
            ("routes.txt",     b"route_id,agency_id,route_short_name,route_type\nR1,1,101,3\n"),
            ("trips.txt",      b"route_id,service_id,trip_id\nR1,SVC1,T1\n"),
            ("stop_times.txt", b"trip_id,arrival_time,departure_time,stop_id,stop_sequence\nT1,08:00:00,08:00:00,S1,1\n"),
            ("calendar.txt",   b"service_id,monday,tuesday,wednesday,thursday,friday,saturday,sunday,start_date,end_date\nSVC1,1,1,1,1,1,0,0,20240101,20241231\n"),
        ]);
        let k1 = parse(&zip).unwrap();
        assert!(
            k1.notices.iter().any(|n| n.rule_id == "ARC_010"),
            "ARC_010 notice bekleniyor"
        );
    }

    #[test]
    fn header_whitespace_produces_arc014_notice() {
        let csv = b"stop_id, stop_name,stop_lat,stop_lon\nS1,Stop1,41.0,29.0\n";
        let zip = zip_with_files(&[
            ("agency.txt",     b"agency_id,agency_name,agency_url,agency_timezone\n1,Test,http://x.com,UTC\n"),
            ("stops.txt",      csv),
            ("routes.txt",     b"route_id,agency_id,route_short_name,route_type\nR1,1,101,3\n"),
            ("trips.txt",      b"route_id,service_id,trip_id\nR1,SVC1,T1\n"),
            ("stop_times.txt", b"trip_id,arrival_time,departure_time,stop_id,stop_sequence\nT1,08:00:00,08:00:00,S1,1\n"),
            ("calendar.txt",   b"service_id,monday,tuesday,wednesday,thursday,friday,saturday,sunday,start_date,end_date\nSVC1,1,1,1,1,1,0,0,20240101,20241231\n"),
        ]);
        let k1 = parse(&zip).unwrap();
        assert!(
            k1.notices.iter().any(|n| n.rule_id == "ARC_014"),
            "ARC_014 notice bekleniyor"
        );
    }

    #[test]
    fn arc009_tolerates_an_empty_calendar_when_calendar_dates_carries_the_service() {
        // #166: spec "calendar.txt — Required UNLESS all dates of service are defined in
        // calendar_dates.txt". Suppression had been written only in the opposite direction;
        // 59 of the corpus's 104 CRITICAL ARC_009 feeds were exactly this case.
        let zip = zip_with_files(&[
            ("agency.txt",     b"agency_id,agency_name,agency_url,agency_timezone\n1,Test,http://x.com,UTC\n"),
            ("stops.txt",      b"stop_id,stop_name,stop_lat,stop_lon\nS1,Stop1,41.0,29.0\n"),
            ("routes.txt",     b"route_id,agency_id,route_short_name,route_long_name,route_type\nR1,1,101,Main,3\n"),
            ("trips.txt",      b"route_id,service_id,trip_id\nR1,SVC1,T1\n"),
            ("stop_times.txt", b"trip_id,arrival_time,departure_time,stop_id,stop_sequence\nT1,08:00:00,08:00:00,S1,1\n"),
            ("calendar.txt",   b"service_id,monday,tuesday,wednesday,thursday,friday,saturday,sunday,start_date,end_date\n"),
            ("calendar_dates.txt", b"service_id,date,exception_type\nSVC1,20240101,1\n"),
        ]);
        let k1 = parse(&zip).unwrap();
        let hits: Vec<_> = k1
            .notices
            .iter()
            .filter(|n| n.rule_id == "ARC_009" && n.file.as_deref() == Some("calendar.txt"))
            .collect();
        assert!(
            hits.is_empty(),
            "calendar_dates.txt servisi taşıyorken boş calendar.txt geçerlidir: {hits:?}"
        );
    }

    #[test]
    fn arc035_still_reports_an_empty_required_file() {
        // Tolerance applies only to the CONDITIONAL PAIR; the rule must not go silent when a
        // genuinely required file is empty. On 2026-08-31 this requirement moved from ARC_009
        // to ARC_035 (Critical·Spec) — because the class could not be dynamic, the remaining
        // Quality ARC_009 could pass the R1 gate, so an empty required file did not fail
        // publication. THE CLAIM IS THE SAME: an empty required file must not be silent.
        let zip = zip_with_files(&[
            ("agency.txt",     b"agency_id,agency_name,agency_url,agency_timezone\n1,Test,http://x.com,UTC\n"),
            ("stops.txt",      b"stop_id,stop_name,stop_lat,stop_lon\n"),
            ("routes.txt",     b"route_id,agency_id,route_short_name,route_long_name,route_type\nR1,1,101,Main,3\n"),
            ("trips.txt",      b"route_id,service_id,trip_id\nR1,SVC1,T1\n"),
            ("stop_times.txt", b"trip_id,arrival_time,departure_time,stop_id,stop_sequence\nT1,08:00:00,08:00:00,S1,1\n"),
            ("calendar.txt",   b"service_id,monday,tuesday,wednesday,thursday,friday,saturday,sunday,start_date,end_date\nSVC1,1,1,1,1,1,0,0,20240101,20241231\n"),
        ]);
        let k1 = parse(&zip).unwrap();
        assert!(
            k1.notices
                .iter()
                .any(|n| n.rule_id == "ARC_035" && n.file.as_deref() == Some("stops.txt")),
            "boş stops.txt ARC_035 üretmeli"
        );
        assert!(
            !k1.notices
                .iter()
                .any(|n| n.rule_id == "ARC_009" && n.file.as_deref() == Some("stops.txt")),
            "zorunlu dosya ARC_009'a DÜŞMEMELİ — çift raporlama olurdu"
        );
    }

    #[test]
    fn arc034_reports_a_header_row_repeated_as_data() {
        // #168: the header is repeated in the middle of agency.txt in mdb-1003/mdb-1004.
        // One structural defect used to spread into AGN_004 + AGN_005 + TRP_006, and none of
        // them told the user the root problem.
        let zip = zip_with_files(&[
            ("agency.txt",     b"agency_id,agency_name,agency_url,agency_timezone\n1,Test,http://x.com,UTC\nagency_id,agency_name,agency_url,agency_timezone\n2,Test2,http://y.com,UTC\n"),
            ("stops.txt",      b"stop_id,stop_name,stop_lat,stop_lon\nS1,Stop1,41.0,29.0\n"),
            ("routes.txt",     b"route_id,agency_id,route_short_name,route_long_name,route_type\nR1,1,101,Main,3\n"),
            ("trips.txt",      b"route_id,service_id,trip_id\nR1,SVC1,T1\n"),
            ("stop_times.txt", b"trip_id,arrival_time,departure_time,stop_id,stop_sequence\nT1,08:00:00,08:00:00,S1,1\n"),
            ("calendar.txt",   b"service_id,monday,tuesday,wednesday,thursday,friday,saturday,sunday,start_date,end_date\nSVC1,1,1,1,1,1,0,0,20240101,20241231\n"),
        ]);
        let k1 = parse(&zip).unwrap();
        let hits: Vec<_> = k1
            .notices
            .iter()
            .filter(|n| n.rule_id == "ARC_034")
            .collect();
        assert_eq!(
            hits.len(),
            1,
            "tekrarlanan başlık BİR kez raporlanmalı: {:?}",
            k1.notices.iter().map(|n| &n.rule_id).collect::<Vec<_>>()
        );
        assert_eq!(hits[0].file.as_deref(), Some("agency.txt"));
        // Do NOT STORE the row as data: no phantom agency must be created.
        let agencies = k1
            .files
            .get("agency.txt")
            .map(|f| f.rows.len())
            .unwrap_or(0);
        assert_eq!(agencies, 2, "başlık tekrarı veri satırı sayılmamalı");
    }

    #[test]
    fn arc034_does_not_fire_when_only_one_value_matches_its_column() {
        // One field equalling its own column name is legitimate; the rule speaks only when ALL
        // fields match.
        let zip = zip_with_files(&[
            ("agency.txt",     b"agency_id,agency_name,agency_url,agency_timezone\nagency_id,Test,http://x.com,UTC\n"),
            ("stops.txt",      b"stop_id,stop_name,stop_lat,stop_lon\nS1,Stop1,41.0,29.0\n"),
            ("routes.txt",     b"route_id,agency_id,route_short_name,route_long_name,route_type\nR1,agency_id,101,Main,3\n"),
            ("trips.txt",      b"route_id,service_id,trip_id\nR1,SVC1,T1\n"),
            ("stop_times.txt", b"trip_id,arrival_time,departure_time,stop_id,stop_sequence\nT1,08:00:00,08:00:00,S1,1\n"),
            ("calendar.txt",   b"service_id,monday,tuesday,wednesday,thursday,friday,saturday,sunday,start_date,end_date\nSVC1,1,1,1,1,1,0,0,20240101,20241231\n"),
        ]);
        let k1 = parse(&zip).unwrap();
        assert!(
            !k1.notices.iter().any(|n| n.rule_id == "ARC_034"),
            "tek alan eşleşmesi ARC_034 üretmemeli"
        );
    }

    #[test]
    fn dq016_declares_whitespace_in_an_identity_column() {
        // #169: whitespace in an identity field is not the same as in a description field.
        // If `routes.txt` writes the key with whitespace and `trips.txt` without it, every
        // cross-file lookup silently misses. The decision to compare identities RAW (#85) is
        // preserved; the only change is DECLARING this fact. In `mdb-2653`, the 1,502 block
        // inconsistencies missed by `TRP_024` appeared nowhere.
        let zip = zip_with_files(&[
            ("agency.txt",     b"agency_id,agency_name,agency_url,agency_timezone\n1,Test,http://x.com,UTC\n"),
            ("stops.txt",      b"stop_id,stop_name,stop_lat,stop_lon\nS1,Stop1,41.0,29.0\n"),
            ("routes.txt",     b"route_id,agency_id,route_short_name,route_long_name,route_type\nR1  ,1,101,Main,3\n"),
            ("trips.txt",      b"route_id,service_id,trip_id\nR1,SVC1,T1\n"),
            ("stop_times.txt", b"trip_id,arrival_time,departure_time,stop_id,stop_sequence\nT1,08:00:00,08:00:00,S1,1\n"),
            ("calendar.txt",   b"service_id,monday,tuesday,wednesday,thursday,friday,saturday,sunday,start_date,end_date\nSVC1,1,1,1,1,1,0,0,20240101,20241231\n"),
        ]);
        let k1 = parse(&zip).unwrap();
        let routes_root = k1
            .notices
            .iter()
            .find(|n| n.rule_id == "DQ_016" && n.file.as_deref() == Some("routes.txt"))
            .and_then(|n| n.details.clone())
            .expect("routes.txt için DQ_016 kökü bekleniyor");
        assert_eq!(
            routes_root
                .get("identity_whitespace_fields")
                .map(String::as_str),
            Some("route_id"),
            "kimlik sütunu adıyla beyan edilmeli: {routes_root:?}"
        );
        assert!(
            routes_root.contains_key("cross_file_analysis"),
            "dosyalar arası analizin güvenilmezliği beyan edilmeli: {routes_root:?}"
        );
        // Planner context: the sample's line and the record (raw key) it sits on.
        assert_eq!(
            routes_root.get("record_field").map(String::as_str),
            Some("route_id")
        );
        assert_eq!(
            routes_root.get("sample_0_field").map(String::as_str),
            Some("route_id")
        );
        assert_eq!(
            routes_root.get("sample_0_line").map(String::as_str),
            Some("2")
        );
        assert_eq!(
            routes_root.get("sample_0_record").map(String::as_str),
            Some("R1  ")
        );
    }

    #[test]
    fn dq016_samples_name_the_record_and_line_they_sit_on() {
        // A stop name with spaces on the second stop: the sample names stop S2 on line 3.
        let zip = zip_with_files(&[
            ("agency.txt",     b"agency_id,agency_name,agency_url,agency_timezone\n1,Test,http://x.com,UTC\n"),
            ("stops.txt",      b"stop_name,stop_id,stop_lat,stop_lon\nStop1,S1,41.0,29.0\n Liman ,S2,41.1,29.1\nPark ,S3,41.2,29.2\n"),
            ("routes.txt",     b"route_id,agency_id,route_short_name,route_long_name,route_type\nR1,1,101,Main,3\n"),
            ("trips.txt",      b"route_id,service_id,trip_id\nR1,SVC1,T1\n"),
            ("stop_times.txt", b"trip_id,arrival_time,departure_time,stop_id,stop_sequence\nT1,08:00:00,08:00:00,S1,1\n"),
            ("calendar.txt",   b"service_id,monday,tuesday,wednesday,thursday,friday,saturday,sunday,start_date,end_date\nSVC1,1,1,1,1,1,0,0,20240101,20241231\n"),
        ]);
        let k1 = parse(&zip).unwrap();
        let stops_root = k1
            .notices
            .iter()
            .find(|n| n.rule_id == "DQ_016" && n.file.as_deref() == Some("stops.txt"))
            .and_then(|n| n.details.clone())
            .expect("stops.txt için DQ_016 kökü bekleniyor");
        assert_eq!(
            stops_root.get("affected_rows").map(String::as_str),
            Some("2")
        );
        assert_eq!(
            stops_root.get("record_field").map(String::as_str),
            Some("stop_id")
        );
        assert_eq!(
            stops_root.get("sample_0_field").map(String::as_str),
            Some("stop_name")
        );
        assert_eq!(
            stops_root.get("sample_0_line").map(String::as_str),
            Some("3")
        );
        assert_eq!(
            stops_root.get("sample_0_record").map(String::as_str),
            Some("S2")
        );
    }

    #[test]
    fn dq016_does_not_call_a_description_column_an_identity() {
        // Do NOT declare identity whitespace when only a description field has it — otherwise
        // the declaration would appear in every feed and lose its meaning.
        let zip = zip_with_files(&[
            ("agency.txt",     b"agency_id,agency_name,agency_url,agency_timezone\n1,Test,http://x.com,UTC\n"),
            ("stops.txt",      b"stop_id,stop_name,stop_lat,stop_lon\nS1,Stop1,41.0,29.0\n"),
            ("routes.txt",     b"route_id,agency_id,route_short_name,route_long_name,route_type\nR1,1,101,Main Line ,3\n"),
            ("trips.txt",      b"route_id,service_id,trip_id\nR1,SVC1,T1\n"),
            ("stop_times.txt", b"trip_id,arrival_time,departure_time,stop_id,stop_sequence\nT1,08:00:00,08:00:00,S1,1\n"),
            ("calendar.txt",   b"service_id,monday,tuesday,wednesday,thursday,friday,saturday,sunday,start_date,end_date\nSVC1,1,1,1,1,1,0,0,20240101,20241231\n"),
        ]);
        let k1 = parse(&zip).unwrap();
        let routes_root = k1
            .notices
            .iter()
            .find(|n| n.rule_id == "DQ_016" && n.file.as_deref() == Some("routes.txt"))
            .and_then(|n| n.details.clone())
            .expect("routes.txt için DQ_016 kökü bekleniyor");
        assert!(
            !routes_root.contains_key("identity_whitespace_fields"),
            "route_long_name bir kimlik değildir: {routes_root:?}"
        );
    }

    #[test]
    fn dq016_flags_typed_and_text_fields_with_raw_evidence() {
        // DQ_016 now reports the raw lexical violation independently of parser tolerance:
        // whitespace in a time field is also a root finding. Typed-parse/FK symptoms are linked
        // to the root finding later in K7.
        let zip = zip_with_files(&[
            ("agency.txt",     b"agency_id,agency_name,agency_url,agency_timezone\n1,Test,http://x.com,UTC\n"),
            ("stops.txt",      b"stop_id,stop_name,stop_lat,stop_lon\nS1,Stop1,41.0,29.0\n"),
            ("routes.txt",     b"route_id,agency_id,route_short_name,route_long_name,route_type\nR1,1,101,Main Line , 3\n"),
            ("trips.txt",      b"route_id,service_id,trip_id\nR1,SVC1,T1\n"),
            ("stop_times.txt", b"trip_id,arrival_time,departure_time,stop_id,stop_sequence\nT1, 8:00:00, 8:00:00,S1,1\n"),
            ("calendar.txt",   b"service_id,monday,tuesday,wednesday,thursday,friday,saturday,sunday,start_date,end_date\nSVC1,1,1,1,1,1,0,0,20240101,20241231\n"),
        ]);
        let k1 = parse(&zip).unwrap();
        let dq016: Vec<_> = k1
            .notices
            .iter()
            .filter(|n| n.rule_id == "DQ_016")
            .collect();
        assert!(
            dq016.iter().any(|n| n.message.contains("route_long_name")),
            "route_long_name sondaki boşluk → DQ_016 beklenir: {:?}",
            dq016.iter().map(|n| &n.message).collect::<Vec<_>>()
        );
        assert!(
            dq016.iter().any(|n| n.message.contains("route_type")),
            "tipli enum alanı da DQ_016 kök bulgusu olmalı: {:?}",
            dq016.iter().map(|n| &n.message).collect::<Vec<_>>()
        );
        let evidence = dq016
            .iter()
            .find_map(|n| n.details.as_ref())
            .and_then(|d| d.get("raw_samples"))
            .cloned()
            .unwrap_or_default();
        assert!(
            evidence.contains("route_type=\" 3\""),
            "ham tipli değer notice kanıtında korunmalı: {evidence}"
        );
    }

    #[test]
    fn no_calendar_produces_arc008_notice() {
        // Both calendar.txt and calendar_dates.txt are absent.
        let zip = zip_with_files(&[
            ("agency.txt",     b"agency_id,agency_name,agency_url,agency_timezone\n1,Test,http://x.com,UTC\n"),
            ("stops.txt",      b"stop_id,stop_name,stop_lat,stop_lon\nS1,Stop1,41.0,29.0\n"),
            ("routes.txt",     b"route_id,agency_id,route_short_name,route_type\nR1,1,101,3\n"),
            ("trips.txt",      b"route_id,service_id,trip_id\nR1,SVC1,T1\n"),
            ("stop_times.txt", b"trip_id,arrival_time,departure_time,stop_id,stop_sequence\nT1,08:00:00,08:00:00,S1,1\n"),
        ]);
        let k1 = parse(&zip).unwrap();
        assert!(
            k1.notices.iter().any(|n| n.rule_id == "ARC_008"),
            "ARC_008 notice bekleniyor"
        );
    }

    #[test]
    fn empty_calendar_dates_with_calendar_suppresses_arc009() {
        // calendar.txt defines service + calendar_dates.txt is header-only → ARC_009 is a false
        // positive and must be suppressed (calendar_dates is conditionally required).
        let zip = zip_with_files(&[
            ("agency.txt",     b"agency_id,agency_name,agency_url,agency_timezone\n1,Test,http://x.com,UTC\n"),
            ("stops.txt",      b"stop_id,stop_name,stop_lat,stop_lon\nS1,Stop1,41.0,29.0\n"),
            ("routes.txt",     b"route_id,agency_id,route_short_name,route_type\nR1,1,101,3\n"),
            ("trips.txt",      b"route_id,service_id,trip_id\nR1,SVC1,T1\n"),
            ("stop_times.txt", b"trip_id,arrival_time,departure_time,stop_id,stop_sequence\nT1,08:00:00,08:00:00,S1,1\n"),
            ("calendar.txt",   b"service_id,monday,tuesday,wednesday,thursday,friday,saturday,sunday,start_date,end_date\nSVC1,1,1,1,1,1,0,0,20240101,20241231\n"),
            ("calendar_dates.txt", b"service_id,date,exception_type\n"),
        ]);
        let k1 = parse(&zip).unwrap();
        let cd_arc009 = k1
            .notices
            .iter()
            .any(|n| n.rule_id == "ARC_009" && n.file.as_deref() == Some("calendar_dates.txt"));
        assert!(
            !cd_arc009,
            "calendar.txt varken boş calendar_dates.txt ARC_009 üretmemeli"
        );
    }

    #[test]
    fn empty_calendar_dates_without_calendar_produces_arc009() {
        // calendar.txt ABSENT + calendar_dates.txt header-only → no service is defined → ARC_009
        // REMAINS (the sole signal; ARC_008 does not fire because calendar_dates exists).
        let zip = zip_with_files(&[
            ("agency.txt",     b"agency_id,agency_name,agency_url,agency_timezone\n1,Test,http://x.com,UTC\n"),
            ("stops.txt",      b"stop_id,stop_name,stop_lat,stop_lon\nS1,Stop1,41.0,29.0\n"),
            ("routes.txt",     b"route_id,agency_id,route_short_name,route_type\nR1,1,101,3\n"),
            ("trips.txt",      b"route_id,service_id,trip_id\nR1,SVC1,T1\n"),
            ("stop_times.txt", b"trip_id,arrival_time,departure_time,stop_id,stop_sequence\nT1,08:00:00,08:00:00,S1,1\n"),
            ("calendar_dates.txt", b"service_id,date,exception_type\n"),
        ]);
        let k1 = parse(&zip).unwrap();
        let cd_arc009 = k1
            .notices
            .iter()
            .any(|n| n.rule_id == "ARC_009" && n.file.as_deref() == Some("calendar_dates.txt"));
        assert!(
            cd_arc009,
            "calendar.txt yokken boş calendar_dates.txt ARC_009 üretmeli"
        );
        assert!(
            !k1.notices.iter().any(|n| n.rule_id == "ARC_008"),
            "calendar_dates.txt mevcutken ARC_008 fire etmemeli"
        );
    }

    #[test]
    fn empty_optional_file_arc009_is_info_not_critical() {
        // Empty (header-only) optional transfers.txt → ARC_009 fires but is not CRITICAL; it is INFO.
        let zip = zip_with_files(&[
            ("agency.txt",     b"agency_id,agency_name,agency_url,agency_timezone\n1,Test,http://x.com,UTC\n"),
            ("stops.txt",      b"stop_id,stop_name,stop_lat,stop_lon\nS1,Stop1,41.0,29.0\n"),
            ("routes.txt",     b"route_id,agency_id,route_short_name,route_type\nR1,1,101,3\n"),
            ("trips.txt",      b"route_id,service_id,trip_id\nR1,SVC1,T1\n"),
            ("stop_times.txt", b"trip_id,arrival_time,departure_time,stop_id,stop_sequence\nT1,08:00:00,08:00:00,S1,1\n"),
            ("calendar.txt",   b"service_id,monday,tuesday,wednesday,thursday,friday,saturday,sunday,start_date,end_date\nSVC1,1,1,1,1,1,0,0,20240101,20241231\n"),
            ("transfers.txt",  b"from_stop_id,to_stop_id,transfer_type,min_transfer_time\n"),
        ]);
        let k1 = parse(&zip).unwrap();
        let n = k1
            .notices
            .iter()
            .find(|n| n.rule_id == "ARC_009" && n.file.as_deref() == Some("transfers.txt"))
            .expect("boş transfers.txt ARC_009 üretmeli");
        assert_eq!(
            n.severity,
            Severity::Bilgi,
            "boş OPSİYONEL dosya ARC_009 BİLGİ olmalı (KRİTİK değil)"
        );
    }

    #[test]
    fn tokenize_csv_quoted_field() {
        let text = "a,\"b,c\",d\n1,\"hello\nworld\",3\n";
        let (records, _, _) = tokenize_csv(text, None).unwrap();
        assert_eq!(records[0][0], "a");
        assert_eq!(records[0][1], "b,c");
        assert_eq!(records[0][2], "d");
        assert_eq!(records[1][1], "hello\nworld");
    }

    /// Text after a closing quote stays in the SAME field — the field count is preserved.
    ///
    /// 🔴 Regression: the old tokenizer closed the field at the quote and made the remainder a
    /// NEW field, so a cell in `tdg-80973/routes.txt` split into two fields and later columns
    /// shifted. When `route_id` looked empty, `RTS_031` invented the Spec·CRITICAL "route_id is
    /// required" finding — on exactly the 7 rows counted by `ARC_033`. REPORTING the violation
    /// was correct; CORRUPTING THE DATA was not.
    #[test]
    fn tokenize_csv_keeps_text_after_closing_quote_in_same_field() {
        let text = "a,b,c\n1,\"Thonon Les Arts\" A compter du 01/09,3\n";
        let (records, _, rfc) = tokenize_csv(text, None).unwrap();
        // The field count must match the header — NO drift.
        assert_eq!(
            records[1].len(),
            3,
            "kapanış tırnağı sonrası metin yeni alan açmamalı"
        );
        assert_eq!(records[1][1], "Thonon Les Arts A compter du 01/09");
        assert_eq!(records[1][2], "3", "sonraki sütun kaymamalı");
        // The violation must still be reported (ARC_033).
        assert!(rfc.rows > 0, "RFC 4180 ihlali raporlanmaya devam etmeli");
    }

    #[test]
    fn tokenize_csv_accepts_quoted_final_field_at_eof() {
        let text = "a,b\n\"1\",\"a\"\"b\"";
        let (records, _, _) = tokenize_csv(text, None).unwrap();
        assert_eq!(records[1], vec!["1", "a\"b"]);
    }

    #[test]
    fn tokenize_csv_unclosed_quote_returns_err() {
        assert!(tokenize_csv("\"unclosed\n", None).is_err());
        assert!(tokenize_csv("\"unclosed", None).is_err());
    }

    #[test]
    fn tokenize_csv_trailing_comma_no_newline() {
        // When a file ends with a trailing comma and no newline, the empty field must not be omitted.
        let text = "a,b,c\n1,2,";
        let (records, _, _) = tokenize_csv(text, None).unwrap();
        assert_eq!(records.len(), 2);
        assert_eq!(
            records[1].len(),
            3,
            "sondaki virgül boş 3. alanı temsil eder"
        );
        assert_eq!(records[1][2], "");
    }

    #[test]
    fn notice_ids_are_unique() {
        let zip = minimal_gtfs_zip();
        let k1 = parse(&zip).unwrap();
        let ids: Vec<&str> = k1.notices.iter().map(|n| n.id.as_str()).collect();
        let unique: HashSet<&str> = ids.iter().copied().collect();
        assert_eq!(ids.len(), unique.len(), "Tekrar eden notice ID var");
    }

    #[test]
    fn all_notices_have_valid_rule_ids() {
        let zip = minimal_gtfs_zip();
        let k1 = parse(&zip).unwrap();
        for n in &k1.notices {
            assert!(
                get_rule(&n.rule_id).is_some(),
                "Geçersiz rule_id: {}",
                n.rule_id
            );
        }
    }

    #[test]
    fn arc_021_fires_for_non_ascii_char() {
        // Private-use character (U+E000) in stop_id in stops.txt — problematic character.
        let zip = zip_with_files(&[
            ("agency.txt",     b"agency_id,agency_name,agency_url,agency_timezone\n1,Test,http://x.com,UTC\n"),
            ("stops.txt",      "stop_id,stop_name,stop_lat,stop_lon\nS\u{E000}1,Durak,41.0,29.0\n".as_bytes()),
            ("routes.txt",     b"route_id,agency_id,route_short_name,route_type\nR1,1,101,3\n"),
            ("trips.txt",      b"route_id,service_id,trip_id\nR1,SVC1,T1\n"),
            ("stop_times.txt", b"trip_id,arrival_time,departure_time,stop_id,stop_sequence\nT1,08:00:00,08:00:00,S1,1\n"),
            ("calendar.txt",   b"service_id,monday,tuesday,wednesday,thursday,friday,saturday,sunday,start_date,end_date\nSVC1,1,1,1,1,1,0,0,20240101,20241231\n"),
        ]);
        let k1 = parse(&zip).unwrap();
        assert!(
            k1.notices.iter().any(|n| n.rule_id == "ARC_021"),
            "ARC_021 bekleniyor"
        );
    }

    #[test]
    fn arc_030_fires_for_newline_inside_quoted_value() {
        // A line ending inside quotes becomes part of the field value; the spec forbids this.
        let zip = zip_with_files(&[
            ("agency.txt",     "agency_id,agency_name,agency_url,agency_timezone\n1,\"Test\nTransit\",http://x.com,UTC\n".as_bytes()),
            ("stops.txt",      b"stop_id,stop_name,stop_lat,stop_lon\nS1,Durak,41.0,29.0\n"),
            ("routes.txt",     b"route_id,agency_id,route_short_name,route_type\nR1,1,101,3\n"),
            ("trips.txt",      b"route_id,service_id,trip_id\nR1,SVC1,T1\n"),
            ("stop_times.txt", b"trip_id,arrival_time,departure_time,stop_id,stop_sequence\nT1,08:00:00,08:00:00,S1,1\n"),
            ("calendar.txt",   b"service_id,monday,tuesday,wednesday,thursday,friday,saturday,sunday,start_date,end_date\nSVC1,1,1,1,1,1,0,0,20240101,20241231\n"),
        ]);
        let k1 = parse(&zip).unwrap();
        assert!(
            k1.notices.iter().any(|n| n.rule_id == "ARC_030"),
            "ARC_030 bekleniyor"
        );
        // ARC_021 deliberately exempts this character; the two rules must not overlap.
        assert!(
            !k1.notices.iter().any(|n| n.rule_id == "ARC_021"),
            "ARC_021 tetiklenmemeli"
        );
    }

    #[test]
    fn arc_032_detects_real_markup() {
        // From the wild: 1,176 stop names in mdb-1924 had this form.
        assert_eq!(
            arc032_markup("[KMB+CTB] HIU TSUI STREET/<BR>HIU TSUI STREET, SIU SAI WAN ROAD"),
            Some("<BR>".to_string())
        );
        assert_eq!(
            arc032_markup("Merkez <b>Durağı</b>"),
            Some("<b>".to_string())
        );
        assert_eq!(
            arc032_markup("Kapanış </p> etiketi"),
            Some("</p>".to_string())
        );
        assert_eq!(
            arc032_markup("<span class=\"x\">A</span>"),
            Some("<span class=\"x\">".to_string())
        );
        assert_eq!(
            arc032_markup("A <!-- yorum --> B"),
            Some("<!--".to_string())
        );
        assert_eq!(arc032_markup("Ana&nbsp;Cadde"), Some("&nbsp;".to_string()));
        assert_eq!(arc032_markup("Bir &amp; iki"), Some("&amp;".to_string()));
        assert_eq!(
            arc032_markup("Kod &#39;tırnak&#39;"),
            Some("&#39;".to_string())
        );
        assert_eq!(
            arc032_markup("Onaltılık &#x27; kaçış"),
            Some("&#x27;".to_string())
        );
    }

    /// issue #79: the list expanded from 26 names to ALL OF HTML5. The auditor requested five
    /// cases: a tag already in the list · a valid tag NOT in the old list · an HTML comment · a
    /// supported escape · plain text containing `<`/`>`/`&`.
    /// issue #79 — entity/escape side: the list expanded from six names to ALL OF HTML5 (2,125).
    /// issue #79 (reopened) — `svg` and `math` are foreign-content root elements.
    #[test]
    fn arc_032_covers_foreign_content_roots() {
        assert!(arc032_markup("<svg>x</svg>").is_some(), "<svg> yakalanmalı");
        assert!(
            arc032_markup("<math>x</math>").is_some(),
            "<math> yakalanmalı"
        );
        assert!(arc032_markup("Durak </svg>").is_some(), "kapanış biçimi de");
        assert_eq!(arc032_markup("<TBD>"), None, "uydurma ad HTML DEĞİLDİR");
    }

    #[test]
    fn arc_032_covers_the_html5_named_entities() {
        // Reported counterexamples — all were ABSENT from the old list.
        for v in [
            "© 2026 &copy; Metro",
            "Metro&reg; hattı",
            "5&euro; bilet",
            "A&mdash;B",
        ] {
            assert!(arc032_markup(v).is_some(), "kaçış dizisi yakalanmalı: {v}");
        }
        // Numeric forms (already supported; regression protection).
        assert_eq!(arc032_markup("&#8212; tire"), Some("&#8212;".to_string()));
        assert_eq!(arc032_markup("&#x2014; tire"), Some("&#x2014;".to_string()));
        // 🔴 FALSE-POSITIVE GUARD: the `&` + word + `;` pattern MUST NOT be allowed freely.
        // This value was measured in the corpus, and `Tire` is NOT an entity name.
        assert_eq!(arc032_markup("K.A.Wheel&Tire;"), None);
        assert_eq!(arc032_markup("A & B"), None);
        assert_eq!(
            arc032_markup("Kadıköy&İskele;"),
            None,
            "ASCII olmayan ad entity değildir"
        );
        // CASE-SENSITIVE: `&COPY;` and `&Copy;` are separate entries in the list.
        assert!(arc032_markup("x &COPY; y").is_some());
        assert_eq!(
            arc032_markup("x &cOpY; y"),
            None,
            "listede olmayan yazım entity değildir"
        );
        // LEGACY form without a semicolon (HTML5 recognises these without `;`).
        assert_eq!(arc032_markup("2026 &copy Metro"), Some("&copy".to_string()));
        assert!(arc032_markup("A &reg B").is_some());
        // ⚠️ The exception does NOT APPLY to all names: `&mdash` (without a semicolon) was NOT in
        // the old list.
        assert_eq!(
            arc032_markup("A &mdash B"),
            None,
            "yalnız kaynağın izin verdiği adlar"
        );
        // False-positive guards — the `&` + word pattern is NOT free-form.
        assert_eq!(arc032_markup("Wheel&Tire servisi"), None);
        assert_eq!(arc032_markup("AT&T"), None);
        assert_eq!(arc032_markup("Rock&Roll"), None);
    }

    #[test]
    fn arc_032_covers_the_whole_html_element_set() {
        // Was in the old list — must not regress.
        assert_eq!(
            arc032_markup("Merkez <b>Durağı</b>"),
            Some("<b>".to_string())
        );
        // NOT in the old list: the rule says "HTML tag", and these are HTML tags too.
        for tag in [
            "script", "section", "iframe", "style", "video", "article", "h4",
        ] {
            let v = format!("Durak <{tag}>x</{tag}>");
            assert!(arc032_markup(&v).is_some(), "<{tag}> yakalanmalı");
        }
        // Names that are NOT HTML elements must remain silent.
        assert_eq!(arc032_markup("Durak <bilinmeyen>x</bilinmeyen>"), None);
        // 🔴 A CUSTOM ELEMENT (hyphenated name) is NOT `svg` — the first version of this test
        // caught exactly this false match: name extraction stopped at the hyphen and `<svg-yok>`
        // counted as "svg".
        assert_eq!(arc032_markup("<svg-yok>"), None, "tireli ad özel elemandır");
        assert_eq!(arc032_markup("<article-list>"), None);
        // Genuine `svg`/`math` and attribute-bearing forms are still caught.
        assert!(arc032_markup("<svg width=\"3\">").is_some());
        assert!(arc032_markup("<math>").is_some());
        assert_eq!(
            arc032_markup("Not <!-- gizli --> son"),
            Some("<!--".to_string())
        );
        assert_eq!(
            arc032_markup("Kadıköy&nbsp;İskele"),
            Some("&nbsp;".to_string())
        );
        // Plain text: a fabricated name is NOT an HTML tag — the list is deliberately closed.
        assert_eq!(arc032_markup("<Bilinmiyor>"), None);
        assert_eq!(arc032_markup("<TBD> hattı"), None);
        assert_eq!(arc032_markup("A & B < C > D"), None);
    }

    #[test]
    fn arc_032_silent_for_legitimate_text() {
        // These three values falsely matched a LOOSE pattern during measurement (the source of
        // 1,991 spurious findings). The closed list must allow them — regression is locked here.
        assert_eq!(arc032_markup("K.A.Wheel&Tire;"), None); // unknown "entity"
        assert_eq!(arc032_markup("St Sauvant >< St Césaire"), None); // not a tag
        assert_eq!(arc032_markup("La Clisse >< Luchat >< Pisany"), None);
        // Also: comparison signs, an unknown tag name, and an unclosed tag.
        assert_eq!(arc032_markup("a < b ve c > d"), None);
        assert_eq!(arc032_markup("<durak>"), None); // name not in the list
        assert_eq!(arc032_markup("3 <br sonra"), None); // no `>`
        assert_eq!(arc032_markup("Fiyat < 5 TL > indirim"), None);
        assert_eq!(arc032_markup("Şişli Durağı"), None);
        assert_eq!(arc032_markup("東京駅"), None);
    }

    #[test]
    fn arc_032_emits_one_notice_per_file_with_columns() {
        let zip = zip_with_files(&[
            ("agency.txt", b"agency_id,agency_name,agency_url,agency_timezone\nA1,Test,https://a.com,Europe/Istanbul\n" as &[u8]),
            ("stops.txt",  b"stop_id,stop_name,stop_lat,stop_lon\nS1,Ana<br>Durak,41.0,29.0\nS2,Yan&nbsp;Durak,41.1,29.1\n"),
            ("routes.txt", b"route_id,route_short_name,route_type\nR1,1,3\n"),
            ("trips.txt",  b"route_id,service_id,trip_id\nR1,SVC1,T1\n"),
            ("stop_times.txt", b"trip_id,arrival_time,departure_time,stop_id,stop_sequence\nT1,08:00:00,08:00:00,S1,1\n"),
            ("calendar.txt", b"service_id,monday,tuesday,wednesday,thursday,friday,saturday,sunday,start_date,end_date\nSVC1,1,1,1,1,1,0,0,20240101,20241231\n"),
        ]);
        let k1 = parse(&zip).unwrap();
        let hits: Vec<_> = k1
            .notices
            .iter()
            .filter(|n| n.rule_id == "ARC_032")
            .collect();
        // Two malformed rows, but ONE emit per FILE — DQ_016/STM_050 burst prevention.
        assert_eq!(
            hits.len(),
            1,
            "dosya başına tek notice bekleniyor: {hits:?}"
        );
        assert_eq!(hits[0].field.as_deref(), Some("stop_name"));
        assert!(
            hits[0].message.contains("2 satırda"),
            "satır sayısı: {}",
            hits[0].message
        );
        assert!(
            hits[0].message.contains("<br>"),
            "örnek işaret: {}",
            hits[0].message
        );
    }

    #[test]
    fn arc_032_defers_to_stp_023_on_tts_stop_name() {
        // STP_023 reports ANY `<`/`>` marker in tts_stop_name and is broader; emitting both would
        // report the same phenomenon twice.
        let mut acc = Arc032Acc::default();
        let headers = vec!["stop_id".to_string(), "tts_stop_name".to_string()];
        acc.observe(2, ["S1", "Ana <b>Durak</b>"].into_iter(), &headers);
        assert!(
            acc.summary("stops.txt").is_none(),
            "tts_stop_name ARC_032'ye girmemeli"
        );
    }

    #[test]
    fn arc_030_fires_for_tab_inside_value() {
        // The spec also forbids tabs: "must not contain tabs, carriage returns or new lines".
        assert_eq!(arc030_bad_whitespace(["ab\tcd"].into_iter()), Some(9));
        assert_eq!(arc030_bad_whitespace(["ab\rcd"].into_iter()), Some(13));
        assert_eq!(arc030_bad_whitespace(["ab\ncd"].into_iter()), Some(10));
    }

    #[test]
    fn arc_030_silent_for_normal_and_unicode_values() {
        // Whitespace, Turkish/Japanese text, and punctuation are not problems.
        assert_eq!(
            arc030_bad_whitespace(["Şişli Durağı", "東京駅", "a, b"].into_iter()),
            None
        );
        let k1 = parse(&minimal_gtfs_zip()).unwrap();
        assert!(
            !k1.notices.iter().any(|n| n.rule_id == "ARC_030"),
            "ARC_030 tetiklenmemeli"
        );
    }

    #[test]
    fn arc_021_silent_for_ascii_only() {
        let zip = minimal_gtfs_zip(); // ASCII only
        let k1 = parse(&zip).unwrap();
        assert!(
            !k1.notices.iter().any(|n| n.rule_id == "ARC_021"),
            "ARC_021 tetiklenmemeli"
        );
    }

    #[test]
    fn loc_001_fires_for_invalid_geometry_type() {
        // ZIP together with locations.geojson
        let geojson = br#"{"type":"FeatureCollection","features":[{"type":"Feature","geometry":{"type":"Point","coordinates":[29.0,41.0]},"properties":{}}]}"#;
        let zip = zip_with_files(&[
            ("agency.txt",     b"agency_id,agency_name,agency_url,agency_timezone\n1,Test,http://x.com,UTC\n"),
            ("stops.txt",      b"stop_id,stop_name,stop_lat,stop_lon\nS1,Stop1,41.0,29.0\n"),
            ("routes.txt",     b"route_id,agency_id,route_short_name,route_type\nR1,1,101,3\n"),
            ("trips.txt",      b"route_id,service_id,trip_id\nR1,SVC1,T1\n"),
            ("stop_times.txt", b"trip_id,arrival_time,departure_time,stop_id,stop_sequence\nT1,08:00:00,08:00:00,S1,1\n"),
            ("calendar.txt",   b"service_id,monday,tuesday,wednesday,thursday,friday,saturday,sunday,start_date,end_date\nSVC1,1,1,1,1,1,0,0,20240101,20241231\n"),
            ("locations.geojson", geojson),
        ]);
        let k1 = parse(&zip).unwrap();
        assert!(
            k1.notices.iter().any(|n| n.rule_id == "LOC_001"),
            "LOC_001 bekleniyor"
        );
    }

    #[test]
    fn loc_001_silent_for_valid_polygon() {
        let geojson = br#"{"type":"FeatureCollection","features":[{"type":"Feature","geometry":{"type":"Polygon","coordinates":[[[29.0,41.0],[29.1,41.0],[29.1,41.1],[29.0,41.1],[29.0,41.0]]]},"properties":{}}]}"#;
        let zip = zip_with_files(&[
            ("agency.txt",     b"agency_id,agency_name,agency_url,agency_timezone\n1,Test,http://x.com,UTC\n"),
            ("stops.txt",      b"stop_id,stop_name,stop_lat,stop_lon\nS1,Stop1,41.0,29.0\n"),
            ("routes.txt",     b"route_id,agency_id,route_short_name,route_type\nR1,1,101,3\n"),
            ("trips.txt",      b"route_id,service_id,trip_id\nR1,SVC1,T1\n"),
            ("stop_times.txt", b"trip_id,arrival_time,departure_time,stop_id,stop_sequence\nT1,08:00:00,08:00:00,S1,1\n"),
            ("calendar.txt",   b"service_id,monday,tuesday,wednesday,thursday,friday,saturday,sunday,start_date,end_date\nSVC1,1,1,1,1,1,0,0,20240101,20241231\n"),
            ("locations.geojson", geojson),
        ]);
        let k1 = parse(&zip).unwrap();
        assert!(
            !k1.notices.iter().any(|n| n.rule_id == "LOC_001"),
            "LOC_001 tetiklenmemeli"
        );
    }

    /// Measurement from corpus run 16 (`tdg-81648`..`tdg-81652`): the file was NOT JSON at all.
    /// It used to produce `LOC_001`, whose localized template said "invalid geometry type";
    /// the parser failed on the first character before reading any geometry.
    #[test]
    fn loc_012_fires_for_a_document_that_is_not_json_and_loc_001_stays_silent() {
        let zip = zip_with_files(&[
            ("agency.txt",     b"agency_id,agency_name,agency_url,agency_timezone\n1,Test,http://x.com,UTC\n"),
            ("stops.txt",      b"stop_id,stop_name,stop_lat,stop_lon\nS1,Stop1,41.0,29.0\n"),
            ("routes.txt",     b"route_id,agency_id,route_short_name,route_type\nR1,1,101,3\n"),
            ("trips.txt",      b"route_id,service_id,trip_id\nR1,SVC1,T1\n"),
            ("stop_times.txt", b"trip_id,arrival_time,departure_time,stop_id,stop_sequence\nT1,08:00:00,08:00:00,S1,1\n"),
            ("calendar.txt",   b"service_id,monday,tuesday,wednesday,thursday,friday,saturday,sunday,start_date,end_date\nSVC1,1,1,1,1,1,0,0,20240101,20241231\n"),
            ("locations.geojson", b"<html>not json at all</html>"),
        ]);
        let k1 = parse(&zip).unwrap();
        assert!(
            k1.notices.iter().any(|n| n.rule_id == "LOC_012"),
            "JSON olmayan belge LOC_012 üretmeli"
        );
        assert!(
            !k1.notices.iter().any(|n| n.rule_id == "LOC_001"),
            "geometri hiç okunmadı — LOC_001 konuşmamalı"
        );
    }

    /// A wrong root type is still a document-level issue, not a geometry-level issue.
    #[test]
    fn loc_012_fires_when_the_root_type_is_not_a_feature_collection() {
        let zip = zip_with_files(&[
            ("agency.txt",     b"agency_id,agency_name,agency_url,agency_timezone\n1,Test,http://x.com,UTC\n"),
            ("stops.txt",      b"stop_id,stop_name,stop_lat,stop_lon\nS1,Stop1,41.0,29.0\n"),
            ("routes.txt",     b"route_id,agency_id,route_short_name,route_type\nR1,1,101,3\n"),
            ("trips.txt",      b"route_id,service_id,trip_id\nR1,SVC1,T1\n"),
            ("stop_times.txt", b"trip_id,arrival_time,departure_time,stop_id,stop_sequence\nT1,08:00:00,08:00:00,S1,1\n"),
            ("calendar.txt",   b"service_id,monday,tuesday,wednesday,thursday,friday,saturday,sunday,start_date,end_date\nSVC1,1,1,1,1,1,0,0,20240101,20241231\n"),
            ("locations.geojson", br#"{"type":"Feature","geometry":null}"#),
        ]);
        let k1 = parse(&zip).unwrap();
        let n = k1
            .notices
            .iter()
            .find(|n| n.rule_id == "LOC_012")
            .expect("kök tip yanlışsa LOC_012 bekleniyor");
        assert_eq!(n.field.as_deref(), Some("type"));
        assert_eq!(n.observed_value.as_deref(), Some("Feature"));
    }

    /// LOC_006 fixture: creates a locations.geojson with the supplied ring.
    fn zip_with_polygon(ring: &str) -> Vec<u8> {
        let geojson = format!(
            r#"{{"type":"FeatureCollection","features":[{{"type":"Feature","id":"z1","geometry":{{"type":"Polygon","coordinates":[{ring}]}},"properties":{{}}}}]}}"#
        );
        zip_with_files(&[
            ("agency.txt",     b"agency_id,agency_name,agency_url,agency_timezone\n1,Test,http://x.com,UTC\n"),
            ("stops.txt",      b"stop_id,stop_name,stop_lat,stop_lon\nS1,Stop1,41.0,29.0\n"),
            ("routes.txt",     b"route_id,agency_id,route_short_name,route_type\nR1,1,101,3\n"),
            ("trips.txt",      b"route_id,service_id,trip_id\nR1,SVC1,T1\n"),
            ("stop_times.txt", b"trip_id,arrival_time,departure_time,stop_id,stop_sequence\nT1,08:00:00,08:00:00,S1,1\n"),
            ("calendar.txt",   b"service_id,monday,tuesday,wednesday,thursday,friday,saturday,sunday,start_date,end_date\nSVC1,1,1,1,1,1,0,0,20240101,20241231\n"),
            ("locations.geojson", geojson.as_bytes()),
        ])
    }

    #[test]
    fn loc_006_silent_for_thin_diagonal_zone_with_large_bounding_box() {
        // Thin diagonal strip: bbox ~4800 km² (the old measurement would fire), actual area ~186 km².
        // Service zones are not convex; this distinction caused the rule's false positives.
        let ring = "[[29.0,41.0],[30.0,41.5],[30.0,41.52],[29.0,41.02],[29.0,41.0]]";
        let k1 = parse(&zip_with_polygon(ring)).unwrap();
        assert!(
            !k1.notices.iter().any(|n| n.rule_id == "LOC_006"),
            "gerçek alanı eşiğin altında olan bölge LOC_006 üretmemeli"
        );
    }

    #[test]
    fn loc_006_fires_for_genuinely_large_zone() {
        // ~1.0° longitude × 0.1° latitude, ≈929 km² at the 41st parallel → above the (500) threshold.
        let ring = "[[29.0,41.0],[30.0,41.0],[30.0,41.1],[29.0,41.1],[29.0,41.0]]";
        let k1 = parse(&zip_with_polygon(ring)).unwrap();
        assert!(
            k1.notices.iter().any(|n| n.rule_id == "LOC_006"),
            "gerçekten büyük bölge LOC_006 üretmeli"
        );
    }

    #[test]
    fn loc_006_subtracts_holes_from_the_outer_ring() {
        // Outer ring ~929 km², with a ~836 km² hole inside → net ~93 km², below the threshold.
        let ring = "[[29.0,41.0],[30.0,41.0],[30.0,41.1],[29.0,41.1],[29.0,41.0]],\
                    [[29.05,41.005],[29.95,41.005],[29.95,41.095],[29.05,41.095],[29.05,41.005]]";
        let k1 = parse(&zip_with_polygon(ring)).unwrap();
        assert!(
            !k1.notices.iter().any(|n| n.rule_id == "LOC_006"),
            "delikler dış ringden düşülmeli"
        );
    }

    // ── ARC_036: ZIP stream-view consistency ───────────────────────────────────
    //
    // Unit tests target the PURE function because the real risk is in byte interpretation,
    // not the end-to-end flow: if the rule is loosened, it would flag 40% of the corpus
    // (feeds that use descriptors LEGITIMATELY). `mesru_akis_zipinde_susar` is that gate.

    /// Produces a single-entry synthetic local header plus an optional data descriptor.
    /// `descriptor`: `None` → writes nothing, followed by another local header.
    fn synthetic_entry(
        flag: u16,
        local_csize: u32,
        local_usize: u32,
        data: &[u8],
        descriptor: Option<(u32, u32, u32)>,
        descriptor_signature: bool,
    ) -> Vec<u8> {
        let name = b"agency.txt";
        let mut b = Vec::new();
        b.extend_from_slice(&LFH_SIGNATURE.to_le_bytes());
        b.extend_from_slice(&20u16.to_le_bytes()); // version
        b.extend_from_slice(&flag.to_le_bytes());
        b.extend_from_slice(&0u16.to_le_bytes()); // method: stored
        b.extend_from_slice(&0u16.to_le_bytes()); // time
        b.extend_from_slice(&0u16.to_le_bytes()); // date
        b.extend_from_slice(&0u32.to_le_bytes()); // crc (local field)
        b.extend_from_slice(&local_csize.to_le_bytes());
        b.extend_from_slice(&local_usize.to_le_bytes());
        b.extend_from_slice(&(name.len() as u16).to_le_bytes());
        b.extend_from_slice(&0u16.to_le_bytes()); // extra len
        b.extend_from_slice(name);
        b.extend_from_slice(data);
        match descriptor {
            Some((crc, csize, usize_)) => {
                if descriptor_signature {
                    b.extend_from_slice(&DATA_DESCRIPTOR_SIGNATURE.to_le_bytes());
                }
                b.extend_from_slice(&crc.to_le_bytes());
                b.extend_from_slice(&csize.to_le_bytes());
                b.extend_from_slice(&usize_.to_le_bytes());
            }
            // No descriptor: the `tdg-81618` pattern — the next entry's header follows immediately.
            None => b.extend_from_slice(&LFH_SIGNATURE.to_le_bytes()),
        }
        b
    }

    const DATA: &[u8] = b"agency_id,agency_name\n1,Test\n";

    /// A RULE-COMPLIANT streaming ZIP used by 40% of the corpus: local sizes are 0,
    /// the descriptor is present and matches the central directory exactly. Firing here is catastrophic.
    #[test]
    fn arc036_mesru_akis_zipinde_susar() {
        let crc = 0xDEAD_BEEF;
        let n = DATA.len() as u32;
        for with_sig in [true, false] {
            let bytes = synthetic_entry(
                FLAG_DATA_DESCRIPTOR,
                0,
                0,
                DATA,
                Some((crc, n, n)),
                with_sig,
            );
            let entries = [(0u64, u64::from(n), u64::from(n), crc)];
            assert_eq!(
                count_inconsistent_stream_headers(&bytes, &entries),
                0,
                "meşru akış zip'i (descriptor imzası: {with_sig}) bulgu üretmemeli",
            );
        }
    }

    /// `tdg-81618` pattern: flag set, local sizes populated, no descriptor record.
    #[test]
    fn arc036_descriptor_yoksa_ateslenir() {
        let crc = 0xDEAD_BEEF;
        let n = DATA.len() as u32;
        let bytes = synthetic_entry(FLAG_DATA_DESCRIPTOR, n, n, DATA, None, false);
        let entries = [(0u64, u64::from(n), u64::from(n), crc)];
        assert_eq!(count_inconsistent_stream_headers(&bytes, &entries), 1);
    }

    /// `mdb-2607` pattern: descriptor PRESENT, crc/csize correct, but `usize` is written as zero.
    /// A narrow "is there a descriptor?" check would MISS this.
    #[test]
    fn arc036_descriptor_celisirse_ateslenir() {
        let crc = 0xDEAD_BEEF;
        let n = DATA.len() as u32;
        let bytes = synthetic_entry(FLAG_DATA_DESCRIPTOR, 0, 0, DATA, Some((crc, n, 0)), true);
        let entries = [(0u64, u64::from(n), u64::from(n), crc)];
        assert_eq!(count_inconsistent_stream_headers(&bytes, &entries), 1);
    }

    /// If the flag is not set, the entry is not inspected; an entry using the ZIP64 escape is SKIPPED
    /// (the real size is in the extra field and cannot be read here — we do not blame the unreadable).
    #[test]
    fn arc036_bayraksiz_ve_zip64_girdiler_atlanir() {
        let crc = 0xDEAD_BEEF;
        let n = DATA.len() as u32;
        let no_flag = synthetic_entry(0, n, n, DATA, None, false);
        assert_eq!(
            count_inconsistent_stream_headers(&no_flag, &[(0, u64::from(n), u64::from(n), crc)]),
            0,
        );
        let zip64 = synthetic_entry(
            FLAG_DATA_DESCRIPTOR,
            ZIP64_SENTINEL,
            ZIP64_SENTINEL,
            DATA,
            None,
            false,
        );
        assert_eq!(
            count_inconsistent_stream_headers(&zip64, &[(0, u64::from(n), u64::from(n), crc)]),
            0,
        );
    }

    /// End to end: the normal feed is silent, while the same feed with only its flag bits flipped
    /// produces ONE finding (feed-level dedup) and carries the count in `observed_value`.
    #[test]
    fn arc036_uctan_uca_yalniz_bayrak_biti_cevrilince_atesler() {
        let clean = minimal_gtfs_zip();
        let r = parse(&clean).unwrap();
        assert!(
            !r.notices.iter().any(|n| n.rule_id == "ARC_036"),
            "sağlam arşivde ARC_036 ateşlememeli",
        );

        let mut flipped = clean.clone();
        let starts: Vec<u64> = {
            let mut a = zip::ZipArchive::new(std::io::Cursor::new(&clean[..])).unwrap();
            (0..a.len())
                .map(|i| a.by_index_raw(i).unwrap().header_start())
                .collect()
        };
        for s in &starts {
            let off = *s as usize + 6; // general purpose bit flag
            let flag = u16::from_le_bytes([flipped[off], flipped[off + 1]]);
            flipped[off..off + 2].copy_from_slice(&(flag | FLAG_DATA_DESCRIPTOR).to_le_bytes());
        }
        // The archive should still open because the central directory is intact; the rule measures this.
        let r = parse(&flipped).unwrap();
        let hits: Vec<_> = r
            .notices
            .iter()
            .filter(|n| n.rule_id == "ARC_036")
            .collect();
        assert_eq!(hits.len(), 1, "feed düzeyinde tek bulgu bekleniyordu");
        assert_eq!(
            hits[0].observed_value.as_deref(),
            Some(starts.len().to_string().as_str()),
            "etkilenen girdi sayısı observed_value'da taşınmalı",
        );
    }
}
/// `required_fields` (the basis for ARC_025) must match the spec's `Required` columns EXACTLY.
///
/// ARC_025 is Critical·Spec, so it blocks R1. Listing a CONDITIONAL field takes a valid
/// feed out of publication; omitting a truly Required field leaves the rule unenforced.
/// On 2026-08-02, `transfers.txt` had BOTH:
/// `from_stop_id`/`to_stop_id` were listed while conditional (a hidden false positive),
/// while `transfer_type` was not listed even though unconditionally Required (unenforced).
///
/// Source: `spec-audit/spec_fields.json` → `presence == "Required"`.
#[test]
fn required_columns_match_the_specification() {
    let path =
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../spec-audit/spec_fields.json");
    let doc: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(&path).unwrap()).unwrap();
    let files = doc["files"].as_object().unwrap();
    assert!(
        files.len() >= 25,
        "spec_fields.json çok küçük — yeniden üretin"
    );

    let mut problems: Vec<String> = Vec::new();
    for (fname, entry) in files {
        let spec_required: std::collections::BTreeSet<&str> = entry["fields"]
            .as_array()
            .unwrap()
            .iter()
            .filter(|f| f["presence"].as_str() == Some("Required"))
            .filter_map(|f| f["name"].as_str())
            .collect();
        let ours: std::collections::BTreeSet<&str> =
            required_fields(fname).iter().copied().collect();
        for miss in spec_required.difference(&ours) {
            problems.push(format!(
                    "{fname}: '{miss}' spec'te Required ama required_fields'ta YOK → ARC_025 denetlemiyor"
                ));
        }
        for extra in ours.difference(&spec_required) {
            problems.push(format!(
                    "{fname}: '{extra}' required_fields'ta ama spec'te Required DEĞİL →                      geçerli feed ARC_025 (Kritik) alabilir"
                ));
        }
    }
    assert!(
            problems.is_empty(),
            "required_fields spec ile uyuşmuyor ({} sorun):\n{}\n\n             Spec değiştiyse önce `python3 spec-audit/extract_fields.py` çalıştırın.",
            problems.len(), problems.join("\n"),
        );
}

#[test]
fn malformed_eol_detects_bare_and_repeated_cr() {
    assert!(has_malformed_eol(b"a\rb\n"));
    assert!(has_malformed_eol(b"a\r\r\nb"));
    assert!(!has_malformed_eol(b"a\r\nb\n"));
    assert!(!has_malformed_eol(b"a\n\nb\n"));
}
