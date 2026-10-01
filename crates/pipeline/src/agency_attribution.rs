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
//! - `stop_id` / `shape_id` / `service_id` → onu kullanan seferlerin agency'leri: tek agency
//!   ⇒ `Resolved`, birden fazla ⇒ [`AgencyAttribution::Shared`] (istasyon ve girişler alt
//!   duraklarının agency'lerini devralır)
//! - başka bir scope (pathway, transfer çifti…)   ⇒ [`AgencyAttribution::Unsupported`]
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
    RuleAgencyCounts, SharedKind, UnattributedReason,
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
    trips: std::cell::OnceCell<IdIndex<'a, (&'a str, u32)>>,
    /// stop/shape/service → agency kümesi. TEMBEL ve tür başına ayrı: o türden bir bulgu
    /// sorulana kadar kurulmaz.
    shared: [std::cell::OnceCell<SharedIndex<'a>>; 3],
}

/// Paylaşılabilen bir varlığın sahibi.
#[derive(Debug, Clone, Copy)]
enum Owner {
    One(AgencyRef),
    /// `SharedIndex::sets` içinde indeks (en az iki üye).
    Set(u32),
    /// Hiçbir atfedilebilir sefer kullanmıyor.
    Unused,
}

struct SharedIndex<'a> {
    by_id: IdIndex<'a, Owner>,
    /// Sıralı, tekrarsız üye kümeleri; sıra deterministiktir (aynı kayıtlar → aynı indeks).
    sets: Vec<Vec<u32>>,
}

fn kind_slot(kind: SharedKind) -> usize {
    match kind {
        SharedKind::Stop => 0,
        SharedKind::Shape => 1,
        SharedKind::Service => 2,
    }
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

        Self {
            records,
            agency_ids,
            agencies,
            routes,
            fares,
            trips: std::cell::OnceCell::new(),
            shared: Default::default(),
        }
    }

    /// trip_id → (route_id, `records.trips` sırası).
    fn trips(&self) -> &IdIndex<'a, (&'a str, u32)> {
        self.trips.get_or_init(|| {
            let interns = &self.records.trip_interns;
            IdIndex::build(
                self.records
                    .trips
                    .iter()
                    .enumerate()
                    .map(|(i, trip)| (trip.trip_id.as_str(), (interns.route_id(trip), i as u32))),
            )
        })
    }

    /// Bulgunun konusu bir seferse o seferin `records.trips` sırası: kural `trip_id` scope'u
    /// beyan ediyorsa `scope_key`, scope beyan etmiyorsa `Trip` varlığının `entity_id`'si.
    pub fn trip_of(&self, notice: &Notice) -> Option<u32> {
        let scope = gtfs_rules::get_rule(&notice.rule_id).and_then(|meta| meta.scope_key_field);
        let key = match scope {
            Some("trip_id") => notice.scope_key.as_deref(),
            None if notice.entity_type == gtfs_core::EntityType::Trip => notice.entity_id.as_deref(),
            _ => None,
        }?;
        self.trips().get(key).ok().map(|((_, index), _)| index)
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
                // Varlık türü belli ama kimliği yok: feed düzeyi özet (ör. TRP_021).
                if notice.entity_id.as_deref().is_none_or(str::is_empty) {
                    return AgencyAttribution::NotApplicable;
                }
                let scope = match notice.entity_type {
                    gtfs_core::EntityType::Agency => "agency_id",
                    gtfs_core::EntityType::Route => "route_id",
                    gtfs_core::EntityType::Trip => "trip_id",
                    gtfs_core::EntityType::Fare => "fare_id",
                    gtfs_core::EntityType::Stop => "stop_id",
                    gtfs_core::EntityType::Shape => "shape_id",
                    gtfs_core::EntityType::Service => "service_id",
                    _ => return AgencyAttribution::NotApplicable,
                };
                self.attribute_scoped(notice, scope, notice.entity_id.as_deref())
            }
            other => other,
        }
    }

    fn attribute_scoped(&self, notice: &Notice, scope: &str, key: Option<&str>) -> AgencyAttribution {
        let shared_kind = match scope {
            "stop_id" => Some(SharedKind::Stop),
            "shape_id" => Some(SharedKind::Shape),
            "service_id" => Some(SharedKind::Service),
            "agency_id" | "route_id" | "trip_id" | "fare_id" => None,
            _ => return AgencyAttribution::Unsupported,
        };
        let Some(key) = key.filter(|k| !k.is_empty()) else {
            return AgencyAttribution::Unattributed(
                if notice.entity_type == gtfs_core::EntityType::Feed {
                    UnattributedReason::FeedLevelSummary
                } else {
                    UnattributedReason::MissingScopeKey
                },
            );
        };
        if let Some(kind) = shared_kind {
            let index = self.shared(kind);
            return match index.by_id.get(key) {
                Ok((Owner::One(agency), resolution)) => AgencyAttribution::Resolved { agency, resolution },
                Ok((Owner::Set(set), _)) => AgencyAttribution::Shared { kind, set },
                Ok((Owner::Unused, _)) => AgencyAttribution::Unattributed(UnattributedReason::UnusedEntity),
                Err(miss) => unattributed(
                    miss,
                    match kind {
                        SharedKind::Stop => UnattributedReason::UnknownStop,
                        SharedKind::Shape => UnattributedReason::UnknownShape,
                        SharedKind::Service => UnattributedReason::UnknownService,
                    },
                ),
            };
        }
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
                Ok(((route_id, _), resolution)) => self.route(route_id, resolution),
                Err(miss) => return unattributed(miss, UnattributedReason::UnknownTrip),
            },
        };
        match resolved {
            Ok((agency, resolution)) => AgencyAttribution::Resolved { agency, resolution },
            Err(reason) => AgencyAttribution::Unattributed(reason),
        }
    }

    /// Paylaşılan kümenin üyeleri (agency sırası).
    pub fn shared_members(&self, kind: SharedKind, set: u32) -> &[u32] {
        self.shared(kind).sets.get(set as usize).map_or(&[], Vec::as_slice)
    }

    fn trip_agency(&self, trip: &crate::k2::trips::TripRecord) -> Option<u32> {
        self.route(self.records.trip_interns.route_id(trip), Resolution::Exact)
            .ok()
            .map(|(agency, _)| agency.0)
    }

    fn shared(&self, kind: SharedKind) -> &SharedIndex<'a> {
        self.shared[kind_slot(kind)].get_or_init(|| self.build_shared(kind))
    }

    fn build_shared(&self, kind: SharedKind) -> SharedIndex<'a> {
        use std::collections::BTreeSet;
        let records = self.records;
        let interns = &records.trip_interns;
        let mut users: FxHashMap<&'a str, BTreeSet<u32>> = FxHashMap::default();
        // Evren: feed'deki bütün kimlikler (kullanılmayanlar dahil).
        let universe: Vec<&'a str> = match kind {
            SharedKind::Stop => {
                let index = &records.stop_times_index;
                let trip_agency: FxHashMap<&str, u32> = records
                    .trips
                    .iter()
                    .filter_map(|t| self.trip_agency(t).map(|a| (t.trip_id.as_str(), a)))
                    .collect();
                for (trip_id, rows) in index.iter_trips() {
                    let Some(&agency) = trip_agency.get(trip_id.as_str()) else { continue };
                    for row in rows {
                        let stop = index.stop_id_of(row);
                        if !stop.is_empty() {
                            users.entry(stop).or_default().insert(agency);
                        }
                        // Flex satırı durak yerine lokasyon grubu gösterebilir.
                        if let Some(group) = index.flex_of(row).and_then(|f| f.location_group_id.as_deref()) {
                            users.entry(group).or_default().insert(agency);
                        }
                    }
                }
                // Flex: stop_times bir lokasyon grubunu gösteriyorsa grubun durakları o seferin
                // agency'sine aittir (location_group_stops).
                for member in &records.location_group_stops {
                    if let Some(group) = users.get(member.location_group_id.as_str()).cloned() {
                        users.entry(member.stop_id.as_str()).or_default().extend(group);
                    }
                }
                // İstasyon/giriş: alt duraklarının agency'lerini devralır (iki kademe:
                // giriş/bekleme alanı → durak → istasyon).
                let known: rustc_hash::FxHashSet<&str> =
                    records.stops.iter().map(|s| s.stop_id.as_str()).collect();
                for _ in 0..2 {
                    for stop in &records.stops {
                        let Some(parent) = stop
                            .row
                            .get("parent_station")
                            .map(String::as_str)
                            .filter(|p| !p.trim().is_empty() && known.contains(p))
                        else {
                            continue;
                        };
                        let Some(child) = users.get(stop.stop_id.as_str()).cloned() else { continue };
                        users.entry(parent).or_default().extend(child);
                    }
                }
                records.stops.iter().map(|s| s.stop_id.as_str()).collect()
            }
            SharedKind::Shape => {
                for trip in &records.trips {
                    let (Some(shape), Some(agency)) = (interns.shape_id(trip), self.trip_agency(trip)) else { continue };
                    users.entry(shape).or_default().insert(agency);
                }
                let mut ids: Vec<&'a str> = (1..records.shape_interns.len())
                    .map(|i| records.shape_interns.id_at(i as u32))
                    .collect();
                ids.extend(users.keys().copied());
                ids
            }
            SharedKind::Service => {
                for trip in &records.trips {
                    let Some(agency) = self.trip_agency(trip) else { continue };
                    users.entry(interns.service_id(trip)).or_default().insert(agency);
                }
                let mut ids: Vec<&'a str> = records.calendars.iter().map(|c| c.service_id.as_str()).collect();
                ids.extend(records.calendar_dates.added.keys().map(|k| k.as_str()));
                ids.extend(records.calendar_dates.removed.keys().map(|k| k.as_str()));
                ids.extend(users.keys().copied());
                ids
            }
        };
        let mut sets: Vec<Vec<u32>> = users
            .values()
            .filter(|s| s.len() > 1)
            .map(|s| s.iter().copied().collect())
            .collect();
        sets.sort_unstable();
        sets.dedup();
        let single = (self.agency_ids.len() == 1).then_some(AgencyRef(0));
        let owner = |id: &str| -> Owner {
            match users.get(id) {
                Some(set) if set.len() == 1 => Owner::One(AgencyRef(*set.iter().next().unwrap())),
                Some(set) if set.len() > 1 => {
                    let members: Vec<u32> = set.iter().copied().collect();
                    Owner::Set(sets.binary_search(&members).unwrap() as u32)
                }
                // Tek agency'li feed'de kullanılmayan varlığın sahibi de o agency'dir.
                _ => single.map_or(Owner::Unused, Owner::One),
            }
        };
        let by_id = IdIndex::build(universe.into_iter().filter(|id| !id.is_empty()).map(|id| (id, owner(id))));
        SharedIndex { by_id, sets }
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
    /// `records.trips` sırasıyla: en az bir bulgunun konusu olan seferler.
    affected_trips: Vec<bool>,
}

impl<'a> AgencyCounter<'a> {
    pub fn new(records: &'a EntityRecords) -> Self {
        Self {
            resolver: AgencyResolver::new(records),
            rules: Default::default(),
            affected_trips: vec![false; records.trips.len()],
        }
    }

    pub fn add(&mut self, notice: &Notice) {
        match notice.member_trips.as_deref() {
            Some(trips) => trips.iter().for_each(|&t| self.affected_trips[t as usize] = true),
            None => {
                if let Some(t) = self.resolver.trip_of(notice) {
                    self.affected_trips[t as usize] = true;
                }
            }
        }
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
            agency_affected_trip_counts: {
                let mut counts = vec![0u64; self.resolver.agency_ids.len()];
                let trips = &self.resolver.records.trips;
                for (i, _) in self.affected_trips.iter().enumerate().filter(|(_, hit)| **hit) {
                    if let Some(agency) = self.resolver.trip_agency(&trips[i]) {
                        counts[agency as usize] += 1;
                    }
                }
                counts
            },
            agency_notice_indices: Vec::new(),
            // Kümeler deterministik numaralıdır: toplamada başka bir resolver'ın yazdığı
            // anahtarlar da bu resolver'da aynı üyelere çözülür.
            shared_sets: self
                .rules
                .values()
                .flat_map(|c| c.by_attribution.keys())
                .filter_map(|a| match a {
                    AgencyAttribution::Shared { kind, set } => {
                        Some(((*kind, *set), self.resolver.shared_members(*kind, *set).to_vec()))
                    }
                    _ => None,
                })
                .collect(),
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
                    distribution.keys().flat_map(|a| members(&resolver, a)).collect();
                // Anahtarlar önce varyanta göre sıralı: aynı agency Direct ve Resolved'da
                // ayrı yerlerde olabilir.
                agencies.sort_unstable();
                agencies.dedup();
                for agency in agencies {
                    index[agency as usize].push(i);
                }
            }
            None => {
                for agency in members(&resolver, &resolver.attribute_member(notice)) {
                    index[agency as usize].push(i);
                }
            }
        }
    }
    breakdown.agency_notice_indices = index;
}

/// Atfın agency üyeleri: tek agency, paylaşılan kümenin üyeleri ya da hiçbiri.
fn members(resolver: &AgencyResolver<'_>, attribution: &AgencyAttribution) -> Vec<u32> {
    match attribution {
        AgencyAttribution::Shared { kind, set } => resolver.shared_members(*kind, *set).to_vec(),
        other => other.agency().map(|a| vec![a.0]).unwrap_or_default(),
    }
}
