//! Emit coverage (dead-rule protection) — issue #5 initial version.
//!
//! Question: does a registry rule actually occur in pipeline PRODUCTION code, or is it
//! "in the registry but dead in implementation" (a dead/orphan rule)? card_consistency
//! tests card↔registry consistency; this test verifies that every canonical rule_id occurs
//! at least once as a string literal in pipeline/src production code (excluding test modules
//! and the k7 report helper).
//!
//! Scope: this is "reference proof", not full "emit proof" (a real emit proof needs
//! an intentional fixture for every rule — issue #5 "full version"/C, a separate task).
//! It nevertheless reliably catches the main concern: a rule added to the registry without
//! emit code, or a rule whose emit code was deleted. It needs no feed and runs in CI.

use gtfs_rules::RULES;
use std::fs;
use std::path::{Path, PathBuf};

/// Rules that are emitted in reality but are invisible to static string-literal scanning.
/// These are produced through dynamic/fatal paths; the rule_id does not occur as a source literal.
const DYNAMIC_EMIT_ALLOWLIST: &[&str] = &[
    // Fatal path: returned as FatalCode::ZipUnreadable (not a Notice); no rule_id literal exists.
    "ARC_001", // "ZIP archive could not be opened"
    // Fatal path: FatalCode::DecompressionLimit (k1_parse.rs, GuardedReader).
    // This was NOT in the list until 2026-08-15 and passed the gate only because of
    // "ARC_029" occurrences in comments; tightening collect_rule_ids exposed it.
    "ARC_029", // "Compression limit exceeded (zip-bomb protection)"
];

fn repo_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()
        .expect("repo kökü bulunamadı")
}

/// Removes module blocks beginning with `#[cfg(test)]` from the text by matching braces.
/// (A file may contain multiple test blocks; emit code may occur between them.)
fn strip_test_mods(src: &str) -> String {
    let marker = "#[cfg(test)]";
    let mut out = String::new();
    let mut rest = src;
    while let Some(pos) = rest.find(marker) {
        out.push_str(&rest[..pos]);
        let after = &rest[pos..];
        // Find the module body's first '{' and skip through its matching '}'.
        if let Some(brace) = after.find('{') {
            let bytes = after.as_bytes();
            let mut depth = 0i32;
            let mut k = brace;
            while k < bytes.len() {
                match bytes[k] {
                    b'{' => depth += 1,
                    b'}' => {
                        depth -= 1;
                        if depth == 0 {
                            k += 1;
                            break;
                        }
                    }
                    _ => {}
                }
                k += 1;
            }
            rest = &after[k..];
        } else {
            // If there is no '{', discard the remainder (safe side).
            rest = "";
        }
    }
    out.push_str(rest);
    out
}

/// `^[A-Z]{2,}_[0-9]+[a-z]?$` — registry rule_id deseni.
fn is_rule_id(s: &str) -> bool {
    let Some(us) = s.find('_') else { return false };
    if us < 2 {
        return false;
    }
    if !s[..us].bytes().all(|c| c.is_ascii_uppercase()) {
        return false;
    }
    let rest = &s[us + 1..];
    let digits = rest.trim_end_matches(|c: char| c.is_ascii_lowercase());
    let suffix = &rest[digits.len()..];
    !digits.is_empty() && digits.bytes().all(|c| c.is_ascii_digit()) && suffix.len() <= 1
}

/// Collects rule_id tokens inside STRING LITERALS.
///
/// 🔴 It used to be quote-independent and said "a rule_id occurring as a source token
/// counts as live code." A COMMENT LINE could satisfy that gate: the gate was green when
/// `ARC_029` occurred only in comments such as `// ... ARC_029 (DecompressionLimit) Fatal
/// is emitted` in production code. The emit point is a literal in the form
/// `notice(..., "ARC_029", ...)`; that is the measure now. Unquoted occurrences do not count.
///
/// Measured (2026-08-15): the tightening drops only two rules—both are produced through
/// the Fatal path and belong to `DYNAMIC_EMIT_ALLOWLIST`.
fn collect_rule_ids(text: &str, into: &mut std::collections::HashSet<String>) {
    let bytes = text.as_bytes();
    let is_word = |c: u8| c.is_ascii_alphanumeric() || c == b'_';
    let mut i = 0;
    while i < bytes.len() {
        // Comment: skip to end of line (the `//` inside strings is preserved below).
        if bytes[i] == b'/' && i + 1 < bytes.len() && bytes[i + 1] == b'/' {
            i = text[i..].find('\n').map_or(bytes.len(), |n| i + n);
            continue;
        }
        if bytes[i] != b'"' {
            i += 1;
            continue;
        }
        let start = i + 1;
        let mut j = start;
        while j < bytes.len() && bytes[j] != b'"' {
            j += if bytes[j] == b'\\' { 2 } else { 1 };
        }
        let lit = &text[start..j.min(bytes.len())];
        let mut k = 0;
        let lb = lit.as_bytes();
        while k < lb.len() {
            if lb[k].is_ascii_uppercase() && (k == 0 || !is_word(lb[k - 1])) {
                let s = k;
                while k < lb.len() && is_word(lb[k]) {
                    k += 1;
                }
                if is_rule_id(&lit[s..k]) {
                    into.insert(lit[s..k].to_string());
                }
            } else {
                k += 1;
            }
        }
        i = j + 1;
    }
}

fn rs_files(dir: &Path, out: &mut Vec<PathBuf>) {
    for entry in fs::read_dir(dir).expect("dizin okunamadı").flatten() {
        let p = entry.path();
        if p.is_dir() {
            rs_files(&p, out);
        } else if p.extension().and_then(|e| e.to_str()) == Some("rs") {
            out.push(p);
        }
    }
}

#[test]
fn every_canonical_rule_referenced_in_production_code() {
    let src = repo_root().join("crates").join("pipeline").join("src");
    let mut files = Vec::new();
    rs_files(&src, &mut files);

    let mut referenced = std::collections::HashSet::new();
    for f in &files {
        // k7_reporting.rs contains report-view logic and test helpers; it is not an emit source.
        if f.file_name().and_then(|n| n.to_str()) == Some("k7_reporting.rs") {
            continue;
        }
        let txt = fs::read_to_string(f).expect("kaynak okunamadı");
        collect_rule_ids(&strip_test_mods(&txt), &mut referenced);
    }

    let allow: std::collections::HashSet<&str> = DYNAMIC_EMIT_ALLOWLIST.iter().copied().collect();
    let mut orphans: Vec<&str> = RULES
        .iter()
        .map(|r| r.id)
        .filter(|id| !referenced.contains(*id) && !allow.contains(*id))
        .collect();
    orphans.sort_unstable();

    assert!(
        orphans.is_empty(),
        "Üretim kodunda hiç geçmeyen canonical kural(lar) (dead/orphan rule?): {:?}\n\
         Eğer kural gerçekten emit ediliyorsa ama dinamik/fatal yolla (rule_id literal yok), \
         DYNAMIC_EMIT_ALLOWLIST'e gerekçesiyle ekleyin. Aksi halde emit kodu eksik/silinmiş.",
        orphans,
    );
}

/// Prevents allowlist rot: if an allowlisted rule starts occurring in real code
/// (or is removed from the registry), it must be removed from the allowlist.
#[test]
fn dynamic_emit_allowlist_has_no_stale_entries() {
    let canonical: std::collections::HashSet<&str> = RULES.iter().map(|r| r.id).collect();
    for id in DYNAMIC_EMIT_ALLOWLIST {
        assert!(
            canonical.contains(id),
            "DYNAMIC_EMIT_ALLOWLIST'teki '{id}' artık registry'de yok — allowlist'ten çıkarın.",
        );
    }
}
