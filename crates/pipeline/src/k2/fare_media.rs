use gtfs_core::EntityType;

use super::common::{build_row_map, get_trimmed_field, make_k2_notice, parse_u32, validate_enum, RowMap};
use crate::k1_parse::RawFile;

#[derive(Debug, Clone)]
pub struct FareMediaRecord {
    pub fare_media_id: String,
    pub fare_media_name: Option<String>,
    pub fare_media_type: Option<u32>,
    pub row: RowMap,
    pub line: u64,
}

pub fn validate_fare_media(
    file: &RawFile,
) -> (Vec<FareMediaRecord>, Vec<gtfs_core::Notice>) {
    let mut notices = Vec::new();
    let mut records = Vec::new();
    let mut counter = 0;

    for (row_idx, row) in file.rows.iter().enumerate() {
        let line = (row_idx + 2) as u64;
        let row_map = build_row_map(&file.headers, row);
        let id = get_trimmed_field(&row_map, "fare_media_id").unwrap_or("").to_string();
        let entity_id = (!id.is_empty()).then_some(id.clone());

        let fare_media_type = match parse_u32(&row_map, "fare_media_type") {
            Ok(value) => {
                if let Some(v) = value {
                    if !validate_enum(&v.to_string(), &["0", "1", "2", "3", "4"]) {
                        notices.push(make_k2_notice(
                            &mut counter, "FMD_002", EntityType::Row, entity_id.clone(), Some(&row_map),
                            &file.name, Some(line), Some("fare_media_type"), Some(v.to_string()),
                            Some("0–4".to_string()),
                            "fare_media_type geçerli bir enum değeri değil.".to_string(),
                            "0 (yok), 1 (fiziksel kart), 2 (mobil uygulama), 3 (EMV temassız), 4 (transit kuruluş uygulaması) kullanın.",
                        ));
                    }
                } else if get_trimmed_field(&row_map, "fare_media_type") == Some("") {
                    notices.push(make_k2_notice(
                        &mut counter, "FMD_002", EntityType::Row, entity_id.clone(), Some(&row_map),
                        &file.name, Some(line), Some("fare_media_type"), None,
                        Some("0–4".to_string()),
                        "fare_media_type zorunludur.".to_string(),
                        "Geçerli bir fare_media_type değeri girin.",
                    ));
                }
                value
            }
            Err(err) => {
                notices.push(make_k2_notice(
                    &mut counter, "FMD_002", EntityType::Row, entity_id.clone(), Some(&row_map),
                    &file.name, Some(line), Some("fare_media_type"),
                    get_trimmed_field(&row_map, "fare_media_type").map(str::to_string),
                    Some("0–4".to_string()), err,
                    "Geçerli bir fare_media_type değeri girin.",
                ));
                None
            }
        };

        let fare_media_name = get_trimmed_field(&row_map, "fare_media_name")
            .filter(|v| !v.is_empty())
            .map(str::to_string);

        // FMD_003: TransitCard/MobileApp/AgencyApp için fare_media_name tavsiye edilir
        if fare_media_name.is_none() && matches!(fare_media_type, Some(1) | Some(2) | Some(4)) {
            let type_label = match fare_media_type {
                Some(1) => "fiziksel kart",
                Some(2) => "mobil uygulama",
                _ => "transit kuruluş uygulaması",
            };
            notices.push(make_k2_notice(
                &mut counter, "FMD_003", EntityType::Row, entity_id.clone(), Some(&row_map),
                &file.name, Some(line), Some("fare_media_name"), None, None,
                format!("fare_media_type={} ({type_label}) için fare_media_name tavsiye edilir.", fare_media_type.unwrap()),
                "Kullanıcıların ödeme aracını tanıyabilmesi için fare_media_name ekleyin.",
            ));
        }

        records.push(FareMediaRecord {
            fare_media_id: id,
            fare_media_name,
            fare_media_type,
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

    fn make_file(headers: &[&str], rows: Vec<Vec<&str>>) -> RawFile {
        RawFile {
            name: "fare_media.txt".into(),
            headers: headers.iter().map(|s| s.to_string()).collect(),
            rows: rows.iter().map(|r| r.iter().map(|v| SmolStr::new(*v)).collect()).collect(),
            bytes: 0,
            raw_text: None,
        }
    }

    #[test]
    fn fmd_003_fires_for_transit_card_without_name() {
        let file = make_file(
            &["fare_media_id", "fare_media_type"],
            vec![vec!["FM1", "1"]],
        );
        let (_, notices) = validate_fare_media(&file);
        assert!(notices.iter().any(|n| n.rule_id == "FMD_003"), "FMD_003 bekleniyor");
    }

    #[test]
    fn fmd_003_silent_when_name_present() {
        let file = make_file(
            &["fare_media_id", "fare_media_type", "fare_media_name"],
            vec![vec!["FM1", "1", "İstanbulkart"]],
        );
        let (_, notices) = validate_fare_media(&file);
        assert!(!notices.iter().any(|n| n.rule_id == "FMD_003"), "FMD_003 tetiklenmemeli");
    }

    #[test]
    fn fmd_003_silent_for_type_0_and_3() {
        let file = make_file(
            &["fare_media_id", "fare_media_type"],
            vec![vec!["FM1", "0"], vec!["FM2", "3"]],
        );
        let (_, notices) = validate_fare_media(&file);
        assert!(!notices.iter().any(|n| n.rule_id == "FMD_003"), "FMD_003 tetiklenmemeli");
    }
}
