use serde::{Deserialize, Serialize};

// ── Calendar override rule model ─────────────────────────────────────────────

/// Defines the base/override service relationship for a route.
/// Rules OPR_021/022/023 rely on this structure.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CalendarOverrideRule {
    pub route_id: String,
    pub base_service_ids: Vec<String>,
    pub override_service_ids: Vec<String>,
    /// Start date of the override window (YYYYMMDD).
    pub start_date: u32,
    /// End date of the override window (YYYYMMDD).
    pub end_date: u32,
}

// ── Compile-time default constants ────────────────────────────────────────────

const DEF_MAX_SPEED_BUS_KMH: f64 = 120.0;
const DEF_MAX_SPEED_TRAM_KMH: f64 = 100.0;
const DEF_MAX_SPEED_METRO_KMH: f64 = 150.0;
const DEF_MAX_SPEED_RAIL_KMH: f64 = 300.0;
const DEF_MAX_SPEED_FERRY_KMH: f64 = 80.0;
const DEF_MAX_SPEED_CABLECAR_KMH: f64 = 30.0;
const DEF_MIN_TRANSFER_TIME_SEC: u32 = 180;
const DEF_MAX_TRANSFER_DISTANCE_M: f64 = 500.0;
const DEF_MAX_SHAPE_JUMP_KM: f64 = 10.0;
const DEF_MAX_SHAPE_JUMP_KM_RAIL: f64 = 30.0;
const DEF_STOP_TOO_CLOSE_M: f64 = 5.0;
const DEF_STOP_FAR_FROM_SHAPE_M: f64 = 100.0;
const DEF_STOP_FAR_FROM_SHAPE_M_RAIL: f64 = 200.0;
const DEF_STOP_FAR_FROM_PARENT_M: f64 = 150.0;
const DEF_FEED_EXPIRY_WARNING_DAYS: u32 = 30;
const DEF_FEED_INFO_EXPIRY_WARNING_DAYS: u32 = 7;
const DEF_SERVICE_GAP_DAYS: u32 = 7;
const DEF_UPCOMING_SERVICE_DAYS: u32 = 7;
const DEF_MAX_TRIP_DURATION_HOURS: f64 = 24.0;
const DEF_MAX_TRIP_DURATION_HOURS_RAIL: f64 = 48.0;
const DEF_MIN_TRIP_DURATION_SEC: u32 = 60;
const DEF_MAX_HEADWAY_WARNING_MIN: u32 = 240;
const DEF_MAX_HEADWAY_WARNING_MIN_RAIL: u32 = 720;
const DEF_SERVICE_DAY_WINDOW_HOURS_RAIL: u32 = 48;
const DEF_BUNCHING_THRESHOLD_MIN: u32 = 2;
const DEF_RAIL_STOP_DISTANCE_KM: f64 = 500.0;
const DEF_MAX_TRIPS_PER_ROUTE: u32 = 500;
const DEF_DURATION_OUTLIER_SIGMA: f64 = 3.5;
const DEF_HEADWAY_OUTLIER_SIGMA: f64 = 2.5;
const DEF_SERVICE_DAY_START_HOUR: u32 = 3;
const DEF_MAX_CALENDAR_FUTURE_YEARS: u32 = 3;
const DEF_BIG_GAP_DAYS: u32 = 14;

// The JSON config delta is user-controlled; these collections cannot grow without
// limit during analysis. The bounds are far above normal use and only prevent the
// WASM/native parser from allocating enormous maps/Vecs.
const MAX_RURAL_ROUTE_IDS: usize = 100_000;
const MAX_DISABLED_RULE_IDS: usize = 100_000;
const MAX_CALENDAR_OVERRIDE_RULES: usize = 100_000;
const MAX_OVERRIDE_SERVICE_IDS_PER_RULE: usize = 100_000;
const MAX_CONFIG_STRING_BYTES: usize = 4 * 1024 * 1024;

/// Canonical list of keys accepted in the JSON config delta.
/// The SDK's typed `ValidatorConfigDelta` surface stays in parity with this list.
pub const KNOWN_CONFIG_KEYS: &[&str] = &[
    "max_speed_bus_kmh",
    "max_speed_tram_kmh",
    "max_speed_metro_kmh",
    "max_speed_rail_kmh",
    "max_speed_ferry_kmh",
    "max_speed_cablecar_kmh",
    "min_transfer_time_sec",
    "max_transfer_distance_m",
    "max_shape_jump_km",
    "max_shape_jump_km_rail",
    "stop_too_close_m",
    "stop_far_from_shape_m",
    "stop_far_from_shape_m_rail",
    "stop_far_from_parent_m",
    "feed_expiry_warning_days",
    "feed_info_expiry_warning_days",
    "service_gap_days",
    "big_gap_days",
    "upcoming_service_days",
    "max_trip_duration_hours",
    "max_trip_duration_hours_rail",
    "min_trip_duration_sec",
    "max_headway_warning_min",
    "max_headway_warning_min_rail",
    "service_day_window_hours_rail",
    "bunching_threshold_min",
    "rail_stop_distance_km",
    "max_trips_per_route",
    "duration_outlier_sigma",
    "headway_outlier_sigma",
    "service_day_start_hour",
    "max_calendar_future_years",
    "source_url",
    "stop_name_best_practices",
    "rural_route_ids",
    "calendar_override_rules",
    "gtfs_jp_profile",
    "disabled_rule_ids",
    "disabled_rule_ids_gtfs_jp",
];

/// GTFS-JP rule coverage profile.
///
/// `Auto` is the default and preserves the existing legacy behavior without making
/// a version claim; this includes the old compatibility checks for `routes_jp.txt`.
/// `V3` selects the Japan-specific extension files remaining in V3 and the backward
/// compatibility checks for `routes_jp.txt`; `V4` selects the current reference
/// coverage. The selected profile applies only when the feed is detected as GTFS-JP.
/// The version is not inferred automatically from the feed contents.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum GtfsJpProfile {
    #[default]
    Auto,
    V3,
    V4,
}

impl GtfsJpProfile {
    /// JP validation is enabled only when the feed carries a GTFS-JP signal.
    ///
    /// The selected profile chooses the version-specific rule set; it must not
    /// turn a non-JP feed into a GTFS-JP feed and produce unrelated JPN notices.
    pub const fn jp_validation_enabled(self, detected: bool) -> bool {
        let _ = self;
        detected
    }

    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Auto => "auto",
            Self::V3 => "v3",
            Self::V4 => "v4",
        }
    }
}

/// Validator parameters. All thresholds come from architecture v0.8, Section 6.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ValidatorConfig {
    /// External publication URL for analyses started from a URL; None in upload mode.
    pub source_url: Option<String>,
    /// GTFS-JP rule coverage. The default `Auto` preserves current behavior without
    /// making a version claim; V3/V4 can be selected explicitly.
    #[serde(default)]
    pub gtfs_jp_profile: GtfsJpProfile,
    /// STP_040/041: language-dependent stop-naming best-practice checks (disabled by default).
    pub stop_name_best_practices: bool,
    pub max_speed_bus_kmh: f64,
    pub max_speed_tram_kmh: f64,
    pub max_speed_metro_kmh: f64,
    pub max_speed_rail_kmh: f64,
    pub max_speed_ferry_kmh: f64,
    pub max_speed_cablecar_kmh: f64,
    pub min_transfer_time_sec: u32,
    pub max_transfer_distance_m: f64,
    pub max_shape_jump_km: f64,
    /// GEO_006/007: separate jump threshold (km) for rail types (route_type 2/12/100-117).
    /// Intercity rail geometry is legitimately simplified on long, straight sections —
    /// the 13.4 km between two consecutive shape points on the Konya-Karaman route is
    /// real geometry, not a gap. The 10 km threshold remains for urban types, where that
    /// gap indicates a missing point.
    pub max_shape_jump_km_rail: f64,
    pub stop_too_close_m: f64,
    pub stop_far_from_shape_m: f64,
    /// GEO_009/SHP_012/014/024: separate stop-to-route distance threshold (m) for rail
    /// types (route_type 2/12/100-117). At train stations, the stop coordinate represents
    /// the platform/building center while the shape is a single track centerline; 100 m
    /// would make a legitimate deviation a false positive in this structure. The 100 m
    /// threshold remains for urban types.
    pub stop_far_from_shape_m_rail: f64,
    pub stop_far_from_parent_m: f64,
    pub feed_expiry_warning_days: u32,
    /// FIN_019: near-expiry warning window (days) for feed_info.feed_end_date.
    /// Kept separate from CAL_008's service-calendar threshold; the default is 7 days.
    pub feed_info_expiry_warning_days: u32,
    /// CAL_010: "very short service" when a service has fewer active days than this threshold.
    pub service_gap_days: u32,
    /// CAL_007/012: "large gap" threshold (days) between consecutive active dates of a service.
    /// Aligned with MobilityData `big_gap_in_service` (14); kept separate from
    /// `service_gap_days` (very short service) so small gaps are not flagged as large gaps.
    pub big_gap_days: u32,
    /// CAL_021: how many days from today to search for an active trip (operational freshness window).
    pub upcoming_service_days: u32,
    pub max_trip_duration_hours: f64,
    /// STM_028: separate upper threshold for rail types (route_type 2/12/100-117). Long-distance
    /// train schedules legitimately exceed 24 hours — Ankara-Kars 26:26 and Ankara-Tatvan
    /// 26:37 are real trips. The 24-hour threshold remains for urban types, where that
    /// duration indicates a data error.
    pub max_trip_duration_hours_rail: f64,
    pub min_trip_duration_sec: u32,
    pub max_headway_warning_min: u32,
    /// OPR_001: separate headway threshold (min) for rail types (route_type 2/12/100-117).
    /// An hourly train is not expected on intercity rail — a 580-minute gap on the
    /// Ankara-Sivas YHT is normal operation, not a gap. The 240-minute threshold remains
    /// for urban types.
    pub max_headway_warning_min_rail: u32,
    /// STM_045: service-day window (hours) for rail types. In long-distance train schedules,
    /// departure times such as 30:37, 33:56, and 38:10 are VALID GTFS (post-midnight times
    /// are written as 24:xx+) and converting them to 24-hour times breaks trip chronology.
    /// For urban types, the window remains `24 + service_day_start_hour`.
    pub service_day_window_hours_rail: u32,
    pub bunching_threshold_min: u32,
    /// STM_026 stop-to-stop distance threshold (km) for rail (route_type 2/12/100-117) trips.
    pub rail_stop_distance_km: f64,
    /// OPR_024: suspected "data merge" when the number of trips for one route_id exceeds this threshold.
    pub max_trips_per_route: u32,
    /// SHP_026: suspected excessive detail when one shape has more points than this threshold.
    /// ⚠️ The threshold moved here from HARD-CODED code on 2026-08-07 — solely so a rule
    /// fixture could set it (`OPR_024` precedent). Default behavior DID NOT CHANGE.
    pub max_shape_points: u32,
    /// STM_043: when one trip has more stops than this threshold. Same rationale.
    pub max_stops_per_trip: u32,
    /// STM_044: WASM performance warning when the feed's total stop_times rows exceed this threshold.
    pub max_total_stop_times: u32,
    /// ARC_022: when a file read outside the stream exceeds this row-count threshold.
    pub max_file_rows: u32,
    /// VAT_003: how many robust-σ (MAD) deviations the trip duration may have from the
    /// expected value (residual) for the same pattern+time band before it is an outlier.
    /// The residual scale is tight after intraday deseasonalization, so a conservative
    /// value (3.5) is used.
    pub duration_outlier_sigma: f64,
    /// OPR_005: how many robust-σ (MAD) deviations a route's mean headway may have from the
    /// median of routes with the same route_type before it is considered "unusually
    /// frequent/sparse".
    pub headway_outlier_sigma: f64,
    /// Service-day start hour (0–6). Times (00:xx) of trips crossing midnight are normalized
    /// to service-day notation (24:xx) when below this threshold; this prevents false
    /// positives such as STM_008's "time goes backward". 0 = normalization disabled.
    pub service_day_start_hour: u32,
    /// CAL_023: calendar/calendar_dates end_date is considered suspicious data when it is
    /// more than this many years in the future (e.g. 2050/2099). A low-quality/incorrect-date signal.
    pub max_calendar_future_years: u32,
    /// Manual rural-route override for OPR_001 (sparse service / large departure gap).
    /// OPR_001 is never produced for these route_ids (intentionally sparse service). A
    /// safety net for routes misclassified by automatic CV-based detection. Empty = automatic only.
    pub rural_route_ids: Vec<String>,
    /// Calendar override rules — for OPR_021/022/023.
    /// Empty by default; no override analysis is performed.
    pub calendar_override_rules: Vec<CalendarOverrideRule>,
    /// Rules not reported only for feeds detected as GTFS-JP.
    ///
    /// Publishing practices in Japan make some checks meaningless (for example, warnings
    /// caused only by Japan-specific reasons); this list applies to those feeds and does
    /// not CHANGE behavior for feeds elsewhere in the world. The general list
    /// (`disabled_rule_ids`) continues to apply to every feed.
    pub disabled_rule_ids_gtfs_jp: Vec<String>,
    /// Rule IDs the user does not want reported.
    ///
    /// Rules continue to run; notices are removed before K7 reporting. This allows
    /// low-priority or locally unnecessary field checks (such as `DQ_003`, `DQ_004`, and
    /// `TRP_021`) to be disabled by profile.
    pub disabled_rule_ids: Vec<String>,
}

impl Default for ValidatorConfig {
    fn default() -> Self {
        Self {
            source_url: None,
            gtfs_jp_profile: GtfsJpProfile::Auto,
            stop_name_best_practices: false,
            max_speed_bus_kmh: DEF_MAX_SPEED_BUS_KMH,
            max_speed_tram_kmh: DEF_MAX_SPEED_TRAM_KMH,
            max_speed_metro_kmh: DEF_MAX_SPEED_METRO_KMH,
            max_speed_rail_kmh: DEF_MAX_SPEED_RAIL_KMH,
            max_speed_ferry_kmh: DEF_MAX_SPEED_FERRY_KMH,
            max_speed_cablecar_kmh: DEF_MAX_SPEED_CABLECAR_KMH,
            min_transfer_time_sec: DEF_MIN_TRANSFER_TIME_SEC,
            max_transfer_distance_m: DEF_MAX_TRANSFER_DISTANCE_M,
            max_shape_jump_km: DEF_MAX_SHAPE_JUMP_KM,
            max_shape_jump_km_rail: DEF_MAX_SHAPE_JUMP_KM_RAIL,
            stop_too_close_m: DEF_STOP_TOO_CLOSE_M,
            stop_far_from_shape_m: DEF_STOP_FAR_FROM_SHAPE_M,
            stop_far_from_shape_m_rail: DEF_STOP_FAR_FROM_SHAPE_M_RAIL,
            stop_far_from_parent_m: DEF_STOP_FAR_FROM_PARENT_M,
            feed_expiry_warning_days: DEF_FEED_EXPIRY_WARNING_DAYS,
            feed_info_expiry_warning_days: DEF_FEED_INFO_EXPIRY_WARNING_DAYS,
            service_gap_days: DEF_SERVICE_GAP_DAYS,
            big_gap_days: DEF_BIG_GAP_DAYS,
            upcoming_service_days: DEF_UPCOMING_SERVICE_DAYS,
            max_trip_duration_hours: DEF_MAX_TRIP_DURATION_HOURS,
            max_trip_duration_hours_rail: DEF_MAX_TRIP_DURATION_HOURS_RAIL,
            min_trip_duration_sec: DEF_MIN_TRIP_DURATION_SEC,
            max_headway_warning_min: DEF_MAX_HEADWAY_WARNING_MIN,
            max_headway_warning_min_rail: DEF_MAX_HEADWAY_WARNING_MIN_RAIL,
            service_day_window_hours_rail: DEF_SERVICE_DAY_WINDOW_HOURS_RAIL,
            bunching_threshold_min: DEF_BUNCHING_THRESHOLD_MIN,
            rail_stop_distance_km: DEF_RAIL_STOP_DISTANCE_KM,
            max_trips_per_route: DEF_MAX_TRIPS_PER_ROUTE,
            max_shape_points: 5_000,
            max_stops_per_trip: 200,
            max_total_stop_times: 2_000_000,
            max_file_rows: 1_000_000,
            duration_outlier_sigma: DEF_DURATION_OUTLIER_SIGMA,
            headway_outlier_sigma: DEF_HEADWAY_OUTLIER_SIGMA,
            service_day_start_hour: DEF_SERVICE_DAY_START_HOUR,
            max_calendar_future_years: DEF_MAX_CALENDAR_FUTURE_YEARS,
            rural_route_ids: Vec::new(),
            calendar_override_rules: Vec::new(),
            disabled_rule_ids: Vec::new(),
            disabled_rule_ids_gtfs_jp: Vec::new(),
        }
    }
}

fn is_leap_year(y: u32) -> bool {
    (y.is_multiple_of(4) && !y.is_multiple_of(100)) || y.is_multiple_of(400)
}

fn days_in_month(y: u32, m: u32) -> u32 {
    match m {
        1 | 3 | 5 | 7 | 8 | 10 | 12 => 31,
        4 | 6 | 9 | 11 => 30,
        2 => {
            if is_leap_year(y) {
                29
            } else {
                28
            }
        }
        _ => 0,
    }
}

fn valid_yyyymmdd(d: u32) -> bool {
    let y = d / 10_000;
    let m = (d / 100) % 100;
    let day = d % 100;
    (2000..=2099).contains(&y) && (1..=12).contains(&m) && day >= 1 && day <= days_in_month(y, m)
}

/// Checks parameter ranges defined in Taxonomy Section 6.
/// Called after `merge_delta` has been applied successfully.
fn validate_ranges(cfg: &ValidatorConfig) -> Result<(), String> {
    macro_rules! chk_f64 {
        ($field:expr, $name:literal, $lo:expr, $hi:expr) => {
            if $field < $lo || $field > $hi {
                return Err(format!(
                    "'{}' out of range: {} (expected {}-{})",
                    $name, $field, $lo, $hi
                ));
            }
        };
    }
    macro_rules! chk_u32 {
        ($field:expr, $name:literal, $lo:expr, $hi:expr) => {
            if $field < $lo || $field > $hi {
                return Err(format!(
                    "'{}' out of range: {} (expected {}-{})",
                    $name, $field, $lo, $hi
                ));
            }
        };
    }
    chk_f64!(cfg.max_speed_bus_kmh, "max_speed_bus_kmh", 60.0, 200.0);
    chk_f64!(cfg.max_speed_tram_kmh, "max_speed_tram_kmh", 40.0, 160.0);
    chk_f64!(cfg.max_speed_metro_kmh, "max_speed_metro_kmh", 80.0, 250.0);
    chk_f64!(cfg.max_speed_rail_kmh, "max_speed_rail_kmh", 100.0, 400.0);
    chk_f64!(cfg.max_speed_ferry_kmh, "max_speed_ferry_kmh", 20.0, 150.0);
    chk_f64!(
        cfg.max_speed_cablecar_kmh,
        "max_speed_cablecar_kmh",
        10.0,
        60.0
    );
    chk_u32!(cfg.min_transfer_time_sec, "min_transfer_time_sec", 30, 1800);
    chk_f64!(
        cfg.max_transfer_distance_m,
        "max_transfer_distance_m",
        50.0,
        2000.0
    );
    chk_f64!(cfg.max_shape_jump_km, "max_shape_jump_km", 1.0, 50.0);
    chk_f64!(
        cfg.max_shape_jump_km_rail,
        "max_shape_jump_km_rail",
        1.0,
        100.0
    );
    chk_f64!(cfg.stop_too_close_m, "stop_too_close_m", 1.0, 20.0);
    chk_f64!(
        cfg.stop_far_from_shape_m,
        "stop_far_from_shape_m",
        20.0,
        500.0
    );
    chk_f64!(
        cfg.stop_far_from_shape_m_rail,
        "stop_far_from_shape_m_rail",
        20.0,
        1000.0
    );
    chk_f64!(
        cfg.stop_far_from_parent_m,
        "stop_far_from_parent_m",
        10.0,
        1000.0
    );
    chk_u32!(
        cfg.feed_expiry_warning_days,
        "feed_expiry_warning_days",
        1,
        60
    );
    chk_u32!(
        cfg.feed_info_expiry_warning_days,
        "feed_info_expiry_warning_days",
        1,
        60
    );
    chk_u32!(cfg.service_gap_days, "service_gap_days", 3, 30);
    chk_u32!(cfg.big_gap_days, "big_gap_days", 7, 60);
    chk_u32!(cfg.upcoming_service_days, "upcoming_service_days", 1, 90);
    chk_f64!(
        cfg.max_trip_duration_hours,
        "max_trip_duration_hours",
        8.0,
        72.0
    );
    chk_f64!(
        cfg.max_trip_duration_hours_rail,
        "max_trip_duration_hours_rail",
        8.0,
        168.0
    );
    chk_u32!(cfg.min_trip_duration_sec, "min_trip_duration_sec", 10, 300);
    chk_u32!(
        cfg.max_headway_warning_min,
        "max_headway_warning_min",
        60,
        720
    );
    chk_u32!(
        cfg.max_headway_warning_min_rail,
        "max_headway_warning_min_rail",
        60,
        2880
    );
    chk_u32!(
        cfg.service_day_window_hours_rail,
        "service_day_window_hours_rail",
        24,
        120
    );
    chk_u32!(cfg.bunching_threshold_min, "bunching_threshold_min", 1, 10);
    chk_f64!(
        cfg.rail_stop_distance_km,
        "rail_stop_distance_km",
        50.0,
        2000.0
    );
    chk_u32!(cfg.max_trips_per_route, "max_trips_per_route", 50, 20000);
    chk_u32!(cfg.max_shape_points, "max_shape_points", 10, 1_000_000);
    chk_u32!(cfg.max_stops_per_trip, "max_stops_per_trip", 5, 100_000);
    chk_u32!(
        cfg.max_total_stop_times,
        "max_total_stop_times",
        100,
        100_000_000
    );
    chk_u32!(cfg.max_file_rows, "max_file_rows", 10, 100_000_000);
    chk_f64!(
        cfg.duration_outlier_sigma,
        "duration_outlier_sigma",
        1.0,
        6.0
    );
    chk_f64!(cfg.headway_outlier_sigma, "headway_outlier_sigma", 1.0, 6.0);
    // service_day_start_hour: the u32 lower bound is 0, so checking `< 0` is meaningless
    // due to the type limit (chk_u32! would warn "comparison is useless" here); check only
    // the upper bound.
    if cfg.service_day_start_hour > 6 {
        return Err(format!(
            "'{}' out of range: {} (expected {}-{})",
            "service_day_start_hour", cfg.service_day_start_hour, 0, 6
        ));
    }
    chk_u32!(
        cfg.max_calendar_future_years,
        "max_calendar_future_years",
        1,
        50
    );
    Ok(())
}

/// Applies a JSON delta to the base config.
///
/// Contract:
/// - An unknown key returns `Err`; the base is unchanged.
/// - After all known keys are applied, returns `Ok(yeni_config)`.
/// - A numeric type mismatch (a string where f64 is required, etc.) returns `Err`.
pub fn merge_delta(base: &ValidatorConfig, delta_json: &str) -> Result<ValidatorConfig, String> {
    let map: serde_json::Map<String, serde_json::Value> =
        serde_json::from_str(delta_json).map_err(|e| format!("JSON parse error: {e}"))?;

    // Collect unknown keys first — nothing is applied if the result is Err.
    let unknown: Vec<&str> = map
        .keys()
        .filter(|k| !KNOWN_CONFIG_KEYS.contains(&k.as_str()))
        .map(String::as_str)
        .collect();
    if !unknown.is_empty() {
        return Err(format!(
            "Unknown config keys: {}",
            unknown.join(", ")
        ));
    }

    let mut cfg = base.clone();

    if let Some(v) = map.get("stop_name_best_practices") {
        cfg.stop_name_best_practices = v.as_bool().ok_or_else(|| {
            format!("'stop_name_best_practices' expects a bool, got: {v}")
        })?;
    }
    if let Some(v) = map.get("source_url") {
        cfg.source_url = if v.is_null() {
            None
        } else {
            Some(
                v.as_str()
                    .ok_or_else(|| {
                        format!("'source_url' expects a string or null, got: {v}")
                    })?
                    .trim()
                    .to_string(),
            )
        };
        if cfg.source_url.as_deref() == Some("") {
            cfg.source_url = None;
        }
    }

    if let Some(v) = map.get("gtfs_jp_profile") {
        cfg.gtfs_jp_profile = serde_json::from_value(v.clone()).map_err(|e| {
            format!("'gtfs_jp_profile' parse error (expected auto, v3, or v4): {e}")
        })?;
    }

    macro_rules! apply_f64 {
        ($key:literal, $field:expr) => {
            if let Some(v) = map.get($key) {
                $field = v
                    .as_f64()
                    .ok_or_else(|| format!("'{}' expects f64, got: {v}", $key))?;
            }
        };
    }
    macro_rules! apply_u32 {
        ($key:literal, $field:expr) => {
            if let Some(v) = map.get($key) {
                let n = v
                    .as_u64()
                    .ok_or_else(|| format!("'{}' expects u32, got: {v}", $key))?;
                $field = u32::try_from(n)
                    .map_err(|_| format!("'{}' exceeds the u32 limit: {n}", $key))?;
            }
        };
    }

    apply_f64!("max_speed_bus_kmh", cfg.max_speed_bus_kmh);
    apply_f64!("max_speed_tram_kmh", cfg.max_speed_tram_kmh);
    apply_f64!("max_speed_metro_kmh", cfg.max_speed_metro_kmh);
    apply_f64!("max_speed_rail_kmh", cfg.max_speed_rail_kmh);
    apply_f64!("max_speed_ferry_kmh", cfg.max_speed_ferry_kmh);
    apply_f64!("max_speed_cablecar_kmh", cfg.max_speed_cablecar_kmh);
    apply_u32!("min_transfer_time_sec", cfg.min_transfer_time_sec);
    apply_f64!("max_transfer_distance_m", cfg.max_transfer_distance_m);
    apply_f64!("max_shape_jump_km", cfg.max_shape_jump_km);
    apply_f64!("max_shape_jump_km_rail", cfg.max_shape_jump_km_rail);
    apply_f64!("stop_too_close_m", cfg.stop_too_close_m);
    apply_f64!("stop_far_from_shape_m", cfg.stop_far_from_shape_m);
    apply_f64!("stop_far_from_shape_m_rail", cfg.stop_far_from_shape_m_rail);
    apply_f64!("stop_far_from_parent_m", cfg.stop_far_from_parent_m);
    apply_u32!("feed_expiry_warning_days", cfg.feed_expiry_warning_days);
    apply_u32!(
        "feed_info_expiry_warning_days",
        cfg.feed_info_expiry_warning_days
    );
    apply_u32!("service_gap_days", cfg.service_gap_days);
    apply_u32!("big_gap_days", cfg.big_gap_days);
    apply_u32!("upcoming_service_days", cfg.upcoming_service_days);
    apply_f64!("max_trip_duration_hours", cfg.max_trip_duration_hours);
    apply_f64!(
        "max_trip_duration_hours_rail",
        cfg.max_trip_duration_hours_rail
    );
    apply_u32!("min_trip_duration_sec", cfg.min_trip_duration_sec);
    apply_u32!("max_headway_warning_min", cfg.max_headway_warning_min);
    apply_u32!(
        "max_headway_warning_min_rail",
        cfg.max_headway_warning_min_rail
    );
    apply_u32!(
        "service_day_window_hours_rail",
        cfg.service_day_window_hours_rail
    );
    apply_u32!("bunching_threshold_min", cfg.bunching_threshold_min);
    apply_f64!("rail_stop_distance_km", cfg.rail_stop_distance_km);
    apply_u32!("max_trips_per_route", cfg.max_trips_per_route);
    apply_f64!("duration_outlier_sigma", cfg.duration_outlier_sigma);
    apply_f64!("headway_outlier_sigma", cfg.headway_outlier_sigma);
    apply_u32!("service_day_start_hour", cfg.service_day_start_hour);
    apply_u32!("max_calendar_future_years", cfg.max_calendar_future_years);

    if let Some(v) = map.get("rural_route_ids") {
        cfg.rural_route_ids = serde_json::from_value(v.clone())
            .map_err(|e| format!("'rural_route_ids' parse error: {e}"))?;
    }

    if let Some(v) = map.get("calendar_override_rules") {
        cfg.calendar_override_rules = serde_json::from_value(v.clone())
            .map_err(|e| format!("'calendar_override_rules' parse error: {e}"))?;
        for (i, rule) in cfg.calendar_override_rules.iter().enumerate() {
            if rule.route_id.is_empty() {
                return Err(format!("calendar_override_rules[{i}]: route_id must not be empty"));
            }
            if rule.base_service_ids.is_empty() {
                return Err(format!(
                    "calendar_override_rules[{i}]: base_service_ids must not be empty"
                ));
            }
            if rule.override_service_ids.is_empty() {
                return Err(format!(
                    "calendar_override_rules[{i}]: override_service_ids must not be empty"
                ));
            }
            if !valid_yyyymmdd(rule.start_date) {
                return Err(format!(
                    "calendar_override_rules[{i}]: start_date '{}' is not a valid calendar date (YYYYMMDD, 2000-2099)",
                    rule.start_date
                ));
            }
            if !valid_yyyymmdd(rule.end_date) {
                return Err(format!(
                    "calendar_override_rules[{i}]: end_date '{}' is not a valid calendar date (YYYYMMDD, 2000-2099)",
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

    for key in ["disabled_rule_ids", "disabled_rule_ids_gtfs_jp"] {
        let Some(v) = map.get(key) else { continue };
        let mut ids: Vec<String> = serde_json::from_value(v.clone())
            .map_err(|e| format!("'{key}' parse error: {e}"))?;
        for (i, rule_id) in ids.iter_mut().enumerate() {
            *rule_id = rule_id.trim().to_string();
            if rule_id.is_empty() {
                return Err(format!("{key}[{i}] must not be empty"));
            }
        }
        ids.sort_unstable();
        ids.dedup();
        if key == "disabled_rule_ids" {
            cfg.disabled_rule_ids = ids;
        } else {
            cfg.disabled_rule_ids_gtfs_jp = ids;
        }
    }

    if cfg.rural_route_ids.len() > MAX_RURAL_ROUTE_IDS {
        return Err(format!(
            "'rural_route_ids' exceeds the {}-item safety limit",
            MAX_RURAL_ROUTE_IDS
        ));
    }
    if cfg.calendar_override_rules.len() > MAX_CALENDAR_OVERRIDE_RULES {
        return Err(format!(
            "'calendar_override_rules' exceeds the {}-item safety limit",
            MAX_CALENDAR_OVERRIDE_RULES
        ));
    }
    for (key, ids) in [
        ("disabled_rule_ids", &cfg.disabled_rule_ids),
        ("disabled_rule_ids_gtfs_jp", &cfg.disabled_rule_ids_gtfs_jp),
    ] {
        if ids.len() > MAX_DISABLED_RULE_IDS {
            return Err(format!(
                "'{key}' exceeds the {}-item safety limit",
                MAX_DISABLED_RULE_IDS
            ));
        }
    }
    let total_config_string_bytes: usize = cfg
        .rural_route_ids
        .iter()
        .map(String::len)
        .chain(cfg.disabled_rule_ids.iter().map(String::len))
        .chain(cfg.disabled_rule_ids_gtfs_jp.iter().map(String::len))
        .chain(cfg.calendar_override_rules.iter().flat_map(|rule| {
            std::iter::once(rule.route_id.len())
                .chain(rule.base_service_ids.iter().map(String::len))
                .chain(rule.override_service_ids.iter().map(String::len))
        }))
        .sum();
    if total_config_string_bytes > MAX_CONFIG_STRING_BYTES {
        return Err(format!(
            "config string fields exceed the {}-byte safety limit",
            MAX_CONFIG_STRING_BYTES
        ));
    }
    for (i, rule) in cfg.calendar_override_rules.iter().enumerate() {
        if rule.base_service_ids.len() > MAX_OVERRIDE_SERVICE_IDS_PER_RULE
            || rule.override_service_ids.len() > MAX_OVERRIDE_SERVICE_IDS_PER_RULE
        {
            return Err(format!(
                "calendar_override_rules[{i}] service list exceeds the {}-item safety limit",
                MAX_OVERRIDE_SERVICE_IDS_PER_RULE
            ));
        }
    }

    validate_ranges(&cfg)?;
    Ok(cfg)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn jp_validation_requires_detection_for_every_profile() {
        for profile in [GtfsJpProfile::Auto, GtfsJpProfile::V3, GtfsJpProfile::V4] {
            assert!(!profile.jp_validation_enabled(false));
            assert!(profile.jp_validation_enabled(true));
        }
    }

    #[test]
    fn default_values() {
        let cfg = ValidatorConfig::default();
        assert_eq!(cfg.gtfs_jp_profile, GtfsJpProfile::Auto);
        assert_eq!(cfg.max_speed_bus_kmh, 120.0);
        assert_eq!(cfg.max_speed_tram_kmh, 100.0);
        assert_eq!(cfg.max_speed_metro_kmh, 150.0);
        assert_eq!(cfg.max_speed_rail_kmh, 300.0);
        assert_eq!(cfg.max_speed_ferry_kmh, 80.0);
        assert_eq!(cfg.max_speed_cablecar_kmh, 30.0);
        assert_eq!(cfg.min_transfer_time_sec, 180);
        assert_eq!(cfg.max_transfer_distance_m, 500.0);
        assert_eq!(cfg.max_shape_jump_km, 10.0);
        assert_eq!(cfg.max_shape_jump_km_rail, 30.0);
        assert_eq!(cfg.stop_too_close_m, 5.0);
        assert_eq!(cfg.stop_far_from_shape_m, 100.0);
        assert_eq!(cfg.stop_far_from_shape_m_rail, 200.0);
        assert_eq!(cfg.feed_expiry_warning_days, 30);
        assert_eq!(cfg.feed_info_expiry_warning_days, 7);
        assert_eq!(cfg.service_gap_days, 7);
        assert_eq!(cfg.upcoming_service_days, 7);
        assert_eq!(cfg.max_trip_duration_hours, 24.0);
        assert_eq!(cfg.min_trip_duration_sec, 60);
        assert_eq!(cfg.max_headway_warning_min, 240);
        assert_eq!(cfg.max_headway_warning_min_rail, 720);
        assert_eq!(cfg.service_day_window_hours_rail, 48);
        assert_eq!(cfg.bunching_threshold_min, 2);
    }

    #[test]
    fn merge_delta_applies_known_keys() {
        let base = ValidatorConfig::default();
        let result = merge_delta(
            &base,
            r#"{"max_speed_bus_kmh": 90.0, "service_gap_days": 14}"#,
        );
        let cfg = result.expect("merge should succeed");
        assert_eq!(cfg.max_speed_bus_kmh, 90.0);
        assert_eq!(cfg.service_gap_days, 14);
        // Untouched fields must not change.
        assert_eq!(cfg.max_speed_rail_kmh, 300.0);
        assert_eq!(cfg.min_transfer_time_sec, 180);
    }

    #[test]
    fn gtfs_jp_profile_is_an_explicit_config_delta() {
        let base = ValidatorConfig::default();
        let cfg = merge_delta(&base, r#"{"gtfs_jp_profile":"v4"}"#)
            .expect("v4 profile selection should be accepted");
        assert_eq!(cfg.gtfs_jp_profile, GtfsJpProfile::V4);
        assert_eq!(cfg.gtfs_jp_profile.as_str(), "v4");

        let err = merge_delta(&base, r#"{"gtfs_jp_profile":"v5"}"#)
            .expect_err("unknown profile should be rejected");
        assert!(
            err.contains("gtfs_jp_profile"),
            "the error should identify the profile field: {err}"
        );
    }

    #[test]
    fn feed_info_expiry_warning_horizon_is_independent() {
        let base = ValidatorConfig::default();
        let cfg = merge_delta(&base, r#"{"feed_info_expiry_warning_days": 30}"#)
            .expect("feed_info expiry threshold should be applied");
        assert_eq!(cfg.feed_info_expiry_warning_days, 30);
        assert_eq!(
            cfg.feed_expiry_warning_days, 30,
            "the CAL_008 threshold should remain separate"
        );
    }

    #[test]
    fn merge_delta_rejects_unknown_key() {
        let base = ValidatorConfig::default();
        let err = merge_delta(&base, r#"{"unknown_param": 42}"#)
            .expect_err("an unknown key should return Err");
        assert_eq!(err, "Unknown config keys: unknown_param");
        assert!(
            err.contains("unknown_param"),
            "the error message should contain the key name: {err}"
        );
    }

    #[test]
    fn merge_delta_rejects_partial_on_unknown() {
        // One known key plus one unknown key → cancel the entire override.
        let base = ValidatorConfig::default();
        let err = merge_delta(&base, r#"{"max_speed_bus_kmh": 50.0, "nonexistent": true}"#)
            .expect_err("an unknown key should produce Err");
        assert!(err.contains("nonexistent"));
    }

    #[test]
    fn merge_delta_rejects_wrong_type() {
        let base = ValidatorConfig::default();
        let err = merge_delta(&base, r#"{"max_speed_bus_kmh": "hız"}"#)
            .expect_err("a type mismatch should return Err");
        assert!(err.contains("max_speed_bus_kmh"));
    }

    #[test]
    fn merge_delta_empty_delta_is_identity() {
        let base = ValidatorConfig::default();
        let cfg = merge_delta(&base, "{}").expect("an empty delta is valid");
        assert_eq!(cfg, base);
    }

    #[test]
    fn merge_delta_invalid_json() {
        let base = ValidatorConfig::default();
        assert!(merge_delta(&base, "not json").is_err());
    }

    // ── Range tests ───────────────────────────────────────────────────────────

    #[test]
    fn range_below_min_rejected() {
        let base = ValidatorConfig::default();
        // max_speed_bus_kmh lower bound = 60
        let err = merge_delta(&base, r#"{"max_speed_bus_kmh": 59.9}"#)
            .expect_err("below the lower bound should return Err");
        assert_eq!(
            err,
            "'max_speed_bus_kmh' out of range: 59.9 (expected 60-200)"
        );
        assert!(err.contains("max_speed_bus_kmh"), "{err}");

        // bunching_threshold_min lower bound = 1
        let err = merge_delta(&base, r#"{"bunching_threshold_min": 0}"#)
            .expect_err("below the lower bound should return Err");
        assert!(err.contains("bunching_threshold_min"), "{err}");

        // stop_far_from_shape_m lower bound = 20
        let err = merge_delta(&base, r#"{"stop_far_from_shape_m": 19.9}"#)
            .expect_err("below the lower bound should return Err");
        assert!(err.contains("stop_far_from_shape_m"), "{err}");

        // stop_far_from_shape_m_rail lower bound = 20 (the rail threshold is checked too)
        let err = merge_delta(&base, r#"{"stop_far_from_shape_m_rail": 19.9}"#)
            .expect_err("below the lower bound should return Err");
        assert!(err.contains("stop_far_from_shape_m_rail"), "{err}");

        let err = merge_delta(&base, r#"{"feed_info_expiry_warning_days": 0}"#)
            .expect_err("feed_info expiry lower bound should be rejected");
        assert!(err.contains("feed_info_expiry_warning_days"), "{err}");
    }

    /// Rail thresholds can be configured with a delta without affecting urban counterparts.
    #[test]
    fn rail_geometry_thresholds_are_configurable() {
        let base = ValidatorConfig::default();
        let cfg = merge_delta(
            &base,
            r#"{"stop_far_from_shape_m_rail": 350.0, "max_shape_jump_km_rail": 60.0}"#,
        )
        .expect("valid rail thresholds should be accepted");
        assert_eq!(cfg.stop_far_from_shape_m_rail, 350.0);
        assert_eq!(cfg.max_shape_jump_km_rail, 60.0);
        // Urban thresholds remain untouched.
        assert_eq!(cfg.stop_far_from_shape_m, 100.0);
        assert_eq!(cfg.max_shape_jump_km, 10.0);

        // max_shape_jump_km_rail upper bound = 100
        let err = merge_delta(&base, r#"{"max_shape_jump_km_rail": 100.1}"#)
            .expect_err("above the upper bound should return Err");
        assert!(err.contains("max_shape_jump_km_rail"), "{err}");
    }

    #[test]
    fn range_above_max_rejected() {
        let base = ValidatorConfig::default();
        // max_speed_rail_kmh upper bound = 400
        let err = merge_delta(&base, r#"{"max_speed_rail_kmh": 400.1}"#)
            .expect_err("above the upper bound should return Err");
        assert!(err.contains("max_speed_rail_kmh"), "{err}");

        // service_gap_days upper bound = 30
        let err = merge_delta(&base, r#"{"service_gap_days": 31}"#)
            .expect_err("above the upper bound should return Err");
        assert!(err.contains("service_gap_days"), "{err}");

        // max_headway_warning_min upper bound = 720
        let err = merge_delta(&base, r#"{"max_headway_warning_min": 721}"#)
            .expect_err("above the upper bound should return Err");
        assert!(err.contains("max_headway_warning_min"), "{err}");
    }

    #[test]
    fn range_boundary_values_accepted() {
        let base = ValidatorConfig::default();
        // Lower bound: max_speed_bus_kmh = 60
        let cfg =
            merge_delta(&base, r#"{"max_speed_bus_kmh": 60.0}"#).expect("lower-bound value is valid");
        assert_eq!(cfg.max_speed_bus_kmh, 60.0);

        // Upper bound: max_speed_rail_kmh = 400
        let cfg = merge_delta(&base, r#"{"max_speed_rail_kmh": 400.0}"#)
            .expect("upper-bound value is valid");
        assert_eq!(cfg.max_speed_rail_kmh, 400.0);

        // u32 bounds: bunching_threshold_min = 1 (lower) and 10 (upper)
        let cfg =
            merge_delta(&base, r#"{"bunching_threshold_min": 1}"#).expect("lower bound is valid");
        assert_eq!(cfg.bunching_threshold_min, 1);

        let cfg =
            merge_delta(&base, r#"{"bunching_threshold_min": 10}"#).expect("upper bound is valid");
        assert_eq!(cfg.bunching_threshold_min, 10);
    }

    #[test]
    fn range_all_fields_default_in_range() {
        // All default values must be within the taxonomy ranges.
        let base = ValidatorConfig::default();
        merge_delta(&base, "{}").expect("default config should fit all ranges");
    }

    // ── calendar_override_rules tests ─────────────────────────────────────────

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
        let cfg = merge_delta(&base, delta).expect("override rules should parse");
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
        let cfg = merge_delta(&base, delta).expect("multiple override rules should parse");
        assert_eq!(cfg.calendar_override_rules.len(), 2);
        assert_eq!(
            cfg.calendar_override_rules[1].base_service_ids,
            vec!["WKD", "SAT"]
        );
    }

    #[test]
    fn calendar_override_rules_invalid_type_rejected() {
        let base = ValidatorConfig::default();
        let err = merge_delta(&base, r#"{"calendar_override_rules": "not_an_array"}"#)
            .expect_err("an invalid type should return Err");
        assert!(err.contains("calendar_override_rules"), "{err}");
    }

    #[test]
    fn calendar_override_rules_empty_array_accepted() {
        let base = ValidatorConfig::default();
        let cfg =
            merge_delta(&base, r#"{"calendar_override_rules": []}"#).expect("an empty array is valid");
        assert!(cfg.calendar_override_rules.is_empty());
    }

    #[test]
    fn calendar_override_rules_empty_route_id_rejected() {
        let base = ValidatorConfig::default();
        let err = merge_delta(
            &base,
            r#"{"calendar_override_rules": [
            {"route_id": "", "base_service_ids": ["WKD"], "override_service_ids": ["HOL"],
             "start_date": 20260101, "end_date": 20260101}]}"#,
        )
        .expect_err("an empty route_id should return Err");
        assert!(err.contains("route_id"), "{err}");
    }

    #[test]
    fn calendar_override_rules_empty_base_rejected() {
        let base = ValidatorConfig::default();
        let err = merge_delta(
            &base,
            r#"{"calendar_override_rules": [
            {"route_id": "R1", "base_service_ids": [], "override_service_ids": ["HOL"],
             "start_date": 20260101, "end_date": 20260101}]}"#,
        )
        .expect_err("empty base_service_ids should return Err");
        assert!(err.contains("base_service_ids"), "{err}");
    }

    #[test]
    fn calendar_override_rules_empty_override_rejected() {
        let base = ValidatorConfig::default();
        let err = merge_delta(
            &base,
            r#"{"calendar_override_rules": [
            {"route_id": "R1", "base_service_ids": ["WKD"], "override_service_ids": [],
             "start_date": 20260101, "end_date": 20260101}]}"#,
        )
        .expect_err("empty override_service_ids should return Err");
        assert!(err.contains("override_service_ids"), "{err}");
    }

    #[test]
    fn calendar_override_rules_invalid_start_date_rejected() {
        let base = ValidatorConfig::default();
        // start_date=0 → invalid
        let err = merge_delta(
            &base,
            r#"{"calendar_override_rules": [
            {"route_id": "R1", "base_service_ids": ["WKD"], "override_service_ids": ["HOL"],
             "start_date": 0, "end_date": 20260101}]}"#,
        )
        .expect_err("start_date=0 should return Err");
        assert!(err.contains("start_date"), "{err}");
        // Month=13 → invalid
        let err = merge_delta(
            &base,
            r#"{"calendar_override_rules": [
            {"route_id": "R1", "base_service_ids": ["WKD"], "override_service_ids": ["HOL"],
             "start_date": 20261301, "end_date": 20261301}]}"#,
        )
        .expect_err("month=13 should return Err");
        assert!(err.contains("start_date"), "{err}");
    }

    #[test]
    fn calendar_override_rules_start_after_end_rejected() {
        let base = ValidatorConfig::default();
        let err = merge_delta(
            &base,
            r#"{"calendar_override_rules": [
            {"route_id": "R1", "base_service_ids": ["WKD"], "override_service_ids": ["HOL"],
             "start_date": 20260201, "end_date": 20260101}]}"#,
        )
        .expect_err("start_date > end_date should return Err");
        assert!(err.contains("start_date"), "{err}");
        assert!(err.contains("end_date"), "{err}");
    }

    // ── Real calendar-date validation tests ───────────────────────────────────

    #[test]
    fn valid_yyyymmdd_rejects_impossible_days() {
        // February 31 → does not exist in any year
        assert!(!valid_yyyymmdd(20260231));
        // April 31 → does not exist (April has 30 days)
        assert!(!valid_yyyymmdd(20260431));
        // June 31 → does not exist
        assert!(!valid_yyyymmdd(20260631));
        // September 31 → does not exist
        assert!(!valid_yyyymmdd(20260931));
        // November 31 → does not exist
        assert!(!valid_yyyymmdd(20261131));
        // Day=0 → invalid
        assert!(!valid_yyyymmdd(20260100));
    }

    #[test]
    fn valid_yyyymmdd_leap_year_feb29() {
        // 2024 is a leap year → February 29 is valid
        assert!(valid_yyyymmdd(20240229));
        // 2000 is a leap year (divisible by 400) → valid
        assert!(valid_yyyymmdd(20000229));
        // 2026 is not a leap year → February 29 is invalid
        assert!(!valid_yyyymmdd(20260229));
        // 2100 is out of range (>2099) → already invalid
        assert!(!valid_yyyymmdd(21000229));
    }

    #[test]
    fn valid_yyyymmdd_non_leap_year_feb28_ok() {
        // February 28 is valid in non-leap years.
        assert!(valid_yyyymmdd(20260228));
        assert!(valid_yyyymmdd(20250228));
    }

    #[test]
    fn valid_yyyymmdd_boundary_months() {
        // The last day of each 31-day month is valid.
        assert!(valid_yyyymmdd(20260131)); // January
        assert!(valid_yyyymmdd(20260331)); // March
        assert!(valid_yyyymmdd(20260531)); // May
        assert!(valid_yyyymmdd(20260731)); // July
        assert!(valid_yyyymmdd(20260831)); // August
        assert!(valid_yyyymmdd(20261031)); // October
        assert!(valid_yyyymmdd(20261231)); // December
                                           // The last day of each 30-day month is valid.
        assert!(valid_yyyymmdd(20260430)); // April
        assert!(valid_yyyymmdd(20260630)); // June
        assert!(valid_yyyymmdd(20260930)); // September
        assert!(valid_yyyymmdd(20261130)); // November
    }

    #[test]
    fn calendar_override_rules_feb31_rejected() {
        let base = ValidatorConfig::default();
        // 20260231 is invalid as a calendar date.
        let err = merge_delta(
            &base,
            r#"{"calendar_override_rules": [
            {"route_id": "R1", "base_service_ids": ["WKD"], "override_service_ids": ["HOL"],
             "start_date": 20260231, "end_date": 20260231}]}"#,
        )
        .expect_err("20260231 should return Err");
        assert!(err.contains("start_date"), "{err}");
    }

    #[test]
    fn calendar_override_rules_apr31_rejected() {
        let base = ValidatorConfig::default();
        let err = merge_delta(
            &base,
            r#"{"calendar_override_rules": [
            {"route_id": "R1", "base_service_ids": ["WKD"], "override_service_ids": ["HOL"],
             "start_date": 20260101, "end_date": 20260431}]}"#,
        )
        .expect_err("20260431 end_date should return Err");
        assert!(err.contains("end_date"), "{err}");
    }

    #[test]
    fn calendar_override_rules_non_leap_feb29_rejected() {
        let base = ValidatorConfig::default();
        // 2026 is not a leap year → February 29 is invalid
        let err = merge_delta(
            &base,
            r#"{"calendar_override_rules": [
            {"route_id": "R1", "base_service_ids": ["WKD"], "override_service_ids": ["HOL"],
             "start_date": 20260229, "end_date": 20260229}]}"#,
        )
        .expect_err("February 29 in a non-leap year should return Err");
        assert!(err.contains("start_date"), "{err}");
    }

    #[test]
    fn calendar_override_rules_leap_feb29_accepted() {
        let base = ValidatorConfig::default();
        // 2024 is a leap year → February 29 is valid
        let cfg = merge_delta(
            &base,
            r#"{"calendar_override_rules": [
            {"route_id": "R1", "base_service_ids": ["WKD"], "override_service_ids": ["HOL"],
             "start_date": 20240229, "end_date": 20240229}]}"#,
        )
        .expect("February 29 in a leap year should be accepted");
        assert_eq!(cfg.calendar_override_rules[0].start_date, 20240229);
    }

    #[test]
    fn stop_name_best_practices_is_opt_in() {
        let base = ValidatorConfig::default();
        assert!(!base.stop_name_best_practices);
        let cfg = merge_delta(&base, r#"{"stop_name_best_practices": true}"#).unwrap();
        assert!(cfg.stop_name_best_practices);
    }

    #[test]
    fn source_url_accepts_string_and_null() {
        let base = ValidatorConfig::default();
        let cfg = merge_delta(&base, r#"{"source_url":"https://example.org/gtfs.zip"}"#).unwrap();
        assert_eq!(
            cfg.source_url.as_deref(),
            Some("https://example.org/gtfs.zip")
        );
        let cfg = merge_delta(&cfg, r#"{"source_url":null}"#).unwrap();
        assert!(cfg.source_url.is_none());
    }

    #[test]
    fn disabled_rule_ids_are_trimmed_deduplicated_and_typed() {
        let base = ValidatorConfig::default();
        let cfg = merge_delta(
            &base,
            r#"{"disabled_rule_ids":[" DQ_004 ","DQ_003","DQ_004"]}"#,
        )
        .expect("disabled_rule_ids should be accepted");
        assert_eq!(cfg.disabled_rule_ids, ["DQ_003", "DQ_004"]);
    }

    #[test]
    fn disabled_rule_ids_reject_empty_values() {
        let base = ValidatorConfig::default();
        let err = merge_delta(&base, r#"{"disabled_rule_ids":["DQ_003", " "]}"#)
            .expect_err("an empty rule ID should be rejected");
        assert!(err.contains("disabled_rule_ids"), "{err}");
    }
}
