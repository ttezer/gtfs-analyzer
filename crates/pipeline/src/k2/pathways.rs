use gtfs_core::EntityType;

use super::common::{
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

    for (row_idx, row) in file.rows.iter().enumerate() {
        let line = (row_idx + 2) as u64;
        let row_map = build_row_map(&file.headers, row);
        let pathway_id = get_trimmed_field(&row_map, "pathway_id").unwrap_or("").to_string();
        let entity_id = (!pathway_id.is_empty()).then_some(pathway_id.clone());
        // PTH_020: pathway_id required (sütun yoksa ARC_025 devralır → atla)
        if get_trimmed_field(&row_map, "pathway_id") == Some("") {
            notices.push(make_k2_notice(
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
        if get_trimmed_field(&row_map, "pathway_mode") == Some("") {
            notices.push(make_k2_notice(&mut counter, "PTH_023", EntityType::Pathway, entity_id.clone(), Some(&row_map), &file.name, Some(line), Some("pathway_mode"), Some(String::new()), None, "pathway_mode zorunludur.".to_string(), "pathway_mode değerini 1-7 arasında girin."));
        }
        if get_trimmed_field(&row_map, "is_bidirectional") == Some("") {
            notices.push(make_k2_notice(&mut counter, "PTH_024", EntityType::Pathway, entity_id.clone(), Some(&row_map), &file.name, Some(line), Some("is_bidirectional"), Some(String::new()), None, "is_bidirectional zorunludur.".to_string(), "is_bidirectional değerini 0 veya 1 girin."));
        }

        let length = parse_nonnegative_f64(
            &row_map, &mut notices, &mut counter, "PTH_006", "length", &entity_id, line, &file.name
        );
        let traversal_time = parse_positive_u32(
            &row_map, &mut notices, &mut counter, "PTH_007", "traversal_time", &entity_id, line, &file.name
        );
        let min_width = parse_positive_f64(
            &row_map, &mut notices, &mut counter, "PTH_010", "min_width", &entity_id, line, &file.name
        );

        let from_stop_id = get_trimmed_field(&row_map, "from_stop_id").unwrap_or("");
        let to_stop_id = get_trimmed_field(&row_map, "to_stop_id").unwrap_or("");
        if get_trimmed_field(&row_map, "from_stop_id") == Some("") {
            notices.push(make_k2_notice(&mut counter, "PTH_021", EntityType::Pathway, entity_id.clone(), Some(&row_map), &file.name, Some(line), Some("from_stop_id"), Some(String::new()), None, "from_stop_id zorunludur.".to_string(), "Geçidin başlangıç durağını girin."));
        }
        if get_trimmed_field(&row_map, "to_stop_id") == Some("") {
            notices.push(make_k2_notice(&mut counter, "PTH_022", EntityType::Pathway, entity_id.clone(), Some(&row_map), &file.name, Some(line), Some("to_stop_id"), Some(String::new()), None, "to_stop_id zorunludur.".to_string(), "Geçidin bitiş durağını girin."));
        }
        if !from_stop_id.is_empty() && from_stop_id == to_stop_id {
            notices.push(make_k2_notice(
                &mut counter,
                "PTH_011",
                EntityType::Pathway,
                entity_id.clone(),
                Some(&row_map),
                &file.name,
                Some(line),
                None,
                Some(from_stop_id.to_string()),
                None,
                "from_stop_id ve to_stop_id aynı.".to_string(),
                "Her pathway satırında iki farklı durak belirtin.",
            ));
        }

        if matches!(pathway_mode, Some(7)) && matches!(is_bidirectional, Some(1)) {
            notices.push(make_k2_notice(
                &mut counter,
                "PTH_016",
                EntityType::Pathway,
                entity_id.clone(),
                Some(&row_map),
                &file.name,
                Some(line),
                None,
                Some("pathway_mode=7, is_bidirectional=1".to_string()),
                Some("is_bidirectional=0".to_string()),
                "Çıkış kapısı geçitleri çift yönlü olamaz.".to_string(),
                "pathway_mode=7 için is_bidirectional değerini 0 olarak ayarlayın.",
            ));
        }

        let max_slope = match parse_f64(&row_map, "max_slope") {
            Ok(v) => v,
            Err(err) => {
                notices.push(make_k2_notice(
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
        if max_slope.is_some() && !matches!(pathway_mode, Some(1 | 3)) {
            notices.push(make_k2_notice(
                &mut counter,
                "PTH_017",
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

        let stair_count = match parse_i32(&row_map, "stair_count") {
            Ok(v) => v,
            Err(_) => None,
        };

        // PTH_008: merdiven geçidinde stair_count belirtilmemiş
        if matches!(pathway_mode, Some(2)) && stair_count.is_none() {
            notices.push(make_k2_notice(
                &mut counter, "PTH_008", EntityType::Pathway,
                entity_id.clone(), Some(&row_map), &file.name, Some(line),
                Some("stair_count"), None, None,
                "pathway_mode=2 (merdiven) için stair_count belirtilmemiş.".to_string(),
                "stair_count alanını doldurarak kat farkını belirtin.",
            ));
        }

        // PTH_009: yürüme yolunda max_slope belirtilmemiş — feed-seviyesi özetle (aşağıda tek emit)
        if matches!(pathway_mode, Some(1)) && max_slope.is_none() {
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
                notices.push(make_k2_notice(
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
        let mut d = std::collections::HashMap::new();
        d.insert("affected_pathways".to_string(), pth009_count.to_string());
        if !pth009_examples.is_empty() {
            d.insert("example_pathways".to_string(), pth009_examples.join(", "));
        }
        n.details = Some(d);
        notices.push(n);
    }

    (records, notices)
}

fn parse_enum_u32(
    row_map: &RowMap,
    notices: &mut Vec<gtfs_core::Notice>,
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
                    notices.push(make_k2_notice(counter, rule_id, EntityType::Pathway, entity_id.clone(), Some(row_map), file_name, Some(line), Some(field), Some(v.to_string()), Some(allowed_str.clone()), format!("{field} geçerli bir değer değil."), &remediation));
                }
            }
            value
        }
        Err(err) => {
            let allowed_str = allowed.join(", ");
            let remediation = format!("{field} için şu değerlerden birini kullanın: {allowed_str}.");
            notices.push(make_k2_notice(counter, rule_id, EntityType::Pathway, entity_id.clone(), Some(row_map), file_name, Some(line), Some(field), get_trimmed_field(row_map, field).map(str::to_string), Some(allowed_str.clone()), err, &remediation));
            None
        }
    }
}

fn parse_nonnegative_f64(
    row_map: &RowMap,
    notices: &mut Vec<gtfs_core::Notice>,
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
                    notices.push(make_k2_notice(counter, rule_id, EntityType::Pathway, entity_id.clone(), Some(row_map), file_name, Some(line), Some(field), Some(v.to_string()), Some(">= 0".to_string()), format!("{field} alanı negatif olamaz."), "Alanı sıfır veya pozitif bir değere ayarlayın."));
                }
            }
            value
        }
        Err(err) => {
            notices.push(make_k2_notice(counter, rule_id, EntityType::Pathway, entity_id.clone(), Some(row_map), file_name, Some(line), Some(field), get_trimmed_field(row_map, field).map(str::to_string), None, err, "Alanı geçerli bir sayısal değere ayarlayın."));
            None
        }
    }
}

fn parse_positive_f64(
    row_map: &RowMap,
    notices: &mut Vec<gtfs_core::Notice>,
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
                    notices.push(make_k2_notice(counter, rule_id, EntityType::Pathway, entity_id.clone(), Some(row_map), file_name, Some(line), Some(field), Some(v.to_string()), Some("> 0".to_string()), format!("{field} alanı pozitif olmalıdır."), "Alanı pozitif bir değere ayarlayın."));
                }
            }
            value
        }
        Err(err) => {
            notices.push(make_k2_notice(counter, rule_id, EntityType::Pathway, entity_id.clone(), Some(row_map), file_name, Some(line), Some(field), get_trimmed_field(row_map, field).map(str::to_string), None, err, "Alanı geçerli bir sayısal değere ayarlayın."));
            None
        }
    }
}

fn parse_positive_u32(
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
        Ok(value) => {
            if let Some(v) = value {
                if v == 0 {
                    notices.push(make_k2_notice(counter, rule_id, EntityType::Pathway, entity_id.clone(), Some(row_map), file_name, Some(line), Some(field), Some(v.to_string()), Some("> 0".to_string()), format!("{field} alanı pozitif olmalıdır."), "Alanı pozitif bir tam sayıya ayarlayın."));
                }
            }
            value
        }
        Err(err) => {
            notices.push(make_k2_notice(counter, rule_id, EntityType::Pathway, entity_id.clone(), Some(row_map), file_name, Some(line), Some(field), get_trimmed_field(row_map, field).map(str::to_string), None, err, "Alanı geçerli bir tam sayıya ayarlayın."));
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
}
