use gtfs_core::EntityType;

use super::common::{
    build_row_map, get_trimmed_field, looks_like_bcp47, looks_like_email,
    looks_like_iana_timezone, looks_like_phone, looks_like_url, make_k2_notice, parse_u32, RowMap,
};
use crate::k1_parse::RawFile;

#[derive(Debug, Clone)]
pub struct AgencyRecord {
    pub agency_id: Option<String>,
    pub agency_name: String,
    pub agency_url: String,
    pub agency_timezone: String,
    pub agency_lang: Option<String>,
    pub agency_phone: Option<String>,
    pub agency_fare_url: Option<String>,
    pub agency_email: Option<String>,
    pub agency_cemv_support: Option<u32>,
    pub row: RowMap,
    pub line: u64,
}

pub fn validate_agency(file: &RawFile) -> (Vec<AgencyRecord>, Vec<gtfs_core::Notice>) {
    let mut notices = Vec::new();
    let mut records = Vec::new();
    let mut counter = 0u32;

    for (row_idx, row) in file.rows.iter().enumerate() {
        let line = (row_idx + 2) as u64;
        let row_map = build_row_map(&file.headers, row);

        let agency_id = get_trimmed_field(&row_map, "agency_id")
            .filter(|v| !v.is_empty())
            .map(str::to_string);
        let entity_id = agency_id.clone();

        // AGN_002: agency_name required
        let agency_name = get_trimmed_field(&row_map, "agency_name").unwrap_or("").to_string();
        if agency_name.is_empty() {
            notices.push(make_k2_notice(
                &mut counter, "AGN_002", EntityType::Agency, entity_id.clone(), Some(&row_map),
                &file.name, Some(line), Some("agency_name"), Some(String::new()), None,
                "agency_name zorunludur.".to_string(), "agency_name alanını doldurun.",
            ));
        }

        // AGN_003: agency_url required + valid URL
        let agency_url = get_trimmed_field(&row_map, "agency_url").unwrap_or("").to_string();
        if agency_url.is_empty() {
            notices.push(make_k2_notice(
                &mut counter, "AGN_003", EntityType::Agency, entity_id.clone(), Some(&row_map),
                &file.name, Some(line), Some("agency_url"), Some(String::new()), None,
                "agency_url zorunludur.".to_string(), "Geçerli bir http/https URL'si girin.",
            ));
        } else if !looks_like_url(&agency_url) {
            notices.push(make_k2_notice(
                &mut counter, "AGN_003", EntityType::Agency, entity_id.clone(), Some(&row_map),
                &file.name, Some(line), Some("agency_url"), Some(agency_url.clone()), None,
                "agency_url geçerli bir URL değil.".to_string(), "Geçerli bir http/https URL'si kullanın.",
            ));
        }

        // AGN_004: agency_timezone required + valid IANA
        let agency_timezone = get_trimmed_field(&row_map, "agency_timezone").unwrap_or("").to_string();
        if agency_timezone.is_empty() {
            notices.push(make_k2_notice(
                &mut counter, "AGN_004", EntityType::Agency, entity_id.clone(), Some(&row_map),
                &file.name, Some(line), Some("agency_timezone"), Some(String::new()), None,
                "agency_timezone zorunludur.".to_string(), "Geçerli bir IANA saat dilimi girin.",
            ));
        } else if !looks_like_iana_timezone(&agency_timezone) {
            notices.push(make_k2_notice(
                &mut counter, "AGN_004", EntityType::Agency, entity_id.clone(), Some(&row_map),
                &file.name, Some(line), Some("agency_timezone"), Some(agency_timezone.clone()), None,
                format!("agency_timezone '{agency_timezone}' geçerli bir IANA saat dilimi değil."),
                "Geçerli bir IANA saat dilimi kullanın (örn. Europe/Istanbul).",
            ));
        }

        // AGN_006: agency_lang must be valid BCP-47
        let agency_lang = get_trimmed_field(&row_map, "agency_lang")
            .filter(|v| !v.is_empty())
            .map(str::to_string);
        if let Some(ref lang) = agency_lang {
            if !looks_like_bcp47(lang) {
                notices.push(make_k2_notice(
                    &mut counter, "AGN_006", EntityType::Agency, entity_id.clone(), Some(&row_map),
                    &file.name, Some(line), Some("agency_lang"), Some(lang.clone()), None,
                    format!("agency_lang '{lang}' geçerli bir BCP-47 dil kodu değil."),
                    "Geçerli bir BCP-47 dil kodu kullanın (örn. tr, en-US).",
                ));
            }
        }

        // AGN_007: agency_phone format
        let agency_phone = get_trimmed_field(&row_map, "agency_phone")
            .filter(|v| !v.is_empty())
            .map(str::to_string);
        if let Some(ref phone) = agency_phone {
            if !looks_like_phone(phone) {
                notices.push(make_k2_notice(
                    &mut counter, "AGN_007", EntityType::Agency, entity_id.clone(), Some(&row_map),
                    &file.name, Some(line), Some("agency_phone"), Some(phone.clone()), None,
                    format!("agency_phone '{phone}' geçerli bir telefon numarası formatında değil."),
                    "En az 7 haneli bir telefon numarası kullanın.",
                ));
            }
        }

        // AGN_008: agency_fare_url valid URL if provided
        let agency_fare_url = get_trimmed_field(&row_map, "agency_fare_url")
            .filter(|v| !v.is_empty())
            .map(str::to_string);
        if let Some(ref url) = agency_fare_url {
            if !looks_like_url(url) {
                notices.push(make_k2_notice(
                    &mut counter, "AGN_008", EntityType::Agency, entity_id.clone(), Some(&row_map),
                    &file.name, Some(line), Some("agency_fare_url"), Some(url.clone()), None,
                    "agency_fare_url geçerli bir URL değil.".to_string(),
                    "agency_fare_url için geçerli bir http/https URL'si kullanın.",
                ));
            }
        }

        // AGN_009: agency_email valid email if provided
        let agency_email = get_trimmed_field(&row_map, "agency_email")
            .filter(|v| !v.is_empty())
            .map(str::to_string);
        if let Some(ref email) = agency_email {
            if !looks_like_email(email) {
                notices.push(make_k2_notice(
                    &mut counter, "AGN_009", EntityType::Agency, entity_id.clone(), Some(&row_map),
                    &file.name, Some(line), Some("agency_email"), Some(email.clone()), None,
                    "agency_email geçerli bir e-posta adresi değil.".to_string(),
                    "agency_email için geçerli bir e-posta adresi kullanın.",
                ));
            }
        }

        // AGN_012: agency_cemv_support must be 0 or 1
        let agency_cemv_support = match parse_u32(&row_map, "agency_cemv_support") {
            Ok(v) => {
                if let Some(val) = v {
                    if val > 1 {
                        notices.push(make_k2_notice(
                            &mut counter, "AGN_012", EntityType::Agency, entity_id.clone(), Some(&row_map),
                            &file.name, Some(line), Some("agency_cemv_support"), Some(val.to_string()),
                            Some("0 veya 1".to_string()),
                            "agency_cemv_support 0 veya 1 olmalıdır.".to_string(),
                            "agency_cemv_support alanını 0 veya 1 olarak ayarlayın.",
                        ));
                    }
                }
                v
            }
            Err(_) => None,
        };

        records.push(AgencyRecord {
            agency_id,
            agency_name,
            agency_url,
            agency_timezone,
            agency_lang,
            agency_phone,
            agency_fare_url,
            agency_email,
            agency_cemv_support,
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

    fn make_file(headers: Vec<&str>, rows: Vec<Vec<&str>>) -> RawFile {
        RawFile {
            name: "agency.txt".to_string(),
            headers: headers.into_iter().map(str::to_string).collect(),
            rows: rows.into_iter().map(|r| r.into_iter().map(smol_str::SmolStr::from).collect()).collect(),
            bytes: 0,
            raw_text: None,
        }
    }

    #[test]
    fn valid_agency_produces_no_notices() {
        let file = make_file(
            vec!["agency_id", "agency_name", "agency_url", "agency_timezone"],
            vec![vec!["A1", "Transit Co", "https://transit.example", "Europe/Istanbul"]],
        );
        let (records, notices) = validate_agency(&file);
        assert_eq!(records.len(), 1);
        assert!(notices.is_empty(), "Geçerli ajans notice üretmemeli: {:?}", notices);
    }

    #[test]
    fn missing_agency_name_produces_agn_002() {
        let file = make_file(
            vec!["agency_id", "agency_name", "agency_url", "agency_timezone"],
            vec![vec!["A1", "", "https://transit.example", "Europe/Istanbul"]],
        );
        let (_, notices) = validate_agency(&file);
        assert!(notices.iter().any(|n| n.rule_id == "AGN_002"));
    }

    #[test]
    fn missing_agency_url_produces_agn_003() {
        let file = make_file(
            vec!["agency_id", "agency_name", "agency_url", "agency_timezone"],
            vec![vec!["A1", "Transit Co", "", "Europe/Istanbul"]],
        );
        let (_, notices) = validate_agency(&file);
        assert!(notices.iter().any(|n| n.rule_id == "AGN_003"));
    }

    #[test]
    fn invalid_timezone_produces_agn_004() {
        let file = make_file(
            vec!["agency_id", "agency_name", "agency_url", "agency_timezone"],
            vec![vec!["A1", "Transit Co", "https://transit.example", "InvalidTZ"]],
        );
        let (_, notices) = validate_agency(&file);
        assert!(notices.iter().any(|n| n.rule_id == "AGN_004"));
    }

    #[test]
    fn invalid_lang_produces_agn_006() {
        let file = make_file(
            vec!["agency_id", "agency_name", "agency_url", "agency_timezone", "agency_lang"],
            vec![vec!["A1", "TC", "https://tc.example", "Europe/Istanbul", "not valid lang!!"]],
        );
        let (_, notices) = validate_agency(&file);
        assert!(notices.iter().any(|n| n.rule_id == "AGN_006"));
    }

    #[test]
    fn invalid_email_produces_agn_009() {
        let file = make_file(
            vec!["agency_id", "agency_name", "agency_url", "agency_timezone", "agency_email"],
            vec![vec!["A1", "TC", "https://tc.example", "Europe/Istanbul", "notanemail"]],
        );
        let (_, notices) = validate_agency(&file);
        assert!(notices.iter().any(|n| n.rule_id == "AGN_009"));
    }
}
