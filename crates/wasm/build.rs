use std::env;
use std::fs;
use std::path::PathBuf;

use serde_json::Value;

fn main() {
    let locale_path =
        PathBuf::from(env::var("CARGO_MANIFEST_DIR").unwrap()).join("../cli/locales/en.json");
    println!("cargo:rerun-if-changed={}", locale_path.display());

    let raw = fs::read_to_string(&locale_path).expect("English locale file is not readable");
    let dictionary: Value =
        serde_json::from_str(&raw).expect("English locale file is not valid JSON");
    let generated = serde_json::to_string(&dictionary).expect("English locale cannot be minified");

    let out_dir = PathBuf::from(env::var("OUT_DIR").unwrap());
    fs::write(out_dir.join("en_locale.json"), generated).expect("cannot write generated locale");
}
