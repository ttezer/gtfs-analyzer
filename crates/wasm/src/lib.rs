use wasm_bindgen::prelude::*;

use gtfs_config::{merge_delta, ValidatorConfig};
use gtfs_core::{FatalCode, FatalError, ValidateResult, ValidationResult};
use gtfs_pipeline::{
    analyze_k6, build_derived, build_entity_map, build_name_index, check_cross_ref,
    collect_file_stats, parse, report_k7, validate_k2, DerivedData, EntityRecords, FileInfo,
};

macro_rules! t_start {
    ($label:expr) => {
        web_sys::console::time_with_label($label)
    };
}
macro_rules! t_end {
    ($label:expr) => {
        web_sys::console::time_end_with_label($label)
    };
}

#[wasm_bindgen(start)]
pub fn wasm_init() {
    web_sys::console::warn_1(&"[GTFS WASM] binary v6 yüklendi".into());
}

// `threads` feature açıkken JS'e `init_thread_pool(n)` verir; UI doğrulamadan önce
// `await init_thread_pool(navigator.hardwareConcurrency)` çağırıp rayon havuzunu kurar.
// Bu çağrı yapılana kadar (veya feature kapalıysa) rayon::scope tek thread'de çalışır.
#[cfg(feature = "threads")]
pub use wasm_bindgen_rayon::init_thread_pool;

// ── Sabitler ─────────────────────────────────────────────────────────────────

const PER_RULE_CAP: usize = 500;
// Yumuşak uyarı eşiği — AŞILSA BİLE doğrulamayı DURDURMAZ; yalnızca konsola uyarı yazar.
// Notice sayısı cap dışı gerçek bir rakamdır; gösterim/render kural başına cap_per_rule ile sınırlanır.
const NOTICE_LIMIT: usize = 1_000_000;
const HIGH_CAP: usize = 2_000;
const HIGH_CAP_RULES: &[&str] = &["TRP_020", "OPR_007", "STP_016", "STP_017"];

// ── CachedState ───────────────────────────────────────────────────────────────

#[wasm_bindgen]
pub struct CachedState {
    k1_k5_notices: Vec<gtfs_core::Notice>,
    records: EntityRecords,
    derived: DerivedData,
    file_stats: Vec<FileInfo>,
}

// ── Yardımcı: aşama callback çağrısı ─────────────────────────────────────────

fn call_stage(f: &js_sys::Function, name: &str, elapsed_ms: u32) {
    let _ = f.call2(
        &JsValue::NULL,
        &JsValue::from_str(name),
        &JsValue::from_f64(elapsed_ms as f64),
    );
}

// ── Public API ────────────────────────────────────────────────────────────────

/// ZIP içindeki kök .txt dosyalarını listeler (ayrıştırma yapmadan, sadece metadata).
/// Dönen değer: JSON dizisi — [{name, uncompressed_size}]
#[wasm_bindgen]
pub fn list_zip_files(zip_bytes: &[u8]) -> JsValue {
    use std::io::Cursor;

    #[derive(serde::Serialize)]
    struct Entry {
        name: String,
        uncompressed_size: u64,
    }

    let cursor = Cursor::new(zip_bytes);
    let mut archive = match zip::ZipArchive::new(cursor) {
        Ok(a) => a,
        Err(_) => return JsValue::from_str("[]"),
    };

    let mut entries: Vec<Entry> = Vec::new();
    for i in 0..archive.len() {
        let Ok(f) = archive.by_index(i) else { continue };
        let name = f.name().to_string();
        if name.ends_with(".txt") && !name.contains('/') && !name.contains('\\') {
            entries.push(Entry { name, uncompressed_size: f.size() });
        }
    }

    JsValue::from_str(&serde_json::to_string(&entries).unwrap_or_else(|_| "[]".into()))
}

/// CachedState içindeki dosya istatistiklerini JSON olarak döner.
/// [{name, rows, bytes}]
#[wasm_bindgen]
pub fn get_cached_file_stats(cache: &CachedState) -> JsValue {
    JsValue::from_str(&serde_json::to_string(&cache.file_stats).unwrap_or_else(|_| "[]".into()))
}

/// Tam pipeline: K1–K7 tek seferde (config panel kullanmayan akış için).
#[wasm_bindgen]
pub fn validate(zip_bytes: &[u8], config_delta_json: &str) -> JsValue {
    let today = today_yyyymmdd();
    let config = match parse_config(config_delta_json) {
        Ok(c) => c,
        Err(e) => return to_js(&ValidateResult::Fatal(e)),
    };
    to_js(&run_full_pipeline(zip_bytes, &config, today))
}

/// K1–K5'i çalıştırır.
/// `on_stage(name, elapsed_ms)`: K1/K2/K3/K4/K5 her biri bittikten sonra çağrılır.
#[wasm_bindgen]
pub fn prepare(zip_bytes: &[u8], config_delta_json: &str, on_stage: &js_sys::Function) -> Result<CachedState, JsValue> {
    // Servis-günü normalizasyonu K2'de (run_k1_k5) yapıldığından config burada gerekir.
    // service_day_start_hour değişimi cache'lenmiş records'a yansımaz → dosya yeniden yüklenir.
    let config = parse_config(config_delta_json).map_err(|e| to_js(&ValidateResult::Fatal(e)))?;
    run_k1_k5(zip_bytes, &config, on_stage)
        .map_err(|fatal| to_js(&ValidateResult::Fatal(fatal)))
}

/// Önbellekten K6+K7'yi çalıştırır.
/// `on_stage(name, elapsed_ms)`: K6 ve K7 bittikten sonra çağrılır.
#[wasm_bindgen]
pub fn rerun_k6_k7(cache: &CachedState, config_delta_json: &str, on_stage: &js_sys::Function) -> JsValue {
    let today = today_yyyymmdd();
    let config = match parse_config(config_delta_json) {
        Ok(c) => c,
        Err(e) => return to_js(&ValidateResult::Fatal(e)),
    };

    let t = js_sys::Date::now();
    t_start!("K6-analytics");
    let k6 = analyze_k6(&cache.records, &cache.derived, &config, today);
    t_end!("K6-analytics");
    call_stage(on_stage, "K6", (js_sys::Date::now() - t) as u32);

    let mut all_notices = cache.k1_k5_notices.clone();
    all_notices.extend(k6.notices);
    web_sys::console::log_1(&format!("[mem] after-K6 (pre-cap): {} notices, {:.1} MB", all_notices.len(), mem_mb()).into());
    web_sys::console::log_1(&format!("[rules] after-K6 top: {}", top_rules_str(&all_notices)).into());

    let real_totals = cap_per_rule(&mut all_notices);
    all_notices.shrink_to_fit();
    log_mem("after-cap");
    let real_total: usize = real_totals.values().map(|&v| v as usize).sum();
    if real_total > NOTICE_LIMIT {
        web_sys::console::warn_1(&format!(
            "[uyarı] {real_total} notice üretildi (>{NOTICE_LIMIT}); gösterim kural başına cap'lendi, doğrulama durdurulmadı."
        ).into());
    }

    let t = js_sys::Date::now();
    let mut k7 = report_k7(all_notices, &cache.records, &cache.derived, cache.file_stats.clone(), true);
    call_stage(on_stage, "K7", (js_sys::Date::now() - t) as u32);

    scale_r9_deltas(&mut k7.reports, &real_totals);
    let capped_totals = build_capped_totals(&real_totals);
    let result = ValidateResult::Ok(ValidationResult {
        notices: k7.notices,
        reports: k7.reports,
        metrics: k7.metrics,
        name_index: build_name_index(&cache.records),
        capped_totals,
    });
    // #15: sonucu JS'e serialize etmek (to_js) büyük feed'de belleğin son sıçraması;
    // bu satır basılıp sonrası gelmiyorsa OOM serialize'da demektir.
    web_sys::console::log_1(&format!("[mem] before to_js (serialize): {:.1} MB", mem_mb()).into());
    let js = to_js(&result);
    web_sys::console::log_1(&format!("[mem] after to_js: {:.1} MB", mem_mb()).into());
    js
}

// ── İç pipeline yardımcıları ──────────────────────────────────────────────────

fn run_full_pipeline(zip_bytes: &[u8], config: &ValidatorConfig, today: u32) -> ValidateResult {
    t_start!("K1-parse");
    let k1 = match parse(zip_bytes) {
        Ok(r) => r,
        Err(e) => return ValidateResult::Fatal(e),
    };
    t_end!("K1-parse");
    let mut file_stats = collect_file_stats(&k1.files);

    t_start!("K2-validate");
    let mut k2 = validate_k2(k1.files, Some(zip_bytes)); // #15 W2 + #38: stop_times ZIP stream
    t_end!("K2-validate");
    // Gece yarısını aşan seferleri (00:xx) servis-günü notasyonuna (24:xx) normalize et
    // (K3–K6 öncesi). pipeline::validate_bytes ile aynı adım; WASM kendi orkestrasyonunu
    // kullandığı için burada da çağrılmalı.
    k2.records.stop_times_index.normalize_service_day(config.service_day_start_hour);
    // Stream edilen dosyalarda K1 rows boş kalır; K2 sayaçları varsa üzerine yaz.
    // Yeni bir dosya stream edildiğinde k2/mod.rs streaming_row_counts'a eklenmesi yeterli.
    for fi in file_stats.iter_mut() {
        if let Some(&count) = k2.records.streaming_row_counts.get(fi.name.as_str()) {
            fi.rows = count as u32;
        }
    }

    t_start!("K3-entity-map");
    let k3 = build_entity_map(&k2.records);
    t_end!("K3-entity-map");

    t_start!("K4-cross-ref");
    let k4 = check_cross_ref(&k2.records, &k3.entity_map, today);
    t_end!("K4-cross-ref");

    // #15: trip_stop_set (büyük feed'de ~226 MB) yalnızca K4'te kullanılır → serbest bırak.
    k2.records.stop_times_index.trip_stop_set = Default::default();

    t_start!("K5-derived");
    let k5 = build_derived(&k2.records, &k3.entity_map);
    t_end!("K5-derived");

    t_start!("K6-analytics");
    let k6 = analyze_k6(&k2.records, &k5.derived, config, today);
    t_end!("K6-analytics");

    let mut all_notices = Vec::new();
    all_notices.extend(k1.notices);
    all_notices.extend(k2.notices);
    all_notices.extend(k3.notices);
    all_notices.extend(k4.notices);
    all_notices.extend(k5.notices);
    all_notices.extend(k6.notices);

    // 1) Dedup sonrası gerçek totalleri say, ardından kural başına gösterim cap'ini uygula.
    // Native pipeline K7'de dedup yaptığı için karşılaştırılabilir "gerçek" sayı bu noktadadır.
    let real_totals = cap_per_rule(&mut all_notices);
    all_notices.shrink_to_fit();
    // 3) DURDURMA YOK — çok büyük feed'de yalnızca konsola uyarı (cap zaten sınırlıyor)
    let real_total: usize = real_totals.values().map(|&v| v as usize).sum();
    if real_total > NOTICE_LIMIT {
        web_sys::console::warn_1(&format!(
            "[uyarı] {real_total} notice üretildi (>{NOTICE_LIMIT}); gösterim kural başına cap'lendi, doğrulama durdurulmadı."
        ).into());
    }

    let mut k7 = report_k7(all_notices, &k2.records, &k5.derived, file_stats, true);
    // 4) Cap'e çarpan kurallarda score delta'yı gerçek toplam oranıyla ölçekle
    scale_r9_deltas(&mut k7.reports, &real_totals);
    let capped_totals = build_capped_totals(&real_totals);
    ValidateResult::Ok(ValidationResult {
        notices: k7.notices,
        reports: k7.reports,
        metrics: k7.metrics,
        name_index: build_name_index(&k2.records),
        capped_totals,
    })
}

fn run_k1_k5(zip_bytes: &[u8], config: &ValidatorConfig, on_stage: &js_sys::Function) -> Result<CachedState, FatalError> {
    let today = today_yyyymmdd();
    web_sys::console::log_1(&format!("[mem] K0-start (zip {:.1} MB): {:.1} MB", zip_bytes.len() as f64 / 1_048_576.0, mem_mb()).into());

    let mut t = js_sys::Date::now();
    t_start!("K1-parse");
    let k1 = parse(zip_bytes)?;
    t_end!("K1-parse");
    call_stage(on_stage, "K1", (js_sys::Date::now() - t) as u32);
    log_mem("after-K1-parse");
    let mut file_stats = collect_file_stats(&k1.files);

    t = js_sys::Date::now();
    t_start!("K2-validate");
    let mut k2 = validate_k2(k1.files, Some(zip_bytes)); // #15 W2 + #38: stop_times ZIP stream
    t_end!("K2-validate");
    // Gece yarısı (00:xx) → servis-günü (24:xx) normalizasyonu — K3–K6 öncesi (bkz. ilk yol).
    k2.records.stop_times_index.normalize_service_day(config.service_day_start_hour);
    call_stage(on_stage, "K2", (js_sys::Date::now() - t) as u32);
    log_mem("after-K2-validate");
    // Stream edilen dosyalarda K1 rows boş kalır; K2 sayaçları varsa üzerine yaz.
    // Yeni bir dosya stream edildiğinde k2/mod.rs streaming_row_counts'a eklenmesi yeterli.
    for fi in file_stats.iter_mut() {
        if let Some(&count) = k2.records.streaming_row_counts.get(fi.name.as_str()) {
            fi.rows = count as u32;
        }
    }

    t = js_sys::Date::now();
    t_start!("K3-entity-map");
    let k3 = build_entity_map(&k2.records);
    t_end!("K3-entity-map");
    call_stage(on_stage, "K3", (js_sys::Date::now() - t) as u32);
    log_mem("after-K3-entity-map");

    t = js_sys::Date::now();
    t_start!("K4-cross-ref");
    let k4 = check_cross_ref(&k2.records, &k3.entity_map, today);
    t_end!("K4-cross-ref");
    call_stage(on_stage, "K4", (js_sys::Date::now() - t) as u32);
    // #15: trip_stop_set (büyük feed'de ~226 MB) yalnızca K4'te kullanılır; cache'lenen records'tan
    // K6 continuation öncesi serbest bırak (K6 peak'ini ~226 MB düşürme potansiyeli).
    k2.records.stop_times_index.trip_stop_set = Default::default();
    log_mem("after-K4-cross-ref");

    t = js_sys::Date::now();
    t_start!("K5-derived");
    let k5 = build_derived(&k2.records, &k3.entity_map);
    t_end!("K5-derived");
    call_stage(on_stage, "K5", (js_sys::Date::now() - t) as u32);
    log_mem("after-K5-derived");

    let mut k1_k5_notices = Vec::new();
    k1_k5_notices.extend(k1.notices);
    k1_k5_notices.extend(k2.notices);
    k1_k5_notices.extend(k3.notices);
    k1_k5_notices.extend(k4.notices);
    k1_k5_notices.extend(k5.notices);
    // DURDURMA YOK: K1-K5 notice'ları cap'lenmeden taşınır; gösterim cap'i K6+K7 yolunda uygulanır.
    web_sys::console::log_1(&format!("[mem] K1-K5 done: {} notices, {:.1} MB", k1_k5_notices.len(), mem_mb()).into());
    web_sys::console::log_1(&format!("[rules] K1-K5 top: {}", top_rules_str(&k1_k5_notices)).into());

    Ok(CachedState { k1_k5_notices, records: k2.records, derived: k5.derived, file_stats })
}

fn parse_config(delta_json: &str) -> Result<ValidatorConfig, FatalError> {
    if delta_json.is_empty() || delta_json == "{}" {
        return Ok(ValidatorConfig::default());
    }
    merge_delta(&ValidatorConfig::default(), delta_json).map_err(|e| FatalError {
        code: FatalCode::InvalidInput,
        message: format!("Config parse hatası: {e}"),
    })
}

// Kural başına notice sınırı (cap):
// Aynı kural çok sayıda satırda tetiklenebilir (örn. her stop_time satırı için STM_017).
// Sınırsız bırakılırsa WASM bellek kullanımı patlar ve UI render süresi uzar.
// Varsayılan cap PER_RULE_CAP (500); gerçek feed'lerde doğal olarak yüksek çıkan
// kurallar HIGH_CAP_RULES listesinde 2000'e yükseltilmiştir.
// Dönüş değeri: cap'e çarpan kuralların {rule_id → gerçek_toplam} haritası.
fn cap_for_rule(rule_id: &str) -> usize {
    if HIGH_CAP_RULES.contains(&rule_id) { HIGH_CAP } else { PER_RULE_CAP }
}

fn count_totals(notices: &[gtfs_core::Notice]) -> std::collections::HashMap<String, u32> {
    let mut m = std::collections::HashMap::new();
    for n in notices { *m.entry(n.rule_id.clone()).or_insert(0u32) += 1; }
    m
}

// #15 Mode A teşhisi: en çok notice üreten ilk 10 kuralı "RULE=count" olarak döner.
fn top_rules_str(notices: &[gtfs_core::Notice]) -> String {
    let m = count_totals(notices);
    let mut v: Vec<(&String, &u32)> = m.iter().collect();
    v.sort_by(|a, b| b.1.cmp(a.1).then(a.0.cmp(b.0)));
    v.iter().take(10).map(|(r, c)| format!("{r}={c}")).collect::<Vec<_>>().join(" ")
}

fn build_capped_totals(real_totals: &std::collections::HashMap<String, u32>) -> std::collections::HashMap<String, u32> {
    real_totals.iter()
        .filter_map(|(rule_id, &total)| {
            if total > cap_for_rule(rule_id) as u32 { Some((rule_id.clone(), total)) } else { None }
        })
        .collect()
}

/// Cap'e çarpan kurallarda R9 score delta'larını gerçek notice sayısıyla orantılı ölçekler.
/// Büyük sayılarda hiperbolik model lineerleşir; ölçekleme iyi bir yaklaşım sağlar.
fn scale_r9_deltas(reports: &mut gtfs_core::ReportSet, real_totals: &std::collections::HashMap<String, u32>) {
    for item in &mut reports.r9.items {
        if let Some(&real) = real_totals.get(&item.rule_id) {
            let cap = cap_for_rule(&item.rule_id) as u32;
            let shown = real.min(cap);
            if shown > 0 && real > shown {
                let scale = real as f64 / shown as f64;
                item.score_delta *= scale;
                item.pub_score_delta *= scale;
            }
        }
    }
}

fn cap_per_rule(notices: &mut Vec<gtfs_core::Notice>) -> std::collections::HashMap<String, u32> {
    // Determinizm + tam temsil: önce dedup (kararlı sıralı, distinct entity), sonra cap.
    // Cap ham notice'lara değil distinct entity'lere uygulanır → gösterilen temsilciler
    // thread-bağımsız ve kümelenme olmaz (ör. OPR_005 cap'e çarpsa bile distinct route
    // sayısı korunur). dedup içinde notice_order_key ile kararlı sıralama yapılır.
    *notices = gtfs_pipeline::k7_reporting::dedup(std::mem::take(notices));
    // Golden sayımları ve skor ölçeklemesi ham emisyonu değil, native K7 ile aynı
    // dedup edilmiş gerçek notice sayısını taşımalı.
    let real_totals = count_totals(notices);
    let mut counts: std::collections::HashMap<String, usize> = std::collections::HashMap::new();
    notices.retain(|n| {
        let c = counts.entry(n.rule_id.clone()).or_insert(0);
        *c += 1;
        *c <= cap_for_rule(&n.rule_id)
    });
    real_totals
}

// #15 Adım-1 enstrümantasyon: WASM linear memory high-water (MB).
// WASM belleği yalnızca BÜYÜR (sayfa iade edilmez) → her aşamadan sonra okunan değer
// o ana dek erişilen tepe kullanımdır; aşamalar arası delta = o aşamanın eklediği kalıcı
// tahsisat. OOM'da feed taşıran aşamada ölür → son log hangi aşamaya kadar geldiğini verir.
fn mem_mb() -> f64 {
    use wasm_bindgen::JsCast;
    let buf = wasm_bindgen::memory()
        .unchecked_into::<js_sys::WebAssembly::Memory>()
        .buffer();
    // js_sys::ArrayBuffer::byte_length() dönüş tipi u32 olduğu için Memory64'te
    // 4 GiB üzerinde sarar (örn. 5313 MB → 1217 MB). WebIDL Number değerini
    // Reflect ile doğrudan f64 oku; web tarafındaki 16 GiB sınırı tam temsil edilir.
    let bytes = js_sys::Reflect::get(buf.as_ref(), &JsValue::from_str("byteLength"))
        .ok()
        .and_then(|v| v.as_f64())
        .unwrap_or(0.0);
    bytes / 1_048_576.0
}

fn log_mem(stage: &str) {
    web_sys::console::log_1(&format!("[mem] {stage}: {:.1} MB", mem_mb()).into());
}

fn today_yyyymmdd() -> u32 {
    let now = js_sys::Date::new_0();
    let y = now.get_full_year();
    let m = now.get_month() + 1;
    let d = now.get_date();
    y * 10000 + m * 100 + d
}

#[cfg(test)]
mod tests {
    use super::cap_per_rule;
    use gtfs_core::{EntityType, Notice, RuleClass, Severity};

    fn trip_notice(id: &str) -> Notice {
        Notice {
            id: id.to_string(), rule_id: "TRP_022".to_string(), severity: Severity::Yuksek,
            rule_class: RuleClass::Spec, entity_type: EntityType::Trip,
            entity_id: Some("trip-a".to_string()), scope_key: Some("trip-a".to_string()),
            file: Some("trips.txt".to_string()), line: None, field: Some("block_id".to_string()),
            observed_value: None, expected_value: None, details: None, title: "test".to_string(),
            message: "test".to_string(), remediation: "test".to_string(), blocks: Vec::new(),
            base_effort: 1, service_id: None,
        }
    }

    #[test]
    fn cap_totals_count_deduped_not_raw_emissions() {
        let mut notices = vec![trip_notice("a"), trip_notice("b")];
        let totals = cap_per_rule(&mut notices);
        assert_eq!(notices.len(), 1);
        assert_eq!(totals.get("TRP_022"), Some(&1));
    }
}

fn to_js<T: serde::Serialize>(value: &T) -> JsValue {
    match serde_json::to_string(value) {
        Ok(json) => JsValue::from_str(&json),
        Err(e) => JsValue::from_str(&format!(r#"{{"error":"serialize failed: {e}"}}"#)),
    }
}
