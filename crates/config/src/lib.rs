use serde::{Deserialize, Serialize};

// ── Takvim override kural modeli ──────────────────────────────────────────────

/// Bir hat için base/override servis ilişkisini tanımlar.
/// OPR_021/022/023 kuralları bu yapıya dayanır.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CalendarOverrideRule {
    pub route_id: String,
    pub base_service_ids: Vec<String>,
    pub override_service_ids: Vec<String>,
    /// Override penceresinin başlangıç tarihi (YYYYMMDD).
    pub start_date: u32,
    /// Override penceresinin bitiş tarihi (YYYYMMDD).
    pub end_date: u32,
}

// ── Compile-time default sabitleri ────────────────────────────────────────────

const DEF_MAX_SPEED_BUS_KMH:        f64 = 120.0;
const DEF_MAX_SPEED_TRAM_KMH:       f64 = 100.0;
const DEF_MAX_SPEED_METRO_KMH:      f64 = 150.0;
const DEF_MAX_SPEED_RAIL_KMH:       f64 = 300.0;
const DEF_MAX_SPEED_FERRY_KMH:      f64 =  80.0;
const DEF_MAX_SPEED_CABLECAR_KMH:   f64 =  30.0;
const DEF_MIN_TRANSFER_TIME_SEC:    u32 = 180;
const DEF_MAX_TRANSFER_DISTANCE_M:  f64 = 500.0;
const DEF_MAX_SHAPE_JUMP_KM:        f64 =  10.0;
const DEF_STOP_TOO_CLOSE_M:         f64 =   5.0;
const DEF_STOP_FAR_FROM_SHAPE_M:    f64 = 100.0;
const DEF_STOP_FAR_FROM_PARENT_M:   f64 = 100.0;
const DEF_FEED_EXPIRY_WARNING_DAYS: u32 =  30;
const DEF_SERVICE_GAP_DAYS:         u32 =   7;
const DEF_MAX_TRIP_DURATION_HOURS:  f64 =  24.0;
const DEF_MIN_TRIP_DURATION_SEC:    u32 =  60;
const DEF_MAX_HEADWAY_WARNING_MIN:  u32 = 240;
const DEF_BUNCHING_THRESHOLD_MIN:   u32 =   2;
const DEF_RAIL_STOP_DISTANCE_KM:    f64 = 500.0;

/// Validator parametreleri. Tüm eşikler architecture v0.8 Bölüm 6'dan.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ValidatorConfig {
    pub max_speed_bus_kmh:        f64,
    pub max_speed_tram_kmh:       f64,
    pub max_speed_metro_kmh:      f64,
    pub max_speed_rail_kmh:       f64,
    pub max_speed_ferry_kmh:      f64,
    pub max_speed_cablecar_kmh:   f64,
    pub min_transfer_time_sec:    u32,
    pub max_transfer_distance_m:  f64,
    pub max_shape_jump_km:        f64,
    pub stop_too_close_m:         f64,
    pub stop_far_from_shape_m:    f64,
    pub stop_far_from_parent_m:   f64,
    pub feed_expiry_warning_days: u32,
    pub service_gap_days:         u32,
    pub max_trip_duration_hours:  f64,
    pub min_trip_duration_sec:    u32,
    pub max_headway_warning_min:  u32,
    pub bunching_threshold_min:   u32,
    /// Demiryolu (route_type 2/12/100-117) seferleri için STM_026 durak arası mesafe eşiği (km).
    pub rail_stop_distance_km:    f64,
    /// Takvim override kuralları — OPR_021/022/023 için.
    /// Boş liste varsayılan; override analizi yapılmaz.
    pub calendar_override_rules: Vec<CalendarOverrideRule>,
}

impl Default for ValidatorConfig {
    fn default() -> Self {
        Self {
            max_speed_bus_kmh:        DEF_MAX_SPEED_BUS_KMH,
            max_speed_tram_kmh:       DEF_MAX_SPEED_TRAM_KMH,
            max_speed_metro_kmh:      DEF_MAX_SPEED_METRO_KMH,
            max_speed_rail_kmh:       DEF_MAX_SPEED_RAIL_KMH,
            max_speed_ferry_kmh:      DEF_MAX_SPEED_FERRY_KMH,
            max_speed_cablecar_kmh:   DEF_MAX_SPEED_CABLECAR_KMH,
            min_transfer_time_sec:    DEF_MIN_TRANSFER_TIME_SEC,
            max_transfer_distance_m:  DEF_MAX_TRANSFER_DISTANCE_M,
            max_shape_jump_km:        DEF_MAX_SHAPE_JUMP_KM,
            stop_too_close_m:         DEF_STOP_TOO_CLOSE_M,
            stop_far_from_shape_m:    DEF_STOP_FAR_FROM_SHAPE_M,
            stop_far_from_parent_m:   DEF_STOP_FAR_FROM_PARENT_M,
            feed_expiry_warning_days: DEF_FEED_EXPIRY_WARNING_DAYS,
            service_gap_days:         DEF_SERVICE_GAP_DAYS,
            max_trip_duration_hours:  DEF_MAX_TRIP_DURATION_HOURS,
            min_trip_duration_sec:    DEF_MIN_TRIP_DURATION_SEC,
            max_headway_warning_min:  DEF_MAX_HEADWAY_WARNING_MIN,
            bunching_threshold_min:   DEF_BUNCHING_THRESHOLD_MIN,
            rail_stop_distance_km:    DEF_RAIL_STOP_DISTANCE_KM,
            calendar_override_rules:  Vec::new(),
        }
    }
}

fn is_leap_year(y: u32) -> bool {
    (y % 4 == 0 && y % 100 != 0) || (y % 400 == 0)
}

fn days_in_month(y: u32, m: u32) -> u32 {
    match m {
        1 | 3 | 5 | 7 | 8 | 10 | 12 => 31,
        4 | 6 | 9 | 11 => 30,
        2 => if is_leap_year(y) { 29 } else { 28 },
        _ => 0,
    }
}

fn valid_yyyymmdd(d: u32) -> bool {
    let y = d / 10_000;
    let m = (d / 100) % 100;
    let day = d % 100;
    (2000..=2099).contains(&y)
        && (1..=12).contains(&m)
        && day >= 1
        && day <= days_in_month(y, m)
}

/// Taxonomy Bölüm 6'da tanımlı parametre aralıklarını kontrol eder.
/// `merge_delta` başarıyla uyguladıktan sonra çağrılır.
fn validate_ranges(cfg: &ValidatorConfig) -> Result<(), String> {
    macro_rules! chk_f64 {
        ($field:expr, $name:literal, $lo:expr, $hi:expr) => {
            if $field < $lo || $field > $hi {
                return Err(format!(
                    "'{}' aralık dışı: {} (beklenen {}-{})",
                    $name, $field, $lo, $hi
                ));
            }
        };
    }
    macro_rules! chk_u32 {
        ($field:expr, $name:literal, $lo:expr, $hi:expr) => {
            if $field < $lo || $field > $hi {
                return Err(format!(
                    "'{}' aralık dışı: {} (beklenen {}-{})",
                    $name, $field, $lo, $hi
                ));
            }
        };
    }
    chk_f64!(cfg.max_speed_bus_kmh,        "max_speed_bus_kmh",         60.0,   200.0);
    chk_f64!(cfg.max_speed_tram_kmh,       "max_speed_tram_kmh",        40.0,   160.0);
    chk_f64!(cfg.max_speed_metro_kmh,      "max_speed_metro_kmh",       80.0,   250.0);
    chk_f64!(cfg.max_speed_rail_kmh,       "max_speed_rail_kmh",       100.0,   400.0);
    chk_f64!(cfg.max_speed_ferry_kmh,      "max_speed_ferry_kmh",       20.0,   150.0);
    chk_f64!(cfg.max_speed_cablecar_kmh,   "max_speed_cablecar_kmh",    10.0,    60.0);
    chk_u32!(cfg.min_transfer_time_sec,    "min_transfer_time_sec",       30,   1800);
    chk_f64!(cfg.max_transfer_distance_m,  "max_transfer_distance_m",   50.0,  2000.0);
    chk_f64!(cfg.max_shape_jump_km,        "max_shape_jump_km",          1.0,    50.0);
    chk_f64!(cfg.stop_too_close_m,         "stop_too_close_m",           1.0,    20.0);
    chk_f64!(cfg.stop_far_from_shape_m,    "stop_far_from_shape_m",     20.0,   500.0);
    chk_f64!(cfg.stop_far_from_parent_m,  "stop_far_from_parent_m",    10.0,  1000.0);
    chk_u32!(cfg.feed_expiry_warning_days, "feed_expiry_warning_days",     1,    60);
    chk_u32!(cfg.service_gap_days,         "service_gap_days",             3,    30);
    chk_f64!(cfg.max_trip_duration_hours,  "max_trip_duration_hours",    8.0,    72.0);
    chk_u32!(cfg.min_trip_duration_sec,    "min_trip_duration_sec",       10,   300);
    chk_u32!(cfg.max_headway_warning_min,  "max_headway_warning_min",     60,   720);
    chk_u32!(cfg.bunching_threshold_min,   "bunching_threshold_min",       1,    10);
    chk_f64!(cfg.rail_stop_distance_km,    "rail_stop_distance_km",       50.0, 2000.0);
    Ok(())
}

/// JSON delta'yı base config üzerine uygular.
///
/// Kontrat:
/// - Bilinmeyen anahtar → `Err` döner, base değişmez.
/// - Bilinen tüm anahtarlar uygulandıktan sonra `Ok(yeni_config)` döner.
/// - Sayısal tip uyuşmazlığı (f64 gereken yerde string vb.) → `Err`.
pub fn merge_delta(base: &ValidatorConfig, delta_json: &str) -> Result<ValidatorConfig, String> {
    let map: serde_json::Map<String, serde_json::Value> =
        serde_json::from_str(delta_json).map_err(|e| format!("JSON parse hatası: {e}"))?;

    // Bilinmeyen anahtarları önce topla — Err durumunda hiçbir şey uygulanmaz.
    let known: &[&str] = &[
        "max_speed_bus_kmh", "max_speed_tram_kmh", "max_speed_metro_kmh",
        "max_speed_rail_kmh", "max_speed_ferry_kmh", "max_speed_cablecar_kmh",
        "min_transfer_time_sec", "max_transfer_distance_m", "max_shape_jump_km",
        "stop_too_close_m", "stop_far_from_shape_m", "stop_far_from_parent_m", "feed_expiry_warning_days",
        "service_gap_days", "max_trip_duration_hours", "min_trip_duration_sec",
        "max_headway_warning_min", "bunching_threshold_min", "rail_stop_distance_km",
        "calendar_override_rules",
    ];
    let unknown: Vec<&str> = map.keys()
        .filter(|k| !known.contains(&k.as_str()))
        .map(String::as_str)
        .collect();
    if !unknown.is_empty() {
        return Err(format!("Bilinmeyen config anahtarları: {}", unknown.join(", ")));
    }

    let mut cfg = base.clone();

    macro_rules! apply_f64 {
        ($key:literal, $field:expr) => {
            if let Some(v) = map.get($key) {
                $field = v.as_f64().ok_or_else(|| {
                    format!("'{}' için f64 bekleniyor, alınan: {v}", $key)
                })?;
            }
        };
    }
    macro_rules! apply_u32 {
        ($key:literal, $field:expr) => {
            if let Some(v) = map.get($key) {
                let n = v.as_u64().ok_or_else(|| {
                    format!("'{}' için u32 bekleniyor, alınan: {v}", $key)
                })?;
                $field = u32::try_from(n)
                    .map_err(|_| format!("'{}' değeri u32 sınırını aşıyor: {n}", $key))?;
            }
        };
    }

    apply_f64!("max_speed_bus_kmh",        cfg.max_speed_bus_kmh);
    apply_f64!("max_speed_tram_kmh",       cfg.max_speed_tram_kmh);
    apply_f64!("max_speed_metro_kmh",      cfg.max_speed_metro_kmh);
    apply_f64!("max_speed_rail_kmh",       cfg.max_speed_rail_kmh);
    apply_f64!("max_speed_ferry_kmh",      cfg.max_speed_ferry_kmh);
    apply_f64!("max_speed_cablecar_kmh",   cfg.max_speed_cablecar_kmh);
    apply_u32!("min_transfer_time_sec",    cfg.min_transfer_time_sec);
    apply_f64!("max_transfer_distance_m",  cfg.max_transfer_distance_m);
    apply_f64!("max_shape_jump_km",        cfg.max_shape_jump_km);
    apply_f64!("stop_too_close_m",         cfg.stop_too_close_m);
    apply_f64!("stop_far_from_shape_m",    cfg.stop_far_from_shape_m);
    apply_f64!("stop_far_from_parent_m",   cfg.stop_far_from_parent_m);
    apply_u32!("feed_expiry_warning_days", cfg.feed_expiry_warning_days);
    apply_u32!("service_gap_days",         cfg.service_gap_days);
    apply_f64!("max_trip_duration_hours",  cfg.max_trip_duration_hours);
    apply_u32!("min_trip_duration_sec",    cfg.min_trip_duration_sec);
    apply_u32!("max_headway_warning_min",  cfg.max_headway_warning_min);
    apply_u32!("bunching_threshold_min",   cfg.bunching_threshold_min);
    apply_f64!("rail_stop_distance_km",    cfg.rail_stop_distance_km);

    if let Some(v) = map.get("calendar_override_rules") {
        cfg.calendar_override_rules = serde_json::from_value(v.clone())
            .map_err(|e| format!("'calendar_override_rules' parse hatası: {e}"))?;
        for (i, rule) in cfg.calendar_override_rules.iter().enumerate() {
            if rule.route_id.is_empty() {
                return Err(format!("calendar_override_rules[{i}]: route_id boş olamaz"));
            }
            if rule.base_service_ids.is_empty() {
                return Err(format!("calendar_override_rules[{i}]: base_service_ids boş olamaz"));
            }
            if rule.override_service_ids.is_empty() {
                return Err(format!("calendar_override_rules[{i}]: override_service_ids boş olamaz"));
            }
            if !valid_yyyymmdd(rule.start_date) {
                return Err(format!(
                    "calendar_override_rules[{i}]: start_date '{}' geçerli takvim tarihi değil (YYYYMMDD, 2000-2099)",
                    rule.start_date
                ));
            }
            if !valid_yyyymmdd(rule.end_date) {
                return Err(format!(
                    "calendar_override_rules[{i}]: end_date '{}' geçerli takvim tarihi değil (YYYYMMDD, 2000-2099)",
                    rule.end_date
                ));
            }
            if rule.start_date > rule.end_date {
                return Err(format!(
                    "calendar_override_rules[{i}]: start_date ({}) > end_date ({})",
                    rule.start_date, rule.end_date
                ));
            }
        }
    }

    validate_ranges(&cfg)?;
    Ok(cfg)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_values() {
        let cfg = ValidatorConfig::default();
        assert_eq!(cfg.max_speed_bus_kmh,        120.0);
        assert_eq!(cfg.max_speed_tram_kmh,       100.0);
        assert_eq!(cfg.max_speed_metro_kmh,      150.0);
        assert_eq!(cfg.max_speed_rail_kmh,       300.0);
        assert_eq!(cfg.max_speed_ferry_kmh,       80.0);
        assert_eq!(cfg.max_speed_cablecar_kmh,    30.0);
        assert_eq!(cfg.min_transfer_time_sec,     180);
        assert_eq!(cfg.max_transfer_distance_m,  500.0);
        assert_eq!(cfg.max_shape_jump_km,         10.0);
        assert_eq!(cfg.stop_too_close_m,           5.0);
        assert_eq!(cfg.stop_far_from_shape_m,    100.0);
        assert_eq!(cfg.feed_expiry_warning_days,   30);
        assert_eq!(cfg.service_gap_days,            7);
        assert_eq!(cfg.max_trip_duration_hours,   24.0);
        assert_eq!(cfg.min_trip_duration_sec,      60);
        assert_eq!(cfg.max_headway_warning_min,   240);
        assert_eq!(cfg.bunching_threshold_min,      2);
    }

    #[test]
    fn merge_delta_applies_known_keys() {
        let base = ValidatorConfig::default();
        let result = merge_delta(&base, r#"{"max_speed_bus_kmh": 90.0, "service_gap_days": 14}"#);
        let cfg = result.expect("merge başarılı olmalı");
        assert_eq!(cfg.max_speed_bus_kmh, 90.0);
        assert_eq!(cfg.service_gap_days, 14);
        // Dokunulmayan alanlar değişmemeli
        assert_eq!(cfg.max_speed_rail_kmh, 300.0);
        assert_eq!(cfg.min_transfer_time_sec, 180);
    }

    #[test]
    fn merge_delta_rejects_unknown_key() {
        let base = ValidatorConfig::default();
        let err = merge_delta(&base, r#"{"unknown_param": 42}"#)
            .expect_err("bilinmeyen anahtar Err döndürmeli");
        assert!(err.contains("unknown_param"), "hata mesajı anahtar adını içermeli: {err}");
    }

    #[test]
    fn merge_delta_rejects_partial_on_unknown() {
        // Bir bilinen + bir bilinmeyen anahtar → tüm override iptal
        let base = ValidatorConfig::default();
        let err = merge_delta(
            &base,
            r#"{"max_speed_bus_kmh": 50.0, "nonexistent": true}"#,
        )
        .expect_err("bilinmeyen anahtar olunca Err bekleniyor");
        assert!(err.contains("nonexistent"));
    }

    #[test]
    fn merge_delta_rejects_wrong_type() {
        let base = ValidatorConfig::default();
        let err = merge_delta(&base, r#"{"max_speed_bus_kmh": "hız"}"#)
            .expect_err("tip uyuşmazlığı Err döndürmeli");
        assert!(err.contains("max_speed_bus_kmh"));
    }

    #[test]
    fn merge_delta_empty_delta_is_identity() {
        let base = ValidatorConfig::default();
        let cfg = merge_delta(&base, "{}").expect("boş delta geçerli");
        assert_eq!(cfg, base);
    }

    #[test]
    fn merge_delta_invalid_json() {
        let base = ValidatorConfig::default();
        assert!(merge_delta(&base, "not json").is_err());
    }

    // ── Aralık testleri ───────────────────────────────────────────────────────

    #[test]
    fn range_below_min_rejected() {
        let base = ValidatorConfig::default();
        // max_speed_bus_kmh alt sınır = 60
        let err = merge_delta(&base, r#"{"max_speed_bus_kmh": 59.9}"#)
            .expect_err("alt sınır altı Err olmalı");
        assert!(err.contains("max_speed_bus_kmh"), "{err}");

        // bunching_threshold_min alt sınır = 1
        let err = merge_delta(&base, r#"{"bunching_threshold_min": 0}"#)
            .expect_err("alt sınır altı Err olmalı");
        assert!(err.contains("bunching_threshold_min"), "{err}");

        // stop_far_from_shape_m alt sınır = 20
        let err = merge_delta(&base, r#"{"stop_far_from_shape_m": 19.9}"#)
            .expect_err("alt sınır altı Err olmalı");
        assert!(err.contains("stop_far_from_shape_m"), "{err}");
    }

    #[test]
    fn range_above_max_rejected() {
        let base = ValidatorConfig::default();
        // max_speed_rail_kmh üst sınır = 400
        let err = merge_delta(&base, r#"{"max_speed_rail_kmh": 400.1}"#)
            .expect_err("üst sınır üstü Err olmalı");
        assert!(err.contains("max_speed_rail_kmh"), "{err}");

        // service_gap_days üst sınır = 30
        let err = merge_delta(&base, r#"{"service_gap_days": 31}"#)
            .expect_err("üst sınır üstü Err olmalı");
        assert!(err.contains("service_gap_days"), "{err}");

        // max_headway_warning_min üst sınır = 720
        let err = merge_delta(&base, r#"{"max_headway_warning_min": 721}"#)
            .expect_err("üst sınır üstü Err olmalı");
        assert!(err.contains("max_headway_warning_min"), "{err}");
    }

    #[test]
    fn range_boundary_values_accepted() {
        let base = ValidatorConfig::default();
        // Alt sınır: max_speed_bus_kmh = 60
        let cfg = merge_delta(&base, r#"{"max_speed_bus_kmh": 60.0}"#)
            .expect("alt sınır değeri geçerli");
        assert_eq!(cfg.max_speed_bus_kmh, 60.0);

        // Üst sınır: max_speed_rail_kmh = 400
        let cfg = merge_delta(&base, r#"{"max_speed_rail_kmh": 400.0}"#)
            .expect("üst sınır değeri geçerli");
        assert_eq!(cfg.max_speed_rail_kmh, 400.0);

        // u32 sınır: bunching_threshold_min = 1 (alt) ve 10 (üst)
        let cfg = merge_delta(&base, r#"{"bunching_threshold_min": 1}"#)
            .expect("alt sınır geçerli");
        assert_eq!(cfg.bunching_threshold_min, 1);

        let cfg = merge_delta(&base, r#"{"bunching_threshold_min": 10}"#)
            .expect("üst sınır geçerli");
        assert_eq!(cfg.bunching_threshold_min, 10);
    }

    #[test]
    fn range_all_fields_default_in_range() {
        // Default değerlerin tümü taxonomy aralığı içinde olmalı
        let base = ValidatorConfig::default();
        merge_delta(&base, "{}").expect("default config tüm aralıklara uymalı");
    }

    // ── calendar_override_rules testleri ──────────────────────────────────────

    #[test]
    fn calendar_override_rules_default_empty() {
        let cfg = ValidatorConfig::default();
        assert!(cfg.calendar_override_rules.is_empty());
    }

    #[test]
    fn calendar_override_rules_parsed_from_delta() {
        let base = ValidatorConfig::default();
        let delta = r#"{
            "calendar_override_rules": [
                {
                    "route_id": "200",
                    "base_service_ids": ["WKD_200"],
                    "override_service_ids": ["HOL_200"],
                    "start_date": 20260515,
                    "end_date": 20260715
                }
            ]
        }"#;
        let cfg = merge_delta(&base, delta).expect("override rules parse edilmeli");
        assert_eq!(cfg.calendar_override_rules.len(), 1);
        let rule = &cfg.calendar_override_rules[0];
        assert_eq!(rule.route_id, "200");
        assert_eq!(rule.base_service_ids, vec!["WKD_200"]);
        assert_eq!(rule.override_service_ids, vec!["HOL_200"]);
        assert_eq!(rule.start_date, 20260515);
        assert_eq!(rule.end_date, 20260715);
    }

    #[test]
    fn calendar_override_rules_multiple_rules() {
        let base = ValidatorConfig::default();
        let delta = r#"{
            "calendar_override_rules": [
                {
                    "route_id": "R1",
                    "base_service_ids": ["WKD"],
                    "override_service_ids": ["HOL"],
                    "start_date": 20260101,
                    "end_date": 20260103
                },
                {
                    "route_id": "R2",
                    "base_service_ids": ["WKD", "SAT"],
                    "override_service_ids": ["HOL"],
                    "start_date": 20260601,
                    "end_date": 20260601
                }
            ]
        }"#;
        let cfg = merge_delta(&base, delta).expect("çoklu override rules parse edilmeli");
        assert_eq!(cfg.calendar_override_rules.len(), 2);
        assert_eq!(cfg.calendar_override_rules[1].base_service_ids, vec!["WKD", "SAT"]);
    }

    #[test]
    fn calendar_override_rules_invalid_type_rejected() {
        let base = ValidatorConfig::default();
        let err = merge_delta(&base, r#"{"calendar_override_rules": "not_an_array"}"#)
            .expect_err("geçersiz tip Err döndürmeli");
        assert!(err.contains("calendar_override_rules"), "{err}");
    }

    #[test]
    fn calendar_override_rules_empty_array_accepted() {
        let base = ValidatorConfig::default();
        let cfg = merge_delta(&base, r#"{"calendar_override_rules": []}"#)
            .expect("boş dizi geçerli");
        assert!(cfg.calendar_override_rules.is_empty());
    }

    #[test]
    fn calendar_override_rules_empty_route_id_rejected() {
        let base = ValidatorConfig::default();
        let err = merge_delta(&base, r#"{"calendar_override_rules": [
            {"route_id": "", "base_service_ids": ["WKD"], "override_service_ids": ["HOL"],
             "start_date": 20260101, "end_date": 20260101}]}"#)
            .expect_err("boş route_id Err döndürmeli");
        assert!(err.contains("route_id"), "{err}");
    }

    #[test]
    fn calendar_override_rules_empty_base_rejected() {
        let base = ValidatorConfig::default();
        let err = merge_delta(&base, r#"{"calendar_override_rules": [
            {"route_id": "R1", "base_service_ids": [], "override_service_ids": ["HOL"],
             "start_date": 20260101, "end_date": 20260101}]}"#)
            .expect_err("boş base_service_ids Err döndürmeli");
        assert!(err.contains("base_service_ids"), "{err}");
    }

    #[test]
    fn calendar_override_rules_empty_override_rejected() {
        let base = ValidatorConfig::default();
        let err = merge_delta(&base, r#"{"calendar_override_rules": [
            {"route_id": "R1", "base_service_ids": ["WKD"], "override_service_ids": [],
             "start_date": 20260101, "end_date": 20260101}]}"#)
            .expect_err("boş override_service_ids Err döndürmeli");
        assert!(err.contains("override_service_ids"), "{err}");
    }

    #[test]
    fn calendar_override_rules_invalid_start_date_rejected() {
        let base = ValidatorConfig::default();
        // start_date=0 → geçersiz
        let err = merge_delta(&base, r#"{"calendar_override_rules": [
            {"route_id": "R1", "base_service_ids": ["WKD"], "override_service_ids": ["HOL"],
             "start_date": 0, "end_date": 20260101}]}"#)
            .expect_err("start_date=0 Err döndürmeli");
        assert!(err.contains("start_date"), "{err}");
        // Ay=13 → geçersiz
        let err = merge_delta(&base, r#"{"calendar_override_rules": [
            {"route_id": "R1", "base_service_ids": ["WKD"], "override_service_ids": ["HOL"],
             "start_date": 20261301, "end_date": 20261301}]}"#)
            .expect_err("ay=13 Err döndürmeli");
        assert!(err.contains("start_date"), "{err}");
    }

    #[test]
    fn calendar_override_rules_start_after_end_rejected() {
        let base = ValidatorConfig::default();
        let err = merge_delta(&base, r#"{"calendar_override_rules": [
            {"route_id": "R1", "base_service_ids": ["WKD"], "override_service_ids": ["HOL"],
             "start_date": 20260201, "end_date": 20260101}]}"#)
            .expect_err("start_date > end_date Err döndürmeli");
        assert!(err.contains("start_date"), "{err}");
        assert!(err.contains("end_date"), "{err}");
    }

    // ── Gerçek takvim tarihi doğrulama testleri ───────────────────────────────

    #[test]
    fn valid_yyyymmdd_rejects_impossible_days() {
        // Şubat 31 → hiçbir yılda yok
        assert!(!valid_yyyymmdd(20260231));
        // Nisan 31 → yok (Nisan 30 gün)
        assert!(!valid_yyyymmdd(20260431));
        // Haziran 31 → yok
        assert!(!valid_yyyymmdd(20260631));
        // Eylül 31 → yok
        assert!(!valid_yyyymmdd(20260931));
        // Kasım 31 → yok
        assert!(!valid_yyyymmdd(20261131));
        // Gün=0 → geçersiz
        assert!(!valid_yyyymmdd(20260100));
    }

    #[test]
    fn valid_yyyymmdd_leap_year_feb29() {
        // 2024 artık yıl → 29 Şubat geçerli
        assert!(valid_yyyymmdd(20240229));
        // 2000 artık yıl (400'e bölünür) → geçerli
        assert!(valid_yyyymmdd(20000229));
        // 2026 artık yıl değil → 29 Şubat geçersiz
        assert!(!valid_yyyymmdd(20260229));
        // 2100 kapsam dışı (>2099) → zaten geçersiz
        assert!(!valid_yyyymmdd(21000229));
    }

    #[test]
    fn valid_yyyymmdd_non_leap_year_feb28_ok() {
        // Artık yıl olmayan yıllarda 28 Şubat geçerli
        assert!(valid_yyyymmdd(20260228));
        assert!(valid_yyyymmdd(20250228));
    }

    #[test]
    fn valid_yyyymmdd_boundary_months() {
        // 31 günlük ayların son günü geçerli
        assert!(valid_yyyymmdd(20260131)); // Ocak
        assert!(valid_yyyymmdd(20260331)); // Mart
        assert!(valid_yyyymmdd(20260531)); // Mayıs
        assert!(valid_yyyymmdd(20260731)); // Temmuz
        assert!(valid_yyyymmdd(20260831)); // Ağustos
        assert!(valid_yyyymmdd(20261031)); // Ekim
        assert!(valid_yyyymmdd(20261231)); // Aralık
        // 30 günlük ayların son günü geçerli
        assert!(valid_yyyymmdd(20260430)); // Nisan
        assert!(valid_yyyymmdd(20260630)); // Haziran
        assert!(valid_yyyymmdd(20260930)); // Eylül
        assert!(valid_yyyymmdd(20261130)); // Kasım
    }

    #[test]
    fn calendar_override_rules_feb31_rejected() {
        let base = ValidatorConfig::default();
        // 20260231 takvimsel olarak geçersiz
        let err = merge_delta(&base, r#"{"calendar_override_rules": [
            {"route_id": "R1", "base_service_ids": ["WKD"], "override_service_ids": ["HOL"],
             "start_date": 20260231, "end_date": 20260231}]}"#)
            .expect_err("20260231 Err döndürmeli");
        assert!(err.contains("start_date"), "{err}");
    }

    #[test]
    fn calendar_override_rules_apr31_rejected() {
        let base = ValidatorConfig::default();
        let err = merge_delta(&base, r#"{"calendar_override_rules": [
            {"route_id": "R1", "base_service_ids": ["WKD"], "override_service_ids": ["HOL"],
             "start_date": 20260101, "end_date": 20260431}]}"#)
            .expect_err("20260431 end_date Err döndürmeli");
        assert!(err.contains("end_date"), "{err}");
    }

    #[test]
    fn calendar_override_rules_non_leap_feb29_rejected() {
        let base = ValidatorConfig::default();
        // 2026 artık yıl değil → 29 Şubat geçersiz
        let err = merge_delta(&base, r#"{"calendar_override_rules": [
            {"route_id": "R1", "base_service_ids": ["WKD"], "override_service_ids": ["HOL"],
             "start_date": 20260229, "end_date": 20260229}]}"#)
            .expect_err("artık yıl olmayan yılda 29 Şubat Err döndürmeli");
        assert!(err.contains("start_date"), "{err}");
    }

    #[test]
    fn calendar_override_rules_leap_feb29_accepted() {
        let base = ValidatorConfig::default();
        // 2024 artık yıl → 29 Şubat geçerli
        let cfg = merge_delta(&base, r#"{"calendar_override_rules": [
            {"route_id": "R1", "base_service_ids": ["WKD"], "override_service_ids": ["HOL"],
             "start_date": 20240229, "end_date": 20240229}]}"#)
            .expect("artık yılda 29 Şubat kabul edilmeli");
        assert_eq!(cfg.calendar_override_rules[0].start_date, 20240229);
    }
}
