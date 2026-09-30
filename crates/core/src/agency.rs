//! Agency atfı tipleri (MobilityData gtfs-validator #2201 karşılığı).
//!
//! Çözümleyici `gtfs_pipeline::agency_attribution::AgencyResolver`'dır; tipler burada
//! durur çünkü feed-level özet notice'ları altlarındaki ham notice'ların dağılımını
//! [`Notice::agency_distribution`](crate::Notice::agency_distribution) içinde taşır.

use std::collections::BTreeMap;

/// Agency kaydının resolver içindeki sırası. `agency_id`'si olmayan tek agency'nin
/// kimliği boş string'dir.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct AgencyRef(pub u32);

/// Kimliğin nasıl eşleştiği. Zincirdeki (trip → route → agency) HERHANGİ bir adım
/// kırpma yedeğiyle çözüldüyse sonuç yedektir.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Resolution {
    Exact,
    UniqueTrimFallback,
}

impl Resolution {
    pub fn and(self, other: Resolution) -> Resolution {
        if self == Resolution::Exact && other == Resolution::Exact {
            Resolution::Exact
        } else {
            Resolution::UniqueTrimFallback
        }
    }
}

/// Atıf beklenebilirken yapılamamasının nedeni.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum UnattributedReason {
    /// Kural scope beyan ediyor ama notice `scope_key` taşımıyor.
    MissingScopeKey,
    /// Scope beyanlı kuralın feed düzeyi özeti (`EntityType::Feed`, `scope_key` yok):
    /// feed-level toplulamalar ve emitter içi özetler (STM_017, STM_032/056 eşik üstü).
    /// Tek bir varlığa ait değildir; dağılım özetten ÖNCE, ham notice'larda korunmalıdır.
    FeedLevelSummary,
    /// `agency_id` feed'de yok (kırık FK dahil).
    UnknownAgency,
    /// `route_id` feed'de yok (kırık FK dahil).
    UnknownRoute,
    /// `trip_id` feed'de yok (kırık FK dahil).
    UnknownTrip,
    /// Birden fazla agency varken route'un `agency_id`'si boş.
    RouteWithoutAgency,
    /// `fare_id` feed'de (`fare_attributes.txt`) yok (kırık FK dahil).
    UnknownFare,
    /// Birden fazla agency varken tarifenin `agency_id`'si boş.
    FareWithoutAgency,
    /// Kırpılmış kimlik birden fazla ham kimliğe düşüyor.
    AmbiguousPaddedId,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum AgencyAttribution {
    /// Notice doğrudan bir agency kaydına ait.
    Direct { agency: AgencyRef, resolution: Resolution },
    /// Route/trip ilişkisinden tek agency bulundu.
    Resolved { agency: AgencyRef, resolution: Resolution },
    Unattributed(UnattributedReason),
    /// Kuralın scope türü henüz çözülmüyor (stop, shape, service… — P6).
    Unsupported,
    /// Kural scope beyan etmiyor: feed/dosya düzeyi bulgu, agency kavramıyla ilişkisi yok.
    NotApplicable,
}

impl AgencyAttribution {
    pub fn agency(&self) -> Option<AgencyRef> {
        match self {
            Self::Direct { agency, .. } | Self::Resolved { agency, .. } => Some(*agency),
            _ => None,
        }
    }
}

/// Bir özet notice'ın altındaki ham notice'ların atıf dağılımı: atıf → ham notice sayısı.
/// Toplamı özetin taşıdığı etkilenen kayıt sayısına eşittir.
pub type AgencyDistribution = BTreeMap<AgencyAttribution, u64>;

/// Bir kuralın sayımları. Üç birim birbirine karıştırılmaz:
/// - `finding_count`: rapora giren bulgu sayısı — boşluk türevleri bastırılmış, dedup
///   edilmiş, kural başına cap UYGULANMAMIŞ.
/// - `affected_entity_count`: bulguların temsil ettiği ham kayıt sayısı; toplulanmış bir
///   özet, altındaki bütün ham notice'lar kadar sayılır (özet başına 1 değil). Emitter
///   içinde özetlenmiş bulgular (üyeleri görülmez) 1 sayılır ve `FeedLevelSummary`
///   kovasına düşer.
/// - `displayed_sample_count`: sonuçta taşınan notice sayısı (WASM cap'i; native'de
///   `finding_count`'a eşit).
///
/// `by_attribution`, `affected_entity_count`'un atıf dağılımıdır; toplamı ona eşittir.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct RuleAgencyCounts {
    pub finding_count: u64,
    pub affected_entity_count: u64,
    pub displayed_sample_count: u64,
    pub by_attribution: AgencyDistribution,
}

/// Feed'in kural × agency dökümü (MobilityData gtfs-validator #2201). Raporlama katmanıdır:
/// severity ve skor bundan etkilenmez.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct AgencyBreakdown {
    /// `false`: WASM notice bütçesi aşıldı, ham notice'ların bir kısmı hiç işlenmedi;
    /// sayımlar alt sınırdır.
    pub complete: bool,
    /// [`AgencyRef`] sırasıyla agency kimlikleri (tek agency'de boş olabilir).
    pub agencies: Vec<String>,
    pub rules: BTreeMap<String, RuleAgencyCounts>,
}
