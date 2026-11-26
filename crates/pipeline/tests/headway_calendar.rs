//! Synthetic GTFS feeds from the service-calendar audit. These exercise the normal
//! ZIP -> K5 -> K6 -> K7 pipeline, including calendar exception overlays.

use gtfs_pipeline::{validate_bytes, ValidateResult, ValidationResult, ValidatorConfig};
use gtfs_core::i18n::{Lang, Translator};

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
fn multi_service_message_is_translated_for_turkish_cli_and_python() {
    let result = run("close");
    let mut notice = result
        .notices
        .iter()
        .find(|n| n.rule_id == "OPR_003")
        .unwrap()
        .clone();
    Translator::new(Lang::Tr)
        .unwrap()
        .unwrap()
        .translate(&mut notice);
    assert!(notice.message.contains("20261130 servis gününde"));
    assert!(notice.message.contains("A, B aktif servisleri"));
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

fn opr005<'a>(result: &'a ValidationResult) -> Vec<&'a gtfs_core::Notice> {
    result.notices.iter().filter(|notice| notice.rule_id == "OPR_005").collect()
}

#[test]
fn opr005_merges_active_services_before_scoring_against_peers() {
    for name in ["outlier_fp", "outlier_fp_merged", "outlier_add", "outlier_dates_only"] {
        assert!(opr005(&run(name)).is_empty(), "{name} must not flag the merged 15-minute schedule");
    }
    for name in ["outlier_fn", "outlier_fn_merged"] {
        let result = run(name);
        let findings = opr005(&result);
        assert_eq!(findings.len(), 1, "{name}");
        let notice = findings[0];
        assert_eq!(notice.entity_id.as_deref(), Some("R"));
        assert_eq!(notice.observed_value.as_deref(), Some("5"));
        assert_eq!(notice.details.as_ref().unwrap().get("median_min").map(String::as_str), Some("15"));
        assert_eq!(notice.details.as_ref().unwrap().get("affected_days").map(String::as_str), Some("1"));
        assert_eq!(notice.service_id.is_none(), name == "outlier_fn");
    }
    let split = run("outlier_fn");
    let details = opr005(&split)[0].details.as_ref().unwrap();
    assert_eq!(details.get("service_ids").map(String::as_str), Some("[\"A\",\"B\",\"C\"]"));
    assert_eq!(details.get("sample_date").map(String::as_str), Some("20261130"));
}

#[test]
fn opr005_respects_removed_services_and_separate_weekends() {
    for name in ["outlier_remove", "outlier_weekend"] {
        let result = run(name);
        let findings = opr005(&result);
        assert_eq!(findings.len(), 1, "{name}");
        assert_eq!(findings[0].service_id.as_deref(), Some("A"));
        assert_eq!(findings[0].observed_value.as_deref(), Some("60"));
    }
}

#[test]
fn opr005_frequency_profiles_are_counted_as_unevaluated() {
    for name in ["freq0", "freq1", "freq_mixed"] {
        let result = run(name);
        assert!(opr005(&result).is_empty(), "{name}");
        let coverage = &result.metrics.headway_coverage;
        assert_eq!(coverage.opr_005_skipped_profiles, 1, "{name}");
        assert_eq!(coverage.opr_005_skipped_days, 1, "{name}");
    }
}

#[test]
fn opr005_multi_service_message_translates_for_turkish_cli() {
    let result = run("outlier_fn");
    let mut notice = opr005(&result)[0].clone();
    Translator::new(Lang::Tr).unwrap().unwrap().translate(&mut notice);
    assert!(notice.message.contains("20261130 servis gününde"));
    assert!(notice.message.contains("A, B, C aktif servisleri"));
}
