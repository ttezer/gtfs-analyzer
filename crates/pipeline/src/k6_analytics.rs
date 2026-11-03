use rustc_hash::{FxHashMap, FxHashSet};
use smol_str::SmolStr;
use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet};
use url::Url;

use gtfs_config::ValidatorConfig;
use gtfs_core::{EntityType, Notice};

use crate::k2::stop_times::CompactStopTime;
use crate::k2::stop_times::StopTimesIndex as K2StopTimesIndex;
use crate::k2::EntityRecords;
use crate::k5_derived::DerivedData;
use crate::recovery::FileAvailability;

// ── Output type ───────────────────────────────────────────────────────────────

#[derive(Debug, Default)]
pub struct K6Result {
    pub notices: Vec<Notice>,
    pub skipped_checks: Vec<String>,
}

/// A conservative identity for the URL comparison rules STP_034/035.
///
/// URL normalization here is intentionally limited to syntax that cannot change
/// the referenced web resource: scheme/host casing, root-path spelling, and
/// explicit default ports. Query strings, fragments, path trailing slashes and
/// percent-encoding remain significant so different pages are not collapsed into
/// one quality finding.
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
struct NormalizedWebUrl {
    scheme: String,
    username: String,
    password: Option<String>,
    host: String,
    port: Option<u16>,
    path: String,
    query: Option<String>,
    fragment: Option<String>,
}

fn normalize_web_url(raw: &str) -> Option<NormalizedWebUrl> {
    let parsed = Url::parse(raw).ok()?;
    if !matches!(parsed.scheme(), "http" | "https") {
        return None;
    }
    let host = parsed.host_str()?.to_ascii_lowercase();
    let scheme = parsed.scheme().to_ascii_lowercase();
    let port = match parsed.port() {
        Some(port) if (scheme == "http" && port == 80) || (scheme == "https" && port == 443) => {
            None
        }
        other => other,
    };
    let path = match parsed.path() {
        "" | "/" => String::new(),
        path => path.to_string(),
    };

    Some(NormalizedWebUrl {
        scheme,
        username: parsed.username().to_string(),
        password: parsed.password().map(str::to_string),
        host,
        port,
        path,
        query: parsed.query().map(str::to_string),
        fragment: parsed.fragment().map(str::to_string),
    })
}

fn format_normalized_web_url(url: &NormalizedWebUrl) -> String {
    let authority = if url.username.is_empty() {
        String::new()
    } else {
        format!(
            "{}{}@",
            url.username,
            url.password
                .as_ref()
                .map(|p| format!(":{p}"))
                .unwrap_or_default()
        )
    };
    let host = if url.host.contains(':') {
        format!("[{0}]", url.host)
    } else {
        url.host.clone()
    };
    let port = url.port.map(|p| format!(":{p}")).unwrap_or_default();
    let path = if url.path.is_empty() { "/" } else { &url.path };
    let query = url
        .query
        .as_ref()
        .map(|q| format!("?{q}"))
        .unwrap_or_default();
    let fragment = url
        .fragment
        .as_ref()
        .map(|f| format!("#{f}"))
        .unwrap_or_default();
    format!(
        "{}://{}{}{}{}{}{}",
        url.scheme, authority, host, port, path, query, fragment
    )
}

// ── Ana fonksiyon ─────────────────────────────────────────────────────────────

/// today_yyyymmdd: date at execution time (e.g. 20260514), for CAL/CLD expiry rules.
pub fn analyze(
    records: &EntityRecords,
    derived: &DerivedData,
    config: &ValidatorConfig,
    today_yyyymmdd: u32,
) -> K6Result {
    analyze_with_files(
        records,
        derived,
        config,
        today_yyyymmdd,
        &FileAvailability::complete(),
    )
}

pub fn analyze_with_files(
    records: &EntityRecords,
    derived: &DerivedData,
    config: &ValidatorConfig,
    today_yyyymmdd: u32,
    availability: &FileAvailability<'_>,
) -> K6Result {
    use crate::timing::Timer;

    let ti_analyze = &records.trip_interns;
    // K2 already interns shape ids. K6 keeps the compact index here instead of
    // repeating an 8-byte shape-id pointer for every stop_times association.
    let shape_id_to_idx: HashMap<&str, u32> = (1..records.shape_interns.len())
        .map(|i| (records.shape_interns.id_at(i as u32), i as u32))
        .collect();
    let trip_shape: HashMap<&str, u32> = records
        .trips
        .iter()
        .filter_map(|t| {
            ti_analyze
                .shape_id(t)
                .and_then(|s| shape_id_to_idx.get(s).copied())
                .map(|shape_idx| (t.trip_id.as_str(), shape_idx))
        })
        .collect();
    // OOM fix Plan D (updated): K6 does NOT CLONE the K2 index. In tests (stop_times_index empty
    // but records.stop_times populated), an owned K2 index is built; in production,
    // records.stop_times_index is borrowed.
    let fallback_k2_owned: Option<K2StopTimesIndex> =
        if records.stop_times_index.total_rows == 0 && !records.stop_times.is_empty() {
            Some(K2StopTimesIndex::from_records(&records.stop_times))
        } else {
            None
        };
    let k2_for_idx: &K2StopTimesIndex = fallback_k2_owned
        .as_ref()
        .unwrap_or(&records.stop_times_index);
    let idx = {
        let _t = Timer::start("K6::idx::build");
        StopTimesIndex::build(records, &trip_shape, k2_for_idx)
    };
    // Shape geometry: ONE setup for four consumers (speed_and_duration, remaining_analytics, shp012, shp022).
    // See the measurement note in the ShapeIndex definition.
    let shape_idx = {
        let _t = Timer::start("K6::shape_idx::build");
        ShapeIndex::build(records)
    };
    // Rail trip/shape sets: ONE setup for the three consumers that relax geometry thresholds
    // (geo_analytics, remaining_analytics, shp012) and for STM_045 — the shape_idx precedent.
    let rail = {
        let _t = Timer::start("K6::rail_index::build");
        rail_index(records)
    };

    // Each independent K6 check produces its OWN (notices, ctr); results are merged in CANONICAL
    // order (1→13), and the final renumbered IDs map EXACTLY through a single-threaded global ctr.
    // Parallelism is BETWEEN checks (each check has its own dedup set and only READS
    // `records/derived/config/idx`) → count/content/order are independent of emission order.
    // With the `parallel` feature, checks run in parallel through rayon::scope; otherwise serially — SAME output.
    type K6Task<'a> = Box<dyn Fn() -> Vec<Notice> + Send + Sync + 'a>;
    let mut tasks: Vec<K6Task> = Vec::new();
    let mut skipped_checks = Vec::new();
    record_data_quality_skips(availability, config, today_yyyymmdd, &mut skipped_checks);
    macro_rules! add_task {
        ($name:literal, $condition:expr, $body:expr) => {
            if $condition {
                tasks.push(Box::new($body));
            } else {
                skipped_checks.push($name.to_string());
            }
        };
    }
    // Each task runs only when the readable files it directly needs are present.
    // Thus a broken shapes.txt disables shape analytics but does not lose stop/route/calendar
    // analytics; an independent file such as feed_info does not cut off all of K6.
    add_task!(
        "K6::speed_and_duration",
        availability.all(&["routes.txt", "trips.txt", "stop_times.txt", "stops.txt"]),
        || {
            let _t = Timer::start("K6::speed_and_duration");
            let mut v = Vec::new();
            let mut c = 0u32;
            check_speed_and_duration(records, config, &idx, &shape_idx, &mut v, &mut c);
            v
        }
    );
    add_task!(
        "K6::frequency_headway",
        availability.all(&[
            "routes.txt",
            "trips.txt",
            "stop_times.txt",
            "frequencies.txt"
        ]),
        || {
            let _t = Timer::start("K6::frequency_headway");
            let mut v = Vec::new();
            let mut c = 0u32;
            check_frequency_headway(records, config, &mut v, &mut c);
            v
        }
    );
    add_task!(
        "K6::route_headway",
        availability.all(&["routes.txt", "trips.txt", "stop_times.txt"]),
        || {
            let _t = Timer::start("K6::route_headway");
            let mut v = Vec::new();
            let mut c = 0u32;
            check_route_headway(records, config, &idx, &mut v, &mut c);
            v
        }
    );
    add_task!(
        "K6::calendar_analytics",
        availability.present_and_available("trips.txt")
            && availability.any_present_and_available(&["calendar.txt", "calendar_dates.txt"]),
        || {
            let _t = Timer::start("K6::calendar_analytics");
            let mut v = Vec::new();
            let mut c = 0u32;
            check_calendar_analytics(records, derived, config, today_yyyymmdd, &mut v, &mut c);
            v
        }
    );
    // Three separate gates: each requires the files its own block ACTUALLY reads.
    // A single `all(&[stops, routes, trips, stop_times])` gate killed checks that needed only
    // `stops.txt` because of an unrelated missing file (#160).
    add_task!("K6::geo_stops", availability.available("stops.txt"), || {
        let _t = Timer::start("K6::geo_stops");
        let mut v = Vec::new();
        let mut c = 0u32;
        check_geo_stop_analytics(records, derived, config, &mut v, &mut c);
        v
    });
    add_task!(
        "K6::geo_shapes",
        availability.available("shapes.txt"),
        || {
            let _t = Timer::start("K6::geo_shapes");
            let mut v = Vec::new();
            let mut c = 0u32;
            check_geo_shape_analytics(records, derived, config, &rail, &mut v, &mut c);
            v
        }
    );
    add_task!(
        "K6::geo_shape_trips",
        availability.all(&["stops.txt", "trips.txt", "stop_times.txt", "shapes.txt"]),
        || {
            let _t = Timer::start("K6::geo_shape_trips");
            let mut v = Vec::new();
            let mut c = 0u32;
            check_geo_shape_trip_analytics(records, config, &rail, &mut v, &mut c);
            v
        }
    );
    add_task!(
        "K6::operational_analytics",
        availability.all(&["stops.txt", "routes.txt", "trips.txt", "stop_times.txt"]),
        || {
            let _t = Timer::start("K6::operational_analytics");
            let mut v = Vec::new();
            let mut c = 0u32;
            check_operational_analytics(
                records,
                derived,
                config,
                &idx,
                today_yyyymmdd,
                &mut v,
                &mut c,
            );
            v
        }
    );
    add_task!(
        "K6::stoptimes_derived",
        availability.available("stop_times.txt"),
        || {
            let _t = Timer::start("K6::stoptimes_derived");
            let mut v = Vec::new();
            let mut c = 0u32;
            check_stoptimes_derived(&idx, &mut v, &mut c);
            v
        }
    );
    add_task!(
        "K6::route_trip_quality",
        availability.all(&["routes.txt", "trips.txt", "stop_times.txt"]),
        || {
            let _t = Timer::start("K6::route_trip_quality");
            let mut v = Vec::new();
            let mut c = 0u32;
            check_route_trip_quality(records, derived, &idx, &mut v, &mut c);
            v
        }
    );
    add_task!("K6::data_quality", true, || {
        let _t = Timer::start("K6::data_quality");
        let mut v = Vec::new();
        let mut c = 0u32;
        check_data_quality(
            records,
            derived,
            config,
            today_yyyymmdd,
            availability,
            &mut v,
            &mut c,
        );
        v
    });
    add_task!(
        "K6::remaining_analytics",
        availability.all(&["stops.txt", "trips.txt", "stop_times.txt"]),
        || {
            let _t = Timer::start("K6::remaining_analytics");
            let mut v = Vec::new();
            let mut c = 0u32;
            check_remaining_analytics(
                records,
                derived,
                config,
                &idx,
                &shape_idx,
                &rail.shapes,
                &mut v,
                &mut c,
            );
            v
        }
    );
    add_task!(
        "K6::shp012",
        availability.all(&["stops.txt", "trips.txt", "stop_times.txt"]),
        || {
            let _t = Timer::start("K6::shp012");
            let mut v = Vec::new();
            let mut c = 0u32;
            check_shp012(
                records,
                config,
                &idx,
                &shape_idx,
                &rail.shapes,
                &mut v,
                &mut c,
            );
            v
        }
    );
    add_task!(
        "K6::shp022",
        availability.all(&["stops.txt", "trips.txt", "shapes.txt"]),
        || {
            let _t = Timer::start("K6::shp022");
            let mut v = Vec::new();
            let mut c = 0u32;
            check_shp022(records, &idx, &shape_idx, &mut v, &mut c);
            v
        }
    );
    add_task!(
        "K6::pathway_analytics",
        availability.all(&["pathways.txt", "stops.txt"]),
        || {
            let _t = Timer::start("K6::pathway_analytics");
            let mut v = Vec::new();
            let mut c = 0u32;
            check_pathway_analytics(records, derived, &mut v, &mut c);
            v
        }
    );
    add_task!(
        "K6::calendar_override",
        availability.present_and_available("trips.txt")
            && availability.any_present_and_available(&["calendar.txt", "calendar_dates.txt"]),
        || {
            let _t = Timer::start("K6::calendar_override");
            let mut v = Vec::new();
            let mut c = 0u32;
            check_calendar_override_analytics(records, derived, config, &idx, &mut v, &mut c);
            v
        }
    );
    add_task!(
        "K6::vat_analytics",
        availability.all(&["routes.txt", "trips.txt", "stops.txt", "stop_times.txt"]),
        || {
            let _t = Timer::start("K6::vat_analytics");
            let mut v = Vec::new();
            let mut c = 0u32;
            check_vat_analytics(records, derived, config, &idx, &mut v, &mut c);
            v
        }
    );
    // APPENDED at the end: because merge order is canonical, existing notice IDs do NOT SHIFT
    // when a new task stays last (renumbering follows position).
    add_task!(
        "K6::linked_trips",
        availability.all(&["trips.txt", "stop_times.txt"]),
        || {
            let _t = Timer::start("K6::linked_trips");
            let mut v = Vec::new();
            let mut c = 0u32;
            check_linked_trip_continuations(records, derived, &mut v, &mut c);
            v
        }
    );

    #[cfg(feature = "parallel")]
    let parts: Vec<Vec<Notice>> = {
        // Running K6 tasks with unlimited parallelism on a large feed (>15M stop_times) raises peak RAM
        // because each task has its own temporary maps. A single-task batch provides a bounded balance
        // between speed and memory; small feeds retain full parallelism. The single-task batch applies
        // only to WASM (browser memory limit); native keeps batches of 3 — on native, batch 1 increased
        // runtime by +1…+11% on >15M feeds (2026-10-30, measurement of 5 feeds).
        let large = records.stop_times_index.total_rows > 15_000_000;
        let batch = if !large {
            tasks.len()
        } else if cfg!(target_family = "wasm") {
            1
        } else {
            3
        };
        let mut out: Vec<Vec<Notice>> = tasks.iter().map(|_| Vec::new()).collect();
        for (i, (out_chunk, task_chunk)) in
            out.chunks_mut(batch).zip(tasks.chunks(batch)).enumerate()
        {
            if large {
                crate::timing::mem_log(&format!("K6 batch {i} start"));
            }
            rayon::scope(|s| {
                for (slot, task) in out_chunk.iter_mut().zip(task_chunk.iter()) {
                    s.spawn(move |_| {
                        *slot = task();
                    });
                }
            });
            if large {
                crate::timing::mem_log(&format!("K6 batch {i} end"));
            }
        }
        out
    };
    #[cfg(not(feature = "parallel"))]
    let parts: Vec<Vec<Notice>> = tasks.iter().map(|t| t()).collect();

    let mut notices: Vec<Notice> = Vec::with_capacity(parts.iter().map(|p| p.len()).sum());
    for p in parts {
        notices.extend(p);
    }
    // Renumber → EXACT IDs (k6/{rule}#{N}) through the single-threaded global ctr.
    // Because merge order is fixed (canonical), parallel and serial runs are indistinguishable.
    for (i, n) in notices.iter_mut().enumerate() {
        n.id = format!("k6/{}#{}", n.rule_id, i + 1);
    }

    K6Result {
        notices,
        skipped_checks,
    }
}

// ── Notice helpers ────────────────────────────────────────────────────────────

// K6 notices are built in the same order as the canonical emit table; keeping independent
// metadata fields in the parameter struct hides the analyzer calls.
#[allow(clippy::too_many_arguments)]
fn k6_notice(
    ctr: &mut u32,
    rule_id: &str,
    entity_type: EntityType,
    entity_id: Option<String>,
    scope_key: Option<String>,
    file: &str,
    line: Option<u64>,
    field: Option<&str>,
    observed: Option<String>,
    expected: Option<String>,
    message: String,
    remediation: &str,
) -> Notice {
    crate::notice_factory::build(
        "K6",
        Some("k6"),
        ctr,
        rule_id,
        entity_type,
        entity_id,
        scope_key,
        Some(file.to_string()),
        line,
        field.map(str::to_string),
        observed,
        expected,
        message,
        remediation,
    )
}

#[allow(clippy::too_many_arguments)]
fn stm012_details(
    stop_a: &str,
    stop_b: &str,
    seq_a: u32,
    seq_b: u32,
    distance_km: f64,
    scheduled_gap_seconds: u32,
    speed_kmh: Option<f64>,
    speed_lower_bound_kmh: Option<f64>,
    intermediate_stop_count: Option<usize>,
) -> BTreeMap<String, String> {
    let mut details = BTreeMap::new();
    details.insert("stop_a".to_string(), stop_a.to_string());
    details.insert("stop_b".to_string(), stop_b.to_string());
    details.insert("stop_sequence_a".to_string(), seq_a.to_string());
    details.insert("stop_sequence_b".to_string(), seq_b.to_string());
    details.insert("distance_km".to_string(), format!("{distance_km:.1}"));
    details.insert(
        "distance_meters".to_string(),
        format!("{:.0}", distance_km * 1000.0),
    );
    details.insert(
        "scheduled_gap_seconds".to_string(),
        scheduled_gap_seconds.to_string(),
    );
    details.insert("distance_method".to_string(), "haversine".to_string());
    if let Some(speed) = speed_kmh {
        details.insert("speed_kmh".to_string(), format!("{speed:.1}"));
    }
    if let Some(speed) = speed_lower_bound_kmh {
        details.insert("speed_lower_bound_kmh".to_string(), format!("{speed:.1}"));
    }
    if let Some(count) = intermediate_stop_count {
        details.insert("intermediate_stop_count".to_string(), count.to_string());
    }
    details
}

fn add_stm012_stop_context(
    details: &mut BTreeMap<String, String>,
    idx: &StopTimesIndex<'_>,
    stimes: &[CompactStopTime],
    a_index: usize,
    b_index: usize,
) {
    let add_times =
        |details: &mut BTreeMap<String, String>, prefix: &str, row: &CompactStopTime| {
            if let Some(time) = row.arrival_time() {
                details.insert(format!("{prefix}_arrival"), format_hms(hms_to_secs(time)));
            }
            if let Some(time) = row.departure_time() {
                details.insert(format!("{prefix}_departure"), format_hms(hms_to_secs(time)));
            }
        };
    if let Some(row) = stimes.get(a_index) {
        add_times(details, "stop_a", row);
    }
    if let Some(row) = stimes.get(b_index) {
        add_times(details, "stop_b", row);
    }
    if let Some(row) = a_index.checked_sub(1).and_then(|i| stimes.get(i)) {
        let stop_id = idx.stop_id_of(row);
        if !stop_id.is_empty() {
            details.insert("prev_stop_id".to_string(), stop_id.to_string());
            add_times(details, "prev", row);
        }
    }
    if let Some(row) = stimes.get(b_index + 1) {
        let stop_id = idx.stop_id_of(row);
        if !stop_id.is_empty() {
            details.insert("next_stop_id".to_string(), stop_id.to_string());
            add_times(details, "next", row);
        }
    }
}

/// Returns the first active service date when a detected GTFS-JP feed contains
/// no active date on or before today. This is a publication-state signal, not
/// a global GTFS quality defect: non-JP feeds retain the existing rules.
fn gtfs_jp_future_only_date(
    records: &EntityRecords,
    derived: &DerivedData,
    today_yyyymmdd: u32,
) -> Option<u32> {
    if records.is_gtfs_jp != Some(true) || today_yyyymmdd == 0 {
        return None;
    }

    let first = derived
        .calendar_bitmap
        .active_dates
        .values()
        .flat_map(|dates| dates.iter().copied())
        .min()?;
    (first > today_yyyymmdd).then_some(first)
}

/// Trips whose every stop_time is a Flex zone or location-group row (no `stop_id`).
/// Such an on-demand trip has no fixed path, so `shape_id` cannot describe it;
/// shape coverage rules (DQ_006, RTS_017) leave these trips out.
fn zone_only_trip_ids(records: &EntityRecords) -> HashSet<&str> {
    let index = &records.stop_times_index;
    index
        .iter_trips()
        .filter(|(_, stops)| {
            !stops.is_empty()
                && stops.iter().all(|st| {
                    st.stop_idx == u32::MAX
                        && index
                            .flex_of(st)
                            .is_some_and(|f| f.location_id.is_some() || f.location_group_id.is_some())
                })
        })
        .map(|(trip_id, _)| trip_id.as_str())
        .collect()
}

/// Number of example rows (from different trips) exposed to the planner in the STM_007 summary.
const STM007_EXAMPLE_ROWS: usize = 20;
/// Row detail → the `example_<i>_<ad>` key in the summary.
const STM007_EXAMPLE_KEYS: &[(&str, &str)] = &[
    ("stop_id", "stop"),
    ("arr", "arrival"),
    ("dep", "departure"),
    ("prev_stop_id", "prev_stop"),
    ("prev_arrival", "prev_arrival"),
    ("prev_departure", "prev_departure"),
    ("next_stop_id", "next_stop"),
    ("next_arrival", "next_arrival"),
    ("next_departure", "next_departure"),
];

/// "HH:MM:SS" (including 24+ hours) → seconds; reads back format_hms output.
fn hms_text_secs(text: &str) -> Option<u32> {
    let mut parts = text.split(':').map(|p| p.parse::<u32>().ok());
    let (h, m, s) = (parts.next()??, parts.next()??, parts.next()??);
    Some(h * 3600 + m * 60 + s)
}

fn finalize_stm007_pending(
    pending: &mut Vec<Notice>,
    notices: &mut Vec<Notice>,
    ctr: &mut u32,
    rule_id: &str,
    threshold: usize,
) {
    let n = pending.len();
    if n > threshold {
        // HashMap/chunk order is not deterministic: sort by (trip, row).
        pending.sort_by(|a, b| (&a.entity_id, a.line).cmp(&(&b.entity_id, b.line)));
        let mut trips: Vec<&str> = pending.iter().filter_map(|n| n.entity_id.as_deref()).collect();
        trips.dedup();
        let affected_trips = trips.len();
        let examples: Vec<String> = trips.iter().take(5).map(|t| t.to_string()).collect();
        let mut notice = k6_notice(
            ctr,
            rule_id,
            EntityType::Feed,
            None,
            None,
            "stop_times.txt",
            None,
            Some("departure_time"),
            Some(n.to_string()),
            None,
            format!("{n} stop_times satırında departure_time arrival_time değerinden önce — aynı satır-içi zaman ihlali yüksek hacimde tekrarlanıyor."),
            "stop_times.txt'te kalkış zamanını varış zamanından sonra (veya eşit) ayarlayın.",
        );
        let mut details = BTreeMap::new();
        details.insert("affected_rows".to_string(), n.to_string());
        details.insert("affected_trips".to_string(), affected_trips.to_string());
        if !examples.is_empty() {
            details.insert("example_trips".to_string(), examples.join(", "));
        }
        // For the planner, STM007_EXAMPLE_ROWS rows with neighboring times (example_<i>_*): first rows
        // at different stops and on different trips (consecutive trips on the same route must not repeat
        // the same stop), then remaining rows from different trips if space remains. Summary count/text unchanged.
        let stop_of = |row: &Notice| row.details.as_ref().and_then(|d| d.get("stop_id")).cloned();
        let mut chosen: Vec<&Notice> = Vec::new();
        let mut used_trips: FxHashSet<&str> = FxHashSet::default();
        let mut used_stops: FxHashSet<String> = FxHashSet::default();
        for distinct_stop in [true, false] {
            for row in pending.iter() {
                if chosen.len() >= STM007_EXAMPLE_ROWS {
                    break;
                }
                let Some(trip) = row.entity_id.as_deref() else { continue };
                if used_trips.contains(trip) {
                    continue;
                }
                let stop = stop_of(row).unwrap_or_default();
                if distinct_stop && used_stops.contains(&stop) {
                    continue;
                }
                used_trips.insert(trip);
                used_stops.insert(stop);
                chosen.push(row);
            }
        }
        for (i, row) in chosen.into_iter().enumerate() {
            let trip = row.entity_id.as_deref();
            details.insert(format!("example_{i}_trip"), trip.unwrap_or_default().to_string());
            if let Some(line) = row.line {
                details.insert(format!("example_{i}_line"), line.to_string());
            }
            if let Some(service) = row.service_id.as_deref() {
                details.insert(format!("example_{i}_service"), service.to_string());
            }
            if let Some(d) = row.details.as_ref() {
                for (from, to) in STM007_EXAMPLE_KEYS {
                    if let Some(v) = d.get(from) {
                        details.insert(format!("example_{i}_{to}"), v.clone());
                    }
                }
            }
        }
        // Pattern: how many seconds departure most often precedes arrival and the number of rows carrying
        // that difference; the five most affected stops. If the same difference repeats across thousands
        // of rows, it is a production rule rather than isolated typos.
        let mut gaps: FxHashMap<u32, usize> = FxHashMap::default();
        let mut stops: FxHashMap<&str, usize> = FxHashMap::default();
        for row in pending.iter() {
            let Some(d) = row.details.as_ref() else { continue };
            if let (Some(arr), Some(dep)) = (d.get("arr"), d.get("dep")) {
                if let (Some(a), Some(b)) = (hms_text_secs(arr), hms_text_secs(dep)) {
                    *gaps.entry(a.saturating_sub(b)).or_default() += 1;
                }
            }
            if let Some(stop) = d.get("stop_id") {
                *stops.entry(stop.as_str()).or_default() += 1;
            }
        }
        if let Some((gap, rows)) = gaps.iter().max_by(|a, b| a.1.cmp(b.1).then(b.0.cmp(a.0))) {
            details.insert("typical_gap_seconds".to_string(), gap.to_string());
            details.insert("typical_gap_rows".to_string(), rows.to_string());
        }
        let mut top: Vec<(&str, usize)> = stops.into_iter().collect();
        top.sort_by(|a, b| b.1.cmp(&a.1).then(a.0.cmp(b.0)));
        if !top.is_empty() {
            details.insert(
                "top_stops".to_string(),
                top.iter().take(5).map(|(s, _)| *s).collect::<Vec<_>>().join(","),
            );
        }
        notice.details = Some(details.into());
        notices.push(notice);
    } else {
        notices.append(pending);
    }
}

// ── Helper functions ──────────────────────────────────────────────────────────

/// Returns the numeric root of stop_id: strips a trailing letter suffix.
/// Returns the original ID when it contains no digit (comparison is unsafe).
/// Example: "2119d" → "2119", "t17d" → "t17", "abc" → "abc"
#[allow(dead_code)]
/// Does `needle` occur in `haystack` at a word boundary (surrounded by a non-alphanumeric
/// character or the start/end of the string)? Case-insensitive.
fn contains_as_word(haystack: &str, needle: &str) -> bool {
    if needle.is_empty() {
        return false;
    }
    let h = haystack.to_lowercase();
    let n = needle.to_lowercase();
    let nb = n.len();
    let mut start = 0usize;
    while let Some(pos) = h[start..].find(n.as_str()) {
        let pos = start + pos;
        let before_ok = pos == 0
            || !h[..pos]
                .chars()
                .next_back()
                .map(|c| c.is_alphanumeric())
                .unwrap_or(false);
        let after_ok = pos + nb >= h.len()
            || !h[pos + nb..]
                .chars()
                .next()
                .map(|c| c.is_alphanumeric())
                .unwrap_or(false);
        if before_ok && after_ok {
            return true;
        }
        // pos+1 may not be a char boundary in multibyte UTF-8 (CJK, etc.) → advance to the next
        // char boundary; otherwise slicing h[start..] panics.
        start = pos + h[pos..].chars().next().map_or(1, |c| c.len_utf8());
    }
    false
}

/// Does the text contain only uppercase alphabetic characters? (at least 2 letters, all uppercase)
/// Allocation-free: no intermediate Vec, short-circuits at the first lowercase letter. Threshold
/// and predicate are IDENTICAL → exactly the same result as the original for every input.
/// Letter count below which all-caps text is not a readability issue (#182).
///
/// 🔴 The threshold was selected by measurement, not intuition. In run-32311885577, **33% (708)**
/// of the 2,127 feeds where the rule fired had four letters or fewer, and their contents were
/// clearly organization abbreviations: YMCA, NVBW, DMV, CVS, CATA, AFPA, CCAS. ALL 2,394 findings
/// in `mdb-2389` were two three-letter place codes (NAN, VAN). An abbreviation has no lowercase
/// form; suggesting "Ymca" is wrong.
///
/// MobilityData is also silent on these feeds and reports MORE than us on shared feeds (1.11x),
/// so the threshold does not put us behind it.
const DQ018_MIN_LETTERS: usize = 5;

fn is_all_caps(s: &str) -> bool {
    let mut n = 0usize;
    for c in s.chars().filter(|c| c.is_alphabetic()) {
        if !c.is_uppercase() {
            return false;
        }
        n += 1;
    }
    n >= DQ018_MIN_LETTERS
}

// All letters lowercase — mixed_case_recommended_field (all-lowercase variant)
fn is_all_lower(s: &str) -> bool {
    let mut n = 0usize;
    for c in s.chars().filter(|c| c.is_alphabetic()) {
        if !c.is_lowercase() {
            return false;
        }
        n += 1;
    }
    n >= 3
}

// haversine_km / ymd_to_jdn / jdn_to_yyyymmdd → k5_derived (single canonical definition).
// These copies were identical to the K5 versions; the K5 version is covered by tests.
use crate::k5_derived::{haversine_km, jdn_to_yyyymmdd, ymd_to_jdn};

/// Returns the minimum distance in meters from a stop (slat, slon) to a shape polyline.
/// For each segment, computes the perpendicular distance; if the projection falls outside
/// the segment, uses the distance to the nearest endpoint.
fn point_to_polyline_dist_m(slat: f64, slon: f64, pts: &[(f64, f64)]) -> f64 {
    if pts.is_empty() {
        return f64::MAX;
    }
    if pts.len() == 1 {
        return haversine_km(slat, slon, pts[0].0, pts[0].1) * 1000.0;
    }
    const DEG_TO_M: f64 = 111_320.0;
    let cos_lat = slat.to_radians().cos();
    // Treat the stop as the local Cartesian origin (0,0).
    let to_local = |lat: f64, lon: f64| -> (f64, f64) {
        ((lon - slon) * cos_lat * DEG_TO_M, (lat - slat) * DEG_TO_M)
    };
    let mut min_dist = f64::MAX;
    for w in pts.windows(2) {
        let (ax, ay) = to_local(w[0].0, w[0].1);
        let (bx, by) = to_local(w[1].0, w[1].1);
        let (dx, dy) = (bx - ax, by - ay);
        let len_sq = dx * dx + dy * dy;
        let dist = if len_sq < 1e-6 {
            (ax * ax + ay * ay).sqrt()
        } else {
            let t = ((-ax * dx) + (-ay * dy)) / len_sq;
            let t = t.clamp(0.0, 1.0);
            let (cx, cy) = (ax + t * dx, ay + t * dy);
            (cx * cx + cy * cy).sqrt()
        };
        if dist < min_dist {
            min_dist = dist;
        }
    }
    min_dist
}

/// The display time for a stop_times row (arrival first, departure otherwise), "HH:MM:SS".
/// Used to show a neighboring stop's time on the planner card; None when both are absent.
fn row_time_text(st: &CompactStopTime) -> Option<String> {
    st.arrival_time().or_else(|| st.departure_time()).map(|t| format_hms(hms_to_secs(t)))
}

fn hms_to_secs(t: (u32, u32, u32)) -> u32 {
    t.0 * 3600 + t.1 * 60 + t.2
}

/// Returns the projected arc length (km) of stop (slat, slon) on the polyline.
/// `pts` are ordered shape points; `cum` is each point's cumulative distance from the start (cum[0]=0).
/// Projection details for SHP_016: the nearest point's (arc, distance) pair and ALSO the distance
/// of the nearest candidate in the first 10% of the shape.
///
/// Why separate: `project_arc_km` returns only the arc, and the "nearest point" alone is NOT
/// directional evidence. On a shape that crosses itself, the first stop can map to TWO places;
/// meters determine which is selected. Measured in the DART (Des Moines) feed — shape 58782,
/// trip 751448: the first stop was 6.8 m from segment 54 and 10.0 m from the shape START.
/// Because of the 3.1 m difference, the rule said "shape drawn backwards", while end-to-end
/// inspection showed the first stop 10 m from the shape start and the last stop 9 m from its end
/// — the route is CORRECT. The returned triple lets the caller see the ambiguity and stay silent.
fn project_arc_detail(
    pts: &[(f64, f64)],
    cum: &[f64],
    slat: f64,
    slon: f64,
) -> (f64, f64, Option<f64>) {
    let cos_lat = (slat.to_radians()).cos().max(0.001_f64);
    let scale = 111.0_f64 * cos_lat;
    let head_limit = cum.last().copied().unwrap_or(0.0) * 0.10;
    let mut best_arc = 0.0_f64;
    let mut best_dsq = f64::INFINITY;
    let mut head_dsq = f64::INFINITY;
    for w in 0..pts.len() - 1 {
        let (alat, alon) = pts[w];
        let (blat, blon) = pts[w + 1];
        let ax = (alon - slon) * scale;
        let ay = (alat - slat) * 111.0_f64;
        let dx = (blon - slon) * scale - ax;
        let dy = (blat - slat) * 111.0_f64 - ay;
        let len_sq = dx * dx + dy * dy;
        let t = if len_sq < 1e-12 {
            0.0_f64
        } else {
            ((-ax * dx) + (-ay * dy)) / len_sq
        }
        .clamp(0.0_f64, 1.0_f64);
        let nx = ax + t * dx;
        let ny = ay + t * dy;
        let dsq = nx * nx + ny * ny;
        let arc = cum[w] + t * (cum[w + 1] - cum[w]);
        if dsq < best_dsq {
            best_dsq = dsq;
            best_arc = arc;
        }
        if arc < head_limit && dsq < head_dsq {
            head_dsq = dsq;
        }
    }
    (
        best_arc,
        best_dsq.sqrt(),
        head_dsq.is_finite().then(|| head_dsq.sqrt()),
    )
}

fn project_arc_km(pts: &[(f64, f64)], cum: &[f64], slat: f64, slon: f64) -> f64 {
    let cos_lat = (slat.to_radians()).cos().max(0.001_f64);
    let scale = 111.0_f64 * cos_lat;
    let mut best_arc = 0.0_f64;
    let mut best_dsq = f64::INFINITY;
    for w in 0..pts.len() - 1 {
        let (alat, alon) = pts[w];
        let (blat, blon) = pts[w + 1];
        let ax = (alon - slon) * scale;
        let ay = (alat - slat) * 111.0_f64;
        let bx = (blon - slon) * scale;
        let by_ = (blat - slat) * 111.0_f64;
        let dx = bx - ax;
        let dy = by_ - ay;
        let len_sq = dx * dx + dy * dy;
        let t = if len_sq < 1e-12 {
            0.0_f64
        } else {
            ((-ax * dx) + (-ay * dy)) / len_sq
        }
        .clamp(0.0_f64, 1.0_f64);
        let nx = ax + t * dx;
        let ny = ay + t * dy;
        let dsq = nx * nx + ny * ny;
        if dsq < best_dsq {
            best_dsq = dsq;
            best_arc = cum[w] + t * (cum[w + 1] - cum[w]);
        }
    }
    best_arc
}

/// Minimum distance (km) from a stop point to a shape polyline by segment projection.
/// Finds the argmin in equirectangular space; calls haversine only once at the end.
/// `safe_sq`: threshold²×0.98 — returns 0.0 for a segment below this value (early exit).
fn seg_min_dist_km(pts: &[(f64, f64)], slat: f64, slon: f64, scale_lon: f64, safe_sq: f64) -> f64 {
    if pts.len() < 2 {
        return haversine_km(slat, slon, pts[0].0, pts[0].1);
    }
    let mut min_sq = f64::INFINITY;
    let mut best_lat = pts[0].0;
    let mut best_lon = pts[0].1;
    for w in pts.windows(2) {
        let (alat, alon) = w[0];
        let (blat, blon) = w[1];
        // Segment A→B, stop P=(0,0) in equirectangular km coords
        let ax = (alon - slon) * scale_lon;
        let ay = (alat - slat) * 111.0_f64;
        let bx = (blon - slon) * scale_lon;
        let by_ = (blat - slat) * 111.0_f64;
        let dx = bx - ax;
        let dy = by_ - ay;
        let len_sq = dx * dx + dy * dy;
        let (nx, ny, nlat, nlon) = if len_sq < 1e-12 {
            // Degenerate (zero-length) segment — use endpoint A
            (ax, ay, alat, alon)
        } else {
            // Scalar projection of P=(0,0) onto A+t*(B-A), clamp t∈[0,1]
            let t = ((-ax * dx) + (-ay * dy)) / len_sq;
            let t = t.clamp(0.0_f64, 1.0_f64);
            (
                ax + t * dx,
                ay + t * dy,
                alat + t * (blat - alat),
                alon + t * (blon - alon),
            )
        };
        let d_sq = nx * nx + ny * ny;
        if d_sq < min_sq {
            min_sq = d_sq;
            best_lat = nlat;
            best_lon = nlon;
        }
        if min_sq <= safe_sq {
            return 0.0;
        }
    }
    if min_sq <= safe_sq {
        0.0
    } else {
        haversine_km(slat, slon, best_lat, best_lon)
    }
}

/// Is this an intercity/long-distance rail route_type? (for STM_017 missing shape_dist and STM_026 threshold)
/// GTFS: 2=Rail, 12=Monorail, 100-117 extended train types.
fn is_rail_route_type(route_type: u32) -> bool {
    route_type == 2 || route_type == 12 || (100..=117).contains(&route_type)
}

/// Reduces an extended route_type (Google "Extended GTFS Route Types") to its base VEHICLE type.
/// Unlike `max_speed_kmh`, which combines bus/trolleybus/taxi for one speed threshold, trolleybus
/// (11/800) and monorail (12/405) remain separate here because the question is vehicle change.
/// Otokar (2xx) is a bus (3) in the base table. Unknown codes are returned unchanged.
fn route_type_vehicle_mode(route_type: u32) -> u32 {
    match route_type {
        0 | 900..=906 => 0,
        1 | 400..=404 => 1,
        2 | 100..=117 => 2,
        3 | 200..=209 | 700..=716 => 3,
        4 | 1000..=1021 | 1200 => 4,
        6 | 1300..=1307 => 6,
        7 | 1400 => 7,
        11 | 800 => 11,
        12 | 405 => 12,
        other => other,
    }
}

/// Read a raw CSV column without a typed field (e.g. `parent_station`, the K4 `row_field` counterpart).
fn row_field_local<'a>(row: &'a std::collections::HashMap<String, String>, key: &str) -> &'a str {
    row.get(key).map(String::as_str).unwrap_or("").trim()
}

/// Trip and shape IDs linked to rail routes — ONE setup at K6 entry.
pub(crate) struct RailIndex<'a> {
    /// trip_ids linked to a rail route (STM_045).
    pub trips: FxHashSet<&'a str>,
    /// shape_ids used by a rail route (GEO_006/007/009, SHP_012/014/024).
    pub shapes: FxHashSet<&'a str>,
}

/// shape_ids used by a rail route (GEO_006/007/009, SHP_012/014/024).
///
/// Rail geometry breaks urban assumptions in two ways: (1) at stations, stop coordinates represent
/// the platform/building center while the shape is one track centerline, and (2) intercity rail is
/// legitimately simplified on long straight sections. These shapes use `*_rail` thresholds.
///
/// **Mixed use chooses the lenient direction:** if ANY route using the shape is rail, the shape
/// counts as rail. A false positive for these rules (calling legitimate geometry an error) costs
/// more than a false negative; this matches the project's general FP-avoidance stance.
///
/// Performance: when there is no rail route (most feeds), trips are not traversed at all;
/// trip and shape sets are built in ONE pass.
fn rail_index(records: &EntityRecords) -> RailIndex<'_> {
    let rail_routes: FxHashSet<&str> = records
        .routes
        .iter()
        .filter(|r| r.route_type.is_some_and(is_rail_route_type))
        .map(|r| r.route_id.as_str())
        .collect();
    let mut out = RailIndex {
        trips: FxHashSet::default(),
        shapes: FxHashSet::default(),
    };
    if rail_routes.is_empty() {
        return out;
    }
    let ti = &records.trip_interns;
    for t in &records.trips {
        if !rail_routes.contains(ti.route_id(t)) {
            continue;
        }
        out.trips.insert(t.trip_id.as_str());
        if let Some(sid) = ti.shape_id(t).filter(|s| !s.is_empty()) {
            out.shapes.insert(sid);
        }
    }
    out
}

/// shape → stop-to-route distance threshold (m). For rail shapes, `stop_far_from_shape_m_rail`.
fn stop_shape_threshold_m(
    shape_id: &str,
    rail_shapes: &FxHashSet<&str>,
    cfg: &ValidatorConfig,
) -> f64 {
    if rail_shapes.contains(shape_id) {
        cfg.stop_far_from_shape_m_rail
    } else {
        cfg.stop_far_from_shape_m
    }
}

/// Get stop coordinates for K6 analytics.
///
/// K2's lexical validation deliberately rejects leading/trailing whitespace in a raw numeric value
/// and proves the DQ_016/STP derivative. K6 must not lose the numeric payload beyond that whitespace
/// root: MobilityData and most GTFS consumers parse `" 43.229610"` as a number and use the same
/// coordinate. K2's strict notice and raw-value evidence are unchanged; only the analytics input is recovered.
fn recoverable_stop_coord(stop: &crate::k2::stops::StopRecord) -> Option<(f64, f64)> {
    if let Some(coords) = stop.stop_lat.zip(stop.stop_lon) {
        return Some(coords);
    }
    let parse = |field: &str| {
        stop.row
            .get(field)
            .and_then(|raw| raw.trim().parse::<f64>().ok())
            .filter(|value| value.is_finite())
    };
    parse("stop_lat").zip(parse("stop_lon"))
}

/// shape → consecutive-point jump threshold (km). For rail shapes, `max_shape_jump_km_rail`.
fn shape_jump_threshold_km(
    shape_id: &str,
    rail_shapes: &FxHashSet<&str>,
    cfg: &ValidatorConfig,
) -> f64 {
    if rail_shapes.contains(shape_id) {
        cfg.max_shape_jump_km_rail
    } else {
        cfg.max_shape_jump_km
    }
}

/// route_type → STM_028 trip-duration upper threshold (seconds).
///
/// Separate (and higher) threshold for rail types: long-distance train schedules legitimately
/// exceed 24 hours. In the TCDD feed, Ankara–Kars 26:26, Ankara–Tatvan 26:37, Kurtalan–Ankara 27:25
/// — all six are real schedules, not data errors (confirmed by the feed owner). Urban types retain
/// 24 hours; for trams/metros, that duration signals a data error.
fn max_trip_duration_secs(route_type: u32, cfg: &ValidatorConfig) -> u32 {
    let hours = if is_rail_route_type(route_type) {
        cfg.max_trip_duration_hours_rail
    } else {
        cfg.max_trip_duration_hours
    };
    (hours * 3600.0) as u32
}

/// route_type → speed threshold from config (km/h).
/// #15/MD calibration: maps STANDARD (0-7,11,12) and EXTENDED European route_types (100-1700)
/// to the correct category. The old version recognized only standard types → feeds using only
/// extended codes, such as VBB Berlin, dropped rail (100-117 = S-Bahn/regional/intercity) to
/// `_ => bus` and used a 120 km/h threshold → legitimate 120-300 km/h trains became STM_014 FPs
/// (4530 vs MD 233). Now consistent with is_rail_route_type.
/// "Distant" stop threshold — same as MobilityData: two stops more than 10 km apart.
/// Pairs below it belong to neighboring-stop speed checks (STM_012/STM_014).
const FAR_STOP_MIN_KM: f64 = 10.0;

fn max_speed_kmh(route_type: u32, cfg: &ValidatorConfig) -> f64 {
    match route_type {
        0 | 900..=906 => cfg.max_speed_tram_kmh, // tram / light rail
        1 | 400..=405 => cfg.max_speed_metro_kmh, // metro / U-Bahn
        2 | 12 | 100..=117 => cfg.max_speed_rail_kmh, // rail / S-Bahn / regional
        3 | 11 | 200..=209 | 700..=716 | 800 | 1500..=1507 => cfg.max_speed_bus_kmh, // bus / coach / trolleybus / taxi
        4 | 1000..=1021 | 1200 => cfg.max_speed_ferry_kmh,                           // su / feribot
        5 | 6 | 7 | 1300..=1307 | 1400 => cfg.max_speed_cablecar_kmh, // cable car / funicular
        _ => cfg.max_speed_bus_kmh,                                   // unknown type → safe default
    }
}

// ── Shared shape index (one K6 pass) ──────────────────────────────────────────
// shape_id → sequence-sorted (lat, lon) + bbox. This setup had previously been copied EXACTLY
// THREE TIMES into check_remaining_analytics (`rem::build_maps`), check_shp012, and check_shp022;
// check_speed_and_duration (`sd::shape_setup`) built a fourth variant.
// Entur (mdb-1078, 40.9M shape points) measurement: build_maps 4.0s + shp012 ~3.0s + shp022 ~3.0s
// — 99.96% of shp022's total time was this setup (1.26ms of the 2,993ms body).
// In parallel mode (wasm32-threads), three copies could be live AT ONCE, so sharing also lowers
// peak memory; in serial mode one copy stays live across K6 (~16B/point).
struct ShapeIndex<'a> {
    coords: FxHashMap<&'a str, Vec<(f64, f64)>>,
    bbox: FxHashMap<&'a str, [f64; 4]>,
}

impl<'a> ShapeIndex<'a> {
    /// #15 (build_maps OOM/312s): shape_coords is built with POINT INDICES. Grouping each point
    /// in a (u32,f64,f64)=24B temporary buffer and then copying to (f64,f64)=16B created transient
    /// ~40B/nokta demekti; indeks (4B) + coords (16B) = ~20B/nokta tepe.
    /// Sorting is STABLE (`sort_by_key`) — file order is preserved for equal `shape_pt_sequence`.
    fn build(records: &'a EntityRecords) -> Self {
        let mut shape_pt_idx: FxHashMap<&str, Vec<u32>> = FxHashMap::default();
        for (i, sp) in records.shapes.iter().enumerate() {
            if sp.shape_pt_lat().is_some() && sp.shape_pt_lon().is_some() {
                shape_pt_idx
                    .entry(records.shape_interns.id(sp))
                    .or_default()
                    .push(i as u32);
            }
        }
        let n_shapes = shape_pt_idx.len();
        let mut coords: FxHashMap<&str, Vec<(f64, f64)>> = FxHashMap::default();
        let mut bbox: FxHashMap<&str, [f64; 4]> = FxHashMap::default();
        coords.reserve(n_shapes);
        bbox.reserve(n_shapes);
        for (sid, mut idxs) in shape_pt_idx {
            idxs.sort_by_key(|&i| records.shapes[i as usize].shape_pt_sequence().unwrap_or(0));
            let pts: Vec<(f64, f64)> = idxs
                .iter()
                .map(|&i| {
                    let sp = &records.shapes[i as usize];
                    (sp.shape_pt_lat().unwrap(), sp.shape_pt_lon().unwrap())
                })
                .collect();
            if !pts.is_empty() {
                let mut mn_lat = pts[0].0;
                let mut mx_lat = pts[0].0;
                let mut mn_lon = pts[0].1;
                let mut mx_lon = pts[0].1;
                for &(la, lo) in pts.iter().skip(1) {
                    if la < mn_lat {
                        mn_lat = la;
                    }
                    if la > mx_lat {
                        mx_lat = la;
                    }
                    if lo < mn_lon {
                        mn_lon = lo;
                    }
                    if lo > mx_lon {
                        mx_lon = lo;
                    }
                }
                bbox.insert(sid, [mn_lat, mx_lat, mn_lon, mx_lon]);
            }
            coords.insert(sid, pts);
        }
        Self { coords, bbox }
    }
}

// ── Shared stop_times index (one K6 pass) ─────────────────────────────────────
// Note: by_trip now carries &CompactStopTime references from the K2 StopTimesIndex.
// The records.stop_times Vec<StopTimeRecord> is not scanned; only the index is used.

struct StopTimesIndex<'a> {
    by_trip: FxHashMap<&'a str, &'a [CompactStopTime]>,
    trip_first_dep: FxHashMap<&'a str, u32>,
    // Shape ids are already interned by K2; retaining u32 indices halves the
    // per-association storage versus Vec<&str> and avoids pointer-heavy peaks.
    stop_shapes: FxHashMap<&'a str, Vec<u32>>,
    trips_missing_sdt: FxHashMap<&'a str, u64>,
    // STM_036: unsorted stop_sequence information detected in K2
    unsorted_seq_trips: &'a Vec<(SmolStr, u32, u32, u64)>,
    // Reference to the K2 intern table: for stop_id, headsign, and Flex access.
    k2: &'a K2StopTimesIndex,
}

impl<'a> StopTimesIndex<'a> {
    fn build(
        records: &'a EntityRecords,
        trip_shape: &HashMap<&'a str, u32>,
        k2: &'a K2StopTimesIndex,
    ) -> Self {
        let n_trips = records.trips.len();
        let n_stops = records.stops.len();

        // BORROW K2 index slices (no clone); the test path supplies a fallback K2 reference.
        let mut by_trip: FxHashMap<&'a str, &'a [CompactStopTime]> = FxHashMap::default();
        let mut stop_shapes: FxHashMap<&'a str, Vec<u32>> = FxHashMap::default();
        by_trip.reserve(n_trips);
        stop_shapes.reserve(n_stops);

        for (trip_id, stops) in k2.iter_trips() {
            let tid: &'a str = trip_id.as_str();
            by_trip.insert(tid, stops);
                if let Some(&shape_idx) = trip_shape.get(tid) {
                for st in stops {
                    let sid = k2.stop_id_of(st);
                    if !sid.is_empty() {
                            stop_shapes.entry(sid).or_default().push(shape_idx);
                    }
                }
            }
        }

        for v in stop_shapes.values_mut() {
            v.sort_unstable();
            v.dedup();
        }

        // Post-build: trips_missing_sdt
        let mut trips_missing_sdt: FxHashMap<&'a str, u64> = FxHashMap::default();
        trips_missing_sdt.reserve(n_trips / 4);
        for (&tid, v) in &by_trip {
            if trip_shape.contains_key(tid) {
                if let Some(st) = v.iter().find(|s| s.shape_dist_traveled().is_none()) {
                    trips_missing_sdt.insert(tid, st.line as u64);
                }
            }
        }

        let trip_first_dep: FxHashMap<&'a str, u32> = by_trip
            .iter()
            .filter_map(|(&tid, v)| {
                v.first()
                    .and_then(|s| s.departure_time())
                    .map(|dep| (tid, hms_to_secs(dep)))
            })
            .collect();

        Self {
            by_trip,
            trip_first_dep,
            stop_shapes,
            trips_missing_sdt,
            unsorted_seq_trips: &k2.unsorted_seq_trips,
            k2,
        }
    }

    // Stop_id helper — delegates to the K2 intern table.
    #[inline]
    fn stop_id_of(&self, st: &CompactStopTime) -> &str {
        self.k2.stop_id_of(st)
    }
}

// ── WP-09a: Speed anomaly + trip duration ────────────────────────────────────

/// STM_014 pattern aggregation: speed exceedances accumulated per (route, direction, segment).
///
/// The same physical segment fired separately for EVERY trip passing it; all 604 TriMet notices
/// came from one segment (Route 20, stops 241→255) and showed the same root cause (schedule/shape
/// inconsistency on the segment). Detection logic is UNCHANGED — only emission is aggregated by pattern.
/// (Reliability Sprint Checkpoint 6 decision: "not a correctness issue, but a presentation and
/// dedup ergonomics issue; can be handled as pattern-level grouping".)
struct Stm014Seg<'a> {
    route_label: &'a str,
    seq_a: u32,
    seq_b: u32,
    threshold: f64,
    /// Smallest stop_times row — for deterministic output (map order is random).
    line: u64,
    speed_min: f64,
    speed_max: f64,
    distance_min_km: f64,
    distance_max_km: f64,
    trips: Vec<&'a str>,
    /// At least one contribution exceeds the threshold even without the route type being lost to whitespace.
    ws_real: bool,
    /// If all contributions exceed the threshold only because of the lost route type, this is the whitespace file.
    ws_file: Option<&'static str>,
}

/// The finding arose only from an ID join broken by whitespace. K7 declares it under the `DQ_016`
/// root of the file carrying the whitespace; if there is no root, the finding remains and this key is visible.
/// The calculation is UNCHANGED (#85 keeps IDs raw); only whether it arises in the trimmed world is tested.
fn mark_whitespace_join(n: &mut Notice, file: &'static str) {
    n.details
        .get_or_insert_with(Default::default)
        .insert("whitespace_join_file".to_string(), file.to_string());
}

/// Number of example trips to show in the details["trips"] list (the full count is trip_count).
const STM014_TRIP_SAMPLE: usize = 12;

/// STM_061 aggregation: impossible speeds accumulated per (route, direction, distant stop pair).
///
/// The rule produced one finding per trip, which was NOT ENOUGH: the same physical stop pair was
/// reported again for EVERY trip passing it. In mdb-1003, all 9,533 findings came from ONE pair
/// (Selva de Mar → Rambla de Sant Just, 11.8 km, 118 km/h) and showed the same root cause — stops
/// from Barcelona's two unconnected tram networks were placed on the same trip. The same pattern
/// produced 10,390 findings in mdb-1004. Detection logic is UNCHANGED; only emission is aggregated
/// at pair level, exactly like STM_014 segment aggregation.
struct Stm061Pair<'a> {
    route_label: &'a str,
    seq_a: u32,
    seq_b: u32,
    threshold: f64,
    dist_km: f64,
    /// Smallest stop_times row — for deterministic output (map order is random).
    line: u64,
    speed_min: f64,
    speed_max: f64,
    trips: Vec<&'a str>,
}

fn check_speed_and_duration<'a>(
    records: &'a EntityRecords,
    config: &ValidatorConfig,
    idx: &StopTimesIndex<'_>,
    shape_idx: &ShapeIndex<'a>,
    notices: &mut Vec<Notice>,
    ctr: &mut u32,
) {
    use crate::timing::Timer;
    let ti_sd = &records.trip_interns;
    let mut stm007_pending: Vec<Notice> = Vec::new();

    let (
        stop_coords,
        stop_name_sd,
        trip_route_type,
        trip_to_route,
        route_short_sd,
        trip_direction,
        trip_headsign_sd,
        trip_service,
        ws_route_type,
    ) = {
        let _t = Timer::start("K6::sd::setup");
        // stop_id → (lat, lon)  FxHashMap: SipHash yerine multiply-xor
        let mut stop_coords: FxHashMap<&str, (f64, f64)> = FxHashMap::default();
        stop_coords.reserve(records.stops.len());
        for s in &records.stops {
            if let Some(coords) = recoverable_stop_coord(s) {
                stop_coords.insert(s.stop_id.as_str(), coords);
            }
        }

        // stop_id → stop_name (for display; omitted when empty/missing, fallback stop_id)
        let mut stop_name_sd: FxHashMap<&str, &str> = FxHashMap::default();
        stop_name_sd.reserve(records.stops.len());
        for s in &records.stops {
            if let Some(name) = s.stop_name.as_deref().filter(|n| !n.is_empty()) {
                stop_name_sd.insert(s.stop_id.as_str(), name);
            }
        }

        // route_id → route_type
        let route_type_map: HashMap<&str, u32> = records
            .routes
            .iter()
            .filter_map(|r| r.route_type.map(|rt| (r.route_id.as_str(), rt)))
            .collect();

        // trip_id → route_type
        let mut trip_route_type: FxHashMap<&str, u32> = FxHashMap::default();
        trip_route_type.reserve(records.trips.len());
        for t in &records.trips {
            if let Some(&rt) = route_type_map.get(ti_sd.route_id(t)) {
                trip_route_type.insert(t.trip_id.as_str(), rt);
            }
        }

        // trip_id → route_id
        let mut trip_to_route: FxHashMap<&str, &str> = FxHashMap::default();
        trip_to_route.reserve(records.trips.len());
        for t in &records.trips {
            trip_to_route.insert(t.trip_id.as_str(), ti_sd.route_id(t));
        }

        // Route type is lost ONLY because of a whitespace ID (`mdb-1065`: padded `routes.route_id`
        // made 243 rail routes fall to the bus threshold). The threshold remains calculated from the raw
        // type; this map tests only whether the finding also arises in the trimmed world.
        // It is not built when there is no miss.
        let mut ws_route_type: FxHashMap<&str, (u32, &'static str)> = FxHashMap::default();
        let stop_times_trip_missing = idx.by_trip.keys().any(|t| !trip_to_route.contains_key(t));
        if trip_route_type.len() < records.trips.len() || stop_times_trip_missing {
            let mut routes_trimmed: FxHashMap<&str, (u32, bool)> = FxHashMap::default();
            for r in &records.routes {
                if let Some(rt) = r.route_type {
                    routes_trimmed
                        .entry(r.route_id.trim())
                        .or_insert((rt, r.route_id != r.route_id.trim()));
                }
            }
            for t in &records.trips {
                if trip_route_type.contains_key(t.trip_id.as_str()) {
                    continue;
                }
                if let Some(&(rt, padded)) = routes_trimmed.get(ti_sd.route_id(t).trim()) {
                    let file = if padded { "routes.txt" } else { "trips.txt" };
                    ws_route_type.insert(t.trip_id.as_str(), (rt, file));
                }
            }
            // The trip ID in stop_times differs from trips.txt only by whitespace.
            if stop_times_trip_missing {
                let trips_trimmed: FxHashMap<&str, &crate::k2::trips::TripRecord> = records
                    .trips
                    .iter()
                    .map(|t| (t.trip_id.trim(), t))
                    .collect();
                for &trip_id in idx.by_trip.keys() {
                    if trip_to_route.contains_key(trip_id) {
                        continue;
                    }
                    let Some(trip) = trips_trimmed.get(trip_id.trim()) else {
                        continue;
                    };
                    let route = ti_sd.route_id(trip);
                    let rt = route_type_map
                        .get(route)
                        .copied()
                        .or_else(|| routes_trimmed.get(route.trim()).map(|&(rt, _)| rt));
                    if let Some(rt) = rt {
                        let file = if trip.trip_id != trip.trip_id.trim() {
                            "trips.txt"
                        } else {
                            "stop_times.txt"
                        };
                        ws_route_type.insert(trip_id, (rt, file));
                    }
                }
            }
        }

        // route_id → display name
        let route_short_sd: FxHashMap<&str, &str> = records
            .routes
            .iter()
            .map(|r| {
                let label = r
                    .route_short_name
                    .as_deref()
                    .filter(|s| !s.is_empty())
                    .unwrap_or(r.route_id.as_str());
                (r.route_id.as_str(), label)
            })
            .collect();

        // trip_id → direction_id (for display)
        let mut trip_direction: FxHashMap<&str, &'static str> = FxHashMap::default();
        trip_direction.reserve(records.trips.len());
        for t in &records.trips {
            let dir = match t.direction_id {
                Some(0) => "0",
                Some(1) => "1",
                _ => "-",
            };
            trip_direction.insert(t.trip_id.as_str(), dir);
        }

        // trip_id → service_id
        let mut trip_service: FxHashMap<&str, &str> = FxHashMap::default();
        trip_service.reserve(records.trips.len());
        for t in &records.trips {
            trip_service.insert(t.trip_id.as_str(), ti_sd.service_id(t));
        }

        // trip_id → trip_headsign (for display; omitted when empty/missing)
        let mut trip_headsign_sd: FxHashMap<&str, &str> = FxHashMap::default();
        trip_headsign_sd.reserve(records.trips.len());
        for t in &records.trips {
            if let Some(hs) = ti_sd.headsign(t).filter(|h| !h.is_empty()) {
                trip_headsign_sd.insert(t.trip_id.as_str(), hs);
            }
        }

        (
            stop_coords,
            stop_name_sd,
            trip_route_type,
            trip_to_route,
            route_short_sd,
            trip_direction,
            trip_headsign_sd,
            trip_service,
            ws_route_type,
        )
    };

    // Shape-based segment distance infrastructure (for speed calculation).
    // Priority: shape polyline projection > haversine fallback.
    // Shape points are borrowed from the shared ShapeIndex created at K6 entry; only cumulative
    // distance (haversine) is calculated here. The old path built a fourth copy and used `sort_unstable`
    // → order was ambiguous for equal `shape_pt_sequence`; the shared index is STABLE-sorted (file
    // order preserved), so this path is now deterministic.
    let shape_pts_speed = &shape_idx.coords;
    let (shape_cum_speed, trip_shape_speed) = {
        let _t = Timer::start("K6::sd::shape_setup");
        let shape_cum: FxHashMap<&str, Vec<f64>> = shape_pts_speed
            .iter()
            .map(|(&sid, pts)| {
                let mut c = Vec::with_capacity(pts.len());
                c.push(0.0_f64);
                for i in 1..pts.len() {
                    c.push(c[i - 1] + haversine_km(pts[i - 1].0, pts[i - 1].1, pts[i].0, pts[i].1));
                }
                (sid, c)
            })
            .collect();
        let trip_shape: FxHashMap<&str, &str> = records
            .trips
            .iter()
            .filter_map(|t| {
                ti_sd
                    .shape_id(t)
                    .filter(|s| !s.is_empty())
                    .map(|s| (t.trip_id.as_str(), s))
            })
            .collect();
        (shape_cum, trip_shape)
    };

    // STM_036: stop_times is not ordered/grouped by trip_id + stop_sequence (MD unsorted_stop_times,
    // INFO — benign: the pipeline already reorders by sequence). Two subcases:
    //   (a) sequence DECREASES — detected AND emitted in K2 (k2/stop_times.rs). Emission stays there
    //       because prev_stop/curr_stop are available only while reading a row; moving it here would
    //       require adding two SmolStr values to unsorted_seq_trips (~+120% memory).
    //       Here those trips are read only to be EXCLUDED from (b); no second notice is produced.
    //   (b) rows are NON-CONTIGUOUS (another trip intervened; not grouped even if sequence increases)
    let stm036_seen: std::collections::HashSet<&str> = idx
        .unsorted_seq_trips
        .iter()
        .map(|(trip_id, ..)| trip_id.as_str())
        .collect();
    // (b) non-contiguous: are a trip's CSV lines consecutive in the finalized index
    // (max-min+1 == row count) — if not, another trip intervened. Sorted by first row for determinism.
    let mut noncontig: Vec<(&str, u32, u32)> = Vec::new();
    for (trip_id, stops) in idx.by_trip.iter() {
        if stm036_seen.contains(*trip_id) {
            continue;
        }
        if stops.len() < 2 {
            continue;
        }
        let (mut lo, mut hi) = (u32::MAX, 0u32);
        for r in *stops {
            lo = lo.min(r.line);
            hi = hi.max(r.line);
        }
        if (hi - lo + 1) as usize != stops.len() {
            noncontig.push((*trip_id, lo, hi));
        }
    }
    noncontig.sort_by_key(|&(_, lo, _)| lo);
    for (trip_id, lo, hi) in noncontig {
        notices.push(k6_notice(
            ctr, "STM_036", EntityType::Trip,
            Some(trip_id.to_string()), Some(trip_id.to_string()),
            "stop_times.txt", Some(lo as u64), Some("stop_sequence"),
            Some(format!("line_range={lo}-{hi}")), Some("trip'e göre gruplu".to_string()),
            format!("'{trip_id}' seferinin stop_times satırları dosyada dağınık (trip_id'ye göre gruplu değil); {lo}–{hi} arasına yayılmış."),
            "stop_times.txt'i trip_id ve stop_sequence'a göre sıralayın.",
        ));
    }

    // Reusable per-trip coordinate buffer: 1 lookup/stop (windows(2) would mean 2 lookups/pair).
    let mut coords_buf: Vec<Option<(f64, f64)>> = Vec::new();

    // Performance: memoize shape arc-length projection for (shape_id, stop_id).
    // project_arc_km walks all shape points; the same (shape, stop) was projected repeatedly across
    // trips/segments. dist_km output is EXACTLY the same — only repetition is removed.
    let mut arc_cache: FxHashMap<(&str, &str), f64> = FxHashMap::default();
    // STM_014 aggregation: (route_id, direction, stop_a, stop_b) → segment summary.
    // NO emission inside the loop; emit one by one in sorted order after the loop.
    let mut stm014_segs: FxHashMap<(&str, &str, &str, &str), Stm014Seg<'_>> = FxHashMap::default();
    // STM_061 aggregation: (route_id, direction, stop_a, stop_b) → distant-pair summary.
    // The key has the SAME form as STM_014; both rules see the same physical segment with different
    // measures, and neither should scale with trip count.
    let mut stm061_pairs: FxHashMap<(&str, &str, &str, &str), Stm061Pair<'_>> =
        FxHashMap::default();
    // STM_053: collect affected trips within one route into one route-level notice.
    // Detection and threshold are unchanged; only per-trip Quality noise moves into the Analytics summary.
    let mut stm053_routes: FxHashMap<&str, (usize, usize, &str, u64)> = FxHashMap::default();

    {
        let _t = Timer::start("K6::sd::loop");
        for (&trip_id, stimes) in &idx.by_trip {
            if stimes.len() < 2 {
                continue;
            }
            let route_type = trip_route_type.get(trip_id).copied().unwrap_or(3);
            let threshold = max_speed_kmh(route_type, config);
            let ws_join = ws_route_type
                .get(trip_id)
                .map(|&(rt, file)| (max_speed_kmh(rt, config), file));
            let route = trip_to_route.get(trip_id).copied().unwrap_or(trip_id);
            let route_label = route_short_sd.get(route).copied().unwrap_or(route);
            let dir_sd = trip_direction.get(trip_id).copied().unwrap_or("-");
            let svc_sd = trip_service.get(trip_id).copied().unwrap_or("-");
            let dep_str = stimes
                .first()
                .and_then(|s| s.departure_time())
                .map(|(h, m, _)| format!("{h:02}:{m:02}"))
                .unwrap_or_default();
            // trip_id enrichment: human-readable departure-time suffix (" 08:15 departure"); empty when unknown.
            // (Same form as the existing pattern in the STM_026 message.)
            let dep_suffix = if dep_str.is_empty() {
                String::new()
            } else {
                format!(" {dep_str} kalkışlı")
            };

            // ── STM_007: departure < arrival at the same stop (moved from K2) ─────────────
            // A final stop crossing midnight (00:xx departure, should have been written as 24:xx) is
            // filtered with service_day_start; real within-row errors use the enriched message.
            {
                let sds_secs = config.service_day_start_hour * 3600;
                for (row_index, st) in stimes.iter().enumerate() {
                    let (Some(arr), Some(dep)) = (st.arrival_time(), st.departure_time()) else {
                        continue;
                    };
                    let arr_s = hms_to_secs(arr);
                    let dep_s = hms_to_secs(dep);
                    if dep_s >= arr_s {
                        continue;
                    }
                    // If adding +24h to departure makes it close to arrival → unintended midnight crossing → skip.
                    if sds_secs > 0 && (dep_s + 86400).saturating_sub(arr_s) <= sds_secs {
                        continue;
                    }
                    let seq = st.stop_sequence().unwrap_or(0);
                    let name = stop_name_sd
                        .get(idx.stop_id_of(st))
                        .copied()
                        .unwrap_or(idx.stop_id_of(st));
                    let mut n = k6_notice(
                    ctr, "STM_007", EntityType::Trip,
                    Some(trip_id.to_string()), Some(trip_id.to_string()),
                    "stop_times.txt", Some(st.line as u64), Some("departure_time"),
                    Some(format!("{} < {}", format_hms(dep_s), format_hms(arr_s))),
                    Some(format!(">= {}", format_hms(arr_s))),
                    format!("'{route_label}' hattı '{trip_id}'{dep_suffix} seferi: {name} durağında (sıra {seq}) kalkış {} < varış {} — kalkış varıştan önce. (yön {dir_sd})",
                        format_hms(dep_s), format_hms(arr_s)),
                    "stop_times.txt'te kalkış zamanını varış zamanından sonra (veya eşit) ayarlayın.",
                );
                    let mut d = std::collections::BTreeMap::new();
                    d.insert("route".to_string(), route_label.to_string());
                    d.insert("stop_name".to_string(), name.to_string());
                    d.insert("seq".to_string(), seq.to_string());
                    d.insert("dir".to_string(), dir_sd.to_string());
                    d.insert("dep".to_string(), format_hms(dep_s));
                    d.insert("arr".to_string(), format_hms(arr_s));
                    // Let the planner show the bad row with neighbors: the stop and the previous/next row's times.
                    d.insert("stop_id".to_string(), idx.stop_id_of(st).to_string());
                    let neighbours = [("prev", row_index.checked_sub(1)), ("next", Some(row_index + 1))];
                    for (side, other) in neighbours {
                        let Some(other) = other.and_then(|j| stimes.get(j)) else {
                            continue;
                        };
                        d.insert(format!("{side}_stop_id"), idx.stop_id_of(other).to_string());
                        if let Some(t) = other.arrival_time() {
                            d.insert(format!("{side}_arrival"), format_hms(hms_to_secs(t)));
                        }
                        if let Some(t) = other.departure_time() {
                            d.insert(format!("{side}_departure"), format_hms(hms_to_secs(t)));
                        }
                    }
                    // The feed-summary branch (`finalize_stm007_pending`) writes affected_rows/example_trips;
                    // because one template serves both branches, the single-item branch must carry them too, or
                    // the summary notice's en/ja/fr text is emitted with an empty placeholder (measured in the corpus).
                    d.insert("affected_rows".to_string(), "1".to_string());
                    d.insert("example_trips".to_string(), trip_id.to_string());
                    n.details = Some(d.into());
                    n.service_id = Some(svc_sd.to_string());
                    stm007_pending.push(n);
                }
            }

            // ── STM_028 / STM_029: trip duration ──────────────────────────────────────────
            let first_dep = stimes
                .first()
                .and_then(|s| s.departure_time())
                .map(hms_to_secs);
            let last_arr = stimes
                .last()
                .and_then(|s| s.arrival_time())
                .map(hms_to_secs);

            if let (Some(dep), Some(arr)) = (first_dep, last_arr) {
                if arr >= dep {
                    let duration_sec = arr - dep;
                    // Separate threshold + Info severity for rail types: a train trip exceeding 24 hours
                    // is legitimate; the signal means "inspect, but do not panic".
                    let rt = trip_route_type.get(trip_id).copied().unwrap_or(u32::MAX);
                    let is_rail = is_rail_route_type(rt);
                    let max_sec = max_trip_duration_secs(rt, config);

                    if duration_sec > max_sec {
                        let mut n = k6_notice(
                        ctr,
                        "STM_028",
                        EntityType::Trip,
                        Some(trip_id.to_string()),
                        Some(trip_id.to_string()),
                        "stop_times.txt",
                        stimes.last().map(|s| s.line as u64),
                        Some("arrival_time"),
                        Some(format_hms(duration_sec)),
                        Some(format_hms(max_sec)),
                        format!("'{}' hattının{} seferinin toplam süresi {} — eşik {}.",
                            route,
                            if dep_str.is_empty() { String::new() } else { format!(" {dep_str}") },
                            format_hms(duration_sec), format_hms(max_sec)),
                        // One message: the locale dictionary has ONE remediation key per rule; a conditional variant
                        // cannot be represented with --lang en/ja (see the i18n architecture).
                        "İlk kalkış ve son varış saatlerini doğrulayın; uzun mesafe tarifelerinde bu süre olağandır, şehir içi hatlarda veri hatası işaretidir.",
                    );
                        if is_rail {
                            n.severity = gtfs_core::Severity::Bilgi;
                        }
                        if let (Some(first), Some(last)) = (stimes.first(), stimes.last()) {
                            n.details = Some(trip_span_details(idx, first, last));
                        }
                        notices.push(n);
                    }

                    if duration_sec < config.min_trip_duration_sec {
                        let mut n = k6_notice(
                        ctr,
                        "STM_029",
                        EntityType::Trip,
                        Some(trip_id.to_string()),
                        Some(trip_id.to_string()),
                        "stop_times.txt",
                        stimes.last().map(|s| s.line as u64),
                        Some("departure_time"),
                        Some(format!("{duration_sec}s")),
                        Some(format!("≥ {}s", config.min_trip_duration_sec)),
                        format!("'{}' hattının{} seferinin toplam süresi {}sn — eşik {}sn.",
                            route,
                            if dep_str.is_empty() { String::new() } else { format!(" {dep_str}") },
                            duration_sec, config.min_trip_duration_sec),
                        "stop_times zaman değerlerini kontrol edin; sefer gerçekten bu kadar kısa olmamalı.",
                    );
                        if let (Some(first), Some(last)) = (stimes.first(), stimes.last()) {
                            n.details = Some(trip_span_details(idx, first, last));
                        }
                        notices.push(n);
                    }
                }
            }

            let mut same_run = 1usize;
            let mut max_same_run = 1usize;
            let mut previous_time: Option<u32> = None;
            for st in stimes.iter() {
                let event_time = st
                    .arrival_time()
                    .or_else(|| st.departure_time())
                    .map(hms_to_secs);
                if event_time.is_some() && event_time == previous_time {
                    same_run += 1;
                    max_same_run = max_same_run.max(same_run);
                } else {
                    same_run = 1;
                }
                previous_time = event_time;
            }
            if max_same_run >= 3 {
                let first_line = stimes.first().map(|s| s.line as u64).unwrap_or(0);
                let entry = stm053_routes.entry(route).or_insert((0, 0, trip_id, first_line));
                entry.0 = entry.0.max(max_same_run);
                entry.1 += 1;
                // Representative = trip with the fewest rows. `by_trip` is an FxHashMap and FxHash
                // depends on `usize` width: the "first seen" trip differed between 64-bit native and
                // 32-bit WASM (same feed, different line/example_trip).
                if (first_line == 0, first_line, trip_id) < (entry.3 == 0, entry.3, entry.2) {
                    entry.2 = trip_id;
                    entry.3 = first_line;
                }
            }

            // ── STM_008: chronology across untimed stops ───────────────────────
            // MobilityData keeps the latest row with a known departure_time and
            // compares each later known arrival_time against it.  A stop without
            // times must not erase that state: interpolated/untimed intermediate
            // stops are valid GTFS and can otherwise hide a real reversal.
            //
            // This pass is deliberately separate from the adjacent-segment speed
            // loop below.  Speed and zero-duration checks still require a physically
            // adjacent pair, while chronology may span any number of untimed rows.
            let mut previous_departure: Option<(&CompactStopTime, u32, usize)> = None;
            for (row_index, st) in stimes.iter().enumerate() {
                if let (Some(arrival), Some((departure_row, departure, departure_index))) =
                    (st.arrival_time().map(hms_to_secs), previous_departure)
                {
                    if arrival < departure {
                        let seq_a = departure_row.stop_sequence().unwrap_or(0);
                        let seq_b = st.stop_sequence().unwrap_or(0);
                        let dep_hms = format_hms(departure);
                        let arr_hms = format_hms(arrival);
                        let name_a = stop_name_sd
                            .get(idx.stop_id_of(departure_row))
                            .copied()
                            .unwrap_or(idx.stop_id_of(departure_row));
                        let name_b = stop_name_sd
                            .get(idx.stop_id_of(st))
                            .copied()
                            .unwrap_or(idx.stop_id_of(st));
                        let headsign = trip_headsign_sd.get(trip_id).copied().unwrap_or("");
                        let hs_sep = if headsign.is_empty() { "" } else { ", " };
                        let mut n = k6_notice(
                        ctr, "STM_008", EntityType::Trip,
                        Some(trip_id.to_string()), Some(trip_id.to_string()),
                        "stop_times.txt", Some(st.line as u64), Some("arrival_time"),
                        Some(format!("{arr_hms} < {dep_hms}")),
                        Some(format!(">= {dep_hms}")),
                        format!("'{route_label}' hattı '{trip_id}'{dep_suffix} seferi: {name_a} durağından (sıra {seq_a}) kalkış {dep_hms}, {name_b} durağına (sıra {seq_b}) varış {arr_hms} — varış kalkıştan önce, zaman geriye gidiyor. (yön {dir_sd}{hs_sep}{headsign})"),
                        "stop_times.txt zaman değerlerini gözden geçirin; seferler boyunca zamanlar monoton artmalıdır.",
                    );
                        let mut d = std::collections::BTreeMap::new();
                        d.insert(
                            "stop_a".to_string(),
                            idx.stop_id_of(departure_row).to_string(),
                        );
                        d.insert("stop_b".to_string(), idx.stop_id_of(st).to_string());
                        d.insert("stop_a_name".to_string(), name_a.to_string());
                        d.insert("stop_b_name".to_string(), name_b.to_string());
                        d.insert("seq_a".to_string(), seq_a.to_string());
                        d.insert("seq_b".to_string(), seq_b.to_string());
                        d.insert("route".to_string(), route_label.to_string());
                        d.insert("headsign".to_string(), headsign.to_string());
                        d.insert("hs_sep".to_string(), hs_sep.to_string());
                        d.insert("dir".to_string(), dir_sd.to_string());
                        d.insert("dep".to_string(), dep_hms.clone());
                        d.insert("arr".to_string(), arr_hms.clone());
                        // Let the planner show the bad time in context: the stop before departure and
                        // the stop after arrival, with their times.
                        if let Some(prev) = departure_index.checked_sub(1).and_then(|i| stimes.get(i)) {
                            d.insert("prev_stop_id".to_string(), idx.stop_id_of(prev).to_string());
                            if let Some(time) = row_time_text(prev) {
                                d.insert("prev_time".to_string(), time);
                            }
                        }
                        if let Some(next) = stimes.get(row_index + 1) {
                            d.insert("next_stop_id".to_string(), idx.stop_id_of(next).to_string());
                            if let Some(time) = row_time_text(next) {
                                d.insert("next_time".to_string(), time);
                            }
                        }
                        n.details = Some(d.into());
                        notices.push(n);
                    }
                }
                if let Some(departure) = st.departure_time() {
                    previous_departure = Some((st, hms_to_secs(departure), row_index));
                }
            }

            // ── STM_014 / OPR_008: speed anomaly ─────────────────────────────────
            // Per-trip coordinate buffer: query each stop once.
            coords_buf.clear();
            coords_buf.extend(
                stimes
                    .iter()
                    .map(|s| stop_coords.get(idx.stop_id_of(s)).copied()),
            );

            // ── STM_061: impossible speed between a NON-CONSECUTIVE distant stop pair ────
            //
            // STM_012/STM_014 compare only NEIGHBORING stops. A trip can look reasonable step by step
            // but be impossible end to end: every segment stays below the threshold while the total route
            // cannot be physically covered.
            //
            // 🔴 I THOUGHT "if neighbors are clean, the distant pair is clean too"; THE CORPUS DISPROVED IT.
            // The argument was: if every segment ≤ T, then route_distance ≤ T·total_time, bird's-eye distance
            // ≤ route distance, so the distant pair is also ≤ T. But the two checks do NOT use the same distance
            // measure — when neighboring distance is measured along the (long) shape, speed comes out low.
            // In run-32197267205, **20 feeds** produce distant-pair findings and NO neighboring findings (#168).
            //
            // ⚡ PERF: the naive form is O(n²) per trip; in VBB, 282k trips × ~22 stops is ~68M pairs.
            // The bound here is NOT APPROXIMATE, but EXACT: `bbox_diag_km` is the MAXIMUM possible bird's-eye
            // distance between two stops in the trip, so once `threshold * Δt >= bbox_diag_km`, no later `j`
            // CAN violate the rule and the loop breaks. No finding is lost.
            {
                let mut lo_lat = f64::INFINITY;
                let mut hi_lat = f64::NEG_INFINITY;
                let mut lo_lon = f64::INFINITY;
                let mut hi_lon = f64::NEG_INFINITY;
                for c in coords_buf.iter().flatten() {
                    lo_lat = lo_lat.min(c.0);
                    hi_lat = hi_lat.max(c.0);
                    lo_lon = lo_lon.min(c.1);
                    hi_lon = hi_lon.max(c.1);
                }
                if lo_lat.is_finite() {
                    let bbox_diag_km = haversine_km(lo_lat, lo_lon, hi_lat, hi_lon);
                    // The 10 km "distant" definition matches MobilityData; pairs below it
                    // belong to the neighboring-stop check and are measured there already.
                    if bbox_diag_km > FAR_STOP_MIN_KM {
                        let mut worst: Option<(f64, f64, usize, usize)> = None;
                        for i in 0..stimes.len() {
                            let Some((la1, lo1)) = coords_buf[i] else {
                                continue;
                            };
                            let Some(dep) = stimes[i].departure_time().map(hms_to_secs) else {
                                continue;
                            };
                            for j in (i + 2)..stimes.len() {
                                let Some(arr) = stimes[j].arrival_time().map(hms_to_secs) else {
                                    continue;
                                };
                                if arr <= dep {
                                    continue;
                                }
                                let dt_h = (arr - dep) as f64 / 3600.0;
                                // HARD BOUND: no later pair can violate the rule.
                                if threshold * dt_h >= bbox_diag_km {
                                    break;
                                }
                                let Some((la2, lo2)) = coords_buf[j] else {
                                    continue;
                                };
                                let dist_km = haversine_km(la1, lo1, la2, lo2);
                                if dist_km <= FAR_STOP_MIN_KM {
                                    continue;
                                }
                                let speed = dist_km / dt_h;
                                if speed > threshold && worst.is_none_or(|(w, _, _, _)| speed > w) {
                                    worst = Some((speed, dist_km, i, j));
                                }
                            }
                        }
                        // Select the worst pair per trip, but DEFER EMISSION: reporting the same physical
                        // pair once per trip would scale findings with timetable frequency, not defect size.
                        if let Some((speed, dist_km, i, j)) = worst {
                            let pair = stm061_pairs
                                .entry((
                                    route,
                                    dir_sd,
                                    idx.stop_id_of(&stimes[i]),
                                    idx.stop_id_of(&stimes[j]),
                                ))
                                .or_insert_with(|| Stm061Pair {
                                    route_label,
                                    seq_a: stimes[i].stop_sequence().unwrap_or(0),
                                    seq_b: stimes[j].stop_sequence().unwrap_or(0),
                                    threshold,
                                    dist_km,
                                    line: stimes[j].line as u64,
                                    speed_min: speed,
                                    speed_max: speed,
                                    trips: Vec::new(),
                                });
                            pair.speed_min = pair.speed_min.min(speed);
                            pair.speed_max = pair.speed_max.max(speed);
                            pair.line = pair.line.min(stimes[j].line as u64);
                            pair.trips.push(trip_id);
                        }
                    }
                }
            }

            let mut trip_max_speed: f64 = 0.0;
            let mut trip_max_speed_line: Option<u64> = None;
            let mut trip_bad_seg_count: u32 = 0;
            // Number of segments still bad under the truncated route's threshold (OPR_008 counterexample).
            let mut trip_ws_bad_count: u32 = 0;
            // Stop-ID pairs for all bad segments — each is drawn as a red line on the UI map.
            let mut bad_seg_stops: Vec<(SmolStr, SmolStr)> = Vec::new();
            // STM_020: the longest zero-travel-time segment per trip.
            let mut worst_zero_seg: Option<(f64, u64, SmolStr, SmolStr, u32, u32)> = None;

            for i in 0..stimes.len() - 1 {
                let a = &stimes[i];
                let b = &stimes[i + 1];

                let dep_a = a.departure_time().map(hms_to_secs);
                let arr_b = b.arrival_time().map(hms_to_secs);

                let (Some(dep), Some(arr)) = (dep_a, arr_b) else {
                    continue;
                };
                // STM_020: zero travel time — 200 m threshold (filters minute-rounding noise).
                if arr == dep {
                    let dep_secs = a.departure_time().map(|(_, _, s)| s).unwrap_or(1);
                    let arr_secs = b.arrival_time().map(|(_, _, s)| s).unwrap_or(1);
                    let dist_km = match (coords_buf[i], coords_buf[i + 1]) {
                        (Some((la1, lo1)), Some((la2, lo2))) => haversine_km(la1, lo1, la2, lo2),
                        _ => 0.0,
                    };
                    // Both times are exact minutes (seconds=0) → the true travel time is unknown:
                    // rounding allows anything from 0 to 59 seconds. Therefore, to call it
                    // "impossible", the distance must exceed the route type's maximum speed
                    // even if a FULL MINUTE had elapsed → threshold = max_speed_kmh / 60.
                    //
                    // The fixed 1.0 km threshold produced FALSE POSITIVES: 1.1 km for a bus
                    // is 66 km/h in 60 seconds — quite normal, not "physically impossible".
                    // Evidence from the 250-feed corpus (mdb-2155): 41 of 42 findings were
                    // 1.1–1.5 km = FP; the only real case was 29.4 km (1764 km/h), which MD
                    // also reports as the ONLY case.
                    let impossible_km = threshold / 60.0;
                    if dep_secs == 0 && arr_secs == 0 {
                        if dist_km >= impossible_km {
                            let mut n012 = k6_notice(
                            ctr, "STM_012", EntityType::Trip,
                            Some(trip_id.to_string()), Some(trip_id.to_string()),
                            "stop_times.txt", Some(b.line as u64), Some("arrival_time"),
                            Some(format!("duration=0s; distance={dist_km:.1}km")),
                            Some(format!("< {impossible_km:.1} km (within the same minute)")),
                            format!("trip_id '{trip_id}'{dep_suffix} stop_sequence {}-{} arası geçiş süresi sıfır ama mesafe {dist_km:.1} km — aynı dakika içinde kapatılamaz (≥ {:.0} km/h gerekirdi, eşik {threshold:.0} km/h).",
                                a.stop_sequence().unwrap_or(0), b.stop_sequence().unwrap_or(0),
                                dist_km * 60.0),
                            "stop_times.txt zaman değerlerini doğrulayın; iki durak aynı dakikaya yazılmış ama aralarındaki mesafe bir dakikada alınamaz.",
                        );
                            let mut details = stm012_details(
                                idx.stop_id_of(a),
                                idx.stop_id_of(b),
                                a.stop_sequence().unwrap_or(0),
                                b.stop_sequence().unwrap_or(0),
                                dist_km,
                                0,
                                None,
                                Some(dist_km * 60.0),
                                None,
                            );
                            add_stm012_stop_context(&mut details, idx, stimes, i, i + 1);
                            n012.details = Some(details.into());
                            notices.push(n012);
                            // As with a zero-time interval containing untimed intermediate stops:
                            // OPR_008 counts this segment too; the lower speed bound assumes a full minute elapsed.
                            let zero_speed = dist_km * 60.0;
                            if zero_speed > trip_max_speed {
                                trip_max_speed = zero_speed;
                                trip_max_speed_line = Some(b.line as u64);
                            }
                            trip_bad_seg_count += 1;
                            trip_ws_bad_count += 1;
                            bad_seg_stops.push((
                                SmolStr::from(idx.stop_id_of(a)),
                                SmolStr::from(idx.stop_id_of(b)),
                            ));
                        }
                        continue;
                    }
                    if dist_km > 0.2 {
                        let is_worse = worst_zero_seg.as_ref().is_none_or(|&(d, ..)| dist_km > d);
                        if is_worse {
                            worst_zero_seg = Some((
                                dist_km,
                                b.line as u64,
                                SmolStr::from(idx.stop_id_of(a)),
                                SmolStr::from(idx.stop_id_of(b)),
                                a.stop_sequence().unwrap_or(0),
                                b.stop_sequence().unwrap_or(0),
                            ));
                        }
                    }
                    continue;
                }
                if arr < dep {
                    // STM_008 chronology was emitted in the stateful pass above.
                    // Keep the adjacent speed loop from interpreting a backwards
                    // segment as a valid (underflowed) travel duration.
                    continue;
                }
                let dt_sec = arr - dep;

                // Distance: shape polyline projection when available, or Haversine fallback.
                // Skip projection for rail (metro/train) trips: straight/tunnel routes are
                // represented adequately by Haversine; winding shapes can produce false positives.
                let (c1, c2) = match (coords_buf[i], coords_buf[i + 1]) {
                    (Some(c1), Some(c2)) => (c1, c2),
                    _ => continue,
                };
                let haver_km = haversine_km(c1.0, c1.1, c2.0, c2.1);
                let dist_km = trip_shape_speed
                    .get(trip_id)
                    .and_then(|&sid| {
                        let pts = shape_pts_speed.get(sid)?;
                        let cum = shape_cum_speed.get(sid)?;
                        if pts.len() < 2 {
                            return None;
                        }
                        // (sid, stop_id) → arc cache: avoids repeated projection across trips/segments
                        // using the same shape. The result is deterministic because stop_id maps to one coordinate.
                        let arc_a = *arc_cache
                            .entry((sid, idx.stop_id_of(a)))
                            .or_insert_with(|| project_arc_km(pts, cum, c1.0, c1.1));
                        let arc_b = *arc_cache
                            .entry((sid, idx.stop_id_of(b)))
                            .or_insert_with(|| project_arc_km(pts, cum, c2.0, c2.1));
                        let d = (arc_b - arc_a).abs();
                        if d > 1e-6 {
                            Some(d)
                        } else {
                            None
                        }
                    })
                    // If shape projection exceeds four times the straight-line distance, the stop likely
                    // matched the WRONG shape segment (the shape approaches/crosses itself — common in
                    // grid networks, outbound and return service on the same street, etc.). Projection
                    // is unreliable then; fall back to Haversine. Otherwise feeds such as LA Metro could
                    // produce false 1000+ km/h STM_012/STM_014 findings.
                    // Shape projection also cannot be shorter than straight-line distance: self-near/crossing
                    // shapes can map two stops to the same small arc section and artificially lower the real
                    // bus speed, producing an STM_014 false negative.
                    // Keep the 4× upper fallback; clamp shape distance to Haversine at the lower bound.
                    // This preserves both real route detours and the physical minimum.
                    .filter(|&d| d <= 4.0 * haver_km.max(0.05))
                    .map(|d| d.max(haver_km))
                    .unwrap_or(haver_km);

                if dist_km < 1e-6 {
                    if idx.stop_id_of(a) == idx.stop_id_of(b) {
                        // STM_035: the same stop appears twice consecutively (terminal/circular route).
                        notices.push(k6_notice(
                        ctr, "STM_035", EntityType::Trip,
                        Some(trip_id.to_string()), Some(trip_id.to_string()),
                        "stop_times.txt", Some(b.line as u64), Some("stop_id"),
                        Some(idx.stop_id_of(a).to_string()),
                        None,
                        format!("trip_id '{trip_id}'{dep_suffix} stop_sequence {}-{} arasında aynı durak ({}) ardışık iki kez ziyaret ediliyor.",
                            a.stop_sequence().unwrap_or(0), b.stop_sequence().unwrap_or(0), idx.stop_id_of(a)),
                        "Terminal veya döngü hattıysa beklenen bir durumdur. Değilse stop_times.txt'teki yinelenen satırı kaldırın.",
                    ));
                    } else {
                        // STM_021: different stop_ids at the same coordinates — a real data error.
                        let mut n = k6_notice(
                        ctr, "STM_021", EntityType::Trip,
                        Some(trip_id.to_string()), Some(trip_id.to_string()),
                        "stop_times.txt", Some(b.line as u64), Some("stop_id"),
                        Some(format!("{} → {}", idx.stop_id_of(a), idx.stop_id_of(b))),
                        Some("> 0 m".to_string()),
                        format!("trip_id '{trip_id}'{dep_suffix} stop_sequence {}-{} arası mesafe 0: '{}' ve '{}' farklı duraklar ama aynı koordinatta.",
                            a.stop_sequence().unwrap_or(0), b.stop_sequence().unwrap_or(0), idx.stop_id_of(a), idx.stop_id_of(b)),
                        "stops.txt'te durak koordinatlarını doğrulayın; ardışık farklı duraklar aynı konumda olmamalıdır.",
                    );
                        let mut d = std::collections::BTreeMap::new();
                        d.insert("stop_a".to_string(), idx.stop_id_of(a).to_string());
                        d.insert("stop_b".to_string(), idx.stop_id_of(b).to_string());
                        n.details = Some(d.into());
                        notices.push(n);
                    }
                    continue;
                }

                // STM_025: <10s is not an error by itself; speed rules assess physical consistency.
                if dt_sec < 10 {
                    let mut n025 = k6_notice(
                    ctr, "STM_025", EntityType::Trip,
                    Some(trip_id.to_string()), Some(trip_id.to_string()),
                    "stop_times.txt", Some(b.line as u64), Some("arrival_time"),
                    Some(format!("{dt_sec}s")), Some("inceleyin".to_string()),
                    format!("trip_id '{trip_id}'{dep_suffix} stop_sequence {}-{} arası planlı süre {dt_sec}s — kısa zamanlama sinyali.",
                        a.stop_sequence().unwrap_or(0), b.stop_sequence().unwrap_or(0)),
                    "Bu süre bilinçliyse işlem gerekmez; değilse stop_times.txt zaman değerlerini kontrol edin.",
                );
                    let mut d = std::collections::BTreeMap::new();
                    d.insert("stop_a".to_string(), idx.stop_id_of(a).to_string());
                    d.insert("stop_b".to_string(), idx.stop_id_of(b).to_string());
                    n025.details = Some(d.into());
                    notices.push(n025);
                }

                // STM_026: the distance between stops is too long — because shape arc projection can be wrong,
                // check straight-line (Haversine) distance; a real problem will be large there too.
                // Higher threshold for rail trips (config.rail_stop_distance_km).
                let stm026_threshold = if is_rail_route_type(route_type) {
                    config.rail_stop_distance_km
                } else {
                    50.0
                };
                if haver_km > stm026_threshold {
                    let mut n026 = k6_notice(
                    ctr, "STM_026", EntityType::Trip,
                    Some(trip_id.to_string()), Some(trip_id.to_string()),
                    "stop_times.txt", Some(b.line as u64), Some("stop_id"),
                    Some(format!("{dist_km:.1} km")), Some(format!("<= {stm026_threshold:.0} km")),
                    format!("trip_id '{trip_id}'{dep_suffix} stop_sequence {}-{} arası mesafe {dist_km:.1} km.",
                        a.stop_sequence().unwrap_or(0), b.stop_sequence().unwrap_or(0)),
                    "stops.txt koordinatlarını ve stop_times.txt sırasını doğrulayın.",
                );
                    let mut d = std::collections::BTreeMap::new();
                    d.insert("stop_a".to_string(), idx.stop_id_of(a).to_string());
                    d.insert("stop_b".to_string(), idx.stop_id_of(b).to_string());
                    n026.details = Some(d.into());
                    notices.push(n026);
                }

                let speed = dist_km / (dt_sec as f64 / 3600.0);

                // STM_012: physically impossible speed (absolute upper bound 700 km/h).
                // The "impossible" claim is proven with a LOWER-BOUND distance (Haversine, straight-line):
                // real road distance ≥ straight-line distance is always true; shape projection can instead
                // match the wrong segment and inflate the value (up to the 4× ceiling). If projected speed
                // exceeds 700 but Haversine speed does not, the case is "extreme", not "impossible" → STM_014.
                let hav_speed = haver_km / (dt_sec as f64 / 3600.0);
                if hav_speed > 700.0 {
                    let mut n012 = k6_notice(
                    ctr, "STM_012", EntityType::Trip,
                    Some(trip_id.to_string()), Some(trip_id.to_string()),
                    "stop_times.txt", Some(b.line as u64), Some("arrival_time"),
                    Some(format!("{hav_speed:.0} km/h")), Some("<= 700 km/h".to_string()),
                    format!("trip_id '{trip_id}'{dep_suffix} stop_sequence {}-{} arası hız {hav_speed:.0} km/h (kuş uçuşu alt sınır) — fiziksel olarak imkansız.",
                        a.stop_sequence().unwrap_or(0), b.stop_sequence().unwrap_or(0)),
                    "stop_times.txt zaman ve stops.txt koordinat verilerini doğrulayın.",
                );
                    let mut details = stm012_details(
                        idx.stop_id_of(a),
                        idx.stop_id_of(b),
                        a.stop_sequence().unwrap_or(0),
                        b.stop_sequence().unwrap_or(0),
                        haver_km,
                        dt_sec,
                        Some(hav_speed),
                        None,
                        None,
                    );
                    add_stm012_stop_context(&mut details, idx, stimes, i, i + 1);
                    n012.details = Some(details.into());
                    notices.push(n012);
                    // OPR_008 counts this segment too and must see its speed and stop pair;
                    // otherwise a trip containing only an STM_012 segment would print "maximum speed 0.0".
                    if hav_speed > trip_max_speed {
                        trip_max_speed = hav_speed;
                        trip_max_speed_line = Some(b.line as u64);
                    }
                    trip_bad_seg_count += 1;
                    trip_ws_bad_count += 1;
                    bad_seg_stops.push((
                        SmolStr::from(idx.stop_id_of(a)),
                        SmolStr::from(idx.stop_id_of(b)),
                    ));
                    continue;
                }

                if speed > threshold {
                    // Emission is deferred: produce one notice for each (route, direction, segment).
                    let seg = stm014_segs
                        .entry((route, dir_sd, idx.stop_id_of(a), idx.stop_id_of(b)))
                        .or_insert_with(|| Stm014Seg {
                            route_label,
                            seq_a: a.stop_sequence().unwrap_or(0),
                            seq_b: b.stop_sequence().unwrap_or(0),
                            threshold,
                            line: b.line as u64,
                            speed_min: speed,
                            speed_max: speed,
                            distance_min_km: haver_km,
                            distance_max_km: haver_km,
                            trips: Vec::new(),
                            ws_real: false,
                            ws_file: None,
                        });
                    seg.speed_min = seg.speed_min.min(speed);
                    seg.speed_max = seg.speed_max.max(speed);
                    seg.distance_min_km = seg.distance_min_km.min(haver_km);
                    seg.distance_max_km = seg.distance_max_km.max(haver_km);
                    seg.line = seg.line.min(b.line as u64);
                    seg.trips.push(trip_id);
                    match ws_join {
                        Some((ws_threshold, file)) if speed <= ws_threshold => {
                            seg.ws_file = Some(file);
                        }
                        _ => {
                            seg.ws_real = true;
                            trip_ws_bad_count += 1;
                        }
                    }

                    if speed > trip_max_speed {
                        trip_max_speed = speed;
                        trip_max_speed_line = Some(b.line as u64);
                    }
                    trip_bad_seg_count += 1;
                    bad_seg_stops.push((
                        SmolStr::from(idx.stop_id_of(a)),
                        SmolStr::from(idx.stop_id_of(b)),
                    ));
                }
            }

            // Intervals spanning untimed intermediate stops: the loop above checks only ADJACENT
            // stops and `continue`s when a stop has no time; STM_061 measures only pairs >10 km.
            // Thus the 3.5 km from 15:15 → 15:15 (mdb-2202, 11 untimed stops) was never seen.
            // Here we evaluate only timed→timed intervals with untimed stops BETWEEN them —
            // adjacent pairs are already measured, so there is no double counting.
            let mut prev_timed: Option<usize> = None;
            for j in 0..stimes.len() {
                let b = &stimes[j];
                let Some(arr) = b.arrival_time().or(b.departure_time()).map(hms_to_secs) else {
                    continue;
                };
                let p = prev_timed.replace(j);
                let Some(p) = p.filter(|&p| j > p + 1) else {
                    continue;
                };
                let a = &stimes[p];
                let Some(dep) = a.departure_time().or(a.arrival_time()).map(hms_to_secs) else {
                    continue;
                };
                if arr < dep {
                    continue; // chronology belongs to STM_008
                }
                // Path: sum of straight-line distances between intermediate stops (longer than direct
                // distance, shorter than the real route). The "impossible" verdict still uses direct
                // (lower-bound) distance.
                let (Some((la1, lo1)), Some((la2, lo2))) = (coords_buf[p], coords_buf[j]) else {
                    continue;
                };
                let mut path_km = 0.0;
                let mut path_ok = true;
                for k in p..j {
                    match (coords_buf[k], coords_buf[k + 1]) {
                        (Some(c1), Some(c2)) => path_km += haversine_km(c1.0, c1.1, c2.0, c2.1),
                        _ => {
                            path_ok = false;
                            break;
                        }
                    }
                }
                let direct_km = haversine_km(la1, lo1, la2, lo2);
                if !path_ok {
                    path_km = direct_km;
                }
                let dt_sec = arr - dep;
                let seq_a = a.stop_sequence().unwrap_or(0);
                let seq_b = b.stop_sequence().unwrap_or(0);
                let impossible = if dt_sec == 0 {
                    // Zero time: the same rule as for an adjacent pair — if both times are exact minutes,
                    // the real duration may be 0–59 seconds; a distance impossible even in one full minute is impossible.
                    let whole = a.departure_time().or(a.arrival_time()).map(|(_, _, s)| s) == Some(0)
                        && b.arrival_time().or(b.departure_time()).map(|(_, _, s)| s) == Some(0);
                    if whole && direct_km >= threshold / 60.0 {
                        // Lower speed bound: assume that one full minute elapsed (for OPR_008's maximum speed).
                        Some((format!("{direct_km:.1} km / 0 dk"), direct_km * 60.0))
                    } else {
                        if direct_km > 0.2
                            && worst_zero_seg.as_ref().is_none_or(|&(d, ..)| direct_km > d)
                        {
                            worst_zero_seg = Some((
                                direct_km,
                                b.line as u64,
                                SmolStr::from(idx.stop_id_of(a)),
                                SmolStr::from(idx.stop_id_of(b)),
                                seq_a,
                                seq_b,
                            ));
                        }
                        None
                    }
                } else {
                    let hav_speed = direct_km / (dt_sec as f64 / 3600.0);
                    (hav_speed > 700.0).then(|| (format!("{hav_speed:.0} km/h"), hav_speed))
                };
                if let Some((observed, impossible_speed)) = impossible {
                    let mut n012 = k6_notice(
                        ctr, "STM_012", EntityType::Trip,
                        Some(trip_id.to_string()), Some(trip_id.to_string()),
                        "stop_times.txt", Some(b.line as u64), Some("arrival_time"),
                        Some(observed.clone()), Some("<= 700 km/h".to_string()),
                        format!("trip_id '{trip_id}'{dep_suffix} stop_sequence {seq_a}-{seq_b} arası (aradaki duraklar saatsiz) {observed} — fiziksel olarak imkansız."),
                        "stop_times.txt zaman ve stops.txt koordinat verilerini doğrulayın.",
                    );
                    let mut details = stm012_details(
                        idx.stop_id_of(a),
                        idx.stop_id_of(b),
                        seq_a,
                        seq_b,
                        direct_km,
                        dt_sec,
                        (dt_sec > 0).then_some(impossible_speed),
                        (dt_sec == 0).then_some(impossible_speed),
                        Some(j - p - 1),
                    );
                    add_stm012_stop_context(&mut details, idx, stimes, p, j);
                    n012.details = Some(details.into());
                    notices.push(n012);
                    if impossible_speed > trip_max_speed {
                        trip_max_speed = impossible_speed;
                        trip_max_speed_line = Some(b.line as u64);
                    }
                    trip_bad_seg_count += 1;
                    trip_ws_bad_count += 1;
                    bad_seg_stops.push((
                        SmolStr::from(idx.stop_id_of(a)),
                        SmolStr::from(idx.stop_id_of(b)),
                    ));
                    continue;
                }
                if dt_sec == 0 {
                    continue;
                }
                let speed = path_km / (dt_sec as f64 / 3600.0);
                if speed > threshold {
                    let seg = stm014_segs
                        .entry((route, dir_sd, idx.stop_id_of(a), idx.stop_id_of(b)))
                        .or_insert_with(|| Stm014Seg {
                            route_label,
                            seq_a,
                            seq_b,
                            threshold,
                            line: b.line as u64,
                            speed_min: speed,
                            speed_max: speed,
                            distance_min_km: path_km,
                            distance_max_km: path_km,
                            trips: Vec::new(),
                            ws_real: false,
                            ws_file: None,
                        });
                    seg.speed_min = seg.speed_min.min(speed);
                    seg.speed_max = seg.speed_max.max(speed);
                    seg.distance_min_km = seg.distance_min_km.min(path_km);
                    seg.distance_max_km = seg.distance_max_km.max(path_km);
                    seg.line = seg.line.min(b.line as u64);
                    seg.trips.push(trip_id);
                    match ws_join {
                        Some((ws_threshold, file)) if speed <= ws_threshold => {
                            seg.ws_file = Some(file);
                        }
                        _ => {
                            seg.ws_real = true;
                            trip_ws_bad_count += 1;
                        }
                    }
                    if speed > trip_max_speed {
                        trip_max_speed = speed;
                        trip_max_speed_line = Some(b.line as u64);
                    }
                    trip_bad_seg_count += 1;
                    bad_seg_stops.push((
                        SmolStr::from(idx.stop_id_of(a)),
                        SmolStr::from(idx.stop_id_of(b)),
                    ));
                }
            }

            // OPR_008: emit a summary notice only when multiple segments are bad.
            // If one segment is bad, STM_014 already carries that information — OPR_008 would duplicate it.
            if trip_bad_seg_count > 1 {
                let mut n = k6_notice(
                ctr,
                "OPR_008",
                EntityType::Trip,
                Some(trip_id.to_string()),
                Some(trip_id.to_string()),
                "stop_times.txt",
                trip_max_speed_line,
                None,
                Some(format!("{trip_max_speed:.1} km/h")),
                Some(format!("≤ {threshold:.0} km/h")),
                format!(
                    "'{route_label}' kodlu hattın {dir_sd} yönünde {svc_sd} çalışma takviminde \
                    '{trip_id}'{dep_suffix} seferindeki {trip_bad_seg_count} bozuk segmentte en yüksek hız {trip_max_speed:.1} km/h — eşik {threshold:.0} km/h.",
                ),
                "stop_times zaman ve koordinat verilerini kontrol edin.",
            );
                // All bad segment pairs → each is a red polyline in the UI.
                // Also put the departure time (when present) in details → available to the EN/JA message template.
                {
                    let mut d = std::collections::BTreeMap::new();
                    for (i, (sa, sb)) in bad_seg_stops.iter().enumerate() {
                        d.insert(format!("bad_seg_{i}_a"), sa.to_string());
                        d.insert(format!("bad_seg_{i}_b"), sb.to_string());
                    }
                    // Departure time → details. The EN/JA template displays it via {dep_suffix};
                    // when empty, the suffix disappears completely (Rust creates the conditional space).
                    d.insert("departure".to_string(), dep_str.clone());
                    d.insert(
                        "dep_suffix".to_string(),
                        if dep_str.is_empty() {
                            String::new()
                        } else {
                            format!(" ({dep_str})")
                        },
                    );
                n.details = Some(d.into());
                }
                if let Some((_, file)) = ws_join.filter(|_| trip_ws_bad_count <= 1) {
                    mark_whitespace_join(&mut n, file);
                }
                notices.push(n);
            }

            // STM_020: longest zero-travel-time segment per trip (one notice per trip).
            if let Some((dist_km, line, stop_a, stop_b, seq_a, seq_b)) = worst_zero_seg {
                let mut n = k6_notice(
                ctr,
                "STM_020",
                EntityType::Trip,
                Some(trip_id.to_string()),
                Some(trip_id.to_string()),
                "stop_times.txt",
                Some(line),
                Some("arrival_time"),
                Some(format!("duration=0s; distance={:.0}m", dist_km * 1000.0)),
                Some("> 0s".to_string()),
                format!(
                    "'{}' hattının{} seferinde {}. ve {}. duraklar arası {:.0}m mesafe var ama geçiş süresi 0 girilmiş.",
                    route,
                    if dep_str.is_empty() { String::new() } else { format!(" {dep_str}") },
                    seq_a, seq_b, dist_km * 1000.0,
                ),
                "Kalkış/varış zamanlarını doğrulayın; ardışık duraklar arasında geçiş süresi sıfır olamaz.",
            );
                n.details = Some(
                    [
                        ("stop_a".to_string(), stop_a.to_string()),
                        ("stop_b".to_string(), stop_b.to_string()),
                    ]
                    .into_iter()
                    .collect(),
                );
                notices.push(n);
            }
        }
    } // K6::sd::loop

    let mut stm053_keys: Vec<&str> = stm053_routes.keys().copied().collect();
    stm053_keys.sort_unstable();
    for route in stm053_keys {
        let &(max_same_run, affected_trips, example_trip, line) = &stm053_routes[route];
        notices.push(k6_notice(
            ctr,
            "STM_053",
            EntityType::Route,
            Some(route.to_string()),
            Some(route.to_string()),
            // `line` is the first stop_times row of the example trip; it must refer to that file.
            "stop_times.txt",
            (line > 0).then_some(line),
            // Measured column: arrival_time (or departure_time). route_id is only a grouping key
            // and is not a column in stop_times.txt.
            Some("arrival_time|departure_time"),
            Some(format!(
                "{affected_trips} trips; max {max_same_run} consecutive stops"
            )),
            Some("no route-level repeated-time pattern".to_string()),
            format!(
                "route_id '{route}' içinde {affected_trips} seferde ardışık duraklar aynı zaman değerini kullanıyor (en uzun seri: {max_same_run}; örnek: '{example_trip}')."
            ),
            "Ardışık stop_times zamanlarını doğrulayın; gerçek bekleme ise zamanları açıklayıcı biçimde düzenleyin.",
        ));
    }

    // ── STM_014: emit accumulated segments individually ─────────────────────
    // FxHashMap iteration is unordered → sort by key (golden determinism).
    {
        let _t = Timer::start("K6::sd::stm014_emit");
        let mut keys: Vec<(&str, &str, &str, &str)> = stm014_segs.keys().copied().collect();
        keys.sort_unstable();
        for key in keys {
            let seg = &stm014_segs[&key];
            let (route, dir, stop_a, stop_b) = key;
            let n_trips = seg.trips.len();
            let Stm014Seg {
                route_label,
                seq_a,
                seq_b,
                threshold,
                line,
                speed_min,
                speed_max,
                distance_min_km,
                distance_max_km,
                ..
            } = *seg;

            // Deterministic, deduplicated trip list (the same trip segment appears once,
            // but it may recur on loop routes).
            let mut trips: Vec<&str> = seg.trips.clone();
            trips.sort_unstable();
            trips.dedup();

            let observed = if (speed_max - speed_min).abs() < 0.05 {
                format!("{speed_max:.1} km/h ({stop_a} → {stop_b})")
            } else {
                format!("{speed_min:.1}-{speed_max:.1} km/h ({stop_a} → {stop_b})")
            };
            let msg = if n_trips == 1 {
                format!(
                    "'{route_label}' kodlu hattın {dir} yönünde '{}' seferinde durak sırası {seq_a}-{seq_b} \
                    arası hız {speed_max:.1} km/h — eşik {threshold:.0} km/h.",
                    trips[0]
                )
            } else {
                format!(
                    "'{route_label}' kodlu hattın {dir} yönünde durak sırası {seq_a}-{seq_b} arası hız eşiği \
                    {n_trips} seferde aşılıyor — {speed_min:.1}-{speed_max:.1} km/h, eşik {threshold:.0} km/h. \
                    Tek segment, tek kök neden: seferler ayrı ayrı listelenmez."
                )
            };
            let mut n = k6_notice(
                ctr,
                "STM_014",
                EntityType::Route,
                // entity_id must identify the segment UNIQUELY: because dedup=Entity,
                // different segments on the same route would otherwise overwrite one another.
                Some(format!("{route}:{dir}:{stop_a}→{stop_b}")),
                // scope_key = route_id → route-level filtering continues to work in the UI.
                Some(route.to_string()),
                "stop_times.txt",
                Some(line),
                Some("arrival_time"),
                Some(observed),
                Some(format!("≤ {threshold:.0} km/h")),
                msg,
                "Zaman damgalarını veya durak koordinatlarını doğrulayın; ardışık stop_times arasındaki hız eşiği aşıyor.",
            );
            let mut d = std::collections::BTreeMap::new();
            d.insert("stop_a".to_string(), stop_a.to_string());
            d.insert("stop_b".to_string(), stop_b.to_string());
            d.insert("route_id".to_string(), route.to_string());
            // route_label = route_short_name (or route_id) — EN/JA templates use {route_label}
            // in the same form as other route-scoped rules.
            d.insert("route_label".to_string(), route_label.to_string());
            d.insert("direction_id".to_string(), dir.to_string());
            d.insert("trip_count".to_string(), trips.len().to_string());
            d.insert("speed_min".to_string(), format!("{speed_min:.1}"));
            d.insert("speed_max".to_string(), format!("{speed_max:.1}"));
            d.insert("threshold_kmh".to_string(), format!("{threshold:.0}"));
            d.insert(
                "distance_min_meters".to_string(),
                format!("{:.0}", distance_min_km * 1000.0),
            );
            d.insert(
                "distance_max_meters".to_string(),
                format!("{:.0}", distance_max_km * 1000.0),
            );
            let sample_trip = trips
                .iter()
                .find_map(|trip_id| idx.by_trip.get(trip_id).map(|stops| (*trip_id, *stops)));
            if let Some((sample_trip_id, stops)) = sample_trip {
                if let Some(service_id) = trip_service.get(sample_trip_id).filter(|id| !id.is_empty()) {
                    d.insert("sample_service_id".to_string(), (*service_id).to_string());
                }
                if let Some(departure) = idx.trip_first_dep.get(sample_trip_id) {
                    d.insert("sample_departure".to_string(), format_hms(*departure));
                }
                if let Some(first_stop) = stops.first().map(|stop| idx.stop_id_of(stop)) {
                    if !first_stop.is_empty() {
                        d.insert("direction_from_stop_id".to_string(), first_stop.to_string());
                    }
                }
                if let Some(last_stop) = stops.last().map(|stop| idx.stop_id_of(stop)) {
                    if !last_stop.is_empty() {
                        d.insert("direction_to_stop_id".to_string(), last_stop.to_string());
                    }
                }
                // Match by stop ids only: seq_a/seq_b come from whichever trip was inserted first,
                // and FxHashMap order differs between 32- and 64-bit builds, so a sequence match
                // would make this context appear on one platform and vanish on the other.
                let a_index = stops.iter().position(|stop| idx.stop_id_of(stop) == stop_a);
                let b_index = a_index.and_then(|a| {
                    stops[a + 1..]
                        .iter()
                        .position(|stop| idx.stop_id_of(stop) == stop_b)
                        .map(|offset| a + 1 + offset)
                });
                if let (Some(a_index), Some(b_index)) = (a_index, b_index) {
                    add_stm012_stop_context(&mut d, idx, stops, a_index, b_index);
                }
            }
            d.insert(
                "trips".to_string(),
                trips
                    .iter()
                    .take(STM014_TRIP_SAMPLE)
                    .copied()
                    .collect::<Vec<_>>()
                    .join(","),
            );
            n.details = Some(d.into());
            if let Some(file) = seg.ws_file.filter(|_| !seg.ws_real) {
                mark_whitespace_join(&mut n, file);
            }
            notices.push(n);
        }
    }

    // ── STM_061: emit accumulated distant stop pairs individually ───────────
    // Same pattern and rationale as STM_014: FxHashMap iteration is unordered,
    // so sort by key or the golden output varies between runs.
    {
        let _t = Timer::start("K6::sd::stm061_emit");
        let mut keys: Vec<(&str, &str, &str, &str)> = stm061_pairs.keys().copied().collect();
        keys.sort_unstable();
        for key in keys {
            let pair = &stm061_pairs[&key];
            let (route, dir, stop_a, stop_b) = key;
            let Stm061Pair {
                route_label,
                seq_a,
                seq_b,
                threshold,
                dist_km,
                line,
                speed_min,
                speed_max,
                ..
            } = *pair;

            let mut trips: Vec<&str> = pair.trips.clone();
            trips.sort_unstable();
            trips.dedup();
            let n_trips = trips.len();

            let observed = if (speed_max - speed_min).abs() < 0.5 {
                format!("{speed_max:.0} km/h ({dist_km:.1} km)")
            } else {
                format!("{speed_min:.0}-{speed_max:.0} km/h ({dist_km:.1} km)")
            };
            let msg = if n_trips == 1 {
                format!(
                    "'{route_label}' kodlu hattın {dir} yönünde '{}' seferi: stop_sequence {seq_a} ile {seq_b} \
                    arası {dist_km:.1} km kuş uçuşu mesafeyi {speed_max:.0} km/h ile kapatıyor (eşik {threshold:.0} km/h) \
                    — duraklar ardışık değil, ara segmentler tek tek makul görünse de bütün imkânsız.",
                    trips[0]
                )
            } else {
                format!(
                    "'{route_label}' kodlu hattın {dir} yönünde stop_sequence {seq_a} ile {seq_b} arası \
                    {dist_km:.1} km kuş uçuşu mesafe {n_trips} seferde imkânsız hızla kapatılıyor — \
                    {speed_min:.0}-{speed_max:.0} km/h, eşik {threshold:.0} km/h. Tek durak çifti, tek kök neden: \
                    seferler ayrı ayrı listelenmez."
                )
            };
            let mut n = k6_notice(
                ctr,
                "STM_061",
                EntityType::Route,
                // entity_id must identify the pair UNIQUELY: because dedup=Entity, different
                // pairs on the same route would otherwise overwrite one another (as in STM_014).
                Some(format!("{route}:{dir}:{stop_a}→{stop_b}")),
                Some(route.to_string()),
                "stop_times.txt",
                Some(line),
                Some("arrival_time"),
                Some(observed),
                Some(format!("<= {threshold:.0} km/h")),
                msg,
                "Uzak durak çifti arasındaki varış/kalkış saatlerini doğrulayın; genellikle bir zaman bloğu kaymış ya da gece yarısı aşımı yanlış yazılmıştır.",
            );
            let mut d = std::collections::BTreeMap::new();
            d.insert("stop_a".to_string(), stop_a.to_string());
            d.insert("stop_b".to_string(), stop_b.to_string());
            d.insert("route_id".to_string(), route.to_string());
            d.insert("route_label".to_string(), route_label.to_string());
            d.insert("direction_id".to_string(), dir.to_string());
            d.insert("trip_count".to_string(), n_trips.to_string());
            d.insert("dist_km".to_string(), format!("{dist_km:.1}"));
            d.insert("speed_min".to_string(), format!("{speed_min:.0}"));
            d.insert("speed_max".to_string(), format!("{speed_max:.0}"));
            d.insert(
                "trips".to_string(),
                trips
                    .iter()
                    .take(STM014_TRIP_SAMPLE)
                    .copied()
                    .collect::<Vec<_>>()
                    .join(","),
            );
            n.details = Some(d.into());
            notices.push(n);
        }
    }

    // STM_007: row-level pinpointing at low volume, one feed summary at high volume.
    // Keep the threshold equal to SHP_010/STP_022; sort examples in finalize_stm007_pending
    // so HashMap/chunk order cannot affect them.
    const STM007_AGG_THRESHOLD: usize = 50;
    finalize_stm007_pending(
        &mut stm007_pending,
        notices,
        ctr,
        "STM_007",
        STM007_AGG_THRESHOLD,
    );
}

// ── WP-09b: Frequency headway ─────────────────────────────────────────────────

fn check_frequency_headway(
    records: &EntityRecords,
    config: &ValidatorConfig,
    notices: &mut Vec<Notice>,
    ctr: &mut u32,
) {
    let max_secs = config.max_headway_warning_min * 60;
    let bunching_secs = config.bunching_threshold_min * 60;

    for frq in &records.frequencies {
        let Some(hw) = frq.headway_secs else { continue };
        let trip_id = if frq.trip_id.is_empty() {
            continue;
        } else {
            &frq.trip_id
        };

        if hw > max_secs {
            notices.push(k6_notice(
                ctr,
                "FRQ_006",
                EntityType::Trip,
                Some(trip_id.clone()),
                Some(trip_id.clone()),
                "frequencies.txt",
                Some(frq.line),
                Some("headway_secs"),
                Some(format!("{hw}s ({:.0}dk)", hw as f64 / 60.0)),
                Some(format!("≤ {}s", max_secs)),
                format!(
                    "'{trip_id}' seferinde sefer aralığı {hw}sn — eşik {max_secs}sn ({} dk).",
                    config.max_headway_warning_min
                ),
                "Seferler arası süreyi azaltın veya ek sefer ekleyin.",
            ));
        }

        // FRQ_010: very high-frequency service (below the bunching threshold) → info
        if hw <= bunching_secs {
            notices.push(k6_notice(
                ctr,
                "FRQ_010",
                EntityType::Trip,
                Some(trip_id.clone()),
                Some(trip_id.clone()),
                "frequencies.txt",
                Some(frq.line),
                Some("headway_secs"),
                Some(format!("{hw}s")),
                Some(format!("> {}s", bunching_secs)),
                format!("'{trip_id}' seferinde sefer aralığı {hw}sn ≤ sıkışma eşiği {bunching_secs}sn — sıkışma riski."),
                "Sefer programını gözden geçirin; bu headway gerçekten isteniyorsa dikkate almayın.",
            ));
        }
    }
}

/// Are departure intervals "regular" — coefficient of variation (CV = std/mean) < 0.25.
/// Regular intervals = intentional sparse service (e.g. rural morning/evening trips) → suppress OPR_001.
/// Irregular intervals (mostly frequent + one huge gap) = a real service gap → emit OPR_001.
/// One interval (2 trips) has no variance → treat it as intentional.
fn is_regular_headway(gaps: &[u32]) -> bool {
    if gaps.len() < 2 {
        return true;
    }
    let n = gaps.len() as f64;
    let mean = gaps.iter().map(|&g| g as f64).sum::<f64>() / n;
    if mean <= 0.0 {
        return true;
    }
    let var = gaps.iter().map(|&g| (g as f64 - mean).powi(2)).sum::<f64>() / n;
    (var.sqrt() / mean) < 0.25
}

// ── WP-09b: Route headway (regular service) ──────────────────────────────────

fn check_route_headway(
    records: &EntityRecords,
    config: &ValidatorConfig,
    idx: &StopTimesIndex<'_>,
    notices: &mut Vec<Notice>,
    ctr: &mut u32,
) {
    let ti_hw = &records.trip_interns;
    let route_short_hw: HashMap<&str, &str> = records
        .routes
        .iter()
        .map(|r| {
            let label = r
                .route_short_name
                .as_deref()
                .filter(|s| !s.is_empty())
                .unwrap_or(r.route_id.as_str());
            (r.route_id.as_str(), label)
        })
        .collect();

    // Two separate pools because the rules measure different things:
    //  • OPR_001 (sparse service / largest gap) uses the route's TOTAL frequency →
    //    coarse key (route_id, direction_key, service_id).
    //  • OPR_003 (bunching / smallest gap) is meaningful only between trips in the SAME
    //    service pattern → the fine key includes (first_stop_id, pattern).
    // Otherwise (a) trips leaving different terminals at about the same time or
    // (b) trips from the same stop to different DESTINATIONS (e.g. B100: "Bedford Av" 3-stop
    // stub vs "Mill Basin" 29-stop full run) produce a false "1-minute bunching" finding. pattern =
    // shape_id (or the last stop); keep first_stop IN THE KEY because without a shape, trips
    // from different origins to the same destination could merge and bring back a terminal FP.
    // Splitting OPR_001 by first stop/pattern would instead incorrectly mark short-line variants
    // that start service only at certain times as "sparse".
    // direction_key: "0"/"1"/... when direction_id exists, otherwise "" (no separate bucket).
    let mut route_departures: HashMap<(&str, &str, &str), Vec<u32>> = HashMap::new();
    let mut route_stop_departures: HashMap<(&str, &str, &str, &str, &str), Vec<u32>> =
        HashMap::new();

    for trip in &records.trips {
        let route_id = ti_hw.route_id(trip);
        if route_id.is_empty() {
            continue;
        }
        let direction_key: &str = match trip.direction_id {
            Some(0) => "0",
            Some(1) => "1",
            Some(d) => {
                // Unexpected value: normalize to an empty string (do not create a separate bucket).
                let _ = d;
                ""
            }
            None => "",
        };
        let service_id = ti_hw.service_id(trip);

        let Some(&dep) = idx.trip_first_dep.get(trip.trip_id.as_str()) else {
            continue;
        };

        let stops = idx.by_trip.get(trip.trip_id.as_str());
        let first_stop: &str = stops
            .and_then(|v| v.first())
            .map(|s| idx.stop_id_of(s))
            .unwrap_or("");
        // Service pattern: use shape_id when present, otherwise the last stop (destination).
        let pattern: &str = match ti_hw.shape_id(trip).filter(|s| !s.is_empty()) {
            Some(s) => s,
            None => stops
                .and_then(|v| v.last())
                .map(|s| idx.stop_id_of(s))
                .unwrap_or(""),
        };

        route_departures
            .entry((route_id, direction_key, service_id))
            .or_default()
            .push(dep);
        route_stop_departures
            .entry((route_id, direction_key, service_id, first_stop, pattern))
            .or_default()
            .push(dep);
    }

    let max_secs = config.max_headway_warning_min * 60;
    let bunching_secs = config.bunching_threshold_min * 60;

    // Split the OPR_001 threshold by route_type: hourly service is not expected on intercity rail
    // — a 580-minute gap on Ankara-Sivas YHT is normal operation, not a gap to close. DO NOT TOUCH
    // OPR_005: it already works RELATIVELY per route_type using median+MAD and has no fixed threshold.
    // FRQ rules are also out of scope — frequencies.txt already declares regular frequency; a large
    // headway there is a separate phenomenon.
    let rail_routes_hw: FxHashSet<&str> = records
        .routes
        .iter()
        .filter(|r| r.route_type.is_some_and(is_rail_route_type))
        .map(|r| r.route_id.as_str())
        .collect();
    let max_secs_rail = config.max_headway_warning_min_rail * 60;
    // Handle cases where the route is rail but is hidden only by whitespace in its ID (`mdb-1065`).
    let routes_raw_hw: FxHashSet<&str> =
        records.routes.iter().map(|r| r.route_id.as_str()).collect();
    let rail_trimmed_hw: FxHashMap<&str, bool> = records
        .routes
        .iter()
        .filter(|r| r.route_type.is_some_and(is_rail_route_type))
        .map(|r| (r.route_id.trim(), r.route_id != r.route_id.trim()))
        .collect();

    // OPR_001: does the largest consecutive departure gap across the route exceed the threshold?
    // HashMap iteration order changes between processes; sort by key so the dedup representative
    // among findings with the same entity_id does not depend on it.
    let mut route_departures: Vec<_> = route_departures.into_iter().collect();
    route_departures.sort_unstable_by_key(|(k, _)| *k);
    for ((route_id, direction_key, service_id), mut deps) in route_departures {
        deps.sort_unstable();
        deps.dedup();
        if deps.len() < 2 {
            continue;
        }
        let gaps: Vec<u32> = deps.windows(2).map(|w| w[1] - w[0]).collect();
        let max_hw = gaps.iter().copied().max().unwrap_or(0);
        // OPR_001 is emitted only when (a) the gap threshold is exceeded, (b) the route is NOT in
        // the manual rural list, and (c) departure intervals are irregular (a real gap). Regular-
        // sparse (low-CV) routes are treated as intentional rural service and silenced.
        let hw_threshold = if rail_routes_hw.contains(route_id) {
            max_secs_rail
        } else {
            max_secs
        };
        if max_hw > hw_threshold
            && !config.rural_route_ids.iter().any(|r| r == route_id)
            && !is_regular_headway(&gaps)
        {
            let dir_display = if direction_key.is_empty() {
                "-"
            } else {
                direction_key
            };
            let route_label_hw = route_short_hw.get(route_id).copied().unwrap_or(route_id);
            let mut n001 = k6_notice(
                ctr,
                "OPR_001",
                EntityType::Route,
                Some(route_id.to_string()),
                Some(route_id.to_string()),
                "stop_times.txt",
                None,
                None,
                Some(format!("{:.0}", max_hw as f64 / 60.0)),
                Some(format!("≤ {}", hw_threshold / 60)),
                format!("'{route_label_hw}' kodlu hattın {dir_display} yönünde {service_id} çalışma takviminde maksimum sefer aralığı {:.0}dk — eşik {}dk.",
                    max_hw as f64 / 60.0, hw_threshold / 60),
                "Pik/saatdışı sefer sayısını artırın ya da büyük boşlukları kapatın.",
            );
            n001.service_id = Some(service_id.to_string());
            if !routes_raw_hw.contains(route_id) && max_hw <= max_secs_rail {
                if let Some(&padded) = rail_trimmed_hw.get(route_id.trim()) {
                    let file = if padded { "routes.txt" } else { "trips.txt" };
                    mark_whitespace_join(&mut n001, file);
                }
            }
            notices.push(n001);
        }
    }

    // OPR_003: is the smallest positive interval between consecutive trips at the same first stop
    // below the bunching threshold?
    // HashMap iteration would make the dedup representative nondeterministic; sort by key.
    let mut route_stop_departures: Vec<_> = route_stop_departures.into_iter().collect();
    route_stop_departures.sort_unstable_by_key(|(k, _)| *k);
    for ((route_id, direction_key, service_id, _first_stop, _pattern), mut deps) in
        route_stop_departures
    {
        deps.sort_unstable();
        // dedup suppresses cross-day repeated departure times for the same
        // service_id; may also hide true parallel same-time trips (known tradeoff)
        deps.dedup();
        if deps.len() < 2 {
            continue;
        }
        let min_hw = deps.windows(2).map(|w| w[1] - w[0]).min().unwrap_or(0);
        if min_hw < bunching_secs && min_hw > 0 {
            let dir_display = if direction_key.is_empty() {
                "-"
            } else {
                direction_key
            };
            let route_label_hw = route_short_hw.get(route_id).copied().unwrap_or(route_id);
            let mut n = k6_notice(
                ctr,
                "OPR_003",
                EntityType::Route,
                Some(route_id.to_string()),
                Some(route_id.to_string()),
                "stop_times.txt",
                None,
                None,
                Some(format!("{:.0}", min_hw as f64 / 60.0)),
                Some(format!("> {}", config.bunching_threshold_min)),
                format!("'{route_label_hw}' kodlu hattın {dir_display} yönünde {service_id} çalışma takviminde minimum sefer aralığı {:.0}dk ≤ sıkışma eşiği {}dk.",
                    min_hw as f64 / 60.0, config.bunching_threshold_min),
                "Sefer programını düzenleyin; çok sık gelen seferler sıkışmaya yol açabilir.",
            );
            n.service_id = Some(service_id.to_string());
            notices.push(n);
        }
    }
}

// ── WP-09b: Takvim analizi ────────────────────────────────────────────────────

fn check_calendar_analytics(
    records: &EntityRecords,
    derived: &DerivedData,
    config: &ValidatorConfig,
    today_yyyymmdd: u32,
    notices: &mut Vec<Notice>,
    ctr: &mut u32,
) {
    let ti_cal = &records.trip_interns;
    let future_only_jp_date = gtfs_jp_future_only_date(records, derived, today_yyyymmdd);
    // CAL_021: how many trips use each service.
    let mut service_trip_counts: HashMap<&str, u32> = HashMap::new();
    for t in &records.trips {
        if !ti_cal.service_id(t).is_empty() {
            *service_trip_counts.entry(ti_cal.service_id(t)).or_insert(0) += 1;
        }
    }

    // CAL_010: total active days are too few (≤ service_gap_days) — per service.
    // CAL_007/012: gaps in a service period are collected across ALL services by the
    // (gap_start, gap_end) signature (#30) → one notice per gap, listing affected service_ids.
    // near_future is a function of the signature (relative to today), not the service → each
    // signature deterministically produces CAL_007 or CAL_012. Emit AFTER the collection loop.
    let today_jdn = yyyymmdd_to_jdn(today_yyyymmdd);
    let gap_threshold = config.service_gap_days; // CAL_010: very-short-service threshold (active days)
    let big_gap_threshold = config.big_gap_days; // CAL_007/012: large-gap threshold (MD ≈ 14)
    let mut gap_services: std::collections::BTreeMap<(u32, u32), Vec<&str>> =
        std::collections::BTreeMap::new();

    // ── Feed's total service window (for CAL_010) ───────────────────────────
    // The "only N active days" claim is meaningful only when the feed publishes a sufficiently long period.
    // On a 17-day feed, a train operating 4–6 days (Vangölü Ekspresi, 4 Eylül Mavi) is not sparse but NORMAL;
    // an absolute 7-day threshold would flag every service. For a short window, use a RATIO threshold:
    // active days / window < 20%.
    const SHORT_FEED_WINDOW_DAYS: u32 = 30;
    const SHORT_FEED_ACTIVE_RATIO: f64 = 0.20;
    let feed_window_days: u32 = {
        let mut lo = u32::MAX;
        let mut hi = 0u32;
        for dates in derived.calendar_bitmap.active_dates.values() {
            for &d in dates {
                if d < lo {
                    lo = d;
                }
                if d > hi {
                    hi = d;
                }
            }
        }
        if hi == 0 {
            0
        } else {
            yyyymmdd_to_jdn(hi).saturating_sub(yyyymmdd_to_jdn(lo)) + 1
        }
    };

    for (service_id, dates) in &derived.calendar_bitmap.active_dates {
        if dates.is_empty() {
            continue;
        }
        let mut sorted: Vec<u32> = dates.iter().copied().collect();
        sorted.sort_unstable();

        let total_days = sorted.len() as u32;

        // CAL_010: very short service — absolute days on a long feed, RATIO threshold on a short feed.
        let cal010_hit = if feed_window_days >= SHORT_FEED_WINDOW_DAYS {
            total_days <= gap_threshold
        } else if feed_window_days > 0 {
            (total_days as f64) < feed_window_days as f64 * SHORT_FEED_ACTIVE_RATIO
        } else {
            false
        };
        if cal010_hit {
            let (observed, expected, msg) = if feed_window_days >= SHORT_FEED_WINDOW_DAYS {
                (
                    format!("{total_days}"),
                    format!("> {gap_threshold}"),
                    format!("'{service_id}' takviminde yalnızca {total_days} aktif gün var."),
                )
            } else {
                let pct = total_days as f64 * 100.0 / feed_window_days as f64;
                (
                    format!("{total_days}/{feed_window_days} days ({pct:.0}%)"),
                    format!("≥ %{:.0}", SHORT_FEED_ACTIVE_RATIO * 100.0),
                    format!("'{service_id}' takvimi feed'in {feed_window_days} günlük yayın penceresinin yalnızca {total_days} gününde (%{pct:.0}) aktif."),
                )
            };
            notices.push(k6_notice(
                ctr,
                "CAL_010",
                EntityType::Service,
                Some(service_id.clone()),
                Some(service_id.clone()),
                "calendar.txt",
                None,
                None,
                Some(observed),
                Some(expected),
                msg,
                "Servis takvimini genişletin ya da bu servisin kısa olduğunu belgeleyin.",
            ));
        }

        // CAL_007/012: a gap of >= big_gap_days between consecutive active dates for this service.
        // MobilityData big_gap_in_service is PER-SERVICE (not a feed-level union). Gaps are collected
        // across ALL services by (gap_start, gap_end) signature (#30) → emit AFTER the loop.
        for pair in sorted.windows(2) {
            let gap_days = yyyymmdd_to_jdn(pair[1])
                .saturating_sub(yyyymmdd_to_jdn(pair[0]))
                .saturating_sub(1);
            if gap_days >= big_gap_threshold {
                gap_services
                    .entry((pair[0], pair[1]))
                    .or_default()
                    .push(service_id.as_str());
            }
        }

        // CAL_021: the service's active period spans today (min ≤ today ≤ max) BUT
        // has no active day in the next N days (config.upcoming_service_days).
        // Fully past (CAL_013) / fully future (CAL_017) services fail the "spans today"
        // condition → no overlap. Only for services used by trips.
        if today_yyyymmdd > 0 {
            let min_d = sorted[0];
            let max_d = *sorted.last().unwrap();
            if min_d <= today_yyyymmdd && max_d >= today_yyyymmdd {
                let window_end_jdn = today_jdn + config.upcoming_service_days;
                let has_upcoming = sorted.iter().any(|&d| {
                    let jdn = yyyymmdd_to_jdn(d);
                    jdn >= today_jdn && jdn <= window_end_jdn
                });
                let trip_count = service_trip_counts
                    .get(service_id.as_str())
                    .copied()
                    .unwrap_or(0);
                if !has_upcoming && trip_count > 0 {
                    let n = config.upcoming_service_days;
                    notices.push(k6_notice(
                        ctr,
                        "CAL_021",
                        EntityType::Service,
                        Some(service_id.clone()),
                        Some(service_id.clone()),
                        "calendar.txt",
                        None,
                        None,
                        Some(format!("0 / {n}")),
                        Some(format!("≥ 1 / {n}")),
                        format!("'{service_id}' servisi bugünü kapsıyor ama önümüzdeki {n} günde aktif günü yok ({trip_count} sefer etkileniyor)."),
                        "Yakın günlerde sefer olmaması kasıtlıysa yok sayın; değilse calendar/calendar_dates ile yakın tarihlere aktif gün ekleyin.",
                    ));
                }
            }
        }
    }

    // CAL_007/012 emit: ONE notice per gap signature (#30). service_id variants sharing the same
    // operational gap are listed in details["services"] (VAT_002 pattern).
    // The summary is feed-level: `entity_id` is the gap signature (Entity dedup key), with no `scope_key` —
    // a date range is not a service_id and service-scoped roots could not map it.
    // Preserve #29 semantics: a near-future gap is CAL_012, a past/distant gap is CAL_007.
    for ((gap_from, gap_to), mut svcs) in gap_services {
        svcs.sort_unstable();
        svcs.dedup();
        let a_jdn = yyyymmdd_to_jdn(gap_from);
        let b_jdn = yyyymmdd_to_jdn(gap_to);
        let gap_days = b_jdn.saturating_sub(a_jdn).saturating_sub(1);
        // Does the gap overlap today/the near future (today +30 days)?
        let near_future = b_jdn.saturating_sub(1) >= today_jdn && (a_jdn + 1) <= today_jdn + 30;
        let svc_suffix = if svcs.len() == 1 {
            format!("'{}' servisinde", svcs[0])
        } else {
            format!("{} serviste: {}", svcs.len(), svcs.join(", "))
        };
        let entity = format!("{gap_from}-{gap_to}");
        let mut n = if near_future {
            k6_notice(
                ctr,
                "CAL_012",
                EntityType::Feed,
                Some(entity.clone()),
                None,
                "calendar.txt",
                None,
                None,
                Some(format!("{gap_days}")),
                Some(format!("< {big_gap_threshold}")),
                format!("{gap_from}-{gap_to} arası {gap_days} günlük yakın dönem inaktif aralık {svc_suffix}."),
                "Bu aralık planlıysa işlem gerekmez; değilse servis takvimini doğrulayın.",
            )
        } else {
            k6_notice(
                ctr,
                "CAL_007",
                EntityType::Feed,
                Some(entity.clone()),
                None,
                "calendar.txt",
                None,
                None,
                Some(format!("{gap_days}")),
                Some(format!("< {big_gap_threshold}")),
                format!("{gap_from}-{gap_to} arası {gap_days} günlük boşluk {svc_suffix}."),
                "Boşluk kasıtlıysa calendar_dates ile açıklayın; değilse takvimi düzeltin.",
            )
        };
        n.details = Some({
            let mut d = std::collections::BTreeMap::new();
            d.insert("services".to_string(), svcs.join(","));
            d.into()
        });
        notices.push(n);
    }

    // CAL_008 / CAL_009: expiry (by end date).
    for cal in &records.calendars {
        let Some((ey, em, ed)) = cal.end_date else {
            continue;
        };
        let end_yyyymmdd = ey * 10000 + em * 100 + ed;

        if today_yyyymmdd > 0 {
            // CAL_023: end_date is max_calendar_future_years or more years after today →
            // suspicious/low-quality data (e.g. 2050/2099 placeholder dates).
            // Use `>=` so dates exactly at the boundary are included ("three years or more").
            let today_year = today_yyyymmdd / 10000;
            if ey >= today_year + config.max_calendar_future_years {
                notices.push(k6_notice(
                    ctr, "CAL_023", EntityType::Service,
                    Some(cal.service_id.clone()), Some(cal.service_id.clone()),
                    "calendar.txt", Some(cal.line), Some("end_date"),
                    Some(format!("{end_yyyymmdd}")),
                    Some(format!("< {}0101", today_year + config.max_calendar_future_years)),
                    format!("'{}' takviminin end_date'i {end_yyyymmdd} — bugünden {} yıl veya daha fazla ileri; şüpheli/düşük-kaliteli veri (yer-tutucu tarih) olabilir.",
                        cal.service_id, config.max_calendar_future_years),
                    "end_date'i gerçek servis bitiş tarihine güncelleyin; uzak-gelecek tarihler genelde yer-tutucu veya hatalıdır.",
                ));
            }
            // CAL_008: expires soon (end_date is in the future and within the warning threshold).
            // CAL_013 (expired) is no longer HERE — instead of calendar.txt end_date, it is calculated
            // in the CAL_017 loop from combined active_dates, so services extended by calendar_dates
            // are not incorrectly marked "expired" (FP fix).
            let warning_days = config.feed_expiry_warning_days;
            let end_jdn = yyyymmdd_to_jdn(end_yyyymmdd);
            let today_jdn = yyyymmdd_to_jdn(today_yyyymmdd);
            if end_jdn > today_jdn && end_jdn - today_jdn <= warning_days {
                notices.push(k6_notice(
                    ctr,
                    "CAL_008",
                    EntityType::Service,
                    Some(cal.service_id.clone()),
                    Some(cal.service_id.clone()),
                    "calendar.txt",
                    Some(cal.line),
                    Some("end_date"),
                    Some(format!("{end_yyyymmdd}")),
                    Some(format!("> {}", warning_days)),
                    format!(
                        "'{}' takvimi {end_yyyymmdd} tarihinde bitiyor — {} gün kaldı.",
                        cal.service_id,
                        end_jdn - today_jdn
                    ),
                    "Feed'i güncellemeyi planlayın.",
                ));
            }
        }
    }

    // CAL_014: service active dates fall outside the feed_info validity window.
    // Same semantics as MD `service_window_outside_feed_period`: compare the first/last active day
    // of every service with trips (not the raw calendar range) to feed_start/end_date. Because the
    // spec RECOMMENDS data outside the window, this is not a defect but authoritative period information (Info).
    // ⚠️ Formerly this lived in the CLD_007 `calendar_dates.exception_count` loop: only services
    // with exceptions were checked, while services in calendar.txt were never seen.
    if let Some(fi) = records.feed_info.first() {
        if let (Some(fs), Some(fe)) = (fi.feed_start_date, fi.feed_end_date) {
            let fs_yyyymmdd = fs.0 * 10000 + fs.1 * 100 + fs.2;
            let fe_yyyymmdd = fe.0 * 10000 + fe.1 * 100 + fe.2;
            let mut services: Vec<&str> = service_trip_counts.keys().copied().collect();
            services.sort_unstable();
            for service_id in services {
                let Some(dates) = derived.calendar_bitmap.active_dates.get(service_id) else {
                    continue;
                };
                let (Some(&first), Some(&last)) = (dates.iter().min(), dates.iter().max()) else {
                    continue;
                };
                // Fully future services are also evaluated: this is an Interop class for MD parity,
                // and MD `checkServiceWindow` makes no such exception. Because this is Info, it does
                // not penalize a feed whose publication window was announced in advance.
                let days_before = if first < fs_yyyymmdd {
                    yyyymmdd_to_jdn(fs_yyyymmdd) - yyyymmdd_to_jdn(first)
                } else {
                    0
                };
                let days_after = if last > fe_yyyymmdd {
                    yyyymmdd_to_jdn(last) - yyyymmdd_to_jdn(fe_yyyymmdd)
                } else {
                    0
                };
                if days_before == 0 && days_after == 0 {
                    continue;
                }
                let mut n014 = k6_notice(
                    ctr,
                    "CAL_014",
                    EntityType::Service,
                    Some(service_id.to_string()),
                    Some(service_id.to_string()),
                    "calendar.txt",
                    None,
                    Some("feed_start_date|feed_end_date"),
                    Some(format!("{first}..{last}")),
                    Some(format!("{fs_yyyymmdd}..{fe_yyyymmdd}")),
                    format!(
                        "'{service_id}' servisinin aktif tarihleri ({first}–{last}) feed_info'nun yetkili dönemi \
                         ({fs_yyyymmdd}–{fe_yyyymmdd}) dışına taşıyor: {days_before} gün önce, {days_after} gün sonra."
                    ),
                    "Spec bu dönem dışındaki veriyi olası servis bilgisi olarak tavsiye eder; tüketiciler onu yetkili \
                     saymaz. Dönem bilinçliyse işlem gerekmez, değilse feed_start_date/feed_end_date'i güncelleyin.",
                );
                let mut d = std::collections::BTreeMap::new();
                d.insert("days_before".to_string(), days_before.to_string());
                d.insert("days_after".to_string(), days_after.to_string());
                n014.details = Some(d.into());
                notices.push(n014);
            }
        }
    }

    // CLD_007: many calendar_dates exceptions for the same service_id (> half of total active days).
    // exception_count = raw valid row count (CalendarDateIndex semantics).
    // CLD_007 prerequisite: does the feed have a base schedule in calendar.txt?
    let has_base_calendar = !records.calendars.is_empty();
    // The old lookup scanned all calendar rows for every calendar_dates service. Large feeds
    // such as mdb-2904 can have hundreds of thousands of both, turning this into an effectively
    // quadratic pass. Preserve `.find(...)`'s first-match semantics while making the lookup O(1).
    let mut weekly_base_by_service: HashMap<&str, bool> =
        HashMap::with_capacity(records.calendars.len());
    for cal in &records.calendars {
        weekly_base_by_service
            .entry(cal.service_id.as_str())
            .or_insert(cal.days.contains(&Some(1)));
    }
    for (service_id, &count) in &records.calendar_dates.exception_count {
        let service_id = service_id.as_str();
        let active = derived
            .calendar_bitmap
            .active_dates
            .get(service_id)
            .map(|s| s.len() as u32)
            .unwrap_or(0);
        let has_weekly_base = weekly_base_by_service
            .get(service_id)
            .copied()
            .unwrap_or(false);
        // CAL_013 (expired) was removed from here → calculated once for all services in the CAL_017
        // loop from combined active_dates (prevents double emission).

        // Suspicious when more than half of the active days are overrides.
        //
        // ⚠️ The rule is silent when calendar.txt is ABSENT. A feed may intentionally define its
        // calendar only through calendar_dates.txt (GTFS explicitly allows this); then EVERY active
        // day is an exception, `count > active/2` is mathematically GUARANTEED, and the notice does
        // not measure "deviation from a base schedule" — it only reads back the feed's modeling choice.
        // This empty-file guard is in the same class as the has_calendar_txt precedent in ARC_009.
        // The existence of a base schedule is measured separately by ARC_008/CAL rules.
        if has_base_calendar && has_weekly_base && active > 0 && count > active / 2 {
            notices.push(k6_notice(
                ctr,
                "CLD_007",
                EntityType::Service,
                Some(service_id.to_string()),
                Some(service_id.to_string()),
                "calendar_dates.txt",
                None,
                None,
                Some(format!("{count} exception")),
                None,
                format!("'{service_id}' takviminde {count} özel gün tanımı — temel programdan çok sapma."),
                "calendar.txt base schedule'ı güncellemeyi tercih edin; calendar_dates'i istisnalar için kullanın.",
            ));
        }
    }

    if today_yyyymmdd > 0 && !derived.calendar_bitmap.active_dates.is_empty() {
        // CAL_015: earliest active date is in the future — the feed is not active yet.
        let min_date = derived
            .calendar_bitmap
            .active_dates
            .values()
            .flat_map(|s| s.iter().copied())
            .min();
        if let Some(first) = min_date {
            if first > today_yyyymmdd {
                let mut notice = k6_notice(
                    ctr, "CAL_015", EntityType::Feed,
                    None, None, "calendar.txt", None, None,
                    Some(format!("{first}")), Some(format!("≤ {today_yyyymmdd}")),
                    format!("Feed'in en erken servis tarihi {first}; bu tarih henüz gelmedi — seferler bugün için mevcut değil."),
                    "Feed yayınlama zamanlamasını gözden geçirin ya da calendar.txt'i düzeltin.",
                );
                if future_only_jp_date == Some(first) {
                    notice.severity = gtfs_core::Severity::Bilgi;
                    notice.message = format!(
                        "Yaklaşan servis veri kümesi — GTFS-JP feed'indeki tüm aktif servis tarihleri gelecekte başlıyor ({first}); bu beklenen yayınlama deseni kalite skorunu etkilemez."
                    );
                    notice.remediation =
                        "İşlem gerekmez; servis başlangıç tarihinden önce bu GTFS-JP veri kümesini yayınlayın."
                            .to_string().into();
                    // Without `message_variant`, the en/ja/fr template printed the generic CAL_015
                    // text ("fix the calendar"); non-Turkish users could not see the correction.
                    notice.details = Some(
                        [
                            ("first_service_date".to_string(), first.to_string()),
                            ("message_variant".to_string(), "future_only_jp".to_string()),
                        ]
                        .into_iter()
                        .collect(),
                    );
                }
                notices.push(notice);
            }
        }

        // CAL_017: all active dates are in the future (not started yet) — per service.
        // CAL_013: all active dates are in the past (expired). service_id variants ending on the same
        // last active date (expiry) are collected into ONE notice (#30 pattern) and listed in details["services"].
        // Source is combined active_dates → services extended by calendar_dates are not considered expired
        // (FP fix). min>today and max<today are mutually exclusive.
        let mut expired_services: std::collections::BTreeMap<u32, Vec<&str>> =
            std::collections::BTreeMap::new();

        // CAL_017 now depends on a FEED-LEVEL condition: one service starting in the future is not
        // an error — it is a valid service entering operation soon (e.g. a train starting on August 2
        // while the rest of the feed runs today). The warning is meaningful only if the ENTIRE feed
        // has not started. A past service also counts as "feed started"; staleness is measured by CAL_013.
        let feed_has_started = derived.calendar_bitmap.active_dates.values().any(|dates| {
            dates
                .iter()
                .copied()
                .min()
                .is_some_and(|min_d| min_d <= today_yyyymmdd)
        });
        // Determinism: HashMap iteration is unordered → collect and sort by service_id.
        let mut future_services: Vec<(&str, u32)> = Vec::new();

        for (service_id, dates) in &derived.calendar_bitmap.active_dates {
            if dates.is_empty() {
                continue;
            }
            let min_svc = dates.iter().copied().min().unwrap();
            let max_svc = dates.iter().copied().max().unwrap();
            if min_svc > today_yyyymmdd {
                if !feed_has_started {
                    future_services.push((service_id.as_str(), min_svc));
                }
            } else if max_svc < today_yyyymmdd {
                expired_services
                    .entry(max_svc)
                    .or_default()
                    .push(service_id.as_str());
            }
        }
        // CAL_017 emit: only when the entire feed is in the future (feed_has_started is false).
        if future_only_jp_date.is_none() {
            future_services.sort_unstable();
            for (service_id, min_svc) in future_services {
                notices.push(k6_notice(
                    ctr, "CAL_017", EntityType::Service,
                    Some(service_id.to_string()), Some(service_id.to_string()),
                    "calendar.txt", None, Some("start_date"),
                    Some(format!("{min_svc}")), Some(format!("≤ {today_yyyymmdd}")),
                    format!("'{service_id}' takvimi henüz başlamamış; en erken aktif tarih {min_svc}. Feed'de bugün aktif olan hiçbir servis yok."),
                    "Takvim başlangıç tarihini veya calendar_dates.txt girişlerini gözden geçirin.",
                ));
            }
        }

        // CAL_013 emit: ONE notice per last-active-date (expiry) signature.
        for (max_svc, mut svcs) in expired_services {
            svcs.sort_unstable();
            svcs.dedup();
            let msg = if svcs.len() == 1 {
                format!("'{}' takviminin süresi dolmuş (son aktif tarih: {max_svc}) — bu servise ait seferler bugün için bulunamaz.", svcs[0])
            } else {
                format!("Son aktif tarihi {max_svc} olan {} servisin süresi dolmuş: {} — bu servislere ait seferler bugün için bulunamaz.", svcs.len(), svcs.join(", "))
            };
            let mut n = k6_notice(
                ctr,
                "CAL_013",
                EntityType::Feed,
                Some(format!("{max_svc}")),
                None,
                "calendar.txt",
                None,
                Some("end_date"),
                Some(format!("{max_svc}")),
                Some(format!("≥ {today_yyyymmdd}")),
                msg,
                "Feed'i yeni geçerlilik tarihleriyle güncelleyin veya servisleri silin.",
            );
            n.details = Some({
                let mut d = std::collections::BTreeMap::new();
                d.insert("services".to_string(), svcs.join(","));
                d.into()
            });
            notices.push(n);
        }

        // CAL_016: the latest active date extends more than two years into the future.
        let today_jdn = yyyymmdd_to_jdn(today_yyyymmdd);
        let far_future_jdn = today_jdn + 730;
        let max_date = derived
            .calendar_bitmap
            .active_dates
            .values()
            .flat_map(|s| s.iter().copied())
            .max();
        if let Some(last) = max_date {
            let last_jdn = yyyymmdd_to_jdn(last);
            if last_jdn > far_future_jdn {
                notices.push(k6_notice(
                    ctr, "CAL_016", EntityType::Feed,
                    None, None, "calendar.txt", None, None,
                    Some(format!("{last}")), Some("≤ +2 years".to_string()),
                    format!("Feed'in en son servis tarihi {last} — bugünden 2 yıldan fazla ileride; bu veri bozuk olabilir."),
                    "Servis takvimini gerçekçi bir bitiş tarihiyle sınırlandırın.",
                ));
            }
        }
    }
}

// ── WP-09c: Geographic analytics ─────────────────────────────────────────────

// 🔴 `K6::geo_analytics` was ONE task and depended on four files at once
// (`stops`, `routes`, `trips`, `stop_times`). It was the UNION of the real needs of the
// checks inside the gate, even though no block read `routes.txt` at all.
// Result: a broken `routes.txt` also killed `GEO_016`, which needs only `stops.txt`
// — we stayed silent while MD reported Null Island in `tdg-83820`.
// This was a DEPENDENCY defect, not a predicate defect (#160), in the same family as
// `recoverable_stop_coord`: the rule was correct, but the plumbing dropped its input.

/// Geographic checks requiring only `stops.txt`.
fn check_geo_stop_analytics(
    records: &EntityRecords,
    derived: &DerivedData,
    config: &ValidatorConfig,
    notices: &mut Vec<Notice>,
    ctr: &mut u32,
) {
    use crate::timing::Timer;
    // STP_017 / STP_016: two stops that are too close to each other.
    // Latitude-sorted sliding window — O(n log n), instead of the old O(n²) spatial grid.
    // With SPATIAL_CELL_DEG=0.5°, an entire city fell into one cell (~4M pairs); this prevents that.
    let _tstp17 = Timer::start("K6::geo::stp_017");
    {
        let too_close_km = config.stop_too_close_m / 1000.0;
        let lat_band = too_close_km / 111.0;

        let mut sorted_stops: Vec<(f64, f64, &crate::k2::stops::StopRecord)> = records
            .stops
            .iter()
            .filter_map(|s| recoverable_stop_coord(s).map(|(la, lo)| (la, lo, s)))
            .collect();
        sorted_stops.sort_by(|a, b| a.0.total_cmp(&b.0));

        for i in 0..sorted_stops.len() {
            let (la_i, lo_i, sa) = sorted_stops[i];
            // Skip Null Island (|lat|<0.1 ∧ |lon|<0.1): the coordinate is a placeholder and the
            // root cause belongs to GEO_016 (reports per stop). Comparing here causes both duplicate
            // reporting and a QUADRATIC explosion — VlakEShop: 983 stops at (0,0) produced 482,653
            // pair notices that were discarded by dedup; a larger cluster would OOM WASM.
            if la_i.abs() < 0.1 && lo_i.abs() < 0.1 {
                continue;
            }
            for &(la_j, lo_j, sb) in sorted_stops.iter().skip(i + 1) {
                if la_j - la_i > lat_band {
                    break;
                }
                // Compare only BOARDABLE stops (empty/0 location_type): the station skeleton
                // (entrance=2, node=3, boarding area=4) is not a stop. Placing an entrance at
                // the same coordinates as a street stop is a modeling choice, not a data error
                // (VBB: 3,093 entrance↔stop same-coordinate FPs) — "merge stops" would be plainly
                // wrong for these pairs.
                // EXCEPTION: keep station↔station (1↔1) pairs in the comparison — two separate
                // stations at the same/nearby point suggest a duplicate station (review K1b).
                let lt_a = sa.location_type.unwrap_or(0);
                let lt_b = sb.location_type.unwrap_or(0);
                if !((lt_a == 0 && lt_b == 0) || (lt_a == 1 && lt_b == 1)) {
                    continue;
                }
                // Parent/child exception: a child stop may be at the same or very near location as
                // its parent station — this is normal GTFS modeling; do not produce an FP.
                let pa = sa
                    .row
                    .get("parent_station")
                    .map(|s| s.trim())
                    .filter(|s| !s.is_empty());
                let pb = sb
                    .row
                    .get("parent_station")
                    .map(|s| s.trim())
                    .filter(|s| !s.is_empty());
                if pa == Some(sb.stop_id.as_str()) || pb == Some(sa.stop_id.as_str()) {
                    continue;
                }
                // Sibling exemption: two children (platforms) of the same station are close by
                // definition of the hierarchy model — in feeds using station hierarchy, these pairs
                // were the main source of FPs (VBB: 88% of close pairs shared a parent).
                if pa.is_some() && pa == pb {
                    continue;
                }
                let dist_km = haversine_km(la_i, lo_i, la_j, lo_j);
                if dist_km == 0.0 {
                    notices.push(k6_notice(
                        ctr,
                        "STP_016",
                        EntityType::Stop,
                        Some(sa.stop_id.clone()),
                        Some(sa.stop_id.clone()),
                        "stops.txt",
                        Some(sa.line),
                        Some("stop_lat|stop_lon"),
                        Some(format!("({la_i}, {lo_i}) == '{}'", sb.stop_id)),
                        None,
                        format!(
                            "'{}' (kod: '{}') ve '{}' (kod: '{}') tam olarak aynı koordinatta ({la_i:.5}, {lo_i:.5}).",
                            sa.stop_name.as_deref().unwrap_or(sa.stop_id.as_str()), sa.stop_id,
                            sb.stop_name.as_deref().unwrap_or(sb.stop_id.as_str()), sb.stop_id,
                        ),
                        "İki durak aynı konumdaysa birleştirin; farklı duraklar ise koordinatları düzeltin.",
                    ));
                } else if dist_km < too_close_km {
                    let dist_m = dist_km * 1000.0;
                    let n = k6_notice(
                        ctr,
                        "STP_017",
                        EntityType::Stop,
                        Some(sa.stop_id.clone()),
                        Some(sa.stop_id.clone()),
                        "stops.txt",
                        Some(sa.line),
                        Some("stop_lat|stop_lon"),
                        Some(format!("{dist_m:.1}m → '{}'", sb.stop_id)),
                        Some(format!("> {:.0}m", config.stop_too_close_m)),
                        format!("'{}' (kod: '{}') ile '{}' (kod: '{}') arası {dist_m:.1}m — eşik {:.0}m.",
                            sa.stop_name.as_deref().unwrap_or(sa.stop_id.as_str()), sa.stop_id,
                            sb.stop_name.as_deref().unwrap_or(sb.stop_id.as_str()), sb.stop_id,
                            config.stop_too_close_m),
                        "Durakları birleştirin ya da konumlarını doğrulayın.",
                    );
                    notices.push(n);
                }
            }
        }
        // cell_deg is referenced only in this context; GEO_014 does not use it.
        let _ = derived.spatial_index.cell_deg;
    }
    drop(_tstp17);

    // GEO_002: stop coordinates are outside the bounding box (feed-wide statistical anomaly).
    // Calculate the median coordinate of all stops in the feed and flag those far away.
    let _tgeo2 = Timer::start("K6::geo::geo_002_014");
    let coords: Vec<(f64, f64)> = records
        .stops
        .iter()
        .filter_map(recoverable_stop_coord)
        .collect();
    if coords.len() >= 3 {
        let mut lats: Vec<f64> = coords.iter().map(|c| c.0).collect();
        let mut lons: Vec<f64> = coords.iter().map(|c| c.1).collect();
        lats.sort_by(f64::total_cmp);
        lons.sort_by(f64::total_cmp);
        let med_lat = lats[lats.len() / 2];
        let med_lon = lons[lons.len() / 2];

        // Threshold: relative to the feed's own scale, NOT an absolute 200 km (2026-07-25).
        // Measurement (VBB mdb-782, 41,949 stops): p50 21.8 km · p99 132.3 km · p99.9 189.2 km ·
        // max 294 km. The fixed 200 km sat inside the feed's OWN tail → all 20 findings were
        // real stations (Berlin–Wrocław Kulturzug corridor, Stralsund/Wismar Baltic route,
        // Zittau border route). The rule was detecting a "wide feed", not "outlier coordinates",
        // and reporting as an error what GEO_014 already reports.
        //
        // Real coordinate errors are FAR beyond this threshold: lat/lon swap is ~4,900 km for
        // Berlin, sign error ~5,800 km, and (0,0) ~5,800 km (already GEO_016).
        // In a small sample, p99 becomes the outlier ITSELF → require at least 100 stops.
        const GEO002_MIN_KM: f64 = 200.0;
        const GEO002_P99_FACTOR: f64 = 3.0;
        const GEO002_MIN_SAMPLE: usize = 100;
        let threshold_km = if coords.len() >= GEO002_MIN_SAMPLE {
            let mut dists: Vec<f64> = coords
                .iter()
                .map(|&(la, lo)| haversine_km(med_lat, med_lon, la, lo))
                .collect();
            dists.sort_by(f64::total_cmp);
            let p99 = dists[((dists.len() as f64) * 0.99) as usize];
            (p99 * GEO002_P99_FACTOR).max(GEO002_MIN_KM)
        } else {
            GEO002_MIN_KM
        };

        for stop in &records.stops {
            if stop.stop_id.is_empty() {
                continue;
            }
            let (Some(lat), Some(lon)) = (stop.stop_lat, stop.stop_lon) else {
                continue;
            };
            // Null Island guard (CONSISTENT with STP_016/017, GEO_012, GEO_019): coordinates near
            // (0,0) are placeholders and inevitably more than 200 km from the median.
            // GEO_016 reports the root cause; repeating it here would count the same stop twice.
            if lat.abs() < 0.1 && lon.abs() < 0.1 {
                continue;
            }
            // Stops above the threshold → potential coordinate error.
            let d = haversine_km(med_lat, med_lon, lat, lon);
            if d > threshold_km {
                let mut n = k6_notice(
                    ctr,
                    "GEO_002",
                    EntityType::Stop,
                    Some(stop.stop_id.clone()),
                    Some(stop.stop_id.clone()),
                    "stops.txt",
                    Some(stop.line),
                    Some("stop_lat|stop_lon"),
                    Some(format!("({lat:.5},{lon:.5}) — median'dan {d:.0}km")),
                    Some(format!("≤ {threshold_km:.0}km (feed medianından)")),
                    format!("'{}' durağı (kod: '{}') feed medianından {d:.0}km uzakta (eşik {threshold_km:.0}km) — koordinat hatası olabilir.",
                        stop.stop_name.as_deref().unwrap_or(stop.stop_id.as_str()), stop.stop_id),
                    "stop_lat ve stop_lon değerlerini doğrulayın.",
                );
                // Map reference: the rule says "distance from the median", but without the median
                // coordinate in the notice, the UI drew one pin and left "far from what?" unanswered.
                // The median is now in details.
                n.details = Some(
                    [
                        ("med_lat".to_string(), format!("{med_lat:.6}")),
                        ("med_lon".to_string(), format!("{med_lon:.6}")),
                        ("dist_km".to_string(), format!("{d:.1}")),
                    ]
                    .into_iter()
                    .collect(),
                );
                notices.push(n);
            }
        }
    }

    // GEO_014: feed geographic coverage information (Info level — calculate the feed bbox).
    if !coords.is_empty() {
        let min_lat = coords.iter().map(|c| c.0).fold(f64::INFINITY, f64::min);
        let max_lat = coords.iter().map(|c| c.0).fold(f64::NEG_INFINITY, f64::max);
        let min_lon = coords.iter().map(|c| c.1).fold(f64::INFINITY, f64::min);
        let max_lon = coords.iter().map(|c| c.1).fold(f64::NEG_INFINITY, f64::max);
        let span_km = haversine_km(min_lat, min_lon, max_lat, max_lon);
        if span_km > 500.0 {
            // Very wide feed → informational notice.
            notices.push(k6_notice(
                ctr,
                "GEO_014",
                EntityType::Feed,
                None,
                None,
                "stops.txt",
                None,
                None,
                Some(format!("{span_km:.0} km")),
                None,
                format!("Feed coğrafi kapsamı geniş: {span_km:.0}km köşegen (bbox: {min_lat:.3},{min_lon:.3} → {max_lat:.3},{max_lon:.3})."),
                "Bu bilgi notu; büyük ağlar için beklenen bir durumdur.",
            ));
        }
    }
    // GEO_015: Japan coordinate bounds — when feed_lang=ja, stop coordinates must be in Japan.
    // Japan geographic bounds: lat 20.25–45.33, lon 122.56–153.59.
    {
        let is_japanese = records
            .feed_info
            .first()
            .map(|fi| fi.feed_lang.starts_with("ja"))
            .unwrap_or(false);
        if is_japanese {
            for stop in &records.stops {
                if stop.stop_id.is_empty() {
                    continue;
                }
                let (Some(lat), Some(lon)) = (stop.stop_lat, stop.stop_lon) else {
                    continue;
                };
                let in_japan = (20.25..=45.33).contains(&lat) && (122.56..=153.59).contains(&lon);
                if !in_japan {
                    let name = stop
                        .stop_name
                        .as_deref()
                        .filter(|s| !s.is_empty())
                        .unwrap_or(&stop.stop_id);
                    notices.push(k6_notice(
                        ctr, "GEO_015", EntityType::Stop,
                        Some(stop.stop_id.clone()), Some(stop.stop_id.clone()),
                        "stops.txt", Some(stop.line), Some("stop_lat|stop_lon"),
                        Some(format!("{lat:.6},{lon:.6}")), None,
                        format!("'{}' durağının koordinatları ({lat:.6},{lon:.6}) Japonya sınırları dışında (lat: 20.25–45.33, lon: 122.56–153.59).",
                            name),
                        "stop_lat ve stop_lon değerlerinin doğru olduğunu kontrol edin.",
                    ));
                }
            }
        }
    }

    // GEO_016: stop coordinates are near Null Island (|lat| < 0.1 and |lon| < 0.1).
    for stop in &records.stops {
        if stop.stop_id.is_empty() {
            continue;
        }
        let (Some(lat), Some(lon)) = (stop.stop_lat, stop.stop_lon) else {
            continue;
        };
        if lat.abs() < 0.1 && lon.abs() < 0.1 {
            let name = stop
                .stop_name
                .as_deref()
                .filter(|s| !s.is_empty())
                .unwrap_or(&stop.stop_id);
            notices.push(k6_notice(
                ctr, "GEO_016", EntityType::Stop,
                Some(stop.stop_id.clone()), Some(stop.stop_id.clone()),
                "stops.txt", Some(stop.line), Some("stop_lat|stop_lon"),
                Some(format!("{lat:.6},{lon:.6}")), None,
                format!("'{name}' durağının koordinatları ({lat:.6},{lon:.6}) Null Island yakınında — olası koordinat hatası."),
                "stop_lat ve stop_lon değerlerinin gerçek konuma karşılık geldiğini doğrulayın.",
            ));
        }
    }

    // GEO_022: stop latitude is too close to a pole (|lat| > 89) — possible coordinate error (point_near_pole).
    for stop in &records.stops {
        if stop.stop_id.is_empty() {
            continue;
        }
        let Some(lat) = stop.stop_lat else { continue };
        if lat.abs() > 89.0 {
            let name = stop
                .stop_name
                .as_deref()
                .filter(|s| !s.is_empty())
                .unwrap_or(&stop.stop_id);
            notices.push(k6_notice(
                ctr, "GEO_022", EntityType::Stop,
                Some(stop.stop_id.clone()), Some(stop.stop_id.clone()),
                "stops.txt", Some(stop.line), Some("stop_lat"),
                Some(format!("{lat:.6}")), None,
                format!("'{name}' durağının enlemi ({lat:.6}) kutba aşırı yakın — olası koordinat hatası."),
                "stop_lat değerinin gerçek konuma karşılık geldiğini doğrulayın.",
            ));
        }
    }

    // GEO_018: all feed stops lie within a 200 m area — test/placeholder data.
    {
        let coords: Vec<(f64, f64)> = records
            .stops
            .iter()
            .filter_map(recoverable_stop_coord)
            .collect();
        if coords.len() >= 3 {
            let min_lat = coords
                .iter()
                .map(|(lat, _)| *lat)
                .fold(f64::INFINITY, f64::min);
            let max_lat = coords
                .iter()
                .map(|(lat, _)| *lat)
                .fold(f64::NEG_INFINITY, f64::max);
            let min_lon = coords
                .iter()
                .map(|(_, lon)| *lon)
                .fold(f64::INFINITY, f64::min);
            let max_lon = coords
                .iter()
                .map(|(_, lon)| *lon)
                .fold(f64::NEG_INFINITY, f64::max);
            let span_km = haversine_km(min_lat, min_lon, max_lat, max_lon);
            if span_km < 0.2 {
                notices.push(k6_notice(
                    ctr, "GEO_018", EntityType::Feed,
                    None, None, "", None, None,
                    Some(format!("{:.0}m", span_km * 1000.0)),
                    Some("≥200m".to_string()),
                    format!("Feed'deki tüm {} durak {}m'lik bir alan içinde — test/yer tutucu veri olabilir.",
                        coords.len(), (span_km * 1000.0) as u64),
                    "Gerçek durak koordinatlarını stops.txt'e ekleyin.",
                ));
            }
        }
    }

    // GEO_019: stop with integer (zero-decimal) coordinates.
    for stop in &records.stops {
        if stop.stop_id.is_empty() {
            continue;
        }
        let (Some(lat), Some(lon)) = (stop.stop_lat, stop.stop_lon) else {
            continue;
        };
        // Null Island (0,0) is also an integer, but GEO_016 catches it more specifically →
        // do not emit twice. Other integer coordinates (e.g. 41.0,29.0, low precision)
        // remain outside GEO_016 and continue to be reported as GEO_019.
        if lat.abs() < 0.1 && lon.abs() < 0.1 {
            continue;
        }
        if (lat - lat.round()).abs() < 1e-9 && (lon - lon.round()).abs() < 1e-9 {
            let name = stop
                .stop_name
                .as_deref()
                .filter(|s| !s.is_empty())
                .unwrap_or(&stop.stop_id);
            notices.push(k6_notice(
                ctr, "GEO_019", EntityType::Stop,
                Some(stop.stop_id.clone()), Some(stop.stop_id.clone()),
                "stops.txt", Some(stop.line), Some("stop_lat|stop_lon"),
                Some(format!("{:.0},{:.0}", lat, lon)), None,
                format!("'{name}' durağının koordinatları ({:.0},{:.0}) tam sayı — düşük hassasiyetli veya yer tutucu veri.", lat, lon),
                "stop_lat ve stop_lon değerlerini en az 5 ondalık basamakla güncelleyin.",
            ));
        }
    }
}

/// Geographic checks requiring only `shapes.txt`.
fn check_geo_shape_analytics(
    records: &EntityRecords,
    derived: &DerivedData,
    config: &ValidatorConfig,
    rail: &RailIndex<'_>,
    notices: &mut Vec<Notice>,
    ctr: &mut u32,
) {
    let rail_shapes = &rail.shapes;
    use crate::timing::Timer;
    // GEO_006: large jump between shape segments.
    // GEO_007: same — slightly different severity from GEO_006, but the same check.
    let _tg6 = Timer::start("K6::geo::geo_006");
    // GEO_007 scans the same segments with a 3× threshold. Previously the conditions were not
    // mutually exclusive → a jump above 3× produced BOTH GEO_006 and GEO_007, so the same segment
    // was reported twice and counted twice in the R5 score. Now they are layered: this range is
    // GEO_006's, above it is GEO_007's.
    for (shape_id, seg) in &derived.shape_geometry.shapes {
        // Threshold per shape: simplified long straight sections are legitimate on rail routes.
        let max_jump_km = shape_jump_threshold_km(shape_id, rail_shapes, config);
        let severe_km = max_jump_km * 3.0;
        for (i, &dist) in seg.segment_distances_km.iter().enumerate() {
            if dist > severe_km {
                continue; // → GEO_007
            }
            if dist > max_jump_km {
                notices.push(k6_notice(
                    ctr,
                    "GEO_006",
                    EntityType::Shape,
                    Some(shape_id.clone()),
                    Some(shape_id.clone()),
                    "shapes.txt",
                    None,
                    Some("shape_pt_lat|shape_pt_lon"),
                    Some(format!("{dist:.2} km (segment {i}→{})", i + 1)),
                    Some(format!("≤ {max_jump_km} km")),
                    format!("'{shape_id}' güzergahının {i}→{}. segmentinde {dist:.2}km atlama — eşik {max_jump_km}km.",
                        i + 1),
                    "Aralarında koordinat eksik olan güzergah noktalarını ekleyin.",
                ));
            }
        }
    }

    drop(_tg6);

    // GEO_017: shape point near Null Island — at most one notice per shape.
    {
        let mut flagged: HashSet<u32> = HashSet::new();
        for pt in &records.shapes {
            if flagged.contains(&pt.shape_idx()) {
                continue;
            }
            let (Some(lat), Some(lon)) = (pt.shape_pt_lat(), pt.shape_pt_lon()) else {
                continue;
            };
            if lat.abs() < 0.1 && lon.abs() < 0.1 {
                flagged.insert(pt.shape_idx());
                let sid = records.shape_interns.id(pt);
                notices.push(k6_notice(
                    ctr, "GEO_017", EntityType::Shape,
                    Some(sid.to_string()), Some(sid.to_string()),
                    "shapes.txt", Some(pt.line as u64), Some("shape_pt_lat|shape_pt_lon"),
                    Some(format!("{lat:.6},{lon:.6}")), None,
                    format!("'{sid}' şeklinde Null Island yakınında nokta bulundu ({lat:.6},{lon:.6}) — GPS veri hatası olabilir."),
                    "shapes.txt'deki sıfır değerli koordinatları kontrol edin.",
                ));
            }
        }
    }
}

/// Geographic checks requiring the Shape ↔ trip ↔ stop relationship.
fn check_geo_shape_trip_analytics(
    records: &EntityRecords,
    config: &ValidatorConfig,
    rail: &RailIndex<'_>,
    notices: &mut Vec<Notice>,
    ctr: &mut u32,
) {
    // GEO_020: all points of a shape have the same coordinates (degenerate geometry).
    {
        // Count points in the SAME pass: previously `records.shapes.iter().filter(...).count()`
        // was called for every degenerate shape → O(degenerate × all points).
        // Key by the intern index: previously shape_id was CLONED for every point (40.9M Strings).
        let mut shape_first: HashMap<u32, (f64, f64)> = HashMap::new();
        let mut shape_counts: HashMap<u32, usize> = HashMap::new();
        let mut shape_varied: HashSet<u32> = HashSet::new();
        for pt in &records.shapes {
            let (Some(lat), Some(lon)) = (pt.shape_pt_lat(), pt.shape_pt_lon()) else {
                continue;
            };
            let sidx = pt.shape_idx();
            *shape_counts.entry(sidx).or_insert(0) += 1;
            if shape_varied.contains(&sidx) {
                continue;
            }
            if let Some(&(first_lat, first_lon)) = shape_first.get(&sidx) {
                if (lat - first_lat).abs() > 1e-8 || (lon - first_lon).abs() > 1e-8 {
                    shape_varied.insert(sidx);
                }
            } else {
                shape_first.insert(sidx, (lat, lon));
            }
        }
        // Determinism: HashMap order is random (K7 sorting masks it, but emission order must be
        // stable — item 4 of the new rule checklist).
        let mut degenerate: Vec<(&u32, &(f64, f64))> = shape_first
            .iter()
            .filter(|(sidx, _)| !shape_varied.contains(*sidx))
            .collect();
        // Sort by TEXT: the intern index is file order, not alphabetical order — sorting by index
        // would silently change emission order.
        degenerate.sort_by(|a, b| {
            records
                .shape_interns
                .id_at(*a.0)
                .cmp(records.shape_interns.id_at(*b.0))
        });
        for (shape_idx, (lat, lon)) in degenerate {
            let shape_id = records.shape_interns.id_at(*shape_idx);
            // A shape whose every point is (0,0) has placeholder coordinates, and GEO_017 reports
            // it more specifically (same division of responsibility as GEO_016/GEO_019) → no duplicate here.
            if lat.abs() < 0.1 && lon.abs() < 0.1 {
                continue;
            }
            let count = shape_counts.get(shape_idx).copied().unwrap_or(0);
            if count >= 2 {
                notices.push(k6_notice(
                    ctr, "GEO_020", EntityType::Shape,
                    Some(shape_id.to_string()), Some(shape_id.to_string()),
                    // The violation is in the coordinate pair.
                    "shapes.txt", None, Some("shape_pt_lat|shape_pt_lon"),
                    Some(format!("{lat:.6},{lon:.6}")), None,
                    format!("'{shape_id}' şeklinin tüm {count} noktası ({lat:.6},{lon:.6}) koordinatında — dejenere geometri."),
                    "shapes.txt'deki koordinatları gerçek güzergaha karşılık gelecek şekilde düzeltin.",
                ));
            }
        }
    }

    // GEO_021: >30% of stops share coordinates with another stop.
    {
        // Only physical stops (location_type=0 or null). An identical coordinate for a station
        // (lt=1) and child platform is NORMAL (hierarchy); counting them creates a false
        // "systematic coordinate problem" in feeds modeling stations/platforms such as Tokyo Toei.
        let mut coord_counts: HashMap<(i64, i64), u32> = HashMap::new();
        for stop in &records.stops {
            if stop.location_type.unwrap_or(0) != 0 {
                continue;
            }
            let (Some(lat), Some(lon)) = (stop.stop_lat, stop.stop_lon) else {
                continue;
            };
            let key = ((lat * 1e6).round() as i64, (lon * 1e6).round() as i64);
            *coord_counts.entry(key).or_default() += 1;
        }
        let total_stops = records
            .stops
            .iter()
            .filter(|s| {
                s.location_type.unwrap_or(0) == 0 && s.stop_lat.is_some() && s.stop_lon.is_some()
            })
            .count();
        let shared: usize = coord_counts
            .values()
            .filter(|&&c| c > 1)
            .map(|&c| c as usize)
            .sum();
        if total_stops >= 5 && shared as f64 / total_stops as f64 > 0.3 {
            let pct = shared as f64 / total_stops as f64 * 100.0;
            notices.push(k6_notice(
                ctr, "GEO_021", EntityType::Feed,
                None, None, "stops.txt", None, None,
                Some(format!("{pct:.0}%")), Some("≤30%".to_string()),
                format!("{shared}/{total_stops} durak ({pct:.0}%) koordinatını başka bir durakla paylaşıyor — sistematik koordinat sorunu."),
                "stops.txt'deki tekrar eden koordinatları düzeltin.",
            ));
        }
    }

    // STM_044: feed stop_times row count exceeds two million.
    {
        let total_st = records.stop_times_index.total_rows;
        if total_st > config.max_total_stop_times as usize {
            notices.push(k6_notice(
                ctr, "STM_044", EntityType::Feed,
                None, None, "stop_times.txt", None, None,
                Some(format!("{total_st}")), Some("≤2.000.000".to_string()),
                format!("stop_times.txt'de {total_st} satır var — WASM tüketicileri için ciddi bellek/performans riski."),
                "Büyük feed'leri zaman veya coğrafi bölgeye göre parçalara bölün.",
            ));
        }
    }

    // STM_045: trip departure exceeds the service-day window (possible data anomaly).
    // Service day [start, start+24) → a departure beyond start+24 is suspicious. The threshold
    // depends on service_day_start_hour (e.g. up to 28h is legitimate for a 04:00 operator start);
    // fixed 26h produced false positives. start_hour=0 (normalization disabled) → old 26h.
    //
    // Rail exception: departure times such as 30:37, 33:56, and 38:10 are VALID GTFS in
    // long-distance train timetables — post-midnight times use 24:xx+ service-day notation,
    // and reducing them to 24h breaks trip chronology. Rail trips use the
    // `service_day_window_hours_rail` window (default 48); real typos (e.g. 99:00) are still
    // caught. STM_028 separately measures total trip duration with `max_trip_duration_hours_rail`.
    {
        let max_dep_h = if config.service_day_start_hour == 0 {
            26
        } else {
            24 + config.service_day_start_hour
        };
        let max_dep_h_rail = config.service_day_window_hours_rail;
        for (trip_id, stops) in records.stop_times_index.iter_trips() {
            let max_dep_h = if rail.trips.contains(trip_id.as_str()) {
                max_dep_h_rail
            } else {
                max_dep_h
            };
            for st in stops {
                let Some((h, m, s)) = st.departure_time() else {
                    continue;
                };
                if h > max_dep_h || (h == max_dep_h && (m > 0 || s > 0)) {
                    let mut n = k6_notice(
                        ctr, "STM_045", EntityType::Trip,
                        Some(trip_id.to_string()), Some(trip_id.to_string()),
                        "stop_times.txt", Some(st.line as u64), Some("departure_time"),
                        Some(format!("{h:02}:{m:02}:{s:02}")), Some(format!("≤{max_dep_h}:00:00")),
                        format!("'{trip_id}' seferinde {h:02}:{m:02}:{s:02} kalkış saati — servis günü penceresini ({max_dep_h} saat) aşıyor."),
                        "departure_time ve service_day_start_hour ayarını kontrol edin; bu pencereyi aşan değerler genellikle veri hatasıdır.",
                    );
                    // Let the planner see which stop has the departure beyond the window.
                    let mut d = std::collections::BTreeMap::new();
                    d.insert(
                        "stop_id".to_string(),
                        records.stop_times_index.stop_id_of(st).to_string(),
                    );
                    n.details = Some(d.into());
                    notices.push(n);
                    break; // one notice per trip is enough
                }
            }
        }
    }

    // SHP_027 (shape assigned to multiple stop patterns) REMOVED (2026-07-31):
    // shapes.txt represents the PHYSICAL route, while stop_times.txt represents the STOPPING MODEL.
    // It is normal for trains stopping at different stations on the same rail corridor to share one
    // shape (Konya-Söğütlüçeşme: some stop at Bilecik, some at Pamukova, some at Bostancı).
    // Patterns not being subsets of one another does not show that the shape is wrong.
    // The remaining REAL signal — a stop not lying on the shape — is measured DIRECTLY by GEO_009
    // and SHP_012; the card's own measurement confirmed this (VBB 14,084 shapes: SHP_027 absent
    // from 83% of SHP_012 findings, so the rule mostly fired on legitimate cases).

    // STM_043: trip has too many stops (>200).
    {
        let trip_ids_set: FxHashSet<&str> =
            records.trips.iter().map(|t| t.trip_id.as_str()).collect();
        for (trip_id, stops) in records.stop_times_index.iter_trips() {
            let count = stops.len() as u32;
            let trip_id = trip_id.as_str();
            if count > config.max_stops_per_trip && trip_ids_set.contains(trip_id) {
                let dep_suffix = stops
                    .first()
                    .and_then(|s| s.departure_time())
                    .map(|(h, m, _)| format!(" {h:02}:{m:02} kalkışlı"))
                    .unwrap_or_default();
                notices.push(k6_notice(
                    ctr, "STM_043", EntityType::Trip,
                    Some((*trip_id).to_string()), Some((*trip_id).to_string()),
                    "stop_times.txt", None, None,
                    Some(count.to_string()), Some("≤200".to_string()),
                    format!("'{trip_id}'{dep_suffix} seferinde {count} durak var — olası veri birleştirme hatası."),
                    "Bu sefer için stop_times.txt'i gözden geçirin; seferler mantıksal segmentlere ayrılabilir.",
                ));
            }
        }
    }

    // SHP_026: shape has too many points (>5000).
    {
        let mut shape_counts: FxHashMap<&str, u32> = FxHashMap::default();
        for pt in &records.shapes {
            *shape_counts
                .entry(records.shape_interns.id(pt))
                .or_default() += 1;
        }
        for (shape_id, count) in &shape_counts {
            if *count > config.max_shape_points {
                notices.push(k6_notice(
                    ctr, "SHP_026", EntityType::Shape,
                    Some((*shape_id).to_string()), Some((*shape_id).to_string()),
                    "shapes.txt", None, None,
                    Some(count.to_string()), Some("≤5000".to_string()),
                    format!("'{shape_id}' şeklinde {count} nokta var — harita render performansını olumsuz etkiler."),
                    "shapes.txt'i basitleştirmek için Douglas-Peucker vb. bir algoritma kullanın.",
                ));
            }
        }
    }

    // (`_tgeo2` now drops at the end of `check_geo_stop_analytics` — #160 split).
}

// ── WP-09c: Operasyonel analitik ─────────────────────────────────────────────

fn check_operational_analytics(
    records: &EntityRecords,
    derived: &DerivedData,
    _config: &ValidatorConfig,
    idx: &StopTimesIndex<'_>,
    today_yyyymmdd: u32,
    notices: &mut Vec<Notice>,
    ctr: &mut u32,
) {
    use crate::timing::Timer;
    let ti_opr = &records.trip_interns;
    let future_only_jp = gtfs_jp_future_only_date(records, derived, today_yyyymmdd).is_some();

    let trip_to_route: HashMap<&str, &str> = records
        .trips
        .iter()
        .map(|t| (t.trip_id.as_str(), ti_opr.route_id(t)))
        .collect();
    let stop_name_map: HashMap<&str, &str> = records
        .stops
        .iter()
        .filter_map(|s| s.stop_name.as_deref().map(|n| (s.stop_id.as_str(), n)))
        .collect();

    // OPR_006: too few stops per trip (< 2) — only trips with stop_times.
    // (XFL_002 in K4 catches "trip has no stop_times"; this is for one-stop trips.)
    let _topr6 = Timer::start("K6::opr::opr_006");
    for (&trip_id, stimes) in &idx.by_trip {
        let count = stimes.len();
        if count < 2 {
            let line = stimes.first().map(|s| s.line as u64);
            notices.push(k6_notice(
                ctr,
                "OPR_006",
                EntityType::Trip,
                Some(trip_id.to_string()),
                Some(trip_id.to_string()),
                "stop_times.txt",
                line,
                None,
                Some(format!("{count}")),
                Some("≥ 2".to_string()),
                {
                    let r = trip_to_route.get(trip_id).copied().unwrap_or(trip_id);
                    format!("'{r}' hattının seferinde yalnızca {count} durak var — işlevsel sefer için en az 2 durak gerekli.")
                },
                "En az 2 durak içeren bir sefer tanımlayın.",
            ));
        }
    }

    drop(_topr6);

    // OPR_007: same stop_id repeated, excluding ring terminals and consecutive repeats owned by STM_035.
    let _topr7 = Timer::start("K6::opr::opr_007");
    {
        let mut stop_counts: HashMap<&str, u32> = HashMap::new();
        for (&trip_id, stimes) in &idx.by_trip {
            stop_counts.clear();
            for st in stimes.iter() {
                if !idx.stop_id_of(st).is_empty() {
                    *stop_counts.entry(idx.stop_id_of(st)).or_default() += 1;
                }
            }
            // Detect ring/circular routes: suppress the terminal repeat when the first and last stops match.
            let mut sorted_stops = stimes
                .iter()
                .filter(|st| !idx.stop_id_of(st).is_empty())
                .collect::<Vec<_>>();
            sorted_stops.sort_by_key(|st| st.stop_sequence().unwrap_or(u32::MAX));
            let first = sorted_stops
                .first()
                .map(|st| idx.stop_id_of(st))
                .unwrap_or("");
            let last = sorted_stops
                .last()
                .map(|st| idx.stop_id_of(st))
                .unwrap_or("");
            let is_ring = !first.is_empty() && first == last;

            // `min_by_key` INSTEAD OF `find`: `stop_counts` is a HashMap and one trip can have
            // multiple repeated stops — `find` left the choice to iteration order, which changes
            // between processes and caused the same feed to report two different stops (306 trips
            // in VBB). The smallest stop_id is deterministic.
            if let Some((&dup_stop, &count)) = stop_counts
                .iter()
                .filter(|(&sid, &c)| {
                    c > 1 && !(is_ring && sid == first) && {
                        let mut prev = None;
                        sorted_stops
                            .iter()
                            .enumerate()
                            .filter(|(_, st)| idx.stop_id_of(st) == sid)
                            .any(|(i, _)| prev.replace(i).is_some_and(|p| i > p + 1))
                    }
                })
                .min_by_key(|(&sid, _)| sid)
            {
                let route = trip_to_route.get(trip_id).copied().unwrap_or(trip_id);
                let stop_name = stop_name_map.get(dup_stop).copied().unwrap_or(dup_stop);
                // Departure time: makes rows distinguishable when a route has many trips.
                let dep = stimes
                    .first()
                    .and_then(|s| s.departure_time().or(s.arrival_time()))
                    .map(|(h, m, _)| format!("{h:02}:{m:02} "))
                    .unwrap_or_default();
                let mut n = k6_notice(
                    ctr,
                    "OPR_007",
                    EntityType::Trip,
                    Some(trip_id.to_string()),
                    Some(trip_id.to_string()),
                    "stop_times.txt",
                    None,
                    None,
                    Some(format!("{count}× repeat")),
                    None,
                    format!("'{}' hattının {}seferinde '{}' durağı (kod: '{}') {}× geçiyor — tekrarlayan operasyon deseni.",
                        route, dep, stop_name, dup_stop, count),
                    "Bu rota deseni bilinçliyse işlem gerekmez; değilse stop_times.txt durak sırasını doğrulayın.",
                );
                // Ordered stop list + repeated stop for the map.
                let stop_list: Vec<&str> = stimes
                    .iter()
                    .filter(|st| !idx.stop_id_of(st).is_empty())
                    .map(|st| idx.stop_id_of(st))
                    .collect();
                let mut det = std::collections::BTreeMap::new();
                det.insert("stops".to_string(), stop_list.join(","));
                det.insert("dup_stop".to_string(), dup_stop.to_string());
                n.details = Some(det.into());
                notices.push(n);
            }
        }
    }

    drop(_topr7);

    // OPR_011: the trip's service_id has no active day in calendar_bitmap.
    // XFL_006 (K4) catches only the "removal exception only" case;
    // OPR_011 is broader and also covers service_ids with no bitmap entry at all.
    {
        let mut reported_services: HashSet<&str> = HashSet::new();
        for t in &records.trips {
            let svc = ti_opr.service_id(t);
            if reported_services.contains(svc) {
                continue;
            }
            let has_active = derived
                .calendar_bitmap
                .active_dates
                .get(svc)
                .map(|dates| !dates.is_empty())
                .unwrap_or(false);
            if !has_active {
                reported_services.insert(svc);
                notices.push(k6_notice(
                    ctr,
                    "OPR_011",
                    EntityType::Service,
                    Some(svc.to_string()),
                    Some(svc.to_string()),
                    "calendar.txt",
                    None,
                    Some("service_id"),
                    Some("0".to_string()),
                    Some("≥ 1".to_string()),
                    format!("'{svc}' takviminde hiçbir aktif gün yok — bu takvimi kullanan seferler çalışmayacak."),
                    "calendar.txt veya calendar_dates.txt'de geçerli aktif günler tanımlayın.",
                ));
            }
        }
    }

    // OPR_016: no service_id in the feed has an active day (feed-level).
    {
        if !records.trips.is_empty() {
            let any_active = derived
                .calendar_bitmap
                .active_dates
                .values()
                .any(|dates| !dates.is_empty());
            if !any_active {
                notices.push(k6_notice(
                    ctr,
                    "OPR_016",
                    EntityType::Feed,
                    None,
                    None,
                    "calendar.txt",
                    None,
                    None,
                    Some("0".to_string()),
                    Some("≥ 1".to_string()),
                    "Feed'deki hiçbir service_id aktif takvim günü içermiyor — tüm seferler pasif.".to_string(),
                    "calendar.txt veya calendar_dates.txt'de en az bir servis için aktif gün tanımlayın.",
                ));
            }
        }
    }

    // TRP_023: no active trip in the next 7 days.
    if today_yyyymmdd > 0 && !records.trips.is_empty() && !future_only_jp {
        let today_jdn = yyyymmdd_to_jdn(today_yyyymmdd);
        let active_in_7days = derived.calendar_bitmap.active_dates.values().any(|dates| {
            dates.iter().any(|&d| {
                let djdn = yyyymmdd_to_jdn(d);
                djdn >= today_jdn && djdn < today_jdn + 7
            })
        });
        if !active_in_7days {
            notices.push(k6_notice(
                ctr, "TRP_023", EntityType::Feed,
                None, None, "calendar.txt", None, None,
                Some("0".to_string()), None,
                "Önümüzdeki 7 günde aktif sefer bulunamadı — feed yakında devre dışı kalabilir.".to_string(),
                "calendar.txt veya calendar_dates.txt'i güncelleyin ya da yeni bir feed yayınlayın.",
            ));
        }
    }

    // CAL_024 (formerly TRP_030): calendar inactive in the next 7 days — aggregate per service_id.
    // (The same inactive calendar can cover thousands of trips; the action unit is service_id,
    // not trip — therefore emit one calendar summary instead of one notice per trip.)
    if today_yyyymmdd > 0 && !records.trips.is_empty() && !future_only_jp {
        let today_jdn = yyyymmdd_to_jdn(today_yyyymmdd);
        // service_id → (affected trip count, first trip row)
        let mut inactive_by_service: FxHashMap<&str, (u32, u64)> = FxHashMap::default();
        for trip in &records.trips {
            if trip.trip_id.is_empty() {
                continue;
            }
            let dates = derived.calendar_bitmap.active_dates.get(ti_opr.service_id(trip));
            let active_in_7 = dates
                .map(|dates| {
                    dates.iter().any(|&d| {
                        let djdn = yyyymmdd_to_jdn(d);
                        djdn >= today_jdn && djdn < today_jdn + 7
                    })
                })
                .unwrap_or(false);
            // A service not yet started (first active day after today) is not a defect:
            // it is the timetable for a future period in the same dataset (e.g. Goshogawara's
            // winter service). Same rationale as CAL_017; if the entire feed is future, the
            // condition is already reported by CAL_015/CAL_017/TRP_023.
            let not_started = dates
                .and_then(|dates| dates.iter().copied().min())
                .is_some_and(|first| first > today_yyyymmdd);
            if !active_in_7 && !not_started {
                let e = inactive_by_service
                    .entry(ti_opr.service_id(trip))
                    .or_insert((0, trip.line));
                e.0 += 1;
            }
        }
        // Sort by service_id for deterministic output.
        let mut services: Vec<(&str, (u32, u64))> = inactive_by_service.into_iter().collect();
        services.sort_by(|a, b| a.0.cmp(b.0));
        for (service_id, (count, line)) in services {
            notices.push(k6_notice(
                ctr, "CAL_024", EntityType::Service,
                Some(service_id.to_string()), Some(service_id.to_string()),
                "trips.txt", Some(line), Some("service_id"),
                Some(count.to_string()), None,
                format!("'{service_id}' takvimi önümüzdeki 7 günde aktif değil — {count} sefer etkileniyor."),
                "calendar.txt veya calendar_dates.txt'i güncelleyin ya da yeni bir feed yayınlayın.",
            ));
        }
    }

    // TRP_026: trip with no active service day (UnusedTripNotice).
    if today_yyyymmdd > 0 {
        for trip in &records.trips {
            if trip.trip_id.is_empty() {
                continue;
            }
            let has_any_date = derived
                .calendar_bitmap
                .active_dates
                .get(ti_opr.service_id(trip))
                .map(|dates| !dates.is_empty())
                .unwrap_or(false);
            if !has_any_date {
                let svc = ti_opr.service_id(trip);
                notices.push(k6_notice(
                    ctr, "TRP_026", EntityType::Trip,
                    Some(trip.trip_id.to_string()), Some(trip.trip_id.to_string()),
                    "trips.txt", Some(trip.line), Some("service_id"),
                    Some(svc.to_string()), None,
                    format!("service_id '{}' için geçerli hizmet tarihi yok; '{}' seferi hiçbir zaman çalışmayacak.",
                        svc, trip.trip_id),
                    "service_id'nin calendar.txt veya calendar_dates.txt'te aktif tarihlere sahip olduğundan emin olun.",
                ));
            }
        }
    }

    // TRP_028/029: missing wheelchair_accessible.
    {
        let total = records
            .trips
            .iter()
            .filter(|t| !t.trip_id.is_empty())
            .count();
        if total > 0 {
            let unset = records
                .trips
                .iter()
                .filter(|t| !t.trip_id.is_empty() && t.wheelchair_accessible.unwrap_or(0) == 0)
                .count();
            if unset == total {
                notices.push(k6_notice(
                    ctr, "TRP_029", EntityType::Feed,
                    None, None,
                    "trips.txt", None, Some("wheelchair_accessible"),
                    Some(format!("{total}")), Some("1 or 2".to_string()),
                    format!("{total} seferin hiçbirinde wheelchair_accessible bilgisi girilmemiş."),
                    "Tekerlekli sandalye erişilebilirliğini wheelchair_accessible alanıyla bildirin (1=erişilebilir, 2=erişilemez).",
                ));
            } else if unset > 0 {
                notices.push(k6_notice(
                    ctr, "TRP_028", EntityType::Feed,
                    None, None,
                    "trips.txt", None, Some("wheelchair_accessible"),
                    Some(format!("{unset}/{total}")), Some("0".to_string()),
                    format!("{total} seferin {unset} tanesinde wheelchair_accessible bilgisi eksik ({:.0}%).",
                        unset as f64 / total as f64 * 100.0),
                    "Tüm seferlerin wheelchair_accessible alanını doldurun (1=erişilebilir, 2=erişilemez).",
                ));
            }
        }
    }

    // STP_037/038: missing wheelchair_boarding (Cal-ITP ACC-1), the stop-level analogue of
    // TRP_028/029. Count only physical stops/platforms (location_type 0 or empty); stations/entrances
    // (1/2/3/4) have different accessibility semantics and are excluded from the denominator (FP prevention).
    {
        let total = records
            .stops
            .iter()
            .filter(|s| !s.stop_id.is_empty() && matches!(s.location_type, None | Some(0)))
            .count();
        if total > 0 {
            let unset = records
                .stops
                .iter()
                .filter(|s| {
                    !s.stop_id.is_empty()
                        && matches!(s.location_type, None | Some(0))
                        && matches!(s.wheelchair_boarding, None | Some(0))
                })
                .count();
            if unset == total {
                notices.push(k6_notice(
                    ctr, "STP_038", EntityType::Feed,
                    None, None,
                    "stops.txt", None, Some("wheelchair_boarding"),
                    Some(format!("{total}")), Some("1 or 2".to_string()),
                    format!("{total} fiziksel durağın hiçbirinde wheelchair_boarding bilgisi girilmemiş."),
                    "Durakların tekerlekli sandalye erişilebilirliğini wheelchair_boarding ile bildirin (1=erişilebilir, 2=erişilemez).",
                ));
            } else if unset > 0 {
                notices.push(k6_notice(
                    ctr, "STP_037", EntityType::Feed,
                    None, None,
                    "stops.txt", None, Some("wheelchair_boarding"),
                    Some(format!("{unset}/{total}")), Some("0".to_string()),
                    format!("{total} fiziksel durağın {unset} tanesinde wheelchair_boarding bilgisi eksik ({:.0}%).",
                        unset as f64 / total as f64 * 100.0),
                    "Eksik duraklarda wheelchair_boarding alanını 1 (erişilebilir) veya 2 (erişilemez) olarak doldurun.",
                ));
            }
        }
    }

    // TRP_024: inconsistent route type within a block — ONE finding per block.
    //
    // 🔴 The rule emitted per trip and scaled with block size: `tdg-83634` produced 5,798 findings
    // while MobilityData reports 204 on the same feed (28x). By contrast, `tdg-81942` gives 1,870
    // for both of us — its blocks are single-trip. The defect is a PROPERTY OF THE BLOCK
    // ("this block changes type"), not of the trip; same pattern as STM_061 pair aggregation
    // and STM_014 segment aggregation.
    {
        let route_type_map: HashMap<&str, u32> = records
            .routes
            .iter()
            .filter(|r| !r.route_id.is_empty())
            .filter_map(|r| r.route_type.map(|rt| (r.route_id.as_str(), rt)))
            .collect();
        // block → first seen (type, trip, row); and block → conflict summary.
        struct BlockConflict<'a> {
            first_type: u32,
            first_trip: &'a str,
            first_line: u64,
            other_types: BTreeSet<u32>,
            trips: Vec<&'a str>,
        }
        let mut block_route_types: HashMap<&str, (u32, &str, u64)> = HashMap::new();
        let mut conflicts: HashMap<&str, BlockConflict<'_>> = HashMap::new();
        for t in &records.trips {
            let Some(bid) = ti_opr.block_id(t) else {
                continue;
            };
            let Some(&rtype) = route_type_map.get(ti_opr.route_id(t)) else {
                continue;
            };
            let entry = block_route_types
                .entry(bid)
                .or_insert((rtype, t.trip_id.as_str(), t.line));
            // Compare VEHICLE mode, not raw codes: 3 ↔ 715 are both buses (2026-10-13,
            // about 96% of TRP_024 in the corpus was this type). Messages still carry raw codes.
            if route_type_vehicle_mode(entry.0) != route_type_vehicle_mode(rtype) {
                let c = conflicts.entry(bid).or_insert_with(|| BlockConflict {
                    first_type: entry.0,
                    first_trip: entry.1,
                    first_line: entry.2,
                    other_types: BTreeSet::new(),
                    trips: Vec::new(),
                });
                c.other_types.insert(rtype);
                c.trips.push(t.trip_id.as_str());
            }
        }
        // HashMap iteration is unordered → sort by block key (golden determinism).
        let mut bids: Vec<&str> = conflicts.keys().copied().collect();
        bids.sort_unstable();
        for bid in bids {
            let BlockConflict {
                first_type,
                first_trip,
                first_line,
                other_types: types,
                trips,
            } = &conflicts[bid];
            let n = trips.len();
            let type_list: Vec<String> = types.iter().map(|t| t.to_string()).collect();
            let msg = if n == 1 {
                format!("block_id '{bid}' içinde farklı rota tipleri: '{first_trip}' tip-{first_type} ve '{}' tip-{}.",
                    trips[0], type_list[0])
            } else {
                format!("block_id '{bid}' içinde farklı rota tipleri: '{first_trip}' tip-{first_type}, ayrıca {n} seferde tip-{}. Tek blok, tek kök neden: seferler ayrı ayrı listelenmez.",
                    type_list.join("/"))
            };
            let mut notice = k6_notice(
                ctr,
                "TRP_024",
                EntityType::Trip,
                Some(bid.to_string()),
                Some(bid.to_string()),
                "trips.txt",
                Some(*first_line),
                Some("block_id"),
                Some(format!("route_type={}", type_list.join("/"))),
                Some(format!("route_type={first_type}")),
                msg,
                "Aynı block içindeki tüm seferlerin aynı rota tipine sahip olmasını sağlayın.",
            );
            let mut d = std::collections::BTreeMap::new();
            d.insert("block_id".to_string(), bid.to_string());
            d.insert("trip_count".to_string(), n.to_string());
            d.insert("first_trip".to_string(), first_trip.to_string());
            let mut sample: Vec<&str> = trips.clone();
            sample.sort_unstable();
            sample.dedup();
            d.insert(
                "trips".to_string(),
                sample
                    .iter()
                    .take(STM014_TRIP_SAMPLE)
                    .copied()
                    .collect::<Vec<_>>()
                    .join(","),
            );
                notice.details = Some(d.into());
            notices.push(notice);
        }
    }

    // TRP_022: overlapping trip times within a block.
    {
        // Calculate [first_dep_secs, last_arr_secs] for each trip.
        let mut trip_range: HashMap<&str, (u32, u32)> = HashMap::new();
        for (&trip_id, stimes) in &idx.by_trip {
            let first_dep = stimes
                .iter()
                .find_map(|s| s.departure_time())
                .map(hms_to_secs);
            let last_arr = stimes
                .iter()
                .rev()
                .find_map(|s| s.arrival_time())
                .map(hms_to_secs);
            if let (Some(dep), Some(arr)) = (first_dep, last_arr) {
                trip_range.insert(trip_id, (dep, arr));
            }
        }

        let mut block_trips: HashMap<&str, Vec<(&str, u32, u32, &str)>> = HashMap::new();
        for t in &records.trips {
            let Some(bid) = ti_opr.block_id(t) else {
                continue;
            };
            let Some(&(dep, arr)) = trip_range.get(t.trip_id.as_str()) else {
                continue;
            };
            block_trips.entry(bid).or_default().push((
                t.trip_id.as_str(),
                dep,
                arr,
                ti_opr.service_id(t),
            ));
        }

        // Calendar-overlap cache (#29): two trips with DIFFERENT service_ids in the same block
        // truly overlap only when active on the SAME day — one vehicle runs only one on different days.
        // Without this guard, large variant-service blocks such as TriMet produced 201k+ FPs (MD: 908, us: 201,476).
        let mut svc_overlap_cache: HashMap<(&str, &str), bool> = HashMap::new();

        for (block_id, trips) in &block_trips {
            for i in 0..trips.len() {
                for j in (i + 1)..trips.len() {
                    let (tid_a, dep_a, arr_a, svc_a) = trips[i];
                    let (tid_b, dep_b, arr_b, svc_b) = trips[j];
                    if dep_a < arr_b && dep_b < arr_a {
                        // Same service → definitely the same day. Different service → calendars must overlap.
                        if svc_a != svc_b {
                            let key = if svc_a < svc_b {
                                (svc_a, svc_b)
                            } else {
                                (svc_b, svc_a)
                            };
                            let same_day =
                                *svc_overlap_cache.entry(key).or_insert_with(|| {
                                    match (
                                        derived.calendar_bitmap.active_dates.get(svc_a),
                                        derived.calendar_bitmap.active_dates.get(svc_b),
                                    ) {
                                        (Some(a), Some(b)) => a.intersection(b).next().is_some(),
                                        _ => false,
                                    }
                                });
                            if !same_day {
                                continue;
                            }
                        }
                        notices.push(k6_notice(
                            ctr, "TRP_022", EntityType::Trip,
                            Some(tid_a.to_string()), Some(tid_a.to_string()),
                            "trips.txt", None, Some("block_id"),
                            Some(format!("{tid_a} / {tid_b}")),
                            None,
                            format!("block_id '{block_id}' içinde '{tid_a}' ve '{tid_b}' seferlerinin saatleri çakışıyor."),
                            "Aynı araç bloğundaki seferlerin saatlerini örtüşmeyecek şekilde düzenleyin.",
                        ));
                    }
                }
            }
        }
    }

    // TRP_025: high ratio (>80%) of trips with missing or unspecified (0) wheelchair_accessible.
    if !records.trips.is_empty() {
        let unknown = records
            .trips
            .iter()
            .filter(|t| !t.trip_id.is_empty())
            .filter(|t| matches!(t.wheelchair_accessible, None | Some(0)))
            .count();
        let total = records
            .trips
            .iter()
            .filter(|t| !t.trip_id.is_empty())
            .count();
        if total > 0 {
            let ratio = unknown as f64 / total as f64;
            // unknown < total: 100% missing belongs to TRP_029. Without the guard, every TRP_029
            // trigger also triggered TRP_025 (100% > 80%), producing two INFO notices for the same
            // feed-level fact. Now TRP_025 = 80–99%, TRP_029 = 100%.
            if ratio > 0.80 && unknown < total {
                notices.push(k6_notice(
                    ctr, "TRP_025", EntityType::Feed,
                    None, None, "trips.txt", None, Some("wheelchair_accessible"),
                    Some(format!("{:.0}%", ratio * 100.0)),
                    Some("≤ 80%".to_string()),
                    format!("Seferlerin %{:.0}'ında wheelchair_accessible bilgisi eksik veya bilinmiyor (0) — erişilebilirlik verisi yetersiz.",
                        ratio * 100.0),
                    "trips.txt'deki wheelchair_accessible alanını 1 (erişilebilir) veya 2 (erişilemez) olarak doldurun.",
                ));
            }
        }
    }
}

// ── WP-09d: Derived stop-times checks ─────────────────────────────────────────

/// STM_015/016 evidence: the untimed edge stop, the same row's departure time (if any), the nearest
/// timed stop (`next` = beyond the first stop, `prev` = before the last stop), and how many stops
/// in the trip have times. This lets the planner see between which times to search for the missing
/// time, or that the trip has no times at all.
/// STM_028/029: total duration is measured from the trip edges; show the first departure and last
/// arrival stops with their times to the planner.
fn trip_span_details(
    idx: &StopTimesIndex<'_>,
    first: &CompactStopTime,
    last: &CompactStopTime,
) -> gtfs_core::NoticeDetails {
    let mut d = gtfs_core::NoticeDetails::new();
    d.insert("first_stop_id".to_string(), idx.stop_id_of(first).to_string());
    if let Some(dep) = first.departure_time() {
        d.insert("first_departure".to_string(), format_hms(hms_to_secs(dep)));
    }
    d.insert("last_stop_id".to_string(), idx.stop_id_of(last).to_string());
    if let Some(arr) = last.arrival_time() {
        d.insert("last_arrival".to_string(), format_hms(hms_to_secs(arr)));
    }
    d
}

fn edge_time_details(
    idx: &StopTimesIndex<'_>,
    stimes: &[CompactStopTime],
    edge: &CompactStopTime,
    side: &str,
) -> gtfs_core::NoticeDetails {
    let mut d = gtfs_core::NoticeDetails::new();
    d.insert("stop_id".to_string(), idx.stop_id_of(edge).to_string());
    if let Some(dep) = edge.departure_time() {
        d.insert("departure".to_string(), format_hms(hms_to_secs(dep)));
    }
    let timed = stimes.iter().filter(|st| row_time_text(st).is_some()).count();
    d.insert("stop_count".to_string(), stimes.len().to_string());
    d.insert("timed_stops".to_string(), timed.to_string());
    fn with_time<'a>(st: &'a CompactStopTime, edge: &CompactStopTime) -> Option<(&'a CompactStopTime, String)> {
        if std::ptr::eq(st, edge) { None } else { row_time_text(st).map(|time| (st, time)) }
    }
    let nearest = if side == "next" {
        stimes.iter().find_map(|st| with_time(st, edge))
    } else {
        stimes.iter().rev().find_map(|st| with_time(st, edge))
    };
    if let Some((other, time)) = nearest {
        d.insert(format!("{side}_stop_id"), idx.stop_id_of(other).to_string());
        d.insert(format!("{side}_time"), time);
    }
    d
}

/// Add the two rows immediately following the first stop. These keys are separate from
/// `next_*`, which keeps its existing meaning: the nearest following row that has a time.
fn add_stm015_following_rows(
    details: &mut gtfs_core::NoticeDetails,
    idx: &StopTimesIndex<'_>,
    stimes: &[CompactStopTime],
) {
    for (prefix, row) in [("following", stimes.get(1)), ("following2", stimes.get(2))] {
        let Some(row) = row else { continue };
        details.insert(format!("{prefix}_stop_id"), idx.stop_id_of(row).to_string());
        if let Some(sequence) = row.stop_sequence() {
            details.insert(format!("{prefix}_stop_sequence"), sequence.to_string());
        }
        if let Some(arrival) = row.arrival_time() {
            details.insert(
                format!("{prefix}_arrival"),
                format_hms(hms_to_secs(arrival)),
            );
        }
        if let Some(departure) = row.departure_time() {
            details.insert(
                format!("{prefix}_departure"),
                format_hms(hms_to_secs(departure)),
            );
        }
    }
}

/// Add the two rows immediately preceding the last stop. These keys are separate from
/// `prev_*`, which keeps its existing meaning: the nearest preceding row that has a time.
fn add_stm016_preceding_rows(
    details: &mut gtfs_core::NoticeDetails,
    idx: &StopTimesIndex<'_>,
    stimes: &[CompactStopTime],
) {
    let rows = [
        (
            "preceding",
            stimes.len().checked_sub(2).and_then(|i| stimes.get(i)),
        ),
        (
            "preceding2",
            stimes.len().checked_sub(3).and_then(|i| stimes.get(i)),
        ),
    ];
    for (prefix, row) in rows {
        let Some(row) = row else { continue };
        details.insert(format!("{prefix}_stop_id"), idx.stop_id_of(row).to_string());
        if let Some(sequence) = row.stop_sequence() {
            details.insert(format!("{prefix}_stop_sequence"), sequence.to_string());
        }
        if let Some(arrival) = row.arrival_time() {
            details.insert(
                format!("{prefix}_arrival"),
                format_hms(hms_to_secs(arrival)),
            );
        }
        if let Some(departure) = row.departure_time() {
            details.insert(
                format!("{prefix}_departure"),
                format_hms(hms_to_secs(departure)),
            );
        }
    }
}

fn check_stoptimes_derived(idx: &StopTimesIndex<'_>, notices: &mut Vec<Notice>, ctr: &mut u32) {
    // STM_015 / STM_016: required time fields for the first/last stop.
    for (&trip_id, stimes) in &idx.by_trip {
        // trip_id enrichment: first-departure-time suffix (" 08:15 departure"); empty when unknown.
        let dep_suffix = stimes
            .first()
            .and_then(|s| s.departure_time())
            .map(|(h, m, _)| format!(" {h:02}:{m:02} kalkışlı"))
            .unwrap_or_default();
        // Flex guard: the spec FORBIDS arrival_time/departure_time when
        // `start_pickup_drop_off_window` or `end_pickup_drop_off_window` is defined
        // ("Forbidden when ... are defined"). A missing time in such a record is expected, not a defect;
        // without the guard, STM_015/016 would produce an FP for every Flex trip.
        let is_flex = |st: &CompactStopTime| {
            idx.k2.flex_of(st).is_some_and(|f| {
                f.start_pickup_drop_off_window.is_some() || f.end_pickup_drop_off_window.is_some()
            })
        };

        // Authority split (2026-07-29): the spec says "Required for the first and last stop in a trip"
        // for `arrival_time` — NORMATIVE. It has NO such clause for `departure_time` (required only
        // at timepoint=1); MD `missing_trip_edge` nevertheless checks both and bases its rationale
        // on the spec's RECOMMENDATION to "enter the same value".
        // Therefore these are two separate rules:
        //   STM_015/016 → arrival_time (Spec, normative requirement)
        //   STM_034     → "only one is defined" (Interop, K2) ALREADY covers departure_time;
        //                 no separate rule is needed.
        // Combining them would let the Spec class bring a non-normative case into the R1 release gate.
        // `arrival_is_absent()` — NOT `arrival_time().is_none()`. The latter also treats a populated
        // but unparsable value as "missing", making this rule speak FALSELY in that case:
        // mdb-2727 reported "arrival_time missing at first/last stop" 101,621 times even though
        // the value was written as HH:MM and STM_003 on the same row already reported the real issue.
        // Because it is Spec·Critical, that false claim entered the R1 release gate.
        if let Some(first) = stimes.first() {
            if !is_flex(first) && first.arrival_is_absent() {
                let mut n = k6_notice(
                    ctr,
                    "STM_015",
                    EntityType::Trip,
                    Some(trip_id.to_string()),
                    Some(trip_id.to_string()),
                    "stop_times.txt",
                    Some(first.line as u64),
                    Some("arrival_time"),
                    None,
                    Some("HH:MM:SS".to_string()),
                    format!("trip_id '{trip_id}' seferinin ilk durağında arrival_time eksik."),
                    "İlk stop_times satırına arrival_time girin.",
                );
                let mut details = edge_time_details(idx, stimes, first, "next");
                if let Some(sequence) = first.stop_sequence() {
                    details.insert("stop_sequence".to_string(), sequence.to_string());
                }
                add_stm015_following_rows(&mut details, idx, stimes);
                n.details = Some(details);
                notices.push(n);
            }
        }

        // (STM_057 REMOVED — 2026-08-01, WP-1.) The rule required two stop_times records for every
        // Flex location. The spec sentence is conditional: "Travel WITHIN THE SAME location group or
        // GeoJSON location requires two records…" — a trip within one location needs two records.
        // A normal point-to-point Flex trip (L1 → L2) uses each location once and is valid; the rule
        // rejected it as Critical·Spec. The only remaining checkable case (a trip with one Flex-location
        // record) is already caught by STM_033 "One-stop trip".

        if let Some(last) = stimes.last() {
            if !is_flex(last) && last.arrival_is_absent() {
                let mut n = k6_notice(
                    ctr, "STM_016", EntityType::Trip,
                    Some(trip_id.to_string()), Some(trip_id.to_string()),
                    "stop_times.txt", Some(last.line as u64),
                    Some("arrival_time"), None, Some("HH:MM:SS".to_string()),
                    format!("trip_id '{trip_id}'{dep_suffix} seferinin son durağında arrival_time eksik."),
                    "Son stop_times satırına arrival_time girin.",
                );
                let mut details = edge_time_details(idx, stimes, last, "prev");
                if let Some(sequence) = last.stop_sequence() {
                    details.insert("stop_sequence".to_string(), sequence.to_string());
                }
                add_stm016_preceding_rows(&mut details, idx, stimes);
                n.details = Some(details);
                notices.push(n);
            }
        }
    }
}

// ── WP-09d: Route + trip kalitesi ────────────────────────────────────────────

fn check_route_trip_quality(
    records: &EntityRecords,
    derived: &DerivedData,
    idx: &StopTimesIndex<'_>,
    notices: &mut Vec<Notice>,
    ctr: &mut u32,
) {
    use crate::timing::Timer;
    let ti_rtq = &records.trip_interns;

    // route_id → trip list.
    let _t0 = Timer::start("K6::rtq::build_trip_maps");
    let mut route_trips: HashMap<&str, Vec<&crate::k2::trips::TripRecord>> = HashMap::new();
    for t in &records.trips {
        if !ti_rtq.route_id(t).is_empty() {
            route_trips.entry(ti_rtq.route_id(t)).or_default().push(t);
        }
    }
    drop(_t0);

    // route_id → display name (route_short_name when present, otherwise route_id).
    let route_short: HashMap<&str, &str> = records
        .routes
        .iter()
        .map(|r| {
            let label = r
                .route_short_name
                .as_deref()
                .filter(|s| !s.is_empty())
                .unwrap_or(r.route_id.as_str());
            (r.route_id.as_str(), label)
        })
        .collect();

    // trip_id → first departure time (HH:MM).
    let trip_first_dep: HashMap<&str, String> = idx
        .by_trip
        .iter()
        .filter_map(|(&tid, sts)| {
            sts.first()
                .and_then(|s| s.departure_time())
                .map(|d| (tid, format!("{:02}:{:02}", d.0, d.1)))
        })
        .collect();

    // TRP_009 (no timed stop in trip) REMOVED (2026-07-23):
    // it was a strict subset of STM_015 (GtfsSpec — departure_time required at the first stop).
    // "No times at all" ⟹ no departure at the first stop ⟹ STM_015 already fires.
    // STM_015 has stronger authority (Spec > Quality) and flags the required-field violation;
    // TRP_009 (ProjectQuality) added no information or authority.

    // TRP_013: too few trips per route (= 1) — frequency issue.
    let _t2 = Timer::start("K6::rtq::trp_013");
    for (route_id, trips) in &route_trips {
        if trips.len() == 1 {
            let mut n013 = k6_notice(
                ctr,
                "TRP_013",
                EntityType::Route,
                Some(route_id.to_string()),
                Some(route_id.to_string()),
                "trips.txt",
                trips.first().map(|t| t.line),
                None,
                Some("1".to_string()),
                None,
                {
                    let rname = route_short.get(*route_id).copied().unwrap_or(route_id);
                    format!("'{rname}' hattında yalnızca 1 sefer var — düşük frekans sinyali.")
                },
                "Bu rotaya ek seferler ekleyin ya da frekans bazlı tanım kullanın.",
            );
            // The single trip's service calendar (if any) — fills R2's "Service Calendar" column.
            if let Some(svc) = trips
                .first()
                .map(|t| ti_rtq.service_id(t))
                .filter(|s| !s.is_empty())
            {
                n013.service_id = Some(svc.to_string());
            }
            notices.push(n013);
        }
    }
    drop(_t2);

    // RTS_016: route → no active service day (empty in calendar_bitmap).
    let _t3 = Timer::start("K6::rtq::RTS_016");
    for (route_id, trips) in &route_trips {
        let has_active = trips.iter().any(|t| {
            derived
                .calendar_bitmap
                .active_dates
                .get(ti_rtq.service_id(t))
                .map(|s| !s.is_empty())
                .unwrap_or(false)
        });
        if !has_active {
            notices.push(k6_notice(
                ctr,
                "RTS_016",
                EntityType::Route,
                Some(route_id.to_string()),
                Some(route_id.to_string()),
                "routes.txt",
                None,
                None,
                None,
                None,
                {
                    let rname = route_short.get(*route_id).copied().unwrap_or(route_id);
                    format!("'{rname}' hattının hiçbir seferinde aktif takvim günü yok.")
                },
                "calendar.txt veya calendar_dates.txt'deki servis tanımlarını düzeltin.",
            ));
        }
    }
    drop(_t3);

    // TRP_011: the passenger receives no identifying information — trip_headsign, trip_short_name,
    // AND route name (route_short/long_name) are absent. If the route is named, the passenger knows
    // the route from it → trip_headsign is optional; do not produce an FP (#29: all TriMet trips
    // lacked headsigns but all routes were named → 52,298 pure FPs). route_id → does any route name exist?
    let route_named: HashMap<&str, bool> = records
        .routes
        .iter()
        .map(|r| {
            (
                r.route_id.as_str(),
                r.route_short_name.as_deref().is_some_and(|s| !s.is_empty())
                    || r.route_long_name.as_deref().is_some_and(|s| !s.is_empty()),
            )
        })
        .collect();
    let _t4 = Timer::start("K6::rtq::trp_011");
    for trip in &records.trips {
        let headsign_missing = ti_rtq.headsign(trip).map(str::is_empty).unwrap_or(true);
        let short_missing = ti_rtq.short_name(trip).map(str::is_empty).unwrap_or(true);
        if headsign_missing && short_missing {
            // If the route is named, the passenger knows the route from it → trip_headsign is optional; skip.
            if route_named
                .get(ti_rtq.route_id(trip))
                .copied()
                .unwrap_or(false)
            {
                continue;
            }
            notices.push(k6_notice(
                ctr,
                "TRP_011",
                EntityType::Trip,
                Some(trip.trip_id.to_string()),
                Some(trip.trip_id.to_string()),
                "trips.txt",
                Some(trip.line),
                Some("trip_headsign"),
                None,
                None,
                {
                    let rid = ti_rtq.route_id(trip);
                    let rname = route_short.get(rid).copied().unwrap_or(rid);
                    let dep = trip_first_dep.get(trip.trip_id.as_str()).map(|s| format!(" {} kalkışlı", s)).unwrap_or_default();
                    format!("'{}' hattının{dep} seferinde yön adı (trip_headsign) ve kısa ad girilmemiş — yolcu bilgisi yok.", rname)
                },
                "trip_headsign veya trip_short_name alanını doldurun.",
            ));
        }
    }
    drop(_t4);

    // TRP_020: trip_headsign matches an intermediate stop name, not the terminal.
    // Fire per matching stop; skip circular trips.
    let _t5 = Timer::start("K6::rtq::trp_020");
    {
        // stop_id → stop_name (lowercase, trimmed).
        let stop_name_lc: HashMap<&str, String> = records
            .stops
            .iter()
            .filter_map(|s| {
                s.stop_name
                    .as_deref()
                    .map(|n| (s.stop_id.as_str(), n.trim().to_lowercase()))
            })
            .collect();

        // stop_id → parent_station (non-empty) — make loop/circular detection parent-aware.
        // A stop's "effective station": parent_station when present, otherwise stop_id.
        let parent_of: HashMap<&str, &str> = records
            .stops
            .iter()
            .filter_map(|s| {
                s.row
                    .get("parent_station")
                    .map(|p| p.trim())
                    .filter(|p| !p.is_empty())
                    .map(|p| (s.stop_id.as_str(), p))
            })
            .collect();
        // Effective station: parent_station when present, otherwise stop_id.
        fn eff_station<'a>(parent_of: &HashMap<&'a str, &'a str>, sid: &'a str) -> &'a str {
            parent_of.get(sid).copied().unwrap_or(sid)
        }

        // #15 (TRP_020 perf): scratch buffers OUTSIDE the trip loop — avoids per-trip Vec/HashMap
        // allocation (both borrow &str from records and are cleared before each use). station_counts
        // is built LAZILY only after a name match; trips without a match (the majority) never build it.
        let mut seq_eff: Vec<(u32, &str)> = Vec::new();
        let mut station_counts: HashMap<&str, u32> = HashMap::new();

        for trip in &records.trips {
            let headsign = match ti_rtq
                .headsign(trip)
                .map(str::trim)
                .filter(|h| !h.is_empty())
            {
                Some(h) => h,
                None => continue,
            };

            let stimes = match idx.by_trip.get(trip.trip_id.as_str()) {
                Some(v) if v.len() >= 2 => v,
                _ => continue,
            };

            // Terminal stop: largest stop_sequence.
            let terminal_stop_id = stimes
                .iter()
                .max_by_key(|s| s.stop_sequence().unwrap_or(0))
                .map(|s| idx.stop_id_of(s))
                .unwrap_or("");

            // First stop: smallest stop_sequence.
            let first_stop_id = stimes
                .iter()
                .min_by_key(|s| s.stop_sequence().unwrap_or(u32::MAX))
                .map(|s| idx.stop_id_of(s))
                .unwrap_or("");

            // Fully circular trip → first and last stops are the same physical station (parent-aware,
            // including loops returning to the same station through different platform stop_ids). Skip it.
            if eff_station(&parent_of, first_stop_id) == eff_station(&parent_of, terminal_stop_id) {
                continue;
            }

            // #15: calculate headsign_lc AFTER early exits (stimes/cyclic) — skipped trips do not
            // pay for the to_lowercase allocation.
            let headsign_lc = headsign.to_lowercase();

            // If the headsign matches the FINAL STOP NAME, the direction name is CORRECT → skip the trip.
            // The rule is defined as "headsign matches an intermediate stop, NOT the final stop";
            // a final-stop match is not misleading.
            //
            // The loop below excludes the terminal by `stop_id` but matches by NAME. DIFFERENT
            // stop_ids with the same name (separate platform/direction records, NO shared parent_station)
            // passed this filter and produced a name match → FP. station_counts suppression did not help:
            // the two records counted as separate "effective stations", so the count was 1.
            //
            // Corpus evidence (mdb-1294, Rome): headsign 'CIMITERO LAURENTINO' appears at both the
            // 34th and FINAL (38th) stop; because it matches the final stop, the headsign is correct.
            // We flagged 9,162 trips; MD flags only 19 (those that truly end at an intermediate stop).
            if stop_name_lc.get(terminal_stop_id) == Some(&headsign_lc) {
                continue;
            }

            // station_counts (number of visits to each effective station in the trip) is built LAZILY
            // at the first name match (counts_built). If the matching stop appears multiple times, it
            // is a legitimate terminal/turnback point (e.g. an airport wye) → suppress that match.
            // An unrelated stop repeat does not suppress another match. #29: CONSECUTIVE duplicate
            // rows for the same stop (dwell/timepoint duplication) count once; a real terminal/turnback
            // repeats with ANOTHER stop between occurrences. seq_eff is sorted by stop_sequence
            // (unwrap_or(0)) — because the by_trip slice is sorted with unwrap_or(u32::MAX), it can
            // DIFFER in a feed with missing sequences; therefore preserve this ordering (do not remove it).
            let mut counts_built = false;

            // One notice per matching intermediate stop (stop at the first match).
            for st in stimes
                .iter()
                .filter(|s| idx.stop_id_of(s) != terminal_stop_id)
            {
                let Some(stop_name) = stop_name_lc.get(idx.stop_id_of(st)) else {
                    continue;
                };
                if *stop_name != headsign_lc {
                    continue;
                }

                if !counts_built {
                    seq_eff.clear();
                    seq_eff.extend(stimes.iter().map(|s| {
                        (
                            s.stop_sequence().unwrap_or(0),
                            eff_station(&parent_of, idx.stop_id_of(s)),
                        )
                    }));
                    seq_eff.sort_by_key(|(seq, _)| *seq);
                    station_counts.clear();
                    let mut prev_eff: Option<&str> = None;
                    for (_, e) in &seq_eff {
                        if prev_eff != Some(*e) {
                            *station_counts.entry(*e).or_insert(0) += 1;
                            prev_eff = Some(*e);
                        }
                    }
                    counts_built = true;
                }

                // The matching stop is a terminal/turnback point (repeated in the trip) → skip.
                if station_counts
                    .get(eff_station(&parent_of, idx.stop_id_of(st)))
                    .copied()
                    .unwrap_or(0)
                    > 1
                {
                    continue;
                }

                // #15: compute rname/dep (format!) only at emission — not for trips without a match.
                let rname = route_short
                    .get(ti_rtq.route_id(trip))
                    .copied()
                    .unwrap_or(ti_rtq.route_id(trip));
                let dep = trip_first_dep
                    .get(trip.trip_id.as_str())
                    .map(|s| format!(" {} kalkışlı", s))
                    .unwrap_or_default();
                notices.push(k6_notice(
                    ctr,
                    "TRP_020",
                    EntityType::Trip,
                    Some(trip.trip_id.to_string()),
                    Some(trip.trip_id.to_string()),
                    "trips.txt",
                    Some(trip.line),
                    Some("trip_headsign"),
                    Some(headsign.to_string()),
                    None,
                    format!(
                        "'{}' hattının{dep} seferinde yön adı '{}' terminal durak değil, ara durak adıyla eşleşiyor.",
                        rname, headsign
                    ),
                    "Headsign bilinçli bir ana hedefi gösteriyorsa işlem gerekmez; değilse yön adını doğrulayın.",
                ));
                // #15: TRP_020 is Entity-scoped (trip_id) → dedup reduces it to one per trip.
                // Emitting for EVERY matching intermediate stop (on a large feed) created and then
                // discarded hundreds of thousands of intermediate notices; stop at the first match (same result).
                break;
            }
        }
    }
    drop(_t5);

    // ── RTS_020: route URL is the same as the agency URL (same_route_and_agency_url) ─
    {
        let _t6 = Timer::start("K6::rtq::rts_020");
        // agency_id → agency_url
        let agency_url_map: HashMap<&str, &str> = records
            .agencies
            .iter()
            .filter_map(|a| {
                let aid = a.agency_id.as_deref().unwrap_or("");
                if a.agency_url.is_empty() {
                    None
                } else {
                    Some((aid, a.agency_url.as_str()))
                }
            })
            .collect();
        // If there is one agency and agency_id is missing, use that agency's URL.
        let default_agency_url = if records.agencies.len() == 1 {
            Some(records.agencies[0].agency_url.as_str())
        } else {
            None
        };

        for route in &records.routes {
            if route.route_id.is_empty() {
                continue;
            }
            let route_url = match route.route_url.as_deref().filter(|u| !u.is_empty()) {
                Some(u) => u,
                None => continue,
            };
            let agency_url = route
                .agency_id
                .as_deref()
                .and_then(|aid| agency_url_map.get(aid).copied())
                .or(default_agency_url);
            if let Some(aurl) = agency_url {
                if route_url == aurl {
                    let rname = route_short
                        .get(route.route_id.as_str())
                        .copied()
                        .unwrap_or(route.route_id.as_str());
                    notices.push(k6_notice(
                        ctr,
                        "RTS_020",
                        EntityType::Route,
                        Some(route.route_id.clone()),
                        Some(route.route_id.clone()),
                        "routes.txt",
                        Some(route.line),
                        Some("route_url"),
                        Some(route_url.to_string()),
                        None,
                        format!(
                            "'{}' hattının route_url değeri acente URL'siyle aynı: '{route_url}'.",
                            rname
                        ),
                        "route_url'yi bu hata özgü bir sayfaya yönlendirin ya da boş bırakın.",
                    ));
                }
            }
        }
    }

    // ── RTS_022: long route name contains the short name (route_long_name_contains_short_name) ─
    {
        let _t7 = Timer::start("K6::rtq::rts_022");
        for route in &records.routes {
            if route.route_id.is_empty() {
                continue;
            }
            let short = match route.route_short_name.as_deref().filter(|s| !s.is_empty()) {
                Some(s) => s,
                None => continue,
            };
            let long = match route.route_long_name.as_deref().filter(|l| !l.is_empty()) {
                Some(l) => l,
                None => continue,
            };
            // Exact equality is a real repetition case even for one-character names.
            // For non-equal containment, protect one-character codes:
            // short="5A", long="5A Line" → fires;
            // short="5", long="Route 5A" → no word boundary, does not fire.
            let names_equal = short.to_lowercase() == long.to_lowercase();
            if names_equal || (short.chars().count() >= 2 && contains_as_word(long, short)) {
                notices.push(k6_notice(
                    ctr,
                    "RTS_022",
                    EntityType::Route,
                    Some(route.route_id.clone()),
                    Some(route.route_id.clone()),
                    "routes.txt",
                    Some(route.line),
                    Some("route_long_name"),
                    Some(long.to_string()),
                    None,
                    format!(
                        "'{}' hattının uzun adı '{}', kısa adı '{}' zaten içeriyor.",
                        route.route_id, long, short
                    ),
                    "route_long_name'i kısa adı tekrar etmeyecek şekilde düzenleyin.",
                ));
            }
        }
    }

    // ── STP_034/035: stop_url is the same as the agency or route URL ─────────
    // Compare URLs with safe syntactic equivalence and collect stops using the same
    // normalized URL into one aggregate notice.
    {
        let agency_urls: BTreeSet<NormalizedWebUrl> = records
            .agencies
            .iter()
            .filter_map(|a| normalize_web_url(&a.agency_url))
            .collect();
        let route_urls: BTreeSet<NormalizedWebUrl> = records
            .routes
            .iter()
            .filter_map(|r| r.route_url.as_deref().and_then(normalize_web_url))
            .collect();
        let mut agency_matches: BTreeMap<NormalizedWebUrl, Vec<usize>> = BTreeMap::new();
        let mut route_matches: BTreeMap<NormalizedWebUrl, Vec<usize>> = BTreeMap::new();

        for (stop_idx, stop) in records.stops.iter().enumerate() {
            let Some(surl) = stop.stop_url.as_deref().filter(|u| !u.is_empty()) else {
                continue;
            };
            let Some(key) = normalize_web_url(surl) else {
                continue;
            };
            if agency_urls.contains(&key) {
                agency_matches
                    .entry(key.clone())
                    .or_default()
                    .push(stop_idx);
            }
            if route_urls.contains(&key) {
                route_matches.entry(key).or_default().push(stop_idx);
            }
        }

        let emit_url_matches = |rule_id: &str,
                                matches: &mut BTreeMap<NormalizedWebUrl, Vec<usize>>,
                                message_name: &str,
                                ctr: &mut u32,
                                notices: &mut Vec<Notice>| {
            for (normalized, stop_indices) in matches.iter_mut() {
                stop_indices
                    .sort_by(|a, b| records.stops[*a].stop_id.cmp(&records.stops[*b].stop_id));
                let representative = &records.stops[stop_indices[0]];
                let stop_ids: Vec<&str> = stop_indices
                    .iter()
                    .take(5)
                    .map(|idx| records.stops[*idx].stop_id.as_str())
                    .collect();
                let raw_url = representative.stop_url.as_deref().unwrap_or_default();
                let mut details: BTreeMap<String, String> = BTreeMap::new();
                details.insert(
                    "normalized_url".to_string(),
                    format_normalized_web_url(normalized),
                );
                details.insert("stop_count".to_string(), stop_indices.len().to_string());
                details.insert("representative_stop_ids".to_string(), stop_ids.join(","));
                if stop_indices.len() > 5 {
                    details.insert(
                        "additional_stop_count".to_string(),
                        (stop_indices.len() - 5).to_string(),
                    );
                }

                let mut notice = k6_notice(
                    ctr, rule_id, EntityType::Stop,
                    Some(representative.stop_id.clone()), Some(representative.stop_id.clone()),
                    "stops.txt", Some(representative.line), Some("stop_url"),
                    Some(raw_url.to_string()), None,
                    format!("'{}' durağının stop_url değeri bir {message_name} URL'siyle aynı; {count} durak aynı URL'yi kullanıyor: '{raw_url}'.", representative.stop_id, count = stop_indices.len()),
                    "stop_url'yi bu durağa özgü bir sayfaya yönlendirin ya da boş bırakın.",
                );
                notice.details = Some(details.into());
                notices.push(notice);
            }
        };

        emit_url_matches("STP_034", &mut agency_matches, "acente", ctr, notices);
        emit_url_matches("STP_035", &mut route_matches, "hat", ctr, notices);
    }

    // ── PDW_006: overlapping pickup/drop-off windows in the same trip+zone ───
    {
        // Spec (stop_times.txt): what is forbidden is the overlap of geometry + time window +
        // `pickup_type` or `drop_off_type` together in the SAME trip. A pickup-only row and a
        // drop-off-only row sharing the same window in the same zone is exactly the spec's
        // "on-demand service within one zone" example (Goshogawara Flex draft, 2026-09-19)
        // — no finding when the roles do not overlap.
        // (trip_id, zone_key) → [(start_secs, end_secs, line, pickup_open, drop_off_open)]
        type ZoneWindows<'a> = HashMap<(&'a str, &'a str), Vec<(u64, u64, u64, bool, bool)>>;
        let mut zone_wins: ZoneWindows<'_> = HashMap::new();
        for (trip_id, stops) in records.stop_times_index.iter_trips() {
            for st in stops {
                let Some(flex) = records.stop_times_index.flex_of(st) else {
                    continue;
                };
                let Some(start) = flex.start_pickup_drop_off_window else {
                    continue;
                };
                let Some(end) = flex.end_pickup_drop_off_window else {
                    continue;
                };
                let zone = if let Some(ref z) = flex.location_id {
                    z.as_str()
                } else if let Some(ref z) = flex.location_group_id {
                    z.as_str()
                } else {
                    continue;
                };
                let s = start.0 as u64 * 3600 + start.1 as u64 * 60 + start.2 as u64;
                let e = end.0 as u64 * 3600 + end.1 as u64 * 60 + end.2 as u64;
                zone_wins
                    .entry((trip_id.as_str(), zone))
                    .or_default()
                    .push((
                        s,
                        e,
                        st.line as u64,
                        flex.pickup_type != Some(1),
                        flex.drop_off_type != Some(1),
                    ));
            }
        }

        for ((trip_id, zone), mut wins) in zone_wins {
            if wins.len() < 2 {
                continue;
            }
            wins.sort_by_key(|&(s, _, _, _, _)| s);
            // There are few rows per trip+zone; because the role condition defeats adjacent-pair
            // scanning, compare pairs directly.
            let overlap_line = (1..wins.len()).find_map(|i| {
                let (cs, _ce, cl, c_pick, c_drop) = wins[i];
                wins[..i]
                    .iter()
                    .any(|&(_ps, pe, _, p_pick, p_drop)| {
                        cs < pe && ((c_pick && p_pick) || (c_drop && p_drop))
                    })
                    .then_some(cl)
            });
            if let Some(cl) = overlap_line {
                notices.push(k6_notice(
                    ctr, "PDW_006", EntityType::Trip,
                    Some(trip_id.to_string()), Some(trip_id.to_string()),
                    "stop_times.txt", Some(cl), Some("start_pickup_drop_off_window"),
                    None, None,
                    format!("trip_id '{}' için zone '{}' içinde aynı biniş/iniş rolüyle örtüşen pickup/drop-off pencereleri var.", trip_id, zone),
                    "Aynı trip+zone içinde biniş (veya iniş) açık satırların zaman pencerelerinin örtüşmediğinden emin olun.",
                ));
            }
        }
    }
}

// ── WP-09d: Data-quality summary rules ────────────────────────────────────────

fn mark_skipped_check(skipped_checks: &mut Vec<String>, name: &str) {
    if !skipped_checks.iter().any(|existing| existing == name) {
        skipped_checks.push(name.to_string());
    }
}

fn record_data_quality_skips(
    availability: &FileAvailability<'_>,
    config: &ValidatorConfig,
    today_yyyymmdd: u32,
    skipped_checks: &mut Vec<String>,
) {
    let agency_usable = availability.present_and_available("agency.txt");
    let routes_usable = availability.present_and_available("routes.txt");
    let trips_usable = availability.present_and_available("trips.txt");
    let stops_usable = availability.present_and_available("stops.txt");
    let stop_times_usable = availability.present_and_available("stop_times.txt");
    let feed_info_usable = availability.present_and_available("feed_info.txt");
    let calendar_source_usable =
        availability.any_present_and_available(&["calendar.txt", "calendar_dates.txt"]);

    if config.stop_name_best_practices && !stops_usable {
        mark_skipped_check(skipped_checks, "K6::STP_040");
        mark_skipped_check(skipped_checks, "K6::STP_041");
    }
    if !(trips_usable && calendar_source_usable) {
        mark_skipped_check(skipped_checks, "K6::DQ_005");
    }
    if !trips_usable {
        mark_skipped_check(skipped_checks, "K6::DQ_006");
        mark_skipped_check(skipped_checks, "K6::DQ_013");
    }
    if !(trips_usable && stop_times_usable) {
        mark_skipped_check(skipped_checks, "K6::DQ_009");
    }
    if !stops_usable {
        mark_skipped_check(skipped_checks, "K6::DQ_011");
        mark_skipped_check(skipped_checks, "K6::DQ_017");
        mark_skipped_check(skipped_checks, "K6::DQ_022");
    }
    if !(agency_usable && routes_usable) {
        mark_skipped_check(skipped_checks, "K6::DQ_012");
        mark_skipped_check(skipped_checks, "K6::RTS_025");
    }
    if !routes_usable {
        mark_skipped_check(skipped_checks, "K6::DQ_003");
        mark_skipped_check(skipped_checks, "K6::DQ_004");
    }
    if today_yyyymmdd > 0 && !feed_info_usable {
        for rule_id in ["FIN_010", "FIN_016", "FIN_017", "FIN_018", "FIN_019"] {
            mark_skipped_check(skipped_checks, &format!("K6::{rule_id}"));
        }
    }
    if !feed_info_usable {
        mark_skipped_check(skipped_checks, "K6::FIN_020");
        mark_skipped_check(skipped_checks, "K6::CAL_020");
    }

    for file in [
        "stops.txt",
        "routes.txt",
        "trips.txt",
        "attributions.txt",
        "route_networks.txt",
        "fare_leg_rules.txt",
        "fare_leg_join_rules.txt",
        "fare_transfer_rules.txt",
        "location_groups.txt",
        "location_group_stops.txt",
        "stop_areas.txt",
        "fare_rules.txt",
        "calendar_dates.txt",
    ] {
        if !availability.present_and_available(file) {
            mark_skipped_check(skipped_checks, &format!("K6::DQ_021::{file}"));
        }
    }

    // ARC_020 still reports a genuinely missing optional file. A present but unreadable
    // file is neither safely verifiable nor equivalent to a missing recommendation.
    if availability.has_inventory() {
        for file in ["shapes.txt", "feed_info.txt"] {
            if availability.present(file) && !availability.available(file) {
                mark_skipped_check(skipped_checks, &format!("K6::ARC_020::{file}"));
            }
        }
    }
}

fn check_data_quality(
    records: &EntityRecords,
    derived: &DerivedData,
    config: &ValidatorConfig,
    today_yyyymmdd: u32,
    availability: &FileAvailability<'_>,
    notices: &mut Vec<Notice>,
    ctr: &mut u32,
) {
    let future_only_jp = gtfs_jp_future_only_date(records, derived, today_yyyymmdd).is_some();
    let agency_usable = availability.present_and_available("agency.txt");
    let routes_usable = availability.present_and_available("routes.txt");
    let trips_usable = availability.present_and_available("trips.txt");
    let stops_usable = availability.present_and_available("stops.txt");
    let stop_times_usable = availability.present_and_available("stop_times.txt");
    let feed_info_usable = availability.present_and_available("feed_info.txt");
    let calendar_source_usable =
        availability.any_present_and_available(&["calendar.txt", "calendar_dates.txt"]);

    if let Some(source_url) = config.source_url.as_deref() {
        let path = source_url.split(['?', '#']).next().unwrap_or(source_url);
        if !path.to_ascii_lowercase().ends_with(".zip") {
            notices.push(k6_notice(
                ctr,
                "ARC_028",
                EntityType::Feed,
                None,
                None,
                "source_url",
                None,
                Some("source_url"),
                Some(source_url.to_string()),
                Some("kalıcı URL .../dosya.zip".to_string()),
                format!("GTFS yayın URL'si bir .zip dosya adıyla bitmiyor: '{source_url}'."),
                "Feed'i kalıcı ve açık bir .zip dosya adı içeren URL'de yayımlayın.",
            ));
        }
    }
    if config.stop_name_best_practices && stops_usable {
        let stop_by_id: HashMap<&str, &crate::k2::stops::StopRecord> = records
            .stops
            .iter()
            .map(|s| (s.stop_id.as_str(), s))
            .collect();
        for stop in &records.stops {
            let Some(name) = stop.stop_name.as_deref() else {
                continue;
            };
            let lower = name.to_lowercase();
            let generic = lower
                .split(|c: char| !c.is_alphanumeric())
                .any(|w| w == "stop" || w == "station");
            let accepted = matches!(lower.as_str(), "union station" | "central station");
            if generic && !accepted {
                notices.push(k6_notice(ctr, "STP_040", EntityType::Stop, Some(stop.stop_id.clone()),
                    Some(stop.stop_id.clone()), "stops.txt", Some(stop.line), Some("stop_name"),
                    Some(name.to_string()), None,
                    format!("stop_id '{}' adı gereksiz genel 'stop/station' sözcüğü içeriyor: '{}'.", stop.stop_id, name),
                    "Genel sözcüğü kaldırın; sözcük resmi adın parçasıysa bu opt-in profili kapatın."));
            }
            if let Some(parent_id) = stop
                .row
                .get("parent_station")
                .filter(|v| !v.trim().is_empty())
            {
                if let Some(parent_name) = stop_by_id
                    .get(parent_id.trim())
                    .and_then(|p| p.stop_name.as_deref())
                {
                    if !lower.contains(&parent_name.to_lowercase()) {
                        notices.push(k6_notice(ctr, "STP_041", EntityType::Stop, Some(stop.stop_id.clone()),
                            Some(stop.stop_id.clone()), "stops.txt", Some(stop.line), Some("stop_name"),
                            Some(name.to_string()), Some(format!("{} içermeli", parent_name)),
                            format!("Alt durak '{}' adı üst istasyon '{}' adını içermiyor.", name, parent_name),
                            "Alt durak adında üst istasyon adını kullanın; yerel adlandırma farklıysa profili kapatın."));
                    }
                }
            }
        }
    }
    // DQ_005: no active trips in the feed (calendar_bitmap is empty or all services are expired).
    let has_any_active = if today_yyyymmdd > 0 {
        derived
            .calendar_bitmap
            .active_dates
            .values()
            .any(|dates| dates.iter().any(|&d| d >= today_yyyymmdd))
    } else {
        !derived.calendar_bitmap.active_dates.is_empty()
    };

    if trips_usable && calendar_source_usable && !records.trips.is_empty() && !has_any_active {
        notices.push(k6_notice(
            ctr,
            "DQ_005",
            EntityType::Feed,
            None,
            None,
            "calendar.txt",
            None,
            None,
            None,
            None,
            "Feed'de bugün itibarıyla aktif seferi olan servis bulunamadı.".to_string(),
            "calendar.txt veya calendar_dates.txt verilerini güncelleyin.",
        ));
    }

    // DQ_006: ratio of trips without a shape is too high (>80%).
    // Exclude trips with only Flex zone/group rows from the denominator: they have no fixed route
    // (the Goshogawara on-demand taxi feed produced a HIGH 100% "no shape" finding).
    let zone_only = zone_only_trip_ids(records);
    let fixed_trips = records.trips.len() - records
        .trips
        .iter()
        .filter(|t| zone_only.contains(t.trip_id.as_str()))
        .count();
    if trips_usable && fixed_trips > 0 {
        let shapeless = records
            .trips
            .iter()
            .filter(|t| t.shape_idx == 0 && !zone_only.contains(t.trip_id.as_str()))
            .count();
        let ratio = shapeless as f64 / fixed_trips as f64;
        if ratio > 0.8 {
            notices.push(k6_notice(
                ctr,
                "DQ_006",
                EntityType::Feed,
                None,
                None,
                "trips.txt",
                None,
                None,
                Some(format!("{:.0}%", ratio * 100.0)),
                Some("≤ %80".to_string()),
                format!("Seferlerin %{:.0}'ında shape_id eksik — harita gösterimi ve coğrafi analiz kısıtlı.",
                    ratio * 100.0),
                "shapes.txt dosyasını oluşturun ve trips.txt'deki shape_id alanlarını doldurun.",
            ));
        }
    }

    // DQ_009: no stop_times in the feed.
    if trips_usable
        && stop_times_usable
        && records.stop_times_index.total_rows == 0
        && !records.trips.is_empty()
    {
        notices.push(k6_notice(
            ctr,
            "DQ_009",
            EntityType::Feed,
            None,
            None,
            "stop_times.txt",
            None,
            None,
            Some("0".to_string()),
            None,
            "Feed'de sefer var ancak hiç stop_times kaydı yok.".to_string(),
            "stop_times.txt dosyasını oluşturun ve doldurun.",
        ));
    }

    // DQ_011: too few stops in the feed (< 2) — not functional transit data.
    if stops_usable && records.stops.len() == 1 {
        notices.push(k6_notice(
            ctr,
            "DQ_011",
            EntityType::Feed,
            None,
            None,
            "stops.txt",
            None,
            None,
            Some("1".to_string()),
            Some("≥ 2".to_string()),
            "Feed'de yalnızca 1 durak var — işlevsel transit verisi oluşturulamaz.".to_string(),
            "stops.txt'e en az 2 durak ekleyin.",
        ));
    }

    // DQ_012: too many agencies in the feed and agency_id is not used.
    if agency_usable && routes_usable && records.agencies.len() > 5 {
        let routes_with_agency = records
            .routes
            .iter()
            .filter(|r| r.agency_id.is_some())
            .count();
        if routes_with_agency == 0 {
            notices.push(k6_notice(
                ctr,
                "DQ_012",
                EntityType::Feed,
                None,
                None,
                "agency.txt",
                None,
                None,
                None,
                None,
                format!(
                    "Feed'de {} işletici var ancak hiçbir rotada agency_id atanmamış.",
                    records.agencies.len()
                ),
                "routes.txt'deki agency_id sütununu doldurun.",
            ));
        }
    }

    // RTS_025: agency_id is empty in routes.txt — a recommended field (best practice). Info level
    // with one/zero agencies; agency_id is required with >1 agency (DQ_012 / cross-ref scope).
    // MD equivalent of missing_recommended_field. Emit a SEPARATE notice for each empty route.
    if agency_usable && routes_usable && records.agencies.len() <= 1 {
        for r in &records.routes {
            if r.agency_id.as_deref().is_none_or(|s| s.trim().is_empty()) {
                let label = r
                    .route_short_name
                    .as_deref()
                    .filter(|s| !s.is_empty())
                    .unwrap_or(r.route_id.as_str());
                notices.push(k6_notice(
                    ctr, "RTS_025", EntityType::Route,
                    Some(r.route_id.to_string()), Some(r.route_id.to_string()),
                    "routes.txt", Some(r.line), Some("agency_id"),
                    None, None,
                    format!("'{label}' hattında agency_id boş — önerilen alan; tek işletici olsa bile doldurulması iyi uygulamadır."),
                    "routes.txt'teki agency_id sütununu işleten ajansın agency_id'siyle doldurun.",
                ));
            }
        }
    }

    // DQ_013: too few trips in the feed (< 3).
    if trips_usable {
        let trip_count = records.trips.len();
        if trip_count > 0 && trip_count < 3 {
            notices.push(k6_notice(
                ctr, "DQ_013", EntityType::Feed,
                None, None, "trips.txt", None, None,
                Some(format!("{trip_count}")), Some("≥ 3".to_string()),
                format!("Feed'de yalnızca {trip_count} sefer var — işlevsel transit veri için en az 3 sefer önerilir."),
                "trips.txt'e daha fazla sefer ekleyin.",
            ));
        }
    }

    // feed_has_content: whether the feed contains real transit content (for ARC_020).
    // DQ_001/002 (missing feed_publisher_name/url) removed: ARC_020 + FIN_001/002
    // + ARC_025 already cover them and were causing double/triple counting.
    let feed_has_content = !records.agencies.is_empty()
        || !records.routes.is_empty()
        || !records.stops.is_empty()
        || !records.trips.is_empty();

    // DQ_003: route description (route_desc) is empty — one notice per route.
    if routes_usable {
        for route in &records.routes {
            if route.route_id.is_empty() {
                continue;
            }
            if route
                .row
                .get("route_desc")
                .map(|v| v.trim().is_empty())
                .unwrap_or(true)
            {
                let label = route
                    .route_short_name
                    .as_deref()
                    .filter(|s| !s.is_empty())
                    .or(route.route_long_name.as_deref().filter(|s| !s.is_empty()))
                    .unwrap_or(&route.route_id);
                notices.push(k6_notice(
                    ctr, "DQ_003", EntityType::Route,
                    Some(route.route_id.clone()), Some(route.route_id.clone()),
                    "routes.txt", Some(route.line), Some("route_desc"),
                    Some(route.route_id.clone()), None,
                    format!("'{}' ({}) hattında route_desc alanı boş; kullanıcılar hat hakkında ek bilgi alamıyor.", route.route_id, label),
                    "routes.txt'e route_desc açıklaması ekleyin.",
                ));
            }
        }
    }

    // DQ_004: route URL (route_url) is missing — one notice per route.
    if routes_usable {
        for route in &records.routes {
            if route.route_id.is_empty() {
                continue;
            }
            if route
                .row
                .get("route_url")
                .map(|v| v.trim().is_empty())
                .unwrap_or(true)
            {
                let label = route
                    .route_short_name
                    .as_deref()
                    .filter(|s| !s.is_empty())
                    .or(route.route_long_name.as_deref().filter(|s| !s.is_empty()))
                    .unwrap_or(&route.route_id);
                notices.push(k6_notice(
                    ctr, "DQ_004", EntityType::Route,
                    Some(route.route_id.clone()), Some(route.route_id.clone()),
                    "routes.txt", Some(route.line), Some("route_url"),
                    Some(route.route_id.clone()), None,
                    format!("'{}' ({}) hattında route_url alanı boş; yolcular hat web sayfasına yönlendirilemiyor.", route.route_id, label),
                    "routes.txt'e route_url bağlantısı ekleyin.",
                ));
            }
        }
    }

    // DQ_017: suspicious coordinates such as (0.0, 0.0) or the middle of the ocean.
    if stops_usable {
        let suspicious = records
            .stops
            .iter()
            .filter(|s| !s.stop_id.is_empty())
            .filter(|s| {
                matches!((s.stop_lat, s.stop_lon), (Some(lat), Some(lon))
                    if lat.abs() < 1.0 && lon.abs() < 1.0)
            })
            .count();
        if suspicious > 0 {
            notices.push(k6_notice(
                ctr, "DQ_017", EntityType::Feed,
                None, None, "stops.txt", None, Some("stop_lat/stop_lon"),
                Some(format!("{suspicious}")), None,
                format!("{suspicious} durağın koordinatı (0°, 0°) civarında — bu Afrika kıyısı açıklarıdır; muhtemelen yanlış değer."),
                "Durak koordinatlarını gerçek konuma göre düzeltin.",
            ));
        }
    }

    // FIN_010: feed validity period has expired.
    if feed_info_usable && today_yyyymmdd > 0 {
        if let Some(fi) = records.feed_info.first() {
            if let Some((y, m, d)) = fi.feed_end_date {
                let end = y * 10000 + m * 100 + d;
                if end < today_yyyymmdd {
                    notices.push(k6_notice(
                        ctr, "FIN_010", EntityType::Feed,
                        None, None, "feed_info.txt", None, Some("feed_end_date"),
                        Some(format!("{y}-{m:02}-{d:02}")),
                        Some("≥ today".to_string()),
                        format!("Feed'in geçerlilik süresi {y}-{m:02}-{d:02} tarihinde dolmuş; mevcut veriler güncel değil."),
                        "feed_info.txt'deki feed_end_date'i güncel tutun ve yeni bir feed yayınlayın.",
                    ));
                }
            }

            // FIN_016: feed_start_date is in the future — feed is not active yet.
            // In GTFS-JP, publishing the upcoming period in advance as a separate dataset is expected;
            // when the calendar is also entirely future, report only CAL_015's zero-penalty information
            // (same exception as CAL_017/CAL_024/TRP_023).
            if let Some((sy, sm, sd)) = fi.feed_start_date {
                let start = sy * 10000 + sm * 100 + sd;
                if start > today_yyyymmdd && !future_only_jp {
                    notices.push(k6_notice(
                        ctr, "FIN_016", EntityType::Feed,
                        None, None, "feed_info.txt", None, Some("feed_start_date"),
                        Some(format!("{sy}-{sm:02}-{sd:02}")),
                        Some("≤ today".to_string()),
                        format!("feed_start_date ({sy}-{sm:02}-{sd:02}) henüz gelmedi — feed bugün için aktif değil."),
                        "feed_start_date'i geçmiş veya bugüne ayarlayın ya da feed'i doğru zamanda yayınlayın.",
                    ));
                }
            }

            // FIN_017: feed_end_date is far in the future (> 2 years).
            if let Some((ey, em, ed)) = fi.feed_end_date {
                let end = ey * 10000 + em * 100 + ed;
                let today_jdn = yyyymmdd_to_jdn(today_yyyymmdd);
                let end_jdn = yyyymmdd_to_jdn(end);
                if end_jdn > today_jdn + 730 {
                    notices.push(k6_notice(
                        ctr, "FIN_017", EntityType::Feed,
                        None, None, "feed_info.txt", None, Some("feed_end_date"),
                        Some(format!("{ey}-{em:02}-{ed:02}")),
                        Some("≤ +2 years".to_string()),
                        format!("feed_end_date ({ey}-{em:02}-{ed:02}) bugünden 2 yıldan fazla ileride — muhtemelen yanlış değer."),
                        "feed_end_date'i gerçekçi bir bitiş tarihiyle güncelleyin.",
                    ));
                }
            }

            // FIN_018: both feed_contact_email and feed_contact_url are missing
            // (missing_feed_contact_email_and_url).
            let has_contact_email = fi
                .feed_contact_email
                .as_deref()
                .map(|e| !e.trim().is_empty())
                .unwrap_or(false);
            let has_contact_url = fi
                .feed_contact_url
                .as_deref()
                .map(|u| !u.trim().is_empty())
                .unwrap_or(false);
            if !has_contact_email && !has_contact_url {
                notices.push(k6_notice(
                    ctr, "FIN_018", EntityType::Feed,
                    None, None, "feed_info.txt", None, Some("feed_contact_email"),
                    None, None,
                    "feed_info.txt'te teknik iletişim bilgisi yok (feed_contact_email veya feed_contact_url).".to_string(),
                    "Öneri: feed_info.txt'e feed_contact_email veya feed_contact_url ekleyin. Bu alanlar GTFS Spec'te isteğe bağlıdır.",
                ));
            }

            // FIN_019: configurable near-expiry window for feed_info.
            // Deliberately separate from CAL_008's service-calendar threshold.
            if today_yyyymmdd > 0 {
                if let Some((ey, em, ed)) = fi.feed_end_date {
                    let end = ey * 10000 + em * 100 + ed;
                    if end >= today_yyyymmdd {
                        let today_jdn = yyyymmdd_to_jdn(today_yyyymmdd);
                        let end_jdn = yyyymmdd_to_jdn(end);
                        let days_left = end_jdn.saturating_sub(today_jdn);
                        let warning_days = config.feed_info_expiry_warning_days;
                        // `days_left == 0` is INCLUDED (#146). Previously the `days_left > 0`
                        // condition meant a feed with `feed_end_date == today` matched no rule:
                        // FIN_010 requires `end < today`, while this branch excluded zero.
                        // Two real corpus cases (mdb-923, tld-366). "Expires today" is EXPIRING,
                        // not EXPIRED — the feed is valid today, not tomorrow — so it belongs here.
                        // The `end >= today_yyyymmdd` outer condition already excludes negative days_left;
                        // the two rules remain disjoint.
                        if days_left <= warning_days {
                            let mut notice = k6_notice(
                                ctr, "FIN_019", EntityType::Feed,
                                None, None, "feed_info.txt", None, Some("feed_end_date"),
                                Some(format!("{ey}-{em:02}-{ed:02}")),
                                Some(format!("> +{warning_days}")),
                                format!("Feed'in geçerlilik süresi {ey}-{em:02}-{ed:02} tarihinde doluyor — {days_left} gün kaldı (uyarı eşiği: {warning_days} gün)."),
                                "Yeni bir feed versiyonu yayınlamaya hazırlanın.",
                            );
                            notice.details = Some(
                                [
                                    ("days_left".to_string(), days_left.to_string()),
                                    ("warning_days".to_string(), warning_days.to_string()),
                                ]
                                .into_iter()
                                .collect(),
                            );
                            notices.push(notice);
                        }
                    }
                }
            }
        }
    }

    // FIN_020: feed validity window < 7 days.
    if feed_info_usable {
        if let Some(fi) = records.feed_info.first() {
            if let (Some((sy, sm, sd)), Some((ey, em, ed))) = (fi.feed_start_date, fi.feed_end_date)
            {
                let start_jdn = yyyymmdd_to_jdn(sy * 10000 + sm * 100 + sd);
                let end_jdn = yyyymmdd_to_jdn(ey * 10000 + em * 100 + ed);
                let span_days = end_jdn.saturating_sub(start_jdn);
                if span_days < 7 {
                    notices.push(k6_notice(
                        ctr, "FIN_020", EntityType::Feed,
                        None, None, "feed_info.txt", None, None,
                        Some(format!("{span_days}")), Some("≥7".to_string()),
                        format!("Feed geçerlilik penceresi yalnızca {span_days} gün ({sy}-{sm:02}-{sd:02} → {ey}-{em:02}-{ed:02}) — operasyonel kullanım için çok kısa."),
                        "feed_start_date ve feed_end_date değerlerini gerçek hizmet dönemiyle güncelleyin.",
                    ));
                }
            }
        }
    }

    // CAL_020: feed validity window > 5 years (about 1,825 days).
    if feed_info_usable {
        if let Some(fi) = records.feed_info.first() {
            if let (Some((sy, sm, sd)), Some((ey, em, ed))) = (fi.feed_start_date, fi.feed_end_date)
            {
                let start_jdn = yyyymmdd_to_jdn(sy * 10000 + sm * 100 + sd);
                let end_jdn = yyyymmdd_to_jdn(ey * 10000 + em * 100 + ed);
                let span_days = end_jdn.saturating_sub(start_jdn);
                if span_days > 1825 {
                    notices.push(k6_notice(
                        ctr, "CAL_020", EntityType::Feed,
                        None, None, "feed_info.txt", None, None,
                        Some(format!("{} years", span_days / 365)), Some("≤5 years".to_string()),
                        format!("Feed geçerlilik penceresi {} gün (~{} yıl) — gerçekçi olmayan zaman dilimi.",
                            span_days, span_days / 365),
                        "feed_start_date ve feed_end_date değerlerini gerçekçi hizmet dönemine göre düzenleyin.",
                    ));
                }
            }
        }
    }

    // DQ_022: >80% of stops share the same stop_name (placeholder/test data).
    if stops_usable {
        let total_named = records
            .stops
            .iter()
            .filter(|s| s.stop_name.is_some())
            .count();
        if total_named >= 5 {
            let mut name_counts: HashMap<&str, u32> = HashMap::new();
            for s in &records.stops {
                if let Some(n) = s.stop_name.as_deref() {
                    *name_counts.entry(n).or_default() += 1;
                }
            }
            if let Some((&most_common_name, &most_count)) =
                name_counts.iter().max_by_key(|(_, &c)| c)
            {
                if most_count as f64 / total_named as f64 > 0.8 {
                    notices.push(k6_notice(
                        ctr, "DQ_022", EntityType::Feed,
                        None, None, "stops.txt", None, Some("stop_name"),
                        Some(format!("{:.0}%", most_count as f64 / total_named as f64 * 100.0)),
                        Some("≤80%".to_string()),
                        format!("Durakların {most_count}/{total_named} tanesinin adı '{most_common_name}' — yer tutucu veya test verisi olabilir."),
                        "Her durağa gerçek konumunu yansıtan benzersiz bir stop_name verin.",
                    ));
                }
            }
        }
    }

    // DQ_021: birincil anahtar yineleniyor (duplicate_key)
    // Check duplicates of stop_id, route_id, trip_id, service_id.
    {
        fn find_dups<'a, I: Iterator<Item = &'a str>>(ids: I) -> Vec<String> {
            let mut seen: HashMap<&str, u32> = HashMap::new();
            let mut dups: Vec<String> = Vec::new();
            for id in ids {
                let e = seen.entry(id).or_default();
                *e += 1;
                if *e == 2 {
                    dups.push(id.to_string());
                }
            }
            dups
        }

        for dup_id in find_dups(
            records
                .stops
                .iter()
                .filter(|_| stops_usable)
                .filter(|s| !s.stop_id.is_empty())
                .map(|s| s.stop_id.as_str()),
        ) {
            notices.push(k6_notice(
                ctr,
                "DQ_021",
                EntityType::Stop,
                Some(dup_id.clone()),
                Some(dup_id.clone()),
                "stops.txt",
                None,
                Some("stop_id"),
                Some(dup_id.clone()),
                None,
                format!("stop_id '{dup_id}' stops.txt'de birden fazla kez tanımlanmış."),
                "stops.txt'de benzersiz stop_id değerleri kullanın.",
            ));
        }
        for dup_id in find_dups(
            records
                .routes
                .iter()
                .filter(|_| routes_usable)
                .filter(|r| !r.route_id.is_empty())
                .map(|r| r.route_id.as_str()),
        ) {
            notices.push(k6_notice(
                ctr,
                "DQ_021",
                EntityType::Route,
                Some(dup_id.clone()),
                Some(dup_id.clone()),
                "routes.txt",
                None,
                Some("route_id"),
                Some(dup_id.clone()),
                None,
                format!("route_id '{dup_id}' routes.txt'de birden fazla kez tanımlanmış."),
                "routes.txt'de benzersiz route_id değerleri kullanın.",
            ));
        }
        for dup_id in find_dups(
            records
                .trips
                .iter()
                .filter(|_| trips_usable)
                .filter(|t| !t.trip_id.is_empty())
                .map(|t| t.trip_id.as_str()),
        ) {
            notices.push(k6_notice(
                ctr,
                "DQ_021",
                EntityType::Trip,
                Some(dup_id.clone()),
                Some(dup_id.clone()),
                "trips.txt",
                None,
                Some("trip_id"),
                Some(dup_id.clone()),
                None,
                format!("trip_id '{dup_id}' trips.txt'de birden fazla kez tanımlanmış."),
                "trips.txt'de benzersiz trip_id değerleri kullanın.",
            ));
        }
        // attributions.txt (attribution_id) — spec type `Unique ID`, the file's primary key.
        // Because the field is Optional, ATR_001 (missing) is Quality; UNIQUENESS is normative
        // and was never checked. An empty id does not count as a duplicate.
        for dup_id in find_dups(
            records
                .attributions
                .iter()
                .filter(|_| availability.present_and_available("attributions.txt"))
                .filter_map(|a| a.attribution_id.as_deref())
                .filter(|id| !id.is_empty()),
        ) {
            notices.push(k6_notice(
                ctr,
                "DQ_021",
                EntityType::Attribution,
                Some(dup_id.clone()),
                Some(dup_id.clone()),
                "attributions.txt",
                None,
                Some("attribution_id"),
                Some(dup_id.clone()),
                None,
                format!(
                    "attribution_id '{dup_id}' attributions.txt'de birden fazla kez tanımlanmış."
                ),
                "attributions.txt'de benzersiz attribution_id değerleri kullanın.",
            ));
        }
        // route_networks.txt (route_id) — spec primary key and explicit rule:
        // "A route_id can only be defined in one network_id." If the same route is in two networks,
        // which fare rule applies becomes ambiguous.
        for dup_id in find_dups(
            records
                .route_networks
                .iter()
                .filter(|_| availability.present_and_available("route_networks.txt"))
                .map(|r| r.route_id.as_str())
                .filter(|id| !id.is_empty()),
        ) {
            notices.push(k6_notice(ctr, "DQ_021", EntityType::Route,
                Some(dup_id.clone()), Some(dup_id.clone()),
                "route_networks.txt", None, Some("route_id"), Some(dup_id.clone()), None,
                format!("route_id '{dup_id}' route_networks.txt'de birden fazla kez tanımlanmış — bir hat yalnız bir ağa ait olabilir."),
                "Her hattı yalnız bir network_id ile eşleştirin."));
        }

        // ── COMPOSITE primary keys ───────────────────────────────────────────
        // The three files above have a single-field key; MOST Fares v2 and Flex files have
        // composite keys, and these duplicates were never checked. Key lists are copied verbatim
        // from the spec (Schedule Reference, file headers):
        //   fare_leg_rules.txt      (network_id, from_area_id, to_area_id,
        //                            from_timeframe_group_id, to_timeframe_group_id,
        //                            fare_product_id)
        //   fare_transfer_rules.txt (from_leg_group_id, to_leg_group_id, fare_product_id,
        //                            transfer_count, duration_limit)
        //   location_groups.txt     (location_group_id)
        //   location_group_stops.txt (*)  → ALL fields, i.e. an exact row duplicate
        // A blank field is part of the key (the spec treats "blank" values as meaningful), so
        // None and "" are represented identically; use unit separator (\u{1f}) as the delimiter
        // to prevent fields from merging.
        //
        // 🔴 Qualify `entity_id` with the FILE NAME — otherwise these would overwrite one another.
        // DQ_021 notices are initially emitted per duplicate key. The orchestration layer
        // later aggregates them by file, preserving file context and a small key sample.
        fn composite_dups<K: AsRef<str>>(keys: impl Iterator<Item = Vec<K>>) -> Vec<String> {
            let mut seen: HashMap<String, u32> = HashMap::new();
            let mut dups: Vec<String> = Vec::new();
            for parts in keys {
                let joined = parts
                    .iter()
                    .map(|p| p.as_ref())
                    .collect::<Vec<_>>()
                    .join("\u{1f}");
                let e = seen.entry(joined.clone()).or_default();
                *e += 1;
                if *e == 2 {
                    dups.push(joined.replace('\u{1f}', ", "));
                }
            }
            dups
        }
        let opt = |v: &Option<String>| v.clone().unwrap_or_default();

        for dup in composite_dups(
            records
                .fare_leg_rules
                .iter()
                .filter(|_| availability.present_and_available("fare_leg_rules.txt"))
                .map(|r| {
                    vec![
                        opt(&r.network_id),
                        opt(&r.from_area_id),
                        opt(&r.to_area_id),
                        opt(&r.from_timeframe_group_id),
                        opt(&r.to_timeframe_group_id),
                        r.fare_product_id.clone(),
                    ]
                }),
        ) {
            notices.push(k6_notice(ctr, "DQ_021", EntityType::Row,
                Some(format!("fare_leg_rules.txt: {dup}")), None,
                "fare_leg_rules.txt", None, Some("network_id|from_area_id|to_area_id|from_timeframe_group_id|to_timeframe_group_id|fare_product_id"),
                Some(dup.clone()), None,
                format!("fare_leg_rules.txt'de birincil anahtar yineleniyor: ({dup})."),
                "Bileşik birincil anahtarın (network_id, from_area_id, to_area_id, from_timeframe_group_id, to_timeframe_group_id, fare_product_id) her satırda benzersiz olmasını sağlayın."));
        }

        // fare_leg_join_rules.txt — the spec primary key is all FOUR fields. A blank value is
        // a meaningful part of the key (the spec says blank is "ignored in matching").
        for dup in composite_dups(
            records
                .fare_leg_join_rules
                .iter()
                .filter(|_| availability.present_and_available("fare_leg_join_rules.txt"))
                .map(|r| {
                    vec![
                        r.from_network_id.clone(),
                        r.to_network_id.clone(),
                        r.from_stop_id.clone(),
                        r.to_stop_id.clone(),
                    ]
                }),
        ) {
            notices.push(k6_notice(ctr, "DQ_021", EntityType::Row,
                Some(format!("fare_leg_join_rules.txt: {dup}")), None,
                "fare_leg_join_rules.txt", None,
                Some("from_network_id|to_network_id|from_stop_id|to_stop_id"),
                Some(dup.clone()), None,
                format!("fare_leg_join_rules.txt'de birincil anahtar yineleniyor: ({dup})."),
                "Bileşik birincil anahtarın (from_network_id, to_network_id, from_stop_id, to_stop_id) her satırda benzersiz olmasını sağlayın."));
        }

        for dup in composite_dups(
            records
                .fare_transfer_rules
                .iter()
                .filter(|_| availability.present_and_available("fare_transfer_rules.txt"))
                .map(|r| {
                    vec![
                        opt(&r.from_leg_group_id),
                        opt(&r.to_leg_group_id),
                        opt(&r.fare_product_id),
                        r.transfer_count.map(|v| v.to_string()).unwrap_or_default(),
                        r.duration_limit.map(|v| v.to_string()).unwrap_or_default(),
                    ]
                }),
        ) {
            notices.push(k6_notice(ctr, "DQ_021", EntityType::Row,
                Some(format!("fare_transfer_rules.txt: {dup}")), None,
                "fare_transfer_rules.txt", None, Some("from_leg_group_id|to_leg_group_id|fare_product_id|transfer_count|duration_limit"),
                Some(dup.clone()), None,
                format!("fare_transfer_rules.txt'de birincil anahtar yineleniyor: ({dup})."),
                "Bileşik birincil anahtarın (from_leg_group_id, to_leg_group_id, fare_product_id, transfer_count, duration_limit) her satırda benzersiz olmasını sağlayın."));
        }

        for dup_id in find_dups(
            records
                .location_groups
                .iter()
                .filter(|_| availability.present_and_available("location_groups.txt"))
                .filter(|g| !g.location_group_id.is_empty())
                .map(|g| g.location_group_id.as_str()),
        ) {
            notices.push(k6_notice(ctr, "DQ_021", EntityType::Row,
                Some(dup_id.clone()), Some(dup_id.clone()),
                "location_groups.txt", None, Some("location_group_id"), Some(dup_id.clone()), None,
                format!("location_group_id '{dup_id}' location_groups.txt'de birden fazla kez tanımlanmış."),
                "location_groups.txt'de benzersiz location_group_id değerleri kullanın."));
        }

        for dup in composite_dups(
            records
                .location_group_stops
                .iter()
                .filter(|_| availability.present_and_available("location_group_stops.txt"))
                .map(|s| vec![s.location_group_id.clone(), s.stop_id.clone()]),
        ) {
            notices.push(k6_notice(ctr, "DQ_021", EntityType::Row,
                Some(format!("location_group_stops.txt: {dup}")), None,
                "location_group_stops.txt", None, Some("location_group_id|stop_id"),
                Some(dup.clone()), None,
                format!("location_group_stops.txt'de aynı satır yineleniyor: ({dup})."),
                "location_group_stops.txt'de her (location_group_id, stop_id) çifti yalnız bir kez bulunmalıdır."));
        }

        // stop_areas.txt (*) and fare_rules.txt (*) — the spec defines the primary key as ALL
        // fields, so an exact row duplicate is a violation. The 2026-08-06 primary-key scan
        // found no rule for these two files (or calendar_dates); `SAR_001..004` check only
        // foreign keys and blanks, not duplicates.
        for dup in composite_dups(
            records
                .stop_areas
                .iter()
                .filter(|_| availability.present_and_available("stop_areas.txt"))
                .map(|s| vec![s.area_id.clone(), s.stop_id.clone()]),
        ) {
            notices.push(k6_notice(
                ctr,
                "DQ_021",
                EntityType::Row,
                Some(format!("stop_areas.txt: {dup}")),
                None,
                "stop_areas.txt",
                None,
                Some("area_id|stop_id"),
                Some(dup.clone()),
                None,
                format!("stop_areas.txt'de aynı satır yineleniyor: ({dup})."),
                "stop_areas.txt'de her (area_id, stop_id) çifti yalnız bir kez bulunmalıdır.",
            ));
        }

        for dup in composite_dups(
            records
                .fare_rules
                .iter()
                .filter(|_| availability.present_and_available("fare_rules.txt"))
                .map(|r| {
                    vec![
                        r.fare_id.clone(),
                        opt(&r.route_id),
                        opt(&r.origin_id),
                        opt(&r.destination_id),
                        opt(&r.contains_id),
                    ]
                }),
        ) {
            notices.push(k6_notice(ctr, "DQ_021", EntityType::Row,
                Some(format!("fare_rules.txt: {dup}")), None,
                "fare_rules.txt", None, Some("fare_id|route_id|origin_id|destination_id|contains_id"),
                Some(dup.clone()), None,
                format!("fare_rules.txt'de birincil anahtar yineleniyor: ({dup})."),
                "Aynı ücret kuralını iki kez yazmayın; tekrar eden satır ücret eşleşmesinde çift sayıma yol açar."));
        }

        // calendar_dates.txt (service_id, date) — ⚠️ THERE IS NO RECORD Vec HERE. The file is
        // STREAMED by file #38; only the K2 index is available. The index does NOT deduplicate
        // (`added`/`removed` retain raw rows and are SORTED after parsing) → duplicates remain visible.
        //
        // All three forms are the same violation, but the third is severe: if both
        // exception_type=1 and =2 are written for the same (service_id, date), whether the service
        // is OPEN or CLOSED that day is undefined and consumers make different decisions. The other
        // two (duplicate rows of the same type) look harmless but are still primary-key violations.
        //
        // ⚠️ Aggregate notices PER SERVICE, not per date: calendar_dates can be the feed's largest
        // file (Entur), and a notice per date would explode. Same pattern as `DQ_016`/`ARC_032`.
        if availability.present_and_available("calendar_dates.txt") {
            let cd = &records.calendar_dates;
            // FxHashMap iteration is nondeterministic → sort the keys.
            let mut services: Vec<&str> = cd
                .added
                .keys()
                .map(|s| s.as_str())
                .chain(cd.removed.keys().map(|s| s.as_str()))
                .collect();
            services.sort_unstable();
            services.dedup();

            // Adjacent equal entries in the sorted Vec = duplicate.
            fn repeated(sorted: &[u32]) -> Vec<u32> {
                let mut out = Vec::new();
                for w in sorted.windows(2) {
                    if w[0] == w[1] && out.last() != Some(&w[0]) {
                        out.push(w[0]);
                    }
                }
                out
            }
            // Common dates between two SORTED lists (merge scan).
            fn shared(a: &[u32], b: &[u32]) -> Vec<u32> {
                let (mut i, mut j) = (0usize, 0usize);
                let mut out = Vec::new();
                while i < a.len() && j < b.len() {
                    match a[i].cmp(&b[j]) {
                        std::cmp::Ordering::Less => i += 1,
                        std::cmp::Ordering::Greater => j += 1,
                        std::cmp::Ordering::Equal => {
                            if out.last() != Some(&a[i]) {
                                out.push(a[i]);
                            }
                            i += 1;
                            j += 1;
                        }
                    }
                }
                out
            }

            for sid in services {
                let added = cd.added.get(sid).map(Vec::as_slice).unwrap_or(&[]);
                let removed = cd.removed.get(sid).map(Vec::as_slice).unwrap_or(&[]);
                let conflicting = shared(added, removed);
                let mut dates: Vec<u32> = repeated(added);
                dates.extend(repeated(removed));
                dates.extend(conflicting.iter().copied());
                dates.sort_unstable();
                dates.dedup();
                if dates.is_empty() {
                    continue;
                }

                let sample: Vec<String> = dates.iter().take(5).map(|d| d.to_string()).collect();
                let more = if dates.len() > 5 {
                    format!(" (+{} tarih daha)", dates.len() - 5)
                } else {
                    String::new()
                };
                let conflict_note = if conflicting.is_empty() {
                    String::new()
                } else {
                    format!(
                        " Bunların {}'inde exception_type hem 1 hem 2 yazılmış — o günün servisi tanımsız.",
                        conflicting.len()
                    )
                };
                notices.push(k6_notice(ctr, "DQ_021", EntityType::Service,
                    Some(sid.to_string()), Some(sid.to_string()),
                    "calendar_dates.txt", None, Some("service_id|date"),
                    Some(format!("{}{}", sample.join(", "), more)), None,
                    format!(
                        "service_id '{sid}' için {} tarih calendar_dates.txt'te birden fazla kez yazılmış.{conflict_note}",
                        dates.len()
                    ),
                    "Her (service_id, date) çifti calendar_dates.txt'te yalnız bir kez bulunmalıdır."));
            }
        }
    }

    // ARC_020: recommended files are missing (missing_recommended_file).
    // GTFS best practices recommend shapes.txt and feed_info.txt.
    //
    // ⚠️ BOUNDARY: the spec has TWO sentences for `feed_info.txt` — "Required if translations.txt is
    // provided. Recommended otherwise." This rule represents only the SECOND sentence (recommendation)
    // and is therefore Quality. The first is a NORM measured by ARC_031 (Critical·Spec).
    // We do not narrow the condition to "when translations is absent": ARC_020 is already Low·Quality
    // and missing feed_info is a quality signal either way; if both findings appear, one describes
    // the norm and the other the recommendation.
    if feed_has_content {
        // shapes.txt exists to draw fixed-route geometry. If the feed is entirely demand-responsive
        // (DRT/flex), there is no route geometry to draw → missing shapes.txt is not a finding.
        // The feed_info.txt expectation is unaffected by DRT and is evaluated separately.
        let continuous_shape_required = records.routes.iter().any(|r| {
            matches!(r.continuous_pickup, Some(0) | Some(2) | Some(3))
                || matches!(r.continuous_drop_off, Some(0) | Some(2) | Some(3))
        }) || !records.stop_times_index.continuous_trips.is_empty();
        let missing_shapes = if availability.has_inventory() {
            !availability.present("shapes.txt")
                && !feed_is_demand_responsive_only(records)
                && !continuous_shape_required
        } else {
            records.shapes.is_empty()
                && !feed_is_demand_responsive_only(records)
                && !continuous_shape_required
        };
        let missing_feed_info = if availability.has_inventory() {
            !availability.present("feed_info.txt")
        } else {
            records.feed_info.is_empty()
        };
        if missing_shapes && missing_feed_info {
            notices.push(k6_notice(
                ctr,
                "ARC_020",
                EntityType::Feed,
                None,
                None,
                "shapes.txt/feed_info.txt",
                None,
                None,
                None,
                None,
                "Feed'de shapes.txt ve feed_info.txt dosyaları yok — her ikisi de önerilir."
                    .to_string(),
                "shapes.txt ile güzergah geometrisi ve feed_info.txt ile yayıncı bilgisi ekleyin.",
            ));
        } else if missing_shapes {
            notices.push(k6_notice(
                ctr,
                "ARC_020",
                EntityType::Feed,
                None,
                None,
                "shapes.txt",
                None,
                None,
                None,
                None,
                "Feed'de shapes.txt dosyası yok — güzergah geometrisi için önerilir.".to_string(),
                "shapes.txt dosyası oluşturarak güzergah geometrisi ekleyin.",
            ));
        } else if missing_feed_info {
            notices.push(k6_notice(
                ctr,
                "ARC_020",
                EntityType::Feed,
                None,
                None,
                "feed_info.txt",
                None,
                None,
                None,
                None,
                "Feed'de feed_info.txt dosyası yok — yayıncı ve geçerlilik bilgisi için önerilir."
                    .to_string(),
                "feed_info.txt dosyası oluşturarak yayıncı bilgisini tanımlayın.",
            ));
        }
    }
}

/// Is the feed entirely demand-responsive (DRT/flex)?
///
/// Condition: no row in stop_times.txt carries `stop_id`, while at least one row defines a
/// service area with `location_id` (area-based DRT) or `location_group_id` (fixed-stop DRT).
///
/// Deliberately STRICT: if even one fixed-stop trip exists, the feed is not DRT-only because
/// it has route geometry to draw and the shapes.txt expectation remains.
/// A loose measure such as "at least one Flex row" would silently mask missing shapes.txt in a
/// predominantly fixed-route feed.
///
/// Cost: `stop_id_set` lookup is O(1); `flex_map` is empty for most feeds.
fn feed_is_demand_responsive_only(records: &EntityRecords) -> bool {
    let idx = &records.stop_times_index;
    if idx.total_rows == 0 || !idx.stop_id_set.is_empty() {
        return false;
    }
    idx.flex_map
        .values()
        .any(|f| f.location_id.is_some() || f.location_group_id.is_some())
}

// ── Remaining K6 rules (STM_017, GEO_007/009/012/013, SHP_013, DQ_005b/005c/010,
//    RTS_017, TRP_012/015, OPR_005/013) ──────────────────────────────────────

const STM017_TRIP_SAMPLE: usize = 12;
const STM017_STOP_SAMPLE: usize = 10;

fn add_stm017_trip_details(
    details: &mut gtfs_core::NoticeDetails,
    idx: &StopTimesIndex<'_>,
    trip_id: &str,
    prefix: &str,
) {
    let Some(stops) = idx.by_trip.get(trip_id) else {
        return;
    };
    let missing: Vec<&CompactStopTime> = stops
        .iter()
        .filter(|stop| stop.shape_dist_traveled().is_none())
        .collect();
    details.insert(format!("{prefix}stop_count"), stops.len().to_string());
    details.insert(
        format!("{prefix}affected_stop_count"),
        missing.len().to_string(),
    );
    for (i, stop) in missing.into_iter().take(STM017_STOP_SAMPLE).enumerate() {
        let key = |name: &str| format!("{prefix}missing_{i}_{name}");
        details.insert(key("stop_id"), idx.stop_id_of(stop).to_string());
        if let Some(sequence) = stop.stop_sequence() {
            details.insert(key("stop_sequence"), sequence.to_string());
        }
        if let Some(arrival) = stop.arrival_time() {
            details.insert(key("arrival"), format_hms(hms_to_secs(arrival)));
        }
        if let Some(departure) = stop.departure_time() {
            details.insert(key("departure"), format_hms(hms_to_secs(departure)));
        }
    }
}

// This analysis stage takes shared indexes as separate borrows; a parameter struct is
// deliberately avoided because it would obscure lifetimes and data ownership.
#[allow(clippy::too_many_arguments)]
fn check_remaining_analytics<'a>(
    records: &'a EntityRecords,
    derived: &DerivedData,
    config: &ValidatorConfig,
    idx: &StopTimesIndex<'_>,
    shape_idx: &ShapeIndex<'a>,
    rail_shapes: &FxHashSet<&str>,
    notices: &mut Vec<Notice>,
    ctr: &mut u32,
) {
    use crate::timing::Timer;
    let ti_rem = &records.trip_interns;

    // ── shape geometry: borrowed from the shared ShapeIndex at K6 entry ──────
    let _tb = Timer::start("K6::rem::build_maps");
    // Built ONCE at K6 entry (see ShapeIndex) — only borrowed here.
    let shape_coords = &shape_idx.coords;
    let shape_bbox = &shape_idx.bbox;

    // ── stop_id → (lat, lon) ──────────────────────────────────────────────────
    let stop_coords: FxHashMap<&str, (f64, f64)> = records
        .stops
        .iter()
        .filter_map(|s| recoverable_stop_coord(s).map(|c| (s.stop_id.as_str(), c)))
        .collect();
    let stop_lines: FxHashMap<&str, u64> = records
        .stops
        .iter()
        .map(|s| (s.stop_id.as_str(), s.line))
        .collect();
    let stop_names: FxHashMap<&str, &str> = records
        .stops
        .iter()
        .filter_map(|s| s.stop_name.as_deref().map(|n| (s.stop_id.as_str(), n)))
        .collect();
    let trip_to_route_rem: FxHashMap<&str, &str> = records
        .trips
        .iter()
        .map(|t| (t.trip_id.as_str(), ti_rem.route_id(t)))
        .collect();
    let trip_to_service_rem: FxHashMap<&str, &str> = records
        .trips
        .iter()
        .map(|t| (t.trip_id.as_str(), ti_rem.service_id(t)))
        .collect();
    let trip_to_shape_rem: FxHashMap<&str, &str> = records
        .trips
        .iter()
        .filter_map(|t| ti_rem.shape_id(t).map(|shape| (t.trip_id.as_str(), shape)))
        .collect();
    let route_type_rem: FxHashMap<&str, u32> = records
        .routes
        .iter()
        .filter_map(|r| r.route_type.map(|rt| (r.route_id.as_str(), rt)))
        .collect();
    let trip_first_dep: FxHashMap<&str, (u32, u32, u32)> = idx
        .by_trip
        .iter()
        .filter_map(|(&tid, sts)| {
            sts.first()
                .and_then(|s| s.departure_time())
                .map(|d| (tid, d))
        })
        .collect();
    // route_id → display label (route_short_name when present, otherwise route_id).
    let route_short: FxHashMap<&str, &str> = records
        .routes
        .iter()
        .map(|r| {
            let label = r
                .route_short_name
                .as_deref()
                .filter(|s| !s.is_empty())
                .unwrap_or(r.route_id.as_str());
            (r.route_id.as_str(), label)
        })
        .collect();

    // shape_id → direction_id (benzersizse Some(dir), belirsizse None)
    // shape_id → route label (short_name for a unique route_id, None when ambiguous).
    type ShapeDirections<'a> = FxHashMap<&'a str, Option<u32>>;
    type ShapeRoutes<'a> = FxHashMap<&'a str, Option<&'a str>>;
    let (shape_directions, shape_to_route): (ShapeDirections<'_>, ShapeRoutes<'_>) = {
        let mut dirs: FxHashMap<&str, Option<u32>> = FxHashMap::default();
        let mut route_ids: FxHashMap<&str, Option<&str>> = FxHashMap::default();
        for t in &records.trips {
            if let Some(shape) = ti_rem.shape_id(t).filter(|s| !s.is_empty()) {
                let de = dirs.entry(shape).or_insert(t.direction_id);
                if *de != t.direction_id {
                    *de = None;
                }
                let rid = ti_rem.route_id(t);
                let re = route_ids.entry(shape).or_insert(Some(rid));
                if *re != Some(rid) {
                    *re = None;
                }
            }
        }
        let shape_to_route = route_ids
            .into_iter()
            .map(|(shape, rid)| {
                let label = rid.map(|r| route_short.get(r).copied().unwrap_or(r));
                (shape, label)
            })
            .collect();
        (dirs, shape_to_route)
    };
    // shape_id → labels of ALL routes using that shape (sorted, unique).
    // Used for the SHP_009/014/020/023 message prefix; falls back to route_id without route_short_name.
    let shape_route_labels: FxHashMap<&str, Vec<&str>> = {
        let mut m: FxHashMap<&str, FxHashSet<&str>> = FxHashMap::default();
        for t in &records.trips {
            if ti_rem.route_id(t).is_empty() {
                continue;
            }
            if let Some(shape) = ti_rem.shape_id(t).filter(|s| !s.is_empty()) {
                let rid = ti_rem.route_id(t);
                let label = route_short.get(rid).copied().unwrap_or(rid);
                m.entry(shape).or_default().insert(label);
            }
        }
        m.into_iter()
            .map(|(shape, set)| {
                let mut labels: Vec<&str> = set.into_iter().collect();
                labels.sort_unstable();
                (shape, labels)
            })
            .collect()
    };
    // Prefix "for routes [12, 14]" (SHP_009/014/020/023). Empty slice → empty prefix (orphan shape).
    let shp_route_prefix = |labels: &[&str]| -> String {
        if labels.is_empty() {
            return String::new();
        }
        let hat = if labels.len() == 1 {
            "hatta"
        } else {
            "hatlara"
        };
        format!("[{}] kodlu {hat} ait ", labels.join(", "))
    };
    drop(_tb);

    // SHP_011 REMOVED (2026-07-22): it scanned the same data as GEO_006 (segment_distances_km)
    // with the same threshold (config.max_shape_jump_km). Because GEO_006 has dedup_level=Entity
    // + scope=shape_id, K7 already reduces it to one notice per shape — SHP_011 had no "per-shape
    // summary" role, and the two rules produced exactly the same output after dedup. The "confusing
    // neighbors" tables in their cards did not mention each other either: the overlap was accidental,
    // not a documented design.

    // ── SHP_023: two consecutive shape points with the same dist_traveled and coordinates ─
    // (equal_shape_distance_same_coordinates)
    {
        let _ts23 = Timer::start("K6::rem::shp_023");
        // #15: instead of materializing shape_raw (seq,dist,lat,lon) ≈40B/point, group point indexes
        // (4B) and access data through records.shapes → removes the second large shape rebuild after
        // the build_maps OOM (the original unreachable path was here). Output/order are IDENTICAL.
        let mut shape_pt_idx: FxHashMap<&str, Vec<u32>> = FxHashMap::default();
        for (i, sp) in records.shapes.iter().enumerate() {
            if sp.shape_pt_lat().is_some() && sp.shape_pt_lon().is_some() {
                shape_pt_idx
                    .entry(records.shape_interns.id(sp))
                    .or_default()
                    .push(i as u32);
            }
        }
        for idxs in shape_pt_idx.values_mut() {
            idxs.sort_by_key(|&i| records.shapes[i as usize].shape_pt_sequence().unwrap_or(0));
        }
        for (shape_id, idxs) in &shape_pt_idx {
            // At most ONE notice of EACH TYPE (SHP_023/028/029) per shape. Previously break at the first
            // equal-distance pair → when a shape had both a repeated point (023) and below-threshold (029),
            // only the FIRST was reported (Spelunca: 6 SHP_023 missed). Now scan all pairs, emit each type
            // once, and exit early once all three are found.
            let mut fired_023 = false;
            let mut fired_028 = false;
            let mut fired_029 = false;
            for w in idxs.windows(2) {
                if fired_023 && fired_028 && fired_029 {
                    break;
                }
                let pa = &records.shapes[w[0] as usize];
                let pb = &records.shapes[w[1] as usize];
                let (da, la, loa) = (
                    pa.shape_dist_traveled(),
                    pa.shape_pt_lat().unwrap(),
                    pa.shape_pt_lon().unwrap(),
                );
                let (db, lb, lob) = (
                    pb.shape_dist_traveled(),
                    pb.shape_pt_lat().unwrap(),
                    pb.shape_pt_lon().unwrap(),
                );
                if let (Some(da_v), Some(db_v)) = (da, db) {
                    const EPS: f64 = 1e-9;
                    // SHP_028/029 coordinate-difference threshold: **1.1 METERS**, not degrees.
                    //
                    // 🔴 Previously the threshold was `max(|Δlat|,|Δlon|) >= 1e-5` degrees and the comment
                    // said "~1.1m". WRONG: 1e-5 degrees of longitude is 1.1 m only at the EQUATOR and shrinks
                    // by cos(lat). In France (49.6°) it becomes 0.72 m → sub-meter GPS jitter crosses the threshold.
                    // Measured in the 2026-08-06 corpus run (mdb-1840): 7 shapes were reported, MD reported 2.
                    // The extra 5 had real distances of 0.73–0.82 m. Haversine filters them and restores EXACT
                    // parity with MD (MD `actualDistanceBetweenShapePoints`: 2.00 m and 16.22 m).
                    const DIFF_THRESHOLD_M: f64 = 1.1;
                    if (da_v - db_v).abs() < EPS {
                        if (la - lb).abs() < EPS && (loa - lob).abs() < EPS {
                            // SHP_023: same distance + same coordinates (repeated point).
                            if !fired_023 {
                                fired_023 = true;
                                let prefix = shp_route_prefix(
                                    shape_route_labels
                                        .get(*shape_id)
                                        .map(|v| v.as_slice())
                                        .unwrap_or(&[]),
                                );
                                notices.push(k6_notice(
                                    ctr, "SHP_023", EntityType::Shape,
                                    Some(shape_id.to_string()), Some(shape_id.to_string()),
                                    "shapes.txt", None, Some("shape_dist_traveled"),
                                    Some(format!("dist={da_v:.4}, ({la:.6},{loa:.6})")), None,
                                    format!("{prefix}'{shape_id}' şeklinde art arda iki noktanın shape_dist_traveled ({da_v:.4}) ve koordinatları aynı — tekrar eden nokta."),
                                    "Yinelenen shape noktasını kaldırın.",
                                ));
                            }
                        } else {
                            // Same distance but different coordinates → position changed without increasing distance.
                            let coord_diff_m = haversine_km(la, loa, lb, lob) * 1000.0;
                            if coord_diff_m >= DIFF_THRESHOLD_M {
                                if !fired_028 {
                                    fired_028 = true;
                                    notices.push(k6_notice(
                                        ctr, "SHP_028", EntityType::Shape,
                                        Some(shape_id.to_string()), Some(shape_id.to_string()),
                                        "shapes.txt", None, Some("shape_dist_traveled"),
                                        Some(format!("dist={da_v:.4}, ({la:.6},{loa:.6})→({lb:.6},{lob:.6})")),
                                        Some("artan dist".to_string()),
                                        format!("'{shape_id}' şeklinde art arda iki noktanın shape_dist_traveled ({da_v:.4}) aynı ama koordinatları farklı — mesafe artmadan konum değişmiş."),
                                        "Ardışık shape noktalarına artan shape_dist_traveled değerleri atayın.",
                                    ));
                                }
                            } else if !fired_029 {
                                fired_029 = true;
                                notices.push(k6_notice(
                                    ctr, "SHP_029", EntityType::Shape,
                                    Some(shape_id.to_string()), Some(shape_id.to_string()),
                                    "shapes.txt", None, Some("shape_dist_traveled"),
                                    Some(format!("dist={da_v:.4}, Δ≈{coord_diff_m:.2}m")),
                                    Some("artan dist".to_string()),
                                    format!("'{shape_id}' şeklinde art arda iki noktanın shape_dist_traveled ({da_v:.4}) aynı, koordinat farkı çok küçük (eşik altı)."),
                                    "Ardışık shape noktalarına artan shape_dist_traveled değerleri atayın ya da yinelenen noktayı kaldırın.",
                                ));
                            }
                        }
                    }
                }
            }
        }
    }

    // ── STM_017: shape trip lacks shape_dist_traveled in stop_times ──────────
    // idx.trips_missing_sdt: trip_shape is filtered during build, and the line number is ready.
    // trip_shape contains only shapes DEFINED in shapes.txt: a trip linked to an undefined shape_id
    // has no shape to validate; TRP_004/ARC_020 report that issue (intentional, test:
    // `trip_referencing_undefined_shape_no_stm_017`).
    {
        let _ts = Timer::start("K6::rem::stm_017");
        // If shape_dist_traveled is unused across the entire feed, emit one feed-level summary instead
        // of tens of thousands of identical per-trip notices (STM_050 precedent).
        // If partially populated (a real inconsistency), preserve trip granularity.
        // Read from the index: in production `records.stop_times` Vec is empty; rows are in the K2 index.
        let any_sdt_present = idx
            .by_trip
            .values()
            .any(|stops| stops.iter().any(|s| s.shape_dist_traveled().is_some()));

        // non-rail (intercity rail does not provide shape_dist_traveled — expected behavior),
        // shape trips missing sdt.
        let flagged: Vec<(&str, u64)> = idx
            .trips_missing_sdt
            .iter()
            .filter(|(trip_id, _)| {
                let route_id = trip_to_route_rem
                    .get(**trip_id)
                    .copied()
                    .unwrap_or(**trip_id);
                let rt = route_type_rem.get(route_id).copied().unwrap_or(3);
                !is_rail_route_type(rt)
            })
            .map(|(&trip_id, &line)| (trip_id, line))
            .collect();

        if !any_sdt_present && flagged.len() > 1 {
            // Completely absent feed-wide → one summary notice (noise ~54k → 1).
            let n = flagged.len();
            let n_shapes = flagged
                .iter()
                .filter_map(|(tid, _)| trip_to_shape_rem.get(*tid).copied())
                .collect::<FxHashSet<&str>>()
                .len();
            let mut notice = k6_notice(
                ctr,
                "STM_017",
                EntityType::Feed,
                None,
                None,
                "stop_times.txt",
                None,
                Some("shape_dist_traveled"),
                Some(n.to_string()),
                None,
                format!("Feed genelinde shape_dist_traveled hiç kullanılmamış: {n_shapes} güzergaha ait {n} sefer etkileniyor — durak konumları güzergah üzerinde doğrulanamıyor."),
                "Tüm stop_times satırlarına shape_dist_traveled ekleyin ya da güzergah bazlı mesafe verisi sağlayın.",
            );
            let mut details = gtfs_core::NoticeDetails::new();
            details.insert("summary_mode".to_string(), "feed".to_string());
            details.insert("affected_trip_count".to_string(), n.to_string());
            details.insert("affected_shape_count".to_string(), n_shapes.to_string());
            let mut examples = flagged.clone();
            examples.sort_unstable_by_key(|(trip_id, _)| *trip_id);
            for (i, (trip_id, line)) in examples.into_iter().take(STM017_TRIP_SAMPLE).enumerate() {
                let prefix = format!("example_{i}_");
                details.insert(format!("{prefix}trip"), trip_id.to_string());
                details.insert(format!("{prefix}line"), line.to_string());
                if let Some(service_id) =
                    trip_to_service_rem.get(trip_id).filter(|id| !id.is_empty())
                {
                    details.insert(format!("{prefix}service_id"), (*service_id).to_string());
                }
                add_stm017_trip_details(&mut details, idx, trip_id, &prefix);
            }
            notice.details = Some(details);
            notices.push(notice);
        } else {
            for (trip_id, line) in flagged {
                let route = trip_to_route_rem.get(trip_id).copied().unwrap_or(trip_id);
                let dep = trip_first_dep
                    .get(trip_id)
                    .map(|(h, m, _)| format!("{h:02}:{m:02}"))
                    .unwrap_or_default();
                let dep_infix = if dep.is_empty() {
                    String::new()
                } else {
                    format!(" {dep}")
                };
                let mut notice = k6_notice(
                    ctr,
                    "STM_017",
                    EntityType::Trip,
                    Some(trip_id.to_string()),
                    Some(trip_id.to_string()),
                    "stop_times.txt",
                    Some(line),
                    Some("shape_dist_traveled"),
                    None,
                    None,
                    format!("'{}' hattının{dep_infix} seferinde güzergah mesafe bilgisi (shape_dist_traveled) eksik — durak konumları doğrulanamıyor.", route),
                    "Tüm stop_times satırlarına shape_dist_traveled ekleyin ya da tümünden kaldırın.",
                );
                if let Some(service_id) =
                    trip_to_service_rem.get(trip_id).filter(|id| !id.is_empty())
                {
                    notice.service_id = Some((*service_id).to_string());
                }
                let mut details = gtfs_core::NoticeDetails::new();
                details.insert("route_id".to_string(), route.to_string());
                if let Some(shape_id) = trip_to_shape_rem.get(trip_id) {
                    details.insert("shape_id".to_string(), (*shape_id).to_string());
                }
                add_stm017_trip_details(&mut details, idx, trip_id, "");
                notice.details = Some(details);
                notices.push(notice);
            }
        }
    }

    // ── SHP_030: stop_times distance present, shape distance missing ─────────
    // MobilityData equivalent of `trip_with_shape_dist_traveled_but_no_shape_distances`.
    // Both fields are optional in GTFS, so this is a consumer-compatibility Quality signal,
    // not a normative Spec rule. Emit one notice per shape: emitting per stop_time row would
    // multiply the same root cause thousands of times.
    {
        let _t30 = Timer::start("K6::rem::shp_030");
        let mut shape_stats: FxHashMap<&str, (u32, u32, u32, Vec<&str>)> = FxHashMap::default();
        // (shape point count, point count with sdt, affected trip count, representative trips)
        let mut shape_points: FxHashMap<&str, (u32, u32)> = FxHashMap::default();
        for sp in &records.shapes {
            let shape_id = records.shape_interns.id(sp);
            if shape_id.is_empty() {
                continue;
            }
            let entry = shape_points.entry(shape_id).or_default();
            entry.0 += 1;
            if sp.shape_dist_traveled().is_some() {
                entry.1 += 1;
            }
        }

        for trip in &records.trips {
            let Some(shape_id) = ti_rem.shape_id(trip).filter(|s| !s.is_empty()) else {
                continue;
            };
            let Some(stimes) = idx.by_trip.get(trip.trip_id.as_str()) else {
                continue;
            };
            if !stimes.iter().any(|st| st.shape_dist_traveled().is_some()) {
                continue;
            }
            let Some(&(point_count, with_sdt)) = shape_points.get(shape_id) else {
                continue;
            };
            if point_count == 0 || with_sdt == point_count {
                continue;
            }
            let entry =
                shape_stats
                    .entry(shape_id)
                    .or_insert((point_count, with_sdt, 0, Vec::new()));
            entry.2 += 1;
            if entry.3.len() < 3 {
                entry.3.push(trip.trip_id.as_str());
            }
        }

        let mut shape_ids: Vec<&str> = shape_stats.keys().copied().collect();
        shape_ids.sort_unstable();
        for shape_id in shape_ids {
            let (point_count, with_sdt, affected_trips, trip_ids) = &shape_stats[shape_id];
            let missing = point_count.saturating_sub(*with_sdt);
            let mut notice = k6_notice(
                ctr, "SHP_030", EntityType::Shape,
                Some(shape_id.to_string()), Some(shape_id.to_string()),
                "shapes.txt", None, Some("shape_dist_traveled"),
                Some(format!("{with_sdt}/{point_count}")),
                Some("tüm shape noktalarında değer".to_string()),
                format!("'{shape_id}' shape'i kullanan {affected_trips} trip stop_times.txt'te shape_dist_traveled kullanıyor, ancak shape noktalarının {missing}/{point_count} tanesinde shapes.txt shape_dist_traveled değeri eksik."),
                "İlgili shape noktalarına stop_times.txt ile aynı birimde shape_dist_traveled ekleyin veya bu alanı ilgili trip'lerin stop_times kayıtlarından tutarlı biçimde kaldırın.",
            );
            notice.details = Some(
                [
                    ("shape_point_count".to_string(), point_count.to_string()),
                    ("shape_points_with_dist".to_string(), with_sdt.to_string()),
                    ("shape_points_missing_dist".to_string(), missing.to_string()),
                    (
                        "affected_trip_count".to_string(),
                        affected_trips.to_string(),
                    ),
                    ("representative_trip_ids".to_string(), trip_ids.join(",")),
                ]
                .into_iter()
                .collect(),
            );
            notices.push(notice);
        }
    }

    // ── GEO_007: severe shape jump (3× threshold) ───────────────────────────
    {
        let _tg7 = Timer::start("K6::rem::geo_007");
        for (shape_id, seg) in &derived.shape_geometry.shapes {
            // Same per-shape threshold as GEO_006; preserve layering (this is above 3×).
            let severe_km = shape_jump_threshold_km(shape_id, rail_shapes, config) * 3.0;
            for (i, &dist) in seg.segment_distances_km.iter().enumerate() {
                if dist > severe_km {
                    notices.push(k6_notice(
                        ctr,
                        "GEO_007",
                        EntityType::Shape,
                        Some(shape_id.clone()),
                        Some(shape_id.clone()),
                        "shapes.txt",
                        None,
                        Some("shape_pt_lat|shape_pt_lon"),
                        Some(format!("{dist:.2} km (segment {i}→{})", i + 1)),
                        Some(format!("≤ {severe_km:.0} km")),
                        format!("'{shape_id}' güzergahının {i}→{}. segmentinde {dist:.2}km kritik atlama (3× eşik).", i + 1),
                        "Eksik güzergah noktaları ekleyin veya koordinatları doğrulayın.",
                    ));
                }
            }
        }
    }

    // ── GEO_009 / SHP_013: stop is too far from the shape ────────────────────
    // Option-1: if another shape on the same route covers the stop (a different route variant),
    // skip that stop to avoid an FP.
    // Trip time: add an example departure time to the notice message.

    // shape_id → route_id set (for the Option-1 comparison).
    let shape_route_set: FxHashMap<&str, FxHashSet<&str>> = {
        let mut m: FxHashMap<&str, FxHashSet<&str>> = FxHashMap::default();
        for t in &records.trips {
            if let Some(sid) = ti_rem.shape_id(t).filter(|s| !s.is_empty()) {
                m.entry(sid).or_default().insert(ti_rem.route_id(t));
            }
        }
        m
    };
    // shape_id → [trip_id] (for the example departure time).
    let shape_trips: FxHashMap<&str, Vec<&str>> = {
        let mut m: FxHashMap<&str, Vec<&str>> = FxHashMap::default();
        for t in &records.trips {
            if let Some(sid) = ti_rem.shape_id(t).filter(|s| !s.is_empty()) {
                m.entry(sid).or_default().push(t.trip_id.as_str());
            }
        }
        m
    };

    {
        let _tg9 = Timer::start("K6::rem::geo_009_shp_013");

        for (&stop_id, shape_ids) in &idx.stop_shapes {
            let Some(&(slat, slon)) = stop_coords.get(stop_id) else {
                continue;
            };
            let cos_lat = (slat.to_radians()).cos().max(0.001_f64);
            let scale_lon = 111.0 * cos_lat;

            // Calculate distance for every shape — the Option-1 filter compares all distances.
            // Because the threshold varies PER SHAPE (rail/urban), carry the bbox margin, safe_sq
            // shortcut, and Option-1 comparison with the threshold; measure each shape by its own threshold.
            let dists: Vec<(&str, f64, f64)> = shape_ids
                .iter()
                .filter_map(|&shape_idx| {
                    let sid = records.shape_interns.id_at(shape_idx);
                    let pts = shape_coords.get(sid)?;
                    if pts.is_empty() {
                        return None;
                    }
                    let threshold_km = stop_shape_threshold_m(sid, rail_shapes, config) / 1000.0;
                    let margin_lat_g = threshold_km / 111.0;
                    let margin_lon = threshold_km / scale_lon;
                    let safe_sq = threshold_km * threshold_km * 0.98;
                    let d = if let Some(&[bmin_la, bmax_la, bmin_lo, bmax_lo]) = shape_bbox.get(sid)
                    {
                        if slat < bmin_la - margin_lat_g
                            || slat > bmax_la + margin_lat_g
                            || slon < bmin_lo - margin_lon
                            || slon > bmax_lo + margin_lon
                        {
                            let clat = slat.clamp(bmin_la, bmax_la);
                            let clon = slon.clamp(bmin_lo, bmax_lo);
                            haversine_km(slat, slon, clat, clon)
                        } else {
                            seg_min_dist_km(pts, slat, slon, scale_lon, safe_sq)
                        }
                    } else {
                        seg_min_dist_km(pts, slat, slon, scale_lon, safe_sq)
                    };
                    Some((sid, d, threshold_km))
                })
                .collect();

            for &(shape_id, min_dist_km, threshold_km) in &dists {
                if min_dist_km <= threshold_km {
                    continue;
                }

                // Option-1: skip if another shape on the same route covers this stop.
                // Evaluate the sibling shape with ITS OWN threshold (a rail sibling covers more broadly).
                if let Some(rx) = shape_route_set.get(shape_id) {
                    let covered = dists.iter().any(|&(sib, sib_dist, sib_thr)| {
                        sib != shape_id
                            && sib_dist <= sib_thr
                            && shape_route_set
                                .get(sib)
                                .is_some_and(|ry| rx.intersection(ry).next().is_some())
                    });
                    if covered {
                        continue;
                    }
                }

                // Example departure time: earliest trip for this shape × stop.
                let example_dep = shape_trips.get(shape_id).and_then(|trips| {
                    trips
                        .iter()
                        .filter_map(|&tid| {
                            idx.by_trip
                                .get(tid)?
                                .iter()
                                .find(|st| idx.stop_id_of(st) == stop_id)
                                .and_then(|st| st.departure_time())
                                .map(|(h, m, _)| h * 3600 + m * 60)
                        })
                        .min()
                        .map(|secs| format!("{:02}:{:02}", secs / 3600, (secs % 3600) / 60))
                });

                let dist_m = min_dist_km * 1000.0;
                let stop_name = stop_names.get(stop_id).copied().unwrap_or(stop_id);
                let dir_label = shape_directions
                    .get(shape_id)
                    .and_then(|d| *d)
                    .map(|d| {
                        if d == 0 {
                            " gidiş yönü"
                        } else {
                            " dönüş yönü"
                        }
                    })
                    .unwrap_or("");
                let ref_label = match shape_to_route.get(shape_id).copied().flatten() {
                    Some(route) => format!("'{}' hattının{dir_label} güzergahından", route),
                    None => format!("'{}' no'lu güzergahtan", shape_id),
                };
                let dep_suffix = example_dep
                    .as_deref()
                    .map(|t| format!(", örn. {t} kalkışlı sefer"))
                    .unwrap_or_default();

                let mut notice = k6_notice(
                    ctr,
                    "GEO_009",
                    EntityType::Stop,
                    Some(stop_id.to_string()),
                    Some(stop_id.to_string()),
                    "stops.txt",
                    stop_lines.get(stop_id).copied(),
                    Some("stop_lat|stop_lon"),
                    Some(format!("{dist_m:.1}m (shape '{shape_id}')")),
                    Some(format!("≤ {:.0}m", threshold_km * 1000.0)),
                    format!("'{}' kodlu '{}' durağı {ref_label} {dist_m:.1}m uzakta{dep_suffix} (eşik: {:.0}m).",
                        stop_id, stop_name, threshold_km * 1000.0),
                    "Durak koordinatlarını veya güzergah noktalarını düzeltin.",
                );
                // Carry shape_id in details (2026-07-25): because the entity is a stop, the shape
                // existed only in observed_value ("… (shape 'X')"). In large-feed mode, name_index
                // filters map data by IDs in notices; because the ID inside the text was not visible,
                // that shape was not serialized and the map route COULD NOT BE DRAWN. The UI now reads
                // details (regex fallback remains).
                let mut det: std::collections::BTreeMap<String, String> =
                    [("shape_id".to_string(), shape_id.to_string())]
                        .into_iter()
                        .collect();
                if let Some(dep) = &example_dep {
                    det.insert("example_dep".to_string(), dep.clone());
                }
                notice.details = Some(det.into());
                notices.push(notice);
                // SHP_013 removed — it reported the same physical condition as GEO_009 (double count).
            }
        }
    }

    // ── SHP_024: stop-to-shape distance is inconsistent with shape_dist_traveled ─
    // (stop_too_far_from_shape_using_user_distance)
    // Difference from GEO_009: calculate distance to the shape point at the position defined by
    // shape_dist_traveled, not the minimum distance to the polyline.
    {
        let _ts24 = Timer::start("K6::rem::shp_024");

        // shape_id → sorted Vec<(dist, lat, lon)> — only points with dist_traveled.
        let mut shape_sdt_pts: FxHashMap<&str, Vec<(f64, f64, f64)>> = FxHashMap::default();
        for sp in &records.shapes {
            if let (Some(dist), Some(lat), Some(lon)) = (
                sp.shape_dist_traveled(),
                sp.shape_pt_lat(),
                sp.shape_pt_lon(),
            ) {
                shape_sdt_pts
                    .entry(records.shape_interns.id(sp))
                    .or_default()
                    .push((dist, lat, lon));
            }
        }
        for pts in shape_sdt_pts.values_mut() {
            pts.sort_by(|a, b| a.0.partial_cmp(&b.0).unwrap_or(std::cmp::Ordering::Equal));
        }
        let stop_sdt_coords: FxHashMap<&str, (f64, f64)> = records
            .stops
            .iter()
            .filter_map(|stop| {
                recoverable_stop_coord(stop).map(|coords| (stop.stop_id.as_str(), coords))
            })
            .collect();

        let mut seen_shp024: FxHashSet<(&str, &str)> = FxHashSet::default(); // (stop_id, shape_id)

        for trip in &records.trips {
            let shape_id = match ti_rem.shape_id(trip).filter(|s| !s.is_empty()) {
                Some(s) => s,
                None => continue,
            };
            let sdt_pts = match shape_sdt_pts.get(shape_id) {
                Some(pts) if pts.len() >= 2 => pts,
                _ => continue,
            };
            // Threshold per shape (platform-center deviation is legitimate at rail stations).
            let threshold_km = stop_shape_threshold_m(shape_id, rail_shapes, config) / 1000.0;
            let stimes = match idx.by_trip.get(trip.trip_id.as_str()) {
                Some(v) => v,
                None => continue,
            };

            for st in stimes.iter() {
                let sdt = match st.shape_dist_traveled() {
                    Some(d) => d,
                    None => continue,
                };
                let stop_id = idx.stop_id_of(st);
                if seen_shp024.contains(&(stop_id, shape_id)) {
                    continue;
                }

                let (slat, slon) = match stop_sdt_coords.get(stop_id) {
                    Some(&c) => c,
                    None => continue,
                };

                // Find the position corresponding to the sdt value by interpolating on the shape.
                let pos = sdt_pts.partition_point(|&(d, _, _)| d <= sdt);
                let (ilat, ilon) = if pos == 0 {
                    let &(_, la, lo) = sdt_pts.first().unwrap();
                    (la, lo)
                } else if pos >= sdt_pts.len() {
                    let &(_, la, lo) = sdt_pts.last().unwrap();
                    (la, lo)
                } else {
                    let (da, la, loa) = sdt_pts[pos - 1];
                    let (db, lb, lob) = sdt_pts[pos];
                    if (db - da).abs() < 1e-9 {
                        (la, loa)
                    } else {
                        let t = (sdt - da) / (db - da);
                        (la + t * (lb - la), loa + t * (lob - loa))
                    }
                };

                let dist_km = haversine_km(slat, slon, ilat, ilon);
                if dist_km > threshold_km {
                    seen_shp024.insert((stop_id, shape_id));
                    let stop_name = stop_names.get(stop_id).copied().unwrap_or(stop_id);
                    notices.push(k6_notice(
                        ctr, "SHP_024", EntityType::Stop,
                        Some(stop_id.to_string()), Some(stop_id.to_string()),
                        "stop_times.txt", Some(st.line as u64), Some("shape_dist_traveled"),
                        Some(format!(
                            "distance={:.1}m; shape_id={shape_id}; shape_dist_traveled={sdt:.3}",
                            dist_km * 1000.0
                        )),
                        Some(format!("≤ {:.0}m", threshold_km * 1000.0)),
                        format!("'{}' durağı (shape_dist_traveled={sdt:.3}) shape_dist_traveled konumundan {:.1}m uzakta (eşik: {:.0}m).",
                            stop_name, dist_km * 1000.0, threshold_km * 1000.0),
                        "stop_times.txt'deki shape_dist_traveled değerini ya da stop koordinatlarını düzeltin.",
                    ));
                }
            }
        }
    }

    // ── SHP_025: stop_times shape_dist_traveled exceeds the shape's total distance ─
    // (trip_distance_exceeds_shape_distance)
    {
        let _ts25 = Timer::start("K6::rem::shp_025");
        // shape_id → maximum shape_dist_traveled value (from shapes.txt).
        let mut shape_max_sdt: FxHashMap<&str, f64> = FxHashMap::default();
        for sp in &records.shapes {
            if let Some(d) = sp.shape_dist_traveled() {
                let e = shape_max_sdt
                    .entry(records.shape_interns.id(sp))
                    .or_insert(0.0);
                if d > *e {
                    *e = d;
                }
            }
        }
        // Check only when every shape point has dist_traveled
        // (comparison is unreliable when some points in shapes.txt lack it).
        let mut shape_has_full_sdt: FxHashSet<&str> = FxHashSet::default();
        {
            let mut shape_total: FxHashMap<&str, u32> = FxHashMap::default();
            let mut shape_with_sdt: FxHashMap<&str, u32> = FxHashMap::default();
            for sp in &records.shapes {
                if sp.shape_idx() != 0 {
                    *shape_total.entry(records.shape_interns.id(sp)).or_default() += 1;
                    if sp.shape_dist_traveled().is_some() {
                        *shape_with_sdt
                            .entry(records.shape_interns.id(sp))
                            .or_default() += 1;
                    }
                }
            }
            for (sid, total) in &shape_total {
                if shape_with_sdt.get(sid).copied().unwrap_or(0) == *total {
                    shape_has_full_sdt.insert(sid);
                }
            }
        }

        for trip in &records.trips {
            let shape_id = match ti_rem.shape_id(trip).filter(|s| !s.is_empty()) {
                Some(s) => s,
                None => continue,
            };
            if !shape_has_full_sdt.contains(shape_id) {
                continue;
            }
            let shape_max = match shape_max_sdt.get(shape_id) {
                Some(&m) => m,
                None => continue,
            };

            let stimes = match idx.by_trip.get(trip.trip_id.as_str()) {
                Some(v) => v,
                None => continue,
            };

            // Maximum shape_dist_traveled value in stop_times.
            let trip_max_sdt = stimes
                .iter()
                .filter_map(|st| st.shape_dist_traveled())
                .fold(f64::NEG_INFINITY, f64::max);

            if trip_max_sdt.is_finite() && trip_max_sdt > shape_max * 1.001 {
                let route = trip_to_route_rem
                    .get(trip.trip_id.as_str())
                    .copied()
                    .unwrap_or(trip.trip_id.as_str());
                notices.push(k6_notice(
                    ctr, "SHP_025", EntityType::Trip,
                    Some(trip.trip_id.to_string()), Some(trip.trip_id.to_string()),
                    "stop_times.txt", None, Some("shape_dist_traveled"),
                    Some(format!("{trip_max_sdt:.3}")),
                    Some(format!("≤ {shape_max:.3}")),
                    format!("'{}' hattının seferinde stop_times shape_dist_traveled ({trip_max_sdt:.3}) şeklin maksimum değerini ({shape_max:.3}) aşıyor.",
                        route),
                    "stop_times.txt'deki shape_dist_traveled değerlerini shapes.txt ölçeğiyle eşleştirin.",
                ));
            }
        }
    }

    // ── GEO_012: stop cluster — more than three stops are too close ──────────
    {
        let _tg12 = Timer::start("K6::rem::geo_012");
        let cluster_km = config.stop_too_close_m / 1000.0;
        for cell_stops in derived.spatial_index.grid.values() {
            if cell_stops.len() < 3 {
                continue;
            }
            // Instead of checking every triple: count all stops in the cell
            // and report a cluster when any are close.
            let cluster_count = cell_stops
                .iter()
                .filter(|&&i| records.stops.get(i).is_some())
                .count();

            if cluster_count >= 3 {
                // Anchor stop: the FIRST ELIGIBLE stop in the cell.
                // Guards CONSISTENT with STP_016/017 (2026-07-24): clustering is meaningful only
                // among BOARDABLE stops. The station skeleton (entrance=2, node=3, boarding=4) and
                // sibling platforms of the same station are already close by hierarchy modeling;
                // the root cause of a Null Island placeholder cluster is GEO_016.
                //
                // ⚠️ 2026-07-25: guards were first applied only to `cell_stops[0]` and `continue`
                // skipped the ENTIRE CELL → if its first stop was a station/placeholder, a real
                // cluster inside was never seen (false negative). Now skip an ineligible anchor
                // and try the next boardable stop in the cell.
                let anchor_entry = cell_stops.iter().copied().find(|&i| {
                    records.stops.get(i).is_some_and(|s| {
                        if s.location_type.unwrap_or(0) != 0 {
                            return false;
                        }
                        recoverable_stop_coord(s)
                            .is_some_and(|(la, lo)| !(la.abs() < 0.1 && lo.abs() < 0.1))
                    })
                });
                let Some(first_idx) = anchor_entry else {
                    continue;
                };
                if let Some(anchor) = records.stops.get(first_idx) {
                    let Some((alat, alon)) = recoverable_stop_coord(anchor) else {
                        continue;
                    };
                    let anchor_parent = anchor
                        .row
                        .get("parent_station")
                        .map(|s| s.trim())
                        .filter(|s| !s.is_empty());
                    let nearby = cell_stops
                        .iter()
                        .filter(|&&i| {
                            i != first_idx
                                && records
                                    .stops
                                    .get(i)
                                    .map(|s| {
                                        if s.location_type.unwrap_or(0) != 0 {
                                            return false;
                                        }
                                        let p = s
                                            .row
                                            .get("parent_station")
                                            .map(|v| v.trim())
                                            .filter(|v| !v.is_empty());
                                        // parent/child or same-parent sibling → do not count in the cluster.
                                        if p == Some(anchor.stop_id.as_str())
                                            || anchor_parent == Some(s.stop_id.as_str())
                                            || (anchor_parent.is_some() && anchor_parent == p)
                                        {
                                            return false;
                                        }
                                        recoverable_stop_coord(s)
                                            .map(|(la, lo)| {
                                                haversine_km(alat, alon, la, lo) < cluster_km
                                            })
                                            .unwrap_or(false)
                                    })
                                    .unwrap_or(false)
                        })
                        .count();
                    if nearby >= 2 {
                        notices.push(k6_notice(
                            ctr,
                            "GEO_012",
                            EntityType::Stop,
                            Some(anchor.stop_id.clone()),
                            Some(anchor.stop_id.clone()),
                            "stops.txt",
                            Some(anchor.line),
                            Some("stop_lat|stop_lon"),
                            Some(format!("{}", nearby + 1)),
                            None,
                            format!("'{}' durağu (kod: '{}') çevresinde {} durak {:.0}m içinde kümelenmiş.",
                                anchor.stop_name.as_deref().unwrap_or(anchor.stop_id.as_str()),
                                anchor.stop_id, nearby + 1, config.stop_too_close_m),
                            "Duraksallar aynı fiziksel noktayı temsil ediyorsa birleştirin.",
                        ));
                    }
                }
            }
        }
    }

    // ── GEO_013: Feed geographic coverage summary (Info) ─────────────────────
    {
        let n_stops_with_coords = records
            .stops
            .iter()
            .filter(|s| s.stop_lat.is_some())
            .count();
        if n_stops_with_coords > 0 {
            notices.push(k6_notice(
                ctr,
                "GEO_013",
                EntityType::Feed,
                None,
                None,
                "stops.txt",
                None,
                None,
                Some(format!("{n_stops_with_coords}")),
                None,
                format!("Feed {n_stops_with_coords} koordinatlı durak içeriyor."),
                "Bu bilgi notu; aksiyona gerek yoktur.",
            ));
        }
    }

    // ── OPR_005: unusual service frequency (relative outlier by route_type) ──
    // "Frequent/sparse" is relative, not absolute: instead of a fixed threshold, flag a
    // (route,direction,calendar) group's average headway when it deviates from the median of
    // groups with the SAME route_type by robust σ (MAD). Use one route_type baseline (do not
    // compare metro with rural buses). A feed with uniform frequency produces zero notices.
    {
        let _topr5 = Timer::start("K6::rem::opr_005");
        let route_short_opr5: HashMap<&str, &str> = records
            .routes
            .iter()
            .map(|r| {
                let label = r
                    .route_short_name
                    .as_deref()
                    .filter(|s| !s.is_empty())
                    .unwrap_or(r.route_id.as_str());
                (r.route_id.as_str(), label)
            })
            .collect();
        let route_type_opr5: HashMap<&str, u32> = records
            .routes
            .iter()
            .map(|r| (r.route_id.as_str(), r.route_type.unwrap_or(3)))
            .collect();

        // (route_id, direction_key, service_id) → first departure times.
        let mut route_headways: HashMap<(&str, &str, &str), Vec<u32>> = HashMap::new();
        for trip in &records.trips {
            let route_id = ti_rem.route_id(trip);
            if route_id.is_empty() {
                continue;
            }
            let dir_key: &str = match trip.direction_id {
                Some(0) => "0",
                Some(1) => "1",
                _ => "-",
            };
            let svc_key = ti_rem.service_id(trip);
            if let Some(&dep) = idx.trip_first_dep.get(trip.trip_id.as_str()) {
                route_headways
                    .entry((route_id, dir_key, svc_key))
                    .or_default()
                    .push(dep);
            }
        }

        // 1) Average headway per group.
        struct Hw<'a> {
            route: &'a str,
            dir: &'a str,
            svc: &'a str,
            hw: u32,
            rtype: u32,
        }
        let mut grps: Vec<Hw> = Vec::new();
        for ((route_id, dir_key, svc_key), mut deps) in route_headways {
            if deps.len() < 2 {
                continue;
            }
            deps.sort_unstable();
            deps.dedup();
            if deps.len() < 2 {
                continue;
            }
            let diffs: Vec<u32> = deps.windows(2).map(|w| w[1] - w[0]).collect();
            let avg_hw = diffs.iter().sum::<u32>() / diffs.len() as u32;
            let rtype = route_type_opr5.get(route_id).copied().unwrap_or(3);
            grps.push(Hw {
                route: route_id,
                dir: dir_key,
                svc: svc_key,
                hw: avg_hw,
                rtype,
            });
        }

        // 2) Median + MAD per route_type (requires ≥5 groups for a baseline).
        let mut by_type: HashMap<u32, Vec<u32>> = HashMap::new();
        for g in &grps {
            by_type.entry(g.rtype).or_default().push(g.hw);
        }
        let mut type_stats: HashMap<u32, (f64, f64)> = HashMap::new();
        for (rt, mut hws) in by_type {
            if hws.len() < 5 {
                continue;
            }
            hws.sort_unstable();
            let median = hws[hws.len() / 2] as f64;
            let mut devs: Vec<f64> = hws.iter().map(|&h| (h as f64 - median).abs()).collect();
            devs.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
            let mad = devs[devs.len() / 2] * 1.4826;
            type_stats.insert(rt, (median, mad));
        }

        // 3) Emit groups that are outliers only against the same route_type median.
        // Sort for determinism.
        grps.sort_by(|a, b| (a.route, a.dir, a.svc).cmp(&(b.route, b.dir, b.svc)));
        for g in &grps {
            let (median, mad) = match type_stats.get(&g.rtype) {
                Some(&s) => s,
                None => continue,
            };
            let hw = g.hw as f64;
            let use_ratio = mad < 30.0; // ~all routes have nearly the same frequency → use ratio
            let is_outlier = if use_ratio {
                let r = hw / median;
                !(0.5..=2.0).contains(&r)
            } else {
                (hw - median).abs() > config.headway_outlier_sigma * mad
            };
            if !is_outlier {
                continue;
            }
            let yon_str = if hw > median {
                "sıradışı seyrek"
            } else {
                "sıradışı sık"
            };
            let route_label = route_short_opr5.get(g.route).copied().unwrap_or(g.route);
            let hw_min = hw / 60.0;
            let med_min = median / 60.0;
            let mut n = k6_notice(
                ctr,
                "OPR_005",
                EntityType::Route,
                Some(g.route.to_string()),
                Some(g.route.to_string()),
                "stop_times.txt",
                None,
                None,
                Some(format!("{hw_min:.0}")),
                None,
                format!("'{route_label}' hattının {} yönünde {} takviminde ortalama sefer aralığı {hw_min:.0}dk — aynı tür hatların feed medyanı {med_min:.0}dk ({yon_str}).",
                    g.dir, g.svc),
                "Bu bilgi notu; sefer sıklığı aynı tür hatlara göre sıradışı.",
            );
            n.service_id = Some(g.svc.to_string());
            notices.push(n);
        }
    }

    // ── OPR_013: one-way route information (all trips share one direction_id) ─
    {
        let _topr13 = Timer::start("K6::rem::opr_013");
        // route_id → display label (route_short_name when present, otherwise route_id).
        let route_short_opr13: HashMap<&str, &str> = records
            .routes
            .iter()
            .map(|r| {
                let label = r
                    .route_short_name
                    .as_deref()
                    .filter(|s| !s.is_empty())
                    .unwrap_or(r.route_id.as_str());
                (r.route_id.as_str(), label)
            })
            .collect();
        let mut route_directions: HashMap<&str, HashSet<u32>> = HashMap::new();
        for t in &records.trips {
            if ti_rem.route_id(t).is_empty() {
                continue;
            }
            if let Some(dir) = t.direction_id {
                route_directions
                    .entry(ti_rem.route_id(t))
                    .or_default()
                    .insert(dir);
            }
        }
        for (route_id, dirs) in &route_directions {
            if dirs.len() == 1 {
                let dir = dirs.iter().next().copied().unwrap_or(0);
                let label = route_short_opr13.get(route_id).copied().unwrap_or(route_id);
                let mut notice = k6_notice(
                    ctr,
                    "OPR_013",
                    EntityType::Route,
                    Some(route_id.to_string()),
                    Some(route_id.to_string()),
                    "trips.txt",
                    None,
                    Some("direction_id"),
                    Some(format!("direction_id={dir}")),
                    None,
                    format!("'{label}' kodlu hatta yalnızca {} yönlü seferler var — karşı yön tanımlı değil.",
                        if dir == 0 { "gidiş" } else { "dönüş" }),
                    "Bu bilgi notu; tek yönlü hatlar için beklenen bir durumdur.",
                );
                // en/ja/fr templates are filled with `{route_label}`.
                let details = notice.details.get_or_insert_with(Default::default);
                details.insert("route_label".to_string(), label.to_string());
                notices.push(notice);
            }
        }
    }

    // ── DQ_005b: no trip in the feed has stop_times ──────────────────────────
    {
        if !records.trips.is_empty() {
            let trips_without = records
                .trips
                .iter()
                .filter(|t| !idx.by_trip.contains_key(t.trip_id.as_str()))
                .count();
            let total = records.trips.len();
            if trips_without == total {
                // No trip has stop_times — broader feed-level scope than XFL_002.
                notices.push(k6_notice(
                    ctr,
                    "DQ_005b",
                    EntityType::Feed,
                    None,
                    None,
                    "stop_times.txt",
                    None,
                    None,
                    None,
                    None,
                    "Feed'deki hiçbir sefer için stop_times kaydı bulunamadı.".to_string(),
                    "stop_times.txt dosyasını oluşturun.",
                ));
            }
        }
    }

    // ── DQ_005c: ratio of stops without coordinates is too high ───────────────
    {
        if !records.stops.is_empty() {
            let without_coords = records
                .stops
                .iter()
                .filter(|s| s.stop_lat.is_none() || s.stop_lon.is_none())
                .count();
            let ratio = without_coords as f64 / records.stops.len() as f64;
            if ratio > 0.5 {
                notices.push(k6_notice(
                    ctr,
                    "DQ_005c",
                    EntityType::Feed,
                    None,
                    None,
                    "stops.txt",
                    None,
                    None,
                    Some(format!("{:.0}%", ratio * 100.0)),
                    Some("≤ %50".to_string()),
                    format!(
                        "Duraksaların %{:.0}'ında koordinat eksik — coğrafi analiz kısıtlı.",
                        ratio * 100.0
                    ),
                    "stop_lat ve stop_lon alanlarını tüm duraklara ekleyin.",
                ));
            }
        }
    }

    // ── DQ_010: route olmayan agency ──────────────────────────────────────────
    {
        let agencies_in_routes: HashSet<&str> = records
            .routes
            .iter()
            .filter_map(|r| r.agency_id.as_deref())
            .collect();
        let has_implicit_single_agency = records.agencies.len() == 1
            && records
                .routes
                .iter()
                .any(|r| r.agency_id.as_deref().is_none_or(|id| id.trim().is_empty()));
        for ag in &records.agencies {
            let Some(ref aid) = ag.agency_id else {
                continue;
            };
            if !agencies_in_routes.contains(aid.as_str()) && !has_implicit_single_agency {
                notices.push(k6_notice(
                    ctr,
                    "DQ_010",
                    EntityType::Agency,
                    // We have `agency_id`; when left empty, the en/ja/fr text became
                    // "Agency '' is not used by any route.".
                    Some(aid.clone()),
                    Some(aid.clone()),
                    "agency.txt",
                    Some(ag.line),
                    Some("agency_id"),
                    Some(aid.clone()),
                    None,
                    format!(
                        "'{}' işleticisi hiçbir hatta kullanılmıyor.",
                        if ag.agency_name.is_empty() {
                            aid.as_str()
                        } else {
                            ag.agency_name.as_str()
                        }
                    ),
                    "Kullanılmayan agency kaydını kaldırın ya da routes.txt'de agency_id atayın.",
                ));
            }
        }
    }

    // ── RTS_017: route without a defined shape — one Info notice per route ───
    // (With the route_short_name label; RTS_025 pattern. The old feed summary showed one route_id;
    // now each route without a shape is visible individually.)
    {
        // Skip routes with only Flex zone/group trips (same rationale as DQ_006).
        let zone_only = zone_only_trip_ids(records);
        let mut route_has_shape: HashMap<&str, bool> = HashMap::new();
        for t in &records.trips {
            if zone_only.contains(t.trip_id.as_str()) {
                continue;
            }
            let entry = route_has_shape.entry(ti_rem.route_id(t)).or_insert(false);
            if t.shape_idx != 0 {
                *entry = true;
            }
        }
        // Only routes with trips but no shapes at all (another rule handles routes without trips).
        for r in &records.routes {
            if route_has_shape.get(r.route_id.as_str()) == Some(&false) {
                let label = r
                    .route_short_name
                    .as_deref()
                    .filter(|s| !s.is_empty())
                    .unwrap_or(r.route_id.as_str());
                notices.push(k6_notice(
                    ctr,
                    "RTS_017",
                    EntityType::Route,
                    Some(r.route_id.to_string()),
                    Some(r.route_id.to_string()),
                    "routes.txt",
                    Some(r.line),
                    Some("route_id"),
                    None,
                    None,
                    format!("'{label}' hattının hiçbir seferinde shape_id tanımlı değil — harita gösterimi kısıtlı."),
                    "Bu hat için shapes.txt tanımlayın ve trips.txt'de shape_id atayın.",
                ));
            }
        }
    }

    // ── TRP_012: trip direction is inconsistent with other trips on the route ─
    // (direction_id is set but conflicts with some trips).
    {
        let mut route_dir_trips: HashMap<(&str, u32), u32> = HashMap::new();
        for t in &records.trips {
            if ti_rem.route_id(t).is_empty() {
                continue;
            }
            if let Some(dir) = t.direction_id {
                *route_dir_trips
                    .entry((ti_rem.route_id(t), dir))
                    .or_default() += 1;
            }
        }
        // Both directions exist on the route; only trips without direction_id → TRP_012.
        let routes_with_both_dirs: HashSet<&str> = {
            let mut r: HashMap<&str, HashSet<u32>> = HashMap::new();
            for t in &records.trips {
                if let Some(d) = t.direction_id {
                    r.entry(ti_rem.route_id(t)).or_default().insert(d);
                }
            }
            r.into_iter()
                .filter(|(_, dirs)| dirs.len() > 1)
                .map(|(rid, _)| rid)
                .collect()
        };
        // Aggregate at route level: how many trips on a bidirectional route lack direction_id.
        // Per-trip emission produced hundreds of identical rows for one route (the message is
        // already route-centered); one notice per route + count.
        let mut route_missing: HashMap<&str, (u32, u64)> = HashMap::new(); // route -> (missing count, first row)
        for t in &records.trips {
            if !routes_with_both_dirs.contains(ti_rem.route_id(t)) {
                continue;
            }
            if t.direction_id.is_none() {
                let e = route_missing
                    .entry(ti_rem.route_id(t))
                    .or_insert((0, t.line));
                e.0 += 1;
                if t.line < e.1 {
                    e.1 = t.line;
                }
            }
        }
        let mut rows: Vec<(&str, u32, u64)> = route_missing
            .into_iter()
            .map(|(r, (c, l))| (r, c, l))
            .collect();
        rows.sort_unstable_by(|a, b| a.0.cmp(b.0)); // deterministic order
        for (route_id, count, line) in rows {
            let label = route_short.get(route_id).copied().unwrap_or(route_id);
            let mut notice = k6_notice(
                ctr,
                "TRP_012",
                EntityType::Route,
                Some(route_id.to_string()),
                Some(route_id.to_string()),
                "trips.txt",
                Some(line),
                Some("direction_id"),
                Some(count.to_string()),
                None,
                format!("'{label}' kodlu hattın {count} seferinde yön bilgisi (direction_id) girilmemiş — hat çift yönlü."),
                "Bu seferlerin direction_id alanını 0 veya 1 olarak doldurun.",
            );
            notice
                .details
                .get_or_insert_with(Default::default)
                .insert("route_label".to_string(), label.to_string());
            notices.push(notice);
        }
    }

    // ── TRP_015: block_id groups no trips (one finding per FEED) ─────────────
    //
    // The block identity is defined by the SPEC: `(block_id, service_id)`. GTFS Schedule Reference,
    // `trips.block_id`: "A block consists of a single trip or many sequential trips made
    // using the same vehicle, defined by shared service days and block_id. A block_id may
    // have trips with different service days, making distinct blocks."
    //
    // 🔴 Two conclusions follow, and both disprove the old rule:
    //   1. A SINGLE-TRIP BLOCK IS VALID — the spec lists it as the first option. The old rule
    //      said "a block must contain at least 2 trips"; that was a spec-contrary norm and emitted
    //      1,549,904 notices per trip in the corpus (675 feeds, no MD equivalent).
    //   2. APPLYING the pair definition would amplify noise: the same block_id forms separate
    //      blocks on different service days, increasing single-trip blocks (14-feed sample,
    //      mdb-2727: 101,089 → 101,621).
    //
    // The only defensible remaining signal is feed-level: if NO block contains two trips,
    // `block_id` is effectively a copy of `trip_id` and carries no vehicle-continuity information.
    // Eight of 14 sample feeds were exactly 100% — the threshold is not arbitrary.
    // Overlapping trips belong to TRP_022; route_type inconsistency within a block belongs to TRP_024.
    {
        let mut block_count: HashMap<(&str, &str), u32> = HashMap::new();
        let mut trips_with_block: u64 = 0;
        for t in &records.trips {
            if let Some(bid) = ti_rem.block_id(t).filter(|b| !b.is_empty()) {
                trips_with_block += 1;
                *block_count.entry((bid, ti_rem.service_id(t))).or_default() += 1;
            }
        }
        let grouped = block_count.values().filter(|&&c| c > 1).count();
        if trips_with_block > 0 && grouped == 0 {
            let mut notice = k6_notice(
                ctr,
                "TRP_015",
                EntityType::Feed,
                None,
                None,
                "trips.txt",
                None,
                Some("block_id"),
                Some(block_count.len().to_string()),
                None,
                format!(
                    "block_id taşıyan {trips_with_block} seferin her biri kendi bloğunda tek — {} blok kodunun hiçbiri iki seferi gruplamıyor, alan fiilen trip_id kopyası.",
                    block_count.len()
                ),
                "block_id'yi aynı araçla ardışık yapılan seferlere ORTAK verin; her sefere ayrı kod vermek alanı bilgisiz bırakır (tek seferlik blok spec'e göre geçerlidir, bulgu onun için değildir).",
            );
            notice
                .details
                .get_or_insert_with(Default::default)
                .insert("trips_with_block".to_string(), trips_with_block.to_string());
            notices.push(notice);
        }
    }

    // ── STP_020: physical stop never used in stop_times ─────────────────────
    {
        let _t20 = Timer::start("K6::rem::stp_020");
        let mut used_stops: FxHashSet<&str> = records
            .stop_times_index
            .stop_id_set
            .iter()
            .map(|s| s.as_str())
            .collect();
        // GTFS-Flex: stop_times can also access a stop through `location_group_id`; those rows
        // have an empty `stop_id`. Stops belonging to the group count as SERVED — otherwise
        // every stop in a pure Flex feed would be "unused" (measured in the Swiss ODV feed:
        // all 1,259 of 1,259 stops, 85% of all feed findings).
        used_stops.extend(
            records
                .location_group_stops
                .iter()
                .filter(|r| !r.stop_id.is_empty())
                .map(|r| r.stop_id.as_str()),
        );

        for stop in &records.stops {
            if stop.stop_id.is_empty() {
                continue;
            }
            // Only physical stops (location_type=0 or null); exclude parent stations/entrances, etc.
            if stop.location_type.unwrap_or(0) != 0 {
                continue;
            }
            if !used_stops.contains(stop.stop_id.as_str()) {
                notices.push(k6_notice(
                    ctr,
                    "STP_020",
                    EntityType::Stop,
                    Some(stop.stop_id.clone()),
                    Some(stop.stop_id.clone()),
                    "stops.txt",
                    Some(stop.line),
                    None,
                    None,
                    None,
                    {
                        let name = stop.stop_name.as_deref().unwrap_or(stop.stop_id.as_str());
                        format!(
                            "'{}' durağı (kod: '{}') hiçbir seferde kullanılmıyor.",
                            name, stop.stop_id
                        )
                    },
                    "Durağı kaldırın veya ilgili bir sefere ekleyin.",
                ));
            }
        }
    }

    // ── SHP_017 / SHP_022 shared infrastructure ─────────────────────────────
    // trip_id → shape_id and shape_id → arc arrays are used by both rules.
    let trip_shape_local: HashMap<&str, &str> = records
        .trips
        .iter()
        .filter_map(|t| ti_rem.shape_id(t).map(|s| (t.trip_id.as_str(), s)))
        .collect();
    let mut shape_cum: FxHashMap<&str, Vec<f64>> = FxHashMap::default();
    // Make SHP_016/017 representative trip and notice order independent of iteration: FxHashMap
    // traversal differs between 32-bit WASM and 64-bit native (the same feed showed web `1150B`,
    // CLI `0850B`). trip_id order is the same on both platforms.
    let mut trips_by_id: Vec<(&str, &[CompactStopTime])> =
        idx.by_trip.iter().map(|(&t, &st)| (t, st)).collect();
    trips_by_id.sort_unstable_by_key(|&(t, _)| t);

    // ── SHP_016: shape is drawn completely backwards (variant-aware pre-pass) ─
    // If the first stop projects into the second half of the shape, the shape is reversed.
    // To avoid variant blindness: fire only when the MINIMUM first-stop projection of ALL trips
    // using the shape is beyond its midpoint (no variant starts near the beginning). One forward/
    // short-turn variant or bidirectional use (XFL_013's subject) suppresses it. Deterministic (min is order-independent).
    let mut reversed_shapes: FxHashSet<&str> = FxHashSet::default();
    {
        let _t16 = Timer::start("K6::rem::shp_016");
        // shape_id → (min_first_arc, total_km, route, trip_id, first_stop)
        let mut agg: FxHashMap<&str, (f64, f64, String, String, String)> = FxHashMap::default();
        // 🔴 Simply SKIPPING a trip with an unreadable projection has the opposite effect: `agg`
        // keeps the MINIMUM projection per shape, and a low minimum means "a variant starts near the
        // beginning" — evidence that SUPPRESSES the rule. Dropping the uncertain trip drops that evidence
        // too and makes the shape fire MORE EASILY — measured: `mdb-1138` rose from 46 to 168.
        // Correct: make NO DECISION about the uncertain trip's shape.
        let mut unreliable: FxHashSet<&str> = FxHashSet::default();
        for &(trip_id, stimes) in &trips_by_id {
            let Some(&shape_id) = trip_shape_local.get(trip_id) else {
                continue;
            };
            let Some(pts) = shape_coords.get(shape_id) else {
                continue;
            };
            if pts.len() < 2 {
                continue;
            }
            let cum = shape_cum.entry(shape_id).or_insert_with(|| {
                let mut c = Vec::with_capacity(pts.len());
                c.push(0.0_f64);
                for i in 1..pts.len() {
                    c.push(c[i - 1] + haversine_km(pts[i - 1].0, pts[i - 1].1, pts[i].0, pts[i].1));
                }
                c
            });
            let total = cum.last().copied().unwrap_or(0.0);
            if total <= 0.1 {
                continue;
            }
            // Circular shape (start≈end) → reverse check is meaningless, skip.
            let ring = haversine_km(
                pts[0].0,
                pts[0].1,
                pts.last().unwrap().0,
                pts.last().unwrap().1,
            );
            if ring < total * 0.10 && ring < 1.0 {
                continue;
            }
            // by_trip slices are ordered by stop_sequence → first element = first stop.
            let Some(first_st) = stimes.first() else {
                continue;
            };
            let Some(&(slat, slon)) = stop_coords.get(idx.stop_id_of(first_st)) else {
                continue;
            };
            let (first_arc, first_dist, head_dist) = project_arc_detail(pts, cum, slat, slon);
            // 🔴 TWO GUARDS — both follow the principle "do not claim what you cannot measure".
            // Thresholded using data from the 14th corpus run over 29 feeds / 1,009 findings.
            //
            // (B) A projection VERY FAR from the stop is not evidence of direction. In `tdg-83210`,
            // the selected projection was 2.561 KM from the stop — the stop coordinate was broken and
            // `GEO_016`/`SHP_014` reported the root defect; SHP_016's finding was derived noise.
            // This branch produced 184 findings (18.2%) in the sample.
            if first_dist > 0.5 {
                unreliable.insert(shape_id);
                continue;
            }
            // (A) WHEN A SHAPE CROSSES ITSELF, the first stop maps to two locations and the choice is
            // determined by meters → direction is unreadable. DART shape 58782: selected 6.8 m, candidate
            // at the shape start 10.0 m (difference 3.1 m); difference was 0.0 m in `mdb-1138`/`mdb-1154`.
            // Measured threshold: the smallest difference among REAL reversed cases after the guard was 61 m,
            // median 5,744 m — a 50 m limit removes ambiguity without cutting real cases.
            if head_dist.is_some_and(|hd| hd - first_dist < 0.05) {
                unreliable.insert(shape_id);
                continue;
            }
            let route = trip_to_route_rem.get(trip_id).copied().unwrap_or(trip_id);
            let entry = agg.entry(shape_id).or_insert_with(|| {
                (
                    f64::INFINITY,
                    total,
                    String::new(),
                    String::new(),
                    String::new(),
                )
            });
            if first_arc < entry.0 {
                entry.0 = first_arc;
                entry.2 = route.to_string();
                entry.3 = trip_id.to_string();
                entry.4 = idx.stop_id_of(first_st).to_string();
            }
        }
        let mut shape_ids: Vec<&str> = agg.keys().copied().collect();
        shape_ids.sort_unstable();
        for shape_id in shape_ids {
            if unreliable.contains(shape_id) {
                continue;
            }
            let (min_arc, total, route, trip_id, first_stop) = &agg[shape_id];
            if !min_arc.is_finite() || *min_arc <= *total * 0.5 {
                continue;
            }
            reversed_shapes.insert(shape_id);
            let mut n016 = k6_notice(
                ctr, "SHP_016",
                EntityType::Shape,
                Some(shape_id.to_string()),
                Some(shape_id.to_string()),
                "trips.txt", None,
                Some("shape_id"),
                Some(shape_id.to_string()),
                None,
                format!(
                    "'{route}' hattının '{shape_id}' güzergahı ters yönde çizilmiş — bu shape'i kullanan tüm seferlerin ilk durağı shape'in başından değil sonuna yakın projekte oluyor (en yakını {:.0}m / {:.0}m).",
                    min_arc * 1000.0, total * 1000.0
                ),
                "shape_pt_sequence sırasını tersine çevirin veya bu yön için ayrı bir shape_id tanımlayın.",
            );
            let mut d = std::collections::BTreeMap::new();
            d.insert("first_stop".to_string(), first_stop.clone());
            d.insert("trip_id".to_string(), trip_id.clone());
                n016.details = Some(d.into());
            notices.push(n016);
        }
    }

    // ── SHP_017: trip stop order conflicts with shape projection ─────────────
    // For each trip, arc-length projections of stops in stop_sequence order must increase monotonically.
    {
        let _t17 = Timer::start("K6::rem::shp_017");
        // (shape_id, stop_id) → arc — multiple trips may share the same pair.
        let mut arc_cache: FxHashMap<(&str, &str), f64> = FxHashMap::default();
        // Emit one notice for each (shape_id, problem_stop_id) pair
        // (all trips on the same route should not repeat the same shape issue).
        let mut shp017_seen: FxHashSet<(&str, &str)> = FxHashSet::default();

        for &(trip_id, stimes) in &trips_by_id {
            let Some(&shape_id) = trip_shape_local.get(trip_id) else {
                continue;
            };
            let Some(pts) = shape_coords.get(shape_id) else {
                continue;
            };
            if pts.len() < 2 {
                continue;
            }

            let cum = shape_cum.entry(shape_id).or_insert_with(|| {
                let mut c = Vec::with_capacity(pts.len());
                c.push(0.0_f64);
                for i in 1..pts.len() {
                    c.push(c[i - 1] + haversine_km(pts[i - 1].0, pts[i - 1].1, pts[i].0, pts[i].1));
                }
                c
            });

            let mut sorted: Vec<&CompactStopTime> = stimes.iter().collect();
            sorted.sort_by_key(|st| st.stop_sequence().unwrap_or(0));

            // Circular shape detection: if shape start and end are close
            // (loop/circular route), arc monotonicity is meaningless → skip.
            {
                let shape_start = pts[0];
                let shape_end = *pts.last().unwrap();
                let ring_dist_km =
                    haversine_km(shape_start.0, shape_start.1, shape_end.0, shape_end.1);
                let shape_total_km = cum.last().copied().unwrap_or(1.0);
                // Circular when start-to-end distance is below 10% of total length AND below 1 km.
                if ring_dist_km < shape_total_km * 0.10 && ring_dist_km < 1.0 {
                    continue;
                }
            }

            // Reversed shape (detected in the SHP_016 pre-pass): suppress SHP_017
            // "order broken" noise for this shape (SHP_016 is the clearer signal).
            if reversed_shapes.contains(shape_id) {
                continue;
            }

            // When stop_times has shape_dist_traveled, geometric projection is unnecessary.
            // Use it as the authoritative source when present for every stop; when mixed, skip
            // the geometric check to avoid unit mismatch.
            let all_have_sdt = sorted.iter().all(|st| st.shape_dist_traveled().is_some());
            let any_have_sdt = sorted.iter().any(|st| st.shape_dist_traveled().is_some());
            if any_have_sdt && !all_have_sdt {
                continue;
            }

            // Is shape_dist_traveled in meters or kilometers? Compare with total shape length.
            let sdt_to_km: f64 = if all_have_sdt {
                let sdt_max = sorted
                    .iter()
                    .filter_map(|st| st.shape_dist_traveled())
                    .fold(0.0_f64, f64::max);
                let shape_total_km = cum.last().copied().unwrap_or(1.0);
                if sdt_max > shape_total_km * 100.0 {
                    0.001
                } else {
                    1.0
                }
            } else {
                1.0
            };

            let mut prev_arc = -1.0_f64;
            let mut prob_idx: Option<usize> = None;
            for (i, st) in sorted.iter().enumerate() {
                let arc = if all_have_sdt {
                    st.shape_dist_traveled().unwrap() * sdt_to_km
                } else {
                    let Some(&(slat, slon)) = stop_coords.get(idx.stop_id_of(st)) else {
                        continue;
                    };
                    *arc_cache
                        .entry((shape_id, idx.stop_id_of(st)))
                        .or_insert_with(|| project_arc_km(pts, cum, slat, slon))
                };
                // 500 m tolerance: filters projection noise and S-curve false positives.
                if arc < prev_arc - 0.500 {
                    prob_idx = Some(i);
                    break;
                }
                if arc > prev_arc {
                    prev_arc = arc;
                }
            }
            if let Some(pi) = prob_idx {
                // Backtrack filter: if later stops after prob_idx exceed arc prev_arc,
                // the shape turns back and continues — false positive, skip.
                let recovers = sorted[pi + 1..].iter().take(3).any(|nxt| {
                    let narc = if all_have_sdt {
                        nxt.shape_dist_traveled().unwrap_or(0.0) * sdt_to_km
                    } else {
                        // Because the break occurs at prob_idx, later stops are not in the cache yet;
                        // project the arc directly. (Relying on the cache returns 0.0 → recovery never fires
                        // on the geometric path = false positive.)
                        match stop_coords.get(idx.stop_id_of(nxt)) {
                            Some(&(slat, slon)) => project_arc_km(pts, cum, slat, slon),
                            None => 0.0,
                        }
                    };
                    narc > prev_arc
                });
                if recovers {
                    continue;
                }

                let st = sorted[pi];
                // Skip if a notice was already emitted for the same (shape_id, problem_stop_id) pair.
                if !shp017_seen.insert((shape_id, idx.stop_id_of(st))) {
                    continue;
                }
                let route = trip_to_route_rem.get(trip_id).copied().unwrap_or(trip_id);
                let dep = trip_first_dep
                    .get(trip_id)
                    .map(|(h, m, _)| format!("{h:02}:{m:02}"))
                    .unwrap_or_default();
                let dep_infix = if dep.is_empty() {
                    String::new()
                } else {
                    format!(" {dep}")
                };
                let sname = stop_names
                    .get(idx.stop_id_of(st))
                    .copied()
                    .unwrap_or(idx.stop_id_of(st));
                let arc = if all_have_sdt {
                    st.shape_dist_traveled().unwrap() * sdt_to_km
                } else {
                    arc_cache
                        .get(&(shape_id, idx.stop_id_of(st)))
                        .copied()
                        .unwrap_or(0.0)
                };
                let mut notice = k6_notice(
                    ctr,
                    "SHP_017",
                    EntityType::Trip,
                    Some(trip_id.to_string()),
        // Registry identity field `shape_id`: roots (XFL_013, SHP_002/006,
        // TRP_004) are shape-scoped; writing trip_id meant `blocks` never matched.
                    Some(shape_id.to_string()),
                    "stop_times.txt",
                    Some(st.line as u64),
                    Some("stop_sequence"),
                    Some(st.stop_sequence().unwrap_or(0).to_string()),
                    None,
                    format!(
                        "'{}' hattının{dep_infix} seferinde '{}' (kod: '{}') {}. sıradaki durak güzergah sırası bozuk — başlangıca uzaklığı önceki durağın gerisinde ({:.0}m < {:.0}m).",
                        route, sname, idx.stop_id_of(st), st.stop_sequence().unwrap_or(0),
                        arc * 1000.0, prev_arc * 1000.0,
                    ),
                    "Projeksiyon döngü veya aynı yolun tekrar kullanımında belirsiz olabilir. Haritada doğrulayın; desen bilinçliyse işlem gerekmez.",
                );
                // Map context: ±3 neighboring stops + shape_id + sequence numbers.
                let ctx_b: Vec<&str> = sorted[..pi]
                    .iter()
                    .rev()
                    .take(3)
                    .rev()
                    .map(|s| idx.stop_id_of(s))
                    .collect();
                let ctx_a: Vec<&str> = sorted[pi + 1..]
                    .iter()
                    .take(3)
                    .map(|s| idx.stop_id_of(s))
                    .collect();
                let seq_b: Vec<String> = sorted[..pi]
                    .iter()
                    .rev()
                    .take(3)
                    .rev()
                    .map(|s| s.stop_sequence().unwrap_or(0).to_string())
                    .collect();
                let seq_a: Vec<String> = sorted[pi + 1..]
                    .iter()
                    .take(3)
                    .map(|s| s.stop_sequence().unwrap_or(0).to_string())
                    .collect();
                let mut det = std::collections::BTreeMap::new();
                det.insert("ctx_b".to_string(), ctx_b.join(","));
                det.insert("ctx_a".to_string(), ctx_a.join(","));
                det.insert("seq_b".to_string(), seq_b.join(","));
                det.insert("seq_a".to_string(), seq_a.join(","));
                det.insert("shape_id".to_string(), shape_id.to_string());
                det.insert("bad_stop".to_string(), idx.stop_id_of(st).to_string());
                notice.details = Some(det.into());
                notices.push(notice);
            }
        }
    }

    // ── SHP_014: first/last stop is far from the route endpoint (variant-aware) ─
    // A shape may be shared by multiple trip variants (short-turn, different terminal). Fire
    // only when the shape endpoint is near NONE of the variant endpoints: min(dist) per shape > threshold.
    // Report the nearest variant; on ties, the smallest trip_id. by_trip FxHashMap order differs
    // between 32/64-bit, and using it as a tiebreak caused different representative trips on wasm32/native/wasm64.
    {
        let _t14 = Timer::start("K6::rem::shp_014");
        // shape_id → (min_dist_km, problem_stop, trip_id)
        let mut start_agg: FxHashMap<&str, (f64, String, String)> = FxHashMap::default();
        let mut end_agg: FxHashMap<&str, (f64, String, String)> = FxHashMap::default();

        for (&trip_id, stimes) in &idx.by_trip {
            let Some(&shape_id) = trip_shape_local.get(trip_id) else {
                continue;
            };
            let Some(pts) = shape_coords.get(shape_id) else {
                continue;
            };
            if pts.len() < 2 {
                continue;
            }
            let shape_start = pts[0];
            let shape_end = *pts.last().unwrap();

            if let Some(first_st) = stimes.first() {
                if let Some(&(slat, slon)) = stop_coords.get(idx.stop_id_of(first_st)) {
                    let d_km = haversine_km(slat, slon, shape_start.0, shape_start.1);
                    let e = start_agg
                        .entry(shape_id)
                        .or_insert_with(|| (f64::INFINITY, String::new(), String::new()));
                    if d_km < e.0 || (d_km == e.0 && trip_id < e.2.as_str()) {
                        e.0 = d_km;
                        e.1 = idx.stop_id_of(first_st).to_string();
                        e.2 = trip_id.to_string();
                    }
                }
            }

            if let Some(last_st) = stimes.last() {
                if let Some(&(slat, slon)) = stop_coords.get(idx.stop_id_of(last_st)) {
                    let d_km = haversine_km(slat, slon, shape_end.0, shape_end.1);
                    let e = end_agg
                        .entry(shape_id)
                        .or_insert_with(|| (f64::INFINITY, String::new(), String::new()));
                    if d_km < e.0 || (d_km == e.0 && trip_id < e.2.as_str()) {
                        e.0 = d_km;
                        e.1 = idx.stop_id_of(last_st).to_string();
                        e.2 = trip_id.to_string();
                    }
                }
            }
        }

        let mut start_ids: Vec<&str> = start_agg.keys().copied().collect();
        start_ids.sort_unstable();
        for shape_id in start_ids {
            let (min_km, stop_id, trip_id) = &start_agg[shape_id];
            let threshold_km = stop_shape_threshold_m(shape_id, rail_shapes, config) / 1000.0;
            if !min_km.is_finite() || *min_km <= threshold_km {
                continue;
            }
            let dist_m = *min_km * 1000.0;
            let sname = stop_names
                .get(stop_id.as_str())
                .copied()
                .unwrap_or(stop_id.as_str());
            let prefix = shp_route_prefix(
                shape_route_labels
                    .get(shape_id)
                    .map(|v| v.as_slice())
                    .unwrap_or(&[]),
            );
            let mut n = k6_notice(
                ctr, "SHP_014", EntityType::Shape,
                Some(shape_id.to_string()), Some(shape_id.to_string()),
                "shapes.txt", None, Some("shape_pt_sequence"),
                Some(format!("{dist_m:.0}m")),
                Some(format!("≤ {:.0}m", threshold_km * 1000.0)),
                format!(
                    "{prefix}'{}' güzergah şeklinin başlangıç noktası, en yakın seferin ilk durağı '{}' (kod: '{}') konumundan {dist_m:.0}m uzakta.",
                    shape_id, sname, stop_id
                ),
                "Bu uzantı bilinçliyse işlem gerekmez; değilse shape başlangıcını ve ilk durağı doğrulayın.",
            );
            let mut d = std::collections::BTreeMap::new();
            d.insert("problem_stop".to_string(), stop_id.clone());
            d.insert("endpoint".to_string(), "start".to_string());
            d.insert("trip_id".to_string(), trip_id.clone());
                n.details = Some(d.into());
            notices.push(n);
        }

        let mut end_ids: Vec<&str> = end_agg.keys().copied().collect();
        end_ids.sort_unstable();
        for shape_id in end_ids {
            let (min_km, stop_id, trip_id) = &end_agg[shape_id];
            let threshold_km = stop_shape_threshold_m(shape_id, rail_shapes, config) / 1000.0;
            if !min_km.is_finite() || *min_km <= threshold_km {
                continue;
            }
            let dist_m = *min_km * 1000.0;
            let sname = stop_names
                .get(stop_id.as_str())
                .copied()
                .unwrap_or(stop_id.as_str());
            let prefix = shp_route_prefix(
                shape_route_labels
                    .get(shape_id)
                    .map(|v| v.as_slice())
                    .unwrap_or(&[]),
            );
            let mut n = k6_notice(
                ctr, "SHP_014", EntityType::Shape,
                Some(shape_id.to_string()), Some(shape_id.to_string()),
                "shapes.txt", None, Some("shape_pt_sequence"),
                Some(format!("{dist_m:.0}m")),
                Some(format!("≤ {:.0}m", threshold_km * 1000.0)),
                format!(
                    "{prefix}'{}' güzergah şeklinin bitiş noktası, en yakın seferin son durağı '{}' (kod: '{}') konumundan {dist_m:.0}m uzakta.",
                    shape_id, sname, stop_id
                ),
                "Bu uzantı bilinçliyse işlem gerekmez; değilse shape bitişini ve son durağı doğrulayın.",
            );
            let mut d = std::collections::BTreeMap::new();
            d.insert("problem_stop".to_string(), stop_id.clone());
            d.insert("endpoint".to_string(), "end".to_string());
            d.insert("trip_id".to_string(), trip_id.clone());
                n.details = Some(d.into());
            notices.push(n);
        }
    }

    // ── STP_029: stop too far from parent_station ────────────────────────────
    {
        let _t29 = Timer::start("K6::rem::stp_029");
        let threshold_m = config.stop_far_from_parent_m;

        // stop_id → (lat, lon)
        let stop_coords: HashMap<&str, (f64, f64)> = records
            .stops
            .iter()
            .filter_map(|s| {
                recoverable_stop_coord(s).map(|(la, lo)| (s.stop_id.as_str(), (la, lo)))
            })
            .collect();

        for stop in &records.stops {
            // Only location_type 0 or undefined (physical stops).
            if stop.location_type.unwrap_or(0) != 0 {
                continue;
            }
            let parent_id = match stop.row.get("parent_station") {
                Some(p) if !p.trim().is_empty() => p.trim(),
                _ => continue,
            };
            let (slat, slon) = match recoverable_stop_coord(stop) {
                Some(c) => c,
                None => continue,
            };
            let (plat, plon) = match stop_coords.get(parent_id) {
                Some(&c) => c,
                None => continue,
            };
            let dist_m = haversine_km(slat, slon, plat, plon) * 1000.0;
            if dist_m > threshold_m {
                let sname = stop.stop_name.as_deref().unwrap_or(stop.stop_id.as_str());
                let mut notice = k6_notice(
                    ctr,
                    "STP_029",
                    EntityType::Stop,
                    Some(stop.stop_id.clone()),
                    Some(stop.stop_id.clone()),
                    "stops.txt",
                    Some(stop.line),
                    Some("parent_station"),
                    Some(format!("{dist_m:.0}m")),
                    Some(format!("≤ {threshold_m:.0}m")),
                    format!(
                        "'{}' kodlu '{}' durağı üst istasyonundan ({}) {dist_m:.0}m uzakta (eşik: {threshold_m:.0}m).",
                        stop.stop_id, sname, parent_id
                    ),
                    "Durak koordinatlarını veya parent_station referansını düzeltin.",
                );
                let mut det = std::collections::BTreeMap::new();
                det.insert("parent_id".to_string(), parent_id.to_string());
                notice.details = Some(det.into());
                notices.push(notice);
            }
        }
    }

    // ── STP_030: parent station with no children (location_type=1) ───────────
    {
        let _t30 = Timer::start("K6::rem::stp_030");

        // Which station_ids have PLATFORMS? (location_type 0 or empty).
        //
        // Count platforms only: `parent_station` is also written by records such as entrance/
        // exit (2) and generic node (3), but trips stop only at PLATFORMS. A station with only
        // entrances and no platform is unusable — the card's decision also says it must be used
        // by at least one stop/platform.
        //
        // Without the filter every child was counted → 250-feed corpus, mdb-2933: all 6 children
        // of `MTR-POA` (Po Lam) station were location_type=2 (entrance), with NO platform; we stayed
        // silent while MobilityData flagged `unused_station`.
        let stations_with_children: HashSet<&str> = records
            .stops
            .iter()
            .filter(|s| matches!(s.location_type, None | Some(0)))
            .filter_map(|s| {
                s.row
                    .get("parent_station")
                    .map(|p| p.trim())
                    .filter(|p| !p.is_empty())
            })
            .collect();

        for stop in &records.stops {
            if stop.location_type != Some(1) {
                continue;
            }
            if stop.stop_id.is_empty() {
                continue;
            }
            if !stations_with_children.contains(stop.stop_id.as_str()) {
                let sname = stop.stop_name.as_deref().unwrap_or(stop.stop_id.as_str());
                notices.push(k6_notice(
                    ctr,
                    "STP_030",
                    EntityType::Stop,
                    Some(stop.stop_id.clone()),
                    Some(stop.stop_id.clone()),
                    "stops.txt",
                    Some(stop.line),
                    None,
                    None,
                    None,
                    format!(
                        "'{}' kodlu '{}' istasyonunun hiç peronu yok — hiçbir durak (location_type=0) \
                         onu parent_station olarak kullanmıyor; yalnız giriş/düğüm kayıtları bağlı olabilir.",
                        stop.stop_id, sname
                    ),
                    "İstasyona bağlı peronlarda (location_type=0) parent_station alanını doldurun veya istasyonu kaldırın.",
                ));
            }
        }
    }

    // B4: scan stop_headsign's 36 MB once for DQ_018+DQ_019 (previously twice).
    // Collect raw (line, value); mint notices below at their original block locations →
    // emission order, ctr, id, and content remain identical (only scans change 2→1).
    type HeadSignSamples = (Vec<(u64, String)>, Vec<(u64, String)>);
    let (dq_caps_hs, dq_lower_hs): HeadSignSamples = {
        let mut caps: Vec<(u64, String)> = Vec::new();
        let mut lower: Vec<(u64, String)> = Vec::new();
        let mut seen_caps: std::collections::HashSet<&str> = std::collections::HashSet::new();
        let mut seen_lower: std::collections::HashSet<&str> = std::collections::HashSet::new();
        for stops in records.stop_times_index.iter_stops() {
            for st in stops {
                if let Some(hs) = records.stop_times_index.stop_headsign_of(st) {
                    let s = hs.as_str();
                    if !seen_caps.contains(s) && is_all_caps(s) {
                        seen_caps.insert(s);
                        caps.push((st.line as u64, s.to_string()));
                    }
                    if !seen_lower.contains(s) && is_all_lower(s) {
                        seen_lower.insert(s);
                        lower.push((st.line as u64, s.to_string()));
                    }
                }
            }
        }
        (caps, lower)
    };

    // ── DQ_018: recommended text fields are all uppercase (mixed_case_recommended_field) ─
    {
        let _t18 = Timer::start("K6::rem::dq_018");

        // stop_name
        for stop in &records.stops {
            if stop.stop_id.is_empty() {
                continue;
            }
            if let Some(name) = stop.stop_name.as_deref().filter(|s| is_all_caps(s)) {
                notices.push(k6_notice(
                    ctr,
                    "DQ_018",
                    EntityType::Stop,
                    Some(stop.stop_id.clone()),
                    Some(stop.stop_id.clone()),
                    "stops.txt",
                    Some(stop.line),
                    Some("stop_name"),
                    Some(name.to_string()),
                    None,
                    format!(
                        "'{}' durağının adı tamamen büyük harf: '{name}'.",
                        stop.stop_id
                    ),
                    "Durak adını düzgün harf kuralıyla yazın (ör. 'Merkez İstasyon').",
                ));
            }
        }

        // route_long_name
        for route in &records.routes {
            if route.route_id.is_empty() {
                continue;
            }
            if let Some(name) = route.route_long_name.as_deref().filter(|s| is_all_caps(s)) {
                let label = route
                    .route_short_name
                    .as_deref()
                    .filter(|s| !s.is_empty())
                    .unwrap_or(route.route_id.as_str());
                notices.push(k6_notice(
                    ctr,
                    "DQ_018",
                    EntityType::Route,
                    Some(route.route_id.clone()),
                    Some(route.route_id.clone()),
                    "routes.txt",
                    Some(route.line),
                    Some("route_long_name"),
                    Some(name.to_string()),
                    None,
                    format!(
                        "'{}' hattının uzun adı tamamen büyük harf: '{name}'.",
                        label
                    ),
                    "Hat adını düzgün harf kuralıyla yazın.",
                ));
            }
        }

        // route_desc
        for route in &records.routes {
            if route.route_id.is_empty() {
                continue;
            }
            if let Some(desc) = route.route_desc.as_deref().filter(|s| is_all_caps(s)) {
                let label = route
                    .route_short_name
                    .as_deref()
                    .filter(|s| !s.is_empty())
                    .unwrap_or(route.route_id.as_str());
                notices.push(k6_notice(
                    ctr,
                    "DQ_018",
                    EntityType::Route,
                    Some(route.route_id.clone()),
                    Some(route.route_id.clone()),
                    "routes.txt",
                    Some(route.line),
                    Some("route_desc"),
                    Some(desc.to_string()),
                    None,
                    format!(
                        "'{}' hattının açıklaması tamamen büyük harf: '{desc}'.",
                        label
                    ),
                    "Hat açıklamasını düzgün harf kuralıyla yazın.",
                ));
            }
        }

        // trip_headsign
        for trip in &records.trips {
            if trip.trip_id.is_empty() {
                continue;
            }
            if let Some(hs) = ti_rem.headsign(trip).filter(|s| is_all_caps(s)) {
                notices.push(k6_notice(
                    ctr,
                    "DQ_018",
                    EntityType::Trip,
                    Some(trip.trip_id.to_string()),
                    Some(trip.trip_id.to_string()),
                    "trips.txt",
                    Some(trip.line),
                    Some("trip_headsign"),
                    Some(hs.to_string()),
                    None,
                    format!(
                        "'{}' seferinin yön adı tamamen büyük harf: '{hs}'.",
                        trip.trip_id
                    ),
                    "Yön adını düzgün harf kuralıyla yazın.",
                ));
            }
        }

        // stop_headsign (B4: caps values collected by the single scan — same order/content/ctr).
        for (line, s) in &dq_caps_hs {
            notices.push(k6_notice(
                ctr,
                "DQ_018",
                EntityType::Row,
                None,
                None,
                "stop_times.txt",
                Some(*line),
                Some("stop_headsign"),
                Some(s.clone()),
                None,
                format!("stop_headsign değeri tamamen büyük harf: '{s}'."),
                "Yön adını düzgün harf kuralıyla yazın.",
            ));
        }

    }

    // ── DQ_019: recommended fields are all lowercase (mixed_case_recommended_field) ─
    {
        let _t19 = Timer::start("K6::rem::dq_019");
        for stop in &records.stops {
            if stop.stop_id.is_empty() {
                continue;
            }
            if let Some(name) = stop.stop_name.as_deref().filter(|s| is_all_lower(s)) {
                notices.push(k6_notice(
                    ctr,
                    "DQ_019",
                    EntityType::Stop,
                    Some(stop.stop_id.clone()),
                    Some(stop.stop_id.clone()),
                    "stops.txt",
                    Some(stop.line),
                    Some("stop_name"),
                    Some(name.to_string()),
                    None,
                    format!(
                        "'{}' durağının adı tamamen küçük harf: '{name}'.",
                        stop.stop_id
                    ),
                    "Durak adını başlık harfiyle yazın (ör. 'Merkez İstasyon').",
                ));
            }
        }
        for route in &records.routes {
            if route.route_id.is_empty() {
                continue;
            }
            if let Some(name) = route.route_long_name.as_deref().filter(|s| is_all_lower(s)) {
                notices.push(k6_notice(
                    ctr,
                    "DQ_019",
                    EntityType::Route,
                    Some(route.route_id.clone()),
                    Some(route.route_id.clone()),
                    "routes.txt",
                    Some(route.line),
                    Some("route_long_name"),
                    Some(name.to_string()),
                    None,
                    format!(
                        "'{}' hattının uzun adı tamamen küçük harf: '{name}'.",
                        route.route_id
                    ),
                    "Hat adını başlık harfiyle yazın.",
                ));
            }
        }
        for route in &records.routes {
            if route.route_id.is_empty() {
                continue;
            }
            if let Some(desc) = route.route_desc.as_deref().filter(|s| is_all_lower(s)) {
                notices.push(k6_notice(
                    ctr,
                    "DQ_019",
                    EntityType::Route,
                    Some(route.route_id.clone()),
                    Some(route.route_id.clone()),
                    "routes.txt",
                    Some(route.line),
                    Some("route_desc"),
                    Some(desc.to_string()),
                    None,
                    format!(
                        "'{}' hattının açıklaması tamamen küçük harf: '{desc}'.",
                        route.route_id
                    ),
                    "Hat açıklamasını başlık harfiyle yazın.",
                ));
            }
        }
        for trip in &records.trips {
            if trip.trip_id.is_empty() {
                continue;
            }
            if let Some(hs) = ti_rem.headsign(trip).filter(|s| is_all_lower(s)) {
                notices.push(k6_notice(
                    ctr,
                    "DQ_019",
                    EntityType::Trip,
                    Some(trip.trip_id.to_string()),
                    Some(trip.trip_id.to_string()),
                    "trips.txt",
                    Some(trip.line),
                    Some("trip_headsign"),
                    Some(hs.to_string()),
                    None,
                    format!(
                        "'{}' seferinin yön adı tamamen küçük harf: '{hs}'.",
                        trip.trip_id
                    ),
                    "Yön adını başlık harfiyle yazın.",
                ));
            }
        }

        // stop_headsign (B4: lowercase values collected by the single scan — same order/content/ctr).
        for (line, s) in &dq_lower_hs {
            notices.push(k6_notice(
                ctr,
                "DQ_019",
                EntityType::Row,
                None,
                None,
                "stop_times.txt",
                Some(*line),
                Some("stop_headsign"),
                Some(s.clone()),
                None,
                format!("stop_headsign değeri tamamen küçük harf: '{s}'."),
                "Yön adını başlık harfiyle yazın.",
            ));
        }

    }

    // ── DQ_020: recommended field missing/empty (missing_recommended_field) ─
    // Field-scoped (registry: VS, Field): one notice for trips.txt when trip_headsign is empty
    // on most/all rows. The old behavior fired for every trip → 33,642 cap-busting findings
    // on feeds such as LA Metro.
    {
        let _t20 = Timer::start("K6::rem::dq_020");
        let total = records
            .trips
            .iter()
            .filter(|t| !t.trip_id.is_empty())
            .count();
        let missing = records
            .trips
            .iter()
            .filter(|t| !t.trip_id.is_empty())
            .filter(|t| {
                ti_rem
                    .headsign(t)
                    .map(|s| s.trim().is_empty())
                    .unwrap_or(true)
            })
            .count();
        if missing > 0 && total > 0 {
            notices.push(k6_notice(
                ctr, "DQ_020", EntityType::Feed,
                None, None, "trips.txt", None, Some("trip_headsign"),
                Some(format!("{missing}/{total}")), None,
                format!("trips.txt'te önerilen trip_headsign alanı {missing}/{total} satırda boş — yolcu bilgi sistemleri için doldurulması önerilir."),
                "trips.txt'teki trip_headsign sütununu doldurun.",
            ));
        }
    }

    // ── OPR_009: night service — one route summary instead of trip repeats ───
    {
        let _t09 = Timer::start("K6::rem::opr_009");
        let mut by_route: FxHashMap<&str, (u32, u32, u32)> = FxHashMap::default();
        let sds_sec = config.service_day_start_hour * 3600;
        for (&trip_id, stimes) in &idx.by_trip {
            if let Some(dep) = stimes.first().and_then(|s| s.departure_time()) {
                let dep_sec = dep.0 * 3600 + dep.1 * 60 + dep.2;
                if dep_sec >= 23 * 3600 || (sds_sec > 0 && dep_sec < sds_sec) {
                    let route = trip_to_route_rem.get(trip_id).copied().unwrap_or(trip_id);
                    let e = by_route.entry(route).or_insert((0, dep_sec, dep_sec));
                    e.0 += 1;
                    e.1 = e.1.min(dep_sec);
                    e.2 = e.2.max(dep_sec);
                }
            }
        }
        let mut routes: Vec<_> = by_route.into_iter().collect();
        routes.sort_by_key(|x| x.0);
        for (route, (count, min, max)) in routes {
            let hm = |s: u32| format!("{:02}:{:02}", s / 3600, s / 60 % 60);
            notices.push(k6_notice(
                ctr,
                "OPR_009",
                EntityType::Route,
                Some(route.to_string()),
                Some(route.to_string()),
                "stop_times.txt",
                None,
                Some("departure_time"),
                Some(format!("{count} trips, {}-{}", hm(min), hm(max))),
                None,
                format!(
                    "'{route}' hattında {count} gece seferi var (ilk kalkış aralığı {}-{}).",
                    hm(min),
                    hm(max)
                ),
                "Bu bilgi notu; gece servisleri için beklenen bir durumdur.",
            ));
        }
    }

    // ── OPR_017: trip covers a very short distance (< 100 m) ─────────────────
    {
        let _t17b = Timer::start("K6::rem::opr_017");
        const MIN_TRIP_KM: f64 = 0.1;
        for (&trip_id, stimes) in &idx.by_trip {
            let dist_km: Option<f64> = if let Some(&shape_id) = trip_shape_local.get(trip_id) {
                derived
                    .shape_geometry
                    .shapes
                    .get(shape_id)
                    .map(|s| s.total_length_km)
            } else {
                // Without a shape, sum route length across ADJACENT stops.
                //
                // 🔴 Previously only straight-line distance between the FIRST and LAST stop was measured;
                // on a CIRCULAR route (first stop == last stop), this is necessarily 0 and reported "0m"
                // even for a 20 km trip. Measured in the 13th corpus run: in Singapore LTA feed (mdb-3051,
                // no shapes.txt), 88,359 of 230,914 trips were circular and OPR_017 produced 92,354 findings
                // — 88% of all feed findings. Median actual route length was 20.9 km; NONE of 2,000 samples
                // was below 100 m. Corpus-wide: 479 feeds, 636,734 findings.
                //
                // Summing adjacent segments is also more accurate on straight routes (straight-line distance
                // always UNDERESTIMATES the real route), so the fix does more than rescue circular routes.
                let mut total = 0.0f64;
                let mut seen_any = false;
                for pair in stimes.windows(2) {
                    let (a, b) = (&pair[0], &pair[1]);
                    if let (Some(&(la1, lo1)), Some(&(la2, lo2))) = (
                        stop_coords.get(idx.stop_id_of(a)),
                        stop_coords.get(idx.stop_id_of(b)),
                    ) {
                        total += haversine_km(la1, lo1, la2, lo2);
                        seen_any = true;
                    }
                }
                // One-stop trip: no segment to measure → STM_033's subject, stay silent here.
                seen_any.then_some(total)
            };
            if let Some(d) = dist_km {
                // 🔴 ZERO does not mean "measured and zero"; it means "UNMEASURABLE" — and this is a
                // DISTANCE claim, so it cannot claim what it cannot measure. In the 14th corpus run
                // (2026-09-01), "0m" was the most common value among samples of the remaining 91,417 findings
                // (47 of 124 feeds). Three case classes were measured and ALL THREE have their real root cause
                // reported by ANOTHER rule already:
                //   · `tld-821`: trip stops twice at the same stop (departure = arrival platform), no intermediate
                //     stop → no segment to sum. `STM_035` reports 5 findings there, OPR_017 also 5 — we reported
                //     the EXACT same fact twice.
                //   · `mdb-1316`/`tfs-658`: 149 of 158 stops are `(0,0)` Null Island. Fully broken-coordinate
                //     trips sum to 0; `GEO_016` reports 149 findings there. The trip exists, but is UNMEASURABLE.
                //   · `ntd-10183`: same stop three times + `STM_033` (one-stop trip) = 37.
                // Truly short trips are unaffected: in `mdb-893`, shape is 24 m, stops are 17 m apart, and the
                // finding is CORRECT — it remains reported because 17 m > 0.
                if d > 0.0 && d < MIN_TRIP_KM {
                    let route = trip_to_route_rem.get(trip_id).copied().unwrap_or(trip_id);
                    let dist_m = d * 1000.0;
                    notices.push(k6_notice(
                        ctr, "OPR_017", EntityType::Trip,
                        Some(trip_id.to_string()), Some(trip_id.to_string()),
                        "trips.txt", None, None,
                        Some(format!("{dist_m:.0}m")),
                        Some(format!("> {:.0}m", MIN_TRIP_KM * 1000.0)),
                        format!("'{route}' hattının seferi yalnızca {dist_m:.0}m mesafe kapsıyor.",),
                        "Sefer güzergahını veya güzergah şekli verilerini kontrol edin.",
                    ));
                }
            }
        }
    }

    // OPR_018 (service period < 3 active days) REMOVED (2026-07-23): it was a strict subset
    // of CAL_010. CAL_010 already catches services with 1–2 active days using
    // `total_days <= service_gap_days` (config minimum 3); same severity/class/score/message.
    // See the change note in docs/rules/CAL/CAL_010.md.

    // ── OPR_010: inconsistent accessibility / bicycle policy on a route ─────
    {
        let _t10 = Timer::start("K6::rem::opr_010");
        // (has_accessible=1, has_not_accessible=2)
        let mut route_wc: FxHashMap<&str, (bool, bool)> = FxHashMap::default();
        let mut route_ba: FxHashMap<&str, (bool, bool)> = FxHashMap::default();
        for t in &records.trips {
            if ti_rem.route_id(t).is_empty() {
                continue;
            }
            if let Some(wc) = t.wheelchair_accessible {
                let e = route_wc.entry(ti_rem.route_id(t)).or_default();
                if wc == 1 {
                    e.0 = true;
                }
                if wc == 2 {
                    e.1 = true;
                }
            }
            if let Some(ba) = t.bikes_allowed {
                let e = route_ba.entry(ti_rem.route_id(t)).or_default();
                if ba == 1 {
                    e.0 = true;
                }
                if ba == 2 {
                    e.1 = true;
                }
            }
        }
        for (route_id, (has_acc, has_noacc)) in &route_wc {
            if *has_acc && *has_noacc {
                let label = route_short.get(*route_id).copied().unwrap_or(*route_id);
                let mut notice = k6_notice(
                    ctr, "OPR_010", EntityType::Route,
                    Some(route_id.to_string()), Some(route_id.to_string()),
                    "trips.txt", None, Some("wheelchair_accessible"),
                    None, None,
                    format!("'{label}' kodlu hatta bazı seferler tekerlekli sandalye erişimli (1), bazıları erişimsiz (2) olarak işaretlenmiş."),
                    "Aynı hattaki tüm seferlerin erişilebilirlik bilgisini tutarlı hâle getirin.",
                );
                notice
                    .details
                    .get_or_insert_with(Default::default)
                    .insert("route_label".to_string(), label.to_string());
                notices.push(notice);
            }
        }
        for (route_id, (has_allow, has_noallow)) in &route_ba {
            if *has_allow && *has_noallow {
                let label = route_short.get(*route_id).copied().unwrap_or(*route_id);
                let mut notice = k6_notice(
                    ctr, "OPR_010", EntityType::Route,
                    Some(route_id.to_string()), Some(route_id.to_string()),
                    "trips.txt", None, Some("bikes_allowed"),
                    None, None,
                    format!("'{label}' kodlu hatta bazı seferler bisiklete izin veriyor (1), bazıları vermiyor (2)."),
                    "Aynı hattaki tüm seferlerin bisiklet politikasını tutarlı hâle getirin.",
                );
                notice
                    .details
                    .get_or_insert_with(Default::default)
                    .insert("route_label".to_string(), label.to_string());
                notices.push(notice);
            }
        }
    }

    // ── OPR_014: average transfer time is long across the feed ───────────────
    {
        let _t14 = Timer::start("K6::rem::opr_014");
        const AVG_TRANSFER_THRESHOLD_SEC: u64 = 600; // 10 dakika
        let timed: Vec<u32> = records
            .transfers
            .iter()
            .filter(|t| t.transfer_type == Some(2))
            .filter_map(|t| t.min_transfer_time)
            .collect();
        if !timed.is_empty() {
            let count = timed.len();
            let avg = timed.iter().map(|&x| x as u64).sum::<u64>() / count as u64;
            if avg > AVG_TRANSFER_THRESHOLD_SEC {
                let avg_min = avg as f64 / 60.0;
                notices.push(k6_notice(
                    ctr, "OPR_014", EntityType::Feed,
                    None, None,
                    "transfers.txt", None, Some("min_transfer_time"),
                    Some(format!("{avg}s ({count})")),
                    Some(format!("≤ {AVG_TRANSFER_THRESHOLD_SEC}s")),
                    format!("Feed genelinde {count} zamanlı aktarmanın ortalama min_transfer_time değeri {avg}s ({avg_min:.1} dakika)."),
                    "Aktarma sürelerini gözden geçirin; uzun aktarmalar yolcu deneyimini olumsuz etkiler.",
                ));
            }
        }
    }

    // ── TRF_011: transfer points are too far apart ───────────────────────────
    {
        let _t11 = Timer::start("K6::rem::trf_011");
        const TRF_DIST_THRESHOLD_M: f64 = 2000.0;
        // 🔴 ONE FINDING PER STOP PAIR (17th run review, 2026-09-12).
        //
        // The rule card already defines the identity as `from_stop_id|to_stop_id`, meaning "the finding's
        // identity is the STOP PAIR"; emission was per ROW and contradicted the rule's own declaration.
        // Same decision made for `TRP_004` that day (there `scope_key` was `shape_id`, emission was per trip).
        //
        // Content requires this too: distance between two stops does not change no matter which trip
        // uses the pair. In feeds with trip-by-trip transfers, the same sentence was repeated dozens of times.
        // Measurement (`tfs-789`, verified with the archived sha256 run): ALL 829 transfer rows exceed 2 km,
        // but there are only **57 distinct stop pairs**, averaging 14.5 rows per pair. ⚠️ MD does NOT count
        // per pair (22nd run, 3 feeds measured): `transfer_distance_above_2_km` counts only 2–10 km ROWS,
        // `_too_large` counts >10 km. Per-pair emission was chosen to cut repeat noise, not for MD parity.
        // Threshold (2,000 m) and Haversine are identical on both sides; the difference is emission unit and band split.
        let mut far_pairs: BTreeMap<(&str, &str), (f64, u64, u64)> = BTreeMap::new();
        for trf in &records.transfers {
            if trf.from_stop_id.is_empty() || trf.to_stop_id == trf.from_stop_id {
                continue;
            }
            if let (Some(&(la1, lo1)), Some(&(la2, lo2))) = (
                stop_coords.get(trf.from_stop_id.as_str()),
                stop_coords.get(trf.to_stop_id.as_str()),
            ) {
                // Null Island (same condition as GEO_016): do not measure distance to the (0,0) placeholder.
                // `mdb-2393` reported a "5,590 km transfer"; the root cause is the broken coordinate and GEO_016
                // already says so. `blocks` cannot solve this: a broad root suppresses only the exact same scope
                // (K7), while GEO_016 carries `stop_id` and TRF_011 carries `from|to`.
                if (la1.abs() < 0.1 && lo1.abs() < 0.1) || (la2.abs() < 0.1 && lo2.abs() < 0.1) {
                    continue;
                }
                let dist_m = haversine_km(la1, lo1, la2, lo2) * 1000.0;
                if dist_m > TRF_DIST_THRESHOLD_M {
                    let e = far_pairs
                        .entry((trf.from_stop_id.as_str(), trf.to_stop_id.as_str()))
                        .or_insert((dist_m, 0, trf.line));
                    e.1 += 1;
                }
            }
        }
        // `BTreeMap` order provides determinism — required by the `XFL_006` lesson.
        for ((from, to), (dist_m, rows, first_line)) in far_pairs {
            let mut n = k6_notice(
                ctr, "TRF_011", EntityType::Transfer,
                Some(format!("{from}|{to}")),
                Some(format!("{from}|{to}")),
                "transfers.txt", Some(first_line),
                Some("from_stop_id|to_stop_id"),
                Some(format!("{dist_m:.0}m")),
                Some(format!("≤ {TRF_DIST_THRESHOLD_M:.0}m")),
                format!(
                    "'{from}' → '{to}' aktarması {dist_m:.0}m uzaklıkta — yürüyüş mesafesi {}m eşiğini aşıyor.{}",
                    TRF_DIST_THRESHOLD_M as u32,
                    if rows > 1 { format!(" Bu çift {rows} aktarma satırında tekrarlanıyor.") } else { String::new() }
                ),
                "Aktarma tanımını gözden geçirin; çok uzak duraklar arasındaki aktarma yolcular için zorlayıcı olabilir.",
            );
            if rows > 1 {
                let mut details: BTreeMap<String, String> = BTreeMap::new();
                details.insert("transfer_rows".to_string(), rows.to_string());
                n.details = Some(details.into());
            }
            notices.push(n);
        }
    }

    // ── SHP_015: shape has statistically too few points ──────────────────────
    {
        let _t15 = Timer::start("K6::rem::shp_015");
        const MIN_POINTS_PER_10KM: f64 = 2.0; // en az 2 nokta / 10km
        for (shape_id, pts) in shape_coords.iter() {
            let total_km: f64 = pts
                .windows(2)
                .map(|w| haversine_km(w[0].0, w[0].1, w[1].0, w[1].1))
                .sum();
            if total_km > 1.0 {
                let density = pts.len() as f64 / (total_km / 10.0);
                if density < MIN_POINTS_PER_10KM {
                    notices.push(k6_notice(
                        ctr, "SHP_015", EntityType::Shape,
                        Some(shape_id.to_string()), Some(shape_id.to_string()),
                        "shapes.txt", None, Some("shape_pt_lat|shape_pt_lon"),
                        Some(format!("{} / {total_km:.1}km", pts.len())),
                        Some(format!("≥ {MIN_POINTS_PER_10KM:.0}/10km")),
                        format!(
                            "'{shape_id}' güzergahı {total_km:.1} km uzunluğunda ancak yalnızca {} nokta var — yoğunluk {density:.1} nokta/10km.",
                            pts.len()
                        ),
                        "Güzergah şekline ara noktalar ekleyin; düşük nokta yoğunluğu harita gösterimini bozar.",
                    ));
                }
            }
        }
    }

    // ── SHP_020: non-consecutive repeated point in a shape ───────────────────
    // Saturation aggregation (SHP_010 precedent, #48): OSM-derived polyline generators trigger
    // on most shapes because of sub-meter micro-jitter (VBB 66%, mdb-1993 83% — apex median 1 m,
    // so this is point noise, not a U-turn). Above the threshold, emit one feed summary instead of
    // a flood of per-shape INFO notices.
    //
    // Geographic dedup: a station/turnout geometry REPEATS in outbound and return shapes,
    // so per-shape emission counts the same PHYSICAL point repeatedly (TCDD: 28 notices = only
    // 8 distinct coordinates). Emit one notice per COORDINATE; representative is the smallest
    // shape_id, and the message/details show how many shapes contain it.
    {
        let _t20b = Timer::start("K6::rem::shp_020");
        const DUP_THRESHOLD_DEG: f64 = 1e-6;
        const SHP020_AGG_THRESHOLD: usize = 50;
        // (rounded lat, lon) → (representative shape_id, i, j, lat, lon, shapes at that coordinate).
        type ShapeDuplicate<'a> = (&'a str, usize, usize, f64, f64, Vec<&'a str>);
        type ShapeDuplicatesByCoord<'a> =
            std::collections::BTreeMap<(i64, i64), ShapeDuplicate<'a>>;
        let mut by_coord: ShapeDuplicatesByCoord<'_> = std::collections::BTreeMap::new();
        for (shape_id, pts) in shape_coords.iter() {
            // Check a small window, not only consecutive pairs.
            'outer: for i in 0..pts.len() {
                for j in (i + 2)..pts.len().min(i + 10) {
                    let dlat = (pts[i].0 - pts[j].0).abs();
                    let dlon = (pts[i].1 - pts[j].1).abs();
                    if dlat < DUP_THRESHOLD_DEG && dlon < DUP_THRESHOLD_DEG {
                        let key = (
                            (pts[i].0 * 1e6).round() as i64,
                            (pts[i].1 * 1e6).round() as i64,
                        );
                        let e = by_coord
                            .entry(key)
                            .or_insert_with(|| (*shape_id, i, j, pts[i].0, pts[i].1, Vec::new()));
                        // Deterministic representative: lexicographically smallest shape_id.
                        if *shape_id < e.0 {
                            e.0 = *shape_id;
                            e.1 = i;
                            e.2 = j;
                        }
                        e.5.push(*shape_id);
                        break 'outer;
                    }
                }
            }
        }

        let mut shp020_pending: Vec<Notice> = Vec::new();
        for (_key, (shape_id, i, j, lat, lon, mut shapes)) in by_coord {
            shapes.sort_unstable();
            shapes.dedup();
            let n_shapes = shapes.len();
            let prefix = shp_route_prefix(
                shape_route_labels
                    .get(shape_id)
                    .map(|v| v.as_slice())
                    .unwrap_or(&[]),
            );
            let repeat_suffix = if n_shapes > 1 {
                format!(" Aynı konum {n_shapes} güzergah şeklinde tekrarlanıyor (istasyon/makas geometrisi).")
            } else {
                String::new()
            };
            let mut n = k6_notice(
                ctr, "SHP_020", EntityType::Shape,
                Some(shape_id.to_string()), Some(shape_id.to_string()),
                "shapes.txt", None, Some("shape_pt_lat|shape_pt_lon"),
                Some(format!("{i}/{j}: ({lat:.6},{lon:.6})")),
                None,
                format!(
                    "{prefix}'{shape_id}' güzergahında nokta {i} ile {j} neredeyse aynı konumda ({lat:.6},{lon:.6}) — tekrarlayan nokta.{repeat_suffix}"
                ),
                "Güzergah şeklindeki tekrarlayan noktaları temizleyin.",
            );
            // The feed-summary branch writes `affected_shapes`/`example_shapes`; the single-item branch
            // carries the same keys so one template is populated in both branches.
            {
                let d = n.details.get_or_insert_with(Default::default);
                d.insert("affected_shapes".to_string(), "1".to_string());
                d.insert("example_shapes".to_string(), shape_id.to_string());
            }
            if n_shapes > 1 {
                // Assignment `= Some(map)` used to delete affected_shapes/example_shapes written above;
                // add to the existing map instead.
                let d = n.details.get_or_insert_with(Default::default);
                d.insert("repeated_shapes".to_string(), n_shapes.to_string());
                d.insert(
                    "shapes".to_string(),
                    shapes
                        .iter()
                        .take(10)
                        .copied()
                        .collect::<Vec<_>>()
                        .join(","),
                );
            }
            shp020_pending.push(n);
        }
        let n20 = shp020_pending.len();
        if n20 > SHP020_AGG_THRESHOLD {
            let examples: Vec<String> = shp020_pending
                .iter()
                .filter_map(|x| x.entity_id.clone())
                .take(5)
                .collect();
            let mut notice = k6_notice(
                ctr, "SHP_020", EntityType::Feed, None, None,
                "shapes.txt", None, Some("shape_pt_lat|shape_pt_lon"),
                Some(n20.to_string()), None,
                format!("{n20} güzergah şeklinde tekrarlayan (neredeyse özdeş) nokta var — üretici kaynaklı yoğun nokta gürültüsü."),
                "Güzergah üretim sürecinde tekrarlayan noktaları eleyin; tek tek shape incelemesi gerekmez.",
            );
            let mut d = std::collections::BTreeMap::new();
            d.insert("affected_shapes".to_string(), n20.to_string());
            if !examples.is_empty() {
                d.insert("example_shapes".to_string(), examples.join(", "));
            }
            notice.details = Some(d.into());
            notices.push(notice);
        } else {
            notices.append(&mut shp020_pending);
        }
    }

    // ── SHP_009: route shape intersects itself ───────────────────────────────
    // Saturation aggregation (SHP_010 precedent): most intersections are near-segment micro-
    // intersections at junctions/stop bays (VBB: 73% have j-i≤5, median gap 34 m; 55% of feeds
    // fired). Above the threshold, emit one feed summary instead of a per-shape flood.
    //
    // Geographic dedup (SHP_020 precedent): a turnout/station approach REPEATS in outbound and
    // return shapes, so per-shape emission counts the same PHYSICAL intersection repeatedly.
    // Measured in TCDD v2: 9 notices = only 4 areas (Kayaş ×4, Bakırköy ×2, Gebze ×2, Goncalı ×1).
    // Emit one notice per intersection LOCATION now.
    {
        let _t09b = Timer::start("K6::rem::shp_009");
        const SHP009_AGG_THRESHOLD: usize = 50;
        // (rounded lat, lon) → (representative shape_id, i, j, shapes at that location).
        type ShapeCrossing<'a> = (&'a str, usize, usize, Vec<&'a str>);
        type ShapeCrossingsByCoord<'a> = std::collections::BTreeMap<(i64, i64), ShapeCrossing<'a>>;
        let mut by_coord: ShapeCrossingsByCoord<'_> = std::collections::BTreeMap::new();
        for (shape_id, pts) in shape_coords.iter() {
            if pts.len() < 4 {
                continue;
            }
            // O(n²) segment crossing: check at most 300 segments for large shapes.
            let n = pts.len().min(301);
            // `segments_cross` decides only on a real intersection; first eliminating axis-aligned
            // bounding boxes cuts expensive cross products while preserving the same result. Use strict
            // `<`: a pair intersecting at the bbox boundary is left to `segments_cross` to preserve endpoint semantics.
            let segment_bbox: Vec<(f64, f64, f64, f64)> = pts[..n]
                .windows(2)
                .map(|w| {
                    (
                        w[0].0.min(w[1].0),
                        w[0].0.max(w[1].0),
                        w[0].1.min(w[1].1),
                        w[0].1.max(w[1].1),
                    )
                })
                .collect();
            'seg_outer: for i in 0..n.saturating_sub(1) {
                let (amin_lat, amax_lat, amin_lon, amax_lon) = segment_bbox[i];
                for j in i + 2..n.saturating_sub(1) {
                    if i == 0 && j == n - 2 {
                        continue;
                    } // adjacent endpoints
                    let (bmin_lat, bmax_lat, bmin_lon, bmax_lon) = segment_bbox[j];
                    if amax_lat < bmin_lat
                        || bmax_lat < amin_lat
                        || amax_lon < bmin_lon
                        || bmax_lon < amin_lon
                    {
                        continue;
                    }
                    if segments_cross(pts[i], pts[i + 1], pts[j], pts[j + 1]) {
                        let key = (
                            (pts[i].0 * 1e3).round() as i64,
                            (pts[i].1 * 1e3).round() as i64,
                        );
                        let e = by_coord
                            .entry(key)
                            .or_insert_with(|| (*shape_id, i, j, Vec::new()));
                        // Deterministic representative: lexicographically smallest shape_id.
                        if *shape_id < e.0 {
                            e.0 = *shape_id;
                            e.1 = i;
                            e.2 = j;
                        }
                        e.3.push(*shape_id);
                        break 'seg_outer;
                    }
                }
            }
        }
        let mut shp009_pending: Vec<Notice> = Vec::new();
        for (_key, (shape_id, i, j, mut shapes)) in by_coord {
            shapes.sort_unstable();
            shapes.dedup();
            let n_shapes = shapes.len();
            let prefix = shp_route_prefix(
                shape_route_labels
                    .get(shape_id)
                    .map(|v| v.as_slice())
                    .unwrap_or(&[]),
            );
            let repeat_suffix = if n_shapes > 1 {
                format!(" Aynı kesişme {n_shapes} güzergah şeklinde tekrarlanıyor (makas/istasyon yaklaşması).")
            } else {
                String::new()
            };
            let mut n = k6_notice(
                ctr, "SHP_009", EntityType::Shape,
                Some(shape_id.to_string()), Some(shape_id.to_string()),
                "shapes.txt", None, Some("shape_pt_lat|shape_pt_lon"),
                Some(format!("segment {i}-{} ∩ {j}-{}", i+1, j+1)), None,
                format!(
                    "{prefix}'{shape_id}' güzergah şekli segment {i}→{} ile segment {j}→{} arasında kendisiyle kesişiyor.{repeat_suffix}",
                    i+1, j+1
                ),
                "Güzergah şeklindeki kesişen segmentleri düzeltin.",
            );
            // The feed-summary branch writes `affected_shapes`/`example_shapes`; the single-item branch
            // carries the same keys so one template is populated in both branches.
            {
                let d = n.details.get_or_insert_with(Default::default);
                d.insert("affected_shapes".to_string(), "1".to_string());
                d.insert("example_shapes".to_string(), shape_id.to_string());
            }
            if n_shapes > 1 {
                let d = n.details.get_or_insert_with(Default::default);
                d.insert("repeated_shapes".to_string(), n_shapes.to_string());
                d.insert(
                    "shapes".to_string(),
                    shapes
                        .iter()
                        .take(10)
                        .copied()
                        .collect::<Vec<_>>()
                        .join(","),
                );
            }
            shp009_pending.push(n);
        }
        let n09 = shp009_pending.len();
        if n09 > SHP009_AGG_THRESHOLD {
            let examples: Vec<String> = shp009_pending
                .iter()
                .filter_map(|x| x.entity_id.clone())
                .take(5)
                .collect();
            let mut notice = k6_notice(
                ctr, "SHP_009", EntityType::Feed, None, None,
                "shapes.txt", None, Some("shape_pt_lat|shape_pt_lon"),
                Some(n09.to_string()), None,
                format!("{n09} güzergah şekli kendisiyle kesişiyor — çoğu kavşak/durak geometrisindeki yakın-segment mikro-kesişmedir."),
                "Güzergah üretim sürecini gözden geçirin; tek tek shape incelemesi gerekmez.",
            );
            let mut d = std::collections::BTreeMap::new();
            d.insert("affected_shapes".to_string(), n09.to_string());
            if !examples.is_empty() {
                d.insert("example_shapes".to_string(), examples.join(", "));
            }
            notice.details = Some(d.into());
            notices.push(notice);
        } else {
            notices.append(&mut shp009_pending);
        }
    }
}

/// SHP_012: the route shape is far from the trip stops.
/// Extracted from check_remaining_analytics as a separate K6 task (to shorten the long-running
/// branch in parallel wall-clock time). The shape_coords/shape_bbox/stop_coords SETUP is
/// IDENTICAL to build_maps (same sort_by_key, same bbox) → output (notice count/value/score) is identical.
/// It has its own local caches (shape_stop_violations, dist_cache); there is NO mutable state
/// shared with another check → safe as an independent task, matching the existing 13-task pattern.
fn check_shp012<'a>(
    records: &'a EntityRecords,
    config: &ValidatorConfig,
    idx: &StopTimesIndex<'_>,
    shape_idx: &ShapeIndex<'a>,
    rail_shapes: &FxHashSet<&str>,
    notices: &mut Vec<Notice>,
    ctr: &mut u32,
) {
    let ti_shp012 = &records.trip_interns;
    // Setup is performed ONCE at K6 entry (see ShapeIndex) — only borrowed here.
    let shape_coords = &shape_idx.coords;
    let shape_bbox = &shape_idx.bbox;
    let stop_coords: FxHashMap<&str, (f64, f64)> = records
        .stops
        .iter()
        .filter_map(|s| recoverable_stop_coord(s).map(|c| (s.stop_id.as_str(), c)))
        .collect();

    // ── SHP_012 body (moved verbatim from check_remaining_analytics) ─────────
    {
        let _t12b = crate::timing::Timer::start("K6::shp012::body");

        // Perf: memoize (shape_id, stop_id) → polyline distance. Dozens of trips using the same
        // shape repeatedly recomputed the same (shape,stop) distance; the expensive
        // point_to_polyline now runs only once per UNIQUE pair.
        //
        // ⚠️ 2026-07-25: the counter used to increase per TRIP-STOP INSTANCE, but the message
        // called it a "stop". In VBB, shape '10342' is used by 579 trips, each one far from
        // one stop → the message said "579 stops >100m away"; the real count was 1
        // (579× inflation, making the rule unreadable). Count UNIQUE stops now.
        // Notice volume is unchanged (dedup Entity: one notice per shape) → score is unchanged;
        // only the observed_value/message count is corrected.
        let mut shape_stop_violations: FxHashMap<&str, FxHashSet<&str>> = FxHashMap::default();
        let mut dist_cache: FxHashMap<(&str, &str), f64> = FxHashMap::default();
        for trip in &records.trips {
            let Some(shape_id) = ti_shp012.shape_id(trip).filter(|s| !s.is_empty()) else {
                continue;
            };
            let Some(pts) = shape_coords.get(shape_id) else {
                continue;
            };
            let Some(stimes) = idx.by_trip.get(trip.trip_id.as_str()) else {
                continue;
            };
            // Threshold per shape: rail routes need wider tolerance for station geometry.
            let shp_stop_threshold_m = stop_shape_threshold_m(shape_id, rail_shapes, config);

            for st in stimes.iter() {
                let stop_id = idx.stop_id_of(st);
                let Some(&(slat, slon)) = stop_coords.get(stop_id) else {
                    continue;
                };
                // Distance to the nearest SEGMENT of the shape (not point-to-point): avoids
                // false positives for stops between sparse shape points.
                let min_dist_m = *dist_cache.entry((shape_id, stop_id)).or_insert_with(|| {
                    // B3 bbox shortcut (following GEO_009): a stop outside bbox+500m is certainly
                    // >500m away → use clamped-corner haversine (>500m guaranteed) instead of a full
                    // polyline calculation. The >threshold boolean, viol_count, and message are preserved exactly.
                    if let Some(&[bmin_la, bmax_la, bmin_lo, bmax_lo]) = shape_bbox.get(shape_id) {
                        let cos_lat = slat.to_radians().cos();
                        let margin_lat = shp_stop_threshold_m / 111_320.0_f64;
                        let margin_lon = shp_stop_threshold_m / (111_320.0_f64 * cos_lat);
                        if slat < bmin_la - margin_lat
                            || slat > bmax_la + margin_lat
                            || slon < bmin_lo - margin_lon
                            || slon > bmax_lo + margin_lon
                        {
                            let clat = slat.clamp(bmin_la, bmax_la);
                            let clon = slon.clamp(bmin_lo, bmax_lo);
                            return haversine_km(slat, slon, clat, clon) * 1000.0;
                        }
                    }
                    point_to_polyline_dist_m(slat, slon, pts)
                });
                if min_dist_m > shp_stop_threshold_m {
                    shape_stop_violations
                        .entry(shape_id)
                        .or_default()
                        .insert(stop_id);
                }
            }
        }

        // Determinism: FxHashMap order depends on insertion; sort by shape_id.
        let mut viol_sorted: Vec<(&&str, &FxHashSet<&str>)> =
            shape_stop_violations.iter().collect();
        viol_sorted.sort_by(|a, b| a.0.cmp(b.0));
        // The ID list is carried in details so the map can show faulty stops in a SEPARATE COLOR.
        // Limit: first 25 for map readability (sorted, deterministic); the count is already in the message.
        const SHP012_MAX_LISTED: usize = 25;
        for (shape_id, viol_set) in viol_sorted {
            let viol_count = viol_set.len();
            let shp_stop_threshold_m = stop_shape_threshold_m(shape_id, rail_shapes, config);
            let mut far: Vec<&str> = viol_set.iter().copied().collect();
            far.sort_unstable();
            far.truncate(SHP012_MAX_LISTED);
            let mut n = k6_notice(
                ctr, "SHP_012", EntityType::Shape,
                Some(shape_id.to_string()), Some(shape_id.to_string()),
                "shapes.txt", None, Some("shape_pt_lat|shape_pt_lon"),
                Some(format!("{viol_count} (>{shp_stop_threshold_m:.0}m)")), None,
                format!(
                    "'{shape_id}' güzergah şekli {viol_count} duraktan >{shp_stop_threshold_m:.0}m uzakta — güzergah doğru çizilmemiş olabilir."
                ),
                "shapes.txt noktalarını durak konumlarına yaklaştırın.",
            );
            n.details = Some(
                [("far_stops".to_string(), far.join(","))]
                    .into_iter()
                    .collect(),
            );
            notices.push(n);
        }
    }
}

/// SHP_022: a stop is near multiple matching regions on a shape.
/// Extracted from check_remaining_analytics as a separate K6 task (the heaviest sub-check in
/// remaining, ~126ms; shortens the long-running branch in parallel wall-clock time). Its maps
/// (shape_coords/shape_bbox/stop_coords/stop_names/trip_shape_local) use SETUP IDENTICAL to build_maps;
/// shape_cum is rebuilt locally (memoization only — same values).
/// Dedup sets (shp022_seen/shp022_done) are local → safe as an independent task, identical output.
fn check_shp022<'a>(
    records: &'a EntityRecords,
    idx: &StopTimesIndex<'_>,
    shape_idx: &ShapeIndex<'a>,
    notices: &mut Vec<Notice>,
    ctr: &mut u32,
) {
    use crate::timing::Timer;
    let ti_shp022 = &records.trip_interns;

    // Setup is performed ONCE at K6 entry (see ShapeIndex) — only borrowed here.
    let shape_coords = &shape_idx.coords;
    let shape_bbox = &shape_idx.bbox;
    let stop_coords: FxHashMap<&str, (f64, f64)> = records
        .stops
        .iter()
        .filter_map(|s| recoverable_stop_coord(s).map(|c| (s.stop_id.as_str(), c)))
        .collect();
    let stop_names: FxHashMap<&str, &str> = records
        .stops
        .iter()
        .filter_map(|s| s.stop_name.as_deref().map(|n| (s.stop_id.as_str(), n)))
        .collect();
    let trip_shape_local: HashMap<&str, &str> = records
        .trips
        .iter()
        .filter_map(|t| ti_shp022.shape_id(t).map(|s| (t.trip_id.as_str(), s)))
        .collect();
    let mut shape_cum: FxHashMap<&str, Vec<f64>> = FxHashMap::default();

    // ── SHP_022 body (moved verbatim from check_remaining_analytics) ─────────
    {
        let _t22 = Timer::start("K6::shp022::body");
        const MATCH_KM: f64 = 0.150; // 150 m — matching threshold
        const SEP_KM: f64 = 0.500; // 500 m — minimum arc gap between clusters

        let mut shp022_seen: FxHashSet<(&str, &str)> = FxHashSet::default();
        // B2 perf: (shape,stop) clustering decision is a pure function — do not repeat the
        // segment scan for dozens of trips sharing the same pair and missing sdt.
        // shp022_seen is only emission dedup; this set is EVALUATION dedup → output is identical.
        let mut shp022_done: FxHashSet<(&str, &str)> = FxHashSet::default();
        // #15: SHP_022 Entity-scope (stop_id) → dedup collapses to ONE representative per stop
        // (min st.line, shape_id tiebreak). Keep a BUFFER per stop instead of emitting per
        // (shape,stop) → no intermediate-notice explosion on large feeds; result is dedup-equivalent
        // and iteration-independent.
        let mut shp022_best: FxHashMap<&str, ((u64, &str), Notice)> = FxHashMap::default();

        for (trip_id, stimes) in &idx.by_trip {
            // Only trips missing shape_dist_traveled.
            if !idx.trips_missing_sdt.contains_key(trip_id) {
                continue;
            }
            let Some(&shape_id) = trip_shape_local.get(trip_id) else {
                continue;
            };
            let Some(pts) = shape_coords.get(shape_id) else {
                continue;
            };
            if pts.len() < 2 {
                continue;
            }

            // #52: when stop_sequence is USABLE (present, strictly increasing, and unique),
            // sequence-aware monotonic matching resolves the stop's position on the shape →
            // SHP_022 is unnecessary (major over-fire on normal feeds). Position is genuinely
            // ambiguous only in trips with missing/invalid sequence (STM_005) or duplicates (STM_032).
            // The by_trip slice is sorted by stop_sequence ascending (None=MAX at the end).
            let seq_usable = {
                let mut last: Option<u32> = None;
                let mut ok = true;
                for st in stimes.iter() {
                    match st.stop_sequence() {
                        None => {
                            ok = false;
                            break;
                        }
                        Some(s) => {
                            if last.is_some_and(|l| s <= l) {
                                ok = false;
                                break;
                            }
                            last = Some(s);
                        }
                    }
                }
                ok
            };
            if seq_usable {
                continue;
            }

            let cum = shape_cum.entry(shape_id).or_insert_with(|| {
                let mut c = Vec::with_capacity(pts.len());
                c.push(0.0_f64);
                for i in 1..pts.len() {
                    c.push(c[i - 1] + haversine_km(pts[i - 1].0, pts[i - 1].1, pts[i].0, pts[i].1));
                }
                c
            });
            let cos_lat = (pts[0].0.to_radians()).cos().max(0.001_f64);
            let scale_lon = 111.0_f64 * cos_lat;
            let match_sq = MATCH_KM * MATCH_KM;

            for st in stimes.iter() {
                let stop_id = idx.stop_id_of(st);
                if shp022_seen.contains(&(shape_id, stop_id)) {
                    continue;
                }
                if !shp022_done.insert((shape_id, stop_id)) {
                    continue;
                }
                let Some(&(slat, slon)) = stop_coords.get(stop_id) else {
                    continue;
                };

                // B2 bbox prefilter (following GEO_009, around line ~3323): a stop outside
                // bbox+MATCH_KM cannot be within MATCH_KM of any segment → close_arcs is empty → no notice.
                if let Some(&[bmin_la, bmax_la, bmin_lo, bmax_lo]) = shape_bbox.get(shape_id) {
                    let margin_lat = MATCH_KM / 111.0_f64;
                    let margin_lon = MATCH_KM / scale_lon;
                    if slat < bmin_la - margin_lat
                        || slat > bmax_la + margin_lat
                        || slon < bmin_lo - margin_lon
                        || slon > bmax_lo + margin_lon
                    {
                        continue;
                    }
                }

                // Compute the minimum distance for each segment and record those below MATCH_KM.
                let mut close_arcs: Vec<f64> = Vec::new();
                for w in 0..pts.len() - 1 {
                    let (alat, alon) = pts[w];
                    let (blat, blon) = pts[w + 1];
                    let ax = (alon - slon) * scale_lon;
                    let ay = (alat - slat) * 111.0_f64;
                    let bx = (blon - slon) * scale_lon;
                    let by_ = (blat - slat) * 111.0_f64;
                    let dx = bx - ax;
                    let dy = by_ - ay;
                    let len_sq = dx * dx + dy * dy;
                    let t = if len_sq < 1e-12 {
                        0.0_f64
                    } else {
                        ((-ax * dx) + (-ay * dy)) / len_sq
                    }
                    .clamp(0.0_f64, 1.0_f64);
                    let nx = ax + t * dx;
                    let ny = ay + t * dy;
                    let dsq = nx * nx + ny * ny;
                    if dsq <= match_sq {
                        close_arcs.push(cum[w] + t * (cum[w + 1] - cum[w]));
                    }
                }
                if close_arcs.is_empty() {
                    continue;
                }

                // Find the number of clusters (non-consecutive arc groups).
                close_arcs.sort_by(|a, b| a.total_cmp(b));
                let mut clusters = 1usize;
                let mut prev = close_arcs[0];
                for &arc in &close_arcs[1..] {
                    if arc - prev > SEP_KM {
                        clusters += 1;
                    }
                    prev = arc;
                }
                if clusters < 2 {
                    continue;
                }

                shp022_seen.insert((shape_id, stop_id));
                // Keep only the smallest (st.line, shape_id) representative per stop (dedup-equivalent).
                let best_key = (st.line as u64, shape_id);
                if shp022_best
                    .get(stop_id)
                    .is_some_and(|(k, _)| *k <= best_key)
                {
                    continue;
                }
                let sname = stop_names.get(stop_id).copied().unwrap_or(stop_id);
                let mut notice = k6_notice(
                    ctr,
                    "SHP_022",
                    EntityType::Stop,
                    Some(stop_id.to_string()),
                    Some(stop_id.to_string()),
                    "stop_times.txt",
                    Some(st.line as u64),
                    Some("stop_id"),
                    Some(stop_id.to_string()),
                    None,
                    format!(
                        "'{}' (kod: '{}') durağı '{}' güzergah şeklinin {} ayrı bölümüne yakın — \
                         shape_dist_traveled eksikken hangi bölümle eşleşeceği belirsiz.",
                        sname, stop_id, shape_id, clusters
                    ),
                    "stop_times'a shape_dist_traveled ekleyerek durağın şekil üzerindeki \
                     konumunu açıkça belirtin.",
                );
                let mut det = std::collections::BTreeMap::new();
                det.insert("shape_id".to_string(), shape_id.to_string());
                notice.details = Some(det.into());
                shp022_best.insert(stop_id, (best_key, notice));
            }
        }
        // Flush the buffer: one (deterministic) representative per stop. Order is irrelevant
        // (K6 renumbering + K7 dedup canonicalizes it later).
        for (_stop, (_key, notice)) in shp022_best {
            notices.push(notice);
        }
    }
}

/// Check whether two 2D line segments intersect (cross-product method).
fn segments_cross(a: (f64, f64), b: (f64, f64), c: (f64, f64), d: (f64, f64)) -> bool {
    let cross = |o: (f64, f64), p: (f64, f64), q: (f64, f64)| -> f64 {
        (p.0 - o.0) * (q.1 - o.1) - (p.1 - o.1) * (q.0 - o.0)
    };
    let d1 = cross(c, d, a);
    let d2 = cross(c, d, b);
    let d3 = cross(a, b, c);
    let d4 = cross(a, b, d);
    if d1 * d2 < 0.0 && d3 * d4 < 0.0 {
        return true;
    }
    false
}

/// Packed YYYYMMDD → Julian Day Number. The formula is in `k5_derived::ymd_to_jdn`;
/// here we only unpack u32 and guard invalid dates (a zero component).
pub(crate) fn yyyymmdd_to_jdn(yyyymmdd: u32) -> u32 {
    let y = yyyymmdd / 10000;
    let m = (yyyymmdd / 100) % 100;
    let d = yyyymmdd % 100;
    if y == 0 || m == 0 || d == 0 {
        return 0;
    }
    ymd_to_jdn(y, m, d)
}

fn format_hms(total_secs: u32) -> String {
    let h = total_secs / 3600;
    let m = (total_secs % 3600) / 60;
    let s = total_secs % 60;
    format!("{h:02}:{m:02}:{s:02}")
}

// ── Linked trip continuations (TRF_022 / TRF_023) ───────────────────────────
//
// GTFS Reference, `transfers.txt` → "Linked trips" section (`transfer_type=4` and `=5`):
//   • "In a 1-to-n continuation, the trips.service_id for each to_trip_id MUST be identical."
//   • "In an n-to-1 continuation, the trips.service_id for each from_trip_id MUST be identical."
//   • "Trips may be linked together as part of multiple distinct continuations, provided that
//      the trip.service_id MUST NOT overlap on any day of service."
//
// ⚠️ THE THIRD SENTENCE LIMITS THE FIRST TWO — applying them literally produces FALSE POSITIVES.
// If we apply "all of them must be identical" directly, we reject a case the spec EXPLICITLY
// ALLOWS: the same trip may continue to A on weekdays and B on Saturdays (two SEPARATE
// continuations, different `service_id`, NO overlapping days). That would be exactly the
// `PTH_017` failure mode — treating a spec permission as a requirement.
//
// The data has NO field saying which continuation a row belongs to; grouping is implicit.
// Therefore the only workable reading is the COMBINATION of both:
//
//     violation = the two shared trips have DIFFERENT `service_id` values **and** are active
//                 AT THE SAME TIME on at least one service day
//
// This preserves genuine 1-to-n splits (identical `service_id` → silent) and separate
// continuations (non-overlapping days → silent); it reports only the ambiguous case:
// on that day the trip belongs to TWO different continuations and the consumer cannot know which to choose.

fn check_linked_trip_continuations(
    records: &EntityRecords,
    derived: &DerivedData,
    notices: &mut Vec<Notice>,
    ctr: &mut u32,
) {
    let ti = &records.trip_interns;
    let mut service_of: HashMap<&str, &str> = HashMap::new();
    for t in &records.trips {
        let sid = ti.service_id(t);
        if !sid.is_empty() {
            service_of.insert(t.trip_id.as_str(), sid);
        }
    }
    if service_of.is_empty() {
        return;
    }

    // transfer_type 4/5 = linked trip. Missing trip_id → TRF_013's domain; undefined trip_id →
    // TRF_006/007's domain; from==to → TRF_018's domain. All three are silently excluded here,
    // otherwise the same defect is reported twice.
    let mut by_from: HashMap<&str, Vec<(&str, u64)>> = HashMap::new();
    let mut by_to: HashMap<&str, Vec<(&str, u64)>> = HashMap::new();
    for trf in &records.transfers {
        if !matches!(trf.transfer_type, Some(4) | Some(5)) {
            continue;
        }
        let (Some(f), Some(t)) = (trf.from_trip_id.as_deref(), trf.to_trip_id.as_deref()) else {
            continue;
        };
        if f == t || !service_of.contains_key(f) || !service_of.contains_key(t) {
            continue;
        }
        by_from.entry(f).or_default().push((t, trf.line));
        by_to.entry(t).or_default().push((f, trf.line));
    }

    let active = &derived.calendar_bitmap.active_dates;
    // Return the FIRST contradiction in a group: (tripA, serviceA, tripB, serviceB, shared day,
    // row). One notice per group is enough — counting every pair for the same ambiguity is noise.
    let first_conflict =
        |partners: &[(&str, u64)]| -> Option<(String, String, String, String, u32, u64)> {
            // FIRST trip per service (sorted by trip_id) → deterministic output.
            let mut sorted: Vec<(&str, u64)> = partners.to_vec();
            sorted.sort_unstable();
            sorted.dedup_by_key(|(t, _)| *t);
            let mut per_service: Vec<(&str, &str, u64)> = Vec::new(); // (service_id, trip_id, line)
            for (trip, line) in sorted {
                let svc = service_of[trip];
                if !per_service.iter().any(|(s, _, _)| *s == svc) {
                    per_service.push((svc, trip, line));
                }
            }
            if per_service.len() < 2 {
                return None;
            }
            for i in 0..per_service.len() {
                for j in (i + 1)..per_service.len() {
                    let (svc_a, trip_a, _) = per_service[i];
                    let (svc_b, trip_b, line_b) = per_service[j];
                    // If the calendar cannot be resolved (calendar/calendar_dates missing or invalid), the conflict
                    // is UNPROVABLE → stay silent. An empty set means "no conflict", not "I don't know",
                    // but neither case has anything to report.
                    let (Some(days_a), Some(days_b)) = (active.get(svc_a), active.get(svc_b))
                    else {
                        continue;
                    };
                    let (small, large) = if days_a.len() <= days_b.len() {
                        (days_a, days_b)
                    } else {
                        (days_b, days_a)
                    };
                    if let Some(day) = small.iter().filter(|d| large.contains(*d)).min() {
                        return Some((
                            trip_a.to_string(),
                            svc_a.to_string(),
                            trip_b.to_string(),
                            svc_b.to_string(),
                            *day,
                            line_b,
                        ));
                    }
                }
            }
            None
        };

    // HashMap traversal order is nondeterministic → sort the keys.
    let mut from_keys: Vec<&str> = by_from.keys().copied().collect();
    from_keys.sort_unstable();
    for trip in from_keys {
        let Some((ta, sa, tb, sb, day, line)) = first_conflict(&by_from[trip]) else {
            continue;
        };
        notices.push(k6_notice(
            ctr,
            "TRF_022",
            EntityType::Trip,
            Some(trip.to_string()),
            Some(trip.to_string()),
            "transfers.txt",
            Some(line),
            Some("to_trip_id"),
            Some(format!("{ta} ({sa}) / {tb} ({sb})")),
            None,
            format!(
                "'{trip}' seferi birden çok sefere bağlanıyor ama devam seferlerinin takvimleri \
                 çelişiyor: '{ta}' → {sa}, '{tb}' → {sb}; ikisi de {day} günü aktif."
            ),
            "1-to-n devamlılıkta tüm to_trip_id seferleri aynı service_id'yi kullanmalıdır. \
             Bunlar ayrı devamlılıklarsa takvimleri hiçbir günde çakışmamalıdır.",
        ));
    }

    let mut to_keys: Vec<&str> = by_to.keys().copied().collect();
    to_keys.sort_unstable();
    for trip in to_keys {
        let Some((ta, sa, tb, sb, day, line)) = first_conflict(&by_to[trip]) else {
            continue;
        };
        notices.push(k6_notice(
            ctr,
            "TRF_023",
            EntityType::Trip,
            Some(trip.to_string()),
            Some(trip.to_string()),
            "transfers.txt",
            Some(line),
            Some("from_trip_id"),
            Some(format!("{ta} ({sa}) / {tb} ({sb})")),
            None,
            format!(
                "'{trip}' seferine birden çok sefer bağlanıyor ama gelen seferlerin takvimleri \
                 çelişiyor: '{ta}' → {sa}, '{tb}' → {sb}; ikisi de {day} günü aktif."
            ),
            "n-to-1 devamlılıkta tüm from_trip_id seferleri aynı service_id'yi kullanmalıdır. \
             Bunlar ayrı devamlılıklarsa takvimleri hiçbir günde çakışmamalıdır.",
        ));
    }
}

// ── Takvim override analitik (OPR_019–023) ───────────────────────────────────

fn check_calendar_override_analytics(
    records: &EntityRecords,
    derived: &DerivedData,
    config: &ValidatorConfig,
    idx: &StopTimesIndex<'_>,
    notices: &mut Vec<Notice>,
    ctr: &mut u32,
) {
    use crate::timing::Timer;
    let ti_cal_ov = &records.trip_interns;
    // ── route_id → service_id set ─────────────────────────────────────────────
    let mut route_services: HashMap<&str, Vec<&str>> = HashMap::new();
    for trip in &records.trips {
        if !ti_cal_ov.route_id(trip).is_empty() && !ti_cal_ov.service_id(trip).is_empty() {
            route_services
                .entry(ti_cal_ov.route_id(trip))
                .or_default()
                .push(ti_cal_ov.service_id(trip));
        }
    }
    // Sort and deduplicate service lists.
    for svcs in route_services.values_mut() {
        svcs.sort_unstable();
        svcs.dedup();
    }

    // ── Services with calendar_dates exceptions (service_id → date membership) ──
    // OPR_020: HIGH on a conflict day if any conflicting service appears in this table.
    // CalendarDateIndex.added / .removed are already sorted with sort_unstable during parsing;
    // binary_search provides O(log n) membership queries — NO separate svc_date_exc map.
    // has_exc helper: does the service have any exception (type 1 or 2) for the given date?
    let has_exc_date = |svc: &str, date: u32| -> bool {
        records
            .calendar_dates
            .added
            .get(svc)
            .is_some_and(|v| v.binary_search(&date).is_ok())
            || records
                .calendar_dates
                .removed
                .get(svc)
                .is_some_and(|v| v.binary_search(&date).is_ok())
    };

    // ── Operational trip identity (OPR_019/OPR_020 conflict definition) ──────────
    //
    // Having ≥2 services active on the same day for a route is NOT a conflict BY ITSELF. Two
    // trains in opposite directions (Ankara→Kurtalan 22014 and Kurtalan→Ankara 52013) use
    // separate service_id values; different train numbers defined as extra trips on the same route
    // are also legitimate. The old definition (route_id + date + ≥2 service_id) counted all of these as conflicts.
    //
    // A real conflict: the SAME operational trip is active from two services on that day —
    // that is, both the base calendar and an override calendar produce the same trip. Trip identity
    // = direction + train name (trip_short_name) + first departure time + ordered stop sequence.
    //
    // Perf: signatures are computed only for routes with ≥2 services.
    let multi_svc_routes: FxHashSet<&str> = route_services
        .iter()
        .filter(|(_, svcs)| svcs.len() >= 2)
        .map(|(&r, _)| r)
        .collect();
    // route_id → (service_id → that service's operational trip signatures)
    let mut route_svc_sigs: HashMap<&str, HashMap<&str, FxHashSet<u64>>> = HashMap::new();
    if !multi_svc_routes.is_empty() {
        use std::hash::{Hash, Hasher};
        for trip in &records.trips {
            let route_id = ti_cal_ov.route_id(trip);
            if !multi_svc_routes.contains(route_id) {
                continue;
            }
            let svc_id = ti_cal_ov.service_id(trip);
            if svc_id.is_empty() {
                continue;
            }
            let Some(stops) = idx.by_trip.get(trip.trip_id.as_str()) else {
                continue;
            };
            if stops.is_empty() {
                continue;
            }
            let mut h = std::collections::hash_map::DefaultHasher::new();
            trip.direction_id.unwrap_or(u32::MAX).hash(&mut h);
            ti_cal_ov.short_name(trip).unwrap_or("").hash(&mut h);
            stops
                .first()
                .and_then(|s| s.departure_time())
                .map(hms_to_secs)
                .unwrap_or(u32::MAX)
                .hash(&mut h);
            for st in stops.iter() {
                idx.stop_id_of(st).hash(&mut h);
            }
            route_svc_sigs
                .entry(route_id)
                .or_default()
                .entry(svc_id)
                .or_default()
                .insert(h.finish());
        }
    }
    // Do any services active on one day share the SAME signature?
    let day_has_conflict = |route_id: &str, svcs: &[&str]| -> bool {
        let Some(svc_sigs) = route_svc_sigs.get(route_id) else {
            return false;
        };
        let mut seen: FxHashSet<u64> = FxHashSet::default();
        for &s in svcs {
            let Some(sigs) = svc_sigs.get(s) else {
                continue;
            };
            for &sig in sigs {
                if !seen.insert(sig) {
                    return true;
                }
            }
        }
        false
    };

    // ── OPR_019 / OPR_020: conflict analysis without config ─────────────
    let mut date_services: HashMap<u32, Vec<&str>> = HashMap::new();

    for (&route_id, service_ids) in &route_services {
        if service_ids.len() < 2 {
            // One service → no conflict possible.
            continue;
        }

        date_services.clear();
        for &svc_id in service_ids {
            if let Some(dates) = derived.calendar_bitmap.active_dates.get(svc_id) {
                for &date in dates {
                    date_services.entry(date).or_default().push(svc_id);
                }
            }
        }

        // Sort conflict days.
        let mut plain_dates: Vec<u32> = Vec::new(); // no exception
        let mut exc_dates: Vec<u32> = Vec::new(); // day with an exception for at least one service
        let mut plain_patterns: Vec<String> = Vec::new();
        let mut exc_patterns: Vec<String> = Vec::new();

        for (&date, svcs) in &date_services {
            if svcs.len() < 2 {
                continue;
            }
            // If the same operational trip is not active from more than one service, there is no conflict.
            if !day_has_conflict(route_id, svcs) {
                continue;
            }
            let has_exc = svcs.iter().any(|&s| has_exc_date(s, date));
            let mut sorted_svcs = svcs.clone();
            sorted_svcs.sort_unstable();
            let pattern = sorted_svcs.join("+");
            if has_exc {
                exc_dates.push(date);
                exc_patterns.push(pattern);
            } else {
                plain_dates.push(date);
                plain_patterns.push(pattern);
            }
        }

        if !plain_dates.is_empty() {
            plain_dates.sort_unstable();
            let count = plain_dates.len();
            let sample_str: String = plain_dates
                .iter()
                .take(3)
                .map(|d| d.to_string())
                .collect::<Vec<_>>()
                .join(", ");
            let patterns_str = summarize_patterns(&plain_patterns);
            let msg = format!(
                "'{}' hattında {} günde birden fazla aktif servis var. Örnek tarihler: {}.",
                route_id, count, sample_str
            );
            let mut n = k6_notice(
                ctr,
                "OPR_019",
                EntityType::Route,
                Some(route_id.to_string()),
                Some(route_id.to_string()),
                "trips.txt",
                None,
                None,
                Some(format!("{count}")),
                Some("≤ 1/day".to_string()),
                msg,
                "Her takvim günü için yalnızca bir servis aktif olacak şekilde düzenleyin.",
            );
            n.details = Some({
                let mut d = std::collections::BTreeMap::new();
                d.insert("conflict_day_count".to_string(), count.to_string());
                d.insert(
                    "sample_dates".to_string(),
                    plain_dates
                        .iter()
                        .take(10)
                        .map(|x| x.to_string())
                        .collect::<Vec<_>>()
                        .join(","),
                );
                d.insert("active_service_patterns".to_string(), patterns_str);
                d.into()
            });
            notices.push(n);
        }

        if !exc_dates.is_empty() {
            exc_dates.sort_unstable();
            let count = exc_dates.len();
            let patterns_str = summarize_patterns(&exc_patterns);
            // Show the conflicting service_id values next to each example date:
            // "20260720 (X, Y)". Keep this language-neutral (date + service_id values only) so
            // the Turkish message and the en/ja templates can safely use the same string.
            let sample_detail: String = exc_dates
                .iter()
                .take(3)
                .map(|d| {
                    let svcs = date_services
                        .get(d)
                        .map(|v| {
                            let mut s = v.clone();
                            s.sort_unstable();
                            s.join(", ")
                        })
                        .unwrap_or_default();
                    if svcs.is_empty() {
                        d.to_string()
                    } else {
                        format!("{d} ({svcs})")
                    }
                })
                .collect::<Vec<_>>()
                .join("; ");
            let msg = format!(
                "'{}' hattında {} exception gününde birden fazla aktif servis var — override çakışması riski. Örnek tarihler (parantezde çakışan çalışma takvimleri): {}.",
                route_id, count, sample_detail
            );
            let mut n = k6_notice(
                ctr, "OPR_020", EntityType::Route,
                Some(route_id.to_string()), Some(route_id.to_string()),
                "trips.txt", None, None,
                Some(format!("{count}")),
                Some("≤ 1/day".to_string()),
                msg,
                "Override günlerinde base servisi calendar_dates.txt ile kaldırın (exception_type=2).",
            );
            n.details = Some({
                let mut d = std::collections::BTreeMap::new();
                d.insert("conflict_day_count".to_string(), count.to_string());
                d.insert(
                    "sample_dates".to_string(),
                    exc_dates
                        .iter()
                        .take(10)
                        .map(|x| x.to_string())
                        .collect::<Vec<_>>()
                        .join(","),
                );
                d.insert("sample_dates_detail".to_string(), sample_detail);
                d.insert("active_service_patterns".to_string(), patterns_str);
                d.into()
            });
            notices.push(n);
        }
    }

    // ── OPR_021 / OPR_022 / OPR_023: config-only ─────────────────────────────
    for rule in &config.calendar_override_rules {
        let base_svc_set: FxHashSet<&str> =
            rule.base_service_ids.iter().map(|s| s.as_str()).collect();
        let override_svc_set: FxHashSet<&str> = rule
            .override_service_ids
            .iter()
            .map(|s| s.as_str())
            .collect();

        let start_jdn = yyyymmdd_to_jdn(rule.start_date);
        let end_jdn = yyyymmdd_to_jdn(rule.end_date);

        let mut dates_021: Vec<u32> = Vec::new();
        let mut dates_022: Vec<u32> = Vec::new();
        let mut dates_023: Vec<u32> = Vec::new();

        for jdn in start_jdn..=end_jdn {
            let date = jdn_to_yyyymmdd(jdn);
            let base_active = base_svc_set.iter().any(|&s| {
                derived
                    .calendar_bitmap
                    .active_dates
                    .get(s)
                    .is_some_and(|set| set.contains(&date))
            });
            let override_active = override_svc_set.iter().any(|&s| {
                derived
                    .calendar_bitmap
                    .active_dates
                    .get(s)
                    .is_some_and(|set| set.contains(&date))
            });
            match (base_active, override_active) {
                (true, true) => dates_021.push(date),
                (true, false) => dates_022.push(date),
                (false, false) => dates_023.push(date),
                (false, true) => {} // correct: base removed, override added
            }
        }

        let base_str = rule.base_service_ids.join(",");
        let override_str = rule.override_service_ids.join(",");

        if !dates_021.is_empty() {
            let count = dates_021.len();
            let msg = format!(
                "'{}' hattında {} günde override servisi ({}) aktif ancak base servisi ({}) de aktif kalmış. İlk gün: {}.",
                rule.route_id, count, override_str, base_str, dates_021[0]
            );
            let mut n = k6_notice(
                ctr,
                "OPR_021",
                EntityType::Route,
                Some(rule.route_id.clone()),
                Some(rule.route_id.clone()),
                "calendar_dates.txt",
                None,
                None,
                None,
                None,
                msg,
                "Override gününde base servisi calendar_dates.txt ile kaldırın (exception_type=2).",
            );
            n.details = Some(build_override_details(
                count,
                &dates_021,
                &base_str,
                &override_str,
            ));
            notices.push(n);
        }

        if !dates_022.is_empty() {
            let count = dates_022.len();
            let msg = format!(
                "'{}' hattında {} günde override servisi ({}) bekleniyor ancak aktif değil; base servis ({}) çalışıyor. İlk gün: {}.",
                rule.route_id, count, override_str, base_str, dates_022[0]
            );
            let mut n = k6_notice(
                ctr, "OPR_022", EntityType::Route,
                Some(rule.route_id.clone()), Some(rule.route_id.clone()),
                "calendar_dates.txt", None, None, None, None, msg,
                "Override günü için override servisini calendar_dates.txt ile ekleyin (exception_type=1).",
            );
            n.details = Some(build_override_details(
                count,
                &dates_022,
                &base_str,
                &override_str,
            ));
            notices.push(n);
        }

        if !dates_023.is_empty() {
            let count = dates_023.len();
            let msg = format!(
                "'{}' hattında {} günde ne base ({}) ne override ({}) servisi aktif — hat servissiz kalmış. İlk gün: {}.",
                rule.route_id, count, base_str, override_str, dates_023[0]
            );
            let mut n = k6_notice(
                ctr,
                "OPR_023",
                EntityType::Route,
                Some(rule.route_id.clone()),
                Some(rule.route_id.clone()),
                "calendar_dates.txt",
                None,
                None,
                None,
                None,
                msg,
                "Override penceresinde hem base hem override servisini aktif tutun.",
            );
            n.details = Some(build_override_details(
                count,
                &dates_023,
                &base_str,
                &override_str,
            ));
            notices.push(n);
        }
    }

    // ── OPR_004: no weekend trips on a route ─────────────────────────────────
    if !derived.calendar_bitmap.active_dates.is_empty() {
        let _t04 = Timer::start("K6::rem::opr_004");
        let route_short_o4: FxHashMap<&str, &str> = records
            .routes
            .iter()
            .map(|r| {
                (
                    r.route_id.as_str(),
                    r.route_short_name
                        .as_deref()
                        .filter(|s| !s.is_empty())
                        .unwrap_or(r.route_id.as_str()),
                )
            })
            .collect();
        let mut route_services: FxHashMap<&str, FxHashSet<&str>> = FxHashMap::default();
        for t in &records.trips {
            if !ti_cal_ov.route_id(t).is_empty() && !ti_cal_ov.service_id(t).is_empty() {
                route_services
                    .entry(ti_cal_ov.route_id(t))
                    .or_default()
                    .insert(ti_cal_ov.service_id(t));
            }
        }
        for (route_id, service_ids) in &route_services {
            let has_weekend = service_ids.iter().any(|&svc| {
                derived
                    .calendar_bitmap
                    .active_dates
                    .get(svc)
                    .is_some_and(|dates| {
                        dates.iter().any(|&d| {
                            let jdn = yyyymmdd_to_jdn(d);
                            jdn % 7 == 5 || jdn % 7 == 6 // Cumartesi=5, Pazar=6
                        })
                    })
            });
            if !has_weekend {
                let label = route_short_o4.get(*route_id).copied().unwrap_or(*route_id);
                let mut notice = k6_notice(
                    ctr, "OPR_004", EntityType::Route,
                    Some(route_id.to_string()), Some(route_id.to_string()),
                    "trips.txt", None, None, None, None,
                    format!("'{label}' kodlu hattın aktif servislerinde hafta sonu (Cumartesi/Pazar) günü yok."),
                    "Bu bilgi notudur; aksiyon gerekmez.",
                );
                // The en/ja/fr templates are filled with `{route_label}`.
                let details = notice.details.get_or_insert_with(Default::default);
                details.insert("route_label".to_string(), label.to_string());
                notices.push(notice);
            }
        }
    }

    // ── OPR_012: large gap in the service period ────────────────────────────
    {
        let _t12 = Timer::start("K6::rem::opr_012");
        let gap_threshold = config.service_gap_days;
        let used_services_12: FxHashSet<&str> = records
            .trips
            .iter()
            .filter(|t| !ti_cal_ov.service_id(t).is_empty())
            .map(|t| ti_cal_ov.service_id(t))
            .collect();
        for (svc_id, dates) in &derived.calendar_bitmap.active_dates {
            if dates.len() < 2 {
                continue;
            }
            if !used_services_12.contains(svc_id.as_str()) {
                continue;
            }
            let mut sorted: Vec<u32> = dates.iter().copied().collect();
            sorted.sort_unstable();
            let mut max_gap = 0u32;
            let mut gap_start = 0u32;
            let mut gap_end = 0u32;
            for w in sorted.windows(2) {
                // Same counting as CAL_007: missing days between two dates (exclusive).
                let d = yyyymmdd_to_jdn(w[1])
                    .saturating_sub(yyyymmdd_to_jdn(w[0]))
                    .saturating_sub(1);
                if d > max_gap {
                    max_gap = d;
                    gap_start = w[0];
                    gap_end = w[1];
                }
            }
            if max_gap >= gap_threshold {
                let mut n012 = k6_notice(
                    ctr, "OPR_012", EntityType::Service,
                    Some(svc_id.clone()), Some(svc_id.clone()),
                    "calendar.txt", None, None,
                    Some(format!("{max_gap}")),
                    Some(format!("≤ {gap_threshold}")),
                    format!("'{svc_id}' servisinde {gap_start}-{gap_end} arasında {max_gap} günlük servis boşluğu var."),
                    "Boşluk planlanmışsa görmezden gelin; aksi hâlde eksik günleri calendar_dates.txt ile ekleyin.",
                );
                n012.service_id = Some(svc_id.clone());
                notices.push(n012);
            }
        }
    }

    // ── OPR_015: only one shape defined on a route ───────────────────────────
    {
        let _t15b = Timer::start("K6::rem::opr_015");
        let mut route_shape_set: FxHashMap<&str, FxHashSet<&str>> = FxHashMap::default();
        let mut route_dirs: FxHashMap<&str, FxHashSet<u32>> = FxHashMap::default();
        for t in &records.trips {
            if ti_cal_ov.route_id(t).is_empty() {
                continue;
            }
            if let Some(shape) = ti_cal_ov.shape_id(t).filter(|s| !s.is_empty()) {
                route_shape_set
                    .entry(ti_cal_ov.route_id(t))
                    .or_default()
                    .insert(shape);
            }
            if let Some(d) = t.direction_id {
                route_dirs
                    .entry(ti_cal_ov.route_id(t))
                    .or_default()
                    .insert(d);
            }
        }
        // route_id → route_type (skip for rail systems).
        let rt_map: FxHashMap<&str, u32> = records
            .routes
            .iter()
            .filter_map(|r| r.route_type.map(|rt| (r.route_id.as_str(), rt)))
            .collect();
        let route_short_o15: FxHashMap<&str, &str> = records
            .routes
            .iter()
            .map(|r| {
                (
                    r.route_id.as_str(),
                    r.route_short_name
                        .as_deref()
                        .filter(|s| !s.is_empty())
                        .unwrap_or(r.route_id.as_str()),
                )
            })
            .collect();
        for (route_id, shapes) in &route_shape_set {
            // Tram(0), Metro(1), Railway(2), Cable tram(5), Monorail(12):
            // one-way rail route; one shape is expected behavior.
            let rt = rt_map.get(*route_id).copied().unwrap_or(3);
            if matches!(rt, 0 | 1 | 2 | 5 | 12) {
                continue;
            }
            // OPR_015 is meaningful only for TWO-WAY routes: if both directions share one shape,
            // recommend "a separate shape for each direction". On a one-way / directionless route,
            // one shape is expected behavior (the recommendation would mislead) → stay silent.
            let bidirectional = route_dirs.get(*route_id).is_some_and(|d| d.len() > 1);
            if shapes.len() == 1 && bidirectional {
                let shape_id = shapes.iter().next().copied().unwrap_or("");
                let mut n015 = k6_notice(
                    ctr, "OPR_015", EntityType::Route,
                    Some(route_id.to_string()), Some(route_id.to_string()),
                    "trips.txt", None, Some("shape_id"),
                    Some(format!("1 shape ({shape_id})")), None,
                    format!("'{}' kodlu hattın tüm seferleri tek bir güzergah şekli kullanıyor ({shape_id}).", route_short_o15.get(*route_id).copied().unwrap_or(*route_id)),
                    "Bu bilgi notu; gidiş-dönüş için ayrı shape_id kullanılması önerilir.",
                );
                let mut d = std::collections::BTreeMap::new();
                d.insert("shape_id".to_string(), shape_id.to_string());
                d.insert(
                    "route_label".to_string(),
                    route_short_o15
                        .get(*route_id)
                        .copied()
                        .unwrap_or(*route_id)
                        .to_string(),
                );
                n015.details = Some(d.into());
                notices.push(n015);
            }
        }
    }
}

/// Count and summarize unique service combinations: "SVC_A+SVC_B: 12 days; SVC_A+SVC_C: 3 days".
fn summarize_patterns(patterns: &[String]) -> String {
    let mut counts: std::collections::BTreeMap<&str, usize> = std::collections::BTreeMap::new();
    for p in patterns {
        *counts.entry(p.as_str()).or_default() += 1;
    }
    counts
        .iter()
        .map(|(k, &v)| format!("{}: {} gün", k, v))
        .collect::<Vec<_>>()
        .join("; ")
}

/// Build the details HashMap for OPR_021/022/023.
fn build_override_details(
    count: usize,
    dates: &[u32],
    base_str: &str,
    override_str: &str,
) -> gtfs_core::NoticeDetails {
    let mut d = gtfs_core::NoticeDetails::new();
    d.insert("conflict_day_count".to_string(), count.to_string());
    d.insert(
        "sample_dates".to_string(),
        dates
            .iter()
            .take(10)
            .map(|x| x.to_string())
            .collect::<Vec<_>>()
            .join(","),
    );
    d.insert("base_services".to_string(), base_str.to_string());
    d.insert("override_services".to_string(), override_str.to_string());
    d
}

// ── WP-14d: Pathway analitik (PTH_012 / PTH_013 / PTH_015) ──────────────────

fn check_pathway_analytics(
    records: &EntityRecords,
    derived: &DerivedData,
    notices: &mut Vec<Notice>,
    ctr: &mut u32,
) {
    if records.pathways.is_empty() {
        return;
    }

    // stop_id → parent_station
    let stop_parent: HashMap<&str, &str> = records
        .stops
        .iter()
        .filter_map(|s| {
            s.row
                .get("parent_station")
                .map(|p| (s.stop_id.as_str(), p.trim()))
        })
        .filter(|(_, p)| !p.is_empty())
        .collect();

    // Station (location_type=1) → child stops.
    let mut station_children: HashMap<&str, Vec<&crate::k2::stops::StopRecord>> = HashMap::new();
    for stop in &records.stops {
        if let Some(&parent) = stop_parent.get(stop.stop_id.as_str()) {
            station_children.entry(parent).or_default().push(stop);
        }
    }

    // Stops present in the pathway graph.
    let graph_stops: HashSet<&str> = derived
        .pathway_graph
        .adjacency
        .keys()
        .map(String::as_str)
        .collect();

    // Wheelchair-accessible pathway indeksleri:
    // max_slope ≤ 8% (0.08) AND min_width ≥ 0.9 m — passes if both are None.
    let accessible_pw_idx: HashSet<usize> = records
        .pathways
        .iter()
        .enumerate()
        .filter(|(_, pw)| {
            pw.max_slope.is_none_or(|s| s.abs() <= 0.08) && pw.min_width.is_none_or(|w| w >= 0.9)
        })
        .map(|(i, _)| i)
        .collect();

    // ── PTH_012 and PTH_013: for each station ─────────────────────────────────
    for station in records.stops.iter().filter(|s| s.location_type == Some(1)) {
        let station_id = station.stop_id.as_str();
        if station_id.is_empty() {
            continue;
        }
        let children = match station_children.get(station_id) {
            Some(c) if !c.is_empty() => c,
            _ => continue,
        };

        // Is there a pathway connection belonging to this station?
        let any_connected = children
            .iter()
            .any(|s| graph_stops.contains(s.stop_id.as_str()));
        if !any_connected {
            continue;
        }

        let entrances: Vec<&str> = children
            .iter()
            .filter(|s| s.location_type == Some(2))
            .map(|s| s.stop_id.as_str())
            .collect();
        let platforms: Vec<&str> = children
            .iter()
            .filter(|s| matches!(s.location_type, None | Some(0) | Some(4)))
            .map(|s| s.stop_id.as_str())
            .collect();

        if platforms.is_empty() {
            continue;
        }

        // Without an entrance → every platform in the pathway graph is inaccessible without an entrance.
        if entrances.is_empty() {
            for &platform in &platforms {
                if graph_stops.contains(platform) {
                    notices.push(k6_notice(
                        ctr,
                        "PTH_012",
                        EntityType::Stop,
                        Some(platform.to_string()),
                        Some(platform.to_string()),
                        "stops.txt",
                        None,
                        None,
                        None,
                        None,
                        format!(
                            "Platform '{platform}' (istasyon '{station_id}') pathway ağında tanımlanmış ama istasyonda location_type=2 entrance yok.",
                        ),
                        "İstasyona giriş noktası (location_type=2) ekleyin ve pathway bağlantılarını tamamlayın.",
                    ));
                }
            }
            continue;
        }

        // Find all stops reachable from the entrance set by BFS (full graph).
        let reachable_from_entrances: HashSet<&str> = {
            let mut visited: HashSet<&str> = HashSet::new();
            let mut queue = std::collections::VecDeque::new();
            for &e in &entrances {
                if visited.insert(e) {
                    queue.push_back(e);
                }
            }
            while let Some(curr) = queue.pop_front() {
                if let Some(neighbors) = derived.pathway_graph.adjacency.get(curr) {
                    for (next, _) in neighbors {
                        if visited.insert(next.as_str()) {
                            queue.push_back(next.as_str());
                        }
                    }
                }
            }
            visited
        };

        // PTH_012: every platform must be reachable from at least one entrance.
        // In nested models (platform lt=0 → boarding area lt=4), the actual pathway node
        // is the boarding area; the platform container itself may not be in the graph. If one
        // child node is reachable from an entrance, the platform is also considered reachable
        // (same nested-station pattern as #50 and #49; mdb-3127 Santiago Metro 286 FP).
        for &platform in &platforms {
            let reachable = reachable_from_entrances.contains(platform)
                || station_children.get(platform).is_some_and(|kids| {
                    kids.iter()
                        .any(|k| reachable_from_entrances.contains(k.stop_id.as_str()))
                });
            if !reachable {
                notices.push(k6_notice(
                    ctr,
                    "PTH_012",
                    EntityType::Stop,
                    Some(platform.to_string()),
                    Some(platform.to_string()),
                    "pathways.txt",
                    None,
                    None,
                    None,
                    None,
                    format!(
                        "Platform '{platform}' (istasyon '{station_id}') hiçbir entrance'tan (location_type=2) pathway graph üzerinden ulaşılamıyor.",
                    ),
                    "Entrance ile bu platform arasındaki pathway bağlantılarını tamamlayın.",
                ));
            }
        }

        // PTH_013: on at least one entrance-to-platform path, every pathway must have
        // max_slope ≤ 8% AND min_width ≥ 0.9 m (wheelchair accessibility test).
        let accessible_reachable: HashSet<&str> = {
            let mut visited: HashSet<&str> = HashSet::new();
            let mut queue = std::collections::VecDeque::new();
            for &e in &entrances {
                if visited.insert(e) {
                    queue.push_back(e);
                }
            }
            while let Some(curr) = queue.pop_front() {
                if let Some(neighbors) = derived.pathway_graph.adjacency.get(curr) {
                    for (next, pw_idx) in neighbors {
                        if accessible_pw_idx.contains(pw_idx) && visited.insert(next.as_str()) {
                            queue.push_back(next.as_str());
                        }
                    }
                }
            }
            visited
        };

        // #51 (same nested-station pattern as #50): the platform container (lt0) may not be an
        // accessible graph node; if its boarding-area (lt4) child is reachable, the platform is
        // also considered reachable. Otherwise an accessible station is falsely treated as "unroutable".
        let any_platform_accessible = platforms.iter().any(|p| {
            accessible_reachable.contains(p)
                || station_children.get(p).is_some_and(|kids| {
                    kids.iter()
                        .any(|k| accessible_reachable.contains(k.stop_id.as_str()))
                })
        });

        if !any_platform_accessible {
            notices.push(k6_notice(
                ctr,
                "PTH_013",
                EntityType::Stop,
                Some(station_id.to_string()),
                Some(station_id.to_string()),
                "pathways.txt",
                None,
                None,
                Some("max_slope > 8% or min_width < 0.9m".to_string()),
                Some("max_slope ≤ 8% ve min_width ≥ 0.9m olan en az 1 path".to_string()),
                format!(
                    "İstasyon '{station_id}' — entrance'tan platforma tekerlekli sandalye ile erişilebilir pathway rotası yok (tüm pathlarda max_slope > 8% veya min_width < 0.9m).",
                ),
                "Accessibility koşullarını (max_slope ≤ 0.08, min_width ≥ 0.9) sağlayan bir entrance-to-platform rotası oluşturun.",
            ));
        }
    }

    // ── PTH_015: traversal_time and length consistency (reasonable speed) ────
    // Speed = length (m) / traversal_time (s); > 3.0 m/s → inconsistent.
    const MAX_PATHWAY_SPEED_MS: f64 = 3.0;

    for pw in &records.pathways {
        if pw.pathway_id.is_empty() {
            continue;
        }
        if let (Some(length), Some(tt)) = (pw.length, pw.traversal_time) {
            if length > 0.0 && tt > 0 {
                let speed = length / tt as f64;
                if speed > MAX_PATHWAY_SPEED_MS {
                    notices.push(k6_notice(
                        ctr,
                        "PTH_015",
                        EntityType::Pathway,
                        Some(pw.pathway_id.clone()),
                        Some(pw.pathway_id.clone()),
                        "pathways.txt",
                        Some(pw.line),
                        Some("traversal_time|length"),
                        Some(format!("{speed:.2} m/s ({length}m / {tt}s)")),
                        Some(format!("≤ {MAX_PATHWAY_SPEED_MS} m/s")),
                        format!(
                            "pathway_id '{}' length={length}m ve traversal_time={tt}s'den türetilen hız {speed:.2} m/s — makul sınır {MAX_PATHWAY_SPEED_MS} m/s.",
                            pw.pathway_id
                        ),
                        "traversal_time ve length değerlerini gerçekçi olarak güncelleyin.",
                    ));
                }
            }
        }
    }
}

// ── VAT: Entity Analytics Detection ──────────────────────────────────────────

/// Canonical ordering for VAT_005 connected components.
///
/// Component membership is independent of traversal order, but the notice keeps
/// a representative component and up to 200 stop IDs. Sort both levels so BFS,
/// DSU, hash-map iteration, and input ordering produce the same notice content.
fn canonicalize_stop_components(mut components: Vec<Vec<&str>>) -> Vec<Vec<&str>> {
    for component in &mut components {
        component.sort_unstable();
    }
    components.sort_unstable_by(|left, right| {
        right.len().cmp(&left.len()).then_with(|| {
            left.first()
                .copied()
                .unwrap_or("")
                .cmp(right.first().copied().unwrap_or(""))
        })
    });
    components
}

fn check_vat_analytics(
    records: &EntityRecords,
    derived: &DerivedData,
    config: &ValidatorConfig,
    idx: &StopTimesIndex<'_>,
    notices: &mut Vec<Notice>,
    ctr: &mut u32,
) {
    let ti_vat = &records.trip_interns;
    // ── Shared indexes ──────────────────────────────────────────────────────

    let route_label: FxHashMap<&str, &str> = records
        .routes
        .iter()
        .map(|r| {
            let label = r
                .route_short_name
                .as_deref()
                .filter(|s| !s.is_empty())
                .unwrap_or(r.route_id.as_str());
            (r.route_id.as_str(), label)
        })
        .collect();

    let route_type_map: FxHashMap<&str, u32> = records
        .routes
        .iter()
        .filter_map(|r| r.route_type.map(|rt| (r.route_id.as_str(), rt)))
        .collect();

    let trip_route: FxHashMap<&str, &str> = records
        .trips
        .iter()
        .map(|t| (t.trip_id.as_str(), ti_vat.route_id(t)))
        .collect();

    let stop_name_map: FxHashMap<&str, &str> = records
        .stops
        .iter()
        .filter_map(|s| s.stop_name.as_deref().map(|n| (s.stop_id.as_str(), n)))
        .collect();

    // One pass: route_stops, stop_routes, route_trip_count.
    let mut route_stops: FxHashMap<&str, FxHashSet<&str>> = FxHashMap::default();
    let mut stop_routes: FxHashMap<&str, FxHashSet<&str>> = FxHashMap::default();
    let mut route_trip_count: FxHashMap<&str, u32> = FxHashMap::default();

    for (&trip_id, stop_times) in &idx.by_trip {
        let route = match trip_route.get(trip_id) {
            Some(&r) => r,
            None => continue,
        };
        *route_trip_count.entry(route).or_insert(0) += 1;
        let route_set = route_stops.entry(route).or_default();
        for st in stop_times.iter() {
            if idx.stop_id_of(st).is_empty() {
                continue;
            }
            let sid = idx.stop_id_of(st);
            route_set.insert(sid);
            stop_routes.entry(sid).or_default().insert(route);
        }
    }

    // Stops appearing in transfers.
    let transfer_stops: FxHashSet<&str> = records
        .transfers
        .iter()
        .flat_map(|t| [t.from_stop_id.as_str(), t.to_stop_id.as_str()])
        .filter(|s| !s.is_empty())
        .collect();

    // ── VAT_001: route similarity (Jaccard ≥ 0.85) ───────────────────────────
    {
        // service_id values used by each route — for the calendar-overlap guard below.
        let route_services: FxHashMap<&str, FxHashSet<&str>> = {
            let mut m: FxHashMap<&str, FxHashSet<&str>> = FxHashMap::default();
            for t in &records.trips {
                m.entry(ti_vat.route_id(t))
                    .or_default()
                    .insert(ti_vat.service_id(t));
            }
            m
        };
        // Do both routes operate on any common day? (Do their service calendars overlap?)
        // A service_id is "dated" if it has resolvable active days; if both are dated
        // AND have no common day → they are temporally separate (not duplicates; see #29).
        let active = &derived.calendar_bitmap.active_dates;
        let route_has_dates = |svcs: &FxHashSet<&str>| -> bool {
            svcs.iter()
                .any(|s| active.get(*s).is_some_and(|d| !d.is_empty()))
        };
        let routes_share_day = |sa: &FxHashSet<&str>, sb: &FxHashSet<&str>| -> bool {
            for &x in sa {
                let Some(da) = active.get(x) else { continue };
                if da.is_empty() {
                    continue;
                }
                for &y in sb {
                    if let Some(db) = active.get(y) {
                        if da.intersection(db).next().is_some() {
                            return true;
                        }
                    }
                }
            }
            false
        };

        let route_list: Vec<(&str, &FxHashSet<&str>, u32)> = route_stops
            .iter()
            .filter(|(_, stops)| stops.len() >= 5)
            .filter_map(|(&rid, stops)| route_type_map.get(rid).map(|&rt| (rid, stops, rt)))
            .collect();

        // O(n²) → skip on large feeds.
        if route_list.len() <= 300 {
            for i in 0..route_list.len() {
                for j in (i + 1)..route_list.len() {
                    let (rid_a, stops_a, rt_a) = route_list[i];
                    let (rid_b, stops_b, rt_b) = route_list[j];
                    if rt_a != rt_b {
                        continue;
                    }
                    let inter = stops_a.iter().filter(|s| stops_b.contains(*s)).count();
                    let union = stops_a.len() + stops_b.len() - inter;
                    if union == 0 {
                        continue;
                    }
                    let jaccard = inter as f64 / union as f64;
                    if jaccard < 0.85 {
                        continue;
                    }

                    // Calendar-overlap guard (#29): the same public route may receive a new
                    // route_id during a service change (TriMet '20'/'20a': '20' summer schedule,
                    // '20a' autumn, separate date ranges). This is a temporal split, not a duplicate;
                    // the "merge" recommendation is invalid. If both routes have resolvable dates
                    // AND no common day, skip. If the calendar is missing (no evidence), emit as before.
                    let (Some(svcs_a), Some(svcs_b)) =
                        (route_services.get(rid_a), route_services.get(rid_b))
                    else {
                        continue;
                    };
                    if route_has_dates(svcs_a)
                        && route_has_dates(svcs_b)
                        && !routes_share_day(svcs_a, svcs_b)
                    {
                        continue;
                    }

                    let la = route_label.get(rid_a).copied().unwrap_or(rid_a);
                    let lb = route_label.get(rid_b).copied().unwrap_or(rid_b);
                    // If the label differs from route_id, show route_id too (so two different routes
                    // with the same short name do not produce an indistinguishable message like "'20' and '20'").
                    let disp = |rid: &str, label: &str| {
                        if label == rid {
                            format!("'{rid}'")
                        } else {
                            format!("'{label}' (route_id '{rid}')")
                        }
                    };
                    let mut n = k6_notice(
                        ctr,
                        "VAT_001",
                        EntityType::Route,
                        Some(rid_a.to_string()),
                        Some(rid_a.to_string()),
                        "routes.txt",
                        None,
                        Some("route_id"),
                        Some(format!("Jaccard={:.0}%", jaccard * 100.0)),
                        None,
                        format!("{} ve {} hatları %{:.0} oranda aynı durağı paylaşıyor — muhtemel kopya hat.",
                            disp(rid_a, la), disp(rid_b, lb), jaccard * 100.0),
                        "İki hattı birleştirin ya da güzergahları ayırt edilir biçimde farklılaştırın.",
                    );
                    // Map: provide both routes so they can be drawn in different colors.
                    // route_b: lets the EN/JA message template name the second route (details
                    // keys become parameters in tMsg).
                    let mut d = std::collections::BTreeMap::new();
                    d.insert("routes".to_string(), format!("{rid_a},{rid_b}"));
                    d.insert("route_b".to_string(), rid_b.to_string());
                    n.details = Some(d.into());
                    notices.push(n);
                }
            }
        }
    }

    // ── Transfer-gap prerequisite (SHARED by VAT_002 + VAT_007) ──────────────
    //
    // In GTFS, a transfer at the SAME stop_id is implicit: a consumer knows passengers can
    // get off and board there, so `transfers.txt` is not needed. That file defines connections
    // between DIFFERENT stops, minimum transfer time, or forbidden transfers. Therefore the
    // suggestion "this stop has 4 routes, define a transfer" was wrong by itself — at a terminal
    // like Ankara Gar, where all trains use the same stop_id, there is nothing to define.
    //
    // The signal is meaningful only if a nearby, SEPARATE stop exists where a transfer could
    // be defined. Also exclude stops with `parent_station`: the complex is already modeled
    // (the old code excluded only the location_type=1 station ITSELF, not its platform children).
    let has_parent: FxHashSet<&str> = records
        .stops
        .iter()
        .filter(|s| !crate::k6_analytics::row_field_local(&s.row, "parent_station").is_empty())
        .map(|s| s.stop_id.as_str())
        .collect();

    // Latitude-sorted stop list (following STP_017): band-limited neighbor scan.
    let xfer_sorted_stops: Vec<(f64, f64, &str)> = {
        let mut v: Vec<(f64, f64, &str)> = records
            .stops
            .iter()
            .filter_map(|s| recoverable_stop_coord(s).map(|(la, lo)| (la, lo, s.stop_id.as_str())))
            .collect();
        v.sort_by(|a, b| a.0.partial_cmp(&b.0).unwrap_or(std::cmp::Ordering::Equal));
        v
    };
    let xfer_stop_pos: FxHashMap<&str, (f64, f64)> = xfer_sorted_stops
        .iter()
        .map(|&(la, lo, id)| (id, (la, lo)))
        .collect();

    // Is there a nearby separate stop serving a route that this stop DOES NOT serve?
    // If so, there is a genuinely undefined transfer between stops.
    let has_nearby_unlinked_route = |stop_id: &str, own: &FxHashSet<&str>| -> bool {
        let Some(&(la, lo)) = xfer_stop_pos.get(stop_id) else {
            return false;
        };
        let max_km = config.max_transfer_distance_m / 1000.0;
        let band = max_km / 111.0;
        let start = xfer_sorted_stops.partition_point(|&(l, _, _)| l < la - band);
        for &(nla, nlo, nid) in &xfer_sorted_stops[start..] {
            if nla > la + band {
                break;
            }
            if nid == stop_id {
                continue;
            }
            if haversine_km(la, lo, nla, nlo) > max_km {
                continue;
            }
            if let Some(nr) = stop_routes.get(nid) {
                if nr.iter().any(|r| !own.contains(r)) {
                    return true;
                }
            }
        }
        false
    };

    // ── VAT_002: undefined transfer hub (≥ 4 routes, no transfer) ────────────
    {
        let is_station: FxHashSet<&str> = records
            .stops
            .iter()
            .filter(|s| s.location_type == Some(1))
            .map(|s| s.stop_id.as_str())
            .collect();

        for (&stop_id, routes) in &stop_routes {
            if routes.len() < 4 {
                continue;
            }
            if transfer_stops.contains(stop_id) {
                continue;
            }
            if is_station.contains(stop_id) {
                continue;
            }
            if has_parent.contains(stop_id) {
                continue;
            }
            if !has_nearby_unlinked_route(stop_id, routes) {
                continue;
            }
            let name = stop_name_map.get(stop_id).copied().unwrap_or(stop_id);
            // Collect routes in stable order: route_ids for the map, readable labels for the message.
            let mut route_ids: Vec<&str> = routes.iter().copied().collect();
            route_ids.sort_unstable();
            let route_labels: Vec<&str> = route_ids
                .iter()
                .map(|rid| route_label.get(rid).copied().unwrap_or(rid))
                .collect();
            let routes_label_str = route_labels.join(", ");
            let mut n002 = k6_notice(
                ctr,
                "VAT_002",
                EntityType::Stop,
                Some(stop_id.to_string()),
                None,
                "stops.txt",
                None,
                Some("stop_id"),
                Some(format!("{} routes", routes.len())),
                None,
                format!("'{name}' (kod: '{stop_id}') durağından {} hat geçiyor ({routes_label_str}); transfers.txt'te aktarma tanımlı değil.",
                    routes.len()),
                "transfers.txt'e bu durak için aktarma kayıtları ekleyin (transfer_type=2 ile bekleme süresi belirtin).",
            );
            n002.details = Some({
                let mut d = std::collections::BTreeMap::new();
                d.insert("routes".to_string(), route_ids.join(","));
                d.into()
            });
            notices.push(n002);
        }
    }

    // ── VAT_003: statistical trip-duration outlier (MAD method) ──────────────
    // The grouping key is NOT route_id, but the trip PATTERN: one route_id often contains
    // multiple patterns (short-turn, variant, direction) whose durations naturally differ;
    // comparing all of them to one median creates mass false positives.
    // The key is shape_id + ordered stop pattern. The same shape may be shared by different
    // stop sequences such as a short-turn and a full trip; grouping only by shape_id treats
    // their natural duration difference as an outlier (BART Green-N: 5 stops/23 min vs 22 stops/83 min).
    // Without a shape, fall back to the route_id + ordered stop-sequence hash.
    {
        let trip_shape: FxHashMap<&str, &str> = records
            .trips
            .iter()
            .filter_map(|t| ti_vat.shape_id(t).map(|s| (t.trip_id.as_str(), s)))
            .collect();

        #[derive(PartialEq, Eq, Hash)]
        enum DurKey<'a> {
            ShapePattern(&'a str, u64),
            Pattern(&'a str, u64),
        }

        let mut groups: FxHashMap<DurKey, Vec<(&str, u32, u32, &str)>> = FxHashMap::default();
        for (&trip_id, stops) in &idx.by_trip {
            if stops.len() < 2 {
                continue;
            }
            let route = match trip_route.get(trip_id) {
                Some(&r) => r,
                None => continue,
            };
            let first_dep = stops
                .first()
                .and_then(|s| s.departure_time())
                .map(hms_to_secs);
            let last_arr = stops.last().and_then(|s| s.arrival_time()).map(hms_to_secs);
            if let (Some(dep), Some(arr)) = (first_dep, last_arr) {
                if arr > dep {
                    // Hash the ORDERED stop sequence. Stop count is not distinctive enough:
                    // different patterns with the same number of stops can be mixed together.
                    use std::hash::{Hash, Hasher};
                    let mut h = std::collections::hash_map::DefaultHasher::new();
                    for st in stops.iter() {
                        idx.stop_id_of(st).hash(&mut h);
                    }
                    let pattern_hash = h.finish();
                    let key = match trip_shape.get(trip_id) {
                        Some(&sh) => DurKey::ShapePattern(sh, pattern_hash),
                        None => DurKey::Pattern(route, pattern_hash),
                    };
                    groups
                        .entry(key)
                        .or_default()
                        .push((trip_id, arr - dep, dep, route));
                }
            }
        }

        // Intra-day (peak/off-peak) deseasonalization: measure duration's deviation from the
        // expected value for its departure HOUR. If an hour has ≥BUCKET_MIN_TRIPS, use that
        // hour's median; otherwise fall back to the pattern-global median (avoids data starvation
        // in sparse hours). Robust-z is now on the residual; the minimum-minute floor is
        // complementary (prevents flagging small deviations with tiny MAD). See issue #24.
        const BUCKET_MIN_TRIPS: usize = 5;
        // Operational minimum deviation: residuals smaller than this (intra-day noise) are never flagged.
        // Deseasonalization compresses residuals, making σ alone overly sensitive; this floor acts
        // as an AND gate that filters small deviations (the issue #24 contributor's "minimum-minute
        // complement" note).
        const RESIDUAL_FLOOR_SECS: f64 = 300.0;

        for (_key, trips) in groups {
            // ── Date-expansion collapse ─────────────────────────────────────
            // Some producers (TCDD pattern) write the same schedule with a SEPARATE trip_id
            // for every operating day. Records with identical pattern + departure + duration
            // are one SCHEDULE, not separate trips. Without collapsing them, two harms occurred:
            //   (1) Report inflation — 4 real schedules produced 40 notices.
            //   (2) Statistical distortion — 10 identical copies made up a quarter of the group,
            //       artificially increasing MAD and masking REAL outliers.
            // Therefore collapse before emission, in the statistical input.
            //
            // ⚠️ The ≥5 gate now counts UNIQUE SCHEDULES. 40 trips / 4 schedules is not enough
            // evidence for an outlier claim (robust-z is meaningless with 4 observations) → stay
            // silent. This is intentional: the old behavior counted the same evidence 10 times
            // and manufactured confidence.
            let mut sched: FxHashMap<(u32, u32), (&str, u32, &str)> = FxHashMap::default();
            for &(trip_id, dur, dep, route) in &trips {
                let e = sched.entry((dur, dep)).or_insert((trip_id, 0, route));
                // Representative = lexicographically smallest trip_id (independent of FxHashMap order).
                if trip_id < e.0 {
                    e.0 = trip_id;
                }
                e.1 += 1;
            }
            let mut durs: Vec<(&str, u32, u32, &str, u32)> = sched
                .iter()
                .map(|(&(dur, dep), &(tid, cnt, route))| (tid, dur, dep, route, cnt))
                .collect();
            // Determinism: FxHashMap iteration order is undefined → sort by representative trip_id.
            durs.sort_unstable_by(|a, b| a.0.cmp(b.0));

            if durs.len() < 5 {
                continue;
            }
            let mut all: Vec<u32> = durs.iter().map(|&(_, d, _, _, _)| d).collect();
            all.sort_unstable();
            let med_all = all[all.len() / 2] as f64;
            if med_all < 120.0 {
                continue;
            }

            // Coarse time bands (instead of hourly): capture intra-day structure without starving
            // buckets of data (the contributor's "coarse buckets degrade more gracefully" note).
            let band = |dep: u32| -> u32 {
                // % 24: banding uses the trip's FIRST departure. If a feed writes a cross-midnight
                // trip's first departure as 24:xx+ (≥86400 sec, valid GTFS), raw hours would put it
                // in the evening (band 4) instead of the night band (band 0); % 24 maps it correctly.
                // NOTE: normalize_service_day does not shift the FIRST stop (it starts at offset 0),
                // so this applies only when the feed itself writes a 24:xx first departure (defensive;
                // no-op for feeds such as Toei that write 00:xx). See issue #26.
                match (dep / 3600) % 24 {
                    0..=5 => 0,   // night / early (24:xx, 25:xx… map here)
                    6..=9 => 1,   // morning peak
                    10..=15 => 2, // daytime
                    16..=19 => 3, // evening peak
                    _ => 4,       // evening / night (20:00–23:59)
                }
            };
            // Dense bands (≥BUCKET_MIN_TRIPS) use their own median; sparse bands fall back to global.
            let mut by_band: FxHashMap<u32, Vec<u32>> = FxHashMap::default();
            for &(_, d, dep, _, _) in &durs {
                by_band.entry(band(dep)).or_default().push(d);
            }
            let mut band_med: FxHashMap<u32, f64> = FxHashMap::default();
            for (b, mut bv) in by_band {
                if bv.len() >= BUCKET_MIN_TRIPS {
                    bv.sort_unstable();
                    band_med.insert(b, bv[bv.len() / 2] as f64);
                }
            }
            let reference = |dep: u32| band_med.get(&band(dep)).copied().unwrap_or(med_all);

            // residual = duration − hour-based expected value; MAD is computed on residuals.
            let mut res: Vec<f64> = durs
                .iter()
                .map(|&(_, d, dep, _, _)| d as f64 - reference(dep))
                .collect();
            res.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
            let med_res = res[res.len() / 2];
            let mut dev: Vec<f64> = res.iter().map(|r| (r - med_res).abs()).collect();
            dev.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
            let mad_r = dev[dev.len() / 2] * 1.4826;

            // Residual-z threshold (config; default 3.5 — issue #24, tightens deseasonalized residuals).
            let sigma = config.duration_outlier_sigma;
            for &(trip_id, dur, dep_secs, route, repeats) in &durs {
                let ref_med = reference(dep_secs);
                let residual = dur as f64 - ref_med;
                // Both conditions together (AND): (1) operationally meaningful deviation (≥ floor),
                // (2) statistical outlier (residual > σ·MAD). If mad_r≈0 (residuals are identical),
                // skip the σ test; only the floor decides.
                let mag_ok = residual.abs() >= RESIDUAL_FLOOR_SECS;
                let stat_ok = mad_r < 1.0 || residual.abs() > sigma * mad_r;
                if !(mag_ok && stat_ok) {
                    continue;
                }

                let label = route_label.get(route).copied().unwrap_or(route);
                let dep_suffix = format!(
                    " {:02}:{:02} kalkışlı",
                    dep_secs / 3600,
                    (dep_secs % 3600) / 60
                );
                let dur_min = dur / 60;
                let ref_min = (ref_med as u32) / 60;
                let yon = if residual >= 0.0 {
                    "beklenenden uzun"
                } else {
                    "beklenenden kısa"
                };
                let sigma_str = if mad_r >= 30.0 {
                    let z = residual / (mad_r / 1.4826);
                    format!("{z:+.1}σ, {yon}")
                } else {
                    format!("{:+}dk, {yon}", (residual / 60.0).round() as i64)
                };
                // If the same schedule spans multiple trip_ids, say so in the message: the user
                // should know how many records one finding affects (the fix applies to all).
                let repeat_suffix = if repeats > 1 {
                    format!(" Aynı tarife {repeats} sefer kaydında tekrarlanıyor.")
                } else {
                    String::new()
                };
                let mut n = k6_notice(
                    ctr,
                    "VAT_003",
                    EntityType::Trip,
                    Some(trip_id.to_string()),
                    Some(trip_id.to_string()),
                    "stop_times.txt",
                    None,
                    Some("trip_id"),
                    Some(format!("{dur_min}min (hour-based expected {ref_min}min)")),
                    None,
                    format!("'{label}' hattının '{trip_id}'{dep_suffix} seferinin süresi {dur_min}dk — bu saatte beklenen ~{ref_min}dk ({sigma_str}).{repeat_suffix}"),
                    "stop_times.txt zaman değerlerini ve sefer güzergahını doğrulayın.",
                );
                if repeats > 1 {
                    let mut d = std::collections::BTreeMap::new();
                    d.insert("duplicate_trips".to_string(), repeats.to_string());
                    n.details = Some(d.into());
                }
                notices.push(n);
            }
        }
    }

    // VAT_004 (route has ≥5 weekday trips, no weekend calendar record) REMOVED
    // (2026-07-23): it was a more incorrect version of OPR_004 (no weekend days in a route's
    // active services). VAT_004 looked at raw calendar.txt flags and DID NOT SEE calendar_dates.txt
    // exceptions → false positive for a route defined only through weekend exceptions. OPR_004
    // uses the derived bitmap (including calendar_dates) and is more accurate. Both are INFO
    // (weight 0, score-neutral), with no MD parity. In practice VAT_004 ⊂ OPR_004 (99.6% corpus overlap).

    // ── VAT_005: isolated stop cluster (Union-Find connected components) ────
    {
        // K2's stop_idx values identify the same numeric node for the same stop_id.
        // Thus, instead of building a persistent bidirectional HashSet graph from 59.7M rows
        // for VAT_005, keep only the component parent and size.
        let stop_count = idx.k2.stop_intern.len();
        let mut parent: Vec<u32> = (0..stop_count as u32).collect();
        let mut component_size = vec![1usize; stop_count];
        let mut active = vec![false; stop_count];

        let find_root = |parent: &mut [u32], mut node: u32| -> u32 {
            let mut root = node;
            while parent[root as usize] != root {
                root = parent[root as usize];
            }
            while parent[node as usize] != node {
                let next = parent[node as usize];
                parent[node as usize] = root;
                node = next;
            }
            root
        };

        for stops in idx.by_trip.values() {
            for w in stops.windows(2) {
                let a = w[0].stop_idx;
                let b = w[1].stop_idx;
                if a == u32::MAX
                    || b == u32::MAX
                    || idx.k2.stop_id_of_idx(a).is_empty()
                    || idx.k2.stop_id_of_idx(b).is_empty()
                {
                    continue;
                }
                let (a_idx, b_idx) = (a as usize, b as usize);
                if a_idx >= stop_count || b_idx >= stop_count {
                    continue;
                }
                active[a_idx] = true;
                active[b_idx] = true;
                let mut root_a = find_root(&mut parent, a);
                let mut root_b = find_root(&mut parent, b);
                if root_a == root_b {
                    continue;
                }
                if component_size[root_a as usize] < component_size[root_b as usize] {
                    std::mem::swap(&mut root_a, &mut root_b);
                }
                parent[root_b as usize] = root_a;
                component_size[root_a as usize] += component_size[root_b as usize];
            }
        }

        let mut components: FxHashMap<u32, Vec<u32>> = FxHashMap::default();
        for (node, is_active) in active.iter().copied().enumerate() {
            if !is_active {
                continue;
            }
            let root = find_root(&mut parent, node as u32);
            components.entry(root).or_default().push(node as u32);
        }
        let comp_nodes = canonicalize_stop_components(
            components
                .into_values()
                .map(|nodes| {
                    nodes
                        .into_iter()
                        .map(|node| idx.k2.stop_id_of_idx(node))
                        .collect()
                })
                .collect(),
        );
        let total: usize = comp_nodes.iter().map(Vec::len).sum();
        // Main network = the component containing at least half of the active stops. Otherwise
        // the feed consists of equivalent disconnected networks and "detached from the main
        // network" is meaningless (2026-10-30 corpus, mdb-905: 386 stops, 34 components, largest 13).
        let has_main_network = comp_nodes.first().is_some_and(|main| main.len() * 2 >= total);
        if total >= 5 && comp_nodes.len() > 1 && has_main_network {
            let main_size = comp_nodes.first().map(Vec::len).unwrap_or(0);
            let small_thresh = (total / 20).max(2);

            // Stop coordinates: in feeds without parent_station, one physical place (e.g. bus
            // terminal platforms) is split into separate stop_ids → a false isolated component.
            // If any stop in an isolated candidate is within this distance of a stop in the main
            // network, treat it as the same physical network and not isolated (false-positive guard).
            let mut stop_coords: FxHashMap<&str, (f64, f64)> = FxHashMap::default();
            for s in &records.stops {
                if let Some(c) = recoverable_stop_coord(s) {
                    stop_coords.insert(s.stop_id.as_str(), c);
                }
            }
            let main_idx = 0;
            let main_coords: Vec<(f64, f64)> = comp_nodes[main_idx]
                .iter()
                .filter_map(|s| stop_coords.get(s).copied())
                .collect();
            const VAT005_MERGE_M: f64 = 200.0;

            // Isolated components: both small and geographically detached from the main network.
            let isolated_comps: Vec<&Vec<&str>> = comp_nodes
                .iter()
                .filter(|c| c.len() < main_size && c.len() <= small_thresh)
                .filter(|c| {
                    !c.iter().any(|s| {
                        stop_coords.get(s).is_some_and(|&(la, lo)| {
                            main_coords.iter().any(|&(mla, mlo)| {
                                haversine_km(la, lo, mla, mlo) * 1000.0 <= VAT005_MERGE_M
                            })
                        })
                    })
                })
                .collect();

            if !isolated_comps.is_empty() {
                // Collect all stops belonging to isolated components (max 200).
                let mut isolated_all: Vec<&str> = Vec::new();
                'outer: for c in &isolated_comps {
                    for &node in c.iter() {
                        isolated_all.push(node);
                        if isolated_all.len() >= 200 {
                            break 'outer;
                        }
                    }
                }

                let isolated_count = isolated_all.len();
                let comp_count = isolated_comps.len();
                let example = isolated_comps[0][0];
                let ex_name = stop_name_map.get(example).copied().unwrap_or(example);

                let mut n005 = k6_notice(
                    ctr,
                    "VAT_005",
                    EntityType::Feed,
                    None,
                    None,
                    "stop_times.txt",
                    None,
                    None,
                    Some(format!("{isolated_count} / {comp_count}")),
                    None,
                    format!("Ağ grafında ana şebekeden (seferlerle bağlı en büyük durak kümesi) kopuk {comp_count} izole durak kümesi var ({isolated_count} durak) — bu duraklar kendi aralarında seferlerle bağlı ama ana şebekeye hiçbir seferle bağlanmıyor. Örnek: '{ex_name}' ('{example}')."),
                    "Ayrı bir servis bölgesi değilse, ortak durakların başka rotalarla aynı stop_id'yi paylaştığını veya bağlantı seferlerinin tanımlı olduğunu kontrol edin (sık neden: aynı yerin platform-özel ayrı stop_id'lerle modellenmesi).",
                );
                let mut d = std::collections::BTreeMap::new();
                d.insert("isolated_stops".to_string(), isolated_all.join(","));
                n005.details = Some(d.into());
                notices.push(n005);
            }
        }
    }

    // ── VAT_006: service-density imbalance (one route ≥ 40%) ────────────────
    {
        let total_trips: u32 = route_trip_count.values().sum();
        // With 1–2 routes, the busiest route is necessarily ≥50%: a 40% threshold fires
        // unconditionally and says nothing about imbalance (a single-route ferry feed reported 76/76, 100%).
        if total_trips >= 50 && route_trip_count.len() >= 3 {
            for (&route, &count) in &route_trip_count {
                let ratio = count as f64 / total_trips as f64;
                if ratio >= 0.40 {
                    let label = route_label.get(route).copied().unwrap_or(route);
                    notices.push(k6_notice(
                        ctr,
                        "VAT_006",
                        EntityType::Route,
                        Some(route.to_string()),
                        Some(route.to_string()),
                        "routes.txt",
                        None,
                        Some("route_id"),
                        Some(format!("{count}/{total_trips} (%{:.0})", ratio * 100.0)),
                        None,
                        format!("'{label}' hattı feed'deki {total_trips} seferin {count}'ini (%{:.0}) oluşturuyor — hizmet yoğunluğu dengesiz.",
                            ratio * 100.0),
                        "Yüksek sefer yoğunluğu operasyonel bir gerçeği yansıtıyorsa bu bilgi notudur. Değilse diğer hatlara sefer ekleyin.",
                    ));
                }
            }
        }
    }

    // ── VAT_007: missing terminus transfer (≥ 3 route termini, no transfer) ──
    {
        let mut terminus_routes: FxHashMap<&str, FxHashSet<&str>> = FxHashMap::default();
        for (&trip_id, stops) in &idx.by_trip {
            let route = match trip_route.get(trip_id) {
                Some(&r) => r,
                None => continue,
            };
            if let Some(first) = stops.first() {
                let sid = idx.stop_id_of(first);
                if !sid.is_empty() {
                    terminus_routes.entry(sid).or_default().insert(route);
                }
            }
            if stops.len() > 1 {
                if let Some(last) = stops.last() {
                    let sid = idx.stop_id_of(last);
                    if !sid.is_empty() {
                        terminus_routes.entry(sid).or_default().insert(route);
                    }
                }
            }
        }

        for (&stop_id, routes) in &terminus_routes {
            if routes.len() < 3 {
                continue;
            }
            if transfer_stops.contains(stop_id) {
                continue;
            }
            if has_parent.contains(stop_id) {
                continue;
            }
            // Same prerequisite as VAT_002: a transfer at the same stop_id is implicit. The
            // neighbor test considers ALL routes serving the stop (not only routes terminating
            // there) — if the neighboring stop serves a route passing through here, there is no new connection.
            let own_all = stop_routes.get(stop_id);
            let empty: FxHashSet<&str> = FxHashSet::default();
            if !has_nearby_unlinked_route(stop_id, own_all.unwrap_or(&empty)) {
                continue;
            }
            let name = stop_name_map.get(stop_id).copied().unwrap_or(stop_id);
            // Determinism: FxHashSet iteration order is undefined. Previously take(5) came
            // BEFORE sorting → which 5 routes entered the message could vary between runs.
            let mut route_ids: Vec<&str> = routes.iter().copied().collect();
            route_ids.sort_unstable();
            let mut labels: Vec<&str> = route_ids
                .iter()
                .take(5)
                .map(|r| route_label.get(r).copied().unwrap_or(r))
                .collect();
            labels.sort_unstable();
            let mut n007 = k6_notice(
                ctr,
                "VAT_007",
                EntityType::Stop,
                Some(stop_id.to_string()),
                None,
                "stops.txt",
                None,
                Some("stop_id"),
                Some(format!("{}", routes.len())),
                None,
                format!("'{name}' (kod: '{stop_id}') {} hattın terminal durağı; transfers.txt'te aktarma tanımlı değil. Hatlar: {}.",
                    routes.len(), labels.join(", ")),
                "transfers.txt'e bu terminus durağı için aktarma kaydı ekleyin.",
            );
            let mut d = std::collections::BTreeMap::new();
            d.insert("routes".to_string(), route_ids.join(","));
            n007.details = Some(d.into());
            notices.push(n007);
        }
    }

    // OPR_024: a route contains too many trips in ONE operating schedule (route_id + service_id).
    // Apply the threshold per service, not per route total: normal trips spread across a week
    // (7 days × daily trips) do not create a false positive; real "data merging" explodes in one schedule.
    {
        let route_short_o24: HashMap<&str, &str> = records
            .routes
            .iter()
            .map(|r| {
                (
                    r.route_id.as_str(),
                    r.route_short_name
                        .as_deref()
                        .filter(|s| !s.is_empty())
                        .unwrap_or(r.route_id.as_str()),
                )
            })
            .collect();
        let mut rs_trip_counts: HashMap<(&str, &str), u32> = HashMap::new();
        for t in &records.trips {
            if !ti_vat.route_id(t).is_empty() {
                *rs_trip_counts
                    .entry((ti_vat.route_id(t), ti_vat.service_id(t)))
                    .or_default() += 1;
            }
        }
        // HashMap iteration → sort (make the dedup representative deterministic).
        let mut rs_trip_counts_sorted: Vec<_> = rs_trip_counts.iter().collect();
        rs_trip_counts_sorted.sort_unstable_by_key(|(k, _)| **k);
        for ((route_id, service_id), count) in rs_trip_counts_sorted {
            if *count > config.max_trips_per_route {
                let mut n = k6_notice(
                    ctr, "OPR_024", EntityType::Route,
                    Some((*route_id).to_string()), Some((*route_id).to_string()),
                    "trips.txt", None, None,
                    Some(count.to_string()), Some(format!("≤{}", config.max_trips_per_route)),
                    format!("'{}' kodlu hat '{service_id}' çalışma takviminde {count} sefer içeriyor — veri birleştirme sorunu olabilir.", route_short_o24.get(*route_id).copied().unwrap_or(*route_id)),
                    "Bu seferlerin doğru route_id ve service_id'ye atandığını kontrol edin.",
                );
                n.service_id = Some((*service_id).to_string());
                n.details.get_or_insert_with(Default::default).insert(
                    "route_label".to_string(),
                    route_short_o24
                        .get(*route_id)
                        .copied()
                        .unwrap_or(*route_id)
                        .to_string(),
                );
                notices.push(n);
            }
        }
    }

    // OPR_025: average trip duration is under 60 seconds (feed-wide).
    {
        let mut trip_durations: Vec<u64> = Vec::new();
        for stops in records.stop_times_index.iter_stops() {
            let mut min_dep: Option<u32> = None;
            let mut max_dep: Option<u32> = None;
            for st in stops {
                if let Some((h, m, s)) = st.departure_time() {
                    let secs = h * 3600 + m * 60 + s;
                    min_dep = Some(min_dep.map_or(secs, |prev: u32| prev.min(secs)));
                    max_dep = Some(max_dep.map_or(secs, |prev: u32| prev.max(secs)));
                }
            }
            if let (Some(f), Some(l)) = (min_dep, max_dep) {
                if l > f {
                    trip_durations.push((l - f) as u64);
                }
            }
        }
        if trip_durations.len() >= 5 {
            let avg = trip_durations.iter().sum::<u64>() / trip_durations.len() as u64;
            if avg < 60 {
                notices.push(k6_notice(
                    ctr, "OPR_025", EntityType::Feed,
                    None, None, "stop_times.txt", None, None,
                    Some(format!("{avg}s")), Some("≥60s".to_string()),
                    format!("Ortalama sefer süresi {avg} saniye — bu kadar kısa süreler genellikle veri hatasını gösterir."),
                    "stop_times.txt'deki departure_time ve arrival_time değerlerini kontrol edin.",
                ));
            }
        }
    }

    // VAT_008: the same shape is used by >30% of the feed's routes.
    {
        let total_routes = records.routes.len();
        if total_routes >= 3 {
            let mut shape_routes: HashMap<&str, HashSet<&str>> = HashMap::new();
            for t in &records.trips {
                if let Some(shape_id) = ti_vat.shape_id(t) {
                    if !shape_id.is_empty() && !ti_vat.route_id(t).is_empty() {
                        shape_routes
                            .entry(shape_id)
                            .or_default()
                            .insert(ti_vat.route_id(t));
                    }
                }
            }
            let threshold = (total_routes as f64 * 0.3).ceil() as usize;
            for (shape_id, route_set) in &shape_routes {
                if route_set.len() > threshold && route_set.len() >= 3 {
                    let pct = route_set.len() as f64 / total_routes as f64 * 100.0;
                    notices.push(k6_notice(
                        ctr, "VAT_008", EntityType::Shape,
                        Some((*shape_id).to_string()), None,
                        "trips.txt", None, None,
                        Some(format!("{:.0}% ({} routes)", pct, route_set.len())),
                        Some(format!("≤{threshold} routes")),
                        format!("'{shape_id}' shape'i {} hatta ({pct:.0}%) kullanılıyor — olası yanlış shape ataması.",
                            route_set.len()),
                        "Her hat ve yön için ayrı shape_id tanımlayın.",
                    ));
                }
            }
        }
    }
}

// ── Testler ───────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::k2::shapes::ShapeInternTable;

    #[test]
    fn max_speed_kmh_maps_extended_route_types() {
        // #15/MD: extended European route_type values must map to the correct category.
        let cfg = ValidatorConfig::default();
        assert_eq!(
            max_speed_kmh(100, &cfg),
            cfg.max_speed_rail_kmh,
            "100 (railway ext) = rail"
        );
        assert_eq!(
            max_speed_kmh(109, &cfg),
            cfg.max_speed_rail_kmh,
            "109 (S-Bahn) = rail"
        );
        assert_eq!(
            max_speed_kmh(106, &cfg),
            cfg.max_speed_rail_kmh,
            "106 (bölgesel rail) = rail"
        );
        assert_eq!(
            max_speed_kmh(400, &cfg),
            cfg.max_speed_metro_kmh,
            "400 (U-Bahn ext) = metro"
        );
        assert_eq!(
            max_speed_kmh(900, &cfg),
            cfg.max_speed_tram_kmh,
            "900 (tram ext) = tram"
        );
        assert_eq!(
            max_speed_kmh(700, &cfg),
            cfg.max_speed_bus_kmh,
            "700 (bus ext) = bus"
        );
        assert_eq!(
            max_speed_kmh(2, &cfg),
            cfg.max_speed_rail_kmh,
            "standart 2 hâlâ rail"
        );
        assert_eq!(
            max_speed_kmh(3, &cfg),
            cfg.max_speed_bus_kmh,
            "standart 3 hâlâ bus"
        );
        // The actual bug: extended rail codes MUST NOT receive the bus threshold.
        assert_ne!(
            max_speed_kmh(100, &cfg),
            cfg.max_speed_bus_kmh,
            "rail ext bus eşiği almamalı"
        );
    }

    use crate::k2::agency::AgencyRecord;
    use crate::k2::routes::RouteRecord;
    use crate::k2::stop_times::StopTimeRecord;
    use crate::k2::stops::StopRecord;
    use crate::k2::trips::TripRecord;
    use crate::k3_entity_graph::EntityMap;

    fn default_config() -> ValidatorConfig {
        ValidatorConfig::default()
    }

    // Regression: contains_as_word must not slice inside a character boundary in multibyte
    // UTF-8 (CJK) and panic. Previously `start = pos + 1` crashed the wasm worker on Japanese
    // feeds (e.g. "市バス１号系統").
    #[test]
    fn contains_as_word_handles_multibyte_utf8() {
        // Must complete without panicking (the actual regression condition).
        assert!(!contains_as_word("市バス１号系統", "bus"));
        assert!(!contains_as_word("六本木ヒルズ前", "ab"));
        assert!(!contains_as_word("東京", ""));
        // ASCII behavior must remain unchanged.
        assert!(contains_as_word("Route 5 Express", "5"));
        assert!(!contains_as_word("Route 51", "5"));
        // An embedded ASCII word inside CJK text must also be found at a boundary.
        assert!(contains_as_word("市バス bus 系統", "bus"));
    }

    #[test]
    fn stop_url_normalization_is_conservative() {
        assert_eq!(
            normalize_web_url("HTTPS://EXAMPLE.ORG:443"),
            normalize_web_url("https://example.org/")
        );
        assert_eq!(
            normalize_web_url("http://example.org:80"),
            normalize_web_url("http://example.org/")
        );
        assert_ne!(
            normalize_web_url("https://example.org/a"),
            normalize_web_url("https://example.org/a/")
        );
        assert_ne!(
            normalize_web_url("https://example.org/?x=1"),
            normalize_web_url("https://example.org/?x=2")
        );
        assert_ne!(
            normalize_web_url("https://example.org/%2F"),
            normalize_web_url("https://example.org//")
        );
        assert!(normalize_web_url("ftp://example.org/").is_none());
    }

    #[test]
    fn stop_url_matches_are_normalized_and_aggregated() {
        let agency = AgencyRecord {
            agency_id: Some("A1".into()),
            agency_name: "Agency".into(),
            agency_url: "https://AGENCY.EXAMPLE:443".into(),
            agency_timezone: "Europe/Istanbul".into(),
            agency_lang: None,
            agency_phone: None,
            agency_fare_url: None,
            agency_email: None,
            agency_cemv_support: None,
            row: Default::default(),
            line: 2,
        };
        let mut line = route("R1", 3);
        line.route_url = Some("https://line.example/route".into());

        let mut first = stop("S1", 41.0, 29.0);
        first.stop_url = Some("https://agency.example/".into());
        let mut second = stop("S2", 41.1, 29.1);
        second.stop_url = Some("HTTPS://AGENCY.EXAMPLE:443".into());
        let mut route_stop = stop("S3", 41.2, 29.2);
        route_stop.stop_url = Some("https://line.example/route".into());
        let mut distinct_query = stop("S4", 41.3, 29.3);
        distinct_query.stop_url = Some("https://line.example/route?stop=4".into());

        let mut records = records_with(
            vec![first, second, route_stop, distinct_query],
            vec![line],
            vec![],
            vec![],
        );
        records.agencies.push(agency);
        let result = analyze(&records, &empty_derived(), &default_config(), 20260514);

        let agency_hits: Vec<_> = result
            .notices
            .iter()
            .filter(|n| n.rule_id == "STP_034")
            .collect();
        let route_hits: Vec<_> = result
            .notices
            .iter()
            .filter(|n| n.rule_id == "STP_035")
            .collect();
        assert_eq!(agency_hits.len(), 1, "normalized agency matches aggregate");
        assert_eq!(route_hits.len(), 1, "query variation is not equated");
        let details = agency_hits[0].details.as_ref().expect("aggregate details");
        assert_eq!(details.get("stop_count").map(String::as_str), Some("2"));
        assert_eq!(
            details.get("representative_stop_ids").map(String::as_str),
            Some("S1,S2")
        );
        assert_eq!(
            details.get("normalized_url").map(String::as_str),
            Some("https://agency.example/")
        );
        assert_eq!(agency_hits[0].entity_id.as_deref(), Some("S1"));
        assert_eq!(route_hits[0].entity_id.as_deref(), Some("S3"));
    }

    fn empty_derived() -> DerivedData {
        DerivedData::default()
    }

    fn stop(stop_id: &str, lat: f64, lon: f64) -> StopRecord {
        StopRecord {
            stop_id: stop_id.into(),
            stop_code: None,
            stop_name: None,
            stop_lat: Some(lat),
            stop_lon: Some(lon),
            location_type: None,
            stop_timezone: None,
            wheelchair_boarding: None,
            stop_access: None,
            level_id: None,
            tts_stop_name: None,
            row: Default::default(),
            line: 2,
            ..Default::default()
        }
    }

    fn route(route_id: &str, route_type: u32) -> RouteRecord {
        RouteRecord {
            route_id: route_id.into(),
            agency_id: None,
            route_short_name: None,
            route_long_name: None,
            route_desc: None,
            route_type: Some(route_type),
            route_url: None,
            route_color: None,
            route_text_color: None,
            route_sort_order: None,
            continuous_pickup: None,
            continuous_drop_off: None,
            network_id: None,
            route_cemv_support: None,
            jp_office_id: None,
            row: Default::default(),
            line: 2,
        }
    }

    use crate::k2::trips::TripInternTable;
    use smol_str::SmolStr;

    // Thread-local TripInternTable: test helpers intern strings here.
    // Each test runs in its own thread, so isolation is automatic.
    thread_local! {
        static TEST_TI: std::cell::RefCell<TripInternTable> =
            std::cell::RefCell::new(TripInternTable::new());
    }

    /// Take the accumulated TripInternTable and replace it with a new empty table.
    fn take_ti() -> TripInternTable {
        TEST_TI.with(|cell| std::mem::replace(&mut *cell.borrow_mut(), TripInternTable::new()))
    }

    /// Basic trip record: route_id + service="SVC" is added to the thread-local intern table.
    fn trip(trip_id: &str, route_id: &str) -> TripRecord {
        TEST_TI.with(|cell| {
            let mut ti = cell.borrow_mut();
            let ri = ti.route_ids.len() as u32;
            ti.route_ids.push(SmolStr::new(route_id));
            let si = ti.service_ids.len() as u32;
            ti.service_ids.push(SmolStr::new("SVC"));
            TripRecord {
                trip_id: trip_id.into(),
                route_idx: ri,
                service_idx: si,
                shape_idx: 0,
                headsign_idx: 0,
                short_name_idx: 0,
                block_idx: 0,
                jp_office_idx: 0,
                jp_pattern_idx: 0,
                direction_id: None,
                wheelchair_accessible: None,
                bikes_allowed: None,
                cars_allowed: None,
                safe_duration_factor: None,
                safe_duration_offset: None,
                line: 2,
            }
        })
    }

    /// Trip + shape_id (added to the thread-local intern table).
    fn trip_sh(trip_id: &str, route_id: &str, shape_id: &str) -> TripRecord {
        let mut t = trip(trip_id, route_id);
        if !shape_id.is_empty() {
            TEST_TI.with(|cell| {
                let mut ti = cell.borrow_mut();
                let shi = ti.shape_ids.len() as u32;
                ti.shape_ids.push(SmolStr::new(shape_id));
                t.shape_idx = shi;
            });
        }
        t
    }

    /// Trip + trip_headsign (added to the thread-local intern table).
    fn trip_hs(trip_id: &str, route_id: &str, headsign: &str) -> TripRecord {
        let mut t = trip(trip_id, route_id);
        TEST_TI.with(|cell| {
            let mut ti = cell.borrow_mut();
            let hi = ti.headsigns.len() as u32;
            ti.headsigns.push(SmolStr::new(headsign));
            t.headsign_idx = hi;
        });
        t
    }

    /// Trip + block_id (added to the thread-local intern table).
    fn trip_blk(trip_id: &str, route_id: &str, block_id: &str) -> TripRecord {
        let mut t = trip(trip_id, route_id);
        if !block_id.is_empty() {
            TEST_TI.with(|cell| {
                let mut ti = cell.borrow_mut();
                let bi = ti.block_ids.len() as u32;
                ti.block_ids.push(SmolStr::new(block_id));
                t.block_idx = bi;
            });
        }
        t
    }

    fn stoptime(
        trip_id: &str,
        seq: u32,
        stop_id: &str,
        arr: (u32, u32, u32),
        dep: (u32, u32, u32),
        line: u64,
    ) -> StopTimeRecord {
        StopTimeRecord {
            trip_id: trip_id.into(),
            stop_id: stop_id.into(),
            stop_sequence: Some(seq),
            arrival_time: Some(arr),
            departure_time: Some(dep),
            line,
            ..Default::default()
        }
    }

    /// records_with: automatically takes the trip intern table from thread-local storage (take_ti).
    fn records_with(
        stops: Vec<StopRecord>,
        routes: Vec<RouteRecord>,
        trips: Vec<TripRecord>,
        stoptimes: Vec<StopTimeRecord>,
    ) -> crate::k2::EntityRecords {
        use crate::k2::stop_times::StopTimesIndex;
        let ti = take_ti();
        crate::k2::EntityRecords {
            stops,
            routes,
            trips,
            trip_interns: ti,
            stop_times_index: StopTimesIndex::from_records(&stoptimes),
            stop_times: stoptimes,
            ..Default::default()
        }
    }

    // ── STM_014 ───────────────────────────────────────────────────────────────

    #[test]
    fn excessive_speed_produces_stm_014() {
        // Istanbul (~41.0, 29.0) → Ankara (~39.9, 32.9) ≈ 350 km, in 2 hours → ~175 km/h.
        // At 30 min it would be ~703 km/h → STM_012 would fire; 2 hours exceeds STM_014's threshold (120 km/h).
        let records = records_with(
            vec![stop("A", 41.0, 29.0), stop("B", 39.9, 32.9)],
            vec![route("R1", 3)], // bus
            vec![trip("T1", "R1")],
            vec![
                stoptime("T1", 1, "A", (8, 0, 0), (8, 0, 0), 2),
                stoptime("T1", 2, "B", (10, 0, 0), (10, 0, 0), 3), // 2 hours, ~350 km → ~175 km/h
            ],
        );
        let result = analyze(&records, &empty_derived(), &default_config(), 20260514);
        assert!(
            result.notices.iter().any(|n| n.rule_id == "STM_014"),
            "STM_014 olmalı"
        );
    }

    #[test]
    fn stm_014_recovers_whitespace_stop_coordinates_for_analytics() {
        let mut first = stop("A", 41.0, 29.0);
        first.stop_lat = None;
        first.stop_lon = None;
        first.row.insert("stop_lat".into(), " 41.0".into());
        first.row.insert("stop_lon".into(), " 29.0".into());
        let mut second = stop("B", 39.9, 32.9);
        second.stop_lat = None;
        second.stop_lon = None;
        second.row.insert("stop_lat".into(), " 39.9".into());
        second.row.insert("stop_lon".into(), " 32.9".into());

        let records = records_with(
            vec![first, second],
            vec![route("R1", 3)],
            vec![trip("T1", "R1")],
            vec![
                stoptime("T1", 1, "A", (8, 0, 0), (8, 0, 0), 2),
                stoptime("T1", 2, "B", (10, 0, 0), (10, 0, 0), 3),
            ],
        );
        let result = analyze(&records, &empty_derived(), &default_config(), 20260514);
        assert!(
            result.notices.iter().any(|n| n.rule_id == "STM_014"),
            "recoverable whitespace coordinates must reach STM_014: {:?}",
            result
                .notices
                .iter()
                .map(|n| &n.rule_id)
                .collect::<Vec<_>>()
        );
    }

    // Aggregation (Reliability Sprint Checkpoint 6): the same physical segment used to emit
    // a separate notice for every trip passing through it (604× for one segment in TriMet).
    #[test]
    fn stm_014_aggregates_same_segment_across_trips() {
        // Same route/direction, SAME A→B segment, 3 trips → 1 notice, not 3.
        let records = records_with(
            vec![
                stop("P", 41.0, 29.0),
                stop("A", 41.0, 29.0),
                stop("B", 39.9, 32.9),
                stop("C", 39.9, 32.9),
            ],
            vec![route("R1", 3)],
            vec![trip("T1", "R1"), trip("T2", "R1"), trip("T3", "R1")],
            vec![
                stoptime("T1", 0, "P", (7, 45, 0), (7, 45, 0), 1),
                stoptime("T1", 1, "A", (8, 0, 0), (8, 0, 0), 2),
                stoptime("T1", 2, "B", (10, 0, 0), (10, 0, 0), 3), // ~175 km/h
                stoptime("T1", 3, "C", (10, 15, 0), (10, 15, 0), 4),
                stoptime("T2", 1, "A", (9, 0, 0), (9, 0, 0), 4),
                stoptime("T2", 2, "B", (11, 30, 0), (11, 30, 0), 5), // ~140 km/h (slower)
                stoptime("T3", 1, "A", (12, 0, 0), (12, 0, 0), 6),
                stoptime("T3", 2, "B", (14, 0, 0), (14, 0, 0), 7), // ~175 km/h
            ],
        );
        let result = analyze(&records, &empty_derived(), &default_config(), 20260514);
        let hits: Vec<_> = result
            .notices
            .iter()
            .filter(|n| n.rule_id == "STM_014")
            .collect();
        assert_eq!(
            hits.len(),
            1,
            "tek segment → tek notice, alınan: {}",
            hits.len()
        );

        let d = hits[0].details.as_ref().expect("details olmalı");
        assert_eq!(
            d.get("trip_count").map(String::as_str),
            Some("3"),
            "3 sefer sayılmalı"
        );
        assert_eq!(d.get("stop_a").map(String::as_str), Some("A"));
        assert_eq!(d.get("stop_b").map(String::as_str), Some("B"));
        assert_eq!(d.get("prev_stop_id").map(String::as_str), Some("P"));
        assert_eq!(d.get("prev_departure").map(String::as_str), Some("07:45:00"));
        assert_eq!(d.get("stop_a_departure").map(String::as_str), Some("08:00:00"));
        assert_eq!(d.get("stop_b_arrival").map(String::as_str), Some("10:00:00"));
        assert_eq!(d.get("next_stop_id").map(String::as_str), Some("C"));
        assert_eq!(d.get("next_arrival").map(String::as_str), Some("10:15:00"));
        assert_eq!(d.get("direction_from_stop_id").map(String::as_str), Some("P"));
        assert_eq!(d.get("direction_to_stop_id").map(String::as_str), Some("C"));
        assert_eq!(d.get("sample_service_id").map(String::as_str), Some("SVC"));
        assert_eq!(d.get("sample_departure").map(String::as_str), Some("07:45:00"));
        assert!(d.contains_key("distance_min_meters"));
        assert!(d.contains_key("distance_max_meters"));
        assert_eq!(d.get("threshold_kmh").map(String::as_str), Some("120"));
        // Example trip list is deterministic and sorted.
        assert_eq!(d.get("trips").map(String::as_str), Some("T1,T2,T3"));
        // Speed range must include the slowest/fastest trip (must not collapse to one value).
        let (lo, hi) = (
            d["speed_min"].parse::<f64>().unwrap(),
            d["speed_max"].parse::<f64>().unwrap(),
        );
        assert!(lo < hi, "hız aralığı olmalı, alınan {lo}-{hi}");
        // scope_key must remain route_id for route-level filtering.
        assert_eq!(hits[0].scope_key.as_deref(), Some("R1"));
    }

    #[test]
    fn stm_014_sample_context_does_not_depend_on_stop_sequence() {
        // The sample trip (T1, smallest id) passes A→B at sequence 1→2; every other trip at 5→6.
        // The segment's seq_a/seq_b come from whichever trip FxHashMap yields first, which differs
        // between 32- and 64-bit builds. The sample context must not depend on that order.
        let records = records_with(
            vec![
                stop("P", 41.0, 29.0),
                stop("A", 41.0, 29.0),
                stop("B", 39.9, 32.9),
                stop("C", 39.9, 32.9),
            ],
            vec![route("R1", 3)],
            vec![trip("T1", "R1"), trip("T2", "R1"), trip("T3", "R1"), trip("T4", "R1"), trip("T5", "R1"), trip("T6", "R1"), trip("T7", "R1"), trip("T8", "R1"), trip("T9", "R1")],
            vec![
                stoptime("T1", 0, "P", (7, 45, 0), (7, 45, 0), 1),
                stoptime("T1", 1, "A", (8, 0, 0), (8, 0, 0), 2),
                stoptime("T1", 2, "B", (10, 0, 0), (10, 0, 0), 3),
                stoptime("T1", 3, "C", (10, 15, 0), (10, 15, 0), 4),
                stoptime("T2", 5, "A", (8, 0, 0), (8, 0, 0), 20),
                stoptime("T2", 6, "B", (10, 0, 0), (10, 0, 0), 21),
                stoptime("T3", 5, "A", (9, 0, 0), (9, 0, 0), 30),
                stoptime("T3", 6, "B", (11, 0, 0), (11, 0, 0), 31),
                stoptime("T4", 5, "A", (10, 0, 0), (10, 0, 0), 40),
                stoptime("T4", 6, "B", (12, 0, 0), (12, 0, 0), 41),
                stoptime("T5", 5, "A", (11, 0, 0), (11, 0, 0), 50),
                stoptime("T5", 6, "B", (13, 0, 0), (13, 0, 0), 51),
                stoptime("T6", 5, "A", (12, 0, 0), (12, 0, 0), 60),
                stoptime("T6", 6, "B", (14, 0, 0), (14, 0, 0), 61),
                stoptime("T7", 5, "A", (13, 0, 0), (13, 0, 0), 70),
                stoptime("T7", 6, "B", (15, 0, 0), (15, 0, 0), 71),
                stoptime("T8", 5, "A", (14, 0, 0), (14, 0, 0), 80),
                stoptime("T8", 6, "B", (16, 0, 0), (16, 0, 0), 81),
                stoptime("T9", 5, "A", (15, 0, 0), (15, 0, 0), 90),
                stoptime("T9", 6, "B", (17, 0, 0), (17, 0, 0), 91),
            ],
        );
        let result = analyze(&records, &empty_derived(), &default_config(), 20260514);
        let notice = result
            .notices
            .iter()
            .find(|n| n.rule_id == "STM_014")
            .expect("STM_014 olmalı");
        let d = notice.details.as_ref().expect("details olmalı");
        assert_eq!(d.get("prev_stop_id").map(String::as_str), Some("P"));
        assert_eq!(d.get("stop_a_departure").map(String::as_str), Some("08:00:00"));
        assert_eq!(d.get("stop_b_arrival").map(String::as_str), Some("10:00:00"));
        assert_eq!(d.get("next_stop_id").map(String::as_str), Some("C"));
    }

    #[test]
    fn stm_014_keeps_distinct_segments_separate() {
        // TWO separate bad segments on the same route (A→B and B→C) → dedup=Entity must
        // not swallow them; entity_id is composite, so 2 notices should be emitted.
        let records = records_with(
            vec![
                stop("A", 41.0, 29.0),
                stop("B", 39.9, 32.9),
                stop("C", 41.0, 29.0),
            ],
            vec![route("R1", 3)],
            vec![trip("T1", "R1")],
            vec![
                stoptime("T1", 1, "A", (8, 0, 0), (8, 0, 0), 2),
                stoptime("T1", 2, "B", (10, 0, 0), (10, 0, 0), 3), // A→B ~175 km/h
                stoptime("T1", 3, "C", (12, 0, 0), (12, 0, 0), 4), // B→C ~175 km/h
            ],
        );
        let result = analyze(&records, &empty_derived(), &default_config(), 20260514);
        let hits: Vec<_> = result
            .notices
            .iter()
            .filter(|n| n.rule_id == "STM_014")
            .collect();
        assert_eq!(
            hits.len(),
            2,
            "iki farklı segment ayrı kalmalı, alınan: {}",
            hits.len()
        );
        let ids: Vec<_> = hits.iter().filter_map(|n| n.entity_id.as_deref()).collect();
        assert_ne!(
            ids[0], ids[1],
            "entity_id'ler benzersiz olmalı (dedup yutmasın): {ids:?}"
        );
    }

    #[test]
    fn normal_speed_no_stm_014() {
        // Two nearby stops, 10 min → low speed.
        let records = records_with(
            vec![stop("A", 41.0, 29.0), stop("B", 41.05, 29.05)],
            vec![route("R1", 3)],
            vec![trip("T1", "R1")],
            vec![
                stoptime("T1", 1, "A", (8, 0, 0), (8, 0, 0), 2),
                stoptime("T1", 2, "B", (8, 10, 0), (8, 10, 0), 3),
            ],
        );
        let result = analyze(&records, &empty_derived(), &default_config(), 20260514);
        assert!(!result.notices.iter().any(|n| n.rule_id == "STM_014"));
    }

    #[test]
    fn opt_in_stop_name_profile_emits_stp_040_and_041() {
        let mut parent = stop("P", 41.0, 29.0);
        parent.stop_name = Some("Kadikoy".into());
        parent.location_type = Some(1);
        let mut child = stop("C", 41.0, 29.0);
        child.stop_name = Some("Platform Stop".into());
        child.row.insert("parent_station".into(), "P".into());
        let records = records_with(vec![parent, child], vec![], vec![], vec![]);
        let mut cfg = default_config();
        cfg.stop_name_best_practices = true;
        let result = analyze(&records, &empty_derived(), &cfg, 20260514);
        assert!(result.notices.iter().any(|n| n.rule_id == "STP_040"));
        assert!(result.notices.iter().any(|n| n.rule_id == "STP_041"));
    }

    #[test]
    fn source_url_without_zip_filename_emits_arc_028() {
        let records = records_with(vec![], vec![], vec![], vec![]);
        let mut cfg = default_config();
        cfg.source_url = Some("https://example.org/gtfs".into());
        let result = analyze(&records, &empty_derived(), &cfg, 20260514);
        assert!(result.notices.iter().any(|n| n.rule_id == "ARC_028"));
        cfg.source_url = Some("https://example.org/feed.zip?rev=2".into());
        let result = analyze(&records, &empty_derived(), &cfg, 20260514);
        assert!(!result.notices.iter().any(|n| n.rule_id == "ARC_028"));
    }
    #[test]
    fn three_consecutive_equal_times_produce_stm_053() {
        let records = records_with(
            vec![
                stop("A", 41.0, 29.0),
                stop("B", 41.001, 29.0),
                stop("C", 41.002, 29.0),
            ],
            vec![route("R1", 3)],
            vec![trip("T1", "R1")],
            vec![
                stoptime("T1", 1, "A", (8, 0, 0), (8, 0, 0), 2),
                stoptime("T1", 2, "B", (8, 0, 0), (8, 0, 0), 3),
                stoptime("T1", 3, "C", (8, 0, 0), (8, 0, 0), 4),
            ],
        );
        let result = analyze(&records, &empty_derived(), &default_config(), 20260514);
        let found: Vec<_> = result
            .notices
            .iter()
            .filter(|n| n.rule_id == "STM_053")
            .collect();
        assert_eq!(found.len(), 1);
        assert_eq!(found[0].entity_type, EntityType::Route);
        assert_eq!(found[0].entity_id.as_deref(), Some("R1"));
    }

    #[test]
    fn stm_053_representative_is_lowest_line_not_hash_order() {
        // `by_trip` is an FxHashMap; if the representative were the "first seen" trip, 64-bit native
        // and 32-bit WASM would show different rows. Choose the trip with the smallest line.
        let trips: Vec<String> = (0..24).map(|i| format!("T{i:02}")).collect();
        let mut stop_times = Vec::new();
        // Lines increase in REVERSE trip-name order: the smallest line is on T23.
        for (i, trip_id) in trips.iter().rev().enumerate() {
            let base = 2 + (i as u64) * 3;
            stop_times.push(stoptime(trip_id, 1, "A", (8, 0, 0), (8, 0, 0), base));
            stop_times.push(stoptime(trip_id, 2, "B", (8, 0, 0), (8, 0, 0), base + 1));
            stop_times.push(stoptime(trip_id, 3, "C", (8, 0, 0), (8, 0, 0), base + 2));
        }
        let records = records_with(
            vec![
                stop("A", 41.0, 29.0),
                stop("B", 41.001, 29.0),
                stop("C", 41.002, 29.0),
            ],
            vec![route("R1", 3)],
            trips.iter().map(|t| trip(t, "R1")).collect(),
            stop_times,
        );
        let result = analyze(&records, &empty_derived(), &default_config(), 20260514);
        let found: Vec<_> = result
            .notices
            .iter()
            .filter(|n| n.rule_id == "STM_053")
            .collect();
        assert_eq!(found.len(), 1);
        assert_eq!(found[0].file.as_deref(), Some("stop_times.txt"));
        assert_eq!(found[0].field.as_deref(), Some("arrival_time|departure_time"));
        assert_eq!(found[0].line, Some(2));
        assert!(
            found[0].message.contains("'T23'"),
            "örnek sefer en küçük satırlı sefer olmalı: {}",
            found[0].message
        );
    }

    // ── OPR_008 ───────────────────────────────────────────────────────────────

    #[test]
    fn excessive_speed_produces_opr_008() {
        // OPR_008 requires trip_bad_seg_count > 1: 3 stops, 2 anomalous segments.
        let records = records_with(
            vec![
                stop("A", 41.0, 29.0),
                stop("B", 39.9, 32.9),
                stop("C", 37.0, 36.0),
            ],
            vec![route("R1", 3)],
            vec![trip("T1", "R1")],
            vec![
                stoptime("T1", 1, "A", (8, 0, 0), (8, 0, 0), 2),
                stoptime("T1", 2, "B", (8, 10, 0), (8, 10, 0), 3), // ~350 km / 10 min → excessive speed
                stoptime("T1", 3, "C", (8, 20, 0), (8, 20, 0), 4), // ~350 km / 10 min → excessive speed
            ],
        );
        let result = analyze(&records, &empty_derived(), &default_config(), 20260514);
        let opr = result
            .notices
            .iter()
            .find(|n| n.rule_id == "OPR_008")
            .expect("OPR_008 olmalı");
        // Both segments enter the STM_012 (>700 km/h) branch; OPR_008 must still carry
        // the highest speed, line, and segment pairs (not 0.0 km/h).
        assert_ne!(opr.observed_value.as_deref(), Some("0.0 km/h"));
        assert!(opr.line.is_some(), "OPR_008 satırı olmalı");
        let d = opr.details.as_ref().expect("details olmalı");
        assert_eq!(d.get("bad_seg_0_a").map(String::as_str), Some("A"));
        assert_eq!(d.get("bad_seg_1_b").map(String::as_str), Some("C"));
    }

    // ── STM_028 ───────────────────────────────────────────────────────────────

    #[test]
    fn long_trip_produces_stm_028() {
        // 30-hour trip > 24-hour threshold.
        let records = records_with(
            vec![stop("A", 41.0, 29.0), stop("B", 41.01, 29.01)],
            vec![route("R1", 3)],
            vec![trip("T1", "R1")],
            vec![
                stoptime("T1", 1, "A", (8, 0, 0), (8, 0, 0), 2),
                stoptime("T1", 2, "B", (38, 0, 0), (38, 0, 0), 3), // GTFS 38:00:00
            ],
        );
        let result = analyze(&records, &empty_derived(), &default_config(), 20260514);
        assert!(result.notices.iter().any(|n| n.rule_id == "STM_028"));
    }

    #[test]
    fn long_rail_trip_under_rail_threshold_no_stm_028() {
        // 30-hour TRAIN trip: exceeds the urban threshold (24h) but stays below the rail threshold (48h).
        // Real cases: TCDD Ankara-Kars 26:26, Ankara-Tatvan 26:37 — legitimate schedules.
        let records = records_with(
            vec![stop("A", 41.0, 29.0), stop("B", 41.01, 29.01)],
            vec![route("R1", 2)], // 2 = Rail
            vec![trip("T1", "R1")],
            vec![
                stoptime("T1", 1, "A", (8, 0, 0), (8, 0, 0), 2),
                stoptime("T1", 2, "B", (38, 0, 0), (38, 0, 0), 3),
            ],
        );
        let result = analyze(&records, &empty_derived(), &default_config(), 20260514);
        assert!(!result.notices.iter().any(|n| n.rule_id == "STM_028"));
    }

    #[test]
    fn very_long_rail_trip_produces_stm_028_as_info() {
        // 50 hours: also exceeds the rail threshold → notice PRESENT, but severity is Info (not urban HIGH).
        let records = records_with(
            vec![stop("A", 41.0, 29.0), stop("B", 41.01, 29.01)],
            vec![route("R1", 2)],
            vec![trip("T1", "R1")],
            vec![
                stoptime("T1", 1, "A", (8, 0, 0), (8, 0, 0), 2),
                stoptime("T1", 2, "B", (58, 0, 0), (58, 0, 0), 3),
            ],
        );
        let result = analyze(&records, &empty_derived(), &default_config(), 20260514);
        let n = result
            .notices
            .iter()
            .find(|n| n.rule_id == "STM_028")
            .expect("50 saatlik tren seferi raylı eşiği de aşar");
        assert_eq!(n.severity, gtfs_core::Severity::Bilgi);
    }

    #[test]
    fn normal_trip_no_stm_028() {
        let records = records_with(
            vec![stop("A", 41.0, 29.0), stop("B", 41.01, 29.01)],
            vec![route("R1", 3)],
            vec![trip("T1", "R1")],
            vec![
                stoptime("T1", 1, "A", (8, 0, 0), (8, 0, 0), 2),
                stoptime("T1", 2, "B", (8, 30, 0), (8, 30, 0), 3),
            ],
        );
        let result = analyze(&records, &empty_derived(), &default_config(), 20260514);
        assert!(!result.notices.iter().any(|n| n.rule_id == "STM_028"));
    }

    // ── STM_029 ───────────────────────────────────────────────────────────────

    #[test]
    fn short_trip_produces_stm_029() {
        // 30-second trip < 60-second threshold.
        let records = records_with(
            vec![stop("A", 41.0, 29.0), stop("B", 41.01, 29.01)],
            vec![route("R1", 3)],
            vec![trip("T1", "R1")],
            vec![
                stoptime("T1", 1, "A", (8, 0, 0), (8, 0, 0), 2),
                stoptime("T1", 2, "B", (8, 0, 30), (8, 0, 30), 3),
            ],
        );
        let result = analyze(&records, &empty_derived(), &default_config(), 20260514);
        assert!(result.notices.iter().any(|n| n.rule_id == "STM_029"));
    }

    // ── STM_012: exact-minute zero duration — threshold depends on route type ─────────
    // If both times are exact minutes, the real travel time is unknown (0–59 sec). To call it
    // "impossible", the distance must exceed the maximum speed even if a full minute elapsed → max_speed/60.
    // Corpus evidence (mdb-2155): a fixed 1.0 km threshold made 41 of 42 findings false positives.

    #[test]
    fn whole_minute_zero_gap_below_one_minute_reach_is_not_stm_012() {
        // ~1.1 km / bus: 66 km/h in a full minute — ordinary, threshold 120 → MUST STAY SILENT.
        let records = records_with(
            vec![stop("A", 41.0, 29.0), stop("B", 41.0099, 29.0)],
            vec![route("R1", 3)], // bus → 120 km/h → impossibility limit 2.0 km
            vec![trip("T1", "R1")],
            vec![
                stoptime("T1", 1, "A", (8, 0, 0), (8, 0, 0), 2),
                stoptime("T1", 2, "B", (8, 0, 0), (8, 0, 0), 3),
            ],
        );
        let result = analyze(&records, &empty_derived(), &default_config(), 20260514);
        let hits: Vec<_> = result
            .notices
            .iter()
            .filter(|n| n.rule_id == "STM_012")
            .collect();
        assert!(
            hits.is_empty(),
            "dakika yuvarlaması ile açıklanabilen mesafe FP olmamalı: {:?}",
            hits.iter().map(|n| &n.observed_value).collect::<Vec<_>>()
        );
    }

    #[test]
    fn whole_minute_zero_gap_beyond_one_minute_reach_is_stm_012() {
        // ~29 km / bus: even a full minute requires ~1740 km/h → REAL error, must fire.
        // (This was the only case of this class also found by MD in mdb-2155.)
        let records = records_with(
            vec![
                stop("P", 41.0, 29.0),
                stop("A", 41.001, 29.0),
                stop("B", 41.26, 29.0),
                stop("N", 41.261, 29.0),
            ],
            vec![route("R1", 3)],
            vec![trip("T1", "R1")],
            vec![
                stoptime("T1", 1, "P", (7, 50, 0), (7, 50, 0), 2),
                stoptime("T1", 2, "A", (8, 0, 0), (8, 0, 0), 3),
                stoptime("T1", 3, "B", (8, 0, 0), (8, 0, 0), 4),
                stoptime("T1", 4, "N", (8, 10, 0), (8, 10, 0), 5),
            ],
        );
        let result = analyze(&records, &empty_derived(), &default_config(), 20260514);
        let notice = result
            .notices
            .iter()
            .find(|n| n.rule_id == "STM_012")
            .expect("bir dakikada alınamayacak mesafe STM_012 üretmeli");
        let details = notice
            .details
            .as_ref()
            .expect("STM_012 yapılandırılmış kanıt taşımalı");
        assert_eq!(
            details.get("stop_sequence_a").map(String::as_str),
            Some("2")
        );
        assert_eq!(
            details.get("stop_sequence_b").map(String::as_str),
            Some("3")
        );
        assert_eq!(
            details.get("scheduled_gap_seconds").map(String::as_str),
            Some("0")
        );
        assert_eq!(
            details.get("distance_method").map(String::as_str),
            Some("haversine")
        );
        assert!(details.contains_key("distance_km"));
        assert!(details.contains_key("distance_meters"));
        assert!(details.contains_key("speed_lower_bound_kmh"));
        assert_eq!(details.get("prev_stop_id").map(String::as_str), Some("P"));
        assert_eq!(
            details.get("prev_arrival").map(String::as_str),
            Some("07:50:00")
        );
        assert_eq!(
            details.get("stop_a_departure").map(String::as_str),
            Some("08:00:00")
        );
        assert_eq!(
            details.get("stop_b_arrival").map(String::as_str),
            Some("08:00:00")
        );
        assert_eq!(details.get("next_stop_id").map(String::as_str), Some("N"));
        assert_eq!(
            details.get("next_departure").map(String::as_str),
            Some("08:10:00")
        );
    }

    #[test]
    fn whole_minute_zero_gap_stm_012_counts_toward_opr_008() {
        // Two consecutive exact-minute zero-duration segments, both ~29 km → two STM_012 findings.
        // Like a route with an untimed intermediate stop, this route must also count toward OPR_008.
        let records = records_with(
            vec![
                stop("A", 41.0, 29.0),
                stop("B", 41.26, 29.0),
                stop("C", 41.52, 29.0),
            ],
            vec![route("R1", 3)],
            vec![trip("T1", "R1")],
            vec![
                stoptime("T1", 1, "A", (8, 0, 0), (8, 0, 0), 2),
                stoptime("T1", 2, "B", (8, 0, 0), (8, 0, 0), 3),
                stoptime("T1", 3, "C", (8, 0, 0), (8, 0, 0), 4),
            ],
        );
        let result = analyze(&records, &empty_derived(), &default_config(), 20260514);
        let opr = result
            .notices
            .iter()
            .find(|n| n.rule_id == "OPR_008")
            .expect("iki sıfır süreli STM_012 segmenti OPR_008 üretmeli");
        assert_ne!(opr.observed_value.as_deref(), Some("0.0 km/h"));
        assert!(opr.line.is_some(), "OPR_008 satırı olmalı");
        let d = opr.details.as_ref().expect("details olmalı");
        assert_eq!(d.get("bad_seg_0_a").map(String::as_str), Some("A"));
        assert_eq!(d.get("bad_seg_1_b").map(String::as_str), Some("C"));
    }

    #[test]
    fn projected_speed_above_700_but_haversine_below_is_not_stm_012() {
        let mut shape_ti = ShapeInternTable::new();
        use crate::k2::shapes::ShapePointRecord;
        // Stops are ~0.5 km apart as the crow flies, 6 sec → hav ~300 km/h (≤700). Shape route ~1.5 km
        // (3× detour, below the 4× ceiling) → projected speed ~900 km/h. Since the "impossible"
        // claim is not proven by the lower bound (haversine), STM_012 MUST NOT fire;
        // the case should fall to STM_014 as excessive speed.
        let mut records = records_with(
            vec![stop("A", 41.0, 29.0), stop("B", 41.0045, 29.0)],
            vec![route("R1", 3)],
            vec![trip_sh("T1", "R1", "S1")],
            vec![
                stoptime("T1", 1, "A", (8, 0, 0), (8, 0, 0), 2),
                stoptime("T1", 2, "B", (8, 0, 6), (8, 0, 6), 3),
            ],
        );
        records.shapes = vec![
            ShapePointRecord::new(
                shape_ti.intern("S1"),
                Some(41.0),
                Some(29.0),
                Some(1),
                None,
                2,
            ),
            ShapePointRecord::new(
                shape_ti.intern("S1"),
                Some(41.0),
                Some(29.006),
                Some(2),
                None,
                3,
            ),
            ShapePointRecord::new(
                shape_ti.intern("S1"),
                Some(41.0045),
                Some(29.006),
                Some(3),
                None,
                4,
            ),
            ShapePointRecord::new(
                shape_ti.intern("S1"),
                Some(41.0045),
                Some(29.0),
                Some(4),
                None,
                5,
            ),
        ];
        records.shape_interns = shape_ti.clone();
        let result = analyze(&records, &empty_derived(), &default_config(), 20260514);
        assert!(
            !result.notices.iter().any(|n| n.rule_id == "STM_012"),
            "haversine ≤700 iken STM_012 çıkmamalı (imkânsızlık alt-sınırla kanıtlanır)"
        );
        assert!(
            result.notices.iter().any(|n| n.rule_id == "STM_014"),
            "aşırı hız STM_014 olarak raporlanmalı"
        );
    }

    #[test]
    fn projected_shape_distance_cannot_reduce_haversine_speed_floor() {
        let mut shape_ti = ShapeInternTable::new();
        use crate::k2::shapes::ShapePointRecord;
        // A and B are about 8.4 km apart. The shape projects the two stops onto nearby parallel
        // vertical sections; the arc difference is about 2.8 km, and the old code used that value
        // to leave the 3-minute segment below the bus threshold.
        let mut records = records_with(
            vec![stop("A", 41.0, 29.0), stop("B", 41.0, 29.1)],
            vec![route("R1", 3)],
            vec![trip_sh("T1", "R1", "S1")],
            vec![
                stoptime("T1", 1, "A", (8, 0, 0), (8, 0, 0), 2),
                stoptime("T1", 2, "B", (8, 3, 0), (8, 3, 0), 3),
            ],
        );
        records.shapes = vec![
            ShapePointRecord::new(
                shape_ti.intern("S1"),
                Some(40.995),
                Some(29.04),
                Some(1),
                None,
                2,
            ),
            ShapePointRecord::new(
                shape_ti.intern("S1"),
                Some(41.005),
                Some(29.04),
                Some(2),
                None,
                3,
            ),
            ShapePointRecord::new(
                shape_ti.intern("S1"),
                Some(41.005),
                Some(29.06),
                Some(3),
                None,
                4,
            ),
            ShapePointRecord::new(
                shape_ti.intern("S1"),
                Some(40.995),
                Some(29.06),
                Some(4),
                None,
                5,
            ),
        ];
        records.shape_interns = shape_ti;
        let result = analyze(&records, &empty_derived(), &default_config(), 20260514);
        assert!(
            result.notices.iter().any(|n| n.rule_id == "STM_014"),
            "Haversine alt sınırı gerçek otobüs hızını koruyarak STM_014 üretmeli: {:?}",
            result
                .notices
                .iter()
                .map(|n| (&n.rule_id, n.observed_value.as_deref()))
                .collect::<Vec<_>>()
        );
        assert!(!result.notices.iter().any(|n| n.rule_id == "STM_012"));
    }

    #[test]
    fn legitimate_rail_speed_below_rail_threshold_has_no_stm_014() {
        let records = records_with(
            vec![stop("A", 41.0, 29.0), stop("B", 39.9, 32.9)],
            vec![route("R1", 2)],
            vec![trip("T1", "R1")],
            vec![
                stoptime("T1", 1, "A", (8, 0, 0), (8, 0, 0), 2),
                stoptime("T1", 2, "B", (10, 0, 0), (10, 0, 0), 3),
            ],
        );
        let result = analyze(&records, &empty_derived(), &default_config(), 20260514);
        assert!(!result.notices.iter().any(|n| n.rule_id == "STM_014"));
        assert!(!result.notices.iter().any(|n| n.rule_id == "STM_012"));
    }

    #[test]
    fn shp_020_saturation_aggregates_to_single_feed_notice() {
        let mut shape_ti = ShapeInternTable::new();
        use crate::k2::shapes::ShapePointRecord;
        // Repeated point in 51 shapes (above the threshold of 50) → ONE feed-summary SHP_020
        // instead of 51 per-shape notices (SHP_010 precedent; VBB 66% saturation lesson).
        let mut records = records_with(
            vec![stop("A", 41.0, 29.0), stop("B", 41.1, 29.1)],
            vec![route("R1", 3)],
            vec![trip_sh("T1", "R1", "S0")],
            vec![
                stoptime("T1", 1, "A", (8, 0, 0), (8, 0, 0), 2),
                stoptime("T1", 2, "B", (8, 30, 0), (8, 30, 0), 3),
            ],
        );
        let mut shapes = Vec::new();
        for k in 0..51u32 {
            let sid = format!("S{k}");
            // Apex pattern: points 1 and 3 are identical. Each shape repeats at its OWN coordinate —
            // saturation is now measured by distinct COORDINATES (shapes sharing a point are grouped
            // into one notice), so 51 separate locations are required.
            let lat = 41.0 + k as f64 * 0.01;
            shapes.push(ShapePointRecord::new(
                shape_ti.intern(&sid),
                Some(lat),
                Some(29.0),
                Some(1),
                None,
                2,
            ));
            shapes.push(ShapePointRecord::new(
                shape_ti.intern(&sid),
                Some(lat + 0.001),
                Some(29.0),
                Some(2),
                None,
                3,
            ));
            shapes.push(ShapePointRecord::new(
                shape_ti.intern(&sid),
                Some(lat),
                Some(29.0),
                Some(3),
                None,
                4,
            ));
        }
        records.shapes = shapes;
        records.shape_interns = shape_ti.clone();
        let result = analyze(&records, &empty_derived(), &default_config(), 20260514);
        let hits: Vec<_> = result
            .notices
            .iter()
            .filter(|n| n.rule_id == "SHP_020")
            .collect();
        assert_eq!(hits.len(), 1, "eşik üstünde tek feed-özet SHP_020 beklenir");
        assert!(
            hits[0].entity_id.is_none(),
            "özet notice feed-seviyesi olmalı (entity_id yok)"
        );
    }

    /// WP-S: A station/junction geometry REPEATS in outbound and return shapes;
    /// per-shape emission counted the same PHYSICAL point repeatedly (TCDD: 28 notices =
    /// 8 distinct coordinates). Now emit one notice per coordinate.
    #[test]
    fn shp_020_reports_one_notice_per_physical_coordinate() {
        let mut shape_ti = ShapeInternTable::new();
        use crate::k2::shapes::ShapePointRecord;
        let mut records = records_with(
            vec![stop("A", 41.0, 29.0), stop("B", 41.1, 29.1)],
            vec![route("R1", 3)],
            vec![trip_sh("T1", "R1", "S0")],
            vec![
                stoptime("T1", 1, "A", (8, 0, 0), (8, 0, 0), 2),
                stoptime("T1", 2, "B", (8, 30, 0), (8, 30, 0), 3),
            ],
        );
        // 8 shapes (direction variants), ALL repeating at the same station point.
        let mut shapes = Vec::new();
        for k in 0..8u32 {
            let sid = format!("S{k}");
            shapes.push(ShapePointRecord::new(
                shape_ti.intern(&sid),
                Some(41.0),
                Some(29.0),
                Some(1),
                None,
                2,
            ));
            shapes.push(ShapePointRecord::new(
                shape_ti.intern(&sid),
                Some(41.001),
                Some(29.0),
                Some(2),
                None,
                3,
            ));
            shapes.push(ShapePointRecord::new(
                shape_ti.intern(&sid),
                Some(41.0),
                Some(29.0),
                Some(3),
                None,
                4,
            ));
        }
        records.shapes = shapes;
        records.shape_interns = shape_ti.clone();
        let result = analyze(&records, &empty_derived(), &default_config(), 20260514);
        let hits: Vec<_> = result
            .notices
            .iter()
            .filter(|n| n.rule_id == "SHP_020")
            .collect();
        assert_eq!(
            hits.len(),
            1,
            "8 shape aynı koordinatta → TEK bildirim (8 değil)"
        );
        assert_eq!(
            hits[0].entity_id.as_deref(),
            Some("S0"),
            "temsilci en küçük shape_id olmalı"
        );
        assert_eq!(
            hits[0]
                .details
                .as_ref()
                .and_then(|d| d.get("repeated_shapes"))
                .map(String::as_str),
            Some("8"),
            "kaç shape'te tekrarlandığı details'ta taşınmalı"
        );
        assert!(
            hits[0]
                .message
                .contains("8 güzergah şeklinde tekrarlanıyor"),
            "mesaj tekrarı söylemeli: {}",
            hits[0].message
        );
    }

    #[test]
    fn shp_009_saturation_aggregates_to_single_feed_notice() {
        let mut shape_ti = ShapeInternTable::new();
        use crate::k2::shapes::ShapePointRecord;
        // Self-intersection in 51 shapes → ONE feed-summary SHP_009.
        let mut records = records_with(
            vec![stop("A", 41.0, 29.0), stop("B", 41.1, 29.1)],
            vec![route("R1", 3)],
            vec![trip_sh("T1", "R1", "X0")],
            vec![
                stoptime("T1", 1, "A", (8, 0, 0), (8, 0, 0), 2),
                stoptime("T1", 2, "B", (8, 30, 0), (8, 30, 0), 3),
            ],
        );
        let mut shapes = Vec::new();
        for k in 0..51u32 {
            let sid = format!("X{k}");
            // 5 points; segment 0 (p1→p2) intersects segment 2 (p3→p4) in an X.
            // Each shape intersects at its OWN location: saturation now counts distinct INTERSECTION
            // LOCATIONS (direction variants sharing the same junction are grouped into one notice).
            let d = k as f64 * 0.1;
            let pts = [
                (41.000 + d, 29.000),
                (41.010 + d, 29.010),
                (41.010 + d, 29.000),
                (41.000 + d, 29.010),
                (41.020 + d, 29.020),
            ];
            for (i, (la, lo)) in pts.iter().enumerate() {
                shapes.push(ShapePointRecord::new(
                    shape_ti.intern(&sid),
                    Some(*la),
                    Some(*lo),
                    Some(i as u32 + 1),
                    None,
                    2 + i as u32,
                ));
            }
        }
        records.shapes = shapes;
        records.shape_interns = shape_ti.clone();
        let result = analyze(&records, &empty_derived(), &default_config(), 20260514);
        let hits: Vec<_> = result
            .notices
            .iter()
            .filter(|n| n.rule_id == "SHP_009")
            .collect();
        assert_eq!(hits.len(), 1, "eşik üstünde tek feed-özet SHP_009 beklenir");
        assert!(
            hits[0].entity_id.is_none(),
            "özet notice feed-seviyesi olmalı (entity_id yok)"
        );
    }

    /// A junction/station approach REPEATS in outbound and return shapes; per-shape emission
    /// counted the same physical intersection repeatedly (TCDD v2: 9 notices = 4 areas).
    #[test]
    fn shp_009_reports_one_notice_per_crossing_location() {
        let mut shape_ti = ShapeInternTable::new();
        use crate::k2::shapes::ShapePointRecord;
        let mut records = records_with(
            vec![stop("A", 41.0, 29.0), stop("B", 41.1, 29.1)],
            vec![route("R1", 3)],
            vec![trip_sh("T1", "R1", "X0")],
            vec![
                stoptime("T1", 1, "A", (8, 0, 0), (8, 0, 0), 2),
                stoptime("T1", 2, "B", (8, 30, 0), (8, 30, 0), 3),
            ],
        );
        // 4 shapes (direction variants), ALL intersecting at the same junction.
        let mut shapes = Vec::new();
        for k in 0..4u32 {
            let sid = format!("X{k}");
            let pts = [
                (41.000, 29.000),
                (41.010, 29.010),
                (41.010, 29.000),
                (41.000, 29.010),
                (41.020, 29.020),
            ];
            for (i, (la, lo)) in pts.iter().enumerate() {
                shapes.push(ShapePointRecord::new(
                    shape_ti.intern(&sid),
                    Some(*la),
                    Some(*lo),
                    Some(i as u32 + 1),
                    None,
                    2 + i as u32,
                ));
            }
        }
        records.shapes = shapes;
        records.shape_interns = shape_ti.clone();
        let result = analyze(&records, &empty_derived(), &default_config(), 20260514);
        let hits: Vec<_> = result
            .notices
            .iter()
            .filter(|n| n.rule_id == "SHP_009")
            .collect();
        assert_eq!(
            hits.len(),
            1,
            "4 shape aynı makasta kesişiyor → TEK bildirim"
        );
        assert_eq!(
            hits[0].entity_id.as_deref(),
            Some("X0"),
            "temsilci en küçük shape_id"
        );
        assert_eq!(
            hits[0]
                .details
                .as_ref()
                .and_then(|d| d.get("repeated_shapes"))
                .map(String::as_str),
            Some("4"),
        );
    }

    #[test]
    fn stm_061_catches_a_far_pair_that_consecutive_checks_miss() {
        // A→B and B→C are reasonable separately, A→C is impossible: the intermediate stop time
        // is too late, covering 60 km end-to-end in 6 minutes (600 km/h, bus threshold 120).
        let records = records_with(
            vec![
                stop("A", 41.000, 29.000),
                stop("B", 41.100, 29.000),
                stop("C", 41.540, 29.000),
            ],
            vec![route("R1", 3)],
            vec![trip("T1", "R1")],
            vec![
                stoptime("T1", 1, "A", (8, 0, 0), (8, 0, 0), 2),
                stoptime("T1", 2, "B", (8, 5, 0), (8, 5, 0), 3),
                stoptime("T1", 3, "C", (8, 6, 0), (8, 6, 0), 4),
            ],
        );
        let result = analyze(&records, &empty_derived(), &default_config(), 20260514);
        assert!(
            result.notices.iter().any(|n| n.rule_id == "STM_061"),
            "uzak çift ihlali STM_061 üretmeli: {:?}",
            result
                .notices
                .iter()
                .map(|n| n.rule_id.as_str())
                .collect::<Vec<_>>()
        );
    }

    #[test]
    fn stm_061_stays_silent_on_a_plausible_long_trip() {
        // Same 60 km, but in 90 minutes = 40 km/h. No pair violates the rule.
        let records = records_with(
            vec![
                stop("A", 41.000, 29.000),
                stop("B", 41.270, 29.000),
                stop("C", 41.540, 29.000),
            ],
            vec![route("R1", 3)],
            vec![trip("T1", "R1")],
            vec![
                stoptime("T1", 1, "A", (8, 0, 0), (8, 0, 0), 2),
                stoptime("T1", 2, "B", (8, 45, 0), (8, 45, 0), 3),
                stoptime("T1", 3, "C", (9, 30, 0), (9, 30, 0), 4),
            ],
        );
        let result = analyze(&records, &empty_derived(), &default_config(), 20260514);
        assert!(!result.notices.iter().any(|n| n.rule_id == "STM_061"));
    }

    #[test]
    fn stm_061_ignores_pairs_closer_than_the_far_threshold() {
        // Pairs under 10 km belong to the adjacent-pair check; even if the speed is impossible,
        // STM_061 stays silent, otherwise we would count the same fact as STM_012/STM_014 twice.
        let records = records_with(
            vec![
                stop("A", 41.000, 29.000),
                stop("B", 41.010, 29.000),
                stop("C", 41.040, 29.000),
            ],
            vec![route("R1", 3)],
            vec![trip("T1", "R1")],
            vec![
                stoptime("T1", 1, "A", (8, 0, 0), (8, 0, 0), 2),
                stoptime("T1", 2, "B", (8, 0, 30), (8, 0, 30), 3),
                stoptime("T1", 3, "C", (8, 1, 0), (8, 1, 0), 4),
            ],
        );
        let result = analyze(&records, &empty_derived(), &default_config(), 20260514);
        assert!(!result.notices.iter().any(|n| n.rule_id == "STM_061"));
    }

    #[test]
    fn whole_minute_zero_gap_threshold_follows_route_type() {
        // Same ~1.1 km distance is IMPOSSIBLE for a cable car (30 km/h → limit 0.5 km).
        // A fixed threshold would miss this case; it is caught because the threshold depends on route type.
        let records = records_with(
            vec![stop("A", 41.0, 29.0), stop("B", 41.0099, 29.0)],
            vec![route("R1", 6)], // teleferik
            vec![trip("T1", "R1")],
            vec![
                stoptime("T1", 1, "A", (8, 0, 0), (8, 0, 0), 2),
                stoptime("T1", 2, "B", (8, 0, 0), (8, 0, 0), 3),
            ],
        );
        let result = analyze(&records, &empty_derived(), &default_config(), 20260514);
        assert!(
            result.notices.iter().any(|n| n.rule_id == "STM_012"),
            "teleferik için 1.1 km bir dakikada alınamaz → STM_012 olmalı"
        );
    }

    // ── STM_020: zero travel time ─────────────────────────────────────────

    #[test]
    fn zero_travel_time_with_distance_produces_stm_020() {
        // Same arrival/departure time (with seconds → not minute rounding), different locations → STM_020.
        let records = records_with(
            vec![stop("A", 41.0, 29.0), stop("B", 41.05, 29.05)], // ~6 km
            vec![route("R1", 3)],
            vec![trip("T1", "R1")],
            vec![
                stoptime("T1", 1, "A", (8, 0, 30), (8, 0, 30), 2),
                stoptime("T1", 2, "B", (8, 0, 30), (8, 0, 30), 3), // same time, different location
            ],
        );
        let result = analyze(&records, &empty_derived(), &default_config(), 20260514);
        assert!(
            result.notices.iter().any(|n| n.rule_id == "STM_020"),
            "STM_020 olmalı"
        );
    }

    #[test]
    fn zero_travel_time_same_stop_no_stm_020() {
        // Same stop, same time → distance 0 → STM_020 must not fire.
        let records = records_with(
            vec![stop("A", 41.0, 29.0)],
            vec![route("R1", 3)],
            vec![trip("T1", "R1")],
            vec![
                stoptime("T1", 1, "A", (8, 0, 0), (8, 0, 0), 2),
                stoptime("T1", 2, "A", (8, 0, 0), (8, 0, 0), 3),
            ],
        );
        let result = analyze(&records, &empty_derived(), &default_config(), 20260514);
        assert!(
            !result.notices.iter().any(|n| n.rule_id == "STM_020"),
            "aynı durak STM_020 üretmemeli"
        );
    }

    #[test]
    fn empty_feed_no_notices() {
        use crate::k2::EntityRecords;
        let result = analyze(
            &EntityRecords::default(),
            &empty_derived(),
            &default_config(),
            20260514,
        );
        assert!(result.notices.is_empty());
    }

    // ── Format helper ─────────────────────────────────────────────────────────

    #[test]
    fn format_hms_correct() {
        assert_eq!(format_hms(3661), "01:01:01");
        assert_eq!(format_hms(86400), "24:00:00");
        assert_eq!(format_hms(0), "00:00:00");
    }

    #[test]
    fn is_regular_headway_distinguishes_rural_from_gap() {
        // Regular sparse (rural): equal intervals → regular (true) → OPR_001 suppressed.
        assert!(
            is_regular_headway(&[18000, 18000, 18000]),
            "5sa eşit aralık = düzenli"
        );
        assert!(
            is_regular_headway(&[36000]),
            "tek aralık (2 sefer) = kasıtlı"
        );
        assert!(is_regular_headway(&[]), "boş = kasıtlı");
        // Irregular (real gap): mostly frequent + one huge gap → irregular (false) → OPR_001 emitted.
        assert!(
            !is_regular_headway(&[900, 900, 900, 14400]),
            "15dk×3 + 4sa boşluk = düzensiz"
        );
        assert!(
            !is_regular_headway(&[600, 600, 7200]),
            "10dk×2 + 2sa = düzensiz"
        );
    }

    // ── WP-09b: FRQ_006 / FRQ_010 ────────────────────────────────────────────

    use crate::k2::frequencies::FrequencyRecord;

    fn frq(trip_id: &str, hw: u32, line: u64) -> FrequencyRecord {
        FrequencyRecord {
            trip_id: trip_id.into(),
            start_time: Some((8, 0, 0)),
            end_time: Some((20, 0, 0)),
            headway_secs: Some(hw),
            exact_times: None,
            row: Default::default(),
            line,
        }
    }

    #[test]
    fn long_headway_produces_frq_006() {
        let r = crate::k2::EntityRecords {
            frequencies: vec![frq("T1", 14401, 2)], // 4 hours 1 minute > 240 min threshold
            ..Default::default()
        };
        let result = analyze(&r, &empty_derived(), &default_config(), 20260514);
        assert!(result.notices.iter().any(|n| n.rule_id == "FRQ_006"));
    }

    #[test]
    fn short_headway_produces_frq_010() {
        let r = crate::k2::EntityRecords {
            frequencies: vec![frq("T1", 60, 2)], // 1 min ≤ 2 min bunching threshold
            ..Default::default()
        };
        let result = analyze(&r, &empty_derived(), &default_config(), 20260514);
        assert!(result.notices.iter().any(|n| n.rule_id == "FRQ_010"));
    }

    #[test]
    fn normal_headway_no_frq_notices() {
        let r = crate::k2::EntityRecords {
            frequencies: vec![frq("T1", 600, 2)], // 10 dk — normal
            ..Default::default()
        };
        let result = analyze(&r, &empty_derived(), &default_config(), 20260514);
        assert!(!result.notices.iter().any(|n| n.rule_id == "FRQ_006"));
        assert!(!result.notices.iter().any(|n| n.rule_id == "FRQ_010"));
    }

    // ── WP-09b: OPR_001 / OPR_003 ────────────────────────────────────────────

    #[test]
    fn large_route_headway_produces_opr_001() {
        // IRREGULAR large gap: 08:00/08:15/08:30 frequent, then 13:00 → gaps [15,15,270] min
        // high CV → real service gap → OPR_001 (rural automatic suppression does NOT apply).
        let records = records_with(
            vec![stop("A", 41.0, 29.0), stop("B", 41.01, 29.01)],
            vec![route("R1", 3)],
            vec![
                trip("T1", "R1"),
                trip("T2", "R1"),
                trip("T3", "R1"),
                trip("T4", "R1"),
            ],
            vec![
                stoptime("T1", 1, "A", (8, 0, 0), (8, 0, 0), 2),
                stoptime("T1", 2, "B", (8, 10, 0), (8, 10, 0), 3),
                stoptime("T2", 1, "A", (8, 15, 0), (8, 15, 0), 4),
                stoptime("T2", 2, "B", (8, 25, 0), (8, 25, 0), 5),
                stoptime("T3", 1, "A", (8, 30, 0), (8, 30, 0), 6),
                stoptime("T3", 2, "B", (8, 40, 0), (8, 40, 0), 7),
                stoptime("T4", 1, "A", (13, 0, 0), (13, 0, 0), 8),
                stoptime("T4", 2, "B", (13, 10, 0), (13, 10, 0), 9),
            ],
        );
        let result = analyze(&records, &empty_derived(), &default_config(), 20260514);
        assert!(result.notices.iter().any(|n| n.rule_id == "OPR_001"));
    }

    #[test]
    fn regular_sparse_headway_suppresses_opr_001() {
        // Regular sparse (rural): 06:00/11:00/16:00 → equal 300-min gaps → low CV → intentional
        // service → OPR_001 is NOT emitted, even if the threshold is exceeded (prevents false positives).
        let records = records_with(
            vec![stop("A", 41.0, 29.0), stop("B", 41.01, 29.01)],
            vec![route("R1", 3)],
            vec![trip("T1", "R1"), trip("T2", "R1"), trip("T3", "R1")],
            vec![
                stoptime("T1", 1, "A", (6, 0, 0), (6, 0, 0), 2),
                stoptime("T1", 2, "B", (6, 10, 0), (6, 10, 0), 3),
                stoptime("T2", 1, "A", (11, 0, 0), (11, 0, 0), 4),
                stoptime("T2", 2, "B", (11, 10, 0), (11, 10, 0), 5),
                stoptime("T3", 1, "A", (16, 0, 0), (16, 0, 0), 6),
                stoptime("T3", 2, "B", (16, 10, 0), (16, 10, 0), 7),
            ],
        );
        let result = analyze(&records, &empty_derived(), &default_config(), 20260514);
        assert!(
            !result.notices.iter().any(|n| n.rule_id == "OPR_001"),
            "düzenli seyrek bastırılmalı"
        );
    }

    #[test]
    fn departure_before_arrival_produces_stm_007() {
        // Real row-level error: stop B arrives at 09:00 but departs at 08:00 → STM_007.
        let records = records_with(
            vec![stop("A", 41.0, 29.0), stop("B", 41.01, 29.01)],
            vec![route("R1", 3)],
            vec![trip("T1", "R1")],
            vec![
                stoptime("T1", 1, "A", (8, 0, 0), (8, 0, 0), 2),
                stoptime("T1", 2, "B", (9, 0, 0), (8, 0, 0), 3),
            ],
        );
        let result = analyze(&records, &empty_derived(), &default_config(), 20260514);
        assert!(result.notices.iter().any(|n| n.rule_id == "STM_007"));
    }

    #[test]
    fn stm007_above_threshold_aggregates_with_sorted_examples() {
        let ids: Vec<String> = (0..51).rev().map(|i| format!("T{i:02}")).collect();
        let trips: Vec<_> = ids.iter().map(|id| trip(id, "R1")).collect();
        let mut stoptimes = Vec::with_capacity(102);
        for (i, id) in ids.iter().enumerate() {
            let line = 2 + (i as u64) * 2;
            stoptimes.push(stoptime(id, 1, "A", (8, 0, 0), (8, 0, 0), line));
            stoptimes.push(stoptime(id, 2, "B", (9, 0, 0), (8, 0, 0), line + 1));
        }
        let records = records_with(
            vec![stop("A", 41.0, 29.0), stop("B", 41.01, 29.01)],
            vec![route("R1", 3)],
            trips,
            stoptimes,
        );
        let result = analyze(&records, &empty_derived(), &default_config(), 20260514);
        let stm007: Vec<_> = result
            .notices
            .iter()
            .filter(|n| n.rule_id == "STM_007")
            .collect();
        assert_eq!(
            stm007.len(),
            1,
            "51 ihlal tek feed-özeti olmalı: {stm007:?}"
        );
        assert_eq!(stm007[0].entity_type, EntityType::Feed);
        assert_eq!(stm007[0].observed_value.as_deref(), Some("51"));
        assert_eq!(
            stm007[0]
                .details
                .as_ref()
                .and_then(|d| d.get("affected_rows"))
                .map(String::as_str),
            Some("51"),
        );
        assert_eq!(
            stm007[0]
                .details
                .as_ref()
                .and_then(|d| d.get("example_trips"))
                .map(String::as_str),
            Some("T00, T01, T02, T03, T04"),
        );
        // Sample rows and pattern for the planner view (without changing the summary count).
        let d = stm007[0].details.as_ref().expect("özet ayrıntısı");
        let get = |k: &str| d.get(k).map(String::as_str);
        assert_eq!(get("affected_trips"), Some("51"));
        assert_eq!(get("example_0_trip"), Some("T00"));
        assert_eq!(get("example_0_stop"), Some("B"));
        assert!(get("example_0_service").is_some(), "örnek satır takvimini taşımalı");
        assert_eq!(get("example_0_arrival"), Some("09:00:00"));
        assert_eq!(get("example_0_departure"), Some("08:00:00"));
        assert_eq!(get("example_0_prev_stop"), Some("A"));
        assert_eq!(get("example_0_prev_departure"), Some("08:00:00"));
        assert_eq!(get("example_19_trip"), Some("T19"));
        assert_eq!(get("example_20_trip"), None, "en fazla 20 örnek satır");
        assert_eq!(get("typical_gap_seconds"), Some("3600"));
        assert_eq!(get("typical_gap_rows"), Some("51"));
        assert_eq!(get("top_stops"), Some("B"));
    }

    #[test]
    fn stm007_at_or_below_threshold_keeps_pinpoint_notices() {
        let records = records_with(
            vec![stop("A", 41.0, 29.0), stop("B", 41.01, 29.01)],
            vec![route("R1", 3)],
            vec![trip("T2", "R1"), trip("T1", "R1")],
            vec![
                stoptime("T2", 1, "A", (8, 0, 0), (8, 0, 0), 2),
                stoptime("T2", 2, "B", (9, 0, 0), (8, 0, 0), 3),
                stoptime("T1", 1, "A", (10, 0, 0), (10, 0, 0), 4),
                stoptime("T1", 2, "B", (11, 0, 0), (10, 0, 0), 5),
            ],
        );
        let result = analyze(&records, &empty_derived(), &default_config(), 20260514);
        let stm007: Vec<_> = result
            .notices
            .iter()
            .filter(|n| n.rule_id == "STM_007")
            .collect();
        assert_eq!(
            stm007.len(),
            2,
            "düşük hacimde iki pinpoint notice korunmalı"
        );
        assert!(stm007.iter().all(|n| n.entity_type == EntityType::Trip));
        assert!(stm007.iter().all(|n| n.details.is_some()));
    }

    #[test]
    fn midnight_departure_suppresses_stm_007() {
        // Midnight: final stop arrives at 23:58, departs at 00:00 (should have been 24:00) → suppressed.
        let records = records_with(
            vec![stop("A", 41.0, 29.0), stop("B", 41.01, 29.01)],
            vec![route("R1", 3)],
            vec![trip("T1", "R1")],
            vec![
                stoptime("T1", 1, "A", (23, 50, 0), (23, 50, 0), 2),
                stoptime("T1", 2, "B", (23, 58, 0), (0, 0, 0), 3),
            ],
        );
        let result = analyze(&records, &empty_derived(), &default_config(), 20260514);
        assert!(
            !result.notices.iter().any(|n| n.rule_id == "STM_007"),
            "gece-yarısı kalkış bastırılmalı"
        );
    }

    #[test]
    fn stm045_threshold_respects_service_day_start_hour() {
        // 26:30 departure: VALID under the default config (service_day_start_hour=3 → threshold 27h).
        let records = records_with(
            vec![stop("A", 41.0, 29.0), stop("B", 41.01, 29.01)],
            vec![route("R1", 3)],
            vec![trip("T1", "R1")],
            vec![
                stoptime("T1", 1, "A", (26, 30, 0), (26, 30, 0), 2),
                stoptime("T1", 2, "B", (26, 40, 0), (26, 40, 0), 3),
            ],
        );
        let r = analyze(&records, &empty_derived(), &default_config(), 20260514);
        assert!(
            !r.notices.iter().any(|n| n.rule_id == "STM_045"),
            "26:30 default eşik (27h) altında STM_045 üretmemeli"
        );

        // start_hour=0 (normalization disabled) → threshold 26h → 26:30 anomaly.
        let mut cfg0 = default_config();
        cfg0.service_day_start_hour = 0;
        let r0 = analyze(&records, &empty_derived(), &cfg0, 20260514);
        assert!(
            r0.notices.iter().any(|n| n.rule_id == "STM_045"),
            "start_hour=0 iken 26:30 STM_045 üretmeli"
        );

        // 28:30 → above the default threshold (27h) → anomaly.
        let records2 = records_with(
            vec![stop("A", 41.0, 29.0), stop("B", 41.01, 29.01)],
            vec![route("R1", 3)],
            vec![trip("T2", "R1")],
            vec![
                stoptime("T2", 1, "A", (28, 30, 0), (28, 30, 0), 2),
                stoptime("T2", 2, "B", (28, 40, 0), (28, 40, 0), 3),
            ],
        );
        let r2 = analyze(&records2, &empty_derived(), &default_config(), 20260514);
        assert!(
            r2.notices.iter().any(|n| n.rule_id == "STM_045"),
            "28:30 default eşik (27h) üstünde STM_045 üretmeli"
        );
    }

    #[test]
    fn wheelchair_boarding_completeness_stp_037_038() {
        let mk = |stops: Vec<StopRecord>| {
            records_with(
                stops,
                vec![route("R1", 3)],
                vec![trip("T1", "R1")],
                vec![
                    stoptime("T1", 1, "A", (8, 0, 0), (8, 0, 0), 2),
                    stoptime("T1", 2, "B", (8, 10, 0), (8, 10, 0), 3),
                ],
            )
        };
        let mut a = stop("A", 41.0, 29.0);
        let mut b = stop("B", 41.01, 29.01);

        // All physical stops missing (wheelchair_boarding None) → STP_038, no STP_037.
        let r = analyze(
            &mk(vec![a.clone(), b.clone()]),
            &empty_derived(),
            &default_config(),
            20260514,
        );
        assert!(
            r.notices.iter().any(|n| n.rule_id == "STP_038"),
            "hepsi eksik → STP_038"
        );
        assert!(!r.notices.iter().any(|n| n.rule_id == "STP_037"));

        // One populated, one missing → STP_037, no STP_038.
        a.wheelchair_boarding = Some(1);
        let r2 = analyze(
            &mk(vec![a.clone(), b.clone()]),
            &empty_derived(),
            &default_config(),
            20260514,
        );
        assert!(
            r2.notices.iter().any(|n| n.rule_id == "STP_037"),
            "kısmi → STP_037"
        );
        assert!(!r2.notices.iter().any(|n| n.rule_id == "STP_038"));

        // All populated → neither notice.
        b.wheelchair_boarding = Some(2);
        let r3 = analyze(
            &mk(vec![a, b]),
            &empty_derived(),
            &default_config(),
            20260514,
        );
        assert!(
            !r3.notices
                .iter()
                .any(|n| n.rule_id == "STP_037" || n.rule_id == "STP_038"),
            "hepsi dolu → notice yok"
        );
    }

    #[test]
    fn small_route_headway_produces_opr_003() {
        // Two trips: 08:00 and 08:01 → 1 min ≤ 2 min bunching threshold.
        let records = records_with(
            vec![stop("A", 41.0, 29.0), stop("B", 41.01, 29.01)],
            vec![route("R1", 3)],
            vec![trip("T1", "R1"), trip("T2", "R1")],
            vec![
                stoptime("T1", 1, "A", (8, 0, 0), (8, 0, 0), 2),
                stoptime("T1", 2, "B", (8, 10, 0), (8, 10, 0), 3),
                stoptime("T2", 1, "A", (8, 1, 0), (8, 1, 0), 4),
                stoptime("T2", 2, "B", (8, 11, 0), (8, 11, 0), 5),
            ],
        );
        let result = analyze(&records, &empty_derived(), &default_config(), 20260514);
        assert!(result.notices.iter().any(|n| n.rule_id == "OPR_003"));
    }

    // ── OPR_001/OPR_003 new tests: dedup behavior ────────────────────────────

    fn trip_with_service(trip_id: &str, route_id: &str, service_id: &str) -> TripRecord {
        TEST_TI.with(|cell| {
            let mut ti = cell.borrow_mut();
            let ri = ti.route_ids.len() as u32;
            ti.route_ids.push(SmolStr::new(route_id));
            let si = ti.service_ids.len() as u32;
            ti.service_ids.push(SmolStr::new(service_id));
            TripRecord {
                trip_id: trip_id.into(),
                route_idx: ri,
                service_idx: si,
                shape_idx: 0,
                headsign_idx: 0,
                short_name_idx: 0,
                block_idx: 0,
                jp_office_idx: 0,
                jp_pattern_idx: 0,
                direction_id: None,
                wheelchair_accessible: None,
                bikes_allowed: None,
                cars_allowed: None,
                safe_duration_factor: None,
                safe_duration_offset: None,
                line: 2,
            }
        })
    }

    #[test]
    fn dedup_same_time_suppresses_opr_003() {
        // 5 trips, all departing at 08:00 → one unique time after dedup → bucket skipped → no OPR_003.
        // Config: bunching_threshold_min = 2 (same as default).
        let cfg = ValidatorConfig {
            bunching_threshold_min: 2,
            ..default_config()
        };
        let trips: Vec<TripRecord> = (1..=5)
            .map(|i| trip_with_service(&format!("T{i}"), "R1", "SVC"))
            .collect();
        let mut stoptimes = Vec::new();
        for i in 1u32..=5 {
            let tid = format!("T{i}");
            stoptimes.push(stoptime(&tid, 1, "A", (8, 0, 0), (8, 0, 0), (i * 2) as u64));
            stoptimes.push(stoptime(
                &tid,
                2,
                "B",
                (8, 10, 0),
                (8, 10, 0),
                (i * 2 + 1) as u64,
            ));
        }
        let records = records_with(
            vec![stop("A", 41.0, 29.0), stop("B", 41.01, 29.01)],
            vec![route("R1", 3)],
            trips,
            stoptimes,
        );
        let result = analyze(&records, &empty_derived(), &cfg, 20260514);
        assert!(
            !result.notices.iter().any(|n| n.rule_id == "OPR_003"),
            "tek unique kalkış saati dedup'tan sonra bucket atlanmalı → OPR_003 olmamalı"
        );
    }

    #[test]
    fn true_bunching_survives_dedup() {
        // 08:00 × 5 trip + 08:01 × 5 trip → dedup: [28800, 28860] → fark 60s = 1dk < 2dk → OPR_003
        // Config: bunching_threshold_min = 2
        let cfg = ValidatorConfig {
            bunching_threshold_min: 2,
            ..default_config()
        };
        let mut trips = Vec::new();
        let mut stoptimes = Vec::new();
        for i in 1u32..=5 {
            let tid = format!("TA{i}");
            trips.push(trip_with_service(&tid, "R1", "SVC"));
            stoptimes.push(stoptime(
                &tid,
                1,
                "A",
                (8, 0, 0),
                (8, 0, 0),
                (i * 10) as u64,
            ));
            stoptimes.push(stoptime(
                &tid,
                2,
                "B",
                (8, 10, 0),
                (8, 10, 0),
                (i * 10 + 1) as u64,
            ));
        }
        for i in 1u32..=5 {
            let tid = format!("TB{i}");
            trips.push(trip_with_service(&tid, "R1", "SVC"));
            stoptimes.push(stoptime(
                &tid,
                1,
                "A",
                (8, 1, 0),
                (8, 1, 0),
                (100 + i * 10) as u64,
            ));
            stoptimes.push(stoptime(
                &tid,
                2,
                "B",
                (8, 11, 0),
                (8, 11, 0),
                (100 + i * 10 + 1) as u64,
            ));
        }
        let records = records_with(
            vec![stop("A", 41.0, 29.0), stop("B", 41.01, 29.01)],
            vec![route("R1", 3)],
            trips,
            stoptimes,
        );
        let result = analyze(&records, &empty_derived(), &cfg, 20260514);
        // After dedup [28800, 28860] → min headway = 60s < 120s (2 min) → OPR_003 must fire.
        assert!(
            result.notices.iter().any(|n| n.rule_id == "OPR_003"),
            "gerçek sıkışma (60s < 120s eşiği) dedup'tan sonra da OPR_003 üretmeli"
        );
    }

    // ── WP-09b: CAL_008 / CAL_009 / CAL_010 / CAL_007 / CAL_012 ─────────────

    use crate::k2::calendar::CalendarRecord;

    fn cal_rec(
        service_id: &str,
        days: [Option<u32>; 7],
        start: (u32, u32, u32),
        end: (u32, u32, u32),
    ) -> CalendarRecord {
        CalendarRecord {
            service_id: service_id.into(),
            days,
            start_date: Some(start),
            end_date: Some(end),
            row: Default::default(),
            line: 2,
        }
    }

    fn all_days() -> [Option<u32>; 7] {
        [Some(1); 7]
    }

    #[test]
    fn expired_service_produces_cal_013_not_cal_009() {
        // One service expired → Info-level CAL_013, not CRITICAL CAL_009.
        // CAL_013 is calculated from combined active_dates (k5 populates it in the real pipeline).
        use crate::k5_derived::CalendarBitmap;
        let derived = DerivedData {
            calendar_bitmap: CalendarBitmap {
                active_dates: [(
                    "SVC".to_string(),
                    [20251201u32, 20251215, 20251231].into_iter().collect(),
                )]
                .into_iter()
                .collect(),
            },
            ..Default::default()
        };
        let records = crate::k2::EntityRecords {
            calendars: vec![cal_rec("SVC", all_days(), (2025, 1, 1), (2025, 12, 31))],
            ..Default::default()
        };
        // today = 20260514 > son aktif tarih 20251231
        let result = analyze(&records, &derived, &default_config(), 20260514);
        assert!(
            result.notices.iter().any(|n| n.rule_id == "CAL_013"),
            "süresi dolmuş tekil servis CAL_013 üretmeli"
        );
        assert!(
            !result.notices.iter().any(|n| n.rule_id == "CAL_009"),
            "tek servis durumunda k6'dan CAL_009 üretmemeli (k4 tümü dolmuşsa atar)"
        );
    }

    fn pth_stop(id: &str, lt: Option<u32>, parent: &str) -> StopRecord {
        let mut s = stop(id, 0.0, 0.0);
        s.location_type = lt;
        if !parent.is_empty() {
            s.row
                .insert("parent_station".to_string(), parent.to_string());
        }
        s
    }
    fn pth_pw(id: &str, from: &str, to: &str) -> crate::k2::pathways::PathwayRecord {
        crate::k2::pathways::PathwayRecord {
            pathway_id: id.into(),
            from_stop_id: from.into(),
            to_stop_id: to.into(),
            pathway_mode: Some(1),
            is_bidirectional: Some(1),
            length: None,
            traversal_time: None,
            stair_count: None,
            max_slope: None,
            min_width: None,
            row: Default::default(),
            line: 2,
        }
    }

    #[test]
    fn pth_012_nested_platform_reachable_via_boarding_area_no_fp() {
        // #50: station ST → platform PL(lt0) → boarding area BA(lt4); entrance EN(lt2).
        // Pathway EN↔BA. Platform record PL is NOT a graph node, but its child BA is reachable
        // from the entrance → PTH_012 MUST NOT produce a false positive.
        let recs = crate::k2::EntityRecords {
            stops: vec![
                pth_stop("ST", Some(1), ""),
                pth_stop("EN", Some(2), "ST"),
                pth_stop("PL", Some(0), "ST"),
                pth_stop("BA", Some(4), "PL"),
            ],
            pathways: vec![pth_pw("P1", "EN", "BA")],
            ..Default::default()
        };
        let mut derived = DerivedData::default();
        derived
            .pathway_graph
            .adjacency
            .insert("EN".into(), vec![("BA".into(), 0)]);
        derived
            .pathway_graph
            .adjacency
            .insert("BA".into(), vec![("EN".into(), 0)]);
        let mut v = Vec::new();
        let mut c = 0u32;
        check_pathway_analytics(&recs, &derived, &mut v, &mut c);
        assert!(
            !v.iter().any(|n| n.rule_id == "PTH_012"),
            "boarding-area çocuğu erişilebilirken PTH_012 çıkmamalı"
        );
    }

    #[test]
    fn pth_012_genuinely_unreachable_platform_fires() {
        // Platform PL is a graph node but unreachable from the entrance and has no lt4 child
        // → PTH_012 MUST fire (preserve the genuine accessibility gap).
        let recs = crate::k2::EntityRecords {
            stops: vec![
                pth_stop("ST", Some(1), ""),
                pth_stop("EN", Some(2), "ST"),
                pth_stop("PL", Some(0), "ST"),
            ],
            pathways: vec![pth_pw("P1", "EN", "X"), pth_pw("P2", "PL", "Y")],
            ..Default::default()
        };
        let mut derived = DerivedData::default();
        derived
            .pathway_graph
            .adjacency
            .insert("EN".into(), vec![("X".into(), 0)]);
        derived
            .pathway_graph
            .adjacency
            .insert("PL".into(), vec![("Y".into(), 0)]);
        let mut v = Vec::new();
        let mut c = 0u32;
        check_pathway_analytics(&recs, &derived, &mut v, &mut c);
        assert!(
            v.iter().any(|n| n.rule_id == "PTH_012"),
            "erişilemeyen peron PTH_012 üretmeli"
        );
    }

    #[test]
    fn pth_013_nested_platform_accessible_via_boarding_area_no_fp() {
        // #51: nested platform (PL lt0) → boarding area (BA lt4); EN→BA is accessible
        // (slope/width None). The child is reachable → PTH_013 MUST NOT produce a false positive.
        let recs = crate::k2::EntityRecords {
            stops: vec![
                pth_stop("ST", Some(1), ""),
                pth_stop("EN", Some(2), "ST"),
                pth_stop("PL", Some(0), "ST"),
                pth_stop("BA", Some(4), "PL"),
            ],
            pathways: vec![pth_pw("P1", "EN", "BA")],
            ..Default::default()
        };
        let mut derived = DerivedData::default();
        derived
            .pathway_graph
            .adjacency
            .insert("EN".into(), vec![("BA".into(), 0)]);
        derived
            .pathway_graph
            .adjacency
            .insert("BA".into(), vec![("EN".into(), 0)]);
        let mut v = Vec::new();
        let mut c = 0u32;
        check_pathway_analytics(&recs, &derived, &mut v, &mut c);
        assert!(
            !v.iter().any(|n| n.rule_id == "PTH_013"),
            "erişilebilir boarding-area çocuğu varken PTH_013 çıkmamalı"
        );
    }

    #[test]
    fn pth_013_inaccessible_station_fires() {
        // The only route is steep (max_slope 0.20 > 0.08) → no wheelchair access
        // → PTH_013 MUST fire (preserve the genuine accessibility gap).
        let mut pw = pth_pw("P1", "EN", "BA");
        pw.max_slope = Some(0.20);
        let recs = crate::k2::EntityRecords {
            stops: vec![
                pth_stop("ST", Some(1), ""),
                pth_stop("EN", Some(2), "ST"),
                pth_stop("PL", Some(0), "ST"),
                pth_stop("BA", Some(4), "PL"),
            ],
            pathways: vec![pw],
            ..Default::default()
        };
        let mut derived = DerivedData::default();
        derived
            .pathway_graph
            .adjacency
            .insert("EN".into(), vec![("BA".into(), 0)]);
        derived
            .pathway_graph
            .adjacency
            .insert("BA".into(), vec![("EN".into(), 0)]);
        let mut v = Vec::new();
        let mut c = 0u32;
        check_pathway_analytics(&recs, &derived, &mut v, &mut c);
        assert!(
            v.iter().any(|n| n.rule_id == "PTH_013"),
            "dik rotalı istasyon PTH_013 üretmeli"
        );
    }

    #[test]
    fn calendar_dates_extended_service_no_cal_013() {
        // CAL_013 FP fix: even if calendar.txt end_date is in the past, if calendar_dates extends
        // the service through today/the future (combined max_date >= today), it has NOT EXPIRED
        // → CAL_013 MUST NOT be emitted. (Root cause of the TriMet over-fire.)
        use crate::k5_derived::CalendarBitmap;
        // calendar.txt end_date would be in the past, but calendar_dates added 20260601 →
        // combined active_dates contains a future date.
        let derived = DerivedData {
            calendar_bitmap: CalendarBitmap {
                active_dates: [(
                    "SVC".to_string(),
                    [20251201u32, 20251231, 20260601].into_iter().collect(),
                )]
                .into_iter()
                .collect(),
            },
            ..Default::default()
        };
        let records = crate::k2::EntityRecords {
            calendars: vec![cal_rec("SVC", all_days(), (2025, 1, 1), (2025, 12, 31))],
            ..Default::default()
        };
        // today = 20260514 < 20260601 (extended active date).
        let result = analyze(&records, &derived, &default_config(), 20260514);
        assert!(
            !result.notices.iter().any(|n| n.rule_id == "CAL_013"),
            "calendar_dates ile hâlâ aktif servis CAL_013 üretmemeli (FP)"
        );
    }

    #[test]
    fn expired_variants_same_date_aggregate_to_one_cal_013() {
        // Two service_id variants ending on the same last active date (20251231, #30 pattern):
        // expect ONE notice per expiry signature instead of 2 CAL_013 notices per service.
        use crate::k5_derived::CalendarBitmap;
        let derived = DerivedData {
            calendar_bitmap: CalendarBitmap {
                active_dates: [
                    (
                        "A.724".to_string(),
                        [20251215u32, 20251231].into_iter().collect(),
                    ),
                    (
                        "B.724".to_string(),
                        [20251201u32, 20251231].into_iter().collect(),
                    ),
                ]
                .into_iter()
                .collect(),
            },
            ..Default::default()
        };
        let records = crate::k2::EntityRecords::default();
        let result = analyze(&records, &derived, &default_config(), 20260514);
        let cal013: Vec<_> = result
            .notices
            .iter()
            .filter(|n| n.rule_id == "CAL_013")
            .collect();
        assert_eq!(
            cal013.len(),
            1,
            "aynı expiry tarihini paylaşan iki varyant için tek CAL_013 beklenir"
        );
        assert_eq!(cal013[0].entity_id.as_deref(), Some("20251231"));
        let services = cal013[0]
            .details
            .as_ref()
            .and_then(|d| d.get("services"))
            .map(String::as_str)
            .unwrap_or("");
        assert!(
            services.contains("A.724") && services.contains("B.724"),
            "details.services her iki varyantı listelemeli, bulundu: {services}"
        );
    }

    #[test]
    fn expiring_soon_produces_cal_008() {
        // End date is 10 days away (< 30-day warning threshold).
        let records = crate::k2::EntityRecords {
            calendars: vec![cal_rec("SVC", all_days(), (2026, 5, 1), (2026, 5, 24))],
            ..Default::default()
        };
        let result = analyze(&records, &empty_derived(), &default_config(), 20260514);
        assert!(result.notices.iter().any(|n| n.rule_id == "CAL_008"));
    }

    #[test]
    fn short_service_produces_cal_010() {
        // Long publication window (≥30 days) → absolute-threshold path: 3 active days < 7.
        // The LONG service opens the window; SVC is genuinely short within that window.
        use crate::k5_derived::CalendarBitmap;
        let derived = DerivedData {
            calendar_bitmap: CalendarBitmap {
                active_dates: [
                    (
                        "SVC".to_string(),
                        [20260511u32, 20260512, 20260513].into_iter().collect(),
                    ),
                    (
                        "LONG".to_string(),
                        [20260401u32, 20260601].into_iter().collect(),
                    ),
                ]
                .into_iter()
                .collect(),
            },
            ..Default::default()
        };
        let records = crate::k2::EntityRecords::default();
        let result = analyze(&records, &derived, &default_config(), 20260514);
        assert!(
            result
                .notices
                .iter()
                .any(|n| n.rule_id == "CAL_010" && n.entity_id.as_deref() == Some("SVC")),
            "62 günlük pencerede 3 aktif gün → CAL_010 çıkmalı"
        );
    }

    // ── WP-P: OPR_019/OPR_020 operational trip identity ──────────────────────
    //
    // Having ≥2 services active on the same day is NOT a conflict BY ITSELF: two trains running
    // in opposite directions use separate service_id values (Ankara→Kurtalan 22014 / Kurtalan→Ankara 52013).
    // A conflict is the SAME operational trip being active from both services.

    /// Two trains running in different directions active on the same day → NOT a conflict.
    #[test]
    fn opr_019_ignores_opposite_direction_trains_on_the_same_day() {
        use crate::k5_derived::CalendarBitmap;
        // 22014 Ankara→Kurtalan (SVC_A), 52013 Kurtalan→Ankara (SVC_B) — opposite directions.
        let mut t1 = trip_with_service("22014", "R1", "SVC_A");
        t1.direction_id = Some(0);
        let mut t2 = trip_with_service("52013", "R1", "SVC_B");
        t2.direction_id = Some(1);
        let r = crate::k2::EntityRecords {
            stops: vec![stop("ANK", 39.9, 32.8), stop("KUR", 37.9, 41.8)],
            routes: vec![route("R1", 2)],
            trips: vec![t1, t2],
            trip_interns: take_ti(),
            stop_times: vec![
                stoptime("22014", 1, "ANK", (8, 0, 0), (8, 0, 0), 2),
                stoptime("22014", 2, "KUR", (20, 0, 0), (20, 0, 0), 3),
                stoptime("52013", 1, "KUR", (8, 0, 0), (8, 0, 0), 4),
                stoptime("52013", 2, "ANK", (20, 0, 0), (20, 0, 0), 5),
            ],
            ..Default::default()
        };
        let derived = DerivedData {
            calendar_bitmap: CalendarBitmap {
                active_dates: [
                    ("SVC_A".to_string(), [20260518u32].into_iter().collect()),
                    ("SVC_B".to_string(), [20260518u32].into_iter().collect()),
                ]
                .into_iter()
                .collect(),
            },
            ..Default::default()
        };
        let result = analyze(&r, &derived, &default_config(), 20260518);
        assert!(
            !result
                .notices
                .iter()
                .any(|n| n.rule_id == "OPR_019" || n.rule_id == "OPR_020"),
            "zıt yönlerde iki ayrı tren çakışma değildir"
        );
    }

    /// Positive control: if the SAME operational trip (same direction, departure, stop sequence)
    /// is active from both services, it is a real override conflict and MUST be caught.
    /// Without this test, the negative test above could pass vacuously.
    #[test]
    fn opr_019_still_catches_the_same_trip_active_from_two_services() {
        use crate::k5_derived::CalendarBitmap;
        let mut t1 = trip_with_service("T_BASE", "R1", "SVC_A");
        t1.direction_id = Some(0);
        let mut t2 = trip_with_service("T_OVERRIDE", "R1", "SVC_B");
        t2.direction_id = Some(0);
        // Both trips are identical: 08:00 ANK → 20:00 KUR.
        let r = crate::k2::EntityRecords {
            stops: vec![stop("ANK", 39.9, 32.8), stop("KUR", 37.9, 41.8)],
            routes: vec![route("R1", 2)],
            trips: vec![t1, t2],
            trip_interns: take_ti(),
            stop_times: vec![
                stoptime("T_BASE", 1, "ANK", (8, 0, 0), (8, 0, 0), 2),
                stoptime("T_BASE", 2, "KUR", (20, 0, 0), (20, 0, 0), 3),
                stoptime("T_OVERRIDE", 1, "ANK", (8, 0, 0), (8, 0, 0), 4),
                stoptime("T_OVERRIDE", 2, "KUR", (20, 0, 0), (20, 0, 0), 5),
            ],
            ..Default::default()
        };
        let derived = DerivedData {
            calendar_bitmap: CalendarBitmap {
                active_dates: [
                    ("SVC_A".to_string(), [20260518u32].into_iter().collect()),
                    ("SVC_B".to_string(), [20260518u32].into_iter().collect()),
                ]
                .into_iter()
                .collect(),
            },
            ..Default::default()
        };
        let result = analyze(&r, &derived, &default_config(), 20260518);
        assert!(
            result.notices.iter().any(|n| n.rule_id == "OPR_019"),
            "aynı sefer iki servisten aktif → gerçek çakışma, OPR_019 çıkmalı"
        );
    }

    // ── WP-R: CLD_007 only when a base schedule exists ───────────────────────

    #[test]
    fn cld_007_preserves_first_calendar_match_for_duplicate_service_id() {
        use crate::k2::calendar_dates::CalendarDateIndex;
        use crate::k5_derived::CalendarBitmap;

        // The current implementation uses the first matching calendar row. Keep that
        // behavior explicit: a later duplicate with Monday enabled must not change the
        // result selected by `.find(...)`.
        let no_monday = [Some(0); 7];
        let mut calendar_dates = CalendarDateIndex::default();
        calendar_dates.exception_count.insert("SVC".into(), 4);
        let records = crate::k2::EntityRecords {
            calendars: vec![
                cal_rec("SVC", no_monday, (2026, 5, 1), (2026, 5, 31)),
                cal_rec("SVC", all_days(), (2026, 5, 1), (2026, 5, 31)),
            ],
            calendar_dates,
            ..Default::default()
        };
        let active_dates: std::collections::HashSet<u32> =
            [20260518, 20260519, 20260520, 20260521].into_iter().collect();
        let derived = DerivedData {
            calendar_bitmap: CalendarBitmap {
                active_dates: [("SVC".to_string(), active_dates)].into_iter().collect(),
            },
            ..Default::default()
        };

        let result = analyze(&records, &derived, &default_config(), 20260519);
        assert!(
            !result.notices.iter().any(|n| n.rule_id == "CLD_007"),
            "ilk duplicate calendar kaydı pazartesi kapalıysa sonraki kayıt CLD_007'yi etkinleştirmemeli"
        );
    }

    /// In a feed intentionally defined only through `calendar_dates.txt`, EVERY active day is
    /// an exception; `count > active/2` is mathematically guaranteed, and the notice measures
    /// nothing like a "deviation from a base schedule".
    #[test]
    fn cld_007_silent_when_the_feed_has_no_calendar_txt() {
        use crate::k2::calendar_dates::CalendarDateIndex;
        use crate::k5_derived::CalendarBitmap;
        // calendars EMPTY — the feed uses only calendar_dates.
        let mut cd = CalendarDateIndex::default();
        cd.exception_count.insert("SVC".into(), 4);
        cd.added
            .insert("SVC".into(), vec![20260518, 20260519, 20260520, 20260521]);
        let r = crate::k2::EntityRecords {
            calendar_dates: cd,
            ..Default::default()
        };
        let derived = DerivedData {
            calendar_bitmap: CalendarBitmap {
                active_dates: [(
                    "SVC".to_string(),
                    [20260518u32, 20260519, 20260520, 20260521]
                        .into_iter()
                        .collect(),
                )]
                .into_iter()
                .collect(),
            },
            ..Default::default()
        };
        let result = analyze(&r, &derived, &default_config(), 20260519);
        assert!(
            !result.notices.iter().any(|n| n.rule_id == "CLD_007"),
            "calendar.txt yoksa CLD_007 üretilmemeli (feed'in modelleme seçimini geri okur)"
        );
    }

    // ── WP-O: CAL_010 / CAL_017 according to feed context ────────────────────

    /// A train running 4–6 days in a 17-day publication window (Vangölü Ekspresi pattern)
    /// is not sparse but NORMAL; an absolute 7-day threshold would flag every service there.
    #[test]
    fn cal_010_uses_a_ratio_on_short_feed_windows() {
        use crate::k5_derived::CalendarBitmap;
        // Window 20260728–20260813 = 17 days (July 28–31 + August 1–13).
        let full: std::collections::HashSet<u32> = (28..=31u32)
            .map(|d| 20260700 + d)
            .chain((1..=13u32).map(|d| 20260800 + d))
            .collect();
        let derived = DerivedData {
            calendar_bitmap: CalendarBitmap {
                active_dates: [
                    ("DAILY".to_string(), full),
                    // 5 active days — 29% of 17 days, above the 20% threshold.
                    (
                        "VANGOLU".to_string(),
                        [20260728u32, 20260730, 20260801, 20260803, 20260805]
                            .into_iter()
                            .collect(),
                    ),
                    // 2 active days — 12%, below the threshold → genuine stub.
                    (
                        "STUB".to_string(),
                        [20260728u32, 20260729].into_iter().collect(),
                    ),
                ]
                .into_iter()
                .collect(),
            },
            ..Default::default()
        };
        let records = crate::k2::EntityRecords::default();
        let result = analyze(&records, &derived, &default_config(), 20260801);
        assert!(
            !result
                .notices
                .iter()
                .any(|n| n.rule_id == "CAL_010" && n.entity_id.as_deref() == Some("VANGOLU")),
            "17 günlük pencerede 5 aktif gün (%29) → CAL_010 ÇIKMAMALI"
        );
        assert!(
            result
                .notices
                .iter()
                .any(|n| n.rule_id == "CAL_010" && n.entity_id.as_deref() == Some("STUB")),
            "17 günlük pencerede 2 aktif gün (%12) → CAL_010 çıkmalı"
        );
    }

    /// A SINGLE service starting in the future is not an error while other services are active today.
    #[test]
    fn cal_017_silent_when_the_feed_has_already_started() {
        use crate::k5_derived::CalendarBitmap;
        let derived = DerivedData {
            calendar_bitmap: CalendarBitmap {
                active_dates: [
                    (
                        "ACTIVE".to_string(),
                        [20260510u32, 20260514, 20260520].into_iter().collect(),
                    ),
                    (
                        "SOON".to_string(),
                        [20260516u32, 20260517].into_iter().collect(),
                    ),
                ]
                .into_iter()
                .collect(),
            },
            ..Default::default()
        };
        let records = crate::k2::EntityRecords::default();
        let result = analyze(&records, &derived, &default_config(), 20260514);
        assert!(
            !result.notices.iter().any(|n| n.rule_id == "CAL_017"),
            "feed bugün çalışıyorken ileride başlayan servis CAL_017 üretmemeli"
        );
    }

    /// If the ENTIRE feed is in the future, the warning is meaningful and retained.
    #[test]
    fn cal_017_fires_when_the_whole_feed_is_still_in_the_future() {
        use crate::k5_derived::CalendarBitmap;
        let derived = DerivedData {
            calendar_bitmap: CalendarBitmap {
                active_dates: [
                    (
                        "A".to_string(),
                        [20260601u32, 20260602].into_iter().collect(),
                    ),
                    ("B".to_string(), [20260610u32].into_iter().collect()),
                ]
                .into_iter()
                .collect(),
            },
            ..Default::default()
        };
        let records = crate::k2::EntityRecords::default();
        let result = analyze(&records, &derived, &default_config(), 20260514);
        let ids: Vec<&str> = result
            .notices
            .iter()
            .filter(|n| n.rule_id == "CAL_017")
            .filter_map(|n| n.entity_id.as_deref())
            .collect();
        assert_eq!(
            ids,
            vec!["A", "B"],
            "tümü gelecekteyse her servis için çıkmalı ve SIRALI olmalı"
        );
    }

    #[test]
    fn global_future_only_feed_keeps_quality_and_operational_notices() {
        use crate::k5_derived::CalendarBitmap;

        let records = records_with(
            vec![stop("A", 41.0, 29.0), stop("B", 41.1, 29.1)],
            vec![route("R1", 3)],
            vec![trip("T1", "R1")],
            vec![
                stoptime("T1", 1, "A", (8, 0, 0), (8, 0, 0), 2),
                stoptime("T1", 2, "B", (8, 10, 0), (8, 10, 0), 3),
            ],
        );
        let derived = DerivedData {
            calendar_bitmap: CalendarBitmap {
                active_dates: [("SVC".to_string(), [20260601u32].into_iter().collect())]
                    .into_iter()
                    .collect(),
            },
            ..Default::default()
        };

        let result = analyze(&records, &derived, &default_config(), 20260514);
        let cal015 = result
            .notices
            .iter()
            .find(|n| n.rule_id == "CAL_015")
            .expect("global future-only feed CAL_015 üretmeli");
        assert_eq!(cal015.severity, gtfs_core::Severity::Bilgi);
        for rule in ["CAL_017", "TRP_023"] {
            assert!(
                result.notices.iter().any(|n| n.rule_id == rule),
                "global future-only feed {rule} üretmeli"
            );
        }
        // A service that has not started is not a CAL_024 case; the feed-level case is covered above.
        assert!(!result.notices.iter().any(|n| n.rule_id == "CAL_024"));
    }

    #[test]
    fn non_jp_future_only_feed_keeps_global_behavior() {
        use crate::k5_derived::CalendarBitmap;

        let mut records = records_with(
            vec![stop("A", 41.0, 29.0), stop("B", 41.1, 29.1)],
            vec![route("R1", 3)],
            vec![trip("T1", "R1")],
            vec![
                stoptime("T1", 1, "A", (8, 0, 0), (8, 0, 0), 2),
                stoptime("T1", 2, "B", (8, 10, 0), (8, 10, 0), 3),
            ],
        );
        records.is_gtfs_jp = Some(false);
        let mut feed_info = feed_info_row();
        feed_info.feed_start_date = Some((2026, 6, 1));
        records.feed_info = vec![feed_info];
        let derived = DerivedData {
            calendar_bitmap: CalendarBitmap {
                active_dates: [("SVC".to_string(), [20260601u32].into_iter().collect())]
                    .into_iter()
                    .collect(),
            },
            ..Default::default()
        };

        let result = analyze(&records, &derived, &default_config(), 20260514);
        assert!(
            result
                .notices
                .iter()
                .any(|n| n.rule_id == "CAL_015" && n.severity == gtfs_core::Severity::Bilgi),
            "non-JP future-only feed global CAL_015 davranışını korumalı"
        );
        for rule in ["CAL_017", "TRP_023", "FIN_016"] {
            assert!(
                result.notices.iter().any(|n| n.rule_id == rule),
                "non-JP future-only feed {rule} üretmeli"
            );
        }
        assert!(!result.notices.iter().any(|n| n.rule_id == "CAL_024"));
    }

    #[test]
    fn gtfs_jp_future_only_feed_emits_one_zero_penalty_calendar_notice() {
        use crate::k5_derived::CalendarBitmap;

        let mut records = records_with(
            vec![stop("A", 41.0, 29.0), stop("B", 41.1, 29.1)],
            vec![route("R1", 3)],
            vec![trip("T1", "R1")],
            vec![
                stoptime("T1", 1, "A", (8, 0, 0), (8, 0, 0), 2),
                stoptime("T1", 2, "B", (8, 10, 0), (8, 10, 0), 3),
            ],
        );
        records.is_gtfs_jp = Some(true);
        let mut feed_info = feed_info_row();
        feed_info.feed_start_date = Some((2026, 6, 1));
        records.feed_info = vec![feed_info];
        let derived = DerivedData {
            calendar_bitmap: CalendarBitmap {
                active_dates: [(
                    "SVC".to_string(),
                    [20260601u32, 20260602].into_iter().collect(),
                )]
                .into_iter()
                .collect(),
            },
            ..Default::default()
        };

        let result = analyze(&records, &derived, &default_config(), 20260514);
        let cal015: Vec<_> = result
            .notices
            .iter()
            .filter(|n| n.rule_id == "CAL_015")
            .collect();
        assert_eq!(cal015.len(), 1, "tek feed-level CAL_015 beklenir");
        assert_eq!(cal015[0].severity, gtfs_core::Severity::Bilgi);
        assert_eq!(
            cal015[0]
                .details
                .as_ref()
                .and_then(|details| details.get("first_service_date"))
                .map(String::as_str),
            Some("20260601")
        );
        // The en/ja/fr template is selected for this variant; otherwise the generic "fix the calendar" text is used.
        assert_eq!(
            cal015[0]
                .details
                .as_ref()
                .and_then(|details| details.get("message_variant"))
                .map(String::as_str),
            Some("future_only_jp")
        );
        for rule in ["CAL_017", "CAL_024", "TRP_023", "FIN_016"] {
            assert!(
                !result.notices.iter().any(|n| n.rule_id == rule),
                "GTFS-JP future-only feed {rule} üretmemeli"
            );
        }
    }

    #[test]
    fn gtfs_jp_feed_with_current_service_keeps_calendar_gap_notice() {
        use crate::k5_derived::CalendarBitmap;

        let mut records = records_with(
            vec![stop("A", 41.0, 29.0), stop("B", 41.1, 29.1)],
            vec![route("R1", 3)],
            vec![
                trip_with_service("T_ACTIVE", "R1", "ACTIVE"),
                trip_with_service("T_GAP", "R1", "GAP"),
            ],
            vec![
                stoptime("T_ACTIVE", 1, "A", (8, 0, 0), (8, 0, 0), 2),
                stoptime("T_ACTIVE", 2, "B", (8, 10, 0), (8, 10, 0), 3),
                stoptime("T_GAP", 1, "A", (9, 0, 0), (9, 0, 0), 4),
                stoptime("T_GAP", 2, "B", (9, 10, 0), (9, 10, 0), 5),
            ],
        );
        records.is_gtfs_jp = Some(true);
        let derived = DerivedData {
            calendar_bitmap: CalendarBitmap {
                active_dates: [
                    ("ACTIVE".to_string(), [20260514u32].into_iter().collect()),
                    // Calendar covering today but with no active day in the next 7 days.
                    ("GAP".to_string(), [20260501u32, 20260601].into_iter().collect()),
                ]
                .into_iter()
                .collect(),
            },
            ..Default::default()
        };

        let result = analyze(&records, &derived, &default_config(), 20260514);
        assert!(
            result
                .notices
                .iter()
                .any(|n| n.rule_id == "CAL_024" && n.entity_id.as_deref() == Some("GAP")),
            "bugünü kapsayan ama 7 gün boşluklu takvimin CAL_024 sinyali JP istisnasıyla kaybolmamalı"
        );
        assert!(
            !result.notices.iter().any(|n| n.rule_id == "CAL_015"),
            "feed bugün başladığında future-only CAL_015 üretilmemeli"
        );
    }

    #[test]
    fn past_service_gap_produces_cal_007_not_cal_012() {
        use crate::k5_derived::CalendarBitmap;
        let mut derived = DerivedData::default();
        // Past gap (Jan→Feb 2026, today=May 2026): CAL_007 yes, CAL_012 no.
        let mut dates = std::collections::HashSet::new();
        dates.insert(20260101u32);
        dates.insert(20260201u32);
        derived.calendar_bitmap = CalendarBitmap {
            active_dates: [("SVC".to_string(), dates)].into_iter().collect(),
        };
        let records = crate::k2::EntityRecords::default();
        let result = analyze(&records, &derived, &default_config(), 20260514);
        assert!(
            result.notices.iter().any(|n| n.rule_id == "CAL_007"),
            "Geçmişteki boşluk CAL_007 üretmeli"
        );
        assert!(
            !result.notices.iter().any(|n| n.rule_id == "CAL_012"),
            "Geçmişteki boşluk CAL_012 üretmemeli"
        );
    }

    /// Goshogawara (2026-09-19): the feed runs today, and the winter schedule starts on December 1.
    /// The future period in the same dataset is not CAL_024; to prove the guard, a calendar
    /// that has started but has a 7-day gap is still reported.
    #[test]
    fn cal_024_skips_not_yet_started_service_in_an_active_feed() {
        use crate::k5_derived::CalendarBitmap;
        let records = records_with(
            vec![stop("A", 41.0, 29.0), stop("B", 41.1, 29.1)],
            vec![route("R1", 3)],
            vec![
                trip_with_service("T_NOW", "R1", "SUMMER"),
                trip_with_service("T_WINTER", "R1", "WINTER"),
                trip_with_service("T_HOLE", "R1", "HOLE"),
            ],
            vec![
                stoptime("T_NOW", 1, "A", (8, 0, 0), (8, 0, 0), 2),
                stoptime("T_NOW", 2, "B", (8, 10, 0), (8, 10, 0), 3),
                stoptime("T_WINTER", 1, "A", (9, 0, 0), (9, 0, 0), 4),
                stoptime("T_WINTER", 2, "B", (9, 10, 0), (9, 10, 0), 5),
                stoptime("T_HOLE", 1, "A", (10, 0, 0), (10, 0, 0), 6),
                stoptime("T_HOLE", 2, "B", (10, 10, 0), (10, 10, 0), 7),
            ],
        );
        let derived = DerivedData {
            calendar_bitmap: CalendarBitmap {
                active_dates: [
                    ("SUMMER".to_string(), [20260919u32, 20260920].into_iter().collect()),
                    ("WINTER".to_string(), [20261201u32, 20261202].into_iter().collect()),
                    ("HOLE".to_string(), [20260901u32, 20261015].into_iter().collect()),
                ]
                .into_iter()
                .collect(),
            },
            ..Default::default()
        };
        let result = analyze(&records, &derived, &default_config(), 20260919);
        let ids: Vec<_> = result
            .notices
            .iter()
            .filter(|n| n.rule_id == "CAL_024")
            .filter_map(|n| n.entity_id.as_deref())
            .collect();
        assert_eq!(ids, ["HOLE"], "yalnız başlamış ve 7 gün boşluklu takvim raporlanmalı");
    }

    #[test]
    fn cal_024_aggregates_inactive_service_to_single_service_notice() {
        use crate::k5_derived::CalendarBitmap;
        let mut derived = DerivedData::default();
        // SVC calendar was active only in the past → not in the next 7 days (20260514+).
        let dates: std::collections::HashSet<u32> = [20260101u32, 20260201].into_iter().collect();
        derived.calendar_bitmap = CalendarBitmap {
            active_dates: [("SVC".to_string(), dates)].into_iter().collect(),
        };
        // Both trips use the same SVC calendar → one notice per calendar instead of two per trip.
        let records = records_with(
            vec![stop("A", 41.0, 29.0), stop("B", 41.1, 29.1)],
            vec![route("R1", 3)],
            vec![trip("T1", "R1"), trip("T2", "R1")],
            vec![
                stoptime("T1", 1, "A", (8, 0, 0), (8, 0, 0), 2),
                stoptime("T1", 2, "B", (8, 10, 0), (8, 10, 0), 3),
                stoptime("T2", 1, "A", (9, 0, 0), (9, 0, 0), 4),
                stoptime("T2", 2, "B", (9, 10, 0), (9, 10, 0), 5),
            ],
        );
        let result = analyze(&records, &derived, &default_config(), 20260514);
        let trp: Vec<_> = result
            .notices
            .iter()
            .filter(|n| n.rule_id == "CAL_024")
            .collect();
        assert_eq!(
            trp.len(),
            1,
            "aktif olmayan tek takvim için tek CAL_024 beklenir"
        );
        assert_eq!(trp[0].entity_type, gtfs_core::EntityType::Service);
        assert_eq!(trp[0].entity_id.as_deref(), Some("SVC"));
        assert_eq!(
            trp[0].observed_value.as_deref(),
            Some("2"),
            "2 sefer etkilenmeli"
        );
        assert_eq!(trp[0].severity, gtfs_core::Severity::Bilgi);
    }

    #[test]
    fn near_future_service_gap_produces_cal_012_not_cal_007() {
        use crate::k5_derived::CalendarBitmap;
        let mut derived = DerivedData::default();
        // Near-future gap (today=20260514, gap 20260516→20260601 = 15 days > threshold 7).
        let mut dates = std::collections::HashSet::new();
        dates.insert(20260516u32);
        dates.insert(20260601u32);
        derived.calendar_bitmap = CalendarBitmap {
            active_dates: [("SVC".to_string(), dates)].into_iter().collect(),
        };
        let records = crate::k2::EntityRecords::default();
        let result = analyze(&records, &derived, &default_config(), 20260514);
        // #29: for a near-future gap, CAL_012 (High) is reported INSTEAD OF CAL_007 (no double emission).
        assert!(
            result.notices.iter().any(|n| n.rule_id == "CAL_012"),
            "Yakın gelecek boşluk CAL_012 üretmeli"
        );
        assert!(
            !result.notices.iter().any(|n| n.rule_id == "CAL_007"),
            "Yakın gelecek boşluk CAL_007 üretmemeli (CAL_012 onun yerine geçer)"
        );
    }

    #[test]
    fn shared_calendar_gap_aggregates_across_services_to_one_notice() {
        use crate::k5_derived::CalendarBitmap;
        let mut derived = DerivedData::default();
        // Two service_ids share the same past gap (20260101→20260201, ~30 days ≥ big_gap_days)
        // (#30): ONE notice per gap signature instead of 2 CAL_007 notices per service.
        let dates_a: std::collections::HashSet<u32> = [20260101u32, 20260201].into_iter().collect();
        let dates_b: std::collections::HashSet<u32> = [20260101u32, 20260201].into_iter().collect();
        derived.calendar_bitmap = CalendarBitmap {
            active_dates: [
                ("SVC_A".to_string(), dates_a),
                ("SVC_B".to_string(), dates_b),
            ]
            .into_iter()
            .collect(),
        };
        let records = crate::k2::EntityRecords::default();
        let result = analyze(&records, &derived, &default_config(), 20260514);
        let cal007: Vec<_> = result
            .notices
            .iter()
            .filter(|n| n.rule_id == "CAL_007")
            .collect();
        assert_eq!(
            cal007.len(),
            1,
            "aynı boşluğu paylaşan iki servis için tek CAL_007 beklenir"
        );
        assert_eq!(cal007[0].entity_id.as_deref(), Some("20260101-20260201"));
        let services = cal007[0]
            .details
            .as_ref()
            .and_then(|d| d.get("services"))
            .map(String::as_str)
            .unwrap_or("");
        assert!(
            services.contains("SVC_A") && services.contains("SVC_B"),
            "details.services her iki servisi listelemeli, bulundu: {services}"
        );
    }

    #[test]
    fn gap_below_big_gap_threshold_no_cal_007() {
        // big_gap_days=14: a 10-day gap (< 14) must not produce CAL_007/012. This is the regression
        // guard for MD alignment — with the old service_gap_days=7 threshold, this gap incorrectly fired.
        use crate::k5_derived::CalendarBitmap;
        let mut derived = DerivedData::default();
        // 20260101 → 20260112 = 10-day gap (< 14).
        let dates: std::collections::HashSet<u32> = [20260101u32, 20260112].into_iter().collect();
        derived.calendar_bitmap = CalendarBitmap {
            active_dates: [("SVC".to_string(), dates)].into_iter().collect(),
        };
        let records = crate::k2::EntityRecords::default();
        let result = analyze(&records, &derived, &default_config(), 20260514);
        assert!(
            !result
                .notices
                .iter()
                .any(|n| n.rule_id == "CAL_007" || n.rule_id == "CAL_012"),
            "big_gap_days eşiğinin altındaki boşluk (10<14) CAL_007/012 üretmemeli"
        );
    }

    // ── CAL_021: service covering today but with no trips active soon ─────────

    fn trip_svc(trip_id: &str, sid: &str) -> TripRecord {
        TEST_TI.with(|cell| {
            let mut ti = cell.borrow_mut();
            let ri = ti.route_ids.len() as u32;
            ti.route_ids.push(SmolStr::new("R1"));
            let si = ti.service_ids.len() as u32;
            ti.service_ids.push(SmolStr::new(sid));
            TripRecord {
                trip_id: trip_id.into(),
                route_idx: ri,
                service_idx: si,
                shape_idx: 0,
                headsign_idx: 0,
                short_name_idx: 0,
                block_idx: 0,
                jp_office_idx: 0,
                jp_pattern_idx: 0,
                direction_id: None,
                wheelchair_accessible: None,
                bikes_allowed: None,
                cars_allowed: None,
                safe_duration_factor: None,
                safe_duration_offset: None,
                line: 2,
            }
        })
    }

    #[test]
    fn spans_today_but_no_upcoming_produces_cal_021() {
        use crate::k5_derived::CalendarBitmap;
        // today=20260514, N=7 → pencere [20260514..20260521].
        // SVC covers today (20260510 ≤ today ≤ 20260601) but has no active day in the window.
        let dates: std::collections::HashSet<u32> = [20260510u32, 20260601].into_iter().collect();
        let derived = DerivedData {
            calendar_bitmap: CalendarBitmap {
                active_dates: [("SVC".to_string(), dates)].into_iter().collect(),
            },
            ..Default::default()
        };
        let records = crate::k2::EntityRecords {
            trips: vec![trip_svc("T1", "SVC")],
            trip_interns: take_ti(),
            ..Default::default()
        };
        let result = analyze(&records, &derived, &default_config(), 20260514);
        assert!(
            result.notices.iter().any(|n| n.rule_id == "CAL_021"),
            "bugünü kapsayan ama yakın günde seferi olmayan servis CAL_021 üretmeli"
        );
    }

    #[test]
    fn cal_021_requires_trips() {
        use crate::k5_derived::CalendarBitmap;
        let mut derived = DerivedData::default();
        let dates: std::collections::HashSet<u32> = [20260510u32, 20260601].into_iter().collect();
        derived.calendar_bitmap = CalendarBitmap {
            active_dates: [("SVC".to_string(), dates)].into_iter().collect(),
        };
        let records = crate::k2::EntityRecords::default(); // no trips
        let result = analyze(&records, &derived, &default_config(), 20260514);
        assert!(
            !result.notices.iter().any(|n| n.rule_id == "CAL_021"),
            "sefersiz servis CAL_021 üretmemeli"
        );
    }

    #[test]
    fn cal_021_not_for_active_or_expired() {
        use crate::k5_derived::CalendarBitmap;
        // (a) active on a nearby day → no CAL_021.
        let mut derived = DerivedData::default();
        let active: std::collections::HashSet<u32> = [20260514u32, 20260515].into_iter().collect();
        derived.calendar_bitmap = CalendarBitmap {
            active_dates: [("SVC".to_string(), active)].into_iter().collect(),
        };
        let records = crate::k2::EntityRecords {
            trips: vec![trip_svc("T1", "SVC")],
            trip_interns: take_ti(),
            ..Default::default()
        };
        let r1 = analyze(&records, &derived, &default_config(), 20260514);
        assert!(
            !r1.notices.iter().any(|n| n.rule_id == "CAL_021"),
            "yakın günde aktif servis CAL_021 üretmemeli"
        );

        // (b) entirely in the past → does not span today → no CAL_021 (CAL_013's domain).
        let mut derived2 = DerivedData::default();
        let past: std::collections::HashSet<u32> = [20251201u32, 20251231].into_iter().collect();
        derived2.calendar_bitmap = CalendarBitmap {
            active_dates: [("SVC".to_string(), past)].into_iter().collect(),
        };
        let r2 = analyze(&records, &derived2, &default_config(), 20260514);
        assert!(
            !r2.notices.iter().any(|n| n.rule_id == "CAL_021"),
            "tamamen geçmiş servis CAL_021 üretmemeli"
        );
    }

    // ── CAL_013: expired service from the bitmap ─────────────────────────────

    #[test]
    fn expired_service_in_bitmap_produces_cal_013() {
        use crate::k2::calendar_dates::CalendarDateIndex;
        use crate::k5_derived::CalendarBitmap;
        // All dates are in the past: max_date = 20251231 < today = 20260514.
        let mut dates = std::collections::HashSet::new();
        dates.insert(20251201u32);
        dates.insert(20251231u32);
        let derived = DerivedData {
            calendar_bitmap: CalendarBitmap {
                active_dates: [("EXPIRED_SVC".to_string(), dates)].into_iter().collect(),
            },
            ..Default::default()
        };
        // Without a record in exception_count, the CLD_007/CAL_013 loop does not run.
        let mut idx = CalendarDateIndex::default();
        idx.exception_count.insert("EXPIRED_SVC".into(), 1);
        let records = crate::k2::EntityRecords {
            calendar_dates: idx,
            ..Default::default()
        };
        let result = analyze(&records, &derived, &default_config(), 20260514);
        assert!(
            result.notices.iter().any(|n| n.rule_id == "CAL_013"),
            "CAL_013 olmalı"
        );
    }

    #[test]
    fn active_service_no_cal_013() {
        use crate::k2::calendar_dates::CalendarDateIndex;
        use crate::k5_derived::CalendarBitmap;
        // Gelecekte tarih var
        let mut dates = std::collections::HashSet::new();
        dates.insert(20260601u32);
        dates.insert(20260630u32);
        let derived = DerivedData {
            calendar_bitmap: CalendarBitmap {
                active_dates: [("ACTIVE_SVC".to_string(), dates)].into_iter().collect(),
            },
            ..Default::default()
        };
        let mut idx = CalendarDateIndex::default();
        idx.exception_count.insert("ACTIVE_SVC".into(), 1);
        let records = crate::k2::EntityRecords {
            calendar_dates: idx,
            ..Default::default()
        };
        let result = analyze(&records, &derived, &default_config(), 20260514);
        assert!(
            !result.notices.iter().any(|n| n.rule_id == "CAL_013"),
            "aktif servis CAL_013 üretmemeli"
        );
    }

    // ── WP-09c: GEO_006 ─────────────────────────────────────────────────────

    #[test]
    fn shape_jump_produces_geo_006() {
        use crate::k5_derived::{ShapeGeometry, ShapeSegments};
        let derived = DerivedData {
            shape_geometry: ShapeGeometry {
                shapes: [(
                    "S1".to_string(),
                    ShapeSegments {
                        segment_distances_km: vec![0.5, 15.0, 0.3], // 15 km > 10 km threshold
                        total_length_km: 15.8,
                        bbox: (40.0, 42.0, 28.0, 30.0),
                    },
                )]
                .into_iter()
                .collect(),
            },
            ..Default::default()
        };
        let result = analyze(
            &crate::k2::EntityRecords::default(),
            &derived,
            &default_config(),
            20260514,
        );
        assert!(result.notices.iter().any(|n| n.rule_id == "GEO_006"));
    }

    #[test]
    fn normal_shape_no_geo_006() {
        use crate::k5_derived::{ShapeGeometry, ShapeSegments};
        let derived = DerivedData {
            shape_geometry: ShapeGeometry {
                shapes: [(
                    "S1".to_string(),
                    ShapeSegments {
                        segment_distances_km: vec![0.5, 0.8, 0.3],
                        total_length_km: 1.6,
                        bbox: (40.0, 40.1, 28.0, 28.1),
                    },
                )]
                .into_iter()
                .collect(),
            },
            ..Default::default()
        };
        let result = analyze(
            &crate::k2::EntityRecords::default(),
            &derived,
            &default_config(),
            20260514,
        );
        assert!(!result.notices.iter().any(|n| n.rule_id == "GEO_006"));
    }

    // ── WP-09c: STP_017 ─────────────────────────────────────────────────────

    #[test]
    fn very_close_stops_produce_stp_017() {
        use crate::k5_derived::SpatialIndex;
        // Two stops 4 meters apart (< 5m threshold): 0.000036° difference ≈ 4m.
        let records = crate::k2::EntityRecords {
            stops: vec![
                stop("A", 41.0, 29.0),
                stop("B", 41.000036, 29.0), // ~4m kuzey
            ],
            ..Default::default()
        };
        let derived = DerivedData {
            spatial_index: SpatialIndex {
                grid: [((82i32, 58i32), vec![0usize, 1usize])]
                    .into_iter()
                    .collect(),
                cell_deg: 0.5,
            },
            ..Default::default()
        };
        let result = analyze(&records, &derived, &default_config(), 20260514);
        assert!(
            result.notices.iter().any(|n| n.rule_id == "STP_017"),
            "STP_017 olmalı"
        );
    }

    #[test]
    fn sibling_platforms_same_parent_no_stp_016_017() {
        use crate::k5_derived::SpatialIndex;
        // Two platforms of the same station (same parent_station) can be 4 m apart, or even at
        // the same coordinates — station-hierarchy modeling, must not produce an FP (VBB lesson).
        let mut pa = stop("PA", 41.0, 29.0);
        pa.row
            .insert("parent_station".to_string(), "ST".to_string());
        let mut pb = stop("PB", 41.0, 29.0); // same coordinates
        pb.row
            .insert("parent_station".to_string(), "ST".to_string());
        let records = crate::k2::EntityRecords {
            stops: vec![pa, pb],
            ..Default::default()
        };
        let derived = DerivedData {
            spatial_index: SpatialIndex {
                grid: [((82i32, 58i32), vec![0usize, 1usize])]
                    .into_iter()
                    .collect(),
                cell_deg: 0.5,
            },
            ..Default::default()
        };
        let result = analyze(&records, &derived, &default_config(), 20260514);
        assert!(
            !result
                .notices
                .iter()
                .any(|n| n.rule_id == "STP_016" || n.rule_id == "STP_017"),
            "aynı-parent kardeş peronlar STP_016/017 üretmemeli"
        );
    }

    #[test]
    fn entrance_at_stop_coordinate_no_stp_016() {
        use crate::k5_derived::SpatialIndex;
        // An entrance node (location_type=2) at the same coordinates as a street stop —
        // a modeling choice; "merge the stops" would be wrong (VBB: 3,093 pairs).
        let mut ent = stop("E1", 41.0, 29.0);
        ent.location_type = Some(2);
        let records = crate::k2::EntityRecords {
            stops: vec![ent, stop("B", 41.0, 29.0)],
            ..Default::default()
        };
        let derived = DerivedData {
            spatial_index: SpatialIndex {
                grid: [((82i32, 58i32), vec![0usize, 1usize])]
                    .into_iter()
                    .collect(),
                cell_deg: 0.5,
            },
            ..Default::default()
        };
        let result = analyze(&records, &derived, &default_config(), 20260514);
        assert!(
            !result
                .notices
                .iter()
                .any(|n| n.rule_id == "STP_016" || n.rule_id == "STP_017"),
            "giriş↔durak aynı-koordinat çifti STP_016/017 üretmemeli"
        );
    }

    #[test]
    fn geo_012_sibling_platforms_not_counted_as_cluster() {
        use crate::k5_derived::SpatialIndex;
        // Three sibling platforms of one station (same parent) + the station itself at the same
        // point — hierarchy modeling, not a cluster (consistent with STP_016/017).
        let mut st = stop("ST", 41.0, 29.0);
        st.location_type = Some(1);
        let mk = |id: &str| {
            let mut s = stop(id, 41.0, 29.0);
            s.row.insert("parent_station".to_string(), "ST".to_string());
            s
        };
        let records = crate::k2::EntityRecords {
            stops: vec![mk("PA"), mk("PB"), mk("PC"), st],
            ..Default::default()
        };
        let derived = DerivedData {
            spatial_index: SpatialIndex {
                grid: [((82i32, 58i32), vec![0usize, 1, 2, 3])]
                    .into_iter()
                    .collect(),
                cell_deg: 0.5,
            },
            ..Default::default()
        };
        let result = analyze(&records, &derived, &default_config(), 20260514);
        assert!(
            !result.notices.iter().any(|n| n.rule_id == "GEO_012"),
            "aynı-parent kardeş peronlar GEO_012 kümesi sayılmamalı"
        );
    }

    #[test]
    fn geo_012_independent_stops_still_cluster() {
        use crate::k5_derived::SpatialIndex;
        // Three boardable stops without a parent at the same point → real cluster, GEO_012 MUST fire.
        let records = crate::k2::EntityRecords {
            stops: vec![
                stop("A", 41.0, 29.0),
                stop("B", 41.0, 29.0),
                stop("C", 41.0, 29.0),
            ],
            ..Default::default()
        };
        let derived = DerivedData {
            spatial_index: SpatialIndex {
                grid: [((82i32, 58i32), vec![0usize, 1, 2])].into_iter().collect(),
                cell_deg: 0.5,
            },
            ..Default::default()
        };
        let result = analyze(&records, &derived, &default_config(), 20260514);
        assert!(
            result.notices.iter().any(|n| n.rule_id == "GEO_012"),
            "bağımsız 3 yakın durak GEO_012 üretmeli"
        );
    }

    #[test]
    fn geo_012_finds_cluster_when_first_cell_stop_is_a_station() {
        use crate::k5_derived::SpatialIndex;
        // The cell's FIRST stop is a station (lt=1) — the old code saw the guard and used `continue`,
        // skipping the whole cell and missing the real cluster behind it.
        // Now skip an unsuitable anchor and use the next boardable stop as the anchor.
        let mut station = stop("ST", 41.0, 29.0);
        station.location_type = Some(1);
        let records = crate::k2::EntityRecords {
            stops: vec![
                station,
                stop("A", 41.0, 29.0),
                stop("B", 41.0, 29.0),
                stop("C", 41.0, 29.0),
            ],
            ..Default::default()
        };
        let derived = DerivedData {
            spatial_index: SpatialIndex {
                grid: [((82i32, 58i32), vec![0usize, 1, 2, 3])]
                    .into_iter()
                    .collect(),
                cell_deg: 0.5,
            },
            ..Default::default()
        };
        let result = analyze(&records, &derived, &default_config(), 20260514);
        let n = result
            .notices
            .iter()
            .find(|n| n.rule_id == "GEO_012")
            .expect("ilk durak istasyon olsa da arkadaki gerçek küme GEO_012 üretmeli");
        assert_eq!(
            n.entity_id.as_deref(),
            Some("A"),
            "anchor ilk BİNİLEBİLİR durak olmalı"
        );
    }

    #[test]
    fn duplicate_stations_same_coordinate_produce_stp_016() {
        use crate::k5_derived::SpatialIndex;
        // Two SEPARATE stations (location_type=1) at exactly the same coordinates — possible
        // duplicate-station case; the skeleton exemption MUST NOT swallow this pair (review K1b exception).
        let mut s1 = stop("ST1", 41.0, 29.0);
        s1.location_type = Some(1);
        let mut s2 = stop("ST2", 41.0, 29.0);
        s2.location_type = Some(1);
        let records = crate::k2::EntityRecords {
            stops: vec![s1, s2],
            ..Default::default()
        };
        let derived = DerivedData {
            spatial_index: SpatialIndex {
                grid: [((82i32, 58i32), vec![0usize, 1usize])]
                    .into_iter()
                    .collect(),
                cell_deg: 0.5,
            },
            ..Default::default()
        };
        let result = analyze(&records, &derived, &default_config(), 20260514);
        assert!(
            result.notices.iter().any(|n| n.rule_id == "STP_016"),
            "istasyon↔istasyon aynı-koordinat çifti STP_016 üretmeli"
        );
    }

    // ── WP-09c: OPR_006 / OPR_007 ───────────────────────────────────────────

    #[test]
    fn single_stop_trip_produces_opr_006() {
        let records = records_with(
            vec![stop("A", 41.0, 29.0)],
            vec![route("R1", 3)],
            vec![trip("T1", "R1")],
            vec![stoptime("T1", 1, "A", (8, 0, 0), (8, 0, 0), 2)],
        );
        let result = analyze(&records, &empty_derived(), &default_config(), 20260514);
        assert!(result.notices.iter().any(|n| n.rule_id == "OPR_006"));
    }

    #[test]
    fn repeated_stop_in_trip_produces_opr_007() {
        // A→B→C→B: non-ring (first=A, last=B), B repeats at an intermediate stop → OPR_007 must fire.
        let records = records_with(
            vec![
                stop("A", 41.0, 29.0),
                stop("B", 41.1, 29.1),
                stop("C", 41.2, 29.2),
            ],
            vec![route("R1", 3)],
            vec![trip("T1", "R1")],
            vec![
                stoptime("T1", 1, "A", (8, 0, 0), (8, 0, 0), 2),
                stoptime("T1", 2, "B", (8, 10, 0), (8, 10, 0), 3),
                stoptime("T1", 3, "C", (8, 20, 0), (8, 20, 0), 4),
                stoptime("T1", 4, "B", (8, 30, 0), (8, 30, 0), 5), // B repeats, not a ring
            ],
        );
        let result = analyze(&records, &empty_derived(), &default_config(), 20260514);
        assert!(result.notices.iter().any(|n| n.rule_id == "OPR_007"));
    }

    #[test]
    fn ring_route_terminal_repeat_no_opr_007() {
        // A→B→A: first==last, ring route → terminal repetition should be suppressed.
        let records = records_with(
            vec![stop("A", 41.0, 29.0), stop("B", 41.1, 29.1)],
            vec![route("R1", 3)],
            vec![trip("T1", "R1")],
            vec![
                stoptime("T1", 1, "A", (8, 0, 0), (8, 0, 0), 2),
                stoptime("T1", 2, "B", (8, 10, 0), (8, 10, 0), 3),
                stoptime("T1", 3, "A", (8, 20, 0), (8, 20, 0), 4),
            ],
        );
        let result = analyze(&records, &empty_derived(), &default_config(), 20260514);
        assert!(
            !result.notices.iter().any(|n| n.rule_id == "OPR_007"),
            "Ring hatta terminal tekrarı OPR_007 üretmemeli"
        );
    }

    // ── WP-09d: STM_027 ─────────────────────────────────────────────────────

    fn stoptime_with_dist(
        trip_id: &str,
        seq: u32,
        stop_id: &str,
        arr: (u32, u32, u32),
        dep: (u32, u32, u32),
        dist: f64,
        line: u64,
    ) -> StopTimeRecord {
        let mut st = stoptime(trip_id, seq, stop_id, arr, dep, line);
        st.shape_dist_traveled = Some(dist);
        st
    }

    #[test]
    fn monotone_dist_no_stm_027() {
        let records = records_with(
            vec![
                stop("A", 41.0, 29.0),
                stop("B", 41.1, 29.1),
                stop("C", 41.2, 29.2),
            ],
            vec![route("R1", 3)],
            vec![trip("T1", "R1")],
            vec![
                stoptime_with_dist("T1", 1, "A", (8, 0, 0), (8, 0, 0), 0.0, 2),
                stoptime_with_dist("T1", 2, "B", (8, 10, 0), (8, 10, 0), 1.0, 3),
                stoptime_with_dist("T1", 3, "C", (8, 20, 0), (8, 20, 0), 2.0, 4),
            ],
        );
        let result = analyze(&records, &empty_derived(), &default_config(), 20260514);
        assert!(!result.notices.iter().any(|n| n.rule_id == "STM_027"));
    }

    #[test]
    fn untimed_intermediate_stops_do_not_produce_retired_stm_013() {
        let untimed = |trip_id: &str, seq: u32, stop_id: &str, line: u64| StopTimeRecord {
            trip_id: trip_id.into(),
            stop_id: stop_id.into(),
            stop_sequence: Some(seq),
            line,
            ..Default::default()
        };
        let records = records_with(
            vec![
                stop("A", 41.0, 29.0),
                stop("B", 41.1, 29.1),
                stop("C", 41.2, 29.2),
                stop("D", 41.3, 29.3),
            ],
            vec![route("R1", 3)],
            vec![trip("T1", "R1"), trip("T2", "R1")],
            vec![
                // TIME / blank / blank / TIME
                stoptime("T1", 1, "A", (8, 0, 0), (8, 0, 0), 2),
                untimed("T1", 2, "B", 3),
                untimed("T1", 3, "C", 4),
                stoptime("T1", 4, "D", (8, 20, 0), (8, 20, 0), 5),
                // TIME / TIME / blank / TIME
                stoptime("T2", 1, "A", (9, 0, 0), (9, 0, 0), 6),
                stoptime("T2", 2, "B", (9, 5, 0), (9, 5, 0), 7),
                untimed("T2", 3, "C", 8),
                stoptime("T2", 4, "D", (9, 20, 0), (9, 20, 0), 9),
            ],
        );
        let result = analyze(&records, &empty_derived(), &default_config(), 20260514);
        assert!(
            !result.notices.iter().any(|n| n.rule_id == "STM_013"),
            "optional intermediate timing gaps must not revive retired STM_013: {:?}",
            result.notices
        );
    }

    // ── WP-09d: TRP_011 / TRP_013 / RTS_016 ────────────────────────────────

    #[test]
    fn trip_without_headsign_produces_trp_011() {
        let records = records_with(
            vec![stop("A", 41.0, 29.0), stop("B", 41.1, 29.1)],
            vec![route("R1", 3)],
            vec![trip("T1", "R1")], // trip() headsign=None, short=None
            vec![
                stoptime("T1", 1, "A", (8, 0, 0), (8, 0, 0), 2),
                stoptime("T1", 2, "B", (8, 10, 0), (8, 10, 0), 3),
            ],
        );
        let result = analyze(&records, &empty_derived(), &default_config(), 20260514);
        assert!(result.notices.iter().any(|n| n.rule_id == "TRP_011"));
    }

    #[test]
    fn single_trip_route_produces_trp_013() {
        let records = records_with(
            vec![stop("A", 41.0, 29.0), stop("B", 41.1, 29.1)],
            vec![route("R1", 3)],
            vec![trip("T1", "R1")], // one trip
            vec![
                stoptime("T1", 1, "A", (8, 0, 0), (8, 0, 0), 2),
                stoptime("T1", 2, "B", (8, 10, 0), (8, 10, 0), 3),
            ],
        );
        let result = analyze(&records, &empty_derived(), &default_config(), 20260514);
        assert!(result.notices.iter().any(|n| n.rule_id == "TRP_013"));
    }

    #[test]
    fn route_without_active_service_produces_rts_016() {
        // Trip exists, but its service_id is absent from calendar_bitmap.
        let records = records_with(
            vec![stop("A", 41.0, 29.0), stop("B", 41.1, 29.1)],
            vec![route("R1", 3)],
            vec![trip("T1", "R1")],
            vec![
                stoptime("T1", 1, "A", (8, 0, 0), (8, 0, 0), 2),
                stoptime("T1", 2, "B", (8, 10, 0), (8, 10, 0), 3),
            ],
        );
        // Empty calendar_bitmap → no active days.
        let derived = DerivedData::default();
        let result = analyze(&records, &derived, &default_config(), 20260514);
        assert!(result.notices.iter().any(|n| n.rule_id == "RTS_016"));
    }

    // ── WP-09d: DQ_005 ──────────────────────────────────────────────────────

    #[test]
    fn no_active_trips_produces_dq_005() {
        use crate::k5_derived::CalendarBitmap;
        let records = records_with(
            vec![stop("A", 41.0, 29.0)],
            vec![route("R1", 3)],
            vec![trip("T1", "R1")],
            vec![stoptime("T1", 1, "A", (8, 0, 0), (8, 0, 0), 2)],
        );
        // All calendar_bitmap dates are in the past.
        let derived = DerivedData {
            calendar_bitmap: CalendarBitmap {
                active_dates: [("SVC".to_string(), [20250101u32].into_iter().collect())]
                    .into_iter()
                    .collect(),
            },
            ..Default::default()
        };
        let result = analyze(&records, &derived, &default_config(), 20260514);
        assert!(result.notices.iter().any(|n| n.rule_id == "DQ_005"));
    }

    #[test]
    fn unavailable_or_missing_calendar_does_not_produce_dq_005() {
        let records = records_with(
            vec![stop("A", 41.0, 29.0)],
            vec![route("R1", 3)],
            vec![trip("T1", "R1")],
            vec![stoptime("T1", 1, "A", (8, 0, 0), (8, 0, 0), 2)],
        );

        let present = std::collections::HashSet::from(["trips.txt".to_string()]);
        let availability = FileAvailability::from_k1(&present, &[]);
        let result = analyze_with_files(
            &records,
            &DerivedData::default(),
            &default_config(),
            20260514,
            &availability,
        );
        assert!(!result.notices.iter().any(|n| n.rule_id == "DQ_005"));

        let present =
            std::collections::HashSet::from(["trips.txt".to_string(), "calendar.txt".to_string()]);
        let unavailable = vec!["calendar.txt".to_string()];
        let availability = FileAvailability::from_k1(&present, &unavailable);
        let result = analyze_with_files(
            &records,
            &DerivedData::default(),
            &default_config(),
            20260514,
            &availability,
        );
        assert!(!result.notices.iter().any(|n| n.rule_id == "DQ_005"));
    }

    #[test]
    fn geo_016_survives_a_broken_routes_file() {
        // #160: `K6::geo_analytics` depended on all four files and read `routes.txt` in NO block.
        // A broken routes.txt also killed the Null Island check, which needs only `stops.txt` —
        // MD reported `point_near_origin` for `tdg-83820` while we stayed silent. It was a
        // DEPENDENCY defect, not an input defect.
        let records = records_with(
            vec![stop("A", 0.0, 0.0)], // Null Island
            vec![route("R1", 3)],
            vec![trip("T1", "R1")],
            vec![stoptime("T1", 1, "A", (8, 0, 0), (8, 0, 0), 2)],
        );

        let present = std::collections::HashSet::from([
            "stops.txt".to_string(),
            "routes.txt".to_string(),
            "trips.txt".to_string(),
            "stop_times.txt".to_string(),
        ]);
        let unavailable = vec!["routes.txt".to_string()];
        let availability = FileAvailability::from_k1(&present, &unavailable);
        let result = analyze_with_files(
            &records,
            &DerivedData::default(),
            &default_config(),
            20260514,
            &availability,
        );
        assert!(
            result.notices.iter().any(|n| n.rule_id == "GEO_016"),
            "routes.txt okunamıyor diye stops.txt'e bakan GEO_016 susmamalı: {:?}",
            result
                .notices
                .iter()
                .map(|n| &n.rule_id)
                .collect::<Vec<_>>()
        );
    }

    #[test]
    fn geo_016_fires_with_stops_alone() {
        // Gate floor: must work with a single file.
        let records = records_with(vec![stop("A", 0.0, 0.0)], vec![], vec![], vec![]);
        let present = std::collections::HashSet::from(["stops.txt".to_string()]);
        let availability = FileAvailability::from_k1(&present, &[]);
        let result = analyze_with_files(
            &records,
            &DerivedData::default(),
            &default_config(),
            20260514,
            &availability,
        );
        assert!(result.notices.iter().any(|n| n.rule_id == "GEO_016"));
    }

    #[test]
    fn geo_016_stays_silent_without_stops() {
        let records = records_with(vec![], vec![], vec![], vec![]);
        let present = std::collections::HashSet::from(["routes.txt".to_string()]);
        let availability = FileAvailability::from_k1(&present, &[]);
        let result = analyze_with_files(
            &records,
            &DerivedData::default(),
            &default_config(),
            20260514,
            &availability,
        );
        assert!(!result.notices.iter().any(|n| n.rule_id == "GEO_016"));
    }

    // ── WP-09e: Entegrasyon smoke testleri ──────────────────────────────────

    #[test]
    fn config_change_changes_stm_014_threshold() {
        // Same feed, different config → different result (config change affects only K6).
        let records = records_with(
            vec![stop("A", 41.0, 29.0), stop("B", 41.05, 29.05)],
            vec![route("R1", 3)],
            vec![trip("T1", "R1")],
            vec![
                stoptime("T1", 1, "A", (8, 0, 0), (8, 0, 0), 2),
                stoptime("T1", 2, "B", (8, 5, 0), (8, 5, 0), 3),
            ],
        );
        // ~8 km/h for this trip.
        let result_default = analyze(&records, &empty_derived(), &default_config(), 20260514);
        assert!(
            !result_default
                .notices
                .iter()
                .any(|n| n.rule_id == "STM_014"),
            "Default config'te STM_014 olmamalı"
        );

        // Very low speed threshold → STM_014 must fire.
        let strict_cfg = ValidatorConfig {
            max_speed_bus_kmh: 1.0,
            ..default_config()
        };
        let result_strict = analyze(&records, &empty_derived(), &strict_cfg, 20260514);
        assert!(
            result_strict.notices.iter().any(|n| n.rule_id == "STM_014"),
            "Düşük eşikli config'te STM_014 olmalı"
        );
    }

    #[test]
    fn full_pipeline_smoke_no_panic() {
        // All functions must handle empty records without panicking.
        let result = analyze(
            &crate::k2::EntityRecords::default(),
            &DerivedData::default(),
            &default_config(),
            20260514,
        );
        // No notice should be emitted for an empty feed.
        assert!(result.notices.is_empty());
    }

    // ── OPR_011: service_id has no active days ───────────────────────────────

    #[test]
    fn service_with_no_active_dates_produces_opr_011() {
        let records = records_with(
            vec![stop("A", 41.0, 29.0), stop("B", 41.1, 29.1)],
            vec![route("R1", 3)],
            vec![trip("T1", "R1")], // service_id = "SVC"
            vec![
                stoptime("T1", 1, "A", (8, 0, 0), (8, 0, 0), 2),
                stoptime("T1", 2, "B", (8, 10, 0), (8, 10, 0), 3),
            ],
        );
        // SVC is absent from calendar_bitmap → no active days.
        let derived = DerivedData::default();
        let result = analyze(&records, &derived, &default_config(), 20260514);
        assert!(
            result.notices.iter().any(|n| n.rule_id == "OPR_011"),
            "OPR_011 olmalı"
        );
    }

    #[test]
    fn service_with_active_dates_no_opr_011() {
        use crate::k5_derived::CalendarBitmap;
        let records = records_with(
            vec![stop("A", 41.0, 29.0), stop("B", 41.1, 29.1)],
            vec![route("R1", 3)],
            vec![trip("T1", "R1")],
            vec![
                stoptime("T1", 1, "A", (8, 0, 0), (8, 0, 0), 2),
                stoptime("T1", 2, "B", (8, 10, 0), (8, 10, 0), 3),
            ],
        );
        let derived = DerivedData {
            calendar_bitmap: CalendarBitmap {
                active_dates: [("SVC".to_string(), [20261001u32].into_iter().collect())]
                    .into_iter()
                    .collect(),
            },
            ..Default::default()
        };
        let result = analyze(&records, &derived, &default_config(), 20260514);
        assert!(!result.notices.iter().any(|n| n.rule_id == "OPR_011"));
    }

    // ── OPR_016: no active service anywhere in the feed ──────────────────────

    #[test]
    fn no_active_services_at_all_produces_opr_016() {
        let records = records_with(
            vec![stop("A", 41.0, 29.0)],
            vec![route("R1", 3)],
            vec![trip("T1", "R1")],
            vec![stoptime("T1", 1, "A", (8, 0, 0), (8, 0, 0), 2)],
        );
        // calendar_bitmap is completely empty.
        let result = analyze(
            &records,
            &DerivedData::default(),
            &default_config(),
            20260514,
        );
        assert!(
            result.notices.iter().any(|n| n.rule_id == "OPR_016"),
            "OPR_016 olmalı"
        );
    }

    #[test]
    fn some_active_service_no_opr_016() {
        use crate::k5_derived::CalendarBitmap;
        let records = records_with(
            vec![stop("A", 41.0, 29.0)],
            vec![route("R1", 3)],
            vec![trip("T1", "R1")],
            vec![stoptime("T1", 1, "A", (8, 0, 0), (8, 0, 0), 2)],
        );
        let derived = DerivedData {
            calendar_bitmap: CalendarBitmap {
                active_dates: [("SVC".to_string(), [20261001u32].into_iter().collect())]
                    .into_iter()
                    .collect(),
            },
            ..Default::default()
        };
        let result = analyze(&records, &derived, &default_config(), 20260514);
        assert!(!result.notices.iter().any(|n| n.rule_id == "OPR_016"));
    }

    // ── DQ_009: no stop_times in the feed ─────────────────────────────────────

    #[test]
    fn trips_with_no_stop_times_produces_dq_009() {
        let records = records_with(
            vec![stop("A", 41.0, 29.0)],
            vec![route("R1", 3)],
            vec![trip("T1", "R1")], // trip var
            vec![],                 // no stop_times
        );
        let result = analyze(
            &records,
            &DerivedData::default(),
            &default_config(),
            20260514,
        );
        assert!(
            result.notices.iter().any(|n| n.rule_id == "DQ_009"),
            "DQ_009 olmalı"
        );
    }

    #[test]
    fn unavailable_stop_times_does_not_produce_dq_009() {
        let records = records_with(
            vec![stop("A", 41.0, 29.0)],
            vec![route("R1", 3)],
            vec![trip("T1", "R1")],
            vec![],
        );
        let present = std::collections::HashSet::from([
            "trips.txt".to_string(),
            "stop_times.txt".to_string(),
        ]);
        let unavailable = vec!["stop_times.txt".to_string()];
        let availability = FileAvailability::from_k1(&present, &unavailable);
        let result = analyze_with_files(
            &records,
            &DerivedData::default(),
            &default_config(),
            20260514,
            &availability,
        );
        assert!(!result.notices.iter().any(|n| n.rule_id == "DQ_009"));
    }

    #[test]
    fn no_trips_no_dq_009() {
        // Without trips, DQ_009 must not fire (condition: trips exist + no stop_times).
        let records = records_with(vec![], vec![], vec![], vec![]);
        let result = analyze(
            &records,
            &DerivedData::default(),
            &default_config(),
            20260514,
        );
        assert!(!result.notices.iter().any(|n| n.rule_id == "DQ_009"));
    }

    // ── STM_017: shape_dist_traveled missing on a trip with a shape ─────────

    #[test]
    fn trip_with_shape_but_no_dist_produces_stm_017() {
        let mut shape_ti = ShapeInternTable::new();
        use crate::k2::shapes::ShapePointRecord;
        let mut records = records_with(
            vec![stop("A", 41.0, 29.0), stop("B", 41.1, 29.1)],
            vec![route("R1", 3)],
            vec![trip_sh("T1", "R1", "S1")],
            vec![
                stoptime("T1", 1, "A", (8, 0, 0), (8, 0, 0), 2),
                stoptime("T1", 2, "B", (8, 10, 0), (8, 10, 0), 3),
            ],
        );
        records.shapes = vec![
            ShapePointRecord::new(
                shape_ti.intern("S1"),
                Some(41.0),
                Some(29.0),
                Some(1),
                None,
                2,
            ),
            ShapePointRecord::new(
                shape_ti.intern("S1"),
                Some(41.1),
                Some(29.1),
                Some(2),
                None,
                3,
            ),
        ];
        records.shape_interns = shape_ti.clone();
        let result = analyze(&records, &empty_derived(), &default_config(), 20260514);
        let notice = result
            .notices
            .iter()
            .find(|n| n.rule_id == "STM_017")
            .expect("STM_017 olmalı");
        let details = notice.details.as_ref().expect("STM_017 details olmalı");
        assert_eq!(details.get("stop_count").map(String::as_str), Some("2"));
        assert_eq!(
            details.get("affected_stop_count").map(String::as_str),
            Some("2")
        );
        assert_eq!(
            details.get("missing_0_stop_id").map(String::as_str),
            Some("A")
        );
        assert_eq!(
            details.get("missing_1_stop_id").map(String::as_str),
            Some("B")
        );
    }

    #[test]
    fn stm_017_feed_wide_missing_aggregates_to_single_feed_notice() {
        let mut shape_ti = ShapeInternTable::new();
        use crate::k2::shapes::ShapePointRecord;
        // Both trips have a shape but lack sdt; no sdt exists anywhere in the feed →
        // expect one feed-level summary instead of 2 trip-level notices.
        let mut records = records_with(
            vec![stop("A", 41.0, 29.0), stop("B", 41.1, 29.1)],
            vec![route("R1", 3)],
            vec![trip_sh("T1", "R1", "S1"), trip_sh("T2", "R1", "S1")],
            vec![
                stoptime("T1", 1, "A", (8, 0, 0), (8, 0, 0), 2),
                stoptime("T1", 2, "B", (8, 10, 0), (8, 10, 0), 3),
                stoptime("T2", 1, "A", (9, 0, 0), (9, 0, 0), 4),
                stoptime("T2", 2, "B", (9, 10, 0), (9, 10, 0), 5),
            ],
        );
        records.shapes = vec![
            ShapePointRecord::new(
                shape_ti.intern("S1"),
                Some(41.0),
                Some(29.0),
                Some(1),
                None,
                2,
            ),
            ShapePointRecord::new(
                shape_ti.intern("S1"),
                Some(41.1),
                Some(29.1),
                Some(2),
                None,
                3,
            ),
        ];
        records.shape_interns = shape_ti.clone();
        let result = analyze(&records, &empty_derived(), &default_config(), 20260514);
        let stm017: Vec<_> = result
            .notices
            .iter()
            .filter(|n| n.rule_id == "STM_017")
            .collect();
        assert_eq!(
            stm017.len(),
            1,
            "feed-geneli eksiklikte tek STM_017 beklenir, alınan: {}",
            stm017.len()
        );
        assert_eq!(
            stm017[0].entity_type,
            gtfs_core::EntityType::Feed,
            "feed-seviyesi olmalı"
        );
        assert_eq!(
            stm017[0].observed_value.as_deref(),
            Some("2"),
            "etkilenen sefer sayısı 2 olmalı"
        );
        let details = stm017[0].details.as_ref().expect("özet details olmalı");
        assert_eq!(
            details.get("affected_trip_count").map(String::as_str),
            Some("2")
        );
        assert_eq!(
            details.get("affected_shape_count").map(String::as_str),
            Some("1")
        );
        assert_eq!(
            details.get("example_0_trip").map(String::as_str),
            Some("T1")
        );
        assert_eq!(
            details.get("example_1_trip").map(String::as_str),
            Some("T2")
        );
        assert_eq!(
            details
                .get("example_0_affected_stop_count")
                .map(String::as_str),
            Some("2")
        );
    }

    #[test]
    fn stm_017_partial_sdt_keeps_trip_level_on_production_index_path() {
        let mut shape_ti = ShapeInternTable::new();
        use crate::k2::shapes::ShapePointRecord;
        // T1 carries sdt, T2/T3 do not → partially populated, expect 2 trip-level notices.
        // In production, stop_times exists only in the K2 index; the `records.stop_times` Vec is empty.
        // Before this path was tested, mode selection inspected the Vec and always fell into feed-summary mode.
        let mut records = records_with(
            vec![stop("A", 41.0, 29.0), stop("B", 41.1, 29.1)],
            vec![route("R1", 3)],
            vec![
                trip_sh("T1", "R1", "S1"),
                trip_sh("T2", "R1", "S1"),
                trip_sh("T3", "R1", "S1"),
            ],
            vec![
                stoptime_with_dist("T1", 1, "A", (8, 0, 0), (8, 0, 0), 0.0, 2),
                stoptime_with_dist("T1", 2, "B", (8, 10, 0), (8, 10, 0), 14.0, 3),
                stoptime("T2", 1, "A", (9, 0, 0), (9, 0, 0), 4),
                stoptime("T2", 2, "B", (9, 10, 0), (9, 10, 0), 5),
                stoptime("T3", 1, "A", (10, 0, 0), (10, 0, 0), 6),
                stoptime("T3", 2, "B", (10, 10, 0), (10, 10, 0), 7),
            ],
        );
        records.stop_times.clear();
        records.shapes = vec![
            ShapePointRecord::new(
                shape_ti.intern("S1"),
                Some(41.0),
                Some(29.0),
                Some(1),
                None,
                2,
            ),
            ShapePointRecord::new(
                shape_ti.intern("S1"),
                Some(41.1),
                Some(29.1),
                Some(2),
                None,
                3,
            ),
        ];
        records.shape_interns = shape_ti.clone();
        let result = analyze(&records, &empty_derived(), &default_config(), 20260514);
        let mut stm017: Vec<_> = result
            .notices
            .iter()
            .filter(|n| n.rule_id == "STM_017")
            .map(|n| (n.entity_type, n.entity_id.clone(), n.line))
            .collect();
        stm017.sort_by(|a, b| a.1.cmp(&b.1));
        assert_eq!(
            stm017,
            vec![
                (gtfs_core::EntityType::Trip, Some("T2".to_string()), Some(4)),
                (gtfs_core::EntityType::Trip, Some("T3".to_string()), Some(6)),
            ],
            "kısmen dolu sdt'de trip-başına STM_017 beklenir"
        );
    }

    #[test]
    fn trip_referencing_undefined_shape_no_stm_017() {
        // shape_id is populated but not defined in shapes.txt (the file is absent). There
        // is no shape against which to validate stop positions; TRP_004 (undefined
        // shape_id) and ARC_020 (missing shapes.txt) report the actual issues. STM_017
        // must not duplicate this variant (2026-10-30 corpus: 8 feeds, all missing
        // shapes.txt).
        let records = records_with(
            vec![stop("A", 41.0, 29.0), stop("B", 41.1, 29.1)],
            vec![route("R1", 3)],
            vec![trip_sh("T1", "R1", "S1"), trip_sh("T2", "R1", "S2")],
            vec![
                stoptime("T1", 1, "A", (8, 0, 0), (8, 0, 0), 2),
                stoptime("T1", 2, "B", (8, 10, 0), (8, 10, 0), 3),
                stoptime("T2", 1, "A", (9, 0, 0), (9, 0, 0), 4),
                stoptime("T2", 2, "B", (9, 10, 0), (9, 10, 0), 5),
            ],
        );
        let result = analyze(&records, &empty_derived(), &default_config(), 20260514);
        assert!(!result.notices.iter().any(|n| n.rule_id == "STM_017"));
    }

    #[test]
    fn trip_without_shape_no_stm_017() {
        let records = records_with(
            vec![stop("A", 41.0, 29.0), stop("B", 41.1, 29.1)],
            vec![route("R1", 3)],
            vec![trip("T1", "R1")], // no shape_id
            vec![
                stoptime("T1", 1, "A", (8, 0, 0), (8, 0, 0), 2),
                stoptime("T1", 2, "B", (8, 10, 0), (8, 10, 0), 3),
            ],
        );
        let result = analyze(&records, &empty_derived(), &default_config(), 20260514);
        assert!(!result.notices.iter().any(|n| n.rule_id == "STM_017"));
    }

    // ── GEO_007: very large shape jump (severe = 3× threshold) ──────────────

    #[test]
    fn severe_shape_jump_produces_geo_007() {
        use crate::k5_derived::{ShapeGeometry, ShapeSegments};
        // max_shape_jump_km default = 10.0 → severe = 30.0 → 50 km segment tetikler
        let derived = DerivedData {
            shape_geometry: ShapeGeometry {
                shapes: [(
                    "S1".to_string(),
                    ShapeSegments {
                        segment_distances_km: vec![50.0],
                        total_length_km: 50.0,
                        bbox: (41.0, 41.5, 29.0, 29.5),
                    },
                )]
                .into_iter()
                .collect(),
            },
            ..Default::default()
        };
        let records = crate::k2::EntityRecords::default();
        let result = analyze(&records, &derived, &default_config(), 20260514);
        assert!(
            result.notices.iter().any(|n| n.rule_id == "GEO_007"),
            "GEO_007 olmalı"
        );
    }

    /// A segment exceeding 3× the threshold must produce GEO_007 ONLY.
    ///
    /// Previously the two conditions were not mutually exclusive: the same segment
    /// produced both GEO_006 and GEO_007, so users saw the same jump twice and it was
    /// counted twice in the R5 score. SHP_011 also reported it a third time with the
    /// same threshold (now removed).
    #[test]
    fn severe_shape_jump_produces_geo_007_only_not_geo_006() {
        use crate::k5_derived::{ShapeGeometry, ShapeSegments};
        // default max_shape_jump_km = 10.0 → severe = 30.0; 50 km exceeds both
        let derived = DerivedData {
            shape_geometry: ShapeGeometry {
                shapes: [(
                    "S1".to_string(),
                    ShapeSegments {
                        segment_distances_km: vec![50.0],
                        total_length_km: 50.0,
                        bbox: (41.0, 41.5, 29.0, 29.5),
                    },
                )]
                .into_iter()
                .collect(),
            },
            ..Default::default()
        };
        let records = crate::k2::EntityRecords::default();
        let result = analyze(&records, &derived, &default_config(), 20260514);
        assert!(
            result.notices.iter().any(|n| n.rule_id == "GEO_007"),
            "GEO_007 olmalı"
        );
        assert!(
            !result.notices.iter().any(|n| n.rule_id == "GEO_006"),
            "3× eşik üstü segment GEO_006 ÜRETMEMELİ (GEO_007'nin işi)"
        );
        assert!(
            !result.notices.iter().any(|n| n.rule_id == "SHP_011"),
            "SHP_011 kaldırıldı — GEO_006 ile aynı eşiği tarıyordu"
        );
    }

    /// A segment between the threshold and 3× produces GEO_006, not GEO_007 (the lower tier).
    #[test]
    fn moderate_shape_jump_produces_geo_006_only() {
        use crate::k5_derived::{ShapeGeometry, ShapeSegments};
        // default max_shape_jump_km = 10.0 → severe = 30.0; 15 km lies between them
        let derived = DerivedData {
            shape_geometry: ShapeGeometry {
                shapes: [(
                    "S1".to_string(),
                    ShapeSegments {
                        segment_distances_km: vec![15.0],
                        total_length_km: 15.0,
                        bbox: (41.0, 41.5, 29.0, 29.5),
                    },
                )]
                .into_iter()
                .collect(),
            },
            ..Default::default()
        };
        let records = crate::k2::EntityRecords::default();
        let result = analyze(&records, &derived, &default_config(), 20260514);
        assert!(
            result.notices.iter().any(|n| n.rule_id == "GEO_006"),
            "GEO_006 olmalı"
        );
        assert!(
            !result.notices.iter().any(|n| n.rule_id == "GEO_007"),
            "GEO_007 olmamalı"
        );
    }

    #[test]
    fn normal_shape_jump_no_geo_007() {
        use crate::k5_derived::{ShapeGeometry, ShapeSegments};
        // 2 km < 15 km severe threshold → tetiklenmemeli
        let derived = DerivedData {
            shape_geometry: ShapeGeometry {
                shapes: [(
                    "S1".to_string(),
                    ShapeSegments {
                        segment_distances_km: vec![2.0],
                        total_length_km: 2.0,
                        bbox: (41.0, 41.02, 29.0, 29.02),
                    },
                )]
                .into_iter()
                .collect(),
            },
            ..Default::default()
        };
        let records = crate::k2::EntityRecords::default();
        let result = analyze(&records, &derived, &default_config(), 20260514);
        assert!(!result.notices.iter().any(|n| n.rule_id == "GEO_007"));
    }

    // ── GEO_009 / SHP_013: stop far from shape ──────────────────────────────

    #[test]
    fn stop_far_from_shape_produces_geo_009() {
        let mut shape_ti = ShapeInternTable::new();
        use crate::k2::shapes::ShapePointRecord;
        // The stop is in Istanbul and the shape is in Ankara → about 350 km apart
        let mut records = records_with(
            vec![stop("IST", 41.0, 29.0)],
            vec![route("R1", 3)],
            vec![trip_sh("T1", "R1", "S1")],
            vec![stoptime("T1", 1, "IST", (8, 0, 0), (8, 0, 0), 2)],
        );
        records.shapes = vec![ShapePointRecord::new(
            shape_ti.intern("S1"),
            Some(39.9),
            Some(32.9),
            Some(1),
            None,
            2,
        )];
        records.shape_interns = shape_ti.clone();
        // default stop_far_from_shape_m = 150.0 → 350 000 m >> 150 m
        let result = analyze(&records, &empty_derived(), &default_config(), 20260514);
        let n = result
            .notices
            .iter()
            .find(|n| n.rule_id == "GEO_009")
            .expect("GEO_009 olmalı");
        assert!(
            !result.notices.iter().any(|n| n.rule_id == "SHP_013"),
            "SHP_013 artık üretilmemeli"
        );
        // shape_id must be carried in details: large-feed mode filters map data in name_index
        // by the IDs in notices; an ID that appears only in the message text is invisible.
        assert_eq!(
            n.details
                .as_ref()
                .and_then(|d| d.get("shape_id"))
                .map(String::as_str),
            Some("S1")
        );
    }

    #[test]
    fn shp_012_counts_distinct_stops_not_trip_occurrences() {
        let mut shape_ti = ShapeInternTable::new();
        use crate::k2::shapes::ShapePointRecord;
        // Three trips use the SAME shape, and all have the SAME far stop.
        // The old counter counted trip-stop pairs and said "3 stops"; the correct count is 1.
        // (In VBB, shape '10342' had 579 trips × 1 far stop = "far from 579 stops".)
        let mut records = records_with(
            vec![stop("NEAR", 41.0, 29.0), stop("FAR", 41.5, 29.5)],
            vec![route("R1", 3)],
            vec![
                trip_sh("T1", "R1", "S1"),
                trip_sh("T2", "R1", "S1"),
                trip_sh("T3", "R1", "S1"),
            ],
            vec![
                stoptime("T1", 1, "NEAR", (8, 0, 0), (8, 0, 0), 2),
                stoptime("T1", 2, "FAR", (8, 10, 0), (8, 10, 0), 3),
                stoptime("T2", 1, "NEAR", (9, 0, 0), (9, 0, 0), 4),
                stoptime("T2", 2, "FAR", (9, 10, 0), (9, 10, 0), 5),
                stoptime("T3", 1, "NEAR", (10, 0, 0), (10, 0, 0), 6),
                stoptime("T3", 2, "FAR", (10, 10, 0), (10, 10, 0), 7),
            ],
        );
        records.shapes = vec![
            ShapePointRecord::new(
                shape_ti.intern("S1"),
                Some(41.0),
                Some(29.0),
                Some(1),
                None,
                2,
            ),
            ShapePointRecord::new(
                shape_ti.intern("S1"),
                Some(41.001),
                Some(29.001),
                Some(2),
                None,
                3,
            ),
        ];
        records.shape_interns = shape_ti.clone();
        let result = analyze(&records, &empty_derived(), &default_config(), 20260514);
        let n = result
            .notices
            .iter()
            .find(|n| n.rule_id == "SHP_012")
            .expect("uzak durak SHP_012 üretmeli");
        let obs = n.observed_value.as_deref().unwrap_or("");
        assert!(
            obs.starts_with("1 "),
            "benzersiz durak sayılmalı (sefer örneği değil): {obs}"
        );
        assert!(
            n.message.contains("1 duraktan"),
            "mesaj da benzersiz sayıyı yazmalı: {}",
            n.message
        );
        // IDs are carried in details so the map can show the faulty stop in a DISTINCT COLOR.
        assert_eq!(
            n.details
                .as_ref()
                .and_then(|d| d.get("far_stops"))
                .map(String::as_str),
            Some("FAR"),
        );
    }

    /// `SHP_012` was DROPPING coordinates with surrounding whitespace — the #121 fix did
    /// not reach this path.
    ///
    /// K2 strict lexical parsing rejects `" 29.5"` as invalid and leaves `stop_lon` as
    /// `None` (the root of DQ_016 is demonstrated there). Because `check_shp012` uses
    /// raw `stop_lat.zip(stop_lon)`, those stops NEVER entered `stop_coords`, so they
    /// were not counted no matter how far they were from the shape.
    ///
    /// In the full-catalog run, 36 feeds were silent for this reason; MobilityData reports
    /// all of them, and all 62 examples exceed the 100 m threshold (median 146 m, maximum
    /// 69 km). In `mdb-1216`, all 97 of 97 stops, and in `mdb-1913`, all 204 of 204 stops,
    /// have whitespace-padded coordinates — the rule was therefore completely blind in
    /// those feeds (#146).
    #[test]
    fn shp_012_recovers_whitespace_stop_coordinates() {
        let mut shape_ti = ShapeInternTable::new();
        use crate::k2::shapes::ShapePointRecord;
        let near = stop("NEAR", 41.0, 29.0);
        // Far stop: K2 rejected it, but the raw row still contains the numeric payload.
        let mut far = stop("FAR", 41.5, 29.5);
        far.stop_lat = None;
        far.stop_lon = None;
        far.row.insert("stop_lat".into(), " 41.5".into());
        far.row.insert("stop_lon".into(), " 29.5".into());

        let mut records = records_with(
            vec![near, far],
            vec![route("R1", 3)],
            vec![trip_sh("T1", "R1", "S1")],
            vec![
                stoptime("T1", 1, "NEAR", (8, 0, 0), (8, 0, 0), 2),
                stoptime("T1", 2, "FAR", (8, 10, 0), (8, 10, 0), 3),
            ],
        );
        records.shapes = vec![
            ShapePointRecord::new(
                shape_ti.intern("S1"),
                Some(41.0),
                Some(29.0),
                Some(1),
                None,
                2,
            ),
            ShapePointRecord::new(
                shape_ti.intern("S1"),
                Some(41.001),
                Some(29.001),
                Some(2),
                None,
                3,
            ),
        ];
        records.shape_interns = shape_ti.clone();

        let result = analyze(&records, &empty_derived(), &default_config(), 20260514);
        let n = result
            .notices
            .iter()
            .find(|n| n.rule_id == "SHP_012")
            .expect("kurtarılabilir boşluklu koordinat SHP_012'ye ULAŞMALI");
        assert_eq!(
            n.details
                .as_ref()
                .and_then(|d| d.get("far_stops"))
                .map(String::as_str),
            Some("FAR"),
            "uzak durak kimliği raporlanmalı"
        );
    }

    // ── WP-N: STM_045 / OPR_001 route_type awareness ─────────────────────────

    /// A 38:10 departure is valid GTFS for a long-distance train (service-day notation
    /// after midnight). The same value exceeds the 27-hour window for a bus and is flagged.
    #[test]
    fn stm_045_uses_a_wider_service_day_window_on_rail() {
        let build = |route_type: u32| {
            let mut r = crate::k2::EntityRecords::default();
            r.stops = vec![stop("A", 41.0, 29.0), stop("B", 41.1, 29.1)];
            r.routes = vec![route("R1", route_type)];
            r.trips = vec![trip("T1", "R1")];
            r.trip_interns = take_ti();
            r.stop_times = vec![
                stoptime("T1", 1, "A", (20, 0, 0), (20, 0, 0), 2),
                stoptime("T1", 2, "B", (38, 10, 0), (38, 10, 0), 3),
            ];
            r.stop_times_index = K2StopTimesIndex::from_records(&r.stop_times);
            analyze(&r, &empty_derived(), &default_config(), 20260514)
        };
        assert!(
            build(3).notices.iter().any(|n| n.rule_id == "STM_045"),
            "otobüste 38:10 > 27 sa penceresi → STM_045 çıkmalı"
        );
        assert!(
            !build(2).notices.iter().any(|n| n.rule_id == "STM_045"),
            "raylı seferde 38:10 < 48 sa rail penceresi → STM_045 ÇIKMAMALI"
        );
    }

    /// The rail window is not unlimited either: a value above 48 hours is a genuine typo.
    #[test]
    fn stm_045_still_catches_absurd_times_on_rail() {
        let mut r = crate::k2::EntityRecords::default();
        r.stops = vec![stop("A", 41.0, 29.0), stop("B", 41.1, 29.1)];
        r.routes = vec![route("R1", 2)];
        r.trips = vec![trip("T1", "R1")];
        r.trip_interns = take_ti();
        r.stop_times = vec![
            stoptime("T1", 1, "A", (20, 0, 0), (20, 0, 0), 2),
            stoptime("T1", 2, "B", (99, 0, 0), (99, 0, 0), 3),
        ];
        r.stop_times_index = K2StopTimesIndex::from_records(&r.stop_times);
        let result = analyze(&r, &empty_derived(), &default_config(), 20260514);
        assert!(
            result.notices.iter().any(|n| n.rule_id == "STM_045"),
            "99:00 raylı pencerede bile veri hatasıdır → STM_045 çıkmalı"
        );
    }

    /// A 580-minute headway is normal for an intercity train; for a bus it exceeds the
    /// 240-minute threshold.
    #[test]
    fn opr_001_uses_a_wider_headway_threshold_on_rail() {
        let build = |route_type: u32| {
            // Irregular departures (so the is_regular_headway guard does not skip them):
            // 06:00, 08:00, 09:00, 18:40 → the largest gap is 580 minutes.
            let deps = [(6u32, 0u32), (8, 0), (9, 0), (18, 40)];
            let mut trips_v = Vec::new();
            let mut sts = Vec::new();
            for (i, (h, m)) in deps.iter().enumerate() {
                let tid = format!("T{i}");
                trips_v.push(trip(&tid, "R1"));
                sts.push(stoptime(&tid, 1, "A", (*h, *m, 0), (*h, *m, 0), 2));
                sts.push(stoptime(&tid, 2, "B", (*h + 1, *m, 0), (*h + 1, *m, 0), 3));
            }
            let r = crate::k2::EntityRecords {
                stops: vec![stop("A", 41.0, 29.0), stop("B", 41.1, 29.1)],
                routes: vec![route("R1", route_type)],
                trips: trips_v,
                trip_interns: take_ti(),
                stop_times: sts,
                ..Default::default()
            };
            analyze(&r, &empty_derived(), &default_config(), 20260514)
        };
        assert!(
            build(3).notices.iter().any(|n| n.rule_id == "OPR_001"),
            "otobüste 580dk > 240dk eşik → OPR_001 çıkmalı"
        );
        assert!(
            !build(2).notices.iter().any(|n| n.rule_id == "OPR_001"),
            "raylı hatta 580dk < 720dk rail eşiği → OPR_001 ÇIKMAMALI"
        );
    }

    // SHP_027 tests REMOVED (2026-07-31): the rule was removed. Trains stopping at
    // different stations on the same rail corridor normally share one shape; the remaining
    // genuine signal is measured directly by GEO_009/SHP_012 (see the note in
    // check_geo_analytics).

    // ── WP-B: rail geometry thresholds ───────────────────────────────────────
    //
    // In rail geometry, the stop coordinate represents the platform/building center, while
    // the shape represents one track centerline; the 100 m threshold produced FPs for
    // legitimate deviations in this setup (TCDD feedback). For rail route_types,
    // `stop_far_from_shape_m_rail` (200 m) is used.

    /// Same geometry: GEO_009/SHP_012 fires for a bus route, but NOT for a rail route.
    /// The stop is about 166 m from the shape — above the 100 m threshold but below the
    /// 200 m rail threshold.
    #[test]
    fn rail_route_widens_stop_shape_distance_threshold() {
        use crate::k2::shapes::ShapePointRecord;

        // route_type is parameterized: 3 = bus, 2 = train.
        let build = |route_type: u32| {
            let mut shape_ti = ShapeInternTable::new();
            let mut records = records_with(
                // The stop is 0.0015° (~166 m) north of the shape line.
                vec![stop("P1", 41.0015, 29.005)],
                vec![route("R1", route_type)],
                vec![trip_sh("T1", "R1", "S1")],
                vec![stoptime("T1", 1, "P1", (8, 0, 0), (8, 0, 0), 2)],
            );
            records.shapes = vec![
                ShapePointRecord::new(
                    shape_ti.intern("S1"),
                    Some(41.0),
                    Some(29.0),
                    Some(1),
                    None,
                    2,
                ),
                ShapePointRecord::new(
                    shape_ti.intern("S1"),
                    Some(41.0),
                    Some(29.01),
                    Some(2),
                    None,
                    3,
                ),
            ];
            records.shape_interns = shape_ti;
            analyze(&records, &empty_derived(), &default_config(), 20260514)
        };

        let bus = build(3);
        assert!(
            bus.notices.iter().any(|n| n.rule_id == "GEO_009"),
            "otobüs hattında 166 m > 100 m eşik → GEO_009 çıkmalı"
        );
        assert!(
            bus.notices.iter().any(|n| n.rule_id == "SHP_012"),
            "otobüs hattında 166 m > 100 m eşik → SHP_012 çıkmalı"
        );

        let rail = build(2);
        assert!(
            !rail.notices.iter().any(|n| n.rule_id == "GEO_009"),
            "raylı hatta 166 m < 200 m rail eşiği → GEO_009 ÇIKMAMALI"
        );
        assert!(
            !rail.notices.iter().any(|n| n.rule_id == "SHP_012"),
            "raylı hatta 166 m < 200 m rail eşiği → SHP_012 ÇIKMAMALI"
        );
    }

    /// The rail threshold has an upper limit too: a deviation above 200 m is also caught
    /// on a rail route (the threshold was relaxed, not disabled).
    #[test]
    fn rail_threshold_still_catches_truly_far_stop() {
        use crate::k2::shapes::ShapePointRecord;
        let mut shape_ti = ShapeInternTable::new();
        let mut records = records_with(
            // 0.005° ≈ 555 m — also above the rail threshold of 200 m.
            vec![stop("P1", 41.005, 29.005)],
            vec![route("R1", 2)],
            vec![trip_sh("T1", "R1", "S1")],
            vec![stoptime("T1", 1, "P1", (8, 0, 0), (8, 0, 0), 2)],
        );
        records.shapes = vec![
            ShapePointRecord::new(
                shape_ti.intern("S1"),
                Some(41.0),
                Some(29.0),
                Some(1),
                None,
                2,
            ),
            ShapePointRecord::new(
                shape_ti.intern("S1"),
                Some(41.0),
                Some(29.01),
                Some(2),
                None,
                3,
            ),
        ];
        records.shape_interns = shape_ti;
        let result = analyze(&records, &empty_derived(), &default_config(), 20260514);
        let n = result
            .notices
            .iter()
            .find(|n| n.rule_id == "GEO_009")
            .expect("555 m raylı hatta da GEO_009 üretmeli");
        // The message/observed shape must report its OWN threshold (200, not 100).
        assert_eq!(
            n.expected_value.as_deref(),
            Some("≤ 200m"),
            "rail eşiği raporlanmalı"
        );
        assert!(
            n.message.contains("200m"),
            "mesaj rail eşiğini yazmalı: {}",
            n.message
        );
    }

    /// Intercity rail shapes contain simplified long straight sections (13.4 km Konya–Karaman):
    /// GEO_006 fires for a bus, but not for a rail route.
    #[test]
    fn rail_route_widens_shape_jump_threshold() {
        use crate::k2::shapes::ShapePointRecord;
        use crate::k5_derived::{ShapeGeometry, ShapeSegments};

        let build = |route_type: u32| {
            let mut shape_ti = ShapeInternTable::new();
            let mut records = records_with(
                vec![stop("A", 41.0, 29.0)],
                vec![route("R1", route_type)],
                vec![trip_sh("T1", "R1", "S1")],
                vec![stoptime("T1", 1, "A", (8, 0, 0), (8, 0, 0), 2)],
            );
            records.shapes = vec![ShapePointRecord::new(
                shape_ti.intern("S1"),
                Some(41.0),
                Some(29.0),
                Some(1),
                None,
                2,
            )];
            records.shape_interns = shape_ti;
            // 13.4 km: above the urban threshold (10), below the rail threshold (30).
            let derived = DerivedData {
                shape_geometry: ShapeGeometry {
                    shapes: [(
                        "S1".to_string(),
                        ShapeSegments {
                            segment_distances_km: vec![13.4],
                            total_length_km: 13.4,
                            bbox: (41.0, 41.5, 29.0, 29.5),
                        },
                    )]
                    .into_iter()
                    .collect(),
                },
                ..Default::default()
            };
            analyze(&records, &derived, &default_config(), 20260514)
        };

        assert!(
            build(3).notices.iter().any(|n| n.rule_id == "GEO_006"),
            "otobüs hattında 13,4 km > 10 km eşik → GEO_006 çıkmalı"
        );
        assert!(
            !build(2).notices.iter().any(|n| n.rule_id == "GEO_006"),
            "raylı hatta 13,4 km < 30 km rail eşiği → GEO_006 ÇIKMAMALI"
        );
    }

    /// Extended European codes (100–117) also count as rail — feeds such as VBB/Entur do
    /// not use the standard route_type values. `is_rail_route_type` provides ONE shared
    /// definition.
    #[test]
    fn extended_rail_route_types_use_rail_threshold() {
        use crate::k2::shapes::ShapePointRecord;
        let mut shape_ti = ShapeInternTable::new();
        let mut records = records_with(
            vec![stop("P1", 41.0015, 29.005)],
            vec![route("R1", 109)], // 109 = suburban train (S-Bahn)
            vec![trip_sh("T1", "R1", "S1")],
            vec![stoptime("T1", 1, "P1", (8, 0, 0), (8, 0, 0), 2)],
        );
        records.shapes = vec![
            ShapePointRecord::new(
                shape_ti.intern("S1"),
                Some(41.0),
                Some(29.0),
                Some(1),
                None,
                2,
            ),
            ShapePointRecord::new(
                shape_ti.intern("S1"),
                Some(41.0),
                Some(29.01),
                Some(2),
                None,
                3,
            ),
        ];
        records.shape_interns = shape_ti;
        let result = analyze(&records, &empty_derived(), &default_config(), 20260514);
        assert!(
            !result.notices.iter().any(|n| n.rule_id == "GEO_009"),
            "route_type 109 raylıdır → 166 m rail eşiği altında kalmalı"
        );
    }

    /// In mixed use, take the RELAXED direction: if a shape is used by both buses and
    /// trains, the rail threshold applies. FPs for these rules (treating valid geometry
    /// as an error) cost more than FNs.
    #[test]
    fn mixed_use_shape_takes_the_looser_rail_threshold() {
        use crate::k2::shapes::ShapePointRecord;
        let mut shape_ti = ShapeInternTable::new();
        let mut records = records_with(
            vec![stop("P1", 41.0015, 29.005)],
            vec![route("BUS", 3), route("RAIL", 2)],
            vec![trip_sh("T1", "BUS", "S1"), trip_sh("T2", "RAIL", "S1")],
            vec![
                stoptime("T1", 1, "P1", (8, 0, 0), (8, 0, 0), 2),
                stoptime("T2", 1, "P1", (9, 0, 0), (9, 0, 0), 3),
            ],
        );
        records.shapes = vec![
            ShapePointRecord::new(
                shape_ti.intern("S1"),
                Some(41.0),
                Some(29.0),
                Some(1),
                None,
                2,
            ),
            ShapePointRecord::new(
                shape_ti.intern("S1"),
                Some(41.0),
                Some(29.01),
                Some(2),
                None,
                3,
            ),
        ];
        records.shape_interns = shape_ti;
        let result = analyze(&records, &empty_derived(), &default_config(), 20260514);
        assert!(
            !result.notices.iter().any(|n| n.rule_id == "GEO_009"),
            "shape'i bir raylı hat da kullanıyorsa rail eşiği geçerli olmalı"
        );
    }

    #[test]
    fn stop_near_shape_no_geo_009() {
        let mut shape_ti = ShapeInternTable::new();
        use crate::k2::shapes::ShapePointRecord;
        let mut records = records_with(
            vec![stop("A", 41.0, 29.0)],
            vec![route("R1", 3)],
            vec![trip_sh("T1", "R1", "S1")],
            vec![stoptime("T1", 1, "A", (8, 0, 0), (8, 0, 0), 2)],
        );
        // The shape point is right beside the stop (< 1 m)
        records.shapes = vec![ShapePointRecord::new(
            shape_ti.intern("S1"),
            Some(41.0),
            Some(29.0),
            Some(1),
            None,
            2,
        )];
        records.shape_interns = shape_ti.clone();
        let result = analyze(&records, &empty_derived(), &default_config(), 20260514);
        assert!(!result.notices.iter().any(|n| n.rule_id == "GEO_009"));
        assert!(!result.notices.iter().any(|n| n.rule_id == "SHP_013"));
    }

    // ── GEO_013: feed coordinate summary (Info) ─────────────────────────────

    #[test]
    fn stops_with_coords_produce_geo_013() {
        let records = records_with(
            vec![stop("A", 41.0, 29.0)],
            vec![route("R1", 3)],
            vec![],
            vec![],
        );
        let result = analyze(&records, &empty_derived(), &default_config(), 20260514);
        assert!(
            result.notices.iter().any(|n| n.rule_id == "GEO_013"),
            "GEO_013 olmalı"
        );
    }

    // ── OPR_005: unusual service frequency (route_type-relative outlier) ────

    // Produces a route with a 15-minute headway (08:00, 08:15).
    fn normal_route(rid: &str, stops: &mut Vec<StopTimeRecord>, trips: &mut Vec<TripRecord>) {
        let (ta, tb) = (format!("{rid}a"), format!("{rid}b"));
        trips.push(trip(&ta, rid));
        trips.push(trip(&tb, rid));
        stops.push(stoptime(&ta, 1, "A", (8, 0, 0), (8, 0, 0), 2));
        stops.push(stoptime(&ta, 2, "B", (8, 10, 0), (8, 10, 0), 3));
        stops.push(stoptime(&tb, 1, "A", (8, 15, 0), (8, 15, 0), 4));
        stops.push(stoptime(&tb, 2, "B", (8, 25, 0), (8, 25, 0), 5));
    }

    #[test]
    fn opr_005_flags_only_unusually_sparse_route() {
        // 5 normal routes (15-minute headway) + 1 sparse route (90 minutes): only the sparse
        // route is flagged.
        let mut routes = Vec::new();
        let mut trips = Vec::new();
        let mut sts = Vec::new();
        for i in 0..5u32 {
            let rid = format!("R{i}");
            routes.push(route(&rid, 3));
            normal_route(&rid, &mut sts, &mut trips);
        }
        // Sparse route: 08:00 and 09:30 → 90-minute headway.
        routes.push(route("RS", 3));
        trips.push(trip("RSa", "RS"));
        trips.push(trip("RSb", "RS"));
        sts.push(stoptime("RSa", 1, "A", (8, 0, 0), (8, 0, 0), 2));
        sts.push(stoptime("RSa", 2, "B", (8, 10, 0), (8, 10, 0), 3));
        sts.push(stoptime("RSb", 1, "A", (9, 30, 0), (9, 30, 0), 4));
        sts.push(stoptime("RSb", 2, "B", (9, 40, 0), (9, 40, 0), 5));
        let records = records_with(
            vec![stop("A", 41.0, 29.0), stop("B", 41.1, 29.1)],
            routes,
            trips,
            sts,
        );
        let result = analyze(&records, &empty_derived(), &default_config(), 20260514);
        let opr: Vec<_> = result
            .notices
            .iter()
            .filter(|n| n.rule_id == "OPR_005")
            .collect();
        assert_eq!(opr.len(), 1, "yalnızca sıradışı seyrek hat işaretlenmeli");
        assert_eq!(opr[0].entity_id.as_deref(), Some("RS"));
    }

    #[test]
    fn opr_017_stays_silent_when_the_distance_cannot_be_measured() {
        // 14th corpus run (2026-09-01): even AFTER the loop fix in run 13, 91,417
        // findings remained and the most common example value was "0m" (47 of 124 feeds).
        // ZERO does not mean "measured and zero"; it means "NOT MEASURABLE". The rule
        // makes a DISTANCE claim.
        // Three case classes were measured, and all three have their root cause reported
        // by ANOTHER rule:
        //   tld-821  → the trip visits the same stop twice (STM_035 had 5 there, OPR_017
        //              also had 5: the same phenomenon reported a second time)
        //   mdb-1316 → 149 of 158 stops are (0,0), Null Island (GEO_016 = 149)
        //   ntd-10183 → the same stop three times + a one-stop trip (STM_033 = 37)
        let stops = vec![stop("A", 0.0, 0.0), stop("B", 0.0, 0.0)];
        let sts = vec![
            stoptime("ZERO", 1, "A", (8, 0, 0), (8, 0, 0), 2),
            stoptime("ZERO", 2, "B", (8, 30, 0), (8, 30, 0), 3),
        ];
        let records = records_with(stops, vec![route("R1", 3)], vec![trip("ZERO", "R1")], sts);
        let result = analyze(&records, &empty_derived(), &default_config(), 20260514);
        assert!(
            !result.notices.iter().any(|n| n.rule_id == "OPR_017"),
            "yol uzunlugu 0 ise mesafe OLCULEMEMISTIR; OPR_017 iddia kurmamali",
        );
    }

    #[test]
    fn opr_017_still_flags_a_genuinely_short_but_measurable_trip() {
        // Counterexample: in mdb-893, the shape is 24 m and the stops are 17 m apart, so
        // the finding is CORRECT. Zero suppression must not hide genuinely short trips.
        let stops = vec![stop("A", 41.00000, 29.00000), stop("B", 41.00015, 29.00000)];
        let sts = vec![
            stoptime("SHORT", 1, "A", (8, 0, 0), (8, 0, 0), 2),
            stoptime("SHORT", 2, "B", (8, 2, 0), (8, 2, 0), 3),
        ];
        let records = records_with(stops, vec![route("R1", 3)], vec![trip("SHORT", "R1")], sts);
        let result = analyze(&records, &empty_derived(), &default_config(), 20260514);
        assert!(
            result.notices.iter().any(|n| n.rule_id == "OPR_017"),
            "olculebilir ve gercekten kisa sefer raporlanmali",
        );
    }

    #[test]
    fn opr_017_measures_the_path_not_the_straight_line_on_a_loop() {
        // 🔴 FP found by the 13th corpus run: when no shape existed, the straight-line
        // distance from the first to the last stop was measured, so a LOOP route (first
        // stop == last stop) always came out as 0 m. This accounted for almost all 92,354
        // findings in the Singapore LTA feed; the median actual route length was 20.9 km.
        // Here the trip loops A→B→C→A: straight-line distance 0, consecutive total ~44 km.
        let stops = vec![
            stop("A", 41.00, 29.00),
            stop("B", 41.10, 29.00),
            stop("C", 41.10, 29.20),
        ];
        let sts = vec![
            stoptime("LOOP", 1, "A", (8, 0, 0), (8, 0, 0), 2),
            stoptime("LOOP", 2, "B", (8, 20, 0), (8, 20, 0), 3),
            stoptime("LOOP", 3, "C", (8, 40, 0), (8, 40, 0), 4),
            stoptime("LOOP", 4, "A", (9, 0, 0), (9, 0, 0), 5),
        ];
        let records = records_with(stops, vec![route("R1", 3)], vec![trip("LOOP", "R1")], sts);
        let result = analyze(&records, &empty_derived(), &default_config(), 20260514);
        assert!(
            !result.notices.iter().any(|n| n.rule_id == "OPR_017"),
            "döngüsel sefer gerçek yolu boyunca uzun; OPR_017 susmalı"
        );
    }

    #[test]
    fn opr_017_still_flags_a_genuinely_short_trip() {
        // What the guard protects: genuinely short trips must STILL be detected. Two stops
        // about 8 m apart.
        let stops = vec![
            stop("A", 41.000_00, 29.000_00),
            stop("B", 41.000_05, 29.000_05),
        ];
        let sts = vec![
            stoptime("SHORT", 1, "A", (8, 0, 0), (8, 0, 0), 2),
            stoptime("SHORT", 2, "B", (8, 1, 0), (8, 1, 0), 3),
        ];
        let records = records_with(stops, vec![route("R1", 3)], vec![trip("SHORT", "R1")], sts);
        let result = analyze(&records, &empty_derived(), &default_config(), 20260514);
        assert!(
            result.notices.iter().any(|n| n.rule_id == "OPR_017"),
            "100 m altındaki gerçek sefer işaretlenmeli"
        );
    }

    #[test]
    fn opr_005_silent_when_frequency_is_uniform() {
        // 6 routes, all with a 15-minute headway → no outlier.
        let mut routes = Vec::new();
        let mut trips = Vec::new();
        let mut sts = Vec::new();
        for i in 0..6u32 {
            let rid = format!("R{i}");
            routes.push(route(&rid, 3));
            normal_route(&rid, &mut sts, &mut trips);
        }
        let records = records_with(
            vec![stop("A", 41.0, 29.0), stop("B", 41.1, 29.1)],
            routes,
            trips,
            sts,
        );
        let result = analyze(&records, &empty_derived(), &default_config(), 20260514);
        assert!(
            !result.notices.iter().any(|n| n.rule_id == "OPR_005"),
            "tekdüze sıklıkta OPR_005 üretilmemeli"
        );
    }

    // ── OPR_013: one-way route information ─────────────────────────────────

    #[test]
    fn single_direction_route_produces_opr_013() {
        let records = records_with(
            vec![stop("A", 41.0, 29.0), stop("B", 41.1, 29.1)],
            vec![route("R1", 3)],
            vec![{
                let mut t = trip("T1", "R1");
                t.direction_id = Some(0);
                t
            }],
            vec![
                stoptime("T1", 1, "A", (8, 0, 0), (8, 0, 0), 2),
                stoptime("T1", 2, "B", (8, 10, 0), (8, 10, 0), 3),
            ],
        );
        let result = analyze(&records, &empty_derived(), &default_config(), 20260514);
        assert!(
            result.notices.iter().any(|n| n.rule_id == "OPR_013"),
            "OPR_013 olmalı"
        );
    }

    // ── DQ_005b: no stop_times for any trip ─────────────────────────────────

    #[test]
    fn trips_without_stoptimes_produces_dq_005b() {
        // Trip exists, but there are no stop_times.
        let records = records_with(
            vec![stop("A", 41.0, 29.0)],
            vec![route("R1", 3)],
            vec![trip("T1", "R1")],
            vec![], // stop_times is empty
        );
        let result = analyze(&records, &empty_derived(), &default_config(), 20260514);
        assert!(
            result.notices.iter().any(|n| n.rule_id == "DQ_005b"),
            "DQ_005b olmalı"
        );
    }

    // ── DQ_005c: proportion of stops without coordinates > 50% ──────────────

    #[test]
    fn mostly_coord_missing_stops_produces_dq_005c() {
        use crate::k2::stops::StopRecord;
        let no_coord = StopRecord {
            stop_id: "X".into(),
            stop_code: None,
            stop_name: None,
            stop_lat: None,
            stop_lon: None,
            location_type: None,
            stop_timezone: None,
            wheelchair_boarding: None,
            stop_access: None,
            level_id: None,
            tts_stop_name: None,
            row: Default::default(),
            line: 3,
            ..Default::default()
        };
        let records = records_with(
            vec![stop("A", 41.0, 29.0), no_coord.clone(), {
                let mut s = no_coord.clone();
                s.stop_id = "Y".into();
                s
            }],
            vec![],
            vec![],
            vec![],
        );
        // 1 with coordinates, 2 without → 67% > 50% → DQ_005c
        let result = analyze(&records, &empty_derived(), &default_config(), 20260514);
        assert!(
            result.notices.iter().any(|n| n.rule_id == "DQ_005c"),
            "DQ_005c olmalı"
        );
    }

    // ── DQ_010: agency_id is unused by any route ────────────────────────────

    #[test]
    fn unused_agency_id_produces_dq_010() {
        use crate::k2::agency::AgencyRecord;
        let mut records = records_with(
            vec![],
            vec![route("R1", 3)], // agency_id = None
            vec![],
            vec![],
        );
        records.agencies = vec![AgencyRecord {
            agency_id: Some("AG1".into()),
            agency_name: "Test Agency".into(),
            agency_url: "https://test.example".into(),
            agency_timezone: "Europe/Istanbul".into(),
            agency_lang: None,
            agency_phone: None,
            agency_fare_url: None,
            agency_email: None,
            agency_cemv_support: None,
            row: Default::default(),
            line: 2,
        }];
        let mut used_agency = records.agencies[0].clone();
        used_agency.agency_id = Some("AG2".into());
        used_agency.line = 3;
        records.agencies.push(used_agency);
        records.routes[0].agency_id = Some("AG2".into());
        let result = analyze(&records, &empty_derived(), &default_config(), 20260514);
        assert!(
            result.notices.iter().any(|n| n.rule_id == "DQ_010"),
            "DQ_010 olmalı"
        );
    }

    // ── RTS_017: shape'siz route ─────────────────────────────────────────────

    #[test]
    fn route_without_shape_produces_rts_017() {
        // The trip has no shape_id → the route has no shape.
        let records = records_with(
            vec![stop("A", 41.0, 29.0), stop("B", 41.1, 29.1)],
            vec![route("R1", 3)],
            vec![trip("T1", "R1")], // shape_id = None
            vec![
                stoptime("T1", 1, "A", (8, 0, 0), (8, 0, 0), 2),
                stoptime("T1", 2, "B", (8, 10, 0), (8, 10, 0), 3),
            ],
        );
        let result = analyze(&records, &empty_derived(), &default_config(), 20260514);
        assert!(
            result.notices.iter().any(|n| n.rule_id == "RTS_017"),
            "RTS_017 olmalı"
        );
    }

    #[test]
    fn route_with_shape_no_rts_017() {
        let records = records_with(
            vec![stop("A", 41.0, 29.0), stop("B", 41.1, 29.1)],
            vec![route("R1", 3)],
            vec![trip_sh("T1", "R1", "S1")],
            vec![
                stoptime("T1", 1, "A", (8, 0, 0), (8, 0, 0), 2),
                stoptime("T1", 2, "B", (8, 10, 0), (8, 10, 0), 3),
            ],
        );
        let result = analyze(&records, &empty_derived(), &default_config(), 20260514);
        assert!(!result.notices.iter().any(|n| n.rule_id == "RTS_017"));
    }

    #[test]
    fn rts_017_one_notice_per_shapeless_route() {
        // Two routes without shapes (R1, R2) + one with a shape (R3) → separate notices
        // only for R1 and R2.
        let records = records_with(
            vec![stop("A", 41.0, 29.0), stop("B", 41.1, 29.1)],
            vec![route("R1", 3), route("R2", 3), route("R3", 3)],
            vec![
                trip("T1", "R1"),
                trip("T2", "R2"),
                trip_sh("T3", "R3", "S1"),
            ],
            vec![
                stoptime("T1", 1, "A", (8, 0, 0), (8, 0, 0), 2),
                stoptime("T1", 2, "B", (8, 10, 0), (8, 10, 0), 3),
                stoptime("T2", 1, "A", (8, 0, 0), (8, 0, 0), 4),
                stoptime("T2", 2, "B", (8, 10, 0), (8, 10, 0), 5),
                stoptime("T3", 1, "A", (8, 0, 0), (8, 0, 0), 6),
                stoptime("T3", 2, "B", (8, 10, 0), (8, 10, 0), 7),
            ],
        );
        let result = analyze(&records, &empty_derived(), &default_config(), 20260514);
        let rts: Vec<_> = result
            .notices
            .iter()
            .filter(|n| n.rule_id == "RTS_017")
            .collect();
        assert_eq!(rts.len(), 2, "her shape'siz hat için ayrı RTS_017 beklenir");
        assert!(rts
            .iter()
            .all(|n| n.entity_type == gtfs_core::EntityType::Route));
        let ids: std::collections::HashSet<_> =
            rts.iter().filter_map(|n| n.entity_id.as_deref()).collect();
        assert!(ids.contains("R1") && ids.contains("R2") && !ids.contains("R3"));
    }

    // ── TRP_012: trip missing direction_id on a bidirectional route ──────────

    #[test]
    fn bidirectional_route_missing_direction_produces_trp_012() {
        let records = records_with(
            vec![stop("A", 41.0, 29.0), stop("B", 41.1, 29.1)],
            vec![route("R1", 3)],
            vec![
                {
                    let mut t = trip("T1", "R1");
                    t.direction_id = Some(0);
                    t
                },
                {
                    let mut t = trip("T2", "R1");
                    t.direction_id = Some(1);
                    t
                },
                trip("T3", "R1"), // direction_id = None — the other trips on this route have one
            ],
            vec![
                stoptime("T1", 1, "A", (8, 0, 0), (8, 0, 0), 2),
                stoptime("T2", 1, "B", (9, 0, 0), (9, 0, 0), 3),
                stoptime("T3", 1, "A", (10, 0, 0), (10, 0, 0), 4),
            ],
        );
        let result = analyze(&records, &empty_derived(), &default_config(), 20260514);
        assert!(
            result.notices.iter().any(|n| n.rule_id == "TRP_012"),
            "TRP_012 olmalı"
        );
    }

    // ── TRP_015: block_id singleton ──────────────────────────────────────────

    #[test]
    fn block_id_that_groups_nothing_produces_one_feed_level_trp_015() {
        // Two trips, two SEPARATE block_ids → no block groups two trips.
        // The field effectively duplicates trip_id → ONE finding per FEED.
        let records = records_with(
            vec![stop("A", 41.0, 29.0), stop("B", 41.1, 29.1)],
            vec![route("R1", 3)],
            vec![trip_blk("T1", "R1", "BLK1"), trip_blk("T2", "R1", "BLK2")],
            vec![
                stoptime("T1", 1, "A", (8, 0, 0), (8, 0, 0), 2),
                stoptime("T2", 1, "B", (9, 0, 0), (9, 0, 0), 3),
            ],
        );
        let result = analyze(&records, &empty_derived(), &default_config(), 20260514);
        let hits: Vec<_> = result
            .notices
            .iter()
            .filter(|n| n.rule_id == "TRP_015")
            .collect();
        assert_eq!(
            hits.len(),
            1,
            "feed basina TEK bulgu bekleniyor, bulunan: {}",
            hits.len()
        );
        assert!(
            hits[0].entity_id.is_none(),
            "feed duzeyi bulgu trip kimligi tasimamali"
        );
    }

    #[test]
    fn a_single_trip_block_is_valid_per_spec_and_stays_silent() {
        // 🔴 ESKI HUKUM SPEC'E AYKIRIYDI. GTFS Schedule Reference, trips.block_id:
        // "A block consists of a single trip or many sequential trips ..." — a single-trip
        // block is VALID. The old rule reported it once per trip and produced 1,549,904
        // notices across 675 feeds. Here BLK1 groups two trips while BLK2 is a single-trip
        // block: grouping EXISTS, so there is no feed-level finding either.
        let records = records_with(
            vec![stop("A", 41.0, 29.0), stop("B", 41.1, 29.1)],
            vec![route("R1", 3)],
            vec![
                trip_blk("T1", "R1", "BLK1"),
                trip_blk("T2", "R1", "BLK1"),
                trip_blk("T3", "R1", "BLK2"),
            ],
            vec![
                stoptime("T1", 1, "A", (8, 0, 0), (8, 0, 0), 2),
                stoptime("T2", 1, "B", (9, 0, 0), (9, 0, 0), 3),
                stoptime("T3", 1, "A", (10, 0, 0), (10, 0, 0), 2),
            ],
        );
        let result = analyze(&records, &empty_derived(), &default_config(), 20260514);
        assert!(
            !result.notices.iter().any(|n| n.rule_id == "TRP_015"),
            "tek seferlik blok spec'e gore gecerlidir; gruplama varken TRP_015 susmali",
        );
    }

    // ── STP_020: unused stop ────────────────────────────────────────────────

    #[test]
    fn unused_stop_produces_stp_020() {
        // Stop "X" is absent from stop_times → STP_020
        let records = records_with(
            vec![stop("A", 41.0, 29.0), stop("X", 41.5, 29.5)],
            vec![route("R1", 3)],
            vec![trip("T1", "R1")],
            vec![
                stoptime("T1", 1, "A", (8, 0, 0), (8, 0, 0), 2),
                stoptime("T1", 2, "A", (8, 10, 0), (8, 10, 0), 3),
            ],
        );
        let result = analyze(&records, &empty_derived(), &default_config(), 20260514);
        assert!(
            result
                .notices
                .iter()
                .any(|n| n.rule_id == "STP_020" && n.entity_id.as_deref() == Some("X")),
            "STP_020 olmalı"
        );
        assert!(
            !result
                .notices
                .iter()
                .any(|n| n.rule_id == "STP_020" && n.entity_id.as_deref() == Some("A")),
            "kullanılan durak STP_020 üretmemeli"
        );
    }

    #[test]
    fn stop_used_only_via_location_group_is_not_stp_020() {
        // GTFS-Flex: stop_times carries location_group_id and stop_id is empty. The stops in
        // the group are served. Without this path, all 1,259 of 1,259 stops in the Swiss ODV
        // feed were incorrectly reported as "unused".
        let mut records = records_with(
            vec![stop("A", 41.0, 29.0), stop("X", 41.5, 29.5)],
            vec![route("R1", 3)],
            vec![trip("T1", "R1")],
            vec![
                stoptime("T1", 1, "A", (8, 0, 0), (8, 0, 0), 2),
                stoptime("T1", 2, "A", (8, 10, 0), (8, 10, 0), 3),
            ],
        );
        records.location_group_stops = vec![crate::k2::location_groups::LocationGroupStopRecord {
            location_group_id: "LG1".to_string(),
            stop_id: "X".to_string(),
            line: 2,
        }];
        let result = analyze(&records, &empty_derived(), &default_config(), 20260514);
        assert!(
            !result
                .notices
                .iter()
                .any(|n| n.rule_id == "STP_020" && n.entity_id.as_deref() == Some("X")),
            "location group üyesi durak STP_020 üretmemeli"
        );
    }

    #[test]
    fn parent_station_not_flagged_stp_020() {
        // location_type=1 (parent station) is not used in stop_times — this is normal
        let mut parent = stop("PS1", 41.0, 29.0);
        parent.location_type = Some(1);
        let records = records_with(
            vec![parent, stop("A", 41.0, 29.0)],
            vec![route("R1", 3)],
            vec![trip("T1", "R1")],
            vec![
                stoptime("T1", 1, "A", (8, 0, 0), (8, 0, 0), 2),
                stoptime("T1", 2, "A", (8, 10, 0), (8, 10, 0), 3),
            ],
        );
        let result = analyze(&records, &empty_derived(), &default_config(), 20260514);
        assert!(
            !result
                .notices
                .iter()
                .any(|n| n.rule_id == "STP_020" && n.entity_id.as_deref() == Some("PS1")),
            "parent station STP_020 üretmemeli"
        );
    }

    // ── SHP_017: stop order conflicts with shape projection ─────────────────

    #[test]
    fn stops_in_correct_shape_order_no_shp_017() {
        let mut shape_ti = ShapeInternTable::new();
        use crate::k2::shapes::ShapePointRecord;
        // Shape: (41.0,29.0) → (41.0,29.1) (straight line heading east)
        // Stop A: 29.0 (start), Stop B: 29.1 (end) — correct order
        let mut records = records_with(
            vec![stop("A", 41.0, 29.0), stop("B", 41.0, 29.1)],
            vec![route("R1", 3)],
            vec![trip_sh("T1", "R1", "S1")],
            vec![
                stoptime("T1", 1, "A", (8, 0, 0), (8, 0, 0), 2),
                stoptime("T1", 2, "B", (8, 10, 0), (8, 10, 0), 3),
            ],
        );
        records.shapes = vec![
            ShapePointRecord::new(
                shape_ti.intern("S1"),
                Some(41.0),
                Some(29.0),
                Some(1),
                None,
                2,
            ),
            ShapePointRecord::new(
                shape_ti.intern("S1"),
                Some(41.0),
                Some(29.1),
                Some(2),
                None,
                3,
            ),
        ];
        records.shape_interns = shape_ti.clone();
        let result = analyze(&records, &empty_derived(), &default_config(), 20260514);
        assert!(
            !result.notices.iter().any(|n| n.rule_id == "SHP_017"),
            "doğru sırada SHP_017 olmamalı"
        );
    }

    #[test]
    fn fully_reversed_shape_produces_shp_016_not_shp_017() {
        let mut shape_ti = ShapeInternTable::new();
        use crate::k2::shapes::ShapePointRecord;
        // Shape: A(29.0) → B(29.1) — line heading east
        // stop_times: B first, A second → shape is completely reversed → SHP_016
        // (SHP_016 fires + continue, SHP_017 is not produced)
        let mut records = records_with(
            vec![stop("A", 41.0, 29.0), stop("B", 41.0, 29.1)],
            vec![route("R1", 3)],
            vec![trip_sh("T1", "R1", "S1")],
            vec![
                stoptime("T1", 1, "B", (8, 0, 0), (8, 0, 0), 2),
                stoptime("T1", 2, "A", (8, 10, 0), (8, 10, 0), 3),
            ],
        );
        records.shapes = vec![
            ShapePointRecord::new(
                shape_ti.intern("S1"),
                Some(41.0),
                Some(29.0),
                Some(1),
                None,
                2,
            ),
            ShapePointRecord::new(
                shape_ti.intern("S1"),
                Some(41.0),
                Some(29.1),
                Some(2),
                None,
                3,
            ),
        ];
        records.shape_interns = shape_ti.clone();
        let result = analyze(&records, &empty_derived(), &default_config(), 20260514);
        assert!(
            result.notices.iter().any(|n| n.rule_id == "SHP_016"),
            "tamamen ters shape → SHP_016 olmalı"
        );
        assert!(
            !result.notices.iter().any(|n| n.rule_id == "SHP_017"),
            "tamamen ters shape → SHP_017 olmamalı (SHP_016 öncelikli)"
        );
    }

    #[test]
    fn shp_016_stays_silent_when_the_shape_crosses_itself() {
        let mut shape_ti = ShapeInternTable::new();
        use crate::k2::shapes::ShapePointRecord;
        // Found in the DART (Des Moines) feed: route 97 / trip 751448 / shape 58782.
        // When the shape crosses ITSELF, the first stop projects to TWO places; the one
        // selected is determined by the distances. The selected projection was 6.8 m, while
        // the candidate at the BEGINNING of the shape was 10.0 m — the 3.1 m difference
        // made the rule call it "reversed". But end to end: the first stop is 10 m from the
        // shape start, the last stop is 9 m from the shape end, and the stops progress along
        // the shape as 0 → 19 → 24 → 93. The shape is CORRECT.
        //
        // This fixture reproduces DART's condition exactly and was MEASURED: the shape
        // travels 22 km north, returns along a parallel corridor 17 m away, then turns
        // east. The first stop is BETWEEN the corridors: 4.2 m from the return and 12.6 m
        // from the outbound path. Thus the NEAREST projection is on the second pass
        // (arc = 91% of the total path), and without the guard the rule calls it "reversed".
        // The difference is 8.4 m < 50 m → the uncertainty guard applies. Distance guard (B)
        // does NOT FIRE HERE (4.2 m < 500 m), so the test measures only A.
        //
        // ⚠️ If the stop is placed EXACTLY on a shape point or corridor, the projection
        // distance is 0, the rule is already silent, and the test measures NOTHING — the
        // first two fixtures were invalid for this reason and still passed when the guard
        // was broken.
        let mut records = records_with(
            vec![stop("A", 41.0001, 29.00015), stop("C", 41.000, 29.0500)],
            vec![route("R1", 3)],
            vec![trip_sh("T1", "R1", "S1")],
            vec![
                stoptime("T1", 1, "A", (8, 0, 0), (8, 0, 0), 2),
                stoptime("T1", 2, "C", (8, 30, 0), (8, 30, 0), 3),
            ],
        );
        records.shapes = vec![
            ShapePointRecord::new(
                shape_ti.intern("S1"),
                Some(41.000),
                Some(29.0000),
                Some(1),
                None,
                2,
            ),
            ShapePointRecord::new(
                shape_ti.intern("S1"),
                Some(41.200),
                Some(29.0000),
                Some(2),
                None,
                3,
            ),
            ShapePointRecord::new(
                shape_ti.intern("S1"),
                Some(41.200),
                Some(29.0002),
                Some(3),
                None,
                4,
            ),
            ShapePointRecord::new(
                shape_ti.intern("S1"),
                Some(41.000),
                Some(29.0002),
                Some(4),
                None,
                5,
            ),
            ShapePointRecord::new(
                shape_ti.intern("S1"),
                Some(41.000),
                Some(29.0500),
                Some(5),
                None,
                6,
            ),
        ];
        records.shape_interns = shape_ti.clone();
        let result = analyze(&records, &empty_derived(), &default_config(), 20260514);
        assert!(
            !result.notices.iter().any(|n| n.rule_id == "SHP_016"),
            "kendi uzerinden gecen shape'te yon OKUNAMAZ; SHP_016 iddia kurmamali",
        );
    }

    #[test]
    fn shp_016_stays_silent_when_the_stop_is_nowhere_near_the_shape() {
        let mut shape_ti = ShapeInternTable::new();
        use crate::k2::shapes::ShapePointRecord;
        // `tdg-83210`: the selected projection is 2.561 km from the stop. The stop's
        // coordinates are corrupt and GEO_016/SHP_014 report the actual issue; SHP_016's
        // output was derivative noise, producing 100 findings by itself.
        // This ISOLATES fixture B: the stop is beyond the END of the shape (arc = total,
        // so the "second half" condition holds), but 4 km from the stop and 12 km from the
        // BEGINNING of the shape. Since the difference is 8 km, uncertainty guard (A) does
        // NOT apply. Thus the test measures only the distance guard.
        let mut records = records_with(
            vec![stop("FAR", 41.000, 29.150), stop("B", 41.000, 29.100)],
            vec![route("R1", 3)],
            vec![trip_sh("T1", "R1", "S1")],
            vec![
                stoptime("T1", 1, "FAR", (8, 0, 0), (8, 0, 0), 2),
                stoptime("T1", 2, "B", (8, 30, 0), (8, 30, 0), 3),
            ],
        );
        records.shapes = vec![
            ShapePointRecord::new(
                shape_ti.intern("S1"),
                Some(41.000),
                Some(29.000),
                Some(1),
                None,
                2,
            ),
            ShapePointRecord::new(
                shape_ti.intern("S1"),
                Some(41.000),
                Some(29.100),
                Some(2),
                None,
                3,
            ),
        ];
        records.shape_interns = shape_ti.clone();
        let result = analyze(&records, &empty_derived(), &default_config(), 20260514);
        assert!(
            !result.notices.iter().any(|n| n.rule_id == "SHP_016"),
            "durak shape'ten cok uzakken projeksiyon yon kaniti degildir",
        );
    }

    #[test]
    fn interleaved_stop_times_rows_produce_stm_036() {
        // Two trips are interleaved in the file (T1,T2,T1,T2 row order) → each trip's rows
        // are scattered, but sequence is still INCREASING. This is the MD
        // unsorted_stop_times (INFO) equivalent: STM_036 is emitted for both trips.
        // (sequence azalmasa bile non-contiguous).
        let records = records_with(
            vec![stop("A", 41.0, 29.0), stop("B", 41.1, 29.1)],
            vec![route("R1", 3)],
            vec![trip("T1", "R1"), trip("T2", "R1")],
            vec![
                stoptime("T1", 1, "A", (8, 0, 0), (8, 0, 0), 2),
                stoptime("T2", 1, "A", (9, 0, 0), (9, 0, 0), 3),
                stoptime("T1", 2, "B", (8, 10, 0), (8, 10, 0), 4),
                stoptime("T2", 2, "B", (9, 10, 0), (9, 10, 0), 5),
            ],
        );
        let result = analyze(&records, &empty_derived(), &default_config(), 20260514);
        let stm036: Vec<_> = result
            .notices
            .iter()
            .filter(|n| n.rule_id == "STM_036")
            .collect();
        assert_eq!(
            stm036.len(),
            2,
            "iki interleaved trip için 2 STM_036 (non-contiguous) beklenir: {:?}",
            stm036.iter().map(|n| &n.message).collect::<Vec<_>>()
        );
    }

    #[test]
    fn equal_dist_diff_coords_produces_shp_028() {
        let mut shape_ti = ShapeInternTable::new();
        use crate::k2::shapes::ShapePointRecord;
        let mut records = records_with(
            vec![stop("A", 41.0, 29.0), stop("B", 41.0, 29.1)],
            vec![route("R1", 3)],
            vec![trip_sh("T1", "R1", "S1")],
            vec![
                stoptime("T1", 1, "A", (8, 0, 0), (8, 0, 0), 2),
                stoptime("T1", 2, "B", (8, 10, 0), (8, 10, 0), 3),
            ],
        );
        // Same shape_dist_traveled (100.0) but different coordinates (Δ=0.1° ≫ threshold)
        // → SHP_028
        records.shapes = vec![
            ShapePointRecord::new(
                shape_ti.intern("S1"),
                Some(41.0),
                Some(29.0),
                Some(1),
                Some(100.0),
                2,
            ),
            ShapePointRecord::new(
                shape_ti.intern("S1"),
                Some(41.0),
                Some(29.1),
                Some(2),
                Some(100.0),
                3,
            ),
        ];
        records.shape_interns = shape_ti.clone();
        let result = analyze(&records, &empty_derived(), &default_config(), 20260514);
        let ids: Vec<&str> = result.notices.iter().map(|n| n.rule_id.as_str()).collect();
        assert!(
            ids.contains(&"SHP_028"),
            "aynı dist + farklı koordinat → SHP_028: {:?}",
            ids
        );
        assert!(
            !ids.contains(&"SHP_029"),
            "eşik üstü fark SHP_029 olmamalı: {:?}",
            ids
        );
    }

    #[test]
    fn shape_with_both_below_threshold_and_repeat_fires_both_types() {
        let mut shape_ti = ShapeInternTable::new();
        use crate::k2::shapes::ShapePointRecord;
        // One shape contains both a below-threshold difference (SHP_029, FIRST) and a
        // repeated point (SHP_023, LATER). The old first-wins `break` missed SHP_023 (6
        // misses in Spelunca); now each type is reported once.
        let mut records = records_with(
            vec![stop("A", 41.0, 29.0), stop("B", 41.0, 29.1)],
            vec![route("R1", 3)],
            vec![trip_sh("T1", "R1", "S1")],
            vec![
                stoptime("T1", 1, "A", (8, 0, 0), (8, 0, 0), 2),
                stoptime("T1", 2, "B", (8, 10, 0), (8, 10, 0), 3),
            ],
        );
        records.shapes = vec![
            ShapePointRecord::new(
                shape_ti.intern("S1"),
                Some(41.0),
                Some(29.0),
                Some(1),
                Some(100.0),
                2,
            ),
            // Δlat=5e-6 (< 1e-5 threshold) → SHP_029 (below threshold), first pair
            ShapePointRecord::new(
                shape_ti.intern("S1"),
                Some(41.000005),
                Some(29.0),
                Some(2),
                Some(100.0),
                3,
            ),
            // Exactly the same as point 2 → SHP_023 (repeated point), second pair
            ShapePointRecord::new(
                shape_ti.intern("S1"),
                Some(41.000005),
                Some(29.0),
                Some(3),
                Some(100.0),
                4,
            ),
        ];
        records.shape_interns = shape_ti.clone();
        let result = analyze(&records, &empty_derived(), &default_config(), 20260514);
        let ids: Vec<&str> = result.notices.iter().map(|n| n.rule_id.as_str()).collect();
        assert!(
            ids.contains(&"SHP_029"),
            "eşik-altı fark → SHP_029: {:?}",
            ids
        );
        assert!(
            ids.contains(&"SHP_023"),
            "tekrar-nokta → SHP_023 (first-wins bug fix): {:?}",
            ids
        );
        assert_eq!(
            ids.iter().filter(|&&x| x == "SHP_023").count(),
            1,
            "SHP_023 shape başına tek"
        );
        assert_eq!(
            ids.iter().filter(|&&x| x == "SHP_029").count(),
            1,
            "SHP_029 shape başına tek"
        );
    }

    #[test]
    fn equal_dist_tiny_coord_diff_produces_shp_029() {
        let mut shape_ti = ShapeInternTable::new();
        use crate::k2::shapes::ShapePointRecord;
        let mut records = records_with(
            vec![stop("A", 41.0, 29.0), stop("B", 41.0, 29.1)],
            vec![route("R1", 3)],
            vec![trip_sh("T1", "R1", "S1")],
            vec![
                stoptime("T1", 1, "A", (8, 0, 0), (8, 0, 0), 2),
                stoptime("T1", 2, "B", (8, 10, 0), (8, 10, 0), 3),
            ],
        );
        // Same distance, coordinate difference below threshold (Δ=1e-6° < 1e-5°) → SHP_029
        records.shapes = vec![
            ShapePointRecord::new(
                shape_ti.intern("S1"),
                Some(41.0),
                Some(29.0),
                Some(1),
                Some(100.0),
                2,
            ),
            ShapePointRecord::new(
                shape_ti.intern("S1"),
                Some(41.0),
                Some(29.000001),
                Some(2),
                Some(100.0),
                3,
            ),
        ];
        records.shape_interns = shape_ti.clone();
        let result = analyze(&records, &empty_derived(), &default_config(), 20260514);
        let ids: Vec<&str> = result.notices.iter().map(|n| n.rule_id.as_str()).collect();
        assert!(
            ids.contains(&"SHP_029"),
            "aynı dist + minik koordinat farkı → SHP_029: {:?}",
            ids
        );
        assert!(
            !ids.contains(&"SHP_028"),
            "eşik altı fark SHP_028 olmamalı: {:?}",
            ids
        );
    }

    #[test]
    fn stop_near_pole_produces_geo_022() {
        let records = records_with(
            vec![stop("A", 89.5, 10.0), stop("B", 41.0, 29.0)],
            vec![route("R1", 3)],
            vec![trip("T1", "R1")],
            vec![
                stoptime("T1", 1, "A", (8, 0, 0), (8, 0, 0), 2),
                stoptime("T1", 2, "B", (8, 10, 0), (8, 10, 0), 3),
            ],
        );
        let result = analyze(&records, &empty_derived(), &default_config(), 20260514);
        let ids: Vec<&str> = result.notices.iter().map(|n| n.rule_id.as_str()).collect();
        assert!(
            ids.contains(&"GEO_022"),
            "kutba yakın durak (lat 89.5) → GEO_022: {:?}",
            ids
        );
    }

    // ── GEO_002: median reference + Null Island guard ────────────────────
    #[test]
    fn geo_002_carries_median_in_details() {
        // 3 stops are in Istanbul and 1 stop is ~2,000 km away → the distant stop produces
        // GEO_002, and the notice must carry the median coordinates so the map can show
        // "far from what".
        let records = records_with(
            vec![
                stop("A", 41.0, 29.0),
                stop("B", 41.1, 29.1),
                stop("C", 41.2, 29.2),
                stop("FAR", 52.5, 13.4),
            ],
            vec![route("R1", 3)],
            vec![trip("T1", "R1")],
            vec![
                stoptime("T1", 1, "A", (8, 0, 0), (8, 0, 0), 2),
                stoptime("T1", 2, "B", (8, 10, 0), (8, 10, 0), 3),
            ],
        );
        let result = analyze(&records, &empty_derived(), &default_config(), 20260514);
        let n = result
            .notices
            .iter()
            .find(|n| n.rule_id == "GEO_002")
            .expect("uzak durak GEO_002 üretmeli");
        assert_eq!(n.entity_id.as_deref(), Some("FAR"));
        let d = n.details.as_ref().expect("details olmalı");
        assert!(
            d.contains_key("med_lat") && d.contains_key("med_lon"),
            "median details'ta olmalı: {d:?}"
        );
        assert!(d
            .get("dist_km")
            .is_some_and(|v| v.parse::<f64>().unwrap_or(0.0) > 200.0));
    }

    #[test]
    fn geo_002_threshold_is_relative_to_feed_extent() {
        // VBB measurement model: a regional/cross-border feed with stops spread over ~280 km.
        // A fixed 200 km threshold treated 20 REAL stations in this feed as errors; the
        // relative threshold (3 × p99) should remain silent for this distribution.
        let mut stops: Vec<_> = (0..120)
            .map(|i| stop(&format!("S{i}"), 52.5 + (i as f64) * 0.02, 13.4))
            .collect();
        stops.push(stop("FAR", 55.0, 13.4)); // ~278 km — corridor endpoint, real station
        let records = records_with(stops, vec![route("R1", 3)], vec![trip("T1", "R1")], vec![]);
        let result = analyze(&records, &empty_derived(), &default_config(), 20260514);
        assert!(
            !result.notices.iter().any(|n| n.rule_id == "GEO_002"),
            "geniş yayılımlı feed'de koridor ucundaki durak GEO_002 üretmemeli: {:?}",
            result
                .notices
                .iter()
                .filter(|n| n.rule_id == "GEO_002")
                .map(|n| (&n.entity_id, &n.observed_value))
                .collect::<Vec<_>>()
        );
    }

    #[test]
    fn geo_002_still_fires_for_real_outlier_in_compact_feed() {
        // Compact feed (~5 km) + one outlier stop (~2,000 km) → genuine coordinate error;
        // the relative-threshold floor drops to 200 km and the rule continues to fire.
        let mut stops: Vec<_> = (0..120)
            .map(|i| stop(&format!("S{i}"), 41.0 + (i as f64) * 0.0004, 29.0))
            .collect();
        stops.push(stop("BROKEN", 52.5, 13.4));
        let records = records_with(stops, vec![route("R1", 3)], vec![trip("T1", "R1")], vec![]);
        let result = analyze(&records, &empty_derived(), &default_config(), 20260514);
        let n = result
            .notices
            .iter()
            .find(|n| n.rule_id == "GEO_002")
            .expect("kompakt feed'deki aykırı durak GEO_002 üretmeli");
        assert_eq!(n.entity_id.as_deref(), Some("BROKEN"));
    }

    #[test]
    fn geo_002_skips_null_island_stop() {
        // The (0,0) placeholder is inevitably >200 km from the median; GEO_016 is the root
        // cause → GEO_002 must not double-emit (consistent with STP_016/017 and GEO_012).
        let records = records_with(
            vec![
                stop("A", 41.0, 29.0),
                stop("B", 41.1, 29.1),
                stop("C", 41.2, 29.2),
                stop("NULL", 0.0, 0.0),
            ],
            vec![route("R1", 3)],
            vec![trip("T1", "R1")],
            vec![
                stoptime("T1", 1, "A", (8, 0, 0), (8, 0, 0), 2),
                stoptime("T1", 2, "B", (8, 10, 0), (8, 10, 0), 3),
            ],
        );
        let result = analyze(&records, &empty_derived(), &default_config(), 20260514);
        assert!(
            !result.notices.iter().any(|n| n.rule_id == "GEO_002"),
            "Null Island durağı GEO_002 üretmemeli: {:?}",
            result
                .notices
                .iter()
                .map(|n| (&n.rule_id, &n.entity_id))
                .collect::<Vec<_>>()
        );
        assert!(
            result.notices.iter().any(|n| n.rule_id == "GEO_016"),
            "GEO_016 üretilmeli"
        );
    }

    #[test]
    fn bidirectional_route_single_shape_produces_opr_015() {
        let mut t0 = trip_sh("T0", "R1", "S1");
        t0.direction_id = Some(0);
        let mut t1 = trip_sh("T1", "R1", "S1");
        t1.direction_id = Some(1);
        let records = records_with(
            vec![stop("A", 41.0, 29.0), stop("B", 41.1, 29.1)],
            vec![route("R1", 3)],
            vec![t0, t1],
            vec![
                stoptime("T0", 1, "A", (8, 0, 0), (8, 0, 0), 2),
                stoptime("T0", 2, "B", (8, 10, 0), (8, 10, 0), 3),
                stoptime("T1", 1, "B", (9, 0, 0), (9, 0, 0), 2),
                stoptime("T1", 2, "A", (9, 10, 0), (9, 10, 0), 3),
            ],
        );
        let result = analyze(&records, &empty_derived(), &default_config(), 20260514);
        assert!(
            result.notices.iter().any(|n| n.rule_id == "OPR_015"),
            "çift yönlü + tek shape → OPR_015"
        );
    }

    #[test]
    fn unidirectional_route_single_shape_no_opr_015() {
        // Both trips have direction_id=0 → one-way; one shape is expected → no OPR_015
        let mut t0 = trip_sh("T0", "R1", "S1");
        t0.direction_id = Some(0);
        let mut t1 = trip_sh("T1", "R1", "S1");
        t1.direction_id = Some(0);
        let records = records_with(
            vec![stop("A", 41.0, 29.0), stop("B", 41.1, 29.1)],
            vec![route("R1", 3)],
            vec![t0, t1],
            vec![
                stoptime("T0", 1, "A", (8, 0, 0), (8, 0, 0), 2),
                stoptime("T0", 2, "B", (8, 10, 0), (8, 10, 0), 3),
                stoptime("T1", 1, "A", (9, 0, 0), (9, 0, 0), 2),
                stoptime("T1", 2, "B", (9, 10, 0), (9, 10, 0), 3),
            ],
        );
        let result = analyze(&records, &empty_derived(), &default_config(), 20260514);
        assert!(
            !result.notices.iter().any(|n| n.rule_id == "OPR_015"),
            "tek yönlü + tek shape → OPR_015 olmamalı"
        );
    }

    #[test]
    fn shape_reused_in_both_directions_no_shp_016() {
        let mut shape_ti = ShapeInternTable::new();
        use crate::k2::shapes::ShapePointRecord;
        // S1: A(29.0) → B(29.1). T1 goes forward (A→B), while T2 uses the same shape in
        // reverse (B→A). Because one forward variant exists, the shape must not be treated
        // as "reversed" (variant-aware minimum).
        let mut records = records_with(
            vec![stop("A", 41.0, 29.0), stop("B", 41.0, 29.1)],
            vec![route("R1", 3)],
            vec![trip_sh("T1", "R1", "S1"), trip_sh("T2", "R1", "S1")],
            vec![
                stoptime("T1", 1, "A", (8, 0, 0), (8, 0, 0), 2),
                stoptime("T1", 2, "B", (8, 10, 0), (8, 10, 0), 3),
                stoptime("T2", 1, "B", (9, 0, 0), (9, 0, 0), 4),
                stoptime("T2", 2, "A", (9, 10, 0), (9, 10, 0), 5),
            ],
        );
        records.shapes = vec![
            ShapePointRecord::new(
                shape_ti.intern("S1"),
                Some(41.0),
                Some(29.0),
                Some(1),
                None,
                2,
            ),
            ShapePointRecord::new(
                shape_ti.intern("S1"),
                Some(41.0),
                Some(29.1),
                Some(2),
                None,
                3,
            ),
        ];
        records.shape_interns = shape_ti.clone();
        let result = analyze(&records, &empty_derived(), &default_config(), 20260514);
        assert!(
            !result.notices.iter().any(|n| n.rule_id == "SHP_016"),
            "ileri varyant varken shape ters sayılmamalı (SHP_016 yok)"
        );
    }

    #[test]
    fn shape_end_matches_one_variant_no_shp_014() {
        let mut shape_ti = ShapeInternTable::new();
        use crate::k2::shapes::ShapePointRecord;
        // S1: A(29.0) → B(29.1). T1 is complete (A→B, last stop B = shape end).
        // T2 is a short variant (A→FAR, last stop far from the shape end).
        // The nearest variant (T1) matches the shape end, so SHP_014 must not fire.
        let mut records = records_with(
            vec![
                stop("A", 41.0, 29.0),
                stop("B", 41.0, 29.1),
                stop("FAR", 42.0, 30.0),
            ],
            vec![route("R1", 3)],
            vec![trip_sh("T1", "R1", "S1"), trip_sh("T2", "R1", "S1")],
            vec![
                stoptime("T1", 1, "A", (8, 0, 0), (8, 0, 0), 2),
                stoptime("T1", 2, "B", (8, 10, 0), (8, 10, 0), 3),
                stoptime("T2", 1, "A", (9, 0, 0), (9, 0, 0), 4),
                stoptime("T2", 2, "FAR", (9, 10, 0), (9, 10, 0), 5),
            ],
        );
        records.shapes = vec![
            ShapePointRecord::new(
                shape_ti.intern("S1"),
                Some(41.0),
                Some(29.0),
                Some(1),
                None,
                2,
            ),
            ShapePointRecord::new(
                shape_ti.intern("S1"),
                Some(41.0),
                Some(29.1),
                Some(2),
                None,
                3,
            ),
        ];
        records.shape_interns = shape_ti.clone();
        let result = analyze(&records, &empty_derived(), &default_config(), 20260514);
        assert!(
            !result.notices.iter().any(|n| n.rule_id == "SHP_014"),
            "shape sonu bir varyantın son durağıyla eşleşiyorsa SHP_014 yok"
        );
    }

    #[test]
    fn shape_end_far_from_all_variants_produces_shp_014() {
        let mut shape_ti = ShapeInternTable::new();
        use crate::k2::shapes::ShapePointRecord;
        // S1: A(29.0) → B(29.1). The only trip, T1, ends at C, far from the shape end.
        // No variant is near the shape end → SHP_014 must fire (regression guard).
        let mut records = records_with(
            vec![stop("A", 41.0, 29.0), stop("C", 42.0, 30.0)],
            vec![route("R1", 3)],
            vec![trip_sh("T1", "R1", "S1")],
            vec![
                stoptime("T1", 1, "A", (8, 0, 0), (8, 0, 0), 2),
                stoptime("T1", 2, "C", (8, 10, 0), (8, 10, 0), 3),
            ],
        );
        records.shapes = vec![
            ShapePointRecord::new(
                shape_ti.intern("S1"),
                Some(41.0),
                Some(29.0),
                Some(1),
                None,
                2,
            ),
            ShapePointRecord::new(
                shape_ti.intern("S1"),
                Some(41.0),
                Some(29.1),
                Some(2),
                None,
                3,
            ),
        ];
        records.shape_interns = shape_ti.clone();
        let result = analyze(&records, &empty_derived(), &default_config(), 20260514);
        assert!(
            result.notices.iter().any(|n| n.rule_id == "SHP_014"),
            "shape sonu hiçbir varyantın son durağına yakın değil → SHP_014"
        );
    }

    #[test]
    fn shp_014_tied_variants_report_smallest_trip_id() {
        let mut shape_ti = ShapeInternTable::new();
        use crate::k2::shapes::ShapePointRecord;
        // Eight trips use the same A→C sequence: their distances from the shape end are
        // EQUAL. If the representative trip is left to hash order, wasm32 (32-bit FxHash)
        // and native/wasm64 report different trip_ids; ties must choose the smallest
        // trip_id.
        let ids = ["T9", "T4", "T7", "T1", "T8", "T3", "T6", "T2"];
        let mut stop_times = Vec::new();
        for (i, id) in ids.iter().enumerate() {
            let line = 2 + 2 * i as u64;
            stop_times.push(stoptime(id, 1, "A", (8, 0, 0), (8, 0, 0), line));
            stop_times.push(stoptime(id, 2, "C", (8, 10, 0), (8, 10, 0), line + 1));
        }
        let mut records = records_with(
            vec![stop("A", 41.0, 29.0), stop("C", 42.0, 30.0)],
            vec![route("R1", 3)],
            ids.iter().map(|id| trip_sh(id, "R1", "S1")).collect(),
            stop_times,
        );
        records.shapes = vec![
            ShapePointRecord::new(
                shape_ti.intern("S1"),
                Some(41.0),
                Some(29.0),
                Some(1),
                None,
                2,
            ),
            ShapePointRecord::new(
                shape_ti.intern("S1"),
                Some(41.0),
                Some(29.1),
                Some(2),
                None,
                3,
            ),
        ];
        records.shape_interns = shape_ti.clone();
        let result = analyze(&records, &empty_derived(), &default_config(), 20260514);
        let trip = result
            .notices
            .iter()
            .find(|n| n.rule_id == "SHP_014")
            .and_then(|n| n.details.as_ref())
            .and_then(|d| d.get("trip_id"))
            .cloned();
        assert_eq!(trip.as_deref(), Some("T1"));
    }

    #[test]
    fn partial_shape_order_violation_produces_shp_017() {
        let mut shape_ti = ShapeInternTable::new();
        use crate::k2::shapes::ShapePointRecord;
        // Shape: A(29.0) → B(29.05) → C(29.1) — three-point line
        // stop_times: A(seq=1) → C(seq=2) → B(seq=3) → B follows C in stop_times but comes first on the shape
        // First stop A is at the shape start → SHP_016 does not fire; B is out of order → SHP_017
        let mut records = records_with(
            vec![
                stop("A", 41.0, 29.0),
                stop("B", 41.0, 29.05),
                stop("C", 41.0, 29.1),
            ],
            vec![route("R1", 3)],
            vec![trip_sh("T1", "R1", "S1")],
            vec![
                stoptime("T1", 1, "A", (8, 0, 0), (8, 0, 0), 2),
                stoptime("T1", 2, "C", (8, 10, 0), (8, 10, 0), 3),
                stoptime("T1", 3, "B", (8, 20, 0), (8, 20, 0), 4),
            ],
        );
        records.shapes = vec![
            ShapePointRecord::new(
                shape_ti.intern("S1"),
                Some(41.0),
                Some(29.0),
                Some(1),
                None,
                2,
            ),
            ShapePointRecord::new(
                shape_ti.intern("S1"),
                Some(41.0),
                Some(29.05),
                Some(2),
                None,
                3,
            ),
            ShapePointRecord::new(
                shape_ti.intern("S1"),
                Some(41.0),
                Some(29.1),
                Some(3),
                None,
                4,
            ),
        ];
        records.shape_interns = shape_ti.clone();
        let result = analyze(&records, &empty_derived(), &default_config(), 20260514);
        let n017 = result
            .notices
            .iter()
            .find(|n| n.rule_id == "SHP_017")
            .expect("kısmi sıra ihlali → SHP_017 olmalı");
        // scope_key is the shape (the registry identity field): shape-scoped roots
        // (XFL_013, SHP_002/006, TRP_004) can suppress it only this way.
        assert_eq!(n017.entity_id.as_deref(), Some("T1"));
        assert_eq!(n017.scope_key.as_deref(), Some("S1"));
    }

    #[test]
    fn shp_017_representative_trip_is_iteration_independent() {
        // Emit one notice for trips sharing the same (shape, stop) violation; the representative
        // must depend on trip_id order, not HashMap order (WASM/native parity).
        let mut shape_ti = ShapeInternTable::new();
        use crate::k2::shapes::ShapePointRecord;
        let ids = ["T7", "T3", "T9", "T1", "T5", "T8", "T2", "T6", "T4"];
        let mut stop_times = Vec::new();
        for (k, id) in ids.iter().enumerate() {
            let h = 6 + k as u32;
            stop_times.push(stoptime(id, 1, "A", (h, 0, 0), (h, 0, 0), 2));
            stop_times.push(stoptime(id, 2, "C", (h, 10, 0), (h, 10, 0), 3));
            stop_times.push(stoptime(id, 3, "B", (h, 20, 0), (h, 20, 0), 4));
        }
        let mut records = records_with(
            vec![
                stop("A", 41.0, 29.0),
                stop("B", 41.0, 29.05),
                stop("C", 41.0, 29.1),
            ],
            vec![route("R1", 3)],
            ids.iter().map(|id| trip_sh(id, "R1", "S1")).collect(),
            stop_times,
        );
        records.shapes = [29.0, 29.05, 29.1]
            .iter()
            .enumerate()
            .map(|(i, &lon)| {
                ShapePointRecord::new(
                    shape_ti.intern("S1"),
                    Some(41.0),
                    Some(lon),
                    Some(i as u32 + 1),
                    None,
                    i as u32 + 2,
                )
            })
            .collect();
        records.shape_interns = shape_ti.clone();
        let result = analyze(&records, &empty_derived(), &default_config(), 20260514);
        let reps: Vec<_> = result
            .notices
            .iter()
            .filter(|n| n.rule_id == "SHP_017")
            .map(|n| n.entity_id.as_deref())
            .collect();
        assert_eq!(reps, vec![Some("T1")]);
    }

    #[test]
    fn backtrack_that_recovers_no_shp_017() {
        let mut shape_ti = ShapeInternTable::new();
        use crate::k2::shapes::ShapePointRecord;
        // Backtrack recovery filter (#17): straight eastbound shape 29.00 → 29.10 (lat 41.0).
        // stop_times: A(29.00) → B(29.03) → C(29.015: B'nin ~1.3km gerisi, >500m tolerans) → D(29.05).
        // C jumps backward, BUT D exceeds the previous maximum (B) within 3 stops → recovery → NO SHP_017.
        let mut records = records_with(
            vec![
                stop("A", 41.0, 29.00),
                stop("B", 41.0, 29.03),
                stop("C", 41.0, 29.015),
                stop("D", 41.0, 29.05),
            ],
            vec![route("R1", 3)],
            vec![trip_sh("T1", "R1", "S1")],
            vec![
                stoptime("T1", 1, "A", (8, 0, 0), (8, 0, 0), 2),
                stoptime("T1", 2, "B", (8, 10, 0), (8, 10, 0), 3),
                stoptime("T1", 3, "C", (8, 20, 0), (8, 20, 0), 4),
                stoptime("T1", 4, "D", (8, 30, 0), (8, 30, 0), 5),
            ],
        );
        records.shapes = vec![
            ShapePointRecord::new(
                shape_ti.intern("S1"),
                Some(41.0),
                Some(29.00),
                Some(1),
                None,
                2,
            ),
            ShapePointRecord::new(
                shape_ti.intern("S1"),
                Some(41.0),
                Some(29.05),
                Some(2),
                None,
                3,
            ),
            ShapePointRecord::new(
                shape_ti.intern("S1"),
                Some(41.0),
                Some(29.10),
                Some(3),
                None,
                4,
            ),
        ];
        records.shape_interns = shape_ti.clone();
        let result = analyze(&records, &empty_derived(), &default_config(), 20260514);
        assert!(!result.notices.iter().any(|n| n.rule_id == "SHP_017"),
            "geri sıçrayıp 3 durak içinde toparlanan trip SHP_017 üretmemeli (backtrack kurtarma filtresi)");
    }

    // ── STP_029: stop far from parent_station ────────────────────────────────

    #[test]
    fn stop_far_from_parent_produces_stp_029() {
        let mut parent = stop("PS1", 41.0, 29.0);
        parent.location_type = Some(1);
        let mut child = stop("S1", 41.1, 29.1); // ~15 km away — threshold exceeded
        child
            .row
            .insert("parent_station".to_string(), "PS1".to_string());
        let records = records_with(
            vec![parent, child],
            vec![route("R1", 3)],
            vec![trip("T1", "R1")],
            vec![stoptime("T1", 1, "S1", (8, 0, 0), (8, 0, 0), 2)],
        );
        let result = analyze(&records, &empty_derived(), &default_config(), 20260514);
        assert!(
            result
                .notices
                .iter()
                .any(|n| n.rule_id == "STP_029" && n.entity_id.as_deref() == Some("S1")),
            "STP_029 olmalı: {:?}",
            result
                .notices
                .iter()
                .map(|n| &n.rule_id)
                .collect::<Vec<_>>()
        );
    }

    #[test]
    fn stop_close_to_parent_no_stp_029() {
        let mut parent = stop("PS1", 41.0, 29.0);
        parent.location_type = Some(1);
        let mut child = stop("S1", 41.0001, 29.0001); // ~15 m — below threshold
        child
            .row
            .insert("parent_station".to_string(), "PS1".to_string());
        let records = records_with(
            vec![parent, child],
            vec![route("R1", 3)],
            vec![trip("T1", "R1")],
            vec![stoptime("T1", 1, "S1", (8, 0, 0), (8, 0, 0), 2)],
        );
        let result = analyze(&records, &empty_derived(), &default_config(), 20260514);
        assert!(
            !result.notices.iter().any(|n| n.rule_id == "STP_029"),
            "yakın durak STP_029 üretmemeli"
        );
    }

    // ── STP_030: parent station with no children ────────────────────────────

    /// A station with only ENTRANCES (no platforms) is also STP_030: service stops at the
    /// platform, not the entrance. Corpus evidence (mdb-2933): all 6 children of `MTR-POA`
    /// have location_type=2 (entrance); we stayed silent while MD flagged `unused_station`.
    #[test]
    fn station_with_only_entrances_produces_stp_030() {
        let mut station = stop("PS1", 41.0, 29.0);
        station.location_type = Some(1);
        let mut entrance = stop("E1", 41.0, 29.0);
        entrance.location_type = Some(2); // entrance/exit — NOT a platform
        entrance
            .row
            .insert("parent_station".to_string(), "PS1".to_string());
        let records = records_with(
            vec![station, entrance, stop("A", 41.0, 29.0)],
            vec![route("R1", 3)],
            vec![trip("T1", "R1")],
            vec![stoptime("T1", 1, "A", (8, 0, 0), (8, 0, 0), 2)],
        );
        let result = analyze(&records, &empty_derived(), &default_config(), 20260514);
        assert!(
            result
                .notices
                .iter()
                .any(|n| n.rule_id == "STP_030" && n.entity_id.as_deref() == Some("PS1")),
            "yalnız girişi olan istasyon STP_030 üretmeli (peron yok)"
        );
    }

    #[test]
    fn station_without_children_produces_stp_030() {
        let mut station = stop("PS1", 41.0, 29.0);
        station.location_type = Some(1);
        let records = records_with(
            vec![station, stop("A", 41.0, 29.0)],
            vec![route("R1", 3)],
            vec![trip("T1", "R1")],
            vec![stoptime("T1", 1, "A", (8, 0, 0), (8, 0, 0), 2)],
        );
        let result = analyze(&records, &empty_derived(), &default_config(), 20260514);
        assert!(
            result
                .notices
                .iter()
                .any(|n| n.rule_id == "STP_030" && n.entity_id.as_deref() == Some("PS1")),
            "STP_030 olmalı"
        );
    }

    #[test]
    fn station_with_children_no_stp_030() {
        let mut station = stop("PS1", 41.0, 29.0);
        station.location_type = Some(1);
        let mut child = stop("S1", 41.0, 29.0);
        child
            .row
            .insert("parent_station".to_string(), "PS1".to_string());
        let records = records_with(
            vec![station, child],
            vec![route("R1", 3)],
            vec![trip("T1", "R1")],
            vec![stoptime("T1", 1, "S1", (8, 0, 0), (8, 0, 0), 2)],
        );
        let result = analyze(&records, &empty_derived(), &default_config(), 20260514);
        assert!(
            !result.notices.iter().any(|n| n.rule_id == "STP_030"),
            "alt durağı olan istasyon STP_030 üretmemeli"
        );
    }

    // ── TRP_020: headsign matches an intermediate stop name ────────────────

    /// If the headsign matches the FINAL STOP'S name, the direction name is CORRECT even
    /// if the same name also occurs at an intermediate stop → TRP_020 must not be emitted.
    ///
    /// The old code excluded the terminal by `stop_id` but matched by NAME; a different
    /// stop_id with the same name (separate platform/direction record, NO shared parent)
    /// passed the filter. Corpus evidence (mdb-1294, Rome): 9,162 trips were flagged, MD 19.
    #[test]
    fn headsign_matching_terminal_name_is_not_trp_020_even_if_name_repeats() {
        let mut a = stop("A", 41.0, 29.0);
        a.stop_name = Some("Başlangıç".into());
        // Intermediate stop and terminal have the SAME NAME, DIFFERENT stop_ids, and NO parent.
        let mut mid = stop("M", 41.05, 29.05);
        mid.stop_name = Some("Cimitero".into());
        let mut term = stop("T", 41.1, 29.1);
        term.stop_name = Some("Cimitero".into());
        let records = records_with(
            vec![a, mid, term],
            vec![route("R1", 3)],
            vec![trip_hs("T1", "R1", "Cimitero")], // headsign = FINAL stop name → correct
            vec![
                stoptime("T1", 1, "A", (8, 0, 0), (8, 0, 0), 2),
                stoptime("T1", 2, "M", (8, 5, 0), (8, 5, 0), 3),
                stoptime("T1", 3, "T", (8, 10, 0), (8, 10, 0), 4),
            ],
        );
        let result = analyze(&records, &empty_derived(), &default_config(), 20260514);
        assert!(
            !result.notices.iter().any(|n| n.rule_id == "TRP_020"),
            "headsign son durağın adıyla eşleşiyor → yön adı doğru, TRP_020 olmamalı"
        );
    }

    #[test]
    fn headsign_matches_intermediate_stop_produces_trp_020() {
        // Stop A (seq 1, ara), Stop B (seq 2, terminal)
        // headsign = "Stop A" (intermediate stop name) → TRP_020 should fire
        let mut sa = stop("A", 41.0, 29.0);
        sa.stop_name = Some("Stop A".into());
        let mut sb = stop("B", 41.1, 29.1);
        sb.stop_name = Some("Stop B".into());
        let t = trip_hs("T1", "R1", "Stop A"); // intermediate stop name
        let records = records_with(
            vec![sa, sb],
            vec![route("R1", 3)],
            vec![t],
            vec![
                stoptime("T1", 1, "A", (8, 0, 0), (8, 0, 0), 2),
                stoptime("T1", 2, "B", (8, 10, 0), (8, 10, 0), 3),
            ],
        );
        let result = analyze(&records, &empty_derived(), &default_config(), 20260514);
        assert!(
            result.notices.iter().any(|n| n.rule_id == "TRP_020"),
            "TRP_020 olmalı: {:?}",
            result
                .notices
                .iter()
                .map(|n| &n.rule_id)
                .collect::<Vec<_>>()
        );
    }

    #[test]
    fn headsign_matches_terminal_stop_no_trp_020() {
        // headsign = terminal stop name → TRP_020 must not fire
        let mut sa = stop("A", 41.0, 29.0);
        sa.stop_name = Some("Stop A".into());
        let mut sb = stop("B", 41.1, 29.1);
        sb.stop_name = Some("Stop B".into());
        let t = trip_hs("T1", "R1", "Stop B"); // terminal stop name
        let records = records_with(
            vec![sa, sb],
            vec![route("R1", 3)],
            vec![t],
            vec![
                stoptime("T1", 1, "A", (8, 0, 0), (8, 0, 0), 2),
                stoptime("T1", 2, "B", (8, 10, 0), (8, 10, 0), 3),
            ],
        );
        let result = analyze(&records, &empty_derived(), &default_config(), 20260514);
        assert!(
            !result.notices.iter().any(|n| n.rule_id == "TRP_020"),
            "terminal eşleşmede TRP_020 olmamalı"
        );
    }

    #[test]
    fn headsign_no_stop_match_no_trp_020() {
        // headsign matches no stop name → TRP_020 must not fire
        let mut sa = stop("A", 41.0, 29.0);
        sa.stop_name = Some("Stop A".into());
        let mut sb = stop("B", 41.1, 29.1);
        sb.stop_name = Some("Stop B".into());
        let t = trip_hs("T1", "R1", "Tren Garı");
        let records = records_with(
            vec![sa, sb],
            vec![route("R1", 3)],
            vec![t],
            vec![
                stoptime("T1", 1, "A", (8, 0, 0), (8, 0, 0), 2),
                stoptime("T1", 2, "B", (8, 10, 0), (8, 10, 0), 3),
            ],
        );
        let result = analyze(&records, &empty_derived(), &default_config(), 20260514);
        assert!(
            !result.notices.iter().any(|n| n.rule_id == "TRP_020"),
            "eşleşme yoksa TRP_020 olmamalı"
        );
    }

    #[test]
    fn headsign_matches_repeated_turnaround_stop_no_trp_020() {
        // Trip: A → B → C → B (repeat) → D. headsign = "Stop B" = REPEATED stop.
        // B is a terminal/turnback point (like an airport wye) → legitimate headsign → TRP_020 must not fire.
        let mut sa = stop("A", 41.0, 29.0);
        sa.stop_name = Some("Stop A".into());
        let mut sb = stop("B", 41.1, 29.1);
        sb.stop_name = Some("Stop B".into());
        let mut sc = stop("C", 41.2, 29.2);
        sc.stop_name = Some("Stop C".into());
        let mut sd = stop("D", 41.3, 29.3);
        sd.stop_name = Some("Stop D".into());
        let t = trip_hs("T1", "R1", "Stop B");
        let records = records_with(
            vec![sa, sb, sc, sd],
            vec![route("R1", 3)],
            vec![t],
            vec![
                stoptime("T1", 1, "A", (8, 0, 0), (8, 0, 0), 2),
                stoptime("T1", 2, "B", (8, 10, 0), (8, 10, 0), 3),
                stoptime("T1", 3, "C", (8, 20, 0), (8, 20, 0), 4),
                stoptime("T1", 4, "B", (8, 30, 0), (8, 30, 0), 5),
                stoptime("T1", 5, "D", (8, 40, 0), (8, 40, 0), 6),
            ],
        );
        let result = analyze(&records, &empty_derived(), &default_config(), 20260514);
        assert!(
            !result.notices.iter().any(|n| n.rule_id == "TRP_020"),
            "headsign tekrar eden uç noktaya eşleşiyor → TRP_020 olmamalı"
        );
    }

    #[test]
    fn headsign_intermediate_with_unrelated_repeat_produces_trp_020() {
        // Trip: A → B → C → B (repeat) → D. headsign = "Stop C" = a SINGLE-PASSING intermediate stop.
        // B's repetition is IRRELEVANT; headsign C is a legitimate intermediate-stop match occurring once.
        // The old "any repeat → loop" logic would incorrectly suppress this (false negative).
        let mut sa = stop("A", 41.0, 29.0);
        sa.stop_name = Some("Stop A".into());
        let mut sb = stop("B", 41.1, 29.1);
        sb.stop_name = Some("Stop B".into());
        let mut sc = stop("C", 41.2, 29.2);
        sc.stop_name = Some("Stop C".into());
        let mut sd = stop("D", 41.3, 29.3);
        sd.stop_name = Some("Stop D".into());
        let t = trip_hs("T1", "R1", "Stop C");
        let records = records_with(
            vec![sa, sb, sc, sd],
            vec![route("R1", 3)],
            vec![t],
            vec![
                stoptime("T1", 1, "A", (8, 0, 0), (8, 0, 0), 2),
                stoptime("T1", 2, "B", (8, 10, 0), (8, 10, 0), 3),
                stoptime("T1", 3, "C", (8, 20, 0), (8, 20, 0), 4),
                stoptime("T1", 4, "B", (8, 30, 0), (8, 30, 0), 5),
                stoptime("T1", 5, "D", (8, 40, 0), (8, 40, 0), 6),
            ],
        );
        let result = analyze(&records, &empty_derived(), &default_config(), 20260514);
        assert!(
            result.notices.iter().any(|n| n.rule_id == "TRP_020"),
            "alakasız tekrar headsign'ın eşleştiği ara durağı bastırmamalı: {:?}",
            result
                .notices
                .iter()
                .map(|n| &n.rule_id)
                .collect::<Vec<_>>()
        );
    }

    #[test]
    fn headsign_intermediate_consecutive_duplicate_produces_trp_020() {
        // #29: Trip A → B → B → C; B has the same stop_id twice IN A ROW (dwell/timepoint
        // duplication, BART SFO seq 26–27 pattern), terminal = C. headsign = "Stop B" is an
        // intermediate stop. An adjacent repeat is NOT a real terminal/turnback → TRP_020
        // MUST fire (the previous logic incorrectly suppressed it).
        let mut sa = stop("A", 41.0, 29.0);
        sa.stop_name = Some("Stop A".into());
        let mut sb = stop("B", 41.1, 29.1);
        sb.stop_name = Some("Stop B".into());
        let mut sc = stop("C", 41.2, 29.2);
        sc.stop_name = Some("Stop C".into());
        let t = trip_hs("T1", "R1", "Stop B");
        let records = records_with(
            vec![sa, sb, sc],
            vec![route("R1", 3)],
            vec![t],
            vec![
                stoptime("T1", 1, "A", (8, 0, 0), (8, 0, 0), 2),
                stoptime("T1", 2, "B", (8, 10, 0), (8, 10, 0), 3),
                stoptime("T1", 3, "B", (8, 11, 0), (8, 11, 0), 4),
                stoptime("T1", 4, "C", (8, 20, 0), (8, 20, 0), 5),
            ],
        );
        let result = analyze(&records, &empty_derived(), &default_config(), 20260514);
        assert!(
            result.notices.iter().any(|n| n.rule_id == "TRP_020"),
            "ardışık yinelenen ara durak (dwell) bastırılmamalı → TRP_020 çıkmalı: {:?}",
            result
                .notices
                .iter()
                .map(|n| &n.rule_id)
                .collect::<Vec<_>>()
        );
    }

    #[test]
    fn headsign_loop_via_parent_station_no_trp_020() {
        // Loop: A1 → B → A2; A1 and A2 have different stop_ids but the same parent_station P.
        // first(A1) != terminal(A2) (stop_id), but the effective station is the same (P) → TRP_020 must not fire.
        let mut sa1 = stop("A1", 41.0, 29.0);
        sa1.stop_name = Some("İstasyon P".into());
        sa1.row.insert("parent_station".into(), "P".into());
        let mut sa2 = stop("A2", 41.0, 29.0);
        sa2.stop_name = Some("İstasyon P".into());
        sa2.row.insert("parent_station".into(), "P".into());
        let mut sb = stop("B", 41.1, 29.1);
        sb.stop_name = Some("Stop B".into());
        let t = trip_hs("T1", "R1", "Stop B"); // intermediate stop name
        let records = records_with(
            vec![sa1, sa2, sb],
            vec![route("R1", 3)],
            vec![t],
            vec![
                stoptime("T1", 1, "A1", (8, 0, 0), (8, 0, 0), 2),
                stoptime("T1", 2, "B", (8, 10, 0), (8, 10, 0), 3),
                stoptime("T1", 3, "A2", (8, 20, 0), (8, 20, 0), 4),
            ],
        );
        let result = analyze(&records, &empty_derived(), &default_config(), 20260514);
        assert!(
            !result.notices.iter().any(|n| n.rule_id == "TRP_020"),
            "parent_station ile dönen loop → TRP_020 olmamalı"
        );
    }

    // ── SHP_022: stop is near multiple matching regions on the shape ────────

    #[test]
    fn stop_near_two_shape_sections_produces_shp_022() {
        let mut shape_ti = ShapeInternTable::new();
        // "U" shape: an out-and-back route — the stop is near both the outbound and return
        // sections, and shape_dist_traveled is ABSENT. #52: sequence must be UNUSABLE here
        // (stop_sequence=1 is duplicated) → ambiguous position → SHP_022 is expected.
        use crate::k2::shapes::ShapePointRecord;
        let t = trip_sh("T1", "R1", "SH1");
        // Coordinates: travel along the lon=0.0 axis, shift 0.001° ≈ 111 m to the right, and return.
        // Stop: lat=0.5, lon=0.00005 — about 5 m from both the outbound (lon≈0) and return (lon≈0.001) segments.
        let mut s = stop("S1", 0.5, 0.00005);
        s.stop_name = Some("Orta Durak".into());
        let mut records = records_with(
            vec![s],
            vec![route("R1", 3)],
            vec![t],
            vec![
                stoptime("T1", 1, "S1", (8, 0, 0), (8, 0, 0), 2),
                stoptime("T1", 1, "S1", (8, 10, 0), (8, 10, 0), 3), // duplicate seq → sequence is unusable
            ],
        );
        // shape: (0,0)→(1,0)→(1,0.001)→(0,0.001) — U benzeri
        // Two vertical edges (lon=0 and lon=0.001) are ~5 m from the stop
        records.shapes = vec![
            ShapePointRecord::new(
                shape_ti.intern("SH1"),
                Some(0.0),
                Some(0.0),
                Some(1),
                None,
                2,
            ),
            ShapePointRecord::new(
                shape_ti.intern("SH1"),
                Some(1.0),
                Some(0.0),
                Some(2),
                None,
                3,
            ),
            ShapePointRecord::new(
                shape_ti.intern("SH1"),
                Some(1.0),
                Some(0.001),
                Some(3),
                None,
                4,
            ),
            ShapePointRecord::new(
                shape_ti.intern("SH1"),
                Some(0.0),
                Some(0.001),
                Some(4),
                None,
                5,
            ),
        ];
        records.shape_interns = shape_ti.clone();
        let result = analyze(&records, &empty_derived(), &default_config(), 20260514);
        assert!(
            result.notices.iter().any(|n| n.rule_id == "SHP_022"),
            "U-şekli + shape_dist eksik + sıra kullanılamaz → SHP_022 beklenir: {:?}",
            result
                .notices
                .iter()
                .map(|n| &n.rule_id)
                .collect::<Vec<_>>()
        );
    }

    #[test]
    fn stop_near_two_sections_valid_sequence_no_shp_022() {
        let mut shape_ti = ShapeInternTable::new();
        // #52: SAME U-shape geometry, but stop_sequence is VALID (strictly increasing 1,2) →
        // sequence-aware matching resolves the ambiguity → SHP_022 MUST NOT FIRE (over-fire prevented).
        use crate::k2::shapes::ShapePointRecord;
        let t = trip_sh("T1", "R1", "SH1");
        let mut s = stop("S1", 0.5, 0.00005);
        s.stop_name = Some("Orta Durak".into());
        let mut records = records_with(
            vec![s],
            vec![route("R1", 3)],
            vec![t],
            vec![
                stoptime("T1", 1, "S1", (8, 0, 0), (8, 0, 0), 2),
                stoptime("T1", 2, "S1", (8, 10, 0), (8, 10, 0), 3), // valid increasing sequence
            ],
        );
        records.shapes = vec![
            ShapePointRecord::new(
                shape_ti.intern("SH1"),
                Some(0.0),
                Some(0.0),
                Some(1),
                None,
                2,
            ),
            ShapePointRecord::new(
                shape_ti.intern("SH1"),
                Some(1.0),
                Some(0.0),
                Some(2),
                None,
                3,
            ),
            ShapePointRecord::new(
                shape_ti.intern("SH1"),
                Some(1.0),
                Some(0.001),
                Some(3),
                None,
                4,
            ),
            ShapePointRecord::new(
                shape_ti.intern("SH1"),
                Some(0.0),
                Some(0.001),
                Some(4),
                None,
                5,
            ),
        ];
        records.shape_interns = shape_ti.clone();
        let result = analyze(&records, &empty_derived(), &default_config(), 20260514);
        assert!(
            !result.notices.iter().any(|n| n.rule_id == "SHP_022"),
            "geçerli stop_sequence ile SHP_022 çıkmamalı (sıra belirsizliği çözer)"
        );
    }

    #[test]
    fn stop_near_one_shape_section_no_shp_022() {
        let mut shape_ti = ShapeInternTable::new();
        // Straight line — the stop is near only one section → SHP_022 must not fire
        use crate::k2::shapes::ShapePointRecord;
        let t = trip_sh("T1", "R1", "SH2");
        let mut s = stop("S1", 0.5, 0.0001); // near only the first segment
        s.stop_name = Some("Tek Bölüm".into());
        let mut records = records_with(
            vec![s],
            vec![route("R1", 3)],
            vec![t],
            vec![
                stoptime("T1", 1, "S1", (8, 0, 0), (8, 0, 0), 2),
                stoptime("T1", 2, "S1", (8, 10, 0), (8, 10, 0), 3),
            ],
        );
        // Straight shape, stop near only the first segment (~11 m)
        records.shapes = vec![
            ShapePointRecord::new(
                shape_ti.intern("SH2"),
                Some(0.0),
                Some(0.0),
                Some(1),
                None,
                2,
            ),
            ShapePointRecord::new(
                shape_ti.intern("SH2"),
                Some(1.0),
                Some(0.0),
                Some(2),
                None,
                3,
            ),
            ShapePointRecord::new(
                shape_ti.intern("SH2"),
                Some(2.0),
                Some(0.0),
                Some(3),
                None,
                4,
            ),
        ];
        records.shape_interns = shape_ti.clone();
        let result = analyze(&records, &empty_derived(), &default_config(), 20260514);
        assert!(
            !result.notices.iter().any(|n| n.rule_id == "SHP_022"),
            "Düz hat → SHP_022 olmamalı"
        );
    }

    // ── DQ_018: all-caps text field ─────────────────────────────────────────

    #[test]
    fn all_caps_stop_name_produces_dq_018() {
        let mut s = stop("S1", 41.0, 29.0);
        s.stop_name = Some("MERKEZ DURAK".into());
        let records = records_with(
            vec![s],
            vec![route("R1", 3)],
            vec![trip("T1", "R1")],
            vec![stoptime("T1", 1, "S1", (8, 0, 0), (8, 0, 0), 2)],
        );
        let result = analyze(&records, &empty_derived(), &default_config(), 20260514);
        assert!(
            result.notices.iter().any(|n| n.rule_id == "DQ_018"),
            "DQ_018 olmalı"
        );
    }

    #[test]
    fn mixed_case_stop_name_no_dq_018() {
        let mut s = stop("S1", 41.0, 29.0);
        s.stop_name = Some("Merkez Durak".into());
        let records = records_with(
            vec![s],
            vec![route("R1", 3)],
            vec![trip("T1", "R1")],
            vec![stoptime("T1", 1, "S1", (8, 0, 0), (8, 0, 0), 2)],
        );
        let result = analyze(&records, &empty_derived(), &default_config(), 20260514);
        assert!(
            !result.notices.iter().any(|n| n.rule_id == "DQ_018"),
            "düzgün harf DQ_018 üretmemeli"
        );
    }

    #[test]
    fn single_letter_stop_name_no_dq_018() {
        // Single-letter abbreviations (e.g. "A") must not count as all_caps (< 2 letters)
        let mut s = stop("S1", 41.0, 29.0);
        s.stop_name = Some("A".into());
        let records = records_with(
            vec![s],
            vec![route("R1", 3)],
            vec![trip("T1", "R1")],
            vec![stoptime("T1", 1, "S1", (8, 0, 0), (8, 0, 0), 2)],
        );
        let result = analyze(&records, &empty_derived(), &default_config(), 20260514);
        assert!(
            !result.notices.iter().any(|n| n.rule_id == "DQ_018"),
            "tek harf DQ_018 üretmemeli"
        );
    }

    #[test]
    fn institution_metadata_case_does_not_produce_dq_018_or_dq_019() {
        let mut records = records_with(
            vec![stop("S1", 41.0, 29.0)],
            vec![route("R1", 3)],
            vec![trip("T1", "R1")],
            vec![stoptime("T1", 1, "S1", (8, 0, 0), (8, 0, 0), 2)],
        );
        records.agencies.push(AgencyRecord {
            agency_id: Some("A1".into()),
            agency_name: "TRAMBAIX".into(),
            agency_url: "https://example.org".into(),
            agency_timezone: "Europe/Istanbul".into(),
            agency_lang: None,
            agency_phone: None,
            agency_fare_url: None,
            agency_email: None,
            agency_cemv_support: None,
            row: Default::default(),
            line: 2,
        });
        let mut fi = feed_info_row();
        fi.feed_publisher_name = "TRAMBESÒS".into();
        records.feed_info = vec![fi];

        let result = analyze(&records, &empty_derived(), &default_config(), 20260514);
        assert!(
            result
                .notices
                .iter()
                .all(|n| !matches!(n.rule_id.as_str(), "DQ_018" | "DQ_019")),
            "kurum/yayıncı metadata'sı DQ_018/019 üretmemeli: {:?}",
            result.notices
        );
    }

    // ── Fix-8 (revised): pathological shape projection falls back to haversine ─

    #[test]
    fn pathological_shape_projection_falls_back_to_haversine() {
        let mut shape_ti = ShapeInternTable::new();
        // Straight-line A→B ≈ 0.84 km. U-shaped projection matches B to the wrong segment
        // and gives arc ≈ 23 km (27× the straight-line distance). This is a WRONG segment
        // match (as in LA Metro grid bus routes). The 4× rule must fall back to haversine:
        // 0.84 km / 5 min ≈ 10 km/h → neither STM_012 nor STM_014 MUST be emitted.
        use crate::k2::shapes::ShapePointRecord;
        let mut records = records_with(
            vec![stop("A", 41.0, 29.0), stop("B", 41.0, 29.01)],
            vec![route("R1", 3)],
            vec![trip_sh("T1", "R1", "S1")],
            vec![
                stoptime("T1", 1, "A", (8, 0, 0), (8, 0, 0), 2),
                stoptime("T1", 2, "B", (8, 5, 0), (8, 5, 0), 3), // 5 dk
            ],
        );
        // U shape: (41.0,29.0) → north to (41.1,29.0) → (41.1,29.01) → south to (41.0,29.01)
        // Arc ≈ 23 km vs haversine ~0.84 km → ratio ~27× → exceeds the 4× threshold → fallback.
        records.shapes = vec![
            ShapePointRecord::new(
                shape_ti.intern("S1"),
                Some(41.0),
                Some(29.00),
                Some(1),
                None,
                2,
            ),
            ShapePointRecord::new(
                shape_ti.intern("S1"),
                Some(41.1),
                Some(29.00),
                Some(2),
                None,
                3,
            ),
            ShapePointRecord::new(
                shape_ti.intern("S1"),
                Some(41.1),
                Some(29.01),
                Some(3),
                None,
                4,
            ),
            ShapePointRecord::new(
                shape_ti.intern("S1"),
                Some(41.0),
                Some(29.01),
                Some(4),
                None,
                5,
            ),
        ];
        records.shape_interns = shape_ti.clone();
        let result = analyze(&records, &empty_derived(), &default_config(), 20260514);
        assert!(
            !result.notices.iter().any(|n| n.rule_id == "STM_012" || n.rule_id == "STM_014"),
            "Patolojik shape projeksiyonu (27× kuş uçuşu) haversine'e düşmeli; STM_012/014 üretilmemeli; notices: {:?}",
            result.notices.iter().map(|n| (&n.rule_id, n.observed_value.as_deref())).collect::<Vec<_>>()
        );
    }

    #[test]
    fn moderate_detour_within_ratio_still_uses_shape() {
        let mut shape_ti = ShapeInternTable::new();
        // Reasonable detour: if the shape remains below 4× the straight-line distance, keep
        // the shape distance. A→B straight-line distance ~11.1 km; gently curved shape ~13 km
        // (≈1.2×) → below 4×. 13 km / 2 min ≈ 390 km/h → 120 < 390 < 700 → STM_014 fires
        // (evidence that the shape distance was used).
        use crate::k2::shapes::ShapePointRecord;
        let mut records = records_with(
            vec![stop("A", 41.0, 29.0), stop("B", 41.1, 29.0)],
            vec![route("R1", 3)],
            vec![trip_sh("T1", "R1", "S1")],
            vec![
                stoptime("T1", 1, "A", (8, 0, 0), (8, 0, 0), 2),
                stoptime("T1", 2, "B", (8, 2, 0), (8, 2, 0), 3), // 2 dk
            ],
        );
        // A(41.0,29.0) → midpoint slightly east (41.05,29.03) → B(41.1,29.0): curved but ~1.2×
        records.shapes = vec![
            ShapePointRecord::new(
                shape_ti.intern("S1"),
                Some(41.00),
                Some(29.00),
                Some(1),
                None,
                2,
            ),
            ShapePointRecord::new(
                shape_ti.intern("S1"),
                Some(41.05),
                Some(29.03),
                Some(2),
                None,
                3,
            ),
            ShapePointRecord::new(
                shape_ti.intern("S1"),
                Some(41.10),
                Some(29.00),
                Some(3),
                None,
                4,
            ),
        ];
        records.shape_interns = shape_ti.clone();
        let result = analyze(&records, &empty_derived(), &default_config(), 20260514);
        assert!(
            result.notices.iter().any(|n| n.rule_id == "STM_014"),
            "Makul detour (≈1.2× kuş uçuşu) shape mesafesini korumalı → STM_014 beklenir; notices: {:?}",
            result.notices.iter().map(|n| (&n.rule_id, n.observed_value.as_deref())).collect::<Vec<_>>()
        );
    }

    #[test]
    fn trip_with_shape_id_but_no_shape_data_falls_back_to_haversine() {
        // shape_id is set, but no matching record exists in records.shapes.
        // Kod `shape_pts_speed.get(sid)` → None → Haversine fallback devreye girer.
        // A→B: ~0.82 km / 10 dk → ~5 km/h → STM_014 tetiklenmemeli.
        let records = records_with(
            vec![stop("A", 41.0, 29.0), stop("B", 41.0, 29.01)],
            vec![route("R1", 3)],
            vec![trip_sh("T1", "R1", "MISSING_SHAPE")],
            vec![
                stoptime("T1", 1, "A", (8, 0, 0), (8, 0, 0), 2),
                stoptime("T1", 2, "B", (8, 10, 0), (8, 10, 0), 3),
            ],
        );
        // records.shapes is empty → "MISSING_SHAPE" cannot be found → normal speed via Haversine
        let result = analyze(&records, &empty_derived(), &default_config(), 20260514);
        assert!(
            !result.notices.iter().any(|n| n.rule_id == "STM_014"),
            "shape verisi eksik → Haversine fallback; normal hız (~5 km/h) → STM_014 olmamalı"
        );
    }

    // ── VAT: entity analytics detection ────────────────────────────────────

    #[test]
    fn similar_routes_produce_vat_001() {
        // R1 and R2 share the same 6 stops → Jaccard=1.0 ≥ 0.85 → VAT_001
        let stops: Vec<StopRecord> = (1..=6)
            .map(|i| stop(&format!("S{i}"), 41.0 + i as f64 * 0.01, 29.0))
            .collect();
        let t1 = trip("T1", "R1");
        let t2 = trip("T2", "R2");
        let stop_times = (1..=6)
            .flat_map(|i| {
                let h = i;
                vec![
                    stoptime(
                        "T1",
                        i,
                        &format!("S{i}"),
                        (h, 0, 0),
                        (h, 0, 0),
                        i as u64 + 1,
                    ),
                    stoptime(
                        "T2",
                        i,
                        &format!("S{i}"),
                        (h, 5, 0),
                        (h, 5, 0),
                        i as u64 + 10,
                    ),
                ]
            })
            .collect();
        let r = crate::k2::EntityRecords {
            stops,
            routes: vec![route("R1", 3), route("R2", 3)],
            trips: vec![t1, t2],
            trip_interns: take_ti(),
            stop_times,
            ..Default::default()
        };
        let result = analyze(&r, &empty_derived(), &default_config(), 20260514);
        assert!(
            result.notices.iter().any(|n| n.rule_id == "VAT_001"),
            "VAT_001 olmalı"
        );
    }

    #[test]
    fn very_different_routes_no_vat_001() {
        // R1: S1–S3, R2: S4–S6 → no shared stops → Jaccard=0 → VAT_001 must not fire
        let r = crate::k2::EntityRecords {
            stops: (1..=6)
                .map(|i| stop(&format!("S{i}"), 41.0 + i as f64 * 0.01, 29.0))
                .collect(),
            routes: vec![route("R1", 3), route("R2", 3)],
            trips: vec![trip("T1", "R1"), trip("T2", "R2")],
            trip_interns: take_ti(),
            stop_times: vec![
                stoptime("T1", 1, "S1", (8, 0, 0), (8, 0, 0), 2),
                stoptime("T1", 2, "S2", (8, 5, 0), (8, 5, 0), 3),
                stoptime("T1", 3, "S3", (8, 10, 0), (8, 10, 0), 4),
                stoptime("T2", 1, "S4", (9, 0, 0), (9, 0, 0), 2),
                stoptime("T2", 2, "S5", (9, 5, 0), (9, 5, 0), 3),
                stoptime("T2", 3, "S6", (9, 10, 0), (9, 10, 0), 4),
            ],
            ..Default::default()
        };
        let result = analyze(&r, &empty_derived(), &default_config(), 20260514);
        assert!(
            !result.notices.iter().any(|n| n.rule_id == "VAT_001"),
            "VAT_001 olmamalı"
        );
    }

    #[test]
    fn busy_stop_no_transfer_produces_vat_002() {
        // Four routes serve S1, AND a route not serving S1 (R5) serves S9, 200 m away
        // → a genuinely undefined TRANSFER BETWEEN STOPS exists → VAT_002.
        let mut sts: Vec<_> = (1..=4)
            .flat_map(|i| {
                vec![
                    stoptime(&format!("T{i}"), 1, "S1", (8, 0, 0), (8, 0, 0), 2),
                    stoptime(&format!("T{i}"), 2, "S2", (8, 10, 0), (8, 10, 0), 3),
                ]
            })
            .collect();
        // R5 serves only neighboring stop S9 — not S1.
        sts.push(stoptime("T5", 1, "S9", (9, 0, 0), (9, 0, 0), 4));
        sts.push(stoptime("T5", 2, "S8", (9, 10, 0), (9, 10, 0), 5));
        let r = crate::k2::EntityRecords {
            stops: vec![
                stop("S1", 41.0, 29.0),
                stop("S2", 41.1, 29.1),
                stop("S9", 41.0018, 29.0), // ~200 m — within max_transfer_distance_m (500 m)
                stop("S8", 41.2, 29.2),
            ],
            routes: (1..=5).map(|i| route(&format!("R{i}"), 3)).collect(),
            trips: (1..=5)
                .map(|i| trip(&format!("T{i}"), &format!("R{i}")))
                .collect(),
            trip_interns: take_ti(),
            stop_times: sts,
            ..Default::default()
        };
        let result = analyze(&r, &empty_derived(), &default_config(), 20260514);
        assert!(
            result.notices.iter().any(|n| n.rule_id == "VAT_002"),
            "VAT_002 olmalı"
        );
    }

    // ── WP-D: transfers at the same stop_id are implicit ────────────────────

    /// If all routes use the SAME stop_id (the Ankara Gar pattern), there is no transfer to
    /// define; GTFS consumers already know that transfers can be made at that stop.
    #[test]
    fn vat_002_silent_when_all_routes_share_one_stop_id() {
        let r = crate::k2::EntityRecords {
            stops: vec![stop("GAR", 41.0, 29.0), stop("S2", 41.1, 29.1)],
            routes: (1..=4).map(|i| route(&format!("R{i}"), 2)).collect(),
            trips: (1..=4)
                .map(|i| trip(&format!("T{i}"), &format!("R{i}")))
                .collect(),
            trip_interns: take_ti(),
            stop_times: (1..=4)
                .flat_map(|i| {
                    vec![
                        stoptime(&format!("T{i}"), 1, "GAR", (8, 0, 0), (8, 0, 0), 2),
                        stoptime(&format!("T{i}"), 2, "S2", (8, 10, 0), (8, 10, 0), 3),
                    ]
                })
                .collect(),
            ..Default::default()
        };
        let result = analyze(&r, &empty_derived(), &default_config(), 20260514);
        assert!(
            !result.notices.iter().any(|n| n.rule_id == "VAT_002"),
            "aynı stop_id'de aktarma örtüktür → VAT_002 çıkmamalı"
        );
    }

    /// Platform stops with a `parent_station` are excluded: the complex is already modeled.
    /// (The old code excluded only the location_type=1 station ITSELF, not its children.)
    #[test]
    fn vat_002_skips_child_stops_of_a_modelled_station() {
        use std::collections::HashMap as StdMap;
        let mut platform = stop("P1", 41.0, 29.0);
        let mut row: StdMap<String, String> = StdMap::new();
        row.insert("parent_station".to_string(), "GAR".to_string());
        platform.row = row;
        let mut sts: Vec<_> = (1..=4)
            .flat_map(|i| {
                vec![
                    stoptime(&format!("T{i}"), 1, "P1", (8, 0, 0), (8, 0, 0), 2),
                    stoptime(&format!("T{i}"), 2, "S2", (8, 10, 0), (8, 10, 0), 3),
                ]
            })
            .collect();
        sts.push(stoptime("T5", 1, "S9", (9, 0, 0), (9, 0, 0), 4));
        sts.push(stoptime("T5", 2, "S8", (9, 10, 0), (9, 10, 0), 5));
        let r = crate::k2::EntityRecords {
            stops: vec![
                platform,
                stop("S2", 41.1, 29.1),
                stop("S9", 41.0018, 29.0), // nearby neighbor + different route → would fire without the guard
                stop("S8", 41.2, 29.2),
            ],
            routes: (1..=5).map(|i| route(&format!("R{i}"), 3)).collect(),
            trips: (1..=5)
                .map(|i| trip(&format!("T{i}"), &format!("R{i}")))
                .collect(),
            trip_interns: take_ti(),
            stop_times: sts,
            ..Default::default()
        };
        let result = analyze(&r, &empty_derived(), &default_config(), 20260514);
        assert!(
            !result
                .notices
                .iter()
                .any(|n| n.rule_id == "VAT_002" && n.entity_id.as_deref() == Some("P1")),
            "parent_station'lı durak için VAT_002 çıkmamalı"
        );
    }

    /// If the neighboring stop offers ONLY the same routes, there is no new connection → silent.
    #[test]
    fn vat_002_silent_when_neighbour_adds_no_new_route() {
        let r = crate::k2::EntityRecords {
            stops: vec![
                stop("S1", 41.0, 29.0),
                stop("S9", 41.0018, 29.0), // nearby, but the same routes
                stop("S2", 41.1, 29.1),
            ],
            routes: (1..=4).map(|i| route(&format!("R{i}"), 3)).collect(),
            trips: (1..=4)
                .map(|i| trip(&format!("T{i}"), &format!("R{i}")))
                .collect(),
            trip_interns: take_ti(),
            stop_times: (1..=4)
                .flat_map(|i| {
                    vec![
                        stoptime(&format!("T{i}"), 1, "S1", (8, 0, 0), (8, 0, 0), 2),
                        stoptime(&format!("T{i}"), 2, "S9", (8, 5, 0), (8, 5, 0), 3),
                        stoptime(&format!("T{i}"), 3, "S2", (8, 10, 0), (8, 10, 0), 4),
                    ]
                })
                .collect(),
            ..Default::default()
        };
        let result = analyze(&r, &empty_derived(), &default_config(), 20260514);
        assert!(
            !result
                .notices
                .iter()
                .any(|n| n.rule_id == "VAT_002" && n.entity_id.as_deref() == Some("S1")),
            "komşu yeni hat sunmuyorsa aktarma boşluğu yoktur → VAT_002 çıkmamalı"
        );
    }

    #[test]
    fn outlier_trip_duration_produces_vat_003() {
        // Route R1 has 5 trips: 4 around 30 minutes, 1 around 3 hours (outlier) → VAT_003
        // Normal: T1–T4 = 30 minutes, outlier: T5 = 180 minutes
        let mut trips_vec = Vec::new();
        let mut stoptimes_vec = Vec::new();
        for i in 1..=4u32 {
            let tid = format!("T{i}");
            trips_vec.push(trip(&tid, "R1"));
            stoptimes_vec.push(stoptime(&tid, 1, "A", (i, 0, 0), (i, 0, 0), 2));
            stoptimes_vec.push(stoptime(&tid, 2, "B", (i, 30, 0), (i, 30, 0), 3));
        }
        // Outlier trip: 180 minutes (3 hours)
        trips_vec.push(trip("T5", "R1"));
        stoptimes_vec.push(stoptime("T5", 1, "A", (6, 0, 0), (6, 0, 0), 2));
        stoptimes_vec.push(stoptime("T5", 2, "B", (9, 0, 0), (9, 0, 0), 3));
        let r = crate::k2::EntityRecords {
            stops: vec![stop("A", 41.0, 29.0), stop("B", 41.1, 29.1)],
            routes: vec![route("R1", 3)],
            trips: trips_vec,
            trip_interns: take_ti(),
            stop_times: stoptimes_vec,
            ..Default::default()
        };
        let result = analyze(&r, &empty_derived(), &default_config(), 20260514);
        assert!(
            result.notices.iter().any(|n| n.rule_id == "VAT_003"),
            "VAT_003 olmalı"
        );
    }

    // ── WP-C: date-expansion collapse ──────────────────────────────────────
    //
    // Some producers (the TCDD pattern) write the same timetable as a separate trip_id
    // for every operating day. These duplicates inflate the report and distort MAD.

    /// If the same timetable spans 10 dates, emit ONE notice, not 10; the message says how
    /// many records were duplicates.
    #[test]
    fn vat_003_collapses_date_expanded_duplicate_trips() {
        let mut trips_vec = Vec::new();
        let mut stoptimes_vec = Vec::new();
        let mut line = 2u64;
        // 5 different timetables (different departure times, all ~30 minutes) — to pass the ≥5 guard.
        // Each timetable spans 10 DATES: same departure + same duration, different trip_ids.
        for h in 1..=5u32 {
            for day in 0..10u32 {
                let tid = format!("T{h}_D{day:02}");
                trips_vec.push(trip(&tid, "R1"));
                stoptimes_vec.push(stoptime(&tid, 1, "A", (h, 0, 0), (h, 0, 0), line));
                line += 1;
                stoptimes_vec.push(stoptime(&tid, 2, "B", (h, 30, 0), (h, 30, 0), line));
                line += 1;
            }
        }
        // Outlier timetable: 180 minutes — also spanning 10 dates.
        for day in 0..10u32 {
            let tid = format!("X_D{day:02}");
            trips_vec.push(trip(&tid, "R1"));
            stoptimes_vec.push(stoptime(&tid, 1, "A", (6, 0, 0), (6, 0, 0), line));
            line += 1;
            stoptimes_vec.push(stoptime(&tid, 2, "B", (9, 0, 0), (9, 0, 0), line));
            line += 1;
        }
        let r = crate::k2::EntityRecords {
            stops: vec![stop("A", 41.0, 29.0), stop("B", 41.1, 29.1)],
            routes: vec![route("R1", 3)],
            trips: trips_vec,
            trip_interns: take_ti(),
            stop_times: stoptimes_vec,
            ..Default::default()
        };

        let result = analyze(&r, &empty_derived(), &default_config(), 20260514);
        let vat: Vec<&Notice> = result
            .notices
            .iter()
            .filter(|n| n.rule_id == "VAT_003")
            .collect();
        assert_eq!(
            vat.len(),
            1,
            "60 sefer / 6 tarife → 1 aykırı TARİFE = 1 notice (10 değil); çıkanlar: {:?}",
            vat.iter()
                .map(|n| n.entity_id.as_deref())
                .collect::<Vec<_>>()
        );
        let n = vat[0];
        // Representative trip: lexicographically smallest (deterministic).
        assert_eq!(
            n.entity_id.as_deref(),
            Some("X_D00"),
            "representative must be the smallest trip_id"
        );
        assert_eq!(
            n.details
                .as_ref()
                .and_then(|d| d.get("duplicate_trips"))
                .map(String::as_str),
            Some("10"),
            "kopya sayısı details'ta taşınmalı"
        );
        assert!(
            n.message.contains("10 sefer kaydında tekrarlanıyor"),
            "mesaj kaç kaydı ilgilendirdiğini söylemeli: {}",
            n.message
        );
    }

    /// Behavior is UNCHANGED on a feed without duplicates (the collapse must be a no-op) — regression guard.
    #[test]
    fn vat_003_unchanged_when_no_duplicate_schedules() {
        let mut trips_vec = Vec::new();
        let mut stoptimes_vec = Vec::new();
        // 5 unique timetables (~30 minutes) + 1 outlier — none are duplicated.
        for i in 1..=5u32 {
            let tid = format!("T{i}");
            trips_vec.push(trip(&tid, "R1"));
            stoptimes_vec.push(stoptime(&tid, 1, "A", (i, 0, 0), (i, 0, 0), 2));
            stoptimes_vec.push(stoptime(&tid, 2, "B", (i, 30, 0), (i, 30, 0), 3));
        }
        trips_vec.push(trip("T9", "R1"));
        stoptimes_vec.push(stoptime("T9", 1, "A", (6, 0, 0), (6, 0, 0), 2));
        stoptimes_vec.push(stoptime("T9", 2, "B", (9, 0, 0), (9, 0, 0), 3));
        let r = crate::k2::EntityRecords {
            stops: vec![stop("A", 41.0, 29.0), stop("B", 41.1, 29.1)],
            routes: vec![route("R1", 3)],
            trips: trips_vec,
            trip_interns: take_ti(),
            stop_times: stoptimes_vec,
            ..Default::default()
        };

        let result = analyze(&r, &empty_derived(), &default_config(), 20260514);
        let vat: Vec<&Notice> = result
            .notices
            .iter()
            .filter(|n| n.rule_id == "VAT_003")
            .collect();
        assert_eq!(vat.len(), 1, "aykırı sefer yine yakalanmalı");
        assert_eq!(vat[0].entity_id.as_deref(), Some("T9"));
        assert!(
            vat[0]
                .details
                .as_ref()
                .is_none_or(|d| !d.contains_key("duplicate_trips")),
            "kopya yoksa duplicate_trips yazılmamalı"
        );
        assert!(
            !vat[0].message.contains("tekrarlanıyor"),
            "kopya yoksa mesaj değişmemeli: {}",
            vat[0].message
        );
    }

    /// The ≥5 guard now counts UNIQUE TIMETABLES: with 40 trips but only 4 timetables,
    /// there is insufficient evidence for an outlier claim → remain silent.
    /// The old behavior counted the same 4 observations 10 times and created artificial confidence.
    #[test]
    fn vat_003_requires_five_distinct_schedules_not_five_trips() {
        let mut trips_vec = Vec::new();
        let mut stoptimes_vec = Vec::new();
        let mut line = 2u64;
        // Only 3 normal timetables, each on 10 dates (30 trips).
        for h in 1..=3u32 {
            for day in 0..10u32 {
                let tid = format!("T{h}_D{day:02}");
                trips_vec.push(trip(&tid, "R1"));
                stoptimes_vec.push(stoptime(&tid, 1, "A", (h, 0, 0), (h, 0, 0), line));
                line += 1;
                stoptimes_vec.push(stoptime(&tid, 2, "B", (h, 30, 0), (h, 30, 0), line));
                line += 1;
            }
        }
        // + 1 outlier timetable × 10 dates = 40 trips / 4 timetables total.
        for day in 0..10u32 {
            let tid = format!("X_D{day:02}");
            trips_vec.push(trip(&tid, "R1"));
            stoptimes_vec.push(stoptime(&tid, 1, "A", (6, 0, 0), (6, 0, 0), line));
            line += 1;
            stoptimes_vec.push(stoptime(&tid, 2, "B", (9, 0, 0), (9, 0, 0), line));
            line += 1;
        }
        let r = crate::k2::EntityRecords {
            stops: vec![stop("A", 41.0, 29.0), stop("B", 41.1, 29.1)],
            routes: vec![route("R1", 3)],
            trips: trips_vec,
            trip_interns: take_ti(),
            stop_times: stoptimes_vec,
            ..Default::default()
        };

        let result = analyze(&r, &empty_derived(), &default_config(), 20260514);
        assert!(
            !result.notices.iter().any(|n| n.rule_id == "VAT_003"),
            "40 sefer / 4 benzersiz tarife robust-z için yetersiz kanıt → VAT_003 çıkmamalı"
        );
    }

    #[test]
    fn vat_003_does_not_cross_flag_distinct_patterns_on_same_route() {
        // Same route_id, two separate patterns: 6 short trips (2 stops, 10 minutes) + 5 long
        // trips (4 stops, 30 minutes). The old route_id grouping flagged the long trips as
        // outliers; grouping by pattern (stop count) makes each cluster internally consistent
        // → no VAT_003.
        let mut trips_vec = Vec::new();
        let mut stoptimes_vec = Vec::new();
        for i in 0..6u32 {
            let tid = format!("S{i}");
            trips_vec.push(trip(&tid, "R1"));
            stoptimes_vec.push(stoptime(&tid, 1, "A", (8, 0, 0), (8, 0, 0), 2));
            stoptimes_vec.push(stoptime(&tid, 2, "B", (8, 10, 0), (8, 10, 0), 3));
        }
        for i in 0..5u32 {
            let tid = format!("L{i}");
            trips_vec.push(trip(&tid, "R1"));
            stoptimes_vec.push(stoptime(&tid, 1, "A", (9, 0, 0), (9, 0, 0), 2));
            stoptimes_vec.push(stoptime(&tid, 2, "B", (9, 10, 0), (9, 10, 0), 3));
            stoptimes_vec.push(stoptime(&tid, 3, "C", (9, 20, 0), (9, 20, 0), 4));
            stoptimes_vec.push(stoptime(&tid, 4, "D", (9, 30, 0), (9, 30, 0), 5));
        }
        let r = crate::k2::EntityRecords {
            stops: vec![
                stop("A", 41.0, 29.0),
                stop("B", 41.1, 29.1),
                stop("C", 41.2, 29.2),
                stop("D", 41.3, 29.3),
            ],
            routes: vec![route("R1", 3)],
            trips: trips_vec,
            trip_interns: take_ti(),
            stop_times: stoptimes_vec,
            ..Default::default()
        };
        let result = analyze(&r, &empty_derived(), &default_config(), 20260514);
        assert!(
            !result.notices.iter().any(|n| n.rule_id == "VAT_003"),
            "ayrı desenler çapraz aykırı işaretlenmemeli"
        );
    }

    #[test]
    fn vat_003_groups_by_stop_sequence_not_count() {
        // Same route, no shape, SAME stop count (2) but DIFFERENT stops:
        // 6 trips A→B (10 minutes) + 5 trips A→C (30 minutes). A stop-count key would mix
        // them and flag the 30-minute trips; the stop-order hash separates them → no VAT_003.
        let mut trips_vec = Vec::new();
        let mut stoptimes_vec = Vec::new();
        for i in 0..6u32 {
            let tid = format!("S{i}");
            trips_vec.push(trip(&tid, "R1"));
            stoptimes_vec.push(stoptime(&tid, 1, "A", (8, 0, 0), (8, 0, 0), 2));
            stoptimes_vec.push(stoptime(&tid, 2, "B", (8, 10, 0), (8, 10, 0), 3));
        }
        for i in 0..5u32 {
            let tid = format!("L{i}");
            trips_vec.push(trip(&tid, "R1"));
            stoptimes_vec.push(stoptime(&tid, 1, "A", (9, 0, 0), (9, 0, 0), 2));
            stoptimes_vec.push(stoptime(&tid, 2, "C", (9, 30, 0), (9, 30, 0), 3));
        }
        let r = crate::k2::EntityRecords {
            stops: vec![
                stop("A", 41.0, 29.0),
                stop("B", 41.1, 29.1),
                stop("C", 41.5, 29.5),
            ],
            routes: vec![route("R1", 3)],
            trips: trips_vec,
            trip_interns: take_ti(),
            stop_times: stoptimes_vec,
            ..Default::default()
        };
        let result = analyze(&r, &empty_derived(), &default_config(), 20260514);
        assert!(
            !result.notices.iter().any(|n| n.rule_id == "VAT_003"),
            "aynı durak sayılı farklı desenler durak-sırasıyla ayrılmalı, çapraz işaretlenmemeli"
        );
    }

    #[test]
    fn vat_003_same_shape_different_stop_patterns_do_not_cross_flag() {
        // Real BART regression: the same shape_id can be shared by a short-turn trip
        // (A→B, 10 minutes) and a full trip (A→B→C→D, 30 minutes). Shape alone is not a
        // pattern; the stop sequence must also be part of the key, otherwise short trips
        // become mass FPs.
        let mut trips_vec = Vec::new();
        let mut stoptimes_vec = Vec::new();
        for i in 0..5u32 {
            let tid = format!("S{i}");
            trips_vec.push(trip_sh(&tid, "R1", "SHARED"));
            stoptimes_vec.push(stoptime(&tid, 1, "A", (8, 0, 0), (8, 0, 0), 2));
            stoptimes_vec.push(stoptime(&tid, 2, "B", (8, 10, 0), (8, 10, 0), 3));
        }
        for i in 0..5u32 {
            let tid = format!("L{i}");
            trips_vec.push(trip_sh(&tid, "R1", "SHARED"));
            stoptimes_vec.push(stoptime(&tid, 1, "A", (9, 0, 0), (9, 0, 0), 2));
            stoptimes_vec.push(stoptime(&tid, 2, "B", (9, 10, 0), (9, 10, 0), 3));
            stoptimes_vec.push(stoptime(&tid, 3, "C", (9, 20, 0), (9, 20, 0), 4));
            stoptimes_vec.push(stoptime(&tid, 4, "D", (9, 30, 0), (9, 30, 0), 5));
        }
        let r = crate::k2::EntityRecords {
            stops: vec![
                stop("A", 41.0, 29.0),
                stop("B", 41.1, 29.1),
                stop("C", 41.2, 29.2),
                stop("D", 41.3, 29.3),
            ],
            routes: vec![route("R1", 3)],
            trips: trips_vec,
            trip_interns: take_ti(),
            stop_times: stoptimes_vec,
            ..Default::default()
        };
        let result = analyze(&r, &empty_derived(), &default_config(), 20260514);
        assert!(
            !result.notices.iter().any(|n| n.rule_id == "VAT_003"),
            "aynı shape'i paylaşan farklı stop desenleri çapraz işaretlenmemeli"
        );
    }

    #[test]
    fn vat_003_deseasonalizes_peak_offpeak_within_pattern() {
        // One pattern (A→B), one route, no shape. Dominant off-peak cluster (hour 10, ~20 minutes),
        // legitimate longer peak trips (hour 8, 28 minutes), and 1 real event (hour 10, 40 minutes).
        // Hour-based deseasonalization must not produce peak FPs; only the event should be flagged.
        // (The old global median/MAD would flag all peak trips as FPs in this fixture.)
        let mut trips_vec = Vec::new();
        let mut sts = Vec::new();
        // Off-peak: hour 10, 19/20/21 minutes (slight spread), 20 trips — dominant cluster.
        for i in 0..20u32 {
            let tid = format!("O{i}");
            trips_vec.push(trip(&tid, "R1"));
            let mins = 19 + (i % 3);
            sts.push(stoptime(&tid, 1, "A", (10, 0, 0), (10, 0, 0), 2));
            sts.push(stoptime(&tid, 2, "B", (10, mins, 0), (10, mins, 0), 3));
        }
        // Peak: hour 8, 28 minutes, 6 trips — legitimately longer.
        for i in 0..6u32 {
            let tid = format!("P{i}");
            trips_vec.push(trip(&tid, "R1"));
            sts.push(stoptime(&tid, 1, "A", (8, 0, 0), (8, 0, 0), 2));
            sts.push(stoptime(&tid, 2, "B", (8, 28, 0), (8, 28, 0), 3));
        }
        // Olay: saat 10, 40dk.
        trips_vec.push(trip("INC", "R1"));
        sts.push(stoptime("INC", 1, "A", (10, 2, 0), (10, 2, 0), 2));
        sts.push(stoptime("INC", 2, "B", (10, 42, 0), (10, 42, 0), 3));
        let r = crate::k2::EntityRecords {
            stops: vec![stop("A", 41.0, 29.0), stop("B", 41.1, 29.1)],
            routes: vec![route("R1", 3)],
            trips: trips_vec,
            trip_interns: take_ti(),
            stop_times: sts,
            ..Default::default()
        };
        let result = analyze(&r, &empty_derived(), &default_config(), 20260514);
        let flagged: std::collections::HashSet<&str> = result
            .notices
            .iter()
            .filter(|n| n.rule_id == "VAT_003")
            .filter_map(|n| n.entity_id.as_deref())
            .collect();
        assert!(flagged.contains("INC"), "gerçek olay (40dk) işaretlenmeli");
        assert!(
            !flagged.iter().any(|id| id.starts_with('P')),
            "meşru peak seferleri saat-bazlı deseasonalize ile işaretlenmemeli"
        );
    }

    #[test]
    fn vat_003_owl_trips_banded_by_clock_time_not_service_day() {
        // Issue #26: trips crossing midnight (owl trips) are encoded as 24:xx+ in service-day
        // notation. Without % 24, the band function put them in the wrong band instead of
        // evening (band 4): a 20-minute night trip is normal but becomes an FP when compared
        // with the evening median (40 minutes). With % 24, owl trips return to the real night
        // band (band 0).
        let mut trips_vec = Vec::new();
        let mut sts = Vec::new();
        // NOTE: departure minutes are intentionally different. Because the rule now treats
        // identical (departure, duration) pairs as one TIMETABLE and collapses them (the
        // date-expansion fix), a fixture filled with identical trips would hit the ≥5
        // unique-timetable guard and this test would measure collapse rather than band
        // assignment.
        // Night band (hour 2, 20 minutes), 5 separate timetables.
        for i in 0..5u32 {
            let tid = format!("N{i}");
            trips_vec.push(trip(&tid, "R1"));
            sts.push(stoptime(&tid, 1, "A", (2, i * 10, 0), (2, i * 10, 0), 2));
            sts.push(stoptime(
                &tid,
                2,
                "B",
                (2, i * 10 + 20, 0),
                (2, i * 10 + 20, 0),
                3,
            ));
        }
        // Evening band (hour 21, 40 minutes — long compared with night traffic), 6 separate timetables.
        for i in 0..6u32 {
            let tid = format!("E{i}");
            trips_vec.push(trip(&tid, "R1"));
            sts.push(stoptime(&tid, 1, "A", (21, i, 0), (21, i, 0), 2));
            sts.push(stoptime(&tid, 2, "B", (21, i + 40, 0), (21, i + 40, 0), 3));
        }
        // Owl trips: 24:xx departure (next day 00:xx), NORMAL 20 minutes for the night regime.
        for i in 0..3u32 {
            let tid = format!("W{i}");
            trips_vec.push(trip(&tid, "R1"));
            sts.push(stoptime(
                &tid,
                1,
                "A",
                (24, 30 + i * 5, 0),
                (24, 30 + i * 5, 0),
                2,
            ));
            sts.push(stoptime(
                &tid,
                2,
                "B",
                (24, 50 + i * 5, 0),
                (24, 50 + i * 5, 0),
                3,
            ));
        }
        // Owl event: 24:35 departure, 40 minutes — ABNORMAL for the night band.
        trips_vec.push(trip("WINC", "R1"));
        sts.push(stoptime("WINC", 1, "A", (24, 35, 0), (24, 35, 0), 2));
        sts.push(stoptime("WINC", 2, "B", (25, 15, 0), (25, 15, 0), 3));
        let r = crate::k2::EntityRecords {
            stops: vec![stop("A", 41.0, 29.0), stop("B", 41.1, 29.1)],
            routes: vec![route("R1", 3)],
            trips: trips_vec,
            trip_interns: take_ti(),
            stop_times: sts,
            ..Default::default()
        };
        let result = analyze(&r, &empty_derived(), &default_config(), 20260514);
        let flagged: std::collections::HashSet<&str> = result
            .notices
            .iter()
            .filter(|n| n.rule_id == "VAT_003")
            .filter_map(|n| n.entity_id.as_deref())
            .collect();
        // With % 24, owl-normal trips fall into the night band → no FP.
        assert!(
            !flagged
                .iter()
                .any(|id| id.starts_with('W') && *id != "WINC"),
            "normal owl seferleri gece bandına göre kıyaslanmalı (yanlış akşam-FP olmamalı)"
        );
        // A genuine owl anomaly must still be caught against the night band.
        assert!(
            flagged.contains("WINC"),
            "gece bandı için anormal (40dk) owl seferi işaretlenmeli"
        );
    }

    #[test]
    fn isolated_stop_cluster_produces_vat_005() {
        // Main network: A-B-C-D (10+ connections), isolated cluster: X-Y
        // Main network: multiple trips A-B-C-D-E-F
        let mut trips_v = Vec::new();
        let mut st_v = Vec::new();
        for i in 1..=12u32 {
            let tid = format!("M{i}");
            trips_v.push(trip(&tid, "R1"));
            st_v.push(stoptime(&tid, 1, "A", (i, 0, 0), (i, 0, 0), 2));
            st_v.push(stoptime(&tid, 2, "B", (i, 5, 0), (i, 5, 0), 3));
            st_v.push(stoptime(&tid, 3, "C", (i, 10, 0), (i, 10, 0), 4));
            st_v.push(stoptime(&tid, 4, "D", (i, 15, 0), (i, 15, 0), 5));
            st_v.push(stoptime(&tid, 5, "E", (i, 20, 0), (i, 20, 0), 6));
            st_v.push(stoptime(&tid, 6, "F", (i, 25, 0), (i, 25, 0), 7));
        }
        // Isolated cluster: X-Y
        trips_v.push(trip("ISO", "R2"));
        st_v.push(stoptime("ISO", 1, "X", (8, 0, 0), (8, 0, 0), 2));
        st_v.push(stoptime("ISO", 2, "Y", (8, 10, 0), (8, 10, 0), 3));
        let r = crate::k2::EntityRecords {
            stops: vec![
                stop("A", 41.0, 29.0),
                stop("B", 41.1, 29.0),
                stop("C", 41.2, 29.0),
                stop("D", 41.3, 29.0),
                stop("E", 41.4, 29.0),
                stop("F", 41.5, 29.0),
                stop("X", 42.0, 30.0),
                stop("Y", 42.1, 30.0), // izole
            ],
            routes: vec![route("R1", 3), route("R2", 3)],
            trips: trips_v,
            trip_interns: take_ti(),
            stop_times: st_v,
            ..Default::default()
        };
        let result = analyze(&r, &empty_derived(), &default_config(), 20260514);
        assert!(
            result.notices.iter().any(|n| n.rule_id == "VAT_005"),
            "VAT_005 olmalı"
        );
    }

    #[test]
    fn no_majority_component_suppresses_vat_005() {
        // Three equal routes (3 stops each) + distant X-Y: the largest component contains only
        // 27% of stops. There is no main network; calling X-Y "detached" from a randomly chosen
        // route is meaningless (2026-10-30 corpus, mdb-905: 386 stops, 34 components, largest 13).
        let mut trips_v = Vec::new();
        let mut st_v = Vec::new();
        for (line, stops) in [("L1", ["A", "B", "C"]), ("L2", ["D", "E", "F"]), ("L3", ["G", "H", "I"])] {
            for i in 1..=3u32 {
                let tid = format!("{line}-{i}");
                trips_v.push(trip(&tid, "R1"));
                for (seq, sid) in stops.iter().enumerate() {
                    let h = i + seq as u32;
                    st_v.push(stoptime(&tid, seq as u32 + 1, sid, (h, 0, 0), (h, 0, 0), 2));
                }
            }
        }
        trips_v.push(trip("ISO", "R2"));
        st_v.push(stoptime("ISO", 1, "X", (8, 0, 0), (8, 0, 0), 2));
        st_v.push(stoptime("ISO", 2, "Y", (8, 10, 0), (8, 10, 0), 3));
        // Stops are 0.1° (~11 km) apart: the 200 m proximity suppression does not apply.
        let stops_v = ["A", "B", "C", "D", "E", "F", "G", "H", "I", "X", "Y"]
            .iter()
            .enumerate()
            .map(|(i, sid)| stop(sid, 40.0 + i as f64 * 0.1, 29.0))
            .collect();
        let r = crate::k2::EntityRecords {
            stops: stops_v,
            routes: vec![route("R1", 3), route("R2", 3)],
            trips: trips_v,
            trip_interns: take_ti(),
            stop_times: st_v,
            ..Default::default()
        };
        let result = analyze(&r, &empty_derived(), &default_config(), 20260514);
        assert!(
            !result.notices.iter().any(|n| n.rule_id == "VAT_005"),
            "ana ağ yokken VAT_005 olmamalı"
        );
    }

    #[test]
    fn isolated_cluster_near_main_network_suppressed_vat_005() {
        // Isolated cluster is geographically VERY close to the main network (multiple platforms
        // without parent_station, e.g. a bus terminal) → physically the same place → VAT_005 suppressed.
        let mut trips_v = Vec::new();
        let mut st_v = Vec::new();
        for i in 1..=12u32 {
            let tid = format!("M{i}");
            trips_v.push(trip(&tid, "R1"));
            st_v.push(stoptime(&tid, 1, "A", (i, 0, 0), (i, 0, 0), 2));
            st_v.push(stoptime(&tid, 2, "B", (i, 5, 0), (i, 5, 0), 3));
            st_v.push(stoptime(&tid, 3, "C", (i, 10, 0), (i, 10, 0), 4));
            st_v.push(stoptime(&tid, 4, "D", (i, 15, 0), (i, 15, 0), 5));
            st_v.push(stoptime(&tid, 5, "E", (i, 20, 0), (i, 20, 0), 6));
            st_v.push(stoptime(&tid, 6, "F", (i, 25, 0), (i, 25, 0), 7));
        }
        // Isolated cluster P-Q: no trip link to the main network, but P's coordinates are adjacent to A (<200 m).
        trips_v.push(trip("ISO", "R2"));
        st_v.push(stoptime("ISO", 1, "P", (8, 0, 0), (8, 0, 0), 2));
        st_v.push(stoptime("ISO", 2, "Q", (8, 10, 0), (8, 10, 0), 3));
        let r = crate::k2::EntityRecords {
            stops: vec![
                stop("A", 41.0, 29.0),
                stop("B", 41.1, 29.0),
                stop("C", 41.2, 29.0),
                stop("D", 41.3, 29.0),
                stop("E", 41.4, 29.0),
                stop("F", 41.5, 29.0),
                stop("P", 41.0003, 29.0),
                stop("Q", 41.0005, 29.0), // ~33 m from A: same physical place
            ],
            routes: vec![route("R1", 3), route("R2", 3)],
            trips: trips_v,
            trip_interns: take_ti(),
            stop_times: st_v,
            ..Default::default()
        };
        let result = analyze(&r, &empty_derived(), &default_config(), 20260514);
        assert!(
            !result.notices.iter().any(|n| n.rule_id == "VAT_005"),
            "İzole küme ana şebekeye <200m yakın → VAT_005 bastırılmalı"
        );
    }

    #[test]
    fn vat_005_canonical_components_match_bfs_and_dsu_order() {
        // Even if BFS/DSU traversal order differs for the same connected components, canonical
        // sorting must select the same main component, isolated components, and self-loop.
        let bfs = vec![
            vec!["B3", "B1", "B2"],
            vec!["Z2", "Z1"],
            vec!["A3", "A1", "A2"],
            vec!["LOOP"],
        ];
        let dsu = vec![
            vec!["A2", "A3", "A1"],
            vec!["LOOP"],
            vec!["Z1", "Z2"],
            vec!["B2", "B1", "B3"],
        ];

        let bfs = canonicalize_stop_components(bfs);
        let dsu = canonicalize_stop_components(dsu);
        assert_eq!(bfs, dsu);
        assert_eq!(bfs[0], vec!["A1", "A2", "A3"]);
        assert_eq!(bfs[1], vec!["B1", "B2", "B3"]);
        assert_eq!(bfs[2], vec!["Z1", "Z2"]);
        assert_eq!(bfs[3], vec!["LOOP"]);
    }

    #[test]
    fn vat_005_self_loop_is_an_active_single_node_component() {
        // A self-loop makes the node visible in the old HashSet graph; DSU must also mark the
        // same node as active and must not omit it from the connected-component calculation.
        let mut trips_v = Vec::new();
        let mut st_v = Vec::new();
        for i in 1..=12u32 {
            let tid = format!("M{i}");
            trips_v.push(trip(&tid, "R1"));
            st_v.push(stoptime(&tid, 1, "A", (i, 0, 0), (i, 0, 0), 2));
            st_v.push(stoptime(&tid, 2, "B", (i, 5, 0), (i, 5, 0), 3));
            st_v.push(stoptime(&tid, 3, "C", (i, 10, 0), (i, 10, 0), 4));
            st_v.push(stoptime(&tid, 4, "D", (i, 15, 0), (i, 15, 0), 5));
            st_v.push(stoptime(&tid, 5, "E", (i, 20, 0), (i, 20, 0), 6));
        }
        trips_v.push(trip("LOOP", "R2"));
        st_v.push(stoptime("LOOP", 1, "X", (8, 0, 0), (8, 0, 0), 2));
        st_v.push(stoptime("LOOP", 2, "X", (8, 1, 0), (8, 1, 0), 3));

        let r = crate::k2::EntityRecords {
            stops: vec![
                stop("A", 41.0, 29.0),
                stop("B", 41.1, 29.0),
                stop("C", 41.2, 29.0),
                stop("D", 41.3, 29.0),
                stop("E", 41.4, 29.0),
                stop("X", 42.0, 30.0),
            ],
            routes: vec![route("R1", 3), route("R2", 3)],
            trips: trips_v,
            trip_interns: take_ti(),
            stop_times: st_v,
            ..Default::default()
        };
        let result = analyze(&r, &empty_derived(), &default_config(), 20260514);
        let vat005 = result.notices.iter().find(|n| n.rule_id == "VAT_005");
        assert!(vat005.is_some(), "self-loop izole bileşeni VAT_005 üretmeli");
        assert!(
            vat005
                .and_then(|n| n.details.as_ref())
                .and_then(|d| d.get("isolated_stops"))
                .is_some_and(|stops| stops == "X"),
            "self-loop bileşeninin tek düğümü details içinde korunmalı"
        );
    }

    #[test]
    fn dominant_route_produces_vat_006() {
        // R1: 80 trips, R2: 10 trips, R3: 10 trips → R1 = 80% → VAT_006
        let mut trips_v = Vec::new();
        let mut st_v = Vec::new();
        for i in 1..=80u32 {
            let tid = format!("T1_{i}");
            trips_v.push(trip(&tid, "R1"));
            st_v.push(stoptime(&tid, 1, "A", (i % 24, 0, 0), (i % 24, 0, 0), 2));
            st_v.push(stoptime(&tid, 2, "B", (i % 24, 10, 0), (i % 24, 10, 0), 3));
        }
        for i in 1..=10u32 {
            let tid2 = format!("T2_{i}");
            let tid3 = format!("T3_{i}");
            trips_v.push(trip(&tid2, "R2"));
            trips_v.push(trip(&tid3, "R3"));
            st_v.push(stoptime(&tid2, 1, "A", (i, 30, 0), (i, 30, 0), 2));
            st_v.push(stoptime(&tid2, 2, "B", (i, 40, 0), (i, 40, 0), 3));
            st_v.push(stoptime(&tid3, 1, "A", (i, 45, 0), (i, 45, 0), 2));
            st_v.push(stoptime(&tid3, 2, "B", (i, 55, 0), (i, 55, 0), 3));
        }
        let r = crate::k2::EntityRecords {
            stops: vec![stop("A", 41.0, 29.0), stop("B", 41.1, 29.1)],
            routes: vec![route("R1", 3), route("R2", 3), route("R3", 3)],
            trips: trips_v,
            trip_interns: take_ti(),
            stop_times: st_v,
            ..Default::default()
        };
        let result = analyze(&r, &empty_derived(), &default_config(), 20260514);
        assert!(
            result.notices.iter().any(|n| n.rule_id == "VAT_006"),
            "VAT_006 olmalı"
        );
    }

    #[test]
    fn two_route_feed_has_no_vat_006() {
        // With 2 routes, the largest share is necessarily ≥50% → a 40% threshold carries no information.
        let mut trips_v = Vec::new();
        let mut st_v = Vec::new();
        for i in 1..=80u32 {
            let (tid, rid) = (format!("T{i}"), if i <= 70 { "R1" } else { "R2" });
            trips_v.push(trip(&tid, rid));
            st_v.push(stoptime(&tid, 1, "A", (i % 24, 0, 0), (i % 24, 0, 0), 2));
            st_v.push(stoptime(&tid, 2, "B", (i % 24, 10, 0), (i % 24, 10, 0), 3));
        }
        let r = crate::k2::EntityRecords {
            stops: vec![stop("A", 41.0, 29.0), stop("B", 41.1, 29.1)],
            routes: vec![route("R1", 3), route("R2", 3)],
            trips: trips_v,
            trip_interns: take_ti(),
            stop_times: st_v,
            ..Default::default()
        };
        let result = analyze(&r, &empty_derived(), &default_config(), 20260514);
        assert!(
            !result.notices.iter().any(|n| n.rule_id == "VAT_006"),
            "1–2 hatlı feed'de VAT_006 olmamalı"
        );
    }

    #[test]
    fn shared_terminus_no_transfer_produces_vat_007() {
        // 3 routes start at HUB, AND a route not serving HUB (R4) passes through HUB2, ~200 m
        // away → a genuine inter-stop transfer gap → VAT_007.
        let r = crate::k2::EntityRecords {
            stops: vec![
                stop("HUB", 41.0, 29.0),
                stop("HUB2", 41.0018, 29.0),
                stop("A", 41.1, 29.0),
                stop("B", 41.0, 29.1),
                stop("C", 41.0, 28.9),
                stop("D", 41.2, 29.2),
            ],
            routes: (1..=4).map(|i| route(&format!("R{i}"), 3)).collect(),
            trips: (1..=4)
                .map(|i| trip(&format!("T{i}"), &format!("R{i}")))
                .collect(),
            trip_interns: take_ti(),
            stop_times: vec![
                stoptime("T1", 1, "HUB", (8, 0, 0), (8, 0, 0), 2),
                stoptime("T1", 2, "A", (8, 10, 0), (8, 10, 0), 3),
                stoptime("T2", 1, "HUB", (8, 0, 0), (8, 0, 0), 2),
                stoptime("T2", 2, "B", (8, 10, 0), (8, 10, 0), 3),
                stoptime("T3", 1, "HUB", (8, 0, 0), (8, 0, 0), 2),
                stoptime("T3", 2, "C", (8, 10, 0), (8, 10, 0), 3),
                // R4 serves only neighboring HUB2.
                stoptime("T4", 1, "HUB2", (9, 0, 0), (9, 0, 0), 4),
                stoptime("T4", 2, "D", (9, 10, 0), (9, 10, 0), 5),
            ],
            ..Default::default()
        };
        // transfers is empty
        let result = analyze(&r, &empty_derived(), &default_config(), 20260514);
        assert!(
            result.notices.iter().any(|n| n.rule_id == "VAT_007"),
            "VAT_007 olmalı"
        );
    }

    /// If all routes use the same stop_id at the terminal, the transfer is implicit → silent.
    #[test]
    fn vat_007_silent_when_terminus_routes_share_one_stop_id() {
        let r = crate::k2::EntityRecords {
            stops: vec![
                stop("GAR", 41.0, 29.0),
                stop("A", 41.1, 29.0),
                stop("B", 41.0, 29.1),
                stop("C", 41.0, 28.9),
            ],
            routes: (1..=3).map(|i| route(&format!("R{i}"), 2)).collect(),
            trips: (1..=3)
                .map(|i| trip(&format!("T{i}"), &format!("R{i}")))
                .collect(),
            trip_interns: take_ti(),
            stop_times: vec![
                stoptime("T1", 1, "GAR", (8, 0, 0), (8, 0, 0), 2),
                stoptime("T1", 2, "A", (8, 10, 0), (8, 10, 0), 3),
                stoptime("T2", 1, "GAR", (8, 0, 0), (8, 0, 0), 2),
                stoptime("T2", 2, "B", (8, 10, 0), (8, 10, 0), 3),
                stoptime("T3", 1, "GAR", (8, 0, 0), (8, 0, 0), 2),
                stoptime("T3", 2, "C", (8, 10, 0), (8, 10, 0), 3),
            ],
            ..Default::default()
        };
        let result = analyze(&r, &empty_derived(), &default_config(), 20260514);
        assert!(
            !result.notices.iter().any(|n| n.rule_id == "VAT_007"),
            "tek stop_id'li terminalde aktarma örtüktür → VAT_007 çıkmamalı"
        );
    }

    // ── Missing EntityMap ────────────────────────────────────────────────────

    fn _uses_entity_map(_: &EntityMap) {}

    #[test]
    fn adjacent_repeated_stop_is_owned_by_stm_035_not_opr_007() {
        let records = records_with(
            vec![stop("A", 41.0, 29.0), stop("B", 41.1, 29.1)],
            vec![route("R1", 3)],
            vec![trip("T1", "R1")],
            vec![
                stoptime("T1", 1, "A", (8, 0, 0), (8, 0, 0), 2),
                stoptime("T1", 2, "A", (8, 1, 0), (8, 1, 0), 3),
                stoptime("T1", 3, "B", (8, 10, 0), (8, 10, 0), 4),
            ],
        );
        let result = analyze(&records, &empty_derived(), &default_config(), 20260514);
        assert!(result.notices.iter().any(|n| n.rule_id == "STM_035"));
        assert!(
            !result.notices.iter().any(|n| n.rule_id == "OPR_007"),
            "ardışık tekrar OPR_007 ile ikinci kez raporlanmamalı"
        );
    }

    #[test]
    fn late_night_trips_are_summarized_once_per_route() {
        let records = records_with(
            vec![stop("A", 41.0, 29.0), stop("B", 41.1, 29.1)],
            vec![route("R1", 3)],
            vec![trip("T1", "R1"), trip("T2", "R1")],
            vec![
                stoptime("T1", 1, "A", (23, 0, 0), (23, 0, 0), 2),
                stoptime("T1", 2, "B", (23, 10, 0), (23, 10, 0), 3),
                stoptime("T2", 1, "A", (24, 0, 0), (24, 0, 0), 4),
                stoptime("T2", 2, "B", (24, 10, 0), (24, 10, 0), 5),
            ],
        );
        let result = analyze(&records, &empty_derived(), &default_config(), 20260514);
        let found: Vec<_> = result
            .notices
            .iter()
            .filter(|n| n.rule_id == "OPR_009")
            .collect();
        assert_eq!(found.len(), 1);
        assert_eq!(found[0].entity_type, EntityType::Route);
        assert!(found[0]
            .observed_value
            .as_deref()
            .unwrap_or("")
            .contains("2 trips"));
    }

    // ── ARC_020 · DRT muafiyeti ───────────────────────────────────────────────
    // shapes.txt is for fixed-route geometry and is not expected in a fully demand-responsive
    // feed. The exemption is intentionally strict: if even one fixed-stop trip exists,
    // shapes.txt remains expected.

    /// DRT counterpart of the `stoptime` helper: stop_id is empty and the service area is
    /// provided through the Flex field.
    fn drt_stoptime(
        trip_id: &str,
        seq: u32,
        location_id: Option<&str>,
        location_group_id: Option<&str>,
        line: u64,
    ) -> StopTimeRecord {
        StopTimeRecord {
            trip_id: trip_id.into(),
            stop_id: "".into(),
            stop_sequence: Some(seq),
            location_id: location_id.map(|s| s.into()),
            location_group_id: location_group_id.map(|s| s.into()),
            line,
            ..Default::default()
        }
    }

    fn feed_info_row() -> crate::k2::feed_info::FeedInfoRecord {
        crate::k2::feed_info::FeedInfoRecord {
            feed_publisher_name: "P".into(),
            feed_publisher_url: "https://example.org".into(),
            feed_lang: "tr".into(),
            feed_start_date: None,
            feed_end_date: None,
            feed_version: None,
            feed_contact_email: None,
            feed_contact_url: None,
            row: Default::default(),
            line: 2,
        }
    }

    fn records_with_feed_end(end: Option<(u32, u32, u32)>) -> crate::k2::EntityRecords {
        let mut records = records_with(vec![], vec![], vec![], vec![]);
        let mut feed_info = feed_info_row();
        feed_info.feed_end_date = end;
        records.feed_info = vec![feed_info];
        records
    }

    /// `feed_end_date == today` was seen by NO rule (#146).
    ///
    /// `FIN_010` required `end < today`, while `FIN_019` required `days_left > 0`; both
    /// were silent exactly at the boundary. Two real corpus cases: `mdb-923` and `tld-366`.
    /// (`feedEndDate = currentDate = 20260816`), MobilityData ikisini de raporluyor.
    ///
    /// "Expires today" logically means EXPIRING, not EXPIRED: the feed is still valid today,
    /// but not tomorrow. Therefore the case belongs to `FIN_019`, not `FIN_010`.
    #[test]
    fn a_feed_expiring_exactly_today_is_reported() {
        let records = records_with_feed_end(Some((2026, 5, 14)));
        let result = analyze(&records, &empty_derived(), &default_config(), 20260514);

        let fin019 = result
            .notices
            .iter()
            .filter(|n| n.rule_id == "FIN_019")
            .count();
        let fin010 = result
            .notices
            .iter()
            .filter(|n| n.rule_id == "FIN_010")
            .count();
        assert_eq!(fin019, 1, "bugün dolan feed FIN_019 üretmeli (0 gün kaldı)");
        assert_eq!(
            fin010, 0,
            "bugün dolan feed HENÜZ dolmamıştır → FIN_010 ÜRETMEMELİ"
        );

        // The other side of the boundary is unchanged: a feed that expired yesterday still gets
        // only FIN_010.
        let expired = records_with_feed_end(Some((2026, 5, 13)));
        let r2 = analyze(&expired, &empty_derived(), &default_config(), 20260514);
        assert_eq!(
            r2.notices.iter().filter(|n| n.rule_id == "FIN_010").count(),
            1
        );
        assert_eq!(
            r2.notices.iter().filter(|n| n.rule_id == "FIN_019").count(),
            0,
            "dolmuş feed FIN_019 ALMAMALI — iki kural ayrık kalmalı"
        );
    }

    #[test]
    fn fin_019_uses_separate_configurable_feed_info_horizon() {
        let cases = [
            ("3 days", (2026, 5, 17), true),
            ("10 days", (2026, 5, 24), false),
            ("29 days", (2026, 6, 12), false),
            ("31 days", (2026, 6, 14), false),
        ];
        for (label, end, expected) in cases {
            let records = records_with_feed_end(Some(end));
            let result = analyze(&records, &empty_derived(), &default_config(), 20260514);
            assert_eq!(
                result
                    .notices
                    .iter()
                    .filter(|n| n.rule_id == "FIN_019")
                    .count()
                    > 0,
                expected,
                "default 7-day FIN_019 behavior for {label}"
            );
        }

        let mut config = default_config();
        config.feed_info_expiry_warning_days = 30;
        for (label, end, expected) in [
            ("10 days", (2026, 5, 24), true),
            ("29 days", (2026, 6, 12), true),
            ("31 days", (2026, 6, 14), false),
        ] {
            let records = records_with_feed_end(Some(end));
            let result = analyze(&records, &empty_derived(), &config, 20260514);
            let notices: Vec<_> = result
                .notices
                .iter()
                .filter(|n| n.rule_id == "FIN_019")
                .collect();
            assert_eq!(
                !notices.is_empty(),
                expected,
                "30-day FIN_019 behavior for {label}"
            );
            if expected {
                let details = notices[0].details.as_ref().expect("FIN_019 details");
                assert_eq!(details.get("warning_days").map(String::as_str), Some("30"));
                assert_eq!(
                    details.get("days_left").map(String::as_str),
                    Some(if label == "10 days" { "10" } else { "29" })
                );
            }
        }
    }

    #[test]
    fn fin_019_excludes_expired_and_missing_feed_end_dates() {
        let expired = analyze(
            &records_with_feed_end(Some((2026, 5, 13))),
            &empty_derived(),
            &default_config(),
            20260514,
        );
        assert!(expired.notices.iter().any(|n| n.rule_id == "FIN_010"));
        assert!(expired.notices.iter().all(|n| n.rule_id != "FIN_019"));

        let missing = analyze(
            &records_with_feed_end(None),
            &empty_derived(),
            &default_config(),
            20260514,
        );
        assert!(missing.notices.iter().all(|n| n.rule_id != "FIN_019"));
    }

    #[test]
    fn unavailable_feed_info_does_not_produce_feed_expiry_findings() {
        let records = records_with_feed_end(Some((2026, 5, 17)));
        let present = std::collections::HashSet::from(["feed_info.txt".to_string()]);
        let unavailable = vec!["feed_info.txt".to_string()];
        let availability = FileAvailability::from_k1(&present, &unavailable);
        let result = analyze_with_files(
            &records,
            &empty_derived(),
            &default_config(),
            20260514,
            &availability,
        );

        assert!(result.notices.iter().all(|n| {
            !matches!(
                n.rule_id.as_str(),
                "FIN_010" | "FIN_016" | "FIN_017" | "FIN_018" | "FIN_019" | "FIN_020" | "CAL_020"
            )
        }));
    }

    fn arc_020_files(records: &crate::k2::EntityRecords) -> Vec<String> {
        let result = analyze(records, &empty_derived(), &default_config(), 20260514);
        result
            .notices
            .iter()
            .filter(|n| n.rule_id == "ARC_020")
            .map(|n| n.file.clone().unwrap_or_default())
            .collect()
    }

    #[test]
    fn arc_020_reports_missing_shapes_on_fixed_stop_feed() {
        let mut records = records_with(
            vec![stop("A", 41.0, 29.0), stop("B", 41.1, 29.1)],
            vec![route("R1", 3)],
            vec![trip("T1", "R1")],
            vec![
                stoptime("T1", 1, "A", (8, 0, 0), (8, 0, 0), 2),
                stoptime("T1", 2, "B", (8, 10, 0), (8, 10, 0), 3),
            ],
        );
        records.feed_info = vec![feed_info_row()];
        assert_eq!(
            arc_020_files(&records),
            vec!["shapes.txt".to_string()],
            "sabit duraklı feed'de shapes.txt eksikliği raporlanmalı"
        );
    }

    #[test]
    fn arc_020_skips_shapes_for_zone_based_drt() {
        use crate::k2::stop_times::StopTimesIndex;
        let stoptimes = vec![
            drt_stoptime("T1", 1, Some("zone_1"), None, 2),
            drt_stoptime("T1", 2, Some("zone_2"), None, 3),
        ];
        let mut records = records_with(
            vec![stop("A", 41.0, 29.0)],
            vec![route("R1", 3)],
            vec![trip("T1", "R1")],
            stoptimes.clone(),
        );
        records.stop_times_index = StopTimesIndex::from_records(&stoptimes);
        records.feed_info = vec![feed_info_row()];
        assert!(
            arc_020_files(&records).is_empty(),
            "alan-tabanlı DRT feed'inde shapes.txt beklenmez"
        );
    }

    #[test]
    fn arc_020_skips_shapes_for_fixed_stops_drt() {
        use crate::k2::stop_times::StopTimesIndex;
        let stoptimes = vec![
            drt_stoptime("T1", 1, None, Some("lg_1"), 2),
            drt_stoptime("T1", 2, None, Some("lg_2"), 3),
        ];
        let mut records = records_with(
            vec![stop("A", 41.0, 29.0)],
            vec![route("R1", 3)],
            vec![trip("T1", "R1")],
            stoptimes.clone(),
        );
        records.stop_times_index = StopTimesIndex::from_records(&stoptimes);
        records.feed_info = vec![feed_info_row()];
        records.location_groups = vec![crate::k2::location_groups::LocationGroupRecord {
            location_group_id: "lg_1".into(),
            line: 2,
        }];
        assert!(
            arc_020_files(&records).is_empty(),
            "sabit-duraklı DRT feed'inde shapes.txt beklenmez"
        );
    }

    #[test]
    fn arc_020_still_reports_missing_shapes_when_drt_is_partial() {
        use crate::k2::stop_times::StopTimesIndex;
        // One fixed-stop trip → the feed is not DRT-only; shapes.txt remains expected.
        let stoptimes = vec![
            drt_stoptime("T1", 1, Some("zone_1"), None, 2),
            stoptime("T2", 1, "A", (8, 0, 0), (8, 0, 0), 3),
            stoptime("T2", 2, "B", (8, 10, 0), (8, 10, 0), 4),
        ];
        let mut records = records_with(
            vec![stop("A", 41.0, 29.0), stop("B", 41.1, 29.1)],
            vec![route("R1", 3)],
            vec![trip("T1", "R1"), trip("T2", "R1")],
            stoptimes.clone(),
        );
        records.stop_times_index = StopTimesIndex::from_records(&stoptimes);
        records.feed_info = vec![feed_info_row()];
        assert_eq!(
            arc_020_files(&records),
            vec!["shapes.txt".to_string()],
            "karışık feed'de shapes.txt eksikliği raporlanmalı"
        );
    }

    #[test]
    fn arc_020_still_reports_missing_feed_info_on_drt_feed() {
        use crate::k2::stop_times::StopTimesIndex;
        // The DRT exemption removes ONLY the shapes.txt requirement; feed_info.txt remains expected.
        let stoptimes = vec![drt_stoptime("T1", 1, Some("zone_1"), None, 2)];
        let mut records = records_with(
            vec![stop("A", 41.0, 29.0)],
            vec![route("R1", 3)],
            vec![trip("T1", "R1")],
            stoptimes.clone(),
        );
        records.stop_times_index = StopTimesIndex::from_records(&stoptimes);
        assert_eq!(
            arc_020_files(&records),
            vec!["feed_info.txt".to_string()],
            "DRT feed'inde bile feed_info.txt eksikliği raporlanmalı"
        );
    }
}
