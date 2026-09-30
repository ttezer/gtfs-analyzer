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

// ── JSON sözleşmesi ─────────────────────────────────────────────────────────
//
// {
//   "complete": true,
//   "agencies": ["A", "B"],
//   "rules": {
//     "TRP_005": {
//       "finding_count": 1, "affected_entity_count": 2, "displayed_sample_count": 1,
//       "agency_sets": [
//         { "agency_ids": ["A"], "affected_entity_count": 1, "trim_fallback_count": 0 }, …
//       ],
//       "unattributed": { "UnknownRoute": 3 }, "unsupported": 0, "not_applicable": 0
//     }
//   }
// }
//
// `agency_sets` yapısaldır (agency_id `|` içerebilir); bugün her küme tek agency'lidir,
// paylaşılan varlıklar (stop/shape/service) gelince birden fazla kimlik taşıyabilir.
// Sıralar deterministiktir: kurallar ve nedenler BTreeMap, kümeler agency sırasıyla.

#[derive(serde::Serialize)]
struct AgencySetJson<'a> {
    agency_ids: [&'a str; 1],
    affected_entity_count: u64,
    trim_fallback_count: u64,
}

#[derive(serde::Serialize)]
struct RuleJson<'a> {
    finding_count: u64,
    affected_entity_count: u64,
    displayed_sample_count: u64,
    agency_sets: Vec<AgencySetJson<'a>>,
    unattributed: BTreeMap<String, u64>,
    unsupported: u64,
    not_applicable: u64,
}

impl serde::Serialize for AgencyBreakdown {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        use serde::ser::SerializeStruct;
        let rules: BTreeMap<&str, RuleJson<'_>> = self
            .rules
            .iter()
            .map(|(rule, counts)| {
                let mut sets: BTreeMap<AgencyRef, (u64, u64)> = BTreeMap::new();
                let mut unattributed = BTreeMap::new();
                let (mut unsupported, mut not_applicable) = (0, 0);
                for (attribution, n) in &counts.by_attribution {
                    match attribution {
                        AgencyAttribution::Direct { agency, resolution }
                        | AgencyAttribution::Resolved { agency, resolution } => {
                            let slot = sets.entry(*agency).or_default();
                            slot.0 += n;
                            if *resolution == Resolution::UniqueTrimFallback {
                                slot.1 += n;
                            }
                        }
                        AgencyAttribution::Unattributed(reason) => {
                            *unattributed.entry(format!("{reason:?}")).or_default() += n;
                        }
                        AgencyAttribution::Unsupported => unsupported += n,
                        AgencyAttribution::NotApplicable => not_applicable += n,
                    }
                }
                let agency_sets = sets
                    .into_iter()
                    .map(|(agency, (affected, trimmed))| AgencySetJson {
                        agency_ids: [self
                            .agencies
                            .get(agency.0 as usize)
                            .map(String::as_str)
                            .unwrap_or("")],
                        affected_entity_count: affected,
                        trim_fallback_count: trimmed,
                    })
                    .collect();
                let json = RuleJson {
                    finding_count: counts.finding_count,
                    affected_entity_count: counts.affected_entity_count,
                    displayed_sample_count: counts.displayed_sample_count,
                    agency_sets,
                    unattributed,
                    unsupported,
                    not_applicable,
                };
                (rule.as_str(), json)
            })
            .collect();
        let mut state = serializer.serialize_struct("AgencyBreakdown", 3)?;
        state.serialize_field("complete", &self.complete)?;
        state.serialize_field("agencies", &self.agencies)?;
        state.serialize_field("rules", &rules)?;
        state.end()
    }
}
