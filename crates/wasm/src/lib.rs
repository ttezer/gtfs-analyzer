use std::collections::HashSet;
use wasm_bindgen::prelude::*;

use gtfs_config::{merge_delta, ValidatorConfig};
use gtfs_core::{
    FatalCode, FatalError, PartialReport, ValidateResult, ValidationResult, ValidationStatus,
};
use gtfs_pipeline::agency_attribution::{
    index_notices, set_displayed, AgencyBreakdown, AgencyCounter,
};
use gtfs_pipeline::{
    aggregate_feed_level_notices, analyze_k6_with_files, apply_report_scope,
    build_derived_with_files, build_entity_map, build_feed_summary, build_name_index,
    check_cross_ref_with_whitespace_roots, check_rule_scope, collect_file_stats, parse_with_limits,
    prepare_k7_notices, reconcile_flex_stop_requirement, report_k7_prepared,
    validate_k2_with_whitespace_roots, DerivedData, EntityRecords, FileAvailability, FileInfo,
    WhitespaceSuppressions, GTFS_JP_FILES,
};

#[cfg(feature = "sdk-en")]
mod i18n;

macro_rules! t_start {
    ($label:expr) => {{
        #[cfg(not(feature = "quiet"))]
        web_sys::console::time_with_label($label);
    }};
}
macro_rules! t_end {
    ($label:expr) => {{
        #[cfg(not(feature = "quiet"))]
        web_sys::console::time_end_with_label($label);
    }};
}

macro_rules! wasm_log {
    ($value:expr) => {{
        #[cfg(not(feature = "quiet"))]
        {
            let value: JsValue = ($value).into();
            web_sys::console::log_1(&value);
        }
    }};
}

macro_rules! wasm_warn {
    ($value:expr) => {{
        #[cfg(not(feature = "quiet"))]
        {
            let value: JsValue = ($value).into();
            web_sys::console::warn_1(&value);
        }
    }};
}

#[wasm_bindgen(start)]
pub fn wasm_init() {
    wasm_warn!("[GTFS WASM] binary v6 yüklendi");
}

// With the `threads` feature enabled, exposes `init_thread_pool(n)` to JS; the UI calls
// `await init_thread_pool(navigator.hardwareConcurrency)` before validation to create the
// rayon pool. Until that call (or when the feature is disabled), rayon::scope runs on one thread.
#[cfg(feature = "threads")]
pub use wasm_bindgen_rayon::init_thread_pool;

// ── Constants ────────────────────────────────────────────────────────────────

const PER_RULE_CAP: usize = 500;
// The global 1M notice budget between stages was removed on 2026-10-30: because notices
// had already been produced in the stage, it did not save memory and only truncated silently
// (DELFI: 69 rules lost, score 74,1 instead of 62,5). Display is limited by PER_RULE_CAP per
// rule; real totals are in `capped_totals`, matching native.
const MAX_ZIP_ENTRIES: usize = 100_000;
const MAX_ZIP_METADATA_BYTES: usize = 16 * 1024 * 1024;
const MAX_SHAPE_COORDS: usize = 100_000;
// Keep a finite input guard, but do not reject valid large feeds before the
// Memory64 large-feed path can inspect them. The UI mirrors this value.
const MAX_INPUT_BYTES: usize = 2 * 1024 * 1024 * 1024;
const HIGH_CAP: usize = 2_000;
const HIGH_CAP_RULES: &[&str] = &["TRP_020", "OPR_007", "STP_016", "STP_017"];

// ── CachedState ───────────────────────────────────────────────────────────────

#[wasm_bindgen]
pub struct CachedState {
    k1_k5_notices: Vec<gtfs_core::Notice>,
    whitespace_suppressions: WhitespaceSuppressions,
    records: EntityRecords,
    derived: DerivedData,
    file_stats: Vec<FileInfo>,
    partial: PartialReport,
    present_files: HashSet<String>,
}

// ── Helper: invoke the stage callback ────────────────────────────────────────

/// issue #133 — the publication decision and score must see coverage loss. The SAME criterion
/// as native `validate_bytes`; WASM runs its own orchestration, so it must be calculated here too.
fn coverage_complete_of(partial: &PartialReport) -> bool {
    !partial
        .unavailable_files
        .iter()
        .any(|f| gtfs_pipeline::k1_parse::is_certification_critical(f))
        && partial.root_structural_errors.is_empty()
}

fn call_stage(f: &js_sys::Function, name: &str, elapsed_ms: u32) {
    let _ = f.call2(
        &JsValue::NULL,
        &JsValue::from_str(name),
        &JsValue::from_f64(elapsed_ms as f64),
    );
}

// ── Public API ────────────────────────────────────────────────────────────────

/// Lists root .txt files inside a ZIP (metadata only, without parsing).
/// Return value: JSON array — [{name, uncompressed_size}]
#[wasm_bindgen]
pub fn list_zip_files(zip_bytes: Vec<u8>) -> JsValue {
    use std::io::Cursor;

    #[derive(serde::Serialize)]
    struct Entry {
        name: String,
        uncompressed_size: u64,
    }

    if zip_bytes.len() > MAX_INPUT_BYTES {
        return JsValue::from_str("[]");
    }
    let cursor = Cursor::new(zip_bytes);
    let mut archive = match zip::ZipArchive::new(cursor) {
        Ok(a) => a,
        Err(_) => return JsValue::from_str("[]"),
    };
    if archive.len() > MAX_ZIP_ENTRIES {
        return JsValue::from_str("[]");
    }

    let mut entries: Vec<Entry> = Vec::new();
// #46 calibration: collect the central-directory (uncompressed, compressed) sizes of all
// entries WITHOUT DECOMPRESSING them — to tune decompression-guard limits to a real feed.
// The report is printed to the console below.
    let mut calib: Vec<(String, u64, u64)> = Vec::new();
    let mut metadata_bytes = 0usize;
    for i in 0..archive.len() {
        let Ok(f) = archive.by_index(i) else { continue };
        let Ok(name) = f.name().map(|n| n.into_owned()) else { continue };
        metadata_bytes = metadata_bytes.saturating_add(name.len());
        if metadata_bytes > MAX_ZIP_METADATA_BYTES {
            return JsValue::from_str("[]");
        }
        let (u, c) = (f.size(), f.compressed_size());
        calib.push((name.clone(), u, c));
        if name.ends_with(".txt") && !name.contains('/') && !name.contains('\\') {
            entries.push(Entry {
                name,
                uncompressed_size: u,
            });
        }
    }
    log_zip_ratio_report(&calib);

    JsValue::from_str(&serde_json::to_string(&entries).unwrap_or_else(|_| "[]".into()))
}

/// #46 decompression-guard calibration report — per-entry uncompressed/compressed ratio,
/// totals, and recommended upper bounds for `DecompressionLimits`. Uses only central-directory
/// metadata; nothing is decompressed and it takes milliseconds. These values calibrate a
/// TRUSTED real feed — the runtime guard still counts actual decompressed bytes and does not
/// trust the declared size.
#[cfg_attr(feature = "quiet", allow(unused_variables))]
fn log_zip_ratio_report(entries: &[(String, u64, u64)]) {
    if entries.is_empty() {
        return;
    }
    let mib = |b: u64| b as f64 / 1_048_576.0;
    let mut sorted: Vec<&(String, u64, u64)> = entries.iter().collect();
    sorted.sort_by_key(|entry| std::cmp::Reverse(entry.1)); // descending by uncompressed size
    let (mut total_u, mut total_c) = (0u64, 0u64);
    let (mut max_ratio, mut max_ratio_name) = (0.0f64, String::new());
    let (mut max_u, mut max_u_name) = (0u64, String::new());
    let mut lines: Vec<String> = Vec::new();
    for (name, u, c) in &sorted {
        let ratio = if *c == 0 {
            f64::INFINITY
        } else {
            *u as f64 / *c as f64
        };
        total_u += *u;
        total_c += *c;
        if ratio.is_finite() && ratio > max_ratio {
            max_ratio = ratio;
            max_ratio_name = name.clone();
        }
        if *u > max_u {
            max_u = *u;
            max_u_name = name.clone();
        }
        lines.push(format!(
            "  {name}: açılmış {:.1} MiB / sıkıştırılmış {:.1} MiB = {:.1}:1",
            mib(*u),
            mib(*c),
            ratio
        ));
    }
    let feed_ratio = if total_c == 0 {
        0.0
    } else {
        total_u as f64 / total_c as f64
    };
    let report = format!(
        "[zip-calib] #46 decompression-guard kalibrasyon (merkezi-dizin, decompress YOK)\n{}\n\
         — en yuksek oran: {max_ratio_name} = {:.1}:1  -> onerilen max_ratio ~ {} (x3)\n\
         — en buyuk girdi: {max_u_name} = {:.1} MiB  -> onerilen max_entry ~ {:.0} MiB (x2)\n\
         — toplam acilmis: {:.1} MiB (feed orani {:.1}:1)  -> onerilen max_total ~ {:.0} MiB (x2, wasm64 zarfi altinda)\n\
         — MAX_PREALLOC feed'den bagimsiz (64 MiB placeholder makul)",
        lines.join("\n"),
        max_ratio,
        (max_ratio * 3.0).ceil() as u64,
        mib(max_u),
        mib(max_u) * 2.0,
        mib(total_u),
        feed_ratio,
        mib(total_u) * 2.0,
    );
    wasm_log!(report);

    // Early warning (issue #46, suggested by @Whatsonyourmind): if the measured worst ratio
    // reaches half the active `max_ratio` cap, measure again BEFORE a false trip. The threshold
    // follows the cap (and therefore tracks a constant change automatically).
    let cap = gtfs_pipeline::decompress_guard::DEFAULT_DECOMPRESSION_LIMITS.max_ratio as f64;
    if max_ratio >= cap / 2.0 {
        let warn = format!(
            "[zip-calib] UYARI: en yuksek oran {max_ratio_name} = {:.1}:1, aktif max_ratio cap'inin ({:.0}) yarisini asti \
             -> false-trip'ten once decompression-guard sabitlerini yeniden olc (bkz. issue #46).",
            max_ratio, cap
        );
        wasm_warn!(warn);
    }
}

/// Returns the file statistics in CachedState as JSON.
/// [{name, rows, bytes}]
#[wasm_bindgen]
pub fn get_cached_file_stats(cache: &CachedState) -> JsValue {
    JsValue::from_str(&serde_json::to_string(&cache.file_stats).unwrap_or_else(|_| "[]".into()))
}

/// Returns one shape's ordered `[[lat, lon], ...]` point list as JSON.
/// In large-feed mode, name_index.shape_coords is not serialized up front; when the UI
/// clicks a map icon, it fetches only the relevant shape HERE (records remain live in memory).
/// Returns `[]` when it is absent or has no geometry.
#[wasm_bindgen]
pub fn shape_coords_of(cache: &CachedState, shape_id: String) -> JsValue {
    let mut pts: Vec<(u32, f64, f64)> = cache
        .records
        .shapes
        .iter()
        .filter(|s| cache.records.shape_interns.id(s) == shape_id.as_str())
        .filter_map(|s| {
            Some((
                s.shape_pt_sequence().unwrap_or(0),
                s.shape_pt_lat()?,
                s.shape_pt_lon()?,
            ))
        })
        .take(MAX_SHAPE_COORDS + 1)
        .collect();
    if pts.len() > MAX_SHAPE_COORDS {
        return JsValue::from_str(
            r#"{"Fatal":{"code":"ResourceLimit","message":"Shape geometry exceeds the SDK safety limit."}}"#,
        );
    }
    pts.sort_unstable_by_key(|&(seq, _, _)| seq);
    let coords: Vec<[f64; 2]> = pts.into_iter().map(|(_, lat, lon)| [lat, lon]).collect();
    JsValue::from_str(&serde_json::to_string(&coords).unwrap_or_else(|_| "[]".into()))
}

/// Full pipeline: K1–K7 in one call (for flows without a config panel).
/// `today` = the browser's local date; use [`validate_with_today`] for deterministic output.
#[wasm_bindgen]
pub fn validate(zip_bytes: Vec<u8>, config_delta_json: String) -> JsValue {
    validate_with_today(zip_bytes, config_delta_json, today_yyyymmdd())
}

/// The SAME pipeline as [`validate`]; the only difference is that `today` is supplied externally.
///
/// Calendar/service rules (CAL_*, OPR_*) are relative to "today"; `validate` reads it from
/// `js_sys::Date`, so output depends on the day it runs. For golden baseline generation and
/// tests, fix the date to obtain deterministic results (the equivalent of the native CLI's
/// `--today` flag).
///
/// `today` format is `YYYYMMDD` (e.g. 20260716). An invalid value returns Fatal.
#[wasm_bindgen]
pub fn validate_with_today(zip_bytes: Vec<u8>, config_delta_json: String, today: u32) -> JsValue {
    if zip_bytes.len() > MAX_INPUT_BYTES {
        return fatal_js(
            FatalError::new(
                FatalCode::ResourceLimit,
                "Input ZIP exceeds the SDK safety limit.",
            )
            .variant("input_size"),
        );
    }
    if !is_valid_yyyymmdd(today) {
        return fatal_js(
            FatalError::new(
                FatalCode::InvalidInput,
                format!("Geçersiz 'today' değeri: {today} (beklenen YYYYMMDD, ör. 20260716)"),
            )
            .variant("today")
            .param("today", today),
        );
    }
    let config = match parse_config(&config_delta_json) {
        Ok(c) => c,
        Err(e) => return fatal_js(e),
    };
    match run_full_pipeline(&zip_bytes, &config, today) {
        ValidateResult::Fatal(err) => fatal_js(err),
        result => to_js(&result),
    }
}

/// Coarse YYYYMMDD validation: return Fatal early instead of passing a meaningless date to the
/// pipeline and silently producing a nonsensical calendar result.
fn is_valid_yyyymmdd(v: u32) -> bool {
    let (y, m, d) = (v / 10000, (v / 100) % 100, v % 100);
    (1970..=9999).contains(&y) && (1..=12).contains(&m) && (1..=31).contains(&d)
}

/// Runs K1–K5.
/// `on_stage(name, elapsed_ms)` is called after each of K1/K2/K3/K4/K5 finishes.
#[wasm_bindgen]
pub fn prepare(
    zip_bytes: Vec<u8>,
    config_delta_json: String,
    on_stage: &js_sys::Function,
) -> Result<CachedState, JsValue> {
    prepare_with_today(zip_bytes, config_delta_json, on_stage, today_yyyymmdd())
}

/// Deterministic K1–K5 preparation; `today` is used by K4 cross-reference date checks.
#[wasm_bindgen]
pub fn prepare_with_today(
    zip_bytes: Vec<u8>,
    config_delta_json: String,
    on_stage: &js_sys::Function,
    today: u32,
) -> Result<CachedState, JsValue> {
    if zip_bytes.len() > MAX_INPUT_BYTES {
        return Err(fatal_js(
            FatalError::new(
                FatalCode::ResourceLimit,
                "Input ZIP exceeds the SDK safety limit.",
            )
            .variant("input_size"),
        ));
    }
    if !is_valid_yyyymmdd(today) {
        return Err(fatal_js(
            FatalError::new(
                FatalCode::InvalidInput,
                format!("Geçersiz 'today' değeri: {today} (beklenen YYYYMMDD, ör. 20260716)"),
            )
            .variant("today")
            .param("today", today),
        ));
    }
    // Service-day normalization happens in K2 (run_k1_k5), so config is required here.
    // A service_day_start_hour change does not affect cached records → reload the files.
    let config = parse_config(&config_delta_json).map_err(fatal_js)?;
    wasm_log!(format!(
        "[mem] K0 input Vec: len={} cap={} bytes ({:.1} MB)",
        zip_bytes.len(),
        zip_bytes.capacity(),
        zip_bytes.capacity() as f64 / 1_048_576.0
    ));
    run_k1_k5(&zip_bytes, &config, on_stage, today).map_err(fatal_js)
}

/// Runs K6+K7 from the cache.
/// `on_stage(name, elapsed_ms)` is called after K6 and K7 finish.
#[wasm_bindgen]
pub fn rerun_k6_k7(
    cache: &CachedState,
    config_delta_json: String,
    on_stage: &js_sys::Function,
) -> JsValue {
    rerun_k6_k7_with_today(cache, config_delta_json, on_stage, today_yyyymmdd())
}

/// Deterministic K6+K7 rerun; `today` is used by calendar and service checks.
#[wasm_bindgen]
pub fn rerun_k6_k7_with_today(
    cache: &CachedState,
    config_delta_json: String,
    on_stage: &js_sys::Function,
    today: u32,
) -> JsValue {
    if !is_valid_yyyymmdd(today) {
        return fatal_js(
            FatalError::new(
                FatalCode::InvalidInput,
                format!("Geçersiz 'today' değeri: {today} (beklenen YYYYMMDD, ör. 20260716)"),
            )
            .variant("today")
            .param("today", today),
        );
    }
    rerun_k6_k7_inner(cache, &config_delta_json, on_stage, today)
}

fn rerun_k6_k7_inner(
    cache: &CachedState,
    config_delta_json: &str,
    on_stage: &js_sys::Function,
    today: u32,
) -> JsValue {
    let config = match parse_config(config_delta_json) {
        Ok(c) => c,
        Err(e) => return fatal_js(e),
    };

    let t = js_sys::Date::now();
    t_start!("K6-analytics");
    let mut partial = cache.partial.clone();
    let unavailable_files = partial.unavailable_files.clone();
    let availability = FileAvailability::from_k1(&cache.present_files, &unavailable_files);
    let k6 = analyze_k6_with_files(
        &cache.records,
        &cache.derived,
        &config,
        today,
        &availability,
    );
    if !partial.is_empty() {
        partial.extend_skipped_checks(k6.skipped_checks);
    }
    t_end!("K6-analytics");
    call_stage(on_stage, "K6", (js_sys::Date::now() - t) as u32);

    let mut all_notices = cache.k1_k5_notices.clone();
    all_notices.extend(k6.notices);
    // Because K2 and K6 subcases enter the same summary, aggregate AFTER adding K6 to the
    // cached raw K1–K5 notices (the cache remains raw).
    aggregate_feed_level_notices(&mut all_notices, &cache.records, &cache.derived);
    // The SAME report scope as native `validate_bytes` (STP_033 uniform-fare exception +
    // `disabled_rule_ids`); must be applied before cap and scoring.
    apply_report_scope(&mut all_notices, &cache.records, &config);
    wasm_log!(format!(
        "[mem] after-K6 (pre-cap): {} notices, {:.1} MB",
        all_notices.len(),
        mem_mb()
    ));
    wasm_log!(format!(
        "[rules] after-K6 top: {}",
        top_rules_str(&all_notices)
    ));

    let (mut all_notices, real_totals, mut agency_breakdown) = prepare_count_cap(
        all_notices,
        &cache.records,
        &cache.derived,
        cache.whitespace_suppressions.clone(),
    );
    #[cfg(feature = "sdk-en")]
    i18n::translate_notices(&mut all_notices);
    all_notices.shrink_to_fit();
    log_mem("after-cap");

    let t = js_sys::Date::now();
    let coverage_complete = coverage_complete_of(&partial);
    let k7 = report_k7_prepared(
        all_notices,
        &cache.records,
        &cache.derived,
        cache.file_stats.clone(),
        coverage_complete,
    );
    call_stage(on_stage, "K7", (js_sys::Date::now() - t) as u32);
    index_notices(&mut agency_breakdown, &k7.notices, &cache.records);

    let capped_totals = build_capped_totals(&real_totals);
    // Filter map data by notices (large-feed mode) → build name_index first.
    let name_index = build_name_index(&cache.records, &k7.notices);
    let result = ValidateResult::Ok(ValidationResult {
        status: if partial.is_empty() {
            ValidationStatus::Complete
        } else {
            ValidationStatus::Partial
        },
        partial: (!partial.is_empty()).then_some(partial),
        notices: k7.notices,
        reports: k7.reports,
        metrics: k7.metrics,
        name_index,
        capped_totals,
        agency_breakdown,
        feed_summary: build_feed_summary(&cache.records),
    });
    // #15: serializing the result to JS (to_js) is the final memory spike on a large feed;
    // if this line is printed but nothing after it, serialization ran out of memory.
    wasm_log!(format!(
        "[mem] before to_js (serialize): {:.1} MB",
        mem_mb()
    ));
    let js = to_js(&result);
    wasm_log!(format!("[mem] after to_js: {:.1} MB", mem_mb()));
    js
}

// ── Internal pipeline helpers ────────────────────────────────────────────────

fn run_full_pipeline(zip_bytes: &[u8], config: &ValidatorConfig, today: u32) -> ValidateResult {
    t_start!("K1-parse");
    let mut k1 = match parse_with_limits(
        zip_bytes,
        config,
        gtfs_pipeline::decompress_guard::SDK_DECOMPRESSION_LIMITS,
        Some(gtfs_pipeline::decompress_guard::SDK_MAX_BUFFERED_FILE_BYTES),
    ) {
        Ok(r) => r,
        Err(e) => return ValidateResult::Fatal(e),
    };
    let mut partial = k1.partial;
    let stops_present = k1.present_files.contains("stops.txt");
    let stops_empty = k1
        .files
        .get("stops.txt")
        .is_some_and(|file| file.rows.is_empty());
    let has_gtfs_jp_file = GTFS_JP_FILES
        .iter()
        .any(|file| k1.present_files.contains(*file));
    let has_pattern_jp_file = k1.present_files.contains("pattern_jp.txt");
    let whitespace_root_files: HashSet<String> = k1
        .notices
        .iter()
        .filter(|notice| notice.rule_id == "DQ_016")
        .filter_map(|notice| notice.file.clone())
        .collect();
    t_end!("K1-parse");
    let mut file_stats = collect_file_stats(&k1.files);

    t_start!("K2-validate");
    // #185: DO NOT ENFORCE a row budget. With `Some(max_file_rows)`, K2 silently truncated
    // streamed files at 1,000,000 rows; references left by the truncation were then reported
    // as feed defects (VBB: 250,407 TRP_004 + 220,752 fabricated HIGH XFL_002 findings).
    // v0.9.7 applied no row limit; restore that behavior. `max_file_rows` remains only the
    // ARC_022 threshold.
    let mut k2 = validate_k2_with_whitespace_roots(
        k1.files,
        Some(zip_bytes),
        config,
        None,
        Some(has_gtfs_jp_file),
        Some(&whitespace_root_files),
    ); // #15 W2 + #38: stop_times ZIP stream
    reconcile_flex_stop_requirement(
        &mut k1.notices,
        &mut k2.notices,
        &mut partial,
        stops_present,
        stops_empty,
        &k1.valid_geojson_location_ids,
        &k2.records,
    );
    let unavailable_files = partial.unavailable_files.clone();
    let availability = FileAvailability::from_k1(&k1.present_files, &unavailable_files);
    t_end!("K2-validate");
    // K1 deliberately retains headers for unknown custom files only; carry that
    // lightweight inventory into K4 so GTFS-JP namespace checks see the same
    // file/field set as the native pipeline.
    k2.records.gtfs_jp_namespace_headers.extend(
        k1.custom_file_headers
            .iter()
            .map(|(name, headers)| (name.clone(), headers.clone())),
    );
    k2.records.has_pattern_jp_file |= has_pattern_jp_file;
    // Normalize trips crossing midnight (00:xx) to service-day notation (24:xx) before
    // K3–K6. This is the same step as pipeline::validate_bytes; WASM must call it here
    // because it runs its own orchestration.
    k2.records
        .stop_times_index
        .normalize_service_day(config.service_day_start_hour);
    // K1 rows remain empty for streamed files; overwrite them when K2 has counters.
    // When a new file is streamed, adding it to streaming_row_counts in k2/mod.rs is enough.
    for fi in file_stats.iter_mut() {
        if let Some(&count) = k2.records.streaming_row_counts.get(fi.name.as_str()) {
            fi.rows = count as u32;
        }
    }

    t_start!("K3-entity-map");
    let mut k3 = build_entity_map(&k2.records);
    k3.entity_map.geojson_location_ids = k1.geojson_location_ids.clone();
    k3.entity_map.geojson_geometries = k1.geojson_geometries.clone();
    t_end!("K3-entity-map");

    t_start!("K4-cross-ref");
    let k4 = check_cross_ref_with_whitespace_roots(
        &k2.records,
        &k3.entity_map,
        today,
        &availability,
        Some(&whitespace_root_files),
    );
    t_end!("K4-cross-ref");

    // #15: trip_stop_set (~226 MB on a large feed) is used only by K4 → release it.
    k2.records.stop_times_index.trip_stop_set = Default::default();

    t_start!("K5-derived");
    let k5 = build_derived_with_files(&k2.records, &k3.entity_map, &availability);
    t_end!("K5-derived");

    t_start!("K6-analytics");
    let k6 = analyze_k6_with_files(&k2.records, &k5.derived, config, today, &availability);
    t_end!("K6-analytics");

    let mut all_notices = k1.notices;
    all_notices.extend(k2.notices);
    all_notices.extend(k3.notices);
    if !partial.is_empty() {
        partial.extend_skipped_checks(k4.skipped_checks);
        partial.extend_skipped_checks(k5.skipped_checks);
        partial.extend_skipped_checks(k6.skipped_checks);
    }
    all_notices.extend(k4.notices);
    all_notices.extend(k5.notices);
    all_notices.extend(k6.notices);
    // The SAME feed-level aggregation as native `validate_bytes`; the registry defines these
    // rules at `Feed` dedup level, so counts are lost if aggregation happens after the cap.
    aggregate_feed_level_notices(&mut all_notices, &k2.records, &k5.derived);
    // The SAME report scope as native `validate_bytes` (STP_033 uniform-fare exception +
    // `disabled_rule_ids`); it must be applied before cap and scoring.
    apply_report_scope(&mut all_notices, &k2.records, config);

    // 1) Suppress whitespace derivatives, count real totals and the agency breakdown after dedup,
    // then apply the per-rule display cap (the same counting point as native).
    let mut whitespace_suppressions = k2.whitespace_suppressions;
    whitespace_suppressions.merge(k4.whitespace_suppressions);
    let (mut all_notices, real_totals, mut agency_breakdown) = prepare_count_cap(
        all_notices,
        &k2.records,
        &k5.derived,
        whitespace_suppressions,
    );
    #[cfg(feature = "sdk-en")]
    i18n::translate_notices(&mut all_notices);
    all_notices.shrink_to_fit();

    let coverage_complete = coverage_complete_of(&partial);
    let k7 = report_k7_prepared(
        all_notices,
        &k2.records,
        &k5.derived,
        file_stats,
        coverage_complete,
    );
    index_notices(&mut agency_breakdown, &k7.notices, &k2.records);
    // 4) Scale score delta by the real-total ratio for rules that hit the cap.
    let capped_totals = build_capped_totals(&real_totals);
    let name_index = build_name_index(&k2.records, &k7.notices);
    ValidateResult::Ok(ValidationResult {
        status: if partial.is_empty() {
            ValidationStatus::Complete
        } else {
            ValidationStatus::Partial
        },
        partial: (!partial.is_empty()).then_some(partial),
        notices: k7.notices,
        reports: k7.reports,
        metrics: k7.metrics,
        name_index,
        capped_totals,
        agency_breakdown,
        feed_summary: build_feed_summary(&k2.records),
    })
}

fn run_k1_k5(
    zip_bytes: &[u8],
    config: &ValidatorConfig,
    on_stage: &js_sys::Function,
    today: u32,
) -> Result<CachedState, FatalError> {
    wasm_log!(format!(
        "[mem] K0-start (zip {:.1} MB): {:.1} MB",
        zip_bytes.len() as f64 / 1_048_576.0,
        mem_mb()
    ));

    let mut t = js_sys::Date::now();
    t_start!("K1-parse");
    let mut k1 = parse_with_limits(
        zip_bytes,
        config,
        gtfs_pipeline::decompress_guard::SDK_DECOMPRESSION_LIMITS,
        Some(gtfs_pipeline::decompress_guard::SDK_MAX_BUFFERED_FILE_BYTES),
    )?;
    t_end!("K1-parse");
    call_stage(on_stage, "K1", (js_sys::Date::now() - t) as u32);
    log_mem("after-K1-parse");
    let mut partial = k1.partial;
    let stops_present = k1.present_files.contains("stops.txt");
    let stops_empty = k1
        .files
        .get("stops.txt")
        .is_some_and(|file| file.rows.is_empty());
    let has_gtfs_jp_file = GTFS_JP_FILES
        .iter()
        .any(|file| k1.present_files.contains(*file));
    let has_pattern_jp_file = k1.present_files.contains("pattern_jp.txt");
    let whitespace_root_files: HashSet<String> = k1
        .notices
        .iter()
        .filter(|notice| notice.rule_id == "DQ_016")
        .filter_map(|notice| notice.file.clone())
        .collect();
    let mut file_stats = collect_file_stats(&k1.files);

    t = js_sys::Date::now();
    t_start!("K2-validate");
    // #185: DO NOT ENFORCE a row budget. With `Some(max_file_rows)`, K2 silently truncated
    // streamed files at 1,000,000 rows; references left by the truncation were then reported
    // as feed defects (VBB: 250,407 TRP_004 + 220,752 fabricated HIGH XFL_002 findings).
    // v0.9.7 applied no row limit; restore that behavior. `max_file_rows` remains only the
    // ARC_022 threshold.
    let mut k2 = validate_k2_with_whitespace_roots(
        k1.files,
        Some(zip_bytes),
        config,
        None,
        Some(has_gtfs_jp_file),
        Some(&whitespace_root_files),
    ); // #15 W2 + #38: stop_times ZIP stream
    reconcile_flex_stop_requirement(
        &mut k1.notices,
        &mut k2.notices,
        &mut partial,
        stops_present,
        stops_empty,
        &k1.valid_geojson_location_ids,
        &k2.records,
    );
    let unavailable_files = partial.unavailable_files.clone();
    let availability = FileAvailability::from_k1(&k1.present_files, &unavailable_files);
    t_end!("K2-validate");
    // Keep the cached WASM path in parity with the full native/WASM pipeline:
    // unknown custom headers are needed by the GTFS-JP namespace rule.
    k2.records.gtfs_jp_namespace_headers.extend(
        k1.custom_file_headers
            .iter()
            .map(|(name, headers)| (name.clone(), headers.clone())),
    );
    k2.records.has_pattern_jp_file |= has_pattern_jp_file;
    // Midnight (00:xx) → service-day (24:xx) normalization — before K3–K6 (see the first path).
    k2.records
        .stop_times_index
        .normalize_service_day(config.service_day_start_hour);
    call_stage(on_stage, "K2", (js_sys::Date::now() - t) as u32);
    log_mem("after-K2-validate");
    // K1 rows remain empty for streamed files; overwrite them when K2 has counters.
    // When a new file is streamed, adding it to streaming_row_counts in k2/mod.rs is enough.
    for fi in file_stats.iter_mut() {
        if let Some(&count) = k2.records.streaming_row_counts.get(fi.name.as_str()) {
            fi.rows = count as u32;
        }
    }

    t = js_sys::Date::now();
    t_start!("K3-entity-map");
    let mut k3 = build_entity_map(&k2.records);
    k3.entity_map.geojson_location_ids = k1.geojson_location_ids.clone();
    k3.entity_map.geojson_geometries = k1.geojson_geometries.clone();
    t_end!("K3-entity-map");
    call_stage(on_stage, "K3", (js_sys::Date::now() - t) as u32);
    log_mem("after-K3-entity-map");

    t = js_sys::Date::now();
    t_start!("K4-cross-ref");
    let k4 = check_cross_ref_with_whitespace_roots(
        &k2.records,
        &k3.entity_map,
        today,
        &availability,
        Some(&whitespace_root_files),
    );
    t_end!("K4-cross-ref");
    call_stage(on_stage, "K4", (js_sys::Date::now() - t) as u32);
    // #15: trip_stop_set (~226 MB on a large feed) is used only by K4; release it from cached
    // records before K6 continuation (potentially reducing the K6 peak by ~226 MB).
    k2.records.stop_times_index.trip_stop_set = Default::default();
    log_mem("after-K4-cross-ref");

    t = js_sys::Date::now();
    t_start!("K5-derived");
    let k5 = build_derived_with_files(&k2.records, &k3.entity_map, &availability);
    t_end!("K5-derived");
    call_stage(on_stage, "K5", (js_sys::Date::now() - t) as u32);
    log_mem("after-K5-derived");

    let mut k1_k5_notices = k1.notices;
    k1_k5_notices.extend(k2.notices);
    k1_k5_notices.extend(k3.notices);
    if !partial.is_empty() {
        partial.extend_skipped_checks(k4.skipped_checks);
        partial.extend_skipped_checks(k5.skipped_checks);
    }
    k1_k5_notices.extend(k4.notices);
    k1_k5_notices.extend(k5.notices);
    wasm_log!(format!(
        "[mem] K1-K5 done: {} notices, {:.1} MB",
        k1_k5_notices.len(),
        mem_mb()
    ));
    wasm_log!(format!(
        "[rules] K1-K5 top: {}",
        top_rules_str(&k1_k5_notices)
    ));

    let mut whitespace_suppressions = k2.whitespace_suppressions;
    whitespace_suppressions.merge(k4.whitespace_suppressions);

    Ok(CachedState {
        k1_k5_notices,
        whitespace_suppressions,
        records: k2.records,
        derived: k5.derived,
        file_stats,
        partial,
        present_files: k1.present_files,
    })
}

fn parse_config(delta_json: &str) -> Result<ValidatorConfig, FatalError> {
    if delta_json.is_empty() || delta_json == "{}" {
        return Ok(ValidatorConfig::default());
    }
    if delta_json.len() > 4 * 1024 * 1024 {
        return Err(FatalError::new(
            FatalCode::ResourceLimit,
            "Config JSON exceeds the 4 MiB safety limit.",
        )
        .variant("config_size"));
    }
    let config = merge_delta(&ValidatorConfig::default(), delta_json).map_err(|e| {
        FatalError::new(FatalCode::InvalidInput, format!("Config parse hatası: {e}"))
            .variant("config_parse")
            .param("detail", &e)
    })?;
    // The same gate as native `validate_bytes`: unknown/Spec rules cannot be disabled.
    check_rule_scope(&config)?;
    Ok(config)
}

// Per-rule notice cap:
// A rule can fire on many rows (e.g. STM_017 for every stop_time row).
// Without a limit, WASM memory use explodes and UI rendering takes longer.
// The default cap is PER_RULE_CAP (500); rules naturally high on real feeds are raised
// to 2,000 in HIGH_CAP_RULES.
// Return value: {rule_id → real_total} for rules that hit the cap.
fn cap_for_rule(rule_id: &str) -> usize {
    if HIGH_CAP_RULES.contains(&rule_id) {
        HIGH_CAP
    } else {
        PER_RULE_CAP
    }
}

#[cfg_attr(feature = "quiet", allow(dead_code))]
fn count_totals(notices: &[gtfs_core::Notice]) -> std::collections::HashMap<String, u32> {
    let mut m = std::collections::HashMap::new();
    for n in notices {
        *m.entry(n.rule_id.clone()).or_insert(0u32) += 1;
    }
    m
}

// #15 Mode A diagnosis: returns the first 10 rules producing the most notices as "RULE=count".
#[cfg_attr(feature = "quiet", allow(dead_code))]
fn top_rules_str(notices: &[gtfs_core::Notice]) -> String {
    let m = count_totals(notices);
    let mut v: Vec<(&String, &u32)> = m.iter().collect();
    v.sort_by(|a, b| b.1.cmp(a.1).then(a.0.cmp(b.0)));
    v.iter()
        .take(10)
        .map(|(r, c)| format!("{r}={c}"))
        .collect::<Vec<_>>()
        .join(" ")
}

fn build_capped_totals(
    real_totals: &std::collections::HashMap<String, u32>,
) -> std::collections::BTreeMap<String, u32> {
    real_totals
        .iter()
        .filter_map(|(rule_id, &total)| {
            if total > cap_for_rule(rule_id) as u32 {
                Some((rule_id.clone(), total))
            } else {
                None
            }
        })
        .collect()
}

/// K7 preparation (suppress whitespace derivatives) → dedup + agency counting + cap in one pass.
/// The SAME counting point as native (#2201): suppressed derivatives are not counted, and the
/// cap comes AFTER counting. Dedup does not allocate a second full-size `Vec`.
fn prepare_count_cap(
    notices: Vec<gtfs_core::Notice>,
    records: &EntityRecords,
    derived: &DerivedData,
    suppressions: WhitespaceSuppressions,
) -> (
    Vec<gtfs_core::Notice>,
    std::collections::HashMap<String, u32>,
    AgencyBreakdown,
) {
    let prepared = prepare_k7_notices(notices, records, derived, true, suppressions);
    let mut counter = AgencyCounter::new(records);
    let (kept, totals) = gtfs_pipeline::k7_reporting::dedup_and_cap_by_rule_visit(
        prepared,
        cap_for_rule,
        |notice| counter.add(notice),
    );
    let mut breakdown = counter.finish(true);
    set_displayed(&mut breakdown, &kept);
    (kept, totals, breakdown)
}

#[cfg(test)]
fn cap_per_rule(notices: &mut Vec<gtfs_core::Notice>) -> std::collections::HashMap<String, u32> {
    // Determinism + full representation: dedup first (stable order, distinct entities), then cap.
    // The cap applies to distinct entities, not raw notices → displayed representatives are
    // thread-independent and do not cluster (e.g. distinct route count is preserved even when
    // OPR_005 hits the cap). Dedup uses notice_order_key for stable ordering.
    // Dedup + cap happen in one pass: preserve real distinct totals without allocating a second
    // full-size Vec for millions of deduplicated Notices.
    let (kept, real_totals) =
        gtfs_pipeline::k7_reporting::dedup_and_cap_by_rule(std::mem::take(notices), cap_for_rule);
    *notices = kept;
    real_totals
}

// #15 Step-1 instrumentation: WASM linear-memory high water (MB).
// WASM memory only GROWS (pages are not returned) → the value read after each stage is the
// peak usage reached so far; the delta between stages is that stage's persistent allocation.
// On OOM, the feed dies at the stage that overflows it → the last log shows how far it got.
#[cfg_attr(feature = "quiet", allow(dead_code))]
fn mem_mb() -> f64 {
    use wasm_bindgen::JsCast;
    let buf = wasm_bindgen::memory()
        .unchecked_into::<js_sys::WebAssembly::Memory>()
        .buffer();
    // js_sys::ArrayBuffer::byte_length() returns u32 and wraps above 4 GiB in Memory64
    // (e.g. 5313 MB → 1217 MB). Read the WebIDL Number value directly as f64 with Reflect;
    // the 16 GiB web-side limit is then represented exactly.
    let bytes = js_sys::Reflect::get(buf.as_ref(), &JsValue::from_str("byteLength"))
        .ok()
        .and_then(|v| v.as_f64())
        .unwrap_or(0.0);
    bytes / 1_048_576.0
}

#[cfg_attr(feature = "quiet", allow(unused_variables))]
fn log_mem(stage: &str) {
    wasm_log!(format!("[mem] {stage}: {:.1} MB", mem_mb()));
}

fn today_yyyymmdd() -> u32 {
    let now = js_sys::Date::new_0();
    let y = now.get_full_year();
    let m = now.get_month() + 1;
    let d = now.get_date();
    y * 10000 + m * 100 + d
}

/// Converts a Fatal result to JS. In the SDK build (`sdk-en`), the message is written from
/// the English template using the same model as notices; a fatal without a template preserves
/// the pipeline text.
fn fatal_js(err: FatalError) -> JsValue {
    #[cfg(feature = "sdk-en")]
    let err = i18n::translate_fatal(err);
    to_js(&ValidateResult::Fatal(err))
}

fn to_js<T: serde::Serialize>(value: &T) -> JsValue {
    match serde_json::to_string(value) {
        Ok(json) => JsValue::from_str(&json),
        Err(e) => JsValue::from_str(&format!(r#"{{"error":"serialize failed: {e}"}}"#)),
    }
}

#[cfg(test)]
mod tests {
    use super::{cap_per_rule, is_valid_yyyymmdd};
    use gtfs_core::{EntityType, Notice, RuleClass, Severity};

    #[test]
    fn yyyymmdd_validation_accepts_real_dates_and_rejects_junk() {
        assert!(is_valid_yyyymmdd(20260716));
        assert!(is_valid_yyyymmdd(19700101));
        assert!(is_valid_yyyymmdd(20261231));
        // Month/day fields out of range.
        assert!(!is_valid_yyyymmdd(20261301), "month 13 should be rejected");
        assert!(!is_valid_yyyymmdd(20260732), "day 32 should be rejected");
        assert!(!is_valid_yyyymmdd(20260700), "day 0 should be rejected");
        assert!(!is_valid_yyyymmdd(20260016), "month 0 should be rejected");
        // Year field: before the Unix epoch and fewer than four digits.
        assert!(!is_valid_yyyymmdd(19691231), "dates before 1970 should be rejected");
        assert!(!is_valid_yyyymmdd(0), "zero should be rejected");
        assert!(
            !is_valid_yyyymmdd(20260716 / 10),
            "a short/malformed number should be rejected"
        );
    }

    fn trip_notice(id: &str) -> Notice {
        Notice {
            id: id.to_string(),
            rule_id: "TRP_022".to_string(),
            severity: Severity::Yuksek,
            rule_class: RuleClass::Spec,
            entity_type: EntityType::Trip,
            entity_id: Some("trip-a".to_string()),
            scope_key: Some("trip-a".to_string()),
            file: Some("trips.txt".to_string()),
            line: None,
            field: Some("block_id".to_string()),
            observed_value: None,
            expected_value: None,
            details: None,
            whitespace_derived: false,
            whitespace_candidate: false,
            title: "test".to_string().into(),
            message: "test".to_string(),
            remediation: "test".to_string().into(),
            blocks: Vec::new(),
            base_effort: 1,
            service_id: None,
            agency_distribution: None,
        }
    }

    #[test]
    fn feed_level_aggregation_runs_before_dedup_cap() {
        // The registry defines TRP_005 at the `Feed` dedup level. If aggregation is skipped,
        // dedup selects one raw-row notice: the count falls to 1 and `observed_value` ("2")
        // is incorrectly read as "2 trips…" in the UI.
        let raw = |trip: &str| {
            let mut n = trip_notice(trip);
            n.rule_id = "TRP_005".to_string();
            n.entity_id = Some(trip.to_string());
            n.scope_key = Some(trip.to_string());
            n.field = Some("direction_id".to_string());
            n.observed_value = Some("2".to_string());
            n
        };
        let mut notices = vec![raw("t1"), raw("t2"), raw("t3")];
        gtfs_pipeline::aggregate_feed_level_notices(
            &mut notices,
            &gtfs_pipeline::EntityRecords::default(),
            &gtfs_pipeline::DerivedData::default(),
        );
        let totals = cap_per_rule(&mut notices);
        assert_eq!(notices.len(), 1);
        assert_eq!(totals.get("TRP_005"), Some(&1));
        let n = &notices[0];
        assert_eq!(n.entity_type, EntityType::Feed);
        assert_eq!(n.observed_value.as_deref(), Some("3"));
        assert_eq!(
            n.details
                .as_ref()
                .and_then(|d| d.get("affected_trips"))
                .map(String::as_str),
            Some("3")
        );
    }

    #[test]
    fn cap_totals_count_deduped_not_raw_emissions() {
        let mut notices = vec![trip_notice("a"), trip_notice("b")];
        let totals = cap_per_rule(&mut notices);
        assert_eq!(notices.len(), 1);
        assert_eq!(totals.get("TRP_022"), Some(&1));
    }
}
