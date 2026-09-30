//! Notice → agency atfı (MobilityData gtfs-validator #2201 karşılığı, P1).
//!
//! Bir raporlama katmanıdır: severity, skor ve notice içeriği DEĞİŞMEZ. Emitter'lar agency
//! hesaplamaz; atıf yalnız iki kaynaktan yapılır: kuralın registry'de beyan ettiği
//! `scope_key_field` ve notice'ın runtime `scope_key` değeri.
//!
//! Çözüm sırası (`scope_key_field`'a göre):
//! - `agency_id` → agency                         ⇒ [`AgencyAttribution::Direct`]
//! - `route_id`  → route.agency_id → agency       ⇒ [`AgencyAttribution::Resolved`]
//! - `trip_id`   → trip.route_id → route → agency ⇒ [`AgencyAttribution::Resolved`]
//! - başka bir scope (stop, shape, service…)      ⇒ [`AgencyAttribution::Unsupported`]
//! - scope beyanı yok (feed/dosya düzeyi)         ⇒ [`AgencyAttribution::NotApplicable`]
//!
//! Kimlik eşleşmesi HAM ID ile yapılır (`"R1"` ile `" R1 "` farklı kimliklerdir). K2 bazı
//! yollarda `scope_key`'i kırpılmış üretir; bu yüzden birebir eşleşme yoksa ve kırpılmış
//! karşılık TEK bir ham ID'ye düşüyorsa ona bağlanır ve [`Resolution::UniqueTrimFallback`]
//! olarak işaretlenir. Birden fazla ham ID'ye düşüyorsa bağlanmaz
//! ([`UnattributedReason::AmbiguousPaddedId`]). 2026-09-30 korpus denetimi: 830 feed'de
//! yedek 463 notice'ta gerekti, belirsizlik hiç çıkmadı.
//!
//! Tek agency'li feed'de `routes.agency_id` boş olabilir (GTFS koşullu zorunluluk); bu
//! durumda route o tek agency'ye bağlanır. Birden fazla agency varken boş `agency_id`
//! bağlanmaz ([`UnattributedReason::RouteWithoutAgency`]).
//!
//! Yinelenen ID'lerde İLK kayıt kazanır (dosya sırası); kopya kimlik ayrı kurallarla
//! raporlanır.

use std::collections::HashMap;

use gtfs_core::Notice;

use crate::k2::EntityRecords;

/// Agency kaydının resolver içindeki sırası. `agency_id`'si olmayan tek agency'nin
/// kimliği boş string'dir.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct AgencyRef(u32);

/// Kimliğin nasıl eşleştiği. Zincirdeki (trip → route → agency) HERHANGİ bir adım
/// kırpma yedeğiyle çözüldüyse sonuç yedektir.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Resolution {
    Exact,
    UniqueTrimFallback,
}

impl Resolution {
    fn and(self, other: Resolution) -> Resolution {
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
    /// Kırpılmış kimlik birden fazla ham kimliğe düşüyor.
    AmbiguousPaddedId,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
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

/// Ham kimlik → değer; kırpılmış kimlik → değer ya da belirsizlik.
struct IdIndex<'a, V> {
    exact: HashMap<&'a str, V>,
    /// `None`: kırpılmış karşılık birden fazla ham kimliğe düşüyor.
    trimmed: HashMap<&'a str, Option<V>>,
}

enum Miss {
    Unknown,
    Ambiguous,
}

impl<'a, V: Copy> IdIndex<'a, V> {
    fn build(entries: impl Iterator<Item = (&'a str, V)>) -> Self {
        let mut exact = HashMap::new();
        let mut trimmed: HashMap<&'a str, Option<V>> = HashMap::new();
        for (id, value) in entries {
            if exact.contains_key(id) {
                continue; // yinelenen kimlik: ilk kayıt kazanır
            }
            exact.insert(id, value);
            trimmed
                .entry(id.trim())
                .and_modify(|slot| *slot = None)
                .or_insert(Some(value));
        }
        Self { exact, trimmed }
    }

    fn get(&self, key: &str) -> Result<(V, Resolution), Miss> {
        if let Some(v) = self.exact.get(key) {
            return Ok((*v, Resolution::Exact));
        }
        match self.trimmed.get(key.trim()) {
            Some(Some(v)) => Ok((*v, Resolution::UniqueTrimFallback)),
            Some(None) => Err(Miss::Ambiguous),
            None => Err(Miss::Unknown),
        }
    }
}

type RouteAgency = Result<(AgencyRef, Resolution), UnattributedReason>;

pub struct AgencyResolver<'a> {
    agency_ids: Vec<&'a str>,
    agencies: IdIndex<'a, AgencyRef>,
    routes: IdIndex<'a, RouteAgency>,
    /// trip_id → route_id (ham).
    trips: IdIndex<'a, &'a str>,
}

impl<'a> AgencyResolver<'a> {
    pub fn new(records: &'a EntityRecords) -> Self {
        let agency_ids: Vec<&str> = records
            .agencies
            .iter()
            .map(|a| a.agency_id.as_deref().unwrap_or(""))
            .collect();
        let agencies = IdIndex::build(
            agency_ids.iter().enumerate().map(|(i, id)| (*id, AgencyRef(i as u32))),
        );
        let single = (agency_ids.len() == 1).then_some(AgencyRef(0));

        let routes = IdIndex::build(records.routes.iter().map(|route| {
            let owner = match route.agency_id.as_deref().filter(|id| !id.trim().is_empty()) {
                Some(id) => match agencies.get(id) {
                    Ok(hit) => Ok(hit),
                    Err(Miss::Unknown) => Err(UnattributedReason::UnknownAgency),
                    Err(Miss::Ambiguous) => Err(UnattributedReason::AmbiguousPaddedId),
                },
                None => single
                    .map(|agency| (agency, Resolution::Exact))
                    .ok_or(UnattributedReason::RouteWithoutAgency),
            };
            (route.route_id.as_str(), owner)
        }));

        let interns = &records.trip_interns;
        let trips = IdIndex::build(
            records
                .trips
                .iter()
                .map(|trip| (trip.trip_id.as_str(), interns.route_id(trip))),
        );

        Self { agency_ids, agencies, routes, trips }
    }

    /// Resolver'ın atfettiği agency'nin ham kimliği (tek agency'de boş olabilir).
    pub fn agency_id(&self, agency: AgencyRef) -> &'a str {
        self.agency_ids[agency.0 as usize]
    }

    pub fn attribute(&self, notice: &Notice) -> AgencyAttribution {
        let scope = gtfs_rules::get_rule(&notice.rule_id).and_then(|meta| meta.scope_key_field);
        let Some(scope) = scope else {
            return AgencyAttribution::NotApplicable;
        };
        if !matches!(scope, "agency_id" | "route_id" | "trip_id") {
            return AgencyAttribution::Unsupported;
        }
        let Some(key) = notice.scope_key.as_deref().filter(|k| !k.is_empty()) else {
            return AgencyAttribution::Unattributed(
                if notice.entity_type == gtfs_core::EntityType::Feed {
                    UnattributedReason::FeedLevelSummary
                } else {
                    UnattributedReason::MissingScopeKey
                },
            );
        };
        let resolved = match scope {
            "agency_id" => {
                return match self.agencies.get(key) {
                    Ok((agency, resolution)) => AgencyAttribution::Direct { agency, resolution },
                    Err(miss) => unattributed(miss, UnattributedReason::UnknownAgency),
                };
            }
            "route_id" => self.route(key, Resolution::Exact),
            _ => match self.trips.get(key) {
                Ok((route_id, resolution)) => self.route(route_id, resolution),
                Err(miss) => return unattributed(miss, UnattributedReason::UnknownTrip),
            },
        };
        match resolved {
            Ok((agency, resolution)) => AgencyAttribution::Resolved { agency, resolution },
            Err(reason) => AgencyAttribution::Unattributed(reason),
        }
    }

    fn route(&self, route_id: &str, via: Resolution) -> RouteAgency {
        match self.routes.get(route_id) {
            Ok((owner, route_hop)) => {
                owner.map(|(agency, agency_hop)| (agency, via.and(route_hop).and(agency_hop)))
            }
            Err(Miss::Unknown) => Err(UnattributedReason::UnknownRoute),
            Err(Miss::Ambiguous) => Err(UnattributedReason::AmbiguousPaddedId),
        }
    }
}

fn unattributed(miss: Miss, unknown: UnattributedReason) -> AgencyAttribution {
    AgencyAttribution::Unattributed(match miss {
        Miss::Unknown => unknown,
        Miss::Ambiguous => UnattributedReason::AmbiguousPaddedId,
    })
}
