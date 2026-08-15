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

/// Bir kuralın **otorite kaynağı** — sınıfın (`RuleClass`) meşruiyet dayanağı.
///
/// Sınıf otorite bütünlüğü (otorite denetim defterinden; yerelde notgit/audits/):
/// `Spec` sınıfı YALNIZCA `GtfsSpec` kaynağıyla meşrudur. Diğer kaynaklar Spec üretemez.
/// Eşleme: GtfsSpec→Spec | GtfsBestPractice→Quality | MobilitydataParity/
/// GoogleTransitInterop/RegionalProfile→Interop | ProjectQuality→Quality |
/// ProjectAnalytics→Analytics | Unknown→asla Spec.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum AuthoritySource {
    /// Resmi GTFS Schedule Reference'ta açık normatif hüküm (required/enum/FK/uniqueness/format).
    #[serde(rename = "GTFS_SPEC")]
    GtfsSpec,
    /// Resmi GTFS Best Practices dokümanı — normatif spec DEĞİL.
    #[serde(rename = "GTFS_BEST_PRACTICE")]
    GtfsBestPractice,
    /// MobilityData validator paritesi (yalnız interop sinyali).
    #[serde(rename = "MOBILITYDATA_PARITY")]
    MobilitydataParity,
    /// Google Transit tüketici davranışı.
    #[serde(rename = "GOOGLE_TRANSIT_INTEROP")]
    GoogleTransitInterop,
    /// Bölgesel profil (ör. GTFS-JP) — resmi Schedule Reference değil.
    #[serde(rename = "REGIONAL_PROFILE")]
    RegionalProfile,
    /// Proje-özel veri kalitesi / okunabilirlik kontrolü.
    #[serde(rename = "PROJECT_QUALITY")]
    ProjectQuality,
    /// Proje-özel istatistiksel / operasyonel sinyal.
    #[serde(rename = "PROJECT_ANALYTICS")]
    ProjectAnalytics,
    /// Otorite henüz belirlenmedi — asla Spec olamaz.
    #[serde(rename = "UNKNOWN")]
    Unknown,
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

/// Doğrulamayı tümüyle durduran hata kodu.
///
/// ⚠️ **HER VARYANT ÜRETİLMİYOR.** Bu yorumlar bir zamanlar üç varyantı ARC_002/ARC_004/
/// ARC_013'e "fatal karşılığı" diye eşliyordu; `66e009c2` (2026-08-09) o üç yolu BİLİNÇLİ
/// olarak kaldırıp kısmi kurtarmaya (`ValidationStatus::Partial`) çevirdi ama yorumlar
/// bayat kaldı. Yorumlara güvenen `spec-audit/silent_rules_scan.py` üç kuralı yanlışlıkla
/// "fatal yolu" diye etiketledi ve `fatals.csv`'den bayat bir "ateşledi" sonucu üretti
/// (issue #132). Aşağıdaki etiketler ölçülmüştür — değiştirirken ÖLÇ.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum FatalCode {
    /// ✅ CANLI — `k1_parse`: ZIP açılamadı (ARC_001).
    ZipUnreadable,
    /// ⛔ HİÇBİR RUST KODU ÜRETMEZ. Zorunlu dosyadaki UTF-8 ihlali artık `ARC_002`
    /// NOTICE'ı + Partial üretir; feed reddedilmez.
    Utf8Critical,
    /// ⛔ HİÇBİR RUST KODU ÜRETMEZ. Zorunlu dosya eksikliği artık `ARC_004` NOTICE'ı +
    /// Partial üretir (`66e009c2`).
    NoRequiredFiles,
    /// ⛔ HİÇBİR RUST KODU ÜRETMEZ. Tokenization hatası artık `ARC_013` NOTICE'ı üretir ve
    /// dosya `partial.unavailable_files`'a düşer.
    CsvMalformed,
    /// ✅ CANLI — `k1_parse`: zip-bomb / açılmış-boyut sınırı aşıldı (ARC_029).
    DecompressionLimit,
    /// ✅ CANLI ama YALNIZ TypeScript tarafında: `ui/src/validator-client.ts` worker
    /// çökmesi ve zaman aşımı için üretir. Rust bu varyantı hiç kurmaz.
    ResourceLimit,
    /// ✅ CANLI — `wasm`: giriş verisi geçersiz (örn. config JSON parse hatası).
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
    /// KRİTİK + SPEC — resmi GTFS spec yayınlanabilirliğini kesin engelleyen hata
    #[serde(rename = "blocker")]
    Blocker,
    /// INTEROP sınıfı — tüketici/interop uyumluluk sinyali (yayın-engeli değil)
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
    /// affected_instance_count > 20
    #[serde(rename = "widespread")]
    Widespread,
    /// Analytics sınıfı kural
    #[serde(rename = "analytics")]
    Analytics,
    /// fix_effort ≥ 4.0 — kapsamlı veri revizyonu gerektirir.
    /// base_effort 1-2-3 skalasına normalize edildikten sonra eşik bilinçli olarak 4.0'da
    /// tutuldu: fix_effort = base_effort × instance_multiplier (maks 2×), yani üst sınır 6.0.
    /// 4.0 eşiği → base_effort=3 + yaygın (≥1.5×) ya da base_effort=2 + çok yaygın (2×).
    /// Eşik 3.0'a çekilseydi tek başına yapısal kurallar (base_effort=3) Hard sayılır,
    /// etiket şişerdi.
    #[serde(rename = "hard")]
    Hard,
    /// affected_instance_count == 1 — tek satırda hata, kolay bulunur
    #[serde(rename = "single")]
    Single,
    /// score_delta veya pub_score_delta yüksek — düzeltme skora büyük katkı sağlar
    #[serde(rename = "high-impact")]
    HighImpact,
}
