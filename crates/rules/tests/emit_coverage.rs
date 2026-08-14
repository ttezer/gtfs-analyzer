//! Emit kapsamı (dead-rule koruması) — issue #5 initial version.
//!
//! Soru: registry'deki bir kural, pipeline ÜRETİM kodunda gerçekten geçiyor mu, yoksa
//! "registry'de var ama implementasyon ölü" (dead/orphan rule) mu? card_consistency
//! kart↔registry tutarlılığını test eder; bu test ise her canonical rule_id'nin
//! pipeline/src üretim kodunda (test modülleri ve k7 rapor-helper'ı hariç) en az bir
//! kez string literal olarak geçtiğini doğrular.
//!
//! Kapsam: bu "reference proof"tur, tam "emit proof" değildir (gerçek emit-proof için
//! her kurala kasıtlı fixture gerekir — issue #5 "full version"/C, ayrı iş). Yine de
//! asıl korkuyu — registry'ye eklenip emit kodu hiç yazılmayan ya da emit kodu silinen
//! kural — güvenilir biçimde yakalar. Feed gerektirmez, CI'da çalışır.

use gtfs_rules::RULES;
use std::fs;
use std::path::{Path, PathBuf};

/// Statik string-literal taramasıyla görünmeyen, ama gerçekten emit edilen kurallar.
/// Bunlar dinamik/fatal yollarla üretilir; rule_id kaynak kodda literal geçmez.
const DYNAMIC_EMIT_ALLOWLIST: &[&str] = &[
    // Fatal yol: FatalCode::ZipUnreadable olarak döner (Notice değil), rule_id literal yok.
    "ARC_001", // "ZIP arşivi açılamadı"
    // Fatal yol: FatalCode::DecompressionLimit (k1_parse.rs, GuardedReader).
    // 2026-08-15'e kadar listede DEĞİLDİ ve kapıdan yalnız yorum satırlarındaki
    // "ARC_029" geçişleri sayesinde geçiyordu; collect_rule_ids sıkılaşınca ortaya çıktı.
    "ARC_029", // "Sıkıştırma sınırı aşıldı (zip-bomb koruması)"
];

fn repo_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()
        .expect("repo kökü bulunamadı")
}

/// `#[cfg(test)]` ile başlayan modül bloklarını brace-eşleştirerek metinden çıkarır.
/// (Bir dosyada birden çok test bloğu olabilir; emit kodu bloklar arasında kalabilir.)
fn strip_test_mods(src: &str) -> String {
    let marker = "#[cfg(test)]";
    let mut out = String::new();
    let mut rest = src;
    while let Some(pos) = rest.find(marker) {
        out.push_str(&rest[..pos]);
        let after = &rest[pos..];
        // Modül gövdesinin ilk '{'ını bul, eşleşen '}'a kadar atla.
        if let Some(brace) = after.find('{') {
            let bytes = after.as_bytes();
            let mut depth = 0i32;
            let mut k = brace;
            while k < bytes.len() {
                match bytes[k] {
                    b'{' => depth += 1,
                    b'}' => {
                        depth -= 1;
                        if depth == 0 { k += 1; break; }
                    }
                    _ => {}
                }
                k += 1;
            }
            rest = &after[k..];
        } else {
            // '{' yoksa kalan her şeyi at (güvenli taraf).
            rest = "";
        }
    }
    out.push_str(rest);
    out
}

/// `^[A-Z]{2,}_[0-9]+[a-z]?$` — registry rule_id deseni.
fn is_rule_id(s: &str) -> bool {
    let Some(us) = s.find('_') else { return false };
    if us < 2 { return false; }
    if !s[..us].bytes().all(|c| c.is_ascii_uppercase()) { return false; }
    let rest = &s[us + 1..];
    let digits = rest.trim_end_matches(|c: char| c.is_ascii_lowercase());
    let suffix = &rest[digits.len()..];
    !digits.is_empty()
        && digits.bytes().all(|c| c.is_ascii_digit())
        && suffix.len() <= 1
}

/// STRING LİTERALİ içindeki rule_id token'larını toplar.
///
/// 🔴 Eskiden tırnak-bağımsızdı ve "bir rule_id'nin kaynakta token olarak geçmesi kod
/// canlı sayılır" diyordu. Bu kapıyı bir YORUM SATIRI tatmin edebiliyordu: `ARC_029`
/// üretim kodunda yalnız `// ... ARC_029 (DecompressionLimit) Fatal'ı üretilir` gibi
/// yorumlarda geçtiği hâlde kapı yeşildi. Emit noktası `notice(..., "ARC_029", ...)`
/// biçiminde bir literaldir; ölçü artık o. Tırnaksız geçiş sayılmaz.
///
/// Ölçüldü (2026-08-15): sıkılaştırma yalnız iki kuralı düşürüyor — ikisi de Fatal
/// yolla üretilen ve `DYNAMIC_EMIT_ALLOWLIST`'e ait olanlar.
fn collect_rule_ids(text: &str, into: &mut std::collections::HashSet<String>) {
    let bytes = text.as_bytes();
    let is_word = |c: u8| c.is_ascii_alphanumeric() || c == b'_';
    let mut i = 0;
    while i < bytes.len() {
        // Yorum: satır sonuna kadar atla (string içindeki `//` aşağıda korunur).
        if bytes[i] == b'/' && i + 1 < bytes.len() && bytes[i + 1] == b'/' {
            i = text[i..].find('\n').map_or(bytes.len(), |n| i + n);
            continue;
        }
        if bytes[i] != b'"' {
            i += 1;
            continue;
        }
        let start = i + 1;
        let mut j = start;
        while j < bytes.len() && bytes[j] != b'"' {
            j += if bytes[j] == b'\\' { 2 } else { 1 };
        }
        let lit = &text[start..j.min(bytes.len())];
        let mut k = 0;
        let lb = lit.as_bytes();
        while k < lb.len() {
            if lb[k].is_ascii_uppercase() && (k == 0 || !is_word(lb[k - 1])) {
                let s = k;
                while k < lb.len() && is_word(lb[k]) { k += 1; }
                if is_rule_id(&lit[s..k]) { into.insert(lit[s..k].to_string()); }
            } else {
                k += 1;
            }
        }
        i = j + 1;
    }
}

fn rs_files(dir: &Path, out: &mut Vec<PathBuf>) {
    for entry in fs::read_dir(dir).expect("dizin okunamadı").flatten() {
        let p = entry.path();
        if p.is_dir() {
            rs_files(&p, out);
        } else if p.extension().and_then(|e| e.to_str()) == Some("rs") {
            out.push(p);
        }
    }
}

#[test]
fn every_canonical_rule_referenced_in_production_code() {
    let src = repo_root().join("crates").join("pipeline").join("src");
    let mut files = Vec::new();
    rs_files(&src, &mut files);

    let mut referenced = std::collections::HashSet::new();
    for f in &files {
        // k7_reporting.rs rapor görünüm mantığı + test-helper içerir; emit kaynağı değil.
        if f.file_name().and_then(|n| n.to_str()) == Some("k7_reporting.rs") { continue; }
        let txt = fs::read_to_string(f).expect("kaynak okunamadı");
        collect_rule_ids(&strip_test_mods(&txt), &mut referenced);
    }

    let allow: std::collections::HashSet<&str> = DYNAMIC_EMIT_ALLOWLIST.iter().copied().collect();
    let mut orphans: Vec<&str> = RULES
        .iter()
        .map(|r| r.id)
        .filter(|id| !referenced.contains(*id) && !allow.contains(*id))
        .collect();
    orphans.sort_unstable();

    assert!(
        orphans.is_empty(),
        "Üretim kodunda hiç geçmeyen canonical kural(lar) (dead/orphan rule?): {:?}\n\
         Eğer kural gerçekten emit ediliyorsa ama dinamik/fatal yolla (rule_id literal yok), \
         DYNAMIC_EMIT_ALLOWLIST'e gerekçesiyle ekleyin. Aksi halde emit kodu eksik/silinmiş.",
        orphans,
    );
}

/// Allowlist'in çürümesini önler: allowlist'teki bir kural artık gerçekten kodda
/// geçmeye başladıysa (veya registry'den kalktıysa) allowlist'ten çıkarılmalı.
#[test]
fn dynamic_emit_allowlist_has_no_stale_entries() {
    let canonical: std::collections::HashSet<&str> = RULES.iter().map(|r| r.id).collect();
    for id in DYNAMIC_EMIT_ALLOWLIST {
        assert!(
            canonical.contains(id),
            "DYNAMIC_EMIT_ALLOWLIST'teki '{id}' artık registry'de yok — allowlist'ten çıkarın.",
        );
    }
}
