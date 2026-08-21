use gtfs_core::EntityType;

use super::common::{get_raw_field, build_row_map, get_trimmed_field, make_k2_notice, parse_u32, validate_enum, RowMap};
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
        let from_leg = get_raw_field(&row_map, "from_leg_group_id")
            .filter(|v| !v.trim().is_empty())
            .map(str::to_string);
        let entity_id = from_leg.clone();

        let fare_transfer_type = match parse_u32(&row_map, "fare_transfer_type") {
            Ok(value) => {
                if let Some(v) = value {
                    if !validate_enum(&v.to_string(), &["0", "1", "2"]) {
                        notices.push( make_k2_notice(
                            &mut counter, "FTR_001", EntityType::Row, entity_id.clone(), Some(&row_map),
                            &file.name, Some(line), Some("fare_transfer_type"), Some(v.to_string()),
                            Some("0–2".to_string()),
                            "fare_transfer_type geçerli bir enum değeri değil.".to_string(),
                            "0, 1 veya 2 kullanın.",
                        ));
                    }
                } else if get_trimmed_field(&row_map, "fare_transfer_type") == Some("") {
                    notices.push( make_k2_notice(
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
                notices.push( make_k2_notice(
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
                        notices.push( make_k2_notice(
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
                notices.push( make_k2_notice(
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
                        notices.push( make_k2_notice(
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
                notices.push( make_k2_notice(
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
            notices.push( make_k2_notice(
                &mut counter, "FTR_007", EntityType::Row, entity_id.clone(), Some(&row_map),
                &file.name, Some(line), Some("duration_limit"), None,
                Some("dolu".to_string()),
                "duration_limit_type tanımlandığında duration_limit de belirtilmelidir.".to_string(),
                "duration_limit alanını doldurun ya da duration_limit_type'ı kaldırın.",
            ));
        }

        // transfer_count: -1 veya pozitif tam sayı
        let transfer_count_raw = get_trimmed_field(&row_map, "transfer_count")
            .filter(|v| !v.trim().is_empty())
            .map(str::to_string);
        let transfer_count = if let Some(ref raw) = transfer_count_raw {
            match raw.parse::<i32>() {
                Ok(v) if v == -1 || v > 0 => Some(v),
                Ok(v) => {
                    notices.push( make_k2_notice(
                        &mut counter, "FTR_008", EntityType::Row, entity_id.clone(), Some(&row_map),
                        &file.name, Some(line), Some("transfer_count"), Some(v.to_string()),
                        Some("-1 veya > 0".to_string()),
                        "transfer_count -1 veya pozitif bir tam sayı olmalıdır.".to_string(),
                        "-1 (sınırsız) veya 1 gibi pozitif bir değer girin.",
                    ));
                    None
                }
                Err(_) => {
                    notices.push( make_k2_notice(
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

        let to_leg = get_raw_field(&row_map, "to_leg_group_id")
            .filter(|v| !v.trim().is_empty())
            .map(str::to_string);

        // FTR_009/010: transfer_count, leg grupları AYNIYKEN zorunlu, FARKLIYKEN yasaktır
        // (GTFS spec + MD: from_leg_group_id == to_leg_group_id → gerekli; != → yasak).
        // Yalnız iki leg grubu da tanımlıyken uygulanır (global/area kuralında atlanır → FP önlemi).
        // Varlık kontrolü ham alan üzerinden (transfer_count_raw); geçersiz değer FTR_008'e bırakılır.
        if let (Some(f), Some(t)) = (from_leg.as_deref(), to_leg.as_deref()) {
            if f == t && transfer_count_raw.is_none() {
                notices.push( make_k2_notice(
                    &mut counter, "FTR_009", EntityType::Row, entity_id.clone(), Some(&row_map),
                    &file.name, Some(line), Some("transfer_count"), None, Some("dolu".to_string()),
                    "from_leg_group_id ile to_leg_group_id aynı olduğunda transfer_count zorunludur.".to_string(),
                    "transfer_count girin (-1 = sınırsız veya pozitif bir değer).",
                ));
            } else if f != t && transfer_count_raw.is_some() {
                notices.push( make_k2_notice(
                    &mut counter, "FTR_010", EntityType::Row, entity_id.clone(), Some(&row_map),
                    &file.name, Some(line), Some("transfer_count"),
                    transfer_count_raw.clone(), Some("(boş)".to_string()),
                    "from_leg_group_id ile to_leg_group_id farklı olduğunda transfer_count tanımlanamaz.".to_string(),
                    "transfer_count alanını kaldırın ya da leg gruplarını eşitleyin.",
                ));
            }
        }

        // FTR_011: duration_limit tanımlıyken duration_limit_type zorunludur (FTR_007'nin ters yönü).
        if duration_limit.is_some() && duration_limit_type.is_none() {
            notices.push( make_k2_notice(
                &mut counter, "FTR_011", EntityType::Row, entity_id.clone(), Some(&row_map),
                &file.name, Some(line), Some("duration_limit_type"), None, Some("dolu".to_string()),
                "duration_limit tanımlandığında duration_limit_type de belirtilmelidir.".to_string(),
                "duration_limit_type alanını (0–3) doldurun ya da duration_limit'i kaldırın.",
            ));
        }

        records.push(FareTransferRuleRecord {
            from_leg_group_id: from_leg,
            to_leg_group_id: to_leg,
            transfer_count,
            duration_limit,
            duration_limit_type,
            fare_transfer_type,
            fare_product_id: get_raw_field(&row_map, "fare_product_id")
                .filter(|v| !v.trim().is_empty())
                .map(str::to_string),
            row: row_map,
            line,
        });
    }

    (records, notices)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_file(headers: Vec<&str>, rows: Vec<Vec<&str>>) -> RawFile {
        RawFile {
            name: "fare_transfer_rules.txt".to_string(),
            headers: headers.into_iter().map(str::to_string).collect(),
            rows: rows.into_iter().map(|r| r.into_iter().map(smol_str::SmolStr::from).collect()).collect(),
            bytes: 0,
            raw_text: None,
        }
    }

    fn has(notices: &[gtfs_core::Notice], id: &str) -> bool {
        notices.iter().any(|n| n.rule_id == id)
    }

    #[test]
    fn transfer_count_required_when_leg_groups_equal_ftr_009() {
        let f = make_file(
            vec!["from_leg_group_id", "to_leg_group_id", "fare_transfer_type"],
            vec![vec!["L1", "L1", "0"]],
        );
        let (_, n) = validate_fare_transfer_rules(&f);
        assert!(has(&n, "FTR_009"), "from==to + transfer_count yok → FTR_009: {n:?}");
        assert!(!has(&n, "FTR_010"));
    }

    #[test]
    fn transfer_count_forbidden_when_leg_groups_differ_ftr_010() {
        let f = make_file(
            vec!["from_leg_group_id", "to_leg_group_id", "transfer_count", "fare_transfer_type"],
            vec![vec!["L1", "L2", "1", "0"]],
        );
        let (_, n) = validate_fare_transfer_rules(&f);
        assert!(has(&n, "FTR_010"), "from!=to + transfer_count dolu → FTR_010: {n:?}");
        assert!(!has(&n, "FTR_009"));
    }

    #[test]
    fn transfer_count_ok_cases_no_ftr_009_010() {
        let f1 = make_file(
            vec!["from_leg_group_id", "to_leg_group_id", "transfer_count"],
            vec![vec!["L1", "L1", "2"]],
        );
        let (_, n1) = validate_fare_transfer_rules(&f1);
        assert!(!has(&n1, "FTR_009") && !has(&n1, "FTR_010"));

        let f2 = make_file(
            vec!["from_leg_group_id", "to_leg_group_id"],
            vec![vec!["L1", "L2"]],
        );
        let (_, n2) = validate_fare_transfer_rules(&f2);
        assert!(!has(&n2, "FTR_009") && !has(&n2, "FTR_010"));
    }

    #[test]
    fn duration_limit_without_type_ftr_011() {
        let f = make_file(
            vec!["from_leg_group_id", "to_leg_group_id", "transfer_count", "duration_limit"],
            vec![vec!["L1", "L1", "1", "3600"]],
        );
        let (_, n) = validate_fare_transfer_rules(&f);
        assert!(has(&n, "FTR_011"), "duration_limit dolu, type yok → FTR_011: {n:?}");

        let f2 = make_file(
            vec!["from_leg_group_id", "to_leg_group_id", "transfer_count", "duration_limit", "duration_limit_type"],
            vec![vec!["L1", "L1", "1", "3600", "1"]],
        );
        let (_, n2) = validate_fare_transfer_rules(&f2);
        assert!(!has(&n2, "FTR_011"));
    }
}
