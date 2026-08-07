use super::common::{get_raw_field, build_row_map, RowMap};
use crate::k1_parse::RawFile;

#[derive(Debug, Clone)]
pub struct FareRuleRecord {
    pub fare_id: String,
    pub route_id: Option<String>,
    pub origin_id: Option<String>,
    pub destination_id: Option<String>,
    pub contains_id: Option<String>,
    pub row: RowMap,
    pub line: u64,
}

pub fn parse_fare_rules(file: &RawFile) -> Vec<FareRuleRecord> {
    file.rows
        .iter()
        .enumerate()
        .map(|(row_idx, row)| {
            let row_map = build_row_map(&file.headers, row);
            FareRuleRecord {
                fare_id: get_raw_field(&row_map, "fare_id").unwrap_or("").to_string(),
                route_id: get_raw_field(&row_map, "route_id").filter(|v| !v.trim().is_empty()).map(str::to_string),
                origin_id: get_raw_field(&row_map, "origin_id").filter(|v| !v.trim().is_empty()).map(str::to_string),
                destination_id: get_raw_field(&row_map, "destination_id").filter(|v| !v.trim().is_empty()).map(str::to_string),
                contains_id: get_raw_field(&row_map, "contains_id").filter(|v| !v.trim().is_empty()).map(str::to_string),
                row: row_map,
                line: (row_idx + 2) as u64,
            }
        })
        .collect()
}
