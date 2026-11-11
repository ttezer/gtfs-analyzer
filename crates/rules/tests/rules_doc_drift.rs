//! RULES.md / RULES.en.md / RULES.ja.md ↔ `gtfs_rules::RULES` drift gate.
//!
//! These tables are GENERATED from the registry by the `gen_rules` example, but nothing
//! forced regeneration: when the registry changed and `gen_rules` was not run, the published
//! tables silently became stale. A real case (caught on 2026-07-16): the tables were generated
//! at `8b4fae3`, then `60e53b0` (TRP_017 → Quality, ATR_009 title fix) and `d24848f`
//! (XFL_020/021 Spec → Quality) changed the registry; all three RULES files published the wrong
//! class for two commits. `card_consistency` linked cards to the registry, but no test linked
//! the RULES files.
//!
//! This test deliberately does NOT reuse `gen_rules` code: it parses the published table and
//! compares it with the registry. A bug in the generator's own logic would be invisible to a
//! test that called that logic—an independent assertion is required.
//!
//! If red: `cargo run -p gtfs-rules --example gen_rules`

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use gtfs_rules::RULES;

#[path = "../examples/support/locale_titles.rs"]
mod locale_titles;

fn repo_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()
        .expect("repo kökü bulunamadı")
}

/// Table row: `| ARC_001 | title | KRİTİK | Spec |`
#[derive(Debug, PartialEq)]
struct Row {
    title: String,
    severity: String,
    class: String,
}

/// Splits a Markdown table row on UNESCAPED `|` characters and resolves `\|` escapes.
///
/// `gen_rules` escapes `|` in titles as `\|` (for example, GEO_016:
/// "Stop near Null Island (\|lat\|<1 OR \|lon\|<1)"). A naive `split('|')`
/// would split such rows into more than four cells and silently discard the row—skipping
/// exactly the rules it is supposed to check.
fn split_cells(line: &str) -> Vec<String> {
    let mut cells: Vec<String> = Vec::new();
    let mut cur = String::new();
    let mut chars = line.chars().peekable();
    while let Some(c) = chars.next() {
        match c {
            '\\' if chars.peek() == Some(&'|') => {
                chars.next();
                cur.push('|'); // Escape resolved: part of the cell content.
            }
            '|' => {
                cells.push(cur.trim().to_string());
                cur.clear();
            }
            _ => cur.push(c),
        }
    }
    cells.push(cur.trim().to_string());
    // `| a | b |` → drop the leading and trailing empty cells.
    if cells.first().is_some_and(String::is_empty) {
        cells.remove(0);
    }
    if cells.last().is_some_and(String::is_empty) {
        cells.pop();
    }
    cells
}

/// Parses all rule rows in a RULES file as ID → Row.
/// Skip the header (`| Kural | ...`) and separator (`|---|`) rows; because rule IDs use
/// the `GRP_NNN` form, other tables are not accidentally captured.
fn parse_rows(md: &str) -> BTreeMap<String, Row> {
    let mut out = BTreeMap::new();
    for line in md.lines() {
        let t = line.trim();
        if !t.starts_with('|') {
            continue;
        }
        let cells = split_cells(t);
        if cells.len() != 4 {
            continue;
        }
        let id = cells[0].as_str();
        let looks_like_rule_id = id.split_once('_').is_some_and(|(g, n)| {
            !g.is_empty()
                && g.chars().all(|c| c.is_ascii_uppercase())
                && !n.is_empty()
                && n.chars().all(|c| c.is_ascii_alphanumeric())
        });
        if !looks_like_rule_id {
            continue;
        }
        out.insert(
            id.to_string(),
            Row {
                title: cells[1].clone(), // Escapes were resolved in split_cells.
                severity: cells[2].clone(),
                class: cells[3].clone(),
            },
        );
    }
    out
}

/// Expected spelling of the registry enum in each language. An INDEPENDENT restatement of
/// the table in gen_rules—a deliberate copy: the test must fail if either one changes.
fn expected_severity(debug_name: &str, lang: usize) -> &'static str {
    match debug_name {
        "Kritik" => ["KRİTİK", "CRITICAL", "致命的", "CRITIQUE"][lang],
        "Yuksek" => ["YÜKSEK", "HIGH", "高", "ÉLEVÉE"][lang],
        "Orta" => ["ORTA", "MEDIUM", "中", "MOYENNE"][lang],
        "Dusuk" => ["DÜŞÜK", "LOW", "低", "FAIBLE"][lang],
        "Bilgi" => ["BİLGİ", "INFO", "情報", "INFO"][lang],
        other => panic!("bilinmeyen severity: {other}"),
    }
}
fn expected_class(debug_name: &str, lang: usize) -> &'static str {
    match debug_name {
        "Spec" => ["Spec", "Spec", "仕様", "Spec"][lang],
        "Interop" => ["Interop", "Interop", "相互運用", "Interop"][lang],
        "Quality" => ["Quality", "Quality", "品質", "Quality"][lang],
        "Analytics" => ["Analytics", "Analytics", "分析", "Analytics"][lang],
        other => panic!("bilinmeyen rule_class: {other}"),
    }
}

fn read(file: &str) -> String {
    let p = repo_root().join(file);
    std::fs::read_to_string(&p).unwrap_or_else(|e| panic!("{} okunamadı: {e}", p.display()))
}

/// For every language: the published rule set equals the registry set, and each row's
/// severity/class columns match the registry.
#[test]
fn rules_docs_match_registry() {
    let mut problems: Vec<String> = vec![];

    for (lang, file) in ["RULES.md", "RULES.en.md", "RULES.ja.md", "RULES.fr.md"]
        .iter()
        .enumerate()
    {
        let rows = parse_rows(&read(file));

        for rule in RULES {
            let Some(row) = rows.get(rule.id) else {
                problems.push(format!("{file}: '{}' tabloda YOK", rule.id));
                continue;
            };
            let want_sev = expected_severity(&format!("{:?}", rule.severity), lang);
            if row.severity != want_sev {
                problems.push(format!(
                    "{file}: {} önem '{}' ≠ registry '{}'",
                    rule.id, row.severity, want_sev
                ));
            }
            let want_cls = expected_class(&format!("{:?}", rule.rule_class), lang);
            if row.class != want_cls {
                problems.push(format!(
                    "{file}: {} sınıf '{}' ≠ registry '{}'",
                    rule.id, row.class, want_cls
                ));
            }
        }

        // A rule removed from the registry must not remain in the table.
        for id in rows.keys() {
            if !RULES.iter().any(|r| r.id == id) {
                problems.push(format!("{file}: '{id}' tabloda var ama registry'de YOK"));
            }
        }
    }

    assert!(
        problems.is_empty(),
        "RULES tabloları registry ile uyuşmuyor ({} sorun) — düzeltmek için:\n  \
         cargo run -p gtfs-rules --example gen_rules\n\n{}",
        problems.len(),
        problems.join("\n")
    );
}

/// Turkish table titles come from `ui/src/locales/tr.ts` (the registry title is English) → they must match exactly.
/// (EN/JA titles come from locale files and are checked in a separate test.)
/// Turkish-leak gate for translated RULES files.
///
/// `gen_rules` writes the registry's TURKISH text when it cannot find a title in the locale.
/// Because the title check above covers only `RULES.md`, a partially translated locale could
/// silently produce Turkish rows in `RULES.en/ja/fr.md`. The characters `ş ğ İ ı` occur in none
/// of those three languages.
#[test]
fn translated_rules_docs_carry_no_turkish_titles() {
    let mut problems: Vec<String> = vec![];

    for file in ["RULES.en.md", "RULES.ja.md", "RULES.fr.md"] {
        for (id, row) in parse_rows(&read(file)) {
            if row.title.contains(['ş', 'ğ', 'İ', 'ı', 'Ş', 'Ğ']) {
                problems.push(format!("{file} · {id}: '{}'", row.title));
            }
        }
    }

    assert!(
        problems.is_empty(),
        "çevrilmiş RULES dosyalarında Türkçe başlık ({} satır):\n  {}\n\
         Eksik başlıkları ui/src/locales/<lang>.ts içine ekleyip \
         `cargo run -p gtfs-rules --example gen_rules` çalıştırın.",
        problems.len(),
        problems.join("\n  ")
    );
}

#[test]
fn turkish_rules_doc_titles_match_turkish_locale_titles() {
    let rows = parse_rows(&read("RULES.md"));
    let tr_titles = locale_titles::turkish_titles(&repo_root());
    let mut problems: Vec<String> = vec![];

    for rule in RULES {
        if let Some(row) = rows.get(rule.id) {
            let tr_title = tr_titles.get(rule.id).map_or(rule.title, String::as_str);
            if row.title != tr_title {
                problems.push(format!(
                    "{}: başlık '{}' ≠ tr.ts başlığı '{}'",
                    rule.id, row.title, tr_title
                ));
            }
        }
    }

    assert!(
        problems.is_empty(),
        "RULES.md başlıkları tr.ts başlıklarıyla uyuşmuyor ({} sorun) — düzeltmek için:\n  \
         cargo run -p gtfs-rules --example gen_rules\n\n{}",
        problems.len(),
        problems.join("\n")
    );
}

/// The SEO text in `ui/index.html` (meta description, og:description, JSON-LD) carried the rule
/// count MANUALLY and no gate checked it: until the 0.14.0 release it said `526 rules` while the
/// catalog had reached 624. This is the first text users see.
#[test]
fn ui_meta_rule_count_matches_registry() {
    let n = RULES.len().to_string();
    let html = read("ui/index.html");
    let mut seen = 0usize;
    for (idx, _) in html.match_indices(" rules").chain(html.match_indices(" validation and quality rules")) {
        let head = &html[..idx];
        let digits: String = head
            .chars()
            .rev()
            .take_while(char::is_ascii_digit)
            .collect::<Vec<_>>()
            .into_iter()
            .rev()
            .collect();
        if digits.is_empty() {
            continue;
        }
        seen += 1;
        assert_eq!(
            digits, n,
            "ui/index.html: SEO metni {digits} kural diyor, registry {n} taşıyor (bayat sayı)"
        );
    }
    assert!(seen >= 2, "ui/index.html'de kural sayısı taşıyan metin bulunamadı (desen değişti mi?)");
}

/// The rule and group counts in the table must match the counts in the introduction
/// ("537 rules, 37 groups"). This is the first thing to drift in a manually edited table.
#[test]
fn rules_doc_intro_counts_match_registry() {
    let n = RULES.len();
    let mut groups: Vec<&str> = RULES
        .iter()
        .map(|r| r.id.split('_').next().unwrap_or(r.id))
        .collect();
    groups.sort_unstable();
    groups.dedup();
    let g = groups.len();

    for file in ["RULES.md", "RULES.en.md", "RULES.ja.md", "RULES.fr.md"] {
        let md = read(file);
        let intro: String = md.lines().take(10).collect::<Vec<_>>().join("\n");
        assert!(
            intro.contains(&n.to_string()),
            "{file}: giriş metni kural sayısı {n}'i içermiyor (bayat sayı?) — \
             cargo run -p gtfs-rules --example gen_rules"
        );
        assert!(
            intro.contains(&g.to_string()),
            "{file}: giriş metni grup sayısı {g}'yi içermiyor (bayat sayı?) — \
             cargo run -p gtfs-rules --example gen_rules"
        );
    }
}
