#!/usr/bin/env python3
"""İKİ TARAYICI AYRIŞMASIN — `measure_modalless` sinyalleri ↔ `extract_provisions` sınıfı.

🔴 Neden var (issue #81): `measure_modalless.py` *"All file and field names are
case-sensitive."* cümlesini NORMATİF sinyal sayıyordu (`SIGNALS["case-sensitivity"]`), ama
kataloğu üreten `extract_provisions.classify()` modal-güdümlüydü ve bu cümleye `""` diyordu.
Sonuç: cümle katalogda hiç yer almadı. Katalogda olmayan hüküm **adjudike edilemez ve öksüz
de sayılamaz** — yani rozetin öksüz kapısı bu boşluğu göremezdi. "147/147" bilinen sert
düzyazı hükümlerinin tamamı hakkında bir cümle değildi.

Bu betik ayrışmayı yapısal olarak kapatır: modalsiz taramanın normatif saydığı HER sinyal
için, o sinyali taşıyan örnek bir cümlenin `classify()` tarafından da normatif sayılması
gerekir. Biri "bu normatiftir" derken diğeri susuyorsa aynı boşluk geri gelmiş demektir.

⚠️ **BU KAPININ ÖLÇMEDİĞİ ŞEY:** spec sayfasındaki HER cümlenin katalogda olduğunu
kanıtlamaz — onun için sayfayı indirmek gerekir ve gtfs.org CI'dan 403 döner (bkz.
`spec_drift.py`). Kanıtladığı şey daha dar ama gerçek: iki betiğin normatiflik TANIMI
ayrışamaz. Sayfa düzeyindeki tam denetim geliştirici makinesinde
`measure_modalless.py --residual` ile yapılır.

Çalıştır: python3 spec-audit/signal_parity.py   (ağ istemez; exit 1 = ayrışma)
"""
import importlib.util
import pathlib
import sys

ROOT = pathlib.Path(__file__).resolve().parent

# 🔴 DİKKAT: `SIGNALS` NORMATİFLİK İDDİASI DEĞİLDİR — kendi docstring'i "ADAY üreticisi,
# sayaç değil" diyor. Sinyaller "bakılması gereken cümle" işaretidir. Bu kapının ilk
# taslağı her sinyali "katalogda olmalı" diye okudu ve dokuz sinyalde birden düştü;
# o tasarım `classify()`'ı koşul/emir kipi cümlelerini de hüküm saymaya zorlardı — yani
# tavsiyeyi norm sayma hatasının (`PTH_017`) defter düzeyindeki kopyası.
#
# Doğru ölçüt issue #81'in kendi cümlesidir: bir sinyal ya kataloğa GİRER ya da neden
# girmediği AÇIKÇA yazılır. İkisi de yoksa boşluk sessizdir — ve `case-sensitivity`
# tam olarak öyle kaybolmuştu.

# Kataloğa GİRMESİ gereken sinyaller: örnek cümle `classify()` tarafından "strong" sayılmalı.
SAMPLES = {
    "case-sensitivity": "All file and field names are case-sensitive.",
}

# Kataloğa GİRMEYEN sinyaller — her biri için gerekçe ZORUNLU. Gerekçe "kalıp tek başına
# normatiflik göstermez" biçiminde olmalı; "şimdilik bakmadık" bir gerekçe değildir.
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
                problems.append(f"'{name}' HEM SAMPLES hem TRIAGE_ONLY'de — karar tek olmalı.")
            continue
        sample = SAMPLES.get(name)
        if sample is None:
            problems.append(
                f"'{name}' sinyali KARARSIZ: ne kataloğa giriyor (SAMPLES) ne de neden "
                f"girmediği yazılı (TRIAGE_ONLY). Sessiz boşluk tam buradan doğar.")
            continue
        verdict = extract.classify(sample)
        if verdict != "strong":
            problems.append(
                f"'{name}': kataloğa girmesi gerekiyor ama classify() '{verdict or 'hiçbir şey'}' "
                f"diyor → cümle katalogda yer almaz, adjudike de öksüz de sayılamaz.\n"
                f"     örnek: {sample}")

    # Ters yön: karar verilmiş ama sinyal listesinden düşmüş bir ad da ayrışmadır.
    for name in list(SAMPLES) + list(TRIAGE_ONLY):
        if name not in modalless.SIGNALS:
            problems.append(f"'{name}' burada kararlı ama measure_modalless.SIGNALS'ta YOK.")

    if problems:
        print("🔴 SİNYAL AYRIŞMASI — iki tarayıcı 'normatif' tanımında uyuşmuyor:")
        for p in problems:
            print(f"  {p}")
        print("  → Ya classify()'a kalıbı ekleyin ya da sinyali modalsiz taramadan düşürün.")
        return 1

    print(f"signal_parity OK — {len(modalless.SIGNALS)} sinyalin tamamı kararlı "
          f"({len(SAMPLES)} katalogda · {len(TRIAGE_ONLY)} gerekçeli triyaj sinyali).")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
