use gtfs_core::EntityType;

use super::common::{
    build_row_map, get_field, get_trimmed_field, looks_like_iana_timezone, make_k2_notice,
    parse_f64, parse_u32, validate_enum, RowMap,
};
use crate::k1_parse::RawFile;

#[derive(Debug, Clone, Default)]
pub struct StopRecord {
    pub stop_id: String,
    pub stop_code: Option<String>,
    pub stop_name: Option<String>,
    pub stop_lat: Option<f64>,
    pub stop_lon: Option<f64>,
    pub location_type: Option<u32>,
    pub stop_timezone: Option<String>,
    pub wheelchair_boarding: Option<u32>,
    pub stop_access: Option<u32>,
    pub level_id: Option<String>,
    pub tts_stop_name: Option<String>,
    pub stop_url: Option<String>,
    pub row: RowMap,
    pub line: u64,
}

pub fn validate_stops(file: &RawFile) -> (Vec<StopRecord>, Vec<gtfs_core::Notice>) {
    let mut notices = Vec::new();
    let mut records = Vec::new();
    let mut counter = 0u32;

    // STP_022 agregasyonu: stop_code feed genelinde hiç kullanılmamışsa (tek üretici
    // kararı), durak-başına on binlerce özdeş notice yerine tek özet üretilir (STM_017
    // emsali). Kısmen doluysa per-durak korunur. Karar döngü sonunda verilir.
    let mut stp022_pending: Vec<gtfs_core::Notice> = Vec::new();
    let mut any_stop_code_present = false;

    for (row_idx, row) in file.rows.iter().enumerate() {
        let line = (row_idx + 2) as u64;
        let row_map = build_row_map(&file.headers, row);

        let stop_id = get_trimmed_field(&row_map, "stop_id").unwrap_or("").to_string();
        let entity_id = (!stop_id.is_empty()).then_some(stop_id.clone());

        // STP_002: stop_id required (sütun yoksa ARC_025 devralır → atla)
        if get_trimmed_field(&row_map, "stop_id") == Some("") {
            notices.push(make_k2_notice(
                &mut counter, "STP_002", EntityType::Stop, None,
                Some(&row_map), &file.name, Some(line), Some("stop_id"),
                Some(String::new()), None,
                "stop_id zorunludur.".to_string(),
                "stop_id alanını doldurun.",
            ));
        }

        let stop_code = get_trimmed_field(&row_map, "stop_code")
            .filter(|v| !v.is_empty())
            .map(str::to_string);
        if stop_code.is_some() {
            any_stop_code_present = true;
        }

        // STP_028: stop_code çok uzun (>50 karakter)
        if let Some(ref code) = stop_code {
            if code.len() > 50 {
                notices.push(make_k2_notice(
                    &mut counter, "STP_028", EntityType::Stop, entity_id.clone(),
                    Some(&row_map), &file.name, Some(line), Some("stop_code"),
                    Some(code.len().to_string()), Some("≤50".to_string()),
                    format!("'{}' durağının stop_code değeri {} karakter; 50 karakteri aşıyor.", stop_id, code.len()),
                    "stop_code'u 50 karakterin altında tutun.",
                ));
            }
        }

        // location_type: optional, 0-4
        let location_type = match parse_u32(&row_map, "location_type") {
            Ok(v) => {
                if let Some(val) = v {
                    if !validate_enum(&val.to_string(), &["0", "1", "2", "3", "4"]) {
                        notices.push(make_k2_notice(
                            &mut counter, "STP_008", EntityType::Stop, entity_id.clone(),
                            Some(&row_map), &file.name, Some(line), Some("location_type"),
                            Some(val.to_string()), Some("0-4".to_string()),
                            format!("location_type {val} geçerli bir değer değil (0-4 olmalı)."),
                            "location_type alanını 0, 1, 2, 3 veya 4 olarak ayarlayın.",
                        ));
                    }
                }
                v
            }
            Err(_) => None,
        };

        // For location_type 0 (or empty — regular stop) and 1 (station), stop_code önerilir.
        let is_stop_or_station = matches!(location_type, None | Some(0) | Some(1));
        // stop_name/stop_lat/stop_lon: spec'e göre stop (0), station (1) VE entrance/exit (2)
        // için zorunlu; generic node (3) ve boarding area (4) için opsiyonel.
        let requires_name_and_coords = matches!(location_type, None | Some(0) | Some(1) | Some(2));

        // STP_022: stop_code eksik (duraklar ve istasyonlar için) — döngü sonunda
        // agregasyon kararı için bekletilir.
        if stop_code.is_none() && is_stop_or_station {
            stp022_pending.push(make_k2_notice(
                &mut counter, "STP_022", EntityType::Stop, entity_id.clone(),
                Some(&row_map), &file.name, Some(line), Some("stop_code"),
                Some(String::new()), None,
                format!("'{}' durağında stop_code eksik.", stop_id),
                "Yolcuların tanıyabileceği kısa bir stop_code değeri girin.",
            ));
        }

        // stop_name: required for stops/stations/entrances (location_type 0, 1, 2 veya boş)
        let stop_name = get_trimmed_field(&row_map, "stop_name")
            .filter(|v| !v.is_empty())
            .map(str::to_string);
        if stop_name.is_none() && requires_name_and_coords {
            notices.push(make_k2_notice(
                &mut counter, "STP_003", EntityType::Stop, entity_id.clone(),
                Some(&row_map), &file.name, Some(line), Some("stop_name"),
                Some(String::new()), None,
                format!("'{}' durağı için durak adı (stop_name) zorunludur.", stop_id),
                "Okunabilir bir stop_name değeri girin.",
            ));
        }

        // STP_025: stop_name baştaki/sondaki boşluk
        if let Some(raw_name) = get_field(&row_map, "stop_name") {
            if !raw_name.trim().is_empty() && raw_name != raw_name.trim() {
                notices.push(make_k2_notice(
                    &mut counter, "STP_025", EntityType::Stop, entity_id.clone(),
                    Some(&row_map), &file.name, Some(line), Some("stop_name"),
                    Some(raw_name.to_string()), None,
                    format!("'{}' durağının stop_name alanında baştaki/sondaki boşluk var.", stop_id),
                    "stop_name değerinin başındaki ve sonundaki boşlukları kaldırın.",
                ));
            }
        }

        // STP_019: stop_name çok uzun (>100 karakter)
        if let Some(ref name) = stop_name {
            if name.len() > 100 {
                notices.push(make_k2_notice(
                    &mut counter, "STP_019", EntityType::Stop, entity_id.clone(),
                    Some(&row_map), &file.name, Some(line), Some("stop_name"),
                    Some(name.len().to_string()), Some("≤100".to_string()),
                    format!("'{}' durağının stop_name değeri {} karakter; 100 karakteri aşıyor.", stop_id, name.len()),
                    "stop_name'i 100 karakterin altında tutun.",
                ));
            }
        }

        // STP_031: stop_name == stop_desc (same_name_and_description_for_stop)
        let stop_desc = get_trimmed_field(&row_map, "stop_desc")
            .filter(|v| !v.is_empty())
            .map(str::to_string);
        if let (Some(ref name), Some(ref desc)) = (&stop_name, &stop_desc) {
            if name.eq_ignore_ascii_case(desc) {
                notices.push(make_k2_notice(
                    &mut counter, "STP_031", EntityType::Stop, entity_id.clone(),
                    Some(&row_map), &file.name, Some(line), Some("stop_desc"),
                    Some(desc.clone()), None,
                    format!("'{}' durağının stop_name ve stop_desc değerleri aynı: '{}'.", stop_id, name),
                    "stop_desc, stop_name'den farklı ve daha açıklayıcı bir değer içermelidir.",
                ));
            }
        }

        // STP_004: stop_lat required for stops/stations/entrances, must be in [-90, 90]
        let stop_lat = match parse_f64(&row_map, "stop_lat") {
            Ok(v) => {
                match v {
                    None => {
                        if requires_name_and_coords {
                            notices.push(make_k2_notice(
                                &mut counter, "STP_006", EntityType::Stop, entity_id.clone(),
                                Some(&row_map), &file.name, Some(line), Some("stop_lat"),
                                Some(String::new()), Some("[-90, 90]".to_string()),
                                format!("'{}' durağı için enlem (stop_lat) zorunludur.", stop_id),
                                "stop_lat alanını ondalıklı enlem değeriyle doldurun.",
                            ));
                        }
                        None
                    }
                    Some(lat) => {
                        if !(-90.0..=90.0).contains(&lat) {
                            notices.push(make_k2_notice(
                                &mut counter, "STP_003", EntityType::Stop, entity_id.clone(),
                                Some(&row_map), &file.name, Some(line), Some("stop_lat"),
                                Some(lat.to_string()), Some("[-90, 90]".to_string()),
                                format!("stop_lat {lat} değeri [-90, 90] aralığı dışında."),
                                "stop_lat için -90 ile 90 arasında bir değer girin.",
                            ));
                        }
                        Some(lat)
                    }
                }
            }
            Err(err) => {
                notices.push(make_k2_notice(
                    &mut counter, "STP_004", EntityType::Stop, entity_id.clone(),
                    Some(&row_map), &file.name, Some(line), Some("stop_lat"),
                    get_trimmed_field(&row_map, "stop_lat").map(str::to_string),
                    Some("[-90, 90]".to_string()), err,
                    "stop_lat alanını ondalıklı enlem değeriyle doldurun.",
                ));
                None
            }
        };

        // STP_007: stop_lon required for stops/stations/entrances, must be in [-180, 180]
        let stop_lon = match parse_f64(&row_map, "stop_lon") {
            Ok(v) => {
                match v {
                    None => {
                        if requires_name_and_coords {
                            notices.push(make_k2_notice(
                                &mut counter, "STP_007", EntityType::Stop, entity_id.clone(),
                                Some(&row_map), &file.name, Some(line), Some("stop_lon"),
                                Some(String::new()), Some("[-180, 180]".to_string()),
                                format!("'{}' durağı için boylam (stop_lon) zorunludur.", stop_id),
                                "stop_lon alanını ondalıklı boylam değeriyle doldurun.",
                            ));
                        }
                        None
                    }
                    Some(lon) => {
                        if !(-180.0..=180.0).contains(&lon) {
                            notices.push(make_k2_notice(
                                &mut counter, "STP_005", EntityType::Stop, entity_id.clone(),
                                Some(&row_map), &file.name, Some(line), Some("stop_lon"),
                                Some(lon.to_string()), Some("[-180, 180]".to_string()),
                                format!("stop_lon {lon} değeri [-180, 180] aralığı dışında."),
                                "stop_lon için -180 ile 180 arasında bir değer girin.",
                            ));
                        }
                        Some(lon)
                    }
                }
            }
            Err(err) => {
                notices.push(make_k2_notice(
                    &mut counter, "STP_005", EntityType::Stop, entity_id.clone(),
                    Some(&row_map), &file.name, Some(line), Some("stop_lon"),
                    get_trimmed_field(&row_map, "stop_lon").map(str::to_string),
                    Some("[-180, 180]".to_string()), err,
                    "stop_lon alanını ondalıklı boylam değeriyle doldurun.",
                ));
                None
            }
        };

        // stop_timezone: must be valid IANA if provided
        let stop_timezone = get_trimmed_field(&row_map, "stop_timezone")
            .filter(|v| !v.is_empty())
            .map(str::to_string);
        if let Some(ref tz) = stop_timezone {
            if !looks_like_iana_timezone(tz) {
                notices.push(make_k2_notice(
                    &mut counter, "STP_014", EntityType::Stop, entity_id.clone(),
                    Some(&row_map), &file.name, Some(line), Some("stop_timezone"),
                    Some(tz.clone()), None,
                    format!("stop_timezone '{tz}' geçerli bir IANA saat dilimi değil."),
                    "Geçerli bir IANA saat dilimi girin (örn. Europe/Istanbul).",
                ));
            }
        }

        // wheelchair_boarding: must be 0, 1, or 2 if provided
        let wheelchair_boarding = match parse_u32(&row_map, "wheelchair_boarding") {
            Ok(v) => {
                if let Some(val) = v {
                    if !validate_enum(&val.to_string(), &["0", "1", "2"]) {
                        notices.push(make_k2_notice(
                            &mut counter, "STP_013", EntityType::Stop, entity_id.clone(),
                            Some(&row_map), &file.name, Some(line), Some("wheelchair_boarding"),
                            Some(val.to_string()), Some("0, 1 veya 2".to_string()),
                            "wheelchair_boarding 0, 1 veya 2 olmalıdır.".to_string(),
                            "wheelchair_boarding alanını 0 (bilgi yok), 1 (erişilebilir) veya 2 (erişilemez) olarak ayarlayın.",
                        ));
                    }
                }
                v
            }
            Err(_) => None,
        };

        // stop_access: 0, 1, or 2
        let stop_access = match parse_u32(&row_map, "stop_access") {
            Ok(v) => {
                if let Some(val) = v {
                    if !validate_enum(&val.to_string(), &["0", "1", "2"]) {
                        notices.push(make_k2_notice(
                            &mut counter, "STP_024", EntityType::Stop, entity_id.clone(),
                            Some(&row_map), &file.name, Some(line), Some("stop_access"),
                            Some(val.to_string()), Some("0–2".to_string()),
                            format!("'{}' durağında stop_access '{val}' geçersiz.", stop_id),
                            "stop_access'i 0 (bilinmiyor), 1 (erişilebilir) veya 2 (erişilemez) olarak ayarlayın.",
                        ));
                    }
                }
                v
            }
            Err(_) => None,
        };

        let level_id = get_trimmed_field(&row_map, "level_id")
            .filter(|v| !v.is_empty())
            .map(str::to_string);
        let tts_stop_name = get_trimmed_field(&row_map, "tts_stop_name")
            .filter(|v| !v.is_empty())
            .map(str::to_string);

        // STP_023: tts_stop_name işaret karakteri içeriyor (SSML/HTML markup)
        if let Some(ref tts) = tts_stop_name {
            if tts.contains('<') || tts.contains('>') {
                notices.push(make_k2_notice(
                    &mut counter, "STP_023", EntityType::Stop, entity_id.clone(),
                    Some(&row_map), &file.name, Some(line), Some("tts_stop_name"),
                    Some(tts.clone()), None,
                    format!("'{}' durağının tts_stop_name değeri '<' veya '>' karakteri içeriyor.", stop_id),
                    "tts_stop_name'de HTML/SSML işaret karakterleri kullanmayın.",
                ));
            }
        }

        // STP_033: zone_id eksik (stop_without_zone_id) — yalnızca regular stop (location_type=0 veya boş)
        let zone_id = get_trimmed_field(&row_map, "zone_id")
            .filter(|v| !v.is_empty())
            .map(str::to_string);
        let loc_type = location_type.unwrap_or(0);
        if zone_id.is_none() && loc_type == 0 {
            notices.push(make_k2_notice(
                &mut counter, "STP_033", EntityType::Stop, entity_id.clone(),
                Some(&row_map), &file.name, Some(line), Some("zone_id"),
                None, None,
                format!("'{}' durağında zone_id eksik; ücret hesaplamaları etkilenebilir.", stop_id),
                "Ücret hesabı için zone_id alanını doldurun.",
            ));
        }

        let stop_url = get_trimmed_field(&row_map, "stop_url")
            .filter(|v| !v.is_empty())
            .map(str::to_string);

        records.push(StopRecord {
            stop_id,
            stop_code,
            stop_name,
            stop_lat,
            stop_lon,
            location_type,
            stop_timezone,
            wheelchair_boarding,
            stop_access,
            level_id,
            tts_stop_name,
            stop_url,
            row: row_map,
            line,
        });
    }

    // STP_022 karar: stop_code feed genelinde hiç yok ve >1 durak etkili → tek özet;
    // aksi halde (kısmi doluluk veya tek durak) durak-başına notice korunur.
    if !any_stop_code_present && stp022_pending.len() > 1 {
        let n = stp022_pending.len();
        notices.push(make_k2_notice(
            &mut counter, "STP_022", EntityType::Feed, None,
            None, &file.name, None, Some("stop_code"),
            Some(n.to_string()), None,
            format!("Feed genelinde stop_code hiç kullanılmamış: {n} durakta eksik — yolcular durağı kısa kodla tanıyamaz."),
            "stops.txt'teki duraklara yolcuların tanıyabileceği stop_code değerleri ekleyin.",
        ));
    } else {
        notices.append(&mut stp022_pending);
    }

    // STP_018: stops.txt'te hiç durak yok
    if records.is_empty() {
        notices.push(make_k2_notice(
            &mut counter,
            "STP_018",
            EntityType::Stop,
            None,
            None,
            &file.name,
            None,
            None,
            None,
            None,
            "stops.txt dosyasında hiç durak kaydı yok.".to_string(),
            "stops.txt'e en az bir durak kaydı ekleyin.",
        ));
    }

    (records, notices)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::k1_parse::RawFile;

    fn make_file(headers: Vec<&str>, rows: Vec<Vec<&str>>) -> RawFile {
        RawFile {
            name: "stops.txt".to_string(),
            headers: headers.into_iter().map(str::to_string).collect(),
            rows: rows.into_iter().map(|r| r.into_iter().map(smol_str::SmolStr::from).collect()).collect(),
            bytes: 0,
            raw_text: None,
        }
    }

    #[test]
    fn valid_stop_produces_no_notices() {
        let file = make_file(
            vec!["stop_id", "stop_name", "stop_lat", "stop_lon", "stop_code", "zone_id"],
            vec![vec!["S1", "Main Stop", "41.0", "29.0", "S001", "Z1"]],
        );
        let (records, notices) = validate_stops(&file);
        assert_eq!(records.len(), 1);
        assert!(notices.is_empty(), "Geçerli durak notice üretmemeli: {:?}", notices);
    }

    #[test]
    fn stp_022_feed_wide_missing_aggregates_to_single_feed_notice() {
        // Hiçbir durakta stop_code yok, >1 durak → tek feed-seviyesi STP_022.
        let file = make_file(
            vec!["stop_id", "stop_name", "stop_lat", "stop_lon"],
            vec![
                vec!["S1", "Stop1", "41.0", "29.0"],
                vec!["S2", "Stop2", "41.1", "29.1"],
                vec!["S3", "Stop3", "41.2", "29.2"],
            ],
        );
        let (_, notices) = validate_stops(&file);
        let stp022: Vec<_> = notices.iter().filter(|n| n.rule_id == "STP_022").collect();
        assert_eq!(stp022.len(), 1, "feed-geneli eksiklikte tek STP_022 beklenir");
        assert_eq!(stp022[0].entity_type, EntityType::Feed);
        assert_eq!(stp022[0].observed_value.as_deref(), Some("3"));
    }

    #[test]
    fn stp_022_partial_missing_stays_per_stop() {
        // Bazı duraklarda stop_code dolu → eksik olanlar durak-başına işaretlenir.
        let file = make_file(
            vec!["stop_id", "stop_name", "stop_lat", "stop_lon", "stop_code"],
            vec![
                vec!["S1", "Stop1", "41.0", "29.0", "S001"],
                vec!["S2", "Stop2", "41.1", "29.1", ""],
                vec!["S3", "Stop3", "41.2", "29.2", ""],
            ],
        );
        let (_, notices) = validate_stops(&file);
        let stp022: Vec<_> = notices.iter().filter(|n| n.rule_id == "STP_022").collect();
        assert_eq!(stp022.len(), 2, "kısmi doluluk durak-başına STP_022 vermeli");
        assert!(stp022.iter().all(|n| n.entity_type == EntityType::Stop));
    }

    #[test]
    fn lat_out_of_range_produces_stp_003() {
        let file = make_file(
            vec!["stop_id", "stop_name", "stop_lat", "stop_lon"],
            vec![vec!["S1", "Stop1", "999.0", "29.0"]],
        );
        let (_, notices) = validate_stops(&file);
        assert!(notices.iter().any(|n| n.rule_id == "STP_003"),
            "stop_lat=999 should produce STP_003, got: {:?}", notices.iter().map(|n| &n.rule_id).collect::<Vec<_>>());
    }

    #[test]
    fn missing_stop_name_produces_stp_003() {
        let file = make_file(
            vec!["stop_id", "stop_name", "stop_lat", "stop_lon"],
            vec![vec!["S1", "", "41.0", "29.0"]],
        );
        let (_, notices) = validate_stops(&file);
        assert!(notices.iter().any(|n| n.rule_id == "STP_003"));
    }

    #[test]
    fn entrance_missing_name_and_coords_produces_notices() {
        // location_type=2 (giriş/çıkış) için stop_name/stop_lat/stop_lon zorunlu
        let file = make_file(
            vec!["stop_id", "stop_name", "stop_lat", "stop_lon", "location_type"],
            vec![vec!["E1", "", "", "", "2"]],
        );
        let (_, notices) = validate_stops(&file);
        let ids: Vec<&str> = notices.iter().map(|n| n.rule_id.as_str()).collect();
        assert!(ids.contains(&"STP_003"), "stop_name eksik → STP_003 bekleniyor: {:?}", ids);
        assert!(ids.contains(&"STP_006"), "stop_lat eksik → STP_006 bekleniyor: {:?}", ids);
        assert!(ids.contains(&"STP_007"), "stop_lon eksik → STP_007 bekleniyor: {:?}", ids);
    }

    #[test]
    fn generic_node_missing_name_and_coords_silent() {
        // location_type=3 (generic node) için stop_name/stop_lat/stop_lon opsiyonel
        let file = make_file(
            vec!["stop_id", "stop_name", "stop_lat", "stop_lon", "location_type"],
            vec![vec!["N1", "", "", "", "3"]],
        );
        let (_, notices) = validate_stops(&file);
        let ids: Vec<&str> = notices.iter().map(|n| n.rule_id.as_str()).collect();
        assert!(!ids.contains(&"STP_003"), "generic node'da stop_name zorunlu olmamalı: {:?}", ids);
        assert!(!ids.contains(&"STP_006"), "generic node'da stop_lat zorunlu olmamalı: {:?}", ids);
        assert!(!ids.contains(&"STP_007"), "generic node'da stop_lon zorunlu olmamalı: {:?}", ids);
    }

    #[test]
    fn invalid_wheelchair_boarding_produces_stp_013() {
        let file = make_file(
            vec!["stop_id", "stop_name", "stop_lat", "stop_lon", "wheelchair_boarding"],
            vec![vec!["S1", "Stop", "41.0", "29.0", "9"]],
        );
        let (_, notices) = validate_stops(&file);
        assert!(notices.iter().any(|n| n.rule_id == "STP_013"));
    }

    #[test]
    fn invalid_location_type_produces_stp_008() {
        let file = make_file(
            vec!["stop_id", "stop_name", "stop_lat", "stop_lon", "location_type"],
            vec![vec!["S1", "Stop", "41.0", "29.0", "9"]],
        );
        let (_, notices) = validate_stops(&file);
        assert!(notices.iter().any(|n| n.rule_id == "STP_008"));
    }
}
