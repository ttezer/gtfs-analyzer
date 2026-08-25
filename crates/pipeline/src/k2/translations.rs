use gtfs_core::EntityType;

use super::common::{get_raw_field, build_row_map, get_trimmed_field, looks_like_bcp47, make_k2_notice, RowMap};
use crate::k1_parse::RawFile;

const TRANSLATION_TABLES: &[&str] = &[
    "agency", "stops", "routes", "trips", "stop_times", "feed_info", "attributions", "pathways",
    "levels",
];

pub(crate) fn valid_fields_for_table(table: &str) -> &'static [&'static str] {
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

pub(crate) fn is_known_translation_table(table: &str) -> bool {
    TRANSLATION_TABLES.contains(&table)
}

/// GTFS-JP v3 örneklerinde `record_sub_id` değeri, alt kimlik gerekmeyen
/// tablolarda boş yerine `NONE` olarak yazılabilir. Bu değer bir gerçek alt
/// kimlik değildir; `stop_times` için ise geçerli bir stop_sequence sayılmaz.
pub(crate) fn is_none_record_sub_id(value: &str) -> bool {
    value.trim().eq_ignore_ascii_case("NONE")
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
    validate_translations_with_profile(file, false, false)
}

/// Public profile-aware translation validator. V4 turns `record_sub_id` into
/// a strict GTFS field only when `is_gtfs_jp` is also true; both facts are
/// explicit so callers cannot accidentally enable JP strictness for a normal
/// feed. The full pipeline computes the signal once in K2 and carries it to
/// K4 in `EntityRecords`.
pub fn validate_translations_with_profile(
    file: &RawFile,
    is_v4_profile: bool,
    is_gtfs_jp: bool,
) -> (Vec<TranslationRecord>, Vec<gtfs_core::Notice>) {
    let mut notices = Vec::new();
    let mut records = Vec::new();
    let mut counter = 0;
    // ARC_025 owns missing required translation headers. A missing header is
    // not the same thing as a row containing an empty value: the latter is a
    // row-level TRN finding, while the former must not be converted into one
    // notice per row.
    let has_table_name = file.headers.iter().any(|h| h == "table_name");
    let has_field_name = file.headers.iter().any(|h| h == "field_name");
    let has_language = file.headers.iter().any(|h| h == "language");
    let has_translation = file.headers.iter().any(|h| h == "translation");

    for (row_idx, row) in file.rows.iter().enumerate() {
        let line = (row_idx + 2) as u64;
        let row_map = build_row_map(&file.headers, row);

        let table_name = get_trimmed_field(&row_map, "table_name").unwrap_or("").to_string();
        let table_known = TRANSLATION_TABLES.contains(&table_name.as_str());
        if has_table_name && !table_known {
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
        // field_name cannot be validated without either required header or its
        // table_name context. ARC_025 already reports the missing prerequisite.
        // 🔴 TABLO BİLİNMİYORSA BU SORU CEVAPSIZDIR (Faz 3.1 adjudikasyonu).
        // `valid_fields_for_table` bilinmeyen tabloda BOŞ liste döner, dolayısıyla HER
        // alan adı "geçersiz" sayılırdı. Kartında zaten yazıyordu — "table_name zaten
        // geçersizse izinli alan listesi boş döner; TRN_002 de tetiklenebilir" — ama
        // ilişki `blocks`'a bağlanmamıştı, yani belgelenmiş ama makineye söylenmemişti.
        // Ölçüm: `mdb-2933` `table_name` sütununa `stops.txt` (uzantılı) yazıyor; spec
        // uzantısız ad ister, TRN_001 DOĞRU ateşliyor — ama peşinden 23.049 TRN_002
        // türev bulgusu geliyordu. Kök zaten raporlandığı için türev susar.
        if has_table_name && has_field_name && table_known
            && !valid_fields_for_table(&table_name).contains(&field_name.as_str()) {
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
        if has_language && !looks_like_bcp47(&language) {
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
        if has_translation && translation.is_empty() {
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

        let record_id = get_raw_field(&row_map, "record_id").filter(|v| !v.trim().is_empty()).map(str::to_string);
        let record_sub_id = get_raw_field(&row_map, "record_sub_id").filter(|v| !v.trim().is_empty()).map(str::to_string);
        let field_value = get_trimmed_field(&row_map, "field_value").filter(|v| !v.trim().is_empty()).map(str::to_string);

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
        if table_name == "stop_times" && record_id.is_some()
            && record_sub_id.is_none() {
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

        // 🔴 `table_known` KOŞULU ŞART. Bu sezgi, alan adının GTFS'te çevrilebilir bir
        // içerik türüne benzediğini varsayar — ama tablo tanınmıyorsa o şemadan hiçbir şey
        // bilinmez ve alanın çevrilebilir olup olmadığı hakkında hüküm verilemez.
        // mdb-2126 tam bu vakadır: `table_name = "directions"` (GTFS tablosu değil, TRN_001
        // zaten 421 bulguyla bunu söylüyor) ve `field_name = "direction"`, çevirileri
        // 'East'/'West'/'North'/'South'. Alan apaçık çevrilebilir; kural aynı 421 satırda
        // ikinci ve YANLIŞ bir Spec iddiası üretiyordu.
        // TRN_002 aynı türev kusurunu taşıyordu ve tablo bilinmediğinde susmaya çevrildi;
        // bu, o düzeltmenin atlanmış ikinci yarısıdır.
        let translatable = ["name", "desc", "url", "email", "phone", "headsign", "signposted_as"];
        if table_known && has_field_name && !translatable.iter().any(|needle| field_name.contains(needle)) {
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

        if table_name != "stop_times"
            && record_sub_id.as_deref().is_some_and(|value| {
                (is_v4_profile && is_gtfs_jp) || !is_none_record_sub_id(value)
            })
        {
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

    #[test]
    fn missing_translation_headers_do_not_cascade_row_rules() {
        // Legacy Google Transit extension. K1 emits ARC_025/ARC_017 for the
        // schema mismatch; K2 must not repeat that mismatch for every row.
        let file = make_file(
            vec!["trans_id", "lang", "translation"],
            vec![
                vec!["1", "EN", "Example"],
                vec!["2", "HE", "דוגמה"],
            ],
        );
        let (_, notices) = validate_translations(&file);
        for rule in ["TRN_001", "TRN_002", "TRN_003", "TRN_008", "TRN_011"] {
            assert!(
                !notices.iter().any(|n| n.rule_id == rule),
                "{rule} must be gated by its missing header: {notices:?}"
            );
        }
    }

    #[test]
    fn gtfs_jp_none_record_sub_id_is_not_trn_014() {
        let file = make_file(
            vec!["table_name", "field_name", "language", "translation", "record_id", "record_sub_id"],
            vec![vec!["stops", "stop_name", "ja-Hrkt", "とうきょう", "S1", "NONE"]],
        );
        let (_, notices) = validate_translations(&file);
        assert!(!notices.iter().any(|n| n.rule_id == "TRN_014"), "{notices:?}");
    }

    #[test]
    fn v4_none_record_sub_id_is_trn_014() {
        let file = make_file(
            vec!["table_name", "field_name", "language", "translation", "record_id", "record_sub_id"],
            vec![vec!["stops", "stop_name", "ja-Hrkt", "とうきょう", "S1", "NONE"]],
        );
        let (_, notices) = validate_translations_with_profile(&file, true, true);
        assert!(notices.iter().any(|n| n.rule_id == "TRN_014"), "{notices:?}");
    }

    #[test]
    fn v4_none_record_sub_id_is_tolerated_without_gtfs_jp_signal() {
        let file = make_file(
            vec!["table_name", "field_name", "language", "translation", "record_id", "record_sub_id"],
            vec![vec!["stops", "stop_name", "en", "Tokyo", "S1", "NONE"]],
        );
        let (_, notices) = validate_translations_with_profile(&file, true, false);
        assert!(!notices.iter().any(|n| n.rule_id == "TRN_014"), "{notices:?}");
    }
}
