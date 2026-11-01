#!/usr/bin/env python3
"""Extracts a catalog of NORMATIVE SENTENCES from the GTFS Schedule Reference (T5 anchor).

Why this is a separate script: `extract_fields.py` extracts the spec's FIELD TABLE,
which is the source of today's 302 provision atoms. But the spec's normative content
does not end in the table — the entire `file-requirements` section is prose ("All files
must be saved as comma-delimited text"), and Description cells carry conditions that
the presence label cannot express. Those prose provisions were completely absent from
the measurement; this script makes them countable.

Run:
    python3 spec-audit/extract_provisions.py            # download from the network
    python3 spec-audit/extract_provisions.py spec.html  # use a local copy

Python's urllib raises `CERTIFICATE_VERIFY_FAILED` in installations without a certificate
store (`extract_fields.py` has the same issue). In that case, download first and use:
    curl -sSo spec.html https://gtfs.org/documentation/schedule/reference/

⚠️ THIS SCRIPT PRODUCES **CANDIDATES**, NOT PROVISIONS. Sentence splitting is coarse: one
sentence may carry multiple provisions ("The files must reside at the root level directly, not in a
subfolder" says two things), and some `must` statements are descriptive (sentences such as
"Consuming software must be able to…" describe the consumer side and are not verifiable by us).
Triage is a human task; the script only makes the denominator visible.

⚠️ The HARD/SOFT distinction is the catalog's most important field. The `PTH_017` case (2026-08-03)
imposed a `should` as a norm and could have blocked a valid feed from release; `ARC_031` was the
opposite case. Therefore `strength` is kept separately, and `soft` candidates also ENTER the catalog
— an unseen recommendation would otherwise be misclassified.

Identity: `id` is a hash of the sentence's normalized text, NOT a sequence number.
The triage ledger relies on these identities so adding a sentence elsewhere in the spec
does not shift existing candidate identities.
"""
import hashlib
import html
import re as _re
import json
import os
import pathlib
import re
import subprocess
import sys
import urllib.request

SPEC_URL = "https://gtfs.org/documentation/schedule/reference/"
OUT = pathlib.Path(__file__).resolve().parent / "spec_provisions.json"

# Strong normative markers. `Required`/`Forbidden` in title case are spec presence
# labels but also occur in prose; triage makes the distinction.
# `may not` states a prohibition, while `may or may not` states PERMISSION and is the
# opposite; without this context, `term-definitions`'s "It may or may not have field values
# defined" sentence was classified as a hard provision.
STRONG = re.compile(
    r"\b(must not|must|shall not|shall|cannot|(?<!or )may not|is not allowed|are not allowed"
    r"|is prohibited|Conditionally Required|Conditionally Forbidden|Required|Forbidden"
    r"|is required|are required)\b"
)
# 🔴 NORMATIVE PATTERNS WITHOUT A MODAL (issue #81).
# The spec states some provisions without a modal: *"All file and field names are case-sensitive."*
# This sentence has no `must`/`shall`, so modal-driven scanning did not see it AT ALL; because it
# was absent from the catalog, it could be neither adjudicated nor counted as orphaned — meaning
# "147/147" was not a statement about ALL known hard prose provisions.
#
# ⚠️ This set defines the SAME phenomenon as `measure_modalless.py::SIGNALS`; if they diverge,
# the same gap returns. `spec-audit/signal_parity.py` checks this in CI.
MODALLESS_STRONG = re.compile(r"\bcase[- ]sensitive\b", re.I)

# Soft markers — NOT provisions, but recommendations. They go to the Quality class.
SOFT = re.compile(r"\b(should not|should|recommended|Recommended|is encouraged|preferably)\b")

# ⚠️ UPPERCASE RFC 2119 KEYWORDS ARE KEPT SEPARATE (2026-08-06, 764 triage cases).
# The spec's `document-conventions` section writes keywords in UPPERCASE, and the
# service-continuity section of `transfers.txt` uses them literally. The regexes above do
# not use `re.I`, so EIGHT sentences from that section had been omitted from the catalog —
# three were hard provisions and were measured by NO rule.
#
# Why `re.I` WAS NOT ADDED: `Required`/`Forbidden` are spec presence LABELS and their
# capitalization is meaningful; `re.I` would also classify ordinary prose words
# "required"/"forbidden" as provisions. Title-case enum labels such as `Must coordinate with driver`
# would likewise create false positives. Only the ALL-CAPS form is included.
UPPER_STRONG = re.compile(r"\b(MUST NOT|MUST|SHALL NOT|SHALL|REQUIRED|FORBIDDEN)\b")
UPPER_SOFT = re.compile(r"\b(SHOULD NOT|SHOULD|RECOMMENDED)\b")
# `MAY`/`OPTIONAL` are deliberately EXCLUDED: they state permission, not provisions,
# and say nothing verifiable (`may not` is already in STRONG).

# ⚠️ THIRD CASE GAP (2026-08-06, individual review of 506 leftovers).
# The two regexes above searched for lowercase modals IN THE MIDDLE of a sentence and the ALL-CAPS form.
# The spec often STARTS field descriptions with a modal — the subject is in the previous sentence:
# *"Must be unique in areas.txt."* · *"Should not be a duplicate of stop_name."*
# It also writes prohibitions in lowercase (*"Values greater than 24:00:00 are forbidden"*)
# and sometimes uses inflected verbs (*"…requires two records…"*). Measurement found **20 sentences** missed this way.
#
# Why there is still NO `re.I`: `Required`/`Forbidden` are presence LABELS; `re.I` would
# also classify ordinary prose words "required"/"forbidden" as provisions. The gap's form
# is narrow and measured, so the pattern remains narrow.
#
# It does not conflict with the `^(Must|Shall)` enum label: enum lines begin with "3 - Must coordinate…",
# not a bare `Must` (all 8 of the 8 matches were real provisions).
LEAD_STRONG = re.compile(r"^(Must|Shall)\b|\b(is|are)\s+forbidden\b|\brequires\b")
LEAD_SOFT = re.compile(r"^Should\b")

# Block-level elements: each produces a separate text fragment. `<td>` is deliberately
# ABSENT here — table cells are collected through the separate `table_desc` path.
BLOCK = re.compile(r"</(p|li|h[1-6]|blockquote|dd|dt)>", re.I)

# Sentence-ending splitter. Abbreviations and URLs interfere with splitting; common ones are protected.
ABBREV = re.compile(r"\b(e\.g|i\.e|etc|vs|cf|St|Mr|Ms|Dr|approx|no|No)\.$")

# The "File: Required" line at the start of file sections. It is a provision BUT does not
# belong in this catalog: `extract_fields.py::file_presence_of()` already extracts it as an atom,
# and the seven conditional file provisions were adjudicated manually in `FILE_LEVEL_PROVISIONS.md`.
# Keeping it here would duplicate the provision in two ledgers and inflate the badge denominator.
FILE_HEADER = re.compile(r"^File:\s*(Required|Optional|Conditionally (Required|Forbidden))\.?$")


# SOURCE FROM WHICH THE SPEC IS GENERATED: gtfs.org renders this markdown.
UPSTREAM_REPO = "google/transit"
UPSTREAM_PATH = "gtfs/spec/en/reference.md"
UPSTREAM_API = (
    f"https://api.github.com/repos/{UPSTREAM_REPO}/commits"
    f"?path={UPSTREAM_PATH}&per_page=1"
)


def upstream_commit() -> dict:
    """Resolve the NEWEST upstream commit touching `reference.md` (issue #72).

    ⚠️ This does NOT REPLACE `sha256` — the two pin DIFFERENT things:
      · `sha256`          → the bytes WE PARSED (rendered HTML)
      · `upstream_commit` → the SOURCE revision of that text (markdown, google/transit)
    The catalog is generated from HTML, not markdown; the commit answers "which spec revision",
    NOT "which bytes". Both remain in the record; neither makes the other unnecessary.

    ⚠️ If it cannot be resolved, do not silently write null — `main()` stops with an error.
    An empty field in the record eventually makes "never populated" indistinguishable from
    "could not be populated" (`AGN_001` lesson). For offline generation, `--no-upstream` exists
    and the record states this EXPLICITLY.
    """
    # ⚠️ `curl`, NOT `urllib` — this script's docstring and `zip_peek.py` note the same trap:
    # development machines may lack Python's certificate store, causing `urlopen` to raise
    # CERTIFICATE_VERIFY_FAILED. `curl` works in both environments.
    cmd = ["curl", "-sSL", "--max-time", "60", "-H", "Accept: application/vnd.github+json",
           "-H", "User-Agent: gtfs-analyzer"]
    # ⚠️ AUTHENTICATED requests are required in CI: the unauthenticated quota is 60 per IP per hour,
    # and GitHub runners share that IP — the first run failed for exactly this reason on 2026-08-07.
    # Locally, continue unauthenticated when no token is present (the quota is enough for one person).
    if token := (os.environ.get("GH_TOKEN") or os.environ.get("GITHUB_TOKEN")):
        cmd += ["-H", f"Authorization: Bearer {token}"]
    p = subprocess.run(cmd + [UPSTREAM_API], capture_output=True)
    if p.returncode != 0:
        raise RuntimeError(f"curl failed: {p.stderr.decode()[:120]}")
    data = json.loads(p.stdout.decode("utf-8"))
    if isinstance(data, dict):  # API error bodies (rate limits, etc.) are JSON too
        raise RuntimeError(f"unexpected API response: {str(data)[:120]}")
    if not data:
        raise RuntimeError(f"no commit returned for {UPSTREAM_PATH}")
    return {
        "upstream_repo": UPSTREAM_REPO,
        "upstream_path": UPSTREAM_PATH,
        "upstream_commit": data[0]["sha"],
        "upstream_commit_date": data[0]["commit"]["committer"]["date"],
    }


def provisions_sha256(rows: list[dict]) -> str:
    """Fingerprint of the catalog ITSELF — not of the page.

    🔴 MEASURED on 2026-08-07: `sha256` (a digest of rendered HTML) CANNOT BE REPRODUCED.
    The page was downloaded three times in a row and produced three DIFFERENT digests; the only
    difference was two lines injected by Cloudflare (`data-cfemail` token + `__CF$cv$params` ray id).
    Thus the claim "the catalog came from different text if the hash does not match" was WRONG:
    the hash NEVER matches, even seconds later.

    This field is where that claim can actually stand: it is computed from candidates,
    independent of the site shell, and a third party downloading the same page obtains the SAME
    value. Drift detection (`spec_drift.py`) and issue deduplication rely on it.
    """
    canon = json.dumps(rows, ensure_ascii=False, sort_keys=True, separators=(",", ":"))
    return hashlib.sha256(canon.encode("utf-8")).hexdigest()


def source_fingerprint(doc: str, origin: str, upstream: dict, rows: list[dict]) -> dict:
    """Make it possible to prove which SOURCE TEXT produced the catalog.

    ⚠️ 2026-08-06 external review: JSON carried only the `_source` URL. gtfs.org is a LIVE
    page; if it changed tomorrow, there would be NO PROOF that today's 299 candidates came from
    the same text. The downloaded HTML's SHA-256 and the spec's own "Revised …" line are now recorded:
    if both match, the catalog can be reproduced bit for bit.

    ⚠️ 2026-08-07 (issue #72): the upstream commit was added — see `upstream_commit()`.
    """
    m = _re.search(r"Revised\s+([A-Z][a-z]+\s+\d{1,2},\s+\d{4})", strip_tags(doc))
    return {
        "url": origin,
        # ⚠️ This digest CANNOT BE REPRODUCED (Cloudflare changes two lines on every response);
        # it records "we saw these bytes", not a verifiable anchor. The anchor is
        # `provisions_sha256`.
        "sha256": hashlib.sha256(doc.encode("utf-8")).hexdigest(),
        "bytes": len(doc.encode("utf-8")),
        "provisions_sha256": provisions_sha256(rows),
        "spec_revision": m.group(1) if m else None,
        **upstream,
    }


def fetch(argv: list[str]) -> str:
    paths = [a for a in argv[1:] if not a.startswith("--")]
    if paths:
        return pathlib.Path(paths[0]).read_text(encoding="utf-8")
    with urllib.request.urlopen(SPEC_URL, timeout=60) as r:
        return r.read().decode("utf-8")


def strip_tags(fragment: str) -> str:
    text = html.unescape(re.sub(r"<[^>]+>", " ", fragment))
    return re.sub(r"\s+", " ", text).strip()


def outer_tables(section: str) -> list[tuple[int, int]]:
    """Ranges (start, end) of the OUTER `<table>` blocks in a section.

    Tables can be nested (such as the value table for `fare_transfer_type`), so depth
    is counted — the same rationale as in `extract_fields.py`.
    """
    out: list[tuple[int, int]] = []
    depth, start = 0, None
    for m in re.finditer(r"</?table\b[^>]*>", section):
        if m.group(0).startswith("</"):
            depth -= 1
            if depth == 0 and start is not None:
                out.append((start, m.end()))
                start = None
        else:
            if depth == 0:
                start = m.start()
            depth += 1
    return out


def prose_of(section: str) -> str:
    """HTML remaining OUTSIDE the tables in a section."""
    spans = outer_tables(section)
    if not spans:
        return section
    parts, cursor = [], 0
    for start, stop in spans:
        parts.append(section[cursor:start])
        cursor = stop
    parts.append(section[cursor:])
    return "".join(parts)


def description_cells(section: str) -> list[tuple[str, str]]:
    """Pairs of (field name, Description cell) from field tables.

    The Presence column is deliberately omitted: it is a LABEL, not a sentence, and is already
    counted as an atom by `extract_fields.py`. Description carries conditions that the presence
    label cannot express ("Required if trips.txt includes …").

    ⚠️ THE FIELD NAME IS REQUIRED. Description sentences without a field cannot be triaged:
    `stop_times.txt`'s "Required for timepoint=1" sentence applies separately to both
    `arrival_time` and `departure_time`, whose corresponding rules are DIFFERENT
    (`STM_015/016` vs `STM_034/047`). The first pass did not extract field names, so none of
    the 36 stop_times candidates could be adjudicated in that form.
    """
    cells: list[tuple[str, str]] = []
    for start, stop in outer_tables(section):
        table = section[start:stop]
        head = strip_tags(table[: table.find("</tr>") + 5]) if "</tr>" in table else ""
        if "field name" not in head.lower():
            continue
        # Walk row by row: cell 1 is the field name, cell 4 is the description. Nested-table
        # cells also match this regex, but they are PARTS of the Description, so this is noise,
        # not loss.
        for row in re.findall(r"<tr\b[^>]*>(.*?)</tr>", table, re.S | re.I):
            tds = re.findall(r"<td\b[^>]*>(.*?)</td>", row, re.S | re.I)
            if len(tds) >= 4:
                name = strip_tags(tds[0]).strip("`").strip()
                if not re.fullmatch(r"[A-Za-z_][A-Za-z0-9_.]*", name):
                    name = ""
                cells.append((name, tds[3]))
    return cells


def sentences(text: str) -> list[str]:
    """Coarse sentence splitting. Try not to split abbreviations and URLs."""
    out: list[str] = []
    buf = ""
    for chunk in re.split(r"(?<=[.!?])\s+", text):
        buf = f"{buf} {chunk}".strip() if buf else chunk
        if ABBREV.search(buf) or buf.endswith(("http:", "https:")) or re.search(r"\(\w\.\w\.$", buf):
            continue
        out.append(buf)
        buf = ""
    if buf:
        out.append(buf)
    return [s for s in (x.strip() for x in out) if len(s) > 12]


def blocks(fragment: str) -> list[str]:
    """Split by block elements, then split each block into sentences."""
    out: list[str] = []
    for piece in BLOCK.split(fragment):
        if piece.lower() in ("p", "li", "blockquote", "dd", "dt") or re.fullmatch(r"h[1-6]", piece, re.I):
            continue
        text = strip_tags(piece)
        if text:
            out.extend(sentences(text))
    return out


def section_bounds(doc: str) -> list[tuple[str, str, int, int]]:
    """(section name, level, start, end) — h2 and h3 anchors together.

    h3 elements are inside h2 elements; ordering both in one list and cutting each at the next
    HEADING makes every text fragment belong to its NEAREST heading. Otherwise the
    `field-definitions` h2 would swallow the entire 100 KB body.
    """
    heads = [
        (m.group(2), m.group(1), m.start(), m.end())
        for m in re.finditer(r'<h([23])[^>]*id="([^"]+)"[^>]*>', doc)
    ]
    out = []
    for i, (anchor, level, start, end) in enumerate(heads):
        stop = heads[i + 1][2] if i + 1 < len(heads) else len(doc)
        out.append((anchor, f"h{level}", end, stop))
    return out


def classify(sentence: str) -> str:
    if (STRONG.search(sentence) or UPPER_STRONG.search(sentence)
            or LEAD_STRONG.search(sentence) or MODALLESS_STRONG.search(sentence)):
        return "strong"
    if SOFT.search(sentence) or UPPER_SOFT.search(sentence) or LEAD_SOFT.search(sentence):
        return "soft"
    return ""


class ProvisionCollision(Exception):
    """Two DIFFERENT canonical keys mapped to the same identity — catalog generation STOPS."""


def canonical_key(section: str, sentence: str) -> str:
    """Canonical key from which the identity is derived. The identity is its hash; comparisons
    are ALWAYS made through this key, not through the truncated hash."""
    return f"{section}|{re.sub(r'[^a-z0-9]+', '', sentence.lower())}"


def ident(section: str, sentence: str) -> str:
    return "P" + hashlib.sha1(canonical_key(section, sentence).encode("utf-8")).hexdigest()[:8]


def claim_id(seen: dict[str, str], pid: str, key: str) -> bool:
    """Record an identity. `True` → new provision, `False` → repeat of the SAME provision (skip).

    🔴 COLLISIONS WERE FAIL-OPEN (issue #83). The old code said `if pid in seen: continue`:
    because the identity is a 32-bit truncated SHA-1, two DIFFERENT sentences could map to the
    same identity and the second would be silently dropped from the catalog — the mechanism that
    produces the denominator. The adjudicated-provision gate cannot see this: a provision absent
    from the catalog cannot be recognized as orphaned. The percentage would continue to claim
    "%100" using an incomplete denominator.

    ⚠️ The identity REMAINS 8 hex characters (extending it would invalidate all 299 decisions
    in the ledger — that is a MIGRATION decision, not a bug fix). Instead, collisions are checked
    EXPLICITLY and generation fails CLOSED.
    """
    prev = seen.get(pid)
    if prev is None:
        seen[pid] = key
        return True
    if prev == key:
        return False  # the same sentence was scanned twice — normal
    raise ProvisionCollision(
        f"kimlik çakışması {pid}: iki FARKLI kanonik anahtar aynı kimliğe düştü.\n"
        f"  1) {prev[:120]}\n  2) {key[:120]}\n"
        f"  → Kimlik şeması bir SÖZLEŞMEDİR (triyaj defteri bu kimliklere işaret eder). "
        f"Çözüm bir MİGRASYONDUR: hash genişletilir ve PROVISION_TRIAGE.md aynı commit'te "
        f"güncellenir. Sessizce düşürmek paydayı bozar."
    )


def _selftest() -> int:
    """FORCE the collision path and prove that it fails closed (issue #83).

    No network is needed; it runs in CI. It calls `claim_id` directly: two different keys are
    manually given the same identity. Writing a gate is not the same as setting one up — break it and see.
    """
    seen: dict[str, str] = {}
    assert claim_id(seen, "Pdeadbeef", "a|birinci cumle"), "ilk kayıt yeni olmalı"
    assert not claim_id(seen, "Pdeadbeef", "a|birinci cumle"), "aynı anahtar tekrarı atlanmalı"
    try:
        claim_id(seen, "Pdeadbeef", "a|ikinci FARKLI cumle")
    except ProvisionCollision as e:
        print(f"selftest OK — collision caught:\n  {str(e).splitlines()[0]}")
        return 0
    print("selftest FAILED: collision passed silently — catalogue failed open.", file=sys.stderr)
    return 1


def main() -> int:
    if "--selftest" in sys.argv:
        return _selftest()
    doc = fetch(sys.argv)
    # issue #72: source revision pin. If it cannot be resolved, do not fail silently.
    if "--no-upstream" in sys.argv:
        upstream = {
            "upstream_repo": UPSTREAM_REPO,
            "upstream_path": UPSTREAM_PATH,
            "upstream_commit": None,
            "upstream_commit_date": None,
            "upstream_commit_note": "--no-upstream ile üretildi: commit ÇÖZÜLMEDİ, "
                                    "'çözülemedi' ile 'hiç denenmedi' karışmasın diye "
                                    "burada açıkça yazılıdır.",
        }
        print("WARNING: --no-upstream — catalogue is NOT PINNED to a source revision.", file=sys.stderr)
    else:
        try:
            upstream = upstream_commit()
        except Exception as e:  # network/API error
            print(f"ERROR: could not resolve upstream commit ({type(e).__name__}: {e}). "
                  f"For offline generation, use: --no-upstream", file=sys.stderr)
            return 1
    rows: list[dict] = []
    seen: dict[str, str] = {}  # pid → canonical key (for collision checking)
    for anchor, level, start, stop in section_bounds(doc):
        section = doc[start:stop]
        items: list[tuple[str, str, str]] = [
            ("prose", "", s) for s in blocks(prose_of(section))
        ]
        items += [
            ("table_desc", name, s)
            for name, cell in description_cells(section)
            for s in sentences(strip_tags(cell))
        ]
        for source, field, sentence in items:
            if FILE_HEADER.match(sentence):
                continue
            strength = classify(sentence)
            if not strength:
                continue
            # The identity includes the field: the same sentence (e.g. "Required for timepoint=1")
            # appears in two field descriptions and represents TWO SEPARATE provisions.
            #
            # ⚠️ NO SEPARATOR IS ADDED FOR FIELDLESS LINES. The identity scheme is a contract:
            # the triage ledger points to these identities. Adding the separator unconditionally
            # when a field name was present shifted prose-line identities, making ALL 27 records
            # from the first pass impossible to resolve. Empty field = old key.
            section_key = f"{anchor}|{field}" if field else anchor
            pid = ident(section_key, sentence)
            if not claim_id(seen, pid, canonical_key(section_key, sentence)):
                continue
            rows.append({
                "id": pid,
                "section": anchor,
                "field": field,
                "level": level,
                "source": source,
                "strength": strength,
                "sentence": sentence,
            })
    if len(rows) < 150:
        print(f"ERROR: found only {len(rows)} candidates; the spec layout may have changed.",
              file=sys.stderr)
        return 1
    OUT.write_text(
        json.dumps(
            {
                "_source": source_fingerprint(doc, SPEC_URL, upstream, rows),
                "_note": "Üretilmiş dosya — elle düzenlemeyin. Yeniden üretmek için: "
                         "python3 spec-audit/extract_provisions.py. Satırlar ADAYDIR, "
                         "hüküm değil; triyaj spec-audit/PROVISION_TRIAGE.md'de. "
                         "`_source.provisions_sha256` kataloğun DOĞRULANABİLİR çapasıdır; tutmuyorsa "
                         "yüzde başka bir metne aittir. (`sha256` yeniden ÜRETİLEMEZ — sayfa her "
                         "yanıtta Cloudflare satırları yüzünden değişir; o alan yalnız kayıttır.) "
                         "`_source.upstream_commit` ise o metnin KAYNAK sürümünü (google/transit "
                         "reference.md) sabitler — sha256'nın yerine geçmez, farklı soruyu "
                         "yanıtlar: 'hangi spec sürümü' vs 'hangi baytlar'.",
                "provisions": rows,
            },
            ensure_ascii=False,
            indent=2,
        )
        + "\n",
        encoding="utf-8",
    )
    strong = sum(1 for r in rows if r["strength"] == "strong")
    prose = sum(1 for r in rows if r["source"] == "prose")
    print(f"wrote: {OUT.name} ({len(rows)} candidates · strong {strong} · soft {len(rows) - strong})")
    print(f"  source: prose {prose} · table descriptions {len(rows) - prose}")
    return 0


if __name__ == "__main__":
    try:
        raise SystemExit(main())
    except ProvisionCollision as exc:
        # Print a DIAGNOSTIC instead of a traceback: the message tells the reader what to do.
        print(f"ERROR: {exc}", file=sys.stderr)
        raise SystemExit(1) from None
