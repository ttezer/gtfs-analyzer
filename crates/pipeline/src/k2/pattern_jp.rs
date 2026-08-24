use super::common::{build_row_map, get_trimmed_field, RowMap};
use crate::k1_parse::RawFile;

/// GTFS-JP `pattern_jp.txt` — opsiyonel duruş paterni bilgisi.
/// Dosya mevcutsa jp_pattern_id satırın zorunlu kimliğidir; diğer alanlar
/// GTFS-JP rehberinde opsiyonel metin/tarih alanlarıdır.
#[derive(Debug, Clone)]
pub struct PatternJpRecord {
    pub jp_pattern_id: String,
    pub route_update_date: Option<String>,
    pub origin_stop: Option<String>,
    pub via_stop: Option<String>,
    pub destination_stop: Option<String>,
    pub row: RowMap,
    pub line: u64,
}

pub fn parse_pattern_jp(file: &RawFile) -> Vec<PatternJpRecord> {
    file.rows
        .iter()
        .enumerate()
        .map(|(row_idx, row)| {
            let row_map = build_row_map(&file.headers, row);
            PatternJpRecord {
                jp_pattern_id: get_trimmed_field(&row_map, "jp_pattern_id")
                    .unwrap_or("")
                    .to_string(),
                route_update_date: get_trimmed_field(&row_map, "route_update_date")
                    .filter(|v| !v.is_empty())
                    .map(str::to_string),
                origin_stop: get_trimmed_field(&row_map, "origin_stop")
                    .filter(|v| !v.is_empty())
                    .map(str::to_string),
                via_stop: get_trimmed_field(&row_map, "via_stop")
                    .filter(|v| !v.is_empty())
                    .map(str::to_string),
                destination_stop: get_trimmed_field(&row_map, "destination_stop")
                    .filter(|v| !v.is_empty())
                    .map(str::to_string),
                row: row_map,
                line: (row_idx + 2) as u64,
            }
        })
        .collect()
}
