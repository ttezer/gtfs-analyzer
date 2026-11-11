pub mod agency;
pub mod agency_jp;
pub mod areas;
pub mod attributions;
pub(crate) mod bcp47_grandfathered;
pub mod booking_rules;
pub mod calendar;
pub mod calendar_dates;
pub mod common;
pub mod fare_attributes;
pub mod fare_leg_join_rules;
pub mod fare_leg_rules;
pub mod fare_media;
pub mod fare_products;
pub mod fare_rules;
pub mod fare_transfer_rules;
pub mod feed_info;
pub mod frequencies;
pub(crate) mod iso4217_generated;
pub mod levels;
pub mod location_groups;
pub mod networks;
pub mod office_jp;
pub mod pathways;
pub mod pattern_jp;
pub mod rider_categories;
pub mod route_networks;
pub mod routes;
pub mod routes_jp;
pub mod shapes;
pub mod stop_areas;
pub mod stop_times;
pub mod stops;
pub mod timeframes;
pub mod transfers;
pub(crate) mod translatable_fields_generated;
pub mod translations;
pub mod trips;

use crate::k1_parse::{RawFile, RawFiles};
use crate::WhitespaceSuppressions;
use agency::{validate_agency, AgencyRecord};
use agency_jp::{parse_agency_jp, AgencyJpRecord};
use areas::{parse_areas, AreaRecord};
use attributions::{validate_attributions, AttributionRecord};
use booking_rules::{validate_booking_rules, BookingRuleRecord};
use calendar::{validate_calendar, CalendarRecord};
use calendar_dates::{validate_calendar_dates_with_limits, CalendarDateIndex};
use common::{has_japanese_kana, is_japanese_language_tag, make_k2_notice};
use fare_attributes::{
    validate_fare_attributes, validate_fare_attributes_with_suppression, FareAttributeRecord,
};
use fare_leg_join_rules::{parse_fare_leg_join_rules, FareLegJoinRuleRecord};
use fare_leg_rules::{validate_fare_leg_rules, FareLegRuleRecord};
use fare_media::{validate_fare_media, FareMediaRecord};
use fare_products::{validate_fare_products, FareProductRecord};
use fare_rules::{parse_fare_rules, FareRuleRecord};
use fare_transfer_rules::{validate_fare_transfer_rules, FareTransferRuleRecord};
use feed_info::{validate_feed_info, FeedInfoRecord};
use frequencies::{validate_frequencies, FrequencyRecord};
use gtfs_config::{GtfsJpProfile, ValidatorConfig};
use gtfs_core::Notice;
use levels::{validate_levels, LevelRecord};
use location_groups::{
    parse_location_group_stops, parse_location_groups, LocationGroupRecord, LocationGroupStopRecord,
};
use networks::{parse_networks, NetworkRecord};
use office_jp::{parse_office_jp, OfficeJpRecord};
use pathways::{validate_pathways, PathwayRecord};
use pattern_jp::{parse_pattern_jp, PatternJpRecord};
use rider_categories::{validate_rider_categories, RiderCategoryRecord};
use route_networks::{parse_route_networks, RouteNetworkRecord};
use routes::{validate_routes, RouteRecord};
use routes_jp::{parse_routes_jp, RoutesJpRecord};
use rustc_hash::FxHashMap;
pub use shapes::ShapeInternTable;
use shapes::{validate_shapes_with_limits, ShapePointRecord};
use std::collections::HashSet;
use stop_areas::{parse_stop_areas, StopAreaRecord};
use stop_times::{
    validate_stop_times_with_limits_and_trip_stop_set, StopTimeRecord, StreamBudget,
    K2_MAX_STREAM_BYTES,
};
pub use stop_times::{CompactStopTime, StopTimesIndex};
use stops::{validate_stops, StopRecord};
use timeframes::{validate_timeframes, TimeframeRecord};
use transfers::{validate_transfers, TransferRecord};
use translations::TranslationRecord;
use trips::{validate_trips_with_limits, TripInternTable, TripRecord};

/// Known files that identify the GTFS-JP profile by physical presence.
pub const GTFS_JP_FILES: [&str; 4] = [
    "agency_jp.txt",
    "office_jp.txt",
    "routes_jp.txt",
    "pattern_jp.txt",
];

/// Container for the typed entity records produced after K2.
#[derive(Debug, Default)]
pub struct EntityRecords {
    /// Explicit GTFS-JP profile selection used by K4/K7. The default is Auto;
    /// `Auto` is only a backward-compatibility mode, not a version claim.
    pub gtfs_jp_profile: GtfsJpProfile,
    pub agencies: Vec<AgencyRecord>,
    pub agency_jp: Vec<AgencyJpRecord>,
    pub areas: Vec<AreaRecord>,
    pub booking_rules: Vec<BookingRuleRecord>,
    pub attributions: Vec<AttributionRecord>,
    pub calendars: Vec<CalendarRecord>,
    pub calendar_dates: CalendarDateIndex,
    pub feed_info: Vec<FeedInfoRecord>,
    pub fare_attributes: Vec<FareAttributeRecord>,
    pub fare_leg_rules: Vec<FareLegRuleRecord>,
    pub fare_media: Vec<FareMediaRecord>,
    pub fare_products: Vec<FareProductRecord>,
    pub fare_rules: Vec<FareRuleRecord>,
    pub fare_transfer_rules: Vec<FareTransferRuleRecord>,
    pub frequencies: Vec<FrequencyRecord>,
    pub levels: Vec<LevelRecord>,
    pub location_groups: Vec<LocationGroupRecord>,
    pub location_group_stops: Vec<LocationGroupStopRecord>,
    /// File-level header metadata used by K4 to separate ARC_025 from XFL value checks.
    /// `None` preserves compatibility for direct synthetic K4 callers without schema metadata.
    pub location_groups_has_location_group_id: Option<bool>,
    pub location_group_stops_has_location_group_id: Option<bool>,
    pub location_group_stops_has_stop_id: Option<bool>,
    pub networks: Vec<NetworkRecord>,
    pub office_jp: Vec<OfficeJpRecord>,
    pub pattern_jp: Vec<PatternJpRecord>,
    /// Preserves that one of the GTFS-JP files is physically present in the ZIP.
    pub has_gtfs_jp_file: bool,
    /// File/header inventory used by K4's GTFS-JP reserved namespace rule.
    /// Unknown files are supplied by K1; known files are copied from their raw
    /// headers before K2 drops the untyped input.
    pub gtfs_jp_namespace_headers: std::collections::HashMap<String, Vec<String>>,
    /// K2's only GTFS-JP signal. The K4 production path reads this field; direct
    /// synthetic K4 callers use it only as the inventory-free compatibility fallback.
    pub is_gtfs_jp: Option<bool>,
    /// Preserves whether `pattern_jp.txt` exists, even when it is empty.
    pub has_pattern_jp_file: bool,
    pub pathways: Vec<PathwayRecord>,
    pub rider_categories: Vec<RiderCategoryRecord>,
    pub routes: Vec<RouteRecord>,
    pub routes_jp: Vec<RoutesJpRecord>,
    pub fare_leg_join_rules: Vec<FareLegJoinRuleRecord>,
    pub route_networks: Vec<RouteNetworkRecord>,
    pub shapes: Vec<ShapePointRecord>,
    /// shapes.txt shape_id intern table (points carry a 4-byte index)
    pub shape_interns: ShapeInternTable,
    pub stop_areas: Vec<StopAreaRecord>,
    pub stops: Vec<StopRecord>,
    // EMPTY in production — K2 no longer builds a Vec (memory savings).
    // Only tests set it directly; the K6 fallback handles this case with from_records.
    pub stop_times: Vec<StopTimeRecord>,
    pub stop_times_index: StopTimesIndex, // production stop_times source (streaming K2)
    pub timeframes: Vec<TimeframeRecord>,
    pub transfers: Vec<TransferRecord>,
    pub translations: Vec<TranslationRecord>,
    pub trips: Vec<TripRecord>,
    pub trip_interns: TripInternTable,
    pub has_route_networks_file: bool,
    /// XFL_019: `routes.network_id` is forbidden even when networks.txt EXISTS (spec: "Forbidden if the
    /// route_networks.txt or networks.txt file exists"). The two files are tracked separately.
    pub has_networks_file: bool,
    /// Physical data-row counts for streamed files (excluding the header).
    /// The lib.rs file_stats loop uses these counters instead of K1 rows.len()==0.
    /// When a new file is streamed, add it here; lib.rs needs no change.
    pub streaming_row_counts: FxHashMap<String, u64>,
}

/// Computes the single GTFS-JP detection predicate shared by K2, K4 and K7.
///
/// `physical_jp_file` is supplied by the caller because K4/K7 may receive the
/// K1 file inventory independently from the typed records. Language signals
/// use the exact BCP-47 primary subtag; `ja` is Japanese, while values such as
/// `jaa` are different languages and must not open JP validation.
pub fn detect_gtfs_jp(records: &EntityRecords, physical_jp_file: bool) -> bool {
    physical_jp_file
        || records.has_gtfs_jp_file
        || records
            .feed_info
            .first()
            .is_some_and(|feed_info| is_japanese_language_tag(&feed_info.feed_lang))
        || records
            .translations
            .iter()
            .any(|translation| translation.language.eq_ignore_ascii_case("ja-Hrkt"))
        || records.agencies.iter().any(|agency| {
            agency
                .agency_lang
                .as_deref()
                .is_some_and(is_japanese_language_tag)
                && agency.agency_timezone.eq_ignore_ascii_case("Asia/Tokyo")
        })
        // feed_lang, agency_lang and agency_timezone are themselves checked by
        // JPN_023..025. Keep an independent text signal so a Japanese feed
        // with all three metadata values wrong can still reach those rules.
        // Kana is required; Kanji-only text is ambiguous with Chinese/Taiwanese
        // feeds and was measured to produce false positives.
        || records
            .agencies
            .iter()
            .any(|agency| has_japanese_kana(&agency.agency_name))
        || records.stops.iter().any(|stop| {
            stop.stop_name
                .as_deref()
                .is_some_and(has_japanese_kana)
        })
}

/// K2 schema/field validation output.
#[derive(Debug, Default)]
pub struct K2Result {
    pub records: EntityRecords,
    pub notices: Vec<Notice>,
    pub whitespace_suppressions: WhitespaceSuppressions,
}

/// STREAMED files above the threshold — the decision source for ARC_022 (issue #75).
///
/// ⚠️ **Sorting does not determine OUTPUT ORDER** — measured on 2026-08-07. K7 provides the
/// final order: `notice_order_key` sorts ARC_022 by `entity_id` (the file name) and
/// reassigns IDs in that order. Determinism comes from there, not from the `sort` here
/// (`FxHashMap` already has a fixed seed; removing the sort produced identical output).
///
/// Why keep it, then: so this function's CONTRACT does not depend on hash internals or insertion
/// order. Depending on K7's sorting would create an undocumented dependency between two distant
/// files; the cost is a three-file `sort`.
fn files_over_row_limit(counts: &FxHashMap<String, u64>, max_rows: u64) -> Vec<(&str, u64)> {
    let mut over: Vec<(&str, u64)> = counts
        .iter()
        .filter(|(_, &rows)| rows > max_rows)
        .map(|(name, &rows)| (name.as_str(), rows))
        .collect();
    over.sort_unstable_by(|a, b| a.0.cmp(b.0));
    over
}

/// `cfg` is read in K2 ONLY for the ARC_022 threshold (`max_file_rows`) — the K2 schema/field
/// layer was historically unconfigured. Added on 2026-08-07 (issue #71): the threshold was
/// hard-coded separately in K1 and K2, so the rule could not be fixture-tested. The check itself
/// now lives in the SINGLE pass at the end of this function (issue #75); there is NO copy in submodules.
pub fn validate(files: RawFiles, zip_bytes: Option<&[u8]>, cfg: &ValidatorConfig) -> K2Result {
    validate_with_stream_limit(files, zip_bytes, cfg, None)
}

/// Passes the config row budget to the four files streamed in SDK/WASM calls.
/// The existing `validate` API remains unlimited for native compatibility.
pub fn validate_with_stream_limit(
    files: RawFiles,
    zip_bytes: Option<&[u8]>,
    cfg: &ValidatorConfig,
    max_stream_rows: Option<usize>,
) -> K2Result {
    validate_with_stream_limit_and_jp_signal(files, zip_bytes, cfg, max_stream_rows, None)
}

/// K1 can know that a JP file exists even when K2 cannot parse/materialise it
/// (for example after a decompression or stream-limit recovery decision). The
/// optional signal keeps that physical-file fact aligned with K4 without
/// changing the historical four-argument API above.
pub fn validate_with_stream_limit_and_jp_signal(
    files: RawFiles,
    zip_bytes: Option<&[u8]>,
    cfg: &ValidatorConfig,
    max_stream_rows: Option<usize>,
    has_gtfs_jp_file: Option<bool>,
) -> K2Result {
    validate_with_stream_limit_and_jp_signal_and_whitespace_roots(
        files,
        zip_bytes,
        cfg,
        max_stream_rows,
        has_gtfs_jp_file,
        None,
    )
}

/// The full pipeline can materialize derivatives only for files where K1 actually produced a
/// DQ_016 root. Direct K2 callers preserve the old output through the API above.
pub fn validate_with_stream_limit_and_jp_signal_and_whitespace_roots(
    mut files: RawFiles,
    zip_bytes: Option<&[u8]>,
    cfg: &ValidatorConfig,
    max_stream_rows: Option<usize>,
    has_gtfs_jp_file: Option<bool>,
    whitespace_root_files: Option<&HashSet<String>>,
) -> K2Result {
    use crate::timing::{mem_log, Timer};
    let mut notices = Vec::new();
    let mut whitespace_suppressions = WhitespaceSuppressions::default();
    // Capture physical JP files before any streaming/drop path. K4 receives
    // the same signal from K1 in the full pipeline; direct K2 callers need the
    // local inventory so translation strictness cannot diverge by entry point.
    let mut records = EntityRecords {
        gtfs_jp_profile: cfg.gtfs_jp_profile,
        has_gtfs_jp_file: has_gtfs_jp_file
            .unwrap_or_else(|| GTFS_JP_FILES.iter().any(|name| files.contains_key(*name))),
        ..EntityRecords::default()
    };
    records.gtfs_jp_namespace_headers = files
        .iter()
        .map(|(name, file)| (name.clone(), file.headers.clone()))
        .collect();
    let mut stream_budget = max_stream_rows.map(|_| StreamBudget::new(K2_MAX_STREAM_BYTES));
    mem_log("K2-start (=after-K1, K1 raw alive)");
    let raw_text_len: usize = files
        .values()
        .filter_map(|file| file.raw_text.as_ref())
        .map(String::len)
        .sum();
    let raw_text_cap: usize = files
        .values()
        .filter_map(|file| file.raw_text.as_ref())
        .map(String::capacity)
        .sum();
    let rows_outer_cap: usize = files.values().map(|file| file.rows.capacity()).sum();
    let rows_inner_cap: usize = files
        .values()
        .flat_map(|file| file.rows.iter())
        .map(Vec::capacity)
        .sum();
    crate::timing::mem_note(&format!(
        "K2 RawFiles: files={} raw_text {raw_text_len}/{raw_text_cap}B; rows_outer cap={} ~{:.1}MB; rows_inner cap={} SmolStr slots ~{:.1}MB",
        files.len(),
        rows_outer_cap,
        rows_outer_cap as f64 * std::mem::size_of::<Vec<smol_str::SmolStr>>() as f64
            / 1_048_576.0,
        rows_inner_cap,
        rows_inner_cap as f64 * std::mem::size_of::<smol_str::SmolStr>() as f64
            / 1_048_576.0,
    ));
    // #15 W2: stop_times is deferred to the very END — it runs AFTER all other files are parsed
    // and the rest of k1.files (especially large shapes raw rows) is dropped. Thus K1 raw is not
    // live at the SAME TIME while the stop_times index is built (~K1-raw peak).
    let stop_times_file = files.remove("stop_times.txt");

    if let Some(file) = files.get("agency.txt") {
        let _t = Timer::start("K2::agency");
        let (agency_records, agency_notices) = validate_agency(file);
        records.agencies = agency_records;
        notices.extend(agency_notices);
    }

    if let Some(file) = files.get("agency_jp.txt") {
        let _t = Timer::start("K2::agency_jp");
        records.agency_jp = parse_agency_jp(file);
    }

    if let Some(file) = files.get("office_jp.txt") {
        let _t = Timer::start("K2::office_jp");
        records.office_jp = parse_office_jp(file);
    }

    if let Some(file) = files.get("pattern_jp.txt") {
        let _t = Timer::start("K2::pattern_jp");
        records.has_pattern_jp_file = true;
        records.pattern_jp = parse_pattern_jp(file);
    }

    if let Some(file) = files.get("routes_jp.txt") {
        let _t = Timer::start("K2::routes_jp");
        records.routes_jp = parse_routes_jp(file);
    }

    if let Some(file) = files.get("attributions.txt") {
        let _t = Timer::start("K2::attributions");
        let (attribution_records, attribution_notices) = validate_attributions(file);
        records.attributions = attribution_records;
        notices.extend(attribution_notices);
    }

    if let Some(file) = files.get("calendar.txt") {
        let _t = Timer::start("K2::calendar");
        let (calendar_records, calendar_notices) = validate_calendar(file);
        records.calendars = calendar_records;
        notices.extend(calendar_notices);
    }

    if let Some(file) = files.get("calendar_dates.txt") {
        let _t = Timer::start("K2::calendar_dates");
        let (calendar_date_records, calendar_date_notices) = validate_calendar_dates_with_limits(
            file,
            zip_bytes,
            max_stream_rows,
            stream_budget.as_mut().map(|budget| &mut *budget),
        );
        records.calendar_dates = calendar_date_records;
        notices.extend(calendar_date_notices);
        records.streaming_row_counts.insert(
            "calendar_dates.txt".to_string(),
            records.calendar_dates.raw_row_count,
        );
        mem_log(&format!(
            "K2::calendar_dates done: svcs={} exc_total={} added_dates={} removed_dates={}",
            records.calendar_dates.exception_count.len(),
            records
                .calendar_dates
                .exception_count
                .values()
                .map(|&v| v as u64)
                .sum::<u64>(),
            records
                .calendar_dates
                .added
                .values()
                .map(|v| v.len())
                .sum::<usize>(),
            records
                .calendar_dates
                .removed
                .values()
                .map(|v| v.len())
                .sum::<usize>(),
        ));
    }

    if let Some(file) = files.get("feed_info.txt") {
        let _t = Timer::start("K2::feed_info");
        let (feed_info_records, feed_info_notices) = validate_feed_info(file);
        records.feed_info = feed_info_records;
        notices.extend(feed_info_notices);
    }

    if let Some(file) = files.get("fare_attributes.txt") {
        let _t = Timer::start("K2::fare_attributes");
        let suppress =
            whitespace_root_files.is_some_and(|files| files.contains("fare_attributes.txt"));
        let (fare_attribute_records, fare_attribute_notices, fare_suppressions) = if suppress {
            validate_fare_attributes_with_suppression(file)
        } else {
            let (records, notices) = validate_fare_attributes(file);
            (records, notices, WhitespaceSuppressions::default())
        };
        records.fare_attributes = fare_attribute_records;
        notices.extend(fare_attribute_notices);
        whitespace_suppressions.merge(fare_suppressions);
    }

    if let Some(file) = files.get("fare_rules.txt") {
        let _t = Timer::start("K2::fare_rules");
        records.fare_rules = parse_fare_rules(file);
    }

    if let Some(file) = files.get("frequencies.txt") {
        let _t = Timer::start("K2::frequencies");
        let (frequency_records, frequency_notices) = validate_frequencies(file);
        records.frequencies = frequency_records;
        notices.extend(frequency_notices);
    }

    if let Some(file) = files.get("levels.txt") {
        let _t = Timer::start("K2::levels");
        let (level_records, level_notices) = validate_levels(file);
        records.levels = level_records;
        notices.extend(level_notices);
    }

    if let Some(file) = files.get("pathways.txt") {
        let _t = Timer::start("K2::pathways");
        let (pathway_records, pathway_notices) = validate_pathways(file);
        records.pathways = pathway_records;
        notices.extend(pathway_notices);
    }

    if let Some(file) = files.get("routes.txt") {
        let _t = Timer::start("K2::routes");
        let (route_records, route_notices) = validate_routes(file);
        records.routes = route_records;
        notices.extend(route_notices);
    }

    if let Some(file) = files.get("shapes.txt") {
        let _t = Timer::start("K2::shapes");
        let (shape_records, shape_interns, shape_notices) = validate_shapes_with_limits(
            file,
            zip_bytes,
            max_stream_rows,
            stream_budget.as_mut().map(|budget| &mut *budget),
        );
        records.shapes = shape_records;
        records.shape_interns = shape_interns;
        notices.extend(shape_notices);
        records
            .streaming_row_counts
            .insert("shapes.txt".to_string(), records.shapes.len() as u64);
        mem_log("K2 after shapes records");
    }

    if let Some(file) = files.get("stops.txt") {
        let _t = Timer::start("K2::stops");
        let (stop_records, stop_notices) = validate_stops(file);
        records.stops = stop_records;
        notices.extend(stop_notices);
    }

    // GTFS-JP detection is a feed signal, independent of the selected profile.
    // Run it after stops.txt is materialised so the independent stop_name kana
    // signal is available in the production K1 -> K2 path. The shared predicate
    // also recovers the five measured Japanese feeds that have
    // agency_lang=ja + Asia/Tokyo but no feed_info/translations/*_jp files.
    records.is_gtfs_jp = Some(detect_gtfs_jp(&records, records.has_gtfs_jp_file));

    if let Some(file) = files.get("transfers.txt") {
        let _t = Timer::start("K2::transfers");
        let (transfer_records, transfer_notices) = validate_transfers(file);
        records.transfers = transfer_records;
        notices.extend(transfer_notices);
    }

    if let Some(file) = files.get("translations.txt") {
        let _t = Timer::start("K2::translations");
        // Detection and the selected validation scope are independent. Explicit
        // V3/V4 selects a rule set; the detection signal still gates validation.
        let signal_before_kana = file
            .headers
            .iter()
            .position(|header| header == "language")
            .is_some_and(|language_idx| {
                file.rows.iter().any(|row| {
                    row.get(language_idx)
                        .is_some_and(|value| value.trim().eq_ignore_ascii_case("ja-Hrkt"))
                })
            });
        let is_gtfs_jp = records.is_gtfs_jp == Some(true);
        records.is_gtfs_jp = Some(is_gtfs_jp || signal_before_kana);
        let (translation_records, translation_notices) =
            translations::validate_translations_with_profile(
                file,
                matches!(cfg.gtfs_jp_profile, GtfsJpProfile::V4),
                cfg.gtfs_jp_profile
                    .jp_validation_enabled(is_gtfs_jp || signal_before_kana),
            );
        records.translations = translation_records;
        notices.extend(translation_notices);
    }

    if let Some(file) = files.get("trips.txt") {
        let _t = Timer::start("K2::trips");
        let (trip_records, trip_interns, trip_notices) = validate_trips_with_limits(
            file,
            zip_bytes,
            max_stream_rows,
            stream_budget.as_mut().map(|budget| &mut *budget),
        );
        records.trips = trip_records;
        records.trip_interns = trip_interns;
        notices.extend(trip_notices);
        records
            .streaming_row_counts
            .insert("trips.txt".to_string(), records.trips.len() as u64);
        mem_log("K2 after trips records");
    }

    if let Some(file) = files.get("areas.txt") {
        let _t = Timer::start("K2::areas");
        records.areas = parse_areas(file);
    }

    if let Some(file) = files.get("booking_rules.txt") {
        let _t = Timer::start("K2::booking_rules");
        let (bkr_records, bkr_notices) = validate_booking_rules(file);
        records.booking_rules = bkr_records;
        notices.extend(bkr_notices);
    }

    if let Some(file) = files.get("stop_areas.txt") {
        let _t = Timer::start("K2::stop_areas");
        records.stop_areas = parse_stop_areas(file);
    }

    if let Some(file) = files.get("location_groups.txt") {
        let _t = Timer::start("K2::location_groups");
        records.location_groups_has_location_group_id = Some(
            file.headers
                .iter()
                .any(|header| header == "location_group_id"),
        );
        records.location_groups = parse_location_groups(file);
    }

    if let Some(file) = files.get("location_group_stops.txt") {
        let _t = Timer::start("K2::location_group_stops");
        records.location_group_stops_has_location_group_id = Some(
            file.headers
                .iter()
                .any(|header| header == "location_group_id"),
        );
        records.location_group_stops_has_stop_id =
            Some(file.headers.iter().any(|header| header == "stop_id"));
        records.location_group_stops = parse_location_group_stops(file);
    }

    if let Some(file) = files.get("networks.txt") {
        let _t = Timer::start("K2::networks");
        records.networks = parse_networks(file);
    }

    if let Some(file) = files.get("rider_categories.txt") {
        let _t = Timer::start("K2::rider_categories");
        let (rcat_records, rcat_notices) = validate_rider_categories(file);
        records.rider_categories = rcat_records;
        notices.extend(rcat_notices);
    }

    if let Some(file) = files.get("fare_media.txt") {
        let _t = Timer::start("K2::fare_media");
        let (fmed_records, fmed_notices) = validate_fare_media(file);
        records.fare_media = fmed_records;
        notices.extend(fmed_notices);
    }

    if let Some(file) = files.get("fare_products.txt") {
        let _t = Timer::start("K2::fare_products");
        let (fprod_records, fprod_notices) = validate_fare_products(file);
        records.fare_products = fprod_records;
        notices.extend(fprod_notices);
    }

    if let Some(file) = files.get("fare_leg_rules.txt") {
        let _t = Timer::start("K2::fare_leg_rules");
        let (flr_records, flr_notices) = validate_fare_leg_rules(file);
        records.fare_leg_rules = flr_records;
        notices.extend(flr_notices);
    }

    if let Some(file) = files.get("fare_transfer_rules.txt") {
        let _t = Timer::start("K2::fare_transfer_rules");
        let (ftr_records, ftr_notices) = validate_fare_transfer_rules(file);
        records.fare_transfer_rules = ftr_records;
        notices.extend(ftr_notices);
    }

    if let Some(file) = files.get("timeframes.txt") {
        let _t = Timer::start("K2::timeframes");
        let (tfr_records, tfr_notices) = validate_timeframes(file);
        records.timeframes = tfr_records;
        notices.extend(tfr_notices);
    }

    records.has_route_networks_file = files.contains_key("route_networks.txt");
    records.has_networks_file = files.contains_key("networks.txt");
    if let Some(file) = files.get("fare_leg_join_rules.txt") {
        let _t = Timer::start("K2::fare_leg_join_rules");
        records.fare_leg_join_rules = parse_fare_leg_join_rules(file);
    }

    if let Some(file) = files.get("route_networks.txt") {
        let _t = Timer::start("K2::route_networks");
        records.route_networks = parse_route_networks(file);
    }

    // #15 W2: all other raw files have been parsed; before building the stop_times index,
    // drop the rest of k1.files (including large shapes raw rows) → K2 peak falls by ~K1-raw.
    mem_log("K2 before stop_times (dropping raw files)");
    let build_trip_stop_set = stop_times_file.is_some()
        && [
            "transfers.txt",
            "fare_rules.txt",
            "fare_attributes.txt",
            "fare_media.txt",
            "fare_products.txt",
            "fare_leg_rules.txt",
            "fare_leg_join_rules.txt",
            "fare_transfer_rules.txt",
        ]
        .iter()
        .any(|name| files.contains_key(*name));
    drop(files);
    mem_log("K2 after drop(files)");
    if let Some(file) = &stop_times_file {
        let _t = Timer::start("K2::stop_times");
        let has_booking_rules = !records.booking_rules.is_empty();
        let (stm_index, stop_time_notices) = validate_stop_times_with_limits_and_trip_stop_set(
            file,
            zip_bytes,
            has_booking_rules,
            max_stream_rows,
            stream_budget.as_mut().map(|budget| &mut *budget),
            build_trip_stop_set,
        );
        records.stop_times_index = stm_index;
        notices.extend(stop_time_notices);
        records.streaming_row_counts.insert(
            "stop_times.txt".to_string(),
            records.stop_times_index.total_rows as u64,
        );
        mem_log("K2 after stop_times index");
    }

    // ARC_022: row-count limit — ONE pass for ALL STREAMED files (issue #75).
    //
    // 🔴 This check used to be copied SEPARATELY into `shapes.rs` and `stop_times.rs`;
    // the other two files streamed in K1 (`trips.txt`, `calendar_dates.txt`) were checked nowhere —
    // `rows` stayed empty in K1 and K2 had no emit points. Thus the rule was SILENT for the
    // files that actually exceeded 1M rows most often. The decision is centralized here instead:
    // adding a file to `streaming_row_counts` brings it into coverage immediately.
    //
    // ⚠️ The reason for sorting is in `files_over_row_limit`'s doc comment — NOT determinism.
    //
    // ⚠️ The count is PARSED records (not K1's raw row count); `file_stats` reports the same
    // source so the two places remain consistent.
    {
        let mut ctr = 0u32;
        for (name, rows) in
            files_over_row_limit(&records.streaming_row_counts, cfg.max_file_rows as u64)
        {
            notices.push(make_k2_notice(
                &mut ctr,
                "ARC_022",
                gtfs_core::EntityType::File,
                Some(name.to_string()),
                None,
                name,
                None,
                None,
                Some(format!("{rows}")),
                None,
                format!(
                    "'{name}' dosyasında {rows} satır var; {} satırlık bilgi eşiğini aşıyor. Dosyanın tamamı doğrulandı.",
                    cfg.max_file_rows
                ),
                "Informational; no fix required. Very large files can increase processing time and memory use for consumers.",
            ));
        }
    }

    // AGN_013 is NOT EMITTED here: K4 `check_agencies` compares every agency. The copy here
    // looked only at the first agency and also fired with empty `feed_lang` (card:
    // "`feed_lang` is empty, AGN_013 does not run"). Registry `Feed` dedup folded both into
    // one notice, so it was hidden; moving dedup to `Entity` exposed the duplicate finding.

    K2Result {
        records,
        notices,
        whitespace_suppressions,
    }
}

#[allow(dead_code)]
fn _assert_rawfile_used(_: &RawFile) {}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::k1_parse::RawFile;
    use std::collections::HashMap;

    #[test]
    fn files_over_row_limit_is_alphabetical_and_strict() {
        // The insertion order is deliberately reversed: without sorting, output would depend on it.
        let mut counts: FxHashMap<String, u64> = FxHashMap::default();
        for (name, rows) in [
            ("stop_times.txt", 11u64),
            ("trips.txt", 11),
            ("calendar_dates.txt", 11),
            ("shapes.txt", 10), // EQUAL TO the threshold → not a violation
            ("agency.txt", 3),
        ] {
            counts.insert(name.to_string(), rows);
        }
        let over = files_over_row_limit(&counts, 10);
        assert_eq!(
            over,
            vec![
                ("calendar_dates.txt", 11),
                ("stop_times.txt", 11),
                ("trips.txt", 11)
            ],
            "eşiği AŞANLAR alfabetik gelmeli; eşiğe eşit olan (shapes) listede OLMAMALI"
        );
    }

    #[test]
    fn validate_empty_files_returns_empty_result() {
        let files: RawFiles = HashMap::new();
        let result = validate(files, None, &ValidatorConfig::default());
        assert!(result.notices.is_empty());
        assert!(result.records.agencies.is_empty());
        assert!(result.records.routes.is_empty());
        assert!(result.records.trips.is_empty());
    }

    #[test]
    fn validate_aggregates_multiple_modules() {
        let mut files: RawFiles = HashMap::new();

        files.insert(
            "agency.txt".to_string(),
            RawFile {
                name: "agency.txt".to_string(),
                headers: vec![
                    "agency_id".into(),
                    "agency_name".into(),
                    "agency_url".into(),
                    "agency_timezone".into(),
                ],
                rows: vec![vec![
                    "A1".into(),
                    "Agency".into(),
                    "https://agency.example".into(),
                    "Europe/Istanbul".into(),
                ]],
                bytes: 0,
                raw_text: None,
                zip_entry_name: None,
            },
        );

        files.insert(
            "routes.txt".to_string(),
            RawFile {
                name: "routes.txt".to_string(),
                headers: vec![
                    "route_id".into(),
                    "route_short_name".into(),
                    "route_type".into(),
                ],
                rows: vec![vec!["R1".into(), "10".into(), "99".into()]],
                bytes: 0,
                raw_text: None,
                zip_entry_name: None,
            },
        );

        files.insert(
            "trips.txt".to_string(),
            RawFile {
                name: "trips.txt".to_string(),
                headers: vec![
                    "route_id".into(),
                    "service_id".into(),
                    "trip_id".into(),
                    "direction_id".into(),
                ],
                rows: vec![vec!["R1".into(), "WKD".into(), "T1".into(), "9".into()]],
                bytes: 0,
                raw_text: None,
                zip_entry_name: None,
            },
        );

        let result = validate(files, None, &ValidatorConfig::default());
        assert_eq!(result.records.agencies.len(), 1);
        assert_eq!(result.records.routes.len(), 1);
        assert_eq!(result.records.trips.len(), 1);
        assert!(result.notices.iter().any(|n| n.rule_id == "RTS_004"));
        assert!(result.notices.iter().any(|n| n.rule_id == "TRP_005"));
    }

    #[test]
    fn validate_wires_fare_rules_without_notices() {
        let mut files: RawFiles = HashMap::new();
        files.insert(
            "fare_rules.txt".to_string(),
            RawFile {
                name: "fare_rules.txt".to_string(),
                headers: vec!["fare_id".into(), "route_id".into()],
                rows: vec![vec!["F1".into(), "R1".into()]],
                bytes: 0,
                raw_text: None,
                zip_entry_name: None,
            },
        );

        let result = validate(files, None, &ValidatorConfig::default());
        assert_eq!(result.records.fare_rules.len(), 1);
        assert!(result.notices.is_empty());
    }

    #[test]
    fn validate_preserves_location_group_header_presence() {
        let mut files: RawFiles = HashMap::new();
        files.insert(
            "location_groups.txt".to_string(),
            RawFile {
                name: "location_groups.txt".to_string(),
                headers: vec!["location_group_name".into()],
                rows: vec![vec!["Group".into()]],
                bytes: 0,
                raw_text: None,
                zip_entry_name: None,
            },
        );
        files.insert(
            "location_group_stops.txt".to_string(),
            RawFile {
                name: "location_group_stops.txt".to_string(),
                headers: vec!["stop_id".into()],
                rows: vec![vec!["S1".into()]],
                bytes: 0,
                raw_text: None,
                zip_entry_name: None,
            },
        );

        let result = validate(files, None, &ValidatorConfig::default());
        assert_eq!(
            result.records.location_groups_has_location_group_id,
            Some(false)
        );
        assert_eq!(
            result.records.location_group_stops_has_location_group_id,
            Some(false)
        );
        assert_eq!(result.records.location_group_stops_has_stop_id, Some(true));
    }
}
