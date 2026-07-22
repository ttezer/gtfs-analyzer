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
pub fn parse_f64_col(raw: &str) -> Result<Option<f64>, ()> {
    if raw.is_empty() {
        return Ok(None);
    }
    raw.parse::<f64>().map(Some).map_err(|_| ())
}

pub fn parse_f64(row: &RowMap, field: &str) -> Result<Option<f64>, String> {
    let Some(raw) = get_trimmed_field(row, field) else {
        return Ok(None);
    };
    if raw.is_empty() {
        return Ok(None);
    }
    raw.parse::<f64>()
        .map(Some)
        .map_err(|_| format!("'{field}' için f64 bekleniyor, alınan: {raw}"))
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

    Ok(Some((year, month, day)))
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

pub fn validate_enum(value: &str, allowed: &[&str]) -> bool {
    allowed.contains(&value)
}

pub fn looks_like_url(value: &str) -> bool {
    Url::parse(value).is_ok()
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
    trimmed.split('-').all(|part| {
        let len = part.len();
        !part.is_empty() && len <= 8 && part.chars().all(|c| c.is_ascii_alphanumeric())
    })
}

pub fn looks_like_phone(value: &str) -> bool {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return false;
    }
    let digit_count = trimmed.chars().filter(|c| c.is_ascii_digit()).count();
    digit_count >= 5
        && trimmed
            .chars()
            .all(|c| c.is_ascii_digit() || matches!(c, '+' | '-' | '(' | ')' | ' ' | '.'))
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
}
