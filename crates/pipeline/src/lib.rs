pub mod k1_parse;
pub mod k2;
pub mod k3_entity_graph;
pub mod k4_cross_ref;
pub mod k5_derived;
pub mod k6_analytics;
pub mod k7_reporting;
pub(crate) mod timing;

pub use k1_parse::{parse, K1Result, RawFile, RawFiles};
pub use k2::{validate as validate_k2, EntityRecords, K2Result};
pub use k3_entity_graph::{build as build_entity_map, EntityMap, K3Result};
pub use k4_cross_ref::{check as check_cross_ref, K4Result};
pub use k5_derived::{build as build_derived, DerivedData, K5Result};
pub use k6_analytics::{analyze as analyze_k6, K6Result};
pub use k7_reporting::{report as report_k7, K7Result};

// Entegrasyon testlerine açık yeniden ihracat
pub use gtfs_config::{CalendarOverrideRule, ValidatorConfig};
pub use gtfs_core::{FatalCode, FatalError, FileInfo, ValidateResult, ValidationResult};

/// K1–K7 tam pipeline — entegrasyon testleri ve araç entegrasyonu için.
/// WASM sürümünden farkı: notice limit yok, `today` dışarıdan verilir.
pub fn validate_bytes(zip: &[u8], config: &ValidatorConfig, today: u32) -> ValidateResult {
    use crate::timing::Timer;

    let k1 = {
        let _t = Timer::start("K1-parse");
        match parse(zip) {
            Ok(r) => r,
            Err(e) => return ValidateResult::Fatal(e),
        }
    };
    let mut file_stats = collect_file_stats(&k1.files);

    let mut k2 = {
        let _t = Timer::start("K2-validate");
        validate_k2(k1.files, Some(zip)) // #15 W2 + #38: ZIP bytes K2'ye → stop_times stream
    };

    // Gece yarısını aşan seferleri (00:xx) servis-günü notasyonuna (24:xx) normalize et.
    // K2 format kuralları (STM_004/007) parse anında ham raw ile çalıştı; bu pass yalnızca
    // K3–K6 türetilmiş/analitik kuralların (STM_008/014/028, headway…) tutarlı görmesini sağlar.
    // Monoton seferlerde no-op; yalnızca gece dönümü içeren seferler kayar.
    {
        let _t = Timer::start("K2-service-day-normalize");
        k2.records.stop_times_index.normalize_service_day(config.service_day_start_hour);
    }

    // OOM fix Plan A: stop_times.txt K1'de stream edildiği için RawFile.rows boştur;
    // gerçek satır sayısı K2 index'inde. file_stats'taki 0 değerini düzelt.
    for fi in file_stats.iter_mut() {
        if fi.name == "stop_times.txt" {
            fi.rows = k2.records.stop_times_index.total_rows as u32;
        } else if fi.name == "shapes.txt" {
            // #15 W3: shapes da stream edildi (rows boş) → gerçek satır sayısını K2'den al.
            fi.rows = k2.records.shapes.len() as u32;
        }
    }

    let k3 = {
        let _t = Timer::start("K3-entity-map");
        let mut k3 = build_entity_map(&k2.records);
        // XFL_025: geojson feature id'leri K1'de toplandı; EntityMap'e taşı.
        k3.entity_map.geojson_location_ids = k1.geojson_location_ids.clone();
        k3
    };

    let k4 = {
        let _t = Timer::start("K4-cross-ref");
        check_cross_ref(&k2.records, &k3.entity_map, today)
    };

    // #15: trip_stop_set (büyük feed'de ~226 MB) yalnızca K4'te kullanılır; K5/K6/K7 ve
    // build_name_index kullanmaz → K4 biter bitmez serbest bırak (K6 öncesi canlı belleği düşürür).
    k2.records.stop_times_index.trip_stop_set = Default::default();

    let k5 = {
        let _t = Timer::start("K5-derived");
        build_derived(&k2.records, &k3.entity_map)
    };

    let k6 = {
        let _t = Timer::start("K6-analytics");
        analyze_k6(&k2.records, &k5.derived, config, today)
    };

    let mut all = Vec::new();
    all.extend(k1.notices);
    all.extend(k2.notices);
    all.extend(k3.notices);
    all.extend(k4.notices);
    all.extend(k5.notices);
    all.extend(k6.notices);

    let k7 = {
        let _t = Timer::start("K7-reporting");
        report_k7(all, &k2.records, &k5.derived, file_stats)
    };

    ValidateResult::Ok(ValidationResult {
        notices: k7.notices,
        reports: k7.reports,
        metrics: k7.metrics,
        name_index: build_name_index(&k2.records),
        capped_totals: std::collections::HashMap::new(),
    })
}

pub fn build_name_index(records: &EntityRecords) -> gtfs_core::NameIndex {
    use std::collections::HashMap;

    // ── BÜYÜK FEED BELLEK MODU (#15 large_feed_memory_mode) ──────────────────────
    // Çok büyük feed'de name_index'in ağır alanları sonucu JS'e serialize ederken (to_js)
    // belleği patlatıyor (yüz MB+ JSON, 4 GB tavanını aşıp OOM). DETERMİNİSTİK eşik (entity
    // sayısı — runtime bellek DEĞİL; reprodüksiyon için). Eşik üstünde:
    //  - HARİTA verisi (shape_coords, trip_stops, trip_shapes, shape_*) atlanır (o ölçekte
    //    harita zaten çizilemez),
    //  - PER-TRIP ETİKET map'leri (trips/trip_routes/trip_directions/trip_first_dep) atlanır
    //    → UI/rapor notice'larında etiket yerine ham ID gösterir (sessiz fallback, kırık değil).
    // KORUNAN (küçük + yüksek değer): stops (durak adı), routes (hat adı), stop_coords (durak pini).
    // Küçük/normal feed'de DAVRANIŞ DEĞİŞMEZ (eşik altı → tüm map'ler dolu).
    const SHAPE_PT_CAP: usize = 800_000;
    const LARGE_FEED_TRIP_CAP: usize = 60_000;
    let large_feed_mode = records.trips.len() > LARGE_FEED_TRIP_CAP;
    let skip_shape_coords = records.shapes.len() > SHAPE_PT_CAP || large_feed_mode;

    let stops: HashMap<String, String> = records.stops.iter()
        .filter_map(|r| r.stop_name.as_ref().map(|n| (r.stop_id.clone(), n.clone())))
        .collect();

    let routes: HashMap<String, String> = records.routes.iter()
        .map(|r| {
            let name = r.route_short_name.as_deref()
                .or(r.route_long_name.as_deref())
                .unwrap_or("")
                .to_string();
            (r.route_id.clone(), name)
        })
        .filter(|(_, n)| !n.is_empty())
        .collect();

    // Per-trip etiket map'leri (#15): büyük feed modunda atlanır → notice'lar ham trip_id gösterir.
    let trips: HashMap<String, String> = if large_feed_mode { HashMap::new() } else {
        records.trips.iter()
            .filter_map(|r| r.trip_headsign.as_ref().map(|h| (r.trip_id.to_string(), h.to_string())))
            .collect()
    };

    let trip_routes: HashMap<String, String> = if large_feed_mode { HashMap::new() } else {
        records.trips.iter()
            .map(|r| (r.trip_id.to_string(), r.route_id.to_string()))
            .collect()
    };

    // trip_id → direction_id ("0"/"1"); yön bilgisi olmayan sefer dahil edilmez.
    let trip_directions: HashMap<String, String> = if large_feed_mode { HashMap::new() } else {
        records.trips.iter()
            .filter_map(|r| r.direction_id.map(|d| (r.trip_id.to_string(), d.to_string())))
            .collect()
    };

    let stop_coords: HashMap<String, [f64; 2]> = records.stops.iter()
        .filter_map(|r| {
            if let (Some(lat), Some(lon)) = (r.stop_lat, r.stop_lon) {
                Some((r.stop_id.clone(), [lat, lon]))
            } else {
                None
            }
        })
        .collect();

    // trip_id → ilk kalkış saati "HH:MM" (büyük feed modunda atlanır)
    let trip_first_dep: HashMap<String, String> = if large_feed_mode { HashMap::new() } else {
        records.stop_times_index.iter_trips()
            .filter_map(|(trip_id, stops)| {
                stops.first()
                    .and_then(|s| s.departure_time())
                    .map(|(h, m, _)| (trip_id.to_string(), format!("{:02}:{:02}", h % 24, m)))
            })
            .collect()
    };

    // shape_id → benzersiz [[route_id, yön]] listesi (harita; büyük feed modunda atlanır)
    let shape_routes: HashMap<String, Vec<[String; 2]>> = if large_feed_mode { HashMap::new() } else {
        let mut seen: std::collections::HashSet<(String, String, String)> = std::collections::HashSet::new();
        let mut map: HashMap<String, Vec<[String; 2]>> = HashMap::new();
        for trip in &records.trips {
            let Some(shape_id) = &trip.shape_id else { continue };
            let dir = match trip.direction_id {
                Some(0) => "Gidiş",
                Some(1) => "Dönüş",
                _       => "",
            };
            let key = (shape_id.to_string(), trip.route_id.to_string(), dir.to_string());
            if seen.insert(key) {
                map.entry(shape_id.to_string())
                    .or_default()
                    .push([trip.route_id.to_string(), dir.to_string()]);
            }
        }
        map
    };

    // shape_id → sıralı [[lat, lon]] nokta listesi (harita çizimi için).
    // #15: çok büyük feed'de atlanır (serialize OOM önlemi).
    let shape_coords: HashMap<String, Vec<[f64; 2]>> = if skip_shape_coords { HashMap::new() } else {
        let mut pts: Vec<(&str, u32, f64, f64)> = records.shapes.iter()
            .filter_map(|s| {
                let lat = s.shape_pt_lat?;
                let lon = s.shape_pt_lon?;
                Some((s.shape_id.as_str(), s.shape_pt_sequence.unwrap_or(0), lat, lon))
            })
            .collect();
        pts.sort_unstable_by_key(|&(id, seq, _, _)| (id, seq));
        let mut map: HashMap<String, Vec<[f64; 2]>> = HashMap::new();
        for (id, _, lat, lon) in pts {
            map.entry(id.to_string()).or_default().push([lat, lon]);
        }
        map
    };

    // trip_id → shape_id (harita; büyük feed modunda atlanır)
    let trip_shapes: HashMap<String, String> = if large_feed_mode { HashMap::new() } else {
        records.trips.iter()
            .filter_map(|t| t.shape_id.as_ref().map(|s| (t.trip_id.to_string(), s.to_string())))
            .collect()
    };

    // trip_id → [stop_id, ...] stop_sequence sıralı (harita; büyük feed modunda atlanır)
    let trip_stops: HashMap<String, Vec<String>> = if large_feed_mode { HashMap::new() } else {
        records.stop_times_index.iter_trips()
            .map(|(trip_id, stops)| {
                let stm_idx = &records.stop_times_index;
                let mut ids: Vec<String> = stops.iter()
                    .filter(|s| s.stop_idx != u32::MAX)
                    .map(|s| stm_idx.stop_id_of(s).to_string())
                    .collect();
                ids.dedup();
                (trip_id.to_string(), ids)
            })
            .collect()
    };

    // shape_id → ilk trip_id (harita; büyük feed modunda atlanır)
    let shape_trips: HashMap<String, String> = if large_feed_mode { HashMap::new() } else {
        let mut map: HashMap<String, String> = HashMap::new();
        for t in &records.trips {
            if let Some(ref shape_id) = t.shape_id {
                map.entry(shape_id.to_string()).or_insert_with(|| t.trip_id.to_string());
            }
        }
        map
    };

    // route_id → [distinct shape_ids] (terminus haritası; büyük feed modunda atlanır)
    let route_shapes: HashMap<String, Vec<String>> = if large_feed_mode { HashMap::new() } else {
        let mut map: HashMap<String, Vec<String>> = HashMap::new();
        for t in &records.trips {
            if let Some(ref shape_id) = t.shape_id {
                let v = map.entry(t.route_id.to_string()).or_default();
                let shape_str = shape_id.as_str();
                if !v.iter().any(|s: &String| s == shape_str) {
                    v.push(shape_id.to_string());
                }
            }
        }
        map
    };

    gtfs_core::NameIndex { stops, routes, trips, trip_routes, trip_directions, stop_coords, trip_first_dep, shape_routes, shape_coords, trip_shapes, trip_stops, shape_trips, route_shapes }
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
