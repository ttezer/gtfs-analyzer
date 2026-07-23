//! Runtime emit-proof (#5 "full version" / C).
//!
//! emit_coverage.rs (rules crate) statik *reference proof* verir (kural prod kodda
//! geçiyor mu). Bu test ise *gerçek emit proof* verir: her fixture bir feed kurup
//! pipeline'ı çalıştırır ve beklenen rule_id'nin `vr.notices` içinde gerçekten
//! üretildiğini doğrular. Manifest'te (henüz) fixture'ı olmayan canonical kurallar
//! `coverage_debt.txt` ledger'ında tutulur; zamanla fixture eklenip borç azaltılır
//! (kademeli "full version"). Ledger eşitlik testi hem gerilemeyi (fixture silinir →
//! borç artar) hem ilerlemeyi (fixture eklenir → borç azalır, ledger güncellenmeli)
//! yakalar.

use std::collections::BTreeSet;
use std::io::Write as _;
use zip::write::SimpleFileOptions;

use gtfs_pipeline::{validate_bytes, CalendarOverrideRule, ValidateResult, ValidatorConfig};
use gtfs_rules::RULES;

const TODAY: u32 = 20_260_515;

// ── Geçerli temel feed (integration.rs ile aynı; tek başına notice üretmez) ─────
const AGENCY: &str = "agency_id,agency_name,agency_url,agency_timezone\n1,Test,http://test.example,UTC\n";
const STOPS: &str = "stop_id,stop_name,stop_lat,stop_lon\nS1,Stop1,41.0,29.0\nS2,Stop2,41.1,29.1\n";
const ROUTES: &str = "route_id,agency_id,route_short_name,route_type\nR1,1,101,3\n";
const TRIPS: &str = "route_id,service_id,trip_id\nR1,SVC1,T1\n";
const STOP_TIMES: &str = "trip_id,arrival_time,departure_time,stop_id,stop_sequence\nT1,08:00:00,08:00:00,S1,1\nT1,08:10:00,08:10:00,S2,2\n";
const CALENDAR: &str = "service_id,monday,tuesday,wednesday,thursday,friday,saturday,sunday,start_date,end_date\nSVC1,1,1,1,1,1,0,0,20250101,20271231\n";

// Dosya içeriği byte olarak tutulur: string fixture'lar UTF-8'e çevrilir, ham (invalid-UTF-8)
// fixture'lar doğrudan byte geçer (ARC_002/003 için).
fn base() -> Vec<(String, Vec<u8>)> {
    [
        ("agency.txt", AGENCY), ("stops.txt", STOPS), ("routes.txt", ROUTES),
        ("trips.txt", TRIPS), ("stop_times.txt", STOP_TIMES), ("calendar.txt", CALENDAR),
    ].iter().map(|(n, c)| (n.to_string(), c.as_bytes().to_vec())).collect()
}

/// Temel feed'i alır; string override'ları, ham byte override'larını uygular (yeni dosya da ekler)
/// ve `removes`'taki dosyaları çıkarır (örn. "agency.txt eksik" senaryosu).
fn with_opts(overrides: &[(&str, &str)], removes: &[&str], raw: &[(&str, &[u8])]) -> Vec<(String, Vec<u8>)> {
    let mut files = base();
    let apply = |name: &str, bytes: Vec<u8>, files: &mut Vec<(String, Vec<u8>)>| {
        if let Some(slot) = files.iter_mut().find(|(n, _)| n == name) { slot.1 = bytes; }
        else { files.push((name.to_string(), bytes)); }
    };
    for (name, content) in overrides { apply(name, content.as_bytes().to_vec(), &mut files); }
    for (name, content) in raw { apply(name, content.to_vec(), &mut files); }
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

fn emitted_rules(files: &[(String, Vec<u8>)], config: &ValidatorConfig) -> BTreeSet<String> {
    match validate_bytes(&make_zip(files), config, TODAY) {
        ValidateResult::Ok(vr) => vr.notices.iter().map(|n| n.rule_id.clone()).collect(),
        ValidateResult::Fatal(e) => {
            // Fatal yol: rule_id'yi koddan türetmek yerine boş set; fatal kurallar allowlist'te.
            let _ = e;
            BTreeSet::new()
        }
    }
}

/// Bir fixture: beklenen rule_id + onu tetikleyen feed (base üzerine override'lar).
struct Fixture {
    rule: &'static str,
    overrides: Vec<(&'static str, &'static str)>,
    removes: Vec<&'static str>,
    raw: Vec<(&'static str, &'static [u8])>,
    config: Option<ValidatorConfig>,
}

fn fx(rule: &'static str, overrides: Vec<(&'static str, &'static str)>) -> Fixture {
    Fixture { rule, overrides, removes: Vec::new(), raw: Vec::new(), config: None }
}

/// Dosya çıkarmalı fixture ("X.txt eksik" senaryoları).
fn fx_rm(rule: &'static str, overrides: Vec<(&'static str, &'static str)>, removes: Vec<&'static str>) -> Fixture {
    Fixture { rule, overrides, removes, raw: Vec::new(), config: None }
}

/// Ham byte fixture — geçersiz UTF-8 senaryoları (ARC_002/003).
fn fx_raw(rule: &'static str, raw: Vec<(&'static str, &'static [u8])>) -> Fixture {
    Fixture { rule, overrides: Vec::new(), removes: Vec::new(), raw, config: None }
}

/// Config-override fixture — ör. calendar_override_rules (OPR_021/022/023).
fn fx_cfg(rule: &'static str, overrides: Vec<(&'static str, &'static str)>, config: ValidatorConfig) -> Fixture {
    Fixture { rule, overrides, removes: Vec::new(), raw: Vec::new(), config: Some(config) }
}

/// Notice olarak emit edilmeyen kurallar (fatal yol veya dinamik) — proof'tan muaf.
const PROOF_ALLOWLIST: &[&str] = &[
    "ARC_001", // fatal FatalCode::ZipUnreadable (Notice değil)
    "ARC_029", // fatal FatalCode::DecompressionLimit (Notice değil) — bu harness notice arar,
               // ARC_029 ise decompression guard tetiklenince Fatal döner. Gerçek uçtan uca kanıt
               // integration.rs::arc029_* testlerinde (DEFAULT limitlerle iki read_fatal yolu +
               // ratio_floor FP guard'ı). Borç DEĞİL: yapısal olarak notice fixture'ı yazılamaz.
    "ARC_004", // ARC_004 notice'ı emit edilir AMA hemen FatalCode::NoRequiredFiles döner →
               // ValidateResult::Fatal, notices kaybolur. Bu harness'ta yapısal olarak kanıtlanamaz.
    "AGN_001", // "agency.txt eksik" — FİİLEN HİÇ emit edilmez (ne Notice ne fatal rule_id).
               // Dosya eksikliğini ARC_004 (Fatal NoRequiredFiles) temsil eder; AGN_001 yalnızca
               // MD `missing_required_file` paritesi için registry'de tutulan, Notice üretmeyen bir
               // kayıttır. Karar: bırak + allowlist (issue #27). Üretim kodunda literal yok.
];

// ── KALICI DEBT (coverage_debt.txt'te kalır) ──
// (AGN_001 → PROOF_ALLOWLIST'te: fatal yol, ARC_004 temsil eder; issue #27)
// #28 ÇÖZÜLEN: ARC_002/003 (fx_raw ham byte), OPR_021/022/023 + ARC_028/STP_040/STP_041 (fx_cfg
// config) artık fixtures()'ta runtime-kanıtlı. Kalan borç yalnız pratik-olmayan büyük feed'ler +
// ARC_027 (yapısal):
//   ARC_022  — > 1.000.000 satır gerektirir (inline yazılamaz).
//   OPR_024  — tek (route, service)'te > 500 sefer gerektirir (inline yazılamaz).
//   SHP_026  — > 5000 shape noktası gerektirir.
//   STM_043  — sefer başına > 200 durak gerektirir.
//   STM_044  — > 2.000.000 stop_times satırı gerektirir.
//   VAT_006  — feed'de >= 50 sefer gerektirir (inline pratik değil).
//   ARC_027  — ZIP entry Unix izin metadata'sı gerektirir (fixture üretmiyor).
// ARC_001/ARC_004/ARC_029 PROOF_ALLOWLIST'te (Fatal yol). Diğerleri coverage_debt.txt'te bilinçli
// bırakıldı. NOT (2026-07-16): ARC_029 borç ledger'ındaydı ama borç DEĞİLDİ — ARC_001 gibi Fatal
// yol; allowlist'e taşındı, uçtan uca kanıtı integration.rs::arc029_*'ta.

// #28 Grup 1 yardımcıları ─────────────────────────────────────────────────────
// OPSİYONEL dosyada (feed_info.txt) geçersiz UTF-8 (0xFF/0xFE) → ARC_002 + ARC_003 (fatal değil).
const BAD_UTF8_FEED_INFO: &[u8] = b"feed_publisher_name,feed_publisher_url,feed_lang\n\xff\xfe,https://x.example,en\n";
// calendar_dates: SVC_B 0101+0102, SVC_O yalnız 0101 aktif. Kural penceresi 0101-0103'te:
// 0101 base+override → OPR_021, 0102 yalnız base → OPR_022, 0103 hiçbiri → OPR_023.
const OVERRIDE_CD: &str = "service_id,date,exception_type\nSVC_B,20260101,1\nSVC_B,20260102,1\nSVC_O,20260101,1\n";
fn override_config() -> ValidatorConfig {
    let mut c = ValidatorConfig::default();
    c.calendar_override_rules = vec![CalendarOverrideRule {
        route_id: "R1".into(),
        base_service_ids: vec!["SVC_B".into()],
        override_service_ids: vec!["SVC_O".into()],
        start_date: 20260101,
        end_date: 20260103,
    }];
    c
}
// ARC_028: source_url verilir ve .zip ile bitmezse tetiklenir (config-gated).
fn source_url_config() -> ValidatorConfig {
    let mut c = ValidatorConfig::default();
    c.source_url = Some("https://example.org/gtfs".into());
    c
}
// STP_040/041: opt-in stop-adı profili. P1 "Main Stop" → generic 'stop' sözcüğü (STP_040) VE
// parent ST1 "Central Hub" adını içermiyor (STP_041). S1/S2 stop_times referansları için korunur.
fn stop_name_config() -> ValidatorConfig {
    let mut c = ValidatorConfig::default();
    c.stop_name_best_practices = true;
    c
}
const STOP_NAME_STOPS: &str = "stop_id,stop_name,stop_lat,stop_lon,location_type,parent_station\nS1,Stop1,41.0,29.0,0,\nS2,Stop2,41.1,29.1,0,\nST1,Central Hub,41.0,29.0,1,\nP1,Main Stop,41.0,29.0,0,ST1\n";

/// rule_id → tetikleyici fixture. Kademeli doldurulur; her satır gerçek runtime proof.
fn fixtures() -> Vec<Fixture> {
    vec![
        // ── #28 Grup 1: ham-byte (ARC_002/003) + config-override (OPR_021/022/023) ──
        fx_raw("ARC_002", vec![("feed_info.txt", BAD_UTF8_FEED_INFO)]),
        fx_raw("ARC_003", vec![("feed_info.txt", BAD_UTF8_FEED_INFO)]),
        fx_cfg("OPR_021", vec![("calendar_dates.txt", OVERRIDE_CD)], override_config()),
        fx_cfg("OPR_022", vec![("calendar_dates.txt", OVERRIDE_CD)], override_config()),
        fx_cfg("OPR_023", vec![("calendar_dates.txt", OVERRIDE_CD)], override_config()),
        // Bonus: config-gated kurallar (fx_cfg ile).
        fx_cfg("ARC_028", vec![], source_url_config()),
        fx_cfg("STP_040", vec![("stops.txt", STOP_NAME_STOPS)], stop_name_config()),
        fx_cfg("STP_041", vec![("stops.txt", STOP_NAME_STOPS)], stop_name_config()),
        // ── Tohum parti: tetikleme koşulu net K2/K6 kuralları ──────────────────
        // STP_003: stop_lat aralık dışı.
        fx("STP_003", vec![("stops.txt", "stop_id,stop_name,stop_lat,stop_lon\nS1,Stop1,999.0,29.0\nS2,Stop2,41.1,29.1\n")]),
        // STP_016: iki durak birebir aynı koordinat (parent/child değil).
        fx("STP_016", vec![("stops.txt", "stop_id,stop_name,stop_lat,stop_lon\nS1,Stop1,41.0,29.0\nS2,Stop2,41.0,29.0\n")]),
        // STP_022: stop_code yok (base zaten stop_code'suz → feed-özeti/per-stop). base'te 2 durak.
        fx("STP_022", vec![]),
        // RTS_017: hat shape'siz (base'te shape yok).
        fx("RTS_017", vec![]),

        // ── AGN grubu (agency.txt alan doğrulamaları) ──────────────────────────
        fx("AGN_002", vec![("agency.txt", "agency_id,agency_name,agency_url,agency_timezone\n1,,https://x.example,UTC\n")]),
        fx("AGN_003", vec![("agency.txt", "agency_id,agency_name,agency_url,agency_timezone\n1,Test,notaurl,UTC\n")]),
        fx("AGN_004", vec![("agency.txt", "agency_id,agency_name,agency_url,agency_timezone\n1,Test,https://x.example,NotAZone\n")]),
        fx("AGN_006", vec![("agency.txt", "agency_id,agency_name,agency_url,agency_timezone,agency_lang\n1,Test,https://x.example,UTC,!!bad\n")]),
        fx("AGN_007", vec![("agency.txt", "agency_id,agency_name,agency_url,agency_timezone,agency_phone\n1,Test,https://x.example,UTC,12\n")]),
        fx("AGN_008", vec![("agency.txt", "agency_id,agency_name,agency_url,agency_timezone,agency_fare_url\n1,Test,https://x.example,UTC,notaurl\n")]),
        fx("AGN_009", vec![("agency.txt", "agency_id,agency_name,agency_url,agency_timezone,agency_email\n1,Test,https://x.example,UTC,notanemail\n")]),
        fx("AGN_012", vec![("agency.txt", "agency_id,agency_name,agency_url,agency_timezone,cemv_support\n1,Test,https://x.example,UTC,5\n")]),
        fx("AGN_015", vec![("agency.txt", "agency_id,agency_name,agency_url,agency_timezone\n1,Test,http://x.example,UTC\n")]),
        fx("AGN_016", vec![("agency.txt", "agency_id,agency_name,agency_url,agency_timezone,agency_phone\n1,Test,https://x.example,UTC,888-281-2681\n")]),
        // Çoklu kuruluş senaryoları:
        fx("AGN_014", vec![("agency.txt", "agency_id,agency_name,agency_url,agency_timezone\n1,A,https://a.example,UTC\n,B,https://b.example,UTC\n")]),
        fx("AGN_005", vec![("agency.txt", "agency_id,agency_name,agency_url,agency_timezone\n1,A,https://a.example,UTC\n2,B,https://b.example,Europe/Istanbul\n")]),
        fx("AGN_017", vec![("agency.txt", "agency_id,agency_name,agency_url,agency_timezone,agency_lang\n1,A,https://a.example,UTC,tr\n2,B,https://b.example,UTC,en\n")]),
        // AGN_011: birden fazla işletici varken route'ta agency_id boş.
        fx("AGN_011", vec![
            ("agency.txt", "agency_id,agency_name,agency_url,agency_timezone\n1,A,https://a.example,UTC\n2,B,https://b.example,UTC\n"),
            ("routes.txt", "route_id,agency_id,route_short_name,route_type\nR1,,101,3\n"),
        ]),
        // NOT (#5 bulgu, #27 karar): AGN_001 "agency.txt eksik" başlıklı ama agency.txt
        // eksikliğini ARC_004 (Fatal NoRequiredFiles) ele alıyor; AGN_001 hiçbir yolla emit
        // edilmiyor. Karar (#27): bırak + PROOF_ALLOWLIST'e taşındı (ARC_001/ARC_004 gibi
        // fatal-yol kuralı); MD missing_required_file paritesi için registry'de kalır.

        // ── ARC grubu (arşiv/dosya/başlık seviyesi) ────────────────────────────
        // ARC_008: takvim dosyası yok (calendar + calendar_dates ikisi de).
        fx_rm("ARC_008", vec![], vec!["calendar.txt"]),
        // ARC_009: dosyada veri satırı yok (sadece başlık).
        fx("ARC_009", vec![("stops.txt", "stop_id,stop_name,stop_lat,stop_lon\n")]),
        // ARC_012: satır sütun sayısı başlıkla uyuşmuyor.
        fx("ARC_012", vec![("stops.txt", "stop_id,stop_name,stop_lat,stop_lon\nS1,Stop1,41.0\nS2,Stop2,41.1,29.1\n")]),
        // ARC_015: yinelenen başlık sütunu.
        fx("ARC_015", vec![("stops.txt", "stop_id,stop_id,stop_lat,stop_lon\nS1,X,41.0,29.0\nS2,Y,41.1,29.1\n")]),
        // ARC_019: başlıkta boş sütun adı.
        fx("ARC_019", vec![("stops.txt", "stop_id,,stop_lat,stop_lon\nS1,X,41.0,29.0\nS2,Y,41.1,29.1\n")]),
        // ARC_025: zorunlu sütun başlıkta yok (stop_id).
        fx("ARC_025", vec![("stops.txt", "stop_name,stop_lat,stop_lon\nStop1,41.0,29.0\nStop2,41.1,29.1\n")]),
        // ARC_024: GTFS .txt ZIP içinde alt dizinde.
        fx("ARC_024", vec![("feed/stops.txt", "stop_id,stop_name,stop_lat,stop_lon\nS9,Extra,41.0,29.0\n")]),
        fx("ARC_026", vec![("feed_info.txt", "feed_publisher_name,feed_publisher_url,feed_lang\rPub,https://x.example,en\r")]),

        // ── ATR grubu (attributions.txt; base'te yok, eklenir) ─────────────────
        fx("ATR_001", vec![("attributions.txt", "organization_name,is_producer\nOrg,1\n")]),
        fx("ATR_002", vec![("attributions.txt", "attribution_id,organization_name,is_producer\nA1,,1\n")]),
        fx("ATR_003", vec![("attributions.txt", "attribution_id,organization_name,is_producer,is_operator,is_authority\nA1,Org,0,0,0\n")]),
        fx("ATR_004", vec![("attributions.txt", "attribution_id,organization_name,is_producer\nA1,Org,5\n")]),
        fx("ATR_005", vec![("attributions.txt", "attribution_id,organization_name,is_operator\nA1,Org,5\n")]),
        fx("ATR_006", vec![("attributions.txt", "attribution_id,organization_name,is_authority\nA1,Org,5\n")]),
        fx("ATR_007", vec![("attributions.txt", "attribution_id,organization_name,is_producer,attribution_url\nA1,Org,1,notaurl\n")]),
        fx("ATR_008", vec![("attributions.txt", "attribution_id,organization_name,is_producer,attribution_email\nA1,Org,1,notanemail\n")]),

        // ── BKR grubu (booking_rules.txt; prior_notice tutarlılığı) ────────────
        fx("BKR_001", vec![("booking_rules.txt", "booking_rule_id,booking_type,prior_notice_duration_min,prior_notice_last_day\nBR1,1,30,2\n")]),
        fx("BKR_002", vec![("booking_rules.txt", "booking_rule_id,booking_type,prior_notice_last_time,prior_notice_start_day,prior_notice_start_time\nBR1,2,12:00:00,7,09:00:00\n")]),
        fx("BKR_003", vec![("booking_rules.txt", "booking_rule_id,prior_notice_start_time\nBR1,09:00:00\n")]),
        fx("BKR_004", vec![("booking_rules.txt", "booking_rule_id,booking_type,prior_notice_duration_min\nBR1,0,15\n")]),
        fx("BKR_005", vec![("booking_rules.txt", "booking_rule_id,booking_type,prior_notice_last_day,prior_notice_last_time,prior_notice_duration_max\nBR1,2,3,12:00:00,10\n")]),
        fx("BKR_006", vec![("booking_rules.txt", "booking_rule_id,booking_type,prior_notice_duration_min\nBR1,1,0\n")]),
        fx("BKR_007", vec![("booking_rules.txt", "booking_rule_id,booking_type\nBR1,1\n")]),
        fx("BKR_008", vec![("booking_rules.txt", "booking_rule_id,booking_type,prior_notice_last_time\nBR1,2,12:00:00\n")]),
        fx("BKR_009", vec![("booking_rules.txt", "booking_rule_id,booking_type,prior_notice_last_day\nBR1,2,3\n")]),
        fx("BKR_010", vec![("booking_rules.txt", "booking_rule_id,booking_type,prior_notice_last_day,prior_notice_last_time,prior_notice_start_day\nBR1,2,3,12:00:00,7\n")]),
        fx("BKR_011", vec![("booking_rules.txt", "booking_rule_id,booking_type,prior_notice_last_day,prior_notice_last_time,prior_notice_start_day,prior_notice_start_time\nBR1,2,5,12:00:00,3,09:00:00\n")]),

        // ── ARS: areas.txt area_id tekrarı ─────────────────────────────────────
        fx("ARS_001", vec![("areas.txt", "area_id,area_name\nA1,Area1\nA1,Area2\n")]),

        // ── CAL grubu (calendar.txt) ───────────────────────────────────────────
        fx("CAL_001", vec![("calendar.txt", "service_id,monday,tuesday,wednesday,thursday,friday,saturday,sunday,start_date,end_date\nSVC1,1,1,1,1,1,0,0,20250101,20271231\nSVC1,1,1,1,1,1,0,0,20250101,20271231\n")]),
        fx("CAL_002", vec![("calendar.txt", "service_id,monday,tuesday,wednesday,thursday,friday,saturday,sunday,start_date,end_date\nSVC1,5,1,1,1,1,0,0,20250101,20271231\n")]),
        fx("CAL_003", vec![("calendar.txt", "service_id,monday,tuesday,wednesday,thursday,friday,saturday,sunday,start_date,end_date\nSVC1,1,1,1,1,1,0,0,,20271231\n")]),
        fx("CAL_004", vec![("calendar.txt", "service_id,monday,tuesday,wednesday,thursday,friday,saturday,sunday,start_date,end_date\nSVC1,1,1,1,1,1,0,0,20250101,\n")]),
        fx("CAL_005", vec![("calendar.txt", "service_id,monday,tuesday,wednesday,thursday,friday,saturday,sunday,start_date,end_date\nSVC1,1,1,1,1,1,0,0,20271231,20250101\n")]),
        fx("CAL_022", vec![("calendar.txt", "service_id,monday,tuesday,wednesday,thursday,friday,saturday,sunday,start_date,end_date\n,1,1,1,1,1,0,0,20250101,20271231\n")]),

        // ── CLD grubu (calendar_dates.txt; base'te yok) ────────────────────────
        fx("CLD_001", vec![("calendar_dates.txt", "service_id,date,exception_type\n,20260601,1\n")]),
        fx("CLD_002", vec![("calendar_dates.txt", "service_id,date,exception_type\nSVC1,notadate,1\n")]),
        fx("CLD_003", vec![("calendar_dates.txt", "service_id,date,exception_type\nSVC1,20260601,5\n")]),
        fx("CLD_005", vec![("calendar_dates.txt", "service_id,date,exception_type\nSVC1,30000601,1\n")]),

        // ── FAR grubu (fare_attributes.txt; fares v1) ──────────────────────────
        fx("FAR_002", vec![("fare_attributes.txt", "fare_id,price,currency_type,payment_method\nF1,-1,USD,0\n")]),
        fx("FAR_003", vec![("fare_attributes.txt", "fare_id,price,currency_type,payment_method\nF1,2.5,usd,0\n")]),
        fx("FAR_004", vec![("fare_attributes.txt", "fare_id,price,currency_type,payment_method\nF1,2.5,USD,5\n")]),
        fx("FAR_005", vec![("fare_attributes.txt", "fare_id,price,currency_type,payment_method,transfers\nF1,2.5,USD,0,9\n")]),
        fx("FAR_006", vec![("fare_attributes.txt", "fare_id,price,currency_type,payment_method,transfer_duration\nF1,2.5,USD,0,0\n")]),
        fx("FAR_011", vec![("fare_attributes.txt", "fare_id,price,currency_type,payment_method\nF1,2.5,USD,\n")]),
        fx("FAR_012", vec![("fare_attributes.txt", "fare_id,price,currency_type,payment_method\n,2.5,USD,0\n")]),

        // ── FIN grubu (feed_info.txt; base'te yok) ─────────────────────────────
        fx("FIN_001", vec![("feed_info.txt", "feed_publisher_name,feed_publisher_url,feed_lang\n,https://x.example,en\n")]),
        fx("FIN_002", vec![("feed_info.txt", "feed_publisher_name,feed_publisher_url,feed_lang\nPub,notaurl,en\n")]),
        fx("FIN_003", vec![("feed_info.txt", "feed_publisher_name,feed_publisher_url,feed_lang\nPub,https://x.example,!!bad\n")]),
        fx("FIN_004", vec![("feed_info.txt", "feed_publisher_name,feed_publisher_url,feed_lang,default_lang\nPub,https://x.example,en,!!bad\n")]),
        fx("FIN_005", vec![("feed_info.txt", "feed_publisher_name,feed_publisher_url,feed_lang,feed_start_date\nPub,https://x.example,en,notadate\n")]),
        fx("FIN_006", vec![("feed_info.txt", "feed_publisher_name,feed_publisher_url,feed_lang,feed_end_date\nPub,https://x.example,en,notadate\n")]),

        // ── FLG: rule_priority negatif (fare_leg_rules.txt) ────────────────────
        fx("FLG_007", vec![("fare_leg_rules.txt", "leg_group_id,rule_priority\nLG1,-5\n")]),

        // ── FRQ grubu (frequencies.txt; trip_id=T1 base'te var) ────────────────
        fx("FRQ_001", vec![("frequencies.txt", "trip_id,start_time,end_time,headway_secs\n,08:00:00,10:00:00,600\n")]),
        fx("FRQ_002", vec![("frequencies.txt", "trip_id,start_time,end_time,headway_secs\nT1,,10:00:00,600\n")]),
        fx("FRQ_003", vec![("frequencies.txt", "trip_id,start_time,end_time,headway_secs\nT1,08:00:00,,600\n")]),
        fx("FRQ_004", vec![("frequencies.txt", "trip_id,start_time,end_time,headway_secs\nT1,08:00:00,10:00:00,\n")]),
        fx("FRQ_005", vec![("frequencies.txt", "trip_id,start_time,end_time,headway_secs\nT1,10:00:00,08:00:00,600\n")]),
        fx("FRQ_007", vec![("frequencies.txt", "trip_id,start_time,end_time,headway_secs,exact_times\nT1,08:00:00,10:00:00,600,5\n")]),
        fx("FRQ_008", vec![("frequencies.txt", "trip_id,start_time,end_time,headway_secs\nT1,08:00:00,10:00:00,0\n")]),
        fx("FRQ_009", vec![("frequencies.txt", "trip_id,start_time,end_time,headway_secs\nT1,08:00:00,10:00:00,30\n")]),

        // ── FTR grubu (fare_transfer_rules.txt; K2 alan kontrolleri) ───────────
        fx("FTR_001", vec![("fare_transfer_rules.txt", "from_leg_group_id,fare_transfer_type\nLG1,5\n")]),
        fx("FTR_005", vec![("fare_transfer_rules.txt", "from_leg_group_id,fare_transfer_type,duration_limit_type,duration_limit\nLG1,0,9,100\n")]),
        fx("FTR_006", vec![("fare_transfer_rules.txt", "from_leg_group_id,fare_transfer_type,duration_limit\nLG1,0,0\n")]),
        fx("FTR_007", vec![("fare_transfer_rules.txt", "from_leg_group_id,fare_transfer_type,duration_limit_type\nLG1,0,1\n")]),
        fx("FTR_008", vec![("fare_transfer_rules.txt", "from_leg_group_id,fare_transfer_type,transfer_count\nLG1,0,-5\n")]),

        // ── FMD / FPD (fare_media.txt / fare_products.txt) ─────────────────────
        fx("FMD_002", vec![("fare_media.txt", "fare_media_id,fare_media_type\nM1,9\n")]),
        fx("FMD_003", vec![("fare_media.txt", "fare_media_id,fare_media_type\nM1,2\n")]),
        fx("FPD_002", vec![("fare_products.txt", "fare_product_id,amount,currency\nP1,-1,USD\n")]),

        // ── LVL (levels.txt) ───────────────────────────────────────────────────
        fx("LVL_007", vec![("levels.txt", "level_id,level_index\nL1,\n")]),
        fx("LVL_008", vec![("levels.txt", "level_id,level_index\n,0\n")]),

        // ── RCT (rider_categories.txt) ─────────────────────────────────────────
        fx("RCT_002", vec![("rider_categories.txt", "rider_category_id,rider_category_name\nRC1,\n")]),
        fx("RCT_003", vec![("rider_categories.txt", "rider_category_id,rider_category_name,is_default_fare_category\nRC1,Adult,5\n")]),
        fx("RCT_004", vec![("rider_categories.txt", "rider_category_id,rider_category_name,min_age\nRC1,Adult,abc\n")]),
        fx("RCT_005", vec![("rider_categories.txt", "rider_category_id,rider_category_name,min_age,max_age\nRC1,Adult,65,18\n")]),

        // ── TFR (timeframes.txt) ───────────────────────────────────────────────
        fx("TFR_001", vec![("timeframes.txt", "timeframe_group_id,start_time,end_time,service_id\n,08:00:00,10:00:00,SVC1\n")]),
        fx("TFR_003", vec![("timeframes.txt", "timeframe_group_id,start_time,end_time,service_id\nTG1,notatime,10:00:00,SVC1\n")]),
        fx("TFR_004", vec![("timeframes.txt", "timeframe_group_id,start_time,end_time,service_id\nTG1,10:00:00,08:00:00,SVC1\n")]),
        fx("TFR_005", vec![("timeframes.txt", "timeframe_group_id,start_time,end_time,service_id\nTG1,08:00:00,12:00:00,SVC1\nTG1,10:00:00,14:00:00,SVC1\n")]),

        // ── STP grubu (stops.txt; K2 alan kontrolleri) ─────────────────────────
        fx("STP_002", vec![("stops.txt", "stop_id,stop_name,stop_lat,stop_lon\n,Stop1,41.0,29.0\nS2,Stop2,41.1,29.1\n")]),
        fx("STP_004", vec![("stops.txt", "stop_id,stop_name,stop_lat,stop_lon\nS1,Stop1,abc,29.0\nS2,Stop2,41.1,29.1\n")]),
        fx("STP_005", vec![("stops.txt", "stop_id,stop_name,stop_lat,stop_lon\nS1,Stop1,41.0,200\nS2,Stop2,41.1,29.1\n")]),
        fx("STP_006", vec![("stops.txt", "stop_id,stop_name,stop_lat,stop_lon\nS1,Stop1,,29.0\nS2,Stop2,41.1,29.1\n")]),
        fx("STP_007", vec![("stops.txt", "stop_id,stop_name,stop_lat,stop_lon\nS1,Stop1,41.0,\nS2,Stop2,41.1,29.1\n")]),
        fx("STP_008", vec![("stops.txt", "stop_id,stop_name,stop_lat,stop_lon,location_type\nS1,Stop1,41.0,29.0,9\nS2,Stop2,41.1,29.1,0\n")]),
        fx("STP_013", vec![("stops.txt", "stop_id,stop_name,stop_lat,stop_lon,wheelchair_boarding\nS1,Stop1,41.0,29.0,9\nS2,Stop2,41.1,29.1,0\n")]),
        fx("STP_014", vec![("stops.txt", "stop_id,stop_name,stop_lat,stop_lon,stop_timezone\nS1,Stop1,41.0,29.0,NotAZone\nS2,Stop2,41.1,29.1,UTC\n")]),
        fx("STP_018", vec![("stops.txt", "stop_id,stop_name,stop_lat,stop_lon\n")]),

        // ── RTS grubu (routes.txt; K2 alan kontrolleri) ────────────────────────
        fx("RTS_003", vec![("routes.txt", "route_id,agency_id,route_type\nR1,1,3\n")]),
        fx("RTS_004", vec![("routes.txt", "route_id,agency_id,route_short_name,route_type\nR1,1,101,9999\n")]),
        fx("RTS_005", vec![("routes.txt", "route_id,agency_id,route_short_name,route_type,route_url\nR1,1,101,3,notaurl\n")]),
        fx("RTS_006", vec![("routes.txt", "route_id,agency_id,route_short_name,route_type,route_color\nR1,1,101,3,XYZ\n")]),
        fx("RTS_013", vec![("routes.txt", "route_id,agency_id,route_short_name,route_type,continuous_pickup\nR1,1,101,3,9\n")]),
        fx("RTS_018", vec![("routes.txt", "route_id,agency_id,route_short_name,route_type,continuous_drop_off\nR1,1,101,3,9\n")]),
        // RTS_026: aynı kısa ad, FARKLI uzun ad (→ RTS_019 değil, Bilgi).
        fx("RTS_026", vec![("routes.txt", "route_id,agency_id,route_short_name,route_long_name,route_type\nR1,1,100,Line North,3\nR2,1,100,Line South,3\n")]),
        // RTS_027: aynı uzun ad, FARKLI kısa ad (→ RTS_019 değil, Bilgi).
        fx("RTS_027", vec![("routes.txt", "route_id,agency_id,route_short_name,route_long_name,route_type\nR1,1,10,City Line,3\nR2,1,20,City Line,3\n")]),

        // ── TRP grubu (trips.txt; K2 alan kontrolleri) ─────────────────────────
        fx("TRP_001", vec![("trips.txt", "route_id,service_id,trip_id\nR1,SVC1,\n")]),
        fx("TRP_005", vec![("trips.txt", "route_id,service_id,trip_id,direction_id\nR1,SVC1,T1,5\n")]),
        fx("TRP_006", vec![("trips.txt", "route_id,service_id,trip_id,wheelchair_accessible\nR1,SVC1,T1,9\n")]),
        fx("TRP_007", vec![("trips.txt", "route_id,service_id,trip_id,bikes_allowed\nR1,SVC1,T1,9\n")]),
        fx("TRP_031", vec![("trips.txt", "route_id,service_id,trip_id\n,SVC1,T1\n")]),
        fx("TRP_032", vec![("trips.txt", "route_id,service_id,trip_id,cars_allowed\nR1,SVC1,T1,9\n")]),

        // ── TRF grubu (transfers.txt) ──────────────────────────────────────────
        fx("TRF_001", vec![("transfers.txt", "from_stop_id,to_stop_id,transfer_type\n,S2,1\n")]),
        fx("TRF_002", vec![("transfers.txt", "from_stop_id,to_stop_id,transfer_type\nS1,,1\n")]),
        fx("TRF_020", vec![("transfers.txt", "from_stop_id,to_stop_id,transfer_type,min_transfer_time\nS1,S2,2,60\n")]),
        fx("TRF_004", vec![("transfers.txt", "from_stop_id,to_stop_id,transfer_type\nS1,S2,9\n")]),
        fx("TRF_005", vec![("transfers.txt", "from_stop_id,to_stop_id,transfer_type,min_transfer_time\nS1,S2,2,abc\n")]),

        // ── SHP grubu (shapes.txt; K2) ─────────────────────────────────────────
        fx("SHP_001", vec![("shapes.txt", "shape_id,shape_pt_lat,shape_pt_lon,shape_pt_sequence\n,41.0,29.0,1\n")]),
        fx("SHP_002", vec![("shapes.txt", "shape_id,shape_pt_lat,shape_pt_lon,shape_pt_sequence\nSH1,999,29.0,1\n")]),
        fx("SHP_003", vec![("shapes.txt", "shape_id,shape_pt_lat,shape_pt_lon,shape_pt_sequence\nSH1,41.0,999,1\n")]),
        fx("SHP_004", vec![("shapes.txt", "shape_id,shape_pt_lat,shape_pt_lon,shape_pt_sequence\nSH1,41.0,29.0,\n")]),
        fx("SHP_005", vec![("shapes.txt", "shape_id,shape_pt_lat,shape_pt_lon,shape_pt_sequence,shape_dist_traveled\nSH1,41.0,29.0,1,10\nSH1,41.1,29.1,2,5\n")]),
        fx("SHP_008", vec![("shapes.txt", "shape_id,shape_pt_lat,shape_pt_lon,shape_pt_sequence\nSH1,41.0,29.0,1\nSH1,41.1,29.1,1\n")]),

        // ── TRN grubu (translations.txt; K2) ───────────────────────────────────
        fx("TRN_001", vec![("translations.txt", "table_name,field_name,language,translation\nbadtable,stop_name,en,X\n")]),
        fx("TRN_002", vec![("translations.txt", "table_name,field_name,language,translation\nstops,,en,X\n")]),
        fx("TRN_003", vec![("translations.txt", "table_name,field_name,language,translation\nstops,stop_name,!!bad,X\n")]),
        fx("TRN_009", vec![("translations.txt", "table_name,field_name,language,translation,record_id,field_value\nstops,stop_name,en,X,S1,val\n")]),
        fx("TRN_011", vec![("translations.txt", "table_name,field_name,language,translation\nstops,stop_lat,en,X\n")]),

        // ── STM grubu (stop_times.txt; K2 satır kontrolleri) ───────────────────
        fx("STM_003", vec![("stop_times.txt", "trip_id,arrival_time,departure_time,stop_id,stop_sequence\nT1,notatime,08:00:00,S1,1\nT1,08:10:00,08:10:00,S2,2\n")]),
        fx("STM_004", vec![("stop_times.txt", "trip_id,arrival_time,departure_time,stop_id,stop_sequence\nT1,08:00:00,notatime,S1,1\nT1,08:10:00,08:10:00,S2,2\n")]),
        fx("STM_005", vec![("stop_times.txt", "trip_id,arrival_time,departure_time,stop_id,stop_sequence\nT1,08:00:00,08:00:00,S1,\nT1,08:10:00,08:10:00,S2,2\n")]),
        fx("STM_006", vec![("stop_times.txt", "trip_id,arrival_time,departure_time,stop_id,stop_sequence\nT1,08:00:00,08:00:00,,1\nT1,08:10:00,08:10:00,S2,2\n")]),
        fx("STM_009", vec![("stop_times.txt", "trip_id,arrival_time,departure_time,stop_id,stop_sequence,pickup_type\nT1,08:00:00,08:00:00,S1,1,9\nT1,08:10:00,08:10:00,S2,2,0\n")]),
        fx("STM_010", vec![("stop_times.txt", "trip_id,arrival_time,departure_time,stop_id,stop_sequence,drop_off_type\nT1,08:00:00,08:00:00,S1,1,9\nT1,08:10:00,08:10:00,S2,2,0\n")]),
        fx("STM_018", vec![("stop_times.txt", "trip_id,arrival_time,departure_time,stop_id,stop_sequence,continuous_pickup\nT1,08:00:00,08:00:00,S1,1,9\nT1,08:10:00,08:10:00,S2,2,1\n")]),
        fx("STM_019", vec![("stop_times.txt", "trip_id,arrival_time,departure_time,stop_id,stop_sequence,continuous_drop_off\nT1,08:00:00,08:00:00,S1,1,9\nT1,08:10:00,08:10:00,S2,2,1\n")]),
        fx("STM_022", vec![("stop_times.txt", "trip_id,arrival_time,departure_time,stop_id,stop_sequence,timepoint\nT1,08:00:00,08:00:00,S1,1,5\nT1,08:10:00,08:10:00,S2,2,1\n")]),
        fx("STM_030", vec![("stop_times.txt", "trip_id,arrival_time,departure_time,stop_id,stop_sequence,shape_dist_traveled\nT1,08:00:00,08:00:00,S1,1,-5\nT1,08:10:00,08:10:00,S2,2,10\n")]),
        fx("STM_034", vec![("stop_times.txt", "trip_id,arrival_time,departure_time,stop_id,stop_sequence\nT1,08:00:00,,S1,1\nT1,08:10:00,08:10:00,S2,2\n")]),
        fx("STM_046", vec![("stop_times.txt", "trip_id,arrival_time,departure_time,stop_id,stop_sequence\n,08:00:00,08:00:00,S1,1\nT1,08:10:00,08:10:00,S2,2\n")]),
        fx("STM_047", vec![("stop_times.txt", "trip_id,arrival_time,departure_time,stop_id,stop_sequence,timepoint\nT1,,,S1,1,1\nT1,08:10:00,08:10:00,S2,2,1\n")]),
        // Flex (has_flex_cols → window sütunları)
        fx("STM_037", vec![("stop_times.txt", "trip_id,arrival_time,departure_time,stop_id,stop_sequence,start_pickup_drop_off_window,end_pickup_drop_off_window\nT1,08:00:00,08:00:00,S1,1,09:00:00,10:00:00\n")]),
        fx("STM_038", vec![("stop_times.txt", "trip_id,stop_id,stop_sequence,start_pickup_drop_off_window,end_pickup_drop_off_window\nT1,S1,1,10:00:00,09:00:00\n")]),
        fx("STM_039", vec![("stop_times.txt", "trip_id,stop_sequence,location_id,start_pickup_drop_off_window\nT1,1,LOC1,09:00:00\n")]),
        fx("STM_040", vec![("stop_times.txt", "trip_id,stop_sequence,location_id,start_pickup_drop_off_window,end_pickup_drop_off_window\nT1,1,LOC1,09:00:00,10:00:00\n")]),
        fx("STM_041", vec![("stop_times.txt", "trip_id,stop_id,stop_sequence,location_id,start_pickup_drop_off_window,end_pickup_drop_off_window,pickup_booking_rule_id\nT1,S1,1,LOC1,09:00:00,10:00:00,BR1\n")]),
        fx("STM_051", vec![("stop_times.txt", "trip_id,stop_sequence,location_id,start_pickup_drop_off_window,end_pickup_drop_off_window,pickup_booking_rule_id,pickup_type\nT1,1,LOC1,09:00:00,10:00:00,BR1,0\n")]),
        fx("STM_052", vec![("stop_times.txt", "trip_id,stop_sequence,location_id,start_pickup_drop_off_window,end_pickup_drop_off_window,pickup_booking_rule_id,drop_off_type\nT1,1,LOC1,09:00:00,10:00:00,BR1,0\n")]),

        // ── PTH grubu (pathways.txt; base'te yok, eklenir) ─────────────────────
        // K2 satır kontrolleri (k2/pathways.rs). Temiz satır şablonu: mode=3, bidir=0.
        fx("PTH_004", vec![("pathways.txt", "pathway_id,from_stop_id,to_stop_id,pathway_mode,is_bidirectional\nP1,S1,S2,9,0\n")]),
        fx("PTH_005", vec![("pathways.txt", "pathway_id,from_stop_id,to_stop_id,pathway_mode,is_bidirectional\nP1,S1,S2,3,5\n")]),
        fx("PTH_006", vec![("pathways.txt", "pathway_id,from_stop_id,to_stop_id,pathway_mode,is_bidirectional,length\nP1,S1,S2,3,0,-1\n")]),
        fx("PTH_007", vec![("pathways.txt", "pathway_id,from_stop_id,to_stop_id,pathway_mode,is_bidirectional,traversal_time\nP1,S1,S2,3,0,0\n")]),
        fx("PTH_008", vec![("pathways.txt", "pathway_id,from_stop_id,to_stop_id,pathway_mode,is_bidirectional\nP1,S1,S2,2,0\n")]),
        fx("PTH_025", vec![("pathways.txt", "pathway_id,from_stop_id,to_stop_id,pathway_mode,is_bidirectional,length\nP1,S1,S2,6,0,\n")]),
        fx("PTH_009", vec![("pathways.txt", "pathway_id,from_stop_id,to_stop_id,pathway_mode,is_bidirectional\nP1,S1,S2,1,0\n")]),
        fx("PTH_010", vec![("pathways.txt", "pathway_id,from_stop_id,to_stop_id,pathway_mode,is_bidirectional,min_width\nP1,S1,S2,3,0,0\n")]),
        fx("PTH_011", vec![("pathways.txt", "pathway_id,from_stop_id,to_stop_id,pathway_mode,is_bidirectional\nP1,S1,S1,3,0\n")]),
        fx("PTH_016", vec![("pathways.txt", "pathway_id,from_stop_id,to_stop_id,pathway_mode,is_bidirectional\nP1,S1,S2,7,1\n")]),
        fx("PTH_017", vec![("pathways.txt", "pathway_id,from_stop_id,to_stop_id,pathway_mode,is_bidirectional,max_slope\nP1,S1,S2,4,0,5\n")]),
        fx("PTH_018", vec![("pathways.txt", concat!("pathway_id,from_stop_id,to_stop_id,pathway_mode,is_bidirectional,signposted_as\nP1,S1,S2,3,0,",
            "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa\n"))]),
        fx("PTH_020", vec![("pathways.txt", "pathway_id,from_stop_id,to_stop_id,pathway_mode,is_bidirectional\n,S1,S2,3,0\n")]),
        fx("PTH_021", vec![("pathways.txt", "pathway_id,from_stop_id,to_stop_id,pathway_mode,is_bidirectional\nP1,,S2,3,0\n")]),
        fx("PTH_022", vec![("pathways.txt", "pathway_id,from_stop_id,to_stop_id,pathway_mode,is_bidirectional\nP1,S1,,3,0\n")]),
        fx("PTH_023", vec![("pathways.txt", "pathway_id,from_stop_id,to_stop_id,pathway_mode,is_bidirectional\nP1,S1,S2,,0\n")]),
        fx("PTH_024", vec![("pathways.txt", "pathway_id,from_stop_id,to_stop_id,pathway_mode,is_bidirectional\nP1,S1,S2,3,\n")]),
        // K3: pathway_id duplicate (k3_entity_graph::build_pathways).
        fx("PTH_001", vec![("pathways.txt", "pathway_id,from_stop_id,to_stop_id,pathway_mode,is_bidirectional\nP1,S1,S2,3,0\nP1,S2,S1,3,0\n")]),
        // K4: cross-ref (k4_cross_ref::check_pathways).
        fx("PTH_002", vec![("pathways.txt", "pathway_id,from_stop_id,to_stop_id,pathway_mode,is_bidirectional\nP1,NOPE,S2,3,0\n")]),
        fx("PTH_003", vec![("pathways.txt", "pathway_id,from_stop_id,to_stop_id,pathway_mode,is_bidirectional\nP1,S1,NOPE,3,0\n")]),
        // PTH_014: from/to farklı istasyonlarda (parent_station ile çözülür).
        fx("PTH_014", vec![
            ("stops.txt", "stop_id,stop_name,stop_lat,stop_lon,location_type,parent_station\nS1,Stop1,41.0,29.0,0,\nS2,Stop2,41.1,29.1,0,\nST_A,StationA,41.0,29.0,1,\nST_B,StationB,41.2,29.2,1,\nP_A,PlatA,41.0,29.0,0,ST_A\nP_B,PlatB,41.2,29.2,0,ST_B\n"),
            ("pathways.txt", "pathway_id,from_stop_id,to_stop_id,pathway_mode,is_bidirectional\nP1,P_A,P_B,3,0\n"),
        ]),
        // PTH_019: generic node (location_type=3) tek pathway'e bağlı → çıkmaz.
        fx("PTH_019", vec![
            ("stops.txt", "stop_id,stop_name,stop_lat,stop_lon,location_type\nS1,Stop1,41.0,29.0,0\nS2,Stop2,41.1,29.1,0\nGN,Node,41.05,29.05,3\n"),
            ("pathways.txt", "pathway_id,from_stop_id,to_stop_id,pathway_mode,is_bidirectional\nP1,GN,S1,3,0\n"),
        ]),
        // K6 analitik (k6_analytics::check_pathway_analytics).
        // PTH_012: istasyonda entrance (location_type=2) yok, platform pathway grafında.
        fx("PTH_012", vec![
            ("stops.txt", "stop_id,stop_name,stop_lat,stop_lon,location_type,parent_station\nS1,Stop1,41.0,29.0,0,\nS2,Stop2,41.1,29.1,0,\nST1,Station,41.0,29.0,1,\nPLAT,Platform,41.0,29.0,0,ST1\nGN,Node,41.01,29.01,3,ST1\n"),
            ("pathways.txt", "pathway_id,from_stop_id,to_stop_id,pathway_mode,is_bidirectional\nP1,PLAT,GN,3,0\n"),
        ]),
        // PTH_013: entrance→platform rotası var ama erişilebilir değil (max_slope>8%).
        fx("PTH_013", vec![
            ("stops.txt", "stop_id,stop_name,stop_lat,stop_lon,location_type,parent_station\nS1,Stop1,41.0,29.0,0,\nS2,Stop2,41.1,29.1,0,\nST1,Station,41.0,29.0,1,\nENT,Entrance,41.0,29.0,2,ST1\nPLAT,Platform,41.0,29.0,0,ST1\n"),
            ("pathways.txt", "pathway_id,from_stop_id,to_stop_id,pathway_mode,is_bidirectional,max_slope\nP1,ENT,PLAT,1,1,0.5\n"),
        ]),
        // PTH_015: length/traversal_time'dan türetilen hız > 3 m/s (100m / 10s = 10 m/s).
        fx("PTH_015", vec![("pathways.txt", "pathway_id,from_stop_id,to_stop_id,pathway_mode,is_bidirectional,length,traversal_time\nP1,S1,S2,3,0,100,10\n")]),

        // ── JPN grubu (GTFS-JP; k4_cross_ref::check_gtfs_jp) ───────────────────
        // Kapı: feed_lang=ja* VEYA office_jp/agency_jp dosyası (is_gtfs_jp).
        // JPN_001/008/009/010 ek kapı: feed_lang ja* VEYA herhangi ja-Hrkt çeviri.
        // JPN_001: durak adında kana (ja-Hrkt) okuması yok (base duraklar).
        fx("JPN_001", vec![("feed_info.txt", "feed_publisher_name,feed_publisher_url,feed_lang\nPub,https://x.example,ja\n")]),
        // JPN_008: route_long_name dolu ama kana okuması yok.
        fx("JPN_008", vec![
            ("feed_info.txt", "feed_publisher_name,feed_publisher_url,feed_lang\nPub,https://x.example,ja\n"),
            ("routes.txt", "route_id,agency_id,route_short_name,route_type,route_long_name\nR1,1,101,3,渋谷線\n"),
        ]),
        // JPN_009: trip_headsign dolu ama kana okuması yok.
        fx("JPN_009", vec![
            ("feed_info.txt", "feed_publisher_name,feed_publisher_url,feed_lang\nPub,https://x.example,ja\n"),
            ("trips.txt", "route_id,service_id,trip_id,trip_headsign\nR1,SVC1,T1,渋谷\n"),
        ]),
        // JPN_010: agency_name dolu ama kana okuması yok (base agency).
        fx("JPN_010", vec![("feed_info.txt", "feed_publisher_name,feed_publisher_url,feed_lang\nPub,https://x.example,ja\n")]),
        // JPN_002: trips.jp_office_id office_jp.txt'te tanımsız.
        fx("JPN_002", vec![
            ("office_jp.txt", "office_id,office_name\nO1,Office1\n"),
            ("trips.txt", "route_id,service_id,trip_id,jp_office_id\nR1,SVC1,T1,BADREF\n"),
        ]),
        // JPN_003: agency_jp.agency_id agency.txt'te tanımsız.
        fx("JPN_003", vec![("agency_jp.txt", "agency_id\nNOPE\n")]),
        // JPN_004: GTFS-JP sinyali (office_jp) var ama translations.txt yok.
        fx("JPN_004", vec![("office_jp.txt", "office_id,office_name\nO1,Office1\n")]),
        // JPN_005: office_jp.office_name boş.
        fx("JPN_005", vec![("office_jp.txt", "office_id,office_name\nO1,\n")]),
        // JPN_006: GTFS-JP (feed_lang=ja) ama fare_attributes/fare_rules yok.
        fx("JPN_006", vec![("feed_info.txt", "feed_publisher_name,feed_publisher_url,feed_lang\nPub,https://x.example,ja\n")]),
        // JPN_007: GTFS-JP sinyali (office_jp) var ama feed_info.txt yok.
        fx("JPN_007", vec![("office_jp.txt", "office_id,office_name\nO1,Office1\n")]),
        // JPN_011: GTFS-JP'de agency_id boş (tek işletici olsa bile zorunlu).
        fx("JPN_011", vec![
            ("feed_info.txt", "feed_publisher_name,feed_publisher_url,feed_lang\nPub,https://x.example,ja\n"),
            ("agency.txt", "agency_id,agency_name,agency_url,agency_timezone\n,Test,http://test.example,UTC\n"),
        ]),

        // ── LOC grubu (locations.geojson; k1_parse::validate_locations_geojson) ─
        // LOC_001: geçersiz JSON.
        fx("LOC_001", vec![("locations.geojson", "not valid json")]),
        // LOC_005: FeatureCollection boş.
        fx("LOC_005", vec![("locations.geojson", "{\"type\":\"FeatureCollection\",\"features\":[]}")]),
        // LOC_007: yinelenen feature 'id'.
        fx("LOC_007", vec![("locations.geojson", "{\"type\":\"FeatureCollection\",\"features\":[{\"type\":\"Feature\",\"id\":\"L1\",\"geometry\":{\"type\":\"Polygon\",\"coordinates\":[[[0,0],[0,0.01],[0.01,0.01],[0,0]]]}},{\"type\":\"Feature\",\"id\":\"L1\",\"geometry\":{\"type\":\"Polygon\",\"coordinates\":[[[0,0],[0,0.01],[0.01,0.01],[0,0]]]}}]}")]),
        // LOC_003: feature'da 'id' yok.
        fx("LOC_003", vec![("locations.geojson", "{\"type\":\"FeatureCollection\",\"features\":[{\"type\":\"Feature\",\"geometry\":{\"type\":\"Polygon\",\"coordinates\":[[[0,0],[0,0.01],[0.01,0.01],[0,0]]]}}]}")]),
        // LOC_002: geometry null.
        fx("LOC_002", vec![("locations.geojson", "{\"type\":\"FeatureCollection\",\"features\":[{\"type\":\"Feature\",\"id\":\"L1\",\"geometry\":null}]}")]),
        // LOC_004: Polygon ring kapalı değil (ilk≠son nokta).
        fx("LOC_004", vec![("locations.geojson", "{\"type\":\"FeatureCollection\",\"features\":[{\"type\":\"Feature\",\"id\":\"L1\",\"geometry\":{\"type\":\"Polygon\",\"coordinates\":[[[0,0],[0,0.01],[0.01,0.01],[0.01,0]]]}}]}")]),
        // LOC_006: bbox alanı > 500km² (~5°×5°).
        fx("LOC_006", vec![("locations.geojson", "{\"type\":\"FeatureCollection\",\"features\":[{\"type\":\"Feature\",\"id\":\"L1\",\"geometry\":{\"type\":\"Polygon\",\"coordinates\":[[[0,0],[0,5],[5,5],[0,0]]]}}]}")]),

        // ── GEO grubu (coğrafi analitik; k6_analytics::check_geo_analytics) ─────
        // Config varsayılan: max_shape_jump_km=10, stop_too_close_m=5, stop_far_from_shape_m=100.
        // GEO_006: shape segmenti > 10km atlama (~22km).
        fx("GEO_006", vec![("shapes.txt", "shape_id,shape_pt_lat,shape_pt_lon,shape_pt_sequence\nSH1,40.0,40.0,1\nSH1,40.2,40.0,2\n")]),
        // GEO_007: kritik shape atlaması > 30km (3× eşik, ~55km).
        fx("GEO_007", vec![("shapes.txt", "shape_id,shape_pt_lat,shape_pt_lon,shape_pt_sequence\nSH1,40.0,40.0,1\nSH1,40.5,40.0,2\n")]),
        // GEO_009: durak kendi seferinin shape'inden > 100m uzak.
        fx("GEO_009", vec![
            ("stops.txt", "stop_id,stop_name,stop_lat,stop_lon\nS1,Stop1,40.0,40.0\nS2,Stop2,40.05,40.05\n"),
            ("trips.txt", "route_id,service_id,trip_id,shape_id\nR1,SVC1,T1,SH1\n"),
            ("shapes.txt", "shape_id,shape_pt_lat,shape_pt_lon,shape_pt_sequence\nSH1,40.0,40.0,1\nSH1,40.0,40.1,2\n"),
        ]),
        // GEO_002: durak feed medianından > 200km uzak (>=3 koordinatlı durak).
        fx("GEO_002", vec![("stops.txt", "stop_id,stop_name,stop_lat,stop_lon\nS1,Stop1,41.0,29.0\nS2,Stop2,41.1,29.1\nS3,Far,10.123,10.123\n")]),
        // GEO_012: 3+ durak 5m içinde kümelenmiş.
        fx("GEO_012", vec![("stops.txt", "stop_id,stop_name,stop_lat,stop_lon\nS1,Stop1,40.0,40.0\nS2,Stop2,40.00001,40.0\nS3,Stop3,40.0,40.00001\n")]),
        // GEO_013: koordinatlı durak içeren feed (Bilgi) — base zaten tetikler.
        fx("GEO_013", vec![]),
        // GEO_014: feed bbox köşegeni > 500km (Bilgi).
        fx("GEO_014", vec![("stops.txt", "stop_id,stop_name,stop_lat,stop_lon\nS1,Stop1,40.0,29.0\nS2,Stop2,45.0,35.0\n")]),
        // GEO_015: feed_lang=ja ama durak Japonya sınırları dışında (base lon 29).
        fx("GEO_015", vec![("feed_info.txt", "feed_publisher_name,feed_publisher_url,feed_lang\nPub,https://x.example,ja\n")]),
        // GEO_016: durak Null Island yakınında (|lat|,|lon| < 0.1).
        fx("GEO_016", vec![("stops.txt", "stop_id,stop_name,stop_lat,stop_lon\nS1,Stop1,0.01,0.01\nS2,Stop2,41.1,29.1\n")]),
        // GEO_017: shape noktası Null Island yakınında.
        fx("GEO_017", vec![("shapes.txt", "shape_id,shape_pt_lat,shape_pt_lon,shape_pt_sequence\nSH1,0.01,0.01,1\nSH1,0.02,0.02,2\n")]),
        // GEO_018: tüm duraklar < 200m'lik alanda (test/placeholder veri).
        fx("GEO_018", vec![("stops.txt", "stop_id,stop_name,stop_lat,stop_lon\nS1,Stop1,40.0,40.0\nS2,Stop2,40.0005,40.0\nS3,Stop3,40.0,40.0005\n")]),
        // GEO_019: tam sayı koordinat — base S1 (41.0,29.0) zaten tetikler.
        fx("GEO_019", vec![]),
        // GEO_020: shape'in tüm noktaları aynı koordinatta (dejenere geometri).
        fx("GEO_020", vec![("shapes.txt", "shape_id,shape_pt_lat,shape_pt_lon,shape_pt_sequence\nSH1,40.0,40.0,1\nSH1,40.0,40.0,2\n")]),
        // GEO_021: durakların > %30'u koordinat paylaşıyor (total>=5).
        fx("GEO_021", vec![("stops.txt", "stop_id,stop_name,stop_lat,stop_lon\nS1,A,40.1,40.1\nS2,B,40.1,40.1\nS3,C,40.1,40.1\nS4,D,40.1,40.1\nS5,E,41.1,29.1\n")]),
        // GEO_022: durak enlemi kutba aşırı yakın (|lat| > 89).
        fx("GEO_022", vec![("stops.txt", "stop_id,stop_name,stop_lat,stop_lon\nS1,Stop1,89.5,29.0\nS2,Stop2,41.1,29.1\n")]),

        // ── DQ grubu (veri kalitesi; k6_analytics::check_data_quality + k1_parse) ─
        // DQ_003: route_desc boş — base R1 zaten tetikler.
        fx("DQ_003", vec![]),
        // DQ_004: route_url boş — base R1 zaten tetikler.
        fx("DQ_004", vec![]),
        // DQ_005: bugün itibarıyla aktif servis yok (takvim süresi dolmuş).
        fx("DQ_005", vec![("calendar.txt", "service_id,monday,tuesday,wednesday,thursday,friday,saturday,sunday,start_date,end_date\nSVC1,1,1,1,1,1,0,0,20200101,20201231\n")]),
        // DQ_005b: hiçbir trip'in stop_times kaydı yok.
        fx("DQ_005b", vec![("stop_times.txt", "trip_id,arrival_time,departure_time,stop_id,stop_sequence\n")]),
        // DQ_005c: durakların > %50'sinde koordinat eksik.
        fx("DQ_005c", vec![("stops.txt", "stop_id,stop_name,stop_lat,stop_lon\nS1,Stop1,,\nS2,Stop2,,\n")]),
        // DQ_006: trips'in > %80'inde shape_id eksik — base (shape'siz tek trip) tetikler.
        fx("DQ_006", vec![]),
        // DQ_009: trip var ama stop_times tamamen boş.
        fx("DQ_009", vec![("stop_times.txt", "trip_id,arrival_time,departure_time,stop_id,stop_sequence\n")]),
        // DQ_010: hiçbir hatta kullanılmayan agency (agency_id=2 boşta).
        fx("DQ_010", vec![("agency.txt", "agency_id,agency_name,agency_url,agency_timezone\n1,A,https://a.example,UTC\n2,B,https://b.example,UTC\n")]),
        // DQ_011: feed'de yalnızca 1 durak.
        fx("DQ_011", vec![("stops.txt", "stop_id,stop_name,stop_lat,stop_lon\nS1,Stop1,41.0,29.0\n")]),
        // DQ_012: >5 agency ve hiçbir rotada agency_id yok.
        fx("DQ_012", vec![
            ("agency.txt", "agency_id,agency_name,agency_url,agency_timezone\n1,A,https://a.example,UTC\n2,B,https://b.example,UTC\n3,C,https://c.example,UTC\n4,D,https://d.example,UTC\n5,E,https://e.example,UTC\n6,F,https://f.example,UTC\n"),
            ("routes.txt", "route_id,agency_id,route_short_name,route_type\nR1,,101,3\n"),
        ]),
        // DQ_013: feed'de < 3 sefer — base (tek trip) tetikler.
        fx("DQ_013", vec![]),
        // DQ_016: değerde baştaki/sondaki boşluk (k1_parse).
        fx("DQ_016", vec![("stops.txt", "stop_id,stop_name,stop_lat,stop_lon\nS1, Stop1 ,41.0,29.0\nS2,Stop2,41.1,29.1\n")]),
        // DQ_017: şüpheli koordinat (|lat|,|lon| < 1°).
        fx("DQ_017", vec![("stops.txt", "stop_id,stop_name,stop_lat,stop_lon\nS1,Stop1,0.5,0.5\nS2,Stop2,41.1,29.1\n")]),
        // DQ_018: önerilen metin alanı tamamen büyük harf (stop_name).
        fx("DQ_018", vec![("stops.txt", "stop_id,stop_name,stop_lat,stop_lon\nS1,MERKEZ,41.0,29.0\nS2,Stop2,41.1,29.1\n")]),
        // DQ_019: önerilen metin alanı tamamen küçük harf (stop_name).
        fx("DQ_019", vec![("stops.txt", "stop_id,stop_name,stop_lat,stop_lon\nS1,merkez,41.0,29.0\nS2,Stop2,41.1,29.1\n")]),
        // DQ_020: önerilen trip_headsign boş — base (headsign'sız trip) tetikler.
        fx("DQ_020", vec![]),
        // DQ_021: birincil anahtar yineleniyor (stop_id duplicate).
        fx("DQ_021", vec![("stops.txt", "stop_id,stop_name,stop_lat,stop_lon\nS1,A,40.0,40.0\nS1,B,41.1,29.1\n")]),
        // DQ_022: durakların > %80'i aynı stop_name (total>=5).
        fx("DQ_022", vec![("stops.txt", "stop_id,stop_name,stop_lat,stop_lon\nS1,Stop,40.0,40.0\nS2,Stop,40.1,40.1\nS3,Stop,40.2,40.2\nS4,Stop,40.3,40.3\nS5,Stop,40.4,40.4\n")]),

        // ── OPR grubu (operasyonel analitik; k6_analytics) ─────────────────────
        // Config: max_headway_warning_min=240, bunching_threshold_min=2, max_speed_bus=120,
        // service_gap_days=7, max_trips_per_route=500, headway_outlier_sigma=2.5.
        // NOT: OPR_021/022/023 yalnız config.calendar_override_rules ile (default'ta imkânsız);
        // OPR_024 >500 sefer gerektirir (inline yazılamaz) → debt'te bırakıldı.
        // OPR_001: hat genelinde maksimum sefer aralığı > 240dk ve düzensiz.
        fx("OPR_001", vec![
            ("trips.txt", "route_id,service_id,trip_id\nR1,SVC1,T1\nR1,SVC1,T2\nR1,SVC1,T3\n"),
            ("stop_times.txt", "trip_id,arrival_time,departure_time,stop_id,stop_sequence\nT1,08:00:00,08:00:00,S1,1\nT1,08:05:00,08:05:00,S2,2\nT2,08:10:00,08:10:00,S1,1\nT2,08:15:00,08:15:00,S2,2\nT3,18:00:00,18:00:00,S1,1\nT3,18:05:00,18:05:00,S2,2\n"),
        ]),
        // OPR_003: aynı kalkış durağında ardışık seferler 2dk'dan sık (sıkışma).
        fx("OPR_003", vec![
            ("trips.txt", "route_id,service_id,trip_id\nR1,SVC1,T1\nR1,SVC1,T2\n"),
            ("stop_times.txt", "trip_id,arrival_time,departure_time,stop_id,stop_sequence\nT1,08:00:00,08:00:00,S1,1\nT1,08:10:00,08:10:00,S2,2\nT2,08:01:00,08:01:00,S1,1\nT2,08:11:00,08:11:00,S2,2\n"),
        ]),
        // OPR_004: hatta hafta sonu servisi yok — base (SVC1 Pzt-Cum) tetikler.
        fx("OPR_004", vec![]),
        // OPR_005: route_type-göreli headway aykırısı (6 grup; R6 6× medyan).
        fx("OPR_005", vec![
            ("routes.txt", "route_id,agency_id,route_short_name,route_type\nR1,1,1,3\nR2,1,2,3\nR3,1,3,3\nR4,1,4,3\nR5,1,5,3\nR6,1,6,3\n"),
            ("trips.txt", "route_id,service_id,trip_id\nR1,SVC1,T1\nR1,SVC1,T2\nR2,SVC1,T3\nR2,SVC1,T4\nR3,SVC1,T5\nR3,SVC1,T6\nR4,SVC1,T7\nR4,SVC1,T8\nR5,SVC1,T9\nR5,SVC1,T10\nR6,SVC1,T11\nR6,SVC1,T12\n"),
            ("stop_times.txt", "trip_id,arrival_time,departure_time,stop_id,stop_sequence\nT1,08:00:00,08:00:00,S1,1\nT2,08:10:00,08:10:00,S1,1\nT3,08:00:00,08:00:00,S1,1\nT4,08:10:00,08:10:00,S1,1\nT5,08:00:00,08:00:00,S1,1\nT6,08:10:00,08:10:00,S1,1\nT7,08:00:00,08:00:00,S1,1\nT8,08:10:00,08:10:00,S1,1\nT9,08:00:00,08:00:00,S1,1\nT10,08:10:00,08:10:00,S1,1\nT11,08:00:00,08:00:00,S1,1\nT12,09:00:00,09:00:00,S1,1\n"),
        ]),
        // OPR_006: seferde < 2 durak.
        fx("OPR_006", vec![("stop_times.txt", "trip_id,arrival_time,departure_time,stop_id,stop_sequence\nT1,08:00:00,08:00:00,S1,1\n")]),
        // OPR_007: seferde ardışık olmayan aynı durak tekrarı (ring değil).
        fx("OPR_007", vec![("stop_times.txt", "trip_id,arrival_time,departure_time,stop_id,stop_sequence\nT1,08:00:00,08:00:00,S1,1\nT1,08:10:00,08:10:00,S2,2\nT1,08:20:00,08:20:00,S3,3\nT1,08:30:00,08:30:00,S2,4\n")]),
        // OPR_008: seferde >1 bozuk hız segmenti (~660 km/h, eşik bus 120).
        fx("OPR_008", vec![
            ("stops.txt", "stop_id,stop_name,stop_lat,stop_lon\nS1,Stop1,40.0,40.0\nS2,Stop2,40.05,40.0\nS3,Stop3,40.10,40.0\n"),
            ("stop_times.txt", "trip_id,arrival_time,departure_time,stop_id,stop_sequence\nT1,08:00:00,08:00:00,S1,1\nT1,08:00:30,08:00:30,S2,2\nT1,08:01:00,08:01:00,S3,3\n"),
        ]),
        // OPR_009: gece seferi (kalkış >= 23:00).
        fx("OPR_009", vec![("stop_times.txt", "trip_id,arrival_time,departure_time,stop_id,stop_sequence\nT1,23:30:00,23:30:00,S1,1\nT1,23:40:00,23:40:00,S2,2\n")]),
        // OPR_010: hatta wheelchair_accessible tutarsız (1 ve 2).
        fx("OPR_010", vec![("trips.txt", "route_id,service_id,trip_id,wheelchair_accessible\nR1,SVC1,T1,1\nR1,SVC1,T2,2\n")]),
        // OPR_011: trip'in service_id'si aktif gün içermiyor (takvimde yok).
        fx("OPR_011", vec![("trips.txt", "route_id,service_id,trip_id\nR1,SVCX,T1\n")]),
        // OPR_012: serviste >= 7 günlük boşluk (calendar_dates iki uzak tarih).
        fx_rm("OPR_012",
            vec![("calendar_dates.txt", "service_id,date,exception_type\nSVC1,20260518,1\nSVC1,20260601,1\n")],
            vec!["calendar.txt"]),
        // OPR_013: hattın tüm seferleri tek yönde (direction_id tek değer).
        fx("OPR_013", vec![("trips.txt", "route_id,service_id,trip_id,direction_id\nR1,SVC1,T1,0\n")]),
        // OPR_014: feed genelinde ortalama aktarma süresi > 10dk (type=2).
        fx("OPR_014", vec![("transfers.txt", "from_stop_id,to_stop_id,transfer_type,min_transfer_time\nS1,S2,2,1200\n")]),
        // OPR_015: çift yönlü hat tek shape kullanıyor (bus).
        fx("OPR_015", vec![
            ("trips.txt", "route_id,service_id,trip_id,shape_id,direction_id\nR1,SVC1,T1,SH1,0\nR1,SVC1,T2,SH1,1\n"),
            ("shapes.txt", "shape_id,shape_pt_lat,shape_pt_lon,shape_pt_sequence\nSH1,41.0,29.0,1\nSH1,41.1,29.1,2\n"),
        ]),
        // OPR_016: feed'de hiçbir service aktif gün içermiyor (tüm flag 0).
        fx("OPR_016", vec![("calendar.txt", "service_id,monday,tuesday,wednesday,thursday,friday,saturday,sunday,start_date,end_date\nSVC1,0,0,0,0,0,0,0,20260518,20260524\n")]),
        // OPR_017: sefer çok kısa mesafe (< 100m, shape'siz → durak koordinatı).
        fx("OPR_017", vec![("stops.txt", "stop_id,stop_name,stop_lat,stop_lon\nS1,Stop1,40.0,40.0\nS2,Stop2,40.0005,40.0\n")]),
        // OPR_019: hatta aynı (exception'sız) günde >1 aktif servis.
        fx("OPR_019", vec![
            ("calendar.txt", "service_id,monday,tuesday,wednesday,thursday,friday,saturday,sunday,start_date,end_date\nSVC1,1,1,1,1,1,1,1,20260518,20260524\nSVC2,1,1,1,1,1,1,1,20260518,20260524\n"),
            ("trips.txt", "route_id,service_id,trip_id\nR1,SVC1,T1\nR1,SVC2,T2\n"),
        ]),
        // OPR_020: hatta exception gününde >1 aktif servis (override çakışması).
        fx("OPR_020", vec![
            ("calendar.txt", "service_id,monday,tuesday,wednesday,thursday,friday,saturday,sunday,start_date,end_date\nSVC1,1,1,1,1,1,1,1,20260518,20260518\n"),
            ("calendar_dates.txt", "service_id,date,exception_type\nSVC2,20260518,1\n"),
            ("trips.txt", "route_id,service_id,trip_id\nR1,SVC1,T1\nR1,SVC2,T2\n"),
        ]),
        // OPR_025: feed ortalama sefer süresi < 60s (5 trip, 10s'lik).
        fx("OPR_025", vec![
            ("trips.txt", "route_id,service_id,trip_id\nR1,SVC1,T1\nR1,SVC1,T2\nR1,SVC1,T3\nR1,SVC1,T4\nR1,SVC1,T5\n"),
            ("stop_times.txt", "trip_id,arrival_time,departure_time,stop_id,stop_sequence\nT1,08:00:00,08:00:00,S1,1\nT1,08:00:10,08:00:10,S2,2\nT2,08:00:00,08:00:00,S1,1\nT2,08:00:10,08:00:10,S2,2\nT3,08:00:00,08:00:00,S1,1\nT3,08:00:10,08:00:10,S2,2\nT4,08:00:00,08:00:00,S1,1\nT4,08:00:10,08:00:10,S2,2\nT5,08:00:00,08:00:00,S1,1\nT5,08:00:10,08:00:10,S2,2\n"),
        ]),

        // ── XFL grubu (cross-file FK; k4_cross_ref::check_xfl + cemv/fares v2) ──
        // XFL_001: trip service_id calendar/calendar_dates'te yok.
        fx("XFL_001", vec![("trips.txt", "route_id,service_id,trip_id\nR1,SVCX,T1\n")]),
        // XFL_002: trip'in stop_times kaydı yok (T2).
        fx("XFL_002", vec![("trips.txt", "route_id,service_id,trip_id\nR1,SVC1,T1\nR1,SVC1,T2\n")]),
        // XFL_003: trip shape_id shapes.txt'te yok.
        fx("XFL_003", vec![("trips.txt", "route_id,service_id,trip_id,shape_id\nR1,SVC1,T1,NOPE\n")]),
        // XFL_004: fare_rules route_id routes.txt'te yok.
        fx("XFL_004", vec![("fare_rules.txt", "fare_id,route_id\nF1,NOPE\n")]),
        // XFL_005: stop_times stop_id stops.txt'te yok.
        fx("XFL_005", vec![("stop_times.txt", "trip_id,arrival_time,departure_time,stop_id,stop_sequence\nT1,08:00:00,08:00:00,S1,1\nT1,08:10:00,08:10:00,NOPE,2\n")]),
        // XFL_006: calendar_dates yalnız exception_type=2 ve calendar'da yok.
        fx("XFL_006", vec![("calendar_dates.txt", "service_id,date,exception_type\nSVCZ,20260601,2\n")]),
        // XFL_007: route agency_id agency.txt'te yok.
        fx("XFL_007", vec![("routes.txt", "route_id,agency_id,route_short_name,route_type\nR1,NOPE,101,3\n")]),
        // XFL_009: stop level_id levels.txt'te yok.
        fx("XFL_009", vec![("stops.txt", "stop_id,stop_name,stop_lat,stop_lon,level_id\nS1,Stop1,41.0,29.0,NOPE\nS2,Stop2,41.1,29.1,\n")]),
        // XFL_010: frequencies trip_id trips.txt'te yok.
        fx("XFL_010", vec![("frequencies.txt", "trip_id,start_time,end_time,headway_secs\nNOPE,08:00:00,10:00:00,600\n")]),
        // XFL_011: calendar aralığı feed_info penceresiyle tutarsız (cal_start < feed_start).
        fx("XFL_011", vec![("feed_info.txt", "feed_publisher_name,feed_publisher_url,feed_lang,feed_start_date,feed_end_date\nPub,https://x.example,en,20250601,20271231\n")]),
        // XFL_012: route'un tüm seferlerinin stop_times'ı yok (boş stop_times).
        fx("XFL_012", vec![("stop_times.txt", "trip_id,arrival_time,departure_time,stop_id,stop_sequence\n")]),
        // XFL_013: shape hem gidiş (0) hem dönüş (1) yönünde kullanılıyor.
        fx("XFL_013", vec![
            ("trips.txt", "route_id,service_id,trip_id,shape_id,direction_id\nR1,SVC1,T1,SH1,0\nR1,SVC1,T2,SH1,1\n"),
            ("shapes.txt", "shape_id,shape_pt_lat,shape_pt_lon,shape_pt_sequence\nSH1,41.0,29.0,1\nSH1,41.1,29.1,2\n"),
        ]),
        // XFL_014: translation record_id var olmayan kayda işaret ediyor.
        fx("XFL_014", vec![("translations.txt", "table_name,field_name,language,translation,record_id\nstops,stop_name,en,X,NOPE\n")]),
        // XFL_015: attribution geçersiz referans (agency_id).
        fx("XFL_015", vec![("attributions.txt", "attribution_id,organization_name,is_producer,agency_id\nA1,Org,1,NOPE\n")]),
        // XFL_016: translation table_name=feed_info ama feed_info.txt yok.
        fx("XFL_016", vec![("translations.txt", "table_name,field_name,language,translation\nfeed_info,feed_publisher_name,en,X\n")]),
        // XFL_019: network_id hem routes.txt hem route_networks.txt'te (çakışma).
        fx("XFL_019", vec![
            ("routes.txt", "route_id,agency_id,route_short_name,route_type,network_id\nR1,1,101,3,N1\n"),
            ("route_networks.txt", "network_id,route_id\nN1,R1\n"),
        ]),
        // XFL_017: route_cemv_support agency_cemv_support ile çelişiyor.
        fx("XFL_017", vec![
            ("agency.txt", "agency_id,agency_name,agency_url,agency_timezone,cemv_support\n1,Test,http://test.example,UTC,1\n"),
            ("routes.txt", "route_id,agency_id,route_short_name,route_type,cemv_support\nR1,1,101,3,0\n"),
        ]),
        // XFL_020: transfer (trip_id, route_id) çifti geçersiz.
        fx("XFL_020", vec![("transfers.txt", "from_stop_id,to_stop_id,transfer_type,from_trip_id,from_route_id\nS1,S2,1,T1,RWRONG\n")]),
        // XFL_021: from_stop_id, from_trip_id'nin stop_times'ında yok.
        fx("XFL_021", vec![
            ("stops.txt", "stop_id,stop_name,stop_lat,stop_lon\nS1,Stop1,41.0,29.0\nS2,Stop2,41.1,29.1\nS3,Stop3,41.2,29.2\n"),
            ("transfers.txt", "from_stop_id,to_stop_id,transfer_type,from_trip_id\nS3,S2,1,T1\n"),
        ]),
        // XFL_022: location_group_stops group location_groups.txt'te yok.
        fx("XFL_022", vec![("location_group_stops.txt", "location_group_id,stop_id\nLGNOPE,S1\n")]),
        // XFL_023: location_group_stops stop_id stops.txt'te yok.
        fx("XFL_023", vec![
            ("location_groups.txt", "location_group_id,location_group_name\nLG1,Group1\n"),
            ("location_group_stops.txt", "location_group_id,stop_id\nLG1,NOPE\n"),
        ]),
        // XFL_024: stop_times.location_group_id location_groups.txt'te yok (Flex).
        fx("XFL_024", vec![("stop_times.txt", "trip_id,stop_sequence,location_group_id,start_pickup_drop_off_window,end_pickup_drop_off_window\nT1,1,NOPE,09:00:00,10:00:00\n")]),
        // XFL_025: stop_times.location_id locations.geojson'da yok (Flex).
        fx("XFL_025", vec![("stop_times.txt", "trip_id,stop_sequence,location_id,start_pickup_drop_off_window,end_pickup_drop_off_window\nT1,1,NOPE,09:00:00,10:00:00\n")]),
        // XFL_026: route cemv=1 ama uygulanabilir contactless (type3) fare product yok.
        fx("XFL_026", vec![
            ("fare_media.txt", "fare_media_id,fare_media_type\nM3,3\n"),
            ("fare_products.txt", "fare_product_id,fare_media_id,amount,currency\nP1,M3,2.5,USD\n"),
            ("fare_leg_rules.txt", "leg_group_id,network_id,fare_product_id\nLG1,NOTHER,P1\n"),
            ("routes.txt", "route_id,agency_id,route_short_name,route_type,network_id,cemv_support\nR1,1,101,3,N1,1\n"),
        ]),
        // XFL_027: route cemv=2 ama uygulanabilir contactless fare product var (çelişki).
        fx("XFL_027", vec![
            ("fare_media.txt", "fare_media_id,fare_media_type\nM3,3\n"),
            ("fare_products.txt", "fare_product_id,fare_media_id,amount,currency\nP1,M3,2.5,USD\n"),
            ("fare_leg_rules.txt", "leg_group_id,fare_product_id\nLG1,P1\n"),
            ("routes.txt", "route_id,agency_id,route_short_name,route_type,cemv_support\nR1,1,101,3,2\n"),
        ]),
        // XFL_028: agency_cemv_support=1 + Fares v2 var ama type3 media yok.
        fx("XFL_028", vec![
            ("agency.txt", "agency_id,agency_name,agency_url,agency_timezone,cemv_support\n1,Test,http://test.example,UTC,1\n"),
            ("fare_products.txt", "fare_product_id,amount,currency\nP1,2.5,USD\n"),
        ]),
        // XFL_029: route_cemv_support=1 + Fares v2 var ama type3 media yok.
        fx("XFL_029", vec![
            ("routes.txt", "route_id,agency_id,route_short_name,route_type,cemv_support\nR1,1,101,3,1\n"),
            ("fare_products.txt", "fare_product_id,amount,currency\nP1,2.5,USD\n"),
        ]),
        // XFL_030: type3 media var ama hiçbir agency/route'da cemv=1 yok.
        fx("XFL_030", vec![("fare_media.txt", "fare_media_id,fare_media_type\nM3,3\n")]),

        // ── NET / PDW ──────────────────────────────────────────────────────────
        // NET_001: networks.txt network_id tekrarı (k3).
        fx("NET_001", vec![("networks.txt", "network_id,network_name\nN1,Net1\nN1,Net2\n")]),
        // PDW_006: aynı trip+zone içinde örtüşen pickup/drop-off pencereleri (k6 Flex).
        fx("PDW_006", vec![("stop_times.txt", "trip_id,stop_sequence,location_id,start_pickup_drop_off_window,end_pickup_drop_off_window\nT1,1,Z1,09:00:00,10:00:00\nT1,2,Z1,09:30:00,11:00:00\n")]),

        // ── CAL grubu (takvim analitiği k6 + cross-ref k4 + k2) ────────────────
        // TODAY=20260515. CAL_006: tüm günler 0 (k2).
        fx("CAL_006", vec![("calendar.txt", "service_id,monday,tuesday,wednesday,thursday,friday,saturday,sunday,start_date,end_date\nSVC1,0,0,0,0,0,0,0,20250101,20271231\n")]),
        // CAL_007: serviste >= 7 günlük boşluk (calendar_dates, calendar yok). Boşluk GEÇMİŞTE
        // (today=20260515 öncesi) → yakın-gelecek değil, CAL_007 üretir (yakın gelecekte CAL_012
        // onun yerine geçer, #29).
        fx_rm("CAL_007", vec![("calendar_dates.txt", "service_id,date,exception_type\nSVC1,20260401,1\nSVC1,20260420,1\n")], vec!["calendar.txt"]),
        // CAL_008: end_date 30 gün içinde bitiyor (bugün+10).
        fx("CAL_008", vec![("calendar.txt", "service_id,monday,tuesday,wednesday,thursday,friday,saturday,sunday,start_date,end_date\nSVC1,1,1,1,1,1,1,1,20260101,20260525\n")]),
        // CAL_009: tüm servisler sona ermiş (k4 kritik).
        fx("CAL_009", vec![("calendar.txt", "service_id,monday,tuesday,wednesday,thursday,friday,saturday,sunday,start_date,end_date\nSVC1,1,1,1,1,1,0,0,20200101,20201231\n")]),
        // CAL_010: toplam aktif gün <= 7 (1 haftalık pencere).
        fx("CAL_010", vec![("calendar.txt", "service_id,monday,tuesday,wednesday,thursday,friday,saturday,sunday,start_date,end_date\nSVC1,1,1,1,1,1,1,1,20260518,20260524\n")]),
        // CAL_011: hiçbir sefer kullanmıyor (SVC2 boşta).
        fx("CAL_011", vec![("calendar.txt", "service_id,monday,tuesday,wednesday,thursday,friday,saturday,sunday,start_date,end_date\nSVC1,1,1,1,1,1,0,0,20250101,20271231\nSVC2,1,1,1,1,1,0,0,20250101,20271231\n")]),
        // CAL_012: yakın gelecekte servis boşluğu.
        fx_rm("CAL_012", vec![("calendar_dates.txt", "service_id,date,exception_type\nSVC1,20260516,1\nSVC1,20260603,1\n")], vec!["calendar.txt"]),
        // CAL_013: tekil servis süresi dolmuş (k6).
        fx("CAL_013", vec![("calendar.txt", "service_id,monday,tuesday,wednesday,thursday,friday,saturday,sunday,start_date,end_date\nSVC1,1,1,1,1,1,0,0,20190101,20201231\n")]),
        // CAL_014: servis tarihleri feed_info penceresi dışında (servis calendar_dates'te
        // görünmeli → override_counts döngüsüne girsin; base takvim 2025 tarihleri pencere-öncesi).
        fx("CAL_014", vec![
            ("feed_info.txt", "feed_publisher_name,feed_publisher_url,feed_lang,feed_start_date,feed_end_date\nPub,https://x.example,en,20260601,20260630\n"),
            ("calendar_dates.txt", "service_id,date,exception_type\nSVC1,20260615,1\n"),
        ]),
        // CAL_015: en erken aktif tarih gelecekte (feed-level).
        fx("CAL_015", vec![("calendar.txt", "service_id,monday,tuesday,wednesday,thursday,friday,saturday,sunday,start_date,end_date\nSVC1,1,1,1,1,1,1,1,20270101,20270107\n")]),
        // CAL_016: en geç aktif tarih > bugün+2yıl.
        fx("CAL_016", vec![("calendar.txt", "service_id,monday,tuesday,wednesday,thursday,friday,saturday,sunday,start_date,end_date\nSVC1,1,1,1,1,1,0,0,20260101,20300101\n")]),
        // CAL_017: tekil servisin tüm tarihleri gelecekte.
        fx("CAL_017", vec![("calendar.txt", "service_id,monday,tuesday,wednesday,thursday,friday,saturday,sunday,start_date,end_date\nSVC1,1,1,1,1,1,1,1,20270101,20270107\n")]),
        // CAL_018: haftanın tüm günleri pasif, exception_type=1 yok (k4).
        fx("CAL_018", vec![("calendar.txt", "service_id,monday,tuesday,wednesday,thursday,friday,saturday,sunday,start_date,end_date\nSVC1,0,0,0,0,0,0,0,20250101,20271231\n")]),
        // CAL_019: servis tarihleri feed_info geçerlilik penceresi dışında (k4).
        fx("CAL_019", vec![("feed_info.txt", "feed_publisher_name,feed_publisher_url,feed_lang,feed_start_date,feed_end_date\nPub,https://x.example,en,20260601,20271231\n")]),
        // CAL_020: feed geçerlilik penceresi > 5 yıl.
        fx("CAL_020", vec![("feed_info.txt", "feed_publisher_name,feed_publisher_url,feed_lang,feed_start_date,feed_end_date\nPub,https://x.example,en,20200101,20271231\n")]),
        // CAL_021: bugünü kapsıyor ama önümüzdeki 7 günde aktif gün yok.
        fx_rm("CAL_021", vec![("calendar_dates.txt", "service_id,date,exception_type\nSVC1,20260101,1\nSVC1,20261231,1\n")], vec!["calendar.txt"]),
        // CAL_023: end_date >= today_year+3 (sınır yılı dahil; ör. 2026+3=2029 eşikte).
        fx("CAL_023", vec![("calendar.txt", "service_id,monday,tuesday,wednesday,thursday,friday,saturday,sunday,start_date,end_date\nSVC1,1,1,1,1,1,0,0,20260101,20290101\n")]),
        // CAL_024: servis önümüzdeki 7 günde aktif değil (service-başına).
        fx("CAL_024", vec![("calendar.txt", "service_id,monday,tuesday,wednesday,thursday,friday,saturday,sunday,start_date,end_date\nSVC1,1,1,1,1,1,1,1,20270101,20270131\n")]),

        // ── FIN grubu (feed_info.txt k2 + k6) ──────────────────────────────────
        // FIN_007: feed_version eksik.
        fx("FIN_007", vec![("feed_info.txt", "feed_publisher_name,feed_publisher_url,feed_lang\nPub,https://x.example,en\n")]),
        // FIN_008: feed_contact_email geçersiz.
        fx("FIN_008", vec![("feed_info.txt", "feed_publisher_name,feed_publisher_url,feed_lang,feed_contact_email\nPub,https://x.example,en,notanemail\n")]),
        // FIN_009: feed_contact_url geçersiz.
        fx("FIN_009", vec![("feed_info.txt", "feed_publisher_name,feed_publisher_url,feed_lang,feed_contact_url\nPub,https://x.example,en,notaurl\n")]),
        // FIN_010: feed_end_date geçmişte (k6).
        fx("FIN_010", vec![("feed_info.txt", "feed_publisher_name,feed_publisher_url,feed_lang,feed_start_date,feed_end_date\nPub,https://x.example,en,20190101,20200101\n")]),
        // FIN_012: feed_start_date > feed_end_date (k2).
        fx("FIN_012", vec![("feed_info.txt", "feed_publisher_name,feed_publisher_url,feed_lang,feed_start_date,feed_end_date\nPub,https://x.example,en,20271231,20250101\n")]),
        // FIN_013: tek agency'de bile fare_attribute agency_id eksikse tetiklenir (k4).
        fx("FIN_013", vec![
            ("agency.txt", "agency_id,agency_name,agency_url,agency_timezone\n1,A,https://a.example,UTC\n"),
            ("fare_attributes.txt", "fare_id,price,currency_type,payment_method\nF1,2.5,USD,0\n"),
        ]),
        // FIN_014: feed_start_date/feed_end_date eksik.
        fx("FIN_014", vec![("feed_info.txt", "feed_publisher_name,feed_publisher_url,feed_lang,feed_version\nPub,https://x.example,en,1.0\n")]),
        // FIN_015: birden fazla feed_info kaydı.
        fx("FIN_015", vec![("feed_info.txt", "feed_publisher_name,feed_publisher_url,feed_lang\nPub,https://x.example,en\nPub2,https://y.example,en\n")]),
        // FIN_016: feed_start_date gelecekte (k6).
        fx("FIN_016", vec![("feed_info.txt", "feed_publisher_name,feed_publisher_url,feed_lang,feed_start_date,feed_end_date\nPub,https://x.example,en,20270101,20271231\n")]),
        // FIN_017: feed_end_date > bugün+2yıl (k6).
        fx("FIN_017", vec![("feed_info.txt", "feed_publisher_name,feed_publisher_url,feed_lang,feed_start_date,feed_end_date\nPub,https://x.example,en,20260101,20300101\n")]),
        // FIN_018: feed_contact_email ve feed_contact_url ikisi de yok (k6).
        fx("FIN_018", vec![("feed_info.txt", "feed_publisher_name,feed_publisher_url,feed_lang,feed_start_date,feed_end_date\nPub,https://x.example,en,20260101,20261231\n")]),
        // FIN_019: feed 7 gün içinde sona eriyor (bugün+5).
        fx("FIN_019", vec![("feed_info.txt", "feed_publisher_name,feed_publisher_url,feed_lang,feed_start_date,feed_end_date\nPub,https://x.example,en,20260101,20260520\n")]),
        // FIN_020: feed geçerlilik penceresi < 7 gün.
        fx("FIN_020", vec![("feed_info.txt", "feed_publisher_name,feed_publisher_url,feed_lang,feed_start_date,feed_end_date\nPub,https://x.example,en,20260101,20260103\n")]),

        // ── CLD grubu (calendar_dates) ─────────────────────────────────────────
        // CLD_004: calendar yok, trip servisinin exception_type=1 kaydı yok (k4).
        fx_rm("CLD_004", vec![("calendar_dates.txt", "service_id,date,exception_type\nSVC1,20260601,2\n")], vec!["calendar.txt"]),
        // CLD_006: bir service_id için > 60 istisna günü (k2).
        fx("CLD_006", vec![("calendar_dates.txt", concat!(
            "service_id,date,exception_type\n",
            "SVC1,20260101,1\nSVC1,20260102,1\nSVC1,20260103,1\nSVC1,20260104,1\nSVC1,20260105,1\nSVC1,20260106,1\nSVC1,20260107,1\nSVC1,20260108,1\nSVC1,20260109,1\nSVC1,20260110,1\n",
            "SVC1,20260111,1\nSVC1,20260112,1\nSVC1,20260113,1\nSVC1,20260114,1\nSVC1,20260115,1\nSVC1,20260116,1\nSVC1,20260117,1\nSVC1,20260118,1\nSVC1,20260119,1\nSVC1,20260120,1\n",
            "SVC1,20260121,1\nSVC1,20260122,1\nSVC1,20260123,1\nSVC1,20260124,1\nSVC1,20260125,1\nSVC1,20260126,1\nSVC1,20260127,1\nSVC1,20260128,1\nSVC1,20260129,1\nSVC1,20260130,1\n",
            "SVC1,20260131,1\nSVC1,20260201,1\nSVC1,20260202,1\nSVC1,20260203,1\nSVC1,20260204,1\nSVC1,20260205,1\nSVC1,20260206,1\nSVC1,20260207,1\nSVC1,20260208,1\nSVC1,20260209,1\n",
            "SVC1,20260210,1\nSVC1,20260211,1\nSVC1,20260212,1\nSVC1,20260213,1\nSVC1,20260214,1\nSVC1,20260215,1\nSVC1,20260216,1\nSVC1,20260217,1\nSVC1,20260218,1\nSVC1,20260219,1\n",
            "SVC1,20260220,1\nSVC1,20260221,1\nSVC1,20260222,1\nSVC1,20260223,1\nSVC1,20260224,1\nSVC1,20260225,1\nSVC1,20260226,1\nSVC1,20260227,1\nSVC1,20260228,1\nSVC1,20260301,1\nSVC1,20260302,1\n"
        ))]),
        // CLD_007: aktif günlerin yarısından fazlası override (kısa servis + 4 removal).
        fx("CLD_007", vec![
            ("calendar.txt", "service_id,monday,tuesday,wednesday,thursday,friday,saturday,sunday,start_date,end_date\nSVC1,1,1,1,1,1,1,1,20260518,20260524\n"),
            ("calendar_dates.txt", "service_id,date,exception_type\nSVC1,20260518,2\nSVC1,20260519,2\nSVC1,20260520,2\nSVC1,20260521,2\n"),
        ]),

        // ── FAR grubu (fares v1) ───────────────────────────────────────────────
        // FAR_001: fare_id tekrarı (k3).
        fx("FAR_001", vec![("fare_attributes.txt", "fare_id,price,currency_type,payment_method\nF1,2.5,USD,0\nF1,2.5,USD,0\n")]),
        // FAR_008: fare_attribute agency_id agency.txt'te yok (k4).
        fx("FAR_008", vec![("fare_attributes.txt", "fare_id,price,currency_type,payment_method,agency_id\nF1,2.5,USD,0,NOPE\n")]),
        // FAR_009: fare_attribute için fare_rules kaydı yok (k4).
        fx("FAR_009", vec![("fare_attributes.txt", "fare_id,price,currency_type,payment_method\nF1,2.5,USD,0\n")]),
        // FAR_010: aynı (route,origin,destination,contains) için birden fazla fare_id (k4).
        fx("FAR_010", vec![("fare_rules.txt", "fare_id,route_id\nF1,R1\nF2,R1\n")]),

        // ── AGN / ATR (kalan) ──────────────────────────────────────────────────
        // AGN_010: agency_id tekrarı (k3).
        fx("AGN_010", vec![("agency.txt", "agency_id,agency_name,agency_url,agency_timezone\n1,A,https://a.example,UTC\n1,B,https://b.example,UTC\n")]),
        // AGN_013: feed_lang ile agency_lang uyuşmuyor (k2).
        fx("AGN_013", vec![
            ("feed_info.txt", "feed_publisher_name,feed_publisher_url,feed_lang\nPub,https://x.example,en\n"),
            ("agency.txt", "agency_id,agency_name,agency_url,agency_timezone,agency_lang\n1,Test,http://test.example,UTC,fr\n"),
        ]),
        // ATR_009: agency_id/route_id/trip_id'den birden fazlası dolu (k4).
        fx("ATR_009", vec![("attributions.txt", "attribution_id,organization_name,is_producer,agency_id,route_id\nA1,Org,1,1,R1\n")]),
        // ATR_010: attribution agency_id agency.txt'te yok (k4).
        fx("ATR_010", vec![("attributions.txt", "attribution_id,organization_name,is_producer,agency_id\nA1,Org,1,NOPE\n")]),

        // ── LVL grubu (levels.txt) ─────────────────────────────────────────────
        // LVL_001: level_id tekrarı (k3).
        fx("LVL_001", vec![("levels.txt", "level_id,level_index\nL1,0\nL1,1\n")]),
        // LVL_002: level_index sayısal değil (k2).
        fx("LVL_002", vec![("levels.txt", "level_id,level_index\nL1,abc\n")]),
        // LVL_003: level_name eksik (k2).
        fx("LVL_003", vec![("levels.txt", "level_id,level_index\nL1,0\n")]),
        // LVL_004: level hiçbir durak tarafından kullanılmıyor (k4).
        fx("LVL_004", vec![("levels.txt", "level_id,level_index,level_name\nL1,0,Ground\n")]),
        // LVL_005: level_name > 255 karakter (k2).
        fx("LVL_005", vec![("levels.txt", concat!("level_id,level_index,level_name\nL1,0,",
            "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa\n"))]),
        // LVL_006: asansör (pathway_mode=5) uç durağında level_id yok (k4).
        fx("LVL_006", vec![("pathways.txt", "pathway_id,from_stop_id,to_stop_id,pathway_mode,is_bidirectional\nP1,S1,S2,5,0\n")]),

        // ── FLG grubu (fare_leg_rules cross-ref k4) ────────────────────────────
        fx("FLG_001", vec![("fare_leg_rules.txt", "leg_group_id,fare_product_id\nLG1,NOPE\n")]),
        fx("FLG_002", vec![("fare_leg_rules.txt", "leg_group_id,network_id\nLG1,NOPE\n")]),
        fx("FLG_003", vec![("fare_leg_rules.txt", "leg_group_id,from_area_id\nLG1,NOPE\n")]),
        fx("FLG_004", vec![("fare_leg_rules.txt", "leg_group_id,to_area_id\nLG1,NOPE\n")]),
        fx("FLG_005", vec![("fare_leg_rules.txt", "leg_group_id,from_timeframe_group_id\nLG1,NOPE\n")]),
        fx("FLG_006", vec![("fare_leg_rules.txt", "leg_group_id,to_timeframe_group_id\nLG1,NOPE\n")]),

        // ── FPD grubu (fare_products) ──────────────────────────────────────────
        // FPD_001: bileşik PK tekrarı (k3).
        fx("FPD_001", vec![("fare_products.txt", "fare_product_id,amount,currency\nP1,2.5,USD\nP1,2.5,USD\n")]),
        // FPD_003: currency geçersiz (k2).
        fx("FPD_003", vec![("fare_products.txt", "fare_product_id,amount,currency\nP1,2.5,usd\n")]),
        // FPD_004: fare_media_id fare_media.txt'te yok (k4).
        fx("FPD_004", vec![("fare_products.txt", "fare_product_id,amount,currency,fare_media_id\nP1,2.5,USD,MNOPE\n")]),
        // FPD_005: rider_category_id rider_categories.txt'te yok (k4).
        fx("FPD_005", vec![("fare_products.txt", "fare_product_id,amount,currency,rider_category_id\nP1,2.5,USD,RCNOPE\n")]),
        // FPD_006: aynı fare_product_id için birden fazla varsayılan (rider boş) (k2).
        fx("FPD_006", vec![("fare_products.txt", "fare_product_id,amount,currency\nP1,2.5,USD\nP1,3.5,EUR\n")]),

        // ── FTR grubu (fare_transfer_rules) ────────────────────────────────────
        fx("FTR_002", vec![("fare_transfer_rules.txt", "from_leg_group_id,fare_transfer_type\nLGNOPE,0\n")]),
        fx("FTR_003", vec![("fare_transfer_rules.txt", "to_leg_group_id,fare_transfer_type\nLGNOPE,0\n")]),
        fx("FTR_004", vec![("fare_transfer_rules.txt", "fare_product_id,fare_transfer_type\nPNOPE,0\n")]),
        fx("FTR_009", vec![("fare_transfer_rules.txt", "from_leg_group_id,to_leg_group_id,fare_transfer_type\nLG1,LG1,0\n")]),
        fx("FTR_010", vec![("fare_transfer_rules.txt", "from_leg_group_id,to_leg_group_id,fare_transfer_type,transfer_count\nLG1,LG2,0,3\n")]),
        fx("FTR_011", vec![("fare_transfer_rules.txt", "from_leg_group_id,fare_transfer_type,duration_limit\nLG1,0,3600\n")]),

        // ── RCT / SAR / TFR / FMD / GGL ────────────────────────────────────────
        // RCT_001: rider_category_id tekrarı (k3).
        fx("RCT_001", vec![("rider_categories.txt", "rider_category_id,rider_category_name\nRC1,A\nRC1,B\n")]),
        // RCT_006: fare_product başına birden fazla varsayılan rider_category (k4).
        fx("RCT_006", vec![
            ("rider_categories.txt", "rider_category_id,rider_category_name,is_default_fare_category\nRC1,Adult,1\nRC2,Senior,1\n"),
            ("fare_products.txt", "fare_product_id,amount,currency,rider_category_id\nP1,2.5,USD,RC1\nP1,3.5,USD,RC2\n"),
        ]),
        // SAR_001: stop_areas area_id areas.txt'te yok (k4).
        fx("SAR_001", vec![("stop_areas.txt", "area_id,stop_id\nANOPE,S1\n")]),
        // SAR_002: stop_areas stop_id stops.txt'te yok (k4).
        fx("SAR_002", vec![
            ("areas.txt", "area_id,area_name\nA1,Area1\n"),
            ("stop_areas.txt", "area_id,stop_id\nA1,NOPE\n"),
        ]),
        // TFR_002: timeframes service_id calendar'da yok (k4).
        fx("TFR_002", vec![("timeframes.txt", "timeframe_group_id,start_time,end_time,service_id\nTG1,08:00:00,10:00:00,SVCX\n")]),
        // FMD_001: fare_media_id tekrarı (k3).
        fx("FMD_001", vec![("fare_media.txt", "fare_media_id,fare_media_type\nM1,2\nM1,2\n")]),
        // GGL_001: transfer_type=4/5 (in-seat) Google desteklemiyor (k2).
        fx("GGL_001", vec![("transfers.txt", "from_stop_id,to_stop_id,transfer_type\nS1,S2,4\n")]),
        // GGL_002: ic_price geçersiz (-5) (k2).
        fx("GGL_002", vec![("fare_products.txt", "fare_product_id,amount,currency,ic_price\nP1,2.5,USD,-5\n")]),

        // ── FRQ grubu (kalan) ──────────────────────────────────────────────────
        // FRQ_006: headway > 240dk (k6).
        fx("FRQ_006", vec![("frequencies.txt", "trip_id,start_time,end_time,headway_secs\nT1,08:00:00,10:00:00,20000\n")]),
        // FRQ_010: headway <= sıkışma eşiği (k6).
        fx("FRQ_010", vec![("frequencies.txt", "trip_id,start_time,end_time,headway_secs\nT1,08:00:00,10:00:00,60\n")]),
        // FRQ_011: aynı trip'in frequencies dönemleri çakışıyor (k2).
        fx("FRQ_011", vec![("frequencies.txt", "trip_id,start_time,end_time,headway_secs\nT1,08:00:00,10:00:00,600\nT1,09:00:00,11:00:00,600\n")]),

        // ── RTS grubu (kalan: k2 alan + k3 dup + k4 FK + k6) ───────────────────
        // RTS_001: route_id tekrarı (k3).
        fx("RTS_001", vec![("routes.txt", "route_id,agency_id,route_short_name,route_type\nR1,1,101,3\nR1,1,102,3\n")]),
        // RTS_002: agency_id agency.txt'te yok (k4 per-route).
        fx("RTS_002", vec![("routes.txt", "route_id,agency_id,route_short_name,route_type\nR1,NOPE,101,3\n")]),
        // RTS_007: route_text_color geçersiz hex (k2).
        fx("RTS_007", vec![("routes.txt", "route_id,agency_id,route_short_name,route_type,route_text_color\nR1,1,101,3,ZZZ\n")]),
        // RTS_008: route_text_color/route_color düşük kontrast (k2).
        fx("RTS_008", vec![("routes.txt", "route_id,agency_id,route_short_name,route_type,route_color,route_text_color\nR1,1,101,3,FFFFFF,FEFEFE\n")]),
        // RTS_010: route_short_name > 12 karakter (k2).
        fx("RTS_010", vec![("routes.txt", "route_id,agency_id,route_short_name,route_type\nR1,1,ABCDEFGHIJKLM,3\n")]),
        // RTS_011: route_long_name > 100 karakter (k2).
        fx("RTS_011", vec![("routes.txt", concat!("route_id,agency_id,route_short_name,route_long_name,route_type\nR1,1,101,",
            "Lorem ipsum dolor sit amet consectetur adipiscing elit sed do eiusmod tempor incididunt ut labore et dolore magna",
            ",3\n"))]),
        // RTS_012: hiçbir trip'te kullanılmayan rota (k4 orphan).
        fx("RTS_012", vec![("routes.txt", "route_id,agency_id,route_short_name,route_type\nR1,1,101,3\nR2,1,102,3\n")]),
        // RTS_016: hattın hiçbir seferinde aktif takvim günü yok (k6).
        fx("RTS_016", vec![("calendar.txt", "service_id,monday,tuesday,wednesday,thursday,friday,saturday,sunday,start_date,end_date\nSVC1,0,0,0,0,0,0,0,20250101,20271231\n")]),
        // RTS_019: yinelenen route_short_name (k2).
        fx("RTS_019", vec![("routes.txt", "route_id,agency_id,route_short_name,route_type\nR1,1,101,3\nR2,1,101,3\n")]),
        // RTS_020: route_url acente URL'siyle aynı (k6).
        fx("RTS_020", vec![("routes.txt", "route_id,agency_id,route_short_name,route_type,route_url\nR1,1,101,3,http://test.example\n")]),
        // RTS_021: route_short_name > 6 karakter (Google eşiği) (k2).
        fx("RTS_021", vec![("routes.txt", "route_id,agency_id,route_short_name,route_type\nR1,1,ABCDEFG,3\n")]),
        // RTS_022: route_long_name route_short_name'i içeriyor (k6).
        fx("RTS_022", vec![("routes.txt", "route_id,agency_id,route_short_name,route_long_name,route_type\nR1,1,5A,5A Line,3\n")]),
        // RTS_023: route_long_name == route_desc (k2).
        fx("RTS_023", vec![("routes.txt", "route_id,agency_id,route_short_name,route_long_name,route_desc,route_type\nR1,1,101,Main Line,Main Line,3\n")]),
        // RTS_024: cemv_support > 2 (k2).
        fx("RTS_024", vec![("routes.txt", "route_id,agency_id,route_short_name,route_type,cemv_support\nR1,1,101,3,5\n")]),
        // RTS_025: tek agency'de route agency_id boş (k6 best-practice).
        fx("RTS_025", vec![("routes.txt", "route_id,agency_id,route_short_name,route_type\nR1,,101,3\n")]),

        // ── TRP grubu (kalan: k2 + k4 FK + k6) ─────────────────────────────────
        // TRP_002: route_id routes.txt'te yok (k4).
        fx("TRP_002", vec![("trips.txt", "route_id,service_id,trip_id\nRNOPE,SVC1,T1\n")]),
        // TRP_003: service_id calendar'da yok (k4).
        fx("TRP_003", vec![("trips.txt", "route_id,service_id,trip_id\nR1,SVCX,T1\n")]),
        // TRP_004: shape_id shapes.txt'te yok (k4).
        fx("TRP_004", vec![("trips.txt", "route_id,service_id,trip_id,shape_id\nR1,SVC1,T1,SHNOPE\n")]),
        // TRP_011: trip_headsign + trip_short_name yok VE route da adsız (route-telafi guard #29).
        // (Base route_short_name=101 → guard eler; route adsız feed gerekiyor.)
        fx("TRP_011", vec![("routes.txt", "route_id,agency_id,route_type\nR1,1,3\n")]),
        // TRP_012: çift yönlü hatta bazı seferlerde direction_id yok (k6).
        fx("TRP_012", vec![("trips.txt", "route_id,service_id,trip_id,direction_id\nR1,SVC1,T1,0\nR1,SVC1,T2,1\nR1,SVC1,T3,\n")]),
        // TRP_013: route başına tek sefer — base tetikler (k6).
        fx("TRP_013", vec![]),
        // TRP_014: trip_short_name > 20 karakter (k2).
        fx("TRP_014", vec![("trips.txt", "route_id,service_id,trip_id,trip_short_name\nR1,SVC1,T1,ABCDEFGHIJKLMNOPQRSTU\n")]),
        // TRP_015: block_id'de tek sefer (k6).
        fx("TRP_015", vec![("trips.txt", "route_id,service_id,trip_id,block_id\nR1,SVC1,T1,B1\n")]),
        // TRP_017: frekans tabanlı sefer stop_times'ta yok (k4).
        fx("TRP_017", vec![
            ("trips.txt", "route_id,service_id,trip_id\nR1,SVC1,T1\nR1,SVC1,T2\n"),
            ("frequencies.txt", "trip_id,start_time,end_time,headway_secs\nT2,08:00:00,10:00:00,600\n"),
        ]),
        // TRP_019: continuous pickup/drop-off aktif ama shape_id yok (k4).
        fx("TRP_019", vec![("routes.txt", "route_id,agency_id,route_short_name,route_type,continuous_pickup\nR1,1,101,3,0\n")]),
        fx("STM_053", vec![("stop_times.txt", "trip_id,arrival_time,departure_time,stop_id,stop_sequence\nT1,08:00:00,08:00:00,S1,1\nT1,08:00:00,08:00:00,S2,2\nT1,08:00:00,08:00:00,S1,3\n")]),
        // TRP_020: trip_headsign terminal değil ara durak adıyla eşleşiyor (k6).
        fx("TRP_020", vec![
            ("stops.txt", "stop_id,stop_name,stop_lat,stop_lon\nS1,Stop1,41.0,29.0\nS2,Stop2,41.1,29.1\nS3,Stop3,41.2,29.2\n"),
            ("trips.txt", "route_id,service_id,trip_id,trip_headsign\nR1,SVC1,T1,Stop2\n"),
            ("stop_times.txt", "trip_id,arrival_time,departure_time,stop_id,stop_sequence\nT1,08:00:00,08:00:00,S1,1\nT1,08:10:00,08:10:00,S2,2\nT1,08:20:00,08:20:00,S3,3\n"),
        ]),
        // TRP_021: bikes_allowed hiç belirtilmemiş — base tetikler (k2).
        fx("TRP_021", vec![]),
        // TRP_022: block içinde çakışan sefer saatleri (k6).
        fx("TRP_022", vec![
            ("trips.txt", "route_id,service_id,trip_id,block_id\nR1,SVC1,T1,B1\nR1,SVC1,T2,B1\n"),
            ("stop_times.txt", "trip_id,arrival_time,departure_time,stop_id,stop_sequence\nT1,08:00:00,08:00:00,S1,1\nT1,09:00:00,09:00:00,S2,2\nT2,08:30:00,08:30:00,S1,1\nT2,09:30:00,09:30:00,S2,2\n"),
        ]),
        // TRP_023: önümüzdeki 7 günde aktif sefer yok (feed-level k6).
        fx("TRP_023", vec![("calendar.txt", "service_id,monday,tuesday,wednesday,thursday,friday,saturday,sunday,start_date,end_date\nSVC1,1,1,1,1,1,1,1,20270101,20270131\n")]),
        // TRP_024: block içinde tutarsız route_type (k6).
        fx("TRP_024", vec![
            ("routes.txt", "route_id,agency_id,route_short_name,route_type\nR1,1,101,3\nR2,1,102,0\n"),
            ("trips.txt", "route_id,service_id,trip_id,block_id\nR1,SVC1,T1,B1\nR2,SVC1,T2,B1\n"),
        ]),
        // TRP_025: wheelchair_accessible bilinmeyen oran > %80 — base tetikler (k6).
        fx("TRP_025", vec![]),
        // TRP_026: servisin hiç aktif tarihi yok (k6).
        fx("TRP_026", vec![("calendar.txt", "service_id,monday,tuesday,wednesday,thursday,friday,saturday,sunday,start_date,end_date\nSVC1,0,0,0,0,0,0,0,20250101,20271231\n")]),
        // TRP_033: aynı blokta otobüs (3) ve tramvay (0) — bir araç mod değiştiremez (k6).
        fx("TRP_033", vec![
            ("routes.txt", "route_id,agency_id,route_short_name,route_type\nR1,1,101,3\nR2,1,102,0\n"),
            ("trips.txt",  "route_id,service_id,trip_id,block_id\nR1,SVC1,T1,B1\nR2,SVC1,T2,B1\n"),
            ("stop_times.txt", "trip_id,arrival_time,departure_time,stop_id,stop_sequence\nT1,08:00:00,08:00:00,S1,1\nT1,08:10:00,08:10:00,S2,2\nT2,09:00:00,09:00:00,S1,1\nT2,09:10:00,09:10:00,S2,2\n"),
        ]),
        // TRP_028: bazı seferlerde wheelchair_accessible eksik (k6).
        fx("TRP_028", vec![("trips.txt", "route_id,service_id,trip_id,wheelchair_accessible\nR1,SVC1,T1,1\nR1,SVC1,T2,\n")]),
        // TRP_029: hiçbir seferde wheelchair_accessible yok — base tetikler (k6).
        fx("TRP_029", vec![]),

        // ── TRN grubu (kalan: k4 + k2) ─────────────────────────────────────────
        // TRN_004: record_id başvurulan tabloda yok (k4).
        fx("TRN_004", vec![("translations.txt", "table_name,field_name,language,translation,record_id\nstops,stop_name,fr,X,NOPE\n")]),
        // TRN_005: aynı anahtar + aynı çeviri birden çok satırda (k4).
        fx("TRN_005", vec![("translations.txt", "table_name,field_name,language,translation,record_id\nstops,stop_name,fr,X,S1\nstops,stop_name,fr,X,S1\n")]),
        // TRN_006: aynı anahtar + farklı çeviri (çelişki) (k4).
        fx("TRN_006", vec![("translations.txt", "table_name,field_name,language,translation,record_id\nstops,stop_name,fr,X,S1\nstops,stop_name,fr,Y,S1\n")]),
        // TRN_007: çeviri dili feed_lang ile aynı (>1 satır → feed-level) (k4).
        fx("TRN_007", vec![
            ("feed_info.txt", "feed_publisher_name,feed_publisher_url,feed_lang\nPub,https://x.example,en\n"),
            ("translations.txt", "table_name,field_name,language,translation,record_id\nstops,stop_name,en,X,S1\nstops,stop_name,en,Y,S2\n"),
        ]),
        // TRN_008: translation değeri boş (k2).
        fx("TRN_008", vec![("translations.txt", "table_name,field_name,language,translation,record_id\nstops,stop_name,fr,,S1\n")]),
        // TRN_010: stop_times çevirisi record_id var ama record_sub_id yok (k2).
        fx("TRN_010", vec![("translations.txt", "table_name,field_name,language,translation,record_id\nstop_times,stop_headsign,fr,X,T1\n")]),
        // TRN_013: feed_info çevirisi record_id/sub_id/field_value kullanıyor (k2).
        fx("TRN_013", vec![("translations.txt", "table_name,field_name,language,translation,record_id\nfeed_info,feed_publisher_name,fr,X,FI\n")]),
        // TRN_014: stop_times dışı tabloda record_sub_id (k2).
        fx("TRN_014", vec![("translations.txt", "table_name,field_name,language,translation,record_id,record_sub_id\nstops,stop_name,fr,X,S1,2\n")]),

        // ── STP grubu (kalan: k2 + k3 dup + k4 FK + k6) ────────────────────────
        // STP_001: stop_id tekrarı (k3).
        fx("STP_001", vec![("stops.txt", "stop_id,stop_name,stop_lat,stop_lon\nS1,A,41.0,29.0\nS1,B,41.1,29.1\n")]),
        // STP_009: parent_station stops.txt'te yok (k4).
        fx("STP_009", vec![("stops.txt", "stop_id,stop_name,stop_lat,stop_lon,parent_station\nS1,Stop1,41.0,29.0,\nS2,Stop2,41.1,29.1,NOPE\n")]),
        // STP_010: parent_station location_type=1 değil (k4).
        fx("STP_010", vec![("stops.txt", "stop_id,stop_name,stop_lat,stop_lon,location_type,parent_station\nS1,Stop1,41.0,29.0,0,\nS2,Stop2,41.1,29.1,0,S1\n")]),
        // STP_011: location_type 2/3/4 için parent_station yok (k4).
        fx("STP_011", vec![("stops.txt", "stop_id,stop_name,stop_lat,stop_lon,location_type\nS1,Stop1,41.0,29.0,2\nS2,Stop2,41.1,29.1,0\n")]),
        // STP_012: stop_times'ta kullanılan durak location_type != 0 (k4).
        fx("STP_012", vec![("stops.txt", "stop_id,stop_name,stop_lat,stop_lon,location_type\nS1,Stop1,41.0,29.0,1\nS2,Stop2,41.1,29.1,0\n")]),
        // STP_015: level_id levels.txt'te yok (k4).
        fx("STP_015", vec![("stops.txt", "stop_id,stop_name,stop_lat,stop_lon,level_id\nS1,Stop1,41.0,29.0,NOPE\nS2,Stop2,41.1,29.1,\n")]),
        // STP_017: iki durak çok yakın (< 5m, > 0) (k6).
        fx("STP_017", vec![("stops.txt", "stop_id,stop_name,stop_lat,stop_lon\nS1,Stop1,40.0,40.0\nS2,Stop2,40.00002,40.0\n")]),
        // STP_019: stop_name > 100 karakter (k2).
        fx("STP_019", vec![("stops.txt", concat!("stop_id,stop_name,stop_lat,stop_lon\nS1,",
            "Lorem ipsum dolor sit amet consectetur adipiscing elit sed do eiusmod tempor incididunt ut labore et dolore",
            ",41.0,29.0\nS2,Stop2,41.1,29.1\n"))]),
        // STP_020: stop_times'ta kullanılmayan fiziksel durak (k6).
        fx("STP_020", vec![("stops.txt", "stop_id,stop_name,stop_lat,stop_lon\nS1,Stop1,41.0,29.0\nS2,Stop2,41.1,29.1\nS3,Stop3,41.2,29.2\n")]),
        // STP_021: boarding area (loc=4) parent'ı platform (loc=0) değil — BA1 parent'ı istasyon (loc=1) (k4).
        fx("STP_021", vec![("stops.txt", "stop_id,stop_name,stop_lat,stop_lon,location_type,parent_station\nST1,Station,41.0,29.0,1,\nBA1,BoardArea,41.0,29.0,4,ST1\nS1,Stop1,41.0,29.0,0,\nS2,Stop2,41.1,29.1,0,\n")]),
        // STP_023: tts_stop_name '<'/'>' içeriyor (k2).
        fx("STP_023", vec![("stops.txt", "stop_id,stop_name,stop_lat,stop_lon,tts_stop_name\nS1,Stop1,41.0,29.0,<b>X\nS2,Stop2,41.1,29.1,\n")]),
        // STP_024: stop_access geçersiz enum (k2).
        fx("STP_024", vec![("stops.txt", "stop_id,stop_name,stop_lat,stop_lon,stop_access\nS1,Stop1,41.0,29.0,5\nS2,Stop2,41.1,29.1,\n")]),
        // STP_025: stop_name baştaki/sondaki boşluk (k2).
        fx("STP_025", vec![("stops.txt", "stop_id,stop_name,stop_lat,stop_lon\nS1, Stop1 ,41.0,29.0\nS2,Stop2,41.1,29.1\n")]),
        // STP_026: stop_access ham geçersiz enum (k4).
        fx("STP_026", vec![("stops.txt", "stop_id,stop_name,stop_lat,stop_lon,stop_access\nS1,Stop1,41.0,29.0,9\nS2,Stop2,41.1,29.1,\n")]),
        // STP_027: pathway tanımlı istasyonda platform stop_access belirsiz (k4).
        fx("STP_027", vec![
            ("stops.txt", "stop_id,stop_name,stop_lat,stop_lon,location_type,parent_station\nST1,Station,41.0,29.0,1,\nS1,Plat1,41.0,29.0,0,ST1\nS2,Plat2,41.01,29.01,0,ST1\n"),
            ("pathways.txt", "pathway_id,from_stop_id,to_stop_id,pathway_mode,is_bidirectional\nP1,S1,S2,3,0\n"),
        ]),
        // STP_028: stop_code > 50 karakter (k2).
        fx("STP_028", vec![("stops.txt", "stop_id,stop_code,stop_name,stop_lat,stop_lon\nS1,AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA,Stop1,41.0,29.0\nS2,C2,Stop2,41.1,29.1\n")]),
        // STP_029: durak parent_station'dan çok uzak (> 150m) (k6).
        fx("STP_029", vec![("stops.txt", "stop_id,stop_name,stop_lat,stop_lon,location_type,parent_station\nST1,Station,41.0,29.0,1,\nS1,Stop1,41.5,29.5,0,ST1\nS2,Stop2,41.1,29.1,0,\n")]),
        // STP_030: çocuğu olmayan üst istasyon (loc=1) (k6).
        fx("STP_030", vec![("stops.txt", "stop_id,stop_name,stop_lat,stop_lon,location_type\nS1,Stop1,41.0,29.0,0\nS2,Stop2,41.1,29.1,0\nST1,Station,41.2,29.2,1\n")]),
        // STP_031: stop_name == stop_desc (k2).
        fx("STP_031", vec![("stops.txt", "stop_id,stop_name,stop_lat,stop_lon,stop_desc\nS1,Stop1,41.0,29.0,Stop1\nS2,Stop2,41.1,29.1,\n")]),
        // STP_032: pathway bağlı platform için parent_station yok (k4).
        fx("STP_032", vec![("pathways.txt", "pathway_id,from_stop_id,to_stop_id,pathway_mode,is_bidirectional\nP1,S1,S2,3,0\n")]),
        // STP_034: stop_url acente URL'siyle aynı (k6).
        fx("STP_034", vec![("stops.txt", "stop_id,stop_name,stop_lat,stop_lon,stop_url\nS1,Stop1,41.0,29.0,http://test.example\nS2,Stop2,41.1,29.1,\n")]),
        // STP_035: stop_url hat URL'siyle aynı (k6).
        fx("STP_035", vec![
            ("routes.txt", "route_id,agency_id,route_short_name,route_type,route_url\nR1,1,101,3,http://r1.example\n"),
            ("stops.txt", "stop_id,stop_name,stop_lat,stop_lon,stop_url\nS1,Stop1,41.0,29.0,http://r1.example\nS2,Stop2,41.1,29.1,\n"),
        ]),
        // STP_036: istasyon (loc=1) parent_station içeriyor (k4).
        fx("STP_036", vec![("stops.txt", "stop_id,stop_name,stop_lat,stop_lon,location_type,parent_station\nST1,Station1,41.0,29.0,1,ST2\nST2,Station2,41.2,29.2,1,\nS1,Stop1,41.0,29.0,0,\nS2,Stop2,41.1,29.1,0,\n")]),
        // STP_037: bazı fiziksel duraklarda wheelchair_boarding eksik (k6).
        fx("STP_037", vec![("stops.txt", "stop_id,stop_name,stop_lat,stop_lon,wheelchair_boarding\nS1,Stop1,41.0,29.0,1\nS2,Stop2,41.1,29.1,\n")]),
        // STP_022: stop_code eksik — base tetikler (k2).
        fx("STP_022", vec![]),
        // STP_033: zone_id eksik — base tetikler (k2).
        fx("STP_033", vec![]),
        // STP_038: hiçbir durakta wheelchair_boarding yok — base tetikler (k6).
        fx("STP_038", vec![]),
        fx("STP_039", vec![("stops.txt", "stop_id,stop_name,stop_lat,stop_lon,stop_code\nS1,Stop1,41.0,29.0,100\nS2,Stop2,41.1,29.1,100\n")]),

        // ── SHP grubu (kalan: k5 geometri + k4 + k6). SHP_026 (>5000 nokta) inline
        //    yazılamaz → debt'te bırakıldı. ─────────────────────────────────────
        // SHP_006: tek noktalı shape (k5).
        fx("SHP_006", vec![("shapes.txt", "shape_id,shape_pt_lat,shape_pt_lon,shape_pt_sequence\nSH1,41.0,29.0,1\n")]),
        // SHP_009: kendisiyle kesişen shape — seg(2→3) ile seg(4→5) kesişir (k6).
        // (4 nokta yetmez: tek karşılaştırma (0,n-2) "bitişik uç" guard'ıyla atlanır.)
        fx("SHP_009", vec![("shapes.txt", "shape_id,shape_pt_lat,shape_pt_lon,shape_pt_sequence\nSH1,40.0,40.0,1\nSH1,40.0,40.1,2\nSH1,40.1,40.1,3\nSH1,40.05,40.15,4\nSH1,40.05,40.05,5\n")]),
        // SHP_010: ardışık özdeş koordinat (k5).
        fx("SHP_010", vec![("shapes.txt", "shape_id,shape_pt_lat,shape_pt_lon,shape_pt_sequence\nSH1,40.0,40.0,1\nSH1,40.0,40.0,2\nSH1,40.1,40.1,3\n")]),
        // SHP_012: shape duraklardan > eşik uzakta (k6).
        fx("SHP_012", vec![
            ("stops.txt", "stop_id,stop_name,stop_lat,stop_lon\nS1,Stop1,40.0,40.0\nS2,Stop2,40.05,40.05\n"),
            ("trips.txt", "route_id,service_id,trip_id,shape_id\nR1,SVC1,T1,SH1\n"),
            ("shapes.txt", "shape_id,shape_pt_lat,shape_pt_lon,shape_pt_sequence\nSH1,40.0,40.0,1\nSH1,40.0,40.05,2\nSH1,40.0,40.1,3\n"),
        ]),
        // SHP_014: shape başlangıç noktası ilk duraktan > 100m uzak (k6).
        fx("SHP_014", vec![
            ("stops.txt", "stop_id,stop_name,stop_lat,stop_lon\nS1,Stop1,41.0,41.0\nS2,Stop2,40.0,40.1\n"),
            ("trips.txt", "route_id,service_id,trip_id,shape_id\nR1,SVC1,T1,SH1\n"),
            ("shapes.txt", "shape_id,shape_pt_lat,shape_pt_lon,shape_pt_sequence\nSH1,40.0,40.0,1\nSH1,40.0,40.1,2\nSH1,40.0,40.2,3\n"),
        ]),
        // SHP_015: düşük nokta yoğunluğu (3 nokta / ~66km) (k6).
        fx("SHP_015", vec![("shapes.txt", "shape_id,shape_pt_lat,shape_pt_lon,shape_pt_sequence\nSH1,40.0,40.0,1\nSH1,40.0,40.3,2\nSH1,40.0,40.6,3\n")]),
        // SHP_016: ters yönde çizilmiş shape (k6).
        fx("SHP_016", vec![
            ("stops.txt", "stop_id,stop_name,stop_lat,stop_lon\nS1,Stop1,40.0,40.0\nS2,Stop2,40.0,40.5\n"),
            ("trips.txt", "route_id,service_id,trip_id,shape_id\nR1,SVC1,T1,SH1\n"),
            ("shapes.txt", "shape_id,shape_pt_lat,shape_pt_lon,shape_pt_sequence\nSH1,40.0,40.5,1\nSH1,40.0,40.25,2\nSH1,40.0,40.0,3\n"),
        ]),
        // SHP_017: durak sırası shape projeksiyonuyla çelişiyor (azalan sdt) (k6).
        fx("SHP_017", vec![
            ("stops.txt", "stop_id,stop_name,stop_lat,stop_lon\nS1,Stop1,40.0,40.0\nS2,Stop2,40.0,40.1\n"),
            ("trips.txt", "route_id,service_id,trip_id,shape_id\nR1,SVC1,T1,SH1\n"),
            ("shapes.txt", "shape_id,shape_pt_lat,shape_pt_lon,shape_pt_sequence\nSH1,40.0,40.0,1\nSH1,40.0,40.1,2\nSH1,40.0,40.2,3\n"),
            ("stop_times.txt", "trip_id,arrival_time,departure_time,stop_id,stop_sequence,shape_dist_traveled\nT1,08:00:00,08:00:00,S1,1,1000\nT1,08:10:00,08:10:00,S2,2,500\n"),
        ]),
        // SHP_018: hiçbir sefer tarafından referans edilmeyen shape (k5).
        fx("SHP_018", vec![("shapes.txt", "shape_id,shape_pt_lat,shape_pt_lon,shape_pt_sequence\nSH1,40.0,40.0,1\nSH1,40.001,40.001,2\nSH1,40.002,40.002,3\n")]),
        // SHP_019: shape referans alan tüm seferlerin stop_times'ı yok (k4).
        fx_rm("SHP_019",
            vec![
                ("trips.txt", "route_id,service_id,trip_id,shape_id\nR1,SVC1,T1,SH1\n"),
                ("shapes.txt", "shape_id,shape_pt_lat,shape_pt_lon,shape_pt_sequence\nSH1,40.0,40.0,1\nSH1,40.1,40.1,2\nSH1,40.2,40.2,3\n"),
                ("stop_times.txt", "trip_id,arrival_time,departure_time,stop_id,stop_sequence\n"),
            ],
            vec![]),
        // SHP_020: ardışık olmayan tekrarlayan nokta (k6).
        fx("SHP_020", vec![("shapes.txt", "shape_id,shape_pt_lat,shape_pt_lon,shape_pt_sequence\nSH1,40.0,40.0,1\nSH1,40.1,40.1,2\nSH1,40.0,40.0,3\n")]),
        // SHP_022: durak shape'in 2 ayrı bölümüne yakın (loop, sdt yok) (k6).
        // #52: kural artık geçerli stop_sequence'lı trip'i atlar (sıra belirsizliği çözer);
        // fixture DUPLICATE stop_sequence kullanır → sıra kullanılamaz → SHP_022 emit eder.
        fx("SHP_022", vec![
            ("stops.txt", "stop_id,stop_name,stop_lat,stop_lon\nS1,Stop1,40.0,40.0\nS2,Stop2,40.1,40.1\n"),
            ("trips.txt", "route_id,service_id,trip_id,shape_id\nR1,SVC1,T1,SH1\n"),
            ("shapes.txt", "shape_id,shape_pt_lat,shape_pt_lon,shape_pt_sequence\nSH1,40.0,40.0,1\nSH1,40.0,40.2,2\nSH1,40.2,40.2,3\nSH1,40.2,40.0,4\nSH1,40.0,40.0,5\n"),
            ("stop_times.txt", "trip_id,arrival_time,departure_time,stop_id,stop_sequence\nT1,08:00:00,08:00:00,S1,1\nT1,08:10:00,08:10:00,S2,1\n"),
        ]),
        // SHP_023: aynı dist_traveled + aynı koordinat (k6).
        fx("SHP_023", vec![("shapes.txt", "shape_id,shape_pt_lat,shape_pt_lon,shape_pt_sequence,shape_dist_traveled\nSH1,40.0,40.0,1,5\nSH1,40.0,40.0,2,5\nSH1,40.1,40.1,3,10\n")]),
        // SHP_024: durak shape_dist_traveled konumundan uzak (k6).
        fx("SHP_024", vec![
            ("stops.txt", "stop_id,stop_name,stop_lat,stop_lon\nS1,Stop1,41.0,41.0\nS2,Stop2,40.0,40.0\n"),
            ("trips.txt", "route_id,service_id,trip_id,shape_id\nR1,SVC1,T1,SH1\n"),
            ("shapes.txt", "shape_id,shape_pt_lat,shape_pt_lon,shape_pt_sequence,shape_dist_traveled\nSH1,40.0,40.0,1,0\nSH1,40.0,40.1,2,1000\n"),
            ("stop_times.txt", "trip_id,arrival_time,departure_time,stop_id,stop_sequence,shape_dist_traveled\nT1,08:00:00,08:00:00,S2,1,0\nT1,08:10:00,08:10:00,S1,2,500\n"),
        ]),
        // SHP_025: trip shape_dist_traveled şeklin maksimumunu aşıyor (k6).
        fx("SHP_025", vec![
            ("trips.txt", "route_id,service_id,trip_id,shape_id\nR1,SVC1,T1,SH1\n"),
            ("shapes.txt", "shape_id,shape_pt_lat,shape_pt_lon,shape_pt_sequence,shape_dist_traveled\nSH1,40.0,40.0,1,0\nSH1,40.0,40.1,2,100\n"),
            ("stop_times.txt", "trip_id,arrival_time,departure_time,stop_id,stop_sequence,shape_dist_traveled\nT1,08:00:00,08:00:00,S1,1,0\nT1,08:10:00,08:10:00,S2,2,200\n"),
        ]),
        // SHP_027: shape farklı durak desenlerine atanmış (k6).
        fx("SHP_027", vec![
            ("trips.txt", "route_id,service_id,trip_id,shape_id\nR1,SVC1,T1,SH1\nR1,SVC1,T2,SH1\n"),
            ("shapes.txt", "shape_id,shape_pt_lat,shape_pt_lon,shape_pt_sequence\nSH1,40.0,40.0,1\nSH1,40.1,40.1,2\n"),
            ("stop_times.txt", "trip_id,arrival_time,departure_time,stop_id,stop_sequence\nT1,08:00:00,08:00:00,S1,1\nT1,08:10:00,08:10:00,S2,2\nT2,09:00:00,09:00:00,S2,1\nT2,09:10:00,09:10:00,S1,2\n"),
        ]),
        // SHP_028: aynı dist farklı koordinat (eşik üstü ~0.1°) (k6).
        fx("SHP_028", vec![("shapes.txt", "shape_id,shape_pt_lat,shape_pt_lon,shape_pt_sequence,shape_dist_traveled\nSH1,40.0,40.0,1,5\nSH1,40.1,40.0,2,5\nSH1,40.2,40.2,3,10\n")]),
        // SHP_029: aynı dist farklı koordinat (eşik altı ~1e-6°) (k6).
        fx("SHP_029", vec![("shapes.txt", "shape_id,shape_pt_lat,shape_pt_lon,shape_pt_sequence,shape_dist_traveled\nSH1,40.0,40.0,1,5\nSH1,40.000001,40.0,2,5\nSH1,40.1,40.1,3,10\n")]),

        // ── STM grubu (kalan: k4 + k6 + k2). STM_043 (>200 durak) ve STM_044
        //    (>2M satır) inline yazılamaz → debt'te bırakıldı. ──────────────────
        // STM_001: stop_times trip_id trips.txt'te yok (k4).
        fx("STM_001", vec![("stop_times.txt", "trip_id,arrival_time,departure_time,stop_id,stop_sequence\nT1,08:00:00,08:00:00,S1,1\nT1,08:10:00,08:10:00,S2,2\nTGHOST,08:00:00,08:00:00,S1,1\n")]),
        // STM_002: stop_times stop_id stops.txt'te yok (k4).
        fx("STM_002", vec![("stop_times.txt", "trip_id,arrival_time,departure_time,stop_id,stop_sequence\nT1,08:00:00,08:00:00,S1,1\nT1,08:10:00,08:10:00,SNOPE,2\n")]),
        // STM_007: aynı durakta kalkış < varış (k6).
        fx("STM_007", vec![("stop_times.txt", "trip_id,arrival_time,departure_time,stop_id,stop_sequence\nT1,08:10:00,08:00:00,S1,1\nT1,08:20:00,08:20:00,S2,2\n")]),
        // STM_008: duraklar arası zaman geriye gidiyor (k6).
        fx("STM_008", vec![("stop_times.txt", "trip_id,arrival_time,departure_time,stop_id,stop_sequence\nT1,09:00:00,09:00:00,S1,1\nT1,08:00:00,08:00:00,S2,2\n")]),
        // STM_012: sıfır geçiş süresi ama mesafe >= 1km (base duraklar ~14km) (k6).
        fx("STM_012", vec![("stop_times.txt", "trip_id,arrival_time,departure_time,stop_id,stop_sequence\nT1,08:00:00,08:00:00,S1,1\nT1,08:00:00,08:00:00,S2,2\n")]),
        // STM_013: ara durakta zaman eksik (k6).
        fx("STM_013", vec![
            ("stops.txt", "stop_id,stop_name,stop_lat,stop_lon\nS1,Stop1,41.0,29.0\nS2,Stop2,41.05,29.05\nS3,Stop3,41.1,29.1\n"),
            ("stop_times.txt", "trip_id,arrival_time,departure_time,stop_id,stop_sequence\nT1,08:00:00,08:00:00,S1,1\nT1,,,S2,2\nT1,08:20:00,08:20:00,S3,3\n"),
        ]),
        // STM_014: hız eşiği aşımı (~208 km/h: bus eşik 120 < hız < 700 imkânsız sınırı) (k6).
        fx("STM_014", vec![("stop_times.txt", "trip_id,arrival_time,departure_time,stop_id,stop_sequence\nT1,08:00:00,08:00:00,S1,1\nT1,08:04:00,08:04:00,S2,2\n")]),
        // STM_015: ilk durakta departure_time yok (k6).
        fx("STM_015", vec![("stop_times.txt", "trip_id,arrival_time,departure_time,stop_id,stop_sequence\nT1,,,S1,1\nT1,08:10:00,08:10:00,S2,2\n")]),
        // STM_016: son durakta arrival_time yok (k6).
        fx("STM_016", vec![("stop_times.txt", "trip_id,arrival_time,departure_time,stop_id,stop_sequence\nT1,08:00:00,08:00:00,S1,1\nT1,,,S2,2\n")]),
        // STM_017: shape'i olan trip'te shape_dist_traveled eksik (k6).
        fx("STM_017", vec![
            ("trips.txt", "route_id,service_id,trip_id,shape_id\nR1,SVC1,T1,SH1\n"),
            ("shapes.txt", "shape_id,shape_pt_lat,shape_pt_lon,shape_pt_sequence\nSH1,41.0,29.0,1\nSH1,41.05,29.05,2\nSH1,41.1,29.1,3\n"),
        ]),
        // STM_020: sıfır geçiş süreli segment, mesafe 0.2-1km, saniye dolu (k6).
        fx("STM_020", vec![
            ("stops.txt", "stop_id,stop_name,stop_lat,stop_lon\nS1,Stop1,40.0,40.0\nS2,Stop2,40.005,40.0\n"),
            ("stop_times.txt", "trip_id,arrival_time,departure_time,stop_id,stop_sequence\nT1,08:00:30,08:00:30,S1,1\nT1,08:00:30,08:00:30,S2,2\n"),
        ]),
        // STM_021: farklı durak aynı koordinatta (k6).
        fx("STM_021", vec![
            ("stops.txt", "stop_id,stop_name,stop_lat,stop_lon\nS1,Stop1,40.0,40.0\nS2,Stop2,40.0,40.0\n"),
            ("stop_times.txt", "trip_id,arrival_time,departure_time,stop_id,stop_sequence\nT1,08:00:00,08:00:00,S1,1\nT1,08:10:00,08:10:00,S2,2\n"),
        ]),
        // STM_024: shape_dist_traveled birim uyumsuzluğu (oran 10×) (k4).
        fx("STM_024", vec![
            ("trips.txt", "route_id,service_id,trip_id,shape_id\nR1,SVC1,T1,SH1\n"),
            ("shapes.txt", "shape_id,shape_pt_lat,shape_pt_lon,shape_pt_sequence,shape_dist_traveled\nSH1,41.0,29.0,1,0\nSH1,41.1,29.1,2,100\n"),
            ("stop_times.txt", "trip_id,arrival_time,departure_time,stop_id,stop_sequence,shape_dist_traveled\nT1,08:00:00,08:00:00,S1,1,0\nT1,08:10:00,08:10:00,S2,2,1000\n"),
        ]),
        // STM_025: segment seyahat süresi < 10s (k6).
        fx("STM_025", vec![
            ("stops.txt", "stop_id,stop_name,stop_lat,stop_lon\nS1,Stop1,40.0,40.0\nS2,Stop2,40.001,40.0\n"),
            ("stop_times.txt", "trip_id,arrival_time,departure_time,stop_id,stop_sequence\nT1,08:00:00,08:00:00,S1,1\nT1,08:00:05,08:00:05,S2,2\n"),
        ]),
        // STM_026: durak arası mesafe > 50km (k6).
        fx("STM_026", vec![
            ("stops.txt", "stop_id,stop_name,stop_lat,stop_lon\nS1,Stop1,40.0,40.0\nS2,Stop2,41.0,40.0\n"),
            ("stop_times.txt", "trip_id,arrival_time,departure_time,stop_id,stop_sequence\nT1,08:00:00,08:00:00,S1,1\nT1,10:00:00,10:00:00,S2,2\n"),
        ]),
        // STM_027: shape_dist_traveled azalıyor (k6).
        fx("STM_027", vec![("stop_times.txt", "trip_id,arrival_time,departure_time,stop_id,stop_sequence,shape_dist_traveled\nT1,08:00:00,08:00:00,S1,1,1000\nT1,08:10:00,08:10:00,S2,2,500\n")]),
        // STM_028: trip süresi > 24 saat (k6).
        fx("STM_028", vec![("stop_times.txt", "trip_id,arrival_time,departure_time,stop_id,stop_sequence\nT1,08:00:00,08:00:00,S1,1\nT1,33:00:00,33:00:00,S2,2\n")]),
        // STM_029: trip süresi < 60s (k6).
        fx("STM_029", vec![
            ("stops.txt", "stop_id,stop_name,stop_lat,stop_lon\nS1,Stop1,40.0,40.0\nS2,Stop2,40.0005,40.0\n"),
            ("stop_times.txt", "trip_id,arrival_time,departure_time,stop_id,stop_sequence\nT1,08:00:00,08:00:00,S1,1\nT1,08:00:30,08:00:30,S2,2\n"),
        ]),
        // STM_032: (trip_id, stop_sequence) tekrarı (k2).
        fx("STM_032", vec![("stop_times.txt", "trip_id,arrival_time,departure_time,stop_id,stop_sequence\nT1,08:00:00,08:00:00,S1,1\nT1,08:10:00,08:10:00,S2,1\n")]),
        // STM_033: tek duraklı sefer (k4).
        fx("STM_033", vec![("stop_times.txt", "trip_id,arrival_time,departure_time,stop_id,stop_sequence\nT1,08:00:00,08:00:00,S1,1\n")]),
        // STM_035: aynı durak ardışık iki kez (k6).
        fx("STM_035", vec![("stop_times.txt", "trip_id,arrival_time,departure_time,stop_id,stop_sequence\nT1,08:00:00,08:00:00,S1,1\nT1,08:10:00,08:10:00,S1,2\n")]),
        // STM_036: stop_sequence azalan sırada (k6, k2 tespit).
        fx("STM_036", vec![("stop_times.txt", "trip_id,arrival_time,departure_time,stop_id,stop_sequence\nT1,08:00:00,08:00:00,S1,2\nT1,08:10:00,08:10:00,S2,1\n")]),
        // STM_042: stop_headsign yasaklı karakter içeriyor (k2).
        fx("STM_042", vec![("stop_times.txt", "trip_id,arrival_time,departure_time,stop_id,stop_sequence,stop_headsign\nT1,08:00:00,08:00:00,S1,1,Bad!Sign\nT1,08:10:00,08:10:00,S2,2,\n")]),
        // STM_045: kalkış saati servis-günü penceresini aşıyor (>27h) (k6).
        fx("STM_045", vec![("stop_times.txt", "trip_id,arrival_time,departure_time,stop_id,stop_sequence\nT1,08:00:00,08:00:00,S1,1\nT1,28:00:00,28:00:00,S2,2\n")]),
        // STM_048: gece yarısı sonrası 00:xx yazılmış (sarma) (k6).
        fx("STM_048", vec![("stop_times.txt", "trip_id,arrival_time,departure_time,stop_id,stop_sequence\nT1,23:50:00,23:50:00,S1,1\nT1,00:10:00,00:10:00,S2,2\n")]),
        // STM_049: gece yarısı sonrası 00:xx kalkış aynı satırda (k6).
        fx("STM_049", vec![("stop_times.txt", "trip_id,arrival_time,departure_time,stop_id,stop_sequence\nT1,23:50:00,00:10:00,S1,1\nT1,01:00:00,01:00:00,S2,2\n")]),
        // STM_050: timepoint sütunu var ama değer boş (k2).
        fx("STM_050", vec![("stop_times.txt", "trip_id,arrival_time,departure_time,stop_id,stop_sequence,timepoint\nT1,08:00:00,08:00:00,S1,1,\nT1,08:10:00,08:10:00,S2,2,\n")]),

        // ── ARC grubu (k1_parse arşiv/dosya/başlık). ARC_002/003 (geçersiz UTF-8,
        //    string fixture hep geçerli UTF-8) ve ARC_022 (>1M satır) inline yazılamaz,
        //    ARC_004 fatal → allowlist. Hepsi debt'te bırakıldı. ──────────────────
        // ARC_006: isteğe bağlı dosya mevcut (feed_info.txt).
        fx("ARC_006", vec![("feed_info.txt", "feed_publisher_name,feed_publisher_url,feed_lang\nPub,https://x.example,en\n")]),
        // ARC_007: GTFS dışı bilinmeyen dosya.
        fx("ARC_007", vec![("extra.txt", "col1\nval1\n")]),
        // ARC_010: UTF-8 BOM (EF BB BF) ile başlayan dosya.
        fx("ARC_010", vec![("stops.txt", "\u{feff}stop_id,stop_name,stop_lat,stop_lon\nS1,Stop1,41.0,29.0\nS2,Stop2,41.1,29.1\n")]),
        // ARC_011: dosya boyutu (Bilgi) — base zaten tetikler.
        fx("ARC_011", vec![]),
        // ARC_013: CSV tokenization hatası (kapanmamış tırnak, opsiyonel dosya).
        fx("ARC_013", vec![("feed_info.txt", "feed_publisher_name,feed_publisher_url,feed_lang\n\"unclosed,https://x.example,en\n")]),
        // ARC_014: başlıkta baştaki/sondaki boşluk.
        fx("ARC_014", vec![("stops.txt", "stop_id, stop_name,stop_lat,stop_lon\nS1,Stop1,41.0,29.0\nS2,Stop2,41.1,29.1\n")]),
        // ARC_017: GTFS dışı bilinmeyen sütun.
        fx("ARC_017", vec![("stops.txt", "stop_id,stop_name,stop_lat,stop_lon,bogus_col\nS1,Stop1,41.0,29.0,x\nS2,Stop2,41.1,29.1,y\n")]),
        // ARC_018: tamamen boş veri satırı.
        fx("ARC_018", vec![("stops.txt", "stop_id,stop_name,stop_lat,stop_lon\nS1,Stop1,41.0,29.0\n,,,\nS2,Stop2,41.1,29.1\n")]),
        // ARC_020: önerilen dosya eksik (shapes/feed_info) — base zaten tetikler.
        fx("ARC_020", vec![]),
        // ARC_021: yazdırılamaz/kontrol karakteri (U+0001) içeren değer.
        fx("ARC_021", vec![("stops.txt", "stop_id,stop_name,stop_lat,stop_lon\nS1,A\u{1}B,41.0,29.0\nS2,Stop2,41.1,29.1\n")]),
        // ARC_023: ZIP içinde iç içe ZIP dosyası.
        fx("ARC_023", vec![("inner.zip", "dummy\n")]),

        // ── TRF grubu (kalan: transfers k4 FK + k6) ────────────────────────────
        // TRF_003: from/to_stop_id stops.txt'te yok (k4).
        fx("TRF_003", vec![("transfers.txt", "from_stop_id,to_stop_id,transfer_type\nNOPE,S2,1\n")]),
        // TRF_006: from_trip_id trips.txt'te yok (k4).
        fx("TRF_006", vec![("transfers.txt", "from_stop_id,to_stop_id,transfer_type,from_trip_id\nS1,S2,1,TNOPE\n")]),
        // TRF_007: to_trip_id trips.txt'te yok (k4).
        fx("TRF_007", vec![("transfers.txt", "from_stop_id,to_stop_id,transfer_type,to_trip_id\nS1,S2,1,TNOPE\n")]),
        // TRF_008: from_route_id routes.txt'te yok (k4).
        fx("TRF_008", vec![("transfers.txt", "from_stop_id,to_stop_id,transfer_type,from_route_id\nS1,S2,1,RNOPE\n")]),
        // TRF_009: to_route_id routes.txt'te yok (k4).
        fx("TRF_009", vec![("transfers.txt", "from_stop_id,to_stop_id,transfer_type,to_route_id\nS1,S2,1,RNOPE\n")]),
        // TRF_010: min_transfer_time > 3600s (k4).
        fx("TRF_010", vec![("transfers.txt", "from_stop_id,to_stop_id,transfer_type,min_transfer_time\nS1,S2,2,5000\n")]),
        // TRF_011: aktarma mesafesi > 2000m (base S1↔S2 ~14km) (k6).
        fx("TRF_011", vec![("transfers.txt", "from_stop_id,to_stop_id,transfer_type,min_transfer_time\nS1,S2,2,300\n")]),
        // TRF_012: yinelenen from/to stop çifti (trip/route bağlamsız) (k4).
        fx("TRF_012", vec![("transfers.txt", "from_stop_id,to_stop_id,transfer_type\nS1,S2,1\nS1,S2,1\n")]),
        // TRF_013: type=4/5 için from/to_trip_id zorunlu (k4).
        fx("TRF_013", vec![("transfers.txt", "from_stop_id,to_stop_id,transfer_type\nS1,S2,4\n")]),
        // TRF_014: in-seat aktarma seferinin stop_times kaydı yok (k4).
        fx("TRF_014", vec![
            ("trips.txt", "route_id,service_id,trip_id\nR1,SVC1,T1\nR1,SVC1,T2\n"),
            ("transfers.txt", "from_stop_id,to_stop_id,transfer_type,from_trip_id,to_trip_id\nS1,S2,4,T2,T1\n"),
        ]),
        // TRF_015: type=4/5 için from/to_stop istasyon (loc=1) olamaz (k4).
        fx("TRF_015", vec![
            ("stops.txt", "stop_id,stop_name,stop_lat,stop_lon,location_type\nS1,Stop1,41.0,29.0,1\nS2,Stop2,41.1,29.1,0\n"),
            ("trips.txt", "route_id,service_id,trip_id\nR1,SVC1,T1\nR1,SVC1,T2\n"),
            ("transfers.txt", "from_stop_id,to_stop_id,transfer_type,from_trip_id,to_trip_id\nS1,S2,4,T1,T2\n"),
        ]),
        // TRF_016: aynı (stop,trip,route) kombinasyonunda çakışan transfer (k4).
        fx("TRF_016", vec![("transfers.txt", "from_stop_id,to_stop_id,transfer_type,from_trip_id\nS1,S2,1,T1\nS1,S2,1,T1\n")]),
        // TRF_017: from_trip_id'nin gerçek route'u from_route_id ile uyuşmuyor (k4).
        fx("TRF_017", vec![("transfers.txt", "from_stop_id,to_stop_id,transfer_type,from_trip_id,from_route_id\nS1,S2,1,T1,RWRONG\n")]),
        // TRF_018: from_trip_id == to_trip_id (anlamsız aktarma) (k4).
        fx("TRF_018", vec![("transfers.txt", "from_stop_id,to_stop_id,transfer_type,from_trip_id,to_trip_id\nS1,S2,1,T1,T1\n")]),
        // TRF_019: in-seat aktarmada farklı route_type (k4).
        fx("TRF_019", vec![
            ("routes.txt", "route_id,agency_id,route_short_name,route_type\nR1,1,101,3\nR2,1,102,0\n"),
            ("trips.txt", "route_id,service_id,trip_id\nR1,SVC1,T1\nR2,SVC1,T2\n"),
            ("transfers.txt", "from_stop_id,to_stop_id,transfer_type,from_trip_id,to_trip_id\nS1,S2,4,T1,T2\n"),
        ]),

        // ── VAT grubu (varlık analitik, k6). VAT_006 (≥50 sefer) inline bloat →
        //    debt'te bırakıldı. ─────────────────────────────────────────────────
        // VAT_001: iki hat (aynı route_type) >= %85 durak paylaşıyor (route >=5 durak).
        fx("VAT_001", vec![
            ("stops.txt", "stop_id,stop_name,stop_lat,stop_lon\nS1,Stop1,41.0,29.0\nS2,Stop2,41.1,29.1\nS3,Stop3,41.2,29.2\nS4,Stop4,41.3,29.3\nS5,Stop5,41.4,29.4\n"),
            ("routes.txt", "route_id,agency_id,route_short_name,route_type\nR1,1,101,3\nR2,1,102,3\n"),
            ("trips.txt", "route_id,service_id,trip_id\nR1,SVC1,T1\nR2,SVC1,T2\n"),
            ("stop_times.txt", "trip_id,arrival_time,departure_time,stop_id,stop_sequence\nT1,08:00:00,08:00:00,S1,1\nT1,08:10:00,08:10:00,S2,2\nT1,08:20:00,08:20:00,S3,3\nT1,08:30:00,08:30:00,S4,4\nT1,08:40:00,08:40:00,S5,5\nT2,09:00:00,09:00:00,S1,1\nT2,09:10:00,09:10:00,S2,2\nT2,09:20:00,09:20:00,S3,3\nT2,09:30:00,09:30:00,S4,4\nT2,09:40:00,09:40:00,S5,5\n"),
        ]),
        // VAT_002: durakta >= 4 hat ama transfers tanımlı değil (aktarma merkezi).
        fx("VAT_002", vec![
            ("routes.txt", "route_id,agency_id,route_short_name,route_type\nR1,1,1,3\nR2,1,2,3\nR3,1,3,3\nR4,1,4,3\n"),
            ("trips.txt", "route_id,service_id,trip_id\nR1,SVC1,T1\nR2,SVC1,T2\nR3,SVC1,T3\nR4,SVC1,T4\n"),
            ("stop_times.txt", "trip_id,arrival_time,departure_time,stop_id,stop_sequence\nT1,08:00:00,08:00:00,S1,1\nT1,08:10:00,08:10:00,S2,2\nT2,08:30:00,08:30:00,S1,1\nT2,08:40:00,08:40:00,S2,2\nT3,09:00:00,09:00:00,S1,1\nT3,09:10:00,09:10:00,S2,2\nT4,09:30:00,09:30:00,S1,1\nT4,09:40:00,09:40:00,S2,2\n"),
        ]),
        // VAT_003: sefer süresi istatistiksel aykırı (aynı shape, 6 sefer, biri ~50dk).
        fx("VAT_003", vec![
            ("trips.txt", "route_id,service_id,trip_id,shape_id\nR1,SVC1,T1,SH1\nR1,SVC1,T2,SH1\nR1,SVC1,T3,SH1\nR1,SVC1,T4,SH1\nR1,SVC1,T5,SH1\nR1,SVC1,T6,SH1\n"),
            ("shapes.txt", "shape_id,shape_pt_lat,shape_pt_lon,shape_pt_sequence\nSH1,41.0,29.0,1\nSH1,41.1,29.1,2\n"),
            ("stop_times.txt", "trip_id,arrival_time,departure_time,stop_id,stop_sequence\nT1,08:00:00,08:00:00,S1,1\nT1,08:10:00,08:10:00,S2,2\nT2,08:05:00,08:05:00,S1,1\nT2,08:15:00,08:15:00,S2,2\nT3,08:10:00,08:10:00,S1,1\nT3,08:20:00,08:20:00,S2,2\nT4,08:15:00,08:15:00,S1,1\nT4,08:25:00,08:25:00,S2,2\nT5,08:20:00,08:20:00,S1,1\nT5,08:30:00,08:30:00,S2,2\nT6,08:25:00,08:25:00,S1,1\nT6,09:15:00,09:15:00,S2,2\n"),
        ]),
        // VAT_005: ana şebekeden kopuk izole durak kümesi (BFS bileşen).
        fx("VAT_005", vec![
            ("stops.txt", "stop_id,stop_name,stop_lat,stop_lon\nS1,Stop1,41.0,29.0\nS2,Stop2,41.01,29.01\nS3,Stop3,41.02,29.02\nS4,Stop4,41.03,29.03\nS5,Iso1,10.0,10.0\nS6,Iso2,10.01,10.01\n"),
            ("trips.txt", "route_id,service_id,trip_id\nR1,SVC1,T1\nR1,SVC1,T2\n"),
            ("stop_times.txt", "trip_id,arrival_time,departure_time,stop_id,stop_sequence\nT1,08:00:00,08:00:00,S1,1\nT1,08:10:00,08:10:00,S2,2\nT1,08:20:00,08:20:00,S3,3\nT1,08:30:00,08:30:00,S4,4\nT2,09:00:00,09:00:00,S5,1\nT2,09:10:00,09:10:00,S6,2\n"),
        ]),
        // VAT_007: terminus durağı >= 3 hat, transfer yok.
        fx("VAT_007", vec![
            ("stops.txt", "stop_id,stop_name,stop_lat,stop_lon\nS1,Stop1,41.0,29.0\nS2,Stop2,41.1,29.1\nS3,Stop3,41.2,29.2\nS4,Stop4,41.3,29.3\n"),
            ("routes.txt", "route_id,agency_id,route_short_name,route_type\nR1,1,1,3\nR2,1,2,3\nR3,1,3,3\n"),
            ("trips.txt", "route_id,service_id,trip_id\nR1,SVC1,T1\nR2,SVC1,T2\nR3,SVC1,T3\n"),
            ("stop_times.txt", "trip_id,arrival_time,departure_time,stop_id,stop_sequence\nT1,08:00:00,08:00:00,S1,1\nT1,08:10:00,08:10:00,S2,2\nT2,08:30:00,08:30:00,S3,1\nT2,08:40:00,08:40:00,S2,2\nT3,09:00:00,09:00:00,S4,1\nT3,09:10:00,09:10:00,S2,2\n"),
        ]),
        // VAT_008: aynı shape >%30 hatta VE >=3 hatta (5 hat, SH1 3'ünde) (k6).
        fx("VAT_008", vec![
            ("routes.txt", "route_id,agency_id,route_short_name,route_type\nR1,1,1,3\nR2,1,2,3\nR3,1,3,3\nR4,1,4,3\nR5,1,5,3\n"),
            ("trips.txt", "route_id,service_id,trip_id,shape_id\nR1,SVC1,T1,SH1\nR2,SVC1,T2,SH1\nR3,SVC1,T3,SH1\nR4,SVC1,T4,\nR5,SVC1,T5,\n"),
            ("shapes.txt", "shape_id,shape_pt_lat,shape_pt_lon,shape_pt_sequence\nSH1,41.0,29.0,1\nSH1,41.1,29.1,2\n"),
            ("stop_times.txt", "trip_id,arrival_time,departure_time,stop_id,stop_sequence\nT1,08:00:00,08:00:00,S1,1\nT1,08:10:00,08:10:00,S2,2\nT2,09:00:00,09:00:00,S1,1\nT2,09:10:00,09:10:00,S2,2\nT3,10:00:00,10:00:00,S1,1\nT3,10:10:00,10:10:00,S2,2\n"),
        ]),

        // ── FRL grubu (fare_rules cross-ref k4) ────────────────────────────────
        // FRL_001: fare_id fare_attributes.txt'te yok.
        fx("FRL_001", vec![("fare_rules.txt", "fare_id,route_id\nFNOPE,R1\n")]),
        // FRL_002: route_id routes.txt'te yok.
        fx("FRL_002", vec![
            ("fare_attributes.txt", "fare_id,price,currency_type,payment_method\nF1,2.5,USD,0\n"),
            ("fare_rules.txt", "fare_id,route_id\nF1,RNOPE\n"),
        ]),
        // FRL_003: origin_id geçerli bir zone değil.
        fx("FRL_003", vec![
            ("fare_attributes.txt", "fare_id,price,currency_type,payment_method\nF1,2.5,USD,0\n"),
            ("fare_rules.txt", "fare_id,origin_id\nF1,ZNOPE\n"),
        ]),
        // FRL_004: destination_id geçerli bir zone değil.
        fx("FRL_004", vec![
            ("fare_attributes.txt", "fare_id,price,currency_type,payment_method\nF1,2.5,USD,0\n"),
            ("fare_rules.txt", "fare_id,destination_id\nF1,ZNOPE\n"),
        ]),
        // FRL_005: contains_id geçerli bir zone değil.
        fx("FRL_005", vec![
            ("fare_attributes.txt", "fare_id,price,currency_type,payment_method\nF1,2.5,USD,0\n"),
            ("fare_rules.txt", "fare_id,contains_id\nF1,ZNOPE\n"),
        ]),
        // FRL_006: fare_attributes var ama fare_rules yok.
        fx("FRL_006", vec![("fare_attributes.txt", "fare_id,price,currency_type,payment_method\nF1,2.5,USD,0\n")]),
        // FRL_007: kuralda ayrıştırıcı kriter yok (route/zone/contains hepsi boş).
        fx("FRL_007", vec![
            ("fare_attributes.txt", "fare_id,price,currency_type,payment_method\nF1,2.5,USD,0\n"),
            ("fare_rules.txt", "fare_id\nF1\n"),
        ]),
        // FRL_008: route tabanlı ücret ama bazı hatlar kapsam dışı (R2).
        fx("FRL_008", vec![
            ("fare_attributes.txt", "fare_id,price,currency_type,payment_method\nF1,2.5,USD,0\n"),
            ("routes.txt", "route_id,agency_id,route_short_name,route_type\nR1,1,101,3\nR2,1,102,3\n"),
            ("fare_rules.txt", "fare_id,route_id\nF1,R1\n"),
        ]),

        // ── SHP_021 (k2) ───────────────────────────────────────────────────────
        // SHP_021: shape_dist_traveled negatif.
        fx("SHP_021", vec![("shapes.txt", "shape_id,shape_pt_lat,shape_pt_lon,shape_pt_sequence,shape_dist_traveled\nSH1,41.0,29.0,1,-5\nSH1,41.1,29.1,2,10\n")]),
    ]
}

#[test]
fn each_fixture_actually_emits_its_rule() {
    let mut failures = Vec::new();
    for f in fixtures() {
        let cfg = f.config.clone().unwrap_or_default();
        let emitted = emitted_rules(&with_opts(&f.overrides, &f.removes, &f.raw), &cfg);
        if !emitted.contains(f.rule) {
            failures.push(format!("  {} emit etmedi → {:?}", f.rule, emitted));
        }
    }
    assert!(failures.is_empty(), "Emit etmeyen fixture(lar):\n{}", failures.join("\n"));
}

/// Aynı fixture gövdesini (overrides+removes+raw+config) paylaşan farklı kurallar.
/// Bugünkü iki duplikasyon kaçışının ORTAK imzası tam buydu: STM_023/STM_036 ve
/// OPR_006/STM_033 için `emit_proof.rs`'te BİREBİR aynı fixture kayıtlıydı ve
/// `each_fixture_actually_emits_its_rule` (salt kapsama) ikisini de yeşil geçirdi.
///
/// Aynı gövde iki farklı kuralı tetikliyorsa ya (a) meşru — aynı senaryo doğal
/// olarak birden çok kuralı ilgilendiriyor (allowlist'te gerekçesiyle) ya da
/// (b) DUPLİKASYON — iki kural aynı olguyu raporluyor (SILINMELI/katmanlanmalı).
/// Boş gövde (`vec![]`, base feed'i olduğu gibi kullananlar) hariç tutulur —
/// base zaten ~15 kural üretir, hepsi aynı boş gövdeyi paylaşır (meşru).
#[test]
fn shared_fixture_bodies_match_ledger() {
    let fxs = fixtures();
    let mut by_body: std::collections::HashMap<String, Vec<&str>> =
        std::collections::HashMap::new();
    for f in &fxs {
        // Boş gövde = base feed doğal çıktısı (base zaten ~15 kural üretir, hepsi
        // aynı boş gövdeyi paylaşır) — atla.
        if f.overrides.is_empty() && f.removes.is_empty() && f.raw.is_empty() {
            continue;
        }
        let key = format!("{:?}|{:?}|{:?}", f.overrides, f.removes, f.raw);
        by_body.entry(key).or_default().push(f.rule);
    }

    let mut pairs: Vec<String> = Vec::new();
    for rules in by_body.values() {
        if rules.len() < 2 {
            continue;
        }
        let mut sorted = rules.clone();
        sorted.sort_unstable();
        for i in 0..sorted.len() {
            for j in (i + 1)..sorted.len() {
                pairs.push(format!("{} {}", sorted[i], sorted[j]));
            }
        }
    }
    pairs.sort_unstable();
    pairs.dedup();

    let ledger_path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests").join("shared_fixture_ledger.txt");
    let ledger_raw = std::fs::read_to_string(&ledger_path).unwrap_or_default();
    let ledger: Vec<String> = ledger_raw
        .lines()
        .map(str::trim)
        .filter(|l| !l.is_empty() && !l.starts_with('#'))
        .map(str::to_string)
        .collect();

    if pairs != ledger {
        if std::env::var("UPDATE_LEDGER").is_ok() {
            let header = "# Aynı fixture gövdesini paylaşan kural çiftleri (G2 duplikasyon-erken-uyarı).\n\
                          # Bugünkü iki kaçışın ortak imzası: STM_023/STM_036 ve OPR_006/STM_033 için\n\
                          # emit_proof'ta BİREBİR aynı fixture kayıtlıydı ve salt-kapsama testi yeşildi.\n\
                          # YENİ satır = ya meşru (aynı senaryo doğal olarak birden çok kuralı ilgilendirir)\n\
                          # ya DUPLİKASYON (aynı olgu iki kez). Şüpheliyse kaldır/katmanla; değilse işle.\n";
            std::fs::write(&ledger_path, format!("{header}{}\n", pairs.join("\n"))).unwrap();
            return;
        }
        let added: Vec<&String> = pairs.iter().filter(|p| !ledger.contains(p)).collect();
        let removed: Vec<&String> = ledger.iter().filter(|l| !pairs.contains(l)).collect();
        panic!(
            "shared_fixture_ledger güncel değil ({} hesaplanan, {} ledger).\n\
             YENİ aynı-gövde çifti (olası DUPLİKASYON — adjudicate et!): {:#?}\n\
             Ledger'da fazla (fixture değişti/kalktı): {:#?}\n\
             Düzeltmek için: UPDATE_LEDGER=1 cargo test -p gtfs-pipeline --test emit_proof shared_fixture_bodies_match_ledger",
            pairs.len(), ledger.len(), added, removed,
        );
    }
}

// FLG_002 regresyon: network_id networks.txt yerine routes.txt.network_id sütunuyla
// tanımlanmışsa (GTFS Fares v2, ikinci geçerli kaynak) FLG_002 yanlış pozitif ÜRETMEMELİ.
#[test]
fn flg_002_network_id_from_routes_txt_is_not_a_false_positive() {
    let routes = "route_id,agency_id,route_short_name,route_type,network_id\nR1,1,101,3,TRAM\n";
    let flr = "leg_group_id,network_id\nLG1,TRAM\n";
    let files = with_opts(
        &[("routes.txt", routes), ("fare_leg_rules.txt", flr)],
        &[],
        &[],
    );
    let emitted = emitted_rules(&files, &ValidatorConfig::default());
    assert!(
        !emitted.contains("FLG_002"),
        "routes.txt.network_id ile tanımlı TRAM için FLG_002 tetiklenmemeli, emit: {:?}",
        emitted,
    );
}

// FLG_002 regresyon: network_id route_networks.txt satırıyla tanımlanmışsa (GTFS Fares v2,
// üçüncü geçerli kaynak) FLG_002 yanlış pozitif ÜRETMEMELİ.
#[test]
fn flg_002_network_id_from_route_networks_txt_is_not_a_false_positive() {
    let route_networks = "network_id,route_id\nTRAM,R1\n";
    let flr = "leg_group_id,network_id\nLG1,TRAM\n";
    let files = with_opts(
        &[("route_networks.txt", route_networks), ("fare_leg_rules.txt", flr)],
        &[],
        &[],
    );
    let emitted = emitted_rules(&files, &ValidatorConfig::default());
    assert!(
        !emitted.contains("FLG_002"),
        "route_networks.txt ile tanımlı TRAM için FLG_002 tetiklenmemeli, emit: {:?}",
        emitted,
    );
}

// FLG_002 pozitif kontrol: network_id hiçbir kaynakta yoksa yine de tetiklenmeli.
#[test]
fn flg_002_still_fires_for_truly_undefined_network_id() {
    let flr = "leg_group_id,network_id\nLG1,NOPE\n";
    let files = with_opts(&[("fare_leg_rules.txt", flr)], &[], &[]);
    let emitted = emitted_rules(&files, &ValidatorConfig::default());
    assert!(emitted.contains("FLG_002"), "tanımsız network_id FLG_002 üretmeli, emit: {:?}", emitted);
}

#[test]
fn coverage_debt_matches_ledger() {
    let canonical: BTreeSet<&str> = RULES.iter().map(|r| r.id).collect();
    let proven: BTreeSet<&str> = fixtures().iter().map(|f| f.rule).collect();
    let allow: BTreeSet<&str> = PROOF_ALLOWLIST.iter().copied().collect();

    let mut debt: Vec<&str> = canonical
        .iter()
        .copied()
        .filter(|id| !proven.contains(id) && !allow.contains(id))
        .collect();
    debt.sort_unstable();

    let ledger_path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests").join("coverage_debt.txt");
    let ledger_raw = std::fs::read_to_string(&ledger_path).unwrap_or_default();
    let ledger: Vec<&str> = ledger_raw.lines().map(str::trim).filter(|l| !l.is_empty() && !l.starts_with('#')).collect();

    if debt != ledger {
        // Yeni ledger içeriğini yaz (UPDATE_LEDGER=1) ya da sadece farkı raporla.
        if std::env::var("UPDATE_LEDGER").is_ok() {
            let header = format!(
                "# emit-proof coverage debt (#5 C). Fixture'ı olmayan canonical kurallar.\n\
                 # Azaltmak için emit_proof.rs::fixtures()'a fixture ekleyin. {} kural.\n",
                debt.len());
            std::fs::write(&ledger_path, format!("{header}{}\n", debt.join("\n"))).unwrap();
            return;
        }
        let added: Vec<&&str> = debt.iter().filter(|d| !ledger.contains(d)).collect();
        let removed: Vec<&&str> = ledger.iter().filter(|l| !debt.contains(l)).collect();
        panic!(
            "coverage_debt ledger güncel değil ({} hesaplanan, {} ledger).\n\
             Ledger'da olmayan (fixture silindi/yeni kural?): {:?}\n\
             Ledger'da fazla (fixture eklendi → ledger küçülmeli): {:?}\n\
             Düzeltmek için: UPDATE_LEDGER=1 cargo test -p gtfs-pipeline --test emit_proof coverage_debt_matches_ledger",
            debt.len(), ledger.len(), added, removed,
        );
    }
}
