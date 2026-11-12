//! Gate for locale-template ↔ emit consistency.
//!
//! `en/ja/fr` messages are `{placeholder}` templates; `i18n::fill` can fill them from only
//! two sources: six fixed `Notice` fields and the `notice.details` map.
//! A missing key does NOT ERROR; it silently becomes an empty string (`params[k] ?? ''`).
//! Because the Turkish text came from `format!` in the rule, the defect lives only in the
//! translation layer — on 2026-08-29 it appeared as `路線''：…` in Katori's Japanese output.
//!
//! There are two gates because the defect has two forms:
//!  1. `locale_placeholders_can_be_filled` — the name requested by a template is produced nowhere
//!     in code. It is static and covers every rule, but cannot answer "does this rule's emit"
//!     produce the name: the name may be produced by another rule, leaving the gate silent.
//!  2. `emitted_notices_fill_their_locale_placeholders` — a generated notice cannot fill its
//!     template. It is exact at rule level, but sees only rules triggered by the fixture.

use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::io::Write as _;
use std::path::{Path, PathBuf};
use std::process::Command;

use zip::write::SimpleFileOptions;

/// Keys fillable from the fixed fields of `Notice` (`i18n::resolve`).
const FIXED_KEYS: &[&str] = &[
    "entity_id",
    "observed_value",
    "expected_value",
    "file",
    "field",
    "line",
];

fn repo_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()
        .expect("repo kökü bulunamadı")
}

fn rust_sources(dir: &Path, out: &mut Vec<PathBuf>) {
    let Ok(entries) = fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            if path.file_name().is_some_and(|n| n == "target") {
                continue;
            }
            rust_sources(&path, out);
        } else if path.extension().is_some_and(|e| e == "rs") {
            out.push(path);
        }
    }
}

/// snake_case names that occur as string literals in the source.
///
/// The `details` map is built differently at each emit — `details.insert(…)`, `d.insert(…)`,
/// `BTreeMap::from([("ad", …)])`, `[(…)].into_iter().collect()`. Searching by key name is
/// the only approach covering all these forms. The gate is deliberately loose: its purpose is
/// to catch a template requesting a name PRODUCED NOWHERE; the runtime gate answers "does this
/// rule's emit write this key?".
fn source_string_literals() -> BTreeSet<String> {
    let mut sources = Vec::new();
    rust_sources(&repo_root().join("crates"), &mut sources);
    let mut keys = BTreeSet::new();
    for path in sources {
        let Ok(src) = fs::read_to_string(&path) else {
            continue;
        };
        // Comment lines and `'"'` char literals hold unpaired quotes; left in, they shift the
        // pairing so the rest of the file yields prose instead of string literals.
        let src: String = src
            .lines()
            .filter(|line| !line.trim_start().starts_with("//"))
            .map(|line| line.replace("'\"'", ""))
            .collect::<Vec<_>>()
            .join("\n");
        let mut rest = src.as_str();
        while let Some(open) = rest.find('"') {
            rest = &rest[open + 1..];
            let Some(end) = rest.find('"') else { break };
            let literal = &rest[..end];
            if !literal.is_empty()
                && literal.len() > 2
                && literal.chars().all(|c| c.is_ascii_lowercase() || c == '_')
            {
                keys.insert(literal.to_string());
            }
            rest = &rest[end + 1..];
        }
    }
    keys
}

fn placeholders(template: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut rest = template;
    while let Some(open) = rest.find('{') {
        let after = &rest[open + 1..];
        let Some(close) = after.find('}') else { break };
        let key = &after[..close];
        if !key.is_empty() && key.chars().all(|c| c.is_alphanumeric() || c == '_') {
            out.push(key.to_string());
        }
        rest = &after[close + 1..];
    }
    out
}

#[test]
fn locale_placeholders_can_be_filled() {
    let allowed: BTreeSet<String> = FIXED_KEYS
        .iter()
        .map(|k| (*k).to_string())
        .chain(source_string_literals())
        .collect();

    // rule id → (placeholder, diller)
    let mut problems: BTreeMap<String, BTreeSet<String>> = BTreeMap::new();
    for locale in ["en", "ja", "fr"] {
        let path = repo_root()
            .join("crates/core/locales")
            .join(format!("{locale}.json"));
        let raw = fs::read_to_string(&path).unwrap_or_else(|e| panic!("{path:?} okunamadı: {e}"));
        let json: serde_json::Value =
            serde_json::from_str(&raw).expect("locale JSON ayrıştırılamadı");
        let messages = json["messages"]
            .as_object()
            .expect("locale dosyasında `messages` bölümü yok");
        for (rule_id, template) in messages {
            let Some(template) = template.as_str() else {
                continue;
            };
            for key in placeholders(template) {
                if !allowed.contains(&key) {
                    problems
                        .entry(format!("{rule_id} [{locale}]"))
                        .or_default()
                        .insert(key);
                }
            }
        }
    }

    assert!(
        problems.is_empty(),
        "\n{} mesaj şablonu doldurulamayan placeholder içeriyor (sessizce boş string olur):\n{}\n\
         Çözüm: emit noktasında `details.insert(\"<ad>\", …)` ile değeri üretin.\n",
        problems.len(),
        problems
            .iter()
            .map(|(rule, keys)| format!(
                "  {rule}: {}",
                keys.iter()
                    .map(|k| format!("{{{k}}}"))
                    .collect::<Vec<_>>()
                    .join(", ")
            ))
            .collect::<Vec<_>>()
            .join("\n")
    );
}

// ── Runtime gate ───────────────────────────────────────────────────────────────
//
// Static scanning catches missing names; runtime fixtures catch rule-specific omissions.
// Keep both checks because neither one subsumes the other.
// The name-level gate sees only "this key is never produced in code". The actual defect is at
// rule level: another rule's emit may produce the key, but it may be absent from THIS rule's
// notice — for `OPR_004` in the Katori feed, the Japanese text was
// `路線''：週末に運行がありません。`. We can see this only by matching a generated notice to its template.

static AGENCY: &[u8] =
    b"agency_id,agency_name,agency_url,agency_timezone\n1,Test,http://test.example,UTC\n";
static STOPS: &[u8] =
    b"stop_id,stop_name,stop_lat,stop_lon\nS1,Stop1,41.0,29.0\nS2,Stop2,41.1,29.1\n";
static ROUTES: &[u8] = b"route_id,agency_id,route_short_name,route_type\nR1,1,101,3\n";
static TRIPS: &[u8] = b"route_id,service_id,trip_id\nR1,SVC1,T1\n";
static STOP_TIMES: &[u8] = b"trip_id,arrival_time,departure_time,stop_id,stop_sequence\n\
      T1,08:00:00,08:00:00,S1,1\nT1,08:10:00,08:10:00,S2,2\n";
/// Closed on weekends: triggers OPR_004.
static CALENDAR: &[u8] =
    b"service_id,monday,tuesday,wednesday,thursday,friday,saturday,sunday,start_date,end_date\n\
      SVC1,1,1,1,1,1,0,0,20250101,20271231\n";

fn fixture_feed() -> PathBuf {
    let files: [(&str, &[u8]); 6] = [
        ("agency.txt", AGENCY),
        ("stops.txt", STOPS),
        ("routes.txt", ROUTES),
        ("trips.txt", TRIPS),
        ("stop_times.txt", STOP_TIMES),
        ("calendar.txt", CALENDAR),
    ];
    let cursor = std::io::Cursor::new(Vec::new());
    let mut writer = zip::ZipWriter::new(cursor);
    for (name, data) in files {
        writer
            .start_file(name, SimpleFileOptions::default())
            .unwrap();
        writer.write_all(data).unwrap();
    }
    let bytes = writer.finish().unwrap().into_inner();

    let path = std::env::temp_dir().join("gtfs-locale-placeholders.zip");
    let staging = std::env::temp_dir().join(format!(
        "gtfs-locale-placeholders.{:?}.tmp",
        std::thread::current().id()
    ));
    fs::write(&staging, bytes).unwrap();
    fs::rename(&staging, &path).unwrap();
    path
}

/// Same contract as `i18n::resolve`: fixed fields + `details`.
fn value_for(key: &str, notice: &serde_json::Value) -> Option<String> {
    if let Some(v) = notice
        .get("details")
        .and_then(|d| d.get(key))
        .and_then(|v| v.as_str())
    {
        return Some(v.to_string());
    }
    let raw = match key {
        "entity_id" | "observed_value" | "expected_value" | "file" | "field" => notice.get(key),
        "line" => notice.get("line"),
        _ => None,
    }?;
    match raw {
        serde_json::Value::Null => None,
        serde_json::Value::String(s) if s.is_empty() => None,
        serde_json::Value::String(s) => Some(s.clone()),
        other => Some(other.to_string()),
    }
}

#[test]
fn emitted_notices_fill_their_locale_placeholders() {
    let feed = fixture_feed();
    let output = Command::new(env!("CARGO_BIN_EXE_gtfs-analyzer"))
        .args([
            "validate",
            feed.to_str().unwrap(),
            "--today",
            "20260515",
            "--lang",
            "en",
            "--json",
        ])
        .output()
        .expect("gtfs-analyzer çalıştırılamadı");
    let json: serde_json::Value =
        serde_json::from_str(&String::from_utf8(output.stdout).unwrap()).expect("JSON değil");
    let notices = json["notices"].as_array().expect("notices dizisi yok");
    assert!(
        !notices.is_empty(),
        "fixture hiç notice üretmedi — kapı ölçmüyor demektir"
    );

    let raw = fs::read_to_string(repo_root().join("crates/core/locales/en.json")).unwrap();
    let templates: serde_json::Value = serde_json::from_str(&raw).unwrap();
    let templates = templates["messages"].as_object().unwrap();

    let mut problems: BTreeMap<String, BTreeSet<String>> = BTreeMap::new();
    for notice in notices {
        let rule_id = notice["rule_id"].as_str().unwrap_or_default();
        let Some(template) = templates.get(rule_id).and_then(|t| t.as_str()) else {
            continue;
        };
        for key in placeholders(template) {
            if value_for(&key, notice).is_none() {
                problems.entry(rule_id.to_string()).or_default().insert(key);
            }
        }
    }

    assert!(
        problems.is_empty(),
        "\n{} kural, ürettiği notice'ta şablonunun beklediği değeri taşımıyor \
         (en/ja/fr metni eksik çıkar, Türkçe metin `format!`'ten geldiği için sağlam görünür):\n{}\n",
        problems.len(),
        problems
            .iter()
            .map(|(rule, keys)| format!(
                "  {rule}: {}",
                keys.iter().map(|k| format!("{{{k}}}")).collect::<Vec<_>>().join(", ")
            ))
            .collect::<Vec<_>>()
            .join("\n")
    );
}
