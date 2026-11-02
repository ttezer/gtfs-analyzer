//! Native/WASM parity guard: feed-level aggregation.
//!
//! Feed-level aggregation for 22 rules existed only in native `validate_bytes`; the WASM
//! orchestrators had skipped it. Because the registry puts those rules at `Feed` dedup level,
//! the browser and npm SDK showed one raw-row notice and lost the affected-record count. A
//! corpus audit running the native CLI CANNOT SEE this; the guard therefore operates at source
//! level: every orchestrator must call aggregation BEFORE report scoping (and BEFORE the cap
//! in WASM).

const WASM_SRC: &str = include_str!("../src/lib.rs");
const NATIVE_SRC: &str = include_str!("../../pipeline/src/lib.rs");

/// Removes line-start `//` comments; rationale comments contain function names.
fn code_only(src: &str) -> String {
    src.lines()
        .filter(|l| !l.trim_start().starts_with("//"))
        .collect::<Vec<_>>()
        .join("\n")
}

/// Returns the body beginning with `fn <name>(` up to the next top-level `fn`.
fn body_of<'a>(src: &'a str, name: &str) -> &'a str {
    let start = src
        .find(&format!("fn {name}("))
        .unwrap_or_else(|| panic!("`{name}` was not found"));
    let rest = &src[start + 1..];
    let end = ["\nfn ", "\npub fn ", "\n#[wasm_bindgen"]
        .iter()
        .filter_map(|m| rest.find(m))
        .min()
        .map_or(rest.len(), |e| e + 1);
    &src[start..start + end]
}

fn assert_aggregates_before(body: &str, name: &str, later: &[&str]) {
    let agg = body
        .find("aggregate_feed_level_notices(")
        .unwrap_or_else(|| panic!("`{name}` does not call feed-level aggregation"));
    for call in later {
        let pos = body
            .find(call)
            .unwrap_or_else(|| panic!("`{name}` does not contain `{call}`"));
        assert!(agg < pos, "`{name}`: aggregation must be called BEFORE `{call}`");
    }
}

#[test]
fn wasm_full_pipeline_aggregates_before_scope_and_cap() {
    let src = code_only(WASM_SRC);
    assert_aggregates_before(
        body_of(&src, "run_full_pipeline"),
        "run_full_pipeline",
        &["apply_report_scope(", "prepare_count_cap("],
    );
}

#[test]
fn wasm_cached_rerun_aggregates_before_scope_and_cap() {
    let src = code_only(WASM_SRC);
    assert_aggregates_before(
        body_of(&src, "rerun_k6_k7_inner"),
        "rerun_k6_k7_inner",
        &["apply_report_scope(", "prepare_count_cap("],
    );
}

#[test]
fn native_validate_bytes_aggregates_before_scope() {
    let src = code_only(NATIVE_SRC);
    // `validate_bytes` is a thin wrapper for the audit observer; the actual orchestrator is
    // `validate_bytes_inspected_with_name_index`. If the wrapper takes another path, the guard
    // goes blind.
    assert!(
        body_of(&src, "validate_bytes").contains("validate_bytes_inspected("),
        "`validate_bytes` no longer delegates to `validate_bytes_inspected`"
    );
    assert_aggregates_before(
        body_of(&src, "validate_bytes_inspected_with_name_index"),
        "validate_bytes_inspected_with_name_index",
        &["apply_report_scope("],
    );
}
