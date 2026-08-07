use super::common::{get_raw_field, build_row_map, get_trimmed_field, RowMap};
use crate::k1_parse::RawFile;

#[derive(Debug, Clone)]
pub struct StopAreaRecord {
    pub area_id: String,
    pub stop_id: String,
    pub row: RowMap,
    pub line: u64,
}

pub fn parse_stop_areas(file: &RawFile) -> Vec<StopAreaRecord> {
    file.rows
        .iter()
        .enumerate()
        .map(|(row_idx, row)| {
            let row_map = build_row_map(&file.headers, row);
            StopAreaRecord {
                area_id: get_raw_field(&row_map, "area_id").unwrap_or("").to_string(),
                stop_id: get_raw_field(&row_map, "stop_id").unwrap_or("").to_string(),
                row: row_map,
                line: (row_idx + 2) as u64,
            }
        })
        .collect()
}
