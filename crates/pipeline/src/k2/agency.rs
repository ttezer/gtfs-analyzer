use gtfs_core::EntityType;

use super::common::{get_lexical_field, get_raw_field,
    build_row_map, get_trimmed_field, looks_like_bcp47, looks_like_email,
    looks_like_iana_timezone, looks_like_phone, looks_like_url, make_k2_notice, parse_u32, RowMap,
};
use crate::k1_parse::RawFile;

#[derive(Debug, Clone)]
pub struct AgencyRecord {
    pub agency_id: Option<String>,
    pub agency_name: String,
    pub agency_url: String,
    pub agency_timezone: String,
    pub agency_lang: Option<String>,
    pub agency_phone: Option<String>,
    pub agency_fare_url: Option<String>,
    pub agency_email: Option<String>,
    pub agency_cemv_support: Option<u32>,
    pub row: RowMap,
    pub line: u64,
}

pub fn validate_agency(file: &RawFile) -> (Vec<AgencyRecord>, Vec<gtfs_core::Notice>) {
    let mut notices = Vec::new();
    let mut records = Vec::new();
    let mut counter = 0u32;

    // agency_id koşullu zorunlu: birden fazla kuruluş varsa her satırda agency_id gerekir
    // (aksi halde routes/fares hangi kuruluşa ait ayırt edilemez). MD: missing_required_agency_id.
    let multiple_agencies = file.rows.len() > 1;

    for (row_idx, row) in file.rows.iter().enumerate() {
        let line = (row_idx + 2) as u64;
        let row_map = build_row_map(&file.headers, row);

        let agency_id = get_raw_field(&row_map, "agency_id")
            .filter(|v| !v.trim().is_empty())
            .map(str::to_string);
        let entity_id = agency_id.clone();

        // AGN_014: birden fazla kuruluş varken bu satırda agency_id boş/eksik
        if multiple_agencies && agency_id.is_none() {
            notices.push(make_k2_notice(
                &mut counter, "AGN_014", EntityType::Agency, None, Some(&row_map),
                &file.name, Some(line), Some("agency_id"), Some(String::new()), None,
                "Birden fazla kuruluş tanımlıyken agency_id zorunludur.".to_string(),
                "Çoklu kuruluş durumunda her agency satırına benzersiz bir agency_id girin.",
            ));
        }

        // AGN_002: agency_name required (sütun başlıkta yoksa ARC_025 devralır → atla)
        let agency_name = get_trimmed_field(&row_map, "agency_name").unwrap_or("").to_string();
        if get_trimmed_field(&row_map, "agency_name") == Some("") {
            notices.push(make_k2_notice(
                &mut counter, "AGN_002", EntityType::Agency, entity_id.clone(), Some(&row_map),
                &file.name, Some(line), Some("agency_name"), Some(String::new()), None,
                "agency_name zorunludur.".to_string(), "agency_name alanını doldurun.",
            ));
        }

        // AGN_003: agency_url required + valid URL (sütun yoksa ARC_025 devralır → atla)
        //
        // ⚠️ İKİ AYRI OKUMA (issue #92): VARLIK kırpılmış değerle, GEÇERLİLİK ham değerle.
        // Tek okumaya indirmek iki yönden de hata verir: hepsi ham olsaydı `"   "` taşıyan
        // bir alan "geçersiz URL" olurdu (oysa o EKSİK alandır); hepsi kırpılmış olsaydı
        // `" https://x "` sessizce geçerli sayılırdı — issue #92'nin karşı örneği.
        let agency_url = get_lexical_field(&row_map, "agency_url").unwrap_or("").to_string();
        match get_trimmed_field(&row_map, "agency_url") {
            Some("") => notices.push(make_k2_notice(
                &mut counter, "AGN_003", EntityType::Agency, entity_id.clone(), Some(&row_map),
                &file.name, Some(line), Some("agency_url"), Some(String::new()), None,
                "agency_url zorunludur.".to_string(), "Geçerli bir http/https URL'si girin.",
            )),
            Some(_) if !looks_like_url(&agency_url) => notices.push(make_k2_notice(
                &mut counter, "AGN_003", EntityType::Agency, entity_id.clone(), Some(&row_map),
                &file.name, Some(line), Some("agency_url"), Some(agency_url.clone()), None,
                "agency_url geçerli bir URL değil.".to_string(), "Geçerli bir http/https URL'si kullanın.",
            )),
            _ => {}
        }

        // AGN_015: agency_url güvensiz 'http://' kullanıyor — 'https://' önerilir.
        if agency_url.starts_with("http://") {
            notices.push(make_k2_notice(
                &mut counter, "AGN_015", EntityType::Agency, entity_id.clone(), Some(&row_map),
                &file.name, Some(line), Some("agency_url"), Some(agency_url.clone()),
                Some("https://".to_string()),
                "agency_url güvensiz 'http://' kullanıyor — 'https://' önerilir.".to_string(),
                "agency_url'yi https:// ile güncelleyin.",
            ));
        }

        // AGN_016: agency_phone bilinen bir yer-tutucu/hizmet-dışı numara (veri üreticisinin
        // kurumsal numarası, tüm feed'lere kopyalanmış vb.) — gerçek iletişim numarası olmayabilir.
        if let Some(phone) = get_trimmed_field(&row_map, "agency_phone").filter(|v| !v.trim().is_empty()) {
            let digits: String = phone.chars().filter(|c| c.is_ascii_digit()).collect();
            const SUSPICIOUS_PHONES: &[&str] = &["8882812681", "18882812681"];
            if SUSPICIOUS_PHONES.contains(&digits.as_str()) {
                notices.push(make_k2_notice(
                    &mut counter, "AGN_016", EntityType::Agency, entity_id.clone(), Some(&row_map),
                    &file.name, Some(line), Some("agency_phone"), Some(phone.to_string()), None,
                    format!("agency_phone '{phone}' bilinen bir yer-tutucu/hizmet-dışı numara — gerçek iletişim numarası olmayabilir."),
                    "agency_phone'u işleten ajansın gerçek, çalışan numarasıyla güncelleyin.",
                ));
            }
        }

        // AGN_004: agency_timezone required + valid IANA (sütun yoksa ARC_025 devralır → atla)
        let agency_timezone = get_trimmed_field(&row_map, "agency_timezone").unwrap_or("").to_string();
        match get_trimmed_field(&row_map, "agency_timezone") {
            Some("") => notices.push(make_k2_notice(
                &mut counter, "AGN_004", EntityType::Agency, entity_id.clone(), Some(&row_map),
                &file.name, Some(line), Some("agency_timezone"), Some(String::new()), None,
                "agency_timezone zorunludur.".to_string(), "Geçerli bir IANA saat dilimi girin.",
            )),
            Some(_) if !looks_like_iana_timezone(&agency_timezone) => notices.push(make_k2_notice(
                &mut counter, "AGN_004", EntityType::Agency, entity_id.clone(), Some(&row_map),
                &file.name, Some(line), Some("agency_timezone"), Some(agency_timezone.clone()), None,
                format!("agency_timezone '{agency_timezone}' geçerli bir IANA saat dilimi değil."),
                "Geçerli bir IANA saat dilimi kullanın (örn. Europe/Istanbul).",
            )),
            _ => {}
        }

        // AGN_006: agency_lang must be valid BCP-47
        let agency_lang = get_lexical_field(&row_map, "agency_lang")
            .filter(|v| !v.trim().is_empty())
            .map(str::to_string);
        if let Some(ref lang) = agency_lang {
            if !looks_like_bcp47(lang) {
                notices.push(make_k2_notice(
                    &mut counter, "AGN_006", EntityType::Agency, entity_id.clone(), Some(&row_map),
                    &file.name, Some(line), Some("agency_lang"), Some(lang.clone()), None,
                    format!("agency_lang '{lang}' geçerli bir BCP-47 dil kodu değil."),
                    "Geçerli bir BCP-47 dil kodu kullanın (örn. tr, en-US); değerin başında veya sonunda boşluk bırakmayın.",
                ));
            }
        }

        // AGN_007: agency_phone format
        let agency_phone = get_trimmed_field(&row_map, "agency_phone")
            .filter(|v| !v.trim().is_empty())
            .map(str::to_string);
        if let Some(ref phone) = agency_phone {
            if !looks_like_phone(phone) {
                notices.push(make_k2_notice(
                    &mut counter, "AGN_007", EntityType::Agency, entity_id.clone(), Some(&row_map),
                    &file.name, Some(line), Some("agency_phone"), Some(phone.clone()), None,
                    format!("agency_phone '{phone}' geçerli bir telefon numarası formatında değil."),
                    "Çevrilebilir bir telefon numarası kullanın; numaranın dışında açıklayıcı metin bulunmasın.",
                ));
            }
        }

        // AGN_008: agency_fare_url valid URL if provided
        let agency_fare_url = get_lexical_field(&row_map, "agency_fare_url")
            .filter(|v| !v.trim().is_empty())
            .map(str::to_string);
        if let Some(ref url) = agency_fare_url {
            if !looks_like_url(url) {
                notices.push(make_k2_notice(
                    &mut counter, "AGN_008", EntityType::Agency, entity_id.clone(), Some(&row_map),
                    &file.name, Some(line), Some("agency_fare_url"), Some(url.clone()), None,
                    "agency_fare_url geçerli bir URL değil.".to_string(),
                    "agency_fare_url için geçerli bir http/https URL'si kullanın.",
                ));
            }
        }

        // AGN_009: agency_email valid email if provided
        let agency_email = get_lexical_field(&row_map, "agency_email")
            .filter(|v| !v.trim().is_empty())
            .map(str::to_string);
        if let Some(ref email) = agency_email {
            if !looks_like_email(email) {
                notices.push(make_k2_notice(
                    &mut counter, "AGN_009", EntityType::Agency, entity_id.clone(), Some(&row_map),
                    &file.name, Some(line), Some("agency_email"), Some(email.clone()), None,
                    "agency_email geçerli bir e-posta adresi değil.".to_string(),
                    "agency_email için geçerli bir e-posta adresi kullanın.",
                ));
            }
        }

        // AGN_012: agency_cemv_support must be 0, 1 or 2 (GTFS: 0/empty=bilinmiyor, 1=destekleniyor, 2=desteklenmiyor)
        let agency_cemv_support = match parse_u32(&row_map, "cemv_support") {
            Ok(v) => {
                if let Some(val) = v {
                    if val > 2 {
                        notices.push(make_k2_notice(
                            &mut counter, "AGN_012", EntityType::Agency, entity_id.clone(), Some(&row_map),
                            &file.name, Some(line), Some("cemv_support"), Some(val.to_string()),
                            Some("0, 1 veya 2".to_string()),
                            "cemv_support 0, 1 veya 2 olmalıdır.".to_string(),
                            "cemv_support alanını 0 (bilgi yok), 1 (destekleniyor) veya 2 (desteklenmiyor) olarak ayarlayın.",
                        ));
                    }
                }
                v
            }
            Err(err) => {
                // Sayı OLMAYAN değer eskiden sessizce düşüyordu: aralık dışı sayı AGN_012 üretirken
                // "abc" hiçbir bulgu vermiyordu. Aynı olgunun iki dalı → aynı kural (PTH_027 emsali).
                notices.push(make_k2_notice(
                    &mut counter, "AGN_012", EntityType::Agency, entity_id.clone(),
                    Some(&row_map), &file.name, Some(line), Some("cemv_support"),
                    get_trimmed_field(&row_map, "cemv_support").map(str::to_string), Some("0, 1 veya 2".to_string()),
                    err,
                    "cemv_support alanını 0 (bilgi yok), 1 (destekleniyor) veya 2 (desteklenmiyor) olarak ayarlayın.",
                ));
                None
            }
        };

        records.push(AgencyRecord {
            agency_id,
            agency_name,
            agency_url,
            agency_timezone,
            agency_lang,
            agency_phone,
            agency_fare_url,
            agency_email,
            agency_cemv_support,
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
        RawFile {
            name: "agency.txt".to_string(),
            headers: headers.into_iter().map(str::to_string).collect(),
            rows: rows.into_iter().map(|r| r.into_iter().map(smol_str::SmolStr::from).collect()).collect(),
            bytes: 0,
            raw_text: None,
        }
    }

    #[test]
    fn valid_agency_produces_no_notices() {
        let file = make_file(
            vec!["agency_id", "agency_name", "agency_url", "agency_timezone"],
            vec![vec!["A1", "Transit Co", "https://transit.example", "Europe/Istanbul"]],
        );
        let (records, notices) = validate_agency(&file);
        assert_eq!(records.len(), 1);
        assert!(notices.is_empty(), "Geçerli ajans notice üretmemeli: {:?}", notices);
    }

    #[test]
    fn missing_agency_name_produces_agn_002() {
        let file = make_file(
            vec!["agency_id", "agency_name", "agency_url", "agency_timezone"],
            vec![vec!["A1", "", "https://transit.example", "Europe/Istanbul"]],
        );
        let (_, notices) = validate_agency(&file);
        assert!(notices.iter().any(|n| n.rule_id == "AGN_002"));
    }

    #[test]
    fn missing_agency_url_produces_agn_003() {
        let file = make_file(
            vec!["agency_id", "agency_name", "agency_url", "agency_timezone"],
            vec![vec!["A1", "Transit Co", "", "Europe/Istanbul"]],
        );
        let (_, notices) = validate_agency(&file);
        assert!(notices.iter().any(|n| n.rule_id == "AGN_003"));
    }

    #[test]
    fn invalid_timezone_produces_agn_004() {
        let file = make_file(
            vec!["agency_id", "agency_name", "agency_url", "agency_timezone"],
            vec![vec!["A1", "Transit Co", "https://transit.example", "InvalidTZ"]],
        );
        let (_, notices) = validate_agency(&file);
        assert!(notices.iter().any(|n| n.rule_id == "AGN_004"));
    }

    #[test]
    fn invalid_lang_produces_agn_006() {
        let file = make_file(
            vec!["agency_id", "agency_name", "agency_url", "agency_timezone", "agency_lang"],
            vec![vec!["A1", "TC", "https://tc.example", "Europe/Istanbul", "not valid lang!!"]],
        );
        let (_, notices) = validate_agency(&file);
        assert!(notices.iter().any(|n| n.rule_id == "AGN_006"));
    }

    #[test]
    fn invalid_email_produces_agn_009() {
        let file = make_file(
            vec!["agency_id", "agency_name", "agency_url", "agency_timezone", "agency_email"],
            vec![vec!["A1", "TC", "https://tc.example", "Europe/Istanbul", "notanemail"]],
        );
        let (_, notices) = validate_agency(&file);
        assert!(notices.iter().any(|n| n.rule_id == "AGN_009"));
    }

    #[test]
    fn multiple_agencies_missing_agency_id_produces_agn_014() {
        let file = make_file(
            vec!["agency_id", "agency_name", "agency_url", "agency_timezone"],
            vec![
                vec!["A1", "Transit Co", "https://transit.example", "Europe/Istanbul"],
                vec!["", "Other Co", "https://other.example", "Europe/Istanbul"],
            ],
        );
        let (_, notices) = validate_agency(&file);
        assert!(notices.iter().any(|n| n.rule_id == "AGN_014"),
            "Çoklu kuruluşta boş agency_id → AGN_014: {:?}",
            notices.iter().map(|n| &n.rule_id).collect::<Vec<_>>());
    }

    #[test]
    fn single_agency_missing_agency_id_silent_for_agn_014() {
        let file = make_file(
            vec!["agency_id", "agency_name", "agency_url", "agency_timezone"],
            vec![vec!["", "Transit Co", "https://transit.example", "Europe/Istanbul"]],
        );
        let (_, notices) = validate_agency(&file);
        assert!(!notices.iter().any(|n| n.rule_id == "AGN_014"),
            "Tek kuruluşta agency_id opsiyonel → AGN_014 yok: {:?}",
            notices.iter().map(|n| &n.rule_id).collect::<Vec<_>>());
    }
}
