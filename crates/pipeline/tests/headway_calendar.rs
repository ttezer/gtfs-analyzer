//! Synthetic GTFS feeds from the service-calendar audit. These exercise the normal
//! ZIP -> K5 -> K6 -> K7 pipeline, including calendar exception overlays.

use gtfs_pipeline::{validate_bytes, ValidateResult, ValidationResult, ValidatorConfig};

fn run(name: &str) -> ValidationResult {
    let path = format!(
        "{}/tests/fixtures/headway_calendar/{name}.zip",
        env!("CARGO_MANIFEST_DIR")
    );
    let zip = std::fs::read(path).unwrap();
    match validate_bytes(&zip, &ValidatorConfig::default(), 20261126) {
        ValidateResult::Ok(result) => result,
        ValidateResult::Fatal(fatal) => panic!("{name}: {fatal:?}"),
    }
}

fn headways(result: &ValidationResult) -> Vec<(&str, &str)> {
    result
        .notices
        .iter()
        .filter(|n| matches!(n.rule_id.as_str(), "OPR_001" | "OPR_003"))
        .map(|n| {
            (
                n.rule_id.as_str(),
                n.observed_value.as_deref().unwrap_or(""),
            )
        })
        .collect()
}

#[test]
fn calendar_profiles_respect_overlaps_and_exceptions() {
    for (name, expected) in [
        ("nelson", vec![]),
        ("gap", vec![]),
        ("gap_merged", vec![]),
        ("gap_split_single", vec![("OPR_001", "495")]),
        ("gap_split_single_merged", vec![("OPR_001", "495")]),
        ("close", vec![("OPR_003", "1")]),
        ("close_merged", vec![("OPR_003", "1")]),
        ("close_window", vec![("OPR_003", "1")]),
        ("weekend", vec![]),
        ("partial", vec![("OPR_003", "1")]),
        ("add", vec![("OPR_003", "1")]),
        ("remove", vec![]),
        ("dates_only", vec![("OPR_003", "1")]),
        ("gap_cancelled", vec![]),
        ("close_cancelled", vec![]),
        ("same_time", vec![]),
        ("terminals", vec![]),
        ("patterns", vec![]),
        ("midnight", vec![("OPR_003", "1")]),
    ] {
        assert_eq!(headways(&run(name)), expected, "{name}");
    }
}

#[test]
fn multi_service_notices_preserve_context_and_single_service_contract() {
    let multi = run("close");
    let notice = multi
        .notices
        .iter()
        .find(|n| n.rule_id == "OPR_003")
        .unwrap();
    assert_eq!(notice.service_id, None);
    assert!(serde_json::to_value(notice)
        .unwrap()
        .get("service_id")
        .is_none());
    let details = notice.details.as_ref().unwrap();
    assert_eq!(
        details.get("service_ids").map(String::as_str),
        Some("[\"A\",\"B\"]")
    );
    assert_eq!(
        details.get("sample_date").map(String::as_str),
        Some("20261130")
    );
    assert_eq!(details.get("affected_days").map(String::as_str), Some("1"));
    assert_eq!(details.get("gap_from").map(String::as_str), Some("08:00"));
    assert_eq!(details.get("gap_to").map(String::as_str), Some("08:01"));

    let single = run("close_merged");
    let notice = single
        .notices
        .iter()
        .find(|n| n.rule_id == "OPR_003")
        .unwrap();
    assert_eq!(notice.service_id.as_deref(), Some("A"));
    assert_eq!(serde_json::to_value(notice).unwrap()["service_id"], "A");
    assert_eq!(
        notice
            .details
            .as_ref()
            .unwrap()
            .get("service_ids")
            .map(String::as_str),
        Some("[\"A\"]")
    );
}

#[test]
fn one_representative_counts_all_affected_days() {
    let result = run("dedup");
    let headways: Vec<_> = result
        .notices
        .iter()
        .filter(|n| n.rule_id == "OPR_003")
        .collect();
    assert_eq!(headways.len(), 1);
    let details = headways[0].details.as_ref().unwrap();
    assert_eq!(details.get("affected_days").map(String::as_str), Some("2"));
    assert_eq!(
        details.get("sample_date").map(String::as_str),
        Some("20261130")
    );
    assert_eq!(headways[0].service_id.as_deref(), Some("A"));
}

#[test]
fn sample_date_prefers_feed_info_window_without_clipping_analysis() {
    let result = run("close_window");
    let notice = result
        .notices
        .iter()
        .find(|n| n.rule_id == "OPR_003")
        .unwrap();
    let details = notice.details.as_ref().unwrap();
    assert_eq!(details.get("affected_days").map(String::as_str), Some("2"));
    assert_eq!(
        details.get("sample_date").map(String::as_str),
        Some("20261201")
    );
}

#[test]
fn simultaneous_departures_are_not_bunching_and_keep_capacity_signal() {
    let result = run("same_time");
    assert!(headways(&result).is_empty());
    assert!(result.notices.iter().any(|n| n.rule_id == "OPR_019"));
}

#[test]
fn frequency_profiles_are_visible_as_unevaluated() {
    for name in ["freq0", "freq1"] {
        let result = run(name);
        assert!(headways(&result).is_empty(), "{name}");
        let coverage = &result.metrics.headway_coverage;
        assert_eq!(coverage.opr_001_skipped_profiles, 1, "{name}");
        assert_eq!(coverage.opr_001_skipped_days, 1, "{name}");
        assert_eq!(coverage.opr_003_skipped_pattern_profiles, 1, "{name}");
    }
    let mixed = run("freq_mixed");
    assert_eq!(headways(&mixed), vec![("OPR_003", "1")]);
    let coverage = &mixed.metrics.headway_coverage;
    assert_eq!(coverage.opr_001_skipped_profiles, 1);
    assert_eq!(coverage.opr_003_skipped_pattern_profiles, 1);
}
