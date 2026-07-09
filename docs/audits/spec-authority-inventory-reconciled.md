# Spec Otorite Denetimi — Birleşik Final Defter (Faz 1 kapanışı)

**Tarih:** 2026-07-09 · **Base:** Claude 278-satır güncel pas (`spec-authority-inventory.csv`)
· **İkinci görüş:** diğer AI 253-satır pas (`spec-authority-inventory-independent-pass.csv`)
· `SHP_007` diğer AI'da stale registry'den gelmiş; bizde silindi (bd2baee) → uzlaşma dışı.
Ham defter: `spec-authority-inventory-reconciled.csv` (15 kolon).

## Uzlaşılan dağılım (278 Spec-etiketli)
| Sonuç | Sayı |
|---|---|
| **Spec KESİN** (GTFS_SPEC / HIGH / round2 yok) | **219** |
| **Spec PROVISIONAL** (round2 — anchor ispatı gerek) | **14** |
| **Spec DIŞI** | **45** (Quality 18 · Interop 22 · Analytics 5) |
| round2_required toplam | 45 |

**Net okuma:** Spec etiketlerinin **~%79'u kesin haklı** (required/enum/FK/uniqueness/format).
Temizlik yüzeyi 45 Spec-dışı + 45 round2 (çoğu MEDIUM demote). Proje "çöp" değildi — hedefli kalibrasyon.

## R1 yayın-engeli değişimi
- **korunur: 149** (GTFS_SPEC + Spec + Kritik).
- **KALKAR: 12** → `ARC_002, ARC_009, ARC_015, ARC_029, CAL_005, CLD_005, FRQ_005, PTH_014, SHP_006, STM_008, STM_023, TRF_013`.
- **round2'ye bağlı: 4** (provisional-Spec, eski R1) → `DQ_021, FPD_001, TRF_005, TRN_006`.
- **En kritik: STM_008** (duraklar arası zaman geri) — şu an Kritik+Spec+R1; spec fetch **açık zaman-monotonluk normu YOK** dedi → Interop'a indi, R1 kalktı. Büyük skor/R1 etkisi.

## Spec metniyle KESİN çözülenler (gtfs.org fetch, 2026-07-09)
- **ARC_002 → Spec DIŞI:** UTF-8 spec'te **"should"** (must değil) → Quality/best-practice.
- **ARC_024 → Spec:** *"files **must** reside at the root level, not in a subfolder"* → GTFS_SPEC.
- **STP_026 → Spec:** stop_access alanı stops.txt'te var (enum) → GTFS_SPEC.
- **TRP_032 → Spec:** cars_allowed alanı trips.txt'te var (enum 0/1/2) → GTFS_SPEC.
- **Sıralama/monotonluk kümesi → Spec DIŞI (açık cümle yok):** `STM_007, STM_008, CAL_005, FRQ_005, TFR_004, RCT_005, BKR_011, STM_034, STM_038`.

## round2_required = true (Faz 4'te anchor bulunamazsa Spec dışı)
14 provisional-Spec + 31 MEDIUM-demote. Öne çıkan açık anlaşmazlıklar (iki AI farklı, kanıt yok):
`ATR_001` (attribution_id Optional mı?), `ATR_009` (phone format), `PTH_011` (döngü),
`PTH_014` (istasyon sınırı), `STP_027` (pathway stop_access). Ayrıca bileşik-PK/koşullu:
`FPD_001` (fare_products bileşik PK — over-fire riski), `XFL_020/021`, `TRF_005/014`,
`TRN_006/010/011/013`, `RCT_006`, `PTH_017`, `XFL_016`, `DQ_021`.

## TASARIM KARARI — GTFS-JP (JPN_002/003/004/005/011)
GTFS-JP **resmi GTFS Schedule Reference değil** → `GTFS_SPEC` olamaz. Seçenek:
1. `AuthoritySource` enum'a **`REGIONAL_PROFILE`** ekle (JP kurallarını ayrı otoriteyle koru) — önerilen.
2. Eklemezsek geçici `PROJECT_INTEROP`/`PROJECT_QUALITY` kovası.
Defterde şimdilik `Interop / REGIONAL_PROFILE(öneri) / MEDIUM / round2`. **Karar Faz 2 enum tasarımında verilecek.**

## Kapsam boşluğu — diğer AI'ın işlemediği 26 kural
Diğer AI pası tüm `frequencies.txt` (FRQ_001-008/011) + `transfers.txt` (TRF_001-019) + `JPN_011`
+ `SHP_028`'i **hiç işlememiş**. Bunlarda tek adjudication Claude'un; çoğu Spec/HIGH (FK/required/enum).
İkinci tur isterse diğer AI bunlara bakmalı; ama uzlaşı defteri Claude kararıyla tamamlandı.

## Taşınan kısıt (Faz 2/4)
`blocks[]` nedensel semptom gruplamasına **dokunulmayacak**; yalnız **R1 / pub_score** source-aware
olacak (`is_pub_relevant`, `build_r1`, `pub_penalty`). Ayrım korunuyor.

## Kapanış
Bu defterle **Faz 1 kapanır.** Faz 2'ye (altyapı) ancak `spec-authority-inventory-reconciled.csv`
commit'lendikten sonra geçilecek. Bu fazda kod/registry/kart/locale/skor/R1/CI'a dokunulmadı.
