use super::common::{build_row_map, get_trimmed_field, RowMap};
use crate::k1_parse::RawFile;

/// GTFS-JP `office_jp.txt` — işletme ofisi tanımları. `office_id` birincil
/// anahtardır ve `trips.jp_office_id` ile referanslanır (JPN_002).
#[derive(Debug, Clone)]
pub struct OfficeJpRecord {
    pub office_id: String,
    pub office_name: Option<String>,
    pub row: RowMap,
    pub line: u64,
}

pub fn parse_office_jp(file: &RawFile) -> Vec<OfficeJpRecord> {
    file.rows
        .iter()
        .enumerate()
        .map(|(row_idx, row)| {
            let row_map = build_row_map(&file.headers, row);
            OfficeJpRecord {
                office_id: get_trimmed_field(&row_map, "office_id").unwrap_or("").to_string(),
                office_name: get_trimmed_field(&row_map, "office_name")
                    .filter(|v| !v.is_empty())
                    .map(str::to_string),
                row: row_map,
                line: (row_idx + 2) as u64,
            }
        })
        .collect()
}
