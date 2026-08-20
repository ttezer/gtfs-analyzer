//! Notice text localisation for the CLI — the Rust counterpart of
//! `ui/src/i18n.ts`.
//!
//! The pipeline emits Turkish text natively, so `tr` needs no dictionary. `en`
//! and `ja` are `{placeholder}` templates keyed by rule id, filled from the
//! notice's own fields exactly like the UI does. Fallback chain, also mirrored
//! from the UI: requested locale → English → the pipeline's Turkish text.
//!
//! The dictionaries are derived from the TypeScript locales by
//! `ui/scripts/export-locales.mjs` (`npm run locales:export`); the locales stay
//! the single source of truth and `locale-parity.test.ts` fails on drift.

use std::collections::HashMap;

use clap::ValueEnum;
use gtfs_core::Notice;
use serde::Deserialize;

const EN_JSON: &str = include_str!("../locales/en.json");
const JA_JSON: &str = include_str!("../locales/ja.json");

#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
pub enum LangArg {
    /// Turkish — the pipeline's native text, no translation applied.
    Tr,
    En,
    Ja,
}

#[derive(Debug, Deserialize)]
struct Dictionary {
    messages: HashMap<String, String>,
    remediations: HashMap<String, String>,
    titles: HashMap<String, String>,
}

impl Dictionary {
    fn parse(raw: &str, lang: &str) -> Result<Self, String> {
        serde_json::from_str(raw)
            .map_err(|err| format!("embedded '{lang}' locale is not readable: {err}"))
    }
}

/// Absent for `--lang tr`: the notices already carry Turkish text.
pub struct Translator {
    primary: Dictionary,
    /// English, consulted when the primary locale lacks the rule.
    fallback: Option<Dictionary>,
}

impl Translator {
    pub fn new(lang: LangArg) -> Result<Option<Self>, String> {
        match lang {
            LangArg::Tr => Ok(None),
            LangArg::En => Ok(Some(Self {
                primary: Dictionary::parse(EN_JSON, "en")?,
                fallback: None,
            })),
            LangArg::Ja => Ok(Some(Self {
                primary: Dictionary::parse(JA_JSON, "ja")?,
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
            notice.title = title.to_string();
        }
        if let Some(template) = self.lookup(|d| &d.messages, &notice.rule_id) {
            notice.message = fill(template, notice);
        }
        if let Some(remediation) = self.lookup(|d| &d.remediations, &notice.rule_id) {
            notice.remediation = remediation.to_string();
        }
    }

    /// Registry title for the `rules` subcommand, which has no notice context.
    pub fn rule_title<'a>(&'a self, rule_id: &str, fallback: &'a str) -> &'a str {
        self.lookup(|d| &d.titles, rule_id).unwrap_or(fallback)
    }
}

/// Substitutes `{field}` placeholders from the notice, like the UI's
/// `tpl.replace(/\{(\w+)\}/g, …)`. Unknown placeholders resolve to an empty
/// string; a brace that is not a `\w+` placeholder is left untouched.
fn fill(template: &str, notice: &Notice) -> String {
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

        out.push_str(&resolve(key, notice));
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
    use gtfs_core::{EntityType, RuleClass, Severity};

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
            title: "departure_time geçersiz format".to_string(),
            message: "Türkçe mesaj".to_string(),
            remediation: "Türkçe çözüm".to_string(),
            blocks: Vec::new(),
            base_effort: 1,
            service_id: None,
        }
    }

    #[test]
    fn turkish_needs_no_dictionary() {
        assert!(Translator::new(LangArg::Tr).unwrap().is_none());
    }

    #[test]
    fn english_rewrites_title_message_and_remediation() {
        let translator = Translator::new(LangArg::En).unwrap().unwrap();
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
        assert_ne!(n.title, "departure_time geçersiz format");
        assert_ne!(n.remediation, "Türkçe çözüm");
    }

    #[test]
    fn unknown_rule_keeps_the_pipeline_text() {
        let translator = Translator::new(LangArg::En).unwrap().unwrap();
        let mut n = notice();
        n.rule_id = "NOPE_999".to_string();
        translator.translate(&mut n);

        assert_eq!(n.message, "Türkçe mesaj");
        assert_eq!(n.title, "departure_time geçersiz format");
    }

    #[test]
    fn japanese_falls_back_to_english_then_turkish() {
        let translator = Translator::new(LangArg::Ja).unwrap().unwrap();
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
        )]));
        assert_eq!(fill("{entity_id}", &n), "override");
    }

    #[test]
    fn registry_titles_cover_every_rule() {
        for lang in [LangArg::En, LangArg::Ja] {
            let translator = Translator::new(lang).unwrap().unwrap();
            for meta in gtfs_rules::RULES {
                assert_ne!(
                    translator.rule_title(meta.id, "MISSING"),
                    "MISSING",
                    "{:?} locale has no title for {}",
                    lang,
                    meta.id
                );
            }
        }
    }

    /// English coverage gate. The dictionaries translate by rule id with `if let Some(..)`,
    /// so a registered rule that is absent from a table falls back SILENTLY to the
    /// pipeline's Turkish text — invisible unless a feed happens to fire that rule.
    /// The public `gtfs-sdk` build has no other text source, so `en` must be total.
    #[test]
    fn every_registered_rule_resolves_in_english_dictionary() {
        // `en` only: it is the public SDK's single text source, and every other
        // locale falls back to it. A gap here degrades to Turkish; a gap in `ja`
        // degrades to English, which is a translation debt rather than a leak.
        for (lang, raw) in [("en", EN_JSON)] {
            let dict = Dictionary::parse(raw, lang).expect("locale parses");
            let mut missing: Vec<String> = Vec::new();
            for rule in gtfs_rules::RULES {
                for (section, table) in [
                    ("titles", &dict.titles),
                    ("messages", &dict.messages),
                    ("remediations", &dict.remediations),
                ] {
                    if !table.contains_key(rule.id) {
                        missing.push(format!("{section}/{}", rule.id));
                    }
                }
            }
            assert!(
                missing.is_empty(),
                "'{lang}' locale is missing {} entries: {missing:?}\n\
                 Add them to ui/src/locales/{lang}.ts, then run `npm run locales:export`.",
                missing.len()
            );
        }
    }

}
