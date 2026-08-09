use std::collections::{HashMap, HashSet};

use gtfs_core::{EntityType, Notice};

use crate::k2::EntityRecords;

// ── Yardımcı ──────────────────────────────────────────────────────────────────

// K3 notice'ı canonical emit tablosuyla aynı sırada kurulur; parametre struct'ı
// benzersizlik kontrolünün kaynak alanlarını çağrı noktasında gizler.
#[allow(clippy::too_many_arguments)]
fn make_notice(
    counter: &mut u32,
    rule_id: &str,
    entity_type: EntityType,
    entity_id: Option<String>,
    scope_key: Option<String>,
    file: &str,
    line: u64,
    field: &str,
    observed_value: Option<String>,
    message: String,
    remediation: &str,
) -> Notice {
    // K3 yalnız benzersizlik (PK tekrarı) ihlali üretir → expected_value sabit "unique".
    crate::notice_factory::build(
        "K3", Some("k3"), counter, rule_id, entity_type, entity_id, scope_key,
        Some(file.to_string()), Some(line), Some(field.to_string()),
        observed_value, Some("unique".to_string()), message, remediation,
    )
}

// ── Tip tanımları ──────────────────────────────────────────────────────────────

/// K2 kayıtlarından türetilen entity lookup indeksleri.
///
/// Her harita: canonical entity ID → K2 records Vec'indeki indeks (ilk görünüm).
/// Tekrarlı PK ise notice üretilir; EntityMap'e yalnızca ilk kayıt eklenir.
#[derive(Debug, Default)]
pub struct EntityMap {
    /// agency_id → agencies indeksi
    pub agencies: HashMap<String, usize>,
    /// stop_id → stops indeksi
    pub stops: HashMap<String, usize>,
    /// route_id → routes indeksi
    pub routes: HashMap<String, usize>,
    /// trip_id → trips indeksi
    pub trips: HashMap<String, usize>,
    /// Geçerli service_id kümesi (calendar + calendar_dates birleşimi)
    pub services: HashSet<String>,
    /// shape_id → shape noktalarının sıralı indeks listesi (shape_pt_sequence artan)
    pub shape_points: HashMap<String, Vec<usize>>,
    /// pathway_id → pathways indeksi
    pub pathways: HashMap<String, usize>,
    /// level_id → levels indeksi
    pub levels: HashMap<String, usize>,
    /// fare_id → fare_attributes indeksi
    pub fare_attrs: HashMap<String, usize>,
    /// stops.txt'te tanımlı zone_id kümesi (fare_rules cross-ref için)
    pub zone_ids: HashSet<String>,
    // ── Fares v2 ──────────────────────────────────────────────────────────────
    /// area_id → areas indeksi
    pub areas: HashMap<String, usize>,
    /// network_id kümesi
    pub network_ids: HashSet<String>,
    /// rider_category_id kümesi
    pub rider_category_ids: HashSet<String>,
    /// fare_media_id → fare_media indeksi
    pub fare_media_ids: HashMap<String, usize>,
    /// fare_product_id → fare_products indeksi
    pub fare_product_ids: HashMap<String, usize>,
    /// fare_leg_rules'tan toplanan leg_group_id kümesi
    pub leg_group_ids: HashSet<String>,
    /// timeframes'ten toplanan timeframe_group_id kümesi
    pub timeframe_group_ids: HashSet<String>,
    /// location_groups'tan toplanan location_group_id kümesi (GTFS-Flex)
    pub location_group_ids: HashSet<String>,
    /// locations.geojson feature id kümesi (XFL_025; k1'den lib.rs'te enjekte edilir)
    pub geojson_location_ids: HashSet<String>,
    /// locations.geojson id → geometri (Pa1fdaa0d, K6'da örtüşme denetimi).
    pub geojson_geometries: HashMap<String, crate::k1_parse::LocationGeometry>,
}

/// K3 çıktısı.
#[derive(Debug, Default)]
pub struct K3Result {
    pub entity_map: EntityMap,
    pub notices: Vec<Notice>,
}

// ── Ana fonksiyon ─────────────────────────────────────────────────────────────

/// K2 entity kayıtlarından lookup indeksleri inşa eder; duplicate PK notice üretir.
pub fn build(records: &EntityRecords) -> K3Result {
    let mut map = EntityMap::default();
    let mut notices = Vec::new();
    let mut ctr = 0u32;

    build_agencies(records, &mut map, &mut notices, &mut ctr);
    build_stops(records, &mut map, &mut notices, &mut ctr);
    build_routes(records, &mut map, &mut notices, &mut ctr);
    build_trips(records, &mut map, &mut notices, &mut ctr);
    build_services(records, &mut map, &mut notices, &mut ctr);
    build_shapes(records, &mut map);
    build_pathways(records, &mut map, &mut notices, &mut ctr);
    build_levels(records, &mut map, &mut notices, &mut ctr);
    build_fare_attrs(records, &mut map, &mut notices, &mut ctr);
    build_fares_v2(records, &mut map, &mut notices, &mut ctr);
    build_location_groups(records, &mut map);

    K3Result { entity_map: map, notices }
}

/// location_groups.txt'ten geçerli location_group_id kümesini toplar (XFL_022/024 için).
fn build_location_groups(records: &EntityRecords, map: &mut EntityMap) {
    for rec in &records.location_groups {
        if !rec.location_group_id.is_empty() {
            map.location_group_ids.insert(rec.location_group_id.clone());
        }
    }
}

// ── AGN_010: agency_id tekil ──────────────────────────────────────────────────

fn build_agencies(
    records: &EntityRecords,
    map: &mut EntityMap,
    notices: &mut Vec<Notice>,
    ctr: &mut u32,
) {
    for (idx, rec) in records.agencies.iter().enumerate() {
        let Some(ref aid) = rec.agency_id else {
            // agency_id yok → duplicate (AGN_010) indeksine eklenmez (tekillik anahtarı yok).
            continue;
        };
        if let Some(&prev_idx) = map.agencies.get(aid.as_str()) {
            let prev_line = records.agencies[prev_idx].line;
            notices.push(make_notice(
                ctr,
                "AGN_010",
                EntityType::Agency,
                Some(aid.clone()),
                Some(aid.clone()),
                "agency.txt",
                rec.line,
                "agency_id",
                Some(aid.clone()),
                format!(
                    "agency_id '{aid}' tekrarlanıyor (ilk görünüm: satır {prev_line})."
                ),
                "Her işleticiye benzersiz bir agency_id atayın.",
            ));
        } else {
            map.agencies.insert(aid.clone(), idx);
        }
    }
}

// ── STP_001: stop_id tekil ────────────────────────────────────────────────────

fn build_stops(
    records: &EntityRecords,
    map: &mut EntityMap,
    notices: &mut Vec<Notice>,
    ctr: &mut u32,
) {
    for (idx, rec) in records.stops.iter().enumerate() {
        let sid = &rec.stop_id;
        if sid.is_empty() {
            continue;
        }
        // zone_id indeksi (fare cross-ref)
        if let Some(ref zid) = rec.row.get("zone_id").and_then(|v| {
            let t = v.trim();
            if t.is_empty() { None } else { Some(t.to_string()) }
        }) {
            map.zone_ids.insert(zid.clone());
        }
        if let Some(&prev_idx) = map.stops.get(sid.as_str()) {
            let prev_line = records.stops[prev_idx].line;
            notices.push(make_notice(
                ctr,
                "STP_001",
                EntityType::Stop,
                Some(sid.clone()),
                Some(sid.clone()),
                "stops.txt",
                rec.line,
                "stop_id",
                Some(sid.clone()),
                format!("'{}' durağı tekrarlanıyor (ilk görünüm: satır {prev_line}).", sid),
                "Her durağa benzersiz bir stop_id atayın.",
            ));
        } else {
            map.stops.insert(sid.clone(), idx);
        }
    }
}

// ── RTS_001: route_id tekil ───────────────────────────────────────────────────

fn build_routes(
    records: &EntityRecords,
    map: &mut EntityMap,
    notices: &mut Vec<Notice>,
    ctr: &mut u32,
) {
    for (idx, rec) in records.routes.iter().enumerate() {
        let rid = &rec.route_id;
        if rid.is_empty() {
            continue;
        }
        if let Some(&prev_idx) = map.routes.get(rid.as_str()) {
            let prev_line = records.routes[prev_idx].line;
            notices.push(make_notice(
                ctr,
                "RTS_001",
                EntityType::Route,
                Some(rid.clone()),
                Some(rid.clone()),
                "routes.txt",
                rec.line,
                "route_id",
                Some(rid.clone()),
                format!("'{}' hattı tekrarlanıyor (ilk görünüm: satır {prev_line}).", rid),
                "Her rotaya benzersiz bir route_id atayın.",
            ));
        } else {
            map.routes.insert(rid.clone(), idx);
        }
    }
}

// ── TRP_001: trip_id tekil ────────────────────────────────────────────────────

fn build_trips(
    records: &EntityRecords,
    map: &mut EntityMap,
    notices: &mut Vec<Notice>,
    ctr: &mut u32,
) {
    for (idx, rec) in records.trips.iter().enumerate() {
        let tid = &rec.trip_id;
        if tid.is_empty() {
            continue;
        }
        if let Some(&prev_idx) = map.trips.get(tid.as_str()) {
            let prev_line = records.trips[prev_idx].line;
            let tid_s = tid.to_string();
            notices.push(make_notice(
                ctr,
                "TRP_001",
                EntityType::Trip,
                Some(tid_s.clone()),
                Some(tid_s.clone()),
                "trips.txt",
                rec.line,
                "trip_id",
                Some(tid_s),
                format!("'{}' sefer kodu tekrarlanıyor (ilk görünüm: satır {prev_line}).", tid),
                "Her sefere benzersiz bir trip_id atayın.",
            ));
        } else {
            map.trips.insert(tid.to_string(), idx);
        }
    }
}

// ── Servis ID kümesi (calendar + calendar_dates) ──────────────────────────────

fn build_services(
    records: &EntityRecords,
    map: &mut EntityMap,
    notices: &mut Vec<Notice>,
    ctr: &mut u32,
) {
    // CAL_001: service_id tekil (calendar.txt)
    let mut seen_cal: HashMap<String, u64> = HashMap::new();
    for rec in &records.calendars {
        let sid = &rec.service_id;
        if sid.is_empty() {
            continue;
        }
        if let Some(&prev_line) = seen_cal.get(sid.as_str()) {
            notices.push(make_notice(
                ctr,
                "CAL_001",
                EntityType::Service,
                Some(sid.clone()),
                Some(sid.clone()),
                "calendar.txt",
                rec.line,
                "service_id",
                Some(sid.clone()),
                format!("'{}' takvim kodu tekrarlanıyor (ilk görünüm: satır {prev_line}).", sid),
                "Her hizmet kaydına benzersiz bir service_id atayın.",
            ));
        } else {
            seen_cal.insert(sid.clone(), rec.line);
            map.services.insert(sid.clone());
        }
    }

    // calendar_dates'ten service_id ekle (CLD_001 zaten K2'de kontrol edildi).
    // exception_count tüm non-empty service_id'leri kapsar (geçersiz tarih/type dahil).
    for svc in records.calendar_dates.exception_count.keys() {
        map.services.insert(svc.to_string());
    }
}

// ── Shape noktaları indeksi (sequence sıralı) ─────────────────────────────────

fn build_shapes(records: &EntityRecords, map: &mut EntityMap) {
    // Önce her shape_id için ham indeksleri topla
    let mut raw: HashMap<&str, Vec<usize>> = HashMap::new();
    for (idx, rec) in records.shapes.iter().enumerate() {
        if rec.shape_idx() == 0 {
            continue;
        }
        raw.entry(records.shape_interns.id(rec)).or_default().push(idx);
    }

    // Her shape'i shape_pt_sequence'e göre sırala
    for (shape_id, mut indices) in raw {
        indices.sort_by_key(|&i| records.shapes[i].shape_pt_sequence().unwrap_or(u32::MAX));
        map.shape_points.insert(shape_id.to_string(), indices);
    }
}

// ── PTH_001: pathway_id tekil ────────────────────────────────────────────────

fn build_pathways(
    records: &EntityRecords,
    map: &mut EntityMap,
    notices: &mut Vec<Notice>,
    ctr: &mut u32,
) {
    for (idx, rec) in records.pathways.iter().enumerate() {
        let pid = &rec.pathway_id;
        if pid.is_empty() {
            continue;
        }
        if let Some(&prev_idx) = map.pathways.get(pid.as_str()) {
            let prev_line = records.pathways[prev_idx].line;
            notices.push(make_notice(
                ctr,
                "PTH_001",
                EntityType::Pathway,
                Some(pid.clone()),
                Some(pid.clone()),
                "pathways.txt",
                rec.line,
                "pathway_id",
                Some(pid.clone()),
                format!("pathway_id '{pid}' tekrarlanıyor (ilk görünüm: satır {prev_line})."),
                "Her geçide benzersiz bir pathway_id atayın.",
            ));
        } else {
            map.pathways.insert(pid.clone(), idx);
        }
    }
}

// ── LVL_001: level_id tekil ───────────────────────────────────────────────────

fn build_levels(
    records: &EntityRecords,
    map: &mut EntityMap,
    notices: &mut Vec<Notice>,
    ctr: &mut u32,
) {
    for (idx, rec) in records.levels.iter().enumerate() {
        let lid = &rec.level_id;
        if lid.is_empty() {
            continue;
        }
        if let Some(&prev_idx) = map.levels.get(lid.as_str()) {
            let prev_line = records.levels[prev_idx].line;
            notices.push(make_notice(
                ctr,
                "LVL_001",
                EntityType::Level,
                Some(lid.clone()),
                Some(lid.clone()),
                "levels.txt",
                rec.line,
                "level_id",
                Some(lid.clone()),
                format!("level_id '{lid}' tekrarlanıyor (ilk görünüm: satır {prev_line})."),
                "Her katmana benzersiz bir level_id atayın.",
            ));
        } else {
            map.levels.insert(lid.clone(), idx);
        }
    }
}

// ── FAR_001: fare_id tekil ────────────────────────────────────────────────────

fn build_fare_attrs(
    records: &EntityRecords,
    map: &mut EntityMap,
    notices: &mut Vec<Notice>,
    ctr: &mut u32,
) {
    for (idx, rec) in records.fare_attributes.iter().enumerate() {
        let fid = &rec.fare_id;
        if fid.is_empty() {
            continue;
        }
        if let Some(&prev_idx) = map.fare_attrs.get(fid.as_str()) {
            let prev_line = records.fare_attributes[prev_idx].line;
            notices.push(make_notice(
                ctr,
                "FAR_001",
                EntityType::Fare,
                Some(fid.clone()),
                Some(fid.clone()),
                "fare_attributes.txt",
                rec.line,
                "fare_id",
                Some(fid.clone()),
                format!("'{}' ücret tarifesi tekrarlanıyor (ilk görünüm: satır {prev_line}).", fid),
                "Her tarife kaydına benzersiz bir fare_id atayın.",
            ));
        } else {
            map.fare_attrs.insert(fid.clone(), idx);
        }
    }
}

// ── Fares v2: areas, networks, rider_categories, fare_media, fare_products ────

fn build_fares_v2(
    records: &EntityRecords,
    map: &mut EntityMap,
    notices: &mut Vec<Notice>,
    ctr: &mut u32,
) {
    // ARS_001: area_id tekil
    for (idx, rec) in records.areas.iter().enumerate() {
        if rec.area_id.is_empty() {
            continue;
        }
        if let Some(&prev_idx) = map.areas.get(rec.area_id.as_str()) {
            let prev_line = records.areas[prev_idx].line;
            notices.push(make_notice(
                ctr, "ARS_001", EntityType::Row,
                Some(rec.area_id.clone()), Some(rec.area_id.clone()),
                "areas.txt", rec.line, "area_id",
                Some(rec.area_id.clone()),
                format!("'{}' alan kodu tekrarlanıyor (ilk görünüm: satır {prev_line}).", rec.area_id),
                "Her alana benzersiz bir area_id atayın.",
            ));
        } else {
            map.areas.insert(rec.area_id.clone(), idx);
        }
    }

    // NET_001: network_id tekil
    for rec in &records.networks {
        if rec.network_id.is_empty() {
            continue;
        }
        if map.network_ids.contains(rec.network_id.as_str()) {
            notices.push(make_notice(
                ctr, "NET_001", EntityType::Row,
                Some(rec.network_id.clone()), Some(rec.network_id.clone()),
                "networks.txt", rec.line, "network_id",
                Some(rec.network_id.clone()),
                format!("'{}' ağ kodu tekrarlanıyor.", rec.network_id),
                "Her ağa benzersiz bir network_id atayın.",
            ));
        } else {
            map.network_ids.insert(rec.network_id.clone());
        }
    }

    // network_id yalnız networks.txt'te değil, routes.txt'teki network_id sütunuyla ve
    // route_networks.txt satırlarıyla da tanımlanabilir (GTFS Fares v2). NET_001 döngüsünden
    // SONRA eklenir; böylece NET_001 tekillik kontrolü yalnız networks.txt satırlarını görür,
    // FLG_002 ise tam kümeye bakar.
    for rec in &records.routes {
        if let Some(nid) = rec.network_id.as_deref().filter(|s| !s.is_empty()) {
            map.network_ids.insert(nid.to_string());
        }
    }
    for rec in &records.route_networks {
        if !rec.network_id.is_empty() {
            map.network_ids.insert(rec.network_id.clone());
        }
    }

    // RCT_001: rider_category_id tekil
    for rec in &records.rider_categories {
        if rec.rider_category_id.is_empty() {
            continue;
        }
        if map.rider_category_ids.contains(rec.rider_category_id.as_str()) {
            notices.push(make_notice(
                ctr, "RCT_001", EntityType::Row,
                Some(rec.rider_category_id.clone()), Some(rec.rider_category_id.clone()),
                "rider_categories.txt", rec.line, "rider_category_id",
                Some(rec.rider_category_id.clone()),
                format!("'{}' yolcu kategorisi tekrarlanıyor.", rec.rider_category_id),
                "Her yolcu kategorisine benzersiz bir rider_category_id atayın.",
            ));
        } else {
            map.rider_category_ids.insert(rec.rider_category_id.clone());
        }
    }

    // FMD_001: fare_media_id tekil
    for (idx, rec) in records.fare_media.iter().enumerate() {
        if rec.fare_media_id.is_empty() {
            continue;
        }
        if let Some(&prev_idx) = map.fare_media_ids.get(rec.fare_media_id.as_str()) {
            let prev_line = records.fare_media[prev_idx].line;
            notices.push(make_notice(
                ctr, "FMD_001", EntityType::Row,
                Some(rec.fare_media_id.clone()), Some(rec.fare_media_id.clone()),
                "fare_media.txt", rec.line, "fare_media_id",
                Some(rec.fare_media_id.clone()),
                format!("'{}' ödeme aracı tekrarlanıyor (ilk görünüm: satır {prev_line}).", rec.fare_media_id),
                "Her ödeme aracına benzersiz bir fare_media_id atayın.",
            ));
        } else {
            map.fare_media_ids.insert(rec.fare_media_id.clone(), idx);
        }
    }

    // FPD_001: fare_products PK = (fare_product_id, rider_category_id, fare_media_id) BİLEŞİK.
    // Aynı fare_product_id farklı rider_category_id/fare_media_id ile MEŞRU tekrar eder (GTFS spec:
    // "Multiple records sharing the same fare_product_id are permitted as long as they contain
    // different fare_media_ids or rider_category_ids"). Yalnız TAM bileşik anahtar tekrarı hatadır.
    // FK varlık haritası (map.fare_product_ids) AYRI: id→ilk idx; FLG_001/FTR_004 "bulunamadı"
    // lookup'ı id bazında olduğundan id ile tutulur (bozulmaz).
    let mut seen_pk: HashMap<(&str, &str, &str), u64> = HashMap::new();
    for (idx, rec) in records.fare_products.iter().enumerate() {
        if rec.fare_product_id.is_empty() {
            continue;
        }
        map.fare_product_ids.entry(rec.fare_product_id.clone()).or_insert(idx);
        let pk = (
            rec.fare_product_id.as_str(),
            rec.rider_category_id.as_deref().unwrap_or(""),
            rec.fare_media_id.as_deref().unwrap_or(""),
        );
        if let Some(&prev_line) = seen_pk.get(&pk) {
            notices.push(make_notice(
                ctr, "FPD_001", EntityType::Row,
                Some(rec.fare_product_id.clone()), Some(rec.fare_product_id.clone()),
                "fare_products.txt", rec.line, "fare_product_id",
                Some(rec.fare_product_id.clone()),
                format!("'{}' ücret ürünü aynı (rider_category_id, fare_media_id) kombinasyonuyla tekrarlanıyor (ilk görünüm: satır {prev_line}).", rec.fare_product_id),
                "Her (fare_product_id, rider_category_id, fare_media_id) kombinasyonu benzersiz olmalıdır.",
            ));
        } else {
            seen_pk.insert(pk, rec.line);
        }
    }

    // leg_group_id ve timeframe_group_id kümelerini topla (K4 cross-ref için)
    for rec in &records.fare_leg_rules {
        if let Some(ref lgid) = rec.leg_group_id {
            if !lgid.is_empty() {
                map.leg_group_ids.insert(lgid.clone());
            }
        }
    }

    for rec in &records.timeframes {
        if !rec.timeframe_group_id.is_empty() {
            map.timeframe_group_ids.insert(rec.timeframe_group_id.clone());
        }
    }
}

// ── Testler ───────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::k2::agency::AgencyRecord;
    use crate::k2::calendar::CalendarRecord;
    use crate::k2::calendar_dates::CalendarDateIndex;
    use crate::k2::fare_attributes::FareAttributeRecord;
    use crate::k2::levels::LevelRecord;
    use crate::k2::pathways::PathwayRecord;
    use crate::k2::routes::RouteRecord;
    use crate::k2::shapes::ShapePointRecord;
    use crate::k2::stops::StopRecord;
    use crate::k2::trips::TripRecord;
    use crate::k2::shapes::ShapeInternTable;

    fn empty_records() -> EntityRecords {
        EntityRecords::default()
    }

    // ── AGN_010 ────────────────────────────────────────────────────────────────

    #[test]
    fn duplicate_agency_id_produces_agn_010() {
        let mut records = empty_records();
        records.agencies = vec![
            AgencyRecord {
                agency_id: Some("A1".into()),
                agency_name: "Agency One".into(),
                agency_url: "https://example.com".into(),
                agency_timezone: "Europe/Istanbul".into(),
                agency_lang: None,
                agency_phone: None,
                agency_fare_url: None,
                agency_email: None,
                agency_cemv_support: None,
                row: Default::default(),
                line: 2,
            },
            AgencyRecord {
                agency_id: Some("A1".into()),
                agency_name: "Agency Duplicate".into(),
                agency_url: "https://example.com".into(),
                agency_timezone: "Europe/Istanbul".into(),
                agency_lang: None,
                agency_phone: None,
                agency_fare_url: None,
                agency_email: None,
                agency_cemv_support: None,
                row: Default::default(),
                line: 3,
            },
        ];
        let result = build(&records);
        assert!(result.notices.iter().any(|n| n.rule_id == "AGN_010"));
        // Canonical: ilk kayıt indekste olmalı
        assert_eq!(*result.entity_map.agencies.get("A1").unwrap(), 0);
    }

    #[test]
    fn fare_product_composite_key_not_flagged_fpd_001() {
        use crate::k2::fare_products::FareProductRecord;
        let mk = |id: &str, rc: Option<&str>, fm: Option<&str>, line: u64| FareProductRecord {
            fare_product_id: id.into(),
            fare_product_name: None,
            rider_category_id: rc.map(|s| s.into()),
            fare_media_id: fm.map(|s| s.into()),
            amount: Some(2.0),
            currency: "USD".into(),
            row: Default::default(),
            line,
        };
        // Aynı fare_product_id, FARKLI rider_category → bileşik PK farklı → MEŞRU, FPD_001 YOK.
        let mut records = empty_records();
        records.fare_products = vec![
            mk("P1", Some("adult"), None, 2),
            mk("P1", Some("child"), None, 3),
        ];
        let r = build(&records);
        assert!(!r.notices.iter().any(|n| n.rule_id == "FPD_001"),
            "aynı id farklı rider_category → FPD_001 olmamalı (bileşik PK)");
        assert!(r.entity_map.fare_product_ids.contains_key("P1"),
            "FK varlık haritası id'yi içermeli (FLG/FTR lookup)");

        // TAM bileşik anahtar tekrarı (id+rider+media aynı) → FPD_001.
        let mut records2 = empty_records();
        records2.fare_products = vec![
            mk("P1", Some("adult"), Some("card"), 2),
            mk("P1", Some("adult"), Some("card"), 3),
        ];
        let r2 = build(&records2);
        assert!(r2.notices.iter().any(|n| n.rule_id == "FPD_001"),
            "tam bileşik anahtar tekrarı → FPD_001");
    }

    #[test]
    fn unique_agency_ids_build_index_without_notices() {
        let mut records = empty_records();
        records.agencies = vec![
            AgencyRecord {
                agency_id: Some("A1".into()),
                agency_name: "Agency One".into(),
                agency_url: "https://example.com".into(),
                agency_timezone: "Europe/Istanbul".into(),
                agency_lang: None, agency_phone: None,
                agency_fare_url: None, agency_email: None,
                agency_cemv_support: None,
                row: Default::default(), line: 2,
            },
            AgencyRecord {
                agency_id: Some("A2".into()),
                agency_name: "Agency Two".into(),
                agency_url: "https://example.com".into(),
                agency_timezone: "Europe/Istanbul".into(),
                agency_lang: None, agency_phone: None,
                agency_fare_url: None, agency_email: None,
                agency_cemv_support: None,
                row: Default::default(), line: 3,
            },
        ];
        let result = build(&records);
        assert!(result.notices.is_empty());
        assert_eq!(result.entity_map.agencies.len(), 2);
    }

    // ── STP_001 ────────────────────────────────────────────────────────────────

    #[test]
    fn duplicate_stop_id_produces_stp_001() {
        let mut records = empty_records();
        let base = StopRecord {
            stop_id: "S1".into(),
            stop_code: None, stop_name: None,
            stop_lat: None, stop_lon: None,
            location_type: None, stop_timezone: None,
            wheelchair_boarding: None, stop_access: None,
            level_id: None, tts_stop_name: None,
            row: Default::default(), line: 2,
            ..Default::default()
        };
        records.stops = vec![
            base.clone(),
            StopRecord { line: 3, ..base },
        ];
        let result = build(&records);
        assert!(result.notices.iter().any(|n| n.rule_id == "STP_001"));
        assert_eq!(*result.entity_map.stops.get("S1").unwrap(), 0);
    }

    // ── RTS_001 ────────────────────────────────────────────────────────────────

    #[test]
    fn duplicate_route_id_produces_RTS_001() {
        let mut records = empty_records();
        let base = RouteRecord {
            route_id: "R1".into(),
            agency_id: None,
            route_short_name: None, route_long_name: None,
            route_desc: None, route_type: Some(3),
            route_url: None, route_color: None,
            route_text_color: None, route_sort_order: None,
            continuous_pickup: None, continuous_drop_off: None,
            network_id: None, route_cemv_support: None, jp_office_id: None,
            row: Default::default(), line: 2,
        };
        records.routes = vec![
            base.clone(),
            RouteRecord { line: 3, ..base },
        ];
        let result = build(&records);
        assert!(result.notices.iter().any(|n| n.rule_id == "RTS_001"));
    }

    // ── TRP_001 ────────────────────────────────────────────────────────────────

    #[test]
    fn duplicate_trip_id_produces_trp_001() {
        let mut records = empty_records();
        let base = TripRecord {
            trip_id: "T1".into(),
            route_idx: 0, service_idx: 0, shape_idx: 0,
            headsign_idx: 0, short_name_idx: 0, block_idx: 0, jp_office_idx: 0,
            direction_id: None, wheelchair_accessible: None,
            bikes_allowed: None, cars_allowed: None,
            safe_duration_factor: None, safe_duration_offset: None,
            line: 2,
        };
        records.trips = vec![
            base.clone(),
            TripRecord { line: 3, ..base },
        ];
        let result = build(&records);
        assert!(result.notices.iter().any(|n| n.rule_id == "TRP_001"));
    }

    // ── CAL_001 ────────────────────────────────────────────────────────────────

    #[test]
    fn duplicate_service_id_in_calendar_produces_cal_001() {
        let mut records = empty_records();
        let base = CalendarRecord {
            service_id: "WKD".into(),
            days: [Some(1); 7],
            start_date: None, end_date: None,
            row: Default::default(), line: 2,
        };
        records.calendars = vec![
            base.clone(),
            CalendarRecord { line: 3, ..base },
        ];
        let result = build(&records);
        assert!(result.notices.iter().any(|n| n.rule_id == "CAL_001"));
    }

    // ── Service set ───────────────────────────────────────────────────────────

    #[test]
    fn services_merged_from_calendar_and_calendar_dates() {
        let mut records = empty_records();
        records.calendars = vec![CalendarRecord {
            service_id: "WKD".into(),
            days: [Some(1); 7],
            start_date: None, end_date: None,
            row: Default::default(), line: 2,
        }];
        // "HOL": exception_type=None → yalnızca exception_count'ta (added/removed'da değil)
        // Boş service_id: exception_count'a GİRMEZ → services kümesine eklenmemeli
        let mut idx = CalendarDateIndex::default();
        idx.exception_count.insert("HOL".into(), 1);
        records.calendar_dates = idx;
        let result = build(&records);
        assert!(result.entity_map.services.contains("WKD"));
        assert!(result.entity_map.services.contains("HOL"));
        assert!(!result.entity_map.services.contains(""));
    }

    // ── Shape indeksi ─────────────────────────────────────────────────────────

    #[test]
    fn shape_points_sorted_by_sequence() {
        let mut shape_ti = ShapeInternTable::new();
        let mut records = empty_records();
        records.shapes = vec![
            ShapePointRecord::new(shape_ti.intern("SHP1"), Some(41.0), Some(29.0), Some(2), None, 3),
            ShapePointRecord::new(shape_ti.intern("SHP1"), Some(41.1), Some(29.1), Some(1), None, 2),
        ];
        records.shape_interns = shape_ti.clone();
        let result = build(&records);
        let pts = result.entity_map.shape_points.get("SHP1").unwrap();
        // Sequence 1 → records.shapes[1], sequence 2 → records.shapes[0]
        assert_eq!(pts[0], 1);
        assert_eq!(pts[1], 0);
    }

    // ── PTH_001 ────────────────────────────────────────────────────────────────

    #[test]
    fn duplicate_pathway_id_produces_pth_001() {
        let mut records = empty_records();
        let base = PathwayRecord {
            pathway_id: "P1".into(),
            from_stop_id: "S1".into(),
            to_stop_id: "S2".into(),
            pathway_mode: Some(1),
            is_bidirectional: Some(1),
            length: None, traversal_time: None,
            stair_count: None, max_slope: None, min_width: None,
            row: Default::default(), line: 2,
        };
        records.pathways = vec![
            base.clone(),
            PathwayRecord { line: 3, ..base },
        ];
        let result = build(&records);
        assert!(result.notices.iter().any(|n| n.rule_id == "PTH_001"));
    }

    // ── LVL_001 ────────────────────────────────────────────────────────────────

    #[test]
    fn duplicate_level_id_produces_lvl_001() {
        let mut records = empty_records();
        let base = LevelRecord {
            level_id: "L1".into(),
            level_index: Some(0.0), level_name: None,
            row: Default::default(), line: 2,
        };
        records.levels = vec![
            base.clone(),
            LevelRecord { line: 3, ..base },
        ];
        let result = build(&records);
        assert!(result.notices.iter().any(|n| n.rule_id == "LVL_001"));
    }

    // ── FAR_001 ────────────────────────────────────────────────────────────────

    #[test]
    fn duplicate_fare_id_produces_far_001() {
        let mut records = empty_records();
        let base = FareAttributeRecord {
            fare_id: "F1".into(),
            price: Some(2.0),
            currency_type: "TRY".into(),
            payment_method: Some(0),
            transfers: None, transfer_duration: None,
            agency_id: None,
            row: Default::default(), line: 2,
        };
        records.fare_attributes = vec![
            base.clone(),
            FareAttributeRecord { line: 3, ..base },
        ];
        let result = build(&records);
        assert!(result.notices.iter().any(|n| n.rule_id == "FAR_001"));
    }

    // ── Boş kayıt seti ────────────────────────────────────────────────────────

    #[test]
    fn empty_records_produces_empty_result() {
        let result = build(&empty_records());
        assert!(result.notices.is_empty());
        assert!(result.entity_map.agencies.is_empty());
        assert!(result.entity_map.stops.is_empty());
        assert!(result.entity_map.services.is_empty());
    }

    // ── zone_id kümesi ────────────────────────────────────────────────────────

    #[test]
    fn zone_ids_collected_from_stops() {
        let mut records = empty_records();
        let mut row: std::collections::HashMap<String, String> = Default::default();
        row.insert("zone_id".into(), "ZoneA".into());
        records.stops = vec![StopRecord {
            stop_id: "S1".into(),
            stop_code: None, stop_name: None,
            stop_lat: None, stop_lon: None,
            location_type: None, stop_timezone: None,
            wheelchair_boarding: None, stop_access: None,
            level_id: None, tts_stop_name: None,
            row,
            line: 2,
            ..Default::default()
        }];
        let result = build(&records);
        assert!(result.entity_map.zone_ids.contains("ZoneA"));
    }
}
