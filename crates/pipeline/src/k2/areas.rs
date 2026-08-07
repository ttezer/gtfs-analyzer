use super::common::{get_raw_field, build_row_map, get_trimmed_field, RowMap};
use crate::k1_parse::RawFile;

#[derive(Debug, Clone)]
pub struct AreaRecord {
    pub area_id: String,
    pub area_name: Option<String>,
    pub row: RowMap,
    pub line: u64,
}

pub fn parse_areas(file: &RawFile) -> Vec<AreaRecord> {
    file.rows
        .iter()
        .enumerate()
        .map(|(row_idx, row)| {
            let row_map = build_row_map(&file.headers, row);
            AreaRecord {
                area_id: get_raw_field(&row_map, "area_id").unwrap_or("").to_string(),
                area_name: get_trimmed_field(&row_map, "area_name")
                    .filter(|v| !v.trim().is_empty())
                    .map(str::to_string),
                row: row_map,
                line: (row_idx + 2) as u64,
            }
        })
        .collect()
}
