//! Agency attribution types (MobilityData gtfs-validator #2201 equivalent).
//!
//! The resolver is `gtfs_pipeline::agency_attribution::AgencyResolver`; the types live
//! here because feed-level summary notices carry the distribution of their raw notices
//! in [`Notice::agency_distribution`](crate::Notice::agency_distribution).

use std::collections::BTreeMap;

/// The agency record's position in the resolver. The ID of the only agency without an
/// `agency_id` is an empty string.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct AgencyRef(pub u32);

/// How the ID was matched. If ANY step in the chain (trip → route → agency) was resolved
/// with a trimmed fallback, the result is a fallback.
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

/// Why attribution could not be made when it was expected.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum UnattributedReason {
    /// The rule declares a scope but the notice does not carry `scope_key`.
    MissingScopeKey,
    /// Feed-level summary of a scoped rule (`EntityType::Feed`, no `scope_key`): feed-level
    /// aggregations and emitter-internal summaries (STM_017, above-threshold STM_032/056).
    /// It does not belong to one entity; preserve the distribution in raw notices BEFORE
    /// summarizing.
    FeedLevelSummary,
    /// `agency_id` is absent from the feed (including a broken FK).
    UnknownAgency,
    /// `route_id` is absent from the feed (including a broken FK).
    UnknownRoute,
    /// `trip_id` is absent from the feed (including a broken FK).
    UnknownTrip,
    /// The route's `agency_id` is empty while multiple agencies exist.
    RouteWithoutAgency,
    /// `fare_id` is absent from the feed (`fare_attributes.txt`) (including a broken FK).
    UnknownFare,
    /// `stop_id` / `shape_id` / `service_id` is absent from the feed (including a broken FK).
    UnknownStop,
    UnknownShape,
    UnknownService,
    /// The entity exists in the feed, but no (attributable) trip uses it; its owner cannot
    /// be determined in a feed with multiple agencies.
    UnusedEntity,
    /// The fare's `agency_id` is empty while multiple agencies exist.
    FareWithoutAgency,
    /// A trimmed ID maps to multiple raw IDs.
    AmbiguousPaddedId,
}

/// Entity types that can be used by multiple agencies.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum SharedKind {
    Stop,
    Shape,
    Service,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum AgencyAttribution {
    /// The notice belongs directly to an agency record.
    Direct { agency: AgencyRef, resolution: Resolution },
    /// A single agency was found through the route/trip relationship.
    Resolved { agency: AgencyRef, resolution: Resolution },
    /// A stop/shape/service is used by trips from multiple agencies. `set` is a deterministic
    /// index in the agency-set table for that type (every resolver built from the same records
    /// returns the same number); members are in [`AgencyBreakdown::shared_sets`].
    Shared { kind: SharedKind, set: u32 },
    Unattributed(UnattributedReason),
    /// The rule's scope type is not resolved yet (stop, shape, service… — P6).
    Unsupported,
    /// The rule declares no scope: a feed/file-level finding unrelated to agencies.
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

/// Attribution distribution of the raw notices under a summary notice: attribution → raw
/// notice count. Its total equals the number of affected records carried by the summary.
pub type AgencyDistribution = BTreeMap<AgencyAttribution, u64>;

/// Counts for one rule. The three units must not be conflated:
/// - `finding_count`: findings included in the report — gap derivatives suppressed and
///   deduplicated, with no per-rule cap applied.
/// - `affected_entity_count`: raw records represented by the findings; an aggregated summary
///   counts all raw notices beneath it (not one per summary). Findings summarized inside the
///   emitter (with invisible members) count as 1 and fall into the `FeedLevelSummary` bucket.
/// - `displayed_sample_count`: notices carried in the result (WASM cap; equal to
///   `finding_count` in native).
///
/// `by_attribution` is the attribution distribution of `affected_entity_count`; its total
/// equals that count.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct RuleAgencyCounts {
    pub finding_count: u64,
    pub affected_entity_count: u64,
    pub displayed_sample_count: u64,
    pub by_attribution: AgencyDistribution,
}

/// Rule × agency breakdown for the feed (MobilityData gtfs-validator #2201). This is a
/// reporting layer; severity and score are not affected.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct AgencyBreakdown {
    /// `false`: some raw notices were not processed, so counts are lower bounds. Because the
    /// 1M-notice budget on the WASM path was removed on 2026-10-30, this is `true` on every
    /// path today; the field remains in the JSON schema and UI warning for compatibility.
    pub complete: bool,
    /// Agency IDs in [`AgencyRef`] order (may be empty for a single agency).
    pub agencies: Vec<String>,
    /// `agency_name` values in the same order as `agencies` (for display).
    pub agency_names: Vec<String>,
    /// Trip counts resolved to each agency through its route, in the same order as `agencies`.
    pub agency_trip_counts: Vec<u64>,
    /// In the same order as `agencies`: ascending indices of notices assigned to each agency
    /// in the result's `notices` array. Each agency in a summary's distribution is listed.
    /// The UI filters details by agency with this list; on WASM it contains only notices
    /// displayed after the cap.
    pub agency_notice_indices: Vec<Vec<u32>>,
    /// Members of [`AgencyAttribution::Shared`] sets used in distributions (agency order).
    pub shared_sets: BTreeMap<(SharedKind, u32), Vec<u32>>,
    pub rules: BTreeMap<String, RuleAgencyCounts>,
}

// ── JSON contract ───────────────────────────────────────────────────────────
//
// {
//   "complete": true,
//   "agencies": [{ "agency_id": "A", "agency_name": "Alpha", "trip_count": 120,
//                  "notice_indices": [0, 7, 9] }, …],
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
// `agency_sets` is structural (agency_id may contain `|`). Stop/shape/service findings used
// by multiple agencies fall into a multi-member set; per-agency totals may therefore exceed
// the rule total (a shared finding is counted for every member).
// Ordering is deterministic: rules and reasons use BTreeMap, sets use agency order.

#[derive(serde::Serialize)]
struct AgencyJson<'a> {
    agency_id: &'a str,
    agency_name: &'a str,
    trip_count: u64,
    notice_indices: &'a [u32],
}

#[derive(serde::Serialize)]
struct AgencySetJson<'a> {
    agency_ids: Vec<&'a str>,
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
                // The set key is the member list: one agency [a], shared [a, b, …].
                // Stop/shape/service sets with the same membership merge into one entry.
                let mut sets: BTreeMap<Vec<u32>, (u64, u64)> = BTreeMap::new();
                let mut unattributed = BTreeMap::new();
                let (mut unsupported, mut not_applicable) = (0, 0);
                for (attribution, n) in &counts.by_attribution {
                    match attribution {
                        AgencyAttribution::Shared { kind, set } => {
                            let members = self.shared_sets.get(&(*kind, *set)).cloned().unwrap_or_default();
                            sets.entry(members).or_default().0 += n;
                        }
                        AgencyAttribution::Direct { agency, resolution }
                        | AgencyAttribution::Resolved { agency, resolution } => {
                            let slot = sets.entry(vec![agency.0]).or_default();
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
                    .map(|(members, (affected, trimmed))| AgencySetJson {
                        agency_ids: members
                            .iter()
                            .map(|m| self.agencies.get(*m as usize).map(String::as_str).unwrap_or(""))
                            .collect(),
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
        let agencies: Vec<AgencyJson<'_>> = self
            .agencies
            .iter()
            .enumerate()
            .map(|(i, id)| AgencyJson {
                agency_id: id,
                agency_name: self.agency_names.get(i).map(String::as_str).unwrap_or(""),
                trip_count: self.agency_trip_counts.get(i).copied().unwrap_or(0),
                notice_indices: self.agency_notice_indices.get(i).map_or(&[], Vec::as_slice),
            })
            .collect();
        state.serialize_field("agencies", &agencies)?;
        state.serialize_field("rules", &rules)?;
        state.end()
    }
}
