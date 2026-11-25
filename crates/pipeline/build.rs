use std::{env, fs, path::PathBuf};

fn main() {
    let source = PathBuf::from(env::var("CARGO_MANIFEST_DIR").unwrap())
        .join("data/spdx-licenses-3.29.0.json");
    println!("cargo:rerun-if-changed={}", source.display());
    let json: serde_json::Value =
        serde_json::from_slice(&fs::read(&source).expect("SPDX list missing"))
            .expect("SPDX list is not valid JSON");
    let version = json["licenseListVersion"]
        .as_str()
        .expect("SPDX list has no version");
    assert_eq!(
        version, "3.29.0",
        "SPDX source version differs from pinned version"
    );
    let mut ids: Vec<String> = json["licenses"]
        .as_array()
        .expect("SPDX list has no licenses")
        .iter()
        .map(|license| {
            license["licenseId"]
                .as_str()
                .expect("SPDX license has no ID")
                .to_ascii_lowercase()
        })
        .collect();
    ids.sort_unstable();
    ids.dedup();
    let mut generated = format!("pub(crate) const VERSION: &str = {version:?};\n");
    generated.push_str("pub(crate) const IDS: &[&str] = &[\n");
    for id in ids {
        generated.push_str(&format!("    {id:?},\n"));
    }
    generated.push_str("];\n");
    fs::write(
        PathBuf::from(env::var("OUT_DIR").unwrap()).join("spdx_ids.rs"),
        generated,
    )
    .expect("cannot write SPDX IDs");
}
