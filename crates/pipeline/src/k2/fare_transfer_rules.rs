use gtfs_core::EntityType;

use super::common::{build_row_map, get_trimmed_field, make_k2_notice, parse_u32, validate_enum, RowMap};
use crate::k1_parse::RawFile;

#[derive(Debug, Clone)]
pub struct FareTransferRuleRecord {
    pub from_leg_group_id: Option<String>,
    pub to_leg_group_id: Option<String>,
    pub transfer_count: Option<i32>,
    pub duration_limit: Option<u32>,
    pub duration_limit_type: Option<u32>,
    pub fare_transfer_type: Option<u32>,
    pub fare_product_id: Option<String>,
    pub row: RowMap,
    pub line: u64,
}

pub fn validate_fare_transfer_rules(
    file: &RawFile,
) -> (Vec<FareTransferRuleRecord>, Vec<gtfs_core::Notice>) {
    let mut notices = Vec::new();
    let mut records = Vec::new();
    let mut counter = 0;

    for (row_idx, row) in file.rows.iter().enumerate() {
        let line = (row_idx + 2) as u64;
        let row_map = build_row_map(&file.headers, row);
        let from_leg = get_trimmed_field(&row_map, "from_leg_group_id")
            .filter(|v| !v.is_empty())
            .map(str::to_string);
        let entity_id = from_leg.clone();

        let fare_transfer_type = match parse_u32(&row_map, "fare_transfer_type") {
            Ok(value) => {
                if let Some(v) = value {
                    if !validate_enum(&v.to_string(), &["0", "1", "2"]) {
                        notices.push(make_k2_notice(
                            &mut counter, "FTR_001", EntityType::Row, entity_id.clone(), Some(&row_map),
                            &file.name, Some(line), Some("fare_transfer_type"), Some(v.to_string()),
                            Some("0–2".to_string()),
                            "fare_transfer_type geçerli bir enum değeri değil.".to_string(),
                            "0, 1 veya 2 kullanın.",
                        ));
                    }
                } else {
                    notices.push(make_k2_notice(
                        &mut counter, "FTR_001", EntityType::Row, entity_id.clone(), Some(&row_map),
                        &file.name, Some(line), Some("fare_transfer_type"), None,
                        Some("0–2".to_string()),
                        "fare_transfer_type zorunludur.".to_string(),
                        "0, 1 veya 2 kullanın.",
                    ));
                }
                value
            }
            Err(err) => {
                notices.push(make_k2_notice(
                    &mut counter, "FTR_001", EntityType::Row, entity_id.clone(), Some(&row_map),
                    &file.name, Some(line), Some("fare_transfer_type"),
                    get_trimmed_field(&row_map, "fare_transfer_type").map(str::to_string),
                    Some("0–2".to_string()), err,
                    "Geçerli bir fare_transfer_type değeri girin.",
                ));
                None
            }
        };

        let duration_limit_type = match parse_u32(&row_map, "duration_limit_type") {
            Ok(value) => {
                if let Some(v) = value {
                    if !validate_enum(&v.to_string(), &["0", "1", "2", "3"]) {
                        notices.push(make_k2_notice(
                            &mut counter, "FTR_005", EntityType::Row, entity_id.clone(), Some(&row_map),
                            &file.name, Some(line), Some("duration_limit_type"), Some(v.to_string()),
                            Some("0–3".to_string()),
                            "duration_limit_type geçerli bir enum değeri değil.".to_string(),
                            "0, 1, 2 veya 3 kullanın.",
                        ));
                    }
                }
                value
            }
            Err(err) => {
                notices.push(make_k2_notice(
                    &mut counter, "FTR_005", EntityType::Row, entity_id.clone(), Some(&row_map),
                    &file.name, Some(line), Some("duration_limit_type"),
                    get_trimmed_field(&row_map, "duration_limit_type").map(str::to_string),
                    Some("0–3".to_string()), err,
                    "Geçerli bir duration_limit_type değeri girin.",
                ));
                None
            }
        };

        let duration_limit = match parse_u32(&row_map, "duration_limit") {
            Ok(value) => {
                if let Some(v) = value {
                    if v == 0 {
                        notices.push(make_k2_notice(
                            &mut counter, "FTR_006", EntityType::Row, entity_id.clone(), Some(&row_map),
                            &file.name, Some(line), Some("duration_limit"), Some(v.to_string()),
                            Some("> 0".to_string()),
                            "duration_limit pozitif olmalıdır.".to_string(),
                            "duration_limit alanını pozitif bir saniye değerine ayarlayın.",
                        ));
                    }
                }
                value
            }
            Err(err) => {
                notices.push(make_k2_notice(
                    &mut counter, "FTR_006", EntityType::Row, entity_id.clone(), Some(&row_map),
                    &file.name, Some(line), Some("duration_limit"),
                    get_trimmed_field(&row_map, "duration_limit").map(str::to_string),
                    Some("> 0".to_string()), err,
                    "duration_limit için pozitif bir saniye değeri girin.",
                ));
                None
            }
        };

        // FTR_007: duration_limit_type, duration_limit olmadan anlamsız
        if duration_limit_type.is_some() && duration_limit.is_none() {
            notices.push(make_k2_notice(
                &mut counter, "FTR_007", EntityType::Row, entity_id.clone(), Some(&row_map),
                &file.name, Some(line), Some("duration_limit"), None,
                Some("dolu".to_string()),
                "duration_limit_type tanımlandığında duration_limit de belirtilmelidir.".to_string(),
                "duration_limit alanını doldurun ya da duration_limit_type'ı kaldırın.",
            ));
        }

        // transfer_count: -1 veya pozitif tam sayı
        let transfer_count_raw = get_trimmed_field(&row_map, "transfer_count")
            .filter(|v| !v.is_empty())
            .map(str::to_string);
        let transfer_count = if let Some(ref raw) = transfer_count_raw {
            match raw.parse::<i32>() {
                Ok(v) if v == -1 || v > 0 => Some(v),
                Ok(v) => {
                    notices.push(make_k2_notice(
                        &mut counter, "FTR_008", EntityType::Row, entity_id.clone(), Some(&row_map),
                        &file.name, Some(line), Some("transfer_count"), Some(v.to_string()),
                        Some("-1 veya > 0".to_string()),
                        "transfer_count -1 veya pozitif bir tam sayı olmalıdır.".to_string(),
                        "-1 (sınırsız) veya 1 gibi pozitif bir değer girin.",
                    ));
                    None
                }
                Err(_) => {
                    notices.push(make_k2_notice(
                        &mut counter, "FTR_008", EntityType::Row, entity_id.clone(), Some(&row_map),
                        &file.name, Some(line), Some("transfer_count"), Some(raw.clone()),
                        Some("-1 veya > 0".to_string()),
                        "transfer_count sayısal bir değer değil.".to_string(),
                        "-1 (sınırsız) veya pozitif bir tam sayı girin.",
                    ));
                    None
                }
            }
        } else {
            None
        };

        records.push(FareTransferRuleRecord {
            from_leg_group_id: from_leg,
            to_leg_group_id: get_trimmed_field(&row_map, "to_leg_group_id")
                .filter(|v| !v.is_empty())
                .map(str::to_string),
            transfer_count,
            duration_limit,
            duration_limit_type,
            fare_transfer_type,
            fare_product_id: get_trimmed_field(&row_map, "fare_product_id")
                .filter(|v| !v.is_empty())
                .map(str::to_string),
            row: row_map,
            line,
        });
    }

    (records, notices)
}
