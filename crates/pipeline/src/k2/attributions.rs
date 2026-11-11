use gtfs_core::EntityType;

use super::common::{
    build_row_map, get_lexical_field, get_raw_field, get_trimmed_field, looks_like_email,
    looks_like_url, make_k2_notice, parse_u32, validate_enum, RowMap,
};
use crate::k1_parse::RawFile;

#[derive(Debug, Clone)]
pub struct AttributionRecord {
    pub attribution_id: Option<String>,
    pub organization_name: String,
    pub is_producer: Option<u32>,
    pub is_operator: Option<u32>,
    pub is_authority: Option<u32>,
    pub agency_id: Option<String>,
    pub route_id: Option<String>,
    pub trip_id: Option<String>,
    pub attribution_url: Option<String>,
    pub attribution_email: Option<String>,
    pub row: RowMap,
    pub line: u64,
}

pub fn validate_attributions(file: &RawFile) -> (Vec<AttributionRecord>, Vec<gtfs_core::Notice>) {
    let mut notices = Vec::new();
    let mut records = Vec::new();
    let mut counter = 0;
    // ATR_001/ATR_003: one summary per FILE (the FAR_013/DQ_016 pattern). Both violations arise
    // from ONE systemic cause — `attribution_id` is not written at all, or the role columns
    // are never populated — and emitting per row repeats the same sentence.
    // Measurement (124 feeds carrying `attributions.txt`): per-row emission meant 391,871 notices
    // for ATR_001 and 6,472 for ATR_003; one feed has 97,261 rows.
    // The summary carries the count in `observed_value`, so the number of affected rows is not lost.
    let mut missing_id_count: u64 = 0;
    let mut missing_id_first: Option<u64> = None;
    let mut no_role_count: u64 = 0;
    let mut no_role_first: Option<u64> = None;

    for (row_idx, row) in file.rows.iter().enumerate() {
        let line = (row_idx + 2) as u64;
        let row_map = build_row_map(&file.headers, row);
        let attribution_id = get_raw_field(&row_map, "attribution_id")
            .filter(|v| !v.trim().is_empty())
            .map(str::to_string);

        // ATR_001: attribution_id is missing (recommended) — one summary at the end of the file
        if attribution_id.is_none() {
            missing_id_count += 1;
            if missing_id_first.is_none() {
                missing_id_first = Some(line);
            }
        }

        let organization_name = get_trimmed_field(&row_map, "organization_name")
            .unwrap_or("")
            .to_string();
        // if the column is absent from the header, ARC_025 takes over → skip
        if get_trimmed_field(&row_map, "organization_name") == Some("") {
            notices.push(make_k2_notice(
                &mut counter,
                "ATR_002",
                EntityType::Attribution,
                attribution_id.clone(),
                Some(&row_map),
                &file.name,
                Some(line),
                Some("organization_name"),
                Some(String::new()),
                None,
                "organization_name zorunludur.".to_string(),
                "Fill in the organization_name field.",
            ));
        }

        let is_producer = parse_role_enum(
            &row_map,
            &mut notices,
            &mut counter,
            "ATR_004",
            "is_producer",
            &attribution_id,
            line,
            &file.name,
        );
        let is_operator = parse_role_enum(
            &row_map,
            &mut notices,
            &mut counter,
            "ATR_005",
            "is_operator",
            &attribution_id,
            line,
            &file.name,
        );
        let is_authority = parse_role_enum(
            &row_map,
            &mut notices,
            &mut counter,
            "ATR_006",
            "is_authority",
            &attribution_id,
            line,
            &file.name,
        );

        if !matches!(is_producer, Some(1))
            && !matches!(is_operator, Some(1))
            && !matches!(is_authority, Some(1))
        {
            no_role_count += 1;
            if no_role_first.is_none() {
                no_role_first = Some(line);
            }
        }

        let attribution_url = get_lexical_field(&row_map, "attribution_url")
            .filter(|v| !v.trim().is_empty())
            .map(str::to_string);
        if let Some(url) = attribution_url.as_deref() {
            if !looks_like_url(url) {
                notices.push(make_k2_notice(
                    &mut counter,
                    "ATR_007",
                    EntityType::Attribution,
                    attribution_id.clone(),
                    Some(&row_map),
                    &file.name,
                    Some(line),
                    Some("attribution_url"),
                    Some(url.to_string()),
                    None,
                    "attribution_url geçerli bir URL değil.".to_string(),
                    "Use a valid http/https URL for attribution_url.",
                ));
            }
        }

        let attribution_email = get_lexical_field(&row_map, "attribution_email")
            .filter(|v| !v.trim().is_empty())
            .map(str::to_string);
        if let Some(email) = attribution_email.as_deref() {
            if !looks_like_email(email) {
                notices.push(make_k2_notice(
                    &mut counter,
                    "ATR_008",
                    EntityType::Attribution,
                    attribution_id.clone(),
                    Some(&row_map),
                    &file.name,
                    Some(line),
                    Some("attribution_email"),
                    Some(email.to_string()),
                    None,
                    "attribution_email geçerli bir e-posta adresi değil.".to_string(),
                    "Use a valid e-mail address for attribution_email.",
                ));
            }
        }

        let agency_id = get_raw_field(&row_map, "agency_id")
            .filter(|v| !v.trim().is_empty())
            .map(str::to_string);
        let route_id = get_raw_field(&row_map, "route_id")
            .filter(|v| !v.trim().is_empty())
            .map(str::to_string);
        let trip_id = get_raw_field(&row_map, "trip_id")
            .filter(|v| !v.trim().is_empty())
            .map(str::to_string);

        records.push(AttributionRecord {
            attribution_id,
            organization_name,
            is_producer,
            is_operator,
            is_authority,
            agency_id,
            route_id,
            trip_id,
            attribution_url,
            attribution_email,
            row: row_map,
            line,
        });
    }

    if let Some(first) = missing_id_first {
        notices.push(make_k2_notice(
            &mut counter,
            "ATR_001",
            EntityType::File,
            None,
            None,
            &file.name,
            Some(first),
            Some("attribution_id"),
            Some(format!("{missing_id_count} rows")),
            None,
            format!(
                "attributions.txt dosyasındaki {missing_id_count} kayıtta attribution_id yok                  (ör. satır {first}); bu kayıtlar tekrarda ayrılamaz."
            ),
            "Assign a unique attribution_id to each attribution record.",
        ));
    }

    if let Some(first) = no_role_first {
        notices.push(make_k2_notice(
            &mut counter,
            "ATR_003",
            EntityType::File,
            None,
            None,
            &file.name,
            Some(first),
            // The rule is that all three role fields are empty TOGETHER; all three are named.
            Some("is_producer|is_operator|is_authority"),
            Some(format!("{no_role_count} rows")),
            Some("en az bir rol = 1".to_string()),
            format!(
                "attributions.txt dosyasındaki {no_role_count} kayıtta hiçbir attribution rolü                  tanımlı değil (ör. satır {first})."
            ),
            "Set at least one of is_producer, is_operator or is_authority to 1.",
        ));
    }

    (records, notices)
}

// The field, row, and notice context are parsed together; a one-use parameter
// struct would not simplify the call surface here, only hide the context.
#[allow(clippy::too_many_arguments)]
fn parse_role_enum(
    row_map: &RowMap,
    notices: &mut Vec<gtfs_core::Notice>,
    counter: &mut u32,
    rule_id: &str,
    field: &str,
    entity_id: &Option<String>,
    line: u64,
    file_name: &str,
) -> Option<u32> {
    match parse_u32(row_map, field) {
        Ok(value) => {
            if let Some(v) = value {
                if !validate_enum(&v.to_string(), &["0", "1"]) {
                    notices.push(make_k2_notice(
                        counter,
                        rule_id,
                        EntityType::Attribution,
                        entity_id.clone(),
                        Some(row_map),
                        file_name,
                        Some(line),
                        Some(field),
                        Some(v.to_string()),
                        Some("0 or 1".to_string()),
                        format!("{field} alanı 0 veya 1 olmalıdır."),
                        "Set each attribution role field to 0 or 1.",
                    ));
                }
            }
            value
        }
        Err(err) => {
            notices.push(make_k2_notice(
                counter,
                rule_id,
                EntityType::Attribution,
                entity_id.clone(),
                Some(row_map),
                file_name,
                Some(line),
                Some(field),
                get_trimmed_field(row_map, field).map(str::to_string),
                Some("0 or 1".to_string()),
                err,
                "Set each attribution role field to 0 or 1.",
            ));
            None
        }
    }
}
