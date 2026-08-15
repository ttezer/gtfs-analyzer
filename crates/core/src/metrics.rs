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
/// MET_004: genel skor (R5 ile eşdeğer; Spec×40%+Interop×30%+Quality×20%+Analytics×10%)
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

    // Feed geçerlilik penceresi (feed_info.txt) — YYYYMMDD, yoksa None
    pub feed_start_date: Option<u32>,
    pub feed_end_date: Option<u32>,

    // Gerçek servis kapsama aralığı (calendar + calendar_dates) — YYYYMMDD, servis yoksa None
    pub service_start_date: Option<u32>,
    pub service_end_date: Option<u32>,

    // MET_003 — Sınıf bazında hata sayısı
    pub spec_notice_count: u32,
    pub interop_notice_count: u32,
    pub quality_notice_count: u32,
    pub analytics_notice_count: u32,

    // MET_004 — Genel skor 0.0–100.0 (R5 ağırlıklı: 0.4·spec + 0.3·interop + 0.2·quality + 0.1·analytics).
    // NOT: R5Report.quality_score (yalnız Quality sınıfı bileşeni) ile karıştırılmamalı.
    pub overall_score: f64,

    /// Skor TAM KAPSAMLI bir koşumdan mı geldi (issue #133).
    ///
    /// 🔴 **`false` ise `overall_score` BAŞKA FEED'LERLE KIYASLANAMAZ.** Skor yalnız
    /// ÜRETİLMİŞ notice'lardan hesaplanır; bir dosya okunamayınca ona bağlı kurallar
    /// koşmaz, ceza üretemez ve skor YÜKSELİR. Ölçüldü (`mdb-1903`): `stops.txt`
    /// okunamaz olunca 604 notice → 178 ve genel skor **89,4 → 93,5**. Korpusta canlı
    /// örnek `mdb-3135`: 2 notice, 62 atlanan kontrol, skor **94,5**.
    ///
    /// Skorun KENDİSİ bilerek düzeltilmiyor: atlanan kontrol başına ceza uydurmak
    /// gerçek bir ölçüme dayanmayan bir sayı üretirdi. Doğru çözüm sayıyı oynatmak
    /// değil, KIYASLANAMAZ olduğunu taşımaktır.
    #[serde(default = "default_true_metrics")]
    pub coverage_complete: bool,

    // Dosya bazında istatistikler (K1 parse)
    pub file_stats: Vec<FileInfo>,

    /// Feed GTFS-JP (Japonya profili) mi — herhangi bir `*_jp.txt` dosyası varsa true.
    /// UI'da "GTFS-JP" rozeti + JPN grubu kurallarının koşullu tetiklenmesi için.
    #[serde(default)]
    pub is_gtfs_jp: bool,
}

fn default_true_metrics() -> bool { true }
