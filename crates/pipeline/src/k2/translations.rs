use gtfs_core::EntityType;

use super::common::{
    build_row_map, get_raw_field, get_trimmed_field, looks_like_bcp47, make_k2_notice, RowMap,
};
use crate::k1_parse::RawFile;

/// Is `translations.txt::field_name` a field of the table?
///
/// 🔴 THIS FUNCTION WAS A HAND-WRITTEN ALLOWLIST and did not represent the spec. The spec's
/// ONLY rule about `field_name` is type-based (`P7c7134fe`, soft): *"Fields with
/// other types should not be translated."* There is NO table-based name list.
/// Measurement (run 18, ALL `translations.txt` files from the 18 feeds triggering `TRN_002`
/// were read from the archive): 1,900 of 1,958 findings translated a field that does NOT
/// EXIST in that table — genuine. The remaining 58 were existing, translatable fields:
/// `stops::stop_code` (Text, 50 findings) and `feed_info::feed_contact_url` (URL, 8 findings).
/// Both were false positives and held three feeds at the publication gate on `Kritik·Spec`.
///
/// The question is now: is the field a column of that table in the official schema catalogue
/// or a known profile extension? Official fields come from the generated spec catalogue,
/// extensions from `known_columns`; the type axis belongs to `TRN_011`.
pub(crate) fn table_has_field(table: &str, field: &str) -> bool {
    crate::k1_parse::known_columns_for_table(table).contains(&field)
        || super::translatable_fields_generated::spec_field_is_translatable(table, field).is_some()
}

pub(crate) fn is_known_translation_table(table: &str) -> bool {
    super::translatable_fields_generated::spec_translation_table_is_known(table)
}

/// In GTFS-JP v3 examples, `record_sub_id` may be written as `NONE` instead of empty
/// in tables that do not require a sub-identifier. This is not a real sub-identifier;
/// for `stop_times`, it is not a valid stop_sequence number.
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

pub fn validate_translations(file: &RawFile) -> (Vec<TranslationRecord>, Vec<gtfs_core::Notice>) {
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

        let table_name = get_trimmed_field(&row_map, "table_name")
            .unwrap_or("")
            .to_string();
        let table_known = is_known_translation_table(&table_name);
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

        let field_name = get_trimmed_field(&row_map, "field_name")
            .unwrap_or("")
            .to_string();
        // field_name cannot be validated without either required header or its
        // table_name context. ARC_025 already reports the missing prerequisite.
        // 🔴 IF THE TABLE IS UNKNOWN, THIS QUESTION HAS NO ANSWER (Phase 3.1 adjudication).
        // `valid_fields_for_table` returns an EMPTY list for an unknown table, so EVERY
        // field name used to be considered "invalid". The card already said — "if table_name
        // is invalid, the allowed-field list is empty; TRN_002 may also trigger" — but the
        // relation was not connected to `blocks`: documented, but not told to the machine.
        // Measurement: `mdb-2933` writes `stops.txt` (with extension) in the `table_name` column;
        // the spec requires the extensionless name, so TRN_001 fires CORRECTLY — but 23,049
        // derivative TRN_002 findings followed. The root is already reported, so the derivative stays silent.
        if has_table_name
            && has_field_name
            && table_known
            && !table_has_field(&table_name, &field_name)
        {
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

        let language = get_trimmed_field(&row_map, "language")
            .unwrap_or("")
            .to_string();
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

        let translation = get_trimmed_field(&row_map, "translation")
            .unwrap_or("")
            .to_string();
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

        let record_id = get_raw_field(&row_map, "record_id")
            .filter(|v| !v.trim().is_empty())
            .map(str::to_string);
        let record_sub_id = get_raw_field(&row_map, "record_sub_id")
            .filter(|v| !v.trim().is_empty())
            .map(str::to_string);
        let field_value = get_trimmed_field(&row_map, "field_value")
            .filter(|v| !v.trim().is_empty())
            .map(str::to_string);

        // TRN_015: the spec says `field_value` is "Required if record_id is empty". If both
        // are empty, the row does not say which record it translates — translation can never apply.
        //
        // TWO NARROWINGS:
        //  • Exclude `feed_info` — all three fields are forbidden there (TRN_013).
        //  • Exclude an INVALID table_name — TRN_001 already rejects that row as "unsupported
        //    table", making "which record does it translate" meaningless. Measured in corpus
        //    `mdb-2519`: without this narrowing, a feed using the old Google
        //    `trans_id,lang,translation` form produced 101,872 TRN_015 findings — those rows
        //    already trigger TRN_001/002/003/006/011, so this would say the same thing SIX times.
        let table_known = is_known_translation_table(&table_name);
        if table_known && table_name != "feed_info" && record_id.is_none() && field_value.is_none()
        {
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
                "record_id ve field_value ikisi de boş; çevirinin hangi kayda ait olduğu belirsiz."
                    .to_string(),
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
                // The violation is that both fields are populated TOGETHER.
                Some("record_id|field_value"),
                None,
                None,
                "record_id ve field_value aynı anda kullanılamaz.".to_string(),
                "record_id veya field_value alanlarından yalnızca birini kullanın.",
            ));
        }

        // 🔴 `table_known` IS REQUIRED. This heuristic assumes the field name resembles a
        // translatable content type in GTFS — but if the table is unknown, nothing is known
        // about its schema and no claim can be made about whether the field is translatable.
        // mdb-2126 is exactly this case: `table_name = "directions"` (not a GTFS table; TRN_001
        // already says so with 421 findings) and `field_name = "direction"`, translated as
        // 'East'/'West'/'North'/'South'. The field is clearly translatable; the rule produced
        // a second and WRONG Spec claim on the same 421 rows.
        // TRN_002 carried the same derivative defect and now stays silent for unknown tables;
        // this is the omitted second half of that fix.
        // 🔴 THERE WAS A NAME HEURISTIC HERE: it searched the field name for "name"/"desc"/"url"/
        // "email"/"phone"/"headsign"/"signposted_as". The spec cares about TYPE, not NAME,
        // and the heuristic got both wrong: `stops::stop_code` has Text type and is translatable,
        // but matched none of those fragments, so the rule fired — both this and TRN_002 spoke
        // on 50 rows in `odpt-NipponChuoBus-Maebashi_Area`. The type now comes from the generated spec catalogue.
        //
        // ⚠️ The extension field's type is UNKNOWN (`None`), so no judgment is made — the same
        // contract applies to the `table_known` condition: we do not claim what we do not know.
        let type_is_translatable = super::translatable_fields_generated::spec_field_is_translatable(
            &table_name,
            &field_name,
        );
        if table_known && has_field_name && type_is_translatable == Some(false) {
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

        if table_name == "feed_info"
            && (record_id.is_some() || record_sub_id.is_some() || field_value.is_some())
        {
            notices.push(make_k2_notice(
                &mut counter,
                "TRN_013",
                EntityType::Row,
                None,
                Some(&row_map),
                &file.name,
                Some(line),
                // All THREE identity fields are forbidden on a feed_info row.
                Some("record_id|record_sub_id|field_value"),
                None,
                None,
                "feed_info çevirileri record_id, record_sub_id veya field_value kullanamaz.".to_string(),
                "feed_info satırları için record_id, record_sub_id ve field_value alanlarını boş bırakın.",
            ));
        }

        // TRN_017: `stop_times.txt` has a composite primary key
        // (`trip_id` + `stop_sequence`); `record_id` carries only the first, `record_sub_id`
        // the second. Without the second, translation cannot identify the ROW — the reference
        // is AMBIGUOUS, not dangling, so `XFL_014` does not see it either.
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
            && record_sub_id
                .as_deref()
                .is_some_and(|value| (is_v4_profile && is_gtfs_jp) || !is_none_record_sub_id(value))
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
            rows: rows
                .into_iter()
                .map(|r| r.into_iter().map(SmolStr::from).collect())
                .collect(),
            bytes: 0,
            raw_text: None,
            zip_entry_name: None,
        }
    }

    #[test]
    fn trn_017_requires_record_sub_id_for_stop_times() {
        // stop_times PK is composite (trip_id + stop_sequence); record_id carries only the first.
        // Without the second, translation cannot identify the ROW.
        let file = make_file(
            vec![
                "table_name",
                "field_name",
                "language",
                "translation",
                "record_id",
                "record_sub_id",
            ],
            vec![
                vec!["stop_times", "stop_headsign", "en", "Centre", "T1", ""], // missing → TRN_017
                vec!["stop_times", "stop_headsign", "en", "Centre", "T2", "3"], // populated → silent
                vec!["stops", "stop_name", "en", "Centre", "S1", ""], // different table → silent
            ],
        );
        let (_, notices) = validate_translations(&file);
        let hits: Vec<_> = notices.iter().filter(|n| n.rule_id == "TRN_017").collect();
        assert_eq!(hits.len(), 1, "yalnız ilk satır: {hits:?}");
        assert_eq!(hits[0].entity_id.as_deref(), Some("T1"));
        assert!(
            !notices.iter().any(|n| n.rule_id == "TRN_010"),
            "eksik sub-id K2'de TRN_010 üretmemeli: {notices:?}"
        );
        // TRN_014 is the inverse branch and must stay silent on these rows.
        assert!(
            !notices.iter().any(|n| n.rule_id == "TRN_014"),
            "{notices:?}"
        );
    }

    #[test]
    fn missing_translation_headers_do_not_cascade_row_rules() {
        // Legacy Google Transit extension. K1 emits ARC_025/ARC_017 for the
        // schema mismatch; K2 must not repeat that mismatch for every row.
        let file = make_file(
            vec!["trans_id", "lang", "translation"],
            vec![vec!["1", "EN", "Example"], vec!["2", "HE", "דוגמה"]],
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
            vec![
                "table_name",
                "field_name",
                "language",
                "translation",
                "record_id",
                "record_sub_id",
            ],
            vec![vec![
                "stops",
                "stop_name",
                "ja-Hrkt",
                "とうきょう",
                "S1",
                "NONE",
            ]],
        );
        let (_, notices) = validate_translations(&file);
        assert!(
            !notices.iter().any(|n| n.rule_id == "TRN_014"),
            "{notices:?}"
        );
    }

    #[test]
    fn v4_none_record_sub_id_is_trn_014() {
        let file = make_file(
            vec![
                "table_name",
                "field_name",
                "language",
                "translation",
                "record_id",
                "record_sub_id",
            ],
            vec![vec![
                "stops",
                "stop_name",
                "ja-Hrkt",
                "とうきょう",
                "S1",
                "NONE",
            ]],
        );
        let (_, notices) = validate_translations_with_profile(&file, true, true);
        assert!(
            notices.iter().any(|n| n.rule_id == "TRN_014"),
            "{notices:?}"
        );
    }

    #[test]
    fn v4_none_record_sub_id_is_tolerated_without_gtfs_jp_signal() {
        let file = make_file(
            vec![
                "table_name",
                "field_name",
                "language",
                "translation",
                "record_id",
                "record_sub_id",
            ],
            vec![vec!["stops", "stop_name", "en", "Tokyo", "S1", "NONE"]],
        );
        let (_, notices) = validate_translations_with_profile(&file, true, false);
        assert!(
            !notices.iter().any(|n| n.rule_id == "TRN_014"),
            "{notices:?}"
        );
    }

    /// 🔴 MEASURED FALSE POSITIVE (`odpt-NipponChuoBus-Maebashi_Area`, 50 rows).
    /// `stop_code` is a real `stops.txt` column and its spec type is **Text**, so it is
    /// translatable. Previously TWO rules spoke: `TRN_002` because it was absent from the
    /// hand-written allowlist, and `TRN_011` because the field name contained none of
    /// "name"/"desc"/"url". Both were unjustified, and `TRN_002` held the feed at the
    /// publication gate because it was `Kritik·Spec`.
    #[test]
    fn translatable_text_field_outside_the_old_whitelist_is_silent() {
        let file = make_file(
            vec![
                "table_name",
                "field_name",
                "language",
                "translation",
                "record_id",
            ],
            vec![vec!["stops", "stop_code", "ja", "1-A", "S1"]],
        );
        let (_, notices) = validate_translations(&file);
        assert!(
            !notices
                .iter()
                .any(|n| n.rule_id == "TRN_002" || n.rule_id == "TRN_011"),
            "{notices:?}"
        );
    }

    /// `feed_contact_url` is a real URL-type field in `feed_info.txt`. It produced four findings
    /// in each of two Spanish feeds (`tfs-655`, `mdb-2715`) and was their only blocker.
    #[test]
    fn translatable_url_field_outside_the_old_whitelist_is_silent() {
        let file = make_file(
            vec!["table_name", "field_name", "language", "translation"],
            vec![vec![
                "feed_info",
                "feed_contact_url",
                "eu",
                "https://example.eus",
            ]],
        );
        let (_, notices) = validate_translations(&file);
        assert!(
            !notices.iter().any(|n| n.rule_id == "TRN_002"),
            "{notices:?}"
        );
    }

    /// The rule's CORE value is here: all 1,900 corpus findings have this shape —
    /// `routes.txt` has NO `long_name` column (`route_long_name` exists). Such a translation
    /// row cannot apply, so `TRN_002` continues to speak.
    #[test]
    fn field_absent_from_the_table_still_raises_trn_002() {
        let file = make_file(
            vec![
                "table_name",
                "field_name",
                "language",
                "translation",
                "record_id",
            ],
            vec![vec!["routes", "long_name", "bg", "Liniya 1", "R1"]],
        );
        let (_, notices) = validate_translations(&file);
        assert!(
            notices.iter().any(|n| n.rule_id == "TRN_002"),
            "{notices:?}"
        );
    }

    /// The type axis remains in `TRN_011`: the field EXISTS but its type is Latitude, so it is
    /// not translatable. `TRN_002` stays SILENT here — the two rules now measure separate facts.
    #[test]
    fn existing_field_with_untranslatable_type_is_trn_011_only() {
        let file = make_file(
            vec![
                "table_name",
                "field_name",
                "language",
                "translation",
                "record_id",
            ],
            vec![vec!["stops", "stop_lat", "en", "41.0", "S1"]],
        );
        let (_, notices) = validate_translations(&file);
        assert!(
            notices.iter().any(|n| n.rule_id == "TRN_011"),
            "TRN_011 beklenir: {notices:?}"
        );
        assert!(
            !notices.iter().any(|n| n.rule_id == "TRN_002"),
            "TRN_002 susmali: {notices:?}"
        );
    }

    /// Extension field: `jp_trip_desc` is in `known_columns`, so `TRN_002` stays silent.
    /// Its type is NOT in the spec catalogue, so `TRN_011` also stays silent — we make no
    /// claim about what we do not know.
    #[test]
    fn extension_field_is_silent_in_both_rules() {
        let file = make_file(
            vec![
                "table_name",
                "field_name",
                "language",
                "translation",
                "record_id",
            ],
            vec![vec!["trips", "jp_trip_desc", "ja-Hrkt", "TOKYO EKI", "T1"]],
        );
        let (_, notices) = validate_translations(&file);
        assert!(
            !notices
                .iter()
                .any(|n| n.rule_id == "TRN_002" || n.rule_id == "TRN_011"),
            "{notices:?}"
        );
    }
}
