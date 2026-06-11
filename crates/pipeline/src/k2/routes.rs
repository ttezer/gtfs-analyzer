use std::collections::HashMap;

use gtfs_core::EntityType;

use super::common::{
    build_row_map, get_trimmed_field, is_hex_color_6, looks_like_url, make_k2_notice,
    parse_u32, wcag_contrast_ratio, RowMap,
};
use crate::k1_parse::RawFile;

#[derive(Debug, Clone)]
pub struct RouteRecord {
    pub route_id: String,
    pub agency_id: Option<String>,
    pub route_short_name: Option<String>,
    pub route_long_name: Option<String>,
    pub route_desc: Option<String>,
    pub route_type: Option<u32>,
    pub route_url: Option<String>,
    pub route_color: Option<String>,
    pub route_text_color: Option<String>,
    pub route_sort_order: Option<u32>,
    pub continuous_pickup: Option<u32>,
    pub continuous_drop_off: Option<u32>,
    pub network_id: Option<String>,
    pub route_cemv_support: Option<u32>,
    pub row: RowMap,
    pub line: u64,
}

/// Returns true if the given route_type value is valid per GTFS spec (basic + extended).
fn is_valid_route_type(v: u32) -> bool {
    // Basic: 0-7, 11, 12
    // Extended: 100-1799
    matches!(v, 0..=7 | 11 | 12 | 100..=1799)
}

pub fn validate_routes(file: &RawFile) -> (Vec<RouteRecord>, Vec<gtfs_core::Notice>) {
    let mut notices = Vec::new();
    let mut records = Vec::new();
    let mut counter = 0u32;

    for (row_idx, row) in file.rows.iter().enumerate() {
        let line = (row_idx + 2) as u64;
        let row_map = build_row_map(&file.headers, row);

        let route_id = get_trimmed_field(&row_map, "route_id").unwrap_or("").to_string();
        let entity_id = (!route_id.is_empty()).then_some(route_id.clone());

        let agency_id = get_trimmed_field(&row_map, "agency_id")
            .filter(|v| !v.is_empty())
            .map(str::to_string);

        let route_short_name = get_trimmed_field(&row_map, "route_short_name")
            .filter(|v| !v.is_empty())
            .map(str::to_string);
        let route_long_name = get_trimmed_field(&row_map, "route_long_name")
            .filter(|v| !v.is_empty())
            .map(str::to_string);

        // RTS_003: both route_short_name and route_long_name are empty
        if route_short_name.is_none() && route_long_name.is_none() {
            notices.push(make_k2_notice(
                &mut counter, "RTS_003", EntityType::Route, entity_id.clone(), Some(&row_map),
                &file.name, Some(line), None, None, None,
                "route_short_name ve route_long_name alanlarının her ikisi de boş; en az biri zorunludur.".to_string(),
                "route_short_name veya route_long_name alanını doldurun.",
            ));
        }

        // RTS_009: short_name ve long_name birbirinin aynısı
        if let (Some(ref s), Some(ref l)) = (&route_short_name, &route_long_name) {
            if s.to_lowercase() == l.to_lowercase() {
                notices.push(make_k2_notice(
                    &mut counter, "RTS_009", EntityType::Route, entity_id.clone(), Some(&row_map),
                    &file.name, Some(line), Some("route_long_name"), Some(l.clone()), None,
                    format!("'{}' hattında route_short_name ve route_long_name aynı değer: '{s}'.", route_id),
                    "route_long_name, route_short_name'den farklı ve daha açıklayıcı olmalıdır.",
                ));
            }
        }

        // RTS_010: route_short_name çok uzun (>12 karakter)
        if let Some(ref s) = route_short_name {
            if s.len() > 12 {
                notices.push(make_k2_notice(
                    &mut counter, "RTS_010", EntityType::Route, entity_id.clone(), Some(&row_map),
                    &file.name, Some(line), Some("route_short_name"),
                    Some(s.len().to_string()), Some("≤12".to_string()),
                    format!("'{}' hattının route_short_name değeri {} karakter; kısa isimler 12 karakteri aşmamalıdır.", route_id, s.len()),
                    "route_short_name'i 12 karakterin altında tutun.",
                ));
            } else if s.len() > 6 {
                // RTS_021: 6 karakterden uzun (Google Transit eşiği)
                notices.push(make_k2_notice(
                    &mut counter, "RTS_021", EntityType::Route, entity_id.clone(), Some(&row_map),
                    &file.name, Some(line), Some("route_short_name"),
                    Some(s.len().to_string()), Some("≤6 (Google)".to_string()),
                    format!("'{}' hattının route_short_name değeri {} karakter; Google Transit 6 karakteri tavsiye ediyor.", route_id, s.len()),
                    "Google Transit uyumluluğu için route_short_name'i 6 karakterin altında tutun.",
                ));
            }
        }

        // RTS_011: route_long_name çok uzun (>100 karakter)
        if let Some(ref l) = route_long_name {
            if l.len() > 100 {
                notices.push(make_k2_notice(
                    &mut counter, "RTS_011", EntityType::Route, entity_id.clone(), Some(&row_map),
                    &file.name, Some(line), Some("route_long_name"),
                    Some(l.len().to_string()), Some("≤100".to_string()),
                    format!("'{}' hattının route_long_name değeri {} karakter; 100 karakteri aşıyor.", route_id, l.len()),
                    "route_long_name'i 100 karakterin altında tutun.",
                ));
            }
        }

        // route_type: required + must be a valid GTFS enum
        let route_type = match parse_u32(&row_map, "route_type") {
            Ok(v) => {
                match v {
                    None => {
                        // RTS_004: route_type missing (sütun yoksa ARC_025 devralır → atla)
                        if get_trimmed_field(&row_map, "route_type") == Some("") {
                            notices.push(make_k2_notice(
                                &mut counter, "RTS_004", EntityType::Route, entity_id.clone(), Some(&row_map),
                                &file.name, Some(line), Some("route_type"),
                                Some(String::new()), None,
                                "route_type zorunludur.".to_string(),
                                "Geçerli bir route_type girin (0-7, 11, 12 veya genişletilmiş 100-1799).",
                            ));
                        }
                        None
                    }
                    Some(val) => {
                        if !is_valid_route_type(val) {
                            // RTS_004: route_type invalid enum value
                            notices.push(make_k2_notice(
                                &mut counter, "RTS_004", EntityType::Route, entity_id.clone(), Some(&row_map),
                                &file.name, Some(line), Some("route_type"),
                                Some(val.to_string()), Some("0-7,11,12,100-1799".to_string()),
                                format!("route_type {val} geçerli bir GTFS hat tipi değil."),
                                "Geçerli bir route_type kullanın (0-7, 11, 12 veya genişletilmiş 100-1799).",
                            ));
                        }
                        Some(val)
                    }
                }
            }
            Err(err) => {
                notices.push(make_k2_notice(
                    &mut counter, "RTS_004", EntityType::Route, entity_id.clone(), Some(&row_map),
                    &file.name, Some(line), Some("route_type"),
                    get_trimmed_field(&row_map, "route_type").map(str::to_string),
                    None, err,
                    "Geçerli bir sayısal route_type girin.",
                ));
                None
            }
        };

        // RTS_005: route_url must be valid URL if provided
        let route_url = get_trimmed_field(&row_map, "route_url")
            .filter(|v| !v.is_empty())
            .map(str::to_string);
        if let Some(ref url) = route_url {
            if !looks_like_url(url) {
                notices.push(make_k2_notice(
                    &mut counter, "RTS_005", EntityType::Route, entity_id.clone(), Some(&row_map),
                    &file.name, Some(line), Some("route_url"), Some(url.clone()), None,
                    "route_url geçerli bir URL değil.".to_string(),
                    "route_url için geçerli bir http/https URL'si kullanın.",
                ));
            }
        }

        // RTS_006: route_color must be valid 6-char hex
        let route_color = get_trimmed_field(&row_map, "route_color")
            .filter(|v| !v.is_empty())
            .map(str::to_string);
        if let Some(ref color) = route_color {
            if !is_hex_color_6(color) {
                notices.push(make_k2_notice(
                    &mut counter, "RTS_006", EntityType::Route, entity_id.clone(), Some(&row_map),
                    &file.name, Some(line), Some("route_color"), Some(color.clone()),
                    Some("6 haneli hex".to_string()),
                    "route_color 6 karakterli hex renk kodu olmalıdır.".to_string(),
                    "Geçerli bir 6 haneli hex renk kullanın (örn. FF0000).",
                ));
            }
        }

        // RTS_007: route_text_color must be valid 6-char hex
        let route_text_color = get_trimmed_field(&row_map, "route_text_color")
            .filter(|v| !v.is_empty())
            .map(str::to_string);
        if let Some(ref color) = route_text_color {
            if !is_hex_color_6(color) {
                notices.push(make_k2_notice(
                    &mut counter, "RTS_007", EntityType::Route, entity_id.clone(), Some(&row_map),
                    &file.name, Some(line), Some("route_text_color"), Some(color.clone()),
                    Some("6 haneli hex".to_string()),
                    "route_text_color 6 karakterli hex renk kodu olmalıdır.".to_string(),
                    "Geçerli bir 6 haneli hex renk kullanın (örn. FFFFFF).",
                ));
            }
        }

        // RTS_008: WCAG contrast ratio check
        if let (Some(ref fg), Some(ref bg)) = (&route_text_color, &route_color) {
            if is_hex_color_6(fg) && is_hex_color_6(bg) {
                if let Some(ratio) = wcag_contrast_ratio(fg, bg) {
                    if ratio < 3.0 {
                        notices.push(make_k2_notice(
                            &mut counter, "RTS_008", EntityType::Route, entity_id.clone(), Some(&row_map),
                            &file.name, Some(line), Some("route_text_color"),
                            Some(format!("{ratio:.2}")), Some(">= 3.0".to_string()),
                            format!("route_text_color ile route_color arasındaki kontrast oranı düşük ({ratio:.2})."),
                            "En az 3:1 WCAG kontrast oranına sahip renkler kullanın.",
                        ));
                    }
                }
            }
        }

        // RTS_013: continuous_pickup invalid enum
        let continuous_pickup = parse_continuous_field(
            &row_map, &mut notices, &mut counter, "RTS_013", "continuous_pickup", &entity_id, line, &file.name,
        );

        // RTS_018: continuous_drop_off invalid enum
        let continuous_drop_off = parse_continuous_field(
            &row_map, &mut notices, &mut counter, "RTS_018", "continuous_drop_off", &entity_id, line, &file.name,
        );

        let route_sort_order = match parse_u32(&row_map, "route_sort_order") {
            Ok(v) => v,
            Err(_) => None,
        };

        let route_desc = get_trimmed_field(&row_map, "route_desc")
            .filter(|v| !v.is_empty())
            .map(str::to_string);

        // RTS_023: route_long_name == route_desc (same_name_and_description_for_route)
        if let (Some(ref l), Some(ref d)) = (&route_long_name, &route_desc) {
            if l.to_lowercase() == d.to_lowercase() {
                notices.push(make_k2_notice(
                    &mut counter, "RTS_023", EntityType::Route, entity_id.clone(), Some(&row_map),
                    &file.name, Some(line), Some("route_desc"), Some(d.clone()), None,
                    format!("'{}' hattının route_long_name ve route_desc değerleri aynı: '{l}'.", route_id),
                    "route_desc, route_long_name'den farklı ve daha açıklayıcı bir açıklama içermelidir.",
                ));
            }
        }

        let network_id = get_trimmed_field(&row_map, "network_id")
            .filter(|v| !v.is_empty())
            .map(str::to_string);

        let route_cemv_support = match parse_u32(&row_map, "route_cemv_support") {
            Ok(v) => v,
            Err(_) => None,
        };

        records.push(RouteRecord {
            route_id,
            agency_id,
            route_short_name,
            route_long_name,
            route_desc,
            route_type,
            route_url,
            route_color,
            route_text_color,
            route_sort_order,
            continuous_pickup,
            continuous_drop_off,
            network_id,
            route_cemv_support,
            row: row_map,
            line,
        });
    }

    // RTS_019: Yinelenen hat adı — önce tüm adları topla, sonra her çakışan için notice üret
    {
        let mut short_groups: HashMap<String, Vec<(String, u64)>> = HashMap::new(); // key → Vec<(route_id, line)>
        let mut long_groups:  HashMap<String, Vec<(String, u64)>> = HashMap::new();
        for rec in &records {
            if let Some(ref sn) = rec.route_short_name {
                if !sn.is_empty() {
                    short_groups.entry(sn.to_lowercase()).or_default().push((rec.route_id.clone(), rec.line));
                }
            }
            if let Some(ref ln) = rec.route_long_name {
                if !ln.is_empty() {
                    long_groups.entry(ln.to_lowercase()).or_default().push((rec.route_id.clone(), rec.line));
                }
            }
        }
        for (key, entries) in &short_groups {
            if entries.len() < 2 { continue; }
            // Kısa adlar aynı olduğu için hatları route_id ile ayırt et; ismi paylaşan TÜM hatları listele
            let group_str = entries.iter().map(|(id, _)| id.as_str()).collect::<Vec<_>>().join(", ");
            for (route_id, line) in entries {
                let display_name = records.iter().find(|r| &r.route_id == route_id)
                    .and_then(|r| r.route_short_name.as_deref())
                    .unwrap_or(key.as_str());
                let mut n = make_k2_notice(
                    &mut counter, "RTS_019", EntityType::Route,
                    Some(route_id.clone()), None,
                    "routes.txt", Some(*line), Some("route_short_name"),
                    Some(display_name.to_string()), None,
                    format!("route_short_name '{}' şu hatlar tarafından paylaşılıyor: {}.", display_name, group_str),
                    "Her hatta benzersiz bir kısa ad verin veya bilerek paylaşılıyorsa bu uyarıyı görmezden gelin.",
                );
                n.details = Some([("conflicting_routes".to_string(), group_str.clone())].into_iter().collect());
                notices.push(n);
            }
        }
        for (key, entries) in &long_groups {
            if entries.len() < 2 { continue; }
            // Uzun adlar aynı; hatları route_short_name ile listele (yoksa route_id) — ismi paylaşan TÜM hatlar
            let group_str = entries.iter().map(|(id, _)| {
                records.iter().find(|r| &r.route_id == id)
                    .and_then(|r| r.route_short_name.clone())
                    .filter(|s| !s.is_empty())
                    .unwrap_or_else(|| id.clone())
            }).collect::<Vec<_>>().join(", ");
            for (route_id, line) in entries {
                let display_name = records.iter().find(|r| &r.route_id == route_id)
                    .and_then(|r| r.route_long_name.as_deref())
                    .unwrap_or(key.as_str());
                let mut n = make_k2_notice(
                    &mut counter, "RTS_019", EntityType::Route,
                    Some(route_id.clone()), None,
                    "routes.txt", Some(*line), Some("route_long_name"),
                    Some(display_name.to_string()), None,
                    format!("route_long_name '{}' şu hatlar tarafından paylaşılıyor: {}.", display_name, group_str),
                    "Her hatta benzersiz bir uzun ad verin veya bilerek paylaşılıyorsa bu uyarıyı görmezden gelin.",
                );
                n.details = Some([("conflicting_routes".to_string(), group_str.clone())].into_iter().collect());
                notices.push(n);
            }
        }
    }

    (records, notices)
}

fn parse_continuous_field(
    row_map: &RowMap,
    notices: &mut Vec<gtfs_core::Notice>,
    counter: &mut u32,
    rule_id: &str,
    field: &str,
    entity_id: &Option<String>,
    line: u64,
    file_name: &str,
) -> Option<u32> {
    match parse_u32(row_map, field) {
        Ok(v) => {
            if let Some(val) = v {
                if !matches!(val, 0 | 1 | 2 | 3) {
                    notices.push(make_k2_notice(
                        counter, rule_id, EntityType::Route, entity_id.clone(), Some(row_map),
                        file_name, Some(line), Some(field), Some(val.to_string()),
                        Some("0-3".to_string()),
                        format!("{field} alanı 0, 1, 2 veya 3 olmalıdır."),
                        "Alanı geçerli bir GTFS sürekli servis değerine ayarlayın (0-3).",
                    ));
                }
            }
            v
        }
        Err(_) => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::k1_parse::RawFile;

    fn make_file(headers: Vec<&str>, rows: Vec<Vec<&str>>) -> RawFile {
        RawFile {
            name: "routes.txt".to_string(),
            headers: headers.into_iter().map(str::to_string).collect(),
            rows: rows.into_iter().map(|r| r.into_iter().map(smol_str::SmolStr::from).collect()).collect(),
            bytes: 0,
            raw_text: None,
        }
    }

    #[test]
    fn valid_route_produces_no_notices() {
        let file = make_file(
            vec!["route_id", "route_short_name", "route_type"],
            vec![vec!["R1", "101", "3"]],
        );
        let (records, notices) = validate_routes(&file);
        assert_eq!(records.len(), 1);
        assert!(notices.is_empty(), "Geçerli rota notice üretmemeli: {:?}", notices);
    }

    #[test]
    fn invalid_route_type_produces_RTS_004() {
        let file = make_file(
            vec!["route_id", "route_short_name", "route_type"],
            vec![vec!["R1", "10", "99"]],
        );
        let (_, notices) = validate_routes(&file);
        assert!(notices.iter().any(|n| n.rule_id == "RTS_004"),
            "route_type=99 must produce RTS_004, got: {:?}", notices.iter().map(|n| &n.rule_id).collect::<Vec<_>>());
    }

    #[test]
    fn both_names_empty_produces_RTS_003() {
        let file = make_file(
            vec!["route_id", "route_short_name", "route_long_name", "route_type"],
            vec![vec!["R1", "", "", "3"]],
        );
        let (_, notices) = validate_routes(&file);
        assert!(notices.iter().any(|n| n.rule_id == "RTS_003"));
    }

    #[test]
    fn invalid_route_color_produces_RTS_006() {
        let file = make_file(
            vec!["route_id", "route_short_name", "route_type", "route_color"],
            vec![vec!["R1", "10", "3", "ZZZZZZ"]],
        );
        let (_, notices) = validate_routes(&file);
        assert!(notices.iter().any(|n| n.rule_id == "RTS_006"));
    }

    #[test]
    fn extended_route_type_is_valid() {
        let file = make_file(
            vec!["route_id", "route_short_name", "route_type"],
            vec![vec!["R1", "Metro", "401"]],
        );
        let (_, notices) = validate_routes(&file);
        assert!(!notices.iter().any(|n| n.rule_id == "RTS_004"),
            "Extended route_type 401 should be valid");
    }
}
