//! Kural kartlarında kozmetik normalizasyon (tek seferlik bakım).
//!
//! - H1 başlık ayıracı: `# ID - başlık` → `# ID — başlık` (em-dash)
//! - Diakritiksiz all-caps severity: KRITIK→KRİTİK, BILGI→BİLGİ
//!
//! Çalıştır: cargo run -p gtfs-rules --example fix_cosmetics

use std::fs;
use std::path::{Path, PathBuf};

fn cards_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../docs/rules").canonicalize().unwrap()
}

fn fix(text: &str) -> String {
    let trailing_nl = text.ends_with('\n');
    let mut lines: Vec<String> = text.lines().map(|l| l.to_string()).collect();
    for l in lines.iter_mut() {
        // H1 başlığında ve künye 'Grup' hücresinde ilk " - " → " — "
        if (l.starts_with("# ") || l.starts_with("| Grup |")) && l.contains(" - ") {
            *l = l.replacen(" - ", " — ", 1);
        }
    }
    let mut out = lines.join("\n");
    if trailing_nl {
        out.push('\n');
    }
    // Diakritiksiz all-caps severity kelimeleri
    out = out.replace("KRITIK", "KRİTİK").replace("BILGI", "BİLGİ");
    out
}

fn main() {
    let mut changed = 0usize;
    let mut h1 = 0usize;
    let mut krit = 0usize;
    let mut bilgi = 0usize;

    for grp in fs::read_dir(cards_dir()).unwrap() {
        let grp = grp.unwrap().path();
        if !grp.is_dir() {
            continue;
        }
        for f in fs::read_dir(&grp).unwrap() {
            let f = f.unwrap().path();
            if f.extension().and_then(|e| e.to_str()) != Some("md") {
                continue;
            }
            let text = fs::read_to_string(&f).unwrap();
            // sayım (düzeltmeden önce)
            if text.lines().next().is_some_and(|l| l.starts_with("# ") && l.contains(" - ")) {
                h1 += 1;
            }
            krit += text.matches("KRITIK").count();
            bilgi += text.matches("BILGI").count();

            let fixed = fix(&text);
            if fixed != text {
                fs::write(&f, fixed).unwrap();
                changed += 1;
            }
        }
    }

    println!("Değişen kart: {changed}");
    println!("  H1 ayıraç (-→—): {h1} kart");
    println!("  KRITIK→KRİTİK  : {krit} yer");
    println!("  BILGI→BİLGİ    : {bilgi} yer");
}
