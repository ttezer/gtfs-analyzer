use gtfs_core::EntityType;

use super::common::{build_row_map, get_trimmed_field, make_k2_notice, parse_u32, validate_enum, RowMap};
use crate::k1_parse::RawFile;

#[derive(Debug, Clone)]
pub struct FareMediaRecord {
    pub fare_media_id: String,
    pub fare_media_name: Option<String>,
    pub fare_media_type: Option<u32>,
    pub row: RowMap,
    pub line: u64,
}

pub fn validate_fare_media(
    file: &RawFile,
) -> (Vec<FareMediaRecord>, Vec<gtfs_core::Notice>) {
    let mut notices = Vec::new();
    let mut records = Vec::new();
    let mut counter = 0;

    for (row_idx, row) in file.rows.iter().enumerate() {
        let line = (row_idx + 2) as u64;
        let row_map = build_row_map(&file.headers, row);
        let id = get_trimmed_field(&row_map, "fare_media_id").unwrap_or("").to_string();
        let entity_id = (!id.is_empty()).then_some(id.clone());

        let fare_media_type = match parse_u32(&row_map, "fare_media_type") {
            Ok(value) => {
                if let Some(v) = value {
                    if !validate_enum(&v.to_string(), &["0", "1", "2", "3", "4"]) {
                        notices.push(make_k2_notice(
                            &mut counter, "FMD_002", EntityType::Row, entity_id.clone(), Some(&row_map),
                            &file.name, Some(line), Some("fare_media_type"), Some(v.to_string()),
                            Some("0–4".to_string()),
                            "fare_media_type geçerli bir enum değeri değil.".to_string(),
                            "0 (yok), 1 (fiziksel kart), 2 (mobil uygulama), 3 (EMV temassız), 4 (transit kuruluş uygulaması) kullanın.",
                        ));
                    }
                } else {
                    notices.push(make_k2_notice(
                        &mut counter, "FMD_002", EntityType::Row, entity_id.clone(), Some(&row_map),
                        &file.name, Some(line), Some("fare_media_type"), None,
                        Some("0–4".to_string()),
                        "fare_media_type zorunludur.".to_string(),
                        "Geçerli bir fare_media_type değeri girin.",
                    ));
                }
                value
            }
            Err(err) => {
                notices.push(make_k2_notice(
                    &mut counter, "FMD_002", EntityType::Row, entity_id.clone(), Some(&row_map),
                    &file.name, Some(line), Some("fare_media_type"),
                    get_trimmed_field(&row_map, "fare_media_type").map(str::to_string),
                    Some("0–4".to_string()), err,
                    "Geçerli bir fare_media_type değeri girin.",
                ));
                None
            }
        };

        records.push(FareMediaRecord {
            fare_media_id: id,
            fare_media_name: get_trimmed_field(&row_map, "fare_media_name")
                .filter(|v| !v.is_empty())
                .map(str::to_string),
            fare_media_type,
            row: row_map,
            line,
        });
    }

    (records, notices)
}
