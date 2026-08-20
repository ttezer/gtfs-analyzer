//! English notice text for the public SDK build.
//!
//! The validator pipeline keeps Turkish as its native text because the Analyzer
//! UI owns the locale selection. The SDK enables `sdk-en` and translates the
//! public notice fields at the WASM boundary, without shipping a separate
//! locale file in the npm package.

use std::collections::HashMap;
use std::sync::OnceLock;

use gtfs_core::Notice;
use serde::Deserialize;

const EN_JSON: &str = include_str!(concat!(env!("OUT_DIR"), "/en_locale.json"));

#[derive(Debug, Deserialize)]
struct Dictionary {
    messages: HashMap<String, String>,
    remediations: HashMap<String, String>,
    titles: HashMap<String, String>,
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
            notice.title = title.clone();
        }
        if let Some(template) = dictionary.messages.get(&notice.rule_id) {
            notice.message = fill(template, notice);
        }
        if let Some(remediation) = dictionary.remediations.get(&notice.rule_id) {
            notice.remediation = remediation.clone();
        }
    }
}

/// Substitutes `{field}` placeholders from the notice, matching the UI and CLI
/// locale implementations. Unknown placeholders resolve to an empty string.
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
            title: "Türkçe başlık".to_string(),
            message: "Türkçe mesaj".to_string(),
            remediation: "Türkçe çözüm".to_string(),
            blocks: Vec::new(),
            base_effort: 1,
            service_id: None,
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
    }
}
