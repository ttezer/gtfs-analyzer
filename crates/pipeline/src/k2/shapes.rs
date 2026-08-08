use std::borrow::Cow;
use std::collections::{HashMap, HashSet};

use gtfs_core::{EntityType, Severity};

use super::common::{get_col_raw, get_col, make_k2_notice, parse_f64_col, parse_u32_col};
use super::stop_times::{next_csv_record, ZipCsvReader};
use crate::k1_parse::RawFile;

/// shape_id intern tablosu — `TripInternTable` ile aynı desen.
///
/// shapes.txt'te her nokta kendi `shape_id`'sini taşıyordu: Entur'da 40,9M `String`,
/// ama yalnız 28.604 farklı değer (ortalama ~48 karakter, SmolStr'ın 23 baytlık inline
/// sınırına sığmıyor → hepsi ayrı heap tahsisi ≈ 2,4 GB). Artık her nokta 4 baytlık
/// indeks tutuyor, metin bir kez bu tabloda duruyor.
///
/// 0. indeks boş/eksik shape_id demek (trips desenindeki gibi).
#[derive(Debug, Clone)]
pub struct ShapeInternTable {
    ids: Vec<smol_str::SmolStr>,
}

impl Default for ShapeInternTable {
    fn default() -> Self { Self::new() }
}

impl ShapeInternTable {
    pub fn new() -> Self {
        Self { ids: vec![smol_str::SmolStr::default()] }
    }
    /// Noktanın shape_id metni; bilinmeyen indeks veya boş değer için "".
    #[inline]
    pub fn id<'a>(&'a self, sp: &ShapePointRecord) -> &'a str {
        self.ids.get(sp.shape_idx as usize).map(smol_str::SmolStr::as_str).unwrap_or("")
    }
    /// İndeksten metin (nokta elde değilken).
    #[inline]
    pub fn id_at(&self, idx: u32) -> &str {
        self.ids.get(idx as usize).map(smol_str::SmolStr::as_str).unwrap_or("")
    }
    pub fn len(&self) -> usize { self.ids.len() }
    pub fn is_empty(&self) -> bool { self.ids.len() <= 1 }

    /// Metni intern eder ve indeksini döndürür. Boş metin → 0.
    /// Parse yolu kendi hash haritasını kullanır (40,9M satırda lineer arama olmaz);
    /// bu metot küçük tablolar ve test kurulumu içindir.
    pub fn intern(&mut self, id: &str) -> u32 {
        if id.is_empty() { return 0; }
        if let Some(pos) = self.ids.iter().position(|x| x == id) {
            return pos as u32;
        }
        self.ids.push(smol_str::SmolStr::from(id));
        (self.ids.len() - 1) as u32
    }
}

/// Tek bir shapes.txt noktası.
///
/// Bellek: Entur (mdb-1078) gibi feed'lerde 40,9M kayıt tutulur, yani her bayt ×40,9M.
/// `Option<f64>`/`Option<u32>` sarmalayıcıları niche taşımadığı için alan başına 8 bayt
/// israf ediyordu; alanlar `CompactStopTime` desenindeki gibi sentinel'e çevrildi ve
/// `Option` API'si accessor'larda korunuyor. `shape_id` ise [`ShapeInternTable`] indeksi.
///
/// `shape_dist_traveled` BİLİNÇLİ olarak f64 kaldı (`CompactStopTime` orada f32 kullanır):
/// SHP_005 monotonluğa TAM karşılaştırmayla bakıyor (#94) ve K4::stm_shape_dist bu değeri
/// stop_times'ınkiyle karşılaştırıyor — 40,9M yoğun noktada f32 yuvarlaması komşu değerleri
/// birbirine çökertip gerçek artışları/azalmaları yutabilirdi.
#[derive(Debug, Clone)]
pub struct ShapePointRecord {
    /// `ShapeInternTable` indeksi; 0 = boş/eksik shape_id
    shape_idx: u32,
    /// NaN = alan yok/parse edilemedi
    lat: f64,
    /// NaN = alan yok/parse edilemedi
    lon: f64,
    /// u32::MAX = alan yok/parse edilemedi
    seq: u32,
    /// NaN = alan yok/parse edilemedi
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
    pub fn shape_idx(&self) -> u32 { self.shape_idx }

    #[inline]
    pub fn shape_pt_lat(&self) -> Option<f64> {
        if self.lat.is_nan() { None } else { Some(self.lat) }
    }
    #[inline]
    pub fn shape_pt_lon(&self) -> Option<f64> {
        if self.lon.is_nan() { None } else { Some(self.lon) }
    }
    #[inline]
    pub fn shape_pt_sequence(&self) -> Option<u32> {
        if self.seq == u32::MAX { None } else { Some(self.seq) }
    }
    #[inline]
    pub fn shape_dist_traveled(&self) -> Option<f64> {
        if self.dist.is_nan() { None } else { Some(self.dist) }
    }
    /// Notice API'si u64 bekler.
    #[inline]
    pub fn line_u64(&self) -> u64 { self.line as u64 }
}

// Boyut guard: 40B'ı aşarsa derleme kırılır.
// 88B (Option'lı) → 56B (sentinel) → 40B (shape_id intern). ×40,9M nokta ≈ 1,9 GB struct,
// artı ortadan kalkan 40,9M ayrı shape_id tahsisi ≈ 2,4 GB.
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
            shape_id:           pos("shape_id"),
            shape_pt_lat:       pos("shape_pt_lat"),
            shape_pt_lon:       pos("shape_pt_lon"),
            shape_pt_sequence:  pos("shape_pt_sequence"),
            shape_dist_traveled: pos("shape_dist_traveled"),
        }
    }
}

pub fn validate_shapes(file: &RawFile, zip_bytes: Option<&[u8]>) -> (Vec<ShapePointRecord>, ShapeInternTable, Vec<gtfs_core::Notice>) {
    // #15 W3: shapes.txt K1'de stream edilir (RawFile.rows boş, gövde raw_text'te). Burada
    // streaming parse edilir; K1'in per-satır generic notice'ları (ARC_012/018/021, DQ_016)
    // bu geçişe taşındı (stop_times deseni — aynı line/severity/rule davranışı). Eski rows
    // yolu test/legacy fallback olarak korunur.
    let mut notices = Vec::new();
    // #15: records Vec'ini önceden boyutlandır — doubling realloc'un geçici 2× peak'i
    // shape-ağır feed'lerde ~500 MB tepe yaratıyordu (K1 stream'i düşünce açığa çıktı).
    //
    // Eski tahmin SABİT "satır ~30 bayt" varsayıyordu. Entur'da (mdb-1078) gerçek ortalama
    // 67,9 bayt çıktı — uzun NeTEx shape_id'leri yüzünden — ve tahmin 2,26× şişti:
    // 92,7M slot ayrıldı, 40,9M kullanıldı, ~2,7 GB hiç dokunulmadan duruyordu.
    // Böleni büyütmek çözüm DEĞİL: kısa shape_id'li feed'lerde bu kez tahmin düşük kalır,
    // Vec doubling'e girer ve kopyalama anında eski+yeni tampon birlikte canlı olur (tepe artar).
    // Bunun yerine ilk SAMPLE_ROWS satırın GERÇEK uzunluğundan ortalama ölçülüp kapasite
    // bir kez ondan türetilir; feed'e uyarlanır. Yalnız KAPASİTE — eleman değeri/sırası etkilenmez.
    const SAMPLE_ROWS: usize = 4096;
    let stream_bytes: Option<usize> = zip_bytes
        .map(|_| file.bytes as usize)
        .or_else(|| file.raw_text.as_ref().map(|t| t.len()));
    let mut sampled_bytes: usize = 0;
    let mut capacity_set = false;
    let mut records: Vec<ShapePointRecord> = match stream_bytes {
        // Stream yolu: örnekleme bitince tek seferde reserve edilir.
        Some(_) => Vec::new(),
        // rows fallback: satır sayısı zaten kesin.
        None => Vec::with_capacity(file.rows.len()),
    };
    let mut counter = 0u32;
    // ARC_033: ihlali okuyucular görür, satır numarasını döngü ekler (bkz. trips.rs).
    let mut rfc_acc = crate::k1_parse::Rfc4180Acc::default();
    let mut scan = super::stop_times::CsvScanState::default();
    let mut zip_unclosed = false;

    let cols = Cols::from_headers(&file.headers);
    let header_count = file.headers.len();
    let mut arc021_fired = false;
    let mut arc030_fired = false;
    // SHP_008 anahtarı artık intern indeksi: eskiden her satırda shape_id KLONLANIYORDU
    // (40,9M String tahsisi yalnız bu map için).
    let mut seen_seq_by_shape: HashMap<u32, HashSet<u32>> = HashMap::new();
    // shape_id intern durumu. Noktalar aynı shape için ardışık geldiğinden son id
    // hatırlanır ve satırların ~%99,9'unda hash araması tamamen atlanır.
    let mut intern = ShapeInternTable::new();
    let mut intern_map: rustc_hash::FxHashMap<smol_str::SmolStr, u32> = Default::default();
    let mut last_shape: Option<(smol_str::SmolStr, u32)> = None;
    let mut total_rows: usize = 0;
    let mut dq016 = crate::k1_parse::Dq016Acc::default();

    {
        // Satır işleyici — hem stream (raw_text) hem rows fallback yolundan çağrılır.
        let mut process = |row: &[Cow<'_, str>], line: u64| {
            // ── Taşınan generic dosya/satır-seviye notice'lar (K1'den) ──
            // ARC_012: sütun sayısı tutarsız
            if let Some((msg, tip, observed, is_info)) =
                crate::k1_parse::arc012_check(&file.name, line, row.len(), header_count)
            {
                let mut n = make_k2_notice(
                    &mut counter, "ARC_012", EntityType::File, Some(file.name.clone()),
                    None, &file.name, Some(line), None,
                    Some(observed), None,
                    msg, tip,
                );
                if is_info {
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

            // Kapasite örneklemesi (yalnız stream yolu): ilk SAMPLE_ROWS satırın gerçek
            // uzunluğundan ortalama çıkarılır, kalan kapasite TEK seferde ayrılır. Böylece
            // ne sabit-varsayım şişmesi ne de doubling'in geçici 2× tepesi oluşur.
            // %5 pay bırakılır: tahmin biraz düşük kalırsa realloc'a düşmeyelim.
            if !capacity_set {
                if let Some(total) = stream_bytes {
                    // alan uzunlukları + ayraç/satır sonu ≈ ham satır baytı
                    sampled_bytes += row.iter().map(|v| v.len()).sum::<usize>() + row.len();
                    if total_rows >= SAMPLE_ROWS {
                        let avg = (sampled_bytes / total_rows).max(1);
                        let est = (total / avg).saturating_mul(105) / 100;
                        records.reserve(est.saturating_sub(records.len()));
                        capacity_set = true;
                    }
                }
            }

            // ARC_021: yazdırılamaz/sorunlu karakter — dosya başına bir kez
            if !arc021_fired {
                if let Some(cp) = crate::k1_parse::arc021_bad_char(row.iter().map(|v| v.as_ref())) {
                    arc021_fired = true;
                    notices.push(make_k2_notice(
                        &mut counter, "ARC_021", EntityType::File, Some(file.name.clone()),
                        None, &file.name, Some(line), None,
                        Some(format!("U+{cp:04X}")), None,
                        crate::k1_parse::arc021_message(&file.name, cp),
                        crate::k1_parse::ARC021_REMEDIATION,
                    ));
                }
            }

            // ARC_030: alan değerinde sekme/satır sonu — dosya başına bir kez
            if !arc030_fired {
                if let Some(cp) = crate::k1_parse::arc030_bad_whitespace(row.iter().map(|v| v.as_ref())) {
                    arc030_fired = true;
                    notices.push(make_k2_notice(
                        &mut counter, "ARC_030", EntityType::File, Some(file.name.clone()),
                        None, &file.name, Some(line), None,
                        Some(format!("U+{cp:04X}")), None,
                        crate::k1_parse::arc030_message(&file.name, cp),
                        crate::k1_parse::ARC030_REMEDIATION,
                    ));
                }
            }

            // DQ_016: değerlerde baştaki/sondaki boşluk
            // DQ_016: dosya-seviyesi birikim; emit döngü sonrası TEK özet (patlama önlemi).
            dq016.observe(line, row.iter().map(|v| v.as_ref()), &file.headers);

            // ── Shape-özel kurallar ──
            let shape_id = get_col_raw(row, cols.shape_id);
            // Intern: metin bir kez saklanır, nokta 4 baytlık indeks taşır. Ardışık aynı
            // shape_id'de (normal durum) hash araması da tahsis de yapılmaz.
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
            // entity_id yalnız notice üretilirken gerekir; eskiden HER satırda klonlanıyordu.
            let entity_id = || (!shape_id.is_empty()).then(|| shape_id.to_string());

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
            let shape_pt_sequence = match parse_u32_col(seq_raw) {
                Ok(v) => {
                    if v.is_none() && file.headers.iter().any(|h| h == "shape_pt_sequence") {
                        notices.push(make_k2_notice(
                            &mut counter, "SHP_004", EntityType::Shape, entity_id(),
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
                        &mut counter, "SHP_004", EntityType::Shape, entity_id(),
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
                if !seen_seq_by_shape.entry(shape_idx).or_default().insert(seq) {
                    notices.push(make_k2_notice(
                        &mut counter, "SHP_008", EntityType::Shape, entity_id(),
                        None, &file.name, Some(line), Some("shape_pt_sequence"),
                        Some(seq.to_string()), None,
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
                            &mut counter, "SHP_002", EntityType::Shape, entity_id(),
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
                            &mut counter, "SHP_002", EntityType::Shape, entity_id(),
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
                        &mut counter, "SHP_002", EntityType::Shape, entity_id(),
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
            let shape_pt_lon = match parse_f64_col(lon_raw) {
                Ok(None) => {
                    if file.headers.iter().any(|h| h == "shape_pt_lon") {
                        notices.push(make_k2_notice(
                            &mut counter, "SHP_003", EntityType::Shape, entity_id(),
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
                            &mut counter, "SHP_003", EntityType::Shape, entity_id(),
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
                        &mut counter, "SHP_003", EntityType::Shape, entity_id(),
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
                    // Sayı OLMAYAN değer eskiden sessizce düşüyordu: aralık dışı sayı SHP_021 üretirken
                    // "abc" hiçbir bulgu vermiyordu. Aynı olgunun iki dalı → aynı kural (PTH_027 emsali).
                    notices.push(make_k2_notice(
                        &mut counter, "SHP_021", EntityType::Shape, entity_id(),
                        None, &file.name, Some(line), Some("shape_dist_traveled"),
                        Some(sdt_raw.to_string()), Some(">= 0".to_string()),
                        format!("shape_dist_traveled '{}' sayı olarak okunamıyor.", sdt_raw),
                        "shape_dist_traveled alanını sıfır veya pozitif bir değere ayarlayın.",
                    ));
                    None
                }
            };

            // SHP_005 (shape_dist_traveled monotonluğu) K5'e taşındı: dosya satır sırası DEĞİL,
            // shape_pt_sequence-sıralı noktalar üzerinde kontrol edilir. Dosya sırası ≠ sequence
            // olan geçerli feed'lerde eski dosya-sırası kontrolü sahte "azalma" (FP) üretiyordu.

            records.push(ShapePointRecord::new(shape_idx, shape_pt_lat, shape_pt_lon, shape_pt_sequence, shape_dist_traveled, line as u32));
        };

        // ── Sürücü: zip_bytes → ZIP stream; raw_text → string stream; rows → fallback ──
        // ZIP yolu #15/#38 deseni: gövde K1'de belleğe ALINMAZ, burada satır satır okunur.
        // shapes.txt bazı feed'lerde tek başına GB'larca (mdb-1078: 2,65 GB) — K1'de
        // raw_text olarak tutulması tepe belleği ikiye katlıyordu.
        if let Some(zb) = zip_bytes {
            let cursor = std::io::Cursor::new(zb);
            match zip::ZipArchive::new(cursor) {
                Err(_) => {}
                Ok(mut archive) => {
                    if let Ok(entry) = archive.by_name(&file.name) {
                        let mut rdr = ZipCsvReader::new(entry);
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
                            let row: Vec<Cow<'_, str>> = fields.iter()
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
            // Fallback (test/legacy): önceden parse edilmiş rows üzerinden.
            for (row_idx, row) in file.rows.iter().enumerate() {
                let cow_row: Vec<Cow<'_, str>> =
                    row.iter().map(|s| Cow::Borrowed(s.as_str())).collect();
                process(&cow_row, (row_idx + 2) as u64);
            }
        }
    }

    // ARC_013: akış gövdesinde kapanmamış tırnak (issue #84) — DOSYA başına tek notice.
    if scan.unclosed || zip_unclosed {
        notices.push(super::common::arc013_unclosed_stream(&file.name, &mut counter));
    }

    // ARC_033: DOSYA başına TEK özet.
    if let Some(n) = super::common::arc033_summary(&rfc_acc, &file.name, &mut counter) {
        notices.push(n);
    }

    // ⚠️ ARC_022 (satır sayısı limiti) BURADA DEĞİL: issue #75'te `k2/mod.rs`'teki tek
    // geçişe toplandı. Kopya denetim, kapsamın sessizce eksik kalmasına yol açmıştı —
    // `trips.txt` ve `calendar_dates.txt` hiçbir kopyaya sahip değildi.

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
        let (records, _interns, notices) = validate_shapes(&file, None);
        assert_eq!(records.len(), 1);
        assert!(notices.is_empty(), "Geçerli shape noktası notice üretmemeli: {:?}", notices);
    }

    #[test]
    fn lat_out_of_range_produces_shp_002() {
        let file = make_file(
            vec!["shape_id", "shape_pt_lat", "shape_pt_lon", "shape_pt_sequence"],
            vec![vec!["SHP1", "999.0", "29.0", "1"]],
        );
        let (_, _interns, notices) = validate_shapes(&file, None);
        assert!(notices.iter().any(|n| n.rule_id == "SHP_002"));
    }

    #[test]
    fn lon_out_of_range_produces_shp_003() {
        let file = make_file(
            vec!["shape_id", "shape_pt_lat", "shape_pt_lon", "shape_pt_sequence"],
            vec![vec!["SHP1", "41.0", "999.0", "1"]],
        );
        let (_, _interns, notices) = validate_shapes(&file, None);
        assert!(notices.iter().any(|n| n.rule_id == "SHP_003"));
    }

    #[test]
    fn missing_sequence_produces_shp_004() {
        let file = make_file(
            vec!["shape_id", "shape_pt_lat", "shape_pt_lon", "shape_pt_sequence"],
            vec![vec!["SHP1", "41.0", "29.0", ""]],
        );
        let (_, _interns, notices) = validate_shapes(&file, None);
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
        let (rec_a, _interns, not_a) = validate_shapes(&make_file(headers.clone(), rows.clone()), None);

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
        let (rec_b, _interns, not_b) = validate_shapes(&file_stream, None);

        assert_eq!(rec_a.len(), rec_b.len(), "record sayısı iki yolda aynı");
        let lines_a: Vec<u64> = rec_a.iter().map(|r| r.line_u64()).collect();
        let lines_b: Vec<u64> = rec_b.iter().map(|r| r.line_u64()).collect();
        assert_eq!(lines_a, lines_b, "line numaraları iki yolda aynı (2,3,4)");
        let mut rules_a: Vec<&str> = not_a.iter().map(|n| n.rule_id.as_str()).collect();
        let mut rules_b: Vec<&str> = not_b.iter().map(|n| n.rule_id.as_str()).collect();
        rules_a.sort();
        rules_b.sort();
        assert_eq!(rules_a, rules_b, "iki yol aynı notice kümesi");
        assert!(rules_b.contains(&"SHP_002") && rules_b.contains(&"SHP_008"));
    }

    /// Kapasite tahmini feed'in GERÇEK satır uzunluğuna uymalı.
    /// Regresyon: sabit "satır ~30 bayt" varsayımı Entur'da (uzun NeTEx shape_id'leri,
    /// gerçek ortalama 67,9 B) 2,26× fazla ayırıyor ve ~2,7 GB'ı hiç kullanmadan tutuyordu.
    fn stream_file(text: String) -> RawFile {
        RawFile {
            name: "shapes.txt".to_string(),
            headers: vec!["shape_id", "shape_pt_lat", "shape_pt_lon", "shape_pt_sequence"]
                .into_iter().map(str::to_string).collect(),
            rows: Vec::new(),
            bytes: text.len() as u32,
            raw_text: Some(text),
        }
    }

    #[test]
    fn capacity_tracks_real_row_length_for_long_shape_ids() {
        let n = 20_000;
        let mut text = String::from("shape_id,shape_pt_lat,shape_pt_lon,shape_pt_sequence\n");
        for i in 0..n {
            // ~70 bayt/satır — eski sabit varsayımın (30 B) iki katından fazlası
            text.push_str(&format!("RUT:JourneyPattern:{i:08}:long-netex-id,59.91,10.75,{i}\n"));
        }
        let (records, _interns, _) = validate_shapes(&stream_file(text), None);
        assert_eq!(records.len(), n, "tüm satırlar okunmalı");
        assert!(
            records.capacity() < records.len() * 3 / 2,
            "kapasite {} / satır {} — sabit-varsayım şişmesi geri gelmiş",
            records.capacity(), records.len()
        );
    }

    #[test]
    fn capacity_does_not_starve_for_short_shape_ids() {
        // Karşıt uç: kısa id'de tahmin DÜŞÜK kalmamalı, yoksa Vec doubling'e girip
        // kopyalama anında eski+yeni tamponu birlikte canlı tutar (tepe artar).
        let n = 20_000;
        let mut text = String::from("shape_id,shape_pt_lat,shape_pt_lon,shape_pt_sequence\n");
        for i in 0..n {
            text.push_str(&format!("S{i},41.0,29.0,{i}\n"));
        }
        let (records, _interns, _) = validate_shapes(&stream_file(text), None);
        assert_eq!(records.len(), n);
        assert!(
            records.capacity() >= records.len(),
            "kapasite {} satır {} altına düşmüş", records.capacity(), records.len()
        );
    }
}

