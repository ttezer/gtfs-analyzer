use std::collections::{HashMap, HashSet};
use rustc_hash::{FxHashMap, FxHashSet};
use smol_str::SmolStr;

use gtfs_config::ValidatorConfig;
use gtfs_core::{EntityType, Notice};
use gtfs_rules::get_rule;

use crate::k2::stop_times::{build_flex, CompactStopTime};
use crate::k2::EntityRecords;
use crate::k5_derived::DerivedData;

// ── Çıktı tipi ────────────────────────────────────────────────────────────────

#[derive(Debug, Default)]
pub struct K6Result {
    pub notices: Vec<Notice>,
}

// ── Ana fonksiyon ─────────────────────────────────────────────────────────────

/// today_yyyymmdd: çalıştırma anındaki tarih (ör. 20260514). CAL/CLD expiry kuralları için.
pub fn analyze(
    records: &EntityRecords,
    derived: &DerivedData,
    config: &ValidatorConfig,
    today_yyyymmdd: u32,
) -> K6Result {
    use crate::timing::Timer;

    let trip_shape: HashMap<&str, &str> = records
        .trips
        .iter()
        .filter_map(|t| t.shape_id.as_deref().map(|s| (t.trip_id.as_str(), s)))
        .collect();
    // OOM fix Plan D: K6 artık K2 index'ini KLONLAMAZ — ödünç alır. Fallback (yalnızca test:
    // stop_times_index boş ama records.stop_times dolu) için owned by_trip burada kurulur,
    // build()'e ref geçilir; idx ondan ödünç alır (yaşam süresi bu frame'de, idx'ten önce tanımlı).
    let fallback_by_trip = if records.stop_times_index.total_rows == 0 && !records.stop_times.is_empty() {
        build_fallback_by_trip(records)
    } else {
        FxHashMap::default()
    };
    let idx = { let _t = Timer::start("K6::idx::build"); StopTimesIndex::build(records, &trip_shape, &fallback_by_trip) };

    // Her bağımsız K6 check'i KENDİ (notices, ctr)'sini üretir; sonuçlar KANONİK sırada
    // (1→13) birleştirilir ve sondaki renumber id'leri tek-iş-parçacıklı global ctr ile
    // BİREBİR eşler. Paralellik check'ler ARASINDA (her check kendi dedup set'ine sahip,
    // sadece `records/derived/config/idx`'i OKUR) → say/içerik/sıra emisyon-sırasından
    // bağımsız. `parallel` feature açıkken rayon::scope ile paralel, kapalıyken seri — AYNI çıktı.
    type K6Task<'a> = Box<dyn Fn() -> Vec<Notice> + Send + Sync + 'a>;
    let tasks: Vec<K6Task> = vec![
        Box::new(|| { let _t = Timer::start("K6::speed_and_duration");    let mut v = Vec::new(); let mut c = 0u32; check_speed_and_duration(records, config, &idx, &mut v, &mut c); v }),
        Box::new(|| { let _t = Timer::start("K6::frequency_headway");     let mut v = Vec::new(); let mut c = 0u32; check_frequency_headway(records, config, &mut v, &mut c); v }),
        Box::new(|| { let _t = Timer::start("K6::route_headway");         let mut v = Vec::new(); let mut c = 0u32; check_route_headway(records, config, &idx, &mut v, &mut c); v }),
        Box::new(|| { let _t = Timer::start("K6::calendar_analytics");    let mut v = Vec::new(); let mut c = 0u32; check_calendar_analytics(records, derived, config, today_yyyymmdd, &mut v, &mut c); v }),
        Box::new(|| { let _t = Timer::start("K6::geo_analytics");         let mut v = Vec::new(); let mut c = 0u32; check_geo_analytics(records, derived, config, &mut v, &mut c); v }),
        Box::new(|| { let _t = Timer::start("K6::operational_analytics"); let mut v = Vec::new(); let mut c = 0u32; check_operational_analytics(records, derived, config, &idx, today_yyyymmdd, &mut v, &mut c); v }),
        Box::new(|| { let _t = Timer::start("K6::stoptimes_derived");     let mut v = Vec::new(); let mut c = 0u32; check_stoptimes_derived(&idx, &mut v, &mut c); v }),
        Box::new(|| { let _t = Timer::start("K6::route_trip_quality");    let mut v = Vec::new(); let mut c = 0u32; check_route_trip_quality(records, derived, &idx, &mut v, &mut c); v }),
        Box::new(|| { let _t = Timer::start("K6::data_quality");          let mut v = Vec::new(); let mut c = 0u32; check_data_quality(records, derived, today_yyyymmdd, &mut v, &mut c); v }),
        Box::new(|| { let _t = Timer::start("K6::remaining_analytics");   let mut v = Vec::new(); let mut c = 0u32; check_remaining_analytics(records, derived, config, &idx, &mut v, &mut c); v }),
        Box::new(|| { let _t = Timer::start("K6::shp012");                let mut v = Vec::new(); let mut c = 0u32; check_shp012(records, &idx, &mut v, &mut c); v }),
        Box::new(|| { let _t = Timer::start("K6::shp022");                let mut v = Vec::new(); let mut c = 0u32; check_shp022(records, &idx, &mut v, &mut c); v }),
        Box::new(|| { let _t = Timer::start("K6::pathway_analytics");     let mut v = Vec::new(); let mut c = 0u32; check_pathway_analytics(records, derived, &mut v, &mut c); v }),
        Box::new(|| { let _t = Timer::start("K6::calendar_override");     let mut v = Vec::new(); let mut c = 0u32; check_calendar_override_analytics(records, derived, config, &mut v, &mut c); v }),
        Box::new(|| { let _t = Timer::start("K6::vat_analytics");         let mut v = Vec::new(); let mut c = 0u32; check_vat_analytics(records, &idx, &mut v, &mut c); v }),
    ];

    #[cfg(feature = "parallel")]
    let parts: Vec<Vec<Notice>> = {
        let mut out: Vec<Vec<Notice>> = tasks.iter().map(|_| Vec::new()).collect();
        rayon::scope(|s| {
            for (slot, task) in out.iter_mut().zip(tasks.iter()) {
                s.spawn(move |_| { *slot = task(); });
            }
        });
        out
    };
    #[cfg(not(feature = "parallel"))]
    let parts: Vec<Vec<Notice>> = tasks.iter().map(|t| t()).collect();

    let mut notices: Vec<Notice> = Vec::with_capacity(parts.iter().map(|p| p.len()).sum());
    for p in parts {
        notices.extend(p);
    }
    // Renumber → tek-iş-parçacıklı global ctr ile BİREBİR id'ler (k6/{rule}#{N}).
    // Birleştirme sırası sabit (kanonik) olduğundan paralel/seri ayırt edilemez.
    for (i, n) in notices.iter_mut().enumerate() {
        n.id = format!("k6/{}#{}", n.rule_id, i + 1);
    }

    K6Result { notices }
}

// ── Notice yardımcısı ─────────────────────────────────────────────────────────

fn k6_notice(
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
    let meta = get_rule(rule_id).unwrap_or_else(|| panic!("K6: bilinmeyen rule_id {rule_id}"));
    Notice {
        id: format!("k6/{rule_id}#{ctr}"),
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
    }
}

// ── Yardımcı fonksiyonlar ─────────────────────────────────────────────────────

/// stop_id'nin sayısal kök kısmını döndürür: sondaki harf ekini ayıklar.
/// Rakam içermiyorsa orijinal id döner (kıyas güvenli değil).
/// Örnek: "2119d" → "2119", "t17d" → "t17", "abc" → "abc"
#[allow(dead_code)]
/// `needle`, `haystack` içinde kelime sınırında (alfanümerik olmayan karakter veya
/// dize başı/sonu ile çevrili) geçiyor mu? Büyük/küçük harf duyarsız.
fn contains_as_word(haystack: &str, needle: &str) -> bool {
    let h = haystack.to_lowercase();
    let n = needle.to_lowercase();
    let nb = n.as_bytes().len();
    let mut start = 0usize;
    while let Some(pos) = h[start..].find(n.as_str()) {
        let pos = start + pos;
        let before_ok = pos == 0
            || !h[..pos].chars().next_back().map(|c| c.is_alphanumeric()).unwrap_or(false);
        let after_ok = pos + nb >= h.len()
            || !h[pos + nb..].chars().next().map(|c| c.is_alphanumeric()).unwrap_or(false);
        if before_ok && after_ok { return true; }
        start = pos + 1;
    }
    false
}

/// Metin yalnızca büyük harf alfabetik karakter içeriyor mu? (en az 2 harf, hepsi büyük)
/// Alloc'suz: ara Vec yok, ilk küçük harfte kısa devre. Eşik + predicate AYNI →
/// her girdi için orijinalle birebir aynı sonuç.
fn is_all_caps(s: &str) -> bool {
    let mut n = 0usize;
    for c in s.chars().filter(|c| c.is_alphabetic()) {
        if !c.is_uppercase() { return false; }
        n += 1;
    }
    n >= 2
}

// Tüm harfler küçük harf — mixed_case_recommended_field (all-lowercase varyantı)
fn is_all_lower(s: &str) -> bool {
    let mut n = 0usize;
    for c in s.chars().filter(|c| c.is_alphabetic()) {
        if !c.is_lowercase() { return false; }
        n += 1;
    }
    n >= 3
}

fn haversine_km(lat1: f64, lon1: f64, lat2: f64, lon2: f64) -> f64 {
    const R: f64 = 6371.0;
    let dlat = (lat2 - lat1).to_radians();
    let dlon = (lon2 - lon1).to_radians();
    let a = (dlat / 2.0).sin().powi(2)
        + lat1.to_radians().cos() * lat2.to_radians().cos() * (dlon / 2.0).sin().powi(2);
    2.0 * R * a.sqrt().asin()
}

/// Bir durağın (slat, slon) bir shape polyline'ına olan minimum mesafesini metre cinsinden döner.
/// Her segment için dik-mesafe (perpendicular projection) hesaplanır; projeksiyon segment
/// dışına taşarsa en yakın uç noktaya mesafe kullanılır.
fn point_to_polyline_dist_m(slat: f64, slon: f64, pts: &[(f64, f64)]) -> f64 {
    if pts.is_empty() { return f64::MAX; }
    if pts.len() == 1 {
        return haversine_km(slat, slon, pts[0].0, pts[0].1) * 1000.0;
    }
    const DEG_TO_M: f64 = 111_320.0;
    let cos_lat = slat.to_radians().cos();
    // Durağı yerel Kartezyen koordinat merkezi (0,0) olarak al
    let to_local = |lat: f64, lon: f64| -> (f64, f64) {
        ((lon - slon) * cos_lat * DEG_TO_M, (lat - slat) * DEG_TO_M)
    };
    let mut min_dist = f64::MAX;
    for w in pts.windows(2) {
        let (ax, ay) = to_local(w[0].0, w[0].1);
        let (bx, by) = to_local(w[1].0, w[1].1);
        let (dx, dy) = (bx - ax, by - ay);
        let len_sq = dx * dx + dy * dy;
        let dist = if len_sq < 1e-6 {
            (ax * ax + ay * ay).sqrt()
        } else {
            let t = ((-ax * dx) + (-ay * dy)) / len_sq;
            let t = t.clamp(0.0, 1.0);
            let (cx, cy) = (ax + t * dx, ay + t * dy);
            (cx * cx + cy * cy).sqrt()
        };
        if dist < min_dist { min_dist = dist; }
    }
    min_dist
}

fn hms_to_secs(t: (u32, u32, u32)) -> u32 {
    t.0 * 3600 + t.1 * 60 + t.2
}

/// Durak (slat, slon) noktasının polyline üzerindeki projeksiyon arc-length'ini döndürür (km).
/// `pts` sıralı shape noktaları; `cum` her noktanın baştan birikimli mesafesi (cum[0]=0).
fn project_arc_km(pts: &[(f64, f64)], cum: &[f64], slat: f64, slon: f64) -> f64 {
    let cos_lat = (slat.to_radians()).cos().max(0.001_f64);
    let scale   = 111.0_f64 * cos_lat;
    let mut best_arc    = 0.0_f64;
    let mut best_dsq    = f64::INFINITY;
    for w in 0..pts.len() - 1 {
        let (alat, alon) = pts[w];
        let (blat, blon) = pts[w + 1];
        let ax = (alon - slon) * scale;
        let ay = (alat - slat) * 111.0_f64;
        let bx = (blon - slon) * scale;
        let by_ = (blat - slat) * 111.0_f64;
        let dx = bx - ax;
        let dy = by_ - ay;
        let len_sq = dx * dx + dy * dy;
        let t = if len_sq < 1e-12 { 0.0_f64 } else {
            ((-ax * dx) + (-ay * dy)) / len_sq
        }.clamp(0.0_f64, 1.0_f64);
        let nx = ax + t * dx;
        let ny = ay + t * dy;
        let dsq = nx * nx + ny * ny;
        if dsq < best_dsq {
            best_dsq  = dsq;
            best_arc  = cum[w] + t * (cum[w + 1] - cum[w]);
        }
    }
    best_arc
}

/// Durak noktasından shape polyline'ına segment projeksiyonu ile minimum uzaklık (km).
/// Equirectangular space'te argmin bulunur; son haversine tek kez çağrılır.
/// `safe_sq`: threshold²×0.98 — bu değerin altındaki segment için 0.0 döner (erken çıkış).
fn seg_min_dist_km(
    pts: &[(f64, f64)],
    slat: f64,
    slon: f64,
    scale_lon: f64,
    safe_sq: f64,
) -> f64 {
    if pts.len() < 2 {
        return haversine_km(slat, slon, pts[0].0, pts[0].1);
    }
    let mut min_sq = f64::INFINITY;
    let mut best_lat = pts[0].0;
    let mut best_lon = pts[0].1;
    for w in pts.windows(2) {
        let (alat, alon) = w[0];
        let (blat, blon) = w[1];
        // Segment A→B, stop P=(0,0) in equirectangular km coords
        let ax = (alon - slon) * scale_lon;
        let ay = (alat - slat) * 111.0_f64;
        let bx = (blon - slon) * scale_lon;
        let by_ = (blat - slat) * 111.0_f64;
        let dx = bx - ax;
        let dy = by_ - ay;
        let len_sq = dx * dx + dy * dy;
        let (nx, ny, nlat, nlon) = if len_sq < 1e-12 {
            // Degenerate (zero-length) segment — use endpoint A
            (ax, ay, alat, alon)
        } else {
            // Scalar projection of P=(0,0) onto A+t*(B-A), clamp t∈[0,1]
            let t = ((-ax * dx) + (-ay * dy)) / len_sq;
            let t = t.clamp(0.0_f64, 1.0_f64);
            (ax + t * dx, ay + t * dy,
             alat + t * (blat - alat), alon + t * (blon - alon))
        };
        let d_sq = nx * nx + ny * ny;
        if d_sq < min_sq {
            min_sq = d_sq;
            best_lat = nlat;
            best_lon = nlon;
        }
        if min_sq <= safe_sq {
            return 0.0;
        }
    }
    if min_sq <= safe_sq { 0.0 } else { haversine_km(slat, slon, best_lat, best_lon) }
}

/// Intercity/uzun mesafe ray route_type'ı mı? (STM_017 shape_dist eksik, STM_026 eşiği için)
/// GTFS: 2=Rail, 12=Monorail, 100-117 genişletilmiş tren tipleri.
fn is_rail_route_type(route_type: u32) -> bool {
    route_type == 2 || route_type == 12 || (100..=117).contains(&route_type)
}

/// Sabit güzergahlı taşıt mı? (shape projeksiyon bypass için)
/// Tram/LRT/Metro/Rail hepsi dahil — haversine yeterince iyi, projeksiyon false positive üretir.
/// GTFS: 0=Tram, 1=Metro, 2=Rail, 12=Monorail, 100-117 genişletilmiş tren tipleri.
fn is_fixed_guideway(route_type: u32) -> bool {
    matches!(route_type, 0 | 1 | 2 | 12) || (100..=117).contains(&route_type)
}

/// route_type → config'ten hız eşiği (km/h)
fn max_speed_kmh(route_type: u32, cfg: &ValidatorConfig) -> f64 {
    match route_type {
        0 => cfg.max_speed_tram_kmh,
        1 => cfg.max_speed_metro_kmh,
        2 => cfg.max_speed_rail_kmh,
        3 => cfg.max_speed_bus_kmh,
        4 => cfg.max_speed_ferry_kmh,
        5 | 6 | 7 => cfg.max_speed_cablecar_kmh,
        11 => cfg.max_speed_bus_kmh,
        12 => cfg.max_speed_rail_kmh,
        _ => cfg.max_speed_bus_kmh, // bilinmeyen tür → güvenli varsayılan
    }
}

// ── Shared stop_times index (K6 tek geçiş) ──────────────────────────────────
// Not: by_trip artık K2 StopTimesIndex'inden &CompactStopTime referansları taşıyor.
// records.stop_times Vec<StopTimeRecord> taranmaz; sadece index kullanılır.

struct StopTimesIndex<'a> {
    by_trip: FxHashMap<&'a str, &'a [CompactStopTime]>,
    trip_first_dep: FxHashMap<&'a str, u32>,
    trip_has_time: FxHashSet<&'a str>,
    stop_shapes: FxHashMap<&'a str, Vec<&'a str>>,
    trips_missing_sdt: FxHashMap<&'a str, u64>,
    // STM_036: K2'de tespit edilen sırasız stop_sequence bilgisi
    unsorted_seq_trips: &'a Vec<(SmolStr, u32, u32, u64)>,
}

impl<'a> StopTimesIndex<'a> {
    fn build(
        records: &'a EntityRecords,
        trip_shape: &HashMap<&'a str, &'a str>,
        fallback: &'a FxHashMap<&'a str, Vec<CompactStopTime>>,
    ) -> Self {
        let n_trips = records.trips.len();
        let n_stops = records.stops.len();
        let k2_idx = &records.stop_times_index;

        // OOM fix Plan D: by_trip K2 index dilimlerini ÖDÜNÇ alır (KLON YOK).
        let mut by_trip: FxHashMap<&'a str, &'a [CompactStopTime]> = FxHashMap::default();
        let mut stop_shapes: FxHashMap<&'a str, Vec<&'a str>> = FxHashMap::default();
        by_trip.reserve(n_trips);
        stop_shapes.reserve(n_stops);

        if k2_idx.total_rows > 0 || records.stop_times.is_empty() {
            // Üretim yolu: K2 index dilimlerini ödünç al — klon yok
            for (trip_id, stops) in k2_idx.iter_trips() {
                let tid: &'a str = trip_id.as_str(); // 'a lifetime: records'tan gelir
                by_trip.insert(tid, stops);
                if let Some(&shape_id) = trip_shape.get(tid) {
                    for st in stops {
                        if !st.stop_id.is_empty() {
                            stop_shapes.entry(st.stop_id.as_str()).or_default().push(shape_id);
                        }
                    }
                }
            }
        } else {
            // Test yolu: çağıranın kurduğu owned fallback map'ini ödünç al (bkz. build_fallback_by_trip)
            for (&tid, stops) in fallback {
                by_trip.insert(tid, stops.as_slice());
                if let Some(&shape_id) = trip_shape.get(tid) {
                    for st in stops {
                        if !st.stop_id.is_empty() {
                            stop_shapes.entry(st.stop_id.as_str()).or_default().push(shape_id);
                        }
                    }
                }
            }
        }

        for v in stop_shapes.values_mut() {
            v.sort_unstable();
            v.dedup();
        }

        // Post-build: trip_has_time + trips_missing_sdt
        let mut trip_has_time: FxHashSet<&'a str> = FxHashSet::default();
        let mut trips_missing_sdt: FxHashMap<&'a str, u64> = FxHashMap::default();
        trip_has_time.reserve(n_trips);
        trips_missing_sdt.reserve(n_trips / 4);
        for (&tid, v) in &by_trip {
            if v.iter().any(|s| s.arrival_time.is_some() || s.departure_time.is_some()) {
                trip_has_time.insert(tid);
            }
            if trip_shape.contains_key(tid) {
                if let Some(st) = v.iter().find(|s| s.shape_dist_traveled.is_none()) {
                    trips_missing_sdt.insert(tid, st.line);
                }
            }
        }

        let trip_first_dep: FxHashMap<&'a str, u32> = by_trip
            .iter()
            .filter_map(|(&tid, v)| {
                v.first()
                    .and_then(|s| s.departure_time)
                    .map(|dep| (tid, hms_to_secs(dep)))
            })
            .collect();

        static EMPTY_UNSORTED: std::sync::OnceLock<Vec<(SmolStr, u32, u32, u64)>> = std::sync::OnceLock::new();
        let unsorted_ref: &'a Vec<(SmolStr, u32, u32, u64)> = if k2_idx.total_rows > 0 || records.stop_times.is_empty() {
            &k2_idx.unsorted_seq_trips
        } else {
            EMPTY_UNSORTED.get_or_init(Vec::new)
        };

        Self {
            by_trip,
            trip_first_dep,
            trip_has_time,
            stop_shapes,
            trips_missing_sdt,
            unsorted_seq_trips: unsorted_ref,
        }
    }
}

/// OOM fix Plan D: YALNIZCA test yolu (stop_times_index boş ama records.stop_times dolu) için
/// records.stop_times'tan owned by_trip kurar. Üretimde çağrılmaz (K2 index ödünç alınır).
/// Map çağıran (analyze_k6) frame'inde yaşar; StopTimesIndex::build ondan &Vec ödünç alır.
fn build_fallback_by_trip<'a>(records: &'a EntityRecords) -> FxHashMap<&'a str, Vec<CompactStopTime>> {
    let mut by_trip: FxHashMap<&'a str, Vec<CompactStopTime>> = FxHashMap::default();
    for st in &records.stop_times {
        if st.trip_id.is_empty() { continue; }
        let tid: &'a str = st.trip_id.as_str();
        by_trip.entry(tid).or_default().push(CompactStopTime {
            stop_id: st.stop_id.clone(),
            stop_sequence: st.stop_sequence,
            arrival_time: st.arrival_time,
            departure_time: st.departure_time,
            stop_headsign: st.stop_headsign.clone(),
            pickup_type: st.pickup_type,
            drop_off_type: st.drop_off_type,
            shape_dist_traveled: st.shape_dist_traveled,
            timepoint: st.timepoint,
            continuous_pickup: st.continuous_pickup,
            continuous_drop_off: st.continuous_drop_off,
            line: st.line,
            flex: build_flex(
                st.start_pickup_drop_off_window,
                st.end_pickup_drop_off_window,
                st.location_id.clone(),
                st.location_group_id.clone(),
                st.pickup_booking_rule_id.clone(),
                st.drop_off_booking_rule_id.clone(),
            ),
        });
    }
    for v in by_trip.values_mut() {
        v.sort_by_key(|s| s.stop_sequence.unwrap_or(u32::MAX));
    }
    by_trip
}

// ── WP-09a: Hız anomalisi + trip süresi ──────────────────────────────────────

fn check_speed_and_duration(
    records: &EntityRecords,
    config: &ValidatorConfig,
    idx: &StopTimesIndex<'_>,
    notices: &mut Vec<Notice>,
    ctr: &mut u32,
) {
    use crate::timing::Timer;

    let (stop_coords, trip_route_type, trip_to_route, route_short_sd, trip_direction, trip_service) = {
        let _t = Timer::start("K6::sd::setup");
        // stop_id → (lat, lon)  FxHashMap: SipHash yerine multiply-xor
        let mut stop_coords: FxHashMap<&str, (f64, f64)> = FxHashMap::default();
        stop_coords.reserve(records.stops.len());
        for s in &records.stops {
            if let Some(coords) = s.stop_lat.zip(s.stop_lon) {
                stop_coords.insert(s.stop_id.as_str(), coords);
            }
        }

        // route_id → route_type
        let route_type_map: HashMap<&str, u32> = records
            .routes
            .iter()
            .filter_map(|r| r.route_type.map(|rt| (r.route_id.as_str(), rt)))
            .collect();

        // trip_id → route_type
        let mut trip_route_type: FxHashMap<&str, u32> = FxHashMap::default();
        trip_route_type.reserve(records.trips.len());
        for t in &records.trips {
            if let Some(&rt) = route_type_map.get(t.route_id.as_str()) {
                trip_route_type.insert(t.trip_id.as_str(), rt);
            }
        }

        // trip_id → route_id
        let mut trip_to_route: FxHashMap<&str, &str> = FxHashMap::default();
        trip_to_route.reserve(records.trips.len());
        for t in &records.trips {
            trip_to_route.insert(t.trip_id.as_str(), t.route_id.as_str());
        }

        // route_id → gösterim adı
        let route_short_sd: FxHashMap<&str, &str> = records.routes.iter()
            .map(|r| {
                let label = r.route_short_name.as_deref().filter(|s| !s.is_empty()).unwrap_or(r.route_id.as_str());
                (r.route_id.as_str(), label)
            })
            .collect();

        // trip_id → direction_id (gösterim için)
        let mut trip_direction: FxHashMap<&str, &'static str> = FxHashMap::default();
        trip_direction.reserve(records.trips.len());
        for t in &records.trips {
            let dir = match t.direction_id { Some(0) => "0", Some(1) => "1", _ => "-" };
            trip_direction.insert(t.trip_id.as_str(), dir);
        }

        // trip_id → service_id
        let mut trip_service: FxHashMap<&str, &str> = FxHashMap::default();
        trip_service.reserve(records.trips.len());
        for t in &records.trips {
            trip_service.insert(t.trip_id.as_str(), t.service_id.as_str());
        }

        (stop_coords, trip_route_type, trip_to_route, route_short_sd, trip_direction, trip_service)
    };

    // Shape-based segment distance altyapısı (hız hesabı için)
    // Öncelik: shape polyline projeksiyonu > haversine fallback
    let (shape_pts_speed, shape_cum_speed, trip_shape_speed) = {
        let _t = Timer::start("K6::sd::shape_setup");
        let mut unsorted: FxHashMap<&str, Vec<(u32, f64, f64)>> = FxHashMap::default();
        for sp in &records.shapes {
            if let (Some(lat), Some(lon)) = (sp.shape_pt_lat, sp.shape_pt_lon) {
                unsorted.entry(sp.shape_id.as_str()).or_default()
                    .push((sp.shape_pt_sequence.unwrap_or(0), lat, lon));
            }
        }
        let shape_pts: FxHashMap<&str, Vec<(f64, f64)>> = unsorted.into_iter()
            .map(|(sid, mut v)| {
                v.sort_unstable_by_key(|&(seq, _, _)| seq);
                (sid, v.into_iter().map(|(_, la, lo)| (la, lo)).collect())
            })
            .collect();
        let shape_cum: FxHashMap<&str, Vec<f64>> = shape_pts.iter()
            .map(|(&sid, pts)| {
                let mut c = Vec::with_capacity(pts.len());
                c.push(0.0_f64);
                for i in 1..pts.len() {
                    c.push(c[i-1] + haversine_km(pts[i-1].0, pts[i-1].1, pts[i].0, pts[i].1));
                }
                (sid, c)
            })
            .collect();
        let trip_shape: FxHashMap<&str, &str> = records.trips.iter()
            .filter_map(|t| t.shape_id.as_deref()
                .filter(|s| !s.is_empty())
                .map(|s| (t.trip_id.as_str(), s)))
            .collect();
        (shape_pts, shape_cum, trip_shape)
    };

    // STM_036: stop_sequence sırasız trip'ler (K2'de tespit edildi, burada notice üretilir)
    for (trip_id, prev_seq, curr_seq, line_no) in idx.unsorted_seq_trips.iter() {
        notices.push(k6_notice(
            ctr, "STM_036", EntityType::Trip,
            Some(trip_id.to_string()), Some(trip_id.to_string()),
            "stop_times.txt", Some(*line_no), Some("stop_sequence"),
            Some(format!("{curr_seq}")), Some(format!("≥ {prev_seq}")),
            format!("'{trip_id}' seferinde stop_sequence {prev_seq}'den {curr_seq}'e düşüyor — değerler artmalı."),
            "stop_times.txt'i trip_id ve stop_sequence'a göre sıralayın.",
        ));
    }

    // Reusable per-trip coord buffer: 1 lookup/stop (windows(2) ile 2 lookup/pair olurdu)
    let mut coords_buf: Vec<Option<(f64, f64)>> = Vec::new();

    // Perf: (shape_id, stop_id) → shape üzerine arc-uzunluğu projeksiyonu MEMOIZE edilir.
    // project_arc_km tüm shape noktalarını gezer; aynı (shape,durak) seferler/segmentler
    // arası tekrar projekte ediliyordu. dist_km çıktısı BİREBİR aynı — sadece tekrar eleniyor.
    let mut arc_cache: FxHashMap<(&str, &str), f64> = FxHashMap::default();

    { let _t = Timer::start("K6::sd::loop");
    for (&trip_id, stimes) in &idx.by_trip {
        if stimes.len() < 2 {
            continue;
        }
        let route_type = trip_route_type.get(trip_id).copied().unwrap_or(3);
        let threshold = max_speed_kmh(route_type, config);
        let route = trip_to_route.get(trip_id).copied().unwrap_or(trip_id);
        let route_label = route_short_sd.get(route).copied().unwrap_or(route);
        let dir_sd = trip_direction.get(trip_id).copied().unwrap_or("-");
        let svc_sd = trip_service.get(trip_id).copied().unwrap_or("-");
        let dep_str = stimes.first()
            .and_then(|s| s.departure_time)
            .map(|(h, m, _)| format!("{h:02}:{m:02}"))
            .unwrap_or_default();

        // ── STM_028 / STM_029: trip süresi ───────────────────────────────────
        let first_dep = stimes.first().and_then(|s| s.departure_time).map(hms_to_secs);
        let last_arr = stimes.last().and_then(|s| s.arrival_time).map(hms_to_secs);

        if let (Some(dep), Some(arr)) = (first_dep, last_arr) {
            if arr >= dep {
                let duration_sec = arr - dep;
                let max_sec = (config.max_trip_duration_hours * 3600.0) as u32;

                if duration_sec > max_sec {
                    notices.push(k6_notice(
                        ctr,
                        "STM_028",
                        EntityType::Trip,
                        Some(trip_id.to_string()),
                        Some(trip_id.to_string()),
                        "stop_times.txt",
                        stimes.last().map(|s| s.line),
                        Some("arrival_time"),
                        Some(format_hms(duration_sec)),
                        Some(format_hms(max_sec)),
                        format!("'{}' hattının{} seferinin toplam süresi {} — eşik {}.",
                            route,
                            if dep_str.is_empty() { String::new() } else { format!(" {dep_str}") },
                            format_hms(duration_sec), format_hms(max_sec)),
                        "Seferi daha kısa parçalara bölün ya da stop_times verilerini doğrulayın.",
                    ));
                }

                if duration_sec < config.min_trip_duration_sec {
                    notices.push(k6_notice(
                        ctr,
                        "STM_029",
                        EntityType::Trip,
                        Some(trip_id.to_string()),
                        Some(trip_id.to_string()),
                        "stop_times.txt",
                        stimes.last().map(|s| s.line),
                        Some("departure_time"),
                        Some(format!("{duration_sec}s")),
                        Some(format!("≥ {}s", config.min_trip_duration_sec)),
                        format!("'{}' hattının{} seferinin toplam süresi {}sn — eşik {}sn.",
                            route,
                            if dep_str.is_empty() { String::new() } else { format!(" {dep_str}") },
                            duration_sec, config.min_trip_duration_sec),
                        "stop_times zaman değerlerini kontrol edin; sefer gerçekten bu kadar kısa olmamalı.",
                    ));
                }
            }
        }

        // ── STM_014 / OPR_008: hız anomalisi ─────────────────────────────────
        // Per-trip coord buffer: her stop bir kez sorgulanır
        coords_buf.clear();
        coords_buf.extend(stimes.iter().map(|s| stop_coords.get(s.stop_id.as_str()).copied()));

        let mut trip_max_speed: f64 = 0.0;
        let mut trip_max_speed_line: Option<u64> = None;
        let mut trip_bad_seg_count: u32 = 0;
        // Tüm bozuk segmentlerin durak ID çiftleri — UI haritasında her biri kırmızı çizilir
        let mut bad_seg_stops: Vec<(SmolStr, SmolStr)> = Vec::new();
        // STM_020: trip başına en büyük mesafeli sıfır-geçiş-süreli segment
        let mut worst_zero_seg: Option<(f64, u64, SmolStr, SmolStr, u32, u32)> = None;

        for i in 0..stimes.len() - 1 {
            let a = &stimes[i];
            let b = &stimes[i + 1];

            let dep_a = a.departure_time.map(hms_to_secs);
            let arr_b = b.arrival_time.map(hms_to_secs);

            let (Some(dep), Some(arr)) = (dep_a, arr_b) else { continue };
            // STM_020: sıfır geçiş süresi — eşik 200m (dakika yuvarlama gürültüsünü filtreler)
            if arr == dep {
                let dep_secs = a.departure_time.map(|(_, _, s)| s).unwrap_or(1);
                let arr_secs = b.arrival_time.map(|(_, _, s)| s).unwrap_or(1);
                let dist_km = match (coords_buf[i], coords_buf[i + 1]) {
                    (Some((la1, lo1)), Some((la2, lo2))) => haversine_km(la1, lo1, la2, lo2),
                    _ => 0.0,
                };
                // Her iki zaman tam dakika (saniye=0) ve duraklar 1 km'den yakın:
                // dakika yuvarlama gürültüsü olabilir — atla.
                // Duraklar 1 km'den uzaksa sıfır süre gerçek bir hata; STM_012 ateşle.
                if dep_secs == 0 && arr_secs == 0 {
                    if dist_km >= 1.0 {
                        let mut n012 = k6_notice(
                            ctr, "STM_012", EntityType::Trip,
                            Some(trip_id.to_string()), Some(trip_id.to_string()),
                            "stop_times.txt", Some(b.line), Some("arrival_time"),
                            Some(format!("sifir sure, {dist_km:.1} km")),
                            Some("<= 700 km/h".to_string()),
                            format!("trip_id '{trip_id}' stop_sequence {}-{} arasi gecis suresi sifir ama mesafe {dist_km:.1} km — fiziksel olarak imkansiz.",
                                a.stop_sequence.unwrap_or(0), b.stop_sequence.unwrap_or(0)),
                            "stop_times.txt zaman degerlerini dogrulayin; ayni dakikada cok uzak iki durak olamaz.",
                        );
                        let mut d = std::collections::HashMap::new();
                        d.insert("stop_a".to_string(), a.stop_id.to_string());
                        d.insert("stop_b".to_string(), b.stop_id.to_string());
                        n012.details = Some(d);
                        notices.push(n012);
                    }
                    continue;
                }
                if dist_km > 0.2 {
                    let is_worse = worst_zero_seg.as_ref().map_or(true, |&(d, ..)| dist_km > d);
                    if is_worse {
                        worst_zero_seg = Some((
                            dist_km, b.line,
                            a.stop_id.clone(), b.stop_id.clone(),
                            a.stop_sequence.unwrap_or(0),
                            b.stop_sequence.unwrap_or(0),
                        ));
                    }
                }
                continue;
            }
            if arr < dep {
                // STM_008: duraklar arası zaman geriye gidiyor
                notices.push(k6_notice(
                    ctr, "STM_008", EntityType::Trip,
                    Some(trip_id.to_string()), Some(trip_id.to_string()),
                    "stop_times.txt", Some(b.line), Some("arrival_time"),
                    Some(format!("{} → {}", format_hms(dep), format_hms(arr))),
                    Some(format!(">= {}", format_hms(dep))),
                    format!("trip_id '{trip_id}' stop_sequence {}-{} arası varış zamanı kalkış zamanından önce geliyor.",
                        a.stop_sequence.unwrap_or(0), b.stop_sequence.unwrap_or(0)),
                    "stop_times.txt zaman değerlerini gözden geçirin; seferler boyunca zamanlar monoton artmalıdır.",
                ));
                continue;
            }
            let dt_sec = arr - dep;

            // Mesafe: shape polyline projeksiyonu (varsa) ya da Haversine fallback.
            // Ray (metro/tren) seferleri için projeksiyon atlanır: düz/tünel hatlar
            // haversine ile yeterince temsil edilir; winding shapes false positive üretebilir.
            let (c1, c2) = match (coords_buf[i], coords_buf[i + 1]) {
                (Some(c1), Some(c2)) => (c1, c2),
                _ => continue,
            };
            let haver_km = haversine_km(c1.0, c1.1, c2.0, c2.1);
            let dist_km = if is_fixed_guideway(route_type) { haver_km } else {
            trip_shape_speed.get(trip_id)
                .and_then(|&sid| {
                    let pts = shape_pts_speed.get(sid)?;
                    let cum = shape_cum_speed.get(sid)?;
                    if pts.len() < 2 { return None; }
                    // (sid, stop_id) → arc cache: aynı shape'i kullanan seferlerde/segmentlerde
                    // tekrar projeksiyonu önler. stop_id → tek koordinat olduğundan sonuç deterministik.
                    let arc_a = *arc_cache.entry((sid, a.stop_id.as_str()))
                        .or_insert_with(|| project_arc_km(pts, cum, c1.0, c1.1));
                    let arc_b = *arc_cache.entry((sid, b.stop_id.as_str()))
                        .or_insert_with(|| project_arc_km(pts, cum, c2.0, c2.1));
                    let d = (arc_b - arc_a).abs();
                    if d > 1e-6 { Some(d) } else { None }
                })
                .unwrap_or(haver_km)
            };

            if dist_km < 1e-6 {
                if a.stop_id == b.stop_id {
                    // STM_035: aynı durak ardışık iki kez (terminal/döngü hattı)
                    notices.push(k6_notice(
                        ctr, "STM_035", EntityType::Trip,
                        Some(trip_id.to_string()), Some(trip_id.to_string()),
                        "stop_times.txt", Some(b.line), Some("stop_id"),
                        Some(a.stop_id.to_string()),
                        None,
                        format!("trip_id '{trip_id}' stop_sequence {}-{} arasında aynı durak ({}) ardışık iki kez ziyaret ediliyor.",
                            a.stop_sequence.unwrap_or(0), b.stop_sequence.unwrap_or(0), a.stop_id),
                        "Terminal veya döngü hattıysa beklenen bir durumdur. Değilse stop_times.txt'teki yinelenen satırı kaldırın.",
                    ));
                } else {
                    // STM_021: farklı stop_id'ler aynı koordinatta — gerçek veri hatası
                    notices.push(k6_notice(
                        ctr, "STM_021", EntityType::Trip,
                        Some(trip_id.to_string()), Some(trip_id.to_string()),
                        "stop_times.txt", Some(b.line), Some("stop_id"),
                        Some(format!("{} → {}", a.stop_id, b.stop_id)),
                        Some("> 0 m".to_string()),
                        format!("trip_id '{trip_id}' stop_sequence {}-{} arası mesafe 0: '{}' ve '{}' farklı duraklar ama aynı koordinatta.",
                            a.stop_sequence.unwrap_or(0), b.stop_sequence.unwrap_or(0), a.stop_id, b.stop_id),
                        "stops.txt'te durak koordinatlarını doğrulayın; ardışık farklı duraklar aynı konumda olmamalıdır.",
                    ));
                }
                continue;
            }

            // STM_025: segment seyahat süresi çok kısa
            if dt_sec < 10 {
                let mut n025 = k6_notice(
                    ctr, "STM_025", EntityType::Trip,
                    Some(trip_id.to_string()), Some(trip_id.to_string()),
                    "stop_times.txt", Some(b.line), Some("arrival_time"),
                    Some(format!("{dt_sec}s")), Some(">= 10s".to_string()),
                    format!("trip_id '{trip_id}' stop_sequence {}-{} arası seyahat süresi yalnızca {dt_sec}s.",
                        a.stop_sequence.unwrap_or(0), b.stop_sequence.unwrap_or(0)),
                    "stop_times.txt zaman değerlerini kontrol edin; segment süresi çok kısa.",
                );
                let mut d = std::collections::HashMap::new();
                d.insert("stop_a".to_string(), a.stop_id.to_string());
                d.insert("stop_b".to_string(), b.stop_id.to_string());
                n025.details = Some(d);
                notices.push(n025);
            }

            // STM_026: durak arası mesafe çok uzun — shape arc projeksiyon hataları olabileceğinden
            // straight-line (haversine) mesafesiyle kontrol edilir; gerçek sorun varsa o da büyük olur.
            // Demiryolu seferleri için daha yüksek eşik (config.rail_stop_distance_km).
            let stm026_threshold = if is_rail_route_type(route_type) {
                config.rail_stop_distance_km
            } else {
                50.0
            };
            if haver_km > stm026_threshold {
                let mut n026 = k6_notice(
                    ctr, "STM_026", EntityType::Trip,
                    Some(trip_id.to_string()), Some(trip_id.to_string()),
                    "stop_times.txt", Some(b.line), Some("stop_id"),
                    Some(format!("{dist_km:.1} km")), Some(format!("<= {stm026_threshold:.0} km")),
                    format!("trip_id '{trip_id}' stop_sequence {}-{} arası mesafe {dist_km:.1} km.",
                        a.stop_sequence.unwrap_or(0), b.stop_sequence.unwrap_or(0)),
                    "stops.txt koordinatlarını ve stop_times.txt sırasını doğrulayın.",
                );
                let mut d = std::collections::HashMap::new();
                d.insert("stop_a".to_string(), a.stop_id.to_string());
                d.insert("stop_b".to_string(), b.stop_id.to_string());
                n026.details = Some(d);
                notices.push(n026);
            }

            let speed = dist_km / (dt_sec as f64 / 3600.0);

            // STM_012: fiziksel olarak imkansız hız (mutlak üst sınır 700 km/h)
            if speed > 700.0 {
                let mut n012 = k6_notice(
                    ctr, "STM_012", EntityType::Trip,
                    Some(trip_id.to_string()), Some(trip_id.to_string()),
                    "stop_times.txt", Some(b.line), Some("arrival_time"),
                    Some(format!("{speed:.0} km/h")), Some("<= 700 km/h".to_string()),
                    format!("trip_id '{trip_id}' stop_sequence {}-{} arası hız {speed:.0} km/h — fiziksel olarak imkansız.",
                        a.stop_sequence.unwrap_or(0), b.stop_sequence.unwrap_or(0)),
                    "stop_times.txt zaman ve stops.txt koordinat verilerini doğrulayın.",
                );
                let mut d = std::collections::HashMap::new();
                d.insert("stop_a".to_string(), a.stop_id.to_string());
                d.insert("stop_b".to_string(), b.stop_id.to_string());
                n012.details = Some(d);
                notices.push(n012);
                trip_bad_seg_count += 1;
                continue;
            }

            if speed > threshold {
                notices.push(k6_notice(
                    ctr,
                    "STM_014",
                    EntityType::Trip,
                    Some(trip_id.to_string()),
                    Some(trip_id.to_string()),
                    "stop_times.txt",
                    Some(b.line),
                    Some("arrival_time"),
                    Some(format!("{speed:.1} km/h ({} → {})", a.stop_id, b.stop_id)),
                    Some(format!("≤ {threshold:.0} km/h")),
                    format!(
                        "'{route_label}' kodlu hattın {dir_sd} yönünde {svc_sd} çalışma takviminde \
                        '{trip_id}' seferindeki durak sırası {}-{} arası hız {:.1} km/h — eşik {:.0} km/h.",
                        a.stop_sequence.unwrap_or(0),
                        b.stop_sequence.unwrap_or(0),
                        speed,
                        threshold
                    ),
                    "Zaman damgalarını veya durak koordinatlarını doğrulayın; ardışık stop_times arasındaki hız eşiği aşıyor.",
                ));

                if speed > trip_max_speed {
                    trip_max_speed = speed;
                    trip_max_speed_line = Some(b.line);
                }
                trip_bad_seg_count += 1;
                bad_seg_stops.push((a.stop_id.clone(), b.stop_id.clone()));
            }
        }

        // OPR_008: yalnızca birden fazla bozuk segment varsa özet notice üret.
        // Tek segment bozuksa STM_014 zaten o bilgiyi taşır — OPR_008 tekrar olur.
        if trip_bad_seg_count > 1 {
            let mut n = k6_notice(
                ctr,
                "OPR_008",
                EntityType::Trip,
                Some(trip_id.to_string()),
                Some(trip_id.to_string()),
                "stop_times.txt",
                trip_max_speed_line,
                None,
                Some(format!("{trip_max_speed:.1} km/h")),
                Some(format!("≤ {threshold:.0} km/h")),
                format!(
                    "'{route_label}' kodlu hattın {dir_sd} yönünde {svc_sd} çalışma takviminde \
                    '{trip_id}' seferindeki {trip_bad_seg_count} bozuk segmentte en yüksek hız {trip_max_speed:.1} km/h — eşik {threshold:.0} km/h."
                ),
                "stop_times zaman ve koordinat verilerini kontrol edin.",
            );
            // Tüm bozuk segment çiftleri → UI'da her biri kırmızı polyline
            if !bad_seg_stops.is_empty() {
                let mut d = std::collections::HashMap::new();
                for (i, (sa, sb)) in bad_seg_stops.iter().enumerate() {
                    d.insert(format!("bad_seg_{i}_a"), sa.to_string());
                    d.insert(format!("bad_seg_{i}_b"), sb.to_string());
                }
                n.details = Some(d);
            }
            notices.push(n);
        }

        // STM_020: trip başına en büyük mesafeli sıfır-geçiş-süreli segment (tek notice/trip)
        if let Some((dist_km, line, stop_a, stop_b, seq_a, seq_b)) = worst_zero_seg {
            let mut n = k6_notice(
                ctr,
                "STM_020",
                EntityType::Trip,
                Some(trip_id.to_string()),
                Some(trip_id.to_string()),
                "stop_times.txt",
                Some(line),
                Some("arrival_time"),
                Some(format!("0s ({:.0}m mesafe)", dist_km * 1000.0)),
                Some("> 0s".to_string()),
                format!(
                    "'{}' hattının{} seferinde {}. ve {}. duraklar arası {:.0}m mesafe var ama geçiş süresi 0 girilmiş.",
                    route,
                    if dep_str.is_empty() { String::new() } else { format!(" {dep_str}") },
                    seq_a, seq_b, dist_km * 1000.0,
                ),
                "Kalkış/varış zamanlarını doğrulayın; ardışık duraklar arasında geçiş süresi sıfır olamaz.",
            );
            n.details = Some([
                ("stop_a".to_string(), stop_a.to_string()),
                ("stop_b".to_string(), stop_b.to_string()),
            ].into_iter().collect());
            notices.push(n);
        }
    }
    } // K6::sd::loop
}

// ── WP-09b: Frequency headway ─────────────────────────────────────────────────

fn check_frequency_headway(
    records: &EntityRecords,
    config: &ValidatorConfig,
    notices: &mut Vec<Notice>,
    ctr: &mut u32,
) {
    let max_secs = config.max_headway_warning_min * 60;
    let bunching_secs = config.bunching_threshold_min * 60;

    for frq in &records.frequencies {
        let Some(hw) = frq.headway_secs else { continue };
        let trip_id = if frq.trip_id.is_empty() { continue } else { &frq.trip_id };

        if hw > max_secs {
            notices.push(k6_notice(
                ctr,
                "FRQ_006",
                EntityType::Trip,
                Some(trip_id.clone()),
                Some(trip_id.clone()),
                "frequencies.txt",
                Some(frq.line),
                Some("headway_secs"),
                Some(format!("{hw}s ({:.0}dk)", hw as f64 / 60.0)),
                Some(format!("≤ {}s", max_secs)),
                format!("'{trip_id}' seferinde sefer aralığı {hw}sn — eşik {max_secs}sn ({} dk).",
                    config.max_headway_warning_min),
                "Seferler arası süreyi azaltın veya ek sefer ekleyin.",
            ));
        }

        // FRQ_010: çok yüksek frekanslı sefer (bunching eşiği altında) → bilgi
        if hw <= bunching_secs {
            notices.push(k6_notice(
                ctr,
                "FRQ_010",
                EntityType::Trip,
                Some(trip_id.clone()),
                Some(trip_id.clone()),
                "frequencies.txt",
                Some(frq.line),
                Some("headway_secs"),
                Some(format!("{hw}s")),
                Some(format!("> {}s", bunching_secs)),
                format!("'{trip_id}' seferinde sefer aralığı {hw}sn ≤ sıkışma eşiği {bunching_secs}sn — sıkışma riski."),
                "Sefer programını gözden geçirin; bu headway gerçekten isteniyorsa dikkate almayın.",
            ));
        }
    }
}

// ── WP-09b: Route headway (düzenli servis) ────────────────────────────────────

fn check_route_headway(
    records: &EntityRecords,
    config: &ValidatorConfig,
    idx: &StopTimesIndex<'_>,
    notices: &mut Vec<Notice>,
    ctr: &mut u32,
) {
    let route_short_hw: HashMap<&str, &str> = records.routes.iter()
        .map(|r| {
            let label = r.route_short_name.as_deref().filter(|s| !s.is_empty()).unwrap_or(r.route_id.as_str());
            (r.route_id.as_str(), label)
        })
        .collect();

    // Grouping key: (route_id, direction_key, service_id)
    // direction_key: direction_id varsa "0"/"1"/..., yoksa "" (ayrı bucket açmaz)
    let mut route_departures: HashMap<(&str, &str, &str), Vec<u32>> = HashMap::new();

    for trip in &records.trips {
        let route_id = trip.route_id.as_str();
        if route_id.is_empty() {
            continue;
        }
        let direction_key: &str = match trip.direction_id {
            Some(0) => "0",
            Some(1) => "1",
            Some(d) => {
                // Beklenmedik değer: boş string ile normalize et (ayrı bucket açma)
                let _ = d;
                ""
            }
            None => "",
        };
        let service_id = trip.service_id.as_str();

        let Some(&dep) = idx.trip_first_dep.get(trip.trip_id.as_str()) else {
            continue;
        };

        route_departures
            .entry((route_id, direction_key, service_id))
            .or_default()
            .push(dep);
    }
    // trip_route HashMap artık gerekli değil — trips üzerinde doğrudan dönüyoruz.

    let max_secs = config.max_headway_warning_min * 60;
    let bunching_secs = config.bunching_threshold_min * 60;

    for ((route_id, direction_key, service_id), mut deps) in route_departures {
        deps.sort_unstable();
        // dedup suppresses cross-day repeated departure times for the same
        // service_id; may also hide true parallel same-time trips (known tradeoff)
        deps.dedup();

        if deps.len() < 2 {
            continue;
        }

        let dir_display = if direction_key.is_empty() { "-" } else { direction_key };
        let route_label_hw = route_short_hw.get(route_id).copied().unwrap_or(route_id);

        let headways: Vec<u32> = deps.windows(2).map(|w| w[1] - w[0]).collect();
        let max_hw = headways.iter().copied().max().unwrap_or(0);
        let min_hw = headways.iter().copied().min().unwrap_or(0);

        if max_hw > max_secs {
            notices.push(k6_notice(
                ctr,
                "OPR_001",
                EntityType::Route,
                Some(route_id.to_string()),
                Some(route_id.to_string()),
                "stop_times.txt",
                None,
                None,
                Some(format!("{:.0}dk", max_hw as f64 / 60.0)),
                Some(format!("≤ {}dk", config.max_headway_warning_min)),
                format!("'{route_label_hw}' kodlu hattın {dir_display} yönünde {service_id} çalışma takviminde maksimum sefer aralığı {:.0}dk — eşik {}dk.",
                    max_hw as f64 / 60.0, config.max_headway_warning_min),
                "Pik/saatdışı sefer sayısını artırın ya da büyük boşlukları kapatın.",
            ));
        }

        if min_hw < bunching_secs && min_hw > 0 {
            notices.push(k6_notice(
                ctr,
                "OPR_003",
                EntityType::Route,
                Some(route_id.to_string()),
                Some(route_id.to_string()),
                "stop_times.txt",
                None,
                None,
                Some(format!("{:.0}dk", min_hw as f64 / 60.0)),
                Some(format!("> {}dk", config.bunching_threshold_min)),
                format!("'{route_label_hw}' kodlu hattın {dir_display} yönünde {service_id} çalışma takviminde minimum sefer aralığı {:.0}dk ≤ sıkışma eşiği {}dk.",
                    min_hw as f64 / 60.0, config.bunching_threshold_min),
                "Sefer programını düzenleyin; çok sık gelen seferler sıkışmaya yol açabilir.",
            ));
        }
    }
}

// ── WP-09b: Takvim analizi ────────────────────────────────────────────────────

fn check_calendar_analytics(
    records: &EntityRecords,
    derived: &DerivedData,
    config: &ValidatorConfig,
    today_yyyymmdd: u32,
    notices: &mut Vec<Notice>,
    ctr: &mut u32,
) {
    // CAL_007: servis içinde ardışık boşluk (service_gap_days'den büyük)
    // CAL_010: toplam aktif gün sayısı çok az (≤ service_gap_days)
    // CAL_012: servis boyunca service_gap_days veya daha büyük boşluk var
    for (service_id, dates) in &derived.calendar_bitmap.active_dates {
        if dates.is_empty() {
            continue;
        }
        let mut sorted: Vec<u32> = dates.iter().copied().collect();
        sorted.sort_unstable();

        let total_days = sorted.len() as u32;
        let gap_threshold = config.service_gap_days;

        // CAL_010: çok kısa servis
        if total_days <= gap_threshold {
            notices.push(k6_notice(
                ctr,
                "CAL_010",
                EntityType::Service,
                Some(service_id.clone()),
                Some(service_id.clone()),
                "calendar.txt",
                None,
                None,
                Some(format!("{total_days} gün")),
                Some(format!("> {gap_threshold} gün")),
                format!("'{service_id}' takviminde yalnızca {total_days} aktif gün var."),
                "Servis takvimini genişletin ya da bu servisin kısa olduğunu belgeleyin.",
            ));
        }

        // CAL_007: servis döneminde boşluk (geçmiş dahil tüm boşluklar)
        // CAL_012: boşluk yakın gelecekte (bugünden ±30 gün) → yolcu etkilenir (Yüksek)
        let today_jdn = yyyymmdd_to_approx_jdn(today_yyyymmdd);
        for pair in sorted.windows(2) {
            let a_jdn = yyyymmdd_to_approx_jdn(pair[0]);
            let b_jdn = yyyymmdd_to_approx_jdn(pair[1]);
            // Boşluk = pair[0] ile pair[1] arasındaki eksik günler (her ikisi dışlayıcı)
            let gap_days = b_jdn.saturating_sub(a_jdn).saturating_sub(1);
            if gap_days >= gap_threshold {
                notices.push(k6_notice(
                    ctr,
                    "CAL_007",
                    EntityType::Service,
                    Some(format!("{}@{}", service_id, pair[0])),
                    Some(service_id.clone()),
                    "calendar.txt",
                    None,
                    None,
                    Some(format!("{gap_days} gün boşluk ({}-{})", pair[0], pair[1])),
                    Some(format!("< {gap_threshold} gün boşluk")),
                    format!("'{service_id}' takviminde {}-{} arası {gap_days} günlük boşluk.",
                        pair[0], pair[1]),
                    "Boşluk kasıtlıysa calendar_dates ile açıklayın; değilse takvimi düzeltin.",
                ));

                // CAL_012: yalnızca boşluk dönemi bugün veya yakın gelecekle örtüşüyorsa
                // (boşluk başı ≤ today + 30 gün VE boşluk sonu ≥ bugün)
                let gap_start_jdn = a_jdn + 1; // ilk eksik gün
                let gap_end_jdn = b_jdn.saturating_sub(1); // son eksik gün
                let near_future = gap_end_jdn >= today_jdn
                    && gap_start_jdn <= today_jdn + 30;
                if near_future {
                    notices.push(k6_notice(
                        ctr,
                        "CAL_012",
                        EntityType::Service,
                        Some(service_id.clone()),
                        Some(service_id.clone()),
                        "calendar.txt",
                        None,
                        None,
                        Some(format!("{gap_days} gün")),
                        Some(format!("< {gap_threshold} gün")),
                        format!("'{service_id}' takviminde {}-{} arası {gap_days} günlük boşluk — yolcu deneyimi etkilenebilir.",
                            pair[0], pair[1]),
                        "Servis boşluğunu kapatın ya da alternatif hat sağlayın.",
                    ));
                }
            }
        }
    }

    // CAL_008 / CAL_009: expiry (bitiş tarihine göre)
    for cal in &records.calendars {
        let Some((ey, em, ed)) = cal.end_date else { continue };
        let end_yyyymmdd = ey * 10000 + em * 100 + ed;

        if today_yyyymmdd > 0 {
            if end_yyyymmdd < today_yyyymmdd {
                // CAL_013: tekil servis süresi dolmuş — blocker değil, bilgi düzeyi.
                // Tüm servisler sona ermişse k4_cross_ref CAL_009 (KRİTİK) zaten atar.
                notices.push(k6_notice(
                    ctr,
                    "CAL_013",
                    EntityType::Service,
                    Some(cal.service_id.clone()),
                    Some(cal.service_id.clone()),
                    "calendar.txt",
                    Some(cal.line),
                    Some("end_date"),
                    Some(format!("{end_yyyymmdd}")),
                    Some(format!("≥ {today_yyyymmdd}")),
                    format!("'{}' takviminin süresi dolmuş (son tarih: {end_yyyymmdd}). Bu servise ait seferler bugün için bulunamaz — diğer aktif servisler etkilenmiyor.",
                        cal.service_id),
                    "Feed'i yeni geçerlilik tarihleriyle güncelleyin veya servisi silin.",
                ));
            } else {
                // CAL_008: yakında bitiyor
                let warning_days = config.feed_expiry_warning_days;
                let end_jdn = yyyymmdd_to_approx_jdn(end_yyyymmdd);
                let today_jdn = yyyymmdd_to_approx_jdn(today_yyyymmdd);
                if end_jdn > today_jdn && end_jdn - today_jdn <= warning_days {
                    notices.push(k6_notice(
                        ctr,
                        "CAL_008",
                        EntityType::Service,
                        Some(cal.service_id.clone()),
                        Some(cal.service_id.clone()),
                        "calendar.txt",
                        Some(cal.line),
                        Some("end_date"),
                        Some(format!("{end_yyyymmdd}")),
                        Some(format!("> {} gün kaldı", warning_days)),
                        format!("'{}' takvimi {end_yyyymmdd} tarihinde bitiyor — {} gün kaldı.",
                            cal.service_id, end_jdn - today_jdn),
                        "Feed'i güncellemeyi planlayın.",
                    ));
                }
            }
        }
    }

    // CLD_007: aynı service_id için çok sayıda calendar_dates exception (> total aktif gün / 2)
    let mut override_counts: HashMap<&str, u32> = HashMap::new();
    for cd in &records.calendar_dates {
        if !cd.service_id.is_empty() {
            *override_counts.entry(cd.service_id.as_str()).or_default() += 1;
        }
    }
    for (service_id, &count) in &override_counts {
        let active = derived
            .calendar_bitmap
            .active_dates
            .get(*service_id)
            .map(|s| s.len() as u32)
            .unwrap_or(0);
        // CAL_013: hizmet calendar_dates'te tanımlı ama tüm tarihleri geçmişte
        // (calendar.txt'ten bağımsız — calendar_dates-only feed'leri de kapsar)
        if today_yyyymmdd > 0 && active > 0 {
            let max_date = derived
                .calendar_bitmap
                .active_dates
                .get(*service_id)
                .and_then(|s| s.iter().max().copied());
            if let Some(max_d) = max_date {
                if max_d < today_yyyymmdd {
                    notices.push(k6_notice(
                        ctr,
                        "CAL_013",
                        EntityType::Service,
                        Some(service_id.to_string()),
                        Some(service_id.to_string()),
                        "calendar_dates.txt",
                        None,
                        Some("date"),
                        Some(format!("{max_d}")),
                        Some(format!("≥ {today_yyyymmdd}")),
                        format!("'{service_id}' takviminin süresi dolmuş (son aktif tarih: {max_d})."),
                        "Feed'i yeni geçerlilik tarihleriyle güncelleyin.",
                    ));
                }
            }
        }

        // CAL_014: servis aktif tarihleri feed_info geçerlilik penceresinin dışında
        if let Some(fi) = records.feed_info.first() {
            if let (Some(fs), Some(fe)) = (fi.feed_start_date, fi.feed_end_date) {
                let fs_yyyymmdd = fs.0 * 10000 + fs.1 * 100 + fs.2;
                let fe_yyyymmdd = fe.0 * 10000 + fe.1 * 100 + fe.2;
                if let Some(dates) = derived.calendar_bitmap.active_dates.get(*service_id) {
                    let has_before = dates.iter().any(|&d| d < fs_yyyymmdd);
                    let has_after  = dates.iter().any(|&d| d > fe_yyyymmdd);
                    if has_before || has_after {
                        notices.push(k6_notice(
                            ctr,
                            "CAL_014",
                            EntityType::Service,
                            Some(service_id.to_string()),
                            Some(service_id.to_string()),
                            "calendar.txt",
                            None,
                            None,
                            None,
                            None,
                            format!(
                                "'{service_id}' servisinin aktif tarihleri feed_info geçerlilik aralığı \
                                 ({fs_yyyymmdd}–{fe_yyyymmdd}) dışına taşıyor."
                            ),
                            "feed_info.txt'deki feed_start_date/feed_end_date değerlerini servis tarih \
                             aralığını kapsayacak şekilde güncelleyin.",
                        ));
                    }
                }
            }
        }

        // Aktif günlerin yarısından fazlası override ise şüpheli
        if active > 0 && count > active / 2 {
            notices.push(k6_notice(
                ctr,
                "CLD_007",
                EntityType::Service,
                Some(service_id.to_string()),
                Some(service_id.to_string()),
                "calendar_dates.txt",
                None,
                None,
                Some(format!("{count} exception")),
                None,
                format!("'{service_id}' takviminde {count} özel gün tanımı — temel programdan çok sapma."),
                "calendar.txt base schedule'ı güncellemeyi tercih edin; calendar_dates'i istisnalar için kullanın.",
            ));
        }
    }

    if today_yyyymmdd > 0 && !derived.calendar_bitmap.active_dates.is_empty() {
        // CAL_015: en erken aktif tarih gelecekte — feed henüz aktif değil
        let min_date = derived.calendar_bitmap.active_dates.values()
            .flat_map(|s| s.iter().copied())
            .min();
        if let Some(first) = min_date {
            if first > today_yyyymmdd {
                notices.push(k6_notice(
                    ctr, "CAL_015", EntityType::Feed,
                    None, None, "calendar.txt", None, None,
                    Some(format!("{first}")), Some(format!("≤ {today_yyyymmdd}")),
                    format!("Feed'in en erken servis tarihi {first}; bu tarih henüz gelmedi — seferler bugün için mevcut değil."),
                    "Feed yayınlama zamanlamasını gözden geçirin ya da calendar.txt'i düzeltin.",
                ));
            }
        }

        // CAL_017: bireysel service_id'nin tüm aktif tarihleri gelecekte
        for (service_id, dates) in &derived.calendar_bitmap.active_dates {
            if dates.is_empty() { continue; }
            let min_svc = dates.iter().copied().min().unwrap();
            if min_svc > today_yyyymmdd {
                notices.push(k6_notice(
                    ctr, "CAL_017", EntityType::Service,
                    Some(service_id.to_string()), Some(service_id.to_string()),
                    "calendar.txt", None, Some("start_date"),
                    Some(format!("{min_svc}")), Some(format!("≤ {today_yyyymmdd}")),
                    format!("'{service_id}' takvimi henüz başlamamış; en erken aktif tarih {min_svc}."),
                    "Takvim başlangıç tarihini veya calendar_dates.txt girişlerini gözden geçirin.",
                ));
            }
        }

        // CAL_016: en geç aktif tarih 2 yıldan fazla ileriye uzanıyor
        let today_jdn = yyyymmdd_to_approx_jdn(today_yyyymmdd);
        let far_future_jdn = today_jdn + 730;
        let max_date = derived.calendar_bitmap.active_dates.values()
            .flat_map(|s| s.iter().copied())
            .max();
        if let Some(last) = max_date {
            let last_jdn = yyyymmdd_to_approx_jdn(last);
            if last_jdn > far_future_jdn {
                notices.push(k6_notice(
                    ctr, "CAL_016", EntityType::Feed,
                    None, None, "calendar.txt", None, None,
                    Some(format!("{last}")), Some(format!("≤ +2 yıl")),
                    format!("Feed'in en son servis tarihi {last} — bugünden 2 yıldan fazla ileride; bu veri bozuk olabilir."),
                    "Servis takvimini gerçekçi bir bitiş tarihiyle sınırlandırın.",
                ));
            }
        }
    }
}

// ── WP-09c: Coğrafi analitik ─────────────────────────────────────────────────

fn check_geo_analytics(
    records: &EntityRecords,
    derived: &DerivedData,
    config: &ValidatorConfig,
    notices: &mut Vec<Notice>,
    ctr: &mut u32,
) {
    use crate::timing::Timer;

    // GEO_006: shape segmentleri arası büyük atlama (jump)
    // GEO_007: aynı — GEO_006'dan biraz farklı ciddiyette ama aynı kontrol
    let _tg6 = Timer::start("K6::geo::geo_006");
    let max_jump_km = config.max_shape_jump_km;
    for (shape_id, seg) in &derived.shape_geometry.shapes {
        for (i, &dist) in seg.segment_distances_km.iter().enumerate() {
            if dist > max_jump_km {
                notices.push(k6_notice(
                    ctr,
                    "GEO_006",
                    EntityType::Shape,
                    Some(shape_id.clone()),
                    Some(shape_id.clone()),
                    "shapes.txt",
                    None,
                    Some("shape_pt_lat|shape_pt_lon"),
                    Some(format!("{dist:.2} km (segment {i}→{})", i + 1)),
                    Some(format!("≤ {max_jump_km} km")),
                    format!("'{shape_id}' güzergahının {i}→{}. segmentinde {dist:.2}km atlama — eşik {max_jump_km}km.",
                        i + 1),
                    "Aralarında koordinat eksik olan güzergah noktalarını ekleyin.",
                ));
            }
        }
    }

    drop(_tg6);

    // STP_017 / STP_016: birbirine çok yakın iki durak
    // Lat-sıralı sliding window — O(n log n), eski O(n²) spatial-grid yerine.
    // SPATIAL_CELL_DEG=0.5° ile tüm şehir tek hücreye düşüyordu (~4M çift); bu önler.
    let _tstp17 = Timer::start("K6::geo::stp_017");
    {
        let too_close_km = config.stop_too_close_m / 1000.0;
        let lat_band = too_close_km / 111.0;

        let mut sorted_stops: Vec<(f64, f64, &crate::k2::stops::StopRecord)> = records
            .stops
            .iter()
            .filter_map(|s| s.stop_lat.zip(s.stop_lon).map(|(la, lo)| (la, lo, s)))
            .collect();
        sorted_stops.sort_by(|a, b| a.0.total_cmp(&b.0));

        for i in 0..sorted_stops.len() {
            let (la_i, lo_i, sa) = sorted_stops[i];
            for j in (i + 1)..sorted_stops.len() {
                let (la_j, lo_j, sb) = sorted_stops[j];
                if la_j - la_i > lat_band {
                    break;
                }
                let dist_km = haversine_km(la_i, lo_i, la_j, lo_j);
                if dist_km == 0.0 {
                    notices.push(k6_notice(
                        ctr,
                        "STP_016",
                        EntityType::Stop,
                        Some(sa.stop_id.clone()),
                        Some(sa.stop_id.clone()),
                        "stops.txt",
                        Some(sa.line),
                        Some("stop_lat|stop_lon"),
                        Some(format!("({la_i}, {lo_i}) == '{}'", sb.stop_id)),
                        None,
                        format!(
                            "'{}' (kod: '{}') ve '{}' (kod: '{}') tam olarak aynı koordinatta ({la_i:.5}, {lo_i:.5}).",
                            sa.stop_name.as_deref().unwrap_or(sa.stop_id.as_str()), sa.stop_id,
                            sb.stop_name.as_deref().unwrap_or(sb.stop_id.as_str()), sb.stop_id,
                        ),
                        "İki durak aynı konumdaysa birleştirin; farklı duraklar ise koordinatları düzeltin.",
                    ));
                } else if dist_km < too_close_km {
                    let dist_m = dist_km * 1000.0;
                    let n = k6_notice(
                        ctr,
                        "STP_017",
                        EntityType::Stop,
                        Some(sa.stop_id.clone()),
                        Some(sa.stop_id.clone()),
                        "stops.txt",
                        Some(sa.line),
                        Some("stop_lat|stop_lon"),
                        Some(format!("{dist_m:.1}m → '{}'", sb.stop_id)),
                        Some(format!("> {:.0}m", config.stop_too_close_m)),
                        format!("'{}' (kod: '{}') ile '{}' (kod: '{}') arası {dist_m:.1}m — eşik {:.0}m.",
                            sa.stop_name.as_deref().unwrap_or(sa.stop_id.as_str()), sa.stop_id,
                            sb.stop_name.as_deref().unwrap_or(sb.stop_id.as_str()), sb.stop_id,
                            config.stop_too_close_m),
                        "Durakları birleştirin ya da konumlarını doğrulayın.",
                    );
                    notices.push(n);
                }
            }
        }
        // cell_deg sadece bu bağlamda referans gerekiyor; GEO_014 onu kullanmıyor
        let _ = derived.spatial_index.cell_deg;
    }
    drop(_tstp17);

    // GEO_002: stop koordinatları bounding box dışında (feed genelinde istatistiksel anomali)
    // Feed'deki tüm durakların median koordinatını hesapla; çok uzakta olanları işaretle
    let _tgeo2 = Timer::start("K6::geo::geo_002_014");
    let coords: Vec<(f64, f64)> = records
        .stops
        .iter()
        .filter_map(|s| s.stop_lat.zip(s.stop_lon))
        .collect();
    if coords.len() >= 3 {
        let mut lats: Vec<f64> = coords.iter().map(|c| c.0).collect();
        let mut lons: Vec<f64> = coords.iter().map(|c| c.1).collect();
        lats.sort_by(f64::total_cmp);
        lons.sort_by(f64::total_cmp);
        let med_lat = lats[lats.len() / 2];
        let med_lon = lons[lons.len() / 2];

        for stop in &records.stops {
            let (Some(lat), Some(lon)) = (stop.stop_lat, stop.stop_lon) else { continue };
            // Median'dan > 200 km uzakta olan duraklar → potansiyel koordinat hatası
            let d = haversine_km(med_lat, med_lon, lat, lon);
            if d > 200.0 {
                notices.push(k6_notice(
                    ctr,
                    "GEO_002",
                    EntityType::Stop,
                    Some(stop.stop_id.clone()),
                    Some(stop.stop_id.clone()),
                    "stops.txt",
                    Some(stop.line),
                    Some("stop_lat|stop_lon"),
                    Some(format!("({lat:.5},{lon:.5}) — median'dan {d:.0}km")),
                    Some("≤ 200km (feed medianından)".to_string()),
                    format!("'{}' durağı (kod: '{}') feed medianından {d:.0}km uzakta — koordinat hatası olabilir.",
                        stop.stop_name.as_deref().unwrap_or(stop.stop_id.as_str()), stop.stop_id),
                    "stop_lat ve stop_lon değerlerini doğrulayın.",
                ));
            }
        }
    }

    // GEO_014: feed coğrafi kapsam bilgisi (Bilgi seviyesi — feed bbox'ı hesapla)
    if !coords.is_empty() {
        let min_lat = coords.iter().map(|c| c.0).fold(f64::INFINITY, f64::min);
        let max_lat = coords.iter().map(|c| c.0).fold(f64::NEG_INFINITY, f64::max);
        let min_lon = coords.iter().map(|c| c.1).fold(f64::INFINITY, f64::min);
        let max_lon = coords.iter().map(|c| c.1).fold(f64::NEG_INFINITY, f64::max);
        let span_km = haversine_km(min_lat, min_lon, max_lat, max_lon);
        if span_km > 500.0 {
            // Çok geniş bir feed → bilgi notu
            notices.push(k6_notice(
                ctr,
                "GEO_014",
                EntityType::Feed,
                None,
                None,
                "stops.txt",
                None,
                None,
                Some(format!("{span_km:.0} km köşegen")),
                None,
                format!("Feed coğrafi kapsamı geniş: {span_km:.0}km köşegen (bbox: {min_lat:.3},{min_lon:.3} → {max_lat:.3},{max_lon:.3})."),
                "Bu bilgi notu; büyük ağlar için beklenen bir durumdur.",
            ));
        }
    }
    // GEO_015: Japonya koordinat sınırı — feed_lang=ja ise durak koordinatları Japonya dışında olmamalı
    // Japonya coğrafi sınırları: lat 20.25–45.33, lon 122.56–153.59
    {
        let is_japanese = records.feed_info.first()
            .map(|fi| fi.feed_lang.starts_with("ja"))
            .unwrap_or(false);
        if is_japanese {
            for stop in &records.stops {
                if stop.stop_id.is_empty() { continue; }
                let (Some(lat), Some(lon)) = (stop.stop_lat, stop.stop_lon) else { continue };
                let in_japan = (20.25..=45.33).contains(&lat) && (122.56..=153.59).contains(&lon);
                if !in_japan {
                    let name = stop.stop_name.as_deref().filter(|s| !s.is_empty()).unwrap_or(&stop.stop_id);
                    notices.push(k6_notice(
                        ctr, "GEO_015", EntityType::Stop,
                        Some(stop.stop_id.clone()), Some(stop.stop_id.clone()),
                        "stops.txt", Some(stop.line), Some("stop_lat|stop_lon"),
                        Some(format!("{lat:.6},{lon:.6}")), None,
                        format!("'{}' durağının koordinatları ({lat:.6},{lon:.6}) Japonya sınırları dışında (lat: 20.25–45.33, lon: 122.56–153.59).",
                            name),
                        "stop_lat ve stop_lon değerlerinin doğru olduğunu kontrol edin.",
                    ));
                }
            }
        }
    }

    // GEO_016: Stop koordinatları Null Island yakınında (|lat| < 0.1 ve |lon| < 0.1)
    for stop in &records.stops {
        if stop.stop_id.is_empty() { continue; }
        let (Some(lat), Some(lon)) = (stop.stop_lat, stop.stop_lon) else { continue };
        if lat.abs() < 0.1 && lon.abs() < 0.1 {
            let name = stop.stop_name.as_deref().filter(|s| !s.is_empty()).unwrap_or(&stop.stop_id);
            notices.push(k6_notice(
                ctr, "GEO_016", EntityType::Stop,
                Some(stop.stop_id.clone()), Some(stop.stop_id.clone()),
                "stops.txt", Some(stop.line), Some("stop_lat|stop_lon"),
                Some(format!("{lat:.6},{lon:.6}")), None,
                format!("'{name}' durağının koordinatları ({lat:.6},{lon:.6}) Null Island yakınında — olası koordinat hatası."),
                "stop_lat ve stop_lon değerlerinin gerçek konuma karşılık geldiğini doğrulayın.",
            ));
        }
    }

    // GEO_017: Shape noktası Null Island yakınında — her shape için en fazla 1 notice
    {
        let mut flagged: HashSet<String> = HashSet::new();
        for pt in &records.shapes {
            if flagged.contains(&pt.shape_id) { continue; }
            let (Some(lat), Some(lon)) = (pt.shape_pt_lat, pt.shape_pt_lon) else { continue };
            if lat.abs() < 0.1 && lon.abs() < 0.1 {
                flagged.insert(pt.shape_id.clone());
                notices.push(k6_notice(
                    ctr, "GEO_017", EntityType::Shape,
                    Some(pt.shape_id.clone()), Some(pt.shape_id.clone()),
                    "shapes.txt", Some(pt.line), Some("shape_pt_lat|shape_pt_lon"),
                    Some(format!("{lat:.6},{lon:.6}")), None,
                    format!("'{}' şeklinde Null Island yakınında nokta bulundu ({lat:.6},{lon:.6}) — GPS veri hatası olabilir.", pt.shape_id),
                    "shapes.txt'deki sıfır değerli koordinatları kontrol edin.",
                ));
            }
        }
    }

    // GEO_018: Tüm feed durakları 200m'lik bir alan içinde — test/yer tutucu veri
    {
        let coords: Vec<(f64, f64)> = records.stops.iter()
            .filter_map(|s| s.stop_lat.zip(s.stop_lon))
            .collect();
        if coords.len() >= 3 {
            let min_lat = coords.iter().map(|(lat,_)| *lat).fold(f64::INFINITY, f64::min);
            let max_lat = coords.iter().map(|(lat,_)| *lat).fold(f64::NEG_INFINITY, f64::max);
            let min_lon = coords.iter().map(|(_,lon)| *lon).fold(f64::INFINITY, f64::min);
            let max_lon = coords.iter().map(|(_,lon)| *lon).fold(f64::NEG_INFINITY, f64::max);
            let span_km = haversine_km(min_lat, min_lon, max_lat, max_lon);
            if span_km < 0.2 {
                notices.push(k6_notice(
                    ctr, "GEO_018", EntityType::Feed,
                    None, None, "", None, None,
                    Some(format!("{:.0}m", span_km * 1000.0)),
                    Some("≥200m".to_string()),
                    format!("Feed'deki tüm {} durak {}m'lik bir alan içinde — test/yer tutucu veri olabilir.",
                        coords.len(), (span_km * 1000.0) as u64),
                    "Gerçek durak koordinatlarını stops.txt'e ekleyin.",
                ));
            }
        }
    }

    // GEO_019: Tam sayı (ondalık sıfır) koordinata sahip durak
    for stop in &records.stops {
        if stop.stop_id.is_empty() { continue; }
        let (Some(lat), Some(lon)) = (stop.stop_lat, stop.stop_lon) else { continue };
        if (lat - lat.round()).abs() < 1e-9 && (lon - lon.round()).abs() < 1e-9 {
            let name = stop.stop_name.as_deref().filter(|s| !s.is_empty()).unwrap_or(&stop.stop_id);
            notices.push(k6_notice(
                ctr, "GEO_019", EntityType::Stop,
                Some(stop.stop_id.clone()), Some(stop.stop_id.clone()),
                "stops.txt", Some(stop.line), Some("stop_lat|stop_lon"),
                Some(format!("{:.0},{:.0}", lat, lon)), None,
                format!("'{name}' durağının koordinatları ({:.0},{:.0}) tam sayı — düşük hassasiyetli veya yer tutucu veri.", lat, lon),
                "stop_lat ve stop_lon değerlerini en az 5 ondalık basamakla güncelleyin.",
            ));
        }
    }

    // GEO_020: Shape'in tüm noktaları aynı koordinatta (dejenere geometri)
    {
        let mut shape_first: HashMap<String, (f64, f64)> = HashMap::new();
        let mut shape_varied: HashSet<String> = HashSet::new();
        for pt in &records.shapes {
            if shape_varied.contains(&pt.shape_id) { continue; }
            let (Some(lat), Some(lon)) = (pt.shape_pt_lat, pt.shape_pt_lon) else { continue };
            if let Some(&(first_lat, first_lon)) = shape_first.get(&pt.shape_id) {
                if (lat - first_lat).abs() > 1e-8 || (lon - first_lon).abs() > 1e-8 {
                    shape_varied.insert(pt.shape_id.clone());
                }
            } else {
                shape_first.insert(pt.shape_id.clone(), (lat, lon));
            }
        }
        for (shape_id, (lat, lon)) in &shape_first {
            if shape_varied.contains(shape_id) { continue; }
            let count = records.shapes.iter().filter(|p| &p.shape_id == shape_id).count();
            if count >= 2 {
                notices.push(k6_notice(
                    ctr, "GEO_020", EntityType::Shape,
                    Some(shape_id.clone()), Some(shape_id.clone()),
                    "shapes.txt", None, None,
                    Some(format!("{lat:.6},{lon:.6}")), None,
                    format!("'{shape_id}' şeklinin tüm {count} noktası ({lat:.6},{lon:.6}) koordinatında — dejenere geometri."),
                    "shapes.txt'deki koordinatları gerçek güzergaha karşılık gelecek şekilde düzeltin.",
                ));
            }
        }
    }

    // GEO_021: Durakların >%30'u koordinatını başka bir durakla paylaşıyor
    {
        let mut coord_counts: HashMap<(i64, i64), u32> = HashMap::new();
        for stop in &records.stops {
            let (Some(lat), Some(lon)) = (stop.stop_lat, stop.stop_lon) else { continue };
            let key = ((lat * 1e6).round() as i64, (lon * 1e6).round() as i64);
            *coord_counts.entry(key).or_default() += 1;
        }
        let total_stops = records.stops.iter()
            .filter(|s| s.stop_lat.is_some() && s.stop_lon.is_some()).count();
        let shared: usize = coord_counts.values().filter(|&&c| c > 1).map(|&c| c as usize).sum();
        if total_stops >= 5 && shared as f64 / total_stops as f64 > 0.3 {
            let pct = shared as f64 / total_stops as f64 * 100.0;
            notices.push(k6_notice(
                ctr, "GEO_021", EntityType::Feed,
                None, None, "stops.txt", None, None,
                Some(format!("{pct:.0}%")), Some("≤30%".to_string()),
                format!("{shared}/{total_stops} durak ({pct:.0}%) koordinatını başka bir durakla paylaşıyor — sistematik koordinat sorunu."),
                "stops.txt'deki tekrar eden koordinatları düzeltin.",
            ));
        }
    }

    // STM_044: Feed stop_times satır sayısı 2 milyonu aşıyor
    {
        let total_st = records.stop_times_index.total_rows;
        if total_st > 2_000_000 {
            notices.push(k6_notice(
                ctr, "STM_044", EntityType::Feed,
                None, None, "stop_times.txt", None, None,
                Some(format!("{total_st}")), Some("≤2.000.000".to_string()),
                format!("stop_times.txt'de {total_st} satır var — WASM tüketicileri için ciddi bellek/performans riski."),
                "Büyük feed'leri zaman veya coğrafi bölgeye göre parçalara bölün.",
            ));
        }
    }

    // STM_045: Trip kalkış saati gece yarısından 26 saatten fazla
    {
        for (trip_id, stops) in records.stop_times_index.iter_trips() {
            for st in stops {
                let Some((h, m, s)) = st.departure_time else { continue };
                if h > 26 || (h == 26 && (m > 0 || s > 0)) {
                    notices.push(k6_notice(
                        ctr, "STM_045", EntityType::Trip,
                        Some(trip_id.to_string()), Some(trip_id.to_string()),
                        "stop_times.txt", Some(st.line), Some("departure_time"),
                        Some(format!("{h:02}:{m:02}:{s:02}")), Some("≤26:00:00".to_string()),
                        format!("'{trip_id}' seferinde {h:02}:{m:02}:{s:02} kalkış saati — gece yarısından 26 saatten fazla."),
                        "departure_time değerini kontrol edin; 26 saati aşan değerler genellikle veri hatasıdır.",
                    ));
                    break; // trip başına bir notice yeterli
                }
            }
        }
    }

    // SHP_027: Aynı shape 200'den fazla sefer tarafından kullanılıyor
    {
        let mut shape_trip_counts: HashMap<String, u32> = HashMap::new();
        for t in &records.trips {
            if let Some(shape_id) = &t.shape_id {
                if !shape_id.is_empty() {
                    *shape_trip_counts.entry(shape_id.clone()).or_default() += 1;
                }
            }
        }
        for (shape_id, count) in &shape_trip_counts {
            if *count > 200 {
                notices.push(k6_notice(
                    ctr, "SHP_027", EntityType::Shape,
                    Some(shape_id.clone()), Some(shape_id.clone()),
                    "trips.txt", None, None,
                    Some(count.to_string()), Some("≤200".to_string()),
                    format!("'{shape_id}' shape'i {count} sefer tarafından kullanılıyor — olası yanlış shape ataması."),
                    "Her güzergah yönü için ayrı shape_id tanımlayın.",
                ));
            }
        }
    }

    // STM_043: Sefer aşırı fazla durağa sahip (>200)
    {
        let trip_ids_set: FxHashSet<&str> = records.trips.iter().map(|t| t.trip_id.as_str()).collect();
        for (trip_id, stops) in records.stop_times_index.iter_trips() {
            let count = stops.len() as u32;
            let trip_id = trip_id.as_str();
            if count > 200 && trip_ids_set.contains(trip_id) {
                notices.push(k6_notice(
                    ctr, "STM_043", EntityType::Trip,
                    Some((*trip_id).to_string()), Some((*trip_id).to_string()),
                    "stop_times.txt", None, None,
                    Some(count.to_string()), Some("≤200".to_string()),
                    format!("'{trip_id}' seferinde {count} durak var — olası veri birleştirme hatası."),
                    "Bu sefer için stop_times.txt'i gözden geçirin; seferler mantıksal segmentlere ayrılabilir.",
                ));
            }
        }
    }

    // SHP_026: Shape aşırı fazla noktaya sahip (>5000)
    {
        let mut shape_counts: FxHashMap<&str, u32> = FxHashMap::default();
        for pt in &records.shapes {
            *shape_counts.entry(pt.shape_id.as_str()).or_default() += 1;
        }
        for (shape_id, count) in &shape_counts {
            if *count > 5000 {
                notices.push(k6_notice(
                    ctr, "SHP_026", EntityType::Shape,
                    Some((*shape_id).to_string()), Some((*shape_id).to_string()),
                    "shapes.txt", None, None,
                    Some(count.to_string()), Some("≤5000".to_string()),
                    format!("'{shape_id}' şeklinde {count} nokta var — harita render performansını olumsuz etkiler."),
                    "shapes.txt'i basitleştirmek için Douglas-Peucker vb. bir algoritma kullanın.",
                ));
            }
        }
    }

    drop(_tgeo2);
}

// ── WP-09c: Operasyonel analitik ─────────────────────────────────────────────

fn check_operational_analytics(
    records: &EntityRecords,
    derived: &DerivedData,
    _config: &ValidatorConfig,
    idx: &StopTimesIndex<'_>,
    today_yyyymmdd: u32,
    notices: &mut Vec<Notice>,
    ctr: &mut u32,
) {
    use crate::timing::Timer;

    let trip_to_route: HashMap<&str, &str> = records.trips.iter()
        .map(|t| (t.trip_id.as_str(), t.route_id.as_str()))
        .collect();
    let stop_name_map: HashMap<&str, &str> = records.stops.iter()
        .filter_map(|s| s.stop_name.as_deref().map(|n| (s.stop_id.as_str(), n)))
        .collect();

    // OPR_006: sefer başına durak sayısı çok az (< 2) — sadece stop_times olan trip'ler
    // (XFL_002 K4'te "trip has no stop_times" yakaladı; burası 1 duraklı trip'ler için)
    let _topr6 = Timer::start("K6::opr::opr_006");
    for (&trip_id, stimes) in &idx.by_trip {
        let count = stimes.len();
        if count < 2 {
            let line = stimes.first().map(|s| s.line);
            notices.push(k6_notice(
                ctr,
                "OPR_006",
                EntityType::Trip,
                Some(trip_id.to_string()),
                Some(trip_id.to_string()),
                "stop_times.txt",
                line,
                None,
                Some(format!("{count} durak")),
                Some("≥ 2 durak".to_string()),
                {
                    let r = trip_to_route.get(trip_id).copied().unwrap_or(trip_id);
                    format!("'{r}' hattının seferinde yalnızca {count} durak var — işlevsel sefer için en az 2 durak gerekli.")
                },
                "En az 2 durak içeren bir sefer tanımlayın.",
            ));
        }
    }

    drop(_topr6);

    // OPR_007: aynı trip_id + stop_id kombinasyonu (döngüsel sefer değilse tekrar)
    let _topr7 = Timer::start("K6::opr::opr_007");
    {
        let mut stop_counts: HashMap<&str, u32> = HashMap::new();
        for (&trip_id, stimes) in &idx.by_trip {
            stop_counts.clear();
            for st in stimes.iter() {
                if !st.stop_id.is_empty() {
                    *stop_counts.entry(st.stop_id.as_str()).or_default() += 1;
                }
            }
            // Ring/döngüsel hat tespiti: ilk ve son durak aynıysa terminal tekrarı suppress et
            let mut sorted_stops = stimes.iter()
                .filter(|st| !st.stop_id.is_empty())
                .collect::<Vec<_>>();
            sorted_stops.sort_by_key(|st| st.stop_sequence.unwrap_or(u32::MAX));
            let first = sorted_stops.first().map(|st| st.stop_id.as_str()).unwrap_or("");
            let last  = sorted_stops.last() .map(|st| st.stop_id.as_str()).unwrap_or("");
            let is_ring = !first.is_empty() && first == last;

            if let Some((&dup_stop, &count)) = stop_counts.iter()
                .find(|(&sid, &c)| c > 1 && !(is_ring && sid == first))
            {
                let route = trip_to_route.get(trip_id).copied().unwrap_or(trip_id);
                let stop_name = stop_name_map.get(dup_stop).copied().unwrap_or(dup_stop);
                let mut n = k6_notice(
                    ctr,
                    "OPR_007",
                    EntityType::Trip,
                    Some(trip_id.to_string()),
                    Some(trip_id.to_string()),
                    "stop_times.txt",
                    None,
                    None,
                    Some(format!("{count}× tekrar")),
                    None,
                    format!("'{}' hattının seferinde '{}' durağı (kod: '{}') {}× geçiyor — ring hat ise normaldir.",
                        route, stop_name, dup_stop, count),
                    "Döngüsel sefer değilse tekrar eden stop_id girişlerini temizleyin.",
                );
                // Harita için sıralı durak listesi + tekrarlayan durak
                let stop_list: Vec<&str> = stimes.iter()
                    .filter(|st| !st.stop_id.is_empty())
                    .map(|st| st.stop_id.as_str())
                    .collect();
                let mut det = HashMap::new();
                det.insert("stops".to_string(), stop_list.join(","));
                det.insert("dup_stop".to_string(), dup_stop.to_string());
                n.details = Some(det);
                notices.push(n);
            }
        }
    }

    drop(_topr7);

    // OPR_011: trip'in service_id'si calendar_bitmap'te aktif gün içermiyor
    // XFL_006 (K4) sadece "yalnızca removal exception" durumunu yakalar;
    // OPR_011 daha geniş: bitmap'te hiç entry olmayan service_id'leri de kapsar.
    {
        let mut reported_services: HashSet<&str> = HashSet::new();
        for t in &records.trips {
            let svc = t.service_id.as_str();
            if reported_services.contains(svc) {
                continue;
            }
            let has_active = derived
                .calendar_bitmap
                .active_dates
                .get(svc)
                .map(|dates| !dates.is_empty())
                .unwrap_or(false);
            if !has_active {
                reported_services.insert(svc);
                notices.push(k6_notice(
                    ctr,
                    "OPR_011",
                    EntityType::Service,
                    Some(svc.to_string()),
                    Some(svc.to_string()),
                    "calendar.txt",
                    None,
                    Some("service_id"),
                    Some("0 aktif gün".to_string()),
                    Some("≥ 1 aktif gün".to_string()),
                    format!("'{svc}' takviminde hiçbir aktif gün yok — bu takvimi kullanan seferler çalışmayacak."),
                    "calendar.txt veya calendar_dates.txt'de geçerli aktif günler tanımlayın.",
                ));
            }
        }
    }

    // OPR_016: feed genelinde hiçbir service_id aktif gün içermiyor (feed-level)
    {
        if !records.trips.is_empty() {
            let any_active = derived
                .calendar_bitmap
                .active_dates
                .values()
                .any(|dates| !dates.is_empty());
            if !any_active {
                notices.push(k6_notice(
                    ctr,
                    "OPR_016",
                    EntityType::Feed,
                    None,
                    None,
                    "calendar.txt",
                    None,
                    None,
                    Some("0 aktif servis".to_string()),
                    Some("≥ 1 aktif servis".to_string()),
                    "Feed'deki hiçbir service_id aktif takvim günü içermiyor — tüm seferler pasif.".to_string(),
                    "calendar.txt veya calendar_dates.txt'de en az bir servis için aktif gün tanımlayın.",
                ));
            }
        }
    }

    // TRP_023: önümüzdeki 7 günde aktif sefer yok
    if today_yyyymmdd > 0 && !records.trips.is_empty() {
        let today_jdn = yyyymmdd_to_approx_jdn(today_yyyymmdd);
        let active_in_7days = derived.calendar_bitmap.active_dates.values().any(|dates| {
            dates.iter().any(|&d| {
                let djdn = yyyymmdd_to_approx_jdn(d);
                djdn >= today_jdn && djdn < today_jdn + 7
            })
        });
        if !active_in_7days {
            notices.push(k6_notice(
                ctr, "TRP_023", EntityType::Feed,
                None, None, "calendar.txt", None, None,
                Some("0 aktif sefer (7 gün)".to_string()), None,
                "Önümüzdeki 7 günde aktif sefer bulunamadı — feed yakında devre dışı kalabilir.".to_string(),
                "calendar.txt veya calendar_dates.txt'i güncelleyin ya da yeni bir feed yayınlayın.",
            ));
        }
    }

    // TRP_030: sefer önümüzdeki 7 günde aktif değil (per-trip 7-day window)
    if today_yyyymmdd > 0 && !records.trips.is_empty() {
        let today_jdn = yyyymmdd_to_approx_jdn(today_yyyymmdd);
        for trip in &records.trips {
            if trip.trip_id.is_empty() { continue; }
            let active_in_7 = derived.calendar_bitmap.active_dates
                .get(trip.service_id.as_str())
                .map(|dates| dates.iter().any(|&d| {
                    let djdn = yyyymmdd_to_approx_jdn(d);
                    djdn >= today_jdn && djdn < today_jdn + 7
                }))
                .unwrap_or(false);
            if !active_in_7 {
                notices.push(k6_notice(
                    ctr, "TRP_030", EntityType::Trip,
                    Some(trip.trip_id.clone()), Some(trip.trip_id.clone()),
                    "trips.txt", Some(trip.line), Some("service_id"),
                    Some(trip.service_id.clone()), None,
                    format!("'{}' seferi önümüzdeki 7 günde aktif değil (service_id: '{}').",
                        trip.trip_id, trip.service_id),
                    "calendar.txt veya calendar_dates.txt'i güncelleyin ya da yeni bir feed yayınlayın.",
                ));
            }
        }
    }

    // TRP_026: hiç aktif hizmet günü olmayan sefer (UnusedTripNotice)
    if today_yyyymmdd > 0 {
        for trip in &records.trips {
            if trip.trip_id.is_empty() { continue; }
            let has_any_date = derived.calendar_bitmap.active_dates
                .get(trip.service_id.as_str())
                .map(|dates| !dates.is_empty())
                .unwrap_or(false);
            if !has_any_date {
                notices.push(k6_notice(
                    ctr, "TRP_026", EntityType::Trip,
                    Some(trip.trip_id.clone()), Some(trip.trip_id.clone()),
                    "trips.txt", Some(trip.line), Some("service_id"),
                    Some(trip.service_id.clone()), None,
                    format!("service_id '{}' için geçerli hizmet tarihi yok; '{}' seferi hiçbir zaman çalışmayacak.",
                        trip.service_id, trip.trip_id),
                    "service_id'nin calendar.txt veya calendar_dates.txt'te aktif tarihlere sahip olduğundan emin olun.",
                ));
            }
        }
    }

    // TRP_028/029: wheelchair_accessible eksikliği
    {
        let total = records.trips.iter().filter(|t| !t.trip_id.is_empty()).count();
        if total > 0 {
            let unset = records.trips.iter()
                .filter(|t| !t.trip_id.is_empty() && t.wheelchair_accessible.unwrap_or(0) == 0)
                .count();
            if unset == total {
                notices.push(k6_notice(
                    ctr, "TRP_029", EntityType::Feed,
                    None, None,
                    "trips.txt", None, Some("wheelchair_accessible"),
                    Some(format!("{total} sefer")), Some("1 veya 2".to_string()),
                    format!("{total} seferin hiçbirinde wheelchair_accessible bilgisi girilmemiş."),
                    "Tekerlekli sandalye erişilebilirliğini wheelchair_accessible alanıyla bildirin (1=erişilebilir, 2=erişilemez).",
                ));
            } else if unset > 0 {
                notices.push(k6_notice(
                    ctr, "TRP_028", EntityType::Feed,
                    None, None,
                    "trips.txt", None, Some("wheelchair_accessible"),
                    Some(format!("{unset}/{total} sefer")), Some("0".to_string()),
                    format!("{total} seferin {unset} tanesinde wheelchair_accessible bilgisi eksik ({:.0}%).",
                        unset as f64 / total as f64 * 100.0),
                    "Tüm seferlerin wheelchair_accessible alanını doldurun (1=erişilebilir, 2=erişilemez).",
                ));
            }
        }
    }

    // TRP_024: block içinde tutarsız rota tipi
    {
        let route_type_map: HashMap<&str, u32> = records.routes.iter()
            .filter(|r| !r.route_id.is_empty())
            .filter_map(|r| r.route_type.map(|rt| (r.route_id.as_str(), rt)))
            .collect();
        let mut block_route_types: HashMap<&str, (u32, &str)> = HashMap::new();
        for t in &records.trips {
            let Some(ref bid) = t.block_id else { continue };
            let Some(&rtype) = route_type_map.get(t.route_id.as_str()) else { continue };
            let entry = block_route_types.entry(bid.as_str()).or_insert((rtype, t.trip_id.as_str()));
            if entry.0 != rtype {
                notices.push(k6_notice(
                    ctr, "TRP_024", EntityType::Trip,
                    Some(t.trip_id.clone()), Some(t.trip_id.clone()),
                    "trips.txt", Some(t.line), Some("block_id"),
                    Some(format!("route_type={rtype}")), Some(format!("route_type={}", entry.0)),
                    format!("block_id '{}' içinde farklı rota tipleri: '{}' tip-{} ve '{}' trip-{rtype}.",
                        bid, entry.1, entry.0, t.trip_id),
                    "Aynı block içindeki tüm seferlerin aynı rota tipine sahip olmasını sağlayın.",
                ));
            }
        }
    }

    // TRP_022: block içinde çakışan sefer saatleri
    {
        // Her trip için [first_dep_secs, last_arr_secs] hesapla
        let mut trip_range: HashMap<&str, (u32, u32)> = HashMap::new();
        for (&trip_id, stimes) in &idx.by_trip {
            let first_dep = stimes.iter().find_map(|s| s.departure_time).map(hms_to_secs);
            let last_arr  = stimes.iter().rev().find_map(|s| s.arrival_time).map(hms_to_secs);
            if let (Some(dep), Some(arr)) = (first_dep, last_arr) {
                trip_range.insert(trip_id, (dep, arr));
            }
        }

        let mut block_trips: HashMap<&str, Vec<(&str, u32, u32)>> = HashMap::new();
        for t in &records.trips {
            let Some(ref bid) = t.block_id else { continue };
            let Some(&(dep, arr)) = trip_range.get(t.trip_id.as_str()) else { continue };
            block_trips.entry(bid.as_str()).or_default().push((t.trip_id.as_str(), dep, arr));
        }

        for (block_id, trips) in &block_trips {
            for i in 0..trips.len() {
                for j in (i + 1)..trips.len() {
                    let (tid_a, dep_a, arr_a) = trips[i];
                    let (tid_b, dep_b, arr_b) = trips[j];
                    if dep_a < arr_b && dep_b < arr_a {
                        notices.push(k6_notice(
                            ctr, "TRP_022", EntityType::Trip,
                            Some(tid_a.to_string()), Some(tid_a.to_string()),
                            "trips.txt", None, Some("block_id"),
                            Some(format!("{tid_a} ve {tid_b}")),
                            None,
                            format!("block_id '{block_id}' içinde '{tid_a}' ve '{tid_b}' seferlerinin saatleri çakışıyor."),
                            "Aynı araç bloğundaki seferlerin saatlerini örtüşmeyecek şekilde düzenleyin.",
                        ));
                    }
                }
            }
        }
    }

    // TRP_025: wheelchair_accessible bilgisi eksik veya belirtilmemiş (0) seferlerin oranı yüksek (> %80)
    if !records.trips.is_empty() {
        let unknown = records.trips.iter()
            .filter(|t| !t.trip_id.is_empty())
            .filter(|t| matches!(t.wheelchair_accessible, None | Some(0)))
            .count();
        let total = records.trips.iter().filter(|t| !t.trip_id.is_empty()).count();
        if total > 0 {
            let ratio = unknown as f64 / total as f64;
            if ratio > 0.80 {
                notices.push(k6_notice(
                    ctr, "TRP_025", EntityType::Feed,
                    None, None, "trips.txt", None, Some("wheelchair_accessible"),
                    Some(format!("%{:.0} bilgi yok", ratio * 100.0)),
                    Some("≤ %80".to_string()),
                    format!("Seferlerin %{:.0}'ında wheelchair_accessible bilgisi eksik veya bilinmiyor (0) — erişilebilirlik verisi yetersiz.",
                        ratio * 100.0),
                    "trips.txt'deki wheelchair_accessible alanını 1 (erişilebilir) veya 2 (erişilemez) olarak doldurun.",
                ));
            }
        }
    }
}

// ── WP-09d: Stop times derived kontrolleri ────────────────────────────────────

fn check_stoptimes_derived(
    idx: &StopTimesIndex<'_>,
    notices: &mut Vec<Notice>,
    ctr: &mut u32,
) {
    // STM_015 / STM_016: ilk/son durağın zorunlu zaman alanları
    for (&trip_id, stimes) in &idx.by_trip {
        if stimes.first().and_then(|s| s.departure_time).is_none() {
            notices.push(k6_notice(
                ctr, "STM_015", EntityType::Trip,
                Some(trip_id.to_string()), Some(trip_id.to_string()),
                "stop_times.txt", stimes.first().map(|s| s.line),
                Some("departure_time"), None, Some("HH:MM:SS".to_string()),
                format!("trip_id '{trip_id}' seferinin ilk durağında departure_time eksik."),
                "İlk stop_times satırına departure_time girin.",
            ));
        }
        if stimes.last().and_then(|s| s.arrival_time).is_none() {
            notices.push(k6_notice(
                ctr, "STM_016", EntityType::Trip,
                Some(trip_id.to_string()), Some(trip_id.to_string()),
                "stop_times.txt", stimes.last().map(|s| s.line),
                Some("arrival_time"), None, Some("HH:MM:SS".to_string()),
                format!("trip_id '{trip_id}' seferinin son durağında arrival_time eksik."),
                "Son stop_times satırına arrival_time girin.",
            ));
        }
    }

    // STM_027: shape_dist_traveled monoton artmıyor
    for (&trip_id, stimes) in &idx.by_trip {

        // STM_027
        let mut prev_dist: Option<f64> = None;
        for st in stimes.iter() {
            let Some(dist) = st.shape_dist_traveled else { continue };
            if let Some(prev) = prev_dist {
                if dist < prev - 1e-6 {
                    notices.push(k6_notice(
                        ctr,
                        "STM_027",
                        EntityType::Trip,
                        Some(trip_id.to_string()),
                        Some(trip_id.to_string()),
                        "stop_times.txt",
                        Some(st.line),
                        Some("shape_dist_traveled"),
                        Some(format!("{dist:.4}")),
                        Some(format!("≥ {prev:.4} (önceki değer)")),
                        format!("'{trip_id}' seferinde güzergah mesafesi (shape_dist_traveled) azalıyor: {prev:.4} → {dist:.4}."),
                        "shape_dist_traveled değerlerini dizi boyunca sıralı olarak düzeltin.",
                    ));
                    break; // trip başına bir kez
                }
            }
            prev_dist = Some(dist);
        }

        // STM_013: bazı stop_times'ta arrival/departure var, bazılarında yok (karışık)
        let has_time: Vec<bool> = stimes
            .iter()
            .map(|s| s.arrival_time.is_some() || s.departure_time.is_some())
            .collect();
        let n = has_time.len();
        if n >= 3 {
            // İlk ve son durak hariç ortadaki duraklarda eksik zaman → STM_013
            let missing_mid = has_time[1..n - 1].iter().any(|&v| !v);
            let has_any = has_time.iter().any(|&v| v);
            if has_any && missing_mid {
                notices.push(k6_notice(
                    ctr,
                    "STM_013",
                    EntityType::Trip,
                    Some(trip_id.to_string()),
                    Some(trip_id.to_string()),
                    "stop_times.txt",
                    stimes.first().map(|s| s.line),
                    Some("arrival_time|departure_time"),
                    None,
                    None,
                    format!("'{trip_id}' seferinde bazı ara duraklarda zaman bilgisi eksik — tutarsız zaman dizisi."),
                    "Tüm duraklara arrival/departure_time ekleyin ya da yalnızca ilk/son durak için gerektiğinde boş bırakın.",
                ));
            }
        }
    }
}

// ── WP-09d: Route + trip kalitesi ────────────────────────────────────────────

fn check_route_trip_quality(
    records: &EntityRecords,
    derived: &DerivedData,
    idx: &StopTimesIndex<'_>,
    notices: &mut Vec<Notice>,
    ctr: &mut u32,
) {
    use crate::timing::Timer;

    // route_id → trip listesi
    let _t0 = Timer::start("K6::rtq::build_trip_maps");
    let mut route_trips: HashMap<&str, Vec<&crate::k2::trips::TripRecord>> = HashMap::new();
    for t in &records.trips {
        if !t.route_id.is_empty() {
            route_trips.entry(t.route_id.as_str()).or_default().push(t);
        }
    }
    drop(_t0);

    // route_id → gösterim adı (route_short_name varsa, yoksa route_id)
    let route_short: HashMap<&str, &str> = records.routes.iter()
        .map(|r| {
            let label = r.route_short_name.as_deref().filter(|s| !s.is_empty()).unwrap_or(r.route_id.as_str());
            (r.route_id.as_str(), label)
        })
        .collect();

    // trip_id → ilk kalkış saati (HH:MM)
    let trip_first_dep: HashMap<&str, String> = idx.by_trip.iter()
        .filter_map(|(&tid, sts)| {
            sts.first().and_then(|s| s.departure_time).map(|d| {
                (tid, format!("{:02}:{:02}", d.0, d.1))
            })
        })
        .collect();

    // TRP_009: trip → çok az durak (< 2 faaliyetli zaman noktası)
    // Not: OPR_006 tek-duraklı tipleri zaten yakalıyor; TRP_009 "en az 1 zaman noktası yok" için
    let _t1 = Timer::start("K6::rtq::trp_009");
    for trip in &records.trips {
        let count = idx.by_trip.get(trip.trip_id.as_str()).map(|v| v.len()).unwrap_or(0);
        if count == 0 {
            continue; // XFL_002 K4'te yakaladı
        }
        let times_present = idx.trip_has_time.contains(trip.trip_id.as_str());
        if !times_present && count > 0 {
            notices.push(k6_notice(
                ctr,
                "TRP_009",
                EntityType::Trip,
                Some(trip.trip_id.clone()),
                Some(trip.trip_id.clone()),
                "stop_times.txt",
                None,
                None,
                Some("tüm zamanlar eksik".to_string()),
                None,
                {
                    let rname = route_short.get(trip.route_id.as_str()).copied().unwrap_or(trip.route_id.as_str());
                    let dep = trip_first_dep.get(trip.trip_id.as_str()).map(|s| format!(" {} kalkışlı", s)).unwrap_or_default();
                    format!("'{}' hattının{dep} seferinde stop_times kaydı var ama hiçbirinde zaman bilgisi girilmemiş.", rname)
                },
                "stop_times'a geçerli arrival/departure_time ekleyin.",
            ));
        }
    }
    drop(_t1);

    // TRP_013: route başına çok az trip (= 1) — frekans sorunu
    let _t2 = Timer::start("K6::rtq::trp_013");
    for (route_id, trips) in &route_trips {
        if trips.len() == 1 {
            notices.push(k6_notice(
                ctr,
                "TRP_013",
                EntityType::Route,
                Some(route_id.to_string()),
                Some(route_id.to_string()),
                "trips.txt",
                trips.first().map(|t| t.line),
                None,
                Some("1 sefer".to_string()),
                None,
                {
                    let rname = route_short.get(*route_id).copied().unwrap_or(route_id);
                    format!("'{rname}' hattında yalnızca 1 sefer var — düşük frekans sinyali.")
                },
                "Bu rotaya ek seferler ekleyin ya da frekans bazlı tanım kullanın.",
            ));
        }
    }
    drop(_t2);

    // RTS_016: route → hiçbir aktif servis günü yok (calendar_bitmap'te boş)
    let _t3 = Timer::start("K6::rtq::RTS_016");
    for (route_id, trips) in &route_trips {
        let has_active = trips.iter().any(|t| {
            derived
                .calendar_bitmap
                .active_dates
                .get(&t.service_id)
                .map(|s| !s.is_empty())
                .unwrap_or(false)
        });
        if !has_active {
            notices.push(k6_notice(
                ctr,
                "RTS_016",
                EntityType::Route,
                Some(route_id.to_string()),
                Some(route_id.to_string()),
                "routes.txt",
                None,
                None,
                Some("aktif gün yok".to_string()),
                None,
                {
                    let rname = route_short.get(*route_id).copied().unwrap_or(route_id);
                    format!("'{rname}' hattının hiçbir seferinde aktif takvim günü yok.")
                },
                "calendar.txt veya calendar_dates.txt'deki servis tanımlarını düzeltin.",
            ));
        }
    }
    drop(_t3);

    // TRP_011: trip_headsign boş veya yok (kısa ad da yoksa) — yolcu bilgisi eksik
    let _t4 = Timer::start("K6::rtq::trp_011");
    for trip in &records.trips {
        let headsign_missing = trip.trip_headsign.as_deref().map(str::is_empty).unwrap_or(true);
        let short_missing = trip.trip_short_name.as_deref().map(str::is_empty).unwrap_or(true);
        if headsign_missing && short_missing {
            notices.push(k6_notice(
                ctr,
                "TRP_011",
                EntityType::Trip,
                Some(trip.trip_id.clone()),
                Some(trip.trip_id.clone()),
                "trips.txt",
                Some(trip.line),
                Some("trip_headsign"),
                None,
                None,
                {
                    let rname = route_short.get(trip.route_id.as_str()).copied().unwrap_or(trip.route_id.as_str());
                    let dep = trip_first_dep.get(trip.trip_id.as_str()).map(|s| format!(" {} kalkışlı", s)).unwrap_or_default();
                    format!("'{}' hattının{dep} seferinde yön adı (trip_headsign) ve kısa ad girilmemiş — yolcu bilgisi yok.", rname)
                },
                "trip_headsign veya trip_short_name alanını doldurun.",
            ));
        }
    }
    drop(_t4);

    // TRP_020: trip_headsign terminal durak değil ara durak adıyla eşleşiyor
    // Per-matching-stop fire; circular trip'ler atlanır
    let _t5 = Timer::start("K6::rtq::trp_020");
    {
        // stop_id → stop_name (küçük harf, trim edilmiş)
        let stop_name_lc: HashMap<&str, String> = records.stops.iter()
            .filter_map(|s| s.stop_name.as_deref().map(|n| (s.stop_id.as_str(), n.trim().to_lowercase())))
            .collect();

        for trip in &records.trips {
            let headsign = match trip.trip_headsign.as_deref().map(str::trim).filter(|h| !h.is_empty()) {
                Some(h) => h,
                None => continue,
            };
            let headsign_lc = headsign.to_lowercase();

            let stimes = match idx.by_trip.get(trip.trip_id.as_str()) {
                Some(v) if v.len() >= 2 => v,
                _ => continue,
            };

            // Terminal durak: en büyük stop_sequence
            let terminal_stop_id = stimes.iter()
                .max_by_key(|s| s.stop_sequence.unwrap_or(0))
                .map(|s| s.stop_id.as_str())
                .unwrap_or("");

            // İlk durak: en küçük stop_sequence
            let first_stop_id = stimes.iter()
                .min_by_key(|s| s.stop_sequence.unwrap_or(u32::MAX))
                .map(|s| s.stop_id.as_str())
                .unwrap_or("");

            // Circular trip: ilk == son durak → atla
            if first_stop_id == terminal_stop_id { continue; }

            let rname = route_short.get(trip.route_id.as_str()).copied().unwrap_or(trip.route_id.as_str());
            let dep = trip_first_dep.get(trip.trip_id.as_str()).map(|s| format!(" {} kalkışlı", s)).unwrap_or_default();

            // Her matching intermediate stop için ayrı notice
            for st in stimes.iter().filter(|s| s.stop_id.as_str() != terminal_stop_id) {
                if let Some(stop_name) = stop_name_lc.get(st.stop_id.as_str()) {
                    if *stop_name == headsign_lc {
                        notices.push(k6_notice(
                            ctr,
                            "TRP_020",
                            EntityType::Trip,
                            Some(trip.trip_id.clone()),
                            Some(trip.trip_id.clone()),
                            "trips.txt",
                            Some(trip.line),
                            Some("trip_headsign"),
                            Some(headsign.to_string()),
                            None,
                            format!(
                                "'{}' hattının{dep} seferinde yön adı '{}' terminal durak değil, ara durak adıyla eşleşiyor.",
                                rname, headsign
                            ),
                            "trip_headsign'ı son durağın (terminal) adına veya o istikameti temsil eden bir yere adına göre ayarlayın.",
                        ));
                    }
                }
            }
        }
    }
    drop(_t5);

    // ── RTS_020: hat URL'si acente URL'siyle aynı (same_route_and_agency_url) ─
    {
        let _t6 = Timer::start("K6::rtq::rts_020");
        // agency_id → agency_url
        let agency_url_map: HashMap<&str, &str> = records.agencies.iter()
            .filter_map(|a| {
                let aid = a.agency_id.as_deref().unwrap_or("");
                if a.agency_url.is_empty() { None } else { Some((aid, a.agency_url.as_str())) }
            })
            .collect();
        // Tek acente varsa ve agency_id eksikse o acente URL'sini kullan
        let default_agency_url = if records.agencies.len() == 1 {
            Some(records.agencies[0].agency_url.as_str())
        } else {
            None
        };

        for route in &records.routes {
            if route.route_id.is_empty() { continue; }
            let route_url = match route.route_url.as_deref().filter(|u| !u.is_empty()) {
                Some(u) => u,
                None => continue,
            };
            let agency_url = route.agency_id.as_deref()
                .and_then(|aid| agency_url_map.get(aid).copied())
                .or(default_agency_url);
            if let Some(aurl) = agency_url {
                if route_url == aurl {
                    let rname = route_short.get(route.route_id.as_str()).copied().unwrap_or(route.route_id.as_str());
                    notices.push(k6_notice(
                        ctr, "RTS_020", EntityType::Route,
                        Some(route.route_id.clone()), Some(route.route_id.clone()),
                        "routes.txt", Some(route.line), Some("route_url"),
                        Some(route_url.to_string()), None,
                        format!("'{}' hattının route_url değeri acente URL'siyle aynı: '{route_url}'.", rname),
                        "route_url'yi bu hata özgü bir sayfaya yönlendirin ya da boş bırakın.",
                    ));
                }
            }
        }
    }

    // ── RTS_022: uzun hat adı kısa adı içeriyor (route_long_name_contains_short_name) ─
    {
        let _t7 = Timer::start("K6::rtq::rts_022");
        for route in &records.routes {
            if route.route_id.is_empty() { continue; }
            let short = match route.route_short_name.as_deref().filter(|s| !s.is_empty()) {
                Some(s) => s,
                None => continue,
            };
            let long = match route.route_long_name.as_deref().filter(|l| !l.is_empty()) {
                Some(l) => l,
                None => continue,
            };
            // Kısa ad en az 2 karakter olmalı ve uzun adda kelime sınırında yer almalı.
            // Örn: short="5A", long="5A Hattı" → ateşler; short="5", long="Route 5A" → ateşlemez.
            if short.len() >= 2 && contains_as_word(long, short) {
                notices.push(k6_notice(
                    ctr, "RTS_022", EntityType::Route,
                    Some(route.route_id.clone()), Some(route.route_id.clone()),
                    "routes.txt", Some(route.line), Some("route_long_name"),
                    Some(long.to_string()), None,
                    format!("'{}' hattının uzun adı '{}', kısa adı '{}' zaten içeriyor.", route.route_id, long, short),
                    "route_long_name'i kısa adı tekrar etmeyecek şekilde düzenleyin.",
                ));
            }
        }
    }

    // ── STP_034/035: stop_url acente veya hat URL'siyle aynı ─────────────────
    {
        let agency_urls: Vec<&str> = records.agencies.iter()
            .filter_map(|a| a.agency_url.as_str().is_empty().then_some(None).unwrap_or_else(|| Some(a.agency_url.as_str())))
            .collect();
        let route_urls: Vec<&str> = records.routes.iter()
            .filter_map(|r| r.route_url.as_deref().filter(|u| !u.is_empty()))
            .collect();

        for stop in &records.stops {
            let Some(ref surl) = stop.stop_url else { continue };
            if surl.is_empty() { continue }

            // STP_034: stop_url == agency_url
            if agency_urls.iter().any(|&au| au == surl.as_str()) {
                notices.push(k6_notice(
                    ctr, "STP_034", EntityType::Stop,
                    Some(stop.stop_id.clone()), Some(stop.stop_id.clone()),
                    "stops.txt", Some(stop.line), Some("stop_url"),
                    Some(surl.clone()), None,
                    format!("'{}' durağının stop_url değeri bir acente URL'siyle aynı: '{surl}'.", stop.stop_id),
                    "stop_url'yi bu durağa özgü bir sayfaya yönlendirin ya da boş bırakın.",
                ));
            }

            // STP_035: stop_url == route_url
            if route_urls.iter().any(|&ru| ru == surl.as_str()) {
                notices.push(k6_notice(
                    ctr, "STP_035", EntityType::Stop,
                    Some(stop.stop_id.clone()), Some(stop.stop_id.clone()),
                    "stops.txt", Some(stop.line), Some("stop_url"),
                    Some(surl.clone()), None,
                    format!("'{}' durağının stop_url değeri bir hat URL'siyle aynı: '{surl}'.", stop.stop_id),
                    "stop_url'yi bu durağa özgü bir sayfaya yönlendirin ya ya boş bırakın.",
                ));
            }
        }
    }


    // ── PDW_006: aynı trip+zone'da örtüşen pickup/drop-off penceresi ──────────
    {
        // (trip_id, zone_key) → [(start_secs, end_secs, line)]
        let mut zone_wins: HashMap<(&str, &str), Vec<(u64, u64, u64)>> = HashMap::new();
        for (trip_id, stops) in records.stop_times_index.iter_trips() {
            for st in stops {
                let Some(flex) = st.flex.as_deref() else { continue };
                let Some(start) = flex.start_pickup_drop_off_window else { continue };
                let Some(end)   = flex.end_pickup_drop_off_window   else { continue };
                let zone = if let Some(ref z) = flex.location_id        { z.as_str() }
                           else if let Some(ref z) = flex.location_group_id { z.as_str() }
                           else { continue };
                let s = start.0 as u64 * 3600 + start.1 as u64 * 60 + start.2 as u64;
                let e = end.0   as u64 * 3600 + end.1   as u64 * 60 + end.2   as u64;
                zone_wins.entry((trip_id.as_str(), zone)).or_default().push((s, e, st.line));
            }
        }

        for ((trip_id, zone), mut wins) in zone_wins {
            if wins.len() < 2 { continue; }
            wins.sort_by_key(|&(s, _, _)| s);
            for i in 1..wins.len() {
                let (_ps, pe, _) = wins[i - 1];
                let (cs, _ce, cl) = wins[i];
                if cs < pe {
                    notices.push(k6_notice(
                        ctr, "PDW_006", EntityType::Trip,
                        Some(trip_id.to_string()), Some(trip_id.to_string()),
                        "stop_times.txt", Some(cl), Some("start_pickup_drop_off_window"),
                        None, None,
                        format!("trip_id '{}' için zone '{}' içinde örtüşen pickup/drop-off pencereleri var.", trip_id, zone),
                        "Aynı trip+zone içindeki zaman pencerelerinin örtüşmediğinden emin olun.",
                    ));
                    break;
                }
            }
        }
    }
}

// ── WP-09d: Veri kalitesi özet kuralları ──────────────────────────────────────

fn check_data_quality(
    records: &EntityRecords,
    derived: &DerivedData,
    today_yyyymmdd: u32,
    notices: &mut Vec<Notice>,
    ctr: &mut u32,
) {
    // DQ_005: feed'de hiç aktif sefer yok (calendar_bitmap tamamen boş veya tüm servisler expired)
    let has_any_active = if today_yyyymmdd > 0 {
        derived.calendar_bitmap.active_dates.values().any(|dates| {
            dates.iter().any(|&d| d >= today_yyyymmdd)
        })
    } else {
        !derived.calendar_bitmap.active_dates.is_empty()
    };

    if !records.trips.is_empty() && !has_any_active {
        notices.push(k6_notice(
            ctr,
            "DQ_005",
            EntityType::Feed,
            None,
            None,
            "calendar.txt",
            None,
            None,
            Some("aktif sefer yok".to_string()),
            None,
            "Feed'de bugün itibarıyla aktif seferi olan servis bulunamadı.".to_string(),
            "calendar.txt veya calendar_dates.txt verilerini güncelleyin.",
        ));
    }

    // DQ_006: şekil (shape) olmayan trip oranı çok yüksek (> %80)
    if !records.trips.is_empty() {
        let shapeless = records.trips.iter().filter(|t| t.shape_id.is_none()).count();
        let ratio = shapeless as f64 / records.trips.len() as f64;
        if ratio > 0.8 {
            notices.push(k6_notice(
                ctr,
                "DQ_006",
                EntityType::Feed,
                None,
                None,
                "trips.txt",
                None,
                None,
                Some(format!("%{:.0} şekil eksik", ratio * 100.0)),
                Some("≤ %80".to_string()),
                format!("Seferlerin %{:.0}'ında shape_id eksik — harita gösterimi ve coğrafi analiz kısıtlı.",
                    ratio * 100.0),
                "shapes.txt dosyasını oluşturun ve trips.txt'deki shape_id alanlarını doldurun.",
            ));
        }
    }

    // DQ_009: feed'de hiç stop_times yok
    if records.stop_times_index.total_rows == 0 && !records.trips.is_empty() {
        notices.push(k6_notice(
            ctr,
            "DQ_009",
            EntityType::Feed,
            None,
            None,
            "stop_times.txt",
            None,
            None,
            Some("0 stop_times kaydı".to_string()),
            None,
            "Feed'de sefer var ancak hiç stop_times kaydı yok.".to_string(),
            "stop_times.txt dosyasını oluşturun ve doldurun.",
        ));
    }

    // DQ_011: feed'de çok az durak (< 2) — işlevsel transit veri değil
    if records.stops.len() == 1 {
        notices.push(k6_notice(
            ctr,
            "DQ_011",
            EntityType::Feed,
            None,
            None,
            "stops.txt",
            None,
            None,
            Some("1 durak".to_string()),
            Some("≥ 2 durak".to_string()),
            "Feed'de yalnızca 1 durak var — işlevsel transit verisi oluşturulamaz.".to_string(),
            "stops.txt'e en az 2 durak ekleyin.",
        ));
    }

    // DQ_012: feed'deki agency sayısı çok fazla ve agency_id kullanılmıyor
    if records.agencies.len() > 5 {
        let routes_with_agency = records.routes.iter().filter(|r| r.agency_id.is_some()).count();
        if routes_with_agency == 0 {
            notices.push(k6_notice(
                ctr,
                "DQ_012",
                EntityType::Feed,
                None,
                None,
                "agency.txt",
                None,
                None,
                Some(format!("{} işletici, 0 rotada agency_id", records.agencies.len())),
                None,
                format!("Feed'de {} işletici var ancak hiçbir rotada agency_id atanmamış.", records.agencies.len()),
                "routes.txt'deki agency_id sütununu doldurun.",
            ));
        }
    }

    // DQ_013: feed'de çok az sefer (< 3)
    {
        let trip_count = records.trips.len();
        if trip_count > 0 && trip_count < 3 {
            notices.push(k6_notice(
                ctr, "DQ_013", EntityType::Feed,
                None, None, "trips.txt", None, None,
                Some(format!("{trip_count} sefer")), Some("≥ 3 sefer".to_string()),
                format!("Feed'de yalnızca {trip_count} sefer var — işlevsel transit veri için en az 3 sefer önerilir."),
                "trips.txt'e daha fazla sefer ekleyin.",
            ));
        }
    }

    // DQ_001/002: feed_info eksiklikleri — yalnızca feed gerçek içerik barındırıyorsa kontrol et
    let feed_has_content = !records.agencies.is_empty()
        || !records.routes.is_empty()
        || !records.stops.is_empty()
        || !records.trips.is_empty();

    if feed_has_content {
        // DQ_001: feed_publisher_name eksik
        let has_publisher = records.feed_info.first()
            .and_then(|fi| fi.row.get("feed_publisher_name"))
            .map(|v| !v.trim().is_empty())
            .unwrap_or(false);
        if !has_publisher {
            notices.push(k6_notice(
                ctr, "DQ_001", EntityType::Feed,
                None, None, "feed_info.txt", None, Some("feed_publisher_name"),
                None, None,
                "feed_publisher_name belirtilmemiş — feed kaynağı tanımlanamıyor.".to_string(),
                "feed_info.txt dosyasına feed_publisher_name ekleyin.",
            ));
        }

        // DQ_002: feed_publisher_url eksik
        let has_url = records.feed_info.first()
            .and_then(|fi| fi.row.get("feed_publisher_url"))
            .map(|v| !v.trim().is_empty())
            .unwrap_or(false);
        if !has_url {
            notices.push(k6_notice(
                ctr, "DQ_002", EntityType::Feed,
                None, None, "feed_info.txt", None, Some("feed_publisher_url"),
                None, None,
                "feed_publisher_url belirtilmemiş.".to_string(),
                "feed_info.txt dosyasına feed_publisher_url ekleyin.",
            ));
        }
    }

    // DQ_003: hat açıklaması (route_desc) boş — hat başına bir notice
    for route in &records.routes {
        if route.route_id.is_empty() { continue; }
        if route.row.get("route_desc").map(|v| v.trim().is_empty()).unwrap_or(true) {
            let label = route.route_short_name.as_deref()
                .filter(|s| !s.is_empty())
                .or(route.route_long_name.as_deref().filter(|s| !s.is_empty()))
                .unwrap_or(&route.route_id);
            notices.push(k6_notice(
                ctr, "DQ_003", EntityType::Route,
                Some(route.route_id.clone()), Some(route.route_id.clone()),
                "routes.txt", Some(route.line), Some("route_desc"),
                Some(route.route_id.clone()), None,
                format!("'{}' ({}) hattında route_desc alanı boş; kullanıcılar hat hakkında ek bilgi alamıyor.", route.route_id, label),
                "routes.txt'e route_desc açıklaması ekleyin.",
            ));
        }
    }

    // DQ_004: hat URL'si (route_url) eksik — hat başına bir notice
    for route in &records.routes {
        if route.route_id.is_empty() { continue; }
        if route.row.get("route_url").map(|v| v.trim().is_empty()).unwrap_or(true) {
            let label = route.route_short_name.as_deref()
                .filter(|s| !s.is_empty())
                .or(route.route_long_name.as_deref().filter(|s| !s.is_empty()))
                .unwrap_or(&route.route_id);
            notices.push(k6_notice(
                ctr, "DQ_004", EntityType::Route,
                Some(route.route_id.clone()), Some(route.route_id.clone()),
                "routes.txt", Some(route.line), Some("route_url"),
                Some(route.route_id.clone()), None,
                format!("'{}' ({}) hattında route_url alanı boş; yolcular hat web sayfasına yönlendirilemiyor.", route.route_id, label),
                "routes.txt'e route_url bağlantısı ekleyin.",
            ));
        }
    }

    // DQ_017: şüpheli koordinat (0.0, 0.0) veya okyanus ortası gibi değerler
    {
        let suspicious = records.stops.iter()
            .filter(|s| !s.stop_id.is_empty())
            .filter(|s| {
                matches!((s.stop_lat, s.stop_lon), (Some(lat), Some(lon))
                    if lat.abs() < 1.0 && lon.abs() < 1.0)
            })
            .count();
        if suspicious > 0 {
            notices.push(k6_notice(
                ctr, "DQ_017", EntityType::Feed,
                None, None, "stops.txt", None, Some("stop_lat/stop_lon"),
                Some(format!("{suspicious} durak")), None,
                format!("{suspicious} durağın koordinatı (0°, 0°) civarında — bu Afrika kıyısı açıklarıdır; muhtemelen yanlış değer."),
                "Durak koordinatlarını gerçek konuma göre düzeltin.",
            ));
        }
    }

    // FIN_010: feed geçerlilik süresi dolmuş
    if today_yyyymmdd > 0 {
        if let Some(fi) = records.feed_info.first() {
            if let Some((y, m, d)) = fi.feed_end_date {
                let end = y * 10000 + m * 100 + d;
                if end < today_yyyymmdd {
                    notices.push(k6_notice(
                        ctr, "FIN_010", EntityType::Feed,
                        None, None, "feed_info.txt", None, Some("feed_end_date"),
                        Some(format!("{y}-{m:02}-{d:02}")),
                        Some("≥ bugün".to_string()),
                        format!("Feed'in geçerlilik süresi {y}-{m:02}-{d:02} tarihinde dolmuş; mevcut veriler güncel değil."),
                        "feed_info.txt'deki feed_end_date'i güncel tutun ve yeni bir feed yayınlayın.",
                    ));
                }
            }

            // FIN_016: feed_start_date gelecekte — feed henüz aktif değil
            if let Some((sy, sm, sd)) = fi.feed_start_date {
                let start = sy * 10000 + sm * 100 + sd;
                if start > today_yyyymmdd {
                    notices.push(k6_notice(
                        ctr, "FIN_016", EntityType::Feed,
                        None, None, "feed_info.txt", None, Some("feed_start_date"),
                        Some(format!("{sy}-{sm:02}-{sd:02}")),
                        Some("≤ bugün".to_string()),
                        format!("feed_start_date ({sy}-{sm:02}-{sd:02}) henüz gelmedi — feed bugün için aktif değil."),
                        "feed_start_date'i geçmiş veya bugüne ayarlayın ya da feed'i doğru zamanda yayınlayın.",
                    ));
                }
            }

            // FIN_017: feed_end_date çok uzak gelecekte (> 2 yıl)
            if let Some((ey, em, ed)) = fi.feed_end_date {
                let end = ey * 10000 + em * 100 + ed;
                let today_jdn = yyyymmdd_to_approx_jdn(today_yyyymmdd);
                let end_jdn   = yyyymmdd_to_approx_jdn(end);
                if end_jdn > today_jdn + 730 {
                    notices.push(k6_notice(
                        ctr, "FIN_017", EntityType::Feed,
                        None, None, "feed_info.txt", None, Some("feed_end_date"),
                        Some(format!("{ey}-{em:02}-{ed:02}")),
                        Some("≤ +2 yıl".to_string()),
                        format!("feed_end_date ({ey}-{em:02}-{ed:02}) bugünden 2 yıldan fazla ileride — muhtemelen yanlış değer."),
                        "feed_end_date'i gerçekçi bir bitiş tarihiyle güncelleyin.",
                    ));
                }
            }

            // FIN_018: feed_contact_email ve feed_contact_url ikisi de eksik
            // (missing_feed_contact_email_and_url)
            let has_contact_email = fi.feed_contact_email.as_deref()
                .map(|e| !e.trim().is_empty()).unwrap_or(false);
            let has_contact_url = fi.feed_contact_url.as_deref()
                .map(|u| !u.trim().is_empty()).unwrap_or(false);
            if !has_contact_email && !has_contact_url {
                notices.push(k6_notice(
                    ctr, "FIN_018", EntityType::Feed,
                    None, None, "feed_info.txt", None, Some("feed_contact_email"),
                    None, None,
                    "feed_contact_email ve feed_contact_url alanlarının ikisi de eksik — kullanıcılar feed sorunlarını nereden bildireceğini bilemiyor.".to_string(),
                    "feed_info.txt'e feed_contact_email veya feed_contact_url ekleyin.",
                ));
            }

            // FIN_019: feed 7 gün içinde sona erecek (feed_expiration_date7_days)
            if today_yyyymmdd > 0 {
                if let Some((ey, em, ed)) = fi.feed_end_date {
                    let end = ey * 10000 + em * 100 + ed;
                    if end >= today_yyyymmdd {
                        let today_jdn = yyyymmdd_to_approx_jdn(today_yyyymmdd);
                        let end_jdn   = yyyymmdd_to_approx_jdn(end);
                        let days_left = end_jdn.saturating_sub(today_jdn);
                        if days_left <= 7 && days_left > 0 {
                            notices.push(k6_notice(
                                ctr, "FIN_019", EntityType::Feed,
                                None, None, "feed_info.txt", None, Some("feed_end_date"),
                                Some(format!("{ey}-{em:02}-{ed:02}")),
                                Some("> +7 gün".to_string()),
                                format!("Feed'in geçerlilik süresi {ey}-{em:02}-{ed:02} tarihinde doluyor — {days_left} gün kaldı."),
                                "Yeni bir feed versiyonu yayınlamaya hazırlanın.",
                            ));
                        }
                    }
                }
            }
        }
    }

    // FIN_020: Feed geçerlilik penceresi < 7 gün
    if let Some(fi) = records.feed_info.first() {
        if let (Some((sy, sm, sd)), Some((ey, em, ed))) = (fi.feed_start_date, fi.feed_end_date) {
            let start_jdn = yyyymmdd_to_approx_jdn(sy * 10000 + sm * 100 + sd);
            let end_jdn   = yyyymmdd_to_approx_jdn(ey * 10000 + em * 100 + ed);
            let span_days = end_jdn.saturating_sub(start_jdn);
            if span_days < 7 {
                notices.push(k6_notice(
                    ctr, "FIN_020", EntityType::Feed,
                    None, None, "feed_info.txt", None, None,
                    Some(format!("{span_days} gün")), Some("≥7 gün".to_string()),
                    format!("Feed geçerlilik penceresi yalnızca {span_days} gün ({sy}-{sm:02}-{sd:02} → {ey}-{em:02}-{ed:02}) — operasyonel kullanım için çok kısa."),
                    "feed_start_date ve feed_end_date değerlerini gerçek hizmet dönemiyle güncelleyin.",
                ));
            }
        }
    }

    // CAL_020: Feed geçerlilik penceresi > 5 yıl (yaklaşık 1825 gün)
    if let Some(fi) = records.feed_info.first() {
        if let (Some((sy, sm, sd)), Some((ey, em, ed))) = (fi.feed_start_date, fi.feed_end_date) {
            let start_jdn = yyyymmdd_to_approx_jdn(sy * 10000 + sm * 100 + sd);
            let end_jdn   = yyyymmdd_to_approx_jdn(ey * 10000 + em * 100 + ed);
            let span_days = end_jdn.saturating_sub(start_jdn);
            if span_days > 1825 {
                notices.push(k6_notice(
                    ctr, "CAL_020", EntityType::Feed,
                    None, None, "feed_info.txt", None, None,
                    Some(format!("{} yıl", span_days / 365)), Some("≤5 yıl".to_string()),
                    format!("Feed geçerlilik penceresi {} gün (~{} yıl) — gerçekçi olmayan zaman dilimi.",
                        span_days, span_days / 365),
                    "feed_start_date ve feed_end_date değerlerini gerçekçi hizmet dönemine göre düzenleyin.",
                ));
            }
        }
    }

    // DQ_022: Durakların >%80'i aynı stop_name değerini paylaşıyor (yer tutucu/test verisi)
    {
        let total_named = records.stops.iter().filter(|s| s.stop_name.is_some()).count();
        if total_named >= 5 {
            let mut name_counts: HashMap<&str, u32> = HashMap::new();
            for s in &records.stops {
                if let Some(n) = s.stop_name.as_deref() {
                    *name_counts.entry(n).or_default() += 1;
                }
            }
            if let Some((&most_common_name, &most_count)) = name_counts.iter().max_by_key(|(_, &c)| c) {
                if most_count as f64 / total_named as f64 > 0.8 {
                    notices.push(k6_notice(
                        ctr, "DQ_022", EntityType::Feed,
                        None, None, "stops.txt", None, Some("stop_name"),
                        Some(format!("{:.0}%", most_count as f64 / total_named as f64 * 100.0)),
                        Some("≤80%".to_string()),
                        format!("Durakların {most_count}/{total_named} tanesinin adı '{most_common_name}' — yer tutucu veya test verisi olabilir."),
                        "Her durağa gerçek konumunu yansıtan benzersiz bir stop_name verin.",
                    ));
                }
            }
        }
    }

    // DQ_021: birincil anahtar yineleniyor (duplicate_key)
    // stop_id, route_id, trip_id, service_id tekrarını kontrol et
    {
        fn find_dups<'a, I: Iterator<Item=&'a str>>(ids: I) -> Vec<String> {
            let mut seen: HashMap<&str, u32> = HashMap::new();
            let mut dups: Vec<String> = Vec::new();
            for id in ids {
                let e = seen.entry(id).or_default();
                *e += 1;
                if *e == 2 { dups.push(id.to_string()); }
            }
            dups
        }

        for dup_id in find_dups(records.stops.iter().filter(|s| !s.stop_id.is_empty()).map(|s| s.stop_id.as_str())) {
            notices.push(k6_notice(ctr, "DQ_021", EntityType::Stop,
                Some(dup_id.clone()), Some(dup_id.clone()),
                "stops.txt", None, Some("stop_id"), Some(dup_id.clone()), None,
                format!("stop_id '{dup_id}' stops.txt'de birden fazla kez tanımlanmış."),
                "stops.txt'de benzersiz stop_id değerleri kullanın."));
        }
        for dup_id in find_dups(records.routes.iter().filter(|r| !r.route_id.is_empty()).map(|r| r.route_id.as_str())) {
            notices.push(k6_notice(ctr, "DQ_021", EntityType::Route,
                Some(dup_id.clone()), Some(dup_id.clone()),
                "routes.txt", None, Some("route_id"), Some(dup_id.clone()), None,
                format!("route_id '{dup_id}' routes.txt'de birden fazla kez tanımlanmış."),
                "routes.txt'de benzersiz route_id değerleri kullanın."));
        }
        for dup_id in find_dups(records.trips.iter().filter(|t| !t.trip_id.is_empty()).map(|t| t.trip_id.as_str())) {
            notices.push(k6_notice(ctr, "DQ_021", EntityType::Trip,
                Some(dup_id.clone()), Some(dup_id.clone()),
                "trips.txt", None, Some("trip_id"), Some(dup_id.clone()), None,
                format!("trip_id '{dup_id}' trips.txt'de birden fazla kez tanımlanmış."),
                "trips.txt'de benzersiz trip_id değerleri kullanın."));
        }
    }

    // ARC_020: önerilen dosyalar eksik (missing_recommended_file)
    // GTFS en iyi uygulamalarına göre shapes.txt ve feed_info.txt önerilir.
    if feed_has_content {
        let has_shapes = !records.shapes.is_empty();
        let has_feed_info = !records.feed_info.is_empty();
        if !has_shapes && !has_feed_info {
            notices.push(k6_notice(
                ctr, "ARC_020", EntityType::Feed,
                None, None, "shapes.txt/feed_info.txt", None, None,
                Some("shapes.txt, feed_info.txt eksik".to_string()), None,
                "Feed'de shapes.txt ve feed_info.txt dosyaları yok — her ikisi de önerilir.".to_string(),
                "shapes.txt ile güzergah geometrisi ve feed_info.txt ile yayıncı bilgisi ekleyin.",
            ));
        } else if !has_shapes {
            notices.push(k6_notice(
                ctr, "ARC_020", EntityType::Feed,
                None, None, "shapes.txt", None, None,
                Some("shapes.txt eksik".to_string()), None,
                "Feed'de shapes.txt dosyası yok — güzergah geometrisi için önerilir.".to_string(),
                "shapes.txt dosyası oluşturarak güzergah geometrisi ekleyin.",
            ));
        } else if !has_feed_info {
            notices.push(k6_notice(
                ctr, "ARC_020", EntityType::Feed,
                None, None, "feed_info.txt", None, None,
                Some("feed_info.txt eksik".to_string()), None,
                "Feed'de feed_info.txt dosyası yok — yayıncı ve geçerlilik bilgisi için önerilir.".to_string(),
                "feed_info.txt dosyası oluşturarak yayıncı bilgisini tanımlayın.",
            ));
        }
    }
}

// ── Eksik K6 kurallar (STM_017, GEO_007/009/012/013, SHP_013, DQ_005b/005c/010,
//    RTS_017, TRP_012/015, OPR_005/013) ──────────────────────────────────────

fn check_remaining_analytics(
    records: &EntityRecords,
    derived: &DerivedData,
    config: &ValidatorConfig,
    idx: &StopTimesIndex<'_>,
    notices: &mut Vec<Notice>,
    ctr: &mut u32,
) {
    use crate::timing::Timer;

    // ── shape_id → sıralı (lat, lon) noktaları + bbox (tek pass) ────────────
    let _tb = Timer::start("K6::rem::build_maps");
    let mut shape_pts_unsorted: FxHashMap<&str, Vec<(u32, f64, f64)>> = FxHashMap::default();
    for sp in &records.shapes {
        if let (Some(lat), Some(lon)) = (sp.shape_pt_lat, sp.shape_pt_lon) {
            shape_pts_unsorted
                .entry(sp.shape_id.as_str())
                .or_default()
                .push((sp.shape_pt_sequence.unwrap_or(0), lat, lon));
        }
    }
    let n_shapes = shape_pts_unsorted.len();
    let mut shape_coords: FxHashMap<&str, Vec<(f64, f64)>> = FxHashMap::default();
    let mut shape_bbox: FxHashMap<&str, [f64; 4]> = FxHashMap::default();
    shape_coords.reserve(n_shapes);
    shape_bbox.reserve(n_shapes);
    for (sid, mut v) in shape_pts_unsorted {
        v.sort_by_key(|&(seq, _, _)| seq);
        let pts: Vec<(f64, f64)> = v.into_iter().map(|(_, la, lo)| (la, lo)).collect();
        if !pts.is_empty() {
            let mut mn_lat = pts[0].0;
            let mut mx_lat = pts[0].0;
            let mut mn_lon = pts[0].1;
            let mut mx_lon = pts[0].1;
            for &(la, lo) in pts.iter().skip(1) {
                if la < mn_lat { mn_lat = la; }
                if la > mx_lat { mx_lat = la; }
                if lo < mn_lon { mn_lon = lo; }
                if lo > mx_lon { mx_lon = lo; }
            }
            shape_bbox.insert(sid, [mn_lat, mx_lat, mn_lon, mx_lon]);
        }
        shape_coords.insert(sid, pts);
    }

    // ── stop_id → (lat, lon) ──────────────────────────────────────────────────
    let stop_coords: FxHashMap<&str, (f64, f64)> = records
        .stops
        .iter()
        .filter_map(|s| s.stop_lat.zip(s.stop_lon).map(|c| (s.stop_id.as_str(), c)))
        .collect();
    let stop_lines: FxHashMap<&str, u64> = records
        .stops
        .iter()
        .map(|s| (s.stop_id.as_str(), s.line))
        .collect();
    let stop_names: FxHashMap<&str, &str> = records
        .stops
        .iter()
        .filter_map(|s| s.stop_name.as_deref().map(|n| (s.stop_id.as_str(), n)))
        .collect();
    let trip_to_route_rem: FxHashMap<&str, &str> = records.trips.iter()
        .map(|t| (t.trip_id.as_str(), t.route_id.as_str()))
        .collect();
    let route_type_rem: FxHashMap<&str, u32> = records.routes.iter()
        .filter_map(|r| r.route_type.map(|rt| (r.route_id.as_str(), rt)))
        .collect();
    let trip_first_dep: FxHashMap<&str, (u32, u32, u32)> = idx.by_trip.iter()
        .filter_map(|(&tid, sts)| sts.first().and_then(|s| s.departure_time).map(|d| (tid, d)))
        .collect();
    // route_id → gösterim etiketi (route_short_name varsa o, yoksa route_id)
    let route_short: FxHashMap<&str, &str> = records.routes.iter()
        .map(|r| {
            let label = r.route_short_name.as_deref()
                .filter(|s| !s.is_empty())
                .unwrap_or(r.route_id.as_str());
            (r.route_id.as_str(), label)
        })
        .collect();

    // shape_id → direction_id (benzersizse Some(dir), belirsizse None)
    // shape_id → hat etiketi   (benzersiz route_id'ye karşılık short_name, belirsizse None)
    let (shape_directions, shape_to_route): (FxHashMap<&str, Option<u32>>, FxHashMap<&str, Option<&str>>) = {
        let mut dirs: FxHashMap<&str, Option<u32>> = FxHashMap::default();
        let mut route_ids: FxHashMap<&str, Option<&str>> = FxHashMap::default();
        for t in &records.trips {
            if let Some(shape) = t.shape_id.as_deref().filter(|s| !s.is_empty()) {
                let de = dirs.entry(shape).or_insert(t.direction_id);
                if *de != t.direction_id { *de = None; }
                let re = route_ids.entry(shape).or_insert(Some(t.route_id.as_str()));
                if *re != Some(t.route_id.as_str()) { *re = None; }
            }
        }
        let shape_to_route = route_ids.into_iter()
            .map(|(shape, rid)| {
                let label = rid.map(|r| route_short.get(r).copied().unwrap_or(r));
                (shape, label)
            })
            .collect();
        (dirs, shape_to_route)
    };
    drop(_tb);

    // ── SHP_011: ardışık shape noktaları arası büyük atlama ───────────────────
    {
        let _t11 = Timer::start("K6::rem::shp_011");
        let threshold_km = config.max_shape_jump_km;
        let mut shp011_fired: FxHashSet<&str> = FxHashSet::default();
        for (shape_id, seg) in &derived.shape_geometry.shapes {
            if shp011_fired.contains(shape_id.as_str()) { continue; }
            for &d in &seg.segment_distances_km {
                if d > threshold_km {
                    shp011_fired.insert(shape_id.as_str());
                    notices.push(k6_notice(
                        ctr,
                        "SHP_011",
                        EntityType::Shape,
                        Some(shape_id.clone()),
                        Some(shape_id.clone()),
                        "shapes.txt",
                        None,
                        None,
                        Some(format!("{d:.2}km")),
                        Some(format!("≤ {threshold_km:.1}km")),
                        format!(
                            "'{}' güzergah şeklinde ardışık iki nokta arası {d:.2} km — eşik {threshold_km:.1} km aşıldı.",
                            shape_id
                        ),
                        "Güzergah şekline ara noktalar ekleyerek büyük atlama noktasını kapatın.",
                    ));
                    break;
                }
            }
        }
    }

    // ── SHP_023: aynı dist_traveled ve koordinatlara sahip ardışık iki shape noktası ─
    // (equal_shape_distance_same_coordinates)
    {
        let _ts23 = Timer::start("K6::rem::shp_023");
        let mut shape_raw: FxHashMap<&str, Vec<(u32, Option<f64>, f64, f64)>> = FxHashMap::default();
        for sp in &records.shapes {
            if let (Some(lat), Some(lon)) = (sp.shape_pt_lat, sp.shape_pt_lon) {
                shape_raw.entry(sp.shape_id.as_str()).or_default()
                    .push((sp.shape_pt_sequence.unwrap_or(0), sp.shape_dist_traveled, lat, lon));
            }
        }
        for (_shape_id, pts) in &mut shape_raw {
            pts.sort_by_key(|&(seq, _, _, _)| seq);
        }
        let mut shp023_fired: FxHashSet<&str> = FxHashSet::default();
        for (shape_id, pts) in &shape_raw {
            if shp023_fired.contains(*shape_id) { continue; }
            for w in pts.windows(2) {
                let (_, da, la, loa) = w[0];
                let (_, db, lb, lob) = w[1];
                if let (Some(da_v), Some(db_v)) = (da, db) {
                    const EPS: f64 = 1e-9;
                    if (da_v - db_v).abs() < EPS && (la - lb).abs() < EPS && (loa - lob).abs() < EPS {
                        shp023_fired.insert(shape_id);
                        notices.push(k6_notice(
                            ctr, "SHP_023", EntityType::Shape,
                            Some(shape_id.to_string()), Some(shape_id.to_string()),
                            "shapes.txt", None, Some("shape_dist_traveled"),
                            Some(format!("dist={da_v:.4}, ({la:.6},{loa:.6})")), None,
                            format!("'{shape_id}' şeklinde art arda iki noktanın shape_dist_traveled ({da_v:.4}) ve koordinatları aynı — tekrar eden nokta."),
                            "Yinelenen shape noktasını kaldırın.",
                        ));
                        break;
                    }
                }
            }
        }
    }

    // ── STM_017: shape olan trip'te stop_times'da shape_dist_traveled eksik ──
    // idx.trips_missing_sdt: build'de trip_shape filtreli, line numarası hazır
    {
        let _ts = Timer::start("K6::rem::stm_017");
        for (&trip_id, &line) in &idx.trips_missing_sdt {
            let route_id = trip_to_route_rem.get(trip_id).copied().unwrap_or(trip_id);
            let rt = route_type_rem.get(route_id).copied().unwrap_or(3);
            if is_rail_route_type(rt) {
                continue; // intercity rail shape_dist_traveled sağlamaz — beklenen davranış
            }
            let route = route_id;
            let dep = trip_first_dep.get(trip_id)
                .map(|(h, m, _)| format!("{h:02}:{m:02}"))
                .unwrap_or_default();
            let dep_infix = if dep.is_empty() { String::new() } else { format!(" {dep}") };
            notices.push(k6_notice(
                ctr,
                "STM_017",
                EntityType::Trip,
                Some(trip_id.to_string()),
                Some(trip_id.to_string()),
                "stop_times.txt",
                Some(line),
                Some("shape_dist_traveled"),
                Some("eksik".to_string()),
                None,
                format!("'{}' hattının{dep_infix} seferinde güzergah mesafe bilgisi (shape_dist_traveled) eksik — durak konumları doğrulanamıyor.", route),
                "Tüm stop_times satırlarına shape_dist_traveled ekleyin ya da tümünden kaldırın.",
            ));
        }
    }

    // ── GEO_007: çok büyük shape atlaması (severe jump = 3× eşik) ─────────────
    {
        let _tg7 = Timer::start("K6::rem::geo_007");
        let severe_km = config.max_shape_jump_km * 3.0;
        for (shape_id, seg) in &derived.shape_geometry.shapes {
            for (i, &dist) in seg.segment_distances_km.iter().enumerate() {
                if dist > severe_km {
                    notices.push(k6_notice(
                        ctr,
                        "GEO_007",
                        EntityType::Shape,
                        Some(shape_id.clone()),
                        Some(shape_id.clone()),
                        "shapes.txt",
                        None,
                        Some("shape_pt_lat|shape_pt_lon"),
                        Some(format!("{dist:.2} km (segment {i}→{})", i + 1)),
                        Some(format!("≤ {severe_km:.0} km")),
                        format!("'{shape_id}' güzergahının {i}→{}. segmentinde {dist:.2}km kritik atlama (3× eşik).", i + 1),
                        "Eksik güzergah noktaları ekleyin veya koordinatları doğrulayın.",
                    ));
                }
            }
        }
    }

    // ── GEO_009 / SHP_013: durak shape'ten çok uzak ──────────────────────────
    // Option-1: aynı hattın başka shape'i durağı kapsıyorsa (farklı güzergah varyantı)
    // false-positive üretmemek için o durak atlanır.
    // Sefer saati: notice mesajına örnek kalkış saati eklenir.

    // shape_id → route_id kümesi (Option-1 karşılaştırması için)
    let shape_route_set: FxHashMap<&str, FxHashSet<&str>> = {
        let mut m: FxHashMap<&str, FxHashSet<&str>> = FxHashMap::default();
        for t in &records.trips {
            if let Some(sid) = t.shape_id.as_deref().filter(|s| !s.is_empty()) {
                m.entry(sid).or_default().insert(t.route_id.as_str());
            }
        }
        m
    };
    // shape_id → [trip_id] (örnek kalkış saati için)
    let shape_trips: FxHashMap<&str, Vec<&str>> = {
        let mut m: FxHashMap<&str, Vec<&str>> = FxHashMap::default();
        for t in &records.trips {
            if let Some(sid) = t.shape_id.as_deref().filter(|s| !s.is_empty()) {
                m.entry(sid).or_default().push(t.trip_id.as_str());
            }
        }
        m
    };

    {
        let _tg9 = Timer::start("K6::rem::geo_009_shp_013");
        let threshold_km = config.stop_far_from_shape_m / 1000.0;
        let margin_lat_g = threshold_km / 111.0;

        for (&stop_id, shape_ids) in &idx.stop_shapes {
            let Some(&(slat, slon)) = stop_coords.get(stop_id) else { continue };
            let cos_lat = (slat.to_radians()).cos().max(0.001_f64);
            let scale_lon = 111.0 * cos_lat;
            let threshold_sq = threshold_km * threshold_km;
            let margin_lon = threshold_km / scale_lon;
            let safe_sq = threshold_sq * 0.98;

            // Her shape için mesafeyi hesapla — Option-1 filtresi tüm mesafeleri karşılaştırır
            let dists: Vec<(&str, f64)> = shape_ids.iter().filter_map(|&sid| {
                let pts = shape_coords.get(sid)?;
                if pts.is_empty() { return None; }
                let d = if let Some(&[bmin_la, bmax_la, bmin_lo, bmax_lo]) = shape_bbox.get(sid) {
                    if slat < bmin_la - margin_lat_g || slat > bmax_la + margin_lat_g
                        || slon < bmin_lo - margin_lon || slon > bmax_lo + margin_lon
                    {
                        let clat = slat.clamp(bmin_la, bmax_la);
                        let clon = slon.clamp(bmin_lo, bmax_lo);
                        haversine_km(slat, slon, clat, clon)
                    } else {
                        seg_min_dist_km(pts, slat, slon, scale_lon, safe_sq)
                    }
                } else {
                    seg_min_dist_km(pts, slat, slon, scale_lon, safe_sq)
                };
                Some((sid, d))
            }).collect();

            for &(shape_id, min_dist_km) in &dists {
                if min_dist_km <= threshold_km { continue; }

                // Option-1: aynı hattın başka shape'i bu durağı kapsıyorsa atla
                if let Some(rx) = shape_route_set.get(shape_id) {
                    let covered = dists.iter().any(|&(sib, sib_dist)| {
                        sib != shape_id
                            && sib_dist <= threshold_km
                            && shape_route_set.get(sib)
                                .map_or(false, |ry| rx.intersection(ry).next().is_some())
                    });
                    if covered { continue; }
                }

                // Örnek kalkış saati: bu shape × durağa ait en erken sefer
                let example_dep = shape_trips.get(shape_id).and_then(|trips| {
                    trips.iter()
                        .filter_map(|&tid| {
                            idx.by_trip.get(tid)?
                                .iter()
                                .find(|st| st.stop_id == stop_id)
                                .and_then(|st| st.departure_time)
                                .map(|(h, m, _)| h * 3600 + m * 60)
                        })
                        .min()
                        .map(|secs| format!("{:02}:{:02}", secs / 3600, (secs % 3600) / 60))
                });

                let dist_m = min_dist_km * 1000.0;
                let stop_name = stop_names.get(stop_id).copied().unwrap_or(stop_id);
                let dir_label = shape_directions.get(shape_id)
                    .and_then(|d| *d)
                    .map(|d| if d == 0 { " gidiş yönü" } else { " dönüş yönü" })
                    .unwrap_or("");
                let ref_label = match shape_to_route.get(shape_id).copied().flatten() {
                    Some(route) => format!("'{}' hattının{dir_label} güzergahından", route),
                    None        => format!("'{}' no'lu güzergahtan", shape_id),
                };
                let dep_suffix = example_dep.as_deref()
                    .map(|t| format!(", örn. {t} kalkışlı sefer"))
                    .unwrap_or_default();

                let mut notice = k6_notice(
                    ctr,
                    "GEO_009",
                    EntityType::Stop,
                    Some(stop_id.to_string()),
                    Some(stop_id.to_string()),
                    "stops.txt",
                    stop_lines.get(stop_id).copied(),
                    Some("stop_lat|stop_lon"),
                    Some(format!("{dist_m:.1}m (shape '{shape_id}')")),
                    Some(format!("≤ {:.0}m", config.stop_far_from_shape_m)),
                    format!("'{}' kodlu '{}' durağı {ref_label} {dist_m:.1}m uzakta{dep_suffix} (eşik: {:.0}m).",
                        stop_id, stop_name, config.stop_far_from_shape_m),
                    "Durak koordinatlarını veya güzergah noktalarını düzeltin.",
                );
                if let Some(dep) = &example_dep {
                    notice.details = Some([("example_dep".to_string(), dep.clone())].into_iter().collect());
                }
                notices.push(notice);
                // SHP_013 kaldırıldı — GEO_009 ile aynı fiziksel koşulu raporluyordu (çift sayım)
            }
        }
    }

    // ── SHP_024: duraktan şekle mesafe shape_dist_traveled ile tutarsız ────────
    // (stop_too_far_from_shape_using_user_distance)
    // GEO_009 ile fark: polyline'a minimum mesafe değil, shape_dist_traveled ile
    // belirlenen konumdaki şekil noktasına olan mesafe hesaplanır.
    {
        let _ts24 = Timer::start("K6::rem::shp_024");
        let threshold_km = config.stop_far_from_shape_m / 1000.0;

        // shape_id → sorted Vec<(dist, lat, lon)> — yalnızca dist_traveled olan noktalar
        let mut shape_sdt_pts: FxHashMap<&str, Vec<(f64, f64, f64)>> = FxHashMap::default();
        for sp in &records.shapes {
            if let (Some(dist), Some(lat), Some(lon)) = (sp.shape_dist_traveled, sp.shape_pt_lat, sp.shape_pt_lon) {
                shape_sdt_pts.entry(sp.shape_id.as_str()).or_default().push((dist, lat, lon));
            }
        }
        for pts in shape_sdt_pts.values_mut() {
            pts.sort_by(|a, b| a.0.partial_cmp(&b.0).unwrap_or(std::cmp::Ordering::Equal));
        }

        let mut seen_shp024: FxHashSet<(&str, &str)> = FxHashSet::default(); // (stop_id, shape_id)

        for trip in &records.trips {
            let shape_id = match trip.shape_id.as_deref().filter(|s| !s.is_empty()) {
                Some(s) => s,
                None => continue,
            };
            let sdt_pts = match shape_sdt_pts.get(shape_id) {
                Some(pts) if pts.len() >= 2 => pts,
                _ => continue,
            };
            let stimes = match idx.by_trip.get(trip.trip_id.as_str()) {
                Some(v) => v,
                None => continue,
            };

            for st in stimes.iter() {
                let sdt = match st.shape_dist_traveled {
                    Some(d) => d,
                    None => continue,
                };
                let stop_id = st.stop_id.as_str();
                if seen_shp024.contains(&(stop_id, shape_id)) { continue; }

                let (slat, slon) = match stop_coords.get(stop_id) {
                    Some(&c) => c,
                    None => continue,
                };

                // sdt değerine karşılık gelen konumu shape üzerinde interpolasyonla bul
                let pos = sdt_pts.partition_point(|&(d, _, _)| d <= sdt);
                let (ilat, ilon) = if pos == 0 {
                    let &(_, la, lo) = sdt_pts.first().unwrap();
                    (la, lo)
                } else if pos >= sdt_pts.len() {
                    let &(_, la, lo) = sdt_pts.last().unwrap();
                    (la, lo)
                } else {
                    let (da, la, loa) = sdt_pts[pos - 1];
                    let (db, lb, lob) = sdt_pts[pos];
                    if (db - da).abs() < 1e-9 {
                        (la, loa)
                    } else {
                        let t = (sdt - da) / (db - da);
                        (la + t * (lb - la), loa + t * (lob - loa))
                    }
                };

                let dist_km = haversine_km(slat, slon, ilat, ilon);
                if dist_km > threshold_km {
                    seen_shp024.insert((stop_id, shape_id));
                    let stop_name = stop_names.get(stop_id).copied().unwrap_or(stop_id);
                    notices.push(k6_notice(
                        ctr, "SHP_024", EntityType::Stop,
                        Some(stop_id.to_string()), Some(stop_id.to_string()),
                        "stop_times.txt", Some(st.line), Some("shape_dist_traveled"),
                        Some(format!("{:.1}m shape '{shape_id}'daki sdt={sdt:.3}", dist_km * 1000.0)),
                        Some(format!("≤ {:.0}m", config.stop_far_from_shape_m)),
                        format!("'{}' durağı (shape_dist_traveled={sdt:.3}) shape_dist_traveled konumundan {:.1}m uzakta (eşik: {:.0}m).",
                            stop_name, dist_km * 1000.0, config.stop_far_from_shape_m),
                        "stop_times.txt'deki shape_dist_traveled değerini ya da stop koordinatlarını düzeltin.",
                    ));
                }
            }
        }
    }

    // ── SHP_025: stop_times shape_dist_traveled şeklin toplam mesafesini aşıyor ─
    // (trip_distance_exceeds_shape_distance)
    {
        let _ts25 = Timer::start("K6::rem::shp_025");
        // shape_id → max shape_dist_traveled değeri (shapes.txt'ten)
        let mut shape_max_sdt: FxHashMap<&str, f64> = FxHashMap::default();
        for sp in &records.shapes {
            if let Some(d) = sp.shape_dist_traveled {
                let e = shape_max_sdt.entry(sp.shape_id.as_str()).or_insert(0.0);
                if d > *e { *e = d; }
            }
        }
        // Yalnızca tüm shape noktaları dist_traveled içeriyorsa kontrol et
        // (shapes.txt'te bazı noktalarda yoksa karşılaştırma güvenilmez)
        let mut shape_has_full_sdt: FxHashSet<&str> = FxHashSet::default();
        {
            let mut shape_total: FxHashMap<&str, u32> = FxHashMap::default();
            let mut shape_with_sdt: FxHashMap<&str, u32> = FxHashMap::default();
            for sp in &records.shapes {
                if !sp.shape_id.is_empty() {
                    *shape_total.entry(sp.shape_id.as_str()).or_default() += 1;
                    if sp.shape_dist_traveled.is_some() {
                        *shape_with_sdt.entry(sp.shape_id.as_str()).or_default() += 1;
                    }
                }
            }
            for (sid, total) in &shape_total {
                if shape_with_sdt.get(sid).copied().unwrap_or(0) == *total {
                    shape_has_full_sdt.insert(sid);
                }
            }
        }

        for trip in &records.trips {
            let shape_id = match trip.shape_id.as_deref().filter(|s| !s.is_empty()) {
                Some(s) => s,
                None => continue,
            };
            if !shape_has_full_sdt.contains(shape_id) { continue; }
            let shape_max = match shape_max_sdt.get(shape_id) {
                Some(&m) => m,
                None => continue,
            };

            let stimes = match idx.by_trip.get(trip.trip_id.as_str()) {
                Some(v) => v,
                None => continue,
            };

            // stop_times'daki en büyük shape_dist_traveled değeri
            let trip_max_sdt = stimes.iter()
                .filter_map(|st| st.shape_dist_traveled)
                .fold(f64::NEG_INFINITY, f64::max);

            if trip_max_sdt.is_finite() && trip_max_sdt > shape_max * 1.001 {
                let route = trip_to_route_rem.get(trip.trip_id.as_str()).copied().unwrap_or(trip.trip_id.as_str());
                notices.push(k6_notice(
                    ctr, "SHP_025", EntityType::Trip,
                    Some(trip.trip_id.clone()), Some(trip.trip_id.clone()),
                    "stop_times.txt", None, Some("shape_dist_traveled"),
                    Some(format!("{trip_max_sdt:.3}")),
                    Some(format!("≤ {shape_max:.3}")),
                    format!("'{}' hattının seferinde stop_times shape_dist_traveled ({trip_max_sdt:.3}) şeklin maksimum değerini ({shape_max:.3}) aşıyor.",
                        route),
                    "stop_times.txt'deki shape_dist_traveled değerlerini shapes.txt ölçeğiyle eşleştirin.",
                ));
            }
        }
    }

    // ── GEO_012: stop kümesi — 3'ten fazla durak çok yakın ───────────────────
    {
        let _tg12 = Timer::start("K6::rem::geo_012");
        let cluster_km = config.stop_too_close_m / 1000.0;
        for cell_stops in derived.spatial_index.grid.values() {
            if cell_stops.len() < 3 {
                continue;
            }
            // Her üçlü kombinasyon için kontrol etmek yerine: hücredeki tüm durağı say
            // ve herhangi biri çok yakınsa cluster olduğunu rapor et
            let cluster_count = cell_stops
                .iter()
                .filter(|&&i| records.stops.get(i).is_some())
                .count();

            if cluster_count >= 3 {
                // Merkez durağı bul (hücredeki ilk)
                let first_idx = cell_stops[0];
                if let Some(anchor) = records.stops.get(first_idx) {
                    let Some((alat, alon)) = anchor.stop_lat.zip(anchor.stop_lon) else { continue };
                    let nearby = cell_stops
                        .iter()
                        .filter(|&&i| {
                            i != first_idx && records.stops.get(i).map(|s| {
                                s.stop_lat.zip(s.stop_lon)
                                    .map(|(la, lo)| haversine_km(alat, alon, la, lo) < cluster_km)
                                    .unwrap_or(false)
                            }).unwrap_or(false)
                        })
                        .count();
                    if nearby >= 2 {
                        notices.push(k6_notice(
                            ctr,
                            "GEO_012",
                            EntityType::Stop,
                            Some(anchor.stop_id.clone()),
                            Some(anchor.stop_id.clone()),
                            "stops.txt",
                            Some(anchor.line),
                            Some("stop_lat|stop_lon"),
                            Some(format!("{} yakın durak kümesi", nearby + 1)),
                            None,
                            format!("'{}' durağu (kod: '{}') çevresinde {} durak {:.0}m içinde kümelenmiş.",
                                anchor.stop_name.as_deref().unwrap_or(anchor.stop_id.as_str()),
                                anchor.stop_id, nearby + 1, config.stop_too_close_m),
                            "Duraksallar aynı fiziksel noktayı temsil ediyorsa birleştirin.",
                        ));
                    }
                }
            }
        }
    }

    // ── GEO_013: Feed coğrafi kapsam özeti (Bilgi) ────────────────────────────
    {
        let n_stops_with_coords = records.stops.iter().filter(|s| s.stop_lat.is_some()).count();
        if n_stops_with_coords > 0 {
            notices.push(k6_notice(
                ctr,
                "GEO_013",
                EntityType::Feed,
                None,
                None,
                "stops.txt",
                None,
                None,
                Some(format!("{n_stops_with_coords} durak koordinatlı")),
                None,
                format!("Feed {n_stops_with_coords} koordinatlı durak içeriyor."),
                "Bu bilgi notu; aksiyona gerek yoktur.",
            ));
        }
    }

    // ── OPR_005: Rota ortalama headway bilgisi (Bilgi) ────────────────────────
    {
        let _topr5 = Timer::start("K6::rem::opr_005");
        // (route_id, direction_key, service_id) → trip departure times
        let route_short_opr5: HashMap<&str, &str> = records.routes.iter()
            .map(|r| {
                let label = r.route_short_name.as_deref().filter(|s| !s.is_empty()).unwrap_or(r.route_id.as_str());
                (r.route_id.as_str(), label)
            })
            .collect();

        let mut route_headways: HashMap<(&str, &str, &str), Vec<u32>> = HashMap::new();
        for trip in &records.trips {
            let route_id = trip.route_id.as_str();
            if route_id.is_empty() { continue; }
            let dir_key: &str = match trip.direction_id { Some(0) => "0", Some(1) => "1", _ => "-" };
            let svc_key = trip.service_id.as_str();
            if let Some(&dep) = idx.trip_first_dep.get(trip.trip_id.as_str()) {
                route_headways.entry((route_id, dir_key, svc_key)).or_default().push(dep);
            }
        }

        for ((route_id, dir_key, svc_key), mut deps) in route_headways {
            if deps.len() < 2 {
                continue;
            }
            deps.sort_unstable();
            deps.dedup();
            if deps.len() < 2 { continue; }
            let diffs: Vec<u32> = deps.windows(2).map(|w| w[1] - w[0]).collect();
            let avg_hw = diffs.iter().sum::<u32>() / diffs.len() as u32;
            let route_label = route_short_opr5.get(route_id).copied().unwrap_or(route_id);
            notices.push(k6_notice(
                ctr,
                "OPR_005",
                EntityType::Route,
                Some(route_id.to_string()),
                Some(route_id.to_string()),
                "stop_times.txt",
                None,
                None,
                Some(format!("{:.0}dk ortalama headway", avg_hw as f64 / 60.0)),
                None,
                format!("'{route_label}' kodlu hattın {dir_key} yönünde {svc_key} çalışma takviminde ortalama sefer aralığı {:.0}dk ({} sefer).",
                    avg_hw as f64 / 60.0, deps.len()),
                "Bu bilgi notu; aksiyona gerek yoktur.",
            ));
        }
    }

    // ── OPR_013: tek yönlü rota bilgisi (tüm seferler aynı direction_id) ──────
    {
        let _topr13 = Timer::start("K6::rem::opr_013");
        let mut route_directions: HashMap<&str, HashSet<u32>> = HashMap::new();
        for t in &records.trips {
            if t.route_id.is_empty() {
                continue;
            }
            if let Some(dir) = t.direction_id {
                route_directions
                    .entry(t.route_id.as_str())
                    .or_default()
                    .insert(dir);
            }
        }
        for (route_id, dirs) in &route_directions {
            if dirs.len() == 1 {
                let dir = dirs.iter().next().copied().unwrap_or(0);
                notices.push(k6_notice(
                    ctr,
                    "OPR_013",
                    EntityType::Route,
                    Some(route_id.to_string()),
                    Some(route_id.to_string()),
                    "trips.txt",
                    None,
                    Some("direction_id"),
                    Some(format!("direction_id={dir}")),
                    None,
                    format!("'{route_id}' hattında yalnızca {} yönlü seferler var — karşı yön tanımlı değil.",
                        if dir == 0 { "gidiş" } else { "dönüş" }),
                    "Bu bilgi notu; tek yönlü hatlar için beklenen bir durumdur.",
                ));
            }
        }
    }

    // ── DQ_005b: feed'de stop_times olan trip yok ─────────────────────────────
    {
        if !records.trips.is_empty() {
            let trips_without = records
                .trips
                .iter()
                .filter(|t| !idx.by_trip.contains_key(t.trip_id.as_str()))
                .count();
            let total = records.trips.len();
            if trips_without == total {
                // Hiçbir trip'in stop_times yok — XFL_002'den daha geniş feed-level kapsam
                notices.push(k6_notice(
                    ctr,
                    "DQ_005b",
                    EntityType::Feed,
                    None,
                    None,
                    "stop_times.txt",
                    None,
                    None,
                    Some("tüm seferler stop_times'sız".to_string()),
                    None,
                    "Feed'deki hiçbir sefer için stop_times kaydı bulunamadı.".to_string(),
                    "stop_times.txt dosyasını oluşturun.",
                ));
            }
        }
    }

    // ── DQ_005c: stop koordinatları olmayan durağın oranı çok yüksek ──────────
    {
        if !records.stops.is_empty() {
            let without_coords = records
                .stops
                .iter()
                .filter(|s| s.stop_lat.is_none() || s.stop_lon.is_none())
                .count();
            let ratio = without_coords as f64 / records.stops.len() as f64;
            if ratio > 0.5 {
                notices.push(k6_notice(
                    ctr,
                    "DQ_005c",
                    EntityType::Feed,
                    None,
                    None,
                    "stops.txt",
                    None,
                    None,
                    Some(format!("%{:.0} koordinatsız durak", ratio * 100.0)),
                    Some("≤ %50".to_string()),
                    format!("Duraksaların %{:.0}'ında koordinat eksik — coğrafi analiz kısıtlı.",
                        ratio * 100.0),
                    "stop_lat ve stop_lon alanlarını tüm duraklara ekleyin.",
                ));
            }
        }
    }

    // ── DQ_010: route olmayan agency ──────────────────────────────────────────
    {
        let agencies_in_routes: HashSet<&str> = records
            .routes
            .iter()
            .filter_map(|r| r.agency_id.as_deref())
            .collect();
        for ag in &records.agencies {
            let Some(ref aid) = ag.agency_id else { continue };
            if !agencies_in_routes.contains(aid.as_str()) {
                notices.push(k6_notice(
                    ctr,
                    "DQ_010",
                    EntityType::Feed,
                    None,
                    None,
                    "agency.txt",
                    Some(ag.line),
                    Some("agency_id"),
                    Some(aid.clone()),
                    None,
                    format!("'{}' işleticisi hiçbir hatta kullanılmıyor.",
                        if ag.agency_name.is_empty() { aid.as_str() } else { ag.agency_name.as_str() }),
                    "Kullanılmayan agency kaydını kaldırın ya da routes.txt'de agency_id atayın.",
                ));
            }
        }
    }

    // ── RTS_017: shape'siz route oranı bilgisi ────────────────────────────────
    {
        let mut route_has_shape: HashMap<&str, bool> = HashMap::new();
        for t in &records.trips {
            let entry = route_has_shape.entry(t.route_id.as_str()).or_insert(false);
            if t.shape_id.is_some() {
                *entry = true;
            }
        }
        let shapeless_routes: Vec<&str> = route_has_shape
            .iter()
            .filter(|(_, &has)| !has)
            .map(|(&rid, _)| rid)
            .collect();
        if !shapeless_routes.is_empty() {
            let count = shapeless_routes.len();
            notices.push(k6_notice(
                ctr,
                "RTS_017",
                EntityType::Route,
                shapeless_routes.first().map(|r| r.to_string()),
                shapeless_routes.first().map(|r| r.to_string()),
                "routes.txt",
                None,
                Some("route_id"),
                Some(format!("{count} rotada güzergah şekli eksik")),
                None,
                format!("{count} rotanın hiçbir seferinde shape_id tanımlı değil — harita gösterimi kısıtlı."),
                "Tüm rotalar için shapes.txt tanımlayın ve trips.txt'de shape_id atayın.",
            ));
        }
    }

    // ── TRP_012: trip yönü route'taki diğer seferlerle tutarsız ──────────────
    // (direction_id set ama bazı seferlerle çelişen)
    {
        let mut route_dir_trips: HashMap<(&str, u32), u32> = HashMap::new();
        for t in &records.trips {
            if t.route_id.is_empty() { continue; }
            if let Some(dir) = t.direction_id {
                *route_dir_trips.entry((t.route_id.as_str(), dir)).or_default() += 1;
            }
        }
        // Aynı rotada her iki yönde de sefer var; yalnızca direction_id set olmayan seferler → TRP_012
        let routes_with_both_dirs: HashSet<&str> = {
            let mut r: HashMap<&str, HashSet<u32>> = HashMap::new();
            for t in &records.trips {
                if let Some(d) = t.direction_id {
                    r.entry(t.route_id.as_str()).or_default().insert(d);
                }
            }
            r.into_iter().filter(|(_, dirs)| dirs.len() > 1).map(|(rid, _)| rid).collect()
        };
        for t in &records.trips {
            if !routes_with_both_dirs.contains(t.route_id.as_str()) { continue; }
            if t.direction_id.is_none() {
                notices.push(k6_notice(
                    ctr,
                    "TRP_012",
                    EntityType::Trip,
                    Some(t.trip_id.clone()),
                    Some(t.trip_id.clone()),
                    "trips.txt",
                    Some(t.line),
                    Some("direction_id"),
                    Some("eksik".to_string()),
                    None,
                    format!("'{}' hattının seferinde yön bilgisi (direction_id) girilmemiş — hat çift yönlü.", t.route_id),
                    "direction_id alanını 0 veya 1 olarak doldurun.",
                ));
            }
        }
    }

    // ── TRP_015: trip block_id tek (block'ta başka sefer yok) ─────────────────
    {
        let mut block_count: HashMap<&str, u32> = HashMap::new();
        for t in &records.trips {
            if let Some(ref bid) = t.block_id {
                if !bid.is_empty() {
                    *block_count.entry(bid.as_str()).or_default() += 1;
                }
            }
        }
        for t in &records.trips {
            if let Some(ref bid) = t.block_id {
                if block_count.get(bid.as_str()).copied().unwrap_or(0) == 1 {
                    notices.push(k6_notice(
                        ctr,
                        "TRP_015",
                        EntityType::Trip,
                        Some(t.trip_id.clone()),
                        Some(t.trip_id.clone()),
                        "trips.txt",
                        Some(t.line),
                        Some("block_id"),
                        Some(bid.clone()),
                        None,
                        format!("'{}' hattının seferinde '{bid}' blok kodu tek kullanılmış — blok en az 2 sefer içermelidir.", t.route_id),
                        "block_id'nin aynı araca atanan birden fazla sefer için kullanıldığından emin olun.",
                    ));
                }
            }
        }
    }

    // ── STP_020: stop_times'da hiç kullanılmayan fiziksel durak ──────────────
    {
        let _t20 = Timer::start("K6::rem::stp_020");
        let used_stops: FxHashSet<&str> = records.stop_times_index.stop_id_set
            .iter()
            .map(|s| s.as_str())
            .collect();

        for stop in &records.stops {
            if stop.stop_id.is_empty() { continue; }
            // Sadece fiziksel duraklar (location_type=0 veya null); parent station/giriş vs. hariç
            if stop.location_type.unwrap_or(0) != 0 { continue; }
            if !used_stops.contains(stop.stop_id.as_str()) {
                notices.push(k6_notice(
                    ctr,
                    "STP_020",
                    EntityType::Stop,
                    Some(stop.stop_id.clone()),
                    Some(stop.stop_id.clone()),
                    "stops.txt",
                    Some(stop.line),
                    None,
                    None,
                    None,
                    {
                        let name = stop.stop_name.as_deref().unwrap_or(stop.stop_id.as_str());
                        format!("'{}' durağı (kod: '{}') hiçbir seferde kullanılmıyor.", name, stop.stop_id)
                    },
                    "Durağı kaldırın veya ilgili bir sefere ekleyin.",
                ));
            }
        }
    }

    // ── SHP_017 / SHP_022 paylaşımlı altyapı ─────────────────────────────────
    // trip_id → shape_id ve shape_id → arc dizisi her iki kural tarafından kullanılır.
    let trip_shape_local: HashMap<&str, &str> = records
        .trips
        .iter()
        .filter_map(|t| t.shape_id.as_deref().map(|s| (t.trip_id.as_str(), s)))
        .collect();
    let mut shape_cum: FxHashMap<&str, Vec<f64>> = FxHashMap::default();

    // ── SHP_017: trip'teki durak sırası shape projeksiyonuyla çelişiyor ──────
    // Her trip için stop_sequence sırasındaki durakların shape üzerindeki
    // arc-length projeksiyonları monoton artmalıdır.
    {
        let _t17 = Timer::start("K6::rem::shp_017");
        // (shape_id, stop_id) → arc — aynı çifti birden fazla trip paylaşabilir
        let mut arc_cache: FxHashMap<(&str, &str), f64> = FxHashMap::default();
        // Aynı (shape_id, problem_stop_id) çifti için tek notice üret
        // (aynı route'un tüm tripleri aynı shape sorununu tekrarlamamalı)
        let mut shp017_seen: FxHashSet<(&str, &str)> = FxHashSet::default();
        // SHP_016: tamamen ters shape — shape_id başına tek notice
        let mut shp016_seen: FxHashSet<&str> = FxHashSet::default();

        for (trip_id, stimes) in &idx.by_trip {
            let Some(&shape_id) = trip_shape_local.get(trip_id) else { continue };
            let Some(pts) = shape_coords.get(shape_id) else { continue };
            if pts.len() < 2 { continue; }

            let cum = shape_cum.entry(shape_id).or_insert_with(|| {
                let mut c = Vec::with_capacity(pts.len());
                c.push(0.0_f64);
                for i in 1..pts.len() {
                    c.push(c[i - 1] + haversine_km(pts[i-1].0, pts[i-1].1, pts[i].0, pts[i].1));
                }
                c
            });

            let mut sorted: Vec<&CompactStopTime> = stimes.iter().collect();
            sorted.sort_by_key(|st| st.stop_sequence.unwrap_or(0));

            // Dairesel shape tespiti: shape başlangıcı ile bitişi birbirine yakınsa
            // (çevre hattı / döngü) arc-monotonicity kontrolü anlamsızlaşır → atla
            {
                let shape_start = pts[0];
                let shape_end   = *pts.last().unwrap();
                let ring_dist_km = haversine_km(shape_start.0, shape_start.1, shape_end.0, shape_end.1);
                let shape_total_km = cum.last().copied().unwrap_or(1.0);
                // Başlangıç-bitiş mesafesi toplam uzunluğun %10'undan az VE 1km'den kısaysa dairesel
                if ring_dist_km < shape_total_km * 0.10 && ring_dist_km < 1.0 {
                    continue;
                }
            }

            // ── SHP_016: shape tamamen ters yönde çizilmiş ───────────────────────
            // İlk durak shape'in ikinci yarısına projekte oluyorsa shape ters takılmış.
            // Bu durumda SHP_017 yanlış konumda "sıra bozuk" der; SHP_016 daha net.
            if !shp016_seen.contains(shape_id) {
                let shape_total_km = cum.last().copied().unwrap_or(0.0);
                if shape_total_km > 0.1 {
                    if let Some(first_st) = sorted.first() {
                        if let Some(&(slat, slon)) = stop_coords.get(first_st.stop_id.as_str()) {
                            let first_arc = project_arc_km(pts, cum, slat, slon);
                            if first_arc > shape_total_km * 0.5 {
                                shp016_seen.insert(shape_id);
                                let route = trip_to_route_rem.get(trip_id).copied().unwrap_or(trip_id);
                                let mut n016 = k6_notice(
                                    ctr,
                                    "SHP_016",
                                    EntityType::Shape,
                                    Some(shape_id.to_string()),
                                    Some(shape_id.to_string()),
                                    "trips.txt",
                                    None,
                                    Some("shape_id"),
                                    Some(shape_id.to_string()),
                                    None,
                                    format!(
                                        "'{route}' hattının '{shape_id}' güzergahı ters yönde çizilmiş — ilk durak shape'in başından değil sonuna yakın projekte oluyor ({:.0}m / {:.0}m).",
                                        first_arc * 1000.0, shape_total_km * 1000.0
                                    ),
                                    "shape_pt_sequence sırasını tersine çevirin veya bu yön için ayrı bir shape_id tanımlayın.",
                                );
                                let mut d = std::collections::HashMap::new();
                                d.insert("first_stop".to_string(), first_st.stop_id.to_string());
                                d.insert("trip_id".to_string(), trip_id.to_string());
                                n016.details = Some(d);
                                notices.push(n016);
                                continue;
                            }
                        }
                    }
                }
            }

            // stop_times'da shape_dist_traveled varsa geometrik projeksiyona gerek yok.
            // Tüm duraklarda mevcutsa yetkili kaynak olarak kullan; karışık durumdaysa
            // birim uyuşmazlığından kaçınmak için geometrik kontrolü atla.
            let all_have_sdt = sorted.iter().all(|st| st.shape_dist_traveled.is_some());
            let any_have_sdt = sorted.iter().any(|st| st.shape_dist_traveled.is_some());
            if any_have_sdt && !all_have_sdt { continue; }

            // shape_dist_traveled metre mi km mi? shape toplam uzunluğuyla karşılaştır.
            let sdt_to_km: f64 = if all_have_sdt {
                let sdt_max = sorted.iter()
                    .filter_map(|st| st.shape_dist_traveled)
                    .fold(0.0_f64, f64::max);
                let shape_total_km = cum.last().copied().unwrap_or(1.0);
                if sdt_max > shape_total_km * 100.0 { 0.001 } else { 1.0 }
            } else {
                1.0
            };

            let mut prev_arc = -1.0_f64;
            let mut prob_idx: Option<usize> = None;
            for (i, st) in sorted.iter().enumerate() {
                let arc = if all_have_sdt {
                    st.shape_dist_traveled.unwrap() * sdt_to_km
                } else {
                    let Some(&(slat, slon)) = stop_coords.get(st.stop_id.as_str()) else { continue };
                    *arc_cache
                        .entry((shape_id, st.stop_id.as_str()))
                        .or_insert_with(|| project_arc_km(pts, cum, slat, slon))
                };
                // 500 m tolerans: projeksiyon gürültüsü ve S-eğri false positive'leri filtreler
                if arc < prev_arc - 0.500 {
                    prob_idx = Some(i);
                    break;
                }
                if arc > prev_arc { prev_arc = arc; }
            }
            if let Some(pi) = prob_idx {
                // Backtrack filtresi: prob_idx sonraki duraklarda arc prev_arc'ı geçiyorsa
                // shape geriye dönüp ilerlemeye devam ediyor — false positive, atla.
                let recovers = sorted[pi + 1..].iter().take(3).any(|nxt| {
                    let narc = if all_have_sdt {
                        nxt.shape_dist_traveled.unwrap_or(0.0) * sdt_to_km
                    } else {
                        arc_cache.get(&(shape_id, nxt.stop_id.as_str())).copied().unwrap_or(0.0)
                    };
                    narc > prev_arc
                });
                if recovers { continue; }

                let st = sorted[pi];
                // Aynı (shape_id, problem_stop_id) çifti için daha önce notice üretildiyse atla
                if !shp017_seen.insert((shape_id, st.stop_id.as_str())) {
                    continue;
                }
                let route = trip_to_route_rem.get(trip_id).copied().unwrap_or(trip_id);
                let dep = trip_first_dep.get(trip_id)
                    .map(|(h, m, _)| format!("{h:02}:{m:02}"))
                    .unwrap_or_default();
                let dep_infix = if dep.is_empty() { String::new() } else { format!(" {dep}") };
                let sname = stop_names.get(st.stop_id.as_str()).copied().unwrap_or(st.stop_id.as_str());
                let arc = if all_have_sdt {
                    st.shape_dist_traveled.unwrap() * sdt_to_km
                } else {
                    arc_cache.get(&(shape_id, st.stop_id.as_str())).copied().unwrap_or(0.0)
                };
                let mut notice = k6_notice(
                    ctr,
                    "SHP_017",
                    EntityType::Trip,
                    Some(trip_id.to_string()),
                    Some(trip_id.to_string()),
                    "stop_times.txt",
                    Some(st.line),
                    Some("stop_sequence"),
                    Some(st.stop_sequence.unwrap_or(0).to_string()),
                    None,
                    format!(
                        "'{}' hattının{dep_infix} seferinde '{}' (kod: '{}') {}. sıradaki durak güzergah sırası bozuk — başlangıca uzaklığı önceki durağın gerisinde ({:.0}m < {:.0}m).",
                        route, sname, st.stop_id, st.stop_sequence.unwrap_or(0),
                        arc * 1000.0, prev_arc * 1000.0,
                    ),
                    "En yaygın neden: aynı shape_id zıt yönlerde kullanılıyor — her yön için ayrı shape_id tanımlayın. Diğer nedenler: shape noktaları coğrafi olarak geri dönüyor, durak koordinatları yanlış veya stop_sequence sırası bozuk.",
                );
                // Harita bağlamı: ±3 komşu durak + shape_id + sıra numaraları
                let ctx_b: Vec<&str> = sorted[..pi].iter().rev().take(3).rev()
                    .map(|s| s.stop_id.as_str()).collect();
                let ctx_a: Vec<&str> = sorted[pi + 1..].iter().take(3)
                    .map(|s| s.stop_id.as_str()).collect();
                let seq_b: Vec<String> = sorted[..pi].iter().rev().take(3).rev()
                    .map(|s| s.stop_sequence.unwrap_or(0).to_string()).collect();
                let seq_a: Vec<String> = sorted[pi + 1..].iter().take(3)
                    .map(|s| s.stop_sequence.unwrap_or(0).to_string()).collect();
                let mut det = HashMap::new();
                det.insert("ctx_b".to_string(), ctx_b.join(","));
                det.insert("ctx_a".to_string(), ctx_a.join(","));
                det.insert("seq_b".to_string(), seq_b.join(","));
                det.insert("seq_a".to_string(), seq_a.join(","));
                det.insert("shape_id".to_string(), shape_id.to_string());
                det.insert("bad_stop".to_string(), st.stop_id.to_string());
                notice.details = Some(det);
                notices.push(notice);
            }
        }
    }


    // ── SHP_014: ilk/son durak güzergah ucundan uzakta ───────────────────────
    {
        let _t14 = Timer::start("K6::rem::shp_014");
        let threshold_km = config.stop_far_from_shape_m / 1000.0;
        let mut shp014_start_seen: FxHashSet<&str> = FxHashSet::default();
        let mut shp014_end_seen: FxHashSet<&str> = FxHashSet::default();

        for (&trip_id, stimes) in &idx.by_trip {
            let Some(&shape_id) = trip_shape_local.get(trip_id) else { continue };
            let Some(pts) = shape_coords.get(shape_id) else { continue };
            if pts.len() < 2 { continue; }

            let shape_start = pts[0];
            let shape_end = *pts.last().unwrap();

            if !shp014_start_seen.contains(shape_id) {
                if let Some(first_st) = stimes.first() {
                    if let Some(&(slat, slon)) = stop_coords.get(first_st.stop_id.as_str()) {
                        let d_km = haversine_km(slat, slon, shape_start.0, shape_start.1);
                        if d_km > threshold_km {
                            shp014_start_seen.insert(shape_id);
                            let dist_m = d_km * 1000.0;
                            let sname = stop_names.get(first_st.stop_id.as_str()).copied().unwrap_or(first_st.stop_id.as_str());
                            let mut n = k6_notice(
                                ctr, "SHP_014",
                                EntityType::Shape,
                                Some(shape_id.to_string()),
                                Some(shape_id.to_string()),
                                "shapes.txt", None,
                                Some("shape_pt_sequence"),
                                Some(format!("{dist_m:.0}m")),
                                Some(format!("≤ {:.0}m", config.stop_far_from_shape_m)),
                                format!(
                                    "'{}' güzergah şeklinin başlangıç noktası, ilk durak '{}' (kod: '{}') konumundan {dist_m:.0}m uzakta.",
                                    shape_id, sname, first_st.stop_id
                                ),
                                "shape_dist_traveled veya güzergah şeklinin başlangıç noktasını ilk durağa hizalayın.",
                            );
                            let mut d = std::collections::HashMap::new();
                            d.insert("problem_stop".to_string(), first_st.stop_id.to_string());
                            d.insert("endpoint".to_string(), "start".to_string());
                            d.insert("trip_id".to_string(), trip_id.to_string());
                            n.details = Some(d);
                            notices.push(n);
                        }
                    }
                }
            }

            if !shp014_end_seen.contains(shape_id) {
                if let Some(last_st) = stimes.last() {
                    if let Some(&(slat, slon)) = stop_coords.get(last_st.stop_id.as_str()) {
                        let d_km = haversine_km(slat, slon, shape_end.0, shape_end.1);
                        if d_km > threshold_km {
                            shp014_end_seen.insert(shape_id);
                            let dist_m = d_km * 1000.0;
                            let sname = stop_names.get(last_st.stop_id.as_str()).copied().unwrap_or(last_st.stop_id.as_str());
                            let mut n = k6_notice(
                                ctr, "SHP_014",
                                EntityType::Shape,
                                Some(shape_id.to_string()),
                                Some(shape_id.to_string()),
                                "shapes.txt", None,
                                Some("shape_pt_sequence"),
                                Some(format!("{dist_m:.0}m")),
                                Some(format!("≤ {:.0}m", config.stop_far_from_shape_m)),
                                format!(
                                    "'{}' güzergah şeklinin bitiş noktası, son durak '{}' (kod: '{}') konumundan {dist_m:.0}m uzakta.",
                                    shape_id, sname, last_st.stop_id
                                ),
                                "shape_dist_traveled veya güzergah şeklinin bitiş noktasını son durağa hizalayın.",
                            );
                            let mut d = std::collections::HashMap::new();
                            d.insert("problem_stop".to_string(), last_st.stop_id.to_string());
                            d.insert("endpoint".to_string(), "end".to_string());
                            d.insert("trip_id".to_string(), trip_id.to_string());
                            n.details = Some(d);
                            notices.push(n);
                        }
                    }
                }
            }
        }
    }

    // ── STP_029: parent_station'dan çok uzak durak ────────────────────────────
    {
        let _t29 = Timer::start("K6::rem::stp_029");
        let threshold_m = config.stop_far_from_parent_m;

        // stop_id → (lat, lon)
        let stop_coords: HashMap<&str, (f64, f64)> = records.stops.iter()
            .filter_map(|s| {
                s.stop_lat.zip(s.stop_lon).map(|(la, lo)| (s.stop_id.as_str(), (la, lo)))
            })
            .collect();

        for stop in &records.stops {
            // Sadece location_type 0 veya tanımsız (fiziksel durak)
            if stop.location_type.unwrap_or(0) != 0 { continue; }
            let parent_id = match stop.row.get("parent_station") {
                Some(p) if !p.trim().is_empty() => p.trim(),
                _ => continue,
            };
            let (slat, slon) = match stop.stop_lat.zip(stop.stop_lon) {
                Some(c) => c,
                None => continue,
            };
            let (plat, plon) = match stop_coords.get(parent_id) {
                Some(&c) => c,
                None => continue,
            };
            let dist_m = haversine_km(slat, slon, plat, plon) * 1000.0;
            if dist_m > threshold_m {
                let sname = stop.stop_name.as_deref().unwrap_or(stop.stop_id.as_str());
                let mut notice = k6_notice(
                    ctr,
                    "STP_029",
                    EntityType::Stop,
                    Some(stop.stop_id.clone()),
                    Some(stop.stop_id.clone()),
                    "stops.txt",
                    Some(stop.line),
                    Some("parent_station"),
                    Some(format!("{dist_m:.0}m")),
                    Some(format!("≤ {threshold_m:.0}m")),
                    format!(
                        "'{}' kodlu '{}' durağı üst istasyonundan ({}) {dist_m:.0}m uzakta (eşik: {threshold_m:.0}m).",
                        stop.stop_id, sname, parent_id
                    ),
                    "Durak koordinatlarını veya parent_station referansını düzeltin.",
                );
                let mut det = HashMap::new();
                det.insert("parent_id".to_string(), parent_id.to_string());
                notice.details = Some(det);
                notices.push(notice);
            }
        }
    }

    // ── STP_030: hiç çocuğu olmayan üst istasyon (location_type=1) ────────────
    {
        let _t30 = Timer::start("K6::rem::stp_030");

        // Hangi station_id'lerin çocuğu var?
        let stations_with_children: HashSet<&str> = records.stops.iter()
            .filter_map(|s| {
                s.row.get("parent_station")
                    .map(|p| p.trim())
                    .filter(|p| !p.is_empty())
            })
            .collect();

        for stop in &records.stops {
            if stop.location_type != Some(1) { continue; }
            if stop.stop_id.is_empty() { continue; }
            if !stations_with_children.contains(stop.stop_id.as_str()) {
                let sname = stop.stop_name.as_deref().unwrap_or(stop.stop_id.as_str());
                notices.push(k6_notice(
                    ctr,
                    "STP_030",
                    EntityType::Stop,
                    Some(stop.stop_id.clone()),
                    Some(stop.stop_id.clone()),
                    "stops.txt",
                    Some(stop.line),
                    None,
                    None,
                    None,
                    format!(
                        "'{}' kodlu '{}' istasyonu hiçbir durağın parent_station'ı olarak kullanılmıyor.",
                        stop.stop_id, sname
                    ),
                    "Bu istasyona bağlı fiziksel duraklar için parent_station alanını doldurun veya istasyonu kaldırın.",
                ));
            }
        }
    }

    // B4: stop_headsign 36MB taraması DQ_018+DQ_019 için BİR kez yapılır (önceden iki kez).
    // Ham (line, value) toplanır; notice'lar aşağıda kendi blok konumlarında MINT edilir →
    // emisyon sırası, ctr, id ve içerik birebir korunur (yalnız tarama sayısı 2→1).
    let (dq_caps_hs, dq_lower_hs): (Vec<(u64, String)>, Vec<(u64, String)>) = {
        let mut caps: Vec<(u64, String)> = Vec::new();
        let mut lower: Vec<(u64, String)> = Vec::new();
        let mut seen_caps: std::collections::HashSet<&str> = std::collections::HashSet::new();
        let mut seen_lower: std::collections::HashSet<&str> = std::collections::HashSet::new();
        for stops in records.stop_times_index.iter_stops() {
            for st in stops {
                if let Some(ref hs) = st.stop_headsign {
                    let s = hs.as_str();
                    if !seen_caps.contains(s) && is_all_caps(s) {
                        seen_caps.insert(s);
                        caps.push((st.line, s.to_string()));
                    }
                    if !seen_lower.contains(s) && is_all_lower(s) {
                        seen_lower.insert(s);
                        lower.push((st.line, s.to_string()));
                    }
                }
            }
        }
        (caps, lower)
    };

    // ── DQ_018: önerilen metin alanları tamamen büyük harf (mixed_case_recommended_field) ──
    {
        let _t18 = Timer::start("K6::rem::dq_018");

        // stop_name
        for stop in &records.stops {
            if stop.stop_id.is_empty() { continue; }
            if let Some(name) = stop.stop_name.as_deref().filter(|s| is_all_caps(s)) {
                notices.push(k6_notice(
                    ctr, "DQ_018", EntityType::Stop,
                    Some(stop.stop_id.clone()), Some(stop.stop_id.clone()),
                    "stops.txt", Some(stop.line), Some("stop_name"),
                    Some(name.to_string()), None,
                    format!("'{}' durağının adı tamamen büyük harf: '{name}'.", stop.stop_id),
                    "Durak adını düzgün harf kuralıyla yazın (ör. 'Merkez İstasyon').",
                ));
            }
        }

        // route_long_name
        for route in &records.routes {
            if route.route_id.is_empty() { continue; }
            if let Some(name) = route.route_long_name.as_deref().filter(|s| is_all_caps(s)) {
                let label = route.route_short_name.as_deref()
                    .filter(|s| !s.is_empty()).unwrap_or(route.route_id.as_str());
                notices.push(k6_notice(
                    ctr, "DQ_018", EntityType::Route,
                    Some(route.route_id.clone()), Some(route.route_id.clone()),
                    "routes.txt", Some(route.line), Some("route_long_name"),
                    Some(name.to_string()), None,
                    format!("'{}' hattının uzun adı tamamen büyük harf: '{name}'.", label),
                    "Hat adını düzgün harf kuralıyla yazın.",
                ));
            }
        }

        // trip_headsign
        for trip in &records.trips {
            if trip.trip_id.is_empty() { continue; }
            if let Some(hs) = trip.trip_headsign.as_deref().filter(|s| is_all_caps(s)) {
                notices.push(k6_notice(
                    ctr, "DQ_018", EntityType::Trip,
                    Some(trip.trip_id.clone()), Some(trip.trip_id.clone()),
                    "trips.txt", Some(trip.line), Some("trip_headsign"),
                    Some(hs.to_string()), None,
                    format!("'{}' seferinin yön adı tamamen büyük harf: '{hs}'.", trip.trip_id),
                    "Yön adını düzgün harf kuralıyla yazın.",
                ));
            }
        }

        // agency_name
        for ag in &records.agencies {
            if ag.agency_name.is_empty() { continue; }
            if is_all_caps(&ag.agency_name) {
                let label = ag.agency_id.as_deref()
                    .filter(|s| !s.is_empty())
                    .unwrap_or(ag.agency_name.as_str());
                notices.push(k6_notice(
                    ctr, "DQ_018", EntityType::Agency,
                    Some(label.to_string()), Some(label.to_string()),
                    "agency.txt", Some(ag.line), Some("agency_name"),
                    Some(ag.agency_name.clone()), None,
                    format!("'{}' işleticisinin adı tamamen büyük harf: '{}'.", label, ag.agency_name),
                    "İşletici adını düzgün harf kuralıyla yazın.",
                ));
            }
        }

        // stop_headsign (B4: tek-tarama'dan toplanan caps değerleri — aynı sıra/içerik/ctr)
        for (line, s) in &dq_caps_hs {
            notices.push(k6_notice(
                ctr, "DQ_018", EntityType::Row,
                None, None,
                "stop_times.txt", Some(*line), Some("stop_headsign"),
                Some(s.clone()), None,
                format!("stop_headsign değeri tamamen büyük harf: '{s}'."),
                "Yön adını düzgün harf kuralıyla yazın.",
            ));
        }

        // feed_publisher_name
        if let Some(fi) = records.feed_info.first() {
            if is_all_caps(&fi.feed_publisher_name) {
                notices.push(k6_notice(
                    ctr, "DQ_018", EntityType::Feed,
                    None, None,
                    "feed_info.txt", Some(fi.line), Some("feed_publisher_name"),
                    Some(fi.feed_publisher_name.clone()), None,
                    format!("feed_publisher_name tamamen büyük harf: '{}'.", fi.feed_publisher_name),
                    "Yayıncı adını düzgün harf kuralıyla yazın.",
                ));
            }
        }
    }

    // ── DQ_019: önerilen alanlarda tümü küçük harf (mixed_case_recommended_field) ──
    {
        let _t19 = Timer::start("K6::rem::dq_019");
        for stop in &records.stops {
            if stop.stop_id.is_empty() { continue; }
            if let Some(name) = stop.stop_name.as_deref().filter(|s| is_all_lower(s)) {
                notices.push(k6_notice(
                    ctr, "DQ_019", EntityType::Stop,
                    Some(stop.stop_id.clone()), Some(stop.stop_id.clone()),
                    "stops.txt", Some(stop.line), Some("stop_name"),
                    Some(name.to_string()), None,
                    format!("'{}' durağının adı tamamen küçük harf: '{name}'.", stop.stop_id),
                    "Durak adını başlık harfiyle yazın (ör. 'Merkez İstasyon').",
                ));
            }
        }
        for route in &records.routes {
            if route.route_id.is_empty() { continue; }
            if let Some(name) = route.route_long_name.as_deref().filter(|s| is_all_lower(s)) {
                notices.push(k6_notice(
                    ctr, "DQ_019", EntityType::Route,
                    Some(route.route_id.clone()), Some(route.route_id.clone()),
                    "routes.txt", Some(route.line), Some("route_long_name"),
                    Some(name.to_string()), None,
                    format!("'{}' hattının uzun adı tamamen küçük harf: '{name}'.", route.route_id),
                    "Hat adını başlık harfiyle yazın.",
                ));
            }
        }
        for trip in &records.trips {
            if trip.trip_id.is_empty() { continue; }
            if let Some(hs) = trip.trip_headsign.as_deref().filter(|s| is_all_lower(s)) {
                notices.push(k6_notice(
                    ctr, "DQ_019", EntityType::Trip,
                    Some(trip.trip_id.clone()), Some(trip.trip_id.clone()),
                    "trips.txt", Some(trip.line), Some("trip_headsign"),
                    Some(hs.to_string()), None,
                    format!("'{}' seferinin yön adı tamamen küçük harf: '{hs}'.", trip.trip_id),
                    "Yön adını başlık harfiyle yazın.",
                ));
            }
        }

        // agency_name
        for ag in &records.agencies {
            if ag.agency_name.is_empty() { continue; }
            if is_all_lower(&ag.agency_name) {
                let label = ag.agency_id.as_deref()
                    .filter(|s| !s.is_empty())
                    .unwrap_or(ag.agency_name.as_str());
                notices.push(k6_notice(
                    ctr, "DQ_019", EntityType::Agency,
                    Some(label.to_string()), Some(label.to_string()),
                    "agency.txt", Some(ag.line), Some("agency_name"),
                    Some(ag.agency_name.clone()), None,
                    format!("'{}' işleticisinin adı tamamen küçük harf: '{}'.", label, ag.agency_name),
                    "İşletici adını başlık harfiyle yazın.",
                ));
            }
        }

        // stop_headsign (B4: tek-tarama'dan toplanan lower değerleri — aynı sıra/içerik/ctr)
        for (line, s) in &dq_lower_hs {
            notices.push(k6_notice(
                ctr, "DQ_019", EntityType::Row,
                None, None,
                "stop_times.txt", Some(*line), Some("stop_headsign"),
                Some(s.clone()), None,
                format!("stop_headsign değeri tamamen küçük harf: '{s}'."),
                "Yön adını başlık harfiyle yazın.",
            ));
        }

        // feed_publisher_name
        if let Some(fi) = records.feed_info.first() {
            if is_all_lower(&fi.feed_publisher_name) {
                notices.push(k6_notice(
                    ctr, "DQ_019", EntityType::Feed,
                    None, None,
                    "feed_info.txt", Some(fi.line), Some("feed_publisher_name"),
                    Some(fi.feed_publisher_name.clone()), None,
                    format!("feed_publisher_name tamamen küçük harf: '{}'.", fi.feed_publisher_name),
                    "Yayıncı adını başlık harfiyle yazın.",
                ));
            }
        }
    }

    // ── DQ_020: önerilen alan eksik (missing_recommended_field) ──────────────
    {
        let _t20 = Timer::start("K6::rem::dq_020");
        // trip_headsign: GTFS spec'te önerilen alan
        for trip in &records.trips {
            if trip.trip_id.is_empty() { continue; }
            if trip.trip_headsign.as_deref().map(|s| s.trim().is_empty()).unwrap_or(true) {
                notices.push(k6_notice(
                    ctr, "DQ_020", EntityType::Trip,
                    Some(trip.trip_id.clone()), Some(trip.trip_id.clone()),
                    "trips.txt", Some(trip.line), Some("trip_headsign"),
                    None, None,
                    format!("'{}' seferinde trip_headsign eksik — yolcu bilgi sistemleri için önerilir.", trip.trip_id),
                    "trips.txt'e trip_headsign sütunu ekleyin.",
                ));
            }
        }
    }

    // ── OPR_009: sefer başlangıç saati 23:00 veya sonrası (gece seferi) ──────
    {
        let _t09 = Timer::start("K6::rem::opr_009");
        const LATE_NIGHT_SEC: u32 = 23 * 3600;
        for (&trip_id, stimes) in &idx.by_trip {
            if let Some(dep) = stimes.first().and_then(|s| s.departure_time) {
                let dep_sec = dep.0 * 3600 + dep.1 * 60 + dep.2;
                if dep_sec >= LATE_NIGHT_SEC {
                    let route = trip_to_route_rem.get(trip_id).copied().unwrap_or(trip_id);
                    let dep_str = format!("{:02}:{:02}", dep.0, dep.1);
                    notices.push(k6_notice(
                        ctr, "OPR_009", EntityType::Trip,
                        Some(trip_id.to_string()), Some(trip_id.to_string()),
                        "stop_times.txt", stimes.first().map(|s| s.line),
                        Some("departure_time"),
                        Some(dep_str.clone()), Some("< 23:00".to_string()),
                        format!("'{route}' hattında {dep_str} kalkışlı gece seferi var."),
                        "Bu bilgi notu; gece servisleri için beklenen bir durumdur.",
                    ));
                }
            }
        }
    }

    // ── OPR_017: sefer çok kısa mesafe (< 100 m) ─────────────────────────────
    {
        let _t17b = Timer::start("K6::rem::opr_017");
        const MIN_TRIP_KM: f64 = 0.1;
        for (&trip_id, stimes) in &idx.by_trip {
            let dist_km: Option<f64> = if let Some(&shape_id) = trip_shape_local.get(trip_id) {
                derived.shape_geometry.shapes.get(shape_id).map(|s| s.total_length_km)
            } else {
                stimes.first().zip(stimes.last()).and_then(|(first, last)| {
                    stop_coords.get(first.stop_id.as_str())
                        .zip(stop_coords.get(last.stop_id.as_str()))
                        .map(|(&(la1, lo1), &(la2, lo2))| haversine_km(la1, lo1, la2, lo2))
                })
            };
            if let Some(d) = dist_km {
                if d < MIN_TRIP_KM {
                    let route = trip_to_route_rem.get(trip_id).copied().unwrap_or(trip_id);
                    let dist_m = d * 1000.0;
                    notices.push(k6_notice(
                        ctr, "OPR_017", EntityType::Trip,
                        Some(trip_id.to_string()), Some(trip_id.to_string()),
                        "trips.txt", None, None,
                        Some(format!("{dist_m:.0}m")),
                        Some(format!("> {:.0}m", MIN_TRIP_KM * 1000.0)),
                        format!("'{route}' hattının seferi yalnızca {dist_m:.0}m mesafe kapsıyor.",),
                        "Sefer güzergahını veya güzergah şekli verilerini kontrol edin.",
                    ));
                }
            }
        }
    }

    // ── OPR_018: servis dönemi çok kısa (< 3 aktif gün) ─────────────────────
    {
        let _t18 = Timer::start("K6::rem::opr_018");
        const MIN_DAYS: usize = 3;
        let used_services_18: FxHashSet<&str> = records.trips.iter()
            .filter(|t| !t.service_id.is_empty())
            .map(|t| t.service_id.as_str())
            .collect();
        for (svc_id, dates) in &derived.calendar_bitmap.active_dates {
            if dates.is_empty() { continue; } // OPR_011 zaten yakaladı
            if dates.len() >= MIN_DAYS { continue; }
            if !used_services_18.contains(svc_id.as_str()) { continue; }
            notices.push(k6_notice(
                ctr, "OPR_018", EntityType::Service,
                Some(svc_id.clone()), Some(svc_id.clone()),
                "calendar.txt", None, None,
                Some(format!("{} aktif gün", dates.len())),
                Some(format!("≥ {MIN_DAYS} aktif gün")),
                format!("'{svc_id}' takviminde yalnızca {} aktif gün var.", dates.len()),
                "Servis dönemi planlamasını gözden geçirin; çok kısa servisler beklenmedik davranışlara yol açabilir.",
            ));
        }
    }

    // ── OPR_010: hatta erişilebilirlik / bisiklet politikası tutarsız ─────────
    {
        let _t10 = Timer::start("K6::rem::opr_010");
        // (has_accessible=1, has_not_accessible=2)
        let mut route_wc: FxHashMap<&str, (bool, bool)> = FxHashMap::default();
        let mut route_ba: FxHashMap<&str, (bool, bool)> = FxHashMap::default();
        for t in &records.trips {
            if t.route_id.is_empty() { continue; }
            if let Some(wc) = t.wheelchair_accessible {
                let e = route_wc.entry(t.route_id.as_str()).or_default();
                if wc == 1 { e.0 = true; }
                if wc == 2 { e.1 = true; }
            }
            if let Some(ba) = t.bikes_allowed {
                let e = route_ba.entry(t.route_id.as_str()).or_default();
                if ba == 1 { e.0 = true; }
                if ba == 2 { e.1 = true; }
            }
        }
        for (route_id, (has_acc, has_noacc)) in &route_wc {
            if *has_acc && *has_noacc {
                notices.push(k6_notice(
                    ctr, "OPR_010", EntityType::Route,
                    Some(route_id.to_string()), Some(route_id.to_string()),
                    "trips.txt", None, Some("wheelchair_accessible"),
                    None, None,
                    format!("'{route_id}' hattında bazı seferler tekerlekli sandalye erişimli (1), bazıları erişimsiz (2) olarak işaretlenmiş."),
                    "Aynı hattaki tüm seferlerin erişilebilirlik bilgisini tutarlı hâle getirin.",
                ));
            }
        }
        for (route_id, (has_allow, has_noallow)) in &route_ba {
            if *has_allow && *has_noallow {
                notices.push(k6_notice(
                    ctr, "OPR_010", EntityType::Route,
                    Some(route_id.to_string()), Some(route_id.to_string()),
                    "trips.txt", None, Some("bikes_allowed"),
                    None, None,
                    format!("'{route_id}' hattında bazı seferler bisiklete izin veriyor (1), bazıları vermiyor (2)."),
                    "Aynı hattaki tüm seferlerin bisiklet politikasını tutarlı hâle getirin.",
                ));
            }
        }
    }

    // ── OPR_014: feed genelinde ortalama aktarma süresi uzun ─────────────────
    {
        let _t14 = Timer::start("K6::rem::opr_014");
        const AVG_TRANSFER_THRESHOLD_SEC: u64 = 600; // 10 dakika
        let timed: Vec<u32> = records.transfers.iter()
            .filter(|t| t.transfer_type == Some(2))
            .filter_map(|t| t.min_transfer_time)
            .collect();
        if !timed.is_empty() {
            let count = timed.len();
            let avg = timed.iter().map(|&x| x as u64).sum::<u64>() / count as u64;
            if avg > AVG_TRANSFER_THRESHOLD_SEC {
                let avg_min = avg as f64 / 60.0;
                notices.push(k6_notice(
                    ctr, "OPR_014", EntityType::Feed,
                    None, None,
                    "transfers.txt", None, Some("min_transfer_time"),
                    Some(format!("{avg}s ortalama ({count} aktarma)")),
                    Some(format!("≤ {AVG_TRANSFER_THRESHOLD_SEC}s")),
                    format!("Feed genelinde {count} zamanlı aktarmanın ortalama min_transfer_time değeri {avg}s ({avg_min:.1} dakika)."),
                    "Aktarma sürelerini gözden geçirin; uzun aktarmalar yolcu deneyimini olumsuz etkiler.",
                ));
            }
        }
    }

    // ── TRF_011: aktarma noktaları arası mesafe çok uzak ─────────────────────
    {
        let _t11 = Timer::start("K6::rem::trf_011");
        const TRF_DIST_THRESHOLD_M: f64 = 2000.0;
        for trf in &records.transfers {
            if trf.from_stop_id.is_empty() || trf.to_stop_id == trf.from_stop_id {
                continue;
            }
            if let (Some(&(la1, lo1)), Some(&(la2, lo2))) = (
                stop_coords.get(trf.from_stop_id.as_str()),
                stop_coords.get(trf.to_stop_id.as_str()),
            ) {
                let dist_m = haversine_km(la1, lo1, la2, lo2) * 1000.0;
                if dist_m > TRF_DIST_THRESHOLD_M {
                    notices.push(k6_notice(
                        ctr, "TRF_011", EntityType::Transfer,
                        Some(format!("{}|{}", trf.from_stop_id, trf.to_stop_id)),
                        Some(format!("{}|{}", trf.from_stop_id, trf.to_stop_id)),
                        "transfers.txt", Some(trf.line),
                        Some("from_stop_id|to_stop_id"),
                        Some(format!("{dist_m:.0}m")),
                        Some(format!("≤ {TRF_DIST_THRESHOLD_M:.0}m")),
                        format!(
                            "'{}' → '{}' aktarması {dist_m:.0}m uzaklıkta — yürüyüş mesafesi {}m eşiğini aşıyor.",
                            trf.from_stop_id, trf.to_stop_id, TRF_DIST_THRESHOLD_M as u32
                        ),
                        "Aktarma tanımını gözden geçirin; çok uzak duraklar arasındaki aktarma yolcular için zorlayıcı olabilir.",
                    ));
                }
            }
        }
    }

    // ── SHP_015: şekil istatistiksel olarak çok az nokta ─────────────────────
    {
        let _t15 = Timer::start("K6::rem::shp_015");
        const MIN_POINTS_PER_10KM: f64 = 2.0; // en az 2 nokta / 10km
        for (shape_id, pts) in &shape_coords {
            if pts.len() < 3 {
                notices.push(k6_notice(
                    ctr, "SHP_015", EntityType::Shape,
                    Some(shape_id.to_string()), Some(shape_id.to_string()),
                    "shapes.txt", None, Some("shape_pt_lat|shape_pt_lon"),
                    Some(format!("{} nokta", pts.len())), Some("≥ 3 nokta".to_string()),
                    format!("'{shape_id}' güzergah şeklinde yalnızca {} nokta var — minimum 3 nokta gerekli.", pts.len()),
                    "Güzergah şekline daha fazla nokta ekleyin.",
                ));
                continue;
            }
            let total_km: f64 = pts.windows(2)
                .map(|w| haversine_km(w[0].0, w[0].1, w[1].0, w[1].1))
                .sum();
            if total_km > 1.0 {
                let density = pts.len() as f64 / (total_km / 10.0);
                if density < MIN_POINTS_PER_10KM {
                    notices.push(k6_notice(
                        ctr, "SHP_015", EntityType::Shape,
                        Some(shape_id.to_string()), Some(shape_id.to_string()),
                        "shapes.txt", None, Some("shape_pt_lat|shape_pt_lon"),
                        Some(format!("{} nokta / {total_km:.1}km", pts.len())),
                        Some(format!("≥ {MIN_POINTS_PER_10KM:.0} nokta/10km")),
                        format!(
                            "'{shape_id}' güzergahı {total_km:.1} km uzunluğunda ancak yalnızca {} nokta var — yoğunluk {density:.1} nokta/10km.",
                            pts.len()
                        ),
                        "Güzergah şekline ara noktalar ekleyin; düşük nokta yoğunluğu harita gösterimini bozar.",
                    ));
                }
            }
        }
    }

    // ── SHP_020: şekilde ardışık olmayan tekrarlayan nokta ────────────────────
    {
        let _t20b = Timer::start("K6::rem::shp_020");
        const DUP_THRESHOLD_DEG: f64 = 1e-6;
        for (shape_id, pts) in &shape_coords {
            // Sadece ardışık çiftleri değil, küçük bir pencere içinde kontrol et
            let mut fired = false;
            'outer: for i in 0..pts.len() {
                for j in (i + 2)..pts.len().min(i + 10) {
                    let dlat = (pts[i].0 - pts[j].0).abs();
                    let dlon = (pts[i].1 - pts[j].1).abs();
                    if dlat < DUP_THRESHOLD_DEG && dlon < DUP_THRESHOLD_DEG {
                        notices.push(k6_notice(
                            ctr, "SHP_020", EntityType::Shape,
                            Some(shape_id.to_string()), Some(shape_id.to_string()),
                            "shapes.txt", None, Some("shape_pt_lat|shape_pt_lon"),
                            Some(format!("nokta {i} ve {j}: ({:.6},{:.6})", pts[i].0, pts[i].1)),
                            None,
                            format!(
                                "'{shape_id}' güzergahında nokta {i} ile {j} neredeyse aynı konumda ({:.6},{:.6}) — tekrarlayan nokta.",
                                pts[i].0, pts[i].1
                            ),
                            "Güzergah şeklindeki tekrarlayan noktaları temizleyin.",
                        ));
                        fired = true;
                        break 'outer;
                    }
                }
            }
            let _ = fired;
        }
    }

    // ── SHP_009: güzergah şekli kendisiyle kesişiyor ──────────────────────────
    {
        let _t09b = Timer::start("K6::rem::shp_009");
        for (shape_id, pts) in &shape_coords {
            if pts.len() < 4 { continue; }
            // O(n²) segment-crossing: büyük shape'lerde maksimum 300 segment kontrol et
            let n = pts.len().min(301);
            let mut crossed = false;
            'seg_outer: for i in 0..n.saturating_sub(1) {
                for j in i + 2..n.saturating_sub(1) {
                    if i == 0 && j == n - 2 { continue; } // bitişik uçlar
                    if segments_cross(pts[i], pts[i+1], pts[j], pts[j+1]) {
                        notices.push(k6_notice(
                            ctr, "SHP_009", EntityType::Shape,
                            Some(shape_id.to_string()), Some(shape_id.to_string()),
                            "shapes.txt", None, Some("shape_pt_lat|shape_pt_lon"),
                            Some(format!("segment {i}-{} ∩ {j}-{}", i+1, j+1)), None,
                            format!(
                                "'{shape_id}' güzergah şekli segment {i}→{} ile segment {j}→{} arasında kendisiyle kesişiyor.",
                                i+1, j+1
                            ),
                            "Güzergah şeklindeki kesişen segmentleri düzeltin.",
                        ));
                        crossed = true;
                        break 'seg_outer;
                    }
                }
            }
            let _ = crossed;
        }
    }

}

/// SHP_012: güzergah şekli sefer duraklarından çok uzak.
/// check_remaining_analytics'ten ayrı bir K6 task'ı olarak çıkarıldı (paralel wall-clock'ta
/// uzun-kolu kısaltmak için). build_maps'teki shape_coords/shape_bbox/stop_coords KURULUMU
/// BİREBİR aynı (aynı sort_by_key, aynı bbox) → çıktı (notice sayısı/değeri/skor) özdeş.
/// Kendi yerel cache'leri (shape_stop_violations, dist_cache) var; başka kontrolle paylaşılan
/// mutable durum YOK → bağımsız task olarak güvenli, mevcut 13-task deseniyle aynı.
fn check_shp012(
    records: &EntityRecords,
    idx: &StopTimesIndex<'_>,
    notices: &mut Vec<Notice>,
    ctr: &mut u32,
) {
    // shape_id → sıralı (lat, lon) + bbox (build_maps ile birebir aynı kurulum)
    let mut shape_pts_unsorted: FxHashMap<&str, Vec<(u32, f64, f64)>> = FxHashMap::default();
    for sp in &records.shapes {
        if let (Some(lat), Some(lon)) = (sp.shape_pt_lat, sp.shape_pt_lon) {
            shape_pts_unsorted
                .entry(sp.shape_id.as_str())
                .or_default()
                .push((sp.shape_pt_sequence.unwrap_or(0), lat, lon));
        }
    }
    let n_shapes = shape_pts_unsorted.len();
    let mut shape_coords: FxHashMap<&str, Vec<(f64, f64)>> = FxHashMap::default();
    let mut shape_bbox: FxHashMap<&str, [f64; 4]> = FxHashMap::default();
    shape_coords.reserve(n_shapes);
    shape_bbox.reserve(n_shapes);
    for (sid, mut v) in shape_pts_unsorted {
        v.sort_by_key(|&(seq, _, _)| seq);
        let pts: Vec<(f64, f64)> = v.into_iter().map(|(_, la, lo)| (la, lo)).collect();
        if !pts.is_empty() {
            let mut mn_lat = pts[0].0;
            let mut mx_lat = pts[0].0;
            let mut mn_lon = pts[0].1;
            let mut mx_lon = pts[0].1;
            for &(la, lo) in pts.iter().skip(1) {
                if la < mn_lat { mn_lat = la; }
                if la > mx_lat { mx_lat = la; }
                if lo < mn_lon { mn_lon = lo; }
                if lo > mx_lon { mx_lon = lo; }
            }
            shape_bbox.insert(sid, [mn_lat, mx_lat, mn_lon, mx_lon]);
        }
        shape_coords.insert(sid, pts);
    }
    let stop_coords: FxHashMap<&str, (f64, f64)> = records
        .stops
        .iter()
        .filter_map(|s| s.stop_lat.zip(s.stop_lon).map(|c| (s.stop_id.as_str(), c)))
        .collect();

    // ── SHP_012 gövdesi (check_remaining_analytics'ten verbatim taşındı) ──────
    {
        let _t12b = crate::timing::Timer::start("K6::shp012::body");
        const SHP_STOP_THRESHOLD_M: f64 = 500.0;

        // Perf: (shape_id, stop_id) → polyline mesafesi MEMOIZE edilir. Aynı shape'i kullanan
        // onlarca sefer aynı (shape,durak) mesafesini tekrar tekrar hesaplıyordu; pahalı
        // point_to_polyline yalnızca BENZERSİZ çift başına bir kez çalışır. Sayım semantiği
        // (sefer-durak örneği başına artış) AYNI kalır → notice sayısı/değeri/skor DEĞİŞMEZ.
        let mut shape_stop_violations: FxHashMap<&str, u32> = FxHashMap::default();
        let mut dist_cache: FxHashMap<(&str, &str), f64> = FxHashMap::default();
        for trip in &records.trips {
            let Some(shape_id) = trip.shape_id.as_deref().filter(|s| !s.is_empty()) else { continue };
            let Some(pts) = shape_coords.get(shape_id) else { continue };
            let Some(stimes) = idx.by_trip.get(trip.trip_id.as_str()) else { continue };

            for st in stimes.iter() {
                let stop_id = st.stop_id.as_str();
                let Some(&(slat, slon)) = stop_coords.get(stop_id) else { continue };
                // Shape'e en yakın SEGMENT mesafesi (nokta-noktaya değil): seyrek shape
                // noktalarında iki nokta arasındaki duraklarda false-positive önlenir.
                let min_dist_m = *dist_cache
                    .entry((shape_id, stop_id))
                    .or_insert_with(|| {
                        // B3 bbox kısayolu (GEO_009 emsali): bbox+500m dışındaki durak kesinlikle
                        // >500m → tam polyline hesabı yerine clamped-corner haversine (>500m garantili)
                        // sakla. >threshold booleanı, viol_count ve mesaj birebir korunur.
                        if let Some(&[bmin_la, bmax_la, bmin_lo, bmax_lo]) = shape_bbox.get(shape_id) {
                            let cos_lat = slat.to_radians().cos();
                            let margin_lat = SHP_STOP_THRESHOLD_M / 111_320.0_f64;
                            let margin_lon = SHP_STOP_THRESHOLD_M / (111_320.0_f64 * cos_lat);
                            if slat < bmin_la - margin_lat || slat > bmax_la + margin_lat
                                || slon < bmin_lo - margin_lon || slon > bmax_lo + margin_lon {
                                let clat = slat.clamp(bmin_la, bmax_la);
                                let clon = slon.clamp(bmin_lo, bmax_lo);
                                return haversine_km(slat, slon, clat, clon) * 1000.0;
                            }
                        }
                        point_to_polyline_dist_m(slat, slon, pts)
                    });
                if min_dist_m > SHP_STOP_THRESHOLD_M {
                    *shape_stop_violations.entry(shape_id).or_insert(0) += 1;
                }
            }
        }

        for (shape_id, viol_count) in &shape_stop_violations {
            notices.push(k6_notice(
                ctr, "SHP_012", EntityType::Shape,
                Some(shape_id.to_string()), Some(shape_id.to_string()),
                "shapes.txt", None, Some("shape_pt_lat|shape_pt_lon"),
                Some(format!("{viol_count} durak > {SHP_STOP_THRESHOLD_M:.0}m")), None,
                format!(
                    "'{shape_id}' güzergah şekli {viol_count} duraktan >{SHP_STOP_THRESHOLD_M:.0}m uzakta — güzergah doğru çizilmemiş olabilir."
                ),
                "shapes.txt noktalarını durak konumlarına yaklaştırın.",
            ));
        }
    }
}

/// SHP_022: durak shape'te birden fazla eşleşme bölgesine yakın.
/// check_remaining_analytics'ten ayrı bir K6 task'ı olarak çıkarıldı (remaining'in en ağır
/// alt-kontrolü ~126ms; paralel wall-clock'ta uzun-kolu kısaltır). Kullandığı map'ler
/// (shape_coords/shape_bbox/stop_coords/stop_names/trip_shape_local) build_maps ile BİREBİR
/// aynı kurulum; shape_cum yerel olarak yeniden kurulur (sadece memoization — aynı değerler).
/// Dedup set'leri (shp022_seen/shp022_done) yerel → bağımsız task olarak güvenli, çıktı özdeş.
fn check_shp022(
    records: &EntityRecords,
    idx: &StopTimesIndex<'_>,
    notices: &mut Vec<Notice>,
    ctr: &mut u32,
) {
    use crate::timing::Timer;

    // ── build_maps ile birebir aynı kurulum (shape_coords + shape_bbox) ──────
    let mut shape_pts_unsorted: FxHashMap<&str, Vec<(u32, f64, f64)>> = FxHashMap::default();
    for sp in &records.shapes {
        if let (Some(lat), Some(lon)) = (sp.shape_pt_lat, sp.shape_pt_lon) {
            shape_pts_unsorted
                .entry(sp.shape_id.as_str())
                .or_default()
                .push((sp.shape_pt_sequence.unwrap_or(0), lat, lon));
        }
    }
    let n_shapes = shape_pts_unsorted.len();
    let mut shape_coords: FxHashMap<&str, Vec<(f64, f64)>> = FxHashMap::default();
    let mut shape_bbox: FxHashMap<&str, [f64; 4]> = FxHashMap::default();
    shape_coords.reserve(n_shapes);
    shape_bbox.reserve(n_shapes);
    for (sid, mut v) in shape_pts_unsorted {
        v.sort_by_key(|&(seq, _, _)| seq);
        let pts: Vec<(f64, f64)> = v.into_iter().map(|(_, la, lo)| (la, lo)).collect();
        if !pts.is_empty() {
            let mut mn_lat = pts[0].0;
            let mut mx_lat = pts[0].0;
            let mut mn_lon = pts[0].1;
            let mut mx_lon = pts[0].1;
            for &(la, lo) in pts.iter().skip(1) {
                if la < mn_lat { mn_lat = la; }
                if la > mx_lat { mx_lat = la; }
                if lo < mn_lon { mn_lon = lo; }
                if lo > mx_lon { mx_lon = lo; }
            }
            shape_bbox.insert(sid, [mn_lat, mx_lat, mn_lon, mx_lon]);
        }
        shape_coords.insert(sid, pts);
    }
    let stop_coords: FxHashMap<&str, (f64, f64)> = records
        .stops
        .iter()
        .filter_map(|s| s.stop_lat.zip(s.stop_lon).map(|c| (s.stop_id.as_str(), c)))
        .collect();
    let stop_names: FxHashMap<&str, &str> = records
        .stops
        .iter()
        .filter_map(|s| s.stop_name.as_deref().map(|n| (s.stop_id.as_str(), n)))
        .collect();
    let trip_shape_local: HashMap<&str, &str> = records
        .trips
        .iter()
        .filter_map(|t| t.shape_id.as_deref().map(|s| (t.trip_id.as_str(), s)))
        .collect();
    let mut shape_cum: FxHashMap<&str, Vec<f64>> = FxHashMap::default();

    // ── SHP_022 gövdesi (check_remaining_analytics'ten verbatim taşındı) ─────
    {
        let _t22 = Timer::start("K6::shp022::body");
        const MATCH_KM: f64 = 0.150;       // 150 m — eşleşme eşiği
        const SEP_KM:   f64 = 0.500;       // 500 m — iki cluster arası min arc fark

        let mut shp022_seen: FxHashSet<(&str, &str)> = FxHashSet::default();
        // B2 perf: (shape,stop) küme kararı saf fonksiyon — aynı çifti paylaşan onlarca
        // sdt-eksik sefer için segment taramasını tekrarlama. shp022_seen yalnız EMİSYON
        // dedup'u; bu set DEĞERLENDİRME dedup'u → çıktı birebir aynı.
        let mut shp022_done: FxHashSet<(&str, &str)> = FxHashSet::default();

        for (trip_id, stimes) in &idx.by_trip {
            // Sadece shape_dist_traveled eksik trippler
            if !idx.trips_missing_sdt.contains_key(trip_id) { continue; }
            let Some(&shape_id) = trip_shape_local.get(trip_id) else { continue };
            let Some(pts) = shape_coords.get(shape_id) else { continue };
            if pts.len() < 2 { continue; }

            let cum = shape_cum.entry(shape_id).or_insert_with(|| {
                let mut c = Vec::with_capacity(pts.len());
                c.push(0.0_f64);
                for i in 1..pts.len() {
                    c.push(c[i-1] + haversine_km(pts[i-1].0, pts[i-1].1, pts[i].0, pts[i].1));
                }
                c
            });
            let cos_lat = (pts[0].0.to_radians()).cos().max(0.001_f64);
            let scale_lon = 111.0_f64 * cos_lat;
            let match_sq = MATCH_KM * MATCH_KM;

            for st in stimes.iter() {
                let stop_id = st.stop_id.as_str();
                if shp022_seen.contains(&(shape_id, stop_id)) { continue; }
                if !shp022_done.insert((shape_id, stop_id)) { continue; }
                let Some(&(slat, slon)) = stop_coords.get(stop_id) else { continue };

                // B2 bbox ön-filtresi (GEO_009 emsali, satır ~3323): bbox+MATCH_KM dışındaki
                // durak hiçbir segmente MATCH_KM kadar yakın olamaz → close_arcs boş → notice yok.
                if let Some(&[bmin_la, bmax_la, bmin_lo, bmax_lo]) = shape_bbox.get(shape_id) {
                    let margin_lat = MATCH_KM / 111.0_f64;
                    let margin_lon = MATCH_KM / scale_lon;
                    if slat < bmin_la - margin_lat || slat > bmax_la + margin_lat
                        || slon < bmin_lo - margin_lon || slon > bmax_lo + margin_lon {
                        continue;
                    }
                }

                // Her segment için minimum mesafeyi hesapla, MATCH_KM altındakileri kaydet
                let mut close_arcs: Vec<f64> = Vec::new();
                for w in 0..pts.len() - 1 {
                    let (alat, alon) = pts[w];
                    let (blat, blon) = pts[w + 1];
                    let ax = (alon - slon) * scale_lon;
                    let ay = (alat - slat) * 111.0_f64;
                    let bx = (blon - slon) * scale_lon;
                    let by_ = (blat - slat) * 111.0_f64;
                    let dx = bx - ax; let dy = by_ - ay;
                    let len_sq = dx * dx + dy * dy;
                    let t = if len_sq < 1e-12 { 0.0_f64 } else {
                        ((-ax * dx) + (-ay * dy)) / len_sq
                    }.clamp(0.0_f64, 1.0_f64);
                    let nx = ax + t * dx; let ny = ay + t * dy;
                    let dsq = nx * nx + ny * ny;
                    if dsq <= match_sq {
                        close_arcs.push(cum[w] + t * (cum[w + 1] - cum[w]));
                    }
                }
                if close_arcs.is_empty() { continue; }

                // Cluster sayısını bul (art arda gelmeyen arc grupları)
                close_arcs.sort_by(|a, b| a.total_cmp(b));
                let mut clusters = 1usize;
                let mut prev = close_arcs[0];
                for &arc in &close_arcs[1..] {
                    if arc - prev > SEP_KM { clusters += 1; }
                    prev = arc;
                }
                if clusters < 2 { continue; }

                shp022_seen.insert((shape_id, stop_id));
                let sname = stop_names.get(stop_id).copied().unwrap_or(stop_id);
                let mut notice = k6_notice(
                    ctr,
                    "SHP_022",
                    EntityType::Stop,
                    Some(stop_id.to_string()),
                    Some(stop_id.to_string()),
                    "stop_times.txt",
                    Some(st.line),
                    Some("stop_id"),
                    Some(stop_id.to_string()),
                    None,
                    format!(
                        "'{}' (kod: '{}') durağı '{}' güzergah şeklinin {} ayrı bölümüne yakın — \
                         shape_dist_traveled eksikken hangi bölümle eşleşeceği belirsiz.",
                        sname, stop_id, shape_id, clusters
                    ),
                    "stop_times'a shape_dist_traveled ekleyerek durağın şekil üzerindeki \
                     konumunu açıkça belirtin.",
                );
                let mut det = HashMap::new();
                det.insert("shape_id".to_string(), shape_id.to_string());
                notice.details = Some(det);
                notices.push(notice);
            }
        }
    }
}

/// İki 2D çizgi segmentinin kesişip kesişmediğini kontrol eder (çapraz çarpım yöntemi).
fn segments_cross(a: (f64, f64), b: (f64, f64), c: (f64, f64), d: (f64, f64)) -> bool {
    let cross = |o: (f64, f64), p: (f64, f64), q: (f64, f64)| -> f64 {
        (p.0 - o.0) * (q.1 - o.1) - (p.1 - o.1) * (q.0 - o.0)
    };
    let d1 = cross(c, d, a);
    let d2 = cross(c, d, b);
    let d3 = cross(a, b, c);
    let d4 = cross(a, b, d);
    if d1 * d2 < 0.0 && d3 * d4 < 0.0 {
        return true;
    }
    false
}

/// Yaklaşık Julian Day Number hesabı (gap/expiry karşılaştırması için yeterli).
/// k5_derived::ymd_to_jdn ile aynı formül; burada u32 döndürür.
fn yyyymmdd_to_approx_jdn(yyyymmdd: u32) -> u32 {
    let y = yyyymmdd / 10000;
    let m = (yyyymmdd / 100) % 100;
    let d = yyyymmdd % 100;
    if y == 0 || m == 0 || d == 0 {
        return 0;
    }
    let a = (14u32.wrapping_sub(m)) / 12;
    let yr = y + 4800 - a;
    let mo = m + 12 * a - 3;
    d + (153 * mo + 2) / 5 + 365 * yr + yr / 4 - yr / 100 + yr / 400 - 32045
}

/// Julian Day Number → YYYYMMDD (yyyymmdd_to_approx_jdn'in tersi).
fn jdn_to_yyyymmdd_k6(jdn: u32) -> u32 {
    let a = jdn + 32044;
    let b = (4 * a + 3) / 146097;
    let c = a - (146097 * b) / 4;
    let d = (4 * c + 3) / 1461;
    let e = c - (1461 * d) / 4;
    let m = (5 * e + 2) / 153;
    let day = e - (153 * m + 2) / 5 + 1;
    let month = m + 3 - 12 * (m / 10);
    let year = 100 * b + d - 4800 + m / 10;
    year * 10000 + month * 100 + day
}

fn format_hms(total_secs: u32) -> String {
    let h = total_secs / 3600;
    let m = (total_secs % 3600) / 60;
    let s = total_secs % 60;
    format!("{h:02}:{m:02}:{s:02}")
}

// ── Takvim override analitik (OPR_019–023) ───────────────────────────────────

fn check_calendar_override_analytics(
    records: &EntityRecords,
    derived: &DerivedData,
    config: &ValidatorConfig,
    notices: &mut Vec<Notice>,
    ctr: &mut u32,
) {
    use crate::timing::Timer;
    // ── route_id → service_id kümesi ─────────────────────────────────────────
    let mut route_services: HashMap<&str, Vec<&str>> = HashMap::new();
    for trip in &records.trips {
        if !trip.route_id.is_empty() && !trip.service_id.is_empty() {
            route_services
                .entry(trip.route_id.as_str())
                .or_default()
                .push(trip.service_id.as_str());
        }
    }
    // Servis listelerini sırala ve dedup et
    for svcs in route_services.values_mut() {
        svcs.sort_unstable();
        svcs.dedup();
    }

    // ── calendar_dates'te exception kaydı olan (service_id, date) çiftleri ──
    // OPR_020: conflict günü, bu set'te herhangi bir çakışan servis varsa HIGH
    let exception_service_dates: FxHashSet<(&str, u32)> = records
        .calendar_dates
        .iter()
        .filter_map(|cd| {
            cd.date.map(|(y, m, d)| {
                (cd.service_id.as_str(), y * 10000 + m * 100 + d)
            })
        })
        .collect();

    // ── OPR_019 / OPR_020: config gerektirmeyen çakışma analizi ─────────────
    let mut date_services: HashMap<u32, Vec<&str>> = HashMap::new();

    for (&route_id, service_ids) in &route_services {
        if service_ids.len() < 2 {
            // Tek servis → çakışma mümkün değil
            continue;
        }

        date_services.clear();
        for &svc_id in service_ids {
            if let Some(dates) = derived.calendar_bitmap.active_dates.get(svc_id) {
                for &date in dates {
                    date_services.entry(date).or_default().push(svc_id);
                }
            }
        }

        // Çakışmalı günleri sırala
        let mut plain_dates: Vec<u32> = Vec::new();   // exception yok
        let mut exc_dates: Vec<u32> = Vec::new();     // en az bir servis exception günü
        let mut plain_patterns: Vec<String> = Vec::new();
        let mut exc_patterns: Vec<String> = Vec::new();

        for (&date, svcs) in &date_services {
            if svcs.len() < 2 {
                continue;
            }
            let has_exc = svcs.iter().any(|&s| exception_service_dates.contains(&(s, date)));
            let mut sorted_svcs = svcs.clone();
            sorted_svcs.sort_unstable();
            let pattern = sorted_svcs.join("+");
            if has_exc {
                exc_dates.push(date);
                exc_patterns.push(pattern);
            } else {
                plain_dates.push(date);
                plain_patterns.push(pattern);
            }
        }

        if !plain_dates.is_empty() {
            plain_dates.sort_unstable();
            let count = plain_dates.len();
            let sample_str: String = plain_dates.iter().take(3)
                .map(|d| d.to_string())
                .collect::<Vec<_>>()
                .join(", ");
            let patterns_str = summarize_patterns(&plain_patterns);
            let msg = format!(
                "'{}' hattında {} günde birden fazla aktif servis var. Örnek tarihler: {}.",
                route_id, count, sample_str
            );
            let mut n = k6_notice(
                ctr, "OPR_019", EntityType::Route,
                Some(route_id.to_string()), Some(route_id.to_string()),
                "trips.txt", None, None,
                Some(format!("{count} çakışma günü")),
                Some("gün başına 1 aktif servis".to_string()),
                msg,
                "Her takvim günü için yalnızca bir servis aktif olacak şekilde düzenleyin.",
            );
            n.details = Some({
                let mut d = std::collections::HashMap::new();
                d.insert("conflict_day_count".to_string(), count.to_string());
                d.insert("sample_dates".to_string(),
                    plain_dates.iter().take(10).map(|x| x.to_string()).collect::<Vec<_>>().join(","));
                d.insert("active_service_patterns".to_string(), patterns_str);
                d
            });
            notices.push(n);
        }

        if !exc_dates.is_empty() {
            exc_dates.sort_unstable();
            let count = exc_dates.len();
            let sample_str: String = exc_dates.iter().take(3)
                .map(|d| d.to_string())
                .collect::<Vec<_>>()
                .join(", ");
            let patterns_str = summarize_patterns(&exc_patterns);
            let msg = format!(
                "'{}' hattında {} exception gününde birden fazla aktif servis var — override çakışması riski. Örnek tarihler: {}.",
                route_id, count, sample_str
            );
            let mut n = k6_notice(
                ctr, "OPR_020", EntityType::Route,
                Some(route_id.to_string()), Some(route_id.to_string()),
                "trips.txt", None, None,
                Some(format!("{count} exception çakışma günü")),
                Some("exception gününde tek aktif servis".to_string()),
                msg,
                "Override günlerinde base servisi calendar_dates.txt ile kaldırın (exception_type=2).",
            );
            n.details = Some({
                let mut d = std::collections::HashMap::new();
                d.insert("conflict_day_count".to_string(), count.to_string());
                d.insert("sample_dates".to_string(),
                    exc_dates.iter().take(10).map(|x| x.to_string()).collect::<Vec<_>>().join(","));
                d.insert("active_service_patterns".to_string(), patterns_str);
                d
            });
            notices.push(n);
        }
    }

    // ── OPR_021 / OPR_022 / OPR_023: config-only ─────────────────────────────
    for rule in &config.calendar_override_rules {
        let base_svc_set: FxHashSet<&str> = rule.base_service_ids.iter().map(|s| s.as_str()).collect();
        let override_svc_set: FxHashSet<&str> = rule.override_service_ids.iter().map(|s| s.as_str()).collect();

        let start_jdn = yyyymmdd_to_approx_jdn(rule.start_date);
        let end_jdn = yyyymmdd_to_approx_jdn(rule.end_date);

        let mut dates_021: Vec<u32> = Vec::new();
        let mut dates_022: Vec<u32> = Vec::new();
        let mut dates_023: Vec<u32> = Vec::new();

        for jdn in start_jdn..=end_jdn {
            let date = jdn_to_yyyymmdd_k6(jdn);
            let base_active = base_svc_set.iter().any(|&s| {
                derived.calendar_bitmap.active_dates.get(s)
                    .is_some_and(|set| set.contains(&date))
            });
            let override_active = override_svc_set.iter().any(|&s| {
                derived.calendar_bitmap.active_dates.get(s)
                    .is_some_and(|set| set.contains(&date))
            });
            match (base_active, override_active) {
                (true, true)   => dates_021.push(date),
                (true, false)  => dates_022.push(date),
                (false, false) => dates_023.push(date),
                (false, true)  => {} // doğru: base kaldırılmış, override eklenmiş
            }
        }

        let base_str = rule.base_service_ids.join(",");
        let override_str = rule.override_service_ids.join(",");

        if !dates_021.is_empty() {
            let count = dates_021.len();
            let msg = format!(
                "'{}' hattında {} günde override servisi ({}) aktif ancak base servisi ({}) de aktif kalmış. İlk gün: {}.",
                rule.route_id, count, override_str, base_str, dates_021[0]
            );
            let mut n = k6_notice(
                ctr, "OPR_021", EntityType::Route,
                Some(rule.route_id.clone()), Some(rule.route_id.clone()),
                "calendar_dates.txt", None, None, None, None, msg,
                "Override gününde base servisi calendar_dates.txt ile kaldırın (exception_type=2).",
            );
            n.details = Some(build_override_details(count, &dates_021, &base_str, &override_str));
            notices.push(n);
        }

        if !dates_022.is_empty() {
            let count = dates_022.len();
            let msg = format!(
                "'{}' hattında {} günde override servisi ({}) bekleniyor ancak aktif değil; base servis ({}) çalışıyor. İlk gün: {}.",
                rule.route_id, count, override_str, base_str, dates_022[0]
            );
            let mut n = k6_notice(
                ctr, "OPR_022", EntityType::Route,
                Some(rule.route_id.clone()), Some(rule.route_id.clone()),
                "calendar_dates.txt", None, None, None, None, msg,
                "Override günü için override servisini calendar_dates.txt ile ekleyin (exception_type=1).",
            );
            n.details = Some(build_override_details(count, &dates_022, &base_str, &override_str));
            notices.push(n);
        }

        if !dates_023.is_empty() {
            let count = dates_023.len();
            let msg = format!(
                "'{}' hattında {} günde ne base ({}) ne override ({}) servisi aktif — hat servissiz kalmış. İlk gün: {}.",
                rule.route_id, count, base_str, override_str, dates_023[0]
            );
            let mut n = k6_notice(
                ctr, "OPR_023", EntityType::Route,
                Some(rule.route_id.clone()), Some(rule.route_id.clone()),
                "calendar_dates.txt", None, None, None, None, msg,
                "Override penceresinde hem base hem override servisini aktif tutun.",
            );
            n.details = Some(build_override_details(count, &dates_023, &base_str, &override_str));
            notices.push(n);
        }
    }

    // ── OPR_004: hatta hafta sonu sefer yok ──────────────────────────────────
    if !derived.calendar_bitmap.active_dates.is_empty() {
        let _t04 = Timer::start("K6::rem::opr_004");
        let mut route_services: FxHashMap<&str, FxHashSet<&str>> = FxHashMap::default();
        for t in &records.trips {
            if !t.route_id.is_empty() && !t.service_id.is_empty() {
                route_services.entry(t.route_id.as_str()).or_default().insert(t.service_id.as_str());
            }
        }
        for (route_id, service_ids) in &route_services {
            let has_weekend = service_ids.iter().any(|&svc| {
                derived.calendar_bitmap.active_dates.get(svc)
                    .is_some_and(|dates| dates.iter().any(|&d| {
                        let jdn = yyyymmdd_to_approx_jdn(d);
                        jdn % 7 == 5 || jdn % 7 == 6 // Cumartesi=5, Pazar=6
                    }))
            });
            if !has_weekend {
                notices.push(k6_notice(
                    ctr, "OPR_004", EntityType::Route,
                    Some(route_id.to_string()), Some(route_id.to_string()),
                    "trips.txt", None, None, None, None,
                    format!("'{route_id}' hattının aktif servislerinde hafta sonu (Cumartesi/Pazar) günü yok."),
                    "Bu bilgi notudur; aksiyon gerekmez.",
                ));
            }
        }
    }

    // ── OPR_012: servis döneminde büyük boşluk ───────────────────────────────
    {
        let _t12 = Timer::start("K6::rem::opr_012");
        let gap_threshold = config.service_gap_days;
        let used_services_12: FxHashSet<&str> = records.trips.iter()
            .filter(|t| !t.service_id.is_empty())
            .map(|t| t.service_id.as_str())
            .collect();
        for (svc_id, dates) in &derived.calendar_bitmap.active_dates {
            if dates.len() < 2 { continue; }
            if !used_services_12.contains(svc_id.as_str()) { continue; }
            let mut sorted: Vec<u32> = dates.iter().copied().collect();
            sorted.sort_unstable();
            let mut max_gap = 0u32;
            let mut gap_start = 0u32;
            let mut gap_end = 0u32;
            for w in sorted.windows(2) {
                // CAL_007 ile aynı sayım: iki tarih arasındaki eksik günler (dışlayıcı)
                let d = yyyymmdd_to_approx_jdn(w[1])
                    .saturating_sub(yyyymmdd_to_approx_jdn(w[0]))
                    .saturating_sub(1);
                if d > max_gap { max_gap = d; gap_start = w[0]; gap_end = w[1]; }
            }
            if max_gap >= gap_threshold {
                notices.push(k6_notice(
                    ctr, "OPR_012", EntityType::Service,
                    Some(svc_id.clone()), Some(svc_id.clone()),
                    "calendar.txt", None, None,
                    Some(format!("{max_gap} gün")),
                    Some(format!("≤ {gap_threshold} gün")),
                    format!("'{svc_id}' servisinde {gap_start}-{gap_end} arasında {max_gap} günlük servis boşluğu var."),
                    "Boşluk planlanmışsa görmezden gelin; aksi hâlde eksik günleri calendar_dates.txt ile ekleyin.",
                ));
            }
        }
    }

    // ── OPR_015: hatta yalnızca tek shape tanımlı ─────────────────────────────
    {
        let _t15b = Timer::start("K6::rem::opr_015");
        let mut route_shape_set: FxHashMap<&str, FxHashSet<&str>> = FxHashMap::default();
        for t in &records.trips {
            if t.route_id.is_empty() { continue; }
            if let Some(shape) = t.shape_id.as_deref().filter(|s| !s.is_empty()) {
                route_shape_set.entry(t.route_id.as_str()).or_default().insert(shape);
            }
        }
        // route_id → route_type (raylı sistemler için skip)
        let rt_map: FxHashMap<&str, u32> = records.routes.iter()
            .filter_map(|r| r.route_type.map(|rt| (r.route_id.as_str(), rt)))
            .collect();
        for (route_id, shapes) in &route_shape_set {
            // Tram(0), Metro(1), Demiryolu(2), Kablo tramvay(5), Monoray(12):
            // tek yönlü ray hattı; tek shape beklenen davranış
            let rt = rt_map.get(*route_id).copied().unwrap_or(3);
            if matches!(rt, 0 | 1 | 2 | 5 | 12) { continue; }
            if shapes.len() == 1 {
                let shape_id = shapes.iter().next().copied().unwrap_or("");
                let mut n015 = k6_notice(
                    ctr, "OPR_015", EntityType::Route,
                    Some(route_id.to_string()), Some(route_id.to_string()),
                    "trips.txt", None, Some("shape_id"),
                    Some(format!("1 shape ({shape_id})")), None,
                    format!("'{route_id}' hattının tüm seferleri tek bir güzergah şekli kullanıyor ({shape_id})."),
                    "Bu bilgi notu; gidiş-dönüş için ayrı shape_id kullanılması önerilir.",
                );
                let mut d = std::collections::HashMap::new();
                d.insert("shape_id".to_string(), shape_id.to_string());
                n015.details = Some(d);
                notices.push(n015);
            }
        }
    }
}

/// Unique servis kombinasyonlarını say ve özetle: "SVC_A+SVC_B: 12 gün; SVC_A+SVC_C: 3 gün"
fn summarize_patterns(patterns: &[String]) -> String {
    let mut counts: std::collections::BTreeMap<&str, usize> = std::collections::BTreeMap::new();
    for p in patterns {
        *counts.entry(p.as_str()).or_default() += 1;
    }
    counts.iter()
        .map(|(k, &v)| format!("{}: {} gün", k, v))
        .collect::<Vec<_>>()
        .join("; ")
}

/// OPR_021/022/023 için details HashMap'i oluşturur.
fn build_override_details(
    count: usize,
    dates: &[u32],
    base_str: &str,
    override_str: &str,
) -> std::collections::HashMap<String, String> {
    let mut d = std::collections::HashMap::new();
    d.insert("conflict_day_count".to_string(), count.to_string());
    d.insert("sample_dates".to_string(),
        dates.iter().take(10).map(|x| x.to_string()).collect::<Vec<_>>().join(","));
    d.insert("base_services".to_string(), base_str.to_string());
    d.insert("override_services".to_string(), override_str.to_string());
    d
}

// ── WP-14d: Pathway analitik (PTH_012 / PTH_013 / PTH_015) ──────────────────

fn check_pathway_analytics(
    records: &EntityRecords,
    derived: &DerivedData,
    notices: &mut Vec<Notice>,
    ctr: &mut u32,
) {
    if records.pathways.is_empty() {
        return;
    }

    // stop_id → parent_station
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

    // İstasyon (location_type=1) → alt duraklar
    let mut station_children: HashMap<&str, Vec<&crate::k2::stops::StopRecord>> = HashMap::new();
    for stop in &records.stops {
        if let Some(&parent) = stop_parent.get(stop.stop_id.as_str()) {
            station_children.entry(parent).or_default().push(stop);
        }
    }

    // Pathway graph'ta yer alan duraklar
    let graph_stops: HashSet<&str> = derived
        .pathway_graph
        .adjacency
        .keys()
        .map(String::as_str)
        .collect();

    // Wheelchair-accessible pathway indeksleri:
    // max_slope ≤ 8% (0.08) VE min_width ≥ 0.9 m — ikisi de None ise geçer
    let accessible_pw_idx: HashSet<usize> = records
        .pathways
        .iter()
        .enumerate()
        .filter(|(_, pw)| {
            pw.max_slope.map_or(true, |s| s.abs() <= 0.08)
                && pw.min_width.map_or(true, |w| w >= 0.9)
        })
        .map(|(i, _)| i)
        .collect();

    // ── PTH_012 ve PTH_013: her istasyon için ────────────────────────────────
    for station in records.stops.iter().filter(|s| s.location_type == Some(1)) {
        let station_id = station.stop_id.as_str();
        if station_id.is_empty() {
            continue;
        }
        let children = match station_children.get(station_id) {
            Some(c) if !c.is_empty() => c,
            _ => continue,
        };

        // Bu istasyona ait pathway bağlantısı var mı?
        let any_connected = children
            .iter()
            .any(|s| graph_stops.contains(s.stop_id.as_str()));
        if !any_connected {
            continue;
        }

        let entrances: Vec<&str> = children
            .iter()
            .filter(|s| s.location_type == Some(2))
            .map(|s| s.stop_id.as_str())
            .collect();
        let platforms: Vec<&str> = children
            .iter()
            .filter(|s| matches!(s.location_type, None | Some(0) | Some(4)))
            .map(|s| s.stop_id.as_str())
            .collect();

        if platforms.is_empty() {
            continue;
        }

        // Entrance yoksa → pathway grafiği olan tüm platformlar entrance'sız erişilemez
        if entrances.is_empty() {
            for &platform in &platforms {
                if graph_stops.contains(platform) {
                    notices.push(k6_notice(
                        ctr,
                        "PTH_012",
                        EntityType::Stop,
                        Some(platform.to_string()),
                        Some(platform.to_string()),
                        "stops.txt",
                        None,
                        None,
                        None,
                        None,
                        format!(
                            "Platform '{platform}' (istasyon '{station_id}') pathway ağında tanımlanmış ama istasyonda location_type=2 entrance yok.",
                        ),
                        "İstasyona giriş noktası (location_type=2) ekleyin ve pathway bağlantılarını tamamlayın.",
                    ));
                }
            }
            continue;
        }

        // Entrance kümesinden BFS ile erişilebilen tüm durakları bul (tam graf)
        let reachable_from_entrances: HashSet<&str> = {
            let mut visited: HashSet<&str> = HashSet::new();
            let mut queue = std::collections::VecDeque::new();
            for &e in &entrances {
                if visited.insert(e) {
                    queue.push_back(e);
                }
            }
            while let Some(curr) = queue.pop_front() {
                if let Some(neighbors) = derived.pathway_graph.adjacency.get(curr) {
                    for (next, _) in neighbors {
                        if visited.insert(next.as_str()) {
                            queue.push_back(next.as_str());
                        }
                    }
                }
            }
            visited
        };

        // PTH_012: her platform için en az bir entrance'tan erişilebilir olmalı
        for &platform in &platforms {
            if !reachable_from_entrances.contains(platform) {
                notices.push(k6_notice(
                    ctr,
                    "PTH_012",
                    EntityType::Stop,
                    Some(platform.to_string()),
                    Some(platform.to_string()),
                    "pathways.txt",
                    None,
                    None,
                    None,
                    None,
                    format!(
                        "Platform '{platform}' (istasyon '{station_id}') hiçbir entrance'tan (location_type=2) pathway graph üzerinden ulaşılamıyor.",
                    ),
                    "Entrance ile bu platform arasındaki pathway bağlantılarını tamamlayın.",
                ));
            }
        }

        // PTH_013: en az bir entrance-to-platform path'inde tüm pathway'ler
        // max_slope ≤ 8% VE min_width ≥ 0.9 m (tekerlekli sandalye erişim testi)
        let accessible_reachable: HashSet<&str> = {
            let mut visited: HashSet<&str> = HashSet::new();
            let mut queue = std::collections::VecDeque::new();
            for &e in &entrances {
                if visited.insert(e) {
                    queue.push_back(e);
                }
            }
            while let Some(curr) = queue.pop_front() {
                if let Some(neighbors) = derived.pathway_graph.adjacency.get(curr) {
                    for (next, pw_idx) in neighbors {
                        if accessible_pw_idx.contains(pw_idx) && visited.insert(next.as_str()) {
                            queue.push_back(next.as_str());
                        }
                    }
                }
            }
            visited
        };

        let any_platform_accessible = platforms
            .iter()
            .any(|p| accessible_reachable.contains(p));

        if !any_platform_accessible {
            notices.push(k6_notice(
                ctr,
                "PTH_013",
                EntityType::Stop,
                Some(station_id.to_string()),
                Some(station_id.to_string()),
                "pathways.txt",
                None,
                None,
                Some("max_slope > 8% veya min_width < 0.9m".to_string()),
                Some("max_slope ≤ 8% ve min_width ≥ 0.9m olan en az 1 path".to_string()),
                format!(
                    "İstasyon '{station_id}' — entrance'tan platforma tekerlekli sandalye ile erişilebilir pathway rotası yok (tüm pathlarda max_slope > 8% veya min_width < 0.9m).",
                ),
                "Accessibility koşullarını (max_slope ≤ 0.08, min_width ≥ 0.9) sağlayan bir entrance-to-platform rotası oluşturun.",
            ));
        }
    }

    // ── PTH_015: traversal_time ile length tutarlılığı (makul hız) ───────────
    // Hız = length (m) / traversal_time (s); > 3.0 m/s → tutarsız
    const MAX_PATHWAY_SPEED_MS: f64 = 3.0;

    for pw in &records.pathways {
        if pw.pathway_id.is_empty() {
            continue;
        }
        if let (Some(length), Some(tt)) = (pw.length, pw.traversal_time) {
            if length > 0.0 && tt > 0 {
                let speed = length / tt as f64;
                if speed > MAX_PATHWAY_SPEED_MS {
                    notices.push(k6_notice(
                        ctr,
                        "PTH_015",
                        EntityType::Pathway,
                        Some(pw.pathway_id.clone()),
                        Some(pw.pathway_id.clone()),
                        "pathways.txt",
                        Some(pw.line),
                        Some("traversal_time|length"),
                        Some(format!("{speed:.2} m/s ({length}m / {tt}s)")),
                        Some(format!("≤ {MAX_PATHWAY_SPEED_MS} m/s")),
                        format!(
                            "pathway_id '{}' length={length}m ve traversal_time={tt}s'den türetilen hız {speed:.2} m/s — makul sınır {MAX_PATHWAY_SPEED_MS} m/s.",
                            pw.pathway_id
                        ),
                        "traversal_time ve length değerlerini gerçekçi olarak güncelleyin.",
                    ));
                }
            }
        }
    }
}

// ── VAT: Varlık Analitik Tespiti ─────────────────────────────────────────────

fn check_vat_analytics(
    records: &EntityRecords,
    idx: &StopTimesIndex<'_>,
    notices: &mut Vec<Notice>,
    ctr: &mut u32,
) {
    // ── Ortak indeksler ─────────────────────────────────────────────────────

    let route_label: FxHashMap<&str, &str> = records.routes.iter()
        .map(|r| {
            let label = r.route_short_name.as_deref()
                .filter(|s| !s.is_empty())
                .unwrap_or(r.route_id.as_str());
            (r.route_id.as_str(), label)
        })
        .collect();

    let route_type_map: FxHashMap<&str, u32> = records.routes.iter()
        .filter_map(|r| r.route_type.map(|rt| (r.route_id.as_str(), rt)))
        .collect();

    let trip_route: FxHashMap<&str, &str> = records.trips.iter()
        .map(|t| (t.trip_id.as_str(), t.route_id.as_str()))
        .collect();

    let stop_name_map: FxHashMap<&str, &str> = records.stops.iter()
        .filter_map(|s| s.stop_name.as_deref().map(|n| (s.stop_id.as_str(), n)))
        .collect();

    // Tek geçiş: route_stops, stop_routes, route_trip_count
    let mut route_stops: FxHashMap<&str, FxHashSet<&str>> = FxHashMap::default();
    let mut stop_routes: FxHashMap<&str, FxHashSet<&str>> = FxHashMap::default();
    let mut route_trip_count: FxHashMap<&str, u32> = FxHashMap::default();

    for (&trip_id, stop_times) in &idx.by_trip {
        let route = match trip_route.get(trip_id) { Some(&r) => r, None => continue };
        *route_trip_count.entry(route).or_insert(0) += 1;
        let route_set = route_stops.entry(route).or_default();
        for st in stop_times.iter() {
            if st.stop_id.is_empty() { continue; }
            let sid = st.stop_id.as_str();
            route_set.insert(sid);
            stop_routes.entry(sid).or_default().insert(route);
        }
    }

    // Transfers'daki stop'lar
    let transfer_stops: FxHashSet<&str> = records.transfers.iter()
        .flat_map(|t| [t.from_stop_id.as_str(), t.to_stop_id.as_str()])
        .filter(|s| !s.is_empty())
        .collect();

    // ── VAT_001: Hat güzergah benzerliği (Jaccard ≥ 0.85) ───────────────────
    {
        let route_list: Vec<(&str, &FxHashSet<&str>, u32)> = route_stops.iter()
            .filter(|(_, stops)| stops.len() >= 5)
            .filter_map(|(&rid, stops)| {
                route_type_map.get(rid).map(|&rt| (rid, stops, rt))
            })
            .collect();

        // O(n²) → büyük feed'de atla
        if route_list.len() <= 300 {
            for i in 0..route_list.len() {
                for j in (i + 1)..route_list.len() {
                    let (rid_a, stops_a, rt_a) = route_list[i];
                    let (rid_b, stops_b, rt_b) = route_list[j];
                    if rt_a != rt_b { continue; }
                    let inter = stops_a.iter().filter(|s| stops_b.contains(*s)).count();
                    let union = stops_a.len() + stops_b.len() - inter;
                    if union == 0 { continue; }
                    let jaccard = inter as f64 / union as f64;
                    if jaccard >= 0.85 {
                        let la = route_label.get(rid_a).copied().unwrap_or(rid_a);
                        let lb = route_label.get(rid_b).copied().unwrap_or(rid_b);
                        notices.push(k6_notice(
                            ctr,
                            "VAT_001",
                            EntityType::Route,
                            Some(rid_a.to_string()),
                            None,
                            "routes.txt",
                            None,
                            Some("route_id"),
                            Some(format!("Jaccard={:.0}%", jaccard * 100.0)),
                            None,
                            format!("'{la}' ve '{lb}' hatları %{:.0} oranda aynı durağı paylaşıyor — muhtemel kopya hat.",
                                jaccard * 100.0),
                            "İki hattı birleştirin ya da güzergahları ayırt edilir biçimde farklılaştırın.",
                        ));
                    }
                }
            }
        }
    }

    // ── VAT_002: Aktarma merkezi tanımsız (≥ 4 route, transfer yok) ─────────
    {
        let is_station: FxHashSet<&str> = records.stops.iter()
            .filter(|s| s.location_type == Some(1))
            .map(|s| s.stop_id.as_str())
            .collect();

        for (&stop_id, routes) in &stop_routes {
            if routes.len() < 4 { continue; }
            if transfer_stops.contains(stop_id) { continue; }
            if is_station.contains(stop_id) { continue; }
            let name = stop_name_map.get(stop_id).copied().unwrap_or(stop_id);
            notices.push(k6_notice(
                ctr,
                "VAT_002",
                EntityType::Stop,
                Some(stop_id.to_string()),
                None,
                "stops.txt",
                None,
                Some("stop_id"),
                Some(format!("{} hat", routes.len())),
                None,
                format!("'{name}' (kod: '{stop_id}') durağından {} hat geçiyor; transfers.txt'te aktarma tanımlı değil.",
                    routes.len()),
                "transfers.txt'e bu durak için aktarma kayıtları ekleyin (transfer_type=2 ile bekleme süresi belirtin).",
            ));
        }
    }

    // ── VAT_003: Sefer süresi istatistiksel aykırı değer (MAD yöntemi) ───────
    {
        let mut route_durs: FxHashMap<&str, Vec<(&str, u32)>> = FxHashMap::default();
        for (&trip_id, stops) in &idx.by_trip {
            if stops.len() < 2 { continue; }
            let route = match trip_route.get(trip_id) { Some(&r) => r, None => continue };
            let first_dep = stops.first().and_then(|s| s.departure_time).map(hms_to_secs);
            let last_arr  = stops.last().and_then(|s| s.arrival_time).map(hms_to_secs);
            if let (Some(dep), Some(arr)) = (first_dep, last_arr) {
                if arr > dep {
                    route_durs.entry(route).or_default().push((trip_id, arr - dep));
                }
            }
        }

        for (route, mut durs) in route_durs {
            if durs.len() < 5 { continue; }
            durs.sort_by_key(|&(_, d)| d);
            let vals: Vec<u32> = durs.iter().map(|&(_, d)| d).collect();
            let median = vals[vals.len() / 2] as f64;
            if median < 120.0 { continue; }
            let mut abs_devs: Vec<f64> = vals.iter()
                .map(|&d| (d as f64 - median).abs())
                .collect();
            abs_devs.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
            let mad = abs_devs[abs_devs.len() / 2] * 1.4826;
            // MAD = 0 (tüm süreler aynı): ratio bazlı fallback — median'ın 3× üstü veya 0.25× altı
            let use_ratio_fallback = mad < 30.0;
            let threshold = if use_ratio_fallback { 0.0 } else { 2.5 * mad };
            for &(trip_id, dur) in &durs {
                let is_outlier = if use_ratio_fallback {
                    let r = dur as f64 / median;
                    r > 3.0 || r < 0.25
                } else {
                    (dur as f64 - median).abs() > threshold
                };
                if is_outlier {
                    let label = route_label.get(route).copied().unwrap_or(route);
                    let dur_min = dur / 60;
                    let med_min = (median as u32) / 60;
                    let sigma_str = if use_ratio_fallback {
                        format!("{:.1}× medyan", dur as f64 / median)
                    } else {
                        let dev = (dur as f64 - median).abs();
                        format!("{:.1}σ sapma", dev / (mad / 1.4826))
                    };
                    notices.push(k6_notice(
                        ctr,
                        "VAT_003",
                        EntityType::Trip,
                        Some(trip_id.to_string()),
                        None,
                        "stop_times.txt",
                        None,
                        Some("trip_id"),
                        Some(format!("{dur_min}dk (medyan {med_min}dk)")),
                        None,
                        format!("'{label}' hattının '{trip_id}' seferinin süresi {dur_min}dk — hat medyanı {med_min}dk ({sigma_str})."),
                        "stop_times.txt zaman değerlerini ve sefer güzergahını doğrulayın.",
                    ));
                }
            }
        }
    }

    // ── VAT_004: Hat hizmet asimetrisi (hafta içi ≥ 5 sefer, hafta sonu sıfır) ──
    {
        // service_id → (has_weekday, has_weekend) — calendar.txt'ten
        // days[0..4]=Mon-Fri, days[5]=Sat, days[6]=Sun
        let mut svc_weekday: FxHashMap<&str, bool> = FxHashMap::default();
        let mut svc_weekend: FxHashMap<&str, bool> = FxHashMap::default();
        for cal in &records.calendars {
            let has_wd = cal.days[0..5].iter().any(|d| *d == Some(1));
            let has_we = cal.days[5..7].iter().any(|d| *d == Some(1));
            svc_weekday.insert(cal.service_id.as_str(), has_wd);
            svc_weekend.insert(cal.service_id.as_str(), has_we);
        }

        // route_id → (weekday_trip_count, weekend_trip_count)
        let mut route_wd: FxHashMap<&str, u32> = FxHashMap::default();
        let mut route_we: FxHashMap<&str, u32> = FxHashMap::default();

        for trip in &records.trips {
            let route = trip.route_id.as_str();
            let svc = trip.service_id.as_str();
            let has_wd = svc_weekday.get(svc).copied().unwrap_or(false);
            let has_we = svc_weekend.get(svc).copied().unwrap_or(false);
            if has_wd { *route_wd.entry(route).or_insert(0) += 1; }
            if has_we { *route_we.entry(route).or_insert(0) += 1; }
        }

        for route in &records.routes {
            let rid = route.route_id.as_str();
            let wd = route_wd.get(rid).copied().unwrap_or(0);
            let we = route_we.get(rid).copied().unwrap_or(0);
            if wd >= 5 && we == 0 {
                let label = route_label.get(rid).copied().unwrap_or(rid);
                notices.push(k6_notice(
                    ctr,
                    "VAT_004",
                    EntityType::Route,
                    Some(rid.to_string()),
                    None,
                    "routes.txt",
                    None,
                    Some("route_id"),
                    Some(format!("{wd} hafta içi, 0 hafta sonu")),
                    None,
                    format!("'{label}' hattı haftanın 5 günü {wd} sefer çalışıyor; hafta sonu calendar kaydı tanımlı değil."),
                    "Hafta sonu hizmet yoksa bu bilgi notudur. Eksik sefer varsa calendar.txt'i güncelleyin.",
                ));
            }
        }
    }

    // ── VAT_005: İzole durak kümesi (BFS bağlı bileşenler) ─────────────────
    {
        let mut adj: FxHashMap<&str, FxHashSet<&str>> = FxHashMap::default();
        for (_, stops) in &idx.by_trip {
            for w in stops.windows(2) {
                let a = w[0].stop_id.as_str();
                let b = w[1].stop_id.as_str();
                if a.is_empty() || b.is_empty() { continue; }
                adj.entry(a).or_default().insert(b);
                adj.entry(b).or_default().insert(a);
            }
        }
        let total = adj.len();
        if total >= 5 {
            let mut visited: FxHashSet<&str> = FxHashSet::default();
            let mut comp_sizes: Vec<usize> = Vec::new();
            let mut comp_examples: Vec<&str> = Vec::new();
            let all_nodes: Vec<&str> = adj.keys().copied().collect();

            for &start in &all_nodes {
                if visited.contains(start) { continue; }
                let mut size = 0usize;
                let mut stack = vec![start];
                visited.insert(start);
                while let Some(node) = stack.pop() {
                    size += 1;
                    if let Some(neighbors) = adj.get(node) {
                        for &nb in neighbors {
                            if visited.insert(nb) {
                                stack.push(nb);
                            }
                        }
                    }
                }
                comp_sizes.push(size);
                comp_examples.push(start);
            }

            if comp_sizes.len() > 1 {
                let main_size = comp_sizes.iter().copied().max().unwrap_or(0);
                let small_thresh = (total / 20).max(2);

                // Hangi bileşenler izole? Örnek düğümleri + BFS tekrarı ile tam stop listesi
                let isolated_comp_examples: Vec<&str> = comp_examples.iter()
                    .zip(comp_sizes.iter())
                    .filter(|(_, &sz)| sz < main_size && sz <= small_thresh)
                    .map(|(&ex, _)| ex)
                    .collect();

                if !isolated_comp_examples.is_empty() {
                    // İzole bileşenlere ait tüm durakları topla (maks 200)
                    let mut isolated_all: Vec<&str> = Vec::new();
                    let mut revisited: FxHashSet<&str> = FxHashSet::default();
                    'outer: for &start in &isolated_comp_examples {
                        let mut stack = vec![start];
                        revisited.insert(start);
                        while let Some(node) = stack.pop() {
                            isolated_all.push(node);
                            if isolated_all.len() >= 200 { break 'outer; }
                            if let Some(neighbors) = adj.get(node) {
                                for &nb in neighbors {
                                    if revisited.insert(nb) {
                                        stack.push(nb);
                                    }
                                }
                            }
                        }
                    }

                    let isolated_count = isolated_all.len();
                    let comp_count = isolated_comp_examples.len();
                    let example = isolated_comp_examples[0];
                    let ex_name = stop_name_map.get(example).copied().unwrap_or(example);

                    let mut n005 = k6_notice(
                        ctr,
                        "VAT_005",
                        EntityType::Feed,
                        None,
                        None,
                        "stop_times.txt",
                        None,
                        None,
                        Some(format!("{isolated_count} durak, {comp_count} bileşen")),
                        None,
                        format!("Ağ grafında ana bileşenden kopuk {comp_count} izole durak kümesi var ({isolated_count} durak). Örnek: '{ex_name}' ('{example}')."),
                        "Kopuk durakların seferlere bağlandığını ve stop_times kayıtlarının doğru olduğunu kontrol edin.",
                    );
                    let mut d = std::collections::HashMap::new();
                    d.insert("isolated_stops".to_string(), isolated_all.join(","));
                    n005.details = Some(d);
                    notices.push(n005);
                }
            }
        }
    }

    // ── VAT_006: Hizmet yoğunluğu dengesizliği (tek hat ≥ %40) ────────────
    {
        let total_trips: u32 = route_trip_count.values().sum();
        if total_trips >= 50 {
            for (&route, &count) in &route_trip_count {
                let ratio = count as f64 / total_trips as f64;
                if ratio >= 0.40 {
                    let label = route_label.get(route).copied().unwrap_or(route);
                    notices.push(k6_notice(
                        ctr,
                        "VAT_006",
                        EntityType::Route,
                        Some(route.to_string()),
                        None,
                        "routes.txt",
                        None,
                        Some("route_id"),
                        Some(format!("{count}/{total_trips} (%{:.0})", ratio * 100.0)),
                        None,
                        format!("'{label}' hattı feed'deki {total_trips} seferin {count}'ini (%{:.0}) oluşturuyor — hizmet yoğunluğu dengesiz.",
                            ratio * 100.0),
                        "Yüksek sefer yoğunluğu operasyonel bir gerçeği yansıtıyorsa bu bilgi notudur. Değilse diğer hatlara sefer ekleyin.",
                    ));
                }
            }
        }
    }

    // ── VAT_007: Terminus aktarma eksikliği (≥ 3 route terminusu, transfer yok) ──
    {
        let mut terminus_routes: FxHashMap<&str, FxHashSet<&str>> = FxHashMap::default();
        for (&trip_id, stops) in &idx.by_trip {
            let route = match trip_route.get(trip_id) { Some(&r) => r, None => continue };
            if let Some(first) = stops.first() {
                let sid = first.stop_id.as_str();
                if !sid.is_empty() {
                    terminus_routes.entry(sid).or_default().insert(route);
                }
            }
            if stops.len() > 1 {
                if let Some(last) = stops.last() {
                    let sid = last.stop_id.as_str();
                    if !sid.is_empty() {
                        terminus_routes.entry(sid).or_default().insert(route);
                    }
                }
            }
        }

        for (&stop_id, routes) in &terminus_routes {
            if routes.len() < 3 { continue; }
            if transfer_stops.contains(stop_id) { continue; }
            let name = stop_name_map.get(stop_id).copied().unwrap_or(stop_id);
            let mut labels: Vec<&str> = routes.iter()
                .take(5)
                .map(|r| route_label.get(r).copied().unwrap_or(r))
                .collect();
            labels.sort_unstable();
            let mut n007 = k6_notice(
                ctr,
                "VAT_007",
                EntityType::Stop,
                Some(stop_id.to_string()),
                None,
                "stops.txt",
                None,
                Some("stop_id"),
                Some(format!("{} hat terminusu", routes.len())),
                None,
                format!("'{name}' (kod: '{stop_id}') {} hattın terminal durağı; transfers.txt'te aktarma tanımlı değil. Hatlar: {}.",
                    routes.len(), labels.join(", ")),
                "transfers.txt'e bu terminus durağı için aktarma kaydı ekleyin.",
            );
            let mut d = std::collections::HashMap::new();
            d.insert("routes".to_string(), routes.iter().copied().collect::<Vec<_>>().join(","));
            n007.details = Some(d);
            notices.push(n007);
        }
    }

    // OPR_024: Hat 500'den fazla sefer içeriyor
    {
        let mut route_trip_counts: HashMap<&str, u32> = HashMap::new();
        for t in &records.trips {
            if !t.route_id.is_empty() {
                *route_trip_counts.entry(t.route_id.as_str()).or_default() += 1;
            }
        }
        for (route_id, count) in &route_trip_counts {
            if *count > 500 {
                notices.push(k6_notice(
                    ctr, "OPR_024", EntityType::Route,
                    Some((*route_id).to_string()), Some((*route_id).to_string()),
                    "trips.txt", None, None,
                    Some(count.to_string()), Some("≤500".to_string()),
                    format!("'{route_id}' hattında {count} sefer var — veri birleştirme sorunu olabilir."),
                    "Bu hattaki seferlerin doğru route_id'ye atandığını kontrol edin.",
                ));
            }
        }
    }

    // OPR_025: Ortalama sefer süresi 60 saniyeden kısa (feed genelinde)
    {
        let mut trip_durations: Vec<u64> = Vec::new();
        for stops in records.stop_times_index.iter_stops() {
            let mut min_dep: Option<u32> = None;
            let mut max_dep: Option<u32> = None;
            for st in stops {
                if let Some((h, m, s)) = st.departure_time {
                    let secs = h as u32 * 3600 + m as u32 * 60 + s as u32;
                    min_dep = Some(min_dep.map_or(secs, |prev: u32| prev.min(secs)));
                    max_dep = Some(max_dep.map_or(secs, |prev: u32| prev.max(secs)));
                }
            }
            if let (Some(f), Some(l)) = (min_dep, max_dep) {
                if l > f { trip_durations.push((l - f) as u64); }
            }
        }
        if trip_durations.len() >= 5 {
            let avg = trip_durations.iter().sum::<u64>() / trip_durations.len() as u64;
            if avg < 60 {
                notices.push(k6_notice(
                    ctr, "OPR_025", EntityType::Feed,
                    None, None, "stop_times.txt", None, None,
                    Some(format!("{avg}s")), Some("≥60s".to_string()),
                    format!("Ortalama sefer süresi {avg} saniye — bu kadar kısa süreler genellikle veri hatasını gösterir."),
                    "stop_times.txt'deki departure_time ve arrival_time değerlerini kontrol edin.",
                ));
            }
        }
    }

    // VAT_008: Aynı shape feed hatlarının >%30'unda kullanılıyor
    {
        let total_routes = records.routes.len();
        if total_routes >= 3 {
            let mut shape_routes: HashMap<&str, HashSet<&str>> = HashMap::new();
            for t in &records.trips {
                if let Some(shape_id) = t.shape_id.as_deref() {
                    if !shape_id.is_empty() && !t.route_id.is_empty() {
                        shape_routes.entry(shape_id).or_default().insert(t.route_id.as_str());
                    }
                }
            }
            let threshold = (total_routes as f64 * 0.3).ceil() as usize;
            for (shape_id, route_set) in &shape_routes {
                if route_set.len() > threshold && route_set.len() >= 3 {
                    let pct = route_set.len() as f64 / total_routes as f64 * 100.0;
                    notices.push(k6_notice(
                        ctr, "VAT_008", EntityType::Shape,
                        Some((*shape_id).to_string()), None,
                        "trips.txt", None, None,
                        Some(format!("{:.0}% ({} hat)", pct, route_set.len())),
                        Some(format!("≤{threshold} hat")),
                        format!("'{shape_id}' shape'i {} hatta ({pct:.0}%) kullanılıyor — olası yanlış shape ataması.",
                            route_set.len()),
                        "Her hat ve yön için ayrı shape_id tanımlayın.",
                    ));
                }
            }
        }
    }
}

// ── Testler ───────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::k2::routes::RouteRecord;
    use crate::k2::stop_times::StopTimeRecord;
    use crate::k2::stops::StopRecord;
    use crate::k2::trips::TripRecord;
    use crate::k3_entity_graph::EntityMap;

    fn default_config() -> ValidatorConfig {
        ValidatorConfig::default()
    }

    fn empty_derived() -> DerivedData {
        DerivedData::default()
    }

    fn stop(stop_id: &str, lat: f64, lon: f64) -> StopRecord {
        StopRecord {
            stop_id: stop_id.into(),
            stop_code: None, stop_name: None,
            stop_lat: Some(lat), stop_lon: Some(lon),
            location_type: None, stop_timezone: None,
            wheelchair_boarding: None, stop_access: None,
            level_id: None, tts_stop_name: None,
            row: Default::default(), line: 2,
            ..Default::default()
        }
    }

    fn route(route_id: &str, route_type: u32) -> RouteRecord {
        RouteRecord {
            route_id: route_id.into(),
            agency_id: None, route_short_name: None, route_long_name: None,
            route_desc: None, route_type: Some(route_type), route_url: None,
            route_color: None, route_text_color: None, route_sort_order: None,
            continuous_pickup: None, continuous_drop_off: None, network_id: None,
            route_cemv_support: None, row: Default::default(), line: 2,
        }
    }

    fn trip(trip_id: &str, route_id: &str) -> TripRecord {
        TripRecord {
            trip_id: trip_id.into(),
            route_id: route_id.into(),
            service_id: "SVC".into(),
            shape_id: None, trip_headsign: None, trip_short_name: None,
            direction_id: None, block_id: None,
            wheelchair_accessible: None, bikes_allowed: None,
            cars_allowed: None, safe_duration_factor: None, safe_duration_offset: None,
            line: 2,
        }
    }

    fn stoptime(trip_id: &str, seq: u32, stop_id: &str, arr: (u32,u32,u32), dep: (u32,u32,u32), line: u64) -> StopTimeRecord {
        StopTimeRecord {
            trip_id: trip_id.into(),
            stop_id: stop_id.into(),
            stop_sequence: Some(seq),
            arrival_time: Some(arr),
            departure_time: Some(dep),
            line,
            ..Default::default()
        }
    }

    fn records_with(
        stops: Vec<StopRecord>,
        routes: Vec<RouteRecord>,
        trips: Vec<TripRecord>,
        stoptimes: Vec<StopTimeRecord>,
    ) -> crate::k2::EntityRecords {
        use crate::k2::stop_times::StopTimesIndex;
        let mut r = crate::k2::EntityRecords::default();
        r.stops = stops;
        r.routes = routes;
        r.trips = trips;
        r.stop_times_index = StopTimesIndex::from_records(&stoptimes);
        r.stop_times = stoptimes;
        r
    }

    // ── STM_014 ───────────────────────────────────────────────────────────────

    #[test]
    fn excessive_speed_produces_stm_014() {
        // İstanbul (~41.0, 29.0) → Ankara (~39.9, 32.9) ≈ 350 km, 2 saatte → ~175 km/h
        // 30 dk olsaydı ~703 km/h → STM_012 tetiklenirdi; 2 saat STM_014 eşiğini (120 km/h) aşar
        let records = records_with(
            vec![stop("A", 41.0, 29.0), stop("B", 39.9, 32.9)],
            vec![route("R1", 3)],  // bus
            vec![trip("T1", "R1")],
            vec![
                stoptime("T1", 1, "A", (8,0,0), (8,0,0), 2),
                stoptime("T1", 2, "B", (10,0,0), (10,0,0), 3), // 2 saat, ~350 km → ~175 km/h
            ],
        );
        let result = analyze(&records, &empty_derived(), &default_config(), 20260514);
        assert!(result.notices.iter().any(|n| n.rule_id == "STM_014"), "STM_014 olmalı");
    }

    #[test]
    fn normal_speed_no_stm_014() {
        // İki yakın durak, 10 dk → düşük hız
        let records = records_with(
            vec![stop("A", 41.0, 29.0), stop("B", 41.05, 29.05)],
            vec![route("R1", 3)],
            vec![trip("T1", "R1")],
            vec![
                stoptime("T1", 1, "A", (8,0,0), (8,0,0), 2),
                stoptime("T1", 2, "B", (8,10,0), (8,10,0), 3),
            ],
        );
        let result = analyze(&records, &empty_derived(), &default_config(), 20260514);
        assert!(!result.notices.iter().any(|n| n.rule_id == "STM_014"));
    }

    // ── OPR_008 ───────────────────────────────────────────────────────────────

    #[test]
    fn excessive_speed_produces_opr_008() {
        // OPR_008 için trip_bad_seg_count > 1 gerekir: 3 durak, 2 anormal segment
        let records = records_with(
            vec![stop("A", 41.0, 29.0), stop("B", 39.9, 32.9), stop("C", 37.0, 36.0)],
            vec![route("R1", 3)],
            vec![trip("T1", "R1")],
            vec![
                stoptime("T1", 1, "A", (8,0,0),  (8,0,0),  2),
                stoptime("T1", 2, "B", (8,10,0), (8,10,0), 3), // ~350km / 10dk → aşırı hız
                stoptime("T1", 3, "C", (8,20,0), (8,20,0), 4), // ~350km / 10dk → aşırı hız
            ],
        );
        let result = analyze(&records, &empty_derived(), &default_config(), 20260514);
        assert!(result.notices.iter().any(|n| n.rule_id == "OPR_008"), "OPR_008 olmalı");
    }

    // ── STM_028 ───────────────────────────────────────────────────────────────

    #[test]
    fn long_trip_produces_stm_028() {
        // 30 saatlik sefer > 24 saatlik eşik
        let records = records_with(
            vec![stop("A", 41.0, 29.0), stop("B", 41.01, 29.01)],
            vec![route("R1", 3)],
            vec![trip("T1", "R1")],
            vec![
                stoptime("T1", 1, "A", (8,0,0), (8,0,0), 2),
                stoptime("T1", 2, "B", (38,0,0), (38,0,0), 3), // GTFS 38:00:00
            ],
        );
        let result = analyze(&records, &empty_derived(), &default_config(), 20260514);
        assert!(result.notices.iter().any(|n| n.rule_id == "STM_028"));
    }

    #[test]
    fn normal_trip_no_stm_028() {
        let records = records_with(
            vec![stop("A", 41.0, 29.0), stop("B", 41.01, 29.01)],
            vec![route("R1", 3)],
            vec![trip("T1", "R1")],
            vec![
                stoptime("T1", 1, "A", (8,0,0), (8,0,0), 2),
                stoptime("T1", 2, "B", (8,30,0), (8,30,0), 3),
            ],
        );
        let result = analyze(&records, &empty_derived(), &default_config(), 20260514);
        assert!(!result.notices.iter().any(|n| n.rule_id == "STM_028"));
    }

    // ── STM_029 ───────────────────────────────────────────────────────────────

    #[test]
    fn short_trip_produces_stm_029() {
        // 30 saniyelik sefer < 60 saniyelik eşik
        let records = records_with(
            vec![stop("A", 41.0, 29.0), stop("B", 41.01, 29.01)],
            vec![route("R1", 3)],
            vec![trip("T1", "R1")],
            vec![
                stoptime("T1", 1, "A", (8,0,0), (8,0,0), 2),
                stoptime("T1", 2, "B", (8,0,30), (8,0,30), 3),
            ],
        );
        let result = analyze(&records, &empty_derived(), &default_config(), 20260514);
        assert!(result.notices.iter().any(|n| n.rule_id == "STM_029"));
    }

    // ── STM_020: sıfır seyahat süresi ─────────────────────────────────────────

    #[test]
    fn zero_travel_time_with_distance_produces_stm_020() {
        // Aynı varış/kalkış saati (saniyeli → dakika yuvarlama değil), farklı konumda → STM_020
        let records = records_with(
            vec![stop("A", 41.0, 29.0), stop("B", 41.05, 29.05)], // ~6 km
            vec![route("R1", 3)],
            vec![trip("T1", "R1")],
            vec![
                stoptime("T1", 1, "A", (8,0,30), (8,0,30), 2),
                stoptime("T1", 2, "B", (8,0,30), (8,0,30), 3), // aynı zaman, farklı konum
            ],
        );
        let result = analyze(&records, &empty_derived(), &default_config(), 20260514);
        assert!(result.notices.iter().any(|n| n.rule_id == "STM_020"), "STM_020 olmalı");
    }

    #[test]
    fn zero_travel_time_same_stop_no_stm_020() {
        // Aynı durak, aynı zaman → mesafe 0 → STM_020 üretmemeli
        let records = records_with(
            vec![stop("A", 41.0, 29.0)],
            vec![route("R1", 3)],
            vec![trip("T1", "R1")],
            vec![
                stoptime("T1", 1, "A", (8,0,0), (8,0,0), 2),
                stoptime("T1", 2, "A", (8,0,0), (8,0,0), 3),
            ],
        );
        let result = analyze(&records, &empty_derived(), &default_config(), 20260514);
        assert!(!result.notices.iter().any(|n| n.rule_id == "STM_020"), "aynı durak STM_020 üretmemeli");
    }

    #[test]
    fn empty_feed_no_notices() {
        use crate::k2::EntityRecords;
        let result = analyze(&EntityRecords::default(), &empty_derived(), &default_config(), 20260514);
        assert!(result.notices.is_empty());
    }

    // ── Format yardımcısı ─────────────────────────────────────────────────────

    #[test]
    fn format_hms_correct() {
        assert_eq!(format_hms(3661), "01:01:01");
        assert_eq!(format_hms(86400), "24:00:00");
        assert_eq!(format_hms(0), "00:00:00");
    }

    // ── WP-09b: FRQ_006 / FRQ_010 ────────────────────────────────────────────

    use crate::k2::frequencies::FrequencyRecord;

    fn frq(trip_id: &str, hw: u32, line: u64) -> FrequencyRecord {
        FrequencyRecord {
            trip_id: trip_id.into(),
            start_time: Some((8, 0, 0)),
            end_time: Some((20, 0, 0)),
            headway_secs: Some(hw),
            exact_times: None,
            row: Default::default(),
            line,
        }
    }

    #[test]
    fn long_headway_produces_frq_006() {
        let mut r = crate::k2::EntityRecords::default();
        r.frequencies = vec![frq("T1", 14401, 2)]; // 4 saat 1 dakika > 240 dk eşiği
        let result = analyze(&r, &empty_derived(), &default_config(), 20260514);
        assert!(result.notices.iter().any(|n| n.rule_id == "FRQ_006"));
    }

    #[test]
    fn short_headway_produces_frq_010() {
        let mut r = crate::k2::EntityRecords::default();
        r.frequencies = vec![frq("T1", 60, 2)]; // 1 dk ≤ 2 dk bunching eşiği
        let result = analyze(&r, &empty_derived(), &default_config(), 20260514);
        assert!(result.notices.iter().any(|n| n.rule_id == "FRQ_010"));
    }

    #[test]
    fn normal_headway_no_frq_notices() {
        let mut r = crate::k2::EntityRecords::default();
        r.frequencies = vec![frq("T1", 600, 2)]; // 10 dk — normal
        let result = analyze(&r, &empty_derived(), &default_config(), 20260514);
        assert!(!result.notices.iter().any(|n| n.rule_id == "FRQ_006"));
        assert!(!result.notices.iter().any(|n| n.rule_id == "FRQ_010"));
    }

    // ── WP-09b: OPR_001 / OPR_003 ────────────────────────────────────────────

    #[test]
    fn large_route_headway_produces_opr_001() {
        // İki sefer: 08:00 ve 13:00 → 300 dk boşluk > 240 dk eşiği
        let records = records_with(
            vec![stop("A", 41.0, 29.0), stop("B", 41.01, 29.01)],
            vec![route("R1", 3)],
            vec![trip("T1", "R1"), trip("T2", "R1")],
            vec![
                stoptime("T1", 1, "A", (8, 0, 0), (8, 0, 0), 2),
                stoptime("T1", 2, "B", (8, 10, 0), (8, 10, 0), 3),
                stoptime("T2", 1, "A", (13, 0, 0), (13, 0, 0), 4),
                stoptime("T2", 2, "B", (13, 10, 0), (13, 10, 0), 5),
            ],
        );
        let result = analyze(&records, &empty_derived(), &default_config(), 20260514);
        assert!(result.notices.iter().any(|n| n.rule_id == "OPR_001"));
    }

    #[test]
    fn small_route_headway_produces_opr_003() {
        // İki sefer: 08:00 ve 08:01 → 1 dk ≤ 2 dk bunching eşiği
        let records = records_with(
            vec![stop("A", 41.0, 29.0), stop("B", 41.01, 29.01)],
            vec![route("R1", 3)],
            vec![trip("T1", "R1"), trip("T2", "R1")],
            vec![
                stoptime("T1", 1, "A", (8, 0, 0), (8, 0, 0), 2),
                stoptime("T1", 2, "B", (8, 10, 0), (8, 10, 0), 3),
                stoptime("T2", 1, "A", (8, 1, 0), (8, 1, 0), 4),
                stoptime("T2", 2, "B", (8, 11, 0), (8, 11, 0), 5),
            ],
        );
        let result = analyze(&records, &empty_derived(), &default_config(), 20260514);
        assert!(result.notices.iter().any(|n| n.rule_id == "OPR_003"));
    }

    // ── OPR_001/OPR_003 yeni testler: dedup davranışı ────────────────────────

    fn trip_with_service(trip_id: &str, route_id: &str, service_id: &str) -> TripRecord {
        TripRecord {
            trip_id: trip_id.into(),
            route_id: route_id.into(),
            service_id: service_id.into(),
            shape_id: None, trip_headsign: None, trip_short_name: None,
            direction_id: None, block_id: None,
            wheelchair_accessible: None, bikes_allowed: None,
            cars_allowed: None, safe_duration_factor: None, safe_duration_offset: None,
            line: 2,
        }
    }

    #[test]
    fn dedup_same_time_suppresses_opr_003() {
        // 5 trip, hepsi 08:00 kalkış → dedup sonrası tek unique saat → bucket atlanır → OPR_003 yok
        // Config: bunching_threshold_min = 2 (default ile aynı)
        let cfg = ValidatorConfig { bunching_threshold_min: 2, ..default_config() };
        let trips: Vec<TripRecord> = (1..=5)
            .map(|i| trip_with_service(&format!("T{i}"), "R1", "SVC"))
            .collect();
        let mut stoptimes = Vec::new();
        for i in 1u32..=5 {
            let tid = format!("T{i}");
            stoptimes.push(stoptime(&tid, 1, "A", (8, 0, 0), (8, 0, 0), (i * 2) as u64));
            stoptimes.push(stoptime(&tid, 2, "B", (8, 10, 0), (8, 10, 0), (i * 2 + 1) as u64));
        }
        let records = records_with(
            vec![stop("A", 41.0, 29.0), stop("B", 41.01, 29.01)],
            vec![route("R1", 3)],
            trips,
            stoptimes,
        );
        let result = analyze(&records, &empty_derived(), &cfg, 20260514);
        assert!(!result.notices.iter().any(|n| n.rule_id == "OPR_003"),
            "tek unique kalkış saati dedup'tan sonra bucket atlanmalı → OPR_003 olmamalı");
    }

    #[test]
    fn true_bunching_survives_dedup() {
        // 08:00 × 5 trip + 08:01 × 5 trip → dedup: [28800, 28860] → fark 60s = 1dk < 2dk → OPR_003
        // Config: bunching_threshold_min = 2
        let cfg = ValidatorConfig { bunching_threshold_min: 2, ..default_config() };
        let mut trips = Vec::new();
        let mut stoptimes = Vec::new();
        for i in 1u32..=5 {
            let tid = format!("TA{i}");
            trips.push(trip_with_service(&tid, "R1", "SVC"));
            stoptimes.push(stoptime(&tid, 1, "A", (8, 0, 0), (8, 0, 0), (i * 10) as u64));
            stoptimes.push(stoptime(&tid, 2, "B", (8, 10, 0), (8, 10, 0), (i * 10 + 1) as u64));
        }
        for i in 1u32..=5 {
            let tid = format!("TB{i}");
            trips.push(trip_with_service(&tid, "R1", "SVC"));
            stoptimes.push(stoptime(&tid, 1, "A", (8, 1, 0), (8, 1, 0), (100 + i * 10) as u64));
            stoptimes.push(stoptime(&tid, 2, "B", (8, 11, 0), (8, 11, 0), (100 + i * 10 + 1) as u64));
        }
        let records = records_with(
            vec![stop("A", 41.0, 29.0), stop("B", 41.01, 29.01)],
            vec![route("R1", 3)],
            trips,
            stoptimes,
        );
        let result = analyze(&records, &empty_derived(), &cfg, 20260514);
        // dedup sonrası [28800, 28860] → min headway = 60s < 120s (2dk) → OPR_003 tetiklenmeli
        assert!(result.notices.iter().any(|n| n.rule_id == "OPR_003"),
            "gerçek sıkışma (60s < 120s eşiği) dedup'tan sonra da OPR_003 üretmeli");
    }

    // ── WP-09b: CAL_008 / CAL_009 / CAL_010 / CAL_007 / CAL_012 ─────────────

    use crate::k2::calendar::CalendarRecord;

    fn cal_rec(service_id: &str, days: [Option<u32>; 7], start: (u32,u32,u32), end: (u32,u32,u32)) -> CalendarRecord {
        CalendarRecord {
            service_id: service_id.into(),
            days,
            start_date: Some(start),
            end_date: Some(end),
            row: Default::default(),
            line: 2,
        }
    }

    fn all_days() -> [Option<u32>; 7] {
        [Some(1); 7]
    }

    #[test]
    fn expired_service_produces_cal_013_not_cal_009() {
        // Tek servis süresi dolmuş → bilgi seviyesi CAL_013, KRİTİK CAL_009 değil
        let mut records = crate::k2::EntityRecords::default();
        records.calendars = vec![cal_rec("SVC", all_days(), (2025, 1, 1), (2025, 12, 31))];
        // today = 20260514 > 20251231
        let result = analyze(&records, &empty_derived(), &default_config(), 20260514);
        assert!(result.notices.iter().any(|n| n.rule_id == "CAL_013"),
            "süresi dolmuş tekil servis CAL_013 üretmeli");
        assert!(!result.notices.iter().any(|n| n.rule_id == "CAL_009"),
            "tek servis durumunda k6'dan CAL_009 üretmemeli (k4 tümü dolmuşsa atar)");
    }

    #[test]
    fn expiring_soon_produces_cal_008() {
        let mut records = crate::k2::EntityRecords::default();
        // Bitiş tarihi 10 gün sonra (< 30 gün uyarı eşiği)
        records.calendars = vec![cal_rec("SVC", all_days(), (2026, 5, 1), (2026, 5, 24))];
        let result = analyze(&records, &empty_derived(), &default_config(), 20260514);
        assert!(result.notices.iter().any(|n| n.rule_id == "CAL_008"));
    }

    #[test]
    fn short_service_produces_cal_010() {
        // Sadece 3 aktif gün (< service_gap_days=7)
        use crate::k5_derived::CalendarBitmap;
        let mut derived = DerivedData::default();
        derived.calendar_bitmap = CalendarBitmap {
            active_dates: [("SVC".to_string(),
                [20260511u32, 20260512, 20260513].into_iter().collect())]
                .into_iter().collect(),
        };
        let records = crate::k2::EntityRecords::default();
        let result = analyze(&records, &derived, &default_config(), 20260514);
        assert!(result.notices.iter().any(|n| n.rule_id == "CAL_010"));
    }

    #[test]
    fn past_service_gap_produces_cal_007_not_cal_012() {
        use crate::k5_derived::CalendarBitmap;
        let mut derived = DerivedData::default();
        // Geçmişteki boşluk (Jan→Feb 2026, today=May 2026): CAL_007 evet, CAL_012 hayır
        let mut dates = std::collections::HashSet::new();
        dates.insert(20260101u32);
        dates.insert(20260201u32);
        derived.calendar_bitmap = CalendarBitmap {
            active_dates: [("SVC".to_string(), dates)].into_iter().collect(),
        };
        let records = crate::k2::EntityRecords::default();
        let result = analyze(&records, &derived, &default_config(), 20260514);
        assert!(result.notices.iter().any(|n| n.rule_id == "CAL_007"),
            "Geçmişteki boşluk CAL_007 üretmeli");
        assert!(!result.notices.iter().any(|n| n.rule_id == "CAL_012"),
            "Geçmişteki boşluk CAL_012 üretmemeli");
    }

    #[test]
    fn near_future_service_gap_produces_cal_007_and_cal_012() {
        use crate::k5_derived::CalendarBitmap;
        let mut derived = DerivedData::default();
        // Yakın gelecekte boşluk (today=20260514, boşluk 20260516→20260601 = 15 gün > 7 eşik)
        let mut dates = std::collections::HashSet::new();
        dates.insert(20260516u32);
        dates.insert(20260601u32);
        derived.calendar_bitmap = CalendarBitmap {
            active_dates: [("SVC".to_string(), dates)].into_iter().collect(),
        };
        let records = crate::k2::EntityRecords::default();
        let result = analyze(&records, &derived, &default_config(), 20260514);
        assert!(result.notices.iter().any(|n| n.rule_id == "CAL_007"),
            "Yakın gelecek boşluk CAL_007 üretmeli");
        assert!(result.notices.iter().any(|n| n.rule_id == "CAL_012"),
            "Yakın gelecek boşluk CAL_012 üretmeli");
    }

    // ── CAL_013: bitmap üzerinden süresi dolmuş servis ────────────────────────

    #[test]
    fn expired_service_in_bitmap_produces_cal_013() {
        use crate::k2::calendar_dates::CalendarDateRecord;
        use crate::k5_derived::CalendarBitmap;
        let mut derived = DerivedData::default();
        // Tüm tarihler geçmişte: max_date = 20251231 < today = 20260514
        let mut dates = std::collections::HashSet::new();
        dates.insert(20251201u32);
        dates.insert(20251231u32);
        derived.calendar_bitmap = CalendarBitmap {
            active_dates: [("EXPIRED_SVC".to_string(), dates)].into_iter().collect(),
        };
        let mut records = crate::k2::EntityRecords::default();
        // calendar_dates kaydı olmadan override_counts boş kalır → loop çalışmaz
        records.calendar_dates = vec![CalendarDateRecord {
            service_id: "EXPIRED_SVC".into(),
            date: Some((2025, 12, 31)),
            exception_type: Some(1),
            row: Default::default(), line: 2,
        }];
        let result = analyze(&records, &derived, &default_config(), 20260514);
        assert!(result.notices.iter().any(|n| n.rule_id == "CAL_013"), "CAL_013 olmalı");
    }

    #[test]
    fn active_service_no_cal_013() {
        use crate::k2::calendar_dates::CalendarDateRecord;
        use crate::k5_derived::CalendarBitmap;
        let mut derived = DerivedData::default();
        // Gelecekte tarih var
        let mut dates = std::collections::HashSet::new();
        dates.insert(20260601u32);
        dates.insert(20260630u32);
        derived.calendar_bitmap = CalendarBitmap {
            active_dates: [("ACTIVE_SVC".to_string(), dates)].into_iter().collect(),
        };
        let mut records = crate::k2::EntityRecords::default();
        records.calendar_dates = vec![CalendarDateRecord {
            service_id: "ACTIVE_SVC".into(),
            date: Some((2026, 6, 1)),
            exception_type: Some(1),
            row: Default::default(), line: 2,
        }];
        let result = analyze(&records, &derived, &default_config(), 20260514);
        assert!(!result.notices.iter().any(|n| n.rule_id == "CAL_013"), "aktif servis CAL_013 üretmemeli");
    }

    // ── WP-09c: GEO_006 ─────────────────────────────────────────────────────

    #[test]
    fn shape_jump_produces_geo_006() {
        use crate::k5_derived::{ShapeGeometry, ShapeSegments};
        let mut derived = DerivedData::default();
        derived.shape_geometry = ShapeGeometry {
            shapes: [(
                "S1".to_string(),
                ShapeSegments {
                    segment_distances_km: vec![0.5, 15.0, 0.3], // 15 km > 10 km eşiği
                    total_length_km: 15.8,
                    bbox: (40.0, 42.0, 28.0, 30.0),
                },
            )]
            .into_iter()
            .collect(),
        };
        let result = analyze(&crate::k2::EntityRecords::default(), &derived, &default_config(), 20260514);
        assert!(result.notices.iter().any(|n| n.rule_id == "GEO_006"));
    }

    #[test]
    fn normal_shape_no_geo_006() {
        use crate::k5_derived::{ShapeGeometry, ShapeSegments};
        let mut derived = DerivedData::default();
        derived.shape_geometry = ShapeGeometry {
            shapes: [(
                "S1".to_string(),
                ShapeSegments {
                    segment_distances_km: vec![0.5, 0.8, 0.3],
                    total_length_km: 1.6,
                    bbox: (40.0, 40.1, 28.0, 28.1),
                },
            )]
            .into_iter()
            .collect(),
        };
        let result = analyze(&crate::k2::EntityRecords::default(), &derived, &default_config(), 20260514);
        assert!(!result.notices.iter().any(|n| n.rule_id == "GEO_006"));
    }

    // ── WP-09c: STP_017 ─────────────────────────────────────────────────────

    #[test]
    fn very_close_stops_produce_stp_017() {
        use crate::k5_derived::SpatialIndex;
        // İki durak 4 metre arayla (< 5m eşiği): 0.000036° fark ≈ 4m
        let mut records = crate::k2::EntityRecords::default();
        records.stops = vec![
            stop("A", 41.0, 29.0),
            stop("B", 41.000036, 29.0), // ~4m kuzey
        ];
        let mut derived = DerivedData::default();
        derived.spatial_index = SpatialIndex {
            grid: [((82i32, 58i32), vec![0usize, 1usize])].into_iter().collect(),
            cell_deg: 0.5,
        };
        let result = analyze(&records, &derived, &default_config(), 20260514);
        assert!(result.notices.iter().any(|n| n.rule_id == "STP_017"), "STP_017 olmalı");
    }

    // ── WP-09c: OPR_006 / OPR_007 ───────────────────────────────────────────

    #[test]
    fn single_stop_trip_produces_opr_006() {
        let records = records_with(
            vec![stop("A", 41.0, 29.0)],
            vec![route("R1", 3)],
            vec![trip("T1", "R1")],
            vec![stoptime("T1", 1, "A", (8, 0, 0), (8, 0, 0), 2)],
        );
        let result = analyze(&records, &empty_derived(), &default_config(), 20260514);
        assert!(result.notices.iter().any(|n| n.rule_id == "OPR_006"));
    }

    #[test]
    fn repeated_stop_in_trip_produces_opr_007() {
        // A→B→C→B: non-ring (first=A, last=B), B ara durakta tekrar → OPR_007 tetiklenmeli
        let records = records_with(
            vec![stop("A", 41.0, 29.0), stop("B", 41.1, 29.1), stop("C", 41.2, 29.2)],
            vec![route("R1", 3)],
            vec![trip("T1", "R1")],
            vec![
                stoptime("T1", 1, "A", (8,  0, 0), (8,  0, 0), 2),
                stoptime("T1", 2, "B", (8, 10, 0), (8, 10, 0), 3),
                stoptime("T1", 3, "C", (8, 20, 0), (8, 20, 0), 4),
                stoptime("T1", 4, "B", (8, 30, 0), (8, 30, 0), 5), // B tekrar, ring değil
            ],
        );
        let result = analyze(&records, &empty_derived(), &default_config(), 20260514);
        assert!(result.notices.iter().any(|n| n.rule_id == "OPR_007"));
    }

    #[test]
    fn ring_route_terminal_repeat_no_opr_007() {
        // A→B→A: first==last, ring hat → terminal tekrarı suppress edilmeli
        let records = records_with(
            vec![stop("A", 41.0, 29.0), stop("B", 41.1, 29.1)],
            vec![route("R1", 3)],
            vec![trip("T1", "R1")],
            vec![
                stoptime("T1", 1, "A", (8,  0, 0), (8,  0, 0), 2),
                stoptime("T1", 2, "B", (8, 10, 0), (8, 10, 0), 3),
                stoptime("T1", 3, "A", (8, 20, 0), (8, 20, 0), 4),
            ],
        );
        let result = analyze(&records, &empty_derived(), &default_config(), 20260514);
        assert!(!result.notices.iter().any(|n| n.rule_id == "OPR_007"),
            "Ring hatta terminal tekrarı OPR_007 üretmemeli");
    }

    // ── WP-09d: STM_027 ─────────────────────────────────────────────────────

    fn stoptime_with_dist(trip_id: &str, seq: u32, stop_id: &str, arr: (u32,u32,u32), dep: (u32,u32,u32), dist: f64, line: u64) -> StopTimeRecord {
        let mut st = stoptime(trip_id, seq, stop_id, arr, dep, line);
        st.shape_dist_traveled = Some(dist);
        st
    }

    #[test]
    fn non_monotone_dist_produces_stm_027() {
        let records = records_with(
            vec![stop("A", 41.0, 29.0), stop("B", 41.1, 29.1), stop("C", 41.2, 29.2)],
            vec![route("R1", 3)],
            vec![trip("T1", "R1")],
            vec![
                stoptime_with_dist("T1", 1, "A", (8,0,0), (8,0,0), 0.0, 2),
                stoptime_with_dist("T1", 2, "B", (8,10,0), (8,10,0), 1.0, 3),
                stoptime_with_dist("T1", 3, "C", (8,20,0), (8,20,0), 0.5, 4), // geriye gidiş
            ],
        );
        let result = analyze(&records, &empty_derived(), &default_config(), 20260514);
        assert!(result.notices.iter().any(|n| n.rule_id == "STM_027"));
    }

    #[test]
    fn monotone_dist_no_stm_027() {
        let records = records_with(
            vec![stop("A", 41.0, 29.0), stop("B", 41.1, 29.1), stop("C", 41.2, 29.2)],
            vec![route("R1", 3)],
            vec![trip("T1", "R1")],
            vec![
                stoptime_with_dist("T1", 1, "A", (8,0,0), (8,0,0), 0.0, 2),
                stoptime_with_dist("T1", 2, "B", (8,10,0), (8,10,0), 1.0, 3),
                stoptime_with_dist("T1", 3, "C", (8,20,0), (8,20,0), 2.0, 4),
            ],
        );
        let result = analyze(&records, &empty_derived(), &default_config(), 20260514);
        assert!(!result.notices.iter().any(|n| n.rule_id == "STM_027"));
    }

    // ── WP-09d: STM_013 ─────────────────────────────────────────────────────

    fn stoptime_no_time(trip_id: &str, seq: u32, stop_id: &str, line: u64) -> StopTimeRecord {
        StopTimeRecord {
            trip_id: trip_id.into(), stop_id: stop_id.into(),
            stop_sequence: Some(seq),
            line,
            ..Default::default()
        }
    }

    #[test]
    fn mixed_timing_produces_stm_013() {
        let records = records_with(
            vec![stop("A", 41.0, 29.0), stop("B", 41.1, 29.1), stop("C", 41.2, 29.2)],
            vec![route("R1", 3)],
            vec![trip("T1", "R1")],
            vec![
                stoptime("T1", 1, "A", (8,0,0), (8,0,0), 2),
                stoptime_no_time("T1", 2, "B", 3), // orta durak zamanı eksik
                stoptime("T1", 3, "C", (8,20,0), (8,20,0), 4),
            ],
        );
        let result = analyze(&records, &empty_derived(), &default_config(), 20260514);
        assert!(result.notices.iter().any(|n| n.rule_id == "STM_013"));
    }

    // ── WP-09d: TRP_011 / TRP_013 / RTS_016 ────────────────────────────────

    #[test]
    fn trip_without_headsign_produces_trp_011() {
        let records = records_with(
            vec![stop("A", 41.0, 29.0), stop("B", 41.1, 29.1)],
            vec![route("R1", 3)],
            vec![trip("T1", "R1")], // trip() headsign=None, short=None
            vec![
                stoptime("T1", 1, "A", (8,0,0), (8,0,0), 2),
                stoptime("T1", 2, "B", (8,10,0), (8,10,0), 3),
            ],
        );
        let result = analyze(&records, &empty_derived(), &default_config(), 20260514);
        assert!(result.notices.iter().any(|n| n.rule_id == "TRP_011"));
    }

    #[test]
    fn single_trip_route_produces_trp_013() {
        let records = records_with(
            vec![stop("A", 41.0, 29.0), stop("B", 41.1, 29.1)],
            vec![route("R1", 3)],
            vec![trip("T1", "R1")], // tek sefer
            vec![
                stoptime("T1", 1, "A", (8,0,0), (8,0,0), 2),
                stoptime("T1", 2, "B", (8,10,0), (8,10,0), 3),
            ],
        );
        let result = analyze(&records, &empty_derived(), &default_config(), 20260514);
        assert!(result.notices.iter().any(|n| n.rule_id == "TRP_013"));
    }

    #[test]
    fn route_without_active_service_produces_RTS_016() {
        // Sefer var ama service_id'si calendar_bitmap'te yok
        let records = records_with(
            vec![stop("A", 41.0, 29.0), stop("B", 41.1, 29.1)],
            vec![route("R1", 3)],
            vec![trip("T1", "R1")],
            vec![
                stoptime("T1", 1, "A", (8,0,0), (8,0,0), 2),
                stoptime("T1", 2, "B", (8,10,0), (8,10,0), 3),
            ],
        );
        // calendar_bitmap boş → aktif gün yok
        let derived = DerivedData::default();
        let result = analyze(&records, &derived, &default_config(), 20260514);
        assert!(result.notices.iter().any(|n| n.rule_id == "RTS_016"));
    }

    // ── WP-09d: DQ_005 ──────────────────────────────────────────────────────

    #[test]
    fn no_active_trips_produces_dq_005() {
        use crate::k5_derived::CalendarBitmap;
        let records = records_with(
            vec![stop("A", 41.0, 29.0)],
            vec![route("R1", 3)],
            vec![trip("T1", "R1")],
            vec![stoptime("T1", 1, "A", (8,0,0), (8,0,0), 2)],
        );
        // Tüm calendar_bitmap tarihleri geçmişte
        let mut derived = DerivedData::default();
        derived.calendar_bitmap = CalendarBitmap {
            active_dates: [("SVC".to_string(),
                [20250101u32].into_iter().collect())]
                .into_iter().collect(),
        };
        let result = analyze(&records, &derived, &default_config(), 20260514);
        assert!(result.notices.iter().any(|n| n.rule_id == "DQ_005"));
    }

    // ── WP-09e: Entegrasyon smoke testleri ──────────────────────────────────

    #[test]
    fn config_change_changes_stm_014_threshold() {
        // Aynı feed, farklı config → farklı sonuç (config değişikliği yalnızca K6'yı etkiler)
        let records = records_with(
            vec![stop("A", 41.0, 29.0), stop("B", 41.05, 29.05)],
            vec![route("R1", 3)],
            vec![trip("T1", "R1")],
            vec![
                stoptime("T1", 1, "A", (8,0,0), (8,0,0), 2),
                stoptime("T1", 2, "B", (8,5,0), (8,5,0), 3),
            ],
        );
        // Bu sefer için ~8 km/h
        let result_default = analyze(&records, &empty_derived(), &default_config(), 20260514);
        assert!(!result_default.notices.iter().any(|n| n.rule_id == "STM_014"),
            "Default config'te STM_014 olmamalı");

        // Çok düşük hız eşiği → STM_014 tetiklenmeli
        let strict_cfg = ValidatorConfig { max_speed_bus_kmh: 1.0, ..default_config() };
        let result_strict = analyze(&records, &empty_derived(), &strict_cfg, 20260514);
        assert!(result_strict.notices.iter().any(|n| n.rule_id == "STM_014"),
            "Düşük eşikli config'te STM_014 olmalı");
    }

    #[test]
    fn full_pipeline_smoke_no_panic() {
        // Tüm fonksiyonlar boş kayıtlarla paniksiz çalışmalı
        let result = analyze(
            &crate::k2::EntityRecords::default(),
            &DerivedData::default(),
            &default_config(),
            20260514,
        );
        // Boş feed'de notice üretilmemeli
        assert!(result.notices.is_empty());
    }

    // ── OPR_011: service_id aktif gün içermiyor ──────────────────────────────

    #[test]
    fn service_with_no_active_dates_produces_opr_011() {
        let records = records_with(
            vec![stop("A", 41.0, 29.0), stop("B", 41.1, 29.1)],
            vec![route("R1", 3)],
            vec![trip("T1", "R1")], // service_id = "SVC"
            vec![
                stoptime("T1", 1, "A", (8,0,0), (8,0,0), 2),
                stoptime("T1", 2, "B", (8,10,0), (8,10,0), 3),
            ],
        );
        // calendar_bitmap'te SVC yok → aktif gün yok
        let derived = DerivedData::default();
        let result = analyze(&records, &derived, &default_config(), 20260514);
        assert!(result.notices.iter().any(|n| n.rule_id == "OPR_011"), "OPR_011 olmalı");
    }

    #[test]
    fn service_with_active_dates_no_opr_011() {
        use crate::k5_derived::CalendarBitmap;
        let records = records_with(
            vec![stop("A", 41.0, 29.0), stop("B", 41.1, 29.1)],
            vec![route("R1", 3)],
            vec![trip("T1", "R1")],
            vec![
                stoptime("T1", 1, "A", (8,0,0), (8,0,0), 2),
                stoptime("T1", 2, "B", (8,10,0), (8,10,0), 3),
            ],
        );
        let mut derived = DerivedData::default();
        derived.calendar_bitmap = CalendarBitmap {
            active_dates: [("SVC".to_string(), [20261001u32].into_iter().collect())]
                .into_iter().collect(),
        };
        let result = analyze(&records, &derived, &default_config(), 20260514);
        assert!(!result.notices.iter().any(|n| n.rule_id == "OPR_011"));
    }

    // ── OPR_016: feed genelinde hiç aktif servis yok ─────────────────────────

    #[test]
    fn no_active_services_at_all_produces_opr_016() {
        let records = records_with(
            vec![stop("A", 41.0, 29.0)],
            vec![route("R1", 3)],
            vec![trip("T1", "R1")],
            vec![stoptime("T1", 1, "A", (8,0,0), (8,0,0), 2)],
        );
        // calendar_bitmap tamamen boş
        let result = analyze(&records, &DerivedData::default(), &default_config(), 20260514);
        assert!(result.notices.iter().any(|n| n.rule_id == "OPR_016"), "OPR_016 olmalı");
    }

    #[test]
    fn some_active_service_no_opr_016() {
        use crate::k5_derived::CalendarBitmap;
        let records = records_with(
            vec![stop("A", 41.0, 29.0)],
            vec![route("R1", 3)],
            vec![trip("T1", "R1")],
            vec![stoptime("T1", 1, "A", (8,0,0), (8,0,0), 2)],
        );
        let mut derived = DerivedData::default();
        derived.calendar_bitmap = CalendarBitmap {
            active_dates: [("SVC".to_string(), [20261001u32].into_iter().collect())]
                .into_iter().collect(),
        };
        let result = analyze(&records, &derived, &default_config(), 20260514);
        assert!(!result.notices.iter().any(|n| n.rule_id == "OPR_016"));
    }

    // ── DQ_009: feed'de stop_times yok ───────────────────────────────────────

    #[test]
    fn trips_with_no_stop_times_produces_dq_009() {
        let records = records_with(
            vec![stop("A", 41.0, 29.0)],
            vec![route("R1", 3)],
            vec![trip("T1", "R1")], // trip var
            vec![],                 // stop_times yok
        );
        let result = analyze(&records, &DerivedData::default(), &default_config(), 20260514);
        assert!(result.notices.iter().any(|n| n.rule_id == "DQ_009"), "DQ_009 olmalı");
    }

    #[test]
    fn no_trips_no_dq_009() {
        // Sefer yoksa DQ_009 tetiklenmemeli (koşul: trips var + stop_times yok)
        let records = records_with(vec![], vec![], vec![], vec![]);
        let result = analyze(&records, &DerivedData::default(), &default_config(), 20260514);
        assert!(!result.notices.iter().any(|n| n.rule_id == "DQ_009"));
    }

    // ── STM_017: shape olan trip'te shape_dist_traveled eksik ───────────────

    #[test]
    fn trip_with_shape_but_no_dist_produces_stm_017() {
        use crate::k2::shapes::ShapePointRecord;
        let mut records = records_with(
            vec![stop("A", 41.0, 29.0), stop("B", 41.1, 29.1)],
            vec![route("R1", 3)],
            vec![{
                let mut t = trip("T1", "R1");
                t.shape_id = Some("S1".into());
                t
            }],
            vec![
                stoptime("T1", 1, "A", (8,0,0), (8,0,0), 2),
                stoptime("T1", 2, "B", (8,10,0), (8,10,0), 3),
            ],
        );
        records.shapes = vec![
            ShapePointRecord { shape_id: "S1".into(), shape_pt_lat: Some(41.0), shape_pt_lon: Some(29.0),
                shape_pt_sequence: Some(1), shape_dist_traveled: None, line: 2 },
            ShapePointRecord { shape_id: "S1".into(), shape_pt_lat: Some(41.1), shape_pt_lon: Some(29.1),
                shape_pt_sequence: Some(2), shape_dist_traveled: None, line: 3 },
        ];
        let result = analyze(&records, &empty_derived(), &default_config(), 20260514);
        assert!(result.notices.iter().any(|n| n.rule_id == "STM_017"), "STM_017 olmalı");
    }

    #[test]
    fn trip_without_shape_no_stm_017() {
        let records = records_with(
            vec![stop("A", 41.0, 29.0), stop("B", 41.1, 29.1)],
            vec![route("R1", 3)],
            vec![trip("T1", "R1")], // shape_id yok
            vec![
                stoptime("T1", 1, "A", (8,0,0), (8,0,0), 2),
                stoptime("T1", 2, "B", (8,10,0), (8,10,0), 3),
            ],
        );
        let result = analyze(&records, &empty_derived(), &default_config(), 20260514);
        assert!(!result.notices.iter().any(|n| n.rule_id == "STM_017"));
    }

    // ── GEO_007: çok büyük shape atlaması (severe = 3× eşik) ────────────────

    #[test]
    fn severe_shape_jump_produces_geo_007() {
        use crate::k5_derived::{ShapeGeometry, ShapeSegments};
        let mut derived = DerivedData::default();
        // max_shape_jump_km default = 5.0 → severe = 15.0 → 50 km segment tetikler
        derived.shape_geometry = ShapeGeometry {
            shapes: [("S1".to_string(), ShapeSegments {
                segment_distances_km: vec![50.0],
                total_length_km: 50.0,
                bbox: (41.0, 41.5, 29.0, 29.5),
            })].into_iter().collect(),
        };
        let records = crate::k2::EntityRecords::default();
        let result = analyze(&records, &derived, &default_config(), 20260514);
        assert!(result.notices.iter().any(|n| n.rule_id == "GEO_007"), "GEO_007 olmalı");
    }

    #[test]
    fn normal_shape_jump_no_geo_007() {
        use crate::k5_derived::{ShapeGeometry, ShapeSegments};
        let mut derived = DerivedData::default();
        // 2 km < 15 km severe threshold → tetiklenmemeli
        derived.shape_geometry = ShapeGeometry {
            shapes: [("S1".to_string(), ShapeSegments {
                segment_distances_km: vec![2.0],
                total_length_km: 2.0,
                bbox: (41.0, 41.02, 29.0, 29.02),
            })].into_iter().collect(),
        };
        let records = crate::k2::EntityRecords::default();
        let result = analyze(&records, &derived, &default_config(), 20260514);
        assert!(!result.notices.iter().any(|n| n.rule_id == "GEO_007"));
    }

    // ── GEO_009 / SHP_013: durak shape'ten çok uzak ─────────────────────────

    #[test]
    fn stop_far_from_shape_produces_geo_009() {
        use crate::k2::shapes::ShapePointRecord;
        // Durak İstanbul'da, shape Ankara'da → ~350 km uzak
        let mut records = records_with(
            vec![stop("IST", 41.0, 29.0)],
            vec![route("R1", 3)],
            vec![{
                let mut t = trip("T1", "R1");
                t.shape_id = Some("S1".into());
                t
            }],
            vec![stoptime("T1", 1, "IST", (8,0,0), (8,0,0), 2)],
        );
        records.shapes = vec![
            ShapePointRecord { shape_id: "S1".into(), shape_pt_lat: Some(39.9), shape_pt_lon: Some(32.9),
                shape_pt_sequence: Some(1), shape_dist_traveled: None, line: 2 },
        ];
        // default stop_far_from_shape_m = 150.0 → 350 000 m >> 150 m
        let result = analyze(&records, &empty_derived(), &default_config(), 20260514);
        assert!(result.notices.iter().any(|n| n.rule_id == "GEO_009"), "GEO_009 olmalı");
        assert!(!result.notices.iter().any(|n| n.rule_id == "SHP_013"), "SHP_013 artık üretilmemeli");
    }

    #[test]
    fn stop_near_shape_no_geo_009() {
        use crate::k2::shapes::ShapePointRecord;
        let mut records = records_with(
            vec![stop("A", 41.0, 29.0)],
            vec![route("R1", 3)],
            vec![{
                let mut t = trip("T1", "R1");
                t.shape_id = Some("S1".into());
                t
            }],
            vec![stoptime("T1", 1, "A", (8,0,0), (8,0,0), 2)],
        );
        // Shape noktası durağın hemen yanında (< 1 m)
        records.shapes = vec![
            ShapePointRecord { shape_id: "S1".into(), shape_pt_lat: Some(41.0), shape_pt_lon: Some(29.0),
                shape_pt_sequence: Some(1), shape_dist_traveled: None, line: 2 },
        ];
        let result = analyze(&records, &empty_derived(), &default_config(), 20260514);
        assert!(!result.notices.iter().any(|n| n.rule_id == "GEO_009"));
        assert!(!result.notices.iter().any(|n| n.rule_id == "SHP_013"));
    }

    // ── GEO_013: feed koordinat özeti (Bilgi) ───────────────────────────────

    #[test]
    fn stops_with_coords_produce_geo_013() {
        let records = records_with(
            vec![stop("A", 41.0, 29.0)],
            vec![route("R1", 3)],
            vec![],
            vec![],
        );
        let result = analyze(&records, &empty_derived(), &default_config(), 20260514);
        assert!(result.notices.iter().any(|n| n.rule_id == "GEO_013"), "GEO_013 olmalı");
    }

    // ── OPR_005: rota ortalama headway bilgisi (Bilgi) ───────────────────────

    #[test]
    fn multiple_trips_same_route_produces_opr_005() {
        let records = records_with(
            vec![stop("A", 41.0, 29.0), stop("B", 41.1, 29.1)],
            vec![route("R1", 3)],
            vec![trip("T1", "R1"), trip("T2", "R1")],
            vec![
                stoptime("T1", 1, "A", (8,0,0), (8,0,0), 2),
                stoptime("T1", 2, "B", (8,10,0), (8,10,0), 3),
                stoptime("T2", 1, "A", (9,0,0), (9,0,0), 4),
                stoptime("T2", 2, "B", (9,10,0), (9,10,0), 5),
            ],
        );
        let result = analyze(&records, &empty_derived(), &default_config(), 20260514);
        assert!(result.notices.iter().any(|n| n.rule_id == "OPR_005"), "OPR_005 olmalı");
    }

    // ── OPR_013: tek yönlü rota bilgisi ─────────────────────────────────────

    #[test]
    fn single_direction_route_produces_opr_013() {
        let records = records_with(
            vec![stop("A", 41.0, 29.0), stop("B", 41.1, 29.1)],
            vec![route("R1", 3)],
            vec![{
                let mut t = trip("T1", "R1");
                t.direction_id = Some(0);
                t
            }],
            vec![
                stoptime("T1", 1, "A", (8,0,0), (8,0,0), 2),
                stoptime("T1", 2, "B", (8,10,0), (8,10,0), 3),
            ],
        );
        let result = analyze(&records, &empty_derived(), &default_config(), 20260514);
        assert!(result.notices.iter().any(|n| n.rule_id == "OPR_013"), "OPR_013 olmalı");
    }

    // ── DQ_005b: hiçbir trip için stop_times yok ─────────────────────────────

    #[test]
    fn trips_without_stoptimes_produces_dq_005b() {
        // Trip var, stop_times yok
        let records = records_with(
            vec![stop("A", 41.0, 29.0)],
            vec![route("R1", 3)],
            vec![trip("T1", "R1")],
            vec![], // stop_times boş
        );
        let result = analyze(&records, &empty_derived(), &default_config(), 20260514);
        assert!(result.notices.iter().any(|n| n.rule_id == "DQ_005b"), "DQ_005b olmalı");
    }

    // ── DQ_005c: koordinatsız durak oranı > %50 ──────────────────────────────

    #[test]
    fn mostly_coord_missing_stops_produces_dq_005c() {
        use crate::k2::stops::StopRecord;
        let no_coord = StopRecord {
            stop_id: "X".into(), stop_code: None, stop_name: None,
            stop_lat: None, stop_lon: None, location_type: None,
            stop_timezone: None, wheelchair_boarding: None, stop_access: None,
            level_id: None, tts_stop_name: None, row: Default::default(), line: 3,
            ..Default::default()
        };
        let records = records_with(
            vec![stop("A", 41.0, 29.0), no_coord.clone(), {
                let mut s = no_coord.clone(); s.stop_id = "Y".into(); s
            }],
            vec![],
            vec![],
            vec![],
        );
        // 1 koordinatlı, 2 koordinatsız → %67 > %50 → DQ_005c
        let result = analyze(&records, &empty_derived(), &default_config(), 20260514);
        assert!(result.notices.iter().any(|n| n.rule_id == "DQ_005c"), "DQ_005c olmalı");
    }

    // ── DQ_010: agency_id hiçbir rotada kullanılmıyor ────────────────────────

    #[test]
    fn unused_agency_id_produces_dq_010() {
        use crate::k2::agency::AgencyRecord;
        let mut records = records_with(
            vec![],
            vec![route("R1", 3)], // agency_id = None
            vec![],
            vec![],
        );
        records.agencies = vec![AgencyRecord {
            agency_id: Some("AG1".into()),
            agency_name: "Test Agency".into(),
            agency_url: "https://test.example".into(),
            agency_timezone: "Europe/Istanbul".into(),
            agency_lang: None, agency_phone: None, agency_fare_url: None,
            agency_email: None, agency_cemv_support: None,
            row: Default::default(), line: 2,
        }];
        let result = analyze(&records, &empty_derived(), &default_config(), 20260514);
        assert!(result.notices.iter().any(|n| n.rule_id == "DQ_010"), "DQ_010 olmalı");
    }

    // ── RTS_017: shape'siz route ─────────────────────────────────────────────

    #[test]
    fn route_without_shape_produces_RTS_017() {
        // Trip'in shape_id yok → route shape'siz
        let records = records_with(
            vec![stop("A", 41.0, 29.0), stop("B", 41.1, 29.1)],
            vec![route("R1", 3)],
            vec![trip("T1", "R1")], // shape_id = None
            vec![
                stoptime("T1", 1, "A", (8,0,0), (8,0,0), 2),
                stoptime("T1", 2, "B", (8,10,0), (8,10,0), 3),
            ],
        );
        let result = analyze(&records, &empty_derived(), &default_config(), 20260514);
        assert!(result.notices.iter().any(|n| n.rule_id == "RTS_017"), "RTS_017 olmalı");
    }

    #[test]
    fn route_with_shape_no_RTS_017() {
        let records = records_with(
            vec![stop("A", 41.0, 29.0), stop("B", 41.1, 29.1)],
            vec![route("R1", 3)],
            vec![{
                let mut t = trip("T1", "R1");
                t.shape_id = Some("S1".into());
                t
            }],
            vec![
                stoptime("T1", 1, "A", (8,0,0), (8,0,0), 2),
                stoptime("T1", 2, "B", (8,10,0), (8,10,0), 3),
            ],
        );
        let result = analyze(&records, &empty_derived(), &default_config(), 20260514);
        assert!(!result.notices.iter().any(|n| n.rule_id == "RTS_017"));
    }

    // ── TRP_012: çift yönlü rotada direction_id eksik sefer ──────────────────

    #[test]
    fn bidirectional_route_missing_direction_produces_trp_012() {
        let records = records_with(
            vec![stop("A", 41.0, 29.0), stop("B", 41.1, 29.1)],
            vec![route("R1", 3)],
            vec![
                { let mut t = trip("T1", "R1"); t.direction_id = Some(0); t },
                { let mut t = trip("T2", "R1"); t.direction_id = Some(1); t },
                trip("T3", "R1"), // direction_id = None — bu rotanın diğerleri var
            ],
            vec![
                stoptime("T1", 1, "A", (8,0,0), (8,0,0), 2),
                stoptime("T2", 1, "B", (9,0,0), (9,0,0), 3),
                stoptime("T3", 1, "A", (10,0,0), (10,0,0), 4),
            ],
        );
        let result = analyze(&records, &empty_derived(), &default_config(), 20260514);
        assert!(result.notices.iter().any(|n| n.rule_id == "TRP_012"), "TRP_012 olmalı");
    }

    // ── TRP_015: block_id singleton ──────────────────────────────────────────

    #[test]
    fn singleton_block_produces_trp_015() {
        let records = records_with(
            vec![stop("A", 41.0, 29.0), stop("B", 41.1, 29.1)],
            vec![route("R1", 3)],
            vec![{
                let mut t = trip("T1", "R1");
                t.block_id = Some("BLK1".into()); // block'ta tek sefer
                t
            }],
            vec![
                stoptime("T1", 1, "A", (8,0,0), (8,0,0), 2),
                stoptime("T1", 2, "B", (8,10,0), (8,10,0), 3),
            ],
        );
        let result = analyze(&records, &empty_derived(), &default_config(), 20260514);
        assert!(result.notices.iter().any(|n| n.rule_id == "TRP_015"), "TRP_015 olmalı");
    }

    #[test]
    fn shared_block_no_trp_015() {
        let records = records_with(
            vec![stop("A", 41.0, 29.0), stop("B", 41.1, 29.1)],
            vec![route("R1", 3)],
            vec![
                { let mut t = trip("T1", "R1"); t.block_id = Some("BLK1".into()); t },
                { let mut t = trip("T2", "R1"); t.block_id = Some("BLK1".into()); t },
            ],
            vec![
                stoptime("T1", 1, "A", (8,0,0), (8,0,0), 2),
                stoptime("T2", 1, "B", (9,0,0), (9,0,0), 3),
            ],
        );
        let result = analyze(&records, &empty_derived(), &default_config(), 20260514);
        assert!(!result.notices.iter().any(|n| n.rule_id == "TRP_015"));
    }

    // ── STP_020: kullanılmayan durak ─────────────────────────────────────────

    #[test]
    fn unused_stop_produces_stp_020() {
        // Stop "X" stop_times'da yok → STP_020
        let records = records_with(
            vec![stop("A", 41.0, 29.0), stop("X", 41.5, 29.5)],
            vec![route("R1", 3)],
            vec![trip("T1", "R1")],
            vec![
                stoptime("T1", 1, "A", (8,0,0), (8,0,0), 2),
                stoptime("T1", 2, "A", (8,10,0), (8,10,0), 3),
            ],
        );
        let result = analyze(&records, &empty_derived(), &default_config(), 20260514);
        assert!(result.notices.iter().any(|n| n.rule_id == "STP_020" && n.entity_id.as_deref() == Some("X")),
            "STP_020 olmalı");
        assert!(!result.notices.iter().any(|n| n.rule_id == "STP_020" && n.entity_id.as_deref() == Some("A")),
            "kullanılan durak STP_020 üretmemeli");
    }

    #[test]
    fn parent_station_not_flagged_stp_020() {
        // location_type=1 (parent station) stop_times'da kullanılmaz — bu normal
        let mut parent = stop("PS1", 41.0, 29.0);
        parent.location_type = Some(1);
        let records = records_with(
            vec![parent, stop("A", 41.0, 29.0)],
            vec![route("R1", 3)],
            vec![trip("T1", "R1")],
            vec![
                stoptime("T1", 1, "A", (8,0,0), (8,0,0), 2),
                stoptime("T1", 2, "A", (8,10,0), (8,10,0), 3),
            ],
        );
        let result = analyze(&records, &empty_derived(), &default_config(), 20260514);
        assert!(!result.notices.iter().any(|n| n.rule_id == "STP_020" && n.entity_id.as_deref() == Some("PS1")),
            "parent station STP_020 üretmemeli");
    }

    // ── SHP_017: durak sırası shape projeksiyonuyla çelişiyor ────────────────

    #[test]
    fn stops_in_correct_shape_order_no_shp_017() {
        use crate::k2::shapes::ShapePointRecord;
        // Shape: (41.0,29.0) → (41.0,29.1) (doğuya giden düz çizgi)
        // Stop A: 29.0 (başlangıç), Stop B: 29.1 (bitiş) — doğru sıra
        let mut records = records_with(
            vec![stop("A", 41.0, 29.0), stop("B", 41.0, 29.1)],
            vec![route("R1", 3)],
            vec![{ let mut t = trip("T1", "R1"); t.shape_id = Some("S1".into()); t }],
            vec![
                stoptime("T1", 1, "A", (8,0,0), (8,0,0), 2),
                stoptime("T1", 2, "B", (8,10,0), (8,10,0), 3),
            ],
        );
        records.shapes = vec![
            ShapePointRecord { shape_id: "S1".into(), shape_pt_lat: Some(41.0), shape_pt_lon: Some(29.0),
                shape_pt_sequence: Some(1), shape_dist_traveled: None, line: 2 },
            ShapePointRecord { shape_id: "S1".into(), shape_pt_lat: Some(41.0), shape_pt_lon: Some(29.1),
                shape_pt_sequence: Some(2), shape_dist_traveled: None, line: 3 },
        ];
        let result = analyze(&records, &empty_derived(), &default_config(), 20260514);
        assert!(!result.notices.iter().any(|n| n.rule_id == "SHP_017"), "doğru sırada SHP_017 olmamalı");
    }

    #[test]
    fn fully_reversed_shape_produces_shp_016_not_shp_017() {
        use crate::k2::shapes::ShapePointRecord;
        // Shape: A(29.0) → B(29.1) — doğuya giden çizgi
        // stop_times: B önce, A sonra → shape tamamen ters takılmış → SHP_016
        // (SHP_016 fires + continue, SHP_017 üretilmez)
        let mut records = records_with(
            vec![stop("A", 41.0, 29.0), stop("B", 41.0, 29.1)],
            vec![route("R1", 3)],
            vec![{ let mut t = trip("T1", "R1"); t.shape_id = Some("S1".into()); t }],
            vec![
                stoptime("T1", 1, "B", (8,0,0), (8,0,0), 2),
                stoptime("T1", 2, "A", (8,10,0), (8,10,0), 3),
            ],
        );
        records.shapes = vec![
            ShapePointRecord { shape_id: "S1".into(), shape_pt_lat: Some(41.0), shape_pt_lon: Some(29.0),
                shape_pt_sequence: Some(1), shape_dist_traveled: None, line: 2 },
            ShapePointRecord { shape_id: "S1".into(), shape_pt_lat: Some(41.0), shape_pt_lon: Some(29.1),
                shape_pt_sequence: Some(2), shape_dist_traveled: None, line: 3 },
        ];
        let result = analyze(&records, &empty_derived(), &default_config(), 20260514);
        assert!(result.notices.iter().any(|n| n.rule_id == "SHP_016"), "tamamen ters shape → SHP_016 olmalı");
        assert!(!result.notices.iter().any(|n| n.rule_id == "SHP_017"), "tamamen ters shape → SHP_017 olmamalı (SHP_016 öncelikli)");
    }

    #[test]
    fn partial_shape_order_violation_produces_shp_017() {
        use crate::k2::shapes::ShapePointRecord;
        // Shape: A(29.0) → B(29.05) → C(29.1) — 3 noktalı çizgi
        // stop_times: A(seq=1) → C(seq=2) → B(seq=3) → B, C'den sonra gelir ama shape'de önce
        // İlk durak A shape'in başında → SHP_016 tetiklenmez; B sırası bozuk → SHP_017
        let mut records = records_with(
            vec![stop("A", 41.0, 29.0), stop("B", 41.0, 29.05), stop("C", 41.0, 29.1)],
            vec![route("R1", 3)],
            vec![{ let mut t = trip("T1", "R1"); t.shape_id = Some("S1".into()); t }],
            vec![
                stoptime("T1", 1, "A", (8,0,0),  (8,0,0),  2),
                stoptime("T1", 2, "C", (8,10,0), (8,10,0), 3),
                stoptime("T1", 3, "B", (8,20,0), (8,20,0), 4),
            ],
        );
        records.shapes = vec![
            ShapePointRecord { shape_id: "S1".into(), shape_pt_lat: Some(41.0), shape_pt_lon: Some(29.0),
                shape_pt_sequence: Some(1), shape_dist_traveled: None, line: 2 },
            ShapePointRecord { shape_id: "S1".into(), shape_pt_lat: Some(41.0), shape_pt_lon: Some(29.05),
                shape_pt_sequence: Some(2), shape_dist_traveled: None, line: 3 },
            ShapePointRecord { shape_id: "S1".into(), shape_pt_lat: Some(41.0), shape_pt_lon: Some(29.1),
                shape_pt_sequence: Some(3), shape_dist_traveled: None, line: 4 },
        ];
        let result = analyze(&records, &empty_derived(), &default_config(), 20260514);
        assert!(result.notices.iter().any(|n| n.rule_id == "SHP_017"), "kısmi sıra ihlali → SHP_017 olmalı");
    }

    // ── STP_029: parent_station'dan çok uzak durak ───────────────────────────

    #[test]
    fn stop_far_from_parent_produces_stp_029() {
        let mut parent = stop("PS1", 41.0, 29.0);
        parent.location_type = Some(1);
        let mut child = stop("S1", 41.1, 29.1); // ~15 km uzakta — eşik aşıldı
        child.row.insert("parent_station".to_string(), "PS1".to_string());
        let records = records_with(
            vec![parent, child],
            vec![route("R1", 3)],
            vec![trip("T1", "R1")],
            vec![stoptime("T1", 1, "S1", (8,0,0), (8,0,0), 2)],
        );
        let result = analyze(&records, &empty_derived(), &default_config(), 20260514);
        assert!(
            result.notices.iter().any(|n| n.rule_id == "STP_029" && n.entity_id.as_deref() == Some("S1")),
            "STP_029 olmalı: {:?}", result.notices.iter().map(|n| &n.rule_id).collect::<Vec<_>>()
        );
    }

    #[test]
    fn stop_close_to_parent_no_stp_029() {
        let mut parent = stop("PS1", 41.0, 29.0);
        parent.location_type = Some(1);
        let mut child = stop("S1", 41.0001, 29.0001); // ~15m — eşik altı
        child.row.insert("parent_station".to_string(), "PS1".to_string());
        let records = records_with(
            vec![parent, child],
            vec![route("R1", 3)],
            vec![trip("T1", "R1")],
            vec![stoptime("T1", 1, "S1", (8,0,0), (8,0,0), 2)],
        );
        let result = analyze(&records, &empty_derived(), &default_config(), 20260514);
        assert!(!result.notices.iter().any(|n| n.rule_id == "STP_029"), "yakın durak STP_029 üretmemeli");
    }

    // ── STP_030: hiç çocuğu olmayan üst istasyon ─────────────────────────────

    #[test]
    fn station_without_children_produces_stp_030() {
        let mut station = stop("PS1", 41.0, 29.0);
        station.location_type = Some(1);
        let records = records_with(
            vec![station, stop("A", 41.0, 29.0)],
            vec![route("R1", 3)],
            vec![trip("T1", "R1")],
            vec![stoptime("T1", 1, "A", (8,0,0), (8,0,0), 2)],
        );
        let result = analyze(&records, &empty_derived(), &default_config(), 20260514);
        assert!(
            result.notices.iter().any(|n| n.rule_id == "STP_030" && n.entity_id.as_deref() == Some("PS1")),
            "STP_030 olmalı"
        );
    }

    #[test]
    fn station_with_children_no_stp_030() {
        let mut station = stop("PS1", 41.0, 29.0);
        station.location_type = Some(1);
        let mut child = stop("S1", 41.0, 29.0);
        child.row.insert("parent_station".to_string(), "PS1".to_string());
        let records = records_with(
            vec![station, child],
            vec![route("R1", 3)],
            vec![trip("T1", "R1")],
            vec![stoptime("T1", 1, "S1", (8,0,0), (8,0,0), 2)],
        );
        let result = analyze(&records, &empty_derived(), &default_config(), 20260514);
        assert!(!result.notices.iter().any(|n| n.rule_id == "STP_030"), "alt durağı olan istasyon STP_030 üretmemeli");
    }

    // ── TRP_020: headsign ara durak adıyla eşleşiyor ────────────────────────

    #[test]
    fn headsign_matches_intermediate_stop_produces_trp_020() {
        // Stop A (seq 1, ara), Stop B (seq 2, terminal)
        // headsign = "Stop A" (ara durak adı) → TRP_020 olmalı
        let mut sa = stop("A", 41.0, 29.0);
        sa.stop_name = Some("Stop A".into());
        let mut sb = stop("B", 41.1, 29.1);
        sb.stop_name = Some("Stop B".into());
        let mut t = trip("T1", "R1");
        t.trip_headsign = Some("Stop A".into()); // ara durak adı
        let records = records_with(
            vec![sa, sb],
            vec![route("R1", 3)],
            vec![t],
            vec![
                stoptime("T1", 1, "A", (8,0,0), (8,0,0), 2),
                stoptime("T1", 2, "B", (8,10,0), (8,10,0), 3),
            ],
        );
        let result = analyze(&records, &empty_derived(), &default_config(), 20260514);
        assert!(
            result.notices.iter().any(|n| n.rule_id == "TRP_020"),
            "TRP_020 olmalı: {:?}", result.notices.iter().map(|n| &n.rule_id).collect::<Vec<_>>()
        );
    }

    #[test]
    fn headsign_matches_terminal_stop_no_trp_020() {
        // headsign = terminal durak adı → TRP_020 olmamalı
        let mut sa = stop("A", 41.0, 29.0);
        sa.stop_name = Some("Stop A".into());
        let mut sb = stop("B", 41.1, 29.1);
        sb.stop_name = Some("Stop B".into());
        let mut t = trip("T1", "R1");
        t.trip_headsign = Some("Stop B".into()); // terminal durak adı
        let records = records_with(
            vec![sa, sb],
            vec![route("R1", 3)],
            vec![t],
            vec![
                stoptime("T1", 1, "A", (8,0,0), (8,0,0), 2),
                stoptime("T1", 2, "B", (8,10,0), (8,10,0), 3),
            ],
        );
        let result = analyze(&records, &empty_derived(), &default_config(), 20260514);
        assert!(!result.notices.iter().any(|n| n.rule_id == "TRP_020"), "terminal eşleşmede TRP_020 olmamalı");
    }

    #[test]
    fn headsign_no_stop_match_no_trp_020() {
        // headsign hiçbir durak adıyla eşleşmiyor → TRP_020 olmamalı
        let mut sa = stop("A", 41.0, 29.0);
        sa.stop_name = Some("Stop A".into());
        let mut sb = stop("B", 41.1, 29.1);
        sb.stop_name = Some("Stop B".into());
        let mut t = trip("T1", "R1");
        t.trip_headsign = Some("Tren Garı".into());
        let records = records_with(
            vec![sa, sb],
            vec![route("R1", 3)],
            vec![t],
            vec![
                stoptime("T1", 1, "A", (8,0,0), (8,0,0), 2),
                stoptime("T1", 2, "B", (8,10,0), (8,10,0), 3),
            ],
        );
        let result = analyze(&records, &empty_derived(), &default_config(), 20260514);
        assert!(!result.notices.iter().any(|n| n.rule_id == "TRP_020"), "eşleşme yoksa TRP_020 olmamalı");
    }

    // ── SHP_022: durak shape'te birden fazla eşleşme bölgesine yakın ─────────

    #[test]
    fn stop_near_two_shape_sections_produces_shp_022() {
        // "U" şekli: gidip dönen hat — durak hem gidiş hem dönüş bölümüne yakın
        // shape_dist_traveled YOK → SHP_022 beklenir
        use crate::k2::shapes::ShapePointRecord;
        let mut t = trip("T1", "R1");
        t.shape_id = Some("SH1".into());
        // Koordinatlar: lon=0.0 ekseni boyunca gidip, 0.001° ≈ 111m sağa kayıp dönüyor
        // Durak: lat=0.5, lon=0.00005 — hem gidiş (lon≈0) hem dönüş (lon≈0.001) segmentine ~5m yakın
        let mut s = stop("S1", 0.5, 0.00005);
        s.stop_name = Some("Orta Durak".into());
        let mut records = records_with(
            vec![s],
            vec![route("R1", 3)],
            vec![t],
            vec![
                stoptime("T1", 1, "S1", (8, 0, 0), (8, 0, 0), 2),
                stoptime("T1", 2, "S1", (8, 10, 0), (8, 10, 0), 3),
            ],
        );
        // shape: (0,0)→(1,0)→(1,0.001)→(0,0.001) — U benzeri
        // İki dikey kenar (lon=0 ve lon=0.001) stop'a ~5m uzaklıkta
        records.shapes = vec![
            ShapePointRecord { shape_id: "SH1".into(), shape_pt_lat: Some(0.0), shape_pt_lon: Some(0.0),    shape_pt_sequence: Some(1), shape_dist_traveled: None, line: 2 },
            ShapePointRecord { shape_id: "SH1".into(), shape_pt_lat: Some(1.0), shape_pt_lon: Some(0.0),    shape_pt_sequence: Some(2), shape_dist_traveled: None, line: 3 },
            ShapePointRecord { shape_id: "SH1".into(), shape_pt_lat: Some(1.0), shape_pt_lon: Some(0.001),  shape_pt_sequence: Some(3), shape_dist_traveled: None, line: 4 },
            ShapePointRecord { shape_id: "SH1".into(), shape_pt_lat: Some(0.0), shape_pt_lon: Some(0.001),  shape_pt_sequence: Some(4), shape_dist_traveled: None, line: 5 },
        ];
        let result = analyze(&records, &empty_derived(), &default_config(), 20260514);
        assert!(
            result.notices.iter().any(|n| n.rule_id == "SHP_022"),
            "U-şekli + shape_dist_traveled eksik → SHP_022 beklenir: {:?}",
            result.notices.iter().map(|n| &n.rule_id).collect::<Vec<_>>()
        );
    }

    #[test]
    fn stop_near_one_shape_section_no_shp_022() {
        // Düz hat — stop yalnızca bir bölüme yakın → SHP_022 olmamalı
        use crate::k2::shapes::ShapePointRecord;
        let mut t = trip("T1", "R1");
        t.shape_id = Some("SH2".into());
        let mut s = stop("S1", 0.5, 0.0001); // sadece ilk segmente yakın
        s.stop_name = Some("Tek Bölüm".into());
        let mut records = records_with(
            vec![s],
            vec![route("R1", 3)],
            vec![t],
            vec![
                stoptime("T1", 1, "S1", (8, 0, 0), (8, 0, 0), 2),
                stoptime("T1", 2, "S1", (8, 10, 0), (8, 10, 0), 3),
            ],
        );
        // Düz shape, stop sadece ilk segmente (~11m) yakın
        records.shapes = vec![
            ShapePointRecord { shape_id: "SH2".into(), shape_pt_lat: Some(0.0), shape_pt_lon: Some(0.0),  shape_pt_sequence: Some(1), shape_dist_traveled: None, line: 2 },
            ShapePointRecord { shape_id: "SH2".into(), shape_pt_lat: Some(1.0), shape_pt_lon: Some(0.0),  shape_pt_sequence: Some(2), shape_dist_traveled: None, line: 3 },
            ShapePointRecord { shape_id: "SH2".into(), shape_pt_lat: Some(2.0), shape_pt_lon: Some(0.0),  shape_pt_sequence: Some(3), shape_dist_traveled: None, line: 4 },
        ];
        let result = analyze(&records, &empty_derived(), &default_config(), 20260514);
        assert!(
            !result.notices.iter().any(|n| n.rule_id == "SHP_022"),
            "Düz hat → SHP_022 olmamalı"
        );
    }

    // ── DQ_018: tamamen büyük harf metin alanı ───────────────────────────────

    #[test]
    fn all_caps_stop_name_produces_dq_018() {
        let mut s = stop("S1", 41.0, 29.0);
        s.stop_name = Some("MERKEZ DURAK".into());
        let records = records_with(
            vec![s],
            vec![route("R1", 3)],
            vec![trip("T1", "R1")],
            vec![stoptime("T1", 1, "S1", (8,0,0), (8,0,0), 2)],
        );
        let result = analyze(&records, &empty_derived(), &default_config(), 20260514);
        assert!(result.notices.iter().any(|n| n.rule_id == "DQ_018"), "DQ_018 olmalı");
    }

    #[test]
    fn mixed_case_stop_name_no_dq_018() {
        let mut s = stop("S1", 41.0, 29.0);
        s.stop_name = Some("Merkez Durak".into());
        let records = records_with(
            vec![s],
            vec![route("R1", 3)],
            vec![trip("T1", "R1")],
            vec![stoptime("T1", 1, "S1", (8,0,0), (8,0,0), 2)],
        );
        let result = analyze(&records, &empty_derived(), &default_config(), 20260514);
        assert!(!result.notices.iter().any(|n| n.rule_id == "DQ_018"), "düzgün harf DQ_018 üretmemeli");
    }

    #[test]
    fn single_letter_stop_name_no_dq_018() {
        // Tek harfli kısaltmalar (ör. "A") all_caps sayılmamalı (< 2 harf)
        let mut s = stop("S1", 41.0, 29.0);
        s.stop_name = Some("A".into());
        let records = records_with(
            vec![s],
            vec![route("R1", 3)],
            vec![trip("T1", "R1")],
            vec![stoptime("T1", 1, "S1", (8,0,0), (8,0,0), 2)],
        );
        let result = analyze(&records, &empty_derived(), &default_config(), 20260514);
        assert!(!result.notices.iter().any(|n| n.rule_id == "DQ_018"), "tek harf DQ_018 üretmemeli");
    }

    // ── Fix-8: hız anomalisi shape projeksiyonu ──────────────────────────────

    #[test]
    fn winding_shape_produces_stm_014_where_haversine_would_not() {
        // Kuş uçuşu A→B ≈ 0.84 km / 5 dk → ~10 km/h (eşik altı; haversine tek başına kaçırır).
        // Küçük U shape (0.1° detour): arc ≈ 23 km / 5 dk → ~277 km/h → STM_014 tetiklenir.
        //
        // Projeksiyon analizi:
        //   A(41.0,29.0) → Segment 0 başlangıcında dsq=0   → arc = 0
        //   B(41.0,29.01)→ Segment 2 sonunda dsq=0 (Seg 0'dan dsq≈0.7) → arc ≈ 23 km
        //   |arc_B − arc_A| ≈ 23 km vs. haversine ~0.84 km.
        //   Hız: 23/300×3600 ≈ 277 km/h → 120 < 277 < 700 → STM_014 (STM_012 değil).
        use crate::k2::shapes::ShapePointRecord;
        let mut records = records_with(
            vec![stop("A", 41.0, 29.0), stop("B", 41.0, 29.01)],
            vec![route("R1", 3)],
            vec![{
                let mut t = trip("T1", "R1");
                t.shape_id = Some("S1".into());
                t
            }],
            vec![
                stoptime("T1", 1, "A", (8, 0, 0), (8, 0, 0), 2),
                stoptime("T1", 2, "B", (8, 5, 0), (8, 5, 0), 3), // 5 dk
            ],
        );
        // U shape: (41.0,29.0) → kuzeye (41.1,29.0) → (41.1,29.01) → güneye (41.0,29.01)
        // Arc ≈ 11.1 + 0.84 + 11.1 = 23 km; B yalnızca son segmente yakın (dsq=0)
        records.shapes = vec![
            ShapePointRecord { shape_id: "S1".into(), shape_pt_lat: Some(41.0), shape_pt_lon: Some(29.00), shape_pt_sequence: Some(1), shape_dist_traveled: None, line: 2 },
            ShapePointRecord { shape_id: "S1".into(), shape_pt_lat: Some(41.1), shape_pt_lon: Some(29.00), shape_pt_sequence: Some(2), shape_dist_traveled: None, line: 3 },
            ShapePointRecord { shape_id: "S1".into(), shape_pt_lat: Some(41.1), shape_pt_lon: Some(29.01), shape_pt_sequence: Some(3), shape_dist_traveled: None, line: 4 },
            ShapePointRecord { shape_id: "S1".into(), shape_pt_lat: Some(41.0), shape_pt_lon: Some(29.01), shape_pt_sequence: Some(4), shape_dist_traveled: None, line: 5 },
        ];
        let result = analyze(&records, &empty_derived(), &default_config(), 20260514);
        assert!(
            result.notices.iter().any(|n| n.rule_id == "STM_014"),
            "U biçimli shape projeksiyonu ~23 km / 5 dk → ~277 km/h → STM_014 beklenir; notices: {:?}",
            result.notices.iter().map(|n| (&n.rule_id, n.observed_value.as_deref())).collect::<Vec<_>>()
        );
    }

    #[test]
    fn trip_with_shape_id_but_no_shape_data_falls_back_to_haversine() {
        // shape_id atanmış fakat records.shapes'te eşleşen kayıt yok.
        // Kod `shape_pts_speed.get(sid)` → None → Haversine fallback devreye girer.
        // A→B: ~0.82 km / 10 dk → ~5 km/h → STM_014 tetiklenmemeli.
        let records = records_with(
            vec![stop("A", 41.0, 29.0), stop("B", 41.0, 29.01)],
            vec![route("R1", 3)],
            vec![{
                let mut t = trip("T1", "R1");
                t.shape_id = Some("MISSING_SHAPE".into());
                t
            }],
            vec![
                stoptime("T1", 1, "A", (8, 0, 0), (8, 0, 0), 2),
                stoptime("T1", 2, "B", (8, 10, 0), (8, 10, 0), 3),
            ],
        );
        // records.shapes boş → "MISSING_SHAPE" bulunamaz → Haversine ile normal hız
        let result = analyze(&records, &empty_derived(), &default_config(), 20260514);
        assert!(
            !result.notices.iter().any(|n| n.rule_id == "STM_014"),
            "shape verisi eksik → Haversine fallback; normal hız (~5 km/h) → STM_014 olmamalı"
        );
    }

    // ── VAT: Varlık Analitik Tespiti ────────────────────────────────────────

    #[test]
    fn similar_routes_produce_vat_001() {
        // R1 ve R2 aynı 6 durağa sahip → Jaccard=1.0 ≥ 0.85 → VAT_001
        let stops: Vec<StopRecord> = (1..=6)
            .map(|i| stop(&format!("S{i}"), 41.0 + i as f64 * 0.01, 29.0))
            .collect();
        let mut r = crate::k2::EntityRecords::default();
        r.stops = stops;
        r.routes = vec![route("R1", 3), route("R2", 3)];
        let mut t1 = trip("T1", "R1"); t1.service_id = "SVC".into();
        let mut t2 = trip("T2", "R2"); t2.service_id = "SVC".into();
        r.trips = vec![t1, t2];
        r.stop_times = (1..=6).flat_map(|i| {
            let h = i as u32;
            vec![
                stoptime("T1", i, &format!("S{i}"), (h, 0, 0), (h, 0, 0), i as u64 + 1),
                stoptime("T2", i, &format!("S{i}"), (h, 5, 0), (h, 5, 0), i as u64 + 10),
            ]
        }).collect();
        let result = analyze(&r, &empty_derived(), &default_config(), 20260514);
        assert!(result.notices.iter().any(|n| n.rule_id == "VAT_001"), "VAT_001 olmalı");
    }

    #[test]
    fn very_different_routes_no_vat_001() {
        // R1: S1-S3, R2: S4-S6 → hiç ortak durak yok → Jaccard=0 → VAT_001 olmamalı
        let mut r = crate::k2::EntityRecords::default();
        r.stops = (1..=6).map(|i| stop(&format!("S{i}"), 41.0 + i as f64 * 0.01, 29.0)).collect();
        r.routes = vec![route("R1", 3), route("R2", 3)];
        r.trips = vec![trip("T1", "R1"), trip("T2", "R2")];
        r.stop_times = vec![
            stoptime("T1", 1, "S1", (8,0,0), (8,0,0), 2),
            stoptime("T1", 2, "S2", (8,5,0), (8,5,0), 3),
            stoptime("T1", 3, "S3", (8,10,0), (8,10,0), 4),
            stoptime("T2", 1, "S4", (9,0,0), (9,0,0), 2),
            stoptime("T2", 2, "S5", (9,5,0), (9,5,0), 3),
            stoptime("T2", 3, "S6", (9,10,0), (9,10,0), 4),
        ];
        let result = analyze(&r, &empty_derived(), &default_config(), 20260514);
        assert!(!result.notices.iter().any(|n| n.rule_id == "VAT_001"), "VAT_001 olmamalı");
    }

    #[test]
    fn busy_stop_no_transfer_produces_vat_002() {
        // S1 durağı 4 farklı hat tarafından ziyaret ediliyor, transfer yok → VAT_002
        let mut r = crate::k2::EntityRecords::default();
        r.stops = vec![stop("S1", 41.0, 29.0), stop("S2", 41.1, 29.1)];
        r.routes = (1..=4).map(|i| route(&format!("R{i}"), 3)).collect();
        r.trips = (1..=4).map(|i| {
            let mut t = trip(&format!("T{i}"), &format!("R{i}"));
            t.service_id = "SVC".into();
            t
        }).collect();
        r.stop_times = (1..=4).flat_map(|i| vec![
            stoptime(&format!("T{i}"), 1, "S1", (8,0,0), (8,0,0), 2),
            stoptime(&format!("T{i}"), 2, "S2", (8,10,0), (8,10,0), 3),
        ]).collect();
        let result = analyze(&r, &empty_derived(), &default_config(), 20260514);
        assert!(result.notices.iter().any(|n| n.rule_id == "VAT_002"), "VAT_002 olmalı");
    }

    #[test]
    fn outlier_trip_duration_produces_vat_003() {
        // R1 hattında 5 sefer: 4 tanesi ~30dk, 1 tanesi ~3saat (aykırı değer) → VAT_003
        let mut r = crate::k2::EntityRecords::default();
        r.stops = vec![stop("A", 41.0, 29.0), stop("B", 41.1, 29.1)];
        r.routes = vec![route("R1", 3)];
        // Normal: T1-T4 = 30dk, aykırı: T5 = 180dk
        let mut trips_vec = Vec::new();
        let mut stoptimes_vec = Vec::new();
        for i in 1..=4u32 {
            let tid = format!("T{i}");
            trips_vec.push(trip(&tid, "R1"));
            stoptimes_vec.push(stoptime(&tid, 1, "A", (i, 0, 0), (i, 0, 0), 2));
            stoptimes_vec.push(stoptime(&tid, 2, "B", (i, 30, 0), (i, 30, 0), 3));
        }
        // Aykırı sefer: 180 dakika (3 saat)
        trips_vec.push(trip("T5", "R1"));
        stoptimes_vec.push(stoptime("T5", 1, "A", (6, 0, 0), (6, 0, 0), 2));
        stoptimes_vec.push(stoptime("T5", 2, "B", (9, 0, 0), (9, 0, 0), 3));
        r.trips = trips_vec;
        r.stop_times = stoptimes_vec;
        let result = analyze(&r, &empty_derived(), &default_config(), 20260514);
        assert!(result.notices.iter().any(|n| n.rule_id == "VAT_003"), "VAT_003 olmalı");
    }

    #[test]
    fn weekday_only_route_produces_vat_004() {
        use crate::k2::calendar::CalendarRecord;
        let mut r = crate::k2::EntityRecords::default();
        r.stops = vec![stop("A", 41.0, 29.0), stop("B", 41.1, 29.1)];
        r.routes = vec![route("R1", 3)];
        r.calendars = vec![CalendarRecord {
            service_id: "WEEKDAY".into(),
            days: [Some(1), Some(1), Some(1), Some(1), Some(1), Some(0), Some(0)], // Mon-Fri
            start_date: None,
            end_date: None,
            row: Default::default(),
            line: 2,
        }];
        r.trips = (1..=5u32).map(|i| {
            let mut t = trip(&format!("T{i}"), "R1");
            t.service_id = "WEEKDAY".into();
            t
        }).collect();
        r.stop_times = (1..=5u32).flat_map(|i| vec![
            stoptime(&format!("T{i}"), 1, "A", (i+7, 0, 0), (i+7, 0, 0), 2),
            stoptime(&format!("T{i}"), 2, "B", (i+7, 30, 0), (i+7, 30, 0), 3),
        ]).collect();
        let result = analyze(&r, &empty_derived(), &default_config(), 20260514);
        assert!(result.notices.iter().any(|n| n.rule_id == "VAT_004"), "VAT_004 olmalı");
    }

    #[test]
    fn isolated_stop_cluster_produces_vat_005() {
        // Ana ağ: A-B-C-D (10+ bağlantı), izole küme: X-Y
        let mut r = crate::k2::EntityRecords::default();
        r.stops = vec![
            stop("A",41.0,29.0), stop("B",41.1,29.0), stop("C",41.2,29.0), stop("D",41.3,29.0),
            stop("E",41.4,29.0), stop("F",41.5,29.0),
            stop("X",42.0,30.0), stop("Y",42.1,30.0), // izole
        ];
        r.routes = vec![route("R1",3), route("R2",3)];
        // Ana ağ: birden fazla sefer A-B-C-D-E-F
        let mut trips_v = Vec::new();
        let mut st_v = Vec::new();
        for i in 1..=12u32 {
            let tid = format!("M{i}");
            trips_v.push(trip(&tid, "R1"));
            st_v.push(stoptime(&tid,1,"A",(i,0,0),(i,0,0),2));
            st_v.push(stoptime(&tid,2,"B",(i,5,0),(i,5,0),3));
            st_v.push(stoptime(&tid,3,"C",(i,10,0),(i,10,0),4));
            st_v.push(stoptime(&tid,4,"D",(i,15,0),(i,15,0),5));
            st_v.push(stoptime(&tid,5,"E",(i,20,0),(i,20,0),6));
            st_v.push(stoptime(&tid,6,"F",(i,25,0),(i,25,0),7));
        }
        // İzole küme: X-Y
        trips_v.push(trip("ISO","R2"));
        st_v.push(stoptime("ISO",1,"X",(8,0,0),(8,0,0),2));
        st_v.push(stoptime("ISO",2,"Y",(8,10,0),(8,10,0),3));
        r.trips = trips_v;
        r.stop_times = st_v;
        let result = analyze(&r, &empty_derived(), &default_config(), 20260514);
        assert!(result.notices.iter().any(|n| n.rule_id == "VAT_005"), "VAT_005 olmalı");
    }

    #[test]
    fn dominant_route_produces_vat_006() {
        // R1: 80 sefer, R2: 10 sefer, R3: 10 sefer → R1 %80 → VAT_006
        let mut r = crate::k2::EntityRecords::default();
        r.stops = vec![stop("A", 41.0, 29.0), stop("B", 41.1, 29.1)];
        r.routes = vec![route("R1", 3), route("R2", 3), route("R3", 3)];
        let mut trips_v = Vec::new();
        let mut st_v = Vec::new();
        for i in 1..=80u32 {
            let tid = format!("T1_{i}");
            trips_v.push(trip(&tid, "R1"));
            st_v.push(stoptime(&tid, 1, "A", (i % 24, 0, 0), (i % 24, 0, 0), 2));
            st_v.push(stoptime(&tid, 2, "B", (i % 24, 10, 0), (i % 24, 10, 0), 3));
        }
        for i in 1..=10u32 {
            let tid2 = format!("T2_{i}");
            let tid3 = format!("T3_{i}");
            trips_v.push(trip(&tid2, "R2"));
            trips_v.push(trip(&tid3, "R3"));
            st_v.push(stoptime(&tid2, 1, "A", (i, 30, 0), (i, 30, 0), 2));
            st_v.push(stoptime(&tid2, 2, "B", (i, 40, 0), (i, 40, 0), 3));
            st_v.push(stoptime(&tid3, 1, "A", (i, 45, 0), (i, 45, 0), 2));
            st_v.push(stoptime(&tid3, 2, "B", (i, 55, 0), (i, 55, 0), 3));
        }
        r.trips = trips_v;
        r.stop_times = st_v;
        let result = analyze(&r, &empty_derived(), &default_config(), 20260514);
        assert!(result.notices.iter().any(|n| n.rule_id == "VAT_006"), "VAT_006 olmalı");
    }

    #[test]
    fn shared_terminus_no_transfer_produces_vat_007() {
        // 3 farklı hat HUB durağında başlıyor ama transfer yok → VAT_007
        let mut r = crate::k2::EntityRecords::default();
        r.stops = vec![
            stop("HUB", 41.0, 29.0),
            stop("A", 41.1, 29.0), stop("B", 41.0, 29.1), stop("C", 41.0, 28.9),
        ];
        r.routes = (1..=3).map(|i| route(&format!("R{i}"), 3)).collect();
        r.trips = (1..=3).map(|i| trip(&format!("T{i}"), &format!("R{i}"))).collect();
        r.stop_times = vec![
            stoptime("T1", 1, "HUB", (8,0,0), (8,0,0), 2),
            stoptime("T1", 2, "A",   (8,10,0), (8,10,0), 3),
            stoptime("T2", 1, "HUB", (8,0,0), (8,0,0), 2),
            stoptime("T2", 2, "B",   (8,10,0), (8,10,0), 3),
            stoptime("T3", 1, "HUB", (8,0,0), (8,0,0), 2),
            stoptime("T3", 2, "C",   (8,10,0), (8,10,0), 3),
        ];
        // transfers boş
        let result = analyze(&r, &empty_derived(), &default_config(), 20260514);
        assert!(result.notices.iter().any(|n| n.rule_id == "VAT_007"), "VAT_007 olmalı");
    }

    // ── EntityMap eksikliği ───────────────────────────────────────────────────

    fn _uses_entity_map(_: &EntityMap) {}
}
