use super::common::{get_raw_field, build_row_map, RowMap};
use crate::k1_parse::RawFile;

/// GTFS-JP `agency_jp.txt` — işleticinin resmî/yasal bilgileri. `agency_id`
/// `agency.txt`'e referans veren yabancı anahtardır (JPN_003).
#[derive(Debug, Clone)]
pub struct AgencyJpRecord {
    pub agency_id: Option<String>,
    pub row: RowMap,
    pub line: u64,
}

pub fn parse_agency_jp(file: &RawFile) -> Vec<AgencyJpRecord> {
    file.rows
        .iter()
        .enumerate()
        .map(|(row_idx, row)| {
            let row_map = build_row_map(&file.headers, row);
            AgencyJpRecord {
                agency_id: get_raw_field(&row_map, "agency_id")
                    .filter(|v| !v.trim().is_empty())
                    .map(str::to_string),
                row: row_map,
                line: (row_idx + 2) as u64,
            }
        })
        .collect()
}
