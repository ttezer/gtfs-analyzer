use gtfs_core::EntityType;

use super::common::{build_row_map, get_trimmed_field, looks_like_bcp47, make_k2_notice, RowMap};
use crate::k1_parse::RawFile;

const TRANSLATION_TABLES: &[&str] = &[
    "agency", "stops", "routes", "trips", "stop_times", "feed_info", "attributions", "pathways",
    "levels",
];

fn valid_fields_for_table(table: &str) -> &'static [&'static str] {
    match table {
        "agency" => &["agency_name", "agency_url", "agency_fare_url", "agency_email", "agency_phone"],
        "stops" => &["stop_name", "stop_desc", "stop_url", "tts_stop_name"],
        "routes" => &["route_short_name", "route_long_name", "route_desc", "route_url"],
        "trips" => &["trip_headsign", "trip_short_name"],
        "stop_times" => &["stop_headsign"],
        "feed_info" => &["feed_publisher_name"],
        "attributions" => &["organization_name", "attribution_url", "attribution_email", "attribution_phone"],
        "pathways" => &["signposted_as", "reversed_signposted_as"],
        "levels" => &["level_name"],
        _ => &[],
    }
}

#[derive(Debug, Clone)]
pub struct TranslationRecord {
    pub table_name: String,
    pub field_name: String,
    pub language: String,
    pub translation: String,
    pub record_id: Option<String>,
    pub record_sub_id: Option<String>,
    pub field_value: Option<String>,
    pub row: RowMap,
    pub line: u64,
}

pub fn validate_translations(
    file: &RawFile,
) -> (Vec<TranslationRecord>, Vec<gtfs_core::Notice>) {
    let mut notices = Vec::new();
    let mut records = Vec::new();
    let mut counter = 0;

    for (row_idx, row) in file.rows.iter().enumerate() {
        let line = (row_idx + 2) as u64;
        let row_map = build_row_map(&file.headers, row);

        let table_name = get_trimmed_field(&row_map, "table_name").unwrap_or("").to_string();
        if !TRANSLATION_TABLES.contains(&table_name.as_str()) {
            notices.push(make_k2_notice(
                &mut counter,
                "TRN_001",
                EntityType::Row,
                None,
                Some(&row_map),
                &file.name,
                Some(line),
                Some("table_name"),
                Some(table_name.clone()),
                None,
                "table_name desteklenen bir GTFS tablosu değil.".to_string(),
                "table_name için geçerli bir GTFS tablo adı kullanın.",
            ));
        }

        let field_name = get_trimmed_field(&row_map, "field_name").unwrap_or("").to_string();
        if !valid_fields_for_table(&table_name).contains(&field_name.as_str()) {
            notices.push(make_k2_notice(
                &mut counter,
                "TRN_002",
                EntityType::Row,
                None,
                Some(&row_map),
                &file.name,
                Some(line),
                Some("field_name"),
                Some(field_name.clone()),
                None,
                "field_name bu table_name için geçerli değil.".to_string(),
                "Seçilen table_name için çevirilebilir geçerli bir alan adı kullanın.",
            ));
        }

        let language = get_trimmed_field(&row_map, "language").unwrap_or("").to_string();
        if !looks_like_bcp47(&language) {
            notices.push(make_k2_notice(
                &mut counter,
                "TRN_003",
                EntityType::Row,
                None,
                Some(&row_map),
                &file.name,
                Some(line),
                Some("language"),
                Some(language.clone()),
                None,
                "language geçerli bir BCP 47 etiketi değil.".to_string(),
                "Geçerli bir IETF BCP 47 dil etiketi kullanın.",
            ));
        }

        let translation = get_trimmed_field(&row_map, "translation").unwrap_or("").to_string();
        if translation.is_empty() {
            notices.push(make_k2_notice(
                &mut counter,
                "TRN_008",
                EntityType::Row,
                None,
                Some(&row_map),
                &file.name,
                Some(line),
                Some("translation"),
                Some(String::new()),
                None,
                "translation değeri boş; çeviri içeriği sağlanmamış.".to_string(),
                "translation alanına çevrilmiş metni girin.",
            ));
        }

        let record_id = get_trimmed_field(&row_map, "record_id").filter(|v| !v.is_empty()).map(str::to_string);
        let record_sub_id = get_trimmed_field(&row_map, "record_sub_id").filter(|v| !v.is_empty()).map(str::to_string);
        let field_value = get_trimmed_field(&row_map, "field_value").filter(|v| !v.is_empty()).map(str::to_string);

        // TRN_015: spec `field_value` için "Required if record_id is empty" der. İkisi de
        // boşsa satır hangi kaydı çevirdiğini söylemez — çeviri hiçbir zaman uygulanamaz.
        //
        // İKİ DARALTMA:
        //  • `feed_info` hariç — orada üç alanın üçü de yasaktır (TRN_013).
        //  • table_name GEÇERSİZ ise hariç — o satırı TRN_001 zaten "desteklenen tablo değil"
        //    diye reddeder ve "hangi kaydı çeviriyor" sorusu anlamını yitirir. Ölçüldü
        //    (korpus, mdb-2519): eski Google `trans_id,lang,translation` biçimini kullanan bir
        //    feed'de bu daraltma olmadan TRN_015 101.872 bulgu üretiyordu — aynı satırlara
        //    zaten TRN_001/002/003/006/011 ateşliyor, yani ALTINCI kez aynı şeyi söylemek olurdu.
        let table_known = TRANSLATION_TABLES.contains(&table_name.as_str());
        if table_known && table_name != "feed_info" && record_id.is_none() && field_value.is_none() {
            notices.push(make_k2_notice(
                &mut counter,
                "TRN_015",
                EntityType::Row,
                None,
                Some(&row_map),
                &file.name,
                Some(line),
                Some("field_value"),
                Some(String::new()),
                None,
                "record_id ve field_value ikisi de boş; çevirinin hangi kayda ait olduğu belirsiz.".to_string(),
                "record_id veya field_value alanlarından birini doldurun.",
            ));
        }

        if record_id.is_some() && field_value.is_some() {
            notices.push(make_k2_notice(
                &mut counter,
                "TRN_009",
                EntityType::Row,
                None,
                Some(&row_map),
                &file.name,
                Some(line),
                // İhlal iki alanın BİRLİKTE dolu olmasıdır.
                Some("record_id|field_value"),
                None,
                None,
                "record_id ve field_value aynı anda kullanılamaz.".to_string(),
                "record_id veya field_value alanlarından yalnızca birini kullanın.",
            ));
        }

        // record_sub_id yalnızca record_id ile eşleştirme modunda zorunludur.
        // field_value modunda (record_id boş) record_sub_id yasaktır → FP üretme.
        if table_name == "stop_times" && record_id.is_some() {
            if record_sub_id.is_none() {
                notices.push(make_k2_notice(
                    &mut counter,
                    "TRN_010",
                    EntityType::Row,
                    record_id.clone(),
                    Some(&row_map),
                    &file.name,
                    Some(line),
                    Some("record_sub_id"),
                    Some(String::new()),
                    Some("stop_sequence".to_string()),
                    "stop_times çevirileri için record_sub_id zorunludur.".to_string(),
                    "record_sub_id değerini stop_sequence değerine ayarlayın.",
                ));
            }
        }

        let translatable = ["name", "desc", "url", "email", "phone", "headsign", "signposted_as"];
        if !translatable.iter().any(|needle| field_name.contains(needle)) {
            notices.push(make_k2_notice(
                &mut counter,
                "TRN_011",
                EntityType::Row,
                None,
                Some(&row_map),
                &file.name,
                Some(line),
                Some("field_name"),
                Some(field_name.clone()),
                None,
                "field_name çevrilebilir bir metin/url/e-posta/telefon alanına benzemiyor.".to_string(),
                "Yalnızca metin, URL, e-posta veya telefon içeriği için tasarlanmış alanları çevirin.",
            ));
        }

        if table_name == "feed_info" && (record_id.is_some() || record_sub_id.is_some() || field_value.is_some()) {
            notices.push(make_k2_notice(
                &mut counter,
                "TRN_013",
                EntityType::Row,
                None,
                Some(&row_map),
                &file.name,
                Some(line),
                // feed_info satırında üç kimlik alanının ÜÇÜ de yasaktır.
                Some("record_id|record_sub_id|field_value"),
                None,
                None,
                "feed_info çevirileri record_id, record_sub_id veya field_value kullanamaz.".to_string(),
                "feed_info satırları için record_id, record_sub_id ve field_value alanlarını boş bırakın.",
            ));
        }

        // TRN_017: TRN_014'ün ters kolu. `stop_times.txt`'in birincil anahtarı bileşiktir
        // (`trip_id` + `stop_sequence`); `record_id` yalnız ilkini taşır, `record_sub_id`
        // ikincisini. İkincisi olmadan çeviri hangi SATIRA ait olduğunu söyleyemez —
        // referans dangling değil BELİRSİZdir, bu yüzden `XFL_014` de görmez.
        if table_name == "stop_times"
            && record_id.as_deref().is_some_and(|r| !r.is_empty())
            && record_sub_id.as_deref().is_none_or(str::is_empty)
        {
            notices.push(make_k2_notice(
                &mut counter, "TRN_017", EntityType::Row, record_id.clone(),
                Some(&row_map), &file.name, Some(line), Some("record_sub_id"),
                None, Some("stop_sequence".to_string()),
                "stop_times çevirisinde record_id verilmiş ama record_sub_id eksik — çeviri hangi satıra ait olduğunu söylemiyor.".to_string(),
                "record_sub_id alanına ilgili stop_sequence değerini girin.",
            ));
        }

        if table_name != "stop_times" && record_sub_id.is_some() {
            notices.push(make_k2_notice(
                &mut counter,
                "TRN_014",
                EntityType::Row,
                record_id.clone(),
                Some(&row_map),
                &file.name,
                Some(line),
                Some("record_sub_id"),
                record_sub_id.clone(),
                None,
                "record_sub_id yalnızca stop_times çevirileri için kullanılabilir.".to_string(),
                "table_name=stop_times olmadıkça record_sub_id alanını boş bırakın.",
            ));
        }

        records.push(TranslationRecord {
            table_name,
            field_name,
            language,
            translation,
            record_id,
            record_sub_id,
            field_value,
            row: row_map,
            line,
        });
    }

    (records, notices)
}

#[cfg(test)]
mod tests {
    use super::*;
    use smol_str::SmolStr;

    fn make_file(headers: Vec<&str>, rows: Vec<Vec<&str>>) -> RawFile {
        RawFile {
            name: "translations.txt".to_string(),
            headers: headers.iter().map(|s| s.to_string()).collect(),
            rows: rows.into_iter().map(|r| r.into_iter().map(SmolStr::from).collect()).collect(),
            bytes: 0,
            raw_text: None,
        }
    }

    #[test]
    fn trn_017_requires_record_sub_id_for_stop_times() {
        // stop_times PK bileşiktir (trip_id + stop_sequence); record_id yalnız ilkini taşır.
        // İkincisi olmadan çeviri hangi SATIRA ait olduğunu söyleyemez.
        let file = make_file(
            vec!["table_name", "field_name", "language", "translation", "record_id", "record_sub_id"],
            vec![
                vec!["stop_times", "stop_headsign", "en", "Centre", "T1", ""],    // eksik → TRN_017
                vec!["stop_times", "stop_headsign", "en", "Centre", "T2", "3"],   // dolu → sessiz
                vec!["stops", "stop_name", "en", "Centre", "S1", ""],             // başka tablo → sessiz
            ],
        );
        let (_, notices) = validate_translations(&file);
        let hits: Vec<_> = notices.iter().filter(|n| n.rule_id == "TRN_017").collect();
        assert_eq!(hits.len(), 1, "yalnız ilk satır: {hits:?}");
        assert_eq!(hits[0].entity_id.as_deref(), Some("T1"));
        // TRN_014 ters koldur ve bu satırlarda konuşmamalı.
        assert!(!notices.iter().any(|n| n.rule_id == "TRN_014"), "{notices:?}");
    }
}
