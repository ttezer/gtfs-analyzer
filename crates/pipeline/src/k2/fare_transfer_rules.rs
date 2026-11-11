use gtfs_core::EntityType;

use super::common::{
    build_row_map, get_raw_field, get_trimmed_field, make_k2_notice, parse_u32, validate_enum,
    RowMap,
};
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
                        notices.push(make_k2_notice(
                            &mut counter,
                            "FTR_001",
                            EntityType::Row,
                            entity_id.clone(),
                            Some(&row_map),
                            &file.name,
                            Some(line),
                            Some("fare_transfer_type"),
                            Some(v.to_string()),
                            Some("0–2".to_string()),
                            "fare_transfer_type geçerli bir enum değeri değil.".to_string(),
                            "Use 0, 1 or 2.",
                        ));
                    }
                } else if get_trimmed_field(&row_map, "fare_transfer_type") == Some("") {
                    notices.push(make_k2_notice(
                        &mut counter,
                        "FTR_001",
                        EntityType::Row,
                        entity_id.clone(),
                        Some(&row_map),
                        &file.name,
                        Some(line),
                        Some("fare_transfer_type"),
                        None,
                        Some("0–2".to_string()),
                        "fare_transfer_type zorunludur.".to_string(),
                        "Use 0, 1 or 2.",
                    ));
                }
                value
            }
            Err(err) => {
                notices.push(make_k2_notice(
                    &mut counter,
                    "FTR_001",
                    EntityType::Row,
                    entity_id.clone(),
                    Some(&row_map),
                    &file.name,
                    Some(line),
                    Some("fare_transfer_type"),
                    get_trimmed_field(&row_map, "fare_transfer_type").map(str::to_string),
                    Some("0–2".to_string()),
                    err,
                    "Enter a valid fare_transfer_type value.",
                ));
                None
            }
        };

        let duration_limit_type = match parse_u32(&row_map, "duration_limit_type") {
            Ok(value) => {
                if let Some(v) = value {
                    if !validate_enum(&v.to_string(), &["0", "1", "2", "3"]) {
                        notices.push(make_k2_notice(
                            &mut counter,
                            "FTR_005",
                            EntityType::Row,
                            entity_id.clone(),
                            Some(&row_map),
                            &file.name,
                            Some(line),
                            Some("duration_limit_type"),
                            Some(v.to_string()),
                            Some("0–3".to_string()),
                            "duration_limit_type geçerli bir enum değeri değil.".to_string(),
                            "Use 0, 1, 2 or 3.",
                        ));
                    }
                }
                value
            }
            Err(err) => {
                notices.push(make_k2_notice(
                    &mut counter,
                    "FTR_005",
                    EntityType::Row,
                    entity_id.clone(),
                    Some(&row_map),
                    &file.name,
                    Some(line),
                    Some("duration_limit_type"),
                    get_trimmed_field(&row_map, "duration_limit_type").map(str::to_string),
                    Some("0–3".to_string()),
                    err,
                    "Enter a valid duration_limit_type value.",
                ));
                None
            }
        };

        let duration_limit = match parse_u32(&row_map, "duration_limit") {
            Ok(value) => {
                if let Some(v) = value {
                    if v == 0 {
                        notices.push(make_k2_notice(
                            &mut counter,
                            "FTR_006",
                            EntityType::Row,
                            entity_id.clone(),
                            Some(&row_map),
                            &file.name,
                            Some(line),
                            Some("duration_limit"),
                            Some(v.to_string()),
                            Some("> 0".to_string()),
                            "duration_limit pozitif olmalıdır.".to_string(),
                            "Set duration_limit to a positive number of seconds.",
                        ));
                    }
                }
                value
            }
            Err(err) => {
                notices.push(make_k2_notice(
                    &mut counter,
                    "FTR_006",
                    EntityType::Row,
                    entity_id.clone(),
                    Some(&row_map),
                    &file.name,
                    Some(line),
                    Some("duration_limit"),
                    get_trimmed_field(&row_map, "duration_limit").map(str::to_string),
                    Some("> 0".to_string()),
                    err,
                    "Enter a positive number of seconds for duration_limit.",
                ));
                None
            }
        };

        // FTR_007: duration_limit_type is meaningless without duration_limit
        if duration_limit_type.is_some() && duration_limit.is_none() {
            notices.push(make_k2_notice(
                &mut counter,
                "FTR_007",
                EntityType::Row,
                entity_id.clone(),
                Some(&row_map),
                &file.name,
                Some(line),
                Some("duration_limit"),
                None,
                Some("dolu".to_string()),
                "duration_limit_type tanımlandığında duration_limit de belirtilmelidir."
                    .to_string(),
                "Fill in duration_limit or remove duration_limit_type.",
            ));
        }

        // transfer_count: -1 or a positive integer
        let transfer_count_raw = get_trimmed_field(&row_map, "transfer_count")
            .filter(|v| !v.trim().is_empty())
            .map(str::to_string);
        let transfer_count = if let Some(ref raw) = transfer_count_raw {
            match raw.parse::<i32>() {
                Ok(v) if v == -1 || v > 0 => Some(v),
                Ok(v) => {
                    notices.push(make_k2_notice(
                        &mut counter,
                        "FTR_008",
                        EntityType::Row,
                        entity_id.clone(),
                        Some(&row_map),
                        &file.name,
                        Some(line),
                        Some("transfer_count"),
                        Some(v.to_string()),
                        Some("-1 or > 0".to_string()),
                        "transfer_count -1 veya pozitif bir tam sayı olmalıdır.".to_string(),
                        "Enter -1 (unlimited) or a positive value such as 1.",
                    ));
                    None
                }
                Err(_) => {
                    notices.push(make_k2_notice(
                        &mut counter,
                        "FTR_008",
                        EntityType::Row,
                        entity_id.clone(),
                        Some(&row_map),
                        &file.name,
                        Some(line),
                        Some("transfer_count"),
                        Some(raw.clone()),
                        Some("-1 or > 0".to_string()),
                        "transfer_count sayısal bir değer değil.".to_string(),
                        "Enter -1 (unlimited) or a positive integer.",
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

        // FTR_009/010: transfer_count is required when leg groups are the SAME and forbidden when
        // they DIFFER (GTFS spec + MD: from_leg_group_id == to_leg_group_id → required; != → forbidden).
        // If one is empty and the other is populated, they are not equal: both spec and MD say forbidden
        // (2026-10-13; previously skipped). If BOTH are empty, skip it: the spec's literal reading
        // is "" = "" (required), while MD forbids it (`hasFromLegGroupId` condition) — ambiguous;
        // both sides stay silent to avoid rejecting a valid global transfer row (mdb-3115).
        // Presence is checked on the raw field (transfer_count_raw); invalid values go to FTR_008.
        if from_leg.is_some() || to_leg.is_some() {
            let (f, t) = (from_leg.as_deref(), to_leg.as_deref());
            if f == t && transfer_count_raw.is_none() {
                notices.push(make_k2_notice(
                    &mut counter, "FTR_009", EntityType::Row, entity_id.clone(), Some(&row_map),
                    &file.name, Some(line), Some("transfer_count"), None, Some("dolu".to_string()),
                    "from_leg_group_id ile to_leg_group_id aynı olduğunda transfer_count zorunludur.".to_string(),
                    "Enter transfer_count (-1 unlimited or a positive value).",
                ));
            } else if f != t && transfer_count_raw.is_some() {
                notices.push(make_k2_notice(
                    &mut counter, "FTR_010", EntityType::Row, entity_id.clone(), Some(&row_map),
                    &file.name, Some(line), Some("transfer_count"),
                    transfer_count_raw.clone(), Some("(boş)".to_string()),
                    "from_leg_group_id ile to_leg_group_id farklı olduğunda transfer_count tanımlanamaz.".to_string(),
                    "Remove transfer_count or make the leg groups equal.",
                ));
            }
        }

        // FTR_011: duration_limit_type is required when duration_limit is defined (the reverse of FTR_007).
        if duration_limit.is_some() && duration_limit_type.is_none() {
            notices.push(make_k2_notice(
                &mut counter,
                "FTR_011",
                EntityType::Row,
                entity_id.clone(),
                Some(&row_map),
                &file.name,
                Some(line),
                Some("duration_limit_type"),
                None,
                Some("dolu".to_string()),
                "duration_limit tanımlandığında duration_limit_type de belirtilmelidir."
                    .to_string(),
                "Fill duration_limit_type (0–3) or remove duration_limit.",
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
            rows: rows
                .into_iter()
                .map(|r| r.into_iter().map(smol_str::SmolStr::from).collect())
                .collect(),
            bytes: 0,
            raw_text: None,
            zip_entry_name: None,
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
        assert!(
            has(&n, "FTR_009"),
            "from==to + transfer_count yok → FTR_009: {n:?}"
        );
        assert!(!has(&n, "FTR_010"));
    }

    #[test]
    fn transfer_count_forbidden_when_leg_groups_differ_ftr_010() {
        let f = make_file(
            vec![
                "from_leg_group_id",
                "to_leg_group_id",
                "transfer_count",
                "fare_transfer_type",
            ],
            vec![vec!["L1", "L2", "1", "0"]],
        );
        let (_, n) = validate_fare_transfer_rules(&f);
        assert!(
            has(&n, "FTR_010"),
            "from!=to + transfer_count dolu → FTR_010: {n:?}"
        );
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

    /// An empty leg group is not equal to a populated one: both spec and MD forbid it. When both
    /// groups are empty the rule is ambiguous (spec "" = "", MD forbids it), so both rules stay silent (mdb-3115).
    #[test]
    fn transfer_count_with_one_empty_leg_group_is_ftr_010() {
        let header = vec!["from_leg_group_id", "to_leg_group_id", "transfer_count"];
        for row in [vec!["", "L1", "2"], vec!["L1", "", "2"]] {
            let f = make_file(header.clone(), vec![row.clone()]);
            let (_, n) = validate_fare_transfer_rules(&f);
            assert!(has(&n, "FTR_010"), "{row:?} → FTR_010: {n:?}");
            assert!(!has(&n, "FTR_009"));
        }
        for row in [vec!["", "", "2"], vec!["", "", ""]] {
            let f = make_file(header.clone(), vec![row.clone()]);
            let (_, n) = validate_fare_transfer_rules(&f);
            assert!(!has(&n, "FTR_009") && !has(&n, "FTR_010"), "{row:?}: {n:?}");
        }
    }

    #[test]
    fn duration_limit_without_type_ftr_011() {
        let f = make_file(
            vec![
                "from_leg_group_id",
                "to_leg_group_id",
                "transfer_count",
                "duration_limit",
            ],
            vec![vec!["L1", "L1", "1", "3600"]],
        );
        let (_, n) = validate_fare_transfer_rules(&f);
        assert!(
            has(&n, "FTR_011"),
            "duration_limit dolu, type yok → FTR_011: {n:?}"
        );

        let f2 = make_file(
            vec![
                "from_leg_group_id",
                "to_leg_group_id",
                "transfer_count",
                "duration_limit",
                "duration_limit_type",
            ],
            vec![vec!["L1", "L1", "1", "3600", "1"]],
        );
        let (_, n2) = validate_fare_transfer_rules(&f2);
        assert!(!has(&n2, "FTR_011"));
    }
}
