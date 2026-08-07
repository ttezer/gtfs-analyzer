#!/usr/bin/env python3
"""HTML5 adlandırılmış karakter referansı tablosunu RESMÎ kaynaktan üret (issue #79).

🔴 Neden: `ARC_032` altı entity adı tanıyordu (`nbsp amp lt gt quot apos`). `&copy;`,
`&reg;`, `&euro;`, `&mdash;` kaçıyordu — yani kural, dayandığı hükmün ("HTML tags,
comments or escape sequences") dediğinden dardı.

⚠️ **Listeyi "rastgele büyütmek" ÇÖZÜM DEĞİL.** `&` + kelime + `;` desenini serbest
bırakmak gerçek metni yakalar: korpusta ölçülmüş örnek `K.A.Wheel&Tire;`. Bu yüzden liste
KAPALI kalır ama artık standardın ALT KÜMESİ değil, standardın KENDİSİDİR — kaynağı,
tarihi ve özeti üretilen dosyada durur.

Kaynak: WHATWG HTML Standard `entities.json` (adlandırılmış karakter referanslarının
normatif listesi).

Çalıştır:
    python3 spec-audit/gen_html_entities.py            # ağdan üret
    python3 spec-audit/gen_html_entities.py --check    # denetle (exit 1 = drift, 2 = bakılamadı)
    python3 spec-audit/gen_html_entities.py entities.json
"""
import hashlib
import json
import pathlib
import re
import subprocess
import sys

SRC = "https://html.spec.whatwg.org/entities.json"
OUT = pathlib.Path(__file__).resolve().parent.parent / "crates/pipeline/src/k1_html_entities.rs"


def fetch(argv: list[str]) -> bytes:
    local = [a for a in argv[1:] if not a.startswith("--")]
    if local:
        return pathlib.Path(local[0]).read_bytes()
    p = subprocess.run(["curl", "-sSL", "--max-time", "90", SRC], capture_output=True)
    if p.returncode != 0:
        raise RuntimeError(f"curl hatası: {p.stderr.decode()[:120]}")
    return p.stdout


def names(raw: bytes) -> tuple[list[str], list[str]]:
    """(tüm adlar, noktalı virgülsüz de geçerli olan ESKİ adlar).

    HTML5 bazı adlandırılmış referansları noktalı virgül OLMADAN da tanır (`&copy`).
    Bunlar kaynakta `&name` anahtarıyla durur ve AYRI tabloya girer: noktalı virgülsüz
    biçimi TÜM adlar için kabul etmek yanlış olurdu.
    """
    data = json.loads(raw)
    all_names = {k[1:-1] if k.endswith(";") else k[1:] for k in data}
    legacy = {k[1:] for k in data if not k.endswith(";")}
    bad = [n for n in all_names if not re.fullmatch(r"[A-Za-z][A-Za-z0-9]*", n)]
    if bad:
        raise RuntimeError(f"beklenmeyen entity adı biçimi: {bad[:5]}")
    return sorted(all_names), sorted(legacy)


def render(digest: str, ns: list[str], legacy: list[str]) -> str:
    body = "\n".join(f'    "{n}",' for n in ns)
    legacy_body = "\n".join(f'    "{n}",' for n in legacy)
    return f'''//! ÜRETİLMİŞ DOSYA — elle düzenlemeyin (issue #79).
//!
//! Kaynak : {SRC}
//! SHA-256: {digest}
//! Üretim : `python3 spec-audit/gen_html_entities.py`
//! Drift  : `python3 spec-audit/gen_html_entities.py --check`
//!
//! HTML5'in adlandırılmış karakter referanslarının TAMAMI ({len(ns)} ad). `ARC_032` bir
//! değerde `&<ad>;` görürse kaçış dizisi sayar.
//!
//! ⚠️ Liste KAPALI olmak ZORUNDA: `&` + kelime + `;` desenini serbest bırakmak gerçek
//! metni yakalar — korpusta ölçülmüş örnek `K.A.Wheel&Tire;`. `Tire` bu listede yoktur.

/// Alfabetik SIRALI — `binary_search` bunu varsayar.
pub(crate) const HTML5_ENTITIES: &[&str] = &[
{body}
];

/// Noktalı virgül OLMADAN da geçerli olan ESKİ adlar ({len(legacy)} ad) — `&copy` gibi.
/// ⚠️ Bu ayrıcalık TÜM adlar için geçerli DEĞİLDİR; kaynak hangilerinin öyle olduğunu
/// kendisi söyler ve tablo ondan üretilir.
pub(crate) const HTML5_ENTITIES_NO_SEMI: &[&str] = &[
{legacy_body}
];
'''


def main() -> int:
    check = "--check" in sys.argv
    try:
        raw = fetch(sys.argv)
        ns, legacy = names(raw)
    except Exception as e:
        print(f"ÖLÇÜLEMEDİ: {type(e).__name__}: {e}", file=sys.stderr)
        print("⚠️ Bu 'drift yok' DEĞİLDİR — kaynağa bakılamadı.", file=sys.stderr)
        return 2
    text = render(hashlib.sha256(raw).hexdigest(), ns, legacy)
    if check:
        cur = OUT.read_text(encoding="utf-8") if OUT.exists() else ""
        # ⚠️ Dosya İKİ tablo taşır; karşılaştırma ikisinin BİRLEŞİMİNE yapılır. İlk
        # sürüm yalnız `ns` ile karşılaştırıyordu ve eski-ad tablosu eklenince kapı
        # uydurma bir "drift" bildirirdi.
        cur_names = set(re.findall(r'^    "([A-Za-z0-9]+)",$', cur, re.M))
        want = set(ns) | set(legacy)
        if cur_names == want:
            print(f"html_entities OK — {len(ns)} ad ({len(legacy)} tanesi noktalı virgülsüz de geçerli)")
            return 0
        added, gone = sorted(want - cur_names), sorted(cur_names - want)
        print(f"🔴 HTML entity DRIFT — eklenen {len(added)}, kaybolan {len(gone)}")
        print(f"  {added[:8]} / {gone[:8]}")
        print("  → python3 spec-audit/gen_html_entities.py")
        return 1
    OUT.write_text(text, encoding="utf-8")
    print(f"yazıldı: {OUT.name} ({len(ns)} ad)")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
