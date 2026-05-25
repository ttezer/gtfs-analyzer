use gtfs_core::EntityType;

use super::common::{build_row_map, get_trimmed_field, make_k2_notice, parse_f64, parse_u32, validate_enum, RowMap};
use crate::k1_parse::RawFile;

#[derive(Debug, Clone)]
pub struct FareAttributeRecord {
    pub fare_id: String,
    pub price: Option<f64>,
    pub currency_type: String,
    pub payment_method: Option<u32>,
    pub transfers: Option<u32>,
    pub transfer_duration: Option<u32>,
    pub agency_id: Option<String>,
    pub row: RowMap,
    pub line: u64,
}

pub fn validate_fare_attributes(
    file: &RawFile,
) -> (Vec<FareAttributeRecord>, Vec<gtfs_core::Notice>) {
    let mut notices = Vec::new();
    let mut records = Vec::new();
    let mut counter = 0;

    for (row_idx, row) in file.rows.iter().enumerate() {
        let line = (row_idx + 2) as u64;
        let row_map = build_row_map(&file.headers, row);
        let fare_id = get_trimmed_field(&row_map, "fare_id").unwrap_or("").to_string();
        let entity_id = (!fare_id.is_empty()).then_some(fare_id.clone());

        let price = match parse_f64(&row_map, "price") {
            Ok(value) => {
                if let Some(v) = value {
                    if v < 0.0 {
                        notices.push(make_k2_notice(
                            &mut counter, "FAR_002", EntityType::Fare, entity_id.clone(), Some(&row_map),
                            &file.name, Some(line), Some("price"), Some(v.to_string()), Some(">= 0".to_string()),
                            "price negatif olamaz.".to_string(), "price alanını sıfır veya pozitif bir değere ayarlayın.",
                        ));
                    }
                }
                value
            }
            Err(err) => {
                notices.push(make_k2_notice(
                    &mut counter, "FAR_002", EntityType::Fare, entity_id.clone(), Some(&row_map),
                    &file.name, Some(line), Some("price"), get_trimmed_field(&row_map, "price").map(str::to_string),
                    None, err, "price için geçerli bir sayısal değer girin.",
                ));
                None
            }
        };

        let currency_type = get_trimmed_field(&row_map, "currency_type").unwrap_or("").to_string();
        if currency_type.len() != 3 || !currency_type.chars().all(|c| c.is_ascii_uppercase()) {
            notices.push(make_k2_notice(
                &mut counter, "FAR_003", EntityType::Fare, entity_id.clone(), Some(&row_map),
                &file.name, Some(line), Some("currency_type"), Some(currency_type.clone()),
                Some("ISO 4217".to_string()), "currency_type geçerli bir ISO 4217 kodu değil.".to_string(),
                "3 harfli büyük harf ISO 4217 para birimi kodu kullanın (örn. TRY, EUR).",
            ));
        }

        let payment_method = parse_enum_u32(
            &row_map, &mut notices, &mut counter, "FAR_004", "payment_method", &["0","1"], &entity_id, line, &file.name
        );
        let transfers = parse_enum_u32(
            &row_map, &mut notices, &mut counter, "FAR_005", "transfers", &["0","1","2"], &entity_id, line, &file.name
        );

        let transfer_duration = match parse_u32(&row_map, "transfer_duration") {
            Ok(value) => {
                if let Some(v) = value {
                    if v == 0 {
                        notices.push(make_k2_notice(
                            &mut counter, "FAR_006", EntityType::Fare, entity_id.clone(), Some(&row_map),
                            &file.name, Some(line), Some("transfer_duration"), Some(v.to_string()),
                            Some("> 0".to_string()), "transfer_duration pozitif olmalıdır.".to_string(),
                            "transfer_duration alanını pozitif bir tam sayıya ayarlayın veya boş bırakın.",
                        ));
                    }
                }
                value
            }
            Err(err) => {
                notices.push(make_k2_notice(
                    &mut counter, "FAR_006", EntityType::Fare, entity_id.clone(), Some(&row_map),
                    &file.name, Some(line), Some("transfer_duration"),
                    get_trimmed_field(&row_map, "transfer_duration").map(str::to_string),
                    Some("> 0".to_string()), err,
                    "transfer_duration değerini pozitif bir tam sayıya ayarlayın veya boş bırakın.",
                ));
                None
            }
        };

        records.push(FareAttributeRecord {
            fare_id,
            price,
            currency_type,
            payment_method,
            transfers,
            transfer_duration,
            agency_id: get_trimmed_field(&row_map, "agency_id").filter(|v| !v.is_empty()).map(str::to_string),
            row: row_map,
            line,
        });
    }

    (records, notices)
}

fn parse_enum_u32(
    row_map: &RowMap,
    notices: &mut Vec<gtfs_core::Notice>,
    counter: &mut u32,
    rule_id: &str,
    field: &str,
    allowed: &[&str],
    entity_id: &Option<String>,
    line: u64,
    file_name: &str,
) -> Option<u32> {
    match parse_u32(row_map, field) {
        Ok(value) => {
            if let Some(v) = value {
                if !validate_enum(&v.to_string(), allowed) {
                    notices.push(make_k2_notice(counter, rule_id, EntityType::Fare, entity_id.clone(), Some(row_map), file_name, Some(line), Some(field), Some(v.to_string()), None, format!("{field} alanı geçerli bir enum değeri değil."), "Alanı geçerli bir spec enum değerine ayarlayın."));
                }
            }
            value
        }
        Err(err) => {
            notices.push(make_k2_notice(counter, rule_id, EntityType::Fare, entity_id.clone(), Some(row_map), file_name, Some(line), Some(field), get_trimmed_field(row_map, field).map(str::to_string), None, err, "Alanı geçerli bir spec enum değerine ayarlayın."));
            None
        }
    }
}
