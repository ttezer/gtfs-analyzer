use std::io::Write as _;
use zip::write::SimpleFileOptions;

use gtfs_pipeline::{k1_parse::parse, validate_bytes, FatalCode, ValidateResult, ValidatorConfig};

// Tüm testler için sabit tarih — deterministik analytics çıktısı
const TODAY: u32 = 20_260_515;

// ── Yardımcılar ───────────────────────────────────────────────────────────────

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

// Tarih aralığı (20250101–20271231) test gününü (20260515, Cuma) kapsar.
// Pazartesi–Cuma=1 → SVC1 aktif → servis boşluğu uyarısı çıkmaz.
static AGENCY: &[u8] =
    b"agency_id,agency_name,agency_url,agency_timezone\n1,Test,http://test.example,UTC\n";
static STOPS: &[u8] =
    b"stop_id,stop_name,stop_lat,stop_lon\nS1,Stop1,41.0,29.0\nS2,Stop2,41.1,29.1\n";
static ROUTES: &[u8] =
    b"route_id,agency_id,route_short_name,route_type\nR1,1,101,3\n";
static TRIPS: &[u8] =
    b"route_id,service_id,trip_id\nR1,SVC1,T1\n";
static TRIPS_DANGLING_ROUTE: &[u8] =
    b"route_id,service_id,trip_id\nR9,SVC1,T1\n";
static STOP_TIMES: &[u8] =
    b"trip_id,arrival_time,departure_time,stop_id,stop_sequence\n\
      T1,08:00:00,08:00:00,S1,1\nT1,08:10:00,08:10:00,S2,2\n";
static CALENDAR: &[u8] =
    b"service_id,monday,tuesday,wednesday,thursday,friday,saturday,sunday,start_date,end_date\n\
      SVC1,1,1,1,1,1,0,0,20250101,20271231\n";

fn base_files() -> Vec<(&'static str, &'static [u8])> {
    vec![
        ("agency.txt",     AGENCY),
        ("stops.txt",      STOPS),
        ("routes.txt",     ROUTES),
        ("trips.txt",      TRIPS),
        ("stop_times.txt", STOP_TIMES),
        ("calendar.txt",   CALENDAR),
    ]
}

fn run(files: &[(&str, &[u8])]) -> ValidateResult {
    validate_bytes(&make_zip(files), &ValidatorConfig::default(), TODAY)
}

const LEGACY_TRANSLATIONS: &[u8] =
    b"trans_id,lang,translation\n1,EN,Example\n2,HE,\xD7\x93\xD7\x95\xD7\x92\xD7\x9E\xD7\x94\n";

const MODERN_TRANSLATIONS: &[u8] =
    b"table_name,field_name,language,translation,record_id\n\
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

// ── issue #108: RTS_022 tek karakterli tam eşitliği kaçırmamalı ───────────────

fn rts022_route_name_feed(routes: &'static [u8]) -> Vec<(&'static str, &'static [u8])> {
    let mut files = base_files();
    files[2] = ("routes.txt", routes);
    files
}

#[test]
fn rts022_flags_one_character_equality_once() {
    static ROUTES: &[u8] =
        b"route_id,agency_id,route_short_name,route_long_name,route_type\n\
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
    static ROUTES: &[u8] =
        b"route_id,agency_id,route_short_name,route_long_name,route_type\n\
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
    static ROUTES: &[u8] =
        b"route_id,agency_id,route_short_name,route_long_name,route_type\n\
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
// ARC_001 yolu: pipeline ilk adımda durur, hiçbir kural çalışmaz.

#[test]
fn arc001_corrupt_zip_returns_fatal_zip_unreadable() {
    let result = validate_bytes(b"not a zip", &ValidatorConfig::default(), TODAY);
    match result {
        ValidateResult::Fatal(e) => assert_eq!(
            e.code,
            FatalCode::ZipUnreadable,
            "Beklenen ZipUnreadable, alınan: {:?}", e.code,
        ),
        _ => panic!("Fatal(ZipUnreadable) beklendi"),
    }
}

// ── Test 1b: Zorunlu dosya eksik → PARTIAL + ARC_004 ─────────────────────────
// ZIP okunabilir kaldığı için bağımsız bulgular korunur; yalnızca eksik dosyaya
// gerçekten bağlı alt kurallar çalışmaz.

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
            assert!(partial.unavailable_files.contains(&"routes.txt".to_string()));
            assert!(!partial.skipped_stages.contains(&"K4-cross-ref".to_string()));
            assert!(partial.skipped_checks.contains(&"K4::routes".to_string()));
            assert!(partial.skipped_checks.contains(&"K6::route_headway".to_string()));
            assert!(vr.notices.iter().any(
                |n| n.rule_id == "ARC_004" && n.observed_value.as_deref() == Some("routes.txt")
            ));
        }
        other => panic!("Partial Ok bekleniyor, gelen: {other:?}"),
    }
}

#[test]
fn malformed_optional_file_is_skipped_with_partial_report() {
    // feed_info.txt opsiyoneldir; bozuk UTF-8 typed kayda dönüştürülmemeli,
    // fakat okunabilir feed'in bağımsız K1/K2 kapsamı korunmalıdır.
    let mut files = base_files();
    files.push(("feed_info.txt", b"feed_publisher_name,feed_lang\nTest,\xFF\n"));

    match run(&files) {
        ValidateResult::Ok(vr) => {
            assert_eq!(vr.status, gtfs_core::ValidationStatus::Partial);
            let partial = vr.partial.expect("partial kapsamı raporlanmalı");
            assert!(partial.unavailable_files.contains(&"feed_info.txt".to_string()));
            assert!(!partial.skipped_stages.contains(&"K4-cross-ref".to_string()));
            assert!(partial.skipped_checks.contains(&"K4::translations".to_string()));
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
                    "FIN_010" | "FIN_016" | "FIN_017" | "FIN_018" | "FIN_019" | "FIN_020" | "CAL_020"
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
            assert!(partial.unavailable_files.contains(&"calendar.txt".to_string()));
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
            assert!(partial.unavailable_files.contains(&"stop_times.txt".to_string()));
            assert!(partial.skipped_checks.contains(&"K6::DQ_009".to_string()));
            assert!(partial.skipped_checks.contains(&"K6::remaining_analytics".to_string()));
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
            assert!(partial.skipped_checks.contains(&"K6::remaining_analytics".to_string()));
        }
        other => panic!("Partial Ok bekleniyor, gelen: {other:?}"),
    }
}

#[test]
fn malformed_feed_info_keeps_independent_cross_ref_findings() {
    let mut files = base_files();
    files[3] = ("trips.txt", TRIPS_DANGLING_ROUTE);
    files.push(("feed_info.txt", b"feed_publisher_name,feed_lang\nTest,\xFF\n"));

    match run(&files) {
        ValidateResult::Ok(vr) => {
            let partial = vr.partial.expect("bozuk feed_info PARTIAL üretmeli");
            assert!(partial.unavailable_files.contains(&"feed_info.txt".to_string()));
            assert!(!partial.skipped_stages.contains(&"K4-cross-ref".to_string()));
            assert!(vr.notices.iter().any(|n| n.rule_id == "TRP_002"),
                "feed_info bağımsız trip/route cross-ref bulgusunu kapatmamalı");
        }
        other => panic!("Partial Ok bekleniyor, gelen: {other:?}"),
    }
}

// ── Test 2: Minimal geçerli feed yayınlanabilir ───────────────────────────────

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

// ── Test 3: STP_003 → blocks[] GEO_009 ──────────────────────────────────────
// parse_coordinate: ilk kural range_rule_id (STP_003 = aralık dışı).
// stop_lat=999.0 → aralık dışı [-90,90] → STP_003 ateşlenir.
// Feed'de shape yok → GEO_009 doğal olarak üretilmez; STP_003.blocks içinde mevcuttur.
// Not: SHP_013 kaldırıldı (GEO_009 ile çift sayım yapıyordu).

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
                vr.notices.iter().map(|n| n.rule_id.as_str()).collect::<Vec<_>>(),
            );

            let blocks = &stp003.unwrap().blocks;
            assert!(
                blocks.iter().any(|b| b == "GEO_009"),
                "STP_003.blocks GEO_009 içermeli. Mevcut blocks: {:?}", blocks,
            );

            // S1 koordinatsız → GEO_009 kök neden olarak ateşlenmemeli
            assert!(
                !vr.notices.iter().any(|n| n.rule_id == "GEO_009"),
                "GEO_009 koordinat eksikken kök neden olarak üretilmemeli",
            );
        }
        _ => panic!("ValidateResult::Ok beklendi"),
    }
}

// ── Test 4: TRF_006 → blocks[] TRF_017 + TRF_018 ─────────────────────────────
// Geçersiz from_trip_id ("MISSING_TRIP") → K4 TRF_006 ateşler.
// Registry tanımı: TRF_006 blocks = [TRF_013, TRF_014, TRF_015, TRF_017, TRF_018, OPR_007].

#[test]
fn trf_006_blocks_contain_trf_017_and_trf_018() {
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
                vr.notices.iter().map(|n| n.rule_id.as_str()).collect::<Vec<_>>(),
            );

            let blocks = &trf006.unwrap().blocks;
            assert!(
                blocks.iter().any(|b| b == "TRF_017"),
                "TRF_006.blocks TRF_017 içermeli. Mevcut blocks: {:?}", blocks,
            );
            assert!(
                blocks.iter().any(|b| b == "TRF_018"),
                "TRF_006.blocks TRF_018 içermeli. Mevcut blocks: {:?}", blocks,
            );
        }
        _ => panic!("ValidateResult::Ok beklendi"),
    }
}

// ── Test 5: TRP_019 — continuous service aktif + shape_id yok → ateşlenir ─────
// route.continuous_pickup=1 → bu route'a ait trip shape_id olmadan geçersizdir.

#[test]
fn trp_019_fires_when_continuous_service_active_no_shape() {
    // continuous_pickup=1 olan route; trip'te shape_id yok
    const ROUTES_CONTINUOUS: &[u8] =
        b"route_id,agency_id,route_short_name,route_type,continuous_pickup\n\
          R1,1,101,3,1\n";

    let mut files = base_files();
    files[2] = ("routes.txt", ROUTES_CONTINUOUS);

    match run(&files) {
        ValidateResult::Ok(vr) => {
            assert!(
                vr.notices.iter().any(|n| n.rule_id == "TRP_019"),
                "TRP_019 olmalı (continuous_pickup=1, shape_id yok). Mevcut: {:?}",
                vr.notices.iter().map(|n| n.rule_id.as_str()).collect::<Vec<_>>(),
            );
        }
        _ => panic!("ValidateResult::Ok beklendi"),
    }
}

// ── Test 6: TRP_019 — continuous service yok → sessiz ────────────────────────
// route'ta continuous_pickup/drop_off yok, stop_times'ta da yok → TRP_019 üretilmez.

#[test]
fn trp_019_silent_when_no_continuous_service() {
    match run(&base_files()) {
        ValidateResult::Ok(vr) => {
            assert!(
                !vr.notices.iter().any(|n| n.rule_id == "TRP_019"),
                "TRP_019 üretilmemeli (continuous service yok). Notices: {:?}",
                vr.notices.iter().map(|n| n.rule_id.as_str()).collect::<Vec<_>>(),
            );
        }
        _ => panic!("ValidateResult::Ok beklendi"),
    }
}

#[test]
fn trp_019_fires_when_stop_time_enables_continuous_service() {
    const STOP_TIMES_CONTINUOUS: &[u8] =
        b"trip_id,arrival_time,departure_time,stop_id,stop_sequence,continuous_drop_off\n\
          T1,08:00:00,08:00:00,S1,1,1\nT1,08:10:00,08:10:00,S2,2,1\n";

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

// ── RTS_028: rotanın herhangi bir seferinde Flex penceresi varken routes.continuous_* ─────
// Rota kapsamlı koşul (MD `forbidden_continuous_pickup_drop_off` paritesi). İhlal eden satır
// stop_times'ta DEĞİL routes.txt'te; pencereyi taşıyan sefer başka bir satır olabilir.

// T1'in 2. durağı Flex pencereli (T1 → R1) — R1 böylece "Flex rotası" olur.
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
            assert_eq!(n.file.as_deref(), Some("routes.txt"), "ihlal routes.txt'te raporlanmalı");
            assert_eq!(n.field.as_deref(), Some("continuous_pickup"));
        }
        _ => panic!("ValidateResult::Ok beklendi"),
    }
}

#[test]
fn rts_028_silent_when_route_has_no_flex_window() {
    // continuous_pickup=0 ama hiçbir seferde pencere yok → spec'e göre Optional.
    const ROUTES_CONTINUOUS: &[u8] =
        b"route_id,agency_id,route_short_name,route_type,continuous_pickup\n\
          R1,1,101,3,0\n";

    let mut files = base_files();
    files[2] = ("routes.txt", ROUTES_CONTINUOUS);

    match run(&files) {
        ValidateResult::Ok(vr) => assert!(
            !vr.notices.iter().any(|n| n.rule_id == "RTS_028"),
            "Flex penceresi olmayan rotada RTS_028 üretilmemeli: {:?}",
            vr.notices.iter().map(|n| n.rule_id.as_str()).collect::<Vec<_>>(),
        ),
        _ => panic!("ValidateResult::Ok beklendi"),
    }
}

#[test]
fn rts_028_silent_when_continuous_is_one() {
    // Flex pencereli rota ama continuous_pickup=1 (izinli değer) → sessiz.
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
            vr.notices.iter().map(|n| n.rule_id.as_str()).collect::<Vec<_>>(),
        ),
        _ => panic!("ValidateResult::Ok beklendi"),
    }
}

#[test]
fn transfer_stop_requirements_cover_recommended_and_in_seat_types() {
    for transfer_type in ["0", "1", "2", "3"] {
        let transfers = format!(
            "from_stop_id,to_stop_id,transfer_type\n,,{transfer_type}\n"
        );
        let mut files = base_files();
        files.push(("transfers.txt", transfers.as_bytes()));
        match run(&files) {
            ValidateResult::Ok(vr) => {
                assert!(vr.notices.iter().any(|n| n.rule_id == "TRF_001"), "type={transfer_type}");
                assert!(vr.notices.iter().any(|n| n.rule_id == "TRF_002"), "type={transfer_type}");
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
            // 2026-08-02: bu vaka PTH_017'den PTH_028'e taşındı. PTH_017 artık yalnız TİP
            // ihlalini (sayı olmayan max_slope) ölçer; "yanlış pathway_mode ile kullanılmış"
            // dalı spec'te "should" olduğu için Quality sınıfında ayrı kuraldır.
            let wrong_mode = vr
                .notices
                .iter()
                .filter(|n| n.rule_id == "PTH_028")
                .collect::<Vec<_>>();
            assert_eq!(wrong_mode.len(), 1, "yalnız elevator max_slope bağlam ihlali olmalı");
            assert!(
                !vr.notices.iter().any(|n| n.rule_id == "PTH_017"),
                "geçerli sayılar PTH_017 (tip ihlali) üretmemeli"
            );
        }
        _ => panic!("ValidateResult::Ok beklendi"),
    }
}

// ── Test 7: PTH_014 — farklı istasyonları bağlayan pathway → ateşlenir ────────
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
                vr.notices.iter().map(|n| n.rule_id.as_str()).collect::<Vec<_>>(),
            );
        }
        _ => panic!("ValidateResult::Ok beklendi"),
    }
}

// ── Test 8: PTH_014 — aynı istasyon içi pathway → sessiz ─────────────────────
// PLT_A ve PLT_B aynı istasyona (STA1) ait → PTH_014 üretilmez.

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
                vr.notices.iter().map(|n| n.rule_id.as_str()).collect::<Vec<_>>(),
            );
        }
        _ => panic!("ValidateResult::Ok beklendi"),
    }
}

// ── Test 9: STP_016 — aynı koordinatlı iki durak → uyarı ─────────────────────

#[test]
fn stp_016_fires_for_stops_at_identical_coordinates() {
    const STOPS_SAME_COORD: &[u8] =
        b"stop_id,stop_name,stop_lat,stop_lon\n\
          S1,Stop1,41.0,29.0\nS2,Stop2,41.0,29.0\n";  // S1 ve S2 aynı koordinat

    let mut files = base_files();
    files[1] = ("stops.txt", STOPS_SAME_COORD);

    match run(&files) {
        ValidateResult::Ok(vr) => {
            assert!(
                vr.notices.iter().any(|n| n.rule_id == "STP_016"),
                "STP_016 olmalı. Mevcut: {:?}",
                vr.notices.iter().map(|n| n.rule_id.as_str()).collect::<Vec<_>>(),
            );
        }
        _ => panic!("ValidateResult::Ok beklendi"),
    }
}

// ── Test 10: STP_016 — farklı koordinatlı duraklar sessiz ────────────────────

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

// ── Test 10b: STP_016 — parent/child aynı koordinat sessiz ───────────────────
// Çocuk durak, ait olduğu istasyonla aynı koordinatta olabilir (normal GTFS).
// STP_016 bu çifti FP olarak işaretlememeli.

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

// ── Test 10c: TRN_010 — field_value modunda record_sub_id muaf ───────────────
// stop_times çevirisi field_value ile eşleştirilirse record_id/record_sub_id boş
// olmalıdır (spec). Bu modda TRN_010 ateşlenmemeli; record_id modunda ateşlenmeli.

#[test]
fn trn_010_silent_in_field_value_mode_but_fires_in_record_id_mode() {
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
            vr.notices.iter().any(|n| n.rule_id == "TRN_010"),
            "record_id modunda record_sub_id yoksa TRN_010 ateşlenmeli.",
        ),
        _ => panic!("ValidateResult::Ok beklendi"),
    }
}

// ── Test 10d: TRN_007 — feed_lang ile aynı dildeki çeviriler tek özette toplanır ──

#[test]
fn trn_007_same_language_translations_aggregate_to_feed_summary() {
    const FEED_INFO: &[u8] =
        b"feed_publisher_name,feed_publisher_url,feed_lang\nP,http://x.example,ja\n";
    // İki ja çeviri (feed_lang=ja) → satır-başına 2 yerine tek feed-seviyesi TRN_007.
    const TR_JA: &[u8] =
        b"table_name,field_name,language,translation,record_id,record_sub_id,field_value\n\
          stops,stop_name,ja,S1 Stop,S1,,\n\
          stops,stop_name,ja,S2 Stop,S2,,\n";
    let mut files = base_files();
    files.push(("feed_info.txt", FEED_INFO));
    files.push(("translations.txt", TR_JA));
    match run(&files) {
        ValidateResult::Ok(vr) => {
            let trn: Vec<_> = vr.notices.iter().filter(|n| n.rule_id == "TRN_007").collect();
            assert_eq!(trn.len(), 1, "aynı dildeki çeviriler tek feed-özetinde toplanmalı");
            assert_eq!(trn[0].observed_value.as_deref(), Some("2"), "etkilenen satır sayısı 2");
        }
        _ => panic!("ValidateResult::Ok beklendi"),
    }
}

// ── Test 11: PTH_012 + PTH_013 — erişilemeyen platform → uyarı ───────────────
// İstasyon: STA → ENT (loc=2), MID (loc=3), PLT (loc=0).
// Pathway: ENT↔MID — PLT hiçbir entrance'tan erişilemiyor → PTH_012.
// Accessible BFS'te de PLT erişilemez → PTH_013 (wheelchair path yok).

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
    // PW1: ENT↔MID — PLT pathway bağlantısı yok
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
                vr.notices.iter().map(|n| n.rule_id.as_str()).collect::<Vec<_>>(),
            );
            // PTH_013 kapsama bilgisi de üretilmeli
            assert!(
                vr.notices.iter().any(|n| n.rule_id == "PTH_013"),
                "PTH_013 bilgi notu olmalı",
            );
        }
        _ => panic!("ValidateResult::Ok beklendi"),
    }
}

// ── Test 12: PTH_013 negatif — erişilebilir rota varsa sessiz ────────────────
// ENT→PLT: max_slope=0.05 (≤0.08) VE min_width=1.0 (≥0.9) → accessible path var.
// PTH_013 üretilmemeli; PTH_012 de üretilmemeli.

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
                vr.notices.iter().map(|n| n.rule_id.as_str()).collect::<Vec<_>>(),
            );
            assert!(
                !vr.notices.iter().any(|n| n.rule_id == "PTH_012"),
                "PTH_012 üretilmemeli (PLT erişilebilir). Notices: {:?}",
                vr.notices.iter().map(|n| n.rule_id.as_str()).collect::<Vec<_>>(),
            );
        }
        _ => panic!("ValidateResult::Ok beklendi"),
    }
}

// ── Test 13: PTH_015 — hız > 3.0 m/s → ateşlenir ───────────────────────────
// length=300m, traversal_time=50s → hız=6.0 m/s > eşik(3.0) → PTH_015.

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
                vr.notices.iter().map(|n| n.rule_id.as_str()).collect::<Vec<_>>(),
            );
        }
        _ => panic!("ValidateResult::Ok beklendi"),
    }
}

// ── Test 14: PTH_015 — hız ≤ 3.0 m/s → sessiz ──────────────────────────────
// length=100m, traversal_time=60s → hız≈1.67 m/s ≤ eşik → PTH_015 üretilmez.

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
                vr.notices.iter().map(|n| n.rule_id.as_str()).collect::<Vec<_>>(),
            );
        }
        _ => panic!("ValidateResult::Ok beklendi"),
    }
}

// ── Test 16: R9 skor hesabı — kritik notice priority_score > 0 üretir ────────
// STP_003 (Kritik, base_effort=2) → R9'da görünür ve priority_score hesaplanır.

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
                vr.reports.r9.items.iter()
                    .map(|i| (i.rule_id.as_str(), i.priority_score))
                    .collect::<Vec<_>>(),
            );
        }
        _ => panic!("ValidateResult::Ok beklendi"),
    }
}

// ── Test 16: PTH_019 — generic node tek pathway'e bağlı → ateşlenir ──────────
// GN1 (location_type=3) yalnızca 1 pathway'de geçiyor → dead-end → PTH_019.

#[test]
fn pth_019_fires_for_dangling_generic_node() {
    const STOPS: &[u8] =
        b"stop_id,stop_name,stop_lat,stop_lon,location_type,parent_station\n\
          S1,Stop1,41.0,29.0,,\n\
          S2,Stop2,41.1,29.1,,\n\
          STA1,Station1,41.2,29.2,1,\n\
          PLT_A,PlatformA,41.2,29.2,0,STA1\n\
          GN1,GenericNode1,41.2,29.21,3,STA1\n";
    const PATHWAYS: &[u8] =
        b"pathway_id,from_stop_id,to_stop_id,pathway_mode,is_bidirectional\n\
          PW1,PLT_A,GN1,1,0\n";

    let mut files = base_files();
    files[1] = ("stops.txt", STOPS);
    files.push(("pathways.txt", PATHWAYS));

    match run(&files) {
        ValidateResult::Ok(vr) => {
            assert!(
                vr.notices.iter().any(|n| n.rule_id == "PTH_019"),
                "PTH_019 olmalı (dangling generic node). Mevcut: {:?}",
                vr.notices.iter().map(|n| n.rule_id.as_str()).collect::<Vec<_>>(),
            );
        }
        _ => panic!("ValidateResult::Ok beklendi"),
    }
}

// ── Test 17: PTH_019 — generic node iki pathway'e bağlı → sessiz ──────────────
// GN1 hem PLT_A→GN1 hem GN1→PLT_B bağlantısında → köprü → PTH_019 üretilmez.

#[test]
fn pth_019_silent_for_connected_generic_node() {
    const STOPS: &[u8] =
        b"stop_id,stop_name,stop_lat,stop_lon,location_type,parent_station\n\
          S1,Stop1,41.0,29.0,,\n\
          S2,Stop2,41.1,29.1,,\n\
          STA1,Station1,41.2,29.2,1,\n\
          PLT_A,PlatformA,41.2,29.2,0,STA1\n\
          PLT_B,PlatformB,41.2,29.25,0,STA1\n\
          GN1,GenericNode1,41.2,29.21,3,STA1\n";
    const PATHWAYS: &[u8] =
        b"pathway_id,from_stop_id,to_stop_id,pathway_mode,is_bidirectional\n\
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
                vr.notices.iter().map(|n| n.rule_id.as_str()).collect::<Vec<_>>(),
            );
        }
        _ => panic!("ValidateResult::Ok beklendi"),
    }
}

// ── Test 18: LVL_006 — asansör pathway'inde level_id eksik → ateşlenir ────────
// ELV_STOP asansöre bağlı ama level_id yok → LVL_006.

#[test]
fn lvl_006_fires_when_elevator_stop_missing_level_id() {
    const STOPS: &[u8] =
        b"stop_id,stop_name,stop_lat,stop_lon,location_type,parent_station,level_id\n\
          S1,Stop1,41.0,29.0,,,\n\
          S2,Stop2,41.1,29.1,,,\n\
          STA1,Station1,41.2,29.2,1,,\n\
          PLT_A,PlatformA,41.2,29.2,0,STA1,L1\n\
          ELV_STOP,ElevatorStop,41.2,29.21,0,STA1,\n";
    const PATHWAYS: &[u8] =
        b"pathway_id,from_stop_id,to_stop_id,pathway_mode,is_bidirectional\n\
          PW1,PLT_A,ELV_STOP,5,1\n";

    let mut files = base_files();
    files[1] = ("stops.txt", STOPS);
    files.push(("pathways.txt", PATHWAYS));

    match run(&files) {
        ValidateResult::Ok(vr) => {
            assert!(
                vr.notices.iter().any(|n| n.rule_id == "LVL_006"),
                "LVL_006 olmalı (asansör stop level_id eksik). Mevcut: {:?}",
                vr.notices.iter().map(|n| n.rule_id.as_str()).collect::<Vec<_>>(),
            );
        }
        _ => panic!("Fatal hata"),
    }
}


// ── Test 19: LVL_006 — asansör pathway'inde level_id dolu → sessiz ────────────
// PLT_A ve PLT_B her ikisinde de level_id var → LVL_006 üretilmez.

#[test]
fn lvl_006_silent_when_elevator_stops_have_level_id() {
    const STOPS: &[u8] =
        b"stop_id,stop_name,stop_lat,stop_lon,location_type,parent_station,level_id\n\
          S1,Stop1,41.0,29.0,,,\n\
          S2,Stop2,41.1,29.1,,,\n\
          STA1,Station1,41.2,29.2,1,,\n\
          PLT_A,PlatformA,41.2,29.2,0,STA1,L0\n\
          PLT_B,PlatformB,41.2,29.25,0,STA1,L1\n";
    const PATHWAYS: &[u8] =
        b"pathway_id,from_stop_id,to_stop_id,pathway_mode,is_bidirectional\n\
          PW1,PLT_A,PLT_B,5,1\n";

    let mut files = base_files();
    files[1] = ("stops.txt", STOPS);
    files.push(("pathways.txt", PATHWAYS));

    match run(&files) {
        ValidateResult::Ok(vr) => {
            assert!(
                !vr.notices.iter().any(|n| n.rule_id == "LVL_006"),
                "LVL_006 üretilmemeli (level_id dolu). Notices: {:?}",
                vr.notices.iter().map(|n| n.rule_id.as_str()).collect::<Vec<_>>(),
            );
        }
        _ => panic!("ValidateResult::Ok beklendi"),
    }
}

// ── Test 20: XFL_019 — routes.network_id + route_networks.txt ikisi birden → ateşlenir
// routes.txt'te network_id dolu VE route_networks.txt mevcut → XFL_019.

#[test]
fn xfl_019_fires_when_network_defined_in_both_files() {
    const ROUTES_WITH_NETWORK: &[u8] =
        b"route_id,agency_id,route_short_name,route_long_name,route_type,network_id\n\
          R1,A1,1,Route One,3,NET1\n";
    const ROUTE_NETWORKS: &[u8] =
        b"network_id,route_id\n\
          NET1,R1\n";

    let mut files = base_files();
    files[2] = ("routes.txt", ROUTES_WITH_NETWORK);
    files.push(("route_networks.txt", ROUTE_NETWORKS));

    match run(&files) {
        ValidateResult::Ok(vr) => {
            assert!(
                vr.notices.iter().any(|n| n.rule_id == "XFL_019"),
                "XFL_019 olmalı (çift ağ tanımı). Mevcut: {:?}",
                vr.notices.iter().map(|n| n.rule_id.as_str()).collect::<Vec<_>>(),
            );
        }
        _ => panic!("ValidateResult::Ok beklendi"),
    }
}

// ── Test 21: XFL_019 — yalnızca route_networks.txt var → sessiz ───────────────
// routes.txt'te network_id yok, route_networks.txt var → XFL_019 üretilmez.

#[test]
fn xfl_019_silent_when_only_route_networks_file() {
    const ROUTE_NETWORKS: &[u8] =
        b"network_id,route_id\n\
          NET1,R1\n";

    let mut files = base_files();
    files.push(("route_networks.txt", ROUTE_NETWORKS));

    match run(&files) {
        ValidateResult::Ok(vr) => {
            assert!(
                !vr.notices.iter().any(|n| n.rule_id == "XFL_019"),
                "XFL_019 üretilmemeli (sadece route_networks.txt). Notices: {:?}",
                vr.notices.iter().map(|n| n.rule_id.as_str()).collect::<Vec<_>>(),
            );
        }
        _ => panic!("ValidateResult::Ok beklendi"),
    }
}

#[test]
fn path_traversal_entries_rejected_and_root_file_intact() {
    // ZIP icindeki traversal/nested giris adlari kok dosya yerine gecmemeli.
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

    let k1 = parse(&make_zip(&files), &ValidatorConfig::default()).expect("gecerli kok dosyalar mevcut oldugunda Ok donmeli");

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

    let agency = k1.files.get("agency.txt").expect("kok agency.txt islenmeli");
    let joined = agency
        .rows
        .iter()
        .flatten()
        .map(|s| s.as_str())
        .collect::<Vec<_>>()
        .join(",");
    assert!(joined.contains("Test"), "kok agency.txt icerigi korunmali: {joined}");
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

// ── ARC_025: zorunlu sutun basligta hic yok (header-level) ────────────────────

#[test]
fn missing_required_column_produces_arc_025() {
    // agency.txt'te zorunlu 'agency_name' sutunu basligta yok → ARC_025 (dosya basina bir kez).
    let bad_agency: &[u8] = b"agency_id,agency_url,agency_timezone\n1,http://test.example,UTC\n";
    let mut files = base_files();
    files[0] = ("agency.txt", bad_agency);
    let k1 = parse(&make_zip(&files), &ValidatorConfig::default()).expect("gecerli kok dosyalar Ok donmeli");

    let arc025_name = k1
        .notices
        .iter()
        .filter(|n| n.rule_id == "ARC_025" && n.field.as_deref() == Some("agency_name"))
        .count();
    assert_eq!(
        arc025_name, 1,
        "agency_name sutunu basligta yok → 1 ARC_025, bulunan: {arc025_name}",
    );

    // Mevcut sutun (agency_url) icin ARC_025 CIKMAMALI — o, deger boslugu (k2 grup kurali) konusudur.
    assert!(
        !k1.notices
            .iter()
            .any(|n| n.rule_id == "ARC_025" && n.field.as_deref() == Some("agency_url")),
        "mevcut sutun icin ARC_025 cikmamali",
    );
}

#[test]
fn complete_headers_produce_no_arc_025() {
    // base_files tum zorunlu sutunlari icerir → hic ARC_025 olmamali.
    let k1 = parse(&make_zip(&base_files()), &ValidatorConfig::default()).expect("gecerli kok dosyalar Ok donmeli");
    let arc025 = k1.notices.iter().filter(|n| n.rule_id == "ARC_025").count();
    assert_eq!(arc025, 0, "tam basliklarda ARC_025 cikmamali, bulunan: {arc025}");
}

// ── XFL_026/027: route-bazlı contactless EMV uygulanabilirliği ─────────────────
// Ortak: 1 route (R1, 2 durak), contactless fare_media(type=3) + fare_product.
// Senaryoya göre routes.txt (cemv/network) + fare_leg_rules + networks değişir.
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
    // R1 cemv_support=2 (desteklenmiyor) ama leg rule GLOBAL (network/area yok) → her route kapsanır → celiski.
    let mut files = cemv_base();
    files.push(("routes.txt", b"route_id,agency_id,route_short_name,route_type,cemv_support\nR1,A1,1,3,2\n"));
    files.push(("fare_leg_rules.txt", b"leg_group_id,fare_product_id\nlg1,p_emv\n"));
    match run(&files) {
        ValidateResult::Ok(vr) => assert!(
            vr.notices.iter().any(|n| n.rule_id == "XFL_027"),
            "XFL_027 bekleniyor (cemv=2 + global contactless). Cikan: {:?}",
            vr.notices.iter().map(|n| n.rule_id.as_str()).collect::<Vec<_>>()),
        other => panic!("Ok bekleniyordu: {other:?}"),
    }
}

#[test]
fn xfl026_route_cemv1_network_mismatch() {
    // R1 cemv=1, network=N1; contactless leg rule network=N2 → R1 kapsanmiyor + cozulebilir → XFL_026.
    let mut files = cemv_base();
    files.push(("routes.txt", b"route_id,agency_id,route_short_name,route_type,network_id,cemv_support\nR1,A1,1,3,N1,1\n"));
    files.push(("networks.txt", b"network_id,network_name\nN1,N1\nN2,N2\n"));
    files.push(("fare_leg_rules.txt", b"leg_group_id,fare_product_id,network_id\nlg1,p_emv,N2\n"));
    match run(&files) {
        ValidateResult::Ok(vr) => assert!(
            vr.notices.iter().any(|n| n.rule_id == "XFL_026"),
            "XFL_026 bekleniyor (cemv=1 + network eslesmiyor). Cikan: {:?}",
            vr.notices.iter().map(|n| n.rule_id.as_str()).collect::<Vec<_>>()),
        other => panic!("Ok bekleniyordu: {other:?}"),
    }
}

#[test]
fn xfl026_silent_when_route_coverage_unresolvable() {
    // R1 cemv=1, network_id BOS; contactless leg rule network-based → route kapsami cozulemez → FP guard SUSAR.
    let mut files = cemv_base();
    files.push(("routes.txt", b"route_id,agency_id,route_short_name,route_type,cemv_support\nR1,A1,1,3,1\n"));
    files.push(("networks.txt", b"network_id,network_name\nN1,N1\n"));
    files.push(("fare_leg_rules.txt", b"leg_group_id,fare_product_id,network_id\nlg1,p_emv,N1\n"));
    match run(&files) {
        ValidateResult::Ok(vr) => {
            let ids: Vec<&str> = vr.notices.iter().map(|n| n.rule_id.as_str()).collect();
            assert!(!ids.contains(&"XFL_026") && !ids.contains(&"XFL_027"),
                "FP guard susmali (network/area cozulemez). Cikan: {ids:?}");
        }
        other => panic!("Ok bekleniyordu: {other:?}"),
    }
}

// ── ARC_029: decompression guard (zip-bomb) uçtan uca ─────────────────────────
// #46 guard'ı GuardedReader seviyesinde birim-testli (decompress_guard.rs), ancak
// DEFAULT_DECOMPRESSION_LIMITS ile validate_bytes üzerinden kablolamayı hiçbir test
// doğrulamıyordu. Bu testler ARC_029'un iki ayrı read_fatal çağrı yerini kanıtlar:
// zip-stream yolu (stop_times/trips/calendar_dates) ve read_to_end yolu (diğer .txt).
//
// Tetikleme koşulu (DEFAULT): entry_decompressed > ratio_floor (16 MiB) VE
// entry_decompressed > 80 × entry_compressed. Sıfır baytlar ~1000:1 sıkışır.
const BOMB_DECOMPRESSED: usize = 24 * 1024 * 1024; // 16 MiB ratio_floor'un üstünde

fn bomb_payload() -> Vec<u8> {
    vec![b'\n'; BOMB_DECOMPRESSED]
}

fn assert_decompression_limit(result: ValidateResult, ctx: &str) {
    match result {
        ValidateResult::Fatal(e) => assert_eq!(
            e.code,
            FatalCode::DecompressionLimit,
            "{ctx}: beklenen DecompressionLimit, alınan {:?} ({})", e.code, e.message,
        ),
        other => panic!("{ctx}: Fatal(DecompressionLimit) beklendi, alınan: {other:?}"),
    }
}

#[test]
fn arc029_zip_bomb_in_streamed_file_returns_fatal_decompression_limit() {
    let bomb = bomb_payload();
    let mut files = base_files();
    // stop_times.txt = #38 zip-stream yolu (k1_parse.rs read_fatal çağrı yeri 1)
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
    // shapes.txt = read_to_end yolu (k1_parse.rs read_fatal çağrı yeri 2)
    files.push(("shapes.txt", &bomb));
    let zip = make_zip(&files);
    assert_decompression_limit(
        validate_bytes(&zip, &ValidatorConfig::default(), TODAY),
        "shapes.txt (read_to_end yolu)",
    );
}

#[test]
fn arc029_legitimate_small_high_ratio_file_does_not_trip_guard() {
    // FP guard: ratio_floor'un ALTINDA kalan çok-sıkışan meşru dosya bombaya sayılmaz.
    // (#46 kapanışında dış katılımcının işaret ettiği "meşru-yüksek-oran" riski.)
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

// ── Determinizm kapısı ────────────────────────────────────────────────────────
// Aynı feed, aynı `today` → BİREBİR aynı çıktı. Bu güvence 2026-07-27'ye kadar YOKTU:
// notice içerikleri ve id'leri koşudan koşuya değişiyordu, çünkü emisyon sırası HashMap
// iterasyonuna bağlıydı ve dedup'ın keep-first temsilcisi o sıradan seçiliyordu (VBB'de
// 829 kayıt oynuyordu). Golden karşılaştırma ve A/B ölçümleri bu yüzden güvenilmezdi.
#[test]
fn same_feed_twice_produces_identical_output() {
    // Nondeterminizmi tetikleyen desenleri içerir: aynı seferde TEKRARLANAN durak (OPR_007),
    // birden çok shape (SHP/GEO aileleri), birden çok hat+servis (OPR_001/003/024).
    let shapes = b"shape_id,shape_pt_lat,shape_pt_lon,shape_pt_sequence\n\
        S1,41.0,29.0,1\nS1,41.0,29.0,2\nS1,41.5,29.5,3\n\
        S2,40.0,28.0,1\nS2,40.0,28.0,2\nS2,40.9,28.9,3\n\
        S3,39.0,27.0,1\nS3,39.0,27.0,2\nS3,39.8,27.8,3\n" as &[u8];
    let routes = b"route_id,agency_id,route_short_name,route_type\nR1,1,101,3\nR2,1,102,3\nR3,1,103,3\n" as &[u8];
    let trips = b"route_id,service_id,trip_id,shape_id\n\
        R1,SVC1,T1,S1\nR2,SVC1,T2,S2\nR3,SVC1,T3,S3\n" as &[u8];
    // T1: S1 durağı iki kez, aralarında başka durak → OPR_007 adayı (birden çok tekrar olası)
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

    // Kapsam: TAM SERİLEŞTİRİLMİŞ JSON — yalnız notice alanları değil, `Notice.details`,
    // `NameIndex` ve `capped_totals` dahil her şey. 2026-07-31'e kadar bu mümkün DEĞİLDİ:
    // o alanlar `HashMap`'ti ve serde anahtarları her koşuda başka sırada yazıyordu, yani
    // içerik aynıyken bayt farklıydı. Hepsi `BTreeMap`'e çevrildi; bu kapı o kazanımı korur.
    // Zayıflatma: bu karşılaştırmayı alan-alt-kümesine geri çekmek, sessizce kaybetmek demektir.
    let render = || match validate_bytes(&zip, &ValidatorConfig::default(), TODAY) {
        ValidateResult::Ok(vr) => serde_json::to_string(&vr)
            .expect("ValidationResult serialize edilebilmeli"),
        ValidateResult::Fatal(e) => panic!("beklenmeyen fatal: {:?} {}", e.code, e.message),
    };

    // Aynı SÜREÇ içinde tekrar: Rust'ta her HashMap örneği kendi tohumunu aldığı için bu,
    // ayrı süreçlerde koşmaktan daha sıkı bir kapıdır.
    let first = render();
    for run in 2..=4 {
        let cur = render();
        if cur != first {
            // İlk ayrışan konumu göster: tam JSON'u basmak okunmaz olurdu.
            let at = first.bytes().zip(cur.bytes()).position(|(a, b)| a != b)
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

// ── STM_015/016: Flex guard ve ilk/son durak kapsamı ──────────────────────────
// Spec, arrival_time/departure_time için "Forbidden when start_pickup_drop_off_window or
// end_pickup_drop_off_window are defined" der. Flex seferde eksik zaman alanı ihlal DEĞİL,
// beklenen durumdur; guard'sız hâlde bu iki kural her Flex seferde yanlış pozitif üretirdi.

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
    // İlk durakta arrival var/departure yok → STM_015 DEĞİL (spec departure'ı zorunlu
    // kılmaz); bu vakayı STM_034 ("yalnız biri tanımlı") zaten karşılar.
    // Son durakta departure var/arrival yok → STM_016 (spec arrival'ı zorunlu kılar).
    static EDGE_STOP_TIMES: &[u8] =
        b"trip_id,arrival_time,departure_time,stop_id,stop_sequence\n\
          T1,08:00:00,,S1,1\nT1,,08:10:00,S2,2\n";
    let mut files = base_files();
    files[4] = ("stop_times.txt", EDGE_STOP_TIMES);

    match run(&files) {
        ValidateResult::Ok(vr) => {
            let ids: Vec<&str> = vr.notices.iter().map(|n| n.rule_id.as_str()).collect();
            assert!(!ids.contains(&"STM_015"), "ilk durakta arrival var → STM_015 olmamalı: {ids:?}");
            assert!(ids.contains(&"STM_016"), "son durakta arrival yok → STM_016 bekleniyor: {ids:?}");
            assert!(ids.contains(&"STM_034"), "departure tarafını STM_034 karşılamalı: {ids:?}");
        }
        other => panic!("ValidateResult::Ok beklendi, gelen: {other:?}"),
    }
}

// ── STM_057 KALDIRILDI (2026-08-01, WP-1) ─────────────────────────────────────
// Kuralın iki testi de buradaydı. Spec cümlesi koşulludur ("travel WITHIN THE SAME
// location group…requires two records"); kural bunu her konum için iki kayıt şartına
// çevirmişti ve L1 → L2 gibi normal noktadan noktaya Flex seferini reddediyordu.
// Yerine geçen kapı: spec_conformance.rs — geçerli Flex feed'leri Spec notice ÜRETMEZ.

// ── stops.txt KOŞULLU zorunlu (yalnızca-Flex feed) ────────────────────────────
// Spec (Schedule Reference, dosya tablosu): "stops.txt — Conditionally Required —
// Optional if demand-responsive zones are defined in locations.geojson. Required otherwise."
// 2026-08-01'e kadar stops.txt koşulsuz REQUIRED_FILES'taydı → geçerli bir yalnızca-Flex
// feed ARC_004 Fatal'ı alıyor ve HİÇ açılmıyordu; kullanıcı tek bulgu bile göremiyordu.

#[test]
fn flex_only_feed_without_stops_txt_is_not_fatal() {
    static GEOJSON: &[u8] = br#"{"type":"FeatureCollection","features":[
        {"type":"Feature","id":"zone_a","geometry":{"type":"Polygon","coordinates":
        [[[29.0,41.0],[29.1,41.0],[29.1,41.1],[29.0,41.1],[29.0,41.0]]]},"properties":{}}]}"#;
    static FLEX_STOP_TIMES: &[u8] =
        b"trip_id,location_id,stop_sequence,start_pickup_drop_off_window,end_pickup_drop_off_window\n\
          T1,zone_a,1,08:00:00,12:00:00\nT1,zone_a,2,08:00:00,12:00:00\n";

    let mut files = base_files();
    files.retain(|(n, _)| *n != "stops.txt" && *n != "stop_times.txt");
    files.push(("stop_times.txt", FLEX_STOP_TIMES));
    files.push(("locations.geojson", GEOJSON));

    match validate_bytes(&make_zip(&files), &ValidatorConfig::default(), TODAY) {
        ValidateResult::Fatal(e) => panic!(
            "locations.geojson varken stops.txt eksikliği Fatal OLMAMALI (spec: koşullu zorunlu); \
             alınan: {:?} — {}", e.code, e.message),
        ValidateResult::Ok(vr) => {
            assert!(
                !vr.notices.iter().any(|n| n.rule_id == "ARC_004"
                    && n.observed_value.as_deref() == Some("stops.txt")),
                "stops.txt için ARC_004 üretilmemeli"
            );
        }
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
            let partial = vr.partial.expect("bozuk locations.geojson PARTIAL üretmeli");
            assert!(partial.unavailable_files.contains(&"locations.geojson".to_string()));
            assert!(partial.unavailable_files.contains(&"stops.txt".to_string()));
            assert!(vr.notices.iter().any(|n| {
                n.rule_id == "ARC_004" && n.observed_value.as_deref() == Some("stops.txt")
            }));
        }
        ValidateResult::Fatal(e) => panic!("bozuk optional GeoJSON Fatal olmamalı: {e:?}"),
    }
}

/// stops.txt VE locations.geojson ikisi de yoksa hizmetin nerede verildiği hiç tanımlı
/// değildir → PARTIAL raporlanır; ZIP okunabilir kaldığı için Fatal değildir.
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

// ── Flex stop_times: stop_id KOŞULLU zorunlu ──────────────────────────────────
// Spec (stop_times.stop_id): "Conditionally Required — Required if
// stop_times.location_group_id AND stop_times.location_id are NOT defined.
// Forbidden if stop_times.location_group_id or stop_times.location_id are defined."
// 2026-08-01'e kadar K1 stop_id'yi koşulsuz zorunlu sayıyor, 6 resmî Flex sütunu
// known_columns'ta bulunmuyor ve STM_006 boş stop_id'de koşulsuz ateşliyordu — oysa
// K2 parser bu alanları ZATEN okuyordu. K1 ile K2 aynı satır hakkında farklı düşünüyordu.

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
        ValidateResult::Fatal(e) => panic!("Flex stop_times Fatal olmamalı: {:?} {}", e.code, e.message),
    };
    let ids: Vec<&str> = vr.notices.iter().map(|n| n.rule_id.as_str()).collect();
    assert!(!ids.contains(&"STM_006"), "location_id taşıyan satır stop_id istemez → STM_006 çıkmamalı");
    assert!(!ids.contains(&"ARC_025"), "Flex dosyada stop_id sütunu beklenmez → ARC_025 çıkmamalı");
    // 6 resmî Flex sütununun hiçbiri "bilinmeyen sütun" sayılmamalı.
    let unknown: Vec<&str> = vr.notices.iter()
        .filter(|n| n.rule_id == "ARC_017")
        .filter_map(|n| n.field.as_deref())
        .collect();
    assert!(unknown.is_empty(), "resmî Flex sütunları ARC_017 üretmemeli: {unknown:?}");
}

/// Daraltmanın bedeli olmamalı: Flex ALANI OLMAYAN bir satırda boş stop_id yine hatadır.
#[test]
fn non_flex_row_with_empty_stop_id_still_produces_stm_006() {
    static BAD_STOP_TIMES: &[u8] =
        b"trip_id,arrival_time,departure_time,stop_id,stop_sequence\n\
          T1,08:00:00,08:00:00,,1\nT1,08:10:00,08:10:00,S2,2\n";
    let mut files = base_files();
    files.retain(|(n, _)| *n != "stop_times.txt");
    files.push(("stop_times.txt", BAD_STOP_TIMES));
    match validate_bytes(&make_zip(&files), &ValidatorConfig::default(), TODAY) {
        ValidateResult::Ok(vr) => assert!(
            vr.notices.iter().any(|n| n.rule_id == "STM_006"),
            "Flex alanı olmayan satırda boş stop_id STM_006 üretmeli"),
        ValidateResult::Fatal(e) => panic!("beklenmeyen fatal: {:?} {}", e.code, e.message),
    }
}

// ── XFL_019: routes.network_id ile AYRI ağ dosyası birlikte kullanılamaz ───────
// Spec (routes.network_id): "Conditionally Forbidden — Forbidden if the route_networks.txt
// OR networks.txt file exists." 2026-08-01'e kadar yalnız route_networks.txt kolu
// denetleniyordu; networks.txt ile birlikte kullanım aynı derecede yasakken kaçıyordu.

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
            "networks.txt + routes.network_id → XFL_019 çıkmalı"),
        ValidateResult::Fatal(e) => panic!("beklenmeyen fatal: {:?} {}", e.code, e.message),
    }
}

/// Ağ dosyası YOKKEN routes.network_id tek başına GEÇERLİDİR (spec: "Optional otherwise").
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
            "ayrı ağ dosyası yokken routes.network_id geçerlidir → XFL_019 çıkmamalı"),
        ValidateResult::Fatal(e) => panic!("beklenmeyen fatal: {:?} {}", e.code, e.message),
    }
}

// ── DQ_021: BİLEŞİK birincil anahtarlar ───────────────────────────────────────
// Fares v2 ve Flex dosyalarının çoğunun anahtarı bileşiktir; DQ_021 yalnız stop_id,
// route_id ve trip_id tekrarını tarıyordu. Anahtar listeleri spec'ten birebir alındı.

#[test]
fn dq021_detects_duplicate_composite_key_in_fare_leg_rules() {
    // Spec PK: (network_id, from_area_id, to_area_id, from_timeframe_group_id,
    //           to_timeframe_group_id, fare_product_id) — iki satır birebir aynı.
    static FLR: &[u8] = b"network_id,from_area_id,to_area_id,fare_product_id\n\
                          N1,A1,A2,P1\nN1,A1,A2,P1\n";
    let mut files = base_files();
    files.push(("fare_leg_rules.txt", FLR));
    match validate_bytes(&make_zip(&files), &ValidatorConfig::default(), TODAY) {
        ValidateResult::Ok(vr) => assert!(
            vr.notices.iter().any(|n| n.rule_id == "DQ_021"
                && n.file.as_deref() == Some("fare_leg_rules.txt")),
            "aynı bileşik anahtara sahip iki satır DQ_021 üretmeli"),
        ValidateResult::Fatal(e) => panic!("beklenmeyen fatal: {:?} {}", e.code, e.message),
    }
}

/// Anahtarın TEK bir alanı bile farklıysa satırlar ayrıdır — yanlış pozitif olmamalı.
#[test]
fn dq021_silent_when_one_composite_field_differs() {
    static FLR: &[u8] = b"network_id,from_area_id,to_area_id,fare_product_id\n\
                          N1,A1,A2,P1\nN1,A1,A3,P1\n";
    let mut files = base_files();
    files.push(("fare_leg_rules.txt", FLR));
    match validate_bytes(&make_zip(&files), &ValidatorConfig::default(), TODAY) {
        ValidateResult::Ok(vr) => assert!(
            !vr.notices.iter().any(|n| n.rule_id == "DQ_021"
                && n.file.as_deref() == Some("fare_leg_rules.txt")),
            "to_area_id farklı → ayrı anahtarlar, DQ_021 çıkmamalı"),
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
            assert!(vr.notices.iter().any(|n| n.rule_id == "DQ_021"
                && n.file.as_deref() == Some("location_groups.txt")),
                "yinelenen location_group_id DQ_021 üretmeli");
            assert!(vr.notices.iter().any(|n| n.rule_id == "DQ_021"
                && n.file.as_deref() == Some("location_group_stops.txt")),
                "yinelenen (location_group_id, stop_id) satırı DQ_021 üretmeli");
        }
        ValidateResult::Fatal(e) => panic!("beklenmeyen fatal: {:?} {}", e.code, e.message),
    }
}

// ── XFL_031: ortak kimlik isim alanı ──────────────────────────────────────────
// Spec (location_groups.location_group_id): "ID must be unique across all stops.stop_id,
// locations.geojson id, and location_groups.location_group_id values."

#[test]
fn xfl031_detects_location_group_id_clashing_with_stop_id() {
    // LG1 hem stops.txt'te hem location_groups.txt'te → stop_times.location_group_id
    // referansı belirsizleşir.
    static STOPS_CLASH: &[u8] = b"stop_id,stop_name,stop_lat,stop_lon\n\
        S1,Durak1,41.0,29.0\nS2,Durak2,41.1,29.1\nLG1,Cakisan,41.2,29.2\n";
    static LG: &[u8] = b"location_group_id,location_group_name\nLG1,Bolge\n";
    let mut files = base_files();
    files.retain(|(n, _)| *n != "stops.txt");
    files.push(("stops.txt", STOPS_CLASH));
    files.push(("location_groups.txt", LG));
    match validate_bytes(&make_zip(&files), &ValidatorConfig::default(), TODAY) {
        ValidateResult::Ok(vr) => assert!(
            vr.notices.iter().any(|n| n.rule_id == "XFL_031"
                && n.entity_id.as_deref() == Some("LG1")),
            "stop_id ile çakışan location_group_id XFL_031 üretmeli"),
        ValidateResult::Fatal(e) => panic!("beklenmeyen fatal: {:?} {}", e.code, e.message),
    }
}

/// Kimlikler ayrıksa notice çıkmamalı — kural sadece ÇAKIŞMAYI ölçer.
#[test]
fn xfl031_silent_when_ids_are_distinct() {
    static LG: &[u8] = b"location_group_id,location_group_name\nLG1,Bolge\n";
    let mut files = base_files();
    files.push(("location_groups.txt", LG));
    match validate_bytes(&make_zip(&files), &ValidatorConfig::default(), TODAY) {
        ValidateResult::Ok(vr) => assert!(
            !vr.notices.iter().any(|n| n.rule_id == "XFL_031"),
            "ayrık kimliklerde XFL_031 çıkmamalı"),
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
            vr.notices.iter().any(|n| n.rule_id == "XFL_031" && n.entity_id.as_deref() == Some("S1")),
            "stops.txt'teki S1 ile çakışan geojson id XFL_031 üretmeli"),
        ValidateResult::Fatal(e) => panic!("beklenmeyen fatal: {:?} {}", e.code, e.message),
    }
}

// ── LOC_008/009/010: locations.geojson zorunlu alanları ───────────────────────
// Spec (locations.geojson alan tablosu) her Feature için type / properties /
// geometry.coordinates alanlarını Required işaretler. Bunlar hiç denetlenmiyordu:
// eksikliklerinde kod sessizce atlıyor, dosya "doğrulandı" görünüyordu.

fn geojson_feed(geojson: &'static [u8]) -> Vec<(&'static str, &'static [u8])> {
    let mut files = base_files();
    files.push(("locations.geojson", geojson));
    files
}

#[test]
fn loc008_009_010_detect_missing_required_feature_members() {
    // type yok · properties yok · coordinates yok
    static BAD: &[u8] = br#"{"type":"FeatureCollection","features":[
        {"id":"z1","geometry":{"type":"Polygon"}}]}"#;
    match validate_bytes(&make_zip(&geojson_feed(BAD)), &ValidatorConfig::default(), TODAY) {
        ValidateResult::Ok(vr) => {
            let ids: Vec<&str> = vr.notices.iter().map(|n| n.rule_id.as_str()).collect();
            assert!(ids.contains(&"LOC_008"), "'type' eksik → LOC_008 çıkmalı: {ids:?}");
            assert!(ids.contains(&"LOC_009"), "'properties' eksik → LOC_009 çıkmalı: {ids:?}");
            assert!(ids.contains(&"LOC_010"), "'coordinates' eksik → LOC_010 çıkmalı: {ids:?}");
        }
        ValidateResult::Fatal(e) => panic!("beklenmeyen fatal: {:?} {}", e.code, e.message),
    }
}

/// Geçerli bir feature hiçbirini üretmemeli; boş `properties` nesnesi GEÇERLİDİR
/// (stop_name/stop_desc opsiyoneldir).
#[test]
fn loc008_009_010_silent_on_a_valid_feature() {
    static GOOD: &[u8] = br#"{"type":"FeatureCollection","features":[
        {"type":"Feature","id":"z1","properties":{},"geometry":{"type":"Polygon","coordinates":
        [[[29.0,41.0],[29.1,41.0],[29.1,41.1],[29.0,41.1],[29.0,41.0]]]}}]}"#;
    match validate_bytes(&make_zip(&geojson_feed(GOOD)), &ValidatorConfig::default(), TODAY) {
        ValidateResult::Ok(vr) => {
            let bad: Vec<&str> = vr.notices.iter().map(|n| n.rule_id.as_str())
                .filter(|r| matches!(*r, "LOC_008" | "LOC_009" | "LOC_010")).collect();
            assert!(bad.is_empty(), "geçerli feature notice üretmemeli: {bad:?}");
        }
        ValidateResult::Fatal(e) => panic!("beklenmeyen fatal: {:?} {}", e.code, e.message),
    }
}

// ── rule_priority yalnız fare_leg_rules.txt'in alanıdır ───────────────────────
// Spec (27 Nisan 2026): `rule_priority` fare_leg_rules.txt'te tanımlıdır; fare_transfer_rules.txt
// alan tablosunda YOKTUR. K1 ikisini de "bilinen sütun" sayıyordu, yani yanlış dosyaya konmuş
// bir rule_priority sütunu ARC_017 üretmiyordu (WP-2 spec çapa turunda yakalandı).

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
            assert!(hit, "fare_transfer_rules.rule_priority ARC_017 üretmeli, emit: {:?}",
                vr.notices.iter().map(|n| n.rule_id.as_str()).collect::<Vec<_>>());
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
            let hit = vr.notices.iter().any(|n| {
                n.rule_id == "ARC_017" && n.field.as_deref() == Some("rule_priority")
            });
            assert!(!hit, "fare_leg_rules.rule_priority resmî alandır, ARC_017 çıkmamalı");
        }
        other => panic!("ValidateResult::Ok beklendi, gelen: {other:?}"),
    }
}

// ── Bağlı sefer devamlılıkları: TRF_022 / TRF_023 ────────────────────────────
//
// Spec (`transfers.txt` → "Linked trips") ÜÇ cümle yazar ve üçüncüsü ilk ikisini SINIRLAR:
// devam seferlerinin `service_id`'si özdeş OLMALI, AMA bir sefer birden çok ayrı
// devamlılığa girebilir — yeter ki takvimler hiçbir günde çakışmasın. Veride devamlılık
// grubunu işaretleyen alan olmadığı için uygulanabilir tek okuma ikisinin bileşimidir.
// Aşağıdaki dört test tam olarak bu sınırı çiziyor: biri ateşlemeyi, ikisi SUSMAYI kanıtlıyor.

static CAL_TWO_SERVICES: &[u8] =
    b"service_id,monday,tuesday,wednesday,thursday,friday,saturday,sunday,start_date,end_date\n\
      SVC1,1,1,1,1,1,0,0,20250101,20271231\n\
      SVC2,1,1,1,1,1,1,1,20250101,20271231\n";
// SVC3 yalnız hafta sonu → SVC1 (hafta içi) ile HİÇBİR gün çakışmaz.
static CAL_DISJOINT: &[u8] =
    b"service_id,monday,tuesday,wednesday,thursday,friday,saturday,sunday,start_date,end_date\n\
      SVC1,1,1,1,1,1,0,0,20250101,20271231\n\
      SVC3,0,0,0,0,0,1,1,20250101,20271231\n";
static ST_THREE_TRIPS: &[u8] =
    b"trip_id,arrival_time,departure_time,stop_id,stop_sequence\n\
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
    // T1 hem T2'ye (SVC1, hafta içi) hem T3'e (SVC2, her gün) bağlanıyor.
    // Hafta içi günlerde İKİSİ birden aktif → hangi devamlılık geçerli belirsiz.
    let files = linked_trip_files(
        CAL_TWO_SERVICES,
        b"route_id,service_id,trip_id\nR1,SVC1,T1\nR1,SVC1,T2\nR1,SVC2,T3\n",
        b"from_stop_id,to_stop_id,transfer_type,from_trip_id,to_trip_id\nS1,S2,4,T1,T2\nS1,S2,4,T1,T3\n",
    );
    match run(&files) {
        ValidateResult::Ok(vr) => {
            assert!(has(&vr, "TRF_022"),
                "TRF_022 olmalı. Mevcut: {:?}",
                vr.notices.iter().map(|n| n.rule_id.as_str()).collect::<Vec<_>>());
            // Ters yön ateşlememeli: T2 ve T3'ün her birine tek sefer bağlanıyor.
            assert!(!has(&vr, "TRF_023"), "n-to-1 çelişkisi yok, TRF_023 çıkmamalı");
        }
        other => panic!("ValidateResult::Ok beklendi, gelen: {other:?}"),
    }
}

#[test]
fn trf022_silent_when_continuation_trips_share_one_service_id() {
    // GERÇEK 1-to-n ayrımı: T1 ikiye ayrılıyor ama ikisi de AYNI takvimde.
    // Spec'in istediği tam olarak bu — kural susmalı.
    let files = linked_trip_files(
        CAL_TWO_SERVICES,
        b"route_id,service_id,trip_id\nR1,SVC1,T1\nR1,SVC1,T2\nR1,SVC1,T3\n",
        b"from_stop_id,to_stop_id,transfer_type,from_trip_id,to_trip_id\nS1,S2,4,T1,T2\nS1,S2,4,T1,T3\n",
    );
    match run(&files) {
        ValidateResult::Ok(vr) => assert!(!has(&vr, "TRF_022"),
            "Özdeş service_id geçerli 1-to-n devamlılıktır, TRF_022 çıkmamalı"),
        other => panic!("ValidateResult::Ok beklendi, gelen: {other:?}"),
    }
}

#[test]
fn trf022_silent_when_separate_continuations_do_not_share_a_service_day() {
    // AYRI DEVAMLILIKLAR: T1 hafta içi T2'ye, hafta sonu T3'e devam ediyor.
    // service_id'ler FARKLI ama hiçbir gün çakışmıyor → spec bunu AÇIKÇA serbest bırakır.
    // Bu test olmasaydı kural, spec'in iznini norm sanan `PTH_017` hatasını tekrarlardı.
    let files = linked_trip_files(
        CAL_DISJOINT,
        b"route_id,service_id,trip_id\nR1,SVC1,T1\nR1,SVC1,T2\nR1,SVC3,T3\n",
        b"from_stop_id,to_stop_id,transfer_type,from_trip_id,to_trip_id\nS1,S2,4,T1,T2\nS1,S2,4,T1,T3\n",
    );
    match run(&files) {
        ValidateResult::Ok(vr) => assert!(!has(&vr, "TRF_022"),
            "Çakışmayan takvimler ayrı devamlılıktır, TRF_022 çıkmamalı. Mevcut: {:?}",
            vr.notices.iter().map(|n| n.rule_id.as_str()).collect::<Vec<_>>()),
        other => panic!("ValidateResult::Ok beklendi, gelen: {other:?}"),
    }
}

#[test]
fn trf023_flags_n_to_one_continuation_with_overlapping_calendars() {
    // T2 (SVC1) ve T3 (SVC2) ikisi de T1'e bağlanıyor → gelen taraf çelişiyor.
    let files = linked_trip_files(
        CAL_TWO_SERVICES,
        b"route_id,service_id,trip_id\nR1,SVC1,T1\nR1,SVC1,T2\nR1,SVC2,T3\n",
        b"from_stop_id,to_stop_id,transfer_type,from_trip_id,to_trip_id\nS1,S2,4,T2,T1\nS1,S2,4,T3,T1\n",
    );
    match run(&files) {
        ValidateResult::Ok(vr) => {
            assert!(has(&vr, "TRF_023"),
                "TRF_023 olmalı. Mevcut: {:?}",
                vr.notices.iter().map(|n| n.rule_id.as_str()).collect::<Vec<_>>());
            assert!(!has(&vr, "TRF_022"), "1-to-n çelişkisi yok, TRF_022 çıkmamalı");
        }
        other => panic!("ValidateResult::Ok beklendi, gelen: {other:?}"),
    }
}

// ── DQ_021 birincil anahtar — 2026-08-06'da eklenen üç dosya ─────────────────

#[test]
fn dq021_flags_calendar_dates_with_conflicting_exception_types() {
    // Aynı (service_id, date) hem exception_type=1 hem =2 → o günün servisi TANIMSIZ.
    // Birincil anahtar ihlalinin en ağır biçimi; hiçbir CLD_* kuralı bunu görmüyordu.
    static CLD: &[u8] =
        b"service_id,date,exception_type\nSVC1,20260701,1\nSVC1,20260701,2\n";
    let mut files = base_files();
    files.push(("calendar_dates.txt", CLD));
    match run(&files) {
        ValidateResult::Ok(vr) => {
            let hit = vr.notices.iter().find(|n| {
                n.rule_id == "DQ_021" && n.file.as_deref() == Some("calendar_dates.txt")
            });
            assert!(hit.is_some(),
                "calendar_dates için DQ_021 olmalı. Mevcut: {:?}",
                vr.notices.iter().map(|n| n.rule_id.as_str()).collect::<Vec<_>>());
        }
        other => panic!("ValidateResult::Ok beklendi, gelen: {other:?}"),
    }
}

#[test]
fn dq021_silent_when_calendar_dates_keys_are_unique() {
    static CLD: &[u8] =
        b"service_id,date,exception_type\nSVC1,20260701,2\nSVC1,20260702,2\n";
    let mut files = base_files();
    files.push(("calendar_dates.txt", CLD));
    match run(&files) {
        ValidateResult::Ok(vr) => assert!(
            !vr.notices.iter().any(|n| {
                n.rule_id == "DQ_021" && n.file.as_deref() == Some("calendar_dates.txt")
            }),
            "Benzersiz (service_id, date) çiftlerinde DQ_021 çıkmamalı"),
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
            let files_hit: Vec<&str> = vr.notices.iter()
                .filter(|n| n.rule_id == "DQ_021")
                .filter_map(|n| n.file.as_deref())
                .collect();
            assert!(files_hit.contains(&"stop_areas.txt"),
                "stop_areas.txt için DQ_021 olmalı. DQ_021 dosyaları: {files_hit:?}");
            assert!(files_hit.contains(&"fare_rules.txt"),
                "fare_rules.txt için DQ_021 olmalı. DQ_021 dosyaları: {files_hit:?}");
        }
        other => panic!("ValidateResult::Ok beklendi, gelen: {other:?}"),
    }
}

// ── PTH_031: stop_access=1 olan durak pathway uç noktası olamaz ──────────────
//
// Spec `pathways.from_stop_id`/`to_stop_id`: "Values for stop_id that identify stations
// (location_type=1), **or stops (location_type=0 or empty) with stop_access=1**, are
// forbidden." İkinci kol 2026-08-06'ya kadar ölçülmüyordu; katalog da göremiyordu çünkü
// yasak KÜÇÜK harfle ("are forbidden") yazılı.

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
        ValidateResult::Ok(vr) => assert!(has(&vr, "PTH_031"),
            "PTH_031 olmalı. Mevcut: {:?}",
            vr.notices.iter().map(|n| n.rule_id.as_str()).collect::<Vec<_>>()),
        other => panic!("ValidateResult::Ok beklendi, gelen: {other:?}"),
    }
}

#[test]
fn pth031_silent_when_endpoints_are_not_street_accessed() {
    // stop_access=0 = "sokaktan doğrudan erişilemez, pathway kullanılmalı" → tam da
    // pathway'in beklendiği hâl. Kural burada susmalı.
    match run(&pathway_files(STOPS_NO_ACCESS)) {
        ValidateResult::Ok(vr) => assert!(!has(&vr, "PTH_031"),
            "stop_access=0 pathway'in beklendiği hâldir, PTH_031 çıkmamalı"),
        other => panic!("ValidateResult::Ok beklendi, gelen: {other:?}"),
    }
}

#[test]
fn stp024_k2_range_and_stp026_normative_enum_are_distinct() {
    static STOPS_WITH_ENUMS: &[u8] =
        b"stop_id,stop_name,stop_lat,stop_lon,stop_access\n\
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
                2,
                "K4 normatif enumunda hem 2 hem 9 geçersiz olmalı"
            );
        }
        other => panic!("ValidateResult::Ok beklendi, alınan: {other:?}"),
    }
}

// ── Pc3b911a6: "Dosyalar virgülle ayrılmış metin olmalı" — VARSAYIM DEĞİL, ÖLÇÜM ────
//
// Defter bu hükmü DOLAYLI sayıyordu ("ayraç virgül değilse başlık tek sütuna çöker →
// ARC_025 fatal") ama gerekçenin yanında **[Varsayım] — bu yol ölçülmedi** yazıyordu.
// 2026-08-06 dış denetimi haklı olarak şunu söyledi: `badge_status.py` payı
// `KANITLI + DOLAYLI` diye hesaplıyor, yani test edilmemiş bir çıkarım "ölçülmüş"
// sayılıyordu. Varsayımı düşürmek yerine ÖLÇÜYORUZ.

#[test]
fn semicolon_delimited_files_are_rejected_pc3b911a6() {
    // Aynı içerik, ayraç NOKTALI VİRGÜL. Başlık tek sütuna çöker → zorunlu sütunlar
    // bulunamaz. Beklenen: ya Fatal(NoRequiredFiles) ya da ARC_025 (zorunlu sütun eksik).
    static SEMI_STOPS: &[u8] = b"stop_id;stop_name;stop_lat;stop_lon\nS1;Stop1;41.0;29.0\nS2;Stop2;41.1;29.1\n";
    let mut files = base_files();
    files.retain(|(n, _)| *n != "stops.txt");
    files.push(("stops.txt", SEMI_STOPS));

    match run(&files) {
        ValidateResult::Fatal(e) => {
            // Zorunlu dosya sütunları çözülemedi → fatal. Hüküm ÖLÇÜLÜYOR.
            assert_eq!(e.code, FatalCode::NoRequiredFiles,
                "virgül olmayan ayraç fatal üretmeli, gelen: {:?}", e.code);
        }
        ValidateResult::Ok(vr) => {
            assert!(has(&vr, "ARC_025"),
                "noktalı virgülle ayrılmış stops.txt ARC_025 üretmeli. Mevcut: {:?}",
                vr.notices.iter().map(|n| n.rule_id.as_str()).collect::<Vec<_>>());
        }
    }

    // 🔴 KONTROL GİRDİSİ (issue #87). İhlalli girdi tek başına yetmez: komşu kural HER
    // feed'de ateşliyorsa "ölçüm" bir şey kanıtlamaz. Aynı içerik VİRGÜLLE ayrılmış hâlde
    // sessiz kalmalı — fark, hükmün gerçekten ölçüldüğünü gösteren şeydir.
    match run(&base_files()) {
        ValidateResult::Ok(vr) => assert!(!has(&vr, "ARC_025"),
            "virgülle ayrılmış aynı içerik ARC_025 ÜRETMEMELİ (kontrol)"),
        other => panic!("kontrol girdisi Ok olmalı: {other:?}"),
    }
}

// ── Pd59e5eaa: ilk satır ALAN ADLARINI içermeli (issue #87 kanıt çifti) ────────
//
// Defter bu hükmü DOLAYLI sayıyordu ("başlık yerine veri satırı varsa ARC_025 + ARC_019 +
// ARC_017 birlikte ateşler") ama bu bir KOD OKUMASIYDI, ölçüm değil. #87 haklı: pay
// `KANITLI + DOLAYLI` diye hesaplanıyor, yani ölçülmemiş bir çıkarım sayılıyordu.

#[test]
fn first_line_must_be_a_header_pd59e5eaa() {
    // İHLAL: başlık satırı yok — dosya doğrudan veriyle başlıyor. O zaman ilk VERİ satırı
    // başlık sanılır, zorunlu sütun adları bulunamaz.
    static NO_HEADER: &[u8] = b"S1,Stop1,41.0,29.0
S2,Stop2,41.1,29.1
";
    let mut files = base_files();
    for slot in files.iter_mut() {
        if slot.0 == "stops.txt" { slot.1 = NO_HEADER; }
    }
    let violating_fires = match run(&files) {
        ValidateResult::Fatal(e) => e.code == FatalCode::NoRequiredFiles,
        ValidateResult::Ok(vr) => has(&vr, "ARC_025"),
    };
    assert!(violating_fires, "başlıksız dosya zorunlu sütun ihlali üretmeli");

    // KONTROL: aynı veri, başlık satırıyla → sessiz.
    match run(&base_files()) {
        ValidateResult::Ok(vr) => assert!(!has(&vr, "ARC_025"),
            "başlığı olan aynı içerik ARC_025 ÜRETMEMELİ (kontrol)"),
        other => panic!("kontrol girdisi Ok olmalı: {other:?}"),
    }
}

// ── LOC_011: OpenGIS SFS 6.1.11 poligon geçerliliği — genişletilmiş maddeler ────────
//
// 2026-08-06 dış denetimi `LOC_011`'in kartında yazılı dört boşluğu gösterdi. Üçü
// kapatıldı (delik-delik kesişimi · deliğin KISMEN dışarı taşması · spike/cut line);
// dördüncüsü (ring yönü) ölçülmüyor çünkü **6.1.11 yönelim şart koşmuyor** — o GeoJSON
// RFC 7946'nın serileştirme kuralıdır ve onu ihlal saymak yanlış pozitif üretirdi.

/// Tek feature'lık `locations.geojson` kurar; geometri gövdesi test başına verilir.
/// `Box::leak`: `run()` `&'static [u8]` bekliyor, test başına tek küçük sızıntı kabul.
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
    // İki delik üst üste biniyor → 6.1.11 (3) "no two Rings cross".
    assert!(loc011_fires(
        r#"{"type":"Polygon","coordinates":[
             [[0,0],[0,10],[10,10],[10,0],[0,0]],
             [[1,1],[1,5],[5,5],[5,1],[1,1]],
             [[3,3],[3,7],[7,7],[7,3],[3,3]]]}"#
    ), "birbirini kesen iki delik LOC_011 üretmeli");
}

#[test]
fn loc011_flags_hole_partially_outside_the_shell() {
    // Deliğin İLK NOKTASI kabuğun içinde ama gövdesi dışarı taşıyor. Eski predikat
    // yalnız ilk noktaya baktığı için bunu KAÇIRIYORDU.
    assert!(loc011_fires(
        r#"{"type":"Polygon","coordinates":[
             [[0,0],[0,10],[10,10],[10,0],[0,0]],
             [[5,5],[5,15],[8,15],[8,5],[5,5]]]}"#
    ), "kısmen dışarı taşan delik LOC_011 üretmeli");
}

#[test]
fn loc011_flags_spike() {
    // Kabuk bir doğru boyunca çıkıp aynı doğrudan dönüyor → sıfır alanlı çıkıntı.
    assert!(loc011_fires(
        r#"{"type":"Polygon","coordinates":[
             [[0,0],[0,10],[5,10],[20,10],[5,10],[10,10],[10,0],[0,0]]]}"#
    ), "spike/cut line LOC_011 üretmeli");
}

#[test]
fn loc011_silent_on_valid_polygon_with_hole() {
    // Tamamen geçerli: delik kabuğun içinde, kesişme yok, spike yok.
    assert!(!loc011_fires(
        r#"{"type":"Polygon","coordinates":[
             [[0,0],[0,10],[10,10],[10,0],[0,0]],
             [[2,2],[2,4],[4,4],[4,2],[2,2]]]}"#
    ), "geçerli poligon LOC_011 üretmemeli");
}

#[test]
fn loc011_silent_on_clockwise_shell() {
    // ⚠️ EN ÖNEMLİ TEST: ring yönü 6.1.11'in geçerlilik koşulu DEĞİLDİR. Saat yönünde
    // yazılmış kabuk GEÇERLİDİR; burada ateşlemek `PTH_017` hatasını tekrarlamak olurdu.
    assert!(!loc011_fires(
        r#"{"type":"Polygon","coordinates":[
             [[0,0],[10,0],[10,10],[0,10],[0,0]]]}"#
    ), "saat yönü kabuk 6.1.11'e göre GEÇERLİDİR, LOC_011 çıkmamalı");
}

// ── STM_060: geojson bölgesi + pencere + davranış EŞZAMANLI örtüşmesi ──────────────
//
// Hüküm ÜÇ koşulun BİRLİKTE sağlanmasını yasaklar. Testlerin çoğu bilerek NEGATİF:
// koşullardan biri düşerse kural SUSMALI, yoksa Flex feed'lerinde yanlış pozitif üretir.

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

// İki bölge: Z1 ve Z2 ÜST ÜSTE biniyor; Z3 tamamen ayrı.
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
    // Z1∩Z2 uzayda kesişiyor · 08:00-10:00 ile 09:00-11:00 zamanda kesişiyor · ikisi de biniş.
    static ST: &[u8] = b"trip_id,location_id,stop_sequence,start_pickup_drop_off_window,end_pickup_drop_off_window,pickup_type,drop_off_type\n\
        T1,Z1,1,08:00:00,10:00:00,2,2\nT1,Z2,2,09:00:00,11:00:00,2,2\n";
    assert!(stm060_fires(ST), "üç koşul da sağlanıyor → STM_060 çıkmalı");
}

#[test]
fn stm060_silent_when_zones_are_spatially_disjoint() {
    // Z1 ve Z3 hiç kesişmiyor → hüküm ihlal edilmiyor.
    static ST: &[u8] = b"trip_id,location_id,stop_sequence,start_pickup_drop_off_window,end_pickup_drop_off_window,pickup_type,drop_off_type\n\
        T1,Z1,1,08:00:00,10:00:00,2,2\nT1,Z3,2,09:00:00,11:00:00,2,2\n";
    assert!(!stm060_fires(ST), "ayrık bölgelerde STM_060 çıkmamalı");
}

#[test]
fn stm060_silent_when_time_windows_do_not_overlap() {
    // Bölgeler kesişiyor ama pencereler ardışık, çakışmıyor.
    static ST: &[u8] = b"trip_id,location_id,stop_sequence,start_pickup_drop_off_window,end_pickup_drop_off_window,pickup_type,drop_off_type\n\
        T1,Z1,1,08:00:00,10:00:00,2,2\nT1,Z2,2,10:00:00,12:00:00,2,2\n";
    assert!(!stm060_fires(ST), "çakışmayan pencerelerde STM_060 çıkmamalı");
}

#[test]
fn stm060_silent_when_behaviour_does_not_overlap() {
    // Bölge ve zaman çakışıyor ama biri YALNIZ biniş, diğeri YALNIZ iniş sunuyor.
    // pickup_type=1 → biniş yok · drop_off_type=1 → iniş yok.
    static ST: &[u8] = b"trip_id,location_id,stop_sequence,start_pickup_drop_off_window,end_pickup_drop_off_window,pickup_type,drop_off_type\n\
        T1,Z1,1,08:00:00,10:00:00,2,1\nT1,Z2,2,09:00:00,11:00:00,1,2\n";
    assert!(!stm060_fires(ST), "davranış örtüşmesi yoksa STM_060 çıkmamalı");
}

#[test]
fn stm060_silent_across_different_trips() {
    // Hüküm AYNI trip_id içindeki kayıtlar için. Farklı seferler serbesttir.
    static ST: &[u8] = b"trip_id,location_id,stop_sequence,start_pickup_drop_off_window,end_pickup_drop_off_window,pickup_type,drop_off_type\n\
        T1,Z1,1,08:00:00,10:00:00,2,2\nT2,Z2,1,09:00:00,11:00:00,2,2\n";
    assert!(!stm060_fires(ST), "farklı trip'lerde STM_060 çıkmamalı");
}

#[test]
fn stm060_silent_for_two_records_in_the_same_zone() {
    // 🔴 EN ÖNEMLİ NEGATİF TEST. Spec'in KENDİSİ bunu zorunlu kılıyor:
    // *"Travel within the same location group or GeoJSON location requires two records in
    // stop_times.txt with the same location_group_id or location_id."*
    // Aynı bölgeye ait iki kayıt bölge-İÇİ seyahatin tek ifade biçimidir.
    //
    // İlk uygulamam bunu ihlal sayıyordu ve `ch-odv-flex`/`mdb-2053`'te **21 yanlış pozitif**
    // üretti; MobilityData aynı kuralı (`overlapping_zone_and_pickup_drop_off_window`)
    // uyguladığı hâlde o feed'de SUSUYOR. Spec'in bir yerde zorunlu kıldığını başka yerde
    // yasak saymak `PTH_017` hatasının aynısıdır.
    static ST: &[u8] = b"trip_id,location_id,stop_sequence,start_pickup_drop_off_window,end_pickup_drop_off_window,pickup_type,drop_off_type\n\
        T1,Z1,1,07:00:00,11:00:00,2,2\nT1,Z1,2,07:00:00,11:00:00,2,2\n";
    assert!(!stm060_fires(ST),
        "aynı bölgedeki iki kayıt bölge-içi seyahattir, STM_060 çıkmamalı");
}

#[test]
fn loc011_flags_hole_touching_shell_at_two_points() {
    // OpenGIS 6.1.11 madde 5: iç kısım BAĞLANTILI olmalı. Delik dış ring'e İKİ noktada
    // dokunursa poligonun içi o temaslar arasında ikiye bölünür.
    // ⚠️ Madde 3 TEK noktada teğetliğe izin verir → aşağıdaki negatif test onu korur.
    assert!(loc011_fires(
        r#"{"type":"Polygon","coordinates":[
             [[0,0],[0,10],[10,10],[10,0],[0,0]],
             [[0,3],[5,5],[0,7],[0,3]]]}"#
    ), "kabuğa iki noktada dokunan delik iç kısmı böler → LOC_011");
}

#[test]
fn loc011_silent_when_hole_touches_shell_at_one_point() {
    // Tek noktada teğetlik 6.1.11'in AÇIKÇA izin verdiği hâldir. Burada ateşlemek
    // `PTH_017` hatasını tekrarlamak olurdu.
    assert!(!loc011_fires(
        r#"{"type":"Polygon","coordinates":[
             [[0,0],[0,10],[10,10],[10,0],[0,0]],
             [[0,5],[3,7],[3,3],[0,5]]]}"#
    ), "tek noktada teğet delik 6.1.11'e göre GEÇERLİDİR");
}

#[test]
fn loc011_flags_two_holes_touching_at_two_points() {
    assert!(loc011_fires(
        r#"{"type":"Polygon","coordinates":[
             [[0,0],[0,20],[20,20],[20,0],[0,0]],
             [[2,2],[2,8],[8,8],[8,2],[2,2]],
             [[8,2],[8,8],[14,8],[14,2],[8,2]]]}"#
    ), "iki noktada dokunan iki delik iç kısmı böler → LOC_011");
}

// ── ARC_022: dosya satır sayısı eşiği — ÜÇ emit noktası ───────────────────────
//
// Issue #71. Eşik 2026-08-07'ye kadar üç yerde SABİT KODLUYDU (`k1_parse`, `k2::shapes`,
// `k2::stop_times`) ve K1'in `parse()`'ı `ValidatorConfig` almadığı için kurala fixture
// yazılamıyordu — borç defteri bunu "1M satır gerektirir, inline yazılamaz" diye
// kaydetmişti. Doğru gerekçe "imza config almıyor" idi; imza değişti.
//
// ⚠️ Fixture (emit_proof) kuralın YALNIZ K1 yolunu kanıtlar. Üç emit noktasının üçü de
// aynı config alanını okumalı; burada üçü ayrı ayrı ölçülür, ayrıca eşiğe EŞİT satır
// sayısında sustuğu (sınır davranışı) doğrulanır.
//
// ⚠️ BİLİNEN BOŞLUK (bu turda KAPATILMADI): `trips.txt` ve `calendar_dates.txt` K1'de
// stream edilir (`rows` boş kalır) ve K2'de ARC_022 emit noktaları YOKTUR → bu iki dosya
// kaç satır olursa olsun kural susar. Kapatmak yeni bulgular üretir (8M satırlık
// calendar_dates taşıyan feed'ler var), yani davranış değişikliğidir; #71'in kapsamı
// kanıttır, kapsam genişletmesi değil.

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

/// `file` dosyasını `n` satırla kurup ARC_022'nin O DOSYA için çıkıp çıkmadığını döner.
/// Eşik `max_file_rows = 10` (config alt sınırı) — 1M satırlık zip üretmeye gerek yok.
fn arc022_fires_for(file: &str, n: usize) -> bool {
    let body = arc022_rows(file, n);
    let mut files = base_files();
    if let Some(slot) = files.iter_mut().find(|(name, _)| *name == file) {
        slot.1 = Box::leak(body.clone().into_bytes().into_boxed_slice());
    } else {
        files.push((Box::leak(file.to_string().into_boxed_str()),
                    Box::leak(body.clone().into_bytes().into_boxed_slice())));
    }
    let cfg = ValidatorConfig { max_file_rows: 10, ..ValidatorConfig::default() };
    match validate_bytes(&make_zip(&files), &cfg, TODAY) {
        ValidateResult::Ok(vr) => vr.notices.iter().any(|n| {
            n.rule_id == "ARC_022" && n.entity_id.as_deref() == Some(file)
        }),
        other => panic!("fatal beklenmiyordu: {other:?}"),
    }
}

#[test]
fn arc022_fires_on_k1_non_streamed_file() {
    // stops.txt akış-dışı okunur → eşik k1_parse'ta okunur.
    assert!(arc022_fires_for("stops.txt", 11),
        "11 satır > eşik 10 → stops.txt için ARC_022 çıkmalı");
}

#[test]
fn arc022_fires_on_k2_stop_times_stream() {
    // stop_times.txt K1'de stream edilir; ARC_022'yi K2 üretir (rows K1'de boştur).
    assert!(arc022_fires_for("stop_times.txt", 11),
        "11 satır > eşik 10 → stop_times.txt için ARC_022 çıkmalı");
}

#[test]
fn arc022_fires_on_k2_shapes_stream() {
    assert!(arc022_fires_for("shapes.txt", 11),
        "11 satır > eşik 10 → shapes.txt için ARC_022 çıkmalı");
}

// ── issue #75: akış dosyalarının KALAN İKİSİ ────────────────────────────────
// `trips.txt` ve `calendar_dates.txt` de K1'de stream edilir ama K2'de emit noktaları
// YOKTU → kural onlarda kaç satır olursa olsun susuyordu. Denetim artık `k2/mod.rs`'te
// TEK geçişte; bu iki test o geçişin kapsadığını sabitler.

#[test]
fn arc022_fires_on_k2_trips_stream() {
    assert!(arc022_fires_for("trips.txt", 11),
        "11 satır > eşik 10 → trips.txt için ARC_022 çıkmalı");
}

#[test]
fn arc022_fires_on_k2_calendar_dates_stream() {
    assert!(arc022_fires_for("calendar_dates.txt", 11),
        "11 satır > eşik 10 → calendar_dates.txt için ARC_022 çıkmalı");
}

#[test]
fn arc022_silent_at_the_threshold_on_every_path() {
    // Sınır: eşiğe EŞİT satır sayısı ihlal DEĞİLDİR (`>` karşılaştırması).
    for file in ["stops.txt", "stop_times.txt", "shapes.txt", "trips.txt", "calendar_dates.txt"] {
        assert!(!arc022_fires_for(file, 10),
            "{file}: eşiğe eşit (10) satır ARC_022 üretmemeli");
    }
}

#[test]
fn arc022_notice_order_is_stable_across_runs() {
    // ⚠️ Bu test K2'deki sıralamanın bekçisi DEĞİLDİR — 2026-08-07'de ölçüldü: alfabetik
    // çıktıyı asıl K7 garanti ediyor (`notice_order_key` ARC_022'yi `entity_id`'ye göre
    // sıralar), K2'deki `sort` kaldırılınca da bu test geçiyordu. O sıralamayı denetleyen
    // birim testi `k2/mod.rs::files_over_row_limit_is_alphabetical_and_strict`.
    // Buradaki iddia daha dar ama gerçek: uçtan uca çıktı sırası KARARLI ve alfabetik.
    let mut files = base_files();
    for name in ["shapes.txt", "trips.txt", "calendar_dates.txt"] {
        let body = Box::leak(arc022_rows(name, 11).into_bytes().into_boxed_slice());
        match files.iter_mut().find(|(n, _)| *n == name) {
            Some(slot) => slot.1 = body,
            None => files.push((name, body)),
        }
    }
    let cfg = ValidatorConfig { max_file_rows: 10, ..ValidatorConfig::default() };
    let zip = make_zip(&files);
    let order = |vr: &gtfs_core::ValidationResult| -> Vec<String> {
        vr.notices.iter().filter(|n| n.rule_id == "ARC_022")
            .map(|n| n.entity_id.clone().unwrap_or_default()).collect()
    };
    let first = match validate_bytes(&zip, &cfg, TODAY) {
        ValidateResult::Ok(vr) => order(&vr),
        other => panic!("fatal beklenmiyordu: {other:?}"),
    };
    assert_eq!(first, vec!["calendar_dates.txt", "shapes.txt", "trips.txt"],
        "ARC_022 bulguları dosya adına göre sıralı gelmeli");
    for _ in 0..5 {
        let again = match validate_bytes(&zip, &cfg, TODAY) {
            ValidateResult::Ok(vr) => order(&vr),
            other => panic!("fatal beklenmiyordu: {other:?}"),
        };
        assert_eq!(again, first, "aynı girdi aynı sırayı vermeli");
    }
}

// ── LOC_011 · 6.1.11 madde 5'in KALINTISI: temas grafiği çevrimi (issue #74) ───
//
// 2026-08-07'ye kadar ölçülen ölçüt "iki ring İKİ noktada dokunuyor" idi; bu, temas
// grafiğinin en kısa çevrimi (iki paralel kenar). Kalıntı: üç ring ikişer ikişer BİRER
// noktada dokunarak da çevrim kapatabilir ve iç kısmı böler. Ölçüt artık çevrimin
// kendisidir — özel hâl kendiliğinden kapsanır.

#[test]
fn loc011_flags_three_holes_touching_pairwise_in_a_chain() {
    // A–B, B–C, C–A: her temas TEK nokta (madde 3 tek tek hepsine izin verir) ama üçü
    // birlikte çevrim kapatır → aralarında kalan alan iç kısmın geri kalanından KOPAR.
    // Delikler kabuğa DOKUNMAZ; çevrim yalnız delikler arasındadır.
    assert!(loc011_fires(
        r#"{"type":"Polygon","coordinates":[
             [[0,0],[0,20],[20,20],[20,0],[0,0]],
             [[2,2],[12,2],[2,12],[2,2]],
             [[18,4],[18,12],[12,2],[18,4]],
             [[4,18],[2,12],[18,12],[4,18]]]}"#
    ), "üçgen temas zinciri iç kısmı böler → LOC_011");
}

#[test]
fn loc011_silent_on_open_chain_of_single_point_contacts() {
    // A–B ve B–C birer noktada dokunuyor, C–A DOKUNMUYOR → grafik AĞAÇ, çevrim yok.
    // Kıstırma noktalarının etrafından dolaşılabildiği için iç kısım BAĞLANTILI kalır.
    // Bu, çevrim ölçütünün yanlış pozitife dönüşmediğinin kanıtıdır (`PTH_017` dersi).
    assert!(!loc011_fires(
        r#"{"type":"Polygon","coordinates":[
             [[0,0],[0,20],[20,20],[20,0],[0,0]],
             [[2,2],[12,2],[2,12],[2,2]],
             [[18,4],[18,12],[12,2],[18,4]],
             [[4,18],[6,14],[18,12],[4,18]]]}"#
    ), "kapanmayan temas zinciri iç kısmı bölmez, LOC_011 çıkmamalı");
}

#[test]
fn loc011_flags_ring_that_touches_itself() {
    // Sekiz şeklinde kabuk: (5,5) köşesine İKİ kez uğruyor. `ring_is_simple` bunu
    // göremez — teğetlik bilinçli olarak "kesişme" sayılmaz — ama iç kısım iki loba
    // ayrılır, yani madde 5 ihlalidir (temas grafiğinde ÖZ-DÖNGÜ).
    assert!(loc011_fires(
        r#"{"type":"Polygon","coordinates":[
             [[0,0],[10,0],[5,5],[10,10],[0,10],[5,5],[0,0]]]}"#
    ), "kendine dokunan ring iç kısmı böler → LOC_011");
}

#[test]
fn loc011_silent_on_duplicate_consecutive_vertex() {
    // ARDIŞIK yinelenen koordinat (sıfır uzunluklu kenar) gerçek veride yaygındır ve
    // öz-temas DEĞİLDİR. Öz-temas denetimi komşu indeksleri atlamasaydı bu yaygın
    // özensizlik anında yanlış pozitif olurdu.
    assert!(!loc011_fires(
        r#"{"type":"Polygon","coordinates":[
             [[0,0],[0,10],[0,10],[10,10],[10,0],[0,0]]]}"#
    ), "ardışık yinelenen köşe öz-temas değildir, LOC_011 çıkmamalı");
}

#[test]
fn loc011_silent_on_collinear_vertex_on_a_straight_edge() {
    // Düz kenar üzerindeki ara köşe ((0,5)) kendi komşu kenarlarının üstündedir;
    // öz-temas SAYILMAZ. Aksi hâlde her yoğunlaştırılmış (densified) sınır patlardı.
    assert!(!loc011_fires(
        r#"{"type":"Polygon","coordinates":[
             [[0,0],[0,5],[0,10],[10,10],[10,0],[0,0]]]}"#
    ), "düz kenar üzerindeki ara köşe öz-temas değildir, LOC_011 çıkmamalı");
}

// ── ARC_033: RFC 4180 tırnak kuralı ────────────────────────────────────────────
//
// 🔴 İlk gerçek FALSIFIER (2026-08-07, dış okuma). Tokenizer'ın tırnaksız alan dalı yalnız
// `,`, LF ve CR'de duruyordu; `"` sıradan karakter sayılıyordu. Yani spec'in İKİ sert
// hükmü (`Pfedb83cf`, `Pa4c60372`) defterde `ARC_013` üzerinden DOLAYLI sayılırken ihlal
// gerçekte HİÇ görünmüyordu — ölçüldü: bulgu kümesi geçerli feed'le BİREBİR aynıydı.
//
// ⚠️ Kuralın ÖLÇÜLEBİLİR olması alan sınırlarını bilmeyi gerektirir: `"12"" Street"`
// (geçerli) ile `12" Street` (ihlal) AYNI değeri üretir. Bu yüzden karar tokenizer'da.

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
    assert!(arc033_fires(
        "stop_id,stop_name,stop_lat,stop_lon\nS1,12\" Street,41.0,29.0\nS2,Stop2,41.1,29.1\n"
    ), "tırnaksız alandaki tırnak RFC 4180 ihlalidir → ARC_033");
}

#[test]
fn arc033_flags_characters_after_a_closing_quote() {
    // `"Broadway"Ave` — içteki tırnak KAÇIRILMAMIŞ. Tokenizer bugüne dek kaydı sessizce
    // bölüyordu; ARC_012 çıkıyordu ama sebep hiçbir yerde yazmıyordu.
    assert!(arc033_fires(
        "stop_id,stop_name,stop_lat,stop_lon\nS1,\"Broadway\"Ave,41.0,29.0\nS2,Stop2,41.1,29.1\n"
    ), "kapanış tırnağından sonra karakter RFC 4180 ihlalidir → ARC_033");
}

#[test]
fn arc033_silent_on_correctly_escaped_quotes() {
    // 🔴 YANLIŞ POZİTİF KORUYUCUSU. Bu satır ayrıştırıldığında değer `12" Street` olur —
    // yani ihlalli hâlle AYNI. Kural değere baksaydı burada da ateşlerdi ve GEÇERLİ veriyi
    // reddederdi (`PTH_017` hatası).
    assert!(!arc033_fires(
        "stop_id,stop_name,stop_lat,stop_lon\nS1,\"12\"\" Street\",41.0,29.0\nS2,Stop2,41.1,29.1\n"
    ), "doğru kaçırılmış tırnak GEÇERLİDİR, ARC_033 çıkmamalı");
}

#[test]
fn arc033_silent_on_quoted_value_containing_a_comma() {
    // Virgül içeren değer tırnak içindeyse kural karşılanmıştır.
    assert!(!arc033_fires(
        "stop_id,stop_name,stop_lat,stop_lon\nS1,\"Main St, North\",41.0,29.0\nS2,Stop2,41.1,29.1\n"
    ), "tırnaklanmış virgül GEÇERLİDİR, ARC_033 çıkmamalı");
}

#[test]
fn arc033_silent_on_a_feed_with_no_quotes_at_all() {
    assert!(!arc033_fires(
        "stop_id,stop_name,stop_lat,stop_lon\nS1,Stop1,41.0,29.0\nS2,Stop2,41.1,29.1\n"
    ), "tırnaksız temiz feed ARC_033 üretmemeli");
}

#[test]
fn arc033_reports_one_summary_per_file_not_one_per_row() {
    // ARC_032/DQ_016 deseni: 1176 durak adı işaretli bir feed'de satır başına emit
    // tarayıcıyı boğar. Özet, kaç SATIR etkilendiğini söyler.
    let mut stops = String::from("stop_id,stop_name,stop_lat,stop_lon\n");
    for i in 1..=5 {
        stops.push_str(&format!("S{i},{i}\" Street,41.{i:03},29.{i:03}\n"));
    }
    match run(&arc033_feed(&stops)) {
        ValidateResult::Ok(vr) => {
            let hits: Vec<_> = vr.notices.iter().filter(|n| n.rule_id == "ARC_033").collect();
            assert_eq!(hits.len(), 1, "dosya başına TEK özet bekleniyor, gelen: {}", hits.len());
            let observed = hits[0].observed_value.clone().unwrap_or_default();
            assert!(observed.starts_with("5 satır"), "özet satır sayısını taşımalı: {observed}");
        }
        other => panic!("Ok beklendi: {other:?}"),
    }
}

#[test]
fn arc033_covers_streamed_files_too() {
    // ⚠️ `trips.txt` K1'de STREAM edilir (`rows` boş kalır) — tokenize_csv yalnız başlığı
    // görür. Gövde taraması `csv_scan`'de; iki tarayıcı ayrışırsa aynı dosya akış modunda
    // sessiz kalırdı. Bu, ARC_022'de yaşanan kapsam boşluğunun aynısı olurdu.
    let mut files = base_files();
    let trips: &'static [u8] = Box::leak(
        "route_id,service_id,trip_id,trip_headsign\nR1,SVC1,T1,5\" Line\n"
            .to_string().into_bytes().into_boxed_slice());
    for slot in files.iter_mut() {
        if slot.0 == "trips.txt" {
            slot.1 = trips;
        }
    }
    match run(&files) {
        ValidateResult::Ok(vr) => assert!(
            vr.notices.iter().any(|n| n.rule_id == "ARC_033" && n.entity_id.as_deref() == Some("trips.txt")),
            "akış modundaki trips.txt için de ARC_033 çıkmalı"),
        other => panic!("Ok beklendi: {other:?}"),
    }
}

// ── P71943a6f: dosya ve alan adları büyük/küçük harfe DUYARLIDIR ───────────────
//
// Spec (File Requirements): *"All file and field names are case-sensitive."* Bu cümle
// modal taşımadığı için 2026-08-07'ye kadar KATALOGDA HİÇ YOKTU (issue #81) — yani
// adjudike de öksüz de sayılamıyordu. Davranış zaten doğruydu ama YAZILI DEĞİLDİ;
// bu testler onu sabitler.

#[test]
fn case_sensitivity_wrong_case_file_is_not_the_required_file() {
    // `Stops.txt` ≠ `stops.txt`. Yanlış-case dosya zorunlu dosya SAYILMAZ → PARTIAL.
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
    // `STOP_ID` ≠ `stop_id` → zorunlu sütun YOK (ARC_025) + bilinmeyen sütun (ARC_017).
    let files = arc033_feed(
        "STOP_ID,stop_name,stop_lat,stop_lon\nS1,A,41.0,29.0\nS2,B,41.1,29.1\n");
    match run(&files) {
        ValidateResult::Ok(vr) => {
            assert!(has(&vr, "ARC_025"), "yanlış-case zorunlu sütun ARC_025 üretmeli");
            assert!(has(&vr, "ARC_017"), "yanlış-case sütun bilinmeyen sütun olarak da görünmeli");
        }
        other => panic!("Ok beklendi: {other:?}"),
    }
}

#[test]
fn case_sensitivity_correct_case_stays_silent() {
    // Koruyucu: doğru yazım bu iki kuralı ÜRETMEMELİ.
    let files = arc033_feed(
        "stop_id,stop_name,stop_lat,stop_lon\nS1,A,41.0,29.0\nS2,B,41.1,29.1\n");
    match run(&files) {
        ValidateResult::Ok(vr) => {
            assert!(!has(&vr, "ARC_025"), "doğru yazımda ARC_025 çıkmamalı");
            assert!(!has(&vr, "ARC_017"), "doğru yazımda ARC_017 çıkmamalı");
        }
        other => panic!("Ok beklendi: {other:?}"),
    }
}

// ── issue #85: ID alanları PK/FK öncesi SESSİZCE kırpılıyordu ──────────────────
//
// 🔴 5. denetim turu. `get_trimmed_field`/`get_col` her değeri kırpıyor, ID'ler o
// kırpılmış değerden kuruluyordu. `" A "` ile `A` aynı SÖZLÜKSEL değer değildir; PK/FK
// sert semantiktir. `main = 44d516d8` üzerinde ölçülen iki hâl:
//   1) stops `" A "` + stop_times FK `A` → eşleşiyordu (uydurma FK BAŞARISI)
//   2) stops'ta `A` ve `" A "` → `STP_001` duplicate (uydurma ÇAKIŞMA)
// Fazladan boşluk ayrı bir kalite sinyalidir ve `DQ_016` onu zaten bildiriyor.

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
    // stops yalnız `" A "` tanımlıyor; stop_times `A`ya atıf yapıyor → FK KARŞILANMAMALI.
    let files = id_ws_feed(
        "stop_id,stop_name,stop_lat,stop_lon\n\" A \",Bosluklu,41.0,29.0\nS2,Iki,41.1,29.1\n",
        "trip_id,arrival_time,departure_time,stop_id,stop_sequence\n\
         T1,08:00:00,08:00:00,A,1\nT1,08:10:00,08:10:00,S2,2\n",
    );
    match run(&files) {
        ValidateResult::Ok(vr) => assert!(
            vr.notices.iter().any(|n| n.rule_id == "STM_002"),
            "`A` referansı `\" A \"` durağıyla karşılanmamalı → STM_002 (stop_id stops.txt'te yok)"),
        other => panic!("Ok beklendi: {other:?}"),
    }
}

#[test]
fn id85_padded_and_bare_ids_are_not_duplicates() {
    // `A` ve `" A "` FARKLI kimliklerdir; duplicate raporlamak uydurma çakışmadır.
    let files = id_ws_feed(
        "stop_id,stop_name,stop_lat,stop_lon\nA,Bir,41.0,29.0\n\" A \",Iki,41.1,29.1\n",
        "trip_id,arrival_time,departure_time,stop_id,stop_sequence\n\
         T1,08:00:00,08:00:00,A,1\nT1,08:10:00,08:10:00,A,2\n",
    );
    match run(&files) {
        ValidateResult::Ok(vr) => assert!(
            !vr.notices.iter().any(|n| n.rule_id == "STP_001"),
            "farklı sözlüksel kimlikler duplicate DEĞİLDİR"),
        other => panic!("Ok beklendi: {other:?}"),
    }
}

#[test]
fn id85_whitespace_only_id_still_reports_the_required_field() {
    // ⚠️ SINIR: boşluk teşhisi kırpma KULLANIR ama kimliği değiştirmez. Yalnızca
    // boşluktan oluşan `stop_id` hâlâ "zorunlu alan boş" (`STP_002`) saymalı — aksi
    // hâlde bu düzeltme bir bulgu KAYBEDERDİ. Ayrıca fazladan boşluk kalite sinyali
    // (`DQ_016`) ayrı yoldan bildirilmeye devam eder.
    let files = id_ws_feed(
        "stop_id,stop_name,stop_lat,stop_lon\n\"   \",Bosluk,41.0,29.0\nS2,Iki,41.1,29.1\n",
        "trip_id,arrival_time,departure_time,stop_id,stop_sequence\n\
         T1,08:00:00,08:00:00,S2,1\nT1,08:10:00,08:10:00,S2,2\n",
    );
    match run(&files) {
        ValidateResult::Ok(vr) => {
            assert!(has(&vr, "STP_002"), "yalnız boşluktan oluşan stop_id zorunlu-alan ihlalidir");
            assert!(has(&vr, "DQ_016"), "fazladan boşluk AYRI kalite sinyali olarak kalmalı");
        }
        other => panic!("Ok beklendi: {other:?}"),
    }
}

// ── issue #112: whitespace kökü, strict lexical semantik ve cascade kapısı ────

#[test]
fn whitespace112_keeps_root_and_independent_errors_but_suppresses_derivatives() {
    let files = vec![
        ("agency.txt", AGENCY),
        // S1'in 91.0 enlemindeki aralık ihlali whitespace'ten bağımsızdır ve korunur;
        // S2'nin 41.1 değeri ise yalnızca whitespace köküne indirgenebilir.
        ("stops.txt", b"stop_id,stop_name,stop_lat,stop_lon\nS1,Stop1, 91.0,181.0\nS2,Stop2, 41.1,29.1\n" as &[u8]),
        // R1'in 99 değeri semantik olarak da geçersizdir; R2'nin 3 değeri yalnızca
        // lexical whitespace taşır. İki bağımsız URL hatası ayrıca görünür kalır.
        ("routes.txt", b"route_id,agency_id,route_short_name,route_type,route_url\n\
          R1,1,101, 99,ftp://bad.example\nR2,1,102, 3,ftp://bad.example\n" as &[u8]),
        ("trips.txt", TRIPS),
        // " S1" ham FK değeridir. Kök bulgu kanıtı bu değeri korur; STM_002 ise
        // yalnızca aynı kaydın trim edilmiş karşılığı stops.txt'te bulunduğu için türetilmiştir.
        // Bilinmeyen " UNKNOWN " değeri ise gerçek FK hatası olarak kalmalıdır.
        ("stop_times.txt", b"trip_id,arrival_time,departure_time,stop_id,stop_sequence\n\
          T1,08:00:00,08:00:00,\" S1\",1\nT1,08:10:00,08:10:00,S2,2\nT1,08:20:00,08:20:00,\" UNKNOWN \",3\n" as &[u8]),
        ("calendar.txt", CALENDAR),
    ];

    match run(&files) {
        ValidateResult::Ok(vr) => {
            let routes_root = vr.notices.iter()
                .find(|n| n.rule_id == "DQ_016" && n.file.as_deref() == Some("routes.txt"))
                .expect("routes.txt için DQ_016 kök bulgusu");
            let evidence = routes_root.details.as_ref()
                .and_then(|d| d.get("raw_samples"))
                .cloned()
                .unwrap_or_default();
            assert!(evidence.contains("route_type=\" 99\""),
                "ham enum değeri korunmalı: {evidence}");
            assert!(routes_root.details.as_ref()
                .and_then(|d| d.get("suppressed_derivative_rules"))
                .is_some_and(|rules| rules.contains("RTS_004")),
                "typed route_type türevi audit özetinde görünmeli");
            assert_eq!(vr.notices.iter().filter(|n| n.rule_id == "RTS_004").count(), 1,
                "trim sonrası bağımsız geçersiz enum korunmalı, yalnız R2 türevi bastırılmalı");
            assert!(vr.notices.iter().any(|n| n.rule_id == "RTS_005"),
                "aynı route entity'deki bağımsız URL hatası korunmalı");

            let stops_root = vr.notices.iter()
                .find(|n| n.rule_id == "DQ_016" && n.file.as_deref() == Some("stops.txt"))
                .expect("stops.txt için DQ_016 kök bulgusu");
            assert!(stops_root.details.as_ref()
                .and_then(|d| d.get("suppressed_derivative_rules"))
                .is_some_and(|rules| rules.contains("STP_004")),
                "numeric parse türevi audit özetinde görünmeli");
            assert_eq!(vr.notices.iter().filter(|n| n.rule_id == "STP_004").count(), 1,
                "trim sonrası bağımsız geçersiz enlem korunmalı, yalnız S2 türevi bastırılmalı");
            assert!(vr.notices.iter().any(|n| n.rule_id == "STP_005"),
                "aynı stop entity'deki bağımsız longitude aralık hatası korunmalı");

            let stop_times_root = vr.notices.iter()
                .find(|n| n.rule_id == "DQ_016" && n.file.as_deref() == Some("stop_times.txt"))
                .expect("stop_times.txt için DQ_016 kök bulgusu");
            let stm_details = stop_times_root.details.as_ref().expect("audit details");
            assert!(stm_details.get("raw_samples").is_some_and(|v| v.contains("stop_id=\" S1\"")),
                "ham PK/FK değeri kanıtta korunmalı");
            assert!(stm_details.get("suppressed_derivative_rules")
                .is_some_and(|rules| rules.contains("STM_002")),
                "FK türevi audit özetinde görünmeli");
            let stm_fk: Vec<_> = vr.notices.iter().filter(|n| n.rule_id == "STM_002").collect();
            assert_eq!(stm_fk.len(), 1,
                "trim edilmiş hedefin türevi bastırılmalı, bilinmeyen FK hatası korunmalı");
            assert!(stm_fk[0].observed_value.as_deref().is_some_and(|v| v.contains("UNKNOWN")),
                "korunan STM_002 bilinmeyen ham FK değerini göstermeli: {:?}", stm_fk[0]);
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
            assert!(has(&vr, "DQ_016"), "pathways.txt whitespace kök bulgusu korunmalı");
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

/// #126: `suppressed_derivative_*` sayacı OKUYUCU REJİMİNİ tarif eder, feed'i değil —
/// `trips.txt` `get_col` ile trim ederek okur, orada türev hiç doğmaz ve anahtar hiç
/// yazılmaz. Anahtarın yokluğu "burada gizlenmiş bir şey yok" diye okunabiliyordu.
/// `typed_whitespace_fields` iki rejimde de üretilir ve boşluğun tipli bir değeri
/// gizlediği yeri adıyla söyler.
#[test]
fn dq016_reports_typed_whitespace_in_both_reader_regimes() {
    let mut files = base_files();
    for slot in files.iter_mut() {
        // RowMap yolu: ham okunur, türev doğar, K7 bastırır.
        // `route_long_name` de boşluklu ama METİNDİR: trim edilse de sayı olmaz, bu yüzden
        // listeye girmemeli — aksi halde anahtar "boşluklu her alan" demeye başlar.
        if slot.0 == "routes.txt" {
            slot.1 = b"route_id,agency_id,route_short_name,route_long_name,route_type\n\
                       R1,1,101, Ana Hat , 3\n" as &[u8];
        }
        // get_col yolu: trim edilir, türev HİÇ doğmaz.
        if slot.0 == "trips.txt" {
            slot.1 = b"route_id,service_id,trip_id,wheelchair_accessible\nR1,SV1,T1, 1 \n" as &[u8];
        }
    }

    match run(&files) {
        ValidateResult::Ok(vr) => {
            let details = |file: &str| {
                vr.notices.iter()
                    .find(|n| n.rule_id == "DQ_016" && n.file.as_deref() == Some(file))
                    .unwrap_or_else(|| panic!("{file} için DQ_016 kök bulgusu"))
                    .details.as_ref()
                    .unwrap_or_else(|| panic!("{file} DQ_016 kanıt taşımalı"))
                    .clone()
            };

            let routes = details("routes.txt");
            let trips = details("trips.txt");

            // Asimetri hâlâ VAR ve doğrudur: türev yalnız ham okuyan yolda doğar.
            assert!(routes.contains_key("suppressed_derivative_rules"),
                "ham okuyan yolda bastırılan türev kaydedilmeli: {routes:?}");
            assert!(!trips.contains_key("suppressed_derivative_rules"),
                "trim eden yolda bastırılacak türev yok: {trips:?}");

            // Kanıt ise artık İKİ rejimde de var — asimetriyi yanlış okumayı engelleyen şey bu.
            assert_eq!(routes.get("typed_whitespace_fields").map(String::as_str),
                Some("route_type"), "{routes:?}");
            assert_eq!(trips.get("typed_whitespace_fields").map(String::as_str),
                Some("wheelchair_accessible"), "{trips:?}");
        }
        other => panic!("ValidateResult::Ok beklendi, alınan: {other:?}"),
    }
}

/// #125'te kaldırılan K7 dalının DAYANAĞINI çapalar. O dal, akış yolundaki bir parse
/// reddinin gözlenen değerinde boşluk bulacağını varsayıyordu; oysa `stop_times`/`shapes`
/// emit'leri değeri `get_col` ile okur ve o `.trim()` uygular, yani böyle bir notice hiç
/// doğmaz. `get_col` bir gün trim'i bırakırsa bu test düşer ve kaldırma kararı yeniden
/// tartışılmalıdır — sessizce yanlışa dönmemeli.
#[test]
fn streaming_files_never_report_a_value_with_surrounding_whitespace() {
    let mut files = base_files();
    for slot in files.iter_mut() {
        if slot.0 == "stop_times.txt" {
            slot.1 = b"trip_id,arrival_time,departure_time,stop_id,stop_sequence,shape_dist_traveled\n\
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
                matches!(n.file.as_deref(), Some("stop_times.txt") | Some("shapes.txt"))
            }) {
                let Some(observed) = n.observed_value.as_deref() else { continue };
                assert_eq!(
                    observed, observed.trim(),
                    "{} akış yolunda çevresi boşluklu değer raporladı: {observed:?}",
                    n.rule_id
                );
            }
        }
        other => panic!("ValidateResult::Ok beklendi, alınan: {other:?}"),
    }
}

// ── issue #84: akış dosyası GÖVDESİNDE kapanmamış tırnak ───────────────────────
//
// K1 bu dört dosyanın gövdesini hiç açmaz; K1'in gövde tarayıcısına giden dal ÖLÜYDÜ
// (kaldırıldı) ve K2 okuyucuları kapanmamış tırnağı sessizce tolere ediyordu. Yani tek
// kaçak tırnak dosyanın kalanını yutabilir ve doğrulayıcı bunu HİÇ söylemezdi.
// ⚠️ Bulgu KRİTİK ama FATAL DEĞİL — gerekçe `common::arc013_unclosed_stream` doc'unda.

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
    assert!(match run(&stream_quote_feed(
        "trip_id,arrival_time,departure_time,stop_id,stop_sequence,stop_headsign\n\
         T1,08:00:00,08:00:00,S1,1,\"Kapanmamis\n\
         T1,08:10:00,08:10:00,S2,2,Normal\n")) {
        ValidateResult::Ok(vr) => has(&vr, "ARC_013"),
        other => panic!("Ok beklendi: {other:?}"),
    }, "akış gövdesindeki kapanmamış tırnak ARC_013 üretmeli");
}

#[test]
fn arc013_stream_silent_on_valid_quoted_comma() {
    assert!(match run(&stream_quote_feed(
        "trip_id,arrival_time,departure_time,stop_id,stop_sequence,stop_headsign\n\
         T1,08:00:00,08:00:00,S1,1,\"Merkez, Kuzey\"\n\
         T1,08:10:00,08:10:00,S2,2,Normal\n")) {
        ValidateResult::Ok(vr) => !has(&vr, "ARC_013") && !has(&vr, "ARC_033"),
        other => panic!("Ok beklendi: {other:?}"),
    }, "tırnaklanmış virgül GEÇERLİDİR");
}

#[test]
fn arc013_stream_silent_on_valid_doubled_quote() {
    assert!(match run(&stream_quote_feed(
        "trip_id,arrival_time,departure_time,stop_id,stop_sequence,stop_headsign\n\
         T1,08:00:00,08:00:00,S1,1,\"5\"\" Hat\"\n\
         T1,08:10:00,08:10:00,S2,2,Normal\n")) {
        ValidateResult::Ok(vr) => !has(&vr, "ARC_013") && !has(&vr, "ARC_033"),
        other => panic!("Ok beklendi: {other:?}"),
    }, "doğru kaçırılmış tırnak GEÇERLİDİR");
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
                !vr.notices.iter().any(|n| n.rule_id == "ARC_033" && n.file.as_deref() == Some("shapes.txt")),
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
            vr.notices.iter().any(|n| n.rule_id == "ARC_013" && n.file.as_deref() == Some("shapes.txt")),
            "kapanış tırnağı olmadan EOF shapes.txt ARC_013 üretmeli"
        ),
        other => panic!("ValidateResult::Ok beklendi, alınan: {other:?}"),
    }
}

#[test]
fn arc013_stream_bare_quote_is_arc033_not_arc013() {
    // İki olgu AYRI: kaçırma ihlali veriyi bozmaz (ARC_033), kapanmamış tırnak dosyanın
    // kalanını yutar (ARC_013). Aynı kanaldan raporlamak ikisini karıştırırdı.
    match run(&stream_quote_feed(
        "trip_id,arrival_time,departure_time,stop_id,stop_sequence,stop_headsign\n\
         T1,08:00:00,08:00:00,S1,1,5\" Hat\n\
         T1,08:10:00,08:10:00,S2,2,Normal\n")) {
        ValidateResult::Ok(vr) => {
            assert!(has(&vr, "ARC_033"), "çıplak tırnak ARC_033");
            assert!(!has(&vr, "ARC_013"), "çıplak tırnak tokenization HATASI değildir");
        }
        other => panic!("Ok beklendi: {other:?}"),
    }
}

// ── issue #92: SERT tip predikatı HAM değeri görmeli ───────────────────────────
//
// `#85` kimliği ham yaptı; bu, tip predikatlarındaki karşılığı. `" https://example.com "`
// ham hâlde kaçırılmamış boşluk taşır — spec "özel karakterler doğru kaçırılmalı" der —
// ama predikat kırpılmış vekili görüyordu ve sert kural SUSUYORDU. Geriye yalnız `DQ_016`
// kalite sinyali kalıyordu, yani bir Spec iddiası (`P5f72fb5a` KANITLI) normalize edilmiş
// bir değer üzerinden kuruluyordu.

fn agency_url_feed(agency: &'static str) -> Vec<(&'static str, &'static [u8])> {
    let mut files = base_files();
    for slot in files.iter_mut() {
        if slot.0 == "agency.txt" { slot.1 = agency.as_bytes(); }
    }
    files
}

#[test]
fn url92_padded_url_does_not_satisfy_the_hard_predicate() {
    assert!(match run(&agency_url_feed(
        "agency_id,agency_name,agency_url,agency_timezone\n1,T,\" https://example.com \",UTC\n")) {
        ValidateResult::Ok(vr) => has(&vr, "AGN_003"),
        other => panic!("Ok beklendi: {other:?}"),
    }, "kırpılınca geçerli olan URL sert predikatı KARŞILAMAMALI");
}

#[test]
fn url92_clean_url_stays_valid() {
    assert!(match run(&agency_url_feed(
        "agency_id,agency_name,agency_url,agency_timezone\n1,T,https://example.com,UTC\n")) {
        ValidateResult::Ok(vr) => !has(&vr, "AGN_003"),
        other => panic!("Ok beklendi: {other:?}"),
    }, "zaten kaçırılmış URL geçerli kalmalı");
}

#[test]
fn url92_whitespace_only_url_is_missing_not_invalid() {
    // 🔴 SINIR: `"   "` EKSİK alandır, "geçersiz URL" değil. Varlık teşhisi kırpar,
    // geçerlilik hamı görür — tek okumaya indirmek bu ayrımı yok ederdi.
    match run(&agency_url_feed(
        "agency_id,agency_name,agency_url,agency_timezone\n1,T,\"   \",UTC\n")) {
        ValidateResult::Ok(vr) => {
            let msg = vr.notices.iter().find(|n| n.rule_id == "AGN_003")
                .map(|n| n.message.clone()).unwrap_or_default();
            assert!(msg.contains("zorunludur"),
                "boşluk-yalnız değer EKSİK alan olarak bildirilmeli, gelen: {msg}");
        }
        other => panic!("Ok beklendi: {other:?}"),
    }
}

// ── issue #94: SHP_005 monotonluğu TAM karşılaştırma ────────────────────────
// Eski predikat `d < prev - 1e-6` idi: 1e-6'dan küçük GERÇEK azalma sessizce kabul
// ediliyordu. Bu test CSV'den geçer — yalnız K5 birim testi, ondalık→f64 okumasının
// üreticinin sırasını koruduğunu göstermez.

fn shp005_feed(shapes: &'static str) -> Vec<(&'static str, &'static [u8])> {
    let mut files = base_files();
    files.push(("shapes.txt", shapes.as_bytes()));
    files
}

#[test]
fn shp005_detects_decrease_smaller_than_old_epsilon() {
    // issue #94'ün karşı-örneği birebir: azalma 5e-7.
    match run(&shp005_feed("shape_id,shape_pt_lat,shape_pt_lon,shape_pt_sequence,shape_dist_traveled\n\
        S1,41.0,29.0,1,1.0000005\nS1,41.1,29.1,2,1.0000000\n")) {
        ValidateResult::Ok(vr) => assert!(has(&vr, "SHP_005"),
            "1e-6'dan küçük azalma SHP_005 üretmeli (#94)"),
        other => panic!("Ok beklendi: {other:?}"),
    }
}

#[test]
fn shp005_silent_on_tiny_increase_from_csv() {
    // Aynı ondalık basamak sayısı, ters yön → susmalı (tolerans kaldırma FP getirmemeli).
    match run(&shp005_feed("shape_id,shape_pt_lat,shape_pt_lon,shape_pt_sequence,shape_dist_traveled\n\
        S1,41.0,29.0,1,1.0000000\nS1,41.1,29.1,2,1.0000005\n")) {
        ValidateResult::Ok(vr) => assert!(!has(&vr, "SHP_005"),
            "artan shape_dist_traveled SHP_005 üretmemeli"),
        other => panic!("Ok beklendi: {other:?}"),
    }
}

#[test]
fn shp005_silent_on_equal_values_from_csv() {
    // Eşitlik AYRI vaka (#94): kural yalnız `önceki > güncel` içindir.
    match run(&shp005_feed("shape_id,shape_pt_lat,shape_pt_lon,shape_pt_sequence,shape_dist_traveled\n\
        S1,41.0,29.0,1,1.0000000\nS1,41.1,29.1,2,1.0000000\n")) {
        ValidateResult::Ok(vr) => assert!(!has(&vr, "SHP_005"),
            "eşit shape_dist_traveled SHP_005 üretmemeli"),
        other => panic!("Ok beklendi: {other:?}"),
    }
}
