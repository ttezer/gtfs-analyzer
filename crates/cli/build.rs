use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

fn main() {
    println!("cargo:rerun-if-changed=.cargo_vcs_info.json");
    println!("cargo:rerun-if-env-changed=GTFS_ANALYZER_COMMIT");

    let manifest_dir = PathBuf::from(env::var_os("CARGO_MANIFEST_DIR").unwrap());
    let build_commit = match env::var("GTFS_ANALYZER_COMMIT") {
        Ok(commit) if !commit.trim().is_empty() => Some((commit, "build_env".to_owned())),
        Ok(_) => panic!("GTFS_ANALYZER_COMMIT must be set for container builds"),
        Err(env::VarError::NotPresent) => None,
        Err(err) => panic!("could not read GTFS_ANALYZER_COMMIT: {err}"),
    };
    let (commit, source) = build_commit
        .or_else(|| commit_from_cargo_vcs_info(&manifest_dir))
        .or_else(|| commit_from_git(&manifest_dir))
        .unwrap_or_else(|| ("unknown".to_owned(), "unavailable".to_owned()));

    let out_dir = PathBuf::from(env::var_os("OUT_DIR").unwrap());
    let generated =
        format!("pub const COMMIT: &str = {commit:?};\npub const SOURCE: &str = {source:?};\n");
    fs::write(out_dir.join("provenance.rs"), generated).unwrap();
}

fn commit_from_git(manifest_dir: &Path) -> Option<(String, String)> {
    let output = Command::new("git")
        .args(["rev-parse", "HEAD"])
        .current_dir(manifest_dir)
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }
    let commit = String::from_utf8(output.stdout).ok()?.trim().to_owned();
    (!commit.is_empty()).then(|| (commit, "git".to_owned()))
}

fn commit_from_cargo_vcs_info(manifest_dir: &Path) -> Option<(String, String)> {
    let path = manifest_dir.join(".cargo_vcs_info.json");
    let contents = fs::read_to_string(path).ok()?;
    let json: serde_json::Value = serde_json::from_str(&contents).ok()?;
    let commit = json.get("git")?.get("sha1")?.as_str()?.trim();
    (!commit.is_empty()).then(|| (commit.to_owned(), "cargo_vcs_info".to_owned()))
}
