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

// Re-exports exposed to integration tests.
pub use gtfs_config::{CalendarOverrideRule, GtfsJpProfile, ValidatorConfig};
pub use gtfs_core::{
    FatalCode, FatalError, FileInfo, ValidateResult, ValidationResult, ValidationStatus,
};

/// Completes the two-stage proof for the `stops.txt` exemption.
///
/// K1 knows the structural validity of `locations.geojson` Features, but because large
/// `stop_times.txt` bodies are read as a stream in K2, it cannot know whether these regions are
/// actually used. After K2 completes, the exemption is applied only when a `stop_times.location_id`
/// points to a valid feature ID.
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
        // K1 already avoids producing ARC_004/035 because the region is structurally valid.
        // These retains are defensive: direct/future call paths should preserve the same rule.
        k1_notices.retain(|n| {
            !is_stops_notice(n, "ARC_004") && !is_stops_notice(n, "ARC_035")
        });
        k2_notices.retain(|n| n.rule_id != "STP_018");
        if !stops_present {
            partial.unavailable_files.retain(|f| f != "stops.txt");
        }
        return;
    }

    // A structurally valid GeoJSON that no trip uses does not define a service location.
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
            "Zorunlu GTFS dosyası eksik: 'stops.txt'.".to_string(),
            "Eksik dosyayı feed ZIP arşivine ekleyin.",
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
            "Zorunlu GTFS dosyası 'stops.txt' hiç veri satırı içermiyor.".to_string(),
            "Dosyaya gerçek verileri ekleyin; boş bırakmak dosyayı eklememekle aynıdır.",
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

/// Maximum number of trip examples in the STM_036 feed summary.
const STM036_EXAMPLE_CAP: usize = 50;

/// STM_036 summary context: up to [`STM036_EXAMPLE_CAP`] trips in trip_id order, each with its
/// case (`example_<i>_kind`: `order` — a stop written after a later one, from K2, with
/// `curr_stop`/`curr_seq` and `prev_stop`/`prev_seq`; `scattered` — the trip's rows spread over the
/// file, from K6, with `line_from`/`line_to`) and the count of trips in each case.
fn stm036_row_examples(
    representative: &gtfs_core::Notice,
    rest: &[gtfs_core::Notice],
) -> std::collections::BTreeMap<String, String> {
    let mut rows: Vec<&gtfs_core::Notice> =
        std::iter::once(representative).chain(rest.iter()).collect();
    rows.sort_by(|a, b| {
        a.entity_id
            .cmp(&b.entity_id)
            .then_with(|| a.line.cmp(&b.line))
    });
    let is_order = |n: &gtfs_core::Notice| {
        n.details
            .as_ref()
            .is_some_and(|d| d.get("curr_stop").is_some())
    };
    let mut out = std::collections::BTreeMap::new();
    let order = rows.iter().filter(|n| is_order(n)).count();
    out.insert("order_trips".to_string(), order.to_string());
    out.insert(
        "scattered_trips".to_string(),
        (rows.len() - order).to_string(),
    );
    for (i, row) in rows.iter().take(STM036_EXAMPLE_CAP).enumerate() {
        let prefix = format!("example_{i}_");
        out.insert(
            format!("{prefix}trip"),
            row.entity_id.clone().unwrap_or_default(),
        );
        out.insert(
            format!("{prefix}line"),
            row.line.unwrap_or_default().to_string(),
        );
        out.insert(
            format!("{prefix}kind"),
            if is_order(row) { "order" } else { "scattered" }.to_string(),
        );
        if let Some(d) = row.details.as_ref() {
            for key in [
                "curr_stop",
                "curr_seq",
                "prev_stop",
                "prev_seq",
                "line_from",
                "line_to",
            ] {
                if let Some(value) = d.get(key) {
                    out.insert(format!("{prefix}{key}"), value.clone());
                }
            }
        }
    }
    out
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
    // Planner context, before the representative loses its trip: which of the two cases each
    // trip has (a stop written after a later one, or its rows spread over the file) and where.
    let row_examples = stm036_row_examples(&aggregate, &matched);

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
        details.extend(row_examples);
        details.into()
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
        // Key → file lines (from the per-key notices), read before the aggregate is rewritten.
        let key_lines: BTreeMap<String, String> = std::iter::once(&aggregate)
            .chain(matches.iter())
            .filter_map(|n| {
                let lines = n.details.as_ref()?.get("lines")?.clone();
                Some((n.observed_value.clone()?, lines))
            })
            .collect();
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
            // Planner context: each example key with the file lines that carry it (when known).
            for (i, key) in examples.iter().enumerate() {
                details.insert(format!("example_{i}_key"), key.clone());
                if let Some(lines) = key_lines.get(key) {
                    details.insert(format!("example_{i}_lines"), lines.clone());
                }
            }
            details.into()
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
            details.into()
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
            details.into()
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
        "shapes.txt içinde {affected_shapes} shape için shape_dist_traveled azalıyor."
    );
    aggregate.details = Some({
        let mut details = std::collections::BTreeMap::new();
        details.insert("affected_shapes".to_string(), affected_shapes.to_string());
        if !examples.is_empty() {
            details.insert("example_shapes".to_string(), examples.join(", "));
        }
        details.into()
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
            "{affected_segments} zaman geriye gidişi bu seferin kronolojisini bozuyor."
        );
        aggregate.details.get_or_insert_with(gtfs_core::NoticeDetails::new).insert(
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
            "trip_id '{trip_id}' için {affected_rows} kesin zaman noktası satırında saat eksik."
        );
        // Planner examples: the rows' stops, sequences, the time that is there and the missing
        // side, in stop order, at most 50 (the row details are replaced by the trip summary).
        let mut rows: Vec<&gtfs_core::NoticeDetails> = std::iter::once(&aggregate)
            .chain(matches.iter())
            .filter_map(|notice| notice.details.as_ref())
            .collect();
        rows.sort_by_key(|row| {
            row.get("stop_sequence")
                .and_then(|seq| seq.parse::<u64>().ok())
                .unwrap_or(u64::MAX)
        });
        let mut details = BTreeMap::new();
        details.insert("affected_rows".to_string(), affected_rows.to_string());
        for (i, row) in rows.iter().take(50).enumerate() {
            for key in ["stop_id", "stop_sequence", "arrival", "departure"] {
                if let Some(value) = row.get(key) {
                    details.insert(format!("example_{i}_{key}"), value.to_string());
                }
            }
            if let Some(variant) = row.get("message_variant") {
                let missing = variant.strip_prefix("missing_").unwrap_or(variant);
                details.insert(format!("example_{i}_missing"), missing.to_string());
            }
        }
        aggregate.details = Some(details.into());
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
            "pathway_id '{pathway_id}' için {affected_rows} satırda traversal_time geçersiz."
        );
        aggregate.details = Some({
            let mut details = BTreeMap::new();
            details.insert("affected_rows".to_string(), affected_rows.to_string());
            details.into()
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
        details.into()
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
        "translations.txt içinde {affected_rows} satırda table_name geçersiz."
    );
    aggregate.details = Some({
        let mut details = std::collections::BTreeMap::new();
        details.insert("affected_rows".to_string(), affected_rows.to_string());
        if !examples.is_empty() {
            details.insert("example_table_names".to_string(), examples.join(", "));
        }
        details.into()
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
        "stops.txt içinde {affected_stops} stop için stop_lat sayısal değil."
    );
    aggregate.details = Some({
        let mut details = std::collections::BTreeMap::new();
        details.insert("affected_stops".to_string(), affected_stops.to_string());
        if !examples.is_empty() {
            details.insert("example_stop_ids".to_string(), examples.join(", "));
        }
        details.into()
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
        "stops.txt içinde {affected_stops} stop için stop_lon geçersiz veya aralık dışında."
    );
    aggregate.details = Some({
        let mut details = std::collections::BTreeMap::new();
        details.insert("affected_stops".to_string(), affected_stops.to_string());
        if !examples.is_empty() {
            details.insert("example_stop_ids".to_string(), examples.join(", "));
        }
        details.into()
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
        "transfers.txt içinde {affected_rows} satırda min_transfer_time eksik veya geçersiz."
    );
    aggregate.details = Some({
        let mut details = std::collections::BTreeMap::new();
        details.insert("affected_rows".to_string(), affected_rows.to_string());
        if !examples.is_empty() {
            details.insert("example_transfers".to_string(), examples.join(", "));
        }
        details.into()
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
        "pathways.txt grafiğinde {affected_platforms} platform entrance üzerinden erişilemiyor."
    );
    aggregate.details = Some({
        let mut details = std::collections::BTreeMap::new();
        details.insert("affected_platforms".to_string(), affected_platforms.to_string());
        if !examples.is_empty() {
            details.insert("example_platforms".to_string(), examples.join(", "));
        }
        details.into()
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
    aggregate.message = format!("stops.txt içinde {affected_stops} stop için stop_url geçersiz.");
    aggregate.details = Some({
        let mut details = std::collections::BTreeMap::new();
        details.insert("affected_stops".to_string(), affected_stops.to_string());
        if !examples.is_empty() {
            details.insert("example_stop_ids".to_string(), examples.join(", "));
        }
        details.into()
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
        "pathway ağına bağlı {affected_platforms} platform için parent_station eksik."
    );
    aggregate.details = Some({
        let mut details = std::collections::BTreeMap::new();
        details.insert("affected_platforms".to_string(), affected_platforms.to_string());
        if !examples.is_empty() {
            details.insert("example_platforms".to_string(), examples.join(", "));
        }
        details.into()
    });
    aggregate.service_id = None;
    aggregate.whitespace_derived = matched.iter().all(|n| n.whitespace_derived)
        && aggregate.whitespace_derived;
    aggregate.whitespace_candidate = matched.iter().all(|n| n.whitespace_candidate)
        && aggregate.whitespace_candidate;
    retained.insert(first_position.unwrap_or(retained.len()).min(retained.len()), aggregate);
    *notices = retained;
}

/// FAR_010 is emitted for every raw colliding `fare_rules` row (846,630 in one corpus feed).
/// Registry deduplication is at `Feed` level: without aggregation, deduplication kept one random
/// row and the collision count was lost. Use one summary with the count and sample pairs.
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
        "fare_rules.txt içinde {affected_rows} kural, aynı koşul kümesini başka bir tarifeyle çakışacak biçimde tanımlıyor."
    );
    aggregate.details = Some({
        let mut details = std::collections::BTreeMap::new();
        details.insert("affected_rows".to_string(), affected_rows.to_string());
        if !examples.is_empty() {
            details.insert("example_fares".to_string(), examples.join(", "));
        }
        details.into()
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
    // Planner context: the invalid values as written (distinct, sorted, at most five).
    let mut values: Vec<String> = aggregate
        .observed_value
        .clone()
        .into_iter()
        .chain(
            matched
                .iter()
                .filter_map(|notice| notice.observed_value.clone()),
        )
        .collect();
    values.sort_unstable();
    values.dedup();
    values.truncate(5);
    aggregate.entity_type = gtfs_core::EntityType::Feed;
    aggregate.entity_id = None;
    aggregate.scope_key = None;
    aggregate.file = Some("trips.txt".to_string());
    aggregate.line = None;
    aggregate.observed_value = Some(affected_trips.to_string());
    aggregate.expected_value = Some("direction_id 0 or 1".to_string());
    aggregate.message = format!("trips.txt içinde {affected_trips} seferde direction_id geçersiz.");
    aggregate.details = Some({
        let mut details = std::collections::BTreeMap::new();
        details.insert("affected_trips".to_string(), affected_trips.to_string());
        if !examples.is_empty() {
            details.insert("example_trips".to_string(), examples.join(", "));
        }
        if !values.is_empty() {
            details.insert("example_values".to_string(), values.join(" | "));
        }
        details.into()
    });
    aggregate.service_id = None;
    aggregate.whitespace_derived = matched.iter().all(|n| n.whitespace_derived)
        && aggregate.whitespace_derived;
    aggregate.whitespace_candidate = matched.iter().all(|n| n.whitespace_candidate)
        && aggregate.whitespace_candidate;
    retained.insert(first_position.unwrap_or(retained.len()).min(retained.len()), aggregate);
    *notices = retained;
}

/// Maximum number of trip examples in a finding grouped across trips (STM_021, STM_035).
const TRIP_GROUP_EXAMPLE_CAP: usize = 50;

/// STM_021 is a stops.txt problem: two different stops share one coordinate. K6 finds it on every
/// trip that runs between them, so one bad pair used to appear once per trip (one FlixBus pair:
/// 18 identical-looking rows that differed only in their monthly calendar). Reduce the findings to
/// one per unordered stop pair (A→B and B→A are the same record to fix).
fn aggregate_stm021(notices: &mut Vec<gtfs_core::Notice>, resolver: &AgencyResolver) {
    aggregate_across_trips(
        notices,
        resolver,
        "STM_021",
        |d| {
            let a = d.get("stop_a")?.clone();
            let b = d.get("stop_b")?.clone();
            Some(if a <= b { (a, b) } else { (b, a) })
        },
        &["stop_a", "stop_b", "stop_a_sequence", "stop_b_sequence"],
        |trips| format!("Aynı durak çifti {trips} seferde ardışık kullanılıyor."),
    );
}

/// STM_035: the same stop twice in a row is written in a route's stop pattern, so it repeats on
/// every trip of that pattern (corpus: 593,671 findings). One finding per route and stop: the
/// record to fix is that route's pattern, and the same terminal on two routes may be a deliberate
/// loop on one and an extra row on the other.
fn aggregate_stm035(notices: &mut Vec<gtfs_core::Notice>, resolver: &AgencyResolver) {
    aggregate_across_trips(
        notices,
        resolver,
        "STM_035",
        |d| Some((d.get("route_id")?.clone(), d.get("stop_id")?.clone())),
        &[
            "stop_a_sequence",
            "stop_b_sequence",
            "stop_a_departure",
            "stop_b_arrival",
        ],
        |trips| format!("Aynı durak bu hattın {trips} seferinde ardışık iki kez geçiyor."),
    );
}

/// Reduces a trip-level rule's findings to one per `key` (taken from the finding's `details`).
/// The representative is the first trip in trip_id/line order and keeps its own row context;
/// `affected_trips`, `affected_rows` and up to [`TRIP_GROUP_EXAMPLE_CAP`] `example_<i>_*` trips
/// (with `example_keys` copied) list where it occurs. Findings without a key are left as they are.
fn aggregate_across_trips(
    notices: &mut Vec<gtfs_core::Notice>,
    resolver: &AgencyResolver,
    rule_id: &str,
    key: impl Fn(&gtfs_core::NoticeDetails) -> Option<(String, String)>,
    example_keys: &[&str],
    message_suffix: impl Fn(usize) -> String,
) {
    let mut first_position = None;
    let mut groups: std::collections::BTreeMap<(String, String), Vec<gtfs_core::Notice>> =
        std::collections::BTreeMap::new();
    let mut retained = Vec::with_capacity(notices.len());
    for (index, notice) in notices.drain(..).enumerate() {
        let group_key = (notice.rule_id == rule_id)
            .then(|| notice.details.as_ref().and_then(&key))
            .flatten();
        match group_key {
            Some(k) => {
                first_position.get_or_insert(index);
                groups.entry(k).or_default().push(notice);
            }
            None => retained.push(notice),
        }
    }
    if groups.is_empty() {
        *notices = retained;
        return;
    }
    let mut summaries = Vec::with_capacity(groups.len());
    for (_, mut group) in groups {
        group.sort_by(|a, b| {
            a.entity_id
                .cmp(&b.entity_id)
                .then_with(|| a.line.cmp(&b.line))
        });
        let mut trips: Vec<&str> = group
            .iter()
            .filter_map(|n| n.entity_id.as_deref())
            .collect();
        trips.dedup();
        let affected_trips = trips.len();
        let mut examples = std::collections::BTreeMap::new();
        for (i, row) in group.iter().take(TRIP_GROUP_EXAMPLE_CAP).enumerate() {
            let prefix = format!("example_{i}_");
            let Some(d) = row.details.as_ref() else {
                continue;
            };
            examples.insert(
                format!("{prefix}trip"),
                row.entity_id.clone().unwrap_or_default(),
            );
            examples.insert(
                format!("{prefix}line"),
                row.line.unwrap_or_default().to_string(),
            );
            for key in example_keys {
                if let Some(value) = d.get(key) {
                    examples.insert(format!("{prefix}{key}"), value.clone());
                }
            }
        }
        let rows = group.len();
        let mut summary = take_representative(&mut group, resolver);
        if affected_trips > 1 {
            summary.message = format!("{} {}", summary.message, message_suffix(affected_trips));
        }
        let mut details: std::collections::BTreeMap<String, String> = summary
            .details
            .as_ref()
            .map(|d| d.iter().map(|(k, v)| (k.to_string(), v.clone())).collect())
            .unwrap_or_default();
        details.insert("affected_trips".to_string(), affected_trips.to_string());
        details.insert("affected_rows".to_string(), rows.to_string());
        details.extend(examples);
        summary.details = Some(details.into());
        summaries.push(summary);
    }
    let at = first_position.unwrap_or(retained.len()).min(retained.len());
    retained.splice(at..at, summaries);
    *notices = retained;
}

/// Maximum number of row examples in the STM_022 feed summary.
const STM022_EXAMPLE_CAP: usize = 50;
/// Maximum number of distinct invalid values listed in the STM_022 feed summary.
const STM022_VALUE_CAP: usize = 10;

/// Row examples and the invalid-value distribution for the STM_022 feed summary.
///
/// Examples (`example_<i>_*`) are taken round-robin over trips in trip_id/line order: each trip
/// gives its first invalid row before any trip gives a second, up to [`STM022_EXAMPLE_CAP`].
/// Each carries the trip, line, written value and the row's stop context (`stop_id`,
/// `stop_sequence`, `arrival`, `departure`). `value_<i>` / `value_<i>_count` list the distinct
/// written values by frequency, so a planner sees whether one setting causes every row.
fn stm022_row_examples(
    representative: &gtfs_core::Notice,
    rest: &[gtfs_core::Notice],
) -> std::collections::BTreeMap<String, String> {
    let mut out = std::collections::BTreeMap::new();
    let all: Vec<&gtfs_core::Notice> = std::iter::once(representative).chain(rest.iter()).collect();

    let mut values: std::collections::HashMap<&str, usize> = std::collections::HashMap::new();
    for notice in &all {
        *values
            .entry(notice.observed_value.as_deref().unwrap_or(""))
            .or_default() += 1;
    }
    let mut values: Vec<(&str, usize)> = values.into_iter().collect();
    values.sort_by(|a, b| b.1.cmp(&a.1).then_with(|| a.0.cmp(b.0)));
    for (i, (value, count)) in values.into_iter().take(STM022_VALUE_CAP).enumerate() {
        out.insert(format!("value_{i}"), value.to_string());
        out.insert(format!("value_{i}_count"), count.to_string());
    }

    let mut rows: Vec<&gtfs_core::Notice> = all
        .into_iter()
        .filter(|notice| notice.entity_id.is_some() && notice.line.is_some())
        .collect();
    rows.sort_by(|a, b| {
        a.entity_id
            .cmp(&b.entity_id)
            .then_with(|| a.line.cmp(&b.line))
    });
    let mut by_trip: Vec<Vec<&gtfs_core::Notice>> = Vec::new();
    for row in rows {
        match by_trip.last_mut() {
            Some(group) if group[0].entity_id == row.entity_id => group.push(row),
            _ => by_trip.push(vec![row]),
        }
    }
    let mut index = 0;
    let mut round = 0;
    'rounds: loop {
        let mut took_any = false;
        for group in &by_trip {
            let Some(row) = group.get(round) else {
                continue;
            };
            took_any = true;
            let prefix = format!("example_{index}_");
            out.insert(
                format!("{prefix}trip"),
                row.entity_id.clone().unwrap_or_default(),
            );
            out.insert(
                format!("{prefix}line"),
                row.line.unwrap_or_default().to_string(),
            );
            if let Some(value) = row.observed_value.as_ref() {
                out.insert(format!("{prefix}observed"), value.clone());
            }
            if let Some(details) = row.details.as_ref() {
                for (key, value) in details.iter() {
                    out.insert(format!("{prefix}{key}"), value.to_string());
                }
            }
            index += 1;
            if index == STM022_EXAMPLE_CAP {
                break 'rounds;
            }
        }
        if !took_any {
            break;
        }
        round += 1;
    }
    out
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
    // Before the representative loses its trip and line below.
    let row_examples = stm022_row_examples(&aggregate, &matched);
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
    aggregate.message = format!("stop_times.txt içinde {affected_rows} satırda timepoint geçersiz.");
    aggregate.details = Some({
        let mut details = std::collections::BTreeMap::new();
        details.insert("affected_rows".to_string(), affected_rows.to_string());
        if !examples.is_empty() {
            details.insert("example_trips".to_string(), examples.join(", "));
        }
        details.extend(row_examples);
        details.into()
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
    aggregate.message = format!("frequencies.txt içinde {affected_trips} seferde exact_times geçersiz.");
    aggregate.details = Some({
        let mut details = std::collections::BTreeMap::new();
        details.insert("affected_trips".to_string(), affected_trips.to_string());
        if !examples.is_empty() {
            details.insert("example_trips".to_string(), examples.join(", "));
        }
        details.into()
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
    aggregate.expected_value = Some("from_trip_id ve to_trip_id aynı route_type".to_string());
    aggregate.message = format!("transfers.txt içinde {affected_transfers} in-seat aktarmada route_type uyumsuz.");
    aggregate.details = Some({
        let mut details = std::collections::BTreeMap::new();
        details.insert("affected_transfers".to_string(), affected_transfers.to_string());
        if !examples.is_empty() {
            details.insert("example_route_types".to_string(), examples.join(", "));
        }
        details.into()
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
        // Days left is the same for every calendar ending on this date; read it before rewriting.
        let days_left = aggregate
            .details
            .as_ref()
            .and_then(|d| d.get("days_left").cloned());
        let mut examples: Vec<String> = aggregate.entity_id.clone().into_iter()
            .chain(matches.iter().filter_map(|notice| notice.entity_id.clone()))
            .collect();
        examples.sort_unstable();
        examples.dedup();
        // Every calendar ending on this date, so the planner context counts all their trips.
        let services = examples.join(",");
        examples.truncate(5);
        aggregate.entity_type = gtfs_core::EntityType::Feed;
        aggregate.entity_id = None;
        aggregate.scope_key = None;
        aggregate.file = Some("calendar.txt".to_string());
        aggregate.line = None;
        aggregate.field = Some("end_date".to_string());
        aggregate.observed_value = Some(end_date.clone());
        aggregate.message = format!(
            "calendar.txt içinde {affected_services} servisinin end_date değeri {end_date} tarihinde sona eriyor."
        );
        aggregate.details = Some({
            let mut details = std::collections::BTreeMap::new();
            details.insert("affected_services".to_string(), affected_services.to_string());
            if !examples.is_empty() {
                details.insert("example_services".to_string(), examples.join(", "));
            }
            details.insert("services".to_string(), services.clone());
            if let Some(days) = &days_left {
                details.insert("days_left".to_string(), days.clone());
            }
            details.into()
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
    aggregate.message = format!("transfers.txt içinde {affected_transfers} kayıt Google Transit tarafından yoksayılıyor.");
    aggregate.details = Some({
        let mut details = std::collections::BTreeMap::new();
        details.insert("affected_transfers".to_string(), affected_transfers.to_string());
        details.insert("transfer_types".to_string(), types.join(", "));
        details.into()
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
    // `zone_id` is Optional in the spec; its only consumers are fare_rules zone columns. If
    // fare_rules is absent, a missing zone does not break any fare calculation (including free/
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

/// Full K1–K7 pipeline—for integration tests and tool integration.
/// Unlike the WASM version, there is no notice limit and `today` is supplied externally.
pub fn validate_bytes(zip: &[u8], config: &ValidatorConfig, today: u32) -> ValidateResult {
    validate_bytes_inspected(zip, config, today, &mut |_, _, _, _| {})
}

/// Native CLI variant that skips the optional name lookup index.
///
/// The ordinary [`validate_bytes`] path keeps building the index for SDK/WASM and existing
/// callers. The CLI uses this variant unless `--include-name-index` is requested, avoiding a
/// large allocation that would otherwise be discarded immediately after validation.
pub fn validate_bytes_without_name_index(
    zip: &[u8],
    config: &ValidatorConfig,
    today: u32,
) -> ValidateResult {
    validate_bytes_inspected_with_name_index(zip, config, today, false, &mut |_, _, _, _| {})
}

/// Observer supplied to [`validate_bytes_inspected`]: raw K1–K6 notices and the records,
/// derived data, and whitespace-suppression lists needed to interpret them.
#[doc(hidden)]
pub type NoticeInspector<'a> =
    dyn FnMut(&[gtfs_core::Notice], &EntityRecords, &DerivedData, &WhitespaceSuppressions) + 'a;

/// The same pipeline as [`validate_bytes`]; `inspect` sees K1–K6 notices in raw form, BEFORE
/// feed-level aggregation and report scoping. Intended for audit tools (for example,
/// `examples/scope_audit.rs`): aggregation removes `scope_key`/`entity_id`.
#[doc(hidden)]
pub fn validate_bytes_inspected(
    zip: &[u8],
    config: &ValidatorConfig,
    today: u32,
    inspect: &mut NoticeInspector<'_>,
) -> ValidateResult {
    validate_bytes_inspected_with_name_index(zip, config, today, true, inspect)
}

fn validate_bytes_inspected_with_name_index(
    zip: &[u8],
    config: &ValidatorConfig,
    today: u32,
    include_name_index: bool,
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

    // Normalize trips crossing midnight (00:xx) to service-day notation (24:xx).
    // K2 emits the STM_048 raw Spec finding before this pass; this pass only keeps derived/
    // analytical K3–K6 rules (STM_008/014/028, headway…) consistent.
    // No-op for monotonic trips; only trips containing a midnight rollover shift.
    {
        let _t = Timer::start("K2-service-day-normalize");
        k2.records
            .stop_times_index
            .normalize_service_day(config.service_day_start_hour);
    }

    // K1 rows remain empty for streamed files; overwrite them when K2 has counters.
    // When a new file is streamed, it is enough to add it to k2/mod.rs streaming_row_counts.
    for fi in file_stats.iter_mut() {
        if let Some(&count) = k2.records.streaming_row_counts.get(fi.name.as_str()) {
            fi.rows = count as u32;
        }
    }

    let k3 = {
        let _t = Timer::start("K3-entity-map");
        let mut k3 = build_entity_map(&k2.records);
        // XFL_025: GeoJSON feature IDs were collected in K1; move them to EntityMap.
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

    // #15: trip_stop_set (~226 MB in a large feed) is used only by K4; K5/K6/K7 and
    // build_name_index do not use it → release it as soon as K4 finishes (reduces live memory before K6).
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
    crate::timing::log_notice_memory("K1-K6 notices before K7", &all);

    let mut whitespace_suppressions = k2.whitespace_suppressions;
    whitespace_suppressions.merge(k4.whitespace_suppressions);

    inspect(&all, &k2.records, &k5.derived, &whitespace_suppressions);
    aggregate_feed_level_notices(&mut all, &k2.records, &k5.derived);
    apply_report_scope(&mut all, &k2.records, config);

    // issue #133—the publication decision and score must expose loss of COVERAGE. If a required file
    // cannot be read, rules depending on it did not run; absence of a finding is not evidence of absence.
    // ⚠️ Loss of an optional file does NOT LOWER this flag (measured: a malformed `attributions.txt`
    // also makes the run PARTIAL)—blocking a valid feed from publication would be too harsh.
    let coverage_complete = !partial
        .unavailable_files
        .iter()
        .any(|f| k1_parse::is_certification_critical(f))
        && partial.root_structural_errors.is_empty();

    let (k7, agency_breakdown) = {
        let _t = Timer::start("K7-reporting");
        let prepared =
            prepare_k7_notices(all, &k2.records, &k5.derived, false, whitespace_suppressions);
        // #2201: count AFTER suppression; native has no cap, so the listing is complete.
        let agency_breakdown = {
            let _t = Timer::start("K7::agency_breakdown");
            agency_attribution::breakdown(&prepared, &k2.records, true)
        };
        let k7 = report_k7_prepared(prepared, &k2.records, &k5.derived, file_stats, coverage_complete);
        let mut agency_breakdown = agency_breakdown;
        agency_attribution::index_notices(&mut agency_breakdown, &k7.notices, &k2.records);
        (k7, agency_breakdown)
    };
    crate::timing::log_notice_memory("K7 canonical notices", &k7.notices);
    crate::timing::log_report_memory("K7 reports", &k7.reports);
    crate::timing::mem_log("after K7-reporting");

    // name_index filters map data by notices (large-feed mode) → notices must not carry borrowed data.
    let name_index = if include_name_index {
        build_name_index(&k2.records, &k7.notices)
    } else {
        Default::default()
    };
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

/// Removes the group's representative and writes the agency distribution of ALL raw notices in the
/// group to it (#2201). Called BEFORE aggregation removes the representative's `scope_key`/`entity_id`;
/// the distribution total is the group size, i.e. the number of affected records carried by the summary.
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
/// MUST call the same function before `apply_report_scope`: the registry defines these rules at
/// `Feed` dedup level, and locale text reads `observed_value` as the affected-record count. If
/// aggregation is skipped, deduplication selects one raw-row notice and the count is lost.
pub fn aggregate_feed_level_notices(
    all: &mut Vec<gtfs_core::Notice>,
    records: &EntityRecords,
    derived: &DerivedData,
) {
    // K7's whitespace-join resolution uses entity_id; mark which file contains the padding before
    // feed-level aggregation removes it.
    annotate_whitespace_join_provenance(all, records, derived);
    let resolver = AgencyResolver::new(records);

    // STM_036's K2 sequence regression and K6 scattered-row subcases belong to the same
    // feed-level unsorted_stop_times signal; show the user one summary.
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
    timed_aggregate!("STM_021", aggregate_stm021);
    timed_aggregate!("STM_035", aggregate_stm035);
    timed_aggregate!("FRQ_007", aggregate_frq007);
    timed_aggregate!("TRF_019", aggregate_trf019);
    timed_aggregate!("CAL_008", aggregate_cal008);
    timed_aggregate!("GGL_001", aggregate_ggl001);
    timed_aggregate!("FAR_010", aggregate_far010);
    {
        let _t = crate::timing::Timer::start("aggregate::calendar_usage");
        annotate_calendar_usage(all, records);
    }
}

/// Calendar findings whose planner card names the trips and routes running on the calendar(s).
const CALENDAR_USAGE_RULES: [&str; 27] = [
    "CAL_001", "CAL_002", "CAL_003", "CAL_004", "CAL_005", "CAL_006", "CAL_007", "CAL_008",
    "CAL_009", "CAL_010", "CAL_011", "CAL_012", "CAL_013", "CAL_014", "CAL_015", "CAL_016",
    "CAL_017", "CAL_018", "CAL_021", "CAL_023", "CAL_024", "CAL_025", "OPR_011", "OPR_012",
    "OPR_016", "TRP_023", "TRP_026",
];
/// Routes listed per calendar finding (most trips first).
const CALENDAR_USAGE_ROUTES: usize = 10;

/// Planner context for the calendar findings (CAL_001–CAL_025 but CAL_020/022): which trips and routes run on the
/// calendar(s) the finding is about — `trip_count`, `route_count`, the routes with the most trips
/// (`route_<i>_{route_id,trips,example_trip_id}`) and one `example_trip_id` to draw on the map
/// (a trip with stop times first, in file order). A service finding is about its own calendar;
/// CAL_007/008/012/013/016 summaries about the calendars in `details.services`; CAL_009 about every
/// calendar, CAL_015/OPR_016 about every calendar with trips; OPR_011/012 about their calendar; CAL_025 (a calendar row) about its own. Runs after aggregation (the summaries carry their full calendar list then) in the
/// shared native/WASM step. Details only: counts, texts and severities are unchanged.
fn annotate_calendar_usage(all: &mut [gtfs_core::Notice], records: &EntityRecords) {
    use rustc_hash::FxHashMap;
    if !all
        .iter()
        .any(|n| CALENDAR_USAGE_RULES.contains(&n.rule_id.as_str()))
    {
        return;
    }
    let ti = &records.trip_interns;
    let has_times = &records.stop_times_index.trip_id_set;
    // service → route → (trips, example trip, example has stop times); trips in file order.
    let mut usage: FxHashMap<&str, FxHashMap<&str, (usize, &str, bool)>> = FxHashMap::default();
    for t in &records.trips {
        let service = ti.service_id(t);
        if service.is_empty() {
            continue;
        }
        let entry = usage
            .entry(service)
            .or_default()
            .entry(ti.route_id(t))
            .or_insert((0, "", false));
        entry.0 += 1;
        let timed = has_times.contains(t.trip_id.as_str());
        if entry.1.is_empty() || (timed && !entry.2) {
            entry.1 = t.trip_id.as_str();
            entry.2 = timed;
        }
    }
    for n in all.iter_mut() {
        if !CALENDAR_USAGE_RULES.contains(&n.rule_id.as_str()) {
            continue;
        }
        let mut services: Vec<String> = if n.rule_id == "CAL_009" {
            records
                .calendars
                .iter()
                .filter(|c| !c.service_id.is_empty())
                .map(|c| c.service_id.clone())
                .collect()
        } else if n.rule_id == "CAL_015" || n.rule_id == "OPR_016" || n.rule_id == "TRP_023" {
            // No calendar runs yet: every trip of the feed waits for the first service date.
            usage.keys().map(|s| s.to_string()).collect()
        } else if n.rule_id == "TRP_026" {
            // A trip finding about its calendar: the calendar is the observed service_id.
            n.observed_value.clone().into_iter().collect()
        } else if n.entity_type == gtfs_core::EntityType::Service || n.rule_id == "CAL_025" {
            // CAL_025 is a calendar row finding: its record is the calendar itself.
            n.entity_id.clone().into_iter().collect()
        } else {
            n.details
                .as_ref()
                .and_then(|d| d.get("services"))
                .map(|list| {
                    list.split(',')
                        .map(|s| s.trim().to_string())
                        .filter(|s| !s.is_empty())
                        .collect()
                })
                .unwrap_or_default()
        };
        services.sort_unstable();
        services.dedup();
        if services.is_empty() {
            continue;
        }
        // Routes across the calendars: summed trips; the example comes from the first calendar
        // (sorted) that has one, preferring a trip with stop times.
        let mut routes: std::collections::BTreeMap<&str, (usize, &str, bool)> =
            std::collections::BTreeMap::new();
        for service in &services {
            let Some(by_route) = usage.get(service.as_str()) else {
                continue;
            };
            for (&route, &(trips, example, timed)) in by_route {
                let entry = routes.entry(route).or_insert((0, "", false));
                entry.0 += trips;
                if entry.1.is_empty() || (timed && !entry.2) {
                    entry.1 = example;
                    entry.2 = timed;
                }
            }
        }
        let trip_count: usize = routes.values().map(|v| v.0).sum();
        let mut ranked: Vec<_> = routes.into_iter().collect();
        ranked.sort_by(|a, b| b.1 .0.cmp(&a.1 .0).then_with(|| a.0.cmp(b.0)));
        let details = n.details.get_or_insert_with(Default::default);
        details.insert("trip_count", trip_count.to_string());
        details.insert("route_count", ranked.len().to_string());
        let example = ranked.iter().find(|(_, v)| v.2).or(ranked.first());
        if let Some((_, (_, trip, _))) = example {
            details.insert("example_trip_id", trip.to_string());
        }
        for (i, (route, (trips, trip, _))) in
            ranked.into_iter().take(CALENDAR_USAGE_ROUTES).enumerate()
        {
            details.insert(format!("route_{i}_route_id"), route.to_string());
            details.insert(format!("route_{i}_trips"), trips.to_string());
            details.insert(format!("route_{i}_example_trip_id"), trip.to_string());
        }
    }
}

pub fn build_name_index(
    records: &EntityRecords,
    notices: &[gtfs_core::Notice],
) -> gtfs_core::NameIndex {
    const LARGE_FEED_TRIP_CAP: usize = 60_000;
    build_name_index_impl(records, notices, records.trips.len() > LARGE_FEED_TRIP_CAP)
}

/// Implementation of [`build_name_index`], separated so tests can force `large_feed_mode`
/// (the 60,000-trip threshold is impractical to construct in a unit test).
pub(crate) fn build_name_index_impl(
    records: &EntityRecords,
    notices: &[gtfs_core::Notice],
    large_feed_mode: bool,
) -> gtfs_core::NameIndex {
    use std::collections::BTreeMap;

    // ── LARGE-FEED MEMORY MODE (#15 large_feed_memory_mode) ──────────────────────
    // On a very large feed, name_index's heavy fields cause an OOM while serializing to JS (to_js)
    // (100+ MB JSON, exceeding the 4 GB ceiling). DETERMINISTIC threshold (entity count—not
    // runtime memory—for reproducibility).
    //
    // 2026-07-25 FIX—the maps were completely emptied above the threshold, justified by
    // "a map cannot be drawn at that scale". That was wrong: a map opens for ONE notice,
    // not the whole feed. On VBB (282k trips), the result was that stops disappeared for shape
    // rules and the map icon never appeared for trip/route rules.
    //
    // Now FILTER instead of emptying: populate only entities mentioned by a notice.
    // Memory is bounded by the number of notices (not feed size)—a few thousand instead of
    // 282k trips on VBB. Behavior is UNCHANGED for small/normal feeds (the filter means "all").
    const SHAPE_PT_CAP: usize = 800_000;
    // ⚠️ IDS ARE CHEAP, GEOMETRY IS EXPENSIVE (2026-07-25 measurement). I tried filtering
    // shape_coords by notices before serialization: on VBB, a ROUTE notice (OPR_001=3004) pulled
    // all trips for that route, which pulled all their shapes, turning the filter into "all"; to_js
    // jumped from 3,079 → 3,591 MB (+512 MB, dangerously close to the 4 GB ceiling)—4.7M shape points.
    // Geometry is NEVER serialized for a large feed; the UI fetches it on demand for one notice
    // (rerunning requestShapeCoords + buildMapOptions). Filtering applies only to ID maps:
    // trip_stops/trip_shapes/shape_trips/route_shapes are a few hundred KB, and they already
    // restore the map icon and stops.
    let skip_shape_coords = records.shapes.len() > SHAPE_PT_CAP || large_feed_mode;

    // ALL ID candidates mentioned by notices (entity_id + details values). No rule-specific
    // knowledge is needed: SHP_014's details.trip_id and OPR_008's bad_seg stops always arrive here.
    // The candidate pool is then intersected with the real ID sets.
    let mut cand: std::collections::HashSet<&str> = std::collections::HashSet::new();
    if large_feed_mode {
        for n in notices {
            if let Some(e) = n.entity_id.as_deref() {
                cand.insert(e);
            }
            if let Some(d) = &n.details {
                for v in d.values() {
                    // Comma-separated ID lists (ctx_a/ctx_b/stops/pattern_trips).
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
    // Interest sets—notice-referenced entities in large-feed mode, otherwise "all".
    let ti_f = &records.trip_interns;
    let mut want_trip: std::collections::HashSet<&str> = std::collections::HashSet::new();
    let mut want_shape: std::collections::HashSet<&str> = std::collections::HashSet::new();
    if large_feed_mode {
        // 1) Notices with a direct shape entity (SHP_*/GEO_006/007). Intersect in one pass—scanning
        //    shapes.txt per candidate would be O(candidate × point) (4.7M points on VBB).
        for p in &records.shapes {
            let sid = records.shape_interns.id(p);
            if cand.contains(sid) {
                want_shape.insert(sid);
            }
        }
        // 2) Trips: keep a trip if a notice touches the trip, route, or shape. For shape-entity
        //    rules, stops are drawn through shape_trips → trip_stops, so the representative trip
        //    for that shape must also be kept (otherwise "shape exists, no stops" would remain).
        for t in &records.trips {
            let tid = t.trip_id.as_str();
            let sid = ti_f.shape_id(t).unwrap_or("");
            // Do NOT pull in route-candidate trips: the UI's route map draws only route_shapes →
            // shapes and does not show stops. It would pull hundreds of trips and shapes for one
            // route notice, making the filter meaningless.
            let shape_hit = !sid.is_empty() && (cand.contains(sid) || want_shape.contains(sid));
            if cand.contains(tid) || shape_hit {
                want_trip.insert(tid);
                if !sid.is_empty() {
                    want_shape.insert(sid);
                }
            }
        }
        // 3) Route candidates: only SHAPE IDs (no trips; geometry is not serialized anyway).
        //    The UI route map draws at most 4 shapes → limit to 4 per route.
        //    Trips were decided in step 2, so this pass is SEPARATE: shapes added here must not
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

    // trip_id → direction_id ("0"/"1"); trips without direction information are excluded.
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

    // trip_id → first departure time "HH:MM" (skipped in large-feed mode).
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

    // shape_id → unique [[route_id, direction]] list (map; skipped in large-feed mode).
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

    // shape_id → ordered [[lat, lon]] point list (for map rendering).
    // #15: skipped for very large feeds (serialization OOM prevention).
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

    // trip_id → shape_id (map; skipped in large-feed mode).
    let trip_shapes: BTreeMap<String, String> = records
        .trips
        .iter()
        .filter(|t| keep_trip(t.trip_id.as_str()))
        .filter_map(|t| {
            ti.shape_id(t)
                .map(|s| (t.trip_id.to_string(), s.to_string()))
        })
        .collect();

    // trip_id → [stop_id, ...] ordered by stop_sequence (map; in large feeds, trips touched by notices).
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

    // The map uses one pin per physical stop, but retains the source stop_sequence
    // alongside it so planner popups can identify the exact position in the trip.
    let trip_stop_sequences: BTreeMap<String, Vec<u32>> = records
        .stop_times_index
        .iter_trips()
        .filter(|(trip_id, _)| keep_trip(trip_id.as_str()))
        .map(|(trip_id, stops)| {
            let stm_idx = &records.stop_times_index;
            let mut ids: Vec<String> = Vec::new();
            let mut sequences: Vec<u32> = Vec::new();
            for stop in stops.iter().filter(|s| s.stop_idx != u32::MAX) {
                let id = stm_idx.stop_id_of(stop).to_string();
                if ids.last().map(String::as_str) == Some(id.as_str()) {
                    continue;
                }
                ids.push(id);
                sequences.push(stop.sequence);
            }
            (trip_id.to_string(), sequences)
        })
        .collect();

    // shape_id → first trip_id (map; skipped in large-feed mode).
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

    // route_id → [distinct shape_ids] (terminus map; skipped in large-feed mode).
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

    // route_id → [route_color, route_text_color]; invalid hex is omitted, and missing text color stays empty.
    let is_hex6 = |v: &str| v.len() == 6 && v.bytes().all(|b| b.is_ascii_hexdigit());
    let route_colors: BTreeMap<String, [String; 2]> = records
        .routes
        .iter()
        .filter_map(|r| {
            let bg = r.route_color.as_deref().map(str::trim).filter(|v| is_hex6(v))?;
            let fg = r
                .route_text_color
                .as_deref()
                .map(str::trim)
                .filter(|v| is_hex6(v))
                .unwrap_or("");
            Some((r.route_id.clone(), [bg.to_string(), fg.to_string()]))
        })
        .collect();

    // service_id → [day mask, start, end, exception count]; for a service defined only in
    // calendar_dates, the mask is empty and the range is the first/last added date.
    let ymd = |d: (u32, u32, u32)| format!("{:04}{:02}{:02}", d.0, d.1, d.2);
    let cd = &records.calendar_dates;
    let exceptions = |sid: &str| cd.exception_count.get(sid).copied().unwrap_or(0).to_string();
    let mut service_days: BTreeMap<String, [String; 4]> = records
        .calendars
        .iter()
        .map(|c| {
            let mask: String = c.days.iter().map(|d| if *d == Some(1) { '1' } else { '0' }).collect();
            let start = c.start_date.map(ymd).unwrap_or_default();
            let end = c.end_date.map(ymd).unwrap_or_default();
            (c.service_id.clone(), [mask, start, end, exceptions(c.service_id.as_str())])
        })
        .collect();
    for (sid, dates) in &cd.added {
        if service_days.contains_key(sid.as_str()) || dates.is_empty() {
            continue;
        }
        let (first, last) = (dates[0], dates[dates.len() - 1]);
        service_days.insert(
            sid.to_string(),
            [String::new(), first.to_string(), last.to_string(), exceptions(sid.as_str())],
        );
    }

    let route_long_names: BTreeMap<String, String> = records
        .routes
        .iter()
        .filter_map(|r| {
            let short = r.route_short_name.as_deref().map(str::trim).filter(|v| !v.is_empty())?;
            let long = r.route_long_name.as_deref().map(str::trim).filter(|v| !v.is_empty() && *v != short)?;
            Some((r.route_id.clone(), long.to_string()))
        })
        .collect();
    let stop_codes: BTreeMap<String, String> = records
        .stops
        .iter()
        .filter_map(|r| {
            let code = r.stop_code.as_deref().map(str::trim).filter(|v| !v.is_empty())?;
            Some((r.stop_id.clone(), code.to_string()))
        })
        .collect();

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
        trip_stop_sequences,
        shape_trips,
        route_shapes,
        route_colors,
        service_days,
        route_long_names,
        stop_codes,
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
            title: String::new().into(),
            message: String::new(),
            remediation: String::new().into(),
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
            "small feed must not be filtered"
        );
        assert!(!idx.map_data_deferred);
    }

    #[test]
    fn large_feed_keeps_only_entities_referenced_by_notices() {
        // OLD BEHAVIOR: these maps were completely EMPTY → the map icon never appeared.
        let recs = records();
        let idx = build_name_index_impl(&recs, &[notice_for("T1")], true);
        assert!(
            idx.trip_stops.contains_key("T1"),
            "trip with a notice must be drawable on the map"
        );
        assert_eq!(idx.trip_stops.get("T1").map(|v| v.len()), Some(2));
        assert_eq!(idx.trip_stop_sequences.get("T1").map(|v| v.as_slice()), Some([1, 2].as_slice()));
        assert!(
            !idx.trip_stops.contains_key("T2"),
            "trip without a notice must be excluded to save memory"
        );
        assert_eq!(idx.trip_shapes.get("T1").map(String::as_str), Some("SH1"));
        assert!(!idx.trip_shapes.contains_key("T2"));
    }

    #[test]
    fn large_feed_resolves_shape_entity_to_its_trip_stops() {
        // Shape-entity rule (SHP_012/GEO_006): stops are drawn through shape_trips → trip_stops;
        // the shape's representative trip must also be retained.
        let recs = records();
        let mut n = notice_for("SH2");
        n.rule_id = "SHP_012".into();
        let idx = build_name_index_impl(&recs, &[n], true);
        assert_eq!(
            idx.shape_trips.get("SH2").map(String::as_str),
            Some("T2"),
            "the shape must have a representative trip"
        );
        assert!(
            idx.trip_stops.contains_key("T2"),
            "that trip must have stops (otherwise 'shape exists, no stops')"
        );
        assert!(!idx.trip_stops.contains_key("T1"), "unrelated trip is excluded");
    }

    #[test]
    fn large_feed_never_serializes_shape_geometry() {
        let mut shape_ti = ShapeInternTable::new();
        // MEMORY PROTECTION: geometry does not enter JSON for large feeds (+512 MB measured on VBB).
        // The UI fetches missing geometry on demand; map_data_deferred is its flag.
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
            "geometry must not be serialized for a large feed"
        );
        assert!(
            idx.map_data_deferred,
            "the UI must see the flag to fetch on demand"
        );
        // Geometry remains available for a small feed:
        let small = build_name_index_impl(&recs, &[], false);
        assert!(small.shape_coords.contains_key("SH1"));
    }

    #[test]
    fn large_feed_route_notice_does_not_drag_in_trips() {
        // If a route notice pulls in HUNDREDS of trips, the filter loses its meaning (on VBB,
        // route notice OPR_001=3004 pulled in the entire feed).
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
        // Route-entity rule (OPR_001/RTS_*): the icon never appears if route_shapes is empty.
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

    #[test]
    fn route_colors_keep_only_valid_hex_values() {
        let route = |id: &str, color: Option<&str>, text: Option<&str>| crate::k2::routes::RouteRecord {
            route_id: id.into(),
            agency_id: None,
            route_short_name: None,
            route_long_name: None,
            route_desc: None,
            route_type: Some(3),
            route_url: None,
            route_color: color.map(Into::into),
            route_text_color: text.map(Into::into),
            route_sort_order: None,
            continuous_pickup: None,
            continuous_drop_off: None,
            network_id: None,
            route_cemv_support: None,
            jp_office_id: None,
            row: Default::default(),
            line: 2,
        };
        let mut recs = records();
        recs.routes = vec![
            route("A", Some("B5121B"), Some("FFFFFF")),
            route("B", Some("0067a5"), Some("bad")),
            route("C", Some("#B5121B"), None),
            route("D", None, Some("000000")),
        ];
        let idx = build_name_index_impl(&recs, &[], false);
        assert_eq!(idx.route_colors.get("A"), Some(&["B5121B".to_string(), "FFFFFF".to_string()]));
        assert_eq!(idx.route_colors.get("B"), Some(&["0067a5".to_string(), String::new()]));
        assert!(!idx.route_colors.contains_key("C"), "# önekli değer GTFS hex biçimi değil");
        assert!(!idx.route_colors.contains_key("D"));
    }

    #[test]
    fn route_long_names_and_stop_codes_keep_only_useful_values() {
        let route = |id: &str, short: Option<&str>, long: Option<&str>| crate::k2::routes::RouteRecord {
            route_id: id.into(),
            agency_id: None,
            route_short_name: short.map(Into::into),
            route_long_name: long.map(Into::into),
            route_desc: None,
            route_type: Some(3),
            route_url: None,
            route_color: None,
            route_text_color: None,
            route_sort_order: None,
            continuous_pickup: None,
            continuous_drop_off: None,
            network_id: None,
            route_cemv_support: None,
            jp_office_id: None,
            row: Default::default(),
            line: 2,
        };
        let mut recs = records();
        recs.routes = vec![
            route("A", Some("K701"), Some("İstasyon - Hisar Köy")),
            route("B", None, Some("Yalnız uzun ad")),
            route("C", Some("12"), Some("12")),
        ];
        recs.stops = vec![
            crate::k2::stops::StopRecord { stop_id: "S1".into(), stop_code: Some(" 1204 ".into()), ..Default::default() },
            crate::k2::stops::StopRecord { stop_id: "S2".into(), stop_code: Some("  ".into()), ..Default::default() },
        ];
        let idx = build_name_index_impl(&recs, &[], false);
        assert_eq!(idx.route_long_names.get("A").map(String::as_str), Some("İstasyon - Hisar Köy"));
        assert!(!idx.route_long_names.contains_key("B"), "kısa ad yoksa uzun ad zaten routes'ta");
        assert!(!idx.route_long_names.contains_key("C"), "aynı ad tekrar yazılmaz");
        assert_eq!(idx.stop_codes.get("S1").map(String::as_str), Some("1204"));
        assert!(!idx.stop_codes.contains_key("S2"));
    }

    #[test]
    fn service_days_summarise_weekdays_and_date_only_services() {
        let mut recs = records();
        recs.calendars = vec![crate::k2::calendar::CalendarRecord {
            service_id: "HI".into(),
            days: [Some(1), Some(1), Some(1), Some(1), Some(1), Some(0), Some(0)],
            start_date: Some((2026, 1, 1)),
            end_date: Some((2026, 12, 31)),
            row: Default::default(),
            line: 2,
        }];
        recs.calendar_dates.added.insert("BAYRAM".into(), vec![20260520, 20260521]);
        recs.calendar_dates.exception_count.insert("BAYRAM".into(), 2);
        let idx = build_name_index_impl(&recs, &[], false);
        assert_eq!(
            idx.service_days.get("HI"),
            Some(&["1111100".to_string(), "20260101".into(), "20261231".into(), "0".into()])
        );
        assert_eq!(
            idx.service_days.get("BAYRAM"),
            Some(&[String::new(), "20260520".into(), "20260521".into(), "2".into()])
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
            title: String::new().into(),
            message: String::new(),
            remediation: String::new().into(),
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
