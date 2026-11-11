use std::borrow::Cow;
use std::io::Cursor;

use gtfs_core::EntityType;
use rustc_hash::FxHashMap;
use smol_str::SmolStr;

use super::common::{get_col, get_col_raw, make_k2_notice};
use super::stop_times::{next_csv_record, ZipCsvReader};
use crate::k1_parse::RawFile;

// ── Output type ────────────────────────────────────────────────────────────────

/// Memory-efficient index for calendar_dates.txt.
///
/// `added`/`removed` Map semantics:
///   - Key: service_id → at least one type=1 (added) / type=2 (removed) record EXISTS
///     (independent of date validity; K4 XFL_006 and CAL_018 inspect this presence).
///   - Vec<u32> value: only VALID YYYYMMDD dates (parse failures are excluded).
///     Vecs are sorted with `sort_unstable()` after parsing; membership is O(log n) via binary_search.
///
/// `exception_count`: counts all valid rows with a non-empty service_id
///   (including invalid dates / exception_type). CLD_006 and K6 CLD_007 use this count.
///
/// K5 bitmap behavior: all `added` dates are processed first, then `removed`.
///   ⚠️ Conflict note: if both type=1 and type=2 exist for the same (service_id, date),
///   `removed` wins (independent of CSV row order). This does not occur in valid GTFS;
///   this is the defined behavior for broken feeds.
#[derive(Debug, Default)]
pub struct CalendarDateIndex {
    /// service_ids with exception_type=1 → sorted YYYYMMDD dates (valid dates).
    pub added: FxHashMap<SmolStr, Vec<u32>>,
    /// service_ids with exception_type=2 → sorted YYYYMMDD dates (valid dates).
    pub removed: FxHashMap<SmolStr, Vec<u32>>,
    /// service_id → first-seen row number (for the K4 CLD_004 notice).
    pub first_line: FxHashMap<SmolStr, u64>,
    /// service_id → total exception rows (raw count for CLD_006 / K6 CLD_007).
    /// Includes rows with invalid dates or exception_type; excludes empty service_id.
    pub exception_count: FxHashMap<SmolStr, u32>,
    /// Total data rows excluding the header (including empty service_id).
    /// For the file summary (RAOR page) — same semantics as stop_times_index.total_rows.
    pub raw_row_count: u64,
}

// ── Parsing ─────────────────────────────────────────────────────────────────────

struct Cols {
    service_id: Option<usize>,
    date: Option<usize>,
    exception_type: Option<usize>,
}

impl Cols {
    fn from_headers(headers: &[String]) -> Self {
        let pos = |name: &str| headers.iter().position(|h| h == name);
        Self {
            service_id: pos("service_id"),
            date: pos("date"),
            exception_type: pos("exception_type"),
        }
    }
}

fn parse_date_raw(raw: &str) -> Result<Option<(u32, u32, u32)>, String> {
    if raw.is_empty() {
        return Ok(None);
    }
    if raw.len() != 8 {
        return Err(format!("'{raw}' YYYYMMDD formatında değil."));
    }
    let year = raw[0..4]
        .parse::<u32>()
        .map_err(|_| format!("'{raw}' geçersiz yıl."))?;
    let month = raw[4..6]
        .parse::<u32>()
        .map_err(|_| format!("'{raw}' geçersiz ay."))?;
    let day = raw[6..8]
        .parse::<u32>()
        .map_err(|_| format!("'{raw}' geçersiz gün."))?;
    // ⚠️ SHARED helper (issue #82): this used to check `month ≤ 12, day ≤ 31` and let `20260231`
    // through. The streaming and non-streaming paths must make the SAME decision.
    if !super::common::is_valid_calendar_date(year, month, day) {
        return Err(format!("'{raw}' takvimde olmayan tarih."));
    }
    Ok(Some((year, month, day)))
}

pub fn validate_calendar_dates(
    file: &RawFile,
    zip_bytes: Option<&[u8]>,
) -> (CalendarDateIndex, Vec<gtfs_core::Notice>) {
    validate_calendar_dates_with_limits(file, zip_bytes, None, None)
}

pub fn validate_calendar_dates_with_limits(
    file: &RawFile,
    zip_bytes: Option<&[u8]>,
    max_data_rows: Option<usize>,
    mut stream_budget: Option<&mut super::stop_times::StreamBudget>,
) -> (CalendarDateIndex, Vec<gtfs_core::Notice>) {
    // #38: K1 reads only the header of calendar_dates.txt; K2 streams it from zip_bytes.
    let mut notices = Vec::new();
    let mut counter = 0u32;

    let mut index = CalendarDateIndex::default();

    // service_id: few unique values repeated across millions of rows → intern.
    let mut service_id_cache: FxHashMap<String, SmolStr> = FxHashMap::default();

    let cols = Cols::from_headers(&file.headers);
    let has_service_id_col = file.headers.iter().any(|h| h == "service_id");
    let has_exception_type_col = file.headers.iter().any(|h| h == "exception_type");
    let mut dq016 = crate::k1_parse::Dq016Acc::default();
    let mut arc037 = crate::k1_parse::Arc037Acc::default();
    let mut arc032 = crate::k1_parse::Arc032Acc::default();
    let mut arc021_fired = false;
    let mut arc030_fired = false;

    let header_count = file.headers.len();
    let mut process = |row: &[Cow<'_, str>], line: u64| {
        // ARC_012: the row's field count does not match the header. This is the same mechanism
        // described by the `ARC_034` comment below — because the file is streamed in K1, it never
        // enters K1's row loop, and the check would NEVER run here if left there. `stop_times.rs` and
        // `shapes.rs` had moved it, but `trips.rs` had not (found in corpus run 17; evidence:
        // `mdb-2013`'s trips.txt, where all 15,170 malformed rows were silent).
        // ⚠️ The rule is Critical·Spec, so it is visible to the R1 publication gate.
        if let Some((msg, tip, observed, is_info)) =
            crate::k1_parse::arc012_check(&file.name, line, row.len(), header_count)
        {
            let mut n = make_k2_notice(
                &mut counter,
                "ARC_012",
                EntityType::File,
                Some(file.name.clone()),
                None,
                &file.name,
                Some(line),
                None,
                Some(observed),
                None,
                msg,
                tip,
            );
            if is_info {
                n.severity = gtfs_core::Severity::Bilgi;
            }
            notices.push(n);
        }

        // ARC_034: repeated header → notice + DO NOT STORE the row. This file is streamed in K1
        // and only its header is read there; if the check stays in K1, it never runs here (#181).
        if crate::k1_parse::arc034_is_header_repeat(row, &file.headers) {
            notices.push(make_k2_notice(
                &mut counter, "ARC_034", EntityType::File, Some(file.name.clone()),
                None, &file.name, Some(line), None, None, None,
                format!("'{}' {line}. satırı başlık satırının tekrarı — veri olarak okunuyor.", file.name),
                "Remove the repeated header row in the middle of the file; it usually comes from two files being concatenated.",
            ));
            return;
        }
        // ARC_018: completely empty row → notice + DO NOT STORE the row (K1/stop_times precedent).
        // ARC_018/021/030 had not been moved in #181: this file is streamed in K1, where only the
        // header is read, so the check never ran (2026-10-13). An empty row would derive TRP_001, etc.
        if row.iter().all(|v| v.trim().is_empty()) {
            notices.push(make_k2_notice(
                &mut counter,
                "ARC_018",
                EntityType::File,
                Some(file.name.clone()),
                None,
                &file.name,
                Some(line),
                None,
                None,
                None,
                format!("'{}' {line}. satırı tamamen boş.", file.name),
                "Remove empty rows from the file.",
            ));
            return;
        }
        // ARC_021: unprintable/problematic character — once per file
        if !arc021_fired {
            if let Some(cp) = crate::k1_parse::arc021_bad_char(row.iter().map(|v| v.as_ref())) {
                arc021_fired = true;
                notices.push(make_k2_notice(
                    &mut counter,
                    "ARC_021",
                    EntityType::File,
                    Some(file.name.clone()),
                    None,
                    &file.name,
                    Some(line),
                    None,
                    Some(format!("U+{cp:04X}")),
                    None,
                    crate::k1_parse::arc021_message(&file.name, cp),
                    crate::k1_parse::ARC021_REMEDIATION,
                ));
            }
        }
        // ARC_030: tab/newline in a field value — once per file
        if !arc030_fired {
            if let Some(cp) = crate::k1_parse::arc030_bad_whitespace(row.iter().map(|v| v.as_ref()))
            {
                arc030_fired = true;
                notices.push(make_k2_notice(
                    &mut counter,
                    "ARC_030",
                    EntityType::File,
                    Some(file.name.clone()),
                    None,
                    &file.name,
                    Some(line),
                    None,
                    Some(format!("U+{cp:04X}")),
                    None,
                    crate::k1_parse::arc030_message(&file.name, cp),
                    crate::k1_parse::ARC030_REMEDIATION,
                ));
            }
        }
        index.raw_row_count += 1;
        dq016.observe(line, row.iter().map(|value| value.as_ref()), &file.headers);
        arc037.observe(line, row.iter().map(|value| value.as_ref()), &file.headers);
        arc032.observe(line, row.iter().map(|value| value.as_ref()), &file.headers);
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
                &mut counter,
                "CLD_001",
                EntityType::Service,
                None,
                None,
                &file.name,
                Some(line),
                Some("service_id"),
                Some(String::new()),
                None,
                "service_id zorunludur.".to_string(),
                "Fill in the service_id field.",
            ));
        }

        // CLD_002: date required + valid YYYYMMDD
        let date_raw = get_col(row, cols.date);
        let date = match parse_date_raw(date_raw) {
            Ok(v) => {
                if date_raw.is_empty() {
                    notices.push(make_k2_notice(
                        &mut counter,
                        "CLD_002",
                        EntityType::Service,
                        entity_id.clone(),
                        None,
                        &file.name,
                        Some(line),
                        Some("date"),
                        Some(String::new()),
                        Some("YYYYMMDD".to_string()),
                        "date zorunludur.".to_string(),
                        "Enter the date in YYYYMMDD format.",
                    ));
                }
                // CLD_005: date outside 2000–2099
                if let Some((year, month, day)) = v {
                    if !(2000..=2099).contains(&year) {
                        notices.push(make_k2_notice(
                            &mut counter,
                            "CLD_005",
                            EntityType::Service,
                            entity_id.clone(),
                            None,
                            &file.name,
                            Some(line),
                            Some("date"),
                            Some(format!("{year}-{month:02}-{day:02}")),
                            Some("2000–2099".to_string()),
                            format!(
                                "service_id '{}' için tarih {year}-{month:02}-{day:02} \
                                 makul yıl aralığı dışında.",
                                service_id
                            ),
                            "GTFS dates must be between 2000 and 2099.",
                        ));
                    }
                }
                v
            }
            Err(err) => {
                notices.push(make_k2_notice(
                    &mut counter,
                    "CLD_002",
                    EntityType::Service,
                    entity_id.clone(),
                    None,
                    &file.name,
                    Some(line),
                    Some("date"),
                    Some(date_raw.to_string()),
                    Some("YYYYMMDD".to_string()),
                    err,
                    "Enter the date in YYYYMMDD format.",
                ));
                None
            }
        };

        // CLD_003: exception_type required + enum 1 or 2
        let et_raw = get_col(row, cols.exception_type);
        let exception_type =
            if et_raw.is_empty() {
                if has_exception_type_col {
                    notices.push(make_k2_notice(
                        &mut counter,
                        "CLD_003",
                        EntityType::Service,
                        entity_id.clone(),
                        None,
                        &file.name,
                        Some(line),
                        Some("exception_type"),
                        Some(String::new()),
                        Some("1 or 2".to_string()),
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
                        Some(val.to_string()), Some("1 or 2".to_string()),
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
                        Some(et_raw.to_string()), Some("1 or 2".to_string()),
                        format!("exception_type '{et_raw}' geçersiz; 1 veya 2 olmalıdır."),
                        "exception_type değerini 1 (hizmet eklendi) veya 2 (hizmet kaldırıldı) \
                         olarak ayarlayın.",
                    ));
                        None
                    }
                }
            };

        // ── Update the index ───────────────────────────────────────────────────
        if service_id.is_empty() {
            return;
        }
        *index.exception_count.entry(service_id.clone()).or_insert(0) += 1;
        index.first_line.entry(service_id.clone()).or_insert(line);

        // Add valid dates to the added/removed Vec; only invalid exception_type records
        // (None or ∉{1,2}) stay out of added/removed; they still count toward exception_count.
        let date_u32 = date.map(|(y, m, d)| y * 10000 + m * 100 + d);
        match exception_type {
            Some(1) => {
                let v = index.added.entry(service_id.clone()).or_default();
                if let Some(d) = date_u32 {
                    v.push(d);
                }
            }
            Some(2) => {
                let v = index.removed.entry(service_id.clone()).or_default();
                if let Some(d) = date_u32 {
                    v.push(d);
                }
            }
            _ => {
                // exception_type is missing or invalid: included in exception_count, NOT added/removed.
                // NOTE: do not touch the added/removed key; this preserves that the service has no
                // type=1/2 record (XFL_006 / CAL_018 inspect key presence).
            }
        }
    };

    // ARC_033 accumulation: READERS see the violation (only they know field boundaries),
    // and this loop adds the row number. Emit in one place: `arc033_summary`.
    let mut rfc_acc = crate::k1_parse::Rfc4180Acc::default();
    let mut scan = super::stop_times::CsvScanState::default();
    let mut zip_unclosed = false;

    // ── Driver: zip_bytes → ZIP stream; raw_text → string stream; rows → fallback ──
    if let Some(zb) = zip_bytes {
        let cursor = Cursor::new(zb);
        match zip::ZipArchive::new(cursor) {
            Err(e) => {
                notices.push(make_k2_notice(
                    &mut counter,
                    "ARC_009",
                    EntityType::File,
                    Some(file.name.clone()),
                    None,
                    &file.name,
                    None,
                    None,
                    None,
                    None,
                    format!("'{}' ZIP yeniden açılamadı: {e}.", file.name),
                    "ZIP arşivini kontrol edin.",
                ));
            }
            Ok(mut archive) => {
                match archive.by_name(file.zip_entry_name.as_deref().unwrap_or(&file.name)) {
                    Err(e) => {
                        notices.push(make_k2_notice(
                            &mut counter,
                            "ARC_009",
                            EntityType::File,
                            Some(file.name.clone()),
                            None,
                            &file.name,
                            None,
                            None,
                            None,
                            None,
                            format!("'{}' ZIP girdisi bulunamadı: {e}.", file.name),
                            "ZIP arşivini kontrol edin.",
                        ));
                    }
                    Ok(entry) => {
                        let stream_limit = stream_budget
                            .as_ref()
                            .map(|budget| budget.limit())
                            .unwrap_or(super::stop_times::stream_byte_limit(max_data_rows));
                        let mut csv_reader =
                            ZipCsvReader::with_limits(entry, stream_limit, max_data_rows);
                        let mut raw_fields: Vec<Vec<u8>> = Vec::with_capacity(4);
                        let mut zip_line: u64 = 2;
                        let mut header_skipped = false;
                        while csv_reader.next_record(&mut raw_fields) {
                            if raw_fields.len() == 1 && raw_fields[0].is_empty() {
                                continue;
                            }
                            if !header_skipped {
                                header_skipped = true;
                                continue;
                            }
                            let strings: Vec<String> = raw_fields
                                .iter()
                                .map(|f| match std::str::from_utf8(f) {
                                    Ok(s) => s.to_owned(),
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
                        let bytes_read = csv_reader.bytes_read();
                        if let Some(budget) = stream_budget.as_mut() {
                            budget.consume(bytes_read);
                        }
                        if csv_reader.truncated() {
                            if let Some(max) = max_data_rows {
                                index.raw_row_count =
                                    index.raw_row_count.max(max.saturating_add(1) as u64);
                            }
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
            if buf.len() == 1 && buf[0].is_empty() {
                continue;
            }
            if !header_skipped {
                header_skipped = true;
                continue;
            }
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

    // CLD_006: too many exception days for one service_id (> 60)
    for (sid, &count) in &index.exception_count {
        if count > 60 {
            notices.push(make_k2_notice(
                &mut counter,
                "CLD_006",
                EntityType::Service,
                Some(sid.to_string()),
                None,
                &file.name,
                None,
                Some("service_id"),
                Some(count.to_string()),
                Some("≤ 60".to_string()),
                format!("'{sid}' servisinin {count} istisna günü var; bu çok fazla olabilir."),
                "Define a regular service period in calendar.txt instead of using many exceptions.",
            ));
        }
    }

    // Parsing is complete: sort the date lists. NO deduplication — exception_count must reflect
    // the raw row count. Duplicate (service_id, date) pairs do not break binary_search.
    for v in index.added.values_mut() {
        v.sort_unstable();
    }
    for v in index.removed.values_mut() {
        v.sort_unstable();
    }

    // DQ_016: one root summary per FILE for streamed calendar_dates.txt.
    // ARC_032: HTML marker — ONE summary per FILE (same as K1). It did not run on streamed files
    // until 2026-10-13 (trip_headsign/stop_headsign were not checked).
    if let Some((observed, msg, cols)) = arc032.summary(&file.name) {
        notices.push(make_k2_notice(
            &mut counter,
            "ARC_032",
            EntityType::File,
            Some(file.name.clone()),
            None,
            &file.name,
            arc032.first_line,
            Some(cols.as_str()),
            Some(observed),
            None,
            msg,
            crate::k1_parse::ARC032_REMEDIATION,
        ));
    }

    // ARC_037: non-ASCII character in an ID field — ONE summary per FILE (same as K1).
    if let Some((observed, msg, cols)) = arc037.summary(&file.name) {
        notices.push(make_k2_notice(
            &mut counter,
            "ARC_037",
            EntityType::File,
            Some(file.name.clone()),
            None,
            &file.name,
            arc037.first_line,
            Some(cols.as_str()),
            Some(observed),
            None,
            msg,
            crate::k1_parse::ARC037_REMEDIATION,
        ));
    }
    if let Some((observed, msg, cols)) = dq016.summary(&file.name) {
        let mut n = make_k2_notice(
            &mut counter,
            "DQ_016",
            EntityType::File,
            Some(file.name.clone()),
            None,
            &file.name,
            dq016.first_line,
            Some(cols.as_str()),
            Some(observed),
            None,
            msg,
            crate::k1_parse::DQ016_REMEDIATION,
        );
        n.details = dq016.evidence_details().map(Into::into);
        notices.push(n);
    }

    // ARC_013: unclosed quote in the streamed body (issue #84) — one notice per FILE.
    if scan.unclosed || zip_unclosed {
        notices.push(super::common::arc013_unclosed_stream(
            &file.name,
            &mut counter,
        ));
    }

    // ARC_033: ONE summary per FILE (no duplicate check — lesson from #75).
    if let Some(n) = super::common::arc033_summary(&rfc_acc, &file.name, &mut counter) {
        notices.push(n);
    }

    (index, notices)
}

// ── Tests ──────────────────────────────────────────────────────────────────────

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
            zip_entry_name: None,
        }
    }

    /// Streaming-path fixture — `raw_text` populated, `rows` empty.
    fn make_file_streaming(headers: Vec<&str>, rows: Vec<Vec<&str>>) -> RawFile {
        let text = format!(
            "{}\n{}\n",
            headers.join(","),
            rows.iter()
                .map(|r| r.join(","))
                .collect::<Vec<_>>()
                .join("\n")
        );
        RawFile {
            name: "calendar_dates.txt".to_string(),
            headers: headers.into_iter().map(str::to_string).collect(),
            rows: vec![],
            bytes: text.len() as u32,
            raw_text: Some(text),
            zip_entry_name: None,
        }
    }

    /// Same blind spot as `trips.txt`: the file is streamed in K1, so if `ARC_012` is left there,
    /// it never runs here. There was no EVIDENCE of malformed rows for this file in the corpus;
    /// it was found by reading the whitespace code and this test locks it in.
    #[test]
    fn arc012_fires_on_the_streaming_path_for_calendar_dates() {
        let file = make_file_streaming(
            vec!["service_id", "date", "exception_type"],
            vec![
                vec!["SVC1", "20260101", "1"],
                vec!["SVC2", "20260102", "1", "FAZLA"],
            ],
        );
        let (_idx, notices) = validate_calendar_dates(&file, None);
        let hit = notices
            .iter()
            .find(|n| n.rule_id == "ARC_012")
            .expect("fazla alanlı satır ARC_012 üretmeli");
        assert_eq!(hit.file.as_deref(), Some("calendar_dates.txt"));
        assert_eq!(hit.severity, gtfs_core::Severity::Kritik);
    }

    // ── Existing tests (behavior must not change) ─────────────────────────────

    #[test]
    fn valid_calendar_date_produces_no_notices() {
        let file = make_file(
            vec!["service_id", "date", "exception_type"],
            vec![vec!["SVC1", "20260101", "1"]],
        );
        let (idx, notices) = validate_calendar_dates(&file, None);
        assert_eq!(*idx.exception_count.get("SVC1" as &str).unwrap_or(&0), 1);
        assert!(idx.added.contains_key("SVC1" as &str));
        assert!(
            notices.is_empty(),
            "Geçerli kayıt notice üretmemeli: {:?}",
            notices
        );
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
        // exception_count increases but no date enters added (date = None)
        assert_eq!(*idx.exception_count.get("SVC1" as &str).unwrap_or(&0), 1);
        assert!(
            idx.added.get("SVC1" as &str).is_none_or(|v| v.is_empty()),
            "Geçersiz tarih added Vec'e girmemeli"
        );
    }

    #[test]
    fn invalid_exception_type_produces_cld_003() {
        let file = make_file(
            vec!["service_id", "date", "exception_type"],
            vec![vec!["SVC1", "20260101", "3"]],
        );
        let (idx, notices) = validate_calendar_dates(&file, None);
        assert!(notices.iter().any(|n| n.rule_id == "CLD_003"));
        // exception_count increases but it enters neither added nor removed
        assert_eq!(*idx.exception_count.get("SVC1" as &str).unwrap_or(&0), 1);
        assert!(
            !idx.added.contains_key("SVC1" as &str),
            "Geçersiz type added'a girmemeli"
        );
        assert!(
            !idx.removed.contains_key("SVC1" as &str),
            "Geçersiz type removed'a girmemeli"
        );
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

    // ── New regression tests ───────────────────────────────────────────────────

    /// Two type=1 records for the same (service_id, date): exception_count=2,
    /// the same date appears twice in the added Vec (NO deduplication — raw count is preserved).
    #[test]
    fn duplicate_service_date_type1_keeps_raw_count() {
        let file = make_file(
            vec!["service_id", "date", "exception_type"],
            vec![vec!["SVC1", "20260601", "1"], vec!["SVC1", "20260601", "1"]],
        );
        let (idx, _) = validate_calendar_dates(&file, None);
        assert_eq!(
            *idx.exception_count.get("SVC1" as &str).unwrap(),
            2,
            "Ham satır sayısı 2 olmalı"
        );
        let dates = idx.added.get("SVC1" as &str).unwrap();
        assert_eq!(dates.len(), 2, "Dedup olmadan ikisi de Vec'te");
        assert!(dates.windows(2).all(|w| w[0] <= w[1]), "Sıralı olmalı");
    }

    /// type=1 followed by type=2 (same day): exception_count=2, the date is in added,
    /// and it is also in removed. In K5, "all added then removed" → removed wins.
    #[test]
    fn type1_then_type2_same_date_both_in_index() {
        let file = make_file(
            vec!["service_id", "date", "exception_type"],
            vec![vec!["SVC1", "20260601", "1"], vec!["SVC1", "20260601", "2"]],
        );
        let (idx, _) = validate_calendar_dates(&file, None);
        assert_eq!(*idx.exception_count.get("SVC1" as &str).unwrap(), 2);
        let added = idx.added.get("SVC1" as &str).unwrap();
        let removed = idx.removed.get("SVC1" as &str).unwrap();
        assert!(
            added.binary_search(&20260601u32).is_ok(),
            "added'da tarih olmalı"
        );
        assert!(
            removed.binary_search(&20260601u32).is_ok(),
            "removed'da tarih olmalı"
        );
        // K5 semantics documented: removed wins (different from the old code, but defined behavior)
    }

    /// type=2 followed by type=1 (same day): the old CSV-ordered code let type=1 win.
    /// The new code lets removed win — this semantic difference is accepted for broken feeds.
    #[test]
    fn type2_then_type1_same_date_documents_new_behavior() {
        let file = make_file(
            vec!["service_id", "date", "exception_type"],
            vec![vec!["SVC1", "20260601", "2"], vec!["SVC1", "20260601", "1"]],
        );
        let (idx, _) = validate_calendar_dates(&file, None);
        assert_eq!(*idx.exception_count.get("SVC1" as &str).unwrap(), 2);
        // Both are in the index; K5 removed-wins behavior is documented.
        assert!(idx.added.contains_key("SVC1" as &str));
        assert!(idx.removed.contains_key("SVC1" as &str));
    }

    /// Service with only a type=2 record: key exists in removed, NOT in added.
    /// K4 XFL_006 catches this service as a "ghost".
    #[test]
    fn only_removed_service_key_in_removed_only() {
        let file = make_file(
            vec!["service_id", "date", "exception_type"],
            vec![vec!["GHOST", "20260601", "2"]],
        );
        let (idx, _) = validate_calendar_dates(&file, None);
        assert!(
            idx.removed.contains_key("GHOST" as &str),
            "removed'da anahtar olmalı"
        );
        assert!(
            !idx.added.contains_key("GHOST" as &str),
            "added'da anahtar olmamalı"
        );
        assert_eq!(*idx.exception_count.get("GHOST" as &str).unwrap(), 1);
    }

    /// type=2 record + date=None: key exists in removed but the Vec is empty.
    /// XFL_006 checks key presence, not date validity.
    #[test]
    fn type2_with_invalid_date_key_present_vec_empty() {
        let file = make_file(
            vec!["service_id", "date", "exception_type"],
            vec![vec!["GHOST", "notadate", "2"]],
        );
        let (idx, notices) = validate_calendar_dates(&file, None);
        assert!(
            notices.iter().any(|n| n.rule_id == "CLD_002"),
            "CLD_002 olmalı"
        );
        assert!(
            idx.removed.contains_key("GHOST" as &str),
            "Anahtar yine de removed'da olmalı"
        );
        assert!(
            idx.removed.get("GHOST" as &str).unwrap().is_empty(),
            "Geçersiz tarih Vec'e girmemeli"
        );
        assert_eq!(*idx.exception_count.get("GHOST" as &str).unwrap(), 1);
    }

    /// Invalid date: exception_count increases, first_line is recorded, and a notice is emitted.
    #[test]
    fn invalid_date_counted_but_not_indexed() {
        let file = make_file(
            vec!["service_id", "date", "exception_type"],
            vec![vec!["SVC1", "bad", "1"]],
        );
        let (idx, notices) = validate_calendar_dates(&file, None);
        assert!(notices.iter().any(|n| n.rule_id == "CLD_002"));
        assert_eq!(*idx.exception_count.get("SVC1" as &str).unwrap(), 1);
        assert_eq!(
            *idx.first_line.get("SVC1" as &str).unwrap(),
            2,
            "İlk satır numarası 2 olmalı"
        );
        // Date is invalid; it has no place in the added Vec.
        assert!(idx.added.get("SVC1" as &str).is_none_or(|v| v.is_empty()));
    }

    /// first_line always shows the first row seen.
    #[test]
    fn first_line_tracks_first_occurrence() {
        let file = make_file(
            vec!["service_id", "date", "exception_type"],
            vec![vec!["SVC1", "20260601", "1"], vec!["SVC1", "20260602", "1"]],
        );
        let (idx, _) = validate_calendar_dates(&file, None);
        assert_eq!(
            *idx.first_line.get("SVC1" as &str).unwrap(),
            2,
            "first_line ilk satır (2) olmalı"
        );
    }
}
