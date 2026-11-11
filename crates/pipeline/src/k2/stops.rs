use gtfs_core::EntityType;
use std::collections::HashMap;

use super::common::{
    build_row_map, get_lexical_field, get_raw_field, get_trimmed_field, looks_like_iana_timezone,
    looks_like_url, make_k2_notice, parse_f64, parse_u32, validate_enum, RowMap,
};
use crate::k1_parse::RawFile;

#[derive(Debug, Clone, Default)]
pub struct StopRecord {
    pub stop_id: String,
    pub stop_code: Option<String>,
    pub stop_name: Option<String>,
    pub stop_lat: Option<f64>,
    pub stop_lon: Option<f64>,
    pub location_type: Option<u32>,
    pub stop_timezone: Option<String>,
    pub wheelchair_boarding: Option<u32>,
    pub stop_access: Option<u32>,
    pub level_id: Option<String>,
    pub tts_stop_name: Option<String>,
    pub stop_url: Option<String>,
    pub row: RowMap,
    pub line: u64,
}

pub fn validate_stops(file: &RawFile) -> (Vec<StopRecord>, Vec<gtfs_core::Notice>) {
    let mut notices = Vec::new();
    let mut records = Vec::new();
    let mut counter = 0u32;

    // STP_022 aggregation: if stop_code is unused across the feed (single-producer
    // decision), emit one summary instead of tens of thousands of identical per-stop notices
    // (STM_017 precedent). If partially populated, preserve per-stop notices. Decide at loop end.
    let mut stp022_pending: Vec<gtfs_core::Notice> = Vec::new();
    let mut any_stop_code_present = false;
    let mut stop_code_owner: HashMap<String, String> = HashMap::new();

    // STP_033 aggregation (#48): missing zone_id is nearly universal in large feeds
    // (DELFI: 91% of 548K stops) — emit one feed summary above the threshold instead of
    // tens of thousands of per-stop Info notices. STP_022 precedent; decide at loop end.
    let mut stp033_pending: Vec<gtfs_core::Notice> = Vec::new();
    // STP_045: platform_code populated, parent_station empty — one summary per feed.
    let mut stp045_count: usize = 0;
    let mut stp045_examples: Vec<String> = Vec::new();

    for (row_idx, row) in file.rows.iter().enumerate() {
        let line = (row_idx + 2) as u64;
        let row_map = build_row_map(&file.headers, row);

        let stop_id = get_raw_field(&row_map, "stop_id").unwrap_or("").to_string();
        let entity_id = (!stop_id.is_empty()).then_some(stop_id.clone());

        // STP_002: stop_id required (if the column is absent, ARC_025 takes over → skip)
        if get_raw_field(&row_map, "stop_id").map(str::trim) == Some("") {
            let mut notice = make_k2_notice(
                &mut counter,
                "STP_002",
                EntityType::Stop,
                None,
                Some(&row_map),
                &file.name,
                Some(line),
                Some("stop_id"),
                Some(String::new()),
                None,
                "stop_id zorunludur.".to_string(),
                "Fill in the stop_id field.",
            );
            // Planner context: the code-less row's name, code and position.
            let details = notice.details.get_or_insert_with(Default::default);
            for field in ["stop_name", "stop_code", "stop_lat", "stop_lon"] {
                if let Some(value) = get_trimmed_field(&row_map, field).filter(|v| !v.is_empty()) {
                    details.insert(field.to_string(), value.to_string());
                }
            }
            notices.push(notice);
        }

        let stop_code = get_trimmed_field(&row_map, "stop_code")
            .filter(|v| !v.trim().is_empty())
            .map(str::to_string);
        if stop_code.is_some() {
            any_stop_code_present = true;
        }
        if let Some(code) = &stop_code {
            if let Some(first_stop) = stop_code_owner.get(code) {
                let mut n039 = make_k2_notice(&mut counter, "STP_039", EntityType::Stop, entity_id.clone(),
                    Some(&row_map), &file.name, Some(line), Some("stop_code"), Some(code.clone()),
                    Some("unique within the feed".to_string()),
                    format!("stop_code '{code}' hem '{first_stop}' hem '{stop_id}' duraklarında kullanılıyor."),
                    "Her yolcu-facing stop_code değerini feed içinde benzersiz yapın.");
                // Planner context: the stop that already carries the code.
                n039.details
                    .get_or_insert_with(Default::default)
                    .insert("other_stop_id".to_string(), first_stop.clone());
                notices.push(n039);
            } else {
                stop_code_owner.insert(code.clone(), stop_id.clone());
            }
        }

        // STP_028: stop_code is too long (>50 characters)
        if let Some(ref code) = stop_code {
            if code.len() > 50 {
                let mut n028 = make_k2_notice(
                    &mut counter,
                    "STP_028",
                    EntityType::Stop,
                    entity_id.clone(),
                    Some(&row_map),
                    &file.name,
                    Some(line),
                    Some("stop_code"),
                    Some(code.len().to_string()),
                    Some("≤50".to_string()),
                    format!(
                        "'{}' durağının stop_code değeri {} karakter; 50 karakteri aşıyor.",
                        stop_id,
                        code.len()
                    ),
                    "stop_code'u 50 karakterin altında tutun.",
                );
                // Planner context: the code itself.
                n028.details
                    .get_or_insert_with(Default::default)
                    .insert("stop_code".to_string(), code.clone());
                notices.push(n028);
            }
        }

        // location_type: optional, 0-4
        let location_type = match parse_u32(&row_map, "location_type") {
            Ok(v) => {
                if let Some(val) = v {
                    if !validate_enum(&val.to_string(), &["0", "1", "2", "3", "4"]) {
                        notices.push(make_k2_notice(
                            &mut counter,
                            "STP_008",
                            EntityType::Stop,
                            entity_id.clone(),
                            Some(&row_map),
                            &file.name,
                            Some(line),
                            Some("location_type"),
                            Some(val.to_string()),
                            Some("0-4".to_string()),
                            format!("location_type {val} geçerli bir değer değil (0-4 olmalı)."),
                            "location_type alanını 0, 1, 2, 3 veya 4 olarak ayarlayın.",
                        ));
                    }
                }
                v
            }
            Err(err) => {
                // A NON-NUMERIC value used to be silently dropped: an out-of-range number produced
                // STP_008, while "abc" produced no finding. Two branches of the same fact → one rule
                // (the PTH_027 precedent).
                notices.push(make_k2_notice(
                    &mut counter,
                    "STP_008",
                    EntityType::Stop,
                    entity_id.clone(),
                    Some(&row_map),
                    &file.name,
                    Some(line),
                    Some("location_type"),
                    get_trimmed_field(&row_map, "location_type").map(str::to_string),
                    Some("0-4".to_string()),
                    err,
                    "location_type alanını 0, 1, 2, 3 veya 4 olarak ayarlayın.",
                ));
                None
            }
        };

        // For location_type 0 (or empty — regular stop) and 1 (station), stop_code is recommended.
        let is_stop_or_station = matches!(location_type, None | Some(0) | Some(1));
        // stop_name/stop_lat/stop_lon: required by the spec for stop (0), station (1), AND
        // entrance/exit (2); optional for generic node (3) and boarding area (4).
        let requires_name_and_coords = matches!(location_type, None | Some(0) | Some(1) | Some(2));

        // STP_022: stop_code missing (for stops and stations) — held until the end of the loop
        // for the aggregation decision.
        if stop_code.is_none() && is_stop_or_station {
            let mut notice = make_k2_notice(
                &mut counter,
                "STP_022",
                EntityType::Stop,
                entity_id.clone(),
                Some(&row_map),
                &file.name,
                Some(line),
                Some("stop_code"),
                Some(String::new()),
                None,
                format!("'{}' durağında stop_code eksik.", stop_id),
                "Yolcuların tanıyabileceği kısa bir stop_code değeri girin.",
            );
            // The rule is emitted at two scales (per stop / feed summary) but has one message
            // template. Populate count+example even on the individual branch so en/ja/fr text
            // is populated on both paths (the `affected_records` precedent in k4).
            let details = notice.details.get_or_insert_with(Default::default);
            details.insert("affected_stops".to_string(), "1".to_string());
            details.insert("example_stops".to_string(), stop_id.to_string());
            stp022_pending.push(notice);
        }

        // stop_name: required for stops/stations/entrances (location_type 0, 1, 2 or empty)
        let stop_name = get_trimmed_field(&row_map, "stop_name")
            .filter(|v| !v.trim().is_empty())
            .map(str::to_string);
        if stop_name.is_none() && requires_name_and_coords {
            notices.push(make_k2_notice(
                &mut counter,
                "STP_003",
                EntityType::Stop,
                entity_id.clone(),
                Some(&row_map),
                &file.name,
                Some(line),
                Some("stop_name"),
                Some(String::new()),
                None,
                format!(
                    "'{}' durağı için durak adı (stop_name) zorunludur.",
                    stop_id
                ),
                "Okunabilir bir stop_name değeri girin.",
            ));
        }

        // STP_019: stop_name is too long (>100 characters)
        if let Some(ref name) = stop_name {
            if name.len() > 100 {
                notices.push(make_k2_notice(
                    &mut counter,
                    "STP_019",
                    EntityType::Stop,
                    entity_id.clone(),
                    Some(&row_map),
                    &file.name,
                    Some(line),
                    Some("stop_name"),
                    Some(name.len().to_string()),
                    Some("≤100".to_string()),
                    format!(
                        "'{}' durağının stop_name değeri {} karakter; 100 karakteri aşıyor.",
                        stop_id,
                        name.len()
                    ),
                    "stop_name'i 100 karakterin altında tutun.",
                ));
            }
        }

        // STP_031: stop_name == stop_desc (same_name_and_description_for_stop)
        let stop_desc = get_trimmed_field(&row_map, "stop_desc")
            .filter(|v| !v.trim().is_empty())
            .map(str::to_string);
        if let (Some(ref name), Some(ref desc)) = (&stop_name, &stop_desc) {
            // 🔴 NOT `eq_ignore_ascii_case`: it equates only the a-z/A-Z range and is blind to
            // diacritic alphabets because we compare two USER TEXT values.
            // Measured in corpus run 14 — `mdb-1000` (Poland): normalization finds 6 stops,
            // MD also reports 6, while we found 4. The two missed pairs were exactly non-ASCII:
            // `Stanisławczyk`/`STANISŁAWCZYK` (ł↔Ł) and `Witoszyńce`/`WITOSZYŃCE` (ń↔Ń).
            // The same blindness affected Turkish (ş↔Ş, ğ↔Ğ), Czech, Greek, and Cyrillic feeds.
            // `tdg-83513` 158↔160 and `mdb-784` 73,944↔73,946 showed the same fixed gap —
            // in all three feeds, the missing items were exactly the non-ASCII pairs.
            // Allocation-free: lazily lowercase both sides and compare characters.
            if name
                .chars()
                .flat_map(char::to_lowercase)
                .eq(desc.chars().flat_map(char::to_lowercase))
            {
                notices.push(make_k2_notice(
                    &mut counter,
                    "STP_031",
                    EntityType::Stop,
                    entity_id.clone(),
                    Some(&row_map),
                    &file.name,
                    Some(line),
                    Some("stop_desc"),
                    Some(desc.clone()),
                    None,
                    format!(
                        "'{}' durağının stop_name ve stop_desc değerleri aynı: '{}'.",
                        stop_id, name
                    ),
                    "stop_desc, stop_name'den farklı ve daha açıklayıcı bir değer içermelidir.",
                ));
            }
        }

        // STP_004: stop_lat required for stops/stations/entrances, must be in [-90, 90]
        let stop_lat = match parse_f64(&row_map, "stop_lat") {
            Ok(v) => match v {
                None => {
                    if requires_name_and_coords {
                        notices.push(make_k2_notice(
                            &mut counter,
                            "STP_006",
                            EntityType::Stop,
                            entity_id.clone(),
                            Some(&row_map),
                            &file.name,
                            Some(line),
                            Some("stop_lat"),
                            Some(String::new()),
                            Some("[-90, 90]".to_string()),
                            format!("'{}' durağı için enlem (stop_lat) zorunludur.", stop_id),
                            "stop_lat alanını ondalıklı enlem değeriyle doldurun.",
                        ));
                    }
                    None
                }
                Some(lat) => {
                    if !(-90.0..=90.0).contains(&lat) {
                        notices.push(make_k2_notice(
                            &mut counter,
                            "STP_003",
                            EntityType::Stop,
                            entity_id.clone(),
                            Some(&row_map),
                            &file.name,
                            Some(line),
                            Some("stop_lat"),
                            Some(lat.to_string()),
                            Some("[-90, 90]".to_string()),
                            format!("stop_lat {lat} değeri [-90, 90] aralığı dışında."),
                            "stop_lat için -90 ile 90 arasında bir değer girin.",
                        ));
                    }
                    Some(lat)
                }
            },
            Err(err) => {
                notices.push(make_k2_notice(
                    &mut counter,
                    "STP_004",
                    EntityType::Stop,
                    entity_id.clone(),
                    Some(&row_map),
                    &file.name,
                    Some(line),
                    Some("stop_lat"),
                    get_trimmed_field(&row_map, "stop_lat").map(str::to_string),
                    Some("[-90, 90]".to_string()),
                    err,
                    "stop_lat alanını ondalıklı enlem değeriyle doldurun.",
                ));
                None
            }
        };

        // STP_007: stop_lon required for stops/stations/entrances, must be in [-180, 180]
        let stop_lon = match parse_f64(&row_map, "stop_lon") {
            Ok(v) => match v {
                None => {
                    if requires_name_and_coords {
                        notices.push(make_k2_notice(
                            &mut counter,
                            "STP_007",
                            EntityType::Stop,
                            entity_id.clone(),
                            Some(&row_map),
                            &file.name,
                            Some(line),
                            Some("stop_lon"),
                            Some(String::new()),
                            Some("[-180, 180]".to_string()),
                            format!("'{}' durağı için boylam (stop_lon) zorunludur.", stop_id),
                            "stop_lon alanını ondalıklı boylam değeriyle doldurun.",
                        ));
                    }
                    None
                }
                Some(lon) => {
                    if !(-180.0..=180.0).contains(&lon) {
                        notices.push(make_k2_notice(
                            &mut counter,
                            "STP_005",
                            EntityType::Stop,
                            entity_id.clone(),
                            Some(&row_map),
                            &file.name,
                            Some(line),
                            Some("stop_lon"),
                            Some(lon.to_string()),
                            Some("[-180, 180]".to_string()),
                            format!("stop_lon {lon} değeri [-180, 180] aralığı dışında."),
                            "stop_lon için -180 ile 180 arasında bir değer girin.",
                        ));
                    }
                    Some(lon)
                }
            },
            Err(err) => {
                notices.push(make_k2_notice(
                    &mut counter,
                    "STP_005",
                    EntityType::Stop,
                    entity_id.clone(),
                    Some(&row_map),
                    &file.name,
                    Some(line),
                    Some("stop_lon"),
                    get_trimmed_field(&row_map, "stop_lon").map(str::to_string),
                    Some("[-180, 180]".to_string()),
                    err,
                    "stop_lon alanını ondalıklı boylam değeriyle doldurun.",
                ));
                None
            }
        };

        // stop_timezone: must be valid IANA if provided
        let stop_timezone = get_trimmed_field(&row_map, "stop_timezone")
            .filter(|v| !v.trim().is_empty())
            .map(str::to_string);
        if let Some(ref tz) = stop_timezone {
            if !looks_like_iana_timezone(tz) {
                notices.push(make_k2_notice(
                    &mut counter,
                    "STP_014",
                    EntityType::Stop,
                    entity_id.clone(),
                    Some(&row_map),
                    &file.name,
                    Some(line),
                    Some("stop_timezone"),
                    Some(tz.clone()),
                    None,
                    format!("stop_timezone '{tz}' geçerli bir IANA saat dilimi değil."),
                    "Geçerli bir IANA saat dilimi girin (örn. Europe/Istanbul).",
                ));
            }
        }

        // wheelchair_boarding: must be 0, 1, or 2 if provided
        let wheelchair_boarding = match parse_u32(&row_map, "wheelchair_boarding") {
            Ok(v) => {
                if let Some(val) = v {
                    if !validate_enum(&val.to_string(), &["0", "1", "2"]) {
                        notices.push(make_k2_notice(
                            &mut counter, "STP_013", EntityType::Stop, entity_id.clone(),
                            Some(&row_map), &file.name, Some(line), Some("wheelchair_boarding"),
                            Some(val.to_string()), Some("0, 1 or 2".to_string()),
                            "wheelchair_boarding 0, 1 veya 2 olmalıdır.".to_string(),
                            "wheelchair_boarding alanını 0 (bilgi yok), 1 (erişilebilir) veya 2 (erişilemez) olarak ayarlayın.",
                        ));
                    }
                }
                v
            }
            Err(err) => {
                // A NON-NUMERIC value used to be silently dropped: an out-of-range number produced
                // STP_013, while "abc" produced no finding. Two branches of the same fact → one rule
                // (the PTH_027 precedent).
                notices.push(make_k2_notice(
                    &mut counter, "STP_013", EntityType::Stop, entity_id.clone(),
                    Some(&row_map), &file.name, Some(line), Some("wheelchair_boarding"),
                    get_trimmed_field(&row_map, "wheelchair_boarding").map(str::to_string), Some("0, 1 or 2".to_string()),
                    err,
                    "wheelchair_boarding alanını 0 (bilgi yok), 1 (erişilebilir) veya 2 (erişilemez) olarak ayarlayın.",
                ));
                None
            }
        };

        // K2 compatibility domain: 0, 1, or 2. Normative GTFS validity is checked by
        // STP_026 in K4, where only 0 and 1 are accepted.
        let stop_access =
            match parse_u32(&row_map, "stop_access") {
                Ok(v) => {
                    if let Some(val) = v {
                        if !validate_enum(&val.to_string(), &["0", "1", "2"]) {
                            notices.push(make_k2_notice(
                            &mut counter, "STP_024", EntityType::Stop, entity_id.clone(),
                            Some(&row_map), &file.name, Some(line), Some("stop_access"),
                            Some(val.to_string()), Some("0–2".to_string()),
                            format!("'{}' durağında stop_access '{val}' geçersiz.", stop_id),
                            "stop_access için 0 veya 1 kullanın; 2 GTFS tarafından tanımlanmaz.",
                        ));
                        }
                    }
                    v
                }
                Err(err) => {
                    // A NON-NUMERIC value used to be silently dropped: an out-of-range number produced
                    // STP_024, while "abc" produced no finding. Two branches of the same fact → one rule
                    // (the PTH_027 precedent).
                    notices.push(make_k2_notice(
                        &mut counter,
                        "STP_024",
                        EntityType::Stop,
                        entity_id.clone(),
                        Some(&row_map),
                        &file.name,
                        Some(line),
                        Some("stop_access"),
                        get_trimmed_field(&row_map, "stop_access").map(str::to_string),
                        Some("0–2".to_string()),
                        err,
                        "stop_access için 0 veya 1 kullanın; 2 GTFS tarafından tanımlanmaz.",
                    ));
                    None
                }
            };

        let level_id = get_raw_field(&row_map, "level_id")
            .filter(|v| !v.trim().is_empty())
            .map(str::to_string);
        let tts_stop_name = get_trimmed_field(&row_map, "tts_stop_name")
            .filter(|v| !v.trim().is_empty())
            .map(str::to_string);

        // STP_044: platform_code must contain only a platform IDENTIFIER ("2b", "A", "5");
        // the spec explicitly excludes the words "platform"/"track" and their feed-language equivalent.
        //
        // ⚠️ The list is MULTILINGUAL and deliberately NARROW. Ambiguous words such as Italian
        // `via` ("street") are EXCLUDED: their chance of appearing in a platform code is not
        // lower than their chance of being part of a legitimate street name. Check corpus examples
        // before expanding it.
        //
        // STP_040 applies the same pattern to `stop_name` but is OPT-IN because "Union Station"
        // is a legitimate name. Here it is not opt-in because the spec directly forbids it.
        if let Some(pc) =
            get_trimmed_field(&row_map, "platform_code").filter(|v| !v.trim().is_empty())
        {
            const PLATFORM_WORDS: &[&str] = &[
                "platform",
                "track",
                "gleis",
                "peron",
                "perron",
                "binario",
                "voie",
                "quai",
                "spor",
                "anden",
                "andén",
                "plataforma",
            ];
            let lower = pc.to_lowercase();
            if lower
                .split(|c: char| !c.is_alphanumeric())
                .any(|w| PLATFORM_WORDS.contains(&w))
            {
                notices.push(make_k2_notice(
                    &mut counter, "STP_044", EntityType::Stop, entity_id.clone(),
                    Some(&row_map), &file.name, Some(line), Some("platform_code"),
                    Some(pc.to_string()), None,
                    format!("platform_code '{pc}' gereksiz platform/track sözcüğü içeriyor; yalnız tanımlayıcı olmalı."),
                    "platform_code'da yalnız platform tanımlayıcısını bırakın (ör. 'Gleis 5' yerine '5').",
                ));
            }
        }

        // STP_045: the spec defines platform_code as "a platform stop (a stop belonging to a
        // station)"; a stop with a platform code but no station parent contradicts this definition.
        // MD `platform_without_parent_station` (INFO) measures the same axis. STP_032 only checks
        // pathway endpoints; this rule is independent of pathways.
        if matches!(location_type, None | Some(0))
            && get_trimmed_field(&row_map, "platform_code").is_some_and(|v| !v.trim().is_empty())
            && get_trimmed_field(&row_map, "parent_station").is_none_or(|v| v.trim().is_empty())
        {
            stp045_count += 1;
            if stp045_examples.len() < 5 {
                if let Some(id) = entity_id.clone() {
                    stp045_examples.push(id);
                }
            }
        }

        // STP_023: tts_stop_name contains markup characters (SSML/HTML markup)
        if let Some(ref tts) = tts_stop_name {
            if tts.contains('<') || tts.contains('>') {
                notices.push(make_k2_notice(
                    &mut counter,
                    "STP_023",
                    EntityType::Stop,
                    entity_id.clone(),
                    Some(&row_map),
                    &file.name,
                    Some(line),
                    Some("tts_stop_name"),
                    Some(tts.clone()),
                    None,
                    format!(
                        "'{}' durağının tts_stop_name değeri '<' veya '>' karakteri içeriyor.",
                        stop_id
                    ),
                    "tts_stop_name'de HTML/SSML işaret karakterleri kullanmayın.",
                ));
            }
        }

        // STP_033: zone_id missing (stop_without_zone_id) — regular stops only (location_type=0 or empty)
        let zone_id = get_raw_field(&row_map, "zone_id")
            .filter(|v| !v.trim().is_empty())
            .map(str::to_string);
        let loc_type = location_type.unwrap_or(0);
        if zone_id.is_none() && loc_type == 0 {
            let mut notice = make_k2_notice(
                &mut counter,
                "STP_033",
                EntityType::Stop,
                entity_id.clone(),
                Some(&row_map),
                &file.name,
                Some(line),
                Some("zone_id"),
                None,
                None,
                format!(
                    "'{}' durağında zone_id eksik; ücret hesaplamaları etkilenebilir.",
                    stop_id
                ),
                "Ücret hesabı için zone_id alanını doldurun.",
            );
            let details = notice.details.get_or_insert_with(Default::default);
            details.insert("affected_stops".to_string(), "1".to_string());
            details.insert("example_stops".to_string(), stop_id.to_string());
            stp033_pending.push(notice);
        }

        let stop_url = get_lexical_field(&row_map, "stop_url")
            .filter(|v| !v.trim().is_empty())
            .map(str::to_string);
        // STP_042: spec type `URL`. All sibling fields have a format rule
        // (exactly the same pattern as route_url→RTS_005); stop_url only had STP_034/035
        // (same as another URL), so format was never measured.
        if let Some(ref url) = stop_url {
            if !looks_like_url(url) {
                notices.push(make_k2_notice(
                    &mut counter,
                    "STP_042",
                    EntityType::Stop,
                    entity_id.clone(),
                    Some(&row_map),
                    &file.name,
                    Some(line),
                    Some("stop_url"),
                    Some(url.clone()),
                    None,
                    "stop_url geçerli bir URL değil.".to_string(),
                    "stop_url için geçerli bir http/https URL'si kullanın.",
                ));
            }
        }

        records.push(StopRecord {
            stop_id,
            stop_code,
            stop_name,
            stop_lat,
            stop_lon,
            location_type,
            stop_timezone,
            wheelchair_boarding,
            stop_access,
            level_id,
            tts_stop_name,
            stop_url,
            row: row_map,
            line,
        });
    }

    // STP_022 decision (#15): three cases —
    //  (a) no stop_code anywhere in the feed and >1 stop → one summary ("unused");
    //  (b) stop_code is PARTIAL but missing-stop count is HIGH (>threshold) → one summary
    //      ("partial") — per-stop emit headroom/noise issue in large feeds (old aggregation gap);
    //  (c) otherwise (low volume), preserve per-stop notices (pinpoint detail is valuable).
    const STP022_AGG_THRESHOLD: usize = 50;
    let n = stp022_pending.len();
    if n > 1 && (!any_stop_code_present || n > STP022_AGG_THRESHOLD) {
        let msg = if !any_stop_code_present {
            format!("Feed genelinde stop_code hiç kullanılmamış: {n} durakta eksik — yolcular durağı kısa kodla tanıyamaz.")
        } else {
            format!("{n} durakta stop_code eksik (feed'de stop_code kısmen kullanılıyor) — yolcular bu durakları kısa kodla tanıyamaz.")
        };
        let examples: Vec<String> = stp022_pending
            .iter()
            .filter_map(|x| x.entity_id.clone())
            .take(5)
            .collect();
        let mut notice = make_k2_notice(
            &mut counter, "STP_022", EntityType::Feed, None,
            None, &file.name, None, Some("stop_code"),
            Some(n.to_string()), None,
            msg,
            "stops.txt'teki ilgili duraklara yolcuların tanıyabileceği stop_code değerleri ekleyin.",
        );
        let mut d = std::collections::BTreeMap::new();
        d.insert("affected_stops".to_string(), n.to_string());
        if !examples.is_empty() {
            d.insert("example_stops".to_string(), examples.join(", "));
        }
        notice.details = Some(d.into());
        notices.push(notice);
    } else {
        notices.append(&mut stp022_pending);
    }

    // STP_033 decision (#48): one feed summary when the missing-zone_id count exceeds the
    // threshold; otherwise preserve per-stop notices (pinpoint detail is valuable at low volume).
    const STP033_AGG_THRESHOLD: usize = 50;
    let n33 = stp033_pending.len();
    if n33 > STP033_AGG_THRESHOLD {
        let examples: Vec<String> = stp033_pending
            .iter()
            .filter_map(|x| x.entity_id.clone())
            .take(5)
            .collect();
        let mut notice = make_k2_notice(
            &mut counter,
            "STP_033",
            EntityType::Feed,
            None,
            None,
            &file.name,
            None,
            Some("zone_id"),
            Some(n33.to_string()),
            None,
            format!(
                "{n33} durakta zone_id eksik — ücret bölgesi tabanlı hesaplamalar etkilenebilir."
            ),
            "Ücret hesabı gerekiyorsa ilgili duraklara zone_id ekleyin.",
        );
        let mut d = std::collections::BTreeMap::new();
        d.insert("affected_stops".to_string(), n33.to_string());
        if !examples.is_empty() {
            d.insert("example_stops".to_string(), examples.join(", "));
        }
        notice.details = Some(d.into());
        notices.push(notice);
    } else {
        notices.append(&mut stp033_pending);
    }

    // STP_045: ONE summary per feed (STP_033 pattern, no threshold — informational note).
    if stp045_count > 0 {
        let mut notice = make_k2_notice(
            &mut counter,
            "STP_045",
            EntityType::Feed,
            None,
            None,
            &file.name,
            None,
            Some("parent_station"),
            Some(stp045_count.to_string()),
            None,
            format!(
                "{stp045_count} durak platform_code taşıyor ama parent_station boş — peron bir istasyona bağlanmamış."
            ),
            "Peronları location_type=1 olan istasyona parent_station ile bağlayın; peron değilse platform_code'u kaldırın.",
        );
        let mut d = std::collections::BTreeMap::new();
        d.insert("affected_stops".to_string(), stp045_count.to_string());
        if !stp045_examples.is_empty() {
            d.insert("example_stops".to_string(), stp045_examples.join(", "));
        }
        notice.details = Some(d.into());
        notices.push(notice);
    }

    // STP_018: no stops in stops.txt
    if records.is_empty() {
        notices.push(make_k2_notice(
            &mut counter,
            "STP_018",
            EntityType::Stop,
            None,
            None,
            &file.name,
            None,
            None,
            None,
            None,
            "stops.txt dosyasında hiç durak kaydı yok.".to_string(),
            "stops.txt'e en az bir durak kaydı ekleyin veya geçerli bir locations.geojson bölgesini stop_times.location_id ile kullanın.",
        ));
    }

    (records, notices)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::k1_parse::RawFile;

    fn make_file(headers: Vec<&str>, rows: Vec<Vec<&str>>) -> RawFile {
        RawFile {
            name: "stops.txt".to_string(),
            headers: headers.into_iter().map(str::to_string).collect(),
            rows: rows
                .into_iter()
                .map(|r| r.into_iter().map(smol_str::SmolStr::from).collect())
                .collect(),
            bytes: 0,
            raw_text: None,
            zip_entry_name: None,
        }
    }

    #[test]
    fn valid_stop_produces_no_notices() {
        let file = make_file(
            vec![
                "stop_id",
                "stop_name",
                "stop_lat",
                "stop_lon",
                "stop_code",
                "zone_id",
            ],
            vec![vec!["S1", "Main Stop", "41.0", "29.0", "S001", "Z1"]],
        );
        let (records, notices) = validate_stops(&file);
        assert_eq!(records.len(), 1);
        assert!(
            notices.is_empty(),
            "Geçerli durak notice üretmemeli: {:?}",
            notices
        );
    }

    #[test]
    fn stp_031_compares_case_beyond_ascii() {
        // Corpus run 14: the rule used `eq_ignore_ascii_case`, which equates ONLY a-z/A-Z.
        // It was blind in diacritic alphabets because it compares two USER TEXT values.
        // Measured with `mdb-1000` (Poland): normalization finds 6 stops, MD also says 6,
        // while we found 4. The two missed values were exactly the non-ASCII pair.
        // The same blindness affected Turkish (s-cedilla, g-breve), Czech, Greek, and Cyrillic.
        let file = make_file(
            vec!["stop_id", "stop_name", "stop_lat", "stop_lon", "stop_desc"],
            vec![
                // ASCII pair — was already caught, REGRESSION guard
                vec!["S1", "Buszkowiczki", "41.0", "29.0", "BUSZKOWICZKI"],
                // Non-ASCII pair (l-stroke) — was previously MISSED
                vec![
                    "S2",
                    "Stanis\u{142}awczyk",
                    "41.1",
                    "29.1",
                    "STANIS\u{141}AWCZYK",
                ],
                // Turkish pair (c-cedilla, o-umlaut, s-cedilla) — same class.
                // NOTE: `i` and dotless-i are deliberately not used. In Unicode's locale-neutral
                // casing this pair is ASYMMETRIC: dotless-i uppercases to `I`, but `I` lowercases
                // to `i`, not dotless-i. Thus "BAGCILAR" and "Bagcilar" (dotless) are NOT EQUAL
                // under Unicode, and the rule correctly stays silent; Turkish-specific casing
                // requires locale information and is outside this rule's scope.
                vec![
                    "S3",
                    "\u{e7}orlu g\u{f6}l \u{15f}ube",
                    "41.2",
                    "29.2",
                    "\u{c7}ORLU G\u{d6}L \u{15e}UBE",
                ],
                // GENUINELY different description — must stay silent
                vec!["S4", "Merkez", "41.3", "29.3", "Otogar karsisi"],
            ],
        );
        let (_records, notices) = validate_stops(&file);
        let hits: Vec<&str> = notices
            .iter()
            .filter(|n| n.rule_id == "STP_031")
            .filter_map(|n| n.entity_id.as_deref())
            .collect();
        assert!(
            hits.contains(&"S1"),
            "ASCII pair should be caught (regression): {hits:?}"
        );
        assert!(
            hits.contains(&"S2"),
            "non-ASCII pair (l-stroke) should be caught: {hits:?}"
        );
        assert!(hits.contains(&"S3"), "Turkish pair should be caught: {hits:?}");
        assert!(
            !hits.contains(&"S4"),
            "genuinely different description must stay SILENT: {hits:?}"
        );
        assert_eq!(
            hits.len(),
            3,
            "tam olarak uc esdeger cift beklenir: {hits:?}"
        );
    }

    #[test]
    fn stp_022_feed_wide_missing_aggregates_to_single_feed_notice() {
        // No stop has stop_code, >1 stop → one feed-level STP_022.
        let file = make_file(
            vec!["stop_id", "stop_name", "stop_lat", "stop_lon"],
            vec![
                vec!["S1", "Stop1", "41.0", "29.0"],
                vec!["S2", "Stop2", "41.1", "29.1"],
                vec!["S3", "Stop3", "41.2", "29.2"],
            ],
        );
        let (_, notices) = validate_stops(&file);
        let stp022: Vec<_> = notices.iter().filter(|n| n.rule_id == "STP_022").collect();
        assert_eq!(
            stp022.len(),
            1,
            "feed-geneli eksiklikte tek STP_022 beklenir"
        );
        assert_eq!(stp022[0].entity_type, EntityType::Feed);
        assert_eq!(stp022[0].observed_value.as_deref(), Some("3"));
    }

    #[test]
    fn stp_022_partial_missing_stays_per_stop() {
        // Some stops have stop_code → missing ones are flagged per stop.
        let file = make_file(
            vec!["stop_id", "stop_name", "stop_lat", "stop_lon", "stop_code"],
            vec![
                vec!["S1", "Stop1", "41.0", "29.0", "S001"],
                vec!["S2", "Stop2", "41.1", "29.1", ""],
                vec!["S3", "Stop3", "41.2", "29.2", ""],
            ],
        );
        let (_, notices) = validate_stops(&file);
        let stp022: Vec<_> = notices.iter().filter(|n| n.rule_id == "STP_022").collect();
        assert_eq!(
            stp022.len(),
            2,
            "partial population should produce per-stop STP_022"
        );
        assert!(stp022.iter().all(|n| n.entity_type == EntityType::Stop));
    }

    #[test]
    fn stp_022_partial_but_high_volume_aggregates() {
        // #15: stop_code is PARTIAL but the missing-stop count is high (>50) → one feed summary.
        let ids: Vec<String> = (0..=51).map(|i| format!("S{i}")).collect();
        let mut rows: Vec<Vec<&str>> = vec![vec![ids[0].as_str(), "Has", "41.0", "29.0", "C0"]];
        for id in &ids[1..] {
            rows.push(vec![id.as_str(), "X", "41.0", "29.0", ""]); // stop_code missing
        }
        let file = make_file(
            vec!["stop_id", "stop_name", "stop_lat", "stop_lon", "stop_code"],
            rows,
        );
        let (_, notices) = validate_stops(&file);
        let stp022: Vec<_> = notices.iter().filter(|n| n.rule_id == "STP_022").collect();
        assert_eq!(
            stp022.len(),
            1,
            "partial absence at high volume should produce one feed summary"
        );
        assert_eq!(stp022[0].entity_type, EntityType::Feed);
        assert_eq!(
            stp022[0]
                .details
                .as_ref()
                .and_then(|d| d.get("affected_stops"))
                .map(String::as_str),
            Some("51"),
        );
    }

    #[test]
    fn stp_033_low_volume_stays_per_stop() {
        // #48: missing zone_id count is low (≤50) → preserve per-stop STP_033.
        // Keep stop_code populated so STP_022 noise does not interfere.
        let file = make_file(
            vec!["stop_id", "stop_name", "stop_lat", "stop_lon", "stop_code"],
            vec![
                vec!["S1", "Stop1", "41.0", "29.0", "C1"],
                vec!["S2", "Stop2", "41.1", "29.1", "C2"],
                vec!["S3", "Stop3", "41.2", "29.2", "C3"],
            ],
        );
        let (_, notices) = validate_stops(&file);
        let stp033: Vec<_> = notices.iter().filter(|n| n.rule_id == "STP_033").collect();
        assert_eq!(
            stp033.len(),
            3,
            "per-stop STP_033 expected at low volume"
        );
        assert!(stp033.iter().all(|n| n.entity_type == EntityType::Stop));
    }

    #[test]
    fn stp_033_high_volume_aggregates_to_single_feed_notice() {
        // #48: missing zone_id count is high (>50) → one feed summary.
        let ids: Vec<String> = (0..=51).map(|i| format!("S{i}")).collect();
        let mut rows: Vec<Vec<&str>> = Vec::new();
        for id in &ids {
            rows.push(vec![id.as_str(), "X", "41.0", "29.0", id.as_str()]); // stop_code populated, zone_id missing
        }
        let file = make_file(
            vec!["stop_id", "stop_name", "stop_lat", "stop_lon", "stop_code"],
            rows,
        );
        let (_, notices) = validate_stops(&file);
        let stp033: Vec<_> = notices.iter().filter(|n| n.rule_id == "STP_033").collect();
        assert_eq!(stp033.len(), 1, "one feed summary expected at high volume");
        assert_eq!(stp033[0].entity_type, EntityType::Feed);
        assert_eq!(stp033[0].observed_value.as_deref(), Some("52"));
        assert_eq!(
            stp033[0]
                .details
                .as_ref()
                .and_then(|d| d.get("affected_stops"))
                .map(String::as_str),
            Some("52"),
        );
    }

    #[test]
    fn lat_out_of_range_produces_stp_003() {
        let file = make_file(
            vec!["stop_id", "stop_name", "stop_lat", "stop_lon"],
            vec![vec!["S1", "Stop1", "999.0", "29.0"]],
        );
        let (_, notices) = validate_stops(&file);
        assert!(
            notices.iter().any(|n| n.rule_id == "STP_003"),
            "stop_lat=999 should produce STP_003, got: {:?}",
            notices.iter().map(|n| &n.rule_id).collect::<Vec<_>>()
        );
    }

    #[test]
    fn missing_stop_name_produces_stp_003() {
        let file = make_file(
            vec!["stop_id", "stop_name", "stop_lat", "stop_lon"],
            vec![vec!["S1", "", "41.0", "29.0"]],
        );
        let (_, notices) = validate_stops(&file);
        assert!(notices.iter().any(|n| n.rule_id == "STP_003"));
    }

    #[test]
    fn entrance_missing_name_and_coords_produces_notices() {
        // stop_name/stop_lat/stop_lon are required for location_type=2 (entrance/exit)
        let file = make_file(
            vec![
                "stop_id",
                "stop_name",
                "stop_lat",
                "stop_lon",
                "location_type",
            ],
            vec![vec!["E1", "", "", "", "2"]],
        );
        let (_, notices) = validate_stops(&file);
        let ids: Vec<&str> = notices.iter().map(|n| n.rule_id.as_str()).collect();
        assert!(
            ids.contains(&"STP_003"),
            "missing stop_name → STP_003 expected: {:?}",
            ids
        );
        assert!(
            ids.contains(&"STP_006"),
            "missing stop_lat → STP_006 expected: {:?}",
            ids
        );
        assert!(
            ids.contains(&"STP_007"),
            "missing stop_lon → STP_007 expected: {:?}",
            ids
        );
    }

    #[test]
    fn generic_node_missing_name_and_coords_silent() {
        // stop_name/stop_lat/stop_lon are optional for location_type=3 (generic node)
        let file = make_file(
            vec![
                "stop_id",
                "stop_name",
                "stop_lat",
                "stop_lon",
                "location_type",
            ],
            vec![vec!["N1", "", "", "", "3"]],
        );
        let (_, notices) = validate_stops(&file);
        let ids: Vec<&str> = notices.iter().map(|n| n.rule_id.as_str()).collect();
        assert!(
            !ids.contains(&"STP_003"),
            "stop_name must not be required for a generic node: {:?}",
            ids
        );
        assert!(
            !ids.contains(&"STP_006"),
            "stop_lat must not be required for a generic node: {:?}",
            ids
        );
        assert!(
            !ids.contains(&"STP_007"),
            "stop_lon must not be required for a generic node: {:?}",
            ids
        );
    }

    #[test]
    fn invalid_wheelchair_boarding_produces_stp_013() {
        let file = make_file(
            vec![
                "stop_id",
                "stop_name",
                "stop_lat",
                "stop_lon",
                "wheelchair_boarding",
            ],
            vec![vec!["S1", "Stop", "41.0", "29.0", "9"]],
        );
        let (_, notices) = validate_stops(&file);
        assert!(notices.iter().any(|n| n.rule_id == "STP_013"));
    }

    #[test]
    fn invalid_location_type_produces_stp_008() {
        let file = make_file(
            vec![
                "stop_id",
                "stop_name",
                "stop_lat",
                "stop_lon",
                "location_type",
            ],
            vec![vec!["S1", "Stop", "41.0", "29.0", "9"]],
        );
        let (_, notices) = validate_stops(&file);
        assert!(notices.iter().any(|n| n.rule_id == "STP_008"));
    }
    #[test]
    fn duplicate_stop_code_produces_stp_039() {
        let file = make_file(
            vec!["stop_id", "stop_name", "stop_lat", "stop_lon", "stop_code"],
            vec![
                vec!["S1", "One", "41", "29", "100"],
                vec!["S2", "Two", "41.1", "29.1", "100"],
            ],
        );
        let (_, notices) = validate_stops(&file);
        assert_eq!(notices.iter().filter(|n| n.rule_id == "STP_039").count(), 1);
    }
    #[test]
    fn stp_045_flags_platform_code_without_parent_station_once_per_feed() {
        // Spec: platform_code "Platform identifier for a platform stop (a stop belonging to a
        // station)". Corpus jbda-akishimacity: 2 of 114 stops without a parent that carry
        // platform_code receive MD platform_without_parent_station.
        let file = make_file(
            vec![
                "stop_id",
                "stop_name",
                "stop_lat",
                "stop_lon",
                "location_type",
                "parent_station",
                "platform_code",
            ],
            vec![
                vec!["P1", "Peron 1", "41.0", "29.0", "0", "", "1"],
                vec!["P2", "Peron 2", "41.0", "29.0", "", "", "2"],
                vec!["P3", "Peron 3", "41.0", "29.0", "0", "ST", "3"],
                vec!["S1", "Durak", "41.0", "29.0", "0", "", ""],
                vec!["ST", "İstasyon", "41.0", "29.0", "1", "", ""],
            ],
        );
        let (_, notices) = validate_stops(&file);
        let hits: Vec<_> = notices.iter().filter(|n| n.rule_id == "STP_045").collect();
        assert_eq!(hits.len(), 1, "one summary per feed: {hits:?}");
        assert_eq!(hits[0].observed_value.as_deref(), Some("2"));
        let ex = hits[0]
            .details
            .as_ref()
            .and_then(|d| d.get("example_stops"))
            .unwrap();
        assert_eq!(ex, "P1, P2");
    }

    #[test]
    fn stp_044_flags_platform_word_in_platform_code() {
        let file = make_file(
            vec![
                "stop_id",
                "stop_name",
                "stop_lat",
                "stop_lon",
                "platform_code",
            ],
            vec![
                vec!["S1", "Ana", "41.0", "29.0", "Gleis 5"],
                vec!["S2", "Yan", "41.1", "29.1", "Track 2"],
                vec!["S3", "Üç", "41.2", "29.2", "peron 3"],
            ],
        );
        let (_, notices) = validate_stops(&file);
        let hits: Vec<_> = notices.iter().filter(|n| n.rule_id == "STP_044").collect();
        assert_eq!(hits.len(), 3, "must be caught in all three languages: {hits:?}");
        assert_eq!(hits[0].field.as_deref(), Some("platform_code"));
    }

    #[test]
    fn stp_044_silent_for_plain_identifiers() {
        // Correct use of platform_code: identifier only.
        let file = make_file(
            vec![
                "stop_id",
                "stop_name",
                "stop_lat",
                "stop_lon",
                "platform_code",
            ],
            vec![
                vec!["S1", "A", "41.0", "29.0", "5"],
                vec!["S2", "B", "41.1", "29.1", "2b"],
                vec!["S3", "C", "41.2", "29.2", "A"],
                vec!["S4", "D", "41.3", "29.3", ""],
                // `via` is deliberately NOT in the list — Italian for "street", ambiguous.
                vec!["S5", "E", "41.4", "29.4", "Via 4"],
                // A value containing the word as a PART must not match (word boundary).
                vec!["S6", "F", "41.5", "29.5", "trackside"],
            ],
        );
        let (_, notices) = validate_stops(&file);
        let hits: Vec<_> = notices.iter().filter(|n| n.rule_id == "STP_044").collect();
        assert!(hits.is_empty(), "false positive: {hits:?}");
    }
}
