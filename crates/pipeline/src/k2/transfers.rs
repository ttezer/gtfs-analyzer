use gtfs_core::EntityType;

use super::common::{build_row_map, get_trimmed_field, make_k2_notice, parse_u32, validate_enum, RowMap};
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

pub fn validate_transfers(file: &RawFile) -> (Vec<TransferRecord>, Vec<gtfs_core::Notice>) {
    let mut notices = Vec::new();
    let mut records = Vec::new();
    let mut counter = 0;

    for (row_idx, row) in file.rows.iter().enumerate() {
        let line = (row_idx + 2) as u64;
        let row_map = build_row_map(&file.headers, row);
        let from_stop_id = get_trimmed_field(&row_map, "from_stop_id").unwrap_or("").to_string();
        let to_stop_id = get_trimmed_field(&row_map, "to_stop_id").unwrap_or("").to_string();
        let entity_id = (!from_stop_id.is_empty() && !to_stop_id.is_empty()).then_some(format!("{from_stop_id}|{to_stop_id}"));

        // TRF_004: transfer_type geçersiz (TRF_001/002 koşulu buna bağlı olduğundan önce parse edilir)
        let transfer_type = match parse_u32(&row_map, "transfer_type") {
            Ok(value) => {
                if let Some(v) = value {
                    if !validate_enum(&v.to_string(), &["0", "1", "2", "3", "4", "5"]) {
                        notices.push(make_k2_notice(
                            &mut counter, "TRF_004", EntityType::Row, entity_id.clone(), Some(&row_map),
                            &file.name, Some(line), Some("transfer_type"), Some(v.to_string()),
                            Some("0–5".to_string()),
                            format!("transfer_type '{v}' geçersiz."),
                            "transfer_type'ı geçerli bir GTFS enum değerine ayarlayın (0–5).",
                        ));
                    }
                }
                value
            }
            Err(err) => {
                notices.push(make_k2_notice(
                    &mut counter, "TRF_004", EntityType::Row, entity_id.clone(), Some(&row_map),
                    &file.name, Some(line), Some("transfer_type"),
                    get_trimmed_field(&row_map, "transfer_type").map(str::to_string),
                    Some("0–5".to_string()), err,
                    "transfer_type'ı geçerli bir GTFS enum değerine ayarlayın (0–5).",
                ));
                None
            }
        };

        // from_stop_id/to_stop_id koşullu zorunlu: transfer_type 0/1/2/3 için zorunlu,
        // 4/5 (in-seat / scheduled connection) için opsiyonel — bunlar from_trip_id/to_trip_id
        // kullanır. Sütun yoksa ARC_025 devralır → atla (get_trimmed_field == Some("") sütun var
        // ama değer boş demek). transfer_type geçersiz/yoksa varsayılan 0 → zorunlu kabul edilir.
        let stop_ids_required = !matches!(transfer_type, Some(4) | Some(5));

        // TRF_001: from_stop_id zorunlu
        if stop_ids_required && get_trimmed_field(&row_map, "from_stop_id") == Some("") {
            notices.push(make_k2_notice(
                &mut counter, "TRF_001", EntityType::Row, entity_id.clone(), Some(&row_map),
                &file.name, Some(line), Some("from_stop_id"), Some(String::new()), None,
                "from_stop_id zorunludur.".to_string(),
                "Aktarma kaydına from_stop_id ekleyin.",
            ));
        }

        // TRF_002: to_stop_id zorunlu
        if stop_ids_required && get_trimmed_field(&row_map, "to_stop_id") == Some("") {
            notices.push(make_k2_notice(
                &mut counter, "TRF_002", EntityType::Row, entity_id.clone(), Some(&row_map),
                &file.name, Some(line), Some("to_stop_id"), Some(String::new()), None,
                "to_stop_id zorunludur.".to_string(),
                "Aktarma kaydına to_stop_id ekleyin.",
            ));
        }

        // TRF_005: transfer_type=2 için min_transfer_time zorunlu
        let min_transfer_time = match parse_u32(&row_map, "min_transfer_time") {
            Ok(value) => value,
            Err(err) => {
                notices.push(make_k2_notice(
                    &mut counter, "TRF_005", EntityType::Row, entity_id.clone(), Some(&row_map),
                    &file.name, Some(line), Some("min_transfer_time"),
                    get_trimmed_field(&row_map, "min_transfer_time").map(str::to_string),
                    Some(">= 0".to_string()), err,
                    "min_transfer_time için sıfır veya pozitif bir tamsayı girin.",
                ));
                None
            }
        };

        if matches!(transfer_type, Some(2)) && min_transfer_time.is_none() {
            notices.push(make_k2_notice(
                &mut counter, "TRF_005", EntityType::Row, entity_id.clone(), Some(&row_map),
                &file.name, Some(line), Some("min_transfer_time"), Some(String::new()),
                None, "transfer_type=2 için min_transfer_time zorunludur.".to_string(),
                "min_transfer_time alanını doldurun.",
            ));
        }

        // GGL_001: transfer_type=4/5 Google Transit tarafından desteklenmiyor
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

        records.push(TransferRecord {
            from_stop_id,
            to_stop_id,
            transfer_type,
            min_transfer_time,
            from_trip_id: get_trimmed_field(&row_map, "from_trip_id").filter(|v| !v.is_empty()).map(str::to_string),
            to_trip_id: get_trimmed_field(&row_map, "to_trip_id").filter(|v| !v.is_empty()).map(str::to_string),
            from_route_id: get_trimmed_field(&row_map, "from_route_id").filter(|v| !v.is_empty()).map(str::to_string),
            to_route_id: get_trimmed_field(&row_map, "to_route_id").filter(|v| !v.is_empty()).map(str::to_string),
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
            headers: vec!["from_stop_id".into(), "to_stop_id".into(), "transfer_type".into()],
            rows: rows.iter().map(|r| r.iter().map(|v| SmolStr::new(*v)).collect()).collect(),
            bytes: 0,
            raw_text: None,
        }
    }

    #[test]
    fn ggl_001_fires_for_in_seat_transfer_type_4() {
        let file = make_file(vec![vec!["S1", "S2", "4"]]);
        let (_, notices) = validate_transfers(&file);
        assert!(notices.iter().any(|n| n.rule_id == "GGL_001"), "GGL_001 bekleniyor");
    }

    #[test]
    fn ggl_001_fires_for_in_seat_transfer_type_5() {
        let file = make_file(vec![vec!["S1", "S2", "5"]]);
        let (_, notices) = validate_transfers(&file);
        assert!(notices.iter().any(|n| n.rule_id == "GGL_001"), "GGL_001 bekleniyor");
    }

    #[test]
    fn ggl_001_silent_for_standard_transfer_types() {
        let file = make_file(vec![vec!["S1", "S2", "0"], vec!["S3", "S4", "1"]]);
        let (_, notices) = validate_transfers(&file);
        assert!(!notices.iter().any(|n| n.rule_id == "GGL_001"), "GGL_001 tetiklenmemeli");
    }

    #[test]
    fn trf_001_002_skipped_for_transfer_type_4() {
        // transfer_type=4 (in-seat) için from/to_stop_id opsiyonel → boş olsa da TRF_001/002 yok
        let file = make_file(vec![vec!["", "", "4"]]);
        let (_, notices) = validate_transfers(&file);
        let ids: Vec<&str> = notices.iter().map(|n| n.rule_id.as_str()).collect();
        assert!(!ids.contains(&"TRF_001"), "transfer_type=4'te from_stop_id zorunlu olmamalı: {:?}", ids);
        assert!(!ids.contains(&"TRF_002"), "transfer_type=4'te to_stop_id zorunlu olmamalı: {:?}", ids);
    }

    #[test]
    fn trf_001_fires_for_transfer_type_1_missing_stop() {
        // transfer_type=1 için from/to_stop_id zorunlu → boşsa TRF_001/002
        let file = make_file(vec![vec!["", "", "1"]]);
        let (_, notices) = validate_transfers(&file);
        let ids: Vec<&str> = notices.iter().map(|n| n.rule_id.as_str()).collect();
        assert!(ids.contains(&"TRF_001"), "transfer_type=1'de from_stop_id eksik → TRF_001: {:?}", ids);
        assert!(ids.contains(&"TRF_002"), "transfer_type=1'de to_stop_id eksik → TRF_002: {:?}", ids);
    }
}
