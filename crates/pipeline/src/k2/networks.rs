use super::common::{build_row_map, get_trimmed_field, RowMap};
use crate::k1_parse::RawFile;

#[derive(Debug, Clone)]
pub struct NetworkRecord {
    pub network_id: String,
    pub network_name: Option<String>,
    pub row: RowMap,
    pub line: u64,
}

pub fn parse_networks(file: &RawFile) -> Vec<NetworkRecord> {
    file.rows
        .iter()
        .enumerate()
        .map(|(row_idx, row)| {
            let row_map = build_row_map(&file.headers, row);
            NetworkRecord {
                network_id: get_trimmed_field(&row_map, "network_id").unwrap_or("").to_string(),
                network_name: get_trimmed_field(&row_map, "network_name")
                    .filter(|v| !v.is_empty())
                    .map(str::to_string),
                row: row_map,
                line: (row_idx + 2) as u64,
            }
        })
        .collect()
}
