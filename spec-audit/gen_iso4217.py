#!/usr/bin/env python3
"""ISO 4217 tablosunu RESMÎ kaynaktan üret (issue #82).

🔴 Neden üretiliyor, elle yazılmıyor: elle gömülü liste iki şeyi birden kaybeder —
NEREDEN geldiğini ve NE ZAMAN geçerli olduğunu. 2026-08-07'de tam bu oldu: kodda "üç büyük
ASCII harf" denetimi vardı, sonra elle bir aktif-kod listesi eklendi, ama `CLF` ve `UYW`
için minor unit **4** olduğu halde tablo "0/2/3 dışında değer yok" varsayıyordu ve ikisine
de 2 veriyordu.

Kaynak: SIX (ISO 4217 kayıt otoritesi) `list-one.xml` — aktif kodlar (Table A.1).
Üretilen dosya kaynağın `Pblshd` tarihini ve SHA-256'sını taşır; drift denetimi bunlara
bakar.

Çalıştır:
    python3 spec-audit/gen_iso4217.py                 # ağdan indirir, tabloyu yazar
    python3 spec-audit/gen_iso4217.py --check         # SADECE denetler (exit 1 = drift)
    python3 spec-audit/gen_iso4217.py list-one.xml    # yerel kopyadan

⚠️ `--check` AĞ İSTER. CI'da koşacaksa "ölçülemedi" ile "drift yok" ayrımı korunmalı;
`spec_drift.py`'nin dersi burada da geçerli.
"""
import hashlib
import pathlib
import re
import subprocess
import sys
import xml.etree.ElementTree as ET

SRC = "https://www.six-group.com/dam/download/financial-information/data-center/iso-currrency/lists/list-one.xml"
OUT = pathlib.Path(__file__).resolve().parent.parent / "crates/pipeline/src/k2/iso4217_generated.rs"


def fetch(argv: list[str]) -> bytes:
    local = [a for a in argv[1:] if not a.startswith("--")]
    if local:
        return pathlib.Path(local[0]).read_bytes()
    p = subprocess.run(["curl", "-sSL", "--max-time", "90", SRC], capture_output=True)
    if p.returncode != 0:
        raise RuntimeError(f"curl hatası: {p.stderr.decode()[:120]}")
    return p.stdout


def parse(raw: bytes) -> tuple[str, list[tuple[str, int]]]:
    root = ET.fromstring(raw)
    published = root.attrib.get("Pblshd", "?")
    seen: dict[str, int] = {}
    for entry in root.iter("CcyNtry"):
        code = (entry.findtext("Ccy") or "").strip()
        if not code:
            continue  # para birimi olmayan ülkeler (ör. ANTARCTICA) — kod taşımaz
        minor = (entry.findtext("CcyMnrUnts") or "").strip()
        # `N.A.` → fon/kıymetli maden gibi ondalığı tanımsız kodlar.
        units = int(minor) if minor.isdigit() else 255
        if code in seen and seen[code] != units:
            raise RuntimeError(f"{code}: aynı kod için iki farklı minor unit ({seen[code]} / {units})")
        seen[code] = units
    return published, sorted(seen.items())


def render(published: str, digest: str, rows: list[tuple[str, int]]) -> str:
    body = "\n".join(f'    ("{c}", {u}),' for c, u in rows)
    return f'''//! ÜRETİLMİŞ DOSYA — elle düzenlemeyin (issue #82).
//!
//! Kaynak : {SRC}
//! Yayın   : {published}   (kaynağın kendi `Pblshd` alanı)
//! SHA-256 : {digest}
//! Üretim  : `python3 spec-audit/gen_iso4217.py`
//! Drift   : `python3 spec-audit/gen_iso4217.py --check`
//!
//! Tablo ISO 4217'nin **aktif** kodlarıdır (Table A.1). Tarihe karışmış kodlar burada
//! YOKTUR: bir feed bugünkü bir para birimi bildirmelidir.
//!
//! ⚠️ `255` = ondalık basamak TANIMSIZ (kaynakta `N.A.`) — fonlar ve kıymetli madenler
//! (`XAU`, `XDR`, …). Bunlar geçerli KODDUR ama tutar ondalığı denetlenemez.

/// (kod, minor unit) — koda göre SIRALI; `binary_search_by_key` bunu varsayar.
pub(crate) const ISO4217: &[(&str, u8)] = &[
{body}
];
'''


def main() -> int:
    check = "--check" in sys.argv
    try:
        raw = fetch(sys.argv)
        published, rows = parse(raw)
    except Exception as e:
        print(f"ÖLÇÜLEMEDİ: {type(e).__name__}: {e}", file=sys.stderr)
        print("⚠️ Bu 'drift yok' DEĞİLDİR — kaynağa bakılamadı.", file=sys.stderr)
        return 2
    digest = hashlib.sha256(raw).hexdigest()
    text = render(published, digest, rows)

    if check:
        cur = OUT.read_text(encoding="utf-8") if OUT.exists() else ""
        cur_codes = set(re.findall(r'\("([A-Z]{3})", (\d+)\)', cur))
        new_codes = set(re.findall(r'\("([A-Z]{3})", (\d+)\)', text))
        if cur_codes == new_codes:
            print(f"iso4217 OK — {len(rows)} kod, kaynak yayını {published}")
            return 0
        added, gone = sorted(new_codes - cur_codes), sorted(cur_codes - new_codes)
        print(f"🔴 ISO 4217 DRIFT — kaynak yayını {published}")
        if added:
            print(f"  eklenen/değişen: {added[:12]}")
        if gone:
            print(f"  kaybolan/eski  : {gone[:12]}")
        print("  → python3 spec-audit/gen_iso4217.py ile tabloyu yenileyin.")
        return 1

    OUT.write_text(text, encoding="utf-8")
    print(f"yazıldı: {OUT.name} ({len(rows)} kod · yayın {published})")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
