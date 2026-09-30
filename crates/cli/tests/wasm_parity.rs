//! Native ↔ WASM parite referansı.
//!
//! `tests/fixtures/wasm-parity/cases.json` vakalarını gerçek CLI ile koşar ve notice'ların
//! karşılaştırılan alanlarını `expected.json` ile kıyaslar. SDK tarafı
//! (`sdk/scripts/parity-test.mjs`) AYNI vakaları WASM'da koşup AYNI dosyayla kıyaslar;
//! ikisi de eşleşirse native ve WASM birbirine eşittir. Böylece CI'da iki runtime aynı
//! job'da derlenmek zorunda kalmaz.
//!
//! 2026-09-30: 22 feed-level toplulama yalnız native'de çalışıyordu; native CLI koşan
//! korpus denetimi WASM sapmasını göremediği için hata yayına çıktı. Bu kapı onu yakalar.
//!
//! Beklenen çıktıyı yeniden üretmek (bilinçli davranış değişikliğinde):
//! `GTFS_BLESS_PARITY=1 cargo test -p gtfs-analyzer --test wasm_parity`

use std::io::Write as _;
use std::path::PathBuf;
use std::process::{Command, Stdio};

use serde_json::{json, Map, Value};
use zip::write::SimpleFileOptions;

/// Karşılaştırılan notice alanları. `id` bilinçli olarak yok: K7 numaralandırması
/// cap'lenmiş listeden yapılır. SDK betiği aynı listeyi kullanır.
const FIELDS: &[&str] = &[
    "rule_id",
    "severity",
    "rule_class",
    "entity_type",
    "entity_id",
    "scope_key",
    "file",
    "line",
    "field",
    "observed_value",
    "expected_value",
    "details",
    "service_id",
    "title",
    "message",
    "remediation",
];

fn fixture_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../tests/fixtures/wasm-parity")
}

fn make_zip(files: &Map<String, Value>) -> Vec<u8> {
    let mut writer = zip::ZipWriter::new(std::io::Cursor::new(Vec::new()));
    let options = SimpleFileOptions::default().compression_method(zip::CompressionMethod::Stored);
    for (name, content) in files {
        writer.start_file(name.as_str(), options).expect("zip girdisi açılamadı");
        writer
            .write_all(content.as_str().expect("dosya içeriği string olmalı").as_bytes())
            .expect("zip girdisi yazılamadı");
    }
    writer.finish().expect("zip kapanmadı").into_inner()
}

fn run_cli(zip: &[u8], today: &str) -> Value {
    let mut child = Command::new(env!("CARGO_BIN_EXE_gtfs-analyzer"))
        .args(["validate", "-", "--json", "--lang", "en", "--today", today])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("gtfs-analyzer başlatılamadı");
    child.stdin.take().unwrap().write_all(zip).unwrap();
    let out = child.wait_with_output().unwrap();
    // 0 = temiz, 1 = yayın engeli; ikisi de geçerli sonuçtur.
    assert!(
        matches!(out.status.code(), Some(0 | 1)),
        "CLI beklenmeyen çıkış: {:?}\n{}",
        out.status,
        String::from_utf8_lossy(&out.stderr)
    );
    serde_json::from_slice(&out.stdout).expect("CLI çıktısı JSON değil")
}

/// SDK betiğindeki `project` ile birebir aynı olmalı.
fn project(result: &Value) -> Value {
    let mut notices: Vec<Value> = result["notices"]
        .as_array()
        .expect("notices dizisi yok")
        .iter()
        .map(|n| {
            let mut out = Map::new();
            for field in FIELDS {
                out.insert(field.to_string(), n.get(*field).cloned().unwrap_or(Value::Null));
            }
            Value::Object(out)
        })
        .collect();
    notices.sort_by_cached_key(|n| n.to_string());
    json!({
        "publishable": result["reports"]["r1"]["publishable"],
        "score": result["reports"]["r5"]["score"],
        "notices": notices,
    })
}

/// `case` ile `base` notice çoklukümeleri arasındaki fark (her iki yönde, sıralı).
/// SDK betiğindeki `diff` ile birebir aynı olmalı.
fn diff(case: &Value, base: &Value) -> Value {
    let key = |n: &Value| n.to_string();
    let mut remaining: Vec<&Value> = base["notices"].as_array().unwrap().iter().collect();
    let mut added = Vec::new();
    for n in case["notices"].as_array().unwrap() {
        match remaining.iter().position(|b| key(b) == key(n)) {
            Some(i) => {
                remaining.remove(i);
            }
            None => added.push(n.clone()),
        }
    }
    let removed: Vec<Value> = remaining.into_iter().cloned().collect();
    json!({
        "publishable": case["publishable"],
        "score": case["score"],
        "added": added,
        "removed": removed,
    })
}

fn files_of(base: &Map<String, Value>, overrides: Option<&Value>) -> Map<String, Value> {
    let mut files = base.clone();
    if let Some(overrides) = overrides {
        for (file, content) in overrides.as_object().unwrap() {
            files.insert(file.clone(), content.clone());
        }
    }
    files
}

#[test]
fn native_output_matches_the_shared_parity_reference() {
    let dir = fixture_dir();
    let cases: Value =
        serde_json::from_str(&std::fs::read_to_string(dir.join("cases.json")).unwrap()).unwrap();
    let today = cases["today"].as_str().unwrap();
    let base_files = cases["base"].as_object().unwrap();

    // Taban feed'in bulguları her vakada tekrar eder; bir kez saklanır, vakalar yalnız
    // farkı taşır. Taban eşit VE fark eşitse tüm küme eşittir.
    let base = project(&run_cli(&make_zip(&files_of(base_files, None)), today));
    let mut by_case = Map::new();
    for case in cases["cases"].as_array().unwrap() {
        let name = case["name"].as_str().unwrap();
        let projected = project(&run_cli(&make_zip(&files_of(base_files, Some(&case["files"]))), today));

        // Vaka amacını kaybetmesin: hedef kural ateşlemeli.
        let rule = case["rule"].as_str().unwrap();
        assert!(
            projected["notices"]
                .as_array()
                .unwrap()
                .iter()
                .any(|n| n["rule_id"] == rule),
            "{name}: {rule} ateşlemedi; fixture artık bir şey sınamıyor"
        );
        by_case.insert(name.to_string(), diff(&projected, &base));
    }
    let actual = json!({ "base": base, "cases": by_case });
    let actual = serde_json::to_string_pretty(&actual).unwrap() + "\n";

    let path = dir.join("expected.json");
    if std::env::var_os("GTFS_BLESS_PARITY").is_some() {
        std::fs::write(&path, &actual).unwrap();
        return;
    }
    let expected = std::fs::read_to_string(&path)
        .expect("expected.json yok; GTFS_BLESS_PARITY=1 ile üretin");
    assert!(
        expected == actual,
        "native çıktı parite referansından ayrıştı. Değişiklik bilinçliyse \
         GTFS_BLESS_PARITY=1 ile yeniden üretin ve SDK parite testini koşun."
    );
}
