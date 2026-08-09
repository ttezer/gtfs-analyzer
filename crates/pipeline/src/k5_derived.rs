use std::collections::{HashMap, HashSet};

use gtfs_core::{EntityType, Notice};

use crate::k2::EntityRecords;
use crate::k3_entity_graph::EntityMap;

// ── Çıktı tipleri ─────────────────────────────────────────────────────────────

/// K5'te hesaplanan shape geometry verileri.
#[derive(Debug, Default)]
pub struct ShapeGeometry {
    /// shape_id → segment verileri (sıralı nokta çiftleri arası Haversine km, toplam uzunluk, bbox)
    pub shapes: HashMap<String, ShapeSegments>,
}

#[derive(Debug, Clone)]
pub struct ShapeSegments {
    /// Ardışık nokta çiftleri arası mesafe (km). Uzunluk = nokta sayısı - 1.
    pub segment_distances_km: Vec<f64>,
    /// Toplam polyline uzunluğu (km).
    pub total_length_km: f64,
    /// (min_lat, max_lat, min_lon, max_lon)
    pub bbox: (f64, f64, f64, f64),
}

/// K5 takvim bitmap — K6 takvim kuralları tarafından tüketilir.
#[derive(Debug, Default)]
pub struct CalendarBitmap {
    /// service_id → aktif YYYYMMDD günleri kümesi
    pub active_dates: HashMap<String, HashSet<u32>>,
}

/// K5 uzamsal indeks — K6 STP_016/017 tarafından tüketilir.
#[derive(Debug, Default)]
pub struct SpatialIndex {
    /// Grid-tabanlı durak koordinat indeksi: (grid_row, grid_col) → stop Vec indeksleri
    pub grid: HashMap<(i32, i32), Vec<usize>>,
    pub cell_deg: f64,
}

/// K5 pathway graph — K6 PTH_012–015 tarafından tüketilir.
#[derive(Debug, Default)]
pub struct PathwayGraph {
    /// stop_id → komşu (stop_id, pathway_idx) listesi
    pub adjacency: HashMap<String, Vec<(String, usize)>>,
}

/// K5 fare network — fare_id → routes ve zone referansları.
#[derive(Debug, Default)]
pub struct FareNetwork {
    /// fare_id → fare_rules'tan gelen route_id listesi
    pub fare_routes: HashMap<String, Vec<String>>,
    /// fare_rules'ta kullanılan tüm zone ID'leri (origin/destination/contains)
    pub zone_ids: HashSet<String>,
}

/// K5 türev veri yapılarının kapsayıcısı.
#[derive(Debug, Default)]
pub struct DerivedData {
    pub shape_geometry: ShapeGeometry,
    pub calendar_bitmap: CalendarBitmap,
    pub spatial_index: SpatialIndex,
    pub pathway_graph: PathwayGraph,
    pub fare_network: FareNetwork,
}

/// K5 çıktısı.
#[derive(Debug, Default)]
pub struct K5Result {
    pub derived: DerivedData,
    pub notices: Vec<Notice>,
}

// ── Ana fonksiyon ─────────────────────────────────────────────────────────────

pub fn build(records: &EntityRecords, entity_map: &EntityMap) -> K5Result {
    use crate::timing::Timer;
    let mut derived = DerivedData::default();
    let mut notices = Vec::new();
    let mut ctr = 0u32;

    { let _t = Timer::start("K5::shape_geometry");   build_shape_geometry(records, entity_map, &mut derived, &mut notices, &mut ctr); }
    { let _t = Timer::start("K5::calendar_bitmap");  build_calendar_bitmap(records, &mut derived); }
    { let _t = Timer::start("K5::spatial_index");    build_spatial_index(records, &mut derived); }
    { let _t = Timer::start("K5::pathway_graph");    build_pathway_graph(records, &mut derived); }
    { let _t = Timer::start("K5::fare_network");     build_fare_network(records, &mut derived); }

    K5Result { derived, notices }
}

// ── Notice yardımcısı ─────────────────────────────────────────────────────────

// K5 notice'ları canonical emit tablosuyla aynı sırada kurulur; bu ortak yardımcıda
// parametre struct'ı mevcut rule çağrılarını daha güvenli veya daha okunur yapmaz.
#[allow(clippy::too_many_arguments)]
fn k5_notice(
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
        "K5", Some("k5"), ctr, rule_id, entity_type, entity_id, scope_key,
        Some(file.to_string()), line, field.map(str::to_string),
        observed, expected, message, remediation,
    )
}

// ── Haversine mesafe (km) ─────────────────────────────────────────────────────

pub(crate) fn haversine_km(lat1: f64, lon1: f64, lat2: f64, lon2: f64) -> f64 {
    const R: f64 = 6371.0;
    let dlat = (lat2 - lat1).to_radians();
    let dlon = (lon2 - lon1).to_radians();
    let a = (dlat / 2.0).sin().powi(2)
        + lat1.to_radians().cos() * lat2.to_radians().cos() * (dlon / 2.0).sin().powi(2);
    2.0 * R * a.sqrt().asin()
}

// ── WP-08b: Calendar bitmap ───────────────────────────────────────────────────

/// YYYYMMDD tuple → Julian Day Number.
pub(crate) fn ymd_to_jdn(y: u32, m: u32, d: u32) -> u32 {
    let a = (14u32.wrapping_sub(m)) / 12;
    let yr = y + 4800 - a;
    let mo = m + 12 * a - 3;
    d + (153 * mo + 2) / 5 + 365 * yr + yr / 4 - yr / 100 + yr / 400 - 32045
}

/// Julian Day Number → YYYYMMDD.
pub(crate) fn jdn_to_yyyymmdd(jdn: u32) -> u32 {
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

fn build_calendar_bitmap(records: &EntityRecords, derived: &mut DerivedData) {
    // calendar.txt: base schedule
    for rec in &records.calendars {
        if rec.service_id.is_empty() {
            continue;
        }
        let (start, end) = match (rec.start_date, rec.end_date) {
            (Some(s), Some(e)) => (s, e),
            _ => continue,
        };
        let start_jdn = ymd_to_jdn(start.0, start.1, start.2);
        let end_jdn = ymd_to_jdn(end.0, end.1, end.2);
        if end_jdn < start_jdn {
            continue; // CAL_005 K2'de zaten yakalandı
        }

        let set = derived
            .calendar_bitmap
            .active_dates
            .entry(rec.service_id.to_string())
            .or_default();

        let mut jdn = start_jdn;
        while jdn <= end_jdn {
            // JDN % 7: 0=Mon … 6=Sun — calendar.txt days[] sırası aynı
            let dow = (jdn % 7) as usize;
            if rec.days[dow] == Some(1) {
                set.insert(jdn_to_yyyymmdd(jdn));
            }
            jdn += 1;
        }
    }

    // calendar_dates.txt: exception overlay.
    // Önce tüm exception_type=1 (added) tarihleri ekle, ardından exception_type=2 (removed) kaldır.
    // ⚠️ Çakışma davranışı: aynı (service_id, date) hem added hem removed'daysa removed kazanır
    // (CSV satır sırasından bağımsız). Geçerli GTFS feed'lerinde bu çakışma olmamalı.
    for (svc, dates) in &records.calendar_dates.added {
        let set = derived.calendar_bitmap.active_dates.entry(svc.to_string()).or_default();
        for &d in dates { set.insert(d); }
    }
    for (svc, dates) in &records.calendar_dates.removed {
        if let Some(set) = derived.calendar_bitmap.active_dates.get_mut(svc.as_str()) {
            for &d in dates { set.remove(&d); }
        }
    }
}

// ── WP-08a: Shape geometry ────────────────────────────────────────────────────

fn build_shape_geometry(
    records: &EntityRecords,
    entity_map: &EntityMap,
    derived: &mut DerivedData,
    notices: &mut Vec<Notice>,
    ctr: &mut u32,
) {
    // Hangi shape_id'lerin trip tarafından kullanıldığını topla (SHP_018 için)
    let ti = &records.trip_interns;
    let referenced_shapes: HashSet<&str> = records
        .trips
        .iter()
        .filter_map(|t| ti.shape_id(t))
        .filter(|s| !s.is_empty())
        .collect();

    // SHP_010 agregasyonu (#48): ardışık özdeş koordinat büyük feed'de neredeyse-evrensel
    // (DELFI: 268K shape'in %98'i) — shape-başına Düşük notice yerine eşik aşılırsa tek
    // feed-özet üretilir. STP_022 emsali; karar dış döngü sonunda verilir.
    let mut shp010_pending: Vec<Notice> = Vec::new();

    for (shape_id, point_indices) in &entity_map.shape_points {
        let n_pts = point_indices.len();
        if n_pts == 0 { continue; }
        let first_line = point_indices.first().map(|&i| records.shapes[i].line_u64());
        if n_pts == 1 {
            // SHP_006: tek noktalı shape
            notices.push(k5_notice(ctr, "SHP_006", EntityType::Shape,
                Some(shape_id.clone()), Some(shape_id.clone()),
                "shapes.txt", first_line, Some("shape_pt_sequence"),
                Some("1".to_string()), Some(">= 2".to_string()),
                format!("'{}' güzergah şekli yalnızca 1 noktadan oluşuyor; en az 2 nokta gerekir.", shape_id),
                "shapes.txt'e bu shape_id için en az bir nokta daha ekleyin.",
            ));
            continue;
        }
        // n_pts == 2: geçerli düz segment (GTFS ≥3 nokta dayatmaz; feribat/kısa
        // düz hat meşru). Geometri aşağıda tek segment olarak hesaplanır.

        let mut segment_distances_km: Vec<f64> = Vec::with_capacity(point_indices.len() - 1);
        let mut total_km = 0.0f64;
        let mut min_lat = f64::MAX;
        let mut max_lat = f64::MIN;
        let mut min_lon = f64::MAX;
        let mut max_lon = f64::MIN;

        let mut prev_lat: Option<f64> = None;
        let mut prev_lon: Option<f64> = None;
        let mut prev_line: u64 = 0;
        let mut prev_dist: Option<f64> = None;
        // SHP_010 Entity-scope (shape_id) → dedup shape başına TEKE çökertir. Büyük feed'de
        // bir shape'in her ardışık-tekrar noktası için emit etmek yüz binlerce ara-notice
        // üretip sonra atıyordu (#15 Mode A). Shape başına bir kez emit = birebir aynı sonuç.
        let mut shp010_fired = false;

        for &idx in point_indices {
            let pt = &records.shapes[idx];

            // SHP_005: shape_dist_traveled shape_pt_sequence ile artmalı. point_indices K3'te
            // sequence'a göre SIRALI olduğundan burada kontrol edilir — K2 dosya-satır sırasında
            // baksaydı (ve bakıyordu) sequence-dışı sıralı feed'lerde sahte "azalma" görürdü (FP fix).
            // Karşılaştırma TAM (#94): eskiden `d < prev - 1e-6` idi ve 1e-6'dan küçük GERÇEK
            // azalmaları sessizce kabul ediyordu — kritik bir Spec predikatının kabul kümesini
            // spec'te olmayan bir toleransla genişletmek. Tolerans gerekmiyor: değerler
            // `parse_f64_col` ile ÜRETİCİNİN yazdığı metinden doğrudan okunuyor, üzerlerinde
            // aritmetik yapılmıyor. Ondalık→f64 dönüşümü monoton olduğundan `d < prev` ancak
            // ondalık değer de azalmışsa doğrudur (FP yok); ayırt edilemeyecek kadar yakın iki
            // değer eşitliğe çöker ve eşitlik zaten emit etmez.
            if let Some(d) = pt.shape_dist_traveled() {
                if let Some(prev) = prev_dist {
                    if d < prev {
                        notices.push(k5_notice(
                            ctr, "SHP_005", EntityType::Shape,
                            Some(shape_id.clone()), Some(shape_id.clone()),
                            "shapes.txt", Some(pt.line_u64()), Some("shape_dist_traveled"),
                            Some(format!("{d}")), Some(format!("≥ {prev} (önceki sıra)")),
                            format!("shape_id '{shape_id}' için shape_dist_traveled shape_pt_sequence ile azalıyor: {prev} → {d}."),
                            "shape_dist_traveled değerlerini her shape için shape_pt_sequence'e göre artan yazın.",
                        ));
                    }
                }
                prev_dist = Some(d);
            }

            let (lat, lon) = match (pt.shape_pt_lat(), pt.shape_pt_lon()) {
                (Some(la), Some(lo)) => (la, lo),
                _ => {
                    // lat/lon yoksa geometri için bu noktayı atla; SHP_003/004 K2'de yakalandı.
                    prev_lat = None;
                    prev_lon = None;
                    continue;
                }
            };

            // Bounding box güncelle
            min_lat = min_lat.min(lat);
            max_lat = max_lat.max(lat);
            min_lon = min_lon.min(lon);
            max_lon = max_lon.max(lon);

            if let (Some(plat), Some(plon)) = (prev_lat, prev_lon) {
                // SHP_010: ardışık özdeş koordinat — shape başına bir kez (dedup zaten teke indirir)
                if (lat - plat).abs() < f64::EPSILON && (lon - plon).abs() < f64::EPSILON && !shp010_fired {
                    shp010_fired = true;
                    shp010_pending.push(k5_notice(
                        ctr,
                        "SHP_010",
                        EntityType::Shape,
                        Some(shape_id.clone()),
                        Some(shape_id.clone()),
                        "shapes.txt",
                        Some(pt.line_u64()),
                        Some("shape_pt_lat|shape_pt_lon"),
                        Some(format!("({lat},{lon})")),
                        None,
                        format!(
                            "shape_id '{shape_id}' satır {} ile {} arasında ardışık özdeş koordinat.",
                            prev_line, pt.line
                        ),
                        "Tekrarlanan güzergah noktasını kaldırın.",
                    ));
                }

                let d = haversine_km(plat, plon, lat, lon);
                segment_distances_km.push(d);
                total_km += d;
            }

            prev_lat = Some(lat);
            prev_lon = Some(lon);
            prev_line = pt.line_u64();
        }

        derived.shape_geometry.shapes.insert(
            shape_id.clone(),
            ShapeSegments {
                segment_distances_km,
                total_length_km: total_km,
                bbox: (min_lat, max_lat, min_lon, max_lon),
            },
        );
    }

    // SHP_010 karar (#48): ardışık-özdeş-koordinatlı shape sayısı eşiği aşarsa tek feed-özet;
    // aksi halde shape-başına notice korunur (düşük hacimde pinpoint değerli).
    const SHP010_AGG_THRESHOLD: usize = 50;
    let n10 = shp010_pending.len();
    if n10 > SHP010_AGG_THRESHOLD {
        // shp010_pending, shape_points (HashMap) iterasyon sırasında dolduğu için ham
        // `take(5)` her koşuda başka örnekler veriyordu. Önce sırala, sonra ilk 5'i al.
        let mut examples: Vec<String> = shp010_pending.iter()
            .filter_map(|x| x.entity_id.clone()).collect();
        examples.sort_unstable();
        examples.truncate(5);
        let mut notice = k5_notice(
            ctr, "SHP_010", EntityType::Feed, None, None,
            "shapes.txt", None, Some("shape_pt_lat|shape_pt_lon"),
            Some(n10.to_string()), None,
            format!("{n10} güzergah şeklinde ardışık özdeş koordinat var — tekrarlanan güzergah noktaları."),
            "İlgili shape'lerdeki tekrarlanan güzergah noktalarını kaldırın.",
        );
        let mut d = std::collections::BTreeMap::new();
        d.insert("affected_shapes".to_string(), n10.to_string());
        if !examples.is_empty() { d.insert("example_shapes".to_string(), examples.join(", ")); }
        notice.details = Some(d);
        notices.push(notice);
    } else {
        notices.append(&mut shp010_pending);
    }

    // SHP_018: orphan shape — trip tarafından referans edilmeyen shape
    for shape_id in entity_map.shape_points.keys() {
        if !referenced_shapes.contains(shape_id.as_str()) {
            // İlk noktanın satır numarasını bul
            let line = entity_map.shape_points[shape_id]
                .first()
                .map(|&i| records.shapes[i].line_u64());
            notices.push(k5_notice(
                ctr,
                "SHP_018",
                EntityType::Shape,
                Some(shape_id.clone()),
                Some(shape_id.clone()),
                "shapes.txt",
                line,
                Some("shape_id"),
                Some(shape_id.clone()),
                None,
                format!("'{}' güzergahı hiçbir sefer tarafından referans edilmiyor.", shape_id),
                "Kullanılmayan güzergah şeklini kaldırın ya da bir sefere atayın.",
            ));
        }
    }
}

// ── WP-08c: Spatial index ─────────────────────────────────────────────────────

const SPATIAL_CELL_DEG: f64 = 0.5;

fn build_spatial_index(records: &EntityRecords, derived: &mut DerivedData) {
    derived.spatial_index.cell_deg = SPATIAL_CELL_DEG;
    for (idx, stop) in records.stops.iter().enumerate() {
        let (lat, lon) = match (stop.stop_lat, stop.stop_lon) {
            (Some(la), Some(lo)) => (la, lo),
            _ => continue,
        };
        let row = (lat / SPATIAL_CELL_DEG).floor() as i32;
        let col = (lon / SPATIAL_CELL_DEG).floor() as i32;
        derived.spatial_index.grid.entry((row, col)).or_default().push(idx);
    }
}

// ── WP-08d: Pathway graph ─────────────────────────────────────────────────────

fn build_pathway_graph(records: &EntityRecords, derived: &mut DerivedData) {
    for (idx, pw) in records.pathways.iter().enumerate() {
        if pw.from_stop_id.is_empty() || pw.to_stop_id.is_empty() {
            continue;
        }
        derived
            .pathway_graph
            .adjacency
            .entry(pw.from_stop_id.clone())
            .or_default()
            .push((pw.to_stop_id.clone(), idx));

        if pw.is_bidirectional == Some(1) {
            derived
                .pathway_graph
                .adjacency
                .entry(pw.to_stop_id.clone())
                .or_default()
                .push((pw.from_stop_id.clone(), idx));
        }
    }
}

// ── WP-08d: Fare network ──────────────────────────────────────────────────────

fn build_fare_network(records: &EntityRecords, derived: &mut DerivedData) {
    for rule in &records.fare_rules {
        if rule.fare_id.is_empty() {
            continue;
        }
        if let Some(route_id) = rule.route_id.as_deref() {
            derived
                .fare_network
                .fare_routes
                .entry(rule.fare_id.clone())
                .or_default()
                .push(route_id.to_string());
        }
        for zone in [
            rule.origin_id.as_deref(),
            rule.destination_id.as_deref(),
            rule.contains_id.as_deref(),
        ]
        .into_iter()
        .flatten()
        {
            derived.fare_network.zone_ids.insert(zone.to_string());
        }
    }
}

// ── Testler ───────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::k2::fare_rules::FareRuleRecord;
    use crate::k2::pathways::PathwayRecord;
    use crate::k2::shapes::ShapePointRecord;
    use crate::k2::stops::StopRecord;
    use crate::k2::trips::TripRecord;
    use crate::k2::shapes::ShapeInternTable;

    fn empty_records() -> EntityRecords {
        EntityRecords::default()
    }

    fn empty_map() -> EntityMap {
        EntityMap::default()
    }

    fn shape_pt(ti: &mut ShapeInternTable, shape_id: &str, seq: u32, lat: f64, lon: f64, line: u64) -> ShapePointRecord {
        ShapePointRecord::new(ti.intern(shape_id), Some(lat), Some(lon), Some(seq), None, line as u32)
    }

    fn shape_pt_d(ti: &mut ShapeInternTable, shape_id: &str, seq: u32, lat: f64, lon: f64, dist: f64, line: u64) -> ShapePointRecord {
        ShapePointRecord::new(ti.intern(shape_id), Some(lat), Some(lon), Some(seq), Some(dist), line as u32)
    }

    fn trip_with_shape(trip_id: &str, shape_id: &str) -> (TripRecord, crate::k2::trips::TripInternTable) {
        use crate::k2::trips::TripInternTable;
        use smol_str::SmolStr;
        let mut ti = TripInternTable::new();
        let ri = ti.route_ids.len() as u32;
        ti.route_ids.push(SmolStr::new("R1"));
        let si = ti.service_ids.len() as u32;
        ti.service_ids.push(SmolStr::new("SVC"));
        let shi = ti.shape_ids.len() as u32;
        ti.shape_ids.push(SmolStr::new(shape_id));
        let trip = TripRecord {
            trip_id: trip_id.into(),
            route_idx: ri, service_idx: si, shape_idx: shi,
            headsign_idx: 0, short_name_idx: 0, block_idx: 0, jp_office_idx: 0,
            direction_id: None, wheelchair_accessible: None,
            bikes_allowed: None, cars_allowed: None,
            safe_duration_factor: None, safe_duration_offset: None,
            line: 2,
        };
        (trip, ti)
    }

    // ── Haversine ─────────────────────────────────────────────────────────────

    #[test]
    fn haversine_istanbul_ankara_approx_350km() {
        // İstanbul (~41.0, 29.0) → Ankara (~39.9, 32.9) ≈ 350 km
        let d = haversine_km(41.0, 29.0, 39.9, 32.9);
        assert!(d > 340.0 && d < 360.0, "Beklenen ~350 km, bulunan {d:.1}");
    }

    #[test]
    fn haversine_same_point_is_zero() {
        assert!((haversine_km(41.0, 29.0, 41.0, 29.0)).abs() < 1e-9);
    }

    // ── ShapeGeometry ─────────────────────────────────────────────────────────

    #[test]
    fn three_point_shape_produces_two_segments() {
        let mut shape_ti = ShapeInternTable::new();
        let mut records = empty_records();
        let mut map = empty_map();
        records.shapes = vec![
            shape_pt(&mut shape_ti, "S1", 1, 41.0, 29.0, 2),
            shape_pt(&mut shape_ti, "S1", 2, 41.1, 29.1, 3),
            shape_pt(&mut shape_ti, "S1", 3, 41.2, 29.2, 4),
        ];
        records.shape_interns = shape_ti.clone();
        let (t1, ti1) = trip_with_shape("T1", "S1");
        records.trips = vec![t1];
        records.trip_interns = ti1;
        map.shape_points.insert("S1".into(), vec![0, 1, 2]);
        map.trips.insert("T1".into(), 0);

        let result = build(&records, &map);
        let seg = result.derived.shape_geometry.shapes.get("S1").unwrap();
        assert_eq!(seg.segment_distances_km.len(), 2);
        assert!(seg.total_length_km > 0.0);
        assert!(result.notices.is_empty(), "Temiz shape'te notice üretilmemeli");
    }

    #[test]
    fn shp_005_no_false_positive_when_file_order_differs_from_sequence() {
        let mut shape_ti = ShapeInternTable::new();
        // Dosyada seq 2 (dist 20) seq 1 (dist 10)'dan ÖNCE listelenmiş; K3 sequence'a göre sıralar.
        // Sequence-sıralı dist artıyor → SHP_005 ÇIKMAMALI (Athens mdb-3220 FP'sinin köku).
        let mut records = empty_records();
        let mut map = empty_map();
        records.shapes = vec![
            shape_pt_d(&mut shape_ti, "S1", 2, 41.1, 29.1, 20.0, 2),
            shape_pt_d(&mut shape_ti, "S1", 1, 41.0, 29.0, 10.0, 3),
        ];
        records.shape_interns = shape_ti.clone();
        let (t1, ti1) = trip_with_shape("T1", "S1");
        records.trips = vec![t1]; records.trip_interns = ti1;
        map.shape_points.insert("S1".into(), vec![1, 0]); // sequence-sıralı: seq1, seq2
        map.trips.insert("T1".into(), 0);
        let result = build(&records, &map);
        assert!(!result.notices.iter().any(|n| n.rule_id == "SHP_005"),
            "sequence-sıralı artan shape_dist için SHP_005 çıkmamalı (FP fix)");
    }

    #[test]
    fn shp_005_fires_on_true_sequence_decrease() {
        let mut shape_ti = ShapeInternTable::new();
        // shape_pt_sequence sırasında gerçekten azalıyor (seq1=10 → seq2=5) → SHP_005 çıkmalı.
        let mut records = empty_records();
        let mut map = empty_map();
        records.shapes = vec![
            shape_pt_d(&mut shape_ti, "S1", 1, 41.0, 29.0, 10.0, 2),
            shape_pt_d(&mut shape_ti, "S1", 2, 41.1, 29.1, 5.0, 3),
        ];
        records.shape_interns = shape_ti.clone();
        let (t1, ti1) = trip_with_shape("T1", "S1");
        records.trips = vec![t1]; records.trip_interns = ti1;
        map.shape_points.insert("S1".into(), vec![0, 1]);
        map.trips.insert("T1".into(), 0);
        let result = build(&records, &map);
        assert!(result.notices.iter().any(|n| n.rule_id == "SHP_005"),
            "sequence sırasında azalan shape_dist SHP_005 üretmeli");
    }

    /// #94 sınır matrisi: azalma eski `1e-6` toleransından KÜÇÜK olsa da emit edilmeli,
    /// eşitlik ve artış ise sessiz kalmalı.
    fn shp_005_fires_for(d1: f64, d2: f64) -> bool {
        let mut shape_ti = ShapeInternTable::new();
        let mut records = empty_records();
        let mut map = empty_map();
        records.shapes = vec![
            shape_pt_d(&mut shape_ti, "S1", 1, 41.0, 29.0, d1, 2),
            shape_pt_d(&mut shape_ti, "S1", 2, 41.1, 29.1, d2, 3),
        ];
        records.shape_interns = shape_ti.clone();
        let (t1, ti1) = trip_with_shape("T1", "S1");
        records.trips = vec![t1]; records.trip_interns = ti1;
        map.shape_points.insert("S1".into(), vec![0, 1]);
        map.trips.insert("T1".into(), 0);
        build(&records, &map).notices.iter().any(|n| n.rule_id == "SHP_005")
    }

    #[test]
    fn shp_005_fires_on_decrease_below_old_epsilon() {
        // issue #94 karşı-örneği: azalma 5e-7, eski `d < prev - 1e-6` eşiğinin ALTINDA.
        assert!(shp_005_fires_for(1.0000005, 1.0000000),
            "1e-6'dan küçük gerçek azalma da SHP_005 üretmeli (#94)");
        // f64'te temsil edilebilen en küçük azalma da yakalanmalı.
        assert!(shp_005_fires_for(1.0, 1.0 - f64::EPSILON / 2.0),
            "bir ULP'lik azalma da SHP_005 üretmeli (#94)");
    }

    #[test]
    fn shp_005_silent_on_equal_or_increasing() {
        assert!(!shp_005_fires_for(10.0, 10.0), "eşit değerler SHP_005 üretmemeli");
        assert!(!shp_005_fires_for(10.0, 10.0 + f64::EPSILON * 8.0),
            "en küçük artış bile SHP_005 üretmemeli");
        assert!(!shp_005_fires_for(10.0, 20.0), "artan değerler SHP_005 üretmemeli");
    }

    #[test]
    fn bounding_box_correct() {
        let mut shape_ti = ShapeInternTable::new();
        let mut records = empty_records();
        let mut map = empty_map();
        records.shapes = vec![
            shape_pt(&mut shape_ti, "S1", 1, 40.0, 28.0, 2),
            shape_pt(&mut shape_ti, "S1", 2, 42.0, 30.0, 3),
        ];
        records.shape_interns = shape_ti.clone();
        let (t1, ti1) = trip_with_shape("T1", "S1");
        records.trips = vec![t1];
        records.trip_interns = ti1;
        map.shape_points.insert("S1".into(), vec![0, 1]);
        map.trips.insert("T1".into(), 0);

        let result = build(&records, &map);
        let seg = result.derived.shape_geometry.shapes.get("S1").unwrap();
        assert!((seg.bbox.0 - 40.0).abs() < 1e-9); // min_lat
        assert!((seg.bbox.1 - 42.0).abs() < 1e-9); // max_lat
        assert!((seg.bbox.2 - 28.0).abs() < 1e-9); // min_lon
        assert!((seg.bbox.3 - 30.0).abs() < 1e-9); // max_lon
    }

    // ── SHP_010 ───────────────────────────────────────────────────────────────

    #[test]
    fn duplicate_coordinate_produces_shp_010() {
        let mut shape_ti = ShapeInternTable::new();
        let mut records = empty_records();
        let mut map = empty_map();
        records.shapes = vec![
            shape_pt(&mut shape_ti, "S1", 1, 41.0, 29.0, 2),
            shape_pt(&mut shape_ti, "S1", 2, 41.0, 29.0, 3), // özdeş koordinat
            shape_pt(&mut shape_ti, "S1", 3, 41.1, 29.1, 4),
        ];
        records.shape_interns = shape_ti.clone();
        let (t1, ti1) = trip_with_shape("T1", "S1");
        records.trips = vec![t1];
        records.trip_interns = ti1;
        map.shape_points.insert("S1".into(), vec![0, 1, 2]);
        map.trips.insert("T1".into(), 0);

        let result = build(&records, &map);
        assert!(result.notices.iter().any(|n| n.rule_id == "SHP_010"));
    }

    #[test]
    fn shp_010_fires_once_per_shape_despite_many_duplicates() {
        let mut shape_ti = ShapeInternTable::new();
        // #15 Mode A: bir shape'te ÇOK ardışık-tekrar nokta olsa da SHP_010 shape başına
        // TEK üretilmeli (dedup zaten teke indirir; ara-üretim patlamasını önle).
        let mut records = empty_records();
        let mut map = empty_map();
        records.shapes = vec![
            shape_pt(&mut shape_ti, "S1", 1, 41.0, 29.0, 1),
            shape_pt(&mut shape_ti, "S1", 2, 41.0, 29.0, 2), // dup 1
            shape_pt(&mut shape_ti, "S1", 3, 41.0, 29.0, 3), // dup 2
            shape_pt(&mut shape_ti, "S1", 4, 41.0, 29.0, 4), // dup 3
            shape_pt(&mut shape_ti, "S1", 5, 41.1, 29.1, 5),
        ];
        records.shape_interns = shape_ti.clone();
        let (t1, ti1) = trip_with_shape("T1", "S1");
        records.trips = vec![t1];
        records.trip_interns = ti1;
        map.shape_points.insert("S1".into(), vec![0, 1, 2, 3, 4]);
        map.trips.insert("T1".into(), 0);

        let result = build(&records, &map);
        let shp010 = result.notices.iter().filter(|n| n.rule_id == "SHP_010").count();
        assert_eq!(shp010, 1, "3 ardışık tekrar olsa da SHP_010 shape başına 1 kez: {shp010}");
    }

    #[test]
    fn shp_010_high_volume_aggregates_to_single_feed_notice() {
        let mut shape_ti = ShapeInternTable::new();
        // #48: ardışık-özdeş-koordinatlı shape sayısı yüksek (>50) → tek feed-özet.
        let mut records = empty_records();
        let mut map = empty_map();
        let mut shapes = Vec::new();
        for i in 0..=51u32 {
            let sid = format!("S{i}");
            let base = shapes.len();
            // her shape: 2 özdeş nokta (dup) → SHP_010 tetikler
            shapes.push(shape_pt(&mut shape_ti, &sid, 1, 41.0, 29.0, 1));
            shapes.push(shape_pt(&mut shape_ti, &sid, 2, 41.0, 29.0, 2));
            map.shape_points.insert(sid, vec![base, base + 1]);
        }
        records.shapes = shapes;
        records.shape_interns = shape_ti.clone();

        let result = build(&records, &map);
        let shp010: Vec<_> = result.notices.iter().filter(|n| n.rule_id == "SHP_010").collect();
        assert_eq!(shp010.len(), 1, "yüksek hacimde tek feed-özeti beklenir");
        assert_eq!(shp010[0].entity_type, EntityType::Feed);
        assert_eq!(shp010[0].observed_value.as_deref(), Some("52"));
        assert_eq!(
            shp010[0].details.as_ref().and_then(|d| d.get("affected_shapes")).map(String::as_str),
            Some("52"),
        );
    }

    // ── SHP_018 ───────────────────────────────────────────────────────────────

    #[test]
    fn orphan_shape_produces_shp_018() {
        let mut shape_ti = ShapeInternTable::new();
        let mut records = empty_records();
        let mut map = empty_map();
        records.shapes = vec![
            shape_pt(&mut shape_ti, "ORPHAN", 1, 41.0, 29.0, 2),
            shape_pt(&mut shape_ti, "ORPHAN", 2, 41.1, 29.1, 3),
        ];
        records.shape_interns = shape_ti.clone();
        // trips: shape'e referans yok
        map.shape_points.insert("ORPHAN".into(), vec![0, 1]);

        let result = build(&records, &map);
        assert!(result.notices.iter().any(|n| n.rule_id == "SHP_018"));
    }

    #[test]
    fn referenced_shape_does_not_produce_shp_018() {
        let mut shape_ti = ShapeInternTable::new();
        let mut records = empty_records();
        let mut map = empty_map();
        records.shapes = vec![
            shape_pt(&mut shape_ti, "S1", 1, 41.0, 29.0, 2),
            shape_pt(&mut shape_ti, "S1", 2, 41.1, 29.1, 3),
        ];
        records.shape_interns = shape_ti.clone();
        let (t1, ti1) = trip_with_shape("T1", "S1");
        records.trips = vec![t1];
        records.trip_interns = ti1;
        map.shape_points.insert("S1".into(), vec![0, 1]);
        map.trips.insert("T1".into(), 0);

        let result = build(&records, &map);
        assert!(!result.notices.iter().any(|n| n.rule_id == "SHP_018"));
    }

    // ── Boş feed ─────────────────────────────────────────────────────────────

    #[test]
    fn empty_feed_produces_no_notices() {
        let result = build(&empty_records(), &empty_map());
        assert!(result.notices.is_empty());
        assert!(result.derived.shape_geometry.shapes.is_empty());
    }

    // ── WP-08b: CalendarBitmap ────────────────────────────────────────────────

    use crate::k2::calendar::CalendarRecord;
    use crate::k2::calendar_dates::CalendarDateIndex;

    fn calendar_rec(service_id: &str, days: [Option<u32>; 7], start: (u32,u32,u32), end: (u32,u32,u32)) -> CalendarRecord {
        CalendarRecord {
            service_id: service_id.into(),
            days,
            start_date: Some(start),
            end_date: Some(end),
            row: Default::default(),
            line: 2,
        }
    }

    fn cal_date_idx(service_id: &str, date: (u32, u32, u32), exception_type: u32) -> CalendarDateIndex {
        let mut idx = CalendarDateIndex::default();
        let d = date.0 * 10000 + date.1 * 100 + date.2;
        idx.exception_count.insert(service_id.into(), 1);
        idx.first_line.insert(service_id.into(), 2);
        match exception_type {
            1 => { idx.added.insert(service_id.into(), vec![d]); }
            2 => { idx.removed.insert(service_id.into(), vec![d]); }
            _ => {}
        }
        idx
    }

    #[test]
    fn calendar_bitmap_monday_only_in_week() {
        // 2026-05-11 is a Monday. Mon 11 May 2026 = service active; Tue 12 should not be.
        let mut records = empty_records();
        // days: [mon=1, tue=0, wed=0, thu=0, fri=0, sat=0, sun=0]
        records.calendars = vec![calendar_rec(
            "SVC1",
            [Some(1), Some(0), Some(0), Some(0), Some(0), Some(0), Some(0)],
            (2026, 5, 11),
            (2026, 5, 17),
        )];
        let result = build(&records, &empty_map());
        let dates = result.derived.calendar_bitmap.active_dates.get("SVC1").unwrap();
        assert!(dates.contains(&20260511), "Pazartesi olmalı");
        assert!(!dates.contains(&20260512), "Salı olmamalı");
        assert!(!dates.contains(&20260517), "Pazar olmamalı");
        assert_eq!(dates.len(), 1);
    }

    #[test]
    fn calendar_dates_exception_type2_removes_date() {
        let mut records = empty_records();
        // Tüm hafta aktif
        records.calendars = vec![calendar_rec(
            "SVC1",
            [Some(1), Some(1), Some(1), Some(1), Some(1), Some(1), Some(1)],
            (2026, 5, 11),
            (2026, 5, 11),
        )];
        // Pazartesi'yi kaldır (exception_type=2)
        records.calendar_dates = cal_date_idx("SVC1", (2026, 5, 11), 2);
        let result = build(&records, &empty_map());
        let dates = result.derived.calendar_bitmap.active_dates.get("SVC1").unwrap();
        assert!(!dates.contains(&20260511), "İptal edilen gün olmamalı");
    }

    #[test]
    fn calendar_dates_exception_type1_adds_date() {
        let mut records = empty_records();
        // Takvim yok, sadece ek gün
        records.calendar_dates = cal_date_idx("SVC_NEW", (2026, 6, 1), 1);
        let result = build(&records, &empty_map());
        let dates = result.derived.calendar_bitmap.active_dates.get("SVC_NEW").unwrap();
        assert!(dates.contains(&20260601));
    }

    // ── WP-08c: SpatialIndex ─────────────────────────────────────────────────

    fn stop_rec(stop_id: &str, lat: f64, lon: f64) -> StopRecord {
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

    #[test]
    fn spatial_index_places_stops_in_correct_cells() {
        let mut records = empty_records();
        // İki durak aynı 0.5° hücresinde (41.0 → cell row=82, 29.0 → cell col=58)
        records.stops = vec![
            stop_rec("S1", 41.0, 29.0),
            stop_rec("S2", 41.2, 29.3),
            stop_rec("S3", 41.6, 29.0), // farklı hücre (row=83)
        ];
        let result = build(&records, &empty_map());
        let idx = &result.derived.spatial_index;
        let cell_82_58 = idx.grid.get(&(82, 58)).expect("Hücre (82,58) olmalı");
        assert_eq!(cell_82_58.len(), 2, "S1 ve S2 aynı hücrede");
        let cell_83_58 = idx.grid.get(&(83, 58)).expect("Hücre (83,58) olmalı");
        assert_eq!(cell_83_58.len(), 1, "S3 farklı hücrede");
    }

    #[test]
    fn spatial_index_skips_stops_without_coords() {
        let mut records = empty_records();
        records.stops = vec![StopRecord {
            stop_id: "S_NO_COORD".into(),
            stop_code: None, stop_name: None,
            stop_lat: None, stop_lon: None,
            location_type: None, stop_timezone: None,
            wheelchair_boarding: None, stop_access: None,
            level_id: None, tts_stop_name: None,
            row: Default::default(), line: 2,
            ..Default::default()
        }];
        let result = build(&records, &empty_map());
        assert!(result.derived.spatial_index.grid.is_empty());
    }

    // ── WP-08d: PathwayGraph ──────────────────────────────────────────────────

    fn pathway_rec(pathway_id: &str, from: &str, to: &str, bidirectional: u32) -> PathwayRecord {
        PathwayRecord {
            pathway_id: pathway_id.into(),
            from_stop_id: from.into(),
            to_stop_id: to.into(),
            pathway_mode: Some(1),
            is_bidirectional: Some(bidirectional),
            length: None, traversal_time: None, stair_count: None,
            max_slope: None, min_width: None,
            row: Default::default(), line: 2,
        }
    }

    #[test]
    fn unidirectional_pathway_adds_one_edge() {
        let mut records = empty_records();
        records.pathways = vec![pathway_rec("PW1", "A", "B", 0)];
        let result = build(&records, &empty_map());
        let adj = &result.derived.pathway_graph.adjacency;
        assert!(adj.get("A").is_some_and(|v| v.iter().any(|(to, _)| to == "B")));
        assert!(adj.get("B").is_none_or(|v| !v.iter().any(|(to, _)| to == "A")));
    }

    #[test]
    fn bidirectional_pathway_adds_two_edges() {
        let mut records = empty_records();
        records.pathways = vec![pathway_rec("PW1", "A", "B", 1)];
        let result = build(&records, &empty_map());
        let adj = &result.derived.pathway_graph.adjacency;
        assert!(adj.get("A").is_some_and(|v| v.iter().any(|(to, _)| to == "B")));
        assert!(adj.get("B").is_some_and(|v| v.iter().any(|(to, _)| to == "A")));
    }

    // ── WP-08d: FareNetwork ───────────────────────────────────────────────────

    fn fare_rule_rec(fare_id: &str, route_id: Option<&str>, origin: Option<&str>, dest: Option<&str>) -> FareRuleRecord {
        FareRuleRecord {
            fare_id: fare_id.into(),
            route_id: route_id.map(str::to_string),
            origin_id: origin.map(str::to_string),
            destination_id: dest.map(str::to_string),
            contains_id: None,
            row: Default::default(), line: 2,
        }
    }

    #[test]
    fn fare_network_indexes_routes_and_zones() {
        let mut records = empty_records();
        records.fare_rules = vec![
            fare_rule_rec("F1", Some("R1"), Some("Z1"), Some("Z2")),
            fare_rule_rec("F1", Some("R2"), None, None),
            fare_rule_rec("F2", None, Some("Z3"), None),
        ];
        let result = build(&records, &empty_map());
        let fn_ = &result.derived.fare_network;
        assert_eq!(fn_.fare_routes.get("F1").unwrap().len(), 2);
        assert!(fn_.zone_ids.contains("Z1"));
        assert!(fn_.zone_ids.contains("Z2"));
        assert!(fn_.zone_ids.contains("Z3"));
        assert!(!fn_.fare_routes.contains_key("F2"), "F2'nin route_id'si yok");
    }
}
