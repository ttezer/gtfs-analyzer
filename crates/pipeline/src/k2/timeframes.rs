use gtfs_core::EntityType;

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

        if group_id.is_empty() {
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

    (records, notices)
}
