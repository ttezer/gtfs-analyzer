use gtfs_core::EntityType;

use super::common::{build_row_map, get_trimmed_field, make_k2_notice, RowMap};
use crate::k1_parse::RawFile;

#[derive(Debug, Clone)]
pub struct BookingRuleRecord {
    pub booking_rule_id: String,
    pub booking_type: Option<u8>,
    pub prior_notice_duration_min: Option<i64>,
    pub prior_notice_duration_max: Option<i64>,
    pub prior_notice_last_day: Option<i64>,
    pub prior_notice_last_time: Option<String>,
    pub prior_notice_start_day: Option<i64>,
    pub prior_notice_start_time: Option<String>,
    pub prior_notice_service_id: Option<String>,
    pub row: RowMap,
    pub line: u64,
}

fn opt_str(row: &RowMap, field: &str) -> Option<String> {
    get_trimmed_field(row, field)
        .filter(|v| !v.is_empty())
        .map(str::to_string)
}

fn has_field(row: &RowMap, field: &str) -> bool {
    get_trimmed_field(row, field).map(|v| !v.is_empty()).unwrap_or(false)
}

fn opt_int(row: &RowMap, field: &str) -> Option<i64> {
    get_trimmed_field(row, field)
        .filter(|v| !v.is_empty())
        .and_then(|v| v.parse::<i64>().ok())
}

pub fn validate_booking_rules(file: &RawFile) -> (Vec<BookingRuleRecord>, Vec<gtfs_core::Notice>) {
    let mut notices = Vec::new();
    let mut records = Vec::new();
    let mut ctr = 0u32;

    for (row_idx, row) in file.rows.iter().enumerate() {
        let line = (row_idx + 2) as u64;
        let row_map = build_row_map(&file.headers, row);

        let id = get_trimmed_field(&row_map, "booking_rule_id").unwrap_or("").to_string();
        let entity_id = (!id.is_empty()).then_some(id.clone());

        let btype_str = get_trimmed_field(&row_map, "booking_type").unwrap_or("").to_string();
        let booking_type: Option<u8> = btype_str.parse::<u8>().ok().filter(|&v| v <= 2);

        let has_duration_min = has_field(&row_map, "prior_notice_duration_min");
        let has_duration_max = has_field(&row_map, "prior_notice_duration_max");
        let has_last_day    = has_field(&row_map, "prior_notice_last_day");
        let has_last_time   = has_field(&row_map, "prior_notice_last_time");
        let has_start_day   = has_field(&row_map, "prior_notice_start_day");
        let has_start_time  = has_field(&row_map, "prior_notice_start_time");

        let duration_min = opt_int(&row_map, "prior_notice_duration_min");
        let duration_max = opt_int(&row_map, "prior_notice_duration_max");
        let last_day     = opt_int(&row_map, "prior_notice_last_day");
        let start_day    = opt_int(&row_map, "prior_notice_start_day");

        if let Some(btype) = booking_type {
            // BKR_004: booking_type=0 iken prior_notice alanlarÄ± yasak
            if btype == 0
                && (has_duration_min || has_duration_max || has_last_day
                    || has_last_time || has_start_day || has_start_time)
            {
                notices.push(make_k2_notice(
                    &mut ctr, "BKR_004", EntityType::Row, entity_id.clone(), Some(&row_map),
                    &file.name, Some(line), Some("prior_notice_duration_min"),
                    Some(btype_str.clone()), Some("(boÅŸ)".to_string()),
                    "booking_type=0 (anlÄ±k rezervasyon) iken prior_notice alanlarÄ± dolu olmamalÄ±.".to_string(),
                    "AnlÄ±k rezervasyon iÃ§in prior_notice alanlarÄ±nÄ± kaldÄ±rÄ±n ya da booking_type deÄŸerini dÃ¼zeltin.",
                ));
            }

            // BKR_005: booking_type=2 iken prior_notice_duration_max yasak
            if btype == 2 && has_duration_max {
                notices.push(make_k2_notice(
                    &mut ctr, "BKR_005", EntityType::Row, entity_id.clone(), Some(&row_map),
                    &file.name, Some(line), Some("prior_notice_duration_max"),
                    get_trimmed_field(&row_map, "prior_notice_duration_max").map(str::to_string),
                    Some("(boÅŸ)".to_string()),
                    "booking_type=2 iken prior_notice_duration_max yasaktÄ±r.".to_string(),
                    "prior_notice_duration_max yalnÄ±zca booking_type=1 (aynÄ± gÃ¼n) ile kullanÄ±labilir.",
                ));
            }

            // BKR_007: booking_type=1 iken prior_notice_duration_min zorunlu
            if btype == 1 && !has_duration_min {
                notices.push(make_k2_notice(
                    &mut ctr, "BKR_007", EntityType::Row, entity_id.clone(), Some(&row_map),
                    &file.name, Some(line), Some("prior_notice_duration_min"),
                    None, Some("integer > 0".to_string()),
                    "booking_type=1 iken prior_notice_duration_min zorunludur.".to_string(),
                    "AynÄ± gÃ¼n rezervasyon iÃ§in minimum Ã¶nceden bildirim sÃ¼resini (dakika) girin.",
                ));
            }

            // BKR_001: booking_typeâ‰ 2 iken prior_notice_last_day/start_day yasak
            if btype != 2 && (has_last_day || has_start_day) {
                notices.push(make_k2_notice(
                    &mut ctr, "BKR_001", EntityType::Row, entity_id.clone(), Some(&row_map),
                    &file.name, Some(line), Some("prior_notice_last_day"),
                    Some(btype_str.clone()), Some("2".to_string()),
                    format!("booking_type={} iken prior_notice_last_day/start_day alanlarÄ± yasaktÄ±r.", btype),
                    "Ã–nceki gÃ¼n bazlÄ± alanlarÄ± yalnÄ±zca booking_type=2 ile kullanÄ±n.",
                ));
            }

            // BKR_008: booking_type=2 iken prior_notice_last_day zorunlu
            if btype == 2 && !has_last_day {
                notices.push(make_k2_notice(
                    &mut ctr, "BKR_008", EntityType::Row, entity_id.clone(), Some(&row_map),
                    &file.name, Some(line), Some("prior_notice_last_day"),
                    None, Some("integer â‰¥ 0".to_string()),
                    "booking_type=2 iken prior_notice_last_day zorunludur.".to_string(),
                    "Ã–nceki gÃ¼n rezervasyonu iÃ§in son bildirim gÃ¼nÃ¼nÃ¼ girin.",
                ));
            }

            // BKR_009: booking_type=2 iken prior_notice_last_time zorunlu
            if btype == 2 && !has_last_time {
                notices.push(make_k2_notice(
                    &mut ctr, "BKR_009", EntityType::Row, entity_id.clone(), Some(&row_map),
                    &file.name, Some(line), Some("prior_notice_last_time"),
                    None, Some("HH:MM:SS".to_string()),
                    "booking_type=2 iken prior_notice_last_time zorunludur.".to_string(),
                    "Son bildirim gÃ¼nÃ¼ iÃ§in saat bilgisini girin (HH:MM:SS).",
                ));
            }
        }

        // BKR_006: prior_notice_duration_min â‰¤ 0 veya sayÄ±sal deÄŸil
        if has_duration_min {
            match duration_min {
                Some(v) if v <= 0 => {
                    notices.push(make_k2_notice(
                        &mut ctr, "BKR_006", EntityType::Row, entity_id.clone(), Some(&row_map),
                        &file.name, Some(line), Some("prior_notice_duration_min"),
                        Some(v.to_string()), Some("> 0".to_string()),
                        "prior_notice_duration_min sÄ±fÄ±r veya negatif olamaz.".to_string(),
                        "Minimum bildirim sÃ¼resini pozitif bir dakika deÄŸeri olarak girin.",
                    ));
                }
                None => {
                    notices.push(make_k2_notice(
                        &mut ctr, "BKR_006", EntityType::Row, entity_id.clone(), Some(&row_map),
                        &file.name, Some(line), Some("prior_notice_duration_min"),
                        get_trimmed_field(&row_map, "prior_notice_duration_min").map(str::to_string),
                        Some("integer > 0".to_string()),
                        "prior_notice_duration_min sayÄ±sal bir deÄŸer deÄŸil.".to_string(),
                        "GeÃ§erli bir tam sayÄ± (dakika) girin.",
                    ));
                }
                _ => {}
            }
        }

        // BKR_002: prior_notice_start_day dolu ama prior_notice_last_day yok
        if has_start_day && !has_last_day {
            notices.push(make_k2_notice(
                &mut ctr, "BKR_002", EntityType::Row, entity_id.clone(), Some(&row_map),
                &file.name, Some(line), Some("prior_notice_start_day"),
                get_trimmed_field(&row_map, "prior_notice_start_day").map(str::to_string), None,
                "prior_notice_start_day yalnÄ±zca prior_notice_last_day ile birlikte kullanÄ±labilir.".to_string(),
                "prior_notice_last_day ekleyin ya da prior_notice_start_day kaldÄ±rÄ±n.",
            ));
        }

        // BKR_003: prior_notice_start_time dolu ama prior_notice_start_day yok
        if has_start_time && !has_start_day {
            notices.push(make_k2_notice(
                &mut ctr, "BKR_003", EntityType::Row, entity_id.clone(), Some(&row_map),
                &file.name, Some(line), Some("prior_notice_start_time"),
                get_trimmed_field(&row_map, "prior_notice_start_time").map(str::to_string), None,
                "prior_notice_start_time yalnÄ±zca prior_notice_start_day ile birlikte kullanÄ±labilir.".to_string(),
                "prior_notice_start_day ekleyin ya da prior_notice_start_time kaldÄ±rÄ±n.",
            ));
        }

        // BKR_010: prior_notice_start_day dolu ama prior_notice_start_time yok
        if has_start_day && !has_start_time {
            notices.push(make_k2_notice(
                &mut ctr, "BKR_010", EntityType::Row, entity_id.clone(), Some(&row_map),
                &file.name, Some(line), Some("prior_notice_start_time"),
                None, Some("HH:MM:SS".to_string()),
                "prior_notice_start_day belirtilmiÅŸse prior_notice_start_time zorunludur.".to_string(),
                "Erken rezervasyon penceresi baÅŸlangÄ±Ã§ saatini girin (HH:MM:SS).",
            ));
        }

        // BKR_011: prior_notice_last_day > prior_notice_start_day (rezervasyon penceresi geÃ§ersiz)
        if let (Some(ld), Some(sd)) = (last_day, start_day) {
            if ld > sd {
                notices.push(make_k2_notice(
                    &mut ctr, "BKR_011", EntityType::Row, entity_id.clone(), Some(&row_map),
                    &file.name, Some(line), Some("prior_notice_last_day"),
                    Some(format!("last_day={ld}, start_day={sd}")), None,
                    format!("prior_notice_last_day ({ld}) > prior_notice_start_day ({sd}): rezervasyon penceresi geÃ§ersiz."),
                    "prior_notice_start_day deÄŸerini prior_notice_last_day deÄŸerinden bÃ¼yÃ¼k ya da eÅŸit yapÄ±n.",
                ));
            }
        }

        records.push(BookingRuleRecord {
            booking_rule_id: id,
            booking_type,
            prior_notice_duration_min: duration_min,
            prior_notice_duration_max: duration_max,
            prior_notice_last_day: last_day,
            prior_notice_last_time: opt_str(&row_map, "prior_notice_last_time"),
            prior_notice_start_day: start_day,
            prior_notice_start_time: opt_str(&row_map, "prior_notice_start_time"),
            prior_notice_service_id: opt_str(&row_map, "prior_notice_service_id"),
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
        use smol_str::SmolStr;
        RawFile {
            name: "booking_rules.txt".to_string(),
            headers: headers.into_iter().map(str::to_string).collect(),
            rows: rows.into_iter().map(|r| r.into_iter().map(SmolStr::new).collect()).collect(),
            bytes: 0,
            raw_text: None,
        }
    }

    #[test]
    fn bkr_007_duration_min_missing_for_type1() {
        let file = make_file(
            vec!["booking_rule_id", "booking_type"],
            vec![vec!["BR1", "1"]],
        );
        let (_, notices) = validate_booking_rules(&file);
        assert!(notices.iter().any(|n| n.rule_id == "BKR_007"), "BKR_007 bekleniyor");
    }

    #[test]
    fn bkr_008_last_day_missing_for_type2() {
        let file = make_file(
            vec!["booking_rule_id", "booking_type", "prior_notice_last_time"],
            vec![vec!["BR1", "2", "12:00:00"]],
        );
        let (_, notices) = validate_booking_rules(&file);
        assert!(notices.iter().any(|n| n.rule_id == "BKR_008"), "BKR_008 bekleniyor");
    }

    #[test]
    fn bkr_009_last_time_missing_for_type2() {
        let file = make_file(
            vec!["booking_rule_id", "booking_type", "prior_notice_last_day"],
            vec![vec!["BR1", "2", "3"]],
        );
        let (_, notices) = validate_booking_rules(&file);
        assert!(notices.iter().any(|n| n.rule_id == "BKR_009"), "BKR_009 bekleniyor");
    }

    #[test]
    fn bkr_001_last_day_forbidden_for_type1() {
        let file = make_file(
            vec!["booking_rule_id", "booking_type", "prior_notice_duration_min", "prior_notice_last_day"],
            vec![vec!["BR1", "1", "30", "2"]],
        );
        let (_, notices) = validate_booking_rules(&file);
        assert!(notices.iter().any(|n| n.rule_id == "BKR_001"), "BKR_001 bekleniyor");
    }

    #[test]
    fn bkr_004_prior_notice_forbidden_for_type0() {
        let file = make_file(
            vec!["booking_rule_id", "booking_type", "prior_notice_duration_min"],
            vec![vec!["BR1", "0", "15"]],
        );
        let (_, notices) = validate_booking_rules(&file);
        assert!(notices.iter().any(|n| n.rule_id == "BKR_004"), "BKR_004 bekleniyor");
    }

    #[test]
    fn bkr_006_duration_min_zero() {
        let file = make_file(
            vec!["booking_rule_id", "booking_type", "prior_notice_duration_min"],
            vec![vec!["BR1", "1", "0"]],
        );
        let (_, notices) = validate_booking_rules(&file);
        assert!(notices.iter().any(|n| n.rule_id == "BKR_006"), "BKR_006 bekleniyor");
    }

    #[test]
    fn bkr_011_last_day_after_start_day() {
        let file = make_file(
            vec!["booking_rule_id", "booking_type", "prior_notice_last_day", "prior_notice_last_time", "prior_notice_start_day", "prior_notice_start_time"],
            vec![vec!["BR1", "2", "5", "12:00:00", "3", "09:00:00"]],
        );
        let (_, notices) = validate_booking_rules(&file);
        assert!(notices.iter().any(|n| n.rule_id == "BKR_011"), "BKR_011 bekleniyor");
    }

    #[test]
    fn valid_type2_booking_no_notices() {
        let file = make_file(
            vec!["booking_rule_id", "booking_type", "prior_notice_last_day", "prior_notice_last_time"],
            vec![vec!["BR1", "2", "3", "12:00:00"]],
        );
        let (recs, notices) = validate_booking_rules(&file);
        assert_eq!(recs.len(), 1);
        assert!(notices.is_empty(), "GeÃ§erli type=2 iÃ§in notice olmamalÄ±: {:?}", notices);
    }

    #[test]
    fn bkr_002_start_day_without_last_day() {
        let file = make_file(
            vec!["booking_rule_id", "booking_type", "prior_notice_last_time", "prior_notice_start_day", "prior_notice_start_time"],
            vec![vec!["BR1", "2", "12:00:00", "7", "09:00:00"]],
        );
        let (_, notices) = validate_booking_rules(&file);
        assert!(notices.iter().any(|n| n.rule_id == "BKR_002"), "BKR_002 bekleniyor");
    }

    #[test]
    fn bkr_010_start_day_without_start_time() {
        let file = make_file(
            vec!["booking_rule_id", "booking_type", "prior_notice_last_day", "prior_notice_last_time", "prior_notice_start_day"],
            vec![vec!["BR1", "2", "3", "12:00:00", "7"]],
        );
        let (_, notices) = validate_booking_rules(&file);
        assert!(notices.iter().any(|n| n.rule_id == "BKR_010"), "BKR_010 bekleniyor");
    }
}
