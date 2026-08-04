#!/usr/bin/env python3
"""Kataloğun KÖR NOKTASINI ölç: modal taşımayan ama bağlayıcı olabilen cümleler.

`extract_provisions.py` modal-tabanlıdır (`must`/`shall`/`Required`/`should`…). Bu betik
onun ELEDİĞİ cümleleri toplar ve normatif olma ihtimaline göre sınıflar.

⚠️ Bu bir ADAY üreticisidir, sayaç değil. Betimleyici cümleyi hükümden ayırmak insan işidir;
betik yalnız "bakılması gereken kaç cümle var" sorusuna alt sınır verir.
"""
import html, json, pathlib, re, sys, collections

sys.path.insert(0, str(pathlib.Path(__file__).resolve().parent.parent))
SPEC = sys.argv[1] if len(sys.argv) > 1 else "spec.html"

# extract_provisions.py ile AYNI mantık — yoksa "elenmiş" küme yanlış olur.
sys.path.insert(0, "spec-audit")
import extract_provisions as ep

# Normatif OLABİLİR sinyalleri. Modal yok ama kısıt/yasak/koşul bildiriyor olabilir.
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
# Betimleyici olduğuna işaret eden kalıplar — normatif sayılma ihtimalini DÜŞÜRÜR.
DESCRIPTIVE = re.compile(
    r"\b(defines?|indicates?|specifies|describes?|denotes?|identifies|represents?|"
    r"for example|e\.g\.|i\.e\.|see the|refer to https?|available at)\b", re.I)


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
            if ep.classify(sentence):          # modal VAR → katalogda zaten
                modal_hits += 1
                continue
            modalless.append((anchor, field, source, sentence))

    # sinyal eşleştirme
    tagged = collections.defaultdict(list)
    untagged = []
    for anchor, field, source, s in modalless:
        hits = [name for name, pat in SIGNALS.items() if re.search(pat, s)]
        if hits:
            tagged[tuple(sorted(hits))].append((anchor, field, s))
        else:
            untagged.append((anchor, field, s))

    total_modalless = len(modalless)
    signalled = sum(len(v) for v in tagged.values())
    # betimleyici işareti taşıyanları ayır
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
