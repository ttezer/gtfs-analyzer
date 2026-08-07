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
