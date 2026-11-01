#!/usr/bin/env python3
"""KEEP THE TWO SCANNERS IN SYNC — `measure_modalless` signals ↔ `extract_provisions` classification.

🔴 Why this exists (issue #81): `measure_modalless.py` treated *"All file and field names are
case-sensitive."* cümlesini NORMATİF sinyal sayıyordu (`SIGNALS["case-sensitivity"]`), ama
kataloğu üreten `extract_provisions.classify()` modal-güdümlüydü ve bu cümleye `""` diyordu.
Sonuç: cümle katalogda hiç yer almadı. Katalogda olmayan hüküm **adjudike edilemez ve öksüz
de sayılamaz** — yani rozetin öksüz kapısı bu boşluğu göremezdi. "147/147" bilinen sert
düzyazı hükümlerinin tamamı hakkında bir cümle değildi.

This script closes the divergence structurally: for EVERY signal the modal-less scan considers normative,
için, o sinyali taşıyan örnek bir cümlenin `classify()` tarafından da normatif sayılması
gerekir. Biri "bu normatiftir" derken diğeri susuyorsa aynı boşluk geri gelmiş demektir.

⚠️ **WHAT THIS GATE DOES NOT MEASURE:** it does not prove that EVERY sentence on the spec page
kanıtlamaz — onun için sayfayı indirmek gerekir ve gtfs.org CI'dan 403 döner (bkz.
`spec_drift.py`). Kanıtladığı şey daha dar ama gerçek: iki betiğin normatiflik TANIMI
ayrışamaz. Sayfa düzeyindeki tam denetim geliştirici makinesinde
`measure_modalless.py --residual` ile yapılır.

Run: python3 spec-audit/signal_parity.py   (no network; exit 1 = divergence)
"""
import importlib.util
import pathlib
import sys

ROOT = pathlib.Path(__file__).resolve().parent

# 🔴 NOTE: `SIGNALS` is NOT A CLAIM OF NORMATIVITY — its docstring says "candidate generator,
# not a counter". Signals mark a "sentence to review". The first draft of this gate read every
# signal as "must be in the catalogue" and failed on all nine; that design would have forced
# `classify()` to count conditional/imperative sentences as provisions — a ledger-level copy of
# the mistake of treating advice as a norm (`PTH_017`).
#
# The correct criterion is issue #81's own sentence: a signal either ENTERS the catalogue or its
# reason for exclusion is written CLEARLY. If neither exists, the gap is silent — exactly how
# `case-sensitivity` was lost.

# Signals that MUST ENTER the catalogue: `classify()` must consider their sample sentence "strong".
SAMPLES = {
    "case-sensitivity": "All file and field names are case-sensitive.",
}

# Signals that do NOT ENTER the catalogue — each requires a reason. The reason must say that
# "the pattern alone does not show normativity"; "we have not looked yet" is not a reason.
TRIAGE_ONLY = {
    "kısıt (only/just)":
        "'only' çoğunlukla BETİMLEYİCİDİR ('only available in…'). 2026-08-06'da bu kalıbı "
        "taşıyan cümleler tek tek okundu; normatif olanlar zaten bir modal de taşıyordu.",
    "sayısal sınır":
        "Sayısal sınırlar alan tablosunun ENUM/aralık ekseninde ölçülür (300 atom), "
        "düzyazı paydasında değil — iki eksen ÖRTÜŞÜR, toplanmaz.",
    "mutlak (never/always)":
        "'always'/'never' spec'te tüketici davranışını anlatan cümlelerde geçiyor; "
        "doğrulayıcıyı bağlayan hükümlerde modal var.",
    "öncelik/geçersiz kılma":
        "Öncelik cümleleri (`takes precedence`) davranış TANIMIDIR; ihlal edilebilir bir "
        "yükümlülük değil, çakışma çözüm kuralıdır.",
    "olumsuz durum":
        "`is not` çok geniş: tanım cümlelerinin çoğunda geçer. Yasak bildiren biçimleri "
        "(`is not allowed`, `is prohibited`) `STRONG` zaten yakalıyor.",
    "uygulanma":
        "`applies to` kapsam belirtir, yükümlülük değil.",
    "eşitlik/özdeşlik":
        "`same as` alan anlambilimini tanımlar; ihlali alan tablosu ekseninde ölçülür.",
    "emir kipi":
        "Emir kipi cümlelerinin (`Use`, `Refer`) hedefi çoğunlukla ÜRETİCİ ya da OKUYUCUDUR; "
        "feed'den doğrulanabilir olanları modal taşıyor.",
    "koşul (if/when/unless)":
        "Koşul CÜMLE BAŞIDIR, hüküm değil: koşulun ardından gelen yüklem modal taşır ve "
        "hüküm o cümleyle katalogda zaten vardır.",
}


def load(name: str):
    spec = importlib.util.spec_from_file_location(name, ROOT / f"{name}.py")
    mod = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(mod)
    return mod


def main() -> int:
    modalless = load("measure_modalless")
    extract = load("extract_provisions")

    problems: list[str] = []
    for name in modalless.SIGNALS:
        if name in TRIAGE_ONLY:
            if name in SAMPLES:
                problems.append(f"'{name}' is in both SAMPLES and TRIAGE_ONLY — choose one.")
            continue
        sample = SAMPLES.get(name)
        if sample is None:
            problems.append(
                f"'{name}' signal is undecided: it is neither in the catalogue (SAMPLES) nor "
                f"documented as excluded (TRIAGE_ONLY). This is where a silent gap arises.")
            continue
        verdict = extract.classify(sample)
        if verdict != "strong":
            problems.append(
                f"'{name}': should enter the catalogue but classify() says '{verdict or 'nothing'}' "
                f"→ the sentence is absent from the catalogue and cannot be adjudicated or counted as orphaned.\n"
                f"     example: {sample}")

    # Reverse direction: a decided name missing from the signal list is also a divergence.
    for name in list(SAMPLES) + list(TRIAGE_ONLY):
        if name not in modalless.SIGNALS:
            problems.append(f"'{name}' is stable here but missing from measure_modalless.SIGNALS.")

    if problems:
        print("🔴 SIGNAL DIVERGENCE — the two scanners disagree on the 'normative' definition:")
        for p in problems:
            print(f"  {p}")
        print("  → Add the pattern to classify() or remove the signal from the modal-less scan.")
        return 1

    print(f"signal_parity OK — all {len(modalless.SIGNALS)} signals are stable "
          f"({len(SAMPLES)} in the catalogue · {len(TRIAGE_ONLY)} justified triage signals).")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
