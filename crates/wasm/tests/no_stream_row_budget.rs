//! #185 nöbeti: WASM yolu K2'ye satır bütçesi GEÇMEMELİ.
//!
//! Regresyonun kendisi bir çağrı yeriydi, bir fonksiyon değil: `run_full_pipeline` ve
//! önbellekli yol `validate_k2_with_stream_limit`'e `Some(config.max_file_rows)`
//! veriyordu. Sonuç, stream edilen dosyaların 1.000.000 satırda sessizce kesilmesi ve
//! kesmenin ürettiği boşta referansların feed'in kusuru olarak raporlanmasıydı
//! (VBB/mdb-782: 250.407 uydurma TRP_004 + 220.752 XFL_002, gerçek sayı 25.369).
//!
//! Bu davranış tarayıcıya özgü olduğu için tam korpus denetimi ONU ASLA GÖREMEZ —
//! benchmark/audit_all native CLI'ı koşar. Kapı bu yüzden burada, kaynak düzeyinde
//! kurulur: çağrı yerinin dördüncü argümanı `None` olmak zorundadır.

const SRC: &str = include_str!("../src/lib.rs");

/// Satır başı `//` yorumlarını atar; gerekçe yorumları kural adlarını içerdiği için
/// tarama yalnız gerçek koda bakmalıdır.
fn code_only(src: &str) -> String {
    src.lines()
        .filter(|l| !l.trim_start().starts_with("//"))
        .collect::<Vec<_>>()
        .join("\n")
}

#[test]
fn wasm_never_passes_a_row_budget_to_k2() {
    let code = code_only(SRC);

    let mut calls = 0usize;
    let mut rest = code.as_str();
    while let Some(i) = rest.find("validate_k2_with_stream_limit(") {
        let after = &rest[i..];
        let end = after.find(");").expect("çağrı kapanmamış") + 2;
        let block = &after[..end];
        calls += 1;

        assert!(
            !block.contains("max_file_rows"),
            "#185: K2 çağrısına satır bütçesi geçilmiş. Bu, stream edilen dosyaları \
             sessizce kesip kesmenin hasarını feed'in kusuru olarak raporlar. \
             Bulunan çağrı:\n{block}"
        );
        assert!(
            block.contains("None"),
            "#185: K2 çağrısının satır bütçesi argümanı `None` olmalı. Bulunan:\n{block}"
        );

        rest = &after[end..];
    }

    assert_eq!(
        calls, 2,
        "WASM'da iki K2 çağrı yeri bekleniyor (run_full_pipeline + önbellekli yol); \
         sayı değiştiyse bu nöbet yeni çağrıyı da kapsamalı"
    );
}
