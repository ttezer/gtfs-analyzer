//! Native/WASM parite nöbeti: feed-level toplulama.
//!
//! 22 kuralın feed-level toplulaması yalnız native `validate_bytes`'a eklenmiş, WASM
//! orkestratörleri atlamıştı. Registry aynı kuralları `Feed` dedup seviyesine aldığı
//! için tarayıcı ve npm SDK ham satır notice'larından birini gösteriyor, etkilenen
//! kayıt sayısını kaybediyordu. Native CLI koşan korpus denetimi bunu GÖREMEZ; kapı
//! bu yüzden kaynak düzeyinde kurulur: her orkestratör toplulamayı rapor kapsamından
//! (ve WASM'da cap'ten) ÖNCE çağırmak zorundadır.

const WASM_SRC: &str = include_str!("../src/lib.rs");
const NATIVE_SRC: &str = include_str!("../../pipeline/src/lib.rs");

/// Satır başı `//` yorumlarını atar; gerekçe yorumları fonksiyon adlarını içerir.
fn code_only(src: &str) -> String {
    src.lines()
        .filter(|l| !l.trim_start().starts_with("//"))
        .collect::<Vec<_>>()
        .join("\n")
}

/// `fn <name>(` ile başlayan gövdeyi bir sonraki üst düzey `fn` başlangıcına kadar döner.
fn body_of<'a>(src: &'a str, name: &str) -> &'a str {
    let start = src
        .find(&format!("fn {name}("))
        .unwrap_or_else(|| panic!("`{name}` bulunamadı"));
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
        .unwrap_or_else(|| panic!("`{name}` feed-level toplulamayı çağırmıyor"));
    for call in later {
        let pos = body
            .find(call)
            .unwrap_or_else(|| panic!("`{name}` içinde `{call}` yok"));
        assert!(agg < pos, "`{name}`: toplulama `{call}`'dan ÖNCE çağrılmalı");
    }
}

#[test]
fn wasm_full_pipeline_aggregates_before_scope_and_cap() {
    let src = code_only(WASM_SRC);
    assert_aggregates_before(
        body_of(&src, "run_full_pipeline"),
        "run_full_pipeline",
        &["apply_report_scope(", "cap_per_rule("],
    );
}

#[test]
fn wasm_cached_rerun_aggregates_before_scope_and_cap() {
    let src = code_only(WASM_SRC);
    assert_aggregates_before(
        body_of(&src, "rerun_k6_k7_inner"),
        "rerun_k6_k7_inner",
        &["apply_report_scope(", "cap_per_rule("],
    );
}

#[test]
fn native_validate_bytes_aggregates_before_scope() {
    let src = code_only(NATIVE_SRC);
    assert_aggregates_before(
        body_of(&src, "validate_bytes"),
        "validate_bytes",
        &["apply_report_scope("],
    );
}
