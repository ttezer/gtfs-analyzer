use super::common::{get_raw_field, build_row_map, RowMap};
use crate::k1_parse::RawFile;

#[derive(Debug, Clone)]
pub struct RouteNetworkRecord {
    pub network_id: String,
    pub route_id: String,
    pub row: RowMap,
    pub line: u64,
}

pub fn parse_route_networks(file: &RawFile) -> Vec<RouteNetworkRecord> {
    file.rows
        .iter()
        .enumerate()
        .map(|(row_idx, row)| {
            let row_map = build_row_map(&file.headers, row);
            RouteNetworkRecord {
                network_id: get_raw_field(&row_map, "network_id").unwrap_or("").to_string(),
                route_id: get_raw_field(&row_map, "route_id").unwrap_or("").to_string(),
                row: row_map,
                line: (row_idx + 2) as u64,
            }
        })
        .collect()
}
