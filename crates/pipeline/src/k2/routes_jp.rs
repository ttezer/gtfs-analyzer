use super::common::{build_row_map, get_trimmed_field, RowMap};
use crate::k1_parse::RawFile;

/// GTFS-JP `routes_jp.txt` — routes.txt'e bağlı Japonya-özel rota bilgisi.
/// `origin_stop`, `via_stop` ve `destination_stop` GTFS-JP'de başvuru metnidir;
/// stop_id foreign key'i olarak yorumlanmaz.
#[derive(Debug, Clone)]
pub struct RoutesJpRecord {
    pub route_id: String,
    pub route_update_date: Option<String>,
    pub origin_stop: Option<String>,
    pub via_stop: Option<String>,
    pub destination_stop: Option<String>,
    pub row: RowMap,
    pub line: u64,
}

pub fn parse_routes_jp(file: &RawFile) -> Vec<RoutesJpRecord> {
    file.rows
        .iter()
        .enumerate()
        .map(|(row_idx, row)| {
            let row_map = build_row_map(&file.headers, row);
            RoutesJpRecord {
                route_id: get_trimmed_field(&row_map, "route_id")
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
