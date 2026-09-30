//! `--lang` for the CLI. Translation itself lives in `gtfs_core::i18n`, shared
//! with the Python package.

use clap::ValueEnum;
use gtfs_core::i18n::Lang;

pub use gtfs_core::i18n::Translator;

#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
pub enum LangArg {
    /// Turkish — the pipeline's native text, no translation applied.
    Tr,
    En,
    Ja,
    Fr,
}

impl From<LangArg> for Lang {
    fn from(arg: LangArg) -> Self {
        match arg {
            LangArg::Tr => Lang::Tr,
            LangArg::En => Lang::En,
            LangArg::Ja => Lang::Ja,
            LangArg::Fr => Lang::Fr,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn registry_titles_cover_every_rule() {
        for lang in [Lang::En, Lang::Ja, Lang::Fr] {
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

    /// Coverage gate for the locales that are promised to be COMPLETE.
    ///
    /// The dictionaries translate by rule id with `if let Some(..)`, so a registered
    /// rule that is absent from a table falls back SILENTLY — invisible unless a feed
    /// happens to fire that rule. Two different reasons put a locale in this gate:
    ///
    /// * `en` — LEAK gate. It is the public `gtfs-sdk` build's single text source and
    ///   every other locale falls back to it, so a gap here degrades all the way to
    ///   the pipeline's Turkish text.
    /// * `fr` — POLICY gate. A gap degrades to English, which is harmless on its own;
    ///   the gate exists because `fr` was committed to as a complete translation and
    ///   nothing else would hold that promise. Without it a newly added rule would
    ///   quietly skip French and the locale would erode into a partial one.
    ///
    /// `ja` is deliberately OUTSIDE this gate: it is knowingly partial (messages and
    /// remediations fall back to English) and that is accepted translation debt.
    #[test]
    fn every_registered_rule_resolves_in_complete_dictionaries() {
        for (name, lang) in [("en", Lang::En), ("fr", Lang::Fr)] {
            let translator = Translator::new(lang).unwrap().unwrap();
            let missing: Vec<String> = gtfs_rules::RULES
                .iter()
                .flat_map(|rule| {
                    translator
                        .missing_sections(rule.id)
                        .into_iter()
                        .map(move |section| format!("{section}/{}", rule.id))
                })
                .collect();
            assert!(
                missing.is_empty(),
                "'{name}' locale is missing {} entries: {missing:?}\n\
                 Add them to ui/src/locales/{name}.ts, then run `npm run locales:export`.",
                missing.len()
            );
        }
    }
}
