pub mod agency;
pub mod areas;
pub mod attributions;
pub mod calendar;
pub mod calendar_dates;
pub mod common;
pub mod fare_attributes;
pub mod fare_leg_rules;
pub mod fare_media;
pub mod fare_products;
pub mod fare_rules;
pub mod fare_transfer_rules;
pub mod feed_info;
pub mod frequencies;
pub mod levels;
pub mod networks;
pub mod pathways;
pub mod rider_categories;
pub mod routes;
pub mod shapes;
pub mod stop_areas;
pub mod stops;
pub mod stop_times;
pub mod timeframes;
pub mod transfers;
pub mod translations;
pub mod trips;

use agency::{validate_agency, AgencyRecord};
use areas::{parse_areas, AreaRecord};
use attributions::{validate_attributions, AttributionRecord};
use common::make_k2_notice;
use calendar::{validate_calendar, CalendarRecord};
use calendar_dates::{validate_calendar_dates, CalendarDateRecord};
use gtfs_core::Notice;
use crate::k1_parse::{RawFile, RawFiles};
use fare_attributes::{validate_fare_attributes, FareAttributeRecord};
use fare_leg_rules::{validate_fare_leg_rules, FareLegRuleRecord};
use fare_media::{validate_fare_media, FareMediaRecord};
use fare_products::{validate_fare_products, FareProductRecord};
use fare_rules::{parse_fare_rules, FareRuleRecord};
use fare_transfer_rules::{validate_fare_transfer_rules, FareTransferRuleRecord};
use feed_info::{validate_feed_info, FeedInfoRecord};
use frequencies::{validate_frequencies, FrequencyRecord};
use levels::{validate_levels, LevelRecord};
use networks::{parse_networks, NetworkRecord};
use pathways::{validate_pathways, PathwayRecord};
use rider_categories::{validate_rider_categories, RiderCategoryRecord};
use routes::{validate_routes, RouteRecord};
use shapes::{validate_shapes, ShapePointRecord};
use stop_areas::{parse_stop_areas, StopAreaRecord};
use stops::{validate_stops, StopRecord};
use stop_times::{validate_stop_times, StopTimeRecord};
use timeframes::{validate_timeframes, TimeframeRecord};
use transfers::{validate_transfers, TransferRecord};
use translations::{validate_translations, TranslationRecord};
use trips::{validate_trips, TripRecord};

/// K2 sonrasında üretilecek typed entity kayıtlarının kapsayıcısı.
#[derive(Debug, Default)]
pub struct EntityRecords {
    pub agencies: Vec<AgencyRecord>,
    pub areas: Vec<AreaRecord>,
    pub attributions: Vec<AttributionRecord>,
    pub calendars: Vec<CalendarRecord>,
    pub calendar_dates: Vec<CalendarDateRecord>,
    pub feed_info: Vec<FeedInfoRecord>,
    pub fare_attributes: Vec<FareAttributeRecord>,
    pub fare_leg_rules: Vec<FareLegRuleRecord>,
    pub fare_media: Vec<FareMediaRecord>,
    pub fare_products: Vec<FareProductRecord>,
    pub fare_rules: Vec<FareRuleRecord>,
    pub fare_transfer_rules: Vec<FareTransferRuleRecord>,
    pub frequencies: Vec<FrequencyRecord>,
    pub levels: Vec<LevelRecord>,
    pub networks: Vec<NetworkRecord>,
    pub pathways: Vec<PathwayRecord>,
    pub rider_categories: Vec<RiderCategoryRecord>,
    pub routes: Vec<RouteRecord>,
    pub shapes: Vec<ShapePointRecord>,
    pub stop_areas: Vec<StopAreaRecord>,
    pub stops: Vec<StopRecord>,
    pub stop_times: Vec<StopTimeRecord>,
    pub timeframes: Vec<TimeframeRecord>,
    pub transfers: Vec<TransferRecord>,
    pub translations: Vec<TranslationRecord>,
    pub trips: Vec<TripRecord>,
    pub has_route_networks_file: bool,
}

/// K2 schema/field validation çıktısı.
#[derive(Debug, Default)]
pub struct K2Result {
    pub records: EntityRecords,
    pub notices: Vec<Notice>,
}

pub fn validate(files: &RawFiles) -> K2Result {
    use crate::timing::Timer;
    let mut notices = Vec::new();
    let mut records = EntityRecords::default();

    if let Some(file) = files.get("agency.txt") {
        let _t = Timer::start("K2::agency");
        let (agency_records, agency_notices) = validate_agency(file);
        records.agencies = agency_records;
        notices.extend(agency_notices);
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
        let (calendar_date_records, calendar_date_notices) = validate_calendar_dates(file);
        records.calendar_dates = calendar_date_records;
        notices.extend(calendar_date_notices);
    }

    if let Some(file) = files.get("feed_info.txt") {
        let _t = Timer::start("K2::feed_info");
        let (feed_info_records, feed_info_notices) = validate_feed_info(file);
        records.feed_info = feed_info_records;
        notices.extend(feed_info_notices);
    }

    if let Some(file) = files.get("fare_attributes.txt") {
        let _t = Timer::start("K2::fare_attributes");
        let (fare_attribute_records, fare_attribute_notices) = validate_fare_attributes(file);
        records.fare_attributes = fare_attribute_records;
        notices.extend(fare_attribute_notices);
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
        let (shape_records, shape_notices) = validate_shapes(file);
        records.shapes = shape_records;
        notices.extend(shape_notices);
    }

    if let Some(file) = files.get("stops.txt") {
        let _t = Timer::start("K2::stops");
        let (stop_records, stop_notices) = validate_stops(file);
        records.stops = stop_records;
        notices.extend(stop_notices);
    }

    if let Some(file) = files.get("stop_times.txt") {
        let _t = Timer::start("K2::stop_times");
        let (stop_time_records, stop_time_notices) = validate_stop_times(file);
        records.stop_times = stop_time_records;
        notices.extend(stop_time_notices);
    }

    if let Some(file) = files.get("transfers.txt") {
        let _t = Timer::start("K2::transfers");
        let (transfer_records, transfer_notices) = validate_transfers(file);
        records.transfers = transfer_records;
        notices.extend(transfer_notices);
    }

    if let Some(file) = files.get("translations.txt") {
        let _t = Timer::start("K2::translations");
        let (translation_records, translation_notices) = validate_translations(file);
        records.translations = translation_records;
        notices.extend(translation_notices);
    }

    if let Some(file) = files.get("trips.txt") {
        let _t = Timer::start("K2::trips");
        let (trip_records, trip_notices) = validate_trips(file);
        records.trips = trip_records;
        notices.extend(trip_notices);
    }

    if let Some(file) = files.get("areas.txt") {
        let _t = Timer::start("K2::areas");
        records.areas = parse_areas(file);
    }

    if let Some(file) = files.get("stop_areas.txt") {
        let _t = Timer::start("K2::stop_areas");
        records.stop_areas = parse_stop_areas(file);
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

    // AGN_013: Feed dili ile ajans dili uyuşmuyor (feed_info_lang_and_agency_lang_mismatch)
    if let (Some(fi), Some(ag)) = (records.feed_info.first(), records.agencies.first()) {
        let feed_lang = fi.feed_lang.to_lowercase();
        let agency_lang = ag.agency_lang.as_deref().unwrap_or("").to_lowercase();
        if !agency_lang.is_empty() && feed_lang != agency_lang {
            let mut ctr = 0u32;
            notices.push(make_k2_notice(
                &mut ctr, "AGN_013", gtfs_core::EntityType::Feed, None,
                None, "feed_info.txt", None, Some("feed_lang"),
                Some(fi.feed_lang.clone()),
                Some(ag.agency_lang.clone().unwrap_or_default()),
                format!(
                    "feed_lang ('{}') ile agency_lang ('{}') uyuşmuyor.",
                    fi.feed_lang,
                    ag.agency_lang.as_deref().unwrap_or("")
                ),
                "feed_info.txt'deki feed_lang ile agency.txt'deki agency_lang değerlerini tutarlı hale getirin.",
            ));
        }
    }

    K2Result { records, notices }
}

#[allow(dead_code)]
fn _assert_rawfile_used(_: &RawFile) {}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::k1_parse::RawFile;
    use std::collections::HashMap;

    #[test]
    fn validate_empty_files_returns_empty_result() {
        let files: RawFiles = HashMap::new();
        let result = validate(&files);
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
            },
        );

        let result = validate(&files);
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
            },
        );

        let result = validate(&files);
        assert_eq!(result.records.fare_rules.len(), 1);
        assert!(result.notices.is_empty());
    }
}
