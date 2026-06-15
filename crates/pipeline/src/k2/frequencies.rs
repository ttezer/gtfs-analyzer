use gtfs_core::EntityType;

use super::common::{
    build_row_map, get_trimmed_field, make_k2_notice, parse_gtfs_time, parse_u32,
    validate_enum, RowMap,
};
use crate::k1_parse::RawFile;

#[derive(Debug, Clone)]
pub struct FrequencyRecord {
    pub trip_id: String,
    pub start_time: Option<(u32, u32, u32)>,
    pub end_time: Option<(u32, u32, u32)>,
    pub headway_secs: Option<u32>,
    pub exact_times: Option<u32>,
    pub row: RowMap,
    pub line: u64,
}

pub fn validate_frequencies(file: &RawFile) -> (Vec<FrequencyRecord>, Vec<gtfs_core::Notice>) {
    let mut notices = Vec::new();
    let mut records = Vec::new();
    let mut counter = 0u32;

    for (row_idx, row) in file.rows.iter().enumerate() {
        let line = (row_idx + 2) as u64;
        let row_map = build_row_map(&file.headers, row);

        let trip_id = get_trimmed_field(&row_map, "trip_id").unwrap_or("").to_string();
        let entity_id = (!trip_id.is_empty()).then_some(trip_id.clone());

        // FRQ_001: trip_id required (sütun yoksa ARC_025 devralır → atla)
        if get_trimmed_field(&row_map, "trip_id") == Some("") {
            notices.push(make_k2_notice(
                &mut counter, "FRQ_001", EntityType::Trip, None,
                Some(&row_map), &file.name, Some(line), Some("trip_id"),
                Some(String::new()), None,
                "trip_id zorunludur.".to_string(),
                "trip_id alanını doldurun.",
            ));
        }

        // FRQ_002: start_time required + valid HH:MM:SS
        let start_time = match parse_gtfs_time(&row_map, "start_time") {
            Ok(v) => {
                if get_trimmed_field(&row_map, "start_time") == Some("") {
                    notices.push(make_k2_notice(
                        &mut counter, "FRQ_002", EntityType::Trip, entity_id.clone(),
                        Some(&row_map), &file.name, Some(line), Some("start_time"),
                        Some(String::new()), Some("HH:MM:SS".to_string()),
                        "start_time zorunludur.".to_string(),
                        "HH:MM:SS formatında start_time girin.",
                    ));
                }
                v
            }
            Err(err) => {
                notices.push(make_k2_notice(
                    &mut counter, "FRQ_002", EntityType::Trip, entity_id.clone(),
                    Some(&row_map), &file.name, Some(line), Some("start_time"),
                    get_trimmed_field(&row_map, "start_time").map(str::to_string),
                    Some("HH:MM:SS".to_string()), err,
                    "HH:MM:SS formatında start_time girin.",
                ));
                None
            }
        };

        // FRQ_003: end_time required + valid HH:MM:SS
        let end_time = match parse_gtfs_time(&row_map, "end_time") {
            Ok(v) => {
                if get_trimmed_field(&row_map, "end_time") == Some("") {
                    notices.push(make_k2_notice(
                        &mut counter, "FRQ_003", EntityType::Trip, entity_id.clone(),
                        Some(&row_map), &file.name, Some(line), Some("end_time"),
                        Some(String::new()), Some("HH:MM:SS".to_string()),
                        "end_time zorunludur.".to_string(),
                        "HH:MM:SS formatında end_time girin.",
                    ));
                }
                v
            }
            Err(err) => {
                notices.push(make_k2_notice(
                    &mut counter, "FRQ_003", EntityType::Trip, entity_id.clone(),
                    Some(&row_map), &file.name, Some(line), Some("end_time"),
                    get_trimmed_field(&row_map, "end_time").map(str::to_string),
                    Some("HH:MM:SS".to_string()), err,
                    "HH:MM:SS formatında end_time girin.",
                ));
                None
            }
        };

        // FRQ_004: headway_secs required + positive
        let headway_secs = match parse_u32(&row_map, "headway_secs") {
            Ok(v) => {
                match v {
                    None => {
                        // sütun yoksa ARC_025 devralır → atla
                        if get_trimmed_field(&row_map, "headway_secs") == Some("") {
                            notices.push(make_k2_notice(
                                &mut counter, "FRQ_004", EntityType::Trip, entity_id.clone(),
                                Some(&row_map), &file.name, Some(line), Some("headway_secs"),
                                Some(String::new()), Some("> 0".to_string()),
                                "headway_secs zorunludur.".to_string(),
                                "headway_secs pozitif bir tam sayı olarak girin.",
                            ));
                        }
                        None
                    }
                    Some(val) => {
                        // FRQ_008: headway_secs == 0
                        if val == 0 {
                            notices.push(make_k2_notice(
                                &mut counter, "FRQ_008", EntityType::Trip, entity_id.clone(),
                                Some(&row_map), &file.name, Some(line), Some("headway_secs"),
                                Some("0".to_string()), Some("> 0".to_string()),
                                "headway_secs sıfırdan büyük olmalıdır.".to_string(),
                                "headway_secs değerini pozitif bir saniye sayısına ayarlayın.",
                            ));
                        }
                        // FRQ_009: headway_secs çok kısa (< 60s)
                        if val > 0 && val < 60 {
                            notices.push(make_k2_notice(
                                &mut counter, "FRQ_009", EntityType::Trip, entity_id.clone(),
                                Some(&row_map), &file.name, Some(line), Some("headway_secs"),
                                Some(format!("{val}s")), Some("≥ 60s".to_string()),
                                format!("headway_secs '{val}' saniye — 1 dakikadan kısa frekans gerçekçi değil."),
                                "headway_secs değerini en az 60 saniye olarak ayarlayın.",
                            ));
                        }
                        Some(val)
                    }
                }
            }
            Err(err) => {
                notices.push(make_k2_notice(
                    &mut counter, "FRQ_004", EntityType::Trip, entity_id.clone(),
                    Some(&row_map), &file.name, Some(line), Some("headway_secs"),
                    get_trimmed_field(&row_map, "headway_secs").map(str::to_string),
                    Some("> 0".to_string()), err,
                    "headway_secs pozitif bir tam sayı olarak girin.",
                ));
                None
            }
        };

        // FRQ_005: end_time must be after start_time
        if let (Some(s), Some(e)) = (start_time, end_time) {
            let s_secs = s.0 * 3600 + s.1 * 60 + s.2;
            let e_secs = e.0 * 3600 + e.1 * 60 + e.2;
            if e_secs <= s_secs {
                notices.push(make_k2_notice(
                    &mut counter, "FRQ_005", EntityType::Trip, entity_id.clone(),
                    Some(&row_map), &file.name, Some(line), Some("end_time"),
                    get_trimmed_field(&row_map, "end_time").map(str::to_string),
                    Some("start_time'dan sonra".to_string()),
                    "end_time, start_time'dan sonra olmalıdır.".to_string(),
                    "end_time değerini start_time'dan sonra olacak şekilde ayarlayın.",
                ));
            }
        }

        // FRQ_007: exact_times must be 0 or 1 if provided
        let exact_times = match parse_u32(&row_map, "exact_times") {
            Ok(v) => {
                if let Some(val) = v {
                    if !validate_enum(&val.to_string(), &["0", "1"]) {
                        notices.push(make_k2_notice(
                            &mut counter, "FRQ_007", EntityType::Trip, entity_id.clone(),
                            Some(&row_map), &file.name, Some(line), Some("exact_times"),
                            Some(val.to_string()), Some("0 veya 1".to_string()),
                            "exact_times 0 veya 1 olmalıdır.".to_string(),
                            "exact_times değerini 0 (frekans bazlı) veya 1 (tarifeli) olarak ayarlayın.",
                        ));
                    }
                }
                v
            }
            Err(_) => None,
        };

        records.push(FrequencyRecord {
            trip_id,
            start_time,
            end_time,
            headway_secs,
            exact_times,
            row: row_map,
            line,
        });
    }

    // FRQ_011: aynı trip için frequencies dönemleri zaman aralığı çakışıyor (MD overlapping_frequency).
    // Geçerli [start, end) aralıklarını trip başına grupla, start'a göre sırala, max-end takibiyle
    // ardışık olmayan çakışmaları da yakala. Geçersiz aralık (end ≤ start, FRQ_005) hariç.
    use std::collections::HashMap;
    let mut by_trip: HashMap<&str, Vec<(u32, u32, u64, &RowMap)>> = HashMap::new();
    for rec in &records {
        if rec.trip_id.is_empty() {
            continue;
        }
        if let (Some(s), Some(e)) = (rec.start_time, rec.end_time) {
            let s_secs = s.0 * 3600 + s.1 * 60 + s.2;
            let e_secs = e.0 * 3600 + e.1 * 60 + e.2;
            if e_secs > s_secs {
                by_trip
                    .entry(rec.trip_id.as_str())
                    .or_default()
                    .push((s_secs, e_secs, rec.line, &rec.row));
            }
        }
    }
    for (trip, mut periods) in by_trip {
        if periods.len() < 2 {
            continue;
        }
        periods.sort_by_key(|p| p.0);
        let mut max_end = periods[0].1;
        for &(cur_start, cur_end, cur_line, cur_row) in &periods[1..] {
            if cur_start < max_end {
                notices.push(make_k2_notice(
                    &mut counter, "FRQ_011", EntityType::Trip, Some(trip.to_string()),
                    Some(cur_row), &file.name, Some(cur_line), Some("start_time"),
                    get_trimmed_field(cur_row, "start_time").map(str::to_string),
                    Some("önceki dönemle çakışmamalı".to_string()),
                    format!("trip_id '{trip}' için frequencies dönemleri zaman aralığı çakışıyor."),
                    "Aynı trip'in frequencies dönemlerini çakışmayacak biçimde ayarlayın.",
                ));
            }
            if cur_end > max_end {
                max_end = cur_end;
            }
        }
    }

    (records, notices)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::k1_parse::RawFile;

    fn make_file(headers: Vec<&str>, rows: Vec<Vec<&str>>) -> RawFile {
        RawFile {
            name: "frequencies.txt".to_string(),
            headers: headers.into_iter().map(str::to_string).collect(),
            rows: rows.into_iter().map(|r| r.into_iter().map(smol_str::SmolStr::from).collect()).collect(),
            bytes: 0,
            raw_text: None,
        }
    }

    #[test]
    fn valid_frequency_produces_no_notices() {
        let file = make_file(
            vec!["trip_id", "start_time", "end_time", "headway_secs"],
            vec![vec!["T1", "08:00:00", "12:00:00", "600"]],
        );
        let (records, notices) = validate_frequencies(&file);
        assert_eq!(records.len(), 1);
        assert!(notices.is_empty(), "Geçerli kayıt notice üretmemeli: {:?}", notices);
    }

    #[test]
    fn missing_trip_id_produces_frq_001() {
        let file = make_file(
            vec!["trip_id", "start_time", "end_time", "headway_secs"],
            vec![vec!["", "08:00:00", "12:00:00", "600"]],
        );
        let (_, notices) = validate_frequencies(&file);
        assert!(notices.iter().any(|n| n.rule_id == "FRQ_001"));
    }

    #[test]
    fn zero_headway_produces_frq_008() {
        let file = make_file(
            vec!["trip_id", "start_time", "end_time", "headway_secs"],
            vec![vec!["T1", "08:00:00", "12:00:00", "0"]],
        );
        let (_, notices) = validate_frequencies(&file);
        assert!(notices.iter().any(|n| n.rule_id == "FRQ_008"));
    }

    #[test]
    fn end_before_start_produces_frq_005() {
        let file = make_file(
            vec!["trip_id", "start_time", "end_time", "headway_secs"],
            vec![vec!["T1", "12:00:00", "08:00:00", "600"]],
        );
        let (_, notices) = validate_frequencies(&file);
        assert!(notices.iter().any(|n| n.rule_id == "FRQ_005"));
    }

    #[test]
    fn overlapping_periods_produce_frq_011() {
        let file = make_file(
            vec!["trip_id", "start_time", "end_time", "headway_secs"],
            vec![
                vec!["T1", "08:00:00", "12:00:00", "600"],
                vec!["T1", "11:00:00", "14:00:00", "900"], // 11:00 < 12:00 → çakışma
            ],
        );
        let (_, notices) = validate_frequencies(&file);
        assert!(notices.iter().any(|n| n.rule_id == "FRQ_011"),
            "çakışan dönemler → FRQ_011: {:?}",
            notices.iter().map(|n| &n.rule_id).collect::<Vec<_>>());
    }

    #[test]
    fn adjacent_periods_no_frq_011() {
        let file = make_file(
            vec!["trip_id", "start_time", "end_time", "headway_secs"],
            vec![
                vec!["T1", "08:00:00", "12:00:00", "600"],
                vec!["T1", "12:00:00", "14:00:00", "900"], // bitişik (12:00==12:00) → çakışma değil
            ],
        );
        let (_, notices) = validate_frequencies(&file);
        assert!(!notices.iter().any(|n| n.rule_id == "FRQ_011"),
            "bitişik dönemler FRQ_011 üretmemeli: {:?}",
            notices.iter().map(|n| &n.rule_id).collect::<Vec<_>>());
    }
}
