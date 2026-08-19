use gtfs_core::EntityType;

use super::common::{get_raw_field, build_row_map, get_trimmed_field, looks_like_phone, looks_like_url, make_k2_notice, parse_gtfs_time, RowMap};
use crate::k1_parse::RawFile;

#[derive(Debug, Clone)]
pub struct BookingRuleRecord {
    pub booking_rule_id: String,
    pub booking_type: Option<u8>,
    pub prior_notice_duration_min: Option<i64>,
    pub prior_notice_duration_max: Option<i64>,
    pub prior_notice_last_day: Option<i64>,
    pub prior_notice_last_time: Option<String>,
    pub prior_notice_start_day: Option<i64>,
    pub prior_notice_start_time: Option<String>,
    pub prior_notice_service_id: Option<String>,
    pub row: RowMap,
    pub line: u64,
}

fn opt_str(row: &RowMap, field: &str) -> Option<String> {
    get_trimmed_field(row, field)
        .filter(|v| !v.trim().is_empty())
        .map(str::to_string)
}

fn has_field(row: &RowMap, field: &str) -> bool {
    get_trimmed_field(row, field).map(|v| !v.is_empty()).unwrap_or(false)
}


/// `opt_int`'in raporlayan hâli: sayı olmayan değer sessizce düşmez, `rule` ile bildirilir.
///
/// Spec bu dört alanı `Integer` olarak tipler. Eski `opt_int` "abc"yi sessizce yutuyordu:
/// koşullu-yasak hükmü (BKR_002/BKR_004/BKR_005) ateşliyor ama DEĞERİN sayı olmadığı hiç
/// söylenmiyordu — yani kullanıcı bozuk bir rezervasyon penceresini göremiyordu.
#[allow(clippy::too_many_arguments)]
fn opt_int_checked(
    row: &RowMap,
    field: &'static str,
    rule: &'static str,
    entity_id: Option<String>,
    file_name: &str,
    line: u64,
    notices: &mut Vec<gtfs_core::Notice>,
    ctr: &mut u32,
) -> Option<i64> {
    let raw = get_trimmed_field(row, field).filter(|v| !v.trim().is_empty())?;
    match raw.parse::<i64>() {
        Ok(v) => Some(v),
        Err(_) => {
            notices.push(make_k2_notice(
                ctr, rule, EntityType::Row, entity_id, Some(row),
                file_name, Some(line), Some(field),
                Some(raw.to_string()), Some("tam sayı".to_string()),
                format!("{field} '{raw}' tam sayı olarak okunamıyor."),
                "Bu alanı tam sayı olarak girin.",
            ));
            None
        }
    }
}

pub fn validate_booking_rules(file: &RawFile) -> (Vec<BookingRuleRecord>, Vec<gtfs_core::Notice>) {
    let mut notices = Vec::new();
    let mut records = Vec::new();
    let mut ctr = 0u32;
    // BKR_019: booking_rule_id birincil anahtardır (spec "Primary key (booking_rule_id)").
    let mut seen_ids: std::collections::HashSet<String> = std::collections::HashSet::new();

    for (row_idx, row) in file.rows.iter().enumerate() {
        let line = (row_idx + 2) as u64;
        let row_map = build_row_map(&file.headers, row);

        let id = get_raw_field(&row_map, "booking_rule_id").unwrap_or("").to_string();
        let entity_id = (!id.is_empty()).then_some(id.clone());

        // BKR_019: booking_rule_id eksik (boş) veya yineleniyor.
        // Sütun başlıkta hiç yoksa ARC_025 devralır (RTS_004 deseni) → burada susulur.
        if id.is_empty() {
            if get_raw_field(&row_map, "booking_rule_id").map(str::trim) == Some("") {
                notices.push(make_k2_notice(
                    &mut ctr, "BKR_019", EntityType::Row, None, Some(&row_map),
                    &file.name, Some(line), Some("booking_rule_id"),
                    Some(String::new()), None,
                    "booking_rule_id zorunludur.".to_string(),
                    "Her rezervasyon kuralına benzersiz bir booking_rule_id verin.",
                ));
            }
        } else if !seen_ids.insert(id.clone()) {
            notices.push(make_k2_notice(
                &mut ctr, "BKR_019", EntityType::Row, entity_id.clone(), Some(&row_map),
                &file.name, Some(line), Some("booking_rule_id"),
                Some(id.clone()), None,
                format!("booking_rule_id '{id}' yineleniyor; bu alan dosyanın birincil anahtarıdır."),
                "Her satıra benzersiz bir booking_rule_id verin.",
            ));
        }

        let btype_str = get_trimmed_field(&row_map, "booking_type").unwrap_or("").to_string();
        let booking_type: Option<u8> = btype_str.parse::<u8>().ok().filter(|&v| v <= 2);

        // BKR_016: booking_type eksik veya geçersiz (spec: Required, enum 0/1/2).
        // Sütun başlıkta yoksa ARC_025 devralır → yalnız sütun VARKEN denetlenir (RTS_004 deseni).
        // ⚠️ booking_type okunamadığında SEKİZ tür-bağımlı kural (BKR_001/004/005/007/008/009/012/014)
        // sessizce devre dışı kalır — bu yüzden `blocks` listesi o kuralları sayar.
        if booking_type.is_none() && get_trimmed_field(&row_map, "booking_type").is_some() {
            let (observed, message) = if btype_str.is_empty() {
                (String::new(), "booking_type zorunludur.".to_string())
            } else {
                (btype_str.clone(), format!("booking_type '{btype_str}' geçerli bir değer değil (0, 1 veya 2 olmalıdır)."))
            };
            notices.push(make_k2_notice(
                &mut ctr, "BKR_016", EntityType::Row, entity_id.clone(), Some(&row_map),
                &file.name, Some(line), Some("booking_type"),
                Some(observed), Some("0-2".to_string()), message,
                "booking_type alanını 0 (anlık), 1 (aynı gün) veya 2 (önceki gün) yapın.",
            ));
        }

        let has_duration_min = has_field(&row_map, "prior_notice_duration_min");
        let has_duration_max = has_field(&row_map, "prior_notice_duration_max");
        let has_last_day    = has_field(&row_map, "prior_notice_last_day");
        let has_last_time   = has_field(&row_map, "prior_notice_last_time");
        let has_start_day   = has_field(&row_map, "prior_notice_start_day");
        let has_start_time  = has_field(&row_map, "prior_notice_start_time");
        let has_service_id  = has_field(&row_map, "prior_notice_service_id");

        // BKR_025: prior_notice zaman alanı GTFS Time olarak ayrıştırılamıyor.
        //
        // BKR_023 aynı işi SAYI alanları için yapıyordu; zaman alanlarının tip
        // kardeşi yoktu. Modül `prior_notice_start_time`/`_last_time` değerlerini
        // yalnızca String olarak okuyor ve hangi gün alanıyla eşleştiklerini
        // denetliyordu (BKR_003/010/013) — biçimlerini hiç ayrıştırmıyordu. Bozuk
        // bir saat bize tamamen görünmezdi (#164, `tdg-80694` ve `tdg-84001`).
        //
        // ⚠️ `Err` YUTULMAZ: `parse_gtfs_time` ayrıştırılamayan değer için `Err`,
        // alan yoksa `Ok(None)` döner. İkisini birbirine karıştırmak bu depoda
        // dört kez kural körleştirdi.
        for field in ["prior_notice_start_time", "prior_notice_last_time"] {
            if let Err(raw) = parse_gtfs_time(&row_map, field) {
                notices.push(make_k2_notice(
                    &mut ctr, "BKR_025", EntityType::Row, entity_id.clone(), Some(&row_map),
                    &file.name, Some(line), Some(field),
                    Some(raw), Some("HH:MM:SS".to_string()),
                    format!("{field} '{}' geçerli bir GTFS saati değil.",
                            get_trimmed_field(&row_map, field).unwrap_or("")),
                    "Saati HH:MM:SS biçiminde yazın; gece yarısını aşan servisler için 24'ten büyük saat kullanın (örn. 25:10:00).",
                ));
            }
        }

        let duration_min = opt_int_checked(&row_map, "prior_notice_duration_min", "BKR_023", Some(id.clone()), &file.name, line, &mut notices, &mut ctr);
        let duration_max = opt_int_checked(&row_map, "prior_notice_duration_max", "BKR_023", Some(id.clone()), &file.name, line, &mut notices, &mut ctr);
        let last_day     = opt_int_checked(&row_map, "prior_notice_last_day", "BKR_023", Some(id.clone()), &file.name, line, &mut notices, &mut ctr);
        let start_day    = opt_int_checked(&row_map, "prior_notice_start_day", "BKR_023", Some(id.clone()), &file.name, line, &mut notices, &mut ctr);

        if let Some(btype) = booking_type {
            // BKR_004: booking_type=0 iken prior_notice alanları yasak
            if btype == 0
                && (has_duration_min || has_duration_max || has_last_day
                    || has_last_time || has_start_day || has_start_time)
            {
                notices.push(make_k2_notice(
                    &mut ctr, "BKR_004", EntityType::Row, entity_id.clone(), Some(&row_map),
                    &file.name, Some(line), Some("prior_notice_duration_min"),
                    Some(btype_str.clone()), Some("(boş)".to_string()),
                    "booking_type=0 (anlık rezervasyon) iken prior_notice alanları dolu olmamalı.".to_string(),
                    "Anlık rezervasyon için prior_notice alanlarını kaldırın ya da booking_type değerini düzeltin.",
                ));
            }

            // BKR_005: booking_type=2 iken prior_notice_duration_max yasak
            if btype == 2 && has_duration_max {
                notices.push(make_k2_notice(
                    &mut ctr, "BKR_005", EntityType::Row, entity_id.clone(), Some(&row_map),
                    &file.name, Some(line), Some("prior_notice_duration_max"),
                    get_trimmed_field(&row_map, "prior_notice_duration_max").map(str::to_string),
                    Some("(boş)".to_string()),
                    "booking_type=2 iken prior_notice_duration_max yasaktır.".to_string(),
                    "prior_notice_duration_max yalnızca booking_type=1 (aynı gün) ile kullanılabilir.",
                ));
            }

            // BKR_012: booking_type=2 iken prior_notice_duration_min yasak.
            // Spec (booking_rules.txt): "Required for booking_type=1. Forbidden otherwise."
            // type=0 kolu BKR_004'te (tüm prior_notice alanları) — burada tekrar edilmez.
            // BKR_005'in (duration_max) birebir aynası.
            if btype == 2 && has_duration_min {
                notices.push(make_k2_notice(
                    &mut ctr, "BKR_012", EntityType::Row, entity_id.clone(), Some(&row_map),
                    &file.name, Some(line), Some("prior_notice_duration_min"),
                    get_trimmed_field(&row_map, "prior_notice_duration_min").map(str::to_string),
                    Some("(boş)".to_string()),
                    "booking_type=2 iken prior_notice_duration_min yasaktır.".to_string(),
                    "prior_notice_duration_min yalnızca booking_type=1 (aynı gün) ile kullanılabilir.",
                ));
            }

            // BKR_014: prior_notice_service_id yalnızca booking_type=2 ile kullanılabilir.
            // Spec: "Optional if booking_type=2. Forbidden otherwise."
            // BKR_004 bu alanı KAPSAMAZ (yalnız altı prior_notice_* alanı) → type=0 da buraya dahil.
            if btype != 2 && has_service_id {
                notices.push(make_k2_notice(
                    &mut ctr, "BKR_014", EntityType::Row, entity_id.clone(), Some(&row_map),
                    &file.name, Some(line), Some("prior_notice_service_id"),
                    Some(btype_str.clone()), Some("2".to_string()),
                    format!("booking_type={btype} iken prior_notice_service_id yasaktır; bu alan yalnızca booking_type=2 ile kullanılır."),
                    "prior_notice_service_id alanını kaldırın ya da booking_type değerini 2 (önceki gün rezervasyonu) yapın.",
                ));
            }

            // BKR_007: booking_type=1 iken prior_notice_duration_min zorunlu
            if btype == 1 && !has_duration_min {
                notices.push(make_k2_notice(
                    &mut ctr, "BKR_007", EntityType::Row, entity_id.clone(), Some(&row_map),
                    &file.name, Some(line), Some("prior_notice_duration_min"),
                    None, Some("integer > 0".to_string()),
                    "booking_type=1 iken prior_notice_duration_min zorunludur.".to_string(),
                    "Aynı gün rezervasyon için minimum önceden bildirim süresini (dakika) girin.",
                ));
            }

            // BKR_001: prior_notice_last_day yalnız booking_type=2 için tanımlanabilir
            // (spec booking_rules.txt: "Required for booking_type=2. Forbidden otherwise").
            if btype != 2 && has_last_day {
                notices.push(make_k2_notice(
                    &mut ctr, "BKR_001", EntityType::Row, entity_id.clone(), Some(&row_map),
                    &file.name, Some(line), Some("prior_notice_last_day"),
                    Some(btype_str.clone()), Some("2".to_string()),
                    format!("booking_type={btype} iken prior_notice_last_day yasaktır; bu alan yalnızca booking_type=2 ile kullanılır."),
                    "prior_notice_last_day alanını yalnızca booking_type=2 ile kullanın ya da booking_type değerini düzeltin.",
                ));
            }
            // BKR_001: prior_notice_start_day SADECE booking_type=0'da, veya booking_type=1
            // + prior_notice_duration_max tanımlıyken yasaktır. Spec: "Forbidden for
            // booking_type=0. Forbidden for booking_type=1 if prior_notice_duration_max is
            // defined. Optional otherwise." → type=1 (duration_max'sız) ve type=2'de MEŞRU.
            // (Eski koşul btype≠2 tüm type=1+start_day'i yanlışça yasaklıyordu — FP fix.)
            if has_start_day && (btype == 0 || (btype == 1 && has_duration_max)) {
                let neden = if btype == 0 {
                    "booking_type=0"
                } else {
                    "booking_type=1 ve prior_notice_duration_max tanımlı"
                };
                notices.push(make_k2_notice(
                    &mut ctr, "BKR_001", EntityType::Row, entity_id.clone(), Some(&row_map),
                    &file.name, Some(line), Some("prior_notice_start_day"),
                    Some(btype_str.clone()), Some("(boş)".to_string()),
                    format!("{neden} iken prior_notice_start_day yasaktır."),
                    "prior_notice_start_day alanını kaldırın ya da booking_type / prior_notice_duration_max değerlerini düzeltin.",
                ));
            }

            // BKR_008: booking_type=2 iken prior_notice_last_day zorunlu
            if btype == 2 && !has_last_day {
                notices.push(make_k2_notice(
                    &mut ctr, "BKR_008", EntityType::Row, entity_id.clone(), Some(&row_map),
                    &file.name, Some(line), Some("prior_notice_last_day"),
                    None, Some("integer ≥ 0".to_string()),
                    "booking_type=2 iken prior_notice_last_day zorunludur.".to_string(),
                    "Önceki gün rezervasyonu için son bildirim gününü girin.",
                ));
            }

            // BKR_009: booking_type=2 iken prior_notice_last_time zorunlu
            if btype == 2 && !has_last_time {
                notices.push(make_k2_notice(
                    &mut ctr, "BKR_009", EntityType::Row, entity_id.clone(), Some(&row_map),
                    &file.name, Some(line), Some("prior_notice_last_time"),
                    None, Some("HH:MM:SS".to_string()),
                    "booking_type=2 iken prior_notice_last_time zorunludur.".to_string(),
                    "Son bildirim günü için saat bilgisini girin (HH:MM:SS).",
                ));
            }
        }

        // BKR_006: prior_notice_duration_min ≤ 0 veya sayısal değil
        if has_duration_min {
            match duration_min {
                Some(v) if v <= 0 => {
                    notices.push(make_k2_notice(
                        &mut ctr, "BKR_006", EntityType::Row, entity_id.clone(), Some(&row_map),
                        &file.name, Some(line), Some("prior_notice_duration_min"),
                        Some(v.to_string()), Some("> 0".to_string()),
                        "prior_notice_duration_min sıfır veya negatif olamaz.".to_string(),
                        "Minimum bildirim süresini pozitif bir dakika değeri olarak girin.",
                    ));
                }
                None => {
                    notices.push(make_k2_notice(
                        &mut ctr, "BKR_006", EntityType::Row, entity_id.clone(), Some(&row_map),
                        &file.name, Some(line), Some("prior_notice_duration_min"),
                        get_trimmed_field(&row_map, "prior_notice_duration_min").map(str::to_string),
                        Some("integer > 0".to_string()),
                        "prior_notice_duration_min sayısal bir değer değil.".to_string(),
                        "Geçerli bir tam sayı (dakika) girin.",
                    ));
                }
                _ => {}
            }
        }

        // BKR_024: aynı gün rezervasyonda (booking_type=1) üst süre sınırı varken başlangıç
        // günü çelişkilidir — spec: "Forbidden for booking_type=1 if prior_notice_duration_max
        // is defined." `BKR_002` komşudur ama başka hükmü ölçer (start_day yalnız last_day ile).
        if booking_type == Some(1) && has_duration_max && has_start_day {
            notices.push(make_k2_notice(
                &mut ctr, "BKR_024", EntityType::Row, entity_id.clone(), Some(&row_map),
                &file.name, Some(line), Some("prior_notice_start_day"),
                get_trimmed_field(&row_map, "prior_notice_start_day").map(str::to_string),
                Some("(boş)".to_string()),
                "booking_type=1 ve prior_notice_duration_max tanımlıyken prior_notice_start_day yasaktır.".to_string(),
                "prior_notice_start_day alanını boşaltın ya da prior_notice_duration_max'i kaldırın.",
            ));
        }

        // BKR_002: prior_notice_start_day dolu ama prior_notice_last_day yok
        if has_start_day && !has_last_day {
            notices.push(make_k2_notice(
                &mut ctr, "BKR_002", EntityType::Row, entity_id.clone(), Some(&row_map),
                &file.name, Some(line), Some("prior_notice_start_day"),
                get_trimmed_field(&row_map, "prior_notice_start_day").map(str::to_string), None,
                "prior_notice_start_day yalnızca prior_notice_last_day ile birlikte kullanılabilir.".to_string(),
                "prior_notice_last_day ekleyin ya da prior_notice_start_day kaldırın.",
            ));
        }

        // BKR_003: prior_notice_start_time dolu ama prior_notice_start_day yok
        if has_start_time && !has_start_day {
            notices.push(make_k2_notice(
                &mut ctr, "BKR_003", EntityType::Row, entity_id.clone(), Some(&row_map),
                &file.name, Some(line), Some("prior_notice_start_time"),
                get_trimmed_field(&row_map, "prior_notice_start_time").map(str::to_string), None,
                "prior_notice_start_time yalnızca prior_notice_start_day ile birlikte kullanılabilir.".to_string(),
                "prior_notice_start_day ekleyin ya da prior_notice_start_time kaldırın.",
            ));
        }

        // BKR_013: prior_notice_last_time dolu ama prior_notice_last_day yok.
        // Spec: "Required if prior_notice_last_day is defined. Forbidden otherwise."
        // BKR_003'ün (start_time ↔ start_day) birebir aynası; BKR_003 gibi booking_type'tan
        // BAĞIMSIZ çalışır (booking_type okunamayan satırlarda da geçerli bir spec ihlali).
        if has_last_time && !has_last_day {
            notices.push(make_k2_notice(
                &mut ctr, "BKR_013", EntityType::Row, entity_id.clone(), Some(&row_map),
                &file.name, Some(line), Some("prior_notice_last_time"),
                get_trimmed_field(&row_map, "prior_notice_last_time").map(str::to_string), None,
                "prior_notice_last_time yalnızca prior_notice_last_day ile birlikte kullanılabilir.".to_string(),
                "prior_notice_last_day ekleyin ya da prior_notice_last_time kaldırın.",
            ));
        }

        // BKR_010: prior_notice_start_day dolu ama prior_notice_start_time yok
        if has_start_day && !has_start_time {
            notices.push(make_k2_notice(
                &mut ctr, "BKR_010", EntityType::Row, entity_id.clone(), Some(&row_map),
                &file.name, Some(line), Some("prior_notice_start_time"),
                None, Some("HH:MM:SS".to_string()),
                "prior_notice_start_day belirtilmişse prior_notice_start_time zorunludur.".to_string(),
                "Erken rezervasyon penceresi başlangıç saatini girin (HH:MM:SS).",
            ));
        }

        // BKR_011: prior_notice_last_day > prior_notice_start_day (rezervasyon penceresi geçersiz)
        if let (Some(ld), Some(sd)) = (last_day, start_day) {
            if ld > sd {
                notices.push(make_k2_notice(
                    &mut ctr, "BKR_011", EntityType::Row, entity_id.clone(), Some(&row_map),
                    &file.name, Some(line), Some("prior_notice_last_day"),
                    Some(format!("last_day={ld}, start_day={sd}")), None,
                    format!("prior_notice_last_day ({ld}) > prior_notice_start_day ({sd}): rezervasyon penceresi geçersiz."),
                    "prior_notice_start_day değerini prior_notice_last_day değerinden büyük ya da eşit yapın.",
                ));
            }
        }

        // BKR_020/021/022: üç iletişim alanı `known_columns`'da listeliydi ama hiçbir denetim
        // aşaması okumuyordu (#60) — değerler sessizce geçiyordu. URL'ler Spec (tip `URL`),
        // telefon Quality (spec `Phone number` tipi dilbilgisi tanımlamaz → AGN_007 emsali).
        for (field, rule, msg, fix) in [
            ("booking_url", "BKR_020",
             "booking_url geçerli bir URL değil.",
             "booking_url için geçerli bir http/https URL'si kullanın."),
            ("info_url", "BKR_021",
             "info_url geçerli bir URL değil.",
             "info_url için geçerli bir http/https URL'si kullanın."),
        ] {
            if let Some(url) = opt_str(&row_map, field) {
                if !looks_like_url(&url) {
                    notices.push(make_k2_notice(
                        &mut ctr, rule, EntityType::Row, Some(id.clone()), Some(&row_map),
                        &file.name, Some(line), Some(field), Some(url), None,
                        msg.to_string(), fix,
                    ));
                }
            }
        }
        if let Some(phone) = opt_str(&row_map, "phone_number") {
            if !looks_like_phone(&phone) {
                notices.push(make_k2_notice(
                    &mut ctr, "BKR_022", EntityType::Row, Some(id.clone()), Some(&row_map),
                    &file.name, Some(line), Some("phone_number"), Some(phone.clone()), None,
                    format!("phone_number '{phone}' geçerli bir telefon numarası formatında değil."),
                    "Çevrilebilir bir telefon numarası kullanın; numaranın dışında açıklayıcı metin bulunmasın.",
                ));
            }
        }

        records.push(BookingRuleRecord {
            booking_rule_id: id,
            booking_type,
            prior_notice_duration_min: duration_min,
            prior_notice_duration_max: duration_max,
            prior_notice_last_day: last_day,
            prior_notice_last_time: opt_str(&row_map, "prior_notice_last_time"),
            prior_notice_start_day: start_day,
            prior_notice_start_time: opt_str(&row_map, "prior_notice_start_time"),
            prior_notice_service_id: opt_str(&row_map, "prior_notice_service_id"),
            row: row_map,
            line,
        });
    }

    (records, notices)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::k1_parse::RawFile;

    fn make_file(headers: Vec<&str>, rows: Vec<Vec<&str>>) -> RawFile {
        use smol_str::SmolStr;
        RawFile {
            name: "booking_rules.txt".to_string(),
            headers: headers.into_iter().map(str::to_string).collect(),
            rows: rows.into_iter().map(|r| r.into_iter().map(SmolStr::new).collect()).collect(),
            bytes: 0,
            raw_text: None,
        }
    }

    #[test]
    fn bkr_025_reports_a_malformed_prior_notice_time() {
        let file = make_file(
            vec!["booking_rule_id", "booking_type", "prior_notice_start_day", "prior_notice_start_time"],
            vec![vec!["BR1", "2", "3", "yarin sabah"]],
        );
        let (_, notices) = validate_booking_rules(&file);
        assert!(notices.iter().any(|n| n.rule_id == "BKR_025"),
            "Ayrıştırılamayan saat BKR_025 üretmeli: {:?}",
            notices.iter().map(|n| &n.rule_id).collect::<Vec<_>>());
    }

    #[test]
    fn bkr_025_accepts_a_time_past_midnight() {
        // GTFS saatleri 24'ü aşabilir; 25:10:00 GEÇERLİDİR ve kural susmalıdır.
        let file = make_file(
            vec!["booking_rule_id", "booking_type", "prior_notice_start_day", "prior_notice_start_time"],
            vec![vec!["BR1", "2", "3", "25:10:00"]],
        );
        let (_, notices) = validate_booking_rules(&file);
        assert!(!notices.iter().any(|n| n.rule_id == "BKR_025"));
    }

    #[test]
    fn bkr_025_stays_silent_when_the_field_is_absent() {
        // Alanın YOKLUĞU bir tip hatası değildir; onu BKR_010/013 sahiplenir.
        let file = make_file(
            vec!["booking_rule_id", "booking_type"],
            vec![vec!["BR1", "1"]],
        );
        let (_, notices) = validate_booking_rules(&file);
        assert!(!notices.iter().any(|n| n.rule_id == "BKR_025"));
    }

    #[test]
    fn bkr_025_covers_the_last_time_field_too() {
        let file = make_file(
            vec!["booking_rule_id", "booking_type", "prior_notice_last_day", "prior_notice_last_time"],
            vec![vec!["BR1", "2", "3", "12:60"]],
        );
        let (_, notices) = validate_booking_rules(&file);
        assert!(notices.iter().any(|n| n.rule_id == "BKR_025"));
    }

    #[test]
    fn bkr_007_duration_min_missing_for_type1() {
        let file = make_file(
            vec!["booking_rule_id", "booking_type"],
            vec![vec!["BR1", "1"]],
        );
        let (_, notices) = validate_booking_rules(&file);
        assert!(notices.iter().any(|n| n.rule_id == "BKR_007"), "BKR_007 bekleniyor");
    }

    #[test]
    fn bkr_008_last_day_missing_for_type2() {
        let file = make_file(
            vec!["booking_rule_id", "booking_type", "prior_notice_last_time"],
            vec![vec!["BR1", "2", "12:00:00"]],
        );
        let (_, notices) = validate_booking_rules(&file);
        assert!(notices.iter().any(|n| n.rule_id == "BKR_008"), "BKR_008 bekleniyor");
    }

    #[test]
    fn bkr_009_last_time_missing_for_type2() {
        let file = make_file(
            vec!["booking_rule_id", "booking_type", "prior_notice_last_day"],
            vec![vec!["BR1", "2", "3"]],
        );
        let (_, notices) = validate_booking_rules(&file);
        assert!(notices.iter().any(|n| n.rule_id == "BKR_009"), "BKR_009 bekleniyor");
    }

    #[test]
    fn bkr_001_last_day_forbidden_for_type1() {
        let file = make_file(
            vec!["booking_rule_id", "booking_type", "prior_notice_duration_min", "prior_notice_last_day"],
            vec![vec!["BR1", "1", "30", "2"]],
        );
        let (_, notices) = validate_booking_rules(&file);
        assert!(notices.iter().any(|n| n.rule_id == "BKR_001"), "BKR_001 bekleniyor");
    }

    #[test]
    fn bkr_001_start_day_allowed_for_type1_without_duration_max() {
        // Spec: prior_notice_start_day type=1'de yalnız prior_notice_duration_max
        // tanımlıysa yasak; yoksa Optional → BKR_001 ÇIKMAMALI (FP fix, 2026-07-24).
        let file = make_file(
            vec!["booking_rule_id", "booking_type", "prior_notice_duration_min", "prior_notice_start_day", "prior_notice_start_time"],
            vec![vec!["BR1", "1", "30", "2", "00:00:00"]],
        );
        let (_, notices) = validate_booking_rules(&file);
        assert!(
            !notices.iter().any(|n| n.rule_id == "BKR_001"),
            "type=1 + start_day (duration_max yok) BKR_001 üretmemeli (FP fix)"
        );
    }

    #[test]
    fn bkr_001_start_day_forbidden_for_type1_with_duration_max() {
        // duration_max tanımlıyken start_day yasaklanır.
        let file = make_file(
            vec!["booking_rule_id", "booking_type", "prior_notice_duration_min", "prior_notice_duration_max", "prior_notice_start_day", "prior_notice_start_time", "prior_notice_last_day", "prior_notice_last_time"],
            vec![vec!["BR1", "1", "30", "60", "2", "00:00:00", "2", "17:00:00"]],
        );
        let (_, notices) = validate_booking_rules(&file);
        assert!(
            notices.iter().any(|n| n.rule_id == "BKR_001" && n.field.as_deref() == Some("prior_notice_start_day")),
            "type=1 + duration_max + start_day BKR_001 üretmeli"
        );
    }

    #[test]
    fn bkr_004_prior_notice_forbidden_for_type0() {
        let file = make_file(
            vec!["booking_rule_id", "booking_type", "prior_notice_duration_min"],
            vec![vec!["BR1", "0", "15"]],
        );
        let (_, notices) = validate_booking_rules(&file);
        assert!(notices.iter().any(|n| n.rule_id == "BKR_004"), "BKR_004 bekleniyor");
    }

    #[test]
    fn bkr_006_duration_min_zero() {
        let file = make_file(
            vec!["booking_rule_id", "booking_type", "prior_notice_duration_min"],
            vec![vec!["BR1", "1", "0"]],
        );
        let (_, notices) = validate_booking_rules(&file);
        assert!(notices.iter().any(|n| n.rule_id == "BKR_006"), "BKR_006 bekleniyor");
    }

    #[test]
    fn bkr_011_last_day_after_start_day() {
        let file = make_file(
            vec!["booking_rule_id", "booking_type", "prior_notice_last_day", "prior_notice_last_time", "prior_notice_start_day", "prior_notice_start_time"],
            vec![vec!["BR1", "2", "5", "12:00:00", "3", "09:00:00"]],
        );
        let (_, notices) = validate_booking_rules(&file);
        assert!(notices.iter().any(|n| n.rule_id == "BKR_011"), "BKR_011 bekleniyor");
    }

    #[test]
    fn valid_type2_booking_no_notices() {
        let file = make_file(
            vec!["booking_rule_id", "booking_type", "prior_notice_last_day", "prior_notice_last_time"],
            vec![vec!["BR1", "2", "3", "12:00:00"]],
        );
        let (recs, notices) = validate_booking_rules(&file);
        assert_eq!(recs.len(), 1);
        assert!(notices.is_empty(), "Geçerli type=2 için notice olmamalı: {:?}", notices);
    }

    #[test]
    fn bkr_002_start_day_without_last_day() {
        let file = make_file(
            vec!["booking_rule_id", "booking_type", "prior_notice_last_time", "prior_notice_start_day", "prior_notice_start_time"],
            vec![vec!["BR1", "2", "12:00:00", "7", "09:00:00"]],
        );
        let (_, notices) = validate_booking_rules(&file);
        assert!(notices.iter().any(|n| n.rule_id == "BKR_002"), "BKR_002 bekleniyor");
    }

    // ── BKR_016/019 (issue #58: dosya bütünlüğü — zorunlu enum + birincil anahtar) ──
    #[test]
    fn bkr_016_invalid_booking_type() {
        let file = make_file(
            vec!["booking_rule_id", "booking_type"],
            vec![vec!["BR1", "7"]],
        );
        let (_, notices) = validate_booking_rules(&file);
        let n = notices.iter().find(|n| n.rule_id == "BKR_016").expect("BKR_016 bekleniyor");
        assert_eq!(n.observed_value.as_deref(), Some("7"));
    }

    #[test]
    fn bkr_016_empty_booking_type() {
        let file = make_file(
            vec!["booking_rule_id", "booking_type"],
            vec![vec!["BR1", ""]],
        );
        let (_, notices) = validate_booking_rules(&file);
        assert!(notices.iter().any(|n| n.rule_id == "BKR_016"), "boş booking_type → BKR_016");
    }

    #[test]
    fn bkr_016_silent_when_column_absent() {
        // Sütun başlıkta yoksa ARC_025 devralır (RTS_004 deseni) → satır başına BKR_016 yağmuru olmaz.
        let file = make_file(
            vec!["booking_rule_id"],
            vec![vec!["BR1"]],
        );
        let (_, notices) = validate_booking_rules(&file);
        assert!(!notices.iter().any(|n| n.rule_id == "BKR_016"),
            "booking_type sütunu yokken BKR_016 üretilmemeli: {:?}",
            notices.iter().map(|n| &n.rule_id).collect::<Vec<_>>());
    }

    #[test]
    fn bkr_016_silent_for_valid_types() {
        for t in ["0", "1", "2"] {
            let file = make_file(
                vec!["booking_rule_id", "booking_type", "prior_notice_duration_min", "prior_notice_last_day", "prior_notice_last_time"],
                vec![vec!["BR1", t, if t == "1" { "30" } else { "" }, if t == "2" { "3" } else { "" }, if t == "2" { "12:00:00" } else { "" }]],
            );
            let (_, notices) = validate_booking_rules(&file);
            assert!(!notices.iter().any(|n| n.rule_id == "BKR_016"), "booking_type={t} geçerli");
        }
    }

    #[test]
    fn bkr_019_duplicate_booking_rule_id() {
        let file = make_file(
            vec!["booking_rule_id", "booking_type", "prior_notice_duration_min"],
            vec![vec!["BR1", "1", "30"], vec!["BR1", "1", "45"]],
        );
        let (_, notices) = validate_booking_rules(&file);
        let dups: Vec<_> = notices.iter().filter(|n| n.rule_id == "BKR_019").collect();
        assert_eq!(dups.len(), 1, "yalnız İKİNCİ satır BKR_019 üretmeli: {dups:?}");
        assert_eq!(dups[0].line, Some(3));
    }

    #[test]
    fn bkr_019_missing_booking_rule_id() {
        let file = make_file(
            vec!["booking_rule_id", "booking_type", "prior_notice_duration_min"],
            vec![vec!["", "1", "30"]],
        );
        let (_, notices) = validate_booking_rules(&file);
        assert!(notices.iter().any(|n| n.rule_id == "BKR_019"), "boş booking_rule_id → BKR_019");
    }

    #[test]
    fn bkr_019_silent_for_unique_ids() {
        let file = make_file(
            vec!["booking_rule_id", "booking_type", "prior_notice_duration_min"],
            vec![vec!["BR1", "1", "30"], vec!["BR2", "1", "45"]],
        );
        let (_, notices) = validate_booking_rules(&file);
        assert!(!notices.iter().any(|n| n.rule_id == "BKR_019"), "benzersiz id → BKR_019 yok");
    }

    // ── BKR_012/013/014 (issue #56: spec presence matrisindeki üç boşluk) ──
    #[test]
    fn bkr_012_duration_min_forbidden_for_type2() {
        let file = make_file(
            vec!["booking_rule_id", "booking_type", "prior_notice_last_day", "prior_notice_last_time", "prior_notice_duration_min"],
            vec![vec!["BR1", "2", "3", "12:00:00", "30"]],
        );
        let (_, notices) = validate_booking_rules(&file);
        assert!(notices.iter().any(|n| n.rule_id == "BKR_012"), "BKR_012 bekleniyor: {:?}",
            notices.iter().map(|n| &n.rule_id).collect::<Vec<_>>());
    }

    #[test]
    fn bkr_012_silent_for_type1_where_duration_min_is_required() {
        let file = make_file(
            vec!["booking_rule_id", "booking_type", "prior_notice_duration_min"],
            vec![vec!["BR1", "1", "30"]],
        );
        let (_, notices) = validate_booking_rules(&file);
        assert!(!notices.iter().any(|n| n.rule_id == "BKR_012"),
            "type=1'de duration_min ZORUNLU → BKR_012 çıkmamalı: {:?}",
            notices.iter().map(|n| &n.rule_id).collect::<Vec<_>>());
    }

    #[test]
    fn bkr_013_last_time_without_last_day() {
        // type=1 + last_time: bugüne kadar hiçbir kural yakalamıyordu (issue #56/B).
        let file = make_file(
            vec!["booking_rule_id", "booking_type", "prior_notice_duration_min", "prior_notice_last_time"],
            vec![vec!["BR1", "1", "30", "17:00:00"]],
        );
        let (_, notices) = validate_booking_rules(&file);
        assert!(notices.iter().any(|n| n.rule_id == "BKR_013"), "BKR_013 bekleniyor: {:?}",
            notices.iter().map(|n| &n.rule_id).collect::<Vec<_>>());
    }

    #[test]
    fn bkr_013_silent_when_last_day_present() {
        let file = make_file(
            vec!["booking_rule_id", "booking_type", "prior_notice_last_day", "prior_notice_last_time"],
            vec![vec!["BR1", "2", "3", "12:00:00"]],
        );
        let (_, notices) = validate_booking_rules(&file);
        assert!(!notices.iter().any(|n| n.rule_id == "BKR_013"),
            "last_day varken BKR_013 çıkmamalı: {:?}",
            notices.iter().map(|n| &n.rule_id).collect::<Vec<_>>());
    }

    #[test]
    fn bkr_014_service_id_forbidden_for_type1() {
        let file = make_file(
            vec!["booking_rule_id", "booking_type", "prior_notice_duration_min", "prior_notice_service_id"],
            vec![vec!["BR1", "1", "30", "SVC1"]],
        );
        let (_, notices) = validate_booking_rules(&file);
        assert!(notices.iter().any(|n| n.rule_id == "BKR_014"), "BKR_014 bekleniyor: {:?}",
            notices.iter().map(|n| &n.rule_id).collect::<Vec<_>>());
    }

    #[test]
    fn bkr_014_silent_for_type2() {
        // Spec: "Optional if booking_type=2" → geçerli kullanım.
        let file = make_file(
            vec!["booking_rule_id", "booking_type", "prior_notice_last_day", "prior_notice_last_time", "prior_notice_service_id"],
            vec![vec!["BR1", "2", "3", "12:00:00", "SVC1"]],
        );
        let (_, notices) = validate_booking_rules(&file);
        assert!(notices.is_empty(), "type=2 + service_id geçerli, notice olmamalı: {:?}", notices);
    }

    #[test]
    fn bkr_010_start_day_without_start_time() {
        let file = make_file(
            vec!["booking_rule_id", "booking_type", "prior_notice_last_day", "prior_notice_last_time", "prior_notice_start_day"],
            vec![vec!["BR1", "2", "3", "12:00:00", "7"]],
        );
        let (_, notices) = validate_booking_rules(&file);
        assert!(notices.iter().any(|n| n.rule_id == "BKR_010"), "BKR_010 bekleniyor");
    }
    #[test]
    fn bkr_024_forbids_start_day_with_duration_max_on_same_day_booking() {
        let file = make_file(
            vec!["booking_rule_id", "booking_type", "prior_notice_duration_min",
                 "prior_notice_duration_max", "prior_notice_start_day", "prior_notice_last_day"],
            vec![
                vec!["B1", "1", "30", "120", "2", "3"],   // type=1 + max + start_day → BKR_024
                vec!["B2", "1", "30", "120", "", ""],     // start_day yok → sessiz
                vec!["B3", "1", "30", "", "2", "3"],      // max yok → sessiz
            ],
        );
        let (_, notices) = validate_booking_rules(&file);
        let hits: Vec<_> = notices.iter().filter(|n| n.rule_id == "BKR_024").collect();
        assert_eq!(hits.len(), 1, "yalnız B1: {hits:?}");
        assert_eq!(hits[0].entity_id.as_deref(), Some("B1"));
    }

}
