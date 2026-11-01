use serde::{Deserialize, Serialize};

/// Size and row information for one GTFS file.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileInfo {
    pub name: String,
    pub rows: u32,
    pub bytes: u32,
}

/// Feed-level summary metrics.
///
/// MET_001: entity counts
/// MET_002: service coverage density
/// MET_003: error counts by class
/// MET_004: overall score (equivalent to R5; Spec×40%+Interop×30%+Quality×20%+Analytics×10%)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FeedMetrics {
    // MET_001 — Total stop / route / trip / shape counts
    pub stop_count: u32,
    pub route_count: u32,
    pub trip_count: u32,
    pub shape_count: u32,

    // MET_002 — Service coverage density
    pub active_service_days: u32,
    pub avg_daily_trips: f64,

    // Feed validity window (feed_info.txt) — YYYYMMDD, None if absent
    pub feed_start_date: Option<u32>,
    pub feed_end_date: Option<u32>,

    // Actual service coverage range (calendar + calendar_dates) — YYYYMMDD, None if no service
    pub service_start_date: Option<u32>,
    pub service_end_date: Option<u32>,

    // MET_003 — Error counts by class
    pub spec_notice_count: u32,
    pub interop_notice_count: u32,
    pub quality_notice_count: u32,
    pub analytics_notice_count: u32,

    // MET_004 — Overall score 0.0–100.0 (R5 weighted: 0.4·spec + 0.3·interop + 0.2·quality + 0.1·analytics).
    // NOTE: Do not confuse this with R5Report.quality_score (the Quality-class component only).
    pub overall_score: f64,

    /// Did the score come from a FULL-COVERAGE run (issue #133)?
    ///
    /// 🔴 **If `false`, `overall_score` CANNOT BE COMPARED WITH OTHER FEEDS.** The score is
    /// calculated only from notices PRODUCED; when a file cannot be read, dependent rules
    /// do not run, cannot produce penalties, and the score INCREASES. Measured (`mdb-1903`):
    /// when `stops.txt` could not be read, 604 notices → 178 and the overall score **89,4 → 93,5**.
    /// Live corpus example `mdb-3135`: 2 notices, 62 skipped checks, score **94,5**.
    ///
    /// The score ITSELF is deliberately not adjusted: inventing a penalty per skipped check
    /// would produce a number not based on a real measurement. The correct solution is not
    /// to alter the number, but to carry the fact that it CANNOT BE COMPARED.
    #[serde(default = "default_true_metrics")]
    pub coverage_complete: bool,

    // File-level statistics (K1 parse)
    pub file_stats: Vec<FileInfo>,

    /// Whether the feed carries a GTFS-JP signal (`*_jp` file, `feed_lang=ja`,
    /// `agency_lang=ja` + `Asia/Tokyo`, or a `ja-Hrkt` translation).
    /// Used for the "GTFS-JP" badge in the UI and conditional triggering of JPN rules.
    #[serde(default)]
    pub is_gtfs_jp: bool,

    /// Selected rule coverage for GTFS-JP.
    ///
    /// This field does not detect the feed's official v3/v4 version; it only exposes the
    /// explicit profile selection used for analysis to UI/SDK consumers.
    #[serde(default)]
    pub gtfs_jp_profile: Option<String>,
}

fn default_true_metrics() -> bool {
    true
}
