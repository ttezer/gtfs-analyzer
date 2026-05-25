use gtfs_core::EntityType;

use super::common::{build_row_map, get_trimmed_field, parse_f64, make_k2_notice, RowMap};
use crate::k1_parse::RawFile;

#[derive(Debug, Clone)]
pub struct LevelRecord {
    pub level_id: String,
    pub level_index: Option<f64>,
    pub level_name: Option<String>,
    pub row: RowMap,
    pub line: u64,
}

pub fn validate_levels(file: &RawFile) -> (Vec<LevelRecord>, Vec<gtfs_core::Notice>) {
    let mut notices = Vec::new();
    let mut records = Vec::new();
    let mut counter = 0;

    for (row_idx, row) in file.rows.iter().enumerate() {
        let line = (row_idx + 2) as u64;
        let row_map = build_row_map(&file.headers, row);
        let level_id = get_trimmed_field(&row_map, "level_id").unwrap_or("").to_string();
        let entity_id = (!level_id.is_empty()).then_some(level_id.clone());

        let level_index = match parse_f64(&row_map, "level_index") {
            Ok(v) => v,
            Err(err) => {
                notices.push(make_k2_notice(
                    &mut counter,
                    "LVL_002",
                    EntityType::Level,
                    entity_id.clone(),
                    Some(&row_map),
                    &file.name,
                    Some(line),
                    Some("level_index"),
                    get_trimmed_field(&row_map, "level_index").map(str::to_string),
                    None,
                    err,
                    "level_index için geçerli bir sayısal değer girin.",
                ));
                None
            }
        };

        let level_name = get_trimmed_field(&row_map, "level_name")
            .filter(|v| !v.is_empty())
            .map(str::to_string);
        if level_name.is_none() {
            notices.push(make_k2_notice(
                &mut counter,
                "LVL_003",
                EntityType::Level,
                entity_id.clone(),
                Some(&row_map),
                &file.name,
                Some(line),
                Some("level_name"),
                Some(String::new()),
                None,
                "level_name önerilen ancak eksik.".to_string(),
                "Okunabilir bir level_name ekleyin.",
            ));
        }

        // LVL_005: level_name çok uzun (> 255 karakter)
        if let Some(ref name) = level_name {
            if name.len() > 255 {
                notices.push(make_k2_notice(
                    &mut counter, "LVL_005", EntityType::Level, entity_id.clone(),
                    Some(&row_map), &file.name, Some(line), Some("level_name"),
                    Some(format!("{} karakter", name.len())), Some("≤ 255 karakter".to_string()),
                    format!("'{}' katmanının level_name değeri {} karakter — 255 sınırını aşıyor.", level_id, name.len()),
                    "level_name'i 255 karakterin altına kısaltın.",
                ));
            }
        }

        records.push(LevelRecord {
            level_id,
            level_index,
            level_name,
            row: row_map,
            line,
        });
    }

    (records, notices)
}
