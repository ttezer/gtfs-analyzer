//! Synchronizes rule cards with the registry and source (one-time maintenance tool).
//!
//! - Makes the R9 "Rule message:" line equal to the registry `title`.
//! - Makes the metadata `Önem`/`Sınıf` values equal to the registry severity/rule_class.
//!
//! Code references in cards link files without line numbers (since 2026-11-02), so there is
//! no line drift to fix; `card_consistency` checks that the linked files exist and emit the rule.
//!
//! Run: cargo run -p gtfs-rules --example sync_cards
//! The card_consistency test should then move closer to green.

use gtfs_core::ReportId;
use gtfs_rules::{RuleMeta, RULES};
use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};

fn views_token(views: &[ReportId]) -> &'static str {
    let s: BTreeSet<String> = views.iter().map(|r| format!("{r:?}")).collect();
    let m = |a: &[&str]| {
        a.iter()
            .map(|x| x.to_string())
            .collect::<BTreeSet<String>>()
    };
    if s == m(&["R1", "R2", "R5", "R9"]) {
        "VS_K"
    } else if s == m(&["R2", "R5", "R9"]) {
        "VS"
    } else if s == m(&["R2", "R5", "R8", "R9"]) {
        "VI"
    } else if s == m(&["R2", "R3", "R5", "R9"]) {
        "VA"
    } else if s == m(&["R2", "R4", "R5", "R8", "R9"]) {
        "VI_GEO"
    } else if s == m(&["R2", "R4", "R5", "R9"]) {
        "VS_GEO"
    } else if s == m(&["R2", "R3", "R4", "R5", "R9"]) {
        "VA_GEO"
    } else if s == m(&["R2", "R5", "R7", "R8", "R9"]) {
        "VI_ACC"
    } else if s == m(&["R2", "R3", "R5", "R7", "R9"]) {
        "VA_ACC"
    } else if s == m(&["R2", "R5", "R7", "R9"]) {
        "VS_ACC"
    } else {
        "?"
    }
}

fn fmt_views(rule: &RuleMeta) -> String {
    let rs: Vec<String> = rule
        .report_views
        .iter()
        .map(|r| format!("`{r:?}`"))
        .collect();
    format!("{} ({})", views_token(rule.report_views), rs.join(", "))
}

fn fmt_scope(rule: &RuleMeta) -> String {
    match rule.scope_key_field {
        Some(f) => format!("`{}`", f.replace('|', "\\|")),
        None => "-".to_string(),
    }
}

fn fmt_blocks(rule: &RuleMeta) -> String {
    if rule.blocks.is_empty() {
        "-".to_string()
    } else {
        rule.blocks
            .iter()
            .map(|b| format!("`{b}`"))
            .collect::<Vec<_>>()
            .join(", ")
    }
}

fn repo_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()
        .unwrap()
}
fn group_of(id: &str) -> &str {
    id.split('_').next().unwrap_or(id)
}
fn card_path(id: &str) -> PathBuf {
    repo_root()
        .join("docs/rules")
        .join(group_of(id))
        .join(format!("{id}.md"))
}

fn severity_display(dbg: &str) -> &str {
    match dbg {
        "Kritik" => "Kritik",
        "Yuksek" => "Yüksek",
        "Orta" => "Orta",
        "Dusuk" => "Düşük",
        "Bilgi" => "Bilgi",
        o => o,
    }
}

fn main() {
    let mut r9_fix = 0usize;
    let mut meta_fix = 0usize;

    for rule in RULES {
        let path = card_path(rule.id);
        let Ok(text) = fs::read_to_string(&path) else {
            continue;
        };
        let has_score_weight = text.lines().any(|line| {
            line.trim_start().starts_with("| Skor ağırlığı |")
        });
        let mut out: Vec<String> = Vec::with_capacity(text.lines().count());

        for line in text.lines() {
            let mut new_line = line.to_string();

            // 1) R9 "Rule message:"
            if let Some(idx) = line.find("**Kural mesajı:**") {
                let prefix = &line[..idx + "**Kural mesajı:**".len()];
                let cur = line[idx + "**Kural mesajı:**".len()..].trim();
                if cur != rule.title {
                    new_line = format!("{prefix} {}", rule.title);
                    r9_fix += 1;
                }
            }
            // 2) Metadata `Önem` / `Sınıf`.
            else if line.trim_start().starts_with("| Önem |") {
                let dbg = format!("{:?}", rule.severity);
                let want = severity_display(&dbg);
                let rebuilt = format!("| Önem | {want} |");
                if line.trim() != rebuilt {
                    new_line = rebuilt;
                    meta_fix += 1;
                }
            } else if line.trim_start().starts_with("| Sınıf |") {
                let want = format!("{:?}", rule.rule_class);
                let rebuilt = format!("| Sınıf | {want} |");
                if line.trim() != rebuilt {
                    new_line = rebuilt;
                    meta_fix += 1;
                }
            } else if line.trim_start().starts_with("| Skor tabanı |") {
                let rebuilt = format!("| Skor tabanı | {} |", rule.base_effort);
                if line.trim() != rebuilt {
                    new_line = rebuilt;
                    meta_fix += 1;
                }
                out.push(new_line);
                if rule.score_weight == 0.0 && !has_score_weight {
                    out.push("| Skor ağırlığı | 0 (no-score) |".to_string());
                    meta_fix += 1;
                }
                continue;
            } else if line.trim_start().starts_with("| Varlık |") {
                let rebuilt = format!("| Varlık | {:?} |", rule.dedup_level);
                if line.trim() != rebuilt {
                    new_line = rebuilt;
                    meta_fix += 1;
                }
            } else if line.trim_start().starts_with("| Kimlik alanı |") {
                let rebuilt = format!("| Kimlik alanı | {} |", fmt_scope(rule));
                if line.trim() != rebuilt {
                    new_line = rebuilt;
                    meta_fix += 1;
                }
            } else if line.trim_start().starts_with("| Bloke ettiği kurallar |") {
                let rebuilt = format!("| Bloke ettiği kurallar | {} |", fmt_blocks(rule));
                if line.trim() != rebuilt {
                    new_line = rebuilt;
                    meta_fix += 1;
                }
            } else if line.trim_start().starts_with("| Görünürlük |") {
                let rebuilt = format!("| Görünürlük | {} |", fmt_views(rule));
                if line.trim() != rebuilt {
                    new_line = rebuilt;
                    meta_fix += 1;
                }
            } else if line.contains("severity.weight() =") {
                let mark = "severity.weight() =";
                let idx = line.find(mark).unwrap() + mark.len();
                let pre = &line[..idx];
                let after = line[idx..].trim_start();
                let num: String = after
                    .chars()
                    .take_while(|c| c.is_ascii_digit() || *c == '.')
                    .collect();
                let rest = &after[num.len()..];
                let want = format!("{:.1}", rule.severity.weight());
                if num != want {
                    new_line = format!("{pre} {want}{rest}");
                    meta_fix += 1;
                }
            }
            out.push(new_line);
        }

        let mut joined = out.join("\n");
        if text.ends_with('\n') {
            joined.push('\n');
        }
        if joined != text {
            fs::write(&path, joined).unwrap();
        }
    }

    println!("R9 fixes: {r9_fix}");
    println!("Metadata (Önem/Sınıf) fixes: {meta_fix}");
}
