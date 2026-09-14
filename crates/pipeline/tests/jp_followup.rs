use gtfs_core::{Notice, Severity};
use gtfs_pipeline::{validate_bytes, GtfsJpProfile, ValidateResult, ValidatorConfig};
use std::io::Write;
use zip::{write::SimpleFileOptions, ZipWriter};

fn feed(extra: &[(&str, &str)]) -> Vec<u8> {
    let mut files = std::collections::BTreeMap::from([
        ("agency.txt", "agency_id,agency_name,agency_url,agency_timezone,agency_lang\nA,Test,https://example.jp,Asia/Tokyo,ja\n"),
        ("stops.txt", "stop_id,stop_name,stop_lat,stop_lon,location_type,zone_id\nS1,One,35,139,0,Z\nS2,Two,35.001,139,0,\n"),
        ("routes.txt", "route_id,agency_id,route_short_name,route_type\nR1,A,1,3\n"),
        ("trips.txt", "route_id,service_id,trip_id\nR1,SVC,T1\n"),
        ("stop_times.txt", "trip_id,arrival_time,departure_time,stop_id,stop_sequence\nT1,08:00:00,08:00:00,S1,1\nT1,08:10:00,08:10:00,S2,2\n"),
        ("calendar.txt", "service_id,monday,tuesday,wednesday,thursday,friday,saturday,sunday,start_date,end_date\nSVC,1,1,1,1,1,1,1,20260101,20271231\n"),
        ("translations.txt", "table_name,field_name,language,translation,record_id\nstops,stop_name,en,One,S1\n"),
        ("feed_info.txt", "feed_publisher_name,feed_publisher_url,feed_lang,feed_start_date,feed_end_date,feed_version\nTest,https://example.jp,ja,20260101,20271231,test\n"),
    ]);
    files.extend(extra.iter().copied());
    let mut zip = ZipWriter::new(std::io::Cursor::new(Vec::new()));
    for (name, body) in files {
        zip.start_file(name, SimpleFileOptions::default()).unwrap();
        zip.write_all(body.as_bytes()).unwrap();
    }
    zip.finish().unwrap().into_inner()
}

fn validate(extra: &[(&str, &str)], profile: GtfsJpProfile) -> gtfs_core::ValidationResult {
    let result = validate_bytes(
        &feed(extra),
        &ValidatorConfig {
            gtfs_jp_profile: profile,
            ..Default::default()
        },
        20260914,
    );
    match result {
        ValidateResult::Ok(result) => result,
        other => panic!("{other:?}"),
    }
}

fn select<'a>(notices: &'a [Notice], rule: &str) -> Vec<&'a Notice> {
    notices.iter().filter(|n| n.rule_id == rule).collect()
}

#[test]
fn v3_route_type_constraint_is_explicit_and_skips_unparseable_values() {
    for (value, fires) in [
        ("2", true),
        ("700", true),
        ("999", true),
        ("3", false),
        ("", false),
        ("bad", false),
        ("-1", false),
    ] {
        let routes =
            format!("route_id,agency_id,route_short_name,route_type\nR1,A,1,3\nR2,A,2,{value}\n");
        for profile in [GtfsJpProfile::Auto, GtfsJpProfile::V3, GtfsJpProfile::V4] {
            let result = validate(&[("routes.txt", &routes)], profile);
            assert_eq!(
                !select(&result.notices, "JPN_027").is_empty(),
                fires && profile == GtfsJpProfile::V3,
                "{profile:?} {value}"
            );
        }
    }
}

#[test]
fn v3_route_type_constraint_is_silent_for_pure_non_bus_feeds() {
    let routes =
        "route_id,agency_id,route_short_name,route_type\nR1,A,rail,2\nR2,A,tram,0\nR3,A,ferry,4\n";
    let result = validate(&[("routes.txt", routes)], GtfsJpProfile::V3);
    assert!(select(&result.notices, "JPN_027").is_empty());
}

#[test]
fn v3_translation_pairs_aggregate_and_merge_duplicate_work() {
    let stop_times = "trip_id,arrival_time,departure_time,stop_id,stop_sequence,stop_headsign\nT1,08:00:00,08:00:00,S1,1,渋谷\nT1,08:10:00,08:10:00,S2,2,渋谷\n";
    let result = validate(&[("stop_times.txt", stop_times)], GtfsJpProfile::V3);
    let kana = select(&result.notices, "JPN_028")
        .into_iter()
        .filter(|notice| notice.field.as_deref() == Some("stop_headsign"))
        .collect::<Vec<_>>();
    let japanese = select(&result.notices, "JPN_030")
        .into_iter()
        .filter(|notice| notice.field.as_deref() == Some("stop_headsign"))
        .collect::<Vec<_>>();
    assert_eq!(kana.len(), 1);
    assert!(japanese.is_empty());
    let details = kana[0].details.as_ref().unwrap();
    assert_eq!(details["message_variant"], "aggregate_both");
    assert_eq!(details["affected_records"], "2");
}

#[test]
fn v3_translation_aggregation_keeps_japanese_only_findings() {
    let translations = "table_name,field_name,language,translation,field_value\nstop_times,stop_headsign,ja-Hrkt,シブヤ,渋谷\n";
    let result = validate(
        &[("stop_times.txt", "trip_id,arrival_time,departure_time,stop_id,stop_sequence,stop_headsign\nT1,08:00:00,08:00:00,S1,1,渋谷\nT1,08:10:00,08:10:00,S2,2,渋谷\n"), ("translations.txt", translations)],
        GtfsJpProfile::V3,
    );
    assert!(select(&result.notices, "JPN_028")
        .into_iter()
        .all(|notice| { notice.field.as_deref() != Some("stop_headsign") }));
    let japanese = select(&result.notices, "JPN_030")
        .into_iter()
        .filter(|notice| notice.field.as_deref() == Some("stop_headsign"))
        .collect::<Vec<_>>();
    assert_eq!(japanese.len(), 1);
    assert_eq!(
        japanese[0].details.as_ref().unwrap()["affected_records"],
        "2"
    );
}

#[test]
fn v4_missing_fare_file_is_unscored_manual_review_but_empty_file_is_medium() {
    for profile in [GtfsJpProfile::Auto, GtfsJpProfile::V3, GtfsJpProfile::V4] {
        let result = validate(&[], profile);
        let finding = select(&result.notices, "JPN_006")[0];
        assert_eq!(
            finding.severity,
            if profile == GtfsJpProfile::V4 {
                Severity::Bilgi
            } else {
                Severity::Orta
            }
        );
        if profile == GtfsJpProfile::V4 {
            assert_eq!(finding.severity.weight(), 0.0);
            assert_eq!(finding.details.as_ref().unwrap()["review_required"], "true");
            let mut without = result.notices.clone();
            without.retain(|n| n.rule_id != "JPN_006");
            let reports = gtfs_pipeline::k7_reporting::report(
                without,
                &Default::default(),
                &Default::default(),
                vec![],
                true,
                true,
            )
            .reports;
            assert_eq!(result.reports.r5.score, reports.r5.score);
        }
        for body in ["", "fare_id,price,currency_type,payment_method,transfers\n"] {
            let empty = validate(&[("fare_attributes.txt", body)], profile);
            assert_eq!(
                select(&empty.notices, "JPN_006")[0].severity,
                Severity::Orta
            );
        }
    }
    for profile in [GtfsJpProfile::V3, GtfsJpProfile::V4] {
        for body in [
            "wrong_column\nunusable\n",
            "fare_id,price,currency_type,payment_method,transfers\nF,bad,JPY,0,0\n",
            "fare_id,price,currency_type,payment_method,transfers\nF,100,JPY,8,0\n",
        ] {
            let result = validate(&[("fare_attributes.txt", body)], profile);
            let findings = select(&result.notices, "JPN_006");
            assert_eq!(findings.len(), 1, "{profile:?}: {body}");
            assert_eq!(findings[0].severity, Severity::Orta);
            assert_eq!(
                findings[0].details.as_ref().unwrap()["message_variant"],
                "unusable"
            );
        }
    }
}

#[test]
fn missing_fare_rules_stays_medium_and_points_to_fare_rules() {
    let result = validate(&[("fare_attributes.txt", "fare_id,price,currency_type,payment_method,transfers\nF1,100,JPY,0,0\nF2,200,JPY,0,0\n")], GtfsJpProfile::V4);
    let n = select(&result.notices, "JPN_006")[0];
    assert_eq!(n.severity, Severity::Orta);
    assert_eq!(n.file.as_deref(), Some("fare_rules.txt"));
}

#[test]
fn kana_groups_only_missing_records_by_table_field_and_exact_source() {
    let rows = "trip_id,arrival_time,departure_time,stop_id,stop_sequence,stop_headsign\nT1,08:00:00,08:00:00,S1,1,渋谷\nT1,08:10:00,08:10:00,S2,2,渋谷\nT2,08:00:00,08:00:00,S1,1,渋谷\nT2,08:10:00,08:10:00,S2,2,東京\n";
    let translations = "table_name,field_name,language,translation,record_id,record_sub_id,field_value\nstop_times,stop_headsign,ja-Hrkt,シブヤ,T1,1,\n";
    let extra = [
        ("stop_times.txt", rows),
        ("trips.txt", "route_id,service_id,trip_id\nR1,SVC,T1\nR1,SVC,T2\n"),
        ("attributions.txt", "attribution_id,organization_name,is_producer,is_operator,is_authority\na1,渋谷,1,0,0\na2,渋谷,1,0,0\n"),
        ("translations.txt", translations),
    ];
    let result = validate(&extra, GtfsJpProfile::V4);
    let groups = select(&result.notices, "JPN_029");
    assert_eq!(groups.len(), 3);
    let shibuya = groups
        .iter()
        .find(|n| {
            n.field.as_deref() == Some("stop_headsign")
                && n.observed_value.as_deref() == Some("渋谷")
        })
        .unwrap();
    let details = shibuya.details.as_ref().unwrap();
    assert_eq!(details["affected_records"], "2");
    assert!(details["example_record_ids"].contains("trip_id=T1,stop_sequence=2"));
    assert!(details["example_record_ids"].contains("trip_id=T2,stop_sequence=1"));
    assert!(!details["example_record_ids"].contains("trip_id=T1,stop_sequence=1"));
    let mut with_value = extra.to_vec();
    with_value[3].1 = "table_name,field_name,language,translation,record_id,record_sub_id,field_value\nstop_times,stop_headsign,ja-Hrkt,シブヤ,,,渋谷\n";
    let result = validate(&with_value, GtfsJpProfile::V4);
    assert_eq!(
        select(&result.notices, "JPN_029").len(),
        2,
        "field_value covers all matching stop_times, not attributions"
    );
}

#[test]
fn repeated_headsign_allocates_one_k4_notice_and_five_ordered_examples() {
    let mut rows =
        String::from("trip_id,arrival_time,departure_time,stop_id,stop_sequence,stop_headsign\n");
    for seq in 1..=50_000 {
        rows.push_str(&format!("T1,08:00:00,08:00:00,S1,{seq},渋谷\n"));
    }
    let bytes = feed(&[("stop_times.txt", &rows)]);
    let config = ValidatorConfig {
        gtfs_jp_profile: GtfsJpProfile::V4,
        ..Default::default()
    };
    let parsed = gtfs_pipeline::k1_parse::parse(&bytes, &config).unwrap();
    let k2 = gtfs_pipeline::k2::validate(parsed.files, Some(&bytes), &config);
    let map = gtfs_pipeline::k3_entity_graph::build(&k2.records).entity_map;
    let k4 = gtfs_pipeline::k4_cross_ref::check(&k2.records, &map, 20260914);
    let n = select(&k4.notices, "JPN_029");
    assert_eq!(n.len(), 1, "aggregation must happen before K7");
    let details = n[0].details.as_ref().unwrap();
    assert_eq!(details["affected_records"], "50000");
    assert_eq!(details["example_record_ids"].split("; ").count(), 5);
    assert!(details["example_record_ids"].starts_with("trip_id=T1,stop_sequence=1"));
}

const FARES: &str = "fare_id,price,currency_type,payment_method,transfers,agency_id\nF,100,JPY,0,0,A\nU,200,JPY,0,0,A\n";
const MULTI_ROUTES: &str =
    "route_id,agency_id,route_short_name,route_type\nR1,A,1,3\nR2,A,2,3\nR3,B,3,3\n";
const MULTI_TRIPS: &str = "route_id,service_id,trip_id\nR1,SVC,T1\nR2,SVC,T2\nR3,SVC,T3\n";
const MULTI_STOPS: &str = "stop_id,stop_name,stop_lat,stop_lon,location_type,zone_id\nS1,One,35,139,0,Z\nS2,Two,35.001,139,,\nS3,Three,35.002,139,0,\nS4,Four,35.003,139,0,\nUNUSED,Unused,35.004,139,0,\n";
const MULTI_TIMES: &str = "trip_id,arrival_time,departure_time,stop_id,stop_sequence\nT1,08:00:00,08:00:00,S1,1\nT1,08:10:00,08:10:00,S2,2\nT2,08:00:00,08:00:00,S1,1\nT2,08:10:00,08:10:00,S3,2\nT3,08:00:00,08:00:00,S1,1\nT3,08:10:00,08:10:00,S4,2\n";

fn zone_result(rules: &str, profile: GtfsJpProfile) -> gtfs_core::ValidationResult {
    validate(&[
        ("agency.txt", "agency_id,agency_name,agency_url,agency_timezone,agency_lang\nA,First,https://example.jp,Asia/Tokyo,ja\nB,Second,https://example.jp,Asia/Tokyo,ja\n"),
        ("routes.txt", MULTI_ROUTES), ("trips.txt", MULTI_TRIPS), ("stops.txt", MULTI_STOPS), ("stop_times.txt", MULTI_TIMES),
        ("fare_attributes.txt", FARES), ("fare_rules.txt", rules),
    ], profile)
}

#[test]
fn zone_requirement_respects_route_agency_uniform_and_unused_stop_boundaries() {
    for profile in [GtfsJpProfile::Auto, GtfsJpProfile::V3, GtfsJpProfile::V4] {
        for rule in ["F,R1,Z,,", "F,R1,,Z,", "F,R1,,,Z", "F,,Z,,\nU,R2,,,"] {
            let rules = format!("fare_id,route_id,origin_id,destination_id,contains_id\n{rule}\n");
            let result = zone_result(&rules, profile);
            let missing = select(&result.notices, "JPN_031");
            assert_eq!(
                missing.len(),
                usize::from(profile != GtfsJpProfile::Auto),
                "{profile:?}: {rule}"
            );
            if let Some(n) = missing.first() {
                assert_eq!(n.entity_id.as_deref(), Some("S2"));
                assert_eq!(n.details.as_ref().unwrap()["example_route_ids"], "R1");
            }
        }
    }
}

#[test]
fn uniform_fares_bad_references_and_mismatched_agencies_do_not_prove_zone_requirement() {
    for row in [
        "F,R1,,,",
        "F,R1, Z,,",
        "MISSING,R1,Z,,",
        "F,MISSING,Z,,",
        "F,R3,Z,,",
    ] {
        let rules = format!("fare_id,route_id,origin_id,destination_id,contains_id\n{row}\n");
        let result = zone_result(&rules, GtfsJpProfile::V4);
        assert!(select(&result.notices, "JPN_031").is_empty(), "{row}");
    }
}

#[test]
fn agency_wide_zone_fare_requires_a_served_zone_anchor_and_unambiguous_agency() {
    let rules = "fare_id,origin_id\nF,Z\n";
    let disconnected = "trip_id,arrival_time,departure_time,stop_id,stop_sequence\nT1,08:00:00,08:00:00,S1,1\nT1,08:10:00,08:10:00,S2,2\nT2,08:00:00,08:00:00,S3,1\nT2,08:10:00,08:10:00,S3,2\n";
    let result = validate(
        &[
            ("routes.txt", MULTI_ROUTES),
            ("trips.txt", MULTI_TRIPS),
            ("stops.txt", MULTI_STOPS),
            ("stop_times.txt", disconnected),
            ("fare_attributes.txt", FARES),
            ("fare_rules.txt", rules),
        ],
        GtfsJpProfile::V4,
    );
    let missing = select(&result.notices, "JPN_031");
    assert_eq!(missing.len(), 1);
    assert_eq!(missing[0].entity_id.as_deref(), Some("S2"));

    let ambiguous = validate(&[
        ("agency.txt", "agency_id,agency_name,agency_url,agency_timezone\nA,A,https://example.jp,Asia/Tokyo\nB,B,https://example.jp,Asia/Tokyo\n"),
        ("fare_attributes.txt", "fare_id,price,currency_type,payment_method,transfers\nF,100,JPY,0,0\n"),
        ("fare_rules.txt", rules),
    ], GtfsJpProfile::V4);
    assert!(select(&ambiguous.notices, "JPN_031").is_empty());
}

#[test]
fn zone_notices_are_per_stop_and_exclude_non_platforms_and_invalid_locations() {
    for location in ["0", "", "1", "2", "3", "4", "bad", "99"] {
        let stops = format!("stop_id,stop_name,stop_lat,stop_lon,location_type,zone_id\nS1,One,35,139,0,Z\nS2,Two,35.001,139,{location},\n");
        let result = validate(&[
            ("stops.txt", &stops),
            ("fare_attributes.txt", FARES),
            ("fare_rules.txt", "fare_id,route_id,origin_id\nF,R1,Z\nU,R1,Z\n"),
            ("trips.txt", "route_id,service_id,trip_id\nR1,SVC,T1\nR1,SVC,T2\n"),
            ("stop_times.txt", "trip_id,arrival_time,departure_time,stop_id,stop_sequence\nT1,08:00:00,08:00:00,S1,1\nT1,08:10:00,08:10:00,S2,2\nT2,08:00:00,08:00:00,S1,1\nT2,08:10:00,08:10:00,S2,2\n"),
        ], GtfsJpProfile::V4);
        assert_eq!(
            select(&result.notices, "JPN_031").len(),
            usize::from(matches!(location, "0" | "")),
            "{location}"
        );
    }
}
