pub mod decompress_guard;
pub(crate) mod k1_html_elements;
pub(crate) mod k1_html_entities;
pub mod k1_parse;
pub mod k2;
pub mod k3_entity_graph;
pub mod k4_cross_ref;
pub mod k5_derived;
pub mod k6_analytics;
pub mod k7_reporting;
pub(crate) mod notice_factory;
pub mod recovery;
pub(crate) mod timing;
pub mod whitespace_suppression;

pub use k1_parse::{parse, parse_with_limits, K1Result, RawFile, RawFiles};
pub use k2::{
    validate as validate_k2, validate_with_stream_limit as validate_k2_with_stream_limit,
    validate_with_stream_limit_and_jp_signal as validate_k2_with_jp_signal,
    validate_with_stream_limit_and_jp_signal_and_whitespace_roots as validate_k2_with_whitespace_roots,
    EntityRecords, K2Result, GTFS_JP_FILES,
};
pub use k3_entity_graph::{build as build_entity_map, EntityMap, K3Result};
pub use k4_cross_ref::{
    check as check_cross_ref, check_with_files as check_cross_ref_with_files,
    check_with_files_and_whitespace_roots as check_cross_ref_with_whitespace_roots, K4Result,
};
pub use k5_derived::{
    build as build_derived, build_with_files as build_derived_with_files, DerivedData, K5Result,
};
pub use k6_analytics::{
    analyze as analyze_k6, analyze_with_files as analyze_k6_with_files, K6Result,
};
pub use k7_reporting::{
    report as report_k7, report_with_whitespace_suppressions as report_k7_with_suppressions,
    K7Result,
};
pub use recovery::FileAvailability;
pub use whitespace_suppression::WhitespaceSuppressions;

// Entegrasyon testlerine açık yeniden ihracat
pub use gtfs_config::{CalendarOverrideRule, GtfsJpProfile, ValidatorConfig};
pub use gtfs_core::{
    FatalCode, FatalError, FileInfo, ValidateResult, ValidationResult, ValidationStatus,
};

/// Returns true only when the feed itself proves that every served route has
/// an all-trip fare. This is the narrow exception for STP_033: a uniform fare
/// does not need stop zones, while a zone-based or ambiguous fare still does.
///
/// Uses only `EntityRecords` so the WASM rerun path (which caches records but
/// not the K3 entity map) evaluates the same predicate as `validate_bytes`.
fn has_uniform_fare_coverage(records: &EntityRecords) -> bool {
    use std::collections::HashSet;
    let valid_fares: HashSet<&str> = records
        .fare_attributes
        .iter()
        .filter(|fare| {
            !fare.fare_id.is_empty()
                && fare
                    .price
                    .is_some_and(|price| price.is_finite() && price >= 0.0)
                && crate::k2::common::iso4217_minor_unit(&fare.currency_type).is_some()
        })
        .map(|fare| fare.fare_id.as_str())
        .collect();
    if valid_fares.is_empty() {
        return false;
    }

    // A single valid fare without fare_rules is GTFS's uniform-fare form.
    if records.fare_rules.is_empty() && valid_fares.len() == 1 {
        return true;
    }

    let unqualified = |rule: &&crate::k2::fare_rules::FareRuleRecord| {
        valid_fares.contains(rule.fare_id.as_str())
            && rule.origin_id.is_none()
            && rule.destination_id.is_none()
            && rule.contains_id.is_none()
    };
    if records
        .fare_rules
        .iter()
        .filter(unqualified)
        .any(|rule| rule.route_id.is_none())
    {
        return true;
    }

    let uniform_routes: HashSet<&str> = records
        .fare_rules
        .iter()
        .filter(unqualified)
        .filter_map(|rule| rule.route_id.as_deref())
        .collect();
    let known_routes: HashSet<&str> = records
        .routes
        .iter()
        .map(|route| route.route_id.as_str())
        .filter(|id| !id.is_empty())
        .collect();
    let served_routes: HashSet<&str> = records
        .trips
        .iter()
        .map(|trip| records.trip_interns.route_id(trip))
        .filter(|route_id| known_routes.contains(*route_id))
        .collect();
    !served_routes.is_empty()
        && served_routes
            .iter()
            .all(|route_id| uniform_routes.contains(route_id))
}

/// Rejects a `disabled_rule_ids` list the report cannot honour faithfully.
///
/// * An unknown or retired id would silently disable nothing while the user
///   believes a check is off.
/// * A Spec-class rule is the normative evidence behind the publishability
///   verdict (R1); hiding it could turn an invalid feed into "publishable".
///
/// Every orchestration calls this before running, so native, WASM, and SDK
/// fail the same way.
pub fn check_rule_scope(config: &ValidatorConfig) -> Result<(), FatalError> {
    let listed = config
        .disabled_rule_ids
        .iter()
        .chain(config.disabled_rule_ids_gtfs_jp.iter());
    for rule_id in listed {
        match gtfs_rules::registry::get_rule(rule_id) {
            None => {
                return Err(FatalError::new(
                    FatalCode::InvalidInput,
                    format!("disabled_rule_ids: '{rule_id}' bilinen bir kural kimliği değil."),
                )
                .variant("disabled_rule_unknown")
                .param("rule_id", rule_id));
            }
            Some(meta) if meta.rule_class == gtfs_core::RuleClass::Spec => {
                return Err(FatalError::new(
                    FatalCode::InvalidInput,
                    format!(
                        "disabled_rule_ids: '{rule_id}' bir Spec kuralı; yayın kararı bu kurallara dayandığı için kapatılamaz."
                    ),
                )
                .variant("disabled_rule_spec")
                .param("rule_id", rule_id));
            }
            Some(_) => {}
        }
    }
    Ok(())
}

/// Report-scope filtering shared by every orchestration (native
/// `validate_bytes` and both WASM paths). Runs after K6 and before K7 so that
/// removed notices cannot affect scores, report views, or the R9 queue.
///
/// 1. STP_033 is dropped when the feed proves uniform fare coverage.
/// 2. Rules listed in `config.disabled_rule_ids` are dropped.
/// 3. Rules listed in `config.disabled_rule_ids_gtfs_jp` are dropped ONLY when the
///    feed is a detected GTFS-JP feed. Japanese publishing practice makes some
///    checks meaningless there; feeds elsewhere keep them.
fn aggregate_stm036(notices: &mut Vec<gtfs_core::Notice>) {
    let mut first_position = None;
    let mut matched = Vec::new();
    let mut retained = Vec::with_capacity(notices.len());
    for (index, notice) in notices.drain(..).enumerate() {
        if notice.rule_id == "STM_036" {
            first_position.get_or_insert(index);
            matched.push(notice);
        } else {
            retained.push(notice);
        }
    }
    if matched.is_empty() {
        *notices = retained;
        return;
    }

    let mut aggregate = matched.swap_remove(0);
    let affected_trips = matched.len() + 1;
    let mut examples: Vec<String> = aggregate.entity_id.clone().into_iter()
        .chain(matched.iter().filter_map(|notice| notice.entity_id.clone()))
        .collect();
    examples.sort_unstable();
    examples.dedup();
    examples.truncate(5);

    aggregate.entity_type = gtfs_core::EntityType::Feed;
    aggregate.entity_id = None;
    aggregate.scope_key = None;
    aggregate.file = Some("stop_times.txt".to_string());
    aggregate.line = None;
    aggregate.field = Some("trip_id".to_string());
    aggregate.observed_value = Some(affected_trips.to_string());
    aggregate.expected_value = Some("feed-level aggregate".to_string());
    aggregate.message = format!(
        "stop_times.txt içinde {affected_trips} seferin satırları trip_id + stop_sequence düzeninde değil."
    );
    aggregate.details = Some({
        let mut details = std::collections::BTreeMap::new();
        details.insert("affected_trips".to_string(), affected_trips.to_string());
        if !examples.is_empty() {
            details.insert("example_trips".to_string(), examples.join(", "));
        }
        details
    });
    aggregate.service_id = None;
    aggregate.whitespace_derived = matched.iter().all(|n| n.whitespace_derived)
        && aggregate.whitespace_derived;
    aggregate.whitespace_candidate = matched.iter().all(|n| n.whitespace_candidate)
        && aggregate.whitespace_candidate;

    let insert_at = first_position.unwrap_or(retained.len()).min(retained.len());
    retained.insert(insert_at, aggregate);
    *notices = retained;
}

/// DQ_021 emits one notice per duplicate key. Keep file context while
/// reducing large feeds to one deterministic summary per affected file.
fn aggregate_dq021(notices: &mut Vec<gtfs_core::Notice>) {
    use std::collections::BTreeMap;

    let mut first_positions: BTreeMap<String, usize> = BTreeMap::new();
    let mut grouped: BTreeMap<String, Vec<gtfs_core::Notice>> = BTreeMap::new();
    let mut retained = Vec::with_capacity(notices.len());
    for (index, notice) in notices.drain(..).enumerate() {
        if notice.rule_id == "DQ_021" {
            let file = notice
                .file
                .clone()
                .unwrap_or_else(|| "unknown file".to_string());
            first_positions.entry(file.clone()).or_insert(index);
            grouped.entry(file).or_default().push(notice);
        } else {
            retained.push(notice);
        }
    }
    if grouped.is_empty() {
        *notices = retained;
        return;
    }

    let mut aggregates = Vec::with_capacity(grouped.len());
    for (file, mut matches) in grouped {
        let mut aggregate = matches.swap_remove(0);
        let affected_duplicates = matches.len() + 1;
    let mut examples: Vec<String> = aggregate.observed_value.clone().into_iter()
            .chain(matches.iter().filter_map(|notice| notice.observed_value.clone()))
            .collect();
        examples.sort_unstable();
        examples.dedup();
        examples.truncate(5);

        aggregate.entity_type = gtfs_core::EntityType::File;
        aggregate.entity_id = Some(file.clone());
        aggregate.scope_key = None;
        aggregate.file = Some(file.clone());
        aggregate.line = None;
        aggregate.field = None;
        aggregate.observed_value = Some(affected_duplicates.to_string());
        aggregate.expected_value = Some("file-level aggregate".to_string());
        aggregate.message = format!(
            "{file} içinde {affected_duplicates} birincil anahtar yinelenmesi tespit edildi."
        );
        aggregate.details = Some({
            let mut details = BTreeMap::new();
            details.insert(
                "affected_duplicates".to_string(),
                affected_duplicates.to_string(),
            );
            if !examples.is_empty() {
                details.insert("example_keys".to_string(), examples.join(" | "));
            }
            details
        });
        aggregate.service_id = None;
        aggregate.whitespace_derived = matches.iter().all(|n| n.whitespace_derived)
            && aggregate.whitespace_derived;
        aggregate.whitespace_candidate = matches.iter().all(|n| n.whitespace_candidate)
            && aggregate.whitespace_candidate;
        aggregates.push((file, aggregate));
    }

    aggregates.sort_by(|(left, _), (right, _)| left.cmp(right));
    for (file, aggregate) in aggregates.into_iter().rev() {
        let insert_at = first_positions
            .get(&file)
            .copied()
            .unwrap_or(retained.len())
            .min(retained.len());
        retained.insert(insert_at, aggregate);
    }
    *notices = retained;
}

/// ARC_012 is emitted per malformed row by both buffered and streaming paths.
/// Aggregate by file and severity so short optional tails remain INFO while
/// rows with extra columns remain CRITICAL.
fn aggregate_arc012(notices: &mut Vec<gtfs_core::Notice>) {
    use std::collections::BTreeMap;

    let mut first_positions: BTreeMap<(String, u8), usize> = BTreeMap::new();
    let mut grouped: BTreeMap<(String, u8), Vec<gtfs_core::Notice>> = BTreeMap::new();
    let mut retained = Vec::with_capacity(notices.len());
    for (index, notice) in notices.drain(..).enumerate() {
        if notice.rule_id == "ARC_012" {
            let file = notice
                .file
                .clone()
                .unwrap_or_else(|| "unknown file".to_string());
            let severity_key = match notice.severity {
                gtfs_core::Severity::Kritik => 0,
                gtfs_core::Severity::Yuksek => 1,
                gtfs_core::Severity::Orta => 2,
                gtfs_core::Severity::Dusuk => 3,
                gtfs_core::Severity::Bilgi => 4,
            };
            let key = (file, severity_key);
            first_positions.entry(key.clone()).or_insert(index);
            grouped.entry(key).or_default().push(notice);
        } else {
            retained.push(notice);
        }
    }
    if grouped.is_empty() {
        *notices = retained;
        return;
    }

    let mut aggregates = Vec::with_capacity(grouped.len());
    for ((file, severity_key), mut matches) in grouped {
        let mut aggregate = matches.swap_remove(0);
        let affected_rows = matches.len() + 1;
        let mut lines: Vec<u64> = aggregate.line.into_iter()
            .chain(matches.iter().filter_map(|notice| notice.line))
            .collect();
        lines.sort_unstable();
        lines.dedup();
        lines.truncate(5);

        aggregate.entity_type = gtfs_core::EntityType::File;
        aggregate.entity_id = Some(file.clone());
        aggregate.scope_key = None;
        aggregate.file = Some(file.clone());
        aggregate.line = None;
        aggregate.field = None;
        aggregate.observed_value = Some(affected_rows.to_string());
        aggregate.expected_value = Some("file-level aggregate".to_string());
        aggregate.message = match severity_key {
            4 => format!(
                "{file} içinde {affected_rows} satırda sondaki isteğe bağlı sütunlar eksik."
            ),
            _ => format!(
                "{file} içinde {affected_rows} satırın sütun sayısı başlıkla uyuşmuyor."
            ),
        };
        aggregate.details = Some({
            let mut details = BTreeMap::new();
            details.insert("affected_rows".to_string(), affected_rows.to_string());
            if !lines.is_empty() {
                details.insert(
                    "example_lines".to_string(),
                    lines.iter().map(u64::to_string).collect::<Vec<_>>().join(", "),
                );
            }
            details
        });
        aggregate.service_id = None;
        aggregate.whitespace_derived = matches.iter().all(|n| n.whitespace_derived)
            && aggregate.whitespace_derived;
        aggregate.whitespace_candidate = matches.iter().all(|n| n.whitespace_candidate)
            && aggregate.whitespace_candidate;
        aggregates.push(((file, severity_key), aggregate));
    }

    aggregates.sort_by(|((left_file, left_severity), _), ((right_file, right_severity), _)| {
        left_file
            .cmp(right_file)
            .then(left_severity.cmp(right_severity))
    });
    for (key, aggregate) in aggregates.into_iter().rev() {
        let insert_at = first_positions
            .get(&key)
            .copied()
            .unwrap_or(retained.len())
            .min(retained.len());
        retained.insert(insert_at, aggregate);
    }
    *notices = retained;
}

/// CLD_003 is a file-local enum validation. Collapse its row notices while
/// retaining the affected-row count and a few source line examples.
fn aggregate_cld003(notices: &mut Vec<gtfs_core::Notice>) {
    use std::collections::BTreeMap;

    let mut first_positions: BTreeMap<String, usize> = BTreeMap::new();
    let mut grouped: BTreeMap<String, Vec<gtfs_core::Notice>> = BTreeMap::new();
    let mut retained = Vec::with_capacity(notices.len());
    for (index, notice) in notices.drain(..).enumerate() {
        if notice.rule_id == "CLD_003" {
            let file = notice
                .file
                .clone()
                .unwrap_or_else(|| "calendar_dates.txt".to_string());
            first_positions.entry(file.clone()).or_insert(index);
            grouped.entry(file).or_default().push(notice);
        } else {
            retained.push(notice);
        }
    }
    if grouped.is_empty() {
        *notices = retained;
        return;
    }

    let mut aggregates = Vec::with_capacity(grouped.len());
    for (file, mut matches) in grouped {
        let mut aggregate = matches.swap_remove(0);
        let affected_rows = matches.len() + 1;
        let mut lines: Vec<u64> = aggregate.line.into_iter()
            .chain(matches.iter().filter_map(|notice| notice.line))
            .collect();
        lines.sort_unstable();
        lines.dedup();
        lines.truncate(5);

        aggregate.entity_type = gtfs_core::EntityType::File;
        aggregate.entity_id = Some(file.clone());
        aggregate.scope_key = None;
        aggregate.file = Some(file.clone());
        aggregate.line = None;
        aggregate.field = Some("exception_type".to_string());
        aggregate.observed_value = Some(affected_rows.to_string());
        aggregate.expected_value = Some("1 or 2".to_string());
        aggregate.message = format!(
            "{file} içinde {affected_rows} satırda exception_type eksik veya geçersiz."
        );
        aggregate.details = Some({
            let mut details = BTreeMap::new();
            details.insert("affected_rows".to_string(), affected_rows.to_string());
            if !lines.is_empty() {
                details.insert(
                    "example_lines".to_string(),
                    lines.iter().map(u64::to_string).collect::<Vec<_>>().join(", "),
                );
            }
            details
        });
        aggregate.service_id = None;
        aggregate.whitespace_derived = matches.iter().all(|n| n.whitespace_derived)
            && aggregate.whitespace_derived;
        aggregate.whitespace_candidate = matches.iter().all(|n| n.whitespace_candidate)
            && aggregate.whitespace_candidate;
        aggregates.push((file, aggregate));
    }

    aggregates.sort_by(|(left, _), (right, _)| left.cmp(right));
    for (file, aggregate) in aggregates.into_iter().rev() {
        let insert_at = first_positions
            .get(&file)
            .copied()
            .unwrap_or(retained.len())
            .min(retained.len());
        retained.insert(insert_at, aggregate);
    }
    *notices = retained;
}

fn aggregate_shp005(notices: &mut Vec<gtfs_core::Notice>) {
    let mut first_position = None;
    let mut matched = Vec::new();
    let mut retained = Vec::with_capacity(notices.len());
    for (index, notice) in notices.drain(..).enumerate() {
        if notice.rule_id == "SHP_005" {
            first_position.get_or_insert(index);
            matched.push(notice);
        } else {
            retained.push(notice);
        }
    }
    if matched.is_empty() {
        *notices = retained;
        return;
    }

    let mut aggregate = matched.swap_remove(0);
    let affected_shapes = matched.len() + 1;
    let mut examples: Vec<String> = aggregate.entity_id.clone().into_iter()
        .chain(matched.iter().filter_map(|notice| notice.entity_id.clone()))
        .collect();
    examples.sort_unstable();
    examples.dedup();
    examples.truncate(5);
    aggregate.entity_type = gtfs_core::EntityType::Feed;
    aggregate.entity_id = None;
    aggregate.scope_key = None;
    aggregate.file = Some("shapes.txt".to_string());
    aggregate.line = None;
    aggregate.field = Some("shape_dist_traveled".to_string());
    aggregate.observed_value = Some(affected_shapes.to_string());
    aggregate.expected_value = Some("feed-level aggregate".to_string());
    aggregate.message = format!(
        "shapes.txt içinde {affected_shapes} shape için shape_dist_traveled azalıyor."
    );
    aggregate.details = Some({
        let mut details = std::collections::BTreeMap::new();
        details.insert("affected_shapes".to_string(), affected_shapes.to_string());
        if !examples.is_empty() {
            details.insert("example_shapes".to_string(), examples.join(", "));
        }
        details
    });
    aggregate.service_id = None;
    aggregate.whitespace_derived = matched.iter().all(|n| n.whitespace_derived)
        && aggregate.whitespace_derived;
    aggregate.whitespace_candidate = matched.iter().all(|n| n.whitespace_candidate)
        && aggregate.whitespace_candidate;
    retained.insert(
        first_position.unwrap_or(retained.len()).min(retained.len()),
        aggregate,
    );
    *notices = retained;
}

fn aggregate_stm008(notices: &mut Vec<gtfs_core::Notice>) {
    use std::collections::BTreeMap;

    let mut first_positions: BTreeMap<String, usize> = BTreeMap::new();
    let mut grouped: BTreeMap<String, Vec<gtfs_core::Notice>> = BTreeMap::new();
    let mut retained = Vec::with_capacity(notices.len());
    for (index, notice) in notices.drain(..).enumerate() {
        if notice.rule_id == "STM_008" {
            let trip_id = notice
                .entity_id
                .clone()
                .unwrap_or_else(|| "unknown trip".to_string());
            first_positions.entry(trip_id.clone()).or_insert(index);
            grouped.entry(trip_id).or_default().push(notice);
        } else {
            retained.push(notice);
        }
    }
    if grouped.is_empty() {
        *notices = retained;
        return;
    }

    let mut aggregates = Vec::with_capacity(grouped.len());
    for (trip_id, mut matches) in grouped {
        let mut aggregate = matches.swap_remove(0);
        let affected_segments = matches.len() + 1;
        aggregate.entity_type = gtfs_core::EntityType::Trip;
        aggregate.entity_id = Some(trip_id.clone());
        aggregate.scope_key = Some(trip_id.clone());
        aggregate.observed_value = Some(affected_segments.to_string());
        aggregate.expected_value = Some("monotonic trip chronology".to_string());
        aggregate.message = format!(
            "{affected_segments} zaman geriye gidişi bu seferin kronolojisini bozuyor."
        );
        aggregate.details.get_or_insert_with(BTreeMap::new).insert(
            "affected_segments".to_string(),
            affected_segments.to_string(),
        );
        aggregate.service_id = None;
        aggregate.whitespace_derived = matches.iter().all(|n| n.whitespace_derived)
            && aggregate.whitespace_derived;
        aggregate.whitespace_candidate = matches.iter().all(|n| n.whitespace_candidate)
            && aggregate.whitespace_candidate;
        aggregates.push((trip_id, aggregate));
    }

    aggregates.sort_by(|(left, _), (right, _)| left.cmp(right));
    for (trip_id, aggregate) in aggregates.into_iter().rev() {
        let insert_at = first_positions
            .get(&trip_id)
            .copied()
            .unwrap_or(retained.len())
            .min(retained.len());
        retained.insert(insert_at, aggregate);
    }
    *notices = retained;
}

fn aggregate_stm047(notices: &mut Vec<gtfs_core::Notice>) {
    use std::collections::BTreeMap;

    let mut first_positions: BTreeMap<String, usize> = BTreeMap::new();
    let mut grouped: BTreeMap<String, Vec<gtfs_core::Notice>> = BTreeMap::new();
    let mut retained = Vec::with_capacity(notices.len());
    for (index, notice) in notices.drain(..).enumerate() {
        if notice.rule_id == "STM_047" {
            let trip_id = notice.entity_id.clone().unwrap_or_default();
            first_positions.entry(trip_id.clone()).or_insert(index);
            grouped.entry(trip_id).or_default().push(notice);
        } else {
            retained.push(notice);
        }
    }
    if grouped.is_empty() {
        *notices = retained;
        return;
    }

    let mut aggregates = Vec::with_capacity(grouped.len());
    for (trip_id, mut matches) in grouped {
        let mut aggregate = matches.swap_remove(0);
        let affected_rows = matches.len() + 1;
        let mut fields: Vec<String> = aggregate.field.clone().into_iter()
            .chain(matches.iter().filter_map(|notice| notice.field.clone()))
            .flat_map(|field| field.split('|').map(str::to_string).collect::<Vec<_>>())
            .collect();
        fields.sort_unstable();
        fields.dedup();
        aggregate.entity_type = gtfs_core::EntityType::Trip;
        aggregate.entity_id = Some(trip_id.clone());
        aggregate.scope_key = Some(trip_id.clone());
        aggregate.line = None;
        aggregate.field = (!fields.is_empty()).then(|| fields.join("|"));
        aggregate.observed_value = Some(affected_rows.to_string());
        aggregate.expected_value = Some("filled at timepoint=1".to_string());
        aggregate.message = format!(
            "trip_id '{trip_id}' için {affected_rows} kesin zaman noktası satırında saat eksik."
        );
        aggregate.details = Some({
            let mut details = BTreeMap::new();
            details.insert("affected_rows".to_string(), affected_rows.to_string());
            details
        });
        aggregate.service_id = None;
        aggregate.whitespace_derived = matches.iter().all(|n| n.whitespace_derived)
            && aggregate.whitespace_derived;
        aggregate.whitespace_candidate = matches.iter().all(|n| n.whitespace_candidate)
            && aggregate.whitespace_candidate;
        aggregates.push((trip_id, aggregate));
    }

    aggregates.sort_by(|(left, _), (right, _)| left.cmp(right));
    for (trip_id, aggregate) in aggregates.into_iter().rev() {
        let insert_at = first_positions
            .get(&trip_id)
            .copied()
            .unwrap_or(retained.len())
            .min(retained.len());
        retained.insert(insert_at, aggregate);
    }
    *notices = retained;
}

fn aggregate_pth007(notices: &mut Vec<gtfs_core::Notice>) {
    use std::collections::BTreeMap;

    let mut first_positions: BTreeMap<String, usize> = BTreeMap::new();
    let mut grouped: BTreeMap<String, Vec<gtfs_core::Notice>> = BTreeMap::new();
    let mut retained = Vec::with_capacity(notices.len());
    for (index, notice) in notices.drain(..).enumerate() {
        if notice.rule_id == "PTH_007" {
            let pathway_id = notice.entity_id.clone().unwrap_or_default();
            first_positions.entry(pathway_id.clone()).or_insert(index);
            grouped.entry(pathway_id).or_default().push(notice);
        } else {
            retained.push(notice);
        }
    }
    if grouped.is_empty() {
        *notices = retained;
        return;
    }

    let mut aggregates = Vec::with_capacity(grouped.len());
    for (pathway_id, mut matches) in grouped {
        let mut aggregate = matches.swap_remove(0);
        let affected_rows = matches.len() + 1;
        aggregate.entity_type = gtfs_core::EntityType::Pathway;
        aggregate.entity_id = Some(pathway_id.clone());
        aggregate.scope_key = Some(pathway_id.clone());
        aggregate.line = None;
        aggregate.observed_value = Some(affected_rows.to_string());
        aggregate.expected_value = Some("positive integer seconds".to_string());
        aggregate.message = format!(
            "pathway_id '{pathway_id}' için {affected_rows} satırda traversal_time geçersiz."
        );
        aggregate.details = Some({
            let mut details = BTreeMap::new();
            details.insert("affected_rows".to_string(), affected_rows.to_string());
            details
        });
        aggregate.service_id = None;
        aggregate.whitespace_derived = matches.iter().all(|n| n.whitespace_derived)
            && aggregate.whitespace_derived;
        aggregate.whitespace_candidate = matches.iter().all(|n| n.whitespace_candidate)
            && aggregate.whitespace_candidate;
        aggregates.push((pathway_id, aggregate));
    }

    aggregates.sort_by(|(left, _), (right, _)| left.cmp(right));
    for (pathway_id, aggregate) in aggregates.into_iter().rev() {
        let insert_at = first_positions
            .get(&pathway_id)
            .copied()
            .unwrap_or(retained.len())
            .min(retained.len());
        retained.insert(insert_at, aggregate);
    }
    *notices = retained;
}

fn aggregate_trp003(notices: &mut Vec<gtfs_core::Notice>) {
    let mut first_position = None;
    let mut matched = Vec::new();
    let mut retained = Vec::with_capacity(notices.len());
    for (index, notice) in notices.drain(..).enumerate() {
        if notice.rule_id == "TRP_003" {
            first_position.get_or_insert(index);
            matched.push(notice);
        } else {
            retained.push(notice);
        }
    }
    if matched.is_empty() {
        *notices = retained;
        return;
    }

    let mut aggregate = matched.swap_remove(0);
    let affected_trips = matched.len() + 1;
    let mut examples: Vec<String> = aggregate.entity_id.clone().into_iter()
        .chain(matched.iter().filter_map(|notice| notice.entity_id.clone()))
        .collect();
    examples.sort_unstable();
    examples.dedup();
    examples.truncate(5);
    let mut missing_services: Vec<String> = aggregate.observed_value.clone().into_iter()
        .chain(matched.iter().filter_map(|notice| notice.observed_value.clone()))
        .collect();
    missing_services.sort_unstable();
    missing_services.dedup();
    missing_services.truncate(5);

    aggregate.entity_type = gtfs_core::EntityType::Feed;
    aggregate.entity_id = None;
    aggregate.scope_key = None;
    aggregate.file = Some("trips.txt".to_string());
    aggregate.line = None;
    aggregate.field = Some("service_id".to_string());
    aggregate.observed_value = Some(affected_trips.to_string());
    aggregate.expected_value = Some("service_id defined in calendar.txt or calendar_dates.txt".to_string());
    aggregate.message = format!(
        "trips.txt içinde {affected_trips} seferin service_id değeri takvimde tanımlı değil."
    );
    aggregate.details = Some({
        let mut details = std::collections::BTreeMap::new();
        details.insert("affected_trips".to_string(), affected_trips.to_string());
        if !examples.is_empty() {
            details.insert("example_trips".to_string(), examples.join(", "));
        }
        if !missing_services.is_empty() {
            details.insert("example_service_ids".to_string(), missing_services.join(", "));
        }
        details
    });
    aggregate.service_id = None;
    aggregate.whitespace_derived = matched.iter().all(|n| n.whitespace_derived)
        && aggregate.whitespace_derived;
    aggregate.whitespace_candidate = matched.iter().all(|n| n.whitespace_candidate)
        && aggregate.whitespace_candidate;
    retained.insert(
        first_position.unwrap_or(retained.len()).min(retained.len()),
        aggregate,
    );
    *notices = retained;
}

fn aggregate_trn001(notices: &mut Vec<gtfs_core::Notice>) {
    let mut first_position = None;
    let mut matched = Vec::new();
    let mut retained = Vec::with_capacity(notices.len());
    for (index, notice) in notices.drain(..).enumerate() {
        if notice.rule_id == "TRN_001" {
            first_position.get_or_insert(index);
            matched.push(notice);
        } else {
            retained.push(notice);
        }
    }
    if matched.is_empty() {
        *notices = retained;
        return;
    }

    let mut aggregate = matched.swap_remove(0);
    let affected_rows = matched.len() + 1;
    let mut examples: Vec<String> = aggregate.observed_value.clone().into_iter()
        .chain(matched.iter().filter_map(|notice| notice.observed_value.clone()))
        .collect();
    examples.sort_unstable();
    examples.dedup();
    examples.truncate(5);
    aggregate.entity_type = gtfs_core::EntityType::Feed;
    aggregate.entity_id = None;
    aggregate.scope_key = None;
    aggregate.file = Some("translations.txt".to_string());
    aggregate.line = None;
    aggregate.field = Some("table_name".to_string());
    aggregate.observed_value = Some(affected_rows.to_string());
    aggregate.expected_value = Some("supported GTFS table name".to_string());
    aggregate.message = format!(
        "translations.txt içinde {affected_rows} satırda table_name geçersiz."
    );
    aggregate.details = Some({
        let mut details = std::collections::BTreeMap::new();
        details.insert("affected_rows".to_string(), affected_rows.to_string());
        if !examples.is_empty() {
            details.insert("example_table_names".to_string(), examples.join(", "));
        }
        details
    });
    aggregate.service_id = None;
    aggregate.whitespace_derived = matched.iter().all(|n| n.whitespace_derived)
        && aggregate.whitespace_derived;
    aggregate.whitespace_candidate = matched.iter().all(|n| n.whitespace_candidate)
        && aggregate.whitespace_candidate;
    retained.insert(
        first_position.unwrap_or(retained.len()).min(retained.len()),
        aggregate,
    );
    *notices = retained;
}

fn aggregate_stp004(notices: &mut Vec<gtfs_core::Notice>) {
    let mut first_position = None;
    let mut matched = Vec::new();
    let mut retained = Vec::with_capacity(notices.len());
    for (index, notice) in notices.drain(..).enumerate() {
        if notice.rule_id == "STP_004" {
            first_position.get_or_insert(index);
            matched.push(notice);
        } else {
            retained.push(notice);
        }
    }
    if matched.is_empty() {
        *notices = retained;
        return;
    }

    let mut aggregate = matched.swap_remove(0);
    let affected_stops = matched.len() + 1;
    let mut examples: Vec<String> = aggregate.entity_id.clone().into_iter()
        .chain(matched.iter().filter_map(|notice| notice.entity_id.clone()))
        .collect();
    examples.sort_unstable();
    examples.dedup();
    examples.truncate(5);
    aggregate.entity_type = gtfs_core::EntityType::Feed;
    aggregate.entity_id = None;
    aggregate.scope_key = None;
    aggregate.file = Some("stops.txt".to_string());
    aggregate.line = None;
    aggregate.observed_value = Some(affected_stops.to_string());
    aggregate.expected_value = Some("valid floating-point latitude".to_string());
    aggregate.message = format!(
        "stops.txt içinde {affected_stops} stop için stop_lat sayısal değil."
    );
    aggregate.details = Some({
        let mut details = std::collections::BTreeMap::new();
        details.insert("affected_stops".to_string(), affected_stops.to_string());
        if !examples.is_empty() {
            details.insert("example_stop_ids".to_string(), examples.join(", "));
        }
        details
    });
    aggregate.service_id = None;
    aggregate.whitespace_derived = matched.iter().all(|n| n.whitespace_derived)
        && aggregate.whitespace_derived;
    aggregate.whitespace_candidate = matched.iter().all(|n| n.whitespace_candidate)
        && aggregate.whitespace_candidate;
    retained.insert(
        first_position.unwrap_or(retained.len()).min(retained.len()),
        aggregate,
    );
    *notices = retained;
}

fn aggregate_stp005(notices: &mut Vec<gtfs_core::Notice>) {
    let mut first_position = None;
    let mut matched = Vec::new();
    let mut retained = Vec::with_capacity(notices.len());
    for (index, notice) in notices.drain(..).enumerate() {
        if notice.rule_id == "STP_005" {
            first_position.get_or_insert(index);
            matched.push(notice);
        } else {
            retained.push(notice);
        }
    }
    if matched.is_empty() {
        *notices = retained;
        return;
    }

    let mut aggregate = matched.swap_remove(0);
    let affected_stops = matched.len() + 1;
    let mut examples: Vec<String> = aggregate.entity_id.clone().into_iter()
        .chain(matched.iter().filter_map(|notice| notice.entity_id.clone()))
        .collect();
    examples.sort_unstable();
    examples.dedup();
    examples.truncate(5);
    aggregate.entity_type = gtfs_core::EntityType::Feed;
    aggregate.entity_id = None;
    aggregate.scope_key = None;
    aggregate.file = Some("stops.txt".to_string());
    aggregate.line = None;
    aggregate.observed_value = Some(affected_stops.to_string());
    aggregate.expected_value = Some("valid longitude in [-180, 180]".to_string());
    aggregate.message = format!(
        "stops.txt içinde {affected_stops} stop için stop_lon geçersiz veya aralık dışında."
    );
    aggregate.details = Some({
        let mut details = std::collections::BTreeMap::new();
        details.insert("affected_stops".to_string(), affected_stops.to_string());
        if !examples.is_empty() {
            details.insert("example_stop_ids".to_string(), examples.join(", "));
        }
        details
    });
    aggregate.service_id = None;
    aggregate.whitespace_derived = matched.iter().all(|n| n.whitespace_derived)
        && aggregate.whitespace_derived;
    aggregate.whitespace_candidate = matched.iter().all(|n| n.whitespace_candidate)
        && aggregate.whitespace_candidate;
    retained.insert(
        first_position.unwrap_or(retained.len()).min(retained.len()),
        aggregate,
    );
    *notices = retained;
}

fn aggregate_trf005(notices: &mut Vec<gtfs_core::Notice>) {
    let mut first_position = None;
    let mut matched = Vec::new();
    let mut retained = Vec::with_capacity(notices.len());
    for (index, notice) in notices.drain(..).enumerate() {
        if notice.rule_id == "TRF_005" {
            first_position.get_or_insert(index);
            matched.push(notice);
        } else {
            retained.push(notice);
        }
    }
    if matched.is_empty() {
        *notices = retained;
        return;
    }

    let mut aggregate = matched.swap_remove(0);
    let affected_rows = matched.len() + 1;
    let mut examples: Vec<String> = aggregate.entity_id.clone().into_iter()
        .chain(matched.iter().filter_map(|notice| notice.entity_id.clone()))
        .collect();
    examples.sort_unstable();
    examples.dedup();
    examples.truncate(5);
    aggregate.entity_type = gtfs_core::EntityType::Feed;
    aggregate.entity_id = None;
    aggregate.scope_key = None;
    aggregate.file = Some("transfers.txt".to_string());
    aggregate.line = None;
    aggregate.observed_value = Some(affected_rows.to_string());
    aggregate.expected_value = Some("non-negative integer seconds".to_string());
    aggregate.message = format!(
        "transfers.txt içinde {affected_rows} satırda min_transfer_time eksik veya geçersiz."
    );
    aggregate.details = Some({
        let mut details = std::collections::BTreeMap::new();
        details.insert("affected_rows".to_string(), affected_rows.to_string());
        if !examples.is_empty() {
            details.insert("example_transfers".to_string(), examples.join(", "));
        }
        details
    });
    aggregate.service_id = None;
    aggregate.whitespace_derived = matched.iter().all(|n| n.whitespace_derived)
        && aggregate.whitespace_derived;
    aggregate.whitespace_candidate = matched.iter().all(|n| n.whitespace_candidate)
        && aggregate.whitespace_candidate;
    retained.insert(
        first_position.unwrap_or(retained.len()).min(retained.len()),
        aggregate,
    );
    *notices = retained;
}

fn aggregate_pth012(notices: &mut Vec<gtfs_core::Notice>) {
    let mut first_position = None;
    let mut matched = Vec::new();
    let mut retained = Vec::with_capacity(notices.len());
    for (index, notice) in notices.drain(..).enumerate() {
        if notice.rule_id == "PTH_012" {
            first_position.get_or_insert(index);
            matched.push(notice);
        } else {
            retained.push(notice);
        }
    }
    if matched.is_empty() {
        *notices = retained;
        return;
    }

    let mut aggregate = matched.swap_remove(0);
    let affected_platforms = matched.len() + 1;
    let mut examples: Vec<String> = aggregate.entity_id.clone().into_iter()
        .chain(matched.iter().filter_map(|notice| notice.entity_id.clone()))
        .collect();
    examples.sort_unstable();
    examples.dedup();
    examples.truncate(5);
    aggregate.entity_type = gtfs_core::EntityType::Feed;
    aggregate.entity_id = None;
    aggregate.scope_key = None;
    aggregate.file = Some("pathways.txt".to_string());
    aggregate.line = None;
    aggregate.observed_value = Some(affected_platforms.to_string());
    aggregate.expected_value = Some("every platform reachable from an entrance".to_string());
    aggregate.message = format!(
        "pathways.txt grafiğinde {affected_platforms} platform entrance üzerinden erişilemiyor."
    );
    aggregate.details = Some({
        let mut details = std::collections::BTreeMap::new();
        details.insert("affected_platforms".to_string(), affected_platforms.to_string());
        if !examples.is_empty() {
            details.insert("example_platforms".to_string(), examples.join(", "));
        }
        details
    });
    aggregate.service_id = None;
    aggregate.whitespace_derived = matched.iter().all(|n| n.whitespace_derived)
        && aggregate.whitespace_derived;
    aggregate.whitespace_candidate = matched.iter().all(|n| n.whitespace_candidate)
        && aggregate.whitespace_candidate;
    retained.insert(
        first_position.unwrap_or(retained.len()).min(retained.len()),
        aggregate,
    );
    *notices = retained;
}

pub fn apply_report_scope(
    notices: &mut Vec<gtfs_core::Notice>,
    records: &EntityRecords,
    config: &ValidatorConfig,
) {
    if notices.iter().any(|n| n.rule_id == "STP_033") && has_uniform_fare_coverage(records) {
        notices.retain(|notice| notice.rule_id != "STP_033");
    }
    let jp_scope = records.is_gtfs_jp == Some(true);
    let disabled: std::collections::HashSet<&str> = config
        .disabled_rule_ids
        .iter()
        .chain(
            jp_scope
                .then_some(config.disabled_rule_ids_gtfs_jp.iter())
                .into_iter()
                .flatten(),
        )
        .map(String::as_str)
        .collect();
    if !disabled.is_empty() {
        notices.retain(|notice| !disabled.contains(notice.rule_id.as_str()));
    }
}

/// K1–K7 tam pipeline — entegrasyon testleri ve araç entegrasyonu için.
/// WASM sürümünden farkı: notice limit yok, `today` dışarıdan verilir.
pub fn validate_bytes(zip: &[u8], config: &ValidatorConfig, today: u32) -> ValidateResult {
    use crate::timing::Timer;

    if let Err(e) = check_rule_scope(config) {
        return ValidateResult::Fatal(e);
    }

    let k1 = {
        let _t = Timer::start("K1-parse");
        match parse(zip, config) {
            Ok(r) => r,
            Err(e) => return ValidateResult::Fatal(e),
        }
    };
    let mut partial = k1.partial;
    let unavailable_files = partial.unavailable_files.clone();
    let availability = FileAvailability::from_k1(&k1.present_files, &unavailable_files);
    let has_gtfs_jp_file = GTFS_JP_FILES
        .iter()
        .any(|file| k1.present_files.contains(*file));
    let has_pattern_jp_file = k1.present_files.contains("pattern_jp.txt");
    let whitespace_root_files: std::collections::HashSet<String> = k1
        .notices
        .iter()
        .filter(|notice| notice.rule_id == "DQ_016")
        .filter_map(|notice| notice.file.clone())
        .collect();
    let mut file_stats = collect_file_stats(&k1.files);

    let mut k2 = {
        let _t = Timer::start("K2-validate");
        validate_k2_with_whitespace_roots(
            k1.files,
            Some(zip),
            config,
            None,
            Some(has_gtfs_jp_file),
            Some(&whitespace_root_files),
        )
        // #15 W2 + #38: ZIP bytes K2'ye → stop_times stream
    };
    // K1 deliberately drops unknown file bodies after recording their headers;
    // retain that lightweight inventory alongside K2's known-file headers so K4
    // can apply GTFS-JP namespace rules without reopening or retaining payloads.
    k2.records.gtfs_jp_namespace_headers.extend(
        k1.custom_file_headers
            .iter()
            .map(|(name, headers)| (name.clone(), headers.clone())),
    );
    k2.records.has_pattern_jp_file |= has_pattern_jp_file;

    // Gece yarısını aşan seferleri (00:xx) servis-günü notasyonuna (24:xx) normalize et.
    // K2, STM_048 raw Spec bulgusunu bu pass'ten önce üretir; bu pass yalnızca
    // K3–K6 türetilmiş/analitik kuralların (STM_008/014/028, headway…) tutarlı görmesini sağlar.
    // Monoton seferlerde no-op; yalnızca gece dönümü içeren seferler kayar.
    {
        let _t = Timer::start("K2-service-day-normalize");
        k2.records
            .stop_times_index
            .normalize_service_day(config.service_day_start_hour);
    }

    // Stream edilen dosyalarda K1 rows boş kalır; K2 sayaçları varsa üzerine yaz.
    // Yeni bir dosya stream edildiğinde k2/mod.rs streaming_row_counts'a eklenmesi yeterli.
    for fi in file_stats.iter_mut() {
        if let Some(&count) = k2.records.streaming_row_counts.get(fi.name.as_str()) {
            fi.rows = count as u32;
        }
    }

    let k3 = {
        let _t = Timer::start("K3-entity-map");
        let mut k3 = build_entity_map(&k2.records);
        // XFL_025: geojson feature id'leri K1'de toplandı; EntityMap'e taşı.
        k3.entity_map.geojson_location_ids = k1.geojson_location_ids.clone();
        k3.entity_map.geojson_geometries = k1.geojson_geometries.clone();
        k3
    };

    let k4 = {
        let _t = Timer::start("K4-cross-ref");
        check_cross_ref_with_whitespace_roots(
            &k2.records,
            &k3.entity_map,
            today,
            &availability,
            Some(&whitespace_root_files),
        )
    };

    // #15: trip_stop_set (büyük feed'de ~226 MB) yalnızca K4'te kullanılır; K5/K6/K7 ve
    // build_name_index kullanmaz → K4 biter bitmez serbest bırak (K6 öncesi canlı belleği düşürür).
    k2.records.stop_times_index.trip_stop_set = Default::default();

    let k5 = {
        let _t = Timer::start("K5-derived");
        build_derived_with_files(&k2.records, &k3.entity_map, &availability)
    };

    let k6 = {
        let _t = Timer::start("K6-analytics");
        analyze_k6_with_files(&k2.records, &k5.derived, config, today, &availability)
    };

    let mut all = Vec::new();
    // `skipped_checks` explains reduced coverage; an ordinary complete feed may
    // legitimately omit optional files and must not become PARTIAL merely because
    // those optional checks have no input. K1's recovery marker is the authority
    // for whether this run should expose the trace.
    if !partial.is_empty() {
        partial.extend_skipped_checks(k4.skipped_checks);
        partial.extend_skipped_checks(k5.skipped_checks);
        partial.extend_skipped_checks(k6.skipped_checks);
    }
    all.extend(k1.notices);
    all.extend(k2.notices);
    all.extend(k3.notices);
    all.extend(k4.notices);
    all.extend(k5.notices);
    all.extend(k6.notices);

    // STM_036'in K2 sequence-gerilemesi ve K6 dağınık-satır alt-vakaları aynı
    // feed-level unsorted_stop_times sinyaline aittir; kullanıcıya tek özet göster.
    aggregate_stm036(&mut all);
    aggregate_dq021(&mut all);
    aggregate_arc012(&mut all);
    aggregate_cld003(&mut all);
    aggregate_shp005(&mut all);
    aggregate_stm008(&mut all);
    aggregate_stm047(&mut all);
    aggregate_pth007(&mut all);
    aggregate_trp003(&mut all);
    aggregate_trn001(&mut all);
    aggregate_stp004(&mut all);
    aggregate_stp005(&mut all);
    aggregate_trf005(&mut all);
    aggregate_pth012(&mut all);
    apply_report_scope(&mut all, &k2.records, config);

    // issue #133 — yayın kararı ve skor, KAPSAM kaybını görmek zorunda. Zorunlu bir dosya
    // okunamadıysa ona bağlı kurallar hiç koşmamıştır; bulgu yokluğu kanıt yokluğudur.
    // ⚠️ İsteğe bağlı dosya kaybı bu bayrağı DÜŞÜRMEZ (ölçüldü: bozuk `attributions.txt` de
    // koşumu PARTIAL yapıyor) — geçerli feed'i yayından men etmek ağır taraftır.
    let coverage_complete = !partial
        .unavailable_files
        .iter()
        .any(|f| k1_parse::is_certification_critical(f))
        && partial.root_structural_errors.is_empty();

    let mut whitespace_suppressions = k2.whitespace_suppressions;
    whitespace_suppressions.merge(k4.whitespace_suppressions);

    let k7 = {
        let _t = Timer::start("K7-reporting");
        report_k7_with_suppressions(
            all,
            &k2.records,
            &k5.derived,
            file_stats,
            false,
            coverage_complete,
            whitespace_suppressions,
        )
    };

    // name_index harita verisini notice'lara göre filtreler (büyük feed modu) → notice'lar
    // ödünç verilirken taşınmamalı.
    let name_index = build_name_index(&k2.records, &k7.notices);
    ValidateResult::Ok(ValidationResult {
        status: if partial.is_empty() {
            ValidationStatus::Complete
        } else {
            ValidationStatus::Partial
        },
        partial: (!partial.is_empty()).then_some(partial),
        notices: k7.notices,
        reports: k7.reports,
        metrics: k7.metrics,
        name_index,
        capped_totals: std::collections::BTreeMap::new(),
    })
}

pub fn build_name_index(
    records: &EntityRecords,
    notices: &[gtfs_core::Notice],
) -> gtfs_core::NameIndex {
    const LARGE_FEED_TRIP_CAP: usize = 60_000;
    build_name_index_impl(records, notices, records.trips.len() > LARGE_FEED_TRIP_CAP)
}

/// [`build_name_index`]'in gövdesi; `large_feed_mode` testten zorlanabilsin diye ayrıldı
/// (eşik 60.000 sefer — birim testte o hacmi kurmak pratik değil).
pub(crate) fn build_name_index_impl(
    records: &EntityRecords,
    notices: &[gtfs_core::Notice],
    large_feed_mode: bool,
) -> gtfs_core::NameIndex {
    use std::collections::BTreeMap;

    // ── BÜYÜK FEED BELLEK MODU (#15 large_feed_memory_mode) ──────────────────────
    // Çok büyük feed'de name_index'in ağır alanları sonucu JS'e serialize ederken (to_js)
    // belleği patlatıyor (yüz MB+ JSON, 4 GB tavanını aşıp OOM). DETERMİNİSTİK eşik (entity
    // sayısı — runtime bellek DEĞİL; reprodüksiyon için).
    //
    // 2026-07-25 DÜZELTME — eşik üstünde bu map'ler TÜMÜYLE boşaltılıyordu ve gerekçe
    // "o ölçekte harita zaten çizilemez"di. Yanlıştı: harita TEK bir notice için açılıyor,
    // feed'in tamamı için değil. VBB'de (282k sefer) sonuç şuydu: shape kurallarında duraklar
    // görünmüyor, sefer/hat kurallarında harita ikonu hiç çıkmıyordu.
    //
    // Artık boşaltma yerine FİLTRE: yalnız bir notice'ta geçen varlıklar için doldurulur.
    // Bellek notice sayısıyla sınırlıdır (feed büyüklüğüyle değil) — VBB'de 282k sefer yerine
    // birkaç bin. Küçük/normal feed'de DAVRANIŞ DEĞİŞMEZ (filtre "hepsi"ne eşittir).
    const SHAPE_PT_CAP: usize = 800_000;
    // ⚠️ ID'ler UCUZ, GEOMETRİ PAHALI (2026-07-25 ölçümü). shape_coords'u notice'lara göre
    // filtreleyip serialize etmeyi denedim: VBB'de bir HAT notice'ı (OPR_001=3004) o hattın
    // tüm seferlerini, onlar da tüm shape'lerini çekince filtre "hepsi"ne dönüştü ve to_js
    // 3079 → 3591 MB (+512 MB, 4 GB tavanına tehlikeli yakın) sıçradı — 4,7M shape noktası.
    // Geometri büyük feed'de HİÇ serialize edilmez; UI tek notice için on-demand çeker
    // (requestShapeCoords + buildMapOptions yeniden çalışır). Filtre yalnız id map'lerinde:
    // trip_stops/trip_shapes/shape_trips/route_shapes birkaç yüz KB, harita ikonunu ve
    // durakları geri getiren de zaten onlar.
    let skip_shape_coords = records.shapes.len() > SHAPE_PT_CAP || large_feed_mode;

    // Notice'larda geçen TÜM id adayları (entity_id + details değerleri). Kural bazlı bilgi
    // gerektirmez: SHP_014'ün details.trip_id'si, OPR_008'in
    // bad_seg durakları hep buradan gelir. Aday havuzu sonra gerçek id kümeleriyle kesiştirilir.
    let mut cand: std::collections::HashSet<&str> = std::collections::HashSet::new();
    if large_feed_mode {
        for n in notices {
            if let Some(e) = n.entity_id.as_deref() {
                cand.insert(e);
            }
            if let Some(d) = &n.details {
                for v in d.values() {
                    // Virgülle ayrılmış id listeleri (ctx_a/ctx_b/stops/pattern_trips)
                    for part in v.split(',') {
                        let p = part.trim();
                        if !p.is_empty() {
                            cand.insert(p);
                        }
                    }
                }
            }
        }
    }
    // İlgi kümeleri — büyük feed modunda notice'lara, aksi hâlde "hepsi"ne karşılık gelir.
    let ti_f = &records.trip_interns;
    let mut want_trip: std::collections::HashSet<&str> = std::collections::HashSet::new();
    let mut want_shape: std::collections::HashSet<&str> = std::collections::HashSet::new();
    if large_feed_mode {
        // 1) Doğrudan shape entity'li notice'lar (SHP_*/GEO_006/007). Tek geçişte kesiştirilir —
        //    aday başına shapes.txt taramak O(aday × nokta) olurdu (VBB'de 4,7M nokta).
        for p in &records.shapes {
            let sid = records.shape_interns.id(p);
            if cand.contains(sid) {
                want_shape.insert(sid);
            }
        }
        // 2) Seferler: notice sefere, hattına ya da shape'ine değiyorsa tut. Shape entity'li
        //    kurallarda duraklar shape_trips → trip_stops üzerinden çizildiği için o shape'in
        //    temsilci seferi de tutulmalı (yoksa "shape var, durak yok" durumu sürerdi).
        for t in &records.trips {
            let tid = t.trip_id.as_str();
            let sid = ti_f.shape_id(t).unwrap_or("");
            // Hat adayı seferleri İÇERİ ÇEKMEZ: UI'nin hat haritası yalnız route_shapes →
            // shape çizer, durak göstermez. Çekseydi tek hat notice'ı o hattın yüzlerce
            // seferini ve shape'ini sürükler, filtre anlamını yitirirdi.
            let shape_hit = !sid.is_empty() && (cand.contains(sid) || want_shape.contains(sid));
            if cand.contains(tid) || shape_hit {
                want_trip.insert(tid);
                if !sid.is_empty() {
                    want_shape.insert(sid);
                }
            }
        }
        // 3) Hat adayları: yalnız SHAPE id'leri (sefer yok, geometri zaten serialize edilmiyor).
        //    UI hat haritasında en fazla 4 shape çiziyor → hat başına 4 ile sınırlanır.
        //    Seferler 2. adımdan sonra kararlaştığı için bu geçiş AYRI: buradan eklenen
        //    shape'ler yeni sefer sürüklemesin.
        let mut per_route: std::collections::BTreeMap<&str, u8> = std::collections::BTreeMap::new();
        for t in &records.trips {
            let rid = ti_f.route_id(t);
            if !cand.contains(rid) {
                continue;
            }
            let Some(sid) = ti_f.shape_id(t).filter(|s| !s.is_empty()) else {
                continue;
            };
            if want_shape.contains(sid) {
                continue;
            }
            let n = per_route.entry(rid).or_insert(0);
            if *n >= 4 {
                continue;
            }
            *n += 1;
            want_shape.insert(sid);
        }
    }
    let keep_trip = |id: &str| !large_feed_mode || want_trip.contains(id);
    let keep_shape = |id: &str| !large_feed_mode || want_shape.contains(id);

    let stops: BTreeMap<String, String> = records
        .stops
        .iter()
        .filter_map(|r| r.stop_name.as_ref().map(|n| (r.stop_id.clone(), n.clone())))
        .collect();

    let routes: BTreeMap<String, String> = records
        .routes
        .iter()
        .map(|r| {
            let name = r
                .route_short_name
                .as_deref()
                .or(r.route_long_name.as_deref())
                .unwrap_or("")
                .to_string();
            (r.route_id.clone(), name)
        })
        .filter(|(_, n)| !n.is_empty())
        .collect();

    let ti = &records.trip_interns;

    // Per-trip etiket map'leri (#15): büyük feed modunda atlanır → notice'lar ham trip_id gösterir.
    let trips: BTreeMap<String, String> = records
        .trips
        .iter()
        .filter(|r| keep_trip(r.trip_id.as_str()))
        .filter_map(|r| {
            ti.headsign(r)
                .map(|h| (r.trip_id.to_string(), h.to_string()))
        })
        .collect();

    let trip_routes: BTreeMap<String, String> = records
        .trips
        .iter()
        .filter(|r| keep_trip(r.trip_id.as_str()))
        .map(|r| (r.trip_id.to_string(), ti.route_id(r).to_string()))
        .collect();

    // trip_id → direction_id ("0"/"1"); yön bilgisi olmayan sefer dahil edilmez.
    let trip_directions: BTreeMap<String, String> = records
        .trips
        .iter()
        .filter(|r| keep_trip(r.trip_id.as_str()))
        .filter_map(|r| {
            r.direction_id
                .map(|d| (r.trip_id.to_string(), d.to_string()))
        })
        .collect();

    let stop_coords: BTreeMap<String, [f64; 2]> = records
        .stops
        .iter()
        .filter_map(|r| {
            if let (Some(lat), Some(lon)) = (r.stop_lat, r.stop_lon) {
                Some((r.stop_id.clone(), [lat, lon]))
            } else {
                None
            }
        })
        .collect();

    // trip_id → ilk kalkış saati "HH:MM" (büyük feed modunda atlanır)
    let trip_first_dep: BTreeMap<String, String> = records
        .stop_times_index
        .iter_trips()
        .filter(|(trip_id, _)| keep_trip(trip_id.as_str()))
        .filter_map(|(trip_id, stops)| {
            stops
                .first()
                .and_then(|s| s.departure_time())
                .map(|(h, m, _)| (trip_id.to_string(), format!("{:02}:{:02}", h % 24, m)))
        })
        .collect();

    // shape_id → benzersiz [[route_id, yön]] listesi (harita; büyük feed modunda atlanır)
    let shape_routes: BTreeMap<String, Vec<[String; 2]>> = {
        let mut seen: std::collections::HashSet<(String, String, String)> =
            std::collections::HashSet::new();
        let mut map: BTreeMap<String, Vec<[String; 2]>> = BTreeMap::new();
        for trip in &records.trips {
            let Some(shape_id) = ti.shape_id(trip) else {
                continue;
            };
            if !keep_shape(shape_id) {
                continue;
            }
            let rid = ti.route_id(trip);
            let dir = match trip.direction_id {
                Some(0) => "Gidiş",
                Some(1) => "Dönüş",
                _ => "",
            };
            let key = (shape_id.to_string(), rid.to_string(), dir.to_string());
            if seen.insert(key) {
                map.entry(shape_id.to_string())
                    .or_default()
                    .push([rid.to_string(), dir.to_string()]);
            }
        }
        map
    };

    // shape_id → sıralı [[lat, lon]] nokta listesi (harita çizimi için).
    // #15: çok büyük feed'de atlanır (serialize OOM önlemi).
    let shape_coords: BTreeMap<String, Vec<[f64; 2]>> = if skip_shape_coords {
        BTreeMap::new()
    } else {
        let mut pts: Vec<(&str, u32, f64, f64)> = records
            .shapes
            .iter()
            .filter(|s| keep_shape(records.shape_interns.id(s)))
            .filter_map(|s| {
                let lat = s.shape_pt_lat()?;
                let lon = s.shape_pt_lon()?;
                Some((
                    records.shape_interns.id(s),
                    s.shape_pt_sequence().unwrap_or(0),
                    lat,
                    lon,
                ))
            })
            .collect();
        pts.sort_unstable_by_key(|&(id, seq, _, _)| (id, seq));
        let mut map: BTreeMap<String, Vec<[f64; 2]>> = BTreeMap::new();
        for (id, _, lat, lon) in pts {
            map.entry(id.to_string()).or_default().push([lat, lon]);
        }
        map
    };

    // trip_id → shape_id (harita; büyük feed modunda atlanır)
    let trip_shapes: BTreeMap<String, String> = records
        .trips
        .iter()
        .filter(|t| keep_trip(t.trip_id.as_str()))
        .filter_map(|t| {
            ti.shape_id(t)
                .map(|s| (t.trip_id.to_string(), s.to_string()))
        })
        .collect();

    // trip_id → [stop_id, ...] stop_sequence sıralı (harita; büyük feed'de notice'a değenler)
    let trip_stops: BTreeMap<String, Vec<String>> = records
        .stop_times_index
        .iter_trips()
        .filter(|(trip_id, _)| keep_trip(trip_id.as_str()))
        .map(|(trip_id, stops)| {
            let stm_idx = &records.stop_times_index;
            let mut ids: Vec<String> = stops
                .iter()
                .filter(|s| s.stop_idx != u32::MAX)
                .map(|s| stm_idx.stop_id_of(s).to_string())
                .collect();
            ids.dedup();
            (trip_id.to_string(), ids)
        })
        .collect();

    // shape_id → ilk trip_id (harita; büyük feed modunda atlanır)
    let shape_trips: BTreeMap<String, String> = {
        let mut map: BTreeMap<String, String> = BTreeMap::new();
        for t in &records.trips {
            if let Some(shape_id) = ti.shape_id(t) {
                if !keep_shape(shape_id) || !keep_trip(t.trip_id.as_str()) {
                    continue;
                }
                map.entry(shape_id.to_string())
                    .or_insert_with(|| t.trip_id.to_string());
            }
        }
        map
    };

    // route_id → [distinct shape_ids] (terminus haritası; büyük feed modunda atlanır)
    let route_shapes: BTreeMap<String, Vec<String>> = {
        let mut map: BTreeMap<String, Vec<String>> = BTreeMap::new();
        for t in &records.trips {
            if let Some(shape_id) = ti.shape_id(t) {
                if !keep_shape(shape_id) {
                    continue;
                }
                let v = map.entry(ti.route_id(t).to_string()).or_default();
                if !v.iter().any(|s: &String| s == shape_id) {
                    v.push(shape_id.to_string());
                }
            }
        }
        map
    };

    gtfs_core::NameIndex {
        stops,
        routes,
        trips,
        trip_routes,
        trip_directions,
        stop_coords,
        trip_first_dep,
        shape_routes,
        shape_coords,
        trip_shapes,
        trip_stops,
        shape_trips,
        route_shapes,
        map_data_deferred: skip_shape_coords,
    }
}

pub fn collect_file_stats(files: &RawFiles) -> Vec<FileInfo> {
    let mut stats: Vec<FileInfo> = files
        .values()
        .map(|f| FileInfo {
            name: f.name.clone(),
            rows: f.rows.len() as u32,
            bytes: f.bytes,
        })
        .collect();
    stats.sort_by(|a, b| a.name.cmp(&b.name));
    stats
}

#[cfg(test)]
mod name_index_tests {
    use super::*;
    use crate::k2::shapes::ShapeInternTable;
    use crate::k2::stop_times::{StopTimeRecord, StopTimesIndex};
    use crate::k2::trips::TripInternTable;

    /// İki sefer (T1/T2) + iki shape (SH1/SH2); yalnız T1'in bir notice'ı var.
    fn records() -> EntityRecords {
        let mut recs = EntityRecords::default();
        let mut ti = TripInternTable::new();
        for (tid, sid) in [("T1", "SH1"), ("T2", "SH2")] {
            let ri = ti.route_ids.len() as u32;
            ti.route_ids.push("R1".into());
            let si = ti.service_ids.len() as u32;
            ti.service_ids.push("SVC".into());
            let sh = ti.shape_ids.len() as u32;
            ti.shape_ids.push(sid.into());
            recs.trips.push(crate::k2::trips::TripRecord {
                trip_id: tid.into(),
                route_idx: ri,
                service_idx: si,
                shape_idx: sh,
                headsign_idx: 0,
                short_name_idx: 0,
                block_idx: 0,
                jp_office_idx: 0,
                jp_pattern_idx: 0,
                direction_id: None,
                wheelchair_accessible: None,
                bikes_allowed: None,
                cars_allowed: None,
                safe_duration_factor: None,
                safe_duration_offset: None,
                line: 2,
            });
        }
        recs.trip_interns = ti;
        recs.stop_times_index = StopTimesIndex::from_records(&[
            StopTimeRecord {
                trip_id: "T1".into(),
                stop_id: "S1".into(),
                stop_sequence: Some(1),
                line: 2,
                ..Default::default()
            },
            StopTimeRecord {
                trip_id: "T1".into(),
                stop_id: "S2".into(),
                stop_sequence: Some(2),
                line: 3,
                ..Default::default()
            },
            StopTimeRecord {
                trip_id: "T2".into(),
                stop_id: "S3".into(),
                stop_sequence: Some(1),
                line: 4,
                ..Default::default()
            },
        ]);
        recs
    }

    fn notice_for(entity: &str) -> gtfs_core::Notice {
        gtfs_core::Notice {
            id: "n1".into(),
            rule_id: "STM_017".into(),
            severity: gtfs_core::Severity::Bilgi,
            rule_class: gtfs_core::RuleClass::Quality,
            entity_type: gtfs_core::EntityType::Trip,
            entity_id: Some(entity.to_string()),
            scope_key: None,
            file: None,
            line: None,
            field: None,
            observed_value: None,
            expected_value: None,
            details: None,
            whitespace_derived: false,
            whitespace_candidate: false,
            title: String::new(),
            message: String::new(),
            remediation: String::new(),
            blocks: vec![],
            base_effort: 1,
            service_id: None,
        }
    }

    #[test]
    fn small_feed_keeps_every_map_entry() {
        let recs = records();
        let idx = build_name_index_impl(&recs, &[], false);
        assert!(
            idx.trip_stops.contains_key("T1") && idx.trip_stops.contains_key("T2"),
            "küçük feed'de filtre uygulanmamalı"
        );
        assert!(!idx.map_data_deferred);
    }

    #[test]
    fn large_feed_keeps_only_entities_referenced_by_notices() {
        // ESKİ DAVRANIŞ: bu map'ler tümüyle BOŞTU → harita ikonu hiç çıkmıyordu.
        let recs = records();
        let idx = build_name_index_impl(&recs, &[notice_for("T1")], true);
        assert!(
            idx.trip_stops.contains_key("T1"),
            "notice'lı sefer haritada çizilebilmeli"
        );
        assert_eq!(idx.trip_stops.get("T1").map(|v| v.len()), Some(2));
        assert!(
            !idx.trip_stops.contains_key("T2"),
            "notice'sız sefer bellek için dışarıda kalmalı"
        );
        assert_eq!(idx.trip_shapes.get("T1").map(String::as_str), Some("SH1"));
        assert!(!idx.trip_shapes.contains_key("T2"));
    }

    #[test]
    fn large_feed_resolves_shape_entity_to_its_trip_stops() {
        // Shape entity'li kural (SHP_012/GEO_006): duraklar shape_trips → trip_stops
        // üzerinden çizilir; shape'in temsilci seferi de tutulmalı.
        let recs = records();
        let mut n = notice_for("SH2");
        n.rule_id = "SHP_012".into();
        let idx = build_name_index_impl(&recs, &[n], true);
        assert_eq!(
            idx.shape_trips.get("SH2").map(String::as_str),
            Some("T2"),
            "shape'in temsilci seferi olmalı"
        );
        assert!(
            idx.trip_stops.contains_key("T2"),
            "o seferin durakları olmalı (yoksa 'shape var, durak yok')"
        );
        assert!(!idx.trip_stops.contains_key("T1"), "ilgisiz sefer dışarıda");
    }

    #[test]
    fn large_feed_never_serializes_shape_geometry() {
        let mut shape_ti = ShapeInternTable::new();
        // BELLEK KORUMASI: geometri büyük feed'de JSON'a girmez (VBB'de +512 MB ölçüldü).
        // UI eksik geometriyi on-demand çeker; map_data_deferred bunun bayrağıdır.
        let mut recs = records();
        recs.shapes = vec![crate::k2::shapes::ShapePointRecord::new(
            shape_ti.intern("SH1"),
            Some(41.0),
            Some(29.0),
            Some(1),
            None,
            2,
        )];
        recs.shape_interns = shape_ti.clone();
        let idx = build_name_index_impl(&recs, &[notice_for("T1")], true);
        assert!(
            idx.shape_coords.is_empty(),
            "büyük feed'de geometri serialize edilmemeli"
        );
        assert!(
            idx.map_data_deferred,
            "UI on-demand çekebilmek için bayrağı görmeli"
        );
        // Küçük feed'de geometri yerinde:
        let small = build_name_index_impl(&recs, &[], false);
        assert!(small.shape_coords.contains_key("SH1"));
    }

    #[test]
    fn large_feed_route_notice_does_not_drag_in_trips() {
        // Hat notice'ı hattın YÜZLERCE seferini çekerse filtre anlamını yitirir (VBB'de
        // OPR_001=3004 hat notice'ı feed'in tamamını içeri almıştı).
        let recs = records();
        let mut n = notice_for("R1");
        n.rule_id = "RTS_012".into();
        let idx = build_name_index_impl(&recs, &[n], true);
        assert!(
            idx.trip_stops.is_empty(),
            "hat notice'ı sefer duraklarını çekmemeli: {:?}",
            idx.trip_stops.keys().collect::<Vec<_>>()
        );
        assert!(
            !idx.route_shapes.is_empty(),
            "ama hattın shape id'leri kalmalı (harita ikonu)"
        );
    }

    #[test]
    fn large_feed_resolves_route_entity_to_its_shapes() {
        // Hat entity'li kural (OPR_001/RTS_*): route_shapes boş kalırsa ikon hiç çıkmaz.
        let recs = records();
        let mut n = notice_for("R1");
        n.rule_id = "RTS_012".into();
        let idx = build_name_index_impl(&recs, &[n], true);
        let shapes = idx.route_shapes.get("R1").cloned().unwrap_or_default();
        assert!(
            shapes.contains(&"SH1".to_string()) && shapes.contains(&"SH2".to_string()),
            "hattın shape'leri çözülmeli: {shapes:?}"
        );
    }
}
