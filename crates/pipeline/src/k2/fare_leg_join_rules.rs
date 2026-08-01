use super::common::{build_row_map, get_trimmed_field, RowMap};
use crate::k1_parse::RawFile;

/// GTFS Fares v2 `fare_leg_join_rules.txt` — aktarmayla birleşen iki bacağı tek etkin
/// ücret bacağı sayan eşleşme kuralları.
///
/// Dört alanın DÖRDÜ de bileşik birincil anahtarın parçasıdır; bu yüzden hepsi ham dize
/// olarak saklanır (boş değer anahtarın anlamlı bir parçasıdır — spec boş alanı
/// "eşleşmede yok sayılır" diye tanımlar, "yok" diye değil).
#[derive(Debug, Clone)]
pub struct FareLegJoinRuleRecord {
    pub from_network_id: String,
    pub to_network_id: String,
    pub from_stop_id: String,
    pub to_stop_id: String,
    pub row: RowMap,
    pub line: u64,
}

pub fn parse_fare_leg_join_rules(file: &RawFile) -> Vec<FareLegJoinRuleRecord> {
    file.rows
        .iter()
        .enumerate()
        .map(|(row_idx, row)| {
            let row_map = build_row_map(&file.headers, row);
            let f = |name: &str| get_trimmed_field(&row_map, name).unwrap_or("").to_string();
            FareLegJoinRuleRecord {
                from_network_id: f("from_network_id"),
                to_network_id: f("to_network_id"),
                from_stop_id: f("from_stop_id"),
                to_stop_id: f("to_stop_id"),
                row: row_map,
                line: (row_idx + 2) as u64,
            }
        })
        .collect()
}
