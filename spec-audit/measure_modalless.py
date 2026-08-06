#!/usr/bin/env python3
"""Kataloğun KÖR NOKTASINI ölç: modal taşımayan ama bağlayıcı olabilen cümleler.

`extract_provisions.py` modal-tabanlıdır (`must`/`shall`/`Required`/`should`…). Bu betik
onun ELEDİĞİ cümleleri toplar ve normatif olma ihtimaline göre sınıflar.

⚠️ Bu bir ADAY üreticisidir, sayaç değil. Betimleyici cümleyi hükümden ayırmak insan işidir;
betik yalnız "bakılması gereken kaç cümle var" sorusuna alt sınır verir.
"""
import html, json, pathlib, random, re, sys, collections

sys.path.insert(0, str(pathlib.Path(__file__).resolve().parent.parent))
_args = [a for a in sys.argv[1:] if not a.startswith("--")]
_flags = {a.split("=")[0]: a.split("=", 1)[-1] for a in sys.argv[1:] if a.startswith("--")}
SPEC = _args[0] if _args else "spec.html"
# --sample=N: sinyalsiz kümeden N cümlelik DETERMİNİSTİK örneklem bas (triyaj girdisi).
SAMPLE = int(_flags.get("--sample", 0))
SEED = int(_flags.get("--seed", 764))

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

# Sinyal TAŞIMAYAN cümlelerin YAPISAL kategorileri (2026-08-06 triyajı).
# Bunlar tek tek okunmaz, kategori olarak adjudike edilir ve TAM sayılır:
#   • PRIMARY KEY → normatif (benzersizlik ekseni); 32'si de adjudike edildi.
#   • ENUM / PRESENCE → alan tablosu ekseninin atomları; düzyazı paydasında değiller.
#   • BAŞLIK / ÖRNEK / JS → çıkarım gürültüsü ve betimleyici.
# ⚠️ `Primary Key` BÜYÜK K ile de yazılıyor (`fare_leg_join_rules.txt`) — case varsayımı
# bu betikte bir kez daha ısırmıştı.
STRUCTURAL = {
    "PRIMARY KEY (normatif)":        r"^Primary [Kk]ey\s*\(",
    "ENUM değeri (alan tablosu)":    r"^\s*\d+( or empty)?\s*[-–]\s|^Valid options are:",
    "BAŞLIK/TOC (gürültü)":          r"¶$|^[a-z_]+\.txt$|^(Field Definitions|File Requirements|Dataset Files|Term Definitions)",
    # `^Example[: ]` BÜYÜK E bilinçli (cümle başı etiketi); `for example` ise cümle içinde
    # de başında da geçer → yalnız O ALTERNATİF case-insensitive (`(?i:...)`).
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

    if "--residual" in _flags:
        # Ne modal, ne normatif sinyal, ne de yapısal kategori taşıyan cümleler.
        # 2026-08-06'da bu kümenin TAMAMI okundu; çıktı defterle karşılaştırılabilsin diye
        # sıra DETERMİNİSTİK (belge sırası) ve numaralı.
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
