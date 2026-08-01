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
    pub jp_office_id: Option<String>,
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
        //
        // `field` iki alanı BİRLİKTE adlandırır (repoda yerleşik boru konvansiyonu:
        // `from_stop_id|to_stop_id`, `start_date|end_date`, `stop_lat|stop_lon`). Eskiden `None`
        // idi: R2'nin "Alan" sütunu, feed'i yayından ALIKOYAN bir bulguda boş kalıyordu.
        // Tek alan adı yazmak yanlış olurdu — hüküm ikisinin BİRLİKTE boş olmasıdır.
        if route_short_name.is_none() && route_long_name.is_none() {
            notices.push(make_k2_notice(
                &mut counter, "RTS_003", EntityType::Route, entity_id.clone(), Some(&row_map),
                &file.name, Some(line), Some("route_short_name|route_long_name"), None, None,
                "route_short_name ve route_long_name alanlarının her ikisi de boş; en az biri zorunludur.".to_string(),
                "route_short_name veya route_long_name alanını doldurun.",
            ));
        }

        // RTS_009 (short == long) KALDIRILDI (2026-07-23): RTS_022'nin (K6,
        // route_long_name_contains_short_name) saf alt kümesiydi — eşitlik de bir
        // "içerme" vakasıdır. RTS_022 MD paritesi taşır ve eşitliği zaten yakalar;
        // RTS_009 kaldırılınca parite korunur (RTS_022'ye dokunulmadı).

        // RTS_010: route_short_name çok uzun (>12 karakter)
        if let Some(ref s) = route_short_name {
            // KARAKTER sayısı, BAYT değil. `str::len()` UTF-8 bayt döndürür: "Kızılay"
            // 7 karakter ama 8 bayt, Japonca üç karakterlik bir kısa ad 9 bayttır. Eşik
            // ve mesaj "karakter" dediği için Türkçe/Japonca feed'lerde bayt sayımı
            // yanlış pozitif üretiyordu.
            let char_len = s.chars().count();
            if char_len > 12 {
                notices.push(make_k2_notice(
                    &mut counter, "RTS_010", EntityType::Route, entity_id.clone(), Some(&row_map),
                    &file.name, Some(line), Some("route_short_name"),
                    Some(char_len.to_string()), Some("≤12".to_string()),
                    format!("'{}' hattının route_short_name değeri {} karakter; kısa isimler 12 karakteri aşmamalıdır.", route_id, char_len),
                    "route_short_name'i 12 karakterin altında tutun.",
                ));
            } else if char_len > 6 && route_long_name.as_ref().is_none_or(|l| l.trim().is_empty()) {
                // RTS_021: 6 karakterden uzun (Google Transit eşiği).
                // Google'ın kendi muafiyeti: `route_long_name` doluysa uyarı göz ardı
                // edilebilir — uzun ad zaten okunabilir gösterimi taşır. Guard olmadan
                // "Green Line" gibi meşru adlandırmalarda gereksiz gürültü üretiyordu.
                notices.push(make_k2_notice(
                    &mut counter, "RTS_021", EntityType::Route, entity_id.clone(), Some(&row_map),
                    &file.name, Some(line), Some("route_short_name"),
                    Some(char_len.to_string()), Some("≤6 (Google)".to_string()),
                    format!("'{}' hattının route_short_name değeri {} karakter; Google Transit 6 karakteri tavsiye ediyor.", route_id, char_len),
                    "Google Transit uyumluluğu için route_short_name'i 6 karakterin altında tutun ya da route_long_name'i doldurun.",
                ));
            }
        }

        // RTS_011: route_long_name çok uzun (>100 karakter)
        if let Some(ref l) = route_long_name {
            let char_len = l.chars().count(); // bayt değil karakter — bkz. RTS_010/021 notu
            if char_len > 100 {
                notices.push(make_k2_notice(
                    &mut counter, "RTS_011", EntityType::Route, entity_id.clone(), Some(&row_map),
                    &file.name, Some(line), Some("route_long_name"),
                    Some(char_len.to_string()), Some("≤100".to_string()),
                    format!("'{}' hattının route_long_name değeri {} karakter; 100 karakteri aşıyor.", route_id, char_len),
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

        // RTS_029: spec tipi `Non-negative integer`. Negatif ya da sayı olmayan değer eskiden
        // sessizce düşüyordu — hattın sıralaması kayboluyor, kullanıcı hiçbir şey görmüyordu.
        let route_sort_order = match parse_u32(&row_map, "route_sort_order") {
            Ok(v) => v,
            Err(err) => {
                notices.push(make_k2_notice(
                    &mut counter, "RTS_029", EntityType::Route, entity_id.clone(), Some(&row_map),
                    &file.name, Some(line), Some("route_sort_order"),
                    get_trimmed_field(&row_map, "route_sort_order").map(str::to_string),
                    Some("negatif olmayan tam sayı".to_string()), err,
                    "route_sort_order değerini negatif olmayan bir tam sayı olarak girin.",
                ));
                None
            }
        };

        let route_desc = get_trimmed_field(&row_map, "route_desc")
            .filter(|v| !v.is_empty())
            .map(str::to_string);

        // RTS_023: route_desc, route_long_name VEYA route_short_name ile aynı
        // (MD `same_name_and_description_for_route`; MD örnekleri `specifiedField` ile
        // hangi alanın eşleştiğini söyler ve her İKİ alanı da denetler).
        //
        // route_short_name kolu 2026-07-17'de eklendi: yalnız long_name'e bakmak gerçek
        // bir KAPSAM BOŞLUĞUYDU — 250-feed corpus'ta mdb-2898'de MD 436 bulgu verirken biz
        // 0 veriyorduk ve 436'sının da `specifiedField` değeri route_short_name'di
        // (route_desc="EXT" ↔ route_short_name="EXT" gibi).
        //
        // Her iki alan da eşleşse bile hat başına TEK notice (aynı kök neden: açıklama
        // bir adın kopyası); hangi alan(lar) eşleşti mesajda yazar.
        {
            let dl = route_desc.as_ref().map(|d| d.to_lowercase());
            let matches_long = matches!((&route_long_name, &dl), (Some(l), Some(d)) if &l.to_lowercase() == d);
            let matches_short = matches!((&route_short_name, &dl), (Some(s), Some(d)) if &s.to_lowercase() == d);
            if let (Some(d), true) = (&route_desc, matches_long || matches_short) {
                // Eşleşen alan adı `details.matched_field`'e yazılır: mesaj şablonları
                // (tr/en/ja) hangi alanın kopyalandığını buradan okur — locale'de sabit
                // "route_long_name" yazmak short_name vakalarında YANLIŞ BİLGİ olurdu.
                let matched_field = match (matches_long, matches_short) {
                    (true, true) => "route_long_name, route_short_name",
                    (true, false) => "route_long_name",
                    _ => "route_short_name",
                };
                let mut n = make_k2_notice(
                    &mut counter, "RTS_023", EntityType::Route, entity_id.clone(), Some(&row_map),
                    &file.name, Some(line), Some("route_desc"), Some(d.clone()), None,
                    format!("'{route_id}' hattının route_desc değeri {matched_field} ile aynı: '{d}'."),
                    "route_desc, hat adlarından farklı ve daha açıklayıcı bir açıklama içermelidir.",
                );
                n.details = Some(std::collections::BTreeMap::from([
                    ("matched_field".to_string(), matched_field.to_string()),
                ]));
                notices.push(n);
            }
        }

        let network_id = get_trimmed_field(&row_map, "network_id")
            .filter(|v| !v.is_empty())
            .map(str::to_string);

        // GTFS-JP: jp_office_id (resmî spec routes.txt'te tanımlar; JPN_002 office_jp FK denetimi)
        let jp_office_id = get_trimmed_field(&row_map, "jp_office_id")
            .filter(|v| !v.is_empty())
            .map(str::to_string);

        // RTS_024: route_cemv_support 0, 1 veya 2 olmalı (AGN_012 route ikizi)
        let route_cemv_support = match parse_u32(&row_map, "cemv_support") {
            Ok(v) => {
                if let Some(val) = v {
                    if val > 2 {
                        notices.push(make_k2_notice(
                            &mut counter, "RTS_024", EntityType::Route, entity_id.clone(), Some(&row_map),
                            &file.name, Some(line), Some("cemv_support"), Some(val.to_string()),
                            Some("0, 1 veya 2".to_string()),
                            "cemv_support 0, 1 veya 2 olmalıdır.".to_string(),
                            "cemv_support alanını 0 (bilgi yok), 1 (destekleniyor) veya 2 (desteklenmiyor) olarak ayarlayın.",
                        ));
                    }
                }
                v
            }
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
            jp_office_id,
            row: row_map,
            line,
        });
    }

    // Aşağıdaki yinelenen-ad kontrolleri her grubun anchor route kaydına tekrar erişir.
    // `records.iter().find(...)` kullanmak, çok sayıda route ve yinelenen grup içeren
    // feed'lerde O(grup × route) davranışı üretir (VlakEShopAll: 64.711 grup × 321.962
    // route). İlk kaydı koruyan bu indeks mevcut semantiği değiştirmeden lookup'u O(1)
    // yapar; yinelenen route_id varsa önceki doğrusal `find` gibi ilk satır kazanır.
    let mut route_by_id: HashMap<&str, &RouteRecord> = HashMap::with_capacity(records.len());
    for rec in &records {
        route_by_id.entry(rec.route_id.as_str()).or_insert(rec);
    }

    // RTS_019: Yinelenen hat adı — bir hat başka biriyle HEM route_short_name HEM route_long_name
    // bakımından aynıysa (gerçek kopya hat). Yalnız kısa VEYA yalnız uzun adı paylaşmak NORMALDİR
    // (aynı hat numarası varyant/yön için tekrar kullanılır; farklı hatlar aynı uzun adı taşıyabilir)
    // → MobilityData `duplicate_route_name` ile hizalı; (kısa|uzun) tek-başına eşleşme FP üretiyordu.
    // Çakışan (kısa,uzun) çift başına TEK notice; adı paylaşan tüm hatları (route_id) listeler.
    {
        // Anahtar (kısa, uzun, TÜR, AJANS): "ayırt edilemez kopya" iddiası ancak aynı
        // taşıma türü VE aynı işletmeci içinde kurulabilir. İki ayrı ajansın "10" numaralı
        // hattı olması olağandır (yolcu ajansa göre ayırt eder); bir otobüs "10" ile bir
        // tramvay "10" da moda göre ayrılır. GTFS/MobilityData da bunu böyle tanımlar:
        // "All routes of the same route_type with the same agency_id should have unique
        // combinations of route_short_name and route_long_name."
        //
        // Tür/ajans anahtarda YOKKEN yanlış pozitif üretiyordu — 250-feed corpus, mdb-1091
        // (Lüksemburg): 32 gruptan 29'u farklı route_type/agency_id taşıyordu (ör. "10" →
        // LU::Authority:16:: ve LU::Authority:26::). Anahtar tamamlanınca 32 → 3 = MD ile birebir.
        type NameKey = (String, String, Option<u32>, String);
        let mut name_groups: HashMap<NameKey, Vec<(String, u64)>> = HashMap::new();
        for rec in &records {
            let sn = rec.route_short_name.as_deref().unwrap_or("").trim().to_lowercase();
            let ln = rec.route_long_name.as_deref().unwrap_or("").trim().to_lowercase();
            if sn.is_empty() && ln.is_empty() { continue; }
            let agency = rec.agency_id.as_deref().unwrap_or("").trim().to_string();
            name_groups
                .entry((sn, ln, rec.route_type, agency))
                .or_default()
                .push((rec.route_id.clone(), rec.line));
        }
        // Deterministik çıktı için anahtara göre sırala.
        let mut groups: Vec<_> = name_groups.into_iter().filter(|(_, e)| e.len() >= 2).collect();
        groups.sort_by(|a, b| a.0.cmp(&b.0));
        for (_key, entries) in &groups {
            let group_str = entries.iter().map(|(id, _)| id.as_str()).collect::<Vec<_>>().join(", ");
            let (anchor_id, anchor_line) = &entries[0];
            let anchor = route_by_id.get(anchor_id.as_str()).copied();
            let sn_disp = anchor.and_then(|r| r.route_short_name.as_deref()).unwrap_or("");
            let ln_disp = anchor.and_then(|r| r.route_long_name.as_deref()).unwrap_or("");
            let display = match (sn_disp.is_empty(), ln_disp.is_empty()) {
                (false, false) => format!("{sn_disp} — {ln_disp}"),
                (false, true)  => sn_disp.to_string(),
                _              => ln_disp.to_string(),
            };
            let mut n = make_k2_notice(
                &mut counter, "RTS_019", EntityType::Route,
                Some(anchor_id.clone()), None,
                "routes.txt", Some(*anchor_line), Some("route_long_name"),
                Some(display.clone()), None,
                format!("'{display}' hat adı (kısa+uzun aynı) şu hatlar tarafından paylaşılıyor: {group_str}."),
                "Aynı kısa+uzun adı taşıyan hatları birleştirin ya da adlarını benzersizleştirin; bilerek kopya ise görmezden gelin.",
            );
            n.details = Some([("conflicting_routes".to_string(), group_str.clone())].into_iter().collect());
            notices.push(n);
        }
    }

    // RTS_026: Yinelenen kısa hat adı — ≥2 hat aynı route_short_name'i paylaşıyor AMA aralarında
    // ≥2 farklı route_long_name var (aynı numara, farklı adlar). İkisi de aynı olsaydı RTS_019
    // (ORTA) olurdu → o durum burada dışlanır. Sıkça bilinçlidir (varyant/yön) → BİLGİ. Kısa ad
    // başına TEK notice; paylaşan tüm hatları listeler.
    {
        let mut short_groups: HashMap<String, Vec<(String, String, u64)>> = HashMap::new();
        for rec in &records {
            let sn = rec.route_short_name.as_deref().unwrap_or("").trim().to_lowercase();
            if sn.is_empty() { continue; }
            let ln = rec.route_long_name.as_deref().unwrap_or("").trim().to_lowercase();
            short_groups.entry(sn).or_default().push((rec.route_id.clone(), ln, rec.line));
        }
        let mut groups: Vec<_> = short_groups.into_iter()
            .filter(|(_, e)| {
                e.len() >= 2
                    && e.iter().map(|(_, ln, _)| ln.as_str())
                        .collect::<std::collections::HashSet<_>>().len() >= 2
            })
            .collect();
        groups.sort_by(|a, b| a.0.cmp(&b.0));
        for (_sn, entries) in &groups {
            let group_str = entries.iter().map(|(id, _, _)| id.as_str()).collect::<Vec<_>>().join(", ");
            let (anchor_id, _, anchor_line) = &entries[0];
            let sn_disp = route_by_id.get(anchor_id.as_str()).copied()
                .and_then(|r| r.route_short_name.as_deref()).unwrap_or("");
            let mut n = make_k2_notice(
                &mut counter, "RTS_026", EntityType::Route,
                Some(anchor_id.clone()), None,
                "routes.txt", Some(*anchor_line), Some("route_short_name"),
                Some(sn_disp.to_string()), None,
                format!("'{sn_disp}' kısa hat adı, farklı uzun adlı şu hatlar tarafından paylaşılıyor: {group_str}."),
                "Aynı hat numarası varyant/yön için bilinçli kullanılıyorsa yok sayın; değilse benzersizleştirin.",
            );
            n.details = Some([("conflicting_routes".to_string(), group_str.clone())].into_iter().collect());
            notices.push(n);
        }
    }

    // RTS_027: Yinelenen uzun hat adı — ≥2 hat aynı route_long_name'i paylaşıyor AMA aralarında
    // ≥2 farklı route_short_name var (farklı numaralar, aynı açıklama). İkisi de aynı olsaydı
    // RTS_019 (ORTA) olurdu → dışlanır. BİLGİ. Uzun ad başına TEK notice; paylaşan hatları listeler.
    {
        let mut long_groups: HashMap<String, Vec<(String, String, u64)>> = HashMap::new();
        for rec in &records {
            let ln = rec.route_long_name.as_deref().unwrap_or("").trim().to_lowercase();
            if ln.is_empty() { continue; }
            let sn = rec.route_short_name.as_deref().unwrap_or("").trim().to_lowercase();
            long_groups.entry(ln).or_default().push((rec.route_id.clone(), sn, rec.line));
        }
        let mut groups: Vec<_> = long_groups.into_iter()
            .filter(|(_, e)| {
                e.len() >= 2
                    && e.iter().map(|(_, sn, _)| sn.as_str())
                        .collect::<std::collections::HashSet<_>>().len() >= 2
            })
            .collect();
        groups.sort_by(|a, b| a.0.cmp(&b.0));
        for (_ln, entries) in &groups {
            let group_str = entries.iter().map(|(id, _, _)| id.as_str()).collect::<Vec<_>>().join(", ");
            let (anchor_id, _, anchor_line) = &entries[0];
            let ln_disp = route_by_id.get(anchor_id.as_str()).copied()
                .and_then(|r| r.route_long_name.as_deref()).unwrap_or("");
            let mut n = make_k2_notice(
                &mut counter, "RTS_027", EntityType::Route,
                Some(anchor_id.clone()), None,
                "routes.txt", Some(*anchor_line), Some("route_long_name"),
                Some(ln_disp.to_string()), None,
                format!("'{ln_disp}' uzun hat adı, farklı kısa adlı şu hatlar tarafından paylaşılıyor: {group_str}."),
                "Aynı uzun ad bilinçli paylaşılıyorsa yok sayın; değilse her hatta ayırt edici bir ad verin.",
            );
            n.details = Some([("conflicting_routes".to_string(), group_str.clone())].into_iter().collect());
            notices.push(n);
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

    fn ids(file: &RawFile) -> Vec<String> {
        validate_routes(file).1.iter().map(|n| n.rule_id.clone()).collect()
    }

    #[test]
    fn length_rules_count_characters_not_bytes() {
        // "Kızılay" 7 karakter / 8 bayt; bayt sayımıyla 6'lık Google eşiği yanlış aşılırdı.
        // route_long_name BOŞ bırakıldı ki RTS_021 muafiyeti devreye girmesin.
        let f = make_file(
            vec!["route_id", "route_short_name", "route_long_name", "route_type"],
            vec![vec!["R1", "Kızıla", "", "3"]],   // 6 karakter / 7 bayt → eşik altı
        );
        assert!(!ids(&f).contains(&"RTS_021".to_string()), "6 karakter eşiği aşmamalı: {:?}", ids(&f));

        // 12 karakterlik Japonca ad = 36 bayt; bayt sayımıyla RTS_010 yanlış tetiklenirdi.
        let f = make_file(
            vec!["route_id", "route_short_name", "route_long_name", "route_type"],
            vec![vec!["R1", "東京都交通局都営バス系統", "", "3"]],
        );
        let got = ids(&f);
        assert!(!got.contains(&"RTS_010".to_string()), "12 karakter >12 değildir: {got:?}");
        assert!(got.contains(&"RTS_021".to_string()), "12 karakter Google eşiğini aşar: {got:?}");
    }

    #[test]
    fn rts_021_is_waived_when_long_name_is_present() {
        // Google'ın muafiyeti: route_long_name doluysa kısa ad uyarısı göz ardı edilebilir.
        let with_long = make_file(
            vec!["route_id", "route_short_name", "route_long_name", "route_type"],
            vec![vec!["R1", "Green Line", "Green Line Downtown", "3"]],
        );
        assert!(!ids(&with_long).contains(&"RTS_021".to_string()),
            "long_name doluyken RTS_021 üretilmemeli: {:?}", ids(&with_long));

        // Boşluk-dolu long_name muafiyet saymaz.
        let blank_long = make_file(
            vec!["route_id", "route_short_name", "route_long_name", "route_type"],
            vec![vec!["R1", "Green Line", "   ", "3"]],
        );
        assert!(ids(&blank_long).contains(&"RTS_021".to_string()),
            "boş long_name muafiyet saymamalı: {:?}", ids(&blank_long));
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

    /// Aynı ad FARKLI AJANSTA kopya değildir — iki işletmecinin "10" hattı olması olağan.
    /// Corpus kanıtı (mdb-1091, Lüksemburg): 32 gruptan 29'u farklı ajans/tür; anahtar
    /// tamamlanınca 32 → 3 = MobilityData ile birebir.
    /// route_desc == route_short_name de RTS_023'tür (MD `same_name_and_description_for_route`
    /// her iki adı da denetler). Yalnız long_name'e bakmak kapsam boşluğuydu: mdb-2898'de
    /// MD 436 bulgu verirken biz 0 veriyorduk, 436'sı da short_name eşleşmesiydi.
    #[test]
    fn route_desc_equal_to_short_name_produces_rts_023() {
        let file = make_file(
            vec!["route_id", "route_short_name", "route_long_name", "route_desc", "route_type"],
            vec![vec!["R1", "EXT", "Merkez - Sahil", "EXT", "3"]],
        );
        let (_, notices) = validate_routes(&file);
        assert_eq!(notices.iter().filter(|n| n.rule_id == "RTS_023").count(), 1,
            "route_desc == route_short_name → RTS_023 olmalı");
    }

    /// Her iki ad da eşleşse bile hat başına TEK notice (aynı kök neden).
    #[test]
    fn route_desc_equal_to_both_names_produces_single_rts_023() {
        let file = make_file(
            vec!["route_id", "route_short_name", "route_long_name", "route_desc", "route_type"],
            vec![vec!["R1", "X", "X", "X", "3"]],
        );
        let (_, notices) = validate_routes(&file);
        assert_eq!(notices.iter().filter(|n| n.rule_id == "RTS_023").count(), 1,
            "iki alan da eşleşse bile tek notice");
    }

    /// route_desc farklıysa tetiklememeli (fix aşırı-tetiklememeli).
    #[test]
    fn route_desc_different_from_names_is_not_rts_023() {
        let file = make_file(
            vec!["route_id", "route_short_name", "route_long_name", "route_desc", "route_type"],
            vec![vec!["R1", "10", "Merkez - Sahil", "Sahil boyunca ekspres", "3"]],
        );
        let (_, notices) = validate_routes(&file);
        assert!(!notices.iter().any(|n| n.rule_id == "RTS_023"),
            "gerçekten açıklayıcı route_desc tetiklememeli");
    }

    #[test]
    fn same_name_different_agency_is_not_rts_019() {
        let file = make_file(
            vec!["route_id", "agency_id", "route_short_name", "route_long_name", "route_type"],
            vec![
                vec!["R1", "A1", "10", "", "3"],
                vec!["R2", "A2", "10", "", "3"], // farklı ajans → kopya DEĞİL
            ],
        );
        let (_, notices) = validate_routes(&file);
        assert!(
            !notices.iter().any(|n| n.rule_id == "RTS_019"),
            "farklı ajansın aynı hat numarası kopya sayılmamalı"
        );
    }

    /// Aynı ad FARKLI TÜRDE de kopya değildir — otobüs "10" ile tramvay "10" moda göre ayrılır.
    #[test]
    fn same_name_different_route_type_is_not_rts_019() {
        let file = make_file(
            vec!["route_id", "agency_id", "route_short_name", "route_long_name", "route_type"],
            vec![
                vec!["R1", "A1", "10", "Merkez", "3"], // otobüs
                vec!["R2", "A1", "10", "Merkez", "0"], // tramvay
            ],
        );
        let (_, notices) = validate_routes(&file);
        assert!(
            !notices.iter().any(|n| n.rule_id == "RTS_019"),
            "farklı taşıma türünün aynı adı kopya sayılmamalı"
        );
    }

    /// AYNI ajans + AYNI tür + aynı ad → gerçek kopya, tetiklemeli (fix bunu bozmamalı).
    #[test]
    fn same_name_same_agency_and_type_is_rts_019() {
        let file = make_file(
            vec!["route_id", "agency_id", "route_short_name", "route_long_name", "route_type"],
            vec![
                vec!["R1", "A1", "10", "Merkez", "3"],
                vec!["R2", "A1", "10", "Merkez", "3"],
            ],
        );
        let (_, notices) = validate_routes(&file);
        assert_eq!(
            notices.iter().filter(|n| n.rule_id == "RTS_019").count(), 1,
            "aynı ajans+tür+ad gerçek kopya → RTS_019 üretmeli"
        );
    }

    #[test]
    fn shared_route_name_produces_single_rts_019_per_group() {
        // 3 hat HEM aynı short HEM aynı long → gerçek kopya → çakışan grup başına TEK RTS_019
        let file = make_file(
            vec!["route_id", "route_short_name", "route_long_name", "route_type"],
            vec![
                vec!["R1", "QM2", "Astoria - Midtown", "3"],
                vec!["R2", "QM2", "Astoria - Midtown", "3"],
                vec!["R3", "QM2", "Astoria - Midtown", "3"],
            ],
        );
        let (_, notices) = validate_routes(&file);
        let rts019: Vec<_> = notices.iter().filter(|n| n.rule_id == "RTS_019").collect();
        assert_eq!(rts019.len(), 1, "Grup başına tek RTS_019 bekleniyor, {} üretildi: {:?}",
            rts019.len(), rts019.iter().map(|n| &n.message).collect::<Vec<_>>());
        // Mesaj grubun TÜM üyelerini (route_id) listelemeli
        let msg = &rts019[0].message;
        assert!(msg.contains("R1") && msg.contains("R2") && msg.contains("R3"),
            "Mesaj tüm grubu listelemeli: {}", msg);
        // Hepsi HEM kısa HEM uzun aynı → yalnız RTS_019; RTS_026/027 (tek-alan) tetiklenmemeli.
        assert!(!notices.iter().any(|n| n.rule_id == "RTS_026" || n.rule_id == "RTS_027"),
            "both-same grup RTS_026/027 üretmemeli (RTS_019 dışında)");
    }

    #[test]
    fn shared_short_name_different_long_produces_rts_026_not_rts_019() {
        // Aynı route_short_name ama FARKLI route_long_name (varyant/yön) → gerçek kopya DEĞİL →
        // RTS_019 (ORTA) YOK; ama aynı numara farklı adlar → RTS_026 (Bilgi). Athens mdb-3220
        // FP'sinin köku buydu (109 grup); artık ORTA değil BİLGİ.
        let file = make_file(
            vec!["route_id", "route_short_name", "route_long_name", "route_type"],
            vec![
                vec!["R1", "221", "Merkez - Kuzey", "3"],
                vec!["R2", "221", "Merkez - Güney", "3"],
            ],
        );
        let (_, notices) = validate_routes(&file);
        assert!(!notices.iter().any(|n| n.rule_id == "RTS_019"),
            "aynı short + farklı long RTS_019 üretmemeli");
        let rts026: Vec<_> = notices.iter().filter(|n| n.rule_id == "RTS_026").collect();
        assert_eq!(rts026.len(), 1, "aynı short farklı long → tek RTS_026 beklenir");
        assert!(rts026[0].message.contains("R1") && rts026[0].message.contains("R2"),
            "RTS_026 mesajı tüm grubu listelemeli: {}", rts026[0].message);
        assert!(!notices.iter().any(|n| n.rule_id == "RTS_027"),
            "farklı long → RTS_027 üretmemeli");
    }

    #[test]
    fn shared_long_name_different_short_produces_rts_027() {
        // Aynı route_long_name ama FARKLI route_short_name → RTS_027 (Bilgi); RTS_019/026 YOK.
        let file = make_file(
            vec!["route_id", "route_short_name", "route_long_name", "route_type"],
            vec![
                vec!["R1", "10", "Şehir Merkezi Hattı", "3"],
                vec!["R2", "20", "Şehir Merkezi Hattı", "3"],
            ],
        );
        let (_, notices) = validate_routes(&file);
        let rts027: Vec<_> = notices.iter().filter(|n| n.rule_id == "RTS_027").collect();
        assert_eq!(rts027.len(), 1, "aynı long farklı short → tek RTS_027 beklenir");
        assert!(rts027[0].message.contains("R1") && rts027[0].message.contains("R2"),
            "RTS_027 mesajı tüm grubu listelemeli: {}", rts027[0].message);
        assert!(!notices.iter().any(|n| n.rule_id == "RTS_019" || n.rule_id == "RTS_026"),
            "aynı long farklı short durumda RTS_019/026 üretmemeli");
    }

    #[test]
    fn shared_short_name_produces_single_rts_019() {
        let file = make_file(
            vec!["route_id", "route_short_name", "route_type"],
            vec![vec!["R1", "10", "3"], vec!["R2", "10", "3"]],
        );
        let (_, notices) = validate_routes(&file);
        assert_eq!(notices.iter().filter(|n| n.rule_id == "RTS_019").count(), 1,
            "Aynı short_name → tek RTS_019");
    }

    #[test]
    fn many_duplicate_route_groups_do_not_rescan_all_records() {
        // 20 bin ayrı yinelenen grup, toplam 40 bin route. Anchor kaydı her grup için
        // `records.iter().find` ile aranırsa ~800 milyon karşılaştırmaya çıkar; indeksli
        // uygulama route sayısıyla doğrusal büyür. Geniş süre payı yavaş CI makinelerini
        // tolere ederken karesel regresyonu yakalar.
        const GROUPS: usize = 20_000;
        let mut rows = Vec::with_capacity(GROUPS * 2);
        for group in 0..GROUPS {
            let name = format!("N{group}");
            for member in 0..2 {
                rows.push(vec![
                    smol_str::SmolStr::from(format!("R{group}_{member}")),
                    smol_str::SmolStr::from(name.as_str()),
                    smol_str::SmolStr::from(name.as_str()),
                    smol_str::SmolStr::from("2"),
                ]);
            }
        }
        let file = RawFile {
            name: "routes.txt".to_string(),
            headers: [
                "route_id",
                "route_short_name",
                "route_long_name",
                "route_type",
            ]
            .into_iter()
            .map(str::to_string)
            .collect(),
            rows,
            bytes: 0,
            raw_text: None,
        };

        let started = std::time::Instant::now();
        let (_, notices) = validate_routes(&file);
        let elapsed = started.elapsed();

        assert_eq!(
            notices.iter().filter(|n| n.rule_id == "RTS_019").count(),
            GROUPS,
        );
        assert!(
            elapsed < std::time::Duration::from_secs(10),
            "40 bin route doğrulaması doğrusal kalmalı; geçen süre: {elapsed:?}",
        );
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

    #[test]
    fn invalid_route_cemv_support_produces_rts_024() {
        let file = make_file(
            vec!["route_id", "route_short_name", "route_type", "cemv_support"],
            vec![vec!["R1", "Metro", "1", "3"]],
        );
        let (_, notices) = validate_routes(&file);
        assert!(notices.iter().any(|n| n.rule_id == "RTS_024"));
    }

    #[test]
    fn valid_route_cemv_support_produces_no_rts_024() {
        let file = make_file(
            vec!["route_id", "route_short_name", "route_type", "cemv_support"],
            vec![vec!["R1", "Metro", "1", "2"]],
        );
        let (_, notices) = validate_routes(&file);
        assert!(!notices.iter().any(|n| n.rule_id == "RTS_024"));
    }
}
