//! RULES.md / RULES.en.md / RULES.ja.md dosyalarını registry'den yeniden üretir.
//!
//! Tek doğruluk kaynağı `registry.rs` (`RULES`). Kural başlıkları:
//! - TR: registry `title` alanı.
//! - EN/JA: `ui/src/locales/{en,ja}.ts` `ruleTitles:` bloğundan.
//!
//! Grup açıklamaları (`## ARC — ...`) mevcut RULES dosyalarından korunur (registry'de yok).
//!
//! Çalıştır: cargo run -p gtfs-rules --example gen_rules
//! Bu, manuel RULES drift'ini (kural sayısı + eksik kurallar) kalıcı olarak çözer.

use gtfs_rules::{RuleMeta, RULES};
use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

fn repo_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..").canonicalize().unwrap()
}
fn group_of(id: &str) -> &str {
    id.split('_').next().unwrap_or(id)
}

// severity enum (Debug adı) → [TR, EN, JA]
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
// class enum (Debug adı) → [TR, EN, JA, FR]
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

/// locale .ts dosyasındaki `ruleTitles: { 'ID': 'title', ... }` bloğunu okur.
fn parse_titles(ts: &str) -> BTreeMap<String, String> {
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
        // Satır biçimi: `'ID': 'title',` — ama hizalama için çift boşluk (`':  '`)
        // ve bazı değerler çift tırnak (`"..."`) kullanabilir. Boşluk/tırnak-duyarsız ayrıştır.
        if let Some((idp, valp)) = t.split_once(':') {
            let id = idp.trim().trim_matches(|c| c == '\'' || c == '"').trim();
            let v = valp.trim().trim_end_matches(',').trim();
            let val = if v.starts_with('"') {
                v.trim_matches('"')
            } else {
                v.trim_matches('\'')
            };
            if !id.is_empty() && id.chars().all(|c| c.is_ascii_alphanumeric() || c == '_') {
                m.insert(id.to_string(), val.to_string());
            }
        }
    }
    m
}

struct Lang {
    file: &'static str,
    idx: usize,
    title_hdr: &'static str,   // `# ...`
    nav: &'static str,
    intro: &'static str,       // {n}/{g} placeholder'lı 3 satır
    table_hdr: &'static str,   // `| Rule | Title | Severity | Class |`
    groups: BTreeMap<String, String>,
    titles: Option<BTreeMap<String, String>>, // None = registry.title (TR)
}

fn main() {
    let root = repo_root();
    let rd = |p: &str| fs::read_to_string(root.join(p)).unwrap_or_default();

    let g_tr = parse_groups(&rd("RULES.md"));
    let g_en = parse_groups(&rd("RULES.en.md"));
    let g_ja = parse_groups(&rd("RULES.ja.md"));
    let g_fr = parse_groups(&rd("RULES.fr.md"));
    let t_en = parse_titles(&rd("ui/src/locales/en.ts"));
    let t_ja = parse_titles(&rd("ui/src/locales/ja.ts"));
    let t_fr = parse_titles(&rd("ui/src/locales/fr.ts"));

    // registry sırası: grupların ilk görülme sırası + grup içi kural sırası
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
            groups: g_tr, titles: None,
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
        out.push_str(&lang.intro.replace("{N}", &n.to_string()).replace("{G}", &g.to_string()));
        out.push_str("\n\n---\n");

        for grp in &group_order {
            let heading = lang.groups.get(*grp).cloned().unwrap_or_else(|| grp.to_string());
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
                out.push_str(&format!("| {} | {} | {} | {} |\n", r.id, safe_title, sev, cls));
            }
        }

        fs::write(root.join(lang.file), out).unwrap();
        println!("yazıldı: {} ({n} kural, {g} grup)", lang.file);
    }

    if missing_titles.is_empty() {
        println!("Tüm başlıklar locale'den çözüldü.");
    } else {
        // Eksik başlık registry'nin TÜRKÇE metnine düşer ve o metin dosyaya YAZILIR.
        // `rules_doc_drift` başlık sütununu yalnız RULES.md için doğruladığından
        // (turkish_rules_doc_titles_match_registry_titles), sızıntı hiçbir testi
        // kırmazdı. Bu yüzden generator burada durur: yarım çevrilmiş bir locale ile
        // RULES.<lang>.md üretmek sessiz bir Türkçe sızıntısıdır.
        eprintln!(
            "HATA: locale'de başlığı bulunamayan (TR'ye düşen) {} kural:",
            missing_titles.len()
        );
        for m in &missing_titles {
            eprintln!("  {m}");
        }
        eprintln!("Eksik başlıkları ui/src/locales/<lang>.ts içine ekleyip tekrar çalıştırın.");
        std::process::exit(1);
    }
}
