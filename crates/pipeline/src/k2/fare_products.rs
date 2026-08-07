use gtfs_core::EntityType;
use std::collections::HashMap;

use super::common::{amount_has_iso4217_decimals, iso4217_minor_unit, build_row_map, get_trimmed_field, make_k2_notice, parse_f64, RowMap};
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
    // FPD_007: dosya başına TEK özet — ölçümde tek feed 1,1M satır üretiyordu.
    let mut iso_bad: u64 = 0;
    let mut iso_first: Option<(u64, String, String)> = None;

    for (row_idx, row) in file.rows.iter().enumerate() {
        let line = (row_idx + 2) as u64;
        let row_map = build_row_map(&file.headers, row);
        let id = get_trimmed_field(&row_map, "fare_product_id").unwrap_or("").to_string();
        let entity_id = (!id.is_empty()).then_some(id.clone());

        // FPD_007: tutar, para biriminin ISO 4217 ondalık basamak sayısını taşımalı.
        {
            let a = get_trimmed_field(&row_map, "amount").unwrap_or("");
            let c = get_trimmed_field(&row_map, "currency").unwrap_or("");
            if !amount_has_iso4217_decimals(a, c) {
                iso_bad += 1;
                if iso_first.is_none() {
                    iso_first = Some((line, a.to_string(), c.to_string()));
                }
            }
        }

        // NEGATİF TUTAR GEÇERLİDİR — işaretlenmez. Spec (fare_products.amount):
        //   "May be negative to represent transfer discounts. May be zero to represent a
        //    fare product that is free."
        // Eskiden her negatif değer FPD_002 ile KRİTİK·Spec işaretleniyordu; aktarma
        // indirimi tanımlayan geçerli bir feed yayın skorunu kaybediyordu. FPD_002 artık
        // yalnız alanın EKSİK veya SAYISAL OLMAMASI durumunu ölçer.
        let amount = match parse_f64(&row_map, "amount") {
            Ok(value) => {
                if value.is_none() && get_trimmed_field(&row_map, "amount") == Some("") {
                    notices.push(make_k2_notice(
                        &mut counter, "FPD_002", EntityType::Row, entity_id.clone(), Some(&row_map),
                        &file.name, Some(line), Some("amount"), None,
                        Some("sayısal değer".to_string()),
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

        // GGL_002: ic_price (Google-özel alan) — varsa -1 veya pozitif olmalı
        if let Some(ic_price_raw) = get_trimmed_field(&row_map, "ic_price").filter(|v| !v.is_empty()) {
            match ic_price_raw.parse::<f64>() {
                Ok(v) if v >= 0.0 || (v - (-1.0)).abs() < 1e-9 => {}
                Ok(v) => {
                    notices.push(make_k2_notice(
                        &mut counter, "GGL_002", EntityType::Row, entity_id.clone(), Some(&row_map),
                        &file.name, Some(line), Some("ic_price"),
                        Some(v.to_string()), Some("-1 veya >= 0".to_string()),
                        format!("ic_price '{v}' geçersiz: -1 veya sıfırdan büyük bir değer olmalıdır."),
                        "ic_price değerini -1 (bilinmiyor) veya pozitif bir sayı olarak ayarlayın.",
                    ));
                }
                Err(_) => {
                    notices.push(make_k2_notice(
                        &mut counter, "GGL_002", EntityType::Row, entity_id.clone(), Some(&row_map),
                        &file.name, Some(line), Some("ic_price"),
                        Some(ic_price_raw.to_string()), Some("-1 veya >= 0".to_string()),
                        format!("ic_price '{ic_price_raw}' sayısal değil."),
                        "ic_price değerini -1 (bilinmiyor) veya pozitif bir sayı olarak ayarlayın.",
                    ));
                }
            }
        }

        let currency = get_trimmed_field(&row_map, "currency").unwrap_or("").to_string();
        // ⚠️ ISO 4217 AKTİF kod listesi (issue #82): eski denetim "üç büyük harf" idi,
        // `ZZZ` geçiyordu ve `iso4217_minor_unit` onu sessizce 2 ondalık sayıyordu.
        if !super::common::is_iso4217(&currency) {
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

    // FPD_006: aynı fare_product_id için birden fazla varsayılan (boş rider_category_id)
    let mut default_count: HashMap<&str, (u64, u32)> = HashMap::new();
    for rec in &records {
        if rec.rider_category_id.is_none() {
            let entry = default_count.entry(rec.fare_product_id.as_str()).or_insert((rec.line, 0));
            entry.1 += 1;
        }
    }
    for (fare_product_id, (first_line, count)) in &default_count {
        if *count > 1 {
            notices.push(make_k2_notice(
                &mut counter, "FPD_006", EntityType::Row,
                Some(fare_product_id.to_string()), None,
                &file.name, Some(*first_line), Some("rider_category_id"),
                Some(count.to_string()), Some("1".to_string()),
                format!("fare_product_id '{}' için {count} varsayılan rider category tanımlı (rider_category_id boş).", fare_product_id),
                "Bir fare_product'ta en fazla bir varsayılan (rider_category_id boş) kayıt olabilir.",
            ));
        }
    }

    // FPD_007: DOSYA başına tek özet (DQ_016/ARC_032 deseni). Satır başına emit ölçümde
    // 2 milyon notice demekti ve toplamın %99,7'si iki feed'den geliyordu.
    if let Some((line, amount, currency)) = iso_first {
        let want = iso4217_minor_unit(&currency);
        notices.push(make_k2_notice(
            &mut counter, "FPD_007", EntityType::File, None, None,
            &file.name, Some(line), Some("amount"),
            Some(format!("{iso_bad} satır")), Some(format!("{want} ondalık basamak")),
            format!(
                "fare_products.txt dosyasında {iso_bad} tutar, para biriminin ISO 4217 ondalık basamak \
                 sayısını taşımıyor (ör. satır {line}: {currency} {amount} — {want} basamak beklenir)."
            ),
            "Tutarları para biriminin ondalık basamak sayısıyla yazın (ör. EUR için 0.90, JPY için 150).",
        ));
    }

    (records, notices)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn raw(rows: &[&str]) -> RawFile {
        RawFile {
            name: "fare_products.txt".to_string(),
            headers: vec!["fare_product_id".into(), "amount".into(), "currency".into()],
            rows: rows.iter().map(|r| r.split(',').map(Into::into).collect()).collect(),
            ..Default::default()
        }
    }

    /// Spec: "amount … May be negative to represent transfer discounts."
    /// Aktarma indirimi tanımlayan geçerli bir feed KRİTİK·Spec hatası almamalı.
    #[test]
    fn negative_amount_is_valid_and_not_flagged() {
        let (recs, notices) = validate_fare_products(&raw(&["P1,-1.50,USD"]));
        assert!(
            !notices.iter().any(|n| n.rule_id == "FPD_002"),
            "negatif tutar geçerlidir (aktarma indirimi) → FPD_002 çıkmamalı: {:?}",
            notices.iter().map(|n| &n.message).collect::<Vec<_>>()
        );
        assert_eq!(recs[0].amount, Some(-1.5), "değer kaydedilmeli");
    }

    #[test]
    fn zero_amount_is_valid() {
        let (_, notices) = validate_fare_products(&raw(&["P1,0,USD"]));
        assert!(!notices.iter().any(|n| n.rule_id == "FPD_002"), "sıfır tutar geçerlidir (ücretsiz ürün)");
    }

    /// Daraltmanın bedeli olmamalı: eksik ve sayısal-olmayan tutar YİNE yakalanır.
    #[test]
    fn missing_or_non_numeric_amount_still_flagged() {
        let (_, n1) = validate_fare_products(&raw(&["P1,,USD"]));
        assert!(n1.iter().any(|n| n.rule_id == "FPD_002"), "boş tutar FPD_002 üretmeli");
        let (_, n2) = validate_fare_products(&raw(&["P1,abc,USD"]));
        assert!(n2.iter().any(|n| n.rule_id == "FPD_002"), "sayısal olmayan tutar FPD_002 üretmeli");
    }
}
