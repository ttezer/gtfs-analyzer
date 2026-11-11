use std::borrow::Cow;
use std::collections::{HashMap, HashSet};

use gtfs_core::{EntityType, Severity};

use super::common::{get_col, get_col_raw, make_k2_notice, parse_f64_col, parse_u32_col};
use super::stop_times::{next_csv_record, ZipCsvReader};
use crate::k1_parse::RawFile;

/// shape_id interning table — same pattern as `TripInternTable`.
///
/// In shapes.txt every point used to carry its own `shape_id`: 40.9M `String` values in Entur,
/// but only 28,604 distinct values (average ~48 characters, too large for SmolStr's 23-byte
/// inline limit → a separate heap allocation for each, ≈2.4 GB). Each point now stores a
/// 4-byte index and the text is kept once in this table.
///
/// Index 0 means an empty/missing shape_id (as in the trips pattern).
#[derive(Debug, Clone)]
pub struct ShapeInternTable {
    ids: Vec<smol_str::SmolStr>,
}

impl Default for ShapeInternTable {
    fn default() -> Self {
        Self::new()
    }
}

impl ShapeInternTable {
    pub fn new() -> Self {
        Self {
            ids: vec![smol_str::SmolStr::default()],
        }
    }
    /// The point's shape_id text; "" for an unknown index or empty value.
    #[inline]
    pub fn id<'a>(&'a self, sp: &ShapePointRecord) -> &'a str {
        self.ids
            .get(sp.shape_idx as usize)
            .map(smol_str::SmolStr::as_str)
            .unwrap_or("")
    }
    /// Text from an index (when the point is not available).
    #[inline]
    pub fn id_at(&self, idx: u32) -> &str {
        self.ids
            .get(idx as usize)
            .map(smol_str::SmolStr::as_str)
            .unwrap_or("")
    }
    pub fn len(&self) -> usize {
        self.ids.len()
    }
    pub fn is_empty(&self) -> bool {
        self.ids.len() <= 1
    }

    /// Interns text and returns its index. Empty text → 0.
    /// The parse path uses its own hash map (no linear search over 40.9M rows);
    /// this method is for small tables and test setup.
    pub fn intern(&mut self, id: &str) -> u32 {
        if id.is_empty() {
            return 0;
        }
        if let Some(pos) = self.ids.iter().position(|x| x == id) {
            return pos as u32;
        }
        self.ids.push(smol_str::SmolStr::from(id));
        (self.ids.len() - 1) as u32
    }
}

/// One shapes.txt point.
///
/// Memory: feeds such as Entur (mdb-1078) hold 40.9M records, so every byte costs ×40.9M.
/// `Option<f64>`/`Option<u32>` wrappers do not carry a niche and wasted 8 bytes per field;
/// fields were converted to sentinels as in the `CompactStopTime` pattern, while the
/// `Option` API is preserved in accessors. `shape_id` is a `ShapeInternTable` index.
///
/// `shape_dist_traveled` deliberately remains f64 (`CompactStopTime` uses f32 there):
/// SHP_005 checks monotonicity with an EXACT comparison (#94), and K4::stm_shape_dist
/// compares this value with stop_times' — at 40.9M dense points, f32 rounding could
/// collapse adjacent values and hide real increases/decreases.
#[derive(Debug, Clone)]
pub struct ShapePointRecord {
    /// `ShapeInternTable` index; 0 = empty/missing shape_id
    shape_idx: u32,
    /// NaN = field missing/unparseable
    lat: f64,
    /// NaN = field missing/unparseable
    lon: f64,
    /// u32::MAX = field missing/unparseable
    seq: u32,
    /// NaN = field missing/unparseable
    dist: f64,
    pub line: u32,
}

impl ShapePointRecord {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        shape_idx: u32,
        shape_pt_lat: Option<f64>,
        shape_pt_lon: Option<f64>,
        shape_pt_sequence: Option<u32>,
        shape_dist_traveled: Option<f64>,
        line: u32,
    ) -> Self {
        Self {
            shape_idx,
            lat: shape_pt_lat.unwrap_or(f64::NAN),
            lon: shape_pt_lon.unwrap_or(f64::NAN),
            seq: shape_pt_sequence.unwrap_or(u32::MAX),
            dist: shape_dist_traveled.unwrap_or(f64::NAN),
            line,
        }
    }

    #[inline]
    pub fn shape_idx(&self) -> u32 {
        self.shape_idx
    }

    #[inline]
    pub fn shape_pt_lat(&self) -> Option<f64> {
        if self.lat.is_nan() {
            None
        } else {
            Some(self.lat)
        }
    }
    #[inline]
    pub fn shape_pt_lon(&self) -> Option<f64> {
        if self.lon.is_nan() {
            None
        } else {
            Some(self.lon)
        }
    }
    #[inline]
    pub fn shape_pt_sequence(&self) -> Option<u32> {
        if self.seq == u32::MAX {
            None
        } else {
            Some(self.seq)
        }
    }
    #[inline]
    pub fn shape_dist_traveled(&self) -> Option<f64> {
        if self.dist.is_nan() {
            None
        } else {
            Some(self.dist)
        }
    }
    /// The Notice API expects u64.
    #[inline]
    pub fn line_u64(&self) -> u64 {
        self.line as u64
    }
}

// Size guard: compilation fails if this exceeds 40 B.
// 88 B (with Option) → 56 B (sentinel) → 40 B (shape_id interning). ×40.9M points ≈ 1.9 GB
// for the structs, plus ≈2.4 GB from the eliminated 40.9M separate shape_id allocations.
const _: () = assert!(std::mem::size_of::<ShapePointRecord>() <= 40);

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
            shape_id: pos("shape_id"),
            shape_pt_lat: pos("shape_pt_lat"),
            shape_pt_lon: pos("shape_pt_lon"),
            shape_pt_sequence: pos("shape_pt_sequence"),
            shape_dist_traveled: pos("shape_dist_traveled"),
        }
    }
}

pub fn validate_shapes(
    file: &RawFile,
    zip_bytes: Option<&[u8]>,
) -> (
    Vec<ShapePointRecord>,
    ShapeInternTable,
    Vec<gtfs_core::Notice>,
) {
    validate_shapes_with_limits(file, zip_bytes, None, None)
}

pub fn validate_shapes_with_limits(
    file: &RawFile,
    zip_bytes: Option<&[u8]>,
    max_data_rows: Option<usize>,
    mut stream_budget: Option<&mut super::stop_times::StreamBudget>,
) -> (
    Vec<ShapePointRecord>,
    ShapeInternTable,
    Vec<gtfs_core::Notice>,
) {
        // #15 W3: shapes.txt is streamed in K1 (RawFile.rows is empty, body in raw_text). Here
        // it is parsed as a stream; K1's per-row generic notices (ARC_012/018/021, DQ_016)
        // were moved into this pass (stop_times pattern — same line/severity/rule behavior).
        // The old rows path is retained as a test/legacy fallback.
    let mut notices = Vec::new();
    // #15: pre-size the records Vec — the temporary 2× peak from doubling reallocations
    // reached ~500 MB in shape-heavy feeds (exposed after K1 began streaming).
    //
    // The old estimate assumed a FIXED "row ~30 bytes". In Entur (mdb-1078) the real average
    // was 67.9 bytes — due to long NeTEx shape_ids — and the estimate inflated by 2.26×:
    // 92.7M slots were allocated, 40.9M used, and ~2.7 GB remained untouched.
    // Increasing the divisor is NOT a solution: feeds with short shape_ids would then be
    // underestimated, triggering Vec doubling and keeping old+new buffers alive during copying.
    // Instead, measure the REAL average from the first SAMPLE_ROWS rows and derive capacity
    // once from it; adapt to the feed. CAPACITY only — element values/order are unaffected.
    const SAMPLE_ROWS: usize = 4096;
    let stream_bytes: Option<usize> = zip_bytes
        // ZIP central-directory sizes are untrusted; disable metadata-based
        // preallocation for the streamed path.
        .map(|_| 0)
        .or_else(|| file.raw_text.as_ref().map(|t| t.len()));
    let mut sampled_bytes: usize = 0;
    let mut capacity_set = false;
    let mut records: Vec<ShapePointRecord> = match stream_bytes {
        // Stream path: reserve once after sampling completes.
        Some(_) => Vec::new(),
        // rows fallback: the row count is already exact.
        None => Vec::with_capacity(file.rows.len()),
    };
    let mut counter = 0u32;
    // ARC_033: readers observe the violation; this loop adds the line number (see trips.rs).
    let mut rfc_acc = crate::k1_parse::Rfc4180Acc::default();
    let mut scan = super::stop_times::CsvScanState::default();
    let mut zip_unclosed = false;

    let cols = Cols::from_headers(&file.headers);
    let header_count = file.headers.len();
    let mut arc021_fired = false;
    let mut arc030_fired = false;
    // SHP_008's key is now an intern index: shape_id used to be CLONED on every row
    // (40.9M String allocations for this map alone).
    let mut seen_seq_by_shape: HashMap<u32, HashSet<u32>> = HashMap::new();
    // shape_id interning state. Since points for the same shape arrive consecutively, remember
    // the last id and skip hash lookup entirely on ~99.9% of rows.
    let mut intern = ShapeInternTable::new();
    let mut intern_map: rustc_hash::FxHashMap<smol_str::SmolStr, u32> = Default::default();
    let mut last_shape: Option<(smol_str::SmolStr, u32)> = None;
    let mut total_rows: usize = 0;
    let mut stream_truncated = false;
    let mut dq016 = crate::k1_parse::Dq016Acc::default();
    let mut arc037 = crate::k1_parse::Arc037Acc::default();
    let mut arc032 = crate::k1_parse::Arc032Acc::default();

    {
        // Row processor — called by both the stream (raw_text) and rows fallback paths.
        let mut process = |row: &[Cow<'_, str>], line: u64| {
            // ── Moved generic file/row-level notices (from K1) ──
            // ARC_012: inconsistent column count
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
                    n.severity = Severity::Bilgi;
                }
                notices.push(n);
            }

            // ARC_034: repeated header → notice + DO NOT RECORD (#181, K1 streams this file).
            if crate::k1_parse::arc034_is_header_repeat(row, &file.headers) {
                notices.push(make_k2_notice(
                    &mut counter, "ARC_034", EntityType::File, Some(file.name.clone()),
                    None, &file.name, Some(line), None, None, None,
                    format!("'{}' {line}. satırı başlık satırının tekrarı — veri olarak okunuyor.", file.name),
                    "Remove the repeated header row in the middle of the file; it usually comes from two files being concatenated.",
                ));
                return;
            }

            // ARC_018: completely empty row → notice + DO NOT RECORD
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
            total_rows += 1;

            // Capacity sampling (stream path only): derive the average from the first SAMPLE_ROWS
            // rows' actual lengths and reserve the remaining capacity ONCE. This avoids both
            // fixed-assumption inflation and doubling's temporary 2× peak.
            // Leave 5% headroom so a slightly low estimate does not trigger realloc.
            if !capacity_set {
                if let Some(total) = stream_bytes {
                    // field lengths + separators/line ending ≈ raw row bytes
                    sampled_bytes += row.iter().map(|v| v.len()).sum::<usize>() + row.len();
                    if total_rows >= SAMPLE_ROWS {
                        let avg = (sampled_bytes / total_rows).max(1);
                        let est = (total / avg).saturating_mul(105) / 100;
                        records.reserve(est.saturating_sub(records.len()));
                        capacity_set = true;
                    }
                }
            }

            // ARC_021: non-printable/problematic character — once per file
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
                if let Some(cp) =
                    crate::k1_parse::arc030_bad_whitespace(row.iter().map(|v| v.as_ref()))
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

            // DQ_016: leading/trailing whitespace in values
            // DQ_016: file-level accumulation; ONE summary after the emit loop (burst prevention).
            dq016.observe(line, row.iter().map(|v| v.as_ref()), &file.headers);
            arc037.observe(line, row.iter().map(|v| v.as_ref()), &file.headers);
            arc032.observe(line, row.iter().map(|v| v.as_ref()), &file.headers);

            // ── Shape-specific rules ──
            let shape_id = get_col_raw(row, cols.shape_id);
            // Interning: text is stored once and the point carries a 4-byte index. For consecutive
            // identical shape_ids (the normal case), there is no hash lookup or allocation.
            let shape_idx: u32 = if shape_id.is_empty() {
                0
            } else {
                match &last_shape {
                    Some((s, i)) if s.as_str() == shape_id => *i,
                    _ => {
                        let key = smol_str::SmolStr::from(shape_id);
                        let idx = match intern_map.get(&key) {
                            Some(&i) => i,
                            None => {
                                intern.ids.push(key.clone());
                                let i = (intern.ids.len() - 1) as u32;
                                intern_map.insert(key.clone(), i);
                                i
                            }
                        };
                        last_shape = Some((key, idx));
                        idx
                    }
                }
            };
            // entity_id is needed only when producing a notice; it used to be cloned on EVERY row.
            let entity_id = || (!shape_id.is_empty()).then(|| shape_id.to_string());

            // SHP_001: shape_id required (if the column is absent, ARC_025 takes over → skip)
            if shape_id.is_empty() && file.headers.iter().any(|h| h == "shape_id") {
                notices.push(make_k2_notice(
                    &mut counter,
                    "SHP_001",
                    EntityType::Shape,
                    None,
                    None,
                    &file.name,
                    Some(line),
                    Some("shape_id"),
                    Some(String::new()),
                    None,
                    "shape_id zorunludur.".to_string(),
                    "shape_id alanını doldurun.",
                ));
            }

            // SHP_004: shape_pt_sequence required
            let seq_raw = get_col(row, cols.shape_pt_sequence);
            let shape_pt_sequence = match parse_u32_col(seq_raw) {
                Ok(v) => {
                    if v.is_none() && file.headers.iter().any(|h| h == "shape_pt_sequence") {
                        notices.push(make_k2_notice(
                            &mut counter,
                            "SHP_004",
                            EntityType::Shape,
                            entity_id(),
                            None,
                            &file.name,
                            Some(line),
                            Some("shape_pt_sequence"),
                            Some(String::new()),
                            None,
                            "shape_pt_sequence zorunludur.".to_string(),
                            "shape_pt_sequence negatif olmayan bir tam sayı olarak girin.",
                        ));
                    }
                    v
                }
                Err(_) => {
                    notices.push(make_k2_notice(
                        &mut counter,
                        "SHP_004",
                        EntityType::Shape,
                        entity_id(),
                        None,
                        &file.name,
                        Some(line),
                        Some("shape_pt_sequence"),
                        Some(seq_raw.to_string()),
                        None,
                        format!("shape_pt_sequence '{seq_raw}' geçersiz."),
                        "shape_pt_sequence negatif olmayan bir tam sayı olarak girin.",
                    ));
                    None
                }
            };

            // SHP_008: shape_pt_sequence yineleniyor
            if let Some(seq) = shape_pt_sequence {
                if !seen_seq_by_shape.entry(shape_idx).or_default().insert(seq) {
                    notices.push(make_k2_notice(
                        &mut counter,
                        "SHP_008",
                        EntityType::Shape,
                        entity_id(),
                        None,
                        &file.name,
                        Some(line),
                        Some("shape_pt_sequence"),
                        Some(seq.to_string()),
                        None,
                        format!("shape_id '{shape_id}' için shape_pt_sequence {seq} yineleniyor."),
                        "Her shape noktasına benzersiz bir shape_pt_sequence atayın.",
                    ));
                }
            }

            // SHP_002: shape_pt_lat required, in range [-90, 90]
            let lat_raw = get_col(row, cols.shape_pt_lat);
            let shape_pt_lat = match parse_f64_col(lat_raw) {
                Ok(None) => {
                    if file.headers.iter().any(|h| h == "shape_pt_lat") {
                        notices.push(make_k2_notice(
                            &mut counter,
                            "SHP_002",
                            EntityType::Shape,
                            entity_id(),
                            None,
                            &file.name,
                            Some(line),
                            Some("shape_pt_lat"),
                            Some(String::new()),
                            Some("[-90, 90]".to_string()),
                            "shape_pt_lat zorunludur.".to_string(),
                            "shape_pt_lat alanını ondalıklı enlem değeriyle doldurun.",
                        ));
                    }
                    None
                }
                Ok(Some(lat)) => {
                    if !(-90.0..=90.0).contains(&lat) {
                        notices.push(make_k2_notice(
                            &mut counter,
                            "SHP_002",
                            EntityType::Shape,
                            entity_id(),
                            None,
                            &file.name,
                            Some(line),
                            Some("shape_pt_lat"),
                            Some(lat.to_string()),
                            Some("[-90, 90]".to_string()),
                            format!("shape_pt_lat {lat} değeri [-90, 90] aralığı dışında."),
                            "shape_pt_lat için -90 ile 90 arasında bir değer girin.",
                        ));
                    }
                    Some(lat)
                }
                Err(_) => {
                    notices.push(make_k2_notice(
                        &mut counter,
                        "SHP_002",
                        EntityType::Shape,
                        entity_id(),
                        None,
                        &file.name,
                        Some(line),
                        Some("shape_pt_lat"),
                        Some(lat_raw.to_string()),
                        Some("[-90, 90]".to_string()),
                        format!("shape_pt_lat '{lat_raw}' geçersiz."),
                        "shape_pt_lat alanını ondalıklı enlem değeriyle doldurun.",
                    ));
                    None
                }
            };

            // SHP_003: shape_pt_lon required, in range [-180, 180]
            let lon_raw = get_col(row, cols.shape_pt_lon);
            let shape_pt_lon = match parse_f64_col(lon_raw) {
                Ok(None) => {
                    if file.headers.iter().any(|h| h == "shape_pt_lon") {
                        notices.push(make_k2_notice(
                            &mut counter,
                            "SHP_003",
                            EntityType::Shape,
                            entity_id(),
                            None,
                            &file.name,
                            Some(line),
                            Some("shape_pt_lon"),
                            Some(String::new()),
                            Some("[-180, 180]".to_string()),
                            "shape_pt_lon zorunludur.".to_string(),
                            "shape_pt_lon alanını ondalıklı boylam değeriyle doldurun.",
                        ));
                    }
                    None
                }
                Ok(Some(lon)) => {
                    if !(-180.0..=180.0).contains(&lon) {
                        notices.push(make_k2_notice(
                            &mut counter,
                            "SHP_003",
                            EntityType::Shape,
                            entity_id(),
                            None,
                            &file.name,
                            Some(line),
                            Some("shape_pt_lon"),
                            Some(lon.to_string()),
                            Some("[-180, 180]".to_string()),
                            format!("shape_pt_lon {lon} değeri [-180, 180] aralığı dışında."),
                            "shape_pt_lon için -180 ile 180 arasında bir değer girin.",
                        ));
                    }
                    Some(lon)
                }
                Err(_) => {
                    notices.push(make_k2_notice(
                        &mut counter,
                        "SHP_003",
                        EntityType::Shape,
                        entity_id(),
                        None,
                        &file.name,
                        Some(line),
                        Some("shape_pt_lon"),
                        Some(lon_raw.to_string()),
                        Some("[-180, 180]".to_string()),
                        format!("shape_pt_lon '{lon_raw}' geçersiz."),
                        "shape_pt_lon alanını ondalıklı boylam değeriyle doldurun.",
                    ));
                    None
                }
            };

            // shape_dist_traveled: optional, non-negative (SHP_021)
            let sdt_raw = get_col(row, cols.shape_dist_traveled);
            let shape_dist_traveled = match parse_f64_col(sdt_raw) {
                Ok(v) => {
                    if let Some(d) = v {
                        if d < 0.0 {
                            notices.push(make_k2_notice(
                                &mut counter, "SHP_021", EntityType::Shape, entity_id(),
                                None, &file.name, Some(line), Some("shape_dist_traveled"),
                                Some(d.to_string()), Some(">= 0".to_string()),
                                "shape_dist_traveled negatif olamaz.".to_string(),
                                "shape_dist_traveled alanını sıfır veya pozitif bir değere ayarlayın.",
                            ));
                        }
                    }
                    v
                }
                Err(()) => {
                    // A NON-NUMERIC value used to be silently dropped: an out-of-range number produced
                    // SHP_021, while "abc" produced no finding. Two branches of the same fact → one rule
                    // (the PTH_027 precedent).
                    notices.push(make_k2_notice(
                        &mut counter,
                        "SHP_021",
                        EntityType::Shape,
                        entity_id(),
                        None,
                        &file.name,
                        Some(line),
                        Some("shape_dist_traveled"),
                        Some(sdt_raw.to_string()),
                        Some(">= 0".to_string()),
                        format!("shape_dist_traveled '{}' sayı olarak okunamıyor.", sdt_raw),
                        "shape_dist_traveled alanını sıfır veya pozitif bir değere ayarlayın.",
                    ));
                    None
                }
            };

            // SHP_005 (shape_dist_traveled monotonicity) moved to K5: it is checked on points
            // ordered by shape_pt_sequence, NOT file row order. For valid feeds where file order
            // differs from sequence, the old file-order check produced false "decrease" findings.

            records.push(ShapePointRecord::new(
                shape_idx,
                shape_pt_lat,
                shape_pt_lon,
                shape_pt_sequence,
                shape_dist_traveled,
                line as u32,
            ));
        };

        // ── Driver: zip_bytes → ZIP stream; raw_text → string stream; rows → fallback ──
        // ZIP path follows the #15/#38 pattern: the body is NOT loaded into memory in K1 and
        // is read row by row here. shapes.txt alone can be multiple GB in some feeds
        // (mdb-1078: 2.65 GB); retaining it as raw_text in K1 doubled peak memory.
        if let Some(zb) = zip_bytes {
            let cursor = std::io::Cursor::new(zb);
            match zip::ZipArchive::new(cursor) {
                Err(_) => {}
                Ok(mut archive) => {
                    if let Ok(entry) =
                        archive.by_name(file.zip_entry_name.as_deref().unwrap_or(&file.name))
                    {
                        let stream_limit = stream_budget
                            .as_ref()
                            .map(|budget| budget.limit())
                            .unwrap_or(super::stop_times::stream_byte_limit(max_data_rows));
                        let mut rdr = ZipCsvReader::with_limits(entry, stream_limit, max_data_rows);
                        let mut fields: Vec<Vec<u8>> = Vec::with_capacity(8);
                        let mut line: u64 = 2;
                        let mut header_skipped = false;
                        while rdr.next_record(&mut fields) {
                            if fields.len() == 1 && fields[0].is_empty() {
                                continue;
                            }
                            if !header_skipped {
                                header_skipped = true;
                                continue;
                            }
                            let row: Vec<Cow<'_, str>> = fields
                                .iter()
                                .map(|f| match std::str::from_utf8(f) {
                                    Ok(x) => Cow::Borrowed(x),
                                    Err(_) => Cow::Owned(String::from_utf8_lossy(f).into_owned()),
                                })
                                .collect();
                            zip_unclosed |= rdr.unclosed;
                            if let Some((k, ex)) = rdr.rfc.take() {
                                rfc_acc.observe(line, k, &ex);
                            }
                            process(&row, line);
                            line += 1;
                        }
                        if let Some(budget) = stream_budget.as_mut() {
                            budget.consume(rdr.bytes_read());
                        }
                        stream_truncated |= rdr.truncated();
                    }
                }
            }
        } else if let Some(text) = &file.raw_text {
            let mut pos = 0usize;
            let mut buf: Vec<Cow<'_, str>> = Vec::with_capacity(8);
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
    }

    if stream_truncated {
        if let Some(max) = max_data_rows {
            total_rows = total_rows.max(max.saturating_add(1));
        }
    }

    // ARC_013: unclosed quote in the stream body (issue #84) — one notice per FILE.
    if scan.unclosed || zip_unclosed {
        notices.push(super::common::arc013_unclosed_stream(
            &file.name,
            &mut counter,
        ));
    }

    // ARC_033: ONE summary per FILE.
    if let Some(n) = super::common::arc033_summary(&rfc_acc, &file.name, &mut counter) {
        notices.push(n);
    }

    // ⚠️ ARC_022 (row-count limit) is NOT HERE: issue #75 consolidated it into the single
    // pass in `k2/mod.rs`. Duplicate validation had silently left coverage incomplete —
    // `trips.txt` and `calendar_dates.txt` had no duplicate checks.

    // ARC_009: header exists but there is no data row (moved from K1 in stream_mode). shapes
    // is OPTIONAL → Info.
    // 🔴 This branch was DEAD CODE until 2026-08-31: because `shapes.txt` was in the
    // `is_zip_stream` list, K1 always left `raw_text` as None; K1's own empty-file check
    // also required `!stream_mode` and never inspected shapes. As a result, a header-only
    // `shapes.txt` produced no finding. The condition was aligned with `stop_times.rs`.
    if (file.raw_text.is_some() || zip_bytes.is_some()) && total_rows == 0 {
        let mut n = make_k2_notice(
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
            format!(
                "'{}' dosyasında başlık satırı var ama veri satırı yok.",
                file.name
            ),
            "Dosyaya en az bir veri satırı ekleyin (veya gereksizse dosyayı kaldırın).",
        );
        n.severity = Severity::Bilgi;
        notices.push(n);
    }

    // DQ_016: ONE summary per FILE (not per row — burst prevention).
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

    (records, intern, notices)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::k1_parse::RawFile;

    fn make_file(headers: Vec<&str>, rows: Vec<Vec<&str>>) -> RawFile {
        RawFile {
            name: "shapes.txt".to_string(),
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

    #[test]
    fn valid_shape_point_produces_no_notices() {
        let file = make_file(
            vec![
                "shape_id",
                "shape_pt_lat",
                "shape_pt_lon",
                "shape_pt_sequence",
            ],
            vec![vec!["SHP1", "41.0", "29.0", "1"]],
        );
        let (records, _interns, notices) = validate_shapes(&file, None);
        assert_eq!(records.len(), 1);
        assert!(
            notices.is_empty(),
            "Geçerli shape noktası notice üretmemeli: {:?}",
            notices
        );
    }

    #[test]
    fn lat_out_of_range_produces_shp_002() {
        let file = make_file(
            vec![
                "shape_id",
                "shape_pt_lat",
                "shape_pt_lon",
                "shape_pt_sequence",
            ],
            vec![vec!["SHP1", "999.0", "29.0", "1"]],
        );
        let (_, _interns, notices) = validate_shapes(&file, None);
        assert!(notices.iter().any(|n| n.rule_id == "SHP_002"));
    }

    #[test]
    fn lon_out_of_range_produces_shp_003() {
        let file = make_file(
            vec![
                "shape_id",
                "shape_pt_lat",
                "shape_pt_lon",
                "shape_pt_sequence",
            ],
            vec![vec!["SHP1", "41.0", "999.0", "1"]],
        );
        let (_, _interns, notices) = validate_shapes(&file, None);
        assert!(notices.iter().any(|n| n.rule_id == "SHP_003"));
    }

    #[test]
    fn missing_sequence_produces_shp_004() {
        let file = make_file(
            vec![
                "shape_id",
                "shape_pt_lat",
                "shape_pt_lon",
                "shape_pt_sequence",
            ],
            vec![vec!["SHP1", "41.0", "29.0", ""]],
        );
        let (_, _interns, notices) = validate_shapes(&file, None);
        assert!(notices.iter().any(|n| n.rule_id == "SHP_004"));
    }

    #[test]
    fn streaming_raw_text_matches_rows_path() {
        // #15 W3: in production, shapes is streamed through raw_text. For the same data,
        // the raw_text and rows fallback paths must produce exactly the same records/lines/notices.
        let headers = vec![
            "shape_id",
            "shape_pt_lat",
            "shape_pt_lon",
            "shape_pt_sequence",
        ];
        let rows = vec![
            vec!["S1", "41.0", "29.0", "1"],
            vec!["S1", "999.0", "29.0", "2"], // SHP_002 (latitude out of range)
            vec!["S1", "41.0", "29.0", "2"],  // SHP_008 (duplicate sequence)
        ];

        // rows fallback path
        let (rec_a, _interns, not_a) =
            validate_shapes(&make_file(headers.clone(), rows.clone()), None);

        // raw_text streaming path (the form produced by K1 stream_mode)
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
            zip_entry_name: None,
        };
        let (rec_b, _interns, not_b) = validate_shapes(&file_stream, None);

        assert_eq!(rec_a.len(), rec_b.len(), "record count must match on both paths");
        let lines_a: Vec<u64> = rec_a.iter().map(|r| r.line_u64()).collect();
        let lines_b: Vec<u64> = rec_b.iter().map(|r| r.line_u64()).collect();
        assert_eq!(lines_a, lines_b, "line numbers must match on both paths (2,3,4)");
        let mut rules_a: Vec<&str> = not_a.iter().map(|n| n.rule_id.as_str()).collect();
        let mut rules_b: Vec<&str> = not_b.iter().map(|n| n.rule_id.as_str()).collect();
        rules_a.sort();
        rules_b.sort();
        assert_eq!(rules_a, rules_b, "both paths must produce the same notice set");
        assert!(rules_b.contains(&"SHP_002") && rules_b.contains(&"SHP_008"));
    }

    /// Capacity estimation must follow the feed's REAL row length.
    /// Regression: the fixed "row ~30 bytes" assumption over-allocated 2.26× in Entur
    /// (long NeTEx shape_ids, real average 67.9 B) and held ~2.7 GB unused.
    fn stream_file(text: String) -> RawFile {
        RawFile {
            name: "shapes.txt".to_string(),
            headers: vec![
                "shape_id",
                "shape_pt_lat",
                "shape_pt_lon",
                "shape_pt_sequence",
            ]
            .into_iter()
            .map(str::to_string)
            .collect(),
            rows: Vec::new(),
            bytes: text.len() as u32,
            raw_text: Some(text),
            zip_entry_name: None,
        }
    }

    #[test]
    fn capacity_tracks_real_row_length_for_long_shape_ids() {
        let n = 20_000;
        let mut text = String::from("shape_id,shape_pt_lat,shape_pt_lon,shape_pt_sequence\n");
        for i in 0..n {
            // ~70 bytes/row — more than twice the old fixed assumption (30 B)
            text.push_str(&format!(
                "RUT:JourneyPattern:{i:08}:long-netex-id,59.91,10.75,{i}\n"
            ));
        }
        let (records, _interns, _) = validate_shapes(&stream_file(text), None);
        assert_eq!(records.len(), n, "all rows must be read");
        assert!(
            records.capacity() < records.len() * 3 / 2,
            "capacity {} / rows {} — fixed-assumption inflation has returned",
            records.capacity(),
            records.len()
        );
    }

    #[test]
    fn capacity_does_not_starve_for_short_shape_ids() {
        // Opposite extreme: the estimate must not be LOW for short IDs, or Vec doubling
        // keeps old and new buffers alive during copying (increasing the peak).
        let n = 20_000;
        let mut text = String::from("shape_id,shape_pt_lat,shape_pt_lon,shape_pt_sequence\n");
        for i in 0..n {
            text.push_str(&format!("S{i},41.0,29.0,{i}\n"));
        }
        let (records, _interns, _) = validate_shapes(&stream_file(text), None);
        assert_eq!(records.len(), n);
        assert!(
            records.capacity() >= records.len(),
            "capacity {} fell below {} rows",
            records.capacity(),
            records.len()
        );
    }
}
