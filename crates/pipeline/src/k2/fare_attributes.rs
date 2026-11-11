use gtfs_core::EntityType;

use super::common::{
    amount_has_iso4217_decimals, build_row_map, get_raw_field, get_trimmed_field,
    iso4217_minor_unit, make_k2_notice, parse_f64, parse_u32, validate_enum,
    whitespace_only_parser_failure, RowMap,
};
use crate::k1_parse::RawFile;
use crate::WhitespaceSuppressions;
use smol_str::SmolStr;
use std::sync::Arc;

#[derive(Debug, Clone)]
pub struct FareAttributeRecord {
    pub fare_id: String,
    pub price: Option<f64>,
    pub currency_type: String,
    pub payment_method: Option<u32>,
    pub transfers: Option<u32>,
    pub transfer_duration: Option<u32>,
    pub agency_id: Option<String>,
    /// The file's column names; all records share the same list.
    pub columns: Arc<[String]>,
    /// The row's raw values (in `columns` order; extra cells are discarded).
    ///
    /// Previously a `RowMap` was stored per row: in large fare_attributes files
    /// (mdb-992), millions of HashMaps stayed in memory, increasing this stage alone from 1.2 s
    /// to 8.3 s (wasm64), and the whole feed from 12.9 s to 23.8 s.
    pub values: Box<[SmolStr]>,
    pub line: u64,
}

impl FareAttributeRecord {
    /// Same behavior as the old `RowMap::get`: `None` if the column is absent; `Some("")`
    /// if the row is short; the last column wins for a duplicate header.
    pub fn raw_field(&self, field: &str) -> Option<&str> {
        let col = self.columns.iter().rposition(|h| h == field)?;
        Some(self.values.get(col).map(SmolStr::as_str).unwrap_or(""))
    }
}

pub fn validate_fare_attributes(
    file: &RawFile,
) -> (Vec<FareAttributeRecord>, Vec<gtfs_core::Notice>) {
    let (records, notices, _) = validate_fare_attributes_inner(file, false);
    (records, notices)
}

pub(super) fn validate_fare_attributes_with_suppression(
    file: &RawFile,
) -> (
    Vec<FareAttributeRecord>,
    Vec<gtfs_core::Notice>,
    WhitespaceSuppressions,
) {
    validate_fare_attributes_inner(file, true)
}

fn validate_fare_attributes_inner(
    file: &RawFile,
    suppress_whitespace_derivatives: bool,
) -> (
    Vec<FareAttributeRecord>,
    Vec<gtfs_core::Notice>,
    WhitespaceSuppressions,
) {
    let mut notices = Vec::new();
    let mut records = Vec::new();
    let mut whitespace_suppressions = WhitespaceSuppressions::default();
    let mut counter = 0;
    // FAR_013: ONE summary per file — one feed produced 1.1M rows in the measurement.
    let mut iso_bad: u64 = 0;
    let mut iso_first: Option<(u64, String, String)> = None;
    let columns: Arc<[String]> = file.headers.clone().into();

    for (row_idx, row) in file.rows.iter().enumerate() {
        let line = (row_idx + 2) as u64;
        let row_map = build_row_map(&file.headers, row);
        let fare_id = get_raw_field(&row_map, "fare_id").unwrap_or("").to_string();
        // FAR_012: fare_id required (if the column is absent, ARC_025 takes over → skip)
        if get_raw_field(&row_map, "fare_id").map(str::trim) == Some("") {
            notices.push(make_k2_notice(
                &mut counter,
                "FAR_012",
                EntityType::Fare,
                None,
                Some(&row_map),
                &file.name,
                Some(line),
                Some("fare_id"),
                Some(String::new()),
                None,
                "fare_id zorunludur.".to_string(),
                "Give every fare a unique fare_id.",
            ));
        }
        let entity_id = (!fare_id.is_empty()).then_some(fare_id.clone());

        // FAR_013: the amount must use the currency's ISO 4217 number of decimal places.
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
                if value.is_none() && get_trimmed_field(&row_map, "price") == Some("") {
                    notices.push(make_k2_notice(
                        &mut counter,
                        "FAR_002",
                        EntityType::Fare,
                        entity_id.clone(),
                        Some(&row_map),
                        &file.name,
                        Some(line),
                        Some("price"),
                        Some(String::new()),
                        Some("sayısal değer".to_string()),
                        "price zorunludur ve boş bırakılamaz.".to_string(),
                        "price alanına geçerli bir sayısal değer girin.",
                    ));
                }
                if let Some(v) = value {
                    if v < 0.0 {
                        notices.push(make_k2_notice(
                            &mut counter,
                            "FAR_002",
                            EntityType::Fare,
                            entity_id.clone(),
                            Some(&row_map),
                            &file.name,
                            Some(line),
                            Some("price"),
                            Some(v.to_string()),
                            Some(">= 0".to_string()),
                            "price negatif olamaz.".to_string(),
                            "price alanını sıfır veya pozitif bir değere ayarlayın.",
                        ));
                    }
                }
                value
            }
            Err(err) => {
                let observed = get_trimmed_field(&row_map, "price");
                if suppress_whitespace_derivatives
                    && whitespace_only_parser_failure("FAR_002", &row_map, "price", observed)
                {
                    whitespace_suppressions.record(&file.name, "FAR_002");
                } else {
                    notices.push(make_k2_notice(
                        &mut counter,
                        "FAR_002",
                        EntityType::Fare,
                        entity_id.clone(),
                        Some(&row_map),
                        &file.name,
                        Some(line),
                        Some("price"),
                        observed.map(str::to_string),
                        None,
                        err,
                        "price için geçerli bir sayısal değer girin.",
                    ));
                }
                None
            }
        };

        // GGL_002: Google Transit's Japan-specific ic_price extension is defined in
        // fare_attributes.txt, not in Fares v2 fare_products.txt.
        // When present, the value must be -1 (discount unknown) or zero/positive.
        if let Some(ic_price_raw) =
            get_trimmed_field(&row_map, "ic_price").filter(|v| !v.trim().is_empty())
        {
            match ic_price_raw.parse::<f64>() {
                Ok(v) if v >= 0.0 || (v - (-1.0)).abs() < 1e-9 => {}
                Ok(v) => {
                    notices.push(make_k2_notice(
                        &mut counter,
                        "GGL_002",
                        EntityType::Row,
                        entity_id.clone(),
                        Some(&row_map),
                        &file.name,
                        Some(line),
                        Some("ic_price"),
                        Some(v.to_string()),
                        Some("-1 or >= 0".to_string()),
                        format!(
                            "ic_price '{v}' geçersiz: -1 veya sıfır/pozitif (>= 0) bir değer olmalıdır."
                        ),
                        "Set ic_price to -1 (unknown) or a zero-or-positive number.",
                    ));
                }
                Err(_) => {
                    notices.push(make_k2_notice(
                        &mut counter,
                        "GGL_002",
                        EntityType::Row,
                        entity_id.clone(),
                        Some(&row_map),
                        &file.name,
                        Some(line),
                        Some("ic_price"),
                        Some(ic_price_raw.to_string()),
                        Some("-1 or >= 0".to_string()),
                        format!("ic_price '{ic_price_raw}' sayısal değil."),
                        "Set ic_price to -1 (unknown) or a zero-or-positive number.",
                    ));
                }
            }
        }

        let currency_type = get_trimmed_field(&row_map, "currency_type")
            .unwrap_or("")
            .to_string();
        // ⚠️ ISO 4217 ACTIVE code list (issue #82): the old check was "three uppercase letters",
        // so `ZZZ` passed and `iso4217_minor_unit` silently treated it as two decimal places.
        if !super::common::is_iso4217(&currency_type) {
            notices.push(make_k2_notice(
                &mut counter,
                "FAR_003",
                EntityType::Fare,
                entity_id.clone(),
                Some(&row_map),
                &file.name,
                Some(line),
                Some("currency_type"),
                Some(currency_type.clone()),
                Some("ISO 4217".to_string()),
                "currency_type geçerli bir ISO 4217 kodu değil.".to_string(),
                "Use a 3-letter uppercase ISO 4217 currency code (e.g., TRY, EUR).",
            ));
        }

        let payment_method = parse_enum_u32(
            &row_map,
            &mut notices,
            &mut counter,
            "FAR_004",
            "payment_method",
            &["0", "1"],
            &entity_id,
            line,
            &file.name,
            suppress_whitespace_derivatives,
            &mut whitespace_suppressions,
        );
        // FAR_011: payment_method required (if the column is absent, ARC_025 takes over → skip)
        if get_trimmed_field(&row_map, "payment_method") == Some("") {
            notices.push(make_k2_notice(
                &mut counter,
                "FAR_011",
                EntityType::Fare,
                entity_id.clone(),
                Some(&row_map),
                &file.name,
                Some(line),
                Some("payment_method"),
                Some(String::new()),
                None,
                "payment_method zorunludur.".to_string(),
                "Set payment_method to 0 (on board) or 1 (before boarding).",
            ));
        }
        let transfers = parse_enum_u32(
            &row_map,
            &mut notices,
            &mut counter,
            "FAR_005",
            "transfers",
            &["0", "1", "2"],
            &entity_id,
            line,
            &file.name,
            suppress_whitespace_derivatives,
            &mut whitespace_suppressions,
        );

        let transfer_duration = match parse_u32(&row_map, "transfer_duration") {
            Ok(value) => value,
            Err(err) => {
                notices.push(make_k2_notice(
                    &mut counter, "FAR_006", EntityType::Fare, entity_id.clone(), Some(&row_map),
                    &file.name, Some(line), Some("transfer_duration"),
                    get_trimmed_field(&row_map, "transfer_duration").map(str::to_string),
                    Some(">= 0".to_string()), err,
                    "Set transfer_duration to a positive integer or leave it empty.",
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
            agency_id: get_raw_field(&row_map, "agency_id")
                .filter(|v| !v.trim().is_empty())
                .map(str::to_string),
            columns: Arc::clone(&columns),
            values: row.iter().take(columns.len()).cloned().collect(),
            line,
        });
    }

    // FAR_013: one summary per FILE (DQ_016/ARC_032 pattern). Per-row emission meant
    // 2 million notices in the measurement, with 99.7% coming from two feeds.
    if let Some((line, amount, currency)) = iso_first {
        // ⚠️ `None` = the code is not in the table or its decimal places are undefined;
        // the message must say this instead of inventing a "2 places" expectation.
        let want = iso4217_minor_unit(&currency)
            .map(|u| u.to_string())
            .unwrap_or_else(|| "tanımsız".to_string());
        notices.push(make_k2_notice(
            &mut counter, "FAR_013", EntityType::File, None, None,
            &file.name, Some(line), Some("price"),
            Some(format!("{iso_bad} rows")), Some(format!("{want} decimal places")),
            format!(
                "fare_attributes.txt dosyasında {iso_bad} tutar, para biriminin ISO 4217 ondalık basamak \
                 sayısını taşımıyor (ör. satır {line}: {currency} {amount} — {want} basamak beklenir)."
            ),
            "Write amounts with the decimal places of their currency (for example 0.90 for EUR, 150 for JPY).",
        ));
    }

    (records, notices, whitespace_suppressions)
}

// This helper handles different FAR enum fields with the same explicit parse/emit table;
// a one-use parameter struct would not make the semantic fields clearer.
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
    suppress_whitespace_derivatives: bool,
    whitespace_suppressions: &mut WhitespaceSuppressions,
) -> Option<u32> {
    match parse_u32(row_map, field) {
        Ok(value) => {
            if let Some(v) = value {
                if !validate_enum(&v.to_string(), allowed) {
                    notices.push(make_k2_notice(
                        counter,
                        rule_id,
                        EntityType::Fare,
                        entity_id.clone(),
                        Some(row_map),
                        file_name,
                        Some(line),
                        Some(field),
                        Some(v.to_string()),
                        None,
                        format!("{field} alanı geçerli bir enum değeri değil."),
                        "Set the field to a valid spec enum value.",
                    ));
                }
            }
            value
        }
        Err(err) => {
            let observed = get_trimmed_field(row_map, field);
            if suppress_whitespace_derivatives
                && whitespace_only_parser_failure(rule_id, row_map, field, observed)
            {
                whitespace_suppressions.record(file_name, rule_id);
            } else {
                notices.push(make_k2_notice(
                    counter,
                    rule_id,
                    EntityType::Fare,
                    entity_id.clone(),
                    Some(row_map),
                    file_name,
                    Some(line),
                    Some(field),
                    observed.map(str::to_string),
                    None,
                    err,
                    "Set the field to a valid spec enum value.",
                ));
            }
            None
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::k1_parse::RawFile;
    use smol_str::SmolStr;

    fn make_file(headers: &[&str], rows: Vec<Vec<&str>>) -> RawFile {
        RawFile {
            name: "fare_attributes.txt".to_string(),
            headers: headers.iter().map(|s| (*s).to_string()).collect(),
            rows: rows
                .into_iter()
                .map(|row| row.into_iter().map(SmolStr::from).collect())
                .collect(),
            bytes: 0,
            raw_text: None,
            zip_entry_name: None,
        }
    }

    #[test]
    fn raw_field_matches_the_former_row_map() {
        // The same three rules as the old `RowMap` (build_row_map): None if the column is absent,
        // "" if the row is short, and the last column for a duplicate header. JPN_006 and TRN_016
        // rely on this contract.
        let file = make_file(
            &["fare_id", "price", "currency_type", "price"],
            vec![vec!["F1", "100", "JPY", "200"], vec!["F2", "300"]],
        );
        let (records, _) = validate_fare_attributes(&file);
        for (record, row) in records.iter().zip(&file.rows) {
            let map = build_row_map(&file.headers, row);
            for field in ["fare_id", "price", "currency_type", "transfers"] {
                assert_eq!(
                    record.raw_field(field),
                    map.get(field).map(String::as_str),
                    "{field}"
                );
            }
        }
        assert_eq!(records[0].raw_field("price"), Some("200"));
        assert_eq!(records[1].raw_field("currency_type"), Some(""));
        assert_eq!(records[0].raw_field("transfers"), None);
    }

    #[test]
    fn blank_price_produces_far_002() {
        let file = make_file(
            &["fare_id", "price", "currency_type", "payment_method"],
            vec![vec!["F1", "", "TRY", "0"]],
        );
        let (_, notices) = validate_fare_attributes(&file);
        assert!(notices.iter().any(|notice| notice.rule_id == "FAR_002"));
    }

    #[test]
    fn textual_price_produces_far_002() {
        let file = make_file(
            &["fare_id", "price", "currency_type", "payment_method"],
            vec![vec!["F1", "abc", "TRY", "0"]],
        );
        let (_, notices) = validate_fare_attributes(&file);
        assert!(notices.iter().any(|notice| notice.rule_id == "FAR_002"));
    }

    #[test]
    fn whitespace_flags_do_not_allocate_a_details_map_and_can_be_counted_early() {
        let file = make_file(
            &[
                "fare_id",
                "price",
                "currency_type",
                "payment_method",
                "transfers",
            ],
            vec![vec!["F1", " 1.00", "EUR", " 0", " 0"]],
        );

        let (_, notices) = validate_fare_attributes(&file);
        let derivatives: Vec<_> = notices
            .iter()
            .filter(|notice| matches!(notice.rule_id.as_str(), "FAR_002" | "FAR_004" | "FAR_005"))
            .collect();
        assert_eq!(derivatives.len(), 3);
        assert!(derivatives.iter().all(|notice| notice.whitespace_derived));
        assert!(derivatives.iter().all(|notice| notice.details.is_none()));

        let (_, notices, suppressions) = validate_fare_attributes_with_suppression(&file);
        assert!(notices
            .iter()
            .all(|notice| !matches!(notice.rule_id.as_str(), "FAR_002" | "FAR_004" | "FAR_005")));
        let (count, rules) = suppressions
            .audit_for("fare_attributes.txt")
            .expect("üç erken bastırma sayılmalı");
        assert_eq!(count, 3);
        assert_eq!(
            rules,
            ["FAR_002", "FAR_004", "FAR_005"]
                .into_iter()
                .map(str::to_string)
                .collect()
        );
    }

    #[test]
    fn negative_price_keeps_existing_far_002_behavior() {
        let file = make_file(
            &["fare_id", "price", "currency_type", "payment_method"],
            vec![vec!["F1", "-1", "TRY", "0"]],
        );
        let (_, notices) = validate_fare_attributes(&file);
        assert!(notices.iter().any(|notice| notice.rule_id == "FAR_002"));
    }

    #[test]
    fn ic_price_is_checked_in_fare_attributes() {
        let file = make_file(
            &[
                "fare_id",
                "price",
                "currency_type",
                "payment_method",
                "ic_price",
            ],
            vec![vec!["F1", "2.5", "JPY", "0", "-2"]],
        );
        let (_, notices) = validate_fare_attributes(&file);
        assert!(notices.iter().any(|notice| notice.rule_id == "GGL_002"));
        assert!(notices
            .iter()
            .any(|notice| notice.file.as_deref() == Some("fare_attributes.txt")));
    }

    #[test]
    fn valid_ic_price_values_are_silent() {
        let file = make_file(
            &[
                "fare_id",
                "price",
                "currency_type",
                "payment_method",
                "ic_price",
            ],
            vec![
                vec!["F1", "2.5", "JPY", "0", "-1"],
                vec!["F2", "2.5", "JPY", "0", "0"],
                vec!["F3", "2.5", "JPY", "0", "10.5"],
            ],
        );
        let (_, notices) = validate_fare_attributes(&file);
        assert!(!notices.iter().any(|notice| notice.rule_id == "GGL_002"));
    }
}
