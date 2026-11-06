use std::io::Write as _;
use zip::write::SimpleFileOptions;

use gtfs_pipeline::{
    k1_parse::parse, validate_bytes, FatalCode, GtfsJpProfile, ValidateResult, ValidatorConfig,
};

// Fixed date for all tests — deterministic analytics output.
const TODAY: u32 = 20_260_515;

// ── Helpers ───────────────────────────────────────────────────────────────────

fn make_zip(files: &[(&str, &[u8])]) -> Vec<u8> {
    let cur = std::io::Cursor::new(Vec::new());
    let mut zw = zip::ZipWriter::new(cur);
    let opts = SimpleFileOptions::default();
    for (name, data) in files {
        zw.start_file(*name, opts).unwrap();
        zw.write_all(data).unwrap();
    }
    zw.finish().unwrap().into_inner()
}

// The date range (20250101–20271231) contains the test day (20260515, Friday).
// Monday–Friday=1 → SVC1 is active → no service-gap warning is emitted.
static AGENCY: &[u8] =
    b"agency_id,agency_name,agency_url,agency_timezone\n1,Test,http://test.example,UTC\n";
static STOPS: &[u8] =
    b"stop_id,stop_name,stop_lat,stop_lon\nS1,Stop1,41.0,29.0\nS2,Stop2,41.1,29.1\n";
static ROUTES: &[u8] = b"route_id,agency_id,route_short_name,route_type\nR1,1,101,3\n";
static TRIPS: &[u8] = b"route_id,service_id,trip_id\nR1,SVC1,T1\n";
static TRIPS_DANGLING_ROUTE: &[u8] = b"route_id,service_id,trip_id\nR9,SVC1,T1\n";
static STOP_TIMES: &[u8] = b"trip_id,arrival_time,departure_time,stop_id,stop_sequence\n\
      T1,08:00:00,08:00:00,S1,1\nT1,08:10:00,08:10:00,S2,2\n";
static CALENDAR: &[u8] =
    b"service_id,monday,tuesday,wednesday,thursday,friday,saturday,sunday,start_date,end_date\n\
      SVC1,1,1,1,1,1,0,0,20250101,20271231\n";

fn base_files() -> Vec<(&'static str, &'static [u8])> {
    vec![
        ("agency.txt", AGENCY),
        ("stops.txt", STOPS),
        ("routes.txt", ROUTES),
        ("trips.txt", TRIPS),
        ("stop_times.txt", STOP_TIMES),
        ("calendar.txt", CALENDAR),
    ]
}

fn run(files: &[(&str, &[u8])]) -> ValidateResult {
    validate_bytes(&make_zip(files), &ValidatorConfig::default(), TODAY)
}

fn run_with_profile(files: &[(&str, &[u8])], profile: GtfsJpProfile) -> ValidateResult {
    let config = ValidatorConfig {
        gtfs_jp_profile: profile,
        ..ValidatorConfig::default()
    };
    validate_bytes(&make_zip(files), &config, TODAY)
}

/// The GTFS-JP list applies ONLY to a detected JP feed: checks made meaningless
/// by Japanese publishing practice must remain visible in feeds from elsewhere.
#[test]
fn gtfs_jp_scoped_disabled_rules_apply_only_to_detected_jp_feeds() {
    let config = ValidatorConfig {
        disabled_rule_ids_gtfs_jp: vec!["DQ_004".to_string()],
        ..ValidatorConfig::default()
    };
    let check = |files: &[(&str, &[u8])], expect_hidden: bool, label: &str| {
        match validate_bytes(&make_zip(files), &config, TODAY) {
            ValidateResult::Ok(vr) => assert_eq!(
                !has(&vr, "DQ_004"), expect_hidden,
                "{label}: DQ_004 gizlenme beklentisi {expect_hidden}"
            ),
            other => panic!("{label}: ValidateResult::Ok beklendi, alınan: {other:?}"),
        }
    };

    // Non-JP feed: the rule remains visible.
    check(&base_files(), false, "global feed");

    // `office_jp.txt` activates GTFS-JP detection: the rule is hidden.
    let mut jp = base_files();
    jp.push(("office_jp.txt", b"office_id,office_name\nO1,Test\n"));
    check(&jp, true, "GTFS-JP feed");

    // The general list applies to both feeds (proof of the scope distinction).
    let global = ValidatorConfig {
        disabled_rule_ids: vec!["DQ_004".to_string()],
        ..ValidatorConfig::default()
    };
    match validate_bytes(&make_zip(&base_files()), &global, TODAY) {
        ValidateResult::Ok(vr) => assert!(!has(&vr, "DQ_004"), "genel liste JP dışında da uygulanmalı"),
        other => panic!("ValidateResult::Ok beklendi, alınan: {other:?}"),
    }
}

#[test]
fn disabled_rule_ids_reject_unknown_and_spec_rules() {
    // An unknown/retired ID must not silently disable anything; a Spec rule is
    // evidence for the publication decision and must not be hidden.
    for (rule, variant, jp_scope) in [
        ("DQ_04", "disabled_rule_unknown", false),
        ("BKR_002", "disabled_rule_unknown", false),
        ("BKR_001", "disabled_rule_spec", false),
        // The same gate also applies to the GTFS-JP list.
        ("DQ_04", "disabled_rule_unknown", true),
        ("BKR_001", "disabled_rule_spec", true),
    ] {
        let config = if jp_scope {
            ValidatorConfig { disabled_rule_ids_gtfs_jp: vec![rule.to_string()], ..ValidatorConfig::default() }
        } else {
            ValidatorConfig { disabled_rule_ids: vec![rule.to_string()], ..ValidatorConfig::default() }
        };
        match validate_bytes(&make_zip(&base_files()), &config, TODAY) {
            ValidateResult::Fatal(e) => {
                assert_eq!(e.params.get("variant").map(String::as_str), Some(variant), "{rule}: {e:?}");
            }
            other => panic!("{rule} için Fatal beklendi, alınan: {other:?}"),
        }
    }
}

#[test]
fn disabled_rule_ids_remove_selected_findings_from_reports() {
    let baseline = run(&base_files());
    let baseline = match baseline {
        ValidateResult::Ok(vr) => vr,
        other => panic!("ValidateResult::Ok beklendi, alınan: {other:?}"),
    };
    for rule in ["DQ_003", "DQ_004", "TRP_021"] {
        assert!(has(&baseline, rule), "varsayılan feed {rule} üretmeli");
    }

    let config = ValidatorConfig {
        disabled_rule_ids: ["DQ_003", "DQ_004", "TRP_021"]
            .into_iter()
            .map(str::to_string)
            .collect(),
        ..ValidatorConfig::default()
    };
    let filtered = validate_bytes(&make_zip(&base_files()), &config, TODAY);
    let filtered = match filtered {
        ValidateResult::Ok(vr) => vr,
        other => panic!("ValidateResult::Ok beklendi, alınan: {other:?}"),
    };
    for rule in ["DQ_003", "DQ_004", "TRP_021"] {
        assert!(!has(&filtered, rule), "devre dışı {rule} raporlanmamalı");
    }
}

#[test]
fn empty_gtfs_jp_file_still_activates_profile_detection() {
    for file in [
        "agency_jp.txt",
        "office_jp.txt",
        "routes_jp.txt",
        "pattern_jp.txt",
    ] {
        let mut files = base_files();
        files.push((file, b"header_only\n"));
        match run(&files) {
            ValidateResult::Ok(vr) => {
                assert!(vr.metrics.is_gtfs_jp, "{file} GTFS-JP rozeti açmalı");
                assert_eq!(
                    vr.metrics.gtfs_jp_profile.as_deref(),
                    Some("auto"),
                    "varsayılan profil kullanıcıya taşınmalı"
                );
                assert!(
                    has(&vr, "JPN_007"),
                    "{file} GTFS-JP profil kurallarını açmalı"
                );
            }
            other => panic!("ValidateResult::Ok beklendi, alınan: {other:?}"),
        }
    }
}

#[test]
fn selected_gtfs_jp_profile_is_exposed_without_version_inference() {
    let mut files = base_files();
    files.push((
        "pattern_jp.txt",
        b"jp_pattern_id,route_update_date\nP1,20260825\n",
    ));

    match run_with_profile(&files, GtfsJpProfile::V3) {
        ValidateResult::Ok(vr) => {
            assert!(vr.metrics.is_gtfs_jp);
            assert_eq!(vr.metrics.gtfs_jp_profile.as_deref(), Some("v3"));
        }
        other => panic!("ValidateResult::Ok beklendi, alınan: {other:?}"),
    }

    match run(&base_files()) {
        ValidateResult::Ok(vr) => {
            assert!(!vr.metrics.is_gtfs_jp);
            assert_eq!(vr.metrics.gtfs_jp_profile, None);
        }
        other => panic!("ValidateResult::Ok beklendi, alınan: {other:?}"),
    }

    for profile in [GtfsJpProfile::V3, GtfsJpProfile::V4] {
        match run_with_profile(&base_files(), profile) {
            ValidateResult::Ok(vr) => {
                assert!(
                    !vr.metrics.is_gtfs_jp,
                    "açık profil tespit rozetini değiştirmemeli"
                );
                assert_eq!(
                    vr.metrics.gtfs_jp_profile.as_deref(),
                    Some(profile.as_str())
                );
                assert!(
                    !vr.notices
                        .iter()
                        .any(|notice| notice.rule_id.starts_with("JPN_")),
                    "JP sinyali olmayan feed açık profil seçilse de JPN bulgusu üretmemeli: {:?}",
                    vr.notices
                        .iter()
                        .filter(|notice| notice.rule_id.starts_with("JPN_"))
                        .collect::<Vec<_>>()
                );
            }
            other => panic!("ValidateResult::Ok beklendi, alınan: {other:?}"),
        }
    }
}

#[test]
fn agency_language_and_tokyo_timezone_detect_jp_without_optional_jp_files() {
    let mut files = base_files();
    files[0] = (
        "agency.txt",
        b"agency_id,agency_name,agency_url,agency_timezone,agency_lang\n1,Test,http://test.example,Asia/Tokyo,ja\n",
    );

    for profile in [GtfsJpProfile::Auto, GtfsJpProfile::V3, GtfsJpProfile::V4] {
        match run_with_profile(&files, profile) {
            ValidateResult::Ok(vr) => {
                assert!(vr.metrics.is_gtfs_jp, "{profile:?} JP olarak algılanmalı");
                assert!(has(&vr, "JPN_004"), "translations eksikliği raporlanmalı");
                assert!(has(&vr, "JPN_007"), "feed_info eksikliği raporlanmalı");
            }
            other => panic!("ValidateResult::Ok beklendi, alınan: {other:?}"),
        }
    }
}

#[test]
fn agency_language_alone_does_not_detect_jp() {
    let mut files = base_files();
    files[0] = (
        "agency.txt",
        b"agency_id,agency_name,agency_url,agency_timezone,agency_lang\n1,Test,http://test.example,UTC,ja\n2,Other,http://other.example,Asia/Tokyo,en\n",
    );
    match run(&files) {
        ValidateResult::Ok(vr) => {
            assert!(!vr.metrics.is_gtfs_jp);
            assert!(!vr
                .notices
                .iter()
                .any(|notice| notice.rule_id.starts_with("JPN_")));
        }
        other => panic!("ValidateResult::Ok beklendi, alınan: {other:?}"),
    }
}

#[test]
fn non_japanese_primary_language_prefix_does_not_detect_jp() {
    let mut files = base_files();
    files.push((
        "feed_info.txt",
        b"feed_publisher_name,feed_publisher_url,feed_lang\nTest,http://test.example,jaa\n",
    ));
    match run(&files) {
        ValidateResult::Ok(vr) => {
            assert!(!vr.metrics.is_gtfs_jp);
            assert!(!vr
                .notices
                .iter()
                .any(|notice| notice.rule_id.starts_with("JPN_")));
        }
        other => panic!("ValidateResult::Ok beklendi, alınan: {other:?}"),
    }
}

#[test]
fn v4_location_type_distinguishes_missing_column_blank_and_invalid_value() {
    let agency = b"agency_id,agency_name,agency_url,agency_timezone,agency_lang\n1,Test,http://test.example,Asia/Tokyo,ja\n";
    let feed_info = b"feed_publisher_name,feed_publisher_url,feed_lang,feed_start_date,feed_end_date,feed_version\nTest,http://test.example,ja,20250101,20271231,v1\n";
    let cases: [(&[u8], bool, bool, bool); 3] = [
        (b"stop_id,stop_name,stop_lat,stop_lon\nS1,\xE6\x9D\xB1\xE4\xBA\xAC,41.0,29.0\nS2,Stop2,41.1,29.1\n", true, false, true),
        (b"stop_id,stop_name,stop_lat,stop_lon,location_type\nS1,\xE6\x9D\xB1\xE4\xBA\xAC,41.0,29.0,\nS2,Stop2,41.1,29.1,0\n", false, false, true),
        (b"stop_id,stop_name,stop_lat,stop_lon,location_type\nS1,\xE6\x9D\xB1\xE4\xBA\xAC,41.0,29.0,x\nS2,Stop2,41.1,29.1,0\n", false, true, false),
    ];
    for (stops, expect_jpn022, expect_stp008, expect_jpn001) in cases {
        let mut files = base_files();
        files[0] = ("agency.txt", agency);
        files[1] = ("stops.txt", stops);
        files.push(("feed_info.txt", feed_info));
        match run_with_profile(&files, GtfsJpProfile::V4) {
            ValidateResult::Ok(vr) => {
                let location_missing = vr.notices.iter().any(|notice| {
                    notice.rule_id == "JPN_022" && notice.field.as_deref() == Some("location_type")
                });
                assert_eq!(location_missing, expect_jpn022);
                assert_eq!(has(&vr, "STP_008"), expect_stp008);
                assert_eq!(has(&vr, "JPN_001"), expect_jpn001);
            }
            other => panic!("ValidateResult::Ok beklendi, alınan: {other:?}"),
        }
    }
}

#[test]
fn jp_kana_scope_covers_stations_and_both_route_names_but_skips_latin_sources() {
    let mut files = base_files();
    files[0] = (
        "agency.txt",
        b"agency_id,agency_name,agency_url,agency_timezone,agency_lang\n1,Test,http://test.example,Asia/Tokyo,ja\n",
    );
    files[1] = (
        "stops.txt",
        b"stop_id,stop_name,stop_lat,stop_lon,location_type\nS1,Stop1,41.0,29.0,0\nS2,Stop2,41.1,29.1,0\nST,\xE6\x9D\xB1\xE4\xBA\xAC\xE9\xA7\x85,41.2,29.2,1\n",
    );
    files[2] = (
        "routes.txt",
        b"route_id,agency_id,route_short_name,route_long_name,route_type\nR1,1,\xE6\xB8\x8B\xE8\xB0\xB7,\xE6\xB8\x8B\xE8\xB0\xB7\xE7\xB7\x9A,3\n",
    );
    files[3] = (
        "trips.txt",
        b"route_id,service_id,trip_id,trip_headsign\nR1,SVC1,T1,Airport\n",
    );
    files.push((
        "translations.txt",
        b"table_name,field_name,language,translation,record_id\nstops,stop_name,ja-Hrkt,\xE3\x83\x86\xE3\x82\xB9\xE3\x83\x88,S1\n",
    ));
    match run_with_profile(&files, GtfsJpProfile::V4) {
        ValidateResult::Ok(vr) => {
            assert!(vr.notices.iter().any(|notice| {
                notice.rule_id == "JPN_001" && notice.entity_id.as_deref() == Some("ST")
            }));
            let route_fields: std::collections::BTreeSet<_> = vr
                .notices
                .iter()
                .filter(|notice| notice.rule_id == "JPN_008")
                .filter_map(|notice| notice.field.as_deref())
                .collect();
            assert_eq!(route_fields, ["route_long_name", "route_short_name"].into());
            assert!(!has(&vr, "JPN_009"));
            assert!(!has(&vr, "JPN_010"));
        }
        other => panic!("ValidateResult::Ok beklendi, alınan: {other:?}"),
    }
}

#[test]
fn jpn_021_requires_a_reading_and_accepts_half_width_katakana() {
    for (translations, expected) in [
        (
            b"table_name,field_name,language,translation,record_id\nstops,stop_name,ja-Hrkt,\xE6\x9D\xB1\xE4\xBA\xAC,S1\n".as_slice(),
            true,
        ),
        (
            b"table_name,field_name,language,translation,record_id\nstops,stop_name,ja-Hrkt,\xEF\xBE\x84\xEF\xBD\xB3\xEF\xBD\xB7\xEF\xBD\xAE\xEF\xBD\xB3,S1\n".as_slice(),
            false,
        ),
        (
            b"table_name,field_name,language,translation,record_id\nstops,stop_name,ja-Hrkt,\xE3\x81\xA8\xE3\x81\x86\xE3\x81\x8D\xE3\x82\x87\xE3\x81\x861,S1\n".as_slice(),
            false,
        ),
    ] {
        let mut files = base_files();
        files.push(("translations.txt", translations));
        match run(&files) {
            ValidateResult::Ok(vr) => assert_eq!(has(&vr, "JPN_021"), expected),
            other => panic!("ValidateResult::Ok beklendi, alınan: {other:?}"),
        }
    }
}

#[test]
fn v3_requires_bus_route_type_and_routes_agency_id() {
    let mut files = base_files();
    files.push((
        "feed_info.txt",
        b"feed_publisher_name,feed_publisher_url,feed_lang\nTest,http://test.example,ja\n",
    ));
    // Explicit V3 now applies the numeric route_type=3 constant directly; in the
    // two-bus-plus-one-rail fixture, the route_type=2 violation must be reported
    // regardless of the distribution.
    files[2] = (
        "routes.txt",
        b"route_id,route_short_name,route_type\nR1,101,3\nR2,102,3\nR3,103,2\n",
    );
    match run_with_profile(&files, GtfsJpProfile::V3) {
        ValidateResult::Ok(vr) => {
            assert!(vr.notices.iter().any(|notice| notice.rule_id == "JPN_011"
                && notice.file.as_deref() == Some("routes.txt")));
            assert!(
                vr.notices.iter().any(|notice| notice.rule_id == "JPN_027"),
                "açık V3 otobüs profili, otobüs kapsamı içindeki route_type=2 değerini reddetmeli"
            );
        }
        other => panic!("ValidateResult::Ok beklendi, alınan: {other:?}"),
    }
}

#[test]
fn explicit_jp_constants_require_jp_detection() {
    let mut files = base_files();
    files[0] = (
        "agency.txt",
        b"agency_id,agency_name,agency_url,agency_timezone,agency_lang\n1,Test,http://test.example,UTC,en\n",
    );
    files.push((
        "feed_info.txt",
        b"feed_publisher_name,feed_publisher_url,feed_lang\nTest,http://test.example,en\n",
    ));
    files.push((
        "fare_attributes.txt",
        b"fare_id,price,currency_type,payment_method,transfers\nF1,1.00,USD,0,0\n",
    ));
    match run_with_profile(&files, GtfsJpProfile::V4) {
        ValidateResult::Ok(vr) => {
            for rule in ["JPN_023", "JPN_024", "JPN_025", "JPN_026"] {
                assert!(!has(&vr, rule), "{rule} JP sinyali yokken çalışmamalı");
            }
        }
        other => panic!("ValidateResult::Ok beklendi, alınan: {other:?}"),
    }
    match run(&files) {
        ValidateResult::Ok(vr) => {
            for rule in ["JPN_023", "JPN_024", "JPN_025", "JPN_026"] {
                assert!(!has(&vr, rule), "{rule} Auto'da sürüm sabiti dayatmamalı");
            }
        }
        other => panic!("ValidateResult::Ok beklendi, alınan: {other:?}"),
    }
}

#[test]
fn explicit_non_jp_profile_suppresses_jpn_fare_scope_notice() {
    let mut files = base_files();
    files[0] = (
        "agency.txt",
        b"agency_id,agency_name,agency_url,agency_timezone,agency_lang\n1,Test,http://test.example,UTC,en\n",
    );
    files.push((
        "feed_info.txt",
        b"feed_publisher_name,feed_publisher_url,feed_lang\nTest,http://test.example,en\n",
    ));
    files.push((
        "fare_attributes.txt",
        b"fare_id,price,currency_type,payment_method,transfers,agency_id\nF1,1.00,USD,0,0,1\n",
    ));
    files.push(("fare_rules.txt", b"fare_id,route_id,origin_id\nF1,R1,Z\n"));
    files[1] = (
        "stops.txt",
        b"stop_id,stop_name,stop_lat,stop_lon,location_type,zone_id\nS1,Stop1,41.0,29.0,0,Z\nS2,Stop2,41.1,29.1,0,\n",
    );

    for profile in [GtfsJpProfile::V3, GtfsJpProfile::V4] {
        match run_with_profile(&files, profile) {
            ValidateResult::Ok(vr) => assert!(
                !vr.notices
                    .iter()
                    .any(|notice| notice.rule_id.starts_with("JPN_")),
                "{profile:?} JP sinyali yokken JPN bulgusu üretmemeli"
            ),
            other => panic!("ValidateResult::Ok beklendi, alınan: {other:?}"),
        }
    }
}

#[test]
fn v4_free_and_route_specific_fixed_fares_do_not_trigger_jpn_006() {
    let cases: [Vec<(&'static str, &'static [u8])>; 2] = [
        vec![(
            "fare_attributes.txt",
            b"fare_id,price,currency_type,payment_method,transfers\nFREE,0,JPY,0,0\n",
        )],
        vec![
            (
                "routes.txt",
                b"route_id,agency_id,route_short_name,route_type\nR1,1,101,3\nR2,1,102,3\n",
            ),
            (
                "fare_attributes.txt",
                b"fare_id,price,currency_type,payment_method,transfers\nF1,200,JPY,0,0\nF2,300,JPY,0,0\n",
            ),
            (
                "fare_rules.txt",
                b"fare_id,route_id\nF1,R1\nF2,R2\n",
            ),
        ],
    ];

    for additions in cases {
        let mut files = base_files();
        for (name, data) in additions {
            if let Some(existing) = files.iter_mut().find(|(file, _)| *file == name) {
                existing.1 = data;
            } else {
                files.push((name, data));
            }
        }
        match run_with_profile(&files, GtfsJpProfile::V4) {
            ValidateResult::Ok(vr) => assert!(
                !has(&vr, "JPN_006"),
                "ücretsiz veya hatlara eşlenmiş sabit ücret JPN_006 üretmemeli"
            ),
            other => panic!("ValidateResult::Ok beklendi, alınan: {other:?}"),
        }
    }
}

#[test]
fn fare_origin_destination_coverage_checks_zone_pairs_but_accepts_route_uniform_fares() {
    let mut files = base_files();
    files[1] = (
        "stops.txt",
        b"stop_id,stop_name,stop_lat,stop_lon,zone_id\nS1,Stop1,41.0,29.0,A\nS2,Stop2,41.1,29.1,B\n",
    );
    files.push((
        "fare_attributes.txt",
        b"fare_id,price,currency_type,payment_method,transfers\nF1,200,JPY,0,0\n",
    ));
    // Trips run S1(A)→S2(B); A→B is missing when only the reverse direction is priced.
    files.push((
        "fare_rules.txt",
        b"fare_id,route_id,origin_id,destination_id\nF1,R1,B,A\n",
    ));

    let incomplete = run(&files);
    let incomplete = match incomplete {
        ValidateResult::Ok(vr) => vr,
        other => panic!("ValidateResult::Ok beklendi, alınan: {other:?}"),
    };
    assert!(has(&incomplete, "FRL_009"));
    let missing = incomplete
        .notices
        .iter()
        .find(|notice| notice.rule_id == "FRL_009")
        .and_then(|notice| notice.details.as_ref())
        .expect("FRL_009 ayrıntıları olmalı");
    assert_eq!(missing["missing_pairs"], "1");
    assert!(missing["example_pairs"].contains("origin_id=A, destination_id=B"));

    // There is no finding when the only connected pair is priced: the reverse (B→A)
    // and same-zone (A→A, B→B) pairs are not required because no trip can make them.
    let mut served_only = files.clone();
    served_only.pop();
    served_only.push((
        "fare_rules.txt",
        b"fare_id,route_id,origin_id,destination_id\nF1,R1,A,B\n",
    ));
    match run(&served_only) {
        ValidateResult::Ok(vr) => assert!(!has(&vr, "FRL_009"), "yapılamayan çiftler istenmemeli"),
        other => panic!("ValidateResult::Ok beklendi, alınan: {other:?}"),
    }

    // If alighting at S2 is forbidden (drop_off_type=1), A→B is not passenger-accessible;
    // there is no missing pair even if only B→A is priced.
    let mut no_drop_off = files.clone();
    assert_eq!(no_drop_off[4].0, "stop_times.txt");
    no_drop_off[4] = (
        "stop_times.txt",
        b"trip_id,arrival_time,departure_time,stop_id,stop_sequence,pickup_type,drop_off_type\n\
          T1,08:00:00,08:00:00,S1,1,0,0\nT1,08:10:00,08:10:00,S2,2,0,1\n",
    );
    match run(&no_drop_off) {
        ValidateResult::Ok(vr) => assert!(!has(&vr, "FRL_009"), "inilemeyen durağa çift istenmemeli"),
        other => panic!("ValidateResult::Ok beklendi, alınan: {other:?}"),
    }

    let mut uniform = files;
    uniform.pop();
    uniform.push((
        "fare_rules.txt",
        b"fare_id,route_id,origin_id,destination_id\nF1,R1,,\n",
    ));
    let uniform = run(&uniform);
    let uniform = match uniform {
        ValidateResult::Ok(vr) => vr,
        other => panic!("ValidateResult::Ok beklendi, alınan: {other:?}"),
    };
    assert!(!has(&uniform, "FRL_009"));
}

#[test]
fn stp_033_is_suppressed_only_for_proven_uniform_fares() {
    // No fare_rules → zone_id has no consumer → no STP_033.
    let no_fares = match run(&base_files()) {
        ValidateResult::Ok(vr) => vr,
        other => panic!("ValidateResult::Ok beklendi, alınan: {other:?}"),
    };
    assert!(!has(&no_fares, "STP_033"));

    let mut uniform = base_files();
    uniform.push((
        "fare_attributes.txt",
        b"fare_id,price,currency_type,payment_method,transfers\nF1,200,JPY,0,0\n",
    ));
    uniform.push((
        "fare_rules.txt",
        b"fare_id,route_id,origin_id,destination_id\nF1,R1,,\n",
    ));
    let uniform = run(&uniform);
    let uniform = match uniform {
        ValidateResult::Ok(vr) => vr,
        other => panic!("ValidateResult::Ok beklendi, alınan: {other:?}"),
    };
    assert!(!has(&uniform, "STP_033"));

    let mut zone_based = base_files();
    zone_based.push((
        "fare_attributes.txt",
        b"fare_id,price,currency_type,payment_method,transfers\nF1,200,JPY,0,0\n",
    ));
    zone_based.push((
        "fare_rules.txt",
        b"fare_id,route_id,origin_id,destination_id\nF1,R1,A,B\n",
    ));
    let zone_based = run(&zone_based);
    let zone_based = match zone_based {
        ValidateResult::Ok(vr) => vr,
        other => panic!("ValidateResult::Ok beklendi, alınan: {other:?}"),
    };
    assert!(has(&zone_based, "STP_033"));
}

#[test]
fn v3_remaining_translation_fields_require_kana_and_japanese_rows() {
    let mut files = base_files();
    files[2] = (
        "routes.txt",
        b"route_id,agency_id,route_short_name,route_type,route_desc\nR1,1,101,3,\xE6\xB8\x8B\xE8\xB0\xB7\xE6\x96\xB9\xE9\x9D\xA2\n",
    );
    files[3] = (
        "trips.txt",
        b"route_id,service_id,trip_id,jp_trip_desc\nR1,SVC1,T1,\xE6\x80\xA5\xE8\xA1\x8C\n",
    );
    files.push((
        "translations.txt",
        b"table_name,field_name,language,translation,record_id\nstops,stop_name,ja-Hrkt,\xE3\x83\x86\xE3\x82\xB9\xE3\x83\x88,S1\n",
    ));
    match run_with_profile(&files, GtfsJpProfile::V3) {
        ValidateResult::Ok(vr) => {
            for field in ["route_desc", "jp_trip_desc"] {
                let notice = vr
                    .notices
                    .iter()
                    .find(|notice| {
                        notice.rule_id == "JPN_028" && notice.field.as_deref() == Some(field)
                    })
                    .expect("aggregate_both JPN_028 bekleniyordu");
                assert_eq!(
                    notice.details.as_ref().unwrap()["message_variant"],
                    "aggregate_both"
                );
                assert!(!vr.notices.iter().any(|notice| {
                    notice.rule_id == "JPN_030" && notice.field.as_deref() == Some(field)
                }));
            }
        }
        other => panic!("ValidateResult::Ok beklendi, alınan: {other:?}"),
    }
}

#[test]
fn gtfs_jp_translation_fields_are_not_rejected_by_general_translation_validation() {
    let mut files = base_files();
    files[3] = (
        "trips.txt",
        b"route_id,service_id,trip_id,jp_trip_desc\nR1,SVC1,T1,\xE6\x80\xA5\xE8\xA1\x8C\n",
    );
    files.push((
        "feed_info.txt",
        b"feed_publisher_name,feed_publisher_url,feed_lang\n\xE4\xBA\x8B\xE6\xA5\xAD\xE8\x80\x85,http://test.example,ja\n",
    ));
    files.push((
        "translations.txt",
        b"table_name,field_name,language,translation,record_id\ntrips,jp_trip_desc,ja-Hrkt,\xE3\x81\x8D\xE3\x82\x85\xE3\x81\x86\xE3\x81\x93\xE3\x81\x86,T1\nfeed_info,feed_publisher_url,ja,http://ja.example,\n",
    ));

    match run_with_profile(&files, GtfsJpProfile::V3) {
        ValidateResult::Ok(vr) => assert!(
            !has(&vr, "TRN_002"),
            "GTFS-JP jp_trip_desc ve feed_publisher_url çeviri hedefleri geçerli olmalı"
        ),
        other => panic!("ValidateResult::Ok beklendi, alınan: {other:?}"),
    }
}

#[test]
fn v4_stop_times_kana_matching_uses_trip_and_stop_sequence() {
    let mut files = base_files();
    files[3] = (
        "trips.txt",
        b"route_id,service_id,trip_id\nR1,SVC1,T1\nR1,SVC1,T2\n",
    );
    files[4] = (
        "stop_times.txt",
        b"trip_id,arrival_time,departure_time,stop_id,stop_sequence,stop_headsign\nT1,08:00:00,08:00:00,S1,1,\xE6\x9D\xB1\xE4\xBA\xAC\nT2,09:00:00,09:00:00,S1,1,\xE6\x9D\xB1\xE4\xBA\xAC\n",
    );
    files.push((
        "translations.txt",
        b"table_name,field_name,language,translation,record_id,record_sub_id\nstop_times,stop_headsign,ja-Hrkt,\xE3\x81\xA8\xE3\x81\x86\xE3\x81\x8D\xE3\x82\x87\xE3\x81\x86,T1,1\n",
    ));
    match run_with_profile(&files, GtfsJpProfile::V4) {
        ValidateResult::Ok(vr) => {
            let findings: Vec<_> = vr
                .notices
                .iter()
                .filter(|notice| notice.rule_id == "JPN_029")
                .collect();
            assert_eq!(findings.len(), 1);
            let details = findings[0].details.as_ref().unwrap();
            assert_eq!(details["affected_records"], "1");
            assert!(details["example_record_ids"].contains("trip_id=T2,stop_sequence=1"));
            assert!(!details["example_record_ids"].contains("trip_id=T1,"));
        }
        other => panic!("ValidateResult::Ok beklendi, alınan: {other:?}"),
    }
}

#[test]
fn v4_required_field_findings_survive_feed_pipeline_dedup() {
    let mut files = base_files();
    files.push((
        "feed_info.txt",
        b"feed_publisher_name,feed_publisher_url,feed_lang\nTest,http://test.example,ja\n",
    ));

    match run_with_profile(&files, GtfsJpProfile::V4) {
        ValidateResult::Ok(vr) => {
            let fields: std::collections::BTreeSet<_> = vr
                .notices
                .iter()
                .filter(|n| n.rule_id == "JPN_022")
                .filter_map(|n| n.field.as_deref())
                .collect();
            assert_eq!(
                fields.len(),
                5,
                "JPN_022 alan bazında beş bulguya ayrılmalı: {fields:?}"
            );
            assert!(fields.contains("agency_lang"));
            assert!(fields.contains("location_type"));
            assert!(fields.contains("feed_start_date"));
            assert!(fields.contains("feed_end_date"));
            assert!(fields.contains("feed_version"));
        }
        other => panic!("ValidateResult::Ok beklendi, alınan: {other:?}"),
    }
}

#[test]
fn v4_rejects_none_record_sub_id_in_general_and_jp_rules() {
    let mut files = base_files();
    files.push((
        "feed_info.txt",
        b"feed_publisher_name,feed_publisher_url,feed_lang\nTest,http://test.example,ja\n",
    ));
    files.push((
        "translations.txt",
        b"table_name,field_name,language,translation,record_id,record_sub_id\n\
          stops,stop_name,ja-Hrkt,\xE3\x81\xA8\xE3\x81\x86\xE3\x81\x8D\xE3\x82\x87\xE3\x81\x86,S1,NONE\n",
    ));

    match run_with_profile(&files, GtfsJpProfile::V4) {
        ValidateResult::Ok(vr) => {
            assert!(vr.notices.iter().any(|n| n.rule_id == "TRN_014"));
            assert!(vr.notices.iter().any(|n| n.rule_id == "JPN_019"));
        }
        other => panic!("ValidateResult::Ok beklendi, alınan: {other:?}"),
    }

    match run_with_profile(&files, GtfsJpProfile::Auto) {
        ValidateResult::Ok(vr) => {
            assert!(!vr.notices.iter().any(|n| n.rule_id == "TRN_014"));
            assert!(!vr.notices.iter().any(|n| n.rule_id == "JPN_019"));
        }
        other => panic!("ValidateResult::Ok beklendi, alınan: {other:?}"),
    }
}

const LEGACY_TRANSLATIONS: &[u8] =
    b"trans_id,lang,translation\n1,EN,Example\n2,HE,\xD7\x93\xD7\x95\xD7\x92\xD7\x9E\xD7\x94\n";

const MODERN_TRANSLATIONS: &[u8] = b"table_name,field_name,language,translation,record_id\n\
      stops,stop_name,tr,Durak,S1\n\
      stops,stop_name,tr,\xC4\xB0stasyon,S1\n";

#[test]
fn legacy_translation_headers_keep_structural_findings_without_row_cascade() {
    let mut files = base_files();
    files.push(("translations.txt", LEGACY_TRANSLATIONS));

    match run(&files) {
        ValidateResult::Ok(vr) => {
            for field in ["table_name", "field_name", "language"] {
                assert!(
                    vr.notices.iter().any(|n| {
                        n.rule_id == "ARC_025"
                            && n.file.as_deref() == Some("translations.txt")
                            && n.field.as_deref() == Some(field)
                    }),
                    "ARC_025/{field} korunmalı"
                );
            }
            for field in ["trans_id", "lang"] {
                assert!(
                    vr.notices.iter().any(|n| {
                        n.rule_id == "ARC_017"
                            && n.file.as_deref() == Some("translations.txt")
                            && n.field.as_deref() == Some(field)
                    }),
                    "ARC_017/{field} korunmalı"
                );
            }
            for rule in ["TRN_001", "TRN_002", "TRN_003", "TRN_006", "TRN_011"] {
                assert!(
                    !vr.notices.iter().any(|n| n.rule_id == rule),
                    "legacy header nedeniyle {rule} cascade üretmemeli"
                );
            }
        }
        other => panic!("ValidateResult::Ok beklendi, alınan: {other:?}"),
    }
}

#[test]
fn modern_translation_headers_keep_translation_conflict_validation() {
    let mut files = base_files();
    files.push(("translations.txt", MODERN_TRANSLATIONS));

    match run(&files) {
        ValidateResult::Ok(vr) => {
            assert!(
                !vr.notices.iter().any(|n| {
                    n.rule_id == "ARC_025" && n.file.as_deref() == Some("translations.txt")
                }),
                "modern translations schema should not have missing-header findings"
            );
            assert!(
                vr.notices.iter().any(|n| n.rule_id == "TRN_006"),
                "modern translations should retain TRN_006 conflict validation"
            );
        }
        other => panic!("ValidateResult::Ok beklendi, alınan: {other:?}"),
    }
}

#[test]
fn location_groups_translation_uses_catalogue_table_and_field() {
    let mut files = base_files();
    files.push((
        "feed_info.txt",
        b"feed_publisher_name,feed_publisher_url,feed_lang\nTest,http://test.example,ja\n",
    ));
    files.push((
        "location_groups.txt",
        b"location_group_id,location_group_name\nC1,Mokutekichi\nC2,Teiryujo\n",
    ));
    files.push((
        "translations.txt",
        b"table_name,field_name,language,translation,record_id\n\
          location_groups,location_group_name,en,Mokutekichi,C1\n\
          location_groups,location_group_name,en,Teiryujo,C2\n",
    ));

    match run(&files) {
        ValidateResult::Ok(vr) => {
            for rule in ["TRN_001", "TRN_002", "TRN_011"] {
                assert!(
                    !has(&vr, rule),
                    "location_groups/location_group_name çevirisi {rule} üretmemeli"
                );
            }
        }
        other => panic!("ValidateResult::Ok beklendi, alınan: {other:?}"),
    }
}

#[test]
fn location_groups_translation_record_id_must_exist() {
    let mut files = base_files();
    files.push((
        "feed_info.txt",
        b"feed_publisher_name,feed_publisher_url,feed_lang\nTest,http://test.example,ja\n",
    ));
    files.push((
        "location_groups.txt",
        b"location_group_id,location_group_name\nC1,Mokutekichi\n",
    ));
    files.push((
        "translations.txt",
        b"table_name,field_name,language,translation,record_id\n\
          location_groups,location_group_name,en,Destination,C1\n\
          location_groups,location_group_name,en,Ghost,C9\n",
    ));

    match run(&files) {
        ValidateResult::Ok(vr) => {
            let trn004: Vec<_> = vr
                .notices
                .iter()
                .filter(|n| n.rule_id == "TRN_004")
                .map(|n| n.entity_id.as_deref())
                .collect();
            assert_eq!(trn004, [Some("C9")], "yalnız var olmayan C9 raporlanmalı");
        }
        other => panic!("ValidateResult::Ok beklendi, alınan: {other:?}"),
    }
}

#[test]
fn gtfs_jp_kana_profile_keeps_profile_and_general_translation_findings_distinct() {
    let mut files = base_files();
    files.push((
        "feed_info.txt",
        b"feed_publisher_name,feed_publisher_url,feed_lang\nTest,http://test.example,ja\n",
    ));
    files.push((
        "translations.txt",
        b"table_name,field_name,language,translation,record_id\n\
          stops,stop_name,ja-Hrkt,\xE3\x81\xA8\xE3\x81\x86\xE3\x81\x8D\xE3\x82\x87\xE3\x81\x86,S1\n\
          stops,stop_name,ja-Hrkt,Tokyo,S1\n\
          stops,stop_name,ja-Hrkt,,S2\n",
    ));

    match run(&files) {
        ValidateResult::Ok(vr) => {
            // The contradiction plus Latin content at S1 can trigger two JPN_021 predicates;
            // because JPN_021 is Entity-scoped, K7 keeps one representative per record_id.
            assert_eq!(
                vr.notices.iter().filter(|n| n.rule_id == "JPN_021").count(),
                2,
                "JPN_021 S1 ve S2 için birer final bulgu üretmeli"
            );
            // The profile rule does not replace the general GTFS translations rules:
            // TRN_006 (contradiction) and TRN_008 (empty translation) remain in the same feed.
            assert!(vr.notices.iter().any(|n| n.rule_id == "TRN_006"));
            assert!(vr.notices.iter().any(|n| n.rule_id == "TRN_008"));
        }
        other => panic!("ValidateResult::Ok beklendi, alınan: {other:?}"),
    }
}

// ── issue #108: RTS_022 must not miss a one-character exact match ─────────────

fn rts022_route_name_feed(routes: &'static [u8]) -> Vec<(&'static str, &'static [u8])> {
    let mut files = base_files();
    files[2] = ("routes.txt", routes);
    files
}

#[test]
fn rts022_flags_one_character_equality_once() {
    static ROUTES: &[u8] = b"route_id,agency_id,route_short_name,route_long_name,route_type\n\
          R1,1,A,A,3\n";

    match run(&rts022_route_name_feed(ROUTES)) {
        ValidateResult::Ok(vr) => assert_eq!(
            vr.notices.iter().filter(|n| n.rule_id == "RTS_022").count(),
            1,
            "short=long tek karakter olsa da tam eşitlikte tek RTS_022 olmalı"
        ),
        other => panic!("ValidateResult::Ok beklendi, alınan: {other:?}"),
    }
}

#[test]
fn rts022_preserves_one_character_containment_guard() {
    static ROUTES: &[u8] = b"route_id,agency_id,route_short_name,route_long_name,route_type\n\
          R1,1,5,Route 5A,3\n";

    match run(&rts022_route_name_feed(ROUTES)) {
        ValidateResult::Ok(vr) => assert!(
            !vr.notices.iter().any(|n| n.rule_id == "RTS_022"),
            "eşit olmayan tek karakterli substring RTS_022 üretmemeli"
        ),
        other => panic!("ValidateResult::Ok beklendi, alınan: {other:?}"),
    }
}

#[test]
fn rts022_keeps_multicharacter_containment() {
    static ROUTES: &[u8] = b"route_id,agency_id,route_short_name,route_long_name,route_type\n\
          R1,1,5A,5A Hatt\xC4\xB1,3\n";

    match run(&rts022_route_name_feed(ROUTES)) {
        ValidateResult::Ok(vr) => assert!(
            vr.notices.iter().any(|n| n.rule_id == "RTS_022"),
            "çok karakterli containment RTS_022 üretmeye devam etmeli"
        ),
        other => panic!("ValidateResult::Ok beklendi, alınan: {other:?}"),
    }
}

// ── Test 1: Bozuk bayt → Fatal(ZipUnreadable) ─────────────────────────────────
// ARC_001 path: the pipeline stops at the first step; no rule runs.

#[test]
fn arc001_corrupt_zip_returns_fatal_zip_unreadable() {
    let result = validate_bytes(b"not a zip", &ValidatorConfig::default(), TODAY);
    match result {
        ValidateResult::Fatal(e) => assert_eq!(
            e.code,
            FatalCode::ZipUnreadable,
            "Beklenen ZipUnreadable, alınan: {:?}",
            e.code,
        ),
        _ => panic!("Fatal(ZipUnreadable) beklendi"),
    }
}

// ── Test 1b: Required file missing → PARTIAL + ARC_004 ───────────────────────
// The ZIP remains readable, so independent findings are preserved; only rules
// genuinely dependent on the missing file do not run.

#[test]
fn arc004_missing_required_file_returns_partial_report() {
    let files: Vec<(&str, &[u8])> = base_files()
        .into_iter()
        .filter(|(name, _)| *name != "routes.txt")
        .collect();
    match run(&files) {
        ValidateResult::Ok(vr) => {
            assert_eq!(vr.status, gtfs_core::ValidationStatus::Partial);
            let partial = vr.partial.expect("partial kapsamı raporlanmalı");
            assert!(partial
                .unavailable_files
                .contains(&"routes.txt".to_string()));
            assert!(!partial.skipped_stages.contains(&"K4-cross-ref".to_string()));
            assert!(partial.skipped_checks.contains(&"K4::routes".to_string()));
            assert!(partial
                .skipped_checks
                .contains(&"K6::route_headway".to_string()));
            assert!(vr.notices.iter().any(
                |n| n.rule_id == "ARC_004" && n.observed_value.as_deref() == Some("routes.txt")
            ));
        }
        other => panic!("Partial Ok bekleniyor, gelen: {other:?}"),
    }
}

#[test]
fn malformed_optional_file_is_skipped_with_partial_report() {
    // feed_info.txt is optional; malformed UTF-8 must not become a typed record,
    // but the readable feed's independent K1/K2 scope must be preserved.
    let mut files = base_files();
    files.push((
        "feed_info.txt",
        b"feed_publisher_name,feed_lang\nTest,\xFF\n",
    ));

    match run(&files) {
        ValidateResult::Ok(vr) => {
            assert_eq!(vr.status, gtfs_core::ValidationStatus::Partial);
            let partial = vr.partial.expect("partial kapsamı raporlanmalı");
            assert!(partial
                .unavailable_files
                .contains(&"feed_info.txt".to_string()));
            assert!(!partial.skipped_stages.contains(&"K4-cross-ref".to_string()));
            assert!(partial
                .skipped_checks
                .contains(&"K4::translations".to_string()));
            assert!(partial.skipped_checks.contains(&"K6::FIN_019".to_string()));
            assert!(vr
                .notices
                .iter()
                .any(|n| n.rule_id == "ARC_002" && n.file.as_deref() == Some("feed_info.txt")));
            assert!(vr
                .notices
                .iter()
                .any(|n| n.rule_id == "ARC_003" && n.file.as_deref() == Some("feed_info.txt")));
            assert!(vr.notices.iter().all(|notice| {
                !matches!(
                    notice.rule_id.as_str(),
                    "FIN_010"
                        | "FIN_016"
                        | "FIN_017"
                        | "FIN_018"
                        | "FIN_019"
                        | "FIN_020"
                        | "CAL_020"
                )
            }));
        }
        other => panic!("Partial Ok bekleniyor, gelen: {other:?}"),
    }
}

#[test]
fn malformed_calendar_does_not_produce_dq_005() {
    let mut files = base_files();
    files[5] = ("calendar.txt", b"service_id,monday,\xFF\n");

    match run(&files) {
        ValidateResult::Ok(vr) => {
            assert_eq!(vr.status, gtfs_core::ValidationStatus::Partial);
            assert!(vr.notices.iter().all(|notice| notice.rule_id != "DQ_005"));
            let partial = vr.partial.expect("bozuk calendar PARTIAL üretmeli");
            assert!(partial
                .unavailable_files
                .contains(&"calendar.txt".to_string()));
            assert!(partial.skipped_checks.contains(&"K6::DQ_005".to_string()));
        }
        other => panic!("Partial Ok bekleniyor, gelen: {other:?}"),
    }
}

#[test]
fn malformed_stop_times_does_not_produce_empty_stop_time_findings() {
    let mut files = base_files();
    files[4] = ("stop_times.txt", b"trip_id,arrival_time,\xFF\n");

    match run(&files) {
        ValidateResult::Ok(vr) => {
            assert_eq!(vr.status, gtfs_core::ValidationStatus::Partial);
            assert!(vr
                .notices
                .iter()
                .all(|notice| !matches!(notice.rule_id.as_str(), "DQ_009" | "DQ_005b")));
            let partial = vr.partial.expect("bozuk stop_times PARTIAL üretmeli");
            assert!(partial
                .unavailable_files
                .contains(&"stop_times.txt".to_string()));
            assert!(partial.skipped_checks.contains(&"K6::DQ_009".to_string()));
            assert!(partial
                .skipped_checks
                .contains(&"K6::remaining_analytics".to_string()));
        }
        other => panic!("Partial Ok bekleniyor, gelen: {other:?}"),
    }
}

#[test]
fn malformed_stops_does_not_produce_stop_count_quality_findings() {
    let mut files = base_files();
    files[1] = ("stops.txt", b"stop_id,stop_name,stop_lat,\xFF\n");

    match run(&files) {
        ValidateResult::Ok(vr) => {
            assert_eq!(vr.status, gtfs_core::ValidationStatus::Partial);
            assert!(vr.notices.iter().all(|notice| {
                !matches!(
                    notice.rule_id.as_str(),
                    "DQ_011" | "DQ_017" | "DQ_022" | "DQ_005c"
                )
            }));
            let partial = vr.partial.expect("bozuk stops PARTIAL üretmeli");
            assert!(partial.unavailable_files.contains(&"stops.txt".to_string()));
            assert!(partial.skipped_checks.contains(&"K6::DQ_011".to_string()));
            assert!(partial
                .skipped_checks
                .contains(&"K6::remaining_analytics".to_string()));
        }
        other => panic!("Partial Ok bekleniyor, gelen: {other:?}"),
    }
}

#[test]
fn malformed_feed_info_keeps_independent_cross_ref_findings() {
    let mut files = base_files();
    files[3] = ("trips.txt", TRIPS_DANGLING_ROUTE);
    files.push((
        "feed_info.txt",
        b"feed_publisher_name,feed_lang\nTest,\xFF\n",
    ));

    match run(&files) {
        ValidateResult::Ok(vr) => {
            let partial = vr.partial.expect("bozuk feed_info PARTIAL üretmeli");
            assert!(partial
                .unavailable_files
                .contains(&"feed_info.txt".to_string()));
            assert!(!partial.skipped_stages.contains(&"K4-cross-ref".to_string()));
            assert!(
                vr.notices.iter().any(|n| n.rule_id == "TRP_002"),
                "feed_info bağımsız trip/route cross-ref bulgusunu kapatmamalı"
            );
        }
        other => panic!("Partial Ok bekleniyor, gelen: {other:?}"),
    }
}

// ── Test 2: Minimal valid feed is publishable ─────────────────────────────────

#[test]
fn minimal_valid_feed_is_publishable() {
    match run(&base_files()) {
        ValidateResult::Ok(vr) => assert!(
            vr.reports.r1.publishable,
            "Minimal geçerli feed yayınlanabilir olmalı.\n\
             Blocker notice ID'leri: {:?}",
            vr.reports.r1.blocker_notice_ids,
        ),
        _ => panic!("ValidateResult::Ok beklendi"),
    }
}

// ── Test 3: STP_003 → blocks[] GEO_009 ───────────────────────────────────────
// parse_coordinate: the first rule is range_rule_id (STP_003 = out of range).
// stop_lat=999.0 → outside [-90,90] → STP_003 fires.
// The feed has no shape → GEO_009 is not produced naturally; it is present in STP_003.blocks.
// Note: SHP_013 was removed (it double-counted with GEO_009).

#[test]
fn stp_003_blocks_contain_geo_009() {
    const STOPS_BAD_LAT: &[u8] =
        b"stop_id,stop_name,stop_lat,stop_lon\nS1,Stop1,999.0,29.0\nS2,Stop2,41.1,29.1\n";

    let mut files = base_files();
    files[1] = ("stops.txt", STOPS_BAD_LAT);

    match run(&files) {
        ValidateResult::Ok(vr) => {
            let stp003 = vr.notices.iter().find(|n| n.rule_id == "STP_003");
            assert!(
                stp003.is_some(),
                "STP_003 olmalı. Mevcut rule_id'ler: {:?}",
                vr.notices
                    .iter()
                    .map(|n| n.rule_id.as_str())
                    .collect::<Vec<_>>(),
            );

            let blocks = &stp003.unwrap().blocks;
            assert!(
                blocks.iter().any(|b| b == "GEO_009"),
                "STP_003.blocks GEO_009 içermeli. Mevcut blocks: {:?}",
                blocks,
            );

            // S1 has no coordinates → GEO_009 must not fire as the root cause.
            assert!(
                !vr.notices.iter().any(|n| n.rule_id == "GEO_009"),
                "GEO_009 koordinat eksikken kök neden olarak üretilmemeli",
            );
        }
        _ => panic!("ValidateResult::Ok beklendi"),
    }
}

// ── Test 4: TRF_006 → blocks[] OPR_007 ───────────────────────────────────────
// Invalid from_trip_id ("MISSING_TRIP") → K4 fires TRF_006; scope_key is the trip ID.
// A scoped root suppresses only the same scope_key: OPR_007 carries a trip ID, while
// TRF_013..018 carry no scope_key. Those links never matched and were removed on 2026-10-11.

#[test]
fn trf_006_blocks_only_trip_scoped_opr_007() {
    const TRANSFERS: &[u8] =
        b"from_stop_id,to_stop_id,transfer_type,from_trip_id\nS1,S2,3,MISSING_TRIP\n";

    let mut files = base_files();
    files.push(("transfers.txt", TRANSFERS));

    match run(&files) {
        ValidateResult::Ok(vr) => {
            let trf006 = vr.notices.iter().find(|n| n.rule_id == "TRF_006");
            assert!(
                trf006.is_some(),
                "TRF_006 olmalı. Mevcut rule_id'ler: {:?}",
                vr.notices
                    .iter()
                    .map(|n| n.rule_id.as_str())
                    .collect::<Vec<_>>(),
            );

            let trf006 = trf006.unwrap();
            assert_eq!(trf006.scope_key.as_deref(), Some("MISSING_TRIP"));
            assert_eq!(trf006.blocks, vec!["OPR_007".to_string()]);
        }
        _ => panic!("ValidateResult::Ok beklendi"),
    }
}

// TRF_003 covers a transfer ROW (`from_stop_id|to_stop_id`). It used to be treated as a
// feed-level root because it carried no `scope_key`, so one broken stop reference hid ALL
// TRF_004 findings in the feed from R9. Now only that row's TRF_004 is a symptom.
#[test]
fn trf_003_masks_only_its_own_transfer_row() {
    const TRANSFERS: &[u8] = b"from_stop_id,to_stop_id,transfer_type\nS1,MISSING,9\nS1,S2,9\n";
    let mut files = base_files();
    files.push(("transfers.txt", TRANSFERS));

    let ValidateResult::Ok(vr) = run(&files) else {
        panic!("ValidateResult::Ok beklendi");
    };
    let trf003: Vec<_> = vr.notices.iter().filter(|n| n.rule_id == "TRF_003").collect();
    assert_eq!(trf003.len(), 1);
    assert_eq!(trf003[0].scope_key.as_deref(), Some("S1|MISSING"));
    assert_eq!(vr.notices.iter().filter(|n| n.rule_id == "TRF_004").count(), 2);

    let trf004 = vr.reports.r9.items.iter().find(|i| i.rule_id == "TRF_004");
    assert_eq!(
        trf004.map(|i| i.affected_instance_count),
        Some(1),
        "yalnız kırık satırın TRF_004'ü maskelenmeli"
    );
}

// ── Test 5: TRP_019 — shape_id is required only for CONTINUOUS SERVICE ───────
//
// 🔴 Until 2026-08-19 this test locked in the WRONG BEHAVIOR: it EXPECTED a finding
// for `continuous_pickup=1`. In the Spec, 1 (and empty) means "NO continuous pickup";
// continuous service is 0, 2, and 3. Thus the test treated requiring shape_id from a
// valid feed as correct. This produced 215,288 findings across 100 corpus feeds.

#[test]
fn trp_019_fires_when_continuous_service_is_actually_active() {
    for value in ["0", "2", "3"] {
        let routes = format!(
            "route_id,agency_id,route_short_name,route_type,continuous_pickup\nR1,1,101,3,{value}\n"
        );
        let mut files = base_files();
        files[2] = (
            "routes.txt",
            Box::leak(routes.into_bytes().into_boxed_slice()),
        );
        match run(&files) {
            ValidateResult::Ok(vr) => assert!(
                vr.notices.iter().any(|n| n.rule_id == "TRP_019"),
                "continuous_pickup={value} sürekli servistir → TRP_019 beklenir: {:?}",
                vr.notices
                    .iter()
                    .map(|n| n.rule_id.as_str())
                    .collect::<Vec<_>>(),
            ),
            _ => panic!("ValidateResult::Ok beklendi"),
        }
    }
}

#[test]
fn trp_019_stays_silent_when_continuous_service_is_declared_absent() {
    // 1 = "No continuous stopping pickup". It has the same meaning as empty and
    // does NOT REQUIRE shape_id; otherwise a valid feed would be rejected.
    for value in ["1", ""] {
        let routes = format!(
            "route_id,agency_id,route_short_name,route_type,continuous_pickup\nR1,1,101,3,{value}\n"
        );
        let mut files = base_files();
        files[2] = (
            "routes.txt",
            Box::leak(routes.into_bytes().into_boxed_slice()),
        );
        match run(&files) {
            ValidateResult::Ok(vr) => assert!(
                !vr.notices.iter().any(|n| n.rule_id == "TRP_019"),
                "continuous_pickup={value:?} sürekli servis DEĞİLDİR → TRP_019 olmamalı",
            ),
            _ => panic!("ValidateResult::Ok beklendi"),
        }
    }
}

// ── Test 6: TRP_019 — no continuous service → silent ──────────────────────────
// The route has no continuous_pickup/drop_off, nor do stop_times → no TRP_019.

#[test]
fn trp_019_silent_when_no_continuous_service() {
    match run(&base_files()) {
        ValidateResult::Ok(vr) => {
            assert!(
                !vr.notices.iter().any(|n| n.rule_id == "TRP_019"),
                "TRP_019 üretilmemeli (continuous service yok). Notices: {:?}",
                vr.notices
                    .iter()
                    .map(|n| n.rule_id.as_str())
                    .collect::<Vec<_>>(),
            );
        }
        _ => panic!("ValidateResult::Ok beklendi"),
    }
}

#[test]
fn trp_019_fires_when_stop_time_enables_continuous_service() {
    const STOP_TIMES_CONTINUOUS: &[u8] =
        b"trip_id,arrival_time,departure_time,stop_id,stop_sequence,continuous_drop_off\n\
          T1,08:00:00,08:00:00,S1,1,2\nT1,08:10:00,08:10:00,S2,2,2\n";

    let mut files = base_files();
    files[4] = ("stop_times.txt", STOP_TIMES_CONTINUOUS);

    match run(&files) {
        ValidateResult::Ok(vr) => assert!(
            vr.notices.iter().any(|n| n.rule_id == "TRP_019"),
            "stop_times continuous_drop_off aktifken shape_id eksikliği TRP_019 üretmeli",
        ),
        _ => panic!("ValidateResult::Ok beklendi"),
    }
}

// ── RTS_028: routes.continuous_* while any trip on the route has a Flex window ──
// Route-scoped condition (MD `forbidden_continuous_pickup_drop_off` parity). The violating
// row is in routes.txt, NOT stop_times; another row may carry the window.

// T1's second stop has a Flex window (T1 → R1) — R1 therefore becomes a "Flex route".
static STOP_TIMES_FLEX_WINDOW: &[u8] =
    b"trip_id,arrival_time,departure_time,stop_id,stop_sequence,location_id,start_pickup_drop_off_window,end_pickup_drop_off_window,pickup_booking_rule_id\n\
      T1,08:00:00,08:00:00,S1,1,,,,\n\
      T1,,,,2,Z1,09:00:00,17:00:00,BR1\n";

#[test]
fn rts_028_fires_when_route_has_flex_window_trip_and_continuous_pickup() {
    const ROUTES_CONTINUOUS: &[u8] =
        b"route_id,agency_id,route_short_name,route_type,continuous_pickup\n\
          R1,1,101,3,0\n";

    let mut files = base_files();
    files[2] = ("routes.txt", ROUTES_CONTINUOUS);
    files[4] = ("stop_times.txt", STOP_TIMES_FLEX_WINDOW);

    match run(&files) {
        ValidateResult::Ok(vr) => {
            let n = vr.notices.iter().find(|n| n.rule_id == "RTS_028");
            assert!(
                n.is_some(),
                "RTS_028 olmalı (rotanın seferinde Flex penceresi + continuous_pickup=0). Mevcut: {:?}",
                vr.notices.iter().map(|n| n.rule_id.as_str()).collect::<Vec<_>>(),
            );
            let n = n.unwrap();
            assert_eq!(
                n.file.as_deref(),
                Some("routes.txt"),
                "ihlal routes.txt'te raporlanmalı"
            );
            assert_eq!(n.field.as_deref(), Some("continuous_pickup"));
        }
        _ => panic!("ValidateResult::Ok beklendi"),
    }
}

#[test]
fn rts_028_silent_when_route_has_no_flex_window() {
    // continuous_pickup=0 but no trip has a window → Optional according to the Spec.
    const ROUTES_CONTINUOUS: &[u8] =
        b"route_id,agency_id,route_short_name,route_type,continuous_pickup\n\
          R1,1,101,3,0\n";

    let mut files = base_files();
    files[2] = ("routes.txt", ROUTES_CONTINUOUS);

    match run(&files) {
        ValidateResult::Ok(vr) => assert!(
            !vr.notices.iter().any(|n| n.rule_id == "RTS_028"),
            "Flex penceresi olmayan rotada RTS_028 üretilmemeli: {:?}",
            vr.notices
                .iter()
                .map(|n| n.rule_id.as_str())
                .collect::<Vec<_>>(),
        ),
        _ => panic!("ValidateResult::Ok beklendi"),
    }
}

#[test]
fn rts_028_silent_when_continuous_is_one() {
    // Route has a Flex window but continuous_pickup=1 (allowed value) → silent.
    const ROUTES_CONTINUOUS: &[u8] =
        b"route_id,agency_id,route_short_name,route_type,continuous_pickup,continuous_drop_off\n\
          R1,1,101,3,1,\n";

    let mut files = base_files();
    files[2] = ("routes.txt", ROUTES_CONTINUOUS);
    files[4] = ("stop_times.txt", STOP_TIMES_FLEX_WINDOW);

    match run(&files) {
        ValidateResult::Ok(vr) => assert!(
            !vr.notices.iter().any(|n| n.rule_id == "RTS_028"),
            "continuous_pickup=1 → RTS_028 üretilmemeli: {:?}",
            vr.notices
                .iter()
                .map(|n| n.rule_id.as_str())
                .collect::<Vec<_>>(),
        ),
        _ => panic!("ValidateResult::Ok beklendi"),
    }
}

/// Measurement from the 17th corpus run (`tfs-789`): all 829 transfer rows were above 2 km,
/// but there were only 57 distinct stop pairs, averaging 14.5 rows per pair. The rule's
/// identity field is already `from_stop_id|to_stop_id`, i.e. the pair; emitting per row
/// contradicted its own declaration. MD reported 69 for `transfer_distance_above_2_km` —
/// it also counts per pair.
/// Measurement from the 17th corpus run (`mdb-2653`): 108 of 421 trips without a `stop_times`
/// record had whitespace in `trip_id`, and all 108 were suppressed by K7 — we reported 313
/// while MD reported 732. Suppression assumed that the ID failed to match only because of
/// whitespace by consulting the `trip_id` reference set, but that set is populated from the
/// IDs in `trips.txt` ITSELF, and `XFL_002`'s observed value is already the trip's own ID —
/// the condition was therefore always true. The correct set is the IDs PRESENT in `stop_times`.
#[test]
fn xfl_002_survives_suppression_when_the_trip_is_absent_from_stop_times() {
    let trips = "route_id,service_id,trip_id\nR1,SVC1,T1 \nR1,SVC1,T2 \n";
    let stop_times = "trip_id,arrival_time,departure_time,stop_id,stop_sequence\nT2,08:00:00,08:00:00,S1,1\nT2,08:10:00,08:10:00,S2,2\n";
    let mut files = base_files();
    for f in files.iter_mut() {
        if f.0 == "trips.txt" {
            *f = ("trips.txt", trips.as_bytes());
        } else if f.0 == "stop_times.txt" {
            *f = ("stop_times.txt", stop_times.as_bytes());
        }
    }
    match run(&files) {
        ValidateResult::Ok(vr) => {
            let ids: Vec<&str> = vr
                .notices
                .iter()
                .filter(|n| n.rule_id == "XFL_002")
                .filter_map(|n| n.entity_id.as_deref())
                .collect();
            // `T1 ` is NOT present in stop_times under any spelling → the finding is REAL and must remain visible.
            assert!(
                ids.iter().any(|id| id.trim() == "T1"),
                "stop_times'ta hiç geçmeyen sefer bastırılmamalı: {ids:?}"
            );
            // `T2 ` is PRESENT in stop_times after trimming → whitespace is the only difference, so suppress it.
            assert!(
                !ids.iter().any(|id| id.trim() == "T2"),
                "yalnız boşlukla ayrılan sefer DQ_016'nın konusudur, ayrı bulgu değil: {ids:?}"
            );
            // Suppression must not be untraceable: the DQ_016 root must declare the count.
            let root = vr
                .notices
                .iter()
                .find(|n| n.rule_id == "DQ_016" && n.file.as_deref() == Some("trips.txt"))
                .expect("trips.txt için DQ_016 kökü bekleniyor");
            assert!(root
                .details
                .as_ref()
                .and_then(|d| d.get("suppressed_derivative_rules"))
                .is_some_and(|v| v.contains("XFL_002")));
        }
        _ => panic!("ValidateResult::Ok beklendi"),
    }
}

#[test]
fn trf_011_skips_null_island_stops_and_leaves_the_root_to_geo_016() {
    // SN (0,0) placeholder: S1→SN distance is meaningless, with GEO_016 as the root. S1→S2 is a real ~13.7 km.
    let stops: &[u8] =
        b"stop_id,stop_name,stop_lat,stop_lon\nS1,Stop1,41.0,29.0\nS2,Stop2,41.1,29.1\nSN,StopNull,0.0,0.0\n";
    let transfers = "from_stop_id,to_stop_id,transfer_type,min_transfer_time\nS1,SN,2,300\nSN,S2,2,300\nS1,S2,2,300\n";
    let mut files = base_files();
    files[1] = ("stops.txt", stops);
    files.push(("transfers.txt", transfers.as_bytes()));
    match run(&files) {
        ValidateResult::Ok(vr) => {
            let pairs: Vec<_> = vr
                .notices
                .iter()
                .filter(|n| n.rule_id == "TRF_011")
                .map(|n| n.entity_id.as_deref())
                .collect();
            assert_eq!(
                pairs,
                vec![Some("S1|S2")],
                "Null Island çifti TRF_011 üretmemeli"
            );
            assert!(
                vr.notices
                    .iter()
                    .any(|n| n.rule_id == "GEO_016" && n.entity_id.as_deref() == Some("SN")),
                "kök neden GEO_016 olarak raporlanmalı"
            );
        }
        _ => panic!("ValidateResult::Ok beklendi"),
    }
}

#[test]
fn trf_011_reports_one_finding_per_stop_pair_not_per_transfer_row() {
    // The same distant pair appears in THREE rows, and the reverse pair in one row (S1↔S2 ~13.7 km).
    let transfers = "from_stop_id,to_stop_id,transfer_type,min_transfer_time\nS1,S2,2,300\nS1,S2,2,300\nS1,S2,2,300\nS2,S1,2,300\n";
    let mut files = base_files();
    files.push(("transfers.txt", transfers.as_bytes()));
    match run(&files) {
        ValidateResult::Ok(vr) => {
            let hits: Vec<_> = vr
                .notices
                .iter()
                .filter(|n| n.rule_id == "TRF_011")
                .collect();
            assert_eq!(
                hits.len(),
                2,
                "iki yönlü iki çift → iki bulgu, dört satır değil: {:?}",
                hits.iter()
                    .map(|n| n.entity_id.as_deref())
                    .collect::<Vec<_>>()
            );
            let three = hits
                .iter()
                .find(|n| n.entity_id.as_deref() == Some("S1|S2"))
                .expect("S1|S2 çifti bekleniyor");
            assert_eq!(
                three
                    .details
                    .as_ref()
                    .and_then(|d| d.get("transfer_rows"))
                    .map(String::as_str),
                Some("3"),
                "üç satırda tekrarlanan çift sayıyı details'te taşımalı"
            );
            // A single-row pair carries no `details` — do not emit an unnecessary field.
            let single = hits
                .iter()
                .find(|n| n.entity_id.as_deref() == Some("S2|S1"))
                .expect("S2|S1 çifti bekleniyor");
            assert!(single.details.is_none());
        }
        _ => panic!("ValidateResult::Ok beklendi"),
    }
}

#[test]
fn transfer_stop_requirements_cover_recommended_and_in_seat_types() {
    for transfer_type in ["0", "1", "2", "3"] {
        let transfers = format!("from_stop_id,to_stop_id,transfer_type\n,,{transfer_type}\n");
        let mut files = base_files();
        files.push(("transfers.txt", transfers.as_bytes()));
        match run(&files) {
            ValidateResult::Ok(vr) => {
                assert!(
                    vr.notices.iter().any(|n| n.rule_id == "TRF_001"),
                    "type={transfer_type}"
                );
                assert!(
                    vr.notices.iter().any(|n| n.rule_id == "TRF_002"),
                    "type={transfer_type}"
                );
            }
            _ => panic!("ValidateResult::Ok beklendi"),
        }
    }

    let mut files = base_files();
    files.push((
        "transfers.txt",
        b"from_stop_id,to_stop_id,transfer_type\nS1,,\n,S2,\n",
    ));
    match run(&files) {
        ValidateResult::Ok(vr) => {
            assert!(vr.notices.iter().any(|n| n.rule_id == "TRF_001"));
            assert!(vr.notices.iter().any(|n| n.rule_id == "TRF_002"));
        }
        _ => panic!("ValidateResult::Ok beklendi"),
    }

    for transfer_type in ["4", "5"] {
        let transfers = format!(
            "from_stop_id,to_stop_id,transfer_type,from_trip_id,to_trip_id\n,,{transfer_type},T1,T1\n"
        );
        let mut files = base_files();
        files.push(("transfers.txt", transfers.as_bytes()));
        match run(&files) {
            ValidateResult::Ok(vr) => assert!(
                !vr.notices
                    .iter()
                    .any(|n| n.rule_id == "TRF_001" || n.rule_id == "TRF_002"),
                "in-seat transfer_type={transfer_type} stop kimliği istememeli",
            ),
            _ => panic!("ValidateResult::Ok beklendi"),
        }
    }
}

#[test]
fn pathway_recommendations_respect_mode_boundaries() {
    const PATHWAYS: &[u8] =
        b"pathway_id,from_stop_id,to_stop_id,pathway_mode,is_bidirectional,stair_count,max_slope\n\
          WALK,S1,S2,1,0,,0.05\n\
          STAIRS,S1,S2,2,0,,\n\
          MOVING,S1,S2,3,0,,0.05\n\
          ELEVATOR,S1,S2,5,0,,0.05\n";
    let mut files = base_files();
    files.push(("pathways.txt", PATHWAYS));

    match run(&files) {
        ValidateResult::Ok(vr) => {
            assert!(vr.notices.iter().any(|n| n.rule_id == "PTH_008"));
            // 2026-08-02: this case moved from PTH_017 to PTH_028. PTH_017 now measures only
            // the TYPE violation (non-numeric max_slope); the "used with the wrong pathway_mode"
            // branch is a separate Quality rule because the Spec says "should".
            let wrong_mode = vr
                .notices
                .iter()
                .filter(|n| n.rule_id == "PTH_028")
                .collect::<Vec<_>>();
            assert_eq!(
                wrong_mode.len(),
                1,
                "yalnız elevator max_slope bağlam ihlali olmalı"
            );
            assert!(
                !vr.notices.iter().any(|n| n.rule_id == "PTH_017"),
                "geçerli sayılar PTH_017 (tip ihlali) üretmemeli"
            );
        }
        _ => panic!("ValidateResult::Ok beklendi"),
    }
}

// ── Test 7: PTH_014 — pathway connecting different stations → fires ───────────
// PLT_A parent=STA1, PLT_B parent=STA2; PW1 PLT_A→PLT_B → cross-station ihlali.

#[test]
fn pth_014_fires_for_cross_station_pathway() {
    const STOPS_TWO_STATIONS: &[u8] =
        b"stop_id,stop_name,stop_lat,stop_lon,location_type,parent_station\n\
          S1,Stop1,41.0,29.0,,\n\
          S2,Stop2,41.1,29.1,,\n\
          STA1,Station1,41.2,29.2,1,\n\
          STA2,Station2,41.3,29.3,1,\n\
          PLT_A,PlatformA,41.2,29.2,0,STA1\n\
          PLT_B,PlatformB,41.3,29.3,0,STA2\n";
    const PATHWAYS_CROSS: &[u8] =
        b"pathway_id,from_stop_id,to_stop_id,pathway_mode,is_bidirectional\n\
          PW1,PLT_A,PLT_B,1,0\n";

    let mut files = base_files();
    files[1] = ("stops.txt", STOPS_TWO_STATIONS);
    files.push(("pathways.txt", PATHWAYS_CROSS));

    match run(&files) {
        ValidateResult::Ok(vr) => {
            assert!(
                vr.notices.iter().any(|n| n.rule_id == "PTH_014"),
                "PTH_014 olmalı (cross-station pathway). Mevcut: {:?}",
                vr.notices
                    .iter()
                    .map(|n| n.rule_id.as_str())
                    .collect::<Vec<_>>(),
            );
        }
        _ => panic!("ValidateResult::Ok beklendi"),
    }
}

// ── Test 8: PTH_014 — pathway within one station → silent ─────────────────────
// PLT_A and PLT_B belong to the same station (STA1) → no PTH_014.

#[test]
fn pth_014_silent_for_same_station_pathway() {
    const STOPS_ONE_STATION: &[u8] =
        b"stop_id,stop_name,stop_lat,stop_lon,location_type,parent_station\n\
          S1,Stop1,41.0,29.0,,\n\
          S2,Stop2,41.1,29.1,,\n\
          STA1,Station1,41.2,29.2,1,\n\
          PLT_A,PlatformA,41.2,29.2,0,STA1\n\
          PLT_B,PlatformB,41.25,29.25,0,STA1\n";
    const PATHWAYS_SAME: &[u8] =
        b"pathway_id,from_stop_id,to_stop_id,pathway_mode,is_bidirectional\n\
          PW1,PLT_A,PLT_B,1,0\n";

    let mut files = base_files();
    files[1] = ("stops.txt", STOPS_ONE_STATION);
    files.push(("pathways.txt", PATHWAYS_SAME));

    match run(&files) {
        ValidateResult::Ok(vr) => {
            assert!(
                !vr.notices.iter().any(|n| n.rule_id == "PTH_014"),
                "PTH_014 üretilmemeli (aynı istasyon). Notices: {:?}",
                vr.notices
                    .iter()
                    .map(|n| n.rule_id.as_str())
                    .collect::<Vec<_>>(),
            );
        }
        _ => panic!("ValidateResult::Ok beklendi"),
    }
}

// ── Test 9: STP_016 — two stops at the same coordinates → warning ─────────────

#[test]
fn stp_016_fires_for_stops_at_identical_coordinates() {
    const STOPS_SAME_COORD: &[u8] = b"stop_id,stop_name,stop_lat,stop_lon\n\
          S1,Stop1,41.0,29.0\nS2,Stop2,41.0,29.0\n"; // S1 and S2 have the same coordinates

    let mut files = base_files();
    files[1] = ("stops.txt", STOPS_SAME_COORD);

    match run(&files) {
        ValidateResult::Ok(vr) => {
            assert!(
                vr.notices.iter().any(|n| n.rule_id == "STP_016"),
                "STP_016 olmalı. Mevcut: {:?}",
                vr.notices
                    .iter()
                    .map(|n| n.rule_id.as_str())
                    .collect::<Vec<_>>(),
            );
        }
        _ => panic!("ValidateResult::Ok beklendi"),
    }
}

// ── Test 10: STP_016 — stops at different coordinates are silent ──────────────

#[test]
fn stp_016_silent_for_stops_at_different_coordinates() {
    match run(&base_files()) {
        ValidateResult::Ok(vr) => {
            assert!(
                !vr.notices.iter().any(|n| n.rule_id == "STP_016"),
                "STP_016 üretilmemeli (S1 ve S2 farklı koordinat).",
            );
        }
        _ => panic!("ValidateResult::Ok beklendi"),
    }
}

// ── Test 10b: STP_016 — parent/child at the same coordinates is silent ─────────
// A child stop may share coordinates with its station (normal GTFS).
// STP_016 must not flag this pair as an FP.

#[test]
fn stp_016_silent_for_parent_child_at_identical_coordinates() {
    const STOPS_PARENT_CHILD: &[u8] =
        b"stop_id,stop_name,stop_lat,stop_lon,location_type,parent_station\n\
          ST1,Station,41.0,29.0,1,\n\
          ST1-01,Platform,41.0,29.0,0,ST1\n";

    let mut files = base_files();
    files[1] = ("stops.txt", STOPS_PARENT_CHILD);

    match run(&files) {
        ValidateResult::Ok(vr) => assert!(
            !vr.notices.iter().any(|n| n.rule_id == "STP_016"),
            "STP_016 üretilmemeli (çocuk durak parent istasyonla aynı koordinatta).",
        ),
        _ => panic!("ValidateResult::Ok beklendi"),
    }
}

// ── Test 10c: TRN_017 — field_value modunda record_sub_id muaf ───────────────
// If the stop_times translation is matched by field_value, record_id/record_sub_id
// must be empty (Spec). TRN_010/TRN_017 must not fire in this mode; in record_id mode,
// only TRN_017 should fire for a missing record_sub_id.

#[test]
fn trn_017_silent_in_field_value_mode_but_fires_in_record_id_mode() {
    const TR_FIELD_VALUE: &[u8] =
        b"table_name,field_name,language,translation,record_id,record_sub_id,field_value\n\
          stop_times,stop_headsign,ja,Hedef,,,Headsign\n";
    const TR_RECORD_ID: &[u8] =
        b"table_name,field_name,language,translation,record_id,record_sub_id,field_value\n\
          stop_times,stop_headsign,ja,Hedef,T1,,\n";

    let mut files = base_files();
    files.push(("translations.txt", TR_FIELD_VALUE));
    match run(&files) {
        ValidateResult::Ok(vr) => assert!(
            !vr.notices.iter().any(|n| n.rule_id == "TRN_010"),
            "field_value modunda TRN_010 üretilmemeli.",
        ),
        _ => panic!("ValidateResult::Ok beklendi"),
    }

    let mut files = base_files();
    files.push(("translations.txt", TR_RECORD_ID));
    match run(&files) {
        ValidateResult::Ok(vr) => assert!(
            vr.notices.iter().any(|n| n.rule_id == "TRN_017")
                && !vr.notices.iter().any(|n| n.rule_id == "TRN_010"),
            "record_id modunda record_sub_id yoksa yalnız TRN_017 ateşlenmeli.",
        ),
        _ => panic!("ValidateResult::Ok beklendi"),
    }
}

// ── Test 10d: TRN_007 — translations in feed_lang are grouped into one summary ─

#[test]
fn trn_007_same_language_translations_aggregate_to_feed_summary() {
    const FEED_INFO: &[u8] =
        b"feed_publisher_name,feed_publisher_url,feed_lang\nP,http://x.example,ja\n";
    // Two ja translations (feed_lang=ja) → one feed-level TRN_007 instead of two per row.
    const TR_JA: &[u8] =
        b"table_name,field_name,language,translation,record_id,record_sub_id,field_value\n\
          stops,stop_name,ja,S1 Stop,S1,,\n\
          stops,stop_name,ja,S2 Stop,S2,,\n";
    let mut files = base_files();
    files.push(("feed_info.txt", FEED_INFO));
    files.push(("translations.txt", TR_JA));
    match run(&files) {
        ValidateResult::Ok(vr) => {
            let trn: Vec<_> = vr
                .notices
                .iter()
                .filter(|n| n.rule_id == "TRN_007")
                .collect();
            assert_eq!(
                trn.len(),
                1,
                "aynı dildeki çeviriler tek feed-özetinde toplanmalı"
            );
            assert_eq!(
                trn[0].observed_value.as_deref(),
                Some("2"),
                "etkilenen satır sayısı 2"
            );
        }
        _ => panic!("ValidateResult::Ok beklendi"),
    }
}

// ── Test 11: PTH_012 + PTH_013 — inaccessible platform → warning ──────────────
// Station: STA → ENT (loc=2), MID (loc=3), PLT (loc=0).
// Pathway: ENT↔MID — PLT is unreachable from every entrance → PTH_012.
// PLT is also unreachable in accessible BFS → PTH_013 (no wheelchair path).

#[test]
fn pth_012_fires_when_entrance_cannot_reach_platform() {
    const STOPS_WITH_STATION: &[u8] =
        b"stop_id,stop_name,stop_lat,stop_lon,location_type,parent_station\n\
          S1,Stop1,41.0,29.0,,\n\
          S2,Stop2,41.1,29.1,,\n\
          STA,Station,41.05,29.05,1,\n\
          ENT,Entrance,41.05,29.05,2,STA\n\
          MID,Middle,41.05,29.05,3,STA\n\
          PLT,Platform,41.05,29.05,0,STA\n";
    // PW1: ENT↔MID — no pathway connection to PLT
    const PATHWAYS: &[u8] =
        b"pathway_id,from_stop_id,to_stop_id,pathway_mode,is_bidirectional,traversal_time\n\
          PW1,ENT,MID,1,1,30\n";

    let mut files = base_files();
    files[1] = ("stops.txt", STOPS_WITH_STATION);
    files.push(("pathways.txt", PATHWAYS));

    match run(&files) {
        ValidateResult::Ok(vr) => {
            assert!(
                vr.notices.iter().any(|n| n.rule_id == "PTH_012"),
                "PTH_012 olmalı. Mevcut: {:?}",
                vr.notices
                    .iter()
                    .map(|n| n.rule_id.as_str())
                    .collect::<Vec<_>>(),
            );
            // Coverage information for PTH_013 must also be produced.
            assert!(
                vr.notices.iter().any(|n| n.rule_id == "PTH_013"),
                "PTH_013 bilgi notu olmalı",
            );
        }
        _ => panic!("ValidateResult::Ok beklendi"),
    }
}

// ── Test 12: PTH_013 negative — silent when an accessible route exists ─────────
// ENT→PLT: max_slope=0.05 (≤0.08) AND min_width=1.0 (≥0.9) → accessible path exists.
// PTH_013 must not be produced; neither should PTH_012.

#[test]
fn pth_013_silent_when_accessible_path_exists() {
    const STOPS_ACCESSIBLE: &[u8] =
        b"stop_id,stop_name,stop_lat,stop_lon,location_type,parent_station\n\
          S1,Stop1,41.0,29.0,,\n\
          S2,Stop2,41.1,29.1,,\n\
          STA,Station,41.05,29.05,1,\n\
          ENT,Entrance,41.05,29.05,2,STA\n\
          PLT,Platform,41.05,29.05,0,STA\n";
    // PW1: ENT↔PLT, max_slope=0.05 ≤ 0.08, min_width=1.0 ≥ 0.9
    const PATHWAYS_ACCESSIBLE: &[u8] =
        b"pathway_id,from_stop_id,to_stop_id,pathway_mode,is_bidirectional,traversal_time,max_slope,min_width\n\
          PW1,ENT,PLT,1,1,30,0.05,1.0\n";

    let mut files = base_files();
    files[1] = ("stops.txt", STOPS_ACCESSIBLE);
    files.push(("pathways.txt", PATHWAYS_ACCESSIBLE));

    match run(&files) {
        ValidateResult::Ok(vr) => {
            assert!(
                !vr.notices.iter().any(|n| n.rule_id == "PTH_013"),
                "PTH_013 üretilmemeli (accessible path mevcut). Notices: {:?}",
                vr.notices
                    .iter()
                    .map(|n| n.rule_id.as_str())
                    .collect::<Vec<_>>(),
            );
            assert!(
                !vr.notices.iter().any(|n| n.rule_id == "PTH_012"),
                "PTH_012 üretilmemeli (PLT erişilebilir). Notices: {:?}",
                vr.notices
                    .iter()
                    .map(|n| n.rule_id.as_str())
                    .collect::<Vec<_>>(),
            );
        }
        _ => panic!("ValidateResult::Ok beklendi"),
    }
}

// ── Test 13: PTH_015 — speed > 3.0 m/s → fires ────────────────────────────────
// length=300m, traversal_time=50s → speed=6.0 m/s > threshold (3.0) → PTH_015.

#[test]
fn pth_015_fires_when_speed_exceeds_3ms() {
    const PATHWAYS_HIGH_SPEED: &[u8] =
        b"pathway_id,from_stop_id,to_stop_id,pathway_mode,is_bidirectional,traversal_time,length\n\
          PW1,S1,S2,1,0,50,300.0\n";

    let mut files = base_files();
    files.push(("pathways.txt", PATHWAYS_HIGH_SPEED));

    match run(&files) {
        ValidateResult::Ok(vr) => {
            assert!(
                vr.notices.iter().any(|n| n.rule_id == "PTH_015"),
                "PTH_015 olmalı (hız=6.0 m/s). Mevcut: {:?}",
                vr.notices
                    .iter()
                    .map(|n| n.rule_id.as_str())
                    .collect::<Vec<_>>(),
            );
        }
        _ => panic!("ValidateResult::Ok beklendi"),
    }
}

// ── Test 14: PTH_015 — speed ≤ 3.0 m/s → silent ───────────────────────────────
// length=100m, traversal_time=60s → speed≈1.67 m/s ≤ threshold → no PTH_015.

#[test]
fn pth_015_silent_when_speed_within_limit() {
    const PATHWAYS_NORMAL_SPEED: &[u8] =
        b"pathway_id,from_stop_id,to_stop_id,pathway_mode,is_bidirectional,traversal_time,length\n\
          PW1,S1,S2,1,0,60,100.0\n";

    let mut files = base_files();
    files.push(("pathways.txt", PATHWAYS_NORMAL_SPEED));

    match run(&files) {
        ValidateResult::Ok(vr) => {
            assert!(
                !vr.notices.iter().any(|n| n.rule_id == "PTH_015"),
                "PTH_015 üretilmemeli (hız≈1.67 m/s). Notices: {:?}",
                vr.notices
                    .iter()
                    .map(|n| n.rule_id.as_str())
                    .collect::<Vec<_>>(),
            );
        }
        _ => panic!("ValidateResult::Ok beklendi"),
    }
}

// ── Test 16: R9 score calculation — critical notice produces priority_score > 0 ─
// STP_003 (Critical, base_effort=2) → visible in R9 and included in priority_score.

#[test]
fn r9_priority_score_positive_for_critical_notice() {
    const STOPS_BAD_LAT: &[u8] =
        b"stop_id,stop_name,stop_lat,stop_lon\nS1,Stop1,999.0,29.0\nS2,Stop2,41.1,29.1\n";

    let mut files = base_files();
    files[1] = ("stops.txt", STOPS_BAD_LAT);

    match run(&files) {
        ValidateResult::Ok(vr) => {
            assert!(
                !vr.reports.r9.items.is_empty(),
                "STP_003 (Kritik) mevcut — R9 en az 1 item içermeli",
            );
            assert!(
                vr.reports.r9.items.iter().any(|i| i.priority_score > 0.0),
                "En az 1 R9 item priority_score > 0 olmalı. Items: {:?}",
                vr.reports
                    .r9
                    .items
                    .iter()
                    .map(|i| (i.rule_id.as_str(), i.priority_score))
                    .collect::<Vec<_>>(),
            );
        }
        _ => panic!("ValidateResult::Ok beklendi"),
    }
}

// ── Test 16b: PTH_019 — must fire when both directions use two ROWS ────────────
// #158: the rule counted pathway ROWS. In feeds with one row per direction between
// the same two stops, the count is 2 and the rule stayed silent even for a dead-end node.
// MobilityData says "only one incident LOCATION"; the measure is the neighbor count.

#[test]
fn pth_019_fires_when_one_neighbour_is_written_as_two_rows() {
    const STOPS: &[u8] = b"stop_id,stop_name,stop_lat,stop_lon,location_type,parent_station\n\
          S1,Stop1,41.0,29.0,,\n\
          S2,Stop2,41.1,29.1,,\n\
          STA1,Station1,41.2,29.2,1,\n\
          PLT_A,PlatformA,41.2,29.2,0,STA1\n\
          GN1,GenericNode1,41.2,29.21,3,STA1\n";
    // One neighbor (PLT_A), two rows — one outbound and one return.
    const PATHWAYS: &[u8] = b"pathway_id,from_stop_id,to_stop_id,pathway_mode,is_bidirectional\n\
          PW1,PLT_A,GN1,1,0\n\
          PW2,GN1,PLT_A,1,0\n";

    let mut files = base_files();
    files[1] = ("stops.txt", STOPS);
    files.push(("pathways.txt", PATHWAYS));

    match run(&files) {
        ValidateResult::Ok(vr) => {
            assert!(
                vr.notices.iter().any(|n| n.rule_id == "PTH_019"),
                "İki satır tek komşu demektir; PTH_019 ateşlemeli. Mevcut: {:?}",
                vr.notices
                    .iter()
                    .map(|n| n.rule_id.as_str())
                    .collect::<Vec<_>>(),
            );
        }
        _ => panic!("ValidateResult::Ok beklendi"),
    }
}

// ── Test 16c: PTH_019 — a node with two DIFFERENT neighbors must be silent ─────

#[test]
fn pth_019_silent_for_a_generic_node_with_two_neighbours() {
    const STOPS: &[u8] = b"stop_id,stop_name,stop_lat,stop_lon,location_type,parent_station\n\
          S1,Stop1,41.0,29.0,,\n\
          S2,Stop2,41.1,29.1,,\n\
          STA1,Station1,41.2,29.2,1,\n\
          PLT_A,PlatformA,41.2,29.2,0,STA1\n\
          PLT_B,PlatformB,41.2,29.22,0,STA1\n\
          GN1,GenericNode1,41.2,29.21,3,STA1\n";
    const PATHWAYS: &[u8] = b"pathway_id,from_stop_id,to_stop_id,pathway_mode,is_bidirectional\n\
          PW1,PLT_A,GN1,1,0\n\
          PW2,GN1,PLT_B,1,0\n";

    let mut files = base_files();
    files[1] = ("stops.txt", STOPS);
    files.push(("pathways.txt", PATHWAYS));

    match run(&files) {
        ValidateResult::Ok(vr) => {
            assert!(
                !vr.notices.iter().any(|n| n.rule_id == "PTH_019"),
                "İki farklı komşu → çıkmaz değil. Mevcut: {:?}",
                vr.notices
                    .iter()
                    .map(|n| n.rule_id.as_str())
                    .collect::<Vec<_>>(),
            );
        }
        _ => panic!("ValidateResult::Ok beklendi"),
    }
}

// ── Test 16: PTH_019 — generic node with one pathway → fires ──────────────────
// GN1 (location_type=3) occurs in only 1 pathway → dead end → PTH_019.

#[test]
fn pth_019_fires_for_dangling_generic_node() {
    const STOPS: &[u8] = b"stop_id,stop_name,stop_lat,stop_lon,location_type,parent_station\n\
          S1,Stop1,41.0,29.0,,\n\
          S2,Stop2,41.1,29.1,,\n\
          STA1,Station1,41.2,29.2,1,\n\
          PLT_A,PlatformA,41.2,29.2,0,STA1\n\
          GN1,GenericNode1,41.2,29.21,3,STA1\n";
    const PATHWAYS: &[u8] = b"pathway_id,from_stop_id,to_stop_id,pathway_mode,is_bidirectional\n\
          PW1,PLT_A,GN1,1,0\n";

    let mut files = base_files();
    files[1] = ("stops.txt", STOPS);
    files.push(("pathways.txt", PATHWAYS));

    match run(&files) {
        ValidateResult::Ok(vr) => {
            assert!(
                vr.notices.iter().any(|n| n.rule_id == "PTH_019"),
                "PTH_019 olmalı (dangling generic node). Mevcut: {:?}",
                vr.notices
                    .iter()
                    .map(|n| n.rule_id.as_str())
                    .collect::<Vec<_>>(),
            );
        }
        _ => panic!("ValidateResult::Ok beklendi"),
    }
}

// ── Test 17: PTH_019 — generic node with two pathways → silent ────────────────
// GN1 is connected by both PLT_A→GN1 and GN1→PLT_B → bridge → no PTH_019.

#[test]
fn pth_019_silent_for_connected_generic_node() {
    const STOPS: &[u8] = b"stop_id,stop_name,stop_lat,stop_lon,location_type,parent_station\n\
          S1,Stop1,41.0,29.0,,\n\
          S2,Stop2,41.1,29.1,,\n\
          STA1,Station1,41.2,29.2,1,\n\
          PLT_A,PlatformA,41.2,29.2,0,STA1\n\
          PLT_B,PlatformB,41.2,29.25,0,STA1\n\
          GN1,GenericNode1,41.2,29.21,3,STA1\n";
    const PATHWAYS: &[u8] = b"pathway_id,from_stop_id,to_stop_id,pathway_mode,is_bidirectional\n\
          PW1,PLT_A,GN1,1,0\n\
          PW2,GN1,PLT_B,1,0\n";

    let mut files = base_files();
    files[1] = ("stops.txt", STOPS);
    files.push(("pathways.txt", PATHWAYS));

    match run(&files) {
        ValidateResult::Ok(vr) => {
            assert!(
                !vr.notices.iter().any(|n| n.rule_id == "PTH_019"),
                "PTH_019 üretilmemeli (connected generic node). Notices: {:?}",
                vr.notices
                    .iter()
                    .map(|n| n.rule_id.as_str())
                    .collect::<Vec<_>>(),
            );
        }
        _ => panic!("ValidateResult::Ok beklendi"),
    }
}

// ── Test 18: LVL_006 — elevator pathway lacks level_id → fires ─────────────────
// ELV_STOP is connected to an elevator but has no level_id → LVL_006.

#[test]
fn lvl_006_fires_when_elevator_stop_missing_level_id() {
    const STOPS: &[u8] =
        b"stop_id,stop_name,stop_lat,stop_lon,location_type,parent_station,level_id\n\
          S1,Stop1,41.0,29.0,,,\n\
          S2,Stop2,41.1,29.1,,,\n\
          STA1,Station1,41.2,29.2,1,,\n\
          PLT_A,PlatformA,41.2,29.2,0,STA1,L1\n\
          ELV_STOP,ElevatorStop,41.2,29.21,0,STA1,\n";
    const PATHWAYS: &[u8] = b"pathway_id,from_stop_id,to_stop_id,pathway_mode,is_bidirectional\n\
          PW1,PLT_A,ELV_STOP,5,1\n";

    let mut files = base_files();
    files[1] = ("stops.txt", STOPS);
    files.push(("pathways.txt", PATHWAYS));

    match run(&files) {
        ValidateResult::Ok(vr) => {
            assert!(
                vr.notices.iter().any(|n| n.rule_id == "LVL_006"),
                "LVL_006 olmalı (asansör stop level_id eksik). Mevcut: {:?}",
                vr.notices
                    .iter()
                    .map(|n| n.rule_id.as_str())
                    .collect::<Vec<_>>(),
            );
        }
        _ => panic!("Fatal hata"),
    }
}

// ── Test 19: LVL_006 — elevator pathway has level_id → silent ──────────────────
// Both PLT_A and PLT_B have level_id → no LVL_006.

#[test]
fn lvl_006_silent_when_elevator_stops_have_level_id() {
    const STOPS: &[u8] =
        b"stop_id,stop_name,stop_lat,stop_lon,location_type,parent_station,level_id\n\
          S1,Stop1,41.0,29.0,,,\n\
          S2,Stop2,41.1,29.1,,,\n\
          STA1,Station1,41.2,29.2,1,,\n\
          PLT_A,PlatformA,41.2,29.2,0,STA1,L0\n\
          PLT_B,PlatformB,41.2,29.25,0,STA1,L1\n";
    const PATHWAYS: &[u8] = b"pathway_id,from_stop_id,to_stop_id,pathway_mode,is_bidirectional\n\
          PW1,PLT_A,PLT_B,5,1\n";

    let mut files = base_files();
    files[1] = ("stops.txt", STOPS);
    files.push(("pathways.txt", PATHWAYS));

    match run(&files) {
        ValidateResult::Ok(vr) => {
            assert!(
                !vr.notices.iter().any(|n| n.rule_id == "LVL_006"),
                "LVL_006 üretilmemeli (level_id dolu). Notices: {:?}",
                vr.notices
                    .iter()
                    .map(|n| n.rule_id.as_str())
                    .collect::<Vec<_>>(),
            );
        }
        _ => panic!("ValidateResult::Ok beklendi"),
    }
}

// ── Test 20: XFL_019 — both routes.network_id and route_networks.txt → fires ──
// routes.txt has network_id AND route_networks.txt exists → XFL_019.

#[test]
fn xfl_019_fires_when_network_defined_in_both_files() {
    const ROUTES_WITH_NETWORK: &[u8] =
        b"route_id,agency_id,route_short_name,route_long_name,route_type,network_id\n\
          R1,A1,1,Route One,3,NET1\n";
    const ROUTE_NETWORKS: &[u8] = b"network_id,route_id\n\
          NET1,R1\n";

    let mut files = base_files();
    files[2] = ("routes.txt", ROUTES_WITH_NETWORK);
    files.push(("route_networks.txt", ROUTE_NETWORKS));

    match run(&files) {
        ValidateResult::Ok(vr) => {
            assert!(
                vr.notices.iter().any(|n| n.rule_id == "XFL_019"),
                "XFL_019 olmalı (çift ağ tanımı). Mevcut: {:?}",
                vr.notices
                    .iter()
                    .map(|n| n.rule_id.as_str())
                    .collect::<Vec<_>>(),
            );
        }
        _ => panic!("ValidateResult::Ok beklendi"),
    }
}

// ── Test 21: XFL_019 — only route_networks.txt exists → silent ─────────────────
// routes.txt has no network_id, while route_networks.txt exists → no XFL_019.

#[test]
fn xfl_019_silent_when_only_route_networks_file() {
    const ROUTE_NETWORKS: &[u8] = b"network_id,route_id\n\
          NET1,R1\n";

    let mut files = base_files();
    files.push(("route_networks.txt", ROUTE_NETWORKS));

    match run(&files) {
        ValidateResult::Ok(vr) => {
            assert!(
                !vr.notices.iter().any(|n| n.rule_id == "XFL_019"),
                "XFL_019 üretilmemeli (sadece route_networks.txt). Notices: {:?}",
                vr.notices
                    .iter()
                    .map(|n| n.rule_id.as_str())
                    .collect::<Vec<_>>(),
            );
        }
        _ => panic!("ValidateResult::Ok beklendi"),
    }
}

#[test]
fn path_traversal_entries_rejected_and_root_file_intact() {
    // Traversal/nested entry names inside the ZIP must not replace root files.
    let evil: &[u8] =
        b"agency_id,agency_name,agency_url,agency_timezone\n9,EVIL_TRAVERSAL,http://evil.com,UTC\n";
    let mut files = base_files();
    files.extend([
        ("../agency.txt", evil),
        ("nested/agency.txt", evil),
        ("nested\\agency.txt", evil),
        ("/agency.txt", evil),
        ("C:\\agency.txt", evil),
    ]);

    let k1 = parse(&make_zip(&files), &ValidatorConfig::default())
        .expect("gecerli kok dosyalar mevcut oldugunda Ok donmeli");

    assert_eq!(
        k1.files.len(),
        6,
        "yalnizca kok dosyalar islenmeli; traversal girisleri degil",
    );
    for name in [
        "../agency.txt",
        "nested/agency.txt",
        "nested\\agency.txt",
        "/agency.txt",
        "C:\\agency.txt",
    ] {
        assert!(
            !k1.files.contains_key(name),
            "traversal girisi files'a girmemeli: {name}",
        );
    }

    let agency = k1
        .files
        .get("agency.txt")
        .expect("kok agency.txt islenmeli");
    let joined = agency
        .rows
        .iter()
        .flatten()
        .map(|s| s.as_str())
        .collect::<Vec<_>>()
        .join(",");
    assert!(
        joined.contains("Test"),
        "kok agency.txt icerigi korunmali: {joined}"
    );
    assert!(
        !joined.contains("EVIL_TRAVERSAL"),
        "poison icerik canonical dosyaya sizmamali: {joined}",
    );

    let arc024 = k1.notices.iter().filter(|n| n.rule_id == "ARC_024").count();
    assert_eq!(
        arc024, 5,
        "5 traversal .txt girisi icin ARC_024 beklenir, bulunan: {arc024}",
    );
}

#[test]
fn nested_zip_is_reported_in_subdirectories_too() {
    // Korpus 13'te olculdu: bes feed (tdg-83019 · tld-768_1 · mdb-3340 · mdb-657 · tld-752)
    // carries its entire GTFS in a nested ZIP in a SUBDIRECTORY. Previously ARC_023 only saw the root
    // dizini goruyordu; o feed'lerde tek cikti "zorunlu dosya eksik" oluyor, SEBEBI hicbir
    // and the reason was not visible in any finding. Location does not change the rule's verdict.
    let inner: &[u8] =
        b"PK\x05\x06\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00";
    let mut files = base_files();
    files.extend([("root.zip", inner), ("GTFS_20250111/gtfs_data.zip", inner)]);

    let k1 =
        parse(&make_zip(&files), &ValidatorConfig::default()).expect("kok GTFS dosyalari mevcut");

    let found: Vec<&str> = k1
        .notices
        .iter()
        .filter(|n| n.rule_id == "ARC_023")
        .filter_map(|n| n.entity_id.as_deref())
        .collect();
    assert_eq!(
        found.len(),
        2,
        "kok VE alt dizindeki nested ZIP icin ARC_023 beklenir, bulunan: {found:?}",
    );
    assert!(
        found
            .iter()
            .any(|f| f.contains("GTFS_20250111/gtfs_data.zip")),
        "alt dizindeki nested ZIP tam yoluyla raporlanmali: {found:?}",
    );
}

#[test]
fn gtfs_named_file_with_wrong_extension_is_reported_in_subdirectories() {
    // Corpus 13: mdb-3434 and mdb-3135 carry their entire GTFS in a subdirectory with a `.csv` extension.
    // Previously the only output was ARC_004 ("required file missing"), while the files' presence THERE
    // was not visible in any finding. OTHER files in the subdirectory must remain silent — reporting a repo's
    // zip'inin LICENSE/README'sini raporlamak gurultudur.
    let blob: &[u8] = b"x\n";
    let mut files = base_files();
    files.extend([
        ("sub/agency.csv", blob),
        ("sub/stop_times.csv", blob),
        ("sub/LICENSE", blob),
        ("sub/README.md", blob),
        ("sub/lib/main.dart", blob),
    ]);

    let k1 =
        parse(&make_zip(&files), &ValidatorConfig::default()).expect("kok GTFS dosyalari mevcut");

    let found: Vec<&str> = k1
        .notices
        .iter()
        .filter(|n| n.rule_id == "ARC_007")
        .filter_map(|n| n.entity_id.as_deref())
        .collect();
    assert!(
        found.contains(&"sub/agency.csv") && found.contains(&"sub/stop_times.csv"),
        "alt dizindeki GTFS-adli yanlis uzantili dosyalar ARC_007 almali: {found:?}",
    );
    for quiet in ["sub/LICENSE", "sub/README.md", "sub/lib/main.dart"] {
        assert!(
            !found.contains(&quiet),
            "GTFS adi tasimayan alt dizin dosyasi sessiz kalmali: {quiet} — {found:?}",
        );
    }
}

#[test]
fn wrapped_feed_streams_stop_times_instead_of_reading_it_as_empty() {
    // 13th corpus run: K1 strips the root prefix from the wrapped feed and processes the file as
    // `stop_times.txt`, but K2 reopens large files from the ZIP as streams and
    // `by_name("stop_times.txt")` found NOTHING in the wrapped archive. The file was silently
    // treated as empty, so in all 44 of the 44 feeds that triggered ARC_024, not a single STM_*
    // rule fired; moreover every stop appeared to have "no trip passing through it", producing
    // an STP_020 FP (678 findings in mdb-3459).
    let wrapped: Vec<(String, &[u8])> = base_files()
        .into_iter()
        .map(|(name, body)| (format!("GTFS/{name}"), body))
        .collect();
    let files: Vec<(&str, &[u8])> = wrapped.iter().map(|(n, b)| (n.as_str(), *b)).collect();

    let k1 = parse(&make_zip(&files), &ValidatorConfig::default()).expect("sarili feed acilmali");

    let st = k1
        .files
        .get("stop_times.txt")
        .expect("kok oneki dusulmus adla islenmeli");
    assert_eq!(
        st.zip_entry_name.as_deref(),
        Some("GTFS/stop_times.txt"),
        "K2'nin ZIP'i yeniden acabilmesi icin GERCEK girdi adi tasinmali",
    );

    let ValidateResult::Ok(vr) = run(&files) else {
        panic!("sarili feed fatal olmamali")
    };
    // If stop_times was actually read, the stops are USED; if it was not, STP_020
    // ("stop with no trip passing through it") would fire for every stop.
    let stp020 = vr.notices.iter().filter(|n| n.rule_id == "STP_020").count();
    assert_eq!(
        stp020, 0,
        "sarili feed'de stop_times okunmali; STP_020 atesliyorsa stream yolu yine kor",
    );
    // Treating the file as empty also incorrectly triggered ARC_035 (Critical·Spec = publication blocker).
    assert!(
        !vr.notices.iter().any(|n| n.rule_id == "ARC_035"),
        "dolu bir stop_times.txt ARC_035 uretmemeli",
    );
}

#[test]
fn empty_required_file_is_spec_critical_while_empty_optional_stays_informational() {
    // 13th corpus run: the "does the required file exist?" test checked only the file's PRESENCE.
    // 0 baytlik bir stop_times.txt "mevcut" sayilip yayin kapisindan geciyordu — en az
    // In 32 feeds a required file was empty, and five (mdb-1817 · mdb-342 · mdb-3181 · mdb-3459 ·
    // mdb-1819) received a PUBLICATION SCORE of 100.0. Deleting the file produced ARC_004, while
    // emptying it went unpenalized.
    let empty_stops: &[u8] = b"stop_id,stop_name,stop_lat,stop_lon\n";
    // NOTE: the optional side was built with `frequencies.txt`. A header-only `shapes.txt`
    // bugun HIC ARC_009 uretmiyor (k2/shapes.rs kolu `raw_text.is_some()` istiyor, kucuk
    // does not enter the shapes stream path) — a separate, smaller gap outside this test's scope.
    let empty_freq: &[u8] = b"trip_id,start_time,end_time,headway_secs\n";
    let mut files = base_files();
    files.retain(|(n, _)| *n != "stops.txt");
    files.push(("stops.txt", empty_stops));
    files.push(("frequencies.txt", empty_freq));

    let ValidateResult::Ok(vr) = run(&files) else {
        panic!("fatal olmamali")
    };

    let arc035: Vec<&str> = vr
        .notices
        .iter()
        .filter(|n| n.rule_id == "ARC_035")
        .filter_map(|n| n.entity_id.as_deref())
        .collect();
    assert_eq!(
        arc035,
        vec!["stops.txt"],
        "bos ZORUNLU dosya ARC_035 almali: {arc035:?}"
    );

    let arc009: Vec<&str> = vr
        .notices
        .iter()
        .filter(|n| n.rule_id == "ARC_009")
        .filter_map(|n| n.entity_id.as_deref())
        .collect();
    assert!(
        arc009.contains(&"frequencies.txt") && !arc009.contains(&"stops.txt"),
        "bos OPSIYONEL dosya ARC_009'da kalmali, zorunlu olan ARC_035'e gecmeli: {arc009:?}",
    );
}

#[test]
fn header_only_stream_files_are_not_silently_treated_as_present() {
    // For the four files in the `is_zip_stream` list (stop_times · shapes · trips ·
    // calendar_dates), K1 reads ONLY the header and K2 streams the body. K1's own
    // empty-file check requires `!stream_mode`, so it does not inspect these four; the
    // corresponding check must exist in K2. Measured on 2026-08-31: only `stop_times.txt`
    // dogru davraniyordu.
    //   · shapes.txt  → the K2 branch required `raw_text.is_some()`, but for is_zip_stream
    //                   files raw_text is ALWAYS None → the branch was DEAD CODE.
    //   · trips.txt   → no corresponding branch had been written (and trips is REQUIRED → ARC_035).
    // Note: the 0-byte case was already caught by K1's `records.is_empty()` branch;
    // gorunmeyen tam olarak "baslik var, veri yok" araligiydi.
    let mut files = base_files();
    files.push((
        "shapes.txt",
        &b"shape_id,shape_pt_lat,shape_pt_lon,shape_pt_sequence\n"[..],
    ));
    let ValidateResult::Ok(vr) = run(&files) else {
        panic!("fatal olmamali")
    };
    assert!(
        vr.notices
            .iter()
            .any(|n| n.rule_id == "ARC_009" && n.file.as_deref() == Some("shapes.txt")),
        "baslik-only shapes.txt ARC_009 uretmeli (opsiyonel dosya → Bilgi)",
    );

    let mut files = base_files();
    files.retain(|(n, _)| *n != "trips.txt");
    files.push(("trips.txt", &b"route_id,service_id,trip_id\n"[..]));
    let ValidateResult::Ok(vr) = run(&files) else {
        panic!("fatal olmamali")
    };
    assert!(
        vr.notices
            .iter()
            .any(|n| n.rule_id == "ARC_035" && n.file.as_deref() == Some("trips.txt")),
        "baslik-only trips.txt ARC_035 uretmeli (ZORUNLU dosya → Kritik·Spec)",
    );
}

#[test]
fn empty_calendar_dates_stays_suppressed_only_while_calendar_txt_carries_the_service() {
    // Conditional pair tolerance: an empty calendar_dates.txt is a false positive when calendar.txt
    // defines service. But suppression must not apply when calendar.txt is ABSENT — otherwise
    // "hic servis tanimi yok" hali sessizce gecerdi.
    let empty_cd: &[u8] = b"service_id,date,exception_type\n";

    let mut with_calendar = base_files();
    with_calendar.push(("calendar_dates.txt", empty_cd));
    let ValidateResult::Ok(vr) = run(&with_calendar) else {
        panic!("fatal olmamali")
    };
    assert!(
        !vr.notices
            .iter()
            .any(|n| n.rule_id == "ARC_009" && n.file.as_deref() == Some("calendar_dates.txt")),
        "calendar.txt servisi tasirken bos calendar_dates.txt bastirilmali",
    );

    let mut without_calendar = base_files();
    without_calendar.retain(|(n, _)| *n != "calendar.txt");
    without_calendar.push(("calendar_dates.txt", empty_cd));
    let ValidateResult::Ok(vr) = run(&without_calendar) else {
        panic!("fatal olmamali")
    };
    assert!(
        vr.notices
            .iter()
            .any(|n| n.rule_id == "ARC_009" && n.file.as_deref() == Some("calendar_dates.txt")),
        "calendar.txt yokken bos calendar_dates.txt SUSMAMALI",
    );
}

#[test]
fn empty_required_file_alone_blocks_publication() {
    // This is the ONLY REAL justification for ARC_035, and the corpus had no pure example:
    // agency/stops/routes/calendar are FULL, while only trips.txt and stop_times.txt are
    // header-only. Because those two files took the `is_zip_stream` path, the old code did not
    // even emit ARC_009 — the feed was marked `publishable` without producing a single finding.
    // sayiliyordu. (55 feed'lik dogrulama orneklemindeki 10 ARC_035 feed'inin
    // all already had an STP_018 or ARC_025 blocker, so this rule did not turn the gate there
    // cevirmiyor; ceviren yer tam olarak burasi.)
    let files: Vec<(&str, &[u8])> = vec![
        ("agency.txt", AGENCY),
        ("stops.txt", STOPS),
        ("routes.txt", ROUTES),
        ("calendar.txt", CALENDAR),
        ("trips.txt", &b"route_id,service_id,trip_id\n"[..]),
        (
            "stop_times.txt",
            &b"trip_id,arrival_time,departure_time,stop_id,stop_sequence\n"[..],
        ),
    ];
    let ValidateResult::Ok(vr) = run(&files) else {
        panic!("fatal olmamali")
    };

    let blockers: Vec<&str> = vr
        .reports
        .r1
        .blocker_notice_ids
        .iter()
        .filter_map(|id| vr.notices.iter().find(|n| &n.id == id))
        .map(|n| n.rule_id.as_str())
        .collect();
    assert!(
        !vr.reports.r1.publishable,
        "bos zorunlu dosyali feed yayina uygun sayilmamali",
    );
    assert!(
        blockers.iter().all(|r| *r == "ARC_035"),
        "bu vakada TEK engel ARC_035 olmali (baska engel varsa test kendi iddiasini olcmuyor): {blockers:?}",
    );
    assert_eq!(
        blockers.len(),
        2,
        "trips.txt + stop_times.txt icin iki engel: {blockers:?}"
    );
}

// ── ARC_025: required column absent from the header (header-level) ───────────

#[test]
fn missing_required_column_produces_arc_025() {
    // The required 'agency_name' column is absent from agency.txt's header → ARC_025 (once per file).
    let bad_agency: &[u8] = b"agency_id,agency_url,agency_timezone\n1,http://test.example,UTC\n";
    let mut files = base_files();
    files[0] = ("agency.txt", bad_agency);
    let k1 = parse(&make_zip(&files), &ValidatorConfig::default())
        .expect("gecerli kok dosyalar Ok donmeli");

    let arc025_name = k1
        .notices
        .iter()
        .filter(|n| n.rule_id == "ARC_025" && n.field.as_deref() == Some("agency_name"))
        .count();
    assert_eq!(
        arc025_name, 1,
        "agency_name sutunu basligta yok → 1 ARC_025, bulunan: {arc025_name}",
    );

    // ARC_025 must NOT fire for an existing column (agency_url) — that is a value-empty issue (K2 group rule).
    assert!(
        !k1.notices
            .iter()
            .any(|n| n.rule_id == "ARC_025" && n.field.as_deref() == Some("agency_url")),
        "mevcut sutun icin ARC_025 cikmamali",
    );
}

#[test]
fn complete_headers_produce_no_arc_025() {
    // base_files contains all required columns → no ARC_025 should be emitted.
    let k1 = parse(&make_zip(&base_files()), &ValidatorConfig::default())
        .expect("gecerli kok dosyalar Ok donmeli");
    let arc025 = k1.notices.iter().filter(|n| n.rule_id == "ARC_025").count();
    assert_eq!(
        arc025, 0,
        "tam basliklarda ARC_025 cikmamali, bulunan: {arc025}"
    );
}

// ── XFL_027: route-level contactless EMV precedence information ────────────────
// Shared setup: 1 route (R1, 2 stops), contactless fare_media(type=3) + fare_product.
// routes.txt (cemv/network) + fare_leg_rules + networks vary by scenario.
fn cemv_base() -> Vec<(&'static str, &'static [u8])> {
    vec![
        ("agency.txt", b"agency_id,agency_name,agency_url,agency_timezone\nA1,T,http://t.co,UTC\n" as &[u8]),
        ("stops.txt", b"stop_id,stop_name,stop_lat,stop_lon\nS1,S1,40.0,-73.0\nS2,S2,40.1,-73.1\n"),
        ("trips.txt", b"route_id,service_id,trip_id\nR1,SVC,T1\n"),
        ("stop_times.txt", b"trip_id,arrival_time,departure_time,stop_id,stop_sequence\nT1,08:00:00,08:00:00,S1,1\nT1,08:10:00,08:10:00,S2,2\n"),
        ("calendar.txt", b"service_id,monday,tuesday,wednesday,thursday,friday,saturday,sunday,start_date,end_date\nSVC,1,1,1,1,1,0,0,20250101,20261231\n"),
        ("fare_media.txt", b"fare_media_id,fare_media_name,fare_media_type\nemv,Contactless,3\n"),
        ("fare_products.txt", b"fare_product_id,fare_product_name,amount,currency,fare_media_id\np_emv,EMV,2.00,USD,emv\n"),
    ]
}

#[test]
fn xfl027_route_cemv2_with_global_contactless() {
    // R1 cemv_support=2 (unsupported) but the leg rule is GLOBAL (no network/area) → every route is covered → conflict.
    let mut files = cemv_base();
    files.push((
        "routes.txt",
        b"route_id,agency_id,route_short_name,route_type,cemv_support\nR1,A1,1,3,2\n",
    ));
    files.push((
        "fare_leg_rules.txt",
        b"leg_group_id,fare_product_id\nlg1,p_emv\n",
    ));
    match run(&files) {
        ValidateResult::Ok(vr) => assert!(
            vr.notices.iter().any(|n| n.rule_id == "XFL_027"),
            "XFL_027 bekleniyor (cemv=2 + global contactless). Cikan: {:?}",
            vr.notices
                .iter()
                .map(|n| n.rule_id.as_str())
                .collect::<Vec<_>>()
        ),
        other => panic!("Ok bekleniyordu: {other:?}"),
    }
}

#[test]
fn xfl026_route_cemv1_network_mismatch_is_not_reported() {
    // cEMV=1 alone does not require evidence of Fares v2; XFL_026 is no longer produced.
    let mut files = cemv_base();
    files.push((
        "routes.txt",
        b"route_id,agency_id,route_short_name,route_type,network_id,cemv_support\nR1,A1,1,3,N1,1\n",
    ));
    files.push(("networks.txt", b"network_id,network_name\nN1,N1\nN2,N2\n"));
    files.push((
        "fare_leg_rules.txt",
        b"leg_group_id,fare_product_id,network_id\nlg1,p_emv,N2\n",
    ));
    match run(&files) {
        ValidateResult::Ok(vr) => assert!(
            !vr.notices.iter().any(|n| n.rule_id == "XFL_026"),
            "XFL_026 üretilmemeli. Cikan: {:?}",
            vr.notices
                .iter()
                .map(|n| n.rule_id.as_str())
                .collect::<Vec<_>>()
        ),
        other => panic!("Ok bekleniyordu: {other:?}"),
    }
}

#[test]
fn xfl_019_fires_when_networks_file_is_present() {
    let mut files = base_files();
    files[2] = (
        "routes.txt",
        b"route_id,agency_id,route_short_name,route_long_name,route_type,network_id\nR1,A1,1,Route One,3,N1\n",
    );
    files.push(("networks.txt", b"network_id,network_name\nN1,Network\n"));
    match run(&files) {
        ValidateResult::Ok(vr) => assert!(has(&vr, "XFL_019")),
        other => panic!("ValidateResult::Ok beklendi, alınan: {other:?}"),
    }
}

#[test]
fn xfl026_silent_when_route_coverage_unresolvable() {
    // R1 cemv=1, network_id EMPTY; contactless leg rule is network-based → route coverage cannot be resolved → FP guard stays silent.
    let mut files = cemv_base();
    files.push((
        "routes.txt",
        b"route_id,agency_id,route_short_name,route_type,cemv_support\nR1,A1,1,3,1\n",
    ));
    files.push(("networks.txt", b"network_id,network_name\nN1,N1\n"));
    files.push((
        "fare_leg_rules.txt",
        b"leg_group_id,fare_product_id,network_id\nlg1,p_emv,N1\n",
    ));
    match run(&files) {
        ValidateResult::Ok(vr) => {
            let ids: Vec<&str> = vr.notices.iter().map(|n| n.rule_id.as_str()).collect();
            assert!(
                !ids.contains(&"XFL_026") && !ids.contains(&"XFL_027"),
                "FP guard susmali (network/area cozulemez). Cikan: {ids:?}"
            );
        }
        other => panic!("Ok bekleniyordu: {other:?}"),
    }
}

// ── ARC_029: decompression guard (zip bomb) end to end ─────────────────────────
// The #46 guard is unit-tested at GuardedReader level (decompress_guard.rs), but no test
// verified wiring it through validate_bytes with DEFAULT_DECOMPRESSION_LIMITS. These tests
// prove ARC_029's two distinct read_fatal call sites: the zip-stream path
// (stop_times/trips/calendar_dates) and the read_to_end path (other .txt files).
//
// Trigger condition (DEFAULT): entry_decompressed > ratio_floor (16 MiB) AND
// entry_decompressed > 80 × entry_compressed. Zero bytes compress at roughly 1000:1.
const BOMB_DECOMPRESSED: usize = 24 * 1024 * 1024; // above the 16 MiB ratio_floor

fn bomb_payload() -> Vec<u8> {
    vec![b'\n'; BOMB_DECOMPRESSED]
}

fn assert_decompression_limit(result: ValidateResult, ctx: &str) {
    match result {
        ValidateResult::Fatal(e) => assert_eq!(
            e.code,
            FatalCode::DecompressionLimit,
            "{ctx}: beklenen DecompressionLimit, alınan {:?} ({})",
            e.code,
            e.message,
        ),
        other => panic!("{ctx}: Fatal(DecompressionLimit) beklendi, alınan: {other:?}"),
    }
}

#[test]
fn arc029_zip_bomb_in_streamed_file_returns_fatal_decompression_limit() {
    let bomb = bomb_payload();
    let mut files = base_files();
    // stop_times.txt = #38 zip-stream path (k1_parse.rs read_fatal call site 1)
    files.retain(|(n, _)| *n != "stop_times.txt");
    files.push(("stop_times.txt", &bomb));
    let zip = make_zip(&files);
    assert_decompression_limit(
        validate_bytes(&zip, &ValidatorConfig::default(), TODAY),
        "stop_times.txt (zip-stream yolu)",
    );
}

#[test]
fn arc029_zip_bomb_in_buffered_file_returns_fatal_decompression_limit() {
    let bomb = bomb_payload();
    let mut files = base_files();
    // shapes.txt = read_to_end path (k1_parse.rs read_fatal call site 2)
    files.push(("shapes.txt", &bomb));
    let zip = make_zip(&files);
    assert_decompression_limit(
        validate_bytes(&zip, &ValidatorConfig::default(), TODAY),
        "shapes.txt (read_to_end yolu)",
    );
}

#[test]
fn arc029_legitimate_small_high_ratio_file_does_not_trip_guard() {
    // FP guard: a legitimate highly compressed file BELOW ratio_floor is not a bomb.
    // (The "legitimate-high-ratio" risk identified by an external participant when #46 closed.)
    let small_high_ratio = vec![b'\n'; 1024 * 1024]; // 1 MiB << 16 MiB floor, oran ~1000:1
    let mut files = base_files();
    files.push(("shapes.txt", &small_high_ratio));
    let zip = make_zip(&files);
    match validate_bytes(&zip, &ValidatorConfig::default(), TODAY) {
        ValidateResult::Fatal(e) => panic!(
            "Meşru yüksek-oranlı küçük dosya guard'ı tetiklememeli, alınan: {:?} ({})",
            e.code, e.message,
        ),
        ValidateResult::Ok(_) => {}
    }
}

// ── Determinism gate ──────────────────────────────────────────────────────────
// Same feed, same `today` → EXACTLY the same output. This guarantee did NOT EXIST until
// 2026-07-27: notice contents and IDs changed between runs because emission order depended
// on HashMap iteration, and dedup selected its keep-first representative from that order
// (829 records varied in VBB). Golden comparisons and A/B measurements were therefore unreliable.
#[test]
fn same_feed_twice_produces_identical_output() {
    // Contains patterns that trigger nondeterminism: a REPEATED stop in one trip (OPR_007),
    // multiple shapes (SHP/GEO families), and multiple route+service combinations (OPR_001/003/024).
    let shapes = b"shape_id,shape_pt_lat,shape_pt_lon,shape_pt_sequence\n\
        S1,41.0,29.0,1\nS1,41.0,29.0,2\nS1,41.5,29.5,3\n\
        S2,40.0,28.0,1\nS2,40.0,28.0,2\nS2,40.9,28.9,3\n\
        S3,39.0,27.0,1\nS3,39.0,27.0,2\nS3,39.8,27.8,3\n" as &[u8];
    let routes =
        b"route_id,agency_id,route_short_name,route_type\nR1,1,101,3\nR2,1,102,3\nR3,1,103,3\n"
            as &[u8];
    let trips = b"route_id,service_id,trip_id,shape_id\n\
        R1,SVC1,T1,S1\nR2,SVC1,T2,S2\nR3,SVC1,T3,S3\n" as &[u8];
    // T1: stop S1 appears twice, with another stop between → OPR_007 candidate (multiple repeats possible)
    let stop_times = b"trip_id,arrival_time,departure_time,stop_id,stop_sequence\n\
        T1,08:00:00,08:00:00,S1,1\nT1,08:05:00,08:05:00,S2,2\nT1,08:10:00,08:10:00,S1,3\n\
        T2,09:00:00,09:00:00,S1,1\nT2,09:30:00,09:30:00,S2,2\n\
        T3,10:00:00,10:00:00,S2,1\nT3,10:30:00,10:30:00,S1,2\n" as &[u8];

    let mut files = base_files();
    files.retain(|(n, _)| *n != "trips.txt" && *n != "stop_times.txt" && *n != "routes.txt");
    files.push(("routes.txt", routes));
    files.push(("trips.txt", trips));
    files.push(("stop_times.txt", stop_times));
    files.push(("shapes.txt", shapes));
    let zip = make_zip(&files);

    // Scope: FULLY SERIALIZED JSON — everything, not only notice fields, including `Notice.details`,
    // `NameIndex`, and `capped_totals`. This was NOT possible until 2026-07-31: those fields were
    // `HashMap`s and serde wrote keys in a different order on each run, so bytes differed despite
    // identical content. All were changed to `BTreeMap`; this gate protects that gain.
    // Weakening it by narrowing the comparison to a field subset would silently lose the guarantee.
    let render = || match validate_bytes(&zip, &ValidatorConfig::default(), TODAY) {
        ValidateResult::Ok(vr) => {
            serde_json::to_string(&vr).expect("ValidationResult serialize edilebilmeli")
        }
        ValidateResult::Fatal(e) => panic!("beklenmeyen fatal: {:?} {}", e.code, e.message),
    };

    // Repetition within the SAME PROCESS: because each HashMap instance gets its own seed in Rust,
    // this is a stricter gate than running in separate processes.
    let first = render();
    for run in 2..=4 {
        let cur = render();
        if cur != first {
            // Show the first divergence: printing the full JSON would be unreadable.
            let at = first
                .bytes()
                .zip(cur.bytes())
                .position(|(a, b)| a != b)
                .unwrap_or(first.len().min(cur.len()));
            let lo = at.saturating_sub(80);
            panic!(
                "çıktı deterministik değil ({run}. koşu, {at}. bayt):\n  1. koşu: …{}…\n  {run}. koşu: …{}…",
                &first[lo..(at + 80).min(first.len())],
                &cur[lo..(at + 80).min(cur.len())],
            );
        }
    }
}

// ── STM_015/016: Flex guard and first/last stop scope ──────────────────────────
// The Spec says "Forbidden when start_pickup_drop_off_window or
// end_pickup_drop_off_window are defined" for arrival_time/departure_time. A missing time
// field on a Flex trip is NOT a violation; it is expected. Without the guard, these two
// rules would produce a false positive for every Flex trip.

#[test]
fn stm015_016_silent_for_flex_pickup_drop_off_window() {
    static FLEX_STOP_TIMES: &[u8] =
        b"trip_id,stop_id,stop_sequence,start_pickup_drop_off_window,end_pickup_drop_off_window\n\
          T1,S1,1,08:00:00,12:00:00\nT1,S2,2,08:00:00,12:00:00\n";
    let mut files = base_files();
    files[4] = ("stop_times.txt", FLEX_STOP_TIMES);

    match run(&files) {
        ValidateResult::Ok(vr) => {
            let hits: Vec<&str> = vr
                .notices
                .iter()
                .map(|n| n.rule_id.as_str())
                .filter(|id| *id == "STM_015" || *id == "STM_016")
                .collect();
            assert!(
                hits.is_empty(),
                "Flex pencereli seferde STM_015/016 üretilmemeli, çıkanlar: {hits:?}",
            );
        }
        other => panic!("ValidateResult::Ok beklendi, gelen: {other:?}"),
    }
}

#[test]
fn stm015_016_cover_arrival_only_departure_left_to_stm034() {
    // The first stop has arrival but no departure → NOT STM_015 (the Spec does not require
    // departure); STM_034 ("only one defined") already covers this case.
    // The last stop has departure but no arrival → STM_016 (the Spec requires arrival).
    static EDGE_STOP_TIMES: &[u8] = b"trip_id,arrival_time,departure_time,stop_id,stop_sequence\n\
          T1,08:00:00,,S1,1\nT1,,08:10:00,S2,2\n";
    let mut files = base_files();
    files[4] = ("stop_times.txt", EDGE_STOP_TIMES);

    match run(&files) {
        ValidateResult::Ok(vr) => {
            let ids: Vec<&str> = vr.notices.iter().map(|n| n.rule_id.as_str()).collect();
            assert!(
                !ids.contains(&"STM_015"),
                "ilk durakta arrival var → STM_015 olmamalı: {ids:?}"
            );
            assert!(
                ids.contains(&"STM_016"),
                "son durakta arrival yok → STM_016 bekleniyor: {ids:?}"
            );
            assert!(
                ids.contains(&"STM_034"),
                "departure tarafını STM_034 karşılamalı: {ids:?}"
            );
        }
        other => panic!("ValidateResult::Ok beklendi, gelen: {other:?}"),
    }
}

#[test]
fn stm015_016_carry_edge_stop_and_neighbour_time() {
    // The planner cards show each edge stop and its two adjacent actual rows. The existing
    // nearest-timed-neighbor keys remain unchanged for compatibility.
    static EDGE_STOP_TIMES: &[u8] = b"trip_id,arrival_time,departure_time,stop_id,stop_sequence\n\
          T1,,08:00:00,S1,1\nT1,08:10:00,08:10:00,S2,2\nT1,,08:20:00,S3,3\n";
    let mut files = base_files();
    files[4] = ("stop_times.txt", EDGE_STOP_TIMES);

    match run(&files) {
        ValidateResult::Ok(vr) => {
            assert_eq!(
                vr.notices.iter().filter(|n| n.rule_id == "STM_015").count(),
                1,
                "details enrichment must not change the STM_015 notice count"
            );
            assert_eq!(
                vr.notices.iter().filter(|n| n.rule_id == "STM_016").count(),
                1,
                "details enrichment must not change the STM_016 notice count"
            );
            let detail = |rule: &str, key: &str| {
                vr.notices
                    .iter()
                    .find(|n| n.rule_id == rule)
                    .and_then(|n| n.details.as_ref())
                    .and_then(|d| d.get(key).cloned())
            };
            assert_eq!(detail("STM_015", "stop_id").as_deref(), Some("S1"));
            assert_eq!(detail("STM_015", "departure").as_deref(), Some("08:00:00"));
            assert_eq!(detail("STM_015", "next_stop_id").as_deref(), Some("S2"));
            assert_eq!(detail("STM_015", "next_time").as_deref(), Some("08:10:00"));
            assert_eq!(detail("STM_015", "stop_sequence").as_deref(), Some("1"));
            assert_eq!(
                detail("STM_015", "following_stop_id").as_deref(),
                Some("S2")
            );
            assert_eq!(
                detail("STM_015", "following_stop_sequence").as_deref(),
                Some("2")
            );
            assert_eq!(
                detail("STM_015", "following_arrival").as_deref(),
                Some("08:10:00")
            );
            assert_eq!(
                detail("STM_015", "following_departure").as_deref(),
                Some("08:10:00")
            );
            assert_eq!(
                detail("STM_015", "following2_stop_id").as_deref(),
                Some("S3")
            );
            assert_eq!(
                detail("STM_015", "following2_stop_sequence").as_deref(),
                Some("3")
            );
            assert_eq!(
                detail("STM_015", "following2_departure").as_deref(),
                Some("08:20:00")
            );
            assert_eq!(detail("STM_016", "stop_id").as_deref(), Some("S3"));
            assert_eq!(detail("STM_016", "prev_stop_id").as_deref(), Some("S2"));
            assert_eq!(detail("STM_016", "prev_time").as_deref(), Some("08:10:00"));
            assert_eq!(detail("STM_016", "stop_sequence").as_deref(), Some("3"));
            assert_eq!(
                detail("STM_016", "preceding_stop_id").as_deref(),
                Some("S2")
            );
            assert_eq!(
                detail("STM_016", "preceding_stop_sequence").as_deref(),
                Some("2")
            );
            assert_eq!(
                detail("STM_016", "preceding_arrival").as_deref(),
                Some("08:10:00")
            );
            assert_eq!(
                detail("STM_016", "preceding_departure").as_deref(),
                Some("08:10:00")
            );
            assert_eq!(
                detail("STM_016", "preceding2_stop_id").as_deref(),
                Some("S1")
            );
            assert_eq!(
                detail("STM_016", "preceding2_stop_sequence").as_deref(),
                Some("1")
            );
            assert_eq!(
                detail("STM_016", "preceding2_departure").as_deref(),
                Some("08:00:00")
            );
            assert_eq!(detail("STM_015", "stop_count").as_deref(), Some("3"));
            assert_eq!(detail("STM_015", "timed_stops").as_deref(), Some("3"));
        }
        other => panic!("ValidateResult::Ok beklendi, gelen: {other:?}"),
    }
}

#[test]
fn stm008_carries_previous_and_next_stop_times() {
    // In the backward-time segment, the planner also shows the previous and next stop times.
    static BACKWARDS: &[u8] = b"trip_id,arrival_time,departure_time,stop_id,stop_sequence\n\
          T1,07:50:00,07:50:00,S0,1\nT1,08:00:00,08:00:00,S1,2\nT1,07:55:00,07:55:00,S2,3\nT1,08:20:00,08:20:00,S3,4\n";
    let mut files = base_files();
    files[4] = ("stop_times.txt", BACKWARDS);

    match run(&files) {
        ValidateResult::Ok(vr) => {
            let d = vr
                .notices
                .iter()
                .find(|n| n.rule_id == "STM_008")
                .and_then(|n| n.details.clone())
                .expect("STM_008 ayrıntısı bekleniyor");
            assert_eq!(d.get("prev_stop_id").map(String::as_str), Some("S0"));
            assert_eq!(d.get("prev_time").map(String::as_str), Some("07:50:00"));
            assert_eq!(d.get("next_stop_id").map(String::as_str), Some("S3"));
            assert_eq!(d.get("next_time").map(String::as_str), Some("08:20:00"));
        }
        other => panic!("ValidateResult::Ok beklendi, gelen: {other:?}"),
    }
}

#[test]
fn trip_level_time_rules_carry_the_stops_to_fix() {
    // The planner card identifies the bad location by stop name: STM_028/029 use the trip's
    // endpoint stops, STM_045 uses the stop whose departure exceeds the window, and STM_021
    // uses the two stops at the same location.
    fn details_of(files: &[(&str, &[u8])], rule: &str) -> gtfs_core::NoticeDetails {
        match run(files) {
            ValidateResult::Ok(vr) => vr
                .notices
                .iter()
                .find(|n| n.rule_id == rule)
                .and_then(|n| n.details.clone())
                .unwrap_or_else(|| panic!("{rule} ayrıntısı bekleniyor")),
            other => panic!("ValidateResult::Ok beklendi, gelen: {other:?}"),
        }
    }
    let get = |d: &gtfs_core::NoticeDetails, k: &str| d.get(k).cloned();

    static SHORT: &[u8] = b"trip_id,arrival_time,departure_time,stop_id,stop_sequence\n\
          T1,08:00:00,08:00:00,S1,1\nT1,08:00:30,08:00:30,S2,2\n";
    let mut files = base_files();
    files[4] = ("stop_times.txt", SHORT);
    let d = details_of(&files, "STM_029");
    assert_eq!(get(&d, "first_stop_id").as_deref(), Some("S1"));
    assert_eq!(get(&d, "first_departure").as_deref(), Some("08:00:00"));
    assert_eq!(get(&d, "last_stop_id").as_deref(), Some("S2"));
    assert_eq!(get(&d, "last_arrival").as_deref(), Some("08:00:30"));

    static LONG: &[u8] = b"trip_id,arrival_time,departure_time,stop_id,stop_sequence\n\
          T1,01:00:00,01:00:00,S1,1\nT1,26:00:00,26:00:00,S2,2\n";
    files[4] = ("stop_times.txt", LONG);
    let d = details_of(&files, "STM_028");
    assert_eq!(get(&d, "first_stop_id").as_deref(), Some("S1"));
    assert_eq!(get(&d, "last_arrival").as_deref(), Some("26:00:00"));

    static LATE: &[u8] = b"trip_id,arrival_time,departure_time,stop_id,stop_sequence\n\
          T1,08:00:00,08:00:00,S1,1\nT1,28:00:00,28:00:00,S2,2\n";
    files[4] = ("stop_times.txt", LATE);
    assert_eq!(get(&details_of(&files, "STM_045"), "stop_id").as_deref(), Some("S2"));

    static SAME_PLACE: &[u8] =
        b"stop_id,stop_name,stop_lat,stop_lon\nS1,Stop1,41.0,29.0\nS2,Stop2,41.0,29.0\n";
    let mut files = base_files();
    files[1] = ("stops.txt", SAME_PLACE);
    let d = details_of(&files, "STM_021");
    assert_eq!(get(&d, "stop_a").as_deref(), Some("S1"));
    assert_eq!(get(&d, "stop_b").as_deref(), Some("S2"));

    // With neighbours: the card lists the previous stop, the two stops at the same place and
    // the next stop, each with its position in the trip.
    static SAME_PLACE4: &[u8] = b"stop_id,stop_name,stop_lat,stop_lon\n\
          S0,Stop0,40.99,28.99\nS1,Stop1,41.0,29.0\nS2,Stop2,41.0,29.0\nS3,Stop3,41.01,29.01\n";
    static FOUR_STOPS: &[u8] = b"trip_id,arrival_time,departure_time,stop_id,stop_sequence\n\
          T1,07:55:00,07:55:00,S0,4\nT1,08:00:00,08:00:00,S1,5\n\
          T1,08:01:00,08:01:00,S2,6\nT1,08:05:00,08:05:00,S3,7\n";
    files[1] = ("stops.txt", SAME_PLACE4);
    files[4] = ("stop_times.txt", FOUR_STOPS);
    let d = details_of(&files, "STM_021");
    for (key, value) in [
        ("stop_a", "S1"),
        ("stop_b", "S2"),
        ("stop_a_sequence", "5"),
        ("stop_a_departure", "08:00:00"),
        ("stop_b_sequence", "6"),
        ("stop_b_arrival", "08:01:00"),
        ("prev_stop_id", "S0"),
        ("prev_stop_sequence", "4"),
        ("next_stop_id", "S3"),
        ("next_stop_sequence", "7"),
    ] {
        assert_eq!(get(&d, key).as_deref(), Some(value), "{key}");
    }
    assert_eq!(
        get(&d, "stop_a_stop_sequence"),
        None,
        "pair uses the STM_020 key names"
    );
}

#[test]
fn malformed_time_carries_stop_and_neighbour_rows() {
    // Gaziantep (mdb-3401) example: the trip is valid at 09:44, but later stops begin with
    // negative times such as "-15:46:01". The planner should show the bad-time stop and its two neighbors.
    static STOPS3: &[u8] = b"stop_id,stop_name,stop_lat,stop_lon\nS1,Stop1,41.0,29.0\nS2,Stop2,41.01,29.01\nS3,Stop3,41.02,29.02\n";
    static NEGATIVE: &[u8] = b"trip_id,arrival_time,departure_time,stop_id,stop_sequence\n\
          T1,09:44:16,09:44:16,S1,1\nT1,-15:46:01,-15:46:01,S2,2\nT1,-15:47:54,-15:47:54,S3,3\n";
    let mut files = base_files();
    files[1] = ("stops.txt", STOPS3);
    files[4] = ("stop_times.txt", NEGATIVE);
    match run(&files) {
        ValidateResult::Ok(vr) => {
            let d = vr
                .notices
                .iter()
                .find(|n| n.rule_id == "STM_003" && n.observed_value.as_deref() == Some("-15:46:01"))
                .and_then(|n| n.details.clone())
                .expect("STM_003 ayrıntısı bekleniyor");
            let get = |k: &str| d.get(k).map(String::as_str);
            assert_eq!(get("stop_id"), Some("S2"));
            assert_eq!(get("arrival"), Some("-15:46:01"));
            assert_eq!(get("departure"), Some("-15:46:01"));
            assert_eq!(get("prev_stop_id"), Some("S1"));
            assert_eq!(get("prev_arrival"), Some("09:44:16"));
            assert_eq!(get("prev_departure"), Some("09:44:16"));
            assert_eq!(get("next_stop_id"), Some("S3"));
            assert_eq!(get("next_arrival"), Some("-15:47:54"));
            assert_eq!(get("next_departure"), Some("-15:47:54"));
        }
        other => panic!("ValidateResult::Ok beklendi, gelen: {other:?}"),
    }
}

// ── STM_057 KALDIRILDI (2026-08-01, WP-1) ─────────────────────────────────────
// Both tests for the rule used to be here. The Spec sentence is conditional ("travel WITHIN THE SAME
// location group…requires two records"); the rule had turned it into a requirement for two records at
// every location and rejected a normal point-to-point Flex trip such as L1 → L2.
// Replacement gate: spec_conformance.rs — valid Flex feeds PRODUCE NO Spec notice.

// ── stops.txt CONDITIONALLY required (Flex-only feed) ─────────────────────────
// Spec (Schedule Reference, file table): "stops.txt — Conditionally Required —
// Optional if demand-responsive zones are defined in locations.geojson. Required otherwise."
// Until 2026-08-01 stops.txt was unconditionally in REQUIRED_FILES → a valid Flex-only
// feed received ARC_004 Fatal and would not open at all; the user could not see even one finding.

static FLEX_GEOJSON: &[u8] = br#"{"type":"FeatureCollection","features":[
    {"type":"Feature","id":"zone_a","properties":{},"geometry":{"type":"Polygon","coordinates":
    [[[29.0,41.0],[29.1,41.0],[29.1,41.1],[29.0,41.1],[29.0,41.0]]]}}]}"#;
static FLEX_STOP_TIMES: &[u8] =
    b"trip_id,location_id,stop_sequence,start_pickup_drop_off_window,end_pickup_drop_off_window\n\
      T1,zone_a,1,08:00:00,12:00:00\nT1,zone_a,2,08:00:00,12:00:00\n";
static EMPTY_STOPS: &[u8] = b"stop_id,stop_name,stop_lat,stop_lon\n";

#[test]
fn flex_only_feed_without_stops_txt_is_not_fatal() {
    let mut files = base_files();
    files.retain(|(n, _)| *n != "stops.txt" && *n != "stop_times.txt");
    files.push(("stop_times.txt", FLEX_STOP_TIMES));
    files.push(("locations.geojson", FLEX_GEOJSON));

    match validate_bytes(&make_zip(&files), &ValidatorConfig::default(), TODAY) {
        ValidateResult::Fatal(e) => {
            panic!(
            "locations.geojson varken stops.txt eksikliği Fatal OLMAMALI (spec: koşullu zorunlu); \
             alınan: {:?} — {}", e.code, e.message)
        }
        ValidateResult::Ok(vr) => {
            assert!(
                !vr.notices
                    .iter()
                    .any(|n| n.rule_id == "ARC_004"
                        && n.observed_value.as_deref() == Some("stops.txt")),
                "stops.txt için ARC_004 üretilmemeli"
            );
            assert!(!has(&vr, "STP_018"), "geçerli Flex feed duraksız sayılmamalı");
        }
    }
}

#[test]
fn referenced_valid_flex_zone_exempts_header_only_stops() {
    let mut files = base_files();
    files.retain(|(n, _)| *n != "stops.txt" && *n != "stop_times.txt");
    files.push(("stops.txt", EMPTY_STOPS));
    files.push(("stop_times.txt", FLEX_STOP_TIMES));
    files.push(("locations.geojson", FLEX_GEOJSON));

    let ValidateResult::Ok(vr) = run(&files) else {
        panic!("geçerli Flex feed okunmalı");
    };
    assert!(!has(&vr, "ARC_035"), "boş stops.txt Flex bölgesiyle muaf olmalı");
    assert!(!has(&vr, "STP_018"), "STP_018 aynı Flex muafiyetini uygulamalı");
}

#[test]
fn unreferenced_geojson_zone_does_not_exempt_empty_or_missing_stops() {
    static OTHER_ZONE_STOP_TIMES: &[u8] =
        b"trip_id,location_id,stop_sequence,start_pickup_drop_off_window,end_pickup_drop_off_window\n\
          T1,other_zone,1,08:00:00,12:00:00\nT1,other_zone,2,08:00:00,12:00:00\n";

    let mut empty = base_files();
    empty.retain(|(n, _)| *n != "stops.txt" && *n != "stop_times.txt");
    empty.push(("stops.txt", EMPTY_STOPS));
    empty.push(("stop_times.txt", OTHER_ZONE_STOP_TIMES));
    empty.push(("locations.geojson", FLEX_GEOJSON));
    let ValidateResult::Ok(vr) = run(&empty) else {
        panic!("feed okunmalı");
    };
    assert!(has(&vr, "ARC_035"), "kullanılmayan bölge boş stops.txt'i muaf etmemeli");
    assert!(has(&vr, "STP_018"), "kullanılmayan bölge STP_018'i bastırmamalı");

    empty.retain(|(n, _)| *n != "stops.txt");
    let ValidateResult::Ok(vr) = run(&empty) else {
        panic!("eksik stops.txt PARTIAL rapor üretmeli");
    };
    assert!(vr.partial.is_some(), "stops.txt kanıtı yoksa kapsam eksik olmalı");
    assert!(has(&vr, "ARC_004"), "kullanılmayan bölge eksik stops.txt'i muaf etmemeli");
}

#[test]
fn readable_polygon_with_invalid_or_duplicate_feature_does_not_exempt_stops() {
    static MISSING_TYPE: &[u8] = br#"{"type":"FeatureCollection","features":[
        {"id":"zone_a","properties":{},"geometry":{"type":"Polygon","coordinates":
        [[[29.0,41.0],[29.1,41.0],[29.1,41.1],[29.0,41.1],[29.0,41.0]]]}}]}"#;
    static MISSING_PROPERTIES: &[u8] = br#"{"type":"FeatureCollection","features":[
        {"type":"Feature","id":"zone_a","geometry":{"type":"Polygon","coordinates":
        [[[29.0,41.0],[29.1,41.0],[29.1,41.1],[29.0,41.1],[29.0,41.0]]]}}]}"#;
    static DUPLICATE_ID: &[u8] = br#"{"type":"FeatureCollection","features":[
        {"type":"Feature","id":"zone_a","properties":{},"geometry":{"type":"Polygon","coordinates":
        [[[29.0,41.0],[29.1,41.0],[29.1,41.1],[29.0,41.1],[29.0,41.0]]]}},
        {"type":"Feature","id":"zone_a","properties":{},"geometry":{"type":"Polygon","coordinates":
        [[[30.0,41.0],[30.1,41.0],[30.1,41.1],[30.0,41.1],[30.0,41.0]]]}}]}"#;
    static OPEN_RING: &[u8] = br#"{"type":"FeatureCollection","features":[
        {"type":"Feature","id":"zone_a","properties":{},"geometry":{"type":"Polygon","coordinates":
        [[[29.0,41.0],[29.1,41.0],[29.1,41.1],[29.0,41.1]]]}}]}"#;

    for (label, geojson, expected_rule) in [
        ("type eksik", MISSING_TYPE, "LOC_008"),
        ("properties eksik", MISSING_PROPERTIES, "LOC_009"),
        ("id yinelenmiş", DUPLICATE_ID, "LOC_007"),
        ("poligon halkası açık", OPEN_RING, "LOC_004"),
    ] {
        let mut files = base_files();
        files.retain(|(n, _)| *n != "stops.txt" && *n != "stop_times.txt");
        files.push(("stops.txt", EMPTY_STOPS));
        files.push(("stop_times.txt", FLEX_STOP_TIMES));
        files.push(("locations.geojson", geojson));
        let ValidateResult::Ok(vr) = run(&files) else {
            panic!("{label}: feed okunmalı");
        };
        assert!(has(&vr, expected_rule), "{label}: kök GeoJSON bulgusu görünmeli");
        assert!(has(&vr, "ARC_035"), "{label}: geçersiz bölge muafiyet vermemeli");
        assert!(has(&vr, "STP_018"), "{label}: geçersiz bölge STP_018'i bastırmamalı");
    }
}

#[test]
fn invalid_geojson_does_not_make_missing_stops_optional() {
    static INVALID_GEOJSON: &[u8] = b"{not valid geojson";
    let mut files = base_files();
    files.retain(|(n, _)| *n != "stops.txt");
    files.push(("locations.geojson", INVALID_GEOJSON));

    match validate_bytes(&make_zip(&files), &ValidatorConfig::default(), TODAY) {
        ValidateResult::Ok(vr) => {
            let partial = vr
                .partial
                .expect("bozuk locations.geojson PARTIAL üretmeli");
            assert!(partial
                .unavailable_files
                .contains(&"locations.geojson".to_string()));
            assert!(partial.unavailable_files.contains(&"stops.txt".to_string()));
            assert!(vr.notices.iter().any(|n| {
                n.rule_id == "ARC_004" && n.observed_value.as_deref() == Some("stops.txt")
            }));
        }
        ValidateResult::Fatal(e) => panic!("bozuk optional GeoJSON Fatal olmamalı: {e:?}"),
    }
}

/// If both stops.txt AND locations.geojson are absent, where service is provided is undefined
/// → report PARTIAL; it is not Fatal because the ZIP remains readable.
#[test]
fn feed_without_stops_txt_and_without_geojson_is_partial() {
    let mut files = base_files();
    files.retain(|(n, _)| *n != "stops.txt");
    match validate_bytes(&make_zip(&files), &ValidatorConfig::default(), TODAY) {
        ValidateResult::Ok(vr) => {
            assert_eq!(vr.status, gtfs_core::ValidationStatus::Partial);
            assert!(vr
                .partial
                .expect("partial kapsamı raporlanmalı")
                .unavailable_files
                .contains(&"stops.txt".to_string()));
        }
        ValidateResult::Fatal(e) => panic!("okunabilir ZIP Fatal olmamalı: {e:?}"),
    }
}

// ── Flex stop_times: stop_id CONDITIONALLY required ───────────────────────────
// Spec (stop_times.stop_id): "Conditionally Required — Required if
// stop_times.location_group_id AND stop_times.location_id are NOT defined.
// Forbidden if stop_times.location_group_id or stop_times.location_id are defined."
// Until 2026-08-01 K1 treated stop_id as unconditionally required, the six official Flex
// columns were absent from known_columns, and STM_006 fired unconditionally for an empty
// stop_id — although the K2 parser ALREADY read these fields. K1 and K2 disagreed about one row.

#[test]
fn flex_stop_times_row_is_valid_without_stop_id() {
    static FLEX_STOP_TIMES: &[u8] =
        b"trip_id,location_id,stop_sequence,start_pickup_drop_off_window,end_pickup_drop_off_window,\
pickup_booking_rule_id,drop_off_booking_rule_id\n\
          T1,zone_a,1,08:00:00,12:00:00,BR1,BR1\n\
          T1,zone_b,2,08:00:00,12:00:00,BR1,BR1\n";
    let mut files = base_files();
    files.retain(|(n, _)| *n != "stop_times.txt");
    files.push(("stop_times.txt", FLEX_STOP_TIMES));

    let vr = match validate_bytes(&make_zip(&files), &ValidatorConfig::default(), TODAY) {
        ValidateResult::Ok(v) => v,
        ValidateResult::Fatal(e) => {
            panic!("Flex stop_times Fatal olmamalı: {:?} {}", e.code, e.message)
        }
    };
    let ids: Vec<&str> = vr.notices.iter().map(|n| n.rule_id.as_str()).collect();
    assert!(
        !ids.contains(&"STM_006"),
        "location_id taşıyan satır stop_id istemez → STM_006 çıkmamalı"
    );
    assert!(
        !ids.contains(&"ARC_025"),
        "Flex dosyada stop_id sütunu beklenmez → ARC_025 çıkmamalı"
    );
    // None of the six official Flex columns should count as an "unknown column".
    let unknown: Vec<&str> = vr
        .notices
        .iter()
        .filter(|n| n.rule_id == "ARC_017")
        .filter_map(|n| n.field.as_deref())
        .collect();
    assert!(
        unknown.is_empty(),
        "resmî Flex sütunları ARC_017 üretmemeli: {unknown:?}"
    );
}

/// Narrowing the requirement must not come at a cost: an empty stop_id is still an error
/// on a row WITHOUT Flex fields.
#[test]
fn non_flex_row_with_empty_stop_id_still_produces_stm_006() {
    static BAD_STOP_TIMES: &[u8] = b"trip_id,arrival_time,departure_time,stop_id,stop_sequence\n\
          T1,08:00:00,08:00:00,,1\nT1,08:10:00,08:10:00,S2,2\n";
    let mut files = base_files();
    files.retain(|(n, _)| *n != "stop_times.txt");
    files.push(("stop_times.txt", BAD_STOP_TIMES));
    match validate_bytes(&make_zip(&files), &ValidatorConfig::default(), TODAY) {
        ValidateResult::Ok(vr) => assert!(
            vr.notices.iter().any(|n| n.rule_id == "STM_006"),
            "Flex alanı olmayan satırda boş stop_id STM_006 üretmeli"
        ),
        ValidateResult::Fatal(e) => panic!("beklenmeyen fatal: {:?} {}", e.code, e.message),
    }
}

// ── XFL_019: routes.network_id cannot be combined with a SEPARATE network file ─
// Spec (routes.network_id): "Conditionally Forbidden — Forbidden if the route_networks.txt
// OR networks.txt file exists." Until 2026-08-01 only the route_networks.txt branch
// was checked; using networks.txt as well was equally forbidden but went undetected.

#[test]
fn xfl019_fires_when_networks_txt_is_used_with_routes_network_id() {
    static ROUTES_NET: &[u8] =
        b"route_id,agency_id,route_short_name,route_type,network_id\nR1,1,101,3,N1\n";
    static NETWORKS: &[u8] = b"network_id,network_name\nN1,Ana Ag\n";
    let mut files = base_files();
    files.retain(|(n, _)| *n != "routes.txt");
    files.push(("routes.txt", ROUTES_NET));
    files.push(("networks.txt", NETWORKS));
    match validate_bytes(&make_zip(&files), &ValidatorConfig::default(), TODAY) {
        ValidateResult::Ok(vr) => assert!(
            vr.notices.iter().any(|n| n.rule_id == "XFL_019"),
            "networks.txt + routes.network_id → XFL_019 çıkmalı"
        ),
        ValidateResult::Fatal(e) => panic!("beklenmeyen fatal: {:?} {}", e.code, e.message),
    }
}

/// routes.network_id is VALID by itself when no network file exists (Spec: "Optional otherwise").
#[test]
fn xfl019_silent_when_routes_network_id_is_used_alone() {
    static ROUTES_NET: &[u8] =
        b"route_id,agency_id,route_short_name,route_type,network_id\nR1,1,101,3,N1\n";
    let mut files = base_files();
    files.retain(|(n, _)| *n != "routes.txt");
    files.push(("routes.txt", ROUTES_NET));
    match validate_bytes(&make_zip(&files), &ValidatorConfig::default(), TODAY) {
        ValidateResult::Ok(vr) => assert!(
            !vr.notices.iter().any(|n| n.rule_id == "XFL_019"),
            "ayrı ağ dosyası yokken routes.network_id geçerlidir → XFL_019 çıkmamalı"
        ),
        ValidateResult::Fatal(e) => panic!("beklenmeyen fatal: {:?} {}", e.code, e.message),
    }
}

// ── DQ_021: COMPOSITE primary keys ────────────────────────────────────────────
// Most Fares v2 and Flex files use composite keys; DQ_021 used to scan only duplicates of
// stop_id, route_id, and trip_id. The key lists were copied directly from the Spec.

#[test]
fn dq021_detects_duplicate_composite_key_in_fare_leg_rules() {
    // Spec PK: (network_id, from_area_id, to_area_id, from_timeframe_group_id,
    //           to_timeframe_group_id, fare_product_id) — two rows are exactly identical.
    static FLR: &[u8] = b"network_id,from_area_id,to_area_id,fare_product_id\n\
                          N1,A1,A2,P1\nN1,A1,A2,P1\n";
    let mut files = base_files();
    files.push(("fare_leg_rules.txt", FLR));
    match validate_bytes(&make_zip(&files), &ValidatorConfig::default(), TODAY) {
        ValidateResult::Ok(vr) => assert!(
            vr.notices
                .iter()
                .any(|n| n.rule_id == "DQ_021" && n.file.as_deref() == Some("fare_leg_rules.txt")),
            "aynı bileşik anahtara sahip iki satır DQ_021 üretmeli"
        ),
        ValidateResult::Fatal(e) => panic!("beklenmeyen fatal: {:?} {}", e.code, e.message),
    }
}

/// If even ONE key field differs, the rows are distinct — there must be no false positive.
#[test]
fn dq021_silent_when_one_composite_field_differs() {
    static FLR: &[u8] = b"network_id,from_area_id,to_area_id,fare_product_id\n\
                          N1,A1,A2,P1\nN1,A1,A3,P1\n";
    let mut files = base_files();
    files.push(("fare_leg_rules.txt", FLR));
    match validate_bytes(&make_zip(&files), &ValidatorConfig::default(), TODAY) {
        ValidateResult::Ok(vr) => assert!(
            !vr.notices
                .iter()
                .any(|n| n.rule_id == "DQ_021" && n.file.as_deref() == Some("fare_leg_rules.txt")),
            "to_area_id farklı → ayrı anahtarlar, DQ_021 çıkmamalı"
        ),
        ValidateResult::Fatal(e) => panic!("beklenmeyen fatal: {:?} {}", e.code, e.message),
    }
}

#[test]
fn dq021_detects_duplicate_location_group_id_and_group_stop_row() {
    static LG: &[u8] = b"location_group_id,location_group_name\nLG1,Bolge\nLG1,Bolge Tekrar\n";
    static LGS: &[u8] = b"location_group_id,stop_id\nLG1,S1\nLG1,S1\n";
    let mut files = base_files();
    files.push(("location_groups.txt", LG));
    files.push(("location_group_stops.txt", LGS));
    match validate_bytes(&make_zip(&files), &ValidatorConfig::default(), TODAY) {
        ValidateResult::Ok(vr) => {
            assert!(
                vr.notices
                    .iter()
                    .any(|n| n.rule_id == "DQ_021"
                        && n.file.as_deref() == Some("location_groups.txt")),
                "yinelenen location_group_id DQ_021 üretmeli"
            );
            assert!(
                vr.notices.iter().any(|n| n.rule_id == "DQ_021"
                    && n.file.as_deref() == Some("location_group_stops.txt")),
                "yinelenen (location_group_id, stop_id) satırı DQ_021 üretmeli"
            );
        }
        ValidateResult::Fatal(e) => panic!("beklenmeyen fatal: {:?} {}", e.code, e.message),
    }
}

// ── XFL_031: shared identifier namespace ─────────────────────────────────────
// Spec (location_groups.location_group_id): "ID must be unique across all stops.stop_id,
// locations.geojson id, and location_groups.location_group_id values."

#[test]
fn xfl031_detects_location_group_id_clashing_with_stop_id() {
    // LG1 appears in both stops.txt and location_groups.txt → the
    // stop_times.location_group_id reference becomes ambiguous.
    static STOPS_CLASH: &[u8] = b"stop_id,stop_name,stop_lat,stop_lon\n\
        S1,Durak1,41.0,29.0\nS2,Durak2,41.1,29.1\nLG1,Cakisan,41.2,29.2\n";
    static LG: &[u8] = b"location_group_id,location_group_name\nLG1,Bolge\n";
    let mut files = base_files();
    files.retain(|(n, _)| *n != "stops.txt");
    files.push(("stops.txt", STOPS_CLASH));
    files.push(("location_groups.txt", LG));
    match validate_bytes(&make_zip(&files), &ValidatorConfig::default(), TODAY) {
        ValidateResult::Ok(vr) => assert!(
            vr.notices
                .iter()
                .any(|n| n.rule_id == "XFL_031" && n.entity_id.as_deref() == Some("LG1")),
            "stop_id ile çakışan location_group_id XFL_031 üretmeli"
        ),
        ValidateResult::Fatal(e) => panic!("beklenmeyen fatal: {:?} {}", e.code, e.message),
    }
}

/// No notice should be emitted when IDs are distinct — the rule measures only CLASHES.
#[test]
fn xfl031_silent_when_ids_are_distinct() {
    static LG: &[u8] = b"location_group_id,location_group_name\nLG1,Bolge\n";
    let mut files = base_files();
    files.push(("location_groups.txt", LG));
    match validate_bytes(&make_zip(&files), &ValidatorConfig::default(), TODAY) {
        ValidateResult::Ok(vr) => assert!(
            !vr.notices.iter().any(|n| n.rule_id == "XFL_031"),
            "ayrık kimliklerde XFL_031 çıkmamalı"
        ),
        ValidateResult::Fatal(e) => panic!("beklenmeyen fatal: {:?} {}", e.code, e.message),
    }
}

#[test]
fn xfl031_detects_geojson_id_clashing_with_stop_id() {
    static GEOJSON: &[u8] = br#"{"type":"FeatureCollection","features":[
        {"type":"Feature","id":"S1","geometry":{"type":"Polygon","coordinates":
        [[[29.0,41.0],[29.1,41.0],[29.1,41.1],[29.0,41.1],[29.0,41.0]]]},"properties":{}}]}"#;
    let mut files = base_files();
    files.push(("locations.geojson", GEOJSON));
    match validate_bytes(&make_zip(&files), &ValidatorConfig::default(), TODAY) {
        ValidateResult::Ok(vr) => assert!(
            vr.notices
                .iter()
                .any(|n| n.rule_id == "XFL_031" && n.entity_id.as_deref() == Some("S1")),
            "stops.txt'teki S1 ile çakışan geojson id XFL_031 üretmeli"
        ),
        ValidateResult::Fatal(e) => panic!("beklenmeyen fatal: {:?} {}", e.code, e.message),
    }
}

// ── LOC_008/009/010: required locations.geojson members ───────────────────────
// The Spec (locations.geojson field table) marks type / properties /
// geometry.coordinates as Required for every Feature. None were checked:
// missing members were silently skipped and the file appeared "validated".

fn geojson_feed(geojson: &'static [u8]) -> Vec<(&'static str, &'static [u8])> {
    let mut files = base_files();
    files.push(("locations.geojson", geojson));
    files
}

#[test]
fn loc008_009_010_detect_missing_required_feature_members() {
    // no type · no properties · no coordinates
    static BAD: &[u8] = br#"{"type":"FeatureCollection","features":[
        {"id":"z1","geometry":{"type":"Polygon"}}]}"#;
    match validate_bytes(
        &make_zip(&geojson_feed(BAD)),
        &ValidatorConfig::default(),
        TODAY,
    ) {
        ValidateResult::Ok(vr) => {
            let ids: Vec<&str> = vr.notices.iter().map(|n| n.rule_id.as_str()).collect();
            assert!(
                ids.contains(&"LOC_008"),
                "'type' eksik → LOC_008 çıkmalı: {ids:?}"
            );
            assert!(
                ids.contains(&"LOC_009"),
                "'properties' eksik → LOC_009 çıkmalı: {ids:?}"
            );
            assert!(
                ids.contains(&"LOC_010"),
                "'coordinates' eksik → LOC_010 çıkmalı: {ids:?}"
            );
        }
        ValidateResult::Fatal(e) => panic!("beklenmeyen fatal: {:?} {}", e.code, e.message),
    }
}

/// A valid feature must produce none of them; an empty `properties` object is VALID
/// (stop_name/stop_desc are optional).
#[test]
fn loc008_009_010_silent_on_a_valid_feature() {
    static GOOD: &[u8] = br#"{"type":"FeatureCollection","features":[
        {"type":"Feature","id":"z1","properties":{},"geometry":{"type":"Polygon","coordinates":
        [[[29.0,41.0],[29.1,41.0],[29.1,41.1],[29.0,41.1],[29.0,41.0]]]}}]}"#;
    match validate_bytes(
        &make_zip(&geojson_feed(GOOD)),
        &ValidatorConfig::default(),
        TODAY,
    ) {
        ValidateResult::Ok(vr) => {
            let bad: Vec<&str> = vr
                .notices
                .iter()
                .map(|n| n.rule_id.as_str())
                .filter(|r| matches!(*r, "LOC_008" | "LOC_009" | "LOC_010"))
                .collect();
            assert!(bad.is_empty(), "geçerli feature notice üretmemeli: {bad:?}");
        }
        ValidateResult::Fatal(e) => panic!("beklenmeyen fatal: {:?} {}", e.code, e.message),
    }
}

// ── rule_priority belongs only to fare_leg_rules.txt ───────────────────────────
// Spec (27 April 2026): `rule_priority` is defined in fare_leg_rules.txt; it is NOT in the
// fare_transfer_rules.txt field table. K1 treated it as a "known column" in both, so a
// rule_priority column in the wrong file did not produce ARC_017 (caught in the WP-2 Spec anchor pass).

#[test]
fn rule_priority_in_fare_transfer_rules_is_an_unknown_column() {
    static FTR: &[u8] =
        b"from_leg_group_id,to_leg_group_id,fare_transfer_type,rule_priority\nLG1,LG2,0,1\n";
    let mut files = base_files();
    files.push(("fare_transfer_rules.txt", FTR));

    match run(&files) {
        ValidateResult::Ok(vr) => {
            let hit = vr.notices.iter().any(|n| {
                n.rule_id == "ARC_017"
                    && n.file.as_deref() == Some("fare_transfer_rules.txt")
                    && n.field.as_deref() == Some("rule_priority")
            });
            assert!(
                hit,
                "fare_transfer_rules.rule_priority ARC_017 üretmeli, emit: {:?}",
                vr.notices
                    .iter()
                    .map(|n| n.rule_id.as_str())
                    .collect::<Vec<_>>()
            );
        }
        other => panic!("ValidateResult::Ok beklendi, gelen: {other:?}"),
    }
}

#[test]
fn rule_priority_in_fare_leg_rules_stays_a_known_column() {
    static FLR: &[u8] = b"leg_group_id,network_id,rule_priority\nLG1,N1,1\n";
    let mut files = base_files();
    files.push(("fare_leg_rules.txt", FLR));

    match run(&files) {
        ValidateResult::Ok(vr) => {
            let hit = vr
                .notices
                .iter()
                .any(|n| n.rule_id == "ARC_017" && n.field.as_deref() == Some("rule_priority"));
            assert!(
                !hit,
                "fare_leg_rules.rule_priority resmî alandır, ARC_017 çıkmamalı"
            );
        }
        other => panic!("ValidateResult::Ok beklendi, gelen: {other:?}"),
    }
}

// ── Linked-trip continuations: TRF_022 / TRF_023 ─────────────────────────────
//
// The Spec (`transfers.txt` → "Linked trips") has THREE sentences, and the third LIMITS the first two:
// continuation trips MUST have identical `service_id`, BUT one trip may belong to multiple separate
// continuations — as long as their calendars do not overlap on any day. Since the data has no field
// identifying a continuation group, the only implementable reading is their combination.
// The four tests below draw exactly that boundary: one proves firing, and two prove SILENCE.

static CAL_TWO_SERVICES: &[u8] =
    b"service_id,monday,tuesday,wednesday,thursday,friday,saturday,sunday,start_date,end_date\n\
      SVC1,1,1,1,1,1,0,0,20250101,20271231\n\
      SVC2,1,1,1,1,1,1,1,20250101,20271231\n";
// SVC3 is weekends only → it overlaps with SVC1 (weekdays) on NO day.
static CAL_DISJOINT: &[u8] =
    b"service_id,monday,tuesday,wednesday,thursday,friday,saturday,sunday,start_date,end_date\n\
      SVC1,1,1,1,1,1,0,0,20250101,20271231\n\
      SVC3,0,0,0,0,0,1,1,20250101,20271231\n";
static ST_THREE_TRIPS: &[u8] = b"trip_id,arrival_time,departure_time,stop_id,stop_sequence\n\
      T1,08:00:00,08:00:00,S1,1\nT1,08:10:00,08:10:00,S2,2\n\
      T2,09:00:00,09:00:00,S1,1\nT2,09:10:00,09:10:00,S2,2\n\
      T3,10:00:00,10:00:00,S1,1\nT3,10:10:00,10:10:00,S2,2\n";

fn linked_trip_files(
    calendar: &'static [u8],
    trips: &'static [u8],
    transfers: &'static [u8],
) -> Vec<(&'static str, &'static [u8])> {
    let mut files = base_files();
    files.retain(|(n, _)| *n != "calendar.txt" && *n != "trips.txt" && *n != "stop_times.txt");
    files.push(("calendar.txt", calendar));
    files.push(("trips.txt", trips));
    files.push(("stop_times.txt", ST_THREE_TRIPS));
    files.push(("transfers.txt", transfers));
    files
}

fn has(vr: &gtfs_core::ValidationResult, rule: &str) -> bool {
    vr.notices.iter().any(|n| n.rule_id == rule)
}

#[test]
fn trf022_flags_one_to_n_continuation_with_overlapping_calendars() {
    // T1 connects to both T2 (SVC1, weekdays) and T3 (SVC2, every day).
    // Both are active on weekdays → which continuation applies is ambiguous.
    let files = linked_trip_files(
        CAL_TWO_SERVICES,
        b"route_id,service_id,trip_id\nR1,SVC1,T1\nR1,SVC1,T2\nR1,SVC2,T3\n",
        b"from_stop_id,to_stop_id,transfer_type,from_trip_id,to_trip_id\nS1,S2,4,T1,T2\nS1,S2,4,T1,T3\n",
    );
    match run(&files) {
        ValidateResult::Ok(vr) => {
            assert!(
                has(&vr, "TRF_022"),
                "TRF_022 olmalı. Mevcut: {:?}",
                vr.notices
                    .iter()
                    .map(|n| n.rule_id.as_str())
                    .collect::<Vec<_>>()
            );
            // The reverse direction must not fire: T2 and T3 each have only one incoming trip.
            assert!(
                !has(&vr, "TRF_023"),
                "n-to-1 çelişkisi yok, TRF_023 çıkmamalı"
            );
        }
        other => panic!("ValidateResult::Ok beklendi, gelen: {other:?}"),
    }
}

#[test]
fn trf022_silent_when_continuation_trips_share_one_service_id() {
    // A REAL 1-to-n split: T1 branches to two trips, but both use the SAME calendar.
    // This is exactly what the Spec allows — the rule must stay silent.
    let files = linked_trip_files(
        CAL_TWO_SERVICES,
        b"route_id,service_id,trip_id\nR1,SVC1,T1\nR1,SVC1,T2\nR1,SVC1,T3\n",
        b"from_stop_id,to_stop_id,transfer_type,from_trip_id,to_trip_id\nS1,S2,4,T1,T2\nS1,S2,4,T1,T3\n",
    );
    match run(&files) {
        ValidateResult::Ok(vr) => assert!(
            !has(&vr, "TRF_022"),
            "Özdeş service_id geçerli 1-to-n devamlılıktır, TRF_022 çıkmamalı"
        ),
        other => panic!("ValidateResult::Ok beklendi, gelen: {other:?}"),
    }
}

#[test]
fn trf022_silent_when_separate_continuations_do_not_share_a_service_day() {
    // SEPARATE CONTINUATIONS: T1 continues to T2 on weekdays and to T3 on weekends.
    // The service_ids are DIFFERENT but never overlap → the Spec EXPLICITLY allows this.
    // Without this test, the rule would repeat the `PTH_017` mistake of treating Spec permission as a norm.
    let files = linked_trip_files(
        CAL_DISJOINT,
        b"route_id,service_id,trip_id\nR1,SVC1,T1\nR1,SVC1,T2\nR1,SVC3,T3\n",
        b"from_stop_id,to_stop_id,transfer_type,from_trip_id,to_trip_id\nS1,S2,4,T1,T2\nS1,S2,4,T1,T3\n",
    );
    match run(&files) {
        ValidateResult::Ok(vr) => assert!(
            !has(&vr, "TRF_022"),
            "Çakışmayan takvimler ayrı devamlılıktır, TRF_022 çıkmamalı. Mevcut: {:?}",
            vr.notices
                .iter()
                .map(|n| n.rule_id.as_str())
                .collect::<Vec<_>>()
        ),
        other => panic!("ValidateResult::Ok beklendi, gelen: {other:?}"),
    }
}

#[test]
fn trf023_flags_n_to_one_continuation_with_overlapping_calendars() {
    // Both T2 (SVC1) and T3 (SVC2) connect to T1 → the incoming side conflicts.
    let files = linked_trip_files(
        CAL_TWO_SERVICES,
        b"route_id,service_id,trip_id\nR1,SVC1,T1\nR1,SVC1,T2\nR1,SVC2,T3\n",
        b"from_stop_id,to_stop_id,transfer_type,from_trip_id,to_trip_id\nS1,S2,4,T2,T1\nS1,S2,4,T3,T1\n",
    );
    match run(&files) {
        ValidateResult::Ok(vr) => {
            assert!(
                has(&vr, "TRF_023"),
                "TRF_023 olmalı. Mevcut: {:?}",
                vr.notices
                    .iter()
                    .map(|n| n.rule_id.as_str())
                    .collect::<Vec<_>>()
            );
            assert!(
                !has(&vr, "TRF_022"),
                "1-to-n çelişkisi yok, TRF_022 çıkmamalı"
            );
        }
        other => panic!("ValidateResult::Ok beklendi, gelen: {other:?}"),
    }
}

// ── DQ_021 primary key — three files added on 2026-08-06 ─────────────────────

#[test]
fn dq021_flags_calendar_dates_with_conflicting_exception_types() {
    // The same (service_id, date) has both exception_type=1 and =2 → service on that day is UNDEFINED.
    // The most severe form of a primary-key violation; no CLD_* rule detected it.
    static CLD: &[u8] = b"service_id,date,exception_type\nSVC1,20260701,1\nSVC1,20260701,2\n";
    let mut files = base_files();
    files.push(("calendar_dates.txt", CLD));
    match run(&files) {
        ValidateResult::Ok(vr) => {
            let hit = vr
                .notices
                .iter()
                .find(|n| n.rule_id == "DQ_021" && n.file.as_deref() == Some("calendar_dates.txt"));
            assert!(
                hit.is_some(),
                "calendar_dates için DQ_021 olmalı. Mevcut: {:?}",
                vr.notices
                    .iter()
                    .map(|n| n.rule_id.as_str())
                    .collect::<Vec<_>>()
            );
        }
        other => panic!("ValidateResult::Ok beklendi, gelen: {other:?}"),
    }
}

#[test]
fn dq021_silent_when_calendar_dates_keys_are_unique() {
    static CLD: &[u8] = b"service_id,date,exception_type\nSVC1,20260701,2\nSVC1,20260702,2\n";
    let mut files = base_files();
    files.push(("calendar_dates.txt", CLD));
    match run(&files) {
        ValidateResult::Ok(vr) => assert!(
            !vr.notices.iter().any(|n| {
                n.rule_id == "DQ_021" && n.file.as_deref() == Some("calendar_dates.txt")
            }),
            "Benzersiz (service_id, date) çiftlerinde DQ_021 çıkmamalı"
        ),
        other => panic!("ValidateResult::Ok beklendi, gelen: {other:?}"),
    }
}

#[test]
fn dq021_flags_duplicate_rows_in_stop_areas_and_fare_rules() {
    static AREAS: &[u8] = b"area_id\nA1\n";
    static STOP_AREAS: &[u8] = b"area_id,stop_id\nA1,S1\nA1,S1\n";
    static FARE_ATTR: &[u8] =
        b"fare_id,price,currency_type,payment_method,transfers\nF1,2.50,USD,0,0\n";
    static FARE_RULES: &[u8] = b"fare_id,route_id\nF1,R1\nF1,R1\n";
    let mut files = base_files();
    files.push(("areas.txt", AREAS));
    files.push(("stop_areas.txt", STOP_AREAS));
    files.push(("fare_attributes.txt", FARE_ATTR));
    files.push(("fare_rules.txt", FARE_RULES));
    match run(&files) {
        ValidateResult::Ok(vr) => {
            let files_hit: Vec<&str> = vr
                .notices
                .iter()
                .filter(|n| n.rule_id == "DQ_021")
                .filter_map(|n| n.file.as_deref())
                .collect();
            assert!(
                files_hit.contains(&"stop_areas.txt"),
                "stop_areas.txt için DQ_021 olmalı. DQ_021 dosyaları: {files_hit:?}"
            );
            assert!(
                files_hit.contains(&"fare_rules.txt"),
                "fare_rules.txt için DQ_021 olmalı. DQ_021 dosyaları: {files_hit:?}"
            );
        }
        other => panic!("ValidateResult::Ok beklendi, gelen: {other:?}"),
    }
}

// ── PTH_031: a stop with stop_access=1 cannot be a pathway endpoint ──────────
//
// Spec `pathways.from_stop_id`/`to_stop_id`: "Values for stop_id that identify stations
// (location_type=1), **or stops (location_type=0 or empty) with stop_access=1**, are
// forbidden." The second branch was not measured until 2026-08-06; the catalog could not see it
// either because the prohibition was written in lowercase ("are forbidden").

static STOPS_WITH_ACCESS: &[u8] =
    b"stop_id,stop_name,stop_lat,stop_lon,location_type,parent_station,stop_access\n\
      ST,Station,41.0,29.0,1,,\nS1,Platform1,41.0,29.0,0,ST,1\nS2,Platform2,41.1,29.1,0,ST,0\n";
static STOPS_NO_ACCESS: &[u8] =
    b"stop_id,stop_name,stop_lat,stop_lon,location_type,parent_station,stop_access\n\
      ST,Station,41.0,29.0,1,,\nS1,Platform1,41.0,29.0,0,ST,0\nS2,Platform2,41.1,29.1,0,ST,0\n";
static PATHWAY_S1_S2: &[u8] =
    b"pathway_id,from_stop_id,to_stop_id,pathway_mode,is_bidirectional\nP1,S1,S2,1,1\n";

fn pathway_files(stops: &'static [u8]) -> Vec<(&'static str, &'static [u8])> {
    let mut files = base_files();
    files.retain(|(n, _)| *n != "stops.txt");
    files.push(("stops.txt", stops));
    files.push(("pathways.txt", PATHWAY_S1_S2));
    files
}

#[test]
fn pth031_flags_pathway_endpoint_with_stop_access_1() {
    match run(&pathway_files(STOPS_WITH_ACCESS)) {
        ValidateResult::Ok(vr) => assert!(
            has(&vr, "PTH_031"),
            "PTH_031 olmalı. Mevcut: {:?}",
            vr.notices
                .iter()
                .map(|n| n.rule_id.as_str())
                .collect::<Vec<_>>()
        ),
        other => panic!("ValidateResult::Ok beklendi, gelen: {other:?}"),
    }
}

#[test]
fn pth031_silent_when_endpoints_are_not_street_accessed() {
    // stop_access=0 = "not directly accessible from the street; a pathway must be used" →
    // exactly the case where a pathway is expected. The rule must be silent here.
    match run(&pathway_files(STOPS_NO_ACCESS)) {
        ValidateResult::Ok(vr) => assert!(
            !has(&vr, "PTH_031"),
            "stop_access=0 pathway'in beklendiği hâldir, PTH_031 çıkmamalı"
        ),
        other => panic!("ValidateResult::Ok beklendi, gelen: {other:?}"),
    }
}

#[test]
fn stp024_k2_range_and_stp026_normative_enum_are_distinct() {
    static STOPS_WITH_ENUMS: &[u8] = b"stop_id,stop_name,stop_lat,stop_lon,stop_access\n\
          S1,Stop1,41.0,29.0,2\n\
          S2,Stop2,41.1,29.1,9\n";
    let mut files = base_files();
    files.retain(|(name, _)| *name != "stops.txt");
    files.push(("stops.txt", STOPS_WITH_ENUMS));

    match run(&files) {
        ValidateResult::Ok(vr) => {
            assert_eq!(
                vr.notices.iter().filter(|n| n.rule_id == "STP_024").count(),
                1,
                "K2 0/1/2 kümesinde 2 kabul edilmeli, 9 için STP_024 üretilmeli"
            );
            assert_eq!(
                vr.notices.iter().filter(|n| n.rule_id == "STP_026").count(),
                1,
                "9 için STP_024, STP_026'yı bloklar; K4'ün bağımsız 2 bulgusu kalmalı"
            );
            assert_eq!(
                vr.notices
                    .iter()
                    .find(|n| n.rule_id == "STP_026")
                    .and_then(|n| n.observed_value.as_deref()),
                Some("2")
            );
        }
        other => panic!("ValidateResult::Ok beklendi, alınan: {other:?}"),
    }
}

// ── Pc3b911a6: "Files must be comma-delimited text" — MEASUREMENT, NOT ASSUMPTION ─
//
// The ledger counted this provision as INDIRECT ("if the delimiter is not a comma, the header
// collapses to one column → ARC_025 fatal"), but marked the rationale **[Assumption] — this path was not measured**.
// The 2026-08-06 external review correctly pointed out that `badge_status.py` counted it as
// `KANITLI + DOLAYLI`, so an untested inference was treated as "measured". We MEASURE it instead of dropping the assumption.

#[test]
fn semicolon_delimited_files_are_rejected_pc3b911a6() {
    // Same content, with a SEMICOLON delimiter. The header collapses to one column → required
    // columns cannot be found. Expected: either Fatal(NoRequiredFiles) or ARC_025 (required column missing).
    static SEMI_STOPS: &[u8] =
        b"stop_id;stop_name;stop_lat;stop_lon\nS1;Stop1;41.0;29.0\nS2;Stop2;41.1;29.1\n";
    let mut files = base_files();
    files.retain(|(n, _)| *n != "stops.txt");
    files.push(("stops.txt", SEMI_STOPS));

    match run(&files) {
        ValidateResult::Fatal(e) => {
            // Required file columns could not be resolved → fatal. The provision is MEASURED.
            assert_eq!(
                e.code,
                FatalCode::NoRequiredFiles,
                "virgül olmayan ayraç fatal üretmeli, gelen: {:?}",
                e.code
            );
        }
        ValidateResult::Ok(vr) => {
            assert!(
                has(&vr, "ARC_025"),
                "noktalı virgülle ayrılmış stops.txt ARC_025 üretmeli. Mevcut: {:?}",
                vr.notices
                    .iter()
                    .map(|n| n.rule_id.as_str())
                    .collect::<Vec<_>>()
            );
        }
    }

    // 🔴 CONTROL INPUT (issue #87). A violating input alone is insufficient: if a neighboring rule
    // fires in EVERY feed, the "measurement" proves nothing. The same content with a COMMA delimiter
    // must stay silent — the difference shows that the provision is genuinely measured.
    match run(&base_files()) {
        ValidateResult::Ok(vr) => assert!(
            !has(&vr, "ARC_025"),
            "virgülle ayrılmış aynı içerik ARC_025 ÜRETMEMELİ (kontrol)"
        ),
        other => panic!("kontrol girdisi Ok olmalı: {other:?}"),
    }
}

// ── Pd59e5eaa: the first row must contain FIELD NAMES (issue #87 evidence pair) ─
//
// The ledger counted this provision as INDIRECT ("if a data row appears instead of a header,
// ARC_025 + ARC_019 + ARC_017 fire together"), but that was a CODE READING, not a measurement.
// #87 was right: it was counted as `KANITLI + DOLAYLI`, i.e. an unmeasured inference.

#[test]
fn first_line_must_be_a_header_pd59e5eaa() {
    // VIOLATION: no header row — the file starts directly with data. The first DATA row is
    // then treated as the header, so required column names cannot be found.
    static NO_HEADER: &[u8] = b"S1,Stop1,41.0,29.0
S2,Stop2,41.1,29.1
";
    let mut files = base_files();
    for slot in files.iter_mut() {
        if slot.0 == "stops.txt" {
            slot.1 = NO_HEADER;
        }
    }
    let violating_fires = match run(&files) {
        ValidateResult::Fatal(e) => e.code == FatalCode::NoRequiredFiles,
        ValidateResult::Ok(vr) => has(&vr, "ARC_025"),
    };
    assert!(
        violating_fires,
        "başlıksız dosya zorunlu sütun ihlali üretmeli"
    );

    // CONTROL: the same data with a header row → silent.
    match run(&base_files()) {
        ValidateResult::Ok(vr) => assert!(
            !has(&vr, "ARC_025"),
            "başlığı olan aynı içerik ARC_025 ÜRETMEMELİ (kontrol)"
        ),
        other => panic!("kontrol girdisi Ok olmalı: {other:?}"),
    }
}

// ── LOC_011: OpenGIS SFS 6.1.11 polygon validity — extended clauses ───────────
//
// The 2026-08-06 external review identified four gaps written on the `LOC_011` card. Three
// were closed (hole-hole intersection · hole PARTIALLY outside · spike/cut line); the fourth
// (ring orientation) is not measured because **6.1.11 does not require orientation** — that is
// a GeoJSON RFC 7946 serialization rule, and treating it as a violation would create false positives.

/// Builds a one-feature `locations.geojson`; the geometry body is supplied per test.
/// `Box::leak`: `run()` expects `&'static [u8]`; one small leak per test is acceptable.
fn loc011_feed(geometry: &str) -> Vec<(&'static str, &'static [u8])> {
    let doc: &'static [u8] = Box::leak(
        format!(
            r#"{{"type":"FeatureCollection","features":[
                 {{"type":"Feature","id":"z1","properties":{{}},"geometry":{geometry}}}]}}"#
        )
        .into_bytes()
        .into_boxed_slice(),
    );
    geojson_feed(doc)
}

fn loc011_fires(geometry: &str) -> bool {
    match run(&loc011_feed(geometry)) {
        ValidateResult::Ok(vr) => has(&vr, "LOC_011"),
        other => panic!("ValidateResult::Ok beklendi, gelen: {other:?}"),
    }
}

#[test]
fn loc011_flags_holes_that_intersect_each_other() {
    // Two holes overlap → 6.1.11 (3) "no two Rings cross".
    assert!(
        loc011_fires(
            r#"{"type":"Polygon","coordinates":[
             [[0,0],[0,10],[10,10],[10,0],[0,0]],
             [[1,1],[1,5],[5,5],[5,1],[1,1]],
             [[3,3],[3,7],[7,7],[7,3],[3,3]]]}"#
        ),
        "birbirini kesen iki delik LOC_011 üretmeli"
    );
}

#[test]
fn loc011_flags_hole_partially_outside_the_shell() {
    // The hole's FIRST POINT is inside the shell, but its body extends outside. The old predicate
    // MISSED this because it looked only at the first point.
    assert!(
        loc011_fires(
            r#"{"type":"Polygon","coordinates":[
             [[0,0],[0,10],[10,10],[10,0],[0,0]],
             [[5,5],[5,15],[8,15],[8,5],[5,5]]]}"#
        ),
        "kısmen dışarı taşan delik LOC_011 üretmeli"
    );
}

#[test]
fn loc011_flags_spike() {
    // The shell leaves along a straight line and returns along the same line → zero-area spike.
    assert!(
        loc011_fires(
            r#"{"type":"Polygon","coordinates":[
             [[0,0],[0,10],[5,10],[20,10],[5,10],[10,10],[10,0],[0,0]]]}"#
        ),
        "spike/cut line LOC_011 üretmeli"
    );
}

#[test]
fn loc011_silent_on_valid_polygon_with_hole() {
    // Fully valid: the hole is inside the shell, with no intersection and no spike.
    assert!(
        !loc011_fires(
            r#"{"type":"Polygon","coordinates":[
             [[0,0],[0,10],[10,10],[10,0],[0,0]],
             [[2,2],[2,4],[4,4],[4,2],[2,2]]]}"#
        ),
        "geçerli poligon LOC_011 üretmemeli"
    );
}

#[test]
fn loc011_silent_on_clockwise_shell() {
    // ⚠️ MOST IMPORTANT TEST: ring orientation is NOT a 6.1.11 validity condition. A clockwise
    // shell is VALID; firing here would repeat the `PTH_017` mistake.
    assert!(
        !loc011_fires(
            r#"{"type":"Polygon","coordinates":[
             [[0,0],[10,0],[10,10],[0,10],[0,0]]]}"#
        ),
        "saat yönü kabuk 6.1.11'e göre GEÇERLİDİR, LOC_011 çıkmamalı"
    );
}

// ── STM_060: simultaneous overlap of GeoJSON zone + window + behavior ─────────
//
// The provision forbids satisfying all THREE conditions TOGETHER. Most tests are deliberately NEGATIVE:
// if one condition is absent, the rule must stay SILENT, or Flex feeds produce false positives.

fn flex_overlap_feed(
    geojson: &'static [u8],
    stop_times: &'static [u8],
) -> Vec<(&'static str, &'static [u8])> {
    let mut files = base_files();
    files.retain(|(n, _)| *n != "stop_times.txt");
    files.push(("stop_times.txt", stop_times));
    files.push(("locations.geojson", geojson));
    files
}

// Two zones: Z1 and Z2 OVERLAP; Z3 is completely separate.
static ZONES: &[u8] = br#"{"type":"FeatureCollection","features":[
  {"type":"Feature","id":"Z1","properties":{},"geometry":{"type":"Polygon","coordinates":
    [[[0,0],[0,10],[10,10],[10,0],[0,0]]]}},
  {"type":"Feature","id":"Z2","properties":{},"geometry":{"type":"Polygon","coordinates":
    [[[5,5],[5,15],[15,15],[15,5],[5,5]]]}},
  {"type":"Feature","id":"Z3","properties":{},"geometry":{"type":"Polygon","coordinates":
    [[[50,50],[50,60],[60,60],[60,50],[50,50]]]}}]}"#;

fn stm060_fires(stop_times: &'static [u8]) -> bool {
    match run(&flex_overlap_feed(ZONES, stop_times)) {
        ValidateResult::Ok(vr) => has(&vr, "STM_060"),
        other => panic!("ValidateResult::Ok beklendi, gelen: {other:?}"),
    }
}

#[test]
fn stm060_flags_overlapping_zone_window_and_behaviour() {
    // Z1∩Z2 intersect in space · 08:00-10:00 and 09:00-11:00 intersect in time · both allow pickup.
    static ST: &[u8] = b"trip_id,location_id,stop_sequence,start_pickup_drop_off_window,end_pickup_drop_off_window,pickup_type,drop_off_type\n\
        T1,Z1,1,08:00:00,10:00:00,2,2\nT1,Z2,2,09:00:00,11:00:00,2,2\n";
    assert!(stm060_fires(ST), "üç koşul da sağlanıyor → STM_060 çıkmalı");
}

#[test]
fn stm060_silent_when_zones_are_spatially_disjoint() {
    // Z1 and Z3 never intersect → the provision is not violated.
    static ST: &[u8] = b"trip_id,location_id,stop_sequence,start_pickup_drop_off_window,end_pickup_drop_off_window,pickup_type,drop_off_type\n\
        T1,Z1,1,08:00:00,10:00:00,2,2\nT1,Z3,2,09:00:00,11:00:00,2,2\n";
    assert!(!stm060_fires(ST), "ayrık bölgelerde STM_060 çıkmamalı");
}

#[test]
fn stm060_silent_when_time_windows_do_not_overlap() {
    // The zones intersect, but the windows are consecutive and do not overlap.
    static ST: &[u8] = b"trip_id,location_id,stop_sequence,start_pickup_drop_off_window,end_pickup_drop_off_window,pickup_type,drop_off_type\n\
        T1,Z1,1,08:00:00,10:00:00,2,2\nT1,Z2,2,10:00:00,12:00:00,2,2\n";
    assert!(
        !stm060_fires(ST),
        "çakışmayan pencerelerde STM_060 çıkmamalı"
    );
}

#[test]
fn stm060_silent_when_behaviour_does_not_overlap() {
    // The zone and time overlap, but one allows ONLY pickup and the other ONLY drop-off.
    // pickup_type=1 → no pickup · drop_off_type=1 → no drop-off.
    static ST: &[u8] = b"trip_id,location_id,stop_sequence,start_pickup_drop_off_window,end_pickup_drop_off_window,pickup_type,drop_off_type\n\
        T1,Z1,1,08:00:00,10:00:00,2,1\nT1,Z2,2,09:00:00,11:00:00,1,2\n";
    assert!(
        !stm060_fires(ST),
        "davranış örtüşmesi yoksa STM_060 çıkmamalı"
    );
}

#[test]
fn stm060_silent_across_different_trips() {
    // The provision applies to records with the SAME trip_id. Different trips are allowed.
    static ST: &[u8] = b"trip_id,location_id,stop_sequence,start_pickup_drop_off_window,end_pickup_drop_off_window,pickup_type,drop_off_type\n\
        T1,Z1,1,08:00:00,10:00:00,2,2\nT2,Z2,1,09:00:00,11:00:00,2,2\n";
    assert!(!stm060_fires(ST), "farklı trip'lerde STM_060 çıkmamalı");
}

#[test]
fn stm060_silent_for_two_records_in_the_same_zone() {
    // 🔴 MOST IMPORTANT NEGATIVE TEST. The Spec ITSELF requires this:
    // *"Travel within the same location group or GeoJSON location requires two records in
    // stop_times.txt with the same location_group_id or location_id."*
    // Two records for the same zone are the only representation of travel WITHIN that zone.
    //
    // My first implementation treated this as a violation and produced **21 false positives**
    // in `ch-odv-flex`/`mdb-2053`; MobilityData applies the same rule
    // (`overlapping_zone_and_pickup_drop_off_window`) yet stays SILENT on that feed.
    // Treating something the Spec requires in one place as forbidden elsewhere repeats the `PTH_017` mistake.
    static ST: &[u8] = b"trip_id,location_id,stop_sequence,start_pickup_drop_off_window,end_pickup_drop_off_window,pickup_type,drop_off_type\n\
        T1,Z1,1,07:00:00,11:00:00,2,2\nT1,Z1,2,07:00:00,11:00:00,2,2\n";
    assert!(
        !stm060_fires(ST),
        "aynı bölgedeki iki kayıt bölge-içi seyahattir, STM_060 çıkmamalı"
    );
}

#[test]
fn loc011_flags_hole_touching_shell_at_two_points() {
    // OpenGIS 6.1.11 clause 5: the interior must be CONNECTED. If a hole touches the outer ring
    // at TWO points, the polygon interior is split between those contacts.
    // ⚠️ Clause 3 allows tangency at ONE point → the negative test below protects that case.
    assert!(
        loc011_fires(
            r#"{"type":"Polygon","coordinates":[
             [[0,0],[0,10],[10,10],[10,0],[0,0]],
             [[0,3],[5,5],[0,7],[0,3]]]}"#
        ),
        "kabuğa iki noktada dokunan delik iç kısmı böler → LOC_011"
    );
}

#[test]
fn loc011_silent_when_hole_touches_shell_at_one_point() {
    // Tangency at one point is EXPLICITLY allowed by 6.1.11. Firing here would repeat
    // the `PTH_017` mistake.
    assert!(
        !loc011_fires(
            r#"{"type":"Polygon","coordinates":[
             [[0,0],[0,10],[10,10],[10,0],[0,0]],
             [[0,5],[3,7],[3,3],[0,5]]]}"#
        ),
        "tek noktada teğet delik 6.1.11'e göre GEÇERLİDİR"
    );
}

#[test]
fn loc011_flags_two_holes_touching_at_two_points() {
    assert!(
        loc011_fires(
            r#"{"type":"Polygon","coordinates":[
             [[0,0],[0,20],[20,20],[20,0],[0,0]],
             [[2,2],[2,8],[8,8],[8,2],[2,2]],
             [[8,2],[8,8],[14,8],[14,2],[8,2]]]}"#
        ),
        "iki noktada dokunan iki delik iç kısmı böler → LOC_011"
    );
}

// ── ARC_022: file row-count threshold — THREE emit sites ──────────────────────
//
// Issue #71. Until 2026-08-07 the threshold was HARDCODED in three places (`k1_parse`,
// `k2::shapes`, `k2::stop_times`), and the rule could not have a fixture because K1's
// `parse()` accepted no `ValidatorConfig` — the ledger recorded "requires 1M rows, cannot be inline".
// The correct reason was "the signature does not accept config"; the signature changed.
//
// ⚠️ The fixture (emit_proof) proves ONLY the K1 path. All three emit sites must read
// the same config field; here all three are measured separately, and silence at a row
// count EQUAL to the threshold (boundary behavior) is also verified.
//
// ⚠️ KNOWN GAP (NOT CLOSED in this round): `trips.txt` and `calendar_dates.txt` are streamed
// in K1 (`rows` stays empty), and K2 has NO ARC_022 emit sites → the rule stays silent for
// any row count in these two files. Closing it would produce new findings (feeds with 8M-row
// calendar_dates exist), so it is a behavior change; #71's scope is evidence, not scope expansion.

fn arc022_rows(file: &str, n: usize) -> String {
    let mut s = String::new();
    match file {
        "stops.txt" => {
            s.push_str("stop_id,stop_name,stop_lat,stop_lon\n");
            for i in 1..=n {
                s.push_str(&format!("S{i},Stop{i},41.{:03},29.{:03}\n", i, i));
            }
        }
        "stop_times.txt" => {
            s.push_str("trip_id,arrival_time,departure_time,stop_id,stop_sequence\n");
            for i in 1..=n {
                s.push_str(&format!("T1,06:{:02}:00,06:{:02}:00,S1,{i}\n", i, i));
            }
        }
        "shapes.txt" => {
            s.push_str("shape_id,shape_pt_lat,shape_pt_lon,shape_pt_sequence\n");
            for i in 1..=n {
                s.push_str(&format!("SH1,41.{:03},29.{:03},{i}\n", i, i));
            }
        }
        "trips.txt" => {
            s.push_str("route_id,service_id,trip_id\n");
            for i in 1..=n {
                s.push_str(&format!("R1,SVC1,T{i}\n"));
            }
        }
        "calendar_dates.txt" => {
            s.push_str("service_id,date,exception_type\n");
            for i in 1..=n {
                s.push_str(&format!("SVC1,2026{:02}{:02},2\n", 1 + i / 28, 1 + i % 28));
            }
        }
        other => panic!("beklenmeyen dosya: {other}"),
    }
    s
}

/// Builds `file` with `n` rows and returns whether ARC_022 fires for THAT FILE.
/// Threshold `max_file_rows = 10` (config lower bound) — no need to build a 1M-row ZIP.
fn arc022_fires_for(file: &str, n: usize) -> bool {
    let body = arc022_rows(file, n);
    let mut files = base_files();
    if let Some(slot) = files.iter_mut().find(|(name, _)| *name == file) {
        slot.1 = Box::leak(body.clone().into_bytes().into_boxed_slice());
    } else {
        files.push((
            Box::leak(file.to_string().into_boxed_str()),
            Box::leak(body.clone().into_bytes().into_boxed_slice()),
        ));
    }
    let cfg = ValidatorConfig {
        max_file_rows: 10,
        ..ValidatorConfig::default()
    };
    match validate_bytes(&make_zip(&files), &cfg, TODAY) {
        ValidateResult::Ok(vr) => vr
            .notices
            .iter()
            .any(|n| n.rule_id == "ARC_022" && n.entity_id.as_deref() == Some(file)),
        other => panic!("fatal beklenmiyordu: {other:?}"),
    }
}

#[test]
fn arc022_fires_on_k1_non_streamed_file() {
    // stops.txt is read outside the stream path → the threshold is read in k1_parse.
    assert!(
        arc022_fires_for("stops.txt", 11),
        "11 satır > eşik 10 → stops.txt için ARC_022 çıkmalı"
    );
}

#[test]
fn arc022_fires_on_k2_stop_times_stream() {
    // stop_times.txt is streamed in K1; K2 emits ARC_022 (rows is empty in K1).
    assert!(
        arc022_fires_for("stop_times.txt", 11),
        "11 satır > eşik 10 → stop_times.txt için ARC_022 çıkmalı"
    );
}

#[test]
fn arc022_fires_on_k2_shapes_stream() {
    assert!(
        arc022_fires_for("shapes.txt", 11),
        "11 satır > eşik 10 → shapes.txt için ARC_022 çıkmalı"
    );
}

// ── issue #75: the remaining TWO stream files ─────────────────────────────────
// `trips.txt` and `calendar_dates.txt` are also streamed in K1, but they had NO emit sites
// in K2 → the rule stayed silent for any row count in those files. The check is now in one
// PASS in `k2/mod.rs`; these two tests lock in that coverage.

#[test]
fn arc022_fires_on_k2_trips_stream() {
    assert!(
        arc022_fires_for("trips.txt", 11),
        "11 satır > eşik 10 → trips.txt için ARC_022 çıkmalı"
    );
}

#[test]
fn arc022_fires_on_k2_calendar_dates_stream() {
    assert!(
        arc022_fires_for("calendar_dates.txt", 11),
        "11 satır > eşik 10 → calendar_dates.txt için ARC_022 çıkmalı"
    );
}

#[test]
fn arc022_silent_at_the_threshold_on_every_path() {
    // Boundary: a row count EQUAL to the threshold is NOT a violation (`>` comparison).
    for file in [
        "stops.txt",
        "stop_times.txt",
        "shapes.txt",
        "trips.txt",
        "calendar_dates.txt",
    ] {
        assert!(
            !arc022_fires_for(file, 10),
            "{file}: eşiğe eşit (10) satır ARC_022 üretmemeli"
        );
    }
}

#[test]
fn arc022_notice_order_is_stable_across_runs() {
    // ⚠️ This test is NOT the guard for K2 ordering — measured on 2026-08-07: K7 itself guarantees
    // alphabetical output (`notice_order_key` sorts ARC_022 by `entity_id`); this test still passed
    // when K2's `sort` was removed. The unit test that checks that ordering is
    // birim testi `k2/mod.rs::files_over_row_limit_is_alphabetical_and_strict`.
    // The claim here is narrower but real: end-to-end output order is STABLE and alphabetical.
    let mut files = base_files();
    for name in ["shapes.txt", "trips.txt", "calendar_dates.txt"] {
        let body = Box::leak(arc022_rows(name, 11).into_bytes().into_boxed_slice());
        match files.iter_mut().find(|(n, _)| *n == name) {
            Some(slot) => slot.1 = body,
            None => files.push((name, body)),
        }
    }
    let cfg = ValidatorConfig {
        max_file_rows: 10,
        ..ValidatorConfig::default()
    };
    let zip = make_zip(&files);
    let order = |vr: &gtfs_core::ValidationResult| -> Vec<String> {
        vr.notices
            .iter()
            .filter(|n| n.rule_id == "ARC_022")
            .map(|n| n.entity_id.clone().unwrap_or_default())
            .collect()
    };
    let first = match validate_bytes(&zip, &cfg, TODAY) {
        ValidateResult::Ok(vr) => order(&vr),
        other => panic!("fatal beklenmiyordu: {other:?}"),
    };
    assert_eq!(
        first,
        vec!["calendar_dates.txt", "shapes.txt", "trips.txt"],
        "ARC_022 bulguları dosya adına göre sıralı gelmeli"
    );
    for _ in 0..5 {
        let again = match validate_bytes(&zip, &cfg, TODAY) {
            ValidateResult::Ok(vr) => order(&vr),
            other => panic!("fatal beklenmiyordu: {other:?}"),
        };
        assert_eq!(again, first, "aynı girdi aynı sırayı vermeli");
    }
}

// ── LOC_011 · 6.1.11 clause 5 REMAINDER: contact graph cycle (issue #74) ─────
//
// Until 2026-08-07 the measured criterion was "two rings touch at TWO points"; that is
// the shortest cycle in the contact graph (two parallel edges). Remainder: three rings can
// also close a cycle by touching pairwise at ONE point and split the interior. The criterion
// is now the cycle itself — the special case is covered automatically.

#[test]
fn loc011_flags_three_holes_touching_pairwise_in_a_chain() {
    // A–B, B–C, C–A: each contact is ONE point (clause 3 allows each individually), but together
    // they close a cycle → the enclosed area is CUT OFF from the rest of the interior.
    // The holes do not touch the shell; the cycle is only among the holes.
    assert!(
        loc011_fires(
            r#"{"type":"Polygon","coordinates":[
             [[0,0],[0,20],[20,20],[20,0],[0,0]],
             [[2,2],[12,2],[2,12],[2,2]],
             [[18,4],[18,12],[12,2],[18,4]],
             [[4,18],[2,12],[18,12],[4,18]]]}"#
        ),
        "üçgen temas zinciri iç kısmı böler → LOC_011"
    );
}

#[test]
fn loc011_silent_on_open_chain_of_single_point_contacts() {
    // A–B and B–C touch at one point each, C–A does NOT → the graph is a TREE, with no cycle.
    // The interior remains CONNECTED because one can go around the pinch points.
    // This proves that the cycle criterion does not become a false positive (`PTH_017` lesson).
    assert!(
        !loc011_fires(
            r#"{"type":"Polygon","coordinates":[
             [[0,0],[0,20],[20,20],[20,0],[0,0]],
             [[2,2],[12,2],[2,12],[2,2]],
             [[18,4],[18,12],[12,2],[18,4]],
             [[4,18],[6,14],[18,12],[4,18]]]}"#
        ),
        "kapanmayan temas zinciri iç kısmı bölmez, LOC_011 çıkmamalı"
    );
}

#[test]
fn loc011_flags_ring_that_touches_itself() {
    // Figure-eight shell: it visits corner (5,5) TWICE. `ring_is_simple` cannot see this
    // — tangency deliberately does not count as an "intersection" — but the interior splits
    // into two lobes, violating clause 5 (a SELF-LOOP in the contact graph).
    assert!(
        loc011_fires(
            r#"{"type":"Polygon","coordinates":[
             [[0,0],[10,0],[5,5],[10,10],[0,10],[5,5],[0,0]]]}"#
        ),
        "kendine dokunan ring iç kısmı böler → LOC_011"
    );
}

#[test]
fn loc011_silent_on_duplicate_consecutive_vertex() {
    // A CONSECUTIVE repeated coordinate (zero-length edge) is common in real data and is
    // NOT a self-contact. If self-contact checking did not skip adjacent indices, this common
    // imperfection would immediately become a false positive.
    assert!(
        !loc011_fires(
            r#"{"type":"Polygon","coordinates":[
             [[0,0],[0,10],[0,10],[10,10],[10,0],[0,0]]]}"#
        ),
        "ardışık yinelenen köşe öz-temas değildir, LOC_011 çıkmamalı"
    );
}

#[test]
fn loc011_silent_on_collinear_vertex_on_a_straight_edge() {
    // The intermediate vertex ((0,5)) on a straight edge lies on its neighboring edges;
    // it does NOT count as self-contact. Otherwise every densified boundary would fail.
    assert!(
        !loc011_fires(
            r#"{"type":"Polygon","coordinates":[
             [[0,0],[0,5],[0,10],[10,10],[10,0],[0,0]]]}"#
        ),
        "düz kenar üzerindeki ara köşe öz-temas değildir, LOC_011 çıkmamalı"
    );
}

// ── ARC_033: RFC 4180 quote rule ───────────────────────────────────────────────
//
// 🔴 First real FALSIFIER (2026-08-07, external reading). The tokenizer's unquoted-field branch
// stopped only at `,`, LF, and CR; `"` counted as an ordinary character. Thus the Spec's TWO hard
// provisions (`Pfedb83cf`, `Pa4c60372`) were counted INDIRECTLY through `ARC_013` in the ledger,
// while the violation was actually NEVER visible — measured: the finding set matched a valid feed EXACTLY.
//
// ⚠️ Making the rule MEASURABLE requires knowing field boundaries: `"12"" Street"`
// (valid) and `12" Street` (violation) produce the SAME value. The decision therefore belongs in the tokenizer.

fn arc033_feed(stops: &str) -> Vec<(&'static str, &'static [u8])> {
    let mut files = base_files();
    let body: &'static [u8] = Box::leak(stops.to_string().into_bytes().into_boxed_slice());
    for slot in files.iter_mut() {
        if slot.0 == "stops.txt" {
            slot.1 = body;
        }
    }
    files
}

fn arc033_fires(stops: &str) -> bool {
    match run(&arc033_feed(stops)) {
        ValidateResult::Ok(vr) => has(&vr, "ARC_033"),
        other => panic!("ValidateResult::Ok beklendi, gelen: {other:?}"),
    }
}

#[test]
fn arc033_flags_bare_quote_in_unquoted_field() {
    assert!(
        arc033_fires(
            "stop_id,stop_name,stop_lat,stop_lon\nS1,12\" Street,41.0,29.0\nS2,Stop2,41.1,29.1\n"
        ),
        "tırnaksız alandaki tırnak RFC 4180 ihlalidir → ARC_033"
    );
}

#[test]
fn arc033_flags_characters_after_a_closing_quote() {
    // `"Broadway"Ave` — the inner quote is NOT ESCAPED. Until now the tokenizer silently
    // split the record; ARC_012 appeared, but the reason was not recorded anywhere.
    assert!(arc033_fires(
        "stop_id,stop_name,stop_lat,stop_lon\nS1,\"Broadway\"Ave,41.0,29.0\nS2,Stop2,41.1,29.1\n"
    ), "kapanış tırnağından sonra karakter RFC 4180 ihlalidir → ARC_033");
}

#[test]
fn arc033_silent_on_correctly_escaped_quotes() {
    // 🔴 FALSE-POSITIVE GUARD. When parsed, this row produces the value `12" Street` —
    // the SAME value as the violating form. If the rule inspected the value, it would fire here
    // too and reject valid data (`PTH_017` mistake).
    assert!(!arc033_fires(
        "stop_id,stop_name,stop_lat,stop_lon\nS1,\"12\"\" Street\",41.0,29.0\nS2,Stop2,41.1,29.1\n"
    ), "doğru kaçırılmış tırnak GEÇERLİDİR, ARC_033 çıkmamalı");
}

#[test]
fn arc033_silent_on_quoted_value_containing_a_comma() {
    // A value containing a comma satisfies the rule when it is quoted.
    assert!(!arc033_fires(
        "stop_id,stop_name,stop_lat,stop_lon\nS1,\"Main St, North\",41.0,29.0\nS2,Stop2,41.1,29.1\n"
    ), "tırnaklanmış virgül GEÇERLİDİR, ARC_033 çıkmamalı");
}

#[test]
fn arc033_silent_on_a_feed_with_no_quotes_at_all() {
    assert!(
        !arc033_fires(
            "stop_id,stop_name,stop_lat,stop_lon\nS1,Stop1,41.0,29.0\nS2,Stop2,41.1,29.1\n"
        ),
        "tırnaksız temiz feed ARC_033 üretmemeli"
    );
}

#[test]
fn arc033_reports_one_summary_per_file_not_one_per_row() {
    // ARC_032/DQ_016 pattern: per-row emission for a feed with 1,176 marked stop names
    // overwhelms the scanner. The summary says how many ROWS were affected.
    let mut stops = String::from("stop_id,stop_name,stop_lat,stop_lon\n");
    for i in 1..=5 {
        stops.push_str(&format!("S{i},{i}\" Street,41.{i:03},29.{i:03}\n"));
    }
    match run(&arc033_feed(&stops)) {
        ValidateResult::Ok(vr) => {
            let hits: Vec<_> = vr
                .notices
                .iter()
                .filter(|n| n.rule_id == "ARC_033")
                .collect();
            assert_eq!(
                hits.len(),
                1,
                "dosya başına TEK özet bekleniyor, gelen: {}",
                hits.len()
            );
            let observed = hits[0].observed_value.clone().unwrap_or_default();
            assert!(
                observed.starts_with("rows=5;"),
                "özet satır sayısını taşımalı: {observed}"
            );
        }
        other => panic!("Ok beklendi: {other:?}"),
    }
}

#[test]
fn arc033_covers_streamed_files_too() {
    // ⚠️ `trips.txt` is STREAMED in K1 (`rows` stays empty) — tokenize_csv sees only the header.
    // Body scanning happens in `csv_scan`; if the two scanners diverged, the same file would stay
    // silent in stream mode. This would repeat the scope gap seen in ARC_022.
    let mut files = base_files();
    let trips: &'static [u8] = Box::leak(
        "route_id,service_id,trip_id,trip_headsign\nR1,SVC1,T1,5\" Line\n"
            .to_string()
            .into_bytes()
            .into_boxed_slice(),
    );
    for slot in files.iter_mut() {
        if slot.0 == "trips.txt" {
            slot.1 = trips;
        }
    }
    match run(&files) {
        ValidateResult::Ok(vr) => assert!(
            vr.notices
                .iter()
                .any(|n| n.rule_id == "ARC_033" && n.entity_id.as_deref() == Some("trips.txt")),
            "akış modundaki trips.txt için de ARC_033 çıkmalı"
        ),
        other => panic!("Ok beklendi: {other:?}"),
    }
}

// ── P71943a6f: file and field names are CASE-SENSITIVE ────────────────────────
//
// Spec (File Requirements): *"All file and field names are case-sensitive."* Because this sentence
// carries no modal, it was absent from the CATALOG until 2026-08-07 (issue #81) — so it could not
// be adjudicated or counted as orphaned. The behavior was already correct but NOT WRITTEN DOWN;
// bu testler onu sabitler.

#[test]
fn case_sensitivity_wrong_case_file_is_not_the_required_file() {
    // `Stops.txt` ≠ `stops.txt`. A wrong-case file does NOT count as required → PARTIAL.
    let mut files = base_files();
    for slot in files.iter_mut() {
        if slot.0 == "stops.txt" {
            slot.0 = "Stops.txt";
        }
    }
    match run(&files) {
        ValidateResult::Ok(vr) => {
            assert_eq!(vr.status, gtfs_core::ValidationStatus::Partial);
            assert!(vr.notices.iter().any(|n| n.rule_id == "ARC_004"));
        }
        other => panic!("Partial Ok beklendi, gelen: {other:?}"),
    }
}

#[test]
fn case_sensitivity_wrong_case_required_column_is_missing() {
    // `STOP_ID` ≠ `stop_id` → required column MISSING (ARC_025) + unknown column (ARC_017).
    let files =
        arc033_feed("STOP_ID,stop_name,stop_lat,stop_lon\nS1,A,41.0,29.0\nS2,B,41.1,29.1\n");
    match run(&files) {
        ValidateResult::Ok(vr) => {
            assert!(
                has(&vr, "ARC_025"),
                "yanlış-case zorunlu sütun ARC_025 üretmeli"
            );
            assert!(
                has(&vr, "ARC_017"),
                "yanlış-case sütun bilinmeyen sütun olarak da görünmeli"
            );
        }
        other => panic!("Ok beklendi: {other:?}"),
    }
}

#[test]
fn case_sensitivity_correct_case_stays_silent() {
    // Guard: correct casing must produce neither rule.
    let files =
        arc033_feed("stop_id,stop_name,stop_lat,stop_lon\nS1,A,41.0,29.0\nS2,B,41.1,29.1\n");
    match run(&files) {
        ValidateResult::Ok(vr) => {
            assert!(!has(&vr, "ARC_025"), "doğru yazımda ARC_025 çıkmamalı");
            assert!(!has(&vr, "ARC_017"), "doğru yazımda ARC_017 çıkmamalı");
        }
        other => panic!("Ok beklendi: {other:?}"),
    }
}

// ── issue #85: ID fields were silently trimmed before PK/FK checks ─────────────
//
// 🔴 5th audit round. `get_trimmed_field`/`get_col` trimmed every value, and IDs were built
// from the trimmed value. `" A "` and `A` are not the same LEXICAL value; PK/FK semantics are
// strict. Two cases measured on `main = 44d516d8`:
//   1) stops `" A "` + stop_times FK `A` → matched (fabricated FK SUCCESS)
//   2) stops has `A` and `" A "` → `STP_001` duplicate (fabricated CLASH)
// Extra whitespace is a separate quality signal, already reported by `DQ_016`.

fn id_ws_feed(stops: &'static str, stop_times: &'static str) -> Vec<(&'static str, &'static [u8])> {
    let mut files = base_files();
    for slot in files.iter_mut() {
        match slot.0 {
            "stops.txt" => slot.1 = stops.as_bytes(),
            "stop_times.txt" => slot.1 = stop_times.as_bytes(),
            _ => {}
        }
    }
    files
}

#[test]
fn id85_padded_stop_id_does_not_satisfy_a_reference_to_the_bare_id() {
    // stops defines only `" A "`; stop_times refers to `A` → the FK must NOT be satisfied.
    let files = id_ws_feed(
        "stop_id,stop_name,stop_lat,stop_lon\n\" A \",Bosluklu,41.0,29.0\nS2,Iki,41.1,29.1\n",
        "trip_id,arrival_time,departure_time,stop_id,stop_sequence\n\
         T1,08:00:00,08:00:00,A,1\nT1,08:10:00,08:10:00,S2,2\n",
    );
    match run(&files) {
        ValidateResult::Ok(vr) => assert!(
            vr.notices.iter().any(|n| n.rule_id == "STM_002"),
            "`A` referansı `\" A \"` durağıyla karşılanmamalı → STM_002 (stop_id stops.txt'te yok)"
        ),
        other => panic!("Ok beklendi: {other:?}"),
    }
}

#[test]
fn id85_padded_and_bare_ids_are_not_duplicates() {
    // `A` and `" A "` are DIFFERENT IDs; reporting a duplicate is a fabricated clash.
    let files = id_ws_feed(
        "stop_id,stop_name,stop_lat,stop_lon\nA,Bir,41.0,29.0\n\" A \",Iki,41.1,29.1\n",
        "trip_id,arrival_time,departure_time,stop_id,stop_sequence\n\
         T1,08:00:00,08:00:00,A,1\nT1,08:10:00,08:10:00,A,2\n",
    );
    match run(&files) {
        ValidateResult::Ok(vr) => assert!(
            !vr.notices.iter().any(|n| n.rule_id == "STP_001"),
            "farklı sözlüksel kimlikler duplicate DEĞİLDİR"
        ),
        other => panic!("Ok beklendi: {other:?}"),
    }
}

#[test]
fn id85_whitespace_only_id_still_reports_the_required_field() {
    // ⚠️ BOUNDARY: whitespace diagnosis USES trimming but does not change the ID. A `stop_id`
    // made only of whitespace must still count as "required field empty" (`STP_002`) — otherwise
    // this fix would LOSE a finding. Extra whitespace also remains reported separately as a quality
    // signal (`DQ_016`).
    let files = id_ws_feed(
        "stop_id,stop_name,stop_lat,stop_lon\n\"   \",Bosluk,41.0,29.0\nS2,Iki,41.1,29.1\n",
        "trip_id,arrival_time,departure_time,stop_id,stop_sequence\n\
         T1,08:00:00,08:00:00,S2,1\nT1,08:10:00,08:10:00,S2,2\n",
    );
    match run(&files) {
        ValidateResult::Ok(vr) => {
            assert!(
                has(&vr, "STP_002"),
                "yalnız boşluktan oluşan stop_id zorunlu-alan ihlalidir"
            );
            assert!(
                has(&vr, "DQ_016"),
                "fazladan boşluk AYRI kalite sinyali olarak kalmalı"
            );
        }
        other => panic!("Ok beklendi: {other:?}"),
    }
}

// ── issue #112: whitespace root, strict lexical semantics, and cascade gate ───

#[test]
fn whitespace112_keeps_root_and_independent_errors_but_suppresses_derivatives() {
    let files = vec![
        ("agency.txt", AGENCY),
    // The range violation at S1's latitude 91.0 is independent of whitespace and remains;
    // S2's value 41.1 can be reduced to a whitespace-only root.
        ("stops.txt", b"stop_id,stop_name,stop_lat,stop_lon\nS1,Stop1, 91.0,181.0\nS2,Stop2, 41.1,29.1\n" as &[u8]),
    // R1's value 99 is also semantically invalid; R2's value 3 carries only lexical
    // whitespace. The two independent URL errors remain visible as well.
        ("routes.txt", b"route_id,agency_id,route_short_name,route_type,route_url\n\
          R1,1,101, 99,ftp://bad.example\nR2,1,102, 3,ftp://bad.example\n" as &[u8]),
        ("trips.txt", TRIPS),
    // " S1" is the raw FK value. The root finding preserves this value as evidence; STM_002
    // is derived only because the trimmed counterpart of the same record exists in stops.txt.
    // The unknown " UNKNOWN " value must remain a real FK error.
        ("stop_times.txt", b"trip_id,arrival_time,departure_time,stop_id,stop_sequence\n\
          T1,08:00:00,08:00:00,\" S1\",1\nT1,08:10:00,08:10:00,S2,2\nT1,08:20:00,08:20:00,\" UNKNOWN \",3\n" as &[u8]),
        ("calendar.txt", CALENDAR),
    ];

    match run(&files) {
        ValidateResult::Ok(vr) => {
            let routes_root = vr
                .notices
                .iter()
                .find(|n| n.rule_id == "DQ_016" && n.file.as_deref() == Some("routes.txt"))
                .expect("routes.txt için DQ_016 kök bulgusu");
            let evidence = routes_root
                .details
                .as_ref()
                .and_then(|d| d.get("raw_samples"))
                .cloned()
                .unwrap_or_default();
            assert!(
                evidence.contains("route_type=\" 99\""),
                "ham enum değeri korunmalı: {evidence}"
            );
            assert!(
                routes_root
                    .details
                    .as_ref()
                    .and_then(|d| d.get("suppressed_derivative_rules"))
                    .is_some_and(|rules| rules.contains("RTS_004")),
                "typed route_type türevi audit özetinde görünmeli"
            );
            assert_eq!(
                vr.notices.iter().filter(|n| n.rule_id == "RTS_004").count(),
                1,
                "trim sonrası bağımsız geçersiz enum korunmalı, yalnız R2 türevi bastırılmalı"
            );
            assert!(
                vr.notices.iter().any(|n| n.rule_id == "RTS_005"),
                "aynı route entity'deki bağımsız URL hatası korunmalı"
            );

            let stops_root = vr
                .notices
                .iter()
                .find(|n| n.rule_id == "DQ_016" && n.file.as_deref() == Some("stops.txt"))
                .expect("stops.txt için DQ_016 kök bulgusu");
            assert!(
                stops_root
                    .details
                    .as_ref()
                    .and_then(|d| d.get("suppressed_derivative_rules"))
                    .is_some_and(|rules| rules.contains("STP_004")),
                "numeric parse türevi audit özetinde görünmeli"
            );
            assert_eq!(
                vr.notices.iter().filter(|n| n.rule_id == "STP_004").count(),
                1,
                "trim sonrası bağımsız geçersiz enlem korunmalı, yalnız S2 türevi bastırılmalı"
            );
            assert!(
                vr.notices.iter().any(|n| n.rule_id == "STP_005"),
                "aynı stop entity'deki bağımsız longitude aralık hatası korunmalı"
            );

            let stop_times_root = vr
                .notices
                .iter()
                .find(|n| n.rule_id == "DQ_016" && n.file.as_deref() == Some("stop_times.txt"))
                .expect("stop_times.txt için DQ_016 kök bulgusu");
            let stm_details = stop_times_root.details.as_ref().expect("audit details");
            assert!(
                stm_details
                    .get("raw_samples")
                    .is_some_and(|v| v.contains("stop_id=\" S1\"")),
                "ham PK/FK değeri kanıtta korunmalı"
            );
            assert!(
                stm_details
                    .get("suppressed_derivative_rules")
                    .is_some_and(|rules| rules.contains("STM_002")),
                "FK türevi audit özetinde görünmeli"
            );
            let stm_fk: Vec<_> = vr
                .notices
                .iter()
                .filter(|n| n.rule_id == "STM_002")
                .collect();
            assert_eq!(
                stm_fk.len(),
                1,
                "trim edilmiş hedefin türevi bastırılmalı, bilinmeyen FK hatası korunmalı"
            );
            assert!(
                stm_fk[0]
                    .observed_value
                    .as_deref()
                    .is_some_and(|v| v.contains("UNKNOWN")),
                "korunan STM_002 bilinmeyen ham FK değerini göstermeli: {:?}",
                stm_fk[0]
            );
        }
        other => panic!("ValidateResult::Ok beklendi, alınan: {other:?}"),
    }
}

#[test]
fn whitespace_fare_derivatives_are_counted_before_notice_allocation() {
    let mut files = base_files();
    files.push((
        "fare_attributes.txt",
        b"fare_id,price,currency_type,payment_method,transfers,agency_id\n\
          F1, 1.00,EUR, 0, 0, 1\n\
          F2, -1,EUR, 9, 9, UNKNOWN \n",
    ));
    files.push((
        "fare_rules.txt",
        b"fare_id,route_id,origin_id,destination_id,contains_id\nF1,R1,,,\nF2,R1,,,\n",
    ));

    match run(&files) {
        ValidateResult::Ok(vr) => {
            let root = vr
                .notices
                .iter()
                .find(|notice| {
                    notice.rule_id == "DQ_016"
                        && notice.file.as_deref() == Some("fare_attributes.txt")
                })
                .expect("fare_attributes.txt DQ_016 kökü");
            let details = root.details.as_ref().expect("DQ_016 audit özeti");
            assert_eq!(
                details
                    .get("suppressed_derivative_count")
                    .map(String::as_str),
                Some("4")
            );
            assert_eq!(
                details
                    .get("suppressed_derivative_rules")
                    .map(String::as_str),
                Some("FAR_002, FAR_004, FAR_005, FAR_008")
            );

            for rule in ["FAR_002", "FAR_004", "FAR_005", "FAR_008"] {
                let kept: Vec<_> = vr
                    .notices
                    .iter()
                    .filter(|notice| notice.rule_id == rule)
                    .collect();
                assert_eq!(kept.len(), 1, "{rule}: gerçek F2 hatası korunmalı");
                assert_eq!(kept[0].entity_id.as_deref(), Some("F2"));
                if rule == "FAR_008" {
                    assert_eq!(
                        kept[0]
                            .details
                            .as_ref()
                            .and_then(|details| details.get("whitespace_candidate"))
                            .map(String::as_str),
                        Some("true"),
                        "korunan K4 adayı eski JSON details sözleşmesini sürdürmeli"
                    );
                }
            }
        }
        other => panic!("ValidateResult::Ok beklendi, alınan: {other:?}"),
    }
}

/// The FRL family aggregates FK violations per DISTINCT ID. While aggregation wrote
/// `"Z1 (2 rows)"` into `observed_value`, K7 suppression searched for that text as if it were
/// the ID and never matched: in the 17 September counterfactual measurement, 7 feeds had
/// 20,452 Critical·Spec findings and 7 publication decisions came only from surrounding whitespace
/// (`mdb-1196` `origin_id` "ADR ").
#[test]
fn whitespace_fare_rule_fk_derivatives_are_suppressed_after_aggregation() {
    let mut files = base_files();
    files[1] = (
        "stops.txt",
        b"stop_id,stop_name,stop_lat,stop_lon,zone_id\nS1,Stop1,41.0,29.0,Z1\nS2,Stop2,41.1,29.1,Z2\n",
    );
    files.push((
        "fare_attributes.txt",
        b"fare_id,price,currency_type,payment_method,transfers\nF1,1.00,EUR,0,0\n",
    ));
    files.push((
        "fare_rules.txt",
        b"fare_id,route_id,origin_id,destination_id,contains_id\n\
          F1,R1, Z1,Z2 ,\n\
          F1,R1, Z1,Z2 ,\n\
          F1,R1,NOPE ,Z1,\n \
          F1,R1,,,\n\
          FNOPE ,R1,,,\n",
    ));

    match run(&files) {
        ValidateResult::Ok(vr) => {
            let of = |rule: &str| -> Vec<_> {
                vr.notices.iter().filter(|n| n.rule_id == rule).collect()
            };

            let frl003 = of("FRL_003");
            assert_eq!(
                frl003.len(),
                1,
                "yalnız gerçekten tanımsız zone kalmalı: {frl003:?}"
            );
            assert_eq!(frl003[0].observed_value.as_deref(), Some("NOPE "));
            assert_eq!(
                frl003[0]
                    .details
                    .as_ref()
                    .and_then(|d| d.get("affected_rows"))
                    .map(String::as_str),
                Some("1")
            );
            assert!(
                of("FRL_004").is_empty(),
                "' Z2 ' türevi bastırılmalı: {:?}",
                of("FRL_004")
            );

            let frl001 = of("FRL_001");
            assert_eq!(frl001.len(), 1, "yalnız FNOPE kalmalı: {frl001:?}");
            assert_eq!(frl001[0].observed_value.as_deref(), Some("FNOPE "));

            let root = vr
                .notices
                .iter()
                .find(|n| n.rule_id == "DQ_016" && n.file.as_deref() == Some("fare_rules.txt"))
                .expect("fare_rules.txt DQ_016 kökü");
            let rules = root
                .details
                .as_ref()
                .and_then(|d| d.get("suppressed_derivative_rules"))
                .expect("bastırılan türevler kökte beyan edilmeli");
            for rule in ["FRL_001", "FRL_003", "FRL_004"] {
                assert!(rules.contains(rule), "{rule} beyan edilmeli: {rules}");
            }
        }
        other => panic!("ValidateResult::Ok beklendi, alınan: {other:?}"),
    }
}

/// Fixed-width export pads the final column with whitespace: `end_date` = `"20271231    "`.
/// The provision (`CAL_004`) uses the raw value and suppresses it under `DQ_016`; but if
/// `None` is written to the calendar record, service is treated as "no active day" and every
/// run produces `TRP_026` (17 September partial-trimming measurement: 272,440 `TRP_026` and
/// 14,724 `OPR_011` findings came only from this). The same decision for day flags was made
/// for `CAL_006` in `2736bc79`.
#[test]
fn whitespace_padded_calendar_dates_keep_the_service_active() {
    let mut files = base_files();
    files[5] = (
        "calendar.txt",
        b"service_id,monday,tuesday,wednesday,thursday,friday,saturday,sunday,start_date,end_date\n\
          SVC1,1,1,1,1,1,0,0, 20250101,20271231    \n",
    );

    match run(&files) {
        ValidateResult::Ok(vr) => {
            for rule in ["TRP_026", "OPR_011", "RTS_016", "CAL_024"] {
                let found: Vec<_> = vr.notices.iter().filter(|n| n.rule_id == rule).collect();
                assert!(
                    found.is_empty(),
                    "{rule} boşluk türevi, çıkmamalı: {found:?}"
                );
            }
            let root = vr
                .notices
                .iter()
                .find(|n| n.rule_id == "DQ_016" && n.file.as_deref() == Some("calendar.txt"))
                .expect("calendar.txt DQ_016 kökü");
            let rules = root
                .details
                .as_ref()
                .and_then(|d| d.get("suppressed_derivative_rules"))
                .expect("tarih hükmü kökte beyan edilmeli");
            assert!(
                rules.contains("CAL_003") && rules.contains("CAL_004"),
                "{rules}"
            );
        }
        other => panic!("ValidateResult::Ok beklendi, alınan: {other:?}"),
    }

    // A date that is still invalid after trimming does NOT enter the service period and remains visible.
    files[5] = (
        "calendar.txt",
        b"service_id,monday,tuesday,wednesday,thursday,friday,saturday,sunday,start_date,end_date\n\
          SVC1,1,1,1,1,1,0,0,20250101, 2027-12-31 \n",
    );
    match run(&files) {
        ValidateResult::Ok(vr) => {
            assert!(
                vr.notices.iter().any(|n| n.rule_id == "CAL_004"),
                "gerçek biçim hatası korunmalı"
            );
            assert!(
                vr.notices.iter().any(|n| n.rule_id == "TRP_026"),
                "okunamayan end_date servisi aktif yapmamalı"
            );
        }
        other => panic!("ValidateResult::Ok beklendi, alınan: {other:?}"),
    }
}

/// A span over an untimed intermediate stop: the consecutive-pair loop `continue`s at the
/// untimed stop, and STM_061 checks only pairs over 10 km. Corpus mdb-2202: 3.5 km from
/// 15:15 → 15:15, MD reports 210 km/h while we report nothing. (Latitude 41.000 → 41.0315 ≈ 3.5 km.)
#[test]
fn speed_checks_span_untimed_intermediate_stops() {
    let stops: &[u8] = b"stop_id,stop_name,stop_lat,stop_lon\n\
        A,A,41.0000,29.0\nM1,M1,41.0100,29.0\nM2,M2,41.0200,29.0\nB,B,41.0315,29.0\n\
        C,C,41.0855,29.0\nD,D,41.0900,29.0\nE,E,41.1000,29.0\n";
    let stop_times: &[u8] = b"trip_id,arrival_time,departure_time,stop_id,stop_sequence\n\
        T1,15:15:00,15:15:00,A,1\nT1,,,M1,2\nT1,,,M2,3\nT1,15:15:00,15:15:00,B,4\n\
        T1,15:17:00,15:17:00,C,5\n\
        T1,,,D,6\nT1,15:30:00,15:30:00,E,7\n";
    let mut files = base_files();
    files[1] = ("stops.txt", stops);
    files[4] = ("stop_times.txt", stop_times);
    match run(&files) {
        ValidateResult::Ok(vr) => {
            let stm012: Vec<_> = vr.notices.iter().filter(|n| n.rule_id == "STM_012").collect();
            assert!(
                stm012.iter().any(|n| n.message.contains("1-4")),
                "A→B 3,5 km / 0 dk görülmeli: {stm012:?}"
            );
            // B→C consecutive (timed) pair: 6 km / 2 min = 180 km/h → the existing loop already sees it.
            // C→E over untimed D: 1.6 km / 13 min ≈ 7 km/h → no finding.
            let stm014: Vec<_> = vr.notices.iter().filter(|n| n.rule_id == "STM_014").collect();
            assert!(
                stm014.iter().all(|n| !n.message.contains("5-7")),
                "makul hızlı saatsiz aralık bulgu üretmemeli: {stm014:?}"
            );
        }
        other => panic!("ValidateResult::Ok beklendi, alınan: {other:?}"),
    }
}

/// If a color is padded with whitespace (`'FFFFFF    '`), the format provision uses the raw value
/// (under RTS_006/007 and `DQ_016`), but contrast is calculated from the trimmed color and remains
/// VISIBLE: yellow/white illegibility is a real finding independent of whitespace
/// (corpus mdb-1065/2653/465).
#[test]
fn rts008_measures_contrast_on_whitespace_padded_colours() {
    let mut files = base_files();
    files[2] = (
        "routes.txt",
        b"route_id,agency_id,route_short_name,route_type,route_color,route_text_color\n\
          R1,1,101,3,FECB00    ,FFFFFF    \n",
    );
    match run(&files) {
        ValidateResult::Ok(vr) => {
            let rts008: Vec<_> = vr.notices.iter().filter(|n| n.rule_id == "RTS_008").collect();
            assert_eq!(rts008.len(), 1, "sarı/beyaz ≈1,5:1 görünür olmalı: {rts008:?}");
            assert!(vr.notices.iter().any(|n| n.rule_id == "DQ_016" && n.file.as_deref() == Some("routes.txt")));
        }
        other => panic!("ValidateResult::Ok beklendi, alınan: {other:?}"),
    }
}

/// Spec: bikes_allowed *"0 or empty - No bike information for the trip."* Zero is not information;
/// it is the ABSENCE of information. TRP_021 counted it as populated (corpus mdb-1257: all ferry
/// trips were 0, MD reported `missing_bike_allowance` in 43 feeds, we reported 0). Only 1 and 2 carry information.
#[test]
fn trp021_counts_bikes_allowed_zero_as_no_information() {
    let trips = |rows: &'static [u8]| {
        let mut files = base_files();
        files[3] = ("trips.txt", rows);
        files
    };
    let trp021 = |files: &[(&str, &[u8])]| match run(files) {
        ValidateResult::Ok(vr) => vr
            .notices
            .iter()
            .find(|n| n.rule_id == "TRP_021")
            .map(|n| n.observed_value.clone().unwrap_or_default()),
        other => panic!("ValidateResult::Ok beklendi, alınan: {other:?}"),
    };
    assert_eq!(
        trp021(&trips(b"route_id,service_id,trip_id,bikes_allowed\nR1,SVC1,T1,0\n")),
        Some("1/1".to_string()),
        "hepsi 0 → bilgi yok"
    );
    assert_eq!(
        trp021(&trips(b"route_id,service_id,trip_id,bikes_allowed\nR1,SVC1,T1,0\nR1,SVC1,T2,1\n")),
        Some("1/2".to_string()),
        "0 ile 1 karışık → 0'lı sefer eksik sayılır"
    );
    assert_eq!(
        trp021(&trips(b"route_id,service_id,trip_id,bikes_allowed\nR1,SVC1,T1,1\nR1,SVC1,T2,2\n")),
        None,
        "1/2 bilgi taşır"
    );
}

/// The same decision applies to `feed_info` dates: `" 20260301"` gets its format verdict
/// (`FIN_005`/`FIN_006`) from the raw value and is suppressed under `DQ_016`, but writing `None`
/// to the record loses the validity window and makes `CAL_014` silent. MD trims the value when
/// reading it (corpus `mdb-3155`, 2026-10-13).
#[test]
fn whitespace_padded_feed_info_dates_keep_the_validity_window() {
    let mut files = base_files();
    files.push((
        "feed_info.txt",
        b"feed_publisher_name,feed_publisher_url,feed_lang,feed_start_date,feed_end_date\n\
          Test,http://test.example,en, 20250101, 20261231\n",
    ));
    match run(&files) {
        ValidateResult::Ok(vr) => {
            assert!(
                vr.notices.iter().any(|n| n.rule_id == "CAL_014"),
                "kırpılmış feed_end_date servisin taştığını göstermeli"
            );
            let root = vr
                .notices
                .iter()
                .find(|n| n.rule_id == "DQ_016" && n.file.as_deref() == Some("feed_info.txt"))
                .expect("feed_info.txt DQ_016 kökü");
            let rules = root
                .details
                .as_ref()
                .and_then(|d| d.get("suppressed_derivative_rules"))
                .expect("tarih hükmü kökte beyan edilmeli");
            assert!(rules.contains("FIN_005") && rules.contains("FIN_006"), "{rules}");
        }
        other => panic!("ValidateResult::Ok beklendi, alınan: {other:?}"),
    }
}

/// The ID join is broken only by whitespace (2b). Per #85 the ID remains RAW; what changes is
/// that the RESULT finding must be declared under the `DQ_016` root of the file carrying the whitespace.
fn join_run(
    files: &[(&str, &[u8])],
) -> (
    Vec<gtfs_core::Notice>,
    std::collections::BTreeMap<String, String>,
) {
    match run(files) {
        ValidateResult::Ok(vr) => {
            let declared = vr
                .notices
                .iter()
                .filter(|n| n.rule_id == "DQ_016")
                .filter_map(|n| {
                    let rules = n.details.as_ref()?.get("suppressed_derivative_rules")?;
                    Some((n.file.clone()?, rules.clone()))
                })
                .collect();
            (vr.notices, declared)
        }
        other => panic!("ValidateResult::Ok beklendi, alınan: {other:?}"),
    }
}

fn entities(notices: &[gtfs_core::Notice], rule: &str) -> Vec<String> {
    notices
        .iter()
        .filter(|n| n.rule_id == rule)
        .map(|n| n.entity_id.clone().unwrap_or_default())
        .collect()
}

/// `mdb-1065`: `routes.route_id = "10T0001C1  "`, while `trips.route_id` is clean → each of 113,649
/// trips received `TRP_011` for "missing route name". A genuinely unnamed route must remain visible.
#[test]
fn whitespace_join_route_id_padding_in_routes_is_declared_at_routes() {
    let mut files = base_files();
    files[2] = (
        "routes.txt",
        b"route_id,agency_id,route_short_name,route_long_name,route_type\n\
          R1  ,1,101,Main,3\nR2  ,1,,,3\n",
    );
    files[3] = (
        "trips.txt",
        b"route_id,service_id,trip_id\nR1,SVC1,T1\nR2,SVC1,T2\n",
    );
    files[4] = (
        "stop_times.txt",
        b"trip_id,arrival_time,departure_time,stop_id,stop_sequence\n\
          T1,08:00:00,08:00:00,S1,1\nT1,08:10:00,08:10:00,S2,2\n\
          T2,09:00:00,09:00:00,S1,1\nT2,09:10:00,09:10:00,S2,2\n",
    );
    let (notices, declared) = join_run(&files);
    assert_eq!(
        entities(&notices, "TRP_011"),
        vec!["T2"],
        "yalnız adsız hattın seferi kalmalı"
    );
    assert!(
        declared
            .get("routes.txt")
            .is_some_and(|r| r.contains("TRP_011")),
        "boşluğu taşıyan routes.txt kökünde beyan edilmeli: {declared:?}"
    );
}

/// `trips.service_id = " SVC1"`, calendar clean: `TRP_026`/`OPR_011`/`CAL_011` are all derivatives.
/// A calendar defined nowhere remains a real error.
#[test]
fn whitespace_join_service_id_padding_in_trips_is_declared_at_trips() {
    let mut files = base_files();
    files[3] = (
        "trips.txt",
        b"route_id,service_id,trip_id\nR1, SVC1,T1\nR1,SVC9,T2\n",
    );
    files[4] = (
        "stop_times.txt",
        b"trip_id,arrival_time,departure_time,stop_id,stop_sequence\n\
          T1,08:00:00,08:00:00,S1,1\nT1,08:10:00,08:10:00,S2,2\n\
          T2,09:00:00,09:00:00,S1,1\nT2,09:10:00,09:10:00,S2,2\n",
    );
    let (notices, declared) = join_run(&files);
    assert_eq!(
        entities(&notices, "TRP_026"),
        vec!["T2"],
        "yalnız tanımsız SVC9 kalmalı"
    );
    assert!(
        entities(&notices, "CAL_011").is_empty(),
        "SVC1 kırpınca kullanılıyor"
    );
    assert!(
        !entities(&notices, "OPR_011").iter().any(|e| e == " SVC1"),
        "' SVC1' kırpınca aktif: {:?}",
        entities(&notices, "OPR_011")
    );
    assert!(entities(&notices, "TRP_003").contains(&"T2".to_string()));
    let trips = declared.get("trips.txt").cloned().unwrap_or_default();
    for rule in ["TRP_026", "CAL_011"] {
        assert!(
            trips.contains(rule),
            "{rule} trips.txt kökünde beyan edilmeli: {declared:?}"
        );
    }
}

/// `mdb-2137`: `calendar.service_id = " 1"`, `trips.service_id = "1"` → Kritik·Spec `TRP_003`
/// per trip. The whitespace is in calendar.txt, while the finding is in trips.txt — #134's old
/// "own file" requirement could never suppress this.
#[test]
fn whitespace_join_service_id_padding_in_calendar_is_declared_at_calendar() {
    let mut files = base_files();
    files[5] = (
        "calendar.txt",
        b"service_id,monday,tuesday,wednesday,thursday,friday,saturday,sunday,start_date,end_date\n\
          \x20SVC1,1,1,1,1,1,0,0,20250101,20271231\n",
    );
    let (notices, declared) = join_run(&files);
    for rule in ["TRP_003", "TRP_026"] {
        assert!(
            entities(&notices, rule).is_empty(),
            "{rule} türev: {:?}",
            entities(&notices, rule)
        );
    }
    let cal = declared.get("calendar.txt").cloned().unwrap_or_default();
    assert!(
        cal.contains("TRP_003"),
        "calendar.txt kökünde beyan edilmeli: {declared:?}"
    );

    // `mdb-488`: the calendar is CLEAN but dateless in `calendar.txt`; `calendar_dates.txt`
    // adds date `" 6"`. The declaration belongs to the file with the WHITESPACE counterpart,
    // not to the clean definition.
    files[5] = (
        "calendar.txt",
        b"service_id,monday,tuesday,wednesday,thursday,friday,saturday,sunday,start_date,end_date\n\
          SVC1,0,0,0,0,0,0,0,20250101,20271231\n",
    );
    files.push((
        "calendar_dates.txt",
        b"service_id,date,exception_type\n SVC1,20260520,1\n",
    ));
    let (notices, declared) = join_run(&files);
    assert!(
        entities(&notices, "TRP_026").is_empty(),
        "{:?}",
        entities(&notices, "TRP_026")
    );
    assert!(
        !entities(&notices, "OPR_011").contains(&"SVC1".to_string()),
        "tanımlı ama tarihsiz SVC1'in tarihi ' SVC1'den geliyor: {:?}",
        entities(&notices, "OPR_011")
    );
    let dates = declared
        .get("calendar_dates.txt")
        .cloned()
        .unwrap_or_default();
    assert!(
        dates.contains("TRP_026"),
        "calendar_dates.txt kökünde beyan edilmeli: {declared:?}"
    );
}

/// `stop_times.stop_id = " S2"` → `S2` appears unused (`STP_020`); `trips.trip_id =
/// " T1"` → the trip record appears missing (`XFL_002`) and its shape "has no trips" (`SHP_019`);
/// `trips.shape_id = " SH2"` → `SH2` appears unused (`SHP_018`).
/// Truly unused `S3` and truly unrecorded `T3` remain visible.
#[test]
fn whitespace_join_stop_trip_and_shape_padding_is_declared_where_it_sits() {
    let mut files = base_files();
    files[1] = (
        "stops.txt",
        b"stop_id,stop_name,stop_lat,stop_lon\nS1,Stop1,41.0,29.0\nS2,Stop2,41.1,29.1\nS3,Stop3,41.2,29.2\n",
    );
    files[3] = (
        "trips.txt",
        b"route_id,service_id,trip_id,shape_id\nR1,SVC1, T1,SH1\nR1,SVC1,T2, SH2\nR1,SVC1,T3,SH3\n",
    );
    files[4] = (
        "stop_times.txt",
        b"trip_id,arrival_time,departure_time,stop_id,stop_sequence\n\
          T1,08:00:00,08:00:00,S1,1\nT1,08:10:00,08:10:00, S2,2\n\
          T2,09:00:00,09:00:00,S1,1\nT2,09:10:00,09:10:00,S1,2\n",
    );
    files.push((
        "shapes.txt",
        b"shape_id,shape_pt_lat,shape_pt_lon,shape_pt_sequence\n\
          SH1,41.0,29.0,1\nSH1,41.1,29.1,2\nSH2,41.0,29.0,1\nSH2,41.1,29.1,2\n\
          SH3,41.0,29.0,1\nSH3,41.1,29.1,2\n",
    ));
    let (notices, declared) = join_run(&files);
    assert_eq!(
        entities(&notices, "STP_020"),
        vec!["S3"],
        "yalnız gerçekten kullanılmayan S3"
    );
    assert_eq!(
        entities(&notices, "XFL_002"),
        vec!["T3"],
        "yalnız gerçekten kayıtsız T3"
    );
    assert_eq!(
        entities(&notices, "SHP_019"),
        vec!["SH3"],
        "yalnız gerçekten kayıtsız SH3"
    );
    assert!(
        entities(&notices, "SHP_018").is_empty(),
        "SH2 kırpınca kullanılıyor"
    );
    let st = declared.get("stop_times.txt").cloned().unwrap_or_default();
    let trips = declared.get("trips.txt").cloned().unwrap_or_default();
    assert!(st.contains("STP_020"), "{declared:?}");
    for rule in ["XFL_002", "SHP_019", "SHP_018"] {
        assert!(
            trips.contains(rule),
            "{rule} trips.txt kökünde: {declared:?}"
        );
    }
}

/// Route-level aggregators (2c). All are EXISTENTIAL: if trimming the combination of a SINGLE
/// trip on the route fixes it, the rule is silent in the trimmed world.
///   · `R1`: its only trip is `" T1"`, while `stop_times` uses `T1` → `XFL_012` (no recorded trip);
///     `T1` departs at 23:30 and, because the route cannot be found, `OPR_009` is emitted with
///     the TRIP ID instead of the route — this is REAL information and is not suppressed
///     (suppressing it would hide the route's night service).
///   · `R4`: its trip uses calendar `" WE"`, definition `WE` (weekend) → `RTS_016` + `OPR_004`.
/// The facts remain visible: `R3` has no trip record, `R5`'s calendar is defined nowhere,
/// and `R2` really has only weekday and night trips.
#[test]
fn whitespace_join_route_level_rules_are_declared_where_it_sits() {
    let mut files = base_files();
    files[2] = (
        "routes.txt",
        b"route_id,agency_id,route_short_name,route_type\n\
          R1,1,101,3\nR2,1,102,3\nR3,1,103,3\nR4,1,104,3\nR5,1,105,3\n",
    );
    files[3] = (
        "trips.txt",
        b"route_id,service_id,trip_id\n\
          R1,SVC1, T1\nR2,SVC1,T2\nR3,SVC1,T3\nR4, WE,T4\nR5,NONE,T5\n",
    );
    files[4] = (
        "stop_times.txt",
        b"trip_id,arrival_time,departure_time,stop_id,stop_sequence\n\
          T1,23:30:00,23:30:00,S1,1\nT1,23:40:00,23:40:00,S2,2\n\
          T2,23:30:00,23:30:00,S1,1\nT2,23:40:00,23:40:00,S2,2\n\
          T4,08:00:00,08:00:00,S1,1\nT4,08:10:00,08:10:00,S2,2\n\
          T5,08:00:00,08:00:00,S1,1\nT5,08:10:00,08:10:00,S2,2\n",
    );
    files[5] = (
        "calendar.txt",
        b"service_id,monday,tuesday,wednesday,thursday,friday,saturday,sunday,start_date,end_date\n\
          SVC1,1,1,1,1,1,0,0,20250101,20271231\n\
          WE,0,0,0,0,0,1,1,20250101,20271231\n",
    );
    let (notices, declared) = join_run(&files);
    assert_eq!(
        entities(&notices, "XFL_012"),
        vec!["R3"],
        "yalnız gerçekten kayıtsız R3"
    );
    assert_eq!(
        entities(&notices, "RTS_016"),
        vec!["R5"],
        "yalnız takvimi tanımsız R5"
    );
    assert!(
        !entities(&notices, "OPR_004").contains(&"R4".to_string()),
        "R4 kırpınca hafta sonu çalışıyor: {:?}",
        entities(&notices, "OPR_004")
    );
    assert!(
        entities(&notices, "OPR_004").contains(&"R2".to_string()),
        "R2 gerçekten hafta içi"
    );
    let opr009 = entities(&notices, "OPR_009");
    assert!(opr009.contains(&"R2".to_string()), "{opr009:?}");
    assert!(
        opr009.contains(&"T1".to_string()),
        "sefer kimlikli gece özeti gerçek bilgi, görünür kalmalı: {opr009:?}"
    );
    let trips = declared.get("trips.txt").cloned().unwrap_or_default();
    for rule in ["XFL_012", "RTS_016", "OPR_004"] {
        assert!(
            trips.contains(rule),
            "{rule} trips.txt kökünde: {declared:?}"
        );
    }
}

/// `mdb-1065`: all 243 routes are rail and their `route_id`s are padded. When the route cannot
/// be found, K6 falls back to bus `route_type` (`unwrap_or(3)`): a 167 km/h train becomes
/// "too fast" (`STM_014`, `OPR_008`), and a 410-minute gap becomes a "gap" (`OPR_001`).
/// Findings below the trimmed route's rail thresholds (300 km/h, 720 min) are derivatives;
/// those exceeding the rail thresholds are real and remain visible.
#[test]
fn whitespace_join_lost_route_type_keeps_only_findings_above_the_rail_threshold() {
    let mut files = base_files();
    files[1] = (
        "stops.txt",
        b"stop_id,stop_name,stop_lat,stop_lon\n\
          S1,A,41.00,29.0\nS2,B,41.10,29.0\nS3,C,41.20,29.0\nS4,D,41.30,29.0\nS5,E,41.40,29.0\n",
    );
    files[2] = (
        "routes.txt",
        b"route_id,agency_id,route_short_name,route_type\nR1  ,1,IC1,2\nR2  ,1,IC2,2\n",
    );
    files[3] = (
        "trips.txt",
        b"route_id,service_id,trip_id\n\
          R1,SVC1,T1\nR1,SVC1,T2\nR1,SVC1,T3\nR1,SVC1,T9\nR2,SVC1,T5\nR2,SVC1,T6\nR2,SVC1,T7\n",
    );
    files[4] = (
        "stop_times.txt",
        b"trip_id,arrival_time,departure_time,stop_id,stop_sequence\n\
          T1,06:00:00,06:00:00,S1,1\nT1,06:04:00,06:04:00,S2,2\nT1,06:08:00,06:08:00,S3,3\n\
          T2,06:10:00,06:10:00,S3,1\nT2,06:11:30,06:11:30,S4,2\nT2,06:13:00,06:13:00,S5,3\n\
          T3,13:00:00,13:00:00,S1,1\nT3,13:10:00,13:10:00,S2,2\n\
          T9,06:20:00,06:20:00,S3,1\nT9,06:24:00,06:24:00,S4,2\n\
          T5,06:00:00,06:00:00,S1,1\nT5,06:10:00,06:10:00,S2,2\n\
          T6,06:10:00,06:10:00,S1,1\nT6,06:20:00,06:20:00,S2,2\n\
          T7,19:00:00,19:00:00,S1,1\nT7,19:10:00,19:10:00,S2,2\n",
    );
    let (notices, declared) = join_run(&files);
    let stm014 = entities(&notices, "STM_014");
    assert!(
        !stm014
            .iter()
            .any(|e| e.contains("S1→S2") || e.contains("S2→S3")),
        "167 km/h tren ray eşiğinin altında: {stm014:?}"
    );
    // T9 passes S3→S4 at 167 km/h (derivative) and T2 at 444 km/h (real): the segment remains.
    assert!(
        stm014.iter().any(|e| e.contains("S3→S4")),
        "444 km/h gerçek: {stm014:?}"
    );
    assert_eq!(
        entities(&notices, "OPR_008"),
        vec!["T2"],
        "yalnız ray eşiğini aşan sefer"
    );
    let opr001 = entities(&notices, "OPR_001");
    assert!(
        !opr001.contains(&"R1".to_string()),
        "410 dk ray için normal: {opr001:?}"
    );
    assert!(
        opr001.contains(&"R2".to_string()),
        "770 dk ray eşiğini de aşıyor: {opr001:?}"
    );
    let routes = declared.get("routes.txt").cloned().unwrap_or_default();
    for rule in ["STM_014", "OPR_008", "OPR_001"] {
        assert!(
            routes.contains(rule),
            "{rule} routes.txt kökünde: {declared:?}"
        );
    }
}

/// `mdb-1309`/`tfs-109`: all 340 trips had `block_id = " "`. Treating one space as a block ID
/// assigned every trip to the SAME vehicle and produced 286 `TRP_022` "overlapping block" findings.
/// #85: IDs are compared raw BUT a whitespace-only value DOES NOT EXIST — the optional field is empty.
/// A real block conflict remains visible.
#[test]
fn whitespace_only_optional_trip_ids_are_absent() {
    let mut files = base_files();
    files[3] = (
        "trips.txt",
        b"route_id,service_id,trip_id,block_id,shape_id\n\
          R1,SVC1,T1, , \nR1,SVC1,T2, , \nR1,SVC1,T3,B1,\nR1,SVC1,T4,B1,\n",
    );
    files[4] = (
        "stop_times.txt",
        b"trip_id,arrival_time,departure_time,stop_id,stop_sequence\n\
          T1,08:00:00,08:00:00,S1,1\nT1,09:00:00,09:00:00,S2,2\n\
          T2,08:30:00,08:30:00,S1,1\nT2,09:30:00,09:30:00,S2,2\n\
          T3,10:00:00,10:00:00,S1,1\nT3,11:00:00,11:00:00,S2,2\n\
          T4,10:30:00,10:30:00,S1,1\nT4,11:30:00,11:30:00,S2,2\n",
    );
    match run(&files) {
        ValidateResult::Ok(vr) => {
            let trp022: Vec<_> = vr
                .notices
                .iter()
                .filter(|n| n.rule_id == "TRP_022")
                .map(|n| n.observed_value.clone().unwrap_or_default())
                .collect();
            assert_eq!(trp022.len(), 1, "yalnız gerçek B1 bloğu: {trp022:?}");
            assert!(trp022[0].contains("T3"), "{trp022:?}");
            for rule in ["TRP_004", "SHP_019"] {
                let found: Vec<_> = vr.notices.iter().filter(|n| n.rule_id == rule).collect();
                assert!(
                    found.is_empty(),
                    "' ' şekil kimliği yok sayılmalı: {found:?}"
                );
            }
            assert!(
                vr.notices
                    .iter()
                    .any(|n| n.rule_id == "DQ_016" && n.file.as_deref() == Some("trips.txt")),
                "boşluk DQ_016 ile raporlanmaya devam etmeli"
            );
        }
        other => panic!("ValidateResult::Ok beklendi, alınan: {other:?}"),
    }
}

#[test]
fn whitespace112_pathway_semantics_survive_when_trimmed_value_is_invalid() {
    let mut files = base_files();
    files.push((
        "pathways.txt",
        b"pathway_id,from_stop_id,to_stop_id,pathway_mode,is_bidirectional,length,traversal_time,min_width,max_slope\n\
          P1,S1,S2, 9,0,1,30,1\n\
          P2,S1,S2,1, 9,1,30,1\n\
          P3,S1,S2,1,0, -5,30,1\n\
          P4,S1,S2,3,0,1, 0,1\n\
          P5,S1,S2,1,0,1,30, 0\n\
          P6,S1,S2, 3,0,1,30,1\n\
          P7,S1,S2, 7, 1,1,30,1,\n" as &[u8],
    ));

    match run(&files) {
        ValidateResult::Ok(vr) => {
            assert!(
                has(&vr, "DQ_016"),
                "pathways.txt whitespace kök bulgusu korunmalı"
            );
            for rule in ["PTH_004", "PTH_005", "PTH_006", "PTH_007", "PTH_010"] {
                assert_eq!(
                    vr.notices.iter().filter(|n| n.rule_id == rule).count(),
                    1,
                    "{rule} bağımsız semantik hata olarak korunmalı"
                );
            }
            assert_eq!(
                vr.notices.iter().filter(|n| n.rule_id == "PTH_004").count(),
                1,
                "pathway_mode= 3 yalnızca DQ_016 türevi olmalı; PTH_004'e dönüşmemeli"
            );
            assert_eq!(
                vr.notices.iter().filter(|n| n.rule_id == "PTH_016").count(),
                1,
                "trim sonrası geçerli ama birlikte yasak pathway değerleri korunmalı"
            );
        }
        other => panic!("ValidateResult::Ok beklendi, alınan: {other:?}"),
    }
}

/// #126: the `suppressed_derivative_*` counter describes the READER MODE, not the feed —
/// `trips.txt` is read with `get_col`, which trims it; no derivative arises there and no key
/// is written. The missing key could be read as "nothing was hidden here".
/// `typed_whitespace_fields` is produced in both modes and names where whitespace hid a typed value.
#[test]
fn dq016_reports_typed_whitespace_in_both_reader_regimes() {
    let mut files = base_files();
    for slot in files.iter_mut() {
        // RowMap path: read raw, derivative arises, K7 suppresses it.
        // `route_long_name` also contains whitespace but is TEXT: trimming it does not make it a number,
        // so it must not enter the list — otherwise the key would mean "every whitespace field".
        if slot.0 == "routes.txt" {
            slot.1 = b"route_id,agency_id,route_short_name,route_long_name,route_type\n\
                       R1,1,101, Ana Hat , 3\n" as &[u8];
        }
        // get_col path: trimmed, so NO derivative arises.
        if slot.0 == "trips.txt" {
            slot.1 =
                b"route_id,service_id,trip_id,wheelchair_accessible\nR1,SV1,T1, 1 \n" as &[u8];
        }
    }

    match run(&files) {
        ValidateResult::Ok(vr) => {
            let details = |file: &str| {
                vr.notices
                    .iter()
                    .find(|n| n.rule_id == "DQ_016" && n.file.as_deref() == Some(file))
                    .unwrap_or_else(|| panic!("{file} için DQ_016 kök bulgusu"))
                    .details
                    .as_ref()
                    .unwrap_or_else(|| panic!("{file} DQ_016 kanıt taşımalı"))
                    .clone()
            };

            let routes = details("routes.txt");
            let trips = details("trips.txt");

            // The asymmetry still EXISTS and is correct: the derivative arises only on the raw-reading path.
            assert!(
                routes.contains_key("suppressed_derivative_rules"),
                "ham okuyan yolda bastırılan türev kaydedilmeli: {routes:?}"
            );
            assert!(
                !trips.contains_key("suppressed_derivative_rules"),
                "trim eden yolda bastırılacak türev yok: {trips:?}"
            );

            // The evidence now exists in BOTH modes — this prevents misreading the asymmetry.
            assert_eq!(
                routes.get("typed_whitespace_fields").map(String::as_str),
                Some("route_type"),
                "{routes:?}"
            );
            assert_eq!(
                trips.get("typed_whitespace_fields").map(String::as_str),
                Some("wheelchair_accessible"),
                "{trips:?}"
            );
        }
        other => panic!("ValidateResult::Ok beklendi, alınan: {other:?}"),
    }
}

/// Anchors the BASIS for the K7 branch removed in #125. That branch assumed whitespace would
/// appear in the observed value of a parse rejection on the stream path; but `stop_times`/`shapes`
/// emitters read the value with `get_col`, which applies `.trim()`, so such a notice never arises.
/// If `get_col` ever stops trimming, this test must fail and the removal decision must be revisited
/// — it must not silently become wrong.
#[test]
fn streaming_files_never_report_a_value_with_surrounding_whitespace() {
    let mut files = base_files();
    for slot in files.iter_mut() {
        if slot.0 == "stop_times.txt" {
            slot.1 =
                b"trip_id,arrival_time,departure_time,stop_id,stop_sequence,shape_dist_traveled\n\
                       T1, 08:00:00 ,08:00:00,S1, 1 , 0.0 \n\
                       T1,08:10:00, 08:10:00 ,S2,2,100.0\n" as &[u8];
        }
    }
    files.push((
        "shapes.txt",
        b"shape_id,shape_pt_lat,shape_pt_lon,shape_pt_sequence\n\
          SH1, 41.0 ,29.0, 1 \n\
          SH1,41.1, 29.1 ,2\n" as &[u8],
    ));

    match run(&files) {
        ValidateResult::Ok(vr) => {
            assert!(has(&vr, "DQ_016"), "boşluk kök bulgusu görünür kalmalı");
            for n in vr.notices.iter().filter(|n| {
                matches!(
                    n.file.as_deref(),
                    Some("stop_times.txt") | Some("shapes.txt")
                )
            }) {
                let Some(observed) = n.observed_value.as_deref() else {
                    continue;
                };
                assert_eq!(
                    observed,
                    observed.trim(),
                    "{} akış yolunda çevresi boşluklu değer raporladı: {observed:?}",
                    n.rule_id
                );
            }
        }
        other => panic!("ValidateResult::Ok beklendi, alınan: {other:?}"),
    }
}

// ── issue #84: unclosed quote in a stream file BODY ────────────────────────────
//
// K1 never opens the bodies of these four files; the branch leading to K1's body scanner was DEAD
// (removed), and K2 readers silently tolerated an unclosed quote. A single stray quote could therefore
// swallow the rest of the file and the validator would say NOTHING.
// ⚠️ The finding is CRITICAL but NOT FATAL — rationale is in the `common::arc013_unclosed_stream` doc.

fn stream_quote_feed(stop_times: &'static str) -> Vec<(&'static str, &'static [u8])> {
    let mut files = base_files();
    for slot in files.iter_mut() {
        if slot.0 == "stop_times.txt" {
            slot.1 = stop_times.as_bytes();
        }
    }
    files
}

#[test]
fn arc013_stream_body_unclosed_quote_is_reported() {
    assert!(
        match run(&stream_quote_feed(
            "trip_id,arrival_time,departure_time,stop_id,stop_sequence,stop_headsign\n\
         T1,08:00:00,08:00:00,S1,1,\"Kapanmamis\n\
         T1,08:10:00,08:10:00,S2,2,Normal\n"
        )) {
            ValidateResult::Ok(vr) => has(&vr, "ARC_013"),
            other => panic!("Ok beklendi: {other:?}"),
        },
        "akış gövdesindeki kapanmamış tırnak ARC_013 üretmeli"
    );
}

#[test]
fn arc013_stream_silent_on_valid_quoted_comma() {
    assert!(
        match run(&stream_quote_feed(
            "trip_id,arrival_time,departure_time,stop_id,stop_sequence,stop_headsign\n\
         T1,08:00:00,08:00:00,S1,1,\"Merkez, Kuzey\"\n\
         T1,08:10:00,08:10:00,S2,2,Normal\n"
        )) {
            ValidateResult::Ok(vr) => !has(&vr, "ARC_013") && !has(&vr, "ARC_033"),
            other => panic!("Ok beklendi: {other:?}"),
        },
        "tırnaklanmış virgül GEÇERLİDİR"
    );
}

#[test]
fn arc013_stream_silent_on_valid_doubled_quote() {
    assert!(
        match run(&stream_quote_feed(
            "trip_id,arrival_time,departure_time,stop_id,stop_sequence,stop_headsign\n\
         T1,08:00:00,08:00:00,S1,1,\"5\"\" Hat\"\n\
         T1,08:10:00,08:10:00,S2,2,Normal\n"
        )) {
            ValidateResult::Ok(vr) => !has(&vr, "ARC_013") && !has(&vr, "ARC_033"),
            other => panic!("Ok beklendi: {other:?}"),
        },
        "doğru kaçırılmış tırnak GEÇERLİDİR"
    );
}

#[test]
fn arc013_stream_silent_when_quoted_final_field_ends_at_eof() {
    static SHAPES_WITH_QUOTED_EOF: &[u8] =
        b"shape_id,shape_pt_lat,shape_pt_lon,shape_pt_sequence\n\
          \"S1\",\"41.0\",\"29.0\",\"1\"";
    let mut files = base_files();
    files.push(("shapes.txt", SHAPES_WITH_QUOTED_EOF));

    match run(&files) {
        ValidateResult::Ok(vr) => {
            assert!(
                !vr.notices.iter().any(|n| n.rule_id == "ARC_013" && n.file.as_deref() == Some("shapes.txt")),
                "kapanış tırnağından sonra EOF geçerli CSV olduğundan shapes.txt ARC_013 üretmemeli"
            );
            assert!(
                !vr.notices
                    .iter()
                    .any(|n| n.rule_id == "ARC_033" && n.file.as_deref() == Some("shapes.txt")),
                "doğru kapanan/kaçırılan tırnaklar shapes.txt ARC_033 üretmemeli"
            );
        }
        other => panic!("ValidateResult::Ok beklendi, alınan: {other:?}"),
    }
}

#[test]
fn arc013_stream_reports_unclosed_final_quoted_field() {
    static SHAPES_WITH_UNCLOSED_EOF: &[u8] =
        b"shape_id,shape_pt_lat,shape_pt_lon,shape_pt_sequence\n\
          \"S1\",\"41.0\",\"29.0\",\"1";
    let mut files = base_files();
    files.push(("shapes.txt", SHAPES_WITH_UNCLOSED_EOF));

    match run(&files) {
        ValidateResult::Ok(vr) => assert!(
            vr.notices
                .iter()
                .any(|n| n.rule_id == "ARC_013" && n.file.as_deref() == Some("shapes.txt")),
            "kapanış tırnağı olmadan EOF shapes.txt ARC_013 üretmeli"
        ),
        other => panic!("ValidateResult::Ok beklendi, alınan: {other:?}"),
    }
}

#[test]
fn arc013_stream_bare_quote_is_arc033_not_arc013() {
    // The two cases are DISTINCT: a quote-escaping violation does not corrupt data (ARC_033),
    // while an unclosed quote swallows the rest of the file (ARC_013). Reporting them through
    // the same channel would conflate them.
    match run(&stream_quote_feed(
        "trip_id,arrival_time,departure_time,stop_id,stop_sequence,stop_headsign\n\
         T1,08:00:00,08:00:00,S1,1,5\" Hat\n\
         T1,08:10:00,08:10:00,S2,2,Normal\n",
    )) {
        ValidateResult::Ok(vr) => {
            assert!(has(&vr, "ARC_033"), "çıplak tırnak ARC_033");
            assert!(
                !has(&vr, "ARC_013"),
                "çıplak tırnak tokenization HATASI değildir"
            );
        }
        other => panic!("Ok beklendi: {other:?}"),
    }
}

// ── issue #92: STRICT type predicates must see the RAW value ──────────────────
//
// `#85` made IDs raw; this is the corresponding change for type predicates. `" https://example.com "`
// carries unescaped whitespace in raw form — the Spec says "special characters must be escaped correctly" —
// but the predicate saw a trimmed proxy and the strict rule stayed SILENT. Only the `DQ_016`
// quality signal remained, so a Spec provision (`P5f72fb5a` KANITLI) was being asserted over a normalized value.

fn agency_url_feed(agency: &'static str) -> Vec<(&'static str, &'static [u8])> {
    let mut files = base_files();
    for slot in files.iter_mut() {
        if slot.0 == "agency.txt" {
            slot.1 = agency.as_bytes();
        }
    }
    files
}

#[test]
fn url92_padded_url_does_not_satisfy_the_hard_predicate() {
    assert!(
        match run(&agency_url_feed(
            "agency_id,agency_name,agency_url,agency_timezone\n1,T,\" https://example.com \",UTC\n"
        )) {
            ValidateResult::Ok(vr) => has(&vr, "AGN_003"),
            other => panic!("Ok beklendi: {other:?}"),
        },
        "kırpılınca geçerli olan URL sert predikatı KARŞILAMAMALI"
    );
}

#[test]
fn url92_clean_url_stays_valid() {
    assert!(
        match run(&agency_url_feed(
            "agency_id,agency_name,agency_url,agency_timezone\n1,T,https://example.com,UTC\n"
        )) {
            ValidateResult::Ok(vr) => !has(&vr, "AGN_003"),
            other => panic!("Ok beklendi: {other:?}"),
        },
        "zaten kaçırılmış URL geçerli kalmalı"
    );
}

#[test]
fn url92_whitespace_only_url_is_missing_not_invalid() {
    // 🔴 BOUNDARY: `"   "` is a MISSING field, not an "invalid URL". Presence diagnosis trims,
    // while validity sees the raw value — reducing this to one read would erase the distinction.
    match run(&agency_url_feed(
        "agency_id,agency_name,agency_url,agency_timezone\n1,T,\"   \",UTC\n",
    )) {
        ValidateResult::Ok(vr) => {
            let msg = vr
                .notices
                .iter()
                .find(|n| n.rule_id == "AGN_003")
                .map(|n| n.message.clone())
                .unwrap_or_default();
            assert!(
                msg.contains("zorunludur"),
                "boşluk-yalnız değer EKSİK alan olarak bildirilmeli, gelen: {msg}"
            );
        }
        other => panic!("Ok beklendi: {other:?}"),
    }
}

// ── issue #94: SHP_005 exact monotonicity comparison ───────────────────────────
// The old predicate was `d < prev - 1e-6`: a REAL decrease smaller than 1e-6 was silently
// accepted. This test passes through CSV — only the K5 unit test shows that decimal→f64 parsing
// preserves the producer's order.

fn shp005_feed(shapes: &'static str) -> Vec<(&'static str, &'static [u8])> {
    let mut files = base_files();
    files.push(("shapes.txt", shapes.as_bytes()));
    files
}

#[test]
fn shp005_detects_decrease_smaller_than_old_epsilon() {
    // Exact counterexample from issue #94: decrease of 5e-7.
    match run(&shp005_feed(
        "shape_id,shape_pt_lat,shape_pt_lon,shape_pt_sequence,shape_dist_traveled\n\
        S1,41.0,29.0,1,1.0000005\nS1,41.1,29.1,2,1.0000000\n",
    )) {
        ValidateResult::Ok(vr) => assert!(
            has(&vr, "SHP_005"),
            "1e-6'dan küçük azalma SHP_005 üretmeli (#94)"
        ),
        other => panic!("Ok beklendi: {other:?}"),
    }
}

#[test]
fn shp005_silent_on_tiny_increase_from_csv() {
    // Same number of decimal places, reverse direction → silent (removing tolerance must not create an FP).
    match run(&shp005_feed(
        "shape_id,shape_pt_lat,shape_pt_lon,shape_pt_sequence,shape_dist_traveled\n\
        S1,41.0,29.0,1,1.0000000\nS1,41.1,29.1,2,1.0000005\n",
    )) {
        ValidateResult::Ok(vr) => assert!(
            !has(&vr, "SHP_005"),
            "artan shape_dist_traveled SHP_005 üretmemeli"
        ),
        other => panic!("Ok beklendi: {other:?}"),
    }
}

#[test]
fn shp005_silent_on_equal_values_from_csv() {
    // Equality is a SEPARATE case (#94): the rule is only for `previous > current`.
    match run(&shp005_feed(
        "shape_id,shape_pt_lat,shape_pt_lon,shape_pt_sequence,shape_dist_traveled\n\
        S1,41.0,29.0,1,1.0000000\nS1,41.1,29.1,2,1.0000000\n",
    )) {
        ValidateResult::Ok(vr) => assert!(
            !has(&vr, "SHP_005"),
            "eşit shape_dist_traveled SHP_005 üretmemeli"
        ),
        other => panic!("Ok beklendi: {other:?}"),
    }
}

/// When ATR_010/011/012 deduped as `Entity`, every row with an empty `attribution_id` used the same
/// dedup key, so any number of broken references in a feed collapsed to ONE finding. The Spec makes
/// `attributions.attribution_id` **Optional**, so an identity-free file is valid — basing dedup on
/// an optional field silently suppressed the rule. Measurement (`tdg-80245`): 26 dangling `trip_id`
/// → 1 finding; MobilityData reported all 26 in the same feed with `foreign_key_violation`.
#[test]
fn attribution_fk_violations_are_reported_per_row_without_attribution_id() {
    const ATTRIBUTIONS: &[u8] = b"organization_name,is_producer,trip_id\n\
          Org,1,T8\nOrg,1,T9\n";

    let mut files = base_files();
    files.push(("attributions.txt", ATTRIBUTIONS));
    match run(&files) {
        ValidateResult::Ok(vr) => {
            let hits: Vec<_> = vr
                .notices
                .iter()
                .filter(|n| n.rule_id == "ATR_012")
                .collect();
            assert_eq!(
                hits.len(),
                2,
                "kimlik yokken her bozuk trip_id ayrı raporlanmalı: {hits:?}"
            );
            let seen: Vec<&str> = hits
                .iter()
                .filter_map(|n| n.observed_value.as_deref())
                .collect();
            assert!(
                seen.contains(&"T8") && seen.contains(&"T9"),
                "iki farklı trip_id de görünmeli: {seen:?}"
            );
        }
        other => panic!("ValidateResult::Ok beklendi: {other:?}"),
    }
}

/// When ATR_001/ATR_003 deduped as `Entity`, an identity-free file collapsed to one finding and
/// the message did not say how many records were affected. Both violations arise from ONE systemic
/// cause (the column is never written, or role columns are never populated), so per-row emission
/// repeats the same sentence: measurement across 124 feeds meant 391,871 notices for ATR_001 and
/// 6,472 for ATR_003. One file-level summary plus a count in `observed_value` prevents repetition
/// while preserving the affected-record count (FAR_013/DQ_016 pattern).
#[test]
fn attribution_systemic_violations_are_summarized_once_with_a_count() {
    const ATTRIBUTIONS: &[u8] = b"organization_name\nOrg A\nOrg B\nOrg C\n";

    let mut files = base_files();
    files.push(("attributions.txt", ATTRIBUTIONS));
    match run(&files) {
        ValidateResult::Ok(vr) => {
            for rule in ["ATR_001", "ATR_003"] {
                let hits: Vec<_> = vr.notices.iter().filter(|n| n.rule_id == rule).collect();
                assert_eq!(
                    hits.len(),
                    1,
                    "{rule}: dosya başına tek özet beklenir: {hits:?}"
                );
                assert_eq!(
                    hits[0].observed_value.as_deref(),
                    Some("3 rows"),
                    "{rule}: etkilenen kayıt sayısı observed_value'da taşınmalı",
                );
                assert!(
                    hits[0].message.contains('3'),
                    "{rule}: mesaj sayıyı içermeli: {}",
                    hits[0].message,
                );
            }
        }
        other => panic!("ValidateResult::Ok beklendi: {other:?}"),
    }
}

/// ATR_009 belongs to the same class as ATR_001/ATR_003: measured violations equaled the ROW
/// count in every feed (`tdg-82555` 12,240/12,240, `tld-4477` 3,379/3,379), meaning the producer
/// wrote the same two reference fields together on every row. `Entity` dedup reduced this to
/// one per ID, yielding 93 notices in 5 feeds, none saying how many rows were affected.
/// One file-level summary plus a count both avoids repetition and preserves volume.
#[test]
fn attribution_mutually_exclusive_refs_are_summarized_once_with_a_count() {
    const ATTRIBUTIONS: &[u8] =
        b"attribution_id,organization_name,is_producer,agency_id,route_id\n\
          A1,Org A,1,A1,R1\nA2,Org B,1,A1,R1\n";

    let mut files = base_files();
    files.push(("attributions.txt", ATTRIBUTIONS));
    match run(&files) {
        ValidateResult::Ok(vr) => {
            let hits: Vec<_> = vr
                .notices
                .iter()
                .filter(|n| n.rule_id == "ATR_009")
                .collect();
            assert_eq!(hits.len(), 1, "dosya başına tek özet beklenir: {hits:?}");
            assert_eq!(
                hits[0].observed_value.as_deref(),
                Some("2 rows"),
                "etkilenen kayıt sayısı observed_value'da taşınmalı",
            );
        }
        other => panic!("ValidateResult::Ok beklendi: {other:?}"),
    }
}

#[test]
fn japanese_text_signal_opens_jp_validation_when_metadata_is_wrong() {
    let mut files = base_files();
    files[0] = (
        "agency.txt",
        b"agency_id,agency_name,agency_url,agency_timezone,agency_lang\n1,\xE6\x9D\xB1\xE4\xBA\xAC\xE3\x83\x90\xE3\x82\xB9,http://test.example,Europe/London,en\n",
    );
    files[1] = (
        "stops.txt",
        b"stop_id,stop_name,stop_lat,stop_lon\nS1,\xE6\x9D\xB1\xE4\xBA\xAC\xE3\x81\x88\xE3\x81\x8D,41.0,29.0\nS2,London,41.1,29.1\n",
    );
    files.push((
        "feed_info.txt",
        b"feed_publisher_name,feed_publisher_url,feed_lang\nTest,http://test.example,en\n",
    ));
    files.push((
        "fare_attributes.txt",
        b"fare_id,price,currency_type,payment_method,transfers\nF1,1.00,USD,0,0\n",
    ));

    match run_with_profile(&files, GtfsJpProfile::V4) {
        ValidateResult::Ok(vr) => {
            assert!(
                vr.metrics.is_gtfs_jp,
                "Japonca kana metni, bozuk JP metadata'sından bağımsız tespiti açmalı"
            );
            for rule in ["JPN_023", "JPN_024", "JPN_025", "JPN_026"] {
                assert!(has(&vr, rule), "{rule} Japonca metin sinyaliyle çalışmalı");
            }
        }
        other => panic!("ValidateResult::Ok beklendi, alınan: {other:?}"),
    }
}

#[test]
fn stop_name_kana_signal_opens_jp_validation_when_agency_metadata_is_wrong() {
    let mut files = base_files();
    files[0] = (
        "agency.txt",
        b"agency_id,agency_name,agency_url,agency_timezone,agency_lang\n1,London Bus,http://test.example,Europe/London,en\n",
    );
    files[1] = (
        "stops.txt",
        b"stop_id,stop_name,stop_lat,stop_lon\nS1,\xE6\x9D\xb1\xE4\xbA\xac\xE3\x81\x88\xE3\x81\x8D,41.0,29.0\nS2,London,41.1,29.1\n",
    );
    files.push((
        "feed_info.txt",
        b"feed_publisher_name,feed_publisher_url,feed_lang\nTest,http://test.example,en\n",
    ));
    files.push((
        "fare_attributes.txt",
        b"fare_id,price,currency_type,payment_method,transfers\nF1,1.00,USD,0,0\n",
    ));

    match run_with_profile(&files, GtfsJpProfile::V4) {
        ValidateResult::Ok(vr) => {
            assert!(
                vr.metrics.is_gtfs_jp,
                "stop_name kana, independent of agency metadata, should open JP validation"
            );
            for rule in ["JPN_023", "JPN_024", "JPN_025", "JPN_026"] {
                assert!(has(&vr, rule), "{rule} should run from stop_name kana");
            }
        }
        other => panic!("ValidateResult::Ok beklendi, alınan: {other:?}"),
    }
}

#[test]
fn kanji_only_text_does_not_open_jp_validation() {
    let mut files = base_files();
    files[0] = (
        "agency.txt",
        b"agency_id,agency_name,agency_url,agency_timezone,agency_lang\n1,\xE8\x87\xBA\xE5\x8C\x97\xE5\xB8\x82,http://test.example,UTC,en\n",
    );
    files[1] = (
        "stops.txt",
        b"stop_id,stop_name,stop_lat,stop_lon\nS1,\xE8\x87\xBA\xE5\x8C\x97\xE8\xBB\x8A\xE7\xAB\x99,41.0,29.0\nS2,Central Station,41.1,29.1\n",
    );
    files.push((
        "feed_info.txt",
        b"feed_publisher_name,feed_publisher_url,feed_lang\nTest,http://test.example,en\n",
    ));
    files.push((
        "fare_attributes.txt",
        b"fare_id,price,currency_type,payment_method,transfers\nF1,1.00,USD,0,0\n",
    ));

    match run_with_profile(&files, GtfsJpProfile::V4) {
        ValidateResult::Ok(vr) => {
            assert!(
                !vr.metrics.is_gtfs_jp,
                "Kanji tek başına JP sinyali sayılmamalı"
            );
            for rule in ["JPN_023", "JPN_024", "JPN_025", "JPN_026"] {
                assert!(!has(&vr, rule), "{rule} yalnız Kanji metinle çalışmamalı");
            }
        }
        other => panic!("ValidateResult::Ok beklendi, alınan: {other:?}"),
    }
}

/// Goshogawara demand-responsive taxi pattern (2026-09-19): within one stop group, pickup-only
/// and drop-off-only rows share the same window. The overlap forbidden by the Spec exists when
/// the ROLE also overlaps; a trip without a fixed route is not subject to the shape rule.
#[test]
fn zone_only_flex_trip_is_not_an_overlap_or_shape_gap() {
    fn flex_files(stop_times: &'static [u8]) -> Vec<(&'static str, &'static [u8])> {
        let mut files = base_files();
        files.push((
            "location_groups.txt",
            b"location_group_id,location_group_name\nC1,Area\n",
        ));
        files.push((
            "location_group_stops.txt",
            b"location_group_id,stop_id\nC1,S1\nC1,S2\n",
        ));
        files.push((
            "booking_rules.txt",
            b"booking_rule_id,booking_type,prior_notice_duration_min\nBR1,1,30\n",
        ));
        files[4] = ("stop_times.txt", stop_times);
        files
    }
    let split_roles = flex_files(
        b"trip_id,stop_sequence,location_group_id,pickup_type,drop_off_type,start_pickup_drop_off_window,end_pickup_drop_off_window,pickup_booking_rule_id,drop_off_booking_rule_id\n\
          T1,1,C1,2,1,09:00:00,18:00:00,BR1,BR1\n\
          T1,2,C1,1,2,09:00:00,18:00:00,BR1,BR1\n",
    );
    match run(&split_roles) {
        ValidateResult::Ok(vr) => {
            for rule in ["PDW_006", "DQ_006", "RTS_017"] {
                assert!(!has(&vr, rule), "zone-only Flex sefer {rule} üretmemeli");
            }
        }
        other => panic!("ValidateResult::Ok beklendi, alınan: {other:?}"),
    }

    // Proof of the gate: when both rows allow pickup, the overlap is real.
    let same_role = flex_files(
        b"trip_id,stop_sequence,location_group_id,pickup_type,drop_off_type,start_pickup_drop_off_window,end_pickup_drop_off_window,pickup_booking_rule_id,drop_off_booking_rule_id\n\
          T1,1,C1,2,2,09:00:00,18:00:00,BR1,BR1\n\
          T1,2,C1,2,2,09:00:00,18:00:00,BR1,BR1\n",
    );
    match run(&same_role) {
        ValidateResult::Ok(vr) => assert!(has(&vr, "PDW_006"), "aynı rolde örtüşme PDW_006 üretmeli"),
        other => panic!("ValidateResult::Ok beklendi, alınan: {other:?}"),
    }
}

#[test]
fn feed_summary_carries_trimmed_agency_and_feed_info_fields() {
    let mut files = base_files();
    files.push((
        "feed_info.txt",
        b"feed_publisher_name,feed_publisher_url,feed_lang,feed_version,feed_contact_email\n Pub ,https://pub.example,en,,ops@pub.example\n",
    ));
    let vr = match run(&files) {
        ValidateResult::Ok(vr) => vr,
        other => panic!("ValidateResult::Ok beklendi, alınan: {other:?}"),
    };
    let s = &vr.feed_summary;
    assert_eq!(s.agency_count, 1);
    assert_eq!(s.agencies[0].agency_id.as_deref(), Some("1"));
    assert_eq!(s.agencies[0].agency_name, "Test");
    assert_eq!(s.agencies[0].agency_url.as_deref(), Some("http://test.example"));
    assert_eq!(s.agencies[0].agency_phone, None);
    let fi = s.feed_info.as_ref().expect("feed_info özeti olmalı");
    assert_eq!(fi.feed_publisher_name.as_deref(), Some("Pub"));
    assert_eq!(fi.feed_version, None, "boş alan None olmalı");
    assert_eq!(fi.feed_contact_email.as_deref(), Some("ops@pub.example"));

    let vr = match run(&base_files()) {
        ValidateResult::Ok(vr) => vr,
        other => panic!("ValidateResult::Ok beklendi, alınan: {other:?}"),
    };
    assert!(vr.feed_summary.feed_info.is_none(), "feed_info.txt yoksa özet de yok");
}

#[test]
fn cal_014_reports_future_only_service_like_mobilitydata() {
    // MD `service_window_outside_feed_period` checks only the active window; a service entirely
    // in the future is not an exception. CAL_014 behaves the same because it is Interop.
    let mut files = base_files();
    files.retain(|(name, _)| *name != "calendar.txt");
    files.push((
        "calendar.txt",
        b"service_id,monday,tuesday,wednesday,thursday,friday,saturday,sunday,start_date,end_date\n\
          SVC1,1,1,1,1,1,1,1,20300101,20300630\n",
    ));
    files.push((
        "feed_info.txt",
        b"feed_publisher_name,feed_publisher_url,feed_lang,feed_start_date,feed_end_date\n\
          Pub,https://pub.example,en,20300101,20300331\n",
    ));
    let vr = match run(&files) {
        ValidateResult::Ok(vr) => vr,
        other => panic!("ValidateResult::Ok beklendi, alınan: {other:?}"),
    };
    let n = vr
        .notices
        .iter()
        .find(|n| n.rule_id == "CAL_014")
        .expect("gelecekteki servis feed döneminin dışına taşıyor → CAL_014");
    let d = n.details.as_ref().unwrap();
    assert_eq!(d.get("days_before").map(String::as_str), Some("0"));
    assert_eq!(d.get("days_after").map(String::as_str), Some("91"));
}

#[test]
fn invalid_timepoint_summary_carries_row_examples_and_values() {
    // STM_022 stays one feed summary; it now carries up to 50 row examples (one per trip
    // before a second row from the same trip) with the stop and its times, plus the
    // distribution of the invalid values that were written.
    static TRIPS2: &[u8] = b"route_id,service_id,trip_id\nR1,SVC1,T1\nR1,SVC1,T2\n";
    static TIMEPOINTS: &[u8] =
        b"trip_id,arrival_time,departure_time,stop_id,stop_sequence,timepoint\n\
          T1,08:00:00,08:00:00,S1,1,2\nT1,08:10:00,08:10:00,S2,2,2\n\
          T2,09:00:00,09:00:00,S1,1,abc\nT2,09:10:00,09:10:00,S2,2,1\n";
    let mut files = base_files();
    files[3] = ("trips.txt", TRIPS2);
    files[4] = ("stop_times.txt", TIMEPOINTS);
    let ValidateResult::Ok(vr) = run(&files) else {
        panic!("ValidateResult::Ok beklendi");
    };
    let stm022: Vec<_> = vr
        .notices
        .iter()
        .filter(|n| n.rule_id == "STM_022")
        .collect();
    assert_eq!(stm022.len(), 1, "STM_022 tek feed özeti olmalı");
    assert_eq!(stm022[0].observed_value.as_deref(), Some("3"));
    let d = stm022[0].details.as_ref().expect("STM_022 özet ayrıntısı");
    let get = |k: &str| d.get(k).map(String::as_str);
    assert_eq!(get("affected_rows"), Some("3"));
    // Round robin: T1's first row, T2's row, then T1's second row.
    assert_eq!(get("example_0_trip"), Some("T1"));
    assert_eq!(get("example_0_stop_id"), Some("S1"));
    assert_eq!(get("example_0_stop_sequence"), Some("1"));
    assert_eq!(get("example_0_arrival"), Some("08:00:00"));
    assert_eq!(get("example_0_observed"), Some("2"));
    assert_eq!(get("example_1_trip"), Some("T2"));
    assert_eq!(get("example_1_observed"), Some("abc"));
    assert_eq!(get("example_2_trip"), Some("T1"));
    assert_eq!(get("example_2_stop_id"), Some("S2"));
    assert_eq!(get("example_3_trip"), None);
    assert_eq!(get("value_0"), Some("2"));
    assert_eq!(get("value_0_count"), Some("2"));
    assert_eq!(get("value_1"), Some("abc"));
    assert_eq!(get("value_1_count"), Some("1"));
}

#[test]
fn invalid_timepoint_summary_caps_examples_at_fifty() {
    let mut trips = String::from("route_id,service_id,trip_id\n");
    let mut times =
        String::from("trip_id,arrival_time,departure_time,stop_id,stop_sequence,timepoint\n");
    for i in 0..60 {
        trips.push_str(&format!("R1,SVC1,T{i:02}\n"));
        times.push_str(&format!(
            "T{i:02},08:00:00,08:00:00,S1,1,7\nT{i:02},08:10:00,08:10:00,S2,2,1\n"
        ));
    }
    let trips: &'static [u8] = Box::leak(trips.into_bytes().into_boxed_slice());
    let times: &'static [u8] = Box::leak(times.into_bytes().into_boxed_slice());
    let mut files = base_files();
    files[3] = ("trips.txt", trips);
    files[4] = ("stop_times.txt", times);
    let ValidateResult::Ok(vr) = run(&files) else {
        panic!("ValidateResult::Ok beklendi");
    };
    let n = vr
        .notices
        .iter()
        .find(|n| n.rule_id == "STM_022")
        .expect("STM_022");
    let d = n.details.as_ref().expect("ayrıntı");
    assert_eq!(d.get("affected_rows").map(String::as_str), Some("60"));
    assert!(d.get("example_49_trip").is_some());
    assert!(d.get("example_50_trip").is_none(), "en fazla 50 örnek");
}

#[test]
fn unit_mismatch_on_one_shape_is_a_trip_finding_with_the_drawn_length() {
    // shapes.txt in metres, stop_times in kilometres: ratio 0.001. The finding carries the drawn
    // shape length in metres so the planner card can show which side uses the other unit.
    static TRIPS1: &[u8] = b"route_id,service_id,trip_id,shape_id\nR1,SVC1,T1,SH1\n";
    static SHAPES1: &[u8] =
        b"shape_id,shape_pt_lat,shape_pt_lon,shape_pt_sequence,shape_dist_traveled\n\
          SH1,41.0,29.0,1,0\nSH1,41.1,29.1,2,13900\n";
    static TIMES1: &[u8] =
        b"trip_id,arrival_time,departure_time,stop_id,stop_sequence,shape_dist_traveled\n\
          T1,08:00:00,08:00:00,S1,1,0\nT1,08:10:00,08:10:00,S2,2,13.9\n";
    let mut files = base_files();
    files[3] = ("trips.txt", TRIPS1);
    files[4] = ("stop_times.txt", TIMES1);
    files.push(("shapes.txt", SHAPES1));
    let ValidateResult::Ok(vr) = run(&files) else {
        panic!("ValidateResult::Ok beklendi");
    };
    let found: Vec<_> = vr
        .notices
        .iter()
        .filter(|n| n.rule_id == "STM_024")
        .collect();
    assert_eq!(found.len(), 1);
    let n = found[0];
    assert_eq!(n.entity_type, gtfs_core::EntityType::Trip);
    assert_eq!(n.entity_id.as_deref(), Some("T1"));
    assert_eq!(n.observed_value.as_deref(), Some("13.90"));
    assert_eq!(n.expected_value.as_deref(), Some("13900.00"));
    let d = n.details.as_ref().expect("STM_024 ayrıntısı");
    let get = |k: &str| d.get(k).map(String::as_str);
    assert_eq!(get("shape_id"), Some("SH1"));
    assert_eq!(get("stop_times_max"), Some("13.90"));
    assert_eq!(get("shape_max"), Some("13900.00"));
    assert_eq!(get("stop_id"), Some("S2"));
    assert_eq!(get("stop_sequence"), Some("2"));
    assert_eq!(get("affected_trips"), Some("1"));
    let length: f64 = get("shape_length_m")
        .expect("çizim uzunluğu")
        .parse()
        .unwrap();
    assert!((13_000.0..15_000.0).contains(&length), "{length}");
}

#[test]
fn unit_mismatch_on_several_shapes_is_one_feed_summary_with_shape_examples() {
    // Two flagged shapes used by three trips. Before, Row-level dedup (no line) silently kept one
    // trip finding; now the feed gets one summary that names every shape.
    static TRIPS3: &[u8] =
        b"route_id,service_id,trip_id,shape_id\nR1,SVC1,T1,SH1\nR1,SVC1,T2,SH2\nR1,SVC1,T3,SH1\n";
    static SHAPES2: &[u8] =
        b"shape_id,shape_pt_lat,shape_pt_lon,shape_pt_sequence,shape_dist_traveled\n\
          SH1,41.0,29.0,1,0\nSH1,41.1,29.1,2,13900\n\
          SH2,41.1,29.1,1,0\nSH2,41.0,29.0,2,13900\n";
    static TIMES3: &[u8] =
        b"trip_id,arrival_time,departure_time,stop_id,stop_sequence,shape_dist_traveled\n\
          T1,08:00:00,08:00:00,S1,1,0\nT1,08:10:00,08:10:00,S2,2,13.9\n\
          T2,09:00:00,09:00:00,S2,1,0\nT2,09:10:00,09:10:00,S1,2,13.9\n\
          T3,10:00:00,10:00:00,S1,1,0\nT3,10:10:00,10:10:00,S2,2,13.9\n";
    let mut files = base_files();
    files[3] = ("trips.txt", TRIPS3);
    files[4] = ("stop_times.txt", TIMES3);
    files.push(("shapes.txt", SHAPES2));
    let ValidateResult::Ok(vr) = run(&files) else {
        panic!("ValidateResult::Ok beklendi");
    };
    let found: Vec<_> = vr
        .notices
        .iter()
        .filter(|n| n.rule_id == "STM_024")
        .collect();
    assert_eq!(found.len(), 1, "birden çok shape tek feed özeti olmalı");
    let n = found[0];
    assert_eq!(n.entity_type, gtfs_core::EntityType::Feed);
    assert_eq!(n.observed_value.as_deref(), Some("2"));
    let d = n.details.as_ref().expect("özet ayrıntısı");
    let get = |k: &str| d.get(k).map(String::as_str);
    assert_eq!(get("message_variant"), Some("feed_summary"));
    assert_eq!(get("affected_shapes"), Some("2"));
    assert_eq!(get("affected_trips"), Some("3"));
    assert_eq!(get("example_0_shape_id"), Some("SH1"));
    assert_eq!(get("example_0_trip"), Some("T1"));
    assert_eq!(get("example_1_shape_id"), Some("SH2"));
    assert_eq!(get("example_1_trip"), Some("T2"));
    assert_eq!(get("example_1_stop_id"), Some("S1"));
    assert!(get("example_1_shape_length_m").is_some());
    assert_eq!(get("example_2_trip"), None);
}

#[test]
fn same_place_stop_pair_is_one_finding_across_trips_and_directions() {
    // mdb-853 (FlixBus): one stop pair at one coordinate gave 18 findings, one per monthly trip
    // copy and direction. The record to fix is the pair, so A→B and B→A on any number of trips
    // give one finding that lists the trips.
    static STOPS5: &[u8] = b"stop_id,stop_name,stop_lat,stop_lon\n\
          S0,Start,40.9,28.9\nS1,Ferry,41.0,29.0\nS2,Terminal,41.0,29.0\nS3,End,41.1,29.1\n\
          S4,Other1,42.0,30.0\nS5,Other2,42.0,30.0\n";
    static TRIPS4: &[u8] = b"route_id,service_id,trip_id\n\
          R1,SVC1,T1\nR1,SVC1,T2\nR1,SVC1,T3\nR1,SVC1,T4\n";
    static TIMES4: &[u8] = b"trip_id,arrival_time,departure_time,stop_id,stop_sequence\n\
          T1,08:00:00,08:00:00,S0,1\nT1,08:10:00,08:10:00,S1,2\nT1,08:11:00,08:11:00,S2,3\nT1,08:20:00,08:20:00,S3,4\n\
          T2,09:00:00,09:00:00,S0,1\nT2,09:10:00,09:10:00,S1,2\nT2,09:11:00,09:11:00,S2,3\nT2,09:20:00,09:20:00,S3,4\n\
          T3,10:00:00,10:00:00,S3,1\nT3,10:10:00,10:10:00,S2,2\nT3,10:11:00,10:11:00,S1,3\nT3,10:20:00,10:20:00,S0,4\n\
          T4,11:00:00,11:00:00,S4,1\nT4,11:10:00,11:10:00,S5,2\n";
    let mut files = base_files();
    files[1] = ("stops.txt", STOPS5);
    files[3] = ("trips.txt", TRIPS4);
    files[4] = ("stop_times.txt", TIMES4);
    let ValidateResult::Ok(vr) = run(&files) else {
        panic!("ValidateResult::Ok beklendi");
    };
    let found: Vec<_> = vr
        .notices
        .iter()
        .filter(|n| n.rule_id == "STM_021")
        .collect();
    assert_eq!(found.len(), 2, "iki farklı durak çifti, iki bulgu");
    let ferry = found
        .iter()
        .find(|n| n.entity_id.as_deref() == Some("T1"))
        .expect("Ferry/Terminal çifti T1 ile temsil edilmeli");
    let d = ferry.details.as_ref().expect("ayrıntı");
    let get = |k: &str| d.get(k).map(String::as_str);
    assert_eq!(get("affected_trips"), Some("3"));
    assert_eq!(
        get("prev_stop_id"),
        Some("S0"),
        "temsilci kendi satır bağlamını korur"
    );
    assert_eq!(get("example_0_trip"), Some("T1"));
    assert_eq!(get("example_1_trip"), Some("T2"));
    assert_eq!(get("example_2_trip"), Some("T3"));
    assert_eq!(
        get("example_2_stop_a"),
        Some("S2"),
        "ters yön kendi sırasıyla listelenir"
    );
    assert_eq!(get("example_2_stop_a_sequence"), Some("2"));
    assert_eq!(get("example_3_trip"), None);
    assert!(ferry.message.contains("3 seferde"));
    let other = found
        .iter()
        .find(|n| n.entity_id.as_deref() == Some("T4"))
        .expect("ikinci çift");
    assert_eq!(
        other
            .details
            .as_ref()
            .and_then(|d| d.get("affected_trips"))
            .map(String::as_str),
        Some("1")
    );
}

#[test]
fn short_time_and_long_distance_segments_carry_their_stops_and_measures() {
    // STM_025 (under 10 s) and STM_026 (over 50 km straight line) carry the segment context the
    // planner card lists: both stops' sequences and times, the neighbours, and the measured
    // duration or distance. One finding per trip segment, as before.
    static STOPS_FAR: &[u8] = b"stop_id,stop_name,stop_lat,stop_lon\n\
          S1,Near1,41.0,29.0\nS2,Near2,41.0005,29.0\nS3,Far,42.0,29.0\n";
    static TIMES_FAR: &[u8] = b"trip_id,arrival_time,departure_time,stop_id,stop_sequence\n\
          T1,08:00:00,08:00:00,S1,1\nT1,08:00:05,08:00:05,S2,2\nT1,09:30:00,09:30:00,S3,3\n";
    let mut files = base_files();
    files[1] = ("stops.txt", STOPS_FAR);
    files[4] = ("stop_times.txt", TIMES_FAR);
    let ValidateResult::Ok(vr) = run(&files) else {
        panic!("ValidateResult::Ok beklendi");
    };
    let details = |rule: &str| {
        vr.notices
            .iter()
            .find(|n| n.rule_id == rule)
            .and_then(|n| n.details.clone())
            .unwrap_or_else(|| panic!("{rule} ayrıntısı bekleniyor"))
    };
    let d = details("STM_025");
    let get = |k: &str| d.get(k).cloned();
    assert_eq!(get("duration_seconds").as_deref(), Some("5"));
    assert_eq!(get("distance_meters").as_deref(), Some("56"));
    assert_eq!(get("stop_a_sequence").as_deref(), Some("1"));
    assert_eq!(get("stop_b_arrival").as_deref(), Some("08:00:05"));
    assert_eq!(get("next_stop_id").as_deref(), Some("S3"));

    let d = details("STM_026");
    let get = |k: &str| d.get(k).cloned();
    assert_eq!(get("threshold_km").as_deref(), Some("50"));
    assert_eq!(get("stop_a_sequence").as_deref(), Some("2"));
    assert_eq!(get("prev_stop_id").as_deref(), Some("S1"));
    let meters: f64 = get("distance_meters").expect("mesafe").parse().unwrap();
    assert!((100_000.0..120_000.0).contains(&meters), "{meters}");
}

#[test]
fn trip_duration_and_invalid_distance_findings_carry_planner_context() {
    // STM_028 names the longest step between timed stops (a day rolled over by mistake),
    // STM_029 the stop count and the straight-line distance between the end stops, and STM_030
    // the stop with the invalid cumulative distance between its neighbours' values.
    static STOPS3: &[u8] = b"stop_id,stop_name,stop_lat,stop_lon\n\
          S1,Stop1,41.0,29.0\nS2,Stop2,41.1,29.1\nS3,Stop3,41.2,29.2\n";
    let details = |files: &[(&str, &[u8])], rule: &str| match run(files) {
        ValidateResult::Ok(vr) => vr
            .notices
            .iter()
            .find(|n| n.rule_id == rule)
            .and_then(|n| n.details.clone())
            .unwrap_or_else(|| panic!("{rule} ayrıntısı bekleniyor")),
        other => panic!("ValidateResult::Ok beklendi, gelen: {other:?}"),
    };
    let mut files = base_files();
    files[1] = ("stops.txt", STOPS3);

    static LONG: &[u8] = b"trip_id,arrival_time,departure_time,stop_id,stop_sequence\n\
          T1,08:00:00,08:00:00,S1,1\nT1,08:30:00,08:30:00,S2,2\nT1,33:00:00,33:00:00,S3,3\n";
    files[4] = ("stop_times.txt", LONG);
    let d = details(&files, "STM_028");
    let get = |k: &str| d.get(k).cloned();
    assert_eq!(get("gap_stop_a").as_deref(), Some("S2"));
    assert_eq!(get("gap_stop_b").as_deref(), Some("S3"));
    assert_eq!(get("gap_from").as_deref(), Some("08:30:00"));
    assert_eq!(get("gap_to").as_deref(), Some("33:00:00"));
    assert_eq!(get("gap_seconds").as_deref(), Some("88200"));
    assert_eq!(get("first_stop_sequence").as_deref(), Some("1"));
    assert_eq!(get("stop_count").as_deref(), Some("3"));

    static SHORT: &[u8] = b"trip_id,arrival_time,departure_time,stop_id,stop_sequence\n\
          T1,08:00:00,08:00:00,S1,1\nT1,08:00:00,08:00:00,S2,2\nT1,08:00:30,08:00:30,S3,3\n";
    files[4] = ("stop_times.txt", SHORT);
    let d = details(&files, "STM_029");
    let get = |k: &str| d.get(k).cloned();
    assert_eq!(get("stop_count").as_deref(), Some("3"));
    assert_eq!(get("duration_seconds").as_deref(), Some("30"));
    assert_eq!(get("last_stop_sequence").as_deref(), Some("3"));
    let meters: f64 = get("distance_meters").expect("mesafe").parse().unwrap();
    assert!((25_000.0..30_000.0).contains(&meters), "{meters}");

    static NEGATIVE: &[u8] =
        b"trip_id,arrival_time,departure_time,stop_id,stop_sequence,shape_dist_traveled\n\
          T1,08:00:00,08:00:00,S1,1,0\nT1,08:10:00,08:10:00,S2,2,-5\nT1,08:20:00,08:20:00,S3,3,10\n";
    files[4] = ("stop_times.txt", NEGATIVE);
    let d = details(&files, "STM_030");
    let get = |k: &str| d.get(k).cloned();
    assert_eq!(get("stop_id").as_deref(), Some("S2"));
    assert_eq!(get("stop_sequence").as_deref(), Some("2"));
    assert_eq!(get("arrival").as_deref(), Some("08:10:00"));
    assert_eq!(get("prev_stop_id").as_deref(), Some("S1"));
    assert_eq!(get("prev_shape_dist_traveled").as_deref(), Some("0"));
    assert_eq!(get("next_stop_id").as_deref(), Some("S3"));
    assert_eq!(get("next_shape_dist_traveled").as_deref(), Some("10"));
}

#[test]
fn stop_time_findings_032_to_040_carry_planner_context() {
    // STM_032 names the two rows sharing a sequence, STM_033 the trip's only stop, STM_034 the
    // row's stop and times, STM_035 the repeated stop's segment, STM_037–040 the Flex zone, its
    // window and booking rules. Counts are unchanged; only details are added.
    let details = |files: &[(&str, &[u8])], rule: &str| match run(files) {
        ValidateResult::Ok(vr) => vr
            .notices
            .iter()
            .find(|n| n.rule_id == rule)
            .and_then(|n| n.details.clone())
            .unwrap_or_else(|| panic!("{rule} ayrıntısı bekleniyor")),
        other => panic!("ValidateResult::Ok beklendi, gelen: {other:?}"),
    };
    let mut files = base_files();

    static DUP_SEQ: &[u8] = b"trip_id,arrival_time,departure_time,stop_id,stop_sequence\n\
          T1,08:00:00,08:00:00,S1,1\nT1,08:10:00,08:10:00,S2,1\n";
    files[4] = ("stop_times.txt", DUP_SEQ);
    let d = details(&files, "STM_032");
    assert_eq!(d.get("first_stop_id").map(String::as_str), Some("S1"));
    assert_eq!(d.get("second_stop_id").map(String::as_str), Some("S2"));
    assert_eq!(
        d.get("second_arrival").map(String::as_str),
        Some("08:10:00")
    );

    static ONE_STOP: &[u8] = b"trip_id,arrival_time,departure_time,stop_id,stop_sequence\n\
          T1,08:00:00,08:00:00,S2,7\n";
    files[4] = ("stop_times.txt", ONE_STOP);
    let d = details(&files, "STM_033");
    assert_eq!(d.get("stop_id").map(String::as_str), Some("S2"));
    assert_eq!(d.get("stop_sequence").map(String::as_str), Some("7"));
    assert_eq!(d.get("departure").map(String::as_str), Some("08:00:00"));

    static HALF_TIME: &[u8] = b"trip_id,arrival_time,departure_time,stop_id,stop_sequence\n\
          T1,08:00:00,08:00:00,S1,1\nT1,08:10:00,,S2,2\nT1,08:20:00,08:20:00,S1,3\n";
    files[4] = ("stop_times.txt", HALF_TIME);
    let d = details(&files, "STM_034");
    assert_eq!(d.get("stop_id").map(String::as_str), Some("S2"));
    assert_eq!(d.get("stop_sequence").map(String::as_str), Some("2"));
    assert_eq!(d.get("arrival").map(String::as_str), Some("08:10:00"));

    static REPEAT: &[u8] = b"trip_id,arrival_time,departure_time,stop_id,stop_sequence\n\
          T1,08:00:00,08:00:00,S1,1\nT1,08:10:00,08:10:00,S2,2\nT1,08:15:00,08:15:00,S2,3\n";
    files[4] = ("stop_times.txt", REPEAT);
    let d = details(&files, "STM_035");
    assert_eq!(d.get("stop_id").map(String::as_str), Some("S2"));
    assert_eq!(d.get("stop_a_sequence").map(String::as_str), Some("2"));
    assert_eq!(d.get("stop_b_sequence").map(String::as_str), Some("3"));
    assert_eq!(
        d.get("stop_b_arrival").map(String::as_str),
        Some("08:15:00")
    );
    assert_eq!(d.get("prev_stop_id").map(String::as_str), Some("S1"));

    static FLEX: &[u8] = b"trip_id,stop_id,location_id,stop_sequence,arrival_time,departure_time,\
start_pickup_drop_off_window,end_pickup_drop_off_window,pickup_booking_rule_id,drop_off_booking_rule_id\n\
          T1,,Z1,1,08:00:00,08:00:00,10:00:00,09:00:00,,\n\
          T1,,Z2,2,,,11:00:00,,BR1,\n";
    files[4] = ("stop_times.txt", FLEX);
    for rule in ["STM_037", "STM_038", "STM_040"] {
        let d = details(&files, rule);
        assert_eq!(
            d.get("location_id").map(String::as_str),
            Some("Z1"),
            "{rule}"
        );
        assert_eq!(
            d.get("window_start").map(String::as_str),
            Some("10:00:00"),
            "{rule}"
        );
        assert_eq!(
            d.get("window_end").map(String::as_str),
            Some("09:00:00"),
            "{rule}"
        );
        assert_eq!(
            d.get("stop_sequence").map(String::as_str),
            Some("1"),
            "{rule}"
        );
    }
    assert_eq!(
        details(&files, "STM_037")
            .get("arrival")
            .map(String::as_str),
        Some("08:00:00")
    );
    let d = details(&files, "STM_039");
    assert_eq!(d.get("location_id").map(String::as_str), Some("Z2"));
    assert_eq!(d.get("window_start").map(String::as_str), Some("11:00:00"));
    assert_eq!(d.get("window_end"), None);
    assert_eq!(
        d.get("pickup_booking_rule_id").map(String::as_str),
        Some("BR1")
    );
}

#[test]
fn repeated_stop_is_one_finding_per_route_and_stop() {
    // STM_035: S2 twice in a row on two trips of R1 and one trip of R2. The record to fix is each
    // route's stop pattern, so R1 and R2 give one finding each; R1's lists both its trips.
    static ROUTES2: &[u8] =
        b"route_id,agency_id,route_short_name,route_type\nR1,1,101,3\nR2,1,102,3\n";
    static TRIPS3: &[u8] = b"route_id,service_id,trip_id\nR1,SVC1,T1\nR1,SVC1,T2\nR2,SVC1,T3\n";
    static TIMES3: &[u8] = b"trip_id,arrival_time,departure_time,stop_id,stop_sequence\n\
          T1,08:00:00,08:00:00,S1,1\nT1,08:10:00,08:10:00,S2,2\nT1,08:15:00,08:15:00,S2,3\n\
          T2,09:00:00,09:00:00,S1,1\nT2,09:10:00,09:10:00,S2,2\nT2,09:15:00,09:15:00,S2,3\n\
          T3,10:00:00,10:00:00,S1,1\nT3,10:10:00,10:10:00,S2,2\nT3,10:15:00,10:15:00,S2,3\n";
    let mut files = base_files();
    files[2] = ("routes.txt", ROUTES2);
    files[3] = ("trips.txt", TRIPS3);
    files[4] = ("stop_times.txt", TIMES3);
    let ValidateResult::Ok(vr) = run(&files) else {
        panic!("ValidateResult::Ok beklendi");
    };
    let found: Vec<_> = vr
        .notices
        .iter()
        .filter(|n| n.rule_id == "STM_035")
        .collect();
    assert_eq!(found.len(), 2, "hat + durak başına bir bulgu");
    let r1 = found
        .iter()
        .find(|n| n.entity_id.as_deref() == Some("T1"))
        .expect("R1 bulgusu T1 ile temsil edilmeli");
    let d = r1.details.as_ref().expect("ayrıntı");
    let get = |k: &str| d.get(k).map(String::as_str);
    assert_eq!(get("route_id"), Some("R1"));
    assert_eq!(get("affected_trips"), Some("2"));
    assert_eq!(get("example_0_trip"), Some("T1"));
    assert_eq!(get("example_1_trip"), Some("T2"));
    assert_eq!(get("example_1_stop_b_arrival"), Some("09:15:00"));
    assert_eq!(get("example_2_trip"), None);
    assert!(r1.message.contains("2 seferinde"));
    let r2 = found
        .iter()
        .find(|n| n.entity_id.as_deref() == Some("T3"))
        .expect("R2 bulgusu");
    assert_eq!(
        r2.details
            .as_ref()
            .and_then(|d| d.get("affected_trips"))
            .map(String::as_str),
        Some("1")
    );
}

#[test]
fn unsorted_stop_times_summary_names_each_trips_case_and_place() {
    // STM_036 stays one feed summary. T1 writes a stop after a later one (sequence 2 then 1),
    // T2's rows are spread over the file (T1's rows sit between them): each example says which.
    static TRIPS2: &[u8] = b"route_id,service_id,trip_id\nR1,SVC1,T1\nR1,SVC1,T2\n";
    static UNSORTED: &[u8] = b"trip_id,arrival_time,departure_time,stop_id,stop_sequence\n\
          T2,07:00:00,07:00:00,S1,1\n\
          T1,08:10:00,08:10:00,S2,2\nT1,08:00:00,08:00:00,S1,1\n\
          T2,07:10:00,07:10:00,S2,2\n";
    let mut files = base_files();
    files[3] = ("trips.txt", TRIPS2);
    files[4] = ("stop_times.txt", UNSORTED);
    let ValidateResult::Ok(vr) = run(&files) else {
        panic!("ValidateResult::Ok beklendi");
    };
    let found: Vec<_> = vr
        .notices
        .iter()
        .filter(|n| n.rule_id == "STM_036")
        .collect();
    assert_eq!(found.len(), 1, "STM_036 feed başına tek özet");
    let d = found[0].details.as_ref().expect("özet ayrıntısı");
    let get = |k: &str| d.get(k).map(String::as_str);
    assert_eq!(get("affected_trips"), Some("2"));
    assert_eq!(get("order_trips"), Some("1"));
    assert_eq!(get("scattered_trips"), Some("1"));
    assert_eq!(get("example_0_trip"), Some("T1"));
    assert_eq!(get("example_0_kind"), Some("order"));
    assert_eq!(get("example_0_curr_stop"), Some("S1"));
    assert_eq!(get("example_0_prev_stop"), Some("S2"));
    assert_eq!(get("example_1_trip"), Some("T2"));
    assert_eq!(get("example_1_kind"), Some("scattered"));
    assert_eq!(get("example_1_line_from"), Some("2"));
    assert_eq!(get("example_1_line_to"), Some("5"));
}

#[test]
fn stm017_ignores_on_demand_zone_rows_but_keeps_missing_fixed_stops() {
    // mdb-2447: fixed stops alternate with on-demand zone rows (location_id + window, no
    // stop_id). Every fixed stop carries a distance; the zone rows carry none, as they must
    // (MobilityData `forbidden_shape_dist_traveled`). That trip has no STM_017. A trip whose
    // FIXED stop lacks a distance still has one, counted over fixed stops only.
    static TRIPS2: &[u8] =
        b"route_id,service_id,trip_id,shape_id\nR1,SVC1,T1,SH1\nR1,SVC1,T2,SH1\n";
    static SHAPES1: &[u8] =
        b"shape_id,shape_pt_lat,shape_pt_lon,shape_pt_sequence,shape_dist_traveled\n\
          SH1,41.0,29.0,1,0\nSH1,41.1,29.1,2,13900\n";
    static TIMES_FLEX: &[u8] =
        b"trip_id,arrival_time,departure_time,stop_id,location_id,stop_sequence,\
start_pickup_drop_off_window,end_pickup_drop_off_window,shape_dist_traveled\n\
          T1,08:00:00,08:00:00,S1,,1,,,0\n\
          T1,,,,Z1,2,08:00:00,08:10:00,\n\
          T1,08:10:00,08:10:00,S2,,3,,,13900\n\
          T2,09:00:00,09:00:00,S1,,1,,,0\n\
          T2,,,,Z1,2,09:00:00,09:10:00,\n\
          T2,09:10:00,09:10:00,S2,,3,,,\n";
    let mut files = base_files();
    files[3] = ("trips.txt", TRIPS2);
    files[4] = ("stop_times.txt", TIMES_FLEX);
    files.push(("shapes.txt", SHAPES1));
    let ValidateResult::Ok(vr) = run(&files) else {
        panic!("ValidateResult::Ok beklendi");
    };
    let found: Vec<_> = vr
        .notices
        .iter()
        .filter(|n| n.rule_id == "STM_017")
        .collect();
    assert_eq!(
        found.len(),
        1,
        "yalnız sabit durağı eksik olan T2: {found:?}"
    );
    assert_eq!(found[0].entity_id.as_deref(), Some("T2"));
    let d = found[0].details.as_ref().expect("ayrıntı");
    let get = |k: &str| d.get(k).map(String::as_str);
    assert_eq!(get("stop_count"), Some("2"), "bölge satırı durak sayılmaz");
    assert_eq!(get("affected_stop_count"), Some("1"));
    assert_eq!(get("missing_0_stop_id"), Some("S2"));
    assert_eq!(get("missing_1_stop_id"), None);
}

#[test]
fn stop_time_findings_041_to_047_carry_planner_context() {
    // STM_041 names the row's stop and zone, STM_042 the stop and the headsign text, STM_043 the
    // trip's ends and distinct stops, STM_046 the stop and times of the row without a trip, and
    // STM_047 the rows of its trip summary. Counts are unchanged; only details are added.
    let details = |files: &[(&str, &[u8])], rule: &str| match run(files) {
        ValidateResult::Ok(vr) => vr
            .notices
            .iter()
            .find(|n| n.rule_id == rule)
            .and_then(|n| n.details.clone())
            .unwrap_or_else(|| panic!("{rule} ayrıntısı bekleniyor")),
        other => panic!("ValidateResult::Ok beklendi, gelen: {other:?}"),
    };
    let mut files = base_files();
    let get = |d: &gtfs_core::NoticeDetails, key: &str| d.get(key).map(|v| v.to_string());

    static BOTH: &[u8] = b"trip_id,stop_id,location_id,stop_sequence,arrival_time,departure_time,\
start_pickup_drop_off_window,end_pickup_drop_off_window\n\
          T1,S1,Z1,1,08:00:00,08:00:00,,\n";
    files[4] = ("stop_times.txt", BOTH);
    let d = details(&files, "STM_041");
    assert_eq!(get(&d, "stop_id").as_deref(), Some("S1"));
    assert_eq!(get(&d, "location_id").as_deref(), Some("Z1"));
    assert_eq!(get(&d, "stop_sequence").as_deref(), Some("1"));

    static HEADSIGN: &[u8] =
        b"trip_id,arrival_time,departure_time,stop_id,stop_sequence,stop_headsign\n\
          T1,08:00:00,08:00:00,S1,1,Centre*\nT1,08:10:00,08:10:00,S2,2,\n";
    files[4] = ("stop_times.txt", HEADSIGN);
    let d = details(&files, "STM_042");
    assert_eq!(get(&d, "stop_id").as_deref(), Some("S1"));
    assert_eq!(get(&d, "stop_headsign").as_deref(), Some("Centre*"));
    assert_eq!(get(&d, "departure").as_deref(), Some("08:00:00"));

    let mut long = b"trip_id,arrival_time,departure_time,stop_id,stop_sequence\n".to_vec();
    for i in 0..201u32 {
        let (h, m) = (6 + i / 60, i % 60);
        let stop = if i % 2 == 0 { "S1" } else { "S2" };
        long.extend_from_slice(
            format!("T1,{h:02}:{m:02}:00,{h:02}:{m:02}:00,{stop},{}\n", i + 1).as_bytes(),
        );
    }
    files[4] = ("stop_times.txt", &long);
    let d = details(&files, "STM_043");
    assert_eq!(get(&d, "first_stop_id").as_deref(), Some("S1"));
    assert_eq!(get(&d, "first_departure").as_deref(), Some("06:00:00"));
    assert_eq!(get(&d, "last_stop_sequence").as_deref(), Some("201"));
    assert_eq!(get(&d, "last_arrival").as_deref(), Some("09:20:00"));
    assert_eq!(get(&d, "stop_count").as_deref(), Some("201"));
    assert_eq!(get(&d, "distinct_stops").as_deref(), Some("2"));

    static NO_TRIP: &[u8] = b"trip_id,arrival_time,departure_time,stop_id,stop_sequence\n\
          T1,08:00:00,08:00:00,S1,1\nT1,08:10:00,08:10:00,S2,2\n,08:20:00,08:21:00,S3,4\n";
    files[4] = ("stop_times.txt", NO_TRIP);
    let d = details(&files, "STM_046");
    assert_eq!(get(&d, "stop_id").as_deref(), Some("S3"));
    assert_eq!(get(&d, "stop_sequence").as_deref(), Some("4"));
    assert_eq!(get(&d, "arrival").as_deref(), Some("08:20:00"));

    static TIMEPOINT: &[u8] =
        b"trip_id,arrival_time,departure_time,stop_id,stop_sequence,timepoint\n\
          T1,08:00:00,08:00:00,S1,1,1\nT1,,08:12:00,S3,3,1\nT1,,,S2,2,1\n";
    files[4] = ("stop_times.txt", TIMEPOINT);
    let d = details(&files, "STM_047");
    assert_eq!(get(&d, "affected_rows").as_deref(), Some("2"));
    // In stop order: sequence 2 (both missing) before sequence 3 (arrival missing).
    assert_eq!(get(&d, "example_0_stop_id").as_deref(), Some("S2"));
    assert_eq!(get(&d, "example_0_missing").as_deref(), Some("both"));
    assert_eq!(get(&d, "example_1_stop_id").as_deref(), Some("S3"));
    assert_eq!(get(&d, "example_1_missing").as_deref(), Some("arrival"));
    assert_eq!(get(&d, "example_1_departure").as_deref(), Some("08:12:00"));
    assert_eq!(get(&d, "message_variant"), None);
}

#[test]
fn midnight_and_service_day_findings_carry_neighbour_rows() {
    // STM_045, STM_048 and STM_049 name the faulty row's sequence and times and the previous
    // and next row, so the planner card can show previous / faulty / next. Counts unchanged.
    let details = |files: &[(&str, &[u8])], rule: &str| match run(files) {
        ValidateResult::Ok(vr) => vr
            .notices
            .iter()
            .find(|n| n.rule_id == rule)
            .and_then(|n| n.details.clone())
            .unwrap_or_else(|| panic!("{rule} ayrıntısı bekleniyor")),
        other => panic!("ValidateResult::Ok beklendi, gelen: {other:?}"),
    };
    let get = |d: &gtfs_core::NoticeDetails, key: &str| d.get(key).map(|v| v.to_string());
    let mut files = base_files();

    static LATE: &[u8] = b"trip_id,arrival_time,departure_time,stop_id,stop_sequence\n\
          T1,26:50:00,26:50:00,S1,1\nT1,27:20:00,27:30:00,S2,2\nT1,27:40:00,27:40:00,S3,3\n";
    files[4] = ("stop_times.txt", LATE);
    let d = details(&files, "STM_045");
    assert_eq!(get(&d, "stop_id").as_deref(), Some("S2"));
    assert_eq!(get(&d, "stop_sequence").as_deref(), Some("2"));
    assert_eq!(get(&d, "arrival").as_deref(), Some("27:20:00"));
    assert_eq!(get(&d, "departure").as_deref(), Some("27:30:00"));
    assert_eq!(get(&d, "prev_stop_id").as_deref(), Some("S1"));
    assert_eq!(get(&d, "prev_departure").as_deref(), Some("26:50:00"));
    assert_eq!(get(&d, "next_stop_id").as_deref(), Some("S3"));
    assert_eq!(get(&d, "next_stop_sequence").as_deref(), Some("3"));

    static WRAP: &[u8] = b"trip_id,arrival_time,departure_time,stop_id,stop_sequence\n\
          T1,23:40:00,23:40:00,S1,1\nT1,00:10:00,00:10:00,S2,2\nT1,00:20:00,00:20:00,S3,3\n";
    files[4] = ("stop_times.txt", WRAP);
    let d = details(&files, "STM_048");
    assert_eq!(get(&d, "stop_id").as_deref(), Some("S2"));
    assert_eq!(get(&d, "stop_sequence").as_deref(), Some("2"));
    assert_eq!(get(&d, "arrival").as_deref(), Some("00:10:00"));
    assert_eq!(get(&d, "expected_time").as_deref(), Some("24:10:00"));
    assert_eq!(get(&d, "prev_stop_sequence").as_deref(), Some("1"));
    assert_eq!(get(&d, "prev_arrival").as_deref(), Some("23:40:00"));
    assert_eq!(get(&d, "next_stop_id").as_deref(), Some("S3"));

    static SAME_ROW: &[u8] = b"trip_id,arrival_time,departure_time,stop_id,stop_sequence\n\
          T1,23:30:00,23:30:00,S1,1\nT1,23:58:00,00:02:00,S2,2\nT1,24:10:00,24:10:00,S3,3\n";
    files[4] = ("stop_times.txt", SAME_ROW);
    let d = details(&files, "STM_049");
    assert_eq!(get(&d, "stop_sequence").as_deref(), Some("2"));
    assert_eq!(get(&d, "arrival").as_deref(), Some("23:58:00"));
    assert_eq!(get(&d, "departure").as_deref(), Some("00:02:00"));
    assert_eq!(get(&d, "prev_stop_id").as_deref(), Some("S1"));
    assert_eq!(get(&d, "next_arrival").as_deref(), Some("24:10:00"));

    // STM_003/004 (add_malformed_time_context) name the stop sequences too.
    static MALFORMED: &[u8] = b"trip_id,arrival_time,departure_time,stop_id,stop_sequence\n\
          T1,08:00:00,08:00:00,S1,1\nT1,8:6O,08:10:00,S2,2\nT1,08:20:00,08:20:00,S3,3\n";
    files[4] = ("stop_times.txt", MALFORMED);
    let d = details(&files, "STM_003");
    assert_eq!(get(&d, "stop_sequence").as_deref(), Some("2"));
    assert_eq!(get(&d, "prev_stop_sequence").as_deref(), Some("1"));
    assert_eq!(get(&d, "next_stop_sequence").as_deref(), Some("3"));
}
