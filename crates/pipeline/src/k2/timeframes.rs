use gtfs_core::EntityType;
use std::collections::HashMap;

use super::common::{build_row_map, get_trimmed_field, make_k2_notice, parse_gtfs_time, RowMap};
use crate::k1_parse::RawFile;

#[derive(Debug, Clone)]
pub struct TimeframeRecord {
    pub timeframe_group_id: String,
    pub start_time: Option<(u32, u32, u32)>,
    pub end_time: Option<(u32, u32, u32)>,
    pub service_id: String,
    pub row: RowMap,
    pub line: u64,
}

pub fn validate_timeframes(
    file: &RawFile,
) -> (Vec<TimeframeRecord>, Vec<gtfs_core::Notice>) {
    let mut notices = Vec::new();
    let mut records = Vec::new();
    let mut counter = 0;

    for (row_idx, row) in file.rows.iter().enumerate() {
        let line = (row_idx + 2) as u64;
        let row_map = build_row_map(&file.headers, row);
        let group_id = get_trimmed_field(&row_map, "timeframe_group_id").unwrap_or("").to_string();
        let entity_id = (!group_id.is_empty()).then_some(group_id.clone());

        if get_trimmed_field(&row_map, "timeframe_group_id") == Some("") {
            notices.push(make_k2_notice(
                &mut counter, "TFR_001", EntityType::Row, None, Some(&row_map),
                &file.name, Some(line), Some("timeframe_group_id"), None,
                Some("dolu".to_string()),
                "timeframe_group_id zorunludur.".to_string(),
                "Her zaman dilimi kaydı için bir timeframe_group_id girin.",
            ));
        }

        let service_id = get_trimmed_field(&row_map, "service_id").unwrap_or("").to_string();

        let start_time = match parse_gtfs_time(&row_map, "start_time") {
            Ok(v) => v,
            Err(err) => {
                notices.push(make_k2_notice(
                    &mut counter, "TFR_003", EntityType::Row, entity_id.clone(), Some(&row_map),
                    &file.name, Some(line), Some("start_time"),
                    get_trimmed_field(&row_map, "start_time").map(str::to_string),
                    Some("HH:MM:SS".to_string()), err,
                    "start_time için HH:MM:SS formatında geçerli bir değer girin.",
                ));
                None
            }
        };

        let end_time = match parse_gtfs_time(&row_map, "end_time") {
            Ok(v) => v,
            Err(err) => {
                notices.push(make_k2_notice(
                    &mut counter, "TFR_003", EntityType::Row, entity_id.clone(), Some(&row_map),
                    &file.name, Some(line), Some("end_time"),
                    get_trimmed_field(&row_map, "end_time").map(str::to_string),
                    Some("HH:MM:SS".to_string()), err,
                    "end_time için HH:MM:SS formatında geçerli bir değer girin.",
                ));
                None
            }
        };

        // TFR_007: spec her iki alan için simetrik koşullu zorunluluk koyar — start_time
        // "Required if timeframes.end_time is defined. - Forbidden otherwise", end_time için
        // aynısı ters yönde. Yani ikisi de boş olabilir (spec'e göre 00:00:00–24:00:00, tüm
        // gün) ya da ikisi de dolu olabilir; YALNIZCA BİRİ dolu olamaz.
        // Parse edilmiş değere DEĞİL ham alana bakılır: biçim hatasında parse None döner ve
        // TFR_003 zaten emit edilmiştir, buradan ikinci bir notice çıkmamalıdır.
        let start_raw_empty = get_trimmed_field(&row_map, "start_time").unwrap_or("").is_empty();
        let end_raw_empty = get_trimmed_field(&row_map, "end_time").unwrap_or("").is_empty();
        if start_raw_empty != end_raw_empty {
            let (missing, present) = if start_raw_empty {
                ("start_time", "end_time")
            } else {
                ("end_time", "start_time")
            };
            notices.push(make_k2_notice(
                &mut counter, "TFR_007", EntityType::Row, entity_id.clone(), Some(&row_map),
                &file.name, Some(line), Some(missing), Some(String::new()),
                Some("dolu".to_string()),
                format!("{present} tanımlıyken {missing} da tanımlanmalıdır."),
                "timeframes.txt'te start_time ve end_time alanlarını birlikte doldurun veya ikisini de boş bırakın.",
            ));
        }

        // TFR_006: spec `timeframes.txt` start_time/end_time — "Values greater than 24:00:00
        // are forbidden". 24:00:00'ın kendisi geçerlidir, üstü değil. parse_gtfs_time saat
        // bölümüne üst sınır koymaz (25:10:05 bilinçli olarak kabul edilir), bu yüzden sınır
        // burada ayrıca denetlenir. Saniyeye çevirmeden karşılaştırılır: çok büyük saat
        // değerlerinde `hour * 3600` u32'de taşar.
        for (field, parsed) in [("start_time", start_time), ("end_time", end_time)] {
            let Some((hour, minute, second)) = parsed else { continue };
            if hour > 24 || (hour == 24 && (minute > 0 || second > 0)) {
                notices.push(make_k2_notice(
                    &mut counter, "TFR_006", EntityType::Row, entity_id.clone(), Some(&row_map),
                    &file.name, Some(line), Some(field),
                    get_trimmed_field(&row_map, field).map(str::to_string),
                    Some("<= 24:00:00".to_string()),
                    format!("{field} değeri 24:00:00'dan büyük olamaz."),
                    "timeframes.txt'te start_time/end_time değerlerini 24:00:00 ve altına indirin.",
                ));
            }
        }

        if let (Some(st), Some(et)) = (start_time, end_time) {
            // u64: parse_gtfs_time saat bölümüne üst sınır koymaz, `hour * 3600` u32'de taşar.
            let st_secs = st.0 as u64 * 3600 + st.1 as u64 * 60 + st.2 as u64;
            let et_secs = et.0 as u64 * 3600 + et.1 as u64 * 60 + et.2 as u64;
            if et_secs <= st_secs {
                notices.push(make_k2_notice(
                    &mut counter, "TFR_004", EntityType::Row, entity_id.clone(), Some(&row_map),
                    &file.name, Some(line), Some("end_time"),
                    get_trimmed_field(&row_map, "end_time").map(str::to_string),
                    Some("> start_time".to_string()),
                    "end_time, start_time'dan büyük olmalıdır.".to_string(),
                    "end_time değerini start_time'dan sonraya ayarlayın.",
                ));
            }
        }

        records.push(TimeframeRecord {
            timeframe_group_id: group_id,
            start_time,
            end_time,
            service_id,
            row: row_map,
            line,
        });
    }

    // TFR_005: aynı (timeframe_group_id, service_id) grubunda örtüşen zaman aralıkları
    // Gruplama: key → Vec<(start_secs, end_secs, line)>
    let mut groups: HashMap<(&str, &str), Vec<(u64, u64, u64)>> = HashMap::new();
    for rec in &records {
        if let (Some(st), Some(et)) = (rec.start_time, rec.end_time) {
            let st_secs = st.0 as u64 * 3600 + st.1 as u64 * 60 + st.2 as u64;
            let et_secs = et.0 as u64 * 3600 + et.1 as u64 * 60 + et.2 as u64;
            groups
                .entry((rec.timeframe_group_id.as_str(), rec.service_id.as_str()))
                .or_default()
                .push((st_secs, et_secs, rec.line));
        }
    }
    for ((group_id, service_id), mut intervals) in groups {
        intervals.sort_by_key(|&(st, _, _)| st);
        for i in 1..intervals.len() {
            let (prev_st, prev_et, prev_line) = intervals[i - 1];
            let (cur_st, _, cur_line) = intervals[i];
            if cur_st < prev_et {
                notices.push(make_k2_notice(
                    &mut counter, "TFR_005", EntityType::Row,
                    Some(group_id.to_string()), None,
                    &file.name, Some(cur_line), Some("start_time"),
                    Some(format!("{cur_line} / {prev_line}")), None,
                    format!(
                        "timeframe_group_id '{}' service_id '{}': {prev_st}–{prev_et}s ile {cur_st}s aralıkları örtüşüyor.",
                        group_id, service_id
                    ),
                    "Aynı grup ve service_id içindeki zaman aralıklarının örtüşmediğinden emin olun.",
                ));
            }
        }
    }

    (records, notices)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::k1_parse::RawFile;

    fn make_file(rows: Vec<Vec<&str>>) -> RawFile {
        use smol_str::SmolStr;
        RawFile {
            name: "timeframes.txt".to_string(),
            headers: ["timeframe_group_id", "start_time", "end_time", "service_id"]
                .iter()
                .map(|h| h.to_string())
                .collect(),
            rows: rows.into_iter().map(|r| r.into_iter().map(SmolStr::new).collect()).collect(),
            bytes: 0,
            raw_text: None,
        }
    }

    fn tfr_006_count(rows: Vec<Vec<&str>>) -> usize {
        let (_, notices) = validate_timeframes(&make_file(rows));
        notices.iter().filter(|n| n.rule_id == "TFR_006").count()
    }

    #[test]
    fn tfr_006_allows_exactly_twenty_four_hours() {
        // Spec: "Values greater than 24:00:00 are forbidden" — 24:00:00'ın KENDİSİ geçerli.
        assert_eq!(tfr_006_count(vec![vec!["TG1", "08:00:00", "24:00:00", "SVC1"]]), 0);
    }

    #[test]
    fn tfr_006_flags_one_second_past_twenty_four_hours() {
        assert_eq!(tfr_006_count(vec![vec!["TG1", "08:00:00", "24:00:01", "SVC1"]]), 1);
    }

    #[test]
    fn tfr_006_flags_start_and_end_separately() {
        assert_eq!(tfr_006_count(vec![vec!["TG1", "25:00:00", "26:00:00", "SVC1"]]), 2);
    }

    #[test]
    fn tfr_006_ignores_normal_and_empty_times() {
        assert_eq!(tfr_006_count(vec![vec!["TG1", "08:00:00", "12:00:00", "SVC1"]]), 0);
        // Boş değer spec'te 00:00:00 sayılır; parse None döner, kural konusu değildir.
        assert_eq!(tfr_006_count(vec![vec!["TG1", "", "", "SVC1"]]), 0);
    }

    fn tfr_007_count(rows: Vec<Vec<&str>>) -> usize {
        let (_, notices) = validate_timeframes(&make_file(rows));
        notices.iter().filter(|n| n.rule_id == "TFR_007").count()
    }

    #[test]
    fn tfr_007_flags_only_one_side_specified() {
        assert_eq!(tfr_007_count(vec![vec!["TG1", "08:00:00", "", "SVC1"]]), 1);
        assert_eq!(tfr_007_count(vec![vec!["TG1", "", "12:00:00", "SVC1"]]), 1);
    }

    #[test]
    fn tfr_007_allows_both_present_or_both_empty() {
        assert_eq!(tfr_007_count(vec![vec!["TG1", "08:00:00", "12:00:00", "SVC1"]]), 0);
        // Spec: boş start_time = 00:00:00, boş end_time = 24:00:00 → ikisi birden boşsa tüm gün.
        assert_eq!(tfr_007_count(vec![vec!["TG1", "", "", "SVC1"]]), 0);
    }

    #[test]
    fn tfr_007_does_not_double_report_a_format_error() {
        // Biçim hatasında parse None döner ama alan DOLUDUR; TFR_003 emit edilir, TFR_007 edilmez.
        let (_, notices) = validate_timeframes(&make_file(vec![vec![
            "TG1", "notatime", "12:00:00", "SVC1",
        ]]));
        assert_eq!(notices.iter().filter(|n| n.rule_id == "TFR_003").count(), 1);
        assert_eq!(notices.iter().filter(|n| n.rule_id == "TFR_007").count(), 0);
    }

    #[test]
    fn tfr_006_does_not_overflow_on_huge_hour() {
        // hour * 3600 u32'de taşardı; karşılaştırma saniyeye çevirmeden yapılıyor.
        assert_eq!(tfr_006_count(vec![vec!["TG1", "9999999:00:00", "08:00:00", "SVC1"]]), 1);
    }
}
