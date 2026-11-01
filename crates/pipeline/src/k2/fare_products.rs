use gtfs_core::EntityType;

use super::common::{
    amount_has_iso4217_decimals, build_row_map, get_raw_field, get_trimmed_field,
    iso4217_minor_unit, make_k2_notice, parse_f64, RowMap,
};
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

pub fn validate_fare_products(file: &RawFile) -> (Vec<FareProductRecord>, Vec<gtfs_core::Notice>) {
    let mut notices = Vec::new();
    let mut records = Vec::new();
    let mut counter = 0;
    // FPD_007: ONE summary per file — one feed produced 1.1M rows in the measurement.
    let mut iso_bad: u64 = 0;
    let mut iso_first: Option<(u64, String, String)> = None;

    for (row_idx, row) in file.rows.iter().enumerate() {
        let line = (row_idx + 2) as u64;
        let row_map = build_row_map(&file.headers, row);
        let id = get_raw_field(&row_map, "fare_product_id")
            .unwrap_or("")
            .to_string();
        let entity_id = (!id.is_empty()).then_some(id.clone());

        // FPD_007: the amount must use the currency's ISO 4217 number of decimal places.
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

        // NEGATIVE AMOUNTS ARE VALID — do not flag them. Spec (fare_products.amount):
        //   "May be negative to represent transfer discounts. May be zero to represent a
        //    fare product that is free."
        // Previously every negative value was flagged as Kritik·Spec by FPD_002; a valid feed
        // defining a transfer discount lost its publication score. FPD_002 now measures only
        // that the field is MISSING or NON-NUMERIC.
        let amount = match parse_f64(&row_map, "amount") {
            Ok(value) => {
                if value.is_none() && get_trimmed_field(&row_map, "amount") == Some("") {
                    notices.push(make_k2_notice(
                        &mut counter,
                        "FPD_002",
                        EntityType::Row,
                        entity_id.clone(),
                        Some(&row_map),
                        &file.name,
                        Some(line),
                        Some("amount"),
                        None,
                        Some("sayısal değer".to_string()),
                        "amount zorunludur.".to_string(),
                        "Fare ürünü için bir amount (tutar) girin.",
                    ));
                }
                value
            }
            Err(err) => {
                notices.push(make_k2_notice(
                    &mut counter,
                    "FPD_002",
                    EntityType::Row,
                    entity_id.clone(),
                    Some(&row_map),
                    &file.name,
                    Some(line),
                    Some("amount"),
                    get_trimmed_field(&row_map, "amount").map(str::to_string),
                    None,
                    err,
                    "amount için geçerli bir sayısal değer girin.",
                ));
                None
            }
        };

        let currency = get_trimmed_field(&row_map, "currency")
            .unwrap_or("")
            .to_string();
        // ⚠️ ISO 4217 ACTIVE code list (issue #82): the old check was "three uppercase letters",
        // so `ZZZ` passed and `iso4217_minor_unit` silently treated it as two decimal places.
        if !super::common::is_iso4217(&currency) {
            notices.push(make_k2_notice(
                &mut counter,
                "FPD_003",
                EntityType::Row,
                entity_id.clone(),
                Some(&row_map),
                &file.name,
                Some(line),
                Some("currency"),
                Some(currency.clone()),
                Some("ISO 4217".to_string()),
                "currency geçerli bir ISO 4217 kodu değil.".to_string(),
                "3 harfli büyük harf ISO 4217 para birimi kodu kullanın (örn. TRY, EUR, USD).",
            ));
        }

        records.push(FareProductRecord {
            fare_product_id: id,
            fare_product_name: get_trimmed_field(&row_map, "fare_product_name")
                .filter(|v| !v.trim().is_empty())
                .map(str::to_string),
            rider_category_id: get_raw_field(&row_map, "rider_category_id")
                .filter(|v| !v.trim().is_empty())
                .map(str::to_string),
            fare_media_id: get_raw_field(&row_map, "fare_media_id")
                .filter(|v| !v.trim().is_empty())
                .map(str::to_string),
            amount,
            currency,
            row: row_map,
            line,
        });
    }

    // FPD_007: one summary per FILE (DQ_016/ARC_032 pattern). Per-row emission meant
    // 2 million notices in the measurement, with 99.7% coming from two feeds.
    if let Some((line, amount, currency)) = iso_first {
        // ⚠️ `None` = the code is not in the table or its decimal places are undefined;
        // the message must say this instead of inventing a "2 places" expectation.
        let want = iso4217_minor_unit(&currency)
            .map(|u| u.to_string())
            .unwrap_or_else(|| "tanımsız".to_string());
        notices.push(make_k2_notice(
            &mut counter, "FPD_007", EntityType::File, None, None,
            &file.name, Some(line), Some("amount"),
            Some(format!("{iso_bad} rows")), Some(format!("{want} decimal places")),
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
            rows: rows
                .iter()
                .map(|r| r.split(',').map(Into::into).collect())
                .collect(),
            ..Default::default()
        }
    }

    /// Spec: "amount … May be negative to represent transfer discounts."
    /// A valid feed defining a transfer discount must not receive a Kritik·Spec finding.
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
        assert!(
            !notices.iter().any(|n| n.rule_id == "FPD_002"),
            "sıfır tutar geçerlidir (ücretsiz ürün)"
        );
    }

    /// Narrowing the rule must not weaken it: missing and non-numeric amounts are STILL caught.
    #[test]
    fn missing_or_non_numeric_amount_still_flagged() {
        let (_, n1) = validate_fare_products(&raw(&["P1,,USD"]));
        assert!(
            n1.iter().any(|n| n.rule_id == "FPD_002"),
            "boş tutar FPD_002 üretmeli"
        );
        let (_, n2) = validate_fare_products(&raw(&["P1,abc,USD"]));
        assert!(
            n2.iter().any(|n| n.rule_id == "FPD_002"),
            "sayısal olmayan tutar FPD_002 üretmeli"
        );
    }
}
