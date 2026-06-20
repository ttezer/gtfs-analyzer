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
