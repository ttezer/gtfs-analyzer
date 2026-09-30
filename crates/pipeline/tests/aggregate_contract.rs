//! Feed-level toplulama sözleşmesi (`aggregate_feed_level_notices`).
//!
//! 22 kural K1–K6'da satır/varlık başına notice üretir; toplulama bunları gruplarına
//! göre tek özet notice'a indirir ve etkilenen kayıt sayısını taşır. Registry bu
//! kuralların çoğunu `Feed` dedup seviyesinde tanımladığı için toplulama atlanırsa
//! dedup ham notice'lardan birini seçer ve sayı sessizce kaybolur (WASM'da 2026-09-30'a
//! kadar böyleydi). Bu test her kural için sözleşmeyi doğrudan fonksiyon üzerinde kurar:
//! grup başına tek notice, doğru sayı, dokunulmayan komşu notice'lar.

use std::collections::BTreeMap;

use gtfs_core::{EntityType, Notice};
use gtfs_pipeline::{aggregate_feed_level_notices, DerivedData, EntityRecords};

/// Toplulamanın grup anahtarı.
#[derive(Clone, Copy)]
enum Group {
    /// Tüm feed tek özet.
    Feed,
    /// Dosya başına özet (`notice.file`).
    File,
    /// Varlık başına özet (`notice.entity_id`).
    Entity(EntityType),
    /// Aynı `observed_value` (bitiş tarihi) başına özet.
    Observed,
}

/// Özet notice'ta etkilenen kayıt sayısının durduğu yer.
#[derive(Clone, Copy)]
enum Count {
    Observed,
    Detail(&'static str),
}

const CASES: &[(&str, Group, Count)] = &[
    ("STM_036", Group::Feed, Count::Observed),
    ("DQ_021", Group::File, Count::Observed),
    ("ARC_012", Group::File, Count::Observed),
    ("CLD_003", Group::File, Count::Observed),
    ("SHP_005", Group::Feed, Count::Observed),
    ("STM_008", Group::Entity(EntityType::Trip), Count::Observed),
    ("STM_047", Group::Entity(EntityType::Trip), Count::Observed),
    ("PTH_007", Group::Entity(EntityType::Pathway), Count::Observed),
    ("TRP_003", Group::Feed, Count::Observed),
    ("TRN_001", Group::Feed, Count::Observed),
    ("STP_004", Group::Feed, Count::Observed),
    ("STP_005", Group::Feed, Count::Observed),
    ("TRF_005", Group::Feed, Count::Observed),
    ("PTH_012", Group::Feed, Count::Observed),
    ("STP_042", Group::Feed, Count::Observed),
    ("STP_032", Group::Feed, Count::Observed),
    ("TRP_005", Group::Feed, Count::Observed),
    ("STM_022", Group::Feed, Count::Observed),
    ("FRQ_007", Group::Feed, Count::Observed),
    ("TRF_019", Group::Feed, Count::Observed),
    ("CAL_008", Group::Observed, Count::Detail("affected_services")),
    ("GGL_001", Group::Feed, Count::Observed),
    ("FAR_010", Group::Feed, Count::Observed),
];

/// `group` anahtarlı ham notice. `row` satır ve ayrık varlık kimliği üretir.
fn raw(rule_id: &str, group: Group, key: &str, row: u64) -> Notice {
    let meta = gtfs_rules::get_rule(rule_id).expect("registry'de olmayan kural");
    let (entity_type, entity_id) = match group {
        Group::Entity(kind) => (kind, key.to_string()),
        _ => (EntityType::Row, format!("{rule_id}-{row}")),
    };
    Notice {
        id: format!("raw/{rule_id}#{row}"),
        rule_id: rule_id.to_string(),
        severity: meta.severity,
        rule_class: meta.rule_class,
        entity_type,
        entity_id: Some(entity_id.clone()),
        scope_key: Some(entity_id),
        file: Some(match group {
            Group::File => key.to_string(),
            _ => "stop_times.txt".to_string(),
        }),
        line: Some(row),
        field: Some("field".to_string()),
        observed_value: Some(match group {
            Group::Observed => key.to_string(),
            _ => "raw".to_string(),
        }),
        expected_value: None,
        details: None,
        whitespace_derived: false,
        whitespace_candidate: false,
        title: meta.title.to_string(),
        message: "raw".to_string(),
        remediation: "raw".to_string(),
        blocks: Vec::new(),
        base_effort: meta.base_effort,
        service_id: None,
    }
}

fn count_of(notice: &Notice, count: Count) -> Option<&str> {
    match count {
        Count::Observed => notice.observed_value.as_deref(),
        Count::Detail(key) => notice.details.as_ref()?.get(key).map(String::as_str),
    }
}

fn aggregate(mut notices: Vec<Notice>) -> Vec<Notice> {
    aggregate_feed_level_notices(
        &mut notices,
        &EntityRecords::default(),
        &DerivedData::default(),
    );
    notices
}

/// Yabancı bir kuralın notice'ı toplulamadan etkilenmemeli.
fn bystander() -> Notice {
    let mut n = raw("STM_054", Group::Feed, "", 999);
    n.entity_type = EntityType::Trip;
    n.entity_id = Some("bystander".to_string());
    n
}

#[test]
fn every_aggregated_rule_carries_the_affected_count() {
    for &(rule_id, group, count) in CASES {
        let key = match group {
            Group::File => "stop_times.txt",
            Group::Observed => "20260601",
            _ => "K1",
        };
        let mut input: Vec<Notice> = (1..=3).map(|row| raw(rule_id, group, key, row)).collect();
        input.push(bystander());
        let out = aggregate(input);

        let summaries: Vec<&Notice> = out.iter().filter(|n| n.rule_id == rule_id).collect();
        assert_eq!(summaries.len(), 1, "{rule_id}: tek grup tek özet olmalı");
        assert_eq!(
            count_of(summaries[0], count),
            Some("3"),
            "{rule_id}: etkilenen kayıt sayısı taşınmalı"
        );
        if matches!(group, Group::Feed | Group::Observed) {
            assert_eq!(summaries[0].entity_type, EntityType::Feed, "{rule_id}: feed özeti");
            assert_eq!(summaries[0].entity_id, None, "{rule_id}: feed özeti varlık taşımaz");
        }
        let kept = out.iter().filter(|n| n.rule_id == "STM_054").count();
        assert_eq!(kept, 1, "{rule_id}: başka kuralın notice'ı korunmalı");
    }
}

#[test]
fn grouped_rules_summarise_each_group_separately() {
    for &(rule_id, group, count) in CASES {
        let keys: [&str; 2] = match group {
            Group::Feed => continue,
            Group::File => ["stop_times.txt", "trips.txt"],
            Group::Observed => ["20260601", "20260615"],
            Group::Entity(_) => ["K1", "K2"],
        };
        let input = vec![
            raw(rule_id, group, keys[0], 1),
            raw(rule_id, group, keys[0], 2),
            raw(rule_id, group, keys[1], 3),
        ];
        let out = aggregate(input);
        let mut counts: Vec<&str> = out
            .iter()
            .filter(|n| n.rule_id == rule_id)
            .map(|n| count_of(n, count).expect("özet sayı taşımalı"))
            .collect();
        counts.sort_unstable();
        assert_eq!(counts, ["1", "2"], "{rule_id}: grup başına ayrı özet");
    }
}

/// Test tablosu ile gerçek çağrı listesi aynı kalmalı: yeni toplulayıcı eklenip buraya
/// eklenmezse sözleşmesi sınanmadan kalırdı.
#[test]
fn case_table_matches_the_aggregation_calls() {
    let src = include_str!("../src/lib.rs");
    let body_start = src
        .find("pub fn aggregate_feed_level_notices(")
        .expect("aggregate_feed_level_notices bulunamadı");
    let body = &src[body_start..];
    let body = &body[..body.find("\n}\n").expect("fonksiyon sonu yok")];
    let called: Vec<&str> = body
        .lines()
        .filter_map(|l| l.trim().strip_prefix("timed_aggregate!(\""))
        .filter_map(|l| l.split('"').next())
        .collect();
    let tabled: Vec<&str> = CASES.iter().map(|(id, _, _)| *id).collect();
    assert_eq!(called, tabled, "toplulama çağrıları ile test tablosu ayrıştı");

    let unique: BTreeMap<&str, ()> = tabled.iter().map(|id| (*id, ())).collect();
    assert_eq!(unique.len(), tabled.len(), "tabloda tekrar eden kural");
}
