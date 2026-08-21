use gtfs_core::EntityType;

use super::common::{get_raw_field, amount_has_iso4217_decimals, iso4217_minor_unit, build_row_map, get_trimmed_field, make_k2_notice, parse_f64, parse_u32, validate_enum, RowMap};
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
    // FAR_013: dosya başına TEK özet — ölçümde tek feed 1,1M satır üretiyordu.
    let mut iso_bad: u64 = 0;
    let mut iso_first: Option<(u64, String, String)> = None;

    for (row_idx, row) in file.rows.iter().enumerate() {
        let line = (row_idx + 2) as u64;
        let row_map = build_row_map(&file.headers, row);
        let fare_id = get_raw_field(&row_map, "fare_id").unwrap_or("").to_string();
        // FAR_012: fare_id required (sütun yoksa ARC_025 devralır → atla)
        if get_raw_field(&row_map, "fare_id").map(str::trim) == Some("") {
            crate::notice_budget::push(&mut notices, make_k2_notice(
                &mut counter, "FAR_012", EntityType::Fare, None, Some(&row_map),
                &file.name, Some(line), Some("fare_id"), Some(String::new()), None,
                "fare_id zorunludur.".to_string(), "Her ücret tanımına benzersiz bir fare_id verin.",
            ));
        }
        let entity_id = (!fare_id.is_empty()).then_some(fare_id.clone());

        // FAR_013: tutar, para biriminin ISO 4217 ondalık basamak sayısını taşımalı.
        {
            let a = get_trimmed_field(&row_map, "price").unwrap_or("");
            let c = get_trimmed_field(&row_map, "currency_type").unwrap_or("");
            if !amount_has_iso4217_decimals(a, c) {
                iso_bad += 1;
                if iso_first.is_none() {
                    iso_first = Some((line, a.to_string(), c.to_string()));
                }
            }
        }

        let price = match parse_f64(&row_map, "price") {
            Ok(value) => {
                if let Some(v) = value {
                    if v < 0.0 {
                        crate::notice_budget::push(&mut notices, make_k2_notice(
                            &mut counter, "FAR_002", EntityType::Fare, entity_id.clone(), Some(&row_map),
                            &file.name, Some(line), Some("price"), Some(v.to_string()), Some(">= 0".to_string()),
                            "price negatif olamaz.".to_string(), "price alanını sıfır veya pozitif bir değere ayarlayın.",
                        ));
                    }
                }
                value
            }
            Err(err) => {
                crate::notice_budget::push(&mut notices, make_k2_notice(
                    &mut counter, "FAR_002", EntityType::Fare, entity_id.clone(), Some(&row_map),
                    &file.name, Some(line), Some("price"), get_trimmed_field(&row_map, "price").map(str::to_string),
                    None, err, "price için geçerli bir sayısal değer girin.",
                ));
                None
            }
        };

        let currency_type = get_trimmed_field(&row_map, "currency_type").unwrap_or("").to_string();
        // ⚠️ ISO 4217 AKTİF kod listesi (issue #82): eski denetim "üç büyük harf" idi,
        // `ZZZ` geçiyordu ve `iso4217_minor_unit` onu sessizce 2 ondalık sayıyordu.
        if !super::common::is_iso4217(&currency_type) {
            crate::notice_budget::push(&mut notices, make_k2_notice(
                &mut counter, "FAR_003", EntityType::Fare, entity_id.clone(), Some(&row_map),
                &file.name, Some(line), Some("currency_type"), Some(currency_type.clone()),
                Some("ISO 4217".to_string()), "currency_type geçerli bir ISO 4217 kodu değil.".to_string(),
                "3 harfli büyük harf ISO 4217 para birimi kodu kullanın (örn. TRY, EUR).",
            ));
        }

        let payment_method = parse_enum_u32(
            &row_map, &mut notices, &mut counter, "FAR_004", "payment_method", &["0","1"], &entity_id, line, &file.name
        );
        // FAR_011: payment_method required (sütun yoksa ARC_025 devralır → atla)
        if get_trimmed_field(&row_map, "payment_method") == Some("") {
            crate::notice_budget::push(&mut notices, make_k2_notice(
                &mut counter, "FAR_011", EntityType::Fare, entity_id.clone(), Some(&row_map),
                &file.name, Some(line), Some("payment_method"), Some(String::new()), None,
                "payment_method zorunludur.".to_string(),
                "payment_method değerini 0 (peşin) veya 1 (önceden) olarak girin.",
            ));
        }
        let transfers = parse_enum_u32(
            &row_map, &mut notices, &mut counter, "FAR_005", "transfers", &["0","1","2"], &entity_id, line, &file.name
        );

        let transfer_duration = match parse_u32(&row_map, "transfer_duration") {
            Ok(value) => value,
            Err(err) => {
                crate::notice_budget::push(&mut notices, make_k2_notice(
                    &mut counter, "FAR_006", EntityType::Fare, entity_id.clone(), Some(&row_map),
                    &file.name, Some(line), Some("transfer_duration"),
                    get_trimmed_field(&row_map, "transfer_duration").map(str::to_string),
                    Some(">= 0".to_string()), err,
                    "transfer_duration değerini negatif olmayan bir tam sayıya ayarlayın veya boş bırakın.",
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
            agency_id: get_raw_field(&row_map, "agency_id").filter(|v| !v.trim().is_empty()).map(str::to_string),
            row: row_map,
            line,
        });
    }

    // FAR_013: DOSYA başına tek özet (DQ_016/ARC_032 deseni). Satır başına emit ölçümde
    // 2 milyon notice demekti ve toplamın %99,7'si iki feed'den geliyordu.
    if let Some((line, amount, currency)) = iso_first {
        // ⚠️ `None` = kod tabloda yok ya da ondalığı tanımsız; mesajda sayı yerine bunu
        // söylemek gerekir — uydurma bir "2 basamak" beklentisi yazmak yanlış olurdu.
        let want = iso4217_minor_unit(&currency)
            .map(|u| u.to_string())
            .unwrap_or_else(|| "tanımsız".to_string());
        crate::notice_budget::push(&mut notices, make_k2_notice(
            &mut counter, "FAR_013", EntityType::File, None, None,
            &file.name, Some(line), Some("price"),
            Some(format!("{iso_bad} satır")), Some(format!("{want} ondalık basamak")),
            format!(
                "fare_attributes.txt dosyasında {iso_bad} tutar, para biriminin ISO 4217 ondalık basamak \
                 sayısını taşımıyor (ör. satır {line}: {currency} {amount} — {want} basamak beklenir)."
            ),
            "Tutarları para biriminin ondalık basamak sayısıyla yazın (ör. EUR için 0.90, JPY için 150).",
        ));
    }

    (records, notices)
}

// Bu yardımcı farklı FAR enum alanlarını aynı açık parse/emit tablosuyla işler;
// tek kullanımlık bir parametre struct'ı semantik alanları daha görünür kılmaz.
#[allow(clippy::too_many_arguments)]
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
                    crate::notice_budget::push(notices, make_k2_notice(counter, rule_id, EntityType::Fare, entity_id.clone(), Some(row_map), file_name, Some(line), Some(field), Some(v.to_string()), None, format!("{field} alanı geçerli bir enum değeri değil."), "Alanı geçerli bir spec enum değerine ayarlayın."));
                }
            }
            value
        }
        Err(err) => {
            crate::notice_budget::push(notices, make_k2_notice(counter, rule_id, EntityType::Fare, entity_id.clone(), Some(row_map), file_name, Some(line), Some(field), get_trimmed_field(row_map, field).map(str::to_string), None, err, "Alanı geçerli bir spec enum değerine ayarlayın."));
            None
        }
    }
}
