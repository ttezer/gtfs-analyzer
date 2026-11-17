//! Issue #201 closing gate: engine code carries no Turkish user text.
//!
//! Every user-facing text the engine writes is English; Turkish (like Japanese and French) comes
//! from the locale dictionaries (`ui/src/locales/tr.ts` → `crates/core/locales/tr.json`). This
//! test scans the string literals in the production code of every crate and fails if one looks
//! Turkish — Turkish letters, or Turkish words in a multi-word text (Turkish written without its
//! letters, "tanimli degil", slipped past letter-only scans twice during the migration).
//!
//! Skipped: comments, `#[cfg(test)]` modules, `#[test]` functions and `k1dbg!` developer logs.
//! A new rule writes its message in English and adds the Turkish text to `tr.ts`
//! (`trMessageTexts` for a fixed text, `trMessageTemplates` for a text with values).

use std::path::{Path, PathBuf};

/// Literals that legitimately contain Turkish letters, each with its reason.
const ALLOWED: &[(&str, &str)] = &[(
    "valid Unicode letters (e.g. ü, ö, 漢字)",
    "English remediation that names example letters",
)];

/// Turkish words that are not English words; checked only in a text with a space.
const TURKISH_WORDS: &[&str] = &[
    "ve", "ile", "icin", "sefer", "seferde", "seferi", "hatta", "hattin", "durak", "duragi", "gun",
    "dk", "sn", "satir", "satirda", "yok", "eksik", "bos", "tanimli", "degil", "gecerli",
    "gecersiz", "kodlu", "arasi", "zorunlu", "zorunludur", "olmali", "olmalidir", "bulunamadi",
    "kayit", "alani", "dosyasi", "dosyasinda", "okunamadi", "bayt", "girdi", "uzakta", "esik",
    "ucret", "urunu", "varsayilan", "yolcu", "hicbir", "hicbiri", "kategori", "kategorisi",
];

fn turkish_reason(text: &str) -> Option<String> {
    if let Some(c) = text.chars().find(|c| "çğıİşŞöüÇĞÖÜ".contains(*c)) {
        return Some(format!("Turkish letter '{c}'"));
    }
    if !text.contains(' ') {
        return None;
    }
    // Placeholder names (`{sn_disp}`) are code, not words.
    let mut words = String::with_capacity(text.len());
    let mut depth = 0usize;
    for c in text.chars() {
        match c {
            '{' => depth += 1,
            '}' => depth = depth.saturating_sub(1),
            _ if depth == 0 => words.push(c),
            _ => {}
        }
    }
    words
        .split(|c: char| !c.is_ascii_alphanumeric())
        .map(str::to_ascii_lowercase)
        .find(|w| TURKISH_WORDS.contains(&w.as_str()))
        .map(|w| format!("Turkish word '{w}'"))
}

/// String literals of a Rust source with their byte offset, and the source with every literal
/// and comment blanked out (same length), for structural searches.
fn lex(src: &str) -> (Vec<(usize, String)>, Vec<u8>) {
    let b = src.as_bytes();
    let mut code = b.to_vec();
    let mut lits = Vec::new();
    let blank = |code: &mut Vec<u8>, from: usize, to: usize| {
        for byte in &mut code[from..to] {
            if *byte != b'\n' {
                *byte = b' ';
            }
        }
    };
    let ident = |c: u8| c.is_ascii_alphanumeric() || c == b'_';
    let mut i = 0;
    while i < b.len() {
        let prev = if i > 0 { b[i - 1] } else { b' ' };
        if b[i..].starts_with(b"//") {
            let end = src[i..].find('\n').map_or(b.len(), |n| i + n);
            blank(&mut code, i, end);
            i = end;
        } else if b[i..].starts_with(b"/*") {
            let (mut depth, mut j) = (1, i + 2);
            while j < b.len() && depth > 0 {
                if b[j..].starts_with(b"/*") {
                    depth += 1;
                    j += 2;
                } else if b[j..].starts_with(b"*/") {
                    depth -= 1;
                    j += 2;
                } else {
                    j += 1;
                }
            }
            blank(&mut code, i, j);
            i = j;
        } else if (b[i] == b'r' || (b[i] == b'b' && b.get(i + 1) == Some(&b'r'))) && !ident(prev) {
            // Raw string: r"..", r#".."#, br"..".
            let mut j = if b[i] == b'b' { i + 2 } else { i + 1 };
            let hashes = b[j..].iter().take_while(|&&c| c == b'#').count();
            j += hashes;
            if b.get(j) == Some(&b'"') {
                let close = format!("\"{}", "#".repeat(hashes));
                let end = src[j + 1..].find(&close).map_or(b.len(), |n| j + 1 + n);
                lits.push((i, src[j + 1..end].to_string()));
                let stop = (end + close.len()).min(b.len());
                blank(&mut code, i, stop);
                i = stop;
            } else {
                i += 1;
            }
        } else if b[i] == b'"' {
            let mut j = i + 1;
            while j < b.len() && b[j] != b'"' {
                j += if b[j] == b'\\' { 2 } else { 1 };
            }
            lits.push((i, src[i + 1..j.min(b.len())].to_string()));
            blank(&mut code, i, (j + 1).min(b.len()));
            i = j + 1;
        } else if b[i] == b'\'' {
            // A char literal ('x', '\'', '"', '\u{..}'), not a lifetime.
            let rest = &src[i + 1..];
            let len = if let Some(escaped) = rest.strip_prefix('\\') {
                escaped.find('\'').map(|n| n + 2)
            } else {
                rest.chars().next().filter(|c| rest[c.len_utf8()..].starts_with('\'')).map(char::len_utf8)
            };
            match len {
                Some(n) => {
                    blank(&mut code, i, i + n + 2);
                    i += n + 2;
                }
                None => i += 1,
            }
        } else {
            i += 1;
        }
    }
    (lits, code)
}

/// From `start`, the range up to the end of the next `{ … }` (or `( … )` for a macro) block.
fn block_after(code: &[u8], start: usize, open: u8, close: u8) -> std::ops::Range<usize> {
    let Some(first) = code[start..].iter().position(|&c| c == open).map(|n| start + n) else {
        return start..start;
    };
    let mut depth = 0usize;
    for (j, &c) in code.iter().enumerate().skip(first) {
        if c == open {
            depth += 1;
        } else if c == close {
            depth -= 1;
            if depth == 0 {
                return start..j + 1;
            }
        }
    }
    start..code.len()
}

fn skipped_ranges(code: &[u8]) -> Vec<std::ops::Range<usize>> {
    let text = String::from_utf8_lossy(code);
    let mut ranges = Vec::new();
    for marker in ["#[cfg(test)]", "#[test]"] {
        for (at, _) in text.match_indices(marker) {
            ranges.push(block_after(code, at, b'{', b'}'));
        }
    }
    for (at, _) in text.match_indices("k1dbg!") {
        ranges.push(block_after(code, at, b'(', b')'));
    }
    ranges
}

fn rust_files(dir: &Path, out: &mut Vec<PathBuf>) {
    for entry in std::fs::read_dir(dir).expect("readable source directory") {
        let path = entry.expect("directory entry").path();
        if path.is_dir() {
            rust_files(&path, out);
        } else if path.extension().is_some_and(|e| e == "rs") {
            out.push(path);
        }
    }
}

fn findings() -> Vec<String> {
    let crates = Path::new(env!("CARGO_MANIFEST_DIR")).join("..");
    let mut files = Vec::new();
    for name in ["cli", "config", "core", "pipeline", "python", "rules", "wasm"] {
        rust_files(&crates.join(name).join("src"), &mut files);
    }
    files.sort();
    let mut out = Vec::new();
    for path in files {
        let src = std::fs::read_to_string(&path).expect("readable source");
        let (lits, code) = lex(&src);
        let skipped = skipped_ranges(&code);
        for (at, text) in lits {
            if skipped.iter().any(|r| r.contains(&at))
                || ALLOWED.iter().any(|(allowed, _)| text.contains(allowed))
            {
                continue;
            }
            if let Some(reason) = turkish_reason(&text) {
                let line = src[..at].matches('\n').count() + 1;
                let short: String = text.chars().take(90).collect();
                out.push(format!("{}:{line}: {reason}: \"{short}\"", path.display()));
            }
        }
    }
    out
}

#[test]
fn engine_code_has_no_turkish_user_text() {
    let found = findings();
    assert!(
        found.is_empty(),
        "Turkish user text in engine code ({}). Write the text in English and put the Turkish \
         in ui/src/locales/tr.ts (trMessageTexts / trMessageTemplates), then run \
         `npm --prefix ui run locales:export`:\n{}",
        found.len(),
        found.join("\n")
    );
}

#[test]
fn the_scanner_sees_what_it_must_and_skips_what_it_may() {
    let src = r##"
        fn emit() { notice("Durak eksik."); notice("Stop '{}' is missing."); }
        fn ascii() { let m = "kayit tanimli degil"; let c = '"'; let l: &'static str = "ok"; }
        // "yorumdaki Türkçe sayılmaz"
        /* "blok yorum da" /* iç içe "değil" */ */
        fn raw() { let r = r#"ham "metin" Türkçe"#; }
        #[cfg(test)]
        mod tests { #[test] fn t() { assert_eq!(x, "test içinde serbest"); } }
        #[test]
        fn top() { let s = "üst düzey test serbest"; }
        fn dbg() { k1dbg!("[K1] okuma başladı: {}", name); }
        fn placeholder() { let p = format!("{sn_disp} — {ln_disp}"); }
    "##;
    let (lits, code) = lex(src);
    let skipped = skipped_ranges(&code);
    let flagged: Vec<String> = lits
        .into_iter()
        .filter(|(at, _)| !skipped.iter().any(|r| r.contains(at)))
        .filter_map(|(_, text)| turkish_reason(&text).map(|_| text))
        .collect();
    assert_eq!(flagged, ["Durak eksik.", "kayit tanimli degil", "ham \"metin\" Türkçe"]);
}
