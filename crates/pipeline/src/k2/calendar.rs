use gtfs_core::EntityType;

use super::common::{get_raw_field,
    build_row_map, get_trimmed_field, make_k2_notice, parse_service_date, parse_u32,
    validate_enum, RowMap,
};
use crate::k1_parse::RawFile;

#[derive(Debug, Clone)]
pub struct CalendarRecord {
    pub service_id: String,
    /// monday..sunday (days[0]=monday, days[6]=sunday)
    pub days: [Option<u32>; 7],
    pub start_date: Option<(u32, u32, u32)>,
    pub end_date: Option<(u32, u32, u32)>,
    pub row: RowMap,
    pub line: u64,
}

const DAY_FIELDS: [&str; 7] = [
    "monday", "tuesday", "wednesday", "thursday", "friday", "saturday", "sunday",
];

pub fn validate_calendar(file: &RawFile) -> (Vec<CalendarRecord>, Vec<gtfs_core::Notice>) {
    let mut notices = Vec::new();
    let mut records = Vec::new();
    let mut counter = 0u32;

    for (row_idx, row) in file.rows.iter().enumerate() {
        let line = (row_idx + 2) as u64;
        let row_map = build_row_map(&file.headers, row);

        let service_id = get_raw_field(&row_map, "service_id").unwrap_or("").to_string();
        let entity_id = (!service_id.is_empty()).then_some(service_id.clone());

        // CAL_022: service_id required (sütun yoksa ARC_025 devralır → atla)
        if get_raw_field(&row_map, "service_id").map(str::trim) == Some("") {
            notices.push( make_k2_notice(
                &mut counter, "CAL_022", EntityType::Service, None, Some(&row_map),
                &file.name, Some(line), Some("service_id"), Some(String::new()), None,
                "service_id zorunludur.".to_string(),
                "service_id alanını doldurun.",
            ));
        }

        // CAL_002: day fields must be 0 or 1
        let mut days = [None::<u32>; 7];
        let mut all_zero = true;
        for (i, field) in DAY_FIELDS.iter().enumerate() {
            // A numeric weekday with surrounding whitespace is still usable for the
            // weekly-pattern decision; DQ_016 remains the lexical root finding. Keep
            // non-numeric and out-of-enum trimmed values on the CAL_002 path so an
            // independent semantic error is not mistaken for whitespace noise.
            let parsed_day = match get_raw_field(&row_map, field) {
                Some(raw) if raw != raw.trim() && !raw.trim().is_empty() => raw
                    .trim()
                    .parse::<u32>()
                    .map(Some)
                    .map_err(|_| format!("'{field}' için u32 bekleniyor, alınan: {raw}")),
                _ => parse_u32(&row_map, field),
            };
            match parsed_day {
                Ok(v) => {
                    if let Some(val) = v {
                        if !validate_enum(&val.to_string(), &["0", "1"]) {
                            notices.push( make_k2_notice(
                                &mut counter, "CAL_002", EntityType::Service, entity_id.clone(),
                                Some(&row_map), &file.name, Some(line), Some(field),
                                Some(val.to_string()), Some("0 veya 1".to_string()),
                                format!("{field} alanı 0 veya 1 olmalıdır."),
                                "Her gün alanını 0 veya 1 olarak ayarlayın.",
                            ));
                        }
                        if val == 1 {
                            all_zero = false;
                        }
                        days[i] = Some(val);
                    }
                }
                Err(err) => {
                    notices.push( make_k2_notice(
                        &mut counter, "CAL_002", EntityType::Service, entity_id.clone(),
                        Some(&row_map), &file.name, Some(line), Some(field),
                        get_trimmed_field(&row_map, field).map(str::to_string),
                        Some("0 veya 1".to_string()), err,
                        "Her gün alanını 0 veya 1 olarak ayarlayın.",
                    ));
                }
            }
        }

        // CAL_006: haftalık gün alanlarının tümü 0 → haftalık tekrar yok. Bu GEÇERLİ bir
        // GTFS desenidir (servis yalnız calendar_dates.txt istisnalarıyla aktif olabilir),
        // bu yüzden BİLGİ. Servis gerçekten hiç aktif gün içermiyorsa (calendar_dates de yok)
        // bunu OPR_011 (kullanılan) / CAL_011 (kullanılmayan) daha yüksek şiddette yakalar.
        if all_zero && days.iter().any(|d| d.is_some()) {
            notices.push( make_k2_notice(
                &mut counter, "CAL_006", EntityType::Service, entity_id.clone(),
                Some(&row_map), &file.name, Some(line),
                // Olgu YEDİ gün alanının BİRLİKTE 0 olmasıdır; hepsi adlandırılır.
                Some("monday|tuesday|wednesday|thursday|friday|saturday|sunday"), None, None,
                format!("'{}' takviminde haftalık gün alanlarının tümü 0 — haftalık tekrar yok; servis yalnızca calendar_dates.txt istisnalarıyla aktif olabilir.", service_id),
                "Bilinçli bir dates-only servisse işlem gerekmez; değilse en az bir gün alanını 1 yapın.",
            ));
        }

        // CAL_003: start_date required + valid YYYYMMDD
        let start_date = match parse_service_date(&row_map, "start_date") {
            Ok(v) => {
                if get_trimmed_field(&row_map, "start_date") == Some("") {
                    notices.push( make_k2_notice(
                        &mut counter, "CAL_003", EntityType::Service, entity_id.clone(),
                        Some(&row_map), &file.name, Some(line), Some("start_date"),
                        Some(String::new()), None,
                        "start_date zorunludur.".to_string(),
                        "start_date alanını YYYYMMDD formatında doldurun.",
                    ));
                }
                v
            }
            Err(err) => {
                notices.push( make_k2_notice(
                    &mut counter, "CAL_003", EntityType::Service, entity_id.clone(),
                    Some(&row_map), &file.name, Some(line), Some("start_date"),
                    get_trimmed_field(&row_map, "start_date").map(str::to_string),
                    Some("YYYYMMDD".to_string()), err,
                    "start_date alanını YYYYMMDD formatında doldurun.",
                ));
                None
            }
        };

        // CAL_004: end_date required + valid YYYYMMDD
        let end_date = match parse_service_date(&row_map, "end_date") {
            Ok(v) => {
                if get_trimmed_field(&row_map, "end_date") == Some("") {
                    notices.push( make_k2_notice(
                        &mut counter, "CAL_004", EntityType::Service, entity_id.clone(),
                        Some(&row_map), &file.name, Some(line), Some("end_date"),
                        Some(String::new()), None,
                        "end_date zorunludur.".to_string(),
                        "end_date alanını YYYYMMDD formatında doldurun.",
                    ));
                }
                v
            }
            Err(err) => {
                notices.push( make_k2_notice(
                    &mut counter, "CAL_004", EntityType::Service, entity_id.clone(),
                    Some(&row_map), &file.name, Some(line), Some("end_date"),
                    get_trimmed_field(&row_map, "end_date").map(str::to_string),
                    Some("YYYYMMDD".to_string()), err,
                    "end_date alanını YYYYMMDD formatında doldurun.",
                ));
                None
            }
        };

        // CAL_005: end_date must not precede start_date
        if let (Some(s), Some(e)) = (start_date, end_date) {
            let sd = s.0 * 10000 + s.1 * 100 + s.2;
            let ed = e.0 * 10000 + e.1 * 100 + e.2;
            if ed < sd {
                notices.push( make_k2_notice(
                    &mut counter, "CAL_005", EntityType::Service, entity_id.clone(),
                    Some(&row_map), &file.name, Some(line), Some("end_date"),
                    Some(format!("{}", ed)), Some(format!(">= {}", sd)),
                    format!("end_date ({ed}), start_date ({sd}) tarihinden önce."),
                    "end_date tarihini start_date tarihine eşit veya sonrasına ayarlayın.",
                ));
            }
        }

        records.push(CalendarRecord {
            service_id,
            days,
            start_date,
            end_date,
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
            name: "calendar.txt".to_string(),
            headers: headers.into_iter().map(str::to_string).collect(),
            rows: rows.into_iter().map(|r| r.into_iter().map(smol_str::SmolStr::from).collect()).collect(),
            bytes: 0,
            raw_text: None,
        }
    }

    fn all_headers() -> Vec<&'static str> {
        vec![
            "service_id", "monday", "tuesday", "wednesday", "thursday",
            "friday", "saturday", "sunday", "start_date", "end_date",
        ]
    }

    #[test]
    fn valid_calendar_produces_no_notices() {
        let file = make_file(
            all_headers(),
            vec![vec!["SVC1", "1","1","1","1","1","0","0", "20260101", "20271231"]],
        );
        let (records, notices) = validate_calendar(&file);
        assert_eq!(records.len(), 1);
        assert!(notices.is_empty(), "Geçerli takvim notice üretmemeli: {:?}", notices);
    }

    #[test]
    fn invalid_day_value_produces_cal_002() {
        let file = make_file(
            all_headers(),
            vec![vec!["SVC1", "2","1","1","1","1","0","0", "20260101", "20271231"]],
        );
        let (_, notices) = validate_calendar(&file);
        assert!(notices.iter().any(|n| n.rule_id == "CAL_002"));
    }

    #[test]
    fn all_zero_days_produces_cal_006() {
        let file = make_file(
            all_headers(),
            vec![vec!["SVC1", "0","0","0","0","0","0","0", "20260101", "20271231"]],
        );
        let (_, notices) = validate_calendar(&file);
        assert!(notices.iter().any(|n| n.rule_id == "CAL_006"));
    }

    #[test]
    fn whitespace_all_zero_days_still_produces_cal_006() {
        let file = make_file(
            all_headers(),
            vec![vec!["SVC1", " 0"," 0"," 0"," 0"," 0"," 0"," 0", "20260101", "20271231"]],
        );
        let (_, notices) = validate_calendar(&file);
        assert!(notices.iter().any(|n| n.rule_id == "CAL_006"));
        assert!(!notices.iter().any(|n| n.rule_id == "CAL_002"),
            "trim sonrası geçerli 0 değerleri CAL_002 üretmemeli: {notices:?}");
    }

    #[test]
    fn whitespace_weekday_one_keeps_cal_006_silent() {
        let file = make_file(
            all_headers(),
            vec![vec!["SVC1", " 1"," 0"," 0"," 0"," 0"," 0"," 0", "20260101", "20271231"]],
        );
        let (_, notices) = validate_calendar(&file);
        assert!(!notices.iter().any(|n| n.rule_id == "CAL_006"));
    }

    #[test]
    fn whitespace_invalid_weekday_keeps_cal_002() {
        let file = make_file(
            all_headers(),
            vec![vec!["SVC1", " 2"," 0"," 0"," 0"," 0"," 0"," 0", "20260101", "20271231"]],
        );
        let (_, notices) = validate_calendar(&file);
        assert!(notices.iter().any(|n| n.rule_id == "CAL_002"));
        assert!(notices.iter().any(|n| n.rule_id == "CAL_006"),
            "parse edilebilen diğer 0 değerleri bağımsız CAL_006'ı korumalı: {notices:?}");
    }

    #[test]
    fn end_before_start_produces_cal_005() {
        let file = make_file(
            all_headers(),
            vec![vec!["SVC1", "1","0","0","0","0","0","0", "20271231", "20260101"]],
        );
        let (_, notices) = validate_calendar(&file);
        assert!(notices.iter().any(|n| n.rule_id == "CAL_005"));
    }

    #[test]
    fn missing_start_date_produces_cal_003() {
        let file = make_file(
            all_headers(),
            vec![vec!["SVC1", "1","0","0","0","0","0","0", "", "20271231"]],
        );
        let (_, notices) = validate_calendar(&file);
        assert!(notices.iter().any(|n| n.rule_id == "CAL_003"));
    }
}
