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

    let k1 = parse(&make_zip(&files)).expect("gecerli kok dosyalar mevcut oldugunda Ok donmeli");

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
    let k1 = parse(&make_zip(&files)).expect("gecerli kok dosyalar Ok donmeli");

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
    let k1 = parse(&make_zip(&base_files())).expect("gecerli kok dosyalar Ok donmeli");
    let arc025 = k1.notices.iter().filter(|n| n.rule_id == "ARC_025").count();
    assert_eq!(arc025, 0, "tam basliklarda ARC_025 cikmamali, bulunan: {arc025}");
}
