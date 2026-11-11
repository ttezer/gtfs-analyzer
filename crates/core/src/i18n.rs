//! Notice text localisation for the native surfaces (CLI and Python package) —
//! the Rust counterpart of `ui/src/i18n.ts`. Behind the `i18n` feature so the
//! WASM build does not embed the dictionaries.
//!
//! The pipeline emits Turkish text natively, so `tr` needs no dictionary. `en`
//! and `ja` are `{placeholder}` templates keyed by rule id, filled from the
//! notice's own fields exactly like the UI does. Fallback chain, also mirrored
//! from the UI: requested locale → English → the pipeline's Turkish text.
//!
//! Planner cards (`RuleCards`) are plain-language `title`/`what`/`action` texts
//! per rule, read from the UI's `producer.card.<RULE>.*` keys. They exist in
//! every language, Turkish included, and `es` is a cards-only locale.
//!
//! The dictionaries are derived from the TypeScript locales by
//! `ui/scripts/export-locales.mjs` (`npm run locales:export`) into
//! `crates/core/locales/`; the TypeScript locales stay
//! the single source of truth and `locale-parity.test.ts` fails on drift.

use std::collections::HashMap;
use std::str::FromStr;

use serde::Deserialize;

use crate::{FatalError, Notice};

const EN_JSON: &str = include_str!("../locales/en.json");
const JA_JSON: &str = include_str!("../locales/ja.json");
const FR_JSON: &str = include_str!("../locales/fr.json");
const TR_JSON: &str = include_str!("../locales/tr.json");
const ES_JSON: &str = include_str!("../locales/es.json");

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Lang {
    /// Turkish — titles from the dictionary; messages are the pipeline's native text.
    Tr,
    En,
    Ja,
    Fr,
    /// Spanish — cards only; every other text falls back to English.
    Es,
}

impl FromStr for Lang {
    type Err = String;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value {
            "tr" => Ok(Lang::Tr),
            "en" => Ok(Lang::En),
            "ja" => Ok(Lang::Ja),
            "fr" => Ok(Lang::Fr),
            "es" => Ok(Lang::Es),
            other => Err(format!(
                "unknown language '{other}'; expected tr, en, ja, fr or es"
            )),
        }
    }
}

#[derive(Debug, Deserialize)]
struct Dictionary {
    #[serde(default)]
    messages: HashMap<String, String>,
    #[serde(default)]
    remediations: HashMap<String, String>,
    #[serde(default)]
    titles: HashMap<String, String>,
    /// `{FatalCode}.{variant}` → template; parameters are filled from `FatalError::params`.
    #[serde(default)]
    fatal_messages: HashMap<String, String>,
    #[serde(default)]
    cards: HashMap<String, RuleCard>,
}

impl Dictionary {
    fn parse(raw: &str, lang: &str) -> Result<Self, String> {
        serde_json::from_str(raw)
            .map_err(|err| format!("embedded '{lang}' locale is not readable: {err}"))
    }
}

/// For Turkish only the rule titles come from the dictionary; messages and
/// remediations are still the pipeline's own Turkish text (issue #201).
pub struct Translator {
    primary: Dictionary,
    /// English, consulted when the primary locale lacks the rule.
    fallback: Option<Dictionary>,
}

impl Translator {
    pub fn new(lang: Lang) -> Result<Option<Self>, String> {
        match lang {
            Lang::Tr => Ok(Some(Self {
                primary: Dictionary::parse(TR_JSON, "tr")?,
                fallback: None,
            })),
            Lang::En => Ok(Some(Self {
                primary: Dictionary::parse(EN_JSON, "en")?,
                fallback: None,
            })),
            Lang::Ja => Ok(Some(Self {
                primary: Dictionary::parse(JA_JSON, "ja")?,
                fallback: Some(Dictionary::parse(EN_JSON, "en")?),
            })),
            Lang::Fr => Ok(Some(Self {
                primary: Dictionary::parse(FR_JSON, "fr")?,
                fallback: Some(Dictionary::parse(EN_JSON, "en")?),
            })),
            Lang::Es => Ok(Some(Self {
                primary: Dictionary::parse(ES_JSON, "es")?,
                fallback: Some(Dictionary::parse(EN_JSON, "en")?),
            })),
        }
    }

    fn lookup<'a>(
        &'a self,
        field: impl Fn(&'a Dictionary) -> &'a HashMap<String, String>,
        rule_id: &str,
    ) -> Option<&'a str> {
        field(&self.primary)
            .get(rule_id)
            .or_else(|| self.fallback.as_ref().and_then(|d| field(d).get(rule_id)))
            .map(String::as_str)
    }

    /// Rewrites `title`, `message` and `remediation` in place. Rules missing
    /// from the dictionaries keep the pipeline's Turkish text.
    pub fn translate(&self, notice: &mut Notice) {
        if let Some(title) = self.lookup(|d| &d.titles, &notice.rule_id) {
            notice.title = crate::intern_notice_text(title);
        }
        let profile_variant = notice
            .details
            .as_ref()
            .and_then(|details| details.get("jp_profile"))
            .map(|profile| format!("{}.{}", notice.rule_id, profile.to_lowercase()));
        let specific = notice
            .details
            .as_ref()
            .and_then(|details| details.get("message_variant"))
            .map(|kind| {
                profile_variant.as_deref().map_or_else(
                    || format!("{}.{}", notice.rule_id, kind),
                    |key| format!("{key}.{kind}"),
                )
            });
        if let Some(template) = specific
            .as_deref()
            .and_then(|key| self.lookup(|d| &d.messages, key))
            .or_else(|| {
                profile_variant
                    .as_deref()
                    .and_then(|key| self.lookup(|d| &d.messages, key))
            })
            .or_else(|| self.lookup(|d| &d.messages, &notice.rule_id))
        {
            notice.message = fill(template, notice);
        }
        if let Some(remediation) = specific
            .as_deref()
            .and_then(|key| self.lookup(|d| &d.remediations, key))
            .or_else(|| {
                profile_variant
                    .as_deref()
                    .and_then(|key| self.lookup(|d| &d.remediations, key))
            })
            .or_else(|| self.lookup(|d| &d.remediations, &notice.rule_id))
        {
            notice.remediation = crate::intern_notice_text(remediation);
        }
    }

    /// Rewrites a fatal error's message from its template. Without a template (or
    /// without `params`) the pipeline's Turkish text stays.
    pub fn translate_fatal(&self, err: &mut FatalError) {
        if let Some(template) = self.lookup(|d| &d.fatal_messages, &err.template_key()) {
            err.message = fill_with(template, |key| {
                err.params.get(key).cloned().unwrap_or_default()
            });
        }
    }

    /// Sections (`titles`, `messages`, `remediations`) the PRIMARY dictionary lacks
    /// for `rule_id`, ignoring the English fallback. Lets callers that know the
    /// rule registry gate a locale's completeness.
    pub fn missing_sections(&self, rule_id: &str) -> Vec<&'static str> {
        [
            ("titles", &self.primary.titles),
            ("messages", &self.primary.messages),
            ("remediations", &self.primary.remediations),
        ]
        .into_iter()
        .filter(|(_, table)| !table.contains_key(rule_id))
        .map(|(section, _)| section)
        .collect()
    }

    /// Registry title for the `rules` subcommand, which has no notice context.
    pub fn rule_title<'a>(&'a self, rule_id: &str, fallback: &'a str) -> &'a str {
        self.lookup(|d| &d.titles, rule_id).unwrap_or(fallback)
    }

    /// Rule-level message template for registry/catalog consumers that do not have
    /// a concrete notice from which to fill placeholders.
    pub fn rule_message(&self, rule_id: &str) -> Option<&str> {
        self.lookup(|d| &d.messages, rule_id)
    }

    /// Rule-level remediation text for registry/catalog consumers.
    pub fn rule_remediation(&self, rule_id: &str) -> Option<&str> {
        self.lookup(|d| &d.remediations, rule_id)
    }
}

/// Plain-language planner text for one rule, as the UI's planner view shows it.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize, serde::Serialize)]
pub struct RuleCard {
    pub title: String,
    /// `None` when the text needs per-notice placeholders (`{stop_count}` …),
    /// which a rule-level consumer cannot fill.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub what: Option<String>,
    pub action: String,
}

/// Planner cards in one language, with English as the fallback. Unlike
/// `Translator`, Turkish has a dictionary here: the pipeline has no native card text.
pub struct RuleCards {
    primary: HashMap<String, RuleCard>,
    fallback: Option<HashMap<String, RuleCard>>,
}

impl RuleCards {
    pub fn new(lang: Lang) -> Result<Self, String> {
        let (raw, name) = match lang {
            Lang::Tr => (TR_JSON, "tr"),
            Lang::En => (EN_JSON, "en"),
            Lang::Ja => (JA_JSON, "ja"),
            Lang::Fr => (FR_JSON, "fr"),
            Lang::Es => (ES_JSON, "es"),
        };
        let fallback = match lang {
            Lang::En => None,
            _ => Some(Dictionary::parse(EN_JSON, "en")?.cards),
        };
        Ok(Self {
            primary: Dictionary::parse(raw, name)?.cards,
            fallback,
        })
    }

    /// The rule's card, or `None` when no language has one.
    pub fn get(&self, rule_id: &str) -> Option<RuleCard> {
        let mut card = self
            .primary
            .get(rule_id)
            .or_else(|| self.fallback.as_ref().and_then(|cards| cards.get(rule_id)))?
            .clone();
        card.what = card.what.filter(|what| !has_placeholder(what));
        Some(card)
    }

    /// Whether the PRIMARY language has its own card for `rule_id`, ignoring the fallback.
    pub fn has_own(&self, rule_id: &str) -> bool {
        self.primary.contains_key(rule_id)
    }
}

/// Whether `text` holds a `{key}` placeholder in the sense of `fill_with`.
fn has_placeholder(text: &str) -> bool {
    text.split('{').skip(1).any(|part| {
        part.split_once('}').is_some_and(|(key, _)| {
            !key.is_empty() && key.chars().all(|c| c.is_alphanumeric() || c == '_')
        })
    })
}

/// Substitutes `{field}` placeholders from the notice, like the UI's
/// `tpl.replace(/\{(\w+)\}/g, …)`. Unknown placeholders resolve to an empty
/// string; a brace that is not a `\w+` placeholder is left untouched.
fn fill(template: &str, notice: &Notice) -> String {
    fill_with(template, |key| resolve(key, notice))
}

/// Substitutes `{key}` placeholders through `resolve`; malformed braces stay as text.
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

/// `details` shadows the fixed fields — the UI spreads it last, so it wins.
fn resolve(key: &str, notice: &Notice) -> String {
    if let Some(value) = notice.details.as_ref().and_then(|d| d.get(key)) {
        return value.clone();
    }
    match key {
        "entity_id" => notice.entity_id.clone().unwrap_or_default(),
        "observed_value" => notice.observed_value.clone().unwrap_or_default(),
        "expected_value" => notice.expected_value.clone().unwrap_or_default(),
        "file" => notice.file.clone().unwrap_or_default(),
        "field" => notice.field.clone().unwrap_or_default(),
        "line" => notice.line.map(|l| l.to_string()).unwrap_or_default(),
        _ => String::new(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn placeholders(template: &str) -> std::collections::BTreeSet<String> {
        template
            .split('{')
            .skip(1)
            .filter_map(|part| part.split_once('}').map(|(key, _)| key.to_string()))
            .collect()
    }

    /// Every fatal template exists in all three languages and uses the SAME placeholders;
    /// if one were missing, the parameter would silently render empty in that language.
    #[test]
    fn fatal_templates_match_across_locales() {
        let en = Dictionary::parse(EN_JSON, "en").unwrap();
        assert!(
            !en.fatal_messages.is_empty(),
            "English fatal templates are empty"
        );
        for (lang, raw) in [("fr", FR_JSON), ("ja", JA_JSON)] {
            let other = Dictionary::parse(raw, lang).unwrap();
            let mut en_keys: Vec<_> = en.fatal_messages.keys().collect();
            let mut other_keys: Vec<_> = other.fatal_messages.keys().collect();
            en_keys.sort();
            other_keys.sort();
            assert_eq!(
                en_keys, other_keys,
                "{lang}: fatal template keys differ"
            );
            for (key, template) in &en.fatal_messages {
                assert_eq!(
                    placeholders(template),
                    placeholders(&other.fatal_messages[key]),
                    "{lang}: {key} placeholders differ"
                );
            }
        }
    }

    #[test]
    fn fatal_without_template_keeps_the_pipeline_text() {
        let translator = Translator::new(Lang::En).unwrap().unwrap();
        let mut known =
            FatalError::new(crate::FatalCode::ZipUnreadable, "ZIP dosyası okunamadı")
                .variant("entry_read")
                .param("file", "stops.txt")
                .param("detail", "bad crc");
        translator.translate_fatal(&mut known);
        assert_eq!(known.message, "'stops.txt' could not be read: bad crc");

        let mut unknown = FatalError::new(crate::FatalCode::ZipUnreadable, "özgün metin")
            .variant("no_such_variant");
        translator.translate_fatal(&mut unknown);
        assert_eq!(unknown.message, "özgün metin");
    }
    use crate::{EntityType, RuleClass, Severity};

    fn notice() -> Notice {
        Notice {
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
            title: "departure_time geçersiz format".to_string().into(),
            message: "Türkçe mesaj".to_string(),
            remediation: "Türkçe çözüm".to_string().into(),
            blocks: Vec::new(),
            base_effort: 1,
            service_id: None,
            agency_distribution: None,
        }
    }

    #[test]
    fn lang_parses_the_four_codes_and_rejects_others() {
        assert_eq!("en".parse::<Lang>(), Ok(Lang::En));
        assert_eq!("tr".parse::<Lang>(), Ok(Lang::Tr));
        assert!("EN".parse::<Lang>().is_err());
        assert!("de".parse::<Lang>().is_err());
    }

    #[test]
    fn spanish_parses_and_falls_back_to_english_for_rule_texts() {
        assert_eq!("es".parse::<Lang>(), Ok(Lang::Es));
        let es = Translator::new(Lang::Es).unwrap().unwrap();
        let en = Translator::new(Lang::En).unwrap().unwrap();
        assert_eq!(
            es.rule_remediation("STM_056"),
            en.rule_remediation("STM_056")
        );
    }

    #[test]
    fn cards_exist_in_every_language_and_differ_from_english() {
        let en = RuleCards::new(Lang::En).unwrap().get("STM_056").unwrap();
        for lang in [Lang::Tr, Lang::Ja, Lang::Fr, Lang::Es] {
            let cards = RuleCards::new(lang).unwrap();
            assert!(cards.has_own("STM_056"), "{lang:?} has no own STM_056 card");
            let card = cards.get("STM_056").unwrap();
            assert_ne!(card.title, en.title, "{lang:?} card title is English");
            assert_ne!(card.action, en.action, "{lang:?} card action is English");
            assert!(card.what.is_some(), "{lang:?} card has no what");
        }
    }

    #[test]
    fn cards_drop_what_that_needs_notice_placeholders() {
        let card = RuleCards::new(Lang::En).unwrap().get("STM_001").unwrap();
        assert!(
            card.what.is_none(),
            "STM_001 what has per-notice placeholders"
        );
        assert!(!card.title.is_empty() && !card.action.is_empty());
    }

    #[test]
    fn unknown_rule_has_no_card() {
        assert!(RuleCards::new(Lang::Es).unwrap().get("NOPE_999").is_none());
    }

    #[test]
    fn placeholder_detection_matches_fill() {
        assert!(has_placeholder("from {from} to {to}"));
        assert!(!has_placeholder("no braces"));
        assert!(!has_placeholder("{not a key}"));
        assert!(!has_placeholder("unclosed {brace"));
    }

    #[test]
    fn turkish_takes_titles_from_the_dictionary_and_keeps_native_messages() {
        let translator = Translator::new(Lang::Tr).unwrap().unwrap();
        let mut n = notice();
        n.title = "English title from the registry".to_string().into();
        translator.translate(&mut n);

        assert_eq!(&*n.title, translator.rule_title("STM_004", ""));
        assert!(!n.title.is_empty() && &*n.title != "English title from the registry");
        // Messages and remediations stay the pipeline's Turkish text (issue #201).
        assert_eq!(n.message, "Türkçe mesaj");
        assert_eq!(&*n.remediation, "Türkçe çözüm");
    }

    #[test]
    fn english_rewrites_title_message_and_remediation() {
        let translator = Translator::new(Lang::En).unwrap().unwrap();
        let mut n = notice();
        translator.translate(&mut n);

        assert!(
            n.message.contains("T1"),
            "placeholder must be filled: {}",
            n.message
        );
        assert!(
            n.message.is_ascii(),
            "expected English text, got: {}",
            n.message
        );
        assert_ne!(n.title.as_ref(), "departure_time geçersiz format");
        assert_ne!(n.remediation.as_ref(), "Türkçe çözüm");
    }

    #[test]
    fn unknown_rule_keeps_the_pipeline_text() {
        let translator = Translator::new(Lang::En).unwrap().unwrap();
        let mut n = notice();
        n.rule_id = "NOPE_999".to_string();
        translator.translate(&mut n);

        assert_eq!(n.message, "Türkçe mesaj");
        assert_eq!(n.title.as_ref(), "departure_time geçersiz format");
    }

    #[test]
    fn japanese_falls_back_to_english_then_turkish() {
        let translator = Translator::new(Lang::Ja).unwrap().unwrap();
        let mut n = notice();
        n.rule_id = "NOPE_999".to_string();
        translator.translate(&mut n);
        assert_eq!(
            n.message, "Türkçe mesaj",
            "no dictionary entry → pipeline text"
        );
    }

    #[test]
    fn placeholders_resolve_from_notice_fields() {
        let n = notice();
        assert_eq!(
            fill("{entity_id}@{file}:{line}", &n),
            "T1@stop_times.txt:42"
        );
        assert_eq!(fill("[{observed_value}]", &n), "[25:1:00]");
        // Absent optional field → empty, matching the UI's `?? ''`.
        assert_eq!(fill("<{expected_value}>", &n), "<>");
        // Unknown key → empty; non-placeholder braces survive verbatim.
        assert_eq!(fill("{nope}", &n), "");
        assert_eq!(fill("{not a key}", &n), "{not a key}");
        assert_eq!(fill("no braces", &n), "no braces");
        assert_eq!(fill("unclosed {brace", &n), "unclosed {brace");
    }

    #[test]
    fn details_shadow_the_fixed_fields() {
        let mut n = notice();
        n.details = Some(std::collections::BTreeMap::from([(
            "entity_id".to_string(),
            "override".to_string(),
        )]).into());
        assert_eq!(fill("{entity_id}", &n), "override");
    }

    #[test]
    fn jp_profile_selects_the_profile_specific_message() {
        let translator = Translator::new(Lang::En).unwrap().unwrap();
        let mut n = notice();
        n.rule_id = "JPN_006".to_string();
        n.details = Some(std::collections::BTreeMap::from([(
            "jp_profile".to_string(),
            "V4".to_string(),
        )]).into());
        translator.translate(&mut n);

        assert!(n.message.contains("GTFS-JP v4"), "{}", n.message);
        assert!(n.message.contains("cannot be represented"), "{}", n.message);
    }

    #[test]
    fn timepoint_missing_field_variants_translate_without_a_profile() {
        for lang in [Lang::En, Lang::Ja, Lang::Fr] {
            let translator = Translator::new(lang).unwrap().unwrap();
            let mut n = notice();
            n.rule_id = "STM_047".to_string();
            n.entity_id = Some("T1".to_string());
            n.details = Some(std::collections::BTreeMap::from([(
                "message_variant".to_string(),
                "missing_departure".to_string(),
            )]).into());
            translator.translate(&mut n);

            let key = "STM_047.missing_departure";
            assert_eq!(
                n.message,
                translator
                    .lookup(|d| &d.messages, key)
                    .unwrap()
                    .replace("{entity_id}", "T1",)
            );
            assert_eq!(
                n.remediation.as_ref(),
                translator.lookup(|d| &d.remediations, key).unwrap()
            );
        }
    }

    #[test]
    fn jp_manual_review_and_aggregate_templates_are_used_in_all_locales() {
        for lang in [Lang::En, Lang::Ja, Lang::Fr] {
            let translator = Translator::new(lang).unwrap().unwrap();
            let mut n = notice();
            n.rule_id = "JPN_006".into();
            n.details = Some(std::collections::BTreeMap::from([
                ("jp_profile".into(), "V4".into()),
                ("message_variant".into(), "missing_review".into()),
            ]).into());
            translator.translate(&mut n);
            let specific = translator
                .lookup(|d| &d.messages, "JPN_006.v4.missing_review")
                .unwrap();
            assert_eq!(n.message, specific);
            assert_eq!(
                n.remediation.as_ref(),
                translator
                    .lookup(|d| &d.remediations, "JPN_006.v4.missing_review")
                    .unwrap()
            );
            n.rule_id = "JPN_029".into();
            n.field = Some("stop_headsign".into());
            n.details = Some(std::collections::BTreeMap::from([
                ("jp_profile".into(), "v4".into()),
                ("message_variant".into(), "aggregate".into()),
                ("table_name".into(), "stop_times".into()),
                ("source_value".into(), "渋谷".into()),
                ("affected_records".into(), "18426".into()),
                (
                    "example_record_ids".into(),
                    "trip_id=T1,stop_sequence=2".into(),
                ),
            ]).into());
            translator.translate(&mut n);
            for part in [
                "18426",
                "渋谷",
                "stop_times.stop_headsign",
                "trip_id=T1,stop_sequence=2",
            ] {
                assert!(n.message.contains(part), "{:?}: {}", lang, n.message);
            }
        }
    }
}
