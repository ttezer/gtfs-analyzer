#!/usr/bin/env python3
"""Measure the catalogue's BLIND SPOT: sentences without a modal that may still be binding.

`extract_provisions.py` is modal-based (`must`/`shall`/`Required`/`should`…). This script
collects the sentences it FILTERS OUT and classifies them by likelihood of being normative.

⚠️ This is a CANDIDATE generator, not a counter. Separating descriptive sentences from provisions
is human work; the script only gives a lower bound for "how many sentences need review".
"""
import html, json, pathlib, random, re, sys, collections

sys.path.insert(0, str(pathlib.Path(__file__).resolve().parent.parent))
_args = [a for a in sys.argv[1:] if not a.startswith("--")]
_flags = {a.split("=")[0]: a.split("=", 1)[-1] for a in sys.argv[1:] if a.startswith("--")}
SPEC = _args[0] if _args else "spec.html"
# --sample=N: print a DETERMINISTIC sample of N sentences from the unsignalled set (triage input).
SAMPLE = int(_flags.get("--sample", 0))
SEED = int(_flags.get("--seed", 764))

# SAME logic as extract_provisions.py — otherwise the "filtered" set would be wrong.
sys.path.insert(0, "spec-audit")
import extract_provisions as ep

# Signals that MAY be normative. No modal, but they may express a constraint/prohibition/condition.
SIGNALS = {
    "case-sensitivity": r"\bcase[- ]sensitive\b",
    "kısıt (only/just)": r"\b(only|just)\b",
    "sayısal sınır": r"\b(at least|at most|no more than|no fewer than|maximum of|minimum of|up to)\b",
    "mutlak (never/always)": r"\b(never|always)\b",
    "öncelik/geçersiz kılma": r"\b(takes? precedence|overrid\w+|supersed\w+)\b",
    "olumsuz durum": r"\b(is|are|has|have|does|do)\s+not\b",
    "uygulanma": r"\b(applies to|apply to|applicable)\b",
    "eşitlik/özdeşlik": r"\b(same as|identical to|equal to|matches?)\b",
    "emir kipi": r"^(Use|Refer|Do not|Avoid|Ensure|Note that|Provide|Set|Include|Omit)\b",
    "koşul (if/when/unless)": r"^(If|When|Unless|Otherwise)\b|\b(if and only if)\b",
}
# Patterns indicating a descriptive sentence — LOWER the likelihood of treating it as normative.
DESCRIPTIVE = re.compile(
    r"\b(defines?|indicates?|specifies|describes?|denotes?|identifies|represents?|"
    r"for example|e\.g\.|i\.e\.|see the|refer to https?|available at)\b", re.I)

# STRUCTURAL categories of sentences with NO SIGNAL (2026-08-06 triage).
# These are not read one by one; adjudicate the category and count it in FULL:
#   • PRIMARY KEY → normative (uniqueness axis); all 32 were adjudicated.
#   • ENUM / PRESENCE → atoms of the field-table axis; not in the prose denominator.
#   • HEADING / EXAMPLE / JS → extraction noise and descriptive text.
# ⚠️ `Primary Key` also appears with an uppercase K (`fare_leg_join_rules.txt`) — the case assumption
# bit this script once again.
STRUCTURAL = {
    "PRIMARY KEY (normatif)":        r"^Primary [Kk]ey\s*\(",
    "ENUM değeri (alan tablosu)":    r"^\s*\d+( or empty)?\s*[-–]\s|^Valid options are:",
    "BAŞLIK/TOC (gürültü)":          r"¶$|^[a-z_]+\.txt$|^(Field Definitions|File Requirements|Dataset Files|Term Definitions)",
    # `^Example[: ]` has an intentional uppercase E (sentence-start label); `for example` can occur
    # both inside and at the start of a sentence → only THAT ALTERNATIVE is case-insensitive (`(?i:...)`).
    "ÖRNEK (betimleyici)":           r"^Example[: ]|\b(?i:for example)\b",
    "PRESENCE parçası (alan tablosu)": r"^-\s*(Optional|Required|Forbidden|Conditionally)",
    "JS/sayfa çöpü (gürültü)":       r"function\(|__md_get|document\.|window\.__",
}


def main() -> int:
    doc = pathlib.Path(SPEC).read_text(encoding="utf-8")
    modal_hits, modalless = 0, []
    for anchor, level, start, stop in ep.section_bounds(doc):
        section = doc[start:stop]
        items = [("prose", "", s) for s in ep.blocks(ep.prose_of(section))]
        items += [("table_desc", name, s)
                  for name, cell in ep.description_cells(section)
                  for s in ep.sentences(ep.strip_tags(cell))]
        for source, field, sentence in items:
            if ep.FILE_HEADER.match(sentence):
                continue
            if ep.classify(sentence):          # modal PRESENT → already in the catalogue
                modal_hits += 1
                continue
            modalless.append((anchor, field, source, sentence))

    # Match signals.
    tagged = collections.defaultdict(list)
    untagged = []
    for anchor, field, source, s in modalless:
        hits = [name for name, pat in SIGNALS.items() if re.search(pat, s)]
        if hits:
            tagged[tuple(sorted(hits))].append((anchor, field, s))
        else:
            untagged.append((anchor, field, s))

    if "--residual" in _flags:
        # Sentences with neither a modal, a normative signal, nor a structural category.
        # This set was read in FULL on 2026-08-06; make the order DETERMINISTIC (document order)
        # and numbered so the output can be compared with the ledger.
        pats = {k: re.compile(v) for k, v in STRUCTURAL.items()}
        rest = [(a, f, s) for a, f, s in untagged
                if not any(p.search(s) for p in pats.values())]
        print(f"# sinyalsiz {len(untagged)} · yapısal kategori {len(untagged) - len(rest)} "
              f"· SINIFLANMAMIŞ KALAN {len(rest)}")
        for i, (anchor, field, s) in enumerate(rest, 1):
            loc = anchor + (f".{field}" if field else "")
            print(f"{i:3d}. [{loc}] {s}")
        return 0

    if SAMPLE:
        picked = random.Random(SEED).sample(untagged, min(SAMPLE, len(untagged)))
        print(f"# sinyalsiz küme: {len(untagged)} · örneklem: {len(picked)} · seed={SEED}")
        for i, (anchor, field, s) in enumerate(sorted(picked), 1):
            loc = anchor + (f".{field}" if field else "")
            print(f"{i:3d}. [{loc}] {s}")
        return 0

    total_modalless = len(modalless)
    signalled = sum(len(v) for v in tagged.values())
    # Separate those carrying a descriptive marker.
    strong_candidates = []
    weak = 0
    for combo, rows in tagged.items():
        for anchor, field, s in rows:
            if DESCRIPTIVE.search(s):
                weak += 1
            else:
                strong_candidates.append((combo, anchor, field, s))

    print(f"modal TAŞIYAN cümle (katalogda)      : {modal_hits}")
    print(f"modal TAŞIMAYAN cümle                : {total_modalless}")
    print(f"  ├─ normatif sinyal taşıyan         : {signalled}")
    print(f"  │    ├─ betimleyici kalıp da var   : {weak}  (düşük olasılık)")
    print(f"  │    └─ SAF ADAY                   : {len(strong_candidates)}")
    print(f"  └─ hiç sinyal yok                  : {len(untagged)}  (büyük olasılıkla betimleyici)")
    print()
    print("=== SAF ADAYLAR, sinyal grubuna göre ===")
    by_combo = collections.Counter(c for c, *_ in strong_candidates)
    for combo, n in by_combo.most_common():
        print(f"\n--- {' + '.join(combo)}  ({n}) ---")
        shown = 0
        for c, anchor, field, s in strong_candidates:
            if c != combo or shown >= 4:
                continue
            loc = f"{anchor}" + (f".{field}" if field else "")
            print(f"  [{loc[:30]:30s}] {s[:120]}")
            shown += 1
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
