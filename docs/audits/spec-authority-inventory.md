# Spec Otorite Denetimi — Faz 1 Envanteri (Claude bağımsız pası)

**Tarih:** 2026-07-09 · **Kapsam:** registry'de `Sınıf = Spec` olan **278 kural**
**Yöntem:** her kural resmi GTFS Schedule Reference'a karşı yargılandı. Karar kuralı:
`confidence=HIGH + açık normatif anchor` → Spec kalır; `MEDIUM` → varsayılan Spec-dışı
(anchor bulunursa terfi, "incele"); `LOW` → asla Spec. MD/Guru/Google/best-practice/proje-özel
sezgi Spec kanıtı DEĞİL. Bu **Claude'un bağımsız pasıdır**; diğer AI'ın pasıyla yalnız
**çelişkili kararlar** uzlaştırılacak. Ham veri: `spec-authority-inventory.csv`.

## Manşet — durum sanılandan çok daha sağlıklı
| | Sayı | Oran |
|---|---|---|
| Spec-etiketli (mevcut) | 278 | — |
| **Spec KALIR** (haklı) | **235** | **%85** |
| ↳ HIGH (net normatif anchor) | 220 | — |
| ↳ MEDIUM (anchor doğrulaması gereken, incele) | 15 | — |
| **Spec DIŞI** (yeniden sınıflandır) | **43** | **%15** |

**Önemli düzeltme:** Daha önceki "doğru Spec ~40–80, ~200 uydurma" tahmini **YANLIŞTI**
(demoralize anında aşırı-düzeltme). Gerçek: GTFS Schedule Reference yüzlerce normatif kısıt
içerir (zorunlu alan, enum, foreign key, benzersizlik, format) ve bu validatör alan-düzeyinde
derin kapsar — dolayısıyla **~235 Spec etiketi gerçekten haklı.** Temizlik yüzeyi **43 downgrade
+ 15 incele = ~58 kural**; proje "çöp" değil, hedefli bir kalibrasyon.

## Spec DIŞI öneriler (43) — yeni sınıf dağılımı
- **Interop (23)** — çoğu MD-parite / zaman-monotonluk / türetilmiş aktarma mantığı:
  STM_007, STM_008, STM_033, STM_034, STM_038, CAL_005, FRQ_005, RCT_005, TFR_004, BKR_011,
  TRF_013/015/016/017/019, TRP_017, XFL_002, ARC_024, JPN_002/003/004/005/011.
  (GTFS-JP kuralları resmi Schedule Reference değil = profil; enum'a `REGIONAL_PROFILE` değeri gerekebilir.)
- **Quality (15)** — best-practice / proje-özel kalite / uzantı-alan:
  SHP_006, RTS_009, TRF_018, ARC_023, ARC_029, ATR_001, ATR_009, CLD_005, PTH_011, PTH_014,
  STM_023, STP_026, STP_027, TRP_032.
- **Analytics (5)** — türetilmiş operasyonel/istatistiksel:
  FRQ_011, PDW_006, TFR_005, TRP_022, XFL_006.

## R1 yayın-engelini KAYBEDEN kurallar (9)
Şu an R1-blocker (feed'i "yayınlanamaz" ilan eder) ama öneri Spec-dışı → R1'den çıkmalı:
`ARC_029, CAL_005, CLD_005, FRQ_005, PTH_014, SHP_006, STM_008, STM_023, TRF_013`.
**En dikkat çekici: STM_008** (duraklar arası zaman geri) — şu an Kritik+Spec+R1, ama spec'te
açık zaman-monotonluk normu YOK (yalnız MD ERROR paritesi). Interop'a inmesi büyük skor/R1
etkisi yaratır → çift-AI uzlaşısında öncelikli tartışma.

## MEDIUM-Spec (15) — Spec kalabilir AMA anchor doğrulaması şart
`ARC_009, FPD_001, FPD_006, PTH_017, RCT_006, TRF_005, TRF_014, TRN_006, TRN_010, TRN_011,
TRN_013, TRP_019, XFL_016, XFL_020, XFL_021`. Bunlar koşullu-required/forbidden veya bileşik-PK
vakaları; Faz 4'te tam anchor bulunmazsa Spec-dışına düşer. Örn. **FPD_001**: fare_products PK
bileşiktir (fare_product_id+rider_category_id+fare_media_id) — yalnız fare_product_id yinelenmesi
ihlal olmayabilir (olası over-fire).

## Reddedilen "Spec kanıtı" gerekçeleri (kayıt)
- Zaman-monotonluk (STM_007/008): güçlü mantık + MD ERROR, ama spec açık normatif cümle vermiyor.
- Minimum-nokta/durak eşikleri (SHP_006, STM_033): spec ≥N normu yok.
- Ordering (start>end: CAL_005, FRQ_005, TFR_004, RCT_005): spec alan tablosunda açık ordering yok.
- GTFS-JP (JPN_*): ulusal profil, resmi Schedule Reference değil.
- Uzantı/öneri alanlar (stop_access, cars_allowed): resmi Schedule'da yerleşik değil.
- Proje-özel güvenlik (ARC_029): GTFS normu değil, işleme koruması.

## Sonraki adım (Faz 1 kapanışı)
1. Diğer AI'ın bağımsız pası gelince: yalnız **çelişen kararları** (özellikle STM_007/008,
   TRF ailesi, JPN profili, MEDIUM-Spec 15'i) uzlaştır.
2. Uzlaşılan defter Faz 2/3'ün girdisidir. Bu fazda kod/registry/kart/locale/skor/R1/CI'a dokunulmadı.
