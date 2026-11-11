//! Regenerates RULES.md / RULES.en.md / RULES.ja.md from the registry.
//!
//! The single source of truth is `registry.rs` (`RULES`). Rule titles come from the
//! `ruleTitles:` block in `ui/src/locales/{tr,en,ja,fr}.ts` (the registry `title` is
//! English and is used only when a locale lacks a rule).
//!
//! Group descriptions (`## ARC — ...`) are preserved from the existing RULES files (they are not in the registry).
//!
//! Run: cargo run -p gtfs-rules --example gen_rules
//! This permanently resolves manually maintained RULES drift (rule counts + missing rules).

use gtfs_rules::{RuleMeta, RULES};

#[path = "support/locale_titles.rs"]
mod locale_titles;
use locale_titles::parse_titles;
use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

fn repo_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()
        .unwrap()
}
fn group_of(id: &str) -> &str {
    id.split('_').next().unwrap_or(id)
}

// Severity enum (Debug name) → [TR, EN, JA]
fn sev4(s: &str) -> [&'static str; 4] {
    match s {
        "Kritik" => ["KRİTİK", "CRITICAL", "致命的", "CRITIQUE"],
        "Yuksek" => ["YÜKSEK", "HIGH", "高", "ÉLEVÉE"],
        "Orta" => ["ORTA", "MEDIUM", "中", "MOYENNE"],
        "Dusuk" => ["DÜŞÜK", "LOW", "低", "FAIBLE"],
        "Bilgi" => ["BİLGİ", "INFO", "情報", "INFO"],
        _ => ["?", "?", "?", "?"],
    }
}
// Class enum (Debug name) → [TR, EN, JA, FR]
fn cls4(c: &str) -> [&'static str; 4] {
    match c {
        "Spec" => ["Spec", "Spec", "仕様", "Spec"],
        "Interop" => ["Interop", "Interop", "相互運用", "Interop"],
        "Quality" => ["Quality", "Quality", "品質", "Quality"],
        "Analytics" => ["Analytics", "Analytics", "分析", "Analytics"],
        _ => ["?", "?", "?", "?"],
    }
}

/// `## ARC — Archive / File Level` → "ARC" => "ARC — Archive / File Level"
fn parse_groups(md: &str) -> BTreeMap<String, String> {
    let mut m = BTreeMap::new();
    for line in md.lines() {
        if let Some(rest) = line.strip_prefix("## ") {
            let grp = rest.split_whitespace().next().unwrap_or("").to_string();
            if !grp.is_empty() {
                m.insert(grp, rest.trim().to_string());
            }
        }
    }
    m
}

struct Lang {
    file: &'static str,
    idx: usize,
    title_hdr: &'static str, // `# ...`
    nav: &'static str,
    intro: &'static str,     // Three lines containing {n}/{g} placeholders.
    table_hdr: &'static str, // `| Rule | Title | Severity | Class |`
    groups: BTreeMap<String, String>,
    titles: Option<BTreeMap<String, String>>, // None = registry.title (English)
}

fn main() {
    let root = repo_root();
    let rd = |p: &str| fs::read_to_string(root.join(p)).unwrap_or_default();

    let g_tr = parse_groups(&rd("RULES.md"));
    let g_en = parse_groups(&rd("RULES.en.md"));
    let g_ja = parse_groups(&rd("RULES.ja.md"));
    let g_fr = parse_groups(&rd("RULES.fr.md"));
    let t_tr = locale_titles::turkish_titles(&repo_root());
    let t_en = parse_titles(&rd("ui/src/locales/en.ts"));
    let t_ja = parse_titles(&rd("ui/src/locales/ja.ts"));
    let t_fr = parse_titles(&rd("ui/src/locales/fr.ts"));

    // Registry order: first-seen group order, then rule order within each group.
    let mut group_order: Vec<&str> = vec![];
    let mut by_group: BTreeMap<&str, Vec<&RuleMeta>> = BTreeMap::new();
    for r in RULES.iter() {
        let g = group_of(r.id);
        if !group_order.contains(&g) {
            group_order.push(g);
        }
        by_group.entry(g).or_default().push(r);
    }
    let n = RULES.len();
    let g = group_order.len();

    let langs = [
        Lang {
            file: "RULES.md", idx: 0,
            title_hdr: "# GTFS Validator & Analyzer — Kural Listesi",
            nav: "🇹🇷 **Türkçe** · 🇬🇧 [English](RULES.en.md) · 🇯🇵 [日本語](RULES.ja.md) · 🇫🇷 [Français](RULES.fr.md)",
            intro: "{N} kural, {G} grup. Her kural benzersiz bir ID, önem seviyesi ve sınıf ile tanımlanır.\nÖnem seviyeleri: **KRİTİK** (yayın engelleyici) · **YÜKSEK** · **ORTA** · **DÜŞÜK** · **BİLGİ**\nSınıflar: **Spec** (GTFS Geçerliliği) · **Interop** (GTFS Uyumluluğu) · **Quality** (GTFS Kalitesi) · **Analytics** (GTFS Analitiği)",
            table_hdr: "| Kural | Başlık | Önem | Sınıf |",
            groups: g_tr, titles: Some(t_tr),
        },
        Lang {
            file: "RULES.en.md", idx: 1,
            title_hdr: "# GTFS Validator & Analyzer — Rule List",
            nav: "🇹🇷 [Türkçe](RULES.md) · 🇬🇧 **English** · 🇯🇵 [日本語](RULES.ja.md) · 🇫🇷 [Français](RULES.fr.md)",
            intro: "{N} rules, {G} groups. Each rule is identified by a unique ID, severity level, and class.\nSeverity levels: **CRITICAL** (publish blocker) · **HIGH** · **MEDIUM** · **LOW** · **INFO**\nClasses: **Spec** (GTFS Validity) · **Interop** (GTFS Interoperability) · **Quality** (GTFS Quality) · **Analytics** (GTFS Analytics)",
            table_hdr: "| Rule | Title | Severity | Class |",
            groups: g_en, titles: Some(t_en),
        },
        Lang {
            file: "RULES.ja.md", idx: 2,
            title_hdr: "# GTFS Validator & Analyzer — ルール一覧",
            nav: "🇹🇷 [Türkçe](RULES.md) · 🇬🇧 [English](RULES.en.md) · 🇯🇵 **日本語** · 🇫🇷 [Français](RULES.fr.md)",
            intro: "{N}ルール、{G}グループ。各ルールは一意のID、重要度、クラスで定義されます。\n重要度: **致命的**（公開ブロッカー）· **高** · **中** · **低** · **情報**\nクラス: **仕様**（GTFS妥当性）· **相互運用**（GTFSインターオペラビリティ）· **品質**（GTFS品質）· **分析**（GTFSアナリティクス）",
            table_hdr: "| ルール | タイトル | 重要度 | クラス |",
            groups: g_ja, titles: Some(t_ja),
        },
        Lang {
            file: "RULES.fr.md", idx: 3,
            title_hdr: "# GTFS Validator & Analyzer — Liste des règles",
            nav: "🇹🇷 [Türkçe](RULES.md) · 🇬🇧 [English](RULES.en.md) · 🇯🇵 [日本語](RULES.ja.md) · 🇫🇷 **Français**",
            intro: "{N} règles, {G} groupes. Chaque règle est identifiée par un ID unique, un niveau de gravité et une classe.\nNiveaux de gravité : **CRITIQUE** (bloquant pour la publication) · **ÉLEVÉE** · **MOYENNE** · **FAIBLE** · **INFO**\nClasses : **Spec** (validité GTFS) · **Interop** (interopérabilité GTFS) · **Quality** (qualité GTFS) · **Analytics** (analytique GTFS)",
            table_hdr: "| Règle | Titre | Gravité | Classe |",
            groups: g_fr, titles: Some(t_fr),
        },
    ];

    let mut missing_titles: Vec<String> = vec![];

    for lang in &langs {
        let mut out = String::new();
        out.push_str(lang.title_hdr);
        out.push_str("\n\n");
        out.push_str(lang.nav);
        out.push_str("\n\n");
        out.push_str(
            &lang
                .intro
                .replace("{N}", &n.to_string())
                .replace("{G}", &g.to_string()),
        );
        out.push_str("\n\n---\n");

        for grp in &group_order {
            let heading = lang
                .groups
                .get(*grp)
                .cloned()
                .unwrap_or_else(|| grp.to_string());
            out.push_str(&format!("\n## {heading}\n\n"));
            out.push_str(lang.table_hdr);
            out.push_str("\n|---|---|---|---|\n");
            for r in &by_group[*grp] {
                let title = match &lang.titles {
                    None => r.title.to_string(),
                    Some(map) => map.get(r.id).cloned().unwrap_or_else(|| {
                        missing_titles.push(format!("{} ({})", r.id, lang.file));
                        r.title.to_string()
                    }),
                };
                let sev = sev4(&format!("{:?}", r.severity))[lang.idx];
                let cls = cls4(&format!("{:?}", r.rule_class))[lang.idx];
                let safe_title = title.replace('|', "\\|");
                out.push_str(&format!(
                    "| {} | {} | {} | {} |\n",
                    r.id, safe_title, sev, cls
                ));
            }
        }

        fs::write(root.join(lang.file), out).unwrap();
        println!("written: {} ({n} rules, {g} groups)", lang.file);
    }

    if missing_titles.is_empty() {
        println!("All titles were resolved from the locales.");
    } else {
        // A missing title falls back to the registry's TURKISH text, and that text is WRITTEN to the file.
        // Because `rules_doc_drift` checks the title column only for RULES.md
        // (turkish_rules_doc_titles_match_registry_titles), the leak would not
        // break any test. The generator therefore stops here: generating RULES.<lang>.md
        // with a partially translated locale would silently leak Turkish text.
        eprintln!(
            "ERROR: {} rules have no title in the locale (they fall back to TR):",
            missing_titles.len()
        );
        for m in &missing_titles {
            eprintln!("  {m}");
        }
        eprintln!("Add the missing titles to ui/src/locales/<lang>.ts and run again.");
        std::process::exit(1);
    }
}
