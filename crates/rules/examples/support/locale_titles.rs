//! Shared by the `gen_rules` / `sync_cards` examples and the rules crate's doc tests.
//!
//! Rule titles live in the UI locales (`ui/src/locales/<lang>.ts`). The registry
//! `title` is English (issue #201); Turkish titles come from `tr.ts`.

use std::collections::BTreeMap;
use std::path::Path;

/// Reads the `ruleTitles: { 'ID': 'title', ... }` block from a locale .ts file.
pub fn parse_titles(ts: &str) -> BTreeMap<String, String> {
    let mut m = BTreeMap::new();
    let mut in_block = false;
    for line in ts.lines() {
        let t = line.trim();
        if !in_block {
            if t.starts_with("ruleTitles:") {
                in_block = true;
            }
            continue;
        }
        if t == "}," || t == "}" {
            break;
        }
        // Line format: `'ID': 'title',` — but alignment may use two spaces (`':  '`)
        // and some values may use double quotes (`"..."`). Parse without depending on spacing or quote style.
        if let Some((idp, valp)) = t.split_once(':') {
            let id = idp.trim().trim_matches(|c| c == '\'' || c == '"').trim();
            let v = valp.trim().trim_end_matches(',').trim();
            // Strip exactly one outer quote: a title may end with an escaped quote (`\"`).
            let quote = if v.starts_with('"') { '"' } else { '\'' };
            let val = v
                .strip_prefix(quote)
                .and_then(|inner| inner.strip_suffix(quote))
                .unwrap_or(v);
            if !id.is_empty() && id.chars().all(|c| c.is_ascii_alphanumeric() || c == '_') {
                m.insert(id.to_string(), unescape(val));
            }
        }
    }
    m
}

/// Resolves the escapes a TS string literal may carry (`\'`, `\"`, `\\`).
fn unescape(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut chars = s.chars();
    while let Some(c) = chars.next() {
        if c == '\\' {
            match chars.next() {
                Some(next @ ('\'' | '"' | '\\')) => out.push(next),
                Some(other) => {
                    out.push('\\');
                    out.push(other);
                }
                None => out.push('\\'),
            }
        } else {
            out.push(c);
        }
    }
    out
}

/// Turkish rule titles from `ui/src/locales/tr.ts`, relative to the repository root.
pub fn turkish_titles(repo_root: &Path) -> BTreeMap<String, String> {
    let path = repo_root.join("ui/src/locales/tr.ts");
    let ts = std::fs::read_to_string(&path)
        .unwrap_or_else(|err| panic!("cannot read {}: {err}", path.display()));
    parse_titles(&ts)
}
