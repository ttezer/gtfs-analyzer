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
    make_zip_with_modes(files, &[])
}

/// `no_read`'teki dosyalar OKUMA İZNİ OLMADAN yazılır (0o200) — ARC_027 için.
/// Diğerleri varsayılan izinle gider.
fn make_zip_with_modes(files: &[(String, Vec<u8>)], no_read: &[&str]) -> Vec<u8> {
    let cur = std::io::Cursor::new(Vec::new());
    let mut zw = zip::ZipWriter::new(cur);
    for (name, data) in files {
        let opts = if no_read.contains(&name.as_str()) {
            SimpleFileOptions::default().unix_permissions(0o200)
        } else {
            SimpleFileOptions::default()
        };
        zw.start_file(name, opts).unwrap();
        zw.write_all(data).unwrap();
    }
    zw.finish().unwrap().into_inner()
}

/// ARC_027 fixture'ı: verilen dosyalar okuma izinsiz yazılır.
fn fx_noread(rule: &'static str, no_read: Vec<&'static str>) -> Fixture {
    Fixture { rule, overrides: Vec::new(), removes: Vec::new(), raw: Vec::new(),
              config: None, no_read }
}

fn emitted_rules(files: &[(String, Vec<u8>)], config: &ValidatorConfig) -> BTreeSet<String> {
    emitted_rules_modes(files, config, &[])
}

fn emitted_rules_modes(files: &[(String, Vec<u8>)], config: &ValidatorConfig, no_read: &[&str]) -> BTreeSet<String> {
    match validate_bytes(&make_zip_with_modes(files, no_read), config, TODAY) {
        ValidateResult::Ok(vr) => vr.notices.iter().map(|n| n.rule_id.clone()).collect(),
        ValidateResult::Fatal(e) => {
            // Fatal yol: rule_id'yi koddan türetmek yerine boş set; arşiv fatal'ları allowlist'te.
            let _ = e;
            BTreeSet::new()
        }
    }
}

fn notices_for(files: &[(String, Vec<u8>)], config: &ValidatorConfig) -> Vec<gtfs_core::Notice> {
    match validate_bytes(&make_zip(files), config, TODAY) {
        ValidateResult::Ok(vr) => vr.notices,
        ValidateResult::Fatal(e) => panic!("fixture validation unexpectedly fatal: {e:?}"),
    }
}

/// Bir fixture: beklenen rule_id + onu tetikleyen feed (base üzerine override'lar).
struct Fixture {
    rule: &'static str,
    overrides: Vec<(&'static str, &'static str)>,
    removes: Vec<&'static str>,
    raw: Vec<(&'static str, &'static [u8])>,
    config: Option<ValidatorConfig>,
    /// ARC_027 için: bu dosyalar zip'e OKUMA İZNİ OLMADAN yazılır (unix mode 0o200).
    /// Kural `zf.unix_mode() & 0o400 == 0` arıyor; içerikle değil zip ÜST VERİSİYLE
    /// tetikleniyor, bu yüzden normal fixture yolu onu üretemiyordu.
    no_read: Vec<&'static str>,
}

fn fx(rule: &'static str, overrides: Vec<(&'static str, &'static str)>) -> Fixture {
    Fixture { rule, overrides, removes: Vec::new(), raw: Vec::new(), config: None, no_read: Vec::new() }
}

/// Dosya çıkarmalı fixture ("X.txt eksik" senaryoları).
fn fx_rm(rule: &'static str, overrides: Vec<(&'static str, &'static str)>, removes: Vec<&'static str>) -> Fixture {
    Fixture { rule, overrides, removes, raw: Vec::new(), config: None, no_read: Vec::new() }
}

/// Ham byte fixture — geçersiz UTF-8 senaryoları (ARC_002/003).
fn fx_raw(rule: &'static str, raw: Vec<(&'static str, &'static [u8])>) -> Fixture {
    Fixture { rule, overrides: Vec::new(), removes: Vec::new(), raw, config: None, no_read: Vec::new() }
}

/// Config-override fixture — ör. calendar_override_rules (OPR_021/022/023).
fn fx_cfg(rule: &'static str, overrides: Vec<(&'static str, &'static str)>, config: ValidatorConfig) -> Fixture {
    Fixture { rule, overrides, removes: Vec::new(), raw: Vec::new(), config: Some(config), no_read: Vec::new() }
}

/// Notice olarak emit edilmeyen kurallar (fatal yol veya dinamik) — proof'tan muaf.
/// Notice olarak emit edilmeyen kurallar — bu harness'tan MUAF.
///
/// ⚠️ **MUAFİYET, KANIT YOKLUĞU DEMEK DEĞİLDİR.** Bu listeye giren her kural için kanıtın
/// NEREDE olduğu yazılmalıdır; yazılamıyorsa kural ya ölüdür ya da eksiktir. 2026-08-02'de
/// denetlendi: üç girişin ikisinin kanıtı vardı; artık ARC_004 için doğrudan fixture vardır.
///
/// Bu liste iki defterin de kör noktasıdır: buradaki kurallar hiç notice üretmediği için
/// kapsam defterinde, iddia defterinde ve field=None defterinde GÖRÜNMEZLER.
const PROOF_ALLOWLIST: &[&str] = &[
    // Fatal yol — kanıt: integration.rs::arc001_corrupt_zip_returns_fatal_zip_unreadable
    "ARC_001",
    // Fatal yol — kanıt: integration.rs::arc029_* (DEFAULT limitlerle iki read_fatal yolu +
    // ratio_floor FP guard'ı). Notice fixture'ı yapısal olarak yazılamaz.
    "ARC_029",
];

// ── DEBT DEFTERİ ARTIK BOŞ (coverage_debt.txt) ──
// #28 ÇÖZÜLEN: ARC_002/003 (fx_raw ham byte), OPR_021/022/023 + ARC_028/STP_040/STP_041 (fx_cfg
// config) fixtures()'ta runtime-kanıtlı.
//
// 2026-08-07 (issue #71): kalan YEDİ kaydın hepsi kapandı. Defterdeki gerekçeler
// ("inline yazılamaz", "pratik değil") ÇOĞUNLUKLA YANLIŞTI; gerçek engel üç kez eşiğin
// SABİT KODLU olmasıydı, bir kez de zip üst verisiydi:
//   OPR_024/VAT_006/STM_043 — eşik zaten config'teydi ya da satır sayısı zaten yazılabilirdi
//   SHP_026/STM_044/ARC_022 — eşik `ValidatorConfig`'e taşındı (varsayılan DEĞİŞMEDİ)
//   ARC_027                 — harness'a `unix_permissions` yazan kurucu eklendi (fx_noread)
// ⚠️ DERS: "yazılamaz" etiketi bir ÖLÇÜM DEĞİL, bir tahmindi. Her biri denendiğinde düştü.
// ARC_001/ARC_029 PROOF_ALLOWLIST'te (Fatal yol). NOT (2026-07-16): ARC_029 borç
// ledger'ındaydı ama borç DEĞİLDİ — ARC_001 gibi Fatal yol; allowlist'e taşındı, uçtan uca
// kanıtı integration.rs::arc029_*'ta.

// #28 Grup 1 yardımcıları ─────────────────────────────────────────────────────
// OPSİYONEL dosyada (feed_info.txt) geçersiz UTF-8 (0xFF/0xFE) → ARC_002 + ARC_003 (fatal değil).
const BAD_UTF8_FEED_INFO: &[u8] = b"feed_publisher_name,feed_publisher_url,feed_lang\n\xff\xfe,https://x.example,en\n";
// calendar_dates: SVC_B 0101+0102, SVC_O yalnız 0101 aktif. Kural penceresi 0101-0103'te:
// 0101 base+override → OPR_021, 0102 yalnız base → OPR_022, 0103 hiçbiri → OPR_023.
const OVERRIDE_CD: &str = "service_id,date,exception_type\nSVC_B,20260101,1\nSVC_B,20260102,1\nSVC_O,20260101,1\n";
fn override_config() -> ValidatorConfig {
    ValidatorConfig {
        calendar_override_rules: vec![CalendarOverrideRule {
            route_id: "R1".into(),
            base_service_ids: vec!["SVC_B".into()],
            override_service_ids: vec!["SVC_O".into()],
            start_date: 20260101,
            end_date: 20260103,
        }],
        ..ValidatorConfig::default()
    }
}
// ARC_028: source_url verilir ve .zip ile bitmezse tetiklenir (config-gated).
fn source_url_config() -> ValidatorConfig {
    ValidatorConfig {
        source_url: Some("https://example.org/gtfs".into()),
        ..ValidatorConfig::default()
    }
}
// STP_040/041: opt-in stop-adı profili. P1 "Main Stop" → generic 'stop' sözcüğü (STP_040) VE
// parent ST1 "Central Hub" adını içermiyor (STP_041). S1/S2 stop_times referansları için korunur.
fn stop_name_config() -> ValidatorConfig {
    ValidatorConfig {
        stop_name_best_practices: true,
        ..ValidatorConfig::default()
    }
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
        // AGN_011 İKİ dosyayı kapsar: routes.txt ve fare_attributes.txt (spec aynı hükmü
        // ikisi için de yazar). Fixture ikisini de tetikler ki her iki çapa da düşsün.
        fx("AGN_011", vec![
            ("agency.txt", "agency_id,agency_name,agency_url,agency_timezone\n1,A,https://a.example,UTC\n2,B,https://b.example,UTC\n"),
            ("routes.txt", "route_id,agency_id,route_short_name,route_type\nR1,,101,3\n"),
            ("fare_attributes.txt", "fare_id,price,currency_type,payment_method,transfers\nF1,1.00,USD,0,0\n"),
        ]),
        // ── ARC grubu (arşiv/dosya/başlık seviyesi) ────────────────────────────
        // ARC_008: takvim dosyası yok (calendar + calendar_dates ikisi de).
        fx_rm("ARC_008", vec![], vec!["calendar.txt"]),
        // ARC_004: eksik required file notice + PARTIAL olarak raporlanır.
        fx_rm("ARC_004", vec![], vec!["routes.txt"]),
        // ARC_031: translations.txt var, feed_info.txt yok → spec bu hâlde feed_info'yu
        // ZORUNLU kılar (koşul sağlanmazsa yalnız tavsiye eder → ARC_020, Quality).
        fx("ARC_031", vec![("translations.txt", "table_name,field_name,language,translation,record_id\nstops,stop_name,fr,Gare,S1\n")]),
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
        fx("BKR_012", vec![("booking_rules.txt", "booking_rule_id,booking_type,prior_notice_last_day,prior_notice_last_time,prior_notice_duration_min\nBR1,2,3,12:00:00,30\n")]),
        fx("BKR_013", vec![("booking_rules.txt", "booking_rule_id,booking_type,prior_notice_duration_min,prior_notice_last_time\nBR1,1,30,17:00:00\n")]),
        fx("BKR_014", vec![("booking_rules.txt", "booking_rule_id,booking_type,prior_notice_duration_min,prior_notice_service_id\nBR1,1,30,SVC1\n")]),
        // BKR_015: type=2'de alan meşru ama servis calendar'da yok (k4 cross-ref).
        fx("BKR_015", vec![("booking_rules.txt", "booking_rule_id,booking_type,prior_notice_last_day,prior_notice_last_time,prior_notice_service_id\nBR1,2,3,12:00:00,MISSING_SVC\n")]),
        // BKR_016: booking_type enum dışı. BKR_019: booking_rule_id yineleniyor.
        fx("BKR_016", vec![("booking_rules.txt", "booking_rule_id,booking_type\nBR1,7\n")]),
        fx("BKR_019", vec![("booking_rules.txt", "booking_rule_id,booking_type,prior_notice_duration_min\nBR1,1,30\nBR1,1,45\n")]),
        // BKR_020/021/022: üç iletişim alanı hiç okunmuyordu (#60).
        fx("BKR_020", vec![("booking_rules.txt", "booking_rule_id,booking_type,booking_url\nBR1,0,notaurl\n")]),
        fx("BKR_021", vec![("booking_rules.txt", "booking_rule_id,booking_type,info_url\nBR1,0,notaurl\n")]),
        fx("BKR_022", vec![("booking_rules.txt", "booking_rule_id,booking_type,phone_number\nBR1,0,abc\n")]),
        // BKR_023: prior_notice sayı alanı tam sayı değil (eskiden opt_int sessizce yutuyordu).
        // DÖRT satır: BKR_023 dört alanı da ölçer, dedup Entity (booking_rule_id) → her alan
        // için ayrı kayıt gerekir, yoksa yalnız biri çapalanır (CAL_002 dersi).
        fx("BKR_023", vec![("booking_rules.txt", concat!(
            "booking_rule_id,booking_type,prior_notice_duration_min,prior_notice_duration_max,prior_notice_last_day,prior_notice_start_day\n",
            "BR1,2,abc,,,\n",
            "BR2,2,,abc,,\n",
            "BR3,2,,,abc,\n",
            "BR4,2,,,,abc\n",
        ))]),
        fx("BKR_024", vec![("booking_rules.txt","booking_rule_id,booking_type,prior_notice_duration_min,prior_notice_duration_max,prior_notice_start_day,prior_notice_last_day\nB1,1,30,120,2,3\n")]),
        // BKR_017/018: stop_times'taki booking_rule_id booking_rules.txt'te yok (k4 cross-ref).
        fx("BKR_017", vec![
            ("booking_rules.txt", "booking_rule_id,booking_type,prior_notice_duration_min\nBR1,1,30\n"),
            ("stop_times.txt", "trip_id,arrival_time,departure_time,stop_id,stop_sequence,location_id,start_pickup_drop_off_window,end_pickup_drop_off_window,pickup_booking_rule_id\nT1,08:00:00,08:00:00,S1,1,,,,\nT1,,,,2,LOC1,09:00:00,10:00:00,MISSING_BR\n"),
        ]),
        fx("BKR_018", vec![
            ("booking_rules.txt", "booking_rule_id,booking_type,prior_notice_duration_min\nBR1,1,30\n"),
            ("stop_times.txt", "trip_id,arrival_time,departure_time,stop_id,stop_sequence,location_id,start_pickup_drop_off_window,end_pickup_drop_off_window,drop_off_booking_rule_id\nT1,08:00:00,08:00:00,S1,1,,,,\nT1,,,,2,LOC1,09:00:00,10:00:00,MISSING_BR\n"),
        ]),

        // ── ARS: areas.txt area_id tekrarı ─────────────────────────────────────
        fx("ARS_001", vec![("areas.txt", "area_id,area_name\nA1,Area1\nA1,Area2\n")]),

        // ── CAL grubu (calendar.txt) ───────────────────────────────────────────
        fx("CAL_001", vec![("calendar.txt", "service_id,monday,tuesday,wednesday,thursday,friday,saturday,sunday,start_date,end_date\nSVC1,1,1,1,1,1,0,0,20250101,20271231\nSVC1,1,1,1,1,1,0,0,20250101,20271231\n")]),
        // CAL_002 YEDİ günü de denetler; fixture eskiden yalnız `monday`'i bozuyordu, bu yüzden
        // kapsam ölçümü diğer altı sütunu "çapasız" sayıyordu — kural boşluğu DEĞİL, fixture
        // zayıflığıydı.
        //
        // ⚠️ Tek satırda yedi günü birden bozmak YETMEZ: CAL_002'nin dedup düzeyi Entity
        // (`service_id`), yani servis başına TEK notice kalır ve yalnız bir gün çapalanır.
        // Bu yüzden yedi AYRI servis, her biri farklı bir günü bozuyor.
        fx("CAL_002", vec![("calendar.txt", concat!(
            "service_id,monday,tuesday,wednesday,thursday,friday,saturday,sunday,start_date,end_date\n",
            "SVC1,5,1,1,1,1,0,0,20250101,20271231\n",
            "SVC2,1,5,1,1,1,0,0,20250101,20271231\n",
            "SVC3,1,1,5,1,1,0,0,20250101,20271231\n",
            "SVC4,1,1,1,5,1,0,0,20250101,20271231\n",
            "SVC5,1,1,1,1,5,0,0,20250101,20271231\n",
            "SVC6,1,1,1,1,1,5,0,20250101,20271231\n",
            "SVC7,1,1,1,1,1,0,5,20250101,20271231\n",
        ))]),
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
        fx("FAR_006", vec![("fare_attributes.txt", "fare_id,price,currency_type,payment_method,transfer_duration\nF1,2.5,USD,0,-1\n")]),
        fx("FAR_011", vec![("fare_attributes.txt", "fare_id,price,currency_type,payment_method\nF1,2.5,USD,\n")]),
        fx("FAR_012", vec![("fare_attributes.txt", "fare_id,price,currency_type,payment_method\n,2.5,USD,0\n")]),
        fx("FAR_013", vec![("fare_attributes.txt","fare_id,price,currency_type,payment_method,transfers\nF1,0.9,EUR,0,0\n")]),

        // ── FIN grubu (feed_info.txt; base'te yok) ─────────────────────────────
        fx("FIN_001", vec![("feed_info.txt", "feed_publisher_name,feed_publisher_url,feed_lang\n,https://x.example,en\n")]),
        fx("FIN_002", vec![("feed_info.txt", "feed_publisher_name,feed_publisher_url,feed_lang\nPub,notaurl,en\n")]),
        fx("FIN_003", vec![("feed_info.txt", "feed_publisher_name,feed_publisher_url,feed_lang\nPub,https://x.example,!!bad\n")]),
        fx("FIN_004", vec![("feed_info.txt", "feed_publisher_name,feed_publisher_url,feed_lang,default_lang\nPub,https://x.example,en,!!bad\n")]),
        fx("FIN_005", vec![("feed_info.txt", "feed_publisher_name,feed_publisher_url,feed_lang,feed_start_date\nPub,https://x.example,en,notadate\n")]),
        fx("FIN_006", vec![("feed_info.txt", "feed_publisher_name,feed_publisher_url,feed_lang,feed_end_date\nPub,https://x.example,en,notadate\n")]),

        // ── FLG: rule_priority negatif (fare_leg_rules.txt) ────────────────────
        fx("FLG_007", vec![("fare_leg_rules.txt", "leg_group_id,rule_priority\nLG1,-5\n")]),
        // FLJ_001..004: fare_leg_join_rules.txt (#59) — dosya 2026-08-02'ye kadar hiç
        // tanınmıyordu. Ağ alanları koşulsuz Required; durak alanları KARŞILIKLI koşullu.
        fx("FLJ_001", vec![
            ("networks.txt", "network_id,network_name\nN1,Net1\n"),
            ("fare_leg_join_rules.txt", "from_network_id,to_network_id\nNOPE,N1\n"),
        ]),
        fx("FLJ_002", vec![
            ("networks.txt", "network_id,network_name\nN1,Net1\n"),
            ("fare_leg_join_rules.txt", "from_network_id,to_network_id\nN1,NOPE\n"),
        ]),
        // FLJ_003 İKİ hükmü birden ölçer; iki satır ikisini de kanıtlar:
        //   satır 1 — from_stop_id boş ama to_stop_id dolu → karşılıklı KOŞUL ihlali
        //   satır 2 — from_stop_id dolu ama stops.txt'te yok → YABANCI ANAHTAR ihlali
        fx("FLJ_003", vec![
            ("networks.txt", "network_id,network_name\nN1,Net1\n"),
            ("fare_leg_join_rules.txt", concat!(
                "from_network_id,to_network_id,from_stop_id,to_stop_id\n",
                "N1,N1,,S1\n",
                "N1,N1,NOPE,S1\n",
            )),
        ]),
        // FLJ_004 aynı biçimde: satır 1 yabancı anahtar, satır 2 karşılıklı koşul.
        fx("FLJ_004", vec![
            ("networks.txt", "network_id,network_name\nN1,Net1\n"),
            ("fare_leg_join_rules.txt", concat!(
                "from_network_id,to_network_id,from_stop_id,to_stop_id\n",
                "N1,N1,S1,NOPE\n",
                "N1,N1,S1,\n",
            )),
        ]),

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
        // FPD_002 artık EKSİK/SAYISAL-OLMAYAN tutarı ölçer. Negatif tutar GEÇERLİDİR
        // (spec: "May be negative to represent transfer discounts") → fixture sayısal olmayan.
        fx("FPD_002", vec![("fare_products.txt", "fare_product_id,amount,currency\nP1,abc,USD\n")]),

        // ── LVL (levels.txt) ───────────────────────────────────────────────────
        fx("LVL_007", vec![("levels.txt", "level_id,level_index\nL1,\n")]),
        fx("LVL_008", vec![("levels.txt", "level_id,level_index\n,0\n")]),

        // ── RCT (rider_categories.txt) ─────────────────────────────────────────
        fx("RCT_002", vec![("rider_categories.txt", "rider_category_id,rider_category_name\nRC1,\n")]),
        fx("RCT_003", vec![("rider_categories.txt", "rider_category_id,rider_category_name,is_default_fare_category\nRC1,Adult,5\n")]),
        fx("RCT_004", vec![("rider_categories.txt", "rider_category_id,rider_category_name,min_age\nRC1,Adult,abc\n")]),
        fx("RCT_005", vec![("rider_categories.txt", "rider_category_id,rider_category_name,min_age,max_age\nRC1,Adult,65,18\n")]),
        // RCT_007: eligibility_url URL biçiminde değil.
        fx("RCT_007", vec![("rider_categories.txt", "rider_category_id,rider_category_name,eligibility_url\nRC1,Adult,notaurl\n")]),

        // ── TFR (timeframes.txt) ───────────────────────────────────────────────
        fx("TFR_001", vec![("timeframes.txt", "timeframe_group_id,start_time,end_time,service_id\n,08:00:00,10:00:00,SVC1\n")]),
        fx("TFR_003", vec![("timeframes.txt", "timeframe_group_id,start_time,end_time,service_id\nTG1,notatime,10:00:00,SVC1\n")]),
        fx("TFR_004", vec![("timeframes.txt", "timeframe_group_id,start_time,end_time,service_id\nTG1,10:00:00,08:00:00,SVC1\n")]),
        fx("TFR_005", vec![("timeframes.txt", "timeframe_group_id,start_time,end_time,service_id\nTG1,08:00:00,12:00:00,SVC1\nTG1,10:00:00,14:00:00,SVC1\n")]),
        fx("TFR_006", vec![("timeframes.txt", "timeframe_group_id,start_time,end_time,service_id\nTG1,25:00:00,26:00:00,SVC1\n")]),
        // İKİ yön: TFR_007 eksik olanı adlandırır, tek satır yalnız birini çapalar.
        fx("TFR_007", vec![("timeframes.txt", concat!(
            "timeframe_group_id,start_time,end_time,service_id\n",
            "TG1,08:00:00,,SVC1\n",
            "TG2,,17:00:00,SVC1\n",
        ))]),
        fx("ARC_030", vec![("agency.txt", "agency_id,agency_name,agency_url,agency_timezone\n1,\"Test\nTransit\",http://x.com,UTC\n")]),
        // ARC_032: `<BR>` biçimi vahşi doğadan (mdb-1924, 1176 durak adı).
        fx("ARC_032", vec![("stops.txt", "stop_id,stop_name,stop_lat,stop_lon\nS1,Ana<br>Durak,41.0,29.0\n")]),
        fx("ARS_002", vec![("areas.txt","area_id,area_name\n,Bolge\n")]),
        // ⚠️ YEDİ AYRI SERVİS ŞART. Tek satırda yedi günü birden boşaltmak YETMEZ: dedup
        // aynı satır için tek notice bırakır ve yalnız bir gün alanı çapalanır. Aynı tuzak
        // CAL_002 fixture'ında da yaşanmıştı (kapsam defteri 2026-08-03 triyajı).
        fx("CAL_025", vec![("calendar.txt","service_id,monday,tuesday,wednesday,thursday,friday,saturday,sunday,start_date,end_date\nSVC1,,1,1,1,1,1,1,20250101,20261231\nSVC2,1,,1,1,1,1,1,20250101,20261231\nSVC3,1,1,,1,1,1,1,20250101,20261231\nSVC4,1,1,1,,1,1,1,20250101,20261231\nSVC5,1,1,1,1,,1,1,20250101,20261231\nSVC6,1,1,1,1,1,,1,20250101,20261231\nSVC7,1,1,1,1,1,1,,20250101,20261231\n")]),
        fx("FMD_004", vec![("fare_media.txt","fare_media_id,fare_media_name,fare_media_type\n,Kart,2\n")]),
        fx("NET_004", vec![("networks.txt","network_id,network_name\n,Ag\n")]),
        fx("RCT_008", vec![("rider_categories.txt","rider_category_id,rider_category_name\n,Yetiskin\n")]),
        fx("XFL_032", vec![("location_groups.txt","location_group_id,location_group_name\n,Grup\n")]),
        fx("XFL_033", vec![("location_group_stops.txt","location_group_id,stop_id\n,S1\n")]),
        fx("XFL_034", vec![("location_group_stops.txt","location_group_id,stop_id\nLG1,\n"), ("location_groups.txt","location_group_id,location_group_name\nLG1,Grup\n")]),
        fx("SAR_003", vec![("stop_areas.txt","area_id,stop_id\n,S1\n")]),
        fx("SAR_004", vec![("stop_areas.txt","area_id,stop_id\nA1,\n"), ("areas.txt","area_id,area_name\nA1,Bolge\n")]),
        fx("TFR_008", vec![("timeframes.txt","timeframe_group_id,start_time,end_time,service_id\nTG1,08:00:00,10:00:00,\n")]),
        fx("TRP_035", vec![("trips.txt","route_id,service_id,trip_id\nR1,,T1\n")]),

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
        // RTS_030: genişletilmiş (HVT) route_type — çekirdek enum dışı, RTS_004 DEĞİL (#93).
        fx("RTS_030", vec![("routes.txt", "route_id,agency_id,route_short_name,route_type\nR1,1,101,1501\n")]),
        // RTS_029: route_sort_order negatif olmayan tam sayı değil (eskiden sessizdi).
        fx("RTS_029", vec![("routes.txt", "route_id,agency_id,route_short_name,route_type,route_sort_order\nR1,1,101,3,-5\n")]),
        fx("RTS_006", vec![("routes.txt", "route_id,agency_id,route_short_name,route_type,route_color\nR1,1,101,3,XYZ\n")]),
        fx("RTS_013", vec![("routes.txt", "route_id,agency_id,route_short_name,route_type,continuous_pickup\nR1,1,101,3,9\n")]),
        fx("RTS_018", vec![("routes.txt", "route_id,agency_id,route_short_name,route_type,continuous_drop_off\nR1,1,101,3,9\n")]),
        // RTS_026: aynı kısa ad, FARKLI uzun ad (→ RTS_019 değil, Bilgi).
        fx("RTS_026", vec![("routes.txt", "route_id,agency_id,route_short_name,route_long_name,route_type\nR1,1,100,Line North,3\nR2,1,100,Line South,3\n")]),
        // RTS_027: aynı uzun ad, FARKLI kısa ad (→ RTS_019 değil, Bilgi).
        fx("RTS_027", vec![("routes.txt", "route_id,agency_id,route_short_name,route_long_name,route_type\nR1,1,10,City Line,3\nR2,1,20,City Line,3\n")]),
        // RTS_028: rotanın bir seferinde Flex penceresi var → routes.continuous_pickup=0 yasak (K4).
        fx("RTS_028", vec![
            ("routes.txt", "route_id,agency_id,route_short_name,route_type,continuous_pickup\nR1,1,101,3,0\n"),
            ("stop_times.txt", "trip_id,arrival_time,departure_time,stop_id,stop_sequence,location_id,start_pickup_drop_off_window,end_pickup_drop_off_window,pickup_booking_rule_id\nT1,08:00:00,08:00:00,S1,1,,,,\nT1,,,,2,LOC1,09:00:00,10:00:00,BR1\n"),
        ]),

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
        // İKİ sefer: STM_039 eksik olan pencere alanını adlandırır; tek satır yalnız birini
        // çapalar. T1'de end_ eksik, T2'de start_ eksik.
        fx("STM_039", vec![("stop_times.txt", concat!(
            "trip_id,stop_sequence,location_id,start_pickup_drop_off_window,end_pickup_drop_off_window\n",
            "T1,1,LOC1,09:00:00,\n",
            "T2,1,LOC1,,17:00:00\n",
        ))]),
        // STM_058: pencere alanı Time olarak parse edilemiyor (eskiden sessizce yutuluyordu).
        // İKİ satır: STM_058 iki pencere alanını da ölçer ama dedup düzeyi Entity (trip_id),
        // yani tek satırda ikisini birden bozmak yalnız BİRİNİ çapalar (CAL_002 ile aynı ders).
        fx("STM_058", vec![("stop_times.txt", concat!(
            "trip_id,stop_id,stop_sequence,start_pickup_drop_off_window,end_pickup_drop_off_window\n",
            "T1,S1,1,9am,10:00:00\n",
            "T2,S1,1,09:00:00,10pm\n",
        ))]),
        fx("STM_040", vec![("stop_times.txt", "trip_id,stop_sequence,location_id,start_pickup_drop_off_window,end_pickup_drop_off_window\nT1,1,LOC1,09:00:00,10:00:00\n")]),
        // STM_060: aynı seferde İKİ FARKLI bölge — geometrileri kesişiyor, pencereleri
        // çakışıyor, ikisi de biniş sunuyor. ⚠️ AYNI bölgeye ait iki kayıt İHLAL DEĞİLDİR
        // (spec bölge-içi seyahat için onu zorunlu kılar) → fixture bilerek FARKLI id kullanır.
        // OPR_024: hat başına aşırı sefer. Eşik `config.max_trips_per_route` (varsayılan 500,
        // yapılandırma alt sınırı 50) → fixture eşiği 50'ye indirip 51 sefer yazar.
        // 2026-08-07 dış denetimi "7 fixture borcu HÂLÂ 7" dedi; borç defterindeki kayıtların
        // hepsi "inline yazılamaz" gerekçesiyle duruyordu ama BU kural config ile yazılabilirdi.
        // VAT_006: >=50 sefer VE tek hat toplamın >=%40'ı. 2026-08-07'de borç defterinden
        // çıkarıldı — "inline yazılamaz" etiketi YANLIŞTI, 50 satır yetiyor.
        fx("VAT_006", vec![
            ("routes.txt", "route_id,agency_id,route_short_name,route_type\nR1,1,101,3\nR2,1,102,3\n"),
            ("trips.txt", "route_id,service_id,trip_id\nR1,SVC1,T1\nR1,SVC1,T2\nR1,SVC1,T3\nR1,SVC1,T4\nR1,SVC1,T5\nR1,SVC1,T6\nR1,SVC1,T7\nR1,SVC1,T8\nR1,SVC1,T9\nR1,SVC1,T10\nR1,SVC1,T11\nR1,SVC1,T12\nR1,SVC1,T13\nR1,SVC1,T14\nR1,SVC1,T15\nR1,SVC1,T16\nR1,SVC1,T17\nR1,SVC1,T18\nR1,SVC1,T19\nR1,SVC1,T20\nR1,SVC1,T21\nR1,SVC1,T22\nR1,SVC1,T23\nR1,SVC1,T24\nR1,SVC1,T25\nR2,SVC1,T26\nR2,SVC1,T27\nR2,SVC1,T28\nR2,SVC1,T29\nR2,SVC1,T30\nR2,SVC1,T31\nR2,SVC1,T32\nR2,SVC1,T33\nR2,SVC1,T34\nR2,SVC1,T35\nR2,SVC1,T36\nR2,SVC1,T37\nR2,SVC1,T38\nR2,SVC1,T39\nR2,SVC1,T40\nR2,SVC1,T41\nR2,SVC1,T42\nR2,SVC1,T43\nR2,SVC1,T44\nR2,SVC1,T45\nR2,SVC1,T46\nR2,SVC1,T47\nR2,SVC1,T48\nR2,SVC1,T49\nR2,SVC1,T50\n"),
            ("stop_times.txt", "trip_id,arrival_time,departure_time,stop_id,stop_sequence\nT1,08:01:00,08:01:00,S1,1\nT1,09:01:00,09:01:00,S2,2\nT2,08:02:00,08:02:00,S1,1\nT2,09:02:00,09:02:00,S2,2\nT3,08:03:00,08:03:00,S1,1\nT3,09:03:00,09:03:00,S2,2\nT4,08:04:00,08:04:00,S1,1\nT4,09:04:00,09:04:00,S2,2\nT5,08:05:00,08:05:00,S1,1\nT5,09:05:00,09:05:00,S2,2\nT6,08:06:00,08:06:00,S1,1\nT6,09:06:00,09:06:00,S2,2\nT7,08:07:00,08:07:00,S1,1\nT7,09:07:00,09:07:00,S2,2\nT8,08:08:00,08:08:00,S1,1\nT8,09:08:00,09:08:00,S2,2\nT9,08:09:00,08:09:00,S1,1\nT9,09:09:00,09:09:00,S2,2\nT10,08:10:00,08:10:00,S1,1\nT10,09:10:00,09:10:00,S2,2\nT11,08:11:00,08:11:00,S1,1\nT11,09:11:00,09:11:00,S2,2\nT12,08:12:00,08:12:00,S1,1\nT12,09:12:00,09:12:00,S2,2\nT13,08:13:00,08:13:00,S1,1\nT13,09:13:00,09:13:00,S2,2\nT14,08:14:00,08:14:00,S1,1\nT14,09:14:00,09:14:00,S2,2\nT15,08:15:00,08:15:00,S1,1\nT15,09:15:00,09:15:00,S2,2\nT16,08:16:00,08:16:00,S1,1\nT16,09:16:00,09:16:00,S2,2\nT17,08:17:00,08:17:00,S1,1\nT17,09:17:00,09:17:00,S2,2\nT18,08:18:00,08:18:00,S1,1\nT18,09:18:00,09:18:00,S2,2\nT19,08:19:00,08:19:00,S1,1\nT19,09:19:00,09:19:00,S2,2\nT20,08:20:00,08:20:00,S1,1\nT20,09:20:00,09:20:00,S2,2\nT21,08:21:00,08:21:00,S1,1\nT21,09:21:00,09:21:00,S2,2\nT22,08:22:00,08:22:00,S1,1\nT22,09:22:00,09:22:00,S2,2\nT23,08:23:00,08:23:00,S1,1\nT23,09:23:00,09:23:00,S2,2\nT24,08:24:00,08:24:00,S1,1\nT24,09:24:00,09:24:00,S2,2\nT25,08:25:00,08:25:00,S1,1\nT25,09:25:00,09:25:00,S2,2\nT26,08:26:00,08:26:00,S1,1\nT26,09:26:00,09:26:00,S2,2\nT27,08:27:00,08:27:00,S1,1\nT27,09:27:00,09:27:00,S2,2\nT28,08:28:00,08:28:00,S1,1\nT28,09:28:00,09:28:00,S2,2\nT29,08:29:00,08:29:00,S1,1\nT29,09:29:00,09:29:00,S2,2\nT30,08:30:00,08:30:00,S1,1\nT30,09:30:00,09:30:00,S2,2\nT31,08:31:00,08:31:00,S1,1\nT31,09:31:00,09:31:00,S2,2\nT32,08:32:00,08:32:00,S1,1\nT32,09:32:00,09:32:00,S2,2\nT33,08:33:00,08:33:00,S1,1\nT33,09:33:00,09:33:00,S2,2\nT34,08:34:00,08:34:00,S1,1\nT34,09:34:00,09:34:00,S2,2\nT35,08:35:00,08:35:00,S1,1\nT35,09:35:00,09:35:00,S2,2\nT36,08:36:00,08:36:00,S1,1\nT36,09:36:00,09:36:00,S2,2\nT37,08:37:00,08:37:00,S1,1\nT37,09:37:00,09:37:00,S2,2\nT38,08:38:00,08:38:00,S1,1\nT38,09:38:00,09:38:00,S2,2\nT39,08:39:00,08:39:00,S1,1\nT39,09:39:00,09:39:00,S2,2\nT40,08:40:00,08:40:00,S1,1\nT40,09:40:00,09:40:00,S2,2\nT41,08:41:00,08:41:00,S1,1\nT41,09:41:00,09:41:00,S2,2\nT42,08:42:00,08:42:00,S1,1\nT42,09:42:00,09:42:00,S2,2\nT43,08:43:00,08:43:00,S1,1\nT43,09:43:00,09:43:00,S2,2\nT44,08:44:00,08:44:00,S1,1\nT44,09:44:00,09:44:00,S2,2\nT45,08:45:00,08:45:00,S1,1\nT45,09:45:00,09:45:00,S2,2\nT46,08:46:00,08:46:00,S1,1\nT46,09:46:00,09:46:00,S2,2\nT47,08:47:00,08:47:00,S1,1\nT47,09:47:00,09:47:00,S2,2\nT48,08:48:00,08:48:00,S1,1\nT48,09:48:00,09:48:00,S2,2\nT49,08:49:00,08:49:00,S1,1\nT49,09:49:00,09:49:00,S2,2\nT50,08:50:00,08:50:00,S1,1\nT50,09:50:00,09:50:00,S2,2\n"),
        ]),
        // STM_043: tek seferde 200'den fazla stop_time. Aynı şekilde yazılabilirmiş.
        fx("STM_043", vec![
            ("stop_times.txt", "trip_id,arrival_time,departure_time,stop_id,stop_sequence\nT1,06:01:00,06:01:00,S1,1\nT1,06:02:00,06:02:00,S2,2\nT1,06:03:00,06:03:00,S1,3\nT1,06:04:00,06:04:00,S2,4\nT1,06:05:00,06:05:00,S1,5\nT1,06:06:00,06:06:00,S2,6\nT1,06:07:00,06:07:00,S1,7\nT1,06:08:00,06:08:00,S2,8\nT1,06:09:00,06:09:00,S1,9\nT1,06:10:00,06:10:00,S2,10\nT1,06:11:00,06:11:00,S1,11\nT1,06:12:00,06:12:00,S2,12\nT1,06:13:00,06:13:00,S1,13\nT1,06:14:00,06:14:00,S2,14\nT1,06:15:00,06:15:00,S1,15\nT1,06:16:00,06:16:00,S2,16\nT1,06:17:00,06:17:00,S1,17\nT1,06:18:00,06:18:00,S2,18\nT1,06:19:00,06:19:00,S1,19\nT1,06:20:00,06:20:00,S2,20\nT1,06:21:00,06:21:00,S1,21\nT1,06:22:00,06:22:00,S2,22\nT1,06:23:00,06:23:00,S1,23\nT1,06:24:00,06:24:00,S2,24\nT1,06:25:00,06:25:00,S1,25\nT1,06:26:00,06:26:00,S2,26\nT1,06:27:00,06:27:00,S1,27\nT1,06:28:00,06:28:00,S2,28\nT1,06:29:00,06:29:00,S1,29\nT1,06:30:00,06:30:00,S2,30\nT1,06:31:00,06:31:00,S1,31\nT1,06:32:00,06:32:00,S2,32\nT1,06:33:00,06:33:00,S1,33\nT1,06:34:00,06:34:00,S2,34\nT1,06:35:00,06:35:00,S1,35\nT1,06:36:00,06:36:00,S2,36\nT1,06:37:00,06:37:00,S1,37\nT1,06:38:00,06:38:00,S2,38\nT1,06:39:00,06:39:00,S1,39\nT1,06:40:00,06:40:00,S2,40\nT1,06:41:00,06:41:00,S1,41\nT1,06:42:00,06:42:00,S2,42\nT1,06:43:00,06:43:00,S1,43\nT1,06:44:00,06:44:00,S2,44\nT1,06:45:00,06:45:00,S1,45\nT1,06:46:00,06:46:00,S2,46\nT1,06:47:00,06:47:00,S1,47\nT1,06:48:00,06:48:00,S2,48\nT1,06:49:00,06:49:00,S1,49\nT1,06:50:00,06:50:00,S2,50\nT1,06:51:00,06:51:00,S1,51\nT1,06:52:00,06:52:00,S2,52\nT1,06:53:00,06:53:00,S1,53\nT1,06:54:00,06:54:00,S2,54\nT1,06:55:00,06:55:00,S1,55\nT1,06:56:00,06:56:00,S2,56\nT1,06:57:00,06:57:00,S1,57\nT1,06:58:00,06:58:00,S2,58\nT1,06:59:00,06:59:00,S1,59\nT1,07:00:00,07:00:00,S2,60\nT1,07:01:00,07:01:00,S1,61\nT1,07:02:00,07:02:00,S2,62\nT1,07:03:00,07:03:00,S1,63\nT1,07:04:00,07:04:00,S2,64\nT1,07:05:00,07:05:00,S1,65\nT1,07:06:00,07:06:00,S2,66\nT1,07:07:00,07:07:00,S1,67\nT1,07:08:00,07:08:00,S2,68\nT1,07:09:00,07:09:00,S1,69\nT1,07:10:00,07:10:00,S2,70\nT1,07:11:00,07:11:00,S1,71\nT1,07:12:00,07:12:00,S2,72\nT1,07:13:00,07:13:00,S1,73\nT1,07:14:00,07:14:00,S2,74\nT1,07:15:00,07:15:00,S1,75\nT1,07:16:00,07:16:00,S2,76\nT1,07:17:00,07:17:00,S1,77\nT1,07:18:00,07:18:00,S2,78\nT1,07:19:00,07:19:00,S1,79\nT1,07:20:00,07:20:00,S2,80\nT1,07:21:00,07:21:00,S1,81\nT1,07:22:00,07:22:00,S2,82\nT1,07:23:00,07:23:00,S1,83\nT1,07:24:00,07:24:00,S2,84\nT1,07:25:00,07:25:00,S1,85\nT1,07:26:00,07:26:00,S2,86\nT1,07:27:00,07:27:00,S1,87\nT1,07:28:00,07:28:00,S2,88\nT1,07:29:00,07:29:00,S1,89\nT1,07:30:00,07:30:00,S2,90\nT1,07:31:00,07:31:00,S1,91\nT1,07:32:00,07:32:00,S2,92\nT1,07:33:00,07:33:00,S1,93\nT1,07:34:00,07:34:00,S2,94\nT1,07:35:00,07:35:00,S1,95\nT1,07:36:00,07:36:00,S2,96\nT1,07:37:00,07:37:00,S1,97\nT1,07:38:00,07:38:00,S2,98\nT1,07:39:00,07:39:00,S1,99\nT1,07:40:00,07:40:00,S2,100\nT1,07:41:00,07:41:00,S1,101\nT1,07:42:00,07:42:00,S2,102\nT1,07:43:00,07:43:00,S1,103\nT1,07:44:00,07:44:00,S2,104\nT1,07:45:00,07:45:00,S1,105\nT1,07:46:00,07:46:00,S2,106\nT1,07:47:00,07:47:00,S1,107\nT1,07:48:00,07:48:00,S2,108\nT1,07:49:00,07:49:00,S1,109\nT1,07:50:00,07:50:00,S2,110\nT1,07:51:00,07:51:00,S1,111\nT1,07:52:00,07:52:00,S2,112\nT1,07:53:00,07:53:00,S1,113\nT1,07:54:00,07:54:00,S2,114\nT1,07:55:00,07:55:00,S1,115\nT1,07:56:00,07:56:00,S2,116\nT1,07:57:00,07:57:00,S1,117\nT1,07:58:00,07:58:00,S2,118\nT1,07:59:00,07:59:00,S1,119\nT1,08:00:00,08:00:00,S2,120\nT1,08:01:00,08:01:00,S1,121\nT1,08:02:00,08:02:00,S2,122\nT1,08:03:00,08:03:00,S1,123\nT1,08:04:00,08:04:00,S2,124\nT1,08:05:00,08:05:00,S1,125\nT1,08:06:00,08:06:00,S2,126\nT1,08:07:00,08:07:00,S1,127\nT1,08:08:00,08:08:00,S2,128\nT1,08:09:00,08:09:00,S1,129\nT1,08:10:00,08:10:00,S2,130\nT1,08:11:00,08:11:00,S1,131\nT1,08:12:00,08:12:00,S2,132\nT1,08:13:00,08:13:00,S1,133\nT1,08:14:00,08:14:00,S2,134\nT1,08:15:00,08:15:00,S1,135\nT1,08:16:00,08:16:00,S2,136\nT1,08:17:00,08:17:00,S1,137\nT1,08:18:00,08:18:00,S2,138\nT1,08:19:00,08:19:00,S1,139\nT1,08:20:00,08:20:00,S2,140\nT1,08:21:00,08:21:00,S1,141\nT1,08:22:00,08:22:00,S2,142\nT1,08:23:00,08:23:00,S1,143\nT1,08:24:00,08:24:00,S2,144\nT1,08:25:00,08:25:00,S1,145\nT1,08:26:00,08:26:00,S2,146\nT1,08:27:00,08:27:00,S1,147\nT1,08:28:00,08:28:00,S2,148\nT1,08:29:00,08:29:00,S1,149\nT1,08:30:00,08:30:00,S2,150\nT1,08:31:00,08:31:00,S1,151\nT1,08:32:00,08:32:00,S2,152\nT1,08:33:00,08:33:00,S1,153\nT1,08:34:00,08:34:00,S2,154\nT1,08:35:00,08:35:00,S1,155\nT1,08:36:00,08:36:00,S2,156\nT1,08:37:00,08:37:00,S1,157\nT1,08:38:00,08:38:00,S2,158\nT1,08:39:00,08:39:00,S1,159\nT1,08:40:00,08:40:00,S2,160\nT1,08:41:00,08:41:00,S1,161\nT1,08:42:00,08:42:00,S2,162\nT1,08:43:00,08:43:00,S1,163\nT1,08:44:00,08:44:00,S2,164\nT1,08:45:00,08:45:00,S1,165\nT1,08:46:00,08:46:00,S2,166\nT1,08:47:00,08:47:00,S1,167\nT1,08:48:00,08:48:00,S2,168\nT1,08:49:00,08:49:00,S1,169\nT1,08:50:00,08:50:00,S2,170\nT1,08:51:00,08:51:00,S1,171\nT1,08:52:00,08:52:00,S2,172\nT1,08:53:00,08:53:00,S1,173\nT1,08:54:00,08:54:00,S2,174\nT1,08:55:00,08:55:00,S1,175\nT1,08:56:00,08:56:00,S2,176\nT1,08:57:00,08:57:00,S1,177\nT1,08:58:00,08:58:00,S2,178\nT1,08:59:00,08:59:00,S1,179\nT1,09:00:00,09:00:00,S2,180\nT1,09:01:00,09:01:00,S1,181\nT1,09:02:00,09:02:00,S2,182\nT1,09:03:00,09:03:00,S1,183\nT1,09:04:00,09:04:00,S2,184\nT1,09:05:00,09:05:00,S1,185\nT1,09:06:00,09:06:00,S2,186\nT1,09:07:00,09:07:00,S1,187\nT1,09:08:00,09:08:00,S2,188\nT1,09:09:00,09:09:00,S1,189\nT1,09:10:00,09:10:00,S2,190\nT1,09:11:00,09:11:00,S1,191\nT1,09:12:00,09:12:00,S2,192\nT1,09:13:00,09:13:00,S1,193\nT1,09:14:00,09:14:00,S2,194\nT1,09:15:00,09:15:00,S1,195\nT1,09:16:00,09:16:00,S2,196\nT1,09:17:00,09:17:00,S1,197\nT1,09:18:00,09:18:00,S2,198\nT1,09:19:00,09:19:00,S1,199\nT1,09:20:00,09:20:00,S2,200\nT1,09:21:00,09:21:00,S1,201\n"),
        ]),
        // SHP_026 / STM_044: eşikleri 2026-08-07'de `ValidatorConfig`'e taşındı (OPR_024
        // emsali) — tek sebebi fixture yazılabilmesiydi, varsayılan davranış değişmedi.
        // Borç defteri bunları "inline yazılamaz" diye tutuyordu; doğrusu "eşik sabit
        // kodluydu" idi ve o düzeltilebilir bir şeydi.
        fx_cfg("SHP_026", vec![
            ("shapes.txt", "shape_id,shape_pt_lat,shape_pt_lon,shape_pt_sequence\nSH1,41.001,29.001,1\nSH1,41.002,29.002,2\nSH1,41.003,29.003,3\nSH1,41.004,29.004,4\nSH1,41.005,29.005,5\nSH1,41.006,29.006,6\nSH1,41.007,29.007,7\nSH1,41.008,29.008,8\nSH1,41.009,29.009,9\nSH1,41.010,29.010,10\nSH1,41.011,29.011,11\nSH1,41.012,29.012,12\n"),
            ("trips.txt", "route_id,service_id,trip_id,shape_id\nR1,SVC1,T1,SH1\n"),
        ], ValidatorConfig { max_shape_points: 10, ..ValidatorConfig::default() }),
        fx_cfg("STM_044", vec![
            ("stop_times.txt", "trip_id,arrival_time,departure_time,stop_id,stop_sequence\nT1,06:01:00,06:01:00,S1,1\nT1,06:02:00,06:02:00,S2,2\nT1,06:03:00,06:03:00,S1,3\nT1,06:04:00,06:04:00,S2,4\nT1,06:05:00,06:05:00,S1,5\nT1,06:06:00,06:06:00,S2,6\nT1,06:07:00,06:07:00,S1,7\nT1,06:08:00,06:08:00,S2,8\nT1,06:09:00,06:09:00,S1,9\nT1,06:10:00,06:10:00,S2,10\nT1,06:11:00,06:11:00,S1,11\nT1,06:12:00,06:12:00,S2,12\nT1,06:13:00,06:13:00,S1,13\nT1,06:14:00,06:14:00,S2,14\nT1,06:15:00,06:15:00,S1,15\nT1,06:16:00,06:16:00,S2,16\nT1,06:17:00,06:17:00,S1,17\nT1,06:18:00,06:18:00,S2,18\nT1,06:19:00,06:19:00,S1,19\nT1,06:20:00,06:20:00,S2,20\nT1,06:21:00,06:21:00,S1,21\nT1,06:22:00,06:22:00,S2,22\nT1,06:23:00,06:23:00,S1,23\nT1,06:24:00,06:24:00,S2,24\nT1,06:25:00,06:25:00,S1,25\nT1,06:26:00,06:26:00,S2,26\nT1,06:27:00,06:27:00,S1,27\nT1,06:28:00,06:28:00,S2,28\nT1,06:29:00,06:29:00,S1,29\nT1,06:30:00,06:30:00,S2,30\nT1,06:31:00,06:31:00,S1,31\nT1,06:32:00,06:32:00,S2,32\nT1,06:33:00,06:33:00,S1,33\nT1,06:34:00,06:34:00,S2,34\nT1,06:35:00,06:35:00,S1,35\nT1,06:36:00,06:36:00,S2,36\nT1,06:37:00,06:37:00,S1,37\nT1,06:38:00,06:38:00,S2,38\nT1,06:39:00,06:39:00,S1,39\nT1,06:40:00,06:40:00,S2,40\nT1,06:41:00,06:41:00,S1,41\nT1,06:42:00,06:42:00,S2,42\nT1,06:43:00,06:43:00,S1,43\nT1,06:44:00,06:44:00,S2,44\nT1,06:45:00,06:45:00,S1,45\nT1,06:46:00,06:46:00,S2,46\nT1,06:47:00,06:47:00,S1,47\nT1,06:48:00,06:48:00,S2,48\nT1,06:49:00,06:49:00,S1,49\nT1,06:50:00,06:50:00,S2,50\nT1,06:51:00,06:51:00,S1,51\nT1,06:52:00,06:52:00,S2,52\nT1,06:53:00,06:53:00,S1,53\nT1,06:54:00,06:54:00,S2,54\nT1,06:55:00,06:55:00,S1,55\nT1,06:56:00,06:56:00,S2,56\nT1,06:57:00,06:57:00,S1,57\nT1,06:58:00,06:58:00,S2,58\nT1,06:59:00,06:59:00,S1,59\nT1,07:00:00,07:00:00,S2,60\nT1,07:01:00,07:01:00,S1,61\nT1,07:02:00,07:02:00,S2,62\nT1,07:03:00,07:03:00,S1,63\nT1,07:04:00,07:04:00,S2,64\nT1,07:05:00,07:05:00,S1,65\nT1,07:06:00,07:06:00,S2,66\nT1,07:07:00,07:07:00,S1,67\nT1,07:08:00,07:08:00,S2,68\nT1,07:09:00,07:09:00,S1,69\nT1,07:10:00,07:10:00,S2,70\nT1,07:11:00,07:11:00,S1,71\nT1,07:12:00,07:12:00,S2,72\nT1,07:13:00,07:13:00,S1,73\nT1,07:14:00,07:14:00,S2,74\nT1,07:15:00,07:15:00,S1,75\nT1,07:16:00,07:16:00,S2,76\nT1,07:17:00,07:17:00,S1,77\nT1,07:18:00,07:18:00,S2,78\nT1,07:19:00,07:19:00,S1,79\nT1,07:20:00,07:20:00,S2,80\nT1,07:21:00,07:21:00,S1,81\nT1,07:22:00,07:22:00,S2,82\nT1,07:23:00,07:23:00,S1,83\nT1,07:24:00,07:24:00,S2,84\nT1,07:25:00,07:25:00,S1,85\nT1,07:26:00,07:26:00,S2,86\nT1,07:27:00,07:27:00,S1,87\nT1,07:28:00,07:28:00,S2,88\nT1,07:29:00,07:29:00,S1,89\nT1,07:30:00,07:30:00,S2,90\nT1,07:31:00,07:31:00,S1,91\nT1,07:32:00,07:32:00,S2,92\nT1,07:33:00,07:33:00,S1,93\nT1,07:34:00,07:34:00,S2,94\nT1,07:35:00,07:35:00,S1,95\nT1,07:36:00,07:36:00,S2,96\nT1,07:37:00,07:37:00,S1,97\nT1,07:38:00,07:38:00,S2,98\nT1,07:39:00,07:39:00,S1,99\nT1,07:40:00,07:40:00,S2,100\nT1,07:41:00,07:41:00,S1,101\n"),
        ], ValidatorConfig { max_total_stop_times: 100, ..ValidatorConfig::default() }),
        // ARC_033: RFC 4180 — tırnaksız alanda tırnak. `12" Street` GEÇERSİZ; doğrusu
        // `"12"" Street"`. Tokenizer 2026-08-07'ye kadar bunu sıradan karakter sayıyordu.
        fx("ARC_033", vec![
            ("stops.txt", "stop_id,stop_name,stop_lat,stop_lon\nS1,12\" Street,41.0,29.0\nS2,Stop2,41.1,29.1\n"),
        ]),
        // ARC_022: dosya satır sayısı eşiği. 2026-08-07'de (issue #71) eşik `k1_parse`'ın
        // SABİT KODUNDAN `ValidatorConfig`'e taşındı — borç defteri "1M satır gerektirir,
        // inline yazılamaz" diyordu, doğrusu "K1 `parse()` config ALMIYORDU" idi ve o
        // düzeltilebilir bir şeydi. Burada K1 yolu (stops.txt akış-dışı okunur) kanıtlanır;
        // K2 akış yolları (shapes/stop_times) integration.rs::arc022_* ile kanıtlı.
        // Config alt sınırı 10 → 11 satır yeter.
        fx_cfg("ARC_022", vec![
            ("stops.txt", "stop_id,stop_name,stop_lat,stop_lon\nS1,Stop1,41.001,29.001\nS2,Stop2,41.002,29.002\nS3,Stop3,41.003,29.003\nS4,Stop4,41.004,29.004\nS5,Stop5,41.005,29.005\nS6,Stop6,41.006,29.006\nS7,Stop7,41.007,29.007\nS8,Stop8,41.008,29.008\nS9,Stop9,41.009,29.009\nS10,Stop10,41.010,29.010\nS11,Stop11,41.011,29.011\n"),
        ], ValidatorConfig { max_file_rows: 10, ..ValidatorConfig::default() }),
        // ARC_027: ZIP girdisinde sahip-okuma biti yok (0o200). İçerikle değil zip ÜST
        // VERİSİYLE tetiklendiği için normal fixture yolu üretemiyordu; harness'a
        // `unix_permissions` yazan bir kurucu eklendi.
        fx_noread("ARC_027", vec!["stops.txt"]),
        fx_cfg("OPR_024", vec![
            ("trips.txt", "route_id,service_id,trip_id\nR1,SVC1,T1\nR1,SVC1,T2\nR1,SVC1,T3\nR1,SVC1,T4\nR1,SVC1,T5\nR1,SVC1,T6\nR1,SVC1,T7\nR1,SVC1,T8\nR1,SVC1,T9\nR1,SVC1,T10\nR1,SVC1,T11\nR1,SVC1,T12\nR1,SVC1,T13\nR1,SVC1,T14\nR1,SVC1,T15\nR1,SVC1,T16\nR1,SVC1,T17\nR1,SVC1,T18\nR1,SVC1,T19\nR1,SVC1,T20\nR1,SVC1,T21\nR1,SVC1,T22\nR1,SVC1,T23\nR1,SVC1,T24\nR1,SVC1,T25\nR1,SVC1,T26\nR1,SVC1,T27\nR1,SVC1,T28\nR1,SVC1,T29\nR1,SVC1,T30\nR1,SVC1,T31\nR1,SVC1,T32\nR1,SVC1,T33\nR1,SVC1,T34\nR1,SVC1,T35\nR1,SVC1,T36\nR1,SVC1,T37\nR1,SVC1,T38\nR1,SVC1,T39\nR1,SVC1,T40\nR1,SVC1,T41\nR1,SVC1,T42\nR1,SVC1,T43\nR1,SVC1,T44\nR1,SVC1,T45\nR1,SVC1,T46\nR1,SVC1,T47\nR1,SVC1,T48\nR1,SVC1,T49\nR1,SVC1,T50\nR1,SVC1,T51\n"),
            ("stop_times.txt", "trip_id,arrival_time,departure_time,stop_id,stop_sequence\nT1,08:01:00,08:01:00,S1,1\nT1,09:01:00,09:01:00,S2,2\nT2,08:02:00,08:02:00,S1,1\nT2,09:02:00,09:02:00,S2,2\nT3,08:03:00,08:03:00,S1,1\nT3,09:03:00,09:03:00,S2,2\nT4,08:04:00,08:04:00,S1,1\nT4,09:04:00,09:04:00,S2,2\nT5,08:05:00,08:05:00,S1,1\nT5,09:05:00,09:05:00,S2,2\nT6,08:06:00,08:06:00,S1,1\nT6,09:06:00,09:06:00,S2,2\nT7,08:07:00,08:07:00,S1,1\nT7,09:07:00,09:07:00,S2,2\nT8,08:08:00,08:08:00,S1,1\nT8,09:08:00,09:08:00,S2,2\nT9,08:09:00,08:09:00,S1,1\nT9,09:09:00,09:09:00,S2,2\nT10,08:10:00,08:10:00,S1,1\nT10,09:10:00,09:10:00,S2,2\nT11,08:11:00,08:11:00,S1,1\nT11,09:11:00,09:11:00,S2,2\nT12,08:12:00,08:12:00,S1,1\nT12,09:12:00,09:12:00,S2,2\nT13,08:13:00,08:13:00,S1,1\nT13,09:13:00,09:13:00,S2,2\nT14,08:14:00,08:14:00,S1,1\nT14,09:14:00,09:14:00,S2,2\nT15,08:15:00,08:15:00,S1,1\nT15,09:15:00,09:15:00,S2,2\nT16,08:16:00,08:16:00,S1,1\nT16,09:16:00,09:16:00,S2,2\nT17,08:17:00,08:17:00,S1,1\nT17,09:17:00,09:17:00,S2,2\nT18,08:18:00,08:18:00,S1,1\nT18,09:18:00,09:18:00,S2,2\nT19,08:19:00,08:19:00,S1,1\nT19,09:19:00,09:19:00,S2,2\nT20,08:20:00,08:20:00,S1,1\nT20,09:20:00,09:20:00,S2,2\nT21,08:21:00,08:21:00,S1,1\nT21,09:21:00,09:21:00,S2,2\nT22,08:22:00,08:22:00,S1,1\nT22,09:22:00,09:22:00,S2,2\nT23,08:23:00,08:23:00,S1,1\nT23,09:23:00,09:23:00,S2,2\nT24,08:24:00,08:24:00,S1,1\nT24,09:24:00,09:24:00,S2,2\nT25,08:25:00,08:25:00,S1,1\nT25,09:25:00,09:25:00,S2,2\nT26,08:26:00,08:26:00,S1,1\nT26,09:26:00,09:26:00,S2,2\nT27,08:27:00,08:27:00,S1,1\nT27,09:27:00,09:27:00,S2,2\nT28,08:28:00,08:28:00,S1,1\nT28,09:28:00,09:28:00,S2,2\nT29,08:29:00,08:29:00,S1,1\nT29,09:29:00,09:29:00,S2,2\nT30,08:30:00,08:30:00,S1,1\nT30,09:30:00,09:30:00,S2,2\nT31,08:31:00,08:31:00,S1,1\nT31,09:31:00,09:31:00,S2,2\nT32,08:32:00,08:32:00,S1,1\nT32,09:32:00,09:32:00,S2,2\nT33,08:33:00,08:33:00,S1,1\nT33,09:33:00,09:33:00,S2,2\nT34,08:34:00,08:34:00,S1,1\nT34,09:34:00,09:34:00,S2,2\nT35,08:35:00,08:35:00,S1,1\nT35,09:35:00,09:35:00,S2,2\nT36,08:36:00,08:36:00,S1,1\nT36,09:36:00,09:36:00,S2,2\nT37,08:37:00,08:37:00,S1,1\nT37,09:37:00,09:37:00,S2,2\nT38,08:38:00,08:38:00,S1,1\nT38,09:38:00,09:38:00,S2,2\nT39,08:39:00,08:39:00,S1,1\nT39,09:39:00,09:39:00,S2,2\nT40,08:40:00,08:40:00,S1,1\nT40,09:40:00,09:40:00,S2,2\nT41,08:41:00,08:41:00,S1,1\nT41,09:41:00,09:41:00,S2,2\nT42,08:42:00,08:42:00,S1,1\nT42,09:42:00,09:42:00,S2,2\nT43,08:43:00,08:43:00,S1,1\nT43,09:43:00,09:43:00,S2,2\nT44,08:44:00,08:44:00,S1,1\nT44,09:44:00,09:44:00,S2,2\nT45,08:45:00,08:45:00,S1,1\nT45,09:45:00,09:45:00,S2,2\nT46,08:46:00,08:46:00,S1,1\nT46,09:46:00,09:46:00,S2,2\nT47,08:47:00,08:47:00,S1,1\nT47,09:47:00,09:47:00,S2,2\nT48,08:48:00,08:48:00,S1,1\nT48,09:48:00,09:48:00,S2,2\nT49,08:49:00,08:49:00,S1,1\nT49,09:49:00,09:49:00,S2,2\nT50,08:50:00,08:50:00,S1,1\nT50,09:50:00,09:50:00,S2,2\nT51,08:51:00,08:51:00,S1,1\nT51,09:51:00,09:51:00,S2,2\n"),
        ], ValidatorConfig { max_trips_per_route: 50, ..ValidatorConfig::default() }),
        fx("STM_060", vec![
            ("locations.geojson", concat!(
                r#"{"type":"FeatureCollection","features":["#,
                r#"{"type":"Feature","id":"Z1","properties":{},"geometry":{"type":"Polygon","coordinates":[[[0,0],[0,1],[1,1],[1,0],[0,0]]]}},"#,
                r#"{"type":"Feature","id":"Z2","properties":{},"geometry":{"type":"Polygon","coordinates":[[[0.5,0.5],[0.5,1.5],[1.5,1.5],[1.5,0.5],[0.5,0.5]]]}}]}"#)),
            ("stop_times.txt", concat!(
                "trip_id,location_id,stop_sequence,start_pickup_drop_off_window,end_pickup_drop_off_window,pickup_type,drop_off_type\n",
                "T1,Z1,1,08:00:00,10:00:00,2,2\n",
                "T1,Z2,2,09:00:00,11:00:00,2,2\n")),
        ]),
        fx("STM_059", vec![("stop_times.txt", "trip_id,stop_id,stop_sequence,pickup_type\nT1,S1,1,2\n"), ("booking_rules.txt","booking_rule_id,booking_type\nBR1,0\n")]),
        fx("STM_041", vec![("stop_times.txt", "trip_id,stop_id,stop_sequence,location_id,start_pickup_drop_off_window,end_pickup_drop_off_window,pickup_booking_rule_id\nT1,S1,1,LOC1,09:00:00,10:00:00,BR1\n")]),
        fx("STM_051", vec![("stop_times.txt", "trip_id,stop_sequence,location_id,start_pickup_drop_off_window,end_pickup_drop_off_window,pickup_booking_rule_id,pickup_type\nT1,1,LOC1,09:00:00,10:00:00,BR1,0\n")]),
        fx("STM_052", vec![("stop_times.txt", "trip_id,stop_sequence,location_id,start_pickup_drop_off_window,end_pickup_drop_off_window,pickup_booking_rule_id,drop_off_type\nT1,1,LOC1,09:00:00,10:00:00,BR1,0\n")]),
        // STM_054/055: Flex penceresi tanımlıyken continuous_pickup/drop_off 1/boş dışında (satır kapsamlı).
        fx("STM_054", vec![("stop_times.txt", "trip_id,stop_sequence,location_id,start_pickup_drop_off_window,end_pickup_drop_off_window,pickup_booking_rule_id,continuous_pickup\nT1,1,LOC1,09:00:00,10:00:00,BR1,0\n")]),
        fx("STM_055", vec![("stop_times.txt", "trip_id,stop_sequence,location_id,start_pickup_drop_off_window,end_pickup_drop_off_window,pickup_booking_rule_id,continuous_drop_off\nT1,1,LOC1,09:00:00,10:00:00,BR1,2\n")]),

        // ── PTH grubu (pathways.txt; base'te yok, eklenir) ─────────────────────
        // K2 satır kontrolleri (k2/pathways.rs). Temiz satır şablonu: mode=3, bidir=0.
        fx("PTH_004", vec![("pathways.txt", "pathway_id,from_stop_id,to_stop_id,pathway_mode,is_bidirectional\nP1,S1,S2,9,0\n")]),
        fx("PTH_005", vec![("pathways.txt", "pathway_id,from_stop_id,to_stop_id,pathway_mode,is_bidirectional\nP1,S1,S2,3,5\n")]),
        fx("PTH_006", vec![("pathways.txt", "pathway_id,from_stop_id,to_stop_id,pathway_mode,is_bidirectional,length\nP1,S1,S2,3,0,-1\n")]),
        fx("PTH_007", vec![("pathways.txt", "pathway_id,from_stop_id,to_stop_id,pathway_mode,is_bidirectional,traversal_time\nP1,S1,S2,3,0,0\n")]),
        fx("PTH_008", vec![("pathways.txt", "pathway_id,from_stop_id,to_stop_id,pathway_mode,is_bidirectional\nP1,S1,S2,2,0\n")]),
        // PTH_027: stair_count sıfır — `Non-null integer` tipinin dışladığı tek değer.
        fx("PTH_027", vec![("pathways.txt", "pathway_id,from_stop_id,to_stop_id,pathway_mode,is_bidirectional,stair_count\nP1,S1,S2,2,0,0\n")]),
        fx("PTH_025", vec![("pathways.txt", "pathway_id,from_stop_id,to_stop_id,pathway_mode,is_bidirectional,length\nP1,S1,S2,6,0,\n")]),
        fx("PTH_029", vec![("pathways.txt", "pathway_id,from_stop_id,to_stop_id,pathway_mode,is_bidirectional,traversal_time\nP1,S1,S2,4,0,\n")]),
        fx("PTH_030", vec![("stops.txt","stop_id,stop_name,stop_lat,stop_lon,location_type,parent_station\nST1,St,41.0,29.0,1,\nP1,Pl,41.0,29.0,0,ST1\nBA1,Ba,41.0,29.0,4,P1\nE1,En,41.0,29.0,2,ST1\n"), ("pathways.txt","pathway_id,from_stop_id,to_stop_id,pathway_mode,is_bidirectional\nW1,E1,P1,1,0\n")]),
        fx("PTH_009", vec![("pathways.txt", "pathway_id,from_stop_id,to_stop_id,pathway_mode,is_bidirectional\nP1,S1,S2,1,0\n")]),
        fx("PTH_010", vec![("pathways.txt", "pathway_id,from_stop_id,to_stop_id,pathway_mode,is_bidirectional,min_width\nP1,S1,S2,3,0,0\n")]),
        fx("PTH_011", vec![("pathways.txt", "pathway_id,from_stop_id,to_stop_id,pathway_mode,is_bidirectional\nP1,S1,S1,3,0\n")]),
        fx("PTH_016", vec![("pathways.txt", "pathway_id,from_stop_id,to_stop_id,pathway_mode,is_bidirectional\nP1,S1,S2,7,1\n")]),
        // PTH_017 artık YALNIZ tip ihlali: max_slope sayı değil (spec tipi `Float`).
        fx("PTH_017", vec![("pathways.txt", "pathway_id,from_stop_id,to_stop_id,pathway_mode,is_bidirectional,max_slope\nP1,S1,S2,1,0,abc\n")]),
        // PTH_028: max_slope geçerli ama pathway_mode 1/3 değil — spec burada "should" der.
        fx("PTH_028", vec![("pathways.txt", "pathway_id,from_stop_id,to_stop_id,pathway_mode,is_bidirectional,max_slope\nP1,S1,S2,4,0,0.05\n")]),
        // TRP_034: safe_duration alanı ondalık sayı değil (eskiden sessizce yutuluyordu).
        // İKİ sefer: TRP_034 de Entity (trip_id) dedup'lıdır, tek satırda iki alanı bozmak
        // yalnız birini çapalar (CAL_002 ile aynı ders).
        fx("TRP_034", vec![("trips.txt", concat!(
            "route_id,service_id,trip_id,safe_duration_factor,safe_duration_offset\n",
            "R1,SVC1,T1,abc,\n",
            "R1,SVC1,T2,,abc\n",
        ))]),
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
        // PTH_026: pathway uç noktası istasyon (loc=1) olamaz (k4).
        fx("PTH_026", vec![
            ("stops.txt", "stop_id,stop_name,stop_lat,stop_lon,location_type\nS1,Station,41.0,29.0,1\nS2,Stop2,41.1,29.1,0\n"),
            ("pathways.txt", "pathway_id,from_stop_id,to_stop_id,pathway_mode,is_bidirectional\nP1,S1,S2,3,0\n"),
        ]),
        // PTH_031: aynı spec cümlesinin ikinci kolu — uç nokta stop_access=1 olan bir peron.
        // `stop_access` yalnız parent_station'ı olan peronda meşrudur (yoksa STP_026/028),
        // bu yüzden fixture bir istasyon + iki peron kurar.
        fx("PTH_031", vec![
            ("stops.txt", "stop_id,stop_name,stop_lat,stop_lon,location_type,parent_station,stop_access\nST,Station,41.0,29.0,1,,\nS1,Platform1,41.0,29.0,0,ST,1\nS2,Platform2,41.1,29.1,0,ST,0\n"),
            ("stop_times.txt", "trip_id,arrival_time,departure_time,stop_id,stop_sequence\nT1,08:00:00,08:00:00,S1,1\nT1,08:10:00,08:10:00,S2,2\n"),
            ("pathways.txt", "pathway_id,from_stop_id,to_stop_id,pathway_mode,is_bidirectional\nP1,S1,S2,1,1\n"),
        ]),
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
        // LOC_008: feature "type" yok.
        fx("LOC_008", vec![("locations.geojson", "{\"type\":\"FeatureCollection\",\"features\":[{\"id\":\"L1\",\"properties\":{},\"geometry\":{\"type\":\"Polygon\",\"coordinates\":[[[0,0],[0,0.01],[0.01,0.01],[0,0]]]}}]}")]),
        // LOC_009: feature "properties" yok.
        fx("LOC_009", vec![("locations.geojson", "{\"type\":\"FeatureCollection\",\"features\":[{\"type\":\"Feature\",\"id\":\"L1\",\"geometry\":{\"type\":\"Polygon\",\"coordinates\":[[[0,0],[0,0.01],[0.01,0.01],[0,0]]]}}]}")]),
        // LOC_010: geometry var ama "coordinates" yok.
        fx("LOC_010", vec![("locations.geojson", "{\"type\":\"FeatureCollection\",\"features\":[{\"type\":\"Feature\",\"id\":\"L1\",\"properties\":{},\"geometry\":{\"type\":\"Polygon\"}}]}")]),
        // Kelebek (bowtie): dış ring kendini keser — OpenGIS 6.1.11 ihlali.
        fx("LOC_011", vec![("locations.geojson", "{\"type\":\"FeatureCollection\",\"features\":[{\"type\":\"Feature\",\"id\":\"L1\",\"properties\":{},\"geometry\":{\"type\":\"Polygon\",\"coordinates\":[[[0,0],[2,2],[2,0],[0,2],[0,0]]]}}]}")]),
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
        // DQ_021 iki tekil birincil anahtarı birden kanıtlar: stops.stop_id ve
        // attributions.attribution_id (spec tipi `Unique ID`). İkincisi ayrı bir fixture
        // gerektirmez — aynı kural, aynı olgu, iki dosya.
        fx("DQ_021", vec![
            ("stops.txt", "stop_id,stop_name,stop_lat,stop_lon\nS1,A,40.0,40.0\nS1,B,41.1,29.1\n"),
            ("attributions.txt", "attribution_id,organization_name,is_producer\nA1,Org,1\nA1,Org2,1\n"),
        ]),
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
        // T1 ve T2 AYNI operasyonel sefer (aynı yön, aynı kalkış, aynı durak dizisi) ama
        // iki ayrı servisten aktif → gerçek çakışma. Farklı seferler olsaydı meşru olurdu.
        fx("OPR_019", vec![
            ("calendar.txt", "service_id,monday,tuesday,wednesday,thursday,friday,saturday,sunday,start_date,end_date\nSVC1,1,1,1,1,1,1,1,20260518,20260524\nSVC2,1,1,1,1,1,1,1,20260518,20260524\n"),
            ("trips.txt", "route_id,service_id,trip_id\nR1,SVC1,T1\nR1,SVC2,T2\n"),
            ("stop_times.txt", "trip_id,arrival_time,departure_time,stop_id,stop_sequence\nT1,08:00:00,08:00:00,S1,1\nT1,08:10:00,08:10:00,S2,2\nT2,08:00:00,08:00:00,S1,1\nT2,08:10:00,08:10:00,S2,2\n"),
        ]),
        // OPR_020: hatta exception gününde >1 aktif servis (override çakışması).
        fx("OPR_020", vec![
            ("calendar.txt", "service_id,monday,tuesday,wednesday,thursday,friday,saturday,sunday,start_date,end_date\nSVC1,1,1,1,1,1,1,1,20260518,20260518\n"),
            ("calendar_dates.txt", "service_id,date,exception_type\nSVC2,20260518,1\n"),
            ("trips.txt", "route_id,service_id,trip_id\nR1,SVC1,T1\nR1,SVC2,T2\n"),
            ("stop_times.txt", "trip_id,arrival_time,departure_time,stop_id,stop_sequence\nT1,08:00:00,08:00:00,S1,1\nT1,08:10:00,08:10:00,S2,2\nT2,08:00:00,08:00:00,S1,1\nT2,08:10:00,08:10:00,S2,2\n"),
        ]),
        // OPR_025: feed ortalama sefer süresi < 60s (5 trip, 10s'lik).
        fx("OPR_025", vec![
            ("trips.txt", "route_id,service_id,trip_id\nR1,SVC1,T1\nR1,SVC1,T2\nR1,SVC1,T3\nR1,SVC1,T4\nR1,SVC1,T5\n"),
            ("stop_times.txt", "trip_id,arrival_time,departure_time,stop_id,stop_sequence\nT1,08:00:00,08:00:00,S1,1\nT1,08:00:10,08:00:10,S2,2\nT2,08:00:00,08:00:00,S1,1\nT2,08:00:10,08:00:10,S2,2\nT3,08:00:00,08:00:00,S1,1\nT3,08:00:10,08:00:10,S2,2\nT4,08:00:00,08:00:00,S1,1\nT4,08:00:10,08:00:10,S2,2\nT5,08:00:00,08:00:00,S1,1\nT5,08:00:10,08:00:10,S2,2\n"),
        ]),

        // ── XFL grubu (cross-file FK; k4_cross_ref::check_xfl + cemv/fares v2) ──
        // XFL_002: trip'in stop_times kaydı yok (T2).
        fx("XFL_002", vec![("trips.txt", "route_id,service_id,trip_id\nR1,SVC1,T1\nR1,SVC1,T2\n")]),
        // XFL_006: calendar_dates yalnız exception_type=2 ve calendar'da yok.
        fx("XFL_006", vec![("calendar_dates.txt", "service_id,date,exception_type\nSVCZ,20260601,2\n")]),
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
        // XFL_031: location_group_id ile stop_id aynı isim alanında çakışıyor.
        fx("XFL_031", vec![
            ("stops.txt", "stop_id,stop_name,stop_lat,stop_lon\nS1,Stop1,41.0,29.0\nS2,Stop2,41.1,29.1\nLG1,Cakisan,41.2,29.2\n"),
            ("location_groups.txt", "location_group_id,location_group_name\nLG1,Bolge\n"),
        ]),
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
        // NET_002: route_networks.network_id networks.txt'te yok (k4).
        fx("NET_002", vec![
            ("networks.txt", "network_id,network_name\nN1,Net1\n"),
            ("route_networks.txt", "network_id,route_id\nNOPE,R1\n"),
        ]),
        // NET_003: route_networks.route_id routes.txt'te yok (k4).
        fx("NET_003", vec![
            ("networks.txt", "network_id,network_name\nN1,Net1\n"),
            ("route_networks.txt", "network_id,route_id\nN1,NOPE\n"),
        ]),
        // PDW_006: aynı trip+zone içinde örtüşen pickup/drop-off pencereleri (k6 Flex).
        fx("PDW_006", vec![("stop_times.txt", "trip_id,stop_sequence,location_id,start_pickup_drop_off_window,end_pickup_drop_off_window\nT1,1,Z1,09:00:00,10:00:00\nT1,2,Z1,09:30:00,11:00:00\n")]),

        // ── CAL grubu (takvim analitiği k6 + cross-ref k4 + k2) ────────────────
        // TODAY=20260515. CAL_006: mdb-2830 biçimindeki boşluklu all-zero row (k2),
        // calendar_dates override'ı olsa da haftalık pattern bulgusu korunur.
        fx("CAL_006", vec![
            ("calendar.txt", "service_id,monday,tuesday,wednesday,thursday,friday,saturday,sunday,start_date,end_date\nSVC1, 0, 0, 0, 0, 0, 0, 0, 20250101, 20271231\n"),
            ("calendar_dates.txt", "service_id,date,exception_type\nSVC1,20260520,1\n"),
        ]),
        // CAL_007: serviste >= 7 günlük boşluk (calendar_dates, calendar yok). Boşluk GEÇMİŞTE
        // (today=20260515 öncesi) → yakın-gelecek değil, CAL_007 üretir (yakın gelecekte CAL_012
        // onun yerine geçer, #29).
        fx_rm("CAL_007", vec![("calendar_dates.txt", "service_id,date,exception_type\nSVC1,20260401,1\nSVC1,20260420,1\n")], vec!["calendar.txt"]),
        // CAL_008: end_date 30 gün içinde bitiyor (bugün+10).
        fx("CAL_008", vec![("calendar.txt", "service_id,monday,tuesday,wednesday,thursday,friday,saturday,sunday,start_date,end_date\nSVC1,1,1,1,1,1,1,1,20260101,20260525\n")]),
        // CAL_009: tüm servisler sona ermiş (k4 kritik).
        fx("CAL_009", vec![("calendar.txt", "service_id,monday,tuesday,wednesday,thursday,friday,saturday,sunday,start_date,end_date\nSVC1,1,1,1,1,1,0,0,20200101,20201231\n")]),
        // CAL_010: toplam aktif gün <= 7 (1 haftalık pencere).
        // CAL_010: kısa servis. SVC2 yayın penceresini ≥30 güne açar (mutlak eşik yolu);
        // pencere kısa olsaydı kural ORAN testine düşer ve %100 kapsayan SVC1 susardı.
        fx("CAL_010", vec![("calendar.txt", "service_id,monday,tuesday,wednesday,thursday,friday,saturday,sunday,start_date,end_date\nSVC1,1,1,1,1,1,1,1,20260518,20260524\nSVC2,1,1,1,1,1,1,1,20260101,20261231\n")]),
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
        // ATR_011: attribution route_id routes.txt'te yok (k4).
        fx("ATR_011", vec![("attributions.txt", "attribution_id,organization_name,is_producer,route_id\nA1,Org,1,NOPE\n")]),
        // ATR_012: attribution trip_id trips.txt'te yok (k4).
        fx("ATR_012", vec![("attributions.txt", "attribution_id,organization_name,is_producer,trip_id\nA1,Org,1,NOPE\n")]),

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
        // FPD_001: bileşik PK tekrarı (k3), aynı fare_product_id + rider_category_id + fare_media_id.
        fx("FPD_001", vec![
            ("fare_media.txt", "fare_media_id,fare_media_name,fare_media_type\ncash,Cash,0\n"),
            ("fare_products.txt", "fare_product_id,amount,currency,fare_media_id\nP1,2.5,USD,cash\nP1,2.5,USD,cash\n"),
        ]),
        // FPD_003: currency geçersiz (k2).
        fx("FPD_003", vec![("fare_products.txt", "fare_product_id,amount,currency\nP1,2.5,usd\n")]),
        // FPD_004: fare_media_id fare_media.txt'te yok (k4).
        fx("FPD_004", vec![("fare_products.txt", "fare_product_id,amount,currency,fare_media_id\nP1,2.5,USD,MNOPE\n")]),
        // FPD_005: rider_category_id rider_categories.txt'te yok (k4).
        fx("FPD_005", vec![("fare_products.txt", "fare_product_id,amount,currency,rider_category_id\nP1,2.5,USD,RCNOPE\n")]),
        fx("FPD_007", vec![("fare_products.txt","fare_product_id,fare_product_name,amount,currency\nP1,Bilet,0.9,EUR\n")]),

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
        fx("FRQ_012", vec![("frequencies.txt", "trip_id,start_time,end_time,headway_secs,exact_times\nT1,05:00:00,06:00:00,3600,1\n"), ("trips.txt","route_id,service_id,trip_id\nR1,SVC,T1\n")]),

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
        // TRP_025: wheelchair_accessible bilinmeyen oran %80–99. Base feed'in tek seferi
        // %100 verirdi, o ise TRP_029'un kapsamı — burada 6 seferin 1'i bildirilmiş (%83).
        fx("TRP_025", vec![("trips.txt", "route_id,service_id,trip_id,wheelchair_accessible\n\
              R1,SVC1,T1,1\nR1,SVC1,T2,\nR1,SVC1,T3,\nR1,SVC1,T4,\nR1,SVC1,T5,\nR1,SVC1,T6,\n")]),
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
        // TRN_015: record_id ve field_value ikisi de boş — "Required if record_id is empty".
        fx("TRN_015", vec![("translations.txt", "table_name,field_name,language,translation\nstops,stop_name,fr,X\n")]),
        fx("TRN_016", vec![("stops.txt","stop_id,stop_name,stop_lat,stop_lon\nS1,Merkez,41.0,29.0\n"), ("translations.txt","table_name,field_name,language,translation,field_value\nstops,stop_name,en,Centre,Yokistan\n")]),
        fx("TRN_017", vec![("stop_times.txt","trip_id,stop_id,stop_sequence,arrival_time,departure_time\nT1,S1,1,08:00:00,08:00:00\n"), ("translations.txt","table_name,field_name,language,translation,record_id\nstop_times,stop_headsign,en,Centre,T1\n")]),

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
        // STP_042: stop_url URL biçiminde değil.
        fx("STP_042", vec![("stops.txt", "stop_id,stop_name,stop_lat,stop_lon,stop_url\nS1,Stop1,41.0,29.0,notaurl\n")]),
        // STP_043: stop_access, parent_station'ı olmayan bir durakta kullanılmış (spec: yasak).
        fx("STP_043", vec![("stops.txt", "stop_id,stop_name,stop_lat,stop_lon,location_type,parent_station,stop_access\nS1,Stop1,41.0,29.0,0,,1\nS2,Stop2,41.1,29.1,,,\n")]),
        fx("STP_044", vec![("stops.txt", "stop_id,stop_name,stop_lat,stop_lon,platform_code\nS1,Stop1,41.0,29.0,Gleis 5\n")]),
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
        // SHP_006: kullanılan tek noktalı shape (k5); kullanılmayan tek nokta SHP_018 kapsamındadır.
        fx("SHP_006", vec![
            ("trips.txt", "route_id,service_id,trip_id,shape_id\nR1,SVC1,T1,SH1\n"),
            ("shapes.txt", "shape_id,shape_pt_lat,shape_pt_lon,shape_pt_sequence\nSH1,41.0,29.0,1\n"),
        ]),
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
        // SHP_028: aynı dist farklı koordinat (eşik üstü ~0.1°) (k6).
        fx("SHP_028", vec![("shapes.txt", "shape_id,shape_pt_lat,shape_pt_lon,shape_pt_sequence,shape_dist_traveled\nSH1,40.0,40.0,1,5\nSH1,40.1,40.0,2,5\nSH1,40.2,40.2,3,10\n")]),
        // SHP_029: aynı dist farklı koordinat (eşik altı ~1e-6°) (k6).
        fx("SHP_029", vec![("shapes.txt", "shape_id,shape_pt_lat,shape_pt_lon,shape_pt_sequence,shape_dist_traveled\nSH1,40.0,40.0,1,5\nSH1,40.000001,40.0,2,5\nSH1,40.1,40.1,3,10\n")]),
        // SHP_030: stop_times shape_dist_traveled kullanıyor, ancak shape noktalarının
        // yalnızca birinde karşılık gelen shapes.txt mesafesi var (k6, shape özeti).
        fx("SHP_030", vec![
            ("trips.txt", "route_id,service_id,trip_id,shape_id\nR1,SVC1,T1,SH1\n"),
            ("shapes.txt", "shape_id,shape_pt_lat,shape_pt_lon,shape_pt_sequence,shape_dist_traveled\nSH1,41.0,29.0,1,0\nSH1,41.1,29.1,2,\n"),
            ("stop_times.txt", "trip_id,arrival_time,departure_time,stop_id,stop_sequence,shape_dist_traveled\nT1,08:00:00,08:00:00,S1,1,0\nT1,08:10:00,08:10:00,S2,2,1\n"),
        ]),

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
        // STM_028: trip süresi > 24 saat (k6).
        fx("STM_028", vec![("stop_times.txt", "trip_id,arrival_time,departure_time,stop_id,stop_sequence\nT1,08:00:00,08:00:00,S1,1\nT1,33:00:00,33:00:00,S2,2\n")]),
        // STM_029: trip süresi < 60s (k6).
        fx("STM_029", vec![
            ("stops.txt", "stop_id,stop_name,stop_lat,stop_lon\nS1,Stop1,40.0,40.0\nS2,Stop2,40.0005,40.0\n"),
            ("stop_times.txt", "trip_id,arrival_time,departure_time,stop_id,stop_sequence\nT1,08:00:00,08:00:00,S1,1\nT1,08:00:30,08:00:30,S2,2\n"),
        ]),
        // STM_032: (trip_id, stop_sequence) tekrarı (k2).
        fx("STM_032", vec![("stop_times.txt", "trip_id,arrival_time,departure_time,stop_id,stop_sequence\nT1,08:00:00,08:00:00,S1,1\nT1,08:10:00,08:10:00,S2,1\n")]),
        // STM_056: ardışık shape_dist_traveled artmıyor (ikinci değer birinciye eşit).
        fx("STM_056", vec![("stop_times.txt", "trip_id,arrival_time,departure_time,stop_id,stop_sequence,shape_dist_traveled\nT1,08:00:00,08:00:00,S1,1,100.0\nT1,08:10:00,08:10:00,S2,2,100.0\n")]),
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
        // STM_048: gece yarısı sonrası 00:xx yazılmış (k2 raw Spec, normalization öncesi).
        fx("STM_048", vec![("stop_times.txt", "trip_id,arrival_time,departure_time,stop_id,stop_sequence\nT1,23:50:00,23:50:00,S1,1\nT1,00:10:00,00:10:00,S2,2\n")]),
        // STM_049: gece yarısı sonrası 00:xx kalkış aynı satırda (k2 raw Spec).
        fx("STM_049", vec![("stop_times.txt", "trip_id,arrival_time,departure_time,stop_id,stop_sequence\nT1,23:50:00,00:10:00,S1,1\nT1,24:10:00,24:10:00,S2,2\n")]),
        // STM_050: timepoint sütunu var ama değer boş (k2).
        fx("STM_050", vec![("stop_times.txt", "trip_id,arrival_time,departure_time,stop_id,stop_sequence,timepoint\nT1,08:00:00,08:00:00,S1,1,\nT1,08:10:00,08:10:00,S2,2,\n")]),

        // ── ARC grubu (k1_parse arşiv/dosya/başlık). ARC_002/003 ham byte fixture'ıyla
        //    (fx_raw), ARC_022 config eşiğiyle (fx_cfg, yukarıda) kanıtlı; ARC_004 fatal
        //    → allowlist. Debt defteri BOŞ. ──────────────────────────────────────
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
        // TRF_021: aktarma uç noktası giriş/ara düğüm/biniş alanı olamaz (k4).
        fx("TRF_021", vec![
            ("stops.txt", "stop_id,stop_name,stop_lat,stop_lon,location_type\nS1,Entrance,41.0,29.0,2\nS2,Stop2,41.1,29.1,0\n"),
            ("transfers.txt", "from_stop_id,to_stop_id,transfer_type\nS1,S2,1\n"),
        ]),
        // TRF_016: aynı (stop,trip,route) kombinasyonunda çakışan transfer (k4).
        fx("TRF_016", vec![("transfers.txt", "from_stop_id,to_stop_id,transfer_type,from_trip_id\nS1,S2,1,T1\nS1,S2,1,T1\n")]),
        // TRF_017: from_trip_id'nin gerçek route'u from_route_id ile uyuşmuyor (k4).
        fx("TRF_017", vec![("transfers.txt", "from_stop_id,to_stop_id,transfer_type,from_trip_id,from_route_id\nS1,S2,1,T1,RWRONG\n")]),
        // TRF_018: from_trip_id == to_trip_id (anlamsız aktarma) (k4).
        fx("TRF_018", vec![("transfers.txt", "from_stop_id,to_stop_id,transfer_type,from_trip_id,to_trip_id\nS1,S2,1,T1,T1\n")]),
        // TRF_022: 1-to-n devamlılık — T1 hem T2'ye hem T3'e bağlanıyor, takvimleri FARKLI
        // ama çakışıyor (SVC1 hafta içi ⊂ SVC2 her gün) → o günlerde hangi devamlılık
        // geçerli belirsiz. ⚠️ Aynı takvimle yazılsa kural SUSMALI (gerçek 1-to-n ayrımı),
        // çakışmasalar da SUSMALI (spec'in izin verdiği ayrı devamlılıklar) — bkz. k6 notu.
        fx("TRF_022", vec![
            ("calendar.txt", "service_id,monday,tuesday,wednesday,thursday,friday,saturday,sunday,start_date,end_date\nSVC1,1,1,1,1,1,0,0,20250101,20271231\nSVC2,1,1,1,1,1,1,1,20250101,20271231\n"),
            ("trips.txt", "route_id,service_id,trip_id\nR1,SVC1,T1\nR1,SVC1,T2\nR1,SVC2,T3\n"),
            ("stop_times.txt", "trip_id,arrival_time,departure_time,stop_id,stop_sequence\nT1,08:00:00,08:00:00,S1,1\nT1,08:10:00,08:10:00,S2,2\nT2,09:00:00,09:00:00,S1,1\nT2,09:10:00,09:10:00,S2,2\nT3,10:00:00,10:00:00,S1,1\nT3,10:10:00,10:10:00,S2,2\n"),
            ("transfers.txt", "from_stop_id,to_stop_id,transfer_type,from_trip_id,to_trip_id\nS1,S2,4,T1,T2\nS1,S2,4,T1,T3\n"),
        ]),
        // TRF_023: n-to-1 devamlılık — T2 ve T3'ün ikisi de T1'e bağlanıyor, aynı çakışma.
        fx("TRF_023", vec![
            ("calendar.txt", "service_id,monday,tuesday,wednesday,thursday,friday,saturday,sunday,start_date,end_date\nSVC1,1,1,1,1,1,0,0,20250101,20271231\nSVC2,1,1,1,1,1,1,1,20250101,20271231\n"),
            ("trips.txt", "route_id,service_id,trip_id\nR1,SVC1,T1\nR1,SVC1,T2\nR1,SVC2,T3\n"),
            ("stop_times.txt", "trip_id,arrival_time,departure_time,stop_id,stop_sequence\nT1,08:00:00,08:00:00,S1,1\nT1,08:10:00,08:10:00,S2,2\nT2,09:00:00,09:00:00,S1,1\nT2,09:10:00,09:10:00,S2,2\nT3,10:00:00,10:00:00,S1,1\nT3,10:10:00,10:10:00,S2,2\n"),
            ("transfers.txt", "from_stop_id,to_stop_id,transfer_type,from_trip_id,to_trip_id\nS1,S2,4,T2,T1\nS1,S2,4,T3,T1\n"),
        ]),
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
        // S1'den 4 hat geçer VE ~200 m ötedeki S9'dan S1'de OLMAYAN bir hat (R5) geçer:
        // aynı stop_id'deki aktarma örtük olduğu için kural artık yakında bağlanmamış
        // ayrı bir durak ister.
        fx("VAT_002", vec![
            ("stops.txt", "stop_id,stop_name,stop_lat,stop_lon\nS1,Stop1,41.0,29.0\nS2,Stop2,41.1,29.1\nS9,Near,41.0018,29.0\nS8,Other,41.05,29.05\n"),
            ("routes.txt", "route_id,agency_id,route_short_name,route_type\nR1,1,1,3\nR2,1,2,3\nR3,1,3,3\nR4,1,4,3\nR5,1,5,3\n"),
            ("trips.txt", "route_id,service_id,trip_id\nR1,SVC1,T1\nR2,SVC1,T2\nR3,SVC1,T3\nR4,SVC1,T4\nR5,SVC1,T5\n"),
            ("stop_times.txt", "trip_id,arrival_time,departure_time,stop_id,stop_sequence\nT1,08:00:00,08:00:00,S1,1\nT1,08:10:00,08:10:00,S2,2\nT2,08:30:00,08:30:00,S1,1\nT2,08:40:00,08:40:00,S2,2\nT3,09:00:00,09:00:00,S1,1\nT3,09:10:00,09:10:00,S2,2\nT4,09:30:00,09:30:00,S1,1\nT4,09:40:00,09:40:00,S2,2\nT5,10:00:00,10:00:00,S9,1\nT5,10:10:00,10:10:00,S8,2\n"),
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
        // S2 üç hattın terminusu VE ~200 m ötedeki S9'dan S2'de OLMAYAN bir hat (R4) geçer.
        fx("VAT_007", vec![
            ("stops.txt", "stop_id,stop_name,stop_lat,stop_lon\nS1,Stop1,41.0,29.0\nS2,Stop2,41.1,29.1\nS3,Stop3,41.2,29.2\nS4,Stop4,41.3,29.3\nS9,Near,41.1018,29.1\nS5,Other,41.15,29.15\n"),
            ("routes.txt", "route_id,agency_id,route_short_name,route_type\nR1,1,1,3\nR2,1,2,3\nR3,1,3,3\nR4,1,4,3\n"),
            ("trips.txt", "route_id,service_id,trip_id\nR1,SVC1,T1\nR2,SVC1,T2\nR3,SVC1,T3\nR4,SVC1,T4\n"),
            ("stop_times.txt", "trip_id,arrival_time,departure_time,stop_id,stop_sequence\nT1,08:00:00,08:00:00,S1,1\nT1,08:10:00,08:10:00,S2,2\nT2,08:30:00,08:30:00,S3,1\nT2,08:40:00,08:40:00,S2,2\nT3,09:00:00,09:00:00,S4,1\nT3,09:10:00,09:10:00,S2,2\nT4,10:00:00,10:00:00,S9,1\nT4,10:10:00,10:10:00,S5,2\n"),
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
        let no_read: Vec<&str> = f.no_read.clone();
        let emitted = emitted_rules_modes(&with_opts(&f.overrides, &f.removes, &f.raw), &cfg, &no_read);
        if !emitted.contains(f.rule) {
            failures.push(format!("  {} emit etmedi → {:?}", f.rule, emitted));
        }
    }
    assert!(failures.is_empty(), "Emit etmeyen fixture(lar):\n{}", failures.join("\n"));
}

#[test]
fn shp030_aggregates_and_does_not_duplicate_related_findings() {
    let overrides = [
        ("trips.txt", "route_id,service_id,trip_id,shape_id\nR1,SVC1,T1,SH1\nR1,SVC1,T2,SH1\n"),
        ("shapes.txt", "shape_id,shape_pt_lat,shape_pt_lon,shape_pt_sequence,shape_dist_traveled\nSH1,41.0,29.0,1,0\nSH1,41.1,29.1,2,\n"),
        ("stop_times.txt", "trip_id,arrival_time,departure_time,stop_id,stop_sequence,shape_dist_traveled\nT1,08:00:00,08:00:00,S1,1,0\nT1,08:10:00,08:10:00,S2,2,1\nT2,09:00:00,09:00:00,S1,1,0\nT2,09:10:00,09:10:00,S2,2,1\n"),
    ];
    let positive = notices_for(&with_opts(&overrides, &[], &[]), &ValidatorConfig::default());
    let shp030: Vec<_> = positive.iter().filter(|n| n.rule_id == "SHP_030").collect();
    assert_eq!(shp030.len(), 1, "aynı shape için tek SHP_030 beklenir");
    assert_eq!(shp030[0].entity_id.as_deref(), Some("SH1"));
    assert_eq!(shp030[0].details.as_ref().and_then(|d| d.get("affected_trip_count")).map(String::as_str), Some("2"));
    for related in ["STM_017", "SHP_024", "SHP_025"] {
        assert!(!positive.iter().any(|n| n.rule_id == related), "{related} SHP_030 fixture'ında türememeli");
    }

    let complete_shapes = "shape_id,shape_pt_lat,shape_pt_lon,shape_pt_sequence,shape_dist_traveled\nSH1,41.0,29.0,1,0\nSH1,41.1,29.1,2,1\n";
    let negative = notices_for(&with_opts(&[
        overrides[0], ("shapes.txt", complete_shapes), overrides[2]
    ], &[], &[]), &ValidatorConfig::default());
    assert!(!negative.iter().any(|n| n.rule_id == "SHP_030"), "tam shape mesafesi SHP_030 üretmemeli");
}

#[test]
fn shp024_recovers_whitespace_coordinates_and_preserves_thresholds() {
    // tdg-83134 reduced case: coordinates are numerically valid after trimming, but the
    // lexical K2 path keeps them invalid so DQ_016/STP evidence remains available. SHP_024
    // must still use that payload for the user-distance comparison (139.4m and 165.6m).
    let stops = "stop_id,stop_name,stop_lat,stop_lon\n\
S1,Lycée Dupuy, 43.229610,   0.062651\n\
S2,Delacroix, 43.227402,   0.061883\n";
    let routes = "route_id,agency_id,route_short_name,route_type\nR1,1,101,3\n";
    let trips = "route_id,service_id,trip_id,shape_id\nR1,SVC1,T1,SH1\n";
    let shapes = "shape_id,shape_pt_lat,shape_pt_lon,shape_pt_sequence,shape_dist_traveled\n\
SH1,43.229612,0.060398,1,2.094\n\
SH1,43.229628,0.061190,2,2.158\n\
SH1,43.229660,0.062172,3,2.238\n\
SH1,43.229665,0.062354,4,2.253\n\
SH1,43.228891,0.062634,5,2.376\n\
SH1,43.228047,0.062443,6,2.471\n";
    let stop_times = "trip_id,arrival_time,departure_time,stop_id,stop_sequence,shape_dist_traveled\n\
T1,08:00:00,08:00:00,S1,1,2.137\n\
T1,08:10:00,08:10:00,S2,2,2.387\n";
    let files = with_opts(
        &[
            ("stops.txt", stops),
            ("routes.txt", routes),
            ("trips.txt", trips),
            ("shapes.txt", shapes),
            ("stop_times.txt", stop_times),
        ],
        &[],
        &[],
    );
    let bus = notices_for(&files, &ValidatorConfig::default());
    let bus_shp024: Vec<_> = bus.iter().filter(|n| n.rule_id == "SHP_024").collect();
    assert_eq!(bus_shp024.len(), 2, "iki >100m otobüs vakası ayrı raporlanmalı: {bus_shp024:?}");
    assert!(bus_shp024.iter().any(|n| n.entity_id.as_deref() == Some("S1")
        && n.observed_value.as_deref().is_some_and(|v| v.starts_with("139."))));
    assert!(bus_shp024.iter().any(|n| n.entity_id.as_deref() == Some("S2")
        && n.observed_value.as_deref().is_some_and(|v| v.starts_with("165."))));

    // Same geometry on a rail route: the intentional 200m rail threshold keeps both
    // ~139m/~166m platform-center offsets unflagged.
    let rail_routes = "route_id,agency_id,route_short_name,route_type\nR1,1,101,2\n";
    let rail_files = with_opts(
        &[("routes.txt", rail_routes), ("stops.txt", stops), ("trips.txt", trips),
          ("shapes.txt", shapes), ("stop_times.txt", stop_times)],
        &[],
        &[],
    );
    let rail = notices_for(&rail_files, &ValidatorConfig::default());
    assert!(!rail.iter().any(|n| n.rule_id == "SHP_024"),
        "rail eşik istisnası korunmalı: {rail:?}");
}

#[test]
fn shp024_boundary_uses_bus_threshold_not_geometric_projection() {
    let stops = "stop_id,stop_name,stop_lat,stop_lon\n\
NEAR,Near,40.0,0.0058\n\
FAR,Far,40.0,0.0062\n";
    let trips = "route_id,service_id,trip_id,shape_id\nR1,SVC1,T1,SH1\n";
    let shapes = "shape_id,shape_pt_lat,shape_pt_lon,shape_pt_sequence,shape_dist_traveled\n\
SH1,40.0,0.0,1,0\nSH1,40.0,0.01,2,1\n";
    let stop_times = "trip_id,arrival_time,departure_time,stop_id,stop_sequence,shape_dist_traveled\n\
T1,08:00:00,08:00:00,NEAR,1,0.5\n\
T1,08:10:00,08:10:00,FAR,2,0.5\n";
    let files = with_opts(
        &[("stops.txt", stops), ("trips.txt", trips), ("shapes.txt", shapes),
          ("stop_times.txt", stop_times)],
        &[],
        &[],
    );
    let notices = notices_for(&files, &ValidatorConfig::default());
    let shp024: Vec<_> = notices.iter().filter(|n| n.rule_id == "SHP_024").collect();
    assert_eq!(shp024.len(), 1, "100m üzerindeki yalnız FAR vakası raporlanmalı: {shp024:?}");
    assert_eq!(shp024[0].entity_id.as_deref(), Some("FAR"));
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

// #105 regression: aynı fare_product_id'nin farklı fare_media_id varyantları
// composite PK açısından birbirinden bağımsız geçerli kayıtlardır.
#[test]
fn fpd_distinct_fare_media_variants_are_not_duplicate_fpd_001() {
    let files = with_opts(
        &[
            ("fare_media.txt", "fare_media_id,fare_media_name,fare_media_type\ncash,Cash,0\ncontactless,Contactless,3\n"),
            ("fare_products.txt", "fare_product_id,amount,currency,fare_media_id\nP1,2.00,USD,cash\nP1,2.00,USD,contactless\n"),
        ],
        &[],
        &[],
    );
    let emitted = emitted_rules(&files, &ValidatorConfig::default());
    assert!(
        !emitted.contains("FPD_001"),
        "farklı fare_media_id varyantları FPD_001 üretmemeli, emit: {:?}",
        emitted,
    );
}

// #109 regression: raw after-midnight notation is a Spec finding, while the normalized
// analytical representation must not create a duplicate STM_008 finding.
#[test]
fn stm_048_reports_raw_rollover_as_spec_without_stm_008_duplicate() {
    let files = with_opts(
        &[
            (
                "stop_times.txt",
                "trip_id,arrival_time,departure_time,stop_id,stop_sequence\nT1,23:50:00,23:50:00,S1,1\nT1,00:10:00,00:10:00,S2,2\n",
            ),
        ],
        &[],
        &[],
    );
    let notices = notices_for(&files, &ValidatorConfig::default());
    let stm048: Vec<_> = notices.iter().filter(|n| n.rule_id == "STM_048").collect();
    assert_eq!(stm048.len(), 1, "raw rollover tek feed özeti olmalı: {stm048:?}");
    assert_eq!(stm048[0].rule_class, gtfs_core::RuleClass::Spec);
    assert_eq!(stm048[0].severity, gtfs_core::Severity::Yuksek);
    assert_eq!(stm048[0].entity_type, gtfs_core::EntityType::Trip);
    assert_eq!(stm048[0].entity_id.as_deref(), Some("T1"));
    assert_eq!(stm048[0].line, Some(3));
    assert_eq!(stm048[0].field.as_deref(), Some("arrival_time"));
    assert_eq!(stm048[0].observed_value.as_deref(), Some("00:10:00"));
    assert_eq!(stm048[0].expected_value.as_deref(), Some("24:10:00"));
    assert!(!notices.iter().any(|n| n.rule_id == "STM_008"),
        "normalization sonrası aynı olay STM_008 olarak tekrarlanmamalı: {notices:?}");
}

#[test]
fn stm_048_raw_detection_is_not_disabled_by_service_day_config() {
    let files = with_opts(
        &[(
            "stop_times.txt",
            "trip_id,arrival_time,departure_time,stop_id,stop_sequence\nT1,23:50:00,23:50:00,S1,1\nT1,00:10:00,00:10:00,S2,2\n",
        )],
        &[],
        &[],
    );
    let config = ValidatorConfig { service_day_start_hour: 0, ..ValidatorConfig::default() };
    let notices = notices_for(&files, &config);
    assert!(notices.iter().any(|n| n.rule_id == "STM_048"),
        "service_day_start_hour=0 raw STM_048'i kapatmamalı: {notices:?}");
}

#[test]
fn stm_048_detects_rollover_after_the_normalization_threshold() {
    let files = with_opts(
        &[(
            "stop_times.txt",
            "trip_id,arrival_time,departure_time,stop_id,stop_sequence\nT1,23:50:00,23:50:00,S1,1\nT1,03:30:00,03:30:00,S2,2\n",
        )],
        &[],
        &[],
    );
    let notices = notices_for(&files, &ValidatorConfig::default());
    assert!(notices.iter().any(|n| n.rule_id == "STM_048"),
        "03:30 raw rollover STM_048 üretmeli: {notices:?}");
}

#[test]
fn raw_same_row_departure_is_stm_049_spec_without_config_gate() {
    let files = with_opts(
        &[(
            "stop_times.txt",
            "trip_id,arrival_time,departure_time,stop_id,stop_sequence\nT1,23:59:00,00:02:00,S1,1\nT1,24:10:00,24:10:00,S2,2\n",
        )],
        &[],
        &[],
    );
    let config = ValidatorConfig { service_day_start_hour: 0, ..ValidatorConfig::default() };
    let notices = notices_for(&files, &config);
    let stm049: Vec<_> = notices.iter().filter(|n| n.rule_id == "STM_049").collect();
    assert_eq!(stm049.len(), 1, "same-row raw departure tek feed özeti olmalı: {stm049:?}");
    assert_eq!(stm049[0].rule_class, gtfs_core::RuleClass::Spec);
    assert_eq!(stm049[0].severity, gtfs_core::Severity::Yuksek);
    assert_eq!(stm049[0].entity_type, gtfs_core::EntityType::Trip);
    assert_eq!(stm049[0].entity_id.as_deref(), Some("T1"));
    assert_eq!(stm049[0].observed_value.as_deref(), Some("00:02:00"));
    assert_eq!(stm049[0].expected_value.as_deref(), Some("24:02:00"));
    assert!(!notices.iter().any(|n| n.rule_id == "STM_048"),
        "aynı satır departure olayı STM_048'e karışmamalı: {notices:?}");
}

#[test]
fn stm_048_and_stm_049_are_separate_high_spec_findings() {
    let files = with_opts(
        &[(
            "stop_times.txt",
            "trip_id,arrival_time,departure_time,stop_id,stop_sequence\n\
T1,23:50:00,23:50:00,S1,1\n\
T1,00:10:00,00:10:00,S2,2\n\
T2,23:59:00,00:02:00,S3,1\n\
T2,24:10:00,24:10:00,S4,2\n",
        )],
        &[],
        &[],
    );
    let notices = notices_for(&files, &ValidatorConfig::default());
    let stm048: Vec<_> = notices.iter().filter(|n| n.rule_id == "STM_048").collect();
    let stm049: Vec<_> = notices.iter().filter(|n| n.rule_id == "STM_049").collect();

    assert_eq!(stm048.len(), 1, "arrival rollover kendi finding'i olmalı: {stm048:?}");
    assert_eq!(stm049.len(), 1, "same-row departure rollover kendi finding'i olmalı: {stm049:?}");
    assert_eq!(stm048[0].field.as_deref(), Some("arrival_time"));
    assert_eq!(stm049[0].field.as_deref(), Some("departure_time"));
    assert_eq!(stm048[0].severity, gtfs_core::Severity::Yuksek);
    assert_eq!(stm049[0].severity, gtfs_core::Severity::Yuksek);
    assert_eq!(stm048[0].rule_class, gtfs_core::RuleClass::Spec);
    assert_eq!(stm049[0].rule_class, gtfs_core::RuleClass::Spec);
}

#[test]
fn each_raw_midnight_wrap_is_its_own_trip_finding() {
    let files = with_opts(
        &[
            (
                "trips.txt",
                "route_id,service_id,trip_id\nR1,SVC1,T1\nR1,SVC1,T2\nR1,SVC1,T3\nR1,SVC1,T4\n",
            ),
            (
                "stop_times.txt",
                "trip_id,arrival_time,departure_time,stop_id,stop_sequence\n\
T1,23:50:00,23:50:00,S1,1\nT1,00:10:00,00:10:00,S2,2\n\
T2,23:51:00,23:51:00,S1,1\nT2,00:11:00,00:11:00,S2,2\n\
T3,23:52:00,23:52:00,S1,1\nT3,00:12:00,00:12:00,S2,2\n\
T4,23:53:00,23:53:00,S1,1\nT4,00:13:00,00:13:00,S2,2\n",
            ),
        ],
        &[],
        &[],
    );
    let notices = notices_for(&files, &ValidatorConfig::default());
    let stm048: Vec<_> = notices.iter().filter(|n| n.rule_id == "STM_048").collect();
    assert_eq!(stm048.len(), 4, "dört raw olay dört ayrı finding olmalı: {stm048:?}");
    let trips: BTreeSet<_> = stm048.iter().filter_map(|n| n.entity_id.as_deref()).collect();
    assert_eq!(trips, BTreeSet::from(["T1", "T2", "T3", "T4"]));
    assert!(stm048.iter().all(|n| n.line.is_some() && n.details.is_some()));
}

#[test]
fn trip_starting_after_midnight_without_raw_wrap_is_silent() {
    let files = with_opts(
        &[(
            "stop_times.txt",
            "trip_id,arrival_time,departure_time,stop_id,stop_sequence\nT1,00:10:00,00:10:00,S1,1\nT1,00:20:00,00:20:00,S2,2\nT1,00:35:00,00:35:00,S1,3\n",
        )],
        &[],
        &[],
    );
    let notices = notices_for(&files, &ValidatorConfig::default());
    assert!(!notices.iter().any(|n| n.rule_id == "STM_048"), "gece yarısından başlayan trip STM_048 üretmemeli");
    assert!(!notices.iter().any(|n| n.rule_id == "STM_049"), "gece yarısından başlayan trip STM_049 üretmemeli");
    assert!(!notices.iter().any(|n| n.rule_id == "STM_008"), "monoton trip STM_008 üretmemeli");
}

#[test]
fn valid_service_day_rollover_does_not_emit_stm_048() {
    let files = with_opts(
        &[(
            "stop_times.txt",
            "trip_id,arrival_time,departure_time,stop_id,stop_sequence\nT1,23:50:00,23:50:00,S1,1\nT1,24:10:00,24:10:00,S2,2\n",
        )],
        &[],
        &[],
    );
    let notices = notices_for(&files, &ValidatorConfig::default());
    assert!(!notices.iter().any(|n| n.rule_id == "STM_048"), "valid 24:xx rollover işaretlenmemeli");
    assert!(!notices.iter().any(|n| n.rule_id == "STM_008"), "valid 24:xx rollover STM_008 üretmemeli");
}

#[test]
fn real_non_midnight_decrease_remains_stm_008_without_stm_048() {
    let files = with_opts(
        &[(
            "stop_times.txt",
            "trip_id,arrival_time,departure_time,stop_id,stop_sequence\nT1,08:00:00,08:00:00,S1,1\nT1,07:55:00,07:55:00,S2,2\n",
        )],
        &[],
        &[],
    );
    let notices = notices_for(&files, &ValidatorConfig::default());
    assert!(notices.iter().any(|n| n.rule_id == "STM_008"), "gerçek gün-içi geriye gidiş STM_008 üretmeli");
    assert!(!notices.iter().any(|n| n.rule_id == "STM_048"), "gün-içi geriye gidiş STM_048 olmamalı");
}

#[test]
fn stm008_carries_previous_departure_across_one_untimed_stop() {
    let files = with_opts(
        &[(
            "stop_times.txt",
            "trip_id,arrival_time,departure_time,stop_id,stop_sequence\nT1,08:00:00,08:00:00,S1,1\nT1,,,S2,2\nT1,07:55:00,07:55:00,S1,3\n",
        )],
        &[],
        &[],
    );
    let notices = notices_for(&files, &ValidatorConfig::default());
    let stm008: Vec<_> = notices.iter().filter(|n| n.rule_id == "STM_008").collect();
    assert_eq!(stm008.len(), 1, "untimed ara durak gerçek geriye gidişi gizlememeli: {stm008:?}");
    assert_eq!(stm008[0].line, Some(4), "finding sonraki zamanlı satırı göstermeli");
    assert_eq!(stm008[0].details.as_ref().and_then(|d| d.get("seq_a")).map(String::as_str), Some("1"));
    assert_eq!(stm008[0].details.as_ref().and_then(|d| d.get("seq_b")).map(String::as_str), Some("3"));
}

#[test]
fn stm008_carries_previous_departure_across_multiple_untimed_stops() {
    let files = with_opts(
        &[(
            "stop_times.txt",
            "trip_id,arrival_time,departure_time,stop_id,stop_sequence\nT1,08:00:00,08:00:00,S1,1\nT1,,,S2,2\nT1,,,S1,3\nT1,,,S2,4\nT1,07:55:00,07:55:00,S1,5\n",
        )],
        &[],
        &[],
    );
    let notices = notices_for(&files, &ValidatorConfig::default());
    assert_eq!(
        notices.iter().filter(|n| n.rule_id == "STM_008").count(),
        1,
        "birden çok untimed ara durak gerçek geriye gidişi gizlememeli"
    );
}

#[test]
fn stm008_does_not_flag_monotonic_interpolation_across_untimed_stop() {
    let files = with_opts(
        &[(
            "stop_times.txt",
            "trip_id,arrival_time,departure_time,stop_id,stop_sequence\nT1,08:00:00,08:00:00,S1,1\nT1,,,S2,2\nT1,08:05:00,08:05:00,S1,3\n",
        )],
        &[],
        &[],
    );
    let notices = notices_for(&files, &ValidatorConfig::default());
    assert!(!notices.iter().any(|n| n.rule_id == "STM_008"), "monoton interpolasyon false-positive üretmemeli");
}

#[test]
fn stm008_keeps_midnight_normalization_silent_across_untimed_stop() {
    let files = with_opts(
        &[(
            "stop_times.txt",
            "trip_id,arrival_time,departure_time,stop_id,stop_sequence\nT1,23:50:00,23:50:00,S1,1\nT1,,,S2,2\nT1,00:10:00,00:10:00,S1,3\n",
        )],
        &[],
        &[],
    );
    let notices = notices_for(&files, &ValidatorConfig::default());
    assert!(notices.iter().any(|n| n.rule_id == "STM_048"), "raw rollover STM_048 olarak korunmalı");
    assert!(!notices.iter().any(|n| n.rule_id == "STM_008"), "normalize edilen rollover STM_008'e çoğalmamalı");
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


// ── WP-2: alan düzeyinde spec çapası ──────────────────────────────────────────
// emit_proof "kural ateşleyebiliyor mu", spec_conformance.rs "geçerli veri sessiz mi"
// der. Bu üçüncü kapı sorar: **kural spec'te GERÇEKTEN VAR OLAN bir alanı mı ölçüyor?**
// Yakaladığı sınıf: yanlış SINIFLANDIRMA. RCT_004 (2026-08-01 öncesi) `min_age`/`max_age`
// yayıyordu; bu alanlar 27 Nisan 2026 Schedule Reference'ta HİÇ geçmiyor — yani bir uzantı
// alanı `Spec` sınıfındaydı ve yayın skoru kapısını besliyordu.
//
// Çapa kuralın KENDİ ÇIKTISINDAN gelir (notice'ın `file` + `field` alanları), elle tutulan
// bir listeden değil: elle liste bayatlar, notice bayatlayamaz.

/// Spec'te alan tablosu OLMAYAN dosyalar — çapa kurulamaz, atlanır.
/// Liste bilinçli olarak KISA tutulur: burada olmayan bilinmeyen bir dosya kapıyı düşürür
/// (ör. `Spec` otoriteli bir kural GTFS-JP dosyasına çapalanırsa sınıfı yanlıştır).
const NO_FIELD_TABLE: &[&str] = &[
    // GeoJSON; spec üyeleri (type/properties/geometry/coordinates) düzyazıyla anlatır,
    // "Field Name" tablosu yoktur. LOC_002/003/004/007/008/009/010 buraya düşer.
    "locations.geojson",
];

/// `file` alanında dosya adı olmayan sözde-değerler (feed düzeyi bağlam).
const PSEUDO_FILES: &[&str] = &["feed"];

#[test]
fn spec_rules_anchor_to_fields_that_exist_in_the_spec() {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../spec-audit/spec_fields.json");
    let raw = std::fs::read_to_string(&path)
        .unwrap_or_else(|e| panic!("{} okunamadı: {e}", path.display()));
    let doc: serde_json::Value = serde_json::from_str(&raw).unwrap();
    let table = doc["files"].as_object().expect("spec_fields.json: 'files' nesnesi yok");
    assert!(table.len() >= 25, "spec_fields.json çok küçük ({}) — yeniden üretin", table.len());

    let mut failures: BTreeSet<String> = BTreeSet::new();

    for f in fixtures() {
        let cfg = f.config.clone().unwrap_or_default();
        let vr = match validate_bytes(
            &make_zip(&with_opts(&f.overrides, &f.removes, &f.raw)), &cfg, TODAY,
        ) {
            ValidateResult::Ok(vr) => vr,
            ValidateResult::Fatal(_) => continue,
        };
        for n in &vr.notices {
            if gtfs_rules::registry::authority_source(&n.rule_id)
                != gtfs_core::AuthoritySource::GtfsSpec
            {
                continue;
            }
            // field yoksa çapalanacak bir alan da yok (dosya/feed düzeyi bulgu).
            let (Some(file), Some(field)) = (n.file.as_deref(), n.field.as_deref()) else {
                continue;
            };
            // "a.txt/b.txt" gibi çok-dosyalı etiketler bölünür.
            let parts: Vec<&str> = file.split('/').map(str::trim).collect();
            let known: Vec<&serde_json::Value> =
                parts.iter().filter_map(|p| table.get(*p)).collect();

            if known.is_empty() {
                if parts.iter().all(|p| NO_FIELD_TABLE.contains(p) || PSEUDO_FILES.contains(p)) {
                    continue;
                }
                failures.insert(format!(
                    "{}: '{}' spec dosya tablosunda YOK (GtfsSpec otoritesi bu dosyaya \
                     çapalanamaz — sınıf yanlış olabilir)",
                    n.rule_id, file,
                ));
                continue;
            }

            // Bileşik alan etiketleri ("stop_lat|stop_lon") tek tek denetlenir; alan,
            // etiketteki dosyalardan EN AZ BİRİNDE tanımlı olmalı.
            for one in field.split(['|', ',']).map(str::trim).filter(|s| !s.is_empty()) {
                let ok = known.iter().any(|entry| {
                    entry["fields"].as_array().unwrap().iter()
                        .any(|f| f["name"].as_str() == Some(one))
                });
                if !ok {
                    failures.insert(format!(
                        "{}: '{}' alanı '{}' içinde spec'te YOK",
                        n.rule_id, one, file,
                    ));
                }
            }
        }
    }

    assert!(
        failures.is_empty(),
        "GtfsSpec otoriteli kural(lar) spec'te olmayan bir alana çapalanıyor — sınıf \
         Quality/Interop olmalı ya da alan adı yanlış:\n{}\n\n\
         (Spec listesi: spec-audit/spec_fields.json — yeniden üretmek için \
         `python3 spec-audit/extract_fields.py`)",
        failures.iter().map(|s| format!("  {s}")).collect::<Vec<_>>().join("\n"),
    );
}

// ── WP-3: hüküm kapsamı (A = eksik hüküm · B = kısmi kapsam) ──────────────────
// WP-1 ve WP-2 KESİNLİK kapılarıdır: "yanlış yere hata atmıyoruz". Bu üçüncüsü
// KAPSAM sorar: "spec'in normatif hükmü var, bizim onu ölçen kuralımız var mı?"
//
// KAPI DEĞİL, DEFTER: kapsam boşluğu bugün "hata" değil (eksik kural kimseyi yanlış
// REDDETMEZ); ama SESSİZCE BÜYÜMEMELİ. `spec_coverage_ledger.txt` boşlukları sabitler;
// spec yeni alan ekleyince ya da bir kural kaldırılınca defter değişir ve test düşer.
//
// ══ ÖLÇÜMÜN ÜÇ SINIRI — SAYILARI AKTARIRKEN BUNLARI DA AKTAR ══════════════════
// 1. ALT SINIRDIR. Soru "kural var mı" değil, "emit_proof korpusunda bu file+field'a
//    Spec notice'ı çapalandı mı". Çok alanlı kural (CAL_002 yedi günü denetler, fixture
//    yalnız `monday`'i bozar) ve `field=None` üretenler (RTS_003) boşluk GİBİ görünür.
//    Doğru ifade: "32 alanın normatif hükmü var ama korpusta Spec çapası görünmüyor" —
//    "32 eksik kuralımız var" DEĞİL.
// 2. HÜKÜM ÜRETİMİ KABA. Yalnız alan tablosunun Presence/Type/PK sütunlarından türetilir.
//    KAPSAM DIŞI: düzyazı koşullar ("yalnız B=1 iken kullanılabilir", "ikisinden biri
//    zorunlu", "A ve B birlikte verilmeli", "şu dosya yoksa kullanılabilir"), dosyalar
//    arası koşullar, sefer içi tutarlılık, GeoJSON iç yapı şartları. Yani "192 hüküm
//    taşıyan alan" GTFS Spec'in TOPLAM normatif hüküm sayısı DEĞİLDİR.
// 3. `[yalnız: …]` HÜKÜM KARŞILANDI DEMEZ. Yalnız "bu kural bu alanda notice üretti"
//    der. Örnek: `route_text_color` → RTS_007 (hex format) + RTS_008 (kontrast). RTS_008
//    meşru bir Quality kuralıdır ama spec'in Color-tipi hükmünü KARŞILAMAZ. Satırın
//    sorduğu şey kapsam değil SINIFLANDIRMA'dır.

/// Bir alanın taşıdığı normatif hüküm sayısı (0 ise spec o alan için bir şey dayatmıyor).
/// `presence` + `type` sütunlarından türetilir; ikisi de spec'in KENDİ sözlüğüdür.
/// ⚠️ Yukarıdaki 2. sınır: düzyazı/çapraz-dosya hükümleri buradan çıkmaz.
fn provision_atoms(ftype: &str, presence: &str) -> Vec<&'static str> {
    let mut atoms = Vec::new();
    match presence {
        // "Optional"/"Recommended" bir şey DAYATMAZ → hüküm değil (STM_040 dersi).
        "Required" => atoms.push("presence:required"),
        "Conditionally Required" => atoms.push("presence:conditional"),
        "Conditionally Forbidden" => atoms.push("presence:conditional-forbidden"),
        _ => {}
    }
    if ftype.starts_with("Foreign ID") {
        atoms.push("foreign-key");
    }
    if ftype == "Unique ID" {
        atoms.push("unique");
    }
    match ftype {
        "Enum" => atoms.push("enum-domain"),
        // `Phone number` BİLİNÇLİ OLARAK YOK: spec'in tip tanımı birebir "A phone number." —
        // dilbilgisi dayatmıyor, dolayısıyla DENETLENEBİLİR bir hüküm de yok. Onu buraya
        // koymak defterde olmayan bir borç üretiyordu (agency_phone → AGN_007'yi "eksik"
        // gösteriyordu, oysa AGN_007'nin Quality olması doğru). Katı bir telefon kontrolü
        // geçerli uluslararası biçimleri reddeder.
        "Time" | "Local time" | "Date" | "Color" | "URL" | "Email"
        | "Language code" | "Timezone" | "Currency code" | "Currency amount"
        | "Latitude" | "Longitude" => atoms.push("format"),
        "Non-negative integer" | "Positive integer" | "Non-negative float"
        | "Positive float" | "Non-null integer" | "Non-zero integer" => atoms.push("range"),
        // Nitelenmemiş sayısal tipler bir ARALIK dayatmaz ama PARSE EDİLEBİLİRLİK dayatır:
        // "abc" bir Float değildir. Ayrı atom, çünkü `range` aralık kısıtını anlatır.
        // `Text`, `ID` ve `Text or URL or …` BİLİNÇLİ OLARAK YOK: spec bunlara herhangi bir
        // UTF-8 dizisi diyor, yani denetlenebilir bir hüküm yok (`Phone number` ile aynı ders).
        "Float" | "Integer" => atoms.push("numeric"),
        _ => {}
    }
    atoms
}

/// Alan bazında: hangi hükümler var · hangi `GtfsSpec` kuralları çapalı · hangi BAŞKA SINIF
/// kuralları çapalı.
///
/// Üçüncü küme kritik: `agency_phone`'u AGN_007 (Quality) ölçer, hiçbir Spec kuralı ölçmez.
/// Yalnız Spec'e bakan bir defter onu "denetimsiz" gösterir ve okuyanı VAR OLAN kuralı yeniden
/// yazmaya iter. Ölçüldü: 32 defter satırının 11'i bu durumda.
type CoverageRow = (String, String, Vec<&'static str>, BTreeSet<String>, BTreeSet<String>);

fn coverage_rows() -> Vec<CoverageRow> {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../spec-audit/spec_fields.json");
    let doc: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(&path).unwrap()).unwrap();
    let table = doc["files"].as_object().unwrap();

    // (dosya, alan) → çapalanan kurallar; Spec olanlar ve olmayanlar ayrı.
    type Anchors = std::collections::HashMap<(String, String), BTreeSet<String>>;
    let (mut spec, mut other): (Anchors, Anchors) = Default::default();
    for f in fixtures() {
        let cfg = f.config.clone().unwrap_or_default();
        let vr = match validate_bytes(
            &make_zip(&with_opts(&f.overrides, &f.removes, &f.raw)), &cfg, TODAY,
        ) {
            ValidateResult::Ok(vr) => vr,
            ValidateResult::Fatal(_) => continue,
        };
        for n in &vr.notices {
            let (Some(file), Some(field)) = (n.file.as_deref(), n.field.as_deref()) else {
                continue;
            };
            let bucket = if gtfs_rules::registry::authority_source(&n.rule_id)
                == gtfs_core::AuthoritySource::GtfsSpec { &mut spec } else { &mut other };
            for p in file.split('/').map(str::trim).filter(|p| table.contains_key(*p)) {
                for one in field.split(['|', ',']).map(str::trim).filter(|s| !s.is_empty()) {
                    bucket.entry((p.to_string(), one.to_string()))
                        .or_default().insert(n.rule_id.clone());
                }
            }
        }
    }

    let mut rows = Vec::new();
    for (file, entry) in table {
        for f in entry["fields"].as_array().unwrap() {
            let name = f["name"].as_str().unwrap().to_string();
            let atoms = provision_atoms(
                f["type"].as_str().unwrap_or(""), f["presence"].as_str().unwrap_or(""),
            );
            let key = (file.clone(), name.clone());
            rows.push((
                file.clone(), name,
                atoms,
                spec.get(&key).cloned().unwrap_or_default(),
                other.get(&key).cloned().unwrap_or_default(),
            ));
        }
    }
    rows.sort();
    rows
}

/// DENETİM aşamalarında (k2–k7) `"alan_adı"` literali geçiyor mu?
///
/// `k1_parse.rs` BİLİNÇLİ DIŞLANIR: `known_columns` orada spec'in HER resmî sütununu literal
/// olarak sayar (ARC_017 "bilinmeyen sütun" için). Onu da sayarsak ölçüt her alanı "kodda var"
/// gösterir ve hiçbir şey ayırt etmez — ölçtüm: dışlamadan yalnız 2, dışlayınca 5 kesin boşluk
/// çıkıyor (`booking_url`/`info_url`/`phone_number` YALNIZ sütun kaydında duruyor).
///
/// Geçmiyorsa hiçbir denetim o alana bakmıyor → boşluk KESİN. Geçiyorsa yalnız "olabilir":
/// literal başka bir dosya için konmuş olabilir (`from_stop_id` transfers/pathways'te var,
/// fare_leg_join_rules için değil). Ölçüt YÜKSEK KESİNLİK / düşük duyarlılıktır.
fn identifiers_in_check_stages() -> BTreeSet<String> {
    fn walk(dir: &std::path::Path, out: &mut Vec<std::path::PathBuf>) {
        for e in std::fs::read_dir(dir).unwrap().flatten() {
            let p = e.path();
            if p.is_dir() { walk(&p, out); }
            else if p.extension().is_some_and(|x| x == "rs")
                && p.file_name().is_some_and(|n| n != "k1_parse.rs") { out.push(p); }
        }
    }
    let mut files = Vec::new();
    walk(&std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src"), &mut files);

    let mut ids = BTreeSet::new();
    for f in files {
        let text = std::fs::read_to_string(&f).unwrap();
        for lit in text.split('"').skip(1).step_by(2) {
            if !lit.is_empty()
                && lit.len() <= 40
                && lit.bytes().all(|b| b.is_ascii_lowercase() || b == b'_' || b.is_ascii_digit())
            {
                ids.insert(lit.to_string());
            }
        }
    }
    ids
}

/// A kategorisi — normatif hükmü olan ama emit_proof korpusunda hiçbir `GtfsSpec` notice'ının
/// çapalanmadığı alanlar. Defterle eşleşmeli: boşluk kapanınca defter küçülür, spec büyüyünce
/// fark görünür.
///
/// ⚠️ **ALT SINIRDIR — "kural yok" DEMEK DEĞİLDİR.** Bir kural alanı ölçüyor ama fixture'ı
/// başka bir alanı bozuyorsa (CAL_002 yedi günü de denetler, fixture yalnız `monday`'i bozar)
/// ya da notice'ı `field=None` ile üretiyorsa (RTS_003, "ikisinden biri zorunlu") satır burada
/// çıkar. Bu yüzden defterde KANIT sütunu var: `[kaynakta-yok]` = alan adı üretim kaynağında
/// hiç geçmiyor → boşluk KESİN. `[yalnız: …]` = alanı ölçen kural VAR ama Spec sınıfı değil
/// (ör. `agency_phone` → AGN_007, Quality); önce "hüküm gerçekten Spec meselesi mi?" sorusu
/// cevaplanmalı, yeni kural yazılmamalı. Kanıtsız satır ADJUDİKASYON ister.
/// **Satıra yeni kural yazmadan önce mevcut kuralı ARA** (tekrar eden hata sınıfı).
#[test]
fn spec_coverage_gaps_match_ledger() {
    let ids = identifiers_in_check_stages();
    let gaps: Vec<String> = coverage_rows().into_iter()
        .filter(|(_, _, atoms, spec, _)| !atoms.is_empty() && spec.is_empty())
        .map(|(file, field, atoms, _, other)| {
            // Kanıt sırası: en kesinden en belirsize.
            let proof = if !other.is_empty() {
                // Alanı ölçen bir kural VAR, yalnız Spec sınıfı değil → yeni kural yazmadan
                // önce karar: hüküm gerçekten Spec meselesi mi, yoksa mevcut kural yeterli mi?
                format!("  [yalnız: {}]", other.iter().cloned().collect::<Vec<_>>().join(","))
            } else if !ids.contains(&field) {
                "  [denetim-yok]".to_string()
            } else {
                String::new()
            };
            format!("{file}:{field}  {}{proof}", atoms.join(","))
        })
        .collect();

    let ledger_path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests").join("spec_coverage_ledger.txt");
    let ledger_raw = std::fs::read_to_string(&ledger_path).unwrap_or_default();
    let ledger: Vec<String> = ledger_raw.lines().map(str::trim)
        .filter(|l| !l.is_empty() && !l.starts_with('#')).map(str::to_string).collect();

    if gaps != ledger {
        if std::env::var("UPDATE_LEDGER").is_ok() {
            let header = format!(
                "# WP-3/A — spec'te normatif hükmü olan ama emit_proof korpusunda hiçbir\n\
                 # GtfsSpec notice'ının ÇAPALANMADIĞI alanlar. Biçim: dosya:alan  hükümler [kanıt].\n\
                 # {} alan.\n\
                 #\n\
                 # Bu bir BORÇ defteridir, kapı değil: eksik kural kimseyi yanlış REDDETMEZ,\n\
                 # ama boşluk sessizce BÜYÜMEMELİ. Kural eklenince satır düşer.\n\
                 #\n\
                 # ⚠️ ALT SINIR — \"hiçbir kural yok\" DEMEK DEĞİL. Bir hüküm ölçülüyor olduğu\n\
                 # hâlde burada GÖRÜNEBİLİR: fixture kuralın yalnız bir dalını tetiklerse ya da\n\
                 # kural `field=None` emit ederse çapa düşmez. İkisi de 2026-08-02'de gerçekten\n\
                 # oldu ve İKİSİ DE GİDERİLDİ (aşağıya bak) — ama desen tekrar edebilir.\n\
                 # DOĞRU İFADE: \"bu alanların normatif hükmü var ama emit_proof korpusunda\n\
                 # bunlara bağlı bir Spec notice çapası görünmüyor\" — \"N eksik kuralımız var\" DEĞİL.\n\
                 #\n\
                 # ⚠️ HÜKÜM ÜRETİMİ KABA: yalnız alan tablosunun Presence/Type/PK sütunlarından\n\
                 # türetilir. Düzyazı koşullar (\"yalnız B=1 iken\", \"ikisinden biri zorunlu\",\n\
                 # \"A ve B birlikte\"), dosyalar arası koşullar, sefer içi tutarlılık ve GeoJSON\n\
                 # iç yapı şartları KAPSAM DIŞIDIR. Bu defter spec'in TÜM normatif hükümlerini\n\
                 # ölçmez, yalnız alan tablosundan türetilebilen bölümünü ölçer.\n\
                 #   [denetim-yok] = alan adı k2–k7 denetim aşamalarında hiç geçmiyor → boşluk KESİN\n\
                 #                   (k1_parse.rs dışlanır: known_columns her sütunu zaten sayar).\n\
                 #   [yalnız: …]   = alanı ölçen kural VAR, ama Spec sınıfı DEĞİL. Boşluk değil;\n\
                 #                   karar: hüküm Spec meselesi mi, mevcut kural yeterli mi?\n\
                 #                   ⚠️ Bu işaret \"hüküm KARŞILANDI\" DEMEZ — yalnız \"bu kural bu\n\
                 #                   alanda notice üretti\" der. Ör. route_text_color → RTS_007\n\
                 #                   (hex format) + RTS_008 (kontrast); RTS_008 meşru Quality'dir\n\
                 #                   ama spec'in Color-tipi hükmünü karşılamaz.\n\
                 #   kanıtsız satır  = ADJUDİKASYON gerekir; ÖNCE MEVCUT KURALI ARA.\n\
                 #\n\
                 # ── DURUM (2026-08-02): DEFTER BOŞ — ölçülebilen her hüküm çapalanıyor ────\n\
                 # Defter 32 satırla açıldı; 9'u KAPANDI (RTS_007 Spec'e taşındı;\n\
                 # AGN_012/RTS_024/TRN_008 sınıfı düzeltildi; `Phone number` hükmü uydurmaydı,\n\
                 # atom kaldırıldı → 3 satır düştü; attributions route_id/trip_id issue #62\n\
                 # ile ATR_011/ATR_012'ye ayrıldı → 2 satır düştü). Kalanların HER BİRİ incelendi:\n\
                 #   • booking_rules booking_url/info_url .. ✅ KAPANDI (BKR_020/021)\n\
                 #   • fare_leg_join_rules (4 alan) ........ ✅ KAPANDI (FLJ_001..004)\n\
                 #   • issue #61 ........................... TAMAMI KAPANDI (9 satır):\n\
                 #     STM_058 · DQ_021 genişletildi (attribution_id + route_networks.route_id)\n\
                 #     PTH_027 · STP_042 · NET_002 · NET_003 · RCT_007 · RTS_029 · TRN_015\n\
                 #   • calendar 6 gün (enum) ............... ✅ KAPANDI: artefakt olduğu\n\
                 #     doğrulandı ve GİDERİLDİ — fixture yedi AYRI servisle yedi günü de bozuyor\n\
                 #     (tek satır yetmez: CAL_002 dedup düzeyi Entity, servis başına tek notice).\n\
                 #   • routes route_short_name/route_long_name  ✅ KAPANDI: RTS_003 hükmü zaten\n\
                 #     ölçüyordu ama `field=None` emit ediyordu. Artık iki alanı da adlandırıyor\n\
                 #     (`route_short_name|route_long_name`) — repoda yerleşik boru konvansiyonu.\n\
                 #     Değişikliğin gerekçesi defter DEĞİL: R2\'nin \"Alan\" sütunu, feed\'i yayından\n\
                 #     alıkoyan bir bulguda boş kalıyordu.\n\
                 #\n\
                 # ⚠️ DEFTERİN BOŞ OLMASI \"SPEC\'İN TAMAMI ÖLÇÜLÜYOR\" DEMEK DEĞİLDİR — yalnız\n\
                 # ALAN TABLOSUNDAN türetilebilen hükümler için geçerlidir (yukarıdaki kapsam\n\
                 # uyarısına bak). YENİ bir satır çıkarsa ya spec değişmiştir ya bir kural\n\
                 # kalkmıştır ya da bir fixture zayıflamıştır — testi düşürür, adjudike edilmelidir.\n\
                 #\n\
                 # Yeniden üretmek: UPDATE_LEDGER=1 cargo test -p gtfs-pipeline --test emit_proof \\\n\
                 #   spec_coverage_gaps_match_ledger\n",
                gaps.len());
            std::fs::write(&ledger_path, format!("{header}{}\n", gaps.join("\n"))).unwrap();
            return;
        }
        let added: Vec<&String> = gaps.iter().filter(|g| !ledger.contains(g)).collect();
        let removed: Vec<&String> = ledger.iter().filter(|l| !gaps.contains(l)).collect();
        panic!(
            "spec kapsam defteri güncel değil ({} hesaplanan, {} defter).\n\
             YENİ boşluk (spec alan mı ekledi, kural mı kalktı?): {:#?}\n\
             Defterde fazla (boşluk KAPANDI → defter küçülmeli): {:#?}\n\
             Düzeltmek için: UPDATE_LEDGER=1 cargo test -p gtfs-pipeline --test emit_proof \
             spec_coverage_gaps_match_ledger",
            gaps.len(), ledger.len(), added, removed,
        );
    }
}

/// B kategorisi — RAPOR (kapı DEĞİL): hüküm sayısı çapalı kural sayısını aşan alanlar.
///
/// ⚠️ Bu bir SİNYAL, karar değil. Bir kural birden çok hükmü aynı anda ölçebilir
/// (ör. "monday geçersiz" hem varlığı hem enum kümesini kapsayabilir), o yüzden
/// "2 hüküm, 1 kural" satırlarının çoğu meşrudur. Kural başlıklarından hangi hükmün
/// ölçüldüğünü türetmeyi denedim: başlıkların yalnız %48'i sınıflanabiliyor
/// (487'nin 252'si hiçbir anahtar sözcüğe uymuyor) — bu güvenilirlikte bir
/// sınıflandırıcıyı repoya koymak, kaçınmaya çalıştığımız "bayatlayan elle liste"nin
/// başka biçimi olurdu. Liste bu yüzden İNSAN adjudikasyonu için kuyruktur.
///
/// ## ADJUDİKASYON (2026-08-02) — 62 satır 13'e indi
/// Önce ölçüm düzeltildi: `presence:required` atomu, alan **ARC_025'in zorunlu-sütun
/// listesindeyse** artık sayılmıyor. Gerekçe: ARC_025 eksik sütunu `field=<sütun>` ile emit
/// eder (fixture'ı `stops.txt:stop_id` üzerinden kanıtlar), mekanizma tek bir döngüdür ve liste
/// `required_columns_match_the_specification` ile spec'e kilitlidir. Bu 46 satırı düşürdü.
/// Ardından `STM_058`'in fixture'ı iki pencere alanını da bozacak şekilde güçlendirildi (−1).
///
/// **Kalan 13'ün triyajı — üç sınıf:**
/// 1. **ÖLÇÜM KÖRLÜĞÜ, boşluk YOK** — tek kural iki atomu birden karşılıyor, ölçüm alan başına
///    KURAL sayar, atom eşleştirmez: `FLJ_003`/`FLJ_004` (karşılıklı koşul + FK'yi birlikte
///    ölçerler, ben yazdım), `STM_058`↔`STM_039` çifti (biçim + varlık).
/// 2. **BAŞKA KURAL KARŞILIYOR ama çapalamıyor** — `routes.continuous_pickup/drop_off`'un
///    koşullu-yasak hükmünü `RTS_028` (Interop) ölçer; `stops.stop_access`'inkini `STP_027`.
///    İkisi de fixture'da o alanı çapalamıyor.
/// 3. **SPEC METNİ OKUNDU (2026-08-02) — 7 adayın 2'si KAPALI çıktı:**
///    - `timeframes.start_time` ✅ — *"Required if end_time is defined. Forbidden otherwise."*
///      `TFR_007` tam bunu ölçer ("start_time ve end_time yalnızca biri tanımlı").
///    - `trips.shape_id` ✅ — *"Required if the trip has a continuous pickup or drop-off
///      behavior."* `TRP_019` tam bunu ölçer.
///
///    **Kalan 5 GERÇEK BOŞLUK ADAYI:**
///    - `booking_rules.prior_notice_duration_max` · `prior_notice_start_day` — `numeric` atomu
///      denetlenmiyor: `opt_int()` `.parse::<i64>().ok()` kullanıyor, sayı olmayan değer sessiz
///      düşüyor. **Bu bir SİSTEMİK desenin parçası, aşağıya bak.**
///    - `fare_attributes.agency_id` — *"Required if multiple agencies are defined."* Kodda
///      karşılığı bulunamadı (`FAR_008` yalnız FK'yi ölçer).
///    - `stop_times.departure_time` — iki alt hükmü var: Flex penceresiyle YASAK (`STM_037` ✅)
///      ve *"Required for timepoint=1"* (karşılığı bulunamadı). ⚠️ Kalıcı incelik doğrulandı:
///      spec ilk/son durak zorunluluğunu YALNIZ `arrival_time` için yazar.
///    - `stop_times.location_group_id` — koşullu-yasak hükmü denetlenmiyor (`XFL_024` FK'dir).
///
/// ## 🔁 SİSTEMİK BULGU — sessiz parse yutması (12 nokta)
/// `Err(_) => None` / `.parse().ok()` deseni k2'de **12 yerde** duruyor ve 11'inin HEMEN
/// YANINDA o alanın kuralı var: `location_type`→STP_008 · `wheelchair_boarding`→STP_013 ·
/// `stop_access`→STP_024 · `timepoint`→STM_022 · `shape_dist_traveled`→STM_030/SHP_021 ·
/// `cemv_support`→AGN_012/RTS_024 · `wheelchair_accessible`→TRP_006 · `bikes_allowed`→TRP_007 ·
/// `exact_times`→FRQ_007 · booking_rules'un 4 alanı→`opt_int`.
/// **Sonuç: ARALIK DIŞI sayı yakalanıyor, SAYI OLMAYAN değer sessizce düşüyor.** Bu oturumda
/// aynı desen 5 kez tek tek düzeltildi (STM_058 · PTH_027 · RTS_029 · TRP_034 · ARC_031 turu);
/// 12'si birden düzeltilmeli — çoğunda yanındaki kuralı yeniden kullanmak yeter (PTH_027 emsali).
/// TERS YÖN — kapsam defterinin aynadaki görüntüsü.
///
/// Defter "her hüküm ölçülüyor mu" diye sorar; bu rapor "her Spec iddiası bir hükme dayanıyor
/// mu" diye sorar. İkinci soru DAHA PAHALI bir hatayı arar: kapsam boşluğu kullanıcıya eksik
/// bilgi verir, yanlış Spec iddiası ise GEÇERLİ BİR FEED'İ YAYINDAN ALIKOYAR (R1 kapısı saf
/// `Spec ∧ Kritik`).
///
/// Ölçüt: `Spec` sınıfı bir kural, spec'in HİÇBİR hüküm atomu üretmediği bir alana çapalanıyor
/// mu? Alan `Optional`/`Recommended` ve tipi bir şey dayatmıyorsa, o alanda "spec böyle diyor"
/// demek dayanaksızdır — STM_040 tam olarak buydu (Optional alanda Spec sınıfı, Quality'ye
/// taşındı).
///
/// KAPI DEĞİL, RAPOR: meşru örnekler olabilir (kuralın ölçtüğü hüküm alan tablosunda değil
/// düzyazıda tanımlıdır). Her satır insan adjudikasyonu ister.
/// İKİ DEFTERİN DE ORTAK KÖR NOKTASI: `field: None` emit eden kurallar.
///
/// Kapsam defteri de iddia defteri de çapayı `(file, field)` üzerinden kurar. Bir kural
/// `field` yazmıyorsa HİÇBİRİNDE görünmez: ne "bu hüküm ölçülüyor" der, ne "bu iddia
/// dayanaksız" denetimine girer. `RTS_003` tam bu yüzden defterde "boşluk" görünüyordu.
///
/// **Bu defter yalnız Entity/Row düzeyini tutar.** Dosya/feed düzeyi bulgularda (`DedupLevel::
/// Feed`/`File`) tek bir alan yoktur ve `None` doğrudur — onlar sayılır ama listelenmez.
///
/// ## ADJUDİKASYON (2026-08-02) — iki sınıf
/// **(A) TÜRETİLMİŞ OLGU → `None` DOĞRU.** Bulgu tek bir alanın değerinden değil, satırlar
/// arası bir ilişkiden/istatistikten doğar: "hiç sefer geçmeyen durak" (STP_020) `stops.txt`'te
/// yanlış bir alan göstermez, `stop_times` ile kurulan ilişkiden gelir. Bir alan adı yazmak
/// kullanıcıyı yanlış yere bakmaya iter. Bu defterdeki satırların ÇOĞU budur:
/// analiz (OPR_*, CAL_007/010/012/021, CLD_007, VAT_008, PTH_012/013, STP_020),
/// graf/istatistik (RTS_016, STP_030, TRP_013, ATR_003'ün karşıtı değil).
///
/// **(B) SATIRIN ŞEKLİ → `None` DOĞRU.** `ARC_012` (sütun sayısı başlıkla uyuşmuyor) ve
/// `ARC_018` (boş veri satırı) satırın kendisiyle ilgilidir, herhangi bir alanla değil.
///
/// **(C) ÇAPRAZ DOSYA → `None` ZORUNLU.** `RCT_006` ihlali `rider_categories.
/// is_default_fare_category`'dedir ama notice `fare_products.txt`'i gösterir. O dosyada olmayan
/// bir alan adı yazmak `spec_rules_anchor_to_fields_that_exist_in_the_spec` kapısını HAKLI
/// olarak düşürür. Doğru çözüm notice'ın dosyasını değiştirmek olurdu — ayrı bir karar.
///
/// ## 2026-08-02'de DÜZELTİLENLER (belirli alanların ihlaliydi, `None` yanlıştı)
/// `RTS_003` · `ATR_003` · `PTH_016` · `TRF_012` · `TRF_016` · `TRN_005` · `TRN_006` ·
/// `TRN_009` · `TRN_013` · `ATR_009` · `CAL_006` · `CAL_018` · `FRL_007` · `GEO_020` · `PTH_011`
///
/// ## Sınır
/// `PROOF_ALLOWLIST`'teki kurallar (fatal yola gidenler) hiç notice üretmediği için burada
/// GÖRÜNMEZ — ayrı bir kör nokta, ayrı iş.
#[test]
fn field_none_emitters_match_ledger() {
    use gtfs_core::DedupLevel;
    let mut with_field: BTreeSet<String> = BTreeSet::new();
    let mut without_field: BTreeSet<String> = BTreeSet::new();
    for f in fixtures() {
        let cfg = f.config.clone().unwrap_or_default();
        let vr = match validate_bytes(
            &make_zip(&with_opts(&f.overrides, &f.removes, &f.raw)), &cfg, TODAY,
        ) {
            ValidateResult::Ok(vr) => vr,
            ValidateResult::Fatal(_) => continue,
        };
        for n in &vr.notices {
            if n.field.is_some() { with_field.insert(n.rule_id.clone()); }
            else { without_field.insert(n.rule_id.clone()); }
        }
    }
    let mut found: Vec<String> = Vec::new();
    for id in without_field.iter().filter(|r| !with_field.contains(*r)) {
        let Some(m) = gtfs_rules::registry::get_rule(id) else { continue };
        if matches!(m.dedup_level, DedupLevel::Feed | DedupLevel::File) { continue; }
        found.push(format!("{id}  {:?}", m.rule_class));
    }
    found.sort_unstable();

    let ledger_path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests").join("field_none_ledger.txt");
    let ledger_raw = std::fs::read_to_string(&ledger_path).unwrap_or_default();
    let ledger: Vec<String> = ledger_raw.lines().map(str::trim)
        .filter(|l| !l.is_empty() && !l.starts_with('#')).map(str::to_string).collect();

    if found != ledger {
        if std::env::var("UPDATE_LEDGER").is_ok() {
            let header = format!(
                "# Entity/Row düzeyinde olup HER ZAMAN `field: None` emit eden kurallar. {} kural.\n\
                 # Biçim: RULE_ID  sınıf\n\
                 #\n\
                 # Kapsam defteri ve iddia defteri çapayı `(file, field)` üzerinden kurar; bu\n\
                 # kurallar İKİSİNDE DE GÖRÜNMEZ. Dosya/feed düzeyi bulgular (DedupLevel Feed/\n\
                 # File) burada SAYILMAZ — orada tek bir alan yoktur ve None doğrudur.\n\
                 #\n\
                 # ⚠️ SATIR OLMASI HATA DEĞİLDİR. Adjudikasyon testin doc yorumunda; üç meşru\n\
                 # sınıf var: (A) türetilmiş olgu — bulgu satırlar arası ilişkiden doğar, bir\n\
                 # alan adı kullanıcıyı yanlış yere baktırır; (B) satırın şekli (ARC_012/018);\n\
                 # (C) çapraz dosya — alan başka dosyada, yazmak çapa kapısını düşürür (RCT_006).\n\
                 #\n\
                 # YENİ bir satır çıkarsa: kural gerçekten türetilmiş bir olgu mu ölçüyor, yoksa\n\
                 # belirli alanların ihlalini mi anlatıyor? İkincisiyse BORU KONVANSİYONU kullan\n\
                 # (`a|b`), None bırakma — R2'nin \"Alan\" sütunu boş kalır.\n\
                 #\n\
                 # Yeniden üretmek: UPDATE_LEDGER=1 cargo test -p gtfs-pipeline --test emit_proof \\\n\
                 #   field_none_emitters_match_ledger\n",
                found.len());
            std::fs::write(&ledger_path, format!("{header}{}\n", found.join("\n"))).unwrap();
            return;
        }
        let added: Vec<&String> = found.iter().filter(|f| !ledger.contains(f)).collect();
        let removed: Vec<&String> = ledger.iter().filter(|l| !found.contains(l)).collect();
        panic!(
            "field=None defteri güncel değil ({} hesaplanan, {} defter).\n\
             YENİ alansız kural (türetilmiş olgu mu, yoksa boru konvansiyonu mu gerekiyor?): {:#?}\n\
             Defterde fazla (alan yazmaya başladı): {:#?}\n\
             Düzeltmek için: UPDATE_LEDGER=1 cargo test -p gtfs-pipeline --test emit_proof \
             field_none_emitters_match_ledger",
            found.len(), ledger.len(), added, removed,
        );
    }
}

#[test]
fn spec_claims_without_a_provision_match_ledger() {
    let rows = coverage_rows();
    let mut found: Vec<String> = rows.iter()
        .filter(|(_, _, atoms, spec_rules, _)| atoms.is_empty() && !spec_rules.is_empty())
        .map(|(file, field, _, spec_rules, _)| {
            let mut rs: Vec<String> = spec_rules.iter().cloned().collect();
            rs.sort();
            format!("{file}:{field}  {}", rs.join(","))
        })
        .collect();
    found.sort_unstable();

    let ledger_path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests").join("spec_claims_ledger.txt");
    let ledger_raw = std::fs::read_to_string(&ledger_path).unwrap_or_default();
    let ledger: Vec<String> = ledger_raw.lines().map(str::trim)
        .filter(|l| !l.is_empty() && !l.starts_with('#')).map(str::to_string).collect();

    if found != ledger {
        if std::env::var("UPDATE_LEDGER").is_ok() {
            let header = format!(
                "# TERS YÖN — kapsam defterinin aynadaki görüntüsü. {} satır.\n\
                 # Biçim: dosya:alan  kural[,kural]\n\
                 #\n\
                 # Kapsam defteri \"her hüküm ölçülüyor mu\" diye sorar. Bu defter tersini sorar:\n\
                 # \"her Spec İDDİASI bir hükme dayanıyor mu?\" İkinci soru DAHA PAHALI bir hatayı\n\
                 # arar — kapsam boşluğu kullanıcıya eksik bilgi verir, yanlış Spec iddiası ise\n\
                 # GEÇERLİ BİR FEED'İ YAYINDAN ALIKOYAR (R1 kapısı saf `Spec ∧ Kritik`).\n\
                 #\n\
                 # Ölçüt: `Spec` sınıfı bir kural, alan tablosunun HİÇBİR hüküm atomu üretmediği\n\
                 # bir alana çapalanıyor mu? Alan Optional/Recommended ve tipi (Text/ID/Phone\n\
                 # number gibi) bir şey dayatmıyorsa, orada \"spec böyle diyor\" demek dayanaksızdır.\n\
                 # Emsal: STM_040 (Optional alanda Spec sınıfı → Quality'ye taşındı),\n\
                 # PTH_017/PTH_028 (spec \"should\" der → tavsiye dalı Quality'ye ayrıldı).\n\
                 #\n\
                 # ⚠️ SATIR OLMASI HER ZAMAN HATA DEĞİLDİR: hüküm alan tablosunda değil DÜZYAZIDA\n\
                 # tanımlı olabilir. Ama her satır spec metniyle DOĞRULANMALI; doğrulanamayan\n\
                 # kuralın sınıfı Quality olmalıdır.\n\
                 #\n\
                 # ⚠️ ALT SINIR, iki yönden: (1) `field=None` emit eden kurallar görünmez;\n\
                 # (2) yalnız \"hiç atom yok\" hâli yakalanır — alanın BİR hükmü varsa ve kural\n\
                 # BAŞKA bir şeyi dayatıyorsa görünmez (kapsam defterinin atom-körlüğünün aynası).\n\
                 #\n\
                 # Yeniden üretmek: UPDATE_LEDGER=1 cargo test -p gtfs-pipeline --test emit_proof \\\n\
                 #   spec_claims_without_a_provision_match_ledger\n",
                found.len());
            std::fs::write(&ledger_path, format!("{header}{}\n", found.join("\n"))).unwrap();
            return;
        }
        let added: Vec<&String> = found.iter().filter(|f| !ledger.contains(f)).collect();
        let removed: Vec<&String> = ledger.iter().filter(|l| !found.contains(l)).collect();
        panic!(
            "spec iddia defteri güncel değil ({} hesaplanan, {} defter).\n\
             YENİ dayanaksız Spec iddiası (spec metniyle DOĞRULA — yoksa Quality olmalı): {:#?}\n\
             Defterde fazla (iddia kalktı/sınıf düzeldi): {:#?}\n\
             Düzeltmek için: UPDATE_LEDGER=1 cargo test -p gtfs-pipeline --test emit_proof \
             spec_claims_without_a_provision_match_ledger",
            found.len(), ledger.len(), added, removed,
        );
    }
}

#[test]
#[ignore = "rapor: cargo test -p gtfs-pipeline --test emit_proof spec_partial -- --ignored --nocapture"]
fn spec_partial_coverage_report() {
    let rows = coverage_rows();
    let total: usize = rows.iter().map(|(_, _, a, _, _)| a.len()).sum();
    let (mut none, mut partial, mut full) = (0usize, 0usize, 0usize);
    println!("\n### B — hüküm sayısı > çapalı kural sayısı (SİNYAL, karar değil)");
    println!("`presence:required` atomu, alan ARC_025'in zorunlu-sütun listesindeyse SAYILMAZ:");
    println!("kural eksik sütunu `field=<sütun>` ile emit eder, mekanizma tek döngüdür ve liste");
    println!("`required_columns_match_the_specification` ile spec'e kilitlidir.\n");
    for (file, field, atoms, rules, _) in &rows {
        if atoms.is_empty() { continue; }
        // Beyan edilmiş çapa kaynağı: ARC_025'in zorunlu-sütun listesi.
        let declared_presence = gtfs_pipeline::k1_parse::required_fields(file)
            .contains(&field.as_str());
        let atoms: Vec<&str> = atoms.iter().copied()
            .filter(|a| !(*a == "presence:required" && declared_presence))
            .collect();
        if atoms.is_empty() { full += 1; continue; }
        if rules.is_empty() { none += 1; }
        else if rules.len() < atoms.len() {
            partial += 1;
            println!("  {file}:{field} — [{}] · {:?}", atoms.join(","), rules);
        } else { full += 1; }
    }
    println!("\n### ÖZET ({} hüküm taşıyan alan · {total} hüküm atomu):", none + partial + full);
    println!("  Spec çapası YOK ....... {none}   (defterde; \"eksik kural\" DEĞİL — bkz. başlık)");
    println!("  hüküm > çapalı kural .. {partial}   (sinyal, karar değil)");
    println!("  hüküm <= çapalı kural . {full}");
    println!("\n⚠️ Bu sayılar SPEC'İN TOPLAM NORMATİF HÜKMÜ DEĞİLDİR: yalnız alan tablosunun");
    println!("   Presence/Type/PK sütunlarından türetilen hükümlerdir. Kapsam DIŞI kalanlar:");
    println!("   düzyazı koşullar (\"yalnız B=1 iken\", \"ikisinden biri zorunlu\", \"A ve B birlikte\"),");
    println!("   dosyalar arası koşullar, sefer içi tutarlılık, GeoJSON iç yapı şartları.");
}

/// RAPOR (kapı DEĞİL): çapa ALAN düzeyindedir, ATOM düzeyinde değil.
///
/// `spec_coverage_gaps_match_ledger` bir alanda EN AZ BİR Spec notice görürse o alanı
/// "kapsanmış" sayar. Ama bir alanın birden çok hüküm atomu olabilir (`presence:required`,
/// `format` ve `foreign-key`) ve tek bir notice hepsini birden çapalar. Bu rapor, o
/// kabalığın BÜYÜKLÜĞÜNÜ ölçer: kaç atom, kaç alan, ve kaç alanda atom sayısı onu çapalayan
/// farklı kural sayısını AŞIYOR.
///
/// Aşan alan "kanıtsız" DEMEK DEĞİL — tek kural birden çok atomu meşru olarak ölçebilir
/// (`STP_003` hem `presence` hem `format` bakar). Şüphe listesidir, borç değil.
#[test]
#[ignore = "rapor: cargo test -- --ignored anchor_granularity_report --nocapture"]
fn anchor_granularity_report() {
    let rows = coverage_rows();
    let (mut atoms, mut fields_with_atoms, mut suspect) = (0usize, 0usize, Vec::new());
    for (file, field, provs, spec_rules, other_rules) in &rows {
        if provs.is_empty() { continue; }
        atoms += provs.len();
        fields_with_atoms += 1;
        if provs.len() > spec_rules.len() {
            suspect.push((file.clone(), field.clone(), provs.clone(),
                          spec_rules.clone(), other_rules.clone()));
        }
    }
    suspect.sort_by_key(|(f, n, p, s, _)| (std::cmp::Reverse(p.len() - s.len()), f.clone(), n.clone()));
    println!("\n=== ÇAPA GRANÜLERLİĞİ ===");
    println!("hüküm atomu           : {atoms}");
    println!("atomu olan alan       : {fields_with_atoms}");
    println!("atom > Spec kuralı    : {} alan", suspect.len());
    let gap: usize = suspect.iter().map(|(_, _, p, s, _)| p.len() - s.len()).sum();
    println!("kapatılmamış atom farkı: {gap}");
    println!("\n--- en büyük 20 fark ---");
    for (file, field, provs, spec_rules, other) in suspect.iter() {
        let titles: Vec<String> = spec_rules.iter()
            .map(|r| format!("{r}={}", gtfs_rules::registry::get_rule(r).map(|x| x.title).unwrap_or("?")))
            .collect();
        println!("{file}:{field}\t{:?}\t{}\t{:?}", provs, titles.join(" | "), other);
    }
}

/// `FILE_LEVEL_PROVISIONS.md`, spec'in DOSYA düzeyi koşullu hükümlerinin elle adjudikasyonudur
/// ve hiçbir kapıya bağlı DEĞİLDİ.
///
/// Bu hükümler alan tablosundan türemez, dolayısıyla kapsam defteri onları görmez; düzyazı
/// kataloğu da `File: …` satırlarını BİLİNÇLİ dışarıda bırakır (`extract_provisions.py`,
/// gerekçe: bu dosyada zaten adjudike edildiler). Yani yedi hüküm YALNIZ o belgede yaşıyor.
///
/// Belgenin kendisi şunu yazıyordu: "Bu ölçüm otomatik DEĞİLDİR — kimse bu adımı
/// hatırlatmaz." Bu test o cümleyi geçersiz kılar: spec'e yeni bir koşullu dosya eklenir
/// ya da mevcut biri koşulsuzlaşırsa, belge güncellenene kadar kırmızı yanar.
#[test]
fn file_level_provisions_doc_covers_every_conditional_file() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let doc: serde_json::Value = serde_json::from_str(
        &std::fs::read_to_string(root.join("spec-audit/spec_fields.json")).unwrap()).unwrap();
    let md = std::fs::read_to_string(root.join("spec-audit/FILE_LEVEL_PROVISIONS.md")).unwrap();

    let mut missing = Vec::new();
    let mut conditional = 0usize;
    for (file, entry) in doc["files"].as_object().unwrap() {
        let presence = entry["presence"].as_str().unwrap_or("");
        if !presence.starts_with("Conditionally") {
            continue;
        }
        conditional += 1;
        if !md.contains(&format!("`{file}`")) {
            missing.push(format!("{file} ({presence})"));
        }
    }
    assert!(
        missing.is_empty(),
        "spec'te koşullu ama FILE_LEVEL_PROVISIONS.md'de adjudike EDİLMEMİŞ {} dosya:\n  {}\n\n\
         Bu hükümler başka hiçbir defterde yok. Belgeyi güncelleyin: koşul metnini, karşılayan \
         kuralı ve kararı yazın (mevcut yedi satırın biçimi örnek).",
        missing.len(), missing.join("\n  ")
    );
    assert!(
        conditional >= 7,
        "spec_fields.json'da yalnız {conditional} koşullu dosya var; 2026-08-02'de yediydi. \
         Bir dosya koşulsuzlaştıysa FILE_LEVEL_PROVISIONS.md'deki ilgili satır da düşmeli."
    );
}

/// Defterin ÖZET bölümleri, DURUM MAKİNESİ tablosuyla çelişmemeli.
///
/// `PROVISION_TRIAGE.md` 900 satırı aşıyor ve tur bölümleri o günkü kararları taşıyor.
/// Oturum içinde iki kez oldu: bir boşluk kapatıldı, kural yazıldı, ama özet tablo hâlâ
/// "AÇIK" diyordu — ve `badge_status.py` durum makinesini okuduğu için yüzde ile belge
/// birbirini tutmuyordu. Bu test o sapmayı yakalar.
#[test]
fn triage_ledger_has_no_stale_open_claims() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let md = std::fs::read_to_string(root.join("spec-audit/PROVISION_TRIAGE.md")).unwrap();

    // Durum makinesinde kapanmış sayılan adaylar
    let start = md.find("## 📊 DURUM MAKİNESİ").expect("DURUM MAKİNESİ bölümü yok");
    let end = md[start..].find("**KISMİ adjudikasyonu").map(|i| start + i).unwrap_or(md.len());
    let closed: Vec<&str> = md[start..end]
        .lines()
        .filter(|l| l.starts_with('|') && l.contains('`'))
        .flat_map(|l| l.split('`').skip(1).step_by(2))
        .filter(|s| s.len() == 9 && s.starts_with('P'))
        .collect();
    assert!(closed.len() >= 12, "durum makinesi çok kısa: {} kayıt", closed.len());

    // Kapanmış bir aday, "AÇIK" ya da "KAPATILMAYACAK" diyen bir satırda geçmemeli
    let mut stale = Vec::new();
    for line in md.lines() {
        if !line.contains("AÇIK") && !line.contains("KAPATILMAYACAK") {
            continue;
        }
        for id in &closed {
            if line.contains(&format!("`{id}`")) {
                stale.push(format!("{id}: {}", line.trim().chars().take(90).collect::<String>()));
            }
        }
    }
    assert!(
        stale.is_empty(),
        "durum makinesinde KAPALI olan {} aday, defterde hâlâ AÇIK görünüyor:\n  {}\n\n\
         Kapanışı tek yerde tutun; özet satırını da güncelleyin.",
        stale.len(), stale.join("\n  ")
    );
}


/// Defterin BAŞLIK SAYAÇLARI katalogla ve hesaplanan yüzdeyle tutmalı.
///
/// ⚠️ 2026-08-06 DIŞ DENETİMİ: `triage_ledger_has_no_stale_open_claims` yalnız AÇIK/KAPALI
/// etiketlerini denetliyordu. Denetçi defterin üst kısmının hâlâ "273 aday · 131/131"
/// dediğini, alt kısmının ise "299 · 146/146" dediğini gösterdi — aynı dosya kendi içinde
/// tutarsızdı ve bunun için yazılmış bir kapı VARDI ama sayaçları kapsamıyordu.
///
/// Bu test iki şeyi bağlar: (a) defterdeki katalog toplamı `spec_provisions.json` ile,
/// (b) paya sayılan (KANITLI/DOLAYLI) hiçbir satır "[Varsayım]" taşımamalı — test
/// edilmemiş bir çıkarım "ölçülmüş" sayılamaz.
#[test]
fn ledger_header_counts_match_catalogue() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let md = std::fs::read_to_string(root.join("spec-audit/PROVISION_TRIAGE.md")).unwrap();
    let json = std::fs::read_to_string(root.join("spec-audit/spec_provisions.json")).unwrap();

    // Katalogdaki aday sayısı: `"id": "P…"` satırlarını say (bağımlılıksız).
    let total = json.matches("\"id\":").count();
    assert!(total > 200, "katalog okunamadı ({total})");
    let head: String = md.lines().take(20).collect::<Vec<_>>().join("\n");
    assert!(
        head.contains(&format!("**{total} aday**")),
        "defterin başlığı katalogla tutmuyor: katalogda {total} aday var ama ilk 20 satırda \
         '**{total} aday**' geçmiyor. Katalog büyüdüğünde bu satır ELLE güncellenmeli.\n\
         Başlık:\n{head}"
    );

    // Paya sayılan satırlarda test edilmemiş varsayım olmamalı.
    let bad: Vec<&str> = md
        .lines()
        .filter(|l| l.starts_with('|'))
        .filter(|l| l.contains("KANITLI") || l.contains("DOLAYLI"))
        .filter(|l| l.contains("Varsayım") || l.contains("varsayım"))
        .collect();
    assert!(
        bad.is_empty(),
        "paya (KANITLI+DOLAYLI) sayılan {} satır 'varsayım' kelimesi taşıyor — ya ÖLÇ ya \
         KISMİ'ye indir. ⚠️ Kapı bilinçli olarak KATI: kelime yalnız CANLI bir varsayımı \
         anlatmak için kullanılmalı, tarihsel notta bile geçmemeli (aksi hâlde kapı \
         ayırt edemez ve zekileştirilirse güvenilmez olur):\n  {}",
        bad.len(),
        bad.join("\n  ")
    );
}


// Full-catalog regression: transfer_duration=0 is a valid non-negative integer.
#[test]
fn full_catalog_far006_zero_is_valid_non_negative() {
    let files = with_opts(
        &[("fare_attributes.txt", "fare_id,price,currency_type,payment_method,transfer_duration\nF1,2.5,USD,0,0\n")],
        &[], &[],
    );
    let notices = notices_for(&files, &ValidatorConfig::default());
    assert!(!notices.iter().any(|n| n.rule_id == "FAR_006"));
}

// Full-catalog regression: FRQ_001 owns a missing frequency trip FK; TRP_017
// remains for a real trip that has no stop_times rows.
#[test]
fn full_catalog_trp017_requires_an_existing_trip() {
    let missing_trip = with_opts(
        &[("frequencies.txt", "trip_id,start_time,end_time,headway_secs\nMISSING,08:00:00,10:00:00,600\n")],
        &[], &[],
    );
    let notices = notices_for(&missing_trip, &ValidatorConfig::default());
    assert!(notices.iter().any(|n| n.rule_id == "FRQ_001"));
    assert!(!notices.iter().any(|n| n.rule_id == "TRP_017"));

    let known_without_stop_times = with_opts(
        &[
            ("trips.txt", "route_id,service_id,trip_id\nR1,SVC1,T1\nR1,SVC1,T2\n"),
            ("frequencies.txt", "trip_id,start_time,end_time,headway_secs\nT2,08:00:00,10:00:00,600\n"),
        ],
        &[], &[],
    );
    let notices = notices_for(&known_without_stop_times, &ValidatorConfig::default());
    assert!(!notices.iter().any(|n| n.rule_id == "FRQ_001"));
    assert!(notices.iter().any(|n| n.rule_id == "TRP_017"));
}
