//! RULES.md / RULES.en.md / RULES.ja.md ↔ `gtfs_rules::RULES` drift kapısı.
//!
//! Bu tablolar `gen_rules` example'ı ile registry'den ÜRETİLİR, ama üretimi
//! zorlayan bir şey yoktu: registry değişip `gen_rules` çalıştırılmayınca
//! yayınlanan tablolar sessizce bayatlıyordu. Gerçek vaka (2026-07-16'da
//! yakalandı): tablolar `8b4fae3`'te üretilmiş, ardından `60e53b0` (TRP_017 →
//! Quality, ATR_009 başlık düzeltmesi) ve `d24848f` (XFL_020/021 Spec → Quality)
//! registry'yi değiştirmiş; RULES×3 iki commit boyunca yanlış sınıf yayınladı.
//! `card_consistency` kartları registry'ye bağlıyordu, RULES dosyalarını hiçbir
//! test bağlamıyordu.
//!
//! Bu test BİLEREK `gen_rules`'un kodunu yeniden kullanmaz: yayınlanmış tabloyu
//! ayrıştırıp registry ile karşılaştırır. Generator'ın kendi mantığındaki bir
//! hata, o mantığı çağıran bir teste görünmez olurdu — bağımsız iddia gerekir.
//!
//! Kırmızı yanarsa: `cargo run -p gtfs-rules --example gen_rules`

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use gtfs_rules::RULES;

fn repo_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()
        .expect("repo kökü bulunamadı")
}

/// Tablo satırı: `| ARC_001 | Başlık | KRİTİK | Spec |`
#[derive(Debug, PartialEq)]
struct Row {
    title: String,
    severity: String,
    class: String,
}

/// Markdown tablo satırını KAÇIŞSIZ `|` karakterlerinden böler ve `\|` kaçışlarını çözer.
///
/// `gen_rules` başlıktaki `|` karakterini `\|` olarak escape'ler (ör. GEO_016:
/// "Durak Null Island yakınında (\|lat\|<1 VE \|lon\|<1)"). Naif bir `split('|')`
/// böyle satırları 4'ten fazla hücreye böler ve satır sessizce elenir — yani
/// tam da denetlemesi gereken kuralları atlar.
fn split_cells(line: &str) -> Vec<String> {
    let mut cells: Vec<String> = Vec::new();
    let mut cur = String::new();
    let mut chars = line.chars().peekable();
    while let Some(c) = chars.next() {
        match c {
            '\\' if chars.peek() == Some(&'|') => {
                chars.next();
                cur.push('|'); // kaçış çözüldü: hücre içeriğinin parçası
            }
            '|' => {
                cells.push(cur.trim().to_string());
                cur.clear();
            }
            _ => cur.push(c),
        }
    }
    cells.push(cur.trim().to_string());
    // `| a | b |` → baştaki ve sondaki boş hücreleri at
    if cells.first().is_some_and(String::is_empty) {
        cells.remove(0);
    }
    if cells.last().is_some_and(String::is_empty) {
        cells.pop();
    }
    cells
}

/// RULES dosyasındaki tüm kural satırlarını ID → Row olarak ayrıştırır.
/// Başlık satırı (`| Kural | ...`) ve ayırıcı (`|---|`) atlanır; kural ID'si
/// `GRP_NNN` biçiminde olduğu için diğer tablolar yanlışlıkla yakalanmaz.
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
        let looks_like_rule_id = id
            .split_once('_')
            .is_some_and(|(g, n)| {
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
                title: cells[1].clone(), // kaçışlar split_cells'te çözüldü
                severity: cells[2].clone(),
                class: cells[3].clone(),
            },
        );
    }
    out
}

/// Registry enum'unun her dildeki beklenen yazımı. gen_rules'taki tablonun
/// BAĞIMSIZ yeniden ifadesi — kasıtlı kopya: biri değişirse test kırılmalı.
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

/// Her dilde: yayınlanan kural kümesi = registry kümesi, ve her satırın
/// önem/sınıf sütunu registry ile aynı.
#[test]
fn rules_docs_match_registry() {
    let mut problems: Vec<String> = vec![];

    for (lang, file) in ["RULES.md", "RULES.en.md", "RULES.ja.md", "RULES.fr.md"].iter().enumerate() {
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

        // Registry'den silinen kural tabloda kalmamalı.
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

/// Türkçe tablo başlıkları doğrudan `registry.title`'dan gelir → birebir eşleşmeli.
/// (EN/JA başlıkları locale dosyalarından geldiği için ayrı testte kontrol edilir.)
/// Türkçe sızıntı kapısı — çevrilmiş RULES dosyaları için.
///
/// `gen_rules`, locale'de bulamadığı başlığın yerine registry'nin TÜRKÇE metnini
/// yazar. Yukarıdaki başlık denetimi yalnız `RULES.md`'yi kapsadığından, yarım
/// çevrilmiş bir locale ile üretilen `RULES.en/ja/fr.md` sessizce Türkçe satır
/// taşıyabilirdi. `ş ğ İ ı` harfleri bu üç dilin hiçbirinde yoktur.
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
fn turkish_rules_doc_titles_match_registry_titles() {
    let rows = parse_rows(&read("RULES.md"));
    let mut problems: Vec<String> = vec![];

    for rule in RULES {
        if let Some(row) = rows.get(rule.id) {
            if row.title != rule.title {
                problems.push(format!(
                    "{}: başlık '{}' ≠ registry.title '{}'",
                    rule.id, row.title, rule.title
                ));
            }
        }
    }

    assert!(
        problems.is_empty(),
        "RULES.md başlıkları registry.title ile uyuşmuyor ({} sorun) — düzeltmek için:\n  \
         cargo run -p gtfs-rules --example gen_rules\n\n{}",
        problems.len(),
        problems.join("\n")
    );
}

/// Tablodaki kural sayısı ve grup sayısı, giriş metninde yazan sayılarla tutmalı
/// ("537 kural, 37 grup"). Elle düzenlenmiş bir tabloda ilk sapan şey budur.
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
