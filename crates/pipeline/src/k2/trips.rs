use std::borrow::Cow;
use std::io::Cursor;

use gtfs_core::EntityType;
use rustc_hash::FxHashMap;
use smol_str::SmolStr;

use super::common::{get_col, make_k2_notice, parse_f64_col, parse_u32_col};
use super::stop_times::{next_csv_record, ZipCsvReader};
use crate::k1_parse::RawFile;

// TripRecord boyutunu sabitle — alan ekleme struct'ı şişirirse CI kırar.
const _: () = assert!(std::mem::size_of::<TripRecord>() <= 128);

/// String intern tablosu — TripRecord u32 indekslerinin çözümlendiği yer.
/// Index 0 = boş string / None sentinel (her tablo None/absent için 0'ı saklar).
#[derive(Debug, Default)]
pub struct TripInternTable {
    pub route_ids:   Vec<SmolStr>,
    pub service_ids: Vec<SmolStr>,
    pub shape_ids:   Vec<SmolStr>,
    pub headsigns:   Vec<SmolStr>,
    pub short_names: Vec<SmolStr>,
    pub block_ids:   Vec<SmolStr>,
    pub jp_offices:  Vec<SmolStr>,
}

impl TripInternTable {
    pub fn new() -> Self {
        let empty = || vec![SmolStr::default()];
        Self {
            route_ids:   empty(),
            service_ids: empty(),
            shape_ids:   empty(),
            headsigns:   empty(),
            short_names: empty(),
            block_ids:   empty(),
            jp_offices:  empty(),
        }
    }

    #[inline]
    pub fn route_id<'a>(&'a self, t: &TripRecord) -> &'a str {
        self.route_ids.get(t.route_idx as usize).map(SmolStr::as_str).unwrap_or("")
    }
    #[inline]
    pub fn service_id<'a>(&'a self, t: &TripRecord) -> &'a str {
        self.service_ids.get(t.service_idx as usize).map(SmolStr::as_str).unwrap_or("")
    }
    #[inline]
    pub fn shape_id<'a>(&'a self, t: &TripRecord) -> Option<&'a str> {
        if t.shape_idx == 0 { return None; }
        self.shape_ids.get(t.shape_idx as usize).map(SmolStr::as_str)
    }
    #[inline]
    pub fn headsign<'a>(&'a self, t: &TripRecord) -> Option<&'a str> {
        if t.headsign_idx == 0 { return None; }
        self.headsigns.get(t.headsign_idx as usize).map(SmolStr::as_str)
    }
    #[inline]
    pub fn short_name<'a>(&'a self, t: &TripRecord) -> Option<&'a str> {
        if t.short_name_idx == 0 { return None; }
        self.short_names.get(t.short_name_idx as usize).map(SmolStr::as_str)
    }
    #[inline]
    pub fn block_id<'a>(&'a self, t: &TripRecord) -> Option<&'a str> {
        if t.block_idx == 0 { return None; }
        self.block_ids.get(t.block_idx as usize).map(SmolStr::as_str)
    }
    #[inline]
    pub fn jp_office_id<'a>(&'a self, t: &TripRecord) -> Option<&'a str> {
        if t.jp_office_idx == 0 { return None; }
        self.jp_offices.get(t.jp_office_idx as usize).map(SmolStr::as_str)
    }
}

#[derive(Debug, Clone)]
pub struct TripRecord {
    pub trip_id:        SmolStr,
    pub route_idx:      u32,
    pub service_idx:    u32,
    pub shape_idx:      u32,
    pub headsign_idx:   u32,
    pub short_name_idx: u32,
    pub block_idx:      u32,
    pub jp_office_idx:  u32,
    pub direction_id:   Option<u32>,
    pub wheelchair_accessible: Option<u32>,
    pub bikes_allowed:  Option<u32>,
    pub cars_allowed:   Option<u32>,
    pub safe_duration_factor: Option<f64>,
    /// Spec tipi **Float** (saniye). 2026-08-02'ye kadar `u32` okunuyordu → `12.5` gibi
    /// GEÇERLİ bir değer sessizce siliniyordu.
    pub safe_duration_offset: Option<f64>,
    pub line: u64,
}

/// String intern yardımcısı — boş string → 0 sentinel, aksi hâlde tabloya ekle/bul.
#[inline]
fn intern_idx(raw: &str, table: &mut Vec<SmolStr>, map: &mut FxHashMap<String, u32>) -> u32 {
    if raw.is_empty() { return 0; }
    if let Some(&idx) = map.get(raw) { return idx; }
    let idx = table.len() as u32;
    table.push(SmolStr::new(raw));
    map.insert(raw.to_string(), idx);
    idx
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

pub fn validate_trips(file: &RawFile, zip_bytes: Option<&[u8]>) -> (Vec<TripRecord>, TripInternTable, Vec<gtfs_core::Notice>) {
    // #38: trips.txt K1'de yalnızca başlık okunur; K2 zip_bytes'tan stream eder.
    // Eski rows yolu test/legacy fallback olarak korunur.
    let mut notices = Vec::new();
    // Kapasite tahmini: sıkışık boyut / 60 (ortalama satır).
    let est_rows = zip_bytes.map(|_| (file.bytes as usize / 60).max(1))
        .or_else(|| file.raw_text.as_ref().map(|t| t.len() / 60))
        .unwrap_or(file.rows.len());
    let mut records: Vec<TripRecord> = Vec::with_capacity(est_rows);
    let mut counter = 0u32;

    let cols = Cols::from_headers(&file.headers);
    let mut trp021_missing_count: usize = 0;
    let mut trp021_missing_examples: Vec<SmolStr> = Vec::new();
    let mut trp021_first_line: Option<u64> = None;
    let mut bikes_allowed_set_count: u32 = 0;

    let has_trip_id_col   = file.headers.iter().any(|h| h == "trip_id");
    let has_route_id_col  = file.headers.iter().any(|h| h == "route_id");

    // Intern tablo + per-field lookup map'leri
    let mut interns = TripInternTable::new();
    let mut route_map:      FxHashMap<String, u32> = FxHashMap::default();
    let mut service_map:    FxHashMap<String, u32> = FxHashMap::default();
    let mut shape_map:      FxHashMap<String, u32> = FxHashMap::default();
    let mut headsign_map:   FxHashMap<String, u32> = FxHashMap::default();
    let mut short_name_map: FxHashMap<String, u32> = FxHashMap::default();
    let mut block_map:      FxHashMap<String, u32> = FxHashMap::default();
    let mut jp_office_map:  FxHashMap<String, u32> = FxHashMap::default();

    // Satır işleyici — hem stream (raw_text) hem rows fallback yolundan çağrılır.
    let mut process = |row: &[Cow<'_, str>], line: u64| {
        let trip_id = SmolStr::new(get_col(row, cols.trip_id));
        let entity_id = (!trip_id.is_empty()).then(|| trip_id.to_string());

        // TRP_001: trip_id zorunlu
        if trip_id.is_empty() && has_trip_id_col {
            notices.push(make_k2_notice(
                &mut counter, "TRP_001", EntityType::Trip, None,
                None, &file.name, Some(line), Some("trip_id"),
                Some(String::new()), None,
                "trip_id zorunludur.".to_string(),
                "Her sefere benzersiz bir trip_id atayın.",
            ));
        }

        let route_idx = intern_idx(get_col(row, cols.route_id), &mut interns.route_ids, &mut route_map);
        let service_idx = intern_idx(get_col(row, cols.service_id), &mut interns.service_ids, &mut service_map);

        // TRP_031: route_id required — intern_idx 0 döndürdüyse boş demektir
        if route_idx == 0 && has_route_id_col {
            notices.push(make_k2_notice(
                &mut counter, "TRP_031", EntityType::Trip, None,
                None, &file.name, Some(line), Some("route_id"),
                Some(String::new()), None,
                "route_id zorunludur.".to_string(),
                "Her sefere geçerli bir route_id atayın.",
            ));
        }

        let shape_idx = intern_idx(get_col(row, cols.shape_id), &mut interns.shape_ids, &mut shape_map);

        // trip_headsign ve trip_short_name için önce raw string al (TRP_014 kontrolü için)
        let headsign_raw   = get_col(row, cols.trip_headsign);
        let short_name_raw = get_col(row, cols.trip_short_name);

        // TRP_014: trip_short_name çok uzun (>20 karakter)
        // KARAKTER sayısı, BAYT değil: `str::len()` UTF-8 bayt döndürür ve çok baytlı
        // alfabelerde (Türkçe, Japonca) eşiği erken tetikliyordu.
        let short_name_chars = short_name_raw.chars().count();
        if !short_name_raw.is_empty() && short_name_chars > 20 {
            notices.push(make_k2_notice(
                &mut counter, "TRP_014", EntityType::Trip, entity_id.clone(),
                None, &file.name, Some(line), Some("trip_short_name"),
                Some(short_name_chars.to_string()), Some("≤20".to_string()),
                format!("trip_short_name {} karakter; 20'yi aşmamalıdır.", short_name_chars),
                "trip_short_name'i kısaltın.",
            ));
        }

        let headsign_idx    = intern_idx(headsign_raw,   &mut interns.headsigns,    &mut headsign_map);
        let short_name_idx  = intern_idx(short_name_raw, &mut interns.short_names,  &mut short_name_map);
        let block_idx       = intern_idx(get_col(row, cols.block_id),     &mut interns.block_ids,  &mut block_map);
        let jp_office_idx   = intern_idx(get_col(row, cols.jp_office_id), &mut interns.jp_offices, &mut jp_office_map);

        // TRP_005: direction_id 0 veya 1 olmalı
        let dir_raw = get_col(row, cols.direction_id);
        let direction_id = match parse_u32_col(dir_raw) {
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
        let wheelchair_accessible = match parse_u32_col(wc_raw) {
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
            Err(()) => {
                // Sayı OLMAYAN değer eskiden sessizce düşüyordu: aralık dışı sayı TRP_006 üretirken
                // "abc" hiçbir bulgu vermiyordu. Aynı olgunun iki dalı → aynı kural (PTH_027 emsali).
                notices.push(make_k2_notice(
                    &mut counter, "TRP_006", EntityType::Trip, entity_id.clone(),
                    None, &file.name, Some(line), Some("wheelchair_accessible"),
                    Some(wc_raw.to_string()), Some("0, 1 veya 2".to_string()),
                    format!("wheelchair_accessible '{}' sayı olarak okunamıyor.", wc_raw),
                    "wheelchair_accessible değerini 0 (bilgi yok), 1 (erişilebilir) veya 2 (erişilemez) olarak ayarlayın.",
                ));
                None
            }
        };

        // TRP_007: bikes_allowed 0, 1 veya 2 olmalı
        let ba_raw = get_col(row, cols.bikes_allowed);
        let bikes_allowed = match parse_u32_col(ba_raw) {
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
            Err(()) => {
                // Sayı OLMAYAN değer eskiden sessizce düşüyordu: aralık dışı sayı TRP_007 üretirken
                // "abc" hiçbir bulgu vermiyordu. Aynı olgunun iki dalı → aynı kural (PTH_027 emsali).
                notices.push(make_k2_notice(
                    &mut counter, "TRP_007", EntityType::Trip, entity_id.clone(),
                    None, &file.name, Some(line), Some("bikes_allowed"),
                    Some(ba_raw.to_string()), Some("0, 1 veya 2".to_string()),
                    format!("bikes_allowed '{}' sayı olarak okunamıyor.", ba_raw),
                    "bikes_allowed değerini 0 (bilgi yok), 1 (izinli) veya 2 (izinsiz) olarak ayarlayın.",
                ));
                None
            }
        };

        // TRP_021: bikes_allowed eksik sayacı
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

        // TRP_032: cars_allowed 0, 1 veya 2 olmalı
        let ca_raw = get_col(row, cols.cars_allowed);
        let cars_allowed = match parse_u32_col(ca_raw) {
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

        // TRP_034: iki alan da spec'te `Float`. Parse hatası eskiden `.ok().flatten()` ile
        // yutuluyordu; ayrıca offset `u32` okunuyordu ve ondalık saniye değerleri düşüyordu.
        let mut parse_safe = |raw: &str, field: &'static str| -> Option<f64> {
            match parse_f64_col(raw) {
                Ok(v) => v,
                Err(()) => {
                    notices.push(make_k2_notice(
                        &mut counter, "TRP_034", EntityType::Trip,
                        Some(trip_id.to_string()), None,
                        &file.name, Some(line), Some(field),
                        Some(raw.to_string()), Some("ondalık sayı".to_string()),
                        format!("{field} '{raw}' ondalık sayı olarak okunamıyor."),
                        "safe_duration alanlarını ondalık sayı olarak girin.",
                    ));
                    None
                }
            }
        };
        let safe_duration_factor = parse_safe(get_col(row, cols.safe_duration_factor), "safe_duration_factor");
        let safe_duration_offset = parse_safe(get_col(row, cols.safe_duration_offset), "safe_duration_offset");

        records.push(TripRecord {
            trip_id,
            route_idx, service_idx, shape_idx,
            headsign_idx, short_name_idx, block_idx, jp_office_idx,
            direction_id,
            wheelchair_accessible, bikes_allowed, cars_allowed,
            safe_duration_factor, safe_duration_offset,
            line,
        });
    };

    // ── Sürücü: zip_bytes → ZIP stream; raw_text → string stream; rows → fallback ──
    if let Some(zb) = zip_bytes {
        let cursor = Cursor::new(zb);
        match zip::ZipArchive::new(cursor) {
            Err(e) => {
                notices.push(make_k2_notice(
                    &mut counter, "ARC_009", EntityType::File, Some(file.name.clone()),
                    None, &file.name, None, None, None, None,
                    format!("'{}' ZIP yeniden açılamadı: {e}.", file.name),
                    "ZIP arşivini kontrol edin.",
                ));
            }
            Ok(mut archive) => {
                match archive.by_name(&file.name) {
                    Err(e) => {
                        notices.push(make_k2_notice(
                            &mut counter, "ARC_009", EntityType::File, Some(file.name.clone()),
                            None, &file.name, None, None, None, None,
                            format!("'{}' ZIP girdisi bulunamadı: {e}.", file.name),
                            "ZIP arşivini kontrol edin.",
                        ));
                    }
                    Ok(entry) => {
                        let mut csv_reader = ZipCsvReader::new(entry);
                        let mut raw_fields: Vec<Vec<u8>> = Vec::with_capacity(16);
                        let mut zip_line: u64 = 2;
                        let mut header_skipped = false;
                        while csv_reader.next_record(&mut raw_fields) {
                            if raw_fields.len() == 1 && raw_fields[0].is_empty() { continue; }
                            if !header_skipped { header_skipped = true; continue; }
                            let strings: Vec<String> = raw_fields.iter()
                                .map(|f| match std::str::from_utf8(f) {
                                    Ok(s) => s.to_owned(),
                                    Err(_) => String::from_utf8_lossy(f).into_owned(),
                                })
                                .collect();
                            let cow_row: Vec<Cow<'_, str>> =
                                strings.iter().map(|s| Cow::Borrowed(s.as_str())).collect();
                            process(&cow_row, zip_line);
                            zip_line += 1;
                        }
                    }
                }
            }
        }
    } else if let Some(text) = &file.raw_text {
        let mut pos = 0usize;
        let mut buf: Vec<Cow<'_, str>> = Vec::with_capacity(16);
        let mut data_idx = 0usize;
        let mut header_skipped = false;
        while next_csv_record(text, &mut pos, &mut buf) {
            if buf.len() == 1 && buf[0].is_empty() { continue; }
            if !header_skipped { header_skipped = true; continue; }
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

    // TRP_021: loop sonrası tek özet notice
    if trp021_missing_count > 0 {
        let total = trp021_missing_count + bikes_allowed_set_count as usize;
        if bikes_allowed_set_count == 0 {
            notices.push(make_k2_notice(
                &mut counter, "TRP_021", EntityType::Trip, None,
                None, &file.name, None, Some("bikes_allowed"),
                Some(format!("{trp021_missing_count}/{total}")), Some("0".to_string()),
                format!("Bu feed'de bikes_allowed alanı hiçbir seferde belirtilmemiş ({trp021_missing_count} sefer)."),
                "bikes_allowed değerini 0 (bilgi yok), 1 (bisiklet izinli) veya 2 (bisiklet izinsiz) olarak ayarlayın.",
            ));
        } else {
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
                Some(format!("{trp021_missing_count}/{total}")), Some("0".to_string()),
                format!("{total} seferin {trp021_missing_count} tanesinde bikes_allowed belirtilmemiş ({:.0}%).{examples}",
                    trp021_missing_count as f64 / total as f64 * 100.0),
                "Tüm seferlerin bikes_allowed alanını 0 (bilgi yok), 1 (bisiklet izinli) veya 2 (bisiklet izinsiz) olarak doldurun.",
            ));
        }
    }

    (records, interns, notices)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::k1_parse::RawFile;

    // Testler rows fallback yolunu kullanır (raw_text=None).
    fn make_file(headers: Vec<&str>, rows: Vec<Vec<&str>>) -> RawFile {
        RawFile {
            name: "trips.txt".to_string(),
            headers: headers.into_iter().map(str::to_string).collect(),
            rows: rows.into_iter().map(|r| r.into_iter().map(smol_str::SmolStr::from).collect()).collect(),
            bytes: 0,
            raw_text: None,
        }
    }

    // Streaming yolunu test et — shapes.rs deseni.
    fn make_file_streaming(headers: Vec<&str>, rows: Vec<Vec<&str>>) -> RawFile {
        let header_line = headers.join(",");
        let data_lines: Vec<String> = rows.iter()
            .map(|r| r.join(","))
            .collect();
        let text = format!("{}\n{}\n", header_line, data_lines.join("\n"));
        RawFile {
            name: "trips.txt".to_string(),
            headers: headers.into_iter().map(str::to_string).collect(),
            rows: vec![],
            bytes: text.len() as u32,
            raw_text: Some(text),
        }
    }

    #[test]
    fn valid_trip_produces_no_notices() {
        let file = make_file(
            vec!["route_id", "service_id", "trip_id", "bikes_allowed"],
            vec![vec!["R1", "SVC1", "T1", "0"]],
        );
        let (records, _ti, notices) = validate_trips(&file, None);
        assert_eq!(records.len(), 1);
        assert!(notices.is_empty(), "Geçerli sefer notice üretmemeli: {:?}", notices);
    }

    #[test]
    fn streaming_matches_rows_path() {
        let headers = vec!["route_id", "service_id", "trip_id", "direction_id", "bikes_allowed"];
        let rows = vec![
            vec!["R1", "SVC1", "T1", "0", "1"],
            vec!["R2", "SVC2", "T2", "1", "2"],
        ];
        let (rec_rows, ti_rows, notices_rows) = validate_trips(&make_file(headers.clone(), rows.clone()), None);
        let (rec_stream, ti_stream, notices_stream) = validate_trips(&make_file_streaming(headers, rows), None);
        assert_eq!(rec_rows.len(), rec_stream.len());
        assert_eq!(notices_rows.len(), notices_stream.len());
        for (a, b) in rec_rows.iter().zip(rec_stream.iter()) {
            assert_eq!(a.trip_id, b.trip_id);
            assert_eq!(ti_rows.route_id(a), ti_stream.route_id(b));
            assert_eq!(a.direction_id, b.direction_id);
        }
    }

    #[test]
    fn invalid_direction_id_produces_trp_005() {
        let file = make_file(
            vec!["route_id", "service_id", "trip_id", "direction_id"],
            vec![vec!["R1", "WKD", "T1", "9"]],
        );
        let (_, _ti, notices) = validate_trips(&file, None);
        assert!(notices.iter().any(|n| n.rule_id == "TRP_005"),
            "direction_id=9 must produce TRP_005, got: {:?}", notices.iter().map(|n| &n.rule_id).collect::<Vec<_>>());
    }

    #[test]
    fn invalid_wheelchair_accessible_produces_trp_006() {
        let file = make_file(
            vec!["route_id", "service_id", "trip_id", "wheelchair_accessible"],
            vec![vec!["R1", "SVC1", "T1", "5"]],
        );
        let (_, _ti, notices) = validate_trips(&file, None);
        assert!(notices.iter().any(|n| n.rule_id == "TRP_006"));
    }

    #[test]
    fn invalid_cars_allowed_produces_trp_032() {
        let file = make_file(
            vec!["route_id", "service_id", "trip_id", "cars_allowed"],
            vec![vec!["R1", "SVC1", "T1", "5"]],
        );
        let (_, _ti, notices) = validate_trips(&file, None);
        assert!(notices.iter().any(|n| n.rule_id == "TRP_032"));
    }

    #[test]
    fn valid_cars_allowed_produces_no_trp_032() {
        let file = make_file(
            vec!["route_id", "service_id", "trip_id", "cars_allowed"],
            vec![vec!["R1", "SVC1", "T1", "2"]],
        );
        let (_, _ti, notices) = validate_trips(&file, None);
        assert!(!notices.iter().any(|n| n.rule_id == "TRP_032"));
    }
    #[test]
    fn valid_direction_id_zero_produces_no_notice() {
        let file = make_file(
            vec!["route_id", "service_id", "trip_id", "direction_id"],
            vec![vec!["R1", "SVC1", "T1", "0"]],
        );
        let (_, _ti, notices) = validate_trips(&file, None);
        assert!(!notices.iter().any(|n| n.rule_id == "TRP_005"));
    }
}
