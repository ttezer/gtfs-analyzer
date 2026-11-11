use std::collections::HashMap;

use gtfs_core::EntityType;

use super::common::{
    build_row_map, get_lexical_field, get_raw_field, get_trimmed_field, is_hex_color_6,
    looks_like_url, make_k2_notice, parse_u32, wcag_contrast_ratio, RowMap,
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

/// CORE GTFS Schedule Reference enum — the `routes.txt::route_type` field definition counts
/// exactly these: 0-7, 11, 12. The words "Extended"/HVT do NOT appear in the official field definition.
fn is_core_route_type(v: u32) -> bool {
    matches!(v, 0..=7 | 11 | 12)
}

/// Extended (HVT) route-type range — a convention of Google Transit and its derivatives,
/// NOT part of the core Schedule Reference.
fn is_extended_route_type(v: u32) -> bool {
    matches!(v, 100..=1799)
}

// JPN_027 performs the strict V3 route_type check in the cross-reference stage.
// General route classification remains local to the core and extended enum checks above.
// Keeping this boundary explicit prevents the regional rule from changing general GTFS
// semantics or from treating HVT bus values as V3-compliant values.
//
// The V3 profile requires the literal numeric value `3`; missing and malformed values are
// reported by the general route_type validation, while the regional check handles typed rows.
// This note also keeps the surrounding source anchors stable for the rule-card audit.
// It is intentionally descriptive rather than executable: route classification is not
// a regional-profile signal.

pub fn validate_routes(file: &RawFile) -> (Vec<RouteRecord>, Vec<gtfs_core::Notice>) {
    let mut notices = Vec::new();
    let mut records = Vec::new();
    let mut counter = 0u32;

    for (row_idx, row) in file.rows.iter().enumerate() {
        let line = (row_idx + 2) as u64;
        let row_map = build_row_map(&file.headers, row);

        let route_id = get_raw_field(&row_map, "route_id")
            .unwrap_or("")
            .to_string();
        let entity_id = (!route_id.is_empty()).then_some(route_id.clone());

        // RTS_031: `route_id` is empty — no primary key. This row is IDENTITY-LESS and
        // `entity_id` becomes `None` above; no rule used to see it.
        // `RTS_001` measures DUPLICATION, and a missing identity cannot be duplicated,
        // so the empty value fell between the two rules (#153, 4 feeds in the corpus).
        if route_id.trim().is_empty() {
            let mut n031 = make_k2_notice(
                &mut counter,
                "RTS_031",
                EntityType::Route,
                None,
                Some(&row_map),
                &file.name,
                Some(line),
                Some("route_id"),
                Some(String::new()),
                None,
                "route_id zorunludur.".to_string(),
                "Give every row in routes.txt a unique route_id.",
            );
            // Planner context: the code-less row's number and name.
            let details = n031.details.get_or_insert_with(Default::default);
            for field in ["route_short_name", "route_long_name"] {
                if let Some(value) = get_trimmed_field(&row_map, field).filter(|v| !v.is_empty()) {
                    details.insert(field.to_string(), value.to_string());
                }
            }
            notices.push(n031);
        }

        let agency_id = get_raw_field(&row_map, "agency_id")
            .filter(|v| !v.trim().is_empty())
            .map(str::to_string);

        let route_short_name = get_trimmed_field(&row_map, "route_short_name")
            .filter(|v| !v.trim().is_empty())
            .map(str::to_string);
        let route_long_name = get_trimmed_field(&row_map, "route_long_name")
            .filter(|v| !v.trim().is_empty())
            .map(str::to_string);

        // RTS_003: both route_short_name and route_long_name are empty
        //
        // `field` names both fields TOGETHER (the repository's established pipe convention:
        // `from_stop_id|to_stop_id`, `start_date|end_date`, `stop_lat|stop_lon`). It used to be `None`:
        // R2's "Field" column was empty for a finding that BLOCKS publication.
        // Naming only one field would be wrong — the rule is that both are empty TOGETHER.
        if route_short_name.is_none() && route_long_name.is_none() {
            notices.push(make_k2_notice(
                &mut counter, "RTS_003", EntityType::Route, entity_id.clone(), Some(&row_map),
                &file.name, Some(line), Some("route_short_name|route_long_name"), None, None,
                "route_short_name ve route_long_name alanlarının her ikisi de boş; en az biri zorunludur.".to_string(),
                "Fill in route_short_name or route_long_name.",
            ));
        }

        // RTS_009 (short == long) was REMOVED (2026-07-23): it was a strict subset of RTS_022
        // (K6, route_long_name_contains_short_name) — equality is also a containment case.
        // RTS_022 carries MD parity and already catches equality; removing RTS_009 preserves
        // parity (RTS_022 was untouched).

        // RTS_010: route_short_name is too long (>12 characters)
        if let Some(ref s) = route_short_name {
            // Count CHARACTERS, not BYTES. `str::len()` returns UTF-8 bytes: "Kızılay"
            // is 7 characters but 8 bytes, and a three-character Japanese short name is 9 bytes.
            // Because the threshold and message say "characters", byte counting produced false
            // positives in Turkish/Japanese feeds.
            let char_len = s.chars().count();
            if char_len > 12 {
                let mut n010 = make_k2_notice(
                    &mut counter, "RTS_010", EntityType::Route, entity_id.clone(), Some(&row_map),
                    &file.name, Some(line), Some("route_short_name"),
                    Some(char_len.to_string()), Some("≤12".to_string()),
                    format!("'{}' hattının route_short_name değeri {} karakter; kısa isimler 12 karakteri aşmamalıdır.", route_id, char_len),
                    "Keep route_short_name under 12 characters.",
                );
                // Planner context: the name itself.
                n010.details
                    .get_or_insert_with(Default::default)
                    .insert("name".to_string(), s.clone());
                notices.push(n010);
            } else if char_len > 6 && route_long_name.as_ref().is_none_or(|l| l.trim().is_empty()) {
                // RTS_021: longer than 6 characters (Google Transit threshold).
                // Google's own exemption: if `route_long_name` is populated, the warning may be
                // ignored — the long name already provides a readable display. Without the guard,
                // legitimate names such as "Green Line" produced unnecessary noise.
                let mut n021 = make_k2_notice(
                    &mut counter, "RTS_021", EntityType::Route, entity_id.clone(), Some(&row_map),
                    &file.name, Some(line), Some("route_short_name"),
                    Some(char_len.to_string()), Some("≤6 (Google)".to_string()),
                    format!("'{}' hattının route_short_name değeri {} karakter; Google Transit 6 karakteri tavsiye ediyor.", route_id, char_len),
                    "Keep route_short_name under 6 characters for Google Transit compatibility, or fill in route_long_name.",
                );
                // Planner context: the name itself.
                n021.details
                    .get_or_insert_with(Default::default)
                    .insert("name".to_string(), s.clone());
                notices.push(n021);
            }
        }

        // RTS_011: route_long_name is too long (>100 characters)
        if let Some(ref l) = route_long_name {
            let char_len = l.chars().count(); // characters, not bytes — see RTS_010/021 note
            if char_len > 100 {
                let mut n011 = make_k2_notice(
                    &mut counter,
                    "RTS_011",
                    EntityType::Route,
                    entity_id.clone(),
                    Some(&row_map),
                    &file.name,
                    Some(line),
                    Some("route_long_name"),
                    Some(char_len.to_string()),
                    Some("≤100".to_string()),
                    format!(
                        "'{}' hattının route_long_name değeri {} karakter; 100 karakteri aşıyor.",
                        route_id, char_len
                    ),
                    "Keep route_long_name under 100 characters.",
                );
                // Planner context: the name itself.
                n011.details
                    .get_or_insert_with(Default::default)
                    .insert("name".to_string(), l.clone());
                notices.push(n011);
            }
        }

        // route_type: required + must be a valid GTFS enum
        let route_type = match parse_u32(&row_map, "route_type") {
            Ok(v) => {
                match v {
                    None => {
                        // RTS_004: route_type missing (if the column is absent, ARC_025 takes over → skip)
                        if get_trimmed_field(&row_map, "route_type") == Some("") {
                            notices.push(make_k2_notice(
                                &mut counter,
                                "RTS_004",
                                EntityType::Route,
                                entity_id.clone(),
                                Some(&row_map),
                                &file.name,
                                Some(line),
                                Some("route_type"),
                                Some(String::new()),
                                None,
                                "route_type zorunludur.".to_string(),
                                "Enter a valid route_type (0-7, 11, 12).",
                            ));
                        }
                        None
                    }
                    Some(val) => {
                        // Two AUTHORITIES, two rules (#93). RTS_004 is `Spec`/KRİTİK and in the R1
                        // publication gate; the core Schedule Reference enum counts only 0-7,11,12.
                        // Adding the extended 100-1799 range to RTS_004's accepted set would present
                        // an extension convention as core-spec compliance. The reverse would also be
                        // wrong: reporting 1501 through RTS_004 would block publication for 19 of
                        // 225 feeds (8.4% of the corpus) that legitimately use an extension.
                        // The distinction moved to RTS_030.
                        if is_extended_route_type(val) {
                            // RTS_030: extended (HVT) type — outside the core enum, Interop
                            notices.push(make_k2_notice(
                                &mut counter, "RTS_030", EntityType::Route, entity_id.clone(), Some(&row_map),
                                &file.name, Some(line), Some("route_type"),
                                Some(val.to_string()), Some("0-7,11,12".to_string()),
                                format!("route_type {val} çekirdek GTFS enum'unda yok; genişletilmiş (HVT) hat tipi."),
                                "Use a core route_type (0-7, 11, 12) for core GTFS compliance; the extended value may be deliberate if your consumers support it.",
                            ));
                        } else if !is_core_route_type(val) {
                            // RTS_004: route_type invalid enum value
                            notices.push(make_k2_notice(
                                &mut counter,
                                "RTS_004",
                                EntityType::Route,
                                entity_id.clone(),
                                Some(&row_map),
                                &file.name,
                                Some(line),
                                Some("route_type"),
                                Some(val.to_string()),
                                Some("0-7,11,12".to_string()),
                                format!("route_type {val} geçerli bir GTFS hat tipi değil."),
                                "Use a valid route_type (0-7, 11, 12).",
                            ));
                        }
                        Some(val)
                    }
                }
            }
            Err(err) => {
                notices.push(make_k2_notice(
                    &mut counter,
                    "RTS_004",
                    EntityType::Route,
                    entity_id.clone(),
                    Some(&row_map),
                    &file.name,
                    Some(line),
                    Some("route_type"),
                    get_trimmed_field(&row_map, "route_type").map(str::to_string),
                    None,
                    err,
                    "Enter a numeric route_type.",
                ));
                None
            }
        };

        // RTS_005: route_url must be valid URL if provided
        let route_url = get_lexical_field(&row_map, "route_url")
            .filter(|v| !v.trim().is_empty())
            .map(str::to_string);
        if let Some(ref url) = route_url {
            if !looks_like_url(url) {
                notices.push(make_k2_notice(
                    &mut counter,
                    "RTS_005",
                    EntityType::Route,
                    entity_id.clone(),
                    Some(&row_map),
                    &file.name,
                    Some(line),
                    Some("route_url"),
                    Some(url.clone()),
                    None,
                    "route_url geçerli bir URL değil.".to_string(),
                    "Use a valid http/https URL for route_url.",
                ));
            }
        }

        // RTS_006: route_color must be valid 6-char hex
        let route_color = get_lexical_field(&row_map, "route_color")
            .filter(|v| !v.trim().is_empty())
            .map(str::to_string);
        if let Some(ref color) = route_color {
            if !is_hex_color_6(color) {
                notices.push(make_k2_notice(
                    &mut counter,
                    "RTS_006",
                    EntityType::Route,
                    entity_id.clone(),
                    Some(&row_map),
                    &file.name,
                    Some(line),
                    Some("route_color"),
                    Some(color.clone()),
                    Some("6 haneli hex".to_string()),
                    "route_color 6 karakterli hex renk kodu olmalıdır.".to_string(),
                    "Use a valid 6-digit hex colour (e.g., FF0000).",
                ));
            }
        }

        // RTS_007: route_text_color must be valid 6-char hex
        let route_text_color = get_lexical_field(&row_map, "route_text_color")
            .filter(|v| !v.trim().is_empty())
            .map(str::to_string);
        if let Some(ref color) = route_text_color {
            if !is_hex_color_6(color) {
                notices.push(make_k2_notice(
                    &mut counter,
                    "RTS_007",
                    EntityType::Route,
                    entity_id.clone(),
                    Some(&row_map),
                    &file.name,
                    Some(line),
                    Some("route_text_color"),
                    Some(color.clone()),
                    Some("6 haneli hex".to_string()),
                    "route_text_color 6 karakterli hex renk kodu olmalıdır.".to_string(),
                    "Use a valid 6-digit hex colour (e.g., FFFFFF).",
                ));
            }
        }

        // RTS_008: WCAG contrast ratio check. The format rule (RTS_006/007) uses the raw value;
        // contrast is measured with the trimmed color — `'FFFFFF    '` is still unreadable on a
        // yellow background (corpus mdb-1065/2653/465, 2026-10-13; `whitespace_trimmed_service_date` precedent).
        if let (Some(fg), Some(bg)) = (route_text_color.as_deref(), route_color.as_deref()) {
            let (fg, bg) = (fg.trim(), bg.trim());
            if is_hex_color_6(fg) && is_hex_color_6(bg) {
                if let Some(ratio) = wcag_contrast_ratio(fg, bg) {
                    if ratio < 3.0 {
                        let mut n008 = make_k2_notice(
                            &mut counter, "RTS_008", EntityType::Route, entity_id.clone(), Some(&row_map),
                            &file.name, Some(line), Some("route_text_color"),
                            Some(format!("{ratio:.2}")), Some(">= 3.0".to_string()),
                            format!("route_text_color ile route_color arasındaki kontrast oranı düşük ({ratio:.2})."),
                            "Use colours with at least a 3:1 WCAG contrast ratio.",
                        );
                        // Planner context: the two colours, to preview the badge.
                        let d = n008.details.get_or_insert_with(Default::default);
                        d.insert("color".to_string(), bg.to_string());
                        d.insert("text_color".to_string(), fg.to_string());
                        notices.push(n008);
                    }
                }
            }
        }

        // RTS_013: continuous_pickup invalid enum
        let continuous_pickup = parse_continuous_field(
            &row_map,
            &mut notices,
            &mut counter,
            "RTS_013",
            "continuous_pickup",
            &entity_id,
            line,
            &file.name,
        );

        // RTS_018: continuous_drop_off invalid enum
        let continuous_drop_off = parse_continuous_field(
            &row_map,
            &mut notices,
            &mut counter,
            "RTS_018",
            "continuous_drop_off",
            &entity_id,
            line,
            &file.name,
        );

        // RTS_029: spec type `Non-negative integer`. Negative or non-numeric values used to be
        // silently dropped — the route's ordering disappeared and the user saw nothing.
        let route_sort_order = match parse_u32(&row_map, "route_sort_order") {
            Ok(v) => v,
            Err(err) => {
                notices.push(make_k2_notice(
                    &mut counter,
                    "RTS_029",
                    EntityType::Route,
                    entity_id.clone(),
                    Some(&row_map),
                    &file.name,
                    Some(line),
                    Some("route_sort_order"),
                    get_trimmed_field(&row_map, "route_sort_order").map(str::to_string),
                    Some("negatif olmayan tam sayı".to_string()),
                    err,
                    "Give route_sort_order as a non-negative whole number.",
                ));
                None
            }
        };

        let route_desc = get_trimmed_field(&row_map, "route_desc")
            .filter(|v| !v.trim().is_empty())
            .map(str::to_string);

        // RTS_023: route_desc equals route_long_name OR route_short_name
        // (MD `same_name_and_description_for_route`; MD examples use `specifiedField` to say
        // which field matched, and check BOTH fields).
        //
        // The route_short_name branch was added on 2026-07-17: checking only long_name was a real
        // COVERAGE GAP — in the 250-feed corpus, MD reported 436 findings for mdb-2898 while we
        // reported 0, and all 436 had `specifiedField` = route_short_name
        // (for example, route_desc="EXT" ↔ route_short_name="EXT").
        //
        // Even if both fields match, emit ONE notice per route (same root cause: the description
        // copies a name); the message states which field(s) matched.
        {
            let dl = route_desc.as_ref().map(|d| d.to_lowercase());
            let matches_long =
                matches!((&route_long_name, &dl), (Some(l), Some(d)) if &l.to_lowercase() == d);
            let matches_short =
                matches!((&route_short_name, &dl), (Some(s), Some(d)) if &s.to_lowercase() == d);
            if let (Some(d), true) = (&route_desc, matches_long || matches_short) {
                // The matched field name is written to `details.matched_field`: message templates
                // (tr/en/ja) read which field was copied from here — hard-coding "route_long_name"
                // in the locale would be WRONG INFORMATION for short_name cases.
                let matched_field = match (matches_long, matches_short) {
                    (true, true) => "route_long_name, route_short_name",
                    (true, false) => "route_long_name",
                    _ => "route_short_name",
                };
                let mut n = make_k2_notice(
                    &mut counter, "RTS_023", EntityType::Route, entity_id.clone(), Some(&row_map),
                    &file.name, Some(line), Some("route_desc"), Some(d.clone()), None,
                    format!("'{route_id}' hattının route_desc değeri {matched_field} ile aynı: '{d}'."),
                    "route_desc should describe the route, not repeat its name.",
                );
                n.details = Some(std::collections::BTreeMap::from([(
                    "matched_field".to_string(),
                    matched_field.to_string(),
                )]).into());
                notices.push(n);
            }
        }

        let network_id = get_raw_field(&row_map, "network_id")
            .filter(|v| !v.trim().is_empty())
            .map(str::to_string);

        // GTFS-JP: jp_office_id (defined in the official routes.txt spec; JPN_002 office_jp FK check)
        let jp_office_id = get_raw_field(&row_map, "jp_office_id")
            .filter(|v| !v.trim().is_empty())
            .map(str::to_string);

        // RTS_024: route_cemv_support must be 0, 1, or 2 (AGN_012 route twin)
        let route_cemv_support = match parse_u32(&row_map, "cemv_support") {
            Ok(v) => {
                if let Some(val) = v {
                    if val > 2 {
                        notices.push(make_k2_notice(
                            &mut counter, "RTS_024", EntityType::Route, entity_id.clone(), Some(&row_map),
                            &file.name, Some(line), Some("cemv_support"), Some(val.to_string()),
                            Some("0, 1 or 2".to_string()),
                            "cemv_support 0, 1 veya 2 olmalıdır.".to_string(),
                            "Set cemv_support to 0 (no info), 1 (supported) or 2 (not supported).",
                        ));
                    }
                }
                v
            }
            Err(err) => {
                // A NON-NUMERIC value used to be silently dropped: an out-of-range number produced
                // RTS_024, while "abc" produced no finding. Two branches of the same fact → one rule
                // (the PTH_027 precedent).
                notices.push(make_k2_notice(
                    &mut counter, "RTS_024", EntityType::Route, entity_id.clone(),
                    Some(&row_map), &file.name, Some(line), Some("cemv_support"),
                    get_trimmed_field(&row_map, "cemv_support").map(str::to_string), Some("0, 1 or 2".to_string()),
                    err,
                    "Set cemv_support to 0 (no info), 1 (supported) or 2 (not supported).",
                ));
                None
            }
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

    // The duplicate-name checks below repeatedly access each group's anchor route record.
    // Using `records.iter().find(...)` produces O(group × route) behavior in feeds with many
    // routes and duplicate groups (VlakEShopAll: 64,711 groups × 321,962 routes). This index
    // preserves the first record and makes lookup O(1) without changing semantics; if route_id
    // is duplicated, the first row wins just as with the old linear `find`.
    let mut route_by_id: HashMap<&str, &RouteRecord> = HashMap::with_capacity(records.len());
    for rec in &records {
        route_by_id.entry(rec.route_id.as_str()).or_insert(rec);
    }

    // RTS_019: duplicate route name — a route matches another in BOTH route_short_name AND
    // route_long_name (a true duplicate route). Sharing only the short OR only the long name is
    // NORMAL (the same route number is reused for variants/directions; different routes may share
    // a long name) → aligned with MobilityData `duplicate_route_name`; matching only one caused false positives.
    // ONE notice per conflicting (short,long) pair; list all routes (route_id) sharing the name.
    {
        // Key (short, long, TYPE, AGENCY): an "indistinguishable duplicate" claim is valid only
        // within the same mode AND the same operator. Two agencies having a route "10" is normal
        // (passengers distinguish them by agency); a bus "10" and a tram "10" are distinguished
        // by mode. GTFS/MobilityData defines it this way:
        // "All routes of the same route_type with the same agency_id should have unique
        // combinations of route_short_name and route_long_name."
        //
        // Without type/agency in the key it produced false positives — 250-feed corpus, mdb-1091
        // (Luxembourg): 29 of 32 groups had different route_type/agency_id values (e.g. "10" →
        // LU::Authority:16:: and LU::Authority:26::). Completing the key changed 32 → 3, exactly matching MD.
        type NameKey = (String, String, Option<u32>, String);
        let mut name_groups: HashMap<NameKey, Vec<(String, u64)>> = HashMap::new();
        for rec in &records {
            let sn = rec
                .route_short_name
                .as_deref()
                .unwrap_or("")
                .trim()
                .to_lowercase();
            let ln = rec
                .route_long_name
                .as_deref()
                .unwrap_or("")
                .trim()
                .to_lowercase();
            if sn.is_empty() && ln.is_empty() {
                continue;
            }
            let agency = rec.agency_id.as_deref().unwrap_or("").trim().to_string();
            name_groups
                .entry((sn, ln, rec.route_type, agency))
                .or_default()
                .push((rec.route_id.clone(), rec.line));
        }
        // Sort by key for deterministic output.
        let mut groups: Vec<_> = name_groups
            .into_iter()
            .filter(|(_, e)| e.len() >= 2)
            .collect();
        groups.sort_by(|a, b| a.0.cmp(&b.0));
        for (_key, entries) in &groups {
            let group_str = entries
                .iter()
                .map(|(id, _)| id.as_str())
                .collect::<Vec<_>>()
                .join(", ");
            let (anchor_id, anchor_line) = &entries[0];
            let anchor = route_by_id.get(anchor_id.as_str()).copied();
            let sn_disp = anchor
                .and_then(|r| r.route_short_name.as_deref())
                .unwrap_or("");
            let ln_disp = anchor
                .and_then(|r| r.route_long_name.as_deref())
                .unwrap_or("");
            let display = match (sn_disp.is_empty(), ln_disp.is_empty()) {
                (false, false) => format!("{sn_disp} — {ln_disp}"),
                (false, true) => sn_disp.to_string(),
                _ => ln_disp.to_string(),
            };
            let mut n = make_k2_notice(
                &mut counter, "RTS_019", EntityType::Route,
                Some(anchor_id.clone()), None,
                "routes.txt", Some(*anchor_line), Some("route_long_name"),
                Some(display.clone()), None,
                format!("'{display}' adı birden fazla hatta kullanılıyor: {group_str}."),
                "Check the routes: give distinct names to separate services, or use one route_id for the same service. No action is needed if this is intentional.",
            );
            n.details = Some(
                [("conflicting_routes".to_string(), group_str.clone())]
                    .into_iter()
                    .collect(),
            );
            notices.push(n);
        }
    }

    // RTS_026: duplicate short route name — ≥2 routes share route_short_name BUT have
    // ≥2 different route_long_name values (same number, different names). If both matched,
    // it would be RTS_019 (MEDIUM) → excluded here. Often intentional (variant/direction) → INFO.
    // ONE notice per short name; list all routes sharing it.
    {
        let mut short_groups: HashMap<String, Vec<(String, String, u64)>> = HashMap::new();
        for rec in &records {
            let sn = rec
                .route_short_name
                .as_deref()
                .unwrap_or("")
                .trim()
                .to_lowercase();
            if sn.is_empty() {
                continue;
            }
            let ln = rec
                .route_long_name
                .as_deref()
                .unwrap_or("")
                .trim()
                .to_lowercase();
            short_groups
                .entry(sn)
                .or_default()
                .push((rec.route_id.clone(), ln, rec.line));
        }
        let mut groups: Vec<_> = short_groups
            .into_iter()
            .filter(|(_, e)| {
                e.len() >= 2
                    && e.iter()
                        .map(|(_, ln, _)| ln.as_str())
                        .collect::<std::collections::HashSet<_>>()
                        .len()
                        >= 2
            })
            .collect();
        groups.sort_by(|a, b| a.0.cmp(&b.0));
        for (_sn, entries) in &groups {
            let group_str = entries
                .iter()
                .map(|(id, _, _)| id.as_str())
                .collect::<Vec<_>>()
                .join(", ");
            let (anchor_id, _, anchor_line) = &entries[0];
            let sn_disp = route_by_id
                .get(anchor_id.as_str())
                .copied()
                .and_then(|r| r.route_short_name.as_deref())
                .unwrap_or("");
            let mut n = make_k2_notice(
                &mut counter, "RTS_026", EntityType::Route,
                Some(anchor_id.clone()), None,
                "routes.txt", Some(*anchor_line), Some("route_short_name"),
                Some(sn_disp.to_string()), None,
                format!("'{sn_disp}' kısa hat adı, farklı uzun adlı şu hatlar tarafından paylaşılıyor: {group_str}."),
                "Ignore if the route number is intentionally reused for variants/directions; otherwise make it unique.",
            );
            n.details = Some(
                [("conflicting_routes".to_string(), group_str.clone())]
                    .into_iter()
                    .collect(),
            );
            notices.push(n);
        }
    }

    // RTS_027: duplicate long route name — ≥2 routes share route_long_name BUT have
    // ≥2 different route_short_name values (different numbers, same description). If both matched,
    // it would be RTS_019 (MEDIUM) → excluded. INFO. ONE notice per long name; list sharing routes.
    {
        let mut long_groups: HashMap<String, Vec<(String, String, u64)>> = HashMap::new();
        for rec in &records {
            let ln = rec
                .route_long_name
                .as_deref()
                .unwrap_or("")
                .trim()
                .to_lowercase();
            if ln.is_empty() {
                continue;
            }
            let sn = rec
                .route_short_name
                .as_deref()
                .unwrap_or("")
                .trim()
                .to_lowercase();
            long_groups
                .entry(ln)
                .or_default()
                .push((rec.route_id.clone(), sn, rec.line));
        }
        let mut groups: Vec<_> = long_groups
            .into_iter()
            .filter(|(_, e)| {
                e.len() >= 2
                    && e.iter()
                        .map(|(_, sn, _)| sn.as_str())
                        .collect::<std::collections::HashSet<_>>()
                        .len()
                        >= 2
            })
            .collect();
        groups.sort_by(|a, b| a.0.cmp(&b.0));
        for (_ln, entries) in &groups {
            let group_str = entries
                .iter()
                .map(|(id, _, _)| id.as_str())
                .collect::<Vec<_>>()
                .join(", ");
            let (anchor_id, _, anchor_line) = &entries[0];
            let ln_disp = route_by_id
                .get(anchor_id.as_str())
                .copied()
                .and_then(|r| r.route_long_name.as_deref())
                .unwrap_or("");
            let mut n = make_k2_notice(
                &mut counter, "RTS_027", EntityType::Route,
                Some(anchor_id.clone()), None,
                "routes.txt", Some(*anchor_line), Some("route_long_name"),
                Some(ln_disp.to_string()), None,
                format!("'{ln_disp}' uzun hat adı, farklı kısa adlı şu hatlar tarafından paylaşılıyor: {group_str}."),
                "Ignore if the long name is shared intentionally; otherwise give each route a distinguishing name.",
            );
            n.details = Some(
                [("conflicting_routes".to_string(), group_str.clone())]
                    .into_iter()
                    .collect(),
            );
            notices.push(n);
        }
    }

    (records, notices)
}

// The parsed field and notice metadata are inputs to the same small validator;
// adding a parameter struct here would be a one-use abstraction.
#[allow(clippy::too_many_arguments)]
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
                if !matches!(val, 0..=3) {
                    notices.push(make_k2_notice(
                        counter,
                        rule_id,
                        EntityType::Route,
                        entity_id.clone(),
                        Some(row_map),
                        file_name,
                        Some(line),
                        Some(field),
                        Some(val.to_string()),
                        Some("0-3".to_string()),
                        format!("{field} alanı 0, 1, 2 veya 3 olmalıdır."),
                        "Alanı geçerli bir GTFS sürekli servis değerine ayarlayın (0-3).",
                    ));
                }
            }
            v
        }
        Err(err) => {
            notices.push(make_k2_notice(
                counter,
                rule_id,
                EntityType::Route,
                entity_id.clone(),
                Some(row_map),
                file_name,
                Some(line),
                Some(field),
                get_trimmed_field(row_map, field).map(str::to_string),
                Some("0-3".to_string()),
                err,
                "Alanı 0, 1, 2 veya 3 değerlerinden biri olarak girin.",
            ));
            None
        }
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
    fn valid_route_produces_no_notices() {
        let file = make_file(
            vec!["route_id", "route_short_name", "route_type"],
            vec![vec!["R1", "101", "3"]],
        );
        let (records, notices) = validate_routes(&file);
        assert_eq!(records.len(), 1);
        assert!(
            notices.is_empty(),
            "Geçerli rota notice üretmemeli: {:?}",
            notices
        );
    }

    fn ids(file: &RawFile) -> Vec<String> {
        validate_routes(file)
            .1
            .iter()
            .map(|n| n.rule_id.clone())
            .collect()
    }

    #[test]
    fn length_rules_count_characters_not_bytes() {
        // "Kızılay" is 7 characters / 8 bytes; byte counting would incorrectly exceed
        // Google's threshold of 6.
        // Leave `route_long_name` empty so the RTS_021 exemption does not apply.
        let f = make_file(
            vec![
                "route_id",
                "route_short_name",
                "route_long_name",
                "route_type",
            ],
            vec![vec!["R1", "Kızıla", "", "3"]], // 6 characters / 7 bytes → below the threshold
        );
        assert!(
            !ids(&f).contains(&"RTS_021".to_string()),
            "6 karakter eşiği aşmamalı: {:?}",
            ids(&f)
        );

        // A 12-character Japanese name = 36 bytes; byte counting would incorrectly trigger RTS_010.
        let f = make_file(
            vec![
                "route_id",
                "route_short_name",
                "route_long_name",
                "route_type",
            ],
            vec![vec!["R1", "東京都交通局都営バス系統", "", "3"]],
        );
        let got = ids(&f);
        assert!(
            !got.contains(&"RTS_010".to_string()),
            "12 characters is not >12: {got:?}"
        );
        assert!(
            got.contains(&"RTS_021".to_string()),
            "12 characters exceed Google's threshold: {got:?}"
        );
    }

    #[test]
    fn rts_021_is_waived_when_long_name_is_present() {
        // Google's exemption: when `route_long_name` is populated, the short-name warning may be ignored.
        let with_long = make_file(
            vec![
                "route_id",
                "route_short_name",
                "route_long_name",
                "route_type",
            ],
            vec![vec!["R1", "Green Line", "Green Line Downtown", "3"]],
        );
        assert!(
            !ids(&with_long).contains(&"RTS_021".to_string()),
            "RTS_021 should not be produced when long_name is populated: {:?}",
            ids(&with_long)
        );

        // A whitespace-only long_name does not count for the exemption.
        let blank_long = make_file(
            vec![
                "route_id",
                "route_short_name",
                "route_long_name",
                "route_type",
            ],
            vec![vec!["R1", "Green Line", "   ", "3"]],
        );
        assert!(
            ids(&blank_long).contains(&"RTS_021".to_string()),
            "a blank long_name should not count for the exemption: {:?}",
            ids(&blank_long)
        );
    }

    #[test]
    fn invalid_route_type_produces_rts_004() {
        let file = make_file(
            vec!["route_id", "route_short_name", "route_type"],
            vec![vec!["R1", "10", "99"]],
        );
        let (_, notices) = validate_routes(&file);
        assert!(
            notices.iter().any(|n| n.rule_id == "RTS_004"),
            "route_type=99 must produce RTS_004, got: {:?}",
            notices.iter().map(|n| &n.rule_id).collect::<Vec<_>>()
        );
    }

    #[test]
    fn both_names_empty_produces_rts_003() {
        let file = make_file(
            vec![
                "route_id",
                "route_short_name",
                "route_long_name",
                "route_type",
            ],
            vec![vec!["R1", "", "", "3"]],
        );
        let (_, notices) = validate_routes(&file);
        assert!(notices.iter().any(|n| n.rule_id == "RTS_003"));
    }

    #[test]
    fn invalid_route_color_produces_rts_006() {
        let file = make_file(
            vec!["route_id", "route_short_name", "route_type", "route_color"],
            vec![vec!["R1", "10", "3", "ZZZZZZ"]],
        );
        let (_, notices) = validate_routes(&file);
        assert!(notices.iter().any(|n| n.rule_id == "RTS_006"));
    }

    #[test]
    fn non_numeric_continuous_values_produce_their_rules() {
        let file = make_file(
            vec![
                "route_id",
                "route_short_name",
                "route_type",
                "continuous_pickup",
                "continuous_drop_off",
            ],
            vec![vec!["R1", "10", "3", "abc", "xyz"]],
        );
        let (_, notices) = validate_routes(&file);
        assert!(
            notices.iter().any(|n| n.rule_id == "RTS_013"),
            "RTS_013 bekleniyor: {notices:?}"
        );
        assert!(
            notices.iter().any(|n| n.rule_id == "RTS_018"),
            "RTS_018 bekleniyor: {notices:?}"
        );
    }

    /// The same name under DIFFERENT AGENCIES is not a duplicate — two operators having a
    /// route "10" is normal. Corpus evidence (mdb-1091, Luxembourg): 29 of 32 groups have
    /// different agency/type; once the key is complete, 32 → 3, matching MobilityData exactly.
    /// route_desc == route_short_name is also RTS_023 (MD `same_name_and_description_for_route`
    /// checks both names). Looking only at long_name was a coverage gap: in mdb-2898, MD
    /// reported 436 findings while we reported 0; all 436 were short_name matches.
    #[test]
    fn route_desc_equal_to_short_name_produces_rts_023() {
        let file = make_file(
            vec![
                "route_id",
                "route_short_name",
                "route_long_name",
                "route_desc",
                "route_type",
            ],
            vec![vec!["R1", "EXT", "Merkez - Sahil", "EXT", "3"]],
        );
        let (_, notices) = validate_routes(&file);
        assert_eq!(
            notices.iter().filter(|n| n.rule_id == "RTS_023").count(),
            1,
            "route_desc == route_short_name should produce RTS_023"
        );
    }

    /// Even when both names match, produce ONE notice per route (same root cause).
    #[test]
    fn route_desc_equal_to_both_names_produces_single_rts_023() {
        let file = make_file(
            vec![
                "route_id",
                "route_short_name",
                "route_long_name",
                "route_desc",
                "route_type",
            ],
            vec![vec!["R1", "X", "X", "X", "3"]],
        );
        let (_, notices) = validate_routes(&file);
        assert_eq!(
            notices.iter().filter(|n| n.rule_id == "RTS_023").count(),
            1,
            "only one notice even when both fields match"
        );
    }

    /// A different route_desc must not trigger it (the fix must not over-trigger).
    #[test]
    fn route_desc_different_from_names_is_not_rts_023() {
        let file = make_file(
            vec![
                "route_id",
                "route_short_name",
                "route_long_name",
                "route_desc",
                "route_type",
            ],
            vec![vec![
                "R1",
                "10",
                "Merkez - Sahil",
                "Sahil boyunca ekspres",
                "3",
            ]],
        );
        let (_, notices) = validate_routes(&file);
        assert!(
            !notices.iter().any(|n| n.rule_id == "RTS_023"),
            "a genuinely descriptive route_desc must not trigger it"
        );
    }

    #[test]
    fn same_name_different_agency_is_not_rts_019() {
        let file = make_file(
            vec![
                "route_id",
                "agency_id",
                "route_short_name",
                "route_long_name",
                "route_type",
            ],
            vec![
                vec!["R1", "A1", "10", "", "3"],
                vec!["R2", "A2", "10", "", "3"], // different agency → NOT a duplicate
            ],
        );
        let (_, notices) = validate_routes(&file);
        assert!(
            !notices.iter().any(|n| n.rule_id == "RTS_019"),
            "the same route number under a different agency must not count as a duplicate"
        );
    }

    /// The same name under a DIFFERENT TYPE is also not a duplicate — bus "10" and tram "10"
    /// are distinct by mode.
    #[test]
    fn same_name_different_route_type_is_not_rts_019() {
        let file = make_file(
            vec![
                "route_id",
                "agency_id",
                "route_short_name",
                "route_long_name",
                "route_type",
            ],
            vec![
                vec!["R1", "A1", "10", "Merkez", "3"], // bus
                vec!["R2", "A1", "10", "Merkez", "0"], // tram
            ],
        );
        let (_, notices) = validate_routes(&file);
        assert!(
            !notices.iter().any(|n| n.rule_id == "RTS_019"),
            "the same name under a different transport type must not count as a duplicate"
        );
    }

    /// SAME agency + SAME type + same name → a real duplicate, must trigger (the fix must preserve this).
    #[test]
    fn same_name_same_agency_and_type_is_rts_019() {
        let file = make_file(
            vec![
                "route_id",
                "agency_id",
                "route_short_name",
                "route_long_name",
                "route_type",
            ],
            vec![
                vec!["R1", "A1", "10", "Merkez", "3"],
                vec!["R2", "A1", "10", "Merkez", "3"],
            ],
        );
        let (_, notices) = validate_routes(&file);
        assert_eq!(
            notices.iter().filter(|n| n.rule_id == "RTS_019").count(),
            1,
            "same agency+type+name is a real duplicate → should produce RTS_019"
        );
    }

    #[test]
    fn shared_route_name_produces_single_rts_019_per_group() {
        // 3 routes with BOTH the same short and long names → real duplicates →
        // ONE RTS_019 per overlapping group.
        let file = make_file(
            vec![
                "route_id",
                "route_short_name",
                "route_long_name",
                "route_type",
            ],
            vec![
                vec!["R1", "QM2", "Astoria - Midtown", "3"],
                vec!["R2", "QM2", "Astoria - Midtown", "3"],
                vec!["R3", "QM2", "Astoria - Midtown", "3"],
            ],
        );
        let (_, notices) = validate_routes(&file);
        let rts019: Vec<_> = notices.iter().filter(|n| n.rule_id == "RTS_019").collect();
        assert_eq!(
            rts019.len(),
            1,
            "Grup başına tek RTS_019 bekleniyor, {} üretildi: {:?}",
            rts019.len(),
            rts019.iter().map(|n| &n.message).collect::<Vec<_>>()
        );
        // The message must list ALL members (route_id) of the group.
        let msg = &rts019[0].message;
        assert!(
            msg.contains("R1") && msg.contains("R2") && msg.contains("R3"),
            "Message must list the entire group: {}",
            msg
        );
        // ALL short and long names match → only RTS_019; the single-field RTS_026/027
        // rules must not trigger.
        assert!(
            !notices
                .iter()
                .any(|n| n.rule_id == "RTS_026" || n.rule_id == "RTS_027"),
            "both-same group must not produce RTS_026/027 (only RTS_019)"
        );
    }

    #[test]
    fn shared_short_name_different_long_produces_rts_026_not_rts_019() {
        // Same route_short_name but DIFFERENT route_long_name (variant/direction) → NOT real
        // duplicates → no RTS_019 (MEDIUM), but same number with different names → RTS_026
        // (Info). This was the root of the Athens mdb-3220 FP (109 groups); it is now INFO,
        // not MEDIUM.
        let file = make_file(
            vec![
                "route_id",
                "route_short_name",
                "route_long_name",
                "route_type",
            ],
            vec![
                vec!["R1", "221", "Merkez - Kuzey", "3"],
                vec!["R2", "221", "Merkez - Güney", "3"],
            ],
        );
        let (_, notices) = validate_routes(&file);
        assert!(
            !notices.iter().any(|n| n.rule_id == "RTS_019"),
            "same short + different long must not produce RTS_019"
        );
        let rts026: Vec<_> = notices.iter().filter(|n| n.rule_id == "RTS_026").collect();
        assert_eq!(
            rts026.len(),
            1,
            "same short with different long → one RTS_026 expected"
        );
        assert!(
            rts026[0].message.contains("R1") && rts026[0].message.contains("R2"),
            "RTS_026 message must list the entire group: {}",
            rts026[0].message
        );
        assert!(
            !notices.iter().any(|n| n.rule_id == "RTS_027"),
            "different long must not produce RTS_027"
        );
    }

    #[test]
    fn shared_long_name_different_short_produces_rts_027() {
        // Same route_long_name but DIFFERENT route_short_name → RTS_027 (Info); no RTS_019/026.
        let file = make_file(
            vec![
                "route_id",
                "route_short_name",
                "route_long_name",
                "route_type",
            ],
            vec![
                vec!["R1", "10", "Şehir Merkezi Hattı", "3"],
                vec!["R2", "20", "Şehir Merkezi Hattı", "3"],
            ],
        );
        let (_, notices) = validate_routes(&file);
        let rts027: Vec<_> = notices.iter().filter(|n| n.rule_id == "RTS_027").collect();
        assert_eq!(
            rts027.len(),
            1,
            "aynı long farklı short → tek RTS_027 beklenir"
        );
        assert!(
            rts027[0].message.contains("R1") && rts027[0].message.contains("R2"),
            "RTS_027 mesajı tüm grubu listelemeli: {}",
            rts027[0].message
        );
        assert!(
            !notices
                .iter()
                .any(|n| n.rule_id == "RTS_019" || n.rule_id == "RTS_026"),
            "aynı long farklı short durumda RTS_019/026 üretmemeli"
        );
    }

    #[test]
    fn shared_short_name_produces_single_rts_019() {
        let file = make_file(
            vec!["route_id", "route_short_name", "route_type"],
            vec![vec!["R1", "10", "3"], vec!["R2", "10", "3"]],
        );
        let (_, notices) = validate_routes(&file);
        assert_eq!(
            notices.iter().filter(|n| n.rule_id == "RTS_019").count(),
            1,
            "Aynı short_name → tek RTS_019"
        );
    }

    #[test]
    fn many_duplicate_route_groups_do_not_rescan_all_records() {
        // 20,000 separate duplicate groups, 40,000 routes total. If each group's anchor
        // record is found with `records.iter().find`, this grows to ~800 million comparisons;
        // the indexed implementation grows linearly with the route count. The generous time
        // allowance accommodates slow CI machines while still catching quadratic regressions.
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
            zip_entry_name: None,
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

    /// #93 authority matrix: the core enum range belongs to RTS_004 (Spec·KRİTİK), the
    /// extended range belongs to RTS_030 (Interop·DÜŞÜK), and a value in neither remains RTS_004.
    fn route_type_rule_ids(value: &str) -> Vec<String> {
        let file = make_file(
            vec!["route_id", "route_short_name", "route_type"],
            vec![vec!["R1", "Metro", value]],
        );
        let (_, notices) = validate_routes(&file);
        notices
            .iter()
            .filter(|n| n.rule_id == "RTS_004" || n.rule_id == "RTS_030")
            .map(|n| n.rule_id.clone())
            .collect()
    }

    #[test]
    fn core_route_types_produce_no_type_notice() {
        for v in ["0", "3", "7", "11", "12"] {
            assert!(
                route_type_rule_ids(v).is_empty(),
                "çekirdek route_type {v} hiçbir tip notice'ı üretmemeli, gelen: {:?}",
                route_type_rule_ids(v)
            );
        }
    }

    #[test]
    fn extended_route_type_is_interop_not_core_spec() {
        // 401 (old test), 100 (range start), 1501 (issue #93 counterexample), 1799 (range end).
        for v in ["100", "401", "715", "1501", "1799"] {
            assert_eq!(
                route_type_rule_ids(v),
                vec!["RTS_030".to_string()],
                "extended route_type {v} should produce only RTS_030 (NOT RTS_004)"
            );
        }
    }

    #[test]
    fn non_core_non_extended_route_type_stays_rts_004() {
        // Neither core nor extended: behavior is unchanged, a KRİTİK Spec violation.
        for v in ["8", "13", "99", "1800", "9999"] {
            assert_eq!(
                route_type_rule_ids(v),
                vec!["RTS_004".to_string()],
                "route_type {v} should produce RTS_004 as a core Spec violation"
            );
        }
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
