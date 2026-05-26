use gtfs_core::EntityType;

use super::common::{build_row_map, get_trimmed_field, make_k2_notice, parse_f64, RowMap};
use crate::k1_parse::RawFile;

#[derive(Debug, Clone)]
pub struct FareProductRecord {
    pub fare_product_id: String,
    pub fare_product_name: Option<String>,
    pub rider_category_id: Option<String>,
    pub fare_media_id: Option<String>,
    pub amount: Option<f64>,
    pub currency: String,
    pub row: RowMap,
    pub line: u64,
}

pub fn validate_fare_products(
    file: &RawFile,
) -> (Vec<FareProductRecord>, Vec<gtfs_core::Notice>) {
    let mut notices = Vec::new();
    let mut records = Vec::new();
    let mut counter = 0;

    for (row_idx, row) in file.rows.iter().enumerate() {
        let line = (row_idx + 2) as u64;
        let row_map = build_row_map(&file.headers, row);
        let id = get_trimmed_field(&row_map, "fare_product_id").unwrap_or("").to_string();
        let entity_id = (!id.is_empty()).then_some(id.clone());

        let amount = match parse_f64(&row_map, "amount") {
            Ok(value) => {
                if let Some(v) = value {
                    if v < 0.0 {
                        notices.push(make_k2_notice(
                            &mut counter, "FPD_002", EntityType::Row, entity_id.clone(), Some(&row_map),
                            &file.name, Some(line), Some("amount"), Some(v.to_string()),
                            Some(">= 0".to_string()),
                            "amount negatif olamaz.".to_string(),
                            "amount alanını sıfır veya pozitif bir değere ayarlayın.",
                        ));
                    }
                } else {
                    notices.push(make_k2_notice(
                        &mut counter, "FPD_002", EntityType::Row, entity_id.clone(), Some(&row_map),
                        &file.name, Some(line), Some("amount"), None,
                        Some(">= 0".to_string()),
                        "amount zorunludur.".to_string(),
                        "Fare ürünü için bir amount (tutar) girin.",
                    ));
                }
                value
            }
            Err(err) => {
                notices.push(make_k2_notice(
                    &mut counter, "FPD_002", EntityType::Row, entity_id.clone(), Some(&row_map),
                    &file.name, Some(line), Some("amount"),
                    get_trimmed_field(&row_map, "amount").map(str::to_string),
                    None, err,
                    "amount için geçerli bir sayısal değer girin.",
                ));
                None
            }
        };

        let currency = get_trimmed_field(&row_map, "currency").unwrap_or("").to_string();
        if currency.len() != 3 || !currency.chars().all(|c| c.is_ascii_uppercase()) {
            notices.push(make_k2_notice(
                &mut counter, "FPD_003", EntityType::Row, entity_id.clone(), Some(&row_map),
                &file.name, Some(line), Some("currency"), Some(currency.clone()),
                Some("ISO 4217".to_string()),
                "currency geçerli bir ISO 4217 kodu değil.".to_string(),
                "3 harfli büyük harf ISO 4217 para birimi kodu kullanın (örn. TRY, EUR, USD).",
            ));
        }

        records.push(FareProductRecord {
            fare_product_id: id,
            fare_product_name: get_trimmed_field(&row_map, "fare_product_name")
                .filter(|v| !v.is_empty())
                .map(str::to_string),
            rider_category_id: get_trimmed_field(&row_map, "rider_category_id")
                .filter(|v| !v.is_empty())
                .map(str::to_string),
            fare_media_id: get_trimmed_field(&row_map, "fare_media_id")
                .filter(|v| !v.is_empty())
                .map(str::to_string),
            amount,
            currency,
            row: row_map,
            line,
        });
    }

    (records, notices)
}
