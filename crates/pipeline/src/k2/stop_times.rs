use gtfs_core::EntityType;
use rustc_hash::{FxHashMap, FxHashSet};
use smol_str::SmolStr;

use super::common::make_k2_notice;
use crate::k1_parse::RawFile;

#[derive(Debug, Clone)]
pub struct StopTimeRecord {
    pub trip_id: SmolStr,
    pub stop_id: SmolStr,
    pub stop_sequence: Option<u32>,
    pub arrival_time: Option<(u32, u32, u32)>,
    pub departure_time: Option<(u32, u32, u32)>,
    pub stop_headsign: Option<SmolStr>,
    pub pickup_type: Option<u32>,
    pub drop_off_type: Option<u32>,
    pub shape_dist_traveled: Option<f64>,
    pub timepoint: Option<u32>,
    pub continuous_pickup: Option<u32>,
    pub continuous_drop_off: Option<u32>,
    pub line: u64,
}

// ── Kolon indeks yapısı — header'dan bir kez hesaplanır ─────────────────────

struct Cols {
    trip_id:              Option<usize>,
    stop_id:              Option<usize>,
    arrival_time:         Option<usize>,
    departure_time:       Option<usize>,
    stop_headsign:        Option<usize>,
    pickup_type:          Option<usize>,
    drop_off_type:        Option<usize>,
    shape_dist_traveled:  Option<usize>,
    timepoint:            Option<usize>,
    continuous_pickup:    Option<usize>,
    continuous_drop_off:  Option<usize>,
    stop_sequence:        Option<usize>,
}

impl Cols {
    fn from_headers(headers: &[String]) -> Self {
        let pos = |name: &str| headers.iter().position(|h| h == name);
        Self {
            trip_id:             pos("trip_id"),
            stop_id:             pos("stop_id"),
            arrival_time:        pos("arrival_time"),
            departure_time:      pos("departure_time"),
            stop_headsign:       pos("stop_headsign"),
            pickup_type:         pos("pickup_type"),
            drop_off_type:       pos("drop_off_type"),
            shape_dist_traveled: pos("shape_dist_traveled"),
            timepoint:           pos("timepoint"),
            continuous_pickup:   pos("continuous_pickup"),
            continuous_drop_off: pos("continuous_drop_off"),
            stop_sequence:       pos("stop_sequence"),
        }
    }
}

#[inline]
fn get_col<'a>(row: &'a [SmolStr], col: Option<usize>) -> &'a str {
    col.and_then(|i| row.get(i)).map(|s| s.as_str().trim()).unwrap_or("")
}

// ── Parser yardımcıları (RowMap olmaksızın) ──────────────────────────────────

fn parse_u32_raw(raw: &str, field: &str) -> Result<Option<u32>, String> {
    if raw.is_empty() {
        return Ok(None);
    }
    raw.parse::<u32>()
        .map(Some)
        .map_err(|_| format!("'{field}' için u32 bekleniyor, alınan: {raw}"))
}

fn parse_f64_raw(raw: &str, field: &str) -> Result<Option<f64>, String> {
    if raw.is_empty() {
        return Ok(None);
    }
    raw.parse::<f64>()
        .map(Some)
        .map_err(|_| format!("'{field}' için f64 bekleniyor, alınan: {raw}"))
}

fn parse_gtfs_time_raw(raw: &str, field: &str) -> Result<Option<(u32, u32, u32)>, String> {
    if raw.is_empty() {
        return Ok(None);
    }
    let bad = || format!("'{field}' için HH:MM:SS bekleniyor, alınan: {raw}");
    let mut it = raw.splitn(3, ':');
    let hour   = it.next().ok_or_else(bad)?.parse::<u32>().map_err(|_| bad())?;
    let minute = it.next().ok_or_else(bad)?.parse::<u32>().map_err(|_| bad())?;
    let second = it.next().ok_or_else(bad)?.parse::<u32>().map_err(|_| bad())?;
    if minute > 59 || second > 59 {
        return Err(format!("'{field}' dakika/saniye aralığı geçersiz: {raw}"));
    }
    Ok(Some((hour, minute, second)))
}


// ── SmolStr interning: >22 byte string'ler için Arc alloc sayısını azaltır ──

/// ≤22 byte → inline SmolStr (no heap alloc). >22 byte → ilk karşılaşmada Arc alloc,
/// sonraki hit'lerde Arc::clone (~2ns). Cache miss: O(N_unique), hit: O(1) no-alloc.
#[inline]
fn intern_smolstr(raw: &str, cache: &mut FxHashMap<String, SmolStr>) -> SmolStr {
    if raw.len() <= 22 {
        SmolStr::new(raw)
    } else if let Some(cached) = cache.get(raw) {
        cached.clone()
    } else {
        let s = SmolStr::new(raw);
        cache.insert(raw.to_string(), s.clone());
        s
    }
}

// ── Ana doğrulayıcı ─────────────────────────────────────────────────────────

pub fn validate_stop_times(file: &RawFile) -> (Vec<StopTimeRecord>, Vec<gtfs_core::Notice>) {
    let mut notices = Vec::new();
    let mut records = Vec::with_capacity(file.rows.len());
    let mut counter = 0u32;

    let cols = Cols::from_headers(&file.headers);

    // Intern cache: unique long (>22 byte) trip_id / stop_id başına bir Arc alloc
    let mut trip_id_cache: FxHashMap<String, SmolStr> = FxHashMap::default();
    let mut stop_id_cache: FxHashMap<String, SmolStr> = FxHashMap::default();

    // STM_023 / STM_032: sıralama ve tekrar takibi
    let mut last_seq_by_trip: FxHashMap<SmolStr, u32> = FxHashMap::default();
    let mut seen_trip_seq: FxHashMap<SmolStr, FxHashSet<u32>> = FxHashMap::default();
    let mut stm023_fired: FxHashSet<SmolStr> = FxHashSet::default();

    for (row_idx, row) in file.rows.iter().enumerate() {
        let line = (row_idx + 2) as u64;

        let trip_id_raw = get_col(row, cols.trip_id);
        let trip_id = intern_smolstr(trip_id_raw, &mut trip_id_cache);
        // entity_id is only materialized when a notice is actually pushed
        let eid = || (!trip_id.is_empty()).then(|| trip_id.to_string());

        // STM_005: stop_sequence required
        let seq_raw = get_col(row, cols.stop_sequence);
        let stop_sequence = match parse_u32_raw(seq_raw, "stop_sequence") {
            Ok(v) => {
                if v.is_none() {
                    notices.push(make_k2_notice(
                        &mut counter, "STM_005", EntityType::Trip, eid(),
                        None, &file.name, Some(line), Some("stop_sequence"),
                        Some(String::new()), None,
                        "stop_sequence zorunludur.".to_string(),
                        "stop_sequence negatif olmayan bir tam sayı olarak girin.",
                    ));
                }
                v
            }
            Err(err) => {
                notices.push(make_k2_notice(
                    &mut counter, "STM_005", EntityType::Trip, eid(),
                    None, &file.name, Some(line), Some("stop_sequence"),
                    Some(seq_raw.to_string()), None, err,
                    "stop_sequence negatif olmayan bir tam sayı olarak girin.",
                ));
                None
            }
        };

        // STM_023 / STM_032: sıralama ve yineleme
        if let Some(seq) = stop_sequence {
            // STM_032: aynı (trip_id, stop_sequence) çifti tekrar
            let seen = seen_trip_seq.entry(trip_id.clone()).or_default();
            if !seen.insert(seq) {
                notices.push(make_k2_notice(
                    &mut counter, "STM_032", EntityType::Trip, eid(),
                    None, &file.name, Some(line), Some("stop_sequence"),
                    Some(seq.to_string()), None,
                    format!("trip_id '{}' için stop_sequence {seq} tekrar ediyor.", trip_id),
                    "Her (trip_id, stop_sequence) çifti stop_times.txt'te benzersiz olmalıdır.",
                ));
            }
            // STM_023: dosya satır sırası stop_sequence sırasıyla uyuşmuyor
            if !stm023_fired.contains(&trip_id) {
                if let Some(&last) = last_seq_by_trip.get(&trip_id) {
                    if seq < last {
                        notices.push(make_k2_notice(
                            &mut counter, "STM_023", EntityType::Trip, eid(),
                            None, &file.name, Some(line), Some("stop_sequence"),
                            Some(seq.to_string()), Some(format!("> {last}")),
                            format!("trip_id '{}' satırları stop_sequence sırasında değil: {seq} < {last}.", trip_id),
                            "stop_times.txt'i stop_sequence değerine göre sıralayın.",
                        ));
                        stm023_fired.insert(trip_id.clone());
                    } else if seq > last {
                        last_seq_by_trip.insert(trip_id.clone(), seq);
                    }
                } else {
                    last_seq_by_trip.insert(trip_id.clone(), seq);
                }
            }
        }

        // STM_006: stop_id required
        let stop_id = intern_smolstr(get_col(row, cols.stop_id), &mut stop_id_cache);
        if stop_id.is_empty() {
            notices.push(make_k2_notice(
                &mut counter, "STM_006", EntityType::Stop, eid(),
                None, &file.name, Some(line), Some("stop_id"),
                Some(String::new()), None,
                "stop_id zorunludur.".to_string(),
                "stop_id alanını doldurun.",
            ));
        }

        // arrival_time
        let arr_raw = get_col(row, cols.arrival_time);
        let arrival_time = match parse_gtfs_time_raw(arr_raw, "arrival_time") {
            Ok(v) => v,
            Err(err) => {
                notices.push(make_k2_notice(
                    &mut counter, "STM_003", EntityType::Trip, eid(),
                    None, &file.name, Some(line), Some("arrival_time"),
                    Some(arr_raw.to_string()), Some("HH:MM:SS".to_string()), err,
                    "HH:MM:SS formatında arrival_time girin.",
                ));
                None
            }
        };

        // departure_time
        let dep_raw = get_col(row, cols.departure_time);
        let departure_time = match parse_gtfs_time_raw(dep_raw, "departure_time") {
            Ok(v) => v,
            Err(err) => {
                notices.push(make_k2_notice(
                    &mut counter, "STM_004", EntityType::Trip, eid(),
                    None, &file.name, Some(line), Some("departure_time"),
                    Some(dep_raw.to_string()), Some("HH:MM:SS".to_string()), err,
                    "HH:MM:SS formatında departure_time girin.",
                ));
                None
            }
        };

        // STM_007: departure_time >= arrival_time
        if let (Some(arr), Some(dep)) = (arrival_time, departure_time) {
            let arr_secs = arr.0 * 3600 + arr.1 * 60 + arr.2;
            let dep_secs = dep.0 * 3600 + dep.1 * 60 + dep.2;
            if dep_secs < arr_secs {
                notices.push(make_k2_notice(
                    &mut counter, "STM_007", EntityType::Trip, eid(),
                    None, &file.name, Some(line), Some("departure_time"),
                    Some(dep_raw.to_string()), Some(">= arrival_time".to_string()),
                    "departure_time, arrival_time'dan büyük veya eşit olmalıdır.".to_string(),
                    "departure_time değerini arrival_time'dan sonra veya eşit olacak şekilde ayarlayın.",
                ));
            }
        }

        // STM_034: varış veya kalkış zamanından yalnızca biri tanımlı
        match (arrival_time, departure_time) {
            (Some(_), None) => {
                notices.push(make_k2_notice(
                    &mut counter, "STM_034", EntityType::Trip, eid(),
                    None, &file.name, Some(line), Some("departure_time"),
                    None, Some("dolu".to_string()),
                    format!("trip_id '{}' satırında arrival_time tanımlı ama departure_time eksik.", trip_id),
                    "Her iki zaman alanını birlikte doldurun veya ikisini de boş bırakın.",
                ));
            }
            (None, Some(_)) => {
                notices.push(make_k2_notice(
                    &mut counter, "STM_034", EntityType::Trip, eid(),
                    None, &file.name, Some(line), Some("arrival_time"),
                    None, Some("dolu".to_string()),
                    format!("trip_id '{}' satırında departure_time tanımlı ama arrival_time eksik.", trip_id),
                    "Her iki zaman alanını birlikte doldurun veya ikisini de boş bırakın.",
                ));
            }
            _ => {}
        }

        // STM_009: pickup_type 0-3
        let pickup_type = parse_pickup_dropoff_col(
            get_col(row, cols.pickup_type), &mut notices, &mut counter,
            "STM_009", "pickup_type", trip_id.as_str(), line, &file.name,
        );

        // STM_010: drop_off_type 0-3
        let drop_off_type = parse_pickup_dropoff_col(
            get_col(row, cols.drop_off_type), &mut notices, &mut counter,
            "STM_010", "drop_off_type", trip_id.as_str(), line, &file.name,
        );

        // STM_018: continuous_pickup 0-3
        let continuous_pickup = parse_pickup_dropoff_col(
            get_col(row, cols.continuous_pickup), &mut notices, &mut counter,
            "STM_018", "continuous_pickup", trip_id.as_str(), line, &file.name,
        );

        // STM_019: continuous_drop_off 0-3
        let continuous_drop_off = parse_pickup_dropoff_col(
            get_col(row, cols.continuous_drop_off), &mut notices, &mut counter,
            "STM_019", "continuous_drop_off", trip_id.as_str(), line, &file.name,
        );

        // STM_022: timepoint 0 or 1
        let tp_raw = get_col(row, cols.timepoint);
        let timepoint = match parse_u32_raw(tp_raw, "timepoint") {
            Ok(v) => {
                if let Some(val) = v {
                    if val > 1 {
                        notices.push(make_k2_notice(
                            &mut counter, "STM_022", EntityType::Trip, eid(),
                            None, &file.name, Some(line), Some("timepoint"),
                            Some(val.to_string()), Some("0 veya 1".to_string()),
                            "timepoint 0 veya 1 olmalıdır.".to_string(),
                            "timepoint değerini 0 (yaklaşık) veya 1 (kesin) olarak ayarlayın.",
                        ));
                    }
                }
                v
            }
            Err(_) => None,
        };

        // shape_dist_traveled: non-negative
        let sdt_raw = get_col(row, cols.shape_dist_traveled);
        let shape_dist_traveled = match parse_f64_raw(sdt_raw, "shape_dist_traveled") {
            Ok(v) => {
                if let Some(d) = v {
                    if d < 0.0 {
                        notices.push(make_k2_notice(
                            &mut counter, "STM_030", EntityType::Trip, eid(),
                            None, &file.name, Some(line), Some("shape_dist_traveled"),
                            Some(d.to_string()), Some(">= 0".to_string()),
                            "shape_dist_traveled negatif olamaz.".to_string(),
                            "shape_dist_traveled değerini sıfır veya pozitif bir sayıya ayarlayın.",
                        ));
                    }
                }
                v
            }
            Err(_) => None,
        };

        let stop_headsign_raw = get_col(row, cols.stop_headsign);
        let stop_headsign = if stop_headsign_raw.is_empty() {
            None
        } else {
            Some(SmolStr::new(stop_headsign_raw))
        };

        records.push(StopTimeRecord {
            trip_id,
            stop_id,
            stop_sequence,
            arrival_time,
            departure_time,
            stop_headsign,
            pickup_type,
            drop_off_type,
            shape_dist_traveled,
            timepoint,
            continuous_pickup,
            continuous_drop_off,
            line,
        });
    }

    (records, notices)
}

fn parse_pickup_dropoff_col(
    raw: &str,
    notices: &mut Vec<gtfs_core::Notice>,
    counter: &mut u32,
    rule_id: &str,
    field: &str,
    trip_id: &str,
    line: u64,
    file_name: &str,
) -> Option<u32> {
    match parse_u32_raw(raw, field) {
        Ok(v) => {
            if let Some(val) = v {
                if val > 3 {
                    let entity_id = (!trip_id.is_empty()).then(|| trip_id.to_string());
                    notices.push(make_k2_notice(
                        counter, rule_id, EntityType::Trip, entity_id,
                        None, file_name, Some(line), Some(field),
                        Some(val.to_string()), Some("0-3".to_string()),
                        format!("{field} 0, 1, 2 veya 3 olmalıdır."),
                        "Alanı geçerli bir GTFS biniş/iniş enum değerine (0-3) ayarlayın.",
                    ));
                }
            }
            v
        }
        Err(err) => {
            let entity_id = (!trip_id.is_empty()).then(|| trip_id.to_string());
            notices.push(make_k2_notice(
                counter, rule_id, EntityType::Trip, entity_id,
                None, file_name, Some(line), Some(field),
                Some(raw.to_string()), Some("0-3".to_string()), err,
                "Alanı geçerli bir GTFS biniş/iniş enum değerine (0-3) ayarlayın.",
            ));
            None
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::k1_parse::RawFile;

    fn make_file(headers: Vec<&str>, rows: Vec<Vec<&str>>) -> RawFile {
        RawFile {
            name: "stop_times.txt".to_string(),
            headers: headers.into_iter().map(str::to_string).collect(),
            rows: rows.into_iter().map(|r| r.into_iter().map(SmolStr::from).collect()).collect(),
            bytes: 0,
        }
    }

    #[test]
    fn valid_stop_time_produces_no_notices() {
        let file = make_file(
            vec!["trip_id", "arrival_time", "departure_time", "stop_id", "stop_sequence"],
            vec![
                vec!["T1", "08:00:00", "08:00:00", "S1", "1"],
                vec!["T1", "08:10:00", "08:10:00", "S2", "2"],
            ],
        );
        let (records, notices) = validate_stop_times(&file);
        assert_eq!(records.len(), 2);
        assert!(notices.is_empty(), "Geçerli stop_times notice üretmemeli: {:?}", notices);
    }

    #[test]
    fn missing_stop_sequence_produces_stm_005() {
        let file = make_file(
            vec!["trip_id", "arrival_time", "departure_time", "stop_id", "stop_sequence"],
            vec![vec!["T1", "08:00:00", "08:00:00", "S1", ""]],
        );
        let (_, notices) = validate_stop_times(&file);
        assert!(notices.iter().any(|n| n.rule_id == "STM_005"));
    }

    #[test]
    fn missing_stop_id_produces_stm_006() {
        let file = make_file(
            vec!["trip_id", "arrival_time", "departure_time", "stop_id", "stop_sequence"],
            vec![vec!["T1", "08:00:00", "08:00:00", "", "1"]],
        );
        let (_, notices) = validate_stop_times(&file);
        assert!(notices.iter().any(|n| n.rule_id == "STM_006"));
    }

    #[test]
    fn departure_before_arrival_produces_stm_007() {
        let file = make_file(
            vec!["trip_id", "arrival_time", "departure_time", "stop_id", "stop_sequence"],
            vec![vec!["T1", "09:00:00", "08:00:00", "S1", "1"]],
        );
        let (_, notices) = validate_stop_times(&file);
        assert!(notices.iter().any(|n| n.rule_id == "STM_007"));
    }

    #[test]
    fn invalid_pickup_type_produces_stm_009() {
        let file = make_file(
            vec!["trip_id", "arrival_time", "departure_time", "stop_id", "stop_sequence", "pickup_type"],
            vec![vec!["T1", "08:00:00", "08:00:00", "S1", "1", "9"]],
        );
        let (_, notices) = validate_stop_times(&file);
        assert!(notices.iter().any(|n| n.rule_id == "STM_009"));
    }

    #[test]
    fn invalid_timepoint_produces_stm_022() {
        let file = make_file(
            vec!["trip_id", "arrival_time", "departure_time", "stop_id", "stop_sequence", "timepoint"],
            vec![vec!["T1", "08:00:00", "08:00:00", "S1", "1", "5"]],
        );
        let (_, notices) = validate_stop_times(&file);
        assert!(notices.iter().any(|n| n.rule_id == "STM_022"));
    }

    #[test]
    fn scope_key_is_trip_id() {
        let file = make_file(
            vec!["trip_id", "arrival_time", "departure_time", "stop_id", "stop_sequence"],
            vec![vec!["TRIP_X", "08:00:00", "08:00:00", "S1", ""]],
        );
        let (_, notices) = validate_stop_times(&file);
        let n = notices.iter().find(|n| n.rule_id == "STM_005").expect("STM_005 olmalı");
        assert_eq!(n.scope_key.as_deref(), Some("TRIP_X"), "scope_key trip_id olmalı");
    }

    #[test]
    fn only_arrival_time_produces_stm_034() {
        let file = make_file(
            vec!["trip_id", "arrival_time", "departure_time", "stop_id", "stop_sequence"],
            vec![vec!["T1", "08:00:00", "", "S1", "1"]],
        );
        let (_, notices) = validate_stop_times(&file);
        assert!(notices.iter().any(|n| n.rule_id == "STM_034"),
            "Yalnızca arrival_time dolu → STM_034 olmalı. Notices: {:?}", notices);
    }

    #[test]
    fn only_departure_time_produces_stm_034() {
        let file = make_file(
            vec!["trip_id", "arrival_time", "departure_time", "stop_id", "stop_sequence"],
            vec![vec!["T1", "", "08:00:00", "S1", "1"]],
        );
        let (_, notices) = validate_stop_times(&file);
        assert!(notices.iter().any(|n| n.rule_id == "STM_034"),
            "Yalnızca departure_time dolu → STM_034 olmalı. Notices: {:?}", notices);
    }

    #[test]
    fn both_times_empty_no_stm_034() {
        let file = make_file(
            vec!["trip_id", "arrival_time", "departure_time", "stop_id", "stop_sequence"],
            vec![vec!["T1", "", "", "S1", "1"]],
        );
        let (_, notices) = validate_stop_times(&file);
        assert!(!notices.iter().any(|n| n.rule_id == "STM_034"),
            "İkisi de boş → STM_034 üretilmemeli. Notices: {:?}", notices);
    }
}
