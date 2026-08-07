use std::borrow::Cow;
use std::io::Cursor;

use gtfs_core::EntityType;
use rustc_hash::FxHashMap;
use smol_str::SmolStr;

use super::common::{get_col_raw, get_col, make_k2_notice};
use super::stop_times::{next_csv_record, ZipCsvReader};
use crate::k1_parse::RawFile;

// ── Çıktı tipi ────────────────────────────────────────────────────────────────

/// calendar_dates.txt'in bellek-verimli indeksi.
///
/// `added`/`removed` Map semantiği:
///   - Anahtar: service_id → en az bir type=1 (added) / type=2 (removed) kaydı VAR anlamına gelir
///     (tarih geçerliliğinden bağımsız; K4 XFL_006 ve CAL_018 bu varlığa bakır).
///   - Değer Vec<u32>: yalnızca GEÇERLİ YYYYMMDD tarihler (parse hatası olanlara yer yok).
///     Vec'ler parse sonunda `sort_unstable()` ile sıralanır; binary_search ile O(log n) üyelik.
///
/// `exception_count`: boş olmayan service_id'ye sahip tüm geçerli satırları sayar
///   (geçersiz tarih / exception_type dahil). CLD_006 ve K6 CLD_007 bu sayıya bakar.
///
/// K5 bitmap davranışı: tüm `added` tarihleri önce işlenir, ardından `removed`.
///   ⚠️ Çakışma notu: aynı (service_id, date) için hem type=1 hem type=2 varsa
///   `removed` kazanır (CSV satır sırasından bağımsız). Geçerli GTFS'te bu durum olmaz;
///   bozuk feed'ler için tanımlanmış davranış budur.
#[derive(Debug, Default)]
pub struct CalendarDateIndex {
    /// exception_type=1 olan service_id'ler → sıralı YYYYMMDD tarihleri (geçerli tarihler).
    pub added: FxHashMap<SmolStr, Vec<u32>>,
    /// exception_type=2 olan service_id'ler → sıralı YYYYMMDD tarihleri (geçerli tarihler).
    pub removed: FxHashMap<SmolStr, Vec<u32>>,
    /// service_id → ilk görülen satır numarası (K4 CLD_004 notice için).
    pub first_line: FxHashMap<SmolStr, u64>,
    /// service_id → toplam exception satırı (CLD_006 / K6 CLD_007 için ham sayı).
    /// Geçersiz tarih veya exception_type içeren satırlar da dahil; boş service_id hariç.
    pub exception_count: FxHashMap<SmolStr, u32>,
    /// Başlık hariç toplam veri satırı sayısı (boş service_id dahil).
    /// Dosya özeti (RAOR sayfası) için — stop_times_index.total_rows ile aynı semantik.
    pub raw_row_count: u64,
}

// ── Parse ──────────────────────────────────────────────────────────────────────

struct Cols {
    service_id:     Option<usize>,
    date:           Option<usize>,
    exception_type: Option<usize>,
}

impl Cols {
    fn from_headers(headers: &[String]) -> Self {
        let pos = |name: &str| headers.iter().position(|h| h == name);
        Self {
            service_id:     pos("service_id"),
            date:           pos("date"),
            exception_type: pos("exception_type"),
        }
    }
}

fn parse_date_raw(raw: &str) -> Result<Option<(u32, u32, u32)>, String> {
    if raw.is_empty() { return Ok(None); }
    if raw.len() != 8 {
        return Err(format!("'{raw}' YYYYMMDD formatında değil."));
    }
    let year  = raw[0..4].parse::<u32>().map_err(|_| format!("'{raw}' geçersiz yıl."))?;
    let month = raw[4..6].parse::<u32>().map_err(|_| format!("'{raw}' geçersiz ay."))?;
    let day   = raw[6..8].parse::<u32>().map_err(|_| format!("'{raw}' geçersiz gün."))?;
    // ⚠️ ORTAK yardımcı (issue #82): burada `ay ≤ 12, gün ≤ 31` deniyordu ve `20260231`
    // geçiyordu. Akış ile akış-dışı yol AYNI kararı vermek zorunda.
    if !super::common::is_valid_calendar_date(year, month, day) {
        return Err(format!("'{raw}' takvimde olmayan tarih."));
    }
    Ok(Some((year, month, day)))
}

pub fn validate_calendar_dates(
    file: &RawFile,
    zip_bytes: Option<&[u8]>,
) -> (CalendarDateIndex, Vec<gtfs_core::Notice>) {
    // #38: calendar_dates.txt K1'de yalnızca başlık okunur; K2 zip_bytes'tan stream eder.
    let mut notices = Vec::new();
    let mut counter = 0u32;

    let mut index = CalendarDateIndex::default();

    // service_id: çok az benzersiz değer, milyonlarca satırda tekrar eder → intern.
    let mut service_id_cache: FxHashMap<String, SmolStr> = FxHashMap::default();

    let cols = Cols::from_headers(&file.headers);
    let has_service_id_col     = file.headers.iter().any(|h| h == "service_id");
    let has_exception_type_col = file.headers.iter().any(|h| h == "exception_type");

    let mut process = |row: &[Cow<'_, str>], line: u64| {
        index.raw_row_count += 1;
        let raw_sid = get_col_raw(row, cols.service_id);
        let service_id: SmolStr = if raw_sid.is_empty() {
            SmolStr::default()
        } else if raw_sid.len() <= 22 {
            SmolStr::new(raw_sid)
        } else {
            service_id_cache
                .entry(raw_sid.to_string())
                .or_insert_with(|| SmolStr::new(raw_sid))
                .clone()
        };
        let entity_id = (!service_id.is_empty()).then(|| service_id.to_string());

        // CLD_001: service_id required
        if service_id.is_empty() && has_service_id_col {
            notices.push(make_k2_notice(
                &mut counter, "CLD_001", EntityType::Service, None,
                None, &file.name, Some(line), Some("service_id"),
                Some(String::new()), None,
                "service_id zorunludur.".to_string(),
                "service_id alanını doldurun.",
            ));
        }

        // CLD_002: date required + valid YYYYMMDD
        let date_raw = get_col(row, cols.date);
        let date = match parse_date_raw(date_raw) {
            Ok(v) => {
                if date_raw.is_empty() {
                    notices.push(make_k2_notice(
                        &mut counter, "CLD_002", EntityType::Service, entity_id.clone(),
                        None, &file.name, Some(line), Some("date"),
                        Some(String::new()), Some("YYYYMMDD".to_string()),
                        "date zorunludur.".to_string(),
                        "YYYYMMDD formatında tarihi girin.",
                    ));
                }
                // CLD_005: tarih 2000–2099 dışında
                if let Some((year, month, day)) = v {
                    if year < 2000 || year > 2099 {
                        notices.push(make_k2_notice(
                            &mut counter, "CLD_005", EntityType::Service, entity_id.clone(),
                            None, &file.name, Some(line), Some("date"),
                            Some(format!("{year}-{month:02}-{day:02}")),
                            Some("2000–2099".to_string()),
                            format!(
                                "service_id '{}' için tarih {year}-{month:02}-{day:02} \
                                 makul yıl aralığı dışında.", service_id
                            ),
                            "GTFS tarihleri 2000–2099 yılları arasında olmalıdır.",
                        ));
                    }
                }
                v
            }
            Err(err) => {
                notices.push(make_k2_notice(
                    &mut counter, "CLD_002", EntityType::Service, entity_id.clone(),
                    None, &file.name, Some(line), Some("date"),
                    Some(date_raw.to_string()), Some("YYYYMMDD".to_string()),
                    err,
                    "YYYYMMDD formatında tarihi girin.",
                ));
                None
            }
        };

        // CLD_003: exception_type required + enum 1 or 2
        let et_raw = get_col(row, cols.exception_type);
        let exception_type = if et_raw.is_empty() {
            if has_exception_type_col {
                notices.push(make_k2_notice(
                    &mut counter, "CLD_003", EntityType::Service, entity_id.clone(),
                    None, &file.name, Some(line), Some("exception_type"),
                    Some(String::new()), Some("1 veya 2".to_string()),
                    "exception_type zorunludur.".to_string(),
                    "exception_type değerini 1 (eklendi) veya 2 (kaldırıldı) olarak ayarlayın.",
                ));
            }
            None
        } else {
            match et_raw.parse::<u32>() {
                Ok(val) if val == 1 || val == 2 => Some(val),
                Ok(val) => {
                    notices.push(make_k2_notice(
                        &mut counter, "CLD_003", EntityType::Service, entity_id.clone(),
                        None, &file.name, Some(line), Some("exception_type"),
                        Some(val.to_string()), Some("1 veya 2".to_string()),
                        "exception_type 1 veya 2 olmalıdır.".to_string(),
                        "exception_type değerini 1 (hizmet eklendi) veya 2 (hizmet kaldırıldı) \
                         olarak ayarlayın.",
                    ));
                    Some(val)
                }
                Err(_) => {
                    notices.push(make_k2_notice(
                        &mut counter, "CLD_003", EntityType::Service, entity_id.clone(),
                        None, &file.name, Some(line), Some("exception_type"),
                        Some(et_raw.to_string()), Some("1 veya 2".to_string()),
                        format!("exception_type '{et_raw}' geçersiz; 1 veya 2 olmalıdır."),
                        "exception_type değerini 1 (hizmet eklendi) veya 2 (hizmet kaldırıldı) \
                         olarak ayarlayın.",
                    ));
                    None
                }
            }
        };

        // ── İndeks güncelleme ─────────────────────────────────────────────────
        if service_id.is_empty() {
            return;
        }
        *index.exception_count.entry(service_id.clone()).or_insert(0) += 1;
        index.first_line.entry(service_id.clone()).or_insert(line);

        // Tarih geçerliyse added/removed Vec'e ekle; sadece geçersiz exception_type kayıtları
        // (None veya ∉{1,2}) added/removed'a girmez; exception_count'a yine de girer.
        let date_u32 = match date {
            Some((y, m, d)) => Some(y * 10000 + m * 100 + d),
            None => None,
        };
        match exception_type {
            Some(1) => {
                let v = index.added.entry(service_id.clone()).or_default();
                if let Some(d) = date_u32 { v.push(d); }
            }
            Some(2) => {
                let v = index.removed.entry(service_id.clone()).or_default();
                if let Some(d) = date_u32 { v.push(d); }
            }
            _ => {
                // exception_type yok veya geçersiz: exception_count'a dahil, added/removed'a DEĞİL.
                // NOT: added/removed anahtarına dokunmuyoruz; bu service'in type=1/2 kaydı
                // olmadığını korumak için (XFL_006 / CAL_018 anahtar varlığına bakar).
            }
        }
    };

    // ARC_033 birikimi: ihlali OKUYUCULAR görür (alan sınırlarını yalnız onlar bilir),
    // satır numarasını bu döngü ekler. Emit tek yerde: `arc033_summary`.
    let mut rfc_acc = crate::k1_parse::Rfc4180Acc::default();
    let mut scan = super::stop_times::CsvScanState::default();
    let mut zip_unclosed = false;

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
                        let mut raw_fields: Vec<Vec<u8>> = Vec::with_capacity(4);
                        let mut zip_line: u64 = 2;
                        let mut header_skipped = false;
                        while csv_reader.next_record(&mut raw_fields) {
                            if raw_fields.len() == 1 && raw_fields[0].is_empty() { continue; }
                            if !header_skipped { header_skipped = true; continue; }
                            let strings: Vec<String> = raw_fields.iter()
                                .map(|f| match std::str::from_utf8(f) {
                                    Ok(s)  => s.to_owned(),
                                    Err(_) => String::from_utf8_lossy(f).into_owned(),
                                })
                                .collect();
                            let cow_row: Vec<Cow<'_, str>> =
                                strings.iter().map(|s| Cow::Borrowed(s.as_str())).collect();
                            zip_unclosed |= csv_reader.unclosed;
                            if let Some((k, ex)) = csv_reader.rfc.take() {
                                rfc_acc.observe(zip_line, k, &ex);
                            }
                            process(&cow_row, zip_line);
                            zip_line += 1;
                        }
                    }
                }
            }
        }
    } else if let Some(text) = &file.raw_text {
        let mut pos = 0usize;
        let mut buf: Vec<Cow<'_, str>> = Vec::with_capacity(4);
        let mut data_idx = 0usize;
        let mut header_skipped = false;
        while next_csv_record(text, &mut pos, &mut buf, &mut scan) {
            if buf.len() == 1 && buf[0].is_empty() { continue; }
            if !header_skipped { header_skipped = true; continue; }
            if let Some((k, ex)) = scan.rfc.take() {
                rfc_acc.observe((data_idx + 2) as u64, k, &ex);
            }
            process(&buf, (data_idx + 2) as u64);
            data_idx += 1;
        }
    } else {
        for (row_idx, row) in file.rows.iter().enumerate() {
            let cow_row: Vec<Cow<'_, str>> =
                row.iter().map(|s| Cow::Borrowed(s.as_str())).collect();
            process(&cow_row, (row_idx + 2) as u64);
        }
    }

    // CLD_006: bir service_id için çok fazla istisna günü (> 60)
    for (sid, &count) in &index.exception_count {
        if count > 60 {
            notices.push(make_k2_notice(
                &mut counter, "CLD_006", EntityType::Service, Some(sid.to_string()),
                None, &file.name, None, Some("service_id"),
                Some(count.to_string()), Some("≤ 60".to_string()),
                format!("'{sid}' servisinin {count} istisna günü var; bu çok fazla olabilir."),
                "Çok sayıda istisna yerine calendar.txt'te normal servis periyodu tanımlayın.",
            ));
        }
    }

    // Parse bitti: tarih listelerini sırala. Dedup YOK — exception_count ham satır sayısını
    // yansıtmalı. Aynı (service_id, date) duplicate'leri binary_search'ü bozmaz.
    for v in index.added.values_mut()   { v.sort_unstable(); }
    for v in index.removed.values_mut() { v.sort_unstable(); }

    // ARC_013: akış gövdesinde kapanmamış tırnak (issue #84) — DOSYA başına tek notice.
    if scan.unclosed || zip_unclosed {
        notices.push(super::common::arc013_unclosed_stream(&file.name, &mut counter));
    }

    // ARC_033: DOSYA başına TEK özet (kopya denetim yok — #75 dersi).
    if let Some(n) = super::common::arc033_summary(&rfc_acc, &file.name, &mut counter) {
        notices.push(n);
    }

    (index, notices)
}

// ── Testler ────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::k1_parse::RawFile;

    fn make_file(headers: Vec<&str>, rows: Vec<Vec<&str>>) -> RawFile {
        RawFile {
            name: "calendar_dates.txt".to_string(),
            headers: headers.into_iter().map(str::to_string).collect(),
            rows: rows
                .into_iter()
                .map(|r| r.into_iter().map(smol_str::SmolStr::from).collect())
                .collect(),
            bytes: 0,
            raw_text: None,
        }
    }

    // ── Mevcut testler (davranış değişmemeli) ────────────────────────────────

    #[test]
    fn valid_calendar_date_produces_no_notices() {
        let file = make_file(
            vec!["service_id", "date", "exception_type"],
            vec![vec!["SVC1", "20260101", "1"]],
        );
        let (idx, notices) = validate_calendar_dates(&file, None);
        assert_eq!(*idx.exception_count.get("SVC1" as &str).unwrap_or(&0), 1);
        assert!(idx.added.contains_key("SVC1" as &str));
        assert!(notices.is_empty(), "Geçerli kayıt notice üretmemeli: {:?}", notices);
    }

    #[test]
    fn missing_service_id_produces_cld_001() {
        let file = make_file(
            vec!["service_id", "date", "exception_type"],
            vec![vec!["", "20260101", "1"]],
        );
        let (idx, notices) = validate_calendar_dates(&file, None);
        assert!(notices.iter().any(|n| n.rule_id == "CLD_001"));
        assert!(idx.exception_count.is_empty(), "Boş service_id sayılmamalı");
    }

    #[test]
    fn invalid_date_produces_cld_002() {
        let file = make_file(
            vec!["service_id", "date", "exception_type"],
            vec![vec!["SVC1", "2026-01-01", "1"]],
        );
        let (idx, notices) = validate_calendar_dates(&file, None);
        assert!(notices.iter().any(|n| n.rule_id == "CLD_002"));
        // exception_count artar ama added'a tarih girmez (date = None)
        assert_eq!(*idx.exception_count.get("SVC1" as &str).unwrap_or(&0), 1);
        assert!(idx.added.get("SVC1" as &str).map_or(true, |v| v.is_empty()),
            "Geçersiz tarih added Vec'e girmemeli");
    }

    #[test]
    fn invalid_exception_type_produces_cld_003() {
        let file = make_file(
            vec!["service_id", "date", "exception_type"],
            vec![vec!["SVC1", "20260101", "3"]],
        );
        let (idx, notices) = validate_calendar_dates(&file, None);
        assert!(notices.iter().any(|n| n.rule_id == "CLD_003"));
        // exception_count artar ama added/removed'a girmez
        assert_eq!(*idx.exception_count.get("SVC1" as &str).unwrap_or(&0), 1);
        assert!(!idx.added.contains_key("SVC1" as &str), "Geçersiz type added'a girmemeli");
        assert!(!idx.removed.contains_key("SVC1" as &str), "Geçersiz type removed'a girmemeli");
    }

    #[test]
    fn date_out_of_range_produces_cld_005() {
        let file = make_file(
            vec!["service_id", "date", "exception_type"],
            vec![vec!["SVC1", "19991231", "1"]],
        );
        let (_, notices) = validate_calendar_dates(&file, None);
        assert!(notices.iter().any(|n| n.rule_id == "CLD_005"));
    }

    // ── Yeni regresyon testleri ───────────────────────────────────────────────

    /// Aynı (service_id, date) çifti için iki type=1 kaydı: exception_count=2,
    /// added Vec'inde iki kez aynı tarih (dedup YOK — ham sayı korunur).
    #[test]
    fn duplicate_service_date_type1_keeps_raw_count() {
        let file = make_file(
            vec!["service_id", "date", "exception_type"],
            vec![
                vec!["SVC1", "20260601", "1"],
                vec!["SVC1", "20260601", "1"],
            ],
        );
        let (idx, _) = validate_calendar_dates(&file, None);
        assert_eq!(*idx.exception_count.get("SVC1" as &str).unwrap(), 2,
            "Ham satır sayısı 2 olmalı");
        let dates = idx.added.get("SVC1" as &str).unwrap();
        assert_eq!(dates.len(), 2, "Dedup olmadan ikisi de Vec'te");
        assert!(dates.windows(2).all(|w| w[0] <= w[1]), "Sıralı olmalı");
    }

    /// type=1 ardından type=2 (aynı gün): exception_count=2, added'da tarih var,
    /// removed'da da tarih var. K5'te "all added then removed" → removed kazanır.
    #[test]
    fn type1_then_type2_same_date_both_in_index() {
        let file = make_file(
            vec!["service_id", "date", "exception_type"],
            vec![
                vec!["SVC1", "20260601", "1"],
                vec!["SVC1", "20260601", "2"],
            ],
        );
        let (idx, _) = validate_calendar_dates(&file, None);
        assert_eq!(*idx.exception_count.get("SVC1" as &str).unwrap(), 2);
        let added = idx.added.get("SVC1" as &str).unwrap();
        let removed = idx.removed.get("SVC1" as &str).unwrap();
        assert!(added.binary_search(&20260601u32).is_ok(), "added'da tarih olmalı");
        assert!(removed.binary_search(&20260601u32).is_ok(), "removed'da tarih olmalı");
        // K5 semantiği belgelendi: removed kazanır (mevcut koddan farklı ancak tanımlı davranış)
    }

    /// type=2 ardından type=1 (aynı gün): mevcut CSV-sıralı kodda type=1 kazanırdı.
    /// Yeni kodda removed kazanır — bu semantik fark bozuk feed'lerde kabul edilir.
    #[test]
    fn type2_then_type1_same_date_documents_new_behavior() {
        let file = make_file(
            vec!["service_id", "date", "exception_type"],
            vec![
                vec!["SVC1", "20260601", "2"],
                vec!["SVC1", "20260601", "1"],
            ],
        );
        let (idx, _) = validate_calendar_dates(&file, None);
        assert_eq!(*idx.exception_count.get("SVC1" as &str).unwrap(), 2);
        // Her ikisi de indekste; K5 removed-wins davranışı belgelendi
        assert!(idx.added.contains_key("SVC1" as &str));
        assert!(idx.removed.contains_key("SVC1" as &str));
    }

    /// Yalnızca type=2 kayıtlı service: removed'da anahtar var, added'da YOK.
    /// K4 XFL_006 bu servisi "ghost" olarak yakalar.
    #[test]
    fn only_removed_service_key_in_removed_only() {
        let file = make_file(
            vec!["service_id", "date", "exception_type"],
            vec![vec!["GHOST", "20260601", "2"]],
        );
        let (idx, _) = validate_calendar_dates(&file, None);
        assert!(idx.removed.contains_key("GHOST" as &str), "removed'da anahtar olmalı");
        assert!(!idx.added.contains_key("GHOST" as &str), "added'da anahtar olmamalı");
        assert_eq!(*idx.exception_count.get("GHOST" as &str).unwrap(), 1);
    }

    /// type=2 kaydı + date=None: anahtar removed'da var ama Vec boş.
    /// XFL_006 kontrolü anahtar varlığına bakır, tarih geçerliliğine değil.
    #[test]
    fn type2_with_invalid_date_key_present_vec_empty() {
        let file = make_file(
            vec!["service_id", "date", "exception_type"],
            vec![vec!["GHOST", "notadate", "2"]],
        );
        let (idx, notices) = validate_calendar_dates(&file, None);
        assert!(notices.iter().any(|n| n.rule_id == "CLD_002"), "CLD_002 olmalı");
        assert!(idx.removed.contains_key("GHOST" as &str), "Anahtar yine de removed'da olmalı");
        assert!(idx.removed.get("GHOST" as &str).unwrap().is_empty(), "Geçersiz tarih Vec'e girmemeli");
        assert_eq!(*idx.exception_count.get("GHOST" as &str).unwrap(), 1);
    }

    /// Bozuk tarih: exception_count artar, first_line kaydedilir, notice üretilir.
    #[test]
    fn invalid_date_counted_but_not_indexed() {
        let file = make_file(
            vec!["service_id", "date", "exception_type"],
            vec![vec!["SVC1", "bad", "1"]],
        );
        let (idx, notices) = validate_calendar_dates(&file, None);
        assert!(notices.iter().any(|n| n.rule_id == "CLD_002"));
        assert_eq!(*idx.exception_count.get("SVC1" as &str).unwrap(), 1);
        assert_eq!(*idx.first_line.get("SVC1" as &str).unwrap(), 2,
            "İlk satır numarası 2 olmalı");
        // Tarih geçersiz, added Vec'te yer yok
        assert!(idx.added.get("SVC1" as &str).map_or(true, |v| v.is_empty()));
    }

    /// first_line daima ilk görülen satırı gösterir.
    #[test]
    fn first_line_tracks_first_occurrence() {
        let file = make_file(
            vec!["service_id", "date", "exception_type"],
            vec![
                vec!["SVC1", "20260601", "1"],
                vec!["SVC1", "20260602", "1"],
            ],
        );
        let (idx, _) = validate_calendar_dates(&file, None);
        assert_eq!(*idx.first_line.get("SVC1" as &str).unwrap(), 2,
            "first_line ilk satır (2) olmalı");
    }
}
