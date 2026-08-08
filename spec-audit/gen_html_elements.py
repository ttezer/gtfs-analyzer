#!/usr/bin/env python3
"""HTML eleman adlarını WHATWG dizininden üret (issue #79).

🔴 Neden: `ARC_032`'nin etiket listesi ELLE yazılmıştı ve "HTML5'in TAMAMI" diye
iddia ediliyordu. 6. denetim `svg` ve `math`in eksik olduğunu gösterdi — iddia fazlaydı.
Entity tablosu kaynaktan üretilirken etiket listesinin elle kalması bir tutarsızlıktı.

⚠️ **DİZİN TEK BAŞINA YETMİYOR** ve bu, üretimin gizleyemeyeceği bir gerçek:

1. `svg` ve `math` HTML eleman dizininde **YOKTUR** — onlar SVG/MathML spec'lerinin
   elemanlarıdır. HTML ayrıştırıcısında ise TANINAN başlangıç etiketleridir ve yabancı
   içerik kipine geçerler. Hüküm "HTML tags" dediğine göre kapsam dışı bırakılamazlar.
2. Dizin YALNIZ yaşayan standardı listeler; `font`, `center`, `marquee`, `applet` gibi
   ESKİMİŞ elemanlar orada yoktur ama gerçek feed'lerde geçer (korpusta `<BR>` ve `<font>`
   ölçüldü). Onları düşürmek bulgu KAYBETMEK olurdu.

Bu yüzden çıktı: **üretilen dizin ∪ iki AÇIKÇA gerekçeli ek küme**. Ek kümeler burada
durur, `k1_parse.rs`'te değil — böylece "tam küme" iddiası tek yerde ve gerekçesiyle
okunur.

Çalıştır:
    python3 spec-audit/gen_html_elements.py             # ağdan üret
    python3 spec-audit/gen_html_elements.py --check     # denetle (1 = drift, 2 = bakılamadı)
    python3 spec-audit/gen_html_elements.py indices.html
"""
import hashlib
import pathlib
import re
import subprocess
import sys

SRC = "https://html.spec.whatwg.org/multipage/indices.html"
OUT = pathlib.Path(__file__).resolve().parent.parent / "crates/pipeline/src/k1_html_elements.rs"

# HTML ayrıştırıcısının yabancı içerik kökleri — dizinde yoklar, çünkü HTML elemanı
# değiller; ama `<svg>`/`<math>` HTML'de TANINAN başlangıç etiketleridir.
FOREIGN_ROOTS = ["math", "svg"]

# Yaşayan standarttan düşmüş ama gerçek veride geçen elemanlar. Korpus ölçümü: bir feed'in
# 1176 durak adında `<BR>`, başka feed'lerde `<font>` bulundu.
OBSOLETE = [
    "acronym", "applet", "basefont", "big", "blink", "center", "dir", "font",
    "frame", "frameset", "marquee", "noframes", "param", "strike", "tt",
]


def fetch(argv: list[str]) -> bytes:
    local = [a for a in argv[1:] if not a.startswith("--")]
    if local:
        return pathlib.Path(local[0]).read_bytes()
    p = subprocess.run(["curl", "-sSL", "--max-time", "120", SRC], capture_output=True)
    if p.returncode != 0:
        raise RuntimeError(f"curl hatası: {p.stderr.decode()[:120]}")
    return p.stdout


def parse(raw: bytes) -> list[str]:
    doc = raw.decode("utf-8", "replace")
    try:
        start = doc.index("<h3 id=elements-3")
        end = doc.index("<h3 id=element-content-categories", start)
    except ValueError as e:
        raise RuntimeError(f"eleman dizini bölümü bulunamadı: {e}") from e
    seg = doc[start:end]
    names: set[str] = set()
    # Her satırın BAŞLIK hücresi (ilk `<td>`ye kadar) bir ya da DAHA FAZLA eleman taşır:
    # `h1`–`h6` tek satırda altı ad olarak listelenir. İlk taslak satır başına tek ad
    # okuyordu ve `h2..h6` düşüyordu — üretim, elle listeden daha DAR olacaktı.
    for row in seg.split("<tr>"):
        head = row.split("<td>", 1)[0]
        names.update(re.findall(r"<code id=elements-3:[^>]*><a href=[^>]*>([a-z0-9]+)</a></code>", head))
    if len(names) < 90:
        raise RuntimeError(f"yalnız {len(names)} eleman bulundu; sayfa düzeni değişmiş olabilir")
    return sorted(names)


def render(digest: str, indexed: list[str]) -> str:
    all_names = sorted(set(indexed) | set(FOREIGN_ROOTS) | set(OBSOLETE))
    body = "\n".join(f'    "{n}",' for n in all_names)
    return f'''//! ÜRETİLMİŞ DOSYA — elle düzenlemeyin (issue #79).
//!
//! Kaynak : {SRC}
//! SHA-256: {digest}
//! Üretim : `python3 spec-audit/gen_html_elements.py`
//! Drift  : `python3 spec-audit/gen_html_elements.py --check`
//!
//! Küme = WHATWG eleman dizini ({len(indexed)}) ∪ yabancı içerik kökleri
//! ({len(FOREIGN_ROOTS)}: {", ".join(FOREIGN_ROOTS)}) ∪ eskimiş elemanlar ({len(OBSOLETE)}).
//! Toplam {len(all_names)} ad. İki ek kümenin GEREKÇESİ üreticinin docstring'indedir:
//! `svg`/`math` dizinde yoktur ama HTML'de tanınan başlangıç etiketleridir; eskimiş
//! elemanlar yaşayan standarttan düşmüştür ama gerçek feed'lerde geçer.
//!
//! ⚠️ Liste KAPALI olmak zorunda: açılı ayraç içindeki her kelimeyi etiket saymak
//! `<TBD>`, `<Bilinmiyor>`, `<未定>` gibi gerçek metni işaretlerdi.

/// Alfabetik SIRALI — `binary_search` bunu varsayar.
pub(crate) const HTML_ELEMENTS: &[&str] = &[
{body}
];
'''


def main() -> int:
    check = "--check" in sys.argv
    try:
        raw = fetch(sys.argv)
        indexed = parse(raw)
    except Exception as e:
        print(f"ÖLÇÜLEMEDİ: {type(e).__name__}: {e}", file=sys.stderr)
        print("⚠️ Bu 'drift yok' DEĞİLDİR — kaynağa bakılamadı.", file=sys.stderr)
        return 2
    text = render(hashlib.sha256(raw).hexdigest(), indexed)
    want = set(indexed) | set(FOREIGN_ROOTS) | set(OBSOLETE)
    if check:
        cur = OUT.read_text(encoding="utf-8") if OUT.exists() else ""
        cur_names = set(re.findall(r'^    "([a-z0-9]+)",$', cur, re.M))
        if cur_names == want:
            print(f"html_elements OK — {len(want)} ad (dizin {len(indexed)} + ek {len(want)-len(indexed)})")
            return 0
        added, gone = sorted(want - cur_names), sorted(cur_names - want)
        print(f"🔴 HTML eleman DRIFT — eklenen {added[:8]} · kaybolan {gone[:8]}")
        print("  → python3 spec-audit/gen_html_elements.py")
        return 1
    OUT.write_text(text, encoding="utf-8")
    print(f"yazıldı: {OUT.name} ({len(want)} ad · dizin {len(indexed)} + ek {len(want)-len(indexed)})")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
