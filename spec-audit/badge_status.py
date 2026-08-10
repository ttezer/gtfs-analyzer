#!/usr/bin/env python3
"""GTFS Spec rozeti: düzyazı ekseninde tamamlanma yüzdesi.

Girdi `PROVISION_TRIAGE.md`'dir — karar etiketleri satırlardan, kapanışlar "DURUM MAKİNESİ"
bölümünden okunur. Yüzde ELLE hesaplanmaz.

⚠️ PAYDA SERT HÜKÜMLERDİR. Yumuşak (`should`) hükümler spec'in zorunlu kılmadığı şeylerdir;
onları "karşılanmamış spec hükmü" saymak PTH_017'nin hatasının defter düzeyinde tekrarıdır.
⚠️ KAPSAM DIŞI + META paydadan DÜŞER: tüketici davranışını bağlayan ya da tanım olan bir
cümleyi bir doğrulayıcının ölçmemesi eksiklik değil, tanım gereğidir.
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

    # KISMİ adjudikasyonu ve kapanışlar — "DURUM MAKİNESİ" bölümünden
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

    # ⚠️ ADJUDİKE EDİLMEMİŞ HÜKÜM = SESSİZ PAYDA KAYBI. Defterde adı geçmeyen bir aday
    # yukarıdaki sayımların hiçbirine girmez; yüzde "%100" demeye devam eder. 2026-08-06'da
    # katalog 273→279 büyüdüğünde tam olarak bu olurdu. Kimlik hash'tir, sıra numarası değil:
    # katalog büyüdüğünde eski kimlikler kaymaz, yalnız YENİLER adjudike edilmemiş kalır.
    orphans = sorted(set(prov) - set(verdict))
    if orphans:
        print(f"\n🔴 ADJUDİKE EDİLMEMİŞ {len(orphans)} HÜKÜM — yüzde EKSİK PAYDA üzerinden.")
        for pid in orphans:
            print(f"  {pid} [{prov[pid]['strength']}] [{prov[pid]['section']}] "
                  f"{prov[pid]['sentence'][:70]}")
        print("  → PROVISION_TRIAGE.md'ye karar satırı ekleyin; yüzde ancak ondan sonra geçerlidir.")

    # 🔴 DEFTER BAŞLIĞI HESAPLANAN YÜZDEYLE TUTMALI — bu bir KAPIDIR.
    # 2026-08-07: `STM_060` eklendikten sonra defterin başlığı 145/147'de kaldı,
    # EVIDENCE_BASE.md 146/147 yazıyordu. `ledger_header_counts_match_catalogue` testi
    # yalnız KATALOG SAYISINI ("299 aday") denetliyordu, yüzdeye hiç bakmıyordu — üstelik
    # defterin o satırı "test bayatlarsa CI'ı kırar" DİYE İDDİA EDİYORDU.
    # Kontrol buraya kondu çünkü sayıyı hesaplayan tek yer burası; Rust tarafına
    # taşımak hesabı ikinci kez yazmak olurdu.
    # ⚠️ "ilk 20 satırda geçiyor mu" YETMEZ — başlık bloğu bir de SEYİR satırı taşıyor
    # ("131/131 → … → 146/147") ve orada geçtiği için DURUM satırı bayatlasa bile kontrol
    # geçiyordu. Kapıyı bilerek bozunca çıktı: kapı, koruduğu SATIRA çapalanmalı.
    durum = next((l for l in text.split("\n") if l.startswith("> **DURUM")), "")
    if f"{n}/{d}" not in durum:
        print(f"\n🔴 DEFTER BAŞLIĞI BAYAT — hesaplanan {n}/{d}, ama PROVISION_TRIAGE.md'nin"
              f" '> **DURUM' satırında '{n}/{d}' geçmiyor.\n  satır: {durum[:110]}")
        print("  → Başlıktaki DURUM satırını güncelleyin; yüzde iki yerde farklı duramaz.")
        orphans = list(orphans) + ["<defter-başlığı>"]

    stale: list[str] = []

    # 🔴 DOLAYLI = PAYDA DEĞİL PAYDA SAYILIYOR — kanıt ZORUNLU (issue #87).
    # `num = KANITLI + DOLAYLI` olduğu için DOLAYLI bir satır yüzdeyi yukarı çeker; ama
    # etiket, komşu kuralın ihlali gerçekten ürettiğine dair hiçbir şey istemiyordu.
    # `Pfedb83cf`/`Pa4c60372` tam böyle çürüdü: kod okuması ölçüm sanılıyordu.
    # ⚠️ İHLALLİ girdi tek başına yetmez — komşu kural her feed'de ateşliyorsa fark yoktur.
    # Bu yüzden defter satırı KONTROL girdisini de adlandırmak ZORUNDA.
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

    # 🔴 KANIT SINIFI KAPISI (issue #96) — pay, ölçen kuralın SINIFINA bakmıyordu.
    # Sert bir Spec hükmü, arkasındaki kuralların hepsi Quality/Interop iken "kanıtlı"
    # sayılabiliyordu; Spec'e filtreleyen kullanıcı ya da R1 görünümünü okuyan biri o ihlali
    # HİÇ görmüyordu. 2026-08-09 taraması 21 böyle satır buldu (issue 14 demişti ve alt sınır
    # olduğunu kendisi yazıyordu — düzyazıdan kural adı çıkarmak retorik atıfları da sayıyor).
    # ⚠️ Kapı DÜZYAZIYI OKUMAZ: kanıt `provision_evidence.tsv`'dedir. Gerekçe, issue'nun
    # kendi bulgusu — defterde on satır `PTH_017`'yi "bu hatayı tekrarlama" DERSİ olarak anar
    # ve düzyazı taraması onu kanıt sanıyordu. Kanıt beyan edilir, çıkarılmaz.
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

    # 🔐 Kanıt fingerprint kapısı: kural kimliği veya sınıfı değiştiğinde eski
    # adjudication sessizce geçerli kalmasın. Bu yalnız BAYATLIĞI yakalar; fingerprint
    # semantic coverage kanıtı değildir (bir kuralın hükmün tamamını ölçüp ölçmediğini
    # insan adjudication'ı belirler). Severity bilerek fingerprint'e dahil edilmez.
    stale.extend(
        check_evidence_fingerprints(
            hard,
            prov_ev,
            rule_class,
            ROOT / "provision_evidence.lock",
        )
    )

    # 🔴 KANIT BELGESİNİN §0 SAYILARI DA BU HESAPTAN TÜRER — ikinci kapı.
    # 2026-08-07: kullanıcı iki bayat sayı yakaladı (İKİNCİ kez elle yakalıyordu).
    # §0 "184 sert hükmün 25'i KAPSAM DIŞI, 13'ü META; 184 − 38 = 146" diyordu; o gün
    # hesap 24 / 13 / 184 − 37 = 147'ydi. Defter başlığı kapısı bunu göremezdi çünkü
    # BAŞKA DOSYAYA bakıyor. Sayının hesaplandığı yer burası, kapı da burada olmalı.
    # ⚠️ Kapı KORUDUĞU SATIRA çapalanır (defter başlığı dersi): §0'ın 4. maddesi ve
    # 3. maddesi ayrı ayrı bulunur, "dosyada geçiyor mu" denmez.
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
        # ⚠️ Sayı ETİKETE çapalanır. "satırda geçiyor mu" demek yetmezdi: aritmetikteki
        # bir rakam (ör. 37) tesadüfen aranan sayıya eşit olsaydı kontrol boşa geçerdi.
        # `\S*` Türkçe ekin ("24'ü") biçimini serbest bırakır, sayıyı bırakmaz.
        if not re.search(rf"{want}\S*\s+(?:\*\*)?{label}", arith_text):
            stale.append(f"§0/4 {label} sayısı {want} değil → {arith_text.strip()[:90]}")

    # §1 tablosundaki oran da hesaptan türer (2026-08-07'de 147/147'de kalmıştı).
    axis_line = next((l for l in ev_lines if "Düzyazı hükümleri" in l), "")
    if f"{n} / {d}" not in axis_line and f"{n}/{d}" not in axis_line:
        stale.append(f"§1 eksen satırı {n}/{d} demiyor → {axis_line.strip()[:90]}")

    # Korpus büyüklüğünün TEK KAYNAĞI manifesttir; §0/3 ondan sapmamalı.
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
    # ⚠️ ORPHAN VARSA ÇIKIŞ KODU 1 — uyarı basıp 0 ile çıkmak KAPI DEĞİL, TAVSİYEDİR.
    # 2026-08-06 dış denetimi bunu yakaladı: belge "yüzdeyi geçersiz sayar" diyordu ama
    # betik yine de yüzdeyi basıp başarıyla çıkıyordu → insan yorumuna bağlı bir "kapı".
    return 1 if orphans else 0


if __name__ == "__main__":
    raise SystemExit(main())
