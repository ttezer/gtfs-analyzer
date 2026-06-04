//! Sapma Defteri (divergence ledger) üreteci — Katman A seed.
//!
//! Her kuralın kartındaki "Dış araç eşleşmesi" bölümünden MD karşılığını / proje-özel
//! durumunu otomatik çıkarır ve docs/DIVERGENCE_LEDGER.md üretir. Karar sütunları
//! (Bilinçli sapma? / Gerekçe) boş bırakılır — fark görüldükçe elle doldurulur (Katman B).
//!
//! Çalıştır: cargo run -p gtfs-rules --example gen_divergence_ledger

use gtfs_rules::RULES;
use std::fs;
use std::path::{Path, PathBuf};

fn repo_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..").canonicalize().unwrap()
}
fn group_of(id: &str) -> &str { id.split('_').next().unwrap_or(id) }
fn card_path(id: &str) -> PathBuf {
    repo_root().join("docs/rules").join(group_of(id)).join(format!("{id}.md"))
}

/// Karttan "### Dış araç eşleşmesi" bölümünün metnini döndürür.
fn external_block(text: &str) -> String {
    let mut lines = text.lines();
    let mut block = String::new();
    let mut in_block = false;
    for line in &mut lines {
        if line.trim() == "### Dış araç eşleşmesi" {
            in_block = true;
            continue;
        }
        if in_block {
            if line.starts_with("### ") {
                break;
            }
            block.push_str(line);
            block.push('\n');
        }
    }
    block
}

/// Bloktaki ilk backtick'li (alt çizgili = notice gibi) token.
fn first_notice(block: &str) -> Option<String> {
    let bytes: Vec<char> = block.chars().collect();
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == '`' {
            let mut j = i + 1;
            let mut tok = String::new();
            while j < bytes.len() && bytes[j] != '`' {
                tok.push(bytes[j]);
                j += 1;
            }
            if tok.contains('_') && !tok.contains(' ') && !tok.contains("::") {
                return Some(tok);
            }
            i = j + 1;
        } else {
            i += 1;
        }
    }
    None
}

/// "**Marker:**" ile başlayıp verilen sonlandırıcılara kadar olan metni döndürür.
fn field<'a>(s: &'a str, marker: &str, ends: &[&str]) -> Option<String> {
    let i = s.find(marker)? + marker.len();
    let rest = &s[i..];
    let mut end = rest.len();
    for e in ends {
        if let Some(p) = rest.find(e) {
            end = end.min(p);
        }
    }
    Some(rest[..end].trim().trim_matches('`').trim().to_string())
}

/// Kartın "Dış araç eşleşmesi" bölümündeki makine-okunur özet satırını ayrıştırır.
/// `- **Eşleşme:** \`x\` · **Tür:** MD-parite · **Bilinçli:** evet — gerekçe`
/// Döner: (md_karşılığı, tür, bilinçli, gerekçe). Özet satırı yoksa None.
fn parse_structured(block: &str) -> Option<(String, String, String, String)> {
    let line = block.lines().find(|l| l.contains("**Eşleşme:**"))?;
    let md = field(line, "**Eşleşme:**", &["·", "**Tür"]).unwrap_or_else(|| "—".into());
    let tur = field(line, "**Tür:**", &["·", "**Bilinçli"]).unwrap_or_else(|| "İNCELE".into());
    let bil_full = field(line, "**Bilinçli:**", &[]).unwrap_or_default();
    // "evet — gerekçe" → bilinçli="evet", gerekçe="gerekçe"
    let (bil, ger) = match bil_full.split_once('—') {
        Some((b, g)) => (b.trim().to_string(), g.trim().to_string()),
        None => (bil_full.trim().to_string(), String::new()),
    };
    let md_out = if md == "—" || md.is_empty() { "—".to_string() } else { format!("`{md}`") };
    Some((md_out, tur, bil, ger))
}

fn classify(block: &str) -> (String, &'static str) {
    let proj = block.contains("proje-özel");
    let notice = first_notice(block);
    let md = match &notice {
        Some(n) => format!("`{n}`"),
        None => "—".to_string(),
    };
    let tur = if proj {
        if notice.is_some() { "proje-özel (yakın MD var)" } else { "proje-özel" }
    } else if notice.is_some() {
        "MD-eşleşme"
    } else {
        "İNCELE"
    };
    (md, tur)
}

fn sev_display(dbg: &str) -> &str {
    match dbg {
        "Kritik" => "Kritik", "Yuksek" => "Yüksek", "Orta" => "Orta",
        "Dusuk" => "Düşük", "Bilgi" => "Bilgi", o => o,
    }
}

fn main() {
    let mut md = String::new();
    md.push_str("# Sapma Defteri (Divergence Ledger)\n\n");
    md.push_str("> **Amaç:** Bir feed'i MobilityData/GTFS Guru ile karşılaştırınca çıkan farkın **bizde bilinçli mi, bug mı** olduğunu kesin cevaplamak. `docs/RULE_TRIAGE.md` Adım 3'ün referansıdır.\n>\n");
    md.push_str("> **Katman A (otomatik):** `MD karşılığı` + `Tür` sütunları kartların \"Dış araç eşleşmesi\" bölümünden üretilir — `cargo run -p gtfs-rules --example gen_divergence_ledger` ile yenilenir.\n>\n");
    md.push_str("> **Katman B (elle):** `Bilinçli sapma?` + `Gerekçe / parite` sütunları boş doğar; bir fark araştırıldıkça doldurulur. Bir kez doldurulan satır, aynı fark tekrar çıktığında anında karar verir.\n>\n");
    md.push_str("> **Tür değerleri:** `MD-eşleşme` (MD'de birebir/yakın karşılık var) · `proje-özel` (MD/Guru'da karşılık yok) · `İNCELE` (otomatik sınıflanamadı).\n\n");

    let mut counts = (0usize, 0usize, 0usize); // md, proje, incele
    let mut cur_group = "";

    for rule in RULES {
        let g = group_of(rule.id);
        if g != cur_group {
            cur_group = g;
            md.push_str(&format!(
                "\n## {g}\n\n| ID | Önem·Sınıf | MD karşılığı | Tür | Bilinçli sapma? | Gerekçe / parite |\n|---|---|---|---|---|---|\n"
            ));
        }
        let text = fs::read_to_string(card_path(rule.id)).unwrap_or_default();
        let block = external_block(&text);
        // Önce makine-okunur özet satırı (kart = tek kaynak); yoksa prose'a düş.
        let (md_match, tur, bilincli, gerekce) = match parse_structured(&block) {
            Some((m, t, b, g)) => (m, t, b, g),
            None => {
                let (m, t) = classify(&block);
                (m, t.to_string(), String::new(), String::new())
            }
        };
        if tur.starts_with("MD") {
            counts.0 += 1;
        } else if tur == "İNCELE" {
            counts.2 += 1;
        } else {
            counts.1 += 1;
        }
        let sev_dbg = format!("{:?}", rule.severity);
        let sev = sev_display(&sev_dbg);
        let cls = format!("{:?}", rule.rule_class);
        // Serbest-metin hücrelerde `|` tablo ayıracını bozmasın diye kaçışla.
        md.push_str(&format!(
            "| {} | {}·{} | {} | {} | {} | {} |\n",
            rule.id, sev, cls, md_match, tur,
            bilincli.replace('|', "\\|"),
            gerekce.replace('|', "\\|")
        ));
    }

    let out = repo_root().join("docs/DIVERGENCE_LEDGER.md");
    fs::write(&out, md).unwrap();

    println!("Yazıldı: {}", out.display());
    println!("Toplam kural: {}", RULES.len());
    println!("  MD-eşleşme : {}", counts.0);
    println!("  proje-özel : {}", counts.1);
    println!("  İNCELE     : {}", counts.2);
}
