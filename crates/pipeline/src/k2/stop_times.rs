use gtfs_core::{EntityType, Notice, Severity};
use rustc_hash::{FxHashMap, FxHashSet};
use smol_str::SmolStr;
use std::borrow::Cow;

use super::common::make_k2_notice;
use crate::k1_parse::RawFile;

// ── CompactStopTime: trip_id taşımaz (HashMap key'i), sadece gerekli alanlar ──

#[derive(Debug, Clone, Default)]
pub struct CompactStopTime {
    pub stop_id:              SmolStr,
    pub stop_sequence:        Option<u32>,
    pub arrival_time:         Option<(u32, u32, u32)>,
    pub departure_time:       Option<(u32, u32, u32)>,
    pub stop_headsign:        Option<SmolStr>,
    pub pickup_type:          Option<u32>,
    pub drop_off_type:        Option<u32>,
    pub shape_dist_traveled:  Option<f64>,
    pub timepoint:            Option<u32>,
    pub continuous_pickup:    Option<u32>,
    pub continuous_drop_off:  Option<u32>,
    pub line:                 u64,
    // Flex alanları — OOM fix Plan C: feed'lerin %99'unda None olduğundan Box'lanır.
    // None iken 8 byte (boxsuz ~128 byte) — 2.48M satırda ~300 MB tasarruf.
    pub flex: Option<Box<StopTimeFlex>>,
}

/// CompactStopTime'ın nadir kullanılan GTFS-Flex alanları (Box'lanır — bkz. CompactStopTime.flex).
#[derive(Debug, Clone, Default)]
pub struct StopTimeFlex {
    pub start_pickup_drop_off_window:  Option<(u32, u32, u32)>,
    pub end_pickup_drop_off_window:    Option<(u32, u32, u32)>,
    pub location_id:                   Option<SmolStr>,
    pub location_group_id:             Option<SmolStr>,
    pub pickup_booking_rule_id:        Option<SmolStr>,
    pub drop_off_booking_rule_id:      Option<SmolStr>,
}

/// 6 flex değerinden herhangi biri Some ise Box'lanmış StopTimeFlex, aksi halde None döner.
#[inline]
pub fn build_flex(
    start_window: Option<(u32, u32, u32)>,
    end_window: Option<(u32, u32, u32)>,
    location_id: Option<SmolStr>,
    location_group_id: Option<SmolStr>,
    pickup_booking_rule_id: Option<SmolStr>,
    drop_off_booking_rule_id: Option<SmolStr>,
) -> Option<Box<StopTimeFlex>> {
    if start_window.is_some() || end_window.is_some() || location_id.is_some()
        || location_group_id.is_some() || pickup_booking_rule_id.is_some()
        || drop_off_booking_rule_id.is_some()
    {
        Some(Box::new(StopTimeFlex {
            start_pickup_drop_off_window: start_window,
            end_pickup_drop_off_window: end_window,
            location_id,
            location_group_id,
            pickup_booking_rule_id,
            drop_off_booking_rule_id,
        }))
    } else {
        None
    }
}

// ── StopTimesIndex: K4/K6 için tek referans noktası ──────────────────────────

#[derive(Debug, Default)]
pub struct StopTimesIndex {
    /// Flat satır deposu — trip'e göre gruplu, her grup stop_sequence sıralı.
    /// OOM/perf: per-trip ayrı `Vec` YOK; tek contiguous buffer (erişim accessor'lardan).
    pub rows: Vec<CompactStopTime>,
    /// trip_id → `rows` içindeki [start, end) aralığı
    pub trip_ranges: FxHashMap<SmolStr, (u32, u32)>,
    /// K4: XFL_002 — stop_times'ta geçen trip_id'ler
    pub trip_id_set: FxHashSet<SmolStr>,
    /// K4: STP_012 / STM_001/002 — stop_times'ta geçen stop_id'ler
    pub stop_id_set: FxHashSet<SmolStr>,
    /// K4: XFL_021 — per-trip stop_id seti
    pub trip_stop_set: FxHashMap<SmolStr, FxHashSet<SmolStr>>,
    /// K4: TRP_019 — continuous pickup/drop-off olan seferler
    pub continuous_trips: FxHashSet<SmolStr>,
    /// K2/K6: toplam satır sayısı (STM_044 için)
    pub total_rows: usize,
    /// K4: STM_001 — her trip_id'nin stop_times.txt'teki ilk satır numarası
    pub trip_first_line: FxHashMap<SmolStr, u64>,
    /// K4: STM_002 — her stop_id'nin stop_times.txt'teki ilk satır numarası
    pub stop_first_line: FxHashMap<SmolStr, u64>,
    /// K6: STM_036 — dosya sırasında stop_sequence gerileyen seferler: (trip_id, prev_seq, curr_seq, line)
    pub unsorted_seq_trips: Vec<(SmolStr, u32, u32, u64)>,
    /// K6: STM_048 — normalize_service_day'in 00:xx→24:xx kaydırdığı (gece yarısını aşan) trip sayısı.
    pub midnight_wrapped_trips: u32,
}

impl StopTimesIndex {
    /// trip başına sıralı (stop_sequence artan) stop dilimi. `idx.trips.get(...)` yerine BU kullanılır.
    pub fn sorted_stops(&self, trip_id: &str) -> Option<&[CompactStopTime]> {
        let &(s, e) = self.trip_ranges.get(trip_id)?;
        Some(&self.rows[s as usize..e as usize])
    }

    /// Gece yarısını aşan seferleri servis-günü notasyonuna (24:xx, 25:xx…) normalize eder.
    /// `start_hour` (0–6): servis günü başlangıç saati; `0` ise normalizasyon kapalıdır.
    ///
    /// Her trip'in sıralı duraklarını gezer: bir durağın `arrival_time`'ı önceki normalize
    /// edilmiş zamandan KÜÇÜK **ve** ham değeri `start_hour*3600` saniyenin ALTINDA ise gece
    /// dönümü kabul edilir; o noktadan itibaren `+24sa` offset uygulanır. `departure_time` aynı
    /// offset'i takip eder ama kendi unwrap'ını TETİKLEMEZ — böylece aynı satırda `dep < arr`
    /// (STM_007) gerçek hatası bozulmaz. Eşik ÜSTÜndeki geriye-gidişler (gerçek hata) dokunulmaz;
    /// STM_008 onları yakalamaya devam eder. Monoton trip'lerde offset 0 kalır → satır yeniden yazılmaz.
    pub fn normalize_service_day(&mut self, start_hour: u32) {
        if start_hour == 0 {
            return;
        }
        let start_secs = start_hour * 3600;
        let mut wrapped = 0u32;
        for &(s, e) in self.trip_ranges.values() {
            let slice = &mut self.rows[s as usize..e as usize];
            let mut offset: u32 = 0;
            let mut prev: Option<u32> = None; // önceki normalize edilmiş zaman (saniye)
            for st in slice.iter_mut() {
                if let Some((h, m, sec)) = st.arrival_time {
                    let raw = h * 3600 + m * 60 + sec;
                    if let Some(p) = prev {
                        if raw + offset < p && raw < start_secs {
                            offset += 86400;
                        }
                    }
                    let v = raw + offset;
                    if offset > 0 {
                        st.arrival_time = Some((v / 3600, (v % 3600) / 60, v % 60));
                    }
                    prev = Some(v);
                }
                if let Some((h, m, sec)) = st.departure_time {
                    let raw = h * 3600 + m * 60 + sec;
                    let v = raw + offset; // offset'i takip eder; kendi unwrap'ını tetiklemez
                    if offset > 0 {
                        st.departure_time = Some((v / 3600, (v % 3600) / 60, v % 60));
                    }
                    prev = Some(v);
                }
            }
            if offset > 0 {
                wrapped += 1;
            }
        }
        self.midnight_wrapped_trips = wrapped;
    }

    /// Tüm trip'leri (trip_id, sıralı stop dilimi) olarak gez. `&idx.trips` iterasyonu yerine BU kullanılır.
    pub fn iter_trips(&self) -> impl Iterator<Item = (&SmolStr, &[CompactStopTime])> {
        self.trip_ranges
            .iter()
            .map(move |(tid, &(s, e))| (tid, &self.rows[s as usize..e as usize]))
    }

    /// Sadece stop dilimlerini gez. `idx.trips.values()` yerine BU kullanılır.
    pub fn iter_stops(&self) -> impl Iterator<Item = &[CompactStopTime]> {
        self.trip_ranges
            .values()
            .map(move |&(s, e)| &self.rows[s as usize..e as usize])
    }

    /// trip_id'nin stop_times'ta olup olmadığı (K4)
    pub fn has_trip(&self, trip_id: &str) -> bool {
        self.trip_ranges.contains_key(trip_id)
    }

    /// trip'in stop_times'taki stop sayısı (K6: STM_043)
    pub fn stop_count(&self, trip_id: &str) -> usize {
        self.trip_ranges.get(trip_id).map_or(0, |&(s, e)| (e - s) as usize)
    }

    /// Test yardımcısı: Vec<StopTimeRecord>'dan index oluşturur.
    /// Gerçek pipeline'da K2 streaming tarafından doldurulur.
    pub fn from_records(records: &[StopTimeRecord]) -> Self {
        let mut idx = Self::default();
        let mut by_trip: FxHashMap<SmolStr, Vec<CompactStopTime>> = FxHashMap::default();
        for st in records {
            if st.trip_id.is_empty() { continue; }
            idx.total_rows += 1;
            idx.trip_id_set.insert(st.trip_id.clone());
            idx.trip_first_line.entry(st.trip_id.clone()).or_insert(st.line);
            if !st.stop_id.is_empty() {
                idx.stop_id_set.insert(st.stop_id.clone());
                idx.stop_first_line.entry(st.stop_id.clone()).or_insert(st.line);
                idx.trip_stop_set
                    .entry(st.trip_id.clone())
                    .or_default()
                    .insert(st.stop_id.clone());
            }
            if matches!(st.continuous_pickup, Some(0) | Some(1))
                || matches!(st.continuous_drop_off, Some(0) | Some(1))
            {
                idx.continuous_trips.insert(st.trip_id.clone());
            }
            by_trip.entry(st.trip_id.clone()).or_default().push(CompactStopTime {
                stop_id:            st.stop_id.clone(),
                stop_sequence:      st.stop_sequence,
                arrival_time:       st.arrival_time,
                departure_time:     st.departure_time,
                stop_headsign:      st.stop_headsign.clone(),
                pickup_type:        st.pickup_type,
                drop_off_type:      st.drop_off_type,
                shape_dist_traveled: st.shape_dist_traveled,
                timepoint:          st.timepoint,
                continuous_pickup:  st.continuous_pickup,
                continuous_drop_off: st.continuous_drop_off,
                line:               st.line,
                flex: build_flex(
                    st.start_pickup_drop_off_window,
                    st.end_pickup_drop_off_window,
                    st.location_id.clone(),
                    st.location_group_id.clone(),
                    st.pickup_booking_rule_id.clone(),
                    st.drop_off_booking_rule_id.clone(),
                ),
            });
        }
        // Test yardımcısı: per-trip map → flat rows + trip_ranges (veri küçük, perf önemsiz).
        for (tid, mut stops) in by_trip {
            stops.sort_by_key(|st| st.stop_sequence.unwrap_or(u32::MAX));
            let start = idx.rows.len() as u32;
            idx.rows.extend(stops);
            let end = idx.rows.len() as u32;
            idx.trip_ranges.insert(tid, (start, end));
        }
        idx
    }
}

// ── StopTimeRecord — sadece test fixture'ları için (production'da kullanılmaz) ─
// Testler bu struct ile veri kurar; StopTimesIndex::from_records() index'e çevirir.

#[derive(Debug, Clone, Default)]
pub struct StopTimeRecord {
    pub trip_id: SmolStr,
    pub stop_id: SmolStr,
    pub stop_sequence: Option<u32>,
    pub arrival_time: Option<(u32, u32, u32)>,
    pub departure_time: Option<(u32, u32, u32)>,
    pub stop_headsign: Option<SmolStr>,
    pub pickup_type: Option<u32>,
    pub drop_off_type: Option<u32>,
    pub shape_dist_traveled: Option<f64>,
    pub timepoint: Option<u32>,
    pub continuous_pickup: Option<u32>,
    pub continuous_drop_off: Option<u32>,
    // Flex GTFS alanları
    pub start_pickup_drop_off_window: Option<(u32, u32, u32)>,
    pub end_pickup_drop_off_window: Option<(u32, u32, u32)>,
    pub location_id: Option<SmolStr>,
    pub location_group_id: Option<SmolStr>,
    pub pickup_booking_rule_id: Option<SmolStr>,
    pub drop_off_booking_rule_id: Option<SmolStr>,
    pub line: u64,
}

// ── Kolon indeks yapısı — header'dan bir kez hesaplanır ─────────────────────

struct Cols {
    trip_id:              Option<usize>,
    stop_id:              Option<usize>,
    arrival_time:         Option<usize>,
    departure_time:       Option<usize>,
    stop_headsign:        Option<usize>,
    pickup_type:          Option<usize>,
    drop_off_type:        Option<usize>,
    shape_dist_traveled:  Option<usize>,
    timepoint:            Option<usize>,
    continuous_pickup:    Option<usize>,
    continuous_drop_off:  Option<usize>,
    stop_sequence:        Option<usize>,
    // Flex
    start_pickup_drop_off_window:  Option<usize>,
    end_pickup_drop_off_window:    Option<usize>,
    location_id:                   Option<usize>,
    location_group_id:             Option<usize>,
    pickup_booking_rule_id:        Option<usize>,
    drop_off_booking_rule_id:      Option<usize>,
}

impl Cols {
    fn from_headers(headers: &[String]) -> Self {
        let pos = |name: &str| headers.iter().position(|h| h == name);
        Self {
            trip_id:             pos("trip_id"),
            stop_id:             pos("stop_id"),
            arrival_time:        pos("arrival_time"),
            departure_time:      pos("departure_time"),
            stop_headsign:       pos("stop_headsign"),
            pickup_type:         pos("pickup_type"),
            drop_off_type:       pos("drop_off_type"),
            shape_dist_traveled: pos("shape_dist_traveled"),
            timepoint:           pos("timepoint"),
            continuous_pickup:   pos("continuous_pickup"),
            continuous_drop_off: pos("continuous_drop_off"),
            stop_sequence:       pos("stop_sequence"),
            start_pickup_drop_off_window: pos("start_pickup_drop_off_window"),
            end_pickup_drop_off_window:   pos("end_pickup_drop_off_window"),
            location_id:                  pos("location_id"),
            location_group_id:            pos("location_group_id"),
            pickup_booking_rule_id:       pos("pickup_booking_rule_id"),
            drop_off_booking_rule_id:     pos("drop_off_booking_rule_id"),
        }
    }
}

#[inline]
fn get_col<'a>(row: &'a [Cow<'_, str>], col: Option<usize>) -> &'a str {
    col.and_then(|i| row.get(i)).map(|s| s.as_ref().trim()).unwrap_or("")
}

// ── Parser yardımcıları (RowMap olmaksızın) ──────────────────────────────────

/// `str::parse::<u32>()` ile BİREBİR aynı sonucu verir: opsiyonel tek '+' öneki,
/// ardından ≥1 ASCII rakam; '-'/boşluk/ASCII-dışı/boş → None; overflow → None.
/// (parse_tests modülü str::parse'a karşı brute-force + fuzz doğrular.) Hot path'te
/// str::parse'ın generic FromStr dispatch + Result kurma maliyetini kaldırır.
#[inline]
fn parse_u32_ascii(s: &str) -> Option<u32> {
    let b = s.as_bytes();
    let digits = if b.first() == Some(&b'+') { &b[1..] } else { b };
    if digits.is_empty() {
        return None;
    }
    let mut acc: u32 = 0;
    for &c in digits {
        if !c.is_ascii_digit() {
            return None;
        }
        acc = acc.checked_mul(10)?.checked_add((c - b'0') as u32)?;
    }
    Some(acc)
}

fn parse_u32_raw(raw: &str, field: &str) -> Result<Option<u32>, String> {
    if raw.is_empty() {
        return Ok(None);
    }
    match parse_u32_ascii(raw) {
        Some(n) => Ok(Some(n)),
        None => Err(format!("'{field}' için u32 bekleniyor, alınan: {raw}")),
    }
}

fn parse_f64_raw(raw: &str, field: &str) -> Result<Option<f64>, String> {
    if raw.is_empty() {
        return Ok(None);
    }
    raw.parse::<f64>()
        .map(Some)
        .map_err(|_| format!("'{field}' için f64 bekleniyor, alınan: {raw}"))
}

fn parse_gtfs_time_raw(raw: &str, field: &str) -> Result<Option<(u32, u32, u32)>, String> {
    if raw.is_empty() {
        return Ok(None);
    }
    let bad = || format!("'{field}' için HH:MM:SS bekleniyor, alınan: {raw}");
    let mut it = raw.splitn(3, ':');
    // parse_u32_ascii(seg).is_some() == seg.parse::<u32>().is_ok() (parse_tests garantisi) →
    // Ok/Err kararı orijinalle birebir; splitn + ok_or_else(bad) yapısı değişmedi.
    let hour   = parse_u32_ascii(it.next().ok_or_else(bad)?).ok_or_else(bad)?;
    let minute = parse_u32_ascii(it.next().ok_or_else(bad)?).ok_or_else(bad)?;
    let second = parse_u32_ascii(it.next().ok_or_else(bad)?).ok_or_else(bad)?;
    if minute > 59 || second > 59 {
        return Err(format!("'{field}' dakika/saniye aralığı geçersiz: {raw}"));
    }
    Ok(Some((hour, minute, second)))
}

#[cfg(test)]
mod parse_tests {
    use super::{parse_u32_ascii, parse_gtfs_time_raw};

    // str::parse referans HH:MM:SS — değiştirdiğimiz fonksiyonun ESKİ mantığı.
    fn ref_time(raw: &str) -> Result<Option<(u32, u32, u32)>, ()> {
        if raw.is_empty() { return Ok(None); }
        let mut it = raw.splitn(3, ':');
        let h = it.next().ok_or(())?.parse::<u32>().map_err(|_| ())?;
        let m = it.next().ok_or(())?.parse::<u32>().map_err(|_| ())?;
        let s = it.next().ok_or(())?.parse::<u32>().map_err(|_| ())?;
        if m > 59 || s > 59 { return Err(()); }
        Ok(Some((h, m, s)))
    }

    #[test]
    fn u32_ascii_matches_std_bruteforce() {
        for n in (0u32..=100_000).chain([100_001, 999_999, 1_000_000, 4_294_967_294, 4_294_967_295]) {
            let s = n.to_string();
            assert_eq!(parse_u32_ascii(&s), s.parse::<u32>().ok(), "düz {s}");
            let plus = format!("+{s}");
            assert_eq!(parse_u32_ascii(&plus), plus.parse::<u32>().ok(), "+önek {plus}");
        }
    }

    #[test]
    fn u32_ascii_matches_std_edge_cases() {
        for s in ["", "+", "-", "-5", "+5", "0", "00", "007", "+007", " 5", "5 ", "5a", "a5",
                  "4294967295", "4294967296", "42949672960", "99999999999999",
                  "+0", "++5", "1.5", "0x10", "१२", "८", "  ", "\t5", "+\u{0661}"] {
            assert_eq!(parse_u32_ascii(s), s.parse::<u32>().ok(), "edge {s:?}");
        }
    }

    #[test]
    fn u32_ascii_matches_std_fuzz() {
        let alphabet = b"0123456789+-  aZ\t.x";
        let mut state: u64 = 0x9E37_79B9_7F4A_7C15;
        let mut next = || { state = state.wrapping_mul(6364136223846793005).wrapping_add(1); (state >> 33) as usize };
        for _ in 0..50_000 {
            let len = next() % 13;
            let bytes: Vec<u8> = (0..len).map(|_| alphabet[next() % alphabet.len()]).collect();
            let s = String::from_utf8_lossy(&bytes).into_owned();
            assert_eq!(parse_u32_ascii(&s), s.parse::<u32>().ok(), "fuzz {s:?}");
        }
    }

    #[test]
    fn gtfs_time_matches_reference() {
        let cases = ["", "08:30:45", "0:0:0", "25:00:00", "24:00:00", "12:60:00", "12:00:60",
                     "1:2:3", "12:3:45", "+8:30:45", "8:+30:45", "ab:cd:ef", "12:30", "12:30:45:6",
                     "12::45", ":30:45", "100:00:00", "12:30:45 ", " 12:30:45", "0x:30:45", "१२:३०:४५"];
        for c in cases {
            assert_eq!(parse_gtfs_time_raw(c, "f").map_err(|_| ()), ref_time(c), "time {c:?}");
        }
    }
}


// ── SmolStr interning: >22 byte string'ler için Arc alloc sayısını azaltır ──

/// ≤22 byte → inline SmolStr (no heap alloc). >22 byte → ilk karşılaşmada Arc alloc,
/// sonraki hit'lerde Arc::clone (~2ns). Cache miss: O(N_unique), hit: O(1) no-alloc.
#[inline]
fn intern_smolstr(raw: &str, cache: &mut FxHashMap<String, SmolStr>) -> SmolStr {
    if raw.len() <= 22 {
        SmolStr::new(raw)
    } else if let Some(cached) = cache.get(raw) {
        cached.clone()
    } else {
        let s = SmolStr::new(raw);
        cache.insert(raw.to_string(), s.clone());
        s
    }
}

// ── Ana doğrulayıcı ─────────────────────────────────────────────────────────

// ── Streaming CSV okuyucu: tek reused buffer, satır başına yeni Vec YOK ─────────
//
// OOM fix Plan A: K1 stop_times'ı Vec<Vec<SmolStr>>'e açmaz; ham metni `raw_text`'te
// verir. Bu fonksiyon ham metni satır satır, tek `out` buffer'ı yeniden kullanarak
// işler — 2.48M satır için tek seferde 714 MB değil, anlık bir satır kadar bellek.
//
// RFC 4180 uyumlu (tokenize_csv ile aynı semantik). EOF'ta `false` döner.
fn next_csv_record<'a>(text: &'a str, pos: &mut usize, out: &mut Vec<Cow<'a, str>>) -> bool {
    let bytes = text.as_bytes();
    let n = bytes.len();
    if *pos >= n {
        return false;
    }
    out.clear();
    loop {
        if bytes[*pos] == b'"' {
            // OOM fix Plan E: tırnaklı alan NADİR (stop_times'ta neredeyse yok) — owned String.
            // Tırnaksız alanlar (asıl yol) ham metinden &str olarak ÖDÜNÇ alınır (alloc YOK).
            *pos += 1;
            let mut buf = String::new();
            while *pos < n {
                let b = bytes[*pos];
                if b == b'"' {
                    *pos += 1;
                    if *pos < n && bytes[*pos] == b'"' {
                        buf.push('"');
                        *pos += 1;
                    } else {
                        break;
                    }
                } else if b < 0x80 {
                    buf.push(b as char);
                    *pos += 1;
                } else {
                    let ch = text[*pos..].chars().next().unwrap();
                    buf.push(ch);
                    *pos += ch.len_utf8();
                }
            }
            // Kapanmamış tırnak: best-effort (zorunlu dosya CSV'si K1 header aşamasında
            // ve gövdede burada toleranslı işlenir; satır düşürülmez).
            out.push(Cow::Owned(buf));
        } else {
            let start = *pos;
            while *pos < n {
                let b = bytes[*pos];
                if b == b',' || b == b'\n' || b == b'\r' {
                    break;
                }
                *pos += 1;
            }
            out.push(Cow::Borrowed(&text[start..*pos]));
        }

        if *pos >= n {
            break;
        }
        match bytes[*pos] {
            b',' => {
                *pos += 1;
                if *pos >= n {
                    out.push(Cow::Borrowed(""));
                    break;
                }
                continue;
            }
            b'\r' => {
                *pos += 1;
                if *pos < n && bytes[*pos] == b'\n' {
                    *pos += 1;
                }
                break;
            }
            b'\n' => {
                *pos += 1;
                break;
            }
            _ => break,
        }
    }
    true
}

// ── Per-trip toplama: trip_id-anahtarlı TÜM durum tek struct'ta (hashmap churn azaltma) ──
#[derive(Default)]
struct TripAgg {
    idx: u32,                     // yoğun trip indeksi (counting-placement için)
    first_line: u64,
    seen_seq: FxHashSet<u32>,     // STM_032: tekrar eden stop_sequence
    last_seq: Option<u32>,        // STM_023: sıralama takibi
    stm023_fired: bool,           // STM_023: trip başına bir kez
    stop_set: FxHashSet<SmolStr>, // çıktı: trip_stop_set
    continuous: bool,             // çıktı: continuous_trips
}
impl TripAgg {
    fn new(first_line: u64, idx: u32) -> Self {
        Self { idx, first_line, ..Default::default() }
    }
}

pub fn validate_stop_times(file: &RawFile) -> (StopTimesIndex, Vec<gtfs_core::Notice>) {
    let mut notices: Vec<Notice> = Vec::new();
    let mut counter = 0u32;
    let cols = Cols::from_headers(&file.headers);
    // B1: feed'de hiç Flex sütunu yoksa satır başı flex işini tümüyle atla.
    // Çıktı birebir aynı: get_col hep "" döner → STM_037-041 boş-yere-false, 6 alan None.
    let has_flex_cols = cols.start_pickup_drop_off_window.is_some()
        || cols.end_pickup_drop_off_window.is_some()
        || cols.location_id.is_some()
        || cols.location_group_id.is_some()
        || cols.pickup_booking_rule_id.is_some()
        || cols.drop_off_booking_rule_id.is_some();
    let header_count = file.headers.len();
    // DQ_016: ilk "*_id" sütunu (döngü dışında bir kez)
    let dq016_pk_idx = file.headers.iter().position(|h| h.ends_with("_id"));

    // Intern cache: unique long (>22 byte) trip_id / stop_id başına bir Arc alloc
    let mut trip_id_cache: FxHashMap<String, SmolStr> = FxHashMap::default();
    let mut stop_id_cache: FxHashMap<String, SmolStr> = FxHashMap::default();
    // OOM/perf: trip_id-anahtarlı TÜM per-trip durum tek map'te (TripAgg) → satır başı ~8 hashmap op
    // yerine 1-2. Çıktı setleri (trip_id_set/trip_first_line/trip_stop_set/continuous_trips/trips)
    // finalize pass'le buradan TÜRETİLİR → StopTimesIndex şekli ve DAVRANIŞ değişmez.
    let mut trips_agg: FxHashMap<SmolStr, TripAgg> = FxHashMap::default();
    let mut stop_first_line: FxHashMap<SmolStr, u64> = FxHashMap::default(); // stop_id_set bundan türetilir
    let mut total_rows: usize = 0;
    let mut unsorted_seq_trips: Vec<(SmolStr, u32, u32, u64)> = Vec::new();
    let mut arc021_fired = false;
    // STM_050: boş-timepoint satır sayısı. Satır-başına notice DEĞİL (büyük feed'lerde milyonlarca
    // notice → tarayıcı OOM, bkz. Kocaeli 2.4M); döngü sonunda TEK feed-seviyesi özet emit edilir.
    let mut stm050_empty: u32 = 0;
    // OOM/perf (Aşama 1b): per-trip Vec YOK. Tüm satırlar (trip_idx etiketli) tek flat buffer'da
    // dosya sırasında toplanır; finalize'da counting-placement ile trip'e göre gruplanır.
    let mut all_rows: Vec<(u32, CompactStopTime)> = Vec::new();
    // Düz tamponun log₂(N) realloc/memcpy'sini önle: satır sayısını ham metin
    // uzunluğundan tahmin et (stop_times satırı tipik ≥~40 bayt → muhafazakâr
    // alt-sınır bölen). Yalnızca KAPASITE; eleman değeri/sırası etkilenmez.
    if let Some(text) = &file.raw_text {
        all_rows.reserve(text.len() / 40);
    }

    {
        // Satır işleyici — hem stream (raw_text) hem rows yolundan çağrılır.
        // Tüm değişken durum closure ile yakalanır (notices, counter, index, caches).
        let mut process = |row: &[Cow<'_, str>], line: u64| {
            // ── Taşınan dosya/satır-seviye notice'lar (K1'den) — eksik veri YOK ──
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

            // ARC_018: tamamen boş satır → notice + indexleme YAPMA (K1'deki continue)
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
            {
                let eid016: String = dq016_pk_idx
                    .and_then(|i| row.get(i))
                    .map(|v| v.as_ref())
                    .filter(|v: &&str| !v.is_empty())
                    .unwrap_or(&file.name)
                    .to_string();
                let ws_fields: Vec<&str> = row.iter().enumerate()
                    .filter(|(_, v)| { let s: &str = v.as_ref(); s != s.trim() })
                    .filter_map(|(i, _)| file.headers.get(i).map(|s| s.as_str()))
                    .collect();
                if !ws_fields.is_empty() {
                    let fields_str = ws_fields.join(", ");
                    notices.push(make_k2_notice(
                        &mut counter, "DQ_016", EntityType::Row, Some(eid016.clone()),
                        None, &file.name, Some(line), Some(fields_str.as_str()),
                        Some(format!("{line}")), None,
                        format!("'{}' kaydında ({}, satır {line}): '{}' alanlarında baştaki/sondaki boşluk var.", eid016, file.name, fields_str),
                        "Değerlerdeki gereksiz baştaki/sondaki boşlukları kaldırın.",
                    ));
                }
            }

            // ── Mevcut STM_* doğrulamaları + index ──
            let trip_id_raw = get_col(row, cols.trip_id);
        let trip_id = intern_smolstr(trip_id_raw, &mut trip_id_cache);
        // entity_id is only materialized when a notice is actually pushed
        let eid = || (!trip_id.is_empty()).then(|| trip_id.to_string());

        // STM_046: trip_id required (sütun yoksa ARC_025 devralır → atla)
        if trip_id.is_empty() && file.headers.iter().any(|h| h == "trip_id") {
            notices.push(make_k2_notice(
                &mut counter, "STM_046", EntityType::Trip, None,
                None, &file.name, Some(line), Some("trip_id"),
                Some(String::new()), None,
                "trip_id zorunludur.".to_string(),
                "Her stop_times satırına geçerli bir trip_id girin.",
            ));
        }

        // STM_005: stop_sequence required
        let seq_raw = get_col(row, cols.stop_sequence);
        let stop_sequence = match parse_u32_raw(seq_raw, "stop_sequence") {
            Ok(v) => {
                if v.is_none() && file.headers.iter().any(|h| h == "stop_sequence") {
                    notices.push(make_k2_notice(
                        &mut counter, "STM_005", EntityType::Trip, eid(),
                        None, &file.name, Some(line), Some("stop_sequence"),
                        Some(String::new()), None,
                        "stop_sequence zorunludur.".to_string(),
                        "stop_sequence negatif olmayan bir tam sayı olarak girin.",
                    ));
                }
                v
            }
            Err(err) => {
                notices.push(make_k2_notice(
                    &mut counter, "STM_005", EntityType::Trip, eid(),
                    None, &file.name, Some(line), Some("stop_sequence"),
                    Some(seq_raw.to_string()), None, err,
                    "stop_sequence negatif olmayan bir tam sayı olarak girin.",
                ));
                None
            }
        };

        // STM_023 / STM_032: sıralama ve yineleme (per-trip durum TripAgg'da; trip_id boş olsa da
        // orijinaldeki gibi "" kovasında izlenir — finalize çıktıdan "" hariç tutar)
        if let Some(seq) = stop_sequence {
            let next_idx = trips_agg.len() as u32;
            let agg = trips_agg.entry(trip_id.clone()).or_insert_with(|| TripAgg::new(line, next_idx));
            // STM_032: aynı (trip_id, stop_sequence) çifti tekrar
            if !agg.seen_seq.insert(seq) {
                notices.push(make_k2_notice(
                    &mut counter, "STM_032", EntityType::Trip, eid(),
                    None, &file.name, Some(line), Some("stop_sequence"),
                    Some(seq.to_string()), None,
                    format!("trip_id '{}' için stop_sequence {seq} tekrar ediyor.", trip_id),
                    "Her (trip_id, stop_sequence) çifti stop_times.txt'te benzersiz olmalıdır.",
                ));
            }
            // STM_023: dosya satır sırası stop_sequence sırasıyla uyuşmuyor
            if !agg.stm023_fired {
                if let Some(last) = agg.last_seq {
                    if seq < last {
                        notices.push(make_k2_notice(
                            &mut counter, "STM_023", EntityType::Trip, eid(),
                            None, &file.name, Some(line), Some("stop_sequence"),
                            Some(seq.to_string()), Some(format!("> {last}")),
                            format!("trip_id '{}' satırları stop_sequence sırasında değil: {seq} < {last}.", trip_id),
                            "stop_times.txt'i stop_sequence değerine göre sıralayın.",
                        ));
                        agg.stm023_fired = true;
                        unsorted_seq_trips.push((trip_id.clone(), last, seq, line));
                    } else if seq > last {
                        agg.last_seq = Some(seq);
                    }
                } else {
                    agg.last_seq = Some(seq);
                }
            }
        }

        // STM_006: stop_id required (sütun yoksa ARC_025 devralır → atla)
        let stop_id = intern_smolstr(get_col(row, cols.stop_id), &mut stop_id_cache);
        if stop_id.is_empty() && file.headers.iter().any(|h| h == "stop_id") {
            notices.push(make_k2_notice(
                &mut counter, "STM_006", EntityType::Stop, eid(),
                None, &file.name, Some(line), Some("stop_id"),
                Some(String::new()), None,
                "stop_id zorunludur.".to_string(),
                "stop_id alanını doldurun.",
            ));
        }

        // arrival_time
        let arr_raw = get_col(row, cols.arrival_time);
        let arrival_time = match parse_gtfs_time_raw(arr_raw, "arrival_time") {
            Ok(v) => v,
            Err(err) => {
                notices.push(make_k2_notice(
                    &mut counter, "STM_003", EntityType::Trip, eid(),
                    None, &file.name, Some(line), Some("arrival_time"),
                    Some(arr_raw.to_string()), Some("HH:MM:SS".to_string()), err,
                    "HH:MM:SS formatında arrival_time girin.",
                ));
                None
            }
        };

        // departure_time
        let dep_raw = get_col(row, cols.departure_time);
        let departure_time = match parse_gtfs_time_raw(dep_raw, "departure_time") {
            Ok(v) => v,
            Err(err) => {
                notices.push(make_k2_notice(
                    &mut counter, "STM_004", EntityType::Trip, eid(),
                    None, &file.name, Some(line), Some("departure_time"),
                    Some(dep_raw.to_string()), Some("HH:MM:SS".to_string()), err,
                    "HH:MM:SS formatında departure_time girin.",
                ));
                None
            }
        };

        // STM_007 (departure_time >= arrival_time) K6'ya taşındı: orada servis-günü
        // normalize'li veri + hat/yön/servis bağlamı mevcut, gece-yarısı (00:xx kalkış)
        // yanlış-pozitifleri elenir. Bkz. k6_analytics.rs.

        // STM_034: varış veya kalkış zamanından yalnızca biri tanımlı
        match (arrival_time, departure_time) {
            (Some(_), None) => {
                notices.push(make_k2_notice(
                    &mut counter, "STM_034", EntityType::Trip, eid(),
                    None, &file.name, Some(line), Some("departure_time"),
                    None, Some("dolu".to_string()),
                    format!("trip_id '{}' satırında arrival_time tanımlı ama departure_time eksik.", trip_id),
                    "Her iki zaman alanını birlikte doldurun veya ikisini de boş bırakın.",
                ));
            }
            (None, Some(_)) => {
                notices.push(make_k2_notice(
                    &mut counter, "STM_034", EntityType::Trip, eid(),
                    None, &file.name, Some(line), Some("arrival_time"),
                    None, Some("dolu".to_string()),
                    format!("trip_id '{}' satırında departure_time tanımlı ama arrival_time eksik.", trip_id),
                    "Her iki zaman alanını birlikte doldurun veya ikisini de boş bırakın.",
                ));
            }
            _ => {}
        }

        // STM_009: pickup_type 0-3
        let pickup_type = parse_pickup_dropoff_col(
            get_col(row, cols.pickup_type), &mut notices, &mut counter,
            "STM_009", "pickup_type", trip_id.as_str(), line, &file.name,
        );

        // STM_010: drop_off_type 0-3
        let drop_off_type = parse_pickup_dropoff_col(
            get_col(row, cols.drop_off_type), &mut notices, &mut counter,
            "STM_010", "drop_off_type", trip_id.as_str(), line, &file.name,
        );

        // STM_018: continuous_pickup 0-3
        let continuous_pickup = parse_pickup_dropoff_col(
            get_col(row, cols.continuous_pickup), &mut notices, &mut counter,
            "STM_018", "continuous_pickup", trip_id.as_str(), line, &file.name,
        );

        // STM_019: continuous_drop_off 0-3
        let continuous_drop_off = parse_pickup_dropoff_col(
            get_col(row, cols.continuous_drop_off), &mut notices, &mut counter,
            "STM_019", "continuous_drop_off", trip_id.as_str(), line, &file.name,
        );

        // STM_022: timepoint 0 or 1
        let tp_raw = get_col(row, cols.timepoint);
        // STM_050: timepoint sütunu feed'de mevcut ama bu satırda boş (MD missing_timepoint_value).
        // Boş timepoint örtük 1 (kesin) sayılır → yaklaşık zamanlar yanlış kesin görünür; açık değer önerilir.
        // Satır-başına DEĞİL sayılır; döngü sonunda tek feed-seviyesi özet (büyük feed OOM önlemi).
        if cols.timepoint.is_some() && tp_raw.trim().is_empty() {
            stm050_empty += 1;
        }
        let timepoint = match parse_u32_raw(tp_raw, "timepoint") {
            Ok(v) => {
                if let Some(val) = v {
                    if val > 1 {
                        notices.push(make_k2_notice(
                            &mut counter, "STM_022", EntityType::Trip, eid(),
                            None, &file.name, Some(line), Some("timepoint"),
                            Some(val.to_string()), Some("0 veya 1".to_string()),
                            "timepoint 0 veya 1 olmalıdır.".to_string(),
                            "timepoint değerini 0 (yaklaşık) veya 1 (kesin) olarak ayarlayın.",
                        ));
                    }
                }
                v
            }
            Err(_) => None,
        };

        // STM_047: timepoint=1 (kesin zaman noktası) iken hem arrival_time hem departure_time
        // eksik. Yalnızca biri dolu olduğunda STM_034 asimetriyi yakalar → örtüşme yok.
        if timepoint == Some(1) && arrival_time.is_none() && departure_time.is_none() {
            notices.push(make_k2_notice(
                &mut counter, "STM_047", EntityType::Trip, eid(),
                None, &file.name, Some(line), Some("arrival_time"),
                Some(String::new()), Some("dolu".to_string()),
                format!("trip_id '{}' satırında timepoint=1 (kesin zaman noktası) ama arrival_time ve departure_time tanımlı değil.", trip_id),
                "Kesin zaman noktalarında (timepoint=1) hem arrival_time hem departure_time değerlerini girin.",
            ));
        }

        // shape_dist_traveled: non-negative
        let sdt_raw = get_col(row, cols.shape_dist_traveled);
        let shape_dist_traveled = match parse_f64_raw(sdt_raw, "shape_dist_traveled") {
            Ok(v) => {
                if let Some(d) = v {
                    if d < 0.0 {
                        notices.push(make_k2_notice(
                            &mut counter, "STM_030", EntityType::Trip, eid(),
                            None, &file.name, Some(line), Some("shape_dist_traveled"),
                            Some(d.to_string()), Some(">= 0".to_string()),
                            "shape_dist_traveled negatif olamaz.".to_string(),
                            "shape_dist_traveled değerini sıfır veya pozitif bir sayıya ayarlayın.",
                        ));
                    }
                }
                v
            }
            Err(_) => None,
        };

        let stop_headsign_raw = get_col(row, cols.stop_headsign);
        let stop_headsign = if stop_headsign_raw.is_empty() {
            None
        } else {
            Some(SmolStr::new(stop_headsign_raw))
        };

        // STM_042: stop_headsign'da Google Transit'in yasakladığı özel karakterler
        if let Some(ref hs) = stop_headsign {
            const FORBIDDEN: &[char] = &['!', '$', '%', '\\', '*', '=', '_'];
            if let Some(bad) = hs.chars().find(|c| FORBIDDEN.contains(c)) {
                notices.push(make_k2_notice(
                    &mut counter, "STM_042", EntityType::Trip, eid(),
                    None, &file.name, Some(line), Some("stop_headsign"),
                    Some(format!("'{bad}' karakteri içeriyor")), None,
                    format!("stop_headsign '{}' Google Transit tarafından desteklenmeyen karakter içeriyor: '{bad}'.", hs),
                    "stop_headsign değerinden ! $ % \\ * = _ karakterlerini kaldırın.",
                ));
            }
        }

        // ── Flex GTFS alanları (B1: yalnızca feed'de flex sütunu varsa işlenir) ──
        let (start_window, end_window, location_id, location_group_id,
             pickup_booking_rule_id, drop_off_booking_rule_id): (
            Option<(u32, u32, u32)>, Option<(u32, u32, u32)>,
            Option<SmolStr>, Option<SmolStr>, Option<SmolStr>, Option<SmolStr>,
        ) = if has_flex_cols {
        let start_window_raw = get_col(row, cols.start_pickup_drop_off_window);
        let end_window_raw   = get_col(row, cols.end_pickup_drop_off_window);
        let loc_id_raw       = get_col(row, cols.location_id);
        let loc_grp_raw      = get_col(row, cols.location_group_id);
        let pbr_raw          = get_col(row, cols.pickup_booking_rule_id);
        let dobr_raw         = get_col(row, cols.drop_off_booking_rule_id);

        let start_window = parse_gtfs_time_raw(start_window_raw, "start_pickup_drop_off_window").ok().flatten();
        let end_window   = parse_gtfs_time_raw(end_window_raw,   "end_pickup_drop_off_window").ok().flatten();

        let has_start_window = !start_window_raw.is_empty();
        let has_end_window   = !end_window_raw.is_empty();
        let has_any_window   = has_start_window || has_end_window;
        let has_location     = !loc_id_raw.is_empty() || !loc_grp_raw.is_empty();

        // STM_037: Flex penceresinde arrival_time/departure_time yasak
        if has_any_window && (arrival_time.is_some() || departure_time.is_some()) {
            notices.push(make_k2_notice(
                &mut counter, "STM_037", EntityType::Trip, eid(),
                None, &file.name, Some(line), Some("arrival_time"),
                Some(arr_raw.to_string()), Some("(boş)".to_string()),
                format!("trip_id '{}' Flex penceresi tanımlı iken arrival_time/departure_time yasaktır.", trip_id),
                "Flex stop_times satırlarında arrival_time ve departure_time alanlarını kaldırın.",
            ));
        }

        // STM_051: Flex penceresi tanımlı iken pickup_type=0 (düzenli) veya 3 (sürücü koordinasyonu) yasak.
        // GTFS spec [Kesin]: pickup_type 0/3 forbidden if start/end_pickup_drop_off_window defined.
        if has_any_window {
            if let Some(pt) = pickup_type {
                if pt == 0 || pt == 3 {
                    notices.push(make_k2_notice(
                        &mut counter, "STM_051", EntityType::Trip, eid(),
                        None, &file.name, Some(line), Some("pickup_type"),
                        Some(pt.to_string()), Some("1 veya 2".to_string()),
                        format!("trip_id '{trip_id}' Flex penceresi tanımlı iken pickup_type={pt} yasaktır."),
                        "Flex penceresi olan stop_times satırlarında pickup_type'ı 1 (alış yok) veya 2 (telefonla) yapın.",
                    ));
                }
            }
            // STM_052: Flex penceresi tanımlı iken drop_off_type=0 (düzenli) yasak.
            if drop_off_type == Some(0) {
                notices.push(make_k2_notice(
                    &mut counter, "STM_052", EntityType::Trip, eid(),
                    None, &file.name, Some(line), Some("drop_off_type"),
                    Some("0".to_string()), Some("1 veya 2".to_string()),
                    format!("trip_id '{trip_id}' Flex penceresi tanımlı iken drop_off_type=0 yasaktır."),
                    "Flex penceresi olan stop_times satırlarında drop_off_type'ı 1 (iniş yok) veya 2 (telefonla) yapın.",
                ));
            }
        }

        // STM_038: start_window > end_window
        if let (Some(sw), Some(ew)) = (start_window, end_window) {
            let sw_secs = sw.0 * 3600 + sw.1 * 60 + sw.2;
            let ew_secs = ew.0 * 3600 + ew.1 * 60 + ew.2;
            if sw_secs > ew_secs {
                notices.push(make_k2_notice(
                    &mut counter, "STM_038", EntityType::Trip, eid(),
                    None, &file.name, Some(line), Some("start_pickup_drop_off_window"),
                    Some(format!("{start_window_raw} > {end_window_raw}")), None,
                    format!("trip_id '{}' start_pickup_drop_off_window, end_pickup_drop_off_window'dan sonra.", trip_id),
                    "start_pickup_drop_off_window değerini end_pickup_drop_off_window'dan küçük ya da eşit yapın.",
                ));
            }
        }

        // STM_039: location_id/group_id var ama pencere eksik
        if has_location && (!has_start_window || !has_end_window) {
            let missing = if !has_start_window { "start_pickup_drop_off_window" } else { "end_pickup_drop_off_window" };
            notices.push(make_k2_notice(
                &mut counter, "STM_039", EntityType::Trip, eid(),
                None, &file.name, Some(line), Some(missing),
                None, Some("HH:MM:SS".to_string()),
                format!("trip_id '{}' location_id/group_id tanımlı ama {missing} eksik.", trip_id),
                "Flex stop_times için hem start_pickup_drop_off_window hem end_pickup_drop_off_window girin.",
            ));
        }

        // STM_040: Flex penceresi var ama booking_rule_id yok
        if has_any_window && pbr_raw.is_empty() && dobr_raw.is_empty() {
            notices.push(make_k2_notice(
                &mut counter, "STM_040", EntityType::Trip, eid(),
                None, &file.name, Some(line), Some("pickup_booking_rule_id"),
                None, None,
                format!("trip_id '{}' Flex penceresi tanımlı ama pickup/drop_off_booking_rule_id eksik.", trip_id),
                "Flex rezervasyon için pickup_booking_rule_id veya drop_off_booking_rule_id girin.",
            ));
        }

        // STM_041: stop_id ve location_id/group_id aynı anda dolu (çakışma)
        if !stop_id.is_empty() && has_location {
            notices.push(make_k2_notice(
                &mut counter, "STM_041", EntityType::Trip, eid(),
                None, &file.name, Some(line), Some("location_id"),
                Some(loc_id_raw.to_string()), Some("(boş)".to_string()),
                format!("trip_id '{}' stop_id ve location_id/group_id aynı anda tanımlı olamaz.", trip_id),
                "Standart stop için yalnızca stop_id, Flex stop için yalnızca location_id veya location_group_id kullanın.",
            ));
        }

        let smol_opt = |s: &str| if s.is_empty() { None } else { Some(SmolStr::new(s)) };

        let location_id         = smol_opt(loc_id_raw);
        let location_group_id   = smol_opt(loc_grp_raw);
        let pickup_booking_rule_id   = smol_opt(pbr_raw);
        let drop_off_booking_rule_id = smol_opt(dobr_raw);
            (start_window, end_window, location_id, location_group_id,
             pickup_booking_rule_id, drop_off_booking_rule_id)
        } else {
            (None, None, None, None, None, None)
        };

        // ── Per-trip toplama (çıktı setleri finalize'da türetilir) ──
        total_rows += 1;
        if !trip_id.is_empty() {
            let next_idx = trips_agg.len() as u32;
            let agg = trips_agg.entry(trip_id.clone()).or_insert_with(|| TripAgg::new(line, next_idx));
            if !stop_id.is_empty() {
                stop_first_line.entry(stop_id.clone()).or_insert(line);
                agg.stop_set.insert(stop_id.clone());
            }
            if matches!(continuous_pickup, Some(0) | Some(1))
                || matches!(continuous_drop_off, Some(0) | Some(1))
            {
                agg.continuous = true;
            }
            let trip_idx = agg.idx;
            all_rows.push((trip_idx, CompactStopTime {
                stop_id:            stop_id.clone(),
                stop_sequence,
                arrival_time,
                departure_time,
                stop_headsign:      stop_headsign.clone(),
                pickup_type,
                drop_off_type,
                shape_dist_traveled,
                timepoint,
                continuous_pickup,
                continuous_drop_off,
                line,
                flex: build_flex(
                    start_window,
                    end_window,
                    location_id.clone(),
                    location_group_id.clone(),
                    pickup_booking_rule_id.clone(),
                    drop_off_booking_rule_id.clone(),
                ),
            }));
        }
        };

        // ── Sürücü: stream raw_text (başlığı atla) veya rows fallback ──
        if let Some(text) = &file.raw_text {
            let mut pos = 0usize;
            let mut buf: Vec<Cow<'_, str>> = Vec::with_capacity(16);
            let mut data_idx = 0usize;
            let mut header_skipped = false;
            while next_csv_record(text, &mut pos, &mut buf) {
                // Boş satır (tek boş alan) — tokenize_csv ile aynı filtre
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
            // Fallback (test/legacy): önceden parse edilmiş rows üzerinden. SmolStr'ları
            // Cow::Borrowed olarak sar (alloc yok) — closure'ın &[Cow] imzasıyla uyum.
            for (row_idx, row) in file.rows.iter().enumerate() {
                let cow_row: Vec<Cow<'_, str>> =
                    row.iter().map(|s| Cow::Borrowed(s.as_str())).collect();
                process(&cow_row, (row_idx + 2) as u64);
            }
        }
    }

    // ARC_022: satır sayısı limiti (K1'den taşındı — stream_mode'da burada üretilir)
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

    // ARC_009: başlık var ama veri satırı yok (K1'den taşındı)
    if file.raw_text.is_some() && total_rows == 0 {
        notices.push(make_k2_notice(
            &mut counter, "ARC_009", EntityType::File, Some(file.name.clone()),
            None, &file.name, None, None,
            None, None,
            format!("'{}' dosyasında başlık satırı var ama veri satırı yok.", file.name),
            "Dosyaya en az bir veri satırı ekleyin.",
        ));
    }

    // STM_050: feed-seviyesi TEK özet (satır-başına değil — büyük feed OOM önlemi).
    if stm050_empty > 0 {
        notices.push(make_k2_notice(
            &mut counter, "STM_050", EntityType::Feed, None,
            None, &file.name, None, Some("timepoint"),
            Some(format!("{stm050_empty}")), None,
            format!("{stm050_empty} stop_times satırında timepoint sütunu var ama değer boş; bu satırlar örtük olarak 1 (kesin zaman) sayılır."),
            "Yaklaşık zamanlı duraklarda timepoint=0, kesin zamanlı duraklarda timepoint=1 olarak açıkça girin.",
        ));
    }

    // ── Finalize: counting-placement (Aşama 1b) ──
    // all_rows (trip_idx etiketli, dosya sırasında) → trip'e göre gruplu tek flat `rows`.
    // Per-trip Vec YOK; sayım → prefix-sum → doğrudan yerleştirme → per-trip range sort.
    let n_trips = trips_agg.len();
    let mut counts = vec![0u32; n_trips];
    for &(ti, _) in &all_rows {
        counts[ti as usize] += 1;
    }
    // offsets[i] = trip i'nin rows içindeki başlangıcı; offsets[n_trips] = toplam
    let mut offsets = vec![0u32; n_trips + 1];
    for i in 0..n_trips {
        offsets[i + 1] = offsets[i] + counts[i];
    }
    let total_placed = all_rows.len();
    let mut rows: Vec<CompactStopTime> = vec![CompactStopTime::default(); total_placed];
    let mut write: Vec<u32> = offsets[..n_trips].to_vec();
    for (ti, row) in all_rows.into_iter() {
        let w = write[ti as usize] as usize;
        rows[w] = row;
        write[ti as usize] += 1;
    }
    // Her trip dilimini stop_sequence'e göre sırala (dosya içi sıra korunur, sequence'e göre düzenlenir)
    for i in 0..n_trips {
        let (s, e) = (offsets[i] as usize, offsets[i + 1] as usize);
        rows[s..e].sort_by_key(|st| st.stop_sequence.unwrap_or(u32::MAX));
    }

    // stop_id_set = stop_first_line anahtarları (aynı guard'larla → birebir aynı küme).
    let mut index = StopTimesIndex {
        total_rows,
        unsorted_seq_trips,
        stop_first_line,
        rows,
        ..Default::default()
    };
    index.stop_id_set = index.stop_first_line.keys().cloned().collect();
    for (tid, agg) in trips_agg {
        if tid.is_empty() {
            continue; // boş trip_id çıktıya girmez (orijinal `if !trip_id.is_empty()` guard'ı)
        }
        let i = agg.idx as usize;
        index.trip_ranges.insert(tid.clone(), (offsets[i], offsets[i + 1]));
        index.trip_id_set.insert(tid.clone());
        index.trip_first_line.insert(tid.clone(), agg.first_line);
        if agg.continuous {
            index.continuous_trips.insert(tid.clone());
        }
        if !agg.stop_set.is_empty() {
            index.trip_stop_set.insert(tid.clone(), agg.stop_set);
        }
    }

    (index, notices)
}

fn parse_pickup_dropoff_col(
    raw: &str,
    notices: &mut Vec<gtfs_core::Notice>,
    counter: &mut u32,
    rule_id: &str,
    field: &str,
    trip_id: &str,
    line: u64,
    file_name: &str,
) -> Option<u32> {
    match parse_u32_raw(raw, field) {
        Ok(v) => {
            if let Some(val) = v {
                if val > 3 {
                    let entity_id = (!trip_id.is_empty()).then(|| trip_id.to_string());
                    notices.push(make_k2_notice(
                        counter, rule_id, EntityType::Trip, entity_id,
                        None, file_name, Some(line), Some(field),
                        Some(val.to_string()), Some("0-3".to_string()),
                        format!("{field} 0, 1, 2 veya 3 olmalıdır."),
                        "Alanı geçerli bir GTFS biniş/iniş enum değerine (0-3) ayarlayın.",
                    ));
                }
            }
            v
        }
        Err(err) => {
            let entity_id = (!trip_id.is_empty()).then(|| trip_id.to_string());
            notices.push(make_k2_notice(
                counter, rule_id, EntityType::Trip, entity_id,
                None, file_name, Some(line), Some(field),
                Some(raw.to_string()), Some("0-3".to_string()), err,
                "Alanı geçerli bir GTFS biniş/iniş enum değerine (0-3) ayarlayın.",
            ));
            None
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::k1_parse::RawFile;

    #[test]
    fn normalize_service_day_wraps_midnight_keeps_real_errors() {
        let rec = |tid: &str, seq: u32, t: (u32, u32, u32)| StopTimeRecord {
            trip_id: SmolStr::new(tid),
            stop_id: SmolStr::new(format!("S{seq}")),
            stop_sequence: Some(seq),
            arrival_time: Some(t),
            departure_time: Some(t),
            ..Default::default()
        };
        // T1: gece dönümü — 23:58 → 00:01 (00:01 < 03:00 eşik) → 24:01'e normalize.
        // T2: gerçek geriye-gidiş — 08:00 → 07:55 (07:55 ≥ 03:00) → dokunulmaz.
        let recs = vec![
            rec("T1", 1, (23, 58, 0)),
            rec("T1", 2, (0, 1, 0)),
            rec("T2", 1, (8, 0, 0)),
            rec("T2", 2, (7, 55, 0)),
        ];

        let mut idx = StopTimesIndex::from_records(&recs);
        idx.normalize_service_day(3);
        let t1 = idx.sorted_stops("T1").unwrap();
        assert_eq!(t1[0].departure_time, Some((23, 58, 0)), "ilk durak dokunulmamalı");
        assert_eq!(t1[1].arrival_time, Some((24, 1, 0)), "00:01 → 24:01 normalize");
        assert_eq!(t1[1].departure_time, Some((24, 1, 0)), "departure aynı offset'i takip eder");
        let t2 = idx.sorted_stops("T2").unwrap();
        assert_eq!(t2[1].arrival_time, Some((7, 55, 0)), "gerçek hata (eşik üstü) korunur");

        // start_hour = 0 → normalizasyon kapalı.
        let mut idx0 = StopTimesIndex::from_records(&recs);
        idx0.normalize_service_day(0);
        assert_eq!(idx0.sorted_stops("T1").unwrap()[1].arrival_time, Some((0, 1, 0)));
    }

    fn make_file(headers: Vec<&str>, rows: Vec<Vec<&str>>) -> RawFile {
        // OOM fix Plan A: testler de streaming yolunu kullanır — headers+rows CSV'ye
        // serialize edilir, rows boş bırakılır. (Test değerlerinde virgül/tırnak yok.)
        let mut text = headers.join(",");
        text.push('\n');
        for r in &rows {
            text.push_str(&r.join(","));
            text.push('\n');
        }
        RawFile {
            name: "stop_times.txt".to_string(),
            headers: headers.into_iter().map(str::to_string).collect(),
            rows: Vec::new(),
            bytes: 0,
            raw_text: Some(text),
        }
    }

    #[test]
    fn valid_stop_time_produces_no_notices() {
        let file = make_file(
            vec!["trip_id", "arrival_time", "departure_time", "stop_id", "stop_sequence"],
            vec![
                vec!["T1", "08:00:00", "08:00:00", "S1", "1"],
                vec!["T1", "08:10:00", "08:10:00", "S2", "2"],
            ],
        );
        let (records_idx, notices) = validate_stop_times(&file);
        assert_eq!(records_idx.rows.len(), 2);
        assert!(notices.is_empty(), "Geçerli stop_times notice üretmemeli: {:?}", notices);
    }

    #[test]
    fn missing_stop_sequence_produces_stm_005() {
        let file = make_file(
            vec!["trip_id", "arrival_time", "departure_time", "stop_id", "stop_sequence"],
            vec![vec!["T1", "08:00:00", "08:00:00", "S1", ""]],
        );
        let (_, notices) = validate_stop_times(&file);
        assert!(notices.iter().any(|n| n.rule_id == "STM_005"));
    }

    #[test]
    fn missing_stop_id_produces_stm_006() {
        let file = make_file(
            vec!["trip_id", "arrival_time", "departure_time", "stop_id", "stop_sequence"],
            vec![vec!["T1", "08:00:00", "08:00:00", "", "1"]],
        );
        let (_, notices) = validate_stop_times(&file);
        assert!(notices.iter().any(|n| n.rule_id == "STM_006"));
    }

    // STM_007 testi K6'ya taşındı (k6_analytics.rs: departure_before_arrival_*).

    #[test]
    fn invalid_pickup_type_produces_stm_009() {
        let file = make_file(
            vec!["trip_id", "arrival_time", "departure_time", "stop_id", "stop_sequence", "pickup_type"],
            vec![vec!["T1", "08:00:00", "08:00:00", "S1", "1", "9"]],
        );
        let (_, notices) = validate_stop_times(&file);
        assert!(notices.iter().any(|n| n.rule_id == "STM_009"));
    }

    #[test]
    fn invalid_timepoint_produces_stm_022() {
        let file = make_file(
            vec!["trip_id", "arrival_time", "departure_time", "stop_id", "stop_sequence", "timepoint"],
            vec![vec!["T1", "08:00:00", "08:00:00", "S1", "1", "5"]],
        );
        let (_, notices) = validate_stop_times(&file);
        assert!(notices.iter().any(|n| n.rule_id == "STM_022"));
    }

    #[test]
    fn timepoint_one_without_times_produces_stm_047() {
        let file = make_file(
            vec!["trip_id", "arrival_time", "departure_time", "stop_id", "stop_sequence", "timepoint"],
            vec![vec!["T1", "", "", "S1", "1", "1"]],
        );
        let (_, notices) = validate_stop_times(&file);
        let ids: Vec<&str> = notices.iter().map(|n| n.rule_id.as_str()).collect();
        assert!(ids.contains(&"STM_047"), "timepoint=1 + saatsiz → STM_047: {:?}", ids);
        assert!(!ids.contains(&"STM_034"), "Her ikisi boşken STM_034 tetiklenmemeli: {:?}", ids);
    }

    #[test]
    fn timepoint_one_with_one_time_is_stm_034_not_047() {
        let file = make_file(
            vec!["trip_id", "arrival_time", "departure_time", "stop_id", "stop_sequence", "timepoint"],
            vec![vec!["T1", "08:00:00", "", "S1", "1", "1"]],
        );
        let (_, notices) = validate_stop_times(&file);
        let ids: Vec<&str> = notices.iter().map(|n| n.rule_id.as_str()).collect();
        assert!(ids.contains(&"STM_034"), "Yalnız biri dolu → STM_034: {:?}", ids);
        assert!(!ids.contains(&"STM_047"), "Biri dolu iken STM_047 tetiklenmemeli: {:?}", ids);
    }

    #[test]
    fn timepoint_zero_without_times_silent_for_stm_047() {
        let file = make_file(
            vec!["trip_id", "arrival_time", "departure_time", "stop_id", "stop_sequence", "timepoint"],
            vec![vec!["T1", "", "", "S1", "1", "0"]],
        );
        let (_, notices) = validate_stop_times(&file);
        assert!(!notices.iter().any(|n| n.rule_id == "STM_047"),
            "timepoint=0 (yaklaşık) → STM_047 tetiklenmemeli: {:?}",
            notices.iter().map(|n| &n.rule_id).collect::<Vec<_>>());
    }

    #[test]
    fn empty_timepoint_value_produces_stm_050() {
        // 3 boş-timepoint satırı → TEK feed-seviyesi STM_050 özeti (satır-başına DEĞİL),
        // observed_value = 3. (Büyük feed OOM önlemi: bkz. Kocaeli 2.4M.)
        let file = make_file(
            vec!["trip_id", "arrival_time", "departure_time", "stop_id", "stop_sequence", "timepoint"],
            vec![
                vec!["T1", "08:00:00", "08:00:00", "S1", "1", ""],
                vec!["T1", "08:10:00", "08:10:00", "S2", "2", ""],
                vec!["T2", "09:00:00", "09:00:00", "S1", "1", ""],
            ],
        );
        let (_, notices) = validate_stop_times(&file);
        let stm050: Vec<&_> = notices.iter().filter(|n| n.rule_id == "STM_050").collect();
        assert_eq!(stm050.len(), 1, "STM_050 satır-başına değil TEK özet olmalı: {}", stm050.len());
        assert_eq!(stm050[0].observed_value.as_deref(), Some("3"), "özet 3 boş satır saymalı");
        assert!(!notices.iter().any(|n| n.rule_id == "STM_022"), "boş değer STM_022 tetiklememeli");
    }

    #[test]
    fn absent_timepoint_column_silent_for_stm_050() {
        let file = make_file(
            vec!["trip_id", "arrival_time", "departure_time", "stop_id", "stop_sequence"],
            vec![vec!["T1", "08:00:00", "08:00:00", "S1", "1"]],
        );
        let (_, notices) = validate_stop_times(&file);
        assert!(!notices.iter().any(|n| n.rule_id == "STM_050"),
            "timepoint kolonu yok → STM_050 tetiklenmemeli: {:?}",
            notices.iter().map(|n| &n.rule_id).collect::<Vec<_>>());
    }

    #[test]
    fn flex_window_forbidden_pickup_type_produces_stm_051() {
        let file = make_file(
            vec!["trip_id", "stop_sequence", "location_id", "start_pickup_drop_off_window", "end_pickup_drop_off_window", "pickup_type", "drop_off_type"],
            vec![vec!["T1", "1", "Z1", "08:00:00", "18:00:00", "0", "2"]],
        );
        let (_, notices) = validate_stop_times(&file);
        let ids: Vec<&str> = notices.iter().map(|n| n.rule_id.as_str()).collect();
        assert!(ids.contains(&"STM_051"), "Flex penceresi + pickup_type=0 → STM_051: {:?}", ids);
    }

    #[test]
    fn flex_window_forbidden_drop_off_type_produces_stm_052() {
        let file = make_file(
            vec!["trip_id", "stop_sequence", "location_id", "start_pickup_drop_off_window", "end_pickup_drop_off_window", "pickup_type", "drop_off_type"],
            vec![vec!["T1", "1", "Z1", "08:00:00", "18:00:00", "2", "0"]],
        );
        let (_, notices) = validate_stop_times(&file);
        assert!(notices.iter().any(|n| n.rule_id == "STM_052"),
            "Flex penceresi + drop_off_type=0 → STM_052: {:?}",
            notices.iter().map(|n| &n.rule_id).collect::<Vec<_>>());
    }

    #[test]
    fn no_flex_window_pickup_type_zero_silent_for_stm_051() {
        let file = make_file(
            vec!["trip_id", "arrival_time", "departure_time", "stop_id", "stop_sequence", "pickup_type"],
            vec![vec!["T1", "08:00:00", "08:00:00", "S1", "1", "0"]],
        );
        let (_, notices) = validate_stop_times(&file);
        assert!(!notices.iter().any(|n| n.rule_id == "STM_051"),
            "Flex penceresi yok → pickup_type=0 STM_051 üretmemeli: {:?}",
            notices.iter().map(|n| &n.rule_id).collect::<Vec<_>>());
    }

    // ── STM_023/032 + finalize çıktı alanları (K2 hashmap-birleştirme refactor güvenlik ağı) ──
    #[test]
    fn duplicate_stop_sequence_produces_stm_032() {
        let file = make_file(
            vec!["trip_id", "arrival_time", "departure_time", "stop_id", "stop_sequence"],
            vec![
                vec!["T1", "08:00:00", "08:00:00", "S1", "1"],
                vec!["T1", "08:10:00", "08:10:00", "S2", "1"], // aynı stop_sequence tekrar
            ],
        );
        let (_, notices) = validate_stop_times(&file);
        assert!(notices.iter().any(|n| n.rule_id == "STM_032"), "STM_032 bekleniyor: {:?}", notices);
    }

    #[test]
    fn out_of_order_stop_sequence_produces_stm_023() {
        let file = make_file(
            vec!["trip_id", "arrival_time", "departure_time", "stop_id", "stop_sequence"],
            vec![
                vec!["T1", "08:00:00", "08:00:00", "S1", "5"],
                vec!["T1", "08:10:00", "08:10:00", "S2", "2"], // 2 < 5 → dosya sırası bozuk
            ],
        );
        let (idx, notices) = validate_stop_times(&file);
        assert!(notices.iter().any(|n| n.rule_id == "STM_023"), "STM_023 bekleniyor: {:?}", notices);
        assert_eq!(idx.unsorted_seq_trips.len(), 1, "STM_023 unsorted_seq_trips'e bir kayıt eklemeli");
    }

    #[test]
    fn finalize_index_fields_preserved() {
        let file = make_file(
            vec!["trip_id", "arrival_time", "departure_time", "stop_id", "stop_sequence", "continuous_pickup"],
            vec![
                vec!["T1", "08:00:00", "08:00:00", "S1", "1", "0"], // continuous_pickup=0 → T1 continuous
                vec!["T1", "08:10:00", "08:10:00", "S2", "2", ""],
                vec!["T2", "09:00:00", "09:00:00", "S1", "1", ""],
            ],
        );
        let (idx, _) = validate_stop_times(&file);
        assert_eq!(idx.total_rows, 3);
        assert_eq!(idx.trip_ranges.len(), 2);
        assert!(idx.trip_id_set.contains("T1") && idx.trip_id_set.contains("T2"));
        assert!(idx.stop_id_set.contains("S1") && idx.stop_id_set.contains("S2"));
        assert_eq!(idx.trip_first_line.get("T1").copied(), Some(2)); // ilk veri satırı = satır 2
        assert_eq!(idx.trip_first_line.get("T2").copied(), Some(4));
        assert_eq!(idx.stop_first_line.get("S1").copied(), Some(2)); // S1 ilk satır 2'de
        assert!(idx.continuous_trips.contains("T1"), "T1 continuous_pickup=0 → continuous_trips'te olmalı");
        assert!(!idx.continuous_trips.contains("T2"));
        assert_eq!(idx.trip_stop_set.get("T1").map(|s| s.len()), Some(2)); // S1,S2
        // trips stop_sequence'e göre sıralı
        let t1 = idx.sorted_stops("T1").expect("T1 trips'te olmalı");
        assert_eq!(t1[0].stop_sequence, Some(1));
        assert_eq!(t1[1].stop_sequence, Some(2));
    }

    #[test]
    fn scope_key_is_trip_id() {
        let file = make_file(
            vec!["trip_id", "arrival_time", "departure_time", "stop_id", "stop_sequence"],
            vec![vec!["TRIP_X", "08:00:00", "08:00:00", "S1", ""]],
        );
        let (_, notices) = validate_stop_times(&file);
        let n = notices.iter().find(|n| n.rule_id == "STM_005").expect("STM_005 olmalı");
        assert_eq!(n.scope_key.as_deref(), Some("TRIP_X"), "scope_key trip_id olmalı");
    }

    #[test]
    fn only_arrival_time_produces_stm_034() {
        let file = make_file(
            vec!["trip_id", "arrival_time", "departure_time", "stop_id", "stop_sequence"],
            vec![vec!["T1", "08:00:00", "", "S1", "1"]],
        );
        let (_, notices) = validate_stop_times(&file);
        assert!(notices.iter().any(|n| n.rule_id == "STM_034"),
            "Yalnızca arrival_time dolu → STM_034 olmalı. Notices: {:?}", notices);
    }

    #[test]
    fn only_departure_time_produces_stm_034() {
        let file = make_file(
            vec!["trip_id", "arrival_time", "departure_time", "stop_id", "stop_sequence"],
            vec![vec!["T1", "", "08:00:00", "S1", "1"]],
        );
        let (_, notices) = validate_stop_times(&file);
        assert!(notices.iter().any(|n| n.rule_id == "STM_034"),
            "Yalnızca departure_time dolu → STM_034 olmalı. Notices: {:?}", notices);
    }

    #[test]
    fn both_times_empty_no_stm_034() {
        let file = make_file(
            vec!["trip_id", "arrival_time", "departure_time", "stop_id", "stop_sequence"],
            vec![vec!["T1", "", "", "S1", "1"]],
        );
        let (_, notices) = validate_stop_times(&file);
        assert!(!notices.iter().any(|n| n.rule_id == "STM_034"),
            "İkisi de boş → STM_034 üretilmemeli. Notices: {:?}", notices);
    }

    // ── STM Flex ────────────────────────────────────────────────────────────────

    #[test]
    fn stm_037_arrival_time_forbidden_in_flex_window() {
        let file = make_file(
            vec!["trip_id", "stop_id", "stop_sequence", "start_pickup_drop_off_window", "arrival_time", "departure_time"],
            vec![vec!["T1", "", "1", "08:00:00", "08:00:00", ""]],
        );
        let (_, notices) = validate_stop_times(&file);
        assert!(notices.iter().any(|n| n.rule_id == "STM_037"), "STM_037 bekleniyor: {:?}", notices);
    }

    #[test]
    fn stm_038_start_window_after_end_window() {
        let file = make_file(
            vec!["trip_id", "stop_id", "stop_sequence", "start_pickup_drop_off_window", "end_pickup_drop_off_window", "pickup_booking_rule_id"],
            vec![vec!["T1", "", "1", "10:00:00", "09:00:00", "BR1"]],
        );
        let (_, notices) = validate_stop_times(&file);
        assert!(notices.iter().any(|n| n.rule_id == "STM_038"), "STM_038 bekleniyor: {:?}", notices);
    }

    #[test]
    fn stm_039_location_id_without_window() {
        let file = make_file(
            vec!["trip_id", "stop_sequence", "location_id"],
            vec![vec!["T1", "1", "LOC1"]],
        );
        let (_, notices) = validate_stop_times(&file);
        assert!(notices.iter().any(|n| n.rule_id == "STM_039"), "STM_039 bekleniyor: {:?}", notices);
    }

    #[test]
    fn stm_040_flex_window_without_booking_rule() {
        let file = make_file(
            vec!["trip_id", "stop_sequence", "start_pickup_drop_off_window", "end_pickup_drop_off_window"],
            vec![vec!["T1", "1", "08:00:00", "10:00:00"]],
        );
        let (_, notices) = validate_stop_times(&file);
        assert!(notices.iter().any(|n| n.rule_id == "STM_040"), "STM_040 bekleniyor: {:?}", notices);
    }

    #[test]
    fn stm_041_stop_id_and_location_id_conflict() {
        let file = make_file(
            vec!["trip_id", "stop_id", "stop_sequence", "location_id", "start_pickup_drop_off_window", "end_pickup_drop_off_window", "pickup_booking_rule_id"],
            vec![vec!["T1", "S1", "1", "LOC1", "08:00:00", "10:00:00", "BR1"]],
        );
        let (_, notices) = validate_stop_times(&file);
        assert!(notices.iter().any(|n| n.rule_id == "STM_041"), "STM_041 bekleniyor: {:?}", notices);
    }

    #[test]
    fn valid_flex_stop_time_no_notices() {
        let file = make_file(
            vec!["trip_id", "stop_sequence", "location_id", "start_pickup_drop_off_window", "end_pickup_drop_off_window", "pickup_booking_rule_id"],
            vec![vec!["T1", "1", "LOC1", "08:00:00", "10:00:00", "BR1"]],
        );
        let (_, notices) = validate_stop_times(&file);
        let flex_notices: Vec<_> = notices.iter().filter(|n| matches!(n.rule_id.as_str(), "STM_037"|"STM_038"|"STM_039"|"STM_040"|"STM_041")).collect();
        assert!(flex_notices.is_empty(), "Geçerli Flex stop_time için Flex notice olmamalı: {:?}", flex_notices);
    }
}
