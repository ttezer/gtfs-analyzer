use gtfs_core::EntityType;

use super::common::{
    build_row_map, get_trimmed_field, make_k2_notice, parse_service_date, parse_u32,
    validate_enum, RowMap,
};
use crate::k1_parse::RawFile;

#[derive(Debug, Clone)]
pub struct CalendarDateRecord {
    pub service_id: String,
    pub date: Option<(u32, u32, u32)>,
    pub exception_type: Option<u32>,
    pub row: RowMap,
    pub line: u64,
}

pub fn validate_calendar_dates(file: &RawFile) -> (Vec<CalendarDateRecord>, Vec<gtfs_core::Notice>) {
    let mut notices = Vec::new();
    let mut records = Vec::new();
    let mut counter = 0u32;

    for (row_idx, row) in file.rows.iter().enumerate() {
        let line = (row_idx + 2) as u64;
        let row_map = build_row_map(&file.headers, row);

        let service_id = get_trimmed_field(&row_map, "service_id").unwrap_or("").to_string();
        let entity_id = (!service_id.is_empty()).then_some(service_id.clone());

        // CLD_001: service_id required
        if service_id.is_empty() {
            notices.push(make_k2_notice(
                &mut counter, "CLD_001", EntityType::Service, None,
                Some(&row_map), &file.name, Some(line), Some("service_id"),
                Some(String::new()), None,
                "service_id zorunludur.".to_string(),
                "service_id alanını doldurun.",
            ));
        }

        // CLD_002: date required + valid YYYYMMDD
        let date = match parse_service_date(&row_map, "date") {
            Ok(v) => {
                if v.is_none() {
                    notices.push(make_k2_notice(
                        &mut counter, "CLD_002", EntityType::Service, entity_id.clone(),
                        Some(&row_map), &file.name, Some(line), Some("date"),
                        Some(String::new()), Some("YYYYMMDD".to_string()),
                        "date zorunludur.".to_string(),
                        "YYYYMMDD formatında tarihi girin.",
                    ));
                }
                // CLD_005: tarih makul yıl aralığı dışında (2000–2099)
                if let Some((year, month, day)) = v {
                    if year < 2000 || year > 2099 {
                        notices.push(make_k2_notice(
                            &mut counter, "CLD_005", EntityType::Service, entity_id.clone(),
                            Some(&row_map), &file.name, Some(line), Some("date"),
                            Some(format!("{year}-{month:02}-{day:02}")), Some("2000–2099".to_string()),
                            format!("service_id '{}' için tarih {year}-{month:02}-{day:02} makul yıl aralığı dışında.",
                                service_id),
                            "GTFS tarihleri 2000–2099 yılları arasında olmalıdır.",
                        ));
                    }
                }
                v
            }
            Err(err) => {
                notices.push(make_k2_notice(
                    &mut counter, "CLD_002", EntityType::Service, entity_id.clone(),
                    Some(&row_map), &file.name, Some(line), Some("date"),
                    get_trimmed_field(&row_map, "date").map(str::to_string),
                    Some("YYYYMMDD".to_string()), err,
                    "YYYYMMDD formatında tarihi girin.",
                ));
                None
            }
        };

        // CLD_003: exception_type required + valid enum (1 or 2)
        let exception_type = match parse_u32(&row_map, "exception_type") {
            Ok(v) => {
                match v {
                    None => {
                        notices.push(make_k2_notice(
                            &mut counter, "CLD_003", EntityType::Service, entity_id.clone(),
                            Some(&row_map), &file.name, Some(line), Some("exception_type"),
                            Some(String::new()), Some("1 veya 2".to_string()),
                            "exception_type zorunludur.".to_string(),
                            "exception_type değerini 1 (eklendi) veya 2 (kaldırıldı) olarak ayarlayın.",
                        ));
                        None
                    }
                    Some(val) => {
                        if !validate_enum(&val.to_string(), &["1", "2"]) {
                            notices.push(make_k2_notice(
                                &mut counter, "CLD_003", EntityType::Service, entity_id.clone(),
                                Some(&row_map), &file.name, Some(line), Some("exception_type"),
                                Some(val.to_string()), Some("1 veya 2".to_string()),
                                "exception_type 1 veya 2 olmalıdır.".to_string(),
                                "exception_type değerini 1 (hizmet eklendi) veya 2 (hizmet kaldırıldı) olarak ayarlayın.",
                            ));
                        }
                        Some(val)
                    }
                }
            }
            Err(err) => {
                notices.push(make_k2_notice(
                    &mut counter, "CLD_003", EntityType::Service, entity_id.clone(),
                    Some(&row_map), &file.name, Some(line), Some("exception_type"),
                    get_trimmed_field(&row_map, "exception_type").map(str::to_string),
                    Some("1 veya 2".to_string()), err,
                    "exception_type değerini 1 (hizmet eklendi) veya 2 (hizmet kaldırıldı) olarak ayarlayın.",
                ));
                None
            }
        };

        records.push(CalendarDateRecord {
            service_id,
            date,
            exception_type,
            row: row_map,
            line,
        });
    }

    // CLD_006: bir service_id için çok fazla istisna günü (> 60)
    let mut exception_counts: std::collections::HashMap<&str, usize> = std::collections::HashMap::new();
    for rec in &records {
        if !rec.service_id.is_empty() {
            *exception_counts.entry(rec.service_id.as_str()).or_insert(0) += 1;
        }
    }
    for (sid, count) in &exception_counts {
        if *count > 60 {
            notices.push(make_k2_notice(
                &mut counter,
                "CLD_006",
                EntityType::Service,
                Some(sid.to_string()),
                None,
                &file.name,
                None,
                Some("service_id"),
                Some(count.to_string()),
                Some("≤ 60".to_string()),
                format!("'{sid}' servisinin {count} istisna günü var; bu çok fazla olabilir."),
                "Çok sayıda istisna yerine calendar.txt'te normal servis periyodu tanımlayın.",
            ));
        }
    }

    (records, notices)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::k1_parse::RawFile;

    fn make_file(headers: Vec<&str>, rows: Vec<Vec<&str>>) -> RawFile {
        RawFile {
            name: "calendar_dates.txt".to_string(),
            headers: headers.into_iter().map(str::to_string).collect(),
            rows: rows.into_iter().map(|r| r.into_iter().map(smol_str::SmolStr::from).collect()).collect(),
            bytes: 0,
        }
    }

    #[test]
    fn valid_calendar_date_produces_no_notices() {
        let file = make_file(
            vec!["service_id", "date", "exception_type"],
            vec![vec!["SVC1", "20260101", "1"]],
        );
        let (records, notices) = validate_calendar_dates(&file);
        assert_eq!(records.len(), 1);
        assert!(notices.is_empty(), "Geçerli kayıt notice üretmemeli: {:?}", notices);
    }

    #[test]
    fn missing_service_id_produces_cld_001() {
        let file = make_file(
            vec!["service_id", "date", "exception_type"],
            vec![vec!["", "20260101", "1"]],
        );
        let (_, notices) = validate_calendar_dates(&file);
        assert!(notices.iter().any(|n| n.rule_id == "CLD_001"));
    }

    #[test]
    fn invalid_date_produces_cld_002() {
        let file = make_file(
            vec!["service_id", "date", "exception_type"],
            vec![vec!["SVC1", "2026-01-01", "1"]],
        );
        let (_, notices) = validate_calendar_dates(&file);
        assert!(notices.iter().any(|n| n.rule_id == "CLD_002"));
    }

    #[test]
    fn invalid_exception_type_produces_cld_003() {
        let file = make_file(
            vec!["service_id", "date", "exception_type"],
            vec![vec!["SVC1", "20260101", "3"]],
        );
        let (_, notices) = validate_calendar_dates(&file);
        assert!(notices.iter().any(|n| n.rule_id == "CLD_003"));
    }
}
