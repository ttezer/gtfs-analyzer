use serde::{Deserialize, Serialize};

/// Tek bir GTFS dosyasına ait boyut ve satır bilgisi.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileInfo {
    pub name: String,
    pub rows: u32,
    pub bytes: u32,
}

/// Feed düzeyinde özet metrikler.
///
/// MET_001: entity sayıları
/// MET_002: servis kapsama yoğunluğu
/// MET_003: sınıf bazında hata sayısı
/// MET_004: genel kalite skoru (R5 ile eşdeğer)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FeedMetrics {
    // MET_001 — Toplam durak / hat / sefer / şekil sayısı
    pub stop_count: u32,
    pub route_count: u32,
    pub trip_count: u32,
    pub shape_count: u32,

    // MET_002 — Servis kapsama yoğunluğu
    pub active_service_days: u32,
    pub avg_daily_trips: f64,

    // MET_003 — Sınıf bazında hata sayısı
    pub spec_notice_count: u32,
    pub interop_notice_count: u32,
    pub quality_notice_count: u32,
    pub analytics_notice_count: u32,

    // MET_004 — Genel kalite skoru 0.0–100.0
    pub quality_score: f64,

    // Dosya bazında istatistikler (K1 parse)
    pub file_stats: Vec<FileInfo>,
}
