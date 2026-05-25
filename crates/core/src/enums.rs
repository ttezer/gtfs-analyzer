use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub enum Severity {
    #[serde(rename = "CRITICAL")]
    Kritik,
    #[serde(rename = "HIGH")]
    Yuksek,
    #[serde(rename = "MEDIUM")]
    Orta,
    #[serde(rename = "LOW")]
    Dusuk,
    #[serde(rename = "INFO")]
    Bilgi,
}

impl Severity {
    /// R9 skor formülünde kullanılan severity_weight.
    pub fn weight(self) -> f64 {
        match self {
            Self::Kritik => 4.0,
            Self::Yuksek => 3.0,
            Self::Orta   => 2.0,
            Self::Dusuk  => 1.0,
            Self::Bilgi  => 0.0,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum RuleClass {
    #[serde(rename = "SPEC")]
    Spec,
    #[serde(rename = "INTEROP")]
    Interop,
    #[serde(rename = "QUALITY")]
    Quality,
    #[serde(rename = "ANALYTICS")]
    Analytics,
}

/// Bir notice'ın hangi GTFS entity granülaritesinde üretildiğini belirtir.
///
/// Strateji (architecture Bölüm 3):
/// - `frequencies`, `calendar_dates`, `fare_rules` → `Row`
/// - `feed_info` → `Feed`
/// - diğer entity'ler → ilgili varyant
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum EntityType {
    Feed,
    File,
    Agency,
    Stop,
    Route,
    Trip,
    Shape,
    Service,
    Fare,
    Transfer,
    Pathway,
    Level,
    Translation,
    Attribution,
    Row,
}

impl EntityType {
    /// Dedup key gibi sabit bağlamlarda kullanılacak stabil string temsili.
    /// Debug formatının aksine enum variant yeniden adlandırılsa bile değişmez.
    pub fn stable_name(self) -> &'static str {
        match self {
            Self::Feed        => "feed",
            Self::File        => "file",
            Self::Agency      => "agency",
            Self::Stop        => "stop",
            Self::Route       => "route",
            Self::Trip        => "trip",
            Self::Shape       => "shape",
            Self::Service     => "service",
            Self::Fare        => "fare",
            Self::Transfer    => "transfer",
            Self::Pathway     => "pathway",
            Self::Level       => "level",
            Self::Translation => "translation",
            Self::Attribution => "attribution",
            Self::Row         => "row",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum FatalCode {
    /// ARC_001 — ZIP açılamadı
    ZipUnreadable,
    /// ARC_002 — UTF-8 ihlali, dosya okunamıyor
    Utf8Critical,
    /// ARC_004 — zorunlu dosyalar eksik
    NoRequiredFiles,
    /// ARC_013 — CSV tokenization başarısız
    CsvMalformed,
    /// WASM bellek veya notice sayısı limiti aşıldı (browser ortamı)
    ResourceLimit,
    /// Giriş verisi geçersiz (örn. config JSON parse hatası)
    InvalidInput,
}

/// Notice'ın hangi granülaritede deduplikasyon yapıldığını belirtir.
///
/// Dedup anahtarı (architecture Bölüm K1):
/// - `Feed`   → rule_id
/// - `Entity` → rule_id + entity_type + entity_id
/// - `Row`    → rule_id + file + line
/// - `Field`  → rule_id + file + line + field
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum DedupLevel {
    /// rule_id → feed başına tek notice
    Feed,
    /// rule_id + file → dosya başına tek notice (ARC dosya seviyesi kontroller)
    File,
    /// rule_id + entity_type + entity_id → entity başına tek notice
    Entity,
    /// rule_id + file + line → satır başına tek notice
    Row,
    /// rule_id + file + line + field → alan başına tek notice
    Field,
}

/// Veri raporu kimliği. R1–R5, R7–R9.
/// R6 rapor değil (PDF/HTML export aksiyonu); bu enum'da yer almaz.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum ReportId {
    R1, R2, R3, R4, R5, R7, R8, R9,
}

/// R9 remediation queue etiketleri — koşullar architecture Bölüm 5'te tanımlı.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum R9Label {
    /// KRİTİK + (SPEC veya INTEROP) — feed yayınlanabilirliğini kesin engelleyen hata
    #[serde(rename = "blocker")]
    Blocker,
    /// YÜKSEK + INTEROP — journey planner degraded davranış riski
    #[serde(rename = "conditional_blocker")]
    ConditionalBlocker,
    /// INTEROP sınıfı, blocker/conditional_blocker dışı
    #[serde(rename = "interop")]
    Interop,
    /// realized_dependent_count > 2
    #[serde(rename = "propagation")]
    Propagation,
    /// fix_effort == 1 AND severity >= YÜKSEK
    #[serde(rename = "quick-win")]
    QuickWin,
    /// QUALITY sınıfı + YÜKSEK severity
    #[serde(rename = "quality")]
    Quality,
    /// affected_instance_count > 50
    #[serde(rename = "widespread")]
    Widespread,
}
