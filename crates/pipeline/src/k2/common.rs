use std::borrow::Cow;
use std::collections::HashMap;

use gtfs_core::{EntityType, Notice};
use super::bcp47_grandfathered::GRANDFATHERED;
use super::iso4217_generated::ISO4217;
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
/// KİMLİK alanları için HAM değer — `trim` YAPILMAZ (issue #85).
///
/// 🔴 `get_trimmed_field`/`get_col` her değeri kırpıyordu ve ID'ler o kırpılmış değerden
/// kuruluyordu. Sonuç, kimlik semantiğinin sessizce normalize edilmesiydi:
///   · `stops.stop_id=" A "` + `stop_times.stop_id=A` → FK EŞLEŞİYORDU (ölçüldü)
///   · aynı dosyada `A` ve `" A "` → UYDURMA duplicate (`STP_001`, ölçüldü)
/// `" A "` ile `A` aynı sözlüksel değer DEĞİLDİR; PK/FK sert semantiktir ve normalize
/// edilemez. Fazladan boşluk ayrı bir KALİTE sinyalidir (`DQ_016` onu zaten bildiriyor).
///
/// ⚠️ BOŞLUK TEŞHİSİ İÇİN yine `trim` kullanılır — ama kimliği DEĞİŞTİRMEDEN:
/// `get_raw_field(..).filter(|v| !v.trim().is_empty())` deyimi "yalnızca boşluktan oluşan
/// değer YOKTUR" der, kimliği ise ham bırakır.
pub fn get_raw_field<'a>(row: &'a RowMap, field: &str) -> Option<&'a str> {
    row.get(field).map(|s| s.as_str())
}

/// KİMLİK sütunları için ham akış değeri — `get_col`in trim YAPMAYAN eşi (issue #85).
pub fn get_col_raw<'a>(row: &'a [Cow<'_, str>], col: Option<usize>) -> &'a str {
    col.and_then(|i| row.get(i)).map(|s| s.as_ref()).unwrap_or("")
}

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

/// SERT tip predikatları için alan okuması — **HAM değer** (issue #92).
///
/// 🔴 `get_trimmed_field` her değeri kırpıyordu ve tip predikatı kırpılmışını görüyordu:
/// `" https://example.com "` ham hâlde kaçırılmamış boşluk taşır, ama sert URL kuralı
/// susuyordu; geriye yalnız `DQ_016` kalite sinyali kalıyordu. Yani bir Spec iddiası
/// (`P5f72fb5a` KANITLI) normalize edilmiş bir vekil değer üzerinden kuruluyordu.
/// `#85`'in kimlik için verdiği kararın tip predikatlarındaki karşılığı budur.
///
/// ⚠️ **BOŞLUK-YALNIZ değer YOK sayılır** — kırpma yalnız VARLIK teşhisinde kullanılır,
/// değeri değiştirmeden. Aksi hâlde `"   "` taşıyan opsiyonel bir alan "geçersiz tip"
/// olurdu ve bu, `#85`'te bilinçle korunan davranışın tersine dönmesi demekti.
pub fn get_lexical_field<'a>(row: &'a RowMap, field: &str) -> Option<&'a str> {
    let raw = get_raw_field(row, field)?;
    if raw.trim().is_empty() { None } else { Some(raw) }
}

pub fn parse_f64(row: &RowMap, field: &str) -> Result<Option<f64>, String> {
    let Some(raw) = get_lexical_field(row, field) else {
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
    let Some(raw) = get_lexical_field(row, field) else {
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
    let Some(raw) = get_lexical_field(row, field) else {
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
    let Some(raw) = get_lexical_field(row, field) else {
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
    // ⚠️ `gtfs_config::is_leap_year` ile AYNI hesap, ayrı crate'te (o, config aralığı
    // doğrulaması için kullanıyor). İki satırlık evrensel takvim olgusu; bağımlılık
    // eklemek yerine aynı deyim kullanılıyor ki ikisi okurken eşleşsin.
    let leap = (year.is_multiple_of(4) && !year.is_multiple_of(100)) || year.is_multiple_of(400);
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
    let Some(raw) = get_lexical_field(row, field) else {
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
    ISO4217.binary_search_by_key(&code, |(k, _)| *k).is_ok()
}


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
    // ⚠️ İÇ `trim` KALDIRILDI (issue #92): predikat kendisi kırparsa çağıran ham değeri
    // verse bile ölçü yine normalize edilmiş olurdu. Boşluk zaten `url_strict_ok`un
    // reddettiği karakterlerden biri — yani baştaki/sondaki boşluk KAÇIRILMAMIŞ demektir.
    let trimmed = value;
    let has_web_scheme = starts_with_ci(trimmed, "http://") || starts_with_ci(trimmed, "https://");
    has_web_scheme && url_strict_ok(trimmed) && Url::parse(trimmed).is_ok()
}

/// GTFS `URL` tipinin SÖZLÜKSEL yarısı — **KATI**, spec'in kendi ölçüsüyle (issue #80).
///
/// Hükmün tam metni (pinlenmiş katalogdan, `P5f72fb5a`):
/// *"A fully qualified URL that includes http:// or https://, and any special characters
/// in the URL must be correctly escaped."* — ve atıf yaptığı belge W3C'nin URI
/// önerileridir. Orada (ve RFC 3986'da) **ASCII dışı karakter bir URI'de çıplak geçemez**;
/// yüzde kodlanmalıdır.
///
/// 🔴 **ÖNCEKİ TURDA BU YANLIŞ KARARDI.** 2026-08-07 sabahı ham Unicode'u "IRI kullanımı
/// yaygın" diyerek bilinçli kabul etmiştim ve `P5f72fb5a`'yı KANITLI bırakmıştım. Ama o bir
/// ÜRÜN toleransıydı, spec ölçüsü değil — ve hükmün ikinci yarısı ancak spec'in ölçüsüyle
/// ölçülürse KANITLI olabilir. Tolerans ile doğrulama karıştırılamaz.
///
/// Ölçüm (20 feed · 12.897 URL): çıplak özel karakter 0 · bozuk yüzde kaçışı 0 ·
/// **ASCII dışı 0**. Yani katı ölçü bugünkü veride hiçbir feed'i etkilemiyor.
///
/// ⚠️ Ayrı bir "toleranslı" predikat EKLENMEDİ: onu çağıran kimse olmayacaktı ve çağrılmayan
/// bir gevşetme, ileride kimin hangi ölçüyü kullandığını belirsizleştirirdi.
pub fn url_strict_ok(value: &str) -> bool {
    value.is_ascii() && url_escaping_ok(value)
}

/// GTFS `URL` tipinin İKİNCİ yarısı: *"…any special characters … correctly escaped."*
///
/// 🔴 `Url::parse` bunu KANITLAMAZ (issue #80). Ayrıştırıcı bilinçli olarak toleranslıdır
/// ve girdiyi NORMALİZE eder; 2026-08-07'de ölçüldü:
/// ```text
///   "https://a.example/a b"    → Ok, a%20b   (kaçırılmamış boşluk sessizce kodlandı)
///   "https://a.example/a"b"   → Ok, a%22b
///   "https://a.example/a<b>"   → Ok, a%3Cb%3E
///   "https://a.example/a%zzb"  → Ok, OLDUĞU GİBİ (BOZUK yüzde kodlaması kabul edildi)
///   "https://a.example/a\b"   → Ok, a/b      (ters bölü eğik çizgiye çevrildi)
/// ```
/// Yani "parse ediliyor" ile "üretici doğru kaçırmış" AYNI ŞEY DEĞİLDİ; hükmün ikinci
/// yarısı kanıtsız sayılıyordu.
///
/// ⚠️ **ASCII DIŞI karakterler REDDEDİLMEZ** — bilinçli kalıntı. Spec "özel karakter"
/// der; `ü`, `ş`, kiril vb. RFC 3986'nın ayraç kümesinde değildir ve RFC 3987 (IRI)
/// kullanımı yaygındır. Reddetmek uluslararası feed'leri kırardı. Ölçüm: 20 feed ·
/// 12.897 URL'de ASCII dışı karaktere HİÇ rastlanmadı, yani bu yönde kanıt da yok.
pub fn url_escaping_ok(value: &str) -> bool {
    // RFC 3986'da bir URI'de ÇIPLAK geçemeyecek karakterler (ayrıştırıcının sessizce
    // kodladıkları) + kontrol karakterleri.
    const MUST_ESCAPE: &[char] = &[' ', '"', '<', '>', '\\', '^', '`', '{', '}', '|'];
    if value.chars().any(|c| MUST_ESCAPE.contains(&c) || c.is_control()) {
        return false;
    }
    // Yüzde kaçışı İKİ onaltılık basamak ister; `%zz` ve satır sonundaki `%` bozuktur.
    let b = value.as_bytes();
    let mut i = 0;
    while i < b.len() {
        if b[i] == b'%' {
            if i + 2 >= b.len()
                || !b[i + 1].is_ascii_hexdigit()
                || !b[i + 2].is_ascii_hexdigit()
            {
                return false;
            }
            i += 3;
        } else {
            i += 1;
        }
    }
    true
}

/// ISO 4217 minor unit — **kod GEÇERLİLİĞİNDEN AYRI kavram** (issue #82).
///
/// `None` iki AYRI şey demektir ve çağıran ikisini de aynı biçimde ele alamaz:
///   · kod tabloda YOK → geçersiz kod; onu `FAR_003`/`FPD_003` bildirir, burada değil
///   · ondalık TANIMSIZ (`255`, kaynakta `N.A.`) → fonlar/kıymetli madenler (`XAU`, `XDR`)
///
/// 🔴 Eski sürüm "0/2/3 dışında değer yok" VARSAYIYORDU ve bilinmeyen her kodu sessizce
/// 2 ondalıklı sayıyordu. Ölçüldü: `CLF` ve `UYW` **4** ondalıklıdır, yani varsayım yanlıştı
/// ve uydurma bir kod (`ZZZ`) da uydurma bir biçimlendirme alıyordu.
pub fn iso4217_minor_unit(code: &str) -> Option<u8> {
    let c = code.trim();
    ISO4217
        .binary_search_by_key(&c, |(k, _)| *k)
        .ok()
        .map(|i| ISO4217[i].1)
        .filter(|&u| u != 255)
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
    // Kod geçersizse ya da ondalığı tanımsızsa (fon/kıymetli maden) BURADA rapor yok:
    // geçersiz kodu `FAR_003`/`FPD_003` bildirir, iki kez raporlamak olur.
    match iso4217_minor_unit(currency.trim()) {
        Some(units) => dec as u8 == units,
        None => true,
    }
}

/// GTFS `Email` tipi — **SÖZDİZİMİ** doğrulaması (issue #86).
///
/// 🔴 Eski predikat `split_once('@')` kullanıyordu ve ilk `@`'dan sonrasını alan adı
/// sayıyordu: `a@@b.com` için `local="a"`, `domain="@b.com"` çıkıyor ve dört koşulun
/// dördü de geçiyordu. Yani ikinci bir `@` HİÇ görülmüyordu.
///
/// ## Hangi dilbilgisi seviyesi (kartlar bundan fazlasını iddia ETMEMELİ)
///
/// RFC 5322 `addr-spec`in **dot-atom** biçimi. DESTEKLENEN: `atext` karakterleri
/// (`A-Za-z0-9` ve ``!#$%&'*+-/=?^_`{|}~``), noktayla ayrılmış atomlar, en az iki etiketli
/// alan adı. DESTEKLENMEYEN ve bilinçli olarak GEÇERSİZ sayılan: tırnaklı local-part
/// (`"a b"@x.com`), yorum/`CFWS`, IP-literal alan adı (`a@[192.0.2.1]`), kaynak yönlendirme.
/// Bunlar RFC'de geçerlidir ama bir GTFS iletişim alanında gerçekçi değildir; kabul etmek
/// predikatı savunulamaz biçimde genişletirdi.
///
/// ⚠️ **TESLİM EDİLEBİLİRLİK ÖLÇÜLMEZ** — DNS/MX sorgusu yok, olamaz da: doğrulayıcı ağa
/// çıkmaz. "Sözdizimi geçerli" ile "adres çalışıyor" ayrı şeylerdir.
///
/// ⚠️ ASCII DIŞI karakterler kabul edilir (EAI / SMTPUTF8, RFC 6531): `info@şehir.example`
/// gerçek bir adres olabilir ve reddetmek uluslararası feed'leri kırardı. Ölçüm: 20 korpus
/// feed'inde ASCII dışı e-postaya rastlanmadı, yani bu yönde karşı kanıt da yok.
pub fn looks_like_email(value: &str) -> bool {
    // ⚠️ İÇ `trim` yok (issue #92): boşluk zaten aşağıda reddedilir ve ham değer ölçülür.
    let trimmed = value;
    // Ayraç TAM OLARAK BİR tane olmalı. Eski kodun kaçırdığı sınıf tam burasıydı.
    let mut parts = trimmed.split('@');
    let (Some(local), Some(domain), None) = (parts.next(), parts.next(), parts.next()) else {
        return false;
    };
    if local.is_empty() || local.len() > 64 || domain.is_empty() || domain.len() > 255 {
        return false;
    }
    if trimmed.chars().any(|c| c.is_whitespace() || c.is_control()) {
        return false;
    }
    // local-part: nokta ile ayrılmış atomlar; boş atom, baştaki/sondaki nokta yok.
    let atext = |c: char| {
        c.is_alphanumeric()
            || "!#$%&'*+-/=?^_`{|}~".contains(c)
            || (!c.is_ascii() && !c.is_whitespace() && !c.is_control())
    };
    if !local.split('.').all(|atom| !atom.is_empty() && atom.chars().all(atext)) {
        return false;
    }
    // domain: en az iki etiket; her etiket harf/rakam ile başlar ve biter, içinde tire olabilir.
    let labels: Vec<&str> = domain.split('.').collect();
    if labels.len() < 2 {
        return false;
    }
    let label_ok = |l: &str| {
        !l.is_empty()
            && l.len() <= 63
            && !l.starts_with('-')
            && !l.ends_with('-')
            && l.chars().all(|c| c.is_alphanumeric() || c == '-')
    };
    if !labels.iter().all(|l| label_ok(l)) {
        return false;
    }
    // Son etiket (TLD) sayısal olamaz — `a@b.1` adres değildir.
    labels.last().is_some_and(|tld| tld.chars().all(|c| c.is_alphabetic()) && tld.len() >= 2)
}

/// GTFS `Language` tipi — **RFC 5646 (BCP 47) well-formed** sözdizimi (issue #82).
///
/// 🔴 Eski predikat yalnız "alfanümerik, ≤8" diyordu ve şu üçü GEÇİYORDU (ölçüldü):
/// `en-a` (uzantısı olmayan singleton) · `en-a-b-c` (boş uzantı dizisi) ·
/// `en-a-bbb-a-ccc` (YİNELENEN singleton). Bunlar RFC 5646'da well-formed DEĞİLDİR.
///
/// Ölçülen dilbilgisi (`langtag`):
/// ```text
/// language ["-" script] ["-" region] *("-" variant) *("-" extension) ["-" privateuse]
///   language   2-3 alpha [3 kez 3-alpha extlang] | 4 alpha (ayrılmış) | 5-8 alpha
///   script     4 alpha            region   2 alpha | 3 digit
///   variant    5-8 alphanum | digit + 3 alphanum
///   extension  singleton (x hariç alphanum) + 1..* (2-8 alphanum)   ← singleton BENZERSİZ
///   privateuse "x" + 1..* (1-8 alphanum)
/// ```
/// ⚠️ **WELL-FORMED ≠ VALID.** IANA kayıt defterine bakılmaz: `zz-ZZ` sözdizimsel olarak
/// doğrudur ama kayıtlı bir dil değildir. Kayıt denetimi ağ/gömülü defter ister ve kart
/// bunu iddia ETMEZ.
/// ⚠️ Tümüyle `x-…` özel kullanım etiketleri ve `i-…` eski (grandfathered) biçimleri
/// kabul edilir — RFC onları geçerli sayar.
pub fn looks_like_bcp47(value: &str) -> bool {
    // ⚠️ İÇ `trim` yok (issue #92); boşluk alt etiket karakteri değildir, aşağıda düşer.
    let t = value;
    if t.is_empty() || t.len() > 100 {
        return false;
    }
    let parts: Vec<&str> = t.split('-').collect();
    if parts.iter().any(|p| p.is_empty() || p.len() > 8 || !p.chars().all(|c| c.is_ascii_alphanumeric())) {
        return false;
    }
    let alpha = |p: &str| p.chars().all(|c| c.is_ascii_alphabetic());
    let digit = |p: &str| p.chars().all(|c| c.is_ascii_digit());

    // Tümü özel kullanım: x-abc
    if parts[0].eq_ignore_ascii_case("x") {
        return parts.len() >= 2;
    }
    // 🔴 GRANDFATHERED ETİKETLER SABİT BİR KAYIT DEFTERİDİR (issue #82, yeniden açıldı).
    // Buradaki eski kod `i-` önekini AÇIK bir ad alanı gibi ele alıyordu: `i-` + herhangi
    // bir harf dizisi geçiyordu, yani `i-foo` ve `i-whatever` well-formed sayılıyordu.
    // RFC 5646'da bu biçimler IANA kayıt defterinden gelir; uydurma bir `i-foo` ne
    // grandfathered bir etikettir ne de normal bir `langtag`.
    //
    // ⚠️ Kartta yazılı `zz-ZZ` KALINTISIYLA karıştırılmamalı: `zz-ZZ` sözdizimsel olarak
    // well-formed ama kayıtlı değildir (bilinçli kabul). `i-foo` sözdizimsel olarak
    // GEÇERSİZDİR — yani kalıntı değil, hataydı.
    //
    // Karşılaştırma küçük harfe normalize edilir (RFC 5646 §2.1.1: etiketler büyük/küçük
    // harfe duyarsızdır). Tablo üretilmiştir; kaynağı ve tarihi dosyanın başında.
    let lowered = t.to_ascii_lowercase();
    if GRANDFATHERED.binary_search(&lowered.as_str()).is_ok() {
        return true;
    }
    // `i-…` yalnız kayıt defterinde varsa geçerlidir; yukarıdaki arama onu zaten yakaladı.
    if parts[0].eq_ignore_ascii_case("i") {
        return false;
    }

    let mut idx = 0usize;
    // language
    let lang = parts[idx];
    if !alpha(lang) || !(2..=8).contains(&lang.len()) {
        return false;
    }
    idx += 1;
    if (2..=3).contains(&lang.len()) {
        // en fazla 3 extlang (3 alpha)
        let mut ext = 0;
        while ext < 3 && idx < parts.len() && parts[idx].len() == 3 && alpha(parts[idx]) {
            // 3-alpha aynı zamanda script DEĞİL (script 4) ve region DEĞİL (2 alpha/3 digit)
            idx += 1;
            ext += 1;
        }
    }
    // script (4 alpha)
    if idx < parts.len() && parts[idx].len() == 4 && alpha(parts[idx]) {
        idx += 1;
    }
    // region (2 alpha | 3 digit)
    if idx < parts.len()
        && ((parts[idx].len() == 2 && alpha(parts[idx])) || (parts[idx].len() == 3 && digit(parts[idx])))
    {
        idx += 1;
    }
    // variant* (5-8 alphanum | digit + 3 alphanum)
    while idx < parts.len() {
        let p = parts[idx];
        let is_variant = (5..=8).contains(&p.len())
            || (p.len() == 4 && p.starts_with(|c: char| c.is_ascii_digit()));
        if !is_variant {
            break;
        }
        idx += 1;
    }
    // extension* — her singleton BİR KEZ ve ardından en az bir 2-8 alt etiket
    let mut seen_singletons: Vec<char> = Vec::new();
    while idx < parts.len() && parts[idx].len() == 1 && !parts[idx].eq_ignore_ascii_case("x") {
        let sing = parts[idx].chars().next().unwrap().to_ascii_lowercase();
        if !sing.is_ascii_alphanumeric() || seen_singletons.contains(&sing) {
            return false;
        }
        seen_singletons.push(sing);
        idx += 1;
        let mut n = 0;
        while idx < parts.len() && (2..=8).contains(&parts[idx].len()) {
            idx += 1;
            n += 1;
        }
        if n == 0 {
            return false; // singleton var, uzantı YOK
        }
    }
    // privateuse
    if idx < parts.len() && parts[idx].eq_ignore_ascii_case("x") {
        idx += 1;
        if idx >= parts.len() {
            return false;
        }
        idx = parts.len(); // x'ten sonrası 1-8 alphanum: baştaki kontrol zaten sağladı
    }
    idx == parts.len()
}

/// GTFS `Phone number` tipi. Spec (`agency_phone`): *"Dialable text (for example, TriMet's
/// **"503-238-RIDE"**) is permitted, but the field must not contain any other descriptive
/// text."* Aynı tip `booking_rules.phone_number` için de geçerlidir.
///
/// Kontrol 2026-08-03'e kadar harf içeren HER değeri reddediyordu, yani spec'in kendi
/// örneğini de. Bu `PTH_017` biçimi bir hataydı: açıkça izinli veriyi ihlal saymak.
///
/// **Vanity ile açıklayıcı metin ayrımı:** harf grupları değerin SONUNDA, bitişik bir kuyruk
/// oluşturmalıdır — `503-238-RIDE`, `+1 800 FLOWERS`, `1-800-GO-FEDEX` geçerli;
/// `Call 503-238-1234` ("Call" başta), `555-1234 ext 99` ("ext" ortada) geçersiz. Vanity
/// numaralarda harfler numaranın son bloklarını oluşturur; açıklayıcı metin numaranın başına
/// ya da arasına girer.
///
/// **Birden çok harf grubu ek koşul ister (#95):** hepsi BÜYÜK HARF olmalıdır. Vanity yazımı
/// tuş takımı harflerini işaret etmek için evrensel olarak büyük harfle yazılır
/// (`1-800-GO-FEDEX`); `1234 call us now` gibi düzyazı bu koşulu geçemez. Tek harf grubunda
/// bu koşul ARANMAZ — yani değişiklik kabul kümesini yalnız GENİŞLETİR, hiçbir değeri yeni
/// baştan reddetmez.
///
/// ⚠️ **Bu bir KALİTE yaklaşımıdır, telefon numarası grameri DEĞİL.** GTFS `Phone number`
/// tipi tam bir gramer tanımlamaz; burada ölçülen şey "çevrilebilir metin mi yoksa açıklayıcı
/// metin mi" ayrımıdır ve sezgiseldir. Bilinen sınır: baştan sona büyük harfle yazılmış düzyazı
/// (`1234 CALL US NOW`) kabul edilir. Kural `AGN_007`/`BKR_022` üzerinden yalnız `Quality`
/// otoritesiyle raporlanır; sert bir Spec iddiası kurmaz.
///
/// ⚠️ Sezgiselin ölçülmüş bir faydası YOK: 239 feed'lik korpusta tek bir vanity numara bile
/// geçmiyor. Yazılma gerekçesi spec uyumu; koruma gerekçesi ise ayrımın korpusta gerçekten var
/// olan iki değeri (`80000078 (Liepājā); …`) reddetmeye devam etmesidir — o değerler spec'in
/// yasakladığı açıklayıcı metindir ve kural onlarda DOĞRU.
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
    if letter_group_indices.is_empty() {
        return true; // saf numara
    }
    // Harf grupları bitişik bir KUYRUK oluşturmalı: son n parçanın hepsi harf.
    let n = letter_group_indices.len();
    let tail_is_contiguous = letter_group_indices
        .iter()
        .enumerate()
        .all(|(k, &i)| i == parts.len() - n + k);
    if !tail_is_contiguous {
        return false; // harf başta ya da arada → açıklayıcı metin
    }
    // Tek grup: eski davranış (büyük/küçük harf aranmaz). Birden çok grup: vanity yazım
    // konvansiyonu olan BÜYÜK HARF şartı — düzyazıyı ayıran tek sinyal bu (#95).
    n == 1
        || letter_group_indices.iter().all(|&i| {
            parts[i].chars().all(|c| c.is_ascii_uppercase())
        })
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
pub(crate) fn arc033_summary(
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

/// `ARC_013` — akış dosyasının GÖVDESİNDE kapanmamış tırnak (issue #84).
///
/// 🔴 Bu bulgu 2026-08-07'ye kadar HİÇBİR YERDE üretilmiyordu. K1 bu dört dosyanın
/// gövdesini hiç açmıyor (`is_zip_stream`), K1'in gövde tarayıcısına giden dal ise ÖLÜYDÜ
/// ve kaldırıldı; K2 okuyucuları da kapanmamış tırnağı "best-effort" tolere ediyordu.
/// Yani `stop_times.txt`'te tek kaçak tırnak dosyanın kalanını yutabilir ve doğrulayıcı
/// bunu hiç söylemezdi.
///
/// ⚠️ **FATAL DEĞİL — bilinçli ve K1'den FARKLI.** Akış-dışı zorunlu bir dosyada aynı
/// bozukluk `FatalError`'a çevrilir; K2'nin fatal kanalı YOKTUR (pipeline K1'den sonra
/// durdurulamaz) ve gövdeyi K1'de yeniden taramak `#38`'in bellek tasarımını geri alırdı.
/// Bulgu aynı kuraldır ve KRİTİK'tir; farklı olan, boru hattının durabilme yeteneğidir.
/// Korpus ölçümü (2026-08-07): 20 feed · 75 akış dosyası · kapanmamış tırnak **0** —
/// yani bu karar bugünkü veride hiçbir feed'i etkilemiyor.
pub fn arc013_unclosed_stream(file: &str, counter: &mut u32) -> gtfs_core::Notice {
    let msg = "Kapanmamış tırnak işareti (unclosed quote)";
    make_k2_notice(
        counter, "ARC_013", EntityType::File, Some(file.to_string()),
        None, file, None, None,
        Some(msg.to_string()), None,
        format!("'{file}' CSV tokenization hatası: {msg} — kaydın kalanı okunamadı."),
        "CSV formatını kontrol edin; tırnak işaretlerinin doğru kapandığından emin olun.",
    )
}

// K2 notice'ları canonical emit tablosu olarak çağrılır; bağımsız alanları bir
// parametre struct'ında toplamak bu hot-path'te mevcut tablo düzenini bozacaktır.
#[allow(clippy::too_many_arguments)]
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

    let whitespace_derived = row.zip(field).is_some_and(|(r, f)| {
        whitespace_parser_derivative(rule_id, r, f, observed_value.as_deref(), &message)
    });
    let mut notice = crate::notice_factory::build(
        "K2", Some("k2"), counter, rule_id, entity_type, entity_id, scope_key,
        Some(file.to_string()), line, field.map(str::to_string),
        observed_value, expected_value, message, remediation,
    );
    if whitespace_derived {
        notice.details = Some([(
            "whitespace_derived".to_string(),
            "true".to_string(),
        )].into_iter().collect());
    }
    notice
}

/// Bir K2 tip/enum notice'ının, ham değerin çevresindeki boşluk yüzünden parse edilemediğini
/// tanır. İçerik hataları ("abc" gibi) bu kapıdan geçmez; böylece aynı satırdaki bağımsız
/// semantic ihlaller korunur. field bileşik anahtarlarda (a|b) da desteklenir.
fn whitespace_parser_derivative(
    rule_id: &str,
    row: &RowMap,
    field: &str,
    observed: Option<&str>,
    message: &str,
) -> bool {
    let parser_error = message.contains("bekleniyor, alınan:")
        || message.contains("sayı olarak okunamıyor")
        || message.contains("ondalık sayı olarak okunamıyor")
        || message.contains("geçersiz.");
    if !parser_error {
        return false;
    }
    field.split('|').any(|name| {
        let Some(raw) = row.get(name).map(String::as_str) else { return false };
        let trimmed = raw.trim();
        !trimmed.is_empty()
            && raw != trimmed
            && observed.is_some_and(|value| value == raw || value == trimmed)
            && trimmed_value_is_semantically_valid(rule_id, name, trimmed)
    })
}

/// Ham değerin trim edilmiş biçimi de bağımsız bir aralık/enum ihlali taşıyorsa, yalnızca
/// whitespace köküne indirgenemez. Bu küçük alan tablosu özellikle 91.0 gibi değerleri
/// korur; tanınmayan alanlar için parse başarısı yeterli kök kanıtıdır.
fn trimmed_value_is_semantically_valid(rule_id: &str, field: &str, value: &str) -> bool {
    if field.ends_with("_lat") {
        return value.parse::<f64>().is_ok_and(|v| v.is_finite() && (-90.0..=90.0).contains(&v));
    }
    if field.ends_with("_lon") {
        return value.parse::<f64>().is_ok_and(|v| v.is_finite() && (-180.0..=180.0).contains(&v));
    }
    if matches!(field, "price" | "amount" | "length" | "max_slope" | "level_index") {
        return value.parse::<f64>().is_ok_and(|v| v.is_finite() && v >= 0.0);
    }
    if field == "min_width" {
        return value.parse::<f64>().is_ok_and(|v| v.is_finite() && v > 0.0);
    }
    if field == "traversal_time" {
        return value.parse::<u32>().is_ok_and(|v| v > 0);
    }
    if field == "pathway_mode" {
        return value.parse::<u32>().is_ok_and(|v| (1..=7).contains(&v));
    }
    if field == "is_bidirectional" {
        return value.parse::<u32>().is_ok_and(|v| v <= 1);
    }
    if field == "stair_count" {
        return value.parse::<i32>().is_ok_and(|v| v != 0);
    }
    if field == "route_type" {
        return value.parse::<u32>().is_ok_and(|v| matches!(v, 0..=7 | 11 | 12));
    }
    if field == "location_type" {
        return value.parse::<u32>().is_ok_and(|v| v <= 4);
    }
    if field == "wheelchair_boarding" || field == "wheelchair_accessible"
        || field == "bikes_allowed" || field == "cars_allowed"
    {
        return value.parse::<u32>().is_ok_and(|v| v <= 2);
    }
    if field == "payment_method" {
        return value.parse::<u32>().is_ok_and(|v| v <= 1);
    }
    if field == "transfers" {
        return value.parse::<u32>().is_ok_and(|v| v <= 2);
    }
    if field == "transfer_type" {
        return value.parse::<u32>().is_ok_and(|v| v <= 5);
    }
    if field == "stop_access" {
        return value.parse::<u32>().is_ok_and(|v| v <= 2);
    }
    // Rule-specific custom parser messages still need a conservative guard.
    if rule_id == "RTS_004" {
        return value.parse::<u32>().is_ok_and(|v| matches!(v, 0..=7 | 11 | 12));
    }
    true
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

    /// issue #82 (yeniden açıldı) — `i-*` KEYFİ bir ad alanı DEĞİLDİR.
    #[test]
    fn bcp47_rejects_invented_grandfathered_tags() {
        assert!(!super::looks_like_bcp47("i-foo"), "uydurma i-* well-formed DEĞİLDİR");
        assert!(!super::looks_like_bcp47("i-whatever"));
        assert!(super::looks_like_bcp47("i-klingon"), "kayıtlı grandfathered etiket");
        assert!(super::looks_like_bcp47("i-navajo"));
        assert!(super::looks_like_bcp47("en-GB-oed"), "düzensiz grandfathered etiket");
        assert!(super::looks_like_bcp47("I-KLINGON"), "etiketler büyük/küçük harfe DUYARSIZ");
        assert!(super::looks_like_bcp47("zh-min-nan"), "düzenli grandfathered etiket");
        // ⚠️ `zz-ZZ` ile karıştırma: o well-formed ama kayıtlı değil (bilinçli kalıntı).
        assert!(super::looks_like_bcp47("zz-ZZ"), "kayıt defteri DENETLENMEZ — belgelenmiş kalıntı");
        // Normal langtag yolu grandfathered listesinden bağımsız çalışmalı.
        assert!(super::looks_like_bcp47("tr") && super::looks_like_bcp47("en-US"));
    }

    /// issue #86 — GTFS `Email` tipi SÖZDİZİMİ doğrulaması.
    /// Karşı örnek `a@@b.com` mevcut `main`de KABUL EDİLİYORDU: eski predikat
    /// `split_once('@')` ile ilk ayraçtan sonrasını alan adı sayıyordu.
    #[test]
    fn email_rejects_invalid_addr_spec_shapes() {
        // Geçerli — desteklenen dot-atom biçimi.
        for ok in ["info@example.com", "a.b-c@sub.example.co.uk",
                   "ticket+iyi_haber@example.org", "x!#$%&'*+-/=?^_`{|}~@example.com",
                   "info@şehir.example"] {
            assert!(super::looks_like_email(ok), "geçerli adres reddedildi: {ok}");
        }
        // Geçersiz — her biri AYRI bir bozukluk sınıfı.
        for bad in [
            "a@@b.com",          // bildirilen karşı örnek: iki ayraç
            "a@b@c.com",         // iki ayraç, ikisi de ayrı yerde
            "@example.com",      // local yok
            "a@",                // domain yok
            "a@b",               // tek etiket
            "a@.example.com",    // boş etiket
            "a@example..com",    // boş etiket (ortada)
            "a@-example.com",    // etiket tire ile başlıyor
            "a@example-.com",    // etiket tire ile bitiyor
            "a@example.1",       // sayısal TLD
            "a@example.c",       // tek harfli TLD
            ".a@example.com",    // baştaki nokta
            "a.@example.com",    // sondaki nokta
            "a..b@example.com",  // boş atom
            "a b@example.com",   // boşluk
            "a\t@example.com",   // sekme
            "noatsign.example.com",
            "",
        ] {
            assert!(!super::looks_like_email(bad), "geçersiz adres kabul edildi: {bad:?}");
        }
    }

    /// issue #82 — paylaşılan tip yardımcıları, temsil ettikleri standarttan GENİŞTİ.
    /// Her biri için: bildirilen GEÇERSİZ örnek + geçerli sınır durumu.
    #[test]
    fn type_helpers_reject_values_outside_the_declared_types() {
        // 1) BCP 47: birincil alt etiket ALFABETİK olmalı.
        // GEÇERLİ (RFC 5646 well-formed)
        for ok in ["tr", "en-US", "zh-Hant-TW", "es-419", "x-private", "de-CH-1901",
                   "en-a-bbb", "en-a-bbb-x-priv", "i-klingon", "sl-rozaj-biske"] {
            assert!(super::looks_like_bcp47(ok), "geçerli dil etiketi reddedildi: {ok}");
        }
        // GEÇERSİZ — her biri ayrı bir sözdizimi kuralı
        for bad in ["123", "1-tr", "en-a", "en-a-b-c", "en-a-bbb-a-ccc", "en--US", "en-",
                    "x-", "-en", "en-toolongsubtag", "en-US-"] {
            assert!(!super::looks_like_bcp47(bad), "geçersiz dil etiketi kabul edildi: {bad:?}");
        }

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

    #[test]
    fn looks_like_url_requires_a_web_scheme() {
        // Spec: "A fully qualified URL that includes http:// or https://."
        assert!(super::looks_like_url("http://a.com"));
        assert!(super::looks_like_url("https://a.com/x?y=1#z"));
        assert!(super::looks_like_url("HTTPS://A.COM"), "şema adı büyük/küçük harfe duyarsız");
        // issue #92: baştaki/sondaki boşluk ARTIK KIRPILMAZ. Bu test eski toleransı
        // koruyordu; spec "özel karakterler doğru kaçırılmalı" diyor ve boşluk kaçırılmamış
        // bir karakterdir. VARLIK teşhisi hâlâ kırpar — `"   "` EKSİK alandır, geçersiz
        // URL değil; o ayrım çağıran tarafta iki ayrı okumayla korunuyor.
        assert!(!super::looks_like_url("  https://a.com  "), "ham değerde çıplak boşluk");
        assert!(super::looks_like_url("https://a.com"));

        // Bunların hepsi `Url::parse` için GEÇERLİ ve şema kontrolü eklenmeden önce
        // doğrulamadan geçiyordu (T5 boşluk #1, 2026-08-03 ölçümü).
        assert!(!super::looks_like_url("mailto:info@a.com"));
        assert!(!super::looks_like_url("ftp://a.com/x"));
        assert!(!super::looks_like_url("file:///etc/passwd"));
        assert!(!super::looks_like_url("javascript:alert(1)"));

        // issue #80 — hükmün İKİNCİ yarısı: özel karakterler DOĞRU kaçırılmış olmalı.
        // `Url::parse` bunların hepsine Ok diyordu (normalize ederek).
        assert!(super::looks_like_url("https://a.example/a%20b"), "doğru yüzde kodlaması");
        assert!(!super::looks_like_url("https://a.example/a b"), "kaçırılmamış boşluk");
        assert!(!super::looks_like_url("https://a.example/a\"b"), "kaçırılmamış tırnak");
        assert!(!super::looks_like_url("https://a.example/a<b>"), "kaçırılmamış açılı ayraç");
        assert!(!super::looks_like_url("https://a.example/a%zzb"), "BOZUK yüzde kodlaması");
        assert!(!super::looks_like_url("https://a.example/a%2"), "eksik yüzde kodlaması");
        assert!(!super::looks_like_url("https://a.example/a\\b"), "ters bölü çıplak geçemez");
        // issue #80: ASCII dışı ÇIPLAK geçemez — spec "correctly escaped" diyor ve atıf
        // yaptığı W3C belgesi URI'yi ASCII olarak tanımlıyor. Yüzde kodlanmış EŞDEĞERİ
        // geçerlidir; reddedilen şey karakterin kendisi değil, KAÇIRILMAMIŞ olması.
        assert!(!super::looks_like_url("https://example.com/güzergah"), "çıplak ASCII dışı");
        assert!(super::looks_like_url("https://example.com/g%C3%BCzergah"), "yüzde kodlanmış eşdeğeri GEÇERLİ");
        assert!(!super::looks_like_url("https://şehir.example/yol"), "IDN alan adı da çıplak ASCII dışıdır");
        assert!(super::looks_like_url("https://xn--ehir-jua.example/yol"), "punycode eşdeğeri GEÇERLİ");
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
        // #95: BİRDEN ÇOK vanity harf grubu — hepsi büyük harf ve kuyrukta.
        assert!(super::looks_like_phone("1-800-GO-FEDEX"), "iki vanity grubu kabul edilmeli");
        assert!(super::looks_like_phone("1 800 GO FEDEX"), "ayırıcı boşluk da olabilir");
        assert!(super::looks_like_phone("+1-800-NEW-CARS"));
        // Tek grupta büyük harf ŞARTI YOK — eski davranış aynen korunur (kabul kümesi
        // yalnız genişledi, hiçbir değer yeni baştan reddedilmedi).
        assert!(super::looks_like_phone("503-238-ride"));
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
        // #95 sınırı: çok gruplu KUYRUK ancak BÜYÜK HARFse vanity sayılır; düzyazı geçemez.
        assert!(!super::looks_like_phone("1234 call us now"), "küçük harf kuyruk düzyazıdır");
        assert!(!super::looks_like_phone("555 1234 Call Us"), "baş harfi büyük düzyazı vanity değil");
        assert!(!super::looks_like_phone("GO FEDEX 1 800"), "harf kuyrukta değil, BAŞTA");
        assert!(!super::looks_like_phone("1-800-GO-FEDEX-now"), "kuyruğun son grubu küçük harf");
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
        assert_eq!(mu("EUR"), Some(2));
        assert_eq!(mu("JPY"), Some(0));
        assert_eq!(mu("KWD"), Some(3));
        // issue #82: "0/2/3 dışında değer yok" VARSAYIMI YANLIŞTI — ölçüldü.
        assert_eq!(mu("CLF"), Some(4), "CLF dört ondalıklıdır");
        assert_eq!(mu("UYW"), Some(4), "UYW dört ondalıklıdır");
        // Kod geçerliliği ile ondalık AYRI kavram: ikisi de `None` döner ama sebepleri farklı.
        assert_eq!(mu("ZZZ"), None, "bilinmeyen kod — 2 VARSAYILMAZ");
        assert_eq!(mu("XAU"), None, "kıymetli maden: ondalık TANIMSIZ (kaynakta N.A.)");
        assert!(super::is_iso4217("XAU"), "…ama XAU GEÇERLİ bir koddur");
        // 🔴 Elle yazılmış liste yanlıştı: BGN 2026-01-01 listesinde AKTİF DEĞİL
        // (Bulgaristan euro'ya geçti). Üretilmiş tablo bunu kaynaktan alıyor.
        assert!(!super::is_iso4217("BGN"), "BGN artık aktif kod değil");

        assert!(ok("2.50", "EUR"));
        assert!(ok("150", "JPY"), "yen ondalık taşımaz");
        assert!(ok("1.500", "KWD"));

        // Korpusta ölçülen gerçek ihlaller.
        assert!(!ok("0.9", "EUR"), "EUR iki basamak ister");
        assert!(!ok("6.7", "HKD"));
        assert!(!ok("8", "EUR"));
        assert!(!ok("100.00", "JPY"), "yen ondalık TAŞIMAMALI");
        // Geçersiz kodda ondalık denetimi SUSAR — o ihlali FAR_003/FPD_003 bildirir.
        assert!(ok("1.23456", "ZZZ"), "geçersiz kod iki kez raporlanmaz");
        assert!(ok("1.2", "XAU"), "ondalığı tanımsız kodda denetim yapılmaz");
        assert!(!ok("0.0000", "HKD"), "fazla basamak da ihlal");

        // Sayı olmayan / boş değer FPD_002'nin alanı — burada sessiz.
        assert!(ok("", "EUR"));
        assert!(ok("abc", "EUR"));
        assert!(ok("2.50", ""), "para birimi yoksa karar verilemez");
    }

}
