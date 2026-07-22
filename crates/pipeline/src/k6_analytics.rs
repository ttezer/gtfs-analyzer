use std::collections::{BTreeSet, HashMap, HashSet};
use rustc_hash::{FxHashMap, FxHashSet};
use smol_str::SmolStr;

use gtfs_config::ValidatorConfig;
use gtfs_core::{EntityType, Notice};

use crate::k2::stop_times::CompactStopTime;
use crate::k2::stop_times::StopTimesIndex as K2StopTimesIndex;
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

    let ti_analyze = &records.trip_interns;
    let trip_shape: HashMap<&str, &str> = records
        .trips
        .iter()
        .filter_map(|t| ti_analyze.shape_id(t).map(|s| (t.trip_id.as_str(), s)))
        .collect();
    // OOM fix Plan D (güncellendi): K6 K2 index'ini KLONLAMAZ. Test yolunda (stop_times_index boş
    // ama records.stop_times dolu) owned K2 index kurulur; üretimde records.stop_times_index ödünç.
    let fallback_k2_owned: Option<K2StopTimesIndex> =
        if records.stop_times_index.total_rows == 0 && !records.stop_times.is_empty() {
            Some(K2StopTimesIndex::from_records(&records.stop_times))
        } else {
            None
        };
    let k2_for_idx: &K2StopTimesIndex = fallback_k2_owned.as_ref().unwrap_or(&records.stop_times_index);
    let idx = { let _t = Timer::start("K6::idx::build"); StopTimesIndex::build(records, &trip_shape, k2_for_idx) };

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
        Box::new(|| { let _t = Timer::start("K6::data_quality");          let mut v = Vec::new(); let mut c = 0u32; check_data_quality(records, derived, config, today_yyyymmdd, &mut v, &mut c); v }),
        Box::new(|| { let _t = Timer::start("K6::remaining_analytics");   let mut v = Vec::new(); let mut c = 0u32; check_remaining_analytics(records, derived, config, &idx, &mut v, &mut c); v }),
        Box::new(|| { let _t = Timer::start("K6::shp012");                let mut v = Vec::new(); let mut c = 0u32; check_shp012(records, config, &idx, &mut v, &mut c); v }),
        Box::new(|| { let _t = Timer::start("K6::shp022");                let mut v = Vec::new(); let mut c = 0u32; check_shp022(records, &idx, &mut v, &mut c); v }),
        Box::new(|| { let _t = Timer::start("K6::pathway_analytics");     let mut v = Vec::new(); let mut c = 0u32; check_pathway_analytics(records, derived, &mut v, &mut c); v }),
        Box::new(|| { let _t = Timer::start("K6::calendar_override");     let mut v = Vec::new(); let mut c = 0u32; check_calendar_override_analytics(records, derived, config, &mut v, &mut c); v }),
        Box::new(|| { let _t = Timer::start("K6::vat_analytics");         let mut v = Vec::new(); let mut c = 0u32; check_vat_analytics(records, derived, config, &idx, &mut v, &mut c); v }),
    ];

    #[cfg(feature = "parallel")]
    let parts: Vec<Vec<Notice>> = {
        // Büyük feed'de (>15M stop_times) 15 eşzamanlı görev 4GB WASM sınırını aşıyor.
        // 3'lü batch: peak ~600MB/batch, 1850MB headroom içinde. Küçük feedlerde değişmez.
        let large = records.stop_times_index.total_rows > 15_000_000;
        let batch = if large { 3 } else { tasks.len() };
        let mut out: Vec<Vec<Notice>> = tasks.iter().map(|_| Vec::new()).collect();
        for (i, (out_chunk, task_chunk)) in out.chunks_mut(batch).zip(tasks.chunks(batch)).enumerate() {
            if large {
                crate::timing::mem_log(&format!("K6 batch {i} start"));
            }
            rayon::scope(|s| {
                for (slot, task) in out_chunk.iter_mut().zip(task_chunk.iter()) {
                    s.spawn(move |_| { *slot = task(); });
                }
            });
            if large {
                crate::timing::mem_log(&format!("K6 batch {i} end"));
            }
        }
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
    crate::notice_factory::build(
        "K6", Some("k6"), ctr, rule_id, entity_type, entity_id, scope_key,
        Some(file.to_string()), line, field.map(str::to_string),
        observed, expected, message, remediation,
    )
}

// ── Yardımcı fonksiyonlar ─────────────────────────────────────────────────────

/// stop_id'nin sayısal kök kısmını döndürür: sondaki harf ekini ayıklar.
/// Rakam içermiyorsa orijinal id döner (kıyas güvenli değil).
/// Örnek: "2119d" → "2119", "t17d" → "t17", "abc" → "abc"
#[allow(dead_code)]
/// `needle`, `haystack` içinde kelime sınırında (alfanümerik olmayan karakter veya
/// dize başı/sonu ile çevrili) geçiyor mu? Büyük/küçük harf duyarsız.
fn contains_as_word(haystack: &str, needle: &str) -> bool {
    if needle.is_empty() { return false; }
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
        // pos+1 çok-baytlı UTF-8'de (CJK vb.) char sınırında olmayabilir → bir sonraki
        // char sınırına ilerle; aksi halde h[start..] dilimlemesi panik atar.
        start = pos + h[pos..].chars().next().map_or(1, |c| c.len_utf8());
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

// haversine_km / ymd_to_jdn / jdn_to_yyyymmdd → k5_derived (tek kanonik tanım).
// Buradaki kopyaları k5'inkilerle birebir aynıydı; k5 sürümü test kapsamında.
use crate::k5_derived::{haversine_km, jdn_to_yyyymmdd, ymd_to_jdn};

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

/// route_type → config'ten hız eşiği (km/h).
/// #15/MD-kalibrasyon: STANDART (0-7,11,12) VE GENİŞLETİLMİŞ Avrupa route_type'larını (100-1700)
/// doğru kategoriye eşler. Eski sürüm yalnız standartları tanıyordu → VBB Berlin gibi tamamı
/// extended-kod kullanan feed'lerde rail (100-117 = S-Bahn/bölgesel/intercity) `_ => bus`'a
/// düşüp 120 km/h eşik alıyordu → meşru 120-300 km/h trenler STM_014 FP (4530 vs MD 233).
/// is_rail_route_type ile artık tutarlı.
fn max_speed_kmh(route_type: u32, cfg: &ValidatorConfig) -> f64 {
    match route_type {
        0 | 900..=906                                   => cfg.max_speed_tram_kmh,   // tram / hafif raylı
        1 | 400..=405                                   => cfg.max_speed_metro_kmh,  // metro / U-Bahn
        2 | 12 | 100..=117                              => cfg.max_speed_rail_kmh,   // rail / S-Bahn / bölgesel
        3 | 11 | 200..=209 | 700..=716 | 800 | 1500..=1507 => cfg.max_speed_bus_kmh, // bus / coach / troleybüs / taksi
        4 | 1000..=1021 | 1200                          => cfg.max_speed_ferry_kmh,  // su / feribot
        5 | 6 | 7 | 1300..=1307 | 1400                  => cfg.max_speed_cablecar_kmh, // teleferik / füniküler
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
    // K2 intern tablosuna referans: stop_id, headsign, flex erişimi için
    k2: &'a K2StopTimesIndex,
}

impl<'a> StopTimesIndex<'a> {
    fn build(
        records: &'a EntityRecords,
        trip_shape: &HashMap<&'a str, &'a str>,
        k2: &'a K2StopTimesIndex,
    ) -> Self {
        let n_trips = records.trips.len();
        let n_stops = records.stops.len();

        // K2 index dilimlerini ÖDÜNÇ al (klon yok); test yolunda fallback K2 ref'i gelir.
        let mut by_trip: FxHashMap<&'a str, &'a [CompactStopTime]> = FxHashMap::default();
        let mut stop_shapes: FxHashMap<&'a str, Vec<&'a str>> = FxHashMap::default();
        by_trip.reserve(n_trips);
        stop_shapes.reserve(n_stops);

        for (trip_id, stops) in k2.iter_trips() {
            let tid: &'a str = trip_id.as_str();
            by_trip.insert(tid, stops);
            if let Some(&shape_id) = trip_shape.get(tid) {
                for st in stops {
                    let sid = k2.stop_id_of(st);
                    if !sid.is_empty() {
                        stop_shapes.entry(sid).or_default().push(shape_id);
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
            if v.iter().any(|s| s.arrival_time().is_some() || s.departure_time().is_some()) {
                trip_has_time.insert(tid);
            }
            if trip_shape.contains_key(tid) {
                if let Some(st) = v.iter().find(|s| s.shape_dist_traveled().is_none()) {
                    trips_missing_sdt.insert(tid, st.line as u64);
                }
            }
        }

        let trip_first_dep: FxHashMap<&'a str, u32> = by_trip
            .iter()
            .filter_map(|(&tid, v)| {
                v.first()
                    .and_then(|s| s.departure_time())
                    .map(|dep| (tid, hms_to_secs(dep)))
            })
            .collect();

        Self {
            by_trip,
            trip_first_dep,
            trip_has_time,
            stop_shapes,
            trips_missing_sdt,
            unsorted_seq_trips: &k2.unsorted_seq_trips,
            k2,
        }
    }

    // Stop_id yardımcısı — K2 intern tablosuna delege eder
    #[inline] fn stop_id_of(&self, st: &CompactStopTime) -> &str { self.k2.stop_id_of(st) }
}


// ── WP-09a: Hız anomalisi + trip süresi ──────────────────────────────────────

/// STM_014 pattern toplulaması: (hat, yön, segment) başına biriken hız aşımları.
///
/// Aynı fiziksel segment o segmentten geçen HER seferde ayrı ayrı tetikleniyordu;
/// TriMet'te 604 notice'ın tamamı tek segmentten (Route 20, durak 241→255) geliyordu
/// ve hepsi aynı kök nedeni (segmentin schedule/shape tutarsızlığı) gösteriyordu.
/// Tespit mantığı DEĞİŞMEDİ — yalnız emit pattern düzeyinde toplanır.
/// (Güvenilirlik Sprint'i Checkpoint 6 kararı: "doğruluk sorunu değil, sunum ve
/// dedup ergonomisi sorunu; pattern-level gruplama olarak ele alınabilir".)
struct Stm014Seg<'a> {
    route_label: &'a str,
    seq_a: u32,
    seq_b: u32,
    threshold: f64,
    /// En küçük stop_times satırı — deterministik çıktı için (map sırası rastgele).
    line: u64,
    speed_min: f64,
    speed_max: f64,
    trips: Vec<&'a str>,
}

/// details["trips"] listesinde gösterilecek örnek sefer sayısı (tam sayı trip_count'ta).
const STM014_TRIP_SAMPLE: usize = 12;

fn check_speed_and_duration(
    records: &EntityRecords,
    config: &ValidatorConfig,
    idx: &StopTimesIndex<'_>,
    notices: &mut Vec<Notice>,
    ctr: &mut u32,
) {
    use crate::timing::Timer;
    let ti_sd = &records.trip_interns;

    let (stop_coords, stop_name_sd, trip_route_type, trip_to_route, route_short_sd, trip_direction, trip_headsign_sd, trip_service) = {
        let _t = Timer::start("K6::sd::setup");
        // stop_id → (lat, lon)  FxHashMap: SipHash yerine multiply-xor
        let mut stop_coords: FxHashMap<&str, (f64, f64)> = FxHashMap::default();
        stop_coords.reserve(records.stops.len());
        for s in &records.stops {
            if let Some(coords) = s.stop_lat.zip(s.stop_lon) {
                stop_coords.insert(s.stop_id.as_str(), coords);
            }
        }

        // stop_id → stop_name (gösterim için; boş/yoksa eklenmez, fallback stop_id)
        let mut stop_name_sd: FxHashMap<&str, &str> = FxHashMap::default();
        stop_name_sd.reserve(records.stops.len());
        for s in &records.stops {
            if let Some(name) = s.stop_name.as_deref().filter(|n| !n.is_empty()) {
                stop_name_sd.insert(s.stop_id.as_str(), name);
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
            if let Some(&rt) = route_type_map.get(ti_sd.route_id(t)) {
                trip_route_type.insert(t.trip_id.as_str(), rt);
            }
        }

        // trip_id → route_id
        let mut trip_to_route: FxHashMap<&str, &str> = FxHashMap::default();
        trip_to_route.reserve(records.trips.len());
        for t in &records.trips {
            trip_to_route.insert(t.trip_id.as_str(), ti_sd.route_id(t));
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
            trip_service.insert(t.trip_id.as_str(), ti_sd.service_id(t));
        }

        // trip_id → trip_headsign (gösterim için; boş/yoksa eklenmez)
        let mut trip_headsign_sd: FxHashMap<&str, &str> = FxHashMap::default();
        trip_headsign_sd.reserve(records.trips.len());
        for t in &records.trips {
            if let Some(hs) = ti_sd.headsign(t).filter(|h| !h.is_empty()) {
                trip_headsign_sd.insert(t.trip_id.as_str(), hs);
            }
        }

        (stop_coords, stop_name_sd, trip_route_type, trip_to_route, route_short_sd, trip_direction, trip_headsign_sd, trip_service)
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
            .filter_map(|t| ti_sd.shape_id(t)
                .filter(|s| !s.is_empty())
                .map(|s| (t.trip_id.as_str(), s)))
            .collect();
        (shape_pts, shape_cum, trip_shape)
    };

    // STM_036: stop_times trip_id + stop_sequence'a göre sıralı/gruplu değil (MD unsorted_stop_times,
    // INFO — benign: pipeline zaten sequence'e göre yeniden sıralar). İki alt-vaka:
    //   (a) sequence AZALIYOR (K2'de tespit, unsorted_seq_trips)
    //   (b) satırlar DAĞINIK (araya başka trip girmiş; sequence artsa bile gruplu değil)
    let mut stm036_seen: std::collections::HashSet<&str> = std::collections::HashSet::new();
    for (trip_id, prev_seq, curr_seq, line_no) in idx.unsorted_seq_trips.iter() {
        stm036_seen.insert(trip_id.as_str());
        notices.push(k6_notice(
            ctr, "STM_036", EntityType::Trip,
            Some(trip_id.to_string()), Some(trip_id.to_string()),
            "stop_times.txt", Some(*line_no), Some("stop_sequence"),
            Some(format!("stop_sequence {prev_seq}→{curr_seq} azalıyor")), Some("artan + gruplu".to_string()),
            format!("'{trip_id}' seferinde stop_times satır sırası bozuk: stop_sequence {prev_seq}'den {curr_seq}'e düşüyor."),
            "stop_times.txt'i trip_id ve stop_sequence'a göre sıralayın.",
        ));
    }
    // (b) non-contiguous: finalize edilmiş index'te trip satırlarının CSV line'ları ardışık mı
    // (max-min+1 == satır sayısı) — değilse araya başka trip girmiş. Deterministik olsun diye ilk-satıra göre sıralanır.
    let mut noncontig: Vec<(&str, u32, u32)> = Vec::new();
    for (trip_id, stops) in idx.by_trip.iter() {
        if stm036_seen.contains(*trip_id) { continue; }
        if stops.len() < 2 { continue; }
        let (mut lo, mut hi) = (u32::MAX, 0u32);
        for r in *stops { lo = lo.min(r.line); hi = hi.max(r.line); }
        if (hi - lo + 1) as usize != stops.len() {
            noncontig.push((*trip_id, lo, hi));
        }
    }
    noncontig.sort_by_key(|&(_, lo, _)| lo);
    for (trip_id, lo, hi) in noncontig {
        notices.push(k6_notice(
            ctr, "STM_036", EntityType::Trip,
            Some(trip_id.to_string()), Some(trip_id.to_string()),
            "stop_times.txt", Some(lo as u64), Some("stop_sequence"),
            Some("dağınık satırlar".to_string()), Some("trip'e göre gruplu".to_string()),
            format!("'{trip_id}' seferinin stop_times satırları dosyada dağınık (trip_id'ye göre gruplu değil); {lo}–{hi} arasına yayılmış."),
            "stop_times.txt'i trip_id ve stop_sequence'a göre sıralayın.",
        ));
    }

    // Reusable per-trip coord buffer: 1 lookup/stop (windows(2) ile 2 lookup/pair olurdu)
    let mut coords_buf: Vec<Option<(f64, f64)>> = Vec::new();

    // Perf: (shape_id, stop_id) → shape üzerine arc-uzunluğu projeksiyonu MEMOIZE edilir.
    // project_arc_km tüm shape noktalarını gezer; aynı (shape,durak) seferler/segmentler
    // arası tekrar projekte ediliyordu. dist_km çıktısı BİREBİR aynı — sadece tekrar eleniyor.
    let mut arc_cache: FxHashMap<(&str, &str), f64> = FxHashMap::default();
    // STM_038: aynı satırda 00:xx kalkış (gece-yarısı) yüzünden STM_007'den elenen durak sayısı.
    let mut stm007_midnight_count = 0u32;

    // STM_014 birikimi: (route_id, direction, stop_a, stop_b) → segment özeti.
    // Döngü içinde emit YOK; döngü bitince sıralı olarak tek tek üretilir.
    let mut stm014_segs: FxHashMap<(&str, &str, &str, &str), Stm014Seg<'_>> = FxHashMap::default();

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
            .and_then(|s| s.departure_time())
            .map(|(h, m, _)| format!("{h:02}:{m:02}"))
            .unwrap_or_default();
        // trip_id zenginleştirme: insan-tanır kalkış saati eki (" 08:15 kalkışlı"); bilinmiyorsa boş.
        // (STM_026 mesajındaki mevcut desenle aynı biçim.)
        let dep_suffix = if dep_str.is_empty() { String::new() } else { format!(" {dep_str} kalkışlı") };

        // ── STM_007: aynı durakta departure < arrival (K2'den taşındı) ───────────
        // Gece yarısını aşan son durak (00:xx kalkış, 24:xx yazılmalıydı) yanlış-pozitifi
        // service_day_start ile elenir; gerçek satır-içi hatalar zenginleştirilmiş mesajla verilir.
        {
            let sds_secs = config.service_day_start_hour * 3600;
            for st in stimes.iter() {
                let (Some(arr), Some(dep)) = (st.arrival_time(), st.departure_time()) else { continue };
                let arr_s = hms_to_secs(arr);
                let dep_s = hms_to_secs(dep);
                if dep_s >= arr_s { continue; }
                // departure'ı +24sa alınca arrival'a yakınsa → gece-yarısı geçişi (kasıtsız) → atla
                if sds_secs > 0 && (dep_s + 86400).saturating_sub(arr_s) <= sds_secs {
                    stm007_midnight_count += 1;
                    continue;
                }
                let seq = st.stop_sequence().unwrap_or(0);
                let name = stop_name_sd.get(idx.stop_id_of(st)).copied().unwrap_or(idx.stop_id_of(st));
                let mut n = k6_notice(
                    ctr, "STM_007", EntityType::Trip,
                    Some(trip_id.to_string()), Some(trip_id.to_string()),
                    "stop_times.txt", Some(st.line as u64), Some("departure_time"),
                    Some(format!("{} < {}", format_hms(dep_s), format_hms(arr_s))),
                    Some(format!(">= {}", format_hms(arr_s))),
                    format!("'{route_label}' hattı '{trip_id}'{dep_suffix} seferi: {name} durağında (sıra {seq}) kalkış {} < varış {} — kalkış varıştan önce. (yön {dir_sd})",
                        format_hms(dep_s), format_hms(arr_s)),
                    "stop_times.txt'te kalkış zamanını varış zamanından sonra (veya eşit) ayarlayın.",
                );
                let mut d = std::collections::HashMap::new();
                d.insert("route".to_string(), route_label.to_string());
                d.insert("stop_name".to_string(), name.to_string());
                d.insert("seq".to_string(), seq.to_string());
                d.insert("dir".to_string(), dir_sd.to_string());
                d.insert("dep".to_string(), format_hms(dep_s));
                d.insert("arr".to_string(), format_hms(arr_s));
                n.details = Some(d);
                n.service_id = Some(svc_sd.to_string());
                notices.push(n);
            }
        }

        // ── STM_028 / STM_029: trip süresi ───────────────────────────────────
        let first_dep = stimes.first().and_then(|s| s.departure_time()).map(hms_to_secs);
        let last_arr = stimes.last().and_then(|s| s.arrival_time()).map(hms_to_secs);

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
                        stimes.last().map(|s| s.line as u64),
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
                        stimes.last().map(|s| s.line as u64),
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

        let mut same_run = 1usize;
        let mut max_same_run = 1usize;
        let mut previous_time: Option<u32> = None;
        for st in stimes.iter() {
            let event_time = st.arrival_time().or_else(|| st.departure_time()).map(hms_to_secs);
            if event_time.is_some() && event_time == previous_time {
                same_run += 1; max_same_run = max_same_run.max(same_run);
            } else { same_run = 1; }
            previous_time = event_time;
        }
        if max_same_run >= 3 {
            notices.push(k6_notice(ctr, "STM_053", EntityType::Trip,
                Some(trip_id.to_string()), Some(trip_id.to_string()), "stop_times.txt",
                stimes.first().map(|s| s.line as u64), Some("arrival_time"),
                Some(format!("{max_same_run} ardışık durak")), Some("< 3 ardışık durak".to_string()),
                format!("trip_id '{trip_id}' içinde {max_same_run} ardışık durak aynı zaman değerini kullanıyor."),
                "Ardışık stop_times zamanlarını doğrulayın; gerçek bekleme ise zamanları açıklayıcı biçimde düzenleyin."));
        }

        // ── STM_014 / OPR_008: hız anomalisi ─────────────────────────────────
        // Per-trip coord buffer: her stop bir kez sorgulanır
        coords_buf.clear();
        coords_buf.extend(stimes.iter().map(|s| stop_coords.get(idx.stop_id_of(s)).copied()));

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

            let dep_a = a.departure_time().map(hms_to_secs);
            let arr_b = b.arrival_time().map(hms_to_secs);

            let (Some(dep), Some(arr)) = (dep_a, arr_b) else { continue };
            // STM_020: sıfır geçiş süresi — eşik 200m (dakika yuvarlama gürültüsünü filtreler)
            if arr == dep {
                let dep_secs = a.departure_time().map(|(_, _, s)| s).unwrap_or(1);
                let arr_secs = b.arrival_time().map(|(_, _, s)| s).unwrap_or(1);
                let dist_km = match (coords_buf[i], coords_buf[i + 1]) {
                    (Some((la1, lo1)), Some((la2, lo2))) => haversine_km(la1, lo1, la2, lo2),
                    _ => 0.0,
                };
                // Her iki zaman tam dakika (saniye=0) → gerçek geçiş süresi bilinmiyor:
                // yuvarlama yüzünden 0-59 sn arası herhangi bir şey olabilir. Bu yüzden
                // "imkânsız" diyebilmek için mesafe, TAM BİR DAKİKA geçmiş olsa BİLE
                // rota tipinin azami hızını aşmalı → eşik = max_speed_kmh / 60.
                //
                // Sabit 1.0 km eşiği YANLIŞ POZİTİF üretiyordu: 1.1 km bir otobüs için
                // 60 sn'de 66 km/h eder — gayet olağan, "fiziksel olarak imkânsız" değil.
                // 250-feed corpus kanıtı (mdb-2155): 42 bulgunun 41'i 1.1-1.5 km = FP;
                // gerçek olan tek vaka 29.4 km (1764 km/h) ve MD de YALNIZ onu raporluyor.
                let impossible_km = threshold / 60.0;
                if dep_secs == 0 && arr_secs == 0 {
                    if dist_km >= impossible_km {
                        let mut n012 = k6_notice(
                            ctr, "STM_012", EntityType::Trip,
                            Some(trip_id.to_string()), Some(trip_id.to_string()),
                            "stop_times.txt", Some(b.line as u64), Some("arrival_time"),
                            Some(format!("sıfır süre, {dist_km:.1} km")),
                            Some(format!("< {impossible_km:.1} km (aynı dakika içinde)")),
                            format!("trip_id '{trip_id}'{dep_suffix} stop_sequence {}-{} arası geçiş süresi sıfır ama mesafe {dist_km:.1} km — aynı dakika içinde kapatılamaz (≥ {:.0} km/h gerekirdi, eşik {threshold:.0} km/h).",
                                a.stop_sequence().unwrap_or(0), b.stop_sequence().unwrap_or(0),
                                dist_km * 60.0),
                            "stop_times.txt zaman değerlerini doğrulayın; iki durak aynı dakikaya yazılmış ama aralarındaki mesafe bir dakikada alınamaz.",
                        );
                        let mut d = std::collections::HashMap::new();
                        d.insert("stop_a".to_string(), idx.stop_id_of(a).to_string());
                        d.insert("stop_b".to_string(), idx.stop_id_of(b).to_string());
                        n012.details = Some(d);
                        notices.push(n012);
                    }
                    continue;
                }
                if dist_km > 0.2 {
                    let is_worse = worst_zero_seg.as_ref().map_or(true, |&(d, ..)| dist_km > d);
                    if is_worse {
                        worst_zero_seg = Some((
                            dist_km, b.line as u64,
                            SmolStr::from(idx.stop_id_of(a)), SmolStr::from(idx.stop_id_of(b)),
                            a.stop_sequence().unwrap_or(0),
                            b.stop_sequence().unwrap_or(0),
                        ));
                    }
                }
                continue;
            }
            if arr < dep {
                // STM_008: duraklar arası zaman geriye gidiyor (b durağına varış < a durağından kalkış)
                let seq_a = a.stop_sequence().unwrap_or(0);
                let seq_b = b.stop_sequence().unwrap_or(0);
                let dep_hms = format_hms(dep);
                let arr_hms = format_hms(arr);
                let name_a = stop_name_sd.get(idx.stop_id_of(a)).copied().unwrap_or(idx.stop_id_of(a));
                let name_b = stop_name_sd.get(idx.stop_id_of(b)).copied().unwrap_or(idx.stop_id_of(b));
                let headsign = trip_headsign_sd.get(trip_id).copied().unwrap_or("");
                let hs_sep = if headsign.is_empty() { "" } else { ", " };
                let mut n = k6_notice(
                    ctr, "STM_008", EntityType::Trip,
                    Some(trip_id.to_string()), Some(trip_id.to_string()),
                    "stop_times.txt", Some(b.line as u64), Some("arrival_time"),
                    Some(format!("{arr_hms} < {dep_hms}")),
                    Some(format!(">= {dep_hms}")),
                    format!("'{route_label}' hattı '{trip_id}'{dep_suffix} seferi: {name_a} durağından (sıra {seq_a}) kalkış {dep_hms}, {name_b} durağına (sıra {seq_b}) varış {arr_hms} — varış kalkıştan önce, zaman geriye gidiyor. (yön {dir_sd}{hs_sep}{headsign})"),
                    "stop_times.txt zaman değerlerini gözden geçirin; seferler boyunca zamanlar monoton artmalıdır.",
                );
                let mut d = std::collections::HashMap::new();
                d.insert("stop_a".to_string(), idx.stop_id_of(a).to_string());
                d.insert("stop_b".to_string(), idx.stop_id_of(b).to_string());
                d.insert("stop_a_name".to_string(), name_a.to_string());
                d.insert("stop_b_name".to_string(), name_b.to_string());
                d.insert("seq_a".to_string(), seq_a.to_string());
                d.insert("seq_b".to_string(), seq_b.to_string());
                d.insert("route".to_string(), route_label.to_string());
                d.insert("headsign".to_string(), headsign.to_string());
                d.insert("hs_sep".to_string(), hs_sep.to_string());
                d.insert("dir".to_string(), dir_sd.to_string());
                d.insert("dep".to_string(), dep_hms.clone());
                d.insert("arr".to_string(), arr_hms.clone());
                n.details = Some(d);
                notices.push(n);
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
            let dist_km = trip_shape_speed.get(trip_id)
                .and_then(|&sid| {
                    let pts = shape_pts_speed.get(sid)?;
                    let cum = shape_cum_speed.get(sid)?;
                    if pts.len() < 2 { return None; }
                    // (sid, stop_id) → arc cache: aynı shape'i kullanan seferlerde/segmentlerde
                    // tekrar projeksiyonu önler. stop_id → tek koordinat olduğundan sonuç deterministik.
                    let arc_a = *arc_cache.entry((sid, idx.stop_id_of(a)))
                        .or_insert_with(|| project_arc_km(pts, cum, c1.0, c1.1));
                    let arc_b = *arc_cache.entry((sid, idx.stop_id_of(b)))
                        .or_insert_with(|| project_arc_km(pts, cum, c2.0, c2.1));
                    let d = (arc_b - arc_a).abs();
                    if d > 1e-6 { Some(d) } else { None }
                })
                // Shape projeksiyon kuş uçuşunun 4 katından büyükse durak büyük olasılıkla
                // YANLIŞ shape segmentine eşleşmiştir (shape kendine yaklaşıyor/kesişiyor —
                // grid ağlarında, gidiş-dönüş aynı caddede vb.). Bu durumda projeksiyon
                // güvenilmez; haversine'e düş. Aksi halde LA Metro gibi feed'lerde sahte
                // 1000+ km/h hızlar STM_012/STM_014 yanlış pozitifleri üretir.
                .filter(|&d| d <= 4.0 * haver_km.max(0.05))
                .unwrap_or(haver_km);

            if dist_km < 1e-6 {
                if idx.stop_id_of(a) == idx.stop_id_of(b) {
                    // STM_035: aynı durak ardışık iki kez (terminal/döngü hattı)
                    notices.push(k6_notice(
                        ctr, "STM_035", EntityType::Trip,
                        Some(trip_id.to_string()), Some(trip_id.to_string()),
                        "stop_times.txt", Some(b.line as u64), Some("stop_id"),
                        Some(idx.stop_id_of(a).to_string()),
                        None,
                        format!("trip_id '{trip_id}'{dep_suffix} stop_sequence {}-{} arasında aynı durak ({}) ardışık iki kez ziyaret ediliyor.",
                            a.stop_sequence().unwrap_or(0), b.stop_sequence().unwrap_or(0), idx.stop_id_of(a)),
                        "Terminal veya döngü hattıysa beklenen bir durumdur. Değilse stop_times.txt'teki yinelenen satırı kaldırın.",
                    ));
                } else {
                    // STM_021: farklı stop_id'ler aynı koordinatta — gerçek veri hatası
                    notices.push(k6_notice(
                        ctr, "STM_021", EntityType::Trip,
                        Some(trip_id.to_string()), Some(trip_id.to_string()),
                        "stop_times.txt", Some(b.line as u64), Some("stop_id"),
                        Some(format!("{} → {}", idx.stop_id_of(a), idx.stop_id_of(b))),
                        Some("> 0 m".to_string()),
                        format!("trip_id '{trip_id}'{dep_suffix} stop_sequence {}-{} arası mesafe 0: '{}' ve '{}' farklı duraklar ama aynı koordinatta.",
                            a.stop_sequence().unwrap_or(0), b.stop_sequence().unwrap_or(0), idx.stop_id_of(a), idx.stop_id_of(b)),
                        "stops.txt'te durak koordinatlarını doğrulayın; ardışık farklı duraklar aynı konumda olmamalıdır.",
                    ));
                }
                continue;
            }

            // STM_025: <10s tek başına hata değildir; fiziksel tutarlılığı hız kuralları değerlendirir.
            if dt_sec < 10 {
                let mut n025 = k6_notice(
                    ctr, "STM_025", EntityType::Trip,
                    Some(trip_id.to_string()), Some(trip_id.to_string()),
                    "stop_times.txt", Some(b.line as u64), Some("arrival_time"),
                    Some(format!("{dt_sec}s")), Some("inceleyin".to_string()),
                    format!("trip_id '{trip_id}'{dep_suffix} stop_sequence {}-{} arası planlı süre {dt_sec}s — kısa zamanlama sinyali.",
                        a.stop_sequence().unwrap_or(0), b.stop_sequence().unwrap_or(0)),
                    "Bu süre bilinçliyse işlem gerekmez; değilse stop_times.txt zaman değerlerini kontrol edin.",
                );
                let mut d = std::collections::HashMap::new();
                d.insert("stop_a".to_string(), idx.stop_id_of(a).to_string());
                d.insert("stop_b".to_string(), idx.stop_id_of(b).to_string());
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
                    "stop_times.txt", Some(b.line as u64), Some("stop_id"),
                    Some(format!("{dist_km:.1} km")), Some(format!("<= {stm026_threshold:.0} km")),
                    format!("trip_id '{trip_id}'{dep_suffix} stop_sequence {}-{} arası mesafe {dist_km:.1} km.",
                        a.stop_sequence().unwrap_or(0), b.stop_sequence().unwrap_or(0)),
                    "stops.txt koordinatlarını ve stop_times.txt sırasını doğrulayın.",
                );
                let mut d = std::collections::HashMap::new();
                d.insert("stop_a".to_string(), idx.stop_id_of(a).to_string());
                d.insert("stop_b".to_string(), idx.stop_id_of(b).to_string());
                n026.details = Some(d);
                notices.push(n026);
            }

            let speed = dist_km / (dt_sec as f64 / 3600.0);

            // STM_012: fiziksel olarak imkansız hız (mutlak üst sınır 700 km/h)
            if speed > 700.0 {
                let mut n012 = k6_notice(
                    ctr, "STM_012", EntityType::Trip,
                    Some(trip_id.to_string()), Some(trip_id.to_string()),
                    "stop_times.txt", Some(b.line as u64), Some("arrival_time"),
                    Some(format!("{speed:.0} km/h")), Some("<= 700 km/h".to_string()),
                    format!("trip_id '{trip_id}'{dep_suffix} stop_sequence {}-{} arası hız {speed:.0} km/h — fiziksel olarak imkansız.",
                        a.stop_sequence().unwrap_or(0), b.stop_sequence().unwrap_or(0)),
                    "stop_times.txt zaman ve stops.txt koordinat verilerini doğrulayın.",
                );
                let mut d = std::collections::HashMap::new();
                d.insert("stop_a".to_string(), idx.stop_id_of(a).to_string());
                d.insert("stop_b".to_string(), idx.stop_id_of(b).to_string());
                n012.details = Some(d);
                notices.push(n012);
                trip_bad_seg_count += 1;
                continue;
            }

            if speed > threshold {
                // Emit ertelenir: aynı (hat, yön, segment) için tek notice üretilecek.
                let seg = stm014_segs
                    .entry((route, dir_sd, idx.stop_id_of(a), idx.stop_id_of(b)))
                    .or_insert_with(|| Stm014Seg {
                        route_label,
                        seq_a: a.stop_sequence().unwrap_or(0),
                        seq_b: b.stop_sequence().unwrap_or(0),
                        threshold,
                        line: b.line as u64,
                        speed_min: speed,
                        speed_max: speed,
                        trips: Vec::new(),
                    });
                seg.speed_min = seg.speed_min.min(speed);
                seg.speed_max = seg.speed_max.max(speed);
                seg.line = seg.line.min(b.line as u64);
                seg.trips.push(trip_id);

                if speed > trip_max_speed {
                    trip_max_speed = speed;
                    trip_max_speed_line = Some(b.line as u64);
                }
                trip_bad_seg_count += 1;
                bad_seg_stops.push((SmolStr::from(idx.stop_id_of(a)), SmolStr::from(idx.stop_id_of(b))));
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
                    '{trip_id}'{dep_suffix} seferindeki {trip_bad_seg_count} bozuk segmentte en yüksek hız {trip_max_speed:.1} km/h — eşik {threshold:.0} km/h.",
                ),
                "stop_times zaman ve koordinat verilerini kontrol edin.",
            );
            // Tüm bozuk segment çiftleri → UI'da her biri kırmızı polyline
            // Ayrıca kalkış saati (varsa) details'e → EN/JA mesaj şablonunda gösterilebilir.
            {
                let mut d = std::collections::HashMap::new();
                for (i, (sa, sb)) in bad_seg_stops.iter().enumerate() {
                    d.insert(format!("bad_seg_{i}_a"), sa.to_string());
                    d.insert(format!("bad_seg_{i}_b"), sb.to_string());
                }
                // Kalkış saati → details'e. EN/JA şablonu {dep_suffix} ile gösterir;
                // boşken suffix tamamen kaybolur (koşullu boşluk için Rust'ta üretilir).
                d.insert("departure".to_string(), dep_str.clone());
                d.insert(
                    "dep_suffix".to_string(),
                    if dep_str.is_empty() { String::new() } else { format!(" ({dep_str})") },
                );
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

    // ── STM_014: biriken segmentleri tek tek emit et ─────────────────────────
    // FxHashMap iterasyonu sırasız → anahtara göre sırala (golden determinizmi).
    {
        let _t = Timer::start("K6::sd::stm014_emit");
        let mut keys: Vec<(&str, &str, &str, &str)> = stm014_segs.keys().copied().collect();
        keys.sort_unstable();
        for key in keys {
            let seg = &stm014_segs[&key];
            let (route, dir, stop_a, stop_b) = key;
            let n_trips = seg.trips.len();
            let Stm014Seg { route_label, seq_a, seq_b, threshold, line, speed_min, speed_max, .. } = *seg;

            // Sefer listesi deterministik + tekilleştirilmiş (aynı sefer segmenti bir kez geçer,
            // ama loop hatlarda tekrar edebilir).
            let mut trips: Vec<&str> = seg.trips.clone();
            trips.sort_unstable();
            trips.dedup();

            let observed = if (speed_max - speed_min).abs() < 0.05 {
                format!("{speed_max:.1} km/h ({stop_a} → {stop_b})")
            } else {
                format!("{speed_min:.1}-{speed_max:.1} km/h ({stop_a} → {stop_b})")
            };
            let msg = if n_trips == 1 {
                format!(
                    "'{route_label}' kodlu hattın {dir} yönünde '{}' seferinde durak sırası {seq_a}-{seq_b} \
                    arası hız {speed_max:.1} km/h — eşik {threshold:.0} km/h.",
                    trips[0]
                )
            } else {
                format!(
                    "'{route_label}' kodlu hattın {dir} yönünde durak sırası {seq_a}-{seq_b} arası hız eşiği \
                    {n_trips} seferde aşılıyor — {speed_min:.1}-{speed_max:.1} km/h, eşik {threshold:.0} km/h. \
                    Tek segment, tek kök neden: seferler ayrı ayrı listelenmez."
                )
            };
            let mut n = k6_notice(
                ctr,
                "STM_014",
                EntityType::Route,
                // entity_id segmenti BENZERSİZ tanımlamalı: dedup=Entity olduğu için
                // aynı hattaki farklı segmentler aksi halde birbirini yutar.
                Some(format!("{route}:{dir}:{stop_a}→{stop_b}")),
                // scope_key = route_id → UI'da hat bazlı filtre çalışmaya devam eder.
                Some(route.to_string()),
                "stop_times.txt",
                Some(line),
                Some("arrival_time"),
                Some(observed),
                Some(format!("≤ {threshold:.0} km/h")),
                msg,
                "Zaman damgalarını veya durak koordinatlarını doğrulayın; ardışık stop_times arasındaki hız eşiği aşıyor.",
            );
            let mut d = std::collections::HashMap::new();
            d.insert("stop_a".to_string(), stop_a.to_string());
            d.insert("stop_b".to_string(), stop_b.to_string());
            d.insert("route_id".to_string(), route.to_string());
            // route_label = route_short_name (yoksa route_id) — EN/JA şablonları
            // diğer route-scoped kurallarla aynı biçimde {route_label} kullanır.
            d.insert("route_label".to_string(), route_label.to_string());
            d.insert("direction_id".to_string(), dir.to_string());
            d.insert("trip_count".to_string(), trips.len().to_string());
            d.insert("speed_min".to_string(), format!("{speed_min:.1}"));
            d.insert("speed_max".to_string(), format!("{speed_max:.1}"));
            d.insert("trips".to_string(), trips.iter().take(STM014_TRIP_SAMPLE).copied().collect::<Vec<_>>().join(","));
            n.details = Some(d);
            notices.push(n);
        }
    }

    // STM_048 / STM_049: gece yarısı sonrası saatleri 00:xx yazan feed'ler için bilgi notu.
    // STM_048 = duraklar arası (normalize edilen trip'ler); STM_049 = aynı satır (00:xx kalkış).
    {
        let wrapped = records.stop_times_index.midnight_wrapped_trips;
        if wrapped > 0 {
            notices.push(k6_notice(
                ctr, "STM_048", EntityType::Feed,
                None, None, "stop_times.txt", None, None,
                Some(format!("{wrapped}")), None,
                format!("{wrapped} sefer gece yarısı sonrası saatleri 00:xx olarak yazmış; GTFS servis günü için 24:00:00+ önerilir. Analiz için otomatik 24:xx olarak yorumlandı."),
                "Gece yarısını aşan saatleri 24:00:00, 25:00:00… biçiminde yazın (00:xx yerine).",
            ));
        }
        if stm007_midnight_count > 0 {
            notices.push(k6_notice(
                ctr, "STM_049", EntityType::Feed,
                None, None, "stop_times.txt", None, None,
                Some(format!("{stm007_midnight_count}")), None,
                format!("{stm007_midnight_count} durakta gece yarısı sonrası kalkış 00:xx yazılmış (aynı satırda kalkış varıştan önce görünüyor); 24:xx önerilir. STM_007 bu vakalarda bastırıldı."),
                "Gece yarısını aşan kalkış saatlerini 24:00:00+ biçiminde yazın.",
            ));
        }
    }
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
                Some(frq.line as u64),
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
                Some(frq.line as u64),
                Some("headway_secs"),
                Some(format!("{hw}s")),
                Some(format!("> {}s", bunching_secs)),
                format!("'{trip_id}' seferinde sefer aralığı {hw}sn ≤ sıkışma eşiği {bunching_secs}sn — sıkışma riski."),
                "Sefer programını gözden geçirin; bu headway gerçekten isteniyorsa dikkate almayın.",
            ));
        }
    }
}

/// Kalkış aralıkları "düzenli" mi — varyasyon katsayısı (CV = std/ortalama) < 0.25.
/// Düzenli aralık = kasıtlı seyrek servis (ör. kırsal sabah/akşam seferleri) → OPR_001
/// bastırılır. Düzensiz (çoğu sık + bir dev boşluk) = gerçek servis boşluğu → OPR_001 üretilir.
/// Tek aralık (2 sefer) varyans taşımaz → kasıtlı kabul edilir.
fn is_regular_headway(gaps: &[u32]) -> bool {
    if gaps.len() < 2 {
        return true;
    }
    let n = gaps.len() as f64;
    let mean = gaps.iter().map(|&g| g as f64).sum::<f64>() / n;
    if mean <= 0.0 {
        return true;
    }
    let var = gaps.iter().map(|&g| (g as f64 - mean).powi(2)).sum::<f64>() / n;
    (var.sqrt() / mean) < 0.25
}

// ── WP-09b: Route headway (düzenli servis) ────────────────────────────────────

fn check_route_headway(
    records: &EntityRecords,
    config: &ValidatorConfig,
    idx: &StopTimesIndex<'_>,
    notices: &mut Vec<Notice>,
    ctr: &mut u32,
) {
    let ti_hw = &records.trip_interns;
    let route_short_hw: HashMap<&str, &str> = records.routes.iter()
        .map(|r| {
            let label = r.route_short_name.as_deref().filter(|s| !s.is_empty()).unwrap_or(r.route_id.as_str());
            (r.route_id.as_str(), label)
        })
        .collect();

    // İki ayrı havuz, çünkü iki kural farklı şeyleri ölçer:
    //  • OPR_001 (seyrek servis / en büyük aralık) hattın TOPLAM frekansına bakar →
    //    kaba anahtar (route_id, direction_key, service_id).
    //  • OPR_003 (sıkışma / en küçük aralık) yalnızca AYNI servis desenindeki ardışık
    //    seferler arasında anlamlıdır → ince anahtar (first_stop_id, pattern) içerir.
    // Aksi hâlde (a) farklı terminallerden ~aynı saatte kalkan seferler ya da
    // (b) aynı duraktan farklı HEDEFE giden seferler (örn. B100: "Bedford Av" 3-durak
    // stub vs "Mill Basin" 29-durak tam run) sahte "1dk sıkışma" üretir. pattern =
    // shape_id (yoksa son durak); first_stop ANAHTARDA KALIR, çünkü shape yoksa farklı
    // başlangıçtan aynı varışa giden seferlerin birleşip terminal-FP'sini geri
    // getirmesini engeller. OPR_001'i ilk-durağa/desene bölmek ise yalnız belirli
    // saatlerde sefer başlatan kısa-hat varyantlarını yanlışlıkla "seyrek" gösterir.
    // direction_key: direction_id varsa "0"/"1"/..., yoksa "" (ayrı bucket açmaz).
    let mut route_departures: HashMap<(&str, &str, &str), Vec<u32>> = HashMap::new();
    let mut route_stop_departures: HashMap<(&str, &str, &str, &str, &str), Vec<u32>> =
        HashMap::new();

    for trip in &records.trips {
        let route_id = ti_hw.route_id(trip);
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
        let service_id = ti_hw.service_id(trip);

        let Some(&dep) = idx.trip_first_dep.get(trip.trip_id.as_str()) else {
            continue;
        };

        let stops = idx.by_trip.get(trip.trip_id.as_str());
        let first_stop: &str = stops
            .and_then(|v| v.first())
            .map(|s| idx.stop_id_of(s))
            .unwrap_or("");
        // Servis deseni: shape_id varsa onu, yoksa son durağı (varış) kullan.
        let pattern: &str = match ti_hw.shape_id(trip).filter(|s| !s.is_empty()) {
            Some(s) => s,
            None => stops
                .and_then(|v| v.last())
                .map(|s| idx.stop_id_of(s))
                .unwrap_or(""),
        };

        route_departures
            .entry((route_id, direction_key, service_id))
            .or_default()
            .push(dep);
        route_stop_departures
            .entry((route_id, direction_key, service_id, first_stop, pattern))
            .or_default()
            .push(dep);
    }

    let max_secs = config.max_headway_warning_min * 60;
    let bunching_secs = config.bunching_threshold_min * 60;

    // OPR_001: hattın tamamında en büyük ardışık kalkış boşluğu eşiği aşıyor mu?
    for ((route_id, direction_key, service_id), mut deps) in route_departures {
        deps.sort_unstable();
        deps.dedup();
        if deps.len() < 2 {
            continue;
        }
        let gaps: Vec<u32> = deps.windows(2).map(|w| w[1] - w[0]).collect();
        let max_hw = gaps.iter().copied().max().unwrap_or(0);
        // OPR_001 yalnız (a) boşluk eşiği aşar, (b) hat manuel kırsal listesinde DEĞİL,
        // (c) kalkış aralıkları düzensiz (gerçek boşluk) ise üretilir. Düzenli-seyrek
        // (düşük CV) hatlar kasıtlı kırsal servis kabul edilip susturulur.
        if max_hw > max_secs
            && !config.rural_route_ids.iter().any(|r| r == route_id)
            && !is_regular_headway(&gaps)
        {
            let dir_display = if direction_key.is_empty() { "-" } else { direction_key };
            let route_label_hw = route_short_hw.get(route_id).copied().unwrap_or(route_id);
            let mut n001 = k6_notice(
                ctr,
                "OPR_001",
                EntityType::Route,
                Some(route_id.to_string()),
                Some(route_id.to_string()),
                "stop_times.txt",
                None,
                None,
                Some(format!("{:.0}", max_hw as f64 / 60.0)),
                Some(format!("≤ {}", config.max_headway_warning_min)),
                format!("'{route_label_hw}' kodlu hattın {dir_display} yönünde {service_id} çalışma takviminde maksimum sefer aralığı {:.0}dk — eşik {}dk.",
                    max_hw as f64 / 60.0, config.max_headway_warning_min),
                "Pik/saatdışı sefer sayısını artırın ya da büyük boşlukları kapatın.",
            );
            n001.service_id = Some(service_id.to_string());
            notices.push(n001);
        }
    }

    // OPR_003: aynı kalkış durağında ardışık seferler arası en küçük pozitif aralık
    // sıkışma eşiğinin altında mı?
    for ((route_id, direction_key, service_id, _first_stop, _pattern), mut deps) in route_stop_departures {
        deps.sort_unstable();
        // dedup suppresses cross-day repeated departure times for the same
        // service_id; may also hide true parallel same-time trips (known tradeoff)
        deps.dedup();
        if deps.len() < 2 {
            continue;
        }
        let min_hw = deps.windows(2).map(|w| w[1] - w[0]).min().unwrap_or(0);
        if min_hw < bunching_secs && min_hw > 0 {
            let dir_display = if direction_key.is_empty() { "-" } else { direction_key };
            let route_label_hw = route_short_hw.get(route_id).copied().unwrap_or(route_id);
            let mut n = k6_notice(
                ctr,
                "OPR_003",
                EntityType::Route,
                Some(route_id.to_string()),
                Some(route_id.to_string()),
                "stop_times.txt",
                None,
                None,
                Some(format!("{:.0}", min_hw as f64 / 60.0)),
                Some(format!("> {}", config.bunching_threshold_min)),
                format!("'{route_label_hw}' kodlu hattın {dir_display} yönünde {service_id} çalışma takviminde minimum sefer aralığı {:.0}dk ≤ sıkışma eşiği {}dk.",
                    min_hw as f64 / 60.0, config.bunching_threshold_min),
                "Sefer programını düzenleyin; çok sık gelen seferler sıkışmaya yol açabilir.",
            );
            n.service_id = Some(service_id.to_string());
            notices.push(n);
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
    let ti_cal = &records.trip_interns;
    // CAL_021 için: her servisin kaç sefer (trip) tarafından kullanıldığı.
    let mut service_trip_counts: HashMap<&str, u32> = HashMap::new();
    for t in &records.trips {
        if !ti_cal.service_id(t).is_empty() {
            *service_trip_counts.entry(ti_cal.service_id(t)).or_insert(0) += 1;
        }
    }

    // CAL_010: toplam aktif gün sayısı çok az (≤ service_gap_days) — per-service.
    // CAL_007/012: servis dönemindeki boşluklar (gap_start, gap_end) imzasıyla TÜM servisler
    // arasında toplanır (#30) → gap başına tek notice, etkilenen service_id'ler listelenir.
    // near_future imzanın (bugüne bağlı) fonksiyonudur, servise değil → her imza tekdüze
    // CAL_007 ya da CAL_012 üretir. Emit döngü SONRASINDA yapılır.
    let today_jdn = yyyymmdd_to_jdn(today_yyyymmdd);
    let gap_threshold = config.service_gap_days;   // CAL_010: çok-kısa servis eşiği (aktif gün)
    let big_gap_threshold = config.big_gap_days;   // CAL_007/012: büyük-boşluk eşiği (MD ≈ 14)
    let mut gap_services: std::collections::BTreeMap<(u32, u32), Vec<&str>> =
        std::collections::BTreeMap::new();
    for (service_id, dates) in &derived.calendar_bitmap.active_dates {
        if dates.is_empty() {
            continue;
        }
        let mut sorted: Vec<u32> = dates.iter().copied().collect();
        sorted.sort_unstable();

        let total_days = sorted.len() as u32;

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
                Some(format!("{total_days}")),
                Some(format!("> {gap_threshold}")),
                format!("'{service_id}' takviminde yalnızca {total_days} aktif gün var."),
                "Servis takvimini genişletin ya da bu servisin kısa olduğunu belgeleyin.",
            ));
        }

        // CAL_007/012: bu servisin ardışık aktif tarihleri arasında >= big_gap_days boşluk.
        // MobilityData big_gap_in_service PER-SERVICE'tir (feed-level union DEĞİL). Boşluklar
        // (gap_start, gap_end) imzasıyla TÜM servisler arasında toplanır (#30) → emit döngü SONRASI.
        for pair in sorted.windows(2) {
            let gap_days = yyyymmdd_to_jdn(pair[1])
                .saturating_sub(yyyymmdd_to_jdn(pair[0]))
                .saturating_sub(1);
            if gap_days >= big_gap_threshold {
                gap_services.entry((pair[0], pair[1])).or_default().push(service_id.as_str());
            }
        }

        // CAL_021: servis aktif dönemi bugünü kapsıyor (min ≤ today ≤ max) AMA
        // önümüzdeki N günde (config.upcoming_service_days) hiç aktif günü yok.
        // Tamamen geçmiş (CAL_013) / tamamen gelecek (CAL_017) servisleri "spans today"
        // koşulu dışlar → çakışma yok. Yalnızca sefer kullanan servisler için.
        if today_yyyymmdd > 0 {
            let min_d = sorted[0];
            let max_d = *sorted.last().unwrap();
            if min_d <= today_yyyymmdd && max_d >= today_yyyymmdd {
                let window_end_jdn = today_jdn + config.upcoming_service_days;
                let has_upcoming = sorted.iter().any(|&d| {
                    let jdn = yyyymmdd_to_jdn(d);
                    jdn >= today_jdn && jdn <= window_end_jdn
                });
                let trip_count = service_trip_counts.get(service_id.as_str()).copied().unwrap_or(0);
                if !has_upcoming && trip_count > 0 {
                    let n = config.upcoming_service_days;
                    notices.push(k6_notice(
                        ctr,
                        "CAL_021",
                        EntityType::Service,
                        Some(service_id.clone()),
                        Some(service_id.clone()),
                        "calendar.txt",
                        None,
                        None,
                        Some(format!("0 / {n}")),
                        Some(format!("≥ 1 / {n}")),
                        format!("'{service_id}' servisi bugünü kapsıyor ama önümüzdeki {n} günde aktif günü yok ({trip_count} sefer etkileniyor)."),
                        "Yakın günlerde sefer olmaması kasıtlıysa yok sayın; değilse calendar/calendar_dates ile yakın tarihlere aktif gün ekleyin.",
                    ));
                }
            }
        }
    }

    // CAL_007/012 emit: gap imzası başına TEK notice (#30). Aynı operasyonel boşluğu
    // paylaşan service_id varyantları details["services"]'te listelenir (VAT_002 deseni).
    // #29 mantığı korunur: yakın-gelecek boşluk CAL_012, geçmiş/uzak boşluk CAL_007.
    for ((gap_from, gap_to), mut svcs) in gap_services {
        svcs.sort_unstable();
        svcs.dedup();
        let a_jdn = yyyymmdd_to_jdn(gap_from);
        let b_jdn = yyyymmdd_to_jdn(gap_to);
        let gap_days = b_jdn.saturating_sub(a_jdn).saturating_sub(1);
        // Boşluk bugün/yakın gelecekle (bugünden +30 gün) örtüşüyor mu?
        let near_future =
            b_jdn.saturating_sub(1) >= today_jdn && (a_jdn + 1) <= today_jdn + 30;
        let svc_suffix = if svcs.len() == 1 {
            format!("'{}' servisinde", svcs[0])
        } else {
            format!("{} serviste: {}", svcs.len(), svcs.join(", "))
        };
        let entity = format!("{gap_from}-{gap_to}");
        let mut n = if near_future {
            k6_notice(
                ctr,
                "CAL_012",
                EntityType::Service,
                Some(entity.clone()),
                Some(entity.clone()),
                "calendar.txt",
                None,
                None,
                Some(format!("{gap_days}")),
                Some(format!("< {big_gap_threshold}")),
                format!("{gap_from}-{gap_to} arası {gap_days} günlük yakın dönem inaktif aralık {svc_suffix}."),
                "Bu aralık planlıysa işlem gerekmez; değilse servis takvimini doğrulayın.",
            )
        } else {
            k6_notice(
                ctr,
                "CAL_007",
                EntityType::Service,
                Some(entity.clone()),
                Some(entity.clone()),
                "calendar.txt",
                None,
                None,
                Some(format!("{gap_days}")),
                Some(format!("< {big_gap_threshold}")),
                format!("{gap_from}-{gap_to} arası {gap_days} günlük boşluk {svc_suffix}."),
                "Boşluk kasıtlıysa calendar_dates ile açıklayın; değilse takvimi düzeltin.",
            )
        };
        n.details = Some({
            let mut d = std::collections::HashMap::new();
            d.insert("services".to_string(), svcs.join(","));
            d
        });
        notices.push(n);
    }

    // CAL_008 / CAL_009: expiry (bitiş tarihine göre)
    for cal in &records.calendars {
        let Some((ey, em, ed)) = cal.end_date else { continue };
        let end_yyyymmdd = ey * 10000 + em * 100 + ed;

        if today_yyyymmdd > 0 {
            // CAL_023: end_date bugünden max_calendar_future_years veya daha fazla ileri →
            // şüpheli/düşük-kaliteli veri (ör. 2050/2099 yer-tutucu tarihler).
            // `>=` kullanıyoruz: sınır yılına denk gelen tarihler de dahil ("3 yıl veya daha fazla").
            let today_year = today_yyyymmdd / 10000;
            if ey >= today_year + config.max_calendar_future_years {
                notices.push(k6_notice(
                    ctr, "CAL_023", EntityType::Service,
                    Some(cal.service_id.clone()), Some(cal.service_id.clone()),
                    "calendar.txt", Some(cal.line as u64), Some("end_date"),
                    Some(format!("{end_yyyymmdd}")),
                    Some(format!("< {}0101", today_year + config.max_calendar_future_years)),
                    format!("'{}' takviminin end_date'i {end_yyyymmdd} — bugünden {} yıl veya daha fazla ileri; şüpheli/düşük-kaliteli veri (yer-tutucu tarih) olabilir.",
                        cal.service_id, config.max_calendar_future_years),
                    "end_date'i gerçek servis bitiş tarihine güncelleyin; uzak-gelecek tarihler genelde yer-tutucu veya hatalıdır.",
                ));
            }
            // CAL_008: yakında bitiyor (end_date gelecekte ve uyarı eşiği içinde).
            // CAL_013 (süresi dolmuş) artık BURADA DEĞİL — calendar.txt end_date yerine
            // birleşik active_dates'e göre CAL_017 döngüsünde hesaplanır; böylece
            // calendar_dates ile ileri taşınan servisler yanlış "dolmuş" sayılmaz (FP fix).
            let warning_days = config.feed_expiry_warning_days;
            let end_jdn = yyyymmdd_to_jdn(end_yyyymmdd);
            let today_jdn = yyyymmdd_to_jdn(today_yyyymmdd);
            if end_jdn > today_jdn && end_jdn - today_jdn <= warning_days {
                notices.push(k6_notice(
                    ctr,
                    "CAL_008",
                    EntityType::Service,
                    Some(cal.service_id.clone()),
                    Some(cal.service_id.clone()),
                    "calendar.txt",
                    Some(cal.line as u64),
                    Some("end_date"),
                    Some(format!("{end_yyyymmdd}")),
                    Some(format!("> {}", warning_days)),
                    format!("'{}' takvimi {end_yyyymmdd} tarihinde bitiyor — {} gün kaldı.",
                        cal.service_id, end_jdn - today_jdn),
                    "Feed'i güncellemeyi planlayın.",
                ));
            }
        }
    }

    // CLD_007: aynı service_id için çok sayıda calendar_dates exception (> total aktif gün / 2)
    // exception_count = ham geçerli satır sayısı (CalendarDateIndex semantiği)
    for (service_id, &count) in &records.calendar_dates.exception_count {
        let service_id = service_id.as_str();
        let active = derived
            .calendar_bitmap
            .active_dates
            .get(service_id)
            .map(|s| s.len() as u32)
            .unwrap_or(0);
        // CAL_013 (süresi dolmuş) buradan kaldırıldı → tüm servisler için birleşik
        // active_dates üzerinden CAL_017 döngüsünde tek yerde hesaplanır (çift-emit önlenir).

        // CAL_014: servis aktif tarihleri feed_info geçerlilik penceresinin dışında
        if let Some(fi) = records.feed_info.first() {
            if let (Some(fs), Some(fe)) = (fi.feed_start_date, fi.feed_end_date) {
                let fs_yyyymmdd = fs.0 * 10000 + fs.1 * 100 + fs.2;
                let fe_yyyymmdd = fe.0 * 10000 + fe.1 * 100 + fe.2;
                if let Some(dates) = derived.calendar_bitmap.active_dates.get(service_id) {
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

        // CAL_017: tüm aktif tarihler gelecekte (henüz başlamamış) — per-service.
        // CAL_013: tüm aktif tarihler geçmişte (süresi dolmuş). Aynı son-aktif-tarihte (expiry)
        // biten service_id varyantları TEK notice'ta toplanır (#30 deseni), details["services"]'te
        // listelenir. Kaynak BİRLEŞİK active_dates → calendar_dates ile ileri taşınan servisler
        // expired sayılmaz (FP fix). min>today ile max<today karşılıklı dışlayıcıdır.
        let mut expired_services: std::collections::BTreeMap<u32, Vec<&str>> =
            std::collections::BTreeMap::new();
        for (service_id, dates) in &derived.calendar_bitmap.active_dates {
            if dates.is_empty() { continue; }
            let min_svc = dates.iter().copied().min().unwrap();
            let max_svc = dates.iter().copied().max().unwrap();
            if min_svc > today_yyyymmdd {
                notices.push(k6_notice(
                    ctr, "CAL_017", EntityType::Service,
                    Some(service_id.to_string()), Some(service_id.to_string()),
                    "calendar.txt", None, Some("start_date"),
                    Some(format!("{min_svc}")), Some(format!("≤ {today_yyyymmdd}")),
                    format!("'{service_id}' takvimi henüz başlamamış; en erken aktif tarih {min_svc}."),
                    "Takvim başlangıç tarihini veya calendar_dates.txt girişlerini gözden geçirin.",
                ));
            } else if max_svc < today_yyyymmdd {
                expired_services.entry(max_svc).or_default().push(service_id.as_str());
            }
        }
        // CAL_013 emit: son-aktif-tarih (expiry) imzası başına TEK notice.
        for (max_svc, mut svcs) in expired_services {
            svcs.sort_unstable();
            svcs.dedup();
            let msg = if svcs.len() == 1 {
                format!("'{}' takviminin süresi dolmuş (son aktif tarih: {max_svc}) — bu servise ait seferler bugün için bulunamaz.", svcs[0])
            } else {
                format!("Son aktif tarihi {max_svc} olan {} servisin süresi dolmuş: {} — bu servislere ait seferler bugün için bulunamaz.", svcs.len(), svcs.join(", "))
            };
            let mut n = k6_notice(
                ctr, "CAL_013", EntityType::Service,
                Some(format!("{max_svc}")), Some(format!("{max_svc}")),
                "calendar.txt", None, Some("end_date"),
                Some(format!("{max_svc}")), Some(format!("≥ {today_yyyymmdd}")),
                msg,
                "Feed'i yeni geçerlilik tarihleriyle güncelleyin veya servisleri silin.",
            );
            n.details = Some({
                let mut d = std::collections::HashMap::new();
                d.insert("services".to_string(), svcs.join(","));
                d
            });
            notices.push(n);
        }

        // CAL_016: en geç aktif tarih 2 yıldan fazla ileriye uzanıyor
        let today_jdn = yyyymmdd_to_jdn(today_yyyymmdd);
        let far_future_jdn = today_jdn + 730;
        let max_date = derived.calendar_bitmap.active_dates.values()
            .flat_map(|s| s.iter().copied())
            .max();
        if let Some(last) = max_date {
            let last_jdn = yyyymmdd_to_jdn(last);
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
    let ti_geo = &records.trip_interns;

    // GEO_006: shape segmentleri arası büyük atlama (jump)
    // GEO_007: aynı — GEO_006'dan biraz farklı ciddiyette ama aynı kontrol
    let _tg6 = Timer::start("K6::geo::geo_006");
    let max_jump_km = config.max_shape_jump_km;
    // GEO_007 aynı segmentleri 3× eşikle tarıyor. Eskiden iki koşul birbirini
    // dışlamıyordu → 3× üstü bir atlama HEM GEO_006 HEM GEO_007 üretiyor, aynı segment
    // iki kez raporlanıp R5 skorunda iki kez sayılıyordu. Artık katmanlı: bu aralık
    // GEO_006'nın, üstü GEO_007'nin.
    let severe_km = max_jump_km * 3.0;
    for (shape_id, seg) in &derived.shape_geometry.shapes {
        for (i, &dist) in seg.segment_distances_km.iter().enumerate() {
            if dist > severe_km {
                continue; // → GEO_007
            }
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
                // parent/child istisnası: çocuk durak, ait olduğu istasyonla aynı/çok
                // yakın konumda olabilir — bu normal GTFS modellemesidir, FP üretmeyelim.
                let pa = sa.row.get("parent_station").map(|s| s.trim()).filter(|s| !s.is_empty());
                let pb = sb.row.get("parent_station").map(|s| s.trim()).filter(|s| !s.is_empty());
                if pa == Some(sb.stop_id.as_str()) || pb == Some(sa.stop_id.as_str()) {
                    continue;
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
                        Some(sa.line as u64),
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
                        Some(sa.line as u64),
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
                    Some(stop.line as u64),
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
                Some(format!("{span_km:.0} km")),
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
                        "stops.txt", Some(stop.line as u64), Some("stop_lat|stop_lon"),
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
                "stops.txt", Some(stop.line as u64), Some("stop_lat|stop_lon"),
                Some(format!("{lat:.6},{lon:.6}")), None,
                format!("'{name}' durağının koordinatları ({lat:.6},{lon:.6}) Null Island yakınında — olası koordinat hatası."),
                "stop_lat ve stop_lon değerlerinin gerçek konuma karşılık geldiğini doğrulayın.",
            ));
        }
    }

    // GEO_022: Stop enlemi kutba aşırı yakın (|lat| > 89) — olası koordinat hatası (point_near_pole)
    for stop in &records.stops {
        if stop.stop_id.is_empty() { continue; }
        let Some(lat) = stop.stop_lat else { continue };
        if lat.abs() > 89.0 {
            let name = stop.stop_name.as_deref().filter(|s| !s.is_empty()).unwrap_or(&stop.stop_id);
            notices.push(k6_notice(
                ctr, "GEO_022", EntityType::Stop,
                Some(stop.stop_id.clone()), Some(stop.stop_id.clone()),
                "stops.txt", Some(stop.line as u64), Some("stop_lat"),
                Some(format!("{lat:.6}")), None,
                format!("'{name}' durağının enlemi ({lat:.6}) kutba aşırı yakın — olası koordinat hatası."),
                "stop_lat değerinin gerçek konuma karşılık geldiğini doğrulayın.",
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
                    "shapes.txt", Some(pt.line as u64), Some("shape_pt_lat|shape_pt_lon"),
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
                "stops.txt", Some(stop.line as u64), Some("stop_lat|stop_lon"),
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
        // Sadece fiziksel duraklar (location_type=0 veya null). İstasyon (lt=1) ve
        // child platformu aynı koordinatta olmak NORMALDİR (hiyerarşi); bunları
        // saymak Tokyo Toei gibi istasyon/platform modelleyen feed'lerde sahte
        // "sistematik koordinat sorunu" üretir.
        let mut coord_counts: HashMap<(i64, i64), u32> = HashMap::new();
        for stop in &records.stops {
            if stop.location_type.unwrap_or(0) != 0 { continue; }
            let (Some(lat), Some(lon)) = (stop.stop_lat, stop.stop_lon) else { continue };
            let key = ((lat * 1e6).round() as i64, (lon * 1e6).round() as i64);
            *coord_counts.entry(key).or_default() += 1;
        }
        let total_stops = records.stops.iter()
            .filter(|s| s.location_type.unwrap_or(0) == 0 && s.stop_lat.is_some() && s.stop_lon.is_some())
            .count();
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

    // STM_045: Trip kalkış saati servis-günü penceresini aşıyor (olası veri anomalisi).
    // Servis günü [start, start+24) → start+24 saati aşan departure şüphelidir. Eşik
    // service_day_start_hour'a bağlıdır (ör. 04:00 başlayan operatörde 28h'e kadar meşru);
    // sabit 26h yanlış-pozitif üretiyordu. start_hour=0 (normalizasyon kapalı) → eski 26h.
    {
        let max_dep_h = if config.service_day_start_hour == 0 {
            26
        } else {
            24 + config.service_day_start_hour
        };
        for (trip_id, stops) in records.stop_times_index.iter_trips() {
            for st in stops {
                let Some((h, m, s)) = st.departure_time() else { continue };
                if h > max_dep_h || (h == max_dep_h && (m > 0 || s > 0)) {
                    notices.push(k6_notice(
                        ctr, "STM_045", EntityType::Trip,
                        Some(trip_id.to_string()), Some(trip_id.to_string()),
                        "stop_times.txt", Some(st.line as u64), Some("departure_time"),
                        Some(format!("{h:02}:{m:02}:{s:02}")), Some(format!("≤{max_dep_h}:00:00")),
                        format!("'{trip_id}' seferinde {h:02}:{m:02}:{s:02} kalkış saati — servis günü penceresini ({max_dep_h} saat) aşıyor."),
                        "departure_time ve service_day_start_hour ayarını kontrol edin; bu pencereyi aşan değerler genellikle veri hatasıdır.",
                    ));
                    break; // trip başına bir notice yeterli
                }
            }
        }
    }

    // SHP_027: Bir shape, FARKLI durak dizilerine (pattern) sahip seferlere atanmış.
    // Bir shape tek bir güzergah geometrisidir; ≥2 farklı durak desenine atanmışsa
    // geometri bazı seferlere uymaz → olası yanlış shape ataması. (Sık bir hattın tek
    // pattern'i yüzlerce sefere atanması NORMAL; eski düz sayım eşiği o yüzden kaldırıldı.)
    {
        use std::collections::hash_map::DefaultHasher;
        use std::hash::{Hash, Hasher};
        let trip_shape: HashMap<&str, &str> = records.trips.iter()
            .filter_map(|t| ti_geo.shape_id(t).filter(|s| !s.is_empty())
                .map(|s| (t.trip_id.as_str(), s)))
            .collect();
        // shape_id → (pattern_hash → temsili trip_id). Temsili = leksikografik en küçük
        // trip_id (deterministik; HashMap iterasyon sırasından bağımsız).
        let mut shape_patterns: HashMap<&str, HashMap<u64, &str>> = HashMap::new();
        for (trip_id, stops) in records.stop_times_index.iter_trips() {
            let Some(&shape_id) = trip_shape.get(trip_id.as_str()) else { continue };
            let mut h = DefaultHasher::new();
            for st in stops { records.stop_times_index.stop_id_of(st).hash(&mut h); }
            shape_patterns.entry(shape_id).or_default()
                .entry(h.finish())
                .and_modify(|cur| { if trip_id.as_str() < *cur { *cur = trip_id.as_str(); } })
                .or_insert(trip_id.as_str());
        }
        for (shape_id, patterns) in &shape_patterns {
            let n = patterns.len();
            if n >= 2 {
                let mut notice = k6_notice(
                    ctr, "SHP_027", EntityType::Shape,
                    Some((*shape_id).to_string()), Some((*shape_id).to_string()),
                    "trips.txt", None, None,
                    Some(n.to_string()), Some("1".to_string()),
                    format!("'{shape_id}' shape'i {n} farklı durak dizisine (pattern) atanmış — bir shape tek güzergaha karşılık gelmeli; olası yanlış shape ataması."),
                    "Her farklı durak desenine ayrı shape_id atayın.",
                );
                // Haritada her deseni ayrı renkte çizmek için temsili trip_id'ler (UI fix.ts SHP_027).
                // İlk 12 desenle sınırlı (palet boyutu + payload); observed_value (n) tam sayıyı korur.
                let mut rep_trips: Vec<&str> = patterns.values().copied().collect();
                rep_trips.sort_unstable();
                rep_trips.truncate(12);
                let mut d = std::collections::HashMap::new();
                d.insert("pattern_trips".to_string(), rep_trips.join(","));
                notice.details = Some(d);
                notices.push(notice);
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
                let dep_suffix = stops.first().and_then(|s| s.departure_time())
                    .map(|(h, m, _)| format!(" {h:02}:{m:02} kalkışlı")).unwrap_or_default();
                notices.push(k6_notice(
                    ctr, "STM_043", EntityType::Trip,
                    Some((*trip_id).to_string()), Some((*trip_id).to_string()),
                    "stop_times.txt", None, None,
                    Some(count.to_string()), Some("≤200".to_string()),
                    format!("'{trip_id}'{dep_suffix} seferinde {count} durak var — olası veri birleştirme hatası."),
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
    let ti_opr = &records.trip_interns;

    let trip_to_route: HashMap<&str, &str> = records.trips.iter()
        .map(|t| (t.trip_id.as_str(), ti_opr.route_id(t)))
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
            let line = stimes.first().map(|s| s.line as u64);
            notices.push(k6_notice(
                ctr,
                "OPR_006",
                EntityType::Trip,
                Some(trip_id.to_string()),
                Some(trip_id.to_string()),
                "stop_times.txt",
                line,
                None,
                Some(format!("{count}")),
                Some("≥ 2".to_string()),
                {
                    let r = trip_to_route.get(trip_id).copied().unwrap_or(trip_id);
                    format!("'{r}' hattının seferinde yalnızca {count} durak var — işlevsel sefer için en az 2 durak gerekli.")
                },
                "En az 2 durak içeren bir sefer tanımlayın.",
            ));
        }
    }

    drop(_topr6);

    // OPR_007: ring terminali ve STM_035'in sahip olduğu ardışık tekrarlar hariç aynı stop_id
    let _topr7 = Timer::start("K6::opr::opr_007");
    {
        let mut stop_counts: HashMap<&str, u32> = HashMap::new();
        for (&trip_id, stimes) in &idx.by_trip {
            stop_counts.clear();
            for st in stimes.iter() {
                if !idx.stop_id_of(st).is_empty() {
                    *stop_counts.entry(idx.stop_id_of(st)).or_default() += 1;
                }
            }
            // Ring/döngüsel hat tespiti: ilk ve son durak aynıysa terminal tekrarı suppress et
            let mut sorted_stops = stimes.iter()
                .filter(|st| !idx.stop_id_of(st).is_empty())
                .collect::<Vec<_>>();
            sorted_stops.sort_by_key(|st| st.stop_sequence().unwrap_or(u32::MAX));
            let first = sorted_stops.first().map(|st| idx.stop_id_of(st)).unwrap_or("");
            let last  = sorted_stops.last() .map(|st| idx.stop_id_of(st)).unwrap_or("");
            let is_ring = !first.is_empty() && first == last;

            if let Some((&dup_stop, &count)) = stop_counts.iter().find(|(&sid, &c)| c > 1 && !(is_ring && sid == first) && {
                let mut prev = None; sorted_stops.iter().enumerate().filter(|(_, st)| idx.stop_id_of(st) == sid).any(|(i, _)| prev.replace(i).is_some_and(|p| i > p + 1)) })
            {
                let route = trip_to_route.get(trip_id).copied().unwrap_or(trip_id);
                let stop_name = stop_name_map.get(dup_stop).copied().unwrap_or(dup_stop);
                // Kalkış saati: aynı hattın çok seferi varsa satırları ayırt edilebilir kılar.
                let dep = stimes.first()
                    .and_then(|s| s.departure_time().or(s.arrival_time()))
                    .map(|(h, m, _)| format!("{h:02}:{m:02} "))
                    .unwrap_or_default();
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
                    format!("'{}' hattının {}seferinde '{}' durağı (kod: '{}') {}× geçiyor — tekrarlayan operasyon deseni.",
                        route, dep, stop_name, dup_stop, count),
                    "Bu rota deseni bilinçliyse işlem gerekmez; değilse stop_times.txt durak sırasını doğrulayın.",
                );
                // Harita için sıralı durak listesi + tekrarlayan durak
                let stop_list: Vec<&str> = stimes.iter()
                    .filter(|st| !idx.stop_id_of(st).is_empty())
                    .map(|st| idx.stop_id_of(st))
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
            let svc = ti_opr.service_id(t);
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
                    Some("0".to_string()),
                    Some("≥ 1".to_string()),
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
                    Some("0".to_string()),
                    Some("≥ 1".to_string()),
                    "Feed'deki hiçbir service_id aktif takvim günü içermiyor — tüm seferler pasif.".to_string(),
                    "calendar.txt veya calendar_dates.txt'de en az bir servis için aktif gün tanımlayın.",
                ));
            }
        }
    }

    // TRP_023: önümüzdeki 7 günde aktif sefer yok
    if today_yyyymmdd > 0 && !records.trips.is_empty() {
        let today_jdn = yyyymmdd_to_jdn(today_yyyymmdd);
        let active_in_7days = derived.calendar_bitmap.active_dates.values().any(|dates| {
            dates.iter().any(|&d| {
                let djdn = yyyymmdd_to_jdn(d);
                djdn >= today_jdn && djdn < today_jdn + 7
            })
        });
        if !active_in_7days {
            notices.push(k6_notice(
                ctr, "TRP_023", EntityType::Feed,
                None, None, "calendar.txt", None, None,
                Some("0".to_string()), None,
                "Önümüzdeki 7 günde aktif sefer bulunamadı — feed yakında devre dışı kalabilir.".to_string(),
                "calendar.txt veya calendar_dates.txt'i güncelleyin ya da yeni bir feed yayınlayın.",
            ));
        }
    }

    // CAL_024 (eski TRP_030): önümüzdeki 7 günde aktif olmayan takvim — service_id başına agregasyon.
    // (Aynı aktif-olmayan takvim binlerce sefere yayılır; aksiyon birimi service_id'dir,
    // sefer değil — bu yüzden sefer-başına notice yerine takvim-başına tek özet üretilir.)
    if today_yyyymmdd > 0 && !records.trips.is_empty() {
        let today_jdn = yyyymmdd_to_jdn(today_yyyymmdd);
        // service_id → (etkilenen sefer sayısı, ilk trip satırı)
        let mut inactive_by_service: FxHashMap<&str, (u32, u64)> = FxHashMap::default();
        for trip in &records.trips {
            if trip.trip_id.is_empty() { continue; }
            let active_in_7 = derived.calendar_bitmap.active_dates
                .get(ti_opr.service_id(trip))
                .map(|dates| dates.iter().any(|&d| {
                    let djdn = yyyymmdd_to_jdn(d);
                    djdn >= today_jdn && djdn < today_jdn + 7
                }))
                .unwrap_or(false);
            if !active_in_7 {
                let e = inactive_by_service.entry(ti_opr.service_id(trip)).or_insert((0, trip.line));
                e.0 += 1;
            }
        }
        // Deterministik çıktı için service_id'ye göre sırala.
        let mut services: Vec<(&str, (u32, u64))> = inactive_by_service.into_iter().collect();
        services.sort_by(|a, b| a.0.cmp(b.0));
        for (service_id, (count, line)) in services {
            notices.push(k6_notice(
                ctr, "CAL_024", EntityType::Service,
                Some(service_id.to_string()), Some(service_id.to_string()),
                "trips.txt", Some(line), Some("service_id"),
                Some(count.to_string()), None,
                format!("'{service_id}' takvimi önümüzdeki 7 günde aktif değil — {count} sefer etkileniyor."),
                "calendar.txt veya calendar_dates.txt'i güncelleyin ya da yeni bir feed yayınlayın.",
            ));
        }
    }

    // TRP_026: hiç aktif hizmet günü olmayan sefer (UnusedTripNotice)
    if today_yyyymmdd > 0 {
        for trip in &records.trips {
            if trip.trip_id.is_empty() { continue; }
            let has_any_date = derived.calendar_bitmap.active_dates
                .get(ti_opr.service_id(trip))
                .map(|dates| !dates.is_empty())
                .unwrap_or(false);
            if !has_any_date {
                let svc = ti_opr.service_id(trip);
                notices.push(k6_notice(
                    ctr, "TRP_026", EntityType::Trip,
                    Some(trip.trip_id.to_string()), Some(trip.trip_id.to_string()),
                    "trips.txt", Some(trip.line as u64), Some("service_id"),
                    Some(svc.to_string()), None,
                    format!("service_id '{}' için geçerli hizmet tarihi yok; '{}' seferi hiçbir zaman çalışmayacak.",
                        svc, trip.trip_id),
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
                    Some(format!("{total}")), Some("1 veya 2".to_string()),
                    format!("{total} seferin hiçbirinde wheelchair_accessible bilgisi girilmemiş."),
                    "Tekerlekli sandalye erişilebilirliğini wheelchair_accessible alanıyla bildirin (1=erişilebilir, 2=erişilemez).",
                ));
            } else if unset > 0 {
                notices.push(k6_notice(
                    ctr, "TRP_028", EntityType::Feed,
                    None, None,
                    "trips.txt", None, Some("wheelchair_accessible"),
                    Some(format!("{unset}/{total}")), Some("0".to_string()),
                    format!("{total} seferin {unset} tanesinde wheelchair_accessible bilgisi eksik ({:.0}%).",
                        unset as f64 / total as f64 * 100.0),
                    "Tüm seferlerin wheelchair_accessible alanını doldurun (1=erişilebilir, 2=erişilemez).",
                ));
            }
        }
    }

    // STP_037/038: wheelchair_boarding eksikliği (Cal-ITP ACC-1). TRP_028/029'un durak-seviyesi
    // analogu. Sadece fiziksel duraklar/peronlar sayılır (location_type 0 veya boş); istasyon/giriş
    // (1/2/3/4) erişilebilirlik semantiği farklı olduğundan paydaya katılmaz (yanlış-pozitif önlemi).
    {
        let total = records.stops.iter()
            .filter(|s| !s.stop_id.is_empty() && matches!(s.location_type, None | Some(0)))
            .count();
        if total > 0 {
            let unset = records.stops.iter()
                .filter(|s| !s.stop_id.is_empty() && matches!(s.location_type, None | Some(0))
                    && matches!(s.wheelchair_boarding, None | Some(0)))
                .count();
            if unset == total {
                notices.push(k6_notice(
                    ctr, "STP_038", EntityType::Feed,
                    None, None,
                    "stops.txt", None, Some("wheelchair_boarding"),
                    Some(format!("{total}")), Some("1 veya 2".to_string()),
                    format!("{total} fiziksel durağın hiçbirinde wheelchair_boarding bilgisi girilmemiş."),
                    "Durakların tekerlekli sandalye erişilebilirliğini wheelchair_boarding ile bildirin (1=erişilebilir, 2=erişilemez).",
                ));
            } else if unset > 0 {
                notices.push(k6_notice(
                    ctr, "STP_037", EntityType::Feed,
                    None, None,
                    "stops.txt", None, Some("wheelchair_boarding"),
                    Some(format!("{unset}/{total}")), Some("0".to_string()),
                    format!("{total} fiziksel durağın {unset} tanesinde wheelchair_boarding bilgisi eksik ({:.0}%).",
                        unset as f64 / total as f64 * 100.0),
                    "Eksik duraklarda wheelchair_boarding alanını 1 (erişilebilir) veya 2 (erişilemez) olarak doldurun.",
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
            let Some(bid) = ti_opr.block_id(t) else { continue };
            let Some(&rtype) = route_type_map.get(ti_opr.route_id(t)) else { continue };
            let entry = block_route_types.entry(bid).or_insert((rtype, t.trip_id.as_str()));
            if entry.0 != rtype {
                notices.push(k6_notice(
                    ctr, "TRP_024", EntityType::Trip,
                    Some(t.trip_id.to_string()), Some(t.trip_id.to_string()),
                    "trips.txt", Some(t.line as u64), Some("block_id"),
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
            let first_dep = stimes.iter().find_map(|s| s.departure_time()).map(hms_to_secs);
            let last_arr  = stimes.iter().rev().find_map(|s| s.arrival_time()).map(hms_to_secs);
            if let (Some(dep), Some(arr)) = (first_dep, last_arr) {
                trip_range.insert(trip_id, (dep, arr));
            }
        }

        let mut block_trips: HashMap<&str, Vec<(&str, u32, u32, &str)>> = HashMap::new();
        for t in &records.trips {
            let Some(bid) = ti_opr.block_id(t) else { continue };
            let Some(&(dep, arr)) = trip_range.get(t.trip_id.as_str()) else { continue };
            block_trips.entry(bid).or_default().push((t.trip_id.as_str(), dep, arr, ti_opr.service_id(t)));
        }

        // Takvim kesişim önbelleği (#29): aynı block'ta FARKLI service_id'li iki sefer ancak AYNI
        // gün aktifse gerçekten çakışır — bir araç farklı günlerde yalnız birini yapar. Bu guard
        // olmadan TriMet gibi varyant-servisli büyük block'lar 201k+ FP üretiyordu (MD: 908, biz 201476).
        let mut svc_overlap_cache: HashMap<(&str, &str), bool> = HashMap::new();

        for (block_id, trips) in &block_trips {
            for i in 0..trips.len() {
                for j in (i + 1)..trips.len() {
                    let (tid_a, dep_a, arr_a, svc_a) = trips[i];
                    let (tid_b, dep_b, arr_b, svc_b) = trips[j];
                    if dep_a < arr_b && dep_b < arr_a {
                        // Aynı service → kesinlikle aynı gün. Farklı service → takvimleri kesişmeli.
                        if svc_a != svc_b {
                            let key = if svc_a < svc_b { (svc_a, svc_b) } else { (svc_b, svc_a) };
                            let same_day = *svc_overlap_cache.entry(key).or_insert_with(|| {
                                match (
                                    derived.calendar_bitmap.active_dates.get(svc_a),
                                    derived.calendar_bitmap.active_dates.get(svc_b),
                                ) {
                                    (Some(a), Some(b)) => a.intersection(b).next().is_some(),
                                    _ => false,
                                }
                            });
                            if !same_day { continue; }
                        }
                        notices.push(k6_notice(
                            ctr, "TRP_022", EntityType::Trip,
                            Some(tid_a.to_string()), Some(tid_a.to_string()),
                            "trips.txt", None, Some("block_id"),
                            Some(format!("{tid_a} / {tid_b}")),
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
                    Some(format!("{:.0}%", ratio * 100.0)),
                    Some("≤ 80%".to_string()),
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
        // trip_id zenginleştirme: ilk kalkış saati eki (" 08:15 kalkışlı"); bilinmiyorsa boş.
        let dep_suffix = stimes.first().and_then(|s| s.departure_time())
            .map(|(h, m, _)| format!(" {h:02}:{m:02} kalkışlı")).unwrap_or_default();
        if stimes.first().and_then(|s| s.departure_time()).is_none() {
            notices.push(k6_notice(
                ctr, "STM_015", EntityType::Trip,
                Some(trip_id.to_string()), Some(trip_id.to_string()),
                "stop_times.txt", stimes.first().map(|s| s.line as u64),
                Some("departure_time"), None, Some("HH:MM:SS".to_string()),
                format!("trip_id '{trip_id}' seferinin ilk durağında departure_time eksik."),
                "İlk stop_times satırına departure_time girin.",
            ));
        }
        if stimes.last().and_then(|s| s.arrival_time()).is_none() {
            notices.push(k6_notice(
                ctr, "STM_016", EntityType::Trip,
                Some(trip_id.to_string()), Some(trip_id.to_string()),
                "stop_times.txt", stimes.last().map(|s| s.line as u64),
                Some("arrival_time"), None, Some("HH:MM:SS".to_string()),
                format!("trip_id '{trip_id}'{dep_suffix} seferinin son durağında arrival_time eksik."),
                "Son stop_times satırına arrival_time girin.",
            ));
        }
    }

    // STM_027: shape_dist_traveled monoton artmıyor
    for (&trip_id, stimes) in &idx.by_trip {
        let dep_suffix = stimes.first().and_then(|s| s.departure_time())
            .map(|(h, m, _)| format!(" {h:02}:{m:02} kalkışlı")).unwrap_or_default();

        // STM_027
        let mut prev_dist: Option<f64> = None;
        for st in stimes.iter() {
            let Some(dist) = st.shape_dist_traveled() else { continue };
            if let Some(prev) = prev_dist {
                if dist < prev - 1e-6 {
                    notices.push(k6_notice(
                        ctr,
                        "STM_027",
                        EntityType::Trip,
                        Some(trip_id.to_string()),
                        Some(trip_id.to_string()),
                        "stop_times.txt",
                        Some(st.line as u64),
                        Some("shape_dist_traveled"),
                        Some(format!("{dist:.4}")),
                        Some(format!("≥ {prev:.4} (önceki değer)")),
                        format!("'{trip_id}'{dep_suffix} seferinde güzergah mesafesi (shape_dist_traveled) azalıyor: {prev:.4} → {dist:.4}."),
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
            .map(|s| s.arrival_time().is_some() || s.departure_time().is_some())
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
                    stimes.first().map(|s| s.line as u64),
                    Some("arrival_time|departure_time"),
                    None,
                    None,
                    format!("'{trip_id}'{dep_suffix} seferinde bazı ara duraklarda zaman bilgisi eksik — tutarsız zaman dizisi."),
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
    let ti_rtq = &records.trip_interns;

    // route_id → trip listesi
    let _t0 = Timer::start("K6::rtq::build_trip_maps");
    let mut route_trips: HashMap<&str, Vec<&crate::k2::trips::TripRecord>> = HashMap::new();
    for t in &records.trips {
        if !ti_rtq.route_id(t).is_empty() {
            route_trips.entry(ti_rtq.route_id(t)).or_default().push(t);
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
            sts.first().and_then(|s| s.departure_time()).map(|d| {
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
                Some(trip.trip_id.to_string()),
                Some(trip.trip_id.to_string()),
                "stop_times.txt",
                None,
                None,
                None,
                None,
                {
                    let rid = ti_rtq.route_id(trip);
                    let rname = route_short.get(rid).copied().unwrap_or(rid);
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
            let mut n013 = k6_notice(
                ctr,
                "TRP_013",
                EntityType::Route,
                Some(route_id.to_string()),
                Some(route_id.to_string()),
                "trips.txt",
                trips.first().map(|t| t.line),
                None,
                Some("1".to_string()),
                None,
                {
                    let rname = route_short.get(*route_id).copied().unwrap_or(route_id);
                    format!("'{rname}' hattında yalnızca 1 sefer var — düşük frekans sinyali.")
                },
                "Bu rotaya ek seferler ekleyin ya da frekans bazlı tanım kullanın.",
            );
            // Tek seferin çalışma takvimi (varsa) — R2 "Çalışma Takvimi" sütununu doldurur.
            if let Some(svc) = trips.first().map(|t| ti_rtq.service_id(t)).filter(|s| !s.is_empty()) {
                n013.service_id = Some(svc.to_string());
            }
            notices.push(n013);
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
                .get(ti_rtq.service_id(t))
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
                None,
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

    // TRP_011: yolcu hiçbir tanımlayıcı bilgi alamıyor — trip_headsign, trip_short_name VE
    // route adı (route_short/long_name) yoksa. Route adlıysa yolcu hattı route'tan tanır →
    // trip_headsign opsiyoneldir, FP üretme (#29: TriMet'te tüm seferler headsign'sız ama tüm
    // route'lar adlıydı → 52298 saf FP). route_id → en az bir route adı var mı?
    let route_named: HashMap<&str, bool> = records.routes.iter()
        .map(|r| (r.route_id.as_str(),
            r.route_short_name.as_deref().map_or(false, |s| !s.is_empty())
                || r.route_long_name.as_deref().map_or(false, |s| !s.is_empty())))
        .collect();
    let _t4 = Timer::start("K6::rtq::trp_011");
    for trip in &records.trips {
        let headsign_missing = ti_rtq.headsign(trip).map(str::is_empty).unwrap_or(true);
        let short_missing = ti_rtq.short_name(trip).map(str::is_empty).unwrap_or(true);
        if headsign_missing && short_missing {
            // Route adlıysa yolcu hattı route'tan tanır → trip_headsign opsiyonel, atla.
            if route_named.get(ti_rtq.route_id(trip)).copied().unwrap_or(false) {
                continue;
            }
            notices.push(k6_notice(
                ctr,
                "TRP_011",
                EntityType::Trip,
                Some(trip.trip_id.to_string()),
                Some(trip.trip_id.to_string()),
                "trips.txt",
                Some(trip.line as u64),
                Some("trip_headsign"),
                None,
                None,
                {
                    let rid = ti_rtq.route_id(trip);
                    let rname = route_short.get(rid).copied().unwrap_or(rid);
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

        // stop_id → parent_station (boş olmayan) — loop/circular tespiti parent-aware olsun.
        // Bir durağın "etkin istasyonu": parent_station varsa o, yoksa stop_id.
        let parent_of: HashMap<&str, &str> = records.stops.iter()
            .filter_map(|s| s.row.get("parent_station")
                .map(|p| p.trim())
                .filter(|p| !p.is_empty())
                .map(|p| (s.stop_id.as_str(), p)))
            .collect();
        // Etkin istasyon: parent_station varsa o, yoksa stop_id.
        fn eff_station<'a>(parent_of: &HashMap<&'a str, &'a str>, sid: &'a str) -> &'a str {
            parent_of.get(sid).copied().unwrap_or(sid)
        }

        // #15 (TRP_020 perf): scratch'ler trip döngüsü DIŞINDA — per-trip Vec/HashMap tahsisini
        // önler (her ikisi de records'tan &str ödünç alır, her kullanımdan önce clear). station_counts
        // yalnız bir ad-eşleşmesi bulununca TEMBEL kurulur; eşleşmeyen seferler (çoğunluk) hiç kurmaz.
        let mut seq_eff: Vec<(u32, &str)> = Vec::new();
        let mut station_counts: HashMap<&str, u32> = HashMap::new();

        for trip in &records.trips {
            let headsign = match ti_rtq.headsign(trip).map(str::trim).filter(|h| !h.is_empty()) {
                Some(h) => h,
                None => continue,
            };

            let stimes = match idx.by_trip.get(trip.trip_id.as_str()) {
                Some(v) if v.len() >= 2 => v,
                _ => continue,
            };

            // Terminal durak: en büyük stop_sequence
            let terminal_stop_id = stimes.iter()
                .max_by_key(|s| s.stop_sequence().unwrap_or(0))
                .map(|s| idx.stop_id_of(s))
                .unwrap_or("");

            // İlk durak: en küçük stop_sequence
            let first_stop_id = stimes.iter()
                .min_by_key(|s| s.stop_sequence().unwrap_or(u32::MAX))
                .map(|s| idx.stop_id_of(s))
                .unwrap_or("");

            // Tam döngüsel sefer → ilk ve son durak aynı fiziksel istasyon (parent_station-aware;
            // farklı peron stop_id'leriyle aynı istasyona dönen loop'lar dahil). Tüm sefer loop, atla.
            if eff_station(&parent_of, first_stop_id) == eff_station(&parent_of, terminal_stop_id) { continue; }

            // #15: headsign_lc'yi erken-çıkışlardan (stimes/cyclic) SONRA hesapla — elenecek
            // seferler to_lowercase tahsisi ödemez.
            let headsign_lc = headsign.to_lowercase();

            // Headsign SON DURAĞIN ADIYLA eşleşiyorsa yön adı DOĞRUDUR → tüm sefer atlanır.
            // Kuralın tanımı "headsign son durak DEĞİL, bir ara durakla eşleşiyor"; son durakla
            // eşleşiyorsa yanıltıcılık yoktur.
            //
            // Aşağıdaki döngü terminali `stop_id` ile eler ama eşleşmeyi İSİM ile yapar. Aynı
            // ada sahip FARKLI stop_id'ler (ayrı peron/yön kayıtları, ortak parent_station YOK)
            // bu filtreden geçip ad-eşleşmesi üretiyordu → yanlış pozitif. station_counts
            // bastırması da kurtarmıyor: iki kayıt ayrı "etkin istasyon" sayıldığı için sayaç 1.
            //
            // Corpus kanıtı (mdb-1294, Roma): headsign 'CIMITERO LAURENTINO' hem 34. hem SON (38.)
            // durakta geçiyor; son durak eşleştiği için headsign doğru. Biz 9.162 sefer
            // işaretliyorduk, MD yalnız 19 (gerçekten ara durakta bitenler).
            if stop_name_lc.get(terminal_stop_id) == Some(&headsign_lc) { continue; }

            // station_counts (sefer içinde her etkin istasyonun geçiş sayısı) yalnız ilk ad-eşleşmesinde
            // TEMBEL kurulur (counts_built). Eşleşen durak birden fazla geçiyorsa meşru uç/dönüş noktası
            // (ör. havalimanı wye) → o eşleşme bastırılır. Alakasız durak tekrarı başka eşleşmeyi bastırmaz.
            // #29: ARDIŞIK yinelenen aynı durak satırları (dwell/timepoint çiftlemesi) TEK sayılır;
            // gerçek uç/dönüş aralarında BAŞKA durakla tekrar eder. seq_eff stop_sequence'a göre
            // (unwrap_or(0)) sıralanır — by_trip slice'ı unwrap_or(u32::MAX) ile sıralı olduğundan
            // eksik-sequence'li feed'de FARKLI olabilir; bu yüzden sıralama KORUNUR (kaldırılmaz).
            let mut counts_built = false;

            // Her matching intermediate stop için tek notice (ilk eşleşmede dur).
            for st in stimes.iter().filter(|s| idx.stop_id_of(s) != terminal_stop_id) {
                let Some(stop_name) = stop_name_lc.get(idx.stop_id_of(st)) else { continue };
                if *stop_name != headsign_lc { continue; }

                if !counts_built {
                    seq_eff.clear();
                    seq_eff.extend(stimes.iter()
                        .map(|s| (s.stop_sequence().unwrap_or(0), eff_station(&parent_of, idx.stop_id_of(s)))));
                    seq_eff.sort_by_key(|(seq, _)| *seq);
                    station_counts.clear();
                    let mut prev_eff: Option<&str> = None;
                    for (_, e) in &seq_eff {
                        if prev_eff != Some(*e) {
                            *station_counts.entry(*e).or_insert(0) += 1;
                            prev_eff = Some(*e);
                        }
                    }
                    counts_built = true;
                }

                // Eşleşen durak bir uç/dönüş noktası (sefer içinde tekrar ediyor) → atla.
                if station_counts.get(eff_station(&parent_of, idx.stop_id_of(st))).copied().unwrap_or(0) > 1 {
                    continue;
                }

                // #15: rname/dep (format!) yalnız emit anında — eşleşmeyen seferler için değil.
                let rname = route_short.get(ti_rtq.route_id(trip)).copied().unwrap_or(ti_rtq.route_id(trip));
                let dep = trip_first_dep.get(trip.trip_id.as_str()).map(|s| format!(" {} kalkışlı", s)).unwrap_or_default();
                notices.push(k6_notice(
                    ctr,
                    "TRP_020",
                    EntityType::Trip,
                    Some(trip.trip_id.to_string()),
                    Some(trip.trip_id.to_string()),
                    "trips.txt",
                    Some(trip.line as u64),
                    Some("trip_headsign"),
                    Some(headsign.to_string()),
                    None,
                    format!(
                        "'{}' hattının{dep} seferinde yön adı '{}' terminal durak değil, ara durak adıyla eşleşiyor.",
                        rname, headsign
                    ),
                    "Headsign bilinçli bir ana hedefi gösteriyorsa işlem gerekmez; değilse yön adını doğrulayın.",
                ));
                // #15: TRP_020 Entity-scope (trip_id) → dedup trip başına TEKE indirir.
                // Sefer içindeki HER eşleşen ara durağa emit etmek (büyük feed'de) yüz
                // binlerce ara-notice üretip atıyordu; ilk eşleşmede dur (sonuç aynı).
                break;
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
                        "routes.txt", Some(route.line as u64), Some("route_url"),
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
                    "routes.txt", Some(route.line as u64), Some("route_long_name"),
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
                    "stops.txt", Some(stop.line as u64), Some("stop_url"),
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
                    "stops.txt", Some(stop.line as u64), Some("stop_url"),
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
                let Some(flex) = records.stop_times_index.flex_of(st) else { continue };
                let Some(start) = flex.start_pickup_drop_off_window else { continue };
                let Some(end)   = flex.end_pickup_drop_off_window   else { continue };
                let zone = if let Some(ref z) = flex.location_id        { z.as_str() }
                           else if let Some(ref z) = flex.location_group_id { z.as_str() }
                           else { continue };
                let s = start.0 as u64 * 3600 + start.1 as u64 * 60 + start.2 as u64;
                let e = end.0   as u64 * 3600 + end.1   as u64 * 60 + end.2   as u64;
                zone_wins.entry((trip_id.as_str(), zone)).or_default().push((s, e, st.line as u64));
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
    config: &ValidatorConfig,
    today_yyyymmdd: u32,
    notices: &mut Vec<Notice>,
    ctr: &mut u32,
) {
    if let Some(source_url) = config.source_url.as_deref() {
        let path = source_url.split(['?', '#']).next().unwrap_or(source_url);
        if !path.to_ascii_lowercase().ends_with(".zip") {
            notices.push(k6_notice(ctr, "ARC_028", EntityType::Feed, None, None,
                "source_url", None, Some("source_url"), Some(source_url.to_string()),
                Some("kalıcı URL .../dosya.zip".to_string()),
                format!("GTFS yayın URL'si bir .zip dosya adıyla bitmiyor: '{source_url}'."),
                "Feed'i kalıcı ve açık bir .zip dosya adı içeren URL'de yayımlayın."));
        }
    }
    if config.stop_name_best_practices {
        let stop_by_id: HashMap<&str, &crate::k2::stops::StopRecord> = records.stops.iter()
            .map(|s| (s.stop_id.as_str(), s)).collect();
        for stop in &records.stops {
            let Some(name) = stop.stop_name.as_deref() else { continue };
            let lower = name.to_lowercase();
            let generic = lower.split(|c: char| !c.is_alphanumeric()).any(|w| w == "stop" || w == "station");
            let accepted = matches!(lower.as_str(), "union station" | "central station");
            if generic && !accepted {
                notices.push(k6_notice(ctr, "STP_040", EntityType::Stop, Some(stop.stop_id.clone()),
                    Some(stop.stop_id.clone()), "stops.txt", Some(stop.line), Some("stop_name"),
                    Some(name.to_string()), None,
                    format!("stop_id '{}' adı gereksiz genel 'stop/station' sözcüğü içeriyor: '{}'.", stop.stop_id, name),
                    "Genel sözcüğü kaldırın; sözcük resmi adın parçasıysa bu opt-in profili kapatın."));
            }
            if let Some(parent_id) = stop.row.get("parent_station").filter(|v| !v.trim().is_empty()) {
                if let Some(parent_name) = stop_by_id.get(parent_id.trim()).and_then(|p| p.stop_name.as_deref()) {
                    if !lower.contains(&parent_name.to_lowercase()) {
                        notices.push(k6_notice(ctr, "STP_041", EntityType::Stop, Some(stop.stop_id.clone()),
                            Some(stop.stop_id.clone()), "stops.txt", Some(stop.line), Some("stop_name"),
                            Some(name.to_string()), Some(format!("{} içermeli", parent_name)),
                            format!("Alt durak '{}' adı üst istasyon '{}' adını içermiyor.", name, parent_name),
                            "Alt durak adında üst istasyon adını kullanın; yerel adlandırma farklıysa profili kapatın."));
                    }
                }
            }
        }
    }
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
            None,
            None,
            "Feed'de bugün itibarıyla aktif seferi olan servis bulunamadı.".to_string(),
            "calendar.txt veya calendar_dates.txt verilerini güncelleyin.",
        ));
    }

    // DQ_006: şekil (shape) olmayan trip oranı çok yüksek (> %80)
    if !records.trips.is_empty() {
        let shapeless = records.trips.iter().filter(|t| t.shape_idx == 0).count();
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
                Some(format!("{:.0}%", ratio * 100.0)),
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
            Some("1".to_string()),
            Some("≥ 2".to_string()),
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
                None,
                None,
                format!("Feed'de {} işletici var ancak hiçbir rotada agency_id atanmamış.", records.agencies.len()),
                "routes.txt'deki agency_id sütununu doldurun.",
            ));
        }
    }

    // RTS_025: routes.txt'te agency_id boş — önerilen alan (best practice). Tek/sıfır agency'de
    // bilgi düzeyi; >1 agency varsa agency_id zorunludur (DQ_012 / cross-ref kapsamı). MD'nin
    // missing_recommended_field karşılığı. Her boş hat için AYRI notice üretilir.
    if records.agencies.len() <= 1 {
        for r in &records.routes {
            if r.agency_id.as_deref().map_or(true, |s| s.trim().is_empty()) {
                let label = r.route_short_name.as_deref()
                    .filter(|s| !s.is_empty())
                    .unwrap_or(r.route_id.as_str());
                notices.push(k6_notice(
                    ctr, "RTS_025", EntityType::Route,
                    Some(r.route_id.to_string()), Some(r.route_id.to_string()),
                    "routes.txt", Some(r.line as u64), Some("agency_id"),
                    None, None,
                    format!("'{label}' hattında agency_id boş — önerilen alan; tek işletici olsa bile doldurulması iyi uygulamadır."),
                    "routes.txt'teki agency_id sütununu işleten ajansın agency_id'siyle doldurun.",
                ));
            }
        }
    }

    // DQ_013: feed'de çok az sefer (< 3)
    {
        let trip_count = records.trips.len();
        if trip_count > 0 && trip_count < 3 {
            notices.push(k6_notice(
                ctr, "DQ_013", EntityType::Feed,
                None, None, "trips.txt", None, None,
                Some(format!("{trip_count}")), Some("≥ 3".to_string()),
                format!("Feed'de yalnızca {trip_count} sefer var — işlevsel transit veri için en az 3 sefer önerilir."),
                "trips.txt'e daha fazla sefer ekleyin.",
            ));
        }
    }

    // feed_has_content: feed gerçek transit içeriği barındırıyor mu (ARC_020 için).
    // DQ_001/002 (feed_publisher_name/url eksik) kaldırıldı: ARC_020 + FIN_001/002
    // + ARC_025 zaten kapsıyor, çift-sayım/üçleme yapıyordu.
    let feed_has_content = !records.agencies.is_empty()
        || !records.routes.is_empty()
        || !records.stops.is_empty()
        || !records.trips.is_empty();

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
                "routes.txt", Some(route.line as u64), Some("route_desc"),
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
                "routes.txt", Some(route.line as u64), Some("route_url"),
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
                Some(format!("{suspicious}")), None,
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
                let today_jdn = yyyymmdd_to_jdn(today_yyyymmdd);
                let end_jdn   = yyyymmdd_to_jdn(end);
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
                        let today_jdn = yyyymmdd_to_jdn(today_yyyymmdd);
                        let end_jdn   = yyyymmdd_to_jdn(end);
                        let days_left = end_jdn.saturating_sub(today_jdn);
                        if days_left <= 7 && days_left > 0 {
                            notices.push(k6_notice(
                                ctr, "FIN_019", EntityType::Feed,
                                None, None, "feed_info.txt", None, Some("feed_end_date"),
                                Some(format!("{ey}-{em:02}-{ed:02}")),
                                Some("> +7".to_string()),
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
            let start_jdn = yyyymmdd_to_jdn(sy * 10000 + sm * 100 + sd);
            let end_jdn   = yyyymmdd_to_jdn(ey * 10000 + em * 100 + ed);
            let span_days = end_jdn.saturating_sub(start_jdn);
            if span_days < 7 {
                notices.push(k6_notice(
                    ctr, "FIN_020", EntityType::Feed,
                    None, None, "feed_info.txt", None, None,
                    Some(format!("{span_days}")), Some("≥7".to_string()),
                    format!("Feed geçerlilik penceresi yalnızca {span_days} gün ({sy}-{sm:02}-{sd:02} → {ey}-{em:02}-{ed:02}) — operasyonel kullanım için çok kısa."),
                    "feed_start_date ve feed_end_date değerlerini gerçek hizmet dönemiyle güncelleyin.",
                ));
            }
        }
    }

    // CAL_020: Feed geçerlilik penceresi > 5 yıl (yaklaşık 1825 gün)
    if let Some(fi) = records.feed_info.first() {
        if let (Some((sy, sm, sd)), Some((ey, em, ed))) = (fi.feed_start_date, fi.feed_end_date) {
            let start_jdn = yyyymmdd_to_jdn(sy * 10000 + sm * 100 + sd);
            let end_jdn   = yyyymmdd_to_jdn(ey * 10000 + em * 100 + ed);
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
        // shapes.txt'in amacı sabit hat geometrisini çizmektir. Feed tümüyle talep-duyarlı
        // (DRT/flex) ise çizilecek güzergâh yoktur → shapes.txt yokluğu bulgu değildir.
        // feed_info.txt beklentisi DRT'den etkilenmez, ayrı değerlendirilir.
        let missing_shapes = records.shapes.is_empty() && !feed_is_demand_responsive_only(records);
        let missing_feed_info = records.feed_info.is_empty();
        if missing_shapes && missing_feed_info {
            notices.push(k6_notice(
                ctr, "ARC_020", EntityType::Feed,
                None, None, "shapes.txt/feed_info.txt", None, None,
                None, None,
                "Feed'de shapes.txt ve feed_info.txt dosyaları yok — her ikisi de önerilir.".to_string(),
                "shapes.txt ile güzergah geometrisi ve feed_info.txt ile yayıncı bilgisi ekleyin.",
            ));
        } else if missing_shapes {
            notices.push(k6_notice(
                ctr, "ARC_020", EntityType::Feed,
                None, None, "shapes.txt", None, None,
                None, None,
                "Feed'de shapes.txt dosyası yok — güzergah geometrisi için önerilir.".to_string(),
                "shapes.txt dosyası oluşturarak güzergah geometrisi ekleyin.",
            ));
        } else if missing_feed_info {
            notices.push(k6_notice(
                ctr, "ARC_020", EntityType::Feed,
                None, None, "feed_info.txt", None, None,
                None, None,
                "Feed'de feed_info.txt dosyası yok — yayıncı ve geçerlilik bilgisi için önerilir.".to_string(),
                "feed_info.txt dosyası oluşturarak yayıncı bilgisini tanımlayın.",
            ));
        }
    }
}

/// Feed tümüyle talep-duyarlı (DRT/flex) mi?
///
/// Koşul: stop_times.txt'te `stop_id` taşıyan HİÇBİR satır yok, buna karşılık en az bir
/// satır `location_id` (alan-tabanlı DRT) veya `location_group_id` (sabit-duraklı DRT)
/// ile hizmet alanı tanımlıyor.
///
/// Kasıtlı olarak SIKI: tek bir sabit duraklı sefer bile varsa feed DRT sayılmaz, çünkü o
/// sefer için çizilecek güzergâh geometrisi vardır ve shapes.txt beklentisi sürer.
/// "En az bir flex satırı" gibi gevşek bir ölçüt, ağırlıklı olarak sabit hatlı bir feed'de
/// shapes.txt eksikliğini sessizce örterdi.
///
/// Maliyet: `stop_id_set` kontrolü O(1); `flex_map` feed'lerin büyük çoğunluğunda boştur.
fn feed_is_demand_responsive_only(records: &EntityRecords) -> bool {
    let idx = &records.stop_times_index;
    if idx.total_rows == 0 || !idx.stop_id_set.is_empty() {
        return false;
    }
    idx.flex_map
        .values()
        .any(|f| f.location_id.is_some() || f.location_group_id.is_some())
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
    let ti_rem = &records.trip_interns;

    // ── shape_id → sıralı (lat, lon) noktaları + bbox (tek pass) ────────────
    let _tb = Timer::start("K6::rem::build_maps");
    // #15 (build_maps OOM/312s): shape_coords'u NOKTA-İNDEKSLERİYLE kur. Eski yol her noktayı
    // (u32,f64,f64)=24B ara tamponda gruplayıp sonra (f64,f64)=16B'ye kopyalıyordu → transient
    // ~40B/nokta. Devasa-shape feed'inde bu, K6 başındaki ~3.6 GB'ın üstüne binince 4 GB tavanı
    // aşıp OOM ediyordu (ve allocator tavanda tıkanınca 312 sn). İndeks (4B) + coords (16B) =
    // ~20B/nokta tepe. Çıktı (seq-sıralı lat,lon) BİREBİR aynı (stable sort, file-order ties).
    let mut shape_pt_idx: FxHashMap<&str, Vec<u32>> = FxHashMap::default();
    for (i, sp) in records.shapes.iter().enumerate() {
        if sp.shape_pt_lat.is_some() && sp.shape_pt_lon.is_some() {
            shape_pt_idx.entry(sp.shape_id.as_str()).or_default().push(i as u32);
        }
    }
    let n_shapes = shape_pt_idx.len();
    let mut shape_coords: FxHashMap<&str, Vec<(f64, f64)>> = FxHashMap::default();
    let mut shape_bbox: FxHashMap<&str, [f64; 4]> = FxHashMap::default();
    shape_coords.reserve(n_shapes);
    shape_bbox.reserve(n_shapes);
    for (sid, mut idxs) in shape_pt_idx {
        idxs.sort_by_key(|&i| records.shapes[i as usize].shape_pt_sequence.unwrap_or(0));
        let pts: Vec<(f64, f64)> = idxs.iter()
            .map(|&i| {
                let sp = &records.shapes[i as usize];
                (sp.shape_pt_lat.unwrap(), sp.shape_pt_lon.unwrap())
            })
            .collect();
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
        .map(|t| (t.trip_id.as_str(), ti_rem.route_id(t)))
        .collect();
    let route_type_rem: FxHashMap<&str, u32> = records.routes.iter()
        .filter_map(|r| r.route_type.map(|rt| (r.route_id.as_str(), rt)))
        .collect();
    let trip_first_dep: FxHashMap<&str, (u32, u32, u32)> = idx.by_trip.iter()
        .filter_map(|(&tid, sts)| sts.first().and_then(|s| s.departure_time()).map(|d| (tid, d)))
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
            if let Some(shape) = ti_rem.shape_id(t).filter(|s| !s.is_empty()) {
                let de = dirs.entry(shape).or_insert(t.direction_id);
                if *de != t.direction_id { *de = None; }
                let rid = ti_rem.route_id(t);
                let re = route_ids.entry(shape).or_insert(Some(rid));
                if *re != Some(rid) { *re = None; }
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
    // shape_id → o shape'i kullanan TÜM hatların etiketleri (sıralı, tekil).
    // SHP_009/014/020/023 mesaj öneki için; route_short_name yoksa route_id'ye düşer.
    let shape_route_labels: FxHashMap<&str, Vec<&str>> = {
        let mut m: FxHashMap<&str, FxHashSet<&str>> = FxHashMap::default();
        for t in &records.trips {
            if ti_rem.route_id(t).is_empty() { continue; }
            if let Some(shape) = ti_rem.shape_id(t).filter(|s| !s.is_empty()) {
                let rid = ti_rem.route_id(t);
                let label = route_short.get(rid).copied().unwrap_or(rid);
                m.entry(shape).or_default().insert(label);
            }
        }
        m.into_iter()
            .map(|(shape, set)| {
                let mut labels: Vec<&str> = set.into_iter().collect();
                labels.sort_unstable();
                (shape, labels)
            })
            .collect()
    };
    // "[12, 14] kodlu hatlara ait " öneki (SHP_009/014/020/023). Boş slice → boş önek (orphan shape).
    let shp_route_prefix = |labels: &[&str]| -> String {
        if labels.is_empty() { return String::new(); }
        let hat = if labels.len() == 1 { "hatta" } else { "hatlara" };
        format!("[{}] kodlu {hat} ait ", labels.join(", "))
    };
    drop(_tb);

    // SHP_011 KALDIRILDI (2026-07-22): GEO_006 ile aynı veriyi (segment_distances_km),
    // aynı eşikle (config.max_shape_jump_km) tarıyordu. GEO_006 dedup_level=Entity +
    // scope=shape_id olduğundan K7 onu zaten shape başına tek notice'a indiriyor —
    // SHP_011'in "shape başına özet" rolü diye bir şey yoktu, iki kural dedup sonrası
    // birebir aynı çıktıyı veriyordu. İki kuralın kartlarındaki "karışan komşular"
    // tabloları da birbirini anmıyordu: örtüşme belgelenmiş bir tasarım değil, kazaydı.

    // ── SHP_023: aynı dist_traveled ve koordinatlara sahip ardışık iki shape noktası ─
    // (equal_shape_distance_same_coordinates)
    {
        let _ts23 = Timer::start("K6::rem::shp_023");
        // #15: shape_raw'ı (seq,dist,lat,lon)=~40B/nokta materialize etmek yerine nokta-indeksleri
        // (4B) grupla, veriye records.shapes'ten eriş → build_maps OOM'undan sonraki ikinci büyük
        // shape rebuild kalkar (asıl unreachable burada oluyordu). Çıktı/sıra BİREBİR aynı.
        let mut shape_pt_idx: FxHashMap<&str, Vec<u32>> = FxHashMap::default();
        for (i, sp) in records.shapes.iter().enumerate() {
            if sp.shape_pt_lat.is_some() && sp.shape_pt_lon.is_some() {
                shape_pt_idx.entry(sp.shape_id.as_str()).or_default().push(i as u32);
            }
        }
        for (_shape_id, idxs) in &mut shape_pt_idx {
            idxs.sort_by_key(|&i| records.shapes[i as usize].shape_pt_sequence.unwrap_or(0));
        }
        for (shape_id, idxs) in &shape_pt_idx {
            // Shape başına HER TÜRDEN (SHP_023/028/029) en fazla BİR notice. Eskiden ilk eşit-dist
            // çiftinde break ediliyordu → aynı shape'te hem tekrar-nokta (023) hem eşik-altı (029)
            // varsa yalnız İLKİ raporlanıyordu (Spelunca: 6 SHP_023 kaybı). Artık tüm çiftler taranır,
            // her tür bir kez emit edilir; üçü de bulununca erken çıkılır.
            let mut fired_023 = false;
            let mut fired_028 = false;
            let mut fired_029 = false;
            for w in idxs.windows(2) {
                if fired_023 && fired_028 && fired_029 { break; }
                let pa = &records.shapes[w[0] as usize];
                let pb = &records.shapes[w[1] as usize];
                let (da, la, loa) = (pa.shape_dist_traveled, pa.shape_pt_lat.unwrap(), pa.shape_pt_lon.unwrap());
                let (db, lb, lob) = (pb.shape_dist_traveled, pb.shape_pt_lat.unwrap(), pb.shape_pt_lon.unwrap());
                if let (Some(da_v), Some(db_v)) = (da, db) {
                    const EPS: f64 = 1e-9;
                    // SHP_028/029 koordinat-fark eşiği (~1.1m, 1e-5°): altı gürültü (WARNING),
                    // üstü gerçek tutarsızlık (ERROR). MD diff_coordinates(_below_threshold) karşılığı.
                    const DIFF_THRESHOLD: f64 = 1e-5;
                    if (da_v - db_v).abs() < EPS {
                        if (la - lb).abs() < EPS && (loa - lob).abs() < EPS {
                            // SHP_023: aynı dist + aynı koordinat (tekrar eden nokta)
                            if !fired_023 {
                                fired_023 = true;
                                let prefix = shp_route_prefix(shape_route_labels.get(*shape_id).map(|v| v.as_slice()).unwrap_or(&[]));
                                notices.push(k6_notice(
                                    ctr, "SHP_023", EntityType::Shape,
                                    Some(shape_id.to_string()), Some(shape_id.to_string()),
                                    "shapes.txt", None, Some("shape_dist_traveled"),
                                    Some(format!("dist={da_v:.4}, ({la:.6},{loa:.6})")), None,
                                    format!("{prefix}'{shape_id}' şeklinde art arda iki noktanın shape_dist_traveled ({da_v:.4}) ve koordinatları aynı — tekrar eden nokta."),
                                    "Yinelenen shape noktasını kaldırın.",
                                ));
                            }
                        } else {
                            // Aynı dist ama farklı koordinat → mesafe artmadan konum değişmiş.
                            let coord_diff = (la - lb).abs().max((loa - lob).abs());
                            if coord_diff >= DIFF_THRESHOLD {
                                if !fired_028 {
                                    fired_028 = true;
                                    notices.push(k6_notice(
                                        ctr, "SHP_028", EntityType::Shape,
                                        Some(shape_id.to_string()), Some(shape_id.to_string()),
                                        "shapes.txt", None, Some("shape_dist_traveled"),
                                        Some(format!("dist={da_v:.4}, ({la:.6},{loa:.6})→({lb:.6},{lob:.6})")),
                                        Some("artan dist".to_string()),
                                        format!("'{shape_id}' şeklinde art arda iki noktanın shape_dist_traveled ({da_v:.4}) aynı ama koordinatları farklı — mesafe artmadan konum değişmiş."),
                                        "Ardışık shape noktalarına artan shape_dist_traveled değerleri atayın.",
                                    ));
                                }
                            } else if !fired_029 {
                                fired_029 = true;
                                notices.push(k6_notice(
                                    ctr, "SHP_029", EntityType::Shape,
                                    Some(shape_id.to_string()), Some(shape_id.to_string()),
                                    "shapes.txt", None, Some("shape_dist_traveled"),
                                    Some(format!("dist={da_v:.4}, Δ≈{coord_diff:.7}°")),
                                    Some("artan dist".to_string()),
                                    format!("'{shape_id}' şeklinde art arda iki noktanın shape_dist_traveled ({da_v:.4}) aynı, koordinat farkı çok küçük (eşik altı)."),
                                    "Ardışık shape noktalarına artan shape_dist_traveled değerleri atayın ya da yinelenen noktayı kaldırın.",
                                ));
                            }
                        }
                    }
                }
            }
        }
    }

    // ── STM_017: shape olan trip'te stop_times'da shape_dist_traveled eksik ──
    // idx.trips_missing_sdt: build'de trip_shape filtreli, line numarası hazır
    {
        let _ts = Timer::start("K6::rem::stm_017");
        // shape_dist_traveled feed genelinde hiç kullanılmamışsa, trip-başına on
        // binlerce özdeş notice yerine tek feed-seviyesi özet üret (STM_050 emsali).
        // Kısmen doluysa (gerçek tutarsızlık) trip granülerliği korunur.
        let any_sdt_present = records.stop_times.iter().any(|s| s.shape_dist_traveled.is_some());

        // rail dışı (intercity rail shape_dist_traveled sağlamaz — beklenen davranış),
        // shape'i olup sdt eksik trip'ler.
        let flagged: Vec<(&str, u64)> = idx
            .trips_missing_sdt
            .iter()
            .filter(|(trip_id, _)| {
                let route_id = trip_to_route_rem.get(**trip_id).copied().unwrap_or(**trip_id);
                let rt = route_type_rem.get(route_id).copied().unwrap_or(3);
                !is_rail_route_type(rt)
            })
            .map(|(&trip_id, &line)| (trip_id, line))
            .collect();

        if !any_sdt_present && flagged.len() > 1 {
            // Feed-geneli tamamen eksik → tek özet notice (gürültü ~54k → 1).
            let n = flagged.len();
            let trip_to_shape: FxHashMap<&str, &str> = records
                .trips
                .iter()
                .filter_map(|t| ti_rem.shape_id(t).map(|s| (t.trip_id.as_str(), s)))
                .collect();
            let n_shapes = flagged
                .iter()
                .filter_map(|(tid, _)| trip_to_shape.get(*tid).copied())
                .collect::<FxHashSet<&str>>()
                .len();
            notices.push(k6_notice(
                ctr,
                "STM_017",
                EntityType::Feed,
                None,
                None,
                "stop_times.txt",
                None,
                Some("shape_dist_traveled"),
                Some(n.to_string()),
                None,
                format!("Feed genelinde shape_dist_traveled hiç kullanılmamış: {n_shapes} güzergaha ait {n} sefer etkileniyor — durak konumları güzergah üzerinde doğrulanamıyor."),
                "Tüm stop_times satırlarına shape_dist_traveled ekleyin ya da güzergah bazlı mesafe verisi sağlayın.",
            ));
        } else {
            for (trip_id, line) in flagged {
                let route = trip_to_route_rem.get(trip_id).copied().unwrap_or(trip_id);
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
                    None,
                    None,
                    format!("'{}' hattının{dep_infix} seferinde güzergah mesafe bilgisi (shape_dist_traveled) eksik — durak konumları doğrulanamıyor.", route),
                    "Tüm stop_times satırlarına shape_dist_traveled ekleyin ya da tümünden kaldırın.",
                ));
            }
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
            if let Some(sid) = ti_rem.shape_id(t).filter(|s| !s.is_empty()) {
                m.entry(sid).or_default().insert(ti_rem.route_id(t));
            }
        }
        m
    };
    // shape_id → [trip_id] (örnek kalkış saati için)
    let shape_trips: FxHashMap<&str, Vec<&str>> = {
        let mut m: FxHashMap<&str, Vec<&str>> = FxHashMap::default();
        for t in &records.trips {
            if let Some(sid) = ti_rem.shape_id(t).filter(|s| !s.is_empty()) {
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
                                .find(|st| idx.stop_id_of(st) == stop_id)
                                .and_then(|st| st.departure_time())
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
            let shape_id = match ti_rem.shape_id(trip).filter(|s| !s.is_empty()) {
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
                let sdt = match st.shape_dist_traveled() {
                    Some(d) => d,
                    None => continue,
                };
                let stop_id = idx.stop_id_of(st);
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
                        "stop_times.txt", Some(st.line as u64), Some("shape_dist_traveled"),
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
            let shape_id = match ti_rem.shape_id(trip).filter(|s| !s.is_empty()) {
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
                .filter_map(|st| st.shape_dist_traveled())
                .fold(f64::NEG_INFINITY, f64::max);

            if trip_max_sdt.is_finite() && trip_max_sdt > shape_max * 1.001 {
                let route = trip_to_route_rem.get(trip.trip_id.as_str()).copied().unwrap_or(trip.trip_id.as_str());
                notices.push(k6_notice(
                    ctr, "SHP_025", EntityType::Trip,
                    Some(trip.trip_id.to_string()), Some(trip.trip_id.to_string()),
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
                            Some(anchor.line as u64),
                            Some("stop_lat|stop_lon"),
                            Some(format!("{}", nearby + 1)),
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
                Some(format!("{n_stops_with_coords}")),
                None,
                format!("Feed {n_stops_with_coords} koordinatlı durak içeriyor."),
                "Bu bilgi notu; aksiyona gerek yoktur.",
            ));
        }
    }

    // ── OPR_005: Sıradışı sefer sıklığı (route_type bazlı göreli aykırı) ──────
    // "Sık/seyrek" mutlak değil görelidir; mutlak eşik yerine bir (route,yön,takvim)
    // grubunun ortalama headway'i, AYNI route_type'taki grupların medyanından
    // robust-σ (MAD) kadar saparsa işaretlenir. Tek route_type baz alınır (metro ile
    // kırsal otobüsü kıyaslamamak için). Tekdüze sıklıkta feed sıfır notice üretir.
    {
        let _topr5 = Timer::start("K6::rem::opr_005");
        let route_short_opr5: HashMap<&str, &str> = records.routes.iter()
            .map(|r| {
                let label = r.route_short_name.as_deref().filter(|s| !s.is_empty()).unwrap_or(r.route_id.as_str());
                (r.route_id.as_str(), label)
            })
            .collect();
        let route_type_opr5: HashMap<&str, u32> = records.routes.iter()
            .map(|r| (r.route_id.as_str(), r.route_type.unwrap_or(3)))
            .collect();

        // (route_id, direction_key, service_id) → ilk kalkış saatleri
        let mut route_headways: HashMap<(&str, &str, &str), Vec<u32>> = HashMap::new();
        for trip in &records.trips {
            let route_id = ti_rem.route_id(trip);
            if route_id.is_empty() { continue; }
            let dir_key: &str = match trip.direction_id { Some(0) => "0", Some(1) => "1", _ => "-" };
            let svc_key = ti_rem.service_id(trip);
            if let Some(&dep) = idx.trip_first_dep.get(trip.trip_id.as_str()) {
                route_headways.entry((route_id, dir_key, svc_key)).or_default().push(dep);
            }
        }

        // 1) Grup başına ortalama headway.
        struct Hw<'a> { route: &'a str, dir: &'a str, svc: &'a str, hw: u32, rtype: u32 }
        let mut grps: Vec<Hw> = Vec::new();
        for ((route_id, dir_key, svc_key), mut deps) in route_headways {
            if deps.len() < 2 { continue; }
            deps.sort_unstable();
            deps.dedup();
            if deps.len() < 2 { continue; }
            let diffs: Vec<u32> = deps.windows(2).map(|w| w[1] - w[0]).collect();
            let avg_hw = diffs.iter().sum::<u32>() / diffs.len() as u32;
            let rtype = route_type_opr5.get(route_id).copied().unwrap_or(3);
            grps.push(Hw { route: route_id, dir: dir_key, svc: svc_key, hw: avg_hw, rtype });
        }

        // 2) route_type başına medyan + MAD (baz için ≥5 grup gerekir).
        let mut by_type: HashMap<u32, Vec<u32>> = HashMap::new();
        for g in &grps { by_type.entry(g.rtype).or_default().push(g.hw); }
        let mut type_stats: HashMap<u32, (f64, f64)> = HashMap::new();
        for (rt, mut hws) in by_type {
            if hws.len() < 5 { continue; }
            hws.sort_unstable();
            let median = hws[hws.len() / 2] as f64;
            let mut devs: Vec<f64> = hws.iter().map(|&h| (h as f64 - median).abs()).collect();
            devs.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
            let mad = devs[devs.len() / 2] * 1.4826;
            type_stats.insert(rt, (median, mad));
        }

        // 3) Yalnızca aynı route_type medyanına göre aykırı grupları emit et.
        // Determinizm için sırala.
        grps.sort_by(|a, b| (a.route, a.dir, a.svc).cmp(&(b.route, b.dir, b.svc)));
        for g in &grps {
            let (median, mad) = match type_stats.get(&g.rtype) { Some(&s) => s, None => continue };
            let hw = g.hw as f64;
            let use_ratio = mad < 30.0; // ~tüm hatlar neredeyse aynı sıklıkta → orana düş
            let is_outlier = if use_ratio {
                let r = hw / median;
                r > 2.0 || r < 0.5
            } else {
                (hw - median).abs() > config.headway_outlier_sigma * mad
            };
            if !is_outlier { continue; }
            let yon_str = if hw > median { "sıradışı seyrek" } else { "sıradışı sık" };
            let route_label = route_short_opr5.get(g.route).copied().unwrap_or(g.route);
            let hw_min = hw / 60.0;
            let med_min = median / 60.0;
            let mut n = k6_notice(
                ctr,
                "OPR_005",
                EntityType::Route,
                Some(g.route.to_string()),
                Some(g.route.to_string()),
                "stop_times.txt",
                None,
                None,
                Some(format!("{hw_min:.0}")),
                None,
                format!("'{route_label}' hattının {} yönünde {} takviminde ortalama sefer aralığı {hw_min:.0}dk — aynı tür hatların feed medyanı {med_min:.0}dk ({yon_str}).",
                    g.dir, g.svc),
                "Bu bilgi notu; sefer sıklığı aynı tür hatlara göre sıradışı.",
            );
            n.service_id = Some(g.svc.to_string());
            notices.push(n);
        }
    }

    // ── OPR_013: tek yönlü rota bilgisi (tüm seferler aynı direction_id) ──────
    {
        let _topr13 = Timer::start("K6::rem::opr_013");
        // route_id → gösterim etiketi (route_short_name varsa o, yoksa route_id)
        let route_short_opr13: HashMap<&str, &str> = records.routes.iter()
            .map(|r| {
                let label = r.route_short_name.as_deref().filter(|s| !s.is_empty()).unwrap_or(r.route_id.as_str());
                (r.route_id.as_str(), label)
            })
            .collect();
        let mut route_directions: HashMap<&str, HashSet<u32>> = HashMap::new();
        for t in &records.trips {
            if ti_rem.route_id(t).is_empty() {
                continue;
            }
            if let Some(dir) = t.direction_id {
                route_directions
                    .entry(ti_rem.route_id(t))
                    .or_default()
                    .insert(dir);
            }
        }
        for (route_id, dirs) in &route_directions {
            if dirs.len() == 1 {
                let dir = dirs.iter().next().copied().unwrap_or(0);
                let label = route_short_opr13.get(route_id).copied().unwrap_or(route_id);
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
                    format!("'{label}' kodlu hatta yalnızca {} yönlü seferler var — karşı yön tanımlı değil.",
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
                    None,
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
                    Some(format!("{:.0}%", ratio * 100.0)),
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
                    Some(ag.line as u64),
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

    // ── RTS_017: shape tanımlı olmayan hat — her hat için ayrı bilgi notu ─────
    // (route_short_name etiketiyle; RTS_025 deseni. Eski feed-özeti tek route_id
    // gösteriyordu; artık hangi hatların shape'siz olduğu tek tek görülür.)
    {
        let mut route_has_shape: HashMap<&str, bool> = HashMap::new();
        for t in &records.trips {
            let entry = route_has_shape.entry(ti_rem.route_id(t)).or_insert(false);
            if t.shape_idx != 0 {
                *entry = true;
            }
        }
        // Yalnızca seferi olup hiç shape'i olmayan hatlar (sefersiz hatları başka kural ele alır).
        for r in &records.routes {
            if route_has_shape.get(r.route_id.as_str()) == Some(&false) {
                let label = r.route_short_name.as_deref()
                    .filter(|s| !s.is_empty())
                    .unwrap_or(r.route_id.as_str());
                notices.push(k6_notice(
                    ctr,
                    "RTS_017",
                    EntityType::Route,
                    Some(r.route_id.to_string()),
                    Some(r.route_id.to_string()),
                    "routes.txt",
                    Some(r.line as u64),
                    Some("route_id"),
                    None,
                    None,
                    format!("'{label}' hattının hiçbir seferinde shape_id tanımlı değil — harita gösterimi kısıtlı."),
                    "Bu hat için shapes.txt tanımlayın ve trips.txt'de shape_id atayın.",
                ));
            }
        }
    }

    // ── TRP_012: trip yönü route'taki diğer seferlerle tutarsız ──────────────
    // (direction_id set ama bazı seferlerle çelişen)
    {
        let mut route_dir_trips: HashMap<(&str, u32), u32> = HashMap::new();
        for t in &records.trips {
            if ti_rem.route_id(t).is_empty() { continue; }
            if let Some(dir) = t.direction_id {
                *route_dir_trips.entry((ti_rem.route_id(t), dir)).or_default() += 1;
            }
        }
        // Aynı rotada her iki yönde de sefer var; yalnızca direction_id set olmayan seferler → TRP_012
        let routes_with_both_dirs: HashSet<&str> = {
            let mut r: HashMap<&str, HashSet<u32>> = HashMap::new();
            for t in &records.trips {
                if let Some(d) = t.direction_id {
                    r.entry(ti_rem.route_id(t)).or_default().insert(d);
                }
            }
            r.into_iter().filter(|(_, dirs)| dirs.len() > 1).map(|(rid, _)| rid).collect()
        };
        // Rota seviyesinde topla: çift yönlü rotada direction_id'si eksik kaç sefer var.
        // Sefer başına emit etmek aynı rota için yüzlerce özdeş satır üretiyordu (mesaj
        // zaten rota-merkezli); rota başına TEK notice + sayım.
        let mut route_missing: HashMap<&str, (u32, u64)> = HashMap::new(); // route -> (eksik_sayı, ilk_satır)
        for t in &records.trips {
            if !routes_with_both_dirs.contains(ti_rem.route_id(t)) { continue; }
            if t.direction_id.is_none() {
                let e = route_missing.entry(ti_rem.route_id(t)).or_insert((0, t.line));
                e.0 += 1;
                if t.line < e.1 { e.1 = t.line; }
            }
        }
        let mut rows: Vec<(&str, u32, u64)> =
            route_missing.into_iter().map(|(r, (c, l))| (r, c, l)).collect();
        rows.sort_unstable_by(|a, b| a.0.cmp(b.0)); // deterministik sıra
        for (route_id, count, line) in rows {
            notices.push(k6_notice(
                ctr,
                "TRP_012",
                EntityType::Route,
                Some(route_id.to_string()),
                Some(route_id.to_string()),
                "trips.txt",
                Some(line),
                Some("direction_id"),
                Some(count.to_string()),
                None,
                format!("'{}' kodlu hattın {count} seferinde yön bilgisi (direction_id) girilmemiş — hat çift yönlü.", route_short.get(route_id).copied().unwrap_or(route_id)),
                "Bu seferlerin direction_id alanını 0 veya 1 olarak doldurun.",
            ));
        }
    }

    // ── TRP_015: trip block_id tek (block'ta başka sefer yok) ─────────────────
    {
        let mut block_count: HashMap<&str, u32> = HashMap::new();
        for t in &records.trips {
            if let Some(bid) = ti_rem.block_id(t).filter(|b| !b.is_empty()) {
                *block_count.entry(bid).or_default() += 1;
            }
        }
        for t in &records.trips {
            if let Some(bid) = ti_rem.block_id(t).filter(|b| !b.is_empty()) {
                if block_count.get(bid).copied().unwrap_or(0) == 1 {
                    notices.push(k6_notice(
                        ctr,
                        "TRP_015",
                        EntityType::Trip,
                        Some(t.trip_id.to_string()),
                        Some(t.trip_id.to_string()),
                        "trips.txt",
                        Some(t.line as u64),
                        Some("block_id"),
                        Some(bid.to_string()),
                        None,
                        format!("'{}' hattının seferinde '{bid}' blok kodu tek kullanılmış — blok en az 2 sefer içermelidir.", ti_rem.route_id(t)),
                        "block_id'nin aynı araca atanan birden fazla sefer için kullanıldığından emin olun.",
                    ));
                }
            }
        }
    }

    // ── TRP_033: bir blokta birden fazla route_type ───────────────────────────
    // block_id "bu seferler aynı araçla, sırayla yapılır" demektir. Bir araç mod
    // değiştiremeyeceğine göre blok içindeki tüm seferlerin route_type'ı aynı olmalıdır;
    // farklıysa block_id ya yanlış atanmıştır ya da route_type yanlış girilmiştir.
    // Blok BAŞINA tek notice: sefer başına üretmek büyük bloklarda yüzlerce tekrar demekti.
    {
        let route_type_of: HashMap<&str, u32> = records.routes.iter()
            .filter_map(|r| r.route_type.map(|t| (r.route_id.as_str(), t)))
            .collect();
        // block_id → (route_type'lar, ilk sefer, ilk satır)
        let mut blocks: HashMap<&str, (BTreeSet<u32>, &str, u64)> = HashMap::new();
        for t in &records.trips {
            let Some(bid) = ti_rem.block_id(t).filter(|b| !b.is_empty()) else { continue };
            let Some(&rt) = route_type_of.get(ti_rem.route_id(t)) else { continue };
            let entry = blocks.entry(bid).or_insert_with(|| (BTreeSet::new(), t.trip_id.as_str(), t.line as u64));
            entry.0.insert(rt);
        }
        let mut mixed: Vec<(&str, &BTreeSet<u32>, &str, u64)> = blocks.iter()
            .filter(|(_, (types, _, _))| types.len() > 1)
            .map(|(bid, (types, trip, line))| (*bid, types, *trip, *line))
            .collect();
        mixed.sort_by(|a, b| a.0.cmp(b.0)); // HashMap sırası deterministik değil
        for (bid, types, trip_id, line) in mixed {
            let list = types.iter().map(u32::to_string).collect::<Vec<_>>().join(", ");
            notices.push(k6_notice(
                ctr, "TRP_033", EntityType::Trip,
                Some(bid.to_string()), Some(bid.to_string()),
                "trips.txt", Some(line), Some("block_id"),
                Some(list.clone()), Some("tek route_type".to_string()),
                format!("'{bid}' bloğundaki seferler {} farklı route_type taşıyor ({list}) — aynı araç mod değiştiremez (ör. '{trip_id}').", types.len()),
                "Blok zincirini tek moda ayırın ya da yanlış girilmiş route_type değerini düzeltin.",
            ));
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
                    Some(stop.line as u64),
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
        .filter_map(|t| ti_rem.shape_id(t).map(|s| (t.trip_id.as_str(), s)))
        .collect();
    let mut shape_cum: FxHashMap<&str, Vec<f64>> = FxHashMap::default();

    // ── SHP_016: shape tamamen ters yönde çizilmiş (varyant-farkında ön-geçiş) ─
    // İlk durak shape'in ikinci yarısına projekte oluyorsa shape ters takılmış.
    // Varyant körlüğünü önlemek için: shape'i kullanan TÜM trip'lerin ilk-durak
    // projeksiyonunun MİNİMUMU shape ortasını geçiyorsa (hiçbir varyant baştan
    // başlamıyorsa) ateşle. Tek bir ileri/short-turn varyant ya da çift-yön kullanımı
    // (XFL_013'ün konusu) bunu bastırır. Deterministik (min sıradan bağımsız).
    let mut reversed_shapes: FxHashSet<&str> = FxHashSet::default();
    {
        let _t16 = Timer::start("K6::rem::shp_016");
        // shape_id → (min_first_arc, total_km, route, trip_id, first_stop)
        let mut agg: FxHashMap<&str, (f64, f64, String, String, String)> = FxHashMap::default();
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
            let total = cum.last().copied().unwrap_or(0.0);
            if total <= 0.1 { continue; }
            // Dairesel shape (başlangıç≈bitiş) → ters-kontrolü anlamsız, atla
            let ring = haversine_km(pts[0].0, pts[0].1, pts.last().unwrap().0, pts.last().unwrap().1);
            if ring < total * 0.10 && ring < 1.0 { continue; }
            // by_trip dilimleri stop_sequence sıralı → ilk eleman = ilk durak
            let Some(first_st) = stimes.first() else { continue };
            let Some(&(slat, slon)) = stop_coords.get(idx.stop_id_of(first_st)) else { continue };
            let first_arc = project_arc_km(pts, cum, slat, slon);
            let route = trip_to_route_rem.get(trip_id).copied().unwrap_or(trip_id);
            let entry = agg.entry(shape_id).or_insert_with(|| {
                (f64::INFINITY, total, String::new(), String::new(), String::new())
            });
            if first_arc < entry.0 {
                entry.0 = first_arc;
                entry.2 = route.to_string();
                entry.3 = trip_id.to_string();
                entry.4 = idx.stop_id_of(first_st).to_string();
            }
        }
        let mut shape_ids: Vec<&str> = agg.keys().copied().collect();
        shape_ids.sort_unstable();
        for shape_id in shape_ids {
            let (min_arc, total, route, trip_id, first_stop) = &agg[shape_id];
            if !min_arc.is_finite() || *min_arc <= *total * 0.5 { continue; }
            reversed_shapes.insert(shape_id);
            let mut n016 = k6_notice(
                ctr, "SHP_016",
                EntityType::Shape,
                Some(shape_id.to_string()),
                Some(shape_id.to_string()),
                "trips.txt", None,
                Some("shape_id"),
                Some(shape_id.to_string()),
                None,
                format!(
                    "'{route}' hattının '{shape_id}' güzergahı ters yönde çizilmiş — bu shape'i kullanan tüm seferlerin ilk durağı shape'in başından değil sonuna yakın projekte oluyor (en yakını {:.0}m / {:.0}m).",
                    min_arc * 1000.0, total * 1000.0
                ),
                "shape_pt_sequence sırasını tersine çevirin veya bu yön için ayrı bir shape_id tanımlayın.",
            );
            let mut d = std::collections::HashMap::new();
            d.insert("first_stop".to_string(), first_stop.clone());
            d.insert("trip_id".to_string(), trip_id.clone());
            n016.details = Some(d);
            notices.push(n016);
        }
    }

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
            sorted.sort_by_key(|st| st.stop_sequence().unwrap_or(0));

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

            // Ters shape (SHP_016 ön-geçişte tespit edildi): bu shape için SHP_017
            // "sıra bozuk" gürültüsünü bastır (SHP_016 daha net sinyaldir).
            if reversed_shapes.contains(shape_id) { continue; }

            // stop_times'da shape_dist_traveled varsa geometrik projeksiyona gerek yok.
            // Tüm duraklarda mevcutsa yetkili kaynak olarak kullan; karışık durumdaysa
            // birim uyuşmazlığından kaçınmak için geometrik kontrolü atla.
            let all_have_sdt = sorted.iter().all(|st| st.shape_dist_traveled().is_some());
            let any_have_sdt = sorted.iter().any(|st| st.shape_dist_traveled().is_some());
            if any_have_sdt && !all_have_sdt { continue; }

            // shape_dist_traveled metre mi km mi? shape toplam uzunluğuyla karşılaştır.
            let sdt_to_km: f64 = if all_have_sdt {
                let sdt_max = sorted.iter()
                    .filter_map(|st| st.shape_dist_traveled())
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
                    st.shape_dist_traveled().unwrap() * sdt_to_km
                } else {
                    let Some(&(slat, slon)) = stop_coords.get(idx.stop_id_of(st)) else { continue };
                    *arc_cache
                        .entry((shape_id, idx.stop_id_of(st)))
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
                        nxt.shape_dist_traveled().unwrap_or(0.0) * sdt_to_km
                    } else {
                        // Break prob_idx'te gerçekleştiği için sonraki duraklar henüz cache'de
                        // değil; arc'ı doğrudan projekte et. (Cache'e güvenmek 0.0 verir →
                        // geometrik yolda kurtarma asla tetiklenmez = yanlış pozitif.)
                        match stop_coords.get(idx.stop_id_of(nxt)) {
                            Some(&(slat, slon)) => project_arc_km(pts, cum, slat, slon),
                            None => 0.0,
                        }
                    };
                    narc > prev_arc
                });
                if recovers { continue; }

                let st = sorted[pi];
                // Aynı (shape_id, problem_stop_id) çifti için daha önce notice üretildiyse atla
                if !shp017_seen.insert((shape_id, idx.stop_id_of(st))) {
                    continue;
                }
                let route = trip_to_route_rem.get(trip_id).copied().unwrap_or(trip_id);
                let dep = trip_first_dep.get(trip_id)
                    .map(|(h, m, _)| format!("{h:02}:{m:02}"))
                    .unwrap_or_default();
                let dep_infix = if dep.is_empty() { String::new() } else { format!(" {dep}") };
                let sname = stop_names.get(idx.stop_id_of(st)).copied().unwrap_or(idx.stop_id_of(st));
                let arc = if all_have_sdt {
                    st.shape_dist_traveled().unwrap() * sdt_to_km
                } else {
                    arc_cache.get(&(shape_id, idx.stop_id_of(st))).copied().unwrap_or(0.0)
                };
                let mut notice = k6_notice(
                    ctr,
                    "SHP_017",
                    EntityType::Trip,
                    Some(trip_id.to_string()),
                    Some(trip_id.to_string()),
                    "stop_times.txt",
                    Some(st.line as u64),
                    Some("stop_sequence"),
                    Some(st.stop_sequence().unwrap_or(0).to_string()),
                    None,
                    format!(
                        "'{}' hattının{dep_infix} seferinde '{}' (kod: '{}') {}. sıradaki durak güzergah sırası bozuk — başlangıca uzaklığı önceki durağın gerisinde ({:.0}m < {:.0}m).",
                        route, sname, idx.stop_id_of(st), st.stop_sequence().unwrap_or(0),
                        arc * 1000.0, prev_arc * 1000.0,
                    ),
                    "Projeksiyon döngü veya aynı yolun tekrar kullanımında belirsiz olabilir. Haritada doğrulayın; desen bilinçliyse işlem gerekmez.",
                );
                // Harita bağlamı: ±3 komşu durak + shape_id + sıra numaraları
                let ctx_b: Vec<&str> = sorted[..pi].iter().rev().take(3).rev()
                    .map(|s| idx.stop_id_of(s)).collect();
                let ctx_a: Vec<&str> = sorted[pi + 1..].iter().take(3)
                    .map(|s| idx.stop_id_of(s)).collect();
                let seq_b: Vec<String> = sorted[..pi].iter().rev().take(3).rev()
                    .map(|s| s.stop_sequence().unwrap_or(0).to_string()).collect();
                let seq_a: Vec<String> = sorted[pi + 1..].iter().take(3)
                    .map(|s| s.stop_sequence().unwrap_or(0).to_string()).collect();
                let mut det = HashMap::new();
                det.insert("ctx_b".to_string(), ctx_b.join(","));
                det.insert("ctx_a".to_string(), ctx_a.join(","));
                det.insert("seq_b".to_string(), seq_b.join(","));
                det.insert("seq_a".to_string(), seq_a.join(","));
                det.insert("shape_id".to_string(), shape_id.to_string());
                det.insert("bad_stop".to_string(), idx.stop_id_of(st).to_string());
                notice.details = Some(det);
                notices.push(notice);
            }
        }
    }


    // ── SHP_014: ilk/son durak güzergah ucundan uzakta (varyant-farkında) ─────
    // Bir shape birden çok trip varyantı (short-turn, farklı terminal) tarafından
    // paylaşılabilir. Shape ucu HİÇBİR varyantın ucuna yakın değilse ateşle:
    // shape başına min(dist) > eşik. En yakın varyant raporlanır. Deterministik.
    {
        let _t14 = Timer::start("K6::rem::shp_014");
        let threshold_km = config.stop_far_from_shape_m / 1000.0;
        // shape_id → (min_dist_km, problem_stop, trip_id)
        let mut start_agg: FxHashMap<&str, (f64, String, String)> = FxHashMap::default();
        let mut end_agg:   FxHashMap<&str, (f64, String, String)> = FxHashMap::default();

        for (&trip_id, stimes) in &idx.by_trip {
            let Some(&shape_id) = trip_shape_local.get(trip_id) else { continue };
            let Some(pts) = shape_coords.get(shape_id) else { continue };
            if pts.len() < 2 { continue; }
            let shape_start = pts[0];
            let shape_end = *pts.last().unwrap();

            if let Some(first_st) = stimes.first() {
                if let Some(&(slat, slon)) = stop_coords.get(idx.stop_id_of(first_st)) {
                    let d_km = haversine_km(slat, slon, shape_start.0, shape_start.1);
                    let e = start_agg.entry(shape_id)
                        .or_insert_with(|| (f64::INFINITY, String::new(), String::new()));
                    if d_km < e.0 {
                        e.0 = d_km;
                        e.1 = idx.stop_id_of(first_st).to_string();
                        e.2 = trip_id.to_string();
                    }
                }
            }

            if let Some(last_st) = stimes.last() {
                if let Some(&(slat, slon)) = stop_coords.get(idx.stop_id_of(last_st)) {
                    let d_km = haversine_km(slat, slon, shape_end.0, shape_end.1);
                    let e = end_agg.entry(shape_id)
                        .or_insert_with(|| (f64::INFINITY, String::new(), String::new()));
                    if d_km < e.0 {
                        e.0 = d_km;
                        e.1 = idx.stop_id_of(last_st).to_string();
                        e.2 = trip_id.to_string();
                    }
                }
            }
        }

        let mut start_ids: Vec<&str> = start_agg.keys().copied().collect();
        start_ids.sort_unstable();
        for shape_id in start_ids {
            let (min_km, stop_id, trip_id) = &start_agg[shape_id];
            if !min_km.is_finite() || *min_km <= threshold_km { continue; }
            let dist_m = *min_km * 1000.0;
            let sname = stop_names.get(stop_id.as_str()).copied().unwrap_or(stop_id.as_str());
            let prefix = shp_route_prefix(shape_route_labels.get(shape_id).map(|v| v.as_slice()).unwrap_or(&[]));
            let mut n = k6_notice(
                ctr, "SHP_014", EntityType::Shape,
                Some(shape_id.to_string()), Some(shape_id.to_string()),
                "shapes.txt", None, Some("shape_pt_sequence"),
                Some(format!("{dist_m:.0}m")),
                Some(format!("≤ {:.0}m", config.stop_far_from_shape_m)),
                format!(
                    "{prefix}'{}' güzergah şeklinin başlangıç noktası, en yakın seferin ilk durağı '{}' (kod: '{}') konumundan {dist_m:.0}m uzakta.",
                    shape_id, sname, stop_id
                ),
                "Bu uzantı bilinçliyse işlem gerekmez; değilse shape başlangıcını ve ilk durağı doğrulayın.",
            );
            let mut d = std::collections::HashMap::new();
            d.insert("problem_stop".to_string(), stop_id.clone());
            d.insert("endpoint".to_string(), "start".to_string());
            d.insert("trip_id".to_string(), trip_id.clone());
            n.details = Some(d);
            notices.push(n);
        }

        let mut end_ids: Vec<&str> = end_agg.keys().copied().collect();
        end_ids.sort_unstable();
        for shape_id in end_ids {
            let (min_km, stop_id, trip_id) = &end_agg[shape_id];
            if !min_km.is_finite() || *min_km <= threshold_km { continue; }
            let dist_m = *min_km * 1000.0;
            let sname = stop_names.get(stop_id.as_str()).copied().unwrap_or(stop_id.as_str());
            let prefix = shp_route_prefix(shape_route_labels.get(shape_id).map(|v| v.as_slice()).unwrap_or(&[]));
            let mut n = k6_notice(
                ctr, "SHP_014", EntityType::Shape,
                Some(shape_id.to_string()), Some(shape_id.to_string()),
                "shapes.txt", None, Some("shape_pt_sequence"),
                Some(format!("{dist_m:.0}m")),
                Some(format!("≤ {:.0}m", config.stop_far_from_shape_m)),
                format!(
                    "{prefix}'{}' güzergah şeklinin bitiş noktası, en yakın seferin son durağı '{}' (kod: '{}') konumundan {dist_m:.0}m uzakta.",
                    shape_id, sname, stop_id
                ),
                "Bu uzantı bilinçliyse işlem gerekmez; değilse shape bitişini ve son durağı doğrulayın.",
            );
            let mut d = std::collections::HashMap::new();
            d.insert("problem_stop".to_string(), stop_id.clone());
            d.insert("endpoint".to_string(), "end".to_string());
            d.insert("trip_id".to_string(), trip_id.clone());
            n.details = Some(d);
            notices.push(n);
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
                    Some(stop.line as u64),
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

        // Hangi station_id'lerin PERONU var? (location_type 0 veya boş)
        //
        // Yalnız peron sayılır: `parent_station` giriş/çıkış (2), genel düğüm (3) gibi
        // kayıtlarca da yazılır, ama sefer yalnız PERONDA durur. Sadece girişi olup
        // peronu olmayan istasyon işlevsizdir — kartın kararı da "en az bir durak/platform
        // tarafından kullanılmalı" der.
        //
        // Filtre yokken her çocuk sayılıyordu → 250-feed corpus, mdb-2933: `MTR-POA` (Po Lam)
        // istasyonunun 6 çocuğunun HEPSİ location_type=2 (giriş), peronu YOK; biz susuyorduk,
        // MobilityData `unused_station` ile işaretliyordu.
        let stations_with_children: HashSet<&str> = records.stops.iter()
            .filter(|s| matches!(s.location_type, None | Some(0)))
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
                    Some(stop.line as u64),
                    None,
                    None,
                    None,
                    format!(
                        "'{}' kodlu '{}' istasyonunun hiç peronu yok — hiçbir durak (location_type=0) \
                         onu parent_station olarak kullanmıyor; yalnız giriş/düğüm kayıtları bağlı olabilir.",
                        stop.stop_id, sname
                    ),
                    "İstasyona bağlı peronlarda (location_type=0) parent_station alanını doldurun veya istasyonu kaldırın.",
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
                if let Some(hs) = records.stop_times_index.stop_headsign_of(st) {
                    let s = hs.as_str();
                    if !seen_caps.contains(s) && is_all_caps(s) {
                        seen_caps.insert(s);
                        caps.push((st.line as u64, s.to_string()));
                    }
                    if !seen_lower.contains(s) && is_all_lower(s) {
                        seen_lower.insert(s);
                        lower.push((st.line as u64, s.to_string()));
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
                    "stops.txt", Some(stop.line as u64), Some("stop_name"),
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
                    "routes.txt", Some(route.line as u64), Some("route_long_name"),
                    Some(name.to_string()), None,
                    format!("'{}' hattının uzun adı tamamen büyük harf: '{name}'.", label),
                    "Hat adını düzgün harf kuralıyla yazın.",
                ));
            }
        }

        // route_desc
        for route in &records.routes {
            if route.route_id.is_empty() { continue; }
            if let Some(desc) = route.route_desc.as_deref().filter(|s| is_all_caps(s)) {
                let label = route.route_short_name.as_deref()
                    .filter(|s| !s.is_empty()).unwrap_or(route.route_id.as_str());
                notices.push(k6_notice(
                    ctr, "DQ_018", EntityType::Route,
                    Some(route.route_id.clone()), Some(route.route_id.clone()),
                    "routes.txt", Some(route.line as u64), Some("route_desc"),
                    Some(desc.to_string()), None,
                    format!("'{}' hattının açıklaması tamamen büyük harf: '{desc}'.", label),
                    "Hat açıklamasını düzgün harf kuralıyla yazın.",
                ));
            }
        }

        // trip_headsign
        for trip in &records.trips {
            if trip.trip_id.is_empty() { continue; }
            if let Some(hs) = ti_rem.headsign(trip).filter(|s| is_all_caps(s)) {
                notices.push(k6_notice(
                    ctr, "DQ_018", EntityType::Trip,
                    Some(trip.trip_id.to_string()), Some(trip.trip_id.to_string()),
                    "trips.txt", Some(trip.line as u64), Some("trip_headsign"),
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
                    "agency.txt", Some(ag.line as u64), Some("agency_name"),
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
                    "feed_info.txt", Some(fi.line as u64), Some("feed_publisher_name"),
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
                    "stops.txt", Some(stop.line as u64), Some("stop_name"),
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
                    "routes.txt", Some(route.line as u64), Some("route_long_name"),
                    Some(name.to_string()), None,
                    format!("'{}' hattının uzun adı tamamen küçük harf: '{name}'.", route.route_id),
                    "Hat adını başlık harfiyle yazın.",
                ));
            }
        }
        for route in &records.routes {
            if route.route_id.is_empty() { continue; }
            if let Some(desc) = route.route_desc.as_deref().filter(|s| is_all_lower(s)) {
                notices.push(k6_notice(
                    ctr, "DQ_019", EntityType::Route,
                    Some(route.route_id.clone()), Some(route.route_id.clone()),
                    "routes.txt", Some(route.line as u64), Some("route_desc"),
                    Some(desc.to_string()), None,
                    format!("'{}' hattının açıklaması tamamen küçük harf: '{desc}'.", route.route_id),
                    "Hat açıklamasını başlık harfiyle yazın.",
                ));
            }
        }
        for trip in &records.trips {
            if trip.trip_id.is_empty() { continue; }
            if let Some(hs) = ti_rem.headsign(trip).filter(|s| is_all_lower(s)) {
                notices.push(k6_notice(
                    ctr, "DQ_019", EntityType::Trip,
                    Some(trip.trip_id.to_string()), Some(trip.trip_id.to_string()),
                    "trips.txt", Some(trip.line as u64), Some("trip_headsign"),
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
                    "agency.txt", Some(ag.line as u64), Some("agency_name"),
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
                    "feed_info.txt", Some(fi.line as u64), Some("feed_publisher_name"),
                    Some(fi.feed_publisher_name.clone()), None,
                    format!("feed_publisher_name tamamen küçük harf: '{}'.", fi.feed_publisher_name),
                    "Yayıncı adını başlık harfiyle yazın.",
                ));
            }
        }
    }

    // ── DQ_020: önerilen alan eksik/boş (missing_recommended_field) ──────────
    // Field-scoped (registry: VS, Field): trip_headsign çoğu/tüm satırda boşsa
    // trips.txt için TEK notice. Önceki davranış her trip için ateşliyordu →
    // LA Metro gibi feed'lerde 33.642 cap-busting gürültü üretiyordu.
    {
        let _t20 = Timer::start("K6::rem::dq_020");
        let total = records.trips.iter().filter(|t| !t.trip_id.is_empty()).count();
        let missing = records.trips.iter()
            .filter(|t| !t.trip_id.is_empty())
            .filter(|t| ti_rem.headsign(t).map(|s| s.trim().is_empty()).unwrap_or(true))
            .count();
        if missing > 0 && total > 0 {
            notices.push(k6_notice(
                ctr, "DQ_020", EntityType::Feed,
                None, None, "trips.txt", None, Some("trip_headsign"),
                Some(format!("{missing}/{total}")), None,
                format!("trips.txt'te önerilen trip_headsign alanı {missing}/{total} satırda boş — yolcu bilgi sistemleri için doldurulması önerilir."),
                "trips.txt'teki trip_headsign sütununu doldurun.",
            ));
        }
    }

    // ── OPR_009: gece servisi — trip tekrarları yerine route başına tek özet ──
    {
        let _t09 = Timer::start("K6::rem::opr_009");
        let mut by_route: FxHashMap<&str, (u32, u32, u32)> = FxHashMap::default();
        let sds_sec = config.service_day_start_hour * 3600;
        for (&trip_id, stimes) in &idx.by_trip {
            if let Some(dep) = stimes.first().and_then(|s| s.departure_time()) {
                let dep_sec = dep.0 * 3600 + dep.1 * 60 + dep.2;
                if dep_sec >= 23 * 3600 || (sds_sec > 0 && dep_sec < sds_sec) {
                    let route = trip_to_route_rem.get(trip_id).copied().unwrap_or(trip_id);
                    let e = by_route.entry(route).or_insert((0, dep_sec, dep_sec));
                    e.0 += 1; e.1 = e.1.min(dep_sec); e.2 = e.2.max(dep_sec);
                }
            }
        }
        let mut routes: Vec<_> = by_route.into_iter().collect(); routes.sort_by_key(|x| x.0);
        for (route, (count, min, max)) in routes {
            let hm = |s: u32| format!("{:02}:{:02}", s / 3600, s / 60 % 60);
            notices.push(k6_notice(
                ctr, "OPR_009", EntityType::Route,
                Some(route.to_string()), Some(route.to_string()), "stop_times.txt", None,
                Some("departure_time"), Some(format!("{count} sefer, {}-{}", hm(min), hm(max))), None,
                format!("'{route}' hattında {count} gece seferi var (ilk kalkış aralığı {}-{}).", hm(min), hm(max)),
                "Bu bilgi notu; gece servisleri için beklenen bir durumdur.",
            ));
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
                    stop_coords.get(idx.stop_id_of(first))
                        .zip(stop_coords.get(idx.stop_id_of(last)))
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
            .filter(|t| !ti_rem.service_id(t).is_empty())
            .map(|t| ti_rem.service_id(t))
            .collect();
        for (svc_id, dates) in &derived.calendar_bitmap.active_dates {
            if dates.is_empty() { continue; } // OPR_011 zaten yakaladı
            if dates.len() >= MIN_DAYS { continue; }
            if !used_services_18.contains(svc_id.as_str()) { continue; }
            notices.push(k6_notice(
                ctr, "OPR_018", EntityType::Service,
                Some(svc_id.clone()), Some(svc_id.clone()),
                "calendar.txt", None, None,
                Some(format!("{}", dates.len())),
                Some(format!("≥ {MIN_DAYS}")),
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
            if ti_rem.route_id(t).is_empty() { continue; }
            if let Some(wc) = t.wheelchair_accessible {
                let e = route_wc.entry(ti_rem.route_id(t)).or_default();
                if wc == 1 { e.0 = true; }
                if wc == 2 { e.1 = true; }
            }
            if let Some(ba) = t.bikes_allowed {
                let e = route_ba.entry(ti_rem.route_id(t)).or_default();
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
                    format!("'{}' kodlu hatta bazı seferler tekerlekli sandalye erişimli (1), bazıları erişimsiz (2) olarak işaretlenmiş.", route_short.get(*route_id).copied().unwrap_or(*route_id)),
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
                    format!("'{}' kodlu hatta bazı seferler bisiklete izin veriyor (1), bazıları vermiyor (2).", route_short.get(*route_id).copied().unwrap_or(*route_id)),
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
                    Some(format!("{avg}s ({count})")),
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
                        "transfers.txt", Some(trf.line as u64),
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
                    Some(format!("{}", pts.len())), Some("≥ 3".to_string()),
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
                        Some(format!("{} / {total_km:.1}km", pts.len())),
                        Some(format!("≥ {MIN_POINTS_PER_10KM:.0}/10km")),
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
                        let prefix = shp_route_prefix(shape_route_labels.get(*shape_id).map(|v| v.as_slice()).unwrap_or(&[]));
                        notices.push(k6_notice(
                            ctr, "SHP_020", EntityType::Shape,
                            Some(shape_id.to_string()), Some(shape_id.to_string()),
                            "shapes.txt", None, Some("shape_pt_lat|shape_pt_lon"),
                            Some(format!("{i}/{j}: ({:.6},{:.6})", pts[i].0, pts[i].1)),
                            None,
                            format!(
                                "{prefix}'{shape_id}' güzergahında nokta {i} ile {j} neredeyse aynı konumda ({:.6},{:.6}) — tekrarlayan nokta.",
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
                        let prefix = shp_route_prefix(shape_route_labels.get(*shape_id).map(|v| v.as_slice()).unwrap_or(&[]));
                        notices.push(k6_notice(
                            ctr, "SHP_009", EntityType::Shape,
                            Some(shape_id.to_string()), Some(shape_id.to_string()),
                            "shapes.txt", None, Some("shape_pt_lat|shape_pt_lon"),
                            Some(format!("segment {i}-{} ∩ {j}-{}", i+1, j+1)), None,
                            format!(
                                "{prefix}'{shape_id}' güzergah şekli segment {i}→{} ile segment {j}→{} arasında kendisiyle kesişiyor.",
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
    config: &ValidatorConfig,
    idx: &StopTimesIndex<'_>,
    notices: &mut Vec<Notice>,
    ctr: &mut u32,
) {
    let ti_shp012 = &records.trip_interns;
    // shape_id → sıralı (lat, lon) + bbox (build_maps ile birebir aynı kurulum)
    // #15 (build_maps OOM/312s): shape_coords'u NOKTA-İNDEKSLERİYLE kur. Eski yol her noktayı
    // (u32,f64,f64)=24B ara tamponda gruplayıp sonra (f64,f64)=16B'ye kopyalıyordu → transient
    // ~40B/nokta. Devasa-shape feed'inde bu, K6 başındaki ~3.6 GB'ın üstüne binince 4 GB tavanı
    // aşıp OOM ediyordu (ve allocator tavanda tıkanınca 312 sn). İndeks (4B) + coords (16B) =
    // ~20B/nokta tepe. Çıktı (seq-sıralı lat,lon) BİREBİR aynı (stable sort, file-order ties).
    let mut shape_pt_idx: FxHashMap<&str, Vec<u32>> = FxHashMap::default();
    for (i, sp) in records.shapes.iter().enumerate() {
        if sp.shape_pt_lat.is_some() && sp.shape_pt_lon.is_some() {
            shape_pt_idx.entry(sp.shape_id.as_str()).or_default().push(i as u32);
        }
    }
    let n_shapes = shape_pt_idx.len();
    let mut shape_coords: FxHashMap<&str, Vec<(f64, f64)>> = FxHashMap::default();
    let mut shape_bbox: FxHashMap<&str, [f64; 4]> = FxHashMap::default();
    shape_coords.reserve(n_shapes);
    shape_bbox.reserve(n_shapes);
    for (sid, mut idxs) in shape_pt_idx {
        idxs.sort_by_key(|&i| records.shapes[i as usize].shape_pt_sequence.unwrap_or(0));
        let pts: Vec<(f64, f64)> = idxs.iter()
            .map(|&i| {
                let sp = &records.shapes[i as usize];
                (sp.shape_pt_lat.unwrap(), sp.shape_pt_lon.unwrap())
            })
            .collect();
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
        let shp_stop_threshold_m: f64 = config.stop_far_from_shape_m;

        // Perf: (shape_id, stop_id) → polyline mesafesi MEMOIZE edilir. Aynı shape'i kullanan
        // onlarca sefer aynı (shape,durak) mesafesini tekrar tekrar hesaplıyordu; pahalı
        // point_to_polyline yalnızca BENZERSİZ çift başına bir kez çalışır. Sayım semantiği
        // (sefer-durak örneği başına artış) AYNI kalır → notice sayısı/değeri/skor DEĞİŞMEZ.
        let mut shape_stop_violations: FxHashMap<&str, u32> = FxHashMap::default();
        let mut dist_cache: FxHashMap<(&str, &str), f64> = FxHashMap::default();
        for trip in &records.trips {
            let Some(shape_id) = ti_shp012.shape_id(trip).filter(|s| !s.is_empty()) else { continue };
            let Some(pts) = shape_coords.get(shape_id) else { continue };
            let Some(stimes) = idx.by_trip.get(trip.trip_id.as_str()) else { continue };

            for st in stimes.iter() {
                let stop_id = idx.stop_id_of(st);
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
                            let margin_lat = shp_stop_threshold_m / 111_320.0_f64;
                            let margin_lon = shp_stop_threshold_m / (111_320.0_f64 * cos_lat);
                            if slat < bmin_la - margin_lat || slat > bmax_la + margin_lat
                                || slon < bmin_lo - margin_lon || slon > bmax_lo + margin_lon {
                                let clat = slat.clamp(bmin_la, bmax_la);
                                let clon = slon.clamp(bmin_lo, bmax_lo);
                                return haversine_km(slat, slon, clat, clon) * 1000.0;
                            }
                        }
                        point_to_polyline_dist_m(slat, slon, pts)
                    });
                if min_dist_m > shp_stop_threshold_m {
                    *shape_stop_violations.entry(shape_id).or_insert(0) += 1;
                }
            }
        }

        for (shape_id, viol_count) in &shape_stop_violations {
            notices.push(k6_notice(
                ctr, "SHP_012", EntityType::Shape,
                Some(shape_id.to_string()), Some(shape_id.to_string()),
                "shapes.txt", None, Some("shape_pt_lat|shape_pt_lon"),
                Some(format!("{viol_count} (>{shp_stop_threshold_m:.0}m)")), None,
                format!(
                    "'{shape_id}' güzergah şekli {viol_count} duraktan >{shp_stop_threshold_m:.0}m uzakta — güzergah doğru çizilmemiş olabilir."
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
    let ti_shp022 = &records.trip_interns;

    // ── build_maps ile birebir aynı kurulum (shape_coords + shape_bbox) ──────
    // #15 (build_maps OOM/312s): shape_coords'u NOKTA-İNDEKSLERİYLE kur. Eski yol her noktayı
    // (u32,f64,f64)=24B ara tamponda gruplayıp sonra (f64,f64)=16B'ye kopyalıyordu → transient
    // ~40B/nokta. Devasa-shape feed'inde bu, K6 başındaki ~3.6 GB'ın üstüne binince 4 GB tavanı
    // aşıp OOM ediyordu (ve allocator tavanda tıkanınca 312 sn). İndeks (4B) + coords (16B) =
    // ~20B/nokta tepe. Çıktı (seq-sıralı lat,lon) BİREBİR aynı (stable sort, file-order ties).
    let mut shape_pt_idx: FxHashMap<&str, Vec<u32>> = FxHashMap::default();
    for (i, sp) in records.shapes.iter().enumerate() {
        if sp.shape_pt_lat.is_some() && sp.shape_pt_lon.is_some() {
            shape_pt_idx.entry(sp.shape_id.as_str()).or_default().push(i as u32);
        }
    }
    let n_shapes = shape_pt_idx.len();
    let mut shape_coords: FxHashMap<&str, Vec<(f64, f64)>> = FxHashMap::default();
    let mut shape_bbox: FxHashMap<&str, [f64; 4]> = FxHashMap::default();
    shape_coords.reserve(n_shapes);
    shape_bbox.reserve(n_shapes);
    for (sid, mut idxs) in shape_pt_idx {
        idxs.sort_by_key(|&i| records.shapes[i as usize].shape_pt_sequence.unwrap_or(0));
        let pts: Vec<(f64, f64)> = idxs.iter()
            .map(|&i| {
                let sp = &records.shapes[i as usize];
                (sp.shape_pt_lat.unwrap(), sp.shape_pt_lon.unwrap())
            })
            .collect();
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
        .filter_map(|t| ti_shp022.shape_id(t).map(|s| (t.trip_id.as_str(), s)))
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
        // #15: SHP_022 Entity-scope (stop_id) → dedup stop başına TEK temsilciye (min st.line,
        // tiebreak shape_id) çöker. (shape,stop) başına emit yerine stop başına BUFFER tut →
        // büyük feed'de ara-notice patlaması yok; sonuç dedup ile birebir + iterasyon-bağımsız.
        let mut shp022_best: FxHashMap<&str, ((u64, &str), Notice)> = FxHashMap::default();

        for (trip_id, stimes) in &idx.by_trip {
            // Sadece shape_dist_traveled eksik trippler
            if !idx.trips_missing_sdt.contains_key(trip_id) { continue; }
            let Some(&shape_id) = trip_shape_local.get(trip_id) else { continue };
            let Some(pts) = shape_coords.get(shape_id) else { continue };
            if pts.len() < 2 { continue; }

            // #52: stop_sequence KULLANILABİLİR ise (mevcut + kesin artan + tekrarsız)
            // sıra-farkında monoton eşleme durağın shape üzerindeki konumunu çözer →
            // SHP_022 gereksiz (normal feed'lerde büyük over-fire). Yalnız sequence
            // eksik/geçersiz (STM_005) veya duplicate (STM_032) trip'lerde konum gerçekten
            // belirsizdir. by_trip dilimi stop_sequence'a göre artan sıralı (None=MAX sonda).
            let seq_usable = {
                let mut last: Option<u32> = None;
                let mut ok = true;
                for st in stimes.iter() {
                    match st.stop_sequence() {
                        None => { ok = false; break; }
                        Some(s) => {
                            if last.is_some_and(|l| s <= l) { ok = false; break; }
                            last = Some(s);
                        }
                    }
                }
                ok
            };
            if seq_usable { continue; }

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
                let stop_id = idx.stop_id_of(st);
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
                // Stop başına yalnız en küçük (st.line, shape_id) temsilciyi tut (dedup eşdeğeri).
                let best_key = (st.line as u64, shape_id);
                if shp022_best.get(stop_id).is_some_and(|(k, _)| *k <= best_key) {
                    continue;
                }
                let sname = stop_names.get(stop_id).copied().unwrap_or(stop_id);
                let mut notice = k6_notice(
                    ctr,
                    "SHP_022",
                    EntityType::Stop,
                    Some(stop_id.to_string()),
                    Some(stop_id.to_string()),
                    "stop_times.txt",
                    Some(st.line as u64),
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
                shp022_best.insert(stop_id, (best_key, notice));
            }
        }
        // Buffer'ı boşalt: stop başına tek (deterministik) temsilci. Sıra önemsiz (K6 renumber
        // + K7 dedup sonradan kanonik sıralar).
        for (_stop, (_key, notice)) in shp022_best {
            notices.push(notice);
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

/// Paketlenmiş YYYYMMDD → Julian Day Number. Formül `k5_derived::ymd_to_jdn`'de;
/// burada yalnızca u32 açma + geçersiz tarih (0 bileşen) koruması var.
fn yyyymmdd_to_jdn(yyyymmdd: u32) -> u32 {
    let y = yyyymmdd / 10000;
    let m = (yyyymmdd / 100) % 100;
    let d = yyyymmdd % 100;
    if y == 0 || m == 0 || d == 0 {
        return 0;
    }
    ymd_to_jdn(y, m, d)
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
    let ti_cal_ov = &records.trip_interns;
    // ── route_id → service_id kümesi ─────────────────────────────────────────
    let mut route_services: HashMap<&str, Vec<&str>> = HashMap::new();
    for trip in &records.trips {
        if !ti_cal_ov.route_id(trip).is_empty() && !ti_cal_ov.service_id(trip).is_empty() {
            route_services
                .entry(ti_cal_ov.route_id(trip))
                .or_default()
                .push(ti_cal_ov.service_id(trip));
        }
    }
    // Servis listelerini sırala ve dedup et
    for svcs in route_services.values_mut() {
        svcs.sort_unstable();
        svcs.dedup();
    }

    // ── calendar_dates'te exception kaydı olan (service_id → tarih üyelik) ──
    // OPR_020: conflict günü, bu tabloda herhangi bir çakışan servis varsa HIGH.
    // CalendarDateIndex.added / .removed zaten parse anında sort_unstable yapılmış;
    // binary_search ile O(log n) üyelik sorgusu — ayrı svc_date_exc map'i YOK.
    // has_exc helper: serviste belirtilen tarih için herhangi bir exception (type 1 veya 2) var mı?
    let has_exc_date = |svc: &str, date: u32| -> bool {
        records.calendar_dates.added.get(svc).is_some_and(|v| v.binary_search(&date).is_ok())
            || records.calendar_dates.removed.get(svc).is_some_and(|v| v.binary_search(&date).is_ok())
    };

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
            let has_exc = svcs.iter().any(|&s| has_exc_date(s, date));
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
                Some(format!("{count}")),
                Some("≤ 1/gün".to_string()),
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
            let patterns_str = summarize_patterns(&exc_patterns);
            // Her örnek tarihin yanında o tarihte çakışan service_id'leri göster:
            // "20260720 (X, Y)". Dil-nötr (yalnız tarih + service_id'ler) tutulur ki
            // tr mesajı ve en/ja şablonu aynı stringi güvenle kullanabilsin.
            let sample_detail: String = exc_dates.iter().take(3)
                .map(|d| {
                    let svcs = date_services.get(d)
                        .map(|v| {
                            let mut s = v.clone();
                            s.sort_unstable();
                            s.join(", ")
                        })
                        .unwrap_or_default();
                    if svcs.is_empty() {
                        d.to_string()
                    } else {
                        format!("{d} ({svcs})")
                    }
                })
                .collect::<Vec<_>>()
                .join("; ");
            let msg = format!(
                "'{}' hattında {} exception gününde birden fazla aktif servis var — override çakışması riski. Örnek tarihler (parantezde çakışan çalışma takvimleri): {}.",
                route_id, count, sample_detail
            );
            let mut n = k6_notice(
                ctr, "OPR_020", EntityType::Route,
                Some(route_id.to_string()), Some(route_id.to_string()),
                "trips.txt", None, None,
                Some(format!("{count}")),
                Some("≤ 1/gün".to_string()),
                msg,
                "Override günlerinde base servisi calendar_dates.txt ile kaldırın (exception_type=2).",
            );
            n.details = Some({
                let mut d = std::collections::HashMap::new();
                d.insert("conflict_day_count".to_string(), count.to_string());
                d.insert("sample_dates".to_string(),
                    exc_dates.iter().take(10).map(|x| x.to_string()).collect::<Vec<_>>().join(","));
                d.insert("sample_dates_detail".to_string(), sample_detail);
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

        let start_jdn = yyyymmdd_to_jdn(rule.start_date);
        let end_jdn = yyyymmdd_to_jdn(rule.end_date);

        let mut dates_021: Vec<u32> = Vec::new();
        let mut dates_022: Vec<u32> = Vec::new();
        let mut dates_023: Vec<u32> = Vec::new();

        for jdn in start_jdn..=end_jdn {
            let date = jdn_to_yyyymmdd(jdn);
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
        let route_short_o4: FxHashMap<&str, &str> = records.routes.iter()
            .map(|r| (r.route_id.as_str(), r.route_short_name.as_deref().filter(|s| !s.is_empty()).unwrap_or(r.route_id.as_str())))
            .collect();
        let mut route_services: FxHashMap<&str, FxHashSet<&str>> = FxHashMap::default();
        for t in &records.trips {
            if !ti_cal_ov.route_id(t).is_empty() && !ti_cal_ov.service_id(t).is_empty() {
                route_services.entry(ti_cal_ov.route_id(t)).or_default().insert(ti_cal_ov.service_id(t));
            }
        }
        for (route_id, service_ids) in &route_services {
            let has_weekend = service_ids.iter().any(|&svc| {
                derived.calendar_bitmap.active_dates.get(svc)
                    .is_some_and(|dates| dates.iter().any(|&d| {
                        let jdn = yyyymmdd_to_jdn(d);
                        jdn % 7 == 5 || jdn % 7 == 6 // Cumartesi=5, Pazar=6
                    }))
            });
            if !has_weekend {
                notices.push(k6_notice(
                    ctr, "OPR_004", EntityType::Route,
                    Some(route_id.to_string()), Some(route_id.to_string()),
                    "trips.txt", None, None, None, None,
                    format!("'{}' kodlu hattın aktif servislerinde hafta sonu (Cumartesi/Pazar) günü yok.", route_short_o4.get(*route_id).copied().unwrap_or(*route_id)),
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
            .filter(|t| !ti_cal_ov.service_id(t).is_empty())
            .map(|t| ti_cal_ov.service_id(t))
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
                let d = yyyymmdd_to_jdn(w[1])
                    .saturating_sub(yyyymmdd_to_jdn(w[0]))
                    .saturating_sub(1);
                if d > max_gap { max_gap = d; gap_start = w[0]; gap_end = w[1]; }
            }
            if max_gap >= gap_threshold {
                let mut n012 = k6_notice(
                    ctr, "OPR_012", EntityType::Service,
                    Some(svc_id.clone()), Some(svc_id.clone()),
                    "calendar.txt", None, None,
                    Some(format!("{max_gap}")),
                    Some(format!("≤ {gap_threshold}")),
                    format!("'{svc_id}' servisinde {gap_start}-{gap_end} arasında {max_gap} günlük servis boşluğu var."),
                    "Boşluk planlanmışsa görmezden gelin; aksi hâlde eksik günleri calendar_dates.txt ile ekleyin.",
                );
                n012.service_id = Some(svc_id.clone());
                notices.push(n012);
            }
        }
    }

    // ── OPR_015: hatta yalnızca tek shape tanımlı ─────────────────────────────
    {
        let _t15b = Timer::start("K6::rem::opr_015");
        let mut route_shape_set: FxHashMap<&str, FxHashSet<&str>> = FxHashMap::default();
        let mut route_dirs: FxHashMap<&str, FxHashSet<u32>> = FxHashMap::default();
        for t in &records.trips {
            if ti_cal_ov.route_id(t).is_empty() { continue; }
            if let Some(shape) = ti_cal_ov.shape_id(t).filter(|s| !s.is_empty()) {
                route_shape_set.entry(ti_cal_ov.route_id(t)).or_default().insert(shape);
            }
            if let Some(d) = t.direction_id {
                route_dirs.entry(ti_cal_ov.route_id(t)).or_default().insert(d);
            }
        }
        // route_id → route_type (raylı sistemler için skip)
        let rt_map: FxHashMap<&str, u32> = records.routes.iter()
            .filter_map(|r| r.route_type.map(|rt| (r.route_id.as_str(), rt)))
            .collect();
        let route_short_o15: FxHashMap<&str, &str> = records.routes.iter()
            .map(|r| (r.route_id.as_str(), r.route_short_name.as_deref().filter(|s| !s.is_empty()).unwrap_or(r.route_id.as_str())))
            .collect();
        for (route_id, shapes) in &route_shape_set {
            // Tram(0), Metro(1), Demiryolu(2), Kablo tramvay(5), Monoray(12):
            // tek yönlü ray hattı; tek shape beklenen davranış
            let rt = rt_map.get(*route_id).copied().unwrap_or(3);
            if matches!(rt, 0 | 1 | 2 | 5 | 12) { continue; }
            // OPR_015 yalnız ÇİFT YÖNLÜ hatlarda anlamlı: iki yön aynı tek shape'i paylaşıyorsa
            // "gidiş-dönüş için ayrı shape" önerilir. Tek yönlü / yön-tanımsız hatta tek shape
            // beklenen davranıştır (öneri yanıltıcı olur) → susulur.
            let bidirectional = route_dirs.get(*route_id).map_or(false, |d| d.len() > 1);
            if shapes.len() == 1 && bidirectional {
                let shape_id = shapes.iter().next().copied().unwrap_or("");
                let mut n015 = k6_notice(
                    ctr, "OPR_015", EntityType::Route,
                    Some(route_id.to_string()), Some(route_id.to_string()),
                    "trips.txt", None, Some("shape_id"),
                    Some(format!("1 shape ({shape_id})")), None,
                    format!("'{}' kodlu hattın tüm seferleri tek bir güzergah şekli kullanıyor ({shape_id}).", route_short_o15.get(*route_id).copied().unwrap_or(*route_id)),
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

        // PTH_012: her platform için en az bir entrance'tan erişilebilir olmalı.
        // İç içe modellemede (peron lt=0 → boarding area lt=4) asıl pathway düğümü
        // boarding area'dır; peron-container'ın kendisi grafikte olmayabilir. Çocuk
        // düğümlerinden biri entrance'tan erişilebilirse peron da erişilebilir sayılır
        // (#50, #49 ile aynı iç içe-istasyon kalıbı; mdb-3127 Santiago Metro 286 FP).
        for &platform in &platforms {
            let reachable = reachable_from_entrances.contains(platform)
                || station_children.get(platform).map_or(false, |kids| {
                    kids.iter()
                        .any(|k| reachable_from_entrances.contains(k.stop_id.as_str()))
                });
            if !reachable {
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

        // #51 (#50 ile aynı iç içe-istasyon kalıbı): peron-container (lt0) accessible
        // graph düğümü olmayabilir; boarding-area (lt4) çocuğu erişilebilirse peron da
        // erişilebilir sayılır. Aksi halde erişilebilir istasyon "rotasız" sanılır.
        let any_platform_accessible = platforms.iter().any(|p| {
            accessible_reachable.contains(p)
                || station_children.get(p).map_or(false, |kids| {
                    kids.iter()
                        .any(|k| accessible_reachable.contains(k.stop_id.as_str()))
                })
        });

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
                        Some(pw.line as u64),
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
    derived: &DerivedData,
    config: &ValidatorConfig,
    idx: &StopTimesIndex<'_>,
    notices: &mut Vec<Notice>,
    ctr: &mut u32,
) {
    let ti_vat = &records.trip_interns;
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
        .map(|t| (t.trip_id.as_str(), ti_vat.route_id(t)))
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
            if idx.stop_id_of(st).is_empty() { continue; }
            let sid = idx.stop_id_of(st);
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
        // Her hattın kullandığı service_id'ler — takvim-kesişim guard'ı için (aşağıda).
        let route_services: FxHashMap<&str, FxHashSet<&str>> = {
            let mut m: FxHashMap<&str, FxHashSet<&str>> = FxHashMap::default();
            for t in &records.trips {
                m.entry(ti_vat.route_id(t)).or_default().insert(ti_vat.service_id(t));
            }
            m
        };
        // İki hat herhangi bir günde birlikte çalışıyor mu? (servis takvimleri kesişiyor mu)
        // Bir service_id'nin çözülebilir aktif günü varsa "tarihli" sayılır; ikisi de tarihliyse
        // VE hiç ortak gün yoksa → bunlar zamansal olarak ayrık (kopya değil, bk. #29 yorumu).
        let active = &derived.calendar_bitmap.active_dates;
        let route_has_dates = |svcs: &FxHashSet<&str>| -> bool {
            svcs.iter().any(|s| active.get(*s).is_some_and(|d| !d.is_empty()))
        };
        let routes_share_day = |sa: &FxHashSet<&str>, sb: &FxHashSet<&str>| -> bool {
            for &x in sa {
                let Some(da) = active.get(x) else { continue };
                if da.is_empty() { continue; }
                for &y in sb {
                    if let Some(db) = active.get(y) {
                        if da.intersection(db).next().is_some() { return true; }
                    }
                }
            }
            false
        };

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
                    if jaccard < 0.85 { continue; }

                    // Takvim-kesişim guard'ı (#29): aynı public hat servis-değişiminde yeni
                    // route_id alabilir (TriMet '20'/'20a': '20' yaz takvimi, '20a' sonbahar,
                    // tarih aralıkları ayrık). Bu kopya değil zamansal bölünmedir; "birleştir"
                    // önerisi geçersiz. İki hat da çözülebilir tarihliyse VE hiç ortak günü
                    // yoksa atla. Takvim eksikse (kanıt yok) eski davranış: emit.
                    let (Some(svcs_a), Some(svcs_b)) =
                        (route_services.get(rid_a), route_services.get(rid_b)) else { continue };
                    if route_has_dates(svcs_a) && route_has_dates(svcs_b)
                        && !routes_share_day(svcs_a, svcs_b) {
                        continue;
                    }

                    let la = route_label.get(rid_a).copied().unwrap_or(rid_a);
                    let lb = route_label.get(rid_b).copied().unwrap_or(rid_b);
                    // Etiket route_id'den farklıysa route_id'yi de göster (aynı kısa-adlı iki
                    // farklı hat "'20' ve '20'" gibi ayırt edilemez bir mesaj üretmesin).
                    let disp = |rid: &str, label: &str| if label == rid {
                        format!("'{rid}'")
                    } else {
                        format!("'{label}' (route_id '{rid}')")
                    };
                    let mut n = k6_notice(
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
                        format!("{} ve {} hatları %{:.0} oranda aynı durağı paylaşıyor — muhtemel kopya hat.",
                            disp(rid_a, la), disp(rid_b, lb), jaccard * 100.0),
                        "İki hattı birleştirin ya da güzergahları ayırt edilir biçimde farklılaştırın.",
                    );
                    // Harita: her iki hattı farklı renkte çizebilmek için ikisini de ver.
                    // route_b: EN/JA mesaj şablonu ikinci hattı adlandırabilsin diye (details
                    // anahtarları tMsg'de parametre olur).
                    let mut d = std::collections::HashMap::new();
                    d.insert("routes".to_string(), format!("{rid_a},{rid_b}"));
                    d.insert("route_b".to_string(), rid_b.to_string());
                    n.details = Some(d);
                    notices.push(n);
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
            // Hatları kararlı sırada topla: harita için route_id'ler, mesaj için okunur etiketler.
            let mut route_ids: Vec<&str> = routes.iter().copied().collect();
            route_ids.sort_unstable();
            let route_labels: Vec<&str> = route_ids.iter()
                .map(|rid| route_label.get(rid).copied().unwrap_or(rid))
                .collect();
            let routes_label_str = route_labels.join(", ");
            let mut n002 = k6_notice(
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
                format!("'{name}' (kod: '{stop_id}') durağından {} hat geçiyor ({routes_label_str}); transfers.txt'te aktarma tanımlı değil.",
                    routes.len()),
                "transfers.txt'e bu durak için aktarma kayıtları ekleyin (transfer_type=2 ile bekleme süresi belirtin).",
            );
            n002.details = Some({
                let mut d = std::collections::HashMap::new();
                d.insert("routes".to_string(), route_ids.join(","));
                d
            });
            notices.push(n002);
        }
    }

    // ── VAT_003: Sefer süresi istatistiksel aykırı değer (MAD yöntemi) ───────
    // Gruplama anahtarı route_id DEĞİL, sefer DESENİ'dir: bir route_id çoğu zaman
    // birden çok desen içerir (kısa-dönüş, varyant, yön) ve bunların süreleri doğal
    // olarak farklıdır; hepsini tek medyanla karşılaştırmak kitlesel FP üretir.
    // Anahtar shape_id + sıralı durak desenidir. Aynı shape kısa-dönüş ve tam sefer
    // gibi farklı stop dizileriyle paylaşılabilir; yalnız shape_id ile gruplamak bunların
    // doğal süre farkını aykırı değer sanır (BART Green-N: 5 durak/23dk vs 22 durak/83dk).
    // Shape yoksa route_id + sıralı durak dizisi hash'ine düşülür.
    {
        let trip_shape: FxHashMap<&str, &str> = records.trips.iter()
            .filter_map(|t| ti_vat.shape_id(t).map(|s| (t.trip_id.as_str(), s)))
            .collect();

        #[derive(PartialEq, Eq, Hash)]
        enum DurKey<'a> { ShapePattern(&'a str, u64), Pattern(&'a str, u64) }

        let mut groups: FxHashMap<DurKey, Vec<(&str, u32, u32, &str)>> = FxHashMap::default();
        for (&trip_id, stops) in &idx.by_trip {
            if stops.len() < 2 { continue; }
            let route = match trip_route.get(trip_id) { Some(&r) => r, None => continue };
            let first_dep = stops.first().and_then(|s| s.departure_time()).map(hms_to_secs);
            let last_arr  = stops.last().and_then(|s| s.arrival_time()).map(hms_to_secs);
            if let (Some(dep), Some(arr)) = (first_dep, last_arr) {
                if arr > dep {
                    // Durakların SIRALI dizisini hash'le. Durak sayısı yeterince ayırt edici
                    // değildir: aynı sayıda durağa sahip farklı desenler karışabilir.
                    use std::hash::{Hash, Hasher};
                    let mut h = std::collections::hash_map::DefaultHasher::new();
                    for st in stops.iter() { idx.stop_id_of(st).hash(&mut h); }
                    let pattern_hash = h.finish();
                    let key = match trip_shape.get(trip_id) {
                        Some(&sh) => DurKey::ShapePattern(sh, pattern_hash),
                        None => DurKey::Pattern(route, pattern_hash),
                    };
                    groups.entry(key).or_default().push((trip_id, arr - dep, dep, route));
                }
            }
        }

        // Gün-içi (peak/off-peak) deseasonalize: süre, kalkış SAATİNE göre beklenenden
        // sapması ölçülür. Bir saatte ≥BUCKET_MIN_TRIPS sefer varsa o saatin medyanı
        // referans alınır; aksi halde desen-global medyanına düşülür (seyrek saatte
        // veri-açlığını önler). Robust-z artık residual üzerinde; min-dakika tabanı
        // tamamlayıcı (tiny-MAD'da küçük sapmaları işaretleme). Bkz. issue #24.
        const BUCKET_MIN_TRIPS: usize = 5;
        // Operasyonel min sapma: bundan küçük residual'lar (gün-içi gürültü) hiç işaretlenmez.
        // Deseasonalize residual'ları sıkıştırdığından σ tek başına aşırı hassas olur; bu taban
        // AND-gate olarak küçük sapmaları eler (issue #24 katkıcısının "min-dakika tamamlayıcı" notu).
        const RESIDUAL_FLOOR_SECS: f64 = 300.0;

        for (_key, durs) in groups {
            if durs.len() < 5 { continue; }
            let mut all: Vec<u32> = durs.iter().map(|&(_, d, _, _)| d).collect();
            all.sort_unstable();
            let med_all = all[all.len() / 2] as f64;
            if med_all < 120.0 { continue; }

            // Kaba zaman bandı (saatlik yerine): gün-içi yapıyı yakalar ama bucket'ları
            // veri-aç bırakmaz (katkıcının "coarse buckets degrade more gracefully" notu).
            let band = |dep: u32| -> u32 {
                // % 24: bandlama trip'in İLK kalkışına göredir. Bir feed gece yarısını aşan bir
                // seferin ilk kalkışını 24:xx+ (≥86400 sn) yazarsa (geçerli GTFS), ham saatle akşam
                // (band 4) yerine yanlış banda düşerdi; % 24 ile gece bandına (band 0) eşlenir.
                // NOT: normalize_service_day İLK durağı kaydırmaz (offset 0'dan başlar), o yüzden
                // bu yalnız feed'in kendisi 24:xx-ilk-kalkış yazdığında devreye girer (savunmacı;
                // Toei gibi 00:xx yazan feed'lerde no-op). Bkz. issue #26.
                match (dep / 3600) % 24 {
                    0..=5 => 0,    // gece / erken (24:xx, 25:xx… buraya döner)
                    6..=9 => 1,    // sabah zirve
                    10..=15 => 2,  // gündüz
                    16..=19 => 3,  // akşam zirve
                    _ => 4,        // akşam / gece (20:00–23:59)
                }
            };
            // Yoğun bandların (≥BUCKET_MIN_TRIPS) kendi medyanı; seyrek bandlar global'e düşer.
            let mut by_band: FxHashMap<u32, Vec<u32>> = FxHashMap::default();
            for &(_, d, dep, _) in &durs { by_band.entry(band(dep)).or_default().push(d); }
            let mut band_med: FxHashMap<u32, f64> = FxHashMap::default();
            for (b, mut bv) in by_band {
                if bv.len() >= BUCKET_MIN_TRIPS {
                    bv.sort_unstable();
                    band_med.insert(b, bv[bv.len() / 2] as f64);
                }
            }
            let reference = |dep: u32| band_med.get(&band(dep)).copied().unwrap_or(med_all);

            // residual = süre − saat-bazlı beklenen; MAD residual üzerinde.
            let mut res: Vec<f64> = durs.iter().map(|&(_, d, dep, _)| d as f64 - reference(dep)).collect();
            res.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
            let med_res = res[res.len() / 2];
            let mut dev: Vec<f64> = res.iter().map(|r| (r - med_res).abs()).collect();
            dev.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
            let mad_r = dev[dev.len() / 2] * 1.4826;

            // Residual-z eşiği (config; varsayılan 3.5 — issue #24, deseasonalize residual'ı sıkı).
            let sigma = config.duration_outlier_sigma;
            for &(trip_id, dur, dep_secs, route) in &durs {
                let ref_med = reference(dep_secs);
                let residual = dur as f64 - ref_med;
                // İki koşul birlikte (AND): (1) operasyonel olarak anlamlı sapma (≥ taban),
                // (2) istatistiksel aykırılık (residual > σ·MAD). mad_r≈0 ise (residual'lar
                // birebir aynı) σ testi atlanır; yalnızca taban karar verir.
                let mag_ok = residual.abs() >= RESIDUAL_FLOOR_SECS;
                let stat_ok = mad_r < 1.0 || residual.abs() > sigma * mad_r;
                if !(mag_ok && stat_ok) { continue; }

                let label = route_label.get(route).copied().unwrap_or(route);
                let dep_suffix = format!(" {:02}:{:02} kalkışlı", dep_secs / 3600, (dep_secs % 3600) / 60);
                let dur_min = dur / 60;
                let ref_min = (ref_med as u32) / 60;
                let yon = if residual >= 0.0 { "beklenenden uzun" } else { "beklenenden kısa" };
                let sigma_str = if mad_r >= 30.0 {
                    let z = residual / (mad_r / 1.4826);
                    format!("{z:+.1}σ, {yon}")
                } else {
                    format!("{:+}dk, {yon}", (residual / 60.0).round() as i64)
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
                    Some(format!("{dur_min}dk (saat-bazlı beklenen {ref_min}dk)")),
                    None,
                    format!("'{label}' hattının '{trip_id}'{dep_suffix} seferinin süresi {dur_min}dk — bu saatte beklenen ~{ref_min}dk ({sigma_str})."),
                    "stop_times.txt zaman değerlerini ve sefer güzergahını doğrulayın.",
                ));
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
            let route = ti_vat.route_id(trip);
            let svc = ti_vat.service_id(trip);
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
                let a = idx.stop_id_of(&w[0]);
                let b = idx.stop_id_of(&w[1]);
                if a.is_empty() || b.is_empty() { continue; }
                adj.entry(a).or_default().insert(b);
                adj.entry(b).or_default().insert(a);
            }
        }
        let total = adj.len();
        if total >= 5 {
            let mut visited: FxHashSet<&str> = FxHashSet::default();
            let mut comp_nodes: Vec<Vec<&str>> = Vec::new();
            let all_nodes: Vec<&str> = adj.keys().copied().collect();

            for &start in &all_nodes {
                if visited.contains(start) { continue; }
                let mut nodes: Vec<&str> = Vec::new();
                let mut stack = vec![start];
                visited.insert(start);
                while let Some(node) = stack.pop() {
                    nodes.push(node);
                    if let Some(neighbors) = adj.get(node) {
                        for &nb in neighbors {
                            if visited.insert(nb) {
                                stack.push(nb);
                            }
                        }
                    }
                }
                comp_nodes.push(nodes);
            }

            if comp_nodes.len() > 1 {
                let main_size = comp_nodes.iter().map(|c| c.len()).max().unwrap_or(0);
                let small_thresh = (total / 20).max(2);

                // Durak koordinatları: parent_station'sız feed'lerde aynı fiziksel yer
                // (ör. otogar peronları) ayrı stop_id'lere bölünür → sahte izole bileşen.
                // Bir izole adayın herhangi bir durağı ana şebekedeki bir durağa bu mesafeden
                // yakınsa fiziksel olarak aynı ağ kabul edilir, izole sayılmaz (yanlış pozitif önleme).
                let mut stop_coords: FxHashMap<&str, (f64, f64)> = FxHashMap::default();
                for s in &records.stops {
                    if let Some(c) = s.stop_lat.zip(s.stop_lon) {
                        stop_coords.insert(s.stop_id.as_str(), c);
                    }
                }
                let main_idx = comp_nodes.iter().enumerate()
                    .max_by_key(|(_, c)| c.len())
                    .map(|(i, _)| i)
                    .unwrap_or(0);
                let main_coords: Vec<(f64, f64)> = comp_nodes[main_idx].iter()
                    .filter_map(|s| stop_coords.get(s).copied())
                    .collect();
                const VAT005_MERGE_M: f64 = 200.0;

                // İzole bileşenler: hem küçük hem ana şebekeden coğrafi olarak da kopuk.
                let isolated_comps: Vec<&Vec<&str>> = comp_nodes.iter()
                    .filter(|c| c.len() < main_size && c.len() <= small_thresh)
                    .filter(|c| !c.iter().any(|s| {
                        stop_coords.get(s).map_or(false, |&(la, lo)| {
                            main_coords.iter().any(|&(mla, mlo)|
                                haversine_km(la, lo, mla, mlo) * 1000.0 <= VAT005_MERGE_M)
                        })
                    }))
                    .collect();

                if !isolated_comps.is_empty() {
                    // İzole bileşenlere ait tüm durakları topla (maks 200)
                    let mut isolated_all: Vec<&str> = Vec::new();
                    'outer: for c in &isolated_comps {
                        for &node in c.iter() {
                            isolated_all.push(node);
                            if isolated_all.len() >= 200 { break 'outer; }
                        }
                    }

                    let isolated_count = isolated_all.len();
                    let comp_count = isolated_comps.len();
                    let example = isolated_comps[0][0];
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
                        Some(format!("{isolated_count} / {comp_count}")),
                        None,
                        format!("Ağ grafında ana şebekeden (seferlerle bağlı en büyük durak kümesi) kopuk {comp_count} izole durak kümesi var ({isolated_count} durak) — bu duraklar kendi aralarında seferlerle bağlı ama ana şebekeye hiçbir seferle bağlanmıyor. Örnek: '{ex_name}' ('{example}')."),
                        "Ayrı bir servis bölgesi değilse, ortak durakların başka rotalarla aynı stop_id'yi paylaştığını veya bağlantı seferlerinin tanımlı olduğunu kontrol edin (sık neden: aynı yerin platform-özel ayrı stop_id'lerle modellenmesi).",
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
                let sid = idx.stop_id_of(first);
                if !sid.is_empty() {
                    terminus_routes.entry(sid).or_default().insert(route);
                }
            }
            if stops.len() > 1 {
                if let Some(last) = stops.last() {
                    let sid = idx.stop_id_of(last);
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
                Some(format!("{}", routes.len())),
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

    // OPR_024: Bir hat TEK çalışma takviminde (route_id + service_id) çok fazla sefer içeriyor.
    // Eşik route-toplam değil service-başına uygulanır: haftaya yayılan normal sefer sayısı
    // (7 gün × günlük sefer) yanlış-pozitif vermez; gerçek "veri birleştirme" tek takvimde patlar.
    {
        let route_short_o24: HashMap<&str, &str> = records.routes.iter()
            .map(|r| (r.route_id.as_str(), r.route_short_name.as_deref().filter(|s| !s.is_empty()).unwrap_or(r.route_id.as_str())))
            .collect();
        let mut rs_trip_counts: HashMap<(&str, &str), u32> = HashMap::new();
        for t in &records.trips {
            if !ti_vat.route_id(t).is_empty() {
                *rs_trip_counts.entry((ti_vat.route_id(t), ti_vat.service_id(t))).or_default() += 1;
            }
        }
        for ((route_id, service_id), count) in &rs_trip_counts {
            if *count > config.max_trips_per_route {
                let mut n = k6_notice(
                    ctr, "OPR_024", EntityType::Route,
                    Some((*route_id).to_string()), Some((*route_id).to_string()),
                    "trips.txt", None, None,
                    Some(count.to_string()), Some(format!("≤{}", config.max_trips_per_route)),
                    format!("'{}' kodlu hat '{service_id}' çalışma takviminde {count} sefer içeriyor — veri birleştirme sorunu olabilir.", route_short_o24.get(*route_id).copied().unwrap_or(*route_id)),
                    "Bu seferlerin doğru route_id ve service_id'ye atandığını kontrol edin.",
                );
                n.service_id = Some((*service_id).to_string());
                notices.push(n);
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
                if let Some((h, m, s)) = st.departure_time() {
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
                if let Some(shape_id) = ti_vat.shape_id(t) {
                    if !shape_id.is_empty() && !ti_vat.route_id(t).is_empty() {
                        shape_routes.entry(shape_id).or_default().insert(ti_vat.route_id(t));
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

    #[test]
    fn max_speed_kmh_maps_extended_route_types() {
        // #15/MD: genişletilmiş Avrupa route_type'ları doğru kategoriye eşlenmeli.
        let cfg = ValidatorConfig::default();
        assert_eq!(max_speed_kmh(100, &cfg), cfg.max_speed_rail_kmh, "100 (railway ext) = rail");
        assert_eq!(max_speed_kmh(109, &cfg), cfg.max_speed_rail_kmh, "109 (S-Bahn) = rail");
        assert_eq!(max_speed_kmh(106, &cfg), cfg.max_speed_rail_kmh, "106 (bölgesel rail) = rail");
        assert_eq!(max_speed_kmh(400, &cfg), cfg.max_speed_metro_kmh, "400 (U-Bahn ext) = metro");
        assert_eq!(max_speed_kmh(900, &cfg), cfg.max_speed_tram_kmh, "900 (tram ext) = tram");
        assert_eq!(max_speed_kmh(700, &cfg), cfg.max_speed_bus_kmh, "700 (bus ext) = bus");
        assert_eq!(max_speed_kmh(2, &cfg), cfg.max_speed_rail_kmh, "standart 2 hâlâ rail");
        assert_eq!(max_speed_kmh(3, &cfg), cfg.max_speed_bus_kmh, "standart 3 hâlâ bus");
        // Asıl bug: rail extended kodları bus eşiği ALMAMALI.
        assert_ne!(max_speed_kmh(100, &cfg), cfg.max_speed_bus_kmh, "rail ext bus eşiği almamalı");
    }

    use crate::k2::routes::RouteRecord;
    use crate::k2::stop_times::StopTimeRecord;
    use crate::k2::stops::StopRecord;
    use crate::k2::trips::TripRecord;
    use crate::k3_entity_graph::EntityMap;

    fn default_config() -> ValidatorConfig {
        ValidatorConfig::default()
    }

    // Regresyon: contains_as_word çok-baytlı UTF-8'de (CJK) char sınırı içinde
    // dilimleme yapıp panik atmamalı. Eskiden `start = pos + 1` Japonca feed'lerde
    // (ör. "市バス１号系統") wasm worker'ını çökertiyordu.
    #[test]
    fn contains_as_word_handles_multibyte_utf8() {
        // Panik atmadan tamamlanmalı (asıl regresyon koşulu).
        assert!(!contains_as_word("市バス１号系統", "bus"));
        assert!(!contains_as_word("六本木ヒルズ前", "ab"));
        assert!(!contains_as_word("東京", ""));
        // ASCII davranışı korunmalı.
        assert!(contains_as_word("Route 5 Express", "5"));
        assert!(!contains_as_word("Route 51", "5"));
        // CJK gövdesi içinde gömülü ASCII kelimesi de sınırda bulunmalı.
        assert!(contains_as_word("市バス bus 系統", "bus"));
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
            route_cemv_support: None, jp_office_id: None, row: Default::default(), line: 2,
        }
    }

    use crate::k2::trips::TripInternTable;
    use smol_str::SmolStr;

    // Thread-local TripInternTable: test yardımcıları stringleri buraya intern eder.
    // Her test kendi iş parçacığında çalıştığından izolasyon otomatik sağlanır.
    thread_local! {
        static TEST_TI: std::cell::RefCell<TripInternTable> =
            std::cell::RefCell::new(TripInternTable::new());
    }

    /// Birikmiş TripInternTable'ı al ve yerine yeni boş tablo koy.
    fn take_ti() -> TripInternTable {
        TEST_TI.with(|cell| {
            std::mem::replace(&mut *cell.borrow_mut(), TripInternTable::new())
        })
    }

    /// Temel sefer kaydı: route_id + service="SVC" thread-local intern tablosuna eklenir.
    fn trip(trip_id: &str, route_id: &str) -> TripRecord {
        TEST_TI.with(|cell| {
            let mut ti = cell.borrow_mut();
            let ri = ti.route_ids.len() as u32;
            ti.route_ids.push(SmolStr::new(route_id));
            let si = ti.service_ids.len() as u32;
            ti.service_ids.push(SmolStr::new("SVC"));
            TripRecord {
                trip_id: trip_id.into(),
                route_idx: ri, service_idx: si, shape_idx: 0,
                headsign_idx: 0, short_name_idx: 0, block_idx: 0, jp_office_idx: 0,
                direction_id: None, wheelchair_accessible: None,
                bikes_allowed: None, cars_allowed: None,
                safe_duration_factor: None, safe_duration_offset: None,
                line: 2,
            }
        })
    }

    /// Sefer + shape_id (thread-local intern tablosuna eklenir).
    fn trip_sh(trip_id: &str, route_id: &str, shape_id: &str) -> TripRecord {
        let mut t = trip(trip_id, route_id);
        if !shape_id.is_empty() {
            TEST_TI.with(|cell| {
                let mut ti = cell.borrow_mut();
                let shi = ti.shape_ids.len() as u32;
                ti.shape_ids.push(SmolStr::new(shape_id));
                t.shape_idx = shi;
            });
        }
        t
    }

    /// Sefer + trip_headsign (thread-local intern tablosuna eklenir).
    fn trip_hs(trip_id: &str, route_id: &str, headsign: &str) -> TripRecord {
        let mut t = trip(trip_id, route_id);
        TEST_TI.with(|cell| {
            let mut ti = cell.borrow_mut();
            let hi = ti.headsigns.len() as u32;
            ti.headsigns.push(SmolStr::new(headsign));
            t.headsign_idx = hi;
        });
        t
    }

    /// Sefer + block_id (thread-local intern tablosuna eklenir).
    fn trip_blk(trip_id: &str, route_id: &str, block_id: &str) -> TripRecord {
        let mut t = trip(trip_id, route_id);
        if !block_id.is_empty() {
            TEST_TI.with(|cell| {
                let mut ti = cell.borrow_mut();
                let bi = ti.block_ids.len() as u32;
                ti.block_ids.push(SmolStr::new(block_id));
                t.block_idx = bi;
            });
        }
        t
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

    /// records_with: trip intern tablosunu thread-local'dan otomatik alır (take_ti).
    fn records_with(
        stops: Vec<StopRecord>,
        routes: Vec<RouteRecord>,
        trips: Vec<TripRecord>,
        stoptimes: Vec<StopTimeRecord>,
    ) -> crate::k2::EntityRecords {
        use crate::k2::stop_times::StopTimesIndex;
        let ti = take_ti();
        let mut r = crate::k2::EntityRecords::default();
        r.stops = stops;
        r.routes = routes;
        r.trips = trips;
        r.trip_interns = ti;
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

    // Toplulaştırma (Güvenilirlik Sprint'i Checkpoint 6): aynı fiziksel segment o
    // segmentten geçen her seferde ayrı notice üretiyordu (TriMet'te tek segment 604×).
    #[test]
    fn stm_014_aggregates_same_segment_across_trips() {
        // Aynı hat/yön, AYNI A→B segmenti, 3 sefer → 3 değil 1 notice.
        let records = records_with(
            vec![stop("A", 41.0, 29.0), stop("B", 39.9, 32.9)],
            vec![route("R1", 3)],
            vec![trip("T1", "R1"), trip("T2", "R1"), trip("T3", "R1")],
            vec![
                stoptime("T1", 1, "A", (8, 0, 0), (8, 0, 0), 2),
                stoptime("T1", 2, "B", (10, 0, 0), (10, 0, 0), 3),   // ~175 km/h
                stoptime("T2", 1, "A", (9, 0, 0), (9, 0, 0), 4),
                stoptime("T2", 2, "B", (11, 30, 0), (11, 30, 0), 5), // ~140 km/h (daha yavaş)
                stoptime("T3", 1, "A", (12, 0, 0), (12, 0, 0), 6),
                stoptime("T3", 2, "B", (14, 0, 0), (14, 0, 0), 7),   // ~175 km/h
            ],
        );
        let result = analyze(&records, &empty_derived(), &default_config(), 20260514);
        let hits: Vec<_> = result.notices.iter().filter(|n| n.rule_id == "STM_014").collect();
        assert_eq!(hits.len(), 1, "tek segment → tek notice, alınan: {}", hits.len());

        let d = hits[0].details.as_ref().expect("details olmalı");
        assert_eq!(d.get("trip_count").map(String::as_str), Some("3"), "3 sefer sayılmalı");
        assert_eq!(d.get("stop_a").map(String::as_str), Some("A"));
        assert_eq!(d.get("stop_b").map(String::as_str), Some("B"));
        // Örnek sefer listesi deterministik ve sıralı
        assert_eq!(d.get("trips").map(String::as_str), Some("T1,T2,T3"));
        // Hız aralığı en yavaş/en hızlı seferi kapsamalı (tek değere çökmemeli)
        let (lo, hi) = (
            d["speed_min"].parse::<f64>().unwrap(),
            d["speed_max"].parse::<f64>().unwrap(),
        );
        assert!(lo < hi, "hız aralığı olmalı, alınan {lo}-{hi}");
        // scope_key hat bazlı filtre için route_id kalmalı
        assert_eq!(hits[0].scope_key.as_deref(), Some("R1"));
    }

    #[test]
    fn stm_014_keeps_distinct_segments_separate() {
        // Aynı hatta İKİ ayrı bozuk segment (A→B ve B→C) → dedup=Entity onları
        // yutmamalı; entity_id bileşik olduğu için 2 notice çıkmalı.
        let records = records_with(
            vec![stop("A", 41.0, 29.0), stop("B", 39.9, 32.9), stop("C", 41.0, 29.0)],
            vec![route("R1", 3)],
            vec![trip("T1", "R1")],
            vec![
                stoptime("T1", 1, "A", (8, 0, 0), (8, 0, 0), 2),
                stoptime("T1", 2, "B", (10, 0, 0), (10, 0, 0), 3), // A→B ~175 km/h
                stoptime("T1", 3, "C", (12, 0, 0), (12, 0, 0), 4), // B→C ~175 km/h
            ],
        );
        let result = analyze(&records, &empty_derived(), &default_config(), 20260514);
        let hits: Vec<_> = result.notices.iter().filter(|n| n.rule_id == "STM_014").collect();
        assert_eq!(hits.len(), 2, "iki farklı segment ayrı kalmalı, alınan: {}", hits.len());
        let ids: Vec<_> = hits.iter().filter_map(|n| n.entity_id.as_deref()).collect();
        assert_ne!(ids[0], ids[1], "entity_id'ler benzersiz olmalı (dedup yutmasın): {ids:?}");
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

    #[test]
    fn opt_in_stop_name_profile_emits_stp_040_and_041() {
        let mut parent = stop("P", 41.0, 29.0);
        parent.stop_name = Some("Kadikoy".into()); parent.location_type = Some(1);
        let mut child = stop("C", 41.0, 29.0);
        child.stop_name = Some("Platform Stop".into());
        child.row.insert("parent_station".into(), "P".into());
        let records = records_with(vec![parent, child], vec![], vec![], vec![]);
        let mut cfg = default_config(); cfg.stop_name_best_practices = true;
        let result = analyze(&records, &empty_derived(), &cfg, 20260514);
        assert!(result.notices.iter().any(|n| n.rule_id == "STP_040"));
        assert!(result.notices.iter().any(|n| n.rule_id == "STP_041"));
    }

    #[test]
    fn source_url_without_zip_filename_emits_arc_028() {
        let records = records_with(vec![], vec![], vec![], vec![]);
        let mut cfg = default_config(); cfg.source_url = Some("https://example.org/gtfs".into());
        let result = analyze(&records, &empty_derived(), &cfg, 20260514);
        assert!(result.notices.iter().any(|n| n.rule_id == "ARC_028"));
        cfg.source_url = Some("https://example.org/feed.zip?rev=2".into());
        let result = analyze(&records, &empty_derived(), &cfg, 20260514);
        assert!(!result.notices.iter().any(|n| n.rule_id == "ARC_028"));
    }
    #[test]
    fn three_consecutive_equal_times_produce_stm_053() {
        let records = records_with(
            vec![stop("A",41.0,29.0), stop("B",41.001,29.0), stop("C",41.002,29.0)],
            vec![route("R1",3)], vec![trip("T1","R1")],
            vec![
                stoptime("T1",1,"A",(8,0,0),(8,0,0),2),
                stoptime("T1",2,"B",(8,0,0),(8,0,0),3),
                stoptime("T1",3,"C",(8,0,0),(8,0,0),4),
            ]);
        let result = analyze(&records, &empty_derived(), &default_config(), 20260514);
        assert!(result.notices.iter().any(|n| n.rule_id == "STM_053"));
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

    // ── STM_012: tam-dakika sıfır süre — eşik rota tipine bağlı ───────────────
    // Her iki zaman da tam dakikaysa gerçek geçiş süresi bilinmez (0-59 sn). "İmkânsız"
    // diyebilmek için mesafe, tam bir dakika geçse bile azami hızı aşmalı → max_speed/60.
    // Corpus kanıtı (mdb-2155): sabit 1.0 km eşiği 42 bulgunun 41'ini FP yapıyordu.

    #[test]
    fn whole_minute_zero_gap_below_one_minute_reach_is_not_stm_012() {
        // ~1.1 km / otobüs: tam dakikada 66 km/h eder — olağan, eşik 120 → SUSMALI.
        let records = records_with(
            vec![stop("A", 41.0, 29.0), stop("B", 41.0099, 29.0)],
            vec![route("R1", 3)], // bus → 120 km/h → imkânsızlık sınırı 2.0 km
            vec![trip("T1", "R1")],
            vec![
                stoptime("T1", 1, "A", (8, 0, 0), (8, 0, 0), 2),
                stoptime("T1", 2, "B", (8, 0, 0), (8, 0, 0), 3),
            ],
        );
        let result = analyze(&records, &empty_derived(), &default_config(), 20260514);
        let hits: Vec<_> = result.notices.iter().filter(|n| n.rule_id == "STM_012").collect();
        assert!(hits.is_empty(), "dakika yuvarlaması ile açıklanabilen mesafe FP olmamalı: {:?}",
                hits.iter().map(|n| &n.observed_value).collect::<Vec<_>>());
    }

    #[test]
    fn whole_minute_zero_gap_beyond_one_minute_reach_is_stm_012() {
        // ~29 km / otobüs: tam dakikada bile ~1740 km/h gerekir → GERÇEK hata, fire etmeli.
        // (mdb-2155'te MD'nin de bulduğu tek vaka bu sınıftı.)
        let records = records_with(
            vec![stop("A", 41.0, 29.0), stop("B", 41.26, 29.0)],
            vec![route("R1", 3)],
            vec![trip("T1", "R1")],
            vec![
                stoptime("T1", 1, "A", (8, 0, 0), (8, 0, 0), 2),
                stoptime("T1", 2, "B", (8, 0, 0), (8, 0, 0), 3),
            ],
        );
        let result = analyze(&records, &empty_derived(), &default_config(), 20260514);
        assert!(result.notices.iter().any(|n| n.rule_id == "STM_012"),
                "bir dakikada alınamayacak mesafe STM_012 üretmeli");
    }

    #[test]
    fn whole_minute_zero_gap_threshold_follows_route_type() {
        // Aynı ~1.1 km mesafe, teleferik (30 km/h → sınır 0.5 km) için İMKÂNSIZ.
        // Eşik sabit olsaydı bu vaka kaçardı; rota tipine bağlı olduğu için yakalanır.
        let records = records_with(
            vec![stop("A", 41.0, 29.0), stop("B", 41.0099, 29.0)],
            vec![route("R1", 6)], // teleferik
            vec![trip("T1", "R1")],
            vec![
                stoptime("T1", 1, "A", (8, 0, 0), (8, 0, 0), 2),
                stoptime("T1", 2, "B", (8, 0, 0), (8, 0, 0), 3),
            ],
        );
        let result = analyze(&records, &empty_derived(), &default_config(), 20260514);
        assert!(result.notices.iter().any(|n| n.rule_id == "STM_012"),
                "teleferik için 1.1 km bir dakikada alınamaz → STM_012 olmalı");
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

    #[test]
    fn is_regular_headway_distinguishes_rural_from_gap() {
        // Düzenli seyrek (kırsal): eşit aralıklar → düzenli (true) → OPR_001 bastırılır
        assert!(is_regular_headway(&[18000, 18000, 18000]), "5sa eşit aralık = düzenli");
        assert!(is_regular_headway(&[36000]), "tek aralık (2 sefer) = kasıtlı");
        assert!(is_regular_headway(&[]), "boş = kasıtlı");
        // Düzensiz (gerçek boşluk): çoğu sık + bir dev boşluk → düzensiz (false) → OPR_001 üretilir
        assert!(!is_regular_headway(&[900, 900, 900, 14400]), "15dk×3 + 4sa boşluk = düzensiz");
        assert!(!is_regular_headway(&[600, 600, 7200]), "10dk×2 + 2sa = düzensiz");
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
        // DÜZENSIZ büyük boşluk: 08:00/08:15/08:30 sık, sonra 13:00 → aralıklar [15,15,270]dk
        // yüksek CV → gerçek servis boşluğu → OPR_001 (kırsal otomatik bastırma DEVREYE GİRMEZ).
        let records = records_with(
            vec![stop("A", 41.0, 29.0), stop("B", 41.01, 29.01)],
            vec![route("R1", 3)],
            vec![trip("T1", "R1"), trip("T2", "R1"), trip("T3", "R1"), trip("T4", "R1")],
            vec![
                stoptime("T1", 1, "A", (8, 0, 0), (8, 0, 0), 2),
                stoptime("T1", 2, "B", (8, 10, 0), (8, 10, 0), 3),
                stoptime("T2", 1, "A", (8, 15, 0), (8, 15, 0), 4),
                stoptime("T2", 2, "B", (8, 25, 0), (8, 25, 0), 5),
                stoptime("T3", 1, "A", (8, 30, 0), (8, 30, 0), 6),
                stoptime("T3", 2, "B", (8, 40, 0), (8, 40, 0), 7),
                stoptime("T4", 1, "A", (13, 0, 0), (13, 0, 0), 8),
                stoptime("T4", 2, "B", (13, 10, 0), (13, 10, 0), 9),
            ],
        );
        let result = analyze(&records, &empty_derived(), &default_config(), 20260514);
        assert!(result.notices.iter().any(|n| n.rule_id == "OPR_001"));
    }

    #[test]
    fn regular_sparse_headway_suppresses_opr_001() {
        // Düzenli seyrek (kırsal): 06:00/11:00/16:00 → eşit 300dk aralık → düşük CV → kasıtlı
        // servis → OPR_001 ÜRETİLMEZ, eşik aşılsa bile (yanlış-pozitif önlenir).
        let records = records_with(
            vec![stop("A", 41.0, 29.0), stop("B", 41.01, 29.01)],
            vec![route("R1", 3)],
            vec![trip("T1", "R1"), trip("T2", "R1"), trip("T3", "R1")],
            vec![
                stoptime("T1", 1, "A", (6, 0, 0), (6, 0, 0), 2),
                stoptime("T1", 2, "B", (6, 10, 0), (6, 10, 0), 3),
                stoptime("T2", 1, "A", (11, 0, 0), (11, 0, 0), 4),
                stoptime("T2", 2, "B", (11, 10, 0), (11, 10, 0), 5),
                stoptime("T3", 1, "A", (16, 0, 0), (16, 0, 0), 6),
                stoptime("T3", 2, "B", (16, 10, 0), (16, 10, 0), 7),
            ],
        );
        let result = analyze(&records, &empty_derived(), &default_config(), 20260514);
        assert!(!result.notices.iter().any(|n| n.rule_id == "OPR_001"), "düzenli seyrek bastırılmalı");
    }

    #[test]
    fn departure_before_arrival_produces_stm_007() {
        // Gerçek satır-içi hata: B durağında varış 09:00 ama kalkış 08:00 → STM_007.
        let records = records_with(
            vec![stop("A", 41.0, 29.0), stop("B", 41.01, 29.01)],
            vec![route("R1", 3)],
            vec![trip("T1", "R1")],
            vec![
                stoptime("T1", 1, "A", (8, 0, 0), (8, 0, 0), 2),
                stoptime("T1", 2, "B", (9, 0, 0), (8, 0, 0), 3),
            ],
        );
        let result = analyze(&records, &empty_derived(), &default_config(), 20260514);
        assert!(result.notices.iter().any(|n| n.rule_id == "STM_007"));
    }

    #[test]
    fn midnight_departure_suppresses_stm_007() {
        // Gece yarısı: son durakta varış 23:58, kalkış 00:00 (24:00 yazılmalıydı) → bastırılır.
        let records = records_with(
            vec![stop("A", 41.0, 29.0), stop("B", 41.01, 29.01)],
            vec![route("R1", 3)],
            vec![trip("T1", "R1")],
            vec![
                stoptime("T1", 1, "A", (23, 50, 0), (23, 50, 0), 2),
                stoptime("T1", 2, "B", (23, 58, 0), (0, 0, 0), 3),
            ],
        );
        let result = analyze(&records, &empty_derived(), &default_config(), 20260514);
        assert!(!result.notices.iter().any(|n| n.rule_id == "STM_007"), "gece-yarısı kalkış bastırılmalı");
    }

    #[test]
    fn stm045_threshold_respects_service_day_start_hour() {
        // 26:30 kalkış: default config (service_day_start_hour=3 → eşik 27h) altında MEŞRU.
        let records = records_with(
            vec![stop("A", 41.0, 29.0), stop("B", 41.01, 29.01)],
            vec![route("R1", 3)],
            vec![trip("T1", "R1")],
            vec![
                stoptime("T1", 1, "A", (26, 30, 0), (26, 30, 0), 2),
                stoptime("T1", 2, "B", (26, 40, 0), (26, 40, 0), 3),
            ],
        );
        let r = analyze(&records, &empty_derived(), &default_config(), 20260514);
        assert!(!r.notices.iter().any(|n| n.rule_id == "STM_045"),
            "26:30 default eşik (27h) altında STM_045 üretmemeli");

        // start_hour=0 (normalizasyon kapalı) → eşik 26h → 26:30 anomali.
        let mut cfg0 = default_config();
        cfg0.service_day_start_hour = 0;
        let r0 = analyze(&records, &empty_derived(), &cfg0, 20260514);
        assert!(r0.notices.iter().any(|n| n.rule_id == "STM_045"),
            "start_hour=0 iken 26:30 STM_045 üretmeli");

        // 28:30 → default eşik (27h) üstünde → anomali.
        let records2 = records_with(
            vec![stop("A", 41.0, 29.0), stop("B", 41.01, 29.01)],
            vec![route("R1", 3)],
            vec![trip("T2", "R1")],
            vec![
                stoptime("T2", 1, "A", (28, 30, 0), (28, 30, 0), 2),
                stoptime("T2", 2, "B", (28, 40, 0), (28, 40, 0), 3),
            ],
        );
        let r2 = analyze(&records2, &empty_derived(), &default_config(), 20260514);
        assert!(r2.notices.iter().any(|n| n.rule_id == "STM_045"),
            "28:30 default eşik (27h) üstünde STM_045 üretmeli");
    }

    #[test]
    fn wheelchair_boarding_completeness_stp_037_038() {
        let mk = |stops: Vec<StopRecord>| records_with(
            stops,
            vec![route("R1", 3)],
            vec![trip("T1", "R1")],
            vec![
                stoptime("T1", 1, "A", (8, 0, 0), (8, 0, 0), 2),
                stoptime("T1", 2, "B", (8, 10, 0), (8, 10, 0), 3),
            ],
        );
        let mut a = stop("A", 41.0, 29.0);
        let mut b = stop("B", 41.01, 29.01);

        // Tüm fiziksel duraklar eksik (wheelchair_boarding None) → STP_038, STP_037 yok.
        let r = analyze(&mk(vec![a.clone(), b.clone()]), &empty_derived(), &default_config(), 20260514);
        assert!(r.notices.iter().any(|n| n.rule_id == "STP_038"), "hepsi eksik → STP_038");
        assert!(!r.notices.iter().any(|n| n.rule_id == "STP_037"));

        // Biri dolu, biri eksik → STP_037, STP_038 yok.
        a.wheelchair_boarding = Some(1);
        let r2 = analyze(&mk(vec![a.clone(), b.clone()]), &empty_derived(), &default_config(), 20260514);
        assert!(r2.notices.iter().any(|n| n.rule_id == "STP_037"), "kısmi → STP_037");
        assert!(!r2.notices.iter().any(|n| n.rule_id == "STP_038"));

        // Hepsi dolu → ikisi de yok.
        b.wheelchair_boarding = Some(2);
        let r3 = analyze(&mk(vec![a, b]), &empty_derived(), &default_config(), 20260514);
        assert!(!r3.notices.iter().any(|n| n.rule_id == "STP_037" || n.rule_id == "STP_038"),
            "hepsi dolu → notice yok");
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
        TEST_TI.with(|cell| {
            let mut ti = cell.borrow_mut();
            let ri = ti.route_ids.len() as u32;
            ti.route_ids.push(SmolStr::new(route_id));
            let si = ti.service_ids.len() as u32;
            ti.service_ids.push(SmolStr::new(service_id));
            TripRecord {
                trip_id: trip_id.into(),
                route_idx: ri, service_idx: si, shape_idx: 0,
                headsign_idx: 0, short_name_idx: 0, block_idx: 0, jp_office_idx: 0,
                direction_id: None, wheelchair_accessible: None,
                bikes_allowed: None, cars_allowed: None,
                safe_duration_factor: None, safe_duration_offset: None,
                line: 2,
            }
        })
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
        // Tek servis süresi dolmuş → bilgi seviyesi CAL_013, KRİTİK CAL_009 değil.
        // CAL_013 birleşik active_dates'e göre hesaplanır (gerçek boru hattında k5 doldurur).
        use crate::k5_derived::CalendarBitmap;
        let mut derived = DerivedData::default();
        derived.calendar_bitmap = CalendarBitmap {
            active_dates: [("SVC".to_string(),
                [20251201u32, 20251215, 20251231].into_iter().collect())]
                .into_iter().collect(),
        };
        let mut records = crate::k2::EntityRecords::default();
        records.calendars = vec![cal_rec("SVC", all_days(), (2025, 1, 1), (2025, 12, 31))];
        // today = 20260514 > son aktif tarih 20251231
        let result = analyze(&records, &derived, &default_config(), 20260514);
        assert!(result.notices.iter().any(|n| n.rule_id == "CAL_013"),
            "süresi dolmuş tekil servis CAL_013 üretmeli");
        assert!(!result.notices.iter().any(|n| n.rule_id == "CAL_009"),
            "tek servis durumunda k6'dan CAL_009 üretmemeli (k4 tümü dolmuşsa atar)");
    }

    fn pth_stop(id: &str, lt: Option<u32>, parent: &str) -> StopRecord {
        let mut s = stop(id, 0.0, 0.0);
        s.location_type = lt;
        if !parent.is_empty() {
            s.row.insert("parent_station".to_string(), parent.to_string());
        }
        s
    }
    fn pth_pw(id: &str, from: &str, to: &str) -> crate::k2::pathways::PathwayRecord {
        crate::k2::pathways::PathwayRecord {
            pathway_id: id.into(), from_stop_id: from.into(), to_stop_id: to.into(),
            pathway_mode: Some(1), is_bidirectional: Some(1),
            length: None, traversal_time: None, stair_count: None,
            max_slope: None, min_width: None, row: Default::default(), line: 2,
        }
    }

    #[test]
    fn pth_012_nested_platform_reachable_via_boarding_area_no_fp() {
        // #50: station ST → platform PL(lt0) → boarding area BA(lt4); entrance EN(lt2).
        // Pathway EN↔BA. Peron-record PL grafikte düğüm DEĞİL ama çocuğu BA
        // entrance'tan erişilebilir → PTH_012 sahte-pozitif ÇIKMAMALI.
        let mut recs = crate::k2::EntityRecords::default();
        recs.stops = vec![
            pth_stop("ST", Some(1), ""),
            pth_stop("EN", Some(2), "ST"),
            pth_stop("PL", Some(0), "ST"),
            pth_stop("BA", Some(4), "PL"),
        ];
        recs.pathways = vec![pth_pw("P1", "EN", "BA")];
        let mut derived = DerivedData::default();
        derived.pathway_graph.adjacency.insert("EN".into(), vec![("BA".into(), 0)]);
        derived.pathway_graph.adjacency.insert("BA".into(), vec![("EN".into(), 0)]);
        let mut v = Vec::new();
        let mut c = 0u32;
        check_pathway_analytics(&recs, &derived, &mut v, &mut c);
        assert!(!v.iter().any(|n| n.rule_id == "PTH_012"),
            "boarding-area çocuğu erişilebilirken PTH_012 çıkmamalı");
    }

    #[test]
    fn pth_012_genuinely_unreachable_platform_fires() {
        // Peron PL grafikte düğüm ama entrance'tan erişilemiyor ve lt4 çocuğu yok
        // → PTH_012 ÇIKMALI (gerçek erişilebilirlik boşluğu korunur).
        let mut recs = crate::k2::EntityRecords::default();
        recs.stops = vec![
            pth_stop("ST", Some(1), ""),
            pth_stop("EN", Some(2), "ST"),
            pth_stop("PL", Some(0), "ST"),
        ];
        recs.pathways = vec![pth_pw("P1", "EN", "X"), pth_pw("P2", "PL", "Y")];
        let mut derived = DerivedData::default();
        derived.pathway_graph.adjacency.insert("EN".into(), vec![("X".into(), 0)]);
        derived.pathway_graph.adjacency.insert("PL".into(), vec![("Y".into(), 0)]);
        let mut v = Vec::new();
        let mut c = 0u32;
        check_pathway_analytics(&recs, &derived, &mut v, &mut c);
        assert!(v.iter().any(|n| n.rule_id == "PTH_012"),
            "erişilemeyen peron PTH_012 üretmeli");
    }

    #[test]
    fn pth_013_nested_platform_accessible_via_boarding_area_no_fp() {
        // #51: iç içe peron (PL lt0) → boarding area (BA lt4); EN→BA erişilebilir
        // (slope/width None). Çocuk erişilebilir → PTH_013 sahte-pozitif ÇIKMAMALI.
        let mut recs = crate::k2::EntityRecords::default();
        recs.stops = vec![
            pth_stop("ST", Some(1), ""),
            pth_stop("EN", Some(2), "ST"),
            pth_stop("PL", Some(0), "ST"),
            pth_stop("BA", Some(4), "PL"),
        ];
        recs.pathways = vec![pth_pw("P1", "EN", "BA")];
        let mut derived = DerivedData::default();
        derived.pathway_graph.adjacency.insert("EN".into(), vec![("BA".into(), 0)]);
        derived.pathway_graph.adjacency.insert("BA".into(), vec![("EN".into(), 0)]);
        let mut v = Vec::new();
        let mut c = 0u32;
        check_pathway_analytics(&recs, &derived, &mut v, &mut c);
        assert!(!v.iter().any(|n| n.rule_id == "PTH_013"),
            "erişilebilir boarding-area çocuğu varken PTH_013 çıkmamalı");
    }

    #[test]
    fn pth_013_inaccessible_station_fires() {
        // Tek rota dik (max_slope 0.20 > 0.08) → tekerlekli sandalye erişimi yok
        // → PTH_013 ÇIKMALI (gerçek erişilebilirlik boşluğu korunur).
        let mut recs = crate::k2::EntityRecords::default();
        recs.stops = vec![
            pth_stop("ST", Some(1), ""),
            pth_stop("EN", Some(2), "ST"),
            pth_stop("PL", Some(0), "ST"),
            pth_stop("BA", Some(4), "PL"),
        ];
        let mut pw = pth_pw("P1", "EN", "BA");
        pw.max_slope = Some(0.20);
        recs.pathways = vec![pw];
        let mut derived = DerivedData::default();
        derived.pathway_graph.adjacency.insert("EN".into(), vec![("BA".into(), 0)]);
        derived.pathway_graph.adjacency.insert("BA".into(), vec![("EN".into(), 0)]);
        let mut v = Vec::new();
        let mut c = 0u32;
        check_pathway_analytics(&recs, &derived, &mut v, &mut c);
        assert!(v.iter().any(|n| n.rule_id == "PTH_013"),
            "dik rotalı istasyon PTH_013 üretmeli");
    }

    #[test]
    fn calendar_dates_extended_service_no_cal_013() {
        // CAL_013 FP fix: calendar.txt end_date geçmişte olsa da calendar_dates ile
        // servis bugüne/geleceğe uzatılmışsa (birleşik max_date >= today) süresi
        // DOLMAMIŞTIR → CAL_013 ÜRETİLMEMELİ. (TriMet over-fire kök nedeni.)
        use crate::k5_derived::CalendarBitmap;
        let mut derived = DerivedData::default();
        // calendar.txt end_date geçmişte olurdu ama calendar_dates 20260601'i eklemiş →
        // birleşik active_dates gelecekte bir tarih içeriyor.
        derived.calendar_bitmap = CalendarBitmap {
            active_dates: [("SVC".to_string(),
                [20251201u32, 20251231, 20260601].into_iter().collect())]
                .into_iter().collect(),
        };
        let mut records = crate::k2::EntityRecords::default();
        records.calendars = vec![cal_rec("SVC", all_days(), (2025, 1, 1), (2025, 12, 31))];
        // today = 20260514 < 20260601 (uzatılmış aktif tarih)
        let result = analyze(&records, &derived, &default_config(), 20260514);
        assert!(!result.notices.iter().any(|n| n.rule_id == "CAL_013"),
            "calendar_dates ile hâlâ aktif servis CAL_013 üretmemeli (FP)");
    }

    #[test]
    fn expired_variants_same_date_aggregate_to_one_cal_013() {
        // Aynı son-aktif-tarihte (20251231) biten iki service_id varyantı (#30 deseni):
        // per-service 2 CAL_013 yerine expiry-imzası başına TEK notice beklenir.
        use crate::k5_derived::CalendarBitmap;
        let mut derived = DerivedData::default();
        derived.calendar_bitmap = CalendarBitmap {
            active_dates: [
                ("A.724".to_string(), [20251215u32, 20251231].into_iter().collect()),
                ("B.724".to_string(), [20251201u32, 20251231].into_iter().collect()),
            ].into_iter().collect(),
        };
        let records = crate::k2::EntityRecords::default();
        let result = analyze(&records, &derived, &default_config(), 20260514);
        let cal013: Vec<_> = result.notices.iter().filter(|n| n.rule_id == "CAL_013").collect();
        assert_eq!(cal013.len(), 1, "aynı expiry tarihini paylaşan iki varyant için tek CAL_013 beklenir");
        assert_eq!(cal013[0].entity_id.as_deref(), Some("20251231"));
        let services = cal013[0].details.as_ref()
            .and_then(|d| d.get("services")).map(String::as_str).unwrap_or("");
        assert!(services.contains("A.724") && services.contains("B.724"),
            "details.services her iki varyantı listelemeli, bulundu: {services}");
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
    fn cal_024_aggregates_inactive_service_to_single_service_notice() {
        use crate::k5_derived::CalendarBitmap;
        let mut derived = DerivedData::default();
        // SVC takvimi yalnızca geçmişte aktif → önümüzdeki 7 günde (20260514+) değil.
        let dates: std::collections::HashSet<u32> = [20260101u32, 20260201].into_iter().collect();
        derived.calendar_bitmap = CalendarBitmap {
            active_dates: [("SVC".to_string(), dates)].into_iter().collect(),
        };
        // İki sefer de aynı SVC takvimine ait → sefer-başına 2 yerine takvim-başına 1 notice.
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
        let result = analyze(&records, &derived, &default_config(), 20260514);
        let trp: Vec<_> = result.notices.iter().filter(|n| n.rule_id == "CAL_024").collect();
        assert_eq!(trp.len(), 1, "aktif olmayan tek takvim için tek CAL_024 beklenir");
        assert_eq!(trp[0].entity_type, gtfs_core::EntityType::Service);
        assert_eq!(trp[0].entity_id.as_deref(), Some("SVC"));
        assert_eq!(trp[0].observed_value.as_deref(), Some("2"), "2 sefer etkilenmeli");
    }

    #[test]
    fn near_future_service_gap_produces_cal_012_not_cal_007() {
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
        // #29: yakın-gelecek boşlukta CAL_012 (Yüksek) CAL_007'nin YERİNE raporlanır (çift-emit yok).
        assert!(result.notices.iter().any(|n| n.rule_id == "CAL_012"),
            "Yakın gelecek boşluk CAL_012 üretmeli");
        assert!(!result.notices.iter().any(|n| n.rule_id == "CAL_007"),
            "Yakın gelecek boşluk CAL_007 üretmemeli (CAL_012 onun yerine geçer)");
    }

    #[test]
    fn shared_calendar_gap_aggregates_across_services_to_one_notice() {
        use crate::k5_derived::CalendarBitmap;
        let mut derived = DerivedData::default();
        // Aynı geçmiş boşluğu (20260101→20260201, ~30 gün ≥ big_gap_days) iki service_id
        // paylaşıyor (#30): per-service 2 CAL_007 yerine gap-imzası başına TEK notice.
        let dates_a: std::collections::HashSet<u32> = [20260101u32, 20260201].into_iter().collect();
        let dates_b: std::collections::HashSet<u32> = [20260101u32, 20260201].into_iter().collect();
        derived.calendar_bitmap = CalendarBitmap {
            active_dates: [
                ("SVC_A".to_string(), dates_a),
                ("SVC_B".to_string(), dates_b),
            ].into_iter().collect(),
        };
        let records = crate::k2::EntityRecords::default();
        let result = analyze(&records, &derived, &default_config(), 20260514);
        let cal007: Vec<_> = result.notices.iter().filter(|n| n.rule_id == "CAL_007").collect();
        assert_eq!(cal007.len(), 1, "aynı boşluğu paylaşan iki servis için tek CAL_007 beklenir");
        assert_eq!(cal007[0].entity_id.as_deref(), Some("20260101-20260201"));
        let services = cal007[0].details.as_ref()
            .and_then(|d| d.get("services"))
            .map(String::as_str)
            .unwrap_or("");
        assert!(services.contains("SVC_A") && services.contains("SVC_B"),
            "details.services her iki servisi listelemeli, bulundu: {services}");
    }

    #[test]
    fn gap_below_big_gap_threshold_no_cal_007() {
        // big_gap_days=14: 10 günlük boşluk (< 14) CAL_007/012 üretmemeli. Bu, MD ile hizanın
        // regresyon koruması — eski service_gap_days=7 eşiğinde bu boşluk (yanlışça) tetiklerdi.
        use crate::k5_derived::CalendarBitmap;
        let mut derived = DerivedData::default();
        // 20260101 → 20260112 = 10 gün boşluk (< 14)
        let dates: std::collections::HashSet<u32> = [20260101u32, 20260112].into_iter().collect();
        derived.calendar_bitmap = CalendarBitmap {
            active_dates: [("SVC".to_string(), dates)].into_iter().collect(),
        };
        let records = crate::k2::EntityRecords::default();
        let result = analyze(&records, &derived, &default_config(), 20260514);
        assert!(!result.notices.iter().any(|n| n.rule_id == "CAL_007" || n.rule_id == "CAL_012"),
            "big_gap_days eşiğinin altındaki boşluk (10<14) CAL_007/012 üretmemeli");
    }

    // ── CAL_021: bugünü kapsayan ama yakın günlerde aktif seferi olmayan servis ──

    fn trip_svc(trip_id: &str, sid: &str) -> TripRecord {
        TEST_TI.with(|cell| {
            let mut ti = cell.borrow_mut();
            let ri = ti.route_ids.len() as u32;
            ti.route_ids.push(SmolStr::new("R1"));
            let si = ti.service_ids.len() as u32;
            ti.service_ids.push(SmolStr::new(sid));
            TripRecord {
                trip_id: trip_id.into(),
                route_idx: ri, service_idx: si, shape_idx: 0,
                headsign_idx: 0, short_name_idx: 0, block_idx: 0, jp_office_idx: 0,
                direction_id: None, wheelchair_accessible: None,
                bikes_allowed: None, cars_allowed: None,
                safe_duration_factor: None, safe_duration_offset: None,
                line: 2,
            }
        })
    }

    #[test]
    fn spans_today_but_no_upcoming_produces_cal_021() {
        use crate::k5_derived::CalendarBitmap;
        let mut derived = DerivedData::default();
        // today=20260514, N=7 → pencere [20260514..20260521].
        // SVC bugünü kapsıyor (20260510 ≤ today ≤ 20260601) ama pencerede aktif gün yok.
        let dates: std::collections::HashSet<u32> = [20260510u32, 20260601].into_iter().collect();
        derived.calendar_bitmap = CalendarBitmap {
            active_dates: [("SVC".to_string(), dates)].into_iter().collect(),
        };
        let mut records = crate::k2::EntityRecords::default();
        records.trips = vec![trip_svc("T1", "SVC")];
        records.trip_interns = take_ti();
        let result = analyze(&records, &derived, &default_config(), 20260514);
        assert!(result.notices.iter().any(|n| n.rule_id == "CAL_021"),
            "bugünü kapsayan ama yakın günde seferi olmayan servis CAL_021 üretmeli");
    }

    #[test]
    fn cal_021_requires_trips() {
        use crate::k5_derived::CalendarBitmap;
        let mut derived = DerivedData::default();
        let dates: std::collections::HashSet<u32> = [20260510u32, 20260601].into_iter().collect();
        derived.calendar_bitmap = CalendarBitmap {
            active_dates: [("SVC".to_string(), dates)].into_iter().collect(),
        };
        let records = crate::k2::EntityRecords::default(); // sefer yok
        let result = analyze(&records, &derived, &default_config(), 20260514);
        assert!(!result.notices.iter().any(|n| n.rule_id == "CAL_021"),
            "sefersiz servis CAL_021 üretmemeli");
    }

    #[test]
    fn cal_021_not_for_active_or_expired() {
        use crate::k5_derived::CalendarBitmap;
        // (a) yakın günde aktif → CAL_021 yok
        let mut derived = DerivedData::default();
        let active: std::collections::HashSet<u32> = [20260514u32, 20260515].into_iter().collect();
        derived.calendar_bitmap = CalendarBitmap {
            active_dates: [("SVC".to_string(), active)].into_iter().collect(),
        };
        let mut records = crate::k2::EntityRecords::default();
        records.trips = vec![trip_svc("T1", "SVC")];
        records.trip_interns = take_ti();
        let r1 = analyze(&records, &derived, &default_config(), 20260514);
        assert!(!r1.notices.iter().any(|n| n.rule_id == "CAL_021"),
            "yakın günde aktif servis CAL_021 üretmemeli");

        // (b) tamamen geçmiş → spans-today değil → CAL_021 yok (CAL_013 alanı)
        let mut derived2 = DerivedData::default();
        let past: std::collections::HashSet<u32> = [20251201u32, 20251231].into_iter().collect();
        derived2.calendar_bitmap = CalendarBitmap {
            active_dates: [("SVC".to_string(), past)].into_iter().collect(),
        };
        let r2 = analyze(&records, &derived2, &default_config(), 20260514);
        assert!(!r2.notices.iter().any(|n| n.rule_id == "CAL_021"),
            "tamamen geçmiş servis CAL_021 üretmemeli");
    }

    // ── CAL_013: bitmap üzerinden süresi dolmuş servis ────────────────────────

    #[test]
    fn expired_service_in_bitmap_produces_cal_013() {
        use crate::k2::calendar_dates::CalendarDateIndex;
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
        // exception_count'ta kayıt olmadan CLD_007/CAL_013 döngüsü çalışmaz
        let mut idx = CalendarDateIndex::default();
        idx.exception_count.insert("EXPIRED_SVC".into(), 1);
        records.calendar_dates = idx;
        let result = analyze(&records, &derived, &default_config(), 20260514);
        assert!(result.notices.iter().any(|n| n.rule_id == "CAL_013"), "CAL_013 olmalı");
    }

    #[test]
    fn active_service_no_cal_013() {
        use crate::k2::calendar_dates::CalendarDateIndex;
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
        let mut idx = CalendarDateIndex::default();
        idx.exception_count.insert("ACTIVE_SVC".into(), 1);
        records.calendar_dates = idx;
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
            vec![trip_sh("T1", "R1", "S1")],
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
    fn stm_017_feed_wide_missing_aggregates_to_single_feed_notice() {
        use crate::k2::shapes::ShapePointRecord;
        // İki trip de shape'i olup sdt eksik; feed genelinde sdt hiç yok →
        // trip-başına 2 yerine tek feed-seviyesi özet beklenir.
        let mut records = records_with(
            vec![stop("A", 41.0, 29.0), stop("B", 41.1, 29.1)],
            vec![route("R1", 3)],
            vec![
                trip_sh("T1", "R1", "S1"),
                trip_sh("T2", "R1", "S1"),
            ],
            vec![
                stoptime("T1", 1, "A", (8,0,0), (8,0,0), 2),
                stoptime("T1", 2, "B", (8,10,0), (8,10,0), 3),
                stoptime("T2", 1, "A", (9,0,0), (9,0,0), 4),
                stoptime("T2", 2, "B", (9,10,0), (9,10,0), 5),
            ],
        );
        records.shapes = vec![
            ShapePointRecord { shape_id: "S1".into(), shape_pt_lat: Some(41.0), shape_pt_lon: Some(29.0),
                shape_pt_sequence: Some(1), shape_dist_traveled: None, line: 2 },
            ShapePointRecord { shape_id: "S1".into(), shape_pt_lat: Some(41.1), shape_pt_lon: Some(29.1),
                shape_pt_sequence: Some(2), shape_dist_traveled: None, line: 3 },
        ];
        let result = analyze(&records, &empty_derived(), &default_config(), 20260514);
        let stm017: Vec<_> = result.notices.iter().filter(|n| n.rule_id == "STM_017").collect();
        assert_eq!(stm017.len(), 1, "feed-geneli eksiklikte tek STM_017 beklenir, alınan: {}", stm017.len());
        assert_eq!(stm017[0].entity_type, gtfs_core::EntityType::Feed, "feed-seviyesi olmalı");
        assert_eq!(stm017[0].observed_value.as_deref(), Some("2"), "etkilenen sefer sayısı 2 olmalı");
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
        // max_shape_jump_km default = 10.0 → severe = 30.0 → 50 km segment tetikler
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

    /// 3× eşiği aşan segment YALNIZ GEO_007 üretmeli.
    ///
    /// Eskiden iki koşul birbirini dışlamıyordu: aynı segment hem GEO_006 hem GEO_007
    /// üretiyor, kullanıcı aynı atlamayı iki kez görüyor ve R5 skorunda iki kez
    /// sayılıyordu. SHP_011 de aynı eşikle üçüncü kez raporluyordu (kaldırıldı).
    #[test]
    fn severe_shape_jump_produces_geo_007_only_not_geo_006() {
        use crate::k5_derived::{ShapeGeometry, ShapeSegments};
        let mut derived = DerivedData::default();
        // default max_shape_jump_km = 10.0 → severe = 30.0; 50 km ikisinin de üstünde
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
        assert!(
            !result.notices.iter().any(|n| n.rule_id == "GEO_006"),
            "3× eşik üstü segment GEO_006 ÜRETMEMELİ (GEO_007'nin işi)"
        );
        assert!(
            !result.notices.iter().any(|n| n.rule_id == "SHP_011"),
            "SHP_011 kaldırıldı — GEO_006 ile aynı eşiği tarıyordu"
        );
    }

    /// Eşik ile 3× arasındaki segment GEO_006 üretir, GEO_007 üretmez (alt katman).
    #[test]
    fn moderate_shape_jump_produces_geo_006_only() {
        use crate::k5_derived::{ShapeGeometry, ShapeSegments};
        let mut derived = DerivedData::default();
        // default max_shape_jump_km = 10.0 → severe = 30.0; 15 km ikisinin arasında
        derived.shape_geometry = ShapeGeometry {
            shapes: [("S1".to_string(), ShapeSegments {
                segment_distances_km: vec![15.0],
                total_length_km: 15.0,
                bbox: (41.0, 41.5, 29.0, 29.5),
            })].into_iter().collect(),
        };
        let records = crate::k2::EntityRecords::default();
        let result = analyze(&records, &derived, &default_config(), 20260514);
        assert!(result.notices.iter().any(|n| n.rule_id == "GEO_006"), "GEO_006 olmalı");
        assert!(!result.notices.iter().any(|n| n.rule_id == "GEO_007"), "GEO_007 olmamalı");
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
            vec![trip_sh("T1", "R1", "S1")],
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
            vec![trip_sh("T1", "R1", "S1")],
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

    // ── OPR_005: sıradışı sefer sıklığı (route_type bazlı göreli aykırı) ─────

    // 15dk headway (08:00, 08:15) veren bir hat üretir.
    fn normal_route(rid: &str, stops: &mut Vec<StopTimeRecord>, trips: &mut Vec<TripRecord>) {
        let (ta, tb) = (format!("{rid}a"), format!("{rid}b"));
        trips.push(trip(&ta, rid)); trips.push(trip(&tb, rid));
        stops.push(stoptime(&ta, 1, "A", (8,0,0), (8,0,0), 2));
        stops.push(stoptime(&ta, 2, "B", (8,10,0), (8,10,0), 3));
        stops.push(stoptime(&tb, 1, "A", (8,15,0), (8,15,0), 4));
        stops.push(stoptime(&tb, 2, "B", (8,25,0), (8,25,0), 5));
    }

    #[test]
    fn opr_005_flags_only_unusually_sparse_route() {
        // 5 normal hat (15dk headway) + 1 seyrek hat (90dk): yalnızca seyrek olan işaretlenir.
        let mut routes = Vec::new();
        let mut trips = Vec::new();
        let mut sts = Vec::new();
        for i in 0..5u32 {
            let rid = format!("R{i}");
            routes.push(route(&rid, 3));
            normal_route(&rid, &mut sts, &mut trips);
        }
        // Seyrek hat: 08:00 ve 09:30 → 90dk headway.
        routes.push(route("RS", 3));
        trips.push(trip("RSa", "RS")); trips.push(trip("RSb", "RS"));
        sts.push(stoptime("RSa", 1, "A", (8,0,0), (8,0,0), 2));
        sts.push(stoptime("RSa", 2, "B", (8,10,0), (8,10,0), 3));
        sts.push(stoptime("RSb", 1, "A", (9,30,0), (9,30,0), 4));
        sts.push(stoptime("RSb", 2, "B", (9,40,0), (9,40,0), 5));
        let records = records_with(
            vec![stop("A", 41.0, 29.0), stop("B", 41.1, 29.1)], routes, trips, sts,
        );
        let result = analyze(&records, &empty_derived(), &default_config(), 20260514);
        let opr: Vec<_> = result.notices.iter().filter(|n| n.rule_id == "OPR_005").collect();
        assert_eq!(opr.len(), 1, "yalnızca sıradışı seyrek hat işaretlenmeli");
        assert_eq!(opr[0].entity_id.as_deref(), Some("RS"));
    }

    #[test]
    fn opr_005_silent_when_frequency_is_uniform() {
        // 6 hat, hepsi 15dk headway → aykırı yok.
        let mut routes = Vec::new();
        let mut trips = Vec::new();
        let mut sts = Vec::new();
        for i in 0..6u32 {
            let rid = format!("R{i}");
            routes.push(route(&rid, 3));
            normal_route(&rid, &mut sts, &mut trips);
        }
        let records = records_with(
            vec![stop("A", 41.0, 29.0), stop("B", 41.1, 29.1)], routes, trips, sts,
        );
        let result = analyze(&records, &empty_derived(), &default_config(), 20260514);
        assert!(!result.notices.iter().any(|n| n.rule_id == "OPR_005"),
            "tekdüze sıklıkta OPR_005 üretilmemeli");
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
            vec![trip_sh("T1", "R1", "S1")],
            vec![
                stoptime("T1", 1, "A", (8,0,0), (8,0,0), 2),
                stoptime("T1", 2, "B", (8,10,0), (8,10,0), 3),
            ],
        );
        let result = analyze(&records, &empty_derived(), &default_config(), 20260514);
        assert!(!result.notices.iter().any(|n| n.rule_id == "RTS_017"));
    }

    #[test]
    fn rts_017_one_notice_per_shapeless_route() {
        // İki shape'siz hat (R1, R2) + bir shape'li hat (R3) → yalnızca R1,R2 için ayrı notice.
        let records = records_with(
            vec![stop("A", 41.0, 29.0), stop("B", 41.1, 29.1)],
            vec![route("R1", 3), route("R2", 3), route("R3", 3)],
            vec![
                trip("T1", "R1"),
                trip("T2", "R2"),
                trip_sh("T3", "R3", "S1"),
            ],
            vec![
                stoptime("T1", 1, "A", (8,0,0), (8,0,0), 2),
                stoptime("T1", 2, "B", (8,10,0), (8,10,0), 3),
                stoptime("T2", 1, "A", (8,0,0), (8,0,0), 4),
                stoptime("T2", 2, "B", (8,10,0), (8,10,0), 5),
                stoptime("T3", 1, "A", (8,0,0), (8,0,0), 6),
                stoptime("T3", 2, "B", (8,10,0), (8,10,0), 7),
            ],
        );
        let result = analyze(&records, &empty_derived(), &default_config(), 20260514);
        let rts: Vec<_> = result.notices.iter().filter(|n| n.rule_id == "RTS_017").collect();
        assert_eq!(rts.len(), 2, "her shape'siz hat için ayrı RTS_017 beklenir");
        assert!(rts.iter().all(|n| n.entity_type == gtfs_core::EntityType::Route));
        let ids: std::collections::HashSet<_> = rts.iter().filter_map(|n| n.entity_id.as_deref()).collect();
        assert!(ids.contains("R1") && ids.contains("R2") && !ids.contains("R3"));
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
            vec![trip_blk("T1", "R1", "BLK1")], // block'ta tek sefer
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
                trip_blk("T1", "R1", "BLK1"),
                trip_blk("T2", "R1", "BLK1"),
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
            vec![trip_sh("T1", "R1", "S1")],
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
            vec![trip_sh("T1", "R1", "S1")],
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
    fn interleaved_stop_times_rows_produce_stm_036() {
        // İki trip dosyada interleaved (T1,T2,T1,T2 satır sırası) → her trip'in satırları dağınık,
        // sequence yine ARTAN. MD unsorted_stop_times (INFO) karşılığı: STM_036 iki trip için de çıkar
        // (sequence azalmasa bile non-contiguous).
        let records = records_with(
            vec![stop("A", 41.0, 29.0), stop("B", 41.1, 29.1)],
            vec![route("R1", 3)],
            vec![trip("T1", "R1"), trip("T2", "R1")],
            vec![
                stoptime("T1", 1, "A", (8,0,0), (8,0,0), 2),
                stoptime("T2", 1, "A", (9,0,0), (9,0,0), 3),
                stoptime("T1", 2, "B", (8,10,0), (8,10,0), 4),
                stoptime("T2", 2, "B", (9,10,0), (9,10,0), 5),
            ],
        );
        let result = analyze(&records, &empty_derived(), &default_config(), 20260514);
        let stm036: Vec<_> = result.notices.iter().filter(|n| n.rule_id == "STM_036").collect();
        assert_eq!(stm036.len(), 2,
            "iki interleaved trip için 2 STM_036 (non-contiguous) beklenir: {:?}",
            stm036.iter().map(|n| &n.message).collect::<Vec<_>>());
    }

    #[test]
    fn equal_dist_diff_coords_produces_shp_028() {
        use crate::k2::shapes::ShapePointRecord;
        let mut records = records_with(
            vec![stop("A", 41.0, 29.0), stop("B", 41.0, 29.1)],
            vec![route("R1", 3)],
            vec![trip_sh("T1", "R1", "S1")],
            vec![
                stoptime("T1", 1, "A", (8,0,0), (8,0,0), 2),
                stoptime("T1", 2, "B", (8,10,0), (8,10,0), 3),
            ],
        );
        // Aynı shape_dist_traveled (100.0) ama farklı koordinat (Δ=0.1° ≫ eşik) → SHP_028
        records.shapes = vec![
            ShapePointRecord { shape_id: "S1".into(), shape_pt_lat: Some(41.0), shape_pt_lon: Some(29.0),
                shape_pt_sequence: Some(1), shape_dist_traveled: Some(100.0), line: 2 },
            ShapePointRecord { shape_id: "S1".into(), shape_pt_lat: Some(41.0), shape_pt_lon: Some(29.1),
                shape_pt_sequence: Some(2), shape_dist_traveled: Some(100.0), line: 3 },
        ];
        let result = analyze(&records, &empty_derived(), &default_config(), 20260514);
        let ids: Vec<&str> = result.notices.iter().map(|n| n.rule_id.as_str()).collect();
        assert!(ids.contains(&"SHP_028"), "aynı dist + farklı koordinat → SHP_028: {:?}", ids);
        assert!(!ids.contains(&"SHP_029"), "eşik üstü fark SHP_029 olmamalı: {:?}", ids);
    }

    #[test]
    fn shape_with_both_below_threshold_and_repeat_fires_both_types() {
        use crate::k2::shapes::ShapePointRecord;
        // Bir shape hem eşik-altı fark (SHP_029, ÖNCE) hem tekrar-nokta (SHP_023, SONRA) içeriyor.
        // Eski first-wins `break` SHP_023'ü kaçırırdı (Spelunca'da 6 kayıp); artık her tür bir kez.
        let mut records = records_with(
            vec![stop("A", 41.0, 29.0), stop("B", 41.0, 29.1)],
            vec![route("R1", 3)],
            vec![trip_sh("T1", "R1", "S1")],
            vec![
                stoptime("T1", 1, "A", (8,0,0), (8,0,0), 2),
                stoptime("T1", 2, "B", (8,10,0), (8,10,0), 3),
            ],
        );
        records.shapes = vec![
            ShapePointRecord { shape_id: "S1".into(), shape_pt_lat: Some(41.0),      shape_pt_lon: Some(29.0),
                shape_pt_sequence: Some(1), shape_dist_traveled: Some(100.0), line: 2 },
            // Δlat=5e-6 (< 1e-5 eşik) → SHP_029 (eşik-altı), ilk çift
            ShapePointRecord { shape_id: "S1".into(), shape_pt_lat: Some(41.000005), shape_pt_lon: Some(29.0),
                shape_pt_sequence: Some(2), shape_dist_traveled: Some(100.0), line: 3 },
            // 2. ile birebir aynı → SHP_023 (tekrar-nokta), ikinci çift
            ShapePointRecord { shape_id: "S1".into(), shape_pt_lat: Some(41.000005), shape_pt_lon: Some(29.0),
                shape_pt_sequence: Some(3), shape_dist_traveled: Some(100.0), line: 4 },
        ];
        let result = analyze(&records, &empty_derived(), &default_config(), 20260514);
        let ids: Vec<&str> = result.notices.iter().map(|n| n.rule_id.as_str()).collect();
        assert!(ids.contains(&"SHP_029"), "eşik-altı fark → SHP_029: {:?}", ids);
        assert!(ids.contains(&"SHP_023"), "tekrar-nokta → SHP_023 (first-wins bug fix): {:?}", ids);
        assert_eq!(ids.iter().filter(|&&x| x == "SHP_023").count(), 1, "SHP_023 shape başına tek");
        assert_eq!(ids.iter().filter(|&&x| x == "SHP_029").count(), 1, "SHP_029 shape başına tek");
    }

    #[test]
    fn equal_dist_tiny_coord_diff_produces_shp_029() {
        use crate::k2::shapes::ShapePointRecord;
        let mut records = records_with(
            vec![stop("A", 41.0, 29.0), stop("B", 41.0, 29.1)],
            vec![route("R1", 3)],
            vec![trip_sh("T1", "R1", "S1")],
            vec![
                stoptime("T1", 1, "A", (8,0,0), (8,0,0), 2),
                stoptime("T1", 2, "B", (8,10,0), (8,10,0), 3),
            ],
        );
        // Aynı dist, koordinat farkı eşik altı (Δ=1e-6° < 1e-5°) → SHP_029
        records.shapes = vec![
            ShapePointRecord { shape_id: "S1".into(), shape_pt_lat: Some(41.0), shape_pt_lon: Some(29.0),
                shape_pt_sequence: Some(1), shape_dist_traveled: Some(100.0), line: 2 },
            ShapePointRecord { shape_id: "S1".into(), shape_pt_lat: Some(41.0), shape_pt_lon: Some(29.000001),
                shape_pt_sequence: Some(2), shape_dist_traveled: Some(100.0), line: 3 },
        ];
        let result = analyze(&records, &empty_derived(), &default_config(), 20260514);
        let ids: Vec<&str> = result.notices.iter().map(|n| n.rule_id.as_str()).collect();
        assert!(ids.contains(&"SHP_029"), "aynı dist + minik koordinat farkı → SHP_029: {:?}", ids);
        assert!(!ids.contains(&"SHP_028"), "eşik altı fark SHP_028 olmamalı: {:?}", ids);
    }

    #[test]
    fn stop_near_pole_produces_geo_022() {
        let records = records_with(
            vec![stop("A", 89.5, 10.0), stop("B", 41.0, 29.0)],
            vec![route("R1", 3)],
            vec![trip("T1", "R1")],
            vec![
                stoptime("T1", 1, "A", (8,0,0), (8,0,0), 2),
                stoptime("T1", 2, "B", (8,10,0), (8,10,0), 3),
            ],
        );
        let result = analyze(&records, &empty_derived(), &default_config(), 20260514);
        let ids: Vec<&str> = result.notices.iter().map(|n| n.rule_id.as_str()).collect();
        assert!(ids.contains(&"GEO_022"), "kutba yakın durak (lat 89.5) → GEO_022: {:?}", ids);
    }

    #[test]
    fn bidirectional_route_single_shape_produces_opr_015() {
        let mut t0 = trip_sh("T0", "R1", "S1"); t0.direction_id = Some(0);
        let mut t1 = trip_sh("T1", "R1", "S1"); t1.direction_id = Some(1);
        let records = records_with(
            vec![stop("A", 41.0, 29.0), stop("B", 41.1, 29.1)],
            vec![route("R1", 3)],
            vec![t0, t1],
            vec![
                stoptime("T0", 1, "A", (8,0,0), (8,0,0), 2), stoptime("T0", 2, "B", (8,10,0), (8,10,0), 3),
                stoptime("T1", 1, "B", (9,0,0), (9,0,0), 2), stoptime("T1", 2, "A", (9,10,0), (9,10,0), 3),
            ],
        );
        let result = analyze(&records, &empty_derived(), &default_config(), 20260514);
        assert!(result.notices.iter().any(|n| n.rule_id == "OPR_015"),
            "çift yönlü + tek shape → OPR_015");
    }

    #[test]
    fn unidirectional_route_single_shape_no_opr_015() {
        // İki trip de direction_id=0 → tek yönlü; tek shape beklenen davranış → OPR_015 yok
        let mut t0 = trip_sh("T0", "R1", "S1"); t0.direction_id = Some(0);
        let mut t1 = trip_sh("T1", "R1", "S1"); t1.direction_id = Some(0);
        let records = records_with(
            vec![stop("A", 41.0, 29.0), stop("B", 41.1, 29.1)],
            vec![route("R1", 3)],
            vec![t0, t1],
            vec![
                stoptime("T0", 1, "A", (8,0,0), (8,0,0), 2), stoptime("T0", 2, "B", (8,10,0), (8,10,0), 3),
                stoptime("T1", 1, "A", (9,0,0), (9,0,0), 2), stoptime("T1", 2, "B", (9,10,0), (9,10,0), 3),
            ],
        );
        let result = analyze(&records, &empty_derived(), &default_config(), 20260514);
        assert!(!result.notices.iter().any(|n| n.rule_id == "OPR_015"),
            "tek yönlü + tek shape → OPR_015 olmamalı");
    }

    #[test]
    fn shape_reused_in_both_directions_no_shp_016() {
        use crate::k2::shapes::ShapePointRecord;
        // S1: A(29.0) → B(29.1). T1 ileri (A→B), T2 aynı shape'i ters kullanıyor (B→A).
        // Bir ileri varyant olduğu için shape "ters çizilmiş" sayılmamalı (varyant-farkında min).
        let mut records = records_with(
            vec![stop("A", 41.0, 29.0), stop("B", 41.0, 29.1)],
            vec![route("R1", 3)],
            vec![
                trip_sh("T1", "R1", "S1"),
                trip_sh("T2", "R1", "S1"),
            ],
            vec![
                stoptime("T1", 1, "A", (8,0,0), (8,0,0), 2),
                stoptime("T1", 2, "B", (8,10,0), (8,10,0), 3),
                stoptime("T2", 1, "B", (9,0,0), (9,0,0), 4),
                stoptime("T2", 2, "A", (9,10,0), (9,10,0), 5),
            ],
        );
        records.shapes = vec![
            ShapePointRecord { shape_id: "S1".into(), shape_pt_lat: Some(41.0), shape_pt_lon: Some(29.0),
                shape_pt_sequence: Some(1), shape_dist_traveled: None, line: 2 },
            ShapePointRecord { shape_id: "S1".into(), shape_pt_lat: Some(41.0), shape_pt_lon: Some(29.1),
                shape_pt_sequence: Some(2), shape_dist_traveled: None, line: 3 },
        ];
        let result = analyze(&records, &empty_derived(), &default_config(), 20260514);
        assert!(!result.notices.iter().any(|n| n.rule_id == "SHP_016"),
            "ileri varyant varken shape ters sayılmamalı (SHP_016 yok)");
    }

    #[test]
    fn shape_end_matches_one_variant_no_shp_014() {
        use crate::k2::shapes::ShapePointRecord;
        // S1: A(29.0) → B(29.1). T1 tam (A→B, son durak B = shape sonu).
        // T2 kısa varyant (A→FAR, son durak shape sonundan çok uzak).
        // En yakın varyant (T1) shape sonuyla eşleştiği için SHP_014 ateşlememeli.
        let mut records = records_with(
            vec![stop("A", 41.0, 29.0), stop("B", 41.0, 29.1), stop("FAR", 42.0, 30.0)],
            vec![route("R1", 3)],
            vec![
                trip_sh("T1", "R1", "S1"),
                trip_sh("T2", "R1", "S1"),
            ],
            vec![
                stoptime("T1", 1, "A", (8,0,0), (8,0,0), 2),
                stoptime("T1", 2, "B", (8,10,0), (8,10,0), 3),
                stoptime("T2", 1, "A", (9,0,0), (9,0,0), 4),
                stoptime("T2", 2, "FAR", (9,10,0), (9,10,0), 5),
            ],
        );
        records.shapes = vec![
            ShapePointRecord { shape_id: "S1".into(), shape_pt_lat: Some(41.0), shape_pt_lon: Some(29.0),
                shape_pt_sequence: Some(1), shape_dist_traveled: None, line: 2 },
            ShapePointRecord { shape_id: "S1".into(), shape_pt_lat: Some(41.0), shape_pt_lon: Some(29.1),
                shape_pt_sequence: Some(2), shape_dist_traveled: None, line: 3 },
        ];
        let result = analyze(&records, &empty_derived(), &default_config(), 20260514);
        assert!(!result.notices.iter().any(|n| n.rule_id == "SHP_014"),
            "shape sonu bir varyantın son durağıyla eşleşiyorsa SHP_014 yok");
    }

    #[test]
    fn shape_end_far_from_all_variants_produces_shp_014() {
        use crate::k2::shapes::ShapePointRecord;
        // S1: A(29.0) → B(29.1). Tek sefer T1'in son durağı C, shape sonundan çok uzak.
        // Hiçbir varyant shape sonuna yakın değil → SHP_014 ateşlemeli (regresyon koruması).
        let mut records = records_with(
            vec![stop("A", 41.0, 29.0), stop("C", 42.0, 30.0)],
            vec![route("R1", 3)],
            vec![trip_sh("T1", "R1", "S1")],
            vec![
                stoptime("T1", 1, "A", (8,0,0), (8,0,0), 2),
                stoptime("T1", 2, "C", (8,10,0), (8,10,0), 3),
            ],
        );
        records.shapes = vec![
            ShapePointRecord { shape_id: "S1".into(), shape_pt_lat: Some(41.0), shape_pt_lon: Some(29.0),
                shape_pt_sequence: Some(1), shape_dist_traveled: None, line: 2 },
            ShapePointRecord { shape_id: "S1".into(), shape_pt_lat: Some(41.0), shape_pt_lon: Some(29.1),
                shape_pt_sequence: Some(2), shape_dist_traveled: None, line: 3 },
        ];
        let result = analyze(&records, &empty_derived(), &default_config(), 20260514);
        assert!(result.notices.iter().any(|n| n.rule_id == "SHP_014"),
            "shape sonu hiçbir varyantın son durağına yakın değil → SHP_014");
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
            vec![trip_sh("T1", "R1", "S1")],
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

    #[test]
    fn backtrack_that_recovers_no_shp_017() {
        use crate::k2::shapes::ShapePointRecord;
        // Backtrack kurtarma filtresi (#17): doğuya giden düz shape 29.00 → 29.10 (lat 41.0).
        // stop_times: A(29.00) → B(29.03) → C(29.015: B'nin ~1.3km gerisi, >500m tolerans) → D(29.05).
        // C geri sıçrar AMA D 3 durak içinde önceki maksimumu (B) geçer → kurtarma → SHP_017 YOK.
        let mut records = records_with(
            vec![
                stop("A", 41.0, 29.00),
                stop("B", 41.0, 29.03),
                stop("C", 41.0, 29.015),
                stop("D", 41.0, 29.05),
            ],
            vec![route("R1", 3)],
            vec![trip_sh("T1", "R1", "S1")],
            vec![
                stoptime("T1", 1, "A", (8,0,0),  (8,0,0),  2),
                stoptime("T1", 2, "B", (8,10,0), (8,10,0), 3),
                stoptime("T1", 3, "C", (8,20,0), (8,20,0), 4),
                stoptime("T1", 4, "D", (8,30,0), (8,30,0), 5),
            ],
        );
        records.shapes = vec![
            ShapePointRecord { shape_id: "S1".into(), shape_pt_lat: Some(41.0), shape_pt_lon: Some(29.00),
                shape_pt_sequence: Some(1), shape_dist_traveled: None, line: 2 },
            ShapePointRecord { shape_id: "S1".into(), shape_pt_lat: Some(41.0), shape_pt_lon: Some(29.05),
                shape_pt_sequence: Some(2), shape_dist_traveled: None, line: 3 },
            ShapePointRecord { shape_id: "S1".into(), shape_pt_lat: Some(41.0), shape_pt_lon: Some(29.10),
                shape_pt_sequence: Some(3), shape_dist_traveled: None, line: 4 },
        ];
        let result = analyze(&records, &empty_derived(), &default_config(), 20260514);
        assert!(!result.notices.iter().any(|n| n.rule_id == "SHP_017"),
            "geri sıçrayıp 3 durak içinde toparlanan trip SHP_017 üretmemeli (backtrack kurtarma filtresi)");
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

    /// Yalnız GİRİŞİ olan (peronu olmayan) istasyon da STP_030'dur: sefer perona durur,
    /// girişe değil. Corpus kanıtı (mdb-2933): `MTR-POA`'nın 6 çocuğu da location_type=2
    /// (giriş); biz susuyorduk, MD `unused_station` ile işaretliyordu.
    #[test]
    fn station_with_only_entrances_produces_stp_030() {
        let mut station = stop("PS1", 41.0, 29.0);
        station.location_type = Some(1);
        let mut entrance = stop("E1", 41.0, 29.0);
        entrance.location_type = Some(2); // giriş/çıkış — peron DEĞİL
        entrance.row.insert("parent_station".to_string(), "PS1".to_string());
        let records = records_with(
            vec![station, entrance, stop("A", 41.0, 29.0)],
            vec![route("R1", 3)],
            vec![trip("T1", "R1")],
            vec![stoptime("T1", 1, "A", (8, 0, 0), (8, 0, 0), 2)],
        );
        let result = analyze(&records, &empty_derived(), &default_config(), 20260514);
        assert!(
            result.notices.iter().any(|n| n.rule_id == "STP_030" && n.entity_id.as_deref() == Some("PS1")),
            "yalnız girişi olan istasyon STP_030 üretmeli (peron yok)"
        );
    }

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

    /// Headsign SON DURAĞIN adıyla eşleşiyorsa, aynı ad bir ara durakta da geçse bile
    /// yön adı DOĞRUdur → TRP_020 üretilmemeli.
    ///
    /// Eski kod terminali `stop_id` ile eliyor ama eşleşmeyi İSİM ile yapıyordu; aynı ada
    /// sahip farklı stop_id (ayrı peron/yön kaydı, ortak parent YOK) filtreden geçiyordu.
    /// Corpus kanıtı (mdb-1294, Roma): 9.162 sefer işaretleniyordu, MD 19.
    #[test]
    fn headsign_matching_terminal_name_is_not_trp_020_even_if_name_repeats() {
        let mut a = stop("A", 41.0, 29.0);
        a.stop_name = Some("Başlangıç".into());
        // Ara durak ile terminal AYNI ADA sahip, FARKLI stop_id, parent YOK.
        let mut mid = stop("M", 41.05, 29.05);
        mid.stop_name = Some("Cimitero".into());
        let mut term = stop("T", 41.1, 29.1);
        term.stop_name = Some("Cimitero".into());
        let records = records_with(
            vec![a, mid, term],
            vec![route("R1", 3)],
            vec![trip_hs("T1", "R1", "Cimitero")], // headsign = SON durağın adı → doğru
            vec![
                stoptime("T1", 1, "A", (8, 0, 0), (8, 0, 0), 2),
                stoptime("T1", 2, "M", (8, 5, 0), (8, 5, 0), 3),
                stoptime("T1", 3, "T", (8, 10, 0), (8, 10, 0), 4),
            ],
        );
        let result = analyze(&records, &empty_derived(), &default_config(), 20260514);
        assert!(
            !result.notices.iter().any(|n| n.rule_id == "TRP_020"),
            "headsign son durağın adıyla eşleşiyor → yön adı doğru, TRP_020 olmamalı"
        );
    }

    #[test]
    fn headsign_matches_intermediate_stop_produces_trp_020() {
        // Stop A (seq 1, ara), Stop B (seq 2, terminal)
        // headsign = "Stop A" (ara durak adı) → TRP_020 olmalı
        let mut sa = stop("A", 41.0, 29.0);
        sa.stop_name = Some("Stop A".into());
        let mut sb = stop("B", 41.1, 29.1);
        sb.stop_name = Some("Stop B".into());
        let t = trip_hs("T1", "R1", "Stop A"); // ara durak adı
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
        let t = trip_hs("T1", "R1", "Stop B"); // terminal durak adı
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
        let t = trip_hs("T1", "R1", "Tren Garı");
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

    #[test]
    fn headsign_matches_repeated_turnaround_stop_no_trp_020() {
        // Sefer: A → B → C → B (tekrar) → D. headsign = "Stop B" = TEKRAR EDEN durak.
        // B bir uç/dönüş noktası (havalimanı wye gibi) → headsign meşru → TRP_020 olmamalı.
        let mut sa = stop("A", 41.0, 29.0); sa.stop_name = Some("Stop A".into());
        let mut sb = stop("B", 41.1, 29.1); sb.stop_name = Some("Stop B".into());
        let mut sc = stop("C", 41.2, 29.2); sc.stop_name = Some("Stop C".into());
        let mut sd = stop("D", 41.3, 29.3); sd.stop_name = Some("Stop D".into());
        let t = trip_hs("T1", "R1", "Stop B");
        let records = records_with(
            vec![sa, sb, sc, sd],
            vec![route("R1", 3)],
            vec![t],
            vec![
                stoptime("T1", 1, "A", (8,0,0), (8,0,0), 2),
                stoptime("T1", 2, "B", (8,10,0), (8,10,0), 3),
                stoptime("T1", 3, "C", (8,20,0), (8,20,0), 4),
                stoptime("T1", 4, "B", (8,30,0), (8,30,0), 5),
                stoptime("T1", 5, "D", (8,40,0), (8,40,0), 6),
            ],
        );
        let result = analyze(&records, &empty_derived(), &default_config(), 20260514);
        assert!(!result.notices.iter().any(|n| n.rule_id == "TRP_020"), "headsign tekrar eden uç noktaya eşleşiyor → TRP_020 olmamalı");
    }

    #[test]
    fn headsign_intermediate_with_unrelated_repeat_produces_trp_020() {
        // Sefer: A → B → C → B (tekrar) → D. headsign = "Stop C" = TEK GEÇEN ara durak.
        // B'nin tekrarı ALAKASIZ; headsign C bir kez geçen meşru ara durak eşleşmesi.
        // Eski "herhangi tekrar → loop" mantığı bunu yanlışlıkla bastırırdı (yanlış negatif).
        let mut sa = stop("A", 41.0, 29.0); sa.stop_name = Some("Stop A".into());
        let mut sb = stop("B", 41.1, 29.1); sb.stop_name = Some("Stop B".into());
        let mut sc = stop("C", 41.2, 29.2); sc.stop_name = Some("Stop C".into());
        let mut sd = stop("D", 41.3, 29.3); sd.stop_name = Some("Stop D".into());
        let t = trip_hs("T1", "R1", "Stop C");
        let records = records_with(
            vec![sa, sb, sc, sd],
            vec![route("R1", 3)],
            vec![t],
            vec![
                stoptime("T1", 1, "A", (8,0,0), (8,0,0), 2),
                stoptime("T1", 2, "B", (8,10,0), (8,10,0), 3),
                stoptime("T1", 3, "C", (8,20,0), (8,20,0), 4),
                stoptime("T1", 4, "B", (8,30,0), (8,30,0), 5),
                stoptime("T1", 5, "D", (8,40,0), (8,40,0), 6),
            ],
        );
        let result = analyze(&records, &empty_derived(), &default_config(), 20260514);
        assert!(result.notices.iter().any(|n| n.rule_id == "TRP_020"),
            "alakasız tekrar headsign'ın eşleştiği ara durağı bastırmamalı: {:?}",
            result.notices.iter().map(|n| &n.rule_id).collect::<Vec<_>>());
    }

    #[test]
    fn headsign_intermediate_consecutive_duplicate_produces_trp_020() {
        // #29: Sefer A → B → B → C; B aynı stop_id PEŞ PEŞE iki kez (dwell/timepoint çiftlemesi,
        // BART SFO seq 26-27 deseni), terminal = C. headsign = "Stop B" = ara durak.
        // Ardışık yineleme gerçek uç/dönüş DEĞİLDİR → TRP_020 ÇIKMALI (önceki mantık yanlış bastırırdı).
        let mut sa = stop("A", 41.0, 29.0); sa.stop_name = Some("Stop A".into());
        let mut sb = stop("B", 41.1, 29.1); sb.stop_name = Some("Stop B".into());
        let mut sc = stop("C", 41.2, 29.2); sc.stop_name = Some("Stop C".into());
        let t = trip_hs("T1", "R1", "Stop B");
        let records = records_with(
            vec![sa, sb, sc],
            vec![route("R1", 3)],
            vec![t],
            vec![
                stoptime("T1", 1, "A", (8,0,0), (8,0,0), 2),
                stoptime("T1", 2, "B", (8,10,0), (8,10,0), 3),
                stoptime("T1", 3, "B", (8,11,0), (8,11,0), 4),
                stoptime("T1", 4, "C", (8,20,0), (8,20,0), 5),
            ],
        );
        let result = analyze(&records, &empty_derived(), &default_config(), 20260514);
        assert!(result.notices.iter().any(|n| n.rule_id == "TRP_020"),
            "ardışık yinelenen ara durak (dwell) bastırılmamalı → TRP_020 çıkmalı: {:?}",
            result.notices.iter().map(|n| &n.rule_id).collect::<Vec<_>>());
    }

    #[test]
    fn headsign_loop_via_parent_station_no_trp_020() {
        // Loop: A1 → B → A2; A1 ve A2 farklı stop_id ama aynı parent_station P.
        // first(A1) != terminal(A2) (stop_id) ama etkin istasyon aynı (P) → TRP_020 olmamalı.
        let mut sa1 = stop("A1", 41.0, 29.0); sa1.stop_name = Some("İstasyon P".into());
        sa1.row.insert("parent_station".into(), "P".into());
        let mut sa2 = stop("A2", 41.0, 29.0); sa2.stop_name = Some("İstasyon P".into());
        sa2.row.insert("parent_station".into(), "P".into());
        let mut sb = stop("B", 41.1, 29.1); sb.stop_name = Some("Stop B".into());
        let t = trip_hs("T1", "R1", "Stop B"); // ara durak adı
        let records = records_with(
            vec![sa1, sa2, sb],
            vec![route("R1", 3)],
            vec![t],
            vec![
                stoptime("T1", 1, "A1", (8,0,0), (8,0,0), 2),
                stoptime("T1", 2, "B",  (8,10,0), (8,10,0), 3),
                stoptime("T1", 3, "A2", (8,20,0), (8,20,0), 4),
            ],
        );
        let result = analyze(&records, &empty_derived(), &default_config(), 20260514);
        assert!(!result.notices.iter().any(|n| n.rule_id == "TRP_020"), "parent_station ile dönen loop → TRP_020 olmamalı");
    }

    // ── SHP_022: durak shape'te birden fazla eşleşme bölgesine yakın ─────────

    #[test]
    fn stop_near_two_shape_sections_produces_shp_022() {
        // "U" şekli: gidip dönen hat — durak hem gidiş hem dönüş bölümüne yakın,
        // shape_dist_traveled YOK. #52: sıra KULLANILAMAZ olmalı (burada duplicate
        // stop_sequence=1) → konum belirsiz → SHP_022 beklenir.
        use crate::k2::shapes::ShapePointRecord;
        let t = trip_sh("T1", "R1", "SH1");
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
                stoptime("T1", 1, "S1", (8, 10, 0), (8, 10, 0), 3), // duplicate seq → sıra kullanılamaz
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
            "U-şekli + shape_dist eksik + sıra kullanılamaz → SHP_022 beklenir: {:?}",
            result.notices.iter().map(|n| &n.rule_id).collect::<Vec<_>>()
        );
    }

    #[test]
    fn stop_near_two_sections_valid_sequence_no_shp_022() {
        // #52: AYNI U-şekli geometrisi ama stop_sequence GEÇERLİ (1,2 kesin artan) →
        // sıra-farkında eşleme belirsizliği çözer → SHP_022 ÇIKMAMALI (over-fire önlendi).
        use crate::k2::shapes::ShapePointRecord;
        let t = trip_sh("T1", "R1", "SH1");
        let mut s = stop("S1", 0.5, 0.00005);
        s.stop_name = Some("Orta Durak".into());
        let mut records = records_with(
            vec![s],
            vec![route("R1", 3)],
            vec![t],
            vec![
                stoptime("T1", 1, "S1", (8, 0, 0), (8, 0, 0), 2),
                stoptime("T1", 2, "S1", (8, 10, 0), (8, 10, 0), 3), // geçerli artan sıra
            ],
        );
        records.shapes = vec![
            ShapePointRecord { shape_id: "SH1".into(), shape_pt_lat: Some(0.0), shape_pt_lon: Some(0.0),    shape_pt_sequence: Some(1), shape_dist_traveled: None, line: 2 },
            ShapePointRecord { shape_id: "SH1".into(), shape_pt_lat: Some(1.0), shape_pt_lon: Some(0.0),    shape_pt_sequence: Some(2), shape_dist_traveled: None, line: 3 },
            ShapePointRecord { shape_id: "SH1".into(), shape_pt_lat: Some(1.0), shape_pt_lon: Some(0.001),  shape_pt_sequence: Some(3), shape_dist_traveled: None, line: 4 },
            ShapePointRecord { shape_id: "SH1".into(), shape_pt_lat: Some(0.0), shape_pt_lon: Some(0.001),  shape_pt_sequence: Some(4), shape_dist_traveled: None, line: 5 },
        ];
        let result = analyze(&records, &empty_derived(), &default_config(), 20260514);
        assert!(
            !result.notices.iter().any(|n| n.rule_id == "SHP_022"),
            "geçerli stop_sequence ile SHP_022 çıkmamalı (sıra belirsizliği çözer)"
        );
    }

    #[test]
    fn stop_near_one_shape_section_no_shp_022() {
        // Düz hat — stop yalnızca bir bölüme yakın → SHP_022 olmamalı
        use crate::k2::shapes::ShapePointRecord;
        let t = trip_sh("T1", "R1", "SH2");
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

    // ── Fix-8 (revize): patolojik shape projeksiyonu haversine'e düşer ────────

    #[test]
    fn pathological_shape_projection_falls_back_to_haversine() {
        // Kuş uçuşu A→B ≈ 0.84 km. U biçimli shape projeksiyonu B'yi yanlış segmente
        // eşleştirip arc ≈ 23 km verir (27× kuş uçuşu). Bu YANLIŞ segment eşleşmesidir
        // (LA Metro grid bus rotalarındaki gibi). 4× kuralıyla haversine'e düşülmeli:
        // 0.84 km / 5 dk ≈ 10 km/h → STM_012 de STM_014 de ÜRETİLMEMELİ.
        use crate::k2::shapes::ShapePointRecord;
        let mut records = records_with(
            vec![stop("A", 41.0, 29.0), stop("B", 41.0, 29.01)],
            vec![route("R1", 3)],
            vec![trip_sh("T1", "R1", "S1")],
            vec![
                stoptime("T1", 1, "A", (8, 0, 0), (8, 0, 0), 2),
                stoptime("T1", 2, "B", (8, 5, 0), (8, 5, 0), 3), // 5 dk
            ],
        );
        // U shape: (41.0,29.0) → kuzeye (41.1,29.0) → (41.1,29.01) → güneye (41.0,29.01)
        // Arc ≈ 23 km vs haversine ~0.84 km → ratio ~27× → 4× eşiği aşar → fallback.
        records.shapes = vec![
            ShapePointRecord { shape_id: "S1".into(), shape_pt_lat: Some(41.0), shape_pt_lon: Some(29.00), shape_pt_sequence: Some(1), shape_dist_traveled: None, line: 2 },
            ShapePointRecord { shape_id: "S1".into(), shape_pt_lat: Some(41.1), shape_pt_lon: Some(29.00), shape_pt_sequence: Some(2), shape_dist_traveled: None, line: 3 },
            ShapePointRecord { shape_id: "S1".into(), shape_pt_lat: Some(41.1), shape_pt_lon: Some(29.01), shape_pt_sequence: Some(3), shape_dist_traveled: None, line: 4 },
            ShapePointRecord { shape_id: "S1".into(), shape_pt_lat: Some(41.0), shape_pt_lon: Some(29.01), shape_pt_sequence: Some(4), shape_dist_traveled: None, line: 5 },
        ];
        let result = analyze(&records, &empty_derived(), &default_config(), 20260514);
        assert!(
            !result.notices.iter().any(|n| n.rule_id == "STM_012" || n.rule_id == "STM_014"),
            "Patolojik shape projeksiyonu (27× kuş uçuşu) haversine'e düşmeli; STM_012/014 üretilmemeli; notices: {:?}",
            result.notices.iter().map(|n| (&n.rule_id, n.observed_value.as_deref())).collect::<Vec<_>>()
        );
    }

    #[test]
    fn moderate_detour_within_ratio_still_uses_shape() {
        // Makul detour: shape kuş uçuşunun 4× altında kalırsa shape mesafesi korunur.
        // A→B kuş uçuşu ~11.1 km; shape hafif kavisli ~13 km (≈1.2×) → 4× altında.
        // 13 km / 2 dk ≈ 390 km/h → 120 < 390 < 700 → STM_014 tetiklenir (shape kullanıldığının kanıtı).
        use crate::k2::shapes::ShapePointRecord;
        let mut records = records_with(
            vec![stop("A", 41.0, 29.0), stop("B", 41.1, 29.0)],
            vec![route("R1", 3)],
            vec![trip_sh("T1", "R1", "S1")],
            vec![
                stoptime("T1", 1, "A", (8, 0, 0), (8, 0, 0), 2),
                stoptime("T1", 2, "B", (8, 2, 0), (8, 2, 0), 3), // 2 dk
            ],
        );
        // A(41.0,29.0) → orta nokta hafif doğuya (41.05,29.03) → B(41.1,29.0): kavisli ama ~1.2×
        records.shapes = vec![
            ShapePointRecord { shape_id: "S1".into(), shape_pt_lat: Some(41.00), shape_pt_lon: Some(29.00), shape_pt_sequence: Some(1), shape_dist_traveled: None, line: 2 },
            ShapePointRecord { shape_id: "S1".into(), shape_pt_lat: Some(41.05), shape_pt_lon: Some(29.03), shape_pt_sequence: Some(2), shape_dist_traveled: None, line: 3 },
            ShapePointRecord { shape_id: "S1".into(), shape_pt_lat: Some(41.10), shape_pt_lon: Some(29.00), shape_pt_sequence: Some(3), shape_dist_traveled: None, line: 4 },
        ];
        let result = analyze(&records, &empty_derived(), &default_config(), 20260514);
        assert!(
            result.notices.iter().any(|n| n.rule_id == "STM_014"),
            "Makul detour (≈1.2× kuş uçuşu) shape mesafesini korumalı → STM_014 beklenir; notices: {:?}",
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
            vec![trip_sh("T1", "R1", "MISSING_SHAPE")],
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
        let t1 = trip("T1", "R1");
        let t2 = trip("T2", "R2");
        r.trips = vec![t1, t2];
        r.trip_interns = take_ti();
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
        r.trip_interns = take_ti();
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
            trip(&format!("T{i}"), &format!("R{i}"))
        }).collect();
        r.trip_interns = take_ti();
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
        r.trip_interns = take_ti();
        r.stop_times = stoptimes_vec;
        let result = analyze(&r, &empty_derived(), &default_config(), 20260514);
        assert!(result.notices.iter().any(|n| n.rule_id == "VAT_003"), "VAT_003 olmalı");
    }

    #[test]
    fn vat_003_does_not_cross_flag_distinct_patterns_on_same_route() {
        // Aynı route_id, iki ayrı desen: 6 kısa sefer (2 durak, 10dk) + 5 uzun sefer
        // (4 durak, 30dk). Eski route_id gruplaması uzun seferleri aykırı işaretlerdi;
        // desen (durak sayısı) gruplamasıyla her küme kendi içinde tutarlı → VAT_003 yok.
        let mut r = crate::k2::EntityRecords::default();
        r.stops = vec![
            stop("A", 41.0, 29.0), stop("B", 41.1, 29.1),
            stop("C", 41.2, 29.2), stop("D", 41.3, 29.3),
        ];
        r.routes = vec![route("R1", 3)];
        let mut trips_vec = Vec::new();
        let mut stoptimes_vec = Vec::new();
        for i in 0..6u32 {
            let tid = format!("S{i}");
            trips_vec.push(trip(&tid, "R1"));
            stoptimes_vec.push(stoptime(&tid, 1, "A", (8, 0, 0), (8, 0, 0), 2));
            stoptimes_vec.push(stoptime(&tid, 2, "B", (8, 10, 0), (8, 10, 0), 3));
        }
        for i in 0..5u32 {
            let tid = format!("L{i}");
            trips_vec.push(trip(&tid, "R1"));
            stoptimes_vec.push(stoptime(&tid, 1, "A", (9, 0, 0), (9, 0, 0), 2));
            stoptimes_vec.push(stoptime(&tid, 2, "B", (9, 10, 0), (9, 10, 0), 3));
            stoptimes_vec.push(stoptime(&tid, 3, "C", (9, 20, 0), (9, 20, 0), 4));
            stoptimes_vec.push(stoptime(&tid, 4, "D", (9, 30, 0), (9, 30, 0), 5));
        }
        r.trips = trips_vec;
        r.trip_interns = take_ti();
        r.stop_times = stoptimes_vec;
        let result = analyze(&r, &empty_derived(), &default_config(), 20260514);
        assert!(!result.notices.iter().any(|n| n.rule_id == "VAT_003"),
            "ayrı desenler çapraz aykırı işaretlenmemeli");
    }

    #[test]
    fn vat_003_groups_by_stop_sequence_not_count() {
        // Aynı route, shape yok, AYNI durak sayısı (2) ama FARKLI duraklar:
        // 6 sefer A→B (10dk) + 5 sefer A→C (30dk). Durak SAYISI anahtarı bunları
        // karıştırıp 30dk'ları aykırı işaretlerdi; durak SIRASI hash'iyle ayrılır → VAT_003 yok.
        let mut r = crate::k2::EntityRecords::default();
        r.stops = vec![
            stop("A", 41.0, 29.0), stop("B", 41.1, 29.1), stop("C", 41.5, 29.5),
        ];
        r.routes = vec![route("R1", 3)];
        let mut trips_vec = Vec::new();
        let mut stoptimes_vec = Vec::new();
        for i in 0..6u32 {
            let tid = format!("S{i}");
            trips_vec.push(trip(&tid, "R1"));
            stoptimes_vec.push(stoptime(&tid, 1, "A", (8, 0, 0), (8, 0, 0), 2));
            stoptimes_vec.push(stoptime(&tid, 2, "B", (8, 10, 0), (8, 10, 0), 3));
        }
        for i in 0..5u32 {
            let tid = format!("L{i}");
            trips_vec.push(trip(&tid, "R1"));
            stoptimes_vec.push(stoptime(&tid, 1, "A", (9, 0, 0), (9, 0, 0), 2));
            stoptimes_vec.push(stoptime(&tid, 2, "C", (9, 30, 0), (9, 30, 0), 3));
        }
        r.trips = trips_vec;
        r.trip_interns = take_ti();
        r.stop_times = stoptimes_vec;
        let result = analyze(&r, &empty_derived(), &default_config(), 20260514);
        assert!(!result.notices.iter().any(|n| n.rule_id == "VAT_003"),
            "aynı durak sayılı farklı desenler durak-sırasıyla ayrılmalı, çapraz işaretlenmemeli");
    }

    #[test]
    fn vat_003_same_shape_different_stop_patterns_do_not_cross_flag() {
        // Gerçek BART regresyonu: aynı shape_id kısa-dönüş (A→B, 10dk) ve tam sefer
        // (A→B→C→D, 30dk) tarafından paylaşılabilir. Shape tek başına desen değildir;
        // stop dizisi de anahtara katılmalı, aksi halde kısa seferler kitlesel FP olur.
        let mut r = crate::k2::EntityRecords::default();
        r.stops = vec![
            stop("A", 41.0, 29.0), stop("B", 41.1, 29.1),
            stop("C", 41.2, 29.2), stop("D", 41.3, 29.3),
        ];
        r.routes = vec![route("R1", 3)];
        let mut trips_vec = Vec::new();
        let mut stoptimes_vec = Vec::new();
        for i in 0..5u32 {
            let tid = format!("S{i}");
            trips_vec.push(trip_sh(&tid, "R1", "SHARED"));
            stoptimes_vec.push(stoptime(&tid, 1, "A", (8, 0, 0), (8, 0, 0), 2));
            stoptimes_vec.push(stoptime(&tid, 2, "B", (8, 10, 0), (8, 10, 0), 3));
        }
        for i in 0..5u32 {
            let tid = format!("L{i}");
            trips_vec.push(trip_sh(&tid, "R1", "SHARED"));
            stoptimes_vec.push(stoptime(&tid, 1, "A", (9, 0, 0), (9, 0, 0), 2));
            stoptimes_vec.push(stoptime(&tid, 2, "B", (9, 10, 0), (9, 10, 0), 3));
            stoptimes_vec.push(stoptime(&tid, 3, "C", (9, 20, 0), (9, 20, 0), 4));
            stoptimes_vec.push(stoptime(&tid, 4, "D", (9, 30, 0), (9, 30, 0), 5));
        }
        r.trips = trips_vec;
        r.trip_interns = take_ti();
        r.stop_times = stoptimes_vec;
        let result = analyze(&r, &empty_derived(), &default_config(), 20260514);
        assert!(!result.notices.iter().any(|n| n.rule_id == "VAT_003"),
            "aynı shape'i paylaşan farklı stop desenleri çapraz işaretlenmemeli");
    }

    #[test]
    fn vat_003_deseasonalizes_peak_offpeak_within_pattern() {
        // Tek desen (A→B), tek route, shape yok. Baskın off-peak küme (saat 10, ~20dk),
        // meşru daha uzun peak (saat 8, 28dk), 1 gerçek olay (saat 10, 40dk).
        // Saat-bazlı deseasonalize ile peak FP üretmemeli; yalnızca olay işaretlenmeli.
        // (Eski global medyan/MAD bu kurguda tüm peak seferlerini FP olarak işaretlerdi.)
        let mut r = crate::k2::EntityRecords::default();
        r.stops = vec![stop("A", 41.0, 29.0), stop("B", 41.1, 29.1)];
        r.routes = vec![route("R1", 3)];
        let mut trips_vec = Vec::new();
        let mut sts = Vec::new();
        // Off-peak: saat 10, 19/20/21dk (hafif spread), 20 sefer — baskın küme.
        for i in 0..20u32 {
            let tid = format!("O{i}");
            trips_vec.push(trip(&tid, "R1"));
            let mins = 19 + (i % 3);
            sts.push(stoptime(&tid, 1, "A", (10, 0, 0), (10, 0, 0), 2));
            sts.push(stoptime(&tid, 2, "B", (10, mins, 0), (10, mins, 0), 3));
        }
        // Peak: saat 8, 28dk, 6 sefer — meşru uzun.
        for i in 0..6u32 {
            let tid = format!("P{i}");
            trips_vec.push(trip(&tid, "R1"));
            sts.push(stoptime(&tid, 1, "A", (8, 0, 0), (8, 0, 0), 2));
            sts.push(stoptime(&tid, 2, "B", (8, 28, 0), (8, 28, 0), 3));
        }
        // Olay: saat 10, 40dk.
        trips_vec.push(trip("INC", "R1"));
        sts.push(stoptime("INC", 1, "A", (10, 2, 0), (10, 2, 0), 2));
        sts.push(stoptime("INC", 2, "B", (10, 42, 0), (10, 42, 0), 3));
        r.trips = trips_vec;
        r.trip_interns = take_ti();
        r.stop_times = sts;
        let result = analyze(&r, &empty_derived(), &default_config(), 20260514);
        let flagged: std::collections::HashSet<&str> = result.notices.iter()
            .filter(|n| n.rule_id == "VAT_003")
            .filter_map(|n| n.entity_id.as_deref())
            .collect();
        assert!(flagged.contains("INC"), "gerçek olay (40dk) işaretlenmeli");
        assert!(!flagged.iter().any(|id| id.starts_with('P')),
            "meşru peak seferleri saat-bazlı deseasonalize ile işaretlenmemeli");
    }

    #[test]
    fn vat_003_owl_trips_banded_by_clock_time_not_service_day() {
        // Issue #26: gece yarısını aşan (owl) seferler servis-günü notasyonunda 24:xx+ ile
        // kodlanır. Band fonksiyonu (% 24 olmadan) bunları akşam (band 4) yerine yanlış banda
        // koyardı: gece seferi 20dk normaldir ama akşam medyanı (40dk) ile kıyaslanınca FP olur.
        // % 24 ile owl seferleri gerçek gece bandına (band 0) döner.
        let mut r = crate::k2::EntityRecords::default();
        r.stops = vec![stop("A", 41.0, 29.0), stop("B", 41.1, 29.1)];
        r.routes = vec![route("R1", 3)];
        let mut trips_vec = Vec::new();
        let mut sts = Vec::new();
        // Gece bandı (saat 2, 20dk), 5 sefer.
        for i in 0..5u32 {
            let tid = format!("N{i}");
            trips_vec.push(trip(&tid, "R1"));
            sts.push(stoptime(&tid, 1, "A", (2, 0, 0), (2, 0, 0), 2));
            sts.push(stoptime(&tid, 2, "B", (2, 20, 0), (2, 20, 0), 3));
        }
        // Akşam bandı (saat 21, 40dk — gece trafiğine göre uzun), 6 sefer.
        for i in 0..6u32 {
            let tid = format!("E{i}");
            trips_vec.push(trip(&tid, "R1"));
            sts.push(stoptime(&tid, 1, "A", (21, 0, 0), (21, 0, 0), 2));
            sts.push(stoptime(&tid, 2, "B", (21, 40, 0), (21, 40, 0), 3));
        }
        // Owl seferleri: 24:30 kalkış (00:30 ertesi gün), gece rejimi için NORMAL 20dk.
        for i in 0..3u32 {
            let tid = format!("W{i}");
            trips_vec.push(trip(&tid, "R1"));
            sts.push(stoptime(&tid, 1, "A", (24, 30, 0), (24, 30, 0), 2));
            sts.push(stoptime(&tid, 2, "B", (24, 50, 0), (24, 50, 0), 3));
        }
        // Owl olay: 24:35 kalkış, 40dk — gece bandı için ANORMAL.
        trips_vec.push(trip("WINC", "R1"));
        sts.push(stoptime("WINC", 1, "A", (24, 35, 0), (24, 35, 0), 2));
        sts.push(stoptime("WINC", 2, "B", (25, 15, 0), (25, 15, 0), 3));
        r.trips = trips_vec;
        r.trip_interns = take_ti();
        r.stop_times = sts;
        let result = analyze(&r, &empty_derived(), &default_config(), 20260514);
        let flagged: std::collections::HashSet<&str> = result.notices.iter()
            .filter(|n| n.rule_id == "VAT_003")
            .filter_map(|n| n.entity_id.as_deref())
            .collect();
        // % 24 ile owl-normal seferleri gece bandına düşer → FP yok.
        assert!(!flagged.iter().any(|id| id.starts_with('W') && *id != "WINC"),
            "normal owl seferleri gece bandına göre kıyaslanmalı (yanlış akşam-FP olmamalı)");
        // Gerçek owl anomalisi gece bandına göre yine yakalanmalı.
        assert!(flagged.contains("WINC"),
            "gece bandı için anormal (40dk) owl seferi işaretlenmeli");
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
            trip_svc(&format!("T{i}"), "WEEKDAY")
        }).collect();
        r.trip_interns = take_ti();
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
        r.trip_interns = take_ti();
        r.stop_times = st_v;
        let result = analyze(&r, &empty_derived(), &default_config(), 20260514);
        assert!(result.notices.iter().any(|n| n.rule_id == "VAT_005"), "VAT_005 olmalı");
    }

    #[test]
    fn isolated_cluster_near_main_network_suppressed_vat_005() {
        // İzole küme ana şebekeye coğrafi olarak ÇOK yakın (parent_station'sız çoklu
        // peron senaryosu, ör. otogar) → fiziksel olarak aynı yer → VAT_005 bastırılır.
        let mut r = crate::k2::EntityRecords::default();
        r.stops = vec![
            stop("A",41.0,29.0), stop("B",41.1,29.0), stop("C",41.2,29.0), stop("D",41.3,29.0),
            stop("E",41.4,29.0), stop("F",41.5,29.0),
            stop("P",41.0003,29.0), stop("Q",41.0005,29.0), // A'ya ~33m: aynı fiziksel yer
        ];
        r.routes = vec![route("R1",3), route("R2",3)];
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
        // İzole küme P-Q: ana ağa trip bağı yok ama P koordinatı A'ya bitişik (<200m).
        trips_v.push(trip("ISO","R2"));
        st_v.push(stoptime("ISO",1,"P",(8,0,0),(8,0,0),2));
        st_v.push(stoptime("ISO",2,"Q",(8,10,0),(8,10,0),3));
        r.trips = trips_v;
        r.trip_interns = take_ti();
        r.stop_times = st_v;
        let result = analyze(&r, &empty_derived(), &default_config(), 20260514);
        assert!(!result.notices.iter().any(|n| n.rule_id == "VAT_005"),
            "İzole küme ana şebekeye <200m yakın → VAT_005 bastırılmalı");
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
        r.trip_interns = take_ti();
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
        r.trip_interns = take_ti();
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

    #[test]
    fn adjacent_repeated_stop_is_owned_by_stm_035_not_opr_007() {
        let records = records_with(
            vec![stop("A", 41.0, 29.0), stop("B", 41.1, 29.1)],
            vec![route("R1", 3)],
            vec![trip("T1", "R1")],
            vec![
                stoptime("T1", 1, "A", (8, 0, 0), (8, 0, 0), 2),
                stoptime("T1", 2, "A", (8, 1, 0), (8, 1, 0), 3),
                stoptime("T1", 3, "B", (8, 10, 0), (8, 10, 0), 4),
            ],
        );
        let result = analyze(&records, &empty_derived(), &default_config(), 20260514);
        assert!(result.notices.iter().any(|n| n.rule_id == "STM_035"));
        assert!(!result.notices.iter().any(|n| n.rule_id == "OPR_007"),
            "ardışık tekrar OPR_007 ile ikinci kez raporlanmamalı");
    }

    #[test]
    fn late_night_trips_are_summarized_once_per_route() {
        let records = records_with(
            vec![stop("A", 41.0, 29.0), stop("B", 41.1, 29.1)],
            vec![route("R1", 3)], vec![trip("T1", "R1"), trip("T2", "R1")],
            vec![
                stoptime("T1", 1, "A", (23, 0, 0), (23, 0, 0), 2),
                stoptime("T1", 2, "B", (23, 10, 0), (23, 10, 0), 3),
                stoptime("T2", 1, "A", (24, 0, 0), (24, 0, 0), 4),
                stoptime("T2", 2, "B", (24, 10, 0), (24, 10, 0), 5),
            ],
        );
        let result = analyze(&records, &empty_derived(), &default_config(), 20260514);
        let found: Vec<_> = result.notices.iter().filter(|n| n.rule_id == "OPR_009").collect();
        assert_eq!(found.len(), 1);
        assert_eq!(found[0].entity_type, EntityType::Route);
        assert!(found[0].observed_value.as_deref().unwrap_or("").contains("2 sefer"));
    }

    // ── TRP_033 · blokta karışık route_type ───────────────────────────────────

    /// `trip()` + block_id ataması. `block_idx == 0` "block_id yok" demektir, bu yüzden
    /// intern tablosunun 0. yuvası boş bırakılır ve gerçek değerler 1'den başlar.
    fn blocked_trip(trip_id: &str, route_id: &str, block_id: &str) -> TripRecord {
        let mut t = trip(trip_id, route_id);
        t.block_idx = TEST_TI.with(|cell| {
            let mut ti = cell.borrow_mut();
            if ti.block_ids.is_empty() { ti.block_ids.push(SmolStr::default()); }
            match ti.block_ids.iter().position(|b| b == block_id) {
                Some(i) => i as u32,
                None => { ti.block_ids.push(SmolStr::new(block_id)); (ti.block_ids.len() - 1) as u32 }
            }
        });
        t
    }

    #[test]
    fn mixed_route_types_in_one_block_produce_trp_033() {
        // Aynı blok: biri otobüs (3), biri tramvay (0) → araç mod değiştiremez.
        let records = records_with(
            vec![stop("A", 41.0, 29.0), stop("B", 41.1, 29.1)],
            vec![route("BUS", 3), route("TRAM", 0)],
            vec![blocked_trip("T1", "BUS", "B1"), blocked_trip("T2", "TRAM", "B1")],
            vec![
                stoptime("T1", 1, "A", (8, 0, 0), (8, 0, 0), 2),
                stoptime("T1", 2, "B", (8, 10, 0), (8, 10, 0), 3),
                stoptime("T2", 1, "A", (9, 0, 0), (9, 0, 0), 4),
                stoptime("T2", 2, "B", (9, 10, 0), (9, 10, 0), 5),
            ],
        );
        let result = analyze(&records, &empty_derived(), &default_config(), 20260514);
        let hits: Vec<_> = result.notices.iter().filter(|n| n.rule_id == "TRP_033").collect();
        assert_eq!(hits.len(), 1, "blok başına tek notice beklenir");
        assert_eq!(hits[0].entity_id.as_deref(), Some("B1"), "notice bloğa ait olmalı");
        assert_eq!(hits[0].observed_value.as_deref(), Some("0, 3"), "route_type'lar sıralı listelenmeli");
    }

    #[test]
    fn consistent_block_produces_no_trp_033() {
        let records = records_with(
            vec![stop("A", 41.0, 29.0), stop("B", 41.1, 29.1)],
            vec![route("R1", 3), route("R2", 3)], // iki farklı hat, AYNI mod
            vec![blocked_trip("T1", "R1", "B1"), blocked_trip("T2", "R2", "B1")],
            vec![
                stoptime("T1", 1, "A", (8, 0, 0), (8, 0, 0), 2),
                stoptime("T1", 2, "B", (8, 10, 0), (8, 10, 0), 3),
                stoptime("T2", 1, "A", (9, 0, 0), (9, 0, 0), 4),
                stoptime("T2", 2, "B", (9, 10, 0), (9, 10, 0), 5),
            ],
        );
        let result = analyze(&records, &empty_derived(), &default_config(), 20260514);
        assert!(result.notices.iter().all(|n| n.rule_id != "TRP_033"),
            "aynı moddaki blok bulgu ÜRETMEMELİ");
    }

    #[test]
    fn trp_033_reports_each_block_once_not_each_trip() {
        // Tek blokta 6 sefer karışık mod → 6 değil 1 notice (STM_014 dersi).
        let mut trips = Vec::new();
        let mut times = Vec::new();
        for i in 0..6u32 {
            let id = format!("T{i}");
            trips.push(blocked_trip(&id, if i % 2 == 0 { "BUS" } else { "TRAM" }, "B1"));
            times.push(stoptime(&id, 1, "A", (8, 0, 0), (8, 0, 0), (i * 2 + 2) as u64));
            times.push(stoptime(&id, 2, "B", (8, 10, 0), (8, 10, 0), (i * 2 + 3) as u64));
        }
        let records = records_with(
            vec![stop("A", 41.0, 29.0), stop("B", 41.1, 29.1)],
            vec![route("BUS", 3), route("TRAM", 0)], trips, times,
        );
        let result = analyze(&records, &empty_derived(), &default_config(), 20260514);
        assert_eq!(result.notices.iter().filter(|n| n.rule_id == "TRP_033").count(), 1);
    }

    #[test]
    fn trips_without_block_id_are_ignored_by_trp_033() {
        let records = records_with(
            vec![stop("A", 41.0, 29.0), stop("B", 41.1, 29.1)],
            vec![route("BUS", 3), route("TRAM", 0)],
            vec![trip("T1", "BUS"), trip("T2", "TRAM")], // block_id YOK
            vec![
                stoptime("T1", 1, "A", (8, 0, 0), (8, 0, 0), 2),
                stoptime("T1", 2, "B", (8, 10, 0), (8, 10, 0), 3),
                stoptime("T2", 1, "A", (9, 0, 0), (9, 0, 0), 4),
                stoptime("T2", 2, "B", (9, 10, 0), (9, 10, 0), 5),
            ],
        );
        let result = analyze(&records, &empty_derived(), &default_config(), 20260514);
        assert!(result.notices.iter().all(|n| n.rule_id != "TRP_033"),
            "block_id'siz seferler bu kuralın konusu değil");
    }

    // ── ARC_020 · DRT muafiyeti ───────────────────────────────────────────────
    // shapes.txt sabit hat geometrisi içindir; tümüyle talep-duyarlı bir feed'de
    // beklenmez. Muafiyet KASITLI olarak sıkı: tek bir sabit duraklı sefer bile
    // varsa shapes.txt beklentisi sürer.

    /// `stoptime` yardımcısının DRT karşılığı: stop_id boş, hizmet alanı flex alanla verilir.
    fn drt_stoptime(
        trip_id: &str, seq: u32, location_id: Option<&str>, location_group_id: Option<&str>, line: u64,
    ) -> StopTimeRecord {
        StopTimeRecord {
            trip_id: trip_id.into(),
            stop_id: "".into(),
            stop_sequence: Some(seq),
            location_id: location_id.map(|s| s.into()),
            location_group_id: location_group_id.map(|s| s.into()),
            line,
            ..Default::default()
        }
    }

    fn feed_info_row() -> crate::k2::feed_info::FeedInfoRecord {
        crate::k2::feed_info::FeedInfoRecord {
            feed_publisher_name: "P".into(),
            feed_publisher_url: "https://example.org".into(),
            feed_lang: "tr".into(),
            feed_start_date: None,
            feed_end_date: None,
            feed_version: None,
            feed_contact_email: None,
            feed_contact_url: None,
            row: Default::default(),
            line: 2,
        }
    }

    fn arc_020_files(records: &crate::k2::EntityRecords) -> Vec<String> {
        let result = analyze(records, &empty_derived(), &default_config(), 20260514);
        result.notices.iter()
            .filter(|n| n.rule_id == "ARC_020")
            .map(|n| n.file.clone().unwrap_or_default())
            .collect()
    }

    #[test]
    fn arc_020_reports_missing_shapes_on_fixed_stop_feed() {
        let mut records = records_with(
            vec![stop("A", 41.0, 29.0), stop("B", 41.1, 29.1)],
            vec![route("R1", 3)], vec![trip("T1", "R1")],
            vec![
                stoptime("T1", 1, "A", (8, 0, 0), (8, 0, 0), 2),
                stoptime("T1", 2, "B", (8, 10, 0), (8, 10, 0), 3),
            ],
        );
        records.feed_info = vec![feed_info_row()];
        assert_eq!(arc_020_files(&records), vec!["shapes.txt".to_string()],
            "sabit duraklı feed'de shapes.txt eksikliği raporlanmalı");
    }

    #[test]
    fn arc_020_skips_shapes_for_zone_based_drt() {
        use crate::k2::stop_times::StopTimesIndex;
        let stoptimes = vec![
            drt_stoptime("T1", 1, Some("zone_1"), None, 2),
            drt_stoptime("T1", 2, Some("zone_2"), None, 3),
        ];
        let mut records = records_with(
            vec![stop("A", 41.0, 29.0)], vec![route("R1", 3)], vec![trip("T1", "R1")],
            stoptimes.clone(),
        );
        records.stop_times_index = StopTimesIndex::from_records(&stoptimes);
        records.feed_info = vec![feed_info_row()];
        assert!(arc_020_files(&records).is_empty(),
            "alan-tabanlı DRT feed'inde shapes.txt beklenmez");
    }

    #[test]
    fn arc_020_skips_shapes_for_fixed_stops_drt() {
        use crate::k2::stop_times::StopTimesIndex;
        let stoptimes = vec![
            drt_stoptime("T1", 1, None, Some("lg_1"), 2),
            drt_stoptime("T1", 2, None, Some("lg_2"), 3),
        ];
        let mut records = records_with(
            vec![stop("A", 41.0, 29.0)], vec![route("R1", 3)], vec![trip("T1", "R1")],
            stoptimes.clone(),
        );
        records.stop_times_index = StopTimesIndex::from_records(&stoptimes);
        records.feed_info = vec![feed_info_row()];
        records.location_groups = vec![crate::k2::location_groups::LocationGroupRecord {
            location_group_id: "lg_1".into(), line: 2,
        }];
        assert!(arc_020_files(&records).is_empty(),
            "sabit-duraklı DRT feed'inde shapes.txt beklenmez");
    }

    #[test]
    fn arc_020_still_reports_missing_shapes_when_drt_is_partial() {
        use crate::k2::stop_times::StopTimesIndex;
        // Tek bir sabit duraklı sefer → feed DRT-only değil, shapes.txt beklentisi sürer.
        let stoptimes = vec![
            drt_stoptime("T1", 1, Some("zone_1"), None, 2),
            stoptime("T2", 1, "A", (8, 0, 0), (8, 0, 0), 3),
            stoptime("T2", 2, "B", (8, 10, 0), (8, 10, 0), 4),
        ];
        let mut records = records_with(
            vec![stop("A", 41.0, 29.0), stop("B", 41.1, 29.1)],
            vec![route("R1", 3)], vec![trip("T1", "R1"), trip("T2", "R1")],
            stoptimes.clone(),
        );
        records.stop_times_index = StopTimesIndex::from_records(&stoptimes);
        records.feed_info = vec![feed_info_row()];
        assert_eq!(arc_020_files(&records), vec!["shapes.txt".to_string()],
            "karışık feed'de shapes.txt eksikliği raporlanmalı");
    }

    #[test]
    fn arc_020_still_reports_missing_feed_info_on_drt_feed() {
        use crate::k2::stop_times::StopTimesIndex;
        // DRT muafiyeti YALNIZ shapes.txt kolunu düşürür; feed_info.txt beklentisi sürer.
        let stoptimes = vec![drt_stoptime("T1", 1, Some("zone_1"), None, 2)];
        let mut records = records_with(
            vec![stop("A", 41.0, 29.0)], vec![route("R1", 3)], vec![trip("T1", "R1")],
            stoptimes.clone(),
        );
        records.stop_times_index = StopTimesIndex::from_records(&stoptimes);
        assert_eq!(arc_020_files(&records), vec!["feed_info.txt".to_string()],
            "DRT feed'inde bile feed_info.txt eksikliği raporlanmalı");
    }
}
