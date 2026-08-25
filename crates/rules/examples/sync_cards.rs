//! Kural kartlarını registry + kaynakla senkronlar (tek seferlik bakım aracı).
//!
//! - R9 "Kural mesajı:" satırını registry `title`'a eşitler.
//! - Künye `Önem`/`Sınıf` değerlerini registry severity/rule_class'a eşitler.
//! - "Kod referansı"ndaki `crates/...rs#Lnnn` atıflarını, atıf yapılan dosyada
//!   kuralın gerçek emit/test satırına yeniden çözer (satır kayması düzeltilir).
//!
//! Çalıştır: cargo run -p gtfs-rules --example sync_cards
//! card_consistency testi bundan sonra yeşile yaklaşmalı.

use gtfs_core::ReportId;
use gtfs_rules::{RuleMeta, RULES};
use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};

fn views_token(views: &[ReportId]) -> &'static str {
    let s: BTreeSet<String> = views.iter().map(|r| format!("{r:?}")).collect();
    let m = |a: &[&str]| a.iter().map(|x| x.to_string()).collect::<BTreeSet<String>>();
    if s == m(&["R1", "R2", "R5", "R9"]) { "VS_K" }
    else if s == m(&["R2", "R5", "R9"]) { "VS" }
    else if s == m(&["R2", "R5", "R8", "R9"]) { "VI" }
    else if s == m(&["R2", "R3", "R5", "R9"]) { "VA" }
    else if s == m(&["R2", "R4", "R5", "R8", "R9"]) { "VI_GEO" }
    else if s == m(&["R2", "R4", "R5", "R9"]) { "VS_GEO" }
    else if s == m(&["R2", "R3", "R4", "R5", "R9"]) { "VA_GEO" }
    else if s == m(&["R2", "R5", "R7", "R8", "R9"]) { "VI_ACC" }
    else if s == m(&["R2", "R3", "R5", "R7", "R9"]) { "VA_ACC" }
    else if s == m(&["R2", "R5", "R7", "R9"]) { "VS_ACC" }
    else { "?" }
}

fn fmt_views(rule: &RuleMeta) -> String {
    let rs: Vec<String> = rule.report_views.iter().map(|r| format!("`{r:?}`")).collect();
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
        rule.blocks.iter().map(|b| format!("`{b}`")).collect::<Vec<_>>().join(", ")
    }
}

fn repo_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..").canonicalize().unwrap()
}
fn group_of(id: &str) -> &str { id.split('_').next().unwrap_or(id) }
fn card_path(id: &str) -> PathBuf {
    repo_root().join("docs/rules").join(group_of(id)).join(format!("{id}.md"))
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

/// Kaynak dosyada kuralın satırını çöz: want_test=false → emit (test bloğu öncesi),
/// true → test bloğu içi. Aranan: tırnaklı "ID".
fn resolve_line(src: &str, id: &str, want_test: bool) -> Option<usize> {
    let lines: Vec<&str> = src.lines().collect();
    let test_start = lines.iter().position(|l| l.contains("mod tests")).unwrap_or(lines.len());
    let needle = format!("\"{id}\"");
    let range = if want_test { test_start..lines.len() } else { 0..test_start };
    for i in range {
        if lines[i].contains(&needle) {
            return Some(i + 1);
        }
    }
    // fallback: dosyanın herhangi yerinde
    lines.iter().position(|l| l.contains(&needle)).map(|i| i + 1)
}

fn main() {
    let root = repo_root();
    let mut r9_fix = 0usize;
    let mut meta_fix = 0usize;
    let mut ref_fix = 0usize;
    let mut unresolved: Vec<String> = Vec::new();

    for rule in RULES {
        let path = card_path(rule.id);
        let Ok(text) = fs::read_to_string(&path) else { continue };
        let mut out: Vec<String> = Vec::with_capacity(text.lines().count());

        for line in text.lines() {
            let mut new_line = line.to_string();

            // 1) R9 "Kural mesajı:"
            if let Some(idx) = line.find("**Kural mesajı:**") {
                let prefix = &line[..idx + "**Kural mesajı:**".len()];
                let cur = line[idx + "**Kural mesajı:**".len()..].trim();
                if cur != rule.title {
                    new_line = format!("{prefix} {}", rule.title);
                    r9_fix += 1;
                }
            }
            // 2) künye Önem / Sınıf
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
                if line.trim() != rebuilt { new_line = rebuilt; meta_fix += 1; }
            } else if line.trim_start().starts_with("| Varlık |") {
                let rebuilt = format!("| Varlık | {:?} |", rule.dedup_level);
                if line.trim() != rebuilt { new_line = rebuilt; meta_fix += 1; }
            } else if line.trim_start().starts_with("| Kimlik alanı |") {
                let rebuilt = format!("| Kimlik alanı | {} |", fmt_scope(rule));
                if line.trim() != rebuilt { new_line = rebuilt; meta_fix += 1; }
            } else if line.trim_start().starts_with("| Bloke ettiği kurallar |") {
                let rebuilt = format!("| Bloke ettiği kurallar | {} |", fmt_blocks(rule));
                if line.trim() != rebuilt { new_line = rebuilt; meta_fix += 1; }
            } else if line.trim_start().starts_with("| Görünürlük |") {
                let rebuilt = format!("| Görünürlük | {} |", fmt_views(rule));
                if line.trim() != rebuilt { new_line = rebuilt; meta_fix += 1; }
            } else if line.contains("severity.weight() =") {
                let mark = "severity.weight() =";
                let idx = line.find(mark).unwrap() + mark.len();
                let pre = &line[..idx];
                let after = line[idx..].trim_start();
                let num: String = after.chars().take_while(|c| c.is_ascii_digit() || *c == '.').collect();
                let rest = &after[num.len()..];
                let want = format!("{:.1}", rule.severity.weight());
                if num != want {
                    new_line = format!("{pre} {want}{rest}");
                    meta_fix += 1;
                }
            }
            // 3) Kod referansı satırları (.rs#L)
            else if line.contains(".rs#L") {
                let want_test = line.contains("Test:") || line.to_lowercase().contains("test");
                // Satırdaki her atfı tara; düzeltmeleri (start, end, yeni) byte-aralığı
                // olarak topla ve sağdan sola uygula. Konum-temelli değişim, replace-all'ın
                // aksine aynı satırdaki başka bir numarayı (ör. #L42 ile #L420) bozmaz ve
                // offset kayması/UTF-8 panic riski taşımaz.
                let mut edits: Vec<(usize, usize, String)> = Vec::new();
                let mut search_from = 0usize;
                while let Some(rel) = line[search_from..].find(".rs#L") {
                    let pos = search_from + rel;
                    let num_start = pos + 5; // ".rs#L" sonrası ilk rakam
                    let num: String = line[num_start..]
                        .chars()
                        .take_while(|c| c.is_ascii_digit())
                        .collect();
                    let num_end = num_start + num.len();
                    search_from = num_end.max(pos + 5);

                    let head = &line[..pos + 3]; // ".rs" dahil
                    let Some(cstart) = head.rfind("crates/") else { continue };
                    let src_rel = head[cstart..].to_string();
                    let Ok(old_n) = num.parse::<usize>() else { continue };
                    let Ok(src) = fs::read_to_string(root.join(&src_rel)) else { continue };
                    let Some(new_n) = resolve_line(&src, rule.id, want_test) else {
                        unresolved.push(format!("{}: '{}' içinde çözülemedi", rule.id, src_rel));
                        continue;
                    };
                    if new_n == old_n {
                        continue;
                    }
                    // URL tarafı: ...rs#L<old_n>
                    edits.push((num_start, num_end, new_n.to_string()));
                    // Link metni tarafı: anchor numarası daha önce değişmiş,
                    // etiket numarası eski kalmış olabilir. Bu yüzden etiketi
                    // `old_n` ile aramak yerine, bağlantının hemen önündeki
                    // `[...rs:<digits>]` aralığını yapısal olarak bul.
                    if let Some(label_end) = line[..pos].rfind(']') {
                        if let Some(label_start) = line[..label_end].rfind('[') {
                            let label = &line[label_start + 1..label_end];
                            if let Some(colon) = label.rfind(':') {
                                let label_path = &label[..colon];
                                let label_digits = &label[colon + 1..];
                                if label_path.ends_with(".rs")
                                    && !label_digits.is_empty()
                                    && label_digits.chars().all(|c| c.is_ascii_digit())
                                {
                                    let ds = label_start + 1 + colon + 1;
                                    let de = ds + label_digits.len();
                                    edits.push((ds, de, new_n.to_string()));
                                }
                            }
                        }
                    }
                    ref_fix += 1;
                }
                if !edits.is_empty() {
                    edits.sort_by_key(|edit| std::cmp::Reverse(edit.0)); // sağdan sola: offset'ler geçerli kalır
                    let mut s = line.to_string();
                    for (st, en, rep) in edits {
                        s.replace_range(st..en, &rep);
                    }
                    new_line = s;
                }
            }

            out.push(new_line);
        }

        let mut joined = out.join("\n");
        if text.ends_with('\n') { joined.push('\n'); }
        if joined != text {
            fs::write(&path, joined).unwrap();
        }
    }

    println!("R9 düzeltme: {r9_fix}");
    println!("Künye (Önem/Sınıf) düzeltme: {meta_fix}");
    println!("Kod-ref satır düzeltme: {ref_fix}");
    if !unresolved.is_empty() {
        println!("\nÇÖZÜLEMEYEN atıflar ({}):", unresolved.len());
        for u in &unresolved { println!("  {u}"); }
    }
}
