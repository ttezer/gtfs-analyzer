#!/usr/bin/env python3
"""GTFS Spec rozeti: düzyazı ekseninde tamamlanma yüzdesi.

Girdi `PROVISION_TRIAGE.md`'dir — karar etiketleri satırlardan, kapanışlar "DURUM MAKİNESİ"
bölümünden okunur. Yüzde ELLE hesaplanmaz.

⚠️ PAYDA SERT HÜKÜMLERDİR. Yumuşak (`should`) hükümler spec'in zorunlu kılmadığı şeylerdir;
onları "karşılanmamış spec hükmü" saymak PTH_017'nin hatasının defter düzeyinde tekrarıdır.
⚠️ KAPSAM DIŞI + META paydadan DÜŞER: tüketici davranışını bağlayan ya da tanım olan bir
cümleyi bir doğrulayıcının ölçmemesi eksiklik değil, tanım gereğidir.
"""
import collections, json, pathlib, re, sys

ROOT = pathlib.Path(__file__).resolve().parent
ORDER = ["BOŞLUK", "KAPSAM DIŞI", "KANITLI", "KISMİ", "DOLAYLI", "META"]


def main() -> int:
    prov = {r["id"]: r for r in json.loads((ROOT / "spec_provisions.json").read_text())["provisions"]}
    text = (ROOT / "PROVISION_TRIAGE.md").read_text()

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

    print(f"\n{'='*52}\nDÜZYAZI EKSENİ: {n}/{d} = %{100*n/d:.1f}")
    gaps = [k for k, v in hard.items() if v == "BOŞLUK"]
    for g in gaps:
        print(f"  kalan: {g}  {prov[g]['sentence'][:70]}")
    print("\n⚠️ Bu yüzde YALNIZ düzyazı eksenidir. Alan tablosu ekseni ayrı ölçülür")
    print("   (302 atomun 296'sı) ve iki eksen ÖRTÜŞÜR — toplanamazlar.")
    print("⚠️ Payda kataloğun kendi kapsamıdır: modalsiz hükümler görünmez (ölçüldü, boş çıktı),")
    print("   cümle bölme kabadır.")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
