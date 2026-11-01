//! `scope_key` audit (agency attribution P0) — one feed, JSON lines.
//!
//! Usage: `cargo run --release -p gtfs-pipeline --example scope_audit -- <feed.zip> [today]`
//!
//! Sees K1–K6 notices BEFORE feed-level aggregation (`validate_bytes_inspected`);
//! aggregation removes `scope_key`, so an audit on the final list would incorrectly count it
//! as "empty". Prints one JSON line per rule; corpus totals are computed by a separate script.
//!
//! Classes (for populated, single-field, supported scopes; mutually exclusive):
//! - `exact`: `scope_key` exists in the feed as a RAW ID.
//! - `trim_unique`: absent as a raw ID, but its trimmed counterpart maps to ONE raw ID.
//! - `ambiguous`: the trimmed counterpart maps to multiple raw IDs.
//! - `missing`: matches no raw ID (broken FK or the scope carries something else).
//!
//! `verifiable*`: when the notice's `entity_type` matches the declared scope type,
//! the `scope_key == entity_id` identity check is possible; it is not possible for `Row` notices.
//!
//! Counts cover only raw notices that SURVIVE K7 whitespace suppression; suppressed ones are
//! counted separately in `ws_suppressed`. (For 22 aggregated rules, production suppression
//! runs on the summary; here it is applied per raw notice.)
//!
//! `symptoms` lines simulate `resolve_symptoms` twice in the production order (aggregation →
//! report scope → whitespace suppression → dedup): once with exact `scope_key` equality and
//! once with both sides trimmed. The difference is root→symptom matches missed by raw/trimmed
//! mixing. `cross_scope`: the root and blocked rule declare different scope fields; a scoped
//! root can suppress that rule only accidentally.

use std::collections::{BTreeMap, HashMap, HashSet};

use gtfs_core::{DedupLevel, EntityType, Notice};
use gtfs_pipeline::agency_attribution::{AgencyAttribution, AgencyResolver, Resolution};
use gtfs_pipeline::k7_reporting::{annotate_whitespace_join_provenance, dedup, suppress_whitespace_derivatives};
use gtfs_pipeline::{
    aggregate_feed_level_notices, apply_report_scope, validate_bytes_inspected, DerivedData,
    EntityRecords, ValidateResult, ValidatorConfig, WhitespaceSuppressions,
};
use serde_json::json;

/// Declared scope field → (entity type, ID set).
fn id_sets(records: &EntityRecords) -> HashMap<&'static str, (EntityType, HashSet<String>)> {
    let mut services: HashSet<String> =
        records.calendars.iter().map(|c| c.service_id.clone()).collect();
    services.extend(records.calendar_dates.added.keys().map(|s| s.to_string()));
    services.extend(records.calendar_dates.removed.keys().map(|s| s.to_string()));
    HashMap::from([
        (
            "agency_id",
            (
                EntityType::Agency,
                records.agencies.iter().filter_map(|a| a.agency_id.clone()).collect(),
            ),
        ),
        ("route_id", (EntityType::Route, records.routes.iter().map(|r| r.route_id.clone()).collect())),
        ("trip_id", (EntityType::Trip, records.trips.iter().map(|t| t.trip_id.to_string()).collect())),
        ("stop_id", (EntityType::Stop, records.stops.iter().map(|s| s.stop_id.clone()).collect())),
        ("service_id", (EntityType::Service, services)),
    ])
}

#[derive(Default)]
struct RuleStats {
    raw: u64,
    filled: u64,
    exact: u64,
    trim_unique: u64,
    ambiguous: u64,
    missing: u64,
    verifiable: u64,
    verifiable_equal: u64,
    verifiable_trim_only: u64,
    ws_suppressed: u64,
    /// `AgencyResolver` result (only notices surviving suppression).
    attribution: BTreeMap<String, u64>,
}

fn attribution_bucket(a: &AgencyAttribution) -> String {
    let res = |r: &Resolution| match r {
        Resolution::Exact => "exact",
        Resolution::UniqueTrimFallback => "trim",
    };
    match a {
        AgencyAttribution::Direct { resolution, .. } => format!("direct_{}", res(resolution)),
        AgencyAttribution::Resolved { resolution, .. } => format!("resolved_{}", res(resolution)),
        AgencyAttribution::Shared { kind, .. } => format!("shared_{kind:?}"),
        AgencyAttribution::Unattributed(reason) => format!("unattributed_{reason:?}"),
        AgencyAttribution::Unsupported => "unsupported".to_string(),
        AgencyAttribution::NotApplicable => "not_applicable".to_string(),
    }
}

/// Raw notice indices surviving K7 whitespace suppression.
fn survivors(
    notices: &[Notice],
    records: &EntityRecords,
    derived: &DerivedData,
    suppressions: &WhitespaceSuppressions,
) -> HashSet<usize> {
    let mut tagged: Vec<Notice> = notices.to_vec();
    for (i, n) in tagged.iter_mut().enumerate() {
        n.id = i.to_string();
    }
    annotate_whitespace_join_provenance(&mut tagged, records, derived);
    suppress_whitespace_derivatives(tagged, records, derived, suppressions.clone())
        .iter()
        .map(|n| n.id.parse().unwrap())
        .collect()
}

/// Agency distribution of aggregated summaries (#2201 P2): rule → (summary count,
/// total distribution count, total count carried by summaries, attributed part of distribution).
fn aggregate_distributions(
    raw: &[Notice],
    records: &EntityRecords,
    derived: &DerivedData,
) -> BTreeMap<String, (u64, u64, u64, u64)> {
    let mut notices = raw.to_vec();
    aggregate_feed_level_notices(&mut notices, records, derived);
    let mut out: BTreeMap<String, (u64, u64, u64, u64)> = BTreeMap::new();
    for n in &notices {
        let Some(distribution) = n.agency_distribution.as_ref() else { continue };
        let count = if n.rule_id == "CAL_008" {
            n.details.as_ref().and_then(|d| d.get("affected_services")).cloned()
        } else {
            n.observed_value.clone()
        };
        let entry = out.entry(n.rule_id.clone()).or_default();
        entry.0 += 1;
        entry.1 += distribution.values().sum::<u64>();
        entry.2 += count.and_then(|c| c.parse::<u64>().ok()).unwrap_or(0);
        entry.3 += distribution
            .iter()
            .filter(|(a, _)| a.agency().is_some())
            .map(|(_, n)| n)
            .sum::<u64>();
    }
    out
}

/// (root rule, blocked rule) → (exact symptoms, trimmed symptoms, scoped root count,
/// notice count of the blocked rule in the feed).
fn symptom_pairs(
    raw: &[Notice],
    records: &EntityRecords,
    derived: &DerivedData,
    suppressions: &WhitespaceSuppressions,
    config: &ValidatorConfig,
) -> BTreeMap<(String, String), (u64, u64, u64, u64)> {
    let mut notices = raw.to_vec();
    aggregate_feed_level_notices(&mut notices, records, derived);
    apply_report_scope(&mut notices, records, config);
    let notices = dedup(suppress_whitespace_derivatives(notices, records, derived, suppressions.clone()));

    let mut exact: HashMap<(&str, &str), Vec<usize>> = HashMap::new();
    let mut trimmed: HashMap<(&str, &str), Vec<usize>> = HashMap::new();
    for (i, n) in notices.iter().enumerate() {
        if let Some(k) = n.scope_key.as_deref() {
            exact.entry((n.rule_id.as_str(), k)).or_default().push(i);
            trimmed.entry((n.rule_id.as_str(), k.trim())).or_default().push(i);
        }
    }
    let mut hit_exact: HashMap<(String, String), HashSet<usize>> = HashMap::new();
    let mut hit_trim: HashMap<(String, String), HashSet<usize>> = HashMap::new();
    let mut per_rule: HashMap<&str, u64> = HashMap::new();
    for n in &notices {
        *per_rule.entry(n.rule_id.as_str()).or_default() += 1;
    }
    let mut roots: BTreeMap<(String, String), (u64, u64, u64, u64)> = BTreeMap::new();
    for root in &notices {
        let Some(scope) = root.scope_key.as_deref() else { continue };
        for blocked in &root.blocks {
            let pair = (root.rule_id.clone(), blocked.clone());
            roots.entry(pair.clone()).or_default().2 += 1;
            if let Some(v) = exact.get(&(blocked.as_str(), scope)) {
                hit_exact.entry(pair.clone()).or_default().extend(v);
            }
            if let Some(v) = trimmed.get(&(blocked.as_str(), scope.trim())) {
                hit_trim.entry(pair).or_default().extend(v);
            }
        }
    }
    for (pair, entry) in roots.iter_mut() {
        entry.0 = hit_exact.get(pair).map_or(0, |s| s.len() as u64);
        entry.1 = hit_trim.get(pair).map_or(0, |s| s.len() as u64);
        entry.3 = per_rule.get(pair.1.as_str()).copied().unwrap_or(0);
    }
    roots
}

fn aggregated_rules() -> HashSet<&'static str> {
    include_str!("../src/lib.rs")
        .lines()
        .filter_map(|l| l.trim().strip_prefix("timed_aggregate!(\""))
        .filter_map(|l| l.split('"').next())
        .collect()
}

/// Classifies registry `blocks` relationships by scope type. A scoped root
/// (non-empty `scope_key`) suppresses its blocked rule ONLY with the same `scope_key`; if
/// two rules declare different scope fields, this relationship works only for a feed-level root.
fn registry_report() {
    for meta in gtfs_rules::RULES.iter() {
        for blocked in meta.blocks {
            let Some(target) = gtfs_rules::get_rule(blocked) else { continue };
            println!(
                "{}",
                json!({
                    "root": meta.id,
                    "root_scope": meta.scope_key_field,
                    "blocked": blocked,
                    "blocked_scope": target.scope_key_field,
                    "same_scope": meta.scope_key_field == target.scope_key_field,
                })
            );
        }
    }
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    if args.get(1).map(String::as_str) == Some("--registry") {
        registry_report();
        return;
    }
    let Some(path) = args.get(1) else {
        eprintln!("Kullanım: scope_audit <feed.zip> [today_yyyymmdd]");
        std::process::exit(2);
    };
    let today: u32 = args.get(2).and_then(|s| s.parse().ok()).unwrap_or(20_260_930);
    let zip = std::fs::read(path).expect("feed okunamadı");
    let feed = std::path::Path::new(path).file_name().unwrap().to_string_lossy().to_string();

    let mut stats: BTreeMap<String, RuleStats> = BTreeMap::new();
    let mut padded: BTreeMap<&'static str, u64> = BTreeMap::new();
    let mut inspected = false;
    let mut symptoms = BTreeMap::new();
    let mut resolver_us = (0u128, 0u128, 0usize);
    let mut distributions = BTreeMap::new();
    let config = ValidatorConfig::default();

    let result = validate_bytes_inspected(
        &zip,
        &config,
        today,
        &mut |notices: &[Notice],
              records: &EntityRecords,
              derived: &DerivedData,
              suppressions: &WhitespaceSuppressions| {
            inspected = true;
            let alive = survivors(notices, records, derived, suppressions);
            let t0 = std::time::Instant::now();
            let resolver = AgencyResolver::new(records);
            let built = t0.elapsed().as_micros();
            let t1 = std::time::Instant::now();
            let buckets: Vec<String> = notices
                .iter()
                .map(|n| attribution_bucket(&resolver.attribute(n)))
                .collect();
            resolver_us = (built, t1.elapsed().as_micros(), records.agencies.len());
            symptoms = symptom_pairs(notices, records, derived, suppressions, &config);
            distributions = aggregate_distributions(notices, records, derived);
            let sets = id_sets(records);
            // trimmed → raw IDs (for measuring trim fallback and ambiguity)
            let trimmed: HashMap<&str, HashMap<&str, Vec<&str>>> = sets
                .iter()
                .map(|(field, (_, ids))| {
                    let mut index: HashMap<&str, Vec<&str>> = HashMap::new();
                    for id in ids {
                        index.entry(id.trim()).or_default().push(id.as_str());
                    }
                    (*field, index)
                })
                .collect();
            for (field, (_, ids)) in &sets {
                let n = ids.iter().filter(|id| id.trim() != id.as_str()).count() as u64;
                if n > 0 {
                    padded.insert(field, n);
                }
            }

            for (i, notice) in notices.iter().enumerate() {
                let entry = stats.entry(notice.rule_id.clone()).or_default();
                if !alive.contains(&i) {
                    entry.ws_suppressed += 1;
                    continue;
                }
                entry.raw += 1;
                *entry.attribution.entry(buckets[i].clone()).or_default() += 1;
                let Some(key) = notice.scope_key.as_deref().filter(|k| !k.is_empty()) else {
                    continue;
                };
                entry.filled += 1;
                let Some(field) = gtfs_rules::get_rule(&notice.rule_id).and_then(|m| m.scope_key_field)
                else {
                    continue;
                };
                let Some((kind, ids)) = sets.get(field) else {
                    continue; // composite or non-P0 scope
                };
                if ids.contains(key) {
                    entry.exact += 1;
                } else {
                    match trimmed[field].get(key.trim()).map(Vec::len).unwrap_or(0) {
                        0 => entry.missing += 1,
                        1 => entry.trim_unique += 1,
                        _ => entry.ambiguous += 1,
                    }
                }
                if notice.entity_type == *kind {
                    if let Some(eid) = notice.entity_id.as_deref() {
                        entry.verifiable += 1;
                        if eid == key {
                            entry.verifiable_equal += 1;
                        } else if eid.trim() == key.trim() {
                            entry.verifiable_trim_only += 1;
                        }
                    }
                }
            }
        },
    );
    let status = match &result {
        ValidateResult::Ok(_) => "ok",
        ValidateResult::Fatal(_) => "fatal",
    };
    if !inspected {
        println!("{}", json!({ "feed": feed, "status": status, "inspected": false }));
        return;
    }

    let aggregated = aggregated_rules();
    println!(
        "{}",
        json!({
            "feed": feed,
            "status": status,
            "padded_ids": padded,
            "agencies": resolver_us.2,
            "resolver_build_us": resolver_us.0,
            "resolver_attribute_us": resolver_us.1,
        })
    );
    for (rule, s) in stats {
        let meta = gtfs_rules::get_rule(&rule);
        println!(
            "{}",
            json!({
                "feed": feed,
                "rule": rule,
                "scope": meta.and_then(|m| m.scope_key_field),
                "dedup_feed": meta.is_some_and(|m| matches!(m.dedup_level, DedupLevel::Feed)),
                "aggregated": aggregated.contains(rule.as_str()),
                "raw": s.raw,
                "filled": s.filled,
                "exact": s.exact,
                "trim_unique": s.trim_unique,
                "ambiguous": s.ambiguous,
                "missing": s.missing,
                "verifiable": s.verifiable,
                "verifiable_equal": s.verifiable_equal,
                "verifiable_trim_only": s.verifiable_trim_only,
                "ws_suppressed": s.ws_suppressed,
                "attribution": s.attribution,
            })
        );
    }
    for (rule, (summaries, distributed, counted, attributed)) in distributions {
        println!(
            "{}",
            json!({
                "feed": feed,
                "aggregate": rule,
                "summaries": summaries,
                "distributed": distributed,
                "counted": counted,
                "attributed": attributed,
            })
        );
    }
    for ((root, blocked), (exact, trimmed, scoped_roots, candidates)) in symptoms {
        let scope_of = |r: &str| gtfs_rules::get_rule(r).and_then(|m| m.scope_key_field);
        println!(
            "{}",
            json!({
                "feed": feed,
                "symptom_root": root,
                "blocked": blocked,
                "scoped_roots": scoped_roots,
                "candidates": candidates,
                "exact": exact,
                "trimmed": trimmed,
                "cross_scope": scope_of(&root) != scope_of(&blocked),
            })
        );
    }
}
