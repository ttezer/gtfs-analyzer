use std::io::Write as _;
use zip::write::SimpleFileOptions;

use gtfs_pipeline::{validate_bytes, CalendarOverrideRule, ValidateResult, ValidatorConfig};

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

static AGENCY: &[u8] =
    b"agency_id,agency_name,agency_url,agency_timezone\n1,Test,http://test.example,UTC\n";
static STOPS: &[u8] =
    b"stop_id,stop_name,stop_lat,stop_lon\nS1,Stop1,41.0,29.0\nS2,Stop2,41.1,29.1\n";
static ROUTES: &[u8] =
    b"route_id,agency_id,route_short_name,route_type\nR1,1,101,3\n";

fn run_with_config(files: &[(&str, &[u8])], config: ValidatorConfig) -> ValidateResult {
    validate_bytes(&make_zip(files), &config, TODAY)
}

fn run_default(files: &[(&str, &[u8])]) -> ValidateResult {
    validate_bytes(&make_zip(files), &ValidatorConfig::default(), TODAY)
}

fn has_rule(result: &ValidateResult, rule_id: &str) -> bool {
    match result {
        ValidateResult::Ok(vr) => vr.notices.iter().any(|n| n.rule_id == rule_id),
        _ => false,
    }
}

// Tarih notları:
//   20260101 = Perşembe (Ocak 1, 2026)
//   20260105 = Pazartesi
//   WKD = Pazartesi-Cuma aktif

static CALENDAR_WKD: &[u8] =
    b"service_id,monday,tuesday,wednesday,thursday,friday,saturday,sunday,start_date,end_date\n\
      WKD,1,1,1,1,1,0,0,20250101,20271231\n";

fn two_service_trips() -> &'static [u8] {
    b"route_id,service_id,trip_id\nR1,WKD,T_WKD\nR1,HOL,T_HOL\n"
}

fn two_service_stop_times() -> &'static [u8] {
    b"trip_id,arrival_time,departure_time,stop_id,stop_sequence\n\
      T_WKD,08:00:00,08:00:00,S1,1\nT_WKD,08:10:00,08:10:00,S2,2\n\
      T_HOL,09:00:00,09:00:00,S1,1\nT_HOL,09:10:00,09:10:00,S2,2\n"
}

fn override_config_r1(start: u32, end: u32) -> ValidatorConfig {
    let mut config = ValidatorConfig::default();
    config.calendar_override_rules = vec![CalendarOverrideRule {
        route_id: "R1".to_string(),
        base_service_ids: vec!["WKD".to_string()],
        override_service_ids: vec!["HOL".to_string()],
        start_date: start,
        end_date: end,
    }];
    config
}

// ── Test OPR-1: Normal gün, tek servis → hiç OPR notice üretilmemeli ─────────

#[test]
fn opr_single_service_no_conflict_no_notice() {
    let trips: &[u8] = b"route_id,service_id,trip_id\nR1,SVC1,T1\n";
    let stop_times: &[u8] =
        b"trip_id,arrival_time,departure_time,stop_id,stop_sequence\n\
          T1,08:00:00,08:00:00,S1,1\nT1,08:10:00,08:10:00,S2,2\n";
    let calendar: &[u8] =
        b"service_id,monday,tuesday,wednesday,thursday,friday,saturday,sunday,start_date,end_date\n\
          SVC1,1,1,1,1,1,0,0,20250101,20271231\n";

    let files = vec![
        ("agency.txt",     AGENCY),
        ("stops.txt",      STOPS),
        ("routes.txt",     ROUTES),
        ("trips.txt",      trips),
        ("stop_times.txt", stop_times),
        ("calendar.txt",   calendar),
    ];
    let result = run_default(&files);
    assert!(!has_rule(&result, "OPR_019"), "Tek servis → OPR_019 olmamalı");
    assert!(!has_rule(&result, "OPR_020"), "Tek servis → OPR_020 olmamalı");
    assert!(!has_rule(&result, "OPR_021"), "Config yok → OPR_021 olmamalı");
    assert!(!has_rule(&result, "OPR_022"), "Config yok → OPR_022 olmamalı");
    assert!(!has_rule(&result, "OPR_023"), "Config yok → OPR_023 olmamalı");
}

// ── Test OPR-2: Doğru override — base kaldırılmış, override eklenmiş → CRITICAL yok ──
// 20260101 (Perşembe): WKD exception_type=2, HOL exception_type=1 → doğru

#[test]
fn opr_021_absent_when_correct_override() {
    let cal_dates: &[u8] =
        b"service_id,date,exception_type\nWKD,20260101,2\nHOL,20260101,1\n";

    let files = vec![
        ("agency.txt",          AGENCY),
        ("stops.txt",           STOPS),
        ("routes.txt",          ROUTES),
        ("trips.txt",           two_service_trips()),
        ("stop_times.txt",      two_service_stop_times()),
        ("calendar.txt",        CALENDAR_WKD),
        ("calendar_dates.txt",  cal_dates),
    ];

    let result = run_with_config(&files, override_config_r1(20260101, 20260101));
    assert!(!has_rule(&result, "OPR_021"), "Doğru override → OPR_021 olmamalı");
    assert!(!has_rule(&result, "OPR_022"), "Doğru override → OPR_022 olmamalı");
    assert!(!has_rule(&result, "OPR_023"), "Doğru override → OPR_023 olmamalı");
}

// ── Test OPR-3: Base kaldırılmamış + override eklenmiş → OPR_021 ──────────────
// 20260101 (Perşembe): WKD aktif (kaldırılmadı), HOL de exception_type=1 ile eklendi

#[test]
fn opr_021_fires_when_base_not_removed() {
    let cal_dates: &[u8] = b"service_id,date,exception_type\nHOL,20260101,1\n";

    let files = vec![
        ("agency.txt",          AGENCY),
        ("stops.txt",           STOPS),
        ("routes.txt",          ROUTES),
        ("trips.txt",           two_service_trips()),
        ("stop_times.txt",      two_service_stop_times()),
        ("calendar.txt",        CALENDAR_WKD),
        ("calendar_dates.txt",  cal_dates),
    ];

    let result = run_with_config(&files, override_config_r1(20260101, 20260101));
    assert!(has_rule(&result, "OPR_021"),
        "Base kaldırılmamış + override eklenmiş → OPR_021 olmalı");
}

// ── Test OPR-4: Override bekleniyor ama aktif değil → OPR_022 ─────────────────
// 20260101 (Perşembe): WKD aktif, HOL için hiç tanım yok

#[test]
fn opr_022_fires_when_override_not_added() {
    // HOL için hiç calendar tanımı yok
    let files = vec![
        ("agency.txt",     AGENCY),
        ("stops.txt",      STOPS),
        ("routes.txt",     ROUTES),
        ("trips.txt",      two_service_trips()),
        ("stop_times.txt", two_service_stop_times()),
        ("calendar.txt",   CALENDAR_WKD),
    ];

    let result = run_with_config(&files, override_config_r1(20260101, 20260101));
    assert!(has_rule(&result, "OPR_022"),
        "Override bekleniyor ama aktif değil → OPR_022 olmalı");
    assert!(!has_rule(&result, "OPR_021"), "OPR_021 olmamalı");
}

// ── Test OPR-5: Override penceresi — ne base ne override aktif → OPR_023 ───────
// 20260101 (Perşembe): WKD exception_type=2 ile kaldırıldı, HOL hiç yok

#[test]
fn opr_023_fires_when_no_service_in_window() {
    let cal_dates: &[u8] = b"service_id,date,exception_type\nWKD,20260101,2\n";

    let files = vec![
        ("agency.txt",          AGENCY),
        ("stops.txt",           STOPS),
        ("routes.txt",          ROUTES),
        ("trips.txt",           two_service_trips()),
        ("stop_times.txt",      two_service_stop_times()),
        ("calendar.txt",        CALENDAR_WKD),
        ("calendar_dates.txt",  cal_dates),
    ];

    let result = run_with_config(&files, override_config_r1(20260101, 20260101));
    assert!(has_rule(&result, "OPR_023"), "Servissiz gün → OPR_023 olmalı");
    assert!(!has_rule(&result, "OPR_021"), "OPR_021 olmamalı");
    assert!(!has_rule(&result, "OPR_022"), "OPR_022 olmamalı");
}

// ── Test OPR-6: Çoklu servis, config yok → en fazla HIGH, CRITICAL yok ─────────
// SVC_A: Paz-Cum. SVC_B: 20260105 (Pazartesi) exception_type=1 ile eklenmiş.
// Config olmadığı için OPR_021/022/023 üretilmemeli; OPR_020 (HIGH) beklenir.
//
// T_A ve T_B AYNI operasyonel sefer (aynı kalkış saati, aynı durak dizisi) olmak
// ZORUNDA: OPR_020 artık "aynı gün ≥2 servis aktif" demiyor, "AYNI sefer iki servisten
// birden aktif" diyor. Farklı kalkış saatleri iki ayrı tren demektir ve meşrudur —
// bu testin pozitif kontrolü gerçek bir override çakışması olmalı.

#[test]
fn opr_no_critical_without_config() {
    let trips: &[u8] = b"route_id,service_id,trip_id\nR1,SVC_A,T_A\nR1,SVC_B,T_B\n";
    let stop_times: &[u8] =
        b"trip_id,arrival_time,departure_time,stop_id,stop_sequence\n\
          T_A,08:00:00,08:00:00,S1,1\nT_A,08:10:00,08:10:00,S2,2\n\
          T_B,08:00:00,08:00:00,S1,1\nT_B,08:10:00,08:10:00,S2,2\n";
    let calendar: &[u8] =
        b"service_id,monday,tuesday,wednesday,thursday,friday,saturday,sunday,start_date,end_date\n\
          SVC_A,1,1,1,1,1,0,0,20250101,20271231\n";
    // SVC_B yalnızca 20260105 Pazartesi → exception günü çakışması
    let cal_dates: &[u8] = b"service_id,date,exception_type\nSVC_B,20260105,1\n";

    let files = vec![
        ("agency.txt",          AGENCY),
        ("stops.txt",           STOPS),
        ("routes.txt",          ROUTES),
        ("trips.txt",           trips),
        ("stop_times.txt",      stop_times),
        ("calendar.txt",        calendar),
        ("calendar_dates.txt",  cal_dates),
    ];

    let result = run_default(&files);
    assert!(!has_rule(&result, "OPR_021"),
        "Config olmadan OPR_021 (CRITICAL) üretilmemeli");
    assert!(!has_rule(&result, "OPR_022"),
        "Config olmadan OPR_022 (CRITICAL) üretilmemeli");
    assert!(!has_rule(&result, "OPR_023"),
        "Config olmadan OPR_023 (CRITICAL) üretilmemeli");
    assert!(has_rule(&result, "OPR_020"),
        "Exception gününde çoklu aktif servis → OPR_020 (HIGH) olmalı");
}
