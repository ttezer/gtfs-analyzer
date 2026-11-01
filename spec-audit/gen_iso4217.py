#!/usr/bin/env python3
"""Generate the ISO 4217 table from the OFFICIAL source (issue #82).

🔴 Why generate it instead of writing it by hand: a hard-coded list loses both WHERE it came
from and WHEN it is valid. That happened on 2026-08-07: the code checked for "three uppercase
ASCII letters", then an active-code list was added by hand, but although `CLF` and `UYW`
have minor unit **4**, the table assumed there were no values other than "0/2/3" and assigned
2 to both.

Source: SIX (the ISO 4217 registration authority) `list-one.xml` — active codes (Table A.1).
The generated file carries the source's `Pblshd` date and SHA-256; the drift check uses them.

Run:
    python3 spec-audit/gen_iso4217.py                 # download from the network and write the table
    python3 spec-audit/gen_iso4217.py --check         # CHECK ONLY (exit 1 = drift)
    python3 spec-audit/gen_iso4217.py list-one.xml    # use a local copy

⚠️ `--check` NEEDS NETWORK. If it runs in CI, preserve the distinction between "could
not measure" and "no drift"; the lesson from `spec_drift.py` applies here too.
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
            continue  # countries without a currency (e.g. ANTARCTICA) — carry no code
        minor = (entry.findtext("CcyMnrUnts") or "").strip()
        # `N.A.` → codes with undefined decimal places, such as funds and precious metals.
        units = int(minor) if minor.isdigit() else 255
        if code in seen and seen[code] != units:
            raise RuntimeError(f"{code}: aynı kod için iki farklı minor unit ({seen[code]} / {units})")
        seen[code] = units
    return published, sorted(seen.items())


def render(published: str, digest: str, rows: list[tuple[str, int]]) -> str:
    body = "\n".join(f'    ("{c}", {u}),' for c, u in rows)
    return f'''//! GENERATED FILE — do not edit by hand (issue #82).
//!
//! Source : {SRC}
//! Published: {published}   (the source's own `Pblshd` field)
//! SHA-256 : {digest}
//! Generation: `python3 spec-audit/gen_iso4217.py`
//! Drift   : `python3 spec-audit/gen_iso4217.py --check`
//!
//! The table contains ISO 4217 **active** codes (Table A.1). Historical codes are NOT
//! HERE: a feed must report a currency that exists today.
//!
//! ⚠️ `255` = decimal places UNDEFINED (the source says `N.A.`) — funds and precious metals
//! (`XAU`, `XDR`, …). These are valid CODES, but amount decimals cannot be checked.

/// (code, minor unit) — SORTED by code; `binary_search_by_key` relies on this.
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
