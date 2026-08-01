use gtfs_core::EntityType;

use super::common::{build_row_map, get_trimmed_field, looks_like_url, make_k2_notice, parse_u32, validate_enum, RowMap};
use crate::k1_parse::RawFile;

#[derive(Debug, Clone)]
pub struct RiderCategoryRecord {
    pub rider_category_id: String,
    pub rider_category_name: String,
    pub is_default_fare_category: Option<u32>,
    pub min_age: Option<u32>,
    pub max_age: Option<u32>,
    /// Spec alan adı `eligibility_url`. (2026-08-01'e kadar parser
    /// `rider_category_eligibility_url` okuyordu — spec'te böyle bir alan YOK, dolayısıyla
    /// geçerli her değer sessizce kayboluyordu.)
    pub eligibility_url: Option<String>,
    pub row: RowMap,
    pub line: u64,
}

pub fn validate_rider_categories(
    file: &RawFile,
) -> (Vec<RiderCategoryRecord>, Vec<gtfs_core::Notice>) {
    let mut notices = Vec::new();
    let mut records = Vec::new();
    let mut counter = 0;

    for (row_idx, row) in file.rows.iter().enumerate() {
        let line = (row_idx + 2) as u64;
        let row_map = build_row_map(&file.headers, row);
        let id = get_trimmed_field(&row_map, "rider_category_id").unwrap_or("").to_string();
        let entity_id = (!id.is_empty()).then_some(id.clone());

        let name = get_trimmed_field(&row_map, "rider_category_name").unwrap_or("").to_string();
        // sütun başlıkta yoksa ARC_025 devralır → atla
        if get_trimmed_field(&row_map, "rider_category_name") == Some("") {
            notices.push(make_k2_notice(
                &mut counter, "RCT_002", EntityType::Row, entity_id.clone(), Some(&row_map),
                &file.name, Some(line), Some("rider_category_name"), None,
                Some("dolu".to_string()),
                "rider_category_name zorunludur.".to_string(),
                "Her yolcu kategorisi için bir isim girin.",
            ));
        }

        let is_default = match parse_u32(&row_map, "is_default_fare_category") {
            Ok(value) => {
                if let Some(v) = value {
                    if !validate_enum(&v.to_string(), &["0", "1"]) {
                        notices.push(make_k2_notice(
                            &mut counter, "RCT_003", EntityType::Row, entity_id.clone(), Some(&row_map),
                            &file.name, Some(line), Some("is_default_fare_category"), Some(v.to_string()),
                            Some("0 veya 1".to_string()),
                            "is_default_fare_category geçerli bir enum değeri değil.".to_string(),
                            "0 (varsayılan değil) veya 1 (varsayılan) kullanın.",
                        ));
                    }
                }
                value
            }
            Err(err) => {
                notices.push(make_k2_notice(
                    &mut counter, "RCT_003", EntityType::Row, entity_id.clone(), Some(&row_map),
                    &file.name, Some(line), Some("is_default_fare_category"),
                    get_trimmed_field(&row_map, "is_default_fare_category").map(str::to_string),
                    Some("0 veya 1".to_string()), err,
                    "0 veya 1 kullanın.",
                ));
                None
            }
        };

        let min_age = match parse_u32(&row_map, "min_age") {
            Ok(v) => v,
            Err(err) => {
                notices.push(make_k2_notice(
                    &mut counter, "RCT_004", EntityType::Row, entity_id.clone(), Some(&row_map),
                    &file.name, Some(line), Some("min_age"),
                    get_trimmed_field(&row_map, "min_age").map(str::to_string),
                    Some(">= 0".to_string()), err,
                    "min_age için negatif olmayan bir tam sayı girin.",
                ));
                None
            }
        };

        let max_age = match parse_u32(&row_map, "max_age") {
            Ok(v) => v,
            Err(err) => {
                notices.push(make_k2_notice(
                    &mut counter, "RCT_004", EntityType::Row, entity_id.clone(), Some(&row_map),
                    &file.name, Some(line), Some("max_age"),
                    get_trimmed_field(&row_map, "max_age").map(str::to_string),
                    Some(">= 0".to_string()), err,
                    "max_age için negatif olmayan bir tam sayı girin.",
                ));
                None
            }
        };

        if let (Some(mn), Some(mx)) = (min_age, max_age) {
            if mx < mn {
                notices.push(make_k2_notice(
                    &mut counter, "RCT_005", EntityType::Row, entity_id.clone(), Some(&row_map),
                    &file.name, Some(line), Some("max_age"), Some(mx.to_string()),
                    Some(format!(">= {mn}")),
                    format!("max_age ({mx}) min_age ({mn}) değerinden küçük olamaz."),
                    "max_age değerini min_age'e eşit veya daha büyük yapın.",
                ));
            }
        }

        // RCT_007: spec tipi `URL` — stop_url→STP_042, route_url→RTS_005 ile aynı desen.
        let eligibility_url = get_trimmed_field(&row_map, "eligibility_url")
            .filter(|v| !v.is_empty())
            .map(str::to_string);
        if let Some(ref url) = eligibility_url {
            if !looks_like_url(url) {
                notices.push(make_k2_notice(
                    &mut counter, "RCT_007", EntityType::Row, Some(id.clone()), Some(&row_map),
                    &file.name, Some(line), Some("eligibility_url"), Some(url.clone()), None,
                    "eligibility_url geçerli bir URL değil.".to_string(),
                    "eligibility_url için geçerli bir http/https URL'si kullanın.",
                ));
            }
        }

        records.push(RiderCategoryRecord {
            rider_category_id: id,
            rider_category_name: name,
            is_default_fare_category: is_default,
            min_age,
            max_age,
            eligibility_url,
            row: row_map,
            line,
        });
    }

    (records, notices)
}
