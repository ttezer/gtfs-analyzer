use std::collections::{BTreeSet, HashMap, HashSet};

use gtfs_core::{EntityType, Notice};
use gtfs_rules::get_rule;
use rustc_hash::{FxHashMap, FxHashSet};
use smol_str::SmolStr;

use crate::k2::EntityRecords;
use crate::k3_entity_graph::EntityMap;
use crate::timing::Timer;

// �"?�"? �?ıktı �"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?

#[derive(Debug, Default)]
pub struct K4Result {
    pub notices: Vec<Notice>,
}

// �"?�"? Ana fonksiyon �"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?

pub fn check(records: &EntityRecords, entity_map: &EntityMap, today: u32) -> K4Result {
    let mut notices = Vec::new();
    let mut ctr = 0u32;
    let map = entity_map;

    // stop_times geçişi → StopTimesIndex üzerinden (Vec<StopTimeRecord> taranmaz)
    let (stm_used_stop_ids, stm_trip_continuous, stm_trips_in_stm, stm_trip_stm_count, stm_bad_stop_ids) = {
        let _t = Timer::start("K4::stop_times");
        let idx = &records.stop_times_index;

        // STM_001: trip_id stop_times'ta var ama trips.txt'te yok
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

        // STM_002: stop_id stop_times'ta var ama stops.txt'te yok
        let mut bad_stop_ids: HashSet<&str> = HashSet::new();
        for stop_id in &idx.stop_id_set {
            if !map.stops.contains_key(stop_id.as_str()) {
                bad_stop_ids.insert(stop_id.as_str());
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

        // XFL_024: stop_times.location_group_id → location_groups (GTFS-Flex)
        // idx.rows hem test hem production'da dolu; flex None satırlar ucuzca atlanır.
        let mut xfl024_seen: HashSet<&str> = HashSet::new();
        let mut xfl025_seen: HashSet<&str> = HashSet::new();
        for st in &idx.rows {
            let Some(flex) = idx.flex_of(st) else { continue };
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
            // XFL_025: stop_times.location_id → locations.geojson feature id
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
        }

        // index'ten &str görünümlü geçici koleksiyonlar (fonksiyon imzaları değişmeden)
        let used_stop_ids: HashSet<&str>  = idx.stop_id_set.iter().map(|s| s.as_str()).collect();
        let trip_continuous: HashSet<&str> = idx.continuous_trips.iter().map(|s| s.as_str()).collect();
        let trips_in_stm: HashSet<&str>   = idx.trip_id_set.iter().map(|s| s.as_str()).collect();
        let trip_stm_count: HashMap<&str, u32> = idx.iter_trips()
            .map(|(k, v)| (k.as_str(), v.len() as u32))
            .collect();

        (used_stop_ids, trip_continuous, trips_in_stm, trip_stm_count, bad_stop_ids)
    };

    { let _t = Timer::start("K4::agencies");       check_agencies(records, map, &mut notices, &mut ctr); }
    { let _t = Timer::start("K4::stops");          check_stops(records, map, &mut notices, &mut ctr, &stm_used_stop_ids); }
    { let _t = Timer::start("K4::routes");         check_routes(records, map, &mut notices, &mut ctr); }
    { let _t = Timer::start("K4::trips");          check_trips(records, map, &mut notices, &mut ctr, &stm_trip_continuous); }
    { let _t = Timer::start("K4::pathways");       check_pathways(records, map, &mut notices, &mut ctr); }
    { let _t = Timer::start("K4::calendar");       check_calendar(records, map, &mut notices, &mut ctr, today); }
    { let _t = Timer::start("K4::calendar_dates"); check_calendar_dates(records, map, &mut notices, &mut ctr); }
    { let _t = Timer::start("K4::frequencies");    check_frequencies(records, map, &mut notices, &mut ctr, &stm_trips_in_stm); }
    { let _t = Timer::start("K4::transfers");      check_transfers(records, map, &mut notices, &mut ctr, &stm_trips_in_stm, &records.stop_times_index.trip_stop_set, &records.stop_times_index.stop_id_to_idx); }
    { let _t = Timer::start("K4::fare_attributes");check_fare_attributes(records, map, &mut notices, &mut ctr); }
    { let _t = Timer::start("K4::fare_rules");     check_fare_rules(records, map, &mut notices, &mut ctr); }
    { let _t = Timer::start("K4::fares_v2");       check_fares_v2(records, map, &mut notices, &mut ctr); }
    { let _t = Timer::start("K4::levels");         check_levels(records, map, &mut notices, &mut ctr); }
    { let _t = Timer::start("K4::translations");   check_translations(records, map, &mut notices, &mut ctr); }
    { let _t = Timer::start("K4::gtfs_jp");         check_gtfs_jp(records, &mut notices, &mut ctr); }
    { let _t = Timer::start("K4::attributions");   check_attributions(records, map, &mut notices, &mut ctr); }
    { let _t = Timer::start("K4::xfl");            check_xfl(records, map, &mut notices, &mut ctr, &stm_trips_in_stm, &stm_trip_stm_count, &stm_bad_stop_ids); }
    { let _t = Timer::start("K4::stm_shape_dist"); check_stm_shape_dist(records, &mut notices, &mut ctr); }

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

    K4Result { notices }
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
    *ctr += 1;
    let meta = get_rule(rule_id).unwrap_or_else(|| panic!("K4: bilinmeyen rule_id {rule_id}"));
    Notice {
        id: format!("k4/{rule_id}#{ctr}"),
        rule_id: rule_id.to_string(),
        severity: meta.severity,
        rule_class: meta.rule_class,
        entity_type,
        entity_id,
        scope_key,
        file: Some(file.to_string()),
        line,
        field: field.map(str::to_string),
        observed_value: observed,
        expected_value: expected,
        details: None,
        title: meta.title.to_string(),
        message,
        remediation: remediation.to_string(),
        blocks: meta.blocks.iter().map(|s| s.to_string()).collect(),
        base_effort: meta.base_effort,
        service_id: None,
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
    let first_tz = &records.agencies[0].agency_timezone;
    let mismatch = records
        .agencies
        .iter()
        .any(|r| &r.agency_timezone != first_tz);
    if mismatch {
        notices.push(notice(
            ctr,
            "AGN_005",
            EntityType::Agency,
            None,
            None,
            "agency.txt",
            None,
            Some("agency_timezone"),
            None,
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
            Some(format!("{routes_without_agency} hat")),
            Some("dolu".to_string()),
            format!(
                "Feed'de {} işletici var; {routes_without_agency} hatta agency_id belirtilmemiş — hangi işleticinin çalıştırdığı belirlenemiyor.",
                records.agencies.len()
            ),
            "Birden fazla kuruluş olduğunda tüm hatların agency_id alanını doldurun.",
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
        let parent = row_field(&rec.row, "parent_station");
        let loc_type = rec.location_type;

        // STP_009: parent_station geçerli stop_id'ye referans
        if !parent.is_empty() && !map.stops.contains_key(parent) {
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
        } else if !parent.is_empty() {
            // STP_010: parent_station'ın location_type = 1 olması
            if let Some(&pidx) = map.stops.get(parent) {
                let ptype = records.stops[pidx].location_type;
                if ptype != Some(1) {
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
        if matches!(loc_type, Some(2) | Some(3) | Some(4)) && parent.is_empty() {
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
                Some("dolu olmalı".to_string()),
                format!(
                    "location_type={} olan durak için parent_station zorunludur.",
                    loc_type.unwrap()
                ),
                "Bu durak tipine bir istasyon (location_type=1) parent_station olarak atayın.",
            ));
        }

        // STP_036: location_type=1 (station) parent_station içermemeli
        if loc_type == Some(1) && !parent.is_empty() {
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
                Some("boş olmalı".to_string()),
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

        // STP_021: boarding area (location_type=2) parent_station'ı platform (location_type=0) olmalı
        if matches!(loc_type, Some(2)) && !parent.is_empty() {
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

        // STP_026: stop_access geçerli enum (0,1,2)
        let stop_access_raw = row_field(&rec.row, "stop_access");
        if !stop_access_raw.is_empty() {
            if !matches!(stop_access_raw, "0" | "1" | "2") {
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
                    Some("0, 1 veya 2".to_string()),
                    format!("stop_access '{stop_access_raw}' geçersiz enum değeri."),
                    "stop_access için 0, 1 veya 2 kullanın.",
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
}

// �"?�"? TRP_002-004 / TRP_019: sefer cross-ref �"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?

fn check_trips(
    records: &EntityRecords,
    map: &EntityMap,
    notices: &mut Vec<Notice>,
    ctr: &mut u32,
    trip_has_continuous_stm: &HashSet<&str>,
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

        // TRP_019: continuous service aktifken shape_id zorunlu
        if rec.shape_idx == 0 {
            let route_continuous = map
                .routes
                .get(rec_route_id)
                .and_then(|&idx| records.routes.get(idx))
                .map(|r| {
                    matches!(r.continuous_pickup, Some(0) | Some(1))
                        || matches!(r.continuous_drop_off, Some(0) | Some(1))
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

    // PTH_014: from_stop_id ve to_stop_id farklı istasyonlara ait �?' referans bütünlü�Yü ihlali
    for rec in &records.pathways {
        if rec.pathway_id.is_empty() {
            continue;
        }
        let from_station = station_context(rec.from_stop_id.as_str(), &stop_parent, &stop_loc);
        let to_station = station_context(rec.to_stop_id.as_str(), &stop_parent, &stop_loc);
        if let (Some(fs), Some(ts)) = (from_station, to_station) {
            if fs != ts {
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
                    Some("aynı istasyon içinde olmalı".to_string()),
                    format!(
                        "pathway_id '{}' farklı istasyonlar arası bağlantı kuruyor (from_station='{fs}', to_station='{ts}').",
                        rec.pathway_id
                    ),
                    "Pathway yalnızca aynı istasyon içindeki durakları bağlamalıdır.",
                );
                let mut d = std::collections::HashMap::new();
                d.insert("from_station".to_string(), fs.to_string());
                d.insert("to_station".to_string(), ts.to_string());
                n.details = Some(d);
                notices.push(n);
            }
        }
    }

    // PTH_019: generic node (location_type=3) yalnızca tek pathway'e bağlı → dead-end
    {
        let mut conn_count: HashMap<&str, u32> = HashMap::new();
        for rec in &records.pathways {
            *conn_count.entry(rec.from_stop_id.as_str()).or_insert(0) += 1;
            *conn_count.entry(rec.to_stop_id.as_str()).or_insert(0) += 1;
        }
        for stop in &records.stops {
            if stop.location_type != Some(3) {
                continue;
            }
            let count = conn_count.get(stop.stop_id.as_str()).copied().unwrap_or(0);
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
            .map(|s| (s.stop_id.as_str(), s.level_id.as_deref().map_or(false, |l| !l.is_empty())))
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
            let all_inactive = rec.days.iter().all(|d| d.map_or(true, |v| v == 0));
            if all_inactive && !services_with_added.contains(rec.service_id.as_str()) {
                notices.push(notice(
                    ctr,
                    "CAL_018",
                    EntityType::Service,
                    Some(rec.service_id.clone()),
                    Some(rec.service_id.clone()),
                    "calendar.txt",
                    Some(rec.line),
                    None,
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

        // TRP_017: frekans tabanlı sefer stop_times'ta eksik
        if !trips_in_stm.contains(rec.trip_id.as_str()) {
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
        if matches!(ttype, Some(4) | Some(5)) {
            if rec.from_trip_id.is_none() || rec.to_trip_id.is_none() {
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
                    Some("tip 4/5 için her iki trip_id zorunlu".to_string()),
                    format!(
                        "transfer_type={} için from_trip_id ve to_trip_id zorunludur.",
                        ttype.unwrap()
                    ),
                    "Linked-trip transfer için her iki trip_id alanını doldurun.",
                ));
            }
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
                    None,
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
                    None, None, "transfers.txt", Some(rec.line), None,
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
                Some(format!("{missing} eksik kayıt")),
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
                Some("fare_rules.txt'te kayıt".to_string()),
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

    for rec in &records.fare_rules {
        let eid = Some(rec.fare_id.clone());

        // FRL_001: fare_id referansı
        if !rec.fare_id.is_empty() && !map.fare_attrs.contains_key(rec.fare_id.as_str()) {
            notices.push(notice(
                ctr,
                "FRL_001",
                EntityType::Fare,
                eid.clone(),
                eid.clone(),
                "fare_rules.txt",
                Some(rec.line),
                Some("fare_id"),
                Some(rec.fare_id.clone()),
                None,
                format!("'{}' ücret tarifesi fare_attributes.txt'te tanımlı değil.", rec.fare_id),
                "Geçerli bir fare_id kullanın.",
            ));
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

        // FRL_003-005: zone_id referansları
        for (rule, field, opt_id) in [
            ("FRL_003", "origin_id", &rec.origin_id),
            ("FRL_004", "destination_id", &rec.destination_id),
            ("FRL_005", "contains_id", &rec.contains_id),
        ] {
            if let Some(ref zid) = opt_id {
                if !map.zone_ids.contains(zid.as_str()) {
                    notices.push(notice(
                        ctr,
                        rule,
                        EntityType::Fare,
                        eid.clone(),
                        eid.clone(),
                        "fare_rules.txt",
                        Some(rec.line),
                        Some(field),
                        Some(zid.clone()),
                        None,
                        format!("{field} '{zid}' stops.txt'te tanımlı bir zone_id değil."),
                        "Geçerli bir zone_id kullanın.",
                    ));
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
                None,
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
        let mut rule_key_to_fare: HashMap<(Option<&str>, Option<&str>, Option<&str>, Option<&str>), &str> =
            HashMap::new();
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

    // RCT_006: fare_product başına birden fazla varsayılan (is_default_fare_category=1) rider_category
    {
        let default_rcids: HashSet<&str> = records
            .rider_categories
            .iter()
            .filter(|rc| rc.is_default_fare_category == Some(1))
            .map(|rc| rc.rider_category_id.as_str())
            .collect();

        if !default_rcids.is_empty() {
            let mut fp_defaults: HashMap<&str, Vec<u64>> = HashMap::new();
            for fp in &records.fare_products {
                if let Some(ref rcid) = fp.rider_category_id {
                    if default_rcids.contains(rcid.as_str()) {
                        fp_defaults.entry(fp.fare_product_id.as_str()).or_default().push(fp.line);
                    }
                }
            }
            for (fpid, lines) in fp_defaults {
                if lines.len() > 1 {
                    notices.push(notice(
                        ctr, "RCT_006", EntityType::Row,
                        Some(fpid.to_string()), Some(fpid.to_string()),
                        "fare_products.txt", lines.first().copied(), None,
                        Some(lines.len().to_string()), Some("1".to_string()),
                        format!(
                            "'{}' ucret urunu {} varsayilan yolcu kategorisine bagli; en fazla 1 olmali.",
                            fpid, lines.len()
                        ),
                        "Bir fare_product_id icin yalnizca bir rider_category is_default_fare_category=1 olmali.",
                    ));
                }
            }
        }
    }

    // FLG_001-006: fare_leg_rules cross-ref
    for rec in &records.fare_leg_rules {
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
                    format!("'{}' ag kodu networks.txt te tanimli degil.", nid),
                    "Gecerli bir network_id kullanin.",
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
    // TRN_007 agregasyonu: feed_lang ile aynı dildeki çeviriler (yaygın GTFS-JP araç pratiği —
    // her alanı ja/ja-Hrkt/en'e çevirir, ja kaynağı birebir tekrarlar) satır-başına on binlerce
    // notice yerine tek feed-seviyesi özette toplanır (STM_017/STP_022 emsali).
    let mut trn007_pending: Vec<Notice> = Vec::new();

    for rec in &records.translations {
        // TRN_004: record_id başvurulan kayıt bulunamadı
        if let Some(ref rid) = rec.record_id {
            let exists = match rec.table_name.as_str() {
                "agency" => map.agencies.contains_key(rid.as_str()),
                "stops" => map.stops.contains_key(rid.as_str()),
                "routes" => map.routes.contains_key(rid.as_str()),
                "trips" => map.trips.contains_key(rid.as_str()),
                "calendar" | "calendar_dates" => map.services.contains(rid.as_str()),
                "levels" => map.levels.contains_key(rid.as_str()),
                "pathways" => map.pathways.contains_key(rid.as_str()),
                "fare_attributes" => map.fare_attrs.contains_key(rid.as_str()),
                "feed_info" | "stop_times" | "frequencies" | "transfers"
                | "fare_rules" | "shapes" | "attributions" | "translations" => true, // lookup not feasible
                _ => true,
            };
            if !exists {
                notices.push(notice(
                    ctr,
                    "TRN_004",
                    EntityType::Translation,
                    None,
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
                    None,
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
                    None,
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

// �"?�"? ATR_005-007, ATR_009: attribution cross-ref �"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?

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

// ── JPN_001: GTFS-JP feed'inde durak adının kana (ja-Hrkt) okuması eksik ──────
// GTFS-JP, stop_name için かな okumasını (translations, language=ja-Hrkt) ZORUNLU kılar
// (sesli anons + arama için). Yalnız Japon feed'inde çalışır: feed_lang=ja VEYA herhangi
// bir ja-Hrkt çeviri varsa. Bir durağın kanası var sayılır ⇔ translations'ta
// (table=stops, field=stop_name, language=ja-Hrkt) record_id=stop_id VEYA
// field_value=stop_name ile eşleşen satır bulunur.
fn check_gtfs_jp(records: &EntityRecords, notices: &mut Vec<Notice>, ctr: &mut u32) {
    let ti_jpn = &records.trip_interns;
    // ── JPN_001: stop_name kana (ja-Hrkt) okuması — kapı: feed_lang ja* VEYA ja-Hrkt çeviri ──
    let feed_lang_ja = records.feed_info.first()
        .map(|fi| fi.feed_lang.to_lowercase().starts_with("ja"))
        .unwrap_or(false);
    let has_kana = records.translations.iter()
        .any(|t| t.language.eq_ignore_ascii_case("ja-Hrkt"));
    if feed_lang_ja || has_kana {
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

    // ── JPN_004: GTFS-JP feed'inde translations.txt zorunlu ──
    // GTFS-JP profili translations.txt'i (özellikle stop_name kana/ja-Hrkt okumaları için)
    // zorunlu kılar. Kapı: GTFS-JP sinyali (feed_lang ja* VEYA *_jp dosyası) AMA translations hiç yok.
    // has_kana zaten translations dolu demek → bu kuralla çelişmez.
    let is_gtfs_jp = feed_lang_ja
        || !records.office_jp.is_empty()
        || !records.agency_jp.is_empty();
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
            .filter(|a| a.agency_id.as_deref().map_or(true, |id| id.trim().is_empty()))
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

    // ── JPN_006: GTFS-JP'de fare_attributes.txt + fare_rules.txt zorunlu ──
    // FP riski (gerçekten ücretsiz hizmet olabilir) → Orta/Quality, feed-level tek uyarı.
    if is_gtfs_jp && (records.fare_attributes.is_empty() || records.fare_rules.is_empty()) {
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
            "GTFS-JP feed'inde fare_attributes.txt/fare_rules.txt eksik — profil ücret bilgisini zorunlu kılar.".to_string(),
            "fare_attributes.txt ve fare_rules.txt ekleyerek ücret bilgisini sağlayın.",
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

        // ATR_006: route_id referansı
        if let Some(ref rid) = rec.route_id {
            if !map.routes.contains_key(rid.as_str()) {
                notices.push(notice(
                    ctr,
                    "ATR_006",
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

        // ATR_007: trip_id referansı
        if let Some(ref tid) = rec.trip_id {
            if !map.trips.contains_key(tid.as_str()) {
                notices.push(notice(
                    ctr,
                    "ATR_007",
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
                None,
                None,
                Some("en fazla biri dolu olmalı (agency_id/route_id/trip_id)".to_string()),
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
    bad_stop_ids: &HashSet<&str>,
) {
    let ti_xfl = &records.trip_interns;
    // XFL_001: tüm service_id'ler calendar/calendar_dates'te tanımlı (feed-level özet)
    {
        let bad: Vec<&str> = records
            .trips
            .iter()
            .filter(|t| !ti_xfl.service_id(t).is_empty() && !map.services.contains(ti_xfl.service_id(t)))
            .map(|t| ti_xfl.service_id(t))
            .collect::<HashSet<_>>()
            .into_iter()
            .collect();
        if !bad.is_empty() {
            notices.push(notice(
                ctr,
                "XFL_001",
                EntityType::Feed,
                None,
                None,
                "trips.txt",
                None,
                Some("service_id"),
                Some(bad.join(", ")),
                None,
                format!(
                    "Şu service_id'ler calendar veya calendar_dates'te tanımlı değil: {}.",
                    bad.join(", ")
                ),
                "Eksik servis tanımlarını calendar.txt veya calendar_dates.txt'e ekleyin.",
            ));
        }
    }

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

    // XFL_005: stop_times'taki stop_id'ler stops'ta mevcut (feed-level özet)
    {
        if !bad_stop_ids.is_empty() {
            let bad_list: Vec<&str> = bad_stop_ids.iter().copied().collect();
            notices.push(notice(
                ctr,
                "XFL_005",
                EntityType::Feed,
                None,
                None,
                "stop_times.txt",
                None,
                Some("stop_id"),
                Some(bad_list.join(", ")),
                None,
                format!(
                    "stop_times.txt'te stops.txt'te tanımlı olmayan stop_id'ler var: {}.",
                    bad_list.join(", ")
                ),
                "Eksik durakları stops.txt'e ekleyin.",
            ));
        }
    }

    // XFL_007: routes'taki agency_id'ler agency'de mevcut (feed-level özet)
    {
        let bad: HashSet<String> = records
            .routes
            .iter()
            .filter(|r| {
                r.agency_id
                    .as_deref()
                    .map(|a| !a.is_empty() && !map.agencies.contains_key(a))
                    .unwrap_or(false)
            })
            .filter_map(|r| r.agency_id.clone())
            .collect();
        if !bad.is_empty() {
            let bad_list: Vec<&str> = bad.iter().map(String::as_str).collect();
            notices.push(notice(
                ctr,
                "XFL_007",
                EntityType::Feed,
                None,
                None,
                "routes.txt",
                None,
                Some("agency_id"),
                Some(bad_list.join(", ")),
                None,
                format!(
                    "routes.txt'te agency.txt'te tanımlı olmayan agency_id'ler var: {}.",
                    bad_list.join(", ")
                ),
                "Eksik işletici tanımlarını agency.txt'e ekleyin.",
            ));
        }
    }

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

    // XFL_009: stops'taki level_id'ler levels'ta mevcut
    {
        for rec in &records.stops {
            if let Some(ref lid) = rec.level_id {
                if !lid.is_empty() && !map.levels.contains_key(lid.as_str()) {
                    notices.push(notice(
                        ctr,
                        "XFL_009",
                        EntityType::Stop,
                        Some(rec.stop_id.clone()),
                        Some(rec.stop_id.clone()),
                        "stops.txt",
                        Some(rec.line),
                        Some("level_id"),
                        Some(lid.clone()),
                        None,
                        format!("level_id '{lid}' levels.txt'te tanımlı değil."),
                        "Geçerli bir level_id kullanın veya levels.txt'e bu kaydı ekleyin.",
                    ));
                }
            }
        }
    }

    // XFL_010: frequencies'teki trip_id'ler trips'te mevcut (feed-level özet)
    {
        let bad: HashSet<&str> = records
            .frequencies
            .iter()
            .filter(|f| !f.trip_id.is_empty() && !map.trips.contains_key(f.trip_id.as_str()))
            .map(|f| f.trip_id.as_str())
            .collect();
        if !bad.is_empty() {
            let bad_list: Vec<&str> = bad.into_iter().collect();
            notices.push(notice(
                ctr,
                "XFL_010",
                EntityType::Feed,
                None,
                None,
                "frequencies.txt",
                None,
                Some("trip_id"),
                Some(bad_list.join(", ")),
                None,
                format!(
                    "frequencies.txt'te trips.txt'te tanımlı olmayan trip_id'ler var: {}.",
                    bad_list.join(", ")
                ),
                "Eksik seferleri trips.txt'e ekleyin.",
            ));
        }
    }

    // XFL_003: trips'teki shape_id'ler shapes'te mevcut (feed-level özet)
    {
        let bad: HashSet<&str> = records
            .trips
            .iter()
            .filter_map(|t| ti_xfl.shape_id(t))
            .filter(|sid| !sid.is_empty() && !map.shape_points.contains_key(*sid))
            .collect();
        if !bad.is_empty() {
            let bad_list: Vec<&str> = bad.into_iter().collect();
            notices.push(notice(
                ctr,
                "XFL_003",
                EntityType::Feed,
                None,
                None,
                "trips.txt",
                None,
                Some("shape_id"),
                Some(bad_list.join(", ")),
                None,
                format!(
                    "trips.txt'te shapes.txt'te tanımlı olmayan shape_id'ler var: {}.",
                    bad_list.join(", ")
                ),
                "Eksik şekilleri shapes.txt'e ekleyin veya trips.txt'teki shape_id referanslarını kaldırın.",
            ));
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
                    .find(|sp| sp.shape_id.as_str() == *shape_id)
                    .map(|sp| sp.line);
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

    // XFL_004: fare_rules'daki route_id'ler routes'ta mevcut (feed-level özet)
    {
        let bad: HashSet<&str> = records
            .fare_rules
            .iter()
            .filter_map(|r| r.route_id.as_deref())
            .filter(|rid| !rid.is_empty() && !map.routes.contains_key(*rid))
            .collect();
        if !bad.is_empty() {
            let bad_list: Vec<&str> = bad.into_iter().collect();
            notices.push(notice(
                ctr,
                "XFL_004",
                EntityType::Feed,
                None,
                None,
                "fare_rules.txt",
                None,
                Some("route_id"),
                Some(bad_list.join(", ")),
                None,
                format!(
                    "fare_rules.txt'te routes.txt'te tanımlı olmayan route_id'ler var: {}.",
                    bad_list.join(", ")
                ),
                "Eksik rotaları routes.txt'e ekleyin veya fare_rules.txt'teki referansları kaldırın.",
            ));
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

            let mut details: HashMap<String, String> = HashMap::new();
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

    // XFL_014:�?eviri yapılan kayıt silinmiş veya tanımsız (dangling translation feed özeti)
    {
        let mut bad_keys: HashSet<String> = HashSet::new();
        for rec in &records.translations {
            if let Some(ref rid) = rec.record_id {
                if rid.is_empty() {
                    continue;
                }
                let exists = match rec.table_name.as_str() {
                    "agency" => map.agencies.contains_key(rid.as_str()),
                    "stops" => map.stops.contains_key(rid.as_str()),
                    "routes" => map.routes.contains_key(rid.as_str()),
                    "trips" => map.trips.contains_key(rid.as_str()),
                    "calendar" | "calendar_dates" => map.services.contains(rid.as_str()),
                    "levels" => map.levels.contains_key(rid.as_str()),
                    "pathways" => map.pathways.contains_key(rid.as_str()),
                    "fare_attributes" => map.fare_attrs.contains_key(rid.as_str()),
                    _ => true,
                };
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

    // XFL_019: network_id hem routes.txt hem route_networks.txt'te tanımlı → çakışma
    if records.has_route_networks_file {
        let has_route_network_id = records.routes.iter()
            .any(|r| r.network_id.as_deref().map_or(false, |n| !n.is_empty()));
        if has_route_network_id {
            notices.push(notice(
                ctr,
                "XFL_019",
                EntityType::Feed,
                None,
                None,
                "route_networks.txt",
                None,
                Some("network_id"),
                None,
                None,
                "routes.txt'te network_id tanımlı VE route_networks.txt dosyası da mevcut — ağ tanımı iki yerde çakışıyor.".to_string(),
                "Ağ atamasını yalnızca bir yöntemle yapın: ya routes.txt'teki network_id sütununu kullanın ya da route_networks.txt dosyasını kullanın.",
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
        if let Some(d) = sp.shape_dist_traveled {
            let e = shape_max_dist.entry(sp.shape_id.as_str()).or_insert(0.0_f64);
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
        if ratio > 2.5 || ratio < 0.4 {
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
    if let Some(&parent) = stop_parent.get(stop_id) {
        return Some(parent);
    }
    if stop_loc.get(stop_id).map_or(false, |t| *t == Some(1)) {
        return Some(stop_id);
    }
    None
}

// �"?�"? Testler �"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?

#[cfg(test)]
mod tests {
    use super::*;
    use crate::k2::agency::AgencyRecord;
    use crate::k2::attributions::AttributionRecord;
    use crate::k2::calendar::CalendarRecord;
    use crate::k2::fare_attributes::FareAttributeRecord;
    use crate::k2::fare_rules::FareRuleRecord;
    use crate::k2::frequencies::FrequencyRecord;
    use crate::k2::levels::LevelRecord;
    use crate::k2::pathways::PathwayRecord;
    use crate::k2::routes::RouteRecord;
    use crate::k2::stops::StopRecord;
    use crate::k2::stop_times::{StopTimeRecord, StopTimesIndex};
    use crate::k2::transfers::TransferRecord;
    use crate::k2::trips::{TripRecord, TripInternTable};

    fn empty() -> (EntityRecords, EntityMap) {
        (EntityRecords::default(), EntityMap::default())
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
            headsign_idx: 0, short_name_idx: 0, block_idx: 0, jp_office_idx: 0,
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

    #[test]
    fn invalid_agency_ref_in_route_produces_RTS_002() {
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
    fn route_with_no_trips_produces_RTS_012() {
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

    // �"?�"? XFL_009 �"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?

    #[test]
    fn stop_with_missing_level_produces_xfl_009() {
        let (mut recs, _map) = empty();
        recs.stops = vec![StopRecord {
            level_id: Some("MISSING_LEVEL".into()),
            ..stop("S1")
        }];
        let result = check(&recs, &EntityMap::default(), 20260515);
        assert!(result.notices.iter().any(|n| n.rule_id == "XFL_009"));
    }

    // �"?�"? AGN_005 �"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?

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
    fn trip_with_missing_shape_produces_xfl_003() {
        let (mut recs, map) = empty();
        let mut ti = TripInternTable::new();
        recs.trips = vec![trip_s(&mut ti, "T1", "R1", "SVC1", "MISSING_SHAPE", None)];
        recs.trip_interns = ti;
        let result = check(&recs, &map, 20260515);
        assert!(result.notices.iter().any(|n| n.rule_id == "XFL_003"));
    }

    // ── SHP_019: shape trip tarafından referanslanmış ama trip'lerin stop_times'ı yok ────────

    #[test]
    fn shape_with_trip_no_stm_produces_shp_019() {
        use crate::k2::shapes::ShapePointRecord;
        let (mut recs, mut map) = empty();
        map.routes.insert("R1".into(), 0);
        map.services.insert("SVC1".into());
        map.trips.insert("T1".into(), 0);
        let mut ti = TripInternTable::new();
        recs.trips = vec![trip_s(&mut ti, "T1", "R1", "SVC1", "S1", None)];
        recs.trip_interns = ti;
        recs.shapes = vec![ShapePointRecord {
            shape_id: "S1".into(),
            shape_pt_lat: Some(41.0), shape_pt_lon: Some(29.0),
            shape_pt_sequence: Some(1), shape_dist_traveled: None, line: 2,
        }];
        // stop_times yok → SHP_019 ateşlenmeli
        let result = check(&recs, &map, 20260515);
        assert!(result.notices.iter().any(|n| n.rule_id == "SHP_019"), "SHP_019 olmalı: trip var ama stop_times yok");
    }

    #[test]
    fn shape_with_trip_having_stm_no_shp_019() {
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
        recs.shapes = vec![ShapePointRecord {
            shape_id: "S1".into(),
            shape_pt_lat: Some(41.0), shape_pt_lon: Some(29.0),
            shape_pt_sequence: Some(1), shape_dist_traveled: None, line: 2,
        }];
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
        use crate::k2::shapes::ShapePointRecord;
        let (mut recs, map) = empty();
        recs.shapes = vec![ShapePointRecord {
            shape_id: "ORPHAN".into(),
            shape_pt_lat: Some(41.0), shape_pt_lon: Some(29.0),
            shape_pt_sequence: Some(1), shape_dist_traveled: None, line: 2,
        }];
        // Hiç trip yok → SHP_018 ateşler, SHP_019 ateşlemez
        let result = check(&recs, &map, 20260515);
        assert!(!result.notices.iter().any(|n| n.rule_id == "SHP_019"), "orphan shape SHP_019 değil SHP_018 üretmeli");
    }

    //�"?�"? XFL_004 �"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?�"?

    #[test]
    fn fare_rule_with_missing_route_produces_xfl_004() {
        use crate::k2::fare_rules::FareRuleRecord;
        let (mut recs, map) = empty();
        recs.fare_rules = vec![FareRuleRecord {
            fare_id: "F1".into(),
            route_id: Some("MISSING_ROUTE".into()),
            origin_id: None, destination_id: None, contains_id: None,
            row: Default::default(), line: 2,
        }];
        let result = check(&recs, &map, 20260515);
        assert!(result.notices.iter().any(|n| n.rule_id == "XFL_004"));
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
        assert!(result.notices.iter().any(|n| n.rule_id == "TRF_016"));
    }

    // ── RCT_006 ───────────────────────────────────────────────────────────────

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
                rider_category_eligibility_url: None,
                row: Default::default(), line: 2,
            },
            RiderCategoryRecord {
                rider_category_id: "senior".into(),
                rider_category_name: "Senior".into(),
                is_default_fare_category: Some(1),
                min_age: None, max_age: None,
                rider_category_eligibility_url: None,
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
                rider_category_eligibility_url: None,
                row: Default::default(), line: 2,
            },
            RiderCategoryRecord {
                rider_category_id: "child".into(),
                rider_category_name: "Child".into(),
                is_default_fare_category: Some(0),
                min_age: None, max_age: None,
                rider_category_eligibility_url: None,
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
}
