use std::collections::{BTreeSet, HashMap, HashSet};

use gtfs_core::{EntityType, Notice};
use gtfs_config::GtfsJpProfile;
use rustc_hash::{FxHashMap, FxHashSet};
use smol_str::SmolStr;

use crate::k2::{EntityRecords, GTFS_JP_FILES};
use crate::k3_entity_graph::EntityMap;
use crate::recovery::FileAvailability;
use crate::timing::Timer;

// �"?�"? �?ıktı �"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?

#[derive(Debug, Default)]
pub struct K4Result {
    pub notices: Vec<Notice>,
    pub skipped_checks: Vec<String>,
}

// �"?�"? Ana fonksiyon �"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?

pub fn check(records: &EntityRecords, entity_map: &EntityMap, today: u32) -> K4Result {
    check_with_files(records, entity_map, today, &FileAvailability::complete())
}

pub fn check_with_files(
    records: &EntityRecords,
    entity_map: &EntityMap,
    today: u32,
    availability: &FileAvailability<'_>,
) -> K4Result {
    let mut notices = Vec::new();
    let mut skipped_checks = Vec::new();
    let mut ctr = 0u32;
    let map = entity_map;

    macro_rules! gate {
        ($name:literal, $condition:expr, $body:block) => {
            if $condition {
                $body
            } else {
                skipped_checks.push($name.to_string());
            }
        };
    }

    // stop_times geçişi → StopTimesIndex üzerinden (Vec<StopTimeRecord> taranmaz)
    let (stm_used_stop_ids, stm_trip_continuous, stm_trips_in_stm, stm_trip_stm_count,
         stm_flex_window_trips) = if availability.available("stop_times.txt") {
        let _t = Timer::start("K4::stop_times");
        let idx = &records.stop_times_index;

        // STM_001: trip_id stop_times'ta var ama trips.txt'te yok
        if availability.available("trips.txt") {
            for trip_id in &idx.trip_id_set {
                if !map.trips.contains_key(trip_id.as_str()) {
                    let line = idx.trip_first_line.get(trip_id).copied();
                    notices.push(notice(
                        &mut ctr,
                        "STM_001",
                        EntityType::Trip,
                        Some(trip_id.to_string()),
                        Some(trip_id.to_string()),
                        "stop_times.txt",
                        line,
                        Some("trip_id"),
                        Some(trip_id.to_string()),
                        None,
                        format!("'{}' sefer kodu trips.txt'te tanımlı değil.", trip_id),
                        "Geçerli bir trip_id kullanın.",
                    ));
                }
            }
        } else {
            skipped_checks.push("K4::stop_times::trip_cross_reference".to_string());
        }

        // STM_002: stop_id stop_times'ta var ama stops.txt'te yok.
        // Distinct stop_id başına tek notice — aynı eksik durak binlerce satırda geçse de
        // tek kayıt (bkz. FRL_001'deki aynı desen).
        if availability.available("stops.txt") {
            for stop_id in &idx.stop_id_set {
                if !map.stops.contains_key(stop_id.as_str()) {
                    let line = idx.stop_first_line.get(stop_id).copied();
                    notices.push(notice(
                        &mut ctr,
                        "STM_002",
                        EntityType::Stop,
                        Some(stop_id.to_string()),
                        Some(stop_id.to_string()),
                        "stop_times.txt",
                        line,
                        Some("stop_id"),
                        Some(stop_id.to_string()),
                        None,
                        format!("'{}' durağı stops.txt'te tanımlı değil.", stop_id),
                        "Geçerli bir stop_id kullanın.",
                    ));
                }
            }
        } else {
            skipped_checks.push("K4::stop_times::stop_cross_reference".to_string());
        }

        // XFL_024: stop_times.location_group_id → location_groups (GTFS-Flex)
        // idx.rows hem test hem production'da dolu; flex None satırlar ucuzca atlanır.
        let mut xfl024_seen: HashSet<&str> = HashSet::new();
        let mut xfl025_seen: HashSet<&str> = HashSet::new();
        for st in &idx.rows {
            let Some(flex) = idx.flex_of(st) else { continue };
            if availability.available("location_groups.txt") {
                if let Some(lg) = &flex.location_group_id {
                    if !lg.is_empty()
                        && !map.location_group_ids.contains(lg.as_str())
                        && xfl024_seen.insert(lg.as_str())
                    {
                        notices.push(notice(
                            &mut ctr, "XFL_024", EntityType::Row,
                            Some(lg.to_string()), Some(lg.to_string()),
                            "stop_times.txt", Some(st.line as u64), Some("location_group_id"),
                            Some(lg.to_string()), None,
                            format!("'{}' konum grubu location_groups.txt'te tanimli degil.", lg),
                            "Gecerli bir location_group_id kullanin veya grubu location_groups.txt'te tanimlayin.",
                        ));
                    }
                }
            } else {
                skipped_checks.push("K4::stop_times::location_group_cross_reference".to_string());
            }
            // XFL_025: stop_times.location_id → locations.geojson feature id
            if availability.available("locations.geojson") {
                if let Some(loc) = &flex.location_id {
                    if !loc.is_empty()
                        && !map.geojson_location_ids.contains(loc.as_str())
                        && xfl025_seen.insert(loc.as_str())
                    {
                        notices.push(notice(
                            &mut ctr, "XFL_025", EntityType::Row,
                            Some(loc.to_string()), Some(loc.to_string()),
                            "stop_times.txt", Some(st.line as u64), Some("location_id"),
                            Some(loc.to_string()), None,
                            format!("'{}' konum locations.geojson'da tanimli degil.", loc),
                            "Gecerli bir location_id kullanin veya konumu locations.geojson'a ekleyin.",
                        ));
                    }
                }
            } else {
                skipped_checks.push("K4::stop_times::geojson_location_cross_reference".to_string());
            }
        }

        // index'ten &str görünümlü geçici koleksiyonlar (fonksiyon imzaları değişmeden)
        let used_stop_ids: HashSet<&str>  = idx.stop_id_set.iter().map(|s| s.as_str()).collect();
        let trip_continuous: HashSet<&str> = idx.continuous_trips.iter().map(|s| s.as_str()).collect();
        let trips_in_stm: HashSet<&str>   = idx.trip_id_set.iter().map(|s| s.as_str()).collect();
        let trip_stm_count: HashMap<&str, u32> = idx.iter_trips()
            .map(|(k, v)| (k.as_str(), v.len() as u32))
            .collect();

        // RTS_028: Flex pencereli (start/end_pickup_drop_off_window dolu) sefer kümesi.
        // flex_map feed'lerin ~%99'unda boştur → gate sayesinde milyonlarca satırlık
        // tarama tümüyle atlanır (VBB 6,1M stop_times'ta sıfır maliyet).
        let flex_window_trips: HashSet<&str> = if idx.flex_map.is_empty() {
            HashSet::new()
        } else {
            idx.iter_trips()
                .filter(|(_, stops)| {
                    stops.iter().any(|st| {
                        idx.flex_of(st).is_some_and(|f| {
                            f.start_pickup_drop_off_window.is_some()
                                || f.end_pickup_drop_off_window.is_some()
                        })
                    })
                })
                .map(|(tid, _)| tid.as_str())
                .collect()
        };

        (used_stop_ids, trip_continuous, trips_in_stm, trip_stm_count, flex_window_trips)
    } else {
        skipped_checks.push("K4::stop_times".to_string());
        (HashSet::new(), HashSet::new(), HashSet::new(), HashMap::new(), HashSet::new())
    };

    gate!("K4::agency", availability.available("agency.txt"), {
        check_agencies(records, map, &mut notices, &mut ctr);
    });
    gate!("K4::stops", availability.all(&["stops.txt", "stop_times.txt"]), {
        check_stops(records, map, &mut notices, &mut ctr, &stm_used_stop_ids);
    });
    gate!("K4::routes", availability.all(&["routes.txt", "trips.txt", "stop_times.txt"]), {
        check_routes(records, map, &mut notices, &mut ctr, &stm_flex_window_trips);
    });
    gate!("K4::trips", availability.all(&["trips.txt", "routes.txt"]), {
        check_trips(
            records,
            map,
            &mut notices,
            &mut ctr,
            &stm_trip_continuous,
            availability.available("shapes.txt"),
        );
    });
    gate!("K4::pathways", availability.all(&["pathways.txt", "stops.txt"]), {
        check_pathways(records, map, &mut notices, &mut ctr);
    });
    gate!("K4::booking_rules", availability.available("booking_rules.txt"), {
        check_booking_rules(records, map, &mut notices, &mut ctr);
    });
    gate!("K4::calendar", availability.available("calendar.txt"), {
        check_calendar(records, map, &mut notices, &mut ctr, today);
    });
    gate!("K4::calendar_dates", availability.available("calendar_dates.txt"), {
        check_calendar_dates(records, map, &mut notices, &mut ctr);
    });
    gate!("K4::frequencies", availability.all(&["frequencies.txt", "trips.txt"]), {
        check_frequencies(records, map, &mut notices, &mut ctr, &stm_trips_in_stm);
    });
    gate!("K4::transfers", availability.all(&["transfers.txt", "stops.txt", "trips.txt", "stop_times.txt"]), {
        check_transfers(records, map, &mut notices, &mut ctr, &stm_trips_in_stm, &records.stop_times_index.trip_stop_set, &records.stop_times_index.stop_id_to_idx);
    });
    gate!("K4::fare_attributes", availability.available("fare_attributes.txt"), {
        check_fare_attributes(records, map, &mut notices, &mut ctr);
    });
    gate!("K4::fare_rules", availability.all(&["fare_rules.txt", "stops.txt"]), {
        check_fare_rules(records, map, &mut notices, &mut ctr);
    });
    gate!("K4::fares_v2", availability.any(&["fare_products.txt", "fare_media.txt", "fare_leg_rules.txt"]), {
        check_fares_v2(records, map, &mut notices, &mut ctr);
    });
    gate!("K4::levels", availability.all(&["levels.txt", "stops.txt", "pathways.txt"]), {
        check_levels(records, map, &mut notices, &mut ctr);
    });
    gate!("K4::translations", availability.all(&["translations.txt", "feed_info.txt"]), {
        check_translations(records, map, &mut notices, &mut ctr);
    });
    { let _t = Timer::start("K4::gtfs_jp"); check_gtfs_jp(records, &mut notices, &mut ctr, availability); }
    gate!("K4::attributions", availability.available("attributions.txt"), {
        check_attributions(records, map, &mut notices, &mut ctr);
    });
    gate!("K4::route_networks", availability.all(&["routes.txt", "networks.txt", "route_networks.txt"]), {
        check_route_networks(records, map, &mut notices, &mut ctr);
    });
    gate!("K4::flex_zone_overlap", availability.all(&["stop_times.txt", "locations.geojson"]), {
        check_flex_zone_overlap(records, map, &mut notices, &mut ctr);
    });
    gate!("K4::fare_leg_join_rules", availability.any(&["fare_leg_join_rules.txt", "fare_leg_rules.txt"]), {
        check_fare_leg_join_rules(records, map, &mut notices, &mut ctr);
    });
    gate!("K4::xfl", availability.all(&["routes.txt", "trips.txt", "stop_times.txt"]), {
        check_xfl(records, map, &mut notices, &mut ctr, &stm_trips_in_stm, &stm_trip_stm_count);
    });
    gate!("K4::stm_shape_dist", availability.all(&["stop_times.txt", "shapes.txt"]), {
        check_stm_shape_dist(records, &mut notices, &mut ctr);
    });

    // XFL_026-030: cemv_support ↔ Fares v2 contactless media tutarlılığı (feed-level)
    {
        let _t = Timer::start("K4::cemv_fares");
        let has_agency_cemv1 = records.agencies.iter().any(|a| a.agency_cemv_support == Some(1));
        let has_route_cemv1  = records.routes.iter().any(|r| r.route_cemv_support == Some(1));
        let has_route_cemv2  = records.routes.iter().any(|r| r.route_cemv_support == Some(2));
        let has_any_cemv1    = has_agency_cemv1 || has_route_cemv1;
        let has_fares_v2     = !records.fare_products.is_empty();
        let type3_media: HashSet<&str> = records.fare_media.iter()
            .filter(|m| m.fare_media_type == Some(3))
            .map(|m| m.fare_media_id.as_str())
            .collect();
        let has_type3 = !type3_media.is_empty();

        // Feed-level (XFL_028/029/030): cemv ↔ contactless media VARLIK tutarlılığı
        {
            let mut feed_cemv = |rule: &str, msg: String, rem: &str| {
                notices.push(notice(
                    &mut ctr, rule, EntityType::Feed,
                    None, None, "", None, None, None, None, msg, rem,
                ));
            };
            if has_agency_cemv1 && has_fares_v2 && !has_type3 {
                feed_cemv("XFL_028",
                    "agency_cemv_support=1 var ve Fares v2 kullaniliyor ama hic contactless fare media (fare_media_type=3) yok.".to_string(),
                    "Detayli ucret icin fare_media.txt'te fare_media_type=3 olan bir medya tanimlayin.");
            }
            if has_route_cemv1 && has_fares_v2 && !has_type3 {
                feed_cemv("XFL_029",
                    "route_cemv_support=1 var ve Fares v2 kullaniliyor ama hic contactless fare media (fare_media_type=3) yok.".to_string(),
                    "Detayli ucret icin fare_media.txt'te fare_media_type=3 olan bir medya tanimlayin.");
            }
            if has_type3 && !has_any_cemv1 {
                feed_cemv("XFL_030",
                    "Contactless fare media (fare_media_type=3) tanimli ama hicbir agency/route'da cemv_support=1 yok.".to_string(),
                    "Uygulama uyumlulugu icin agency veya route duzeyinde cemv_support=1 belirtmeyi degerlendirin.");
            }
        }

        // Route-bazlı (XFL_026/027): route'a UYGULANABİLİR contactless fare product var mı?
        // Yol: fare_media(type=3) → fare_products → fare_leg_rules → (network_id | from/to_area_id | global)
        if has_type3 && (has_route_cemv1 || has_route_cemv2) {
            let type3_products: HashSet<&str> = records.fare_products.iter()
                .filter(|p| p.fare_media_id.as_deref().is_some_and(|id| type3_media.contains(id)))
                .map(|p| p.fare_product_id.as_str())
                .collect();
            // type3 leg rule'ların kapsamı: global / network / area
            let mut global_type3 = false;
            let mut type3_networks: HashSet<&str> = HashSet::new();
            let mut type3_areas: HashSet<&str> = HashSet::new();
            for lr in &records.fare_leg_rules {
                if !type3_products.contains(lr.fare_product_id.as_str()) { continue; }
                let net = lr.network_id.as_deref().filter(|s| !s.is_empty());
                let fa  = lr.from_area_id.as_deref().filter(|s| !s.is_empty());
                let ta  = lr.to_area_id.as_deref().filter(|s| !s.is_empty());
                if net.is_none() && fa.is_none() && ta.is_none() { global_type3 = true; }
                if let Some(n) = net { type3_networks.insert(n); }
                if let Some(a) = fa  { type3_areas.insert(a); }
                if let Some(a) = ta  { type3_areas.insert(a); }
            }
            // route → area kümesi (yalnız area-based type3 leg rule varsa hesapla — maliyet guard)
            let route_areas: HashMap<&str, HashSet<&str>> = if !type3_areas.is_empty() {
                let mut stop_to_areas: HashMap<&str, Vec<&str>> = HashMap::new();
                for sa in &records.stop_areas {
                    if !sa.stop_id.is_empty() && !sa.area_id.is_empty() {
                        stop_to_areas.entry(sa.stop_id.as_str()).or_default().push(sa.area_id.as_str());
                    }
                }
                let idx = &records.stop_times_index;
                let ti_fa = &records.trip_interns;
                let mut ra: HashMap<&str, HashSet<&str>> = HashMap::new();
                for trip in &records.trips {
                    let trip_route_id = ti_fa.route_id(trip);
                    if trip_route_id.is_empty() { continue; }
                    if let Some(stops) = idx.trip_stop_set.get(trip.trip_id.as_str()) {
                        for &stop_idx in stops {
                            let stop_id = idx.stop_id_of_idx(stop_idx);
                            if let Some(areas) = stop_to_areas.get(stop_id) {
                                let e = ra.entry(trip_route_id).or_default();
                                for a in areas { e.insert(a); }
                            }
                        }
                    }
                }
                ra
            } else { HashMap::new() };

            for r in &records.routes {
                let cemv = r.route_cemv_support;
                if cemv != Some(1) && cemv != Some(2) { continue; }
                let rnet = r.network_id.as_deref().filter(|s| !s.is_empty());
                let rareas = route_areas.get(r.route_id.as_str());
                let network_match = rnet.is_some_and(|n| type3_networks.contains(n));
                let area_match = rareas.is_some_and(|set| set.iter().any(|a| type3_areas.contains(a)));
                let covered = global_type3 || network_match || area_match;
                // FP guard: route'un kapsamı çözülebilir mi? (global / network bilgisi / area bilgisi var)
                let resolvable = global_type3 || rnet.is_some() || rareas.is_some();
                if cemv == Some(1) && resolvable && !covered {
                    notices.push(notice(
                        &mut ctr, "XFL_026", EntityType::Route,
                        Some(r.route_id.clone()), Some(r.route_id.clone()),
                        "routes.txt", None, Some("cemv_support"), None, None,
                        format!("'{}' route'u cemv_support=1 (contactless) ama bu route'a uygulanabilir contactless fare product (fare_media_type=3) yok.", r.route_id),
                        "Bu route'a uygulanabilir bir contactless fare product tanimlayin veya cemv_support degerini gozden gecirin.",
                    ));
                }
                if cemv == Some(2) && covered {
                    notices.push(notice(
                        &mut ctr, "XFL_027", EntityType::Route,
                        Some(r.route_id.clone()), Some(r.route_id.clone()),
                        "routes.txt", None, Some("cemv_support"), None, None,
                        format!("'{}' route'u cemv_support=2 (desteklenmiyor) ama bu route'a uygulanabilir contactless fare product var — celiski.", r.route_id),
                        "cemv_support degerini duzeltin veya contactless fare product kapsamini gozden gecirin.",
                    ));
                }
            }
        }
    }

    K4Result { notices, skipped_checks }
}

// �"?�"? Notice yardımcısı �"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?

#[allow(clippy::too_many_arguments)]
fn notice(
    ctr: &mut u32,
    rule_id: &str,
    entity_type: EntityType,
    entity_id: Option<String>,
    scope_key: Option<String>,
    file: &str,
    line: Option<u64>,
    field: Option<&str>,
    observed: Option<String>,
    expected: Option<String>,
    message: String,
    remediation: &str,
) -> Notice {
    let whitespace_candidate = observed.as_deref().is_some_and(|value| {
        value.split('|').any(|part| !part.is_empty() && part != part.trim())
    });
    let mut notice = crate::notice_factory::build(
        "K4", Some("k4"), ctr, rule_id, entity_type, entity_id, scope_key,
        Some(file.to_string()), line, field.map(str::to_string),
        observed, expected, message, remediation,
    );
    if whitespace_candidate {
        notice.details = Some([(
            "whitespace_candidate".to_string(),
            "true".to_string(),
        )].into_iter().collect());
    }
    notice
}

fn append_jpn022_missing_records(
    notices: &mut Vec<Notice>,
    pending: Vec<Notice>,
) {
    let mut grouped: std::collections::BTreeMap<(String, String), Vec<Notice>> =
        std::collections::BTreeMap::new();
    for finding in pending {
        let key = (
            finding.file.clone().unwrap_or_default(),
            finding.field.clone().unwrap_or_default(),
        );
        grouped.entry(key).or_default().push(finding);
    }

    for ((file, field), mut findings) in grouped {
        if findings.len() == 1 {
            let mut finding = findings.pop().expect("group contains one finding");
            finding.observed_value = Some("1".to_string());
            finding.expected_value = None;
            let details = finding.details.get_or_insert_with(Default::default);
            details.insert("affected_records".to_string(), "1".to_string());
            if let Some(entity_id) = finding.entity_id.as_deref() {
                details.insert("example_record_ids".to_string(), entity_id.to_string());
            }
            finding.message = format!(
                "GTFS-JP v4'te {file} içindeki {field} alanı 1 kayıtta eksik veya boş."
            );
            notices.push(finding);
            continue;
        }

        let affected = findings.len();
        let examples = findings
            .iter()
            .filter_map(|finding| finding.entity_id.as_deref())
            .take(5)
            .map(str::to_string)
            .collect::<Vec<_>>();
        let mut summary = findings.remove(0);
        summary.entity_type = EntityType::Feed;
        summary.entity_id = None;
        summary.scope_key = None;
        summary.line = None;
        summary.observed_value = Some(affected.to_string());
        summary.expected_value = None;
        summary.message = format!(
            "GTFS-JP v4'te {file} içindeki {field} alanı {affected} kayıtta eksik veya boş."
        );
        summary.remediation = format!(
            "{file} içindeki ilgili zorunlu alanı tüm kayıtlar için doldurun."
        );
        let mut details = std::collections::BTreeMap::new();
        details.insert("affected_records".to_string(), affected.to_string());
        if !examples.is_empty() {
            details.insert("example_record_ids".to_string(), examples.join(", "));
        }
        summary.details = Some(details);
        notices.push(summary);
    }
}

// �"?�"? Yardımcı: ham row alanı �"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?

fn row_field<'a>(row: &'a HashMap<String, String>, key: &str) -> &'a str {
    row.get(key).map(String::as_str).unwrap_or("").trim()
}


fn date_to_u32(t: (u32, u32, u32)) -> u32 {
    t.0 * 10000 + t.1 * 100 + t.2
}

// �"?�"? AGN_005: birden fazla ajans farklı timezone �"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?

fn check_agencies(
    records: &EntityRecords,
    _map: &EntityMap,
    notices: &mut Vec<Notice>,
    ctr: &mut u32,
) {
    // AGN_013: feed_info.feed_lang ile agency.agency_lang uyuşmuyor
    if let Some(fi) = records.feed_info.first() {
        let feed_lang = fi.feed_lang.as_str();
        if !feed_lang.is_empty() {
            for rec in &records.agencies {
                if let Some(ref alang) = rec.agency_lang {
                    if !alang.is_empty() && alang.to_lowercase() != feed_lang.to_lowercase() {
                        notices.push(notice(
                            ctr,
                            "AGN_013",
                            EntityType::Agency,
                            rec.agency_id.clone(),
                            rec.agency_id.clone(),
                            "agency.txt",
                            None,
                            Some("agency_lang"),
                            Some(alang.clone()),
                            Some(feed_lang.to_string()),
                            format!(
                                "agency_lang '{}' feed_info.txt'deki feed_lang '{}' ile uyuşmuyor.",
                                alang, feed_lang
                            ),
                            "agency.txt'teki agency_lang değerini feed_info.feed_lang ile uyumlu hale getirin.",
                        ));
                    }
                }
            }
        }
    }

    if records.agencies.len() < 2 {
        return;
    }

    // AGN_017: agency'ler-arası agency_lang tutarsızlığı (inconsistent_agency_lang).
    // 2+ agency garantili (yukarıdaki return). Farklı (büyük/küçük harf duyarsız) dil sayısı > 1 ise uyar.
    {
        let mut langs: Vec<String> = records.agencies.iter()
            .filter_map(|a| a.agency_lang.as_deref())
            .filter(|l| !l.is_empty())
            .map(|l| l.to_lowercase())
            .collect();
        langs.sort();
        langs.dedup();
        if langs.len() > 1 {
            notices.push(notice(
                ctr, "AGN_017", EntityType::Feed,
                None, None, "agency.txt", None, Some("agency_lang"),
                Some(langs.join(", ")), None,
                format!("Feed'deki agency'ler farklı agency_lang değerleri taşıyor ({}) — diller arası tutarsızlık.", langs.join(", ")),
                "Aynı feed'deki agency'lerin agency_lang değerlerini tutarlı hale getirin (çok dilli ağ ise yok sayılabilir).",
            ));
        }
    }
    // AGN_005 yalnız GERÇEK bir çoklu-saat-dilimi anlaşmazlığını raporlamalıdır.
    // Eskiden ham dizeleri `agencies[0]`'a karşı kıyaslıyordu; boş ya da bozuk bir
    // değer tek başına "tutarsızlık" üretiyordu. Korpusta kuralın ateşlediği dört
    // feed'in DÖRDÜ de böyleydi ve hiçbirinde gerçek bir dilim farkı yoktu:
    //   · mdb-1003 / mdb-1004 — agency.txt'nin ORTASINDA başlık satırı tekrarlanıyor,
    //     hayalet bir agency'nin dilimi "agency_timezone" oluyor (gerçekte hepsi
    //     Europe/Madrid). Aynı kusur mdb-1004'te TRP_006'yı da tetikliyor.
    //   · mdb-2946 — 10 agency Europe/Kyiv, birinin dilimi BOŞ.
    //   · mdb-2119 — tek agency; agency_name'deki dengesiz tırnak sütunları kaydırıyor.
    // Boş ve geçersiz değerler zaten AGN_004'ün alanıdır ve dördünde de ateşliyor.
    // Hemen yukarıdaki AGN_017 (agency_lang) boşları BAŞINDAN BERİ eliyordu.
    let mut zones: Vec<&str> = records
        .agencies
        .iter()
        .map(|r| r.agency_timezone.trim())
        .filter(|tz| !tz.is_empty() && crate::k2::common::looks_like_iana_timezone(tz))
        .collect();
    zones.sort_unstable();
    zones.dedup();
    if zones.len() > 1 {
        notices.push(notice(
            ctr,
            "AGN_005",
            EntityType::Agency,
            None,
            None,
            "agency.txt",
            None,
            Some("agency_timezone"),
            Some(zones.join(", ")),
            None,
            "Birden fazla işletici farklı timezone değeri kullanıyor.".to_string(),
            "Tüm işleticilerde aynı IANA timezone'u kullanın.",
        ));
    }

    // AGN_011: birden fazla işletici varsa routes.txt'deki her hatta agency_id zorunlu
    let routes_without_agency = records.routes.iter()
        .filter(|r| !r.route_id.is_empty() && r.agency_id.is_none())
        .count();
    if routes_without_agency > 0 {
        notices.push(notice(
            ctr,
            "AGN_011",
            EntityType::Feed,
            None,
            None,
            "routes.txt",
            None,
            Some("agency_id"),
            Some(format!("{routes_without_agency} routes")),
            Some("dolu".to_string()),
            format!(
                "Feed'de {} işletici var; {routes_without_agency} hatta agency_id belirtilmemiş — hangi işleticinin çalıştırdığı belirlenemiyor.",
                records.agencies.len()
            ),
            "Birden fazla kuruluş olduğunda tüm hatların agency_id alanını doldurun.",
        ));
    }

    // AGN_011 aynı hükmü fare_attributes.txt için de taşır: spec her iki alanı da
    // "Conditionally Required: Required if multiple agencies are defined in agency.txt" diye
    // tanımlar. 2026-08-02'ye kadar YALNIZ routes.txt denetleniyordu; ücret tarafındaki aynı
    // hüküm denetimsizdi. Tek kural, iki dosya (DQ_021 emsali) — olgu birebir aynı.
    let fares_without_agency = records.fare_attributes.iter()
        .filter(|f| !f.fare_id.is_empty() && f.agency_id.is_none())
        .count();
    if fares_without_agency > 0 {
        notices.push(notice(
            ctr,
            "AGN_011",
            EntityType::Feed,
            None,
            None,
            "fare_attributes.txt",
            None,
            Some("agency_id"),
            Some(format!("{fares_without_agency} ücret")),
            Some("dolu".to_string()),
            format!(
                "Feed'de {} işletici var; {fares_without_agency} ücret kaydında agency_id belirtilmemiş — hangi işleticinin ücreti olduğu belirlenemiyor.",
                records.agencies.len()
            ),
            "Birden fazla kuruluş olduğunda tüm fare_attributes kayıtlarının agency_id alanını doldurun.",
        ));
    }
}

// �"?�"? STP_009-012, STP_026-027: durak cross-ref �"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?

fn check_stops(
    records: &EntityRecords,
    map: &EntityMap,
    notices: &mut Vec<Notice>,
    ctr: &mut u32,
    used_in_stm: &HashSet<&str>,
) {

    // Pathway tanımlı istasyonları bul (STP_027 için)
    let station_has_pathway: HashSet<String> = {
        let mut s = HashSet::new();
        for pw in &records.pathways {
            s.insert(pw.from_stop_id.clone());
            s.insert(pw.to_stop_id.clone());
        }
        s
    };

    for rec in &records.stops {
        if rec.stop_id.is_empty() {
            continue;
        }
        let eid = Some(rec.stop_id.clone());
        // 🔴 İKİ AYRI SORU, İKİ AYRI OKUMA:
        //   • "alan DOLU mu"  → DAİMA `parent.trim()` (yalnız boşluktan ibaret alan BOŞTUR)
        //   • "hangi kaydı gösteriyor" → DAİMA HAM `parent` (PK/FK ham sözlüksel değerdir)
        // Bu ayrımın yarısını yapıp diğerini atlamak regresyon üretir: `row_field` trim'i
        // kaldırıldığında aşağıdaki üç `is_empty()` ham değere bakmaya başladı ve tek
        // boşluk taşıyan `parent_station` "dolu" sayıldı — `tfs-535`'te STP_036 0 -> 14.
        //
        // 🔴 FK karşılaştırması HAM değerle yapılır, `row_field` ile DEĞİL.
        // `row_field` trim eder; `map.stops` anahtarları ise ham `stop_id`'dir. İki taraf
        // farklı okununca sondaki boşluk taşıyan bir kimlik eşleşmez: mdb-2003'te
        // `stop_id = "Station Charles Schaumann "` gerçekten TANIMLI, ama trim edilmiş
        // `parent_station` onu bulamıyor ve kural Spec·KRİTİK bir FK ihlali uyduruyordu.
        // #85'in kararı: PK/FK ham sözlüksel değerdir. Doluluk kontrolü trim'li kalır —
        // yalnız boşluktan ibaret bir alan "dolu" sayılmamalı.
        let parent = rec.row.get("parent_station").map(String::as_str).unwrap_or("");
        let loc_type = rec.location_type;

        // STP_009: parent_station geçerli stop_id'ye referans
        if !parent.trim().is_empty() && !map.stops.contains_key(parent) {
            notices.push(notice(
                ctr,
                "STP_009",
                EntityType::Stop,
                eid.clone(),
                eid.clone(),
                "stops.txt",
                Some(rec.line),
                Some("parent_station"),
                Some(parent.to_string()),
                None,
                format!("parent_station '{parent}' stops.txt'te tanımlı değil."),
                "Geçerli bir stop_id'yi parent_station olarak kullanın.",
            ));
        } else if !parent.trim().is_empty() {
            // STP_010: parent_station'ın location_type = 1 (station) olması.
            // İstisna: location_type=4 (Boarding Area) parent'ı PLATFORM (location_type=0)
            // olmalıdır; onu STP_021 denetler, STP_010 kapsamı dışında bırakılır.
            if let Some(&pidx) = map.stops.get(parent) {
                let ptype = records.stops[pidx].location_type;
                if ptype != Some(1) && loc_type != Some(4) {
                    notices.push(notice(
                        ctr,
                        "STP_010",
                        EntityType::Stop,
                        eid.clone(),
                        eid.clone(),
                        "stops.txt",
                        Some(rec.line),
                        Some("parent_station"),
                        ptype.map(|v| v.to_string()),
                        Some("1".to_string()),
                        format!("'{parent}' istasyonunun location_type'ı 1 (station) olmalı."),
                        "parent_station olarak yalnızca location_type=1 olan duraklarla referans verin.",
                    ));
                }
            }
        }

        // STP_011: location_type 2,3,4 için parent_station zorunlu
        if matches!(loc_type, Some(2) | Some(3) | Some(4)) && parent.trim().is_empty() {
            notices.push(notice(
                ctr,
                "STP_011",
                EntityType::Stop,
                eid.clone(),
                eid.clone(),
                "stops.txt",
                Some(rec.line),
                Some("parent_station"),
                None,
                Some("must be set".to_string()),
                format!(
                    "location_type={} olan durak için parent_station zorunludur.",
                    loc_type.unwrap()
                ),
                "Bu durak tipine bir istasyon (location_type=1) parent_station olarak atayın.",
            ));
        }

        // STP_036: location_type=1 (station) parent_station içermemeli
        if loc_type == Some(1) && !parent.trim().is_empty() {
            notices.push(notice(
                ctr,
                "STP_036",
                EntityType::Stop,
                eid.clone(),
                eid.clone(),
                "stops.txt",
                Some(rec.line),
                Some("parent_station"),
                Some(parent.to_string()),
                Some("must be empty".to_string()),
                format!("İstasyon '{}' (location_type=1) parent_station='{}' içeriyor; istasyonlar üst varlık olarak parent_station içermemelidir.", rec.stop_id, parent),
                "İstasyondan parent_station alanını kaldırın; sadece location_type 2/3/4 için parent_station gerekir.",
            ));
        }

        // STP_012: stop_times'ta kullanılan durakların location_type = 0 veya bo�Y olması
        if used_in_stm.contains(rec.stop_id.as_str())
            && !matches!(loc_type, None | Some(0))
        {
            notices.push(notice(
                ctr,
                "STP_012",
                EntityType::Stop,
                eid.clone(),
                eid.clone(),
                "stops.txt",
                Some(rec.line),
                Some("location_type"),
                loc_type.map(|v| v.to_string()),
                Some("0 or empty".to_string()),
                format!(
                    "stop_id '{}' stop_times'ta kullanılıyor ama location_type={} (sadece 0 veya boş olmalı).",
                    rec.stop_id,
                    loc_type.unwrap_or(0)
                ),
                "stop_times'ta yalnızca location_type=0 (veya boş) olan durakları kullanın.",
            ));
        }

        // STP_021: boarding area (location_type=4) parent_station'ı platform (location_type=0) olmalı.
        // GTFS: location_type=2 (Entrance/Exit) parent'ı ISTASYON (location_type=1) olmalıdır (STP_010);
        // platform-parent zorunluluğu yalnız location_type=4 (Boarding Area) içindir.
        if matches!(loc_type, Some(4)) && !parent.is_empty() {
            if let Some(&pidx) = map.stops.get(parent) {
                let ptype = records.stops[pidx].location_type;
                if !matches!(ptype, None | Some(0)) {
                    notices.push(notice(
                        ctr,
                        "STP_021",
                        EntityType::Stop,
                        eid.clone(),
                        eid.clone(),
                        "stops.txt",
                        Some(rec.line),
                        Some("parent_station"),
                        Some(format!("{parent} (location_type={:?})", ptype)),
                        Some("location_type=0".to_string()),
                        format!(
                            "Boarding area '{}' için parent_station '{}' platform (location_type=0) olmalı.",
                            rec.stop_id, parent
                        ),
                        "Boarding area'nın parent_station'ını location_type=0 olan bir platform olarak ayarlayın.",
                    ));
                }
            }
        }

        // STP_015: level_id bulunamadı
        if let Some(ref lid) = rec.level_id {
            if !map.levels.contains_key(lid.as_str()) {
                notices.push(notice(
                    ctr,
                    "STP_015",
                    EntityType::Stop,
                    eid.clone(),
                    eid.clone(),
                    "stops.txt",
                    Some(rec.line),
                    Some("level_id"),
                    Some(lid.clone()),
                    None,
                    format!("'{}' durağının level_id '{lid}' levels.txt'te tanımlı değil.", rec.stop_id),
                    "Geçerli bir level_id kullanın veya levels.txt'e bu katmanı ekleyin.",
                ));
            }
        }

        // STP_026: stop_access geçerli enum.
        // ⚠️ Spec YALNIZ 0 ve 1 tanımlar; kod 2026-08-03'e kadar `2`yi de kabul ediyordu ve
        // geçersiz değer sessizce geçiyordu (gerekçesi kodda yoktu).
        let stop_access_raw = row_field(&rec.row, "stop_access");
        if !stop_access_raw.is_empty()
            && !matches!(stop_access_raw, "0" | "1") {
                notices.push(notice(
                    ctr,
                    "STP_026",
                    EntityType::Stop,
                    eid.clone(),
                    eid.clone(),
                    "stops.txt",
                    Some(rec.line),
                    Some("stop_access"),
                    Some(stop_access_raw.to_string()),
                    Some("0 or 1".to_string()),
                    format!("stop_access '{stop_access_raw}' geçersiz enum değeri."),
                    "stop_access için 0 veya 1 kullanın.",
                ));
            }

        // STP_043: spec'in koşullu-YASAK hükmü — "Forbidden for locations which are stations
        // (location_type=1), entrances (2), generic nodes (3) or boarding areas (4). Forbidden
        // if parent_station is empty." STP_026 enum GEÇERLİLİĞİNİ ölçer, bu BAĞLAMI ölçer;
        // ikisini aynı kimliğe koymak ATR_006'daki yanlış etiketleme olurdu.
        if !stop_access_raw.is_empty() {
            let bad_type = matches!(loc_type, Some(1) | Some(2) | Some(3) | Some(4));
            let no_parent = parent.is_empty();
            if bad_type || no_parent {
                let why = if bad_type {
                    format!("location_type={}", loc_type.unwrap_or(0))
                } else {
                    "parent_station boş".to_string()
                };
                notices.push(notice(
                    ctr,
                    "STP_043",
                    EntityType::Stop,
                    eid.clone(),
                    eid.clone(),
                    "stops.txt",
                    Some(rec.line),
                    Some("stop_access"),
                    Some(stop_access_raw.to_string()),
                    Some("(boş)".to_string()),
                    format!("stop_access yalnız üst istasyonu olan platformlarda kullanılabilir ({why})."),
                    "stop_access alanını yalnız parent_station'ı olan platformlarda (location_type 0/boş) doldurun.",
                ));
            }
        }

        // STP_027: pathway tanımlı istasyonda stop_access=0 olan platform
        if station_has_pathway.contains(&rec.stop_id)
            && matches!(loc_type, None | Some(0))
            && matches!(rec.stop_access, Some(0) | None)
        {
            let parent_is_station = !parent.is_empty()
                && map
                    .stops
                    .get(parent)
                    .map(|&i| records.stops[i].location_type == Some(1))
                    .unwrap_or(false);
            if parent_is_station {
                notices.push(notice(
                    ctr,
                    "STP_027",
                    EntityType::Stop,
                    eid.clone(),
                    eid.clone(),
                    "stops.txt",
                    Some(rec.line),
                    Some("stop_access"),
                    rec.stop_access.map(|v| v.to_string()),
                    None,
                    format!(
                        "'{}' platformunun stop_access'i belirtilmemiş veya 0 (bilinmiyor); pathway tanımlı istasyonda.",
                        rec.stop_id
                    ),
                    "Pathway tanımlı istasyonlarda platformların stop_access alanını doldurun.",
                ));
            }
        }

        // STP_032: pathway bağlantılı platform (location_type=0) için parent_station eksik
        if station_has_pathway.contains(&rec.stop_id)
            && matches!(loc_type, None | Some(0))
            && parent.is_empty()
        {
            notices.push(notice(
                ctr,
                "STP_032",
                EntityType::Stop,
                eid.clone(),
                eid.clone(),
                "stops.txt",
                Some(rec.line),
                Some("parent_station"),
                None,
                None,
                format!(
                    "'{}' platformu pathway ağına bağlı ama parent_station tanımlı değil.",
                    rec.stop_id
                ),
                "Bu platforma bir istasyon (location_type=1) parent_station olarak atayın.",
            ));
        }
    }
}

// �"?�"? RTS_002, RTS_012: rota cross-ref �"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?

fn check_routes(
    records: &EntityRecords,
    map: &EntityMap,
    notices: &mut Vec<Notice>,
    ctr: &mut u32,
    flex_window_trips: &HashSet<&str>,
) {
    // RTS_002: agency_id referansı
    for rec in &records.routes {
        if rec.route_id.is_empty() {
            continue;
        }
        let eid = Some(rec.route_id.clone());
        if let Some(ref aid) = rec.agency_id {
            if !map.agencies.contains_key(aid.as_str()) {
                notices.push(notice(
                    ctr,
                    "RTS_002",
                    EntityType::Route,
                    eid.clone(),
                    eid.clone(),
                    "routes.txt",
                    Some(rec.line),
                    Some("agency_id"),
                    Some(aid.clone()),
                    None,
                    format!("'{}' işletici kodu agency.txt'te tanımlı değil.", aid),
                    "Geçerli bir agency_id kullanın.",
                ));
            }
        }
    }

    // RTS_012: hiçbir trip'te kullanılmayan rota (orphan)
    let ti_rts012 = &records.trip_interns;
    let used_routes: HashSet<&str> = records
        .trips
        .iter()
        .map(|t| ti_rts012.route_id(t))
        .collect();
    for rec in &records.routes {
        if rec.route_id.is_empty() {
            continue;
        }
        if !used_routes.contains(rec.route_id.as_str()) {
            notices.push(notice(
                ctr,
                "RTS_012",
                EntityType::Route,
                Some(rec.route_id.clone()),
                Some(rec.route_id.clone()),
                "routes.txt",
                Some(rec.line),
                Some("route_id"),
                Some(rec.route_id.clone()),
                None,
                {
                    let label = rec.route_short_name.as_deref()
                        .filter(|s| !s.is_empty())
                        .unwrap_or(rec.route_id.as_str());
                    format!("'{}' kodlu hattın hiçbir seferi bulunmamaktadır.", label)
                },
                "Kullanılmayan rotayı silin veya bu rotaya bir sefer ekleyin.",
            ));
        }
    }

    // RTS_028: rotanın HERHANGİ bir seferinde Flex penceresi varsa routes.continuous_pickup /
    // continuous_drop_off 1 (veya boş) dışında olamaz. GTFS spec [Kesin]: "Any value other than 1
    // or empty is Forbidden if stop_times.start/end_pickup_drop_off_window are defined for any
    // trip of this route." MD `forbidden_continuous_pickup_drop_off` (ERROR) paritesi: MD de İKİ
    // alanı TEK notice'ta ele alır → burada da alan başına ayrı kural YOK (parite bozulmasın).
    // stop_times.continuous_* için koşul satır kapsamlıdır ve STM_054/055'in (K2) işidir.
    if !flex_window_trips.is_empty() {
        let ti = &records.trip_interns;
        // route_id → o rotanın Flex pencereli ilk seferi (mesajda örnek olarak gösterilir).
        // trips Vec sırası deterministik → çıktı deterministik.
        let mut flex_routes: HashMap<&str, &str> = HashMap::new();
        for t in &records.trips {
            if flex_window_trips.contains(t.trip_id.as_str()) {
                flex_routes.entry(ti.route_id(t)).or_insert(t.trip_id.as_str());
            }
        }
        for rec in &records.routes {
            if rec.route_id.is_empty() {
                continue;
            }
            let Some(&sample_trip) = flex_routes.get(rec.route_id.as_str()) else { continue };
            // Enum dışı değerler (>3) RTS_013/018'in işi; burada çift emit edilmez.
            let bad = if matches!(rec.continuous_pickup, Some(0) | Some(2) | Some(3)) {
                Some(("continuous_pickup", rec.continuous_pickup.unwrap()))
            } else if matches!(rec.continuous_drop_off, Some(0) | Some(2) | Some(3)) {
                Some(("continuous_drop_off", rec.continuous_drop_off.unwrap()))
            } else {
                None
            };
            if let Some((field, value)) = bad {
                notices.push(notice(
                    ctr,
                    "RTS_028",
                    EntityType::Route,
                    Some(rec.route_id.clone()),
                    Some(rec.route_id.clone()),
                    "routes.txt",
                    Some(rec.line),
                    Some(field),
                    Some(value.to_string()),
                    Some("1 or empty".to_string()),
                    format!(
                        "'{}' hattının '{}' seferinde Flex penceresi tanımlı — {field}={value} yasaktır.",
                        rec.route_id, sample_trip
                    ),
                    "Flex (talep-üzerine) pencereli rotalarda continuous_pickup/continuous_drop_off alanını 1 yapın veya boş bırakın.",
                ));
            }
        }
    }
}

// ── BKR_015: booking_rules.prior_notice_service_id → calendar/calendar_dates ──

/// BKR_015: `prior_notice_service_id`, `calendar.service_id`'ye foreign key'dir
/// (spec booking_rules.txt: "Foreign ID referencing `calendar.service_id`").
/// Tanımsız bir referans, bildirim gününün hangi servis takvimine göre sayılacağını
/// belirsiz bırakır.
///
/// `booking_type` 0 veya 1 iken alan zaten YASAKTIR ve BKR_014 üretilir; aynı alan için
/// ikinci bir notice çıkmasın diye BKR_015 o satırlarda susar (type=2 ve türü okunamayan
/// satırlar denetlenir).
fn check_booking_rules(
    records: &EntityRecords,
    map: &EntityMap,
    notices: &mut Vec<Notice>,
    ctr: &mut u32,
) {
    // BKR_017/018: stop_times.pickup/drop_off_booking_rule_id → booking_rules.booking_rule_id.
    // Spec (stop_times.txt): iki alan da "Foreign ID referencing booking_rules.booking_rule_id".
    // XFL_024/025 ile aynı desen: alan başına ayrı kural, id başına tek notice (dedup).
    // flex_map feed'lerin ~%99'unda boş → gate ile satır taraması tümüyle atlanır.
    let idx = &records.stop_times_index;
    if !idx.flex_map.is_empty() {
        let known: HashSet<&str> = records
            .booking_rules
            .iter()
            .map(|b| b.booking_rule_id.as_str())
            .filter(|s| !s.is_empty())
            .collect();
        let mut seen_pickup: HashSet<&str> = HashSet::new();
        let mut seen_drop_off: HashSet<&str> = HashSet::new();
        for st in &idx.rows {
            let Some(flex) = idx.flex_of(st) else { continue };
            for (rule_id, field, value, seen) in [
                ("BKR_017", "pickup_booking_rule_id", &flex.pickup_booking_rule_id, &mut seen_pickup),
                ("BKR_018", "drop_off_booking_rule_id", &flex.drop_off_booking_rule_id, &mut seen_drop_off),
            ] {
                let Some(br) = value.as_ref().map(|s| s.as_str()).filter(|s| !s.is_empty()) else {
                    continue;
                };
                if known.contains(br) || !seen.insert(br) {
                    continue;
                }
                notices.push(notice(
                    ctr,
                    rule_id,
                    EntityType::Row,
                    Some(br.to_string()),
                    Some(br.to_string()),
                    "stop_times.txt",
                    Some(st.line as u64),
                    Some(field),
                    Some(br.to_string()),
                    None,
                    format!("'{br}' rezervasyon kuralı booking_rules.txt'te tanımlı değil."),
                    "Geçerli bir booking_rule_id kullanın ya da kuralı booking_rules.txt'e ekleyin.",
                ));
            }
        }
    }

    for rec in &records.booking_rules {
        let Some(svc) = rec.prior_notice_service_id.as_deref().filter(|s| !s.is_empty()) else {
            continue;
        };
        if matches!(rec.booking_type, Some(0) | Some(1)) {
            continue; // alan bu türlerde yasak → BKR_014
        }
        if map.services.contains(svc) {
            continue;
        }
        let eid = (!rec.booking_rule_id.is_empty()).then(|| rec.booking_rule_id.clone());
        notices.push(notice(
            ctr,
            "BKR_015",
            EntityType::Row,
            eid.clone(),
            eid,
            "booking_rules.txt",
            Some(rec.line),
            Some("prior_notice_service_id"),
            Some(svc.to_string()),
            None,
            format!("service_id '{svc}' calendar veya calendar_dates'te tanımlı değil."),
            "Geçerli bir service_id kullanın ya da prior_notice_service_id alanını boş bırakın.",
        ));
    }
}

// �"?�"? TRP_002-004 / TRP_019: sefer cross-ref �"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?

fn check_trips(
    records: &EntityRecords,
    map: &EntityMap,
    notices: &mut Vec<Notice>,
    ctr: &mut u32,
    trip_has_continuous_stm: &HashSet<&str>,
    shapes_available: bool,
) {
    let ti = &records.trip_interns;
    for rec in &records.trips {
        if rec.trip_id.is_empty() {
            continue;
        }
        let eid: Option<String> = Some(rec.trip_id.to_string());
        let rec_route_id = ti.route_id(rec);
        let rec_service_id = ti.service_id(rec);

        // TRP_002: route_id referansı
        if !rec_route_id.is_empty() && !map.routes.contains_key(rec_route_id) {
            notices.push(notice(
                ctr,
                "TRP_002",
                EntityType::Trip,
                eid.clone(),
                eid.clone(),
                "trips.txt",
                Some(rec.line),
                Some("route_id"),
                Some(rec_route_id.to_string()),
                None,
                format!("'{}' hattı routes.txt'te tanımlı değil.", rec_route_id),
                "Geçerli bir route_id kullanın.",
            ));
        }

        // TRP_003: service_id referansı
        if !rec_service_id.is_empty() && !map.services.contains(rec_service_id) {
            notices.push(notice(
                ctr,
                "TRP_003",
                EntityType::Trip,
                eid.clone(),
                eid.clone(),
                "trips.txt",
                Some(rec.line),
                Some("service_id"),
                Some(rec_service_id.to_string()),
                None,
                format!(
                    "service_id '{}' calendar veya calendar_dates'te tanımlı değil.",
                    rec_service_id
                ),
                "Geçerli bir service_id kullanın.",
            ));
        }

        // TRP_004: shape_id referansı (varsa)
        if shapes_available {
            if let Some(sid) = ti.shape_id(rec) {
            if !map.shape_points.contains_key(sid) {
                notices.push(notice(
                    ctr,
                    "TRP_004",
                    EntityType::Trip,
                    eid.clone(),
                    Some(sid.to_string()), // scope_key = shape_id
                    "trips.txt",
                    Some(rec.line),
                    Some("shape_id"),
                    Some(sid.to_string()),
                    None,
                    format!("'{}' güzergahı shapes.txt'te tanımlı değil.", sid),
                    "Geçerli bir shape_id kullanın veya alanı boş bırakın.",
                ));
            }
            }
        }

        // TRP_019: continuous service aktifken shape_id zorunlu
        if rec.shape_idx == 0 {
            let route_continuous = map
                .routes
                .get(rec_route_id)
                .and_then(|&idx| records.routes.get(idx))
                .map(|r| {
            // 🔴 DEĞER KÜMESİ {0, 2, 3}'TÜR — 1 DEĞİLDİR (#170).
            //
            // Spec, `trips.shape_id` için: "Required if the trip has a continuous
            // pickup or drop-off behavior defined either in routes.txt or in
            // stop_times.txt." Enum ise şöyle tanımlı:
            //   0        → sürekli biniş VAR
            //   1 / boş  → sürekli biniş YOK        ← şart TETİKLEMEZ
            //   2        → telefonla ayarlanan sürekli biniş  → VAR
            //   3        → sürücüyle ayarlanan sürekli biniş  → VAR
            //
            // Yüklem `Some(0) | Some(1)` idi: hem 1'i (sürekli servis YOK) dahil edip
            // geçerli feed'den shape_id talep ediyor, hem 2/3'ü (sürekli servis VAR)
            // atlayarak gerçek ihlali kaçırıyordu. Korpusta 100 feed / 215.288 bulgu;
            // örneklenen üç feed'in ikisinde değer 1'di (`tdg-83921`, `tdg-81645`).
                    matches!(r.continuous_pickup, Some(0) | Some(2) | Some(3))
                        || matches!(r.continuous_drop_off, Some(0) | Some(2) | Some(3))
                })
                .unwrap_or(false);

            if route_continuous || trip_has_continuous_stm.contains(rec.trip_id.as_str()) {
                notices.push(notice(
                    ctr,
                    "TRP_019",
                    EntityType::Trip,
                    eid.clone(),
                    eid.clone(),
                    "trips.txt",
                    Some(rec.line),
                    Some("shape_id"),
                    None,
                    None,
                    format!(
                        "trip_id '{}' için continuous_pickup/drop_off aktif — shape_id zorunludur.",
                        rec.trip_id
                    ),
                    "Bu sefer için shapes.txt'e güzergah verisi ekleyip trips.txt'te shape_id'yi doldurun.",
                ));
            }
        }
    }
}

// �"?�"? STM_001-002: stop_times cross-ref �"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?


// �"?�"? PTH_014: pathway_id referans bütünlü�Yü �?" aynı istasyon cross-ref �"?�"?�"?�"?�"?�"?�"?�"?�"?

/// PTH_014: bu mesafenin altındaki cross-station pathway'ler komşu durakların
/// gerçekçi yürüme bağlantısı sayılır ve bastırılır (VBB Potsdam "Platz der
/// Einheit" alt-durakları ~150 m). Bu mesafenin üstü olası yanlış stop_id
/// referansıdır ve tetiklenir.
const PTH014_WALK_MAX_M: f64 = 300.0;

/// PTH_014 yürüme mesafesi kanıtı (m). Guard'ın sorduğu soru "uçlar arasındaki
/// YÜRÜYÜŞ kısa mı", dolayısıyla kaynak sırası şudur (#187):
///
/// 1. **Beyan edilen `pathways.txt.length`.** Geçidin gerçek yürüme uzunluğunu
///    ölçen tek alan budur; üretici onu bilerek yazar.
/// 2. **Uç koordinatları** (`length` yoksa). Geodezik mesafe kuş uçuşudur, gerçek
///    yürüme mesafesi değildir — yalnız daha iyi bir kaynak yokken kullanılır.
/// 3. **İkisi de yoksa `None`** → guard uygulanmaz, kural normal yoluna devam eder.
///
/// #187 · MBTA Winter Street Concourse (`dwnxg-385`/`386`): `node-dwnxg-concourse`
/// koordinatsız (generic node'da koordinat OPSİYONELDİR), `length=142,3416` m.
///
/// `length` yalnız sonlu ve `>= 0` ise kanıt sayılır: `PTH_006` negatif değeri
/// raporlar ama değeri yine de kayda geçirir, geçersiz beyan burada elenir.
fn pth014_distance_m(
    rec: &crate::k2::pathways::PathwayRecord,
    stop_coord: &HashMap<&str, (f64, f64)>,
) -> Option<f64> {
    if let Some(length) = rec.length.filter(|v| v.is_finite() && *v >= 0.0) {
        return Some(length);
    }
    match (
        stop_coord.get(rec.from_stop_id.as_str()),
        stop_coord.get(rec.to_stop_id.as_str()),
    ) {
        (Some(&(la, lo)), Some(&(lb, lob))) => {
            Some(crate::k5_derived::haversine_km(la, lo, lb, lob) * 1000.0)
        }
        _ => None,
    }
}

fn check_pathways(
    records: &EntityRecords,
    _map: &EntityMap,
    notices: &mut Vec<Notice>,
    ctr: &mut u32,
) {
    if records.pathways.is_empty() {
        return;
    }

    // stop_id �?' parent_station
    let stop_parent: HashMap<&str, &str> = records
        .stops
        .iter()
        .filter_map(|s| {
            s.row
                .get("parent_station")
                .map(|p| (s.stop_id.as_str(), p.trim()))
        })
        .filter(|(_, p)| !p.is_empty())
        .collect();

    // stop_id �?' location_type
    let stop_loc: HashMap<&str, Option<u32>> = records
        .stops
        .iter()
        .map(|s| (s.stop_id.as_str(), s.location_type))
        .collect();

    // stop_id → (lat, lon) — kısa mesafe guard'ı için (yalnız ikisi de dolu olanlar)
    let stop_coord: HashMap<&str, (f64, f64)> = records
        .stops
        .iter()
        .filter_map(|s| match (s.stop_lat, s.stop_lon) {
            (Some(la), Some(lo)) => Some((s.stop_id.as_str(), (la, lo))),
            _ => None,
        })
        .collect();

    // PTH_014: from_stop_id ve to_stop_id farklı istasyonlara ait �?' referans bütünlü�Yü ihlali
    for rec in &records.pathways {
        if rec.pathway_id.is_empty() {
            continue;
        }
        // A-daraltma (2026-07-24, VBB mdb-782): location_type=2 (giriş/çıkış)
        // istasyonun dış-dünya sınırıdır. Bir ucu girişse pathway sokak üzerinden
        // komşu istasyona geçebilir — bu meşru bir aktarma yürüyüşüdür (VBB rail
        // istasyonları Alexanderplatz/Spandau vb. bu deseni yaygın kullanır), hata
        // değil. İstasyon sınırını aşmak yalnız iki uç da İÇ konumken (peron=0 /
        // generic node=3 / boarding area=4) topoloji sorunu sayılır.
        let from_lt = stop_loc.get(rec.from_stop_id.as_str()).and_then(|l| *l);
        let to_lt = stop_loc.get(rec.to_stop_id.as_str()).and_then(|l| *l);
        if from_lt == Some(2) || to_lt == Some(2) {
            continue;
        }
        let from_station = station_context(rec.from_stop_id.as_str(), &stop_parent, &stop_loc);
        let to_station = station_context(rec.to_stop_id.as_str(), &stop_parent, &stop_loc);
        if let (Some(fs), Some(ts)) = (from_station, to_station) {
            if fs != ts {
                // Kısa mesafeli cross-station pathway: komşu durakların gerçekçi
                // yürüme bağlantısıdır (VBB Potsdam "Platz der Einheit/West" ↔
                // ".../Bildungsforum" ~150 m), topoloji hatası değil → atla. Yalnız
                // uçlar birbirinden uzaksa (olası yanlış stop_id referansı) tetiklenir.
                // Mesafe kaynağı için bkz. `pth014_distance_m`: önce beyan edilen
                // `length`, o yoksa uç koordinatları (#187).
                if pth014_distance_m(rec, &stop_coord)
                    .is_some_and(|meters| meters <= PTH014_WALK_MAX_M)
                {
                    continue;
                }
                let mut n = notice(
                    ctr,
                    "PTH_014",
                    EntityType::Pathway,
                    Some(rec.pathway_id.clone()),
                    Some(rec.pathway_id.clone()),
                    "pathways.txt",
                    Some(rec.line),
                    Some("from_stop_id|to_stop_id"),
                    Some(format!("from_station={fs}, to_station={ts}")),
                    Some("must be within the same station".to_string()),
                    format!(
                        "pathway_id '{}' farklı istasyonlar arası bağlantı kuruyor (from_station='{fs}', to_station='{ts}').",
                        rec.pathway_id
                    ),
                    "İki ucun da aynı parent station'a çözüldüğünü doğrulayın. İki istasyon arasındaki geçit bilinçliyse dış ucunu giriş/çıkış (location_type=2) olarak modelleyin.",
                );
                let mut d = std::collections::BTreeMap::new();
                d.insert("from_station".to_string(), fs.to_string());
                d.insert("to_station".to_string(), ts.to_string());
                n.details = Some(d);
                notices.push(n);
            }
        }
    }

    // PTH_019: generic node (location_type=3) yalnızca tek pathway'e bağlı → dead-end
    {
        // Sayılan şey KOMŞU sayısıdır, pathway SATIRI değil. Aynı iki durak arasına
        // iki ayrı pathway satırı (ör. her yön için bir tane) yazmak node'u çıkmaz
        // olmaktan kurtarmaz: hâlâ tek bir yere açılıyordur. Satır saymak bu düzeni
        // kullanan feed'lerde kuralı kör bırakıyordu. MobilityData da aynı şeyi
        // söylüyor: "A generic node has only one incident location in a pathway graph."
        // Kendine dönen kenar komşu sayılmaz; o PTH_012'nin (döngü) konusudur.
        let mut neighbours: HashMap<&str, std::collections::BTreeSet<&str>> = HashMap::new();
        for rec in &records.pathways {
            let (f, t) = (rec.from_stop_id.as_str(), rec.to_stop_id.as_str());
            if f == t {
                continue;
            }
            neighbours.entry(f).or_default().insert(t);
            neighbours.entry(t).or_default().insert(f);
        }
        for stop in &records.stops {
            if stop.location_type != Some(3) {
                continue;
            }
            let count = neighbours
                .get(stop.stop_id.as_str())
                .map(|s| s.len())
                .unwrap_or(0);
            if count == 1 {
                notices.push(notice(
                    ctr,
                    "PTH_019",
                    EntityType::Stop,
                    Some(stop.stop_id.clone()),
                    stop.stop_name.clone(),
                    "stops.txt",
                    Some(stop.line),
                    Some("stop_id"),
                    Some("1".to_string()),
                    Some("≥2".to_string()),
                    format!(
                        "stop_id '{}' (generic node, location_type=3) yalnızca 1 pathway'e bağlı — geçiş ağında çıkmaz.",
                        stop.stop_id
                    ),
                    "Generic node en az 2 pathway ile bağlanmalıdır (giriş ve çıkış).",
                ));
            }
        }
    }

    // LVL_006: asansör (pathway_mode=5) uç duraklarından birinde level_id eksik
    {
        let stop_level: HashMap<&str, bool> = records
            .stops
            .iter()
            .map(|s| (s.stop_id.as_str(), s.level_id.as_deref().is_some_and(|l| !l.is_empty())))
            .collect();

        let mut seen: HashSet<&str> = HashSet::new();
        for rec in &records.pathways {
            if rec.pathway_mode != Some(5) {
                continue;
            }
            for stop_id in [rec.from_stop_id.as_str(), rec.to_stop_id.as_str()] {
                if seen.contains(stop_id) {
                    continue;
                }
                let has_level = stop_level.get(stop_id).copied().unwrap_or(false);
                if !has_level {
                    seen.insert(stop_id);
                    let stop_rec = records.stops.iter().find(|s| s.stop_id == stop_id);
                    notices.push(notice(
                        ctr,
                        "LVL_006",
                        EntityType::Stop,
                        Some(stop_id.to_string()),
                        stop_rec.and_then(|s| s.stop_name.clone()),
                        "stops.txt",
                        stop_rec.map(|s| s.line),
                        Some("level_id"),
                        None,
                        Some("dolu".to_string()),
                        format!(
                            "stop_id '{stop_id}' bir asansör pathway'ine bağlı ancak level_id tanımlı değil."
                        ),
                        "stops.txt'de bu durağa level_id atayın ve levels.txt'de ilgili katı tanımlayın.",
                    ));
                }
            }
        }
    }
}

// �"?�"? CAL_009, CAL_011: takvim cross-ref �"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?

fn check_calendar(
    records: &EntityRecords,
    _map: &EntityMap,
    notices: &mut Vec<Notice>,
    ctr: &mut u32,
    today: u32,
) {
    // CAL_009: feed yayın aralı�Yı tamamen sona ermi�Yse (hiçbir servis aktif)
    if !records.calendars.is_empty() {
        let all_expired = records.calendars.iter().all(|r| {
            r.end_date
                .map(|d| date_to_u32(d) < today)
                .unwrap_or(false)
        });
        if all_expired {
            // Her servis için bir notice
            for rec in &records.calendars {
                if rec.service_id.is_empty() {
                    continue;
                }
                notices.push(notice(
                    ctr,
                    "CAL_009",
                    EntityType::Service,
                    Some(rec.service_id.clone()),
                    Some(rec.service_id.clone()),
                    "calendar.txt",
                    Some(rec.line),
                    Some("end_date"),
                    rec.end_date.map(|d| format!("{}", date_to_u32(d))),
                    None,
                    format!(
                        "service_id '{}' için end_date geçmişte kaldı; feed'in tüm hizmetleri sona ermiş.",
                        rec.service_id
                    ),
                    "Feed'in geçerlilik tarihini güncelleyin.",
                ));
            }
        }
    }

    // CAL_011: servis hiçbir trip tarafından kullanılmıyor (orphan)
    let ti_cal = &records.trip_interns;
    let used_services: HashSet<&str> = records
        .trips
        .iter()
        .map(|t| ti_cal.service_id(t))
        .collect();
    for rec in &records.calendars {
        if rec.service_id.is_empty() {
            continue;
        }
        if !used_services.contains(rec.service_id.as_str()) {
            notices.push(notice(
                ctr,
                "CAL_011",
                EntityType::Service,
                Some(rec.service_id.clone()),
                Some(rec.service_id.clone()),
                "calendar.txt",
                Some(rec.line),
                Some("service_id"),
                Some(rec.service_id.clone()),
                None,
                format!("'{}' takvimi hiçbir sefer tarafından kullanılmıyor.", rec.service_id),
                "Kullanılmayan servisi silin veya bir sefere atayın.",
            ));
        }
    }

    // CAL_018: haftalık bazda tüm günler pasif VE calendar_dates'te exception_type=1 override yok
    {
        let services_with_added: HashSet<&str> = records.calendar_dates.added.keys()
            .map(|s| s.as_str()).collect();
        for rec in &records.calendars {
            if rec.service_id.is_empty() {
                continue;
            }
            let all_inactive = rec.days.iter().all(|d| d.is_none_or(|v| v == 0));
            if all_inactive && !services_with_added.contains(rec.service_id.as_str()) {
                notices.push(notice(
                    ctr,
                    "CAL_018",
                    EntityType::Service,
                    Some(rec.service_id.clone()),
                    Some(rec.service_id.clone()),
                    "calendar.txt",
                    Some(rec.line),
                    // Olgu YEDİ gün alanının birlikte 0 olmasıdır (CAL_006 ile aynı alan kümesi).
                    Some("monday|tuesday|wednesday|thursday|friday|saturday|sunday"),
                    None,
                    None,
                    format!(
                        "'{}' takviminde haftanın tüm günleri pasif (0) ve calendar_dates.txt'te aktif gün (exception_type=1) de tanımlı değil; bu servis hiç çalışmaz.",
                        rec.service_id
                    ),
                    "Haftalık günlerden en az birini 1 yapın ya da calendar_dates.txt'e bu servis için exception_type=1 kayıt ekleyin.",
                ));
            }
        }
    }
}

// �"?�"? CLD_004: calendar_dates kapsamlılık kontrolü �"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?

fn check_calendar_dates(
    records: &EntityRecords,
    _map: &EntityMap,
    notices: &mut Vec<Notice>,
    ctr: &mut u32,
) {
    // CLD_004: calendar.txt yokken calendar_dates tüm tarihleri kapsıyor mu?
    // Bu kontrol: calendar_dates'te exception_type=1 (eklenen gün) olan
    // servis_id'lerinin en az 1 tarihi olup olmadı�Yını kontrol eder.
    if records.calendars.is_empty() && !records.calendar_dates.exception_count.is_empty() {
        // added.keys() = service_ids with at least one exception_type=1 record (even invalid date)
        let has_added: HashSet<&str> = records.calendar_dates.added.keys()
            .map(|s| s.as_str()).collect();
        // trips'teki her service_id en az bir exception_type=1 kaydına sahip olmalı
        let ti_cld = &records.trip_interns;
        let trip_services: HashSet<&str> = records
            .trips
            .iter()
            .map(|t| ti_cld.service_id(t))
            .collect();
        for sid in &trip_services {
            if !has_added.contains(*sid) {
                // Bu service_id için exception_type=1 kaydı yok
                // �?' calendar.txt olmadan bu servis hiç çalı�Ymaz
                let line = records.calendar_dates.first_line.get(*sid).copied();
                notices.push(notice(
                    ctr,
                    "CLD_004",
                    EntityType::Service,
                    Some(sid.to_string()),
                    Some(sid.to_string()),
                    "calendar_dates.txt",
                    line,
                    Some("service_id"),
                    Some(sid.to_string()),
                    None,
                    format!(
                        "calendar.txt yok; service_id '{sid}' için exception_type=1 (aktif gün) kaydı bulunamadı.",
                    ),
                    "calendar.txt'i ekleyin ya da her servis için en az bir exception_type=1 kaydı tanımlayın.",
                ));
            }
        }
    }
}

// �"?�"? FRQ_001: frekans cross-ref �"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?

fn check_frequencies(
    records: &EntityRecords,
    map: &EntityMap,
    notices: &mut Vec<Notice>,
    ctr: &mut u32,
    trips_in_stm: &HashSet<&str>,
) {
    for rec in &records.frequencies {
        if rec.trip_id.is_empty() { continue; }

        // FRQ_001: trip_id trips.txt'te tanımlı değil
        if !map.trips.contains_key(rec.trip_id.as_str()) {
            notices.push(notice(
                ctr,
                "FRQ_001",
                EntityType::Trip,
                Some(rec.trip_id.clone()),
                Some(rec.trip_id.clone()),
                "frequencies.txt",
                Some(rec.line),
                Some("trip_id"),
                Some(rec.trip_id.clone()),
                None,
                format!("'{}' sefer kodu trips.txt'te tanımlı değil.", rec.trip_id),
                "Geçerli bir trip_id kullanın.",
            ));
        }

        // TRP_017: frekans tabanlı, trips.txt'te GEÇERLİ sefer stop_times'ta eksik.
        // trips.txt'te olmayan trip için FRQ_001 kök FK hatasıdır; türev notice üretme.
        if map.trips.contains_key(rec.trip_id.as_str())
            && !trips_in_stm.contains(rec.trip_id.as_str()) {
            notices.push(notice(
                ctr,
                "TRP_017",
                EntityType::Trip,
                Some(rec.trip_id.clone()),
                Some(rec.trip_id.clone()),
                "frequencies.txt",
                Some(rec.line),
                Some("trip_id"),
                Some(rec.trip_id.clone()),
                None,
                format!("'{}' frekans tabanlı seferin stop_times.txt'te durağı tanımlanmamış.", rec.trip_id),
                "stop_times.txt'e bu sefer için durak saatlerini ekleyin.",
            ));
        }
    }
}

// �"?�"? TRF_006-009, TRF_013-015: transfer cross-ref �"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?

fn check_transfers(
    records: &EntityRecords,
    map: &EntityMap,
    notices: &mut Vec<Notice>,
    ctr: &mut u32,
    trips_in_stm: &HashSet<&str>,
    // #15/#38: trip_stop_set iç tipi u32 (intern indeks); stop_id_to_idx ile dönüştür.
    trip_stops: &FxHashMap<SmolStr, FxHashSet<u32>>,
    stop_id_to_idx: &FxHashMap<smol_str::SmolStr, u32>,
) {
    let ti_trf = &records.trip_interns;
    for rec in &records.transfers {
        let ttype = rec.transfer_type;

        // TRF_003: from_stop_id veya to_stop_id stops.txt'te bulunamıyor
        for (field, stop_id) in [
            ("from_stop_id", rec.from_stop_id.as_str()),
            ("to_stop_id",   rec.to_stop_id.as_str()),
        ] {
            if !stop_id.is_empty() && !map.stops.contains_key(stop_id) {
                notices.push(notice(
                    ctr, "TRF_003", EntityType::Transfer,
                    None, None, "transfers.txt", Some(rec.line), Some(field),
                    Some(stop_id.to_string()), None,
                    format!("{field} '{stop_id}' stops.txt'te tanımlı değil."),
                    "Geçerli bir stop_id kullanın veya eksik durağı stops.txt'e ekleyin.",
                ));
            }
        }

        // TRF_010: min_transfer_time çok uzun (> 3600 s)
        const MAX_TRANSFER_SEC: u32 = 3600;
        if let Some(mtt) = rec.min_transfer_time {
            if mtt > MAX_TRANSFER_SEC {
                notices.push(notice(
                    ctr, "TRF_010", EntityType::Transfer,
                    None, None, "transfers.txt", Some(rec.line), Some("min_transfer_time"),
                    Some(format!("{mtt}s")), Some(format!("≤ {MAX_TRANSFER_SEC}s")),
                    format!("'{}' → '{}' aktarmasının min_transfer_time değeri {}s ({:.0} dakika) çok uzun.",
                        rec.from_stop_id, rec.to_stop_id, mtt, mtt as f64 / 60.0),
                    "min_transfer_time değerini gözden geçirin; gerçekçi bir aktarma süresi girin.",
                ));
            }
            if mtt > 0 {
                let pair = map.stops.get(rec.from_stop_id.as_str()).and_then(|&i| records.stops.get(i)).zip(map.stops.get(rec.to_stop_id.as_str()).and_then(|&i| records.stops.get(i)));
                if let Some((a, b)) = pair {
                    if let (Some(lat1), Some(lon1), Some(lat2), Some(lon2)) = (a.stop_lat, a.stop_lon, b.stop_lat, b.stop_lon) {
                        let p1 = lat1.to_radians(); let p2 = lat2.to_radians();
                        let dp = (lat2 - lat1).to_radians(); let dl = (lon2 - lon1).to_radians();
                        let h = (dp / 2.0).sin().powi(2) + p1.cos() * p2.cos() * (dl / 2.0).sin().powi(2);
                        let distance_m = 6_371_000.0 * 2.0 * h.sqrt().atan2((1.0 - h).sqrt());
                        let speed = distance_m / mtt as f64;
                        if speed > 2.0 {
                            notices.push(notice(ctr, "TRF_020", EntityType::Transfer, None, None,
                                "transfers.txt", Some(rec.line), Some("min_transfer_time"),
                                Some(format!("{speed:.2} m/s")), Some("≤ 2.00 m/s".to_string()),
                                format!("'{}' → '{}' aktarması {:.0}m mesafeyi {}s içinde yürümeyi gerektiriyor ({speed:.2} m/s).", rec.from_stop_id, rec.to_stop_id, distance_m, mtt),
                                "min_transfer_time değerini artırın veya aktarma duraklarını doğrulayın."));
                        }
                    }
                }
            }
        }

        // TRF_018: aynı seferde aktarma (from_trip_id == to_trip_id)
        if let (Some(ref fti), Some(ref tti)) = (&rec.from_trip_id, &rec.to_trip_id) {
            if fti == tti {
                notices.push(notice(
                    ctr, "TRF_018", EntityType::Transfer,
                    None, None, "transfers.txt", Some(rec.line), Some("from_trip_id"),
                    Some(fti.clone()), None,
                    format!("from_trip_id ve to_trip_id aynı sefer ('{fti}'); aktarmanın bir anlamı yok."),
                    "from_trip_id ve to_trip_id farklı seferler olmalıdır.",
                ));
            }
        }

        // TRF_006: from_trip_id referansı
        if let Some(ref fti) = rec.from_trip_id {
            if !map.trips.contains_key(fti.as_str()) {
                notices.push(notice(
                    ctr,
                    "TRF_006",
                    EntityType::Transfer,
                    None,
                    Some(fti.clone()),
                    "transfers.txt",
                    Some(rec.line),
                    Some("from_trip_id"),
                    Some(fti.clone()),
                    None,
                    format!("from_trip_id '{fti}' trips.txt'te tanımlı değil."),
                    "Geçerli bir trip_id kullanın.",
                ));
            }
        }

        // TRF_007: to_trip_id referansı
        if let Some(ref tti) = rec.to_trip_id {
            if !map.trips.contains_key(tti.as_str()) {
                notices.push(notice(
                    ctr,
                    "TRF_007",
                    EntityType::Transfer,
                    None,
                    Some(tti.clone()),
                    "transfers.txt",
                    Some(rec.line),
                    Some("to_trip_id"),
                    Some(tti.clone()),
                    None,
                    format!("to_trip_id '{tti}' trips.txt'te tanımlı değil."),
                    "Geçerli bir trip_id kullanın.",
                ));
            }
        }

        // TRF_008: from_route_id referansı
        if let Some(ref fri) = rec.from_route_id {
            if !map.routes.contains_key(fri.as_str()) {
                notices.push(notice(
                    ctr,
                    "TRF_008",
                    EntityType::Transfer,
                    None,
                    Some(fri.clone()),
                    "transfers.txt",
                    Some(rec.line),
                    Some("from_route_id"),
                    Some(fri.clone()),
                    None,
                    format!("from_route_id '{fri}' routes.txt'te tanımlı değil."),
                    "Geçerli bir route_id kullanın.",
                ));
            }
        }

        // TRF_009: to_route_id referansı
        if let Some(ref tri) = rec.to_route_id {
            if !map.routes.contains_key(tri.as_str()) {
                notices.push(notice(
                    ctr,
                    "TRF_009",
                    EntityType::Transfer,
                    None,
                    Some(tri.clone()),
                    "transfers.txt",
                    Some(rec.line),
                    Some("to_route_id"),
                    Some(tri.clone()),
                    None,
                    format!("to_route_id '{tri}' routes.txt'te tanımlı değil."),
                    "Geçerli bir route_id kullanın.",
                ));
            }
        }

        // TRF_013: transfer_type=4/5 için from_trip_id ve to_trip_id zorunlu
        if matches!(ttype, Some(4) | Some(5))
            && (rec.from_trip_id.is_none() || rec.to_trip_id.is_none()) {
                notices.push(notice(
                    ctr,
                    "TRF_013",
                    EntityType::Transfer,
                    None,
                    None,
                    "transfers.txt",
                    Some(rec.line),
                    Some("from_trip_id|to_trip_id"),
                    None,
                    Some("both trip_id values are required for type 4/5".to_string()),
                    format!(
                        "transfer_type={} için from_trip_id ve to_trip_id zorunludur.",
                        ttype.unwrap()
                    ),
                    "Linked-trip transfer için her iki trip_id alanını doldurun.",
                ));
            }

        // TRF_014: in-seat aktarma için seferde stop_times kaydı yok
        if matches!(ttype, Some(4) | Some(5)) {
            for (field, trip_id_opt) in [
                ("from_trip_id", &rec.from_trip_id),
                ("to_trip_id",   &rec.to_trip_id),
            ] {
                if let Some(tid) = trip_id_opt {
                    if map.trips.contains_key(tid.as_str()) && !trips_in_stm.contains(tid.as_str()) {
                        notices.push(notice(
                            ctr, "TRF_014", EntityType::Transfer,
                            None, None, "transfers.txt", Some(rec.line), Some(field),
                            Some(tid.clone()), None,
                            format!("{field} '{tid}' trips.txt'te mevcut ancak stop_times.txt'te hiç kaydı yok; in-seat aktarma gerçekleşemez."),
                            "İlgili seferin stop_times.txt kayıtlarını ekleyin.",
                        ));
                    }
                }
            }
        }

        // TRF_019: in-seat aktarmada farklı route_type (InconsistentRouteTypeForInSeatTransfer)
        if matches!(ttype, Some(4) | Some(5)) {
            if let (Some(ref fti), Some(ref tti)) = (&rec.from_trip_id, &rec.to_trip_id) {
                let from_rtype = map.trips.get(fti.as_str())
                    .and_then(|&tidx| {
                        let rid = ti_trf.route_id(&records.trips[tidx]);
                        map.routes.get(rid).and_then(|&ridx| records.routes[ridx].route_type)
                    });
                let to_rtype = map.trips.get(tti.as_str())
                    .and_then(|&tidx| {
                        let rid = ti_trf.route_id(&records.trips[tidx]);
                        map.routes.get(rid).and_then(|&ridx| records.routes[ridx].route_type)
                    });
                if let (Some(fr), Some(tr)) = (from_rtype, to_rtype) {
                    if fr != tr {
                        notices.push(notice(
                            ctr, "TRF_019", EntityType::Transfer,
                            None, None, "transfers.txt", Some(rec.line), Some("from_trip_id"),
                            Some(format!("from={fr}, to={tr}")), None,
                            format!(
                                "In-seat aktarma: from_trip_id '{fti}' route_type={fr}, to_trip_id '{tti}' route_type={tr} — uyumsuz."
                            ),
                            "In-seat aktarma yapan seferler aynı route_type'a sahip olmalıdır.",
                        ));
                    }
                }
            }
        }

        // TRF_017: sefer aktarmasında route uyumsuzluğu
        if let (Some(ref fti), Some(ref fri)) = (&rec.from_trip_id, &rec.from_route_id) {
            if let Some(&tidx) = map.trips.get(fti.as_str()) {
                if ti_trf.route_id(&records.trips[tidx]) != fri.as_str() {
                    notices.push(notice(
                        ctr, "TRF_017", EntityType::Transfer,
                        None, None, "transfers.txt", Some(rec.line), Some("from_route_id"),
                        Some(fri.clone()),
                        Some(ti_trf.route_id(&records.trips[tidx]).to_string()),
                        format!(
                            "from_trip_id '{fti}' route_id '{}' ile ilişkili, ancak from_route_id '{fri}' belirtilmiş.",
                            ti_trf.route_id(&records.trips[tidx])
                        ),
                        "from_route_id'yi ilgili seferin route_id'siyle eşleştirin.",
                    ));
                }
            }
        }
        if let (Some(ref tti), Some(ref tri)) = (&rec.to_trip_id, &rec.to_route_id) {
            if let Some(&tidx) = map.trips.get(tti.as_str()) {
                if ti_trf.route_id(&records.trips[tidx]) != tri.as_str() {
                    notices.push(notice(
                        ctr, "TRF_017", EntityType::Transfer,
                        None, None, "transfers.txt", Some(rec.line), Some("to_route_id"),
                        Some(tri.clone()),
                        Some(ti_trf.route_id(&records.trips[tidx]).to_string()),
                        format!(
                            "to_trip_id '{tti}' route_id '{}' ile ilişkili, ancak to_route_id '{tri}' belirtilmiş.",
                            ti_trf.route_id(&records.trips[tidx])
                        ),
                        "to_route_id'yi ilgili seferin route_id'siyle eşleştirin.",
                    ));
                }
            }
        }

        // XFL_021: from_stop_id, from_trip_id'nin stop_times kayıtlarında bulunmuyor
        if let Some(ref fti) = rec.from_trip_id {
            let fsid = rec.from_stop_id.as_str();
            if !fsid.is_empty() && map.trips.contains_key(fti.as_str()) {
                let in_trip = stop_id_to_idx.get(fsid)
                    .and_then(|idx| trip_stops.get(fti.as_str()).map(|s| s.contains(idx)))
                    .unwrap_or(false);
                if !in_trip {
                    notices.push(notice(
                        ctr, "XFL_021", EntityType::Transfer,
                        None, None, "transfers.txt", Some(rec.line), Some("from_stop_id"),
                        Some(fsid.to_string()), None,
                        format!("from_stop_id '{fsid}', from_trip_id '{fti}' seferinin stop_times kayıtlarında yer almıyor."),
                        "from_stop_id'yi from_trip_id'nin gerçekten geçtiği bir durak ile değiştirin.",
                    ));
                }
            }
        }
        if let Some(ref tti) = rec.to_trip_id {
            let tsid = rec.to_stop_id.as_str();
            if !tsid.is_empty() && map.trips.contains_key(tti.as_str()) {
                let in_trip = stop_id_to_idx.get(tsid)
                    .and_then(|idx| trip_stops.get(tti.as_str()).map(|s| s.contains(idx)))
                    .unwrap_or(false);
                if !in_trip {
                    notices.push(notice(
                        ctr, "XFL_021", EntityType::Transfer,
                        None, None, "transfers.txt", Some(rec.line), Some("to_stop_id"),
                        Some(tsid.to_string()), None,
                        format!("to_stop_id '{tsid}', to_trip_id '{tti}' seferinin stop_times kayıtlarında yer almıyor."),
                        "to_stop_id'yi to_trip_id'nin gerçekten geçtiği bir durak ile değiştirin.",
                    ));
                }
            }
        }

        // XFL_020: transfer kaydındaki (trip_id, route_id) çifti geçersiz — sefer belirtilen hatta ait değil
        for (trip_opt, route_opt, trip_field, route_field) in [
            (&rec.from_trip_id, &rec.from_route_id, "from_trip_id", "from_route_id"),
            (&rec.to_trip_id,   &rec.to_route_id,   "to_trip_id",   "to_route_id"),
        ] {
            if let (Some(ref tid), Some(ref rid)) = (trip_opt, route_opt) {
                if let Some(&tidx) = map.trips.get(tid.as_str()) {
                    let actual_route = ti_trf.route_id(&records.trips[tidx]);
                    if actual_route != rid.as_str() {
                        notices.push(notice(
                            ctr, "XFL_020", EntityType::Transfer,
                            None, None, "transfers.txt", Some(rec.line), Some(route_field),
                            Some(rid.clone()), Some(actual_route.to_string()),
                            format!(
                                "{trip_field} '{tid}' seferinin gerçek hattı '{}', ancak {route_field} '{rid}' belirtilmiş.",
                                actual_route
                            ),
                            "Aktarma kaydındaki route_id'yi ilgili seferin gerçek hat kodu ile güncelleyin.",
                        ));
                    }
                }
            }
        }

        // TRF_021: transfers uç noktası yalnızca durak (0) veya istasyon (1) olabilir. Spec:
        // "Identifies a stop (location_type=0) or a station (location_type=1) where a
        // connection between routes begins." Giriş/çıkış (2), ara düğüm (3) ve biniş alanı (4)
        // aktarma referansı olamaz. Boş location_type spec'te 0 sayılır, geçerlidir.
        // TRF_015 ile örtüşmez: o, type=4/5'te location_type=1'i yasaklar — farklı değer kümesi.
        for (field, stop_id) in [
            ("from_stop_id", rec.from_stop_id.as_str()),
            ("to_stop_id", rec.to_stop_id.as_str()),
        ] {
            let Some(&sidx) = map.stops.get(stop_id) else { continue };
            if matches!(records.stops[sidx].location_type, Some(2) | Some(3) | Some(4)) {
                let lt = records.stops[sidx].location_type.unwrap_or(0);
                notices.push(notice(
                    ctr, "TRF_021", EntityType::Transfer,
                    None, None, "transfers.txt", Some(rec.line), Some(field),
                    Some(stop_id.to_string()), Some("location_type 0 or 1".to_string()),
                    format!(
                        "{field} '{stop_id}' location_type={lt}; aktarma yalnızca durak (0) veya istasyon (1) referansı alabilir.",
                    ),
                    "transfers.txt'te giriş/çıkış, ara düğüm veya biniş alanı yerine durak ya da istasyon referansı kullanın.",
                ));
            }
        }

        // TRF_015: type=4/5 için from/to_stop_id station (location_type=1) olamaz
        if matches!(ttype, Some(4) | Some(5)) {
            for (field, stop_id) in [
                ("from_stop_id", rec.from_stop_id.as_str()),
                ("to_stop_id", rec.to_stop_id.as_str()),
            ] {
                if let Some(&sidx) = map.stops.get(stop_id) {
                    if records.stops[sidx].location_type == Some(1) {
                        notices.push(notice(
                            ctr,
                            "TRF_015",
                            EntityType::Transfer,
                            None,
                            None,
                            "transfers.txt",
                            Some(rec.line),
                            Some(field),
                            Some(stop_id.to_string()),
                            Some("location_type != 1".to_string()),
                            format!(
                                "{field} '{stop_id}' bir istasyon (location_type=1); linked-trip transfer'da durak (0) olmalı.",
                            ),
                            "Linked-trip transfer için durak (location_type=0) referansı kullanın.",
                        ));
                    }
                }
            }
        }
    }

    // TRF_016: E�Yit specificity'de çakışan transfer kayıtları
    {
        let mut seen_keys: HashMap<String, u64> = HashMap::new();
        for rec in &records.transfers {
            // 🔴 BAĞLAMSIZ SATIRLAR TRF_012'NİN ALANI (Faz 3.1 adjudikasyonu).
            // İki kuralın kartları temiz bir ayrım İDDİA EDİYORDU — TRF_012 "yalnızca
            // from/to_trip_id ve from/to_route_id alanlarının TAMAMI boş olan satırlar",
            // TRF_016 "tam altı-alanlı anahtar" — ama kodda filtre YOKTU: bağlamsız bir
            // yineleme her iki kuralı da tetikliyordu. Ölçüm: `mdb-1859`'da ikisi de
            // 5.204 bulgu, aynı satırlarda, `observed` değeri `...|||| ` (bağlam boş).
            // Belgelenmiş karar kodda uygulanmamıştı; aynı olgu iki kez raporlanıyordu.
            let has_context = rec.from_trip_id.as_deref().is_some_and(|v| !v.is_empty())
                || rec.to_trip_id.as_deref().is_some_and(|v| !v.is_empty())
                || rec.from_route_id.as_deref().is_some_and(|v| !v.is_empty())
                || rec.to_route_id.as_deref().is_some_and(|v| !v.is_empty());
            if !has_context {
                continue;
            }
            let key = format!(
                "{}|{}|{}|{}|{}|{}",
                rec.from_stop_id,
                rec.to_stop_id,
                rec.from_trip_id.as_deref().unwrap_or(""),
                rec.to_trip_id.as_deref().unwrap_or(""),
                rec.from_route_id.as_deref().unwrap_or(""),
                rec.to_route_id.as_deref().unwrap_or(""),
            );
            if let Some(&prev_line) = seen_keys.get(&key) {
                notices.push(notice(
                    ctr,
                    "TRF_016",
                    EntityType::Transfer,
                    None,
                    None,
                    "transfers.txt",
                    Some(rec.line),
                    // Çakışmayı oluşturan altı alanın tamamı (DQ_021 bileşik anahtar deseni).
                    Some("from_stop_id|to_stop_id|from_trip_id|to_trip_id|from_route_id|to_route_id"),
                    Some(key.clone()),
                    None,
                    format!(
                        "Aynı (from_stop, to_stop, trip, route) kombinasyonunda çakışan transfer kaydı (ilk görünüm: satır {prev_line}); öncelik belirsiz.",
                    ),
                    "Çakışan transfer satırlarından birini kaldırın veya alanları farklılaştırın.",
                ));
            } else {
                seen_keys.insert(key, rec.line);
            }
        }
    }

    // TRF_012: yinelenen from_stop_id + to_stop_id çifti (trip/route gözetmeksizin basit tekrar)
    {
        let mut seen_stops: HashMap<(&str, &str), u64> = HashMap::new();
        for rec in &records.transfers {
            // Sadece trip ve route bağlamı olmayan satırlar için kontrol et
            if rec.from_trip_id.is_some() || rec.to_trip_id.is_some()
                || rec.from_route_id.is_some() || rec.to_route_id.is_some()
            {
                continue;
            }
            let key = (rec.from_stop_id.as_str(), rec.to_stop_id.as_str());
            if let Some(&prev_line) = seen_stops.get(&key) {
                notices.push(notice(
                    ctr, "TRF_012", EntityType::Transfer,
                    None, None, "transfers.txt", Some(rec.line),
                    Some("from_stop_id|to_stop_id"),
                    Some(format!("{}|{}", rec.from_stop_id, rec.to_stop_id)), None,
                    format!(
                        "'{}' → '{}' aktarma çifti yineleniyor (ilk görünüm: satır {prev_line}).",
                        rec.from_stop_id, rec.to_stop_id
                    ),
                    "Yinelenen aktarma satırını kaldırın.",
                ));
            } else {
                seen_stops.insert(key, rec.line);
            }
        }
    }
}

//�"?�"? FAR_008: fare referansı �"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?

fn check_fare_attributes(
    records: &EntityRecords,
    map: &EntityMap,
    notices: &mut Vec<Notice>,
    ctr: &mut u32,
) {
    // FAR_009 için: hangi fare_id'lerin fare_rules'u var
    let fares_with_rules: HashSet<&str> = records.fare_rules
        .iter()
        .map(|fr| fr.fare_id.as_str())
        .collect();

    let multi_agency = records.agencies.len() > 1;

    for rec in &records.fare_attributes {
        if let Some(ref aid) = rec.agency_id {
            if !map.agencies.contains_key(aid.as_str()) {
                notices.push(notice(
                    ctr,
                    "FAR_008",
                    EntityType::Fare,
                    Some(rec.fare_id.clone()),
                    Some(rec.fare_id.clone()),
                    "fare_attributes.txt",
                    Some(rec.line),
                    Some("agency_id"),
                    Some(aid.clone()),
                    None,
                    format!("'{}' işletici kodu agency.txt'te tanımlı değil.", aid),
                    "Geçerli bir agency_id kullanın.",
                ));
            }
        } else {
            // FIN_013: aynı sütun politikasını her fare için tekrarlama; feed başına tek özet.
            if records.fare_attributes.iter().find(|f| f.agency_id.is_none()).is_some_and(|first| std::ptr::eq(first, rec)) {
              let missing = records.fare_attributes.iter().filter(|f| f.agency_id.is_none()).count();
              notices.push(notice(
                ctr,
                "FIN_013",
                EntityType::Feed,
                None,
                None,
                "fare_attributes.txt",
                None,
                Some("agency_id"),
                Some(format!("{missing} missing records")),
                Some("dolu".to_string()),
                format!("{missing} ücret tarifesinde agency_id eksik{}.", if multi_agency { "; birden fazla kuruluşta zorunludur" } else { "; tek kuruluşta önerilir" }),
                "agency_id sütununu ücret tarifeleri için doldurun.",
              ));
            }
        }

        // FAR_009: bu fare_id'ye ait fare_rules kuralı yok
        if !rec.fare_id.is_empty() && !fares_with_rules.contains(rec.fare_id.as_str()) {
            notices.push(notice(
                ctr,
                "FAR_009",
                EntityType::Fare,
                Some(rec.fare_id.clone()),
                Some(rec.fare_id.clone()),
                "fare_attributes.txt",
                Some(rec.line),
                Some("fare_id"),
                Some(rec.fare_id.clone()),
                Some("a record in fare_rules.txt".to_string()),
                format!(
                    "Ücret tarifesi '{}' için fare_rules.txt'te hiç kural tanımlanmamış — hangi hatlara uygulanacağı belli değil.",
                    rec.fare_id
                ),
                "fare_rules.txt'e bu fare_id için en az bir kural ekleyin.",
            ));
        }
    }
}

// �"?�"? FRL_001-005: fare_rules cross-ref �"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?

fn check_fare_rules(
    records: &EntityRecords,
    map: &EntityMap,
    notices: &mut Vec<Notice>,
    ctr: &mut u32,
) {
    // FRL_006: fare_attributes tanımlı ama hiç fare_rules kaydı yok
    if records.fare_rules.is_empty() && !records.fare_attributes.is_empty() {
        notices.push(notice(
            ctr,
            "FRL_006",
            EntityType::Fare,
            None,
            None,
            "fare_rules.txt",
            None,
            None,
            None,
            None,
            format!(
                "fare_attributes.txt'te {} ücret tarifesi tanımlı ama fare_rules.txt kaydı yok.",
                records.fare_attributes.len()
            ),
            "fare_rules.txt'e ücret kuralları ekleyin veya ücret tarifelerini kaldırın.",
        ));
    }

    // FK ihlali toplayıcıları: (kimlik) → (ilk satır, kaç satırda tekrarladı).
    // BTreeMap → deterministik emit sırası.
    let mut frl001_bad: std::collections::BTreeMap<&str, (u64, u64)> = Default::default();
    let mut frl_zone_bad: std::collections::BTreeMap<(&str, &str, &str), (String, u64, u64)> =
        Default::default();

    for rec in &records.fare_rules {
        let eid = Some(rec.fare_id.clone());

        // FRL_001: fare_id referansı — DISTINCT kimlik başına tek notice (aşağıda emit).
        // Bir tanımsız fare_id feed'de binlerce satırda tekrar eder: mdb-2837'de
        // fare_attributes.txt HİÇ YOK ve fare_rules 1.128.840 satır → satır-başına emit
        // 1,1M notice üretiyordu (tek kök neden). STM_002 aynı FK ihlalini zaten
        // distinct stop_id başına toplar; FRL ailesi de öyle.
        if !rec.fare_id.is_empty() && !map.fare_attrs.contains_key(rec.fare_id.as_str()) {
            frl001_bad.entry(rec.fare_id.as_str()).or_insert((rec.line, 0)).1 += 1;
        }

        // FRL_002: route_id referansı (varsa)
        if let Some(ref rid) = rec.route_id {
            if !map.routes.contains_key(rid.as_str()) {
                notices.push(notice(
                    ctr,
                    "FRL_002",
                    EntityType::Fare,
                    eid.clone(),
                    Some(rid.clone()),
                    "fare_rules.txt",
                    Some(rec.line),
                    Some("route_id"),
                    Some(rid.clone()),
                    None,
                    format!("'{}' hattı routes.txt'te tanımlı değil.", rid),
                    "Geçerli bir route_id kullanın.",
                ));
            }
        }

        // FRL_003-005: zone_id referansları — DISTINCT (kural, zone_id) başına tek notice.
        for (rule, field, opt_id) in [
            ("FRL_003", "origin_id", &rec.origin_id),
            ("FRL_004", "destination_id", &rec.destination_id),
            ("FRL_005", "contains_id", &rec.contains_id),
        ] {
            if let Some(ref zid) = opt_id {
                if !map.zone_ids.contains(zid.as_str()) {
                    frl_zone_bad
                        .entry((rule, field, zid.as_str()))
                        .or_insert((rec.fare_id.clone(), rec.line, 0))
                        .2 += 1;
                }
            }
        }

        // FRL_007: hiç ayrıştırıcı kriter yok (route/zone/contains) — tüm seyahatlere uygulanır
        if rec.route_id.is_none()
            && rec.origin_id.is_none()
            && rec.destination_id.is_none()
            && rec.contains_id.is_none()
        {
            notices.push(notice(
                ctr,
                "FRL_007",
                EntityType::Fare,
                eid.clone(),
                eid.clone(),
                "fare_rules.txt",
                Some(rec.line),
                // Olgu DÖRT ayrıştırıcı alanın birlikte boş olmasıdır.
                Some("route_id|origin_id|destination_id|contains_id"),
                None,
                None,
                format!(
                    "'{}' ücret kuralı için route_id, origin_id, destination_id ve contains_id alanlarının tümü boş; bu kural tüm seyahatlere uygulanır.",
                    rec.fare_id
                ),
                "En az bir ayrıştırıcı kriter (route_id, origin_id, destination_id veya contains_id) belirtin.",
            ));
        }
    }

    // ── FRL_001/003/004/005: biriken FK ihlallerini DISTINCT kimlik başına emit et ──
    // observed = kaç satırda tekrarladığı; satır = ilk görüldüğü yer.
    for (fare_id, (line, rows)) in &frl001_bad {
        let eid = Some((*fare_id).to_string());
        notices.push(notice(
            ctr, "FRL_001", EntityType::Fare, eid.clone(), eid,
            "fare_rules.txt", Some(*line), Some("fare_id"),
            Some(format!("{fare_id} ({rows} rows)")), None,
            format!("'{fare_id}' ücret tarifesi fare_attributes.txt'te tanımlı değil ({rows} fare_rules satırında kullanılıyor)."),
            "Geçerli bir fare_id kullanın ya da fare_attributes.txt'e tanımını ekleyin.",
        ));
    }
    for ((rule, field, zid), (fare_id, line, rows)) in &frl_zone_bad {
        notices.push(notice(
            ctr, rule, EntityType::Fare, Some(fare_id.clone()), Some(fare_id.clone()),
            "fare_rules.txt", Some(*line), Some(field),
            Some(format!("{zid} ({rows} rows)")), None,
            format!("{field} '{zid}' stops.txt'te tanımlı bir zone_id değil ({rows} fare_rules satırında kullanılıyor)."),
            "Geçerli bir zone_id kullanın.",
        ));
    }

    // FRL_008: ücret sistemi route tabanlı ama bazı hatlar kapsam dışı
    if !records.fare_attributes.is_empty() && !records.fare_rules.is_empty() {
        let routes_in_rules: HashSet<&str> = records
            .fare_rules
            .iter()
            .filter_map(|fr| fr.route_id.as_deref())
            .collect();
        if !routes_in_rules.is_empty() {
            let uncovered: Vec<&str> = records
                .routes
                .iter()
                .filter(|r| !r.route_id.is_empty() && !routes_in_rules.contains(r.route_id.as_str()))
                .map(|r| r.route_id.as_str())
                .collect();
            if !uncovered.is_empty() {
                notices.push(notice(
                    ctr,
                    "FRL_008",
                    EntityType::Feed,
                    None,
                    None,
                    "fare_rules.txt",
                    None,
                    Some("route_id"),
                    Some(uncovered.join(", ")),
                    None,
                    format!(
                        "Şu hatlar için fare_rules.txt'te ücret kuralı tanımlı değil: {}.",
                        uncovered.join(", ")
                    ),
                    "Tüm hatlar için fare_rules.txt'e kapsayan kurallar ekleyin veya zone tabanlı ücretlendirmeye geçin.",
                ));
            }
        }
    }

    // FAR_010: aynı (route_id, origin_id, destination_id, contains_id) kombinasyonu için birden fazla fare_id
    {
        type FareRuleKey<'a> = (Option<&'a str>, Option<&'a str>, Option<&'a str>, Option<&'a str>);
        type FareRuleByKey<'a> = HashMap<FareRuleKey<'a>, &'a str>;
        let mut rule_key_to_fare: FareRuleByKey<'_> = HashMap::new();
        for rec in &records.fare_rules {
            if rec.fare_id.is_empty() {
                continue;
            }
            let key = (
                rec.route_id.as_deref(),
                rec.origin_id.as_deref(),
                rec.destination_id.as_deref(),
                rec.contains_id.as_deref(),
            );
            if let Some(&prev_fare) = rule_key_to_fare.get(&key) {
                if prev_fare != rec.fare_id.as_str() {
                    notices.push(notice(
                        ctr,
                        "FAR_010",
                        EntityType::Fare,
                        Some(rec.fare_id.clone()),
                        Some(rec.fare_id.clone()),
                        "fare_rules.txt",
                        Some(rec.line),
                        Some("fare_id"),
                        Some(rec.fare_id.clone()),
                        Some(prev_fare.to_string()),
                        format!(
                            "Ücret tarifesi '{}' için tanımlanan kural, '{}' ile çakışıyor — aynı koşul kümesine birden fazla tarife uygulanıyor.",
                            rec.fare_id, prev_fare
                        ),
                        "Her (route_id, origin_id, destination_id, contains_id) kombinasyonu için yalnızca bir ücret tarifesi tanımlayın.",
                    ));
                }
            } else {
                rule_key_to_fare.insert(key, rec.fare_id.as_str());
            }
        }
    }
}


// -- Fares v2 cross-reference

fn check_fares_v2(
    records: &EntityRecords,
    map: &EntityMap,
    notices: &mut Vec<Notice>,
    ctr: &mut u32,
) {
    // FPD_004/005: fare_products foreign keys
    for rec in &records.fare_products {
        if let Some(ref fmid) = rec.fare_media_id {
            if !map.fare_media_ids.contains_key(fmid.as_str()) {
                notices.push(notice(
                    ctr, "FPD_004", EntityType::Row,
                    Some(rec.fare_product_id.clone()), Some(rec.fare_product_id.clone()),
                    "fare_products.txt", Some(rec.line), Some("fare_media_id"),
                    Some(fmid.clone()), None,
                    format!("'{}' odeme araci fare_media.txt te tanimli degil.", fmid),
                    "Gecerli bir fare_media_id kullanin.",
                ));
            }
        }
        if let Some(ref rcid) = rec.rider_category_id {
            if !map.rider_category_ids.contains(rcid.as_str()) {
                notices.push(notice(
                    ctr, "FPD_005", EntityType::Row,
                    Some(rec.fare_product_id.clone()), Some(rec.fare_product_id.clone()),
                    "fare_products.txt", Some(rec.line), Some("rider_category_id"),
                    Some(rcid.clone()), None,
                    format!("'{}' yolcu kategorisi rider_categories.txt te tanimli degil.", rcid),
                    "Gecerli bir rider_category_id kullanin.",
                ));
            }
        }
    }

    // RCT_006: bir fare_product için birden fazla uygun rider_category varsa tam olarak
    // birinin is_default_fare_category=1 olması gerekir. Boş rider_category_id, ürünün
    // tüm tanımlı kategorilere uygun olduğu anlamına gelir; FPD_005'in sahip olduğu
    // bilinmeyen açık ID'leri uygunluk kümesine katmayız.
    {
        let rider_category_ids: HashSet<&str> = records
            .rider_categories
            .iter()
            .map(|rc| rc.rider_category_id.as_str())
            .filter(|id| !id.is_empty())
            .collect();
        let default_rcids: HashSet<&str> = records
            .rider_categories
            .iter()
            .filter(|rc| rc.is_default_fare_category == Some(1))
            .map(|rc| rc.rider_category_id.as_str())
            .collect();

        let mut fp_rows: HashMap<&str, Vec<u64>> = HashMap::new();
        let mut fp_eligible: HashMap<&str, HashSet<&str>> = HashMap::new();
        for fp in &records.fare_products {
            if fp.fare_product_id.is_empty() {
                continue;
            }
            fp_rows.entry(fp.fare_product_id.as_str()).or_default().push(fp.line);
            let eligible = fp_eligible.entry(fp.fare_product_id.as_str()).or_default();
            match fp.rider_category_id.as_deref() {
                Some(rcid) if rider_category_ids.contains(rcid) => {
                    eligible.insert(rcid);
                }
                Some(_) => {} // FPD_005 reports the unknown explicit category.
                None => eligible.extend(rider_category_ids.iter().copied()),
            }
        }

        // Sıra HashMap'ten gelmemeli: hem determinizm hem de örnek listesi buna bağlı.
        let mut violations: Vec<(&str, HashSet<&str>, usize)> = Vec::new();
        for (fpid, eligible) in fp_eligible {
            if eligible.len() <= 1 {
                continue;
            }
            let default_count = eligible.intersection(&default_rcids).count();
            if default_count == 1 {
                continue;
            }
            violations.push((fpid, eligible, default_count));
        }
        violations.sort_unstable_by(|a, b| a.0.cmp(b.0));

        // Feed'de HİÇ varsayılan kategori yoksa kök kusur TEKTİR (rider_categories.txt'te
        // hiçbir satırda is_default_fare_category=1 yok) ve ürün başına emit aynı cümleyi
        // N kez tekrarlar — mdb-13'te 9 kez. "Varsayılan VAR ama bu ürünün uygunluk
        // kümesinde yok" durumu ise ürüne özgüdür ve toplulanmaz.
        if default_rcids.is_empty() && violations.len() > 1 {
            // Tek bir kategoriyi varsayılan yapmak ancak o kategori HER ihlalli ürünün
            // uygunluk kümesindeyse yeter; yani çözücü adaylar kümelerin KESİŞİMİdir.
            // mdb-13 ölçümü: sezgisel görünen `adult` 4 bulguyu açık bırakıyor, çözenler
            // yalnız {disabled, medicare, senior}. Bunu söylemezsek üretici yanlış
            // kategoriyi seçip geri dönüyor.
            let mut resolving: Option<HashSet<&str>> = None;
            for (_, eligible, _) in &violations {
                resolving = Some(match resolving {
                    None => eligible.clone(),
                    Some(acc) => acc.intersection(eligible).copied().collect(),
                });
            }
            let mut resolving: Vec<&str> =
                resolving.unwrap_or_default().into_iter().collect();
            resolving.sort_unstable();

            let n = violations.len();
            let message = if resolving.is_empty() {
                format!(
                    "Feed'de hicbir rider_category varsayilan degil: {n} ucret urununun her birinde tam olarak 1 varsayilan kategori olmali. Tek bir kategoriyi varsayilan yapmak yetmez; urunlerin uygunluk kumeleri ortusmuyor."
                )
            } else {
                format!(
                    "Feed'de hicbir rider_category varsayilan degil: {} ucret urununun her birinde tam olarak 1 varsayilan kategori olmali. Su kategorilerden BIRINI is_default_fare_category=1 yapmak bulgularin tamamini kapatir: {}.",
                    n,
                    resolving.join(", ")
                )
            };
            let mut agg = notice(
                ctr, "RCT_006", EntityType::Feed,
                None, None,
                "rider_categories.txt", None, Some("is_default_fare_category"),
                Some(n.to_string()), None,
                message,
                "Bir fare_product_id icin uygun rider_category'lerden yalnizca biri is_default_fare_category=1 olmali.",
            );
            let mut details: std::collections::BTreeMap<String, String> =
                std::collections::BTreeMap::new();
            details.insert("affected_products".to_string(), n.to_string());
            details.insert(
                "example_products".to_string(),
                violations.iter().take(5).map(|v| v.0).collect::<Vec<_>>().join(", "),
            );
            if !resolving.is_empty() {
                details.insert("resolving_categories".to_string(), resolving.join(", "));
            }
            agg.details = Some(details);
            notices.push(agg);
        } else {
            for (fpid, eligible, default_count) in violations {
                let lines = fp_rows.get(fpid).expect("fare product rows are indexed together");
                let message = if default_count == 0 {
                    format!(
                        "'{}' ucret urununde {} uygun yolcu kategorisi var ancak hicbiri varsayilan degil; tam olarak 1 olmali.",
                        fpid, eligible.len()
                    )
                } else {
                    format!(
                        "'{}' ucret urunu {} varsayilan yolcu kategorisine bagli; en fazla 1 olmali.",
                        fpid, default_count
                    )
                };
                notices.push(notice(
                    ctr, "RCT_006", EntityType::Row,
                    Some(fpid.to_string()), Some(fpid.to_string()),
                    "fare_products.txt", lines.first().copied(), None,
                    Some(default_count.to_string()), Some("1".to_string()),
                    message,
                    "Bir fare_product_id icin uygun rider_category'lerden yalnizca biri is_default_fare_category=1 olmali.",
                ));
            }
        }
    }

    // FLG_001-006: fare_leg_rules cross-ref
    for rec in &records.fare_leg_rules {
        // FLG_008: `fare_product_id` BOŞ. Aşağıdaki FK kontrolü `!is_empty()` koruması
        // taşır — doğru, çünkü boş bir değer "bulunamadı" değildir. Ama o koruma boş
        // vakayı kimseye devretmiyordu: yedi FLG kuralının hepsi FK ya da enum, hiçbiri
        // varlık ölçmüyor. Spec'te alan Required (#153, korpusta 16 feed).
        if rec.fare_product_id.trim().is_empty() {
            let eid = rec.leg_group_id.clone();
            notices.push(notice(
                ctr, "FLG_008", EntityType::Row, eid.clone(), eid.clone(),
                "fare_leg_rules.txt", Some(rec.line), Some("fare_product_id"),
                Some(String::new()), None,
                "fare_product_id zorunludur.".to_string(),
                "fare_leg_rules.txt'teki her satıra fare_products.txt'te tanımlı bir fare_product_id yazın.",
            ));
        }
        if !rec.fare_product_id.is_empty() && !map.fare_product_ids.contains_key(rec.fare_product_id.as_str()) {
            let eid = rec.leg_group_id.clone();
            notices.push(notice(
                ctr, "FLG_001", EntityType::Row, eid.clone(), eid.clone(),
                "fare_leg_rules.txt", Some(rec.line), Some("fare_product_id"),
                Some(rec.fare_product_id.clone()), None,
                format!("'{}' ucret urunu fare_products.txt te tanimli degil.", rec.fare_product_id),
                "Gecerli bir fare_product_id kullanin.",
            ));
        }
        if let Some(ref nid) = rec.network_id {
            if !map.network_ids.contains(nid.as_str()) {
                let eid = rec.leg_group_id.clone();
                notices.push(notice(
                    ctr, "FLG_002", EntityType::Row, eid.clone(), eid.clone(),
                    "fare_leg_rules.txt", Some(rec.line), Some("network_id"),
                    Some(nid.clone()), None,
                    format!("'{}' ag kodu networks.txt/routes.txt/route_networks.txt te tanimli degil.", nid),
                    "Gecerli bir network_id kullanin (networks.txt, routes.txt veya route_networks.txt).",
                ));
            }
        }
        if let Some(ref aid) = rec.from_area_id {
            if !map.areas.contains_key(aid.as_str()) {
                let eid = rec.leg_group_id.clone();
                notices.push(notice(
                    ctr, "FLG_003", EntityType::Row, eid.clone(), eid.clone(),
                    "fare_leg_rules.txt", Some(rec.line), Some("from_area_id"),
                    Some(aid.clone()), None,
                    format!("'{}' alan kodu areas.txt te tanimli degil.", aid),
                    "Gecerli bir from_area_id kullanin.",
                ));
            }
        }
        if let Some(ref aid) = rec.to_area_id {
            if !map.areas.contains_key(aid.as_str()) {
                let eid = rec.leg_group_id.clone();
                notices.push(notice(
                    ctr, "FLG_004", EntityType::Row, eid.clone(), eid.clone(),
                    "fare_leg_rules.txt", Some(rec.line), Some("to_area_id"),
                    Some(aid.clone()), None,
                    format!("'{}' alan kodu areas.txt te tanimli degil.", aid),
                    "Gecerli bir to_area_id kullanin.",
                ));
            }
        }
        if let Some(ref tfid) = rec.from_timeframe_group_id {
            if !map.timeframe_group_ids.contains(tfid.as_str()) {
                let eid = rec.leg_group_id.clone();
                notices.push(notice(
                    ctr, "FLG_005", EntityType::Row, eid.clone(), eid.clone(),
                    "fare_leg_rules.txt", Some(rec.line), Some("from_timeframe_group_id"),
                    Some(tfid.clone()), None,
                    format!("'{}' zaman dilimi timeframes.txt te tanimli degil.", tfid),
                    "Gecerli bir from_timeframe_group_id kullanin.",
                ));
            }
        }
        if let Some(ref tfid) = rec.to_timeframe_group_id {
            if !map.timeframe_group_ids.contains(tfid.as_str()) {
                let eid = rec.leg_group_id.clone();
                notices.push(notice(
                    ctr, "FLG_006", EntityType::Row, eid.clone(), eid.clone(),
                    "fare_leg_rules.txt", Some(rec.line), Some("to_timeframe_group_id"),
                    Some(tfid.clone()), None,
                    format!("'{}' zaman dilimi timeframes.txt te tanimli degil.", tfid),
                    "Gecerli bir to_timeframe_group_id kullanin.",
                ));
            }
        }
    }

    // FTR_002-004: fare_transfer_rules cross-ref
    for rec in &records.fare_transfer_rules {
        if let Some(ref lgid) = rec.from_leg_group_id {
            if !map.leg_group_ids.contains(lgid.as_str()) {
                notices.push(notice(
                    ctr, "FTR_002", EntityType::Row,
                    Some(lgid.clone()), Some(lgid.clone()),
                    "fare_transfer_rules.txt", Some(rec.line), Some("from_leg_group_id"),
                    Some(lgid.clone()), None,
                    format!("'{}' bacak grubu fare_leg_rules.txt te tanimli degil.", lgid),
                    "Gecerli bir from_leg_group_id kullanin.",
                ));
            }
        }
        if let Some(ref lgid) = rec.to_leg_group_id {
            if !map.leg_group_ids.contains(lgid.as_str()) {
                notices.push(notice(
                    ctr, "FTR_003", EntityType::Row,
                    Some(lgid.clone()), Some(lgid.clone()),
                    "fare_transfer_rules.txt", Some(rec.line), Some("to_leg_group_id"),
                    Some(lgid.clone()), None,
                    format!("'{}' bacak grubu fare_leg_rules.txt te tanimli degil.", lgid),
                    "Gecerli bir to_leg_group_id kullanin.",
                ));
            }
        }
        if let Some(ref fpid) = rec.fare_product_id {
            if !map.fare_product_ids.contains_key(fpid.as_str()) {
                notices.push(notice(
                    ctr, "FTR_004", EntityType::Row,
                    rec.from_leg_group_id.clone(), rec.from_leg_group_id.clone(),
                    "fare_transfer_rules.txt", Some(rec.line), Some("fare_product_id"),
                    Some(fpid.clone()), None,
                    format!("'{}' ucret urunu fare_products.txt te tanimli degil.", fpid),
                    "Gecerli bir fare_product_id kullanin.",
                ));
            }
        }
    }

    // SAR_001-002: stop_areas cross-ref
    for rec in &records.stop_areas {
        if !rec.area_id.is_empty() && !map.areas.contains_key(rec.area_id.as_str()) {
            notices.push(notice(
                ctr, "SAR_001", EntityType::Row,
                Some(rec.area_id.clone()), Some(rec.area_id.clone()),
                "stop_areas.txt", Some(rec.line), Some("area_id"),
                Some(rec.area_id.clone()), None,
                format!("'{}' alan kodu areas.txt te tanimli degil.", rec.area_id),
                "Gecerli bir area_id kullanin.",
            ));
        }
        if !rec.stop_id.is_empty() && !map.stops.contains_key(rec.stop_id.as_str()) {
            notices.push(notice(
                ctr, "SAR_002", EntityType::Row,
                Some(rec.stop_id.clone()), Some(rec.stop_id.clone()),
                "stop_areas.txt", Some(rec.line), Some("stop_id"),
                Some(rec.stop_id.clone()), None,
                format!("'{}' durak stops.txt te tanimli degil.", rec.stop_id),
                "Gecerli bir stop_id kullanin.",
            ));
        }
    }

    // XFL_022-023: location_group_stops cross-ref (GTFS-Flex)
    for rec in &records.location_group_stops {
        if !rec.location_group_id.is_empty()
            && !map.location_group_ids.contains(rec.location_group_id.as_str())
        {
            notices.push(notice(
                ctr, "XFL_022", EntityType::Row,
                Some(rec.location_group_id.clone()), Some(rec.location_group_id.clone()),
                "location_group_stops.txt", Some(rec.line), Some("location_group_id"),
                Some(rec.location_group_id.clone()), None,
                format!("'{}' konum grubu location_groups.txt'te tanimli degil.", rec.location_group_id),
                "Gecerli bir location_group_id kullanin veya grubu location_groups.txt'te tanimlayin.",
            ));
        }
        if !rec.stop_id.is_empty() && !map.stops.contains_key(rec.stop_id.as_str()) {
            notices.push(notice(
                ctr, "XFL_023", EntityType::Row,
                Some(rec.stop_id.clone()), Some(rec.stop_id.clone()),
                "location_group_stops.txt", Some(rec.line), Some("stop_id"),
                Some(rec.stop_id.clone()), None,
                format!("'{}' durak stops.txt'te tanimli degil.", rec.stop_id),
                "Gecerli bir stop_id kullanin.",
            ));
        }
    }

    // TFR_002: timeframes.service_id cross-ref
    for rec in &records.timeframes {
        if !rec.service_id.is_empty() && !map.services.contains(rec.service_id.as_str()) {
            notices.push(notice(
                ctr, "TFR_002", EntityType::Row,
                Some(rec.timeframe_group_id.clone()), Some(rec.timeframe_group_id.clone()),
                "timeframes.txt", Some(rec.line), Some("service_id"),
                Some(rec.service_id.clone()), None,
                format!("'{}' servis takvimi tanimli degil.", rec.service_id),
                "Gecerli bir service_id kullanin.",
            ));
        }
    }
}

// �"?�"? LVL_004: kullanılmayan level �"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?

fn check_levels(
    records: &EntityRecords,
    _map: &EntityMap,
    notices: &mut Vec<Notice>,
    ctr: &mut u32,
) {
    let used_levels: HashSet<&str> = records
        .stops
        .iter()
        .filter_map(|s| s.level_id.as_deref())
        .collect();

    for rec in &records.levels {
        if rec.level_id.is_empty() {
            continue;
        }
        if !used_levels.contains(rec.level_id.as_str()) {
            notices.push(notice(
                ctr,
                "LVL_004",
                EntityType::Level,
                Some(rec.level_id.clone()),
                Some(rec.level_id.clone()),
                "levels.txt",
                Some(rec.line),
                Some("level_id"),
                Some(rec.level_id.clone()),
                None,
                format!("level_id '{}' hiçbir durak tarafından referans edilmiyor.", rec.level_id),
                "Kullanılmayan level'ı silin veya bir durağın level_id'sini bu değere ayarlayın.",
            ));
        }
    }
}

// �"?�"? TRN_005-006: çeviri cross-ref �"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?

/// `attributions.txt`'nin birincil anahtarı; `EntityMap` bu tabloyu taşımaz.
fn attribution_id_set(records: &EntityRecords) -> HashSet<&str> {
    records.attributions.iter()
        .filter_map(|a| a.attribution_id.as_deref())
        .filter(|id| !id.is_empty())
        .collect()
}

/// Bir çeviri satırının `record_id`'si kaynak tabloda var mı?
///
/// `None` = tablonun kimliği bu aşamada çözülemez → hiçbir kural iddia üretmemelidir.
/// TRN_004 ve XFL_014 bu eşlemenin iki ayrı kopyasını taşıyordu; `attributions` ikisinde
/// de "lookup not feasible" sayılıyor, yani `attribution_id` elimizdeyken denetlenmiyordu
/// (2026-08-29, Katori feed'i). Kopyayı tekilleştirmek aynı sapmanın tekrarını önler.
/// JPN_019 ayrı bir eşleme kullanır: o blok `EntityMap` yerine ham K2 kayıtlarını okur.
fn translation_record_exists(
    map: &EntityMap,
    attribution_ids: &HashSet<&str>,
    table: &str,
    record_id: &str,
) -> Option<bool> {
    Some(match table {
        "agency" => map.agencies.contains_key(record_id),
        "stops" => map.stops.contains_key(record_id),
        "routes" => map.routes.contains_key(record_id),
        "trips" => map.trips.contains_key(record_id),
        "calendar" | "calendar_dates" => map.services.contains(record_id),
        "levels" => map.levels.contains_key(record_id),
        "pathways" => map.pathways.contains_key(record_id),
        "fare_attributes" => map.fare_attrs.contains_key(record_id),
        "attributions" => attribution_ids.contains(record_id),
        // feed_info/stop_times/shapes/frequencies/transfers/fare_rules/translations:
        // kimlik ya yasaktır (feed_info → TRN_013) ya da tek alanla çözülemez.
        _ => return None,
    })
}

fn check_translations(
    records: &EntityRecords,
    map: &EntityMap,
    notices: &mut Vec<Notice>,
    ctr: &mut u32,
) {
    let feed_lang = records.feed_info.first().map(|fi| fi.feed_lang.as_str()).unwrap_or("");
    // key → ilk görülen translation değeri. Aynı key tekrar görülürse:
    //   değer aynı  → TRN_005 (birebir yinelenen çeviri)
    //   değer farklı → TRN_006 (çelişkili çeviri; hangisi geçerli belirsiz)
    let mut seen: HashMap<String, String> = HashMap::new();
    let attribution_ids = attribution_id_set(records);
    // TRN_007 agregasyonu: feed_lang ile aynı dildeki çeviriler (yaygın GTFS-JP araç pratiği —
    // her alanı ja/ja-Hrkt/en'e çevirir, ja kaynağı birebir tekrarlar) satır-başına on binlerce
    // notice yerine tek feed-seviyesi özette toplanır (STM_017/STP_022 emsali).
    let mut trn007_pending: Vec<Notice> = Vec::new();

    for rec in &records.translations {
        // TRN_004: record_id başvurulan kayıt bulunamadı
        if let Some(ref rid) = rec.record_id {
            let exists = translation_record_exists(
                map, &attribution_ids, &rec.table_name, rid.as_str(),
            ).unwrap_or(true);
            if !exists {
                notices.push(notice(
                    ctr,
                    "TRN_004",
                    EntityType::Translation,
                    // `entity_id` boş bırakılıyordu, oysa en/ja/fr mesaj şablonları
                    // `{entity_id}` ile doldurulur → İngilizce metin "record_id '' not
                    // found." çıkıyordu. Türkçe metin koddaki `format!`ten geldiği için
                    // kusur yalnız diğer dillerde görünüyordu. `scope_key` bilinçli olarak
                    // boş kalıyor: kapsam birimi satırdır, record_id değil.
                    Some(rid.clone()),
                    None,
                    "translations.txt",
                    Some(rec.line),
                    Some("record_id"),
                    Some(rid.clone()),
                    None,
                    format!(
                        "record_id '{rid}' table_name='{}' tablosunda bulunamadı.",
                        rec.table_name
                    ),
                    "Mevcut bir kaydın ID'sini kullanın ya da satırı kaldırın.",
                ));
            }
        }

        // TRN_007: çeviri dili feed_lang ile aynı — gereksiz çeviri (döngü sonunda agregasyon)
        if !feed_lang.is_empty() && rec.language == feed_lang {
            trn007_pending.push(notice(
                ctr,
                "TRN_007",
                EntityType::Translation,
                rec.record_id.clone(),
                rec.record_id.clone(),
                "translations.txt",
                Some(rec.line),
                Some("language"),
                Some(rec.language.clone()),
                Some(format!("≠ {feed_lang}")),
                format!(
                    "Çeviri dili '{}' feed_lang '{}' ile aynı — orijinal dilde çeviri gereksiz.",
                    rec.language, feed_lang
                ),
                "Çevirileri yalnızca feed_lang'dan farklı diller için ekleyin.",
            ));
        }

        // TRN_005/006 need a real translation identity. If a required header
        // is absent, K2 represents it as an empty/None value; indexing those
        // rows would turn every legacy-format row into a duplicate/conflict
        // against the same synthetic key. feed_info is the exception because
        // its table+field+language tuple is its complete identity.
        let has_translation_identity = !rec.table_name.is_empty()
            && !rec.field_name.is_empty()
            && !rec.language.is_empty()
            && (rec.table_name == "feed_info"
                || rec.record_id.is_some()
                || rec.field_value.is_some());
        if !has_translation_identity {
            continue;
        }

        // TRN_006: table+field+language+record kombinasyonu tekil.
        // GTFS spec'i bir çeviri satırını (record_id, record_sub_id) VEYA field_value
        // ile tanımlar; field_value anahtara dahil EDİLMELİ. Aksi hâlde field_value
        // bazlı çevirilerde (örn. her stop_headsign değeri için ayrı satır) record_id
        // boş kalıp anahtar çakışır ve her farklı değer sahte TRN_006 üretir.
        let key = format!(
            "{}|{}|{}|{}|{}|{}",
            rec.table_name,
            rec.field_name,
            rec.language,
            rec.record_id.as_deref().unwrap_or(""),
            rec.record_sub_id.as_deref().unwrap_or(""),
            rec.field_value.as_deref().unwrap_or(""),
        );
        match seen.get(&key) {
            None => {
                seen.insert(key, rec.translation.clone());
            }
            Some(prev) if *prev == rec.translation => {
                // TRN_005: aynı anahtar + aynı çeviri değeri → birebir yinelenen satır
                notices.push(notice(
                    ctr,
                    "TRN_005",
                    EntityType::Translation,
                    None,
                    None,
                    "translations.txt",
                    Some(rec.line),
                    // Yinelemeyi tanımlayan bileşik anahtarın tamamı.
                    Some("table_name|field_name|language|record_id|record_sub_id|field_value"),
                    Some(format!("{}/{}/{}", rec.table_name, rec.field_name, rec.language)),
                    None,
                    format!(
                        "table_name='{}', field_name='{}', language='{}' için aynı çeviri değeri ('{}') birden çok satırda tekrarlanıyor.",
                        rec.table_name, rec.field_name, rec.language, rec.translation
                    ),
                    "Yinelenen çeviri satırını kaldırın.",
                ));
            }
            Some(_) => {
                // TRN_006: aynı anahtar + farklı çeviri değeri → çelişki
                notices.push(notice(
                    ctr,
                    "TRN_006",
                    EntityType::Translation,
                    None,
                    None,
                    "translations.txt",
                    Some(rec.line),
                    // Çelişkiyi tanımlayan bileşik anahtarın tamamı (TRN_005 ile aynı).
                    Some("table_name|field_name|language|record_id|record_sub_id|field_value"),
                    Some(format!("{}/{}/{}", rec.table_name, rec.field_name, rec.language)),
                    None,
                    format!(
                        "table_name='{}', field_name='{}', language='{}' kombinasyonu farklı çeviri değerleriyle tekrarlanıyor; hangisinin geçerli olduğu belirsiz.",
                        rec.table_name, rec.field_name, rec.language
                    ),
                    "Aynı kayıt için aynı dilde yalnızca bir çeviri tanımlayın.",
                ));
            }
        }
    }

    // TRN_007 karar: birden çok satır aynı dildeyse (sistemik) tek feed-seviyesi özet;
    // tek satır ise satır-başına korunur.
    if trn007_pending.len() > 1 {
        let n = trn007_pending.len();
        notices.push(notice(
            ctr,
            "TRN_007",
            EntityType::Feed,
            None,
            None,
            "translations.txt",
            None,
            Some("language"),
            Some(n.to_string()),
            Some(format!("≠ {feed_lang}")),
            format!("Feed genelinde {n} çeviri satırı feed_lang ('{feed_lang}') ile aynı dilde — orijinal dilde çeviri gereksiz."),
            "feed_lang ile aynı dildeki çeviri satırlarını translations.txt'ten kaldırın.",
        ));
    } else {
        notices.append(&mut trn007_pending);
    }
}

// �"?�"? ATR_009-012: attribution cross-ref �"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?

/// translations.txt'ten belirli (table, field) için ja-Hrkt (かな) çevirisi olan
/// record_id ve field_value kümeleri. JPN_001/008/009/010 kana kontrollerinde paylaşılır.
/// String Japonca karakter (Hiragana/Katakana/Kanji) içeriyor mu — char/scalar bazlı (byte DEĞİL).
/// JPN_008'in route_short_name'e düşmesinde sayısal kodları (ör. "42") elemek için.
fn has_japanese(s: &str) -> bool {
    s.chars().any(|c| matches!(c,
        '\u{3040}'..='\u{309F}'   // Hiragana
        | '\u{30A0}'..='\u{30FF}' // Katakana
        | '\u{4E00}'..='\u{9FFF}' // CJK Unified Ideographs (Kanji)
    ))
}

fn collect_kana<'a>(
    translations: &'a [crate::k2::translations::TranslationRecord],
    table: &str,
    field: &str,
) -> (HashSet<&'a str>, HashSet<&'a str>) {
    let mut recs: HashSet<&str> = HashSet::new();
    let mut vals: HashSet<&str> = HashSet::new();
    for t in translations {
        if t.table_name == table && t.field_name == field && t.language.eq_ignore_ascii_case("ja-Hrkt") {
            if let Some(rid) = t.record_id.as_deref() { recs.insert(rid); }
            if let Some(fv) = t.field_value.as_deref() { vals.insert(fv); }
        }
    }
    (recs, vals)
}

fn valid_gtfs_jp_date(raw: &str) -> bool {
    if raw.len() != 8 || !raw.chars().all(|c| c.is_ascii_digit()) { return false; }
    let year = match raw[0..4].parse::<u32>() { Ok(v) => v, Err(_) => return false };
    let month = match raw[4..6].parse::<u32>() { Ok(v) => v, Err(_) => return false };
    let day = match raw[6..8].parse::<u32>() { Ok(v) => v, Err(_) => return false };
    crate::k2::common::is_valid_calendar_date(year, month, day)
}

// ── JPN_001: GTFS-JP feed'inde durak adının kana (ja-Hrkt) okuması eksik ──────
// GTFS-JP, stop_name için かな okumasını (translations, language=ja-Hrkt) ZORUNLU kılar
// (sesli anons + arama için). Yalnız Japon feed'inde çalışır: feed_lang=ja VEYA herhangi
// bir ja-Hrkt çeviri varsa. Bir durağın kanası var sayılır ⇔ translations'ta
// (table=stops, field=stop_name, language=ja-Hrkt) record_id=stop_id VEYA
// field_value=stop_name ile eşleşen satır bulunur.
fn check_gtfs_jp(
    records: &EntityRecords,
    notices: &mut Vec<Notice>,
    ctr: &mut u32,
    availability: &FileAvailability<'_>,
) {
    let ti_jpn = &records.trip_interns;
    // Gerçek pipeline'da K1 envanteri boş/bozuk dosyaları da fiziksel varlık olarak taşır.
    // Doğrudan K4 çağrılarında ise complete() sentetik kayıtları desteklemek için mevcut
    // typed kayıtlar ve pattern işareti geri dönüş ölçütüdür.
    let has_jp_file = records.has_gtfs_jp_file || if availability.has_inventory() {
        GTFS_JP_FILES.iter().any(|name| availability.present(name))
    } else {
        !records.agency_jp.is_empty()
            || !records.office_jp.is_empty()
            || !records.routes_jp.is_empty()
            || records.has_pattern_jp_file
    };
    let has_pattern_jp_file = records.has_pattern_jp_file || if availability.has_inventory() {
        availability.present("pattern_jp.txt")
    } else {
        false
    };
    // ── JPN_001: stop_name kana (ja-Hrkt) okuması — kapı: feed_lang ja* VEYA ja-Hrkt çeviri ──
    let feed_lang_ja = records.feed_info.first()
        .map(|fi| fi.feed_lang.to_lowercase().starts_with("ja"))
        .unwrap_or(false);
    let has_kana = records.translations.iter()
        .any(|t| t.language.eq_ignore_ascii_case("ja-Hrkt"));
    let is_gtfs_jp = records.is_gtfs_jp.unwrap_or({
        // Compatibility path for direct/synthetic K4 callers that predate the
        // K2 signal field. The real K1→K2→K4 pipeline always carries Some(...).
        feed_lang_ja || has_kana || has_jp_file
    });
    // GTFS-JP v4 keeps the Japanese translation and core GTFS checks, but
    // moves agency_jp/office_jp/pattern_jp out of the main standard. The
    // profile is explicit; Auto preserves the pre-v4 behavior.
    let legacy_v3_extensions = !matches!(records.gtfs_jp_profile, GtfsJpProfile::V4);
    if is_gtfs_jp {
        let mut kana_records: HashSet<&str> = HashSet::new();
        let mut kana_values: HashSet<&str> = HashSet::new();
        for t in &records.translations {
            if t.table_name == "stops"
                && t.field_name == "stop_name"
                && t.language.eq_ignore_ascii_case("ja-Hrkt")
            {
                if let Some(rid) = t.record_id.as_deref() { kana_records.insert(rid); }
                if let Some(fv) = t.field_value.as_deref() { kana_values.insert(fv); }
            }
        }

        for stop in &records.stops {
            // Sadece fiziksel duraklar (location_type 0/boş); istasyon/giriş/node hariç.
            if stop.location_type.unwrap_or(0) != 0 {
                continue;
            }
            let name = match stop.stop_name.as_deref() {
                Some(n) if !n.is_empty() => n,
                _ => continue,
            };
            if kana_records.contains(stop.stop_id.as_str()) || kana_values.contains(name) {
                continue;
            }
            notices.push(notice(
                ctr,
                "JPN_001",
                EntityType::Stop,
                Some(stop.stop_id.clone()),
                Some(stop.stop_id.clone()),
                "translations.txt",
                Some(stop.line),
                Some("stop_name"),
                None,
                Some("ja-Hrkt".to_string()),
                format!("'{}' durağının adı ('{}') için kana (ja-Hrkt) okuması eksik — GTFS-JP'de zorunlu.",
                    stop.stop_id, name),
                "translations.txt'e bu durak için language=ja-Hrkt (かな) çevirisi ekleyin.",
            ));
        }

        // ── JPN_008: route adı kana (ja-Hrkt) — GTFS-JP _name alanları zorunlu ──
        // route_long_name varsa onu; yoksa (Tokyo gibi adı route_short_name'de tutan feed'lerde)
        // Japonca karakter içeren route_short_name'i denetle (sayısal kodlar elenir).
        let (krl_rec, krl_val) = collect_kana(&records.translations, "routes", "route_long_name");
        let (krs_rec, krs_val) = collect_kana(&records.translations, "routes", "route_short_name");
        for route in &records.routes {
            let (field, name, kr_rec, kr_val) = match route.route_long_name.as_deref() {
                Some(n) if !n.is_empty() => ("route_long_name", n, &krl_rec, &krl_val),
                _ => match route.route_short_name.as_deref() {
                    Some(s) if !s.is_empty() && has_japanese(s) => ("route_short_name", s, &krs_rec, &krs_val),
                    _ => continue,
                },
            };
            if kr_rec.contains(route.route_id.as_str()) || kr_val.contains(name) {
                continue;
            }
            notices.push(notice(
                ctr, "JPN_008", EntityType::Route,
                Some(route.route_id.clone()), Some(route.route_id.clone()),
                "translations.txt", Some(route.line), Some(field),
                None, Some("ja-Hrkt".to_string()),
                format!("'{}' hattının adı ('{}') için kana (ja-Hrkt) okuması eksik — GTFS-JP'de zorunlu.", route.route_id, name),
                "translations.txt'e bu hat için language=ja-Hrkt (route adı) çevirisi ekleyin.",
            ));
        }

        // ── JPN_009: trip_headsign kana (ja-Hrkt) — GTFS-JP _headsign alanları zorunlu ──
        let (kt_rec, kt_val) = collect_kana(&records.translations, "trips", "trip_headsign");
        for trip in &records.trips {
            let hs = match ti_jpn.headsign(trip) {
                Some(h) if !h.is_empty() => h,
                _ => continue,
            };
            if kt_rec.contains(trip.trip_id.as_str()) || kt_val.contains(hs) {
                continue;
            }
            notices.push(notice(
                ctr, "JPN_009", EntityType::Trip,
                Some(trip.trip_id.to_string()), Some(trip.trip_id.to_string()),
                "translations.txt", Some(trip.line), Some("trip_headsign"),
                None, Some("ja-Hrkt".to_string()),
                format!("'{}' seferinin trip_headsign'ı ('{}') için kana (ja-Hrkt) okuması eksik — GTFS-JP'de zorunlu.", trip.trip_id, hs),
                "translations.txt'e bu sefer için language=ja-Hrkt (trip_headsign) çevirisi ekleyin.",
            ));
        }

        // ── JPN_010: agency_name kana (ja-Hrkt) — GTFS-JP _name alanları zorunlu ──
        let (ka_rec, ka_val) = collect_kana(&records.translations, "agency", "agency_name");
        for ag in &records.agencies {
            if ag.agency_name.is_empty() {
                continue;
            }
            let aid = ag.agency_id.as_deref().unwrap_or("");
            if (!aid.is_empty() && ka_rec.contains(aid)) || ka_val.contains(ag.agency_name.as_str()) {
                continue;
            }
            let eid = ag.agency_id.clone().filter(|s| !s.is_empty()).unwrap_or_else(|| ag.agency_name.clone());
            notices.push(notice(
                ctr, "JPN_010", EntityType::Agency,
                Some(eid.clone()), Some(eid),
                "translations.txt", Some(ag.line), Some("agency_name"),
                None, Some("ja-Hrkt".to_string()),
                format!("'{}' işleticisinin adı için kana (ja-Hrkt) okuması eksik — GTFS-JP'de zorunlu.", ag.agency_name),
                "translations.txt'e bu işletici için language=ja-Hrkt (agency_name) çevirisi ekleyin.",
            ));
        }
    }

    if legacy_v3_extensions {
    // ── JPN_002: trips.jp_office_id → office_jp.office_id ──
    // Kapı: office_jp.txt mevcut (GTFS-JP yapısal sinyali). Boş/eksik jp_office_id atlanır;
    // sadece dolu ama office_jp'de tanımsız referanslar işaretlenir.
    if !records.office_jp.is_empty() {
        let office_ids: HashSet<&str> = records.office_jp.iter()
            .map(|o| o.office_id.as_str())
            .filter(|id| !id.is_empty())
            .collect();
        for trip in &records.trips {
            let oid = match ti_jpn.jp_office_id(trip) {
                Some(o) if !o.is_empty() => o,
                _ => continue,
            };
            if !office_ids.contains(oid) {
                notices.push(notice(
                    ctr,
                    "JPN_002",
                    EntityType::Trip,
                    Some(trip.trip_id.to_string()),
                    Some(trip.trip_id.to_string()),
                    "trips.txt",
                    Some(trip.line),
                    Some("jp_office_id"),
                    Some(oid.to_string()),
                    None,
                    format!("'{}' seferindeki jp_office_id ('{}') office_jp.txt'te tanımlı değil.",
                        trip.trip_id, oid),
                    "jp_office_id değerini office_jp.txt'te tanımlı bir office_id ile eşleştirin.",
                ));
            }
        }
        // JPN_002 (genişletme): routes.jp_office_id → office_jp.office_id.
        // Resmî GTFS-JP spec jp_office_id'yi routes.txt'te tanımlar (Tokyo trips.txt'te koymuş);
        // sürüm-toleranslı olmak için her iki konumu da denetleriz.
        for route in &records.routes {
            let oid = match route.jp_office_id.as_deref() {
                Some(o) if !o.is_empty() => o,
                _ => continue,
            };
            if !office_ids.contains(oid) {
                notices.push(notice(
                    ctr,
                    "JPN_002",
                    EntityType::Route,
                    Some(route.route_id.clone()),
                    Some(route.route_id.clone()),
                    "routes.txt",
                    Some(route.line),
                    Some("jp_office_id"),
                    Some(oid.to_string()),
                    None,
                    format!("'{}' hattındaki jp_office_id ('{}') office_jp.txt'te tanımlı değil.",
                        route.route_id, oid),
                    "jp_office_id değerini office_jp.txt'te tanımlı bir office_id ile eşleştirin.",
                ));
            }
        }
    }

    // ── JPN_003: agency_jp.agency_id → agency.agency_id ──
    // Kapı: agency_jp.txt mevcut. Boş agency_id (ayrı zorunluluk ihlali) atlanır.
    if !records.agency_jp.is_empty() {
        let agency_ids: HashSet<&str> = records.agencies.iter()
            .filter_map(|a| a.agency_id.as_deref())
            .filter(|id| !id.is_empty())
            .collect();
        for aj in &records.agency_jp {
            let aid = match aj.agency_id.as_deref() {
                Some(a) if !a.is_empty() => a,
                _ => continue,
            };
            if !agency_ids.contains(aid) {
                notices.push(notice(
                    ctr,
                    "JPN_003",
                    EntityType::Agency,
                    Some(aid.to_string()),
                    Some(aid.to_string()),
                    "agency_jp.txt",
                    Some(aj.line),
                    Some("agency_id"),
                    Some(aid.to_string()),
                    None,
                    format!("agency_jp.txt'teki agency_id ('{}') agency.txt'te tanımlı değil.", aid),
                    "agency_jp.txt'teki agency_id değerini agency.txt'te tanımlı bir agency_id ile eşleştirin.",
                ));
            }
        }
    }

    }

    // ── JPN_004: GTFS-JP feed'inde translations.txt zorunlu ──
    // GTFS-JP profili translations.txt'i (özellikle stop_name kana/ja-Hrkt okumaları için)
    // zorunlu kılar. Kapı: GTFS-JP sinyali (feed_lang ja* VEYA *_jp dosyası) AMA translations hiç yok.
    // has_kana zaten translations dolu demek → bu kuralla çelişmez.
    if is_gtfs_jp && records.translations.is_empty() {
        notices.push(notice(
            ctr,
            "JPN_004",
            EntityType::Feed,
            None,
            None,
            "translations.txt",
            None,
            Some("translations"),
            None,
            None,
            "GTFS-JP feed'inde translations.txt eksik — profil bunu (özellikle stop_name kana/ja-Hrkt okumaları için) zorunlu kılar.".to_string(),
            "translations.txt ekleyin ve en azından stop_name için language=ja-Hrkt (かな) çevirileri sağlayın.",
        ));
    }

    // ── JPN_011: GTFS-JP'de agency_id her zaman zorunlu (tek işletici olsa bile) ──
    // Standart GTFS tek-agency'de agency_id'yi opsiyonel sayar; AGN_011 yalnız ÇOKLU-agency'de
    // (route-level) fire eder → tek-işletici JP feed'inde agency_id eksikliği boşlukta kalıyordu.
    if is_gtfs_jp {
        let missing = records.agencies.iter()
            .filter(|a| a.agency_id.as_deref().is_none_or(|id| id.trim().is_empty()))
            .count();
        if missing > 0 {
            notices.push(notice(
                ctr,
                "JPN_011",
                EntityType::Feed,
                None,
                None,
                "agency.txt",
                None,
                Some("agency_id"),
                Some(format!("{missing}")),
                Some("dolu".to_string()),
                format!("GTFS-JP: {missing} işleticide agency_id eksik — JP profilinde tek işletici olsa bile agency_id zorunludur."),
                "agency.txt'teki her satıra benzersiz bir agency_id girin.",
            ));
        }
    }

    // ── JPN_022: GTFS-JP v4 ana alan zorunluluğu ─────────────────────────────
    // V4, v3'te sabit/opsiyonel olan bazı alanları açıkça zorunlu kılar. Dolu
    // değerlerin biçim ve anlam denetimi genel GTFS kurallarında kalır; bu kural
    // yalnız eksik/boş değerleri, profil seçimi V4 iken görünür kılar.
    if is_gtfs_jp && matches!(records.gtfs_jp_profile, GtfsJpProfile::V4) {
        let mut jpn022_pending = Vec::new();
        for agency in &records.agencies {
            if agency.agency_lang.as_deref().is_none_or(|value| value.trim().is_empty()) {
                jpn022_pending.push(notice(
                    ctr,
                    "JPN_022",
                    EntityType::Agency,
                    agency.agency_id.clone(),
                    agency.agency_id.clone(),
                    "agency.txt",
                    Some(agency.line),
                    Some("agency_lang"),
                    None,
                    None,
                    "GTFS-JP v4'te agency_lang zorunludur ancak değer eksik.".to_string(),
                    "agency.txt'teki agency_lang alanını doldurun.",
                ));
            }
        }

        // V4 makes stops.location_type Required. The base GTFS parser keeps the
        // v3-compatible default (missing/blank means a regular stop), so only a
        // genuinely missing/blank column is reported here; a non-numeric value
        // remains the responsibility of STP_008 and must not be double-reported.
        for stop in &records.stops {
            let raw_location_type = stop.row.get("location_type").map(String::as_str);
            if stop.location_type.is_none()
                && raw_location_type.is_none_or(|value| value.trim().is_empty())
            {
                jpn022_pending.push(notice(
                    ctr,
                    "JPN_022",
                    EntityType::Stop,
                    Some(stop.stop_id.clone()),
                    Some(stop.stop_id.clone()),
                    "stops.txt",
                    Some(stop.line),
                    Some("location_type"),
                    None,
                    None,
                    "GTFS-JP v4'te location_type zorunludur ancak değer eksik.".to_string(),
                    "stops.txt'teki her durak için location_type değerini 0, 1, 2, 3 veya 4 olarak belirtin.",
                ));
            }
        }

        // A missing/blank required column is a file/field defect, not a distinct
        // defect for every affected row. Keep one-row cases pinpointed, but turn
        // repeated findings for the same file+field into one feed summary with
        // the affected-record count. This prevents v4 from producing thousands
        // of indistinguishable location_type notices on ordinary GTFS feeds.
        if let Some(feed_info) = records.feed_info.first() {
            let required_fields = [
                ("feed_start_date", feed_info.feed_start_date.is_none()),
                ("feed_end_date", feed_info.feed_end_date.is_none()),
                ("feed_version", feed_info.feed_version.as_deref().is_none_or(|value| value.trim().is_empty())),
            ];
            for (field, missing) in required_fields {
                if missing {
                    jpn022_pending.push(notice(
                        ctr,
                        "JPN_022",
                        EntityType::Feed,
                        None,
                        None,
                        "feed_info.txt",
                        Some(feed_info.line),
                        Some(field),
                        None,
                        None,
                        format!("GTFS-JP v4'te {field} zorunludur ancak değer eksik."),
                        "feed_info.txt'teki ilgili zorunlu alanı doldurun.",
                    ));
                }
            }
        }

        append_jpn022_missing_records(notices, jpn022_pending);
    }

    if legacy_v3_extensions {
    // ── JPN_005: office_jp.office_name zorunlu (boş olamaz) ──
    // Kapı: office_jp.txt mevcut. office_id'si dolu ama office_name'i boş/eksik satırlar işaretlenir.
    for oj in &records.office_jp {
        if oj.office_id.is_empty() {
            continue;
        }
        let name_empty = oj.office_name.as_deref().map(|s| s.trim().is_empty()).unwrap_or(true);
        if name_empty {
            notices.push(notice(
                ctr,
                "JPN_005",
                EntityType::Agency,
                Some(oj.office_id.to_string()),
                Some(oj.office_id.to_string()),
                "office_jp.txt",
                Some(oj.line),
                Some("office_name"),
                None,
                None,
                format!("office_jp.txt'teki '{}' ofisinin office_name değeri boş — GTFS-JP'de zorunludur.", oj.office_id),
                "office_jp.txt'te her office_id için office_name değerini doldurun.",
            ));
        }
    }

    }

    // ── JPN_006: GTFS-JP ücret kapsamı ───────────────────────────────────────
    // v3'te fare_attributes.txt zorunludur; fare_rules.txt ise tüm hatlar aynı
    // tek tip ücreti kullanmıyorsa koşullu zorunludur. Tek başına eksik
    // fare_rules.txt, uniform ücret ile route/zone bazlı ücret ayrımını
    // gözlemlenebilir kılmaz; bu yüzden yalnızca birden fazla farklı ücret
    // profili kanıtı varsa koşullu eksiklik raporlanır.
    let fare_profiles: HashSet<(String, String, String, String, String)> = records
        .fare_attributes
        .iter()
        .map(|fare| {
            (
                fare.row.get("price").cloned().unwrap_or_default(),
                fare.row.get("currency_type").cloned().unwrap_or_default(),
                fare.row.get("payment_method").cloned().unwrap_or_default(),
                fare.row.get("transfers").cloned().unwrap_or_default(),
                fare.row.get("transfer_duration").cloned().unwrap_or_default(),
            )
        })
        .collect();
    let fare_rules_conditionally_required = fare_profiles.len() > 1;
    let fare_problem = records.fare_attributes.is_empty()
        || (fare_rules_conditionally_required && records.fare_rules.is_empty());
    if is_gtfs_jp && fare_problem {
        notices.push(notice(
            ctr,
            "JPN_006",
            EntityType::Feed,
            None,
            None,
            "fare_attributes.txt",
            None,
            Some("fare_attributes"),
            None,
            None,
            "GTFS-JP feed'inde fare_attributes.txt eksik veya farklı ücret profilleri için fare_rules.txt eksik — v3 ücret kapsamı koşulunu karşılamıyor.".to_string(),
            "fare_attributes.txt'i ekleyin; hatlara/alanlara göre farklı ücret profilleri varsa fare_rules.txt ile bunları eşleyin.",
        ));
    }

    // ── JPN_007: GTFS-JP'de feed_info.txt zorunlu ──
    if is_gtfs_jp && records.feed_info.is_empty() {
        notices.push(notice(
            ctr,
            "JPN_007",
            EntityType::Feed,
            None,
            None,
            "feed_info.txt",
            None,
            Some("feed_info"),
            None,
            None,
            "GTFS-JP feed'inde feed_info.txt eksik — profil bunu (feed_lang=ja, yayıncı bilgisi) zorunlu kılar.".to_string(),
            "feed_info.txt ekleyin ve feed_lang=ja ile yayıncı bilgilerini doldurun.",
        ));
    }

    if legacy_v3_extensions {
    // ── JPN_012: agency_jp.agency_id zorunlu ─────────────────────────────────
    for aj in &records.agency_jp {
        if aj.agency_id.as_deref().is_none_or(|id| id.trim().is_empty()) {
            notices.push(notice(
                ctr, "JPN_012", EntityType::Agency, None, None,
                "agency_jp.txt", Some(aj.line), Some("agency_id"),
                Some(String::new()), Some("dolu".to_string()),
                "agency_jp.txt satırında agency_id eksik — GTFS-JP bu alanı zorunlu kılar.".to_string(),
                "agency_jp.txt'teki her satırı agency.txt'te tanımlı bir agency_id ile eşleştirin.",
            ));
        }
    }

    // ── JPN_013: agency_zip_number biçimi ────────────────────────────────────
    for aj in &records.agency_jp {
        let Some(raw) = aj.row.get("agency_zip_number").map(String::as_str) else { continue };
        let raw = raw.trim();
        if !raw.is_empty() && (raw.len() != 7 || !raw.chars().all(|c| c.is_ascii_digit())) {
            notices.push(notice(
                ctr, "JPN_013", EntityType::Agency,
                aj.agency_id.clone(), aj.agency_id.clone(),
                "agency_jp.txt", Some(aj.line), Some("agency_zip_number"),
                Some(raw.to_string()), Some("7 ASCII rakam".to_string()),
                format!("agency_zip_number '{}' geçersiz; GTFS-JP posta kodunu tire olmadan 7 rakam olarak ister.", raw),
                "agency_zip_number değerini tire olmadan 7 ASCII rakam olarak girin.",
            ));
        }
    }

    // ── JPN_014: office_jp.office_id zorunlu ve tekil ─────────────────────────
    {
        let mut office_ids: HashMap<&str, u64> = HashMap::new();
        for office in &records.office_jp {
            let office_id = office.office_id.trim();
            if office_id.is_empty() {
                notices.push(notice(
                    ctr, "JPN_014", EntityType::Agency, None, None,
                    "office_jp.txt", Some(office.line), Some("office_id"),
                    Some(String::new()), Some("dolu ve tekil".to_string()),
                    "office_jp.txt satırında office_id eksik — her ofis kaydı benzersiz bir kimlik taşımalıdır.".to_string(),
                    "office_jp.txt'teki her satıra benzersiz bir office_id girin.",
                ));
                continue;
            }
            if let Some(first_line) = office_ids.insert(office_id, office.line) {
                notices.push(notice(
                    ctr, "JPN_014", EntityType::Agency,
                    Some(office_id.to_string()), Some(office_id.to_string()),
                    "office_jp.txt", Some(office.line), Some("office_id"),
                    Some(office_id.to_string()), Some("unique".to_string()),
                    format!("office_id '{}' tekrarlanıyor (ilk görünüm: satır {}).", office_id, first_line),
                    "Her ofis kaydı için benzersiz bir office_id kullanın.",
                ));
            }
        }
    }

    // ── JPN_015: routes_jp.route_id zorunlu, tekil ve routes.txt'e bağlı ────
    {
        let route_ids: HashSet<&str> = records.routes.iter()
            .map(|route| route.route_id.as_str()).filter(|id| !id.is_empty()).collect();
        let mut jp_route_ids: HashMap<&str, u64> = HashMap::new();
        for route in &records.routes_jp {
            let route_id = route.route_id.trim();
            if route_id.is_empty() {
                notices.push(notice(
                    ctr, "JPN_015", EntityType::Route, None, None,
                    "routes_jp.txt", Some(route.line), Some("route_id"),
                    Some(String::new()), Some("routes.txt'te tanımlı route_id".to_string()),
                    "routes_jp.txt satırında route_id eksik.".to_string(),
                    "routes_jp.txt'te routes.txt'te tanımlı bir route_id kullanın.",
                ));
                continue;
            }
            if let Some(first_line) = jp_route_ids.insert(route_id, route.line) {
                notices.push(notice(
                    ctr, "JPN_015", EntityType::Route,
                    Some(route_id.to_string()), Some(route_id.to_string()),
                    "routes_jp.txt", Some(route.line), Some("route_id"),
                    Some(route_id.to_string()), Some("unique".to_string()),
                    format!("routes_jp.txt route_id '{}' tekrarlanıyor (ilk görünüm: satır {}).", route_id, first_line),
                    "Her routes_jp kaydı için benzersiz bir route_id kullanın.",
                ));
            }
            if !route_ids.contains(route_id) {
                notices.push(notice(
                    ctr, "JPN_015", EntityType::Route,
                    Some(route_id.to_string()), Some(route_id.to_string()),
                    "routes_jp.txt", Some(route.line), Some("route_id"),
                    Some(route_id.to_string()), None,
                    format!("routes_jp.txt route_id '{}' routes.txt'te tanımlı değil.", route_id),
                    "route_id değerini routes.txt'te tanımlı bir rota ile eşleştirin.",
                ));
            }
        }
    }

    // ── JPN_016: route_update_date YYYYMMDD ─────────────────────────────────
    // GTFS-JP v3'te tarih pattern_jp.txt içindedir. routes_jp.txt v3 dosyası
    // değildir; ancak eski feed uyumluluğu için aynı tarih alanını doğrulamaya
    // devam ederiz. Böylece legacy feed'lerde gerçek üretici hataları sessizce
    // kaybolmaz.
    for pattern in &records.pattern_jp {
        if let Some(date) = pattern.route_update_date.as_deref() {
            if !valid_gtfs_jp_date(date) {
                notices.push(notice(
                    ctr, "JPN_016", EntityType::Row,
                    Some(pattern.jp_pattern_id.clone()), Some(pattern.jp_pattern_id.clone()),
                    "pattern_jp.txt", Some(pattern.line), Some("route_update_date"),
                    Some(date.to_string()), Some("YYYYMMDD".to_string()),
                    format!("pattern_jp.txt route_update_date '{}' geçerli bir tarih değil.", date),
                    "route_update_date değerini geçerli bir YYYYMMDD tarihi olarak girin.",
                ));
            }
        }
    }
    for route in &records.routes_jp {
        if let Some(date) = route.route_update_date.as_deref() {
            if !valid_gtfs_jp_date(date) {
                notices.push(notice(
                    ctr, "JPN_016", EntityType::Row,
                    Some(route.route_id.clone()), Some(route.route_id.clone()),
                    "routes_jp.txt", Some(route.line), Some("route_update_date"),
                    Some(date.to_string()), Some("YYYYMMDD".to_string()),
                    format!("routes_jp.txt route_update_date '{}' geçerli bir tarih değil.", date),
                    "route_update_date değerini geçerli bir YYYYMMDD tarihi olarak girin.",
                ));
            }
        }
    }

    // ── JPN_017: pattern_jp.jp_pattern_id zorunlu ve tekil ───────────────────
    {
        let mut pattern_ids: HashMap<&str, u64> = HashMap::new();
        for pattern in &records.pattern_jp {
            let pattern_id = pattern.jp_pattern_id.trim();
            if pattern_id.is_empty() {
                notices.push(notice(
                    ctr, "JPN_017", EntityType::Row, None, None,
                    "pattern_jp.txt", Some(pattern.line), Some("jp_pattern_id"),
                    Some(String::new()), Some("dolu ve tekil".to_string()),
                    "pattern_jp.txt satırında jp_pattern_id eksik — dosya mevcutsa bu alan zorunludur.".to_string(),
                    "pattern_jp.txt'teki her satıra benzersiz bir jp_pattern_id girin.",
                ));
                continue;
            }
            if let Some(first_line) = pattern_ids.insert(pattern_id, pattern.line) {
                notices.push(notice(
                    ctr, "JPN_017", EntityType::Row,
                    Some(pattern_id.to_string()), Some(pattern_id.to_string()),
                    "pattern_jp.txt", Some(pattern.line), Some("jp_pattern_id"),
                    Some(pattern_id.to_string()), Some("unique".to_string()),
                    format!("jp_pattern_id '{}' tekrarlanıyor (ilk görünüm: satır {}).", pattern_id, first_line),
                    "Her duruş paterni için benzersiz bir jp_pattern_id kullanın.",
                ));
            }
        }
    }

    // ── JPN_018: trips.jp_pattern_id → pattern_jp.jp_pattern_id ──────────────
    // `pattern_jp.txt` ve trips.jp_pattern_id ayrı ayrı opsiyoneldir. jp_pattern_id
    // pattern_jp.txt yokken kurum içi kod olarak kullanılabilir; yalnızca pattern
    // master dosyası mevcutsa burada foreign-key bütünlüğü aranır.
    if has_pattern_jp_file {
        let pattern_ids: HashSet<&str> = records.pattern_jp.iter()
            .map(|pattern| pattern.jp_pattern_id.as_str()).filter(|id| !id.is_empty()).collect();
        for trip in &records.trips {
            let Some(pattern_id) = ti_jpn.jp_pattern_id(trip).filter(|id| !id.is_empty()) else { continue };
            if !pattern_ids.contains(pattern_id) {
                notices.push(notice(
                    ctr, "JPN_018", EntityType::Trip,
                    Some(trip.trip_id.to_string()), Some(trip.trip_id.to_string()),
                    "trips.txt", Some(trip.line), Some("jp_pattern_id"),
                    Some(pattern_id.to_string()), None,
                    format!("'{}' seferindeki jp_pattern_id '{}' pattern_jp.txt'te tanımlı değil.", trip.trip_id, pattern_id),
                    "jp_pattern_id değerini pattern_jp.txt'te tanımlı bir paterne bağlayın.",
                ));
            }
        }
    }

    }

    // ── JPN_019: ja-Hrkt çevirilerinde gerçek kayıt/alt kayıt bütünlüğü ───────
    {
        let agency_ids: HashSet<&str> = records.agencies.iter().filter_map(|a| a.agency_id.as_deref()).collect();
        let stop_ids: HashSet<&str> = records.stops.iter().map(|s| s.stop_id.as_str()).collect();
        let route_ids: HashSet<&str> = records.routes.iter().map(|r| r.route_id.as_str()).collect();
        let trip_ids: HashSet<&str> = records.trips.iter().map(|t| t.trip_id.as_str()).collect();
        let attribution_ids: HashSet<&str> = records.attributions.iter()
            .filter_map(|a| a.attribution_id.as_deref())
            .filter(|id| !id.is_empty())
            .collect();
        let pathway_ids: HashSet<&str> = records.pathways.iter()
            .map(|p| p.pathway_id.as_str()).filter(|id| !id.is_empty()).collect();
        let level_ids: HashSet<&str> = records.levels.iter()
            .map(|l| l.level_id.as_str()).filter(|id| !id.is_empty()).collect();
        for translation in records.translations.iter().filter(|t| t.language.eq_ignore_ascii_case("ja-Hrkt")) {
            // TRN_001/TRN_002 bilinmeyen table_name/field_name kök bulgularını
            // üretir. JPN_019 yalnız bilinen tabloların GTFS-JP referansını
            // denetler; aksi hâlde aynı satırda türev bulgu çığı oluşur.
            let table_known = crate::k2::translations::is_known_translation_table(&translation.table_name);
            if !table_known {
                continue;
            }
            let valid_field = crate::k2::translations::valid_fields_for_table(&translation.table_name)
                .contains(&translation.field_name.as_str());
            let valid_record = match (translation.table_name.as_str(), translation.record_id.as_deref()) {
                (_, None) => true,
                ("agency", Some(id)) => agency_ids.contains(id),
                ("stops", Some(id)) => stop_ids.contains(id),
                ("routes", Some(id)) => route_ids.contains(id),
                ("trips", Some(id)) | ("stop_times", Some(id)) => trip_ids.contains(id),
                ("attributions", Some(id)) => attribution_ids.contains(id),
                ("pathways", Some(id)) => pathway_ids.contains(id),
                ("levels", Some(id)) => level_ids.contains(id),
                // TRN_013 ile aynı hüküm: feed_info tek satırlıdır, kimlik alanı taşıyamaz.
                ("feed_info", Some(_)) => false,
                // Kimliği çözemediğimiz tablo hakkında İDDİA YOKTUR. `_ => false` bilinen
                // ama kolu yazılmamış her tabloyu sessizce yanlış pozitife çeviriyordu
                // (attributions: Katori feed'inde 8 bulgu; pathways/levels: ölçülen 2).
                // Emsal: TRN_004 aynı sözleşmeyi kullanır ("lookup not feasible" → true).
                _ => true,
            };
            let valid_sub_id = if translation.table_name == "stop_times" {
                match (translation.record_id.as_deref(), translation.record_sub_id.as_deref()) {
                    // `record_sub_id` is the stop_sequence belonging to the trip in
                    // `record_id`; numeric formatting alone does not prove this link.
                    (Some(trip_id), Some(sub)) => sub.parse::<u32>().ok()
                        .filter(|sequence| *sequence != u32::MAX)
                        .is_some_and(|sequence| {
                            records.stop_times_index.sorted_stops(trip_id).is_some_and(|rows| {
                                rows.binary_search_by_key(&sequence, |stop| stop.sequence).is_ok()
                            })
                        }),
                    // TRN_017 owns the missing-sub-id case when record_id is present.
                    (Some(_), None) | (None, None) => true,
                    // A sub-record cannot identify a stop_times row without its trip.
                    (None, Some(_)) => false,
                }
            } else if matches!(records.gtfs_jp_profile, GtfsJpProfile::V4) {
                match (
                    translation.table_name.as_str(),
                    translation.record_id.as_deref(),
                    translation.record_sub_id.as_deref(),
                ) {
                    // V4 follows the GTFS translations contract: record_sub_id
                    // is only used for stop_times. For other record-level tables
                    // the field must be omitted/blank, not replaced with NONE.
                    ("agency" | "stops" | "routes" | "trips", Some(_), Some(_)) => false,
                    ("agency" | "stops" | "routes" | "trips", Some(_), None) => true,
                    // field_value mode has no record identity or sub-record identity.
                    (_, None, None) => true,
                    (_, None, Some(_)) => false,
                    // Other record-level tables must not carry a sub-record id.
                    (_, Some(_), None) => true,
                    (_, Some(_), Some(_)) => false,
                }
            } else {
                translation.record_sub_id.as_deref().is_none_or(|value| {
                    crate::k2::translations::is_none_record_sub_id(value)
                })
            };
            if !valid_field || !valid_record || !valid_sub_id {
                notices.push(notice(
                    ctr, "JPN_019", EntityType::Translation,
                    translation.record_id.clone(), translation.record_id.clone(),
                    "translations.txt", Some(translation.line), Some("record_id|record_sub_id|field_name"),
                    translation.record_id.clone().or_else(|| translation.field_value.clone()).or_else(|| Some(translation.field_name.clone())),
                    None,
                    "ja-Hrkt çevirisi geçerli bir tablo alanını veya mevcut GTFS kaydını göstermiyor.".to_string(),
                    "table_name, field_name, record_id ve stop_times için record_sub_id değerlerini kaynak GTFS kayıtlarıyla eşleştirin.",
                ));
            }
        }
    }

    if legacy_v3_extensions {
    // ── JPN_020: office_url / office_phone kalite biçimi ─────────────────────
    for office in &records.office_jp {
        if let Some(url) = office.row.get("office_url").map(String::as_str).map(str::trim).filter(|v| !v.is_empty()) {
            if !crate::k2::common::looks_like_url(url) {
                notices.push(notice(
                    ctr, "JPN_020", EntityType::Agency,
                    Some(office.office_id.clone()), Some(office.office_id.clone()),
                    "office_jp.txt", Some(office.line), Some("office_url"),
                    Some(url.to_string()), Some("http:// veya https:// URL".to_string()),
                    format!("office_url '{}' geçerli bir http/https URL'si gibi görünmüyor.", url),
                    "office_url için geçerli bir http/https URL'si kullanın.",
                ));
            }
        }
        if let Some(phone) = office.row.get("office_phone").map(String::as_str).map(str::trim).filter(|v| !v.is_empty()) {
            let digits = phone.chars().filter(|c| c.is_ascii_digit()).count();
            let allowed = phone.chars().all(|c| c.is_ascii_digit() || matches!(c, '+' | '-' | '(' | ')' | ' '));
            if digits < 3 || !allowed {
                notices.push(notice(
                    ctr, "JPN_020", EntityType::Agency,
                    Some(office.office_id.clone()), Some(office.office_id.clone()),
                    "office_jp.txt", Some(office.line), Some("office_phone"),
                    Some(phone.to_string()), Some("telefon numarası".to_string()),
                    format!("office_phone '{}' geçerli bir telefon numarası gibi görünmüyor.", phone),
                    "office_phone alanına aranabilir bir telefon numarası girin.",
                ));
            }
        }
    }

    }

    // ── JPN_021: kana satırı boş/çakışmalı/tutarsız ────────────────────────────
    {
        let mut seen: HashMap<String, (String, u64)> = HashMap::new();
        for translation in records.translations.iter().filter(|t| t.language.eq_ignore_ascii_case("ja-Hrkt")) {
            if translation.translation.trim().is_empty() {
                notices.push(notice(
                    ctr, "JPN_021", EntityType::Translation,
                    translation.record_id.clone(), translation.record_id.clone(),
                    "translations.txt", Some(translation.line), Some("translation"),
                    Some(translation.translation.clone()), Some("dolu Japonca kana".to_string()),
                    "GTFS-JP ja-Hrkt çevirisi boş.".to_string(),
                    "ja-Hrkt translation alanını Japonca kana/kanji okumasıyla doldurun.",
                ));
                continue;
            }
            let key = format!("{}|{}|{}|{}|{}", translation.table_name, translation.field_name,
                translation.record_id.as_deref().unwrap_or(""), translation.record_sub_id.as_deref().unwrap_or(""),
                translation.field_value.as_deref().unwrap_or(""));
            if let Some((previous, first_line)) = seen.get(&key) {
                if *previous != translation.translation {
                    notices.push(notice(
                        ctr, "JPN_021", EntityType::Translation,
                        translation.record_id.clone(), translation.record_id.clone(),
                        "translations.txt", Some(translation.line), Some("translation"),
                        Some(translation.translation.clone()), Some(previous.clone()),
                        format!("Aynı ja-Hrkt çeviri hedefi ilk satır {} ile çelişiyor.", first_line),
                        "Aynı çeviri hedefi için tek ve tutarlı bir kana değeri bırakın.",
                    ));
                }
            } else {
                seen.insert(key, (translation.translation.clone(), translation.line));
            }
            if !has_japanese(&translation.translation) {
                notices.push(notice(
                    ctr, "JPN_021", EntityType::Translation,
                    translation.record_id.clone(), translation.record_id.clone(),
                    "translations.txt", Some(translation.line), Some("translation"),
                    Some(translation.translation.clone()), Some("Japonca kana/kanji".to_string()),
                    "ja-Hrkt çevirisi Japonca kana veya kanji içermiyor.".to_string(),
                    "ja-Hrkt translation alanına Japonca okunuşu yazın.",
                ));
            }
        }
    }
}

/// FLJ_001..004 — `fare_leg_join_rules.txt`'in dört Foreign ID alanı (Fares v2, issue #59).
///
/// Ağ alanları için hedef `EntityMap::network_ids`'tir — NET_002'nin aksine burada BİRLEŞİK
/// küme DOĞRUDUR, çünkü spec bu alanları *"referencing routes.network_id **or**
/// networks.network_id"* diye tanımlar. FLG_002 ile aynı hedef.
///
/// Durak alanları karşılıklı koşulludur: biri doluysa diğeri zorunludur. Spec ayrıca hedefin
/// durak (`location_type` 0/boş) veya istasyon (`location_type=1`) olmasını ister; giriş,
/// düğüm ve biniş alanı geçersizdir.
fn check_fare_leg_join_rules(
    records: &EntityRecords,
    map: &EntityMap,
    notices: &mut Vec<Notice>,
    ctr: &mut u32,
) {
    if records.fare_leg_join_rules.is_empty() { return; }

    for rec in &records.fare_leg_join_rules {
        // ── Ağ alanları: koşulsuz Required + FK ──────────────────────────────
        for (value, field, rule) in [
            (&rec.from_network_id, "from_network_id", "FLJ_001"),
            (&rec.to_network_id,   "to_network_id",   "FLJ_002"),
        ] {
            if value.is_empty() {
                notices.push(notice(
                    ctr, rule, EntityType::Row, None, None,
                    "fare_leg_join_rules.txt", Some(rec.line), Some(field),
                    Some(String::new()), None,
                    format!("fare_leg_join_rules.txt satırında {field} zorunludur."),
                    "networks.txt veya routes.network_id içinde tanımlı bir ağ kodu girin.",
                ));
            } else if !map.network_ids.contains(value.as_str()) {
                notices.push(notice(
                    ctr, rule, EntityType::Row, None, None,
                    "fare_leg_join_rules.txt", Some(rec.line), Some(field),
                    Some(value.clone()), None,
                    format!("'{value}' ağ kodu networks.txt/routes.txt içinde tanımlı değil."),
                    "networks.txt veya routes.network_id içinde tanımlı bir ağ kodu kullanın.",
                ));
            }
        }

        // ── Durak alanları: KARŞILIKLI koşullu + FK + location_type kısıtı ───
        let pairs = [
            (&rec.from_stop_id, &rec.to_stop_id, "from_stop_id", "to_stop_id", "FLJ_003"),
            (&rec.to_stop_id, &rec.from_stop_id, "to_stop_id", "from_stop_id", "FLJ_004"),
        ];
        for (value, other, field, other_field, rule) in pairs {
            if value.is_empty() {
                if !other.is_empty() {
                    notices.push(notice(
                        ctr, rule, EntityType::Row, None, None,
                        "fare_leg_join_rules.txt", Some(rec.line), Some(field),
                        Some(String::new()), None,
                        format!("{other_field} tanımlıyken {field} da zorunludur."),
                        "İki durak alanını birlikte doldurun ya da ikisini de boş bırakın.",
                    ));
                }
                continue;
            }
            match map.stops.get(value.as_str()).and_then(|&i| records.stops.get(i)) {
                None => notices.push(notice(
                    ctr, rule, EntityType::Row, None, None,
                    "fare_leg_join_rules.txt", Some(rec.line), Some(field),
                    Some(value.clone()), None,
                    format!("'{value}' durağı stops.txt'te tanımlı değil."),
                    "stops.txt'te tanımlı bir stop_id kullanın.",
                )),
                Some(stop) if !matches!(stop.location_type.unwrap_or(0), 0 | 1) => {
                    notices.push(notice(
                        ctr, rule, EntityType::Row, None, None,
                        "fare_leg_join_rules.txt", Some(rec.line), Some(field),
                        Some(value.clone()), Some("location_type 0 or 1".to_string()),
                        format!("'{value}' bir durak veya istasyon değil (location_type={}).",
                                stop.location_type.unwrap_or(0)),
                        "Durak (location_type 0/boş) veya istasyon (location_type=1) gösterin.",
                    ));
                }
                Some(_) => {}
            }
        }
    }
}

/// NET_002/NET_003 — `route_networks.txt`'in iki Required Foreign ID alanı.
///
/// ⚠️ `network_id` **yalnız `networks.txt`'e** karşı çözülür, `EntityMap::network_ids`'e karşı
/// DEĞİL: o küme `routes.network_id` ve `route_networks.txt`'in KENDİ satırlarını da içerir
/// (FLG_002 tam kümeye bakar), dolayısıyla ona karşı kontrol dairesel olur ve hiçbir zaman
/// ateşlemez. Spec bu alanı "Foreign ID referencing networks.network_id" olarak tanımlar.
fn check_route_networks(
    records: &EntityRecords,
    map: &EntityMap,
    notices: &mut Vec<Notice>,
    ctr: &mut u32,
) {
    if records.route_networks.is_empty() { return; }
    let declared: std::collections::HashSet<&str> = records.networks.iter()
        .map(|n| n.network_id.as_str()).filter(|s| !s.is_empty()).collect();

    for rec in &records.route_networks {
        let eid = Some(rec.route_id.clone()).filter(|s| !s.is_empty());

        // NET_002: network_id — Required + networks.txt'te tanımlı olmalı.
        if rec.network_id.is_empty() {
            notices.push(notice(
                ctr, "NET_002", EntityType::Row, eid.clone(), eid.clone(),
                "route_networks.txt", Some(rec.line), Some("network_id"),
                Some(String::new()), None,
                "route_networks.txt satırında network_id zorunludur.".to_string(),
                "networks.txt'te tanımlı bir network_id girin.",
            ));
        } else if !declared.contains(rec.network_id.as_str()) {
            notices.push(notice(
                ctr, "NET_002", EntityType::Row, eid.clone(), eid.clone(),
                "route_networks.txt", Some(rec.line), Some("network_id"),
                Some(rec.network_id.clone()), None,
                format!("'{}' ağ kodu networks.txt'te tanımlı değil.", rec.network_id),
                "networks.txt'te tanımlı bir network_id kullanın.",
            ));
        }

        // NET_003: route_id — Required + routes.txt'te tanımlı olmalı.
        if rec.route_id.is_empty() {
            notices.push(notice(
                ctr, "NET_003", EntityType::Row, None, None,
                "route_networks.txt", Some(rec.line), Some("route_id"),
                Some(String::new()), None,
                "route_networks.txt satırında route_id zorunludur.".to_string(),
                "routes.txt'te tanımlı bir route_id girin.",
            ));
        } else if !map.routes.contains_key(rec.route_id.as_str()) {
            notices.push(notice(
                ctr, "NET_003", EntityType::Row, eid.clone(), eid.clone(),
                "route_networks.txt", Some(rec.line), Some("route_id"),
                Some(rec.route_id.clone()), None,
                format!("'{}' hattı routes.txt'te tanımlı değil.", rec.route_id),
                "routes.txt'te tanımlı bir route_id kullanın.",
            ));
        }
    }
}

fn check_attributions(
    records: &EntityRecords,
    map: &EntityMap,
    notices: &mut Vec<Notice>,
    ctr: &mut u32,
) {
    for rec in &records.attributions {
        let eid = rec.attribution_id.clone();

        // ATR_010: agency_id cross-ref — attribution'daki agency_id agency.txt'te bulunamadı
        if let Some(ref aid) = rec.agency_id {
            if !map.agencies.contains_key(aid.as_str()) {
                notices.push(notice(
                    ctr,
                    "ATR_010",
                    EntityType::Attribution,
                    eid.clone(),
                    eid.clone(),
                    "attributions.txt",
                    Some(rec.line),
                    Some("agency_id"),
                    Some(aid.clone()),
                    None,
                    format!("'{}' işletici kodu agency.txt'te tanımlı değil.", aid),
                    "Geçerli bir agency_id kullanın.",
                ));
            }
        }

        // ATR_011: route_id referansı
        if let Some(ref rid) = rec.route_id {
            if !map.routes.contains_key(rid.as_str()) {
                notices.push(notice(
                    ctr,
                    "ATR_011",
                    EntityType::Attribution,
                    eid.clone(),
                    eid.clone(),
                    "attributions.txt",
                    Some(rec.line),
                    Some("route_id"),
                    Some(rid.clone()),
                    None,
                    format!("'{}' hattı routes.txt'te tanımlı değil.", rid),
                    "Geçerli bir route_id kullanın.",
                ));
            }
        }

        // ATR_012: trip_id referansı
        if let Some(ref tid) = rec.trip_id {
            if !map.trips.contains_key(tid.as_str()) {
                notices.push(notice(
                    ctr,
                    "ATR_012",
                    EntityType::Attribution,
                    eid.clone(),
                    eid.clone(),
                    "attributions.txt",
                    Some(rec.line),
                    Some("trip_id"),
                    Some(tid.clone()),
                    None,
                    format!("'{}' sefer kodu trips.txt'te tanımlı değil.", tid),
                    "Geçerli bir trip_id kullanın.",
                ));
            }
        }

        // ATR_009: agency_id, route_id, trip_id kar�Yılıklı dı�Ylayıcı
        let filled = [&rec.agency_id, &rec.route_id, &rec.trip_id]
            .iter()
            .filter(|x| x.is_some())
            .count();
        if filled > 1 {
            notices.push(notice(
                ctr,
                "ATR_009",
                EntityType::Attribution,
                eid.clone(),
                eid.clone(),
                "attributions.txt",
                Some(rec.line),
                // Olgu ÜÇ hedef alanının birlikte kullanılmasıdır.
                Some("agency_id|route_id|trip_id"),
                None,
                Some("at most one may be set (agency_id/route_id/trip_id)".to_string()),
                "agency_id, route_id ve trip_id aynı satırda birlikte kullanılamaz.".to_string(),
                "Bu alanlardan yalnızca birini doldurun.",
            ));
        }
    }
}

// �"?�"? XFL: çapraz dosya kontrolleri �"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?

fn check_xfl(
    records: &EntityRecords,
    map: &EntityMap,
    notices: &mut Vec<Notice>,
    ctr: &mut u32,
    trips_in_stm: &HashSet<&str>,
    trip_stm_count: &HashMap<&str, u32>,
) {
    let ti_xfl = &records.trip_interns;

    // XFL_002: her trip'in stop_times'ta kaydı var
    {
        for rec in &records.trips {
            if rec.trip_id.is_empty() {
                continue;
            }
            if !trips_in_stm.contains(rec.trip_id.as_str()) {
                notices.push(notice(
                    ctr,
                    "XFL_002",
                    EntityType::Trip,
                    Some(rec.trip_id.to_string()),
                    Some(rec.trip_id.to_string()),
                    "trips.txt",
                    Some(rec.line),
                    Some("trip_id"),
                    Some(rec.trip_id.to_string()),
                    None,
                    format!("'{}' hattının{} seferi tanımlanmış ama çalışma saatleri girilmemiş (stop_times.txt'te kayıt yok).",
                        ti_xfl.route_id(rec),
                        ti_xfl.headsign(rec)
                            .filter(|h| !h.is_empty())
                            .map(|h| format!(" '{}' istikametli", h))
                            .unwrap_or_default(),
                    ),
                    "Bu sefer için stop_times kayıtları ekleyin.",
                ));
            }
        }
    }

    // STM_033: tek duraklı sefer — stop_times girişi var ama yalnızca 1 tane
    // (sefer hiçbir yere gidemez → "unusable trip")
    {
        for rec in &records.trips {
            if rec.trip_id.is_empty() { continue; }
            if trip_stm_count.get(rec.trip_id.as_str()).copied().unwrap_or(0) == 1 {
                notices.push(notice(
                    ctr,
                    "STM_033",
                    EntityType::Trip,
                    Some(rec.trip_id.to_string()),
                    Some(rec.trip_id.to_string()),
                    "trips.txt",
                    Some(rec.line),
                    Some("trip_id"),
                    Some("1".to_string()),
                    Some("≥ 2".to_string()),
                    format!(
                        "'{}' seferinin stop_times.txt'te yalnızca 1 durağı var; sefer kullanılamaz.",
                        rec.trip_id
                    ),
                    "Sefere en az iki durak durağı ekleyin.",
                ));
            }
        }
    }

    // XFL_005 KALDIRILDI (0.8.0): stop_times→stops FK ihlalini STM_002 ile birebir aynı
    // kümeden (`bad_stop_ids`) raporluyordu. İkisi de Kritik+Spec olduğu için tek eksik
    // durak iki R1 blocker'ı ve iki pub_score cezası üretiyordu; R9 kuyruğunda da iki ayrı
    // iş gibi görünüyordu. STM_002 satır numarası taşıdığı ve distinct stop_id başına
    // toplandığı için korunan taraf o oldu; XFL_005'in blocks'u STM_002'ye devredildi.


    // PTH_002 / PTH_003: pathway'deki from/to stop_id stops.txt'te mevcut
    {
        for rec in &records.pathways {
            for (rule_id, field, stop_id) in [
                ("PTH_002", "from_stop_id", rec.from_stop_id.as_str()),
                ("PTH_003", "to_stop_id",   rec.to_stop_id.as_str()),
            ] {
                if !stop_id.is_empty() && !map.stops.contains_key(stop_id) {
                    notices.push(notice(
                        ctr,
                        rule_id,
                        EntityType::Pathway,
                        Some(rec.pathway_id.clone()),
                        Some(rec.pathway_id.clone()),
                        "pathways.txt",
                        Some(rec.line),
                        Some(field),
                        Some(stop_id.to_string()),
                        None,
                        format!("{field} '{stop_id}' stops.txt'te tanımlı değil."),
                        "Geçerli bir stop_id kullanın.",
                    ));
                }
            }
        }
    }

    // PTH_026: pathway uç noktası istasyon olamaz. Spec, from_stop_id/to_stop_id için:
    // "Must contain a stop_id that identifies a platform (location_type=0 or empty),
    // entrance/exit (2), generic node (3) or boarding area (4). Values for stop_id that
    // identify stations (location_type=1) ... are forbidden."
    // ── presence:required boşlukları (çapa granülerliği triyajı, 2026-08-05) ─────
    //
    // "X yineleniyor" ve "X bulunamadı" başlıklı kurallar BOŞ DEĞERİ HİÇ GÖRMÜYORDU:
    // yinelenme kontrolü boş anahtarları atlar (haklı — boşlar birbirinin kopyası değildir),
    // FK çözümü boşu "referans yok" sayıp geçer. Aradaki "bu alan zorunludur" hükmü
    // ikisinin arasından düşüyordu. Her biri `presence_probe.py` ile boş değerde sınandı.
    //
    // Emit K4'te toplandı çünkü bu kayıtların bir kısmı `parse_*` ile üretiliyor ve notice
    // döndürmüyor; `row` ve `line` ise burada erişilebilir. `trips.service_id` istisnadır
    // (TripRecord bellek için `row` tutmaz) → `TRP_035` K2'de.
    {
        use crate::k2::common::require_nonempty;
        for r in &records.areas {
            require_nonempty(&r.row, "area_id", "ARS_002", EntityType::Row,
                (!r.area_id.is_empty()).then(|| r.area_id.clone()), "areas.txt", r.line,
                "areas.txt'te area_id zorunludur.".to_string(),
                "Her alana benzersiz bir area_id verin.", notices, ctr);
        }
        for r in &records.calendars {
            for day in ["monday","tuesday","wednesday","thursday","friday","saturday","sunday"] {
                require_nonempty(&r.row, day, "CAL_025", EntityType::Row,
                    (!r.service_id.is_empty()).then(|| r.service_id.clone()), "calendar.txt", r.line,
                    format!("service_id '{}' için '{day}' alanı boş; gün alanları zorunludur (0 veya 1).", r.service_id),
                    "Haftanın her günü için 0 ya da 1 girin.", notices, ctr);
            }
        }
        for r in &records.fare_media {
            require_nonempty(&r.row, "fare_media_id", "FMD_004", EntityType::Row,
                (!r.fare_media_id.is_empty()).then(|| r.fare_media_id.clone()), "fare_media.txt", r.line,
                "fare_media_id zorunludur.".to_string(),
                "Her ücret aracına benzersiz bir fare_media_id verin.", notices, ctr);
        }
        for r in &records.networks {
            require_nonempty(&r.row, "network_id", "NET_004", EntityType::Row,
                (!r.network_id.is_empty()).then(|| r.network_id.clone()), "networks.txt", r.line,
                "networks.txt'te network_id zorunludur.".to_string(),
                "Her ağa benzersiz bir network_id verin.", notices, ctr);
        }
        for r in &records.rider_categories {
            require_nonempty(&r.row, "rider_category_id", "RCT_008", EntityType::Row,
                (!r.rider_category_id.is_empty()).then(|| r.rider_category_id.clone()),
                "rider_categories.txt", r.line,
                "rider_category_id zorunludur.".to_string(),
                "Her yolcu kategorisine benzersiz bir rider_category_id verin.", notices, ctr);
        }
        // ⚠️ Bu üç kayıt ham satırı (`row`) TUTMAZ, yalnız alanları taşır. Dolayısıyla
        // "sütun hiç yok" (→ARC_025) ile "sütun var, değer boş" (→bu kurallar) ayrımı
        // BURADA YAPILAMAZ: sütun yoksa alan da boş string olur ve iki notice birden çıkar.
        // Küçük bir gürültü; alternatifi bu kayıtlara RowMap eklemekti ve bu kurallar için
        // bellek maliyetine değmez.
        let emit_empty = |cond: bool, rule: &'static str, file: &'static str,
                              field: &'static str, line: u64, msg: String, fix: &'static str,
                              notices: &mut Vec<gtfs_core::Notice>, ctr: &mut u32| {
            if cond {
                notices.push(notice(ctr, rule, EntityType::Row, None, None, file, Some(line),
                    Some(field), Some(String::new()), Some("(dolu)".to_string()), msg, fix));
            }
        };
        for r in &records.location_groups {
            emit_empty(r.location_group_id.trim().is_empty(), "XFL_032", "location_groups.txt",
                "location_group_id", r.line,
                "location_groups.txt'te location_group_id zorunludur.".to_string(),
                "Her konum grubuna benzersiz bir location_group_id verin.", notices, ctr);
        }
        for r in &records.location_group_stops {
            emit_empty(r.location_group_id.trim().is_empty(), "XFL_033", "location_group_stops.txt",
                "location_group_id", r.line,
                "location_group_stops.txt'te location_group_id zorunludur.".to_string(),
                "Satırı bir location_group_id'ye bağlayın.", notices, ctr);
            emit_empty(r.stop_id.trim().is_empty(), "XFL_034", "location_group_stops.txt",
                "stop_id", r.line,
                "location_group_stops.txt'te stop_id zorunludur.".to_string(),
                "Satırı bir stop_id'ye bağlayın.", notices, ctr);
        }
        for r in &records.stop_areas {
            require_nonempty(&r.row, "area_id", "SAR_003", EntityType::Row,
                None, "stop_areas.txt", r.line,
                "stop_areas.txt'te area_id zorunludur.".to_string(),
                "Satırı bir area_id'ye bağlayın.", notices, ctr);
            require_nonempty(&r.row, "stop_id", "SAR_004", EntityType::Row,
                None, "stop_areas.txt", r.line,
                "stop_areas.txt'te stop_id zorunludur.".to_string(),
                "Satırı bir stop_id'ye bağlayın.", notices, ctr);
        }
        for r in &records.timeframes {
            require_nonempty(&r.row, "service_id", "TFR_008", EntityType::Row,
                (!r.timeframe_group_id.is_empty()).then(|| r.timeframe_group_id.clone()),
                "timeframes.txt", r.line,
                "timeframes.txt'te service_id zorunludur.".to_string(),
                "Her zaman dilimini calendar'da tanımlı bir service_id'ye bağlayın.", notices, ctr);
        }
    }

    // PTH_030: bir platformun boarding area'ları modellenmişse pathway'ler BOARDING AREA'lara
    // bağlanır; spec ekler: "In such cases, the platform must not have pathways assigned."
    //
    // Grafik kodu iç içe modellemeyi TOLERE eder (`k6_analytics.rs` PTH_012, çocuk düğümden
    // erişilebilirlik) ama yasağı ÖLÇMEZ. Korpusta örnek yok — endişe edilen gürültü
    // gerçekleşmiyor, kural sentetik fixture ile kanıtlanır.
    {
        let mut has_boarding_area: HashSet<&str> = HashSet::new();
        for rec in &records.stops {
            if rec.location_type == Some(4) {
                if let Some(parent) = rec.row.get("parent_station").map(String::as_str) {
                    if !parent.trim().is_empty() {
                        has_boarding_area.insert(parent.trim());
                    }
                }
            }
        }
        if !has_boarding_area.is_empty() {
            for rec in &records.pathways {
                for (field, stop_id) in [
                    ("from_stop_id", rec.from_stop_id.as_str()),
                    ("to_stop_id", rec.to_stop_id.as_str()),
                ] {
                    if !has_boarding_area.contains(stop_id) {
                        continue;
                    }
                    notices.push(notice(
                        ctr, "PTH_030", EntityType::Row,
                        Some(rec.pathway_id.clone()), Some(rec.pathway_id.clone()),
                        "pathways.txt", Some(rec.line), Some(field),
                        Some(stop_id.to_string()), None,
                        format!(
                            "pathway '{}' {field}='{stop_id}' platformuna bağlanıyor ama o platformun \
                             boarding area'ları tanımlı — pathway'ler boarding area'lara bağlanmalıdır.",
                            rec.pathway_id
                        ),
                        "Pathway'i platformun boarding area'sına (location_type=4) bağlayın.",
                    ));
                }
            }
        }
    }

    // PTH_031: aynı spec cümlesinin İKİNCİ KOLU — `stop_access=1` olan durak da pathway uç
    // noktası OLAMAZ. `stop_access=1` "bu durağa sokaktan doğrudan erişilir, istasyonun
    // giriş/pathway'lerinden bağımsız yol tarifi üret" demektir; ona pathway bağlamak
    // kendi kendisiyle çelişir. 2026-08-06'ya kadar burada "kapsam dışıdır" yazıyordu ama
    // hükmü ölçen BAŞKA bir kural da yoktu (`STP_027` TERS olguyu ölçer: pathway'li
    // istasyonda `stop_access` BOŞ bırakılmış platform).
    {
        for rec in &records.pathways {
            for (field, stop_id) in [
                ("from_stop_id", rec.from_stop_id.as_str()),
                ("to_stop_id", rec.to_stop_id.as_str()),
            ] {
                let Some(&sidx) = map.stops.get(stop_id) else { continue };
                let stop = &records.stops[sidx];
                // `stop_access` yalnız peron/durakta anlamlıdır (spec: istasyon, giriş,
                // ara düğüm ve biniş alanında Conditionally Forbidden) → location_type
                // 0/boş dışında bakmıyoruz; oradaki hata STP_026'nın alanı.
                if matches!(stop.location_type, None | Some(0)) && stop.stop_access == Some(1) {
                    notices.push(notice(
                        ctr,
                        "PTH_031",
                        EntityType::Pathway,
                        Some(rec.pathway_id.clone()),
                        Some(rec.pathway_id.clone()),
                        "pathways.txt",
                        Some(rec.line),
                        Some(field),
                        Some(stop_id.to_string()),
                        Some("stop_access ≠ 1".to_string()),
                        format!(
                            "{field} '{stop_id}' stop_access=1 ile işaretli (sokaktan doğrudan erişilir); böyle bir durak pathway uç noktası olamaz.",
                        ),
                        "Durağın stop_access değerini 0 yapın ya da bu pathway kaydını kaldırın.",
                    ));
                }
                if stop.location_type == Some(1) {
                    notices.push(notice(
                        ctr,
                        "PTH_026",
                        EntityType::Pathway,
                        Some(rec.pathway_id.clone()),
                        Some(rec.pathway_id.clone()),
                        "pathways.txt",
                        Some(rec.line),
                        Some(field),
                        Some(stop_id.to_string()),
                        Some("location_type 0, 2, 3 or 4".to_string()),
                        format!(
                            "{field} '{stop_id}' bir istasyon (location_type=1); pathway uç noktası peron, giriş/çıkış, ara düğüm veya biniş alanı olmalıdır.",
                        ),
                        "pathways.txt'te istasyon yerine peron (location_type=0), giriş/çıkış (2), ara düğüm (3) veya biniş alanı (4) referansı kullanın.",
                    ));
                }
            }
        }
    }




    // SHP_019: shape trip(ler) tarafından referanslanmış ama o trip'lerin hiçbirinin stop_times'ı yok
    {
        // shape_id → referans eden trip_id listesi
        let mut shape_to_trips: HashMap<&str, Vec<&str>> = HashMap::new();
        for trip in &records.trips {
            if let Some(sid) = ti_xfl.shape_id(trip) {
                if !sid.is_empty() {
                    shape_to_trips.entry(sid).or_default().push(trip.trip_id.as_str());
                }
            }
        }

        for (shape_id, trip_ids) in &shape_to_trips {
            if trip_ids.iter().all(|tid| !trips_in_stm.contains(*tid)) {
                let line = records.shapes.iter()
                    .find(|sp| records.shape_interns.id(sp) == *shape_id)
                    .map(|sp| sp.line_u64());
                let trip_count = trip_ids.len();
                notices.push(notice(
                    ctr,
                    "SHP_019",
                    EntityType::Shape,
                    Some((*shape_id).to_string()),
                    Some((*shape_id).to_string()),
                    "shapes.txt",
                    line,
                    Some("shape_id"),
                    Some(format!("{trip_count}")),
                    None,
                    format!(
                        "'{}' güzergah şekli {} sefer tarafından referans alınmış ama bu seferlerin hiçbirinde stop_times kaydı yok.",
                        shape_id, trip_count
                    ),
                    "Seferlere stop_times ekleyin veya ilgili sefer ve güzergah şekli kayıtlarını kaldırın.",
                ));
            }
        }
    }


    // XFL_006: calendar_dates'te yalnızca exception_type=2 olan ve calendar.txt'te de bulunmayan service_id'ler
    {
        let cal_services: HashSet<&str> = records
            .calendars
            .iter()
            .map(|c| c.service_id.as_str())
            .collect();
        let added_services: HashSet<&str> = records.calendar_dates.added.keys()
            .map(|s| s.as_str()).collect();
        // removed.keys() = services with at least one type=2 record (valid date or not)
        let bad: HashSet<&str> = records.calendar_dates.removed.keys()
            .map(|s| s.as_str())
            .filter(|sid| !cal_services.contains(*sid) && !added_services.contains(*sid))
            .collect();
        if !bad.is_empty() {
            let bad_list: Vec<&str> = bad.into_iter().collect();
            notices.push(notice(
                ctr,
                "XFL_006",
                EntityType::Service,
                None,
                None,
                "calendar_dates.txt",
                None,
                Some("service_id"),
                Some(bad_list.join(", ")),
                None,
                format!(
                    "calendar_dates.txt'te yalnızca exception_type=2 (iptal) kaydı olan ve calendar.txt'te tanımlı olmayan service_id'ler: {}. Bu servisler hiç aktif olamaz.",
                    bad_list.join(", ")
                ),
                "calendar.txt'e servis ekleyin veya exception_type=1 kaydı tanımlayın.",
            ));
        }
    }

    // XFL_011: feed_info start/end vs calendar aralıkları tutarlılı�Yı
    {
        if let Some(fi) = records.feed_info.first() {
            if let (Some(fi_start), Some(fi_end)) = (fi.feed_start_date, fi.feed_end_date) {
                let fi_start_u32 = date_to_u32(fi_start);
                let fi_end_u32 = date_to_u32(fi_end);
                for rec in &records.calendars {
                    if rec.service_id.is_empty() {
                        continue;
                    }
                    let mut inconsistent = false;
                    let mut observed = String::new();
                    if let Some(cal_start) = rec.start_date {
                        let cs = date_to_u32(cal_start);
                        if cs < fi_start_u32 {
                            inconsistent = true;
                            observed = format!("cal_start={cs} < feed_start={fi_start_u32}");
                        }
                    }
                    if let Some(cal_end) = rec.end_date {
                        let ce = date_to_u32(cal_end);
                        if ce > fi_end_u32 {
                            inconsistent = true;
                            if observed.is_empty() {
                                observed = format!("cal_end={ce} > feed_end={fi_end_u32}");
                            } else {
                                observed = format!("{observed}, cal_end={ce} > feed_end={fi_end_u32}");
                            }
                        }
                    }
                    if inconsistent {
                        notices.push(notice(
                            ctr,
                            "XFL_011",
                            EntityType::Service,
                            Some(rec.service_id.clone()),
                            Some(rec.service_id.clone()),
                            "calendar.txt",
                            Some(rec.line),
                            Some("start_date|end_date"),
                            Some(observed),
                            Some(format!("{fi_start_u32}..{fi_end_u32}")),
                            format!(
                                "service_id '{}' takvim aralığı feed_info.txt'deki feed_start/end aralığıyla tutarsız.",
                                rec.service_id
                            ),
                            "feed_info.txt'deki feed_start_date ve feed_end_date'i calendar.txt aralıklarıyla uyumlu hale getirin.",
                        ));
                    }
                }
            }
        }
    }

    // CAL_019: Servis takvim aralığı feed_info.txt geçerlilik penceresi dışına taşıyor
    {
        if let Some(fi) = records.feed_info.first() {
            let fi_start_opt = fi.feed_start_date.map(date_to_u32);
            let fi_end_opt   = fi.feed_end_date.map(date_to_u32);
            if fi_start_opt.is_some() || fi_end_opt.is_some() {
                for rec in &records.calendars {
                    if rec.service_id.is_empty() { continue; }
                    let mut outside = false;
                    let mut observed = String::new();
                    if let (Some(fi_s), Some(cal_start)) = (fi_start_opt, rec.start_date) {
                        let cs = date_to_u32(cal_start);
                        if cs < fi_s {
                            outside = true;
                            observed = format!("start_date={cs} < feed_start_date={fi_s}");
                        }
                    }
                    if let (Some(fi_e), Some(cal_end)) = (fi_end_opt, rec.end_date) {
                        let ce = date_to_u32(cal_end);
                        if ce > fi_e {
                            outside = true;
                            if observed.is_empty() {
                                observed = format!("end_date={ce} > feed_end_date={fi_e}");
                            } else {
                                observed = format!("{observed}, end_date={ce} > feed_end_date={fi_e}");
                            }
                        }
                    }
                    if outside {
                        notices.push(notice(
                            ctr,
                            "CAL_019",
                            EntityType::Service,
                            Some(rec.service_id.clone()),
                            Some(rec.service_id.clone()),
                            "calendar.txt",
                            Some(rec.line),
                            Some("start_date|end_date"),
                            Some(observed),
                            None,
                            format!("Servis '{}' takvim tarihleri feed_info.txt geçerlilik penceresi dışında.", rec.service_id),
                            "feed_info.txt'deki feed_start_date/feed_end_date'i ya da calendar.txt'deki servis tarihlerini güncelleyin.",
                        ));
                    }
                }
            }
        }
    }

    // XFL_012: trips'i olan ama hiçbir trip'inin stop_times'ta kaydı olmayan rota
    {
        let mut route_trip_ids: HashMap<&str, Vec<&str>> = HashMap::new();
        for t in &records.trips {
            if !ti_xfl.route_id(t).is_empty() && !t.trip_id.is_empty() {
                route_trip_ids
                    .entry(ti_xfl.route_id(t))
                    .or_default()
                    .push(t.trip_id.as_str());
            }
        }
        for rec in &records.routes {
            if rec.route_id.is_empty() {
                continue;
            }
            if let Some(trip_ids) = route_trip_ids.get(rec.route_id.as_str()) {
                if !trip_ids.is_empty()
                    && trip_ids.iter().all(|tid| !trips_in_stm.contains(*tid))
                {
                    notices.push(notice(
                        ctr,
                        "XFL_012",
                        EntityType::Route,
                        Some(rec.route_id.clone()),
                        Some(rec.route_id.clone()),
                        "routes.txt",
                        Some(rec.line),
                        Some("route_id"),
                        Some(rec.route_id.clone()),
                        None,
                        format!(
                            "route_id '{}' için tüm seferlerin stop_times.txt kaydı yok; rota fiilen hizmet vermiyor.",
                            rec.route_id
                        ),
                        "Bu rotanın seferlerine stop_times kayıtları ekleyin veya rotayı kaldırın.",
                    ));
                }
            }
        }
    }

    // XFL_013: shape_id hem gidiş hem dönüş yönünde kullanılıyor.
    // Mesaja her yönün hat (route_id), çalışma takvimi (service_id) ve ilk kalkış saatini ekle;
    // farklı hat/takvim/sefer varsa hepsini listele (kalkışlar 6 ile sınırlı, dil-nötr "(+N)").
    {
        #[derive(Default)]
        struct DirInfo {
            routes: BTreeSet<String>,
            services: BTreeSet<String>,
            deps: BTreeSet<String>,
        }
        // dil-nötr birleştirme: "a, b, c (+N)" — sone dilden bağımsız (en/ja details için güvenli)
        fn join_capped(set: &BTreeSet<String>, cap: usize) -> String {
            if set.is_empty() {
                return "—".to_string();
            }
            let n = set.len();
            let shown = set.iter().take(cap).cloned().collect::<Vec<_>>().join(", ");
            if n > cap { format!("{shown} (+{})", n - cap) } else { shown }
        }

        // shape_id → (gidiş, dönüş) detayları
        let mut shapes: HashMap<&str, (DirInfo, DirInfo)> = HashMap::new();
        for t in &records.trips {
            let shape_id = match ti_xfl.shape_id(t).filter(|s| !s.is_empty()) {
                Some(s) => s,
                None => continue,
            };
            let dir = match t.direction_id {
                Some(0) => 0u32,
                Some(1) => 1u32,
                _ => continue,
            };
            let entry = shapes.entry(shape_id).or_default();
            let info = if dir == 0 { &mut entry.0 } else { &mut entry.1 };
            if !ti_xfl.route_id(t).is_empty() {
                info.routes.insert(ti_xfl.route_id(t).to_string());
            }
            if !ti_xfl.service_id(t).is_empty() {
                info.services.insert(ti_xfl.service_id(t).to_string());
            }
            if let Some(dep) = records
                .stop_times_index
                .sorted_stops(&t.trip_id)
                .and_then(|s| s.first())
                .and_then(|st| st.departure_time())
                .map(|(h, m, _)| format!("{h:02}:{m:02}"))
            {
                info.deps.insert(dep);
            }
        }

        let mut shape_ids: Vec<&str> = shapes.keys().copied().collect();
        shape_ids.sort_unstable();
        for shape_id in shape_ids {
            let (d0, d1) = &shapes[shape_id];
            let has0 = !d0.routes.is_empty() || !d0.services.is_empty() || !d0.deps.is_empty();
            let has1 = !d1.routes.is_empty() || !d1.services.is_empty() || !d1.deps.is_empty();
            if !(has0 && has1) {
                continue;
            }

            let fwd_routes = join_capped(&d0.routes, 5);
            let fwd_services = join_capped(&d0.services, 5);
            let fwd_deps = join_capped(&d0.deps, 6);
            let bwd_routes = join_capped(&d1.routes, 5);
            let bwd_services = join_capped(&d1.services, 5);
            let bwd_deps = join_capped(&d1.deps, 6);

            let mut details: std::collections::BTreeMap<String, String> = std::collections::BTreeMap::new();
            details.insert("fwd_routes".into(), fwd_routes.clone());
            details.insert("fwd_services".into(), fwd_services.clone());
            details.insert("fwd_deps".into(), fwd_deps.clone());
            details.insert("bwd_routes".into(), bwd_routes.clone());
            details.insert("bwd_services".into(), bwd_services.clone());
            details.insert("bwd_deps".into(), bwd_deps.clone());

            let mut n = notice(
                ctr,
                "XFL_013",
                EntityType::Shape,
                Some(shape_id.to_string()),
                Some(shape_id.to_string()),
                "trips.txt",
                None,
                Some("shape_id"),
                Some(shape_id.to_string()),
                None,
                format!(
                    "shape_id '{shape_id}' hem gidiş (yön 0) hem dönüş (yön 1) seferlerinde kullanılıyor. \
                     Gidiş → hat {fwd_routes}, takvim {fwd_services}, kalkış {fwd_deps}; \
                     Dönüş → hat {bwd_routes}, takvim {bwd_services}, kalkış {bwd_deps}.",
                ),
                "Gidiş ve dönüş yönleri için ayrı shape_id tanımlayın.",
            );
            n.details = Some(details);
            notices.push(n);
        }
    }

    // TRN_016: `field_value` biçimli çeviri hiçbir kayıtla eşleşmiyor.
    //
    // Spec: "The field must have exactly the value defined in field_value." Bu biçim
    // `record_id` yerine DEĞERE göre eşleşir; eşleşen satır yoksa çeviri sessizce etkisizdir
    // (üretici çevirdiğini sanır). `XFL_014` yalnız `record_id` kolunu kapsar.
    //
    // ⚠️ KAPSAM SINIRI: yalnız ham satırı (`row`) taşıyan tablolar denetlenir. `trips` bellek
    // optimizasyonu nedeniyle `row` tutmaz → `table_name=trips` çevirileri SESSİZCE atlanır.
    // Yanlış negatif yönünde bilinçli tercih; ölçümde trips payı 339/4362 idi.
    //
    // ⚠️ `field_name` hedef dosyada SÜTUN DEĞİLSE burada sayılmaz — o ihlal `TRN_002`'nin
    // ("field_name bu tablo için geçersiz") alanıdır. Ölçüm sırasında bu ayrım yapılmadığında
    // tek feed'de 2546 sahte bulgu çıkmıştı.
    {
        let mut unmatched: Vec<String> = Vec::new();
        for rec in &records.translations {
            let Some(want) = rec.field_value.as_deref().filter(|v| !v.is_empty()) else { continue };
            if rec.record_id.as_deref().is_some_and(|r| !r.is_empty()) {
                continue;                       // `record_id` biçimi → XFL_014'ün alanı
            }
            let field = rec.field_name.as_str();
            if field.is_empty() {
                continue;
            }
            let rows: Vec<&crate::k2::common::RowMap> = match rec.table_name.as_str() {
                "agency" => records.agencies.iter().map(|r| &r.row).collect(),
                "stops" => records.stops.iter().map(|r| &r.row).collect(),
                "routes" => records.routes.iter().map(|r| &r.row).collect(),
                "levels" => records.levels.iter().map(|r| &r.row).collect(),
                "pathways" => records.pathways.iter().map(|r| &r.row).collect(),
                "calendar" => records.calendars.iter().map(|r| &r.row).collect(),
                "fare_attributes" => records.fare_attributes.iter().map(|r| &r.row).collect(),
                "feed_info" => records.feed_info.iter().map(|r| &r.row).collect(),
                _ => continue,                  // desteklenmeyen tablo → sessiz
            };
            if rows.is_empty() || !rows[0].contains_key(field) {
                continue;                       // dosya yok ya da sütun yok (TRN_002)
            }
            if !rows.iter().any(|r| r.get(field).map(String::as_str) == Some(want)) {
                unmatched.push(format!("{}.{field}={want}", rec.table_name));
            }
        }
        if !unmatched.is_empty() {
            unmatched.sort_unstable();
            unmatched.dedup();
            let total = unmatched.len();
            let sample: Vec<&str> = unmatched.iter().take(5).map(String::as_str).collect();
            notices.push(notice(
                ctr, "TRN_016", EntityType::Feed, None, None,
                "translations.txt", None, Some("field_value"),
                Some(total.to_string()), None,
                format!(
                    "{total} çeviri kaydının 'field_value' değeri hedef dosyada hiçbir satırla \
                     eşleşmiyor — bu çeviriler uygulanmaz. Örnek: {}.",
                    sample.join(", ")
                ),
                "field_value değerlerini hedef alanın gerçek içeriğiyle birebir eşleştirin \
                 (baştaki/sondaki boşluk ve büyük/küçük harf dahil).",
            ));
        }
    }

    // XFL_014:�?eviri yapılan kayıt silinmiş veya tanımsız (dangling translation feed özeti)
    {
        let mut bad_keys: HashSet<String> = HashSet::new();
        let attribution_ids = attribution_id_set(records);
        for rec in &records.translations {
            if let Some(ref rid) = rec.record_id {
                if rid.is_empty() {
                    continue;
                }
                let exists = translation_record_exists(
                    map, &attribution_ids, &rec.table_name, rid.as_str(),
                ).unwrap_or(true);
                if !exists {
                    bad_keys.insert(format!("{}:{}", rec.table_name, rid));
                }
            }
        }
        if !bad_keys.is_empty() {
            let mut bad_list: Vec<&str> = bad_keys.iter().map(String::as_str).collect();
            bad_list.sort_unstable();
            notices.push(notice(
                ctr,
                "XFL_014",
                EntityType::Feed,
                None,
                None,
                "translations.txt",
                None,
                Some("record_id"),
                Some(bad_list.join(", ")),
                None,
                format!(
                    "translations.txt'te silinmiş veya tanımsız kayıtlara referans veren çeviriler: {}.",
                    bad_list.join(", ")
                ),
                "İlgili çeviri satırlarını kaldırın veya eksik entity'leri tekrar ekleyin.",
            ));
        }
    }

    // XFL_015: Attribution referans hataları feed-level özet
    {
        let mut bad_refs: HashSet<String> = HashSet::new();
        for rec in &records.attributions {
            if let Some(ref aid) = rec.agency_id {
                if !map.agencies.contains_key(aid.as_str()) {
                    bad_refs.insert(format!("agency_id:{aid}"));
                }
            }
            if let Some(ref rid) = rec.route_id {
                if !map.routes.contains_key(rid.as_str()) {
                    bad_refs.insert(format!("route_id:{rid}"));
                }
            }
            if let Some(ref tid) = rec.trip_id {
                if !map.trips.contains_key(tid.as_str()) {
                    bad_refs.insert(format!("trip_id:{tid}"));
                }
            }
        }
        if !bad_refs.is_empty() {
            let mut bad_list: Vec<&str> = bad_refs.iter().map(String::as_str).collect();
            bad_list.sort_unstable();
            notices.push(notice(
                ctr,
                "XFL_015",
                EntityType::Feed,
                None,
                None,
                "attributions.txt",
                None,
                None,
                Some(bad_list.join(", ")),
                None,
                format!(
                    "attributions.txt'te geçersiz referanslar: {}.",
                    bad_list.join(", ")
                ),
                "attributions.txt'teki agency_id, route_id ve trip_id referanslarını düzeltin.",
            ));
        }
    }

    // XFL_016: translations table_name=feed_info varsa feed_info.txt zorunlu
    {
        let has_feed_info_trn = records
            .translations
            .iter()
            .any(|r| r.table_name == "feed_info");
        if has_feed_info_trn && records.feed_info.is_empty() {
            notices.push(notice(
                ctr,
                "XFL_016",
                EntityType::Feed,
                None,
                None,
                "translations.txt",
                None,
                Some("table_name"),
                Some("feed_info".to_string()),
                None,
                "translations.txt'te table_name='feed_info' kaydı var ama feed_info.txt dosyası yok.".to_string(),
                "feed_info.txt ekleyin veya ilgili çeviri satırını kaldırın.",
            ));
        }
    }

    // XFL_031: ortak kimlik isim alanı çakışması (GTFS-Flex).
    // Spec (location_groups.location_group_id): "ID must be unique across all stops.stop_id,
    // locations.geojson id, and location_groups.location_group_id values." Üç dosya TEK bir
    // isim alanı paylaşır. Çakışan bir kimlik `stop_times.location_id` /
    // `stop_times.location_group_id` referansını belirsiz bırakır: tüketici hangi varlığa
    // bakacağını bilemez. Her çakışan kimlik için TEK notice (kaynak çiftleri mesajda).
    {
        let stop_ids: std::collections::HashSet<&str> = records.stops.iter()
            .map(|s| s.stop_id.as_str())
            .filter(|s| !s.is_empty())
            .collect();
        // Determinizm: kümeler sırasız → çakışanları sıralı topla.
        let mut clashes: Vec<(String, &'static str)> = Vec::new();
        for lg in &records.location_groups {
            let id = lg.location_group_id.as_str();
            if id.is_empty() { continue; }
            if stop_ids.contains(id) {
                clashes.push((id.to_string(), "stops.stop_id"));
            } else if map.geojson_location_ids.contains(id) {
                clashes.push((id.to_string(), "locations.geojson id"));
            }
        }
        for gid in &map.geojson_location_ids {
            if stop_ids.contains(gid.as_str()) {
                clashes.push((gid.clone(), "stops.stop_id (locations.geojson id ile)"));
            }
        }
        clashes.sort();
        clashes.dedup();
        for (id, other) in clashes {
            notices.push(notice(
                ctr, "XFL_031", EntityType::Row,
                Some(id.clone()), Some(id.clone()),
                "location_groups.txt", None, Some("location_group_id"),
                Some(id.clone()), None,
                format!("'{id}' kimliği '{other}' ile çakışıyor — stop_id, locations.geojson id ve location_group_id ortak bir isim alanını paylaşır."),
                "Çakışan kimliklerden birini yeniden adlandırın; bu üç kaynak arasında her kimlik benzersiz olmalıdır.",
            ));
        }
    }

    // XFL_019: routes.network_id ile ayrı ağ dosyası birlikte kullanılmış → çakışma.
    // Spec (routes.network_id): "Conditionally Forbidden: Forbidden if the route_networks.txt
    // OR networks.txt file exists." Eskiden yalnız route_networks.txt kolu denetleniyordu;
    // networks.txt ile birlikte kullanım aynı derecede yasak olduğu hâlde kaçıyordu.
    if records.has_route_networks_file || records.has_networks_file {
        let has_route_network_id = records.routes.iter()
            .any(|r| r.network_id.as_deref().is_some_and(|n| !n.is_empty()));
        if has_route_network_id {
            let other = if records.has_route_networks_file {
                "route_networks.txt"
            } else {
                "networks.txt"
            };
            // ÇAPA `routes.txt`'tir, ayrı ağ dosyası DEĞİL (2026-08-01, WP-3): yasaklanan şey
            // `routes.network_id` sütunudur; networks.txt/route_networks.txt'in VARLIĞI
            // meşrudur. Notice'ı o dosyaya bağlamak kullanıcıyı düzeltilecek yerden başka
            // yere yolluyordu ve spec kapsam defterinde `routes.txt:network_id` hükmü
            // "ölçülmüyor" görünüyordu. Çakışan dosyanın adı mesajda zaten geçiyor.
            notices.push(notice(
                ctr,
                "XFL_019",
                EntityType::Feed,
                None,
                None,
                "routes.txt",
                None,
                Some("network_id"),
                None,
                None,
                format!("routes.txt'te network_id tanımlı VE {other} dosyası da mevcut — ağ tanımı iki yerde çakışıyor."),
                "Ağ atamasını yalnızca bir yöntemle yapın: ya routes.txt'teki network_id sütununu kullanın ya da ayrı ağ dosyasını (networks.txt / route_networks.txt) kullanın.",
            ));
        }
    }

    // XFL_017: route_cemv_support ile agency_cemv_support çelişiyor
    {
        for rec in &records.routes {
            if rec.route_id.is_empty() {
                continue;
            }
            if let (Some(rv), Some(ref aid)) = (rec.route_cemv_support, &rec.agency_id) {
                if let Some(&aidx) = map.agencies.get(aid.as_str()) {
                    if let Some(av) = records.agencies[aidx].agency_cemv_support {
                        if rv != av {
                            notices.push(notice(
                                ctr,
                                "XFL_017",
                                EntityType::Route,
                                Some(rec.route_id.clone()),
                                Some(rec.route_id.clone()),
                                "routes.txt",
                                Some(rec.line),
                                Some("cemv_support"),
                                Some(format!("route={rv}, agency={av}")),
                                None,
                                format!(
                                    "route_id '{}' route_cemv_support={rv} iken agency '{aid}' agency_cemv_support={av}; değerler çelişiyor.",
                                    rec.route_id
                                ),
                                "route_cemv_support ve agency_cemv_support değerlerini tutarlı hale getirin.",
                            ));
                        }
                    }
                }
            }
        }
    }
}

// ── STM_024: shape_dist_traveled birim tutarsızlığı ─────────────────────────

fn check_stm_shape_dist(
    records: &EntityRecords,
    notices: &mut Vec<Notice>,
    ctr: &mut u32,
) {
    let ti_stm = &records.trip_interns;
    // shape_id → o shape'teki max shape_dist_traveled
    let mut shape_max_dist: HashMap<&str, f64> = HashMap::new();
    for sp in &records.shapes {
        if let Some(d) = sp.shape_dist_traveled() {
            let e = shape_max_dist.entry(records.shape_interns.id(sp)).or_insert(0.0_f64);
            if d > *e {
                *e = d;
            }
        }
    }
    if shape_max_dist.is_empty() {
        return;
    }

    let mut flagged_shapes: HashSet<&str> = HashSet::new();

    for rec in &records.trips {
        if rec.trip_id.is_empty() {
            continue;
        }
        let shape_id = match ti_stm.shape_id(rec) {
            Some(s) if !s.is_empty() => s,
            _ => continue,
        };
        let shape_max = match shape_max_dist.get(shape_id) {
            Some(&d) if d > 0.0 => d,
            _ => continue,
        };
        if flagged_shapes.contains(shape_id) {
            continue;
        }

        let stm_max = records
            .stop_times_index
            .sorted_stops(&rec.trip_id)
            .map(|stops| {
                stops
                    .iter()
                    .filter_map(|s| s.shape_dist_traveled())
                    .fold(f64::NEG_INFINITY, f64::max)
            })
            .unwrap_or(f64::NEG_INFINITY);

        if stm_max <= 0.0 || stm_max.is_infinite() {
            continue;
        }

        let ratio = stm_max / shape_max;
        // Olası birim çakışmaları: ayak/metre ≈ 3.28×, metre/km ≈ 1000×, km/metre ≈ 0.001×
        if !(0.4..=2.5).contains(&ratio) {
            flagged_shapes.insert(shape_id);
            notices.push(notice(
                ctr,
                "STM_024",
                EntityType::Trip,
                Some(rec.trip_id.to_string()),
                Some(rec.trip_id.to_string()),
                "stop_times.txt",
                None,
                Some("shape_dist_traveled"),
                Some(format!("{stm_max:.2}")),
                Some(format!("{shape_max:.2}")),
                format!(
                    "'{}' seferinin stop_times.txt'teki shape_dist_traveled (max {stm_max:.2}) ile shape '{shape_id}' toplam mesafesi ({shape_max:.2}) arasındaki oran {ratio:.2}× — birim uyumsuzluğu (metre/ayak/km karışımı) olabilir.",
                    rec.trip_id
                ),
                "stop_times.txt ve shapes.txt'teki shape_dist_traveled değerlerinin aynı birimi (metre, ayak veya kilometre) kullandığını doğrulayın.",
            ));
        }
    }
}

fn station_context<'a>(
    stop_id: &'a str,
    stop_parent: &HashMap<&str, &'a str>,
    stop_loc: &HashMap<&str, Option<u32>>,
) -> Option<&'a str> {
    // parent_station zincirini TEPE istasyona kadar çöz. İç içe modellemede
    // (boarding area → platform → station) direkt parent ara-seviyedir; PTH_014
    // istasyon-sınırı kıyası tepe istasyonu kullanmalı, yoksa aynı istasyon
    // içindeki geçitler sahte KRİTİK üretir (#49, mdb-3127 Santiago Metro 966 FP).
    let mut cur = stop_id;
    for _ in 0..8 {
        match stop_parent.get(cur) {
            Some(&parent) => cur = parent, // bir seviye yukarı çık
            None => {
                if cur == stop_id {
                    // hiç parent yok: yalnızca istasyonun kendisi (lt=1) context üretir
                    return if stop_loc.get(cur).is_some_and(|t| *t == Some(1)) {
                        Some(cur)
                    } else {
                        None
                    };
                }
                return Some(cur); // zincirin tepesi (parent'ı olmayan en üst ata)
            }
        }
    }
    Some(cur) // döngü guard (bozuk/dairesel parent zinciri)
}

// ── STM_060: aynı seferde geojson bölgesi + pencere + davranış EŞZAMANLI örtüşmesi ──
//
// Spec (`stop_times.txt` düzyazısı): *"Simultaneous overlap of locations.geojson id
// geometry, start/end_pickup_drop_off_window time, and pickup_type or drop_off_type
// between two or more stop_times.txt records with the same trip_id is forbidden."*
//
// ÜÇ koşulun HEPSİ birden sağlanmalı — biri bile tutmazsa ihlal YOKTUR:
//   1. iki kaydın geojson bölgeleri UZAYDA kesişiyor
//   2. `[start, end)` pencereleri ZAMANDA kesişiyor
//   3. ikisi de biniş VEYA ikisi de iniş sunuyor (davranış örtüşmesi)
//
// ⚠️ Yalnız DIŞ ring'ler karşılaştırılır (delikler K1'de saklanmıyor). Delik yok saymak
// "örtüşüyor" kararını DAHA KAPSAYICI yapar, yani yanlış pozitif yönüne iter. Bu bir YASAK
// hükmü olduğu için o yön güvenli değil → örtüşme kararı bbox ön elemesinden sonra GERÇEK
// kesişme/kapsama ile verilir, sadece bbox'a bakılmaz. Delikli bir bölgede iki kaydın
// yalnız delik üzerinden "örtüşmesi" teorik bir yanlış pozitif olarak KAYITLIDIR.

/// İki dış ring UZAYDA örtüşüyor mu: kenarları kesişiyor ya da biri diğerini kapsıyor.
fn rings_overlap(a: &[(f64, f64)], b: &[(f64, f64)]) -> bool {
    if a.len() < 3 || b.len() < 3 {
        return false;
    }
    for i in 0..a.len() - 1 {
        for j in 0..b.len() - 1 {
            if crate::k1_parse::segments_cross(a[i], a[i + 1], b[j], b[j + 1]) {
                return true;
            }
        }
    }
    // Kesişme yoksa biri tamamen diğerinin içinde olabilir.
    crate::k1_parse::point_in_ring(a[0], b) || crate::k1_parse::point_in_ring(b[0], a)
}

fn geoms_overlap(
    g1: &crate::k1_parse::LocationGeometry,
    g2: &crate::k1_parse::LocationGeometry,
) -> bool {
    // bbox ön elemesi — ayrık kutularda O(n·m) taramayı hiç başlatma.
    let (a, b) = (g1.bbox, g2.bbox);
    if a.2 < b.0 || b.2 < a.0 || a.3 < b.1 || b.3 < a.1 {
        return false;
    }
    g1.outer_rings
        .iter()
        .any(|r1| g2.outer_rings.iter().any(|r2| rings_overlap(r1, r2)))
}

fn secs(t: (u32, u32, u32)) -> u32 {
    t.0 * 3600 + t.1 * 60 + t.2
}

fn check_flex_zone_overlap(
    records: &EntityRecords,
    map: &EntityMap,
    notices: &mut Vec<Notice>,
    ctr: &mut u32,
) {
    if map.geojson_geometries.is_empty() {
        return;
    }
    // ⚠️ `records.stop_times` DEĞİL, `stop_times_index` — #38 streaming'inden sonra Vec
    // ÜRETİMDE BOŞTUR (k4'ün başındaki yorum da bunu söylüyor). İlk uygulamam Vec'i
    // tarıyordu ve gerçek feed'de sessizce hiç ateşlemiyordu; testi yazınca çıktı.
    //
    // `flex_map` feed'lerin ~%99'unda boştur → o feed'lerde tarama hiç başlamaz.
    let idx = &records.stop_times_index;
    if idx.flex_map.is_empty() {
        return;
    }
    type FlexWindow<'a> = (&'a str, u32, u32, bool, bool, u64);
    type FlexWindowsByTrip<'a> = HashMap<&'a str, Vec<FlexWindow<'a>>>;
    let mut by_trip: FlexWindowsByTrip<'_> = HashMap::new();
    for (trip_id, stops) in idx.iter_trips() {
        for st in stops {
            let Some(f) = idx.flex_of(st) else { continue };
            let (Some(loc), Some(s), Some(e)) = (
                f.location_id.as_deref(),
                f.start_pickup_drop_off_window,
                f.end_pickup_drop_off_window,
            ) else {
                continue;
            };
            if !map.geojson_geometries.contains_key(loc) {
                continue; // tanımsız id → XFL_025'in alanı
            }
            // pickup_type/drop_off_type: 1 = "yok". Boş bırakılan alan hizmet SUNAR demektir.
            by_trip.entry(trip_id.as_str()).or_default().push((
                loc,
                secs(s),
                secs(e),
                f.pickup_type != Some(1),
                f.drop_off_type != Some(1),
                st.line_u64(),
            ));
        }
    }

    // HashMap gezilirken sıra nondeterministiktir → anahtarları sırala.
    let mut trips: Vec<&str> = by_trip.keys().copied().collect();
    trips.sort_unstable();
    for trip in trips {
        let rows = &by_trip[trip];
        if rows.len() < 2 {
            continue;
        }
        let mut found: Option<(&str, &str, u64)> = None;
        'pairs: for i in 0..rows.len() {
            for j in (i + 1)..rows.len() {
                let (l1, s1, e1, p1, d1, _) = rows[i];
                let (l2, s2, e2, p2, d2, line2) = rows[j];
                if s1 >= e2 || s2 >= e1 {
                    continue; // zaman penceresi kesişmiyor
                }
                if !((p1 && p2) || (d1 && d2)) {
                    continue; // davranış örtüşmesi yok
                }
                // 🔴 AYNI BÖLGE İHLAL DEĞİLDİR — spec'in KENDİSİ zorunlu kılıyor:
                // *"Travel within the same location group or GeoJSON location requires two
                // records in stop_times.txt with the same location_group_id or location_id."*
                // Aynı `location_id`'li iki kayıt bölge-İÇİ seyahatin TEK ifade biçimidir ve
                // onları ayırt edecek başka alan yoktur. İlk uygulamam bunu ihlal sayıyordu:
                // `ch-odv-flex`/`mdb-2053`'te 21 bulgu üretti, MD ise AYNI kuralı
                // (`overlapping_zone_and_pickup_drop_off_window`) uyguladığı hâlde SUSUYOR.
                // Spec'in bir yerde ZORUNLU kıldığını başka yerde yasak saymak `PTH_017`
                // hatasının aynısıdır → yalnız FARKLI bölgeler karşılaştırılır.
                if l1 == l2 {
                    continue;
                }
                if !geoms_overlap(&map.geojson_geometries[l1], &map.geojson_geometries[l2]) {
                    continue; // bölgeler uzayda ayrık
                }
                found = Some((l1, l2, line2));
                break 'pairs;
            }
        }
        if let Some((l1, l2, line)) = found {
            let zones = format!("'{l1}' ve '{l2}' bölgeleri");
            notices.push(notice(
                ctr, "STM_060", EntityType::Trip,
                Some(trip.to_string()), Some(trip.to_string()),
                "stop_times.txt", Some(line), Some("location_id"),
                Some(zones.clone()), None,
                format!(
                    "'{trip}' seferinde {zones} eşzamanlı örtüşüyor: geojson geometrileri kesişiyor, \
                     pickup/drop-off pencereleri çakışıyor ve ikisi de aynı hizmeti sunuyor."
                ),
                "Pencereleri ayırın, bölge geometrilerini çakışmayacak şekilde düzeltin ya da \
                 kayıtlardan birinde pickup/drop_off tipini kapatın.",
            ));
        }
    }
}

// �"?�"? Testler �"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?

#[cfg(test)]
mod tests {
    use super::*;
    use crate::k2::agency::AgencyRecord;
    use crate::k2::agency_jp::AgencyJpRecord;
    use crate::k2::attributions::AttributionRecord;
    use crate::k2::calendar::CalendarRecord;
    use crate::k2::fare_attributes::FareAttributeRecord;
    use crate::k2::fare_rules::FareRuleRecord;
    use crate::k2::feed_info::FeedInfoRecord;
    use crate::k2::frequencies::FrequencyRecord;
    use crate::k2::levels::LevelRecord;
    use crate::k2::office_jp::OfficeJpRecord;
    use crate::k2::pattern_jp::PatternJpRecord;
    use crate::k2::pathways::PathwayRecord;
    use crate::k2::routes::RouteRecord;
    use crate::k2::routes_jp::RoutesJpRecord;
    use crate::k2::stops::StopRecord;
    use crate::k2::stop_times::{StopTimeRecord, StopTimesIndex};
    use crate::k2::transfers::TransferRecord;
    use crate::k2::trips::{TripRecord, TripInternTable};
    use crate::k2::translations::TranslationRecord;
    use crate::k2::shapes::ShapeInternTable;

    fn empty() -> (EntityRecords, EntityMap) {
        // Bu yardımcı, aşağıdaki eski JPN_* fixture'larının V3 uzantı kurallarını
        // sınadığı sentetik feed'i temsil eder. Üretim varsayılanı V4 olduğu için
        // legacy testlerinin profil niyeti burada açıkça belirtilmelidir; V4 davranışı
        // ayrıca `v4_profile_treats_v3_extension_files_as_reference_only` ile sınanır.
        let records = EntityRecords {
            gtfs_jp_profile: GtfsJpProfile::V3,
            ..EntityRecords::default()
        };
        (records, EntityMap::default())
    }

    fn route(id: &str) -> RouteRecord {
        RouteRecord {
            route_id: id.into(),
            agency_id: None,
            route_short_name: None, route_long_name: None,
            route_desc: None, route_type: Some(3),
            route_url: None, route_color: None,
            route_text_color: None, route_sort_order: None,
            continuous_pickup: None, continuous_drop_off: None,
            network_id: None, route_cemv_support: None, jp_office_id: None,
            row: Default::default(), line: 2,
        }
    }

    /// Temel sefer kaydı oluşturur; rid/sid intern tablosuna eklenir (index 0 sentinel).
    fn trip(ti: &mut TripInternTable, id: &str, rid: &str, sid: &str) -> TripRecord {
        let ri = ti.route_ids.len() as u32;
        ti.route_ids.push(SmolStr::new(rid));
        let si = ti.service_ids.len() as u32;
        ti.service_ids.push(SmolStr::new(sid));
        TripRecord {
            trip_id: id.into(),
            route_idx: ri, service_idx: si, shape_idx: 0,
            headsign_idx: 0, short_name_idx: 0, block_idx: 0, jp_office_idx: 0, jp_pattern_idx: 0,
            direction_id: None, wheelchair_accessible: None,
            bikes_allowed: None, cars_allowed: None,
            safe_duration_factor: None, safe_duration_offset: None,
            line: 2,
        }
    }

    /// Shape ve yön içeren sefer kaydı oluşturur; intern tablosuna eklenir.
    fn trip_s(ti: &mut TripInternTable, id: &str, rid: &str, sid: &str, shape: &str, dir: Option<u32>) -> TripRecord {
        let mut t = trip(ti, id, rid, sid);
        t.direction_id = dir;
        if !shape.is_empty() {
            let shi = ti.shape_ids.len() as u32;
            ti.shape_ids.push(SmolStr::new(shape));
            t.shape_idx = shi;
        }
        t
    }

    fn stop(id: &str) -> StopRecord {
        StopRecord {
            stop_id: id.into(),
            stop_code: None, stop_name: None,
            stop_lat: None, stop_lon: None,
            location_type: None, stop_timezone: None,
            wheelchair_boarding: None, stop_access: None,
            level_id: None, tts_stop_name: None,
            row: Default::default(), line: 2,
            ..Default::default()
        }
    }

    // �"?�"? RTS_002 �"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?

    /// Yalnız boşluktan ibaret `parent_station` BOŞTUR — üç kural da bunu böyle okumalı.
    ///
    /// 🔴 Regresyon (7. koşumda ölçüldü): `STP_009`'un FK karşılaştırması ham değere
    /// çevrilirken aynı bloktaki üç `is_empty()` kontrolü atlandı. Ham okuyunca `" "`
    /// "dolu" sayıldı ve `tfs-535`'te `STP_036` 0 -> 14 oldu — arşivi bayt bayt aynı
    /// bir feed'de, yani katalog kaymasıyla açıklanamaz.
    ///
    /// İki ayrı soru, iki ayrı okuma: DOLULUK daima `trim()`, KİMLİK daima ham.
    #[test]
    fn whitespace_only_parent_station_counts_as_absent() {
        let (mut recs, mut map) = empty();
        map.stops.insert("STATION".into(), 0);
        let mut station = stop("STATION");
        station.location_type = Some(1);
        station.row.insert("parent_station".to_string(), " ".to_string());
        recs.stops = vec![station];
        let result = check(&recs, &map, 20260515);
        assert!(!result.notices.iter().any(|n| n.rule_id == "STP_036"),
            "yalnız boşluk taşıyan parent_station DOLU sayılmamalı: {:?}",
            result.notices.iter().map(|n| &n.rule_id).collect::<Vec<_>>());
        assert!(!result.notices.iter().any(|n| n.rule_id == "STP_009"),
            "boş parent için FK ihlali de üretilmemeli");
    }

    /// Gerçekten dolu bir parent_station'da STP_036 hâlâ ateşlemeli — kapı fazla geniş olmasın.
    #[test]
    fn station_with_real_parent_still_produces_stp_036() {
        let (mut recs, mut map) = empty();
        map.stops.insert("STATION".into(), 0);
        map.stops.insert("PARENT".into(), 1);
        let mut station = stop("STATION");
        station.location_type = Some(1);
        station.row.insert("parent_station".to_string(), "PARENT".to_string());
        let mut parent = stop("PARENT");
        parent.location_type = Some(1);
        // İki kayıt da `recs.stops`'ta olmalı: `map.stops` indeksleri bu vektöre bakar.
        recs.stops = vec![station, parent];
        let result = check(&recs, &map, 20260515);
        assert!(result.notices.iter().any(|n| n.rule_id == "STP_036"),
            "istasyonun GERÇEK parent'ı varsa STP_036 ateşlemeli: {:?}",
            result.notices.iter().map(|n| &n.rule_id).collect::<Vec<_>>());
    }

    #[test]
    fn invalid_agency_ref_in_route_produces_rts_002() {
        let (mut recs, mut map) = empty();
        map.routes.insert("R1".into(), 0);
        recs.routes = vec![RouteRecord {
            agency_id: Some("NONEXISTENT".into()),
            ..route("R1")
        }];
        let result = check(&recs, &map, 20260515);
        assert!(result.notices.iter().any(|n| n.rule_id == "RTS_002"));
    }

    #[test]
    fn xfl_013_lists_routes_and_services_per_direction() {
        let (mut recs, map) = empty();
        let mut ti = TripInternTable::new();
        recs.trips = vec![
            trip_s(&mut ti, "T1", "R1", "WD",  "S1", Some(0)),
            trip_s(&mut ti, "T2", "R2", "WD",  "S1", Some(0)),
            trip_s(&mut ti, "T3", "R1", "SAT", "S1", Some(1)),
        ];
        recs.trip_interns = ti;
        let result = check(&recs, &map, 20260515);
        let xfl: Vec<_> = result.notices.iter().filter(|n| n.rule_id == "XFL_013").collect();
        assert_eq!(xfl.len(), 1, "tek XFL_013 bekleniyor: {:?}",
            xfl.iter().map(|n| &n.message).collect::<Vec<_>>());
        let m = &xfl[0].message;
        assert!(m.contains("R1") && m.contains("R2"), "gidiş iki hattı listelemeli: {m}");
        assert!(m.contains("WD") && m.contains("SAT"), "takvimler listelenmeli: {m}");
        assert!(m.contains("Gidiş") && m.contains("Dönüş"), "yön etiketleri olmalı: {m}");
        let d = xfl[0].details.as_ref().expect("details olmalı");
        assert_eq!(d.get("fwd_routes").map(String::as_str), Some("R1, R2"));
        assert_eq!(d.get("bwd_routes").map(String::as_str), Some("R1"));
        assert_eq!(d.get("bwd_services").map(String::as_str), Some("SAT"));
    }

    // ── BKR_015: prior_notice_service_id → calendar/calendar_dates ──────────────

    fn booking_rule(id: &str, btype: Option<u8>, svc: Option<&str>) -> crate::k2::booking_rules::BookingRuleRecord {
        crate::k2::booking_rules::BookingRuleRecord {
            booking_rule_id: id.into(),
            booking_type: btype,
            prior_notice_duration_min: None,
            prior_notice_duration_max: None,
            prior_notice_last_day: None,
            prior_notice_last_time: None,
            prior_notice_start_day: None,
            prior_notice_start_time: None,
            prior_notice_service_id: svc.map(str::to_string),
            row: Default::default(),
            line: 2,
        }
    }

    /// stop_times index'i tek Flex satırdan kurar (pickup/drop_off booking_rule_id ile).
    fn stm_index_with_booking_rules(pickup: Option<&str>, drop_off: Option<&str>) -> crate::k2::stop_times::StopTimesIndex {
        use crate::k2::stop_times::{StopTimeRecord, StopTimesIndex};
        StopTimesIndex::from_records(&[StopTimeRecord {
            trip_id: "T1".into(),
            stop_sequence: Some(1),
            start_pickup_drop_off_window: Some((9, 0, 0)),
            end_pickup_drop_off_window: Some((17, 0, 0)),
            location_id: Some("Z1".into()),
            pickup_booking_rule_id: pickup.map(Into::into),
            drop_off_booking_rule_id: drop_off.map(Into::into),
            line: 2,
            ..Default::default()
        }])
    }

    #[test]
    fn unknown_pickup_booking_rule_produces_bkr_017() {
        let (mut recs, map) = empty();
        recs.booking_rules = vec![booking_rule("BR1", Some(2), None)];
        recs.stop_times_index = stm_index_with_booking_rules(Some("MISSING_BR"), None);
        let result = check(&recs, &map, 20260515);
        let n = result.notices.iter().find(|n| n.rule_id == "BKR_017").expect("BKR_017 bekleniyor");
        assert_eq!(n.file.as_deref(), Some("stop_times.txt"));
        assert_eq!(n.field.as_deref(), Some("pickup_booking_rule_id"));
    }

    #[test]
    fn unknown_drop_off_booking_rule_produces_bkr_018() {
        let (mut recs, map) = empty();
        recs.booking_rules = vec![booking_rule("BR1", Some(2), None)];
        recs.stop_times_index = stm_index_with_booking_rules(None, Some("MISSING_BR"));
        let result = check(&recs, &map, 20260515);
        assert!(result.notices.iter().any(|n| n.rule_id == "BKR_018"
            && n.field.as_deref() == Some("drop_off_booking_rule_id")), "BKR_018 bekleniyor");
    }

    #[test]
    fn known_booking_rule_silent_for_bkr_017_018() {
        let (mut recs, map) = empty();
        recs.booking_rules = vec![booking_rule("BR1", Some(2), None)];
        recs.stop_times_index = stm_index_with_booking_rules(Some("BR1"), Some("BR1"));
        let result = check(&recs, &map, 20260515);
        assert!(!result.notices.iter().any(|n| n.rule_id == "BKR_017" || n.rule_id == "BKR_018"),
            "tanımlı booking_rule_id → notice yok");
    }

    #[test]
    fn unknown_prior_notice_service_produces_bkr_015() {
        let (mut recs, map) = empty();
        recs.booking_rules = vec![booking_rule("BR1", Some(2), Some("MISSING_SVC"))];
        let result = check(&recs, &map, 20260515);
        let n = result.notices.iter().find(|n| n.rule_id == "BKR_015")
            .expect("BKR_015 bekleniyor");
        assert_eq!(n.file.as_deref(), Some("booking_rules.txt"));
        assert_eq!(n.field.as_deref(), Some("prior_notice_service_id"));
    }

    #[test]
    fn known_prior_notice_service_silent_for_bkr_015() {
        let (mut recs, mut map) = empty();
        map.services.insert("SVC1".to_string());
        recs.booking_rules = vec![booking_rule("BR1", Some(2), Some("SVC1"))];
        let result = check(&recs, &map, 20260515);
        assert!(!result.notices.iter().any(|n| n.rule_id == "BKR_015"),
            "tanımlı service_id → BKR_015 üretilmemeli");
    }

    #[test]
    fn bkr_015_defers_to_bkr_014_when_field_is_forbidden() {
        // booking_type=1'de alan zaten yasak (BKR_014, K2) → aynı alan için ikinci notice yok.
        let (mut recs, map) = empty();
        recs.booking_rules = vec![booking_rule("BR1", Some(1), Some("MISSING_SVC"))];
        let result = check(&recs, &map, 20260515);
        assert!(!result.notices.iter().any(|n| n.rule_id == "BKR_015"),
            "type=1'de BKR_015 susmalı (BKR_014 kapsıyor)");
    }

    #[test]
    fn bkr_015_checks_rows_with_unreadable_booking_type() {
        // booking_type okunamıyorsa BKR_014 de susar → referans denetimsiz kalmasın.
        let (mut recs, map) = empty();
        recs.booking_rules = vec![booking_rule("BR1", None, Some("MISSING_SVC"))];
        let result = check(&recs, &map, 20260515);
        assert!(result.notices.iter().any(|n| n.rule_id == "BKR_015"),
            "booking_type okunamayan satırda BKR_015 çalışmalı");
    }

    #[test]
    fn xfl_013_silent_for_single_direction() {
        let (mut recs, map) = empty();
        let mut ti = TripInternTable::new();
        recs.trips = vec![
            trip_s(&mut ti, "T1", "R1", "WD", "S1", Some(0)),
            trip_s(&mut ti, "T2", "R1", "WD", "S1", Some(0)),
        ];
        recs.trip_interns = ti;
        let result = check(&recs, &map, 20260515);
        assert!(!result.notices.iter().any(|n| n.rule_id == "XFL_013"),
            "tek yön → XFL_013 üretilmemeli");
    }

    // �"?�"? TRP_002 �"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?

    #[test]
    fn invalid_route_ref_in_trip_produces_trp_002() {
        let (mut recs, map) = empty();
        let mut ti = TripInternTable::new();
        recs.trips = vec![trip(&mut ti, "T1", "MISSING_ROUTE", "SVC1")];
        recs.trip_interns = ti;
        let result = check(&recs, &map, 20260515);
        assert!(result.notices.iter().any(|n| n.rule_id == "TRP_002"));
    }

    // �"?�"? TRP_003 �"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?

    #[test]
    fn invalid_service_ref_in_trip_produces_trp_003() {
        let (mut recs, mut map) = empty();
        map.routes.insert("R1".into(), 0);
        let mut ti = TripInternTable::new();
        recs.trips = vec![trip(&mut ti, "T1", "R1", "MISSING_SERVICE")];
        recs.trip_interns = ti;
        let result = check(&recs, &map, 20260515);
        assert!(result.notices.iter().any(|n| n.rule_id == "TRP_003"));
    }

    // �"?�"? TRP_004 �"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?

    #[test]
    fn invalid_shape_ref_in_trip_produces_trp_004() {
        let (mut recs, mut map) = empty();
        map.routes.insert("R1".into(), 0);
        map.services.insert("SVC1".into());
        let mut ti = TripInternTable::new();
        recs.trips = vec![trip_s(&mut ti, "T1", "R1", "SVC1", "MISSING_SHAPE", None)];
        recs.trip_interns = ti;
        let result = check(&recs, &map, 20260515);
        assert!(result.notices.iter().any(|n| n.rule_id == "TRP_004"));
    }

    // �"?�"? STM_001 �"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?

    #[test]
    fn invalid_trip_ref_in_stop_times_produces_stm_001() {
        let (mut recs, map) = empty();
        recs.stop_times = vec![StopTimeRecord {
            trip_id: "MISSING".into(),
            stop_id: "S1".into(),
            stop_sequence: Some(1),
            line: 2,
            ..Default::default()
        }];
        recs.stop_times_index = StopTimesIndex::from_records(&recs.stop_times);
        let result = check(&recs, &map, 20260515);
        assert!(result.notices.iter().any(|n| n.rule_id == "STM_001"));
    }

    // �"?�"? STM_002 �"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?

    #[test]
    fn invalid_stop_ref_in_stop_times_produces_stm_002() {
        let (mut recs, mut map) = empty();
        map.trips.insert("T1".into(), 0);
        recs.stop_times = vec![StopTimeRecord {
            trip_id: "T1".into(),
            stop_id: "MISSING_STOP".into(),
            stop_sequence: Some(1),
            line: 2,
            ..Default::default()
        }];
        recs.stop_times_index = StopTimesIndex::from_records(&recs.stop_times);
        let result = check(&recs, &map, 20260515);
        assert!(result.notices.iter().any(|n| n.rule_id == "STM_002"));
    }

    // �"?�"? RTS_012 �"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?

    #[test]
    fn route_with_no_trips_produces_rts_012() {
        let (mut recs, mut map) = empty();
        map.routes.insert("R1".into(), 0);
        recs.routes = vec![route("R1")];
        // trips bo�Y �?' rota kullanılmıyor
        let result = check(&recs, &map, 20260515);
        assert!(result.notices.iter().any(|n| n.rule_id == "RTS_012"));
    }

    // �"?�"? CAL_011 �"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?

    #[test]
    fn unused_service_produces_cal_011() {
        let (mut recs, _map) = empty();
        recs.calendars = vec![CalendarRecord {
            service_id: "UNUSED".into(),
            days: [Some(1); 7],
            start_date: None, end_date: None,
            row: Default::default(), line: 2,
        }];
        // trips bo�Y �?' servis kullanılmıyor
        let result = check(&recs, &EntityMap::default(), 20260515);
        assert!(result.notices.iter().any(|n| n.rule_id == "CAL_011"));
    }

    // �"?�"? FRQ_001 �"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?

    #[test]
    fn invalid_trip_ref_in_frequencies_produces_frq_001() {
        let (mut recs, map) = empty();
        recs.frequencies = vec![FrequencyRecord {
            trip_id: "MISSING".into(),
            start_time: None, end_time: None,
            headway_secs: Some(600), exact_times: None,
            row: Default::default(), line: 2,
        }];
        let result = check(&recs, &map, 20260515);
        assert!(result.notices.iter().any(|n| n.rule_id == "FRQ_001"));
    }

    // �"?�"? TRF_006 �"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?

    #[test]
    fn invalid_from_trip_in_transfer_produces_trf_006() {
        let (mut recs, map) = empty();
        recs.transfers = vec![TransferRecord {
            from_stop_id: "S1".into(), to_stop_id: "S2".into(),
            transfer_type: Some(3),
            min_transfer_time: None,
            from_trip_id: Some("MISSING_TRIP".into()),
            to_trip_id: None, from_route_id: None, to_route_id: None,
            row: Default::default(), line: 2,
        }];
        let result = check(&recs, &map, 20260515);
        assert!(result.notices.iter().any(|n| n.rule_id == "TRF_006"));
    }

    #[test]
    fn transfer_time_of_24_hours_produces_trf_010() {
        let (mut recs, map) = empty();
        recs.transfers = vec![TransferRecord {
            from_stop_id: "S1".into(), to_stop_id: "S2".into(),
            transfer_type: Some(2), min_transfer_time: Some(86_400),
            from_trip_id: None, to_trip_id: None, from_route_id: None, to_route_id: None,
            row: Default::default(), line: 2,
        }];
        let result = check(&recs, &map, 20260515);
        assert!(result.notices.iter().any(|n| n.rule_id == "TRF_010"));
    }
    #[test]
    fn transfer_walking_speed_over_two_mps_produces_trf_020() {
        let (mut recs, mut map) = empty();
        let mut a = stop("S1"); a.stop_lat = Some(41.0); a.stop_lon = Some(29.0);
        let mut b = stop("S2"); b.stop_lat = Some(41.01); b.stop_lon = Some(29.0);
        recs.stops = vec![a, b]; map.stops.insert("S1".into(), 0); map.stops.insert("S2".into(), 1);
        recs.transfers = vec![TransferRecord { from_stop_id: "S1".into(), to_stop_id: "S2".into(),
            transfer_type: Some(2), min_transfer_time: Some(60), from_trip_id: None, to_trip_id: None,
            from_route_id: None, to_route_id: None, row: Default::default(), line: 2 }];
        let result = check(&recs, &map, 20260515);
        assert!(result.notices.iter().any(|n| n.rule_id == "TRF_020"));
    }

    // �"?�"? TRF_013 �"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?

    #[test]
    fn type4_transfer_missing_trip_ids_produces_trf_013() {
        let (mut recs, map) = empty();
        recs.transfers = vec![TransferRecord {
            from_stop_id: "S1".into(), to_stop_id: "S2".into(),
            transfer_type: Some(4),
            min_transfer_time: None,
            from_trip_id: None, // eksik
            to_trip_id: None,   // eksik
            from_route_id: None, to_route_id: None,
            row: Default::default(), line: 2,
        }];
        let result = check(&recs, &map, 20260515);
        assert!(result.notices.iter().any(|n| n.rule_id == "TRF_013"));
    }

    // �"?�"? FAR_008 �"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?

    #[test]
    fn invalid_agency_ref_in_fare_produces_far_008() {
        let (mut recs, map) = empty();
        recs.fare_attributes = vec![FareAttributeRecord {
            fare_id: "F1".into(),
            price: Some(2.0),
            currency_type: "TRY".into(),
            payment_method: Some(0),
            transfers: None, transfer_duration: None,
            agency_id: Some("MISSING_AGENCY".into()),
            row: Default::default(), line: 2,
        }];
        let result = check(&recs, &map, 20260515);
        assert!(result.notices.iter().any(|n| n.rule_id == "FAR_008"));
    }

    // �"?�"? FRL_001 �"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?

    #[test]
    fn invalid_fare_ref_in_fare_rules_produces_frl_001() {
        let (mut recs, map) = empty();
        recs.fare_rules = vec![FareRuleRecord {
            fare_id: "MISSING_FARE".into(),
            route_id: None, origin_id: None,
            destination_id: None, contains_id: None,
            row: Default::default(), line: 2,
        }];
        let result = check(&recs, &map, 20260515);
        assert!(result.notices.iter().any(|n| n.rule_id == "FRL_001"));
    }

    /// Aynı tanımsız fare_id yüzlerce satırda geçse de DISTINCT kimlik başına TEK notice.
    /// Gerçek vaka (mdb-2837): fare_attributes.txt hiç yok + 1.128.840 fare_rules satırı →
    /// satır-başına emit 1,1M notice ve 178 sn üretiyordu; distinct'e çevrilince 486 ve 2,8 sn.
    #[test]
    fn repeated_invalid_fare_ref_produces_one_frl_001_per_distinct_id() {
        let (mut recs, map) = empty();
        recs.fare_rules = (0..50)
            .map(|i| FareRuleRecord {
                fare_id: if i % 2 == 0 { "MISSING_A".into() } else { "MISSING_B".into() },
                route_id: None, origin_id: None,
                destination_id: None, contains_id: None,
                row: Default::default(), line: (i + 2) as u64,
            })
            .collect();
        let result = check(&recs, &map, 20260515);
        let hits: Vec<_> = result.notices.iter().filter(|n| n.rule_id == "FRL_001").collect();
        assert_eq!(hits.len(), 2, "2 distinct fare_id → 2 notice, alınan: {}", hits.len());
        // Satır sayısı bilgisi korunmalı (25'er kez tekrar ediyor)
        assert!(hits.iter().all(|n| n.message.contains("25 fare_rules")),
            "mesaj kaç satırda tekrarladığını taşımalı: {:?}",
            hits.iter().map(|n| &n.message).collect::<Vec<_>>());
    }

    /// Tanımsız zone_id de distinct başına tek notice (FRL_003/004).
    #[test]
    fn repeated_invalid_zone_ref_produces_one_notice_per_distinct_zone() {
        let (mut recs, mut map) = empty();
        map.fare_attrs.insert("F1".into(), Default::default());
        recs.fare_rules = (0..30)
            .map(|i| FareRuleRecord {
                fare_id: "F1".into(),
                route_id: None,
                origin_id: Some("NO_ZONE".into()),
                destination_id: Some("NO_ZONE".into()),
                contains_id: None,
                row: Default::default(), line: (i + 2) as u64,
            })
            .collect();
        let result = check(&recs, &map, 20260515);
        assert_eq!(result.notices.iter().filter(|n| n.rule_id == "FRL_003").count(), 1);
        assert_eq!(result.notices.iter().filter(|n| n.rule_id == "FRL_004").count(), 1);
    }

    // �"?�"? LVL_004 �"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?

    #[test]
    fn unreferenced_level_produces_lvl_004() {
        let (mut recs, _map) = empty();
        recs.levels = vec![LevelRecord {
            level_id: "L1".into(),
            level_index: Some(0.0), level_name: None,
            row: Default::default(), line: 2,
        }];
        // stops bo�Y �?' level kullanılmıyor
        let result = check(&recs, &EntityMap::default(), 20260515);
        assert!(result.notices.iter().any(|n| n.rule_id == "LVL_004"));
    }

    // �"?�"? ATR_009 �"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?

    #[test]
    fn attribution_with_multiple_refs_produces_atr_009() {
        let (mut recs, _map) = empty();
        recs.attributions = vec![AttributionRecord {
            attribution_id: None,
            organization_name: "Org".into(),
            is_producer: Some(1), is_operator: None, is_authority: None,
            agency_id: Some("A1".into()),
            route_id: Some("R1".into()), // hem agency hem route → hata
            trip_id: None,
            attribution_url: None, attribution_email: None,
            row: Default::default(), line: 2,
        }];
        let result = check(&recs, &EntityMap::default(), 20260515);
        assert!(result.notices.iter().any(|n| n.rule_id == "ATR_009"));
    }

    // �"?�"? XFL_002 �"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?

    #[test]
    fn trip_without_stop_times_produces_xfl_002() {
        let (mut recs, mut map) = empty();
        map.trips.insert("T1".into(), 0);
        let mut ti = TripInternTable::new();
        recs.trips = vec![trip(&mut ti, "T1", "R1", "SVC1")];
        recs.trip_interns = ti;
        // stop_times bo�Y �?' T1 için kayıt yok
        let result = check(&recs, &map, 20260515);
        assert!(result.notices.iter().any(|n| n.rule_id == "XFL_002"));
    }

    // �"?�"? XFL_008 �"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?

    #[test]
    fn pathway_with_missing_stop_produces_pth_002() {
        let (mut recs, map) = empty();
        recs.pathways = vec![PathwayRecord {
            pathway_id: "P1".into(),
            from_stop_id: "MISSING".into(),
            to_stop_id: "S2".into(),
            pathway_mode: Some(1), is_bidirectional: Some(1),
            length: None, traversal_time: None,
            stair_count: None, max_slope: None, min_width: None,
            row: Default::default(), line: 2,
        }];
        let result = check(&recs, &map, 20260515);
        assert!(result.notices.iter().any(|n| n.rule_id == "PTH_002"));
    }

    fn stop_ctx(id: &str, lt: Option<u32>, parent: &str) -> StopRecord {
        let mut s = stop(id);
        s.location_type = lt;
        if !parent.is_empty() {
            s.row.insert("parent_station".to_string(), parent.to_string());
        }
        s
    }

    #[test]
    fn pth_014_nested_station_no_false_positive() {
        // İç içe modelleme (#49): station ST → platform PL(parent=ST) →
        // boarding BA(parent=PL); concourse CN(parent=ST). CN→BA geçidi AYNI
        // istasyon içindedir; station_context tepe istasyona (ST) çözmeli → PTH_014 ÇIKMAMALI.
        let (mut recs, map) = empty();
        recs.stops = vec![
            stop_ctx("ST", Some(1), ""),   // istasyon
            stop_ctx("PL", Some(0), "ST"), // peron (parent=istasyon)
            stop_ctx("BA", Some(4), "PL"), // boarding area (parent=peron) — 2 seviye
            stop_ctx("CN", Some(3), "ST"), // generic node (parent=istasyon)
        ];
        recs.pathways = vec![PathwayRecord {
            pathway_id: "P1".into(),
            from_stop_id: "CN".into(),
            to_stop_id: "BA".into(),
            pathway_mode: Some(1), is_bidirectional: Some(1),
            length: None, traversal_time: None,
            stair_count: None, max_slope: None, min_width: None,
            row: Default::default(), line: 2,
        }];
        let result = check(&recs, &map, 20260515);
        assert!(
            !result.notices.iter().any(|n| n.rule_id == "PTH_014"),
            "iç içe istasyonda PTH_014 sahte-pozitif çıkmamalı"
        );
    }

    #[test]
    fn pth_014_genuine_cross_station_fires() {
        // Gerçek sınır ihlali: PA(parent=A) ↔ PB(parent=B), A≠B → PTH_014 ÇIKMALI.
        let (mut recs, map) = empty();
        recs.stops = vec![
            stop_ctx("A", Some(1), ""),
            stop_ctx("B", Some(1), ""),
            stop_ctx("PA", Some(0), "A"),
            stop_ctx("PB", Some(0), "B"),
        ];
        recs.pathways = vec![PathwayRecord {
            pathway_id: "P1".into(),
            from_stop_id: "PA".into(),
            to_stop_id: "PB".into(),
            pathway_mode: Some(1), is_bidirectional: Some(1),
            length: None, traversal_time: None,
            stair_count: None, max_slope: None, min_width: None,
            row: Default::default(), line: 2,
        }];
        let result = check(&recs, &map, 20260515);
        assert!(
            result.notices.iter().any(|n| n.rule_id == "PTH_014"),
            "gerçek cross-station geçidinde PTH_014 çıkmalı"
        );
    }

    /// #187 · MBTA `dwnxg-385/386` (Winter Street Concourse, Park St ↔ Downtown
    /// Crossing): iki uç da generic node (lt=3), biri KOORDİNATSIZ — GTFS'te
    /// generic node koordinatı opsiyoneldir. Beyan edilen `length` 142,34 m,
    /// 300 m guard'ının çok altında → PTH_014 ÇIKMAMALI.
    #[test]
    fn pth_014_short_declared_length_without_coordinate_is_suppressed() {
        let (mut recs, map) = empty();
        let mut with_coord = stop_ctx("N_A", Some(3), "A");
        with_coord.stop_lat = Some(42.3564);
        with_coord.stop_lon = Some(-71.0624);
        recs.stops = vec![
            stop_ctx("A", Some(1), ""),
            stop_ctx("B", Some(1), ""),
            with_coord,
            stop_ctx("N_B", Some(3), "B"), // koordinatsız generic node
        ];
        recs.pathways = vec![PathwayRecord {
            pathway_id: "dwnxg-385".into(),
            from_stop_id: "N_A".into(),
            to_stop_id: "N_B".into(),
            pathway_mode: Some(1), is_bidirectional: Some(0),
            length: Some(142.3416), traversal_time: None,
            stair_count: None, max_slope: None, min_width: None,
            row: Default::default(), line: 2,
        }];
        let result = check(&recs, &map, 20260515);
        assert!(
            !result.notices.iter().any(|n| n.rule_id == "PTH_014"),
            "koordinatsız uçta kısa beyan edilen length PTH_014'ü bastırmalı: {:?}",
            result.notices.iter().filter(|n| n.rule_id == "PTH_014").collect::<Vec<_>>()
        );
    }

    /// Guard, `length` VARLIĞINA değil DEĞERİNE bakar: koordinat yokken uzun bir
    /// beyan hâlâ tetiklemeli, yoksa length yazmak kuralı kapatan bir anahtar olurdu.
    #[test]
    fn pth_014_long_declared_length_without_coordinate_fires() {
        let (mut recs, map) = empty();
        recs.stops = vec![
            stop_ctx("A", Some(1), ""),
            stop_ctx("B", Some(1), ""),
            stop_ctx("N_A", Some(3), "A"),
            stop_ctx("N_B", Some(3), "B"),
        ];
        recs.pathways = vec![PathwayRecord {
            pathway_id: "P1".into(),
            from_stop_id: "N_A".into(),
            to_stop_id: "N_B".into(),
            pathway_mode: Some(1), is_bidirectional: Some(0),
            length: Some(5_000.0), traversal_time: None,
            stair_count: None, max_slope: None, min_width: None,
            row: Default::default(), line: 2,
        }];
        let result = check(&recs, &map, 20260515);
        assert!(
            result.notices.iter().any(|n| n.rule_id == "PTH_014"),
            "5 km'lik beyan cross-station PTH_014'ü tetiklemeli"
        );
    }

    /// Öncelik kararı (#187): guard YÜRÜME mesafesini sorar. Koordinat farkı kuş
    /// uçuşudur, yürüme mesafesi değildir; geçidin kendi `length` beyanı varsa
    /// ölçtüğü şey doğrudan odur ve koordinatların önüne geçer.
    #[test]
    fn pth_014_declared_length_outranks_coordinates() {
        let (mut recs, map) = empty();
        let mut from = stop_ctx("N_A", Some(3), "A");
        from.stop_lat = Some(42.3564);
        from.stop_lon = Some(-71.0624);
        let mut to = stop_ctx("N_B", Some(3), "B");
        to.stop_lat = Some(42.4064); // ~5,5 km kuzey
        to.stop_lon = Some(-71.0624);
        recs.stops = vec![stop_ctx("A", Some(1), ""), stop_ctx("B", Some(1), ""), from, to];
        recs.pathways = vec![PathwayRecord {
            pathway_id: "P1".into(),
            from_stop_id: "N_A".into(),
            to_stop_id: "N_B".into(),
            pathway_mode: Some(1), is_bidirectional: Some(0),
            length: Some(10.0), traversal_time: None,
            stair_count: None, max_slope: None, min_width: None,
            row: Default::default(), line: 2,
        }];
        let result = check(&recs, &map, 20260515);
        assert!(
            !result.notices.iter().any(|n| n.rule_id == "PTH_014"),
            "geçerli kısa length beyanı, uzak koordinatlara rağmen PTH_014'ü bastırmalı: {:?}",
            result.notices.iter().filter(|n| n.rule_id == "PTH_014").collect::<Vec<_>>()
        );
    }

    /// PTH_006 negatif `length`'i raporlar ama değeri yine de döndürür; geçersiz
    /// beyan mesafe kanıtı sayılmamalı → kural normal yoluna devam eder.
    #[test]
    fn pth_014_negative_declared_length_is_not_distance_evidence() {
        let (mut recs, map) = empty();
        recs.stops = vec![
            stop_ctx("A", Some(1), ""),
            stop_ctx("B", Some(1), ""),
            stop_ctx("N_A", Some(3), "A"),
            stop_ctx("N_B", Some(3), "B"),
        ];
        recs.pathways = vec![PathwayRecord {
            pathway_id: "P1".into(),
            from_stop_id: "N_A".into(),
            to_stop_id: "N_B".into(),
            pathway_mode: Some(1), is_bidirectional: Some(0),
            length: Some(-1.0), traversal_time: None,
            stair_count: None, max_slope: None, min_width: None,
            row: Default::default(), line: 2,
        }];
        let result = check(&recs, &map, 20260515);
        assert!(
            result.notices.iter().any(|n| n.rule_id == "PTH_014"),
            "negatif length bastırma kanıtı olmamalı"
        );
    }

    #[test]
    fn pth_014_entrance_cross_station_suppressed() {
        // A-daraltma (VBB mdb-782): iki farklı istasyonun GİRİŞLERİ (lt=2) arası
        // pathway, sokak seviyesinde meşru bir aktarma yürüyüşüdür (Alexanderplatz
        // ↔ Memhardstr., Spandau ↔ Rathaus Spandau gibi) → PTH_014 ÇIKMAMALI.
        let (mut recs, map) = empty();
        recs.stops = vec![
            stop_ctx("A", Some(1), ""),
            stop_ctx("B", Some(1), ""),
            stop_ctx("EA", Some(2), "A"), // A istasyonunun girişi
            stop_ctx("EB", Some(2), "B"), // B istasyonunun girişi
        ];
        recs.pathways = vec![PathwayRecord {
            pathway_id: "P1".into(),
            from_stop_id: "EA".into(),
            to_stop_id: "EB".into(),
            pathway_mode: Some(1), is_bidirectional: Some(1),
            length: None, traversal_time: None,
            stair_count: None, max_slope: None, min_width: None,
            row: Default::default(), line: 2,
        }];
        let result = check(&recs, &map, 20260515);
        assert!(
            !result.notices.iter().any(|n| n.rule_id == "PTH_014"),
            "giriş↔giriş istasyon-arası pathway PTH_014 üretmemeli (meşru aktarma)"
        );
    }

    #[test]
    fn pth_014_short_distance_cross_station_suppressed() {
        // İç konum (peron) ↔ iç konum, farklı istasyon AMA uçlar ~110 m yakın
        // (VBB Potsdam "Platz der Einheit" alt-durakları deseni) → meşru yürüyüş,
        // PTH_014 ÇIKMAMALI (mesafe guard'ı).
        let (mut recs, map) = empty();
        let mut pa = stop_ctx("PA", Some(0), "A");
        pa.stop_lat = Some(52.4000);
        pa.stop_lon = Some(13.0000);
        let mut pb = stop_ctx("PB", Some(0), "B");
        pb.stop_lat = Some(52.4010); // ~111 m kuzey
        pb.stop_lon = Some(13.0000);
        recs.stops = vec![stop_ctx("A", Some(1), ""), stop_ctx("B", Some(1), ""), pa, pb];
        recs.pathways = vec![PathwayRecord {
            pathway_id: "P1".into(),
            from_stop_id: "PA".into(),
            to_stop_id: "PB".into(),
            pathway_mode: Some(1), is_bidirectional: Some(1),
            length: None, traversal_time: None,
            stair_count: None, max_slope: None, min_width: None,
            row: Default::default(), line: 2,
        }];
        let result = check(&recs, &map, 20260515);
        assert!(
            !result.notices.iter().any(|n| n.rule_id == "PTH_014"),
            "kısa mesafeli (~110 m) cross-station pathway PTH_014 üretmemeli"
        );
    }

    #[test]
    fn pth_014_far_distance_cross_station_fires() {
        // İç↔iç, farklı istasyon VE uçlar uzak (~5.5 km) → olası yanlış stop_id
        // referansı, PTH_014 ÇIKMALI (mesafe guard'ı bastırmamalı).
        let (mut recs, map) = empty();
        let mut pa = stop_ctx("PA", Some(0), "A");
        pa.stop_lat = Some(52.4000);
        pa.stop_lon = Some(13.0000);
        let mut pb = stop_ctx("PB", Some(0), "B");
        pb.stop_lat = Some(52.4500); // ~5.5 km kuzey
        pb.stop_lon = Some(13.0000);
        recs.stops = vec![stop_ctx("A", Some(1), ""), stop_ctx("B", Some(1), ""), pa, pb];
        recs.pathways = vec![PathwayRecord {
            pathway_id: "P1".into(),
            from_stop_id: "PA".into(),
            to_stop_id: "PB".into(),
            pathway_mode: Some(1), is_bidirectional: Some(1),
            length: None, traversal_time: None,
            stair_count: None, max_slope: None, min_width: None,
            row: Default::default(), line: 2,
        }];
        let result = check(&recs, &map, 20260515);
        assert!(
            result.notices.iter().any(|n| n.rule_id == "PTH_014"),
            "uzak (~5.5 km) cross-station pathway PTH_014 üretmeli"
        );
    }

    // �"?�"? XFL_009 �"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?

    #[test]
    fn stop_with_missing_level_produces_stp_015() {
        let (mut recs, _map) = empty();
        recs.stops = vec![StopRecord {
            level_id: Some("MISSING_LEVEL".into()),
            ..stop("S1")
        }];
        let result = check(&recs, &EntityMap::default(), 20260515);
        assert!(result.notices.iter().any(|n| n.rule_id == "STP_015"));
    }

    // �"?�"? AGN_005 �"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?

    fn agency_tz(id: &str, tz: &str) -> AgencyRecord {
        AgencyRecord {
            agency_id: Some(id.into()),
            agency_name: id.into(), agency_url: "https://a.com".into(),
            agency_timezone: tz.into(),
            agency_lang: None, agency_phone: None,
            agency_fare_url: None, agency_email: None,
            agency_cemv_support: None, row: Default::default(), line: 2,
        }
    }

    #[test]
    fn agn_005_ignores_an_empty_timezone() {
        // mdb-2946: 10 agency Europe/Kyiv, birinin dilimi boş. Bu bir dilim
        // ANLAŞMAZLIĞI değil eksik alandır ve AGN_004 zaten raporlar.
        let (mut recs, _map) = empty();
        recs.agencies = vec![
            agency_tz("A1", "Europe/Kyiv"),
            agency_tz("A2", "Europe/Kyiv"),
            agency_tz("A3", ""),
        ];
        let result = check(&recs, &EntityMap::default(), 20260515);
        assert!(!result.notices.iter().any(|n| n.rule_id == "AGN_005"));
    }

    #[test]
    fn agn_005_ignores_a_repeated_header_row() {
        // mdb-1003/mdb-1004: agency.txt'nin ortasında başlık satırı tekrarlanıyor,
        // hayalet agency'nin dilimi literal "agency_timezone" oluyor. Gerçekte
        // bütün agency'ler Europe/Madrid.
        let (mut recs, _map) = empty();
        recs.agencies = vec![
            agency_tz("A1", "Europe/Madrid"),
            agency_tz("agency_id", "agency_timezone"),
            agency_tz("A2", "Europe/Madrid"),
        ];
        let result = check(&recs, &EntityMap::default(), 20260515);
        assert!(!result.notices.iter().any(|n| n.rule_id == "AGN_005"));
    }

    #[test]
    fn agn_005_still_reports_a_real_disagreement() {
        let (mut recs, _map) = empty();
        recs.agencies = vec![
            agency_tz("A1", "Europe/Madrid"),
            agency_tz("A2", "America/New_York"),
        ];
        let result = check(&recs, &EntityMap::default(), 20260515);
        assert!(result.notices.iter().any(|n| n.rule_id == "AGN_005"));
    }

    #[test]
    fn agencies_with_different_timezones_produce_agn_005() {
        let (mut recs, _map) = empty();
        recs.agencies = vec![
            AgencyRecord {
                agency_id: Some("A1".into()),
                agency_name: "A".into(), agency_url: "https://a.com".into(),
                agency_timezone: "Europe/Istanbul".into(),
                agency_lang: None, agency_phone: None,
                agency_fare_url: None, agency_email: None,
                agency_cemv_support: None, row: Default::default(), line: 2,
            },
            AgencyRecord {
                agency_id: Some("A2".into()),
                agency_name: "B".into(), agency_url: "https://b.com".into(),
                agency_timezone: "America/New_York".into(),
                agency_lang: None, agency_phone: None,
                agency_fare_url: None, agency_email: None,
                agency_cemv_support: None, row: Default::default(), line: 3,
            },
        ];
        let result = check(&recs, &EntityMap::default(), 20260515);
        assert!(result.notices.iter().any(|n| n.rule_id == "AGN_005"));
    }

    // �"?�"? STP_011 �"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?

    #[test]
    fn entrance_without_parent_produces_stp_011() {
        let (mut recs, _map) = empty();
        recs.stops = vec![StopRecord {
            location_type: Some(2), // entrance → parent_station zorunlu
            ..stop("E1")
        }];
        let result = check(&recs, &EntityMap::default(), 20260515);
        assert!(result.notices.iter().any(|n| n.rule_id == "STP_011"));
    }

    // �"?�"? TRN_006 �"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?

    #[test]
    fn identical_translation_rows_produce_trn_005() {
        use crate::k2::translations::TranslationRecord;
        let (mut recs, _map) = empty();
        let base = TranslationRecord {
            table_name: "stops".into(), field_name: "stop_name".into(),
            language: "tr".into(), translation: "Durak".into(),
            record_id: Some("S1".into()),
            record_sub_id: None, field_value: None,
            row: Default::default(), line: 2,
        };
        // Aynı anahtar + aynı değer → birebir kopya → TRN_005
        recs.translations = vec![base.clone(), TranslationRecord { line: 3, ..base }];
        let result = check(&recs, &EntityMap::default(), 20260515);
        assert!(result.notices.iter().any(|n| n.rule_id == "TRN_005"));
        assert!(!result.notices.iter().any(|n| n.rule_id == "TRN_006"));
    }

    #[test]
    fn conflicting_translation_key_produces_trn_006() {
        use crate::k2::translations::TranslationRecord;
        let (mut recs, _map) = empty();
        let base = TranslationRecord {
            table_name: "stops".into(), field_name: "stop_name".into(),
            language: "tr".into(), translation: "Durak".into(),
            record_id: Some("S1".into()),
            record_sub_id: None, field_value: None,
            row: Default::default(), line: 2,
        };
        // Aynı anahtar + farklı değer → çelişki → TRN_006
        recs.translations = vec![
            base.clone(),
            TranslationRecord { line: 3, translation: "İstasyon".into(), ..base },
        ];
        let result = check(&recs, &EntityMap::default(), 20260515);
        assert!(result.notices.iter().any(|n| n.rule_id == "TRN_006"));
        assert!(!result.notices.iter().any(|n| n.rule_id == "TRN_005"));
    }

    #[test]
    fn missing_kana_produces_jpn_001() {
        use crate::k2::translations::TranslationRecord;
        let (mut recs, _map) = empty();
        let mut s1 = stop("S1"); s1.stop_name = Some("六本木".into());
        let mut s2 = stop("S2"); s2.stop_name = Some("赤坂".into());
        recs.stops = vec![s1, s2];
        // S1 için kana var, S2 için yok. ja-Hrkt varlığı GTFS-JP kapısını açar.
        recs.translations = vec![TranslationRecord {
            table_name: "stops".into(), field_name: "stop_name".into(),
            language: "ja-Hrkt".into(), translation: "ろっぽんぎ".into(),
            record_id: Some("S1".into()),
            record_sub_id: None, field_value: None,
            row: Default::default(), line: 2,
        }];
        let result = check(&recs, &EntityMap::default(), 20260515);
        let jpn: Vec<&str> = result.notices.iter()
            .filter(|n| n.rule_id == "JPN_001")
            .filter_map(|n| n.entity_id.as_deref())
            .collect();
        assert_eq!(jpn, vec!["S2"], "yalnız kanası eksik S2 işaretlenmeli");
    }

    #[test]
    fn missing_route_kana_produces_jpn_008() {
        use crate::k2::translations::TranslationRecord;
        let (mut recs, _map) = empty();
        let mut r1 = route("R1"); r1.route_long_name = Some("東京駅行".into());
        let mut r2 = route("R2"); r2.route_long_name = Some("渋谷行".into());
        recs.routes = vec![r1, r2];
        // R1 için kana var, R2 için yok. ja-Hrkt varlığı GTFS-JP kapısını açar.
        recs.translations = vec![TranslationRecord {
            table_name: "routes".into(), field_name: "route_long_name".into(),
            language: "ja-Hrkt".into(), translation: "とうきょうえきゆき".into(),
            record_id: Some("R1".into()), record_sub_id: None, field_value: None,
            row: Default::default(), line: 2,
        }];
        let result = check(&recs, &EntityMap::default(), 20260515);
        let jpn: Vec<&str> = result.notices.iter()
            .filter(|n| n.rule_id == "JPN_008")
            .filter_map(|n| n.entity_id.as_deref())
            .collect();
        assert_eq!(jpn, vec!["R2"], "yalnız kanası eksik R2 hattı işaretlenmeli");
    }

    #[test]
    fn japanese_route_short_name_without_kana_produces_jpn_008() {
        use crate::k2::translations::TranslationRecord;
        let (mut recs, _map) = empty();
        // Tokyo deseni: route_long_name BOŞ, ad route_short_name'de (Japonca)
        let mut r1 = route("R1"); r1.route_long_name = None; r1.route_short_name = Some("波０１".into());
        recs.routes = vec![r1];
        // Kapı: başka alanda ja-Hrkt çeviri; route_short_name kana yok → JPN_008
        recs.translations = vec![TranslationRecord {
            table_name: "stops".into(), field_name: "stop_name".into(),
            language: "ja-Hrkt".into(), translation: "x".into(),
            record_id: Some("S1".into()), record_sub_id: None, field_value: None,
            row: Default::default(), line: 2,
        }];
        let result = check(&recs, &EntityMap::default(), 20260515);
        assert!(result.notices.iter().any(|n| n.rule_id == "JPN_008"),
            "Japonca route_short_name kana yok → JPN_008");
    }

    #[test]
    fn numeric_route_short_name_no_jpn_008() {
        use crate::k2::translations::TranslationRecord;
        let (mut recs, _map) = empty();
        // route_long_name boş, route_short_name sayısal kod → Japonca değil → JPN_008 yok
        let mut r1 = route("R1"); r1.route_long_name = None; r1.route_short_name = Some("42".into());
        recs.routes = vec![r1];
        recs.translations = vec![TranslationRecord {
            table_name: "stops".into(), field_name: "stop_name".into(),
            language: "ja-Hrkt".into(), translation: "x".into(),
            record_id: Some("S1".into()), record_sub_id: None, field_value: None,
            row: Default::default(), line: 2,
        }];
        let result = check(&recs, &EntityMap::default(), 20260515);
        assert!(!result.notices.iter().any(|n| n.rule_id == "JPN_008"),
            "sayısal route_short_name → JPN_008 olmamalı");
    }

    #[test]
    fn missing_trip_headsign_kana_produces_jpn_009() {
        use crate::k2::translations::TranslationRecord;
        let (mut recs, _map) = empty();
        let mut ti = TripInternTable::new();
        let mut t1 = trip(&mut ti, "T1", "R1", "SVC1");
        let hi = ti.headsigns.len() as u32;
        ti.headsigns.push(SmolStr::new("東京行"));
        t1.headsign_idx = hi;
        recs.trips = vec![t1];
        recs.trip_interns = ti;
        // Kapı: başka alanda ja-Hrkt çeviri var; trip_headsign kana yok → JPN_009.
        recs.translations = vec![TranslationRecord {
            table_name: "stops".into(), field_name: "stop_name".into(),
            language: "ja-Hrkt".into(), translation: "x".into(),
            record_id: Some("S1".into()), record_sub_id: None, field_value: None,
            row: Default::default(), line: 2,
        }];
        let result = check(&recs, &EntityMap::default(), 20260515);
        assert!(result.notices.iter().any(|n| n.rule_id == "JPN_009"),
            "trip_headsign kana yok → JPN_009");
    }

    #[test]
    fn missing_agency_name_kana_produces_jpn_010() {
        use crate::k2::agency::AgencyRecord;
        use crate::k2::translations::TranslationRecord;
        let (mut recs, _map) = empty();
        recs.agencies = vec![AgencyRecord {
            agency_id: Some("A1".into()), agency_name: "都営バス".into(),
            agency_url: "https://example.jp".into(), agency_timezone: "Asia/Tokyo".into(),
            agency_lang: None, agency_phone: None, agency_fare_url: None,
            agency_email: None, agency_cemv_support: None,
            row: Default::default(), line: 2,
        }];
        // Kapı: başka alanda ja-Hrkt çeviri var; agency_name kana yok → JPN_010.
        recs.translations = vec![TranslationRecord {
            table_name: "stops".into(), field_name: "stop_name".into(),
            language: "ja-Hrkt".into(), translation: "x".into(),
            record_id: Some("S1".into()), record_sub_id: None, field_value: None,
            row: Default::default(), line: 2,
        }];
        let result = check(&recs, &EntityMap::default(), 20260515);
        assert!(result.notices.iter().any(|n| n.rule_id == "JPN_010"),
            "agency_name kana yok → JPN_010");
    }

    #[test]
    fn empty_gtfs_jp_file_inventory_activates_jp_profile() {
        let (recs, _map) = empty();
        for file in GTFS_JP_FILES {
            let present = HashSet::from([file.to_string()]);
            let availability = FileAvailability::from_k1(&present, &[]);
            let result = check_with_files(&recs, &EntityMap::default(), 20260515, &availability);
            assert!(result.notices.iter().any(|n| n.rule_id == "JPN_007"),
                "boş/başlıksız {file} dosyası GTFS-JP profilini açmalı");
        }
    }

    #[test]
    fn missing_agency_id_in_jp_feed_produces_jpn_011() {
        use crate::k2::agency::AgencyRecord;
        use crate::k2::office_jp::OfficeJpRecord;
        let mk_agency = |id: Option<&str>| AgencyRecord {
            agency_id: id.map(|s| s.into()), agency_name: "X".into(),
            agency_url: "https://example.jp".into(), agency_timezone: "Asia/Tokyo".into(),
            agency_lang: None, agency_phone: None, agency_fare_url: None,
            agency_email: None, agency_cemv_support: None, row: Default::default(), line: 2,
        };
        let mk_office = || OfficeJpRecord {
            office_id: "OF1".into(), office_name: Some("本社".into()),
            row: Default::default(), line: 2,
        };

        // is_gtfs_jp kapısı: office_jp mevcut. Tek agency, agency_id YOK → JPN_011.
        let (mut recs, _m) = empty();
        recs.office_jp = vec![mk_office()];
        recs.agencies = vec![mk_agency(None)];
        let r = check(&recs, &EntityMap::default(), 20260515);
        assert!(r.notices.iter().any(|n| n.rule_id == "JPN_011"), "JP + agency_id yok → JPN_011");

        // agency_id dolu → JPN_011 yok.
        let (mut recs2, _m2) = empty();
        recs2.office_jp = vec![mk_office()];
        recs2.agencies = vec![mk_agency(Some("A1"))];
        let r2 = check(&recs2, &EntityMap::default(), 20260515);
        assert!(!r2.notices.iter().any(|n| n.rule_id == "JPN_011"), "agency_id dolu → JPN_011 yok");

        // JP sinyali yok + agency_id yok → JPN_011 yok (standart feed'de opsiyonel).
        let (mut recs3, _m3) = empty();
        recs3.agencies = vec![mk_agency(None)];
        let r3 = check(&recs3, &EntityMap::default(), 20260515);
        assert!(!r3.notices.iter().any(|n| n.rule_id == "JPN_011"), "JP değil → JPN_011 yok");
    }

    #[test]
    fn inconsistent_agency_lang_produces_agn_017() {
        use crate::k2::agency::AgencyRecord;
        let mk = |id: &str, lang: &str| AgencyRecord {
            agency_id: Some(id.into()), agency_name: "X".into(),
            agency_url: "https://example.com".into(), agency_timezone: "Europe/Istanbul".into(),
            agency_lang: Some(lang.into()), agency_phone: None, agency_fare_url: None,
            agency_email: None, agency_cemv_support: None, row: Default::default(), line: 2,
        };
        let (mut recs, _map) = empty();
        recs.agencies = vec![mk("A1", "tr"), mk("A2", "en")];
        let result = check(&recs, &EntityMap::default(), 20260515);
        assert!(result.notices.iter().any(|n| n.rule_id == "AGN_017"), "farklı agency_lang → AGN_017");
    }

    #[test]
    fn consistent_agency_lang_no_agn_017() {
        use crate::k2::agency::AgencyRecord;
        let mk = |id: &str| AgencyRecord {
            agency_id: Some(id.into()), agency_name: "X".into(),
            agency_url: "https://example.com".into(), agency_timezone: "Europe/Istanbul".into(),
            agency_lang: Some("tr".into()), agency_phone: None, agency_fare_url: None,
            agency_email: None, agency_cemv_support: None, row: Default::default(), line: 2,
        };
        let (mut recs, _map) = empty();
        recs.agencies = vec![mk("A1"), mk("A2")];
        let result = check(&recs, &EntityMap::default(), 20260515);
        assert!(!result.notices.iter().any(|n| n.rule_id == "AGN_017"), "aynı agency_lang → AGN_017 olmamalı");
    }

    #[test]
    fn non_japanese_feed_no_jpn_001() {
        let (mut recs, _map) = empty();
        let mut s1 = stop("S1"); s1.stop_name = Some("Main St".into());
        recs.stops = vec![s1];
        // JP sinyali yok (feed_lang yok, ja-Hrkt yok) → kapı kapalı → JPN_001 yok.
        let result = check(&recs, &EntityMap::default(), 20260515);
        assert!(!result.notices.iter().any(|n| n.rule_id == "JPN_001"));
    }

    // �"?�"? JPN_002 �"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?

    #[test]
    fn dangling_jp_office_id_produces_jpn_002() {
        use crate::k2::office_jp::OfficeJpRecord;
        let (mut recs, _map) = empty();
        recs.office_jp = vec![OfficeJpRecord {
            office_id: "OF1".into(), office_name: Some("本社".into()),
            row: Default::default(), line: 2,
        }];
        // T1 geçerli ofise işaret eder, T2 tanımsız ofise → yalnız T2 işaretlenmeli.
        let mut ti = TripInternTable::new();
        let jpo1 = ti.jp_offices.len() as u32; ti.jp_offices.push(SmolStr::new("OF1"));
        let mut t1 = trip(&mut ti, "T1", "R1", "SVC1"); t1.jp_office_idx = jpo1;
        let jpo2 = ti.jp_offices.len() as u32; ti.jp_offices.push(SmolStr::new("MISSING"));
        let mut t2 = trip(&mut ti, "T2", "R1", "SVC1"); t2.jp_office_idx = jpo2;
        recs.trips = vec![t1, t2];
        recs.trip_interns = ti;
        let result = check(&recs, &EntityMap::default(), 20260515);
        let jpn: Vec<&str> = result.notices.iter()
            .filter(|n| n.rule_id == "JPN_002")
            .filter_map(|n| n.entity_id.as_deref())
            .collect();
        assert_eq!(jpn, vec!["T2"], "yalnız tanımsız ofise işaret eden T2 işaretlenmeli");
    }

    #[test]
    fn dangling_route_jp_office_id_produces_jpn_002() {
        use crate::k2::office_jp::OfficeJpRecord;
        let (mut recs, _map) = empty();
        recs.office_jp = vec![OfficeJpRecord {
            office_id: "OF1".into(), office_name: Some("本社".into()),
            row: Default::default(), line: 2,
        }];
        // routes.jp_office_id (resmî spec konumu): R1 geçerli, R2 tanımsız ofise → yalnız R2.
        recs.routes = vec![
            RouteRecord { jp_office_id: Some("OF1".into()), ..route("R1") },
            RouteRecord { jp_office_id: Some("MISSING".into()), ..route("R2") },
        ];
        let result = check(&recs, &EntityMap::default(), 20260515);
        let jpn: Vec<&str> = result.notices.iter()
            .filter(|n| n.rule_id == "JPN_002")
            .filter_map(|n| n.entity_id.as_deref())
            .collect();
        assert_eq!(jpn, vec!["R2"], "yalnız tanımsız ofise işaret eden R2 işaretlenmeli");
    }

    #[test]
    fn valid_jp_office_id_no_jpn_002() {
        use crate::k2::office_jp::OfficeJpRecord;
        let (mut recs, _map) = empty();
        recs.office_jp = vec![OfficeJpRecord {
            office_id: "OF1".into(), office_name: None,
            row: Default::default(), line: 2,
        }];
        let mut ti = TripInternTable::new();
        let jpo1 = ti.jp_offices.len() as u32; ti.jp_offices.push(SmolStr::new("OF1"));
        let mut t = trip(&mut ti, "T1", "R1", "SVC1"); t.jp_office_idx = jpo1;
        recs.trips = vec![t];
        recs.trip_interns = ti;
        let result = check(&recs, &EntityMap::default(), 20260515);
        assert!(!result.notices.iter().any(|n| n.rule_id == "JPN_002"));
    }

    // �"?�"? JPN_003 �"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?

    #[test]
    fn dangling_agency_jp_id_produces_jpn_003() {
        use crate::k2::agency::AgencyRecord;
        use crate::k2::agency_jp::AgencyJpRecord;
        let (mut recs, _map) = empty();
        recs.agencies = vec![AgencyRecord {
            agency_id: Some("A1".into()), agency_name: "都営".into(),
            agency_url: "https://example.jp".into(), agency_timezone: "Asia/Tokyo".into(),
            agency_lang: None, agency_phone: None, agency_fare_url: None,
            agency_email: None, agency_cemv_support: None,
            row: Default::default(), line: 2,
        }];
        recs.agency_jp = vec![AgencyJpRecord {
            agency_id: Some("NONEXISTENT".into()), row: Default::default(), line: 2,
        }];
        let result = check(&recs, &EntityMap::default(), 20260515);
        assert!(result.notices.iter().any(|n| n.rule_id == "JPN_003"));
    }

    #[test]
    fn valid_agency_jp_id_no_jpn_003() {
        use crate::k2::agency::AgencyRecord;
        use crate::k2::agency_jp::AgencyJpRecord;
        let (mut recs, _map) = empty();
        recs.agencies = vec![AgencyRecord {
            agency_id: Some("A1".into()), agency_name: "都営".into(),
            agency_url: "https://example.jp".into(), agency_timezone: "Asia/Tokyo".into(),
            agency_lang: None, agency_phone: None, agency_fare_url: None,
            agency_email: None, agency_cemv_support: None,
            row: Default::default(), line: 2,
        }];
        recs.agency_jp = vec![AgencyJpRecord {
            agency_id: Some("A1".into()), row: Default::default(), line: 2,
        }];
        let result = check(&recs, &EntityMap::default(), 20260515);
        assert!(!result.notices.iter().any(|n| n.rule_id == "JPN_003"));
    }

    #[test]
    fn missing_translations_in_jp_feed_produces_jpn_004() {
        use crate::k2::office_jp::OfficeJpRecord;
        let (mut recs, _map) = empty();
        // *_jp dosyası = GTFS-JP sinyali; translations boş → JPN_004
        recs.office_jp = vec![OfficeJpRecord {
            office_id: "OF1".into(), office_name: Some("本社".into()),
            row: Default::default(), line: 2,
        }];
        let result = check(&recs, &EntityMap::default(), 20260515);
        assert!(result.notices.iter().any(|n| n.rule_id == "JPN_004"),
            "GTFS-JP sinyali + translations yok → JPN_004");
    }

    #[test]
    fn jp_feed_with_translations_no_jpn_004() {
        use crate::k2::office_jp::OfficeJpRecord;
        use crate::k2::translations::TranslationRecord;
        let (mut recs, _map) = empty();
        recs.office_jp = vec![OfficeJpRecord {
            office_id: "OF1".into(), office_name: Some("本社".into()),
            row: Default::default(), line: 2,
        }];
        recs.translations = vec![TranslationRecord {
            table_name: "stops".into(), field_name: "stop_name".into(),
            language: "ja-Hrkt".into(), translation: "ろっぽんぎ".into(),
            record_id: Some("S1".into()),
            record_sub_id: None, field_value: None,
            row: Default::default(), line: 2,
        }];
        let result = check(&recs, &EntityMap::default(), 20260515);
        assert!(!result.notices.iter().any(|n| n.rule_id == "JPN_004"),
            "translations mevcut → JPN_004 olmamalı");
    }

    #[test]
    fn non_jp_feed_no_jpn_004() {
        // JP sinyali yok (feed_lang yok, *_jp dosyası yok) + translations yok → JPN_004 yok
        let (recs, _map) = empty();
        let result = check(&recs, &EntityMap::default(), 20260515);
        assert!(!result.notices.iter().any(|n| n.rule_id == "JPN_004"));
    }

    #[test]
    fn empty_office_name_produces_jpn_005() {
        use crate::k2::office_jp::OfficeJpRecord;
        let (mut recs, _map) = empty();
        recs.office_jp = vec![
            OfficeJpRecord { office_id: "OF1".into(), office_name: Some("本社".into()), row: Default::default(), line: 2 },
            OfficeJpRecord { office_id: "OF2".into(), office_name: None, row: Default::default(), line: 3 },
        ];
        let result = check(&recs, &EntityMap::default(), 20260515);
        let ids: Vec<&str> = result.notices.iter()
            .filter(|n| n.rule_id == "JPN_005")
            .filter_map(|n| n.entity_id.as_deref())
            .collect();
        assert_eq!(ids, vec!["OF2"], "yalnız office_name boş OF2 işaretlenmeli");
    }

    #[test]
    fn missing_fares_in_jp_feed_produces_jpn_006() {
        use crate::k2::office_jp::OfficeJpRecord;
        let (mut recs, _map) = empty();
        recs.office_jp = vec![OfficeJpRecord { office_id: "OF1".into(), office_name: Some("本社".into()), row: Default::default(), line: 2 }];
        // fare_attributes/fare_rules boş → JPN_006
        let result = check(&recs, &EntityMap::default(), 20260515);
        assert!(result.notices.iter().any(|n| n.rule_id == "JPN_006"), "GTFS-JP + fares yok → JPN_006");
    }

    #[test]
    fn uniform_fare_without_fare_rules_does_not_produce_jpn_006() {
        use crate::k2::office_jp::OfficeJpRecord;
        let (mut recs, _map) = empty();
        recs.office_jp = vec![OfficeJpRecord {
            office_id: "OF1".into(), office_name: Some("本社".into()),
            row: Default::default(), line: 2,
        }];
        let mut row = std::collections::HashMap::new();
        row.insert("price".into(), "200".into());
        row.insert("currency_type".into(), "JPY".into());
        row.insert("payment_method".into(), "0".into());
        recs.fare_attributes = vec![FareAttributeRecord {
            fare_id: "F1".into(), price: Some(200.0), currency_type: "JPY".into(),
            payment_method: Some(0), transfers: None, transfer_duration: None,
            agency_id: None, row, line: 2,
        }];
        let result = check(&recs, &EntityMap::default(), 20260515);
        assert!(!result.notices.iter().any(|n| n.rule_id == "JPN_006"),
            "tek tip ücrette fare_rules koşullu zorunluluk üretmemeli: {:?}", result.notices);
    }

    #[test]
    fn distinct_fare_profiles_without_fare_rules_produce_jpn_006() {
        use crate::k2::office_jp::OfficeJpRecord;
        let (mut recs, _map) = empty();
        recs.office_jp = vec![OfficeJpRecord {
            office_id: "OF1".into(), office_name: Some("本社".into()),
            row: Default::default(), line: 2,
        }];
        let fare = |id: &str, price: &str| {
            let mut row = std::collections::HashMap::new();
            row.insert("price".into(), price.into());
            row.insert("currency_type".into(), "JPY".into());
            row.insert("payment_method".into(), "0".into());
            FareAttributeRecord {
                fare_id: id.into(), price: price.parse().ok(), currency_type: "JPY".into(),
                payment_method: Some(0), transfers: None, transfer_duration: None,
                agency_id: None, row, line: 2,
            }
        };
        recs.fare_attributes = vec![fare("F1", "200"), fare("F2", "300")];
        let result = check(&recs, &EntityMap::default(), 20260515);
        assert!(result.notices.iter().any(|n| n.rule_id == "JPN_006"),
            "farklı ücret profilleri için fare_rules eksikliği raporlanmalı");
    }

    #[test]
    fn missing_feed_info_in_jp_feed_produces_jpn_007() {
        use crate::k2::office_jp::OfficeJpRecord;
        let (mut recs, _map) = empty();
        recs.office_jp = vec![OfficeJpRecord { office_id: "OF1".into(), office_name: Some("本社".into()), row: Default::default(), line: 2 }];
        // feed_info boş → JPN_007
        let result = check(&recs, &EntityMap::default(), 20260515);
        assert!(result.notices.iter().any(|n| n.rule_id == "JPN_007"), "GTFS-JP + feed_info yok → JPN_007");
    }

    #[test]
    fn non_jp_feed_no_jpn_005_006_007() {
        // GTFS-JP sinyali yok → office/fare/feed_info kuralları susar
        let (recs, _map) = empty();
        let result = check(&recs, &EntityMap::default(), 20260515);
        let ids: Vec<&str> = result.notices.iter().map(|n| n.rule_id.as_str()).collect();
        assert!(!ids.contains(&"JPN_005"));
        assert!(!ids.contains(&"JPN_006"));
        assert!(!ids.contains(&"JPN_007"));
    }

    // �"?�"? XFL_003 �"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?

    #[test]
    fn trip_with_missing_shape_produces_trp_004() {
        let (mut recs, map) = empty();
        let mut ti = TripInternTable::new();
        recs.trips = vec![trip_s(&mut ti, "T1", "R1", "SVC1", "MISSING_SHAPE", None)];
        recs.trip_interns = ti;
        let result = check(&recs, &map, 20260515);
        assert!(result.notices.iter().any(|n| n.rule_id == "TRP_004"));
    }

    // ── SHP_019: shape trip tarafından referanslanmış ama trip'lerin stop_times'ı yok ────────

    #[test]
    fn shape_with_trip_no_stm_produces_shp_019() {
        let mut shape_ti = ShapeInternTable::new();
        use crate::k2::shapes::ShapePointRecord;
        let (mut recs, mut map) = empty();
        map.routes.insert("R1".into(), 0);
        map.services.insert("SVC1".into());
        map.trips.insert("T1".into(), 0);
        let mut ti = TripInternTable::new();
        recs.trips = vec![trip_s(&mut ti, "T1", "R1", "SVC1", "S1", None)];
        recs.trip_interns = ti;
        recs.shapes = vec![ShapePointRecord::new(shape_ti.intern("S1"), Some(41.0), Some(29.0), Some(1), None, 2)];
        recs.shape_interns = shape_ti.clone();
        // stop_times yok → SHP_019 ateşlenmeli
        let result = check(&recs, &map, 20260515);
        assert!(result.notices.iter().any(|n| n.rule_id == "SHP_019"), "SHP_019 olmalı: trip var ama stop_times yok");
    }

    #[test]
    fn shape_with_trip_having_stm_no_shp_019() {
        let mut shape_ti = ShapeInternTable::new();
        use crate::k2::shapes::ShapePointRecord;
        use crate::k2::stop_times::StopTimeRecord;
        let (mut recs, mut map) = empty();
        map.routes.insert("R1".into(), 0);
        map.services.insert("SVC1".into());
        map.trips.insert("T1".into(), 0);
        map.stops.insert("STOP1".into(), 0);
        let mut ti = TripInternTable::new();
        recs.trips = vec![trip_s(&mut ti, "T1", "R1", "SVC1", "S1", None)];
        recs.trip_interns = ti;
        recs.shapes = vec![ShapePointRecord::new(shape_ti.intern("S1"), Some(41.0), Some(29.0), Some(1), None, 2)];
        recs.shape_interns = shape_ti.clone();
        recs.stop_times = vec![StopTimeRecord {
            trip_id: "T1".into(), stop_id: "STOP1".into(),
            stop_sequence: Some(1), line: 10,
            ..Default::default()
        }];
        recs.stop_times_index = StopTimesIndex::from_records(&recs.stop_times);
        // stop_times var → SHP_019 ateşlenmemeli
        let result = check(&recs, &map, 20260515);
        assert!(!result.notices.iter().any(|n| n.rule_id == "SHP_019"), "stop_times olan trip için SHP_019 olmamalı");
    }

    #[test]
    fn orphan_shape_no_shp_019() {
        let mut shape_ti = ShapeInternTable::new();
        use crate::k2::shapes::ShapePointRecord;
        let (mut recs, map) = empty();
        recs.shapes = vec![ShapePointRecord::new(shape_ti.intern("ORPHAN"), Some(41.0), Some(29.0), Some(1), None, 2)];
        recs.shape_interns = shape_ti.clone();
        // Hiç trip yok → SHP_018 ateşler, SHP_019 ateşlemez
        let result = check(&recs, &map, 20260515);
        assert!(!result.notices.iter().any(|n| n.rule_id == "SHP_019"), "orphan shape SHP_019 değil SHP_018 üretmeli");
    }

    //�"?�"? XFL_004 �"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?

    #[test]
    fn fare_rule_with_missing_route_produces_frl_002() {
        use crate::k2::fare_rules::FareRuleRecord;
        let (mut recs, map) = empty();
        recs.fare_rules = vec![FareRuleRecord {
            fare_id: "F1".into(),
            route_id: Some("MISSING_ROUTE".into()),
            origin_id: None, destination_id: None, contains_id: None,
            row: Default::default(), line: 2,
        }];
        let result = check(&recs, &map, 20260515);
        assert!(result.notices.iter().any(|n| n.rule_id == "FRL_002"));
    }

    // �"?�"? XFL_006 �"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?

    #[test]
    fn removal_only_service_produces_xfl_006() {
        use crate::k2::calendar_dates::CalendarDateIndex;
        let (mut recs, _map) = empty();
        // Servis yalnızca exception_type=2 (kaldırma) kaydı var, calendar.txt'te veya type=1'de yok.
        // removed'da anahtar var ama Vec boş (date=None): XFL_006 anahtar varlığına bakar.
        let mut idx = CalendarDateIndex::default();
        idx.removed.insert("GHOST_SVC".into(), vec![]);
        idx.exception_count.insert("GHOST_SVC".into(), 1);
        recs.calendar_dates = idx;
        let result = check(&recs, &EntityMap::default(), 20260515);
        assert!(result.notices.iter().any(|n| n.rule_id == "XFL_006"));
    }

    // �"?�"? XFL_011 �"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?

    #[test]
    fn calendar_outside_feed_info_range_produces_xfl_011() {
        use crate::k2::feed_info::FeedInfoRecord;
        let (mut recs, _map) = empty();
        recs.feed_info = vec![FeedInfoRecord {
            feed_publisher_name: "Pub".into(),
            feed_publisher_url: "https://example.com".into(),
            feed_lang: "tr".into(),
            feed_start_date: Some((2025, 1, 1)),
            feed_end_date: Some((2025, 12, 31)),
            feed_version: None, feed_contact_email: None, feed_contact_url: None,
            row: Default::default(), line: 2,
        }];
        recs.calendars = vec![CalendarRecord {
            service_id: "SVC".into(),
            days: [Some(1); 7],
            start_date: Some((2024, 6, 1)), // feed başlangıcından önce
            end_date: Some((2025, 6, 30)),
            row: Default::default(), line: 2,
        }];
        let result = check(&recs, &EntityMap::default(), 20260515);
        assert!(result.notices.iter().any(|n| n.rule_id == "XFL_011"));
    }

    // �"?�"? XFL_012 �"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?

    #[test]
    fn route_with_trips_but_no_stop_times_produces_xfl_012() {
        let (mut recs, mut map) = empty();
        map.routes.insert("R1".into(), 0);
        map.trips.insert("T1".into(), 0);
        recs.routes = vec![route("R1")];
        let mut ti = TripInternTable::new();
        recs.trips = vec![trip(&mut ti, "T1", "R1", "SVC1")];
        recs.trip_interns = ti;
        // stop_times bo�Y �?' T1'in kaydı yok
        let result = check(&recs, &map, 20260515);
        assert!(result.notices.iter().any(|n| n.rule_id == "XFL_012"));
    }

    // �"?�"? XFL_014 �"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?

    #[test]
    fn dangling_translation_produces_xfl_014() {
        use crate::k2::translations::TranslationRecord;
        let (mut recs, _map) = empty();
        recs.translations = vec![TranslationRecord {
            table_name: "stops".into(),
            field_name: "stop_name".into(),
            language: "tr".into(),
            translation: "Durak".into(),
            record_id: Some("DELETED_STOP".into()),
            record_sub_id: None, field_value: None,
            row: Default::default(), line: 2,
        }];
        let result = check(&recs, &EntityMap::default(), 20260515);
        assert!(result.notices.iter().any(|n| n.rule_id == "XFL_014"));
    }

    // �"?�"? XFL_015 �"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?

    #[test]
    fn attribution_with_bad_refs_produces_xfl_015() {
        use crate::k2::attributions::AttributionRecord;
        let (mut recs, _map) = empty();
        recs.attributions = vec![AttributionRecord {
            attribution_id: None,
            organization_name: "Org".into(),
            is_producer: Some(1), is_operator: None, is_authority: None,
            agency_id: Some("MISSING_AGENCY".into()),
            route_id: None, trip_id: None,
            attribution_url: None, attribution_email: None,
            row: Default::default(), line: 2,
        }];
        let result = check(&recs, &EntityMap::default(), 20260515);
        assert!(result.notices.iter().any(|n| n.rule_id == "XFL_015"));
    }

    // �"?�"? XFL_016 �"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?

    #[test]
    fn feed_info_translation_without_feed_info_produces_xfl_016() {
        use crate::k2::translations::TranslationRecord;
        let (mut recs, _map) = empty();
        recs.translations = vec![TranslationRecord {
            table_name: "feed_info".into(),
            field_name: "feed_publisher_name".into(),
            language: "tr".into(),
            translation: "Yayıncı".into(),
            record_id: None, record_sub_id: None, field_value: None,
            row: Default::default(), line: 2,
        }];
        // feed_info bo�Y �?' XFL_016
        let result = check(&recs, &EntityMap::default(), 20260515);
        assert!(result.notices.iter().any(|n| n.rule_id == "XFL_016"));
    }

    // �"?�"? XFL_017 �"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?

    #[test]
    fn conflicting_cemv_support_produces_xfl_017() {
        use crate::k2::agency::AgencyRecord;
        let (mut recs, mut map) = empty();
        map.agencies.insert("A1".into(), 0);
        recs.agencies = vec![AgencyRecord {
            agency_id: Some("A1".into()),
            agency_name: "Agency".into(),
            agency_url: "https://example.com".into(),
            agency_timezone: "Europe/Istanbul".into(),
            agency_lang: None, agency_phone: None,
            agency_fare_url: None, agency_email: None,
            agency_cemv_support: Some(1), // agency=1
            row: Default::default(), line: 2,
        }];
        map.routes.insert("R1".into(), 0);
        recs.routes = vec![RouteRecord {
            route_cemv_support: Some(0), // route=0 → çelişki
            agency_id: Some("A1".into()),
            ..route("R1")
        }];
        let result = check(&recs, &map, 20260515);
        assert!(result.notices.iter().any(|n| n.rule_id == "XFL_017"));
    }

    // �"?�"? TRF_016 �"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?

    #[test]
    fn duplicate_transfer_key_produces_trf_016() {
        // ⚠️ Bu test eskiden BAĞLAMSIZ bir yineleme kuruyordu ve TRF_016 bekliyordu.
        // İki kartın da yazdığı ayrım bunun tersini söylüyor: bağlamsız çift TRF_012'nin,
        // trip/route bağlamlı çakışma TRF_016'nın alanıdır. Ayrımın YARISI kodda vardı
        // (TRF_012 bağlam taşıyanları eliyordu), TRF_016 tarafı eksikti — bu yüzden
        // `mdb-1859`'da ikisi de aynı 5.204 satırda ateşliyordu. Test artık bağlam taşıyor.
        let (mut recs, _map) = empty();
        let base = TransferRecord {
            from_stop_id: "S1".into(), to_stop_id: "S2".into(),
            transfer_type: Some(0),
            min_transfer_time: None,
            from_trip_id: Some("T1".into()), to_trip_id: Some("T2".into()),
            from_route_id: None, to_route_id: None,
            row: Default::default(), line: 2,
        };
        recs.transfers = vec![base.clone(), TransferRecord { line: 3, ..base }];
        let result = check(&recs, &EntityMap::default(), 20260515);
        assert!(result.notices.iter().any(|n| n.rule_id == "TRF_016"));
    }

    #[test]
    fn context_free_duplicate_transfer_belongs_to_trf_012_alone() {
        // Ayrımın ÖTEKİ YÖNÜ. Bağlamsız yineleme TRF_012 üretir ve TRF_016 ÜRETMEZ;
        // aksi hâlde aynı olgu iki kez raporlanır.
        let (mut recs, _map) = empty();
        let base = TransferRecord {
            from_stop_id: "S1".into(), to_stop_id: "S2".into(),
            transfer_type: Some(0),
            min_transfer_time: None,
            from_trip_id: None, to_trip_id: None,
            from_route_id: None, to_route_id: None,
            row: Default::default(), line: 2,
        };
        recs.transfers = vec![base.clone(), TransferRecord { line: 3, ..base }];
        let result = check(&recs, &EntityMap::default(), 20260515);
        assert!(result.notices.iter().any(|n| n.rule_id == "TRF_012"),
            "bağlamsız yineleme TRF_012 üretmeli");
        assert!(!result.notices.iter().any(|n| n.rule_id == "TRF_016"),
            "bağlamsız yineleme TRF_016 ÜRETMEMELİ — aynı olgu iki kez raporlanır");
    }

    // ── RCT_006 ───────────────────────────────────────────────────────────────

    fn rider_cats(specs: &[(&str, u32)]) -> Vec<crate::k2::rider_categories::RiderCategoryRecord> {
        use crate::k2::rider_categories::RiderCategoryRecord;
        specs.iter().enumerate().map(|(i, (id, def))| RiderCategoryRecord {
            rider_category_id: (*id).into(),
            rider_category_name: (*id).into(),
            is_default_fare_category: Some(*def),
            min_age: None, max_age: None,
            eligibility_url: None,
            row: Default::default(), line: i as u64 + 2,
        }).collect()
    }

    /// `None` rider_category_id = ürün TÜM tanımlı kategorilere uygun (spec okuması).
    fn fare_prods(specs: &[(&str, Option<&str>)]) -> Vec<crate::k2::fare_products::FareProductRecord> {
        use crate::k2::fare_products::FareProductRecord;
        specs.iter().enumerate().map(|(i, (pid, rcid))| FareProductRecord {
            fare_product_id: (*pid).into(),
            fare_product_name: None,
            rider_category_id: rcid.map(Into::into),
            fare_media_id: None, amount: None, currency: String::new(),
            row: Default::default(), line: i as u64 + 10,
        }).collect()
    }

    #[test]
    fn multiple_default_rider_categories_per_fare_product_produces_rct_006() {
        use crate::k2::fare_products::FareProductRecord;
        use crate::k2::rider_categories::RiderCategoryRecord;
        let (mut recs, _map) = empty();
        recs.rider_categories = vec![
            RiderCategoryRecord {
                rider_category_id: "adult".into(),
                rider_category_name: "Adult".into(),
                is_default_fare_category: Some(1),
                min_age: None, max_age: None,
                eligibility_url: None,
                row: Default::default(), line: 2,
            },
            RiderCategoryRecord {
                rider_category_id: "senior".into(),
                rider_category_name: "Senior".into(),
                is_default_fare_category: Some(1),
                min_age: None, max_age: None,
                eligibility_url: None,
                row: Default::default(), line: 3,
            },
        ];
        recs.fare_products = vec![
            FareProductRecord {
                fare_product_id: "fp1".into(),
                fare_product_name: None,
                rider_category_id: Some("adult".into()),
                fare_media_id: None, amount: None, currency: String::new(),
                row: Default::default(), line: 10,
            },
            FareProductRecord {
                fare_product_id: "fp1".into(),
                fare_product_name: None,
                rider_category_id: Some("senior".into()),
                fare_media_id: None, amount: None, currency: String::new(),
                row: Default::default(), line: 11,
            },
        ];
        let result = check(&recs, &EntityMap::default(), 20260515);
        assert!(result.notices.iter().any(|n| n.rule_id == "RCT_006"));
    }

    #[test]
    fn single_default_rider_category_per_fare_product_is_ok() {
        use crate::k2::fare_products::FareProductRecord;
        use crate::k2::rider_categories::RiderCategoryRecord;
        let (mut recs, _map) = empty();
        recs.rider_categories = vec![
            RiderCategoryRecord {
                rider_category_id: "adult".into(),
                rider_category_name: "Adult".into(),
                is_default_fare_category: Some(1),
                min_age: None, max_age: None,
                eligibility_url: None,
                row: Default::default(), line: 2,
            },
            RiderCategoryRecord {
                rider_category_id: "child".into(),
                rider_category_name: "Child".into(),
                is_default_fare_category: Some(0),
                min_age: None, max_age: None,
                eligibility_url: None,
                row: Default::default(), line: 3,
            },
        ];
        recs.fare_products = vec![
            FareProductRecord {
                fare_product_id: "fp1".into(),
                fare_product_name: None,
                rider_category_id: Some("adult".into()),
                fare_media_id: None, amount: None, currency: String::new(),
                row: Default::default(), line: 10,
            },
            FareProductRecord {
                fare_product_id: "fp1".into(),
                fare_product_name: None,
                rider_category_id: Some("child".into()),
                fare_media_id: None, amount: None, currency: String::new(),
                row: Default::default(), line: 11,
            },
        ];
        let result = check(&recs, &EntityMap::default(), 20260515);
        assert!(!result.notices.iter().any(|n| n.rule_id == "RCT_006"));
    }

    #[test]
    fn multiple_eligible_non_default_rider_categories_produces_rct_006() {
        use crate::k2::fare_products::FareProductRecord;
        use crate::k2::rider_categories::RiderCategoryRecord;
        let (mut recs, _map) = empty();
        recs.rider_categories = vec![
            RiderCategoryRecord {
                rider_category_id: "adult".into(),
                rider_category_name: "Adult".into(),
                is_default_fare_category: Some(0),
                min_age: None, max_age: None,
                eligibility_url: None,
                row: Default::default(), line: 2,
            },
            RiderCategoryRecord {
                rider_category_id: "senior".into(),
                rider_category_name: "Senior".into(),
                is_default_fare_category: Some(0),
                min_age: None, max_age: None,
                eligibility_url: None,
                row: Default::default(), line: 3,
            },
        ];
        recs.fare_products = vec![
            FareProductRecord {
                fare_product_id: "fp1".into(),
                fare_product_name: None,
                rider_category_id: Some("adult".into()),
                fare_media_id: None, amount: None, currency: String::new(),
                row: Default::default(), line: 10,
            },
            FareProductRecord {
                fare_product_id: "fp1".into(),
                fare_product_name: None,
                rider_category_id: Some("senior".into()),
                fare_media_id: None, amount: None, currency: String::new(),
                row: Default::default(), line: 11,
            },
        ];
        let result = check(&recs, &EntityMap::default(), 20260515);
        assert!(result.notices.iter().any(|n| n.rule_id == "RCT_006"));
    }

    #[test]
    fn blank_rider_category_with_multiple_non_default_categories_produces_rct_006() {
        use crate::k2::fare_products::FareProductRecord;
        use crate::k2::rider_categories::RiderCategoryRecord;
        let (mut recs, _map) = empty();
        recs.rider_categories = vec![
            RiderCategoryRecord {
                rider_category_id: "adult".into(),
                rider_category_name: "Adult".into(),
                is_default_fare_category: Some(0),
                min_age: None, max_age: None,
                eligibility_url: None,
                row: Default::default(), line: 2,
            },
            RiderCategoryRecord {
                rider_category_id: "senior".into(),
                rider_category_name: "Senior".into(),
                is_default_fare_category: Some(0),
                min_age: None, max_age: None,
                eligibility_url: None,
                row: Default::default(), line: 3,
            },
        ];
        recs.fare_products = vec![FareProductRecord {
            fare_product_id: "fp1".into(),
            fare_product_name: None,
            rider_category_id: None,
            fare_media_id: None, amount: None, currency: String::new(),
            row: Default::default(), line: 10,
        }];
        let result = check(&recs, &EntityMap::default(), 20260515);
        assert!(result.notices.iter().any(|n| n.rule_id == "RCT_006"));
    }

    #[test]
    fn one_non_default_rider_category_does_not_produce_rct_006() {
        use crate::k2::fare_products::FareProductRecord;
        use crate::k2::rider_categories::RiderCategoryRecord;
        let (mut recs, _map) = empty();
        recs.rider_categories = vec![RiderCategoryRecord {
            rider_category_id: "adult".into(),
            rider_category_name: "Adult".into(),
            is_default_fare_category: Some(0),
            min_age: None, max_age: None,
            eligibility_url: None,
            row: Default::default(), line: 2,
        }];
        recs.fare_products = vec![FareProductRecord {
            fare_product_id: "fp1".into(),
            fare_product_name: None,
            rider_category_id: Some("adult".into()),
            fare_media_id: None, amount: None, currency: String::new(),
            row: Default::default(), line: 10,
        }];
        let result = check(&recs, &EntityMap::default(), 20260515);
        assert!(!result.notices.iter().any(|n| n.rule_id == "RCT_006"));
    }

    /// mdb-13 minyatürü: hiç varsayılan kategori yok, birden çok ürün etkileniyor.
    /// Tek feed notice çıkmalı ve çözücü kategorileri ADIYLA saymalı — o feed'de
    /// sezgisel görünen kategoriyi seçmek bulguların yarısını açık bırakıyordu.
    #[test]
    fn zero_default_rider_categories_aggregate_to_one_feed_notice_naming_the_fix() {
        let (mut recs, _map) = empty();
        recs.rider_categories = rider_cats(&[("adult", 0), ("senior", 0), ("disabled", 0)]);
        recs.fare_products = fare_prods(&[
            // Tüm kategorilere uygun (boş rider_category_id).
            ("single_ride", None),
            // Yalnız senior+disabled — kesişimi bu ürün daraltır.
            ("day_pass_discount", Some("senior")),
            ("day_pass_discount", Some("disabled")),
        ]);
        let result = check(&recs, &EntityMap::default(), 20260515);
        let hits: Vec<_> = result.notices.iter().filter(|n| n.rule_id == "RCT_006").collect();
        assert_eq!(hits.len(), 1, "kök kusur tek → tek notice");
        assert_eq!(hits[0].entity_type, EntityType::Feed);
        let msg = &hits[0].message;
        assert!(msg.contains("disabled, senior"), "çözücü kategoriler adıyla: {msg}");
        assert!(!msg.contains("adult"), "adult tek başına çözmez, önerilmemeli: {msg}");
        let details = hits[0].details.as_ref().expect("feed özeti details taşır");
        assert_eq!(details.get("affected_products").map(String::as_str), Some("2"));
        assert_eq!(
            details.get("resolving_categories").map(String::as_str),
            Some("disabled, senior")
        );
    }

    /// Varsayılan VAR ama ürünün uygunluk kümesinde yok — bu ürüne özgüdür, kök kusur
    /// tek değildir, toplulanmaz.
    #[test]
    fn default_outside_eligible_set_stays_per_product() {
        let (mut recs, _map) = empty();
        recs.rider_categories = rider_cats(&[("adult", 1), ("senior", 0), ("disabled", 0)]);
        recs.fare_products = fare_prods(&[
            ("p1", Some("senior")), ("p1", Some("disabled")),
            ("p2", Some("senior")), ("p2", Some("disabled")),
        ]);
        let result = check(&recs, &EntityMap::default(), 20260515);
        let hits: Vec<_> = result.notices.iter().filter(|n| n.rule_id == "RCT_006").collect();
        assert_eq!(hits.len(), 2, "ürün başına kalmalı");
        assert!(hits.iter().all(|n| n.entity_type == EntityType::Row));
    }

    /// Uygunluk kümeleri ayrıksa tek kategori yetmez; özet bunu SÖYLEMELİ, yoksa
    /// üretici olmayan bir çözümü arar.
    #[test]
    fn disjoint_eligible_sets_report_that_one_default_is_not_enough() {
        let (mut recs, _map) = empty();
        recs.rider_categories = rider_cats(&[("a", 0), ("b", 0), ("c", 0), ("d", 0)]);
        recs.fare_products = fare_prods(&[
            ("p1", Some("a")), ("p1", Some("b")),
            ("p2", Some("c")), ("p2", Some("d")),
        ]);
        let result = check(&recs, &EntityMap::default(), 20260515);
        let hits: Vec<_> = result.notices.iter().filter(|n| n.rule_id == "RCT_006").collect();
        assert_eq!(hits.len(), 1);
        assert!(hits[0].message.contains("ortusmuyor"), "{}", hits[0].message);
        assert!(hits[0].details.as_ref().is_some_and(|d| !d.contains_key("resolving_categories")));
    }

    #[test]
    fn missing_fare_agency_is_summarized_once_per_feed() {
        let (mut recs, map) = empty();
        recs.fare_attributes = ["F1", "F2"].into_iter().enumerate().map(|(i, id)| FareAttributeRecord {
            fare_id: id.into(), price: Some(2.0), currency_type: "TRY".into(),
            payment_method: Some(0), transfers: None, transfer_duration: None,
            agency_id: None, row: Default::default(), line: i as u64 + 2,
        }).collect();
        let result = check(&recs, &map, 20260515);
        let found: Vec<_> = result.notices.iter().filter(|n| n.rule_id == "FIN_013").collect();
        assert_eq!(found.len(), 1);
        assert_eq!(found[0].entity_type, EntityType::Feed);
        assert!(found[0].message.contains("2 ücret tarifesinde"));
    }
    fn attributions_translation_fixture(
        attribution_id: &str,
        record_id: &str,
        language: &str,
    ) -> EntityRecords {
        let (mut recs, _map) = empty();
        recs.attributions = vec![AttributionRecord {
            attribution_id: Some(attribution_id.into()),
            organization_name: "Org".into(),
            is_producer: Some(1),
            is_operator: None,
            is_authority: None,
            agency_id: None,
            route_id: None,
            trip_id: None,
            attribution_url: None,
            attribution_email: None,
            row: Default::default(),
            line: 2,
        }];
        recs.translations = vec![TranslationRecord {
            table_name: "attributions".into(),
            field_name: "organization_name".into(),
            language: language.into(),
            translation: "Org Sales Office".into(),
            record_id: Some(record_id.into()),
            record_sub_id: None,
            field_value: None,
            row: Default::default(),
            line: 6,
        }];
        recs
    }

    #[test]
    fn trn_004_resolves_attributions_record_id() {
        // `attributions` eskiden "lookup not feasible" sayılıyordu; kimliği çözülebilir
        // olduğu için artık dil bağımsız eksende de denetlenir. Mevcut kimlik susmalı.
        let recs = attributions_translation_fixture("A1", "A1", "en");
        let result = check(&recs, &EntityMap::default(), 20260824);
        assert!(!result.notices.iter().any(|n| n.rule_id == "TRN_004"));
    }

    #[test]
    fn trn_004_reports_missing_attributions_record_id_in_any_language() {
        // JPN_019 yalnız `ja-Hrkt` satırlarına bakar; aynı kusur `en` satırında da
        // görünür olmalıdır, yoksa referans bütünlüğü dil eksenine bağlı kalır.
        let recs = attributions_translation_fixture("A1", "A2", "en");
        let result = check(&recs, &EntityMap::default(), 20260824);
        let trn004 = result.notices.iter().find(|n| n.rule_id == "TRN_004" && n.line == Some(6));
        let trn004 = trn004.expect("TRN_004 bulunmalı");
        // en/ja/fr şablonları `{entity_id}` ile doldurulur; boş bırakılırsa mesaj
        // "record_id '' not found." olur.
        assert_eq!(trn004.entity_id.as_deref(), Some("A2"));
        assert!(!result.notices.iter().any(|n| n.rule_id == "JPN_019"));
        // XFL_014 aynı eşlemeyi kullanır; feed özeti de artık bu satırı görmelidir.
        assert!(result.notices.iter().any(|n| n.rule_id == "XFL_014"));
    }

    #[test]
    fn trn_016_flags_field_value_matching_nothing() {
        use crate::k2::translations::TranslationRecord;
        use crate::k2::stops::StopRecord;
        let (mut recs, _map) = empty();
        let mk = |id: &str, name: &str| {
            let mut row = std::collections::HashMap::new();
            row.insert("stop_id".to_string(), id.to_string());
            row.insert("stop_name".to_string(), name.to_string());
            StopRecord {
                stop_id: id.to_string(), stop_name: Some(name.to_string()),
                row, line: 2, ..Default::default()
            }
        };
        recs.stops = vec![mk("S1", "Merkez"), mk("S2", "Sahil")];
        let base = TranslationRecord {
            table_name: "stops".into(), field_name: "stop_name".into(),
            language: "en".into(), translation: "Centre".into(),
            record_id: None, record_sub_id: None,
            field_value: Some("Merkez".into()),
            row: Default::default(), line: 2,
        };
        // Eşleşen çeviri → sessiz.
        recs.translations = vec![base.clone()];
        let ok = check(&recs, &EntityMap::default(), 20260515);
        assert!(!ok.notices.iter().any(|n| n.rule_id == "TRN_016"), "{:?}", ok.notices);

        // Eşleşmeyen değer → TRN_016.
        recs.translations = vec![TranslationRecord { field_value: Some("Yokistan".into()), ..base.clone() }];
        let bad = check(&recs, &EntityMap::default(), 20260515);
        let hits: Vec<_> = bad.notices.iter().filter(|n| n.rule_id == "TRN_016").collect();
        assert_eq!(hits.len(), 1, "feed başına tek özet: {hits:?}");
        assert!(hits[0].message.contains("Yokistan"), "{}", hits[0].message);
    }

    #[test]
    fn trn_016_defers_to_neighbours() {
        use crate::k2::translations::TranslationRecord;
        use crate::k2::stops::StopRecord;
        let (mut recs, _map) = empty();
        let mut row = std::collections::HashMap::new();
        row.insert("stop_id".to_string(), "S1".to_string());
        row.insert("stop_name".to_string(), "Merkez".to_string());
        recs.stops = vec![StopRecord { stop_id: "S1".into(), stop_name: Some("Merkez".into()), row, line: 2, ..Default::default() }];
        let base = TranslationRecord {
            table_name: "stops".into(), field_name: "stop_name".into(),
            language: "en".into(), translation: "X".into(),
            record_id: None, record_sub_id: None, field_value: Some("Yok".into()),
            row: Default::default(), line: 2,
        };
        // (a) `record_id` biçimi → XFL_014'ün alanı, TRN_016 susar.
        recs.translations = vec![TranslationRecord { record_id: Some("S1".into()), field_value: None, ..base.clone() }];
        assert!(!check(&recs, &EntityMap::default(), 20260515).notices.iter().any(|n| n.rule_id == "TRN_016"));

        // (b) `field_name` o dosyada sütun DEĞİL → TRN_002'nin alanı, TRN_016 susar.
        recs.translations = vec![TranslationRecord { field_name: "long_name".into(), ..base.clone() }];
        assert!(!check(&recs, &EntityMap::default(), 20260515).notices.iter().any(|n| n.rule_id == "TRN_016"));

        // (c) desteklenmeyen tablo (trips — ham satır tutulmaz) → sessiz.
        recs.translations = vec![TranslationRecord { table_name: "trips".into(), field_name: "trip_headsign".into(), ..base }];
        assert!(!check(&recs, &EntityMap::default(), 20260515).notices.iter().any(|n| n.rule_id == "TRN_016"));
    }

    #[test]
    fn pth_030_flags_pathway_on_platform_with_boarding_areas() {
        use crate::k2::stops::StopRecord;
        use crate::k2::pathways::PathwayRecord;
        let (mut recs, mut map) = empty();
        let stop = |id: &str, lt: Option<u32>, parent: &str| {
            let mut row = std::collections::HashMap::new();
            row.insert("stop_id".to_string(), id.to_string());
            row.insert("parent_station".to_string(), parent.to_string());
            StopRecord { stop_id: id.to_string(), location_type: lt, row, line: 2, ..Default::default() }
        };
        // P1 platformunun boarding area'sı var; P2'nin yok.
        recs.stops = vec![
            stop("P1", Some(0), "ST1"), stop("BA1", Some(4), "P1"),
            stop("P2", Some(0), "ST1"), stop("E1", Some(2), "ST1"),
        ];
        for (i, s) in recs.stops.iter().enumerate() { map.stops.insert(s.stop_id.clone(), i); }
        let pw = |id: &str, from: &str, to: &str, line: u64| PathwayRecord {
            pathway_id: id.into(), from_stop_id: from.into(), to_stop_id: to.into(),
            pathway_mode: Some(1), is_bidirectional: Some(0),
            length: None, traversal_time: None, stair_count: None, max_slope: None,
            min_width: None, row: Default::default(), line,
        };
        recs.pathways = vec![pw("W1", "E1", "P1", 2), pw("W2", "E1", "P2", 3)];
        let result = check(&recs, &map, 20260515);
        let hits: Vec<_> = result.notices.iter().filter(|n| n.rule_id == "PTH_030").collect();
        assert_eq!(hits.len(), 1, "yalnız boarding area'sı olan platforma bağlanan: {hits:?}");
        assert_eq!(hits[0].entity_id.as_deref(), Some("W1"));
    }

    #[test]
    fn jpn_012_reports_missing_agency_jp_id() {
        let (mut recs, _map) = empty();
        recs.agency_jp = vec![AgencyJpRecord { agency_id: None, row: Default::default(), line: 7 }];
        let result = check(&recs, &EntityMap::default(), 20260824);
        assert!(result.notices.iter().any(|n| n.rule_id == "JPN_012"
            && n.file.as_deref() == Some("agency_jp.txt") && n.line == Some(7)));
    }

    #[test]
    fn jpn_013_reports_invalid_agency_zip_number() {
        let (mut recs, _map) = empty();
        let mut row = std::collections::HashMap::new();
        row.insert("agency_zip_number".to_string(), "123-4567".to_string());
        recs.agency_jp = vec![AgencyJpRecord { agency_id: Some("A1".into()), row, line: 3 }];
        let result = check(&recs, &EntityMap::default(), 20260824);
        assert!(result.notices.iter().any(|n| n.rule_id == "JPN_013"));
    }

    #[test]
    fn jpn_014_reports_empty_and_duplicate_office_ids() {
        let (mut recs, _map) = empty();
        recs.office_jp = vec![
            OfficeJpRecord { office_id: String::new(), office_name: Some("A".into()), row: Default::default(), line: 2 },
            OfficeJpRecord { office_id: "O1".into(), office_name: Some("B".into()), row: Default::default(), line: 3 },
            OfficeJpRecord { office_id: "O1".into(), office_name: Some("C".into()), row: Default::default(), line: 4 },
        ];
        let result = check(&recs, &EntityMap::default(), 20260824);
        assert_eq!(result.notices.iter().filter(|n| n.rule_id == "JPN_014").count(), 2);
    }

    #[test]
    fn jpn_015_reports_dangling_routes_jp_route_id() {
        let (mut recs, _map) = empty();
        recs.routes = vec![route("R1")];
        recs.routes_jp = vec![RoutesJpRecord {
            route_id: "MISSING".into(), route_update_date: None, origin_stop: Some("metin".into()),
            via_stop: None, destination_stop: None, row: Default::default(), line: 8,
        }];
        let result = check(&recs, &EntityMap::default(), 20260824);
        assert!(result.notices.iter().any(|n| n.rule_id == "JPN_015"
            && n.file.as_deref() == Some("routes_jp.txt") && n.line == Some(8)));
    }

    #[test]
    fn jpn_016_reports_invalid_pattern_jp_update_date() {
        let (mut recs, _map) = empty();
        recs.pattern_jp = vec![PatternJpRecord {
            jp_pattern_id: "P1".into(), route_update_date: Some("20260231".into()),
            origin_stop: None, via_stop: None, destination_stop: None, row: Default::default(), line: 5,
        }];
        let result = check(&recs, &EntityMap::default(), 20260824);
        assert!(result.notices.iter().any(|n| n.rule_id == "JPN_016"
            && n.file.as_deref() == Some("pattern_jp.txt")
            && n.line == Some(5)));
    }

    #[test]
    fn jpn_016_reports_invalid_legacy_routes_jp_update_date() {
        let (mut recs, _map) = empty();
        recs.routes_jp = vec![RoutesJpRecord {
            route_id: "R1".into(), route_update_date: Some("令和8年4月6日".into()),
            origin_stop: None, via_stop: None, destination_stop: None,
            row: Default::default(), line: 7,
        }];
        let result = check(&recs, &EntityMap::default(), 20260824);
        assert!(result.notices.iter().any(|n| n.rule_id == "JPN_016"
            && n.file.as_deref() == Some("routes_jp.txt")
            && n.line == Some(7)));
    }

    #[test]
    fn jpn_017_reports_missing_and_duplicate_pattern_ids() {
        let (mut recs, _map) = empty();
        let pattern = |id: &str, line| PatternJpRecord {
            jp_pattern_id: id.into(), route_update_date: None, origin_stop: None,
            via_stop: None, destination_stop: None, row: Default::default(), line,
        };
        recs.pattern_jp = vec![pattern("", 2), pattern("P1", 3), pattern("P1", 4)];
        let result = check(&recs, &EntityMap::default(), 20260824);
        assert_eq!(result.notices.iter().filter(|n| n.rule_id == "JPN_017").count(), 2);
    }

    #[test]
    fn jpn_018_reports_trip_pattern_not_in_pattern_file() {
        let (mut recs, _map) = empty();
        let mut ti = TripInternTable::new();
        let mut t = trip(&mut ti, "T1", "R1", "S1");
        ti.jp_patterns.push("MISSING_PATTERN".into());
        t.jp_pattern_idx = 1;
        recs.trips = vec![t];
        recs.trip_interns = ti;
        recs.has_pattern_jp_file = true;
        let result = check(&recs, &EntityMap::default(), 20260824);
        assert!(result.notices.iter().any(|n| n.rule_id == "JPN_018"
            && n.entity_id.as_deref() == Some("T1")));
    }

    #[test]
    fn jpn_018_silent_when_pattern_file_is_absent() {
        let (mut recs, _map) = empty();
        let mut ti = TripInternTable::new();
        let mut t = trip(&mut ti, "T1", "R1", "S1");
        ti.jp_patterns.push("INTERNAL_PATTERN_CODE".into());
        t.jp_pattern_idx = 1;
        recs.trips = vec![t];
        recs.trip_interns = ti;
        let result = check(&recs, &EntityMap::default(), 20260824);
        assert!(!result.notices.iter().any(|n| n.rule_id == "JPN_018"));
    }

    #[test]
    fn v4_profile_treats_v3_extension_files_as_reference_only() {
        let (mut recs, _map) = empty();
        recs.gtfs_jp_profile = GtfsJpProfile::V4;
        recs.agency_jp = vec![AgencyJpRecord { agency_id: None, row: Default::default(), line: 2 }];
        recs.office_jp = vec![OfficeJpRecord {
            office_id: "O1".into(), office_name: None, row: Default::default(), line: 3,
        }];
        recs.routes = vec![route("R1")];
        recs.routes_jp = vec![RoutesJpRecord {
            route_id: "MISSING".into(), route_update_date: Some("令和8年4月6日".into()),
            origin_stop: None, via_stop: None, destination_stop: None,
            row: Default::default(), line: 4,
        }];
        recs.pattern_jp = vec![PatternJpRecord {
            jp_pattern_id: "P1".into(), route_update_date: Some("20260231".into()),
            origin_stop: None, via_stop: None, destination_stop: None,
            row: Default::default(), line: 5,
        }];
        recs.has_pattern_jp_file = true;

        let mut ti = TripInternTable::new();
        let mut t = trip(&mut ti, "T1", "R1", "S1");
        ti.jp_patterns.push("MISSING_PATTERN".into());
        t.jp_pattern_idx = 1;
        recs.trips = vec![t];
        recs.trip_interns = ti;

        let result = check(&recs, &EntityMap::default(), 20260824);
        for id in ["JPN_002", "JPN_003", "JPN_005", "JPN_012", "JPN_013",
                   "JPN_014", "JPN_015", "JPN_016", "JPN_017", "JPN_018", "JPN_020"] {
            assert!(!result.notices.iter().any(|n| n.rule_id == id),
                "v4 profilinde v3 uzantı kuralı çalışmamalı: {id}");
        }
    }

    #[test]
    fn jpn_019_reports_invalid_kana_translation_reference() {
        let (mut recs, _map) = empty();
        recs.translations = vec![TranslationRecord {
            table_name: "stops".into(), field_name: "stop_name".into(), language: "ja-Hrkt".into(),
            translation: "とうきょう".into(), record_id: Some("MISSING_STOP".into()), record_sub_id: None,
            field_value: None, row: Default::default(), line: 6,
        }];
        let result = check(&recs, &EntityMap::default(), 20260824);
        assert!(result.notices.iter().any(|n| n.rule_id == "JPN_019"
            && n.file.as_deref() == Some("translations.txt") && n.line == Some(6)));
    }

    #[test]
    fn jpn_019_accepts_attributions_record_id() {
        let (mut recs, _map) = empty();
        recs.attributions = vec![AttributionRecord {
            attribution_id: Some("A1".into()),
            organization_name: "Org".into(),
            is_producer: Some(1),
            is_operator: None,
            is_authority: None,
            agency_id: None,
            route_id: None,
            trip_id: None,
            attribution_url: None,
            attribution_email: None,
            row: Default::default(),
            line: 2,
        }];
        recs.translations = vec![TranslationRecord {
            table_name: "attributions".into(),
            field_name: "organization_name".into(),
            language: "ja-Hrkt".into(),
            translation: "おーぐ".into(),
            record_id: Some("A1".into()),
            record_sub_id: None,
            field_value: None,
            row: Default::default(),
            line: 6,
        }];
        let result = check(&recs, &EntityMap::default(), 20260824);
        assert!(!result.notices.iter().any(|n| n.rule_id == "JPN_019"));
    }

    #[test]
    fn jpn_019_still_reports_missing_attributions_record_id() {
        // Fail-open sözleşmesi yalnız kimliği ÇÖZÜLEMEYEN tablo içindir. attributions.txt
        // yüklüyken bulunmayan bir record_id hâlâ bulgudur; aksi hâlde kolu tümden
        // kabule çeviren bir "düzeltme" de testlerden geçerdi.
        let (mut recs, _map) = empty();
        recs.attributions = vec![AttributionRecord {
            attribution_id: Some("A1".into()),
            organization_name: "Org".into(),
            is_producer: Some(1),
            is_operator: None,
            is_authority: None,
            agency_id: None,
            route_id: None,
            trip_id: None,
            attribution_url: None,
            attribution_email: None,
            row: Default::default(),
            line: 2,
        }];
        recs.translations = vec![TranslationRecord {
            table_name: "attributions".into(),
            field_name: "organization_name".into(),
            language: "ja-Hrkt".into(),
            translation: "おーぐ".into(),
            record_id: Some("A2".into()),
            record_sub_id: None,
            field_value: None,
            row: Default::default(),
            line: 6,
        }];
        let result = check(&recs, &EntityMap::default(), 20260824);
        assert!(result.notices.iter().any(|n| n.rule_id == "JPN_019" && n.line == Some(6)));
    }

    #[test]
    fn jpn_019_accepts_pathways_and_levels_record_ids() {
        let (mut recs, _map) = empty();
        recs.pathways = vec![PathwayRecord {
            pathway_id: "PW1".into(),
            from_stop_id: "S1".into(),
            to_stop_id: "S2".into(),
            pathway_mode: Some(1),
            is_bidirectional: Some(1),
            length: None,
            traversal_time: None,
            stair_count: None,
            max_slope: None,
            min_width: None,
            row: Default::default(),
            line: 2,
        }];
        recs.levels = vec![LevelRecord {
            level_id: "L1".into(),
            level_index: Some(0.0),
            level_name: Some("1F".into()),
            row: Default::default(),
            line: 2,
        }];
        recs.translations = vec![
            TranslationRecord {
                table_name: "pathways".into(),
                field_name: "signposted_as".into(),
                language: "ja-Hrkt".into(),
                translation: "てすとつうろ".into(),
                record_id: Some("PW1".into()),
                record_sub_id: None,
                field_value: None,
                row: Default::default(),
                line: 6,
            },
            TranslationRecord {
                table_name: "levels".into(),
                field_name: "level_name".into(),
                language: "ja-Hrkt".into(),
                translation: "いっかい".into(),
                record_id: Some("L1".into()),
                record_sub_id: None,
                field_value: None,
                row: Default::default(),
                line: 7,
            },
        ];
        let result = check(&recs, &EntityMap::default(), 20260824);
        assert!(!result.notices.iter().any(|n| n.rule_id == "JPN_019"));
    }

    #[test]
    fn jpn_019_still_reports_missing_pathways_record_id() {
        let (mut recs, _map) = empty();
        recs.pathways = vec![PathwayRecord {
            pathway_id: "PW1".into(),
            from_stop_id: "S1".into(),
            to_stop_id: "S2".into(),
            pathway_mode: Some(1),
            is_bidirectional: Some(1),
            length: None,
            traversal_time: None,
            stair_count: None,
            max_slope: None,
            min_width: None,
            row: Default::default(),
            line: 2,
        }];
        recs.translations = vec![TranslationRecord {
            table_name: "pathways".into(),
            field_name: "signposted_as".into(),
            language: "ja-Hrkt".into(),
            translation: "てすとつうろ".into(),
            record_id: Some("PW9".into()),
            record_sub_id: None,
            field_value: None,
            row: Default::default(),
            line: 6,
        }];
        let result = check(&recs, &EntityMap::default(), 20260824);
        assert!(result.notices.iter().any(|n| n.rule_id == "JPN_019" && n.line == Some(6)));
    }

    #[test]
    fn jpn_019_checks_stop_times_record_sub_id_against_source_rows() {
        let (mut recs, _map) = empty();
        let mut ti = TripInternTable::new();
        recs.trips = vec![trip(&mut ti, "T1", "R1", "S1")];
        recs.trip_interns = ti;
        recs.stop_times_index = StopTimesIndex::from_records(&[
            StopTimeRecord { trip_id: "T1".into(), stop_sequence: Some(5), ..Default::default() },
        ]);
        let translation = |sub: &str| TranslationRecord {
            table_name: "stop_times".into(), field_name: "stop_headsign".into(), language: "ja-Hrkt".into(),
            translation: "とうきょう".into(), record_id: Some("T1".into()), record_sub_id: Some(sub.into()),
            field_value: None, row: Default::default(), line: 6,
        };

        recs.translations = vec![translation("5")];
        let valid = check(&recs, &EntityMap::default(), 20260824);
        assert!(!valid.notices.iter().any(|n| n.rule_id == "JPN_019"));

        recs.translations = vec![translation("6")];
        let dangling = check(&recs, &EntityMap::default(), 20260824);
        assert!(dangling.notices.iter().any(|n| n.rule_id == "JPN_019" && n.line == Some(6)));
    }

    #[test]
    fn jpn_019_is_silent_for_unknown_translation_table() {
        let (mut recs, _map) = empty();
        recs.translations = vec![TranslationRecord {
            table_name: "unknown_table".into(), field_name: "unknown_field".into(), language: "ja-Hrkt".into(),
            translation: "とうきょう".into(), record_id: None, record_sub_id: None,
            field_value: None, row: Default::default(), line: 7,
        }];
        let result = check(&recs, &EntityMap::default(), 20260824);
        assert!(!result.notices.iter().any(|n| n.rule_id == "JPN_019"),
            "TRN_001 kök bulgusuna JPN_019 türevi eklenmemeli: {:?}", result.notices);
    }

    #[test]
    fn jpn_019_accepts_gtfs_jp_none_record_sub_id() {
        let (mut recs, _map) = empty();
        recs.stops = vec![stop("S1")];
        recs.translations = vec![TranslationRecord {
            table_name: "stops".into(), field_name: "stop_name".into(), language: "ja-Hrkt".into(),
            translation: "とうきょう".into(), record_id: Some("S1".into()),
            record_sub_id: Some("NONE".into()), field_value: None, row: Default::default(), line: 7,
        }];
        let result = check(&recs, &EntityMap::default(), 20260824);
        assert!(!result.notices.iter().any(|n| n.rule_id == "JPN_019"),
            "GTFS-JP v3'te NONE gerçek alt kimlik sayılmamalı: {:?}", result.notices);
    }

    #[test]
    fn jpn_019_v4_accepts_blank_and_rejects_none_record_sub_id() {
        let (mut recs, _map) = empty();
        recs.gtfs_jp_profile = GtfsJpProfile::V4;
        recs.stops = vec![stop("S1")];
        let mut translation = TranslationRecord {
            table_name: "stops".into(), field_name: "stop_name".into(), language: "ja-Hrkt".into(),
            translation: "とうきょう".into(), record_id: Some("S1".into()), record_sub_id: None,
            field_value: None, row: Default::default(), line: 7,
        };
        recs.translations = vec![translation.clone()];
        let missing = check(&recs, &EntityMap::default(), 20260824);
        assert!(!missing.notices.iter().any(|n| n.rule_id == "JPN_019"));

        translation.record_sub_id = Some("NONE".into());
        recs.translations = vec![translation];
        let invalid = check(&recs, &EntityMap::default(), 20260824);
        assert!(invalid.notices.iter().any(|n| n.rule_id == "JPN_019"));
    }

    #[test]
    fn v4_reports_missing_core_required_fields_for_gtfs_jp() {
        let (mut recs, _map) = empty();
        recs.gtfs_jp_profile = GtfsJpProfile::V4;
        recs.feed_info = vec![FeedInfoRecord {
            feed_publisher_name: "Pub".into(),
            feed_publisher_url: "https://example.com".into(),
            feed_lang: "ja".into(),
            feed_start_date: None,
            feed_end_date: None,
            feed_version: None,
            feed_contact_email: None,
            feed_contact_url: None,
            row: Default::default(),
            line: 2,
        }];
        recs.agencies = vec![agency_tz("A1", "UTC")];
        recs.agencies[0].agency_lang = None;
        recs.stops = vec![stop("S1")];
        let result = check(&recs, &EntityMap::default(), 20260824);
        let findings = result.notices.iter().filter(|n| n.rule_id == "JPN_022").collect::<Vec<_>>();
        assert_eq!(findings.len(), 5);
        assert!(findings.iter().all(|finding| {
            finding
                .details
                .as_ref()
                .and_then(|details| details.get("affected_records"))
                .map(String::as_str)
                == Some("1")
        }));
    }

    #[test]
    fn jpn_022_aggregates_repeated_missing_stop_fields() {
        let (mut recs, _map) = empty();
        recs.gtfs_jp_profile = GtfsJpProfile::V4;
        recs.has_gtfs_jp_file = true;
        recs.stops = vec![stop("S1"), stop("S2"), stop("S3")];

        let result = check(&recs, &EntityMap::default(), 20260824);
        let findings = result
            .notices
            .iter()
            .filter(|notice| {
                notice.rule_id == "JPN_022"
                    && notice.file.as_deref() == Some("stops.txt")
                    && notice.field.as_deref() == Some("location_type")
            })
            .collect::<Vec<_>>();

        assert_eq!(findings.len(), 1);
        assert_eq!(findings[0].entity_type, EntityType::Feed);
        assert_eq!(findings[0].line, None);
        assert_eq!(findings[0].observed_value.as_deref(), Some("3"));
        assert_eq!(findings[0].expected_value, None);
        assert_eq!(
            findings[0]
                .details
                .as_ref()
                .and_then(|details| details.get("affected_records"))
                .map(String::as_str),
            Some("3")
        );
    }

    #[test]
    fn jpn_020_reports_bad_office_url_and_phone() {
        let (mut recs, _map) = empty();
        let mut row = std::collections::HashMap::new();
        row.insert("office_url".to_string(), "not-a-url".to_string());
        row.insert("office_phone".to_string(), "x".to_string());
        recs.office_jp = vec![OfficeJpRecord { office_id: "O1".into(), office_name: Some("Ofis".into()), row, line: 9 }];
        let result = check(&recs, &EntityMap::default(), 20260824);
        assert_eq!(result.notices.iter().filter(|n| n.rule_id == "JPN_020").count(), 2);
    }

    #[test]
    fn jpn_021_reports_conflicting_kana_values() {
        let (mut recs, _map) = empty();
        recs.stops = vec![stop("S1")];
        let translation = |value: &str, line| TranslationRecord {
            table_name: "stops".into(), field_name: "stop_name".into(), language: "ja-Hrkt".into(),
            translation: value.into(), record_id: Some("S1".into()), record_sub_id: None,
            field_value: None, row: Default::default(), line,
        };
        recs.translations = vec![translation("とうきょう", 2), translation("トウキョウ", 3), translation("Tokyo", 4), translation("", 5)];
        let result = check(&recs, &EntityMap::default(), 20260824);
        let conflict = result.notices.iter().find(|n| n.rule_id == "JPN_021" && n.line == Some(3)).expect("JPN_021 çelişki bulgusu");
        assert!(conflict.message.contains("ilk satır 2"), "ilk görülen satır korunmalı: {}", conflict.message);
        assert!(result.notices.iter().any(|n| n.rule_id == "JPN_021" && n.line == Some(4)));
        assert!(result.notices.iter().any(|n| n.rule_id == "JPN_021" && n.line == Some(5)));
    }

}
