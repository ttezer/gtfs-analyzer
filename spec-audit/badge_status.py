#!/usr/bin/env python3
"""GTFS Spec badge: completion percentage for the prose axis.

Input is `PROVISION_TRIAGE.md` — verdict labels are read from its rows, and closures from the
"STATUS MACHINE" section. The percentage is not calculated MANUALLY.

⚠️ The DENOMINATOR is HARD PROVISIONS. Soft (`should`) provisions are not required by the spec;
counting them as "unmet spec provisions" repeats PTH_017's mistake at the ledger level.
⚠️ OUT-OF-SCOPE + META are REMOVED from the denominator: when a validator does not measure a
sentence that constrains consumer behavior or defines a term, that is required by definition, not a gap.
"""
import collections, hashlib, json, pathlib, re, sys

ROOT = pathlib.Path(__file__).resolve().parent
ORDER = ["BOŞLUK", "KAPSAM DIŞI", "KANITLI", "KISMİ", "DOLAYLI", "META"]


def extract_verdicts(prov: dict[str, dict], text: str) -> dict[str, str]:
    verdict, seen = {}, set()
    for line in text.split("\n"):
        ids = [x for x in re.findall(r"`(P[0-9a-f]{8})`", line) if x in prov and x not in seen]
        if not ids:
            continue
        v = next((k for k in ORDER if k in line), None)
        if not v:
            continue
        for x in ids:
            seen.add(x)
            verdict[x] = v

    # KISMİ adjudications and closures — from the "STATUS MACHINE" section.
    section = text[text.index("## 📊 DURUM MAKİNESİ"):text.index("# ✅ KATALOG TAMAMLANDI")]
    closed = set()
    for line in section.split("\n"):
        if line.startswith("|") and "`" in line and "commit" not in line and "aday" not in line:
            closed.update(re.findall(r"`(P[0-9a-f]{8})`", line))
    for m in re.finditer(r"`(P[0-9a-f]{8})`→(BOŞLUK|KAPSAM DIŞI)", section):
        verdict[m.group(1)] = m.group(2)
    for pid in closed:
        verdict[pid] = "KANITLI"
    return verdict


def extract_rule_classes(registry_text: str) -> dict[str, str]:
    return {
        m.group(1): m.group(3)
        for m in re.finditer(
            r'r!\("([A-Z]{2,3}_\d{3}[a-z]?)",\s*(\w+),\s*(\w+)', registry_text
        )
    }


def load_provision_evidence(path: pathlib.Path) -> dict[str, list[str]]:
    rows: dict[str, list[str]] = {}
    if path.exists():
        for line in path.read_text(encoding="utf-8").split("\n"):
            if not line.strip() or line.startswith("#"):
                continue
            cols = [c.strip() for c in line.split("\t")]
            if cols and cols[0]:
                rows[cols[0]] = [
                    r.strip() for r in (cols[2] if len(cols) > 2 else "").split(",") if r.strip()
                ]
    return rows


def evidence_fingerprint(pid: str, rules: list[str], rule_class: dict[str, str]) -> str:
    """Hash the provision and its canonical (rule_id, class) evidence pairs.

    Severity is intentionally absent: severity tuning does not invalidate semantic
    evidence and must not reopen every adjudication.
    """
    pairs = sorted({(rule_id, rule_class[rule_id]) for rule_id in rules})
    canonical = "\n".join([pid, *(f"{rule_id}\t{rule_type}" for rule_id, rule_type in pairs)])
    return hashlib.sha256(canonical.encode("utf-8")).hexdigest()


def check_evidence_fingerprints(
    active: dict[str, str],
    evidence: dict[str, list[str]],
    rule_class: dict[str, str],
    lock_path: pathlib.Path,
) -> list[str]:
    """Require explicit re-adjudication when an active evidence set changes.

    This is a staleness gate, not a semantic proof checker: a reviewer can still
    approve an incomplete rule as KANITLI. The fingerprint only ensures that a
    changed (rule_id, class) set cannot silently retain an old adjudication.
    """
    locked: dict[str, str] = {}
    if lock_path.exists():
        for line in lock_path.read_text(encoding="utf-8").split("\n"):
            if not line.strip() or line.startswith("#"):
                continue
            cols = [c.strip() for c in line.split("\t")]
            if len(cols) == 2 and cols[0]:
                locked[cols[0]] = cols[1]

    stale: list[str] = []
    active_ids = {
        pid
        for pid, verdict in active.items()
        if verdict in ("KANITLI", "KISMİ", "DOLAYLI") and pid in evidence
    }
    for pid in sorted(active_ids):
        rules = evidence.get(pid)
        if not rules or any(rule_id not in rule_class for rule_id in rules):
            continue  # The existing registry/evidence gate reports this separately.
        expected = evidence_fingerprint(pid, rules, rule_class)
        actual = locked.get(pid)
        if actual is None:
            stale.append(f"SERT {pid} kanıt fingerprint'i yok; yeniden adjudike edin.")
        elif actual != expected:
            stale.append(
                f"SERT {pid} kanıt fingerprint'i bayat: (rule_id, class) kümesi değişmiş; "
                "hükmü yeniden adjudike edin."
            )
    extra = sorted(set(locked) - active_ids)
    for pid in extra:
        stale.append(f"Kanıt fingerprint'i artık aktif olmayan hükmü gösteriyor: {pid}.")
    return stale


def main() -> int:
    prov = {r["id"]: r for r in json.loads((ROOT / "spec_provisions.json").read_text())["provisions"]}
    text = (ROOT / "PROVISION_TRIAGE.md").read_text()

    verdict = extract_verdicts(prov, text)

    hard = {k: v for k, v in verdict.items() if prov[k]["strength"] == "strong"}
    soft = {k: v for k, v in verdict.items() if prov[k]["strength"] == "soft"}

    def report(name, bucket):
        c = collections.Counter(bucket.values())
        total = sum(c.values())
        unmeasurable = c["KAPSAM DIŞI"] + c["META"]
        denom = total - unmeasurable
        num = c["KANITLI"] + c["DOLAYLI"]
        print(f"\n=== {name} ===")
        for k in ORDER:
            if c[k]:
                print(f"  {k:12s} {c[k]:3d}")
        print(f"  toplam {total} · ölçülemez {unmeasurable} · payda {denom} · ölçülen {num}")
        if denom:
            print(f"  → %{100 * num / denom:.1f}")
        return num, denom

    n, d = report("SERT hükümler (rozet paydası)", hard)
    report("YUMUŞAK hükümler (ayrı eksen — Quality)", soft)

    # ⚠️ UNADJUDICATED PROVISION = SILENT DENOMINATOR LOSS. A candidate not named in the ledger
    # enters none of the counts above, so the percentage continues to say "%100". This is exactly
    # what happened when the catalogue grew 273→279 on 2026-08-06. The ID is a hash, not a row number:
    # when the catalogue grows, old IDs do not shift; only the NEW ones remain unadjudicated.
    orphans = sorted(set(prov) - set(verdict))
    if orphans:
        print(f"\n🔴 ADJUDİKE EDİLMEMİŞ {len(orphans)} HÜKÜM — yüzde EKSİK PAYDA üzerinden.")
        for pid in orphans:
            print(f"  {pid} [{prov[pid]['strength']}] [{prov[pid]['section']}] "
                  f"{prov[pid]['sentence'][:70]}")
        print("  → PROVISION_TRIAGE.md'ye karar satırı ekleyin; yüzde ancak ondan sonra geçerlidir.")

    # 🔴 THE LEDGER HEADER MUST MATCH THE CALCULATED PERCENTAGE — this is a GATE.
    # 2026-08-07: after `STM_060` was added, the ledger header remained at 145/147,
    # EVIDENCE_BASE.md said 146/147. The `ledger_header_counts_match_catalogue` test checked
    # only the CATALOGUE COUNT ("299 candidates"), never the percentage — even though the ledger
    # claimed that line would "break CI if the test went stale".
    # The check belongs here because this is the only place that calculates the count; moving it
    # to Rust would duplicate the calculation.
    # ⚠️ "Does it occur in the first 20 lines?" is NOT ENOUGH — the header block also contains a
    # PROGRESS line ("131/131 → … → 146/147"), so the check passed there even when the STATUS line was stale.
    # Deliberately breaking the gate exposed the lesson: anchor a gate to the LINE it protects.
    durum = next((l for l in text.split("\n") if l.startswith("> **DURUM")), "")
    if f"{n}/{d}" not in durum:
        print(f"\n🔴 DEFTER BAŞLIĞI BAYAT — hesaplanan {n}/{d}, ama PROVISION_TRIAGE.md'nin"
              f" '> **DURUM' satırında '{n}/{d}' geçmiyor.\n  satır: {durum[:110]}")
        print("  → Başlıktaki DURUM satırını güncelleyin; yüzde iki yerde farklı duramaz.")
        orphans = list(orphans) + ["<defter-başlığı>"]

    stale: list[str] = []

    # 🔴 DOLAYLI = COUNTED IN THE NUMERATOR WITHOUT EVIDENCE — proof is REQUIRED (issue #87).
    # Because `num = KANITLI + DOLAYLI`, a DOLAYLI row raises the percentage, but the
    # label asked for nothing proving that the neighboring rule actually produced a violation.
    # `Pfedb83cf`/`Pa4c60372` failed exactly this way: code reading was mistaken for measurement.
    # ⚠️ A VIOLATING input alone is insufficient — there is no difference if the neighboring rule fires on every feed.
    # Therefore the ledger row MUST also name the CHECK input.
    ev_file = ROOT / "dolayli_evidence.tsv"
    ev_rows: dict[str, list[str]] = {}
    if ev_file.exists():
        for line in ev_file.read_text(encoding="utf-8").split("\n"):
            if not line.strip() or line.startswith("#"):
                continue
            cols = [c.strip() for c in line.split("\t")]
            if cols and cols[0]:
                ev_rows[cols[0]] = cols
    tests_src = ""
    for t in (ROOT.parent / "crates/pipeline/tests").glob("*.rs"):
        tests_src += t.read_text(encoding="utf-8")

    dolayli = sorted([k for k, v in hard.items() if v == "DOLAYLI"])
    for pid in dolayli:
        row = ev_rows.get(pid)
        if row is None:
            stale.append(f"DOLAYLI {pid} kanıtsız: dolayli_evidence.tsv'de satırı YOK "
                         f"(pay onu ölçülmüş sayıyor).")
            continue
        if len(row) < 5 or not all(row[:5]):
            stale.append(f"DOLAYLI {pid} kanıt satırı eksik alan taşıyor: {row}")
            continue
        test_name = row[4]
        if f"fn {test_name}(" not in tests_src:
            stale.append(f"DOLAYLI {pid} kanıtı olmayan bir testi gösteriyor: {test_name}")

    # 🔴 EVIDENCE CLASS GATE (issue #96) — the numerator did not inspect the CLASS of the rule being measured.
    # A hard Spec provision could be counted as "evidenced" even when all supporting rules were Quality/Interop;
    # a user filtering to Spec or reading the R1 view would not see that violation AT ALL. The 2026-08-09 scan
    # found 21 such rows (the issue said 14 and called it a lower bound — extracting rule names from prose also
    # counts rhetorical references).
    # ⚠️ The gate does NOT READ PROSE: evidence is in `provision_evidence.tsv`. The rationale is the issue's own
    # finding — ten ledger rows mentioned `PTH_017` as a "lesson not to repeat" and the prose scan treated it as proof.
    # Evidence is declared, not inferred.
    prov_ev_file = ROOT / "provision_evidence.tsv"
    prov_ev = load_provision_evidence(prov_ev_file)

    rule_class = extract_rule_classes(
        (ROOT.parent / "crates/rules/src/registry.rs").read_text(encoding="utf-8")
    )

    for pid in sorted(k for k, v in hard.items() if v in ("KANITLI", "DOLAYLI")):
        rules = prov_ev.get(pid)
        if rules is None:
            stale.append(f"SERT {pid} paya sayılıyor ama provision_evidence.tsv'de satırı YOK.")
            continue
        unknown = [r for r in rules if r not in rule_class]
        if unknown:
            stale.append(f"SERT {pid} registry'de olmayan kural gösteriyor: {unknown}")
            continue
        if not any(rule_class[r] == "Spec" for r in rules):
            shown = ", ".join(f"{r}({rule_class[r]})" for r in rules) or "<boş>"
            stale.append(f"SERT {pid} hiçbir Spec sınıflı kurala dayanmıyor: {shown} "
                         f"→ kuralı Spec'e taşıyın, atomik Spec kuralı yazın ya da hükmü KISMİ'ye düşürün.")

    # 🔐 Evidence fingerprint gate: prevent an old adjudication from silently remaining valid when
    # a rule ID or class changes. This catches STALENESS only; a fingerprint is not proof of semantic
    # coverage (human adjudication determines whether a rule measures the whole provision). Severity
    # is intentionally excluded from the fingerprint.
    stale.extend(
        check_evidence_fingerprints(
            hard,
            prov_ev,
            rule_class,
            ROOT / "provision_evidence.lock",
        )
    )

    # 🔴 The evidence document's §0 COUNTS ALSO COME FROM THIS CALCULATION — a second gate.
    # 2026-08-07: the user found two stale numbers (the SECOND manual catch).
    # §0 said "25 of 184 hard provisions are OUT OF SCOPE, 13 are META; 184 − 38 = 146"; that day
    # the calculation was 24 / 13 / 184 − 37 = 147. The ledger-header gate could not see this because
    # it looked at a DIFFERENT FILE. The number is calculated here, so the gate belongs here too.
    # ⚠️ Anchor the gate to the LINE IT PROTECTS (the ledger-header lesson): §0's items 4 and 3
    # are found separately; do not ask merely whether the number occurs somewhere in the file.
    ev_lines = (ROOT / "EVIDENCE_BASE.md").read_text(encoding="utf-8").split("\n")

    total, excluded = len(hard), len(hard) - d
    counts = collections.Counter(hard.values())
    arith_line = next((l for l in ev_lines if "sert hükmün" in l), "")
    arith_next = ev_lines[ev_lines.index(arith_line) + 1] if arith_line in ev_lines else ""
    arith_text = (arith_line + " " + arith_next).replace("−", "-")
    want_arith = f"{total} - {excluded} = {d}"
    if want_arith not in arith_text:
        stale.append(f"§0/4 aritmetiği: '{want_arith}' geçmiyor → {arith_text.strip()[:90]}")
    for label in ("KAPSAM DIŞI", "META"):
        want = counts.get(label, 0)
        # ⚠️ Anchor the number to the LABEL. Asking whether it occurs on the line is insufficient:
        # an arithmetic digit (for example, 37) could coincidentally equal the target number.
        # `\S*` permits the Turkish suffix form ("24'ü") without permitting the number to drift.
        if not re.search(rf"{want}\S*\s+(?:\*\*)?{label}", arith_text):
            stale.append(f"§0/4 {label} sayısı {want} değil → {arith_text.strip()[:90]}")

    # The ratio in the §1 table also comes from the calculation (it remained 147/147 on 2026-08-07).
    axis_line = next((l for l in ev_lines if "Düzyazı hükümleri" in l), "")
    if f"{n} / {d}" not in axis_line and f"{n}/{d}" not in axis_line:
        stale.append(f"§1 eksen satırı {n}/{d} demiyor → {axis_line.strip()[:90]}")

    # The manifest is the corpus size's SINGLE SOURCE; §0/3 must not diverge from it.
    man = ROOT / "corpus-evidence" / "corpus_manifest.csv"
    feeds = max(len(man.read_text(encoding="utf-8").strip().split("\n")) - 1, 0)
    corpus_line = next((l for l in ev_lines if "Korpus" in l and "feed'dir" in l), "")
    if not re.search(rf"Korpus {feeds} feed'dir", corpus_line):
        stale.append(f"§0/3 korpus sayısı {feeds} değil (manifest {feeds} satır) → "
                     f"{corpus_line.strip()[:90]}")

    if stale:
        print("\n🔴 KANIT KAPISI — aşağıdakiler payı hak etmiyor:")
        for msg in stale:
            print(f"  {msg}")
        print("  → §0'ı güncelleyin; aynı sayı iki belgede farklı duramaz.")
        orphans = list(orphans) + ["<kanıt-belgesi-§0>"]

    print(f"\n{'='*52}\nDÜZYAZI EKSENİ: {n}/{d} = %{100*n/d:.1f}")
    gaps = [k for k, v in hard.items() if v == "BOŞLUK"]
    for g in gaps:
        print(f"  kalan: {g}  {prov[g]['sentence'][:70]}")
    print("\n⚠️ Bu yüzde YALNIZ düzyazı eksenidir. Alan tablosu ekseni AYRI ölçülür ve iki")
    print("   eksen ÖRTÜŞÜR — toplanamazlar. Alan tablosu durumu (2026-08-05 triyajı):")
    print("   300 GEÇERLİ atomun tamamı çapalı (2 atom HATALI ÜRETİM: transfer_type ve")
    print("   continuous_drop_off — enum düzyazısı boşa izin veriyor, presence sütunu değil);")
    print("   32'si tek kuralla paylaşımlı ve triyajda MEŞRU bulundu; 12 boşluk kapatıldı.")
    print("   Ölçüm: cargo test -- --ignored anchor_granularity_report --nocapture")
    print("⚠️ Payda kataloğun kendi kapsamıdır. Modalsiz cümlelerin TAMAMI 2026-08-06'da okundu")
    print("   ve payda İKİ KEZ düzeldi: büyük harf RFC 2119 (273→279), sonra cümle başı modal +")
    print("   küçük harf `are forbidden` + `requires` (279→299). Ayrıca `Primary key ( … )`")
    print("   bildirimleri (31) BENZERSİZLİK eksenidir, düzyazı paydasında değil — oradaki 3")
    print("   boşluk `DQ_021`'e eklendi.")
    print("   🔴 KALAN ZAYIF NOKTA SAYI DEĞİL, PAYDANIN NASIL KURULDUĞU: üç tur üst üste hata")
    print("   çıktı (iki case varsayımı + bir yanlış KAPSAM DIŞI). Cümle bölme kaba, ~%14 gürültü.")
    # ⚠️ EXIT CODE 1 WHEN ORPHANS EXIST — printing a warning and exiting 0 is not a GATE, but advice.
    # The 2026-08-06 external audit caught this: the document said to "treat the percentage as invalid",
    # but the script still printed it and exited successfully → a "gate" dependent on human interpretation.
    return 1 if orphans else 0


if __name__ == "__main__":
    raise SystemExit(main())
