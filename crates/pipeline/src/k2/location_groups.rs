use super::common::{get_raw_field, build_row_map};
use crate::k1_parse::RawFile;

/// GTFS-Flex `location_groups.txt` — adlandırılmış durak grubu tanımı.
#[derive(Debug, Clone)]
pub struct LocationGroupRecord {
    pub location_group_id: String,
    pub line: u64,
}

pub fn parse_location_groups(file: &RawFile) -> Vec<LocationGroupRecord> {
    file.rows
        .iter()
        .enumerate()
        .map(|(row_idx, row)| {
            let row_map = build_row_map(&file.headers, row);
            LocationGroupRecord {
                location_group_id: get_raw_field(&row_map, "location_group_id")
                    .unwrap_or("")
                    .to_string(),
                line: (row_idx + 2) as u64,
            }
        })
        .collect()
}

/// GTFS-Flex `location_group_stops.txt` — grup ↔ durak eşleştirme (junction).
#[derive(Debug, Clone)]
pub struct LocationGroupStopRecord {
    pub location_group_id: String,
    pub stop_id: String,
    pub line: u64,
}

pub fn parse_location_group_stops(file: &RawFile) -> Vec<LocationGroupStopRecord> {
    file.rows
        .iter()
        .enumerate()
        .map(|(row_idx, row)| {
            let row_map = build_row_map(&file.headers, row);
            LocationGroupStopRecord {
                location_group_id: get_raw_field(&row_map, "location_group_id")
                    .unwrap_or("")
                    .to_string(),
                stop_id: get_raw_field(&row_map, "stop_id").unwrap_or("").to_string(),
                line: (row_idx + 2) as u64,
            }
        })
        .collect()
}
