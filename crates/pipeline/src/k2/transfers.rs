use gtfs_core::EntityType;

use super::common::{
    build_row_map, get_raw_field, get_trimmed_field, make_k2_notice, parse_u32, validate_enum,
    RowMap,
};
use crate::k1_parse::RawFile;

#[derive(Debug, Clone)]
pub struct TransferRecord {
    pub from_stop_id: String,
    pub to_stop_id: String,
    pub transfer_type: Option<u32>,
    pub min_transfer_time: Option<u32>,
    pub from_trip_id: Option<String>,
    pub to_trip_id: Option<String>,
    pub from_route_id: Option<String>,
    pub to_route_id: Option<String>,
    pub row: RowMap,
    pub line: u64,
}

/// Planner context on the row checks (TRF_001/002/004/005): the stops, trips, routes, type
/// and time written on the transfer row — the same keys K4 adds to its transfer findings.
/// Only `details` keys are added.
fn add_transfer_row_context(findings: &mut [gtfs_core::Notice], row: &RowMap) {
    // GGL_001 is raised in this loop too; another group's finding is left untouched.
    for finding in findings
        .iter_mut()
        .filter(|n| n.rule_id.starts_with("TRF_"))
    {
        let details = finding.details.get_or_insert_with(Default::default);
        for key in [
            "from_stop_id",
            "to_stop_id",
            "from_trip_id",
            "to_trip_id",
            "from_route_id",
            "to_route_id",
            "transfer_type",
            "min_transfer_time",
        ] {
            if let Some(value) = get_trimmed_field(row, key).filter(|v| !v.is_empty()) {
                details.insert(key.to_string(), value.to_string());
            }
        }
    }
}

pub fn validate_transfers(file: &RawFile) -> (Vec<TransferRecord>, Vec<gtfs_core::Notice>) {
    let mut notices = Vec::new();
    let mut records = Vec::new();
    let mut counter = 0;

    for (row_idx, row) in file.rows.iter().enumerate() {
        let line = (row_idx + 2) as u64;
        let row_map = build_row_map(&file.headers, row);
        let first_notice = notices.len();
        let from_stop_id = get_raw_field(&row_map, "from_stop_id")
            .unwrap_or("")
            .to_string();
        let to_stop_id = get_raw_field(&row_map, "to_stop_id")
            .unwrap_or("")
            .to_string();
        let entity_id = (!from_stop_id.is_empty() && !to_stop_id.is_empty())
            .then_some(format!("{from_stop_id}|{to_stop_id}"));

        // TRF_004: transfer_type is invalid (parsed first because TRF_001/002 depend on it)
        let transfer_type = match parse_u32(&row_map, "transfer_type") {
            Ok(value) => {
                if let Some(v) = value {
                    if !validate_enum(&v.to_string(), &["0", "1", "2", "3", "4", "5"]) {
                        notices.push(make_k2_notice(
                            &mut counter,
                            "TRF_004",
                            EntityType::Row,
                            entity_id.clone(),
                            Some(&row_map),
                            &file.name,
                            Some(line),
                            Some("transfer_type"),
                            Some(v.to_string()),
                            Some("0–5".to_string()),
                            format!("transfer_type '{v}' geçersiz."),
                            "Set transfer_type to a valid GTFS enum value (0–5).",
                        ));
                    }
                }
                value
            }
            Err(err) => {
                notices.push(make_k2_notice(
                    &mut counter,
                    "TRF_004",
                    EntityType::Row,
                    entity_id.clone(),
                    Some(&row_map),
                    &file.name,
                    Some(line),
                    Some("transfer_type"),
                    get_trimmed_field(&row_map, "transfer_type").map(str::to_string),
                    Some("0–5".to_string()),
                    err,
                    "Set transfer_type to a valid GTFS enum value (0–5).",
                ));
                None
            }
        };

        // from_stop_id/to_stop_id are conditionally required: required for transfer_type 0/1/2/3,
        // optional for 4/5 (in-seat / scheduled connection) — those use from_trip_id/to_trip_id.
        // If the column is absent, ARC_025 takes over → skip (get_trimmed_field == Some("") means
        // the column exists but its value is empty). Missing/invalid transfer_type defaults to 0 → required.
        let stop_ids_required = !matches!(transfer_type, Some(4) | Some(5));

        // TRF_001: from_stop_id is required
        if stop_ids_required && get_raw_field(&row_map, "from_stop_id").map(str::trim) == Some("") {
            notices.push(make_k2_notice(
                &mut counter,
                "TRF_001",
                EntityType::Row,
                entity_id.clone(),
                Some(&row_map),
                &file.name,
                Some(line),
                Some("from_stop_id"),
                Some(String::new()),
                None,
                "from_stop_id zorunludur.".to_string(),
                "Add from_stop_id to the transfer record.",
            ));
        }

        // TRF_002: to_stop_id is required
        if stop_ids_required && get_raw_field(&row_map, "to_stop_id").map(str::trim) == Some("") {
            notices.push(make_k2_notice(
                &mut counter,
                "TRF_002",
                EntityType::Row,
                entity_id.clone(),
                Some(&row_map),
                &file.name,
                Some(line),
                Some("to_stop_id"),
                Some(String::new()),
                None,
                "to_stop_id zorunludur.".to_string(),
                "Add to_stop_id to the transfer record.",
            ));
        }

        // TRF_005: min_transfer_time is required for transfer_type=2
        let min_transfer_time = match parse_u32(&row_map, "min_transfer_time") {
            Ok(value) => value,
            Err(err) => {
                notices.push(make_k2_notice(
                    &mut counter,
                    "TRF_005",
                    EntityType::Row,
                    entity_id.clone(),
                    Some(&row_map),
                    &file.name,
                    Some(line),
                    Some("min_transfer_time"),
                    get_trimmed_field(&row_map, "min_transfer_time").map(str::to_string),
                    Some(">= 0".to_string()),
                    err,
                    "Enter zero or a positive integer for min_transfer_time.",
                ));
                None
            }
        };

        if matches!(transfer_type, Some(2)) && min_transfer_time.is_none() {
            notices.push(make_k2_notice(
                &mut counter,
                "TRF_005",
                EntityType::Row,
                entity_id.clone(),
                Some(&row_map),
                &file.name,
                Some(line),
                Some("min_transfer_time"),
                Some(String::new()),
                None,
                "transfer_type=2 için min_transfer_time zorunludur.".to_string(),
                "Fill in min_transfer_time.",
            ));
        }

        // GGL_001: transfer_type=4/5 is not supported by Google Transit
        if matches!(transfer_type, Some(4) | Some(5)) {
            let ttype = transfer_type.unwrap();
            notices.push(make_k2_notice(
                &mut counter, "GGL_001", EntityType::Row, entity_id.clone(), Some(&row_map),
                &file.name, Some(line), Some("transfer_type"),
                Some(ttype.to_string()), None,
                format!("transfer_type={ttype} (in-seat aktarma) Google Transit tarafından yoksayılıyor."),
                "Google Transit uyumluluğu gerekiyorsa transfer_type değerini 0–3 arasında seçin.",
            ));
        }

        add_transfer_row_context(&mut notices[first_notice..], &row_map);
        records.push(TransferRecord {
            from_stop_id,
            to_stop_id,
            transfer_type,
            min_transfer_time,
            from_trip_id: get_raw_field(&row_map, "from_trip_id")
                .filter(|v| !v.trim().is_empty())
                .map(str::to_string),
            to_trip_id: get_raw_field(&row_map, "to_trip_id")
                .filter(|v| !v.trim().is_empty())
                .map(str::to_string),
            from_route_id: get_raw_field(&row_map, "from_route_id")
                .filter(|v| !v.trim().is_empty())
                .map(str::to_string),
            to_route_id: get_raw_field(&row_map, "to_route_id")
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
    use crate::k1_parse::RawFile;
    use smol_str::SmolStr;

    fn make_file(rows: Vec<Vec<&str>>) -> RawFile {
        RawFile {
            name: "transfers.txt".into(),
            headers: vec![
                "from_stop_id".into(),
                "to_stop_id".into(),
                "transfer_type".into(),
            ],
            rows: rows
                .iter()
                .map(|r| r.iter().map(|v| SmolStr::new(*v)).collect())
                .collect(),
            bytes: 0,
            raw_text: None,
            zip_entry_name: None,
        }
    }

    #[test]
    fn ggl_001_fires_for_in_seat_transfer_type_4() {
        let file = make_file(vec![vec!["S1", "S2", "4"]]);
        let (_, notices) = validate_transfers(&file);
        assert!(
            notices.iter().any(|n| n.rule_id == "GGL_001"),
            "GGL_001 bekleniyor"
        );
    }

    #[test]
    fn ggl_001_fires_for_in_seat_transfer_type_5() {
        let file = make_file(vec![vec!["S1", "S2", "5"]]);
        let (_, notices) = validate_transfers(&file);
        assert!(
            notices.iter().any(|n| n.rule_id == "GGL_001"),
            "GGL_001 bekleniyor"
        );
    }

    #[test]
    fn ggl_001_silent_for_standard_transfer_types() {
        let file = make_file(vec![vec!["S1", "S2", "0"], vec!["S3", "S4", "1"]]);
        let (_, notices) = validate_transfers(&file);
        assert!(
            !notices.iter().any(|n| n.rule_id == "GGL_001"),
            "GGL_001 tetiklenmemeli"
        );
    }

    #[test]
    fn trf_row_checks_carry_the_rows_values_but_not_ggl_001() {
        let file = make_file(vec![vec!["", "S2", "5"]]);
        let (_, notices) = validate_transfers(&file);
        let ggl = notices
            .iter()
            .find(|n| n.rule_id == "GGL_001")
            .expect("GGL_001");
        assert!(ggl
            .details
            .as_ref()
            .is_none_or(|d| !d.contains_key("to_stop_id")));
        let file = make_file(vec![vec!["", "S2", "0"]]);
        let (_, notices) = validate_transfers(&file);
        let d = notices
            .iter()
            .find(|n| n.rule_id == "TRF_001")
            .and_then(|n| n.details.as_ref())
            .expect("TRF_001 details");
        assert_eq!(d.get("to_stop_id").map(String::as_str), Some("S2"));
        assert_eq!(d.get("transfer_type").map(String::as_str), Some("0"));
    }

    #[test]
    fn trf_001_002_skipped_for_transfer_type_4() {
        // from/to_stop_id is optional for transfer_type=4 (in-seat) → no TRF_001/002 even when empty
        let file = make_file(vec![vec!["", "", "4"]]);
        let (_, notices) = validate_transfers(&file);
        let ids: Vec<&str> = notices.iter().map(|n| n.rule_id.as_str()).collect();
        assert!(
            !ids.contains(&"TRF_001"),
            "transfer_type=4'te from_stop_id zorunlu olmamalı: {:?}",
            ids
        );
        assert!(
            !ids.contains(&"TRF_002"),
            "transfer_type=4'te to_stop_id zorunlu olmamalı: {:?}",
            ids
        );
    }

    #[test]
    fn trf_001_fires_for_transfer_type_1_missing_stop() {
        // from/to_stop_id is required for transfer_type=1 → TRF_001/002 when empty
        let file = make_file(vec![vec!["", "", "1"]]);
        let (_, notices) = validate_transfers(&file);
        let ids: Vec<&str> = notices.iter().map(|n| n.rule_id.as_str()).collect();
        assert!(
            ids.contains(&"TRF_001"),
            "transfer_type=1'de from_stop_id eksik → TRF_001: {:?}",
            ids
        );
        assert!(
            ids.contains(&"TRF_002"),
            "transfer_type=1'de to_stop_id eksik → TRF_002: {:?}",
            ids
        );
    }

    /// `transfer_type=2` + `min_transfer_time` MISSING — the conditionally-required half of the rule.
    ///
    /// Google Transit PR #640 explicitly made this field **Conditionally Required** for
    /// `transfer_type=2`. The predicate already measured it, but this branch had NO test;
    /// only the parse/negative branch was pinned down. The branch carrying the spec's new
    /// normative basis therefore had no regression protection.
    #[test]
    fn transfer_type_2_without_min_transfer_time_produces_trf_005() {
        let file = make_file(vec![vec!["S1", "S2", "2"]]);
        let (_, notices) = validate_transfers(&file);
        assert!(
            notices.iter().any(|n| n.rule_id == "TRF_005"),
            "transfer_type=2 min_transfer_time olmadan TRF_005 üretmeli"
        );
    }

    /// The converse: when the value IS present, the rule must stay silent — otherwise a feed
    /// satisfying the requirement would be rejected, the COSTLY side of this asymmetry.
    #[test]
    fn transfer_type_2_with_min_transfer_time_is_silent() {
        let mut file = make_file(vec![vec!["S1", "S2", "2"]]);
        file.headers.push("min_transfer_time".into());
        file.rows[0].push("180".into());
        let (_, notices) = validate_transfers(&file);
        assert!(
            !notices.iter().any(|n| n.rule_id == "TRF_005"),
            "geçerli min_transfer_time TRF_005 ÜRETMEMELİ"
        );
    }

    /// The rule applies ONLY to `transfer_type=2`. For other types, the field's absence
    /// is outside this rule; expanding it would reject valid feeds.
    #[test]
    fn other_transfer_types_do_not_require_min_transfer_time() {
        for ttype in ["0", "1", "3"] {
            let file = make_file(vec![vec!["S1", "S2", ttype]]);
            let (_, notices) = validate_transfers(&file);
            assert!(
                !notices.iter().any(|n| n.rule_id == "TRF_005"),
                "transfer_type={ttype} için min_transfer_time zorunlu DEĞİL"
            );
        }
    }

    #[test]
    fn negative_min_transfer_time_is_rejected_by_trf_005() {
        let mut file = make_file(vec![vec!["S1", "S2", "2"]]);
        file.headers.push("min_transfer_time".into());
        file.rows[0].push("-1".into());
        let (_, notices) = validate_transfers(&file);
        assert!(notices.iter().any(|n| n.rule_id == "TRF_005"));
    }
}
