#!/usr/bin/env python3
"""Generate the HTML5 named character reference table from the OFFICIAL source (issue #79).

🔴 Why: `ARC_032` recognized six entity names (`nbsp amp lt gt quot apos`). `&copy;`,
`&reg;`, `&euro;`, and `&mdash;` escaped it — the rule was narrower than the provision
it implements ("HTML tags, comments or escape sequences").

⚠️ **"Randomly expanding the list" IS NOT A SOLUTION.** Allowing the `&` + word + `;`
pattern to match freely catches real text: the corpus contains `K.A.Wheel&Tire;`.
The list therefore remains CLOSED, but it is now the standard ITSELF rather than a
SUBSET — its source, digest, and summary are recorded in the generated file.

Source: WHATWG HTML Standard `entities.json` (the normative list of named character
references).

Run:
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
        raise RuntimeError(f"curl failed: {p.stderr.decode()[:120]}")
    return p.stdout


def names(raw: bytes) -> tuple[list[str], list[str]]:
    """(all names, legacy names that are also valid without a semicolon).

    HTML5 recognizes some named references even WITHOUT a semicolon (`&copy`).
    They appear under `&name` keys in the source and go into a SEPARATE table:
    accepting the no-semicolon form for ALL names would be wrong.
    """
    data = json.loads(raw)
    all_names = {k[1:-1] if k.endswith(";") else k[1:] for k in data}
    legacy = {k[1:] for k in data if not k.endswith(";")}
    bad = [n for n in all_names if not re.fullmatch(r"[A-Za-z][A-Za-z0-9]*", n)]
    if bad:
        raise RuntimeError(f"unexpected entity name format: {bad[:5]}")
    return sorted(all_names), sorted(legacy)


def render(digest: str, ns: list[str], legacy: list[str]) -> str:
    body = "\n".join(f'    "{n}",' for n in ns)
    legacy_body = "\n".join(f'    "{n}",' for n in legacy)
    return f'''//! GENERATED FILE — do not edit by hand (issue #79).
//!
//! Source : {SRC}
//! SHA-256: {digest}
//! Generate: `python3 spec-audit/gen_html_entities.py`
//! Drift   : `python3 spec-audit/gen_html_entities.py --check`
//!
//! ALL HTML5 named character references ({len(ns)} names). `ARC_032` treats `&<ad>;`
//! in a value as an escape sequence.
//!
//! ⚠️ The list MUST remain CLOSED: allowing the `&` + word + `;` pattern freely catches
//! real text — the corpus contains `K.A.Wheel&Tire;`. `Tire` is not in this list.

/// Alphabetically SORTED — `binary_search` relies on this.
pub(crate) const HTML5_ENTITIES: &[&str] = &[
{body}
];

/// LEGACY names also valid WITHOUT a semicolon ({len(legacy)} names), such as `&copy`.
/// ⚠️ This exception does NOT apply to ALL names; the source identifies which ones qualify,
/// and the table is generated from that source.
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
        print(f"NOT MEASURED: {type(e).__name__}: {e}", file=sys.stderr)
        print("⚠️ This does NOT mean 'no drift' — the source could not be read.", file=sys.stderr)
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
    print(f"wrote: {OUT.name} ({len(ns)} names)")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
