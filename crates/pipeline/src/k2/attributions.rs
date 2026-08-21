use gtfs_core::EntityType;

use super::common::{get_lexical_field, get_raw_field,
    build_row_map, get_trimmed_field, looks_like_email, looks_like_url, make_k2_notice, parse_u32,
    validate_enum, RowMap,
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

pub fn validate_attributions(
    file: &RawFile,
) -> (Vec<AttributionRecord>, Vec<gtfs_core::Notice>) {
    let mut notices = Vec::new();
    let mut records = Vec::new();
    let mut counter = 0;

    for (row_idx, row) in file.rows.iter().enumerate() {
        let line = (row_idx + 2) as u64;
        let row_map = build_row_map(&file.headers, row);
        let attribution_id = get_raw_field(&row_map, "attribution_id")
            .filter(|v| !v.trim().is_empty())
            .map(str::to_string);

        // ATR_001: attribution_id eksik (tavsiye edilen)
        if attribution_id.is_none() {
            crate::notice_budget::push(&mut notices, make_k2_notice(
                &mut counter, "ATR_001", EntityType::Attribution, None,
                Some(&row_map), &file.name, Some(line), Some("attribution_id"),
                Some(String::new()), None,
                "attribution_id belirtilmemiş; kayıt tekrarında ayrım yapılamaz.".to_string(),
                "Her attribution kaydına benzersiz bir attribution_id atayın.",
            ));
        }

        let organization_name = get_trimmed_field(&row_map, "organization_name").unwrap_or("").to_string();
        // sütun başlıkta yoksa ARC_025 devralır → atla
        if get_trimmed_field(&row_map, "organization_name") == Some("") {
            crate::notice_budget::push(&mut notices, make_k2_notice(
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
                "organization_name alanını doldurun.",
            ));
        }

        let is_producer = parse_role_enum(&row_map, &mut notices, &mut counter, "ATR_004", "is_producer", &attribution_id, line, &file.name);
        let is_operator = parse_role_enum(&row_map, &mut notices, &mut counter, "ATR_005", "is_operator", &attribution_id, line, &file.name);
        let is_authority = parse_role_enum(&row_map, &mut notices, &mut counter, "ATR_006", "is_authority", &attribution_id, line, &file.name);

        if !matches!(is_producer, Some(1)) && !matches!(is_operator, Some(1)) && !matches!(is_authority, Some(1)) {
            crate::notice_budget::push(&mut notices, make_k2_notice(
                &mut counter,
                "ATR_003",
                EntityType::Attribution,
                attribution_id.clone(),
                Some(&row_map),
                &file.name,
                Some(line),
                // Hüküm üç rol alanının BİRLİKTE boş olmasıdır; üçü de adlandırılır.
                Some("is_producer|is_operator|is_authority"),
                None,
                Some("en az bir rol = 1".to_string()),
                "En az bir attribution rolü 1 olarak ayarlanmalıdır.".to_string(),
                "is_producer, is_operator veya is_authority değerinden en az birini 1 olarak ayarlayın.",
            ));
        }

        let attribution_url = get_lexical_field(&row_map, "attribution_url")
            .filter(|v| !v.trim().is_empty())
            .map(str::to_string);
        if let Some(url) = attribution_url.as_deref() {
            if !looks_like_url(url) {
                crate::notice_budget::push(&mut notices, make_k2_notice(
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
                    "attribution_url için geçerli bir http/https URL'si kullanın.",
                ));
            }
        }

        let attribution_email = get_lexical_field(&row_map, "attribution_email")
            .filter(|v| !v.trim().is_empty())
            .map(str::to_string);
        if let Some(email) = attribution_email.as_deref() {
            if !looks_like_email(email) {
                crate::notice_budget::push(&mut notices, make_k2_notice(
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
                    "attribution_email için geçerli bir e-posta adresi kullanın.",
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

    (records, notices)
}

// Alan, satır ve notice bağlamı birlikte parse edilir; tek kullanımlık bir parametre
// struct'ı burada çağrı yüzeyini sadeleştirmeyecek, yalnızca bağlamı gizleyecektir.
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
                    crate::notice_budget::push(notices, make_k2_notice(
                        counter,
                        rule_id,
                        EntityType::Attribution,
                        entity_id.clone(),
                        Some(row_map),
                        file_name,
                        Some(line),
                        Some(field),
                        Some(v.to_string()),
                        Some("0 veya 1".to_string()),
                        format!("{field} alanı 0 veya 1 olmalıdır."),
                        "Her attribution rol alanını 0 veya 1 olarak ayarlayın.",
                    ));
                }
            }
            value
        }
        Err(err) => {
            crate::notice_budget::push(notices, make_k2_notice(
                counter,
                rule_id,
                EntityType::Attribution,
                entity_id.clone(),
                Some(row_map),
                file_name,
                Some(line),
                Some(field),
                get_trimmed_field(row_map, field).map(str::to_string),
                Some("0 veya 1".to_string()),
                err,
                "Her attribution rol alanını 0 veya 1 olarak ayarlayın.",
            ));
            None
        }
    }
}
