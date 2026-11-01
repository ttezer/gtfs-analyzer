//! MUTATION evidence on real feeds (issue #127).
//!
//! Question: is a rule that never fires in the corpus "correct but naturally silent", or
//! does its predicate have a blind spot? A fixture cannot distinguish these — it proves that
//! a rule can fire, not that it behaves correctly on the real reader path. (In #75,
//! `ARC_022`'s fixture was green while real large feeds used the streaming path and the
//! rule stayed silent; 8.4M rows passed silently in the Swiss feed.)
//!
//! Method: TAKE a real corpus feed, INJECT the smallest defect, run mutated and unmutated
//! versions with the same `config` and `TODAY`, and compare the **rule-ID set DELTA**.
//!
//! ⚠️ The delta claim is NOT "only the target rule". Injecting a row into a real feed may
//! affect the entity map, feed-level summary rules, and cascade suppression. The criterion:
//! the target rule MUST appear in the delta, and every other rule ID in the delta must be
//! EXPLAINED in the test. Forbidding them would force mutations into cases without cascades
//! and weaken the test.
//!
//! Feeds are NOT in the repository (`notgit/` is outside git), so tests are `#[ignore]`.
//! To run:
//!   python3 spec-audit/corpus-evidence/verify_corpus.py     # downloads + verifies sha256
//!   cargo test -p gtfs-pipeline --test real_feed_mutation -- --ignored
//!
//! Identity chain: `verify_corpus.py` verifies **sha256** while downloading; this test
//! additionally checks the manifest's **byte length**. Calculating sha256 inside the test
//! would require a new crate dependency (`sha2`), and was omitted because the download
//! path already verifies the hash.

use std::collections::BTreeSet;
use std::io::{Read as _, Write as _};
use std::path::{Path, PathBuf};

use gtfs_pipeline::{validate_bytes, FatalCode, ValidateResult, ValidatorConfig};

/// Fixed date — keep calendar rules date-relative and deterministic.
const TODAY: u32 = 20_260_515;

fn repo_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()
        .expect("repo kökü bulunamadı")
}

/// Byte length recorded in the manifest.
fn manifest_bytes(feed: &str) -> u64 {
    let path = repo_root().join("spec-audit/corpus-evidence/corpus_manifest.csv");
    let text = std::fs::read_to_string(&path).expect("corpus_manifest.csv okunamadı");
    let mut lines = text.lines();
    let header: Vec<&str> = lines.next().expect("manifest boş").split(',').collect();
    let bi = header
        .iter()
        .position(|h| *h == "bytes")
        .expect("manifest'te 'bytes' sütunu yok");
    for line in lines {
        let cols: Vec<&str> = line.split(',').collect();
        if cols.first() == Some(&feed) {
            return cols[bi].parse().expect("bytes sayı değil");
        }
    }
    panic!("{feed} manifest'te yok");
}

/// Reads a corpus archive; if absent, fails while explaining HOW to obtain it.
fn corpus_zip(feed: &str) -> Vec<u8> {
    let path = repo_root().join(format!("notgit/corpus/zips/{feed}.zip"));
    let bytes = std::fs::read(&path).unwrap_or_else(|e| {
        panic!(
            "{} okunamadı ({e}).\n\
             Korpus repoda değil. Getirmek için:\n  \
             python3 spec-audit/corpus-evidence/verify_corpus.py",
            path.display()
        )
    });
    let expected = manifest_bytes(feed);
    assert_eq!(
        bytes.len() as u64,
        expected,
        "{feed} bayt uzunluğu manifest ile uyuşmuyor ({} vs {expected}). \
         Yayıncı feed'i tazelemiş olabilir; kanıt olarak KULLANILAMAZ — \
         manifest'i güncelleyip ölçümü yeniden yapın.",
        bytes.len()
    );
    bytes
}

/// Rewrites one archive member and copies the remaining members unchanged.
fn rewrite_member(zip: &[u8], target: &str, f: impl FnOnce(String) -> String) -> Vec<u8> {
    let mut archive = zip::ZipArchive::new(std::io::Cursor::new(zip)).expect("arşiv açılamadı");
    let names: Vec<String> = archive
        .file_names()
        .map(|n| n.expect("üye adı çözülemedi").into_owned())
        .collect();
    let mut out = zip::ZipWriter::new(std::io::Cursor::new(Vec::new()));
    let opts = zip::write::SimpleFileOptions::default();

    let mut hit = false;
    let mut mutate = Some(f);
    for name in &names {
        let mut entry = archive.by_name(name).expect("üye okunamadı");
        let mut buf = Vec::new();
        entry.read_to_end(&mut buf).expect("üye içeriği okunamadı");
        drop(entry);

        out.start_file(name.as_str(), opts).expect("üye yazılamadı");
        if Path::new(name).file_name().and_then(|s| s.to_str()) == Some(target) {
            hit = true;
            let text = String::from_utf8(buf).expect("hedef dosya UTF-8 değil");
            let m = mutate.take().expect("hedef dosya birden fazla kez eşleşti");
            out.write_all(m(text).as_bytes())
                .expect("mutasyon yazılamadı");
        } else {
            out.write_all(&buf).expect("üye kopyalanamadı");
        }
    }
    assert!(hit, "{target} arşivde bulunamadı");
    out.finish().expect("arşiv kapanmadı").into_inner()
}

/// Runs the feed and returns the generated rule-ID set.
fn rule_ids(zip: &[u8]) -> BTreeSet<String> {
    match validate_bytes(zip, &ValidatorConfig::default(), TODAY) {
        ValidateResult::Ok(vr) => vr.notices.iter().map(|n| n.rule_id.to_string()).collect(),
        ValidateResult::Fatal(e) => panic!("Fatal: {:?} — {}", e.code, e.message),
    }
}

/// Rules that a file's DQ_016 root notice DECLARES it suppressed.
///
/// Inferring suppression from an empty delta is weak — the delta would also be empty if the
/// mutation never ran. K7 writes `suppressed_derivative_rules` to the root notice; that is
/// the criterion.
fn suppressed_rules_for(zip: &[u8], file: &str) -> Vec<String> {
    match validate_bytes(zip, &ValidatorConfig::default(), TODAY) {
        ValidateResult::Ok(vr) => vr
            .notices
            .iter()
            .filter(|n| n.rule_id == "DQ_016" && n.file.as_deref() == Some(file))
            .filter_map(|n| n.details.as_ref()?.get("suppressed_derivative_rules"))
            .flat_map(|v| v.split(',').map(|s| s.trim().to_string()))
            .filter(|s| !s.is_empty())
            .collect(),
        ValidateResult::Fatal(e) => panic!("Fatal: {:?}", e.code),
    }
}

/// Rule IDs ADDED and REMOVED by the mutation.
fn delta(before: &BTreeSet<String>, after: &BTreeSet<String>) -> (Vec<String>, Vec<String>) {
    (
        after.difference(before).cloned().collect(),
        before.difference(after).cloned().collect(),
    )
}

/// Changes a column's value in data row `n` (n = 1 → first data row).
fn set_field(text: &str, column: &str, row_ordinal: usize, value: &str) -> String {
    let mut lines: Vec<String> = text
        .lines()
        .map(|l| l.trim_end_matches('\r').to_string())
        .collect();
    let header = header_of(text);
    let ci = header
        .iter()
        .position(|h| h == column)
        .unwrap_or_else(|| panic!("{column} sütunu yok: {header:?}"));
    let li = row_ordinal;
    assert!(li < lines.len(), "{row_ordinal}. veri satırı yok");
    let mut cols = split_csv_line(&lines[li]);
    assert_eq!(cols.len(), header.len(), "alan sayısı başlıkla uyuşmuyor");
    cols[ci] = value.to_string();
    lines[li] = join_csv_fields(&cols);
    lines.join("\n") + "\n"
}

/// Checks the delta: the target MUST APPEAR, and every remaining ID must be EXPLAINED in
/// `explained`.
fn assert_delta(added: &[String], removed: &[String], target: &str, explained: &[&str]) {
    assert!(
        added.iter().any(|r| r == target),
        "{target} mutasyondan sonra ÇIKMADI. Eklenen: {added:?}"
    );
    let unexplained: Vec<&String> = added
        .iter()
        .filter(|r| *r != target && !explained.contains(&r.as_str()))
        .collect();
    assert!(
        unexplained.is_empty(),
        "{target} için AÇIKLANMAMIŞ yan etki: {unexplained:?}\n\
         Bunlar hata olmayabilir (kaskad/özet kuralları) ama teste gerekçesiyle YAZILMALI."
    );
    assert!(
        removed.is_empty(),
        "{target} mutasyonu bulgu KALDIRDI: {removed:?} — bastırma etkisi incelenmeli."
    );
}

/// Returns the first data row (after the header); trims `\r`.
fn first_data_line(text: &str) -> &str {
    text.lines()
        .nth(1)
        .expect("dosyada veri satırı yok")
        .trim_end_matches('\r')
}

/// Appends a line to the text (normalizing the trailing newline).
fn append_line(text: &str, line: &str) -> String {
    let mut s = text.trim_end_matches(['\n', '\r']).to_string();
    s.push('\n');
    s.push_str(line);
    s.push('\n');
    s
}

/// Reads the header row NORMALIZED: trims the BOM and every cell.
///
/// ⚠️ Both are measured requirements, not style. `ch-odv-flex` files start with a UTF-8 BOM
/// (first cell `\u{feff}stop_id`) and the `stop_times.txt` header contains
/// `location_group_id, location_id` — the cell name is `" location_id"`. A raw-comparison
/// helper would report "column missing" for both and fail the test as a PRODUCT BUG.
fn header_of(text: &str) -> Vec<String> {
    let first = text.lines().next().unwrap_or("").trim_end_matches('\r');
    let first = first.strip_prefix('\u{feff}').unwrap_or(first);
    split_csv_line(first)
        .into_iter()
        .map(|c| c.trim().to_string())
        .collect()
}

/// RFC 4180 field splitting. Naive `split(',')` is NOT sufficient, as measured:
/// `service_id` in `mdb-3141`'s `calendar.txt` is `"1,2,3,4,5,6,7"` — six commas
/// inside quotes. Naive splitting would treat the 10-field row as 16 fields and corrupt it.
fn split_csv_line(line: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut cur = String::new();
    let mut in_q = false;
    let mut it = line.chars().peekable();
    while let Some(c) = it.next() {
        match c {
            '"' if in_q && it.peek() == Some(&'"') => {
                cur.push('"');
                it.next();
            }
            '"' => in_q = !in_q,
            ',' if !in_q => out.push(std::mem::take(&mut cur)),
            _ => cur.push(c),
        }
    }
    out.push(cur);
    out
}

/// Joins fields again; quotes a field that contains a comma, quote, or newline.
fn join_csv_fields(fields: &[String]) -> String {
    fields
        .iter()
        .map(|f| {
            if f.contains([',', '"', '\n', '\r']) {
                format!("\"{}\"", f.replace('"', "\"\""))
            } else {
                f.clone()
            }
        })
        .collect::<Vec<_>>()
        .join(",")
}

/// Clears the row's first field (the field count must match the header).
fn blank_first_field(header: &str, line: &str) -> String {
    let n = split_csv_line(header.trim_end_matches('\r')).len();
    let mut cols = split_csv_line(line);
    assert_eq!(cols.len(), n, "alan sayısı başlıkla uyuşmuyor: {line}");
    cols[0] = String::new();
    join_csv_fields(&cols)
}

// ─────────────────────────────────────────────────────────────────────────────
// SHP_005 — shape_dist_traveled must increase with shape_pt_sequence
// ─────────────────────────────────────────────────────────────────────────────

const SHP_FEED: &str = "mdb-2927";

/// Moves the distance of the first shape's LAST point BELOW (or equal to) the previous one.
///
/// ⚠️ The first version set the SECOND point to `first - 1.0` and FAILED: in `mdb-2927`
/// the first point's distance was `0`, so `(0.0-1.0).max(0.0)` was still `0` → it produced
/// EQUALITY rather than a decrease and emitted SHP_028 instead of the expected SHP_005.
/// This was a MUTATION bug, not a rule bug; targeting the last point guarantees a positive
/// distance.
fn break_last_distance(text: &str, equal_instead: bool) -> String {
    let mut lines: Vec<String> = text
        .lines()
        .map(|l| l.trim_end_matches('\r').to_string())
        .collect();
    let header = header_of(text);
    let id_i = header
        .iter()
        .position(|h| h == "shape_id")
        .expect("shape_id yok");
    let d_i = header
        .iter()
        .position(|h| h == "shape_dist_traveled")
        .expect("shape_dist_traveled yok");

    let first_id = split_csv_line(&lines[1])[id_i].clone();
    // Indices of the last two rows of the same shape.
    let idxs: Vec<usize> = lines
        .iter()
        .enumerate()
        .skip(1)
        .filter(|(_, l)| split_csv_line(l).get(id_i) == Some(&first_id))
        .map(|(i, _)| i)
        .collect();
    assert!(idxs.len() >= 2, "ilk shape'in en az iki noktası olmalı");
    let (prev_i, last_i) = (idxs[idxs.len() - 2], idxs[idxs.len() - 1]);

    let prev_d: f64 = split_csv_line(&lines[prev_i])[d_i]
        .parse()
        .expect("önceki mesafe sayı değil");
    assert!(prev_d > 0.0, "mutasyon için önceki mesafe pozitif olmalı");

    let mut cols = split_csv_line(&lines[last_i]);
    cols[d_i] = if equal_instead {
        format!("{prev_d}")
    } else {
        format!("{}", prev_d - 1.0)
    };
    lines[last_i] = join_csv_fields(&cols);
    lines.join("\n") + "\n"
}

#[test]
#[ignore = "gerçek korpus feed'i gerektirir"]
fn shp_005_fires_when_shape_distance_decreases_on_a_real_feed() {
    let zip = corpus_zip(SHP_FEED);
    let before = rule_ids(&zip);
    assert!(
        !before.contains("SHP_005"),
        "{SHP_FEED} mutasyondan ÖNCE zaten SHP_005 üretiyor — kontrol feed'i olamaz"
    );

    let mutated = rewrite_member(&zip, "shapes.txt", |t| break_last_distance(&t, false));
    let after = rule_ids(&mutated);
    let (added, removed) = delta(&before, &after);

    // EXPLAINED side effects (measured, not forbidden):
    //   SHP_024 — "stop-to-shape distance inconsistent with shape_dist_traveled". Reverting
    //             the last point's distance also breaks the distance consistency of its stop.
    //             A DIRECT consequence of the mutation; forbidding the cascade would weaken the test.
    //   SHP_028/029/020 — family of location changes without increasing distance; may appear
    //             in variants that land on equality.
    assert_delta(
        &added,
        &removed,
        "SHP_005",
        &["SHP_024", "SHP_028", "SHP_029", "SHP_020"],
    );
}

#[test]
#[ignore = "gerçek korpus feed'i gerektirir"]
fn shp_005_stays_silent_on_equal_distances_because_three_other_rules_measure_that() {
    // BOUNDARY VALUE. The spec says "must increase", so EQUALITY is also a violation. But
    // adding equality to SHP_005 would make it the FIFTH rule measuring the same fact —
    // the fact is already FULLY PARTITIONED by coordinate behavior:
    //   SHP_023 equal distance + SAME coordinates      (Orta   · Quality   · 38 corpus feeds)
    //   SHP_028 equal distance + DIFFERENT coordinates  (Yuksek · Quality   ·  9 corpus feeds)
    //   SHP_029 equal distance + near coordinates       (Bilgi  · Quality   · 24 corpus feeds)
    //   SHP_020 repeated point                         (Bilgi  · Analytics · 152 corpus feeds)
    // This test ANCHORS current behavior: equality does not produce SHP_005; another rule fires.
    // The remaining question is CLASS, not PREDICATE: a Spec violation is reported only as
    // Quality. It does not affect the publication gate (R1 = Spec ∧ Kritik), because none is
    // Kritik.
    let zip = corpus_zip(SHP_FEED);
    let before = rule_ids(&zip);

    let mutated = rewrite_member(&zip, "shapes.txt", |t| break_last_distance(&t, true));
    let after = rule_ids(&mutated);
    let (added, _) = delta(&before, &after);

    assert!(
        !added.iter().any(|r| r == "SHP_005"),
        "eşit mesafe SHP_005 üretti — davranış değişmiş, kart ve #127 güncellenmeli"
    );
    assert!(
        added
            .iter()
            .any(|r| ["SHP_023", "SHP_028", "SHP_029", "SHP_020"].contains(&r.as_str())),
        "eşit mesafe HİÇBİR kural üretmedi — olgu artık ölçülmüyor. Eklenen: {added:?}"
    );
}

// ─────────────────────────────────────────────────────────────────────────────
// Duplicate rules — CAL_001 · FAR_001 · LVL_001
// ─────────────────────────────────────────────────────────────────────────────

/// Common body: duplicate the first data row EXACTLY → the duplicate rule must fire.
fn duplicate_row_case(feed: &str, file: &str, target: &str, explained: &[&str]) {
    let zip = corpus_zip(feed);
    let before = rule_ids(&zip);
    assert!(
        !before.contains(target),
        "{feed} mutasyondan ÖNCE zaten {target} üretiyor — kontrol feed'i olamaz"
    );

    let mutated = rewrite_member(&zip, file, |t| {
        let dup = first_data_line(&t).to_string();
        append_line(&t, &dup)
    });
    let (added, removed) = delta(&before, &rule_ids(&mutated));
    assert_delta(&added, &removed, target, explained);
}

/// Common body: add two rows with an EMPTY identity → the duplicate rule must not fire
/// (the empty key is skipped with `continue`), but the ROOT required-field rule must fire.
fn empty_key_boundary(feed: &str, file: &str, dup_rule: &str, root_rule: &str) {
    let zip = corpus_zip(feed);
    let before = rule_ids(&zip);

    let mutated = rewrite_member(&zip, file, |t| {
        let header = t.lines().next().expect("başlık yok").to_string();
        let blank = blank_first_field(&header, first_data_line(&t));
        append_line(&append_line(&t, &blank), &blank)
    });
    let (added, _) = delta(&before, &rule_ids(&mutated));

    assert!(
        !added.iter().any(|r| r == dup_rule),
        "boş kimlik {dup_rule} üretti — kod `is_empty()` ile atlıyordu, davranış değişmiş"
    );
    assert!(
        added.iter().any(|r| r == root_rule) || before.contains(root_rule),
        "boş kimlik KÖK kural {root_rule}'i de üretmedi → olgu HİÇ raporlanmıyor, \
         bu gerçek bir boşluktur. Eklenen: {added:?}"
    );
}

#[test]
#[ignore = "gerçek korpus feed'i gerektirir"]
fn cal_001_fires_on_a_duplicated_calendar_row() {
    duplicate_row_case("mdb-3141", "calendar.txt", "CAL_001", &[]);
}

#[test]
#[ignore = "gerçek korpus feed'i gerektirir"]
fn cal_001_ignores_duplicate_empty_service_ids_because_cal_022_reports_the_root() {
    empty_key_boundary("mdb-3141", "calendar.txt", "CAL_001", "CAL_022");
}

#[test]
#[ignore = "gerçek korpus feed'i gerektirir"]
fn far_001_fires_on_a_duplicated_fare_attribute_row() {
    duplicate_row_case("mdb-1996", "fare_attributes.txt", "FAR_001", &[]);
}

#[test]
#[ignore = "gerçek korpus feed'i gerektirir"]
fn far_001_ignores_duplicate_empty_fare_ids_because_far_012_reports_the_root() {
    empty_key_boundary("mdb-1996", "fare_attributes.txt", "FAR_001", "FAR_012");
}

#[test]
#[ignore = "gerçek korpus feed'i gerektirir"]
fn lvl_001_fires_on_a_duplicated_level_row() {
    duplicate_row_case("mdb-3357", "levels.txt", "LVL_001", &[]);
}

#[test]
#[ignore = "gerçek korpus feed'i gerektirir"]
fn lvl_001_ignores_duplicate_empty_level_ids_because_lvl_008_reports_the_root() {
    empty_key_boundary("mdb-3357", "levels.txt", "LVL_001", "LVL_008");
}

// ─────────────────────────────────────────────────────────────────────────────
// TRN_005 — exact duplicate translation row
// ─────────────────────────────────────────────────────────────────────────────

// ⚠️ Feed selection was MEASURED: the translations.txt in issue-suggested `mdb-1158` uses
// the OLD format (`trans_id,lang,translation`) and has ZERO rows — unsuitable for mutation.
// `mdb-2638` uses the modern format and has both `record_id` and `field_value`; the latter
// is required to exercise TRN_005's field_value-based identity path.
const TRN_FEED: &str = "mdb-2638";

#[test]
#[ignore = "gerçek korpus feed'i gerektirir"]
fn trn_005_fires_on_an_exactly_duplicated_translation_row() {
    duplicate_row_case(TRN_FEED, "translations.txt", "TRN_005", &[]);
}

/// Feed whose identity comes from `field_value`. ⚠️ Candidate selection was MEASURED:
/// `mdb-2638` has the `field_value` COLUMN but it is EMPTY in all 128 rows, so that feed
/// cannot exercise this path. `mdb-3176` has no `record_id` column; identity is entirely
/// `field_value`.
const TRN_FIELD_VALUE_FEED: &str = "mdb-3176";

#[test]
#[ignore = "gerçek korpus feed'i gerektirir"]
fn trn_005_uses_field_value_as_identity_when_there_is_no_record_id() {
    // BOUNDARY VALUE. `has_translation_identity` requires table+field+language and takes
    // identity from record_id OR field_value. Removing the entire identity would test the
    // wrong thing (the malformed-row gate, not duplicate detection), so a row whose identity
    // comes only from field_value is duplicated exactly.
    let zip = corpus_zip(TRN_FIELD_VALUE_FEED);
    let before = rule_ids(&zip);
    assert!(
        !before.contains("TRN_005"),
        "{TRN_FIELD_VALUE_FEED} zaten TRN_005 üretiyor"
    );

    let mutated = rewrite_member(&zip, "translations.txt", |t| {
        let header = header_of(&t);
        assert!(
            !header.iter().any(|h| h == "record_id"),
            "bu feed record_id taşıyor — field_value kimlik yolu izole edilemez"
        );
        let fv = header
            .iter()
            .position(|h| h == "field_value")
            .expect("field_value sütunu yok — aday seçimi geçersiz");

        let row = t
            .lines()
            .skip(1)
            .map(|l| l.trim_end_matches('\r'))
            .find(|l| {
                let c = split_csv_line(l);
                c.len() == header.len() && !c[fv].is_empty()
            })
            .expect("field_value'su dolu satır yok")
            .to_string();
        append_line(&t, &row)
    });

    let (added, removed) = delta(&before, &rule_ids(&mutated));
    assert_delta(&added, &removed, "TRN_005", &[]);
}

// ─────────────────────────────────────────────────────────────────────────────
// Typed-field rules — STM_003 · STM_004 · STP_004 · STP_005 (issue #129)
// ─────────────────────────────────────────────────────────────────────────────

// ⚠️ Candidate feed was selected by CONTENT, not size (lesson from batch 1): in `mdb-1903`,
// `arrival_time`/`departure_time` are populated in all first 4,000 rows, `stop_lat`/
// `stop_lon` are populated in all 362 `stops.txt` rows, and all four target rules are
// ZERO at baseline.
//
// 🔑 Sibling-rule checks ran BEFORE mutation and narrowed the question: rules consuming
// parsed time — STM_012 (37 feeds) · STM_036 (31) · STM_035 (21) — and parsed coordinates —
// SHP_009 (162) · SHP_020 (152) · STM_045 (79) — fire in the corpus. The SUCCESS path is
// therefore already proven; these tests measure whether the ERROR path is wired.
const TYPED_FEED: &str = "mdb-1903";

/// The target must appear; every added AND removed side effect must be explained.
fn assert_delta_both(
    added: &[String],
    removed: &[String],
    target: &str,
    explained_added: &[&str],
    explained_removed: &[&str],
) {
    assert!(
        added.iter().any(|r| r == target),
        "{target} mutasyondan sonra ÇIKMADI. Eklenen: {added:?}"
    );
    let bad_add: Vec<&String> = added
        .iter()
        .filter(|r| *r != target && !explained_added.contains(&r.as_str()))
        .collect();
    assert!(
        bad_add.is_empty(),
        "{target}: AÇIKLANMAMIŞ yeni bulgu {bad_add:?}"
    );
    let bad_rem: Vec<&String> = removed
        .iter()
        .filter(|r| !explained_removed.contains(&r.as_str()))
        .collect();
    assert!(
        bad_rem.is_empty(),
        "{target}: AÇIKLANMAMIŞ kaybolan bulgu {bad_rem:?}"
    );
}

fn typed_field_case(
    file: &str,
    column: &str,
    value: &str,
    target: &str,
    explained_added: &[&str],
    explained_removed: &[&str],
) {
    let zip = corpus_zip(TYPED_FEED);
    let before = rule_ids(&zip);
    assert!(
        !before.contains(target),
        "{TYPED_FEED} mutasyondan ÖNCE zaten {target} üretiyor — kontrol feed'i olamaz"
    );
    let (col, val) = (column.to_string(), value.to_string());
    let mutated = rewrite_member(&zip, file, move |t| set_field(&t, &col, 1, &val));
    let (added, removed) = delta(&before, &rule_ids(&mutated));
    assert_delta_both(&added, &removed, target, explained_added, explained_removed);
}

#[test]
#[ignore = "gerçek korpus feed'i gerektirir"]
fn stm_003_fires_on_a_malformed_arrival_time() {
    // EXPLAINED side effects (measured): an unparseable time is treated as ABSENT downstream.
    //   STM_015 — mutation hit a trip's FIRST stop row → "arrival_time missing at first stop".
    //             Direct consequence.
    //   STM_034 — only one of arrival/departure remained defined (Interop).
    typed_field_case(
        "stop_times.txt",
        "arrival_time",
        "abc",
        "STM_003",
        &["STM_015", "STM_034"],
        &[],
    );
}

#[test]
#[ignore = "gerçek korpus feed'i gerektirir"]
fn stm_004_fires_on_a_malformed_departure_time() {
    // STM_034 for the same reason: when departure is broken, only one of arrival/departure
    // remains defined. STM_015 does NOT appear here — that rule checks arrival_time; the
    // asymmetry is intentional (the spec requires arrival_time at the first/last stop, but
    // has NO such requirement for departure_time).
    typed_field_case(
        "stop_times.txt",
        "departure_time",
        "abc",
        "STM_004",
        &["STM_034"],
        &[],
    );
}

#[test]
#[ignore = "gerçek korpus feed'i gerektirir"]
fn stp_004_fires_on_a_non_numeric_stop_lat() {
    // NO side effect (measured): corrupting one stop's coordinates changes no aggregated or
    // cascade rule class.
    typed_field_case("stops.txt", "stop_lat", "abc", "STP_004", &[], &[]);
}

#[test]
#[ignore = "gerçek korpus feed'i gerektirir"]
fn stp_005_fires_on_an_invalid_stop_lon_without_whitespace() {
    // FIRST HALF OF THE CONTRAST. NO whitespace → no DQ_016 root → suppression does not run
    // → STP_005 remains visible. This proves the predicate itself works.
    typed_field_case("stops.txt", "stop_lon", "abc", "STP_005", &[], &[]);
}

#[test]
#[ignore = "gerçek korpus feed'i gerektirir"]
fn stp_005_is_born_and_suppressed_when_the_value_only_wears_whitespace() {
    // SECOND HALF OF THE CONTRAST — the actual question in issue #129.
    //
    // STP_005's silence in the corpus does NOT mean "never emitted"; at least partly it means
    // "emitted and suppressed": in `mdb-2691`, `stop_lon=" -4.732529"` PRODUCES STP_005,
    // and K7's whitespace-derivative cascade suppresses it under the DQ_016 root. This
    // mechanism CANNOT BE MODELED in `silent_rules.tsv` (the table sees only cascades
    // DECLARED in the registry), so the evidence is recorded here.
    //
    // Neither half says anything alone: the test above shows that a whitespace-free invalid
    // value fires; this test shows that a value VALID after trimming does not. Together they
    // settle the question "suppression or broken predicate?"
    let zip = corpus_zip(TYPED_FEED);
    let before = rule_ids(&zip);
    assert!(!before.contains("STP_005"));
    // The baseline ALREADY carries a DQ_016 (stops.txt / stop_name), so the root notice does
    // not appear in the delta; the expected delta is EMPTY.
    assert!(
        before.contains("DQ_016"),
        "taban çizgisi DQ_016 taşımalıydı"
    );

    let lon = {
        let mut a = zip::ZipArchive::new(std::io::Cursor::new(&zip)).unwrap();
        let name = a
            .file_names()
            .map(|n| n.expect("üye adı çözülemedi").into_owned())
            .find(|n| Path::new(n).file_name().and_then(|s| s.to_str()) == Some("stops.txt"))
            .unwrap();
        let mut e = a.by_name(&name).unwrap();
        let mut buf = String::new();
        std::io::Read::read_to_string(&mut e, &mut buf).unwrap();
        let header = header_of(&buf);
        let i = header.iter().position(|h| h == "stop_lon").unwrap();
        split_csv_line(first_data_line(&buf))[i].clone()
    };
    let padded = format!(" {lon}");
    let mutated = rewrite_member(&zip, "stops.txt", move |t| {
        set_field(&t, "stop_lon", 1, &padded)
    });
    let (added, removed) = delta(&before, &rule_ids(&mutated));

    assert!(
        !added.iter().any(|r| r == "STP_005"),
        "trim sonrası GEÇERLİ bir değer STP_005 üretti — whitespace kaskadı artık \
         bastırmıyor demektir; DQ_016 kartı ve #129 güncellenmeli. Eklenen: {added:?}"
    );
    assert!(
        added.is_empty() && removed.is_empty(),
        "beklenen fark BOŞ kümeydi (kök DQ_016 zaten vardı). eklenen={added:?} kaldırılan={removed:?}"
    );

    // 🔑 THE ACTUAL CLAIM. An empty delta alone does not say "suppressed" — it would remain
    // empty if the mutation never ran. K7 WRITES the rules it suppressed to the root notice;
    // the test relies on that declaration. Independent corpus example: the stops.txt root
    // in `mdb-2691` carries `suppressed_derivative_rules = STP_005`.
    let suppressed = suppressed_rules_for(&mutated, "stops.txt");
    assert!(
        suppressed.iter().any(|r| r == "STP_005"),
        "DQ_016 kökü STP_005'i bastırdığını BEYAN ETMİYOR → boş fark bastırmadan değil, \
         mutasyonun hiç doğmamasından geliyor olabilir. Beyan edilen: {suppressed:?}"
    );
}

// ─────────────────────────────────────────────────────────────────────────────
// Cross-reference + Flex — PTH_001 · PTH_026 · TRF_021 · STM_058 (issue #130)
// ─────────────────────────────────────────────────────────────────────────────

// Batch 1/2 covered K3 repetition and K2 type paths; this batch deliberately changes
// CODE PATHS: K4 cross-reference (PTH_026, TRF_021) and K2 Flex flow (STM_058).
//
// 🔑 Sibling check (before mutation, free): pathways are read (`PTH_007` · `PTH_011` ·
// `PTH_012`), transfers are read (`TRF_016` 5204 findings · `TRF_019` · `TRF_022/023`),
// and the Flex path is read (`STM_056` 3.97M findings · `XFL_002` 1315 · `LOC_006`).
// All three prove that the file was processed; what is measured is still the ERROR/VIOLATION path.

/// Returns an archive member's text.
fn member_text(zip: &[u8], file: &str) -> String {
    let mut a = zip::ZipArchive::new(std::io::Cursor::new(zip)).expect("arşiv açılamadı");
    let name = a
        .file_names()
        .map(|n| n.expect("üye adı çözülemedi").into_owned())
        .find(|n| Path::new(n).file_name().and_then(|s| s.to_str()) == Some(file))
        .unwrap_or_else(|| panic!("{file} arşivde yok"));
    let mut e = a.by_name(&name).expect("üye okunamadı");
    let mut buf = String::new();
    std::io::Read::read_to_string(&mut e, &mut buf).expect("üye UTF-8 değil");
    buf
}

/// The first `stop_id` in `stops.txt` with the given `location_type` value.
fn stop_id_with_location_type(zip: &[u8], want: &str) -> String {
    let text = member_text(zip, "stops.txt");
    let header = header_of(&text);
    let id_i = header
        .iter()
        .position(|h| h == "stop_id")
        .expect("stop_id yok");
    let lt_i = header
        .iter()
        .position(|h| h == "location_type")
        .expect("location_type sütunu yok");
    text.lines()
        .skip(1)
        .map(|l| split_csv_line(l.trim_end_matches('\r')))
        .find(|c| c.len() == header.len() && c[lt_i].trim() == want)
        .map(|c| c[id_i].clone())
        .unwrap_or_else(|| panic!("location_type={want} olan durak yok"))
}

/// Ordinal of the first data row whose `key_col` is NOT empty (1 = first data row).
fn first_row_with_value(text: &str, key_col: &str) -> usize {
    let header = header_of(text);
    let ki = header
        .iter()
        .position(|h| h == key_col)
        .unwrap_or_else(|| panic!("{key_col} sütunu yok"));
    text.lines()
        .enumerate()
        .skip(1)
        .find(|(_, l)| {
            let c = split_csv_line(l.trim_end_matches('\r'));
            c.len() == header.len() && !c[ki].trim().is_empty()
        })
        .map(|(i, _)| i)
        .unwrap_or_else(|| panic!("{key_col} dolu satır yok"))
}

const PTH_FEED: &str = "mdb-3357";
const TRF_FEED: &str = "mdb-2848";
const FLEX_FEED: &str = "mdb-3037";

#[test]
#[ignore = "gerçek korpus feed'i gerektirir"]
fn pth_001_fires_on_a_duplicated_pathway_row() {
    duplicate_row_case(PTH_FEED, "pathways.txt", "PTH_001", &[]);
}

#[test]
#[ignore = "gerçek korpus feed'i gerektirir"]
fn pth_026_fires_when_a_pathway_endpoint_is_a_station() {
    // K4 cross-reference. `from_stop_id` is changed to a station that REALLY EXISTS
    // (location_type=1) — using a fabricated ID would test the FK rule, not the endpoint TYPE rule.
    let zip = corpus_zip(PTH_FEED);
    let before = rule_ids(&zip);
    assert!(
        !before.contains("PTH_026"),
        "{PTH_FEED} zaten PTH_026 üretiyor"
    );
    let station = stop_id_with_location_type(&zip, "1");
    let mutated = rewrite_member(&zip, "pathways.txt", move |t| {
        set_field(&t, "from_stop_id", 1, &station)
    });
    let (added, removed) = delta(&before, &rule_ids(&mutated));
    assert_delta_both(&added, &removed, "PTH_026", &[], &[]);
}

#[test]
#[ignore = "gerçek korpus feed'i gerektirir"]
fn trf_021_fires_when_a_transfer_endpoint_is_neither_stop_nor_station() {
    // Same rationale: target an existing ENTRANCE (location_type=2).
    let zip = corpus_zip(TRF_FEED);
    let before = rule_ids(&zip);
    assert!(
        !before.contains("TRF_021"),
        "{TRF_FEED} zaten TRF_021 üretiyor"
    );
    let entrance = stop_id_with_location_type(&zip, "2");
    let mutated = rewrite_member(&zip, "transfers.txt", move |t| {
        set_field(&t, "from_stop_id", 1, &entrance)
    });
    let (added, removed) = delta(&before, &rule_ids(&mutated));
    // EXPLAINED side effect: `TRF_011` (transfer defined but distance is far, Bilgi·Quality).
    // Changing the transfer to an entrance elsewhere in the network moves the two endpoints
    // farther apart geographically; this is the direct geometric result.
    assert_delta_both(&added, &removed, "TRF_021", &["TRF_011"], &[]);
}

#[test]
#[ignore = "gerçek korpus feed'i gerektirir"]
fn stm_058_fires_on_a_malformed_flex_pickup_window() {
    // K2 flow path + Flex. ⚠️ Row selection is MEASURED: 19 corpus feeds carry this header,
    // but ONLY 7 have a value; mutating an empty window would test nothing. Therefore find
    // the first row with a POPULATED window.
    // STM_058 was also the first member of the `Err(_) => None` swallowing class (closed in
    // `778c462b`); this test's purpose is to prove that a repaired parse path still emits.
    let zip = corpus_zip(FLEX_FEED);
    let before = rule_ids(&zip);
    assert!(
        !before.contains("STM_058"),
        "{FLEX_FEED} zaten STM_058 üretiyor"
    );
    let text = member_text(&zip, "stop_times.txt");
    let row = first_row_with_value(&text, "start_pickup_drop_off_window");
    let mutated = rewrite_member(&zip, "stop_times.txt", move |t| {
        set_field(&t, "start_pickup_drop_off_window", row, "abc")
    });
    let (added, removed) = delta(&before, &rule_ids(&mutated));
    // EXPLAINED side effect — and this is a LOST finding: `PDW_006` (overlapping pickup/drop-off
    // window for the same trip+zone, Orta·Analytics). Making the window unparsable DROPS that
    // window from overlap analysis, so the overlap finding it belonged to also disappears.
    // This is legitimate — and exactly why the test explains rather than FORBIDS removals:
    // forbidding them would make this mutation impossible to write.
    assert_delta_both(&added, &removed, "STM_058", &[], &["PDW_006"]);
}

// ─────────────────────────────────────────────────────────────────────────────
// END OF THE Critical∧Spec suspicious bucket — 9 rules, one K2 parse-and-reject shape (#131)
// ─────────────────────────────────────────────────────────────────────────────

// 🔑 The sibling check showed that all five files were read: `ATR_001` (8 feeds)/`ATR_009` (89
// findings) · `CAL_024` (673,569 findings)/`CAL_013` (3296) · `FAR_009` (1091)/`FAR_013` (38) ·
// `FRQ_010` (4033)/`FRQ_007` (1583) · `LVL_006` (114)/`LVL_004`. The ERROR path is still measured.
//
// Three feeds are REUSED from earlier batches (byte lengths and clean baselines were already
// verified); two are new and were selected by CONTENT.

/// Common body: write `value` to row `row` of `feed`/`file`/`column`, then check the delta.
#[allow(clippy::too_many_arguments)]
fn mutate_field_case(
    feed: &str,
    file: &str,
    column: &str,
    row: usize,
    value: &str,
    target: &str,
    explained_added: &[&str],
    explained_removed: &[&str],
) {
    let zip = corpus_zip(feed);
    let before = rule_ids(&zip);
    assert!(
        !before.contains(target),
        "{feed} already produces {target} BEFORE mutation — it cannot be the control feed"
    );
    let (col, val) = (column.to_string(), value.to_string());
    let mutated = rewrite_member(&zip, file, move |t| set_field(&t, &col, row, &val));
    let (added, removed) = delta(&before, &rule_ids(&mutated));
    assert_delta_both(&added, &removed, target, explained_added, explained_removed);
}

/// Finds the first row with a populated `column` and writes there (mutating an empty cell
/// tests nothing — this trap was measured in `start_pickup_drop_off_window` in #130).
fn mutate_first_populated(
    feed: &str,
    file: &str,
    column: &str,
    value: &str,
    target: &str,
    explained_added: &[&str],
) {
    let zip = corpus_zip(feed);
    let before = rule_ids(&zip);
    assert!(!before.contains(target), "{feed} zaten {target} üretiyor");
    let row = first_row_with_value(&member_text(&zip, file), column);
    let (col, val) = (column.to_string(), value.to_string());
    let mutated = rewrite_member(&zip, file, move |t| set_field(&t, &col, row, &val));
    let (added, removed) = delta(&before, &rule_ids(&mutated));
    assert_delta_both(&added, &removed, target, explained_added, &[]);
}

// `mdb-2931`: 18 attribution rows; all three `is_*` columns are populated, `attribution_url` in 8.
// ⚠️ The baseline ALREADY produces `ATR_009` (mutual exclusion) — in all 18 rows.
// It is invisible in the delta because it predates the mutation; this avoids cascade noise.
const ATR_FEED: &str = "mdb-2931";
const FRQ_FEED: &str = "mdb-1914"; // 3909 frequencies rows
/// Smallest feed carrying platform-ended pathways (232 endpoints).
///
/// 🔴 **NO corpus feed has a `stops.stop_access` column** (all 6 pathway feeds were scanned).
/// The field is not adopted in the wild, so `PTH_031` and `STP_043` can NEVER be tested with
/// real data; this mutation is the only way to produce the column.
const PTH_PLATFORM_FEED: &str = "mdb-2933";

#[test]
#[ignore = "gerçek korpus feed'i gerektirir"]
fn atr_004_fires_on_an_invalid_is_producer() {
    mutate_field_case(
        ATR_FEED,
        "attributions.txt",
        "is_producer",
        1,
        "9",
        "ATR_004",
        &[],
        &[],
    );
}

#[test]
#[ignore = "gerçek korpus feed'i gerektirir"]
fn atr_005_fires_on_an_invalid_is_operator() {
    // EXPLAINED side effect: `ATR_003` ("attribution role is undefined", Yüksek·Spec).
    // The first row of `mdb-2931` has `is_producer=0, is_operator=1, is_authority=0` —
    // `is_operator` is the ONLY defined role. Invalidating it leaves no valid role, producing
    // ATR_003. The `ATR_004` (is_producer 0→9) test has NO such SIDE EFFECT; this is a data
    // result, not rule asymmetry: its is_operator=1 role remains valid.
    mutate_field_case(
        ATR_FEED,
        "attributions.txt",
        "is_operator",
        1,
        "9",
        "ATR_005",
        &["ATR_003"],
        &[],
    );
}

#[test]
#[ignore = "gerçek korpus feed'i gerektirir"]
fn atr_006_fires_on_an_invalid_is_authority() {
    mutate_field_case(
        ATR_FEED,
        "attributions.txt",
        "is_authority",
        1,
        "9",
        "ATR_006",
        &[],
        &[],
    );
}

#[test]
#[ignore = "gerçek korpus feed'i gerektirir"]
fn atr_007_fires_on_an_invalid_attribution_url() {
    // `attribution_url` is populated in only 8 of 18 rows → choose the first populated row.
    mutate_first_populated(
        ATR_FEED,
        "attributions.txt",
        "attribution_url",
        "not a url",
        "ATR_007",
        &[],
    );
}

#[test]
#[ignore = "gerçek korpus feed'i gerektirir"]
fn cal_002_fires_on_an_invalid_weekday_value() {
    mutate_field_case(
        "mdb-3141",
        "calendar.txt",
        "monday",
        1,
        "9",
        "CAL_002",
        &[],
        &[],
    );
}

#[test]
#[ignore = "gerçek korpus feed'i gerektirir"]
fn far_004_fires_on_an_invalid_payment_method() {
    mutate_field_case(
        "mdb-1996",
        "fare_attributes.txt",
        "payment_method",
        1,
        "9",
        "FAR_004",
        &[],
        &[],
    );
}

#[test]
#[ignore = "gerçek korpus feed'i gerektirir"]
fn frq_002_fires_on_a_malformed_start_time() {
    mutate_field_case(
        FRQ_FEED,
        "frequencies.txt",
        "start_time",
        1,
        "abc",
        "FRQ_002",
        &[],
        &[],
    );
}

#[test]
#[ignore = "gerçek korpus feed'i gerektirir"]
fn frq_003_fires_on_a_malformed_end_time() {
    mutate_field_case(
        FRQ_FEED,
        "frequencies.txt",
        "end_time",
        1,
        "abc",
        "FRQ_003",
        &[],
        &[],
    );
}

#[test]
#[ignore = "gerçek korpus feed'i gerektirir"]
fn lvl_002_fires_on_a_non_numeric_level_index() {
    mutate_field_case(
        "mdb-3357",
        "levels.txt",
        "level_index",
        1,
        "abc",
        "LVL_002",
        &[],
        &[],
    );
}

/// Adds a NEW column to the file; populated only at `row_ordinal`, empty elsewhere.
///
/// `PTH_031` checks optional `stops.stop_access`, and that column is ABSENT from the
/// corpus pathway feeds. Since no existing value can be changed, the mutation creates the
/// column. Row lengths stay CONSISTENT with the header — leaving short rows would trigger a
/// different rule (CSV field count) and make the test measure the wrong thing.
fn add_column(text: &str, name: &str, row_ordinal: usize, value: &str) -> String {
    let mut out: Vec<String> = Vec::new();
    for (i, line) in text.lines().enumerate() {
        let line = line.trim_end_matches('\r');
        if line.is_empty() {
            continue;
        }
        let mut cols = split_csv_line(line);
        cols.push(if i == 0 {
            name.to_string()
        } else if i == row_ordinal {
            value.to_string()
        } else {
            String::new()
        });
        out.push(join_csv_fields(&cols));
    }
    out.join("\n") + "\n"
}

#[test]
#[ignore = "gerçek korpus feed'i gerektirir"]
fn pth_031_fires_when_a_pathway_endpoint_is_street_accessed() {
    // Last rule in the bucket. `stop_access=1` (a street-accessible stop) cannot be a
    // pathway endpoint. None of the corpus's 10 pathway feeds has a `stop_access` column,
    // so the mutation CREATES the column and sets it to 1 only at the stop referenced by
    // the pathway's `from_stop_id`.
    // ⚠️ FEED SELECTION WAS MEASURED: in 4 pathway feeds including `mdb-3357`, NO pathway
    // endpoint is a platform — all connect stations/entrances/intermediate nodes. Six
    // pathway feeds were scanned; only `mdb-2933` (232 platform endpoints) and `mdb-990`
    // (103) were suitable.
    let zip = corpus_zip(PTH_PLATFORM_FEED);
    let before = rule_ids(&zip);
    assert!(
        !before.contains("PTH_031"),
        "{PTH_PLATFORM_FEED} zaten PTH_031 üretiyor"
    );

    // ⚠️ The endpoint must be a PLATFORM/STOP (`location_type` empty or 0). In the first
    // attempt, the first pathway endpoint was a STATION and `stop_access` was already
    // Conditionally Forbidden there, so `STP_043` fired instead of `PTH_031`. The mutation
    // was testing the wrong assertion; the neighboring rule caught it. The candidate is now
    // selected by MEASUREMENT.
    let stops = member_text(&zip, "stops.txt");
    let sh = header_of(&stops);
    let si = sh.iter().position(|h| h == "stop_id").expect("stop_id yok");
    let lti = sh.iter().position(|h| h == "location_type");
    let is_platform = |cols: &[String]| match lti {
        None => true,
        Some(i) => matches!(cols.get(i).map(|v| v.trim()), None | Some("") | Some("0")),
    };

    let pathways = member_text(&zip, "pathways.txt");
    let ph = header_of(&pathways);
    let fi = ph
        .iter()
        .position(|h| h == "from_stop_id")
        .expect("from_stop_id yok");
    let endpoints: BTreeSet<String> = pathways
        .lines()
        .skip(1)
        .map(|l| split_csv_line(l.trim_end_matches('\r')))
        .filter(|c| c.len() == ph.len())
        .map(|c| c[fi].clone())
        .collect();

    let row = stops
        .lines()
        .enumerate()
        .skip(1)
        .map(|(i, l)| (i, split_csv_line(l.trim_end_matches('\r'))))
        .find(|(_, c)| c.len() == sh.len() && endpoints.contains(&c[si]) && is_platform(c))
        .map(|(i, _)| i)
        .expect("pathway ucu olan peron/durak (location_type boş|0) yok");

    let mutated = rewrite_member(&zip, "stops.txt", move |t| {
        add_column(&t, "stop_access", row, "1")
    });
    let (added, removed) = delta(&before, &rule_ids(&mutated));
    assert_delta_both(&added, &removed, "PTH_031", &[], &[]);
}

// ─────────────────────────────────────────────────────────────────────────────
// FATAL path — ARC_001 · ARC_002 · ARC_013 · ARC_029
// ─────────────────────────────────────────────────────────────────────────────
//
// These four rules produce `FatalError`, not notices, so `rule_ids()` cannot see them —
// that helper panics on Fatal. That is not why they never appear in the corpus: a publisher
// does not publish an archive that cannot be opened, is not UTF-8, or is a zip bomb. This is
// therefore the purest SELECTION BIAS case, with no measurement path other than mutation.
//
// ⚠️ `ARC_004` is NOT on this list: `fatals.csv` shows that it REALLY fired in 4 corpus
// feeds (`NoRequiredFiles`). It looked silent because `rule_stats.csv` did not carry Fatals —
// `silent_rules_scan.py` now closes that trap.

/// Returns the code if Fatal is produced; `None` if a notice is produced.
fn fatal_code(zip: &[u8]) -> Option<FatalCode> {
    match validate_bytes(zip, &ValidatorConfig::default(), TODAY) {
        ValidateResult::Fatal(e) => Some(e.code),
        ValidateResult::Ok(_) => None,
    }
}

/// Rewrites a member's BYTES (for non-text content).
fn rewrite_member_bytes(zip: &[u8], target: &str, bytes: Vec<u8>) -> Vec<u8> {
    let mut archive = zip::ZipArchive::new(std::io::Cursor::new(zip)).expect("arşiv açılamadı");
    let names: Vec<String> = archive
        .file_names()
        .map(|n| n.expect("üye adı çözülemedi").into_owned())
        .collect();
    let mut out = zip::ZipWriter::new(std::io::Cursor::new(Vec::new()));
    let opts = zip::write::SimpleFileOptions::default();
    let mut hit = false;
    let mut payload = Some(bytes);
    for name in &names {
        let mut entry = archive.by_name(name).expect("üye okunamadı");
        let mut buf = Vec::new();
        entry.read_to_end(&mut buf).expect("üye okunamadı");
        drop(entry);
        out.start_file(name.as_str(), opts).expect("üye yazılamadı");
        if Path::new(name).file_name().and_then(|s| s.to_str()) == Some(target) {
            hit = true;
            out.write_all(&payload.take().expect("hedef iki kez eşleşti"))
                .expect("yazılamadı");
        } else {
            out.write_all(&buf).expect("kopyalanamadı");
        }
    }
    assert!(hit, "{target} arşivde yok");
    out.finish().expect("arşiv kapanmadı").into_inner()
}

/// Adds a new member to the archive.
fn add_member(zip: &[u8], name: &str, bytes: &[u8]) -> Vec<u8> {
    let mut archive = zip::ZipArchive::new(std::io::Cursor::new(zip)).expect("arşiv açılamadı");
    let names: Vec<String> = archive
        .file_names()
        .map(|n| n.expect("üye adı çözülemedi").into_owned())
        .collect();
    let mut out = zip::ZipWriter::new(std::io::Cursor::new(Vec::new()));
    let opts = zip::write::SimpleFileOptions::default();
    for n in &names {
        let mut entry = archive.by_name(n).expect("üye okunamadı");
        let mut buf = Vec::new();
        entry.read_to_end(&mut buf).expect("üye okunamadı");
        drop(entry);
        out.start_file(n.as_str(), opts).expect("üye yazılamadı");
        out.write_all(&buf).expect("kopyalanamadı");
    }
    out.start_file(name, opts).expect("yeni üye yazılamadı");
    out.write_all(bytes).expect("yazılamadı");
    out.finish().expect("arşiv kapanmadı").into_inner()
}

/// REMOVES the given file names from the archive.
fn drop_members(zip: &[u8], drop: &[&str]) -> Vec<u8> {
    let mut archive = zip::ZipArchive::new(std::io::Cursor::new(zip)).expect("arşiv açılamadı");
    let names: Vec<String> = archive
        .file_names()
        .map(|n| n.expect("üye adı çözülemedi").into_owned())
        .collect();
    let mut out = zip::ZipWriter::new(std::io::Cursor::new(Vec::new()));
    let opts = zip::write::SimpleFileOptions::default();
    for n in &names {
        let base = Path::new(n)
            .file_name()
            .and_then(|s| s.to_str())
            .unwrap_or("");
        if drop.contains(&base) {
            continue;
        }
        let mut entry = archive.by_name(n).expect("üye okunamadı");
        let mut buf = Vec::new();
        entry.read_to_end(&mut buf).expect("üye okunamadı");
        drop_guard(&mut entry);
        out.start_file(n.as_str(), opts).expect("üye yazılamadı");
        out.write_all(&buf).expect("kopyalanamadı");
    }
    out.finish().expect("arşiv kapanmadı").into_inner()
}

/// Ends the `archive.by_name` borrow early.
fn drop_guard<T>(_: &mut T) {}

/// Baseline archive for Fatal tests. Size is IRRELEVANT here — what is measured is an
/// archive-level failure, not feed content; what matters is that this is a REAL published archive.
const FATAL_FEED: &str = "mdb-1903";

#[test]
#[ignore = "gerçek korpus feed'i gerektirir"]
fn arc_001_fatal_when_the_archive_is_truncated() {
    let zip = corpus_zip(FATAL_FEED);
    assert!(
        fatal_code(&zip).is_none(),
        "taban arşiv zaten Fatal veriyor"
    );
    // Cut the trailing central-directory record → the archive cannot be opened.
    let cut = &zip[..zip.len() - 256];
    assert_eq!(fatal_code(cut), Some(FatalCode::ZipUnreadable));
}

#[test]
#[ignore = "gerçek korpus feed'i gerektirir"]
fn arc_002_reports_invalid_utf8_as_a_notice_not_a_fatal() {
    // 🔴 THIS TEST WAS WRITTEN TO DISPROVE AN ASSUMPTION. Its first version expected
    // `Fatal(Utf8Critical)`, because `core::enums::FatalCode` carries this comment:
    // "Utf8Critical → ARC_002 — UTF-8 violation, file cannot be read". Measurement disproved
    // it: putting invalid bytes in a required file produces a NOTICE, not Fatal.
    //
    // Reason: `FatalCode::Utf8Critical` is NEVER CONSTRUCTED in production code (it appears
    // only in the enum definition and one comment). The same applies to `NoRequiredFiles`
    // and `CsvMalformed` — all three are variants no Rust code produces (issue #132).
    // ⚠️ `ResourceLimit` is NOT on this list: the UI's TypeScript produces it
    // (`ui/src/validator-client.ts`), so it is not dead; it simply has no Rust producer.
    let zip = corpus_zip(FATAL_FEED);
    let before = rule_ids(&zip);
    assert!(
        !before.contains("ARC_002"),
        "taban feed zaten ARC_002 üretiyor"
    );

    let mut bad = b"stop_id,stop_name,stop_lat,stop_lon\n".to_vec();
    bad.extend_from_slice(&[
        b'S', b'1', b',', 0xFF, 0xFE, 0xFF, b',', b'4', b'1', b'.', b'0', b',', b'2', b'9', b'.',
        b'0', b'\n',
    ]);
    let mutated = rewrite_member_bytes(&zip, "stops.txt", bad);

    assert_eq!(
        fatal_code(&mutated),
        None,
        "geçersiz UTF-8 Fatal üretti — davranış değişmiş, ölü varyant canlanmış olabilir"
    );
    let after = rule_ids(&mutated);
    assert!(
        after.contains("ARC_002"),
        "geçersiz UTF-8 ne Fatal ne ARC_002 üretti — ihlal HİÇ raporlanmıyor demektir"
    );
}

#[test]
#[ignore = "gerçek korpus feed'i gerektirir"]
fn arc_004_reports_missing_required_files_as_a_notice_since_partial_recovery() {
    // `fatals.csv` (2026-08-06 corpus run) records the `NoRequiredFiles` Fatal in 4 feeds.
    // That file is STALE: `66e009c2` ("recover and report partial feeds", 2026-08-09) changed
    // the pipeline to recover and REPORT partial feeds. Today missing required files produce
    // NO Fatal; `ARC_004` appears as a NOTICE.
    let zip = corpus_zip(FATAL_FEED);
    let required = [
        "stops.txt",
        "trips.txt",
        "stop_times.txt",
        "routes.txt",
        "agency.txt",
    ];
    let mutated = drop_members(&zip, &required);

    assert_eq!(
        fatal_code(&mutated),
        None,
        "zorunlu dosya eksikliği artık Fatal DEĞİL"
    );
    assert!(
        rule_ids(&mutated).contains("ARC_004"),
        "zorunlu dosyalar atıldı ama ARC_004 çıkmadı"
    );
}

#[test]
#[ignore = "gerçek korpus feed'i gerektirir"]
fn arc_029_fatal_when_a_member_is_a_decompression_bomb() {
    // `ratio_floor` 16 MiB, `max_ratio` 100 → 24 MiB of one repeated byte compresses at
    // about 1000:1, so it fails through the RATIO path (zip-bomb signature), not an absolute cap.
    // ⚠️ The bomb MUST be a root-level `.txt`: K1 skips non-`.txt` entries without reading them
    // (ARC_007), so the guard never activates.
    let zip = corpus_zip(FATAL_FEED);
    let bomb = vec![b'A'; 24 * 1024 * 1024];
    let mutated = add_member(&zip, "bomb.txt", &bomb);
    assert_eq!(fatal_code(&mutated), Some(FatalCode::DecompressionLimit));
}

#[test]
#[ignore = "gerçek korpus feed'i gerektirir"]
fn arc_013_reports_csv_tokenization_failure_as_a_notice_not_a_fatal() {
    // `FatalCode::CsvMalformed` is also a DEAD variant (see the arc_002 test). When
    // `tokenize_csv` returns an error, K1 adds the file to `partial.unavailable_files` and
    // produces an `ARC_013` NOTICE; the feed is not rejected wholesale.
    let zip = corpus_zip(FATAL_FEED);
    let before = rule_ids(&zip);
    assert!(
        !before.contains("ARC_013"),
        "taban feed zaten ARC_013 üretiyor"
    );

    // Unclosed quote → tokenization fails.
    let broken = "route_id,agency_id,route_short_name,route_type\nR1,\"1,ACIK TIRNAK,3\n";
    let mutated = rewrite_member_bytes(&zip, "routes.txt", broken.as_bytes().to_vec());

    assert_eq!(fatal_code(&mutated), None, "bozuk CSV artık Fatal DEĞİL");
    assert!(
        rule_ids(&mutated).contains("ARC_013"),
        "bozuk CSV ne Fatal ne ARC_013 üretti — tokenization hatası HİÇ raporlanmıyor"
    );
}

// ─────────────────────────────────────────────────────────────────────────────
// Coverage loss must affect the publication decision and score (issue #133)
// ─────────────────────────────────────────────────────────────────────────────

fn r1_of(zip: &[u8]) -> (bool, bool, f64) {
    match validate_bytes(zip, &ValidatorConfig::default(), TODAY) {
        ValidateResult::Ok(vr) => (
            vr.reports.r1.publishable,
            vr.reports.r1.coverage_complete,
            vr.metrics.overall_score,
        ),
        ValidateResult::Fatal(e) => panic!("Fatal: {:?}", e.code),
    }
}

#[test]
#[ignore = "gerçek korpus feed'i gerektirir"]
fn losing_a_required_file_withdraws_the_publishable_verdict() {
    // 🔴 OLD BEHAVIOR: when `stops.txt` became unreadable, 29 checks were skipped, notices
    // fell from 604 to 178, and the feed STILL reported `publishable=true` — while the score
    // ROSE from 89.4 to 93.5. Absence of findings is not evidence of absence.
    let zip = corpus_zip(FATAL_FEED);
    let (pub_before, cov_before, score_before) = r1_of(&zip);
    assert!(
        pub_before && cov_before,
        "taban feed temiz ve tam kapsamlı olmalı"
    );

    let mut bad = b"stop_id,stop_name,stop_lat,stop_lon\n".to_vec();
    bad.extend_from_slice(&[
        b'S', b'1', b',', 0xFF, 0xFE, 0xFF, b',', b'4', b'1', b'.', b'0', b',', b'2', b'9', b'.',
        b'0', b'\n',
    ]);
    let mutated = rewrite_member_bytes(&zip, "stops.txt", bad);
    let (pub_after, cov_after, score_after) = r1_of(&mutated);

    assert!(!cov_after, "zorunlu dosya okunamadı ama kapsam TAM sayıldı");
    assert!(!pub_after, "kapsam eksikken feed YAYINA UYGUN sayıldı");
    // The score increase is still real — it is deliberately not corrected, but marked
    // INCOMPARABLE. This assertion ANCHORS that fact so it cannot change silently.
    assert!(
        score_after > score_before,
        "eksik kapsamda skorun YÜKSELDİĞİ olgusu kayboldu ({score_before} → {score_after}); \
         skor artık düzeltiliyorsa bu test ve #133 güncellenmeli"
    );
}

#[test]
#[ignore = "gerçek korpus feed'i gerektirir"]
fn losing_an_optional_file_does_not_withdraw_the_verdict() {
    // ⚠️ ASYMMETRY TEST. An unreadable optional file also makes the run PARTIAL
    // (measured). Dropping the gate for that case would ban a valid feed from publication
    // because of a broken `attributions.txt` — the severe error named in `PTH_017`.
    let zip = corpus_zip(ATR_FEED);
    let (_, cov_before, _) = r1_of(&zip);
    assert!(cov_before, "taban feed tam kapsamlı olmalı");

    let broken = b"attribution_id,agency_id,organization_name\n1,\"ACIK TIRNAK,Test\n".to_vec();
    let mutated = rewrite_member_bytes(&zip, "attributions.txt", broken);
    let (_, cov_after, _) = r1_of(&mutated);

    assert!(
        cov_after,
        "isteğe bağlı dosya kaybı kapsamı EKSİK saydı → geçerli feed'ler yayından men edilir"
    );
}

#[test]
#[ignore = "gerçek korpus feed'i gerektirir"]
fn trn_006_fires_on_a_conflicting_translation() {
    // `TRN_006` entered the bucket LATE: it fired with 101,871 findings in the 2026-08-06
    // baseline, then fell to ZERO when `67fc99b0` (#99) tied translation-row checks to schema
    // prerequisites. Its silence was therefore new and unmeasured.
    //
    // The other arm of the SAME `match` as `TRN_005`: the key collides but the
    // `translation` value is DIFFERENT. The mutation therefore repeats the row with a
    // CHANGED VALUE rather than verbatim — an exact copy would produce TRN_005 and test the
    // wrong arm.
    let zip = corpus_zip(TRN_FEED);
    let before = rule_ids(&zip);
    assert!(
        !before.contains("TRN_006"),
        "{TRN_FEED} zaten TRN_006 üretiyor"
    );

    let mutated = rewrite_member(&zip, "translations.txt", |t| {
        let header = header_of(&t);
        let ti = header
            .iter()
            .position(|h| h == "translation")
            .expect("translation sütunu yok");
        let mut cols = split_csv_line(first_data_line(&t));
        assert_eq!(cols.len(), header.len(), "alan sayısı başlıkla uyuşmuyor");
        cols[ti] = format!("{}-CELISKI", cols[ti]);
        append_line(&t, &join_csv_fields(&cols))
    });

    let (added, removed) = delta(&before, &rule_ids(&mutated));
    assert_delta_both(&added, &removed, "TRN_006", &[], &[]);
}

// ─────────────────────────────────────────────────────────────────────────────
// The whitespace cascade must inspect BOTH SIDES of the FK (issue #134)
// ─────────────────────────────────────────────────────────────────────────────

/// Prepends a space to every `trip_id` in `trips.txt` — it does not touch `stop_times.txt`.
/// This is the real shape of corpus feed `mdb-992`: spaced in one file, clean in the other.
fn pad_trip_ids(text: &str) -> String {
    let mut lines: Vec<String> = text
        .lines()
        .map(|l| l.trim_end_matches('\r').to_string())
        .collect();
    let header = header_of(text);
    let i = header
        .iter()
        .position(|h| h == "trip_id")
        .expect("trip_id yok");
    for line in lines.iter_mut().skip(1) {
        let mut cols = split_csv_line(line);
        if cols.len() == header.len() && !cols[i].is_empty() {
            cols[i] = format!(" {}", cols[i]);
            *line = join_csv_fields(&cols);
        }
    }
    lines.join("\n") + "\n"
}

#[test]
#[ignore = "gerçek korpus feed'i gerektirir"]
fn whitespace_on_the_reference_side_is_attributed_to_the_dq_016_root() {
    // The cascade covered only ONE DIRECTION: when the observed value had whitespace. This is
    // the MIRROR case — the observed side (`stop_times.trip_id`) is clean, while whitespace
    // is on the REFERENCE side (`trips.trip_id`). Corpus feed `mdb-992` is exactly this:
    // raw intersection 0, trimmed intersection 6523, and one producer habit became 103,649 findings.
    let zip = corpus_zip(FATAL_FEED);
    let before = rule_ids(&zip);
    assert!(
        !before.contains("STM_001"),
        "taban feed zaten STM_001 üretiyor"
    );

    let mutated = rewrite_member(&zip, "trips.txt", |t| pad_trip_ids(&t));

    let declared = suppressed_rules_for(&mutated, "trips.txt");
    let after = rule_ids(&mutated);

    // `XFL_002`'s file is `trips.txt` — where the whitespace and therefore the `DQ_016`
    // root live. It is suppressed AND declared.
    assert!(
        !after.contains("XFL_002"),
        "referans tarafındaki boşluktan doğan XFL_002 hâlâ ham bulgu olarak raporlanıyor"
    );
    assert!(
        declared.iter().any(|r| r == "XFL_002"),
        "DQ_016 kökü XFL_002'yi bastırdığını BEYAN ETMİYOR: {declared:?}"
    );

    // ⚠️ `STM_001`'s file is `stop_times.txt`, and there is NO whitespace THERE → no root.
    // Suppression is NOT APPLIED because there is nowhere to write a declaration; the finding
    // remains visible. This is not a gap, but a deliberate invariant: findings must not vanish
    // without a trace.
    assert!(
        after.contains("STM_001"),
        "kökü olmayan dosyada bastırma yapıldı → bulgu izsiz kayboldu"
    );
}

#[test]
#[ignore = "gerçek korpus feed'i gerektirir"]
fn a_genuinely_unknown_id_is_not_swallowed_by_the_cascade() {
    // OVER-SUPPRESSION GUARD. If relaxing the cascade swallows real FK errors, the fix is
    // worse than the disease. A nonexistent trip_id also has no trimmed counterpart → it MUST
    // remain visible.
    let zip = corpus_zip(FATAL_FEED);
    let mutated = rewrite_member(&zip, "stop_times.txt", |t| {
        set_field(&t, "trip_id", 1, "BOYLE-BIR-SEFER-YOK")
    });
    let (added, _) = delta(&rule_ids(&zip), &rule_ids(&mutated));
    assert!(
        added.iter().any(|r| r == "STM_001"),
        "genuinely unknown trip ID was SUPPRESSED — cascade is too broad. Added: {added:?}"
    );
}

// ─────────────────────────────────────────────────────────────────────────────
// STRATIFIED SAMPLING — close Phase 2 by decision (2026-08-16)
// ─────────────────────────────────────────────────────────────────────────────
//
// The 35 rules measured so far were NOT RANDOM: they were deliberately chosen from the most
// suspicious (tightened predicates, repaired swallowing class, known suppression uncertainty).
// That bias does not transfer the "zero broken predicates" result to the REMAINDER.
//
// This block selects 4 rules per bucket from the three remaining buckets
// (SELECTION_BIAS 66 · CROSS_FILE 27 · THIN_FEATURE 28 — all Critical∧Spec), using a
// FIXED-SEED random sample:
//     random.Random(20260816).sample(sorted(pool), 4)
// The goal is not to exhaust the remainder, but to test whether the "zero broken" result
// generalizes.

const SEL_PATH_FEED: &str = "mdb-3357"; // pathways, 7388 rows
const SEL_ST_FEED: &str = "mdb-1903"; // stop_times, targets are 0 in the baseline
const XREF_FEED: &str = "mdb-892"; // stop_areas + route_networks + fare_* in one feed
const RCT_FEED: &str = "mdb-1246"; // rider_categories, 3 rows
const TFR_TIME_FEED: &str = "mdb-2143"; // timeframes (the ONLY corpus feed) + populated fare_leg_rules
const TRF_TRIP_FEED: &str = "mdb-2919"; // from/to_trip_id POPULATED in transfers (1614 rows)

// ── SELECTION_BIAS sample ────────────────────────────────────────────────────

#[test]
#[ignore = "gerçek korpus feed'i gerektirir"]
fn sample_pth_023_fires_on_a_missing_pathway_mode() {
    mutate_field_case(
        SEL_PATH_FEED,
        "pathways.txt",
        "pathway_mode",
        1,
        "",
        "PTH_023",
        &[],
        &[],
    );
}

#[test]
#[ignore = "gerçek korpus feed'i gerektirir"]
fn sample_stm_006_fires_on_a_missing_stop_id() {
    // EXPLAINED side effects are filled in by measurement; an empty `stop_id` also breaks the FK chain.
    mutate_field_case(
        SEL_ST_FEED,
        "stop_times.txt",
        "stop_id",
        1,
        "",
        "STM_006",
        &[],
        &[],
    );
}

#[test]
#[ignore = "gerçek korpus feed'i gerektirir"]
fn sample_trf_006_fires_on_an_unknown_from_trip_id() {
    // ⚠️ FEED CHANGED: `mdb-2848` has a `from_trip_id` COLUMN, but it is EMPTY in all
    // 141,458 rows. Checking that the column exists but not that it is POPULATED is the same
    // trap for the third time in this round. `mdb-2919` has 1614 populated rows.
    mutate_first_populated(
        TRF_TRIP_FEED,
        "transfers.txt",
        "from_trip_id",
        "BOYLE-SEFER-YOK",
        "TRF_006",
        &[],
    );
}

#[test]
#[ignore = "gerçek korpus feed'i gerektirir"]
fn sample_trf_007_fires_on_an_unknown_to_trip_id() {
    mutate_first_populated(
        TRF_TRIP_FEED,
        "transfers.txt",
        "to_trip_id",
        "BOYLE-SEFER-YOK",
        "TRF_007",
        &[],
    );
}

// ── CROSS_FILE sample ────────────────────────────────────────────────────────

#[test]
#[ignore = "gerçek korpus feed'i gerektirir"]
fn sample_ars_001_fires_on_a_duplicated_area_row() {
    // ⚠️ The first attempt mutated `stop_areas.txt` and produced `DQ_021` (generic
    // duplication) — `ARS_001` reads `areas.txt`. The rule was not wrong; the MUTATION
    // was. "ARS" in the rule name refers to the AREA definition, not the stop-area mapping.
    duplicate_row_case(XREF_FEED, "areas.txt", "ARS_001", &[]);
}

#[test]
#[ignore = "gerçek korpus feed'i gerektirir"]
fn sample_net_003_fires_on_an_unknown_route_in_route_networks() {
    mutate_field_case(
        XREF_FEED,
        "route_networks.txt",
        "route_id",
        1,
        "BOYLE-HAT-YOK",
        "NET_003",
        &[],
        &[],
    );
}

// ── THIN_FEATURE sample ──────────────────────────────────────────────────────

#[test]
#[ignore = "gerçek korpus feed'i gerektirir"]
fn sample_flg_005_fires_on_an_unknown_from_timeframe_group() {
    // The column is empty in `mdb-892`; `mdb-2143` has 45 populated rows (the only feed
    // carrying timeframes).
    mutate_first_populated(
        TFR_TIME_FEED,
        "fare_leg_rules.txt",
        "from_timeframe_group_id",
        "BOYLE-GRUP-YOK",
        "FLG_005",
        &[],
    );
}

#[test]
#[ignore = "gerçek korpus feed'i gerektirir"]
fn sample_fpd_004_fires_on_an_unknown_fare_media() {
    mutate_first_populated(
        XREF_FEED,
        "fare_products.txt",
        "fare_media_id",
        "BOYLE-MEDYA-YOK",
        "FPD_004",
        &[],
    );
}

#[test]
#[ignore = "gerçek korpus feed'i gerektirir"]
fn sample_rct_003_fires_on_an_invalid_default_flag() {
    mutate_field_case(
        RCT_FEED,
        "rider_categories.txt",
        "is_default_fare_category",
        1,
        "9",
        "RCT_003",
        &[],
        &[],
    );
}

#[test]
#[ignore = "gerçek korpus feed'i gerektirir"]
fn sample_tfr_006_fires_when_a_timeframe_exceeds_24_hours() {
    // `timeframes.txt` exists in only ONE corpus feed; it is small but real producer data.
    // EXPLAINED side effect: `TFR_004` (end_time < start_time). Making the start 25:00:00
    // reverses the window; this is the direct result.
    mutate_field_case(
        TFR_TIME_FEED,
        "timeframes.txt",
        "start_time",
        1,
        "25:00:00",
        "TFR_006",
        &["TFR_004"],
        &[],
    );
}

const GEOJSON_FEED: &str = "ch-odv-flex"; // locations.geojson + populated stop_times.location_id

#[test]
#[ignore = "gerçek korpus feed'i gerektirir"]
fn sample_xfl_025_fires_on_an_unknown_location_id() {
    // `stop_times.location_id` must point to a feature ID in `locations.geojson`.
    mutate_first_populated(
        GEOJSON_FEED,
        "stop_times.txt",
        "location_id",
        "BOYLE-BOLGE-YOK",
        "XFL_025",
        &[],
    );
}

#[test]
#[ignore = "gerçek korpus feed'i gerektirir"]
fn sample_xfl_031_fires_when_a_geojson_id_collides_with_a_stop_id() {
    // ID-SPACE COLLISION: a feature ID in `locations.geojson` cannot equal a `stop_id` in
    // `stops.txt` — `stop_times.stop_id` and `stop_times.location_id` would give two
    // different meanings to the same name. The mutation moves a REAL stop_id into GeoJSON.
    let zip = corpus_zip(GEOJSON_FEED);
    let before = rule_ids(&zip);
    assert!(
        !before.contains("XFL_031"),
        "{GEOJSON_FEED} zaten XFL_031 üretiyor"
    );

    let stops = member_text(&zip, "stops.txt");
    let sh = header_of(&stops);
    let si = sh.iter().position(|h| h == "stop_id").expect("stop_id yok");
    let victim = split_csv_line(first_data_line(&stops))[si].clone();

    let mutated = rewrite_member(&zip, "locations.geojson", move |t| {
        // Replace the first feature's "id" field with a real stop_id. Use targeted text
        // replacement to avoid reformatting JSON; the first `"id":` is sufficient.
        let key = "\"id\"";
        let i = t.find(key).expect("geojson'da \"id\" alanı yok");
        let q1 = t[i + key.len()..].find('"').expect("id değeri yok") + i + key.len() + 1;
        let q2 = t[q1..].find('"').expect("id değeri kapanmıyor") + q1;
        format!("{}{}{}", &t[..q1], victim, &t[q2..])
    });

    let (added, removed) = delta(&before, &rule_ids(&mutated));
    assert_delta_both(&added, &removed, "XFL_031", &["XFL_025"], &[]);
}

// ─────────────────────────────────────────────────────────────────────────────
// Phase 3.1 — two ATTRIBUTION defects found by concentration triage
// ─────────────────────────────────────────────────────────────────────────────

#[test]
#[ignore = "gerçek korpus feed'i gerektirir"]
fn trn_002_still_fires_when_the_table_is_known_but_the_field_is_not() {
    // OVER-SUPPRESSION GUARD. `TRN_002` now stays silent when the table is UNKNOWN (root
    // `TRN_001` is already reported and "does this field apply to this table?" is unanswered).
    // Prove that this narrowing does not swallow the valid case: the table is VALID, the field invalid.
    let zip = corpus_zip(TRN_FEED);
    let before = rule_ids(&zip);
    assert!(
        !before.contains("TRN_002"),
        "{TRN_FEED} zaten TRN_002 üretiyor"
    );

    let mutated = rewrite_member(&zip, "translations.txt", |t| {
        set_field(&t, "field_name", 1, "boyle_bir_alan_yok")
    });
    let (added, removed) = delta(&before, &rule_ids(&mutated));
    // EXPLAINED side effect: `TRN_011` ("field_name is not translatable") — an invalid field
    // name also makes the translation record for that field meaningless. Direct result.
    assert_delta_both(&added, &removed, "TRN_002", &["TRN_011"], &[]);
}

#[test]
#[ignore = "gerçek korpus feed'i gerektirir"]
fn trf_016_still_fires_on_a_duplicate_that_carries_trip_context() {
    // OVER-SUPPRESSION GUARD. `TRF_016` now skips rows WITHOUT trip/route context (those
    // belong to `TRF_012` — the distinction both cards documented). A duplicate WITH context
    // must remain visible.
    let zip = corpus_zip(TRF_TRIP_FEED);
    let before = rule_ids(&zip);
    assert!(
        !before.contains("TRF_016"),
        "{TRF_TRIP_FEED} zaten TRF_016 üretiyor"
    );

    let mutated = rewrite_member(&zip, "transfers.txt", |t| {
        let row = first_row_with_value(&t, "from_trip_id");
        let dup = t
            .lines()
            .nth(row)
            .expect("satır yok")
            .trim_end_matches('\r')
            .to_string();
        append_line(&t, &dup)
    });
    let (added, removed) = delta(&before, &rule_ids(&mutated));
    // `TRF_012` does not fire for a contextual duplicate (its condition is an EMPTY context).
    assert_delta_both(&added, &removed, "TRF_016", &["TRF_012"], &[]);
}
