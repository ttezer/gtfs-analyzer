use rustc_hash::{FxHashMap, FxHashSet};
use std::collections::{BTreeMap, BTreeSet, HashMap};

use gtfs_core::{
    DedupLevel, EntityType, FeedMetrics, FileInfo, Notice, R1Report, R2Report, R3Report, R4Report,
    R5Report, R7Report, R8Report, R9Item, R9Label, R9Report, ReportId, ReportItem, ReportSet,
    RuleClass, Severity,
};
use gtfs_rules::get_rule;

use crate::k2::{detect_gtfs_jp, EntityRecords, GTFS_JP_FILES};
use crate::k5_derived::DerivedData;
use crate::WhitespaceSuppressions;

// ── Output type ───────────────────────────────────────────────────────────────

pub struct K7Result {
    pub notices: Vec<Notice>,
    pub reports: ReportSet,
    pub metrics: FeedMetrics,
}

// ── Main function ─────────────────────────────────────────────────────────────

pub fn report(
    all_notices: Vec<Notice>,
    records: &EntityRecords,
    derived: &DerivedData,
    file_stats: Vec<FileInfo>,
    already_deduped: bool,
    coverage_complete: bool,
) -> K7Result {
    report_with_whitespace_suppressions(
        all_notices,
        records,
        derived,
        file_stats,
        already_deduped,
        coverage_complete,
        WhitespaceSuppressions::default(),
    )
}

pub fn report_with_whitespace_suppressions(
    all_notices: Vec<Notice>,
    records: &EntityRecords,
    derived: &DerivedData,
    file_stats: Vec<FileInfo>,
    already_deduped: bool,
    coverage_complete: bool,
    early_suppressions: WhitespaceSuppressions,
) -> K7Result {
    let notices =
        prepare_notices(all_notices, records, derived, already_deduped, early_suppressions);
    report_prepared(notices, records, derived, file_stats, coverage_complete)
}

/// First half of K7: suppresses whitespace derivatives, fills `service_id`, and deduplicates.
/// The returned set contains findings that ENTER the report; counts (agency distribution and
/// actual rule totals in WASM) must be taken here: suppressed derivatives are not counted and
/// the cap has not yet been applied.
pub fn prepare_notices(
    all_notices: Vec<Notice>,
    records: &EntityRecords,
    derived: &DerivedData,
    already_deduped: bool,
    early_suppressions: WhitespaceSuppressions,
) -> Vec<Notice> {
    use crate::timing::Timer;
    let all_notices = {
        let _t = Timer::start("K7::suppress_whitespace_derivatives");
        suppress_whitespace_derivatives(all_notices, records, derived, early_suppressions)
    };
    crate::timing::mem_log("K7 after suppress_whitespace_derivatives");
    let all_notices = {
        let _t = Timer::start("K7::fill_service_ids");
        fill_service_ids(all_notices, records)
    };
    crate::timing::mem_log("K7 after fill_service_ids");
    let mut notices = if already_deduped {
        all_notices
    } else {
        let _t = Timer::start("K7::dedup");
        dedup(all_notices)
    };
    crate::timing::mem_log("K7 after dedup");
    materialize_retained_whitespace_flags(&mut notices);
    crate::timing::mem_log("K7 after materialize_whitespace_flags");
    notices
}

/// Second half of K7: builds identities, root-symptom resolution, reports, and metrics from
/// the output of [`prepare_notices`] (in its capped WASM form).
pub fn report_prepared(
    mut notices: Vec<Notice>,
    records: &EntityRecords,
    derived: &DerivedData,
    file_stats: Vec<FileInfo>,
    coverage_complete: bool,
) -> K7Result {
    use crate::timing::Timer;
    // Determinism: IDs used to come from layer EMISSION order, which depended on HashMap
    // iteration and therefore varied between runs (same content, different IDs).
    // Dedup output is stably ordered by `notice_order_key`, so assigning IDs again here
    // makes them deterministic. The layer prefix (k1/k2/…) is preserved; reports are built
    // from this list afterward, so references remain consistent.
    for (i, n) in notices.iter_mut().enumerate() {
        let prefix = n.id.split('/').next().unwrap_or("k").to_string();
        n.id = format!("{prefix}/{}#{}", n.rule_id, i + 1);
    }
    crate::timing::mem_log("K7 after deterministic notice IDs");
    let resolution = {
        let _t = crate::timing::Timer::start("K7::resolve_symptoms");
        resolve_symptoms(&notices)
    };
    crate::timing::mem_log("K7 after resolve_symptoms");
    let reports = {
        let _t = Timer::start("K7::build_reports");
        build_report_set(&notices, &resolution, coverage_complete)
    };
    crate::timing::mem_log("K7 after build_reports");
    let mut metrics = {
        let _t = Timer::start("K7::build_metrics");
        build_metrics(&notices, records, derived, file_stats, coverage_complete)
    };
    crate::timing::mem_log("K7 after build_metrics");
    metrics.overall_score = reports.r5.score;
    K7Result {
        notices,
        reports,
        metrics,
    }
}

/// Preserves the DQ_016 root finding while removing from the report only type/FK symptoms
/// caused by whitespace around the raw value. K2 still parses the raw value; no PK/FK value
/// is normalized here or earlier. A numeric and rule-based audit summary is written to the
/// root finding, keeping the default report small while strict/audit consumers can see what
/// was suppressed.
/// `pub` is only for audit tools (`examples/scope_audit.rs`).
#[doc(hidden)]
pub fn suppress_whitespace_derivatives(
    mut notices: Vec<Notice>,
    records: &EntityRecords,
    derived: &DerivedData,
    early_suppressions: WhitespaceSuppressions,
) -> Vec<Notice> {
    let references = WhitespaceReferences::from_records(records);

    // 🔴 IMMUTABLE (#134): every suppressed finding MUST BE DECLARED SOMEWHERE. The declaration
    // is written to the finding's OWN file's `DQ_016` root; if that file has no root, there
    // is nowhere to write the declaration and the finding disappears WITHOUT A TRACE — worse
    // than suppressing it.
    //
    // The mirror direction exposed this: when whitespace is added to `trips.txt` while
    // `stop_times.txt` remains clean, `STM_001` (whose file is stop_times) is suppressed,
    // but no count was written anywhere because stop_times had no root. The test caught it.
    let files_with_root: BTreeSet<String> = notices
        .iter()
        .filter(|n| n.rule_id == "DQ_016")
        .filter_map(|n| n.file.clone())
        .collect();

    let mut suppressed: BTreeMap<String, (u64, BTreeSet<String>)> = files_with_root
        .iter()
        .filter_map(|file| {
            early_suppressions
                .audit_for(file)
                .map(|audit| (file.clone(), audit))
        })
        .collect();
    // RESULT rules (2b) that fire when an identity join is broken only by whitespace.
    // Building this is cheap, but it is skipped entirely for feeds without whitespace.
    let joins = (!files_with_root.is_empty()
        && notices
            .iter()
            .any(|n| JOIN_DERIVATIVE_RULES.contains(&n.rule_id.as_str())))
    .then(|| JoinWhitespace::from_records(records, derived));
    notices.retain(|notice| {
        // K6 marked this internally: the finding exists because the route type disappeared
        // only due to whitespace (speed and headway thresholds; `mark_whitespace_join`).
        if let Some(file) = notice
            .details
            .as_ref()
            .and_then(|d| d.get("whitespace_join_file"))
        {
            if files_with_root.contains(file.as_str()) {
                let entry = suppressed.entry(file.clone()).or_default();
                entry.0 += 1;
                entry.1.insert(notice.rule_id.clone());
                return false;
            }
        }
        // The declaration is written to the root of the file CARRYING THE WHITESPACE, not the
        // finding's own file: if `routes.route_id = "R1  "`, `TRP_011` (trips.txt) is
        // declared under the `routes.txt` root. #134's core requirement remains: if that
        // root is absent, the finding does not disappear without a trace; it REMAINS.
        if let Some(file) = joins
            .as_ref()
            .and_then(|joins| joins.declaration_file(notice))
        {
            if files_with_root.contains(file) {
                let entry = suppressed.entry(file.to_string()).or_default();
                entry.0 += 1;
                entry.1.insert(notice.rule_id.clone());
                return false;
            }
        }
        if !is_whitespace_derivative(notice, &references) {
            return true;
        }
        // Do not suppress when the file has no root.
        if !notice
            .file
            .as_deref()
            .is_some_and(|f| files_with_root.contains(f))
        {
            return true;
        }
        let file = notice.file.clone().unwrap_or_else(|| "<feed>".to_string());
        let entry = suppressed.entry(file).or_default();
        entry.0 += 1;
        entry.1.insert(notice.rule_id.clone());
        false
    });

    for root in notices.iter_mut().filter(|n| n.rule_id == "DQ_016") {
        let Some(file) = root.file.as_deref() else {
            continue;
        };
        let Some((count, rules)) = suppressed.get(file) else {
            continue;
        };
        let details = root.details.get_or_insert_with(gtfs_core::NoticeDetails::new);
        details.insert("suppressed_derivative_count".to_string(), count.to_string());
        details.insert(
            "suppressed_derivative_rules".to_string(),
            rules.iter().cloned().collect::<Vec<_>>().join(", "),
        );
    }
    notices
}

/// RESULT rules that fire because an identity join is broken only by whitespace.
///
/// The list came from measurement (18 September, 25 feeds; each identity name was trimmed
/// independently). No general graph was built deliberately: an independent defect of THAT
/// TRIP must not be hidden merely because its join is broken. `CAL_024` ("not active within
/// 7 days") is deliberately ABSENT — K7 has no "today", and trimming the service can also
/// make it inactive within 7 days.
///
/// `OPR_009` is deliberately ABSENT (the 18 September full validation showed over-suppression):
/// a shapeless trip's night-service summary is emitted with the TRIP ID instead of the route,
/// but the information is REAL — suppressing it made the route's night service disappear from
/// the report entirely (`mdb-1187`: raw 0, trimmed 9). Correct keying requires trimming the
/// identity, which would violate #85. `STM_014` speed findings keyed per trip are likewise
/// not suppressed for the same reason.
///
/// ⚠️ This list only determines whether the structure is BUILT; rule selection is in the
/// `match` arms of `declaration_file`. Update both together — an arm missing from the list
/// never runs on a feed carrying only that rule's finding.
const JOIN_DERIVATIVE_RULES: &[&str] = &[
    "TRP_011", "TRP_026", "TRP_003", "OPR_011", "CAL_011", "STP_020", "SHP_018", "SHP_019",
    "XFL_002", "XFL_012", "RTS_016", "OPR_004",
];

/// Mark join-derived notices before feed-level aggregation can erase their entity id.
/// K7 uses this marker to declare/suppress the notice in the file carrying the padding.
#[doc(hidden)]
pub fn annotate_whitespace_join_provenance(
    notices: &mut [Notice],
    records: &EntityRecords,
    derived: &DerivedData,
) {
    let joins = JoinWhitespace::from_records(records, derived);
    for notice in notices.iter_mut() {
        if !JOIN_DERIVATIVE_RULES.contains(&notice.rule_id.as_str()) {
            continue;
        }
        let Some(file) = joins.declaration_file(notice) else {
            continue;
        };
        notice
            .details
            .get_or_insert_with(gtfs_core::NoticeDetails::new)
            .insert("whitespace_join_file".to_string(), file.to_string());
    }
}

/// #85 compares identities RAW and this is preserved: `"R1  "` and `R1` are NOT the same
/// route. This structure does not change identities; it only records whether a result finding
/// fired because the raw join was broken while the trimmed join was sound, and WHICH FILE
/// carried the whitespace. The finding is declared under that file's `DQ_016` root.
struct JoinWhitespace<'a> {
    interns: &'a crate::k2::trips::TripInternTable,
    trips: FxHashMap<&'a str, &'a crate::k2::trips::TripRecord>,
    route_raw: FxHashSet<&'a str>,
    /// trimmed route_id → whether the route name exists (same criterion as K6 `TRP_011`: raw name non-empty)
    route_named: FxHashMap<&'a str, bool>,
    service_raw: FxHashSet<&'a str>,
    /// trimmed service_id → (has an active date, file of the whitespace-padded definition; otherwise calendar)
    service_trimmed: FxHashMap<&'a str, (bool, &'static str)>,
    service_active_raw: FxHashSet<&'a str>,
    service_used_raw: FxHashSet<&'a str>,
    service_used_trimmed: FxHashSet<&'a str>,
    stop_used_raw: FxHashSet<&'a str>,
    stop_used_trimmed: FxHashSet<&'a str>,
    shape_used_raw: FxHashSet<&'a str>,
    shape_used_trimmed: FxHashSet<&'a str>,
    /// raw shape_id → trips that use it
    shape_trips: FxHashMap<&'a str, Vec<&'a str>>,
    st_trip_raw: FxHashSet<&'a str>,
    st_trip_trimmed: FxHashSet<&'a str>,
    /// raw route_id (as written in trips) → trips of that route; route-level rules (2c)
    route_trips: FxHashMap<&'a str, Vec<&'a crate::k2::trips::TripRecord>>,
    service_weekend_raw: FxHashSet<&'a str>,
    service_weekend_trimmed: FxHashSet<&'a str>,
}

impl<'a> JoinWhitespace<'a> {
    fn from_records(records: &'a EntityRecords, derived: &'a DerivedData) -> Self {
        let ti = &records.trip_interns;
        let trimmed = |set: &FxHashSet<&'a str>| set.iter().map(|v| v.trim()).collect();

        let mut trips = FxHashMap::default();
        let mut service_used_raw = FxHashSet::default();
        let mut shape_used_raw = FxHashSet::default();
        let mut shape_trips: FxHashMap<&'a str, Vec<&'a str>> = FxHashMap::default();
        let mut route_trips: FxHashMap<&'a str, Vec<&'a crate::k2::trips::TripRecord>> =
            FxHashMap::default();
        for trip in &records.trips {
            trips.insert(trip.trip_id.as_str(), trip);
            route_trips.entry(ti.route_id(trip)).or_default().push(trip);
            service_used_raw.insert(ti.service_id(trip));
            if let Some(shape) = ti.shape_id(trip) {
                shape_used_raw.insert(shape);
                shape_trips
                    .entry(shape)
                    .or_default()
                    .push(trip.trip_id.as_str());
            }
        }

        let route_raw: FxHashSet<&'a str> =
            records.routes.iter().map(|r| r.route_id.as_str()).collect();
        let mut route_named: FxHashMap<&'a str, bool> = FxHashMap::default();
        for route in &records.routes {
            let named = route
                .route_short_name
                .as_deref()
                .is_some_and(|s| !s.is_empty())
                || route
                    .route_long_name
                    .as_deref()
                    .is_some_and(|s| !s.is_empty());
            *route_named.entry(route.route_id.trim()).or_default() |= named;
        }

        let active = &derived.calendar_bitmap.active_dates;
        let calendar: FxHashSet<&'a str> = records
            .calendars
            .iter()
            .map(|c| c.service_id.as_str())
            .collect();
        let mut service_raw = calendar.clone();
        service_raw.extend(
            records
                .calendar_dates
                .exception_count
                .keys()
                .map(|k| k.as_str()),
        );
        let service_active_raw: FxHashSet<&'a str> = active
            .iter()
            .filter(|(_, dates)| !dates.is_empty())
            .map(|(k, _)| k.as_str())
            .collect();
        // SAME weekend criterion as `OPR_004` (JDN mod 7: Saturday=5, Sunday=6).
        let service_weekend_raw: FxHashSet<&'a str> = active
            .iter()
            .filter(|(_, dates)| {
                dates.iter().any(|&d| {
                    let jdn = crate::k6_analytics::yyyymmdd_to_jdn(d);
                    jdn % 7 == 5 || jdn % 7 == 6
                })
            })
            .map(|(k, _)| k.as_str())
            .collect();
        let mut service_trimmed: FxHashMap<&'a str, (bool, &'static str)> = FxHashMap::default();
        for &id in &service_raw {
            let file = if calendar.contains(id) {
                "calendar.txt"
            } else {
                "calendar_dates.txt"
            };
            let entry = service_trimmed.entry(id.trim()).or_insert((false, file));
            entry.0 |= service_active_raw.contains(id);
            // The declaration is written to the file WHERE THE WHITESPACE STOPS. `mdb-488`:
            // clean `6` is undated in calendar.txt, while dated `" 6"` comes from
            // calendar_dates.txt — the root is there.
            if id != id.trim() {
                entry.1 = file;
            }
        }

        let mut stop_used_raw: FxHashSet<&'a str> = records
            .stop_times_index
            .stop_id_set
            .iter()
            .map(smol_str::SmolStr::as_str)
            .collect();
        stop_used_raw.extend(records.stop_times.iter().map(|r| r.stop_id.as_str()));
        let mut st_trip_raw: FxHashSet<&'a str> = records
            .stop_times_index
            .trip_id_set
            .iter()
            .map(smol_str::SmolStr::as_str)
            .collect();
        st_trip_raw.extend(records.stop_times.iter().map(|r| r.trip_id.as_str()));

        Self {
            interns: ti,
            service_weekend_trimmed: trimmed(&service_weekend_raw),
            service_weekend_raw,
            route_trips,
            trips,
            route_named,
            service_trimmed,
            service_active_raw,
            service_used_trimmed: trimmed(&service_used_raw),
            stop_used_trimmed: trimmed(&stop_used_raw),
            shape_used_trimmed: trimmed(&shape_used_raw),
            st_trip_trimmed: trimmed(&st_trip_raw),
            route_raw,
            service_raw,
            service_used_raw,
            stop_used_raw,
            shape_used_raw,
            shape_trips,
            st_trip_raw,
        }
    }

    /// The file carrying the whitespace when the finding results from a whitespace-broken join.
    fn declaration_file(&self, notice: &Notice) -> Option<&'static str> {
        let id = notice.entity_id.as_deref()?;
        let padded = id != id.trim();
        match notice.rule_id.as_str() {
            // The trip's route does not match raw, but matches a NAMED route after trimming.
            "TRP_011" => {
                let route = self.trip_route(id)?;
                if self.route_raw.contains(route) || !*self.route_named.get(route.trim())? {
                    return None;
                }
                Some(if route != route.trim() {
                    "trips.txt"
                } else {
                    "routes.txt"
                })
            }
            // The trip's calendar is undated/undefined raw, but dated/defined after trimming.
            "TRP_026" | "TRP_003" => {
                let service = self.trips.get(id).map(|t| self.service_of(t))?;
                let (active, file) = *self.service_trimmed.get(service.trim())?;
                let broken = if notice.rule_id == "TRP_003" {
                    !self.service_raw.contains(service)
                } else {
                    !self.service_active_raw.contains(service) && active
                };
                broken.then_some(if service != service.trim() {
                    "trips.txt"
                } else {
                    file
                })
            }
            // The used calendar is undated raw (undefined or defined-but-empty), but dated
            // after trimming. `mdb-488`: `6` is undated in calendar.txt; `" 6"` supplies the date.
            "OPR_011" => {
                if self.service_active_raw.contains(id) {
                    return None;
                }
                let (active, file) = *self.service_trimmed.get(id.trim())?;
                active.then_some(if padded && !self.service_raw.contains(id) {
                    "trips.txt"
                } else {
                    file
                })
            }
            // It appears defined but unused; after trimming, it is used.
            "CAL_011" => (!self.service_used_raw.contains(id)
                && self.service_used_trimmed.contains(id.trim()))
            .then_some(if !padded {
                "trips.txt"
            } else if notice.file.as_deref() == Some("calendar_dates.txt") {
                "calendar_dates.txt"
            } else {
                "calendar.txt"
            }),
            "STP_020" => (!self.stop_used_raw.contains(id)
                && self.stop_used_trimmed.contains(id.trim()))
            .then_some(if padded {
                "stops.txt"
            } else {
                "stop_times.txt"
            }),
            "SHP_018" => (!self.shape_used_raw.contains(id)
                && self.shape_used_trimmed.contains(id.trim()))
            .then_some(if padded { "shapes.txt" } else { "trips.txt" }),
            // NONE of the trips using the shape has a raw stop_times record; ALL have one
            // after trimming. If one is genuinely unrecorded, the finding is real.
            "SHP_019" => {
                let trips = self.shape_trips.get(id)?;
                let all_broken = trips.iter().all(|t| {
                    !self.st_trip_raw.contains(t) && self.st_trip_trimmed.contains(t.trim())
                });
                (all_broken && !trips.is_empty()).then_some(
                    if trips.iter().any(|t| *t != t.trim()) {
                        "trips.txt"
                    } else {
                        "stop_times.txt"
                    },
                )
            }
            // ── Route level (2c). These rules are EXISTENTIAL ("no trip of the route has X"):
            // if one trip's join is repaired by trimming, the rule is silent in the trimmed world.
            "XFL_012" => {
                let trip = self.route_trips.get(id)?.iter().find(|t| {
                    !self.st_trip_raw.contains(t.trip_id.as_str())
                        && self.st_trip_trimmed.contains(t.trip_id.trim())
                })?;
                Some(if trip.trip_id != trip.trip_id.trim() {
                    "trips.txt"
                } else {
                    "stop_times.txt"
                })
            }
            "RTS_016" => self.route_service_repair(id, |svc| {
                (!self.service_active_raw.contains(svc))
                    .then(|| self.service_trimmed.get(svc.trim()).filter(|e| e.0))
                    .flatten()
                    .is_some()
            }),
            "OPR_004" => self.route_service_repair(id, |svc| {
                !self.service_weekend_raw.contains(svc)
                    && self.service_weekend_trimmed.contains(svc.trim())
            }),
            "XFL_002" => (!self.st_trip_raw.contains(id)
                && self.st_trip_trimmed.contains(id.trim()))
            .then_some(if padded {
                "trips.txt"
            } else {
                "stop_times.txt"
            }),
            _ => None,
        }
    }

    /// The file carrying whitespace when one of the route's trips satisfies `repaired`
    /// after trimming its calendar.
    fn route_service_repair(
        &self,
        route: &str,
        repaired: impl Fn(&str) -> bool,
    ) -> Option<&'static str> {
        let svc = self
            .route_trips
            .get(route)?
            .iter()
            .map(|t| self.service_of(t))
            .find(|svc| repaired(svc))?;
        let (_, file) = *self.service_trimmed.get(svc.trim())?;
        Some(if svc != svc.trim() && !self.service_raw.contains(svc) {
            "trips.txt"
        } else {
            file
        })
    }

    fn trip_route(&self, trip_id: &str) -> Option<&'a str> {
        self.trips.get(trip_id).map(|t| self.interns.route_id(t))
    }

    fn service_of(&self, trip: &crate::k2::trips::TripRecord) -> &'a str {
        self.interns.service_id(trip)
    }
}

fn is_whitespace_derivative(notice: &Notice, references: &WhitespaceReferences<'_>) -> bool {
    if notice.whitespace_derived {
        return true;
    }

    // ⚠️ A "streaming K2 parse rejection" branch used to be here — REMOVED (#125, 2026-08-12).
    // It required whitespace around the observed value, but all stop_times and shapes stream
    // emits read through `get_col`, which applies `.trim()` (`k2/common.rs`). "raw" in
    // `parse_u32_raw`/`parse_f64_raw`/`parse_gtfs_time_raw` means reading from a slice
    // rather than RowMap; it does not mean RAW VALUE. Thus `observed_value` never carried
    // whitespace, the condition was never satisfied, and the branch was unreachable. Two
    // measurements: (1) a feed with padded shape_pt_lat/shape_dist_traveled produced only
    // DQ_016, with no derivative to suppress; (2) after removing the branch, the full suite
    // passed 921/921. The message-pattern list diverging from K2 was also pointless — a pattern
    // would have been added to an unreachable branch. Different strictness between stream and
    // RowMap reads is a separate issue and remains.

    // K4 reports only a raw ID missing from the target. If its trimmed form exists in the target
    // set, this is exactly a whitespace-derived FK symptom; genuinely unknown "  UNKNOWN "
    // values remain visible.
    if notice.whitespace_candidate && normalized_reference_exists(notice, references) {
        return true;
    }

    // MIRROR DIRECTION (#134): the observed value is CLEAN but the reference side carries whitespace.
    // `whitespace_candidate` is NOT set in this case (K4 looks only at the observed value), so
    // the branch above never saw `mdb-992`.
    //
    // Two guards keep this narrow:
    //   1. `set_for` recognizes only ID fields → range/enum findings cannot enter.
    //   2. The raw value MUST NOT be in the set. If it is, this is not a "target missing"
    //      symptom (for example, a duplicate-key finding) and must not be suppressed.
    // A genuinely unknown ID has no trimmed counterpart either → it remains visible.
    mirror_whitespace_symptom(notice, references)
}

/// Internal flags remain allocation-free until the K7 decision. Recreates the old JSON contract
/// in a retained finding when no root exists or its trimmed counterpart is not truly present.
fn materialize_retained_whitespace_flags(notices: &mut [Notice]) {
    for notice in notices {
        if notice.whitespace_derived {
            notice
                .details
                .get_or_insert_with(gtfs_core::NoticeDetails::new)
                .insert("whitespace_derived".to_string(), "true".to_string());
        }
        if notice.whitespace_candidate {
            notice
                .details
                .get_or_insert_with(gtfs_core::NoticeDetails::new)
                .insert("whitespace_candidate".to_string(), "true".to_string());
        }
    }
}

/// Raw ID is missing but appears when whitespace is ignored → whitespace-derived FK symptom.
/// Internal key naming the `stop_times` branch in `IdSets`. GTFS has no such FIELD; it is
/// accessed only through `reference_field_for`, so it cannot collide with a real field name.
const STOP_TIMES_TRIP_ID: &str = "\0stop_times::trip_id";
/// Internal key naming fares DEFINED only in `fare_attributes.txt` within `IdSets`.
const FARE_ATTRIBUTES_FARE_ID: &str = "\0fare_attributes::fare_id";

/// Selects, per rule, which ID set suppression should inspect.
///
/// 🔴 ROOT BUG (run 17, `mdb-2653`): sets were built under the assumption "the table
/// DEFINING this field" — `trip_id` → `trips.txt`. `XFL_002` asks the REVERSE direction:
/// "does this trip have a record in `stop_times`?" Because the observed value is the trip's
/// OWN ID, `contains_trimmed("trip_id", …)` **always** returned true and the finding was
/// discarded as whitespace residue. Guaranteed false negative: `XFL_002` was invisible on
/// every feed with whitespace in `trip_id`.
///
/// Measurement (`mdb-2653`, 138,962 trips): **108** of 421 trips WITHOUT a `stop_times`
/// record had whitespace in their ID, and all 108 were suppressed; we reported 313, MD reported
/// 732. The IDs of those 108 trips do not occur in `stop_times.txt` in ANY spelling (verified) —
/// the findings were real.
///
/// The correct set is IDs occurring in `stop_times`. The distinction is then correct:
/// · ID NOT THERE → finding is real and remains VISIBLE;
/// · trimmed ID IS THERE → trip is genuinely recorded, difference is only whitespace →
///   suppress it and declare it under the `DQ_016` root.
fn reference_field_for<'f>(rule_id: &str, field: &'f str) -> &'f str {
    match (rule_id, field) {
        ("XFL_002", "trip_id") => STOP_TIMES_TRIP_ID,
        // The fare-side version of the same defect: the general `fare_id` set also contains
        // the IDs defined by `fare_rules.txt` itself. `FRL_001` says that an ID from
        // fare_rules is missing in fare_attributes; the observed value finds itself in the
        // general set, so a truly undefined fare such as `'FNOPE '` is mistaken for whitespace
        // residue and suppressed.
        ("FRL_001", "fare_id") => FARE_ATTRIBUTES_FARE_ID,
        _ => field,
    }
}

fn mirror_whitespace_symptom(notice: &Notice, references: &WhitespaceReferences<'_>) -> bool {
    let Some(field) = notice.field.as_deref() else {
        return false;
    };
    let Some(observed) = notice.observed_value.as_deref() else {
        return false;
    };
    field
        .split('|')
        .zip(observed.split('|'))
        .any(|(field, value)| {
            let field = reference_field_for(&notice.rule_id, field);
            let value = value.trim();
            !value.is_empty()
                && !references.contains(field, value)
                && references.contains_trimmed(field, value)
        })
}

fn normalized_reference_exists(notice: &Notice, references: &WhitespaceReferences<'_>) -> bool {
    let Some(field) = notice.field.as_deref() else {
        return false;
    };
    let Some(observed) = notice.observed_value.as_deref() else {
        return false;
    };
    field
        .split('|')
        .zip(observed.split('|'))
        .any(|(field, value)| {
            let field = reference_field_for(&notice.rule_id, field);
            let value = value.trim();
            !value.is_empty() && references.contains(field, value)
        })
}

struct IdSets<'a> {
    agency_id: FxHashSet<&'a str>,
    stop_id: FxHashSet<&'a str>,
    route_id: FxHashSet<&'a str>,
    trip_id: FxHashSet<&'a str>,
    service_id: FxHashSet<&'a str>,
    fare_id: FxHashSet<&'a str>,
    area_id: FxHashSet<&'a str>,
    fare_product_id: FxHashSet<&'a str>,
    fare_media_id: FxHashSet<&'a str>,
    rider_category_id: FxHashSet<&'a str>,
    network_id: FxHashSet<&'a str>,
    shape_id: FxHashSet<&'a str>,
    zone_id: FxHashSet<&'a str>,
    /// 🔴 Trip IDs OCCURRING in `stop_times.txt` — not those DEFINED in `trips.txt`.
    /// Required for REVERSE-DIRECTION rules such as `XFL_002`; see `reference_field_for`.
    stop_times_trip_id: FxHashSet<&'a str>,
    /// Fares defined only in `fare_attributes.txt`; see `reference_field_for`.
    fare_attributes_fare_id: FxHashSet<&'a str>,
}

impl<'a> IdSets<'a> {
    fn from_records(records: &'a EntityRecords) -> Self {
        let mut refs = Self {
            agency_id: FxHashSet::default(),
            stop_id: FxHashSet::default(),
            route_id: FxHashSet::default(),
            trip_id: FxHashSet::default(),
            service_id: FxHashSet::default(),
            fare_id: FxHashSet::default(),
            area_id: FxHashSet::default(),
            fare_product_id: FxHashSet::default(),
            fare_media_id: FxHashSet::default(),
            rider_category_id: FxHashSet::default(),
            network_id: FxHashSet::default(),
            shape_id: FxHashSet::default(),
            zone_id: FxHashSet::default(),
            stop_times_trip_id: FxHashSet::default(),
            fare_attributes_fare_id: FxHashSet::default(),
        };
        refs.agency_id.extend(
            records
                .agencies
                .iter()
                .filter_map(|r| r.agency_id.as_deref()),
        );
        refs.stop_id
            .extend(records.stops.iter().map(|r| r.stop_id.as_str()));
        refs.route_id
            .extend(records.routes.iter().map(|r| r.route_id.as_str()));
        refs.trip_id
            .extend(records.trips.iter().map(|r| r.trip_id.as_str()));
        // Trip IDs OCCURRING in `stop_times`. `StopTimesIndex::trip_id_set` is already
        // retained for `XFL_002`; collect them from typed records for the buffered branch too.
        refs.stop_times_trip_id.extend(
            records
                .stop_times_index
                .trip_id_set
                .iter()
                .map(smol_str::SmolStr::as_str),
        );
        refs.service_id
            .extend(records.calendars.iter().map(|r| r.service_id.as_str()));
        refs.service_id
            .extend(records.trip_interns.service_ids.iter().map(|v| v.as_str()));
        refs.fare_id
            .extend(records.fare_attributes.iter().map(|r| r.fare_id.as_str()));
        refs.fare_attributes_fare_id
            .extend(records.fare_attributes.iter().map(|r| r.fare_id.as_str()));
        refs.fare_id
            .extend(records.fare_rules.iter().map(|r| r.fare_id.as_str()));
        refs.area_id
            .extend(records.areas.iter().map(|r| r.area_id.as_str()));
        refs.fare_product_id.extend(
            records
                .fare_products
                .iter()
                .map(|r| r.fare_product_id.as_str()),
        );
        refs.fare_media_id
            .extend(records.fare_media.iter().map(|r| r.fare_media_id.as_str()));
        refs.rider_category_id.extend(
            records
                .rider_categories
                .iter()
                .map(|r| r.rider_category_id.as_str()),
        );
        refs.network_id
            .extend(records.networks.iter().map(|r| r.network_id.as_str()));
        refs.shape_id.extend(
            records
                .trips
                .iter()
                .filter_map(|r| records.trip_interns.shape_id(r)),
        );
        refs.shape_id
            .extend(records.shapes.iter().map(|r| records.shape_interns.id(r)));
        refs.zone_id.extend(
            records
                .stops
                .iter()
                .filter_map(|r| r.row.get("zone_id").map(String::as_str)),
        );
        refs
    }

    /// Field name → that field's ID set. Unknown fields return `None`; range/enum findings
    /// therefore cannot enter whitespace attribution.
    fn get(&self, field: &str) -> Option<&FxHashSet<&'a str>> {
        Some(match field {
            "agency_id" => &self.agency_id,
            "stop_id" => &self.stop_id,
            "route_id" => &self.route_id,
            "trip_id" => &self.trip_id,
            "service_id" => &self.service_id,
            "fare_id" => &self.fare_id,
            "area_id" => &self.area_id,
            "fare_product_id" => &self.fare_product_id,
            "fare_media_id" => &self.fare_media_id,
            "rider_category_id" => &self.rider_category_id,
            "network_id" => &self.network_id,
            "shape_id" => &self.shape_id,
            "zone_id" | "origin_id" | "destination_id" | "contains_id" => &self.zone_id,
            STOP_TIMES_TRIP_ID => &self.stop_times_trip_id,
            FARE_ATTRIBUTES_FARE_ID => &self.fare_attributes_fare_id,
            _ => return None,
        })
    }

    /// Trimmed copies of the same sets. Because `str::trim` returns subslices, the lifetime
    /// is the same and the copy is ALLOCATION-FREE.
    fn trimmed(&self) -> Self {
        Self {
            agency_id: self.agency_id.iter().map(|v| v.trim()).collect(),
            stop_id: self.stop_id.iter().map(|v| v.trim()).collect(),
            route_id: self.route_id.iter().map(|v| v.trim()).collect(),
            trip_id: self.trip_id.iter().map(|v| v.trim()).collect(),
            service_id: self.service_id.iter().map(|v| v.trim()).collect(),
            fare_id: self.fare_id.iter().map(|v| v.trim()).collect(),
            area_id: self.area_id.iter().map(|v| v.trim()).collect(),
            fare_product_id: self.fare_product_id.iter().map(|v| v.trim()).collect(),
            fare_media_id: self.fare_media_id.iter().map(|v| v.trim()).collect(),
            rider_category_id: self.rider_category_id.iter().map(|v| v.trim()).collect(),
            network_id: self.network_id.iter().map(|v| v.trim()).collect(),
            shape_id: self.shape_id.iter().map(|v| v.trim()).collect(),
            zone_id: self.zone_id.iter().map(|v| v.trim()).collect(),
            stop_times_trip_id: self.stop_times_trip_id.iter().map(|v| v.trim()).collect(),
            fare_attributes_fare_id: self
                .fare_attributes_fare_id
                .iter()
                .map(|v| v.trim())
                .collect(),
        }
    }
}

/// Raw and trimmed ID sets.
///
/// 🔴 **BOTH ARE INDEXED UP FRONT.** The first implementation searched trimmed values with
/// `set.iter().any(|k| k.trim() == value)` — scanning the ENTIRE set for every finding.
/// Because `is_whitespace_derivative` runs on EVERY notice, this became quadratic:
/// `mdb-2904` (85 MB, 3,763,204 `STM_056` findings alone) did not finish in 30 minutes,
/// although it completed before the patch. The corpus run caught it.
struct WhitespaceReferences<'a> {
    raw: IdSets<'a>,
    trimmed: IdSets<'a>,
}

impl<'a> WhitespaceReferences<'a> {
    fn from_records(records: &'a EntityRecords) -> Self {
        let raw = IdSets::from_records(records);
        let trimmed = raw.trimmed();
        Self { raw, trimmed }
    }

    fn contains(&self, field: &str, value: &str) -> bool {
        self.raw.get(field).is_some_and(|set| set.contains(value))
    }

    /// Is it present in the ID set when WHITESPACE IS IGNORED (issue #134)?
    ///
    /// The cascade covered only ONE DIRECTION — whitespace in the observed value. `mdb-992`
    /// is the MIRROR CASE: `stop_times.trip_id` is clean (`10242975`), whitespace is on
    /// the REFERENCE side (`trips.trip_id` = `' 10242975'`, all 6,523 rows). Raw
    /// intersection 0, trimmed intersection 6,523 — one producer habit created 103,649 findings.
    fn contains_trimmed(&self, field: &str, value: &str) -> bool {
        self.trimmed
            .get(field)
            .is_some_and(|set| set.contains(value))
    }
}

// ── Fill service calendar (service_id) ─────────────────────────────────────────

/// Adds service_id to trip-based (entity_type=Trip) notices from the trip_id in
/// entity_id, for the "Service Calendar" column in R2. Existing values (such as those
/// set during emission for OPR_003/005) are preserved. Non-trip entities (Route/Feed/Stop…)
/// are skipped.
fn fill_service_ids(mut notices: Vec<Notice>, records: &EntityRecords) -> Vec<Notice> {
    let ti = &records.trip_interns;
    let trip_service: std::collections::HashMap<&str, &str> = records
        .trips
        .iter()
        .filter(|t| !ti.service_id(t).is_empty())
        .map(|t| (t.trip_id.as_str(), ti.service_id(t)))
        .collect();
    for n in &mut notices {
        if n.service_id.is_none() && matches!(n.entity_type, EntityType::Trip) {
            if let Some(eid) = n.entity_id.as_deref() {
                if let Some(&svc) = trip_service.get(eid) {
                    n.service_id = Some(svc.to_string());
                }
            }
        }
    }
    notices
}

// ── WP-10a: Dedup ─────────────────────────────────────────────────────────────

/// A thread-independent, stable total ordering key for notices.
/// Threaded K2/K6 notices may be produced in nondeterministic order; because dedup
/// keep-first and the post-dedup cap (on the WASM side) depend on this order, sorting
/// first ensures "same feed → same representatives/same count".
pub fn notice_order_key(n: &Notice) -> (&str, &str, &str, u64, &str, &str, &str) {
    (
        n.rule_id.as_str(),
        n.entity_type.stable_name(),
        n.entity_id.as_deref().unwrap_or(""),
        n.line.unwrap_or(0),
        n.file.as_deref().unwrap_or(""),
        n.field.as_deref().unwrap_or(""),
        n.scope_key.as_deref().unwrap_or(""),
    )
}

pub fn dedup(notices: Vec<Notice>) -> Vec<Notice> {
    // Determinism: sort first so the keep-first representative is thread-independent.
    let mut notices = notices;
    // MUST BE STABLE: otherwise which of two equal-key findings becomes the keep-first
    // representative depends on input permutation and therefore on HashMap iteration.
    notices.sort_by(|a, b| notice_order_key(a).cmp(&notice_order_key(b)));
    let mut seen: FxHashSet<String> = FxHashSet::default();
    seen.reserve(notices.len());
    let mut out: Vec<Notice> = Vec::with_capacity(notices.len());
    // Reusable buffer: push_str/write! instead of format!() — removes the per-notice String alloc
    let mut key_buf = String::with_capacity(128);
    for n in notices {
        let level = get_rule(&n.rule_id)
            .map(|m| m.dedup_level)
            .unwrap_or(DedupLevel::Row);
        key_buf.clear();
        write_dedup_key(&mut key_buf, &n, level);
        if !seen.contains(key_buf.as_str()) {
            seen.insert(key_buf.clone());
            out.push(n);
        }
    }
    out
}

/// While stably deduplicating notices, stores only the representatives to display for each
/// rule. This avoids allocating a second large Vec for millions of deduplicated notices;
/// `totals` still contains the actual distinct counts before the cap.
pub fn dedup_and_cap_by_rule<F>(
    notices: Vec<Notice>,
    cap_for_rule: F,
) -> (Vec<Notice>, HashMap<String, u32>)
where
    F: Fn(&str) -> usize,
{
    dedup_and_cap_by_rule_visit(notices, cap_for_rule, |_| {})
}

/// Same as [`dedup_and_cap_by_rule`]; `visit` sees every DISTINCT notice BEFORE the cap
/// discards it. On WASM, actual counts (the agency breakdown) are collected during this
/// pass without allocating a second full-size `Vec`.
pub fn dedup_and_cap_by_rule_visit<F, V>(
    mut notices: Vec<Notice>,
    cap_for_rule: F,
    mut visit: V,
) -> (Vec<Notice>, HashMap<String, u32>)
where
    F: Fn(&str) -> usize,
    V: FnMut(&Notice),
{
    // MUST BE STABLE: otherwise which of two equal-key findings becomes the keep-first
    // representative depends on input permutation and therefore on HashMap iteration.
    notices.sort_by(|a, b| notice_order_key(a).cmp(&notice_order_key(b)));

    let mut seen: FxHashSet<String> = FxHashSet::default();
    seen.reserve(notices.len());
    let mut kept = Vec::new();
    let mut totals: HashMap<String, u32> = HashMap::new();
    let mut key_buf = String::with_capacity(128);

    for notice in notices {
        let level = get_rule(&notice.rule_id)
            .map(|m| m.dedup_level)
            .unwrap_or(DedupLevel::Row);
        key_buf.clear();
        write_dedup_key(&mut key_buf, &notice, level);
        if !seen.insert(key_buf.clone()) {
            continue;
        }

        visit(&notice);
        let total = totals.entry(notice.rule_id.clone()).or_insert(0);
        *total += 1;
        if (*total as usize) <= cap_for_rule(&notice.rule_id) {
            kept.push(notice);
        }
    }

    (kept, totals)
}

// \x00 is used as the separator — GTFS IDs, file names, and field names cannot contain
// a null byte, preventing collisions caused by characters such as '|' in entity_id/file/field.
fn write_dedup_key(buf: &mut String, n: &Notice, level: DedupLevel) {
    const SEP: char = '\x00';
    match level {
        DedupLevel::Feed => buf.push_str(&n.rule_id),

        DedupLevel::File => {
            buf.push_str(&n.rule_id);
            buf.push(SEP);
            buf.push_str(n.file.as_deref().unwrap_or(""));
        }

        DedupLevel::Entity => {
            buf.push_str(&n.rule_id);
            buf.push(SEP);
            buf.push_str(n.entity_type.stable_name());
            buf.push(SEP);
            buf.push_str(n.entity_id.as_deref().unwrap_or(""));
        }

        DedupLevel::Row => match n.file.as_deref() {
            Some(f) => {
                buf.push_str(&n.rule_id);
                buf.push(SEP);
                buf.push_str(f);
                buf.push(SEP);
                use std::fmt::Write as _;
                let _ = write!(buf, "{}", n.line.unwrap_or(0));
            }
            None => {
                buf.push_str(&n.rule_id);
                buf.push_str("\x00id:");
                buf.push_str(&n.id);
            }
        },

        DedupLevel::Field => match n.file.as_deref() {
            Some(f) => {
                buf.push_str(&n.rule_id);
                buf.push(SEP);
                buf.push_str(f);
                buf.push(SEP);
                use std::fmt::Write as _;
                let _ = write!(buf, "{}", n.line.unwrap_or(0));
                buf.push(SEP);
                buf.push_str(n.field.as_deref().unwrap_or(""));
            }
            None => {
                buf.push_str(&n.rule_id);
                buf.push_str("\x00id:");
                buf.push_str(&n.id);
            }
        },
    }
}

// ── WP-10b: Root Cause / Symptom Resolution ──────────────────────────────────

struct SymptomResolution {
    /// Is notices[i] a symptom?
    is_symptom: Vec<bool>,
    /// rule_id → scope_key → [notice_idx]. The question "does this rule have a notice in
    /// this scope?" becomes a lookup instead of a scan. Three consumers share it:
    /// `resolve_symptoms`, `realized_dep_for_group`, and
    /// `closure_notice_indices_for_group` — all three previously fell into the same
    /// O(root × candidate) trap (#155).
    ///
    /// ⚠️ The flat `rule_id → [(idx, scope)]` view was REMOVED: once its last consumer
    /// moved here, it became dead data and broke the build under clippy `-D warnings`.
    rule_scope_buckets: HashMap<String, HashMap<Option<String>, Vec<usize>>>,
}

fn resolve_symptoms(notices: &[Notice]) -> SymptomResolution {
    // A SEPARATE view indexed by scope for the suppression phase.
    //
    // 🔴 The old version scanned ALL candidates for that rule for every root and compared
    // scopes one by one: O(root × candidate). On `mdb-2727` (3.3 MB, 101,621 trips) this
    // alone took 61 seconds and caused the feed to time out after 300 seconds in the full
    // catalog run — validation itself takes 390 ms (#155).
    // Worse, ADDING the `blocks` relationship multiplied the cost. If STM_003 → STM_015
    // were connected, this would be 203,242 roots × 101,621 candidates = 20.6 billion
    // iterations, turning the cascade fix into a performance disaster. Hence, index FIRST.
    let mut by_rule_scope: HashMap<String, HashMap<Option<String>, Vec<usize>>> = HashMap::new();
    for (i, n) in notices.iter().enumerate() {
        by_rule_scope
            .entry(n.rule_id.clone())
            .or_default()
            .entry(n.scope_key.clone())
            .or_default()
            .push(i);
    }

    let mut is_symptom = vec![false; notices.len()];
    for root in notices.iter() {
        if root.blocks.is_empty() {
            continue;
        }
        for blocked_rule in &root.blocks {
            let Some(by_scope) = by_rule_scope.get(blocked_rule.as_str()) else {
                continue;
            };
            match root.scope_key.as_deref() {
                // An unscoped root affects the entire feed and suppresses every scope of that rule.
                None => {
                    for idxs in by_scope.values() {
                        for ci in idxs {
                            is_symptom[*ci] = true;
                        }
                    }
                }
                // A scoped root → ONLY the same scope. This also used to be found by a full
                // scan; now it is a direct lookup. Behavior is identical: candidates with
                // a `None` scope are NOT suppressed by a scoped root.
                Some(scope) => {
                    if let Some(idxs) = by_scope.get(&Some(scope.to_string())) {
                        for ci in idxs {
                            is_symptom[*ci] = true;
                        }
                    }
                }
            }
        }
    }

    SymptomResolution {
        is_symptom,
        rule_scope_buckets: by_rule_scope,
    }
}

/// Returns the grouped publication penalty per rule for blocker-eligible notices and
/// the rule → contribution map.
///
/// On fixed-rail lines such as trams, all trip/stop errors from the same shape_id arise
/// from the same physical problem; counting every instance independently artificially
/// inflates pub_penalty. instance_multiplier (max 2×) accounts for this consistently
/// with the fix_effort calculation.
fn build_pub_penalty_map(notices: &[Notice]) -> (f64, HashMap<String, f64>) {
    let mut rule_data: HashMap<&str, (f64, u32)> = HashMap::new();
    for n in notices.iter().filter(|n| is_pub_relevant(n)) {
        let e = rule_data
            .entry(n.rule_id.as_str())
            .or_insert((n.severity.weight(), 0));
        e.1 += 1;
    }
    let mut per_rule: HashMap<String, f64> = HashMap::new();
    let mut total = 0.0_f64;
    for (id, &(w, cnt)) in &rule_data {
        let contrib = w * instance_multiplier(cnt);
        per_rule.insert(id.to_string(), contrib);
        total += contrib;
    }
    (total, per_rule)
}

/// The realized_dependent_count of a notice group: the number of blocks[] rules that
/// produced at least one notice whose scope_key matches any notice in the group.
fn realized_dep_for_group(
    group_indices: &[usize],
    notices: &[Notice],
    rule_scope_buckets: &HashMap<String, HashMap<Option<String>, Vec<usize>>>,
) -> u32 {
    let all_blocks: FxHashSet<&str> = group_indices
        .iter()
        .flat_map(|&i| notices[i].blocks.iter().map(String::as_str))
        .collect();

    // Collect the group's scope set ONCE; it used to be rebuilt during every candidate scan.
    // If even one member has a `None` scope, the group affects the entire feed.
    let mut group_scopes: FxHashSet<&str> = FxHashSet::default();
    let mut group_has_unscoped = false;
    for &gi in group_indices {
        match notices[gi].scope_key.as_deref() {
            None => group_has_unscoped = true,
            Some(s) => {
                group_scopes.insert(s);
            }
        }
    }

    all_blocks
        .iter()
        .filter(|&&blocked_rule| {
            let Some(by_scope) = rule_scope_buckets.get(blocked_rule) else {
                return false;
            };
            // An unscoped member → ANY notice for that rule is sufficient.
            if group_has_unscoped {
                return by_scope.values().any(|v| !v.is_empty());
            }
            // Otherwise, search only the group's scopes — a lookup instead of a scan.
            group_scopes.iter().any(|s| {
                by_scope
                    .get(&Some((*s).to_string()))
                    .is_some_and(|v| !v.is_empty())
            })
        })
        .count() as u32
}

/// Returns all notice indices that will close when an R9 item is selected (transitive closure).
///
/// The initial set is `group_indices` (root-cause notices). Rules in each notice's
/// `blocks[]` list are looked up through `rule_scope_buckets`; blocked notices with
/// a matching scope are added to the set, and this repeats (via BFS) until a fixed point.
///
/// Scope rule (same as resolve_symptoms):
/// - root.scope_key == None → affects the entire feed; every scope_key matches.
/// - root.scope_key == Some(x) → only a candidate with the same Some(x) matches.
///
/// The returned set contains both root notices and downstream notices that are closed.
fn closure_notice_indices_for_group(
    group_indices: &[usize],
    notices: &[Notice],
    resolution: &SymptomResolution,
) -> FxHashSet<usize> {
    let mut closure: FxHashSet<usize> = FxHashSet::default();
    let mut queue: Vec<usize> = Vec::new();

    // Initial set: root-cause notices
    for &i in group_indices {
        if closure.insert(i) {
            queue.push(i);
        }
    }

    // BFS: expand each notice's blocks[] chain in a scope-aware manner
    let mut head = 0;
    while head < queue.len() {
        let idx = queue[head];
        head += 1;
        let root_notice = &notices[idx];
        let root_scope = root_notice.scope_key.as_deref();

        for blocked_rule in &root_notice.blocks {
            // Scope-indexed view — this was the THIRD place doing a full scan of the flat
            // list (#155). BFS visited every candidate for that rule for every queue member;
            // because a group could contain 203,242 notices, the cost reached O(group × candidate).
            if let Some(by_scope) = resolution.rule_scope_buckets.get(blocked_rule) {
                // An unscoped root expands all scopes; a scoped root expands only its own scope.
                let selected: Vec<&Vec<usize>> = match root_scope {
                    None => by_scope.values().collect(),
                    Some(sc) => by_scope.get(&Some(sc.to_string())).into_iter().collect(),
                };
                for idxs in selected {
                    for &ci in idxs {
                        if closure.insert(ci) {
                            queue.push(ci);
                        }
                    }
                }
            }
        }
    }

    closure
}

/// Calculates the grouped publication penalty of notices in the closure set.
///
/// Uses the same hyperbolic-decay + instance_multiplier logic as `build_pub_penalty_map`;
/// the only difference is that it processes only notices in `closure_indices`, not all
/// notices. Instance counts are collected separately for each rule_id in the closure set,
/// so rules with multiple closure notices are grouped correctly.
fn build_closure_pub_penalty(closure_indices: &FxHashSet<usize>, notices: &[Notice]) -> f64 {
    let mut rule_data: HashMap<&str, (f64, u32)> = HashMap::new();
    for &i in closure_indices {
        let n = &notices[i];
        if !is_pub_relevant(n) {
            continue;
        }
        let e = rule_data
            .entry(n.rule_id.as_str())
            .or_insert((n.severity.weight(), 0));
        e.1 += 1;
    }
    let mut total = 0.0_f64;
    for &(w, cnt) in rule_data.values() {
        total += w * instance_multiplier(cnt);
    }
    total
}

// ── WP-10c: R9 Priority Score + Bucket Ordering ──────────────────────────────

fn instance_multiplier(count: u32) -> f64 {
    if count <= 5 {
        1.0
    } else if count <= 50 {
        1.5
    } else {
        2.0
    }
}

fn compute_priority_score(
    severity: Severity,
    realized_dep: u32,
    affected: u32,
    fix_effort: f64,
) -> f64 {
    let sw = severity.weight();
    let raw = sw * (1.0 + realized_dep as f64) * (1.0 + affected as f64).log2() / fix_effort;
    (raw * 100.0).round() / 100.0
}

fn r9_labels(
    severity: Severity,
    rule_class: RuleClass,
    realized_dep: u32,
    fix_effort: f64,
    affected: u32,
) -> Vec<R9Label> {
    let mut labels = vec![];

    // Aligned with R1: only Spec+Kritik is a real publication blocker. Interop at every
    // severity is a consumer-compatibility signal (Interop), not a publication blocker.
    match (severity, rule_class) {
        (Severity::Kritik, RuleClass::Spec) => {
            labels.push(R9Label::Blocker);
        }
        (_, RuleClass::Interop) => {
            labels.push(R9Label::Interop);
        }
        (_, RuleClass::Analytics) => {
            labels.push(R9Label::Analytics);
        }
        _ => {}
    }
    if realized_dep > 2 {
        labels.push(R9Label::Propagation);
    }
    // Quick Win: low effort, any severity except Bilgi
    if fix_effort <= 1.5 && !matches!(severity, Severity::Bilgi) {
        labels.push(R9Label::QuickWin);
    }
    // Quality Priority: Quality class, Orta or higher
    if matches!(rule_class, RuleClass::Quality)
        && matches!(
            severity,
            Severity::Kritik | Severity::Yuksek | Severity::Orta
        )
    {
        labels.push(R9Label::Quality);
    }
    // Widespread: more than 20 affected instances
    if affected > 20 {
        labels.push(R9Label::Widespread);
    }
    // Single: exactly 1 instance — easy to locate
    if affected == 1 {
        labels.push(R9Label::Single);
    }
    // Hard: high-effort fix
    if fix_effort >= 4.0 {
        labels.push(R9Label::Hard);
    }
    labels
}

fn bucket_rank(labels: &[R9Label]) -> u8 {
    if labels.contains(&R9Label::Blocker) {
        0
    } else if labels.contains(&R9Label::Interop) {
        2
    } else if labels.contains(&R9Label::Propagation) {
        3
    } else if labels.contains(&R9Label::QuickWin) {
        4
    } else if labels.contains(&R9Label::Quality) {
        5
    } else if labels.contains(&R9Label::Widespread) {
        6
    } else {
        7
    }
}

fn build_r9(notices: &[Notice], resolution: &SymptomResolution) -> R9Report {
    // Per-rule penalty inputs of the R5 overall score (same model as class_score)
    let r5_rules = r5_rule_penalties(notices);
    let cur_overall = r5_overall(&r5_rules, &HashMap::new());

    // Total publication penalty (reference baseline for pub_score_delta)
    let (total_pub_penalty, _) = build_pub_penalty_map(notices);

    // Only root-cause notices enter R9; symptoms are hidden
    let mut groups: HashMap<&str, Vec<usize>> = HashMap::new();
    for (i, n) in notices.iter().enumerate() {
        // GTFS-JP future-only feeds use CAL_015 as a zero-penalty publication
        // notice. It remains visible in R2/R5 but is not an action item in R9.
        if n.rule_id == "CAL_015" && matches!(n.severity, Severity::Bilgi) {
            continue;
        }
        if !resolution.is_symptom[i] {
            groups.entry(n.rule_id.as_str()).or_default().push(i);
        }
    }

    let mut items: Vec<R9Item> = groups
        .into_iter()
        .map(|(rule_id, indices)| {
            let first = &notices[indices[0]];
            let severity = first.severity;
            let rule_class = get_rule(rule_id)
                .map(|m| m.rule_class)
                .unwrap_or(first.rule_class);
            let base_effort = get_rule(rule_id)
                .map(|m| m.base_effort)
                .unwrap_or(first.base_effort);
            let affected = indices.len() as u32;
            let multiplier = instance_multiplier(affected);
            let fix_effort = base_effort as f64 * multiplier;
            let realized_dep =
                realized_dep_for_group(&indices, notices, &resolution.rule_scope_buckets);
            let mut labels = r9_labels(severity, rule_class, realized_dep, fix_effort, affected);
            let priority_score =
                compute_priority_score(severity, realized_dep, affected, fix_effort);

            // Closure set: all notices that close when this R9 item is selected (root + downstream)
            let closure = closure_notice_indices_for_group(&indices, notices, resolution);

            // R5 overall score improvement: R5 recomputed without the closure set, with the
            // same per-rule saturating penalty as class_score. Because instance_multiplier
            // saturates, the result does not depend on how many samples a capped run kept.
            let k = 50.0_f64;
            let score_delta = {
                let mut removed: HashMap<&str, u32> = HashMap::new();
                for &ci in &closure {
                    *removed.entry(notices[ci].rule_id.as_str()).or_default() += 1;
                }
                round1(r5_overall(&r5_rules, &removed) - cur_overall)
            };

            // R5 publication score improvement — use the grouped hyperbolic-decay penalty of
            // pub-relevant notices in the closure set (consistent with build_pub_penalty_map).
            let closure_pub_penalty = build_closure_pub_penalty(&closure, notices);
            let cur_pub = k * 100.0 / (k + total_pub_penalty);
            let new_pub = k * 100.0 / (k + (total_pub_penalty - closure_pub_penalty).max(0.0));
            let pub_score_delta = round1(new_pub - cur_pub);

            // High Impact: score contribution is above a significant threshold
            if score_delta >= 5.0 || pub_score_delta >= 3.0 {
                labels.push(R9Label::HighImpact);
            }

            R9Item {
                rule_id: rule_id.to_string(),
                labels,
                priority_score,
                score_delta,
                pub_score_delta,
                affected_instance_count: affected,
                realized_dependent_count: realized_dep,
                base_effort,
                fix_effort,
                notice_ids: indices.iter().map(|&i| notices[i].id.clone()).collect(),
            }
        })
        .collect();

    // Primary: bucket rank; secondary: descending priority_score;
    // tertiary: rule_id (unique in R9) — guarantees a deterministic order for equal scores,
    // independent of HashMap iteration order.
    items.sort_by(|a, b| {
        bucket_rank(&a.labels)
            .cmp(&bucket_rank(&b.labels))
            .then(
                b.priority_score
                    .partial_cmp(&a.priority_score)
                    .unwrap_or(std::cmp::Ordering::Equal),
            )
            .then_with(|| a.rule_id.cmp(&b.rule_id))
    });

    R9Report { items }
}

// ── WP-10d: Report Projections (R1–R8) ───────────────────────────────────────

fn build_report_set(
    notices: &[Notice],
    resolution: &SymptomResolution,
    coverage_complete: bool,
) -> ReportSet {
    // Per-report timer: "K7 is slow" is not actionable by itself; which report is slow is.
    // Added to locate where the total 78 seconds went in mdb-2727 (#155), and permanent —
    // the same question will matter on the next pathological feed.
    ReportSet {
        r1: {
            let _t = crate::timing::Timer::start("K7::r1");
            build_r1(notices, coverage_complete)
        },
        r2: {
            let _t = crate::timing::Timer::start("K7::r2");
            build_r2(notices)
        },
        r3: {
            let _t = crate::timing::Timer::start("K7::r3");
            build_r3(notices)
        },
        r4: {
            let _t = crate::timing::Timer::start("K7::r4");
            build_r4(notices)
        },
        r5: {
            let _t = crate::timing::Timer::start("K7::r5");
            build_r5(notices)
        },
        r7: {
            let _t = crate::timing::Timer::start("K7::r7");
            build_r7(notices)
        },
        r8: {
            let _t = crate::timing::Timer::start("K7::r8");
            build_r8(notices)
        },
        r9: {
            let _t = crate::timing::Timer::start("K7::r9");
            build_r9(notices, resolution)
        },
    }
}

/// R1 publication gate.
///
/// 🔴 **`publishable` is NO LONGER `blockers.is_empty()`** (issue #133). The old version
/// returned `true` even when a required file could not be read and its rules never ran:
/// measured in `mdb-1903`, making `stops.txt` unreadable skipped 29 checks while the feed
/// remained `publishable=true`. Absence of findings is not the same as absence of evidence.
///
/// ⚠️ `coverage_complete` falls only when a required file is missing. A malformed
/// `attributions.txt` also makes the run PARTIAL (measured), but does not lower the gate —
/// blocking a valid feed from publication is much worse than treating a norm as advice
/// (`PTH_017` asymmetry).
fn build_r1(notices: &[Notice], coverage_complete: bool) -> R1Report {
    let blockers: Vec<String> = notices
        .iter()
        .filter(|n| is_pub_relevant(n))
        .map(|n| n.id.clone())
        .collect();
    R1Report {
        publishable: blockers.is_empty() && coverage_complete,
        coverage_complete,
        blocker_notice_ids: blockers,
    }
}

fn build_r2(notices: &[Notice]) -> R2Report {
    R2Report {
        items: notices
            .iter()
            .map(|n| ReportItem {
                notice_id: n.id.clone(),
                report_id: ReportId::R2,
                display_label: n.rule_id.clone(),
                is_primary: true,
            })
            .collect(),
    }
}

fn build_r3(notices: &[Notice]) -> R3Report {
    R3Report {
        items: notices
            .iter()
            .filter(|n| n.rule_class == RuleClass::Analytics)
            .map(|n| ReportItem {
                notice_id: n.id.clone(),
                report_id: ReportId::R3,
                display_label: n.rule_id.clone(),
                is_primary: true,
            })
            .collect(),
    }
}

fn build_r4(notices: &[Notice]) -> R4Report {
    R4Report {
        items: notices
            .iter()
            .filter(|n| in_report(&n.rule_id, ReportId::R4))
            .map(|n| ReportItem {
                notice_id: n.id.clone(),
                report_id: ReportId::R4,
                display_label: r4_display_label(&n.rule_id),
                is_primary: true,
            })
            .collect(),
    }
}

/// Publication-blocking notice: only `Spec + Kritik` (the official GTFS spec gate).
/// This determines the R1 publication decision and pub_score/pub_score_delta.
/// Because `rule_class==Spec` ⟺ `authority==GtfsSpec` is guaranteed by the registry gate
/// (`spec_class_requires_gtfs_spec_authority`), a runtime authority check is unnecessary.
/// Interop/Quality/Analytics are NEVER publication blockers — consumer/interop readiness is
/// reported in R8 + interop_score.
fn is_pub_relevant(n: &Notice) -> bool {
    matches!(n.severity, Severity::Kritik) && n.rule_class == RuleClass::Spec
}

fn build_r5(notices: &[Notice]) -> R5Report {
    let spec_score = class_score(notices, RuleClass::Spec);
    let interop_score = class_score(notices, RuleClass::Interop);
    let quality_score = class_score(notices, RuleClass::Quality);
    let analytics_score = class_score(notices, RuleClass::Analytics);
    // Weighted average: SPEC 40%, INTEROP 30%, QUALITY 20%, ANALYTICS 10%
    let overall =
        0.4 * spec_score + 0.3 * interop_score + 0.2 * quality_score + 0.1 * analytics_score;
    // Publication score: penalty grouped per rule (consistent with build_pub_penalty_map)
    let (pub_penalty, _) = build_pub_penalty_map(notices);
    let pub_score = 100.0 * 50.0 / (50.0 + pub_penalty);
    R5Report {
        score: round1(overall),
        pub_score: round1(pub_score),
        spec_score: round1(spec_score),
        interop_score: round1(interop_score),
        quality_score: round1(quality_score),
        analytics_score: round1(analytics_score),
    }
}

fn notice_score_weight(notice: &Notice, has_opr011: bool, has_stm033: bool) -> f64 {
    if notice.rule_id == "TRP_026" && has_opr011 {
        return 0.0;
    }
    if notice.rule_id == "OPR_006" && has_stm033 {
        return 0.0;
    }
    get_rule(&notice.rule_id)
        .map(|meta| meta.score_weight)
        .unwrap_or(1.0)
}

/// Per-rule input of the R5 class score: class, `severity × score_weight` of the first
/// notice (as in class_score) and notice count.
struct R5RulePenalty {
    class: RuleClass,
    weight: f64,
    count: u32,
}

fn r5_rule_penalties(notices: &[Notice]) -> HashMap<&str, R5RulePenalty> {
    let mut rules: HashMap<&str, R5RulePenalty> = HashMap::new();
    for n in notices {
        rules
            .entry(n.rule_id.as_str())
            .or_insert_with(|| R5RulePenalty {
                class: n.rule_class,
                weight: n.severity.weight()
                    * get_rule(&n.rule_id).map(|meta| meta.score_weight).unwrap_or(1.0),
                count: 0,
            })
            .count += 1;
    }
    rules
}

/// R5 overall score after `removed` notices per rule are gone. Equals build_r5's
/// unrounded `score` when nothing is removed; the TRP_026/OPR_006 zero weights follow
/// whether OPR_011/STM_033 remain.
fn r5_overall(rules: &HashMap<&str, R5RulePenalty>, removed: &HashMap<&str, u32>) -> f64 {
    let remaining = |id: &str| {
        rules
            .get(id)
            .map(|r| r.count.saturating_sub(removed.get(id).copied().unwrap_or(0)))
            .unwrap_or(0)
    };
    let has_opr011 = remaining("OPR_011") > 0;
    let has_stm033 = remaining("STM_033") > 0;
    let mut penalty: HashMap<RuleClass, f64> = HashMap::new();
    for (&id, rule) in rules {
        let count = remaining(id);
        if count == 0 {
            continue;
        }
        let zeroed = (id == "TRP_026" && has_opr011) || (id == "OPR_006" && has_stm033);
        let weight = if zeroed { 0.0 } else { rule.weight };
        *penalty.entry(rule.class).or_default() += weight * instance_multiplier(count);
    }
    let score = |class| 100.0 * 50.0 / (50.0 + penalty.get(&class).copied().unwrap_or(0.0));
    0.4 * score(RuleClass::Spec)
        + 0.3 * score(RuleClass::Interop)
        + 0.2 * score(RuleClass::Quality)
        + 0.1 * score(RuleClass::Analytics)
}

/// Hyperbolic decay: score = 100 × K / (K + penalty), K=50.
/// instance_multiplier (max 2×) is applied per rule — one high-count rule cannot collapse
/// the entire score. Consistent with build_pub_penalty_map.
fn class_score(notices: &[Notice], class: RuleClass) -> f64 {
    let mut per_rule: HashMap<&str, (f64, u32)> = HashMap::new();
    let has_opr011 = notices.iter().any(|n| n.rule_id == "OPR_011");
    let has_stm033 = notices.iter().any(|n| n.rule_id == "STM_033");
    for n in notices.iter().filter(|n| n.rule_class == class) {
        let e = per_rule.entry(n.rule_id.as_str()).or_insert_with(|| (
                n.severity.weight()
                    * notice_score_weight(n, has_opr011, has_stm033),
                0,
            ));
        e.1 += 1;
    }
    let penalty: f64 = per_rule
        .values()
        .map(|(w, cnt)| w * instance_multiplier(*cnt))
        .sum();
    100.0 * 50.0 / (50.0 + penalty)
}

fn round1(v: f64) -> f64 {
    (v * 10.0).round() / 10.0
}

fn build_r7(notices: &[Notice]) -> R7Report {
    R7Report {
        items: notices
            .iter()
            .filter(|n| in_report(&n.rule_id, ReportId::R7))
            .map(|n| ReportItem {
                notice_id: n.id.clone(),
                report_id: ReportId::R7,
                display_label: n.rule_id.clone(),
                is_primary: true,
            })
            .collect(),
    }
}

fn build_r8(notices: &[Notice]) -> R8Report {
    R8Report {
        items: notices
            .iter()
            .filter(|n| n.rule_class == RuleClass::Interop)
            .map(|n| ReportItem {
                notice_id: n.id.clone(),
                report_id: ReportId::R8,
                display_label: n.rule_id.clone(),
                is_primary: true,
            })
            .collect(),
    }
}

fn in_report(rule_id: &str, report_id: ReportId) -> bool {
    get_rule(rule_id)
        .map(|m| m.report_views.contains(&report_id))
        .unwrap_or(false)
}

fn r4_display_label(rule_id: &str) -> String {
    match rule_id {
        "SHP_014" => "GEO_010".to_string(),
        "SHP_016" => "GEO_011".to_string(),
        _ => rule_id.to_string(),
    }
}

// ── Metrics ───────────────────────────────────────────────────────────────────

fn build_metrics(
    notices: &[Notice],
    records: &EntityRecords,
    derived: &DerivedData,
    file_stats: Vec<FileInfo>,
    coverage_complete: bool,
) -> FeedMetrics {
    let shape_count = records
        .shapes
        .iter()
        .map(|s| records.shape_interns.id(s))
        .collect::<FxHashSet<_>>()
        .len() as u32;

    let all_dates: FxHashSet<u32> = derived
        .calendar_bitmap
        .active_dates
        .values()
        .flat_map(|dates| dates.iter().copied())
        .collect();
    let active_service_days = all_dates.len() as u32;

    // Actual service coverage range (YYYYMMDD min/max). all_dates values are already YYYYMMDD.
    let service_start_date = all_dates.iter().min().copied();
    let service_end_date = all_dates.iter().max().copied();

    // Feed validity window (feed_info.txt) — (y,m,d) tuple → YYYYMMDD u32.
    let yyyymmdd = |(y, m, d): (u32, u32, u32)| y * 10000 + m * 100 + d;
    let feed_info = records.feed_info.first();
    let feed_start_date = feed_info.and_then(|fi| fi.feed_start_date).map(yyyymmdd);
    let feed_end_date = feed_info.and_then(|fi| fi.feed_end_date).map(yyyymmdd);

    // Unique, non-empty trip_id → service_id; duplicate/empty rows must not affect the metric.
    let ti_k7 = &records.trip_interns;
    let mut unique_trips: HashMap<&str, &str> = HashMap::with_capacity(records.trips.len());
    for t in &records.trips {
        if !t.trip_id.is_empty() {
            unique_trips
                .entry(t.trip_id.as_str())
                .or_insert(ti_k7.service_id(t));
        }
    }

    // Calculate daily runs for frequency-based trips (floor((end-start)/headway))
    let mut freq_runs: HashMap<&str, u32> = HashMap::new();
    for freq in &records.frequencies {
        if freq.trip_id.is_empty() {
            continue;
        }
        let (Some(start), Some(end), Some(hw)) =
            (freq.start_time, freq.end_time, freq.headway_secs)
        else {
            continue;
        };
        if hw == 0 {
            continue;
        }
        let start_s = start.0 * 3600 + start.1 * 60 + start.2;
        let end_s = end.0 * 3600 + end.1 * 60 + end.2;
        if end_s <= start_s {
            continue;
        }
        *freq_runs.entry(freq.trip_id.as_str()).or_insert(0) += (end_s - start_s) / hw;
    }

    // trip_count: frequency trips are expanded by their daily run count
    let trip_count: u32 = unique_trips
        .keys()
        .map(|&tid| freq_runs.get(tid).copied().unwrap_or(1).max(1))
        .sum();

    // Count each unique trip for its active service days, including the frequency multiplier.
    let total_trip_days: usize = unique_trips
        .iter()
        .map(|(&tid, &svc)| {
            let active = derived
                .calendar_bitmap
                .active_dates
                .get(svc)
                .map(|dates| dates.len())
                .unwrap_or(0);
            let multiplier = freq_runs.get(tid).copied().unwrap_or(1).max(1) as usize;
            active * multiplier
        })
        .sum();
    let avg_daily_trips = if active_service_days > 0 {
        total_trip_days as f64 / active_service_days as f64
    } else {
        0.0
    };

    // K7 fallback only serves direct callers that did not carry K2's signal.
    // The production pipeline supplies `records.is_gtfs_jp`; keep the fallback
    // semantically identical to K2 so reporting cannot disagree with validation.
    let is_gtfs_jp = records.is_gtfs_jp.unwrap_or_else(|| {
        detect_gtfs_jp(
            records,
            file_stats
                .iter()
                .any(|f| GTFS_JP_FILES.contains(&f.name.as_str())),
        )
    });
    // The profile is user-selected metadata; it is intentionally independent
    // from whether JP validation ran.
    let gtfs_jp_profile = match records.gtfs_jp_profile {
        gtfs_config::GtfsJpProfile::Auto if is_gtfs_jp => Some("auto".to_string()),
        gtfs_config::GtfsJpProfile::Auto => None,
        profile => Some(profile.as_str().to_string()),
    };

    FeedMetrics {
        coverage_complete,
        stop_count: records.stops.len() as u32,
        route_count: records.routes.len() as u32,
        trip_count,
        shape_count,
        active_service_days,
        avg_daily_trips,
        feed_start_date,
        feed_end_date,
        service_start_date,
        service_end_date,
        spec_notice_count: notices
            .iter()
            .filter(|n| n.rule_class == RuleClass::Spec)
            .count() as u32,
        interop_notice_count: notices
            .iter()
            .filter(|n| n.rule_class == RuleClass::Interop)
            .count() as u32,
        quality_notice_count: notices
            .iter()
            .filter(|n| n.rule_class == RuleClass::Quality)
            .count() as u32,
        analytics_notice_count: notices
            .iter()
            .filter(|n| n.rule_class == RuleClass::Analytics)
            .count() as u32,
        overall_score: 0.0, // updated with r5.score in the report() function
        file_stats,
        is_gtfs_jp,
        gtfs_jp_profile,
    }
}

// ── WP-10e: Tests ─────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use gtfs_core::EntityType;

    fn notice(id: &str, rule_id: &str, severity: Severity, class: RuleClass) -> Notice {
        Notice {
            id: id.to_string(),
            rule_id: rule_id.to_string(),
            severity,
            rule_class: class,
            entity_type: EntityType::Feed,
            entity_id: None,
            scope_key: None,
            file: None,
            line: None,
            field: None,
            observed_value: None,
            expected_value: None,
            details: None,
            whitespace_derived: false,
            whitespace_candidate: false,
            title: String::new().into(),
            message: String::new(),
            remediation: String::new().into(),
            blocks: vec![],
            base_effort: 1,
            service_id: None,
            agency_distribution: None,
        }
    }

    #[test]
    fn conditional_score_weights_only_zero_dependent_details() {
        let trp026 = notice("trp", "TRP_026", Severity::Orta, RuleClass::Analytics);
        let opr006 = notice("opr", "OPR_006", Severity::Yuksek, RuleClass::Analytics);

        assert_eq!(notice_score_weight(&trp026, false, false), 1.0);
        assert_eq!(notice_score_weight(&trp026, false, true), 1.0);
        assert_eq!(notice_score_weight(&trp026, true, false), 0.0);
        assert_eq!(notice_score_weight(&opr006, false, false), 1.0);
        assert_eq!(notice_score_weight(&opr006, true, false), 1.0);
        assert_eq!(notice_score_weight(&opr006, false, true), 0.0);
    }

    // ── Dedup ─────────────────────────────────────────────────────────────────

    // ARC_001 → Feed dedup
    #[test]
    fn dedup_feed_level_keeps_first() {
        let mut n1 = notice("n1", "ARC_001", Severity::Kritik, RuleClass::Spec);
        let mut n2 = notice("n2", "ARC_001", Severity::Kritik, RuleClass::Spec);
        n1.file = Some("agency.txt".into());
        n2.file = Some("stops.txt".into());
        assert_eq!(dedup(vec![n1, n2]).len(), 1);
    }

    // ARC_002 → File dedup: different files → two notices
    #[test]
    fn dedup_file_level_different_files() {
        let mut n1 = notice("n1", "ARC_002", Severity::Kritik, RuleClass::Spec);
        let mut n2 = notice("n2", "ARC_002", Severity::Kritik, RuleClass::Spec);
        n1.file = Some("agency.txt".into());
        n2.file = Some("stops.txt".into());
        assert_eq!(dedup(vec![n1, n2]).len(), 2);
    }

    // ARC_002 → File dedup: same file → one notice
    #[test]
    fn dedup_file_level_same_file() {
        let mut n1 = notice("n1", "ARC_002", Severity::Kritik, RuleClass::Spec);
        let mut n2 = notice("n2", "ARC_002", Severity::Kritik, RuleClass::Spec);
        n1.file = Some("stops.txt".into());
        n2.file = Some("stops.txt".into());
        assert_eq!(dedup(vec![n1, n2]).len(), 1);
    }

    // AGN_002 → Entity dedup
    #[test]
    fn dedup_entity_level_same_entity() {
        let mut n1 = notice("n1", "AGN_002", Severity::Kritik, RuleClass::Spec);
        let mut n2 = notice("n2", "AGN_002", Severity::Kritik, RuleClass::Spec);
        n1.entity_type = EntityType::Agency;
        n2.entity_type = EntityType::Agency;
        n1.entity_id = Some("A1".into());
        n2.entity_id = Some("A1".into());
        assert_eq!(dedup(vec![n1, n2]).len(), 1);
    }

    #[test]
    fn dedup_entity_level_different_entities() {
        let mut n1 = notice("n1", "AGN_002", Severity::Kritik, RuleClass::Spec);
        let mut n2 = notice("n2", "AGN_002", Severity::Kritik, RuleClass::Spec);
        n1.entity_id = Some("A1".into());
        n2.entity_id = Some("A2".into());
        assert_eq!(dedup(vec![n1, n2]).len(), 2);
    }

    // ARC_018 → Row dedup
    #[test]
    fn dedup_row_level_same_row() {
        let mut n1 = notice("n1", "ARC_018", Severity::Yuksek, RuleClass::Spec);
        let mut n2 = notice("n2", "ARC_018", Severity::Yuksek, RuleClass::Spec);
        n1.file = Some("stops.txt".into());
        n1.line = Some(5);
        n2.file = Some("stops.txt".into());
        n2.line = Some(5);
        assert_eq!(dedup(vec![n1, n2]).len(), 1);
    }

    #[test]
    fn dedup_row_level_different_rows() {
        let mut n1 = notice("n1", "ARC_018", Severity::Yuksek, RuleClass::Spec);
        let mut n2 = notice("n2", "ARC_018", Severity::Yuksek, RuleClass::Spec);
        n1.file = Some("stops.txt".into());
        n1.line = Some(5);
        n2.file = Some("stops.txt".into());
        n2.line = Some(6);
        assert_eq!(dedup(vec![n1, n2]).len(), 2);
    }

    #[test]
    fn dedup_and_cap_keeps_real_totals_without_materializing_all_notices() {
        let mut notices = Vec::new();
        for line in [4, 2, 3, 2, 1] {
            let mut n = notice(
                &format!("n{line}"),
                "ARC_018",
                Severity::Yuksek,
                RuleClass::Spec,
            );
            n.file = Some("stops.txt".into());
            n.line = Some(line);
            notices.push(n);
        }

        let (kept, totals) = dedup_and_cap_by_rule(notices, |_| 2);

        assert_eq!(totals.get("ARC_018"), Some(&4));
        assert_eq!(kept.len(), 2);
        assert_eq!(
            kept.iter().map(|n| n.line).collect::<Vec<_>>(),
            vec![Some(1), Some(2)]
        );
    }

    // ARC_014 → Field dedup
    #[test]
    fn dedup_field_level_same_field() {
        let mut n1 = notice("n1", "ARC_014", Severity::Orta, RuleClass::Quality);
        let mut n2 = notice("n2", "ARC_014", Severity::Orta, RuleClass::Quality);
        n1.file = Some("stops.txt".into());
        n1.line = Some(3);
        n1.field = Some("stop_name".into());
        n2.file = Some("stops.txt".into());
        n2.line = Some(3);
        n2.field = Some("stop_name".into());
        assert_eq!(dedup(vec![n1, n2]).len(), 1);
    }

    #[test]
    fn dedup_field_level_different_fields() {
        let mut n1 = notice("n1", "ARC_014", Severity::Orta, RuleClass::Quality);
        let mut n2 = notice("n2", "ARC_014", Severity::Orta, RuleClass::Quality);
        n1.file = Some("stops.txt".into());
        n1.line = Some(3);
        n1.field = Some("stop_name".into());
        n2.file = Some("stops.txt".into());
        n2.line = Some(3);
        n2.field = Some("stop_lat".into());
        assert_eq!(dedup(vec![n1, n2]).len(), 2);
    }

    #[test]
    fn dedup_unknown_rule_falls_back_to_row() {
        let mut n1 = notice("n1", "UNKNOWN_999", Severity::Orta, RuleClass::Quality);
        let mut n2 = notice("n2", "UNKNOWN_999", Severity::Orta, RuleClass::Quality);
        n1.file = Some("stops.txt".into());
        n1.line = Some(1);
        n2.file = Some("stops.txt".into());
        n2.line = Some(1);
        assert_eq!(dedup(vec![n1, n2]).len(), 1);
    }

    // Feed dedup: only rule_id matters even when entity_id is populated
    #[test]
    fn dedup_feed_ignores_entity_id() {
        let mut n1 = notice("n1", "ARC_001", Severity::Kritik, RuleClass::Spec);
        let mut n2 = notice("n2", "ARC_001", Severity::Kritik, RuleClass::Spec);
        n1.entity_id = Some("A1".into());
        n2.entity_id = Some("A2".into()); // different entity_id, but Feed dedup → one notice
        assert_eq!(dedup(vec![n1, n2]).len(), 1);
    }

    // Entity dedup: same entity_id but different entity_type → two separate notices
    #[test]
    fn dedup_entity_different_types_not_merged() {
        let mut n1 = notice("n1", "AGN_002", Severity::Kritik, RuleClass::Spec);
        let mut n2 = notice("n2", "AGN_002", Severity::Kritik, RuleClass::Spec);
        n1.entity_type = EntityType::Agency;
        n1.entity_id = Some("X".into());
        n2.entity_type = EntityType::Stop;
        n2.entity_id = Some("X".into()); // same ID, different type
        assert_eq!(dedup(vec![n1, n2]).len(), 2);
    }

    // Row dedup: file=None → must not deduplicate; each notice remains distinct
    #[test]
    fn dedup_row_none_file_not_merged() {
        let n1 = notice("n1", "ARC_018", Severity::Yuksek, RuleClass::Spec);
        let n2 = notice("n2", "ARC_018", Severity::Yuksek, RuleClass::Spec);
        // file=None, line=None → unique key
        assert_eq!(
            dedup(vec![n1, n2]).len(),
            2,
            "file=None → dedup yapılmamalı"
        );
    }

    // ── Symptom resolution ────────────────────────────────────────────────────

    #[test]
    fn symptom_resolution_marks_blocked_rule_with_same_scope() {
        let mut root = notice("root", "STP_001", Severity::Kritik, RuleClass::Spec);
        root.scope_key = Some("S1".into());
        root.blocks = vec!["GEO_002".to_string()];

        let mut symptom = notice("sym", "GEO_002", Severity::Orta, RuleClass::Analytics);
        symptom.scope_key = Some("S1".into());

        let resolution = resolve_symptoms(&[root, symptom]);
        assert!(!resolution.is_symptom[0], "kök neden semptom olmamalı");
        assert!(
            resolution.is_symptom[1],
            "eşleşen scope_key ile GEO_002 semptom olmalı"
        );
    }

    #[test]
    fn symptom_resolution_different_scope_not_marked() {
        let mut root = notice("root", "STP_001", Severity::Kritik, RuleClass::Spec);
        root.scope_key = Some("S1".into());
        root.blocks = vec!["GEO_002".to_string()];

        let mut other = notice("other", "GEO_002", Severity::Orta, RuleClass::Analytics);
        other.scope_key = Some("S2".into()); // different scope

        let resolution = resolve_symptoms(&[root, other]);
        assert!(
            !resolution.is_symptom[1],
            "farklı scope_key → semptom değil"
        );
    }

    #[test]
    fn symptom_resolution_no_blocks_nothing_marked() {
        let n1 = notice("n1", "ARC_001", Severity::Kritik, RuleClass::Spec);
        let n2 = notice("n2", "ARC_008", Severity::Kritik, RuleClass::Spec);
        let resolution = resolve_symptoms(&[n1, n2]);
        assert!(resolution.is_symptom.iter().all(|&s| !s));
    }

    // An unscoped root must mark an unscoped symptom
    #[test]
    fn symptom_resolution_none_scope_root_marks_none_scope_symptom() {
        let mut root = notice("root", "ARC_004", Severity::Kritik, RuleClass::Spec);
        root.scope_key = None;
        root.blocks = vec!["DQ_005".to_string()];

        let symptom = notice("sym", "DQ_005", Severity::Yuksek, RuleClass::Interop);
        // symptom.scope_key = None (default)

        let resolution = resolve_symptoms(&[root, symptom]);
        assert!(resolution.is_symptom[1]);
    }

    // An unscoped root must also mark a Some(x)-scoped symptom (feed-level effect)
    #[test]
    fn symptom_resolution_none_scope_root_marks_entity_scope_symptom() {
        let mut root = notice("root", "ARC_004", Severity::Kritik, RuleClass::Spec);
        root.scope_key = None;
        root.blocks = vec!["TRP_003".to_string()];

        let mut symptom = notice("sym", "TRP_003", Severity::Kritik, RuleClass::Spec);
        symptom.scope_key = Some("trip_1".into());

        let resolution = resolve_symptoms(&[root, symptom]);
        assert!(
            resolution.is_symptom[1],
            "None root → entity-level semptom da bastırılmalı"
        );
    }

    // Chain A→B→C: both B and C must be symptoms
    #[test]
    fn symptom_resolution_chain_a_blocks_b_blocks_c() {
        let mut a = notice("a", "STP_001", Severity::Kritik, RuleClass::Spec);
        a.scope_key = Some("S1".into());
        a.blocks = vec!["GEO_009".to_string()];

        let mut b = notice("b", "GEO_009", Severity::Kritik, RuleClass::Interop);
        b.scope_key = Some("S1".into());
        b.blocks = vec!["SHP_017".to_string()];

        let mut c = notice("c", "SHP_017", Severity::Yuksek, RuleClass::Interop);
        c.scope_key = Some("S1".into());

        let all = [a, b, c];
        let resolution = resolve_symptoms(&all);
        assert!(!resolution.is_symptom[0], "A kök neden olmalı");
        assert!(resolution.is_symptom[1], "B semptom olmalı");
        assert!(resolution.is_symptom[2], "C semptom olmalı");
    }

    // realized_dep_for_group: numeric value validation
    #[test]
    fn realized_dep_for_group_counts_fired_blocked_rules() {
        let mut root = notice("r", "ARC_001", Severity::Kritik, RuleClass::Spec);
        root.blocks = vec!["B1".to_string(), "B2".to_string(), "B3".to_string()];

        let b1 = notice("b1", "B1", Severity::Orta, RuleClass::Quality);
        let b2 = notice("b2", "B2", Severity::Orta, RuleClass::Quality);
        // No notice for B3 → realized_dep = 2

        let all = vec![root, b1, b2];
        let resolution = resolve_symptoms(&all);
        let dep = realized_dep_for_group(&[0], &all, &resolution.rule_scope_buckets);
        assert_eq!(dep, 2, "B1 ve B2 ateşlendi, B3 yok → realized_dep=2");
    }

    // ── R1 ────────────────────────────────────────────────────────────────────

    #[test]
    fn r1_spec_kritik_makes_not_publishable() {
        let n = notice("n1", "X", Severity::Kritik, RuleClass::Spec);
        let r = build_r1(&[n], true);
        assert!(!r.publishable);
        assert_eq!(r.blocker_notice_ids.len(), 1);
    }

    #[test]
    fn r1_high_interop_is_publishable() {
        // Phase 4: Interop is no longer a publication blocker (only Spec+Kritik).
        let n = notice("n1", "X", Severity::Yuksek, RuleClass::Interop);
        let r = build_r1(&[n], true);
        assert!(r.publishable);
        assert!(r.blocker_notice_ids.is_empty());
    }

    #[test]
    fn r1_no_blockers_fully_publishable() {
        let n = notice("n1", "X", Severity::Orta, RuleClass::Quality);
        let r = build_r1(&[n], true);
        assert!(r.publishable);
    }

    #[test]
    fn r1_kritik_interop_is_publishable() {
        // Phase 4: even Kritik Interop does not block publication (not a spec gate).
        let n = notice("n1", "X", Severity::Kritik, RuleClass::Interop);
        let r = build_r1(&[n], true);
        assert!(r.publishable);
        assert!(r.blocker_notice_ids.is_empty());
    }

    // ── R9 ────────────────────────────────────────────────────────────────────

    #[test]
    fn r9_excludes_symptoms() {
        let mut root = notice("root", "STP_001", Severity::Kritik, RuleClass::Spec);
        root.scope_key = Some("S1".into());
        root.blocks = vec!["GEO_002".to_string()];
        let mut sym = notice("sym", "GEO_002", Severity::Orta, RuleClass::Analytics);
        sym.scope_key = Some("S1".into());

        let all = vec![root, sym];
        let resolution = resolve_symptoms(&all);
        let r9 = build_r9(&all, &resolution);
        // Symptom (GEO_002) must not appear in R9
        assert!(
            r9.items.iter().all(|i| i.rule_id != "GEO_002"),
            "semptom R9'da görünmemeli"
        );
    }

    #[test]
    fn r9_bucket_order_blocker_before_interop() {
        // Phase 4: only Spec+Kritik → Blocker. Interop (including Kritik) → Interop bucket;
        // sorted within the bucket by priority_score (higher severity first).
        let interop_low = notice("i1", "INTEROP_LOW", Severity::Orta, RuleClass::Interop);
        let interop_high = notice("ih", "INTEROP_HIGH", Severity::Yuksek, RuleClass::Interop);
        let blocker = notice("bl", "BLOCKER", Severity::Kritik, RuleClass::Spec);

        let resolution =
            resolve_symptoms(&[interop_low.clone(), interop_high.clone(), blocker.clone()]);
        let r9 = build_r9(&[interop_low, interop_high, blocker], &resolution);

        assert_eq!(r9.items.len(), 3);
        assert!(
            r9.items[0].labels.contains(&R9Label::Blocker),
            "ilk sıra: blocker"
        );
        assert!(
            r9.items[1].labels.contains(&R9Label::Interop),
            "ikinci sıra: interop (yüksek)"
        );
        assert!(
            r9.items[2].labels.contains(&R9Label::Interop),
            "üçüncü sıra: interop (orta)"
        );
        assert_eq!(
            r9.items[1].rule_id, "INTEROP_HIGH",
            "yüksek interop, orta'dan önce"
        );
    }

    #[test]
    fn r9_propagation_label_when_realized_dep_gt_2() {
        // All three different blocked rules have notices → realized_dep = 3 → Propagation
        let mut root = notice("root", "ARC_001", Severity::Kritik, RuleClass::Spec);
        root.blocks = vec!["B1".to_string(), "B2".to_string(), "B3".to_string()];

        let b1 = notice("b1", "B1", Severity::Orta, RuleClass::Quality);
        let b2 = notice("b2", "B2", Severity::Orta, RuleClass::Quality);
        let b3 = notice("b3", "B3", Severity::Orta, RuleClass::Quality);

        let all = [root, b1, b2, b3];
        let resolution = resolve_symptoms(&all);
        let r9 = build_r9(&all, &resolution);

        let root_item = r9.items.iter().find(|i| i.rule_id == "ARC_001").unwrap();
        assert!(
            root_item.labels.contains(&R9Label::Propagation),
            "realized_dep=3 → Propagation etiketi beklenir"
        );
    }

    #[test]
    fn r9_quick_win_label() {
        // base_effort=1, 1 instance → fix_effort=1.0, severity=YÜKSEK → QuickWin
        let n = notice("n1", "ARC_001", Severity::Yuksek, RuleClass::Spec);
        let resolution = resolve_symptoms(std::slice::from_ref(&n));
        let r9 = build_r9(&[n], &resolution);
        assert!(r9.items[0].labels.contains(&R9Label::QuickWin));
    }

    // ── R4 display label ──────────────────────────────────────────────────────

    #[test]
    fn r4_display_label_maps_shp_aliases() {
        assert_eq!(r4_display_label("SHP_014"), "GEO_010");
        assert_eq!(r4_display_label("SHP_016"), "GEO_011");
        assert_eq!(r4_display_label("GEO_002"), "GEO_002");
        assert_eq!(r4_display_label("SHP_013"), "SHP_013"); // no alias anymore
    }

    // ── R5 score ──────────────────────────────────────────────────────────────

    #[test]
    fn r5_empty_feed_scores_100() {
        let r5 = build_r5(&[]);
        assert_eq!(r5.score, 100.0);
        assert_eq!(r5.spec_score, 100.0);
    }

    #[test]
    fn r5_errors_reduce_score() {
        let n = notice("n1", "X", Severity::Kritik, RuleClass::Spec);
        let r5 = build_r5(&[n]);
        assert!(r5.score < 100.0, "hata varken skor 100 olamaz");
        assert!(r5.spec_score < 100.0);
        assert_eq!(r5.interop_score, 100.0, "INTEROP hatası yok → 100");
    }

    // ── R9 score formula ──────────────────────────────────────────────────────

    #[test]
    fn priority_score_formula() {
        // severity_weight=4 (KRİTİK), realized_dep=0, affected=1, fix_effort=1.0
        // expected = 4 × 1 × log2(2) / 1 = 4.0
        let score = compute_priority_score(Severity::Kritik, 0, 1, 1.0);
        assert!((score - 4.0).abs() < 0.01, "beklenen ~4.0, gelen {score}");
    }

    #[test]
    fn instance_multiplier_boundaries() {
        assert_eq!(instance_multiplier(1), 1.0);
        assert_eq!(instance_multiplier(5), 1.0);
        assert_eq!(instance_multiplier(6), 1.5);
        assert_eq!(instance_multiplier(50), 1.5);
        assert_eq!(instance_multiplier(51), 2.0);
    }

    // ── Closure: helper function tests ───────────────────────────────────────

    /// Scenario 1: A root notice closes a symptom.
    /// score_delta must be larger because it covers the closure set (root + symptom),
    /// not only the root's own penalty.
    #[test]
    fn closure_score_delta_larger_than_root_only_penalty() {
        // Root: SPEC/KRİTİK — blocks SYMPTOM_RULE
        let mut root = notice("root", "ROOT_001", Severity::Kritik, RuleClass::Spec);
        root.blocks = vec!["SYMPTOM_RULE".to_string()];

        // Symptom: SPEC/YÜKSEK — symptom of the same root cause as the root
        let sym = notice("sym", "SYMPTOM_RULE", Severity::Yuksek, RuleClass::Spec);

        let all = vec![root, sym];
        let resolution = resolve_symptoms(&all);
        let r9 = build_r9(&all, &resolution);

        // Only ROOT_001 should appear in R9 (SYMPTOM_RULE is hidden as a symptom)
        let root_item = r9
            .items
            .iter()
            .find(|i| i.rule_id == "ROOT_001")
            .expect("ROOT_001 R9'da olmalı");

        // Delta calculated with only the root penalty: Spec class, k=50, penalty=4.0
        // cur_cs = 50*100/(50+7) ≈ 87.72; new_cs(root only) = 50*100/(50+3) ≈ 94.34
        // However, the closure delta removes the root+symptom penalty (4+3=7)
        // new_cs(closure) = 50*100/(50+0) = 100
        // Therefore closure delta > root-only delta
        let root_only_delta = {
            let k = 50.0_f64;
            let cur_cp = 7.0_f64; // root(4) + sym(3)
            let cur_cs = k * 100.0 / (k + cur_cp);
            let new_cs = k * 100.0 / (k + (cur_cp - 4.0).max(0.0)); // remove only the root penalty
            round1(0.4 * (new_cs - cur_cs))
        };

        assert!(
            root_item.score_delta > root_only_delta,
            "closure score_delta ({}) sadece-root delta'dan ({}) büyük olmalı",
            root_item.score_delta,
            root_only_delta
        );
    }

    /// R9 score_delta is the R5 overall gain from removing the closure set, computed
    /// with R5's own per-rule saturating penalty.
    #[test]
    fn score_delta_matches_r5_without_the_rule() {
        let mut all: Vec<Notice> = (0..60)
            .map(|i| notice(&format!("a{i}"), "RULE_A", Severity::Yuksek, RuleClass::Spec))
            .collect();
        all.push(notice("b0", "RULE_B", Severity::Orta, RuleClass::Quality));
        let r9 = build_r9(&all, &resolve_symptoms(&all));
        let item = r9.items.iter().find(|i| i.rule_id == "RULE_A").unwrap();

        let without: Vec<Notice> = all.iter().filter(|n| n.rule_id != "RULE_A").cloned().collect();
        let expected = r5_overall(&r5_rule_penalties(&without), &HashMap::new())
            - r5_overall(&r5_rule_penalties(&all), &HashMap::new());
        assert_eq!(item.score_delta, round1(expected));
        assert!(
            (r5_overall(&r5_rule_penalties(&all), &HashMap::new()) - build_r5_unrounded(&all)).abs()
                < 1e-9
        );
    }

    /// A capped run (web: 500 samples per rule) must report the same score_delta as the
    /// full list once the rule is past the instance_multiplier saturation point.
    #[test]
    fn score_delta_does_not_depend_on_sample_count_past_saturation() {
        let delta = |n: usize| {
            let all: Vec<Notice> = (0..n)
                .map(|i| notice(&format!("a{i}"), "RULE_A", Severity::Yuksek, RuleClass::Spec))
                .collect();
            build_r9(&all, &resolve_symptoms(&all)).items[0].score_delta
        };
        assert_eq!(delta(500), delta(5000));
    }

    fn build_r5_unrounded(notices: &[Notice]) -> f64 {
        0.4 * class_score(notices, RuleClass::Spec)
            + 0.3 * class_score(notices, RuleClass::Interop)
            + 0.2 * class_score(notices, RuleClass::Quality)
            + 0.1 * class_score(notices, RuleClass::Analytics)
    }

    /// Scenario 2: Scope mismatch — a blocked notice with a different scope_key must not enter the closure.
    #[test]
    fn closure_scope_mismatch_excluded() {
        let mut root = notice("root", "ROOT_001", Severity::Kritik, RuleClass::Spec);
        root.scope_key = Some("scope_A".into());
        root.blocks = vec!["DOWNSTREAM".to_string()];

        // Different scope: must not enter the closure
        let mut ds_mismatch = notice("ds_mm", "DOWNSTREAM", Severity::Yuksek, RuleClass::Spec);
        ds_mismatch.scope_key = Some("scope_B".into());

        // Same scope: must enter the closure
        let mut ds_match = notice("ds_ok", "DOWNSTREAM", Severity::Yuksek, RuleClass::Spec);
        ds_match.scope_key = Some("scope_A".into());

        let all = vec![root, ds_mismatch, ds_match];
        let resolution = resolve_symptoms(&all);

        // Closure: group_indices = [0] (root)
        let closure = closure_notice_indices_for_group(&[0], &all, &resolution);

        assert!(closure.contains(&0), "root closure'da olmalı");
        assert!(closure.contains(&2), "aynı scope → closure'da olmalı");
        assert!(!closure.contains(&1), "farklı scope → closure'da olmamalı");
    }

    /// Scenario 3: a scope_key=None root must include blocked notices from different scopes in the closure.
    #[test]
    fn closure_none_scope_root_captures_all_scopes() {
        let mut root = notice("root", "FEED_ERR", Severity::Kritik, RuleClass::Spec);
        root.scope_key = None; // feed-level
        root.blocks = vec!["ENTITY_ERR".to_string()];

        let mut ds1 = notice("ds1", "ENTITY_ERR", Severity::Yuksek, RuleClass::Interop);
        ds1.scope_key = Some("trip_1".into());

        let mut ds2 = notice("ds2", "ENTITY_ERR", Severity::Yuksek, RuleClass::Interop);
        ds2.scope_key = Some("trip_2".into());

        let mut ds3 = notice("ds3", "ENTITY_ERR", Severity::Orta, RuleClass::Quality);
        ds3.scope_key = None;

        let all = vec![root, ds1, ds2, ds3];
        let resolution = resolve_symptoms(&all);

        let closure = closure_notice_indices_for_group(&[0], &all, &resolution);

        assert!(closure.contains(&0), "root closure'da");
        assert!(closure.contains(&1), "trip_1 scope → None root ile eşleşir");
        assert!(closure.contains(&2), "trip_2 scope → None root ile eşleşir");
        assert!(
            closure.contains(&3),
            "None scope downstream → None root ile eşleşir"
        );
        assert_eq!(closure.len(), 4);
    }

    /// Scenario 4: Transitive chain A blocks B, B blocks C → A's closure must contain both B and C.
    #[test]
    fn closure_transitive_chain_a_blocks_b_blocks_c() {
        let mut a = notice("a", "RULE_A", Severity::Kritik, RuleClass::Spec);
        a.scope_key = Some("S1".into());
        a.blocks = vec!["RULE_B".to_string()];

        let mut b = notice("b", "RULE_B", Severity::Yuksek, RuleClass::Interop);
        b.scope_key = Some("S1".into());
        b.blocks = vec!["RULE_C".to_string()];

        let mut c = notice("c", "RULE_C", Severity::Orta, RuleClass::Quality);
        c.scope_key = Some("S1".into());

        let all = vec![a, b, c];
        let resolution = resolve_symptoms(&all);

        let closure = closure_notice_indices_for_group(&[0], &all, &resolution);

        assert!(closure.contains(&0), "A closure'da");
        assert!(closure.contains(&1), "B (doğrudan blocked) closure'da");
        assert!(closure.contains(&2), "C (transitif: A→B→C) closure'da");
        assert_eq!(closure.len(), 3);
    }

    /// Scenario 4b: Transitive closure must not loop forever (cyclic blocks).
    #[test]
    fn closure_no_infinite_loop_on_cycle() {
        // A→B→A cycle
        let mut a = notice("a", "RULE_A", Severity::Kritik, RuleClass::Spec);
        a.blocks = vec!["RULE_B".to_string()];

        let mut b = notice("b", "RULE_B", Severity::Yuksek, RuleClass::Interop);
        b.blocks = vec!["RULE_A".to_string()]; // cycle

        let all = vec![a, b];
        let resolution = resolve_symptoms(&all);

        // Must not loop forever
        let closure = closure_notice_indices_for_group(&[0], &all, &resolution);
        assert!(closure.contains(&0));
        assert!(closure.contains(&1));
        assert_eq!(
            closure.len(),
            2,
            "each notice must be counted once even in a cycle"
        );
    }

    /// Scenario 5: pub_score_delta must increase using only pub-relevant notices
    /// and the grouped hyperbolic calculation.
    ///
    /// Setup:
    /// - Root: Kritik/Spec (pub-relevant) — blocks DOWNSTREAM
    /// - Downstream 1: Kritik/Spec (pub-relevant) — ENTERS THE CLOSURE
    /// - Downstream 2: Orta/Quality (NOT pub-relevant) — it does not affect pub even if closed
    ///
    /// Expectation:
    /// - pub_score_delta > 0 (at least the root+downstream1 publication penalty was removed)
    /// - However, the quality notice does not contribute to pub_score_delta
    #[test]
    fn closure_pub_score_delta_only_pub_relevant_notices() {
        let mut root = notice("root", "PUB_ROOT", Severity::Kritik, RuleClass::Spec);
        root.blocks = vec!["PUB_DS".to_string(), "QUAL_DS".to_string()];

        // pub-relevant downstream
        let ds_pub = notice("ds_pub", "PUB_DS", Severity::Kritik, RuleClass::Spec);
        // non-pub-relevant downstream
        let ds_qual = notice("ds_qual", "QUAL_DS", Severity::Orta, RuleClass::Quality);

        let all = vec![root, ds_pub, ds_qual];
        let resolution = resolve_symptoms(&all);
        let r9 = build_r9(&all, &resolution);

        let root_item = r9
            .items
            .iter()
            .find(|i| i.rule_id == "PUB_ROOT")
            .expect("PUB_ROOT R9'da olmalı");

        // pub_score_delta > 0: at least the pub-relevant root penalty was removed
        assert!(
            root_item.pub_score_delta > 0.0,
            "pub-relevant root → pub_score_delta sıfırdan büyük olmalı; değer: {}",
            root_item.pub_score_delta
        );

        // Verify the pub_score_delta calculation manually:
        // total_pub_penalty: PUB_ROOT(4, cnt=1)×1.0 + PUB_DS(4, cnt=1)×1.0 = 8.0
        // closure pub penalty: same = 8.0
        // new_pub = 50*100/(50+0) = 100; cur_pub = 50*100/(50+8) ≈ 86.2
        // pub_score_delta ≈ 13.8
        let k = 50.0_f64;
        let total_pp = 8.0_f64; // PUB_ROOT(4×1) + PUB_DS(4×1)
        let cur_pub = k * 100.0 / (k + total_pp);
        let new_pub = k * 100.0 / (k + (total_pp - total_pp).max(0.0));
        let expected_delta = round1(new_pub - cur_pub);
        assert!(
            (root_item.pub_score_delta - expected_delta).abs() < 0.11,
            "pub_score_delta beklenen ≈{expected_delta}, gelen {}",
            root_item.pub_score_delta
        );
    }

    /// Scenario 5b: when multiple closure notices share a rule_id, the grouped hyperbolic
    /// calculation (instance_multiplier) must be applied correctly.
    #[test]
    fn closure_pub_score_delta_grouped_hyperbolic() {
        // Root: pub-relevant, blocks DS_RULE (multiple instances)
        let mut root = notice("root", "ROOT_PUB", Severity::Kritik, RuleClass::Spec);
        root.blocks = vec!["DS_RULE".to_string()];

        // 10 closure downstream notices with the same rule_id (cnt=10 → multiplier=1.5)
        let mut downstream: Vec<Notice> = (0..10)
            .map(|i| {
                notice(
                    &format!("ds{i}"),
                    "DS_RULE",
                    Severity::Kritik,
                    RuleClass::Spec,
                )
            })
            .collect();
        // All have a None scope → they match the root

        let mut all = vec![root];
        all.append(&mut downstream);

        let resolution = resolve_symptoms(&all);
        let r9 = build_r9(&all, &resolution);

        let root_item = r9
            .items
            .iter()
            .find(|i| i.rule_id == "ROOT_PUB")
            .expect("ROOT_PUB R9'da olmalı");

        // Closure pub penalty:
        // ROOT_PUB: cnt=1 → 4×1.0 = 4
        // DS_RULE:  cnt=10 → 4×1.5 = 6  (10 > 5, cnt in 6..=50 → 1.5)
        // total = 10; closure_pub_penalty = 10; total publication penalty = 10
        // pub_score_delta = 50*100/(50+0) - 50*100/(50+10) = 100 - 83.3 = 16.7
        let k = 50.0_f64;
        let total_pp = 4.0 * 1.0 + 4.0 * 1.5; // root(1 instance) + ds(10 instances → ×1.5)
        let cur_pub = k * 100.0 / (k + total_pp);
        let new_pub = k * 100.0 / (k + 0.0_f64);
        let expected_delta = round1(new_pub - cur_pub);

        assert!(
            (root_item.pub_score_delta - expected_delta).abs() < 0.11,
            "gruplu hiperbolik pub_score_delta beklenen ≈{expected_delta}, gelen {}",
            root_item.pub_score_delta
        );
    }

    // Is R9 ordering deterministic for equal scores? (rule_id tie-break)
    #[test]
    fn r9_order_deterministic_on_score_ties() {
        // Same severity/class, different rule_id → equal bucket_rank and priority_score.
        // IDs absent from the registry → metadata fallback makes scores exactly equal.
        // scope_key None, no blocks → all are root causes and all become R9 items.
        let scrambled = ["ZZ_003", "ZZ_001", "ZZ_002"];
        let notices: Vec<Notice> = scrambled
            .iter()
            .enumerate()
            .map(|(i, rid)| notice(&format!("n{i}"), rid, Severity::Orta, RuleClass::Quality))
            .collect();

        // build_r9 creates a new HashMap (new random seed) on each call; the output order
        // must be the same on every call and ascending by rule_id due to the tie-break.
        let mut seen: Option<Vec<String>> = None;
        for _ in 0..16 {
            let resolution = resolve_symptoms(&notices);
            let r9 = build_r9(&notices, &resolution);
            let order: Vec<String> = r9.items.iter().map(|it| it.rule_id.clone()).collect();
            assert_eq!(
                order,
                vec!["ZZ_001", "ZZ_002", "ZZ_003"],
                "eşit skorlarda rule_id'ye göre artan sıralı olmalı"
            );
            match &seen {
                Some(prev) => assert_eq!(
                    &order, prev,
                    "tekrar çağrıda sıra değişmemeli (deterministik)"
                ),
                None => seen = Some(order),
            }
        }
    }

    #[test]
    fn info_future_only_cal015_is_not_an_r9_action_item() {
        let notices = vec![notice("n1", "CAL_015", Severity::Bilgi, RuleClass::Quality)];
        let resolution = resolve_symptoms(&notices);
        let r9 = build_r9(&notices, &resolution);
        assert!(
            r9.items.is_empty(),
            "zero-penalty future-only CAL_015 R9 düzeltme kuyruğuna girmemeli"
        );
    }
}
