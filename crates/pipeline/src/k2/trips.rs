use std::borrow::Cow;
use std::io::Cursor;

use gtfs_core::EntityType;
use rustc_hash::FxHashMap;
use smol_str::SmolStr;

use super::common::{get_col, get_col_raw, make_k2_notice, parse_f64_col, parse_u32_col};
use super::stop_times::{next_csv_record, ZipCsvReader};
use crate::k1_parse::RawFile;

// Keep TripRecord size fixed — CI fails if adding a field bloats the struct.
const _: () = assert!(std::mem::size_of::<TripRecord>() <= 128);

/// String interning table — resolves TripRecord u32 indices.
/// Index 0 = empty string / None sentinel (every table stores 0 for None/absent).
#[derive(Debug, Default)]
pub struct TripInternTable {
    pub route_ids: Vec<SmolStr>,
    pub service_ids: Vec<SmolStr>,
    pub shape_ids: Vec<SmolStr>,
    pub headsigns: Vec<SmolStr>,
    pub short_names: Vec<SmolStr>,
    /// Header presence is kept separately because an entirely empty optional column
    /// is different from a missing column when translations.field_value is checked.
    pub has_headsign_field: bool,
    pub has_short_name_field: bool,
    pub has_jp_trip_desc_field: bool,
    pub block_ids: Vec<SmolStr>,
    pub jp_offices: Vec<SmolStr>,
    pub jp_patterns: Vec<SmolStr>,
    /// GTFS-JP translation coverage only; sparse so ordinary trips pay no per-row cost.
    pub jp_descriptions: FxHashMap<SmolStr, SmolStr>,
}

impl TripInternTable {
    pub fn new() -> Self {
        let empty = || vec![SmolStr::default()];
        Self {
            route_ids: empty(),
            service_ids: empty(),
            shape_ids: empty(),
            headsigns: empty(),
            short_names: empty(),
            has_headsign_field: false,
            has_short_name_field: false,
            has_jp_trip_desc_field: false,
            block_ids: empty(),
            jp_offices: empty(),
            jp_patterns: empty(),
            jp_descriptions: FxHashMap::default(),
        }
    }

    #[inline]
    pub fn route_id<'a>(&'a self, t: &TripRecord) -> &'a str {
        self.route_ids
            .get(t.route_idx as usize)
            .map(SmolStr::as_str)
            .unwrap_or("")
    }
    #[inline]
    pub fn service_id<'a>(&'a self, t: &TripRecord) -> &'a str {
        self.service_ids
            .get(t.service_idx as usize)
            .map(SmolStr::as_str)
            .unwrap_or("")
    }
    #[inline]
    pub fn shape_id<'a>(&'a self, t: &TripRecord) -> Option<&'a str> {
        if t.shape_idx == 0 {
            return None;
        }
        self.shape_ids
            .get(t.shape_idx as usize)
            .map(SmolStr::as_str)
    }
    #[inline]
    pub fn headsign<'a>(&'a self, t: &TripRecord) -> Option<&'a str> {
        if t.headsign_idx == 0 {
            return None;
        }
        self.headsigns
            .get(t.headsign_idx as usize)
            .map(SmolStr::as_str)
    }
    #[inline]
    pub fn short_name<'a>(&'a self, t: &TripRecord) -> Option<&'a str> {
        if t.short_name_idx == 0 {
            return None;
        }
        self.short_names
            .get(t.short_name_idx as usize)
            .map(SmolStr::as_str)
    }
    #[inline]
    pub fn block_id<'a>(&'a self, t: &TripRecord) -> Option<&'a str> {
        if t.block_idx == 0 {
            return None;
        }
        self.block_ids
            .get(t.block_idx as usize)
            .map(SmolStr::as_str)
    }
    #[inline]
    pub fn jp_office_id<'a>(&'a self, t: &TripRecord) -> Option<&'a str> {
        if t.jp_office_idx == 0 {
            return None;
        }
        self.jp_offices
            .get(t.jp_office_idx as usize)
            .map(SmolStr::as_str)
    }
    #[inline]
    pub fn jp_pattern_id<'a>(&'a self, t: &TripRecord) -> Option<&'a str> {
        if t.jp_pattern_idx == 0 {
            return None;
        }
        self.jp_patterns
            .get(t.jp_pattern_idx as usize)
            .map(SmolStr::as_str)
    }
    #[inline]
    pub fn jp_trip_desc<'a>(&'a self, t: &TripRecord) -> Option<&'a str> {
        self.jp_descriptions.get(&t.trip_id).map(SmolStr::as_str)
    }
}

#[derive(Debug, Clone)]
pub struct TripRecord {
    pub trip_id: SmolStr,
    pub route_idx: u32,
    pub service_idx: u32,
    pub shape_idx: u32,
    pub headsign_idx: u32,
    pub short_name_idx: u32,
    pub block_idx: u32,
    pub jp_office_idx: u32,
    pub jp_pattern_idx: u32,
    pub direction_id: Option<u32>,
    pub wheelchair_accessible: Option<u32>,
    pub bikes_allowed: Option<u32>,
    pub cars_allowed: Option<u32>,
    pub safe_duration_factor: Option<f64>,
    /// Spec type **Float** (seconds). Until 2026-08-02 this was read as `u32` → a VALID
    /// value such as `12.5` was silently discarded.
    pub safe_duration_offset: Option<f64>,
    pub line: u64,
}

/// String interning helper — empty string → 0 sentinel, otherwise insert/find in the table.
#[inline]
/// Optional IDENTITY field (#85): store the raw value, but a whitespace-only value DOES NOT EXIST.
///
/// `mdb-1309`/`tfs-109`: all 340 trips had `block_id = " "`. Treating one space as a block ID
/// put every trip on the same vehicle and produced 286 `TRP_022` "conflicting block" findings;
/// the same pattern produced `SHP_019` for `shape_id = " "`. The whitespace itself belongs to `DQ_016`.
fn optional_id(raw: &str) -> &str {
    if raw.trim().is_empty() {
        ""
    } else {
        raw
    }
}

fn intern_idx(raw: &str, table: &mut Vec<SmolStr>, map: &mut FxHashMap<String, u32>) -> u32 {
    if raw.is_empty() {
        return 0;
    }
    if let Some(&idx) = map.get(raw) {
        return idx;
    }
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
    jp_pattern_id: Option<usize>,
    jp_trip_desc: Option<usize>,
}

impl Cols {
    fn from_headers(headers: &[String]) -> Self {
        let pos = |name: &str| headers.iter().position(|h| h == name);
        Self {
            trip_id: pos("trip_id"),
            route_id: pos("route_id"),
            service_id: pos("service_id"),
            shape_id: pos("shape_id"),
            trip_headsign: pos("trip_headsign"),
            trip_short_name: pos("trip_short_name"),
            direction_id: pos("direction_id"),
            block_id: pos("block_id"),
            wheelchair_accessible: pos("wheelchair_accessible"),
            bikes_allowed: pos("bikes_allowed"),
            cars_allowed: pos("cars_allowed"),
            safe_duration_factor: pos("safe_duration_factor"),
            safe_duration_offset: pos("safe_duration_offset"),
            jp_office_id: pos("jp_office_id"),
            jp_pattern_id: pos("jp_pattern_id"),
            jp_trip_desc: pos("jp_trip_desc"),
        }
    }
}

/// Planner context for a trips.txt row finding without its own identity (TRP_031/TRP_035):
/// the row's other columns that name the trip (raw, trimmed; only when non-empty).
fn row_context(
    n: &mut gtfs_core::Notice,
    row: &[std::borrow::Cow<'_, str>],
    cols: &[(&str, Option<usize>)],
) {
    let details = n.details.get_or_insert_with(Default::default);
    for (key, col) in cols {
        let value = get_col_raw(row, *col).trim();
        if !value.is_empty() {
            details.insert(*key, value.to_string());
        }
    }
}

pub fn validate_trips(
    file: &RawFile,
    zip_bytes: Option<&[u8]>,
) -> (Vec<TripRecord>, TripInternTable, Vec<gtfs_core::Notice>) {
    validate_trips_with_limits(file, zip_bytes, None, None)
}

pub fn validate_trips_with_limits(
    file: &RawFile,
    zip_bytes: Option<&[u8]>,
    max_data_rows: Option<usize>,
    mut stream_budget: Option<&mut super::stop_times::StreamBudget>,
) -> (Vec<TripRecord>, TripInternTable, Vec<gtfs_core::Notice>) {
    // #38: only the header of trips.txt is read in K1; K2 streams it from zip_bytes.
    // The old rows path is retained as a test/legacy fallback.
    let mut notices = Vec::new();
    // ZIP central-directory sizes are attacker-controlled metadata. Grow from
    // observed records rather than trusting the declared size.
    let est_rows = if zip_bytes.is_some() {
        0
    } else {
        file.raw_text
            .as_ref()
            .map(|t| t.len() / 60)
            .unwrap_or(file.rows.len())
    };
    let mut records: Vec<TripRecord> = Vec::with_capacity(est_rows);
    let mut counter = 0u32;

    let cols = Cols::from_headers(&file.headers);
    let mut trp021_missing_count: usize = 0;
    let mut trp021_missing_examples: Vec<SmolStr> = Vec::new();
    let mut trp021_first_line: Option<u64> = None;
    let mut bikes_allowed_set_count: u32 = 0;
    let mut dq016 = crate::k1_parse::Dq016Acc::default();
    let mut arc037 = crate::k1_parse::Arc037Acc::default();
    let mut arc032 = crate::k1_parse::Arc032Acc::default();
    let mut arc021_fired = false;
    let mut arc030_fired = false;

    let has_trip_id_col = file.headers.iter().any(|h| h == "trip_id");
    let has_route_id_col = file.headers.iter().any(|h| h == "route_id");
    let has_service_id_col = file.headers.iter().any(|h| h == "service_id");

    // Intern table + per-field lookup maps
    let mut interns = TripInternTable::new();
    interns.has_headsign_field = cols.trip_headsign.is_some();
    interns.has_short_name_field = cols.trip_short_name.is_some();
    interns.has_jp_trip_desc_field = cols.jp_trip_desc.is_some();
    let mut route_map: FxHashMap<String, u32> = FxHashMap::default();
    let mut service_map: FxHashMap<String, u32> = FxHashMap::default();
    let mut shape_map: FxHashMap<String, u32> = FxHashMap::default();
    let mut headsign_map: FxHashMap<String, u32> = FxHashMap::default();
    let mut short_name_map: FxHashMap<String, u32> = FxHashMap::default();
    let mut block_map: FxHashMap<String, u32> = FxHashMap::default();
    let mut jp_office_map: FxHashMap<String, u32> = FxHashMap::default();
    let mut jp_pattern_map: FxHashMap<String, u32> = FxHashMap::default();

        // Row processor — called by both the stream (raw_text) and rows fallback paths.
    let header_count = file.headers.len();
    let mut process = |row: &[Cow<'_, str>], line: u64| {
        // ARC_012: row field count does not match the header.
        //
        // 🔴 Found in corpus run 17: because this file is in `is_zip_stream`, K1 reads ONLY the
        // header and NEVER enters K1's row loop. `stop_times.rs` and `shapes.rs` moved the check
        // into the stream path, but `trips.rs` and `calendar_dates.rs` did not — row length was
        // therefore NEVER checked in these two files.
        // The `ARC_034` comment in this file already described the mechanism (#181); `ARC_012`
        // was missed in that pass.
        // Evidence `mdb-2013`: all 15,170 rows were malformed; MD reported 38,893
        // `invalid_row_length`, while we reported 23,723 — exactly the trips.txt share.
        // ⚠️ The rule is Kritik·Spec, so it is visible to the R1 publication gate.
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

        // ARC_034: repeated header row — notice + DO NOT STORE the row (K1's continue).
        // This file is streamed in K1 but only its header is read there, so the check would
        // NEVER run for this file if it stayed in K1 (#181).
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
        // ARC_018/021/030 were not moved in #181: this file is streamed in K1 but only the
        // header is read there, so the check never ran (2026-10-13). An empty row would derive TRP_001, etc.
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
        // ARC_021: non-printable/problematic character — once per file
        if !arc021_fired {
            if let Some(cp) = crate::k1_parse::arc021_bad_char(row.iter().map(|v| v.as_ref())) {
                arc021_fired = true;
                let mut arc021 = make_k2_notice(
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
                );
                crate::k1_parse::add_arc021_context(&mut arc021, &file.headers, row.iter().map(|v| v.as_ref()));
                notices.push(arc021);
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
        dq016.observe(line, row.iter().map(|value| value.as_ref()), &file.headers);
        arc037.observe(line, row.iter().map(|value| value.as_ref()), &file.headers);
        arc032.observe(line, row.iter().map(|value| value.as_ref()), &file.headers);
        let trip_id = SmolStr::new(get_col_raw(row, cols.trip_id));
        let entity_id = (!trip_id.is_empty()).then(|| trip_id.to_string());

        // TRP_001: trip_id required
        if trip_id.is_empty() && has_trip_id_col {
            let mut n = make_k2_notice(
                &mut counter,
                "TRP_001",
                EntityType::Trip,
                None,
                None,
                &file.name,
                Some(line),
                Some("trip_id"),
                Some(String::new()),
                None,
                "trip_id zorunludur.".to_string(),
                "Assign a unique trip_id to each trip.",
            );
            // A row without an identity cannot say which trip is its root: if scope_key remains
            // None, `blocks` suppresses ALL STM_001/FRQ_001/XFL_002/OPR_006/OPR_007 findings in
            // the feed. A duplicate trip_id variant (K3) remains the broad root.
            n.blocks.clear();
            // Planner context: the row's route, calendar and headsign name the code-less trip.
            let details = n.details.get_or_insert_with(Default::default);
            for (key, col) in [
                ("route_id", cols.route_id),
                ("service_id", cols.service_id),
                ("trip_headsign", cols.trip_headsign),
            ] {
                let value = get_col_raw(row, col).trim();
                if !value.is_empty() {
                    details.insert(key, value.to_string());
                }
            }
            notices.push(n);
        }

        let route_idx = intern_idx(
            get_col_raw(row, cols.route_id),
            &mut interns.route_ids,
            &mut route_map,
        );
        let service_idx = intern_idx(
            get_col_raw(row, cols.service_id),
            &mut interns.service_ids,
            &mut service_map,
        );

        // TRP_031: route_id required — intern_idx 0 means it is empty
        if route_idx == 0 && has_route_id_col {
            let mut n = make_k2_notice(
                &mut counter,
                "TRP_031",
                EntityType::Trip,
                None,
                None,
                &file.name,
                Some(line),
                Some("route_id"),
                Some(String::new()),
                None,
                "route_id zorunludur.".to_string(),
                "Assign a valid route_id to every trip.",
            );
            row_context(
                &mut n,
                row,
                &[
                    ("trip_id", cols.trip_id),
                    ("service_id", cols.service_id),
                    ("trip_headsign", cols.trip_headsign),
                ],
            );
            notices.push(n);
        }

        // TRP_035: service_id is required. TRP_003 ("service_id not found") measures only FK
        // resolution and treats an empty value as "no reference", letting it pass — found during
        // anchor-granularity triage (24 rows / 5 feeds in the corpus).
        if service_idx == 0 && has_service_id_col {
            let mut n = make_k2_notice(
                &mut counter,
                "TRP_035",
                EntityType::Trip,
                None,
                None,
                &file.name,
                Some(line),
                Some("service_id"),
                Some(String::new()),
                Some("(dolu)".to_string()),
                "service_id zorunludur; boş bırakılan sefer hiçbir takvime bağlanmaz.".to_string(),
                "Assign every trip a service_id defined in calendar.txt or calendar_dates.txt.",
            );
            row_context(
                &mut n,
                row,
                &[
                    ("trip_id", cols.trip_id),
                    ("route_id", cols.route_id),
                    ("trip_headsign", cols.trip_headsign),
                ],
            );
            notices.push(n);
        }

        let shape_idx = intern_idx(
            optional_id(get_col_raw(row, cols.shape_id)),
            &mut interns.shape_ids,
            &mut shape_map,
        );

        // Get raw strings for trip_headsign and trip_short_name first (for TRP_014).
        let headsign_raw = get_col(row, cols.trip_headsign);
        let short_name_raw = get_col(row, cols.trip_short_name);

        // TRP_014: trip_short_name is too long (>20 characters)
        // Count CHARACTERS, not BYTES: `str::len()` returns UTF-8 bytes and triggered the
        // threshold early in multibyte alphabets (Turkish, Japanese).
        let short_name_chars = short_name_raw.chars().count();
        if !short_name_raw.is_empty() && short_name_chars > 20 {
            let mut n = make_k2_notice(
                &mut counter,
                "TRP_014",
                EntityType::Trip,
                entity_id.clone(),
                None,
                &file.name,
                Some(line),
                Some("trip_short_name"),
                Some(short_name_chars.to_string()),
                Some("≤20".to_string()),
                format!(
                    "trip_short_name {} karakter; 20'yi aşmamalıdır.",
                    short_name_chars
                ),
                "Shorten trip_short_name.",
            );
            // Planner context: the trip number as written.
            n.details
                .get_or_insert_with(Default::default)
                .insert("trip_short_name", short_name_raw.to_string());
            notices.push(n);
        }

        let headsign_idx = intern_idx(headsign_raw, &mut interns.headsigns, &mut headsign_map);
        let short_name_idx = intern_idx(
            short_name_raw,
            &mut interns.short_names,
            &mut short_name_map,
        );
        let block_idx = intern_idx(
            optional_id(get_col_raw(row, cols.block_id)),
            &mut interns.block_ids,
            &mut block_map,
        );
        let jp_office_idx = intern_idx(
            optional_id(get_col_raw(row, cols.jp_office_id)),
            &mut interns.jp_offices,
            &mut jp_office_map,
        );
        let jp_pattern_idx = intern_idx(
            optional_id(get_col_raw(row, cols.jp_pattern_id)),
            &mut interns.jp_patterns,
            &mut jp_pattern_map,
        );
        let jp_desc_raw = get_col_raw(row, cols.jp_trip_desc);
        if !jp_desc_raw.is_empty() {
            interns
                .jp_descriptions
                .insert(trip_id.clone(), SmolStr::new(jp_desc_raw));
        }

        // TRP_005: direction_id must be 0 or 1
        let dir_raw = get_col(row, cols.direction_id);
        let direction_id = match parse_u32_col(dir_raw) {
            Ok(v) => {
                if let Some(val) = v {
                    if val > 1 {
                        notices.push(make_k2_notice(
                            &mut counter,
                            "TRP_005",
                            EntityType::Trip,
                            entity_id.clone(),
                            None,
                            &file.name,
                            Some(line),
                            Some("direction_id"),
                            Some(val.to_string()),
                            Some("0 or 1".to_string()),
                            format!("direction_id {val} geçersiz; 0 veya 1 olmalıdır."),
                            "direction_id değerini 0 (gidiş) veya 1 (dönüş) olarak ayarlayın.",
                        ));
                    }
                }
                v
            }
            Err(_) => {
                notices.push(make_k2_notice(
                    &mut counter,
                    "TRP_005",
                    EntityType::Trip,
                    entity_id.clone(),
                    None,
                    &file.name,
                    Some(line),
                    Some("direction_id"),
                    Some(dir_raw.to_string()),
                    Some("0 or 1".to_string()),
                    format!("direction_id '{dir_raw}' geçersiz; 0 veya 1 olmalıdır."),
                    "direction_id değerini 0 (gidiş) veya 1 (dönüş) olarak ayarlayın.",
                ));
                None
            }
        };

        // TRP_006: wheelchair_accessible must be 0, 1, or 2
        let wc_raw = get_col(row, cols.wheelchair_accessible);
        let wheelchair_accessible = match parse_u32_col(wc_raw) {
            Ok(v) => {
                if let Some(val) = v {
                    if val > 2 {
                        notices.push(make_k2_notice(
                            &mut counter, "TRP_006", EntityType::Trip, entity_id.clone(),
                            None, &file.name, Some(line), Some("wheelchair_accessible"),
                            Some(val.to_string()), Some("0, 1 or 2".to_string()),
                            "wheelchair_accessible 0, 1 veya 2 olmalıdır.".to_string(),
                            "Set wheelchair_accessible to 0 (no info), 1 (accessible) or 2 (not accessible).",
                        ));
                    }
                }
                v
            }
            Err(()) => {
                // A NON-NUMERIC value used to be silently dropped: an out-of-range number produced
                // TRP_006, while "abc" produced no finding. Two branches of the same fact → one rule
                // (the PTH_027 precedent).
                notices.push(make_k2_notice(
                    &mut counter, "TRP_006", EntityType::Trip, entity_id.clone(),
                    None, &file.name, Some(line), Some("wheelchair_accessible"),
                    Some(wc_raw.to_string()), Some("0, 1 or 2".to_string()),
                    format!("wheelchair_accessible '{}' sayı olarak okunamıyor.", wc_raw),
                    "Set wheelchair_accessible to 0 (no info), 1 (accessible) or 2 (not accessible).",
                ));
                None
            }
        };

        // TRP_007: bikes_allowed must be 0, 1, or 2
        let ba_raw = get_col(row, cols.bikes_allowed);
        let bikes_allowed = match parse_u32_col(ba_raw) {
            Ok(v) => {
                if let Some(val) = v {
                    if val > 2 {
                        notices.push(make_k2_notice(
                            &mut counter, "TRP_007", EntityType::Trip, entity_id.clone(),
                            None, &file.name, Some(line), Some("bikes_allowed"),
                            Some(val.to_string()), Some("0, 1 or 2".to_string()),
                            "bikes_allowed 0, 1 veya 2 olmalıdır.".to_string(),
                            "Set bikes_allowed to 0 (no info), 1 (allowed) or 2 (not allowed).",
                        ));
                    }
                }
                v
            }
            Err(()) => {
                // A NON-NUMERIC value used to be silently dropped: an out-of-range number produced
                // TRP_007, while "abc" produced no finding. Two branches of the same fact → one rule
                // (the PTH_027 precedent).
                notices.push(make_k2_notice(
                    &mut counter, "TRP_007", EntityType::Trip, entity_id.clone(),
                    None, &file.name, Some(line), Some("bikes_allowed"),
                    Some(ba_raw.to_string()), Some("0, 1 or 2".to_string()),
                    format!("bikes_allowed '{}' sayı olarak okunamıyor.", ba_raw),
                    "Set bikes_allowed to 0 (no info), 1 (allowed) or 2 (not allowed).",
                ));
                None
            }
        };

        // TRP_021: bike-information absence counter. Spec: "0 or empty - No bike information";
        // 0 means no information; only 1/2 carry information (corpus mdb-1257, 2026-10-13).
        if (bikes_allowed.is_none() && ba_raw.is_empty()) || bikes_allowed == Some(0) {
            trp021_missing_count += 1;
            if trp021_first_line.is_none() {
                trp021_first_line = Some(line);
            }
            if trp021_missing_examples.len() < 5 && !trip_id.is_empty() {
                trp021_missing_examples.push(trip_id.clone());
            }
        } else if matches!(bikes_allowed, Some(1 | 2)) {
            bikes_allowed_set_count += 1;
        }

        // TRP_032: cars_allowed must be 0, 1, or 2
        let ca_raw = get_col(row, cols.cars_allowed);
        let cars_allowed = match parse_u32_col(ca_raw) {
            Ok(v) => {
                if let Some(val) = v {
                    if val > 2 {
                        notices.push(make_k2_notice(
                            &mut counter, "TRP_032", EntityType::Trip, entity_id.clone(),
                            None, &file.name, Some(line), Some("cars_allowed"),
                            Some(val.to_string()), Some("0, 1 or 2".to_string()),
                            "cars_allowed 0, 1 veya 2 olmalıdır.".to_string(),
                            "Set cars_allowed to 0 (no info), 1 (allowed) or 2 (not allowed).",
                        ));
                    }
                }
                v
            }
            Err(()) => {
                notices.push(make_k2_notice(
                    &mut counter, "TRP_032", EntityType::Trip, entity_id.clone(),
                    None, &file.name, Some(line), Some("cars_allowed"),
                    Some(ca_raw.to_string()), Some("0, 1 or 2".to_string()),
                    format!("cars_allowed '{}' sayı olarak okunamıyor.", ca_raw),
                    "Set cars_allowed to 0 (no info), 1 (allowed) or 2 (not allowed).",
                ));
                None
            }
        };

        // TRP_034: both fields are `Float` in the spec. Parse errors used to be swallowed by
        // `.ok().flatten()`; offset was also read as `u32`, dropping fractional-second values.
        let mut parse_safe = |raw: &str, field: &'static str| -> Option<f64> {
            match parse_f64_col(raw) {
                Ok(v) => v,
                Err(()) => {
                    notices.push(make_k2_notice(
                        &mut counter,
                        "TRP_034",
                        EntityType::Trip,
                        Some(trip_id.to_string()),
                        None,
                        &file.name,
                        Some(line),
                        Some(field),
                        Some(raw.to_string()),
                        Some("ondalık sayı".to_string()),
                        format!("{field} '{raw}' ondalık sayı olarak okunamıyor."),
                        "Enter the safe_duration fields as decimal numbers.",
                    ));
                    None
                }
            }
        };
        let safe_duration_factor = parse_safe(
            get_col(row, cols.safe_duration_factor),
            "safe_duration_factor",
        );
        let safe_duration_offset = parse_safe(
            get_col(row, cols.safe_duration_offset),
            "safe_duration_offset",
        );

        records.push(TripRecord {
            trip_id,
            route_idx,
            service_idx,
            shape_idx,
            headsign_idx,
            short_name_idx,
            block_idx,
            jp_office_idx,
            jp_pattern_idx,
            direction_id,
            wheelchair_accessible,
            bikes_allowed,
            cars_allowed,
            safe_duration_factor,
            safe_duration_offset,
            line,
        });
    };

    // ARC_033 accumulation: READERS see the violation (only they know field boundaries),
    // and this loop adds the line number. Emit in one place: `arc033_summary`.
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
                    "Check the ZIP archive.",
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
                            "Check the ZIP archive.",
                        ));
                    }
                    Ok(entry) => {
                        let stream_limit = stream_budget
                            .as_ref()
                            .map(|budget| budget.limit())
                            .unwrap_or(super::stop_times::stream_byte_limit(max_data_rows));
                        let mut csv_reader =
                            ZipCsvReader::with_limits(entry, stream_limit, max_data_rows);
                        let mut raw_fields: Vec<Vec<u8>> = Vec::with_capacity(16);
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
                        if let Some(budget) = stream_budget.as_mut() {
                            budget.consume(csv_reader.bytes_read());
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
        // Fallback (test/legacy): use pre-parsed rows.
        for (row_idx, row) in file.rows.iter().enumerate() {
            let cow_row: Vec<Cow<'_, str>> =
                row.iter().map(|s| Cow::Borrowed(s.as_str())).collect();
            process(&cow_row, (row_idx + 2) as u64);
        }
    }

    // DQ_016: one root summary per FILE for streamed trips.txt.
    // ARC_032: HTML marker — ONE summary per FILE (same as K1). It did not run for streamed
    // files until 2026-10-13 (trip_headsign/stop_headsign were not checked).
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

    // TRP_021: one summary notice after the loop
    if trp021_missing_count > 0 {
        let total = trp021_missing_count + bikes_allowed_set_count as usize;
        // Planner context: counts and the first trips without bicycle information.
        let trp021_details = |n: &mut gtfs_core::Notice| {
            let details = n.details.get_or_insert_with(Default::default);
            details.insert("missing_trips", trp021_missing_count.to_string());
            details.insert("trip_count", total.to_string());
            if !trp021_missing_examples.is_empty() {
                details.insert("example_trips", trp021_missing_examples.join(","));
            }
        };
        if bikes_allowed_set_count == 0 {
            let mut n = make_k2_notice(
                &mut counter, "TRP_021", EntityType::Trip, None,
                None, &file.name, None, Some("bikes_allowed"),
                Some(format!("{trp021_missing_count}/{total}")), Some("1|2".to_string()),
                format!("Bu feed'de hiçbir seferde bisiklet bilgisi yok: bikes_allowed boş veya 0 ({trp021_missing_count} sefer)."),
                "Set bikes_allowed to 1 (bikes allowed) or 2 (bikes not allowed); 0 and empty mean no information.",
            );
            trp021_details(&mut n);
            notices.push(n);
        } else {
            let examples = if trp021_missing_examples.is_empty() {
                String::new()
            } else {
                format!(
                    " Örnek seferler: {}{}.",
                    trp021_missing_examples.join(", "),
                    if trp021_missing_count > trp021_missing_examples.len() {
                        ", …"
                    } else {
                        ""
                    }
                )
            };
            let mut n = make_k2_notice(
                &mut counter, "TRP_021", EntityType::Trip, None,
                None, &file.name, trp021_first_line, Some("bikes_allowed"),
                Some(format!("{trp021_missing_count}/{total}")), Some("1|2".to_string()),
                format!("{total} seferin {trp021_missing_count} tanesinde bisiklet bilgisi yok: bikes_allowed boş veya 0 ({:.0}%).{examples}",
                    trp021_missing_count as f64 / total as f64 * 100.0),
                "Fill in bikes_allowed on every trip with 1 (bikes allowed) or 2 (bikes not allowed); 0 and empty mean no information.",
            );
            trp021_details(&mut n);
            notices.push(n);
        }
    }

    // ARC_013: unclosed quote in the stream body (issue #84) — one notice per FILE.
    if scan.unclosed || zip_unclosed {
        notices.push(super::common::arc013_unclosed_stream(
            &file.name,
            &mut counter,
        ));
    }

    // ARC_033: ONE summary per FILE (lesson from #75: no duplicate validation).
    if let Some(n) = super::common::arc033_summary(&rfc_acc, &file.name, &mut counter) {
        notices.push(n);
    }

    // ARC_035: `trips.txt` is REQUIRED and its header-only form produced NO finding until
    // 2026-08-31 — because the file is in `is_zip_stream`, K1's empty-file check never sees it,
    // and K2 had no counterpart. (The 0-byte form was caught by K1's `records.is_empty()` branch;
    // the distinction was invisible.)
    if (file.raw_text.is_some() || zip_bytes.is_some()) && records.is_empty() {
        notices.push(make_k2_notice(
            &mut counter,
            "ARC_035",
            EntityType::File,
            Some(file.name.clone()),
            None,
            &file.name,
            None,
            None,
            Some(file.name.clone()),
            None,
            format!(
                "Zorunlu GTFS dosyası '{}' hiç veri satırı içermiyor.",
                file.name
            ),
            "Dosyaya gerçek verileri ekleyin; boş bırakmak dosyayı eklememekle aynıdır.",
        ));
    }

    (records, interns, notices)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::k1_parse::RawFile;

    // Tests use the rows fallback path (raw_text=None).
    fn make_file(headers: Vec<&str>, rows: Vec<Vec<&str>>) -> RawFile {
        RawFile {
            name: "trips.txt".to_string(),
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

    // Test the streaming path — shapes.rs pattern.
    fn make_file_streaming(headers: Vec<&str>, rows: Vec<Vec<&str>>) -> RawFile {
        let header_line = headers.join(",");
        let data_lines: Vec<String> = rows.iter().map(|r| r.join(",")).collect();
        let text = format!("{}\n{}\n", header_line, data_lines.join("\n"));
        RawFile {
            name: "trips.txt".to_string(),
            headers: headers.into_iter().map(str::to_string).collect(),
            rows: vec![],
            bytes: text.len() as u32,
            raw_text: Some(text),
            zip_entry_name: None,
        }
    }

    /// Measurement, corpus run 17 — blind spot: `trips.txt` is in `is_zip_stream`, so only its
    /// header is read in K1 and K1's row loop NEVER enters it. Since `ARC_012` was left there,
    /// it never ran for this file. Evidence `mdb-2013`: all 15,170 malformed rows were silent;
    /// MD reported 38,893 `invalid_row_length`, while we reported 23,723.
    #[test]
    fn arc012_fires_on_the_streaming_path_for_trips() {
        let file = make_file_streaming(
            vec!["route_id", "service_id", "trip_id"],
            vec![vec!["R1", "SVC1", "T1"], vec!["R2", "SVC2", "T2", "FAZLA"]],
        );
        let (_records, _ti, notices) = validate_trips(&file, None);
        let hit = notices
            .iter()
            .find(|n| n.rule_id == "ARC_012")
            .expect("fazla alanlı satır ARC_012 üretmeli");
        assert_eq!(hit.file.as_deref(), Some("trips.txt"));
        assert_eq!(hit.line, Some(3));
        assert_eq!(hit.severity, gtfs_core::Severity::Kritik);
    }

    /// A missing trailing field is valid CSV practice → Info, NOT Critical.
    #[test]
    fn arc012_downgrades_a_short_trips_row_to_info() {
        let file = make_file_streaming(
            vec!["route_id", "service_id", "trip_id", "trip_headsign"],
            vec![vec!["R1", "SVC1", "T1"]],
        );
        let (_records, _ti, notices) = validate_trips(&file, None);
        let hit = notices
            .iter()
            .find(|n| n.rule_id == "ARC_012")
            .expect("kısa satır ARC_012 üretmeli");
        assert_eq!(hit.severity, gtfs_core::Severity::Bilgi);
    }

    #[test]
    fn valid_trip_produces_no_notices() {
        let file = make_file(
            vec!["route_id", "service_id", "trip_id", "bikes_allowed"],
            // 0 means "no information" and produces TRP_021 (spec); values 1/2 carry information.
            vec![vec!["R1", "SVC1", "T1", "1"]],
        );
        let (records, _ti, notices) = validate_trips(&file, None);
        assert_eq!(records.len(), 1);
        assert!(
            notices.is_empty(),
            "Geçerli sefer notice üretmemeli: {:?}",
            notices
        );
    }

    #[test]
    fn streaming_matches_rows_path() {
        let headers = vec![
            "route_id",
            "service_id",
            "trip_id",
            "direction_id",
            "bikes_allowed",
        ];
        let rows = vec![
            vec!["R1", "SVC1", "T1", "0", "1"],
            vec!["R2", "SVC2", "T2", "1", "2"],
        ];
        let (rec_rows, ti_rows, notices_rows) =
            validate_trips(&make_file(headers.clone(), rows.clone()), None);
        let (rec_stream, ti_stream, notices_stream) =
            validate_trips(&make_file_streaming(headers, rows), None);
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
        assert!(
            notices.iter().any(|n| n.rule_id == "TRP_005"),
            "direction_id=9 must produce TRP_005, got: {:?}",
            notices.iter().map(|n| &n.rule_id).collect::<Vec<_>>()
        );
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
    fn non_numeric_cars_allowed_produces_trp_032() {
        let file = make_file(
            vec!["route_id", "service_id", "trip_id", "cars_allowed"],
            vec![vec!["R1", "SVC1", "T1", "abc"]],
        );
        let (_, _ti, notices) = validate_trips(&file, None);
        assert!(notices.iter().any(|n| n.rule_id == "TRP_032"));
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
