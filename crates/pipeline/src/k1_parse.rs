use std::collections::{HashMap, HashSet};
use std::io::Read;

use gtfs_core::{EntityType, FatalCode, FatalError, Notice, Severity};
use gtfs_rules::get_rule;
use smol_str::SmolStr;

/// Debug-only K1 izleme. Release WASM derlemesinde (debug_assertions kapalı) ve
/// native'de tamamen derlenip çıkarılır — sıfır runtime maliyeti, hiçbir Notice
/// veya akış etkisi yok. (Profilde DevTools açıkken WASM stack sembolizasyonu
/// her console çağrısını felç edici yavaşlattığı için kapatıldı.)
macro_rules! k1dbg {
    ($($arg:tt)*) => {{
        #[cfg(all(target_arch = "wasm32", debug_assertions))]
        web_sys::console::log_1(&format!($($arg)*).into());
    }};
}

// ── GTFS dosya listeleri ──────────────────────────────────────────────────────

const REQUIRED_FILES: &[&str] = &[
    "agency.txt", "stops.txt", "routes.txt", "trips.txt", "stop_times.txt",
];

const CALENDAR_FILES: &[&str] = &["calendar.txt", "calendar_dates.txt"];

const KNOWN_FILES: &[&str] = &[
    "agency.txt", "stops.txt", "routes.txt", "trips.txt", "stop_times.txt",
    "calendar.txt", "calendar_dates.txt", "shapes.txt", "frequencies.txt",
    "transfers.txt", "fare_attributes.txt", "fare_rules.txt",
    "pathways.txt", "levels.txt", "feed_info.txt", "translations.txt", "attributions.txt",
    "route_networks.txt",
    // Fares v2
    "areas.txt", "stop_areas.txt", "networks.txt",
    "rider_categories.txt", "fare_media.txt", "fare_products.txt",
    "fare_leg_rules.txt", "fare_transfer_rules.txt", "timeframes.txt",
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
    *counter += 1;
    let meta = get_rule(rule_id).unwrap_or_else(|| panic!("K1: bilinmeyen rule_id {rule_id}"));
    Notice {
        id: format!("k1/{rule_id}#{counter}"),
        rule_id: rule_id.to_string(),
        severity: meta.severity,
        rule_class: meta.rule_class,
        entity_type,
        entity_id,
        scope_key: None,
        file: file.map(str::to_string),
        line,
        field: field.map(str::to_string),
        observed_value,
        expected_value: None,
        details: None,
        title: meta.title.to_string(),
        message,
        remediation: remediation.to_string(),
        blocks: meta.blocks.iter().map(|s| s.to_string()).collect(),
        base_effort: meta.base_effort,
        service_id: None,
    }
}

// ── UTF-8 BOM ─────────────────────────────────────────────────────────────────

const UTF8_BOM: &[u8] = &[0xEF, 0xBB, 0xBF];

fn strip_bom(bytes: &[u8]) -> (&[u8], bool) {
    if bytes.starts_with(UTF8_BOM) {
        (&bytes[3..], true)
    } else {
        (bytes, false)
    }
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
fn required_fields(filename: &str) -> &'static [&'static str] {
    match filename {
        "agency.txt"          => &["agency_name", "agency_url", "agency_timezone"],
        "stops.txt"           => &["stop_id"],
        "routes.txt"          => &["route_id", "route_type"],
        "trips.txt"           => &["route_id", "service_id", "trip_id"],
        "stop_times.txt"      => &["trip_id", "stop_id", "stop_sequence"],
        "calendar.txt"        => &["service_id", "monday", "tuesday", "wednesday", "thursday", "friday", "saturday", "sunday", "start_date", "end_date"],
        "calendar_dates.txt"  => &["service_id", "date", "exception_type"],
        "shapes.txt"          => &["shape_id", "shape_pt_lat", "shape_pt_lon", "shape_pt_sequence"],
        "frequencies.txt"     => &["trip_id", "start_time", "end_time", "headway_secs"],
        "transfers.txt"       => &["from_stop_id", "to_stop_id"],
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
        "fare_transfer_rules.txt" => &["fare_transfer_type"],
        "areas.txt"               => &["area_id"],
        "stop_areas.txt"          => &["area_id", "stop_id"],
        "networks.txt"            => &["network_id"],
        "timeframes.txt"          => &["timeframe_group_id", "service_id"],
        "booking_rules.txt"       => &["booking_rule_id", "booking_type"],
        "location_groups.txt"        => &["location_group_id"],
        "location_group_stops.txt"   => &["location_group_id", "stop_id"],
        "attributions.txt"    => &[],
        _                     => &[],
    }
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
        "fare_transfer_rules.txt" => &[
            "from_leg_group_id","to_leg_group_id","transfer_count","duration_limit",
            "duration_limit_type","fare_transfer_type","fare_product_id","rule_priority",
        ],
        "timeframes.txt" => &["timeframe_group_id","start_time","end_time","service_id"],
        "areas.txt" => &["area_id","area_name"],
        "stop_areas.txt" => &["area_id","stop_id"],
        "networks.txt" => &["network_id","network_name"],
        "route_networks.txt" => &["network_id","route_id"],
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
    let has_calendar_txt = archive.file_names().any(|n| n == "calendar.txt");

    let mut notices: Vec<Notice> = Vec::new();
    let mut counter: u32 = 0;
    let mut geojson_location_ids: std::collections::HashSet<String> = std::collections::HashSet::new();
    let mut raw_files: RawFiles = HashMap::new();
    let mut present_files: HashSet<String> = HashSet::new();
    // DQ_016 için dosya başına birincil anahtar sütun indeksi — ilk "*_id" sütunu
    let _dq016_dummy = ();

    for i in 0..archive.len() {
        k1dbg!("[K1] by_index({i})...");
        let mut zf = archive.by_index(i).map_err(|e| FatalError {
            code: FatalCode::ZipUnreadable,
            message: format!("ZIP dosyası okunamadı (index {i}): {e}"),
        })?;

        let raw_name = zf.name().to_string();

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

        // Yalnızca kök dizindeki .txt dosyaları işlenir
        if !raw_name.ends_with(".txt") || raw_name.contains('/') || raw_name.contains('\\') {
            continue;
        }

        present_files.insert(raw_name.clone());
        let is_required = REQUIRED_FILES.contains(&raw_name.as_str());
        let is_known   = KNOWN_FILES.contains(&raw_name.as_str());

        // Bayt oku (ARC_011 için is_known kontrolünden önce yapılır)
        k1dbg!("[K1] okuma başladı: {raw_name} (compressed: {} b)", zf.compressed_size());
        let uncompressed_hint = zf.size() as usize;
        let mut bytes = Vec::with_capacity(uncompressed_hint.max(1));
        {
            let _t = crate::timing::Timer::start(format!("K1::decompress::{raw_name}"));
            zf.read_to_end(&mut bytes).map_err(|e| FatalError {
                code: FatalCode::ZipUnreadable,
                message: format!("'{raw_name}' okunamadı: {e}"),
            })?;
        }

        // ARC_011: Dosya boyutu — tüm kök .txt dosyaları için (bilinmeyen dahil)
        notices.push(make_notice(
            &mut counter, "ARC_011",
            EntityType::File, Some(raw_name.clone()),
            Some(&raw_name), None, None,
            Some(format!("{} bayt", bytes.len())),
            format!("'{raw_name}' boyutu: {} bayt.", bytes.len()),
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
        let (bytes, has_bom) = strip_bom(&bytes);
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

        // OOM fix Plan A: stop_times.txt çok büyük (2.3M+ satır). K1'de tam
        // Vec<Vec<SmolStr>>'e açmak ~714 MB + ~96 sn maliyet. Bu dosyada SADECE
        // header tokenize edilir; gövde ham metin (`raw_text`) olarak K2'ye verilip
        // orada streaming işlenir. Tüm per-satır notice'lar (ARC_012/016/018/021, DQ_016,
        // ARC_022, veri-yok ARC_009) K2 stream geçişine taşındı.
        let stream_mode = raw_name == "stop_times.txt";

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
        if stream_mode && csv_has_unclosed_quote(text) {
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
                notices.push(make_notice(
                    &mut counter, "ARC_009",
                    EntityType::File, Some(raw_name.clone()),
                    Some(&raw_name), None, None,
                    None,
                    format!("'{raw_name}' dosyası boş veya yalnızca başlık satırı içeriyor."),
                    "Dosyaya en az bir veri satırı ekleyin.",
                ));
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
        // ARC_025: req_flds'te olup başlıkta OLMAYAN zorunlu sütun (header-level, dosya başına
        // bir kez). MD'nin missing_required_column karşılığı. Değer-boşluğu (sütun var) ilgili
        // k2 grup kuralının konusudur.
        for &f in req_flds {
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
        let mut rows: Vec<Vec<SmolStr>> = Vec::new();
        let mut arc021_fired = false;
        for (row_idx, row) in records.into_iter().enumerate() {
            let line_num = (row_idx + 2) as u64;

            // ARC_012: Sütun sayısı tutarsız
            // row < header: sondaki boş isteğe bağlı alanlar atlanmış — geçerli CSV pratiği → BİLGİ
            // row > header: fazla alan — kaçmamış virgül veya format hatası → KRİTİK
            if row.len() != header_count {
                let missing = header_count.saturating_sub(row.len());
                let (msg, tip) = if row.len() < header_count {
                    (
                        format!("'{raw_name}' {line_num}. satırda sondaki {missing} isteğe bağlı alan atlanmış ({} sütun, başlık: {header_count}).", row.len()),
                        "CSV'de sondaki boş alanlar atlanabilir; zorunlu alanlar boş bırakılmamalıdır.",
                    )
                } else {
                    (
                        format!("'{raw_name}' {line_num}. satırda fazla alan: {} (beklenen {header_count}) — kaçmamış virgül veya format hatası.", row.len()),
                        "Her satırın başlık sayısı kadar virgülle ayrılmış değer içerdiğinden emin olun.",
                    )
                };
                let mut n = make_notice(
                    &mut counter, "ARC_012",
                    EntityType::File, Some(raw_name.clone()),
                    Some(&raw_name), Some(line_num), None,
                    Some(format!("{} sütun (beklenen {})", row.len(), header_count)),
                    msg, &tip,
                );
                if row.len() < header_count {
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
                'arc021: for val in row.iter() {
                    for ch in val.chars() {
                        let cp = ch as u32;
                        // Geçerli Unicode harf/rakam/boşluk → sorun değil
                        if ch.is_alphanumeric() || ch.is_whitespace() { continue; }
                        // Sorunlu: kontrol karakterleri, DEL, yedek alanlar, özel kullanım alanı
                        let is_bad = (cp < 32 && cp != 9)
                            || cp == 127
                            || (0xD800..=0xDFFF).contains(&cp)
                            || (0xE000..=0xF8FF).contains(&cp)
                            || (0xFFF0..=0xFFFF).contains(&cp);
                        if is_bad {
                            arc021_fired = true;
                            notices.push(make_notice(
                                &mut counter, "ARC_021",
                                EntityType::File, Some(raw_name.clone()),
                                Some(&raw_name), Some(line_num), None,
                                Some(format!("U+{cp:04X}")),
                                format!("'{raw_name}' dosyasında ASCII dışı veya yazdırılamaz karakter içeren değer var (U+{cp:04X})."),
                                "Tüm alan değerlerinin yazdırılabilir ASCII karakter içerdiğinden emin olun.",
                            ));
                            break 'arc021;
                        }
                    }
                }
            }

            // DQ_016: değerlerde fazladan boşluk — alan ve satır bazında
            {
                let pk_idx = dq016_pk_idx;
                let eid: String = pk_idx
                    .and_then(|i| row.get(i))
                    .map(|v| v.as_str())
                    .filter(|v| !v.is_empty())
                    .unwrap_or(&raw_name)
                    .to_string();
                let ws_fields: Vec<&str> = row.iter().enumerate()
                    .filter(|(_, v)| { let s = v.as_str(); s != s.trim() })
                    .filter_map(|(i, _)| headers.get(i).map(|s| s.as_str()))
                    .collect();
                if !ws_fields.is_empty() {
                    let fields_str = ws_fields.join(", ");
                    notices.push(make_notice(
                        &mut counter, "DQ_016",
                        EntityType::Row,
                        Some(eid.clone()),
                        Some(raw_name.as_str()),
                        Some(line_num),
                        Some(fields_str.as_str()),
                        Some(format!("{line_num}")),
                        format!("'{}' kaydında ({}, satır {line_num}): '{}' alanlarında baştaki/sondaki boşluk var.",
                            eid, raw_name, fields_str),
                        "Değerlerdeki gereksiz baştaki/sondaki boşlukları kaldırın.",
                    ));
                }
            }

            rows.push(row);
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

        // Başlık satırından oluşan kayıt için ARC_009 kontrolü (yalnızca başlık var, veri yok)
        // stream_mode'da rows her zaman boştur; "veri yok" tespitini K2 (total_rows) yapar.
        if !stream_mode && rows.is_empty()
            && !(raw_name == "calendar_dates.txt" && has_calendar_txt)
        {
            notices.push(make_notice(
                &mut counter, "ARC_009",
                EntityType::File, Some(raw_name.clone()),
                Some(&raw_name), None, None,
                None,
                format!("'{raw_name}' dosyasında başlık satırı var ama veri satırı yok."),
                "Dosyaya en az bir veri satırı ekleyin.",
            ));
        }

        // stream_mode (stop_times.txt): gövdeyi ham metin olarak sakla; rows boş kalır.
        let raw_text = if stream_mode { Some(text.to_string()) } else { None };

        k1dbg!("[K1] bitti: {raw_name} ({} satır)", rows.len());
        raw_files.insert(raw_name.clone(), RawFile { name: raw_name, headers, rows, bytes: bytes.len() as u32, raw_text });
    }

    // ── Dosya varlık kontrolleri ──────────────────────────────────────────────

    // ARC_004: Zorunlu dosya eksik → Fatal
    let missing: Vec<&str> = REQUIRED_FILES
        .iter()
        .filter(|&&f| !present_files.contains(f))
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
                if let Some(rings) = geometry.get("coordinates").and_then(|c| c.as_array()) {
                    check_polygon_rings(counter, fname, feat_num, rings, notices);
                }
            }
            Some("MultiPolygon") => {
                if let Some(polygons) = geometry.get("coordinates").and_then(|c| c.as_array()) {
                    for poly in polygons {
                        if let Some(rings) = poly.as_array() {
                            check_polygon_rings(counter, fname, feat_num, rings, notices);
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

        // bbox için koordinatları topla
        for pt in pts {
            if let (Some(lon), Some(lat)) = (pt.get(0).and_then(|v| v.as_f64()), pt.get(1).and_then(|v| v.as_f64())) {
                all_lats.push(lat);
                all_lons.push(lon);
            }
        }
    }

    // LOC_006: Bounding box alanı > 500km²
    if all_lats.len() >= 3 {
        let min_lat = all_lats.iter().cloned().fold(f64::INFINITY, f64::min);
        let max_lat = all_lats.iter().cloned().fold(f64::NEG_INFINITY, f64::max);
        let min_lon = all_lons.iter().cloned().fold(f64::INFINITY, f64::min);
        let max_lon = all_lons.iter().cloned().fold(f64::NEG_INFINITY, f64::max);
        let avg_lat = (min_lat + max_lat) / 2.0;
        let lat_km = (max_lat - min_lat) * 111.0;
        let lon_km = (max_lon - min_lon) * 111.0 * avg_lat.to_radians().cos();
        let bbox_km2 = lat_km * lon_km;
        if bbox_km2 > 500.0 {
            notices.push(make_notice(
                counter, "LOC_006", EntityType::File, Some(fname.to_string()),
                Some(fname), Some(feat_num), Some("coordinates"), Some(format!("{bbox_km2:.0}km²")),
                format!("'{fname}' özellik {feat_num} Polygon alanı çok büyük (~{bbox_km2:.0}km²) — GTFS Flex bölgesi için gerçekçi değil."),
                "Bölge geometrisini gerçek hizmet alanını kapsayacak şekilde küçültün.",
            ));
        }
    }
}

// ── Testler ───────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
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
        let bad_csv = b"trip_id,stop_id\n\"unclosed quote\n";
        let zip = zip_with_files(&[
            ("agency.txt",     b"agency_id,agency_name,agency_url,agency_timezone\n1,Test,http://x.com,UTC\n"),
            ("stops.txt",      b"stop_id,stop_name,stop_lat,stop_lon\nS1,Stop1,41.0,29.0\n"),
            ("routes.txt",     b"route_id,agency_id,route_short_name,route_type\nR1,1,101,3\n"),
            ("trips.txt",      b"route_id,service_id,trip_id\nR1,SVC1,T1\n"),
            ("stop_times.txt", bad_csv),
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
}
