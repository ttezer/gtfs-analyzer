//! Kural kartı ↔ registry ↔ kod tutarlılık testi.
//!
//! docs/rules/<GRUP>/<ID>.md kartlarının `gtfs_rules::RULES` ile ve atıf yaptıkları
//! kaynak satırlarıyla senkron kalmasını garanti eder. Belge ile kod ayrışırsa CI kırmızı yanar.
//!
//! Kapsam:
//!  1. Her registry kuralının kart dosyası var (ve yetim kart yok).
//!  2. R9 "Kural mesajı" == registry `title` (TEMPLATE kuralı).
//!  3. Künye `Önem`/`Sınıf` == registry severity/rule_class.
//!  4. "Kod referansı"ndaki her `crates/...rs#Lnnn` atfı, o satırın ±3 penceresinde
//!     kuralın ID'sini içeriyor (satır kayması / yanlış atıf yakalanır).

use gtfs_rules::RULES;
use std::collections::HashSet;
use std::fs;
use std::path::{Path, PathBuf};

fn repo_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()
        .expect("repo kökü bulunamadı")
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

/// Karttaki severity gösterimini enum Debug stringine eşler.
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

/// Künye tablosundan "| Etiket | Değer |" satırının değerini döndürür.
fn kunye_value<'a>(text: &'a str, label: &str) -> Option<String> {
    for line in text.lines() {
        let l = line.trim();
        if l.starts_with('|') {
            // Markdown tablosunda kaçışlı pipe (\|) sütun ayıracı değildir — koru.
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

/// R9 "Kural mesajı:" satırından mesajı döndürür.
fn r9_message(text: &str) -> Option<String> {
    const MARK: &str = "**Kural mesajı:**";
    for line in text.lines() {
        if let Some(idx) = line.find(MARK) {
            return Some(line[idx + MARK.len()..].trim().to_string());
        }
    }
    None
}

/// Karttan tüm (kaynak_yolu, satır_no) kod atıflarını çıkarır (`crates/...rs#Lnnn`).
fn code_refs(text: &str) -> Vec<(String, usize)> {
    let mut out = Vec::new();
    for line in text.lines() {
        let mut rest = line;
        while let Some(pos) = rest.find(".rs#L") {
            let head = &rest[..pos + 3]; // ".rs" dahil
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
                problems.push(format!("{}: KART DOSYASI YOK ({})", rule.id, path.display()));
                continue;
            }
        };

        // R9 mesajı == title
        match r9_message(&text) {
            Some(msg) if msg == rule.title => {}
            Some(msg) => problems.push(format!(
                "{}: R9 mesajı ≠ title\n    kart:     '{}'\n    registry: '{}'",
                rule.id, msg, rule.title
            )),
            None => problems.push(format!("{}: R9 'Kural mesajı:' satırı bulunamadı", rule.id)),
        }

        // Önem == severity
        let exp_sev = format!("{:?}", rule.severity);
        match kunye_value(&text, "Önem") {
            Some(v) if severity_display_to_debug(&v) == exp_sev => {}
            Some(v) => problems.push(format!(
                "{}: Önem uyuşmuyor (kart '{}' → '{}', registry '{}')",
                rule.id, v, severity_display_to_debug(&v), exp_sev
            )),
            None => problems.push(format!("{}: künye 'Önem' satırı yok", rule.id)),
        }

        // Sınıf == rule_class
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

/// Metindeki tüm R-rapor token'larını (R1, R2, ...) küme olarak döndürür.
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

/// Metindeki tüm kural-ID token'larını (XFL_001, DQ_005b ...) küme olarak döndürür.
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
        let Ok(text) = fs::read_to_string(&path) else { continue };

        // Skor tabanı == base_effort
        if let Some(v) = kunye_value(&text, "Skor tabanı") {
            match v.trim().parse::<u8>() {
                Ok(n) if n == rule.base_effort => {}
                _ => problems.push(format!(
                    "{}: Skor tabanı '{}' ≠ base_effort {}", rule.id, v, rule.base_effort
                )),
            }
        }

        // Varlık == dedup_level
        let exp_dedup = format!("{:?}", rule.dedup_level);
        if let Some(v) = kunye_value(&text, "Varlık") {
            if v.trim() != exp_dedup {
                problems.push(format!("{}: Varlık '{}' ≠ dedup_level '{}'", rule.id, v, exp_dedup));
            }
        }

        // Kimlik alanı == scope_key_field
        if let Some(v) = kunye_value(&text, "Kimlik alanı") {
            let card = v.replace('`', "").replace("\\|", "|");
            let card = card.trim();
            let exp = rule.scope_key_field.unwrap_or("-");
            if card != exp {
                problems.push(format!(
                    "{}: Kimlik alanı '{}' ≠ scope_key '{}'", rule.id, card, exp
                ));
            }
        }

        // Bloke ettiği kurallar == blocks (küme)
        if let Some(v) = kunye_value(&text, "Bloke ettiği kurallar") {
            let card: std::collections::BTreeSet<String> = rule_id_tokens(&v);
            let exp: std::collections::BTreeSet<String> =
                rule.blocks.iter().map(|s| s.to_string()).collect();
            if card != exp {
                problems.push(format!(
                    "{}: Bloke ettiği {:?} ≠ blocks {:?}", rule.id, card, exp
                ));
            }
        }

        // Görünürlük R-seti == report_views
        if let Some(v) = kunye_value(&text, "Görünürlük") {
            let card = r_tokens(&v);
            let exp: std::collections::BTreeSet<String> =
                rule.report_views.iter().map(|r| format!("{:?}", r)).collect();
            if card != exp {
                problems.push(format!(
                    "{}: Görünürlük R-seti {:?} ≠ report_views {:?}", rule.id, card, exp
                ));
            }
        }

        // severity.weight() == weight(severity)
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
                        "{}: severity.weight() yazımı {} ≠ gerçek {}", rule.id, w, rule.severity.weight()
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

/// Eşik sabiti ↔ kart kontrolü. Küratörlü tablo: (rule_id, eşik değeri, kaynak dosya).
/// Değer hem kartta hem kaynakta bulunmalı; kod sabiti değişirse kart bayatlar → yakalanır.
/// Yalnızca AYIRT EDİCİ değerler eklenir (yaygın küçük sayılar gürültü yapar).
/// Yeni eşik eklendikçe bu tablo elle genişletilir.
const THRESHOLDS: &[(&str, &str, &str)] = &[
    ("SHP_026", "5000", "crates/pipeline/src/k6_analytics.rs"),
    ("TRF_010", "3600", "crates/pipeline/src/k4_cross_ref.rs"),
    ("TRF_011", "2000", "crates/pipeline/src/k6_analytics.rs"),
    ("SHP_012", "500",  "crates/pipeline/src/k6_analytics.rs"),
    ("VAT_001", "0.85", "crates/pipeline/src/k6_analytics.rs"),
    ("FIN_017", "730",  "crates/pipeline/src/k6_analytics.rs"),
];

#[test]
fn threshold_constants_match_code() {
    let root = repo_root();
    let mut problems: Vec<String> = Vec::new();
    for (id, val, src_rel) in THRESHOLDS {
        let card = fs::read_to_string(card_path(id)).unwrap_or_default();
        let src = fs::read_to_string(root.join(src_rel)).unwrap_or_default();
        if !card.contains(val) {
            problems.push(format!("{id}: kart '{val}' eşiğini içermiyor", id = id, val = val));
        }
        if !src.contains(val) {
            problems.push(format!(
                "{id}: kaynak {src_rel} '{val}' içermiyor — kod sabiti değişmiş olabilir, kartı güncelle",
                id = id, src_rel = src_rel, val = val
            ));
        }
    }
    assert!(problems.is_empty(), "\nEşik uyuşmazlığı:\n{}\n", problems.join("\n"));
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
            Err(_) => continue, // dosya yokluğu diğer testte raporlanıyor
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
                    rule.id, src_rel, line_no, lines.len()
                ));
                continue;
            }
            // Generic/veri-güdümlü emit: kural ID'si dosyada literal olarak hiç geçmiyorsa
            // (ör. AGN_001 "agency.txt eksik" k1'de REQUIRED_FILES döngüsüyle üretilir)
            // içerik kontrolü yapılamaz — satır aralığı geçerliyken atla.
            if !src.contains(&format!("\"{}\"", rule.id)) {
                continue;
            }
            // ±3 pencere içinde rule.id geçiyor mu?
            let lo = line_no.saturating_sub(4); // 0-index, line_no-3 .. line_no+3 (1-index)
            let hi = (line_no + 3).min(lines.len());
            let found = lines[lo..hi].iter().any(|l| l.contains(rule.id));
            if !found {
                problems.push(format!(
                    "{}: '{}#L{}' ±3 penceresinde '{}' yok (satır içeriği: {:?})",
                    rule.id, src_rel, line_no, rule.id,
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

/// TEMPLATE.md iskelet uyumu (yapısal): her kartın Karar/Düzeltme satırı, 9 künye
/// alanı ve çekirdek bölüm başlıklarını taşıdığını denetler. Proza değil YAPI kontrol
/// edilir. Opsiyonel-doğası gereği bloklar (Bağımlılık, Dış araç eşleşmesi, GTFS spec
/// referansı) zorunlu DEĞİL — her kurala anlamlı gelmez, filler üretmemek için.
#[test]
fn card_template_shape() {
    // (etiket, kartta aranan alt-dize)
    let kunye = [
        "Grup", "Önem", "Sınıf", "Aşama", "Varlık",
        "Kimlik alanı", "Skor tabanı", "Görünürlük", "Bloke ettiği kurallar",
    ];
    // Başlık-seviyesinden bağımsız (## veya ###) aranan çekirdek bölümler.
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

        // Başlık metinlerini topla (#/##/### fark etmez) — seviye-agnostik eşleşme.
        let headings: Vec<String> = text.lines()
            .filter(|l| l.trim_start().starts_with('#'))
            .map(|l| l.trim_start_matches('#').trim().to_string())
            .collect();

        // Karar satırı — bold (`**Karar:**`) veya düz (`> Karar:`) kabul edilir;
        // korpus çoğunlukla düz kullanıyor, format churn'ü için zorlamıyoruz.
        if !text.contains("Karar:") { missing.push("Karar".into()); }
        // Künye alanları
        for k in kunye {
            if kunye_value(&text, k).is_none() { missing.push(format!("künye:{k}")); }
        }
        // Çekirdek bölümler (başlık-seviyesi bağımsız)
        for s in sections {
            if !headings.iter().any(|h| h.contains(s)) { missing.push(s.to_string()); }
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
