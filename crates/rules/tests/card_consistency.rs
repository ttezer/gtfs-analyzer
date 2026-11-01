//! Rule card ↔ registry ↔ code consistency test.
//!
//! Ensures that `docs/rules/<GRUP>/<ID>.md` cards stay synchronized with `gtfs_rules::RULES`
//! and the source lines they reference. CI fails if the document and code diverge.
//!
//! Scope:
//!  1. Every registry rule has a card file (and there are no orphan cards).
//!  2. R9 "Rule message" == registry `title` (the TEMPLATE rule).
//!  3. Metadata severity/class == registry severity/rule_class.
//!  4. Every `crates/...rs#Lnnn` reference in "Code reference" contains the rule ID
//!     within a ±3-line window (detecting line drift / incorrect references).

use gtfs_rules::RULES;
use std::collections::HashSet;
use std::fs;
use std::path::{Path, PathBuf};

fn repo_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()
        .expect("repository root not found")
}

fn cards_dir() -> PathBuf {
    repo_root().join("docs").join("rules")
}

fn group_of(id: &str) -> &str {
    id.split('_').next().unwrap_or(id)
}

fn card_path(id: &str) -> PathBuf {
    cards_dir().join(group_of(id)).join(format!("{id}.md"))
}

/// Maps the card's severity display to the enum Debug string.
fn severity_display_to_debug(card_val: &str) -> &str {
    match card_val.trim() {
        "Kritik" => "Kritik",
        "Yüksek" => "Yuksek",
        "Orta" => "Orta",
        "Düşük" => "Dusuk",
        "Bilgi" => "Bilgi",
        other => other,
    }
}

/// Returns the value from the "| Label | Value |" row in the metadata table.
fn kunye_value(text: &str, label: &str) -> Option<String> {
    for line in text.lines() {
        let l = line.trim();
        if l.starts_with('|') {
            // In a Markdown table, an escaped pipe (\|) is not a column separator—preserve it.
            let protected = l.replace("\\|", "\u{1}");
            let cells: Vec<String> = protected
                .trim_matches('|')
                .split('|')
                .map(|c| c.trim().replace('\u{1}', "\\|"))
                .collect();
            if cells.len() >= 2 && cells[0] == label {
                return Some(cells[1].clone());
            }
        }
    }
    None
}

/// Returns the message from the R9 "Rule message:" line.
fn r9_message(text: &str) -> Option<String> {
    const MARK: &str = "**Kural mesajı:**";
    for line in text.lines() {
        if let Some(idx) = line.find(MARK) {
            return Some(line[idx + MARK.len()..].trim().to_string());
        }
    }
    None
}

/// Extracts all (source_path, line_number) code references from a card (`crates/...rs#Lnnn`).
fn code_refs(text: &str) -> Vec<(String, usize)> {
    let mut out = Vec::new();
    for line in text.lines() {
        let mut rest = line;
        while let Some(pos) = rest.find(".rs#L") {
            let head = &rest[..pos + 3]; // Include ".rs".
            if let Some(cstart) = head.rfind("crates/") {
                let path = head[cstart..].to_string();
                let after = &rest[pos + 5..];
                let num: String = after.chars().take_while(|c| c.is_ascii_digit()).collect();
                if let Ok(n) = num.parse::<usize>() {
                    out.push((path, n));
                }
            }
            rest = &rest[pos + 5..];
        }
    }
    out
}

#[test]
fn every_rule_has_card_with_matching_metadata() {
    let mut problems: Vec<String> = Vec::new();

    for rule in RULES {
        let path = card_path(rule.id);
        let text = match fs::read_to_string(&path) {
            Ok(t) => t,
            Err(_) => {
                problems.push(format!(
                    "{}: KART DOSYASI YOK ({})",
                    rule.id,
                    path.display()
                ));
                continue;
            }
        };

        // R9 message == title.
        match r9_message(&text) {
            Some(msg) if msg == rule.title => {}
            Some(msg) => problems.push(format!(
                "{}: R9 mesajı ≠ title\n    kart:     '{}'\n    registry: '{}'",
                rule.id, msg, rule.title
            )),
            None => problems.push(format!("{}: R9 'Kural mesajı:' satırı bulunamadı", rule.id)),
        }

        // Severity metadata == severity.
        let exp_sev = format!("{:?}", rule.severity);
        match kunye_value(&text, "Önem") {
            Some(v) if severity_display_to_debug(&v) == exp_sev => {}
            Some(v) => problems.push(format!(
                "{}: Önem uyuşmuyor (kart '{}' → '{}', registry '{}')",
                rule.id,
                v,
                severity_display_to_debug(&v),
                exp_sev
            )),
            None => problems.push(format!("{}: künye 'Önem' satırı yok", rule.id)),
        }

        // Class metadata == rule_class.
        let exp_cls = format!("{:?}", rule.rule_class);
        match kunye_value(&text, "Sınıf") {
            Some(v) if v == exp_cls => {}
            Some(v) => problems.push(format!(
                "{}: Sınıf uyuşmuyor (kart '{}', registry '{}')",
                rule.id, v, exp_cls
            )),
            None => problems.push(format!("{}: künye 'Sınıf' satırı yok", rule.id)),
        }
    }

    assert!(
        problems.is_empty(),
        "\n{} kart metadata sorunu:\n{}\n",
        problems.len(),
        problems.join("\n")
    );
}

/// Returns all R-report tokens (R1, R2, ...) in the text as a set.
fn r_tokens(s: &str) -> std::collections::BTreeSet<String> {
    let ch: Vec<char> = s.chars().collect();
    let mut set = std::collections::BTreeSet::new();
    let mut i = 0;
    while i < ch.len() {
        if ch[i] == 'R' && i + 1 < ch.len() && ch[i + 1].is_ascii_digit() {
            let mut t = String::from("R");
            let mut j = i + 1;
            while j < ch.len() && ch[j].is_ascii_digit() {
                t.push(ch[j]);
                j += 1;
            }
            set.insert(t);
            i = j;
        } else {
            i += 1;
        }
    }
    set
}

/// Returns all rule-ID tokens (XFL_001, DQ_005b ...) in the text as a set.
fn rule_id_tokens(s: &str) -> std::collections::BTreeSet<String> {
    let ch: Vec<char> = s.chars().collect();
    let mut set = std::collections::BTreeSet::new();
    let mut i = 0;
    while i < ch.len() {
        if ch[i].is_ascii_uppercase() {
            let mut j = i;
            while j < ch.len() && ch[j].is_ascii_uppercase() {
                j += 1;
            }
            let letters = j - i;
            if (2..=4).contains(&letters) && j < ch.len() && ch[j] == '_' {
                let mut k = j + 1;
                let dstart = k;
                while k < ch.len() && ch[k].is_ascii_digit() {
                    k += 1;
                }
                if k - dstart == 3 {
                    let mut tok: String = ch[i..k].iter().collect();
                    if k < ch.len() && ch[k].is_ascii_lowercase() {
                        tok.push(ch[k]);
                    }
                    set.insert(tok);
                }
            }
            i = j;
        } else {
            i += 1;
        }
    }
    set
}

#[test]
fn kunye_and_score_match_registry() {
    let mut problems: Vec<String> = Vec::new();

    for rule in RULES {
        let path = card_path(rule.id);
        let Ok(text) = fs::read_to_string(&path) else {
            continue;
        };

        // Score base == base_effort.
        if let Some(v) = kunye_value(&text, "Skor tabanı") {
            match v.trim().parse::<u8>() {
                Ok(n) if n == rule.base_effort => {}
                _ => problems.push(format!(
                    "{}: Skor tabanı '{}' ≠ base_effort {}",
                    rule.id, v, rule.base_effort
                )),
            }
        }

        // The no-score state is stated explicitly in a metadata row separate from base_effort.
        if rule.score_weight == 0.0 {
            match kunye_value(&text, "Skor ağırlığı") {
                Some(v) if v.trim() == "0 (no-score)" => {}
                Some(v) => problems.push(format!(
                    "{}: Skor ağırlığı '{}' ≠ 0 (no-score)",
                    rule.id, v
                )),
                None => problems.push(format!(
                    "{}: no-score kuralında 'Skor ağırlığı' satırı yok",
                    rule.id
                )),
            }
        }

        // Entity level == dedup_level.
        let exp_dedup = format!("{:?}", rule.dedup_level);
        if let Some(v) = kunye_value(&text, "Varlık") {
            if v.trim() != exp_dedup {
                problems.push(format!(
                    "{}: Varlık '{}' ≠ dedup_level '{}'",
                    rule.id, v, exp_dedup
                ));
            }
        }

        // Identity field == scope_key_field.
        if let Some(v) = kunye_value(&text, "Kimlik alanı") {
            let card = v.replace('`', "").replace("\\|", "|");
            let card = card.trim();
            let exp = rule.scope_key_field.unwrap_or("-");
            if card != exp {
                problems.push(format!(
                    "{}: Kimlik alanı '{}' ≠ scope_key '{}'",
                    rule.id, card, exp
                ));
            }
        }

        // Blocked rules == blocks (set).
        if let Some(v) = kunye_value(&text, "Bloke ettiği kurallar") {
            let card: std::collections::BTreeSet<String> = rule_id_tokens(&v);
            let exp: std::collections::BTreeSet<String> =
                rule.blocks.iter().map(|s| s.to_string()).collect();
            if card != exp {
                problems.push(format!(
                    "{}: Bloke ettiği {:?} ≠ blocks {:?}",
                    rule.id, card, exp
                ));
            }
        }

        // Visibility R-set == report_views.
        if let Some(v) = kunye_value(&text, "Görünürlük") {
            let card = r_tokens(&v);
            let exp: std::collections::BTreeSet<String> = rule
                .report_views
                .iter()
                .map(|r| format!("{:?}", r))
                .collect();
            if card != exp {
                problems.push(format!(
                    "{}: Görünürlük R-seti {:?} ≠ report_views {:?}",
                    rule.id, card, exp
                ));
            }
        }

        // severity.weight() == weight(severity).
        if let Some(idx) = text.find("severity.weight() =") {
            let after = &text[idx + "severity.weight() =".len()..];
            let num: String = after
                .trim_start()
                .chars()
                .take_while(|c| c.is_ascii_digit() || *c == '.')
                .collect();
            if let Ok(w) = num.parse::<f64>() {
                if (w - rule.severity.weight()).abs() > 1e-9 {
                    problems.push(format!(
                        "{}: severity.weight() yazımı {} ≠ gerçek {}",
                        rule.id,
                        w,
                        rule.severity.weight()
                    ));
                }
            }
        }
    }

    assert!(
        problems.is_empty(),
        "\n{} künye/skor uyuşmazlığı:\n{}\n",
        problems.len(),
        problems.join("\n")
    );
}

/// Threshold constant ↔ card check. Curated table: (rule_id, threshold value, source file).
/// The value must occur in both card and source; a changed code constant then stales the card → detected.
/// Only DISTINGUISHING values are included (common small numbers create noise).
/// Extend this table manually as new thresholds are added.
const THRESHOLDS: &[(&str, &str, &str)] = &[
    ("SHP_026", "5000", "crates/pipeline/src/k6_analytics.rs"),
    ("TRF_010", "3600", "crates/pipeline/src/k4_cross_ref.rs"),
    ("TRF_011", "2000", "crates/pipeline/src/k6_analytics.rs"),
    ("SHP_012", "500", "crates/pipeline/src/k6_analytics.rs"),
    ("VAT_001", "0.85", "crates/pipeline/src/k6_analytics.rs"),
    ("FIN_017", "730", "crates/pipeline/src/k6_analytics.rs"),
];

#[test]
fn threshold_constants_match_code() {
    let root = repo_root();
    let mut problems: Vec<String> = Vec::new();
    for (id, val, src_rel) in THRESHOLDS {
        let card = fs::read_to_string(card_path(id)).unwrap_or_default();
        let src = fs::read_to_string(root.join(src_rel)).unwrap_or_default();
        if !card.contains(val) {
            problems.push(format!(
                "{id}: kart '{val}' eşiğini içermiyor",
                id = id,
                val = val
            ));
        }
        if !src.contains(val) {
            problems.push(format!(
                "{id}: kaynak {src_rel} '{val}' içermiyor — kod sabiti değişmiş olabilir, kartı güncelle",
                id = id, src_rel = src_rel, val = val
            ));
        }
    }
    assert!(
        problems.is_empty(),
        "\nEşik uyuşmazlığı:\n{}\n",
        problems.join("\n")
    );
}

#[test]
fn no_orphan_cards() {
    let valid: HashSet<&str> = RULES.iter().map(|r| r.id).collect();
    let mut orphans: Vec<String> = Vec::new();
    let dir = cards_dir();
    for grp in fs::read_dir(&dir).expect("docs/rules okunamadı") {
        let grp = grp.unwrap().path();
        if !grp.is_dir() {
            continue;
        }
        for f in fs::read_dir(&grp).unwrap() {
            let f = f.unwrap().path();
            if f.extension().and_then(|e| e.to_str()) != Some("md") {
                continue;
            }
            let id = f.file_stem().unwrap().to_string_lossy().to_string();
            if !valid.contains(id.as_str()) {
                orphans.push(format!("{} (registry'de yok)", f.display()));
            }
        }
    }
    assert!(
        orphans.is_empty(),
        "\nYetim kart(lar):\n{}\n",
        orphans.join("\n")
    );
}

#[test]
fn code_references_point_to_rule() {
    let root = repo_root();
    let mut problems: Vec<String> = Vec::new();

    for rule in RULES {
        let path = card_path(rule.id);
        let text = match fs::read_to_string(&path) {
            Ok(t) => t,
            Err(_) => continue, // Missing files are reported by the other test.
        };
        for (src_rel, line_no) in code_refs(&text) {
            let src_path = root.join(&src_rel);
            let src = match fs::read_to_string(&src_path) {
                Ok(s) => s,
                Err(_) => {
                    problems.push(format!("{}: atıf dosyası yok '{}'", rule.id, src_rel));
                    continue;
                }
            };
            let lines: Vec<&str> = src.lines().collect();
            if line_no == 0 || line_no > lines.len() {
                problems.push(format!(
                    "{}: '{}#L{}' satır aralık dışı (dosya {} satır)",
                    rule.id,
                    src_rel,
                    line_no,
                    lines.len()
                ));
                continue;
            }
            // Generic/data-driven emit: if the rule ID never occurs as a literal in the file
            // (for example, rules generated by data-driven loops), content cannot be checked—
            // skip it while the line range is valid.
            if !src.contains(&format!("\"{}\"", rule.id)) {
                continue;
            }
            // Does rule.id occur within the ±3 window?
            let lo = line_no.saturating_sub(4); // 0-index, line_no-3 .. line_no+3 (1-index).
            let hi = (line_no + 3).min(lines.len());
            let found = lines[lo..hi].iter().any(|l| l.contains(rule.id));
            if !found {
                problems.push(format!(
                    "{}: '{}#L{}' ±3 penceresinde '{}' yok (satır içeriği: {:?})",
                    rule.id,
                    src_rel,
                    line_no,
                    rule.id,
                    lines.get(line_no - 1).unwrap_or(&"").trim()
                ));
            }
        }
    }

    assert!(
        problems.is_empty(),
        "\n{} kod-referansı sorunu (satır kayması olabilir):\n{}\n",
        problems.len(),
        problems.join("\n")
    );
}

/// TEMPLATE.md skeleton compliance (structural): checks that each card has its Decision/Fix line,
/// 9 metadata fields, and core section headings. It checks STRUCTURE, not prose.
/// Optional-by-nature blocks (Dependency, External tool mapping, GTFS spec reference) are NOT
/// required—they are not meaningful for every rule, and filler should not be generated.
#[test]
fn card_template_shape() {
    // (label, substring required in the card)
    let kunye = [
        "Grup",
        "Önem",
        "Sınıf",
        "Aşama",
        "Varlık",
        "Kimlik alanı",
        "Skor tabanı",
        "Görünürlük",
        "Bloke ettiği kurallar",
    ];
    // Core sections searched independently of heading level (## or ###).
    let sections = [
        "Tetikleme koşulu",
        "Yanlış pozitif / negatif",
        "Karışan komşular",
        "Mesaj & çözüm önerisi",
        "R9 Kural Mesajı",
        "Ölçülen alan / değer",
        "Kod değiştirirken minimum test matrisi",
        "Teknik ek",
        "Skora katkı",
        "Hangi raporlarda görünür",
        "Kod referansı",
    ];

    let mut problems: Vec<String> = Vec::new();
    for rule in RULES {
        let path = card_path(rule.id);
        let Ok(text) = fs::read_to_string(&path) else {
            problems.push(format!("{}: KART YOK", rule.id));
            continue;
        };
        let mut missing: Vec<String> = Vec::new();

        // Collect heading text (#/##/### does not matter)—level-agnostic matching.
        let headings: Vec<String> = text
            .lines()
            .filter(|l| l.trim_start().starts_with('#'))
            .map(|l| l.trim_start_matches('#').trim().to_string())
            .collect();

        // Decision line—bold (`**Karar:**`) or plain (`> Karar:`) is accepted;
        // the corpus mostly uses plain text, so we do not force formatting churn.
        if !text.contains("Karar:") {
            missing.push("Karar".into());
        }
        // Metadata fields.
        for k in kunye {
            if kunye_value(&text, k).is_none() {
                missing.push(format!("künye:{k}"));
            }
        }
        // Core sections (independent of heading level).
        for s in sections {
            if !headings.iter().any(|h| h.contains(s)) {
                missing.push(s.to_string());
            }
        }

        if !missing.is_empty() {
            problems.push(format!("{}: eksik → {}", rule.id, missing.join(", ")));
        }
    }

    assert!(
        problems.is_empty(),
        "\n{} kart TEMPLATE iskeletine uymuyor:\n{}\n",
        problems.len(),
        problems.join("\n")
    );
}

// ── CI condition #6: Spec rule cards require a field-level GTFS spec anchor ──────
//
// `Sınıf=Spec` is an authority claim → the card must carry an anchored link
// (`gtfs.org/...#...`) to the official GTFS Schedule Reference in its "GTFS spec reference"
// section. A bare file link or Spec without a reference is an unsupported claim. The allowlist
// is reduced card by card during Phase 4; once empty, the gate is fully active. Do NOT add entries.
const SPEC_ANCHOR_ALLOWLIST: &[&str] = &[];

/// Does a text section contain a `gtfs.org/...#...` URL with a section/field anchor?
fn has_gtfs_anchor(section: &str) -> bool {
    let mut rest = section;
    while let Some(pos) = rest.find("gtfs.org/") {
        let after = &rest[pos..];
        let end = after
            .find(|c: char| c.is_whitespace() || c == ')')
            .unwrap_or(after.len());
        if after[..end].contains('#') {
            return true;
        }
        rest = &after[end.max(1)..];
    }
    false
}

/// From the "### GTFS spec reference" heading to the next `###`/`## ` or end of file.
fn spec_ref_section(text: &str) -> &str {
    const H: &str = "### GTFS spec referansı";
    let Some(start) = text.find(H) else { return "" };
    let body = &text[start + H.len()..];
    let end = body
        .find("\n### ")
        .or_else(|| body.find("\n## "))
        .unwrap_or(body.len());
    &body[..end]
}

#[test]
fn spec_rules_cite_field_anchor() {
    let mut problems: Vec<&str> = Vec::new();
    for rule in RULES {
        if format!("{:?}", rule.rule_class) != "Spec" {
            continue;
        }
        if SPEC_ANCHOR_ALLOWLIST.contains(&rule.id) {
            continue;
        }
        let Ok(text) = fs::read_to_string(card_path(rule.id)) else {
            continue;
        };
        if !has_gtfs_anchor(spec_ref_section(&text)) {
            problems.push(rule.id);
        }
    }
    assert!(
        problems.is_empty(),
        "\n{} Spec kuralı kartında alan-düzeyi `gtfs.org/...#anchor` YOK \
         (kart 'GTFS spec referansı' bölümüne anchor'lı link ekle):\n{:?}\n",
        problems.len(),
        problems
    );
}
