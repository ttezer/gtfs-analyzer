use std::collections::{HashMap, HashSet};

use gtfs_core::EntityType;
use smol_str::SmolStr;

use super::common::make_k2_notice;
use crate::k1_parse::RawFile;

#[derive(Debug, Clone)]
pub struct ShapePointRecord {
    pub shape_id: String,
    pub shape_pt_lat: Option<f64>,
    pub shape_pt_lon: Option<f64>,
    pub shape_pt_sequence: Option<u32>,
    pub shape_dist_traveled: Option<f64>,
    pub line: u64,
}

struct Cols {
    shape_id: Option<usize>,
    shape_pt_lat: Option<usize>,
    shape_pt_lon: Option<usize>,
    shape_pt_sequence: Option<usize>,
    shape_dist_traveled: Option<usize>,
}

impl Cols {
    fn from_headers(headers: &[String]) -> Self {
        let pos = |name: &str| headers.iter().position(|h| h == name);
        Self {
            shape_id:           pos("shape_id"),
            shape_pt_lat:       pos("shape_pt_lat"),
            shape_pt_lon:       pos("shape_pt_lon"),
            shape_pt_sequence:  pos("shape_pt_sequence"),
            shape_dist_traveled: pos("shape_dist_traveled"),
        }
    }
}

#[inline]
fn get_col<'a>(row: &'a [SmolStr], col: Option<usize>) -> &'a str {
    col.and_then(|i| row.get(i)).map(|s| s.as_str().trim()).unwrap_or("")
}

fn parse_f64_raw(raw: &str) -> Result<Option<f64>, ()> {
    if raw.is_empty() { return Ok(None); }
    raw.parse::<f64>().map(Some).map_err(|_| ())
}

fn parse_u32_raw(raw: &str) -> Result<Option<u32>, ()> {
    if raw.is_empty() { return Ok(None); }
    raw.parse::<u32>().map(Some).map_err(|_| ())
}

pub fn validate_shapes(file: &RawFile) -> (Vec<ShapePointRecord>, Vec<gtfs_core::Notice>) {
    let mut notices = Vec::new();
    let mut records = Vec::new();
    let mut counter = 0u32;

    let cols = Cols::from_headers(&file.headers);

    let mut seen_seq_by_shape: HashMap<String, HashSet<u32>> = HashMap::new();
    let mut prev_dist_by_shape: HashMap<String, f64> = HashMap::new();

    for (row_idx, row) in file.rows.iter().enumerate() {
        let line = (row_idx + 2) as u64;

        let shape_id_raw = get_col(row, cols.shape_id);
        let shape_id = shape_id_raw.to_string();
        let entity_id = (!shape_id.is_empty()).then_some(shape_id.clone());

        // SHP_001: shape_id required
        if shape_id.is_empty() {
            notices.push(make_k2_notice(
                &mut counter, "SHP_001", EntityType::Shape, None,
                None, &file.name, Some(line), Some("shape_id"),
                Some(String::new()), None,
                "shape_id zorunludur.".to_string(),
                "shape_id alanını doldurun.",
            ));
        }

        // SHP_004: shape_pt_sequence required
        let seq_raw = get_col(row, cols.shape_pt_sequence);
        let shape_pt_sequence = match parse_u32_raw(seq_raw) {
            Ok(v) => {
                if v.is_none() {
                    notices.push(make_k2_notice(
                        &mut counter, "SHP_004", EntityType::Shape, entity_id.clone(),
                        None, &file.name, Some(line), Some("shape_pt_sequence"),
                        Some(String::new()), None,
                        "shape_pt_sequence zorunludur.".to_string(),
                        "shape_pt_sequence negatif olmayan bir tam sayı olarak girin.",
                    ));
                }
                v
            }
            Err(_) => {
                notices.push(make_k2_notice(
                    &mut counter, "SHP_004", EntityType::Shape, entity_id.clone(),
                    None, &file.name, Some(line), Some("shape_pt_sequence"),
                    Some(seq_raw.to_string()), None,
                    format!("shape_pt_sequence '{seq_raw}' geçersiz."),
                    "shape_pt_sequence negatif olmayan bir tam sayı olarak girin.",
                ));
                None
            }
        };

        // SHP_008: shape_pt_sequence yineleniyor
        if let Some(seq) = shape_pt_sequence {
            if !seen_seq_by_shape.entry(shape_id.clone()).or_default().insert(seq) {
                notices.push(make_k2_notice(
                    &mut counter, "SHP_008", EntityType::Shape, entity_id.clone(),
                    None, &file.name, Some(line), Some("shape_pt_sequence"),
                    Some(seq.to_string()), None,
                    format!("shape_id '{shape_id}' için shape_pt_sequence {seq} yineleniyor."),
                    "Her shape noktasına benzersiz bir shape_pt_sequence atayın.",
                ));
            }
        }

        // SHP_002: shape_pt_lat required, in range [-90, 90]
        let lat_raw = get_col(row, cols.shape_pt_lat);
        let shape_pt_lat = match parse_f64_raw(lat_raw) {
            Ok(None) => {
                notices.push(make_k2_notice(
                    &mut counter, "SHP_002", EntityType::Shape, entity_id.clone(),
                    None, &file.name, Some(line), Some("shape_pt_lat"),
                    Some(String::new()), Some("[-90, 90]".to_string()),
                    "shape_pt_lat zorunludur.".to_string(),
                    "shape_pt_lat alanını ondalıklı enlem değeriyle doldurun.",
                ));
                None
            }
            Ok(Some(lat)) => {
                if !(-90.0..=90.0).contains(&lat) {
                    notices.push(make_k2_notice(
                        &mut counter, "SHP_002", EntityType::Shape, entity_id.clone(),
                        None, &file.name, Some(line), Some("shape_pt_lat"),
                        Some(lat.to_string()), Some("[-90, 90]".to_string()),
                        format!("shape_pt_lat {lat} değeri [-90, 90] aralığı dışında."),
                        "shape_pt_lat için -90 ile 90 arasında bir değer girin.",
                    ));
                }
                Some(lat)
            }
            Err(_) => {
                notices.push(make_k2_notice(
                    &mut counter, "SHP_002", EntityType::Shape, entity_id.clone(),
                    None, &file.name, Some(line), Some("shape_pt_lat"),
                    Some(lat_raw.to_string()), Some("[-90, 90]".to_string()),
                    format!("shape_pt_lat '{lat_raw}' geçersiz."),
                    "shape_pt_lat alanını ondalıklı enlem değeriyle doldurun.",
                ));
                None
            }
        };

        // SHP_003: shape_pt_lon required, in range [-180, 180]
        let lon_raw = get_col(row, cols.shape_pt_lon);
        let shape_pt_lon = match parse_f64_raw(lon_raw) {
            Ok(None) => {
                notices.push(make_k2_notice(
                    &mut counter, "SHP_003", EntityType::Shape, entity_id.clone(),
                    None, &file.name, Some(line), Some("shape_pt_lon"),
                    Some(String::new()), Some("[-180, 180]".to_string()),
                    "shape_pt_lon zorunludur.".to_string(),
                    "shape_pt_lon alanını ondalıklı boylam değeriyle doldurun.",
                ));
                None
            }
            Ok(Some(lon)) => {
                if !(-180.0..=180.0).contains(&lon) {
                    notices.push(make_k2_notice(
                        &mut counter, "SHP_003", EntityType::Shape, entity_id.clone(),
                        None, &file.name, Some(line), Some("shape_pt_lon"),
                        Some(lon.to_string()), Some("[-180, 180]".to_string()),
                        format!("shape_pt_lon {lon} değeri [-180, 180] aralığı dışında."),
                        "shape_pt_lon için -180 ile 180 arasında bir değer girin.",
                    ));
                }
                Some(lon)
            }
            Err(_) => {
                notices.push(make_k2_notice(
                    &mut counter, "SHP_003", EntityType::Shape, entity_id.clone(),
                    None, &file.name, Some(line), Some("shape_pt_lon"),
                    Some(lon_raw.to_string()), Some("[-180, 180]".to_string()),
                    format!("shape_pt_lon '{lon_raw}' geçersiz."),
                    "shape_pt_lon alanını ondalıklı boylam değeriyle doldurun.",
                ));
                None
            }
        };

        // shape_dist_traveled: optional, non-negative
        let sdt_raw = get_col(row, cols.shape_dist_traveled);
        let shape_dist_traveled = match parse_f64_raw(sdt_raw) {
            Ok(v) => {
                if let Some(d) = v {
                    if d < 0.0 {
                        notices.push(make_k2_notice(
                            &mut counter, "SHP_021", EntityType::Shape, entity_id.clone(),
                            None, &file.name, Some(line), Some("shape_dist_traveled"),
                            Some(d.to_string()), Some(">= 0".to_string()),
                            "shape_dist_traveled negatif olamaz.".to_string(),
                            "shape_dist_traveled alanını sıfır veya pozitif bir değere ayarlayın.",
                        ));
                    }
                }
                v
            }
            Err(_) => None,
        };

        // SHP_005: shape_dist_traveled geriye gidiyor
        if let Some(d) = shape_dist_traveled {
            if let Some(&prev) = prev_dist_by_shape.get(&shape_id) {
                if d < prev - 1e-6 {
                    notices.push(make_k2_notice(
                        &mut counter, "SHP_005", EntityType::Shape, entity_id.clone(),
                        None, &file.name, Some(line), Some("shape_dist_traveled"),
                        Some(format!("{d}")), Some(format!("≥ {prev} (önceki değer)")),
                        format!("shape_id '{shape_id}' için shape_dist_traveled azalıyor: {prev} → {d}."),
                        "shape_dist_traveled değerlerini her shape için artan sırada yazın.",
                    ));
                }
            }
            prev_dist_by_shape.insert(shape_id.clone(), d);
        }

        records.push(ShapePointRecord {
            shape_id,
            shape_pt_lat,
            shape_pt_lon,
            shape_pt_sequence,
            shape_dist_traveled,
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
            name: "shapes.txt".to_string(),
            headers: headers.into_iter().map(str::to_string).collect(),
            rows: rows.into_iter().map(|r| r.into_iter().map(smol_str::SmolStr::from).collect()).collect(),
            bytes: 0,
            raw_text: None,
        }
    }

    #[test]
    fn valid_shape_point_produces_no_notices() {
        let file = make_file(
            vec!["shape_id", "shape_pt_lat", "shape_pt_lon", "shape_pt_sequence"],
            vec![vec!["SHP1", "41.0", "29.0", "1"]],
        );
        let (records, notices) = validate_shapes(&file);
        assert_eq!(records.len(), 1);
        assert!(notices.is_empty(), "Geçerli shape noktası notice üretmemeli: {:?}", notices);
    }

    #[test]
    fn lat_out_of_range_produces_shp_002() {
        let file = make_file(
            vec!["shape_id", "shape_pt_lat", "shape_pt_lon", "shape_pt_sequence"],
            vec![vec!["SHP1", "999.0", "29.0", "1"]],
        );
        let (_, notices) = validate_shapes(&file);
        assert!(notices.iter().any(|n| n.rule_id == "SHP_002"));
    }

    #[test]
    fn lon_out_of_range_produces_shp_003() {
        let file = make_file(
            vec!["shape_id", "shape_pt_lat", "shape_pt_lon", "shape_pt_sequence"],
            vec![vec!["SHP1", "41.0", "999.0", "1"]],
        );
        let (_, notices) = validate_shapes(&file);
        assert!(notices.iter().any(|n| n.rule_id == "SHP_003"));
    }

    #[test]
    fn missing_sequence_produces_shp_004() {
        let file = make_file(
            vec!["shape_id", "shape_pt_lat", "shape_pt_lon", "shape_pt_sequence"],
            vec![vec!["SHP1", "41.0", "29.0", ""]],
        );
        let (_, notices) = validate_shapes(&file);
        assert!(notices.iter().any(|n| n.rule_id == "SHP_004"));
    }
}
