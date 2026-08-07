use std::borrow::Cow;
use std::collections::HashMap;

use gtfs_core::{EntityType, Notice};
use gtfs_rules::get_rule;
use smol_str::SmolStr;
use url::Url;

pub type HeaderIndex = HashMap<String, usize>;
pub type RowMap = HashMap<String, String>;

/// K2 modülleri için ortak doğrulama bağlamı.
#[derive(Debug, Clone)]
pub struct K2Context<'a> {
    pub file: &'a str,
    pub headers: &'a [String],
    pub header_index: HeaderIndex,
}

impl<'a> K2Context<'a> {
    pub fn new(file: &'a str, headers: &'a [String]) -> Self {
        Self {
            file,
            headers,
            header_index: build_header_index(headers),
        }
    }
}

/// Header dizisini O(1) lookup için index haritasına çevirir.
pub fn build_header_index(headers: &[String]) -> HeaderIndex {
    headers
        .iter()
        .enumerate()
        .map(|(idx, name)| (name.clone(), idx))
        .collect()
}

/// Bir satırı header adlarıyla hizalı map'e dönüştürür.
///
/// HER başlık map'e girer: satır o sütuna kadar kısaysa değer boş string olur.
/// Böylece `get_trimmed_field` `None` döndürmesi yalnızca "sütun başlıkta hiç yok"
/// anlamına gelir (satır-kısa durumu `Some("")`'tır). Bu ayrım, zorunlu-alan
/// kurallarının sütun-yokken susup ARC_025'e devretmesini sağlar.
pub fn build_row_map(headers: &[String], row: &[SmolStr]) -> RowMap {
    headers
        .iter()
        .enumerate()
        .map(|(i, header)| {
            let value = row.get(i).map(|v| v.to_string()).unwrap_or_default();
            (header.clone(), value)
        })
        .collect()
}

pub fn get_field<'a>(row: &'a RowMap, field: &str) -> Option<&'a str> {
    row.get(field).map(String::as_str)
}

pub fn get_trimmed_field<'a>(row: &'a RowMap, field: &str) -> Option<&'a str> {
    row.get(field).map(String::as_str).map(str::trim)
}

pub fn has_nonempty_field(row: &RowMap, field: &str) -> bool {
    get_trimmed_field(row, field).is_some_and(|v| !v.is_empty())
}

// ── Streaming (stream_mode) yol yardımcıları ─────────────────────────────────
//
// Yukarıdaki `RowMap` tabanlı okuyucular satırı önce map'e çevirir. Streaming yolda
// satır ham `Cow` dilimi olarak gelir ve sütuna indeksle erişilir (map kurulmaz —
// stop_times/shapes ölçeğinde tahsis maliyeti kabul edilemez). İki katman bilinçli
// olarak ayrı; aşağıdakiler `_col` sonekiyle ayırt edilir.

/// Cow dilimden sütun değeri: indeks yoksa veya satır kısaysa boş string.
///
/// `#[inline]` bilinçli: stop_times ölçeğinde (6M+ satır) satır başına birkaç kez
/// çağrılıyor — crate-içi olsa da çağrı maliyeti ölçülebilir.
#[inline]
pub fn get_col<'a>(row: &'a [Cow<'_, str>], col: Option<usize>) -> &'a str {
    col.and_then(|i| row.get(i)).map(|s| s.as_ref().trim()).unwrap_or("")
}

/// Ham değerden u32. Boş → `Ok(None)`, geçersiz → `Err(())`.
/// Hata mesajı ÜRETMEZ — mesaj gerekiyorsa çağıran taraf kurar
/// (bkz. `parse_u32`, `RowMap` sürümü hata metnini kendi döndürür).
#[allow(clippy::result_unit_err)]
pub fn parse_u32_col(raw: &str) -> Result<Option<u32>, ()> {
    if raw.is_empty() {
        return Ok(None);
    }
    raw.parse::<u32>().map(Some).map_err(|_| ())
}

/// Ham değerden f64. Boş → `Ok(None)`, geçersiz → `Err(())`.
#[allow(clippy::result_unit_err)]
/// ⚠️ SONLU OLMAYAN DEĞERLER REDDEDİLİR (issue #82). Rust'ın `f64` ayrıştırıcısı
/// `NaN`, `inf`, `-Infinity` metinlerini KABUL eder; GTFS'in `Float`/`Latitude` tipleri
/// bunları içermez ve bir `NaN` koordinat aşağıdaki her geometri kuralında sessizce
/// "karşılaştırma false" davranışına dönüşür (mesafe hesabı, sıralama, eşik).
pub fn parse_f64_col(raw: &str) -> Result<Option<f64>, ()> {
    if raw.is_empty() {
        return Ok(None);
    }
    match raw.parse::<f64>() {
        Ok(v) if v.is_finite() => Ok(Some(v)),
        _ => Err(()),
    }
}

pub fn parse_f64(row: &RowMap, field: &str) -> Result<Option<f64>, String> {
    let Some(raw) = get_trimmed_field(row, field) else {
        return Ok(None);
    };
    if raw.is_empty() {
        return Ok(None);
    }
    // Ortak yardımcı: akış ve akış-dışı yollar AYNI kararı vermeli (issue #82).
    parse_f64_col(raw)
        .map_err(|_| format!("'{field}' için sonlu bir f64 bekleniyor, alınan: {raw}"))
}

pub fn parse_u32(row: &RowMap, field: &str) -> Result<Option<u32>, String> {
    let Some(raw) = get_trimmed_field(row, field) else {
        return Ok(None);
    };
    if raw.is_empty() {
        return Ok(None);
    }
    raw.parse::<u32>()
        .map(Some)
        .map_err(|_| format!("'{field}' için u32 bekleniyor, alınan: {raw}"))
}

pub fn parse_i32(row: &RowMap, field: &str) -> Result<Option<i32>, String> {
    let Some(raw) = get_trimmed_field(row, field) else {
        return Ok(None);
    };
    if raw.is_empty() {
        return Ok(None);
    }
    raw.parse::<i32>()
        .map(Some)
        .map_err(|_| format!("'{field}' için i32 bekleniyor, alınan: {raw}"))
}

/// GTFS Schedule tarih formatı: YYYYMMDD
pub fn parse_service_date(row: &RowMap, field: &str) -> Result<Option<(u32, u32, u32)>, String> {
    let Some(raw) = get_trimmed_field(row, field) else {
        return Ok(None);
    };
    if raw.is_empty() {
        return Ok(None);
    }
    if raw.len() != 8 || !raw.chars().all(|c| c.is_ascii_digit()) {
        return Err(format!("'{field}' için YYYYMMDD bekleniyor, alınan: {raw}"));
    }

    let year = raw[0..4]
        .parse::<u32>()
        .map_err(|_| format!("'{field}' yıl bölümü geçersiz: {raw}"))?;
    let month = raw[4..6]
        .parse::<u32>()
        .map_err(|_| format!("'{field}' ay bölümü geçersiz: {raw}"))?;
    let day = raw[6..8]
        .parse::<u32>()
        .map_err(|_| format!("'{field}' gün bölümü geçersiz: {raw}"))?;

    // ⚠️ TAKVİM GEÇERLİLİĞİ (issue #82). Eski kod yalnız BİÇİMİ denetliyordu: `20261340`
    // ayrıştırılıp `(2026, 13, 40)` olarak DÖNÜYORDU ve aşağıdaki servis-günü hesapları
    // olmayan bir tarihle çalışıyordu. Akış yolundaki denetim ise `ay ≤ 12, gün ≤ 31`e
    // kadar gelmişti — `20260231` yine geçiyordu. İkisi de artık AYNI yardımcıyı çağırır.
    if !is_valid_calendar_date(year, month, day) {
        return Err(format!("'{field}' takvimde olmayan tarih: {raw}"));
    }

    Ok(Some((year, month, day)))
}

/// Gerçek bir takvim günü mü — artık yıl dahil (issue #82).
pub fn is_valid_calendar_date(year: u32, month: u32, day: u32) -> bool {
    if !(1..=12).contains(&month) || day == 0 {
        return false;
    }
    let leap = (year % 4 == 0 && year % 100 != 0) || year % 400 == 0;
    let max = match month {
        1 | 3 | 5 | 7 | 8 | 10 | 12 => 31,
        4 | 6 | 9 | 11 => 30,
        2 if leap => 29,
        _ => 28,
    };
    day <= max
}

/// GTFS saat formatı: HH:MM:SS (HH 24'ten büyük olabilir).
pub fn parse_gtfs_time(row: &RowMap, field: &str) -> Result<Option<(u32, u32, u32)>, String> {
    let Some(raw) = get_trimmed_field(row, field) else {
        return Ok(None);
    };
    if raw.is_empty() {
        return Ok(None);
    }

    let parts: Vec<&str> = raw.split(':').collect();
    if parts.len() != 3 {
        return Err(format!("'{field}' için HH:MM:SS bekleniyor, alınan: {raw}"));
    }
    // ⚠️ SÖZLÜKSEL GENİŞLİK de tiptir (issue #82): spec `H:MM:SS` veya `HH:MM:SS` der;
    // dakika ve saniye İKİ BASAMAKLIDIR. Eski kod yalnız sayısal ayrıştırma yapıyordu,
    // `1:2:3` geçiyordu. Saat 1–2 basamak olabilir (`25:00:00` servis-günü notasyonu).
    if !gtfs_time_widths_ok(&parts) {
        return Err(format!(
            "'{field}' için [H]H:MM:SS bekleniyor (dakika/saniye iki basamaklı), alınan: {raw}"));
    }

    let hour = parts[0]
        .parse::<u32>()
        .map_err(|_| format!("'{field}' saat bölümü geçersiz: {raw}"))?;
    let minute = parts[1]
        .parse::<u32>()
        .map_err(|_| format!("'{field}' dakika bölümü geçersiz: {raw}"))?;
    let second = parts[2]
        .parse::<u32>()
        .map_err(|_| format!("'{field}' saniye bölümü geçersiz: {raw}"))?;

    if minute > 59 || second > 59 {
        return Err(format!("'{field}' için dakika/saniye aralığı geçersiz: {raw}"));
    }

    Ok(Some((hour, minute, second)))
}

/// GTFS saat biçiminin SÖZLÜKSEL denetimi — `parse_gtfs_time` ve akış tarafındaki
/// `parse_gtfs_time_raw` AYNI kararı vermek zorunda (ikisi ayrışırsa aynı feed akış
/// modunda başka, tam modda başka sonuç verir).
pub fn gtfs_time_widths_ok(parts: &[&str]) -> bool {
    // ⚠️ SAATE BASAMAK SINIRI KONMAZ — bilinçli. İlk taslak `HH`'yi 1–2 basamakla
    // sınırlıyordu ve `tfr_006_does_not_overflow_on_huge_hour` düştü. Asıl mesele test
    // değil: servis günü notasyonunda saat 24'ü aşar (raylı feed'lerde 30:37, 38:10
    // ÖLÇÜLDÜ) ve çok günlü tren seferleri üç basamağa taşabilir. Saati kısıtlamak
    // GEÇERLİ veriyi reddetmek olurdu; issue #82'nin bildirdiği kusur `1:2:3`, yani
    // DAKİKA/SANİYE genişliğidir. Taşma koruması `TFR_006`'da ayrıca duruyor.
    parts.len() == 3
        && !parts[0].is_empty()
        && parts[1].len() == 2
        && parts[2].len() == 2
        && parts.iter().all(|p| p.bytes().all(|b| b.is_ascii_digit()))
}

/// ISO 4217 ALFABETİK kod mu (issue #82).
///
/// ⚠️ Eskiden yalnız "üç büyük ASCII harf" deniyordu: `ZZZ` geçiyordu ve
/// `iso4217_minor_unit` bilinmeyen kodu sessizce 2 ondalıklı sayıyordu — yani uydurma bir
/// para birimi hem geçerli sayılıyor hem de tutar biçimlendirmesi uyduruluyordu.
/// Liste ISO 4217 AKTİF alfabetik kodlarıdır; tarihe karışmış kodlar (ör. `TRL`, `DEM`)
/// bilinçli olarak DIŞARIDADIR — feed bugünkü bir para birimi bildirmelidir.
pub fn is_iso4217(code: &str) -> bool {
    ISO4217_ACTIVE.binary_search(&code).is_ok()
}

/// Sıralı tutulur (`binary_search`). Kaynak: ISO 4217 aktif kod listesi.
const ISO4217_ACTIVE: &[&str] = &[
    "AED", "AFN", "ALL", "AMD", "ANG", "AOA", "ARS", "AUD", "AWG", "AZN", "BAM", "BBD",
    "BDT", "BGN", "BHD", "BIF", "BMD", "BND", "BOB", "BOV", "BRL", "BSD", "BTN", "BWP",
    "BYN", "BZD", "CAD", "CDF", "CHE", "CHF", "CHW", "CLF", "CLP", "CNY", "COP", "COU",
    "CRC", "CUP", "CVE", "CZK", "DJF", "DKK", "DOP", "DZD", "EGP", "ERN", "ETB", "EUR",
    "FJD", "FKP", "GBP", "GEL", "GHS", "GIP", "GMD", "GNF", "GTQ", "GYD", "HKD", "HNL",
    "HTG", "HUF", "IDR", "ILS", "INR", "IQD", "IRR", "ISK", "JMD", "JOD", "JPY", "KES",
    "KGS", "KHR", "KMF", "KPW", "KRW", "KWD", "KYD", "KZT", "LAK", "LBP", "LKR", "LRD",
    "LSL", "LYD", "MAD", "MDL", "MGA", "MKD", "MMK", "MNT", "MOP", "MRU", "MUR", "MVR",
    "MWK", "MXN", "MXV", "MYR", "MZN", "NAD", "NGN", "NIO", "NOK", "NPR", "NZD", "OMR",
    "PAB", "PEN", "PGK", "PHP", "PKR", "PLN", "PYG", "QAR", "RON", "RSD", "RUB", "RWF",
    "SAR", "SBD", "SCR", "SDG", "SEK", "SGD", "SHP", "SLE", "SOS", "SRD", "SSP", "STN",
    "SVC", "SYP", "SZL", "THB", "TJS", "TMT", "TND", "TOP", "TRY", "TTD", "TWD", "TZS",
    "UAH", "UGX", "USD", "USN", "UYI", "UYU", "UYW", "UZS", "VED", "VES", "VND", "VUV",
    "WST", "XAF", "XCD", "XCG", "XDR", "XOF", "XPF", "XSU", "XUA", "YER", "ZAR", "ZMW",
    "ZWG",
];

pub fn validate_enum(value: &str, allowed: &[&str]) -> bool {
    allowed.contains(&value)
}

/// GTFS `URL` tipi: *"A fully qualified URL that includes http:// or https://."*
///
/// `Url::parse` tek başına YETMEZ — o herhangi bir şemayı kabul eder ve `mailto:`,
/// `ftp://`, `file:///`, `javascript:alert(1)`, hatta `foo:bar` için `Ok` döner. Şema
/// kontrolü olmadan bu değerler yolcuya görünen alanlarda geçerli sayılıyordu (2026-08-03
/// ölçümü, T5 boşluk #1).
///
/// Şema adı RFC 3986'ya göre büyük/küçük harfe DUYARSIZDIR (`HTTP://` geçerlidir), bu
/// yüzden karşılaştırma öyle yapılır.
/// ⚠️ Karşılaştırma BAYT üzerinden yapılır, dilim (`&s[..7]`) üzerinden DEĞİL: `&str`
/// dilimlemek çok baytlı bir karakterin ortasına denk gelirse **panik** eder. Alanlar
/// serbest metin taşıyabildiği için `Ünivers…` gibi bir değer bu yolu tetiklerdi.
/// Aradığımız önek saf ASCII olduğundan bayt karşılaştırması hem güvenli hem doğru.
pub fn looks_like_url(value: &str) -> bool {
    fn starts_with_ci(s: &str, prefix: &str) -> bool {
        s.len() >= prefix.len() && s.as_bytes()[..prefix.len()].eq_ignore_ascii_case(prefix.as_bytes())
    }
    let trimmed = value.trim();
    let has_web_scheme = starts_with_ci(trimmed, "http://") || starts_with_ci(trimmed, "https://");
    has_web_scheme && Url::parse(trimmed).is_ok()
}

/// ISO 4217 minor unit (ondalık basamak sayısı). Varsayılan 2; aşağıdakiler ayrıksı.
///
/// Liste kasıtlı olarak KISA: yalnız 0 ve 3 basamaklı para birimleri. Geri kalan her kod
/// 2 varsayılır — ISO 4217'de baskın durum budur ve bilinmeyen bir kod için 2 varsaymak,
/// bilmediğimiz bir birimi yanlış raporlamaktan iyidir.
pub fn iso4217_minor_unit(code: &str) -> u32 {
    const ZERO: &[&str] = &["BIF","CLP","DJF","GNF","ISK","JPY","KMF","KRW","PYG","RWF",
                            "UGX","UYI","VND","VUV","XAF","XOF","XPF"];
    const THREE: &[&str] = &["BHD","IQD","JOD","KWD","LYD","OMR","TND"];
    if ZERO.iter().any(|c| c.eq_ignore_ascii_case(code)) { 0 }
    else if THREE.iter().any(|c| c.eq_ignore_ascii_case(code)) { 3 }
    else { 2 }
}

/// Tutar, para biriminin gerektirdiği ondalık basamak sayısını taşıyor mu?
///
/// Sayı olarak ayrıştırılamayan değerler için `true` döner — o ihlal FPD_002/FAR_002'nin
/// alanıdır ve burada iki kez raporlanmamalı.
pub fn amount_has_iso4217_decimals(amount: &str, currency: &str) -> bool {
    let a = amount.trim();
    if a.is_empty() || currency.trim().len() != 3 || a.parse::<f64>().is_err() {
        return true;
    }
    let dec = a.split_once('.').map(|(_, f)| f.len()).unwrap_or(0);
    dec as u32 == iso4217_minor_unit(currency.trim())
}

pub fn looks_like_email(value: &str) -> bool {
    let trimmed = value.trim();
    let Some((local, domain)) = trimmed.split_once('@') else {
        return false;
    };
    !local.is_empty()
        && !domain.is_empty()
        && !domain.starts_with('.')
        && domain.contains('.')
        && !trimmed.contains(' ')
}

pub fn looks_like_bcp47(value: &str) -> bool {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return false;
    }
    let mut parts = trimmed.split('-');
    // ⚠️ BİRİNCİL ALT ETİKET ALFABETİKTİR (issue #82). Eski kod her alt etikete
    // "alfanümerik, ≤8" diyordu ve `123` dil kodu olarak geçiyordu. BCP 47'de dil alt
    // etiketi harflerden oluşur (2–3 ISO 639, ya da 4–8 kayıtlı); SAYI olamaz.
    // Tek harfli `i`/`x` önekleri grandfathered/private-use dizilerinin başıdır ve
    // geçerlidir — onları reddetmek geçerli etiketi düşürürdü.
    let Some(primary) = parts.next() else { return false };
    let plen = primary.len();
    let primary_ok = primary.chars().all(|c| c.is_ascii_alphabetic())
        && ((2..=8).contains(&plen) || (plen == 1 && matches!(primary, "i" | "x" | "I" | "X")));
    if !primary_ok {
        return false;
    }
    // Sonraki alt etiketler SAYISAL olabilir (bölge kodu `419`, varyant `1901`).
    parts.all(|part| {
        let len = part.len();
        (1..=8).contains(&len) && part.chars().all(|c| c.is_ascii_alphanumeric())
    })
}

/// GTFS `Phone number` tipi. Spec (`agency_phone`): *"Dialable text (for example, TriMet's
/// **"503-238-RIDE"**) is permitted, but the field must not contain any other descriptive
/// text."* Aynı tip `booking_rules.phone_number` için de geçerlidir.
///
/// Kontrol 2026-08-03'e kadar harf içeren HER değeri reddediyordu, yani spec'in kendi
/// örneğini de. Bu `PTH_017` biçimi bir hataydı: açıkça izinli veriyi ihlal saymak.
///
/// **Vanity ile açıklayıcı metin ayrımı:** harf grubu değerin SONUNDA ve TEK olmalıdır —
/// `503-238-RIDE`, `+1 800 FLOWERS` geçerli; `Call 503-238-1234` ("Call" başta),
/// `555-1234 ext 99` ("ext" ortada) geçersiz. Vanity numaralarda harfler numaranın son
/// bloğunu oluşturur; açıklayıcı metin numaranın başına ya da arasına girer.
///
/// ⚠️ Bu sezgisel kuralın ölçülmüş bir faydası YOK: 239 feed'lik korpusta tek bir vanity
/// numara bile geçmiyor. Yazılma gerekçesi spec uyumu; koruma gerekçesi ise ayrımın
/// korpusta gerçekten var olan iki değeri (`80000078 (Liepājā); …`) reddetmeye devam
/// etmesidir — o değerler spec'in yasakladığı açıklayıcı metindir ve kural onlarda DOĞRU.
pub fn looks_like_phone(value: &str) -> bool {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return false;
    }
    // Eşik ÇEVRİLEBİLİR HANE üzerinden: vanity numaralarda harfler rakamların yerini tutar
    // (`+1 800 FLOWERS` yalnız dört rakam taşır ama on bir hanelik bir numaradır). Yine de
    // en az bir rakam aranır, yoksa `FLOWERS` tek başına telefon sayılırdı.
    let digit_count = trimmed.chars().filter(|c| c.is_ascii_digit()).count();
    let dialable_count = trimmed.chars().filter(|c| c.is_ascii_alphanumeric()).count();
    if dialable_count < 5 || digit_count == 0 {
        return false;
    }
    if !trimmed
        .chars()
        .all(|c| c.is_ascii_digit() || c.is_ascii_alphabetic() || matches!(c, '+' | '-' | '(' | ')' | ' ' | '.'))
    {
        return false;
    }
    // Harf grupları: ayırıcılarla bölündüğünde tamamı harften oluşan parçalar.
    let parts: Vec<&str> = trimmed
        .split(|c: char| !c.is_ascii_alphanumeric())
        .filter(|p| !p.is_empty())
        .collect();
    let letter_group_indices: Vec<usize> = parts
        .iter()
        .enumerate()
        .filter(|(_, p)| p.chars().all(|c| c.is_ascii_alphabetic()))
        .map(|(i, _)| i)
        .collect();
    match letter_group_indices.len() {
        0 => true,                                          // saf numara
        1 => letter_group_indices[0] == parts.len() - 1,     // vanity: harf bloğu SONDA
        _ => false,                                          // birden çok harf grubu → açıklayıcı metin
    }
}

pub fn looks_like_iana_timezone(value: &str) -> bool {
    value.parse::<chrono_tz::Tz>().is_ok()
}

pub fn is_hex_color_6(value: &str) -> bool {
    value.len() == 6 && value.chars().all(|c| c.is_ascii_hexdigit())
}

pub fn wcag_contrast_ratio(fg_hex: &str, bg_hex: &str) -> Option<f64> {
    fn channel_to_linear(c: u8) -> f64 {
        let srgb = c as f64 / 255.0;
        if srgb <= 0.03928 {
            srgb / 12.92
        } else {
            ((srgb + 0.055) / 1.055).powf(2.4)
        }
    }

    fn luminance(hex: &str) -> Option<f64> {
        if !is_hex_color_6(hex) {
            return None;
        }
        let r = u8::from_str_radix(&hex[0..2], 16).ok()?;
        let g = u8::from_str_radix(&hex[2..4], 16).ok()?;
        let b = u8::from_str_radix(&hex[4..6], 16).ok()?;
        Some(
            0.2126 * channel_to_linear(r)
                + 0.7152 * channel_to_linear(g)
                + 0.0722 * channel_to_linear(b),
        )
    }

    let fg = luminance(fg_hex)?;
    let bg = luminance(bg_hex)?;
    let (lighter, darker) = if fg >= bg { (fg, bg) } else { (bg, fg) };
    Some((lighter + 0.05) / (darker + 0.05))
}

/// RuleMeta.scope_key_field tanımına göre satırdan scope_key üretir.
///
/// Tek alan için o alanın trimlenmiş değeri döner.
/// Pipe-separated alanlarda eksik bir bileşen varsa `None` döner.
pub fn derive_scope_key(row: &RowMap, scope_key_field: Option<&str>) -> Option<String> {
    let spec = scope_key_field?;
    if spec.contains('|') {
        let mut parts = Vec::new();
        for key in spec.split('|') {
            let value = get_trimmed_field(row, key)?;
            if value.is_empty() {
                return None;
            }
            parts.push(value.to_string());
        }
        Some(parts.join("|"))
    } else {
        let value = get_trimmed_field(row, spec)?;
        if value.is_empty() {
            None
        } else {
            Some(value.to_string())
        }
    }
}

/// K2 modülleri için canonical notice üretir.
/// `presence:required` boşluğu için ortak emit — 12 çağıranı var.
///
/// Neden ayrı bir yardımcı: çapa granülerliği triyajı (2026-08-05) 12 alanda AYNI boşluğu
/// buldu — "X yineleniyor" ve "X bulunamadı" başlıklı kurallar boş değeri hiç görmüyordu.
/// Yinelenme kontrolü boş anahtarları atlar (haklı: boşlar birbirinin kopyası değildir),
/// FK çözümü boşu "referans yok" sayıp geçer; aradaki "bu alan zorunludur" hükmü düşüyordu.
///
/// Alan YOKSA sessiz kalır: sütunun hiç bulunmaması `ARC_025`'in alanıdır, bu kuralların değil.
#[allow(clippy::too_many_arguments)]
pub fn require_nonempty(
    row: &RowMap,
    field: &str,
    rule_id: &'static str,
    entity_type: EntityType,
    entity_id: Option<String>,
    file: &str,
    line: u64,
    message: String,
    remediation: &'static str,
    notices: &mut Vec<Notice>,
    counter: &mut u32,
) {
    // `None` = sütun başlıkta yok → ARC_025; `Some("")` = sütun var, değer boş → bu kural.
    if get_trimmed_field(row, field).is_some_and(str::is_empty) {
        notices.push(make_k2_notice(
            counter, rule_id, entity_type, entity_id, Some(row),
            file, Some(line), Some(field),
            Some(String::new()), Some("(dolu)".to_string()),
            message, remediation,
        ));
    }
}

/// `ARC_033` özeti — DOSYA başına TEK notice. Dört akış dosyası da bunu çağırır;
/// denetimi dört yere kopyalamak `ARC_022`'nin (#75) hatasını tekrarlamak olurdu.
pub fn arc033_summary(
    acc: &crate::k1_parse::Rfc4180Acc,
    file: &str,
    counter: &mut u32,
) -> Option<gtfs_core::Notice> {
    if acc.rows == 0 {
        return None;
    }
    let kind = acc.kind.unwrap_or(crate::k1_parse::RFC4180_BARE_QUOTE);
    let example = acc.example.clone().unwrap_or_default();
    Some(make_k2_notice(
        counter, "ARC_033", EntityType::File, Some(file.to_string()),
        None, file, acc.first_line, None,
        Some(format!("{} satır · {kind}", acc.rows)), None,
        format!("'{file}' RFC 4180'e uymuyor: {kind} ({} satırda; ilk örnek: '{example}').", acc.rows),
        "Tırnak veya virgül içeren alan değerlerini tırnak içine alın; değerin içindeki her \
tırnağı ikiye katlayarak kaçırın (\"12\"\" Street\").",
    ))
}

pub fn make_k2_notice(
    counter: &mut u32,
    rule_id: &str,
    entity_type: EntityType,
    entity_id: Option<String>,
    row: Option<&RowMap>,
    file: &str,
    line: Option<u64>,
    field: Option<&str>,
    observed_value: Option<String>,
    expected_value: Option<String>,
    message: String,
    remediation: &str,
) -> Notice {
    // K2'ye özgü: scope_key satırdan, kuralın kendi scope_key_field tanımına göre türetilir.
    let meta = get_rule(rule_id).unwrap_or_else(|| panic!("K2: bilinmeyen rule_id {rule_id}"));
    let scope_key = row
        .and_then(|r| derive_scope_key(r, meta.scope_key_field))
        .or_else(|| entity_id.clone());

    crate::notice_factory::build(
        "K2", Some("k2"), counter, rule_id, entity_type, entity_id, scope_key,
        Some(file.to_string()), line, field.map(str::to_string),
        observed_value, expected_value, message, remediation,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn build_row_map_aligns_headers() {
        let headers = vec!["a".to_string(), "b".to_string()];
        let row = vec![SmolStr::from("1"), SmolStr::from("2")];
        let map = build_row_map(&headers, &row);
        assert_eq!(get_field(&map, "a"), Some("1"));
        assert_eq!(get_field(&map, "b"), Some("2"));
    }

    #[test]
    fn derive_scope_key_supports_pipe_separated_keys() {
        let row = HashMap::from([
            ("from_stop_id".to_string(), "S1".to_string()),
            ("to_stop_id".to_string(), "S2".to_string()),
        ]);
        let key = derive_scope_key(&row, Some("from_stop_id|to_stop_id"));
        assert_eq!(key.as_deref(), Some("S1|S2"));
    }

    #[test]
    fn parse_gtfs_time_accepts_25h_clock() {
        let row = HashMap::from([("arrival_time".to_string(), "25:10:05".to_string())]);
        let parsed = parse_gtfs_time(&row, "arrival_time").expect("geçerli GTFS saati");
        assert_eq!(parsed, Some((25, 10, 5)));
    }

    #[test]
    fn parse_service_date_rejects_bad_format() {
        let row = HashMap::from([("start_date".to_string(), "2026-05-14".to_string())]);
        assert!(parse_service_date(&row, "start_date").is_err());
    }

    #[test]
    fn timezone_validation_works() {
        assert!(looks_like_iana_timezone("Europe/Istanbul"));
        assert!(!looks_like_iana_timezone("Mars/Base"));
    }

    #[test]
    fn contrast_ratio_black_white_is_high() {
        let ratio = wcag_contrast_ratio("000000", "FFFFFF").expect("hex geçerli");
        assert!(ratio > 20.0);
    }

    #[test]
    fn make_k2_notice_derives_scope_key_from_registry() {
        let row = HashMap::from([("trip_id".to_string(), "T1".to_string())]);
        let mut counter = 0;
        let notice = make_k2_notice(
            &mut counter,
            "STM_001",
            EntityType::Trip,
            Some("T1".to_string()),
            Some(&row),
            "stop_times.txt",
            Some(2),
            Some("trip_id"),
            Some("".to_string()),
            None,
            "trip_id zorunlu".to_string(),
            "Alanı doldurun.",
        );
        assert_eq!(notice.scope_key.as_deref(), Some("T1"));
        assert_eq!(notice.id, "k2/STM_001#1");
    }
    #[test]
    /// issue #82 — paylaşılan tip yardımcıları, temsil ettikleri standarttan GENİŞTİ.
    /// Her biri için: bildirilen GEÇERSİZ örnek + geçerli sınır durumu.
    #[test]
    fn type_helpers_reject_values_outside_the_declared_types() {
        // 1) BCP 47: birincil alt etiket ALFABETİK olmalı.
        assert!(!super::looks_like_bcp47("123"), "sayısal birincil alt etiket dil kodu değildir");
        assert!(!super::looks_like_bcp47("1-tr"), "sayısal birincil alt etiket");
        assert!(super::looks_like_bcp47("tr"));
        assert!(super::looks_like_bcp47("en-US"));
        assert!(super::looks_like_bcp47("es-419"), "BÖLGE kodu sayısal olabilir");
        assert!(super::looks_like_bcp47("zh-Hant-TW"));
        assert!(super::looks_like_bcp47("x-private"), "private-use öneki geçerli");

        // 5) f64: sonlu olmayan değerler tip dışıdır.
        assert!(super::parse_f64_col("NaN").is_err(), "NaN koordinat olamaz");
        assert!(super::parse_f64_col("inf").is_err());
        assert!(super::parse_f64_col("-Infinity").is_err());
        assert_eq!(super::parse_f64_col("41.5").unwrap(), Some(41.5));
        assert_eq!(super::parse_f64_col("").unwrap(), None);

        // 6) Saat: dakika/saniye İKİ basamak; saatte sınır YOK (servis günü).
        assert!(!super::gtfs_time_widths_ok(&["1", "2", "3"]), "1:2:3 tip dışı");
        assert!(!super::gtfs_time_widths_ok(&["08", "5", "00"]));
        assert!(super::gtfs_time_widths_ok(&["8", "05", "00"]), "H:MM:SS geçerli");
        assert!(super::gtfs_time_widths_ok(&["08", "05", "00"]));
        assert!(super::gtfs_time_widths_ok(&["145", "00", "00"]),
                "çok günlü tren seferi — saat basamağı SINIRLANMAZ");

        // 4) ISO 4217: uydurma kod geçmemeli.
        assert!(!super::is_iso4217("ZZZ"), "ZZZ ISO 4217 kodu değildir");
        assert!(!super::is_iso4217("TRL"), "tedavülden kalkmış kod bilinçli olarak dışarıda");
        assert!(!super::is_iso4217("usd"), "kodlar BÜYÜK harftir");
        assert!(super::is_iso4217("TRY") && super::is_iso4217("EUR") && super::is_iso4217("USD"));

        // 2/3) Takvim: biçim doğru ama GÜN yok.
        assert!(!super::is_valid_calendar_date(2026, 13, 40), "13. ay yoktur");
        assert!(!super::is_valid_calendar_date(2026, 2, 31), "31 Şubat yoktur");
        assert!(!super::is_valid_calendar_date(2026, 2, 29), "2026 artık yıl DEĞİL");
        assert!(super::is_valid_calendar_date(2024, 2, 29), "2024 artık yıl");
        assert!(super::is_valid_calendar_date(2026, 12, 31));
    }

    fn looks_like_url_requires_a_web_scheme() {
        // Spec: "A fully qualified URL that includes http:// or https://."
        assert!(super::looks_like_url("http://a.com"));
        assert!(super::looks_like_url("https://a.com/x?y=1#z"));
        assert!(super::looks_like_url("HTTPS://A.COM"), "şema adı büyük/küçük harfe duyarsız");
        assert!(super::looks_like_url("  https://a.com  "), "baştaki/sondaki boşluk kırpılır");

        // Bunların hepsi `Url::parse` için GEÇERLİ ve şema kontrolü eklenmeden önce
        // doğrulamadan geçiyordu (T5 boşluk #1, 2026-08-03 ölçümü).
        assert!(!super::looks_like_url("mailto:info@a.com"));
        assert!(!super::looks_like_url("ftp://a.com/x"));
        assert!(!super::looks_like_url("file:///etc/passwd"));
        assert!(!super::looks_like_url("javascript:alert(1)"));
        assert!(!super::looks_like_url("foo:bar"));
        assert!(!super::looks_like_url("jrutil://invalid"), "korpusta görülen yer tutucu");

        // Şemasız değerler zaten reddediliyordu; davranış korunur.
        assert!(!super::looks_like_url("www.example.com"));
        assert!(!super::looks_like_url("a.com"));
        assert!(!super::looks_like_url(""));
        // `http://` öneki olup URL olarak ayrıştırılamayan değer de reddedilir.
        assert!(!super::looks_like_url("http://"));

        // Çok baytlı karakterle başlayan değer: dilim (`&s[..7]`) kullanılsaydı bayt
        // sınırını böler ve PANİK ederdi. Alanlar serbest metin taşıyabiliyor.
        assert!(!super::looks_like_url("Üniversite Kampüsü"));
        assert!(!super::looks_like_url("東京駅"));
        assert!(!super::looks_like_url("ü"));
    }

    #[test]
    fn looks_like_phone_accepts_dialable_vanity_text() {
        // Spec'in KENDİ örneği — 2026-08-03'e kadar reddediliyordu (PTH_017 biçimi hata).
        assert!(super::looks_like_phone("503-238-RIDE"));
        assert!(super::looks_like_phone("+1 800 FLOWERS"));
        assert!(super::looks_like_phone("1-800-COLLECT"));
        // Saf numaralar: davranış korunur.
        assert!(super::looks_like_phone("+90 212 555 12 34"));
        assert!(super::looks_like_phone("(503) 238-7433"));
        assert!(super::looks_like_phone("555.12.34"));
    }

    #[test]
    fn looks_like_phone_rejects_descriptive_text() {
        // Spec: "must not contain any other descriptive text."
        assert!(!super::looks_like_phone("Call 503-238-1234"), "harf grubu BAŞTA");
        assert!(!super::looks_like_phone("555-1234 ext 99"), "harf grubu ORTADA");
        assert!(!super::looks_like_phone("Call us at 503 238 1234"), "birden çok harf grubu");
        // Korpusta GERÇEKTEN bulunan iki değer (mdb-2337, mdb-992) — kural bunlarda DOĞRU
        // ateşliyor ve düzeltmeden sonra da ateşlemeye devam etmeli.
        assert!(!super::looks_like_phone("80000078 (Liepājā); 80000079 (Pierīgā)"));
        // Yetersiz hane / hiç rakam yok / boş: davranış korunur.
        assert!(!super::looks_like_phone("RIDE"), "beş haneden kısa");
        assert!(!super::looks_like_phone("FLOWERS"), "hiç rakam yok — telefon değil");
        assert!(!super::looks_like_phone("12"));
        assert!(!super::looks_like_phone(""));
    }

    #[test]
    fn iso4217_decimals_follow_the_currency() {
        use super::{amount_has_iso4217_decimals as ok, iso4217_minor_unit as mu};
        assert_eq!(mu("EUR"), 2);
        assert_eq!(mu("JPY"), 0);
        assert_eq!(mu("KWD"), 3);
        assert_eq!(mu("ZZZ"), 2, "bilinmeyen kod 2 varsayılır");

        assert!(ok("2.50", "EUR"));
        assert!(ok("150", "JPY"), "yen ondalık taşımaz");
        assert!(ok("1.500", "KWD"));

        // Korpusta ölçülen gerçek ihlaller.
        assert!(!ok("0.9", "EUR"), "EUR iki basamak ister");
        assert!(!ok("6.7", "HKD"));
        assert!(!ok("8", "EUR"));
        assert!(!ok("100.00", "JPY"), "yen ondalık TAŞIMAMALI");
        assert!(!ok("0.0000", "HKD"), "fazla basamak da ihlal");

        // Sayı olmayan / boş değer FPD_002'nin alanı — burada sessiz.
        assert!(ok("", "EUR"));
        assert!(ok("abc", "EUR"));
        assert!(ok("2.50", ""), "para birimi yoksa karar verilemez");
    }

}
