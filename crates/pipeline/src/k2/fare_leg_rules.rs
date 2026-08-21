use gtfs_core::EntityType;

use super::common::{get_raw_field, build_row_map, get_trimmed_field, make_k2_notice, parse_u32, RowMap};
use crate::k1_parse::RawFile;

#[derive(Debug, Clone)]
pub struct FareLegRuleRecord {
    pub leg_group_id: Option<String>,
    pub network_id: Option<String>,
    pub from_area_id: Option<String>,
    pub to_area_id: Option<String>,
    pub from_timeframe_group_id: Option<String>,
    pub to_timeframe_group_id: Option<String>,
    pub fare_product_id: String,
    pub rule_priority: Option<u32>,
    pub row: RowMap,
    pub line: u64,
}

pub fn validate_fare_leg_rules(
    file: &RawFile,
) -> (Vec<FareLegRuleRecord>, Vec<gtfs_core::Notice>) {
    let mut notices = Vec::new();
    let mut records = Vec::new();
    let mut counter = 0;

    for (row_idx, row) in file.rows.iter().enumerate() {
        let line = (row_idx + 2) as u64;
        let row_map = build_row_map(&file.headers, row);
        let leg_group_id = get_raw_field(&row_map, "leg_group_id")
            .filter(|v| !v.trim().is_empty())
            .map(str::to_string);
        let entity_id = leg_group_id.clone();

        let rule_priority = match parse_u32(&row_map, "rule_priority") {
            Ok(v) => v,
            Err(err) => {
                notices.push(make_k2_notice(
                    &mut counter, "FLG_007", EntityType::Row, entity_id.clone(), Some(&row_map),
                    &file.name, Some(line), Some("rule_priority"),
                    get_trimmed_field(&row_map, "rule_priority").map(str::to_string),
                    Some(">= 0".to_string()), err,
                    "rule_priority için negatif olmayan bir tam sayı girin.",
                ));
                None
            }
        };

        records.push(FareLegRuleRecord {
            leg_group_id,
            network_id: get_raw_field(&row_map, "network_id")
                .filter(|v| !v.trim().is_empty())
                .map(str::to_string),
            from_area_id: get_raw_field(&row_map, "from_area_id")
                .filter(|v| !v.trim().is_empty())
                .map(str::to_string),
            to_area_id: get_raw_field(&row_map, "to_area_id")
                .filter(|v| !v.trim().is_empty())
                .map(str::to_string),
            from_timeframe_group_id: get_raw_field(&row_map, "from_timeframe_group_id")
                .filter(|v| !v.trim().is_empty())
                .map(str::to_string),
            to_timeframe_group_id: get_raw_field(&row_map, "to_timeframe_group_id")
                .filter(|v| !v.trim().is_empty())
                .map(str::to_string),
            fare_product_id: get_raw_field(&row_map, "fare_product_id").unwrap_or("").to_string(),
            rule_priority,
            row: row_map,
            line,
        });
    }

    (records, notices)
}
