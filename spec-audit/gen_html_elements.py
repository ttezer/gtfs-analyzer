#!/usr/bin/env python3
"""Generate HTML element names from the WHATWG index (issue #79).

🔴 Why: the `ARC_032` tag list was hand-written and claimed to be "ALL of HTML5".
Audit 6 showed that `svg` and `math` were missing — the claim was too broad.
Keeping the tag list hand-written while generating the entity table was inconsistent.

⚠️ **THE INDEX ALONE IS NOT ENOUGH**, and generation cannot hide that fact:

1. `svg` and `math` are **NOT** in the HTML element index — they belong to the SVG/MathML
   specs. In the HTML parser they are recognized start tags that enter foreign-content mode.
   Since the rule says "HTML tags", they cannot be excluded.
2. The index lists only the living standard; **OBSOLETE** elements such as `font`, `center`,
   `marquee`, and `applet` are absent but occur in real feeds (`<BR>` and `<font>` were
   measured in the corpus). Dropping them would lose findings.

Therefore the output is **the generated index ∪ two EXPLICITLY justified extra sets**.
The extra sets live here, not in `k1_parse.rs`, so the "complete set" claim and its
justification are visible in one place.

Run:
    python3 spec-audit/gen_html_elements.py             # generate from the network
    python3 spec-audit/gen_html_elements.py --check     # check (1 = drift, 2 = unavailable)
    python3 spec-audit/gen_html_elements.py indices.html
"""
import hashlib
import pathlib
import re
import subprocess
import sys

SRC = "https://html.spec.whatwg.org/multipage/indices.html"
OUT = pathlib.Path(__file__).resolve().parent.parent / "crates/pipeline/src/k1_html_elements.rs"

# Foreign-content roots in the HTML parser — absent from the index because they are not
# HTML elements, but `<svg>`/`<math>` are recognized start tags in HTML.
FOREIGN_ROOTS = ["math", "svg"]

# Elements removed from the living standard but found in real data. Corpus measurement:
# `<BR>` occurred in 1,176 stop names in one feed, and `<font>` occurred in other feeds.
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
        raise RuntimeError(f"curl failed: {p.stderr.decode()[:120]}")
    return p.stdout


def parse(raw: bytes) -> list[str]:
    doc = raw.decode("utf-8", "replace")
    try:
        start = doc.index("<h3 id=elements-3")
        end = doc.index("<h3 id=element-content-categories", start)
    except ValueError as e:
        raise RuntimeError(f"element index section not found: {e}") from e
    seg = doc[start:end]
    names: set[str] = set()
    # Each row's HEADER cell (up to the first `<td>`) can contain one or more elements:
    # `h1`–`h6` are listed as six names on one line. The first draft read one name per row
    # and dropped `h2..h6`, which would make the generated set NARROWER than the hand list.
    for row in seg.split("<tr>"):
        head = row.split("<td>", 1)[0]
        names.update(re.findall(r"<code id=elements-3:[^>]*><a href=[^>]*>([a-z0-9]+)</a></code>", head))
    if len(names) < 90:
        raise RuntimeError(f"found only {len(names)} elements; the page layout may have changed")
    return sorted(names)


def render(digest: str, indexed: list[str]) -> str:
    all_names = sorted(set(indexed) | set(FOREIGN_ROOTS) | set(OBSOLETE))
    body = "\n".join(f'    "{n}",' for n in all_names)
    return f'''//! GENERATED FILE — do not edit by hand (issue #79).
//!
//! Source : {SRC}
//! SHA-256: {digest}
//! Generate: `python3 spec-audit/gen_html_elements.py`
//! Drift   : `python3 spec-audit/gen_html_elements.py --check`
//!
//! Set = WHATWG element index ({len(indexed)}) ∪ foreign-content roots
//! ({len(FOREIGN_ROOTS)}: {", ".join(FOREIGN_ROOTS)}) ∪ obsolete elements ({len(OBSOLETE)}).
//! {len(all_names)} names total. The generator docstring gives the REASON for both extra sets:
//! `svg`/`math` are absent from the index but are recognized start tags in HTML; obsolete
//! elements were removed from the living standard but occur in real feeds.
//!
//! ⚠️ The list MUST remain closed: treating every word inside angle brackets as a tag would
//! flag real text such as `<TBD>`, `<Bilinmiyor>`, and `<未定>`.

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
        print(f"NOT MEASURED: {type(e).__name__}: {e}", file=sys.stderr)
        print("⚠️ This does NOT mean 'no drift' — the source could not be read.", file=sys.stderr)
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
    print(f"wrote: {OUT.name} ({len(want)} names · index {len(indexed)} + {len(want)-len(indexed)} extra)")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
