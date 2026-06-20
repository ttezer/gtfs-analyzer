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

use gtfs_pipeline::{validate_bytes, ValidateResult, ValidatorConfig};
use gtfs_rules::RULES;

const TODAY: u32 = 20_260_515;

// ── Geçerli temel feed (integration.rs ile aynı; tek başına notice üretmez) ─────
const AGENCY: &str = "agency_id,agency_name,agency_url,agency_timezone\n1,Test,http://test.example,UTC\n";
const STOPS: &str = "stop_id,stop_name,stop_lat,stop_lon\nS1,Stop1,41.0,29.0\nS2,Stop2,41.1,29.1\n";
const ROUTES: &str = "route_id,agency_id,route_short_name,route_type\nR1,1,101,3\n";
const TRIPS: &str = "route_id,service_id,trip_id\nR1,SVC1,T1\n";
const STOP_TIMES: &str = "trip_id,arrival_time,departure_time,stop_id,stop_sequence\nT1,08:00:00,08:00:00,S1,1\nT1,08:10:00,08:10:00,S2,2\n";
const CALENDAR: &str = "service_id,monday,tuesday,wednesday,thursday,friday,saturday,sunday,start_date,end_date\nSVC1,1,1,1,1,1,0,0,20250101,20271231\n";

fn base() -> Vec<(String, String)> {
    [
        ("agency.txt", AGENCY), ("stops.txt", STOPS), ("routes.txt", ROUTES),
        ("trips.txt", TRIPS), ("stop_times.txt", STOP_TIMES), ("calendar.txt", CALENDAR),
    ].iter().map(|(n, c)| (n.to_string(), c.to_string())).collect()
}

/// Temel feed'i alır, override'ları uygular (yeni dosya da ekler) ve `removes`'taki
/// dosyaları çıkarır (örn. "agency.txt eksik" senaryosu).
fn with_opts(overrides: &[(&str, &str)], removes: &[&str]) -> Vec<(String, String)> {
    let mut files = base();
    for (name, content) in overrides {
        if let Some(slot) = files.iter_mut().find(|(n, _)| n == name) {
            slot.1 = content.to_string();
        } else {
            files.push((name.to_string(), content.to_string()));
        }
    }
    files.retain(|(n, _)| !removes.contains(&n.as_str()));
    files
}

fn make_zip(files: &[(String, String)]) -> Vec<u8> {
    let cur = std::io::Cursor::new(Vec::new());
    let mut zw = zip::ZipWriter::new(cur);
    for (name, data) in files {
        zw.start_file(name, SimpleFileOptions::default()).unwrap();
        zw.write_all(data.as_bytes()).unwrap();
    }
    zw.finish().unwrap().into_inner()
}

fn emitted_rules(files: &[(String, String)]) -> BTreeSet<String> {
    match validate_bytes(&make_zip(files), &ValidatorConfig::default(), TODAY) {
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
}

fn fx(rule: &'static str, overrides: Vec<(&'static str, &'static str)>) -> Fixture {
    Fixture { rule, overrides, removes: Vec::new() }
}

/// Dosya çıkarmalı fixture ("X.txt eksik" senaryoları).
fn fx_rm(rule: &'static str, overrides: Vec<(&'static str, &'static str)>, removes: Vec<&'static str>) -> Fixture {
    Fixture { rule, overrides, removes }
}

/// Notice olarak emit edilmeyen kurallar (fatal yol veya dinamik) — proof'tan muaf.
const PROOF_ALLOWLIST: &[&str] = &[
    "ARC_001", // fatal FatalCode::ZipUnreadable (Notice değil)
];

/// rule_id → tetikleyici fixture. Kademeli doldurulur; her satır gerçek runtime proof.
fn fixtures() -> Vec<Fixture> {
    vec![
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
        // NOT (#5 bulgu): AGN_001 "agency.txt eksik" başlıklı ama agency.txt eksikliğini
        // ARC_004 (Fatal NoRequiredFiles) ele alıyor; AGN_001 hiçbir yolla emit edilmiyor
        // (emit_coverage'da da statik-görünmez). Muhtemelen ölü kural — fixture yazılamaz,
        // coverage_debt'te bırakıldı; registry temizliği ayrı/onaylı bir iş.

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
        fx("RTS_009", vec![("routes.txt", "route_id,agency_id,route_short_name,route_long_name,route_type\nR1,1,ABC,ABC,3\n")]),
        fx("RTS_013", vec![("routes.txt", "route_id,agency_id,route_short_name,route_type,continuous_pickup\nR1,1,101,3,9\n")]),
        fx("RTS_018", vec![("routes.txt", "route_id,agency_id,route_short_name,route_type,continuous_drop_off\nR1,1,101,3,9\n")]),

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
        fx("STM_023", vec![("stop_times.txt", "trip_id,arrival_time,departure_time,stop_id,stop_sequence\nT1,08:00:00,08:00:00,S1,2\nT1,08:10:00,08:10:00,S2,1\n")]),
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
        // OPR_007: seferde aynı durak tekrar (ring değil).
        fx("OPR_007", vec![("stop_times.txt", "trip_id,arrival_time,departure_time,stop_id,stop_sequence\nT1,08:00:00,08:00:00,S1,1\nT1,08:10:00,08:10:00,S2,2\nT1,08:20:00,08:20:00,S2,3\n")]),
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
        // OPR_018: servis dönemi < 3 aktif gün (2 günlük pencere).
        fx("OPR_018", vec![("calendar.txt", "service_id,monday,tuesday,wednesday,thursday,friday,saturday,sunday,start_date,end_date\nSVC1,1,1,1,1,1,1,1,20260518,20260519\n")]),
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
    ]
}

#[test]
fn each_fixture_actually_emits_its_rule() {
    let mut failures = Vec::new();
    for f in fixtures() {
        let emitted = emitted_rules(&with_opts(&f.overrides, &f.removes));
        if !emitted.contains(f.rule) {
            failures.push(format!("  {} emit etmedi → {:?}", f.rule, emitted));
        }
    }
    assert!(failures.is_empty(), "Emit etmeyen fixture(lar):\n{}", failures.join("\n"));
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
