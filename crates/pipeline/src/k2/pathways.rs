use gtfs_core::EntityType;

use super::common::{get_raw_field,
    build_row_map, get_trimmed_field, make_k2_notice, parse_f64, parse_i32, parse_u32,
    validate_enum, RowMap,
};
use crate::k1_parse::RawFile;

#[derive(Debug, Clone)]
pub struct PathwayRecord {
    pub pathway_id: String,
    pub from_stop_id: String,
    pub to_stop_id: String,
    pub pathway_mode: Option<u32>,
    pub is_bidirectional: Option<u32>,
    pub length: Option<f64>,
    pub traversal_time: Option<u32>,
    pub stair_count: Option<i32>,
    pub max_slope: Option<f64>,
    pub min_width: Option<f64>,
    pub row: RowMap,
    pub line: u64,
}

pub fn validate_pathways(file: &RawFile) -> (Vec<PathwayRecord>, Vec<gtfs_core::Notice>) {
    let mut notices = Vec::new();
    let mut records = Vec::new();
    let mut counter = 0;
    // PTH_009 (#15 Mode A): pathway başına emit büyük feed'de yüz binlerce notice üretiyordu
    // (tek tip opsiyonel-alan eksikliği, aksiyon hepsinde aynı). Feed-seviyesi TEK özete indir
    // (STM_050 deseni): gerçek sayı + ilk örnekler details'te; loop sonunda tek emit.
    let mut pth009_count: u32 = 0;
    let mut pth009_examples: Vec<String> = Vec::new();
    // PTH_008 (#15): PTH_009 ikizi (merdiven geçidinde stair_count eksik) — aynı feed-aggregate.
    let mut pth008_count: u32 = 0;
    let mut pth008_examples: Vec<String> = Vec::new();
    let mut pth025_count: u32 = 0;
    let mut pth025_examples: Vec<String> = Vec::new();
    let mut pth029_count: u32 = 0;
    let mut pth029_examples: Vec<String> = Vec::new();

    for (row_idx, row) in file.rows.iter().enumerate() {
        let line = (row_idx + 2) as u64;
        let row_map = build_row_map(&file.headers, row);
        let pathway_id = get_raw_field(&row_map, "pathway_id").unwrap_or("").to_string();
        let entity_id = (!pathway_id.is_empty()).then_some(pathway_id.clone());
        // PTH_020: pathway_id required (sütun yoksa ARC_025 devralır → atla)
        if get_raw_field(&row_map, "pathway_id").map(str::trim) == Some("") {
            crate::notice_budget::push(&mut notices, make_k2_notice(
                &mut counter, "PTH_020", EntityType::Pathway, None, Some(&row_map),
                &file.name, Some(line), Some("pathway_id"), Some(String::new()), None,
                "pathway_id zorunludur.".to_string(), "Her geçide benzersiz bir pathway_id verin.",
            ));
        }

        let pathway_mode = parse_enum_u32(
            &row_map, &mut notices, &mut counter, "PTH_004", "pathway_mode", &["1","2","3","4","5","6","7"], &entity_id, line, &file.name
        );
        let is_bidirectional = parse_enum_u32(
            &row_map, &mut notices, &mut counter, "PTH_005", "is_bidirectional", &["0","1"], &entity_id, line, &file.name
        );
        // Strict raw parsing above deliberately keeps lexical whitespace visible. For
        // cross-field semantics, a trim-valid value must still participate; otherwise a
        // whitespace-derived parse failure could hide an independent PTH_016/PTH_028/PTH_008
        // or PTH_009 finding.
        let pathway_mode_semantic = get_trimmed_field(&row_map, "pathway_mode")
            .and_then(|value| value.parse::<u32>().ok());
        let is_bidirectional_semantic = get_trimmed_field(&row_map, "is_bidirectional")
            .and_then(|value| value.parse::<u32>().ok());
        if get_trimmed_field(&row_map, "pathway_mode") == Some("") {
            crate::notice_budget::push(&mut notices, make_k2_notice(&mut counter, "PTH_023", EntityType::Pathway, entity_id.clone(), Some(&row_map), &file.name, Some(line), Some("pathway_mode"), Some(String::new()), None, "pathway_mode zorunludur.".to_string(), "pathway_mode değerini 1-7 arasında girin."));
        }
        if get_trimmed_field(&row_map, "is_bidirectional") == Some("") {
            crate::notice_budget::push(&mut notices, make_k2_notice(&mut counter, "PTH_024", EntityType::Pathway, entity_id.clone(), Some(&row_map), &file.name, Some(line), Some("is_bidirectional"), Some(String::new()), None, "is_bidirectional zorunludur.".to_string(), "is_bidirectional değerini 0 veya 1 girin."));
        }

        let length = parse_nonnegative_f64(
            &row_map, &mut notices, &mut counter, "PTH_006", "length", &entity_id, line, &file.name
        );
        if matches!(pathway_mode_semantic, Some(1 | 6 | 7)) && length.is_none()
            && get_trimmed_field(&row_map, "length").is_none_or(str::is_empty)
        {
            pth025_count += 1;
            if pth025_examples.len() < 5 { if let Some(id) = &entity_id { pth025_examples.push(id.clone()); } }
        }
        let traversal_time = parse_positive_u32(
            &row_map, &mut notices, &mut counter, "PTH_007", "traversal_time", &entity_id, line, &file.name
        );
        // PTH_029: spec traversal_time'ı yürüyen bant (3), yürüyen merdiven (4) ve asansör (5)
        // için ÖNERİR. PTH_007 yukarıda değerin geçerliliğini ölçer; bu kural eksikliğini.
        // PTH_025 (length) ile birebir aynı desen — koşul yalnız pathway_mode kümesinde ayrılır.
        if matches!(pathway_mode_semantic, Some(3..=5)) && traversal_time.is_none()
            && get_trimmed_field(&row_map, "traversal_time").is_none_or(str::is_empty)
        {
            pth029_count += 1;
            if pth029_examples.len() < 5 { if let Some(id) = &entity_id { pth029_examples.push(id.clone()); } }
        }
        let min_width = parse_positive_f64(
            &row_map, &mut notices, &mut counter, "PTH_010", "min_width", &entity_id, line, &file.name
        );

        let from_stop_id = get_raw_field(&row_map, "from_stop_id").unwrap_or("");
        let to_stop_id = get_raw_field(&row_map, "to_stop_id").unwrap_or("");
        if get_raw_field(&row_map, "from_stop_id").map(str::trim) == Some("") {
            crate::notice_budget::push(&mut notices, make_k2_notice(&mut counter, "PTH_021", EntityType::Pathway, entity_id.clone(), Some(&row_map), &file.name, Some(line), Some("from_stop_id"), Some(String::new()), None, "from_stop_id zorunludur.".to_string(), "Geçidin başlangıç durağını girin."));
        }
        if get_raw_field(&row_map, "to_stop_id").map(str::trim) == Some("") {
            crate::notice_budget::push(&mut notices, make_k2_notice(&mut counter, "PTH_022", EntityType::Pathway, entity_id.clone(), Some(&row_map), &file.name, Some(line), Some("to_stop_id"), Some(String::new()), None, "to_stop_id zorunludur.".to_string(), "Geçidin bitiş durağını girin."));
        }
        if !from_stop_id.is_empty() && from_stop_id == to_stop_id {
            crate::notice_budget::push(&mut notices, make_k2_notice(
                &mut counter,
                "PTH_011",
                EntityType::Pathway,
                entity_id.clone(),
                Some(&row_map),
                &file.name,
                Some(line),
                // İhlal iki uç noktanın AYNI olmasıdır.
                Some("from_stop_id|to_stop_id"),
                Some(from_stop_id.to_string()),
                None,
                "from_stop_id ve to_stop_id aynı.".to_string(),
                "Her pathway satırında iki farklı durak belirtin.",
            ));
        }

        if matches!(pathway_mode_semantic, Some(7)) && matches!(is_bidirectional_semantic, Some(1)) {
            crate::notice_budget::push(&mut notices, make_k2_notice(
                &mut counter,
                "PTH_016",
                EntityType::Pathway,
                entity_id.clone(),
                Some(&row_map),
                &file.name,
                Some(line),
                // İhlal iki alanın BİRLİKTE aldığı değerdedir; `observed_value` bunu zaten
                // yazıyordu, `field` yazmıyordu.
                Some("pathway_mode|is_bidirectional"),
                Some("pathway_mode=7, is_bidirectional=1".to_string()),
                Some("is_bidirectional=0".to_string()),
                "Çıkış kapısı geçitleri çift yönlü olamaz.".to_string(),
                "pathway_mode=7 için is_bidirectional değerini 0 olarak ayarlayın.",
            ));
        }

        let max_slope = match parse_f64(&row_map, "max_slope") {
            Ok(v) => v,
            Err(err) => {
                crate::notice_budget::push(&mut notices, make_k2_notice(
                    &mut counter,
                    "PTH_017",
                    EntityType::Pathway,
                    entity_id.clone(),
                    Some(&row_map),
                    &file.name,
                    Some(line),
                    Some("max_slope"),
                    get_trimmed_field(&row_map, "max_slope").map(str::to_string),
                    None,
                    err,
                    "max_slope için geçerli bir sayısal değer girin.",
                ));
                None
            }
        };
        // PTH_028 (PTH_017'den ayrıldı): spec burada "should" der ve Presence sütunu düz
        // `Optional`'dır — gerçek yasaklarını 14 alanda `Conditionally Forbidden` yazarak
        // ifade eden bir belge, burada bilinçli olarak yazmamıştır. Tavsiye → Quality.
        if max_slope.is_some() && !matches!(pathway_mode_semantic, Some(1 | 3)) {
            crate::notice_budget::push(&mut notices, make_k2_notice(
                &mut counter,
                "PTH_028",
                EntityType::Pathway,
                entity_id.clone(),
                Some(&row_map),
                &file.name,
                Some(line),
                Some("max_slope"),
                max_slope.map(|v| v.to_string()),
                Some("pathway_mode 1 veya 3".to_string()),
                "max_slope yalnızca pathway_mode 1 veya 3 için geçerlidir.".to_string(),
                "max_slope yalnızca yürüme yolu veya hareketli yürüme yolu geçitlerinde kullanın.",
            ));
        }

        // PTH_027: spec tipi `Non-null integer`. İki ihlal de aynı olgudur:
        //   (a) tam sayı değil — eskiden `Err(_) => None` ile SESSİZCE düşüyordu,
        //   (b) sıfır — tipin dışladığı tek değer; yön bilgisi taşımaz.
        let stair_count = match parse_i32(&row_map, "stair_count") {
            Ok(Some(0)) => {
                crate::notice_budget::push(&mut notices, make_k2_notice(
                    &mut counter, "PTH_027", EntityType::Pathway, entity_id.clone(),
                    Some(&row_map), &file.name, Some(line), Some("stair_count"),
                    Some("0".to_string()), Some("0 dışında tam sayı".to_string()),
                    "stair_count sıfır olamaz — pozitif değer yukarı, negatif aşağı yönü gösterir.".to_string(),
                    "stair_count değerini gerçek basamak sayısıyla (yukarı için pozitif, aşağı için negatif) değiştirin ya da alanı boş bırakın.",
                ));
                None
            }
            Ok(v) => v,
            Err(err) => {
                crate::notice_budget::push(&mut notices, make_k2_notice(
                    &mut counter, "PTH_027", EntityType::Pathway, entity_id.clone(),
                    Some(&row_map), &file.name, Some(line), Some("stair_count"),
                    get_trimmed_field(&row_map, "stair_count").map(str::to_string),
                    Some("0 dışında tam sayı".to_string()), err,
                    "stair_count değerini tam sayı olarak girin (yukarı için pozitif, aşağı için negatif).",
                ));
                None
            }
        };

        // PTH_008: merdiven geçidinde stair_count belirtilmemiş — feed-seviyesi özetle (aşağıda tek emit)
        if matches!(pathway_mode_semantic, Some(2)) && stair_count.is_none() {
            pth008_count += 1;
            if pth008_examples.len() < 5 {
                if let Some(id) = &entity_id {
                    pth008_examples.push(id.clone());
                }
            }
        }

        // PTH_009: yürüme yolunda max_slope belirtilmemiş — feed-seviyesi özetle (aşağıda tek emit)
        if matches!(pathway_mode_semantic, Some(1)) && max_slope.is_none() {
            let raw = get_trimmed_field(&row_map, "max_slope");
            if raw.map(|s| s.is_empty()).unwrap_or(true) {
                pth009_count += 1;
                if pth009_examples.len() < 5 {
                    if let Some(id) = &entity_id {
                        pth009_examples.push(id.clone());
                    }
                }
            }
        }

        // PTH_018: signposted_as çok uzun (> 255 karakter)
        if let Some(sign) = get_trimmed_field(&row_map, "signposted_as") {
            if sign.len() > 255 {
                crate::notice_budget::push(&mut notices, make_k2_notice(
                    &mut counter, "PTH_018", EntityType::Pathway,
                    entity_id.clone(), Some(&row_map), &file.name, Some(line),
                    Some("signposted_as"),
                    Some(format!("{} karakter", sign.len())),
                    Some("≤ 255 karakter".to_string()),
                    format!("signposted_as alanı {} karakter uzunluğunda; önerilen maksimum 255.", sign.len()),
                    "signposted_as değerini 255 karakterin altına kısaltın.",
                ));
            }
        }

        records.push(PathwayRecord {
            pathway_id,
            from_stop_id: from_stop_id.to_string(),
            to_stop_id: to_stop_id.to_string(),
            pathway_mode,
            is_bidirectional,
            length,
            traversal_time,
            stair_count,
            max_slope,
            min_width,
            row: row_map,
            line,
        });
    }

    // PTH_009 feed-seviyesi tek özet. Tek notice → capped_totals etkilenmez; gerçek sayı
    // details.affected_pathways'te (UI "1 gösterilen, N etkilenen" diyebilir). EntityType::Feed
    // + scope None (registry Feed dedup). field=max_slope, file=pathways.txt korunur.
    if pth009_count > 0 {
        let mut n = make_k2_notice(
            &mut counter, "PTH_009", EntityType::Feed, None, None,
            &file.name, None, Some("max_slope"),
            Some(pth009_count.to_string()), None,
            format!("{pth009_count} yürüme yolu geçidinde (pathway_mode=1) max_slope eksik."),
            "İlgili pathways.txt kayıtlarına max_slope ekleyin veya veri kaynağını düzeltin.",
        );
        let mut d = std::collections::BTreeMap::new();
        d.insert("affected_pathways".to_string(), pth009_count.to_string());
        if !pth009_examples.is_empty() {
            d.insert("example_pathways".to_string(), pth009_examples.join(", "));
        }
        n.details = Some(d);
        crate::notice_budget::push(&mut notices, n);
    }

    // PTH_008 feed-seviyesi tek özet (PTH_009 ile aynı desen).
    if pth008_count > 0 {
        let mut n = make_k2_notice(
            &mut counter, "PTH_008", EntityType::Feed, None, None,
            &file.name, None, Some("stair_count"),
            Some(pth008_count.to_string()), None,
            format!("{pth008_count} merdiven geçidinde (pathway_mode=2) stair_count eksik."),
            "İlgili pathways.txt kayıtlarına stair_count ekleyin veya veri kaynağını düzeltin.",
        );
        let mut d = std::collections::BTreeMap::new();
        d.insert("affected_pathways".to_string(), pth008_count.to_string());
        if !pth008_examples.is_empty() {
            d.insert("example_pathways".to_string(), pth008_examples.join(", "));
        }
        n.details = Some(d);
        crate::notice_budget::push(&mut notices, n);
    }
    if pth029_count > 0 {
        let mut n = make_k2_notice(&mut counter, "PTH_029", EntityType::Feed, None, None,
            &file.name, None, Some("traversal_time"), Some(pth029_count.to_string()), None,
            format!("{pth029_count} yürüyen bant/yürüyen merdiven/asansör kaydında önerilen traversal_time eksik."),
            "pathway_mode 3, 4 veya 5 olan kayıtlara saniye cinsinden traversal_time ekleyin.");
        let mut d = std::collections::BTreeMap::new();
        d.insert("affected_pathways".to_string(), pth029_count.to_string());
        if !pth029_examples.is_empty() { d.insert("example_pathways".to_string(), pth029_examples.join(", ")); }
        n.details = Some(d); crate::notice_budget::push(&mut notices, n);
    }
    if pth025_count > 0 {
        let mut n = make_k2_notice(&mut counter, "PTH_025", EntityType::Feed, None, None,
            &file.name, None, Some("length"), Some(pth025_count.to_string()), None,
            format!("{pth025_count} walkway/fare gate/exit gate kaydında önerilen length eksik."),
            "pathway_mode 1, 6 veya 7 olan kayıtlara metre cinsinden length ekleyin.");
        let mut d = std::collections::BTreeMap::new();
        d.insert("affected_pathways".to_string(), pth025_count.to_string());
        if !pth025_examples.is_empty() { d.insert("example_pathways".to_string(), pth025_examples.join(", ")); }
        n.details = Some(d); crate::notice_budget::push(&mut notices, n);
    }

    (records, notices)
}

// Pathway alanlarının parse ve notice bağlamı bilinçli olarak ayrı parametrelerdir;
// ortak bir parametre struct'ı bu dört küçük validator'da gereksiz durum taşır.
#[allow(clippy::too_many_arguments)]
fn parse_enum_u32(
    row_map: &RowMap,
    mut notices: &mut Vec<gtfs_core::Notice>,
    counter: &mut u32,
    rule_id: &str,
    field: &str,
    allowed: &[&str],
    entity_id: &Option<String>,
    line: u64,
    file_name: &str,
) -> Option<u32> {
    match parse_u32(row_map, field) {
        Ok(value) => {
            if let Some(v) = value {
                if !validate_enum(&v.to_string(), allowed) {
                    let allowed_str = allowed.join(", ");
                    let remediation = format!("{field} için şu değerlerden birini kullanın: {allowed_str}.");
                    crate::notice_budget::push(&mut notices, make_k2_notice(counter, rule_id, EntityType::Pathway, entity_id.clone(), Some(row_map), file_name, Some(line), Some(field), Some(v.to_string()), Some(allowed_str.clone()), format!("{field} geçerli bir değer değil."), &remediation));
                }
            }
            value
        }
        Err(err) => {
            let allowed_str = allowed.join(", ");
            let remediation = format!("{field} için şu değerlerden birini kullanın: {allowed_str}.");
            crate::notice_budget::push(&mut notices, make_k2_notice(counter, rule_id, EntityType::Pathway, entity_id.clone(), Some(row_map), file_name, Some(line), Some(field), get_trimmed_field(row_map, field).map(str::to_string), Some(allowed_str.clone()), err, &remediation));
            None
        }
    }
}

// Her numeric helper kendi alan/mesaj kuralını açıkça taşır; parametre struct'ı
// çağrıları kısaltmak yerine rule bağlamını gizler.
#[allow(clippy::too_many_arguments)]
fn parse_nonnegative_f64(
    row_map: &RowMap,
    mut notices: &mut Vec<gtfs_core::Notice>,
    counter: &mut u32,
    rule_id: &str,
    field: &str,
    entity_id: &Option<String>,
    line: u64,
    file_name: &str,
) -> Option<f64> {
    match parse_f64(row_map, field) {
        Ok(value) => {
            if let Some(v) = value {
                if v < 0.0 {
                    crate::notice_budget::push(&mut notices, make_k2_notice(counter, rule_id, EntityType::Pathway, entity_id.clone(), Some(row_map), file_name, Some(line), Some(field), Some(v.to_string()), Some(">= 0".to_string()), format!("{field} alanı negatif olamaz."), "Alanı sıfır veya pozitif bir değere ayarlayın."));
                }
            }
            value
        }
        Err(err) => {
            crate::notice_budget::push(&mut notices, make_k2_notice(counter, rule_id, EntityType::Pathway, entity_id.clone(), Some(row_map), file_name, Some(line), Some(field), get_trimmed_field(row_map, field).map(str::to_string), None, err, "Alanı geçerli bir sayısal değere ayarlayın."));
            None
        }
    }
}

// Her numeric helper kendi alan/mesaj kuralını açıkça taşır; parametre struct'ı
// çağrıları kısaltmak yerine rule bağlamını gizler.
#[allow(clippy::too_many_arguments)]
fn parse_positive_f64(
    row_map: &RowMap,
    mut notices: &mut Vec<gtfs_core::Notice>,
    counter: &mut u32,
    rule_id: &str,
    field: &str,
    entity_id: &Option<String>,
    line: u64,
    file_name: &str,
) -> Option<f64> {
    match parse_f64(row_map, field) {
        Ok(value) => {
            if let Some(v) = value {
                if v <= 0.0 {
                    crate::notice_budget::push(&mut notices, make_k2_notice(counter, rule_id, EntityType::Pathway, entity_id.clone(), Some(row_map), file_name, Some(line), Some(field), Some(v.to_string()), Some("> 0".to_string()), format!("{field} alanı pozitif olmalıdır."), "Alanı pozitif bir değere ayarlayın."));
                }
            }
            value
        }
        Err(err) => {
            crate::notice_budget::push(&mut notices, make_k2_notice(counter, rule_id, EntityType::Pathway, entity_id.clone(), Some(row_map), file_name, Some(line), Some(field), get_trimmed_field(row_map, field).map(str::to_string), None, err, "Alanı geçerli bir sayısal değere ayarlayın."));
            None
        }
    }
}

// Her numeric helper kendi alan/mesaj kuralını açıkça taşır; parametre struct'ı
// çağrıları kısaltmak yerine rule bağlamını gizler.
#[allow(clippy::too_many_arguments)]
fn parse_positive_u32(
    row_map: &RowMap,
    mut notices: &mut Vec<gtfs_core::Notice>,
    counter: &mut u32,
    rule_id: &str,
    field: &str,
    entity_id: &Option<String>,
    line: u64,
    file_name: &str,
) -> Option<u32> {
    match parse_u32(row_map, field) {
        Ok(value) => {
            if let Some(v) = value {
                if v == 0 {
                    crate::notice_budget::push(&mut notices, make_k2_notice(counter, rule_id, EntityType::Pathway, entity_id.clone(), Some(row_map), file_name, Some(line), Some(field), Some(v.to_string()), Some("> 0".to_string()), format!("{field} alanı pozitif olmalıdır."), "Alanı pozitif bir tam sayıya ayarlayın."));
                }
            }
            value
        }
        Err(err) => {
            crate::notice_budget::push(&mut notices, make_k2_notice(counter, rule_id, EntityType::Pathway, entity_id.clone(), Some(row_map), file_name, Some(line), Some(field), get_trimmed_field(row_map, field).map(str::to_string), None, err, "Alanı geçerli bir tam sayıya ayarlayın."));
            None
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use smol_str::SmolStr;

    fn pathways_file(rows: Vec<Vec<&str>>) -> RawFile {
        let headers = ["pathway_id", "from_stop_id", "to_stop_id", "pathway_mode", "is_bidirectional", "max_slope"];
        RawFile {
            name: "pathways.txt".to_string(),
            headers: headers.iter().map(|s| s.to_string()).collect(),
            rows: rows.into_iter().map(|r| r.into_iter().map(SmolStr::from).collect()).collect(),
            bytes: 0,
            raw_text: None,
        }
    }

    #[test]
    fn pth_009_aggregates_to_single_feed_notice_with_count() {
        // #15: 3 yürüme yolu (mode=1) max_slope boş → TEK PTH_009 (feed-seviyesi),
        // affected_pathways=3. mode=2 olan pathway sayılmaz.
        let file = pathways_file(vec![
            vec!["P1", "S1", "S2", "1", "0", ""],
            vec!["P2", "S2", "S3", "1", "0", ""],
            vec!["P3", "S3", "S4", "1", "0", ""],
            vec!["P4", "S4", "S5", "2", "0", ""], // merdiven → PTH_009 sayılmaz
        ]);
        let (_, notices) = validate_pathways(&file);
        let pth009: Vec<_> = notices.iter().filter(|n| n.rule_id == "PTH_009").collect();
        assert_eq!(pth009.len(), 1, "PTH_009 feed-seviyesi tek notice olmalı");
        let n = pth009[0];
        assert_eq!(n.entity_type, EntityType::Feed);
        assert_eq!(n.field.as_deref(), Some("max_slope"));
        assert_eq!(
            n.details.as_ref().and_then(|d| d.get("affected_pathways")).map(String::as_str),
            Some("3"),
        );
    }

    #[test]
    fn pth_008_aggregates_to_single_feed_notice_with_count() {
        // #15: 2 merdiven (mode=2) stair_count boş → TEK PTH_008 (feed-seviyesi), affected=2.
        let file = pathways_file(vec![
            vec!["P1", "S1", "S2", "2", "0", ""],
            vec!["P2", "S2", "S3", "2", "0", ""],
            vec!["P3", "S3", "S4", "1", "0", ""], // yürüme yolu → PTH_008 sayılmaz
        ]);
        let (_, notices) = validate_pathways(&file);
        let pth008: Vec<_> = notices.iter().filter(|n| n.rule_id == "PTH_008").collect();
        assert_eq!(pth008.len(), 1, "PTH_008 feed-seviyesi tek notice olmalı");
        let n = pth008[0];
        assert_eq!(n.entity_type, EntityType::Feed);
        assert_eq!(
            n.details.as_ref().and_then(|d| d.get("affected_pathways")).map(String::as_str),
            Some("2"),
        );
    }
    #[test]
    fn pth_025_counts_recommended_modes_without_length() {
        let headers = ["pathway_id", "from_stop_id", "to_stop_id", "pathway_mode", "is_bidirectional", "length"];
        let file = RawFile { name: "pathways.txt".into(), headers: headers.iter().map(|s| s.to_string()).collect(),
            rows: [["P1","S1","S2","1","0",""],["P2","S1","S2","6","0",""],["P3","S1","S2","7","0","12"],["P4","S1","S2","2","0",""]]
                .into_iter().map(|r| r.into_iter().map(SmolStr::from).collect()).collect(), bytes: 0, raw_text: None };
        let (_, notices) = validate_pathways(&file);
        let n = notices.iter().find(|n| n.rule_id == "PTH_025").unwrap();
        assert_eq!(n.details.as_ref().unwrap().get("affected_pathways").map(String::as_str), Some("2"));
    }
    #[test]
    fn pth_029_flags_missing_traversal_time_for_moving_modes() {
        let headers = ["pathway_id", "from_stop_id", "to_stop_id", "pathway_mode", "is_bidirectional", "traversal_time"];
        let file = RawFile { name: "pathways.txt".into(), headers: headers.iter().map(|s| s.to_string()).collect(),
            rows: [["P1","S1","S2","3","0",""],   // yürüyen bant → sayılır
                   ["P2","S2","S3","4","0",""],   // yürüyen merdiven → sayılır
                   ["P3","S3","S4","5","0",""],   // asansör → sayılır
                   ["P4","S4","S5","1","0",""],   // yürüme yolu → PTH_029 DEĞİL (PTH_025'in alanı)
                   ["P5","S5","S6","4","0","30"]] // dolu → sayılmaz
                .into_iter().map(|r| r.into_iter().map(SmolStr::from).collect()).collect(), bytes: 0, raw_text: None };
        let (_, notices) = validate_pathways(&file);
        let hits: Vec<_> = notices.iter().filter(|n| n.rule_id == "PTH_029").collect();
        assert_eq!(hits.len(), 1, "feed başına tek özet bekleniyor: {hits:?}");
        assert_eq!(hits[0].observed_value.as_deref(), Some("3"), "üç kayıt sayılmalı");
        assert_eq!(hits[0].field.as_deref(), Some("traversal_time"));
    }

    #[test]
    fn pth_029_silent_when_all_moving_modes_have_traversal_time() {
        let headers = ["pathway_id", "from_stop_id", "to_stop_id", "pathway_mode", "is_bidirectional", "traversal_time"];
        let file = RawFile { name: "pathways.txt".into(), headers: headers.iter().map(|s| s.to_string()).collect(),
            rows: [["P1","S1","S2","3","0","20"],["P2","S2","S3","5","0","45"]]
                .into_iter().map(|r| r.into_iter().map(SmolStr::from).collect()).collect(), bytes: 0, raw_text: None };
        let (_, notices) = validate_pathways(&file);
        assert!(!notices.iter().any(|n| n.rule_id == "PTH_029"), "{notices:?}");
    }

}
