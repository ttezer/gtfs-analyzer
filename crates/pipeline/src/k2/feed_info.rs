use gtfs_core::EntityType;

use super::common::{
    build_row_map, get_trimmed_field, looks_like_bcp47, looks_like_email, looks_like_url,
    make_k2_notice, parse_service_date, RowMap,
};
use crate::k1_parse::RawFile;

#[derive(Debug, Clone)]
pub struct FeedInfoRecord {
    pub feed_publisher_name: String,
    pub feed_publisher_url: String,
    pub feed_lang: String,
    pub feed_start_date: Option<(u32, u32, u32)>,
    pub feed_end_date: Option<(u32, u32, u32)>,
    pub feed_version: Option<String>,
    pub feed_contact_email: Option<String>,
    pub feed_contact_url: Option<String>,
    pub row: RowMap,
    pub line: u64,
}

pub fn validate_feed_info(file: &RawFile) -> (Vec<FeedInfoRecord>, Vec<gtfs_core::Notice>) {
    let mut notices = Vec::new();
    let mut records = Vec::new();
    let mut counter = 0;

    for (row_idx, row) in file.rows.iter().enumerate() {
        let line = (row_idx + 2) as u64;
        let row_map = build_row_map(&file.headers, row);

        let publisher_name = get_trimmed_field(&row_map, "feed_publisher_name").unwrap_or("").to_string();
        if publisher_name.is_empty() {
            notices.push(make_k2_notice(
                &mut counter,
                "FIN_001",
                EntityType::Feed,
                None,
                Some(&row_map),
                &file.name,
                Some(line),
                Some("feed_publisher_name"),
                Some(String::new()),
                None,
                "feed_publisher_name zorunludur.".to_string(),
                "feed_publisher_name alanını doldurun.",
            ));
        }

        let publisher_url = get_trimmed_field(&row_map, "feed_publisher_url").unwrap_or("").to_string();
        if publisher_url.is_empty() || !looks_like_url(&publisher_url) {
            notices.push(make_k2_notice(
                &mut counter,
                "FIN_002",
                EntityType::Feed,
                None,
                Some(&row_map),
                &file.name,
                Some(line),
                Some("feed_publisher_url"),
                Some(publisher_url.clone()),
                None,
                "feed_publisher_url eksik veya geçersiz.".to_string(),
                "feed_publisher_url için geçerli bir http/https URL'si girin.",
            ));
        }

        let feed_lang = get_trimmed_field(&row_map, "feed_lang").unwrap_or("").to_string();
        if feed_lang.is_empty() || !looks_like_bcp47(&feed_lang) {
            notices.push(make_k2_notice(
                &mut counter,
                "FIN_003",
                EntityType::Feed,
                None,
                Some(&row_map),
                &file.name,
                Some(line),
                Some("feed_lang"),
                Some(feed_lang.clone()),
                None,
                "feed_lang eksik veya geçersiz.".to_string(),
                "feed_lang için geçerli bir IETF BCP 47 dil etiketi girin.",
            ));
        }

        let default_lang = get_trimmed_field(&row_map, "default_lang")
            .filter(|v| !v.is_empty())
            .map(str::to_string);
        if let Some(ref lang) = default_lang {
            if !looks_like_bcp47(lang) {
                notices.push(make_k2_notice(
                    &mut counter,
                    "FIN_004",
                    EntityType::Feed,
                    None,
                    Some(&row_map),
                    &file.name,
                    Some(line),
                    Some("default_lang"),
                    Some(lang.clone()),
                    None,
                    "default_lang geçerli bir BCP 47 dil etiketi değil.".to_string(),
                    "default_lang için geçerli bir IETF BCP 47 dil etiketi girin.",
                ));
            }
        }

        let feed_start_date = parse_date_field(&row_map, &mut notices, &mut counter, "FIN_005", "feed_start_date", line, &file.name);
        let feed_end_date = parse_date_field(&row_map, &mut notices, &mut counter, "FIN_006", "feed_end_date", line, &file.name);

        // FIN_014: feed_start_date veya feed_end_date eksik (missing_feed_info_date)
        let raw_start = get_trimmed_field(&row_map, "feed_start_date").unwrap_or("").to_string();
        let raw_end   = get_trimmed_field(&row_map, "feed_end_date").unwrap_or("").to_string();
        if raw_start.is_empty() || raw_end.is_empty() {
            notices.push(make_k2_notice(
                &mut counter, "FIN_014", EntityType::Feed, None,
                Some(&row_map), &file.name, Some(line), None,
                None, None,
                "feed_start_date ve/veya feed_end_date eksik; feed geçerlilik aralığı belirlenemiyor.".to_string(),
                "feed_start_date ve feed_end_date alanlarını YYYYMMDD formatında doldurun.",
            ));
        }
        if let (Some(start), Some(end)) = (feed_start_date, feed_end_date) {
            if end < start {
                notices.push(make_k2_notice(
                    &mut counter,
                    "FIN_012",
                    EntityType::Feed,
                    None,
                    Some(&row_map),
                    &file.name,
                    Some(line),
                    Some("feed_start_date"),
                    Some(format!("{}-{:02}-{:02}", start.0, start.1, start.2)),
                    Some(format!("≤ {}-{:02}-{:02}", end.0, end.1, end.2)),
                    format!("feed_start_date ({}-{:02}-{:02}) feed_end_date ({}-{:02}-{:02})'den sonra.",
                        start.0, start.1, start.2, end.0, end.1, end.2),
                    "feed_start_date'i feed_end_date'den önce veya eşit olarak ayarlayın.",
                ));
            }
        }

        let feed_version = get_trimmed_field(&row_map, "feed_version")
            .filter(|v| !v.is_empty())
            .map(str::to_string);
        if feed_version.is_none() {
            notices.push(make_k2_notice(
                &mut counter,
                "FIN_007",
                EntityType::Feed,
                None,
                Some(&row_map),
                &file.name,
                Some(line),
                Some("feed_version"),
                Some(String::new()),
                None,
                "feed_version önerilen ancak eksik.".to_string(),
                "Sürüm takibi için feed_version alanını doldurun.",
            ));
        }

        let feed_contact_email = get_trimmed_field(&row_map, "feed_contact_email")
            .filter(|v| !v.is_empty())
            .map(str::to_string);
        if let Some(email) = feed_contact_email.as_deref() {
            if !looks_like_email(email) {
                notices.push(make_k2_notice(
                    &mut counter,
                    "FIN_008",
                    EntityType::Feed,
                    None,
                    Some(&row_map),
                    &file.name,
                    Some(line),
                    Some("feed_contact_email"),
                    Some(email.to_string()),
                    None,
                    "feed_contact_email geçerli bir e-posta adresi değil.".to_string(),
                    "feed_contact_email için geçerli bir e-posta adresi kullanın.",
                ));
            }
        }

        let feed_contact_url = get_trimmed_field(&row_map, "feed_contact_url")
            .filter(|v| !v.is_empty())
            .map(str::to_string);
        if let Some(url) = feed_contact_url.as_deref() {
            if !looks_like_url(url) {
                notices.push(make_k2_notice(
                    &mut counter,
                    "FIN_009",
                    EntityType::Feed,
                    None,
                    Some(&row_map),
                    &file.name,
                    Some(line),
                    Some("feed_contact_url"),
                    Some(url.to_string()),
                    None,
                    "feed_contact_url geçerli bir URL değil.".to_string(),
                    "feed_contact_url için geçerli bir http/https URL'si kullanın.",
                ));
            }
        }

        records.push(FeedInfoRecord {
            feed_publisher_name: publisher_name,
            feed_publisher_url: publisher_url,
            feed_lang,
            feed_start_date,
            feed_end_date,
            feed_version,
            feed_contact_email,
            feed_contact_url,
            row: row_map,
            line,
        });
    }

    // FIN_015: Birden fazla feed_info kaydı (more_than_one_entity)
    if records.len() > 1 {
        notices.push(make_k2_notice(
            &mut counter, "FIN_015", EntityType::Feed, None,
            None, &file.name, None, None,
            Some(records.len().to_string()), Some("1".to_string()),
            format!("feed_info.txt'de {} kayıt var; yalnızca 1 satır olmalıdır.", records.len()),
            "feed_info.txt'de yalnızca bir kayıt bırakın.",
        ));
    }

    (records, notices)
}

fn parse_date_field(
    row_map: &RowMap,
    notices: &mut Vec<gtfs_core::Notice>,
    counter: &mut u32,
    rule_id: &str,
    field: &str,
    line: u64,
    file_name: &str,
) -> Option<(u32, u32, u32)> {
    match parse_service_date(row_map, field) {
        Ok(value) => value,
        Err(err) => {
            notices.push(make_k2_notice(
                counter,
                rule_id,
                EntityType::Feed,
                None,
                Some(row_map),
                file_name,
                Some(line),
                Some(field),
                get_trimmed_field(row_map, field).map(str::to_string),
                Some("YYYYMMDD".to_string()),
                err,
                "Alanı geçerli bir GTFS servis tarihi formatına (YYYYMMDD) ayarlayın.",
            ));
            None
        }
    }
}
