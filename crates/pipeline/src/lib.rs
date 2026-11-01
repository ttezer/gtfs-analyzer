pub mod agency_attribution;
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
    prepare_notices as prepare_k7_notices, report as report_k7,
    report_prepared as report_k7_prepared,
    report_with_whitespace_suppressions as report_k7_with_suppressions, K7Result,
};
use agency_attribution::AgencyResolver;
use k7_reporting::annotate_whitespace_join_provenance;
pub use recovery::FileAvailability;
pub use whitespace_suppression::WhitespaceSuppressions;

// Re-exported for integration tests
pub use gtfs_config::{CalendarOverrideRule, GtfsJpProfile, ValidatorConfig};
pub use gtfs_core::{
    FatalCode, FatalError, FileInfo, ValidateResult, ValidationResult, ValidationStatus,
};

/// Completes the two-stage proof for the `stops.txt` exemption.
///
/// K1 knows the structural validity of `locations.geojson` Features, but cannot know whether
/// those zones are actually used because large `stop_times.txt` bodies are streamed in K2.
/// After K2 completes, the exemption applies only when `stop_times.location_id` points to a
/// valid feature ID.
pub fn reconcile_flex_stop_requirement(
    k1_notices: &mut Vec<gtfs_core::Notice>,
    k2_notices: &mut Vec<gtfs_core::Notice>,
    partial: &mut gtfs_core::PartialReport,
    stops_present: bool,
    stops_empty: bool,
    valid_geojson_location_ids: &std::collections::HashSet<String>,
    records: &EntityRecords,
) {
    let has_referenced_valid_zone = records
        .stop_times_index
        .flex_map
        .values()
        .filter_map(|flex| flex.location_id.as_deref())
        .any(|id| valid_geojson_location_ids.contains(id));

    let is_stops_notice = |notice: &gtfs_core::Notice, rule_id: &str| {
        notice.rule_id == rule_id
            && (notice.file.as_deref() == Some("stops.txt")
                || notice.observed_value.as_deref() == Some("stops.txt"))
    };

    if has_referenced_valid_zone {
        // K1 already avoids ARC_004/035 because the zone is structurally valid. These
        // retains are defensive: direct/future call paths should preserve the same invariant.
        k1_notices.retain(|n| {
            !is_stops_notice(n, "ARC_004") && !is_stops_notice(n, "ARC_035")
        });
        k2_notices.retain(|n| n.rule_id != "STP_018");
        if !stops_present {
            partial.unavailable_files.retain(|f| f != "stops.txt");
        }
        return;
    }

    // Structurally valid GeoJSON that is used by no trip does not define a service location.
    // Revoke K1's temporary exemption.
    let mut counter = 0;
    if !stops_present && !k1_notices.iter().any(|n| is_stops_notice(n, "ARC_004")) {
        partial.mark_unavailable("stops.txt");
        k1_notices.push(notice_factory::build(
            "K1/K2 Flex reconciliation",
            Some("k2"),
            &mut counter,
            "ARC_004",
            gtfs_core::EntityType::Feed,
            Some("stops.txt".to_string()),
            None,
            Some("stops.txt".to_string()),
            None,
            None,
            Some("stops.txt".to_string()),
            None,
            "Required GTFS file is missing: 'stops.txt'.".to_string(),
            "Add the missing file to the feed ZIP archive.",
        ));
    } else if stops_empty
        && !k1_notices.iter().any(|n| is_stops_notice(n, "ARC_035"))
    {
        k1_notices.push(notice_factory::build(
            "K1/K2 Flex reconciliation",
            Some("k2"),
            &mut counter,
            "ARC_035",
            gtfs_core::EntityType::File,
            Some("stops.txt".to_string()),
            None,
            Some("stops.txt".to_string()),
            None,
            None,
            Some("stops.txt".to_string()),
            None,
            "Required GTFS file 'stops.txt' contains no data rows.".to_string(),
            "Add real data to the file; leaving it empty is equivalent to omitting it.",
        ));
    }
}

/// Agency/publisher metadata for the report page. Uses only `EntityRecords`; native and WASM
/// call the same function (the parity reference compares both).
pub fn build_feed_summary(records: &EntityRecords) -> gtfs_core::feed_summary::FeedSummary {
    use gtfs_core::feed_summary::{clean, AgencyContact, FeedInfoSummary, FeedSummary, MAX_AGENCIES};
    let opt = |v: &Option<String>| v.as_deref().and_then(clean);
    FeedSummary {
        agency_count: records.agencies.len() as u32,
        agencies: records
            .agencies
            .iter()
            .take(MAX_AGENCIES)
            .map(|a| AgencyContact {
                agency_id: opt(&a.agency_id),
                agency_name: a.agency_name.trim().to_string(),
                agency_url: clean(&a.agency_url),
                agency_timezone: clean(&a.agency_timezone),
                agency_lang: opt(&a.agency_lang),
                agency_phone: opt(&a.agency_phone),
                agency_email: opt(&a.agency_email),
                agency_fare_url: opt(&a.agency_fare_url),
            })
            .collect(),
        feed_info: records.feed_info.first().map(|f| FeedInfoSummary {
            feed_publisher_name: clean(&f.feed_publisher_name),
            feed_publisher_url: clean(&f.feed_publisher_url),
            feed_lang: clean(&f.feed_lang),
            feed_version: opt(&f.feed_version),
            feed_contact_email: opt(&f.feed_contact_email),
            feed_contact_url: opt(&f.feed_contact_url),
        }),
    }
}

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
                    format!("disabled_rule_ids: '{rule_id}' is not a known rule ID."),
                )
                .variant("disabled_rule_unknown")
                .param("rule_id", rule_id));
            }
            Some(meta) if meta.rule_class == gtfs_core::RuleClass::Spec => {
                return Err(FatalError::new(
                    FatalCode::InvalidInput,
                    format!(
                        "disabled_rule_ids: '{rule_id}' is a Spec rule and cannot be disabled because publication decisions depend on these rules."
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
/// 1. STP_033 is dropped when the feed has no fare_rules or proves uniform fare coverage.
/// 2. Rules listed in `config.disabled_rule_ids` are dropped.
/// 3. Rules listed in `config.disabled_rule_ids_gtfs_jp` are dropped ONLY when the
///    feed is a detected GTFS-JP feed. Japanese publishing practice makes some
///    checks meaningless there; feeds elsewhere keep them.
fn aggregate_stm036(notices: &mut Vec<gtfs_core::Notice>, resolver: &AgencyResolver) {
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
    let mut aggregate = take_representative(&mut matched, resolver);
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
        "Rows for {affected_trips} trips in stop_times.txt are not ordered by trip_id + stop_sequence."
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
fn aggregate_dq021(notices: &mut Vec<gtfs_core::Notice>, resolver: &AgencyResolver) {
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
        let mut aggregate = take_representative(&mut matches, resolver);
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
            "{affected_duplicates} duplicate primary keys detected in {file}."
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
fn aggregate_arc012(notices: &mut Vec<gtfs_core::Notice>, resolver: &AgencyResolver) {
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
        let mut aggregate = take_representative(&mut matches, resolver);
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
                "Trailing optional columns are missing in {affected_rows} rows in {file}."
            ),
            _ => format!(
                "The column count does not match the header in {affected_rows} rows in {file}."
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
fn aggregate_cld003(notices: &mut Vec<gtfs_core::Notice>, resolver: &AgencyResolver) {
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
        let mut aggregate = take_representative(&mut matches, resolver);
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
            "exception_type is missing or invalid in {affected_rows} rows in {file}."
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

fn aggregate_shp005(notices: &mut Vec<gtfs_core::Notice>, resolver: &AgencyResolver) {
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
    let mut aggregate = take_representative(&mut matched, resolver);
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
        "shape_dist_traveled decreases for {affected_shapes} shapes in shapes.txt."
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

fn aggregate_stm008(notices: &mut Vec<gtfs_core::Notice>, resolver: &AgencyResolver) {
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
        let mut aggregate = take_representative(&mut matches, resolver);
        let affected_segments = matches.len() + 1;
        aggregate.entity_type = gtfs_core::EntityType::Trip;
        aggregate.entity_id = Some(trip_id.clone());
        aggregate.scope_key = Some(trip_id.clone());
        aggregate.observed_value = Some(affected_segments.to_string());
        aggregate.expected_value = Some("monotonic trip chronology".to_string());
        aggregate.message = format!(
            "{affected_segments} backward time transitions break this trip's chronology."
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

fn aggregate_stm047(notices: &mut Vec<gtfs_core::Notice>, resolver: &AgencyResolver) {
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
        let mut aggregate = take_representative(&mut matches, resolver);
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
            "Time is missing in {affected_rows} exact-time rows for trip_id '{trip_id}'."
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

fn aggregate_pth007(notices: &mut Vec<gtfs_core::Notice>, resolver: &AgencyResolver) {
    use std::collections::BTreeMap;

    let original_len = notices.len();
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
        let mut aggregate = take_representative(&mut matches, resolver);
        let affected_rows = matches.len() + 1;
        aggregate.entity_type = gtfs_core::EntityType::Pathway;
        aggregate.entity_id = Some(pathway_id.clone());
        aggregate.scope_key = Some(pathway_id.clone());
        aggregate.line = None;
        aggregate.observed_value = Some(affected_rows.to_string());
        aggregate.expected_value = Some("positive integer seconds".to_string());
        aggregate.message = format!(
            "traversal_time is invalid in {affected_rows} rows for pathway_id '{pathway_id}'."
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

    // Rebuild by original first occurrence instead of repeatedly inserting into the
    // middle of `retained`. A feed with many distinct pathway groups otherwise turns
    // this step into O(n²) vector shifting (mdb-3215 had 23,179 PTH_007 groups).
    let mut positioned: Vec<(usize, gtfs_core::Notice)> = aggregates
        .into_iter()
        .map(|(pathway_id, aggregate)| {
            (
                first_positions
                    .get(&pathway_id)
                    .copied()
                    .unwrap_or(original_len),
                aggregate,
            )
        })
        .collect();
    positioned.sort_unstable_by_key(|(position, _)| *position);

    let mut merged = Vec::with_capacity(retained.len() + positioned.len());
    let mut retained = retained.into_iter();
    let mut positioned = positioned.into_iter().peekable();
    for position in 0..original_len {
        if positioned
            .peek()
            .is_some_and(|(first_position, _)| *first_position == position)
        {
            merged.push(positioned.next().expect("PTH_007 aggregate missing").1);
        } else if let Some(notice) = retained.next() {
            merged.push(notice);
        }
    }
    *notices = merged;
}

fn aggregate_trp003(notices: &mut Vec<gtfs_core::Notice>, resolver: &AgencyResolver) {
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
    if matched.iter().any(|notice| {
        notice
            .details
            .as_ref()
            .and_then(|details| details.get("whitespace_join_file"))
            .is_some()
    }) {
        retained.extend(matched);
        *notices = retained;
        return;
    }

    let mut aggregate = take_representative(&mut matched, resolver);
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
        "service_id is not defined in the calendar for {affected_trips} trips in trips.txt."
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

fn aggregate_trn001(notices: &mut Vec<gtfs_core::Notice>, resolver: &AgencyResolver) {
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

    let mut aggregate = take_representative(&mut matched, resolver);
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
        "table_name is invalid in {affected_rows} rows in translations.txt."
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

fn aggregate_stp004(notices: &mut Vec<gtfs_core::Notice>, resolver: &AgencyResolver) {
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
    if matched.first().is_some_and(|first| {
        matched.iter().any(|notice| {
            notice.whitespace_derived != first.whitespace_derived
                || notice.whitespace_candidate != first.whitespace_candidate
        })
    }) {
        retained.extend(matched);
        *notices = retained;
        return;
    }

    let mut aggregate = take_representative(&mut matched, resolver);
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
        "stop_lat is not numeric for {affected_stops} stops in stops.txt."
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

fn aggregate_stp005(notices: &mut Vec<gtfs_core::Notice>, resolver: &AgencyResolver) {
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

    let mut aggregate = take_representative(&mut matched, resolver);
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
        "stop_lon is invalid or out of range for {affected_stops} stops in stops.txt."
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

fn aggregate_trf005(notices: &mut Vec<gtfs_core::Notice>, resolver: &AgencyResolver) {
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

    let mut aggregate = take_representative(&mut matched, resolver);
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
        "min_transfer_time is missing or invalid in {affected_rows} rows in transfers.txt."
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

fn aggregate_pth012(notices: &mut Vec<gtfs_core::Notice>, resolver: &AgencyResolver) {
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

    let mut aggregate = take_representative(&mut matched, resolver);
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
        "{affected_platforms} platforms in the pathways.txt graph are unreachable through an entrance."
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

fn aggregate_stp042(notices: &mut Vec<gtfs_core::Notice>, resolver: &AgencyResolver) {
    let mut first_position = None;
    let mut matched = Vec::new();
    let mut retained = Vec::with_capacity(notices.len());
    for (index, notice) in notices.drain(..).enumerate() {
        if notice.rule_id == "STP_042" {
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
    let mut aggregate = take_representative(&mut matched, resolver);
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
    aggregate.expected_value = Some("valid http/https URL".to_string());
    aggregate.message = format!("stop_url is invalid for {affected_stops} stops in stops.txt.");
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
    retained.insert(first_position.unwrap_or(retained.len()).min(retained.len()), aggregate);
    *notices = retained;
}

fn aggregate_stp032(notices: &mut Vec<gtfs_core::Notice>, resolver: &AgencyResolver) {
    let mut first_position = None;
    let mut matched = Vec::new();
    let mut retained = Vec::with_capacity(notices.len());
    for (index, notice) in notices.drain(..).enumerate() {
        if notice.rule_id == "STP_032" {
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
    let mut aggregate = take_representative(&mut matched, resolver);
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
    aggregate.file = Some("stops.txt".to_string());
    aggregate.line = None;
    aggregate.observed_value = Some(affected_platforms.to_string());
    aggregate.expected_value = Some("parent_station defined".to_string());
    aggregate.message = format!(
        "parent_station is missing for {affected_platforms} platforms connected to the pathway network."
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
    retained.insert(first_position.unwrap_or(retained.len()).min(retained.len()), aggregate);
    *notices = retained;
}

/// FAR_010 is emitted for every raw conflicting `fare_rules` row (846,630 in one corpus feed).
/// The registry uses `Feed` deduplication: without aggregation, dedup kept one arbitrary row
/// and the conflict count was lost. Keep one summary with the count and example pairs.
fn aggregate_far010(notices: &mut Vec<gtfs_core::Notice>, resolver: &AgencyResolver) {
    let mut first_position = None;
    let mut matched = Vec::new();
    let mut retained = Vec::with_capacity(notices.len());
    for (index, notice) in notices.drain(..).enumerate() {
        if notice.rule_id == "FAR_010" {
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
    let affected_rows = matched.len();
    let mut examples: Vec<String> = matched
        .iter()
        .filter_map(|notice| {
            let fare = notice.observed_value.as_deref()?;
            let other = notice.expected_value.as_deref()?;
            Some(format!("{fare} ↔ {other}"))
        })
        .collect();
    examples.sort_unstable();
    examples.dedup();
    examples.truncate(5);
    let mut aggregate = take_representative(&mut matched, resolver);
    aggregate.entity_type = gtfs_core::EntityType::Feed;
    aggregate.entity_id = None;
    aggregate.scope_key = None;
    aggregate.file = Some("fare_rules.txt".to_string());
    aggregate.line = None;
    aggregate.field = Some("fare_id".to_string());
    aggregate.observed_value = Some(affected_rows.to_string());
    aggregate.expected_value = None;
    aggregate.message = format!(
        "{affected_rows} rules in fare_rules.txt define the same condition set in a way that conflicts with another fare."
    );
    aggregate.details = Some({
        let mut details = std::collections::BTreeMap::new();
        details.insert("affected_rows".to_string(), affected_rows.to_string());
        if !examples.is_empty() {
            details.insert("example_fares".to_string(), examples.join(", "));
        }
        details
    });
    aggregate.service_id = None;
    aggregate.whitespace_derived = matched.iter().all(|n| n.whitespace_derived)
        && aggregate.whitespace_derived;
    aggregate.whitespace_candidate = matched.iter().all(|n| n.whitespace_candidate)
        && aggregate.whitespace_candidate;
    retained.insert(first_position.unwrap_or(retained.len()).min(retained.len()), aggregate);
    *notices = retained;
}

fn aggregate_trp005(notices: &mut Vec<gtfs_core::Notice>, resolver: &AgencyResolver) {
    let mut first_position = None;
    let mut matched = Vec::new();
    let mut retained = Vec::with_capacity(notices.len());
    for (index, notice) in notices.drain(..).enumerate() {
        if notice.rule_id == "TRP_005" {
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
    let mut aggregate = take_representative(&mut matched, resolver);
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
    aggregate.file = Some("trips.txt".to_string());
    aggregate.line = None;
    aggregate.observed_value = Some(affected_trips.to_string());
    aggregate.expected_value = Some("direction_id 0 or 1".to_string());
    aggregate.message = format!("direction_id is invalid in {affected_trips} trips in trips.txt.");
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
    retained.insert(first_position.unwrap_or(retained.len()).min(retained.len()), aggregate);
    *notices = retained;
}

fn aggregate_stm022(notices: &mut Vec<gtfs_core::Notice>, resolver: &AgencyResolver) {
    let mut first_position = None;
    let mut matched = Vec::new();
    let mut retained = Vec::with_capacity(notices.len());
    for (index, notice) in notices.drain(..).enumerate() {
        if notice.rule_id == "STM_022" {
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
    let mut aggregate = take_representative(&mut matched, resolver);
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
    aggregate.file = Some("stop_times.txt".to_string());
    aggregate.line = None;
    aggregate.observed_value = Some(affected_rows.to_string());
    aggregate.expected_value = Some("timepoint 0 or 1".to_string());
    aggregate.message = format!("timepoint is invalid in {affected_rows} rows in stop_times.txt.");
    aggregate.details = Some({
        let mut details = std::collections::BTreeMap::new();
        details.insert("affected_rows".to_string(), affected_rows.to_string());
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
    retained.insert(first_position.unwrap_or(retained.len()).min(retained.len()), aggregate);
    *notices = retained;
}

fn aggregate_frq007(notices: &mut Vec<gtfs_core::Notice>, resolver: &AgencyResolver) {
    let mut first_position = None;
    let mut matched = Vec::new();
    let mut retained = Vec::with_capacity(notices.len());
    for (index, notice) in notices.drain(..).enumerate() {
        if notice.rule_id == "FRQ_007" {
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
    let mut aggregate = take_representative(&mut matched, resolver);
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
    aggregate.file = Some("frequencies.txt".to_string());
    aggregate.line = None;
    aggregate.field = Some("exact_times".to_string());
    aggregate.observed_value = Some(affected_trips.to_string());
    aggregate.expected_value = Some("exact_times 0 or 1".to_string());
    aggregate.message = format!("exact_times is invalid in {affected_trips} trips in frequencies.txt.");
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
    retained.insert(first_position.unwrap_or(retained.len()).min(retained.len()), aggregate);
    *notices = retained;
}

fn aggregate_trf019(notices: &mut Vec<gtfs_core::Notice>, resolver: &AgencyResolver) {
    let mut first_position = None;
    let mut matched = Vec::new();
    let mut retained = Vec::with_capacity(notices.len());
    for (index, notice) in notices.drain(..).enumerate() {
        if notice.rule_id == "TRF_019" {
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
    let mut aggregate = take_representative(&mut matched, resolver);
    let affected_transfers = matched.len() + 1;
    let mut examples: Vec<String> = aggregate.observed_value.clone().into_iter()
        .chain(matched.iter().filter_map(|notice| notice.observed_value.clone()))
        .collect();
    examples.sort_unstable();
    examples.dedup();
    examples.truncate(5);
    aggregate.entity_type = gtfs_core::EntityType::Feed;
    aggregate.entity_id = None;
    aggregate.scope_key = None;
    aggregate.file = Some("transfers.txt".to_string());
    aggregate.line = None;
    aggregate.field = Some("route_type".to_string());
    aggregate.observed_value = Some(affected_transfers.to_string());
    aggregate.expected_value = Some("from_trip_id and to_trip_id have the same route_type".to_string());
    aggregate.message = format!("route_type is inconsistent in {affected_transfers} in-seat transfers in transfers.txt.");
    aggregate.details = Some({
        let mut details = std::collections::BTreeMap::new();
        details.insert("affected_transfers".to_string(), affected_transfers.to_string());
        if !examples.is_empty() {
            details.insert("example_route_types".to_string(), examples.join(", "));
        }
        details
    });
    aggregate.service_id = None;
    aggregate.whitespace_derived = matched.iter().all(|n| n.whitespace_derived)
        && aggregate.whitespace_derived;
    aggregate.whitespace_candidate = matched.iter().all(|n| n.whitespace_candidate)
        && aggregate.whitespace_candidate;
    retained.insert(first_position.unwrap_or(retained.len()).min(retained.len()), aggregate);
    *notices = retained;
}

fn aggregate_cal008(notices: &mut Vec<gtfs_core::Notice>, resolver: &AgencyResolver) {
    let mut groups: std::collections::BTreeMap<String, Vec<gtfs_core::Notice>> =
        std::collections::BTreeMap::new();
    let mut first_positions = std::collections::BTreeMap::new();
    let mut retained = Vec::with_capacity(notices.len());
    for (index, notice) in notices.drain(..).enumerate() {
        if notice.rule_id == "CAL_008" {
            let date = notice.observed_value.clone().unwrap_or_default();
            first_positions.entry(date.clone()).or_insert(index);
            groups.entry(date).or_default().push(notice);
        } else {
            retained.push(notice);
        }
    }
    if groups.is_empty() {
        *notices = retained;
        return;
    }
    let mut aggregates = Vec::with_capacity(groups.len());
    for (end_date, mut matches) in groups {
        let mut aggregate = take_representative(&mut matches, resolver);
        let affected_services = matches.len() + 1;
        let mut examples: Vec<String> = aggregate.entity_id.clone().into_iter()
            .chain(matches.iter().filter_map(|notice| notice.entity_id.clone()))
            .collect();
        examples.sort_unstable();
        examples.dedup();
        examples.truncate(5);
        aggregate.entity_type = gtfs_core::EntityType::Feed;
        aggregate.entity_id = None;
        aggregate.scope_key = None;
        aggregate.file = Some("calendar.txt".to_string());
        aggregate.line = None;
        aggregate.field = Some("end_date".to_string());
        aggregate.observed_value = Some(end_date.clone());
        aggregate.message = format!(
        "end_date for {affected_services} services in calendar.txt ends on {end_date}."
        );
        aggregate.details = Some({
            let mut details = std::collections::BTreeMap::new();
            details.insert("affected_services".to_string(), affected_services.to_string());
            if !examples.is_empty() {
                details.insert("example_services".to_string(), examples.join(", "));
            }
            details
        });
        aggregate.service_id = None;
        aggregate.whitespace_derived = matches.iter().all(|n| n.whitespace_derived)
            && aggregate.whitespace_derived;
        aggregate.whitespace_candidate = matches.iter().all(|n| n.whitespace_candidate)
            && aggregate.whitespace_candidate;
        aggregates.push((end_date, aggregate));
    }
    for (end_date, aggregate) in aggregates.into_iter().rev() {
        let insert_at = first_positions
            .get(&end_date)
            .copied()
            .unwrap_or(retained.len())
            .min(retained.len());
        retained.insert(insert_at, aggregate);
    }
    *notices = retained;
}

fn aggregate_ggl001(notices: &mut Vec<gtfs_core::Notice>, resolver: &AgencyResolver) {
    let mut first_position = None;
    let mut matched = Vec::new();
    let mut retained = Vec::with_capacity(notices.len());
    for (index, notice) in notices.drain(..).enumerate() {
        if notice.rule_id == "GGL_001" {
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
    let mut aggregate = take_representative(&mut matched, resolver);
    let affected_transfers = matched.len() + 1;
    let mut types: Vec<String> = aggregate.observed_value.clone().into_iter()
        .chain(matched.iter().filter_map(|notice| notice.observed_value.clone()))
        .collect();
    types.sort_unstable();
    types.dedup();
    aggregate.entity_type = gtfs_core::EntityType::Feed;
    aggregate.entity_id = None;
    aggregate.scope_key = None;
    aggregate.file = Some("transfers.txt".to_string());
    aggregate.line = None;
    aggregate.field = Some("transfer_type".to_string());
    aggregate.observed_value = Some(affected_transfers.to_string());
    aggregate.expected_value = Some("0, 1, 2 veya 3".to_string());
    aggregate.message = format!("{affected_transfers} records in transfers.txt are ignored by Google Transit.");
    aggregate.details = Some({
        let mut details = std::collections::BTreeMap::new();
        details.insert("affected_transfers".to_string(), affected_transfers.to_string());
        details.insert("transfer_types".to_string(), types.join(", "));
        details
    });
    aggregate.service_id = None;
    aggregate.whitespace_derived = matched.iter().all(|n| n.whitespace_derived)
        && aggregate.whitespace_derived;
    aggregate.whitespace_candidate = matched.iter().all(|n| n.whitespace_candidate)
        && aggregate.whitespace_candidate;
    retained.insert(first_position.unwrap_or(retained.len()).min(retained.len()), aggregate);
    *notices = retained;
}

pub fn apply_report_scope(
    notices: &mut Vec<gtfs_core::Notice>,
    records: &EntityRecords,
    config: &ValidatorConfig,
) {
    // `zone_id` is Optional in the spec; its only consumers are fare_rules zone columns.
    // Without fare_rules, a missing zone cannot break fare calculations (including free or
    // single-fare feeds).
    if notices.iter().any(|n| n.rule_id == "STP_033")
        && (records.fare_rules.is_empty() || has_uniform_fare_coverage(records))
    {
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

/// Full K1–K7 pipeline for integration tests and tool integration.
/// Unlike the WASM version: there is no notice limit, and `today` is supplied externally.
pub fn validate_bytes(zip: &[u8], config: &ValidatorConfig, today: u32) -> ValidateResult {
    validate_bytes_inspected(zip, config, today, &mut |_, _, _, _| {})
}

/// Observer supplied to [`validate_bytes_inspected`]: raw K1–K6 notices and the records,
/// derived data, and whitespace-suppression lists needed to interpret them.
#[doc(hidden)]
pub type NoticeInspector<'a> =
    dyn FnMut(&[gtfs_core::Notice], &EntityRecords, &DerivedData, &WhitespaceSuppressions) + 'a;

/// The same pipeline as [`validate_bytes`]; `inspect` sees raw K1–K6 notices before feed-level
/// aggregation and report scoping. For audit tools (e.g. [`examples/scope_audit.rs`):
/// aggregation removes `scope_key`/`entity_id`.
#[doc(hidden)]
pub fn validate_bytes_inspected(
    zip: &[u8],
    config: &ValidatorConfig,
    today: u32,
    inspect: &mut NoticeInspector<'_>,
) -> ValidateResult {
    use crate::timing::Timer;

    if let Err(e) = check_rule_scope(config) {
        return ValidateResult::Fatal(e);
    }

    let mut k1 = {
        let _t = Timer::start("K1-parse");
        match parse(zip, config) {
            Ok(r) => r,
            Err(e) => return ValidateResult::Fatal(e),
        }
    };
    crate::timing::mem_log("after K1-parse");
    let mut partial = k1.partial;
    let stops_present = k1.present_files.contains("stops.txt");
    let stops_empty = k1
        .files
        .get("stops.txt")
        .is_some_and(|file| file.rows.is_empty());
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
    crate::timing::mem_log("after K2-validate");
    reconcile_flex_stop_requirement(
        &mut k1.notices,
        &mut k2.notices,
        &mut partial,
        stops_present,
        stops_empty,
        &k1.valid_geojson_location_ids,
        &k2.records,
    );
    let unavailable_files = partial.unavailable_files.clone();
    let availability = FileAvailability::from_k1(&k1.present_files, &unavailable_files);
    // K1 deliberately drops unknown file bodies after recording their headers;
    // retain that lightweight inventory alongside K2's known-file headers so K4
    // can apply GTFS-JP namespace rules without reopening or retaining payloads.
    k2.records.gtfs_jp_namespace_headers.extend(
        k1.custom_file_headers
            .iter()
            .map(|(name, headers)| (name.clone(), headers.clone())),
    );
    k2.records.has_pattern_jp_file |= has_pattern_jp_file;

    // Normalize trips crossing midnight (00:xx) into service-day notation (24:xx).
    // K2 emits the STM_048 raw Spec finding before this pass; this pass only ensures that
    // derived/analytic K3–K6 rules (STM_008/014/028, headway…) see consistent values.
    // No-op for monotonic trips; only trips containing a midnight rollover are shifted.
    {
        let _t = Timer::start("K2-service-day-normalize");
        k2.records
            .stop_times_index
            .normalize_service_day(config.service_day_start_hour);
    }

    // K1 rows remain empty for streamed files; overwrite them when K2 has counters.
    // When a new file is streamed, adding it to k2/mod.rs streaming_row_counts is sufficient.
    for fi in file_stats.iter_mut() {
        if let Some(&count) = k2.records.streaming_row_counts.get(fi.name.as_str()) {
            fi.rows = count as u32;
        }
    }

    let k3 = {
        let _t = Timer::start("K3-entity-map");
        let mut k3 = build_entity_map(&k2.records);
        // XFL_025: GeoJSON feature IDs were collected in K1; move them into EntityMap.
        k3.entity_map.geojson_location_ids = k1.geojson_location_ids.clone();
        k3.entity_map.geojson_geometries = k1.geojson_geometries.clone();
        k3
    };
    crate::timing::mem_log("after K3-entity-map");

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
    crate::timing::mem_log("after K4-cross-ref");

    // #15: trip_stop_set (~226 MB on a large feed) is used only in K4; K5/K6/K7 and
    // build_name_index do not use it → release it as soon as K4 ends (reduces live memory before K6).
    k2.records.stop_times_index.trip_stop_set = Default::default();

    let k5 = {
        let _t = Timer::start("K5-derived");
        build_derived_with_files(&k2.records, &k3.entity_map, &availability)
    };
    crate::timing::mem_log("after K5-derived");

    let k6 = {
        let _t = Timer::start("K6-analytics");
        analyze_k6_with_files(&k2.records, &k5.derived, config, today, &availability)
    };
    crate::timing::mem_log("after K6-analytics");

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

    let mut whitespace_suppressions = k2.whitespace_suppressions;
    whitespace_suppressions.merge(k4.whitespace_suppressions);

    inspect(&all, &k2.records, &k5.derived, &whitespace_suppressions);
    aggregate_feed_level_notices(&mut all, &k2.records, &k5.derived);
    apply_report_scope(&mut all, &k2.records, config);

    // issue #133 — the publication decision and score must expose coverage loss. If a required
    // file could not be read, its dependent rules never ran; absence of findings is not proof.
    // ⚠️ Optional-file loss does not LOWER this flag (measured: a corrupt `attributions.txt`
    // also makes the run PARTIAL) — blocking a valid feed from publication is the heavier error.
    let coverage_complete = !partial
        .unavailable_files
        .iter()
        .any(|f| k1_parse::is_certification_critical(f))
        && partial.root_structural_errors.is_empty();

    let (k7, agency_breakdown) = {
        let _t = Timer::start("K7-reporting");
        let prepared =
            prepare_k7_notices(all, &k2.records, &k5.derived, false, whitespace_suppressions);
        // #2201: counted AFTER suppression; native has no cap, so the inventory is complete.
        let agency_breakdown = {
            let _t = Timer::start("K7::agency_breakdown");
            agency_attribution::breakdown(&prepared, &k2.records, true)
        };
        let k7 = report_k7_prepared(prepared, &k2.records, &k5.derived, file_stats, coverage_complete);
        let mut agency_breakdown = agency_breakdown;
        agency_attribution::index_notices(&mut agency_breakdown, &k7.notices, &k2.records);
        (k7, agency_breakdown)
    };
    crate::timing::mem_log("after K7-reporting");

    // name_index filters map data by notice (large-feed mode) → notices must not be moved
    // while they are borrowed.
    let name_index = build_name_index(&k2.records, &k7.notices);
    crate::timing::mem_log("after name-index");
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
        agency_breakdown,
        feed_summary: build_feed_summary(&k2.records),
    })
}

/// Selects the group's representative and writes the agency distribution of ALL raw notices
/// in the group to it (#2201). Called BEFORE aggregation removes the representative's
/// `scope_key`/`entity_id`; the distribution total is the group size, i.e. the affected-record
/// count carried by the summary.
fn take_representative(
    group: &mut Vec<gtfs_core::Notice>,
    resolver: &AgencyResolver,
) -> gtfs_core::Notice {
    let mut distribution = gtfs_core::agency::AgencyDistribution::new();
    for notice in group.iter() {
        *distribution.entry(resolver.attribute_member(notice)).or_default() += 1;
    }
    let mut representative = group.swap_remove(0);
    representative.agency_distribution = Some(Box::new(distribution));
    representative
}

/// Reduces K1–K6 notices to feed-level summaries. Native `validate_bytes` and WASM orchestrators
/// MUST call the SAME function before `apply_report_scope`: the registry defines these rules at
/// `Feed` dedup level, and locale text reads `observed_value` as the affected-record count.
/// If aggregation is skipped, dedup selects one raw-row notice and the count is lost.
pub fn aggregate_feed_level_notices(
    all: &mut Vec<gtfs_core::Notice>,
    records: &EntityRecords,
    derived: &DerivedData,
) {
    // K7's whitespace-join resolution operates through entity_id; mark which file contains
    // the padding before feed-level aggregation removes it.
    annotate_whitespace_join_provenance(all, records, derived);
    let resolver = AgencyResolver::new(records);

    // STM_036's K2 sequence regression and K6 scattered-row subcases share the same
    // feed-level unsorted_stop_times signal; show one summary to the user.
    macro_rules! timed_aggregate {
        ($name:literal, $function:ident) => {{
            let _t = crate::timing::Timer::start(concat!("aggregate::", $name));
            $function(all, &resolver);
        }};
    }
    timed_aggregate!("STM_036", aggregate_stm036);
    timed_aggregate!("DQ_021", aggregate_dq021);
    timed_aggregate!("ARC_012", aggregate_arc012);
    timed_aggregate!("CLD_003", aggregate_cld003);
    timed_aggregate!("SHP_005", aggregate_shp005);
    timed_aggregate!("STM_008", aggregate_stm008);
    timed_aggregate!("STM_047", aggregate_stm047);
    timed_aggregate!("PTH_007", aggregate_pth007);
    timed_aggregate!("TRP_003", aggregate_trp003);
    timed_aggregate!("TRN_001", aggregate_trn001);
    timed_aggregate!("STP_004", aggregate_stp004);
    timed_aggregate!("STP_005", aggregate_stp005);
    timed_aggregate!("TRF_005", aggregate_trf005);
    timed_aggregate!("PTH_012", aggregate_pth012);
    timed_aggregate!("STP_042", aggregate_stp042);
    timed_aggregate!("STP_032", aggregate_stp032);
    timed_aggregate!("TRP_005", aggregate_trp005);
    timed_aggregate!("STM_022", aggregate_stm022);
    timed_aggregate!("FRQ_007", aggregate_frq007);
    timed_aggregate!("TRF_019", aggregate_trf019);
    timed_aggregate!("CAL_008", aggregate_cal008);
    timed_aggregate!("GGL_001", aggregate_ggl001);
    timed_aggregate!("FAR_010", aggregate_far010);
}

pub fn build_name_index(
    records: &EntityRecords,
    notices: &[gtfs_core::Notice],
) -> gtfs_core::NameIndex {
    const LARGE_FEED_TRIP_CAP: usize = 60_000;
    build_name_index_impl(records, notices, records.trips.len() > LARGE_FEED_TRIP_CAP)
}

/// The body of [`build_name_index`] is split out so `large_feed_mode` can be forced by tests
/// (60,000-trip threshold — building that volume in a unit test is impractical).
pub(crate) fn build_name_index_impl(
    records: &EntityRecords,
    notices: &[gtfs_core::Notice],
    large_feed_mode: bool,
) -> gtfs_core::NameIndex {
    use std::collections::BTreeMap;

    // ── LARGE-FEED MEMORY MODE (#15 large_feed_memory_mode) ──────────────────────
    // On a very large feed, name_index's heavy fields caused JS serialization (to_js) to
    // exhaust memory (100+ MB JSON, exceeding the 4 GB ceiling and causing OOM). DETERMINISTIC
    // threshold (entity count — NOT runtime memory; for reproducibility).
    //
    // 2026-07-25 FIX — above the threshold these maps were emptied ENTIRELY, justified by
    // \"the map cannot be drawn at that scale\". That was wrong: the map opens for ONE notice,
    // not the entire feed. On VBB (282k trips), shape rules showed no stops and trip/route
    // rules showed no map icon.
    //
    // Now FILTER instead of emptying: populate only entities mentioned by a notice.
    // Memory is bounded by notice count (not feed size) — a few thousand instead of 282k trips
    // on VBB. Small/normal feeds have NO BEHAVIOR CHANGE (the filter equals \"all\").
    const SHAPE_PT_CAP: usize = 800_000;
    // ⚠️ IDs are CHEAP, GEOMETRY IS EXPENSIVE (2026-07-25 measurement). I tried filtering and
    // serializing shape_coords by notice: on VBB, one ROUTE notice (OPR_001=3004) pulled all
    // trips on that route and all their shapes, so the filter became \"all\" and to_js jumped
    // 3079 → 3591 MB (+512 MB, dangerously near the 4 GB ceiling) — 4.7M shape points.
    // Geometry is NEVER serialized for large feeds; the UI fetches it on demand for one notice
    // (requestShapeCoords + buildMapOptions run again). Filter only ID maps:
    // trip_stops/trip_shapes/shape_trips/route_shapes are a few hundred KB, and they restore
    // the map icon and stops.
    let skip_shape_coords = records.shapes.len() > SHAPE_PT_CAP || large_feed_mode;

    // ALL ID candidates mentioned by notices (entity_id + details values). No rule-specific
    // knowledge is needed: SHP_014's details.trip_id and OPR_008's bad_seg stops all arrive here.
    // The candidate pool is then intersected with real ID sets.
    let mut cand: std::collections::HashSet<&str> = std::collections::HashSet::new();
    if large_feed_mode {
        for n in notices {
            if let Some(e) = n.entity_id.as_deref() {
                cand.insert(e);
            }
            if let Some(d) = &n.details {
                for v in d.values() {
                    // Comma-separated ID lists (ctx_a/ctx_b/stops/pattern_trips)
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
    // Interest sets — notices in large-feed mode, \"all\" otherwise.
    let ti_f = &records.trip_interns;
    let mut want_trip: std::collections::HashSet<&str> = std::collections::HashSet::new();
    let mut want_shape: std::collections::HashSet<&str> = std::collections::HashSet::new();
    if large_feed_mode {
        // 1) Notices with a direct shape entity (SHP_*/GEO_006/007). Intersect in one pass —
        //    scanning shapes.txt per candidate would be O(candidate × point) (4.7M points on VBB).
        for p in &records.shapes {
            let sid = records.shape_interns.id(p);
            if cand.contains(sid) {
                want_shape.insert(sid);
            }
        }
        // 2) Trips: keep a trip if the notice touches its trip, route, or shape. For
        //    shape-entity rules, stops are drawn through shape_trips → trip_stops, so the
        //    representative trip for that shape must also be kept (otherwise \"shape exists,
        //    no stops\" would remain).
        for t in &records.trips {
            let tid = t.trip_id.as_str();
            let sid = ti_f.shape_id(t).unwrap_or("");
            // Route candidates do NOT pull in trips: the UI route map draws only route_shapes →
            // shapes, not stops. If it did, one route notice would pull hundreds of trips and
            // shapes from that route, destroying the filter's meaning.
            let shape_hit = !sid.is_empty() && (cand.contains(sid) || want_shape.contains(sid));
            if cand.contains(tid) || shape_hit {
                want_trip.insert(tid);
                if !sid.is_empty() {
                    want_shape.insert(sid);
                }
            }
        }
        // 3) Route candidates: SHAPE IDs only (no trips; geometry is not serialized anyway).
        //    The UI draws at most 4 shapes on a route map → limit to 4 per route.
        //    Trips are decided in step 2, so this pass is SEPARATE: shapes added here must not
        //    pull in new trips.
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

    // Per-trip label maps (#15): skipped in large-feed mode → notices show raw trip_id values.
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

    // trip_id → direction_id ("0"/"1"); exclude trips without direction information.
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

    // trip_id → first departure time "HH:MM" (skipped in large-feed mode)
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

    // shape_id → unique [[route_id, direction]] list (map; skipped in large-feed mode)
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
                Some(0) => "Outbound",
                Some(1) => "Inbound",
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

    // shape_id → ordered [[lat, lon]] point list (for map drawing).
    // #15: skipped for very large feeds (prevents serialization OOM).
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

    // trip_id → shape_id (map; skipped in large-feed mode)
    let trip_shapes: BTreeMap<String, String> = records
        .trips
        .iter()
        .filter(|t| keep_trip(t.trip_id.as_str()))
        .filter_map(|t| {
            ti.shape_id(t)
                .map(|s| (t.trip_id.to_string(), s.to_string()))
        })
        .collect();

    // trip_id → [stop_id, ...] ordered by stop_sequence (map; notices touching it on large feeds)
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

    // shape_id → first trip_id (map; skipped in large-feed mode)
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

    // route_id → [distinct shape_ids] (terminus map; skipped in large-feed mode)
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

    /// Two trips (T1/T2) + two shapes (SH1/SH2); only T1 has a notice.
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
            agency_distribution: None,
        }
    }

    #[test]
    fn small_feed_keeps_every_map_entry() {
        let recs = records();
        let idx = build_name_index_impl(&recs, &[], false);
        assert!(
            idx.trip_stops.contains_key("T1") && idx.trip_stops.contains_key("T2"),
            "small feeds must not be filtered"
        );
        assert!(!idx.map_data_deferred);
    }

    #[test]
    fn large_feed_keeps_only_entities_referenced_by_notices() {
        // OLD BEHAVIOR: these maps were entirely EMPTY → no map icon appeared.
        let recs = records();
        let idx = build_name_index_impl(&recs, &[notice_for("T1")], true);
        assert!(
            idx.trip_stops.contains_key("T1"),
            "a trip with a notice must be drawable on the map"
        );
        assert_eq!(idx.trip_stops.get("T1").map(|v| v.len()), Some(2));
        assert!(
            !idx.trip_stops.contains_key("T2"),
            "a trip without a notice must be excluded to save memory"
        );
        assert_eq!(idx.trip_shapes.get("T1").map(String::as_str), Some("SH1"));
        assert!(!idx.trip_shapes.contains_key("T2"));
    }

    #[test]
    fn large_feed_resolves_shape_entity_to_its_trip_stops() {
        // Shape-entity rule (SHP_012/GEO_006): stops are drawn through shape_trips → trip_stops
        // through them; the representative trip for the shape must also be kept.
        let recs = records();
        let mut n = notice_for("SH2");
        n.rule_id = "SHP_012".into();
        let idx = build_name_index_impl(&recs, &[n], true);
        assert_eq!(
            idx.shape_trips.get("SH2").map(String::as_str),
            Some("T2"),
            "the shape must retain its representative trip"
        );
        assert!(
            idx.trip_stops.contains_key("T2"),
            "that trip's stops must be present (otherwise 'shape exists, no stops')"
        );
        assert!(!idx.trip_stops.contains_key("T1"), "unrelated trip is excluded");
    }

    #[test]
    fn large_feed_never_serializes_shape_geometry() {
        let mut shape_ti = ShapeInternTable::new();
        // MEMORY PROTECTION: geometry is not put into JSON for large feeds (+512 MB measured on VBB).
        // The UI fetches missing geometry on demand; map_data_deferred is the flag for this.
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
            "geometry must not be serialized for large feeds"
        );
        assert!(
            idx.map_data_deferred,
            "the UI must see the flag so it can fetch on demand"
        );
        // Geometry remains in place for small feeds:
        let small = build_name_index_impl(&recs, &[], false);
        assert!(small.shape_coords.contains_key("SH1"));
    }

    #[test]
    fn large_feed_route_notice_does_not_drag_in_trips() {
        // If a route notice pulls HUNDREDS of trips from the route, the filter loses meaning
        // (on VBB, route notice OPR_001=3004 had pulled in the entire feed).
        let recs = records();
        let mut n = notice_for("R1");
        n.rule_id = "RTS_012".into();
        let idx = build_name_index_impl(&recs, &[n], true);
        assert!(
            idx.trip_stops.is_empty(),
            "route notice must not pull trip stops: {:?}",
            idx.trip_stops.keys().collect::<Vec<_>>()
        );
        assert!(
            !idx.route_shapes.is_empty(),
            "but the route's shape IDs must remain (map icon)"
        );
    }

    #[test]
    fn large_feed_resolves_route_entity_to_its_shapes() {
        // Route-entity rule (OPR_001/RTS_*): no icon appears if route_shapes is empty.
        let recs = records();
        let mut n = notice_for("R1");
        n.rule_id = "RTS_012".into();
        let idx = build_name_index_impl(&recs, &[n], true);
        let shapes = idx.route_shapes.get("R1").cloned().unwrap_or_default();
        assert!(
            shapes.contains(&"SH1".to_string()) && shapes.contains(&"SH2".to_string()),
            "the route's shapes must be resolved: {shapes:?}"
        );
    }
}

#[cfg(test)]
mod aggregation_tests {
    use super::*;

    fn pth007(pathway_id: &str, id: &str) -> gtfs_core::Notice {
        gtfs_core::Notice {
            id: id.into(),
            rule_id: "PTH_007".into(),
            severity: gtfs_core::Severity::Bilgi,
            rule_class: gtfs_core::RuleClass::Quality,
            entity_type: gtfs_core::EntityType::Pathway,
            entity_id: Some(pathway_id.into()),
            scope_key: Some(pathway_id.into()),
            file: Some("pathways.txt".into()),
            line: Some(1),
            field: Some("traversal_time".into()),
            observed_value: Some("0".into()),
            expected_value: Some("> 0".into()),
            details: None,
            whitespace_derived: false,
            whitespace_candidate: false,
            title: String::new(),
            message: String::new(),
            remediation: String::new(),
            blocks: vec![],
            base_effort: 1,
            service_id: None,
            agency_distribution: None,
        }
    }

    fn other(id: &str) -> gtfs_core::Notice {
        let mut notice = pth007("other", id);
        notice.rule_id = "PTH_006".into();
        notice
    }

    #[test]
    fn pth007_aggregates_at_first_occurrence_without_quadratic_inserts() {
        let mut notices = vec![
            pth007("B", "b1"),
            pth007("A", "a1"),
            other("x"),
            pth007("B", "b2"),
        ];

        let records = EntityRecords::default();
        aggregate_pth007(&mut notices, &AgencyResolver::new(&records));

        assert_eq!(
            notices
                .iter()
                .map(|notice| notice.entity_id.as_deref())
                .collect::<Vec<_>>(),
            vec![Some("B"), Some("A"), Some("other")]
        );
        assert_eq!(notices[0].observed_value.as_deref(), Some("2"));
        assert_eq!(notices[1].observed_value.as_deref(), Some("1"));
    }
}
