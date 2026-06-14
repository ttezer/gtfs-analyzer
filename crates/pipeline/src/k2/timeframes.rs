use gtfs_core::EntityType;
use std::collections::HashMap;

use super::common::{build_row_map, get_trimmed_field, make_k2_notice, parse_gtfs_time, RowMap};
use crate::k1_parse::RawFile;

#[derive(Debug, Clone)]
pub struct TimeframeRecord {
    pub timeframe_group_id: String,
    pub start_time: Option<(u32, u32, u32)>,
    pub end_time: Option<(u32, u32, u32)>,
    pub service_id: String,
    pub row: RowMap,
    pub line: u64,
}

pub fn validate_timeframes(
    file: &RawFile,
) -> (Vec<TimeframeRecord>, Vec<gtfs_core::Notice>) {
    let mut notices = Vec::new();
    let mut records = Vec::new();
    let mut counter = 0;

    for (row_idx, row) in file.rows.iter().enumerate() {
        let line = (row_idx + 2) as u64;
        let row_map = build_row_map(&file.headers, row);
        let group_id = get_trimmed_field(&row_map, "timeframe_group_id").unwrap_or("").to_string();
        let entity_id = (!group_id.is_empty()).then_some(group_id.clone());

        if get_trimmed_field(&row_map, "timeframe_group_id") == Some("") {
            notices.push(make_k2_notice(
                &mut counter, "TFR_001", EntityType::Row, None, Some(&row_map),
                &file.name, Some(line), Some("timeframe_group_id"), None,
                Some("dolu".to_string()),
                "timeframe_group_id zorunludur.".to_string(),
                "Her zaman dilimi kaydı için bir timeframe_group_id girin.",
            ));
        }

        let service_id = get_trimmed_field(&row_map, "service_id").unwrap_or("").to_string();

        let start_time = match parse_gtfs_time(&row_map, "start_time") {
            Ok(v) => v,
            Err(err) => {
                notices.push(make_k2_notice(
                    &mut counter, "TFR_003", EntityType::Row, entity_id.clone(), Some(&row_map),
                    &file.name, Some(line), Some("start_time"),
                    get_trimmed_field(&row_map, "start_time").map(str::to_string),
                    Some("HH:MM:SS".to_string()), err,
                    "start_time için HH:MM:SS formatında geçerli bir değer girin.",
                ));
                None
            }
        };

        let end_time = match parse_gtfs_time(&row_map, "end_time") {
            Ok(v) => v,
            Err(err) => {
                notices.push(make_k2_notice(
                    &mut counter, "TFR_003", EntityType::Row, entity_id.clone(), Some(&row_map),
                    &file.name, Some(line), Some("end_time"),
                    get_trimmed_field(&row_map, "end_time").map(str::to_string),
                    Some("HH:MM:SS".to_string()), err,
                    "end_time için HH:MM:SS formatında geçerli bir değer girin.",
                ));
                None
            }
        };

        if let (Some(st), Some(et)) = (start_time, end_time) {
            let st_secs = st.0 * 3600 + st.1 * 60 + st.2;
            let et_secs = et.0 * 3600 + et.1 * 60 + et.2;
            if et_secs <= st_secs {
                notices.push(make_k2_notice(
                    &mut counter, "TFR_004", EntityType::Row, entity_id.clone(), Some(&row_map),
                    &file.name, Some(line), Some("end_time"),
                    get_trimmed_field(&row_map, "end_time").map(str::to_string),
                    Some("> start_time".to_string()),
                    "end_time, start_time'dan büyük olmalıdır.".to_string(),
                    "end_time değerini start_time'dan sonraya ayarlayın.",
                ));
            }
        }

        records.push(TimeframeRecord {
            timeframe_group_id: group_id,
            start_time,
            end_time,
            service_id,
            row: row_map,
            line,
        });
    }

    // TFR_005: aynı (timeframe_group_id, service_id) grubunda örtüşen zaman aralıkları
    // Gruplama: key → Vec<(start_secs, end_secs, line)>
    let mut groups: HashMap<(&str, &str), Vec<(u32, u32, u64)>> = HashMap::new();
    for rec in &records {
        if let (Some(st), Some(et)) = (rec.start_time, rec.end_time) {
            let st_secs = st.0 * 3600 + st.1 * 60 + st.2;
            let et_secs = et.0 * 3600 + et.1 * 60 + et.2;
            groups
                .entry((rec.timeframe_group_id.as_str(), rec.service_id.as_str()))
                .or_default()
                .push((st_secs, et_secs, rec.line));
        }
    }
    for ((group_id, service_id), mut intervals) in groups {
        intervals.sort_by_key(|&(st, _, _)| st);
        for i in 1..intervals.len() {
            let (prev_st, prev_et, prev_line) = intervals[i - 1];
            let (cur_st, _, cur_line) = intervals[i];
            if cur_st < prev_et {
                notices.push(make_k2_notice(
                    &mut counter, "TFR_005", EntityType::Row,
                    Some(group_id.to_string()), None,
                    &file.name, Some(cur_line), Some("start_time"),
                    Some(format!("{cur_line} / {prev_line}")), None,
                    format!(
                        "timeframe_group_id '{}' service_id '{}': {prev_st}–{prev_et}s ile {cur_st}s aralıkları örtüşüyor.",
                        group_id, service_id
                    ),
                    "Aynı grup ve service_id içindeki zaman aralıklarının örtüşmediğinden emin olun.",
                ));
            }
        }
    }

    (records, notices)
}
