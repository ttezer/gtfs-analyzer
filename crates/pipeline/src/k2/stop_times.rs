use gtfs_core::{EntityType, Notice, Severity};
use rustc_hash::{FxHashMap, FxHashSet};
use smol_str::SmolStr;
use std::borrow::Cow;
use std::io::{Cursor, Read};

use super::common::{get_col_raw, get_col, make_k2_notice};
use crate::k1_parse::{RawFile, RFC4180_AFTER_CLOSE, RFC4180_BARE_QUOTE};

// ── CompactStopTime: 24B flat per row — trip_id taşımaz ──────────────────────
// #38: Swiss feed 28M×112B=3.1GB → OOM; 28M×24B=672MB → bütçe dahilinde.
// stop_id → stop_idx (intern); zamanlar saniye (sentinel u32::MAX); f32 dist.
// stop_headsign + flex → StopTimesIndex.stop_headsigns/flex_map (line-anahtarlı).

#[derive(Debug, Clone)]
pub struct CompactStopTime {
    /// stop_id intern indeksi (StopTimesIndex.stop_intern[stop_idx]); u32::MAX = boş/eksik
    pub stop_idx: u32,
    /// Varış zamanı gece yarısından saniye; u32::MAX = None
    pub arrival_secs: u32,
    /// Kalkış zamanı gece yarısından saniye; u32::MAX = None
    pub departure_secs: u32,
    /// stop_sequence değeri; u32::MAX = None
    pub sequence: u32,
    /// shape_dist_traveled f32 olarak; f32::NAN = None. f64→f32 precision farkı kabul edildi.
    pub dist_f32: f32,
    /// CSV satır numarası (28M satır < u32::MAX; u64'ten u32'ye indirgendi)
    pub line: u32,
}

impl Default for CompactStopTime {
    fn default() -> Self {
        Self {
            stop_idx: u32::MAX,
            arrival_secs: u32::MAX,
            departure_secs: u32::MAX,
            sequence: u32::MAX,
            dist_f32: f32::NAN,
            line: 0,
        }
    }
}

/// "Alan DOLU ama ayrıştırılamadı" sentineli — `u32::MAX` ("alan YOK") ile ayrı tutulur.
///
/// 🔴 İki durum tek değere düşünce STM_015/016 mdb-2727'de 101.621 kez "ilk/son durakta
/// arrival_time eksik" dedi; alan doluydu, yalnız `HH:MM` yazılmıştı. Aynı satırlarda
/// `STM_003` zaten "HH:MM:SS bekleniyordu" diyordu, yani doğru kural konuşuyor ve
/// yanındaki yanlış olanı Spec·Kritik ile R1 yayın kapısına sokuyordu. İddia "değer yok"
/// olduğunda, değerin VARLIĞI kuralın önkoşuludur — `Err(_) => None` bu ayrımı siler.
pub const TIME_MALFORMED: u32 = u32::MAX - 1;

impl CompactStopTime {
    /// arrival_time → Option<(saat, dakika, saniye)>; yok VEYA bozuk → None.
    ///
    /// Bozuk değeri de None döndürür, çünkü hesap yapan her çağrı yeri (hız, süre,
    /// sıralama) için kullanılamaz bir değerdir. "Alan boş mu" diye SORAN kurallar
    /// `arrival_is_absent()` kullanmalıdır.
    #[inline]
    pub fn arrival_time(&self) -> Option<(u32, u32, u32)> {
        if self.arrival_secs == u32::MAX || self.arrival_secs == TIME_MALFORMED { None }
        else { let s = self.arrival_secs; Some((s / 3600, (s % 3600) / 60, s % 60)) }
    }
    /// Alan GERÇEKTEN boş mu — dolu-ama-bozuk vakası `false` döner.
    #[inline]
    pub fn arrival_is_absent(&self) -> bool { self.arrival_secs == u32::MAX }
    /// Alan GERÇEKTEN boş mu — dolu-ama-bozuk vakası `false` döner.
    #[inline]
    pub fn departure_is_absent(&self) -> bool { self.departure_secs == u32::MAX }
    /// departure_time → Option<(saat, dakika, saniye)>; u32::MAX → None
    #[inline]
    pub fn departure_time(&self) -> Option<(u32, u32, u32)> {
        if self.departure_secs == u32::MAX || self.departure_secs == TIME_MALFORMED { None }
        else { let s = self.departure_secs; Some((s / 3600, (s % 3600) / 60, s % 60)) }
    }
    /// stop_sequence → Option<u32>; u32::MAX → None
    #[inline]
    pub fn stop_sequence(&self) -> Option<u32> {
        if self.sequence == u32::MAX { None } else { Some(self.sequence) }
    }
    /// shape_dist_traveled → Option<f64>; f32::NAN → None
    #[inline]
    pub fn shape_dist_traveled(&self) -> Option<f64> {
        if self.dist_f32.is_nan() { None } else { Some(self.dist_f32 as f64) }
    }
    /// line as u64 (notice API'si u64 bekler)
    #[inline]
    pub fn line_u64(&self) -> u64 { self.line as u64 }
}

// #38: CompactStopTime boyut guard — 24B'ı aşarsa derleme kırılır.
const _: () = assert!(std::mem::size_of::<CompactStopTime>() <= 24);

/// CompactStopTime'ın nadir kullanılan GTFS-Flex alanları (Box'lanır — bkz. CompactStopTime.flex).
#[derive(Debug, Clone, Default)]
pub struct StopTimeFlex {
    pub start_pickup_drop_off_window:  Option<(u32, u32, u32)>,
    pub end_pickup_drop_off_window:    Option<(u32, u32, u32)>,
    pub location_id:                   Option<SmolStr>,
    pub location_group_id:             Option<SmolStr>,
    pub pickup_booking_rule_id:        Option<SmolStr>,
    pub drop_off_booking_rule_id:      Option<SmolStr>,
    /// STM_060 için: `CompactStopTime` 24 bayta sıkıştırıldığından pickup/drop_off tipini
    /// TAŞIMAZ. Bu kutu yalnız Flex satırlarında ayrıldığı için tipleri burada tutmak
    /// normal feed'lere SIFIR maliyettir (VBB'nin 6,1M satırında hiç kutu ayrılmaz).
    pub pickup_type:                   Option<u32>,
    pub drop_off_type:                 Option<u32>,
}

/// 6 flex değerinden herhangi biri Some ise Box'lanmış StopTimeFlex, aksi halde None döner.
// Flex alanları ayrı tutulur çünkü her biri doğrudan StopTimeFlex alanına karşılık
// gelir; parametre struct'ı bu birebir tabloyu gizler.
#[allow(clippy::too_many_arguments)]
#[inline]
pub fn build_flex(
    start_window: Option<(u32, u32, u32)>,
    end_window: Option<(u32, u32, u32)>,
    location_id: Option<SmolStr>,
    location_group_id: Option<SmolStr>,
    pickup_booking_rule_id: Option<SmolStr>,
    drop_off_booking_rule_id: Option<SmolStr>,
    // ⚠️ Kutunun AYRILIP ayrılmayacağını bu ikisi ETKİLEMEZ — koşul yine 6 flex alanıdır.
    // Aksi hâlde `pickup_type` yazan her normal satır kutu ayırırdı.
    pickup_type: Option<u32>,
    drop_off_type: Option<u32>,
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
            pickup_type,
            drop_off_type,
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
    /// K4: XFL_021 — per-trip stop_idx seti (u32 intern indeksleri; eski SmolStr'dan ~560 MB tasarruf)
    pub trip_stop_set: FxHashMap<SmolStr, FxHashSet<u32>>,
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
    /// K6/diagnostic: normalize_service_day'in 00:xx→24:xx kaydırdığı trip sayısı.
    pub midnight_wrapped_trips: u32,
    /// #38: stop_idx → stop_id (intern tablosu). Index = stop_idx değeri.
    pub stop_intern: Vec<SmolStr>,
    /// #38: stop_id → stop_idx (ters arama; K4 XFL_021 için)
    pub stop_id_to_idx: FxHashMap<SmolStr, u32>,
    /// #38: CSV line → stop_headsign (nadir; CompactStopTime dışına taşındı)
    /// Key = st.line (u32). Sort/permutation'dan bağımsız — line CSV'de sabittir.
    pub stop_headsigns: FxHashMap<u32, SmolStr>,
    /// #38: CSV line → StopTimeFlex (feed'lerin %99'unda boş; CompactStopTime dışına taşındı)
    pub flex_map: FxHashMap<u32, Box<StopTimeFlex>>,
}

/// Raw saatlerin küçük bir gün-içi hatadan ziyade gece yarısı yazımına döndüğünü
/// ayırt etmek için kullanılan sabit analiz sezgisi. Bu, servis-günü normalizasyonunun
/// kullanıcı ayarı değildir; yalnızca `STM_048/049` raw Spec kanıtını sınıflandırır.
const RAW_MIDNIGHT_MIN_BACKSTEP_SECS: u32 = 6 * 3600;

fn raw_midnight_rollover(previous: u32, current: u32) -> bool {
    current < previous
        && previous.saturating_sub(current) >= RAW_MIDNIGHT_MIN_BACKSTEP_SECS
}

/// Tek bir raw after-midnight ihlalinin kullanıcıya gösterilecek kanıtı.
/// Feed-level sayaç yerine bunu taşımak, High · Spec bulgusunun hangi trip/satırda
/// düzeltileceğini kaybetmememizi sağlar.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MidnightViolation {
    pub trip_id: SmolStr,
    pub stop_id: SmolStr,
    pub line: u64,
    pub field: &'static str,
    pub previous_secs: u32,
    pub observed_secs: u32,
    pub expected_secs: u32,
}

fn format_gtfs_time(secs: u32) -> String {
    format!("{:02}:{:02}:{:02}", secs / 3600, (secs % 3600) / 60, secs % 60)
}

impl StopTimesIndex {
    /// trip başına sıralı (stop_sequence artan) stop dilimi. `idx.trips.get(...)` yerine BU kullanılır.
    pub fn sorted_stops(&self, trip_id: &str) -> Option<&[CompactStopTime]> {
        let &(s, e) = self.trip_ranges.get(trip_id)?;
        Some(&self.rows[s as usize..e as usize])
    }

    /// stop_idx'ten stop_id &str; u32::MAX veya out-of-bounds → ""
    pub fn stop_id_of_idx(&self, idx: u32) -> &str {
        if idx == u32::MAX { return ""; }
        self.stop_intern.get(idx as usize).map(|s| s.as_str()).unwrap_or("")
    }
    /// CompactStopTime'ın stop_id'sini &str olarak döner
    pub fn stop_id_of(&self, st: &CompactStopTime) -> &str {
        self.stop_id_of_idx(st.stop_idx)
    }
    /// CompactStopTime'ın stop_id'sini SmolStr olarak döner
    pub fn stop_smolstr_of(&self, st: &CompactStopTime) -> SmolStr {
        if st.stop_idx == u32::MAX { SmolStr::default() }
        else { self.stop_intern.get(st.stop_idx as usize).cloned().unwrap_or_default() }
    }
    /// CompactStopTime'ın stop_headsign'ını döner (CSV line anahtarlı side map'ten)
    pub fn stop_headsign_of(&self, st: &CompactStopTime) -> Option<&SmolStr> {
        self.stop_headsigns.get(&st.line)
    }
    /// CompactStopTime'ın flex alanlarını döner (CSV line anahtarlı side map'ten)
    pub fn flex_of(&self, st: &CompactStopTime) -> Option<&StopTimeFlex> {
        self.flex_map.get(&st.line).map(|b| b.as_ref())
    }

    /// Raw stop_times'ta açık bir saat dönümünü, normalization ayarından bağımsız ölçer.
    /// GTFS raw Spec bulgusu için kullanılır; analitik normalizasyonun `start_hour` eşiği
    /// burada uygulanmaz. Küçük gün-içi geriye gidişler (08:00 → 07:55 gibi) STM_008'e
    /// bırakılır; büyük geri sıçrama ise ham 00:xx yazımının güçlü göstergesidir.
    pub fn find_raw_midnight_wraps(&self) -> Vec<MidnightViolation> {
        let mut violations = Vec::new();
        for (trip_id, &(s, e)) in &self.trip_ranges {
            let slice = &self.rows[s as usize..e as usize];
            let mut prev = None;
            for st in slice {
                // `< TIME_MALFORMED` — `!= u32::MAX` DEĞİL: bozuk değer de kullanılamaz
                // ve aritmetiğe sokulursa saçma bir saniye değeri gibi davranır.
                if st.arrival_secs < TIME_MALFORMED {
                    let raw = st.arrival_secs;
                    if let Some(previous) = prev.filter(|p| raw_midnight_rollover(*p, raw)) {
                        violations.push(MidnightViolation {
                            trip_id: trip_id.clone(),
                            stop_id: self.stop_smolstr_of(st),
                            line: st.line_u64(),
                            field: "arrival_time",
                            previous_secs: previous,
                            observed_secs: raw,
                            expected_secs: raw.saturating_add(86_400),
                        });
                    }
                    prev = Some(raw);
                }
            }
        }
        violations
    }

    /// Eski aggregate API'nin uyumluluk ölçümü; production emit'i olay listesini kullanır.
    pub fn count_raw_midnight_wrap_trips(&self) -> u32 {
        self.find_raw_midnight_wraps().len() as u32
    }

    /// Aynı satırda arrival_time → departure_time yönündeki raw after-midnight
    /// geçişlerini ölçer. STM_049 artık STM_048 ile aynı GTFS Spec dayanağını kullanır;
    /// bu yüzden bu sayım da `service_day_start_hour`'dan bağımsızdır.
    pub fn find_raw_same_row_midnight_departures(&self) -> Vec<MidnightViolation> {
        let mut violations = Vec::new();
        for (trip_id, &(s, e)) in &self.trip_ranges {
            for st in &self.rows[s as usize..e as usize] {
                if st.arrival_secs >= TIME_MALFORMED || st.departure_secs >= TIME_MALFORMED {
                    continue;
                }
                if raw_midnight_rollover(st.arrival_secs, st.departure_secs) {
                    violations.push(MidnightViolation {
                        trip_id: trip_id.clone(),
                        stop_id: self.stop_smolstr_of(st),
                        line: st.line_u64(),
                        field: "departure_time",
                        previous_secs: st.arrival_secs,
                        observed_secs: st.departure_secs,
                        expected_secs: st.departure_secs.saturating_add(86_400),
                    });
                }
            }
        }
        violations
    }

    pub fn count_raw_same_row_midnight_departures(&self) -> u32 {
        self.find_raw_same_row_midnight_departures().len() as u32
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
        if start_hour == 0 { return; }
        let start_secs = start_hour * 3600;
        let mut wrapped = 0u32;
        for &(s, e) in self.trip_ranges.values() {
            let slice = &mut self.rows[s as usize..e as usize];
            let mut offset: u32 = 0;
            let mut prev: Option<u32> = None;
            for st in slice.iter_mut() {
                if st.arrival_secs < TIME_MALFORMED {
                    let raw = st.arrival_secs;
                    if let Some(p) = prev {
                        if raw.saturating_add(offset) < p && raw < start_secs {
                            offset += 86400;
                        }
                    }
                    if offset > 0 { st.arrival_secs = raw + offset; }
                    prev = Some(raw + offset);
                }
                if st.departure_secs < TIME_MALFORMED {
                    let raw = st.departure_secs;
                    let v = raw + offset; // offset'i takip eder; kendi unwrap'ını tetiklemez
                    if offset > 0 { st.departure_secs = v; }
                    prev = Some(v);
                }
            }
            if offset > 0 { wrapped += 1; }
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

    /// #15 teşhis: index yapılarının GERÇEK (canlı) bellek maliyetini logla.
    /// high-water (`memory_size`) DEĞİL — her yapının `len × size_of` + SmolStr heap (key
    /// uzunlukları, >23B = heap) toplamı. K1-K5 bellek refactor'ını önceliklendirmek için.
    /// Native'de mem_note no-op (hesap ucuz, O(trip+stop), tek seferlik).
    pub fn log_mem_report(&self) {
        const SS: usize = std::mem::size_of::<SmolStr>();
        const CST: usize = std::mem::size_of::<CompactStopTime>();
        const SET_U32: usize = std::mem::size_of::<FxHashSet<u32>>();
        let mb = |b: usize| b as f64 / 1_048_576.0;
        fn heap_bytes<'a>(it: impl Iterator<Item = &'a SmolStr>) -> usize {
            it.filter(|s| s.len() > 23).map(|s| s.len()).sum()
        }

        let n = self.rows.len();
        let nt = self.trip_ranges.len();
        let ns = self.stop_id_set.len();
        let nc = self.continuous_trips.len();
        let nu = self.unsorted_seq_trips.len();
        let ni = self.stop_intern.len(); // intern tablo boyutu

        let rows_cap = self.rows.capacity();
        let rows_used_mb = mb(n * CST);
        let rows_cap_mb = mb(rows_cap * CST);
        let tr_mb = mb(nt * (SS + 8));            // trip_ranges: SmolStr key + (u32,u32)
        let tid_mb = mb(nt * SS);                 // trip_id_set
        let sid_mb = mb(ns * SS);                 // stop_id_set
        let tfl_mb = mb(nt * (SS + 8));           // trip_first_line
        let sfl_mb = mb(ns * (SS + 8));           // stop_first_line
        let intern_mb = mb(ni * (SS + 4));        // stop_intern Vec + stop_id_to_idx SmolStr→u32
        let tss_outer_mb = mb(self.trip_stop_set.capacity() * (SS + SET_U32));
        let tss_inner_entries: usize = self.trip_stop_set.values().map(|s| s.capacity()).sum();
        let tss_inner_mb = mb(tss_inner_entries * 4); // u32 = 4 byte, heap yok
        let useq_mb = mb(self.unsorted_seq_trips.capacity()
            * std::mem::size_of::<(SmolStr, u32, u32, u64)>());

        let heap_tid_mb = mb(heap_bytes(self.trip_id_set.iter()));
        let heap_sid_mb = mb(heap_bytes(self.stop_id_set.iter()));
        let heap_intern_mb = mb(heap_bytes(self.stop_intern.iter()));

        crate::timing::mem_note(&format!(
            "stop_times index canli boyut (len x size_of, high-water DEGIL): \
             rows len {n} kullanilan {rows_used_mb:.1} MB / kapasite {rows_cap} ayrilan {rows_cap_mb:.1} MB | trip_ranges {nt} ~{tr_mb:.1} | \
             trip_id_set ~{tid_mb:.1} | stop_id_set {ns} ~{sid_mb:.1} | \
             stop_intern {ni} ~{intern_mb:.1} | \
             trip_stop_set dis ~{tss_outer_mb:.1} ic({tss_inner_entries}) ~{tss_inner_mb:.1} | \
             trip_first_line ~{tfl_mb:.1} | stop_first_line ~{sfl_mb:.1} | \
             continuous {nc} | unsorted_seq {nu} ~{useq_mb:.1} | \
             SmolStr heap: trip ~{heap_tid_mb:.1} stop ~{heap_sid_mb:.1} intern ~{heap_intern_mb:.1} MB"
        ));
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
            // stop intern
            let stop_idx = if st.stop_id.is_empty() {
                u32::MAX
            } else {
                *idx.stop_id_to_idx.entry(st.stop_id.clone()).or_insert_with(|| {
                    let i = idx.stop_intern.len() as u32;
                    idx.stop_intern.push(st.stop_id.clone());
                    i
                })
            };
            if stop_idx != u32::MAX {
                idx.stop_id_set.insert(st.stop_id.clone());
                idx.stop_first_line.entry(st.stop_id.clone()).or_insert(st.line);
                idx.trip_stop_set.entry(st.trip_id.clone()).or_default().insert(stop_idx);
            }
            // Enum kümesi {0,2,3}: 1 ve boş "sürekli biniş YOK" demektir (#170).
            // Bu set TRP_019'u besler; 1'i dahil etmek geçerli feed'den shape_id
            // talep ediyordu, 2/3'ü atlamak gerçek ihlali kaçırıyordu.
            if matches!(st.continuous_pickup, Some(0) | Some(2) | Some(3))
                || matches!(st.continuous_drop_off, Some(0) | Some(2) | Some(3))
            {
                idx.continuous_trips.insert(st.trip_id.clone());
            }
            // stop_headsign side map (line-anahtarlı, sort'tan bağımsız)
            if let Some(ref hs) = st.stop_headsign {
                idx.stop_headsigns.insert(st.line as u32, hs.clone());
            }
            // flex side map — streaming yolun (build_index) yaptığının aynısı; fixture'lardan
            // kurulan index'te de location_id/location_group_id görünür olsun diye.
            if let Some(flex) = build_flex(
                st.start_pickup_drop_off_window,
                st.end_pickup_drop_off_window,
                st.location_id.clone(),
                st.location_group_id.clone(),
                st.pickup_booking_rule_id.clone(),
                st.drop_off_booking_rule_id.clone(),
                st.pickup_type,
                st.drop_off_type,
            ) {
                idx.flex_map.insert(st.line as u32, flex);
            }
            by_trip.entry(st.trip_id.clone()).or_default().push(CompactStopTime {
                stop_idx,
                arrival_secs:   st.arrival_time.map(|(h,m,s)| h*3600+m*60+s).unwrap_or(u32::MAX),
                departure_secs: st.departure_time.map(|(h,m,s)| h*3600+m*60+s).unwrap_or(u32::MAX),
                sequence:       st.stop_sequence.unwrap_or(u32::MAX),
                dist_f32:       st.shape_dist_traveled.map(|d| d as f32).unwrap_or(f32::NAN),
                line:           st.line as u32,
            });
        }
        // Test yardımcısı: per-trip map → flat rows + trip_ranges (veri küçük, perf önemsiz).
        for (tid, mut stops) in by_trip {
            stops.sort_by_key(|st| st.sequence);
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
    // ⚠️ SÖZLÜKSEL GENİŞLİK — `common::gtfs_time_widths_ok` ile AYNI karar (issue #82).
    // İki yol ayrışırsa aynı feed akış modunda başka, tam modda başka sonuç verir.
    let parts: Vec<&str> = raw.split(':').collect();
    if !super::common::gtfs_time_widths_ok(&parts) {
        return Err(format!(
            "'{field}' için [H]H:MM:SS bekleniyor (dakika/saniye iki basamaklı), alınan: {raw}"));
    }
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
        // issue #82: referans da SÖZLÜKSEL genişliği denetler — testin işi hızlı
        // ayrıştırıcının referansla aynı kararı vermesi, ikisinin de ESKİ (gevşek)
        // mantığı sürdürmesi değil.
        let parts: Vec<&str> = raw.split(':').collect();
        if !crate::k2::common::gtfs_time_widths_ok(&parts) { return Err(()); }
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
/// Akış okuyucularının kayıt başına bildirdiği CSV bulguları (issue #84).
///
/// İki AYRI olgu, tek kanal DEĞİL: `rfc` kaçırma ihlalidir (`ARC_033`, veri sağlam),
/// `unclosed` ise tokenization hatasıdır (`ARC_013`, dosyanın kalanı yutulmuş olabilir).
#[derive(Default)]
pub(super) struct CsvScanState {
    pub rfc: Option<(&'static str, String)>,
    /// EOF'a tırnak İÇİNDE varıldı — kayıt hiç kapanmadı.
    pub unclosed: bool,
}

/// `rfc`: bu kayıtta görülen İLK RFC 4180 ihlali (tür + kısa örnek) buraya yazılır —
/// `ARC_033`. Çağıran satır numarasını bilir, bu fonksiyon bilmez; birikimi çağıran yapar.
///
/// 🔴 Neden K1'de değil burada: dört akış dosyasının GÖVDESİ K1'e hiç açılmıyor
/// (`is_zip_stream`), K1'in gövde tarayıcısına giden dal 2026-08-07'de ÖLÜ olduğu ölçüldü.
/// İhlal ancak baytların gerçekten okunduğu yerde görülebilir.
pub(super) fn next_csv_record<'a>(
    text: &'a str,
    pos: &mut usize,
    out: &mut Vec<Cow<'a, str>>,
    st: &mut CsvScanState,
) -> bool {
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
            let mut closed_quote = false;
            while *pos < n {
                let b = bytes[*pos];
                if b == b'"' {
                    *pos += 1;
                    if *pos < n && bytes[*pos] == b'"' {
                        buf.push('"');
                        *pos += 1;
                    } else {
                        closed_quote = true;
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
            // RFC 4180: kapanış tırnağından sonra ayraç gelmeli.
            if *pos < n && !matches!(bytes[*pos], b',' | b'\n' | b'\r') && st.rfc.is_none() {
                st.rfc = Some((RFC4180_AFTER_CLOSE, buf.chars().take(40).collect()));
            }
            // issue #84: tırnak hiç kapanmadan EOF'a varıldıysa kayıt EKSİKTİR. Eski kod
            // burada sessizce elindekini push ediyordu ("best-effort") ve dosyanın kalanı
            // görünmeden kayboluyordu.
            if *pos >= n && !closed_quote {
                st.unclosed = true;
            }
            // Kapanmamış tırnak: best-effort (zorunlu dosya CSV'si K1 header aşamasında
            // ve gövdede burada toleranslı işlenir; satır düşürülmez).
            out.push(Cow::Owned(buf));
        } else {
            let start = *pos;
            let mut bare_quote = false;
            while *pos < n {
                let b = bytes[*pos];
                if b == b',' || b == b'\n' || b == b'\r' {
                    break;
                }
                bare_quote |= b == b'"';
                *pos += 1;
            }
            if bare_quote && st.rfc.is_none() {
                st.rfc = Some((RFC4180_BARE_QUOTE, text[start..*pos].chars().take(40).collect()));
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
// #38: seen_seq (FxHashSet<u32>) ve stop_set (FxHashSet<SmolStr>) kaldırıldı:
//   seen_seq → STM_032 post-finalize'a taşındı (sorted rows'da adjacent aynı seq = tekrar)
//   stop_set<SmolStr> → stop_idx_set<u32> (intern indeksler, ~560MB tasarruf)
//   first_line: u64 → u32 (CSV satır numarası 28M < u32::MAX, güvenli)
//   last_seq: Option<u32> → u32 sentinel (u32::MAX = None, 4B tasarruf)
#[derive(Default)]
struct TripAgg {
    idx: u32,
    first_line: u32,
    last_seq: u32,              // u32::MAX = None; STM_036 sıralama takibi
    last_stop: SmolStr,         // boş = None; last_seq'i ayarlayan durak
    unsorted_fired: bool,
    continuous: bool,
    // stop_idx_set KALDIRILDI (#38 peak-2): 3M trip × ~80B heap → ~240 MB ekstra peak.
    // trip_stop_set finalize SONRASI sorted rows'dan inşa edilir (semantik fark yok).
}
impl TripAgg {
    fn new(first_line: u64, idx: u32) -> Self {
        Self { idx, first_line: first_line as u32, last_seq: u32::MAX, ..Default::default() }
    }
}

// ── ZIP pre-count: stop_times satır sayısını exact ölçer (2-pass) ────────────
// Pass-1: '\n' say (header dahil) → veri satırı = n - 1.
// Amaç: all_rows + row_trip için reserve_exact → sıfır realloc, sıfır artık kapasite.
// CPU bedeli: Swiss 2170 MB → ~10-20 s ekstra decompression. Memory deterministik.
fn count_zip_rows(zb: &[u8], file_name: &str) -> Option<usize> {
    use std::io::BufRead;
    let cursor = std::io::Cursor::new(zb);
    let mut archive = zip::ZipArchive::new(cursor).ok()?;
    let entry = archive.by_name(file_name).ok()?;
    let mut reader = std::io::BufReader::with_capacity(1 << 16, entry);
    let mut count = 0usize;
    let mut buf = Vec::with_capacity(256);
    loop {
        buf.clear();
        match reader.read_until(b'\n', &mut buf) {
            Ok(0) | Err(_) => break,
            Ok(_) => count += 1,
        }
    }
    // count = header + veri satırları (trailing boş satır varsa +1, güvenli; sadece kapasite)
    Some(count.saturating_sub(1))
}

/// Ham alan baytlarından kısa, güvenli bir örnek — notice mesajına girer.
fn lossy40(bytes: &[u8]) -> String {
    String::from_utf8_lossy(bytes).chars().take(40).collect()
}

// ── ZipCsvReader: ZIP entry'den incremental RFC 4180 CSV parse ───────────────
// #38: Büyük dosyalar (stop_times, trips, calendar_dates) K2 ZIP'ten satır satır okur.
// trips ve calendar_dates da aynı struct'ı kullanır (pub(crate)).
pub(crate) struct ZipCsvReader<R: Read> {
    inner: R,
    /// 64 KB okuma penceresi. `BufReader` yerine doğrudan tutulur: eskiden her bayt
    /// `BufReader::read(&mut [0u8; 1])` ile alınıyordu — stop_times.txt'in 402 MB'ı için
    /// ~402 milyon `read` çağrısı. Şimdi bayt başına yalnız indeks ilerlemesi var,
    /// gerçek okuma 64 KB'da bir yapılır.
    buf: Box<[u8]>,
    pos: usize,
    filled: usize,
    pending: Option<u8>,
    /// Bu kayıtta görülen İLK RFC 4180 ihlali (`ARC_033`). Çağıran `take()` ile alır ve
    /// satır numarasını kendisi ekler. Ürün yolu BURASIDIR: dört akış dosyası da
    /// gövdesini bu okuyucudan geçirir.
    pub(crate) rfc: Option<(&'static str, String)>,
    /// issue #84: EOF'a tırnak İÇİNDE varıldı → kayıt hiç kapanmadı (`ARC_013`).
    pub(crate) unclosed: bool,
}

impl<R: Read> ZipCsvReader<R> {
    pub(crate) fn new(r: R) -> Self {
        Self {
            inner: r,
            buf: vec![0u8; 65536].into_boxed_slice(),
            pos: 0,
            filled: 0,
            pending: None,
            rfc: None,
            unclosed: false,
        }
    }

    #[inline(always)]
    fn next_byte(&mut self) -> Option<u8> {
        if let Some(b) = self.pending.take() {
            return Some(b);
        }
        if self.pos == self.filled {
            // Kısa okuma normaldir (deflate akışı): 0 = EOF, hata = akış sonu.
            match self.inner.read(&mut self.buf) {
                Ok(0) | Err(_) => return None,
                Ok(n) => {
                    self.filled = n;
                    self.pos = 0;
                }
            }
        }
        let b = self.buf[self.pos];
        self.pos += 1;
        Some(b)
    }

    /// `out[fi]`'yi yazıma hazırlar: varsa içeriğini boşaltır (KAPASİTE KORUNUR),
    /// yoksa yeni tampon ekler.
    #[inline(always)]
    fn begin_field(out: &mut Vec<Vec<u8>>, fi: usize) {
        match out.get_mut(fi) {
            Some(f) => f.clear(),
            None => out.push(Vec::new()),
        }
    }

    /// Sonraki CSV kaydını `out`'a yazar (ham bayt alanları). EOF'ta false döner.
    ///
    /// Alan tamponları KORUNUR: `out` içindeki `Vec`'ler drop edilmez, `clear`
    /// edilip yeniden doldurulur. Önceki sürüm her alan için `Vec::new()` açıp
    /// `mem::take` ile kapasiteyi çöpe atıyordu; VBB'de (5,68M satır × ~10 alan)
    /// bu ~57M tahsis demekti. Alan içerikleri, alan sayısı ve sıra birebir aynı.
    pub(crate) fn next_record(&mut self, out: &mut Vec<Vec<u8>>) -> bool {
        let mut fi: usize = 0;
        Self::begin_field(out, fi);
        let mut in_quotes = false;
        let mut started = false;
        // Alanı kapat ve sonrakini hazırla.
        macro_rules! commit {
            () => {{
                fi += 1;
                Self::begin_field(out, fi);
            }};
        }
        // Kaydı bitir: kullanılan alan sayısı fi + 1 (son alan henüz kapatılmadı).
        macro_rules! finish {
            () => {{
                fi += 1;
                out.truncate(fi);
                return true;
            }};
        }
        loop {
            let b = match self.next_byte() {
                Some(b) => b,
                None => {
                    // issue #84: girdi kapanış tırnağını görmeden TIRNAK İÇİNDE bitti →
                    // kayıt hiç kapanmadı ve dosyanın kalanı bu alana yutulmuş olabilir.
                    if in_quotes {
                        self.unclosed = true;
                    }
                    if started { finish!(); }
                    out.clear();
                    return false;
                }
            };
            started = true;
            if in_quotes {
                if b == b'"' {
                    match self.next_byte() {
                        Some(b'"') => out[fi].push(b'"'),
                        Some(b',') => { in_quotes = false; commit!(); }
                        Some(b'\n') => { finish!(); }
                        Some(b'\r') => {
                            if let Some(lf) = self.next_byte() {
                                if lf != b'\n' { self.pending = Some(lf); }
                            }
                            finish!();
                        }
                        Some(other) => {
                            // RFC 4180: kapanış tırnağından sonra ayraç gelmeli. Gelmiyorsa
                            // alan içindeki tırnak kaçırılmamıştır → ARC_033.
                            if self.rfc.is_none() {
                                self.rfc = Some((RFC4180_AFTER_CLOSE, lossy40(&out[fi])));
                            }
                            in_quotes = false;
                            self.pending = Some(other);
                        }
                        None => {
                            // Bu tırnak zaten kapanış tırnağıydı. Hemen ardından EOF
                            // gelmesi geçerli CSV'dir; son kaydın newline ile bitmesi
                            // zorunlu değildir. Kapanış tırnağından ÖNCE EOF ise
                            // yukarıdaki `in_quotes` yolu ARC_013 üretir.
                            finish!();
                        }
                    }
                } else {
                    out[fi].push(b); // embedded \n, vb. dahil
                }
            } else {
                match b {
                    b'"' if out[fi].is_empty() => { in_quotes = true; }
                    b'"' => {
                        // 🔴 Alan ZATEN BAŞLAMIŞKEN gelen tırnak (`12" Street`) DÜZ
                        // KARAKTERDİR — tırnak başlangıcı değil. Eski kod burada tırnak
                        // moduna giriyordu ve dosyanın KALANINI o alana yutuyordu; issue
                        // #84'ün testi bunu ortaya çıkardı: çıplak tırnak `ARC_013`
                        // (kapanmamış tırnak) üretiyordu, oysa tokenization hatası YOK.
                        // `tokenize_csv` en başından beri düz karakter sayıyordu; iki
                        // okuyucu artık AYNI kararı veriyor (kabul ölçütü: akış/akış-dışı
                        // eşdeğer yardımcılar aynı sonucu vermeli).
                        if self.rfc.is_none() {
                            self.rfc = Some((RFC4180_BARE_QUOTE, lossy40(&out[fi])));
                        }
                        out[fi].push(b);
                    }
                    b',' => { commit!(); }
                    b'\n' => { finish!(); }
                    b'\r' => {
                        if let Some(lf) = self.next_byte() {
                            if lf != b'\n' { self.pending = Some(lf); }
                        }
                        finish!();
                    }
                    _ => out[fi].push(b),
                }
            }
        }
    }
}

/// `validate_stop_times`in satır döngüsünde biriken TÜM değişken durum.
///
/// Chunk-paralel hazırlığı (Adım 1): bugün TEK örnekle çalışır, davranış birebir
/// aynıdır. Durumun tek yerde toplanması, ileride her chunk'ın kendi örneğini
/// doldurup birleştirmesini mümkün kılar. Alanların merge stratejileri:
/// - `total_rows`, `stm050_empty`      → toplama
/// - `stop_first_line`                 → min-merge
/// - `arc021_fired`                    → "dosya başına bir kez": en küçük satırlı kazanır
/// - `notices`/`counter`               → satır sırasında birleştir + renumber (K6 deseni)
/// - `all_rows`/`row_trip`             → chunk sırasında concat
/// - `stop_id_to_idx`/`stop_intern`    → global tablo + `CompactStopTime.stop_idx` REMAP
/// - `trips_agg`                       → `idx` remap; `last_seq`/`last_stop` chunk SINIRINDA
///   ayrıca karşılaştırılmalı (STM_036)
#[derive(Default)]
struct StChunk {
    /// ARC_033 birikimi — chunk'lar birleşirken `merge` ile toplanır.
    rfc: crate::k1_parse::Rfc4180Acc,
    /// issue #84: bu parçada kapanmamış tırnak görüldü mü (`ARC_013`).
    unclosed: bool,
    notices: Vec<Notice>,
    /// STM_018/019: yüksek hacimde satır-başına notice yerine döngü sonunda
    /// deterministik karar vermek için geçici bulgular.
    stm018_pending: Vec<Notice>,
    stm019_pending: Vec<Notice>,
    counter: u32,
    /// `STM_059` kapısı — feed'de booking_rules.txt var mı.
    has_booking_rules: bool,
    /// Intern cache: unique long (>22 byte) trip_id başına bir Arc alloc
    trip_id_cache: FxHashMap<String, SmolStr>,
    /// #38: stop_id intern tablosu (SmolStr yerine u32 indeks → CompactStopTime 4B stop alanı)
    stop_intern: Vec<SmolStr>,
    stop_id_to_idx: FxHashMap<SmolStr, u32>,
    /// #38: side map'ler — CSV line anahtarlı (sort/permutation'dan bağımsız)
    stop_headsigns: FxHashMap<u32, SmolStr>,
    flex_map: FxHashMap<u32, Box<StopTimeFlex>>,
    /// OOM/perf: trip_id-anahtarlı TÜM per-trip durum tek map'te → satır başı ~8 hashmap op
    /// yerine 1-2. Çıktı setleri finalize pass'le buradan TÜRETİLİR.
    trips_agg: FxHashMap<SmolStr, TripAgg>,
    /// stop_id_set bundan türetilir
    stop_first_line: FxHashMap<SmolStr, u64>,
    total_rows: usize,
    unsorted_seq_trips: Vec<(SmolStr, u32, u32, u64)>,
    arc021_fired: bool,
    arc030_fired: bool,
    /// STM_050: boş-timepoint satır sayısı. Satır-başına notice DEĞİL (büyük feed'lerde
    /// milyonlarca notice → tarayıcı OOM); döngü sonunda TEK feed-seviyesi özet emit edilir.
    stm050_empty: u32,
    dq016: crate::k1_parse::Dq016Acc,
    /// OOM/perf: per-trip Vec YOK. Tüm satırlar (trip_idx etiketli) tek flat buffer'da
    /// dosya sırasında toplanır; finalize'da counting-placement ile trip'e göre gruplanır.
    all_rows: Vec<CompactStopTime>,
    row_trip: Vec<u32>,
}

/// Gövdeyi `n` parçaya böler. Her sınır bir SATIR başına ve TRIP DEĞİŞİMİNE hizalanır:
/// stop_times trip'e göre gruplu olduğundan aynı trip iki parçaya düşmez, böylece STM_036'nın
/// sıra durumu (last_seq/last_stop) parça sınırında kopmaz.
/// Döner: (bayt_başı, bayt_sonu, ilk_satır_numarası). Başlık satırı atlanmıştır.
#[cfg(feature = "parallel")]
fn split_body_at_trip_boundaries(body: &[u8], trip_col: Option<usize>, n: usize) -> Vec<(usize, usize, u64)> {
    // Satırın `trip_col`'uncu alanını kaba okur (tırnak yok varsayımı; trip_id'de nadirdir).
    // Okunamazsa None döner ve sınır bir satır ileri kayar — muhafazakâr taraf.
    fn trip_of(line: &[u8], col: usize) -> Option<&[u8]> {
        line.split(|&b| b == b',').nth(col)
    }
    let Some(tcol) = trip_col else { return Vec::new() };
    let header_end = match body.iter().position(|&b| b == b'\n') { Some(i) => i + 1, None => return Vec::new() };
    if n <= 1 || body.len() - header_end < 4 * 1024 * 1024 {
        return vec![(header_end, body.len(), 2)];
    }

    // Satır başlangıçları + numaraları tek geçişte.
    let mut starts: Vec<usize> = Vec::new();
    starts.push(header_end);
    for (i, &b) in body.iter().enumerate().skip(header_end) {
        if b == b'\n' && i + 1 < body.len() { starts.push(i + 1); }
    }
    if starts.len() < n { return vec![(header_end, body.len(), 2)]; }

    let per = starts.len() / n;
    let mut out: Vec<(usize, usize, u64)> = Vec::with_capacity(n);
    let mut cut_idx = 0usize;
    while cut_idx < starts.len() {
        let mut next = (cut_idx + per).min(starts.len());
        // Sınırı trip değişimine hizala.
        if next < starts.len() {
            let line_of = |i: usize| -> &[u8] {
                let s = starts[i];
                let e = if i + 1 < starts.len() { starts[i + 1] } else { body.len() };
                &body[s..e]
            };
            let prev_trip = trip_of(line_of(next.saturating_sub(1)), tcol).map(|t| t.to_vec());
            while next < starts.len() {
                let cur = trip_of(line_of(next), tcol).map(|t| t.to_vec());
                if cur != prev_trip { break; }
                next += 1;
            }
        }
        let byte_start = starts[cut_idx];
        let byte_end = if next < starts.len() { starts[next] } else { body.len() };
        out.push((byte_start, byte_end, cut_idx as u64 + 2));
        if next >= starts.len() { break; }
        cut_idx = next;
    }
    out
}

impl StChunk {
    /// Dosyada DAHA SONRA gelen `other` parçasını bu parçaya katar.
    ///
    /// İki yeniden eşleme (remap) zorunlu: `stop_id` ve trip indeksleri parça-yerel
    /// üretilir, ama `CompactStopTime.stop_idx` ve `row_trip` bu sayıları GÖVDEDE taşır.
    /// Her ikisi de `other`ın tablosundan bu parçanın tablosuna çevrilir.
    ///
    /// Chunk sınırı TRIP DEĞİŞİMİNDE kurulduğu için normal feed'de aynı trip iki parçaya
    /// düşmez. Düşerse (aynı trip'in satırları dosyada dağınıksa) `TripAgg` alanları
    /// birleştirilir; bu durumda dosya sırası zaten bozuktur ve STM_036 onu ayrıca raporlar.
    fn merge(&mut self, other: StChunk) {
        // ARC_033: satır numarası min-merge, sayım toplanır (`Rfc4180Acc::merge`).
        self.rfc.merge(&other.rfc);
        self.unclosed |= other.unclosed;
        // ── stop_id intern: yerel indeks → global indeks ────────────────────────
        let mut stop_remap: Vec<u32> = Vec::with_capacity(other.stop_intern.len());
        for sid in other.stop_intern {
            let next = self.stop_intern.len() as u32;
            let gidx = *self.stop_id_to_idx.entry(sid.clone()).or_insert_with(|| {
                self.stop_intern.push(sid);
                next
            });
            stop_remap.push(gidx);
        }

        // ── trip indeksleri: yerel → global (TripAgg.idx üzerinden) ────────────
        let mut trip_remap: Vec<u32> = vec![u32::MAX; other.trips_agg.len()];
        // Yerel idx sırasında dolaş: `idx` atama sırası = ilk görülme sırası.
        let mut others: Vec<(SmolStr, TripAgg)> = other.trips_agg.into_iter().collect();
        others.sort_unstable_by_key(|(_, a)| a.idx);
        for (tid, agg) in others {
            let local = agg.idx as usize;
            let next = self.trips_agg.len() as u32;
            match self.trips_agg.get_mut(&tid) {
                Some(cur) => {
                    // Aynı trip iki parçada: `other` sonra geldiği için son durum onunkidir.
                    if agg.last_seq != u32::MAX {
                        cur.last_seq = agg.last_seq;
                        cur.last_stop = agg.last_stop;
                    }
                    cur.unsorted_fired |= agg.unsorted_fired;
                    cur.continuous |= agg.continuous;
                    cur.first_line = cur.first_line.min(agg.first_line);
                    if local < trip_remap.len() { trip_remap[local] = cur.idx; }
                }
                None => {
                    let mut moved = agg;
                    moved.idx = next;
                    self.trips_agg.insert(tid, moved);
                    if local < trip_remap.len() { trip_remap[local] = next; }
                }
            }
        }

        // ── gövdeyi taşı, indeksleri çevir ─────────────────────────────────────
        self.all_rows.reserve(other.all_rows.len());
        for mut row in other.all_rows {
            if row.stop_idx != u32::MAX {
                row.stop_idx = stop_remap[row.stop_idx as usize];
            }
            self.all_rows.push(row);
        }
        self.row_trip.reserve(other.row_trip.len());
        for t in other.row_trip {
            self.row_trip.push(if (t as usize) < trip_remap.len() { trip_remap[t as usize] } else { t });
        }

        // ── toplanabilir / birleşebilir durum ──────────────────────────────────
        // arc021: "dosya başına BİR kez". Bu parça zaten ateşlediyse `other`ınkiler düşer —
        // filtre extend'den ÖNCE, yoksa kendi notice'ımızı da silerdik.
        let mut incoming = other.notices;
        if self.arc021_fired {
            incoming.retain(|n| n.rule_id != "ARC_021");
        }
        if self.arc030_fired {
            incoming.retain(|n| n.rule_id != "ARC_030");
        }
        self.notices.extend(incoming);
        self.stm018_pending.extend(other.stm018_pending);
        self.stm019_pending.extend(other.stm019_pending);
        self.arc021_fired |= other.arc021_fired;
        self.arc030_fired |= other.arc030_fired;
        self.counter += other.counter;
        self.trip_id_cache.extend(other.trip_id_cache);
        self.stop_headsigns.extend(other.stop_headsigns); // anahtar = CSV satırı, çakışmaz
        self.flex_map.extend(other.flex_map);
        for (sid, line) in other.stop_first_line {
            self.stop_first_line.entry(sid).and_modify(|l| *l = (*l).min(line)).or_insert(line);
        }
        self.total_rows += other.total_rows;
        self.unsorted_seq_trips.extend(other.unsorted_seq_trips);
        self.stm050_empty += other.stm050_empty;
        self.dq016.merge(other.dq016);
    }
}

/// `has_booking_rules`: feed `booking_rules.txt` taşıyor mu — `STM_059` bu kapıya bağlı.
/// O dosya yokken bir booking rule id yazmak FK'yı kırar (`BKR_017`), dolayısıyla tavsiye
/// uygulanamaz ve kural susmalıdır.
pub fn validate_stop_times(
    file: &RawFile,
    zip_bytes: Option<&[u8]>,
    has_booking_rules: bool,
) -> (StopTimesIndex, Vec<gtfs_core::Notice>) {
    let mut st = StChunk { has_booking_rules, ..StChunk::default() };
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
    // Intern cache: unique long (>22 byte) trip_id başına bir Arc alloc
    // #38: stop_id intern tablosu (SmolStr yerine u32 indeks → CompactStopTime 4B stop alanı)
    // #38: side map'ler — CSV line anahtarlı (sort/permutation'dan bağımsız)
    // OOM/perf: trip_id-anahtarlı TÜM per-trip durum tek map'te (TripAgg) → satır başı ~8 hashmap op
    // yerine 1-2. Çıktı setleri (trip_id_set/trip_first_line/trip_stop_set/continuous_trips/trips)
    // finalize pass'le buradan TÜRETİLİR → StopTimesIndex şekli ve DAVRANIŞ değişmez.
    // STM_050: boş-timepoint satır sayısı. Satır-başına notice DEĞİL (büyük feed'lerde milyonlarca
    // notice → tarayıcı OOM, bkz. Kocaeli 2.4M); döngü sonunda TEK feed-seviyesi özet emit edilir.
    // OOM/perf (Aşama 1b): per-trip Vec YOK. Tüm satırlar (trip_idx etiketli) tek flat buffer'da
    // dosya sırasında toplanır; finalize'da counting-placement ile trip'e göre gruplanır.
    // #15 W1: tag'siz CompactStopTime + paralel row_trip → finalize'da in-place permütasyon
    // (ikinci tam Vec<CompactStopTime> ASLA ayrılmaz).
    // Düz tamponun log₂(N) realloc/memcpy'sini önle: satır sayısını ham metin
    // uzunluğundan tahmin et (stop_times satırı tipik ≥~40 bayt → muhafazakâr
    // alt-sınır bölen). Yalnızca KAPASITE; eleman değeri/sırası etkilenmez.
    if let Some(text) = &file.raw_text {
        // #15: eski text.len()/40 40B/satir varsayiyordu; VBB gercek ~71B/satir → ~1.78x
        // FAZLA kapasite (~454 MB bos alan). Newline sayisi ~kesin satir sayisi verir
        // (header + olasi gomulu-newline = kucuk UST tahmin, guvenli; realloc yok).
        // shrink_to_fit YAPILMAZ (1GB canliyken yeni buffer = transient peak/OOM).
        let est = text.bytes().filter(|&b| b == b'\n').count();
        st.all_rows.reserve(est);
        st.row_trip.reserve(est);
    } else if let Some(zb) = zip_bytes {
        // #38 (ZIP stream 2-pass): Pass-1 ile exact satır sayısı → reserve_exact → sıfır realloc,
        // sıfır artık kapasite. shrink_to YOK (allocator kumarı). Fallback: bytes/60 capped 50M.
        crate::timing::mem_log("K2 stop_times pre-count pass");
        let est = count_zip_rows(zb, &file.name)
            .unwrap_or_else(|| (file.bytes as usize / 60).clamp(1, 50_000_000));
        crate::timing::mem_log("K2 stop_times pre-count done");
        st.all_rows.reserve_exact(est);
        st.row_trip.reserve_exact(est);
    }

    {
        // Satır işleyici — hem stream (raw_text) hem rows yolundan çağrılır.
        // Tüm değişken durum closure ile yakalanır (notices, counter, index, caches).
        let process = |st: &mut StChunk, row: &[Cow<'_, str>], line: u64| {
            // ── Taşınan dosya/satır-seviye notice'lar (K1'den) — eksik veri YOK ──
            // ARC_012: sütun sayısı tutarsız
            if let Some((msg, tip, observed, is_info)) =
                crate::k1_parse::arc012_check(&file.name, line, row.len(), header_count)
            {
                let mut n = make_k2_notice(
                    &mut st.counter, "ARC_012", EntityType::File, Some(file.name.clone()),
                    None, &file.name, Some(line), None,
                    Some(observed), None,
                    msg, tip,
                );
                if is_info {
                    n.severity = Severity::Bilgi;
                }
                st.notices.push(n);
            }

            // ARC_034: başlık tekrarı → notice + indexleme YAPMA. K1 bu dosyayı stream
            // eder ve yalnız başlığı okur; kontrol orada kalırsa burada hiç çalışmaz (#181).
            if crate::k1_parse::arc034_is_header_repeat(row, &file.headers) {
                st.notices.push(make_k2_notice(
                    &mut st.counter, "ARC_034", EntityType::File, Some(file.name.clone()),
                    None, &file.name, Some(line), None, None, None,
                    format!("'{}' {line}. satırı başlık satırının tekrarı — veri olarak okunuyor.", file.name),
                    "Dosyanın ortasındaki tekrar eden başlık satırını kaldırın; genellikle iki dosyanın uç uca eklenmesinden doğar.",
                ));
                return;
            }

            // ARC_018: tamamen boş satır → notice + indexleme YAPMA (K1'deki continue)
            if row.iter().all(|v| v.trim().is_empty()) {
                st.notices.push(make_k2_notice(
                    &mut st.counter, "ARC_018", EntityType::File, Some(file.name.clone()),
                    None, &file.name, Some(line), None,
                    None, None,
                    format!("'{}' {line}. satırı tamamen boş.", file.name),
                    "Boş satırları kaldırın.",
                ));
                return;
            }

            // ARC_021: yazdırılamaz/sorunlu karakter — dosya başına bir kez
            if !st.arc021_fired {
                if let Some(cp) = crate::k1_parse::arc021_bad_char(row.iter().map(|v| v.as_ref())) {
                    st.arc021_fired = true;
                    st.notices.push(make_k2_notice(
                        &mut st.counter, "ARC_021", EntityType::File, Some(file.name.clone()),
                        None, &file.name, Some(line), None,
                        Some(format!("U+{cp:04X}")), None,
                        crate::k1_parse::arc021_message(&file.name, cp),
                        crate::k1_parse::ARC021_REMEDIATION,
                    ));
                }
            }

            // ARC_030: alan değerinde sekme/satır sonu — dosya başına bir kez
            if !st.arc030_fired {
                if let Some(cp) = crate::k1_parse::arc030_bad_whitespace(row.iter().map(|v| v.as_ref())) {
                    st.arc030_fired = true;
                    st.notices.push(make_k2_notice(
                        &mut st.counter, "ARC_030", EntityType::File, Some(file.name.clone()),
                        None, &file.name, Some(line), None,
                        Some(format!("U+{cp:04X}")), None,
                        crate::k1_parse::arc030_message(&file.name, cp),
                        crate::k1_parse::ARC030_REMEDIATION,
                    ));
                }
            }

            // DQ_016: değerlerde baştaki/sondaki boşluk
            // DQ_016: dosya-seviyesi birikim; emit döngü sonrası TEK özet (patlama önlemi).
            st.dq016.observe(line, row.iter().map(|v| v.as_ref()), &file.headers);

            // ── Mevcut STM_* doğrulamaları + index ──
            let trip_id_raw = get_col_raw(row, cols.trip_id);
        let trip_id = intern_smolstr(trip_id_raw, &mut st.trip_id_cache);
        // entity_id is only materialized when a notice is actually pushed
        let eid = || (!trip_id.is_empty()).then(|| trip_id.to_string());

        // STM_046: trip_id required (sütun yoksa ARC_025 devralır → atla)
        if trip_id.is_empty() && file.headers.iter().any(|h| h == "trip_id") {
            st.notices.push(make_k2_notice(
                &mut st.counter, "STM_046", EntityType::Trip, None,
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
                    st.notices.push(make_k2_notice(
                        &mut st.counter, "STM_005", EntityType::Trip, eid(),
                        None, &file.name, Some(line), Some("stop_sequence"),
                        Some(String::new()), None,
                        "stop_sequence zorunludur.".to_string(),
                        "stop_sequence negatif olmayan bir tam sayı olarak girin.",
                    ));
                }
                v
            }
            Err(err) => {
                st.notices.push(make_k2_notice(
                    &mut st.counter, "STM_005", EntityType::Trip, eid(),
                    None, &file.name, Some(line), Some("stop_sequence"),
                    Some(seq_raw.to_string()), None, err,
                    "stop_sequence negatif olmayan bir tam sayı olarak girin.",
                ));
                None
            }
        };

        // STM_036: per-trip sıralama takibi. STM_032 (duplicate seq) post-finalize'a taşındı.
        if let Some(seq) = stop_sequence {
            let cur_stop = get_col_raw(row, cols.stop_id);
            let next_idx = st.trips_agg.len() as u32;
            let agg = st.trips_agg.entry(trip_id.clone()).or_insert_with(|| TripAgg::new(line, next_idx));
            // STM_036 (a) alt-vakası: dosya satır sırası stop_sequence sırasıyla uyuşmuyor.
            // Trip başına TEK atış — MD unsorted_stop_times paritesi buna dayanır.
            // (b) alt-vakası (dağınık satırlar) K6'da, finalize edilmiş index üzerinden.
            if !agg.unsorted_fired {
                if agg.last_seq != u32::MAX {
                    let last = agg.last_seq;
                    // ⚠️ `<=`, `<` DEĞİL. Spec: *"The values must increase along the trip"* —
                    // EŞİT değer de artmıyordur. 2026-08-06 korpus koşumunda ölçüldü
                    // (mdb-1229): 24 seferin TÜM satırlarında `stop_sequence=0` yazıyor;
                    // sıra ne azalıyor ne satırlar dağınık, hep EŞİT. Eski `seq < last`
                    // koşulu iki dala da girmiyordu → STM_036 sessizdi, MD ise
                    // `unsorted_stop_times` = 24 diyordu.
                    // ⚠️ `STM_032` (yinelenen stop_sequence) ile ÇAKIŞMAZ: o SATIR başına
                    // "bu değer tekrar ediyor" der (MD `duplicate_key` ile birebir, 170.885),
                    // bu ise SEFER başına "satırlar sıralı değil" der. MD de ikisini ayrı
                    // notice olarak verir.
                    if seq <= last {
                        // Daha küçük stop_sequence'li satır, daha büyük sequence'li satırdan SONRA
                        // yazılmış → dosya artan sırada değil. Mesaj durak-odaklı: hangi durak
                        // hangi duraktan sonra yazılmış. prev_stop = max'ı ayarlayan satırın durağı.
                        let prev_stop = agg.last_stop.clone();
                        let mut n = make_k2_notice(
                            &mut st.counter, "STM_036", EntityType::Trip, eid(),
                            None, &file.name, Some(line), Some("stop_sequence"),
                            // observed/expected durak bilgisini de taşır: STM_036'nın locale
                            // şablonu (b) alt-vakasıyla ortak, bu yüzden şablona durak
                            // placeholder'ı EKLENEMEZ — bilgi observed/expected içinde gider.
                            Some(format!("{seq} @ '{cur_stop}'")),
                            Some(format!("> {last} @ '{prev_stop}'")),
                            if seq == last {
                                format!(
                                    "'{}' seferinde stop_times satır sırası artmıyor: '{}' durağı da bir önceki '{}' durağıyla AYNI stop_sequence ({seq}) değerini taşıyor. Değerler sefer boyunca artmalıdır.",
                                    trip_id, cur_stop, prev_stop
                                )
                            } else {
                                format!(
                                    "'{}' seferinde stop_times satır sırası bozuk: stop_sequence {seq} olan '{}' durağı, daha büyük stop_sequence {last} olan '{}' durağından sonra yazılmış. Satırlar artan stop_sequence sırasında olmalı.",
                                    trip_id, cur_stop, prev_stop
                                )
                            },
                            "stop_times.txt satırlarını trip_id ve stop_sequence değerine göre artan sırada düzenleyin.",
                        );
                        let mut d = std::collections::BTreeMap::new();
                        d.insert("curr_stop".to_string(), cur_stop.to_string());
                        d.insert("prev_stop".to_string(), prev_stop.to_string());
                        d.insert("curr_seq".to_string(), seq.to_string());
                        d.insert("prev_seq".to_string(), last.to_string());
                        n.details = Some(d);
                        st.notices.push(n);
                        agg.unsorted_fired = true;
                        st.unsorted_seq_trips.push((trip_id.clone(), last, seq, line));
                    } else if seq > last {
                        agg.last_seq = seq;
                        agg.last_stop = SmolStr::from(cur_stop);
                    }
                } else {
                    agg.last_seq = seq;
                    agg.last_stop = SmolStr::from(cur_stop);
                }
            }
        } // if let Some(seq)

        // STM_006: stop_id KOŞULLU zorunlu (sütun yoksa ARC_025 devralır → atla).
        // Spec: "Required if stop_times.location_group_id AND stop_times.location_id are NOT
        // defined. Forbidden if ... are defined." Yani bir Flex satırı stop_id yerine
        // location_id veya location_group_id taşır ve bu GEÇERLİDİR. Guard'sız hâlde her
        // Flex satırı KRİTİK STM_006 alıyordu.
        let raw_stop = get_col_raw(row, cols.stop_id);
        let has_flex_location = !get_col_raw(row, cols.location_id).is_empty()
            || !get_col_raw(row, cols.location_group_id).is_empty();
        if raw_stop.is_empty() && !has_flex_location && file.headers.iter().any(|h| h == "stop_id") {
            st.notices.push(make_k2_notice(
                &mut st.counter, "STM_006", EntityType::Stop, eid(),
                None, &file.name, Some(line), Some("stop_id"),
                Some(String::new()), None,
                "stop_id zorunludur.".to_string(),
                "stop_id alanını doldurun.",
            ));
        }
        // stop intern: raw_stop → u32 indeks (CompactStopTime.stop_idx)
        let stop_idx: u32 = if raw_stop.is_empty() {
            u32::MAX
        } else if let Some(&i) = st.stop_id_to_idx.get(raw_stop) {
            i
        } else {
            let i = st.stop_intern.len() as u32;
            let ss = SmolStr::new(raw_stop);
            st.stop_id_to_idx.insert(ss.clone(), i);
            st.stop_intern.push(ss);
            i
        };

        // arrival_time
        let arr_raw = get_col(row, cols.arrival_time);
        // `arr_malformed`: alan DOLU ama ayrıştırılamadı. `None` ile aynı kovaya
        // atılırsa "değer yok" iddiasında bulunan kurallar (STM_015/016) dolu bir
        // alan hakkında konuşur — bkz. TIME_MALFORMED.
        let mut arr_malformed = false;
        let arrival_time = match parse_gtfs_time_raw(arr_raw, "arrival_time") {
            Ok(v) => v,
            Err(err) => {
                st.notices.push(make_k2_notice(
                    &mut st.counter, "STM_003", EntityType::Trip, eid(),
                    None, &file.name, Some(line), Some("arrival_time"),
                    Some(arr_raw.to_string()), Some("HH:MM:SS".to_string()), err,
                    "HH:MM:SS formatında arrival_time girin.",
                ));
                arr_malformed = true;
                None
            }
        };

        // departure_time
        let dep_raw = get_col(row, cols.departure_time);
        let mut dep_malformed = false;
        let departure_time = match parse_gtfs_time_raw(dep_raw, "departure_time") {
            Ok(v) => v,
            Err(err) => {
                st.notices.push(make_k2_notice(
                    &mut st.counter, "STM_004", EntityType::Trip, eid(),
                    None, &file.name, Some(line), Some("departure_time"),
                    Some(dep_raw.to_string()), Some("HH:MM:SS".to_string()), err,
                    "HH:MM:SS formatında departure_time girin.",
                ));
                dep_malformed = true;
                None
            }
        };

        // STM_007 (departure_time >= arrival_time) K6'ya taşındı: orada servis-günü
        // normalize'li veri + hat/yön/servis bağlamı mevcut, gece-yarısı (00:xx kalkış)
        // yanlış-pozitifleri elenir. Bkz. k6_analytics.rs.

        // STM_034: varış veya kalkış zamanından yalnızca biri tanımlı
        //
        // 🔴 `None` İKİ ŞEY DEMEK: alan boş, ya da alan dolu ama ayrıştırılamadı. Bu kural
        // "yalnız biri TANIMLI" der, yani DOLULUK hakkında konuşur — bozuk değeri "yok"
        // sayarsa dolu bir alan için asimetri uydurur ve aynı satırda STM_003/STM_004
        // zaten doğrusunu söylerken ikinci bir yanlış bulgu üretir. STM_015/016'nın
        // mdb-2727'de 101.621 kez yaptığı hatanın aynısı (44206d0a).
        match (arrival_time.or(arr_malformed.then_some((0,0,0))),
               departure_time.or(dep_malformed.then_some((0,0,0)))) {
            (Some(_), None) => {
                st.notices.push(make_k2_notice(
                    &mut st.counter, "STM_034", EntityType::Trip, eid(),
                    None, &file.name, Some(line), Some("departure_time"),
                    None, Some("dolu".to_string()),
                    format!("trip_id '{}' satırında arrival_time tanımlı ama departure_time eksik.", trip_id),
                    "Her iki zaman alanını birlikte doldurun veya ikisini de boş bırakın.",
                ));
            }
            (None, Some(_)) => {
                st.notices.push(make_k2_notice(
                    &mut st.counter, "STM_034", EntityType::Trip, eid(),
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
            get_col(row, cols.pickup_type), &mut st.notices, &mut st.counter,
            "STM_009", "pickup_type", trip_id.as_str(), line, &file.name,
        );

        // STM_010: drop_off_type 0-3
        let drop_off_type = parse_pickup_dropoff_col(
            get_col(row, cols.drop_off_type), &mut st.notices, &mut st.counter,
            "STM_010", "drop_off_type", trip_id.as_str(), line, &file.name,
        );

        // STM_018: continuous_pickup 0-3
        let continuous_pickup = parse_pickup_dropoff_col(
            get_col(row, cols.continuous_pickup), &mut st.stm018_pending, &mut st.counter,
            "STM_018", "continuous_pickup", trip_id.as_str(), line, &file.name,
        );

        // STM_019: continuous_drop_off 0-3
        let continuous_drop_off = parse_pickup_dropoff_col(
            get_col(row, cols.continuous_drop_off), &mut st.stm019_pending, &mut st.counter,
            "STM_019", "continuous_drop_off", trip_id.as_str(), line, &file.name,
        );

        // STM_022: timepoint 0 or 1
        let tp_raw = get_col(row, cols.timepoint);
        // STM_050: timepoint sütunu feed'de mevcut ama bu satırda boş (MD missing_timepoint_value).
        // Boş timepoint örtük 1 (kesin) sayılır → yaklaşık zamanlar yanlış kesin görünür; açık değer önerilir.
        // Satır-başına DEĞİL sayılır; döngü sonunda tek feed-seviyesi özet (büyük feed OOM önlemi).
        if cols.timepoint.is_some() && tp_raw.trim().is_empty() {
            st.stm050_empty += 1;
        }
        let timepoint = match parse_u32_raw(tp_raw, "timepoint") {
            Ok(v) => {
                if let Some(val) = v {
                    if val > 1 {
                        st.notices.push(make_k2_notice(
                            &mut st.counter, "STM_022", EntityType::Trip, eid(),
                            None, &file.name, Some(line), Some("timepoint"),
                            Some(val.to_string()), Some("0 veya 1".to_string()),
                            "timepoint 0 veya 1 olmalıdır.".to_string(),
                            "timepoint değerini 0 (yaklaşık) veya 1 (kesin) olarak ayarlayın.",
                        ));
                    }
                }
                v
            }
            Err(err) => {
                // Sayı OLMAYAN değer eskiden sessizce düşüyordu: aralık dışı sayı STM_022 üretirken
                // "abc" hiçbir bulgu vermiyordu. Aynı olgunun iki dalı → aynı kural (PTH_027 emsali).
                st.notices.push(make_k2_notice(
                    &mut st.counter, "STM_022", EntityType::Trip, eid(),
                    None, &file.name, Some(line), Some("timepoint"),
                    Some(tp_raw.to_string()), Some("0 veya 1".to_string()),
                    err,
                    "timepoint değerini 0 (yaklaşık) veya 1 (kesin) olarak ayarlayın.",
                ));
                None
            }
        };

        // STM_047: timepoint=1 (kesin zaman noktası) iken hem arrival_time hem departure_time
        // eksik. Yalnızca biri dolu olduğunda STM_034 asimetriyi yakalar → örtüşme yok.
        // Aynı ayrım: "tanımlı değil" iddiası ancak alan GERÇEKTEN boşken doğrudur.
        // Bozuk yazılmış bir zaman timepoint=1 için eksik değil, hatalıdır — onu STM_003
        // ve STM_004 bildirir.
        if timepoint == Some(1)
            && arrival_time.is_none() && !arr_malformed
            && departure_time.is_none() && !dep_malformed
        {
            st.notices.push(make_k2_notice(
                &mut st.counter, "STM_047", EntityType::Trip, eid(),
                None, &file.name, Some(line),
                // Hüküm İKİ alanı da kapsar: spec "Required for timepoint=1" cümlesini hem
                // arrival_time hem departure_time için yazar. Eskiden yalnız arrival_time
                // yazılıyordu ve departure_time'ın hükmü ölçümde çapasız görünüyordu.
                Some("arrival_time|departure_time"),
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
                        st.notices.push(make_k2_notice(
                            &mut st.counter, "STM_030", EntityType::Trip, eid(),
                            None, &file.name, Some(line), Some("shape_dist_traveled"),
                            Some(d.to_string()), Some(">= 0".to_string()),
                            "shape_dist_traveled negatif olamaz.".to_string(),
                            "shape_dist_traveled değerini sıfır veya pozitif bir sayıya ayarlayın.",
                        ));
                    }
                }
                v
            }
            Err(err) => {
                // Sayı OLMAYAN değer eskiden sessizce düşüyordu: aralık dışı sayı STM_030 üretirken
                // "abc" hiçbir bulgu vermiyordu. Aynı olgunun iki dalı → aynı kural (PTH_027 emsali).
                st.notices.push(make_k2_notice(
                    &mut st.counter, "STM_030", EntityType::Trip, eid(),
                    None, &file.name, Some(line), Some("shape_dist_traveled"),
                    Some(sdt_raw.to_string()), Some(">= 0".to_string()),
                    err,
                    "shape_dist_traveled değerini sıfır veya pozitif bir sayıya ayarlayın.",
                ));
                None
            }
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
                st.notices.push(make_k2_notice(
                    &mut st.counter, "STM_042", EntityType::Trip, eid(),
                    None, &file.name, Some(line), Some("stop_headsign"),
                    Some(format!("'{bad}' karakteri içeriyor")), None,
                    format!("stop_headsign '{}' Google Transit tarafından desteklenmeyen karakter içeriyor: '{bad}'.", hs),
                    "stop_headsign değerinden ! $ % \\ * = _ karakterlerini kaldırın.",
                ));
            }
        }

        // STM_059: `pickup_type=2` / `drop_off_type=2` ("ajansı telefonla arayın") iken
        // spec ilgili booking rule'u ÖNERİR.
        //
        // ⚠️ BU BLOK FLEX BLOĞUNUN DIŞINDADIR ve öyle kalmalı. `has_flex_cols` sütun
        // VARLIĞINA bakar; `pickup_booking_rule_id` sütunu hiç yoksa Flex bloğu tümüyle
        // atlanır — ama tam o durum bu kuralın hedefidir (klasik dial-a-ride pencere de
        // booking sütunu da taşımaz). İlk yazımda blok içindeydi ve `emit_proof` kapısı
        // "STM_059 emit etmedi" diyerek yakaladı.
        //
        // ⚠️ `booking_rules.txt` YOKSA susar: o dosya olmadan bir id yazmak FK'yı kırar
        // (`BKR_017`). Ölçümde bu kapı olmadan 97.355 satır çıkmıştı, kapıyla 27.
        if st.has_booking_rules {
            let pu2 = get_col(row, cols.pickup_type) == "2";
            let do2 = get_col(row, cols.drop_off_type) == "2";
            let pbr = get_col_raw(row, cols.pickup_booking_rule_id);
            let dobr = get_col_raw(row, cols.drop_off_booking_rule_id);
            if (pu2 && pbr.is_empty()) || (do2 && dobr.is_empty()) {
                let (which, field) = if pu2 && pbr.is_empty() {
                    ("pickup_type=2", "pickup_booking_rule_id")
                } else {
                    ("drop_off_type=2", "drop_off_booking_rule_id")
                };
                st.notices.push(make_k2_notice(
                    &mut st.counter, "STM_059", EntityType::Trip, eid(),
                    None, &file.name, Some(line), Some(field),
                    None, None,
                    format!("trip_id '{}' {which} (telefonla rezervasyon) ama {field} eksik.", trip_id),
                    "Talep-üzerine biniş/iniş için booking_rules.txt'teki ilgili kuralı bu alana bağlayın.",
                ));
            }
        }

        // ── Flex GTFS alanları (B1: yalnızca feed'de flex sütunu varsa işlenir) ──
        type ParsedFlexColumns = (
            Option<(u32, u32, u32)>,
            Option<(u32, u32, u32)>,
            Option<SmolStr>,
            Option<SmolStr>,
            Option<SmolStr>,
            Option<SmolStr>,
        );
        let (start_window, end_window, location_id, location_group_id,
             pickup_booking_rule_id, drop_off_booking_rule_id): ParsedFlexColumns = if has_flex_cols {
        let start_window_raw = get_col(row, cols.start_pickup_drop_off_window);
        let end_window_raw   = get_col(row, cols.end_pickup_drop_off_window);
        let loc_id_raw       = get_col_raw(row, cols.location_id);
        let loc_grp_raw      = get_col_raw(row, cols.location_group_id);
        let pbr_raw          = get_col_raw(row, cols.pickup_booking_rule_id);
        let dobr_raw         = get_col_raw(row, cols.drop_off_booking_rule_id);

        // STM_058: Flex pencere alanları `Time` tipindedir — biçim hatası arrival_time gibi
        // RAPOR EDİLİR. Eskiden `.ok().flatten()` ile yutuluyordu: değer kayboluyor, STM_038
        // karşılaştıracak bir şey bulamıyor, satır "pencereli" sayıldığı için varlık kuralları
        // da susuyordu → bozuk bir rezervasyon penceresi feed'de HİÇ bulgu üretmiyordu.
        let mut parse_window = |raw: &str, field: &'static str| -> Option<(u32, u32, u32)> {
            match parse_gtfs_time_raw(raw, field) {
                Ok(v) => v,
                Err(err) => {
                    st.notices.push(make_k2_notice(
                        &mut st.counter, "STM_058", EntityType::Trip, eid(),
                        None, &file.name, Some(line), Some(field),
                        Some(raw.to_string()), Some("HH:MM:SS".to_string()), err,
                        "Flex pencere alanlarını HH:MM:SS formatında girin.",
                    ));
                    None
                }
            }
        };
        let start_window = parse_window(start_window_raw, "start_pickup_drop_off_window");
        let end_window   = parse_window(end_window_raw,   "end_pickup_drop_off_window");

        let has_start_window = !start_window_raw.is_empty();
        let has_end_window   = !end_window_raw.is_empty();
        let has_any_window   = has_start_window || has_end_window;
        let has_location     = !loc_id_raw.is_empty() || !loc_grp_raw.is_empty();

        // STM_037: Flex penceresinde arrival_time/departure_time yasak
        if has_any_window && (arrival_time.is_some() || departure_time.is_some()) {
            st.notices.push(make_k2_notice(
                &mut st.counter, "STM_037", EntityType::Trip, eid(),
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
                    st.notices.push(make_k2_notice(
                        &mut st.counter, "STM_051", EntityType::Trip, eid(),
                        None, &file.name, Some(line), Some("pickup_type"),
                        Some(pt.to_string()), Some("1 veya 2".to_string()),
                        format!("trip_id '{trip_id}' Flex penceresi tanımlı iken pickup_type={pt} yasaktır."),
                        "Flex penceresi olan stop_times satırlarında pickup_type'ı 1 (alış yok) veya 2 (telefonla) yapın.",
                    ));
                }
            }
            // STM_052: Flex penceresi tanımlı iken drop_off_type=0 (düzenli) yasak.
            if drop_off_type == Some(0) {
                st.notices.push(make_k2_notice(
                    &mut st.counter, "STM_052", EntityType::Trip, eid(),
                    None, &file.name, Some(line), Some("drop_off_type"),
                    Some("0".to_string()), Some("1 veya 2".to_string()),
                    format!("trip_id '{trip_id}' Flex penceresi tanımlı iken drop_off_type=0 yasaktır."),
                    "Flex penceresi olan stop_times satırlarında drop_off_type'ı 1 (iniş yok) veya 2 (telefonla) yapın.",
                ));
            }
            // STM_054/055: Flex penceresi tanımlı iken continuous_pickup/continuous_drop_off
            // 1 (veya boş) dışında yasak. GTFS spec [Kesin]: stop_times tarafındaki koşul SATIR
            // kapsamlıdır ("Forbidden if start/end_pickup_drop_off_window are defined") — routes
            // tarafındaki rota-kapsamlı koşul (RTS_028, K4) ile karıştırılmamalı.
            // Enum dışı değerler (>3) STM_018/019'un işi; burada çift emit etmemek için elenir.
            if matches!(continuous_pickup, Some(0) | Some(2) | Some(3)) {
                let cp = continuous_pickup.unwrap();
                st.notices.push(make_k2_notice(
                    &mut st.counter, "STM_054", EntityType::Trip, eid(),
                    None, &file.name, Some(line), Some("continuous_pickup"),
                    Some(cp.to_string()), Some("1 veya boş".to_string()),
                    format!("trip_id '{trip_id}' Flex penceresi tanımlı iken continuous_pickup={cp} yasaktır."),
                    "Flex penceresi olan stop_times satırlarında continuous_pickup'ı 1 yapın veya alanı boş bırakın.",
                ));
            }
            if matches!(continuous_drop_off, Some(0) | Some(2) | Some(3)) {
                let cd = continuous_drop_off.unwrap();
                st.notices.push(make_k2_notice(
                    &mut st.counter, "STM_055", EntityType::Trip, eid(),
                    None, &file.name, Some(line), Some("continuous_drop_off"),
                    Some(cd.to_string()), Some("1 veya boş".to_string()),
                    format!("trip_id '{trip_id}' Flex penceresi tanımlı iken continuous_drop_off={cd} yasaktır."),
                    "Flex penceresi olan stop_times satırlarında continuous_drop_off'u 1 yapın veya alanı boş bırakın.",
                ));
            }
        }

        // STM_038: start_window >= end_window
        // Eşitlik de hatadır: sıfır uzunluklu bir pencere hizmetin hiçbir an açık
        // olmadığı anlamına gelir. Spec bu sıralamayı HİÇ yazmaz; kural Interop'tur
        // ve şeklini MobilityData belirler: "end_pickup_drop_off_window must be
        // strictly later than the start_pickup_drop_off_window". Eskiden yalnız
        // `>` bakıyorduk ve eşitliği kaçırıyorduk (mdb-2741'de MD 38, bizde 0).
        if let (Some(sw), Some(ew)) = (start_window, end_window) {
            let sw_secs = sw.0 * 3600 + sw.1 * 60 + sw.2;
            let ew_secs = ew.0 * 3600 + ew.1 * 60 + ew.2;
            if sw_secs >= ew_secs {
                st.notices.push(make_k2_notice(
                    &mut st.counter, "STM_038", EntityType::Trip, eid(),
                    None, &file.name, Some(line), Some("start_pickup_drop_off_window"),
                    Some(format!("{start_window_raw} >= {end_window_raw}")), None,
                    format!("trip_id '{}' start_pickup_drop_off_window, end_pickup_drop_off_window'dan önce değil.", trip_id),
                    "start_pickup_drop_off_window değerini end_pickup_drop_off_window'dan kesin olarak küçük yapın.",
                ));
            }
        }

        // STM_039: location_id/group_id var ama pencere eksik
        if has_location && (!has_start_window || !has_end_window) {
            let missing = if !has_start_window { "start_pickup_drop_off_window" } else { "end_pickup_drop_off_window" };
            st.notices.push(make_k2_notice(
                &mut st.counter, "STM_039", EntityType::Trip, eid(),
                None, &file.name, Some(line), Some(missing),
                None, Some("HH:MM:SS".to_string()),
                format!("trip_id '{}' location_id/group_id tanımlı ama {missing} eksik.", trip_id),
                "Flex stop_times için hem start_pickup_drop_off_window hem end_pickup_drop_off_window girin.",
            ));
        }

        // STM_040: Flex penceresi var ama booking_rule_id yok
        if has_any_window && pbr_raw.is_empty() && dobr_raw.is_empty() {
            st.notices.push(make_k2_notice(
                &mut st.counter, "STM_040", EntityType::Trip, eid(),
                None, &file.name, Some(line), Some("pickup_booking_rule_id"),
                None, None,
                format!("trip_id '{}' Flex penceresi tanımlı ama pickup/drop_off_booking_rule_id eksik.", trip_id),
                "Flex rezervasyon için pickup_booking_rule_id veya drop_off_booking_rule_id girin.",
            ));
        }

        // STM_041: stop_id ve location_id/group_id aynı anda dolu (çakışma)
        if !raw_stop.is_empty() && has_location {
            st.notices.push(make_k2_notice(
                &mut st.counter, "STM_041", EntityType::Trip, eid(),
                None, &file.name, Some(line),
                // Çakışma üç alan arasındadır; eskiden yalnız "location_id" yazılıyordu ve
                // çakışma location_group_id ileyken kullanıcı yanlış alana bakıyordu.
                Some("stop_id|location_id|location_group_id"),
                Some(if loc_id_raw.is_empty() { loc_grp_raw.to_string() } else { loc_id_raw.to_string() }),
                Some("(boş)".to_string()),
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

        // ── Per-trip toplama ──
        st.total_rows += 1;
        if !trip_id.is_empty() {
            let next_idx = st.trips_agg.len() as u32;
            let agg = st.trips_agg.entry(trip_id.clone()).or_insert_with(|| TripAgg::new(line, next_idx));
            if stop_idx != u32::MAX {
                let stop_smol = st.stop_intern[stop_idx as usize].clone();
                st.stop_first_line.entry(stop_smol).or_insert(line);
                // stop_idx_set kaldırıldı; trip_stop_set finalize sonrası rows'dan inşa edilir
            }
            if matches!(continuous_pickup, Some(0) | Some(1))
                || matches!(continuous_drop_off, Some(0) | Some(1))
            {
                agg.continuous = true;
            }
            let trip_idx = agg.idx;
            st.row_trip.push(trip_idx);
            // side map'ler: CSV satır numarası anahtarlı (permutation'dan bağımsız)
            if let Some(ref hs) = stop_headsign {
                st.stop_headsigns.insert(line as u32, hs.clone());
            }
            if let Some(flex) = build_flex(
                start_window, end_window,
                location_id.clone(), location_group_id.clone(),
                pickup_booking_rule_id.clone(), drop_off_booking_rule_id.clone(),
                pickup_type, drop_off_type,
            ) {
                st.flex_map.insert(line as u32, flex);
            }
            st.all_rows.push(CompactStopTime {
                stop_idx,
                // Bozuk değer u32::MAX'e DÜŞMEZ: "alan yok" ile "alan dolu ama okunamadı"
                // ayrı sentinellerdir, yoksa STM_015/016 dolu bir alan için "eksik" der.
                arrival_secs:   arrival_time.map(|(h,m,s)| h*3600+m*60+s)
                                    .unwrap_or(if arr_malformed { TIME_MALFORMED } else { u32::MAX }),
                departure_secs: departure_time.map(|(h,m,s)| h*3600+m*60+s)
                                    .unwrap_or(if dep_malformed { TIME_MALFORMED } else { u32::MAX }),
                sequence:       stop_sequence.unwrap_or(u32::MAX),
                dist_f32:       shape_dist_traveled.map(|d| d as f32).unwrap_or(f32::NAN),
                line:           line as u32,
            });
        }
        };

        // ARC_033: `next_csv_record` bu değişkene yazar, döngü satır numarasını ekler.
        let mut scan = CsvScanState::default();
        // ── Sürücü: stream raw_text (başlığı atla), ZIP stream veya rows fallback ──
        if let Some(text) = &file.raw_text {
            let mut pos = 0usize;
            let mut buf: Vec<Cow<'_, str>> = Vec::with_capacity(16);
            let mut data_idx = 0usize;
            let mut header_skipped = false;
            while next_csv_record(text, &mut pos, &mut buf, &mut scan) {
                // Boş satır (tek boş alan) — tokenize_csv ile aynı filtre
                if buf.len() == 1 && buf[0].is_empty() {
                    continue;
                }
                if !header_skipped {
                    header_skipped = true;
                    continue;
                }
                st.unclosed |= scan.unclosed;
                if let Some((k, ex)) = scan.rfc.take() {
                    st.rfc.observe((data_idx + 2) as u64, k, &ex);
                }
                process(&mut st, &buf, (data_idx + 2) as u64);
                data_idx += 1;
            }
        } else if let Some(zb) = zip_bytes {
            // #38: stop_times.txt K1'de yalnızca başlık okundu; K2 ZIP'i yeniden açıp stream eder.
            let cursor = Cursor::new(zb);
            match zip::ZipArchive::new(cursor) {
                Err(e) => {
                    st.notices.push(make_k2_notice(
                        &mut st.counter, "ARC_009", EntityType::File, Some(file.name.clone()),
                        None, &file.name, None, None, None, None,
                        format!("'{}' ZIP yeniden açılamadı: {e}.", file.name),
                        "ZIP arşivini kontrol edin.",
                    ));
                }
                Ok(mut archive) => {
                    match archive.by_name(&file.name) {
                        Err(e) => {
                            st.notices.push(make_k2_notice(
                                &mut st.counter, "ARC_009", EntityType::File, Some(file.name.clone()),
                                None, &file.name, None, None, None, None,
                                format!("'{}' ZIP girdisi bulunamadı: {e}.", file.name),
                                "ZIP arşivini kontrol edin.",
                            ));
                        }
                        #[cfg(feature = "parallel")]
                        Ok(mut entry) => {
                            use rayon::prelude::*;
                            // Deflate akışı rastgele erişilemez → gövdeyi bir kez aç, sonra
                            // satır/trip sınırlarında bölüp parçaları paralel ayrıştır.
                            let mut body: Vec<u8> = Vec::with_capacity(file.bytes as usize + 1);
                            if entry.read_to_end(&mut body).is_ok() {
                                let n = std::thread::available_parallelism().map(|v| v.get()).unwrap_or(4);
                                let ranges = split_body_at_trip_boundaries(&body, cols.trip_id, n);
                                let parts: Vec<StChunk> = ranges.par_iter().map(|&(bs, be, first_line)| {
                                    let mut c = StChunk { has_booking_rules, ..StChunk::default() };
                                    // Bayrak chunk'a TAŞINMALI: `StChunk::default()` her chunk için
                                    // yeniden kurulur ve `has_booking_rules` false kalırdı → STM_059
                                    // yalnız tek parçalı feed'lerde çalışırdı. `emit_proof` yakaladı.
                                    let mut rdr = ZipCsvReader::new(Cursor::new(&body[bs..be]));
                                    let mut fields: Vec<Vec<u8>> = Vec::with_capacity(header_count + 1);
                                    let mut line = first_line;
                                    while rdr.next_record(&mut fields) {
                                        if fields.len() == 1 && fields[0].is_empty() { continue; }
                                        let row: Vec<Cow<'_, str>> = fields.iter()
                                            .map(|f| match std::str::from_utf8(f) {
                                                Ok(x) => Cow::Borrowed(x),
                                                Err(_) => Cow::Owned(String::from_utf8_lossy(f).into_owned()),
                                            })
                                            .collect();
                                        c.unclosed |= rdr.unclosed;
                                        if let Some((k, ex)) = rdr.rfc.take() {
                                            c.rfc.observe(line, k, &ex);
                                        }
                                        process(&mut c, &row, line);
                                        line += 1;
                                    }
                                    c
                                }).collect();
                                let mut it = parts.into_iter();
                                if let Some(first) = it.next() { st = first; }
                                for c in it { st.merge(c); }
                            }
                        }
                        #[cfg(not(feature = "parallel"))]
                        Ok(entry) => {
                            let mut csv_reader = ZipCsvReader::new(entry);
                            let mut raw_fields: Vec<Vec<u8>> = Vec::with_capacity(header_count + 1);
                            let mut zip_line: u64 = 2;
                            let mut header_skipped = false;
                            // ── Parçalama (Adım 2: SERİ) ────────────────────────────────
                            // Sınır TRIP DEĞİŞİMİNDE kurulur: stop_times trip'e göre gruplu
                            // olduğundan aynı trip iki parçaya düşmez, dolayısıyla STM_036'nın
                            // parça-sınırı sorunu doğmaz. Şu an paralellik YOK; amaç `merge`
                            // yolunun çıktıyı birebir koruduğunu tek başına kanıtlamak.
                            const CHUNK_ROWS: usize = 500_000;
                            let mut chunks: Vec<StChunk> = Vec::new();
                            let mut cur = StChunk::default();
                            cur.has_booking_rules = has_booking_rules;
                            let mut rows_in_chunk: usize = 0;
                            let mut boundary_trip: Option<SmolStr> = None;
                            while csv_reader.next_record(&mut raw_fields) {
                                if raw_fields.len() == 1 && raw_fields[0].is_empty() {
                                    continue;
                                }
                                if !header_skipped {
                                    header_skipped = true;
                                    continue;
                                }
                                // Alanlar `raw_fields`ten ÖDÜNÇ alınır: geçerli UTF-8'de kopya
                                // yok. `cow_row` `process` dönüşünde düşer, dolayısıyla bir
                                // sonraki `next_record(&mut raw_fields)` ile çakışmaz. (Eskiden
                                // her alan `to_owned()` ile kopyalanıyordu — VBB'de satır başına
                                // 1 Vec + ~10 String, yani ~68M tahsis.) Bozuk UTF-8 nadir
                                // istisnadır ve yalnız o satırda `Cow::Owned`a düşer.
                                let cow_row: Vec<Cow<'_, str>> = raw_fields.iter()
                                    .map(|f| match std::str::from_utf8(f) {
                                        Ok(s) => Cow::Borrowed(s),
                                        Err(_) => Cow::Owned(String::from_utf8_lossy(f).into_owned()),
                                    })
                                    .collect();
                                // Sınıra gelindiyse trip değişimini BEKLE, sonra böl.
                                if rows_in_chunk >= CHUNK_ROWS {
                                    let tid = get_col_raw(&cow_row, cols.trip_id);
                                    match &boundary_trip {
                                        None => boundary_trip = Some(SmolStr::from(tid)),
                                        Some(b) if b.as_str() != tid => {
                                            chunks.push(std::mem::take(&mut cur));
                                            // `mem::take` chunk'ı Default'a sıfırlar → bayrak
                                            // yeniden kurulmalı, yoksa ilk chunk'tan sonrası sessizleşir.
                                            cur.has_booking_rules = has_booking_rules;
                                            rows_in_chunk = 0;
                                            boundary_trip = None;
                                        }
                                        _ => {}
                                    }
                                }
                                cur.unclosed |= csv_reader.unclosed;
                                if let Some((k, ex)) = csv_reader.rfc.take() {
                                    cur.rfc.observe(zip_line, k, &ex);
                                }
                                process(&mut cur, &cow_row, zip_line);
                                rows_in_chunk += 1;
                                zip_line += 1;
                            }
                            chunks.push(cur);
                            // Dosya sırasında birleştir: ilki temel, sonrakiler katılır.
                            let mut it = chunks.into_iter();
                            if let Some(first) = it.next() {
                                st = first;
                            }
                            for c in it {
                                st.merge(c);
                            }
                        }
                    }
                }
            }
        } else {
            // Fallback (test/legacy): önceden parse edilmiş rows üzerinden. SmolStr'ları
            // Cow::Borrowed olarak sar (alloc yok) — closure'ın &[Cow] imzasıyla uyum.
            for (row_idx, row) in file.rows.iter().enumerate() {
                let cow_row: Vec<Cow<'_, str>> =
                    row.iter().map(|s| Cow::Borrowed(s.as_str())).collect();
                process(&mut st, &cow_row, (row_idx + 2) as u64);
            }
        }
    }

    // ARC_013: akış gövdesinde kapanmamış tırnak (issue #84).
    if st.unclosed {
        st.notices.push(super::common::arc013_unclosed_stream(&file.name, &mut st.counter));
    }

    // ARC_033: DOSYA başına TEK özet (chunk'lar `StChunk::merge` ile birleşti).
    if let Some(n) = super::common::arc033_summary(&st.rfc, &file.name, &mut st.counter) {
        st.notices.push(n);
    }

    // ⚠️ ARC_022 BURADA DEĞİL — `k2/mod.rs`'teki tek geçişte (issue #75). Gerekçe
    // shapes.rs'teki notla aynı.

    // ARC_009: başlık var ama veri satırı yok (K1'den taşındı; ZIP streaming yolu dahil)
    if (file.raw_text.is_some() || zip_bytes.is_some()) && st.total_rows == 0 {
        st.notices.push(make_k2_notice(
            &mut st.counter, "ARC_009", EntityType::File, Some(file.name.clone()),
            None, &file.name, None, None,
            None, None,
            format!("'{}' dosyasında başlık satırı var ama veri satırı yok.", file.name),
            "Dosyaya en az bir veri satırı ekleyin.",
        ));
    }

    // DQ_016: DOSYA başına TEK özet (satır-başına değil — patlama önlemi).
    if let Some((observed, msg, cols)) = st.dq016.summary(&file.name) {
        let mut n = make_k2_notice(
            &mut st.counter, "DQ_016", EntityType::File, Some(file.name.clone()),
            None, &file.name, st.dq016.first_line, Some(cols.as_str()),
            Some(observed), None, msg, crate::k1_parse::DQ016_REMEDIATION,
        );
        n.details = st.dq016.evidence_details();
        st.notices.push(n);
    }

    // STM_050: feed-seviyesi TEK özet (satır-başına değil — büyük feed OOM önlemi).
    if st.stm050_empty > 0 {
        st.notices.push(make_k2_notice(
            &mut st.counter, "STM_050", EntityType::Feed, None,
            None, &file.name, None, Some("timepoint"),
            Some(format!("{}", st.stm050_empty)), None,
            format!("{} stop_times satırında timepoint sütunu var ama değer boş; bu satırlar örtük olarak 1 (kesin zaman) sayılır.", st.stm050_empty),
            "Yaklaşık zamanlı duraklarda timepoint=0, kesin zamanlı duraklarda timepoint=1 olarak açıkça girin.",
        ));
    }

    // STM_018/019 agregasyonu (#151): üreticilerde aynı enum ihlali çoğu zaman
    // stop_times satırlarının tamamına yayıldığı için satır-başına raporlamak
    // R2/R5/R9'u gürültüyle doldurur. SHP_010/STP_022 emsali gibi düşük hacimde
    // pinpoint korunur, eşik üstünde tek feed özeti üretilir.
    const STM018_019_AGG_THRESHOLD: usize = 50;
    finalize_stm_pending(
        &mut st.stm018_pending,
        &mut st.notices,
        &mut st.counter,
        &file.name,
        "STM_018",
        "continuous_pickup",
        "continuous_pickup alanlarını 0, 1, 2 veya 3 değerlerinden biri olarak düzeltin.",
        "continuous_pickup alanı 0-3 dışında — aynı enum ihlali yüksek hacimde tekrarlanıyor.",
        STM018_019_AGG_THRESHOLD,
    );
    finalize_stm_pending(
        &mut st.stm019_pending,
        &mut st.notices,
        &mut st.counter,
        &file.name,
        "STM_019",
        "continuous_drop_off",
        "continuous_drop_off alanlarını 0, 1, 2 veya 3 değerlerinden biri olarak düzeltin.",
        "continuous_drop_off alanı 0-3 dışında — aynı enum ihlali yüksek hacimde tekrarlanıyor.",
        STM018_019_AGG_THRESHOLD,
    );

    crate::timing::mem_log("  stop_times: st.all_rows + st.trips_agg built (pre-finalize)");

    // ── Finalize: in-place gruplama (Asama 1b + #15 W1) ──
    // counts/offsets sadece trip_ranges + cikti duzeni icin; satirlar all_rows icinde
    // yerinde (permutasyon) gruplanir. Eski counting-placement ikinci tam rows Vec'i
    // all_rows ile AYNI ANDA tutuyordu (~2x peak); kaldirildi.
    let n_trips = st.trips_agg.len();
    let mut counts = vec![0u32; n_trips];
    for &ti in &st.row_trip {
        counts[ti as usize] += 1;
    }
    // offsets[i] = trip i'nin rows içindeki başlangıcı; offsets[n_trips] = toplam
    let mut offsets = vec![0u32; n_trips + 1];
    for i in 0..n_trips {
        offsets[i + 1] = offsets[i] + counts[i];
    }
    // #15 W1: ikinci tam Vec ASLA ayrilmaz. Cikti sirasi = (trip_idx ASC, stop_sequence
    // [None=MAX] ASC, dosya-ordinali ASC). Dosya-ordinali son tie-breaker => TAM siralama;
    // esit-anahtar nondeterminizmi YOK. Eski davranisla birebir: trip gruplama + trip-ici
    // stable-by-sequence (file-order ties) == bu total-order key.
    let n = st.all_rows.len();
    let mut perm: Vec<u32> = (0..n as u32).collect();
    let _t_sort = crate::timing::Timer::start("K2::st::fin::sort");
    perm.sort_unstable_by(|&a, &b| {
        let (ia, ib) = (a as usize, b as usize);
        st.row_trip[ia]
            .cmp(&st.row_trip[ib])
            .then_with(|| st.all_rows[ia].sequence.cmp(&st.all_rows[ib].sequence))
            .then_with(|| a.cmp(&b))
    });
    drop(_t_sort);
    drop(st.row_trip); // perm kuruldu; 4B/satir tag in-place uygulamadan ONCE serbest birak
    // perm[pos] = pos. cikti pozisyonuna gelecek KAYNAK indeks. In-place icin dest'e
    // (eleman i NEREYE gider) cevir, sonra cycle-swap uygula (ikinci Vec yok).
    let mut dest: Vec<u32> = vec![0u32; n];
    for (pos, &src) in perm.iter().enumerate() {
        dest[src as usize] = pos as u32;
    }
    drop(perm);
    let _t_perm = crate::timing::Timer::start("K2::st::fin::permute");
    for k in 0..n {
        while dest[k] as usize != k {
            let d = dest[k] as usize;
            st.all_rows.swap(k, d);
            dest.swap(k, d);
        }
    }
    drop(_t_perm);
    let rows = st.all_rows; // zero-copy: yerinde gruplandi + siralandi

    // STM_032 post-finalize: sıralanmış satırlarda ardışık aynı stop_sequence → dup.
    // Process closure'da değil (seen_seq set kaldırıldı); burada windows(2) ile O(n) tespit.
    //
    // 🔑 **Bu, `stop_sequence` ARTIŞ hükmünün TAMAMINI ölçer (#97).** Satırlar hemen yukarıda
    // `(trip, stop_sequence, dosya-ordinali)` anahtarıyla sıralandığı için trip içindeki
    // değerler bu görünümde tanım gereği azalmaz; hükmü ihlal edebilecek TEK durum eşitliktir.
    // Yani "değerler sefer boyunca artmalı" ⇔ "(trip_id, stop_sequence) benzersiz" — ki spec
    // bunu ikinci kez `stop_times.txt` başlığında "Primary key (trip_id, stop_sequence)" diye
    // söyler. Ayrı bir "azalma" kuralı BOŞ KÜME ölçerdi.
    // ⚠️ `STM_036` bu hükmün kanıtı DEĞİLDİR: o DOSYA SIRASINI ölçer (MD `unsorted_stop_times`,
    // INFO) ve değerleri düzgün artan geçerli bir feed'de de ateşler. Spec dosya satırlarının
    // sıralı olmasını istemez.
    {
        let _t_032 = crate::timing::Timer::start("K2::st::fin::stm032");
        let mut stm032_pending = Vec::new();
        let mut stm056_pending = Vec::new();
        // trips_agg bir HashMap; emisyon sırası deterministik olsun diye trip_id'ye göre gez.
        let mut tids: Vec<&SmolStr> = st.trips_agg.keys().collect();
        tids.sort_unstable();
        for tid in tids {
            if tid.is_empty() { continue; }
            let agg = &st.trips_agg[tid];
            let (s, e) = (offsets[agg.idx as usize] as usize, offsets[agg.idx as usize + 1] as usize);
            for w in rows[s..e].windows(2) {
                let (a, b) = (&w[0], &w[1]);
                if a.sequence != u32::MAX && a.sequence == b.sequence {
                    stm032_pending.push(make_k2_notice(
                        &mut st.counter, "STM_032", EntityType::Trip, Some(tid.to_string()),
                        None, &file.name, Some(b.line as u64), Some("stop_sequence"),
                        Some(b.sequence.to_string()), None,
                        format!("trip_id '{}' için stop_sequence {} tekrar ediyor.", tid, b.sequence),
                        "Her (trip_id, stop_sequence) çifti stop_times.txt'te benzersiz olmalıdır.",
                    ));
                }
                // STM_056 (MD `decreasing_or_equal_stop_time_distance`): stop_sequence boyunca
                // shape_dist_traveled ARTMALI. Yalnız iki satırda da değer varsa karşılaştırılır;
                // eksik değer STM_017'nin konusudur. NaN karşılaştırması false döner, guard şart.
                if a.dist_f32.is_finite() && b.dist_f32.is_finite() && b.dist_f32 <= a.dist_f32 {
                    stm056_pending.push(make_k2_notice(
                        &mut st.counter, "STM_056", EntityType::Trip, Some(tid.to_string()),
                        None, &file.name, Some(b.line as u64), Some("shape_dist_traveled"),
                        Some(format!("{}", b.dist_f32)), Some(format!("> {}", a.dist_f32)),
                        format!(
                            "'{}' seferinde shape_dist_traveled artmıyor: stop_sequence {} değeri {}, önceki durakta {}. Değerler stop_sequence boyunca artmalıdır.",
                            tid, b.sequence, b.dist_f32, a.dist_f32
                        ),
                        "stop_times.txt'te shape_dist_traveled değerlerini stop_sequence sırasına göre artan biçimde düzeltin.",
                    ));
                }
            }
        }
        const STM032_AGG_THRESHOLD: usize = 50;
        finalize_stm_pending(
            &mut stm032_pending,
            &mut st.notices,
            &mut st.counter,
            &file.name,
            "STM_032",
            "stop_sequence",
            "Her (trip_id, stop_sequence) çifti stop_times.txt'te benzersiz olmalıdır.",
            "stop_sequence tekrar ediyor — aynı (trip_id, stop_sequence) ihlali yüksek hacimde tekrarlanıyor.",
            STM032_AGG_THRESHOLD,
        );
        const STM056_AGG_THRESHOLD: usize = 50;
        finalize_stm_pending(
            &mut stm056_pending,
            &mut st.notices,
            &mut st.counter,
            &file.name,
            "STM_056",
            "shape_dist_traveled",
            "stop_times.txt'te shape_dist_traveled değerlerini stop_sequence sırasına göre artan biçimde düzeltin.",
            "shape_dist_traveled stop_sequence boyunca artmıyor — aynı mesafe sırası ihlali yüksek hacimde tekrarlanıyor.",
            STM056_AGG_THRESHOLD,
        );
    }

    // stop_id_set = stop_first_line anahtarları (aynı guard'larla → birebir aynı küme).
    let mut index = StopTimesIndex {
        total_rows: st.total_rows,
        unsorted_seq_trips: st.unsorted_seq_trips,
        stop_first_line: st.stop_first_line,
        rows,
        ..Default::default()
    };
    index.stop_id_set = index.stop_first_line.keys().cloned().collect();
    index.stop_intern = st.stop_intern;
    index.stop_id_to_idx = st.stop_id_to_idx;
    index.stop_headsigns = st.stop_headsigns;
    index.flex_map = st.flex_map;
    let _t_maps = crate::timing::Timer::start("K2::st::fin::maps");
    for (tid, agg) in st.trips_agg {
        if tid.is_empty() {
            continue; // boş trip_id çıktıya girmez (orijinal `if !trip_id.is_empty()` guard'ı)
        }
        let i = agg.idx as usize;
        index.trip_ranges.insert(tid.clone(), (offsets[i], offsets[i + 1]));
        index.trip_id_set.insert(tid.clone());
        index.trip_first_line.insert(tid.clone(), agg.first_line as u64);
        if agg.continuous {
            index.continuous_trips.insert(tid.clone());
        }
        // stop_idx_set TripAgg'dan kaldırıldı — trip_stop_set aşağıda inşa edilir
    }
    drop(_t_maps);

    // STM_048: ham GTFS servis-günü ihlalini normalization'dan ÖNCE kaydet.
    // K6 daha sonra aynı satırları analitik devamlılık için 24:xx'e çevirir; bu Spec
    // notice'ı o düzeltme nedeniyle kaybolmamalıdır.
    for violation in index.find_raw_midnight_wraps() {
        let observed = format_gtfs_time(violation.observed_secs);
        let expected = format_gtfs_time(violation.expected_secs);
        let previous = format_gtfs_time(violation.previous_secs);
        let mut notice = make_k2_notice(
            &mut st.counter, "STM_048", EntityType::Trip,
            Some(violation.trip_id.to_string()), None, &file.name,
            Some(violation.line), Some(violation.field), Some(observed.clone()),
            Some(expected.clone()),
            format!("'{}' trip'inde {} satırında {} değeri {} olarak yazılmış; önceki saat {}. \
                GTFS servis günü için {} kullanılmalıdır.",
                violation.trip_id, violation.line, violation.field, observed, previous, expected),
            "Gece yarısından sonraki stop_times saatini 24:00:00, 25:00:00… biçiminde yazın; 00:xx kullanmayın.",
        );
        notice.details = Some([
            ("trip_id".to_string(), violation.trip_id.to_string()),
            ("stop_id".to_string(), violation.stop_id.to_string()),
            ("previous_time".to_string(), previous),
            ("observed_time".to_string(), observed),
            ("expected_time".to_string(), expected),
        ].into_iter().collect());
        st.notices.push(notice);
    }

    for violation in index.find_raw_same_row_midnight_departures() {
        let observed = format_gtfs_time(violation.observed_secs);
        let expected = format_gtfs_time(violation.expected_secs);
        let previous = format_gtfs_time(violation.previous_secs);
        let mut notice = make_k2_notice(
            &mut st.counter, "STM_049", EntityType::Trip,
            Some(violation.trip_id.to_string()), None, &file.name,
            Some(violation.line), Some(violation.field), Some(observed.clone()),
            Some(expected.clone()),
            format!("'{}' trip'inde {} satırında departure_time {} olarak yazılmış; aynı satırın arrival_time değeri {}. \
                GTFS servis günü için {} kullanılmalıdır.",
                violation.trip_id, violation.line, observed, previous, expected),
            "Aynı satırdaki gece yarısı sonrası departure_time değerini 24:00:00+ biçiminde yazın; 00:xx kullanmayın.",
        );
        notice.details = Some([
            ("trip_id".to_string(), violation.trip_id.to_string()),
            ("stop_id".to_string(), violation.stop_id.to_string()),
            ("previous_time".to_string(), previous),
            ("observed_time".to_string(), observed),
            ("expected_time".to_string(), expected),
        ].into_iter().collect());
        st.notices.push(notice);
    }

    // ── trip_stop_set post-finalize inşası (#38 peak-2) ──────────────────────────
    // TripAgg'lar drop edildi (stop_idx_set heap'leri serbest); trip_ranges+rows üzerinden
    // per-trip unique stop_idx setleri yeniden inşa edilir. Semantik birebir: same sets.
    crate::timing::mem_log("K2 before trip_stop_set rebuild (st.trips_agg freed)");
    {
        let _t_tss = crate::timing::Timer::start("K2::st::fin::trip_stop_set");
        for (tid, &(s, e)) in &index.trip_ranges {
            if s == e { continue; }
            let set: FxHashSet<u32> = index.rows[s as usize..e as usize]
                .iter()
                .map(|st| st.stop_idx)
                .filter(|&i| i != u32::MAX)
                .collect();
            if !set.is_empty() {
                index.trip_stop_set.insert(tid.clone(), set);
            }
        }
    }
    crate::timing::mem_log("K2 after trip_stop_set rebuild");

    crate::timing::mem_log("  stop_times: index finalized (rows placed, st.all_rows freed, maps built)");
    index.log_mem_report();
    (index, st.notices)
}

#[allow(clippy::too_many_arguments)]
fn finalize_stm_pending(
    pending: &mut Vec<Notice>,
    notices: &mut Vec<Notice>,
    counter: &mut u32,
    file_name: &str,
    rule_id: &str,
    field: &str,
    remediation: &str,
    summary: &str,
    threshold: usize,
) {
    let n = pending.len();
    if n > threshold {
        // Pending listesi HashMap/chunk iterasyonundan geldiği için ham take(5)
        // deterministik değildir. Önce sırala, sonra ilk 5'i al.
        let mut examples: Vec<String> = pending.iter()
            .filter_map(|x| x.entity_id.clone())
            .collect();
        examples.sort_unstable();
        examples.truncate(5);
        let mut notice = make_k2_notice(
            counter, rule_id, EntityType::Feed, None,
            None, file_name, None, Some(field),
            Some(n.to_string()), None,
            format!("{n} stop_times satırında {summary}"),
            remediation,
        );
        let mut d = std::collections::BTreeMap::new();
        d.insert("affected_rows".to_string(), n.to_string());
        if !examples.is_empty() {
            d.insert("example_trips".to_string(), examples.join(", "));
        }
        notice.details = Some(d);
        notices.push(notice);
    } else {
        notices.append(pending);
    }
}

// Bu validator ham değer, alan ve trip satırı bağlamını birlikte taşır; tek kullanımlık
// bir parametre struct'ı emit edilen notice'ın kaynaklarını daha açık yapmaz.
#[allow(clippy::too_many_arguments)]
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
        assert_eq!(t1[0].departure_time(), Some((23, 58, 0)), "ilk durak dokunulmamalı");
        assert_eq!(t1[1].arrival_time(), Some((24, 1, 0)), "00:01 → 24:01 normalize");
        assert_eq!(t1[1].departure_time(), Some((24, 1, 0)), "departure aynı offset'i takip eder");
        let t2 = idx.sorted_stops("T2").unwrap();
        assert_eq!(t2[1].arrival_time(), Some((7, 55, 0)), "gerçek hata (eşik üstü) korunur");

        // start_hour = 0 → normalizasyon kapalı.
        let mut idx0 = StopTimesIndex::from_records(&recs);
        idx0.normalize_service_day(0);
        assert_eq!(idx0.sorted_stops("T1").unwrap()[1].arrival_time(), Some((0, 1, 0)));
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
        let (records_idx, notices) = validate_stop_times(&file, None, false);
        assert_eq!(records_idx.rows.len(), 2);
        assert!(notices.is_empty(), "Geçerli stop_times notice üretmemeli: {:?}", notices);
    }

    #[test]
    fn zip_csv_reader_accepts_quoted_final_field_at_eof() {
        let mut reader = ZipCsvReader::new(Cursor::new(b"a,b\n\"1\",\"a\"\"b\""));
        let mut fields = Vec::new();

        assert!(reader.next_record(&mut fields));
        assert_eq!(fields, vec![b"a".to_vec(), b"b".to_vec()]);
        assert!(reader.next_record(&mut fields));
        assert_eq!(fields, vec![b"1".to_vec(), b"a\"b".to_vec()]);
        assert!(!reader.next_record(&mut fields));
        assert!(!reader.unclosed, "kapanış tırnağından sonraki EOF ARC_013 olmamalı");
    }

    #[test]
    fn zip_csv_reader_reports_unclosed_final_quoted_field() {
        let mut reader = ZipCsvReader::new(Cursor::new(b"a,b\n\"1\",\"unclosed"));
        let mut fields = Vec::new();

        assert!(reader.next_record(&mut fields));
        assert_eq!(fields, vec![b"a".to_vec(), b"b".to_vec()]);
        assert!(reader.next_record(&mut fields));
        assert_eq!(fields, vec![b"1".to_vec(), b"unclosed".to_vec()]);
        assert!(reader.unclosed, "kapanış tırnağı olmadan EOF ARC_013 üretmeli");
    }

    #[test]
    fn missing_stop_sequence_produces_stm_005() {
        let file = make_file(
            vec!["trip_id", "arrival_time", "departure_time", "stop_id", "stop_sequence"],
            vec![vec!["T1", "08:00:00", "08:00:00", "S1", ""]],
        );
        let (_, notices) = validate_stop_times(&file, None, false);
        assert!(notices.iter().any(|n| n.rule_id == "STM_005"));
    }

    #[test]
    fn missing_stop_id_produces_stm_006() {
        let file = make_file(
            vec!["trip_id", "arrival_time", "departure_time", "stop_id", "stop_sequence"],
            vec![vec!["T1", "08:00:00", "08:00:00", "", "1"]],
        );
        let (_, notices) = validate_stop_times(&file, None, false);
        assert!(notices.iter().any(|n| n.rule_id == "STM_006"));
    }

    // STM_007 testi K6'ya taşındı (k6_analytics.rs: departure_before_arrival_*).

    #[test]
    fn invalid_pickup_type_produces_stm_009() {
        let file = make_file(
            vec!["trip_id", "arrival_time", "departure_time", "stop_id", "stop_sequence", "pickup_type"],
            vec![vec!["T1", "08:00:00", "08:00:00", "S1", "1", "9"]],
        );
        let (_, notices) = validate_stop_times(&file, None, false);
        assert!(notices.iter().any(|n| n.rule_id == "STM_009"));
    }

    #[test]
    fn invalid_timepoint_produces_stm_022() {
        let file = make_file(
            vec!["trip_id", "arrival_time", "departure_time", "stop_id", "stop_sequence", "timepoint"],
            vec![vec!["T1", "08:00:00", "08:00:00", "S1", "1", "5"]],
        );
        let (_, notices) = validate_stop_times(&file, None, false);
        assert!(notices.iter().any(|n| n.rule_id == "STM_022"));
    }

    #[test]
    fn timepoint_one_without_times_produces_stm_047() {
        let file = make_file(
            vec!["trip_id", "arrival_time", "departure_time", "stop_id", "stop_sequence", "timepoint"],
            vec![vec!["T1", "", "", "S1", "1", "1"]],
        );
        let (_, notices) = validate_stop_times(&file, None, false);
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
        let (_, notices) = validate_stop_times(&file, None, false);
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
        let (_, notices) = validate_stop_times(&file, None, false);
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
        let (_, notices) = validate_stop_times(&file, None, false);
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
        let (_, notices) = validate_stop_times(&file, None, false);
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
        let (_, notices) = validate_stop_times(&file, None, false);
        let ids: Vec<&str> = notices.iter().map(|n| n.rule_id.as_str()).collect();
        assert!(ids.contains(&"STM_051"), "Flex penceresi + pickup_type=0 → STM_051: {:?}", ids);
    }

    #[test]
    fn flex_window_forbidden_drop_off_type_produces_stm_052() {
        let file = make_file(
            vec!["trip_id", "stop_sequence", "location_id", "start_pickup_drop_off_window", "end_pickup_drop_off_window", "pickup_type", "drop_off_type"],
            vec![vec!["T1", "1", "Z1", "08:00:00", "18:00:00", "2", "0"]],
        );
        let (_, notices) = validate_stop_times(&file, None, false);
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
        let (_, notices) = validate_stop_times(&file, None, false);
        assert!(!notices.iter().any(|n| n.rule_id == "STM_051"),
            "Flex penceresi yok → pickup_type=0 STM_051 üretmemeli: {:?}",
            notices.iter().map(|n| &n.rule_id).collect::<Vec<_>>());
    }

    // ── STM_054/055: Flex penceresi + continuous_pickup/drop_off (satır kapsamlı) ──
    #[test]
    fn flex_window_forbidden_continuous_pickup_produces_stm_054() {
        let file = make_file(
            vec!["trip_id", "stop_sequence", "location_id", "start_pickup_drop_off_window", "end_pickup_drop_off_window", "pickup_type", "continuous_pickup"],
            vec![vec!["T1", "1", "Z1", "08:00:00", "18:00:00", "2", "0"]],
        );
        let (_, notices) = validate_stop_times(&file, None, false);
        let ids: Vec<&str> = notices.iter().map(|n| n.rule_id.as_str()).collect();
        assert!(ids.contains(&"STM_054"), "Flex penceresi + continuous_pickup=0 → STM_054: {ids:?}");
    }

    #[test]
    fn flex_window_forbidden_continuous_drop_off_produces_stm_055() {
        let file = make_file(
            vec!["trip_id", "stop_sequence", "location_id", "start_pickup_drop_off_window", "end_pickup_drop_off_window", "drop_off_type", "continuous_drop_off"],
            vec![vec!["T1", "1", "Z1", "08:00:00", "18:00:00", "2", "3"]],
        );
        let (_, notices) = validate_stop_times(&file, None, false);
        let ids: Vec<&str> = notices.iter().map(|n| n.rule_id.as_str()).collect();
        assert!(ids.contains(&"STM_055"), "Flex penceresi + continuous_drop_off=3 → STM_055: {ids:?}");
    }

    #[test]
    fn flex_window_continuous_one_or_empty_silent_for_stm_054_055() {
        // 1 → izinli; boş → izinli (örtük 1 sayılır, susulur).
        let file = make_file(
            vec!["trip_id", "stop_sequence", "location_id", "start_pickup_drop_off_window", "end_pickup_drop_off_window", "continuous_pickup", "continuous_drop_off"],
            vec![vec!["T1", "1", "Z1", "08:00:00", "18:00:00", "1", ""]],
        );
        let (_, notices) = validate_stop_times(&file, None, false);
        let ids: Vec<&str> = notices.iter().map(|n| n.rule_id.as_str()).collect();
        assert!(!ids.contains(&"STM_054") && !ids.contains(&"STM_055"),
            "continuous 1/boş → STM_054/055 üretmemeli: {ids:?}");
    }

    #[test]
    fn no_flex_window_continuous_zero_silent_for_stm_054() {
        // Pencere yoksa continuous_pickup=0 tümüyle geçerlidir (rota-kapsamlı koşul RTS_028'in işi).
        let file = make_file(
            vec!["trip_id", "arrival_time", "departure_time", "stop_id", "stop_sequence", "continuous_pickup"],
            vec![vec!["T1", "08:00:00", "08:00:00", "S1", "1", "0"]],
        );
        let (_, notices) = validate_stop_times(&file, None, false);
        let ids: Vec<&str> = notices.iter().map(|n| n.rule_id.as_str()).collect();
        assert!(!ids.contains(&"STM_054"), "Flex penceresi yok → STM_054 üretmemeli: {ids:?}");
    }

    #[test]
    fn invalid_continuous_enum_with_flex_window_only_produces_stm_018() {
        // 9 = enum dışı → STM_018'in işi; STM_054 çift emit ETMEMELİ.
        let file = make_file(
            vec!["trip_id", "stop_sequence", "location_id", "start_pickup_drop_off_window", "end_pickup_drop_off_window", "continuous_pickup"],
            vec![vec!["T1", "1", "Z1", "08:00:00", "18:00:00", "9"]],
        );
        let (_, notices) = validate_stop_times(&file, None, false);
        let ids: Vec<&str> = notices.iter().map(|n| n.rule_id.as_str()).collect();
        assert!(ids.contains(&"STM_018"), "continuous_pickup=9 → STM_018: {ids:?}");
        assert!(!ids.contains(&"STM_054"), "enum dışı değer STM_054'ü de tetiklememeli: {ids:?}");
    }

    #[test]
    fn stm018_above_threshold_aggregates_with_sorted_examples() {
        // Girdi ters sırada: feed özeti HashMap/chunk iterasyon sırasına bağlı kalmamalı.
        let ids: Vec<String> = (0..51).rev().map(|i| format!("T{i:02}")).collect();
        let rows: Vec<Vec<&str>> = ids.iter()
            .map(|id| vec![id.as_str(), id.as_str(), "9"])
            .collect();
        let file = make_file(vec!["trip_id", "stop_id", "continuous_pickup"], rows);
        let (_, notices) = validate_stop_times(&file, None, false);
        let stm018: Vec<_> = notices.iter().filter(|n| n.rule_id == "STM_018").collect();
        assert_eq!(stm018.len(), 1, "51 ihlal tek feed-özeti olmalı: {stm018:?}");
        assert_eq!(stm018[0].entity_type, EntityType::Feed);
        assert_eq!(stm018[0].observed_value.as_deref(), Some("51"));
        assert_eq!(
            stm018[0].details.as_ref().and_then(|d| d.get("affected_rows")).map(String::as_str),
            Some("51"),
        );
        assert_eq!(
            stm018[0].details.as_ref().and_then(|d| d.get("example_trips")).map(String::as_str),
            Some("T00, T01, T02, T03, T04"),
        );
    }

    #[test]
    fn stm018_at_or_below_threshold_keeps_pinpoint_notices() {
        let file = make_file(
            vec!["trip_id", "stop_id", "continuous_pickup"],
            vec![vec!["T2", "S2", "9"], vec!["T1", "S1", "9"]],
        );
        let (_, notices) = validate_stop_times(&file, None, false);
        let stm018: Vec<_> = notices.iter().filter(|n| n.rule_id == "STM_018").collect();
        assert_eq!(stm018.len(), 2, "düşük hacimde iki pinpoint notice korunmalı");
        assert!(stm018.iter().all(|n| n.entity_type == EntityType::Trip));
        assert!(stm018.iter().all(|n| n.details.is_none()));
    }

    #[test]
    fn stm019_above_threshold_aggregates_with_sorted_examples() {
        let ids: Vec<String> = (0..51).rev().map(|i| format!("T{i:02}")).collect();
        let rows: Vec<Vec<&str>> = ids.iter()
            .map(|id| vec![id.as_str(), id.as_str(), "9"])
            .collect();
        let file = make_file(vec!["trip_id", "stop_id", "continuous_drop_off"], rows);
        let (_, notices) = validate_stop_times(&file, None, false);
        let stm019: Vec<_> = notices.iter().filter(|n| n.rule_id == "STM_019").collect();
        assert_eq!(stm019.len(), 1, "51 ihlal tek feed-özeti olmalı: {stm019:?}");
        assert_eq!(stm019[0].entity_type, EntityType::Feed);
        assert_eq!(stm019[0].observed_value.as_deref(), Some("51"));
        assert_eq!(
            stm019[0].details.as_ref().and_then(|d| d.get("affected_rows")).map(String::as_str),
            Some("51"),
        );
        assert_eq!(
            stm019[0].details.as_ref().and_then(|d| d.get("example_trips")).map(String::as_str),
            Some("T00, T01, T02, T03, T04"),
        );
    }

    #[test]
    fn stm019_at_or_below_threshold_keeps_pinpoint_notices() {
        let file = make_file(
            vec!["trip_id", "stop_id", "continuous_drop_off"],
            vec![vec!["T2", "S2", "9"], vec!["T1", "S1", "9"]],
        );
        let (_, notices) = validate_stop_times(&file, None, false);
        let stm019: Vec<_> = notices.iter().filter(|n| n.rule_id == "STM_019").collect();
        assert_eq!(stm019.len(), 2, "düşük hacimde iki pinpoint notice korunmalı");
        assert!(stm019.iter().all(|n| n.entity_type == EntityType::Trip));
        assert!(stm019.iter().all(|n| n.details.is_none()));
    }

    // ── STM_036/032 + finalize çıktı alanları (K2 hashmap-birleştirme refactor güvenlik ağı) ──
    #[test]
    fn duplicate_stop_sequence_produces_stm_032() {
        let file = make_file(
            vec!["trip_id", "arrival_time", "departure_time", "stop_id", "stop_sequence"],
            vec![
                vec!["T1", "08:00:00", "08:00:00", "S1", "1"],
                vec!["T1", "08:10:00", "08:10:00", "S2", "1"], // aynı stop_sequence tekrar
            ],
        );
        let (_, notices) = validate_stop_times(&file, None, false);
        assert!(notices.iter().any(|n| n.rule_id == "STM_032"), "STM_032 bekleniyor: {:?}", notices);
    }

    #[test]
    fn stm032_above_threshold_aggregates_with_sorted_examples() {
        let ids: Vec<String> = (0..51).rev().map(|i| format!("T{i:02}")).collect();
        let mut rows = Vec::with_capacity(102);
        for id in &ids {
            rows.push(vec![id.as_str(), "08:00:00", "08:00:00", "S1", "1"]);
            rows.push(vec![id.as_str(), "08:10:00", "08:10:00", "S2", "1"]);
        }
        let file = make_file(
            vec!["trip_id", "arrival_time", "departure_time", "stop_id", "stop_sequence"],
            rows,
        );
        let (_, notices) = validate_stop_times(&file, None, false);
        let stm032: Vec<_> = notices.iter().filter(|n| n.rule_id == "STM_032").collect();
        assert_eq!(stm032.len(), 1, "51 ihlal tek feed-özeti olmalı: {stm032:?}");
        assert_eq!(stm032[0].entity_type, EntityType::Feed);
        assert_eq!(stm032[0].observed_value.as_deref(), Some("51"));
        assert_eq!(
            stm032[0].details.as_ref().and_then(|d| d.get("affected_rows")).map(String::as_str),
            Some("51"),
        );
        assert_eq!(
            stm032[0].details.as_ref().and_then(|d| d.get("example_trips")).map(String::as_str),
            Some("T00, T01, T02, T03, T04"),
        );
    }

    #[test]
    fn stm032_at_or_below_threshold_keeps_pinpoint_notices() {
        let file = make_file(
            vec!["trip_id", "arrival_time", "departure_time", "stop_id", "stop_sequence"],
            vec![
                vec!["T2", "08:00:00", "08:00:00", "S1", "1"],
                vec!["T2", "08:10:00", "08:10:00", "S2", "1"],
                vec!["T1", "09:00:00", "09:00:00", "S1", "1"],
                vec!["T1", "09:10:00", "09:10:00", "S2", "1"],
            ],
        );
        let (_, notices) = validate_stop_times(&file, None, false);
        let stm032: Vec<_> = notices.iter().filter(|n| n.rule_id == "STM_032").collect();
        assert_eq!(stm032.len(), 2, "düşük hacimde iki pinpoint notice korunmalı");
        assert!(stm032.iter().all(|n| n.entity_type == EntityType::Trip));
        assert!(stm032.iter().all(|n| n.details.is_none()));
    }

    /// #97 kabul matrisi: `stop_sequence` ARTIŞ hükmünü ölçen kural SPEC sınıfında olmalı,
    /// ve hükmü ihlal ETMEYEN iki meşru şekilde susmalı.
    #[test]
    fn stop_sequence_increase_provision_is_measured_by_a_spec_rule() {
        let meta = gtfs_rules::RULES.iter().find(|r| r.id == "STM_032")
            .expect("STM_032 registry'de olmalı");
        assert_eq!(meta.rule_class, gtfs_core::RuleClass::Spec,
            "artış hükmünü ölçen kural Spec sınıfında olmalı (#97)");
    }

    #[test]
    fn non_consecutive_increasing_stop_sequence_is_silent() {
        // Spec AÇIKÇA izin verir: "do not need to be consecutive" (1, 5, 9).
        let file = make_file(
            vec!["trip_id", "arrival_time", "departure_time", "stop_id", "stop_sequence"],
            vec![
                vec!["T1", "08:00:00", "08:00:00", "S1", "1"],
                vec!["T1", "08:10:00", "08:10:00", "S2", "5"],
                vec!["T1", "08:20:00", "08:20:00", "S3", "9"],
            ],
        );
        let (_, notices) = validate_stop_times(&file, None, false);
        assert!(!notices.iter().any(|n| n.rule_id == "STM_032"),
            "ardışık olmayan ama artan değerler STM_032 üretmemeli: {notices:?}");
    }

    #[test]
    fn file_order_decrease_with_increasing_values_produces_no_spec_finding() {
        // 🔑 #97'nin yanlış-pozitif kapısı: satırlar DOSYADA ters yazılmış (seq 2 önce),
        // ama trip'in değerleri artıyor → hüküm İHLAL EDİLMEMİŞTİR, Spec bulgusu ÇIKMAMALI.
        // (Dosya sırası sinyali STM_036'nın işidir ve o K6'da üretilir.)
        let file = make_file(
            vec!["trip_id", "arrival_time", "departure_time", "stop_id", "stop_sequence"],
            vec![
                vec!["T1", "08:10:00", "08:10:00", "S2", "2"],
                vec!["T1", "08:00:00", "08:00:00", "S1", "1"],
            ],
        );
        let (_, notices) = validate_stop_times(&file, None, false);
        assert!(!notices.iter().any(|n| n.rule_id == "STM_032"),
            "dosya sırası ters ama değerler artıyor → STM_032 çıkmamalı: {notices:?}");
    }

    #[test]
    fn non_increasing_shape_dist_produces_stm_056() {
        let cols = vec!["trip_id", "arrival_time", "departure_time", "stop_id", "stop_sequence", "shape_dist_traveled"];
        // eşit değer
        let file = make_file(cols.clone(), vec![
            vec!["T1", "08:00:00", "08:00:00", "S1", "1", "100.0"],
            vec!["T1", "08:10:00", "08:10:00", "S2", "2", "100.0"],
        ]);
        let (_, notices) = validate_stop_times(&file, None, false);
        assert!(notices.iter().any(|n| n.rule_id == "STM_056"), "eşit değer STM_056 üretmeli: {notices:?}");

        // azalan değer
        let file = make_file(cols.clone(), vec![
            vec!["T1", "08:00:00", "08:00:00", "S1", "1", "250.0"],
            vec!["T1", "08:10:00", "08:10:00", "S2", "2", "100.0"],
        ]);
        let (_, notices) = validate_stop_times(&file, None, false);
        assert!(notices.iter().any(|n| n.rule_id == "STM_056"), "azalan değer STM_056 üretmeli");
    }

    #[test]
    fn stm056_above_threshold_aggregates_with_sorted_examples() {
        let ids: Vec<String> = (0..51).rev().map(|i| format!("T{i:02}")).collect();
        let mut rows = Vec::with_capacity(102);
        for id in &ids {
            rows.push(vec![id.as_str(), "08:00:00", "08:00:00", "S1", "1", "100"]);
            rows.push(vec![id.as_str(), "08:10:00", "08:10:00", "S2", "2", "100"]);
        }
        let file = make_file(
            vec![
                "trip_id", "arrival_time", "departure_time", "stop_id", "stop_sequence",
                "shape_dist_traveled",
            ],
            rows,
        );
        let (_, notices) = validate_stop_times(&file, None, false);
        let stm056: Vec<_> = notices.iter().filter(|n| n.rule_id == "STM_056").collect();
        assert_eq!(stm056.len(), 1, "51 ihlal tek feed-özeti olmalı: {stm056:?}");
        assert_eq!(stm056[0].entity_type, EntityType::Feed);
        assert_eq!(stm056[0].observed_value.as_deref(), Some("51"));
        assert_eq!(
            stm056[0].details.as_ref().and_then(|d| d.get("affected_rows")).map(String::as_str),
            Some("51"),
        );
        assert_eq!(
            stm056[0].details.as_ref().and_then(|d| d.get("example_trips")).map(String::as_str),
            Some("T00, T01, T02, T03, T04"),
        );
    }

    #[test]
    fn stm056_at_or_below_threshold_keeps_pinpoint_notices() {
        let file = make_file(
            vec![
                "trip_id", "arrival_time", "departure_time", "stop_id", "stop_sequence",
                "shape_dist_traveled",
            ],
            vec![
                vec!["T2", "08:00:00", "08:00:00", "S1", "1", "100"],
                vec!["T2", "08:10:00", "08:10:00", "S2", "2", "100"],
                vec!["T1", "09:00:00", "09:00:00", "S1", "1", "250"],
                vec!["T1", "09:10:00", "09:10:00", "S2", "2", "100"],
            ],
        );
        let (_, notices) = validate_stop_times(&file, None, false);
        let stm056: Vec<_> = notices.iter().filter(|n| n.rule_id == "STM_056").collect();
        assert_eq!(stm056.len(), 2, "düşük hacimde iki pinpoint notice korunmalı");
        assert!(stm056.iter().all(|n| n.entity_type == EntityType::Trip));
        assert!(stm056.iter().all(|n| n.details.is_none()));
    }

    #[test]
    fn increasing_or_missing_shape_dist_no_stm_056() {
        let cols = vec!["trip_id", "arrival_time", "departure_time", "stop_id", "stop_sequence", "shape_dist_traveled"];
        // artan → temiz
        let file = make_file(cols.clone(), vec![
            vec!["T1", "08:00:00", "08:00:00", "S1", "1", "100.0"],
            vec!["T1", "08:10:00", "08:10:00", "S2", "2", "250.0"],
        ]);
        let (_, notices) = validate_stop_times(&file, None, false);
        assert!(!notices.iter().any(|n| n.rule_id == "STM_056"), "artan değer temiz olmalı");

        // biri eksik → karşılaştırma yapılmaz (STM_017 kapsamı)
        let file = make_file(cols, vec![
            vec!["T1", "08:00:00", "08:00:00", "S1", "1", "100.0"],
            vec!["T1", "08:10:00", "08:10:00", "S2", "2", ""],
        ]);
        let (_, notices) = validate_stop_times(&file, None, false);
        assert!(!notices.iter().any(|n| n.rule_id == "STM_056"), "eksik değerde STM_056 olmamalı");
    }

    #[test]
    fn out_of_order_stop_sequence_produces_stm_036() {
        let file = make_file(
            vec!["trip_id", "arrival_time", "departure_time", "stop_id", "stop_sequence"],
            vec![
                vec!["T1", "08:00:00", "08:00:00", "S1", "5"],
                vec!["T1", "08:10:00", "08:10:00", "S2", "2"], // 2 < 5 → dosya sırası bozuk
            ],
        );
        let (idx, notices) = validate_stop_times(&file, None, false);
        let stm036: Vec<_> = notices.iter().filter(|n| n.rule_id == "STM_036").collect();
        assert_eq!(stm036.len(), 1, "STM_036 trip başına TEK kez üretilmeli: {:?}", notices);
        assert_eq!(idx.unsorted_seq_trips.len(), 1, "STM_036 unsorted_seq_trips'e bir kayıt eklemeli");
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
        let (idx, _) = validate_stop_times(&file, None, false);
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
        assert_eq!(t1[0].stop_sequence(), Some(1));
        assert_eq!(t1[1].stop_sequence(), Some(2));
    }

    #[test]
    fn w1_inplace_grouping_orders_by_seq_with_file_order_tiebreak() {
        // #15 W1: trip'ler dosyada İÇ İÇE, sequence'ler bozuk, T1'de seq=1 İKİ kez.
        // In-place permütasyon: (a) trip'e göre grupla, (b) trip-içi sequence ASC,
        // (c) eşit sequence'te DOSYA sırası (determinizm), (d) trip_ranges doğru.
        let file = make_file(
            vec!["trip_id", "arrival_time", "departure_time", "stop_id", "stop_sequence"],
            vec![
                vec!["T2", "09:02:00", "09:02:00", "S_B", "2"], // satır 2  (T2 = idx 0)
                vec!["T1", "08:30:00", "08:30:00", "S_a", "3"], // satır 3  (T1 = idx 1)
                vec!["T2", "09:01:00", "09:01:00", "S_A", "1"], // satır 4
                vec!["T1", "08:00:00", "08:00:00", "S_c", "1"], // satır 5  (seq=1 #1)
                vec!["T1", "08:00:00", "08:00:00", "S_d", "1"], // satır 6  (seq=1 #2 dup)
                vec!["T1", "08:10:00", "08:10:00", "S_b", "2"], // satır 7
            ],
        );
        let (idx, _) = validate_stop_times(&file, None, false);
        assert_eq!(idx.total_rows, 6);
        assert_eq!(idx.trip_ranges.len(), 2);

        let t1: Vec<_> = idx.sorted_stops("T1").unwrap().iter()
            .map(|s| (idx.stop_id_of(s).to_string(), s.stop_sequence())).collect();
        assert_eq!(t1, vec![
            ("S_c".into(), Some(1)), ("S_d".into(), Some(1)),
            ("S_b".into(), Some(2)), ("S_a".into(), Some(3)),
        ], "T1: sequence ASC + eşit seq'te dosya sırası (S_c < S_d)");

        let t2: Vec<_> = idx.sorted_stops("T2").unwrap().iter()
            .map(|s| (idx.stop_id_of(s).to_string(), s.stop_sequence())).collect();
        assert_eq!(t2, vec![("S_A".into(), Some(1)), ("S_B".into(), Some(2))],
            "T2 ayrı grup, sequence ASC");
    }

    #[test]
    fn scope_key_is_trip_id() {
        let file = make_file(
            vec!["trip_id", "arrival_time", "departure_time", "stop_id", "stop_sequence"],
            vec![vec!["TRIP_X", "08:00:00", "08:00:00", "S1", ""]],
        );
        let (_, notices) = validate_stop_times(&file, None, false);
        let n = notices.iter().find(|n| n.rule_id == "STM_005").expect("STM_005 olmalı");
        assert_eq!(n.scope_key.as_deref(), Some("TRIP_X"), "scope_key trip_id olmalı");
    }

    #[test]
    fn only_arrival_time_produces_stm_034() {
        let file = make_file(
            vec!["trip_id", "arrival_time", "departure_time", "stop_id", "stop_sequence"],
            vec![vec!["T1", "08:00:00", "", "S1", "1"]],
        );
        let (_, notices) = validate_stop_times(&file, None, false);
        assert!(notices.iter().any(|n| n.rule_id == "STM_034"),
            "Yalnızca arrival_time dolu → STM_034 olmalı. Notices: {:?}", notices);
    }

    #[test]
    fn only_departure_time_produces_stm_034() {
        let file = make_file(
            vec!["trip_id", "arrival_time", "departure_time", "stop_id", "stop_sequence"],
            vec![vec!["T1", "", "08:00:00", "S1", "1"]],
        );
        let (_, notices) = validate_stop_times(&file, None, false);
        assert!(notices.iter().any(|n| n.rule_id == "STM_034"),
            "Yalnızca departure_time dolu → STM_034 olmalı. Notices: {:?}", notices);
    }

    #[test]
    fn both_times_empty_no_stm_034() {
        let file = make_file(
            vec!["trip_id", "arrival_time", "departure_time", "stop_id", "stop_sequence"],
            vec![vec!["T1", "", "", "S1", "1"]],
        );
        let (_, notices) = validate_stop_times(&file, None, false);
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
        let (_, notices) = validate_stop_times(&file, None, false);
        assert!(notices.iter().any(|n| n.rule_id == "STM_037"), "STM_037 bekleniyor: {:?}", notices);
    }

    #[test]
    fn stm_038_start_window_after_end_window() {
        let file = make_file(
            vec!["trip_id", "stop_id", "stop_sequence", "start_pickup_drop_off_window", "end_pickup_drop_off_window", "pickup_booking_rule_id"],
            vec![vec!["T1", "", "1", "10:00:00", "09:00:00", "BR1"]],
        );
        let (_, notices) = validate_stop_times(&file, None, false);
        assert!(notices.iter().any(|n| n.rule_id == "STM_038"), "STM_038 bekleniyor: {:?}", notices);
    }

    #[test]
    fn stm_038_equal_windows_are_reported() {
        // Sıfır uzunluklu pencere: MobilityData "strictly later" ister ve kural
        // Interop'tur. Kod 2026-08-18'e kadar yalnız `>` bakıyordu; `mdb-2741`'de
        // MD 38 bulgu verirken bizde 0'dı.
        let file = make_file(
            vec!["trip_id", "stop_id", "stop_sequence", "start_pickup_drop_off_window", "end_pickup_drop_off_window", "pickup_booking_rule_id"],
            vec![vec!["T1", "", "1", "13:09:00", "13:09:00", "BR1"]],
        );
        let (_, notices) = validate_stop_times(&file, None, false);
        assert!(notices.iter().any(|n| n.rule_id == "STM_038"),
            "Eşit pencerelerde STM_038 bekleniyor: {:?}", notices);
    }

    #[test]
    fn stm_038_ordered_windows_are_clean() {
        let file = make_file(
            vec!["trip_id", "stop_id", "stop_sequence", "start_pickup_drop_off_window", "end_pickup_drop_off_window", "pickup_booking_rule_id"],
            vec![vec!["T1", "", "1", "09:00:00", "17:00:00", "BR1"]],
        );
        let (_, notices) = validate_stop_times(&file, None, false);
        assert!(!notices.iter().any(|n| n.rule_id == "STM_038"),
            "start < end temiz olmalı: {:?}", notices);
    }

    #[test]
    fn stm_058_malformed_window_is_reported_not_swallowed() {
        // Eskiden `.ok().flatten()` hatayı yutuyordu: hiç bulgu çıkmıyor, değer kayboluyor
        // ve STM_038 karşılaştıracak bir şey bulamadığı için o da susuyordu.
        let file = make_file(
            vec!["trip_id", "stop_id", "stop_sequence", "start_pickup_drop_off_window", "end_pickup_drop_off_window"],
            vec![vec!["T1", "", "1", "9am", "10:00:00"]],
        );
        let (index, notices) = validate_stop_times(&file, None, false);
        assert!(notices.iter().any(|n| n.rule_id == "STM_058"
                && n.field.as_deref() == Some("start_pickup_drop_off_window")),
            "STM_058 bekleniyor ve alanı start_pickup_drop_off_window olmalı: {:?}", notices);
        assert!(index.flex_map.values().all(|f| f.start_pickup_drop_off_window.is_none()),
            "parse edilemeyen değer kayda GİRMEMELİ");
    }

    #[test]
    fn stm_058_silent_for_valid_windows() {
        let file = make_file(
            vec!["trip_id", "stop_id", "stop_sequence", "start_pickup_drop_off_window", "end_pickup_drop_off_window"],
            vec![vec!["T1", "", "1", "09:00:00", "25:30:00"]],  // 24:00 üstü GTFS'te geçerli
        );
        let (_, notices) = validate_stop_times(&file, None, false);
        assert!(!notices.iter().any(|n| n.rule_id == "STM_058"),
            "geçerli pencere STM_058 üretmemeli: {:?}", notices);
    }

    #[test]
    fn stm_039_location_id_without_window() {
        let file = make_file(
            vec!["trip_id", "stop_sequence", "location_id"],
            vec![vec!["T1", "1", "LOC1"]],
        );
        let (_, notices) = validate_stop_times(&file, None, false);
        assert!(notices.iter().any(|n| n.rule_id == "STM_039"), "STM_039 bekleniyor: {:?}", notices);
    }

    #[test]
    fn stm_040_flex_window_without_booking_rule() {
        let file = make_file(
            vec!["trip_id", "stop_sequence", "start_pickup_drop_off_window", "end_pickup_drop_off_window"],
            vec![vec!["T1", "1", "08:00:00", "10:00:00"]],
        );
        let (_, notices) = validate_stop_times(&file, None, false);
        assert!(notices.iter().any(|n| n.rule_id == "STM_040"), "STM_040 bekleniyor: {:?}", notices);
    }

    #[test]
    fn stm_041_stop_id_and_location_id_conflict() {
        let file = make_file(
            vec!["trip_id", "stop_id", "stop_sequence", "location_id", "start_pickup_drop_off_window", "end_pickup_drop_off_window", "pickup_booking_rule_id"],
            vec![vec!["T1", "S1", "1", "LOC1", "08:00:00", "10:00:00", "BR1"]],
        );
        let (_, notices) = validate_stop_times(&file, None, false);
        assert!(notices.iter().any(|n| n.rule_id == "STM_041"), "STM_041 bekleniyor: {:?}", notices);
    }

    #[test]
    fn valid_flex_stop_time_no_notices() {
        let file = make_file(
            vec!["trip_id", "stop_sequence", "location_id", "start_pickup_drop_off_window", "end_pickup_drop_off_window", "pickup_booking_rule_id"],
            vec![vec!["T1", "1", "LOC1", "08:00:00", "10:00:00", "BR1"]],
        );
        let (_, notices) = validate_stop_times(&file, None, false);
        let flex_notices: Vec<_> = notices.iter().filter(|n| matches!(n.rule_id.as_str(), "STM_037"|"STM_038"|"STM_039"|"STM_040"|"STM_041")).collect();
        assert!(flex_notices.is_empty(), "Geçerli Flex stop_time için Flex notice olmamalı: {:?}", flex_notices);
    }
    #[test]
    fn stm_059_flags_on_demand_type_without_booking_rule() {
        let file = make_file(
            vec!["trip_id", "stop_id", "stop_sequence", "pickup_type", "drop_off_type", "pickup_booking_rule_id"],
            vec![
                vec!["T1", "S1", "1", "2", "0", ""],        // pickup_type=2, kural yok → say
                vec!["T2", "S1", "1", "0", "2", ""],        // drop_off_type=2, kural yok → say
                vec!["T3", "S1", "1", "2", "0", "BR1"],     // kural var → sessiz
                vec!["T4", "S1", "1", "0", "0", ""],        // enum 2 değil → sessiz
            ],
        );
        let (_, notices) = validate_stop_times(&file, None, true);
        let hits: Vec<_> = notices.iter().filter(|n| n.rule_id == "STM_059").collect();
        assert_eq!(hits.len(), 2, "yalnız iki satır: {hits:?}");
    }

    #[test]
    fn stm_059_silent_without_booking_rules_file() {
        // booking_rules.txt yoksa bir id yazmak FK'yı kırardı (BKR_017) → tavsiye uygulanamaz.
        // Ölçümde bu kapı olmadan 97.355 satır çıkmıştı; kapıyla 27.
        let file = make_file(
            vec!["trip_id", "stop_id", "stop_sequence", "pickup_type"],
            vec![vec!["T1", "S1", "1", "2"]],
        );
        let (_, notices) = validate_stop_times(&file, None, false);
        assert!(!notices.iter().any(|n| n.rule_id == "STM_059"), "{notices:?}");
    }

    #[test]
    fn stm_059_and_stm_040_measure_different_rows() {
        // STM_040 Flex PENCERESİNE bakar, STM_059 talep-üzerine ENUM DEĞERİNE.
        // Penceresiz dial-a-ride satırı STM_040'ta sessizdi — boşluk buydu.
        let file = make_file(
            vec!["trip_id", "stop_id", "stop_sequence", "pickup_type", "start_pickup_drop_off_window", "end_pickup_drop_off_window"],
            vec![
                vec!["T1", "S1", "1", "2", "", ""],                    // yalnız STM_059
                vec!["T2", "", "1", "0", "08:00:00", "10:00:00"],      // yalnız STM_040
            ],
        );
        let (_, notices) = validate_stop_times(&file, None, true);
        let s59: Vec<_> = notices.iter().filter(|n| n.rule_id == "STM_059").map(|n| n.entity_id.as_deref()).collect();
        let s40: Vec<_> = notices.iter().filter(|n| n.rule_id == "STM_040").map(|n| n.entity_id.as_deref()).collect();
        assert_eq!(s59, vec![Some("T1")], "STM_059 yalnız T1");
        assert_eq!(s40, vec![Some("T2")], "STM_040 yalnız T2");
    }

}
