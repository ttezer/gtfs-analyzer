//! Agency dökümünün sayım sözleşmesi (MobilityData gtfs-validator #2201, P3).
//!
//! Üç birim birbirine karışmaz: `finding_count` (rapora giren bulgu), `affected_entity_count`
//! (özetlerin altındaki ham kayıtlar dahil), `displayed_sample_count` (sonuçta taşınan).
//! Native'de cap yoktur; bu yüzden döküm sonuçtaki notice listesiyle BİREBİR örtüşmelidir.
//! Boşluk türevlerini bastırılmış saymamak da buradan çıkar: bastırılan notice sonuçta yoksa
//! dökümde de yoktur.

use std::collections::BTreeMap;
use std::io::Write as _;
use std::path::PathBuf;

use gtfs_core::agency::AgencyAttribution;
use gtfs_pipeline::{validate_bytes, ValidateResult, ValidationResult, ValidatorConfig};
use serde_json::Value;
use zip::write::SimpleFileOptions;

fn zip_of(files: &serde_json::Map<String, Value>) -> Vec<u8> {
    let mut writer = zip::ZipWriter::new(std::io::Cursor::new(Vec::new()));
    for (name, body) in files {
        writer.start_file(name.as_str(), SimpleFileOptions::default()).unwrap();
        writer.write_all(body.as_str().unwrap().as_bytes()).unwrap();
    }
    writer.finish().unwrap().into_inner()
}

fn run(files: &serde_json::Map<String, Value>, today: u32) -> ValidationResult {
    match validate_bytes(&zip_of(files), &ValidatorConfig::default(), today) {
        ValidateResult::Ok(result) => result,
        ValidateResult::Fatal(err) => panic!("feed fatal: {err:?}"),
    }
}

fn assert_contract(name: &str, result: &ValidationResult) {
    let breakdown = &result.agency_breakdown;
    assert!(breakdown.complete, "{name}: native döküm her zaman tamdır");

    let mut findings: BTreeMap<&str, u64> = BTreeMap::new();
    let mut affected: BTreeMap<&str, u64> = BTreeMap::new();
    for notice in &result.notices {
        *findings.entry(notice.rule_id.as_str()).or_default() += 1;
        *affected.entry(notice.rule_id.as_str()).or_default() += notice
            .agency_distribution
            .as_ref()
            .map_or(1, |d| d.values().sum());
    }
    let counted: BTreeMap<&str, u64> = breakdown
        .rules
        .iter()
        .map(|(rule, c)| (rule.as_str(), c.finding_count))
        .collect();
    assert_eq!(counted, findings, "{name}: finding_count sonuçtaki notice'larla örtüşmeli");

    for (rule, counts) in &breakdown.rules {
        let distributed: u64 = counts.by_attribution.values().sum();
        assert_eq!(distributed, counts.affected_entity_count, "{name}/{rule}: dağılım = etkilenen");
        assert_eq!(counts.affected_entity_count, affected[rule.as_str()], "{name}/{rule}: etkilenen");
        assert_eq!(counts.displayed_sample_count, counts.finding_count, "{name}/{rule}: native cap yok");
        assert!(counts.affected_entity_count >= counts.finding_count, "{name}/{rule}");
    }
}

/// Parite fixture'larının tamamı (taban + her vaka) sözleşmeyi sağlamalı.
#[test]
fn breakdown_matches_the_reported_notices_on_every_parity_fixture() {
    let dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../tests/fixtures/wasm-parity");
    let cases: Value =
        serde_json::from_str(&std::fs::read_to_string(dir.join("cases.json")).unwrap()).unwrap();
    let today: u32 = cases["today"].as_str().unwrap().parse().unwrap();
    let base = cases["base"].as_object().unwrap().clone();
    assert_contract("base", &run(&base, today));
    for case in cases["cases"].as_array().unwrap() {
        let mut files = base.clone();
        for (file, body) in case["files"].as_object().unwrap() {
            files.insert(file.clone(), body.clone());
        }
        assert_contract(case["name"].as_str().unwrap(), &run(&files, today));
    }
}

/// İki agency'nin birer seferinde direction_id geçersiz: TRP_005 tek bulgu, iki etkilenen
/// sefer, A=1 B=1. Route düzeyi bir kural (DQ_003, route_desc yok) route başına sayılır.
#[test]
fn breakdown_separates_findings_from_affected_entities_per_agency() {
    let files: serde_json::Map<String, Value> = [
        ("agency.txt", "agency_id,agency_name,agency_url,agency_timezone\nA,Alpha,http://a.example,UTC\nB,Beta,http://b.example,UTC\n"),
        ("stops.txt", "stop_id,stop_name,stop_lat,stop_lon\nS1,One,41.0,29.0\nS2,Two,41.1,29.1\n"),
        ("routes.txt", "route_id,agency_id,route_short_name,route_type\nR1,A,1,3\nR2,B,2,3\n"),
        ("trips.txt", "route_id,service_id,trip_id,direction_id\nR1,SVC,T1,7\nR2,SVC,T2,9\n"),
        ("stop_times.txt", "trip_id,arrival_time,departure_time,stop_id,stop_sequence\nT1,08:00:00,08:00:00,S1,1\nT1,08:10:00,08:10:00,S2,2\nT2,09:00:00,09:00:00,S1,1\nT2,09:10:00,09:10:00,S2,2\n"),
        ("calendar.txt", "service_id,monday,tuesday,wednesday,thursday,friday,saturday,sunday,start_date,end_date\nSVC,1,1,1,1,1,0,0,20250101,20271231\n"),
    ]
    .into_iter()
    .map(|(k, v)| (k.to_string(), Value::String(v.to_string())))
    .collect();
    let result = run(&files, 20_260_930);
    assert_contract("two-agency", &result);
    let breakdown = &result.agency_breakdown;
    assert_eq!(breakdown.agencies, ["A", "B"]);
    assert_eq!(breakdown.agency_names, ["Alpha", "Beta"]);
    assert_eq!(breakdown.agency_trip_counts, [1, 1]);

    let per_agency = |rule: &str| -> Vec<(String, u64)> {
        breakdown.rules[rule]
            .by_attribution
            .iter()
            .map(|(a, n)| match a.agency() {
                Some(agency) => (breakdown.agencies[agency.0 as usize].clone(), *n),
                None => (format!("{a:?}"), *n),
            })
            .collect()
    };
    let trp005 = &breakdown.rules["TRP_005"];
    assert_eq!((trp005.finding_count, trp005.affected_entity_count), (1, 2));
    assert_eq!(per_agency("TRP_005"), [("A".to_string(), 1), ("B".to_string(), 1)]);
    assert!(
        breakdown.rules["TRP_005"].by_attribution.keys().all(|a| matches!(a, AgencyAttribution::Resolved { .. })),
    );
    let dq003 = &breakdown.rules["DQ_003"];
    assert_eq!((dq003.finding_count, dq003.affected_entity_count), (2, 2));
    assert_eq!(per_agency("DQ_003"), [("A".to_string(), 1), ("B".to_string(), 1)]);
}
