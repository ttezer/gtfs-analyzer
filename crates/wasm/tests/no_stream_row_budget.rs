//! #185 guard: the WASM path MUST NOT pass a row budget to K2.
//!
//! The regression was a call site, not a function: `run_full_pipeline` and the cached path
//! passed `Some(config.max_file_rows)` to `validate_k2_with_whitespace_roots`. The result was
//! silent truncation of streamed files at 1,000,000 rows, with references created by the
//! truncation reported as feed defects (VBB/mdb-782: 250,407 fabricated TRP_004 + 220,752
//! XFL_002, actual count 25,369).
//!
//! Because this behavior is browser-specific, a full corpus audit can NEVER SEE IT —
//! benchmark/audit_all runs the native CLI. The guard therefore operates here at source level:
//! the call site's fourth argument must be `None`.

const SRC: &str = include_str!("../src/lib.rs");

/// Removes line-start `//` comments; because rationale comments contain rule names, the scan
/// must inspect only actual code.
fn code_only(src: &str) -> String {
    src.lines()
        .filter(|l| !l.trim_start().starts_with("//"))
        .collect::<Vec<_>>()
        .join("\n")
}

fn top_level_arguments(call: &str) -> Vec<&str> {
    let open = call.find('(').expect("call has no opening delimiter") + 1;
    let close = call.rfind(");").expect("call has no closing delimiter");
    let args = &call[open..close];
    let mut result = Vec::new();
    let mut depth = 0usize;
    let mut start = 0usize;
    for (index, character) in args.char_indices() {
        match character {
            '(' => depth += 1,
            ')' => depth = depth.checked_sub(1).expect("parantez dengesi bozuk"),
            ',' if depth == 0 => {
                result.push(args[start..index].trim());
                start = index + character.len_utf8();
            }
            _ => {}
        }
    }
    let last = args[start..].trim();
    if !last.is_empty() {
        result.push(last);
    }
    result
}

#[test]
fn wasm_never_passes_a_row_budget_to_k2() {
    let code = code_only(SRC);

    let mut calls = 0usize;
    let mut rest = code.as_str();
    while let Some(i) = rest.find("validate_k2_with_whitespace_roots(") {
        let after = &rest[i..];
        let end = after.find(");").expect("call is not closed") + 2;
        let block = &after[..end];
        calls += 1;

        assert!(
            !block.contains("max_file_rows"),
            "#185: a row budget was passed to K2. This silently truncates streamed files and \
             reports the truncation damage as a feed defect. Found call:\n{block}"
        );
        let args = top_level_arguments(block);
        assert!(
            args.len() >= 6,
            "#185: a K2 call carrying a whitespace-root signal must have six arguments: {block}"
        );
        assert_eq!(
            args[3], "None",
            "#185: K2's fourth, row-budget argument must be `None`. Found:\n{block}"
        );

        rest = &after[end..];
    }

    assert_eq!(
        calls, 2,
        "Expected two K2 call sites in WASM (run_full_pipeline + cached path); \
         if the count changed, extend this guard to cover the new call"
    );
}

#[test]
fn wasm_carries_custom_headers_into_jp_namespace_validation() {
    let code = code_only(SRC);

    let merge = "k2.records.gtfs_jp_namespace_headers.extend";
    let k2_call = "let mut k2 = validate_k2_with_whitespace_roots(";
    let k4_call = "check_cross_ref_with_whitespace_roots(";
    let merge_count = code.matches(merge).count();
    assert_eq!(
        merge_count, 2,
        "both WASM K2 paths must carry K1 custom headers into GTFS-JP namespace validation"
    );
    assert_eq!(
        code.matches("k1.custom_file_headers").count(),
        2,
        "each WASM K2 path must use the K1 custom-file header inventory"
    );

    let mut rest = code.as_str();
    let mut paths = 0usize;
    while let Some(start) = rest.find(k2_call) {
        let path = &rest[start..];
        let next_start = path[k2_call.len()..]
            .find(k2_call)
            .map(|offset| k2_call.len() + offset)
            .unwrap_or(path.len());
        let path = &path[..next_start];
        let merge_pos = path
            .find(merge)
            .expect("each K2 path must merge custom headers");
        let k4_pos = path.find(k4_call).expect("each K2 path must call K4");
        assert!(
            merge_pos < k4_pos,
            "GTFS-JP custom headers must reach K4 before cross-reference validation"
        );
        paths += 1;
        rest = &rest[start + next_start..];
    }
    assert_eq!(
        paths, 2,
        "both WASM K2 paths must be checked for merge ordering"
    );
}
