use std::collections::{HashMap, HashSet};
use std::io::Read;

use crate::decompress_guard::{GuardedReader, DEFAULT_DECOMPRESSION_LIMITS};
use gtfs_core::{EntityType, FatalCode, FatalError, Notice, Severity};
use smol_str::SmolStr;

/// Debug-only K1 izleme. Release WASM derlemesinde (debug_assertions kapalı) ve
/// native'de tamamen derlenip çıkarılır — sıfır runtime maliyeti, hiçbir Notice
/// veya akış etkisi yok. (Profilde DevTools açıkken WASM stack sembolizasyonu
/// her console çağrısını felç edici yavaşlattığı için kapatıldı.)
macro_rules! k1dbg {
    ($($arg:tt)*) => {{
        #[cfg(all(target_family = "wasm", debug_assertions))]
        web_sys::console::log_1(&format!($($arg)*).into());
    }};
}

// ── GTFS dosya listeleri ──────────────────────────────────────────────────────

const REQUIRED_FILES: &[&str] = &[
    "agency.txt", "stops.txt", "routes.txt", "trips.txt", "stop_times.txt",
];

const CALENDAR_FILES: &[&str] = &["calendar.txt", "calendar_dates.txt"];

pub(crate) const KNOWN_FILES: &[&str] = &[
    "agency.txt", "stops.txt", "routes.txt", "trips.txt", "stop_times.txt",
    "calendar.txt", "calendar_dates.txt", "shapes.txt", "frequencies.txt",
    "transfers.txt", "fare_attributes.txt", "fare_rules.txt",
    "pathways.txt", "levels.txt", "feed_info.txt", "translations.txt", "attributions.txt",
    "route_networks.txt",
    // Fares v2
    "areas.txt", "stop_areas.txt", "networks.txt",
    "rider_categories.txt", "fare_media.txt", "fare_products.txt",
    "fare_leg_rules.txt", "fare_leg_join_rules.txt", "fare_transfer_rules.txt", "timeframes.txt",
    // Flex
    "booking_rules.txt", "location_groups.txt", "location_group_stops.txt",
    // GTFS-JP uzantıları (Japonya standardı)
    "agency_jp.txt", "routes_jp.txt", "office_jp.txt",
];

// ── Tipler ────────────────────────────────────────────────────────────────────

/// Ham CSV verisi: başlıklar + satırlar. Tip/enum kontrolü K2'de yapılır.
#[derive(Debug, Default)]
pub struct RawFile {
    pub name: String,
    pub headers: Vec<String>,
    /// Her satır, başlıklarla aynı indekste hizalanmış ham string değerleri taşır.
    /// SmolStr: ≤22 bayt alanlar inline saklanır (heap alloc yok); daha uzunlar Arc<str>.
    ///
    /// NOT: Çok büyük dosyalarda (stop_times.txt) bellek için bu alan BOŞ bırakılır;
    /// ham gövde `raw_text`'te tutulur ve K2 tarafından streaming parse edilir (OOM fix Plan A).
    pub rows: Vec<Vec<SmolStr>>,
    /// Sıkıştırılmamış dosya boyutu (bayt). Büyük dosyalarda READ_LIMIT ile kırpılabilir.
    pub bytes: u32,
    /// Streaming parse edilen dosyalar (stop_times.txt) için BOM'suz, UTF-8 doğrulanmış
    /// ham metin (başlık dahil). `rows` boş kaldığında K2 bunu satır satır işler.
    /// Diğer tüm dosyalarda `None`.
    pub raw_text: Option<String>,
}

pub type RawFiles = HashMap<String, RawFile>;

#[derive(Debug)]
pub struct K1Result {
    pub files: RawFiles,
    pub notices: Vec<Notice>,
    /// locations.geojson feature 'id' kümesi (XFL_025: stop_times.location_id cross-ref).
    pub geojson_location_ids: std::collections::HashSet<String>,
}

// ── Notice yardımcısı ─────────────────────────────────────────────────────────

fn make_notice(
    counter: &mut u32,
    rule_id: &str,
    entity_type: EntityType,
    entity_id: Option<String>,
    file: Option<&str>,
    line: Option<u64>,
    field: Option<&str>,
    observed_value: Option<String>,
    message: String,
    remediation: &str,
) -> Notice {
    // K1 scope_key ve expected_value üretmez — ikisi de sabit None.
    crate::notice_factory::build(
        "K1", Some("k1"), counter, rule_id, entity_type, entity_id, None,
        file.map(str::to_string), line, field.map(str::to_string),
        observed_value, None, message, remediation,
    )
}

// ── UTF-8 BOM ─────────────────────────────────────────────────────────────────

const UTF8_BOM: &[u8] = &[0xEF, 0xBB, 0xBF];

/// ZIP merkezi-dizininde beyan edilen (saldırgan-kontrollü) açılmış boyuttan
/// ön-tahsis edilecek en büyük tampon. `zf.size()` güvenilmez olduğundan, bir arşiv
/// yalnızca devasa bir boyut *beyan ederek* ilk bayt okunmadan büyük bir Vec alloc'una
/// zorlayabilir. Beyanı bu tavana kapatırız; gerçek baytlar geldikçe Vec büyür, yani
/// meşru büyük dosyalar için davranış değişmez, yalnızca başlangıç kapasitesi sınırlanır.
const MAX_PREALLOC: usize = 64 * 1024 * 1024; // 64 MiB

/// K1 ham dosya tamponu için başlangıç kapasitesi: beyan edilen boyutu [`MAX_PREALLOC`]
/// ile sınırlar ve en az 1 döndürür (sıfır-kapasite alloc'u anlamsız).
#[inline]
fn prealloc_capacity(uncompressed_hint: usize) -> usize {
    uncompressed_hint.min(MAX_PREALLOC).max(1)
}

/// GuardedReader okuma hatasını Fatal'a çevirir: guard tetiklendiyse ARC_029
/// (DecompressionLimit), aksi halde ham ZIP okuma hatası (ARC_001 ZipUnreadable).
fn read_fatal<R: Read>(guarded: &GuardedReader<'_, R>, raw_name: &str, e: &std::io::Error) -> FatalError {
    match guarded.tripped() {
        Some(trip) => FatalError {
            code: FatalCode::DecompressionLimit,
            message: format!("'{raw_name}' sıkıştırma koruması sınırını aştı: {}", trip.describe()),
        },
        None => FatalError {
            code: FatalCode::ZipUnreadable,
            message: format!("'{raw_name}' okunamadı: {e}"),
        },
    }
}

/// Kısmi okunan tamponu son TAM UTF-8 karakter sınırına kırpar.
///
/// YALNIZ `is_zip_stream` (8 KB başlık okuması) için çağrılır: oradaki eksik kuyruk
/// BİZİM kesme noktamızın eseridir, dosyanın kusuru değil.
///
/// **Gerçek bozuk baytları GİZLEMEZ.** `Utf8Error` iki durumu ayırır:
/// - `error_len() == None`  → "girdi beklenmedik şekilde bitti" = yarım karakter
///   (yalnızca tamponun EN SONUNDA olabilir) → kırp, sorun yok.
/// - `error_len() == Some(n)` → gövdede geçersiz bayt dizisi → DOKUNMA; ARC_002 /
///   Utf8Critical eskisi gibi tetiklenir.
/// `from_utf8` İLK hatayı bildirdiğinden, gövdede gerçek bir hata varsa `Some` döner
/// ve kırpma yapılmaz — yani gerçek ihlal her zaman kazanır.
///
/// Regresyon: mdb-3176 (JP), mdb-1834 (LT), mdb-1831 (TH) — üçü de geçerli UTF-8
/// trips.txt'e sahipken 8190-8191. bayttaki kesik yüzünden feed'i fatal ediyordu.
fn truncate_to_utf8_boundary(buf: &mut Vec<u8>) {
    if let Err(e) = std::str::from_utf8(buf) {
        if e.error_len().is_none() {
            buf.truncate(e.valid_up_to());
        }
    }
}

fn strip_bom(bytes: &[u8]) -> (&[u8], bool) {
    if bytes.starts_with(UTF8_BOM) {
        (&bytes[3..], true)
    } else {
        (bytes, false)
    }
}

fn has_malformed_eol(bytes: &[u8]) -> bool {
    bytes.iter().enumerate().any(|(i, b)| *b == b'\r' && bytes.get(i + 1) != Some(&b'\n'))
}

// ── CSV tokenizer ─────────────────────────────────────────────────────────────

/// RFC 4180 uyumlu CSV tokenizer. Byte-level scanner; büyük dosyalarda hızlı.
/// `max_data_rows`: başlık hariç maksimum satır sayısı (None = sınırsız).
/// Limit aşıldığında kalan baytlar işlenmez; dönen vec truncated olur.
fn tokenize_csv(text: &str, max_data_rows: Option<usize>) -> Result<(Vec<Vec<SmolStr>>, bool), String> {
    let bytes = text.as_bytes();
    let n = bytes.len();
    let cap = max_data_rows.map(|m| m + 1).unwrap_or_else(|| (n / 50).max(1).min(5_000_000));
    let mut records: Vec<Vec<SmolStr>> = Vec::with_capacity(cap.min(5_000_000));
    let mut pos = 0;
    let mut col_hint: usize = 8;
    let mut truncated = false;

    while pos < n {
        let mut record: Vec<SmolStr> = Vec::with_capacity(col_hint);

        loop {
            if bytes[pos] == b'"' {
                // Quoted field — SmolStr dönüştürme için geçici String gerekir
                pos += 1;
                let mut buf = String::new();
                let mut closed = false;
                while pos < n {
                    let b = bytes[pos];
                    if b == b'"' {
                        pos += 1;
                        if pos < n && bytes[pos] == b'"' {
                            buf.push('"');
                            pos += 1;
                        } else {
                            closed = true;
                            break;
                        }
                    } else if b < 0x80 {
                        buf.push(b as char);
                        pos += 1;
                    } else {
                        let ch = text[pos..].chars().next().unwrap();
                        buf.push(ch);
                        pos += ch.len_utf8();
                    }
                }
                if !closed {
                    return Err("Kapanmamış tırnak işareti (unclosed quote)".to_string());
                }
                record.push(SmolStr::new(&buf));
            } else {
                // Unquoted field — doğrudan slice'tan SmolStr (≤22 byte → inline, heap alloc yok)
                let start = pos;
                while pos < n {
                    let b = bytes[pos];
                    if b == b',' || b == b'\n' || b == b'\r' {
                        break;
                    }
                    pos += 1;
                }
                record.push(SmolStr::new(&text[start..pos]));
            }

            if pos >= n {
                break;
            }
            match bytes[pos] {
                b',' => {
                    pos += 1;
                    if pos >= n {
                        // Sondaki virgül + newline yok: boş alan push et
                        record.push(SmolStr::new(""));
                        break;
                    }
                    continue;
                }
                b'\r' => {
                    pos += 1;
                    if pos < n && bytes[pos] == b'\n' {
                        pos += 1;
                    }
                    break;
                }
                b'\n' => {
                    pos += 1;
                    break;
                }
                _ => break,
            }
        }

        if !(record.len() == 1 && record[0].is_empty()) && !record.is_empty() {
            col_hint = record.len();
            records.push(record);
            if let Some(max) = max_data_rows {
                // records[0] başlık; veri satırı sayısı records.len()-1
                if records.len() > max {
                    truncated = true;
                    break;
                }
            }
        }
    }

    Ok((records, truncated))
}

/// stream_mode (stop_times.txt) için: gövdeyi Vec'e açmadan, yalnızca alloc'suz byte
/// taraması ile kapanmamış tırnak (ARC_013) olup olmadığını tespit eder. tokenize_csv'nin
/// alan-başı tırnak mantığını birebir taklit eder (yalnızca `"` baytlarını izler — UTF-8
/// devam baytlarında 0x22 görünmez, bu yüzden byte taraması güvenlidir).
fn csv_has_unclosed_quote(text: &str) -> bool {
    let b = text.as_bytes();
    let n = b.len();
    let mut pos = 0;
    while pos < n {
        // Alan başı
        if b[pos] == b'"' {
            pos += 1;
            let mut closed = false;
            while pos < n {
                if b[pos] == b'"' {
                    pos += 1;
                    if pos < n && b[pos] == b'"' {
                        pos += 1; // kaçırılmış tırnak ("")
                    } else {
                        closed = true;
                        break;
                    }
                } else {
                    pos += 1;
                }
            }
            if !closed {
                return true;
            }
        } else {
            while pos < n && b[pos] != b',' && b[pos] != b'\n' && b[pos] != b'\r' {
                pos += 1;
            }
        }
        // Ayraç
        if pos < n {
            match b[pos] {
                b'\r' => {
                    pos += 1;
                    if pos < n && b[pos] == b'\n' {
                        pos += 1;
                    }
                }
                _ => pos += 1,
            }
        }
    }
    false
}

// ── Şema yardımcıları ─────────────────────────────────────────────────────────

/// Dosya başına zorunlu sütun seti. Sütun başlıkta yoksa ARC_025; değer boşsa ilgili k2 grup kuralı.
///
/// ⚠️ **ELLE BAKIMLI ve ARC_025 Kritik·Spec'tir** — buraya KOŞULLU bir alan yazmak geçerli bir
/// feed'i yayından alıkoyar. 2026-08-02'de `transfers.from_stop_id`/`to_stop_id` tam olarak
/// böyleydi (spec: "Optional if transfer_type is 4 or 5"). Liste artık spec'e karşı
/// `required_columns_match_the_specification` testiyle kilitli.
///
/// `pub`: üretimde tek çağıranı bu dosyadır, ama kapsam ölçümü (`emit_proof.rs`) bu listeyi
/// **beyan edilmiş çapa kaynağı** olarak kullanır — ARC_025 eksik sütunu `field=<sütun>` ile
/// emit eder ve mekanizma tek bir döngüdür, dolayısıyla listede olmak o alanın
/// `presence:required` hükmünün denetlendiğinin kanıtıdır. (Fixture'la her sütunu tek tek
/// tetiklemek ~100 fixture demek olurdu; CAL_002'de yapılan şey burada ölçeklenmiyor.)
pub fn required_fields(filename: &str) -> &'static [&'static str] {
    match filename {
        "agency.txt"          => &["agency_name", "agency_url", "agency_timezone"],
        "stops.txt"           => &["stop_id"],
        "routes.txt"          => &["route_id", "route_type"],
        "trips.txt"           => &["route_id", "service_id", "trip_id"],
        // stop_id KOŞULLU zorunlu (spec): "Required if stop_times.location_group_id AND
        // stop_times.location_id are NOT defined. Forbidden if ... are defined." Koşul
        // başlıklara bakılarak `required_fields_for` içinde uygulanır.
        "stop_times.txt"      => &["trip_id", "stop_sequence"],
        "calendar.txt"        => &["service_id", "monday", "tuesday", "wednesday", "thursday", "friday", "saturday", "sunday", "start_date", "end_date"],
        "calendar_dates.txt"  => &["service_id", "date", "exception_type"],
        "shapes.txt"          => &["shape_id", "shape_pt_lat", "shape_pt_lon", "shape_pt_sequence"],
        "frequencies.txt"     => &["trip_id", "start_time", "end_time", "headway_secs"],
        // ⚠️ `from_stop_id`/`to_stop_id` BURADA OLMAMALI — spec ikisini de KOŞULLU yapar:
        // "Required if transfer_type is empty, 0, 1, 2, or 3. Optional if transfer_type is 4 or 5."
        // 4/5 koltuk-içi aktarmalardır ve `from_trip_id`/`to_trip_id` kullanırlar. Yalnız 4/5
        // aktarması olan GEÇERLİ bir feed, sütunları taşımadığı için ARC_025 (Kritik·Spec) alıyor
        // ve R1'de düşüyordu — geçerli feed'i yayından alıkoyan bir yanlış pozitif.
        // Gerçekten koşulsuz Required olan alan `transfer_type`'tır ve listede YOKTU.
        // (2026-08-02, ARC_025'in listesi spec'le karşılaştırıldı.)
        "transfers.txt"       => &["transfer_type"],
        // transfers boş = sınırsız transfer (geçerli GTFS değeri) — değer-boş kuralı tetiklenmemeli
        "fare_attributes.txt" => &["fare_id", "price", "currency_type", "payment_method", "transfers"],
        "fare_rules.txt"      => &["fare_id"],
        "pathways.txt"        => &["pathway_id", "from_stop_id", "to_stop_id", "pathway_mode", "is_bidirectional"],
        "levels.txt"          => &["level_id", "level_index"],
        "feed_info.txt"       => &["feed_publisher_name", "feed_publisher_url", "feed_lang"],
        "translations.txt"    => &["table_name", "field_name", "language", "translation"],
        "rider_categories.txt"    => &["rider_category_id", "rider_category_name", "is_default_fare_category"],
        "fare_media.txt"          => &["fare_media_id", "fare_media_type"],
        "fare_products.txt"       => &["fare_product_id", "amount", "currency"],
        "fare_leg_rules.txt"      => &["fare_product_id"],
        // Spec: dört alanın da bileşik birincil anahtar parçası; ağ alanları Required,
        // durak alanları karşılıklı koşullu (FLJ_003/004 ölçer) → required_fields'a yalnız
        // koşulsuz Required olan ikisi girer.
        "fare_leg_join_rules.txt" => &["from_network_id", "to_network_id"],
        "fare_transfer_rules.txt" => &["fare_transfer_type"],
        "areas.txt"               => &["area_id"],
        "stop_areas.txt"          => &["area_id", "stop_id"],
        "networks.txt"            => &["network_id"],
        "timeframes.txt"          => &["timeframe_group_id", "service_id"],
        "booking_rules.txt"       => &["booking_rule_id", "booking_type"],
        "location_groups.txt"        => &["location_group_id"],
        "location_group_stops.txt"   => &["location_group_id", "stop_id"],
        // organization_name spec'te Required; liste boş bırakılmıştı (2026-08-02'de eklendi).
        "attributions.txt"    => &["organization_name"],
        // route_networks.txt'in İKİ alanı da spec'te Required (dosya 2026-08-02'de tanıtıldı).
        "route_networks.txt"  => &["network_id", "route_id"],
        _                     => &[],
    }
}

/// ARC_009: boş (yalnız-başlık) dosya KRİTİK mi? Zorunlu dosyalar (agency/stops/routes/trips/
/// stop_times) + servis tanımlayan takvim dosyaları (calendar/calendar_dates) boşsa veri beklenen
/// yerde yoktur → KRİTİK. Diğer OPSİYONEL dosyaların (transfers, fare_*, shapes, frequencies,
/// translations, Fares v2/Flex/JP …) boş eklenmesi zararsızdır → BİLGİ'ye düşürülür.
fn arc009_critical(filename: &str) -> bool {
    REQUIRED_FILES.contains(&filename) || CALENDAR_FILES.contains(&filename)
}

/// DQ_016: baştaki/sondaki boşluk yalnız STRING/metin alanlarında anlamlıdır (isim, id, url,
/// desc, kod) — join'i kırar ya da görünür. Zaman/tarih/sayı/koordinat/sequence/enum alanlarında
/// boşluk parser'ca tolere edilir ve zararsızdır; MobilityData da bu tipli alanları işaretlemez
/// (ör. mdb-2 arrival_time ' 6:04:00' → MD 0, bizde 65.747 over-fire idi). Tipli sütunları muaf tut.
/// DQ_016 dosya-seviyesi toplayıcı: boşluklu satır sayısı + etkilenen sütun kümesi.
///
/// **Neden özet, satır-başına değil:** baş/son boşluk tek bir ÜRETİCİ ALIŞKANLIĞIDIR
/// (en sık: ayraç olarak `,` yerine `, ` yazmak) → dosyanın HER satırında çıkar.
/// Satır-başına emit `mdb-992`'de **2.361.873** notice üretiyordu (feed toplamının
/// %98,6'sı; MobilityData aynı feed'de 12) — tarayıcıyı OOM eden STM_050 vakasının
/// aynısı. Tespit doğru (RFC 4180'e göre o boşluk alanın parçası), sorun sunum.
///
/// Üç emit sitesi de (K1 genel, K2 stop_times, K2 shapes) bunu kullanır.
#[derive(Default)]
pub(crate) struct Dq016Acc {
    pub rows: u64,
    pub cols: std::collections::BTreeSet<String>,
    pub first_line: Option<u64>,
    last_line: u64,
}

impl Dq016Acc {
    /// Bir satırı işler. `values`/`headers` aynı sırada olmalı.
    pub fn observe<'a>(
        &mut self,
        line: u64,
        values: impl Iterator<Item = &'a str>,
        headers: &[String],
    ) {
        for (i, s) in values.enumerate() {
            if s == s.trim() {
                continue;
            }
            let Some(name) = headers.get(i).map(|h| h.as_str()) else { continue };
            if dq016_is_typed_column(name) {
                continue;
            }
            if self.last_line != line || self.rows == 0 {
                self.last_line = line;
                self.rows += 1;
                if self.first_line.is_none() {
                    self.first_line = Some(line);
                }
            }
            self.cols.insert(name.to_string());
        }
    }

    /// Özet metni: (observed, mesaj, sütun listesi). Boşsa `None`.
    /// Başka bir parçadan biriken sayaçları katar (stop_times chunk birleştirmesi).
    /// `other` dosyada DAHA SONRA gelen parçadır; `first_line` en küçüğü, `last_line`
    /// en büyüğü tutar, `rows` toplanır, `cols` birleşir.
    pub(crate) fn merge(&mut self, other: Dq016Acc) {
        self.rows += other.rows;
        self.cols.extend(other.cols);
        self.first_line = match (self.first_line, other.first_line) {
            (Some(a), Some(b)) => Some(a.min(b)),
            (a, b) => a.or(b),
        };
        self.last_line = self.last_line.max(other.last_line);
    }

    pub fn summary(&self, file_name: &str) -> Option<(String, String, String)> {
        if self.rows == 0 {
            return None;
        }
        let cols = self.cols.iter().map(String::as_str).collect::<Vec<_>>().join(", ");
        Some((
            format!("{} satır", self.rows),
            format!(
                "'{file_name}' dosyasında {} satırda baştaki/sondaki boşluk var; etkilenen sütunlar: {cols}.",
                self.rows
            ),
            cols,
        ))
    }
}

pub(crate) const DQ016_REMEDIATION: &str =
    "Değerlerdeki gereksiz baştaki/sondaki boşlukları kaldırın; ayraç olarak ', ' değil ',' kullanın.";

// ── ARC_012 / ARC_021: K1 ↔ K2-stream ortak mantığı ──────────────────────────
//
// stream_mode'da bu kontroller K1'den K2'ye taşındı, ama iki hedefe (stop_times,
// shapes) ayrı ayrı KOPYALANMIŞTI — üç yerde birebir aynı predicate ve aynı kullanıcı
// mesajı. Eşik/aralık listesine dokunmak üç dosya düzenlemek demekti; ikisini unutmak
// sessizce sapma üretirdi. DQ016_ACC ile aynı gerekçe: tek tanım, üç çağıran.

/// ARC_012: satırın sütun sayısı başlıktan farklı mı?
///
/// `None` → sorun yok. `Some((mesaj, öneri, observed, bilgi_mi))`; `bilgi_mi=true`
/// ise satır KISA (sondaki opsiyonel alanlar atlanmış — geçerli CSV pratiği, severity
/// Bilgi'ye düşürülür), `false` ise FAZLA alan var (kaçmamış virgül → nominal severity).
pub(crate) fn arc012_check(
    file_name: &str,
    line: u64,
    row_len: usize,
    header_count: usize,
) -> Option<(String, &'static str, String, bool)> {
    if row_len == header_count {
        return None;
    }
    let short = row_len < header_count;
    let (msg, tip) = if short {
        let missing = header_count - row_len;
        (
            format!("'{file_name}' {line}. satırda sondaki {missing} isteğe bağlı alan atlanmış ({row_len} sütun, başlık: {header_count})."),
            "CSV'de sondaki boş alanlar atlanabilir; zorunlu alanlar boş bırakılmamalıdır.",
        )
    } else {
        (
            format!("'{file_name}' {line}. satırda fazla alan: {row_len} (beklenen {header_count}) — kaçmamış virgül veya format hatası."),
            "Her satırın başlık sayısı kadar virgülle ayrılmış değer içerdiğinden emin olun.",
        )
    };
    Some((msg, tip, format!("{row_len} sütun (beklenen {header_count})"), short))
}

/// ARC_021: satırda yazdırılamaz/sorunlu karakter arar, ilk bulunanın kod noktasını döner.
///
/// Geçerli Unicode harf/rakam/boşluk (Japonca, Türkçe vb.) sorun DEĞİLDİR — yalnızca
/// kontrol karakterleri, DEL, yedek alanlar (surrogate) ve özel kullanım alanı işaretlenir.
pub(crate) fn arc021_bad_char<'a>(values: impl Iterator<Item = &'a str>) -> Option<u32> {
    for val in values {
        for ch in val.chars() {
            if ch.is_alphanumeric() || ch.is_whitespace() {
                continue;
            }
            let cp = ch as u32;
            let is_bad = (cp < 32 && cp != 9)
                || cp == 127
                || (0xD800..=0xDFFF).contains(&cp)
                || (0xE000..=0xF8FF).contains(&cp)
                || (0xFFF0..=0xFFFF).contains(&cp);
            if is_bad {
                return Some(cp);
            }
        }
    }
    None
}

/// ARC_030: alan değerinde sekme, satır sonu veya satır başı arar; ilk bulunanı döner.
///
/// GTFS spec, Dataset Files: "Field values must not contain tabs, carriage returns or new
/// lines." ARC_021 bu üçünü BİLEREK kaçırır (`is_whitespace()` muafiyeti, ayrıca `cp != 9`):
/// o kural bozuk/yazdırılamaz karakterleri hedefler ve geçerli metni muaf tutar. Buradaki üç
/// karakter ise geçerli metinde de görülebilir, ama spec onları açıkça yasaklar. İki kural
/// aynı olguyu ölçmez.
///
/// Bayt taraması UTF-8 güvenlidir: TAB/LF/CR ASCII'dir, çok baytlı karakterlerin devam
/// baytları 0x80 ve üzerindedir. `chars()` yerine `bytes()` bilinçli — bu K1 sıcak yolu.
pub(crate) fn arc030_bad_whitespace<'a>(values: impl Iterator<Item = &'a str>) -> Option<u32> {
    for val in values {
        for b in val.bytes() {
            if b == b'\t' || b == b'\n' || b == b'\r' {
                return Some(b as u32);
            }
        }
    }
    None
}

pub(crate) fn arc030_message(file_name: &str, cp: u32) -> String {
    let kind = match cp {
        9 => "sekme (TAB)",
        10 => "satır sonu (LF)",
        _ => "satır başı (CR)",
    };
    format!("'{file_name}' dosyasında bir alan değeri {kind} karakteri içeriyor (U+{cp:04X}).")
}

pub(crate) const ARC030_REMEDIATION: &str =
    "Alan değerlerindeki sekme ve satır sonu karakterlerini kaldırın; çok satırlı metni tek satıra indirin.";

// ── ARC_032: HTML etiketi / yorum / kaçış dizisi ─────────────────────────────
//
// GTFS spec, File Requirements: "Field values must not contain HTML tags, comments or
// escape sequences." ARC_021 ve ARC_030 bunu kaçırır — `<` (U+003C) ARC_021'in bozuk-karakter
// aralıklarının hiçbirine girmez, alfanumerik de değildir ama `is_bad` koşullarını sağlamaz.
//
// ⚠️ KAPALI LİSTE, DESEN DEĞİL. Ölçüm sırasında gevşek bir desen (`<\s*/?[a-zA-Z]+...>` +
// herhangi `&kelime;`) 239 feed'de 1991 eşleşme verdi ve örneklere bakınca çoğu MEŞRU metin
// çıktı: `K.A.Wheel&Tire;` bir entity değil, `St Sauvant >< St Césaire` bir etiket değil.
// Kapalı listeye geçince 1955 gerçek eşleşme kaldı (4 feed). Desen genişletilecekse
// örneklere BAKILARAK genişletilmeli.
const HTML_TAGS: &[&str] = &[
    "a", "b", "i", "u", "p", "br", "hr", "em", "strong", "span", "div", "img", "font",
    "ul", "ol", "li", "table", "tr", "td", "th", "h1", "h2", "h3", "small", "sub", "sup",
];
const HTML_ENTITIES: &[&str] = &["nbsp", "amp", "lt", "gt", "quot", "apos"];

/// Değerde HTML etiketi, yorum başlangıcı veya karakter kaçışı arar.
///
/// Dönen: bulunan işaretin metni (`<BR>`, `&nbsp;` gibi), kullanıcıya gösterilmek üzere.
fn arc032_markup(value: &str) -> Option<String> {
    let bytes = value.as_bytes();
    for (i, &b) in bytes.iter().enumerate() {
        if b == b'<' {
            if value[i..].starts_with("<!--") {
                return Some("<!--".to_string());
            }
            // `</` kapanış etiketi de sayılır. Eğik çizgi ÇIKTIDA korunur: kullanıcıya
            // gösterilen örnek, dosyadaki metinle birebir eşleşmeli.
            let after = &value[i + 1..];
            let (slash, rest) = match after.strip_prefix('/') {
                Some(r) => ("/", r),
                None => ("", after),
            };
            let name_len = rest
                .find(|c: char| !c.is_ascii_alphanumeric())
                .unwrap_or(rest.len());
            if name_len == 0 {
                continue;
            }
            let name = &rest[..name_len];
            if !HTML_TAGS.iter().any(|t| t.eq_ignore_ascii_case(name)) {
                continue;
            }
            // Etiket ancak `>` ile kapanırsa etikettir: "a < b" ya da "3 <br" değil.
            let Some(close) = rest[name_len..].find('>') else { continue };
            // `<` ile `>` arasında başka bir `<` varsa bu bir etiket değil, düz metindir.
            let inner = &rest[name_len..name_len + close];
            if inner.contains('<') {
                continue;
            }
            return Some(format!("<{slash}{}{}>", &rest[..name_len], inner));
        }
        if b == b'&' {
            let rest = &value[i + 1..];
            let Some(semi) = rest.find(';') else { continue };
            if semi == 0 || semi > 8 {
                continue;
            }
            let ent = &rest[..semi];
            let known = HTML_ENTITIES.iter().any(|e| e.eq_ignore_ascii_case(ent))
                || (ent.starts_with('#')
                    && ent.len() >= 2
                    && (ent[1..].chars().all(|c| c.is_ascii_digit())
                        || (ent[1..].starts_with(['x', 'X'])
                            && ent.len() >= 3
                            && ent[2..].chars().all(|c| c.is_ascii_hexdigit()))));
            if known {
                return Some(format!("&{ent};"));
            }
        }
    }
    None
}

/// ARC_032 birikimi — DOSYA başına TEK notice (`Dq016Acc` ile aynı gerekçe: bir feed'in
/// 1176 durak adı işaretli ve satır başına emit tarayıcıyı boğar).
#[derive(Default)]
pub(crate) struct Arc032Acc {
    pub rows: u64,
    pub cols: std::collections::BTreeSet<String>,
    pub first_line: Option<u64>,
    pub example: Option<String>,
    last_line: u64,
}

impl Arc032Acc {
    pub fn observe<'a>(
        &mut self,
        line: u64,
        values: impl Iterator<Item = &'a str>,
        headers: &[String],
    ) {
        for (i, s) in values.enumerate() {
            let Some(name) = headers.get(i).map(|h| h.as_str()) else { continue };
            // `tts_stop_name` STP_023'ün alanı: o kural HERHANGİ bir `<`/`>` işaretini
            // (SSML dahil) zaten bildiriyor ve daha geniştir. İkisini birden emit etmek
            // aynı olguyu iki kez raporlamak olur.
            if name == "tts_stop_name" {
                continue;
            }
            let Some(found) = arc032_markup(s) else { continue };
            if self.last_line != line || self.rows == 0 {
                self.last_line = line;
                self.rows += 1;
                if self.first_line.is_none() {
                    self.first_line = Some(line);
                }
            }
            self.cols.insert(name.to_string());
            if self.example.is_none() {
                self.example = Some(found);
            }
        }
    }

    /// Özet: (observed, mesaj, sütun listesi). Boşsa `None`.
    pub fn summary(&self, file_name: &str) -> Option<(String, String, String)> {
        if self.rows == 0 {
            return None;
        }
        let cols = self.cols.iter().map(String::as_str).collect::<Vec<_>>().join(", ");
        let example = self.example.as_deref().unwrap_or("");
        Some((
            example.to_string(),
            format!(
                "'{file_name}' dosyasında {} satırda HTML işareti var (ör. '{example}'); \
                 etkilenen sütunlar: {cols}.",
                self.rows
            ),
            cols,
        ))
    }
}

pub(crate) const ARC032_REMEDIATION: &str =
    "HTML etiketlerini ve karakter kaçışlarını alan değerlerinden kaldırın; \
     GTFS alanları düz metin taşır ve tüketiciler bu işaretleri olduğu gibi gösterir.";

pub(crate) fn arc021_message(file_name: &str, cp: u32) -> String {
    // "ASCII dışı" DEME: kural geçerli Unicode harfleri (ü, ö, 漢字…) bilerek muaf tutar;
    // yalnız kontrol/DEL/surrogate/private-use işaretlenir. Mesaj yaptığından fazlasını
    // iddia etmemeli (VBB Almanca metin tartışması, 2026-07-24).
    format!("'{file_name}' dosyasında yazdırılamaz veya sorunlu karakter içeren değer var (U+{cp:04X}).")
}

pub(crate) const ARC021_REMEDIATION: &str =
    "Alan değerlerindeki kontrol/yazdırılamaz karakterleri kaldırın; geçerli Unicode harfler (ü, ö, 漢字 vb.) sorun değildir.";

pub(crate) fn dq016_is_typed_column(name: &str) -> bool {
    if name.ends_with("_time") || name.ends_with("_date") || name.ends_with("_sequence")
        || name.ends_with("_lat") || name.ends_with("_lon") || name.ends_with("_secs")
    {
        return true;
    }
    matches!(name,
        "route_type" | "location_type" | "direction_id" | "wheelchair_boarding"
        | "wheelchair_accessible" | "bikes_allowed" | "cars_allowed" | "pickup_type"
        | "drop_off_type" | "continuous_pickup" | "continuous_drop_off" | "timepoint"
        | "shape_dist_traveled" | "exception_type" | "payment_method" | "transfers"
        | "transfer_type" | "exact_times" | "is_bidirectional" | "stop_sequence"
    )
}

/// GTFS spesifikasyonunda tanımlı sütun adları (ARC_017).
fn known_columns(filename: &str) -> &'static [&'static str] {
    match filename {
        "agency.txt" => &[
            "agency_id","agency_name","agency_url","agency_timezone",
            "agency_lang","agency_phone","agency_fare_url","agency_email",
            "cemv_support",
        ],
        "stops.txt" => &[
            "stop_id","stop_code","stop_name","stop_desc","stop_lat","stop_lon",
            "zone_id","stop_url","location_type","parent_station","stop_timezone",
            "wheelchair_boarding","level_id","platform_code","stop_access","tts_stop_name",
        ],
        "routes.txt" => &[
            "route_id","agency_id","route_short_name","route_long_name","route_desc",
            "route_type","route_url","route_color","route_text_color","route_sort_order",
            "continuous_pickup","continuous_drop_off","network_id","cemv_support",
            // GTFS-JP
            "jp_parent_route_id","jp_office_id",
        ],
        "trips.txt" => &[
            "route_id","service_id","trip_id","trip_headsign","trip_short_name",
            "direction_id","block_id","shape_id","wheelchair_accessible","bikes_allowed",
            "cars_allowed","safe_duration_factor","safe_duration_offset",
            // GTFS-JP
            "jp_trip_desc","jp_trip_desc_symbol","jp_pattern_id",
        ],
        "stop_times.txt" => &[
            "trip_id","arrival_time","departure_time","stop_id","stop_sequence",
            "stop_headsign","pickup_type","drop_off_type","continuous_pickup",
            "continuous_drop_off","shape_dist_traveled","timepoint",
            // GTFS-Flex (resmî Schedule Reference): talep-duyarlı hizmet alanları.
            // K2 parser bunları ZATEN okuyordu; burada eksik olmaları geçerli bir Flex
            // feed'inde ARC_017 "bilinmeyen sütun" yanlış-pozitifi üretiyordu.
            "location_id","location_group_id",
            "start_pickup_drop_off_window","end_pickup_drop_off_window",
            "pickup_booking_rule_id","drop_off_booking_rule_id",
        ],
        "calendar.txt" => &[
            "service_id","monday","tuesday","wednesday","thursday","friday",
            "saturday","sunday","start_date","end_date",
        ],
        "calendar_dates.txt" => &["service_id","date","exception_type"],
        "shapes.txt" => &[
            "shape_id","shape_pt_lat","shape_pt_lon","shape_pt_sequence","shape_dist_traveled",
        ],
        "frequencies.txt" => &["trip_id","start_time","end_time","headway_secs","exact_times"],
        "transfers.txt" => &[
            "from_stop_id","to_stop_id","transfer_type","min_transfer_time",
            "from_route_id","to_route_id","from_trip_id","to_trip_id",
        ],
        "fare_attributes.txt" => &[
            "fare_id","price","currency_type","payment_method","transfers",
            "agency_id","transfer_duration",
        ],
        "fare_rules.txt" => &["fare_id","route_id","origin_id","destination_id","contains_id"],
        "pathways.txt" => &[
            "pathway_id","from_stop_id","to_stop_id","pathway_mode","is_bidirectional",
            "length","traversal_time","stair_count","max_slope","min_width",
            "signposted_as","reversed_signposted_as",
        ],
        "levels.txt" => &["level_id","level_index","level_name"],
        "feed_info.txt" => &[
            "feed_publisher_name","feed_publisher_url","feed_lang","default_lang",
            "feed_start_date","feed_end_date","feed_version",
            "feed_contact_email","feed_contact_url",
        ],
        "translations.txt" => &[
            "table_name","field_name","language","translation",
            "record_id","record_sub_id","field_value",
        ],
        "attributions.txt" => &[
            "attribution_id","agency_id","route_id","trip_id","organization_name",
            "is_producer","is_operator","is_authority",
            "attribution_url","attribution_email","attribution_phone",
        ],
        // ── Fares v2 ── (sütun adları GTFS spec + repo k2 parser'larıyla doğrulandı)
        "rider_categories.txt" => &[
            "rider_category_id","rider_category_name","is_default_fare_category","eligibility_url",
        ],
        "fare_media.txt" => &["fare_media_id","fare_media_name","fare_media_type"],
        "fare_products.txt" => &[
            "fare_product_id","fare_product_name","rider_category_id","fare_media_id","amount","currency",
        ],
        "fare_leg_rules.txt" => &[
            "leg_group_id","network_id","from_area_id","to_area_id",
            "from_timeframe_group_id","to_timeframe_group_id","fare_product_id","rule_priority",
        ],
        // `rule_priority` BURADA YOK: 27 Nisan 2026 Schedule Reference'ta bu alan yalnız
        // `fare_leg_rules.txt`'te tanımlıdır (FLG_007 orayı ölçer). fare_transfer_rules'un
        // resmî alan listesi aşağıdaki yedidir.
        "fare_transfer_rules.txt" => &[
            "from_leg_group_id","to_leg_group_id","transfer_count","duration_limit",
            "duration_limit_type","fare_transfer_type","fare_product_id",
        ],
        "timeframes.txt" => &["timeframe_group_id","start_time","end_time","service_id"],
        "areas.txt" => &["area_id","area_name"],
        "stop_areas.txt" => &["area_id","stop_id"],
        "networks.txt" => &["network_id","network_name"],
        "route_networks.txt" => &["network_id","route_id"],
        "fare_leg_join_rules.txt" => &["from_network_id","to_network_id","from_stop_id","to_stop_id"],
        // ── Flex ──
        "booking_rules.txt" => &[
            "booking_rule_id","booking_type",
            "prior_notice_duration_min","prior_notice_duration_max",
            "prior_notice_last_day","prior_notice_last_time",
            "prior_notice_start_day","prior_notice_start_time","prior_notice_service_id",
            "message","pickup_message","drop_off_message","phone_number","info_url","booking_url",
        ],
        "location_groups.txt" => &["location_group_id","location_group_name"],
        "location_group_stops.txt" => &["location_group_id","stop_id"],
        // ── GTFS-JP (Japonya profili) — sütunlar resmî format-reference + Tokyo Toei feed'iyle doğrulandı.
        // Not: jp_ önekli sütunlar ARC_017'de zaten atlanır (ek güvenlik marjı).
        "agency_jp.txt" => &[
            "agency_id","agency_official_name","agency_zip_number","agency_address",
            "agency_president_pos","agency_president_name",
        ],
        "office_jp.txt" => &["office_id","office_name","office_url","office_phone"],
        "routes_jp.txt" => &[
            "route_id","route_update_date","origin_stop","via_stop","destination_stop",
        ],
        _ => &[],
    }
}

/// Tek bir üst klasöre sarılmış feed'in kök önekini döndürür (ör. `"TEST GTFS/"`).
///
/// Bazı yayıncılar ZIP'i tek bir klasörle paketliyor: `TEST GTFS/agency.txt`. Spec
/// dosyaların kökte olmasını ister, dolayısıyla bunlar ARC_024 alır — ama katı davranmak
/// kullanıcıya ARC_004 fatal'ı ve **sıfır analiz** verir, oysa feed'in kendisi geçerlidir.
///
/// Hoşgörü KASITLI olarak dar; şu üç koşulun hepsi gerekir:
/// 1. Kökte hiç `.txt` yok (varsa feed sarılı değildir, dokunma),
/// 2. tüm `.txt` girdileri TEK ve AYNI üst klasörün DOĞRUDAN altında (iki kat derinlik yok),
/// 3. o klasör zorunlu GTFS dosyalarının tamamını taşıyor.
///
/// Aksi halde `None` → eski davranış. Böylece "GTFS olmayan repo zip'i" (ör. tek klasör
/// içinde `.csv`'ler) hâlâ fatal olur; hoşgörü yalnız gerçekten geçerli feed'i kurtarır.
fn detect_wrapped_root(entry_names: &[String]) -> Option<String> {
    let normalized: Vec<String> = entry_names.iter().map(|n| n.replace('\\', "/")).collect();
    let txt = || normalized.iter().filter(|n| n.ends_with(".txt"));

    if txt().any(|n| !n.contains('/')) {
        return None; // kökte .txt var → sarılı değil
    }

    let mut prefix: Option<&str> = None;
    for name in txt() {
        let (dir, rest) = name.split_once('/')?;
        if rest.contains('/') {
            return None; // ikinci seviye alt dizin → hoşgörü yok
        }
        match prefix {
            None => prefix = Some(dir),
            Some(p) if p == dir => {}
            Some(_) => return None, // birden fazla üst klasör
        }
    }

    let dir = prefix?;
    REQUIRED_FILES
        .iter()
        .all(|f| normalized.iter().any(|n| n == &format!("{dir}/{f}")))
        .then(|| format!("{dir}/"))
}

// ── Ana parse fonksiyonu ──────────────────────────────────────────────────────

/// K1 Parse katmanı. ZIP baytlarından ham dosya haritası ve ARC_* notice'ları üretir.
///
/// Fatal durumlar (pipeline durur):
/// - ARC_001: ZIP açılamazsa
/// - ARC_002: Zorunlu dosya UTF-8 ile okunamazsa
/// - ARC_004: Zorunlu dosyalar eksikse
/// - ARC_013: Zorunlu dosya CSV tokenization'ı başarısızsa
///
/// ARC_008: Kritik notice üretir, pipeline devam eder (non-Fatal).
pub fn parse(zip_bytes: &[u8]) -> Result<K1Result, FatalError> {
    k1dbg!("[K1] parse başladı, {} bayt", zip_bytes.len());
    let cursor = std::io::Cursor::new(zip_bytes);
    k1dbg!("[K1] ZipArchive::new çağrılıyor...");
    let mut archive = zip::ZipArchive::new(cursor).map_err(|e| FatalError {
        code: FatalCode::ZipUnreadable,
        message: format!("ZIP arşivi açılamadı: {e}"),
    })?;
    k1dbg!("[K1] archive açıldı: {} entry", archive.len());

    // ARC_009 yanlış-pozitif guard'ı: calendar_dates.txt boş (veya yalnızca başlık) olsa bile
    // calendar.txt servisi tanımlıyorsa bu KRİTİK değildir (calendar_dates şartlı zorunludur,
    // calendar.txt varken opsiyoneldir). Döngü-sırasından bağımsız olsun diye önceden hesaplanır.
    // calendar.txt YOKSA boş calendar_dates "hiç servis tanımı yok" demektir → ARC_009 kalır.
    // Tek üst klasöre sarılmış feed → o klasörü kök kabul et (detay: detect_wrapped_root).
    let entry_names: Vec<String> = archive.file_names().map(str::to_string).collect();
    let root_prefix = detect_wrapped_root(&entry_names);
    let calendar_entry = format!("{}calendar.txt", root_prefix.as_deref().unwrap_or(""));
    let has_calendar_txt = entry_names.iter().any(|n| n.replace('\\', "/") == calendar_entry);

    let mut notices: Vec<Notice> = Vec::new();
    let mut counter: u32 = 0;
    let mut geojson_location_ids: std::collections::HashSet<String> = std::collections::HashSet::new();
    let mut raw_files: RawFiles = HashMap::new();
    let mut present_files: HashSet<String> = HashSet::new();
    // locations.geojson `present_files`'a GİRMEZ (aşağıda `continue` ile özel işlenir);
    // ARC_004'ün stops.txt koşulu için ayrı bayrak tutulur.
    let mut has_locations_geojson = false;
    // DQ_016 için dosya başına birincil anahtar sütun indeksi — ilk "*_id" sütunu
    let _dq016_dummy = ();

    // #46: zip-bomb koruması — tüm girdiler boyunca açılan toplam bayt (traversal geneli).
    let mut total_decompressed: u64 = 0;
    for i in 0..archive.len() {
        k1dbg!("[K1] by_index({i})...");
        let mut zf = archive.by_index(i).map_err(|e| FatalError {
            code: FatalCode::ZipUnreadable,
            message: format!("ZIP dosyası okunamadı (index {i}): {e}"),
        })?;

        let entry_name = zf.name().to_string();
        // Sarılı feed'de kök öneki düşülür; sonraki tüm mantık dosyayı kökteymiş gibi görür.
        // Bulgu mesajları yine ZIP'teki GERÇEK yolu gösterir (entry_name).
        let raw_name = match &root_prefix {
            Some(prefix) => entry_name.replace('\\', "/").strip_prefix(prefix.as_str())
                .map_or_else(|| entry_name.clone(), str::to_string),
            None => entry_name.clone(),
        };
        if raw_name.is_empty() {
            continue; // sarılı feed'in klasör girdisinin kendisi
        }
        // Hoşgörü uygulandı: dosya işlenecek ama alt dizinde olduğu yine de bulgudur.
        if raw_name != entry_name && raw_name.ends_with(".txt") {
            notices.push(make_notice(
                &mut counter, "ARC_024",
                EntityType::File, Some(entry_name.clone()),
                Some(&entry_name), None, None,
                Some(entry_name.clone()),
                format!("'{entry_name}' alt dizinde bulunuyor — GTFS dosyaları ZIP kök dizininde düz olmalıdır."),
                "Dosyayı ZIP'in kök dizinine taşıyın.",
            ));
        }
        if raw_name.ends_with(".txt") && zf.unix_mode().is_some_and(|mode| mode & 0o400 == 0) {
            notices.push(make_notice(&mut counter, "ARC_027", EntityType::File, Some(raw_name.clone()),
                Some(&raw_name), None, None, zf.unix_mode().map(|m| format!("{m:o}")),
                format!("'{raw_name}' ZIP girdisinde kullanıcı okuma izni bulunmuyor."),
                "ZIP girdisinin Unix izinlerine user-read (0400) bitini ekleyin."));
        }

        // ARC_023: ZIP içinde nested ZIP dosyası
        if raw_name.ends_with(".zip") && !raw_name.contains('/') && !raw_name.contains('\\') {
            notices.push(make_notice(
                &mut counter, "ARC_023",
                EntityType::File, Some(raw_name.clone()),
                Some(&raw_name), None, None,
                Some(raw_name.clone()),
                format!("'{raw_name}' GTFS ZIP içinde başka bir ZIP dosyası — GTFS bu formatı desteklemiyor."),
                "İç içe ZIP dosyasını kaldırın; GTFS dosyaları tek bir ZIP içinde düz yapıda bulunmalıdır.",
            ));
            continue;
        }

        // locations.geojson: Flex GeoJSON lokasyon dosyası (özel işlem)
        if raw_name == "locations.geojson" {
            has_locations_geojson = true;
            let mut buf = Vec::with_capacity(zf.size() as usize);
            if zf.read_to_end(&mut buf).is_ok() {
                validate_locations_geojson(&buf, &raw_name, &mut notices, &mut counter, &mut geojson_location_ids);
            }
            continue;
        }

        // ARC_024: Alt dizinde .txt dosyası — standart parser'lar bu dosyaları atlar
        if raw_name.ends_with(".txt") && (raw_name.contains('/') || raw_name.contains('\\')) {
            notices.push(make_notice(
                &mut counter, "ARC_024",
                EntityType::File, Some(raw_name.clone()),
                Some(&raw_name), None, None,
                Some(raw_name.clone()),
                format!("'{raw_name}' alt dizinde bulunuyor — GTFS dosyaları ZIP kök dizininde düz olmalıdır."),
                "Dosyayı ZIP'in kök dizinine taşıyın.",
            ));
            continue;
        }

        // ARC_007: kök dizindeki tanınmayan (.txt olmayan) dosyalar da bilinmeyen dosya olarak
        // işaretlenir (ör. .pdf/.out/.csv). .zip → ARC_023, locations.geojson, alt-dizin .txt →
        // ARC_024 yukarıda özel işlendi; bunlar buraya ulaşmadan continue etti.
        if !raw_name.ends_with(".txt") {
            if !raw_name.contains('/') && !raw_name.contains('\\') {
                notices.push(make_notice(
                    &mut counter, "ARC_007",
                    EntityType::File, Some(raw_name.clone()),
                    Some(&raw_name), None, None,
                    Some(raw_name.clone()),
                    format!("'{raw_name}' GTFS spesifikasyonunda tanımlı değil."),
                    "GTFS dışı dosyaları ZIP'ten kaldırın.",
                ));
            }
            continue;
        }
        // Yalnızca kök dizindeki .txt dosyaları işlenir (alt-dizin .txt yukarıda ARC_024 aldı)
        if raw_name.contains('/') || raw_name.contains('\\') {
            continue;
        }

        present_files.insert(raw_name.clone());
        let is_required = REQUIRED_FILES.contains(&raw_name.as_str());
        let is_known   = KNOWN_FILES.contains(&raw_name.as_str());

        // Bayt oku (ARC_011 için is_known kontrolünden önce yapılır)
        k1dbg!("[K1] okuma başladı: {raw_name} (compressed: {} b)", zf.compressed_size());
        let uncompressed_hint = zf.size() as usize;
        // #38: Büyük dosyalar K2 ZIP stream eder; K1 yalnızca başlık satırını okur.
        // stop_times, trips, calendar_dates için 8 KB partial read; raw_text=None.
        let is_zip_stream = matches!(raw_name.as_str(), "stop_times.txt" | "shapes.txt" | "trips.txt" | "calendar_dates.txt");
        let _is_stop_times = raw_name == "stop_times.txt"; // legacy: sadece ARC_011 bytes için artık is_zip_stream
        // Alloc'dan ÖNCE log: crash olursa son satır hangi dosyada OOM çıktığını gösterir.
        crate::timing::mem_note(&format!(
            "K1::alloc::{} hint={}MB{}",
            raw_name, uncompressed_hint / 1_048_576,
            if is_zip_stream { " (partial)" } else { "" }
        ));
        crate::timing::mem_log(&format!("K1::pre-alloc::{raw_name}"));
        let mut raw_vec = if is_zip_stream {
            Vec::with_capacity(8192)
        } else {
            Vec::with_capacity(prealloc_capacity(uncompressed_hint))
        };
        {
            let _t = crate::timing::Timer::start(format!("K1::decompress::{raw_name}"));
            // #46: her girdi, açılan baytları sayan GuardedReader üzerinden okunur.
            // Bomba tetiklerse read() hata döner; tripped() ile plain ZIP hatasından
            // ayrılıp ARC_029 (DecompressionLimit) Fatal'ı üretilir.
            let comp_size = zf.compressed_size();
            let mut guarded = GuardedReader::new(&mut zf, DEFAULT_DECOMPRESSION_LIMITS, &mut total_decompressed, comp_size);
            if is_zip_stream {
                let mut chunk = [0u8; 64 * 1024];
                let mut malformed_eol = false;
                let mut previous_was_cr = false;
                loop {
                    let n = match guarded.read(&mut chunk) {
                        Ok(n) => n,
                        Err(e) => return Err(read_fatal(&guarded, &raw_name, &e)),
                    };
                    if n == 0 { break; }
                    if previous_was_cr && chunk[0] != b'\n' { malformed_eol = true; }
                    malformed_eol |= chunk[..n].windows(2).any(|p| p[0] == b'\r' && p[1] != b'\n');
                    previous_was_cr = chunk[n - 1] == b'\r';
                    let keep = (8192usize.saturating_sub(raw_vec.len())).min(n);
                    raw_vec.extend_from_slice(&chunk[..keep]);
                }
                malformed_eol |= previous_was_cr;
                if malformed_eol { notices.push(make_notice(&mut counter, "ARC_026", EntityType::File, Some(raw_name.clone()), Some(&raw_name), None, None, Some("CR not followed by LF".to_string()), format!("'{raw_name}' CRLF veya LF dışında satır sonu içeriyor."), "Satır sonlarını LF ya da CRLF olarak yeniden yazın.")); }
                // 8192'lik kesme çok baytlı bir UTF-8 karakterini ortadan bölebilir.
                // Kırpılmazsa aşağıdaki from_utf8 GEÇERLİ dosyayı bozuk sanar ve
                // zorunlu dosyada Utf8Critical → feed tümüyle reddedilirdi.
                truncate_to_utf8_boundary(&mut raw_vec);
            } else if let Err(e) = guarded.read_to_end(&mut raw_vec) {
                return Err(read_fatal(&guarded, &raw_name, &e));
            }
        }
        if !is_zip_stream && has_malformed_eol(&raw_vec) {
            notices.push(make_notice(&mut counter, "ARC_026", EntityType::File, Some(raw_name.clone()), Some(&raw_name), None, None, Some("CR not followed by LF".to_string()), format!("'{raw_name}' CRLF veya LF dışında satır sonu içeriyor."), "Satır sonlarını LF ya da CRLF olarak yeniden yazın."));
        }

        // ARC_011: Dosya boyutu — tüm kök .txt dosyaları için (bilinmeyen dahil).
        // zip_stream partial_read'de raw_vec yalnızca başlık (~8KB); gerçek boyut ZIP metadata'sından.
        let file_size_bytes = if is_zip_stream { uncompressed_hint } else { raw_vec.len() };
        notices.push(make_notice(
            &mut counter, "ARC_011",
            EntityType::File, Some(raw_name.clone()),
            Some(&raw_name), None, None,
            Some(format!("{file_size_bytes} bayt")),
            format!("'{raw_name}' boyutu: {file_size_bytes} bayt."),
            "Bilgi amaçlı; düzeltme gerekmez.",
        ));

        // ARC_007: Bilinmeyen dosya — boyut kaydedildikten sonra atla
        if !is_known {
            notices.push(make_notice(
                &mut counter, "ARC_007",
                EntityType::File, Some(raw_name.clone()),
                Some(&raw_name), None, None,
                Some(raw_name.clone()),
                format!("'{raw_name}' GTFS spesifikasyonunda tanımlı değil."),
                "GTFS dışı dosyaları ZIP'ten kaldırın.",
            ));
            continue;
        }

        // ARC_006: İsteğe bağlı dosya mevcut (BİLGİ)
        if !is_required {
            notices.push(make_notice(
                &mut counter, "ARC_006",
                EntityType::File, Some(raw_name.clone()),
                Some(&raw_name), None, None,
                None,
                format!("İsteğe bağlı dosya mevcut: '{raw_name}'."),
                "Bilgi amaçlı; düzeltme gerekmez.",
            ));
        }

        // BOM kontrolü
        let (bytes, has_bom) = strip_bom(&raw_vec);
        if has_bom {
            // ARC_010 ve DQ_014 aynı koşul — ikisi ayrı ayrı ateşlenmez;
            // DQ_014 sadece ARC_002 suppressor listesinde referans, ARC_010 onu kapsar.
            notices.push(make_notice(
                &mut counter, "ARC_010",
                EntityType::File, Some(raw_name.clone()),
                Some(&raw_name), None, None,
                Some("UTF-8 BOM (EF BB BF)".to_string()),
                format!("'{raw_name}' dosyasında UTF-8 BOM var — bazı parser'lar BOM'u ilk alan adının parçası olarak okur."),
                "Dosyayı BOM olmadan UTF-8 olarak kaydedin (UTF-8 without BOM).",
            ));
        }

        k1dbg!("[K1] okundu: {raw_name} ({} bayt)", bytes.len());
        // UTF-8 doğrulama — from_utf8 sıfır maliyetli &str döndürür (kopya yok).
        // Geçersiz UTF-8 durumunda lossy String gerekir; yoksa doğrudan bytes'ı ödünç alıyoruz.
        let lossy_buf: String;
        let text: &str = match std::str::from_utf8(bytes) {
            Ok(s) => s,
            Err(_) => {
                // ARC_002: Kritik UTF-8 ihlali
                notices.push(make_notice(
                    &mut counter, "ARC_002",
                    EntityType::File, Some(raw_name.clone()),
                    Some(&raw_name), None, None,
                    None,
                    format!("'{raw_name}' UTF-8 kodlamasıyla okunamıyor."),
                    "Dosyayı UTF-8 kodlamasıyla yeniden kaydedin.",
                ));
                if is_required {
                    return Err(FatalError {
                        code: FatalCode::Utf8Critical,
                        message: format!("Zorunlu dosya UTF-8 ile okunamıyor: {raw_name}"),
                    });
                }
                // İsteğe bağlı dosya: ARC_003 (encoding kalitesi)
                lossy_buf = String::from_utf8_lossy(bytes).into_owned();
                notices.push(make_notice(
                    &mut counter, "ARC_003",
                    EntityType::File, Some(raw_name.clone()),
                    Some(&raw_name), None, None,
                    None,
                    format!("'{raw_name}' UTF-8 dışı karakter içeriyor; kayıplı dönüşüm uygulandı."),
                    "Tüm GTFS dosyalarını UTF-8 kodlamasıyla kaydedin.",
                ));
                &lossy_buf
            }
        };

        // OOM fix Plan A + #15 W3: stop_times.txt ve shapes.txt çok büyük olabilir; K1'de tam
        // Vec<Vec<SmolStr>>'e açmak yüzlerce MB tutar (shape-ağır feed'de ~920 MB ölçüldü).
        // Bu dosyalarda SADECE header tokenize edilir; gövde ham metin (`raw_text`) olarak
        // K2'ye verilip orada streaming işlenir. Per-satır notice'lar (ARC_012/016/018/021,
        // DQ_016, ARC_022, veri-yok ARC_009) K2 stream geçişine taşındı.
        // #38: trips.txt + calendar_dates.txt K1'de stream edilir.
        // trips: ~3M sefer × 8 alan Vec<Vec<SmolStr>> (~576 MB) TripRecord Vec ile çakışır.
        // calendar_dates: Swiss feed ~8M+ satır → rows Vec<Vec<SmolStr>> 700MB+ tüketir,
        // stop_times.txt decompress için yer kalmaz.
        let stream_mode = raw_name == "stop_times.txt" || raw_name == "shapes.txt"
            || raw_name == "trips.txt" || raw_name == "calendar_dates.txt";

        k1dbg!("[K1] tokenizing: {raw_name}");
        // CSV tokenization — stream_mode'da yalnızca başlık (Some(0)), aksi halde tam veri
        let _t = crate::timing::Timer::start(format!("K1::tokenize::{raw_name}"));
        let (mut records, _) = match tokenize_csv(text, if stream_mode { Some(0) } else { None }) {
            Ok(r) => r,
            Err(msg) => {
                // ARC_013
                notices.push(make_notice(
                    &mut counter, "ARC_013",
                    EntityType::File, Some(raw_name.clone()),
                    Some(&raw_name), None, None,
                    Some(msg.clone()),
                    format!("'{raw_name}' CSV tokenization hatası: {msg}"),
                    "CSV formatını kontrol edin; tırnak işaretlerinin doğru kapandığından emin olun.",
                ));
                if is_required {
                    return Err(FatalError {
                        code: FatalCode::CsvMalformed,
                        message: format!("Zorunlu dosya CSV tokenization hatası: {raw_name}"),
                    });
                }
                continue;
            }
        };

        // stream_mode: K1 yalnızca başlığı tokenize eder; gövdedeki kapanmamış tırnağı
        // (ARC_013) yine de tespit et — zorunlu dosyada Fatal davranışı korunur (alloc YOK).
        // zip_stream dosyalarda gövde yoktur; unclosed quote kontrolü yanlış-pozitif üretir.
        if stream_mode && !is_zip_stream && csv_has_unclosed_quote(text) {
            let msg = "Kapanmamış tırnak işareti (unclosed quote)".to_string();
            notices.push(make_notice(
                &mut counter, "ARC_013",
                EntityType::File, Some(raw_name.clone()),
                Some(&raw_name), None, None,
                Some(msg.clone()),
                format!("'{raw_name}' CSV tokenization hatası: {msg}"),
                "CSV formatını kontrol edin; tırnak işaretlerinin doğru kapandığından emin olun.",
            ));
            if is_required {
                return Err(FatalError {
                    code: FatalCode::CsvMalformed,
                    message: format!("Zorunlu dosya CSV tokenization hatası: {raw_name}"),
                });
            }
            continue;
        }


        // ARC_009: Boş dosya
        if records.is_empty() {
            // calendar.txt servisi tanımlıyorsa boş calendar_dates.txt yanlış-pozitiftir → bastır.
            if !(raw_name == "calendar_dates.txt" && has_calendar_txt) {
                let mut n = make_notice(
                    &mut counter, "ARC_009",
                    EntityType::File, Some(raw_name.clone()),
                    Some(&raw_name), None, None,
                    None,
                    format!("'{raw_name}' dosyası boş veya yalnızca başlık satırı içeriyor."),
                    "Dosyaya en az bir veri satırı ekleyin (veya gereksizse dosyayı kaldırın).",
                );
                // Boş OPSİYONEL dosya (transfers, fare_*, vb.) KRİTİK değildir → Bilgi'ye düşür.
                // Zorunlu dosyalar + servis tanımlayan takvim dosyaları Kritik kalır.
                if !arc009_critical(&raw_name) {
                    n.severity = Severity::Bilgi;
                }
                notices.push(n);
            }
            continue;
        }

        // Başlık satırı (ownership al, clone yok)
        let raw_headers = records.remove(0);

        // ARC_014: Başlıkta boşluk
        for (col_i, hdr) in raw_headers.iter().enumerate() {
            if hdr != hdr.trim() {
                notices.push(make_notice(
                    &mut counter, "ARC_014",
                    EntityType::File, Some(raw_name.clone()),
                    Some(&raw_name), Some(1), Some(hdr.as_str()),
                    Some(format!("{hdr:?}")),
                    format!("'{raw_name}' başlığında '{hdr}' sütununda gereksiz boşluk var."),
                    "CSV başlıklarındaki baştaki/sondaki boşlukları kaldırın.",
                ));
                let _ = col_i;
            }
        }

        // Temiz başlıklar (trim)
        let headers: Vec<String> = raw_headers.iter().map(|h| h.trim().to_string()).collect();

        // ARC_019: Başlıkta boş sütun adı
        for hdr in &headers {
            if hdr.is_empty() {
                notices.push(make_notice(
                    &mut counter, "ARC_019",
                    EntityType::File, Some(raw_name.clone()),
                    Some(&raw_name), Some(1), None,
                    Some("(boş)".to_string()),
                    format!("'{raw_name}' başlığında boş sütun adı var."),
                    "Tüm sütun adlarının dolu olduğundan emin olun.",
                ));
                break; // Dosya başına bir kez yeter
            }
        }

        // ARC_015: Tekrar eden sütun
        let mut seen_hdrs: HashSet<&str> = HashSet::new();
        for hdr in &headers {
            if !seen_hdrs.insert(hdr.as_str()) {
                notices.push(make_notice(
                    &mut counter, "ARC_015",
                    EntityType::File, Some(raw_name.clone()),
                    Some(&raw_name), Some(1), Some(hdr.as_str()),
                    Some(hdr.clone()),
                    format!("'{raw_name}' dosyasında '{hdr}' sütunu tekrarlanıyor."),
                    "Tekrar eden sütunu kaldırın.",
                ));
            }
        }

        // ARC_017: Bilinmeyen sütun (jp_ prefix = GTFS-JP uzantısı, atla)
        let known_cols = known_columns(&raw_name);
        if !known_cols.is_empty() {
            for hdr in &headers {
                if !known_cols.contains(&hdr.as_str()) && !hdr.starts_with("jp_") {
                    notices.push(make_notice(
                        &mut counter, "ARC_017",
                        EntityType::File, Some(raw_name.clone()),
                        Some(&raw_name), Some(1), Some(hdr.as_str()),
                        Some(hdr.clone()),
                        format!("'{raw_name}' dosyasında '{hdr}' GTFS spesifikasyonunda tanımlı değil."),
                        "GTFS standardı dışındaki sütunları kaldırın.",
                    ));
                }
            }
        }

        let header_count = headers.len();
        let req_flds = required_fields(&raw_name);
        // stop_times.stop_id KOŞULLU zorunludur (spec): "Required if stop_times.location_group_id
        // AND stop_times.location_id are NOT defined." Bu ikisinden biri başlıkta varsa dosya
        // Flex kullanıyordur ve stop_id sütunu beklenmez → ARC_025 üretilmez.
        let stop_id_required = raw_name == "stop_times.txt"
            && !headers.iter().any(|h| h == "location_group_id" || h == "location_id");
        let extra_req: &[&str] = if stop_id_required { &["stop_id"] } else { &[] };

        // ARC_025: req_flds'te olup başlıkta OLMAYAN zorunlu sütun (header-level, dosya başına
        // bir kez). MD'nin missing_required_column karşılığı. Değer-boşluğu (sütun var) ilgili
        // k2 grup kuralının konusudur.
        for &f in req_flds.iter().chain(extra_req) {
            if !headers.iter().any(|h| h == f) {
                notices.push(make_notice(
                    &mut counter, "ARC_025",
                    EntityType::File, Some(raw_name.clone()),
                    Some(&raw_name), Some(1), Some(f),
                    Some(String::new()),
                    format!("'{raw_name}' dosyasında zorunlu '{f}' sütunu başlıkta yok."),
                    "Zorunlu sütunu başlığa ekleyin.",
                ));
            }
        }

        // DQ_016: birincil anahtar sütun indeksi döngü-değişmezdir — döngü dışında bir kez hesapla.
        let dq016_pk_idx = headers.iter().position(|h| h.ends_with("_id"));
        let _ = dq016_pk_idx; // özet dosya seviyesinde → satır kimliği gerekmiyor
        let mut dq016 = Dq016Acc::default();
        let mut arc032 = Arc032Acc::default();
        let mut rows: Vec<Vec<SmolStr>> = Vec::new();
        let mut arc021_fired = false;
        let mut arc030_fired = false;
        for (row_idx, row) in records.into_iter().enumerate() {
            let line_num = (row_idx + 2) as u64;

            // ARC_012: Sütun sayısı tutarsız
            // row < header: sondaki boş isteğe bağlı alanlar atlanmış — geçerli CSV pratiği → BİLGİ
            // row > header: fazla alan — kaçmamış virgül veya format hatası → KRİTİK
            if let Some((msg, tip, observed, is_info)) =
                arc012_check(&raw_name, line_num, row.len(), header_count)
            {
                let mut n = make_notice(
                    &mut counter, "ARC_012",
                    EntityType::File, Some(raw_name.clone()),
                    Some(&raw_name), Some(line_num), None,
                    Some(observed),
                    msg, tip,
                );
                if is_info {
                    n.severity = Severity::Bilgi;
                }
                notices.push(n);
            }

            // ARC_018: Boş veri satırı (tüm alanlar boş)
            if row.iter().all(|v| v.trim().is_empty()) {
                notices.push(make_notice(
                    &mut counter, "ARC_018",
                    EntityType::File, Some(raw_name.clone()),
                    Some(&raw_name), Some(line_num), None,
                    None,
                    format!("'{raw_name}' {line_num}. satırı tamamen boş."),
                    "Boş satırları kaldırın.",
                ));
                continue; // Boş satırı kaydetme
            }

            // ARC_021: yazdırılamaz veya sorunlu karakter — geçerli Unicode metni (Japonca vb.) hariç
            if !arc021_fired {
                if let Some(cp) = arc021_bad_char(row.iter().map(|v| v.as_str())) {
                    arc021_fired = true;
                    notices.push(make_notice(
                        &mut counter, "ARC_021",
                        EntityType::File, Some(raw_name.clone()),
                        Some(&raw_name), Some(line_num), None,
                        Some(format!("U+{cp:04X}")),
                        arc021_message(&raw_name, cp),
                        ARC021_REMEDIATION,
                    ));
                }
            }

            // ARC_030: alan değerinde sekme/satır sonu — spec bunları açıkça yasaklar.
            if !arc030_fired {
                if let Some(cp) = arc030_bad_whitespace(row.iter().map(|v| v.as_str())) {
                    arc030_fired = true;
                    notices.push(make_notice(
                        &mut counter, "ARC_030",
                        EntityType::File, Some(raw_name.clone()),
                        Some(&raw_name), Some(line_num), None,
                        Some(format!("U+{cp:04X}")),
                        arc030_message(&raw_name, cp),
                        ARC030_REMEDIATION,
                    ));
                }
            }

            // DQ_016: değerlerde fazladan boşluk — alan ve satır bazında
            // DQ_016: dosya-seviyesi birikim; emit döngü sonrası TEK özet.
            dq016.observe(line_num, row.iter().map(|v| v.as_str()), &headers);

            // ARC_032: HTML etiketi/kaçış dizisi — aynı birikim deseni.
            arc032.observe(line_num, row.iter().map(|v| v.as_str()), &headers);

            rows.push(row);
        }

        // DQ_016: DOSYA başına TEK özet (satır-başına DEĞİL — patlama önlemi, STM_050 deseni).
        if let Some((observed, msg, cols)) = dq016.summary(&raw_name) {
            notices.push(make_notice(
                &mut counter, "DQ_016",
                EntityType::File, Some(raw_name.clone()),
                Some(raw_name.as_str()), dq016.first_line, Some(cols.as_str()),
                Some(observed), msg, DQ016_REMEDIATION,
            ));
        }

        // ARC_032: DOSYA başına TEK özet (DQ_016 ile aynı patlama önlemi).
        if let Some((observed, msg, cols)) = arc032.summary(&raw_name) {
            notices.push(make_notice(
                &mut counter, "ARC_032",
                EntityType::File, Some(raw_name.clone()),
                Some(raw_name.as_str()), arc032.first_line, Some(cols.as_str()),
                Some(observed), msg, ARC032_REMEDIATION,
            ));
        }

        // ARC_022: Dosya satır sayısı limiti (stream_mode'da K2 üretir — rows burada boş)
        const MAX_ROWS: usize = 1_000_000;
        if !stream_mode && rows.len() > MAX_ROWS {
            notices.push(make_notice(
                &mut counter, "ARC_022",
                EntityType::File, Some(raw_name.clone()),
                Some(&raw_name), None, None,
                Some(format!("{}", rows.len())),
                format!("'{raw_name}' dosyasında {} satır var; {} satır sınırını aşıyor.", rows.len(), MAX_ROWS),
                "Dosyayı küçük parçalara bölün veya gereksiz satırları kaldırın.",
            ));
        }

        // Başlık satırından oluşan kayıt için ARC_009 kontrolü (yalnızca başlık var, veri yok).
        // stream_mode'da rows boştur; stop_times/shapes için K2 üretir (total_rows var).
        // calendar_dates.txt stream_mode'da da burada kontrol edilir (K2'nin total_rows'u yok).
        let stream_empty = stream_mode
            && raw_name == "calendar_dates.txt"
            && text.lines().filter(|l| !l.trim().is_empty()).count() <= 1;
        if (!stream_mode && rows.is_empty() || stream_empty)
            && !(raw_name == "calendar_dates.txt" && has_calendar_txt)
        {
            let mut n = make_notice(
                &mut counter, "ARC_009",
                EntityType::File, Some(raw_name.clone()),
                Some(&raw_name), None, None,
                None,
                format!("'{raw_name}' dosyasında başlık satırı var ama veri satırı yok."),
                "Dosyaya en az bir veri satırı ekleyin (veya gereksizse dosyayı kaldırın).",
            );
            // Boş OPSİYONEL dosya KRİTİK değil → Bilgi (bkz. arc009_critical).
            if !arc009_critical(&raw_name) {
                n.severity = Severity::Bilgi;
            }
            notices.push(n);
        }

        // zip_stream partial_read'de bytes = ~8KB snippet; gerçek boyut ZIP metadata'sında.
        // NLL: bytes ödüncü burada biter (son kullanım), ardından raw_vec taşınabilir.
        let bytes_len = if is_zip_stream { uncompressed_hint as u32 } else { bytes.len() as u32 };
        // zip_stream dosyalarda (stop_times, trips, calendar_dates): raw_text=None.
        // K2 zip_bytes alır, ZIP'i yeniden açarak gövdeyi stream eder.
        // shapes.txt: raw_text yoluyla stream (zip_stream değil, raw_text=Some).
        let raw_text = if is_zip_stream {
            None
        } else if stream_mode {
            // shapes.txt — is_required=false, lossy yol kabul edilebilir.
            Some(text.to_string())
        } else {
            None
        };

        k1dbg!("[K1] bitti: {raw_name} ({} satır)", rows.len());
        raw_files.insert(raw_name.clone(), RawFile { name: raw_name, headers, rows, bytes: bytes_len, raw_text });
    }

    // ── Dosya varlık kontrolleri ──────────────────────────────────────────────

    // ARC_004: Zorunlu dosya eksik → Fatal
    //
    // `stops.txt` KOŞULLU zorunludur. Spec (Schedule Reference, dosya tablosu):
    //   "stops.txt — Conditionally Required — Optional if demand-responsive zones are
    //    defined in locations.geojson. Required otherwise."
    // Yalnızca-Flex bir feed hizmetini `locations.geojson` bölgeleriyle tanımlar ve hiç
    // stops.txt taşımayabilir. Koşulsuz listede tutmak, GEÇERLİ bir feed'i ARC_004 Fatal'ı
    // ile tamamen açılamaz yapıyordu — kullanıcı tek bir bulgu bile göremiyordu.
    // İkisi de yoksa hizmetin nerede verildiği hiç tanımlı değildir → yine Fatal.
    let missing: Vec<&str> = REQUIRED_FILES
        .iter()
        .filter(|&&f| !present_files.contains(f))
        .filter(|&&f| !(f == "stops.txt" && has_locations_geojson))
        .copied()
        .collect();

    if !missing.is_empty() {
        for &f in &missing {
            notices.push(make_notice(
                &mut counter, "ARC_004",
                EntityType::Feed, None,
                None, None, None,
                Some(f.to_string()),
                format!("Zorunlu GTFS dosyası eksik: '{f}'."),
                "Eksik dosyayı feed ZIP arşivine ekleyin.",
            ));
        }
        return Err(FatalError {
            code: FatalCode::NoRequiredFiles,
            message: format!("Zorunlu dosyalar eksik: {}", missing.join(", ")),
        });
    }

    // ARC_008: calendar.txt VE calendar_dates.txt ikisi de eksik (Kritik, non-Fatal)
    let has_any_calendar = CALENDAR_FILES
        .iter()
        .any(|&f| present_files.contains(f));
    if !has_any_calendar {
        notices.push(make_notice(
            &mut counter, "ARC_008",
            EntityType::Feed, None,
            None, None, None,
            None,
            "calendar.txt ve calendar_dates.txt dosyalarının ikisi de eksik.".to_string(),
            "En az birini (calendar.txt veya calendar_dates.txt) ZIP'e ekleyin.",
        ));
    }

    // ARC_031: spec dosya düzeyi hükmü — feed_info.txt, translations.txt varsa ZORUNLU.
    // Koşul sağlanmadığında dosya yalnız TAVSİYE edilir; o hâli ARC_020 (Quality) taşır.
    if present_files.contains("translations.txt") && !present_files.contains("feed_info.txt") {
        notices.push(make_notice(
            &mut counter, "ARC_031",
            EntityType::Feed, None,
            None, None, None,
            None,
            "translations.txt var ama feed_info.txt eksik; spec bu durumda feed_info.txt'i zorunlu kılar.".to_string(),
            "feed_info.txt ekleyin ve feed_lang alanını doldurun — çevirilerin hangi dile göre yorumlanacağı buradan okunur.",
        ));
    }

    Ok(K1Result { files: raw_files, notices, geojson_location_ids })
}

// ── locations.geojson validasyon ─────────────────────────────────────────────

fn validate_locations_geojson(bytes: &[u8], fname: &str, notices: &mut Vec<Notice>, counter: &mut u32, geojson_ids: &mut std::collections::HashSet<String>) {
    let text = match std::str::from_utf8(bytes) {
        Ok(s) => s,
        Err(_) => {
            notices.push(make_notice(
                counter, "ARC_002", EntityType::File, Some(fname.to_string()),
                Some(fname), None, None, None,
                format!("'{fname}' UTF-8 kodlamasıyla okunamıyor."),
                "Dosyayı UTF-8 kodlamasıyla yeniden kaydedin.",
            ));
            return;
        }
    };

    let json: serde_json::Value = match serde_json::from_str(text) {
        Ok(v) => v,
        Err(e) => {
            notices.push(make_notice(
                counter, "LOC_001", EntityType::File, Some(fname.to_string()),
                Some(fname), None, None, Some(format!("JSON hatası: {e}")),
                format!("'{fname}' geçerli bir GeoJSON belgesi değil: {e}"),
                "locations.geojson'ın geçerli bir GeoJSON FeatureCollection olduğundan emin olun.",
            ));
            return;
        }
    };

    let root_type = json.get("type").and_then(|t| t.as_str());
    if root_type != Some("FeatureCollection") {
        notices.push(make_notice(
            counter, "LOC_001", EntityType::File, Some(fname.to_string()),
            Some(fname), None, Some("type"), root_type.map(str::to_string),
            format!("'{fname}' kök tipi 'FeatureCollection' olmalıdır; bulundu: '{}'.", root_type.unwrap_or("yok")),
            "locations.geojson dosyasının kök tipi 'FeatureCollection' olmalıdır.",
        ));
        return;
    }

    let Some(features) = json.get("features").and_then(|f| f.as_array()) else { return; };

    // LOC_005: FeatureCollection boş
    if features.is_empty() {
        notices.push(make_notice(
            counter, "LOC_005", EntityType::File, Some(fname.to_string()),
            Some(fname), None, Some("features"), None,
            format!("'{fname}' FeatureCollection'ı boş — hiç feature yok."),
            "GTFS Flex için en az bir Polygon veya MultiPolygon feature ekleyin.",
        ));
        return;
    }

    // LOC_007: Yinelenen feature 'id' değerleri
    {
        let mut seen_ids: std::collections::HashMap<String, usize> = std::collections::HashMap::new();
        for (i, feature) in features.iter().enumerate() {
            if let Some(id_val) = feature.get("id") {
                let id_str = id_val.to_string();
                // XFL_025: stop_times.location_id ham CSV değeriyle eşleşmesi için tırnaksız ham id.
                geojson_ids.insert(id_val.as_str().map(str::to_string).unwrap_or_else(|| id_str.clone()));
                if let Some(first_idx) = seen_ids.get(&id_str) {
                    notices.push(make_notice(
                        counter, "LOC_007", EntityType::File, Some(fname.to_string()),
                        Some(fname), Some((i + 1) as u64), Some("id"), Some(id_str.clone()),
                        format!("'{fname}' özellik {} ve {} aynı 'id' değerine sahip ({id_str}) — GTFS Flex referansı belirsizleşir.", first_idx + 1, i + 1),
                        "Her feature'a benzersiz bir 'id' değeri verin.",
                    ));
                } else {
                    seen_ids.insert(id_str, i);
                }
            }
        }
    }

    for (i, feature) in features.iter().enumerate() {
        let feat_num = (i + 1) as u64;

        // LOC_008: her Feature'ın "type" alanı Required ve değeri "Feature" olmalı (spec tablosu).
        match feature.get("type").and_then(|v| v.as_str()) {
            Some("Feature") => {}
            other => {
                notices.push(make_notice(
                    counter, "LOC_008", EntityType::File, Some(fname.to_string()),
                    Some(fname), Some(feat_num), Some("type"),
                    other.map(str::to_string),
                    format!("'{fname}' özellik {feat_num} için 'type' eksik veya \"Feature\" değil."),
                    "Her feature nesnesine \"type\": \"Feature\" ekleyin.",
                ));
            }
        }

        // LOC_009: "properties" nesnesi Required (spec tablosu). Boş nesne GEÇERLİDİR —
        // stop_name/stop_desc opsiyoneldir; eksik ya da nesne-olmayan değer ihlaldir.
        if !feature.get("properties").is_some_and(|v| v.is_object()) {
            notices.push(make_notice(
                counter, "LOC_009", EntityType::File, Some(fname.to_string()),
                Some(fname), Some(feat_num), Some("properties"), None,
                format!("'{fname}' özellik {feat_num} için 'properties' nesnesi eksik."),
                "Her feature'a bir 'properties' nesnesi ekleyin (boş nesne de geçerlidir).",
            ));
        }

        // LOC_003: Feature'da 'id' property eksik
        if feature.get("id").is_none() {
            notices.push(make_notice(
                counter, "LOC_003", EntityType::File, Some(fname.to_string()),
                Some(fname), Some(feat_num), Some("id"), None,
                format!("'{fname}' özellik {feat_num} için 'id' property eksik — stop_times çapraz referansı için zorunlu."),
                "Her feature'a stop_times.txt'deki location_id değeriyle eşleşen benzersiz bir 'id' ekleyin.",
            ));
        }

        let geometry = feature.get("geometry");

        // LOC_002: Feature'da geometry null veya eksik
        if geometry.map_or(true, |g| g.is_null()) {
            notices.push(make_notice(
                counter, "LOC_002", EntityType::File, Some(fname.to_string()),
                Some(fname), Some(feat_num), Some("geometry"), None,
                format!("'{fname}' özellik {feat_num} için geometry null veya eksik — GTFS Flex gerektiriyor."),
                "Her feature için geçerli bir Polygon veya MultiPolygon geometrisi tanımlayın.",
            ));
            continue;
        }
        let geometry = geometry.unwrap();

        let geom_type = geometry.get("type").and_then(|t| t.as_str());
        match geom_type {
            Some("Polygon") => {
                // LOC_010: "coordinates" Required. Eksik/dizi-olmayan değerde eskiden
                // `if let Some(..)` sessizce atlıyordu → geometri hiç doğrulanmadan geçiyordu.
                match geometry.get("coordinates").and_then(|c| c.as_array()) {
                    Some(rings) => check_polygon_rings(counter, fname, feat_num, rings, notices),
                    None => missing_coordinates(counter, fname, feat_num, notices),
                }
            }
            Some("MultiPolygon") => {
                match geometry.get("coordinates").and_then(|c| c.as_array()) {
                    None => missing_coordinates(counter, fname, feat_num, notices),
                    Some(polygons) => {
                        for poly in polygons {
                            if let Some(rings) = poly.as_array() {
                                check_polygon_rings(counter, fname, feat_num, rings, notices);
                            }
                        }
                    }
                }
            }
            Some(t) => {
                notices.push(make_notice(
                    counter, "LOC_001", EntityType::File, Some(fname.to_string()),
                    Some(fname), Some(feat_num), Some("geometry.type"), Some(t.to_string()),
                    format!("'{fname}' özellik {feat_num} geçersiz geometri tipi: '{t}'. GTFS Flex yalnızca Polygon ve MultiPolygon destekler."),
                    "locations.geojson'da yalnızca Polygon ve MultiPolygon geometrileri kullanın.",
                ));
            }
            None => {
                notices.push(make_notice(
                    counter, "LOC_001", EntityType::File, Some(fname.to_string()),
                    Some(fname), Some(feat_num), Some("geometry.type"), None,
                    format!("'{fname}' özellik {feat_num} için geometri tipi eksik."),
                    "Her feature için Polygon veya MultiPolygon geometrisi tanımlayın.",
                ));
            }
        }
    }
}

/// Polygon ring'leri için LOC_004 (kapalı değil) ve LOC_006 (alan > 500km²) kontrolü.
/// LOC_010: geometry.coordinates eksik veya dizi değil. Spec bu alanı Required işaretler;
/// eksikliğinde bölge geometrisi hiç çözümlenemez, dolayısıyla KRİTİK.
fn missing_coordinates(counter: &mut u32, fname: &str, feat_num: u64, notices: &mut Vec<Notice>) {
    notices.push(make_notice(
        counter, "LOC_010", EntityType::File, Some(fname.to_string()),
        Some(fname), Some(feat_num), Some("coordinates"), None,
        format!("'{fname}' özellik {feat_num} için geometry 'coordinates' eksik veya dizi değil."),
        "Geometriye geçerli bir 'coordinates' dizisi ekleyin.",
    ));
}

fn check_polygon_rings(
    counter: &mut u32,
    fname: &str,
    feat_num: u64,
    rings: &[serde_json::Value],
    notices: &mut Vec<Notice>,
) {
    let mut all_lats = Vec::new();
    let mut all_lons = Vec::new();
    let mut already_reported_closure = false;
    // LOC_006 için ring başına işaretli alan: ilk ring dış sınır, sonrakiler deliktir.
    let mut ring_areas: Vec<f64> = Vec::new();

    for ring in rings {
        let Some(pts) = ring.as_array() else { continue };
        if pts.len() < 2 { continue; }

        // LOC_004: İlk ve son nokta eşit değilse ring kapalı değil
        if !already_reported_closure {
            let first = &pts[0];
            let last  = &pts[pts.len() - 1];
            let first_lon = first.get(0).and_then(|v| v.as_f64());
            let first_lat = first.get(1).and_then(|v| v.as_f64());
            let last_lon  = last.get(0).and_then(|v| v.as_f64());
            let last_lat  = last.get(1).and_then(|v| v.as_f64());
            if let (Some(fl), Some(fa), Some(ll), Some(la)) = (first_lon, first_lat, last_lon, last_lat) {
                if (fl - ll).abs() > 1e-8 || (fa - la).abs() > 1e-8 {
                    notices.push(make_notice(
                        counter, "LOC_004", EntityType::File, Some(fname.to_string()),
                        Some(fname), Some(feat_num), Some("coordinates"), None,
                        format!("'{fname}' özellik {feat_num} Polygon ring'i kapalı değil — ilk nokta [{fl},{fa}] son nokta [{ll},{la}] ile eşleşmiyor."),
                        "GeoJSON Polygon ring'inin ilk ve son koordinatı aynı olmalıdır.",
                    ));
                    already_reported_closure = true;
                }
            }
        }

        // Ring koordinatlarını topla: alan hesabı ve enlem ölçeği için.
        let mut ring_pts: Vec<(f64, f64)> = Vec::with_capacity(pts.len());
        for pt in pts {
            if let (Some(lon), Some(lat)) = (pt.get(0).and_then(|v| v.as_f64()), pt.get(1).and_then(|v| v.as_f64())) {
                all_lats.push(lat);
                all_lons.push(lon);
                ring_pts.push((lon, lat));
            }
        }
        ring_areas.push(signed_ring_area_km2(&ring_pts));
    }

    // LOC_006: GERÇEK poligon alanı > 500km²
    //
    // 2026-08-01'e kadar burada bounding box alanı ölçülüyordu. Sekiz gerçek GTFS-Flex feed'inde
    // ölçüldü: bbox gerçek alanı 1,4×–5,6× şişiriyor, ve kuralın 7 ateşlemesinden 3'ü (Columbia
    // County ×2, Pueblo) SADECE bu şişmeden doğuyordu — gerçek alanları eşiğin altındaydı.
    // Poligonlar dışbükey değil (hizmet bölgeleri idari sınırları izler), dolayısıyla bbox
    // sistematik olarak yukarı saparr. Shoelace ile gerçek alan ölçülür; delikler düşülür.
    if all_lats.len() >= 3 && !ring_areas.is_empty() {
        let outer = ring_areas[0].abs();
        let holes: f64 = ring_areas[1..].iter().map(|a| a.abs()).sum();
        let area_km2 = (outer - holes).max(0.0);
        if area_km2 > 500.0 {
            notices.push(make_notice(
                counter, "LOC_006", EntityType::File, Some(fname.to_string()),
                Some(fname), Some(feat_num), Some("coordinates"), Some(format!("{area_km2:.0}km²")),
                format!("'{fname}' özellik {feat_num} Polygon alanı çok büyük (~{area_km2:.0}km²) — GTFS Flex bölgesi için gerçekçi değil."),
                "Bölge geometrisini gerçek hizmet alanını kapsayacak şekilde küçültün.",
            ));
        }
    }
}

/// Bir GeoJSON ring'inin işaretli alanı (km²), equirectangular izdüşümde shoelace ile.
/// Hizmet bölgeleri küçük olduğu için ring'in kendi ortalama enlemi ölçek olarak yeterlidir;
/// işaret ring yönünü taşır, çağıran mutlak değeri alır.
fn signed_ring_area_km2(pts: &[(f64, f64)]) -> f64 {
    if pts.len() < 3 { return 0.0; }
    let lat0 = pts.iter().map(|p| p.1).sum::<f64>() / pts.len() as f64;
    let kx = 111.320 * lat0.to_radians().cos(); // 1° boylam → km
    let ky = 110.574;                           // 1° enlem   → km
    let mut sum = 0.0;
    for i in 0..pts.len() {
        let (x1, y1) = (pts[i].0 * kx, pts[i].1 * ky);
        let (x2, y2) = (pts[(i + 1) % pts.len()].0 * kx, pts[(i + 1) % pts.len()].1 * ky);
        sum += x1 * y2 - x2 * y1;
    }
    sum / 2.0
}

// ── Testler ───────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    // Notice üretimi notice_factory'ye taşındıktan sonra get_rule yalnız burada
    // (all_notices_have_valid_rule_ids) kullanılıyor.
    use gtfs_rules::get_rule;
    use std::io::Write;
    use zip::write::SimpleFileOptions;

    fn minimal_gtfs_zip() -> Vec<u8> {
        let buf = std::io::Cursor::new(Vec::new());
        let mut zw = zip::ZipWriter::new(buf);
        let opts = SimpleFileOptions::default();

        let files: &[(&str, &str)] = &[
            ("agency.txt",     "agency_id,agency_name,agency_url,agency_timezone\n1,Test,http://x.com,UTC\n"),
            ("stops.txt",      "stop_id,stop_name,stop_lat,stop_lon\nS1,Stop1,41.0,29.0\n"),
            ("routes.txt",     "route_id,agency_id,route_short_name,route_type\nR1,1,101,3\n"),
            ("trips.txt",      "route_id,service_id,trip_id\nR1,SVC1,T1\n"),
            ("stop_times.txt", "trip_id,arrival_time,departure_time,stop_id,stop_sequence\nT1,08:00:00,08:00:00,S1,1\n"),
            ("calendar.txt",   "service_id,monday,tuesday,wednesday,thursday,friday,saturday,sunday,start_date,end_date\nSVC1,1,1,1,1,1,0,0,20240101,20241231\n"),
        ];
        for (name, content) in files {
            zw.start_file(*name, opts).unwrap();
            zw.write_all(content.as_bytes()).unwrap();
        }
        zw.finish().unwrap().into_inner()
    }

    fn zip_with_files(files: &[(&str, &[u8])]) -> Vec<u8> {
        let buf = std::io::Cursor::new(Vec::new());
        let mut zw = zip::ZipWriter::new(buf);
        let opts = SimpleFileOptions::default();
        for (name, content) in files {
            zw.start_file(*name, opts).unwrap();
            zw.write_all(content).unwrap();
        }
        zw.finish().unwrap().into_inner()
    }

    #[test]
    fn minimal_valid_feed_produces_no_error() {
        let zip = minimal_gtfs_zip();
        let result = parse(&zip);
        assert!(result.is_ok(), "Geçerli feed Ok dönmeli: {:?}", result.err());
    }

    #[test]
    fn minimal_valid_feed_has_six_files() {
        let zip = minimal_gtfs_zip();
        let k1 = parse(&zip).unwrap();
        assert_eq!(k1.files.len(), 6);
        assert!(k1.files.contains_key("agency.txt"));
        assert!(k1.files.contains_key("stop_times.txt"));
    }

    #[test]
    fn prealloc_capacity_caps_declared_size() {
        // Devasa beyan → MAX_PREALLOC ile sınırlanır (lying-header DoS koruması).
        assert_eq!(prealloc_capacity(usize::MAX), MAX_PREALLOC);
        assert_eq!(prealloc_capacity(MAX_PREALLOC + 1), MAX_PREALLOC);
        // Sıfır beyan → en az 1 (anlamsız 0-kapasite alloc'unu önle).
        assert_eq!(prealloc_capacity(0), 1);
        // Tavanın altındaki gerçek boyutlar aynen geçer (davranış değişmez).
        assert_eq!(prealloc_capacity(1000), 1000);
        assert_eq!(prealloc_capacity(MAX_PREALLOC), MAX_PREALLOC);
    }

    #[test]
    fn invalid_zip_returns_fatal_zip_unreadable() {
        let err = parse(b"not a zip").expect_err("geçersiz ZIP Fatal olmalı");
        assert_eq!(err.code, FatalCode::ZipUnreadable);
    }

    #[test]
    fn missing_required_file_returns_fatal_no_required_files() {
        // agency.txt olmadan ZIP
        let zip = zip_with_files(&[
            ("stops.txt",      b"stop_id,stop_name,stop_lat,stop_lon\nS1,Stop1,41.0,29.0\n"),
            ("routes.txt",     b"route_id,agency_id,route_short_name,route_type\nR1,1,101,3\n"),
            ("trips.txt",      b"route_id,service_id,trip_id\nR1,SVC1,T1\n"),
            ("stop_times.txt", b"trip_id,arrival_time,departure_time,stop_id,stop_sequence\nT1,08:00:00,08:00:00,S1,1\n"),
            ("calendar.txt",   b"service_id,monday,tuesday,wednesday,thursday,friday,saturday,sunday,start_date,end_date\nSVC1,1,1,1,1,1,0,0,20240101,20241231\n"),
        ]);
        let err = parse(&zip).expect_err("agency.txt eksik Fatal olmalı");
        assert_eq!(err.code, FatalCode::NoRequiredFiles);
        assert!(err.message.contains("agency.txt"), "{}", err.message);
    }

    // ── Tek üst klasöre sarılmış feed (detect_wrapped_root) ───────────────────
    // Gerçek vaka mdb-1814: `TEST GTFS/agency.txt` … geçerli feed, tek klasöre sarılı.
    // Katı davranış kullanıcıya sıfır analiz veriyordu. Hoşgörü DAR; aşağıdaki testler
    // hem kurtarmayı hem de karşıt örneklerin fatal kalmasını sabitler.

    /// Minimal ama TAM bir feed; `prefix` verilirse her dosya o klasörün altına konur.
    fn wrapped_feed_zip(prefix: &str) -> Vec<u8> {
        let p = |name: &str| format!("{prefix}{name}");
        let files: Vec<(String, &[u8])> = vec![
            (p("agency.txt"),     b"agency_id,agency_name,agency_url,agency_timezone\n1,A,https://e.org,Europe/Istanbul\n" as &[u8]),
            (p("stops.txt"),      b"stop_id,stop_name,stop_lat,stop_lon\nS1,Stop1,41.0,29.0\n"),
            (p("routes.txt"),     b"route_id,agency_id,route_short_name,route_type\nR1,1,101,3\n"),
            (p("trips.txt"),      b"route_id,service_id,trip_id\nR1,SVC1,T1\n"),
            (p("stop_times.txt"), b"trip_id,arrival_time,departure_time,stop_id,stop_sequence\nT1,08:00:00,08:00:00,S1,1\n"),
            (p("calendar.txt"),   b"service_id,monday,tuesday,wednesday,thursday,friday,saturday,sunday,start_date,end_date\nSVC1,1,1,1,1,1,0,0,20240101,20241231\n"),
        ];
        zip_with_files(&files.iter().map(|(n, c)| (n.as_str(), *c)).collect::<Vec<_>>())
    }

    #[test]
    fn feed_wrapped_in_single_folder_is_parsed_and_reported() {
        let k1 = parse(&wrapped_feed_zip("TEST GTFS/")).expect("sarılı geçerli feed fatal OLMAMALI");
        // Dosyalar kökteymiş gibi görünür → zorunlu dosya kontrolü geçer.
        for required in REQUIRED_FILES {
            assert!(k1.files.contains_key(*required), "'{required}' kök adıyla görünmeli");
        }
        // Alt dizinde olmaları yine de bulgudur ve GERÇEK yolu gösterir.
        let arc024: Vec<_> = k1.notices.iter().filter(|n| n.rule_id == "ARC_024").collect();
        assert_eq!(arc024.len(), 6, "her .txt için bir ARC_024 beklenir");
        assert!(arc024.iter().all(|n| n.file.as_deref().is_some_and(|f| f.starts_with("TEST GTFS/"))),
            "ARC_024 ZIP'teki gerçek yolu göstermeli");
    }

    #[test]
    fn wrapped_root_is_rejected_when_required_files_are_missing() {
        // GitHub repo zip'i (gerçek vaka mdb-3135): tek klasör ama GTFS değil → fatal kalmalı.
        let zip = zip_with_files(&[
            ("honduras-transit-main/agency.csv", b"a,b\n1,2\n" as &[u8]),
            ("honduras-transit-main/README.md",  b"# repo\n"),
        ]);
        let err = parse(&zip).expect_err("GTFS olmayan sarılı zip fatal kalmalı");
        assert_eq!(err.code, FatalCode::NoRequiredFiles);
    }

    #[test]
    fn wrapped_root_is_rejected_when_more_than_one_folder_holds_txt() {
        let mut files: Vec<(String, &[u8])> = Vec::new();
        for (i, name) in REQUIRED_FILES.iter().enumerate() {
            // Zorunlu dosyalar İKİ ayrı klasöre dağıtılmış → hangisi kök belirsiz, hoşgörü yok.
            files.push((format!("dir{}/{name}", i % 2), b"x\n" as &[u8]));
        }
        let zip = zip_with_files(&files.iter().map(|(n, c)| (n.as_str(), *c)).collect::<Vec<_>>());
        let err = parse(&zip).expect_err("birden fazla üst klasör fatal kalmalı");
        assert_eq!(err.code, FatalCode::NoRequiredFiles);
    }

    #[test]
    fn root_level_feed_is_untouched_by_the_wrapper_tolerance() {
        let k1 = parse(&wrapped_feed_zip("")).expect("normal feed geçerli");
        assert!(k1.notices.iter().all(|n| n.rule_id != "ARC_024"),
            "kökteki feed ARC_024 ALMAMALI");
    }

    /// 8192. bayta denk gelen çok baytlı karakter, GEÇERLİ bir feed'i fatal etmemeli.
    ///
    /// Gerçek vaka: 250-feed corpus koşumunda 3 feed (JP/LT/TH) yalnız bu yüzden
    /// reddediliyordu; MD aynı feed'lerde 0 error raporluyordu. trips.txt #38 ile
    /// kısmi (8 KB) okunur → kesme noktası karakteri bölünce dosya bozuk sanılıyordu.
    /// Yalnız ASCII-dışı trips.txt'i olan feed'leri vurur (ASCII corpus asla yakalamaz).
    #[test]
    fn multibyte_char_across_partial_read_boundary_is_not_fatal() {
        // Kesim K1'de tam 8192 baytta. 3 baytlık 'あ' 8190'da başlarsa baytları
        // 8190/8191/8192'ye düşer → tampon ilk ikisini alır, üçüncüsü kesilir.
        let mut trips = String::from("route_id,service_id,trip_id,trip_headsign\n");
        let mut i = 0;
        while trips.len() + 24 < 8_190 {
            trips.push_str(&format!("R1,SVC1,T{i},AAA\n"));
            i += 1;
        }
        while trips.len() < 8_190 {
            trips.push('A'); // tam 8190'a hizala
        }
        assert_eq!(trips.len(), 8_190, "test kurgusu: hizalama");
        trips.push('あ'); // baytlar 8190,8191,8192 → 8192'deki kesim ORTADAN böler
        trips.push('\n');
        let trips_bytes = trips.as_bytes().to_vec();
        assert!(trips_bytes.len() > 8_192, "test kurgusu: dosya 8KB'ı aşmalı");
        assert!(
            std::str::from_utf8(&trips_bytes[..8_192]).is_err(),
            "test kurgusu: ilk 8192 bayt çok baytlı karakteri bölmeli (yoksa test bugu kanıtlamaz)"
        );
        assert!(std::str::from_utf8(&trips_bytes).is_ok(), "test kurgusu: dosya BÜTÜN olarak geçerli UTF-8");

        let zip = zip_with_files(&[
            ("agency.txt",     b"agency_id,agency_name,agency_url,agency_timezone\n1,Test,http://x.com,UTC\n"),
            ("stops.txt",      b"stop_id,stop_name,stop_lat,stop_lon\nS1,Stop1,41.0,29.0\n"),
            ("routes.txt",     b"route_id,agency_id,route_short_name,route_type\nR1,1,101,3\n"),
            ("trips.txt",      &trips_bytes),
            ("stop_times.txt", b"trip_id,arrival_time,departure_time,stop_id,stop_sequence\nT0,08:00:00,08:00:00,S1,1\n"),
            ("calendar.txt",   b"service_id,monday,tuesday,wednesday,thursday,friday,saturday,sunday,start_date,end_date\nSVC1,1,1,1,1,1,0,0,20240101,20241231\n"),
        ]);
        let res = parse(&zip);
        assert!(res.is_ok(), "geçerli UTF-8 feed fatal olmamalı: {:?}", res.err());
        let notices = &res.unwrap().notices;
        assert!(
            !notices.iter().any(|n| n.rule_id == "ARC_002"),
            "geçerli dosyada ARC_002 (UTF-8 ihlali) üretilmemeli"
        );
    }

    /// Kırpma, GÖVDEDEKİ gerçek bozuk baytları gizlememeli — kısmi okunan dosyada bile.
    #[test]
    fn genuinely_invalid_utf8_in_streamed_file_still_fatal() {
        let mut trips: Vec<u8> = b"route_id,service_id,trip_id\n".to_vec();
        trips.extend_from_slice(b"R1,SVC1,\xFF\xFE_bozuk\n"); // gövdede geçersiz dizi
        while trips.len() < 9_000 {
            trips.extend_from_slice(b"R1,SVC1,T\n");
        }
        let zip = zip_with_files(&[
            ("agency.txt",     b"agency_id,agency_name,agency_url,agency_timezone\n1,Test,http://x.com,UTC\n"),
            ("stops.txt",      b"stop_id,stop_name,stop_lat,stop_lon\nS1,Stop1,41.0,29.0\n"),
            ("routes.txt",     b"route_id,agency_id,route_short_name,route_type\nR1,1,101,3\n"),
            ("trips.txt",      &trips),
            ("stop_times.txt", b"trip_id,arrival_time,departure_time,stop_id,stop_sequence\nT0,08:00:00,08:00:00,S1,1\n"),
            ("calendar.txt",   b"service_id,monday,tuesday,wednesday,thursday,friday,saturday,sunday,start_date,end_date\nSVC1,1,1,1,1,1,0,0,20240101,20241231\n"),
        ]);
        let err = parse(&zip).expect_err("gövdedeki gerçek UTF-8 ihlali hâlâ Fatal olmalı");
        assert_eq!(err.code, FatalCode::Utf8Critical);
    }

    #[test]
    fn utf8_failure_on_required_file_returns_fatal() {
        // stops.txt içinde geçersiz UTF-8 bayt
        let bad_bytes: &[u8] = b"stop_id,stop_name\nS1,\xFF\xFE invalid\n";
        let zip = zip_with_files(&[
            ("agency.txt",     b"agency_id,agency_name,agency_url,agency_timezone\n1,Test,http://x.com,UTC\n"),
            ("stops.txt",      bad_bytes),
            ("routes.txt",     b"route_id,agency_id,route_short_name,route_type\nR1,1,101,3\n"),
            ("trips.txt",      b"route_id,service_id,trip_id\nR1,SVC1,T1\n"),
            ("stop_times.txt", b"trip_id,arrival_time,departure_time,stop_id,stop_sequence\nT1,08:00:00,08:00:00,S1,1\n"),
            ("calendar.txt",   b"service_id,monday,tuesday,wednesday,thursday,friday,saturday,sunday,start_date,end_date\nSVC1,1,1,1,1,1,0,0,20240101,20241231\n"),
        ]);
        let err = parse(&zip).expect_err("UTF-8 hatası Fatal olmalı");
        assert_eq!(err.code, FatalCode::Utf8Critical);
    }

    #[test]
    fn csv_malformed_on_required_file_returns_fatal() {
        // #38: stop_times/trips/calendar_dates K1'de partial (8KB header) okunur → body K2'de.
        // Malformed CSV testi için routes.txt kullanılır (K1'de hâlâ tam okunur).
        let bad_routes = b"route_id,agency_id,route_short_name,route_type\n\"unclosed quote\n";
        let zip = zip_with_files(&[
            ("agency.txt",     b"agency_id,agency_name,agency_url,agency_timezone\n1,Test,http://x.com,UTC\n"),
            ("stops.txt",      b"stop_id,stop_name,stop_lat,stop_lon\nS1,Stop1,41.0,29.0\n"),
            ("routes.txt",     bad_routes),
            ("trips.txt",      b"route_id,service_id,trip_id\nR1,SVC1,T1\n"),
            ("stop_times.txt", b"trip_id,arrival_time,departure_time,stop_id,stop_sequence\nT1,08:00:00,08:00:00,S1,1\n"),
            ("calendar.txt",   b"service_id,monday,tuesday,wednesday,thursday,friday,saturday,sunday,start_date,end_date\nSVC1,1,1,1,1,1,0,0,20240101,20241231\n"),
        ]);
        let err = parse(&zip).expect_err("bozuk CSV Fatal olmalı");
        assert_eq!(err.code, FatalCode::CsvMalformed);
    }

    #[test]
    fn unknown_file_produces_arc007_notice() {
        let zip = zip_with_files(&[
            ("agency.txt",     b"agency_id,agency_name,agency_url,agency_timezone\n1,Test,http://x.com,UTC\n"),
            ("stops.txt",      b"stop_id,stop_name,stop_lat,stop_lon\nS1,Stop1,41.0,29.0\n"),
            ("routes.txt",     b"route_id,agency_id,route_short_name,route_type\nR1,1,101,3\n"),
            ("trips.txt",      b"route_id,service_id,trip_id\nR1,SVC1,T1\n"),
            ("stop_times.txt", b"trip_id,arrival_time,departure_time,stop_id,stop_sequence\nT1,08:00:00,08:00:00,S1,1\n"),
            ("calendar.txt",   b"service_id,monday,tuesday,wednesday,thursday,friday,saturday,sunday,start_date,end_date\nSVC1,1,1,1,1,1,0,0,20240101,20241231\n"),
            ("custom_data.txt",b"foo,bar\n1,2\n"),
        ]);
        let k1 = parse(&zip).unwrap();
        assert!(
            k1.notices.iter().any(|n| n.rule_id == "ARC_007"),
            "ARC_007 notice bekleniyor"
        );
    }

    #[test]
    fn non_txt_unknown_file_produces_arc007() {
        // MobilityData unknown_file paritesi: ZIP içindeki .txt olmayan dosyalar
        // (ör. .pdf/.out) da ARC_007 üretmeli — DELFI feed'inde .pdf + .out vardı.
        let zip = zip_with_files(&[
            ("agency.txt",     b"agency_id,agency_name,agency_url,agency_timezone\n1,Test,http://x.com,UTC\n"),
            ("stops.txt",      b"stop_id,stop_name,stop_lat,stop_lon\nS1,Stop1,41.0,29.0\n"),
            ("routes.txt",     b"route_id,agency_id,route_short_name,route_type\nR1,1,101,3\n"),
            ("trips.txt",      b"route_id,service_id,trip_id\nR1,SVC1,T1\n"),
            ("stop_times.txt", b"trip_id,arrival_time,departure_time,stop_id,stop_sequence\nT1,08:00:00,08:00:00,S1,1\n"),
            ("calendar.txt",   b"service_id,monday,tuesday,wednesday,thursday,friday,saturday,sunday,start_date,end_date\nSVC1,1,1,1,1,1,0,0,20240101,20241231\n"),
            ("readme.pdf",     b"%PDF-1.4 fake\n"),
            ("export.out",     b"legacy dump\n"),
        ]);
        let k1 = parse(&zip).unwrap();
        for f in ["readme.pdf", "export.out"] {
            assert!(
                k1.notices.iter().any(|n| n.rule_id == "ARC_007" && n.file.as_deref() == Some(f)),
                "ARC_007 notice bekleniyor: {f}"
            );
        }
    }

    #[test]
    fn unknown_column_in_fares_v2_produces_arc017() {
        // Regresyon: known_columns Fares v2 dosyalarını içermediği için ARC_017 bu
        // dosyaları tümüyle atlıyordu. rider_categories.txt'te eligibility_url GEÇERLİ
        // (spec), zzz_custom BİLİNMEYEN olmalı.
        let zip = zip_with_files(&[
            ("agency.txt",     b"agency_id,agency_name,agency_url,agency_timezone\n1,Test,http://x.com,UTC\n"),
            ("stops.txt",      b"stop_id,stop_name,stop_lat,stop_lon\nS1,Stop1,41.0,29.0\n"),
            ("routes.txt",     b"route_id,agency_id,route_short_name,route_type\nR1,1,101,3\n"),
            ("trips.txt",      b"route_id,service_id,trip_id\nR1,SVC1,T1\n"),
            ("stop_times.txt", b"trip_id,arrival_time,departure_time,stop_id,stop_sequence\nT1,08:00:00,08:00:00,S1,1\n"),
            ("calendar.txt",   b"service_id,monday,tuesday,wednesday,thursday,friday,saturday,sunday,start_date,end_date\nSVC1,1,1,1,1,1,0,0,20240101,20241231\n"),
            ("rider_categories.txt", b"rider_category_id,rider_category_name,eligibility_url,zzz_custom\nRC1,Adult,http://x.com,foo\n"),
        ]);
        let k1 = parse(&zip).unwrap();
        let arc017_cols: Vec<&str> = k1.notices.iter()
            .filter(|n| n.rule_id == "ARC_017" && n.file.as_deref() == Some("rider_categories.txt"))
            .filter_map(|n| n.field.as_deref())
            .collect();
        assert!(arc017_cols.contains(&"zzz_custom"), "bilinmeyen sütun zzz_custom ARC_017 üretmeli: {arc017_cols:?}");
        assert!(!arc017_cols.contains(&"eligibility_url"), "geçerli sütun eligibility_url ARC_017 üretmemeli");
        assert!(!arc017_cols.contains(&"rider_category_id"), "geçerli sütun rider_category_id ARC_017 üretmemeli");
    }

    #[test]
    fn unknown_column_in_gtfs_jp_produces_arc017() {
        // office_jp.txt: office_id/office_name GEÇERLİ; jp_extra jp_ önekiyle ATLANIR; bad_col BİLİNMEYEN.
        let zip = zip_with_files(&[
            ("agency.txt",     b"agency_id,agency_name,agency_url,agency_timezone\n1,Test,http://x.com,UTC\n"),
            ("stops.txt",      b"stop_id,stop_name,stop_lat,stop_lon\nS1,Stop1,41.0,29.0\n"),
            ("routes.txt",     b"route_id,agency_id,route_short_name,route_type\nR1,1,101,3\n"),
            ("trips.txt",      b"route_id,service_id,trip_id\nR1,SVC1,T1\n"),
            ("stop_times.txt", b"trip_id,arrival_time,departure_time,stop_id,stop_sequence\nT1,08:00:00,08:00:00,S1,1\n"),
            ("calendar.txt",   b"service_id,monday,tuesday,wednesday,thursday,friday,saturday,sunday,start_date,end_date\nSVC1,1,1,1,1,1,0,0,20240101,20241231\n"),
            ("office_jp.txt",  b"office_id,office_name,jp_extra,bad_col\nO1,Ofis,x,y\n"),
        ]);
        let k1 = parse(&zip).unwrap();
        let cols: Vec<&str> = k1.notices.iter()
            .filter(|n| n.rule_id == "ARC_017" && n.file.as_deref() == Some("office_jp.txt"))
            .filter_map(|n| n.field.as_deref())
            .collect();
        assert!(cols.contains(&"bad_col"), "bad_col ARC_017 üretmeli: {cols:?}");
        assert!(!cols.contains(&"office_name"), "office_name geçerli, ARC_017 üretmemeli");
        assert!(!cols.contains(&"jp_extra"), "jp_ önekli sütun atlanmalı");
    }

    #[test]
    fn every_known_file_has_column_list_for_arc017() {
        // ARC_017 (k1: `if !known_cols.is_empty()`) known_columns() boş dönen dosyayı
        // TÜMÜYLE atlar. KNOWN_FILES'taki her dosyanın sütun listesi olmalı; yoksa o
        // dosyadaki bilinmeyen sütunlar sessizce kaçar (bu bug'ın kök nedeniydi).
        let missing: Vec<&str> = KNOWN_FILES.iter()
            .copied()
            .filter(|&f| known_columns(f).is_empty())
            .collect();
        assert!(missing.is_empty(), "known_columns() boş dönen KNOWN_FILES: {missing:?}");
    }

    #[test]
    fn bom_produces_arc010_notice() {
        let bom_csv = b"\xEF\xBB\xBFstop_id,stop_name,stop_lat,stop_lon\nS1,Stop1,41.0,29.0\n";
        let zip = zip_with_files(&[
            ("agency.txt",     b"agency_id,agency_name,agency_url,agency_timezone\n1,Test,http://x.com,UTC\n"),
            ("stops.txt",      bom_csv),
            ("routes.txt",     b"route_id,agency_id,route_short_name,route_type\nR1,1,101,3\n"),
            ("trips.txt",      b"route_id,service_id,trip_id\nR1,SVC1,T1\n"),
            ("stop_times.txt", b"trip_id,arrival_time,departure_time,stop_id,stop_sequence\nT1,08:00:00,08:00:00,S1,1\n"),
            ("calendar.txt",   b"service_id,monday,tuesday,wednesday,thursday,friday,saturday,sunday,start_date,end_date\nSVC1,1,1,1,1,1,0,0,20240101,20241231\n"),
        ]);
        let k1 = parse(&zip).unwrap();
        assert!(
            k1.notices.iter().any(|n| n.rule_id == "ARC_010"),
            "ARC_010 notice bekleniyor"
        );
    }

    #[test]
    fn header_whitespace_produces_arc014_notice() {
        let csv = b"stop_id, stop_name,stop_lat,stop_lon\nS1,Stop1,41.0,29.0\n";
        let zip = zip_with_files(&[
            ("agency.txt",     b"agency_id,agency_name,agency_url,agency_timezone\n1,Test,http://x.com,UTC\n"),
            ("stops.txt",      csv),
            ("routes.txt",     b"route_id,agency_id,route_short_name,route_type\nR1,1,101,3\n"),
            ("trips.txt",      b"route_id,service_id,trip_id\nR1,SVC1,T1\n"),
            ("stop_times.txt", b"trip_id,arrival_time,departure_time,stop_id,stop_sequence\nT1,08:00:00,08:00:00,S1,1\n"),
            ("calendar.txt",   b"service_id,monday,tuesday,wednesday,thursday,friday,saturday,sunday,start_date,end_date\nSVC1,1,1,1,1,1,0,0,20240101,20241231\n"),
        ]);
        let k1 = parse(&zip).unwrap();
        assert!(
            k1.notices.iter().any(|n| n.rule_id == "ARC_014"),
            "ARC_014 notice bekleniyor"
        );
    }

    #[test]
    fn dq016_skips_typed_fields_flags_text() {
        // Zaman alanında baştaki boşluk (' 8:00:00') TİPLİ → DQ_016 YOK; route_long_name'de
        // sondaki boşluk ('Main Line ') METİN → DQ_016 VAR. MD ile hizalı (mdb-2 arrival_time
        // over-fire fix: 65.747 → ~0).
        let zip = zip_with_files(&[
            ("agency.txt",     b"agency_id,agency_name,agency_url,agency_timezone\n1,Test,http://x.com,UTC\n"),
            ("stops.txt",      b"stop_id,stop_name,stop_lat,stop_lon\nS1,Stop1,41.0,29.0\n"),
            ("routes.txt",     b"route_id,agency_id,route_short_name,route_long_name,route_type\nR1,1,101,Main Line ,3\n"),
            ("trips.txt",      b"route_id,service_id,trip_id\nR1,SVC1,T1\n"),
            ("stop_times.txt", b"trip_id,arrival_time,departure_time,stop_id,stop_sequence\nT1, 8:00:00, 8:00:00,S1,1\n"),
            ("calendar.txt",   b"service_id,monday,tuesday,wednesday,thursday,friday,saturday,sunday,start_date,end_date\nSVC1,1,1,1,1,1,0,0,20240101,20241231\n"),
        ]);
        let k1 = parse(&zip).unwrap();
        let dq016: Vec<_> = k1.notices.iter().filter(|n| n.rule_id == "DQ_016").collect();
        assert!(dq016.iter().any(|n| n.message.contains("route_long_name")),
            "route_long_name sondaki boşluk → DQ_016 beklenir: {:?}",
            dq016.iter().map(|n| &n.message).collect::<Vec<_>>());
        assert!(!dq016.iter().any(|n| n.message.contains("arrival_time") || n.message.contains("departure_time")),
            "zaman alanı (tipli) baştaki boşluk DQ_016 ÜRETMEMELİ: {:?}",
            dq016.iter().map(|n| &n.message).collect::<Vec<_>>());
    }

    #[test]
    fn no_calendar_produces_arc008_notice() {
        // calendar.txt ve calendar_dates.txt her ikisi de yok
        let zip = zip_with_files(&[
            ("agency.txt",     b"agency_id,agency_name,agency_url,agency_timezone\n1,Test,http://x.com,UTC\n"),
            ("stops.txt",      b"stop_id,stop_name,stop_lat,stop_lon\nS1,Stop1,41.0,29.0\n"),
            ("routes.txt",     b"route_id,agency_id,route_short_name,route_type\nR1,1,101,3\n"),
            ("trips.txt",      b"route_id,service_id,trip_id\nR1,SVC1,T1\n"),
            ("stop_times.txt", b"trip_id,arrival_time,departure_time,stop_id,stop_sequence\nT1,08:00:00,08:00:00,S1,1\n"),
        ]);
        let k1 = parse(&zip).unwrap();
        assert!(
            k1.notices.iter().any(|n| n.rule_id == "ARC_008"),
            "ARC_008 notice bekleniyor"
        );
    }

    #[test]
    fn empty_calendar_dates_with_calendar_suppresses_arc009() {
        // calendar.txt servisi tanımlıyor + calendar_dates.txt yalnızca başlık →
        // ARC_009 yanlış-pozitif olur, bastırılmalı (calendar_dates şartlı zorunlu).
        let zip = zip_with_files(&[
            ("agency.txt",     b"agency_id,agency_name,agency_url,agency_timezone\n1,Test,http://x.com,UTC\n"),
            ("stops.txt",      b"stop_id,stop_name,stop_lat,stop_lon\nS1,Stop1,41.0,29.0\n"),
            ("routes.txt",     b"route_id,agency_id,route_short_name,route_type\nR1,1,101,3\n"),
            ("trips.txt",      b"route_id,service_id,trip_id\nR1,SVC1,T1\n"),
            ("stop_times.txt", b"trip_id,arrival_time,departure_time,stop_id,stop_sequence\nT1,08:00:00,08:00:00,S1,1\n"),
            ("calendar.txt",   b"service_id,monday,tuesday,wednesday,thursday,friday,saturday,sunday,start_date,end_date\nSVC1,1,1,1,1,1,0,0,20240101,20241231\n"),
            ("calendar_dates.txt", b"service_id,date,exception_type\n"),
        ]);
        let k1 = parse(&zip).unwrap();
        let cd_arc009 = k1.notices.iter().any(|n|
            n.rule_id == "ARC_009" && n.file.as_deref() == Some("calendar_dates.txt"));
        assert!(!cd_arc009, "calendar.txt varken boş calendar_dates.txt ARC_009 üretmemeli");
    }

    #[test]
    fn empty_calendar_dates_without_calendar_produces_arc009() {
        // calendar.txt YOK + calendar_dates.txt yalnızca başlık → hiç servis tanımı yok →
        // ARC_009 KALIR (tek sinyal; ARC_008 calendar_dates mevcut olduğu için fire etmez).
        let zip = zip_with_files(&[
            ("agency.txt",     b"agency_id,agency_name,agency_url,agency_timezone\n1,Test,http://x.com,UTC\n"),
            ("stops.txt",      b"stop_id,stop_name,stop_lat,stop_lon\nS1,Stop1,41.0,29.0\n"),
            ("routes.txt",     b"route_id,agency_id,route_short_name,route_type\nR1,1,101,3\n"),
            ("trips.txt",      b"route_id,service_id,trip_id\nR1,SVC1,T1\n"),
            ("stop_times.txt", b"trip_id,arrival_time,departure_time,stop_id,stop_sequence\nT1,08:00:00,08:00:00,S1,1\n"),
            ("calendar_dates.txt", b"service_id,date,exception_type\n"),
        ]);
        let k1 = parse(&zip).unwrap();
        let cd_arc009 = k1.notices.iter().any(|n|
            n.rule_id == "ARC_009" && n.file.as_deref() == Some("calendar_dates.txt"));
        assert!(cd_arc009, "calendar.txt yokken boş calendar_dates.txt ARC_009 üretmeli");
        assert!(!k1.notices.iter().any(|n| n.rule_id == "ARC_008"),
            "calendar_dates.txt mevcutken ARC_008 fire etmemeli");
    }

    #[test]
    fn empty_optional_file_arc009_is_info_not_critical() {
        // Boş (yalnız-başlık) transfers.txt (OPSİYONEL) → ARC_009 fire eder ama KRİTİK değil, BİLGİ.
        let zip = zip_with_files(&[
            ("agency.txt",     b"agency_id,agency_name,agency_url,agency_timezone\n1,Test,http://x.com,UTC\n"),
            ("stops.txt",      b"stop_id,stop_name,stop_lat,stop_lon\nS1,Stop1,41.0,29.0\n"),
            ("routes.txt",     b"route_id,agency_id,route_short_name,route_type\nR1,1,101,3\n"),
            ("trips.txt",      b"route_id,service_id,trip_id\nR1,SVC1,T1\n"),
            ("stop_times.txt", b"trip_id,arrival_time,departure_time,stop_id,stop_sequence\nT1,08:00:00,08:00:00,S1,1\n"),
            ("calendar.txt",   b"service_id,monday,tuesday,wednesday,thursday,friday,saturday,sunday,start_date,end_date\nSVC1,1,1,1,1,1,0,0,20240101,20241231\n"),
            ("transfers.txt",  b"from_stop_id,to_stop_id,transfer_type,min_transfer_time\n"),
        ]);
        let k1 = parse(&zip).unwrap();
        let n = k1.notices.iter().find(|n|
            n.rule_id == "ARC_009" && n.file.as_deref() == Some("transfers.txt"))
            .expect("boş transfers.txt ARC_009 üretmeli");
        assert_eq!(n.severity, Severity::Bilgi,
            "boş OPSİYONEL dosya ARC_009 BİLGİ olmalı (KRİTİK değil)");
    }

    #[test]
    fn tokenize_csv_quoted_field() {
        let text = "a,\"b,c\",d\n1,\"hello\nworld\",3\n";
        let (records, _) = tokenize_csv(text, None).unwrap();
        assert_eq!(records[0][0], "a");
        assert_eq!(records[0][1], "b,c");
        assert_eq!(records[0][2], "d");
        assert_eq!(records[1][1], "hello\nworld");
    }

    #[test]
    fn tokenize_csv_unclosed_quote_returns_err() {
        assert!(tokenize_csv("\"unclosed\n", None).is_err());
    }

    #[test]
    fn tokenize_csv_trailing_comma_no_newline() {
        // Dosya sondaki virgülle bitip newline olmadığında boş alan eksik sayılmamalı
        let text = "a,b,c\n1,2,";
        let (records, _) = tokenize_csv(text, None).unwrap();
        assert_eq!(records.len(), 2);
        assert_eq!(records[1].len(), 3, "sondaki virgül boş 3. alanı temsil eder");
        assert_eq!(records[1][2], "");
    }

    #[test]
    fn notice_ids_are_unique() {
        let zip = minimal_gtfs_zip();
        let k1 = parse(&zip).unwrap();
        let ids: Vec<&str> = k1.notices.iter().map(|n| n.id.as_str()).collect();
        let unique: HashSet<&str> = ids.iter().copied().collect();
        assert_eq!(ids.len(), unique.len(), "Tekrar eden notice ID var");
    }

    #[test]
    fn all_notices_have_valid_rule_ids() {
        let zip = minimal_gtfs_zip();
        let k1 = parse(&zip).unwrap();
        for n in &k1.notices {
            assert!(
                get_rule(&n.rule_id).is_some(),
                "Geçersiz rule_id: {}", n.rule_id
            );
        }
    }

    #[test]
    fn arc_021_fires_for_non_ascii_char() {
        // stops.txt'de stop_id'ye özel kullanım alanı karakteri (U+E000) — sorunlu karakter
        let zip = zip_with_files(&[
            ("agency.txt",     b"agency_id,agency_name,agency_url,agency_timezone\n1,Test,http://x.com,UTC\n"),
            ("stops.txt",      "stop_id,stop_name,stop_lat,stop_lon\nS\u{E000}1,Durak,41.0,29.0\n".as_bytes()),
            ("routes.txt",     b"route_id,agency_id,route_short_name,route_type\nR1,1,101,3\n"),
            ("trips.txt",      b"route_id,service_id,trip_id\nR1,SVC1,T1\n"),
            ("stop_times.txt", b"trip_id,arrival_time,departure_time,stop_id,stop_sequence\nT1,08:00:00,08:00:00,S1,1\n"),
            ("calendar.txt",   b"service_id,monday,tuesday,wednesday,thursday,friday,saturday,sunday,start_date,end_date\nSVC1,1,1,1,1,1,0,0,20240101,20241231\n"),
        ]);
        let k1 = parse(&zip).unwrap();
        assert!(k1.notices.iter().any(|n| n.rule_id == "ARC_021"), "ARC_021 bekleniyor");
    }

    #[test]
    fn arc_030_fires_for_newline_inside_quoted_value() {
        // Tırnak içindeki satır sonu alan değerinin parçası olur; spec bunu yasaklar.
        let zip = zip_with_files(&[
            ("agency.txt",     "agency_id,agency_name,agency_url,agency_timezone\n1,\"Test\nTransit\",http://x.com,UTC\n".as_bytes()),
            ("stops.txt",      b"stop_id,stop_name,stop_lat,stop_lon\nS1,Durak,41.0,29.0\n"),
            ("routes.txt",     b"route_id,agency_id,route_short_name,route_type\nR1,1,101,3\n"),
            ("trips.txt",      b"route_id,service_id,trip_id\nR1,SVC1,T1\n"),
            ("stop_times.txt", b"trip_id,arrival_time,departure_time,stop_id,stop_sequence\nT1,08:00:00,08:00:00,S1,1\n"),
            ("calendar.txt",   b"service_id,monday,tuesday,wednesday,thursday,friday,saturday,sunday,start_date,end_date\nSVC1,1,1,1,1,1,0,0,20240101,20241231\n"),
        ]);
        let k1 = parse(&zip).unwrap();
        assert!(k1.notices.iter().any(|n| n.rule_id == "ARC_030"), "ARC_030 bekleniyor");
        // ARC_021 bu karakteri bilerek muaf tutar; iki kural çakışmamalı.
        assert!(!k1.notices.iter().any(|n| n.rule_id == "ARC_021"), "ARC_021 tetiklenmemeli");
    }

    #[test]
    fn arc_032_detects_real_markup() {
        // Vahşi doğadan: mdb-1924'ün 1176 durak adı bu biçimde.
        assert_eq!(
            arc032_markup("[KMB+CTB] HIU TSUI STREET/<BR>HIU TSUI STREET, SIU SAI WAN ROAD"),
            Some("<BR>".to_string())
        );
        assert_eq!(arc032_markup("Merkez <b>Durağı</b>"), Some("<b>".to_string()));
        assert_eq!(arc032_markup("Kapanış </p> etiketi"), Some("</p>".to_string()));
        assert_eq!(arc032_markup("<span class=\"x\">A</span>"), Some("<span class=\"x\">".to_string()));
        assert_eq!(arc032_markup("A <!-- yorum --> B"), Some("<!--".to_string()));
        assert_eq!(arc032_markup("Ana&nbsp;Cadde"), Some("&nbsp;".to_string()));
        assert_eq!(arc032_markup("Bir &amp; iki"), Some("&amp;".to_string()));
        assert_eq!(arc032_markup("Kod &#39;tırnak&#39;"), Some("&#39;".to_string()));
        assert_eq!(arc032_markup("Onaltılık &#x27; kaçış"), Some("&#x27;".to_string()));
    }

    #[test]
    fn arc_032_silent_for_legitimate_text() {
        // Bu üç değer ölçüm sırasında GEVŞEK bir desenle yanlış eşleşmişti (1991 uydurma
        // bulgunun kaynağı). Kapalı liste onları geçirmeli — regresyon buraya kilitlendi.
        assert_eq!(arc032_markup("K.A.Wheel&Tire;"), None);           // bilinmeyen "entity"
        assert_eq!(arc032_markup("St Sauvant >< St Césaire"), None);  // etiket değil
        assert_eq!(arc032_markup("La Clisse >< Luchat >< Pisany"), None);
        // Ayrıca: karşılaştırma işaretleri, bilinmeyen etiket adı, kapanmayan etiket.
        assert_eq!(arc032_markup("a < b ve c > d"), None);
        assert_eq!(arc032_markup("<durak>"), None);                   // listede olmayan ad
        assert_eq!(arc032_markup("3 <br sonra"), None);               // `>` yok
        assert_eq!(arc032_markup("Fiyat < 5 TL > indirim"), None);
        assert_eq!(arc032_markup("Şişli Durağı"), None);
        assert_eq!(arc032_markup("東京駅"), None);
    }

    #[test]
    fn arc_032_emits_one_notice_per_file_with_columns() {
        let zip = zip_with_files(&[
            ("agency.txt", b"agency_id,agency_name,agency_url,agency_timezone\nA1,Test,https://a.com,Europe/Istanbul\n" as &[u8]),
            ("stops.txt",  b"stop_id,stop_name,stop_lat,stop_lon\nS1,Ana<br>Durak,41.0,29.0\nS2,Yan&nbsp;Durak,41.1,29.1\n"),
            ("routes.txt", b"route_id,route_short_name,route_type\nR1,1,3\n"),
            ("trips.txt",  b"route_id,service_id,trip_id\nR1,SVC1,T1\n"),
            ("stop_times.txt", b"trip_id,arrival_time,departure_time,stop_id,stop_sequence\nT1,08:00:00,08:00:00,S1,1\n"),
            ("calendar.txt", b"service_id,monday,tuesday,wednesday,thursday,friday,saturday,sunday,start_date,end_date\nSVC1,1,1,1,1,1,0,0,20240101,20241231\n"),
        ]);
        let k1 = parse(&zip).unwrap();
        let hits: Vec<_> = k1.notices.iter().filter(|n| n.rule_id == "ARC_032").collect();
        // İki bozuk satır var ama emit DOSYA başına TEK — DQ_016/STM_050 patlama önlemi.
        assert_eq!(hits.len(), 1, "dosya başına tek notice bekleniyor: {hits:?}");
        assert_eq!(hits[0].field.as_deref(), Some("stop_name"));
        assert!(hits[0].message.contains("2 satırda"), "satır sayısı: {}", hits[0].message);
        assert!(hits[0].message.contains("<br>"), "örnek işaret: {}", hits[0].message);
    }

    #[test]
    fn arc_032_defers_to_stp_023_on_tts_stop_name() {
        // STP_023 tts_stop_name'de HERHANGİ bir `<`/`>` işaretini bildirir ve daha geniştir;
        // ikisini birden emit etmek aynı olguyu iki kez raporlamak olur.
        let mut acc = Arc032Acc::default();
        let headers = vec!["stop_id".to_string(), "tts_stop_name".to_string()];
        acc.observe(2, ["S1", "Ana <b>Durak</b>"].into_iter(), &headers);
        assert!(acc.summary("stops.txt").is_none(), "tts_stop_name ARC_032'ye girmemeli");
    }

    #[test]
    fn arc_030_fires_for_tab_inside_value() {
        // Spec sekmeyi de yasaklar: "must not contain tabs, carriage returns or new lines".
        assert_eq!(arc030_bad_whitespace(["ab\tcd"].into_iter()), Some(9));
        assert_eq!(arc030_bad_whitespace(["ab\rcd"].into_iter()), Some(13));
        assert_eq!(arc030_bad_whitespace(["ab\ncd"].into_iter()), Some(10));
    }

    #[test]
    fn arc_030_silent_for_normal_and_unicode_values() {
        // Boşluk, Türkçe/Japonca metin ve noktalama sorun değildir.
        assert_eq!(arc030_bad_whitespace(["Şişli Durağı", "東京駅", "a, b"].into_iter()), None);
        let k1 = parse(&minimal_gtfs_zip()).unwrap();
        assert!(!k1.notices.iter().any(|n| n.rule_id == "ARC_030"), "ARC_030 tetiklenmemeli");
    }

    #[test]
    fn arc_021_silent_for_ascii_only() {
        let zip = minimal_gtfs_zip(); // yalnızca ASCII içerir
        let k1 = parse(&zip).unwrap();
        assert!(!k1.notices.iter().any(|n| n.rule_id == "ARC_021"), "ARC_021 tetiklenmemeli");
    }

    #[test]
    fn loc_001_fires_for_invalid_geometry_type() {
        // locations.geojson ile birlikte ZIP
        let geojson = br#"{"type":"FeatureCollection","features":[{"type":"Feature","geometry":{"type":"Point","coordinates":[29.0,41.0]},"properties":{}}]}"#;
        let zip = zip_with_files(&[
            ("agency.txt",     b"agency_id,agency_name,agency_url,agency_timezone\n1,Test,http://x.com,UTC\n"),
            ("stops.txt",      b"stop_id,stop_name,stop_lat,stop_lon\nS1,Stop1,41.0,29.0\n"),
            ("routes.txt",     b"route_id,agency_id,route_short_name,route_type\nR1,1,101,3\n"),
            ("trips.txt",      b"route_id,service_id,trip_id\nR1,SVC1,T1\n"),
            ("stop_times.txt", b"trip_id,arrival_time,departure_time,stop_id,stop_sequence\nT1,08:00:00,08:00:00,S1,1\n"),
            ("calendar.txt",   b"service_id,monday,tuesday,wednesday,thursday,friday,saturday,sunday,start_date,end_date\nSVC1,1,1,1,1,1,0,0,20240101,20241231\n"),
            ("locations.geojson", geojson),
        ]);
        let k1 = parse(&zip).unwrap();
        assert!(k1.notices.iter().any(|n| n.rule_id == "LOC_001"), "LOC_001 bekleniyor");
    }

    #[test]
    fn loc_001_silent_for_valid_polygon() {
        let geojson = br#"{"type":"FeatureCollection","features":[{"type":"Feature","geometry":{"type":"Polygon","coordinates":[[[29.0,41.0],[29.1,41.0],[29.1,41.1],[29.0,41.1],[29.0,41.0]]]},"properties":{}}]}"#;
        let zip = zip_with_files(&[
            ("agency.txt",     b"agency_id,agency_name,agency_url,agency_timezone\n1,Test,http://x.com,UTC\n"),
            ("stops.txt",      b"stop_id,stop_name,stop_lat,stop_lon\nS1,Stop1,41.0,29.0\n"),
            ("routes.txt",     b"route_id,agency_id,route_short_name,route_type\nR1,1,101,3\n"),
            ("trips.txt",      b"route_id,service_id,trip_id\nR1,SVC1,T1\n"),
            ("stop_times.txt", b"trip_id,arrival_time,departure_time,stop_id,stop_sequence\nT1,08:00:00,08:00:00,S1,1\n"),
            ("calendar.txt",   b"service_id,monday,tuesday,wednesday,thursday,friday,saturday,sunday,start_date,end_date\nSVC1,1,1,1,1,1,0,0,20240101,20241231\n"),
            ("locations.geojson", geojson),
        ]);
        let k1 = parse(&zip).unwrap();
        assert!(!k1.notices.iter().any(|n| n.rule_id == "LOC_001"), "LOC_001 tetiklenmemeli");
    }

    /// LOC_006 fixture'ı: verilen ring ile bir locations.geojson üretir.
    fn zip_with_polygon(ring: &str) -> Vec<u8> {
        let geojson = format!(
            r#"{{"type":"FeatureCollection","features":[{{"type":"Feature","id":"z1","geometry":{{"type":"Polygon","coordinates":[{ring}]}},"properties":{{}}}}]}}"#
        );
        zip_with_files(&[
            ("agency.txt",     b"agency_id,agency_name,agency_url,agency_timezone\n1,Test,http://x.com,UTC\n"),
            ("stops.txt",      b"stop_id,stop_name,stop_lat,stop_lon\nS1,Stop1,41.0,29.0\n"),
            ("routes.txt",     b"route_id,agency_id,route_short_name,route_type\nR1,1,101,3\n"),
            ("trips.txt",      b"route_id,service_id,trip_id\nR1,SVC1,T1\n"),
            ("stop_times.txt", b"trip_id,arrival_time,departure_time,stop_id,stop_sequence\nT1,08:00:00,08:00:00,S1,1\n"),
            ("calendar.txt",   b"service_id,monday,tuesday,wednesday,thursday,friday,saturday,sunday,start_date,end_date\nSVC1,1,1,1,1,1,0,0,20240101,20241231\n"),
            ("locations.geojson", geojson.as_bytes()),
        ])
    }

    #[test]
    fn loc_006_silent_for_thin_diagonal_zone_with_large_bounding_box() {
        // Çapraz ince şerit: bbox ~4800km² (eski ölçüm ateşlerdi), gerçek alan ~186km².
        // Hizmet bölgeleri dışbükey değildir; bu ayrım kuralın FP'lerinin kaynağıydı.
        let ring = "[[29.0,41.0],[30.0,41.5],[30.0,41.52],[29.0,41.02],[29.0,41.0]]";
        let k1 = parse(&zip_with_polygon(ring)).unwrap();
        assert!(!k1.notices.iter().any(|n| n.rule_id == "LOC_006"),
            "gerçek alanı eşiğin altında olan bölge LOC_006 üretmemeli");
    }

    #[test]
    fn loc_006_fires_for_genuinely_large_zone() {
        // ~1.0° boylam × 0.1° enlem, 41. paralelde ≈ 929km² → eşiğin (500) üstünde.
        let ring = "[[29.0,41.0],[30.0,41.0],[30.0,41.1],[29.0,41.1],[29.0,41.0]]";
        let k1 = parse(&zip_with_polygon(ring)).unwrap();
        assert!(k1.notices.iter().any(|n| n.rule_id == "LOC_006"),
            "gerçekten büyük bölge LOC_006 üretmeli");
    }

    #[test]
    fn loc_006_subtracts_holes_from_the_outer_ring() {
        // Dış ring ~929km², içine ~836km²'lik delik → net ~93km², eşiğin altında.
        let ring = "[[29.0,41.0],[30.0,41.0],[30.0,41.1],[29.0,41.1],[29.0,41.0]],\
                    [[29.05,41.005],[29.95,41.005],[29.95,41.095],[29.05,41.095],[29.05,41.005]]";
        let k1 = parse(&zip_with_polygon(ring)).unwrap();
        assert!(!k1.notices.iter().any(|n| n.rule_id == "LOC_006"),
            "delikler dış ringden düşülmeli");
    }
}
    /// `required_fields` (ARC_025'in dayanağı) spec'in `Required` sütunlarıyla BİREBİR olmalı.
    ///
    /// ARC_025 Kritik·Spec'tir, yani R1'i bloke eder. Listeye KOŞULLU bir alan yazmak geçerli
    /// bir feed'i yayından alıkoyar; gerçekten Required olan bir alanı yazmamak ise hükmü
    /// denetimsiz bırakır. 2026-08-02'de `transfers.txt`'te İKİSİ BİRDEN vardı:
    /// `from_stop_id`/`to_stop_id` koşulluyken listedeydi (yanlış pozitif, gizli),
    /// `transfer_type` koşulsuz Required'ken listede değildi (denetimsiz).
    ///
    /// Kaynak: `spec-audit/spec_fields.json` → `presence == "Required"`.
    #[test]
    fn required_columns_match_the_specification() {
        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../spec-audit/spec_fields.json");
        let doc: serde_json::Value =
            serde_json::from_str(&std::fs::read_to_string(&path).unwrap()).unwrap();
        let files = doc["files"].as_object().unwrap();
        assert!(files.len() >= 25, "spec_fields.json çok küçük — yeniden üretin");

        let mut problems: Vec<String> = Vec::new();
        for (fname, entry) in files {
            let spec_required: std::collections::BTreeSet<&str> = entry["fields"]
                .as_array().unwrap().iter()
                .filter(|f| f["presence"].as_str() == Some("Required"))
                .filter_map(|f| f["name"].as_str())
                .collect();
            let ours: std::collections::BTreeSet<&str> =
                required_fields(fname).iter().copied().collect();
            for miss in spec_required.difference(&ours) {
                problems.push(format!(
                    "{fname}: '{miss}' spec'te Required ama required_fields'ta YOK → ARC_025 denetlemiyor"
                ));
            }
            for extra in ours.difference(&spec_required) {
                problems.push(format!(
                    "{fname}: '{extra}' required_fields'ta ama spec'te Required DEĞİL →                      geçerli feed ARC_025 (Kritik) alabilir"
                ));
            }
        }
        assert!(
            problems.is_empty(),
            "required_fields spec ile uyuşmuyor ({} sorun):\n{}\n\n             Spec değiştiyse önce `python3 spec-audit/extract_fields.py` çalıştırın.",
            problems.len(), problems.join("\n"),
        );
    }

    #[test]
    fn malformed_eol_detects_bare_and_repeated_cr() {
        assert!(has_malformed_eol(b"a\rb\n"));
        assert!(has_malformed_eol(b"a\r\r\nb"));
        assert!(!has_malformed_eol(b"a\r\nb\n"));
        assert!(!has_malformed_eol(b"a\n\nb\n"));
    }
