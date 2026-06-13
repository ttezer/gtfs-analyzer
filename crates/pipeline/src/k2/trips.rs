use gtfs_core::EntityType;
use smol_str::SmolStr;

use super::common::make_k2_notice;
use crate::k1_parse::RawFile;

#[derive(Debug, Clone)]
pub struct TripRecord {
    pub trip_id: String,
    pub route_id: String,
    pub service_id: String,
    pub shape_id: Option<String>,
    pub trip_headsign: Option<String>,
    pub trip_short_name: Option<String>,
    pub direction_id: Option<u32>,
    pub block_id: Option<String>,
    pub wheelchair_accessible: Option<u32>,
    pub bikes_allowed: Option<u32>,
    pub cars_allowed: Option<u32>,
    pub safe_duration_factor: Option<f64>,
    pub safe_duration_offset: Option<u32>,
    /// GTFS-JP: bu seferi işleten ofis (office_jp.office_id'ye referans).
    pub jp_office_id: Option<String>,
    pub line: u64,
}

struct Cols {
    trip_id: Option<usize>,
    route_id: Option<usize>,
    service_id: Option<usize>,
    shape_id: Option<usize>,
    trip_headsign: Option<usize>,
    trip_short_name: Option<usize>,
    direction_id: Option<usize>,
    block_id: Option<usize>,
    wheelchair_accessible: Option<usize>,
    bikes_allowed: Option<usize>,
    cars_allowed: Option<usize>,
    safe_duration_factor: Option<usize>,
    safe_duration_offset: Option<usize>,
    jp_office_id: Option<usize>,
}

impl Cols {
    fn from_headers(headers: &[String]) -> Self {
        let pos = |name: &str| headers.iter().position(|h| h == name);
        Self {
            trip_id:               pos("trip_id"),
            route_id:              pos("route_id"),
            service_id:            pos("service_id"),
            shape_id:              pos("shape_id"),
            trip_headsign:         pos("trip_headsign"),
            trip_short_name:       pos("trip_short_name"),
            direction_id:          pos("direction_id"),
            block_id:              pos("block_id"),
            wheelchair_accessible: pos("wheelchair_accessible"),
            bikes_allowed:         pos("bikes_allowed"),
            cars_allowed:          pos("cars_allowed"),
            safe_duration_factor:  pos("safe_duration_factor"),
            safe_duration_offset:  pos("safe_duration_offset"),
            jp_office_id:          pos("jp_office_id"),
        }
    }
}

#[inline]
fn get_col<'a>(row: &'a [SmolStr], col: Option<usize>) -> &'a str {
    col.and_then(|i| row.get(i)).map(|s| s.as_str().trim()).unwrap_or("")
}

fn parse_u32_raw(raw: &str) -> Result<Option<u32>, ()> {
    if raw.is_empty() { return Ok(None); }
    raw.parse::<u32>().map(Some).map_err(|_| ())
}

fn parse_f64_raw(raw: &str) -> Result<Option<f64>, ()> {
    if raw.is_empty() { return Ok(None); }
    raw.parse::<f64>().map(Some).map_err(|_| ())
}

pub fn validate_trips(file: &RawFile) -> (Vec<TripRecord>, Vec<gtfs_core::Notice>) {
    let mut notices = Vec::new();
    let mut records = Vec::new();
    let mut counter = 0u32;

    let cols = Cols::from_headers(&file.headers);
    // TRP_021: feed genelinde bikes_allowed kullanımını takip et.
    // Eksik seferleri tek tek bildirmek yerine sayar + birkaç örnek trip_id toplar;
    // loop sonrası tek özet notice üretilir (TRP_028/029 deseni).
    let mut trp021_missing_count: usize = 0;
    let mut trp021_missing_examples: Vec<String> = Vec::new();
    let mut trp021_first_line: Option<u64> = None;
    let mut bikes_allowed_set_count: u32 = 0;

    for (row_idx, row) in file.rows.iter().enumerate() {
        let line = (row_idx + 2) as u64;

        let trip_id = get_col(row, cols.trip_id).to_string();
        let entity_id = (!trip_id.is_empty()).then(|| trip_id.clone());

        // TRP_001: trip_id zorunlu (sütun yoksa ARC_025 devralır → atla)
        if trip_id.is_empty() && file.headers.iter().any(|h| h == "trip_id") {
            notices.push(make_k2_notice(
                &mut counter, "TRP_001", EntityType::Trip, None,
                None, &file.name, Some(line), Some("trip_id"),
                Some(String::new()), None,
                "trip_id zorunludur.".to_string(),
                "Her sefere benzersiz bir trip_id atayın.",
            ));
        }

        let route_id = get_col(row, cols.route_id).to_string();
        let service_id = get_col(row, cols.service_id).to_string();

        // TRP_031: route_id required (sütun yoksa ARC_025 devralır → atla)
        if route_id.is_empty() && file.headers.iter().any(|h| h == "route_id") {
            notices.push(make_k2_notice(
                &mut counter, "TRP_031", EntityType::Trip, None,
                None, &file.name, Some(line), Some("route_id"),
                Some(String::new()), None,
                "route_id zorunludur.".to_string(),
                "Her sefere geçerli bir route_id atayın.",
            ));
        }

        let shape_id = {
            let v = get_col(row, cols.shape_id);
            if v.is_empty() { None } else { Some(v.to_string()) }
        };
        let trip_headsign = {
            let v = get_col(row, cols.trip_headsign);
            if v.is_empty() { None } else { Some(v.to_string()) }
        };
        let trip_short_name = {
            let v = get_col(row, cols.trip_short_name);
            if v.is_empty() { None } else { Some(v.to_string()) }
        };

        // TRP_014: trip_short_name çok uzun (>20 karakter)
        if let Some(ref sn) = trip_short_name {
            if sn.len() > 20 {
                notices.push(make_k2_notice(
                    &mut counter, "TRP_014", EntityType::Trip, entity_id.clone(),
                    None, &file.name, Some(line), Some("trip_short_name"),
                    Some(sn.len().to_string()), Some("≤20".to_string()),
                    format!("trip_short_name {} karakter; 20'yi aşmamalıdır.", sn.len()),
                    "trip_short_name'i kısaltın.",
                ));
            }
        }

        let block_id = {
            let v = get_col(row, cols.block_id);
            if v.is_empty() { None } else { Some(v.to_string()) }
        };

        // TRP_005: direction_id must be 0 or 1 if provided
        let dir_raw = get_col(row, cols.direction_id);
        let direction_id = match parse_u32_raw(dir_raw) {
            Ok(v) => {
                if let Some(val) = v {
                    if val > 1 {
                        notices.push(make_k2_notice(
                            &mut counter, "TRP_005", EntityType::Trip, entity_id.clone(),
                            None, &file.name, Some(line), Some("direction_id"),
                            Some(val.to_string()), Some("0 veya 1".to_string()),
                            format!("direction_id {val} geçersiz; 0 veya 1 olmalıdır."),
                            "direction_id değerini 0 (gidiş) veya 1 (dönüş) olarak ayarlayın.",
                        ));
                    }
                }
                v
            }
            Err(_) => {
                notices.push(make_k2_notice(
                    &mut counter, "TRP_005", EntityType::Trip, entity_id.clone(),
                    None, &file.name, Some(line), Some("direction_id"),
                    Some(dir_raw.to_string()), Some("0 veya 1".to_string()),
                    format!("direction_id '{dir_raw}' geçersiz; 0 veya 1 olmalıdır."),
                    "direction_id değerini 0 (gidiş) veya 1 (dönüş) olarak ayarlayın.",
                ));
                None
            }
        };

        // TRP_006: wheelchair_accessible 0, 1 veya 2 olmalı
        let wc_raw = get_col(row, cols.wheelchair_accessible);
        let wheelchair_accessible = match parse_u32_raw(wc_raw) {
            Ok(v) => {
                if let Some(val) = v {
                    if val > 2 {
                        notices.push(make_k2_notice(
                            &mut counter, "TRP_006", EntityType::Trip, entity_id.clone(),
                            None, &file.name, Some(line), Some("wheelchair_accessible"),
                            Some(val.to_string()), Some("0, 1 veya 2".to_string()),
                            "wheelchair_accessible 0, 1 veya 2 olmalıdır.".to_string(),
                            "wheelchair_accessible değerini 0 (bilgi yok), 1 (erişilebilir) veya 2 (erişilemez) olarak ayarlayın.",
                        ));
                    }
                }
                v
            }
            Err(_) => None,
        };

        // TRP_007: bikes_allowed 0, 1 veya 2 olmalı
        let ba_raw = get_col(row, cols.bikes_allowed);
        let bikes_allowed = match parse_u32_raw(ba_raw) {
            Ok(v) => {
                if let Some(val) = v {
                    if val > 2 {
                        notices.push(make_k2_notice(
                            &mut counter, "TRP_007", EntityType::Trip, entity_id.clone(),
                            None, &file.name, Some(line), Some("bikes_allowed"),
                            Some(val.to_string()), Some("0, 1 veya 2".to_string()),
                            "bikes_allowed 0, 1 veya 2 olmalıdır.".to_string(),
                            "bikes_allowed değerini 0 (bilgi yok), 1 (izinli) veya 2 (izinsiz) olarak ayarlayın.",
                        ));
                    }
                }
                v
            }
            Err(_) => None,
        };

        // TRP_021: bikes_allowed kullanım istatistiği — özet bildirim loop sonrası yapılır
        if bikes_allowed.is_none() && ba_raw.is_empty() {
            trp021_missing_count += 1;
            if trp021_first_line.is_none() {
                trp021_first_line = Some(line);
            }
            if trp021_missing_examples.len() < 5 && !trip_id.is_empty() {
                trp021_missing_examples.push(trip_id.clone());
            }
        } else if bikes_allowed.is_some() {
            bikes_allowed_set_count += 1;
        }

        // TRP_032: cars_allowed 0, 1 veya 2 olmalı (TRP_007 bikes_allowed ikizi)
        let ca_raw = get_col(row, cols.cars_allowed);
        let cars_allowed = match parse_u32_raw(ca_raw) {
            Ok(v) => {
                if let Some(val) = v {
                    if val > 2 {
                        notices.push(make_k2_notice(
                            &mut counter, "TRP_032", EntityType::Trip, entity_id.clone(),
                            None, &file.name, Some(line), Some("cars_allowed"),
                            Some(val.to_string()), Some("0, 1 veya 2".to_string()),
                            "cars_allowed 0, 1 veya 2 olmalıdır.".to_string(),
                            "cars_allowed değerini 0 (bilgi yok), 1 (araç izinli) veya 2 (araç izinsiz) olarak ayarlayın.",
                        ));
                    }
                }
                v
            }
            Err(_) => None,
        };
        let safe_duration_factor = parse_f64_raw(get_col(row, cols.safe_duration_factor)).ok().flatten();
        let safe_duration_offset = parse_u32_raw(get_col(row, cols.safe_duration_offset)).ok().flatten();

        let jp_office_id = {
            let v = get_col(row, cols.jp_office_id);
            if v.is_empty() { None } else { Some(v.to_string()) }
        };

        records.push(TripRecord {
            trip_id,
            route_id,
            service_id,
            shape_id,
            trip_headsign,
            trip_short_name,
            direction_id,
            block_id,
            wheelchair_accessible,
            bikes_allowed,
            cars_allowed,
            safe_duration_factor,
            safe_duration_offset,
            jp_office_id,
            line,
        });
    }

    // TRP_021: bikes_allowed eksik seferler — her satır için ayrı notice yerine tek özet notice
    // (TRP_028/029 deseni). Hiç set edilmemişse feed genelinde eksiklik; bazıları set etmişse
    // kalanlar tutarsızlık olarak örneklerle özetlenir.
    if trp021_missing_count > 0 {
        let total = trp021_missing_count + bikes_allowed_set_count as usize;
        if bikes_allowed_set_count == 0 {
            // Feed genelinde alan hiç doldurulmamış — tek özet yeterli
            notices.push(make_k2_notice(
                &mut counter, "TRP_021", EntityType::Trip, None,
                None, &file.name, None, Some("bikes_allowed"),
                Some(format!("{trp021_missing_count}/{total} sefer")), Some("0".to_string()),
                format!("Bu feed'de bikes_allowed alanı hiçbir seferde belirtilmemiş ({trp021_missing_count} sefer)."),
                "bikes_allowed değerini 0 (bilgi yok), 1 (bisiklet izinli) veya 2 (bisiklet izinsiz) olarak ayarlayın.",
            ));
        } else {
            // Bazı seferler set etmiş — eksik olanları tek özette örneklerle bildir (tutarsızlık)
            let examples = if trp021_missing_examples.is_empty() {
                String::new()
            } else {
                format!(" Örnek seferler: {}{}.",
                    trp021_missing_examples.join(", "),
                    if trp021_missing_count > trp021_missing_examples.len() { ", …" } else { "" })
            };
            notices.push(make_k2_notice(
                &mut counter, "TRP_021", EntityType::Trip, None,
                None, &file.name, trp021_first_line, Some("bikes_allowed"),
                Some(format!("{trp021_missing_count}/{total} sefer")), Some("0".to_string()),
                format!("{total} seferin {trp021_missing_count} tanesinde bikes_allowed belirtilmemiş ({:.0}%).{examples}",
                    trp021_missing_count as f64 / total as f64 * 100.0),
                "Tüm seferlerin bikes_allowed alanını 0 (bilgi yok), 1 (bisiklet izinli) veya 2 (bisiklet izinsiz) olarak doldurun.",
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
            name: "trips.txt".to_string(),
            headers: headers.into_iter().map(str::to_string).collect(),
            rows: rows.into_iter().map(|r| r.into_iter().map(smol_str::SmolStr::from).collect()).collect(),
            bytes: 0,
            raw_text: None,
        }
    }

    #[test]
    fn valid_trip_produces_no_notices() {
        let file = make_file(
            vec!["route_id", "service_id", "trip_id", "bikes_allowed"],
            vec![vec!["R1", "SVC1", "T1", "0"]],
        );
        let (records, notices) = validate_trips(&file);
        assert_eq!(records.len(), 1);
        assert!(notices.is_empty(), "Geçerli sefer notice üretmemeli: {:?}", notices);
    }

    #[test]
    fn invalid_direction_id_produces_trp_005() {
        let file = make_file(
            vec!["route_id", "service_id", "trip_id", "direction_id"],
            vec![vec!["R1", "WKD", "T1", "9"]],
        );
        let (_, notices) = validate_trips(&file);
        assert!(notices.iter().any(|n| n.rule_id == "TRP_005"),
            "direction_id=9 must produce TRP_005, got: {:?}", notices.iter().map(|n| &n.rule_id).collect::<Vec<_>>());
    }

    #[test]
    fn invalid_wheelchair_accessible_produces_trp_006() {
        let file = make_file(
            vec!["route_id", "service_id", "trip_id", "wheelchair_accessible"],
            vec![vec!["R1", "SVC1", "T1", "5"]],
        );
        let (_, notices) = validate_trips(&file);
        assert!(notices.iter().any(|n| n.rule_id == "TRP_006"));
    }

    #[test]
    fn invalid_cars_allowed_produces_trp_032() {
        let file = make_file(
            vec!["route_id", "service_id", "trip_id", "cars_allowed"],
            vec![vec!["R1", "SVC1", "T1", "5"]],
        );
        let (_, notices) = validate_trips(&file);
        assert!(notices.iter().any(|n| n.rule_id == "TRP_032"));
    }

    #[test]
    fn valid_cars_allowed_produces_no_trp_032() {
        let file = make_file(
            vec!["route_id", "service_id", "trip_id", "cars_allowed"],
            vec![vec!["R1", "SVC1", "T1", "2"]],
        );
        let (_, notices) = validate_trips(&file);
        assert!(!notices.iter().any(|n| n.rule_id == "TRP_032"));
    }

    #[test]
    fn valid_direction_id_zero_produces_no_notice() {
        let file = make_file(
            vec!["route_id", "service_id", "trip_id", "direction_id"],
            vec![vec!["R1", "SVC1", "T1", "0"]],
        );
        let (_, notices) = validate_trips(&file);
        assert!(!notices.iter().any(|n| n.rule_id == "TRP_005"));
    }
}
