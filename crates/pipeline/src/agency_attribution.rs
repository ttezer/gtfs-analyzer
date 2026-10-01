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
//! - `fare_id`   → fare_attributes.agency_id → agency ⇒ [`AgencyAttribution::Resolved`]
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
//! Tek agency'li feed'de `routes.agency_id` ve `fare_attributes.agency_id` boş olabilir
//! (GTFS koşullu zorunluluk); bu durumda kayıt o tek agency'ye bağlanır. Birden fazla agency
//! varken boş `agency_id` bağlanmaz ([`UnattributedReason::RouteWithoutAgency`],
//! [`UnattributedReason::FareWithoutAgency`]).
//!
//! Yinelenen ID'lerde İLK kayıt kazanır (dosya sırası); kopya kimlik ayrı kurallarla
//! raporlanır.

use rustc_hash::FxHashMap;

use gtfs_core::Notice;

use crate::k2::EntityRecords;

pub use gtfs_core::agency::{
    AgencyAttribution, AgencyBreakdown, AgencyDistribution, AgencyRef, Resolution,
    RuleAgencyCounts, UnattributedReason,
};

/// Ham kimlik → değer; kırpılmış kimlik → değer ya da belirsizlik.
///
/// Kırpılmış harita TEMBELDİR: yalnız birebir eşleşme ilk kez başarısız olduğunda,
/// birebir haritadan türetilir (korpusta yedek 830 feed'in ikisinde gerekti). 1,43 milyon
/// seferli feed'de iki harita ~140 MB tutuyordu; çoğu feed artık yalnız birini öder.
struct IdIndex<'a, V> {
    exact: FxHashMap<&'a str, V>,
    /// `None`: kırpılmış karşılık birden fazla ham kimliğe düşüyor.
    trimmed: std::cell::OnceCell<FxHashMap<&'a str, Option<V>>>,
}

enum Miss {
    Unknown,
    Ambiguous,
}

impl<'a, V: Copy> IdIndex<'a, V> {
    fn build(entries: impl Iterator<Item = (&'a str, V)>) -> Self {
        let mut exact = FxHashMap::default();
        for (id, value) in entries {
            exact.entry(id).or_insert(value); // yinelenen kimlik: ilk kayıt kazanır
        }
        Self { exact, trimmed: std::cell::OnceCell::new() }
    }

    fn trimmed(&self) -> &FxHashMap<&'a str, Option<V>> {
        self.trimmed.get_or_init(|| {
            let mut trimmed: FxHashMap<&'a str, Option<V>> = FxHashMap::default();
            for (id, value) in &self.exact {
                trimmed
                    .entry(id.trim())
                    .and_modify(|slot| *slot = None)
                    .or_insert(Some(*value));
            }
            trimmed
        })
    }

    fn get(&self, key: &str) -> Result<(V, Resolution), Miss> {
        if let Some(v) = self.exact.get(key) {
            return Ok((*v, Resolution::Exact));
        }
        match self.trimmed().get(key.trim()) {
            Some(Some(v)) => Ok((*v, Resolution::UniqueTrimFallback)),
            Some(None) => Err(Miss::Ambiguous),
            None => Err(Miss::Unknown),
        }
    }
}

type RouteAgency = Result<(AgencyRef, Resolution), UnattributedReason>;

pub struct AgencyResolver<'a> {
    records: &'a EntityRecords,
    agency_ids: Vec<&'a str>,
    agencies: IdIndex<'a, AgencyRef>,
    routes: IdIndex<'a, RouteAgency>,
    fares: IdIndex<'a, RouteAgency>,
    /// trip_id → route_id (ham). TEMBEL: trip scope'lu bir notice sorulana kadar kurulmaz.
    /// Büyük feed'de (VBB 282k sefer) iki hash haritası demektir; WASM belleği için
    /// toplulanan kuralların çoğu trip scope'lu olmadığında hiç ödenmez.
    trips: std::cell::OnceCell<IdIndex<'a, &'a str>>,
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

        // Kaydın `agency_id`'si → agency; boşsa tek agency, o da yoksa `without`.
        let owner = |agency_id: Option<&str>, without: UnattributedReason| -> RouteAgency {
            match agency_id.filter(|id| !id.trim().is_empty()) {
                Some(id) => match agencies.get(id) {
                    Ok(hit) => Ok(hit),
                    Err(Miss::Unknown) => Err(UnattributedReason::UnknownAgency),
                    Err(Miss::Ambiguous) => Err(UnattributedReason::AmbiguousPaddedId),
                },
                None => single.map(|agency| (agency, Resolution::Exact)).ok_or(without),
            }
        };
        let routes = IdIndex::build(records.routes.iter().map(|route| {
            let agency = owner(route.agency_id.as_deref(), UnattributedReason::RouteWithoutAgency);
            (route.route_id.as_str(), agency)
        }));
        let fares = IdIndex::build(records.fare_attributes.iter().map(|fare| {
            let agency = owner(fare.agency_id.as_deref(), UnattributedReason::FareWithoutAgency);
            (fare.fare_id.as_str(), agency)
        }));

        Self { records, agency_ids, agencies, routes, fares, trips: std::cell::OnceCell::new() }
    }

    fn trips(&self) -> &IdIndex<'a, &'a str> {
        self.trips.get_or_init(|| {
            let interns = &self.records.trip_interns;
            IdIndex::build(
                self.records
                    .trips
                    .iter()
                    .map(|trip| (trip.trip_id.as_str(), interns.route_id(trip))),
            )
        })
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
        self.attribute_scoped(notice, scope, notice.scope_key.as_deref())
    }

    /// Feed-level toplulamanın ham grup üyesi için. Toplulanan kuralların registry scope'u
    /// ÖZETİN kimliğini anlatır (feed düzeyi, çoğunlukla `None`), üyenin değil: ham TRP_005
    /// sefer başınadır ama kural scope beyan etmez. Scope beyanı yoksa üyenin kendi varlığı
    /// (`Agency`/`Route`/`Trip` + `entity_id`) kullanılır.
    pub fn attribute_member(&self, notice: &Notice) -> AgencyAttribution {
        match self.attribute(notice) {
            AgencyAttribution::NotApplicable => {
                let scope = match notice.entity_type {
                    gtfs_core::EntityType::Agency => "agency_id",
                    gtfs_core::EntityType::Route => "route_id",
                    gtfs_core::EntityType::Trip => "trip_id",
                    gtfs_core::EntityType::Fare => "fare_id",
                    gtfs_core::EntityType::Stop
                    | gtfs_core::EntityType::Shape
                    | gtfs_core::EntityType::Service => return AgencyAttribution::Unsupported,
                    _ => return AgencyAttribution::NotApplicable,
                };
                self.attribute_scoped(notice, scope, notice.entity_id.as_deref())
            }
            other => other,
        }
    }

    fn attribute_scoped(&self, notice: &Notice, scope: &str, key: Option<&str>) -> AgencyAttribution {
        if !matches!(scope, "agency_id" | "route_id" | "trip_id" | "fare_id") {
            return AgencyAttribution::Unsupported;
        }
        let Some(key) = key.filter(|k| !k.is_empty()) else {
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
            "fare_id" => match self.fares.get(key) {
                Ok((owner, hop)) => owner.map(|(agency, agency_hop)| (agency, hop.and(agency_hop))),
                Err(miss) => return unattributed(miss, UnattributedReason::UnknownFare),
            },
            _ => match self.trips().get(key) {
                Ok((route_id, resolution)) => self.route(route_id, resolution),
                Err(miss) => return unattributed(miss, UnattributedReason::UnknownTrip),
            },
        };
        match resolved {
            Ok((agency, resolution)) => AgencyAttribution::Resolved { agency, resolution },
            Err(reason) => AgencyAttribution::Unattributed(reason),
        }
    }

    /// Agency başına sefer sayısı (route üzerinden; çözülemeyen seferler sayılmaz).
    pub fn trip_counts(&self) -> Vec<u64> {
        let mut counts = vec![0u64; self.agency_ids.len()];
        let interns = &self.records.trip_interns;
        for trip in &self.records.trips {
            if let Ok((agency, _)) = self.route(interns.route_id(trip), Resolution::Exact) {
                counts[agency.0 as usize] += 1;
            }
        }
        counts
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

/// Kural × agency dökümünü notice notice biriktirir. Toplulanmış özetler altlarındaki
/// dağılımla, diğerleri [`AgencyResolver::attribute_member`] ile sayılır: toplulamayla aynı
/// atıf kuralı. Beslenen notice'lar K7 hazırlığından çıkmış olmalıdır (boşluk türevleri
/// bastırılmış, dedup edilmiş, cap uygulanmamış).
pub struct AgencyCounter<'a> {
    resolver: AgencyResolver<'a>,
    rules: std::collections::BTreeMap<String, RuleAgencyCounts>,
}

impl<'a> AgencyCounter<'a> {
    pub fn new(records: &'a EntityRecords) -> Self {
        Self { resolver: AgencyResolver::new(records), rules: Default::default() }
    }

    pub fn add(&mut self, notice: &Notice) {
        let counts = self.rules.entry(notice.rule_id.clone()).or_default();
        counts.finding_count += 1;
        counts.displayed_sample_count += 1;
        match notice.agency_distribution.as_deref() {
            Some(distribution) => {
                for (attribution, n) in distribution {
                    *counts.by_attribution.entry(*attribution).or_default() += n;
                    counts.affected_entity_count += n;
                }
            }
            None => {
                let attribution = self.resolver.attribute_member(notice);
                *counts.by_attribution.entry(attribution).or_default() += 1;
                counts.affected_entity_count += 1;
            }
        }
    }

    /// `displayed_sample_count` burada `finding_count`'tur; WASM cap'ten sonra
    /// [`set_displayed`] ile düzeltir.
    pub fn finish(self, complete: bool) -> AgencyBreakdown {
        AgencyBreakdown {
            complete,
            agencies: self.resolver.agency_ids.iter().map(|id| id.to_string()).collect(),
            agency_names: self.resolver.records.agencies.iter().map(|a| a.agency_name.clone()).collect(),
            agency_trip_counts: self.resolver.trip_counts(),
            agency_notice_indices: Vec::new(),
            rules: self.rules,
        }
    }
}

/// Hazır bir notice listesinin dökümü (native: cap yok).
pub fn breakdown(notices: &[Notice], records: &EntityRecords, complete: bool) -> AgencyBreakdown {
    let mut counter = AgencyCounter::new(records);
    for notice in notices {
        counter.add(notice);
    }
    counter.finish(complete)
}

/// Cap'ten sonra sonuçta kalan notice'lara göre `displayed_sample_count`'u yeniler.
pub fn set_displayed(breakdown: &mut AgencyBreakdown, shown: &[Notice]) {
    for counts in breakdown.rules.values_mut() {
        counts.displayed_sample_count = 0;
    }
    for notice in shown {
        if let Some(counts) = breakdown.rules.get_mut(&notice.rule_id) {
            counts.displayed_sample_count += 1;
        }
    }
}

/// Sonuçtaki nihai `notices` dizisi için agency → notice indeksleri. Atıf kuralı sayımla
/// aynıdır ([`AgencyResolver::attribute_member`], özetlerde dağılım). Rapor kurulduktan
/// SONRA çağrılır: indeksler serileştirilen dizinin sırasına bağlıdır.
pub fn index_notices(breakdown: &mut AgencyBreakdown, notices: &[Notice], records: &EntityRecords) {
    let resolver = AgencyResolver::new(records);
    let mut index = vec![Vec::new(); breakdown.agencies.len()];
    for (i, notice) in notices.iter().enumerate() {
        let i = i as u32;
        match notice.agency_distribution.as_deref() {
            Some(distribution) => {
                let mut agencies: Vec<u32> =
                    distribution.keys().filter_map(|a| a.agency()).map(|a| a.0).collect();
                // Anahtarlar önce varyanta göre sıralı: aynı agency Direct ve Resolved'da
                // ayrı yerlerde olabilir.
                agencies.sort_unstable();
                agencies.dedup();
                for agency in agencies {
                    index[agency as usize].push(i);
                }
            }
            None => {
                if let Some(agency) = resolver.attribute_member(notice).agency() {
                    index[agency.0 as usize].push(i);
                }
            }
        }
    }
    breakdown.agency_notice_indices = index;
}
