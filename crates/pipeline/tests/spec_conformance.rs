//! WP-1 — Spec conformance gate: the **mirror of emit_proof**.
//!
//! emit_proof.rs asks "can every rule fire?" (searching for false NEGATIVES).
//! This test does the reverse: **a spec-conformant feed must produce no `Spec`-class notices
//! (searching for false POSITIVES). The 2026-08-01 spec conformance pass
//! would have caught all THREE fixes: FPD_002 / STM_006 / ARC_004.
//!
//! Second (free) gate: the same corpus must also have no `Kritik` severity —
//! it catches XFL_031 + LOC_010 being pushed without measurement.
//!
//! OUT OF SCOPE: the "zero notices" target. The base feed currently produces 23 notices,
//! mostly legitimate Quality/Analytics findings (missing feed_info, missing shapes…).

use std::collections::BTreeSet;
use std::io::Write as _;
use zip::write::SimpleFileOptions;

use gtfs_core::{Notice, RuleClass, Severity};
use gtfs_pipeline::{validate_bytes, ValidateResult, ValidatorConfig};

const TODAY: u32 = 20_260_515;

// ── Valid baseline feed (same body as emit_proof.rs) ──────────────────────────
const AGENCY: &str =
    "agency_id,agency_name,agency_url,agency_timezone\n1,Test,http://test.example,UTC\n";
const STOPS: &str = "stop_id,stop_name,stop_lat,stop_lon\nS1,Stop1,41.0,29.0\nS2,Stop2,41.1,29.1\n";
const ROUTES: &str = "route_id,agency_id,route_short_name,route_type\nR1,1,101,3\n";
const TRIPS: &str = "route_id,service_id,trip_id\nR1,SVC1,T1\n";
const STOP_TIMES: &str = "trip_id,arrival_time,departure_time,stop_id,stop_sequence\nT1,08:00:00,08:00:00,S1,1\nT1,08:10:00,08:10:00,S2,2\n";
const CALENDAR: &str = "service_id,monday,tuesday,wednesday,thursday,friday,saturday,sunday,start_date,end_date\nSVC1,1,1,1,1,1,0,0,20250101,20271231\n";

fn base() -> Vec<(String, Vec<u8>)> {
    [
        ("agency.txt", AGENCY),
        ("stops.txt", STOPS),
        ("routes.txt", ROUTES),
        ("trips.txt", TRIPS),
        ("stop_times.txt", STOP_TIMES),
        ("calendar.txt", CALENDAR),
    ]
    .iter()
    .map(|(n, c)| (n.to_string(), c.as_bytes().to_vec()))
    .collect()
}

fn with_opts(overrides: &[(&str, &str)], removes: &[&str]) -> Vec<(String, Vec<u8>)> {
    let mut files = base();
    for (name, content) in overrides {
        let bytes = content.as_bytes().to_vec();
        if let Some(slot) = files.iter_mut().find(|(n, _)| n == name) {
            slot.1 = bytes;
        } else {
            files.push((name.to_string(), bytes));
        }
    }
    files.retain(|(n, _)| !removes.contains(&n.as_str()));
    files
}

fn make_zip(files: &[(String, Vec<u8>)]) -> Vec<u8> {
    let cur = std::io::Cursor::new(Vec::new());
    let mut zw = zip::ZipWriter::new(cur);
    for (name, data) in files {
        zw.start_file(name, SimpleFileOptions::default()).unwrap();
        zw.write_all(data).unwrap();
    }
    zw.finish().unwrap().into_inner()
}

/// Validates the feed; fails the test on Fatal (a valid feed must not be Fatal).
fn notices_of(feed: &ValidFeed) -> Vec<Notice> {
    let files = with_opts(&feed.overrides, &feed.removes);
    match validate_bytes(&make_zip(&files), &ValidatorConfig::default(), TODAY) {
        ValidateResult::Ok(vr) => vr.notices,
        ValidateResult::Fatal(e) => {
            panic!("'{}' GEÇERLİ bir feed ama Fatal döndü: {:?}", feed.name, e,)
        }
    }
}

/// Rule IDs produced by the canary (a variant with an intentional violation).
fn canary_rules(feed: &ValidFeed) -> BTreeSet<String> {
    let mut ov = feed.overrides.clone();
    for (name, content) in &feed.canary_overrides {
        if let Some(slot) = ov.iter_mut().find(|(n, _)| n == name) {
            slot.1 = content;
        } else {
            ov.push((name, content));
        }
    }
    let files = with_opts(&ov, &feed.removes);
    match validate_bytes(&make_zip(&files), &ValidatorConfig::default(), TODAY) {
        ValidateResult::Ok(vr) => vr.notices.iter().map(|n| n.rule_id.clone()).collect(),
        // The canary may be Fatal (e.g. ARC_004) — then there is no rule_id, so the set is empty.
        ValidateResult::Fatal(_) => BTreeSet::new(),
    }
}

/// A spec-CONFORMANT feed + a canary proving it does not silently pass.
struct ValidFeed {
    name: &'static str,
    overrides: Vec<(&'static str, &'static str)>,
    removes: Vec<&'static str>,
    /// Overrides that add an intentional violation to the fixture's distinguishing file.
    /// `canary_rule` MUST APPEAR as a result — if it does not, the file is silently skipped
    /// and the original fixture passing proves nothing.
    canary_overrides: Vec<(&'static str, &'static str)>,
    canary_rule: &'static str,
}

// ── Flex helpers ──────────────────────────────────────────────────────────────
const GEOJSON_L1_L2: &str = r#"{"type":"FeatureCollection","features":[
{"type":"Feature","id":"L1","properties":{},"geometry":{"type":"Polygon","coordinates":[[[29.00,41.00],[29.00,41.01],[29.01,41.01],[29.00,41.00]]]}},
{"type":"Feature","id":"L2","properties":{},"geometry":{"type":"Polygon","coordinates":[[[29.10,41.10],[29.10,41.11],[29.11,41.11],[29.10,41.10]]]}}]}"#;

const ST_LOCATION_ID: &str = "trip_id,stop_sequence,location_id,start_pickup_drop_off_window,end_pickup_drop_off_window,pickup_type,drop_off_type\nT1,1,L1,09:00:00,10:00:00,2,2\nT1,2,L2,10:00:00,11:00:00,2,2\n";
const ST_LOCATION_GROUP: &str = "trip_id,stop_sequence,location_group_id,start_pickup_drop_off_window,end_pickup_drop_off_window,pickup_type,drop_off_type\nT1,1,LG1,09:00:00,10:00:00,2,2\nT1,2,LG2,10:00:00,11:00:00,2,2\n";
const LOCATION_GROUPS: &str =
    "location_group_id,location_group_name\nLG1,Bolge Bir\nLG2,Bolge Iki\n";
const LOCATION_GROUP_STOPS: &str = "location_group_id,stop_id\nLG1,S1\nLG2,S2\n";

fn valid_feeds() -> Vec<ValidFeed> {
    vec![
        // 1. Classic feed: stop_times uses only stop_id.
        ValidFeed {
            name: "stop_id_only",
            overrides: vec![],
            removes: vec![],
            canary_overrides: vec![("stop_times.txt", "trip_id,arrival_time,departure_time,stop_id,stop_sequence\nT1,08:00:00,08:00:00,,1\nT1,08:10:00,08:10:00,S2,2\n")],
            canary_rule: "STM_006",
        },
        // 2. Flex: location_id only (NO stop_id) — bb2c1fc1 fix.
        ValidFeed {
            name: "location_id_only",
            overrides: vec![("stop_times.txt", ST_LOCATION_ID), ("locations.geojson", GEOJSON_L1_L2)],
            removes: vec![],
            canary_overrides: vec![("stop_times.txt", "trip_id,stop_sequence,location_id,start_pickup_drop_off_window,end_pickup_drop_off_window,pickup_type,drop_off_type\nT1,1,NOPE,09:00:00,10:00:00,2,2\nT1,2,L2,10:00:00,11:00:00,2,2\n")],
            canary_rule: "XFL_025",
        },
        // 3. Flex: location_group_id only.
        ValidFeed {
            name: "location_group_id_only",
            overrides: vec![
                ("stop_times.txt", ST_LOCATION_GROUP),
                ("location_groups.txt", LOCATION_GROUPS),
                ("location_group_stops.txt", LOCATION_GROUP_STOPS),
            ],
            removes: vec![],
            canary_overrides: vec![("stop_times.txt", "trip_id,stop_sequence,location_group_id,start_pickup_drop_off_window,end_pickup_drop_off_window,pickup_type,drop_off_type\nT1,1,NOPE,09:00:00,10:00:00,2,2\nT1,2,LG2,10:00:00,11:00:00,2,2\n")],
            canary_rule: "XFL_024",
        },
        // 4. Flex-only feed: locations.geojson present, stops.txt ABSENT — ARC_004 (83f83d83).
        ValidFeed {
            name: "flex_without_stops_txt",
            overrides: vec![("stop_times.txt", ST_LOCATION_ID), ("locations.geojson", GEOJSON_L1_L2)],
            removes: vec!["stops.txt"],
            canary_overrides: vec![("locations.geojson", "{\"type\":\"FeatureCollection\",\"features\":[]}")],
            canary_rule: "LOC_005",
        },
        // 5. Negative fare amount = transfer discount, valid — FPD_002 (195ecac3).
        ValidFeed {
            name: "negative_fare_amount",
            overrides: vec![("fare_products.txt", "fare_product_id,amount,currency\nP1,-1.50,USD\n")],
            removes: vec![],
            canary_overrides: vec![("fare_products.txt", "fare_product_id,amount,currency\nP1,abc,USD\n")],
            canary_rule: "FPD_002",
        },
        // 6. routes.network_id alone (NO networks/route_networks) — XFL_019 (2f674946).
        ValidFeed {
            name: "routes_network_id_alone",
            overrides: vec![("routes.txt", "route_id,agency_id,route_short_name,route_type,network_id\nR1,1,101,3,N1\n")],
            removes: vec![],
            canary_overrides: vec![("route_networks.txt", "network_id,route_id\nN1,R1\n")],
            canary_rule: "XFL_019",
        },
        // 7. No calendar.txt; service defined only through calendar_dates.txt (valid in the spec).
        ValidFeed {
            name: "calendar_dates_only",
            overrides: vec![("calendar_dates.txt", CALENDAR_DATES_ONLY)],
            removes: vec!["calendar.txt"],
            canary_overrides: vec![("calendar_dates.txt", "service_id,date,exception_type\nSVC1,20260501,5\n")],
            canary_rule: "CLD_003",
        },
        // 8. Rail feed, departure after midnight (38:10) — valid in GTFS.
        ValidFeed {
            name: "rail_after_midnight",
            overrides: vec![
                ("routes.txt", "route_id,agency_id,route_short_name,route_type\nR1,1,101,2\n"),
                ("stop_times.txt", "trip_id,arrival_time,departure_time,stop_id,stop_sequence\nT1,38:00:00,38:00:00,S1,1\nT1,38:10:00,38:10:00,S2,2\n"),
            ],
            removes: vec![],
            canary_overrides: vec![("stop_times.txt", "trip_id,arrival_time,departure_time,stop_id,stop_sequence\nT1,38:00:00,38:00:00,S1,1\nT1,37:10:00,37:10:00,S2,2\n")],
            canary_rule: "STM_008",
        },
    ]
}

// Make the service-day window wide enough around TODAY (20260515).
const CALENDAR_DATES_ONLY: &str = "service_id,date,exception_type\n\
SVC1,20260501,1\nSVC1,20260504,1\nSVC1,20260505,1\nSVC1,20260506,1\nSVC1,20260507,1\n\
SVC1,20260508,1\nSVC1,20260511,1\nSVC1,20260512,1\nSVC1,20260513,1\nSVC1,20260514,1\n\
SVC1,20260515,1\nSVC1,20260518,1\nSVC1,20260519,1\nSVC1,20260520,1\nSVC1,20260521,1\n\
SVC1,20260522,1\nSVC1,20260525,1\nSVC1,20260526,1\nSVC1,20260527,1\nSVC1,20260528,1\n\
SVC1,20260529,1\nSVC1,20260601,1\nSVC1,20260602,1\nSVC1,20260603,1\nSVC1,20260604,1\n";

#[test]
#[ignore = "development helper: cargo test --test spec_conformance dump -- --ignored --nocapture"]
fn dump() {
    for feed in valid_feeds() {
        let ns = notices_of(&feed);
        println!("\n=== {} ({} notice) ===", feed.name, ns.len());
        let mut rows: Vec<String> = ns
            .iter()
            .map(|n| {
                format!(
                    "  {:?} {:?} {} [{}:{}]",
                    n.rule_class,
                    n.severity,
                    n.rule_id,
                    n.file.clone().unwrap_or_default(),
                    n.field.clone().unwrap_or_default()
                )
            })
            .collect();
        rows.sort();
        rows.dedup();
        for r in rows {
            println!("{r}");
        }
        println!(
            "  canary({}) → {}",
            feed.canary_rule,
            if canary_rules(&feed).contains(feed.canary_rule) {
                "OK"
            } else {
                "KAÇIRDI"
            }
        );
    }
}

/// WP-1 MAIN GATE: a spec-conformant feed must produce no `Spec`-class notices.
#[test]
fn valid_feeds_emit_no_spec_notice() {
    let mut failures = Vec::new();
    for feed in valid_feeds() {
        let mut hits: Vec<String> = notices_of(&feed)
            .iter()
            .filter(|n| n.rule_class == RuleClass::Spec)
            .map(|n| {
                format!(
                    "{} ({}:{})",
                    n.rule_id,
                    n.file.clone().unwrap_or_default(),
                    n.field.clone().unwrap_or_default()
                )
            })
            .collect();
        hits.sort();
        hits.dedup();
        if !hits.is_empty() {
            failures.push(format!("  {} → {}", feed.name, hits.join(", ")));
        }
    }
    assert!(
        failures.is_empty(),
        "Spec'e uygun feed'lerde Spec sınıfı notice çıktı (yanlış pozitif):\n{}",
        failures.join("\n")
    );
}

/// Second gate: the same corpus must also have no `Kritik` severity.
#[test]
fn valid_feeds_emit_no_critical_notice() {
    let mut failures = Vec::new();
    for feed in valid_feeds() {
        let mut hits: Vec<String> = notices_of(&feed)
            .iter()
            .filter(|n| n.severity == Severity::Kritik)
            .map(|n| format!("{} [{:?}]", n.rule_id, n.rule_class))
            .collect();
        hits.sort();
        hits.dedup();
        if !hits.is_empty() {
            failures.push(format!("  {} → {}", feed.name, hits.join(", ")));
        }
    }
    assert!(
        failures.is_empty(),
        "Spec'e uygun feed'lerde Kritik notice çıktı:\n{}",
        failures.join("\n")
    );
}

/// Fixtures are not "passing empty": each feed's distinguishing file is actually read.
/// A canary is an intentional violation in the same feed; if the expected rule is absent,
/// the file is being silently skipped and the main gate proves nothing.
#[test]
fn every_valid_feed_is_actually_exercised() {
    let mut failures = Vec::new();
    for feed in valid_feeds() {
        let rules = canary_rules(&feed);
        if !rules.contains(feed.canary_rule) {
            failures.push(format!(
                "  {}: kanarya {} üretmedi → {:?}",
                feed.name, feed.canary_rule, rules
            ));
        }
    }
    assert!(
        failures.is_empty(),
        "Kanarya düşmedi — fixture boş geçiyor olabilir:\n{}",
        failures.join("\n")
    );
}
