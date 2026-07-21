use std::borrow::Cow;
use std::collections::{HashMap, HashSet};

use gtfs_core::{EntityType, Severity};

use super::common::make_k2_notice;
use super::stop_times::next_csv_record;
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
fn get_col<'a>(row: &'a [Cow<'_, str>], col: Option<usize>) -> &'a str {
    col.and_then(|i| row.get(i)).map(|s| s.as_ref().trim()).unwrap_or("")
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
    // #15 W3: shapes.txt K1'de stream edilir (RawFile.rows boş, gövde raw_text'te). Burada
    // streaming parse edilir; K1'in per-satır generic notice'ları (ARC_012/018/021, DQ_016)
    // bu geçişe taşındı (stop_times deseni — aynı line/severity/rule davranışı). Eski rows
    // yolu test/legacy fallback olarak korunur.
    let mut notices = Vec::new();
    // #15: records Vec'ini önceden boyutlandır — doubling realloc'un geçici 2× peak'i
    // shape-ağır feed'lerde ~500 MB tepe yaratıyordu (K1 stream'i düşünce açığa çıktı).
    // Satır ~30 bayt (shape_id,lat,lon,seq[,dist]) → muhafazakâr sayı tahmini. Yalnız KAPASİTE.
    let est_rows = file.raw_text.as_ref().map(|t| t.len() / 30).unwrap_or(file.rows.len());
    let mut records: Vec<ShapePointRecord> = Vec::with_capacity(est_rows);
    let mut counter = 0u32;

    let cols = Cols::from_headers(&file.headers);
    let header_count = file.headers.len();
    let mut arc021_fired = false;
    let mut seen_seq_by_shape: HashMap<String, HashSet<u32>> = HashMap::new();
    let mut total_rows: usize = 0;
    let mut dq016 = crate::k1_parse::Dq016Acc::default();

    {
        // Satır işleyici — hem stream (raw_text) hem rows fallback yolundan çağrılır.
        let mut process = |row: &[Cow<'_, str>], line: u64| {
            // ── Taşınan generic dosya/satır-seviye notice'lar (K1'den) ──
            // ARC_012: sütun sayısı tutarsız
            if row.len() != header_count {
                let missing = header_count.saturating_sub(row.len());
                let (msg, tip) = if row.len() < header_count {
                    (
                        format!("'{}' {line}. satırda sondaki {missing} isteğe bağlı alan atlanmış ({} sütun, başlık: {header_count}).", file.name, row.len()),
                        "CSV'de sondaki boş alanlar atlanabilir; zorunlu alanlar boş bırakılmamalıdır.",
                    )
                } else {
                    (
                        format!("'{}' {line}. satırda fazla alan: {} (beklenen {header_count}) — kaçmamış virgül veya format hatası.", file.name, row.len()),
                        "Her satırın başlık sayısı kadar virgülle ayrılmış değer içerdiğinden emin olun.",
                    )
                };
                let mut n = make_k2_notice(
                    &mut counter, "ARC_012", EntityType::File, Some(file.name.clone()),
                    None, &file.name, Some(line), None,
                    Some(format!("{} sütun (beklenen {header_count})", row.len())), None,
                    msg, tip,
                );
                if row.len() < header_count {
                    n.severity = Severity::Bilgi;
                }
                notices.push(n);
            }

            // ARC_018: tamamen boş satır → notice + kayıt YAPMA
            if row.iter().all(|v| v.trim().is_empty()) {
                notices.push(make_k2_notice(
                    &mut counter, "ARC_018", EntityType::File, Some(file.name.clone()),
                    None, &file.name, Some(line), None,
                    None, None,
                    format!("'{}' {line}. satırı tamamen boş.", file.name),
                    "Boş satırları kaldırın.",
                ));
                return;
            }
            total_rows += 1;

            // ARC_021: yazdırılamaz/sorunlu karakter — dosya başına bir kez
            if !arc021_fired {
                'arc021: for val in row.iter() {
                    for ch in val.chars() {
                        let cp = ch as u32;
                        if ch.is_alphanumeric() || ch.is_whitespace() {
                            continue;
                        }
                        let is_bad = (cp < 32 && cp != 9)
                            || cp == 127
                            || (0xD800..=0xDFFF).contains(&cp)
                            || (0xE000..=0xF8FF).contains(&cp)
                            || (0xFFF0..=0xFFFF).contains(&cp);
                        if is_bad {
                            arc021_fired = true;
                            notices.push(make_k2_notice(
                                &mut counter, "ARC_021", EntityType::File, Some(file.name.clone()),
                                None, &file.name, Some(line), None,
                                Some(format!("U+{cp:04X}")), None,
                                format!("'{}' dosyasında ASCII dışı veya yazdırılamaz karakter içeren değer var (U+{cp:04X}).", file.name),
                                "Tüm alan değerlerinin yazdırılabilir ASCII karakter içerdiğinden emin olun.",
                            ));
                            break 'arc021;
                        }
                    }
                }
            }

            // DQ_016: değerlerde baştaki/sondaki boşluk
            // DQ_016: dosya-seviyesi birikim; emit döngü sonrası TEK özet (patlama önlemi).
            dq016.observe(line, row.iter().map(|v| v.as_ref()), &file.headers);

            // ── Shape-özel kurallar ──
            let shape_id = get_col(row, cols.shape_id).to_string();
            let entity_id = (!shape_id.is_empty()).then_some(shape_id.clone());

            // SHP_001: shape_id required (sütun yoksa ARC_025 devralır → atla)
            if shape_id.is_empty() && file.headers.iter().any(|h| h == "shape_id") {
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
                    if v.is_none() && file.headers.iter().any(|h| h == "shape_pt_sequence") {
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
                    if file.headers.iter().any(|h| h == "shape_pt_lat") {
                        notices.push(make_k2_notice(
                            &mut counter, "SHP_002", EntityType::Shape, entity_id.clone(),
                            None, &file.name, Some(line), Some("shape_pt_lat"),
                            Some(String::new()), Some("[-90, 90]".to_string()),
                            "shape_pt_lat zorunludur.".to_string(),
                            "shape_pt_lat alanını ondalıklı enlem değeriyle doldurun.",
                        ));
                    }
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
                    if file.headers.iter().any(|h| h == "shape_pt_lon") {
                        notices.push(make_k2_notice(
                            &mut counter, "SHP_003", EntityType::Shape, entity_id.clone(),
                            None, &file.name, Some(line), Some("shape_pt_lon"),
                            Some(String::new()), Some("[-180, 180]".to_string()),
                            "shape_pt_lon zorunludur.".to_string(),
                            "shape_pt_lon alanını ondalıklı boylam değeriyle doldurun.",
                        ));
                    }
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

            // shape_dist_traveled: optional, non-negative (SHP_021)
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

            // SHP_005 (shape_dist_traveled monotonluğu) K5'e taşındı: dosya satır sırası DEĞİL,
            // shape_pt_sequence-sıralı noktalar üzerinde kontrol edilir. Dosya sırası ≠ sequence
            // olan geçerli feed'lerde eski dosya-sırası kontrolü sahte "azalma" (FP) üretiyordu.

            records.push(ShapePointRecord {
                shape_id,
                shape_pt_lat,
                shape_pt_lon,
                shape_pt_sequence,
                shape_dist_traveled,
                line,
            });
        };

        // ── Sürücü: stream raw_text (başlığı atla) veya rows fallback ──
        if let Some(text) = &file.raw_text {
            let mut pos = 0usize;
            let mut buf: Vec<Cow<'_, str>> = Vec::with_capacity(8);
            let mut data_idx = 0usize;
            let mut header_skipped = false;
            while next_csv_record(text, &mut pos, &mut buf) {
                if buf.len() == 1 && buf[0].is_empty() {
                    continue;
                }
                if !header_skipped {
                    header_skipped = true;
                    continue;
                }
                process(&buf, (data_idx + 2) as u64);
                data_idx += 1;
            }
        } else {
            // Fallback (test/legacy): önceden parse edilmiş rows üzerinden.
            for (row_idx, row) in file.rows.iter().enumerate() {
                let cow_row: Vec<Cow<'_, str>> =
                    row.iter().map(|s| Cow::Borrowed(s.as_str())).collect();
                process(&cow_row, (row_idx + 2) as u64);
            }
        }
    }

    // ARC_022: satır sayısı limiti (stream_mode'da K1'den taşındı)
    const MAX_ROWS: usize = 1_000_000;
    if total_rows > MAX_ROWS {
        notices.push(make_k2_notice(
            &mut counter, "ARC_022", EntityType::File, Some(file.name.clone()),
            None, &file.name, None, None,
            Some(format!("{total_rows}")), None,
            format!("'{}' dosyasında {total_rows} satır var; {MAX_ROWS} satır sınırını aşıyor.", file.name),
            "Dosyayı küçük parçalara bölün veya gereksiz satırları kaldırın.",
        ));
    }

    // ARC_009: başlık var ama veri satırı yok (stream_mode'da K1'den taşındı). shapes
    // OPSİYONEL → Bilgi (arc009_critical(shapes)=false ile aynı davranış).
    if file.raw_text.is_some() && total_rows == 0 {
        let mut n = make_k2_notice(
            &mut counter, "ARC_009", EntityType::File, Some(file.name.clone()),
            None, &file.name, None, None,
            None, None,
            format!("'{}' dosyasında başlık satırı var ama veri satırı yok.", file.name),
            "Dosyaya en az bir veri satırı ekleyin (veya gereksizse dosyayı kaldırın).",
        );
        n.severity = Severity::Bilgi;
        notices.push(n);
    }

    // DQ_016: DOSYA başına TEK özet (satır-başına değil — patlama önlemi).
    if let Some((observed, msg, cols)) = dq016.summary(&file.name) {
        notices.push(make_k2_notice(
            &mut counter, "DQ_016", EntityType::File, Some(file.name.clone()),
            None, &file.name, dq016.first_line, Some(cols.as_str()),
            Some(observed), None, msg, crate::k1_parse::DQ016_REMEDIATION,
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

    #[test]
    fn streaming_raw_text_matches_rows_path() {
        // #15 W3: production'da shapes raw_text üzerinden stream edilir. Aynı veri için
        // raw_text yolu ile rows fallback yolu BİREBİR aynı record/line/notice üretmeli.
        let headers = vec!["shape_id", "shape_pt_lat", "shape_pt_lon", "shape_pt_sequence"];
        let rows = vec![
            vec!["S1", "41.0", "29.0", "1"],
            vec!["S1", "999.0", "29.0", "2"], // SHP_002 (lat aralık dışı)
            vec!["S1", "41.0", "29.0", "2"],  // SHP_008 (sequence yineleniyor)
        ];

        // rows fallback yolu
        let (rec_a, not_a) = validate_shapes(&make_file(headers.clone(), rows.clone()));

        // raw_text streaming yolu (K1 stream_mode'un ürettiği biçim)
        let mut text = headers.join(",");
        text.push('\n');
        for r in &rows {
            text.push_str(&r.join(","));
            text.push('\n');
        }
        let file_stream = RawFile {
            name: "shapes.txt".to_string(),
            headers: headers.iter().map(|s| s.to_string()).collect(),
            rows: Vec::new(),
            bytes: 0,
            raw_text: Some(text),
        };
        let (rec_b, not_b) = validate_shapes(&file_stream);

        assert_eq!(rec_a.len(), rec_b.len(), "record sayısı iki yolda aynı");
        let lines_a: Vec<u64> = rec_a.iter().map(|r| r.line).collect();
        let lines_b: Vec<u64> = rec_b.iter().map(|r| r.line).collect();
        assert_eq!(lines_a, lines_b, "line numaraları iki yolda aynı (2,3,4)");
        let mut rules_a: Vec<&str> = not_a.iter().map(|n| n.rule_id.as_str()).collect();
        let mut rules_b: Vec<&str> = not_b.iter().map(|n| n.rule_id.as_str()).collect();
        rules_a.sort();
        rules_b.sort();
        assert_eq!(rules_a, rules_b, "iki yol aynı notice kümesi");
        assert!(rules_b.contains(&"SHP_002") && rules_b.contains(&"SHP_008"));
    }
}
