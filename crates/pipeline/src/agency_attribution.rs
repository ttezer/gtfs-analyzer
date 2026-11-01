//! Notice → agency attribution (MobilityData gtfs-validator #2201 counterpart, P1).
//!
//! This is a reporting layer: severity, score, and notice content DO NOT CHANGE. Emitters
//! do not calculate agencies; attribution uses only two sources: the rule's declared
//! `scope_key_field` in the registry and the notice's runtime `scope_key` value.
//!
//! Resolution order (by `scope_key_field`):
//! - `agency_id` → agency                         ⇒ [`AgencyAttribution::Direct`]
//! - `route_id`  → route.agency_id → agency       ⇒ [`AgencyAttribution::Resolved`]
//! - `trip_id`   → trip.route_id → route → agency ⇒ [`AgencyAttribution::Resolved`]
//! - `fare_id`   → fare_attributes.agency_id → agency ⇒ [`AgencyAttribution::Resolved`]
//! - `stop_id` / `shape_id` / `service_id` → agencies of the trips using it: one agency
//!   ⇒ `Resolved`, multiple ⇒ [`AgencyAttribution::Shared`] (stations and entrances
//!   inherit the agencies of their child stops)
//! - another scope (pathway, transfer pair…)     ⇒ [`AgencyAttribution::Unsupported`]
//! - no scope declaration (feed/file level)       ⇒ [`AgencyAttribution::NotApplicable`]
//!
//! Identity matching uses the RAW ID (`"R1"` and `" R1 "` are different identities). K2
//! produces a trimmed `scope_key` on some paths; therefore, when exact matching fails and
//! the trimmed counterpart maps to a SINGLE raw ID, it links to that ID and marks
//! [`Resolution::UniqueTrimFallback`]. If it maps to multiple raw IDs, it does not link
//! ([`UnattributedReason::AmbiguousPaddedId`]). Corpus audit on 2026-09-30: the fallback
//! was needed for 463 notices in 830 feeds; no ambiguity occurred.
//!
//! In a single-agency feed, `routes.agency_id` and `fare_attributes.agency_id` may be empty
//! (GTFS conditional requirement); the record is then linked to that sole agency. An empty
//! `agency_id` is not linked when multiple agencies exist
//! ([`UnattributedReason::RouteWithoutAgency`], [`UnattributedReason::FareWithoutAgency`]).
//!
//! For duplicate IDs, the FIRST record wins (file order); duplicate identities are reported
//! by separate rules.

use rustc_hash::FxHashMap;

use gtfs_core::Notice;

use crate::k2::EntityRecords;

pub use gtfs_core::agency::{
    AgencyAttribution, AgencyBreakdown, AgencyDistribution, AgencyRef, Resolution,
    RuleAgencyCounts, SharedKind, UnattributedReason,
};

/// Raw identity → value; trimmed identity → value or ambiguity.
///
/// The trimmed map is LAZY: it is derived from the exact map only when exact matching first
/// fails (needed for two of 830 fallback feeds in the corpus). On a feed with 1.43 million
/// trips, two maps used ~140 MB; most feeds now pay for only one.
struct IdIndex<'a, V> {
    exact: FxHashMap<&'a str, V>,
    /// `None`: the trimmed counterpart maps to multiple raw identities.
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
            exact.entry(id).or_insert(value); // duplicate identity: first record wins
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
    /// trip_id → route_id (raw). LAZY: not built until a trip-scoped notice is requested.
    /// On a large feed (VBB, 282k trips), this would mean two hash maps; it is avoided for
    /// WASM memory when most aggregated rules are not trip-scoped.
    trips: std::cell::OnceCell<IdIndex<'a, &'a str>>,
    /// stop/shape/service → agency set. LAZY and separate per type: not built until a finding
    /// of that type is requested.
    shared: [std::cell::OnceCell<SharedIndex<'a>>; 3],
}

/// Owner of a shareable entity.
#[derive(Debug, Clone, Copy)]
enum Owner {
    One(AgencyRef),
    /// Index in `SharedIndex::sets` (at least two members).
    Set(u32),
    /// No attributable trip uses it.
    Unused,
}

struct SharedIndex<'a> {
    by_id: IdIndex<'a, Owner>,
    /// Ordered, deduplicated member sets; order is deterministic (same records → same index).
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

        // Record's `agency_id` → agency; if empty, use the sole agency, otherwise `without`.
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

    /// Raw ID of the agency attributed by the resolver (may be empty in a single-agency feed).
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

    /// For a raw group member of feed-level aggregation. The registry scope of an aggregated
    /// rule describes the SUMMARY identity (usually feed-level, `None`), not the member:
    /// raw TRP_005 is per trip, but the rule declares no scope. Without a scope declaration,
    /// the member's own entity (`Agency`/`Route`/`Trip` + `entity_id`) is used.
    pub fn attribute_member(&self, notice: &Notice) -> AgencyAttribution {
        match self.attribute(notice) {
            AgencyAttribution::NotApplicable => {
                // Entity type is known but identity is absent: feed-level summary (e.g. TRP_021).
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
                Ok((route_id, resolution)) => self.route(route_id, resolution),
                Err(miss) => return unattributed(miss, UnattributedReason::UnknownTrip),
            },
        };
        match resolved {
            Ok((agency, resolution)) => AgencyAttribution::Resolved { agency, resolution },
            Err(reason) => AgencyAttribution::Unattributed(reason),
        }
    }

    /// Members of a shared set (in agency order).
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
        // Universe: all identities in the feed (including unused ones).
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
                        // A Flex row may identify a location group instead of a stop.
                        if let Some(group) = index.flex_of(row).and_then(|f| f.location_group_id.as_deref()) {
                            users.entry(group).or_default().insert(agency);
                        }
                    }
                }
                // Flex: if stop_times identifies a location group, its stops belong to the
                // trip's agency (location_group_stops).
                for member in &records.location_group_stops {
                    if let Some(group) = users.get(member.location_group_id.as_str()).cloned() {
                        users.entry(member.stop_id.as_str()).or_default().extend(group);
                    }
                }
                // Station/entrance: inherit the agencies of child stops (two levels:
                // entrance/boarding area → stop → station).
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
                // In a single-agency feed, the sole agency also owns an unused entity.
                _ => single.map_or(Owner::Unused, Owner::One),
            }
        };
        let by_id = IdIndex::build(universe.into_iter().filter(|id| !id.is_empty()).map(|id| (id, owner(id))));
        SharedIndex { by_id, sets }
    }

    /// Trip count per agency (through routes; unresolved trips are not counted).
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

/// Accumulates the rule × agency breakdown one notice at a time. Aggregated summaries use
/// their nested distribution; others use [`AgencyResolver::attribute_member`]: the same
/// attribution rule as aggregation. Supplied notices must come from K7 preparation
/// (whitespace derivatives suppressed, deduplicated, and not capped).
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

    /// Here `displayed_sample_count` equals `finding_count`; after the WASM cap,
    /// [`set_displayed`] corrects it.
    pub fn finish(self, complete: bool) -> AgencyBreakdown {
        AgencyBreakdown {
            complete,
            agencies: self.resolver.agency_ids.iter().map(|id| id.to_string()).collect(),
            agency_names: self.resolver.records.agencies.iter().map(|a| a.agency_name.clone()).collect(),
            agency_trip_counts: self.resolver.trip_counts(),
            agency_notice_indices: Vec::new(),
            // Sets have deterministic numbering: keys written by another resolver during
            // aggregation resolve to the same members in this resolver.
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

/// Breakdown of a prepared notice list (native: no cap).
pub fn breakdown(notices: &[Notice], records: &EntityRecords, complete: bool) -> AgencyBreakdown {
    let mut counter = AgencyCounter::new(records);
    for notice in notices {
        counter.add(notice);
    }
    counter.finish(complete)
}

/// Refreshes `displayed_sample_count` according to notices remaining after the cap.
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

/// Agency → notice indices for the final `notices` array in the result. The attribution rule
/// is the same as counting ([`AgencyResolver::attribute_member`], distribution for summaries).
/// Called AFTER the report is built: indices depend on the serialized array order.
pub fn index_notices(breakdown: &mut AgencyBreakdown, notices: &[Notice], records: &EntityRecords) {
    let resolver = AgencyResolver::new(records);
    let mut index = vec![Vec::new(); breakdown.agencies.len()];
    for (i, notice) in notices.iter().enumerate() {
        let i = i as u32;
        match notice.agency_distribution.as_deref() {
            Some(distribution) => {
                let mut agencies: Vec<u32> =
                    distribution.keys().flat_map(|a| members(&resolver, a)).collect();
                // Keys are first sorted by variant: the same agency may appear separately
                // under Direct and Resolved.
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

/// Agency members of an attribution: one agency, members of a shared set, or none.
fn members(resolver: &AgencyResolver<'_>, attribution: &AgencyAttribution) -> Vec<u32> {
    match attribution {
        AgencyAttribution::Shared { kind, set } => resolver.shared_members(*kind, *set).to_vec(),
        other => other.agency().map(|a| vec![a.0]).unwrap_or_default(),
    }
}
