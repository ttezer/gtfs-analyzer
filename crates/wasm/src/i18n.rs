//! English notice text for the public SDK build.
//!
//! The validator pipeline keeps Turkish as its native text because the Analyzer
//! UI owns the locale selection. The SDK enables `sdk-en` and translates the
//! public notice fields at the WASM boundary, without shipping a separate
//! locale file in the npm package.

use std::collections::HashMap;
use std::sync::OnceLock;

use gtfs_core::{intern_notice_text, FatalError, Notice};
use serde::Deserialize;

const EN_JSON: &str = include_str!(concat!(env!("OUT_DIR"), "/en_locale.json"));

#[derive(Debug, Deserialize)]
struct Dictionary {
    messages: HashMap<String, String>,
    remediations: HashMap<String, String>,
    titles: HashMap<String, String>,
    /// `{FatalCode}.{variant}` → template; the same dictionary as the CLI (`crates/core/locales/en.json`).
    #[serde(default)]
    fatal_messages: HashMap<String, String>,
}

fn dictionary() -> &'static Dictionary {
    static DICTIONARY: OnceLock<Dictionary> = OnceLock::new();
    DICTIONARY.get_or_init(|| {
        serde_json::from_str(EN_JSON).expect("embedded English locale is not readable")
    })
}

/// Rewrites the user-facing notice fields in place.
pub fn translate_notices(notices: &mut [Notice]) {
    let dictionary = dictionary();
    for notice in notices {
        if let Some(title) = dictionary.titles.get(&notice.rule_id) {
            notice.title = intern_notice_text(title);
        }
        let profile = notice
            .details
            .as_ref()
            .and_then(|d| d.get("jp_profile"))
            .map(|p| format!("{}.{}", notice.rule_id, p.to_lowercase()));
        // The SAME selection order as `gtfs_core::i18n::Translator::translate` (CLI/Python):
        // a variant also works without a profile (`CAL_015.future_only_jp`, `STM_047.missing_*`).
        let specific = notice
            .details
            .as_ref()
            .and_then(|d| d.get("message_variant"))
            .map(|kind| {
                profile.as_deref().map_or_else(
                    || format!("{}.{kind}", notice.rule_id),
                    |key| format!("{key}.{kind}"),
                )
            });
        if let Some(template) = specific
            .as_ref()
            .and_then(|k| dictionary.messages.get(k))
            .or_else(|| profile.as_ref().and_then(|k| dictionary.messages.get(k)))
            .or_else(|| dictionary.messages.get(&notice.rule_id))
        {
            notice.message = fill(template, notice);
        }
        if let Some(remediation) = specific
            .as_ref()
            .and_then(|k| dictionary.remediations.get(k))
            .or_else(|| {
                profile
                    .as_ref()
                    .and_then(|k| dictionary.remediations.get(k))
            })
            .or_else(|| dictionary.remediations.get(&notice.rule_id))
        {
            notice.remediation = intern_notice_text(remediation);
        }
    }
}

/// Rewrites a fatal error's message from its English template; without one the
/// pipeline's text stays.
pub fn translate_fatal(mut err: FatalError) -> FatalError {
    if let Some(template) = dictionary().fatal_messages.get(&err.template_key()) {
        let message = fill_with(template, |key| {
            err.params.get(key).cloned().unwrap_or_default()
        });
        err.message = message;
    }
    err
}

/// Substitutes `{field}` placeholders from the notice, matching the UI and CLI
/// locale implementations. Unknown placeholders resolve to an empty string.
fn fill(template: &str, notice: &Notice) -> String {
    fill_with(template, |key| resolve(key, notice))
}

fn fill_with(template: &str, resolve: impl Fn(&str) -> String) -> String {
    let mut out = String::with_capacity(template.len());
    let mut rest = template;

    while let Some(open) = rest.find('{') {
        out.push_str(&rest[..open]);
        let after = &rest[open + 1..];

        let Some(close) = after.find('}') else {
            out.push_str(&rest[open..]);
            return out;
        };

        let key = &after[..close];
        if key.is_empty() || !key.chars().all(|c| c.is_alphanumeric() || c == '_') {
            out.push('{');
            rest = after;
            continue;
        }

        out.push_str(&resolve(key));
        rest = &after[close + 1..];
    }

    out.push_str(rest);
    out
}

/// `details` shadows fixed fields, matching the UI's spread order.
fn resolve(key: &str, notice: &Notice) -> String {
    if let Some(value) = notice.details.as_ref().and_then(|details| details.get(key)) {
        return value.clone();
    }
    match key {
        "entity_id" => notice.entity_id.clone().unwrap_or_default(),
        "observed_value" => notice.observed_value.clone().unwrap_or_default(),
        "expected_value" => notice.expected_value.clone().unwrap_or_default(),
        "file" => notice.file.clone().unwrap_or_default(),
        "field" => notice.field.clone().unwrap_or_default(),
        "line" => notice.line.map(|line| line.to_string()).unwrap_or_default(),
        _ => String::new(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use gtfs_core::{EntityType, RuleClass, Severity};

    /// SDK translation must produce the SAME text as the CLI/Python translator
    /// (`gtfs_core::i18n`). The two copies are deliberate (to avoid putting a ~500 KB
    /// dictionary in the WASM package), but key selection diverged until 2026-09-30:
    /// profile-less `message_variant` was ignored in WASM. The two translators are compared
    /// for EVERY dictionary key shape (rule, rule.profile, rule.variant, rule.profile.variant).
    #[test]
    fn sdk_translation_matches_the_cli_translator() {
        let core = gtfs_core::i18n::Translator::new(gtfs_core::i18n::Lang::En)
            .expect("core dictionary could not be read")
            .expect("English translator is missing");
        let dictionary = dictionary();
        let keys = dictionary
            .messages
            .keys()
            .chain(dictionary.remediations.keys())
            .chain(dictionary.titles.keys());

        let mut checked = 0;
        for key in keys {
            let parts: Vec<&str> = key.split('.').collect();
            let rule = parts[0];
            // A two-part key may be either a profile or a variant: try both.
            let shapes: Vec<(Option<&str>, Option<&str>)> = match parts.as_slice() {
                [_] => vec![(None, None)],
                [_, x] => vec![(Some(*x), None), (None, Some(*x))],
                [_, profile, kind] => vec![(Some(*profile), Some(*kind))],
                _ => continue,
            };
            for (profile, kind) in shapes {
                let mut details = std::collections::BTreeMap::from([
                    ("affected_services".to_string(), "4".to_string()),
                    ("first_service_date".to_string(), "20261001".to_string()),
                ]);
                if let Some(profile) = profile {
                    details.insert("jp_profile".to_string(), profile.to_uppercase());
                }
                if let Some(kind) = kind {
                    details.insert("message_variant".to_string(), kind.to_string());
                }
                let mut notice = parity_notice(rule, details);
                let mut expected = notice.clone();
                core.translate(&mut expected);
                translate_notices(std::slice::from_mut(&mut notice));
                assert_eq!(
                    (&notice.title, &notice.message, &notice.remediation),
                    (&expected.title, &expected.message, &expected.remediation),
                    "SDK and CLI translations differ: {key} (profile {profile:?}, variant {kind:?})"
                );
                checked += 1;
            }
        }
        assert!(checked > 1000, "dictionary is smaller than expected: {checked}");
    }

    fn parity_notice(rule: &str, details: std::collections::BTreeMap<String, String>) -> Notice {
        Notice {
            id: "k/parity#1".to_string(),
            rule_id: rule.to_string(),
            severity: Severity::Orta,
            rule_class: RuleClass::Quality,
            entity_type: EntityType::Trip,
            entity_id: Some("T1".to_string()),
            scope_key: Some("T1".to_string()),
            file: Some("trips.txt".to_string()),
            line: Some(7),
            field: Some("direction_id".to_string()),
            observed_value: Some("9".to_string()),
            expected_value: Some("0 or 1".to_string()),
            details: Some(details),
            whitespace_derived: false,
            whitespace_candidate: false,
            title: "Türkçe başlık".to_string().into(),
            message: "Türkçe mesaj".to_string(),
            remediation: "Türkçe çözüm".to_string().into(),
            blocks: Vec::new(),
            base_effort: 1,
            service_id: None,
            agency_distribution: None,
        }
    }

    /// The SDK also writes fatal messages from the English template; a variant without a
    /// template preserves the pipeline text.
    #[test]
    fn english_dictionary_translates_fatal_errors() {
        let err = translate_fatal(
            FatalError::new(gtfs_core::FatalCode::DecompressionLimit, "Türkçe metin")
                .variant("entry_cap")
                .param("file", "stop_times.txt")
                .param("got", 10)
                .param("cap", 5),
        );
        assert_eq!(
            err.message,
            "'stop_times.txt' exceeded the decompression guard: the per-entry decompressed size limit was exceeded (10 > 5 bytes)"
        );
        let kept = translate_fatal(
            FatalError::new(gtfs_core::FatalCode::InvalidInput, "özgün").variant("yok"),
        );
        assert_eq!(kept.message, "özgün");
    }

    #[test]
    fn english_dictionary_fills_notice_placeholders() {
        let mut notice = Notice {
            id: "k2/STM_004#1".to_string(),
            rule_id: "STM_004".to_string(),
            severity: Severity::Kritik,
            rule_class: RuleClass::Spec,
            entity_type: EntityType::Trip,
            entity_id: Some("T1".to_string()),
            scope_key: None,
            file: Some("stop_times.txt".to_string()),
            line: Some(42),
            field: Some("departure_time".to_string()),
            observed_value: Some("25:1:00".to_string()),
            expected_value: None,
            details: None,
            whitespace_derived: false,
            whitespace_candidate: false,
            title: "Türkçe başlık".to_string().into(),
            message: "Türkçe mesaj".to_string(),
            remediation: "Türkçe çözüm".to_string().into(),
            blocks: Vec::new(),
            base_effort: 1,
            service_id: None,
            agency_distribution: None,
        };

        translate_notices(std::slice::from_mut(&mut notice));

        assert!(notice.title.is_ascii(), "title: {}", notice.title);
        assert!(notice.message.is_ascii(), "message: {}", notice.message);
        assert!(
            notice.remediation.is_ascii(),
            "remediation: {}",
            notice.remediation
        );
        assert!(notice.message.contains("T1"));

        notice.rule_id = "JPN_006".to_string();
        notice.details = Some(std::collections::BTreeMap::from([
            ("jp_profile".into(), "V4".into()),
            ("message_variant".into(), "missing_review".into()),
        ]));
        translate_notices(std::slice::from_mut(&mut notice));
        assert!(notice.message.contains("no score penalty"));
        assert!(notice.remediation.contains("Manually verify"));

        notice.rule_id = "JPN_029".to_string();
        notice.field = Some("stop_headsign".into());
        notice.details = Some(std::collections::BTreeMap::from([
            ("jp_profile".into(), "v4".into()),
            ("message_variant".into(), "aggregate".into()),
            ("table_name".into(), "stop_times".into()),
            ("source_value".into(), "渋谷".into()),
            ("affected_records".into(), "18426".into()),
            (
                "example_record_ids".into(),
                "trip_id=T1,stop_sequence=2".into(),
            ),
        ]));
        translate_notices(std::slice::from_mut(&mut notice));
        assert!(notice.message.contains("stop_times.stop_headsign = 渋谷"));
        assert!(notice.message.contains("18426 rows"));
        assert!(notice.message.contains("trip_id=T1,stop_sequence=2"));
    }
}
