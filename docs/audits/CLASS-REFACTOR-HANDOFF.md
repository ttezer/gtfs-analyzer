# Sınıf Otorite Refactor — DEVİR DOSYASI (Faz 3 devam)

Bu dosya, sınıf-otorite bütünlüğü refactor'ünü **başka bir AI'nın devralması** için kendi-yeterli
talimattır. Claude'un kişisel hafızasına erişimin yok; ihtiyacın olan HER ŞEY repo'da. Sırayla oku.

---

## 0) ÖNCE OKU (bu sırayla)
1. `docs/audits/spec-authority-inventory-reconciled.csv` — **SSOT karar defteri** (278 Spec kuralı, iki-AI uzlaşısı, hedef sınıf/authority/round2).
2. `docs/audits/spec-authority-inventory-reconciled.md` — defterin özeti + gerekçeler.
3. **Bu dosya** — prosedür + uyarılar + kalan worklist + ilerleme günlüğü.
4. `crates/rules/src/registry.rs` — `RULES` (r! makro), `AUTHORITY` tablosu, `SPEC_AUTHORITY_ALLOWLIST` (tests mod'da), `blocker_rules_have_r1` / `spec_class_requires_gtfs_spec_authority` / `authority_covers_all_rules` testleri.
5. `crates/core/src/enums.rs` — `AuthoritySource` enum (8 değer), `RuleClass`.
6. `docs/rules/TEMPLATE.md` — kart künye formatı (`Otorite kaynağı` satırı dahil).

## 1) MANDATE (değiştirilemez)
`Spec` sınıfı bir **otorite iddiasıdır** = "resmi GTFS Schedule Reference bunu açıkça zorunlu/yasak/geçersiz kılar".
Yalnızca `authority_source = GTFS_SPEC` (açık normatif hüküm: required / conditionally required / conditionally
forbidden / enum / foreign-key / uniqueness / format) `Spec` olabilir. Şüpheli → Spec DEĞİL (kanıt yükü Spec'te).
- MD/Google/profil → **Interop**; best-practice/veri-kalitesi → **Quality**; istatistik/operasyonel → **Analytics**.
- **R1 yayın-engeli YALNIZ** `GtfsSpec + Spec + Kritik`. Interop/Quality/Analytics ASLA R1 değil.

## 2) ŞU ANA KADAR YAPILAN (commit sırası, hepsi local, PUSH YOK)
`bd2baee` SHP_007 sil → `fb53c95` Faz1 audit → `4ad456d` Faz2 altyapı (AuthoritySource enum + AUTHORITY tablosu
537 kayıt + warn-mode gate) → `2942291` README en/ja → `d52c722` GTFS_GURU_PARITY kaldırıldı → `7f48e28` batch1 →
`bfed140` batch2 → `3307222` batch3 → `d2988a9` batch4 → `cc37aa2` MD-doğrulama düzeltmesi.
**allowlist 45 → 28** (17 kural yeniden sınıflandı). Bitmiş kurallar:
- Quality: SHP_006, RTS_009, TRF_018, FRQ_005, TFR_004, RCT_005 (+PDW_006/XFL_006 Analytics).
- Interop (MD-teyitli): STM_007, STM_008, CAL_005, BKR_011, STM_034, STM_038, TFR_005.
- Analytics: PDW_006, XFL_006.

## 3) DEĞİŞMEZLER / mimari (BOZMA)
- **`blocks[]` nedensel semptom gruplaması — DOKUNMA.** Otorite değil, nedensellik (A kırıksa B'yi maskeler).
- **Severity/class REGISTRY-DRIVEN:** notice'lar `meta.severity`/`meta.rule_class`'ı registry'den basar (k1/k3/k4/k5/k6).
  Yeniden sınıflandırma için **emit koduna dokunma** — sadece `registry.rs`.
- **`AUTHORITY` tablosu zaten dolu** (537 kayıt, ledger'dan). Genelde doğru ama **DEFAULT'LARI GÜVENİLMEZ** (bkz §5).
- **Gate warn-modu:** `spec_class_requires_gtfs_spec_authority` testi `Sınıf==Spec && authority!=GtfsSpec` olanları
  `SPEC_AUTHORITY_ALLOWLIST`'e karşı kontrol eder. Bir kuralı Spec'ten çıkarınca allowlist'ten de ÇIKAR. Liste
  Faz 4'te boşalır (hard-fail). **Listeye YENİ EKLEME YAPMA.**

## 4) HER KURAL İÇİN PROSEDÜR (adım adım)
1. **registry.rs `r!(...)`:** `Sınıf` (Spec→hedef) + `report_views` düzelt. Eşleme:
   Quality→`VS`, Interop→`VI` (R8 ekler), Analytics→`VA` (R3 ekler). (Erişilebilirlik varyantları: VS_ACC/VA_ACC.)
   Severity'yi KORU (kural gerçek; yalnız sınıf yanlıştı) — istisna: proje-özel bir kural Kritik/Spec+R1 idiyse
   ve artık non-spec ise Orta'ya çekmek makul (bkz SHP_006 emsali).
2. **AUTHORITY tablosu:** girdiyi hedef authority'ye getir (Quality→ProjectQuality/GtfsBestPractice,
   Interop→MobilitydataParity, Analytics→ProjectAnalytics, JPN→RegionalProfile). Çoğu zaten doğru; DOĞRULA.
3. **allowlist:** kuralı `SPEC_AUTHORITY_ALLOWLIST`'ten çıkar + baştaki yorumdaki sayacı güncelle.
4. **Kart** `docs/rules/<GRP>/<ID>.md`: künye `Sınıf` + `Görünürlük` (registry ile BİREBİR eşleşmeli, yoksa
   `card_consistency` kırılır) + `Otorite kaynağı` satırı; `Karar:` satırındaki sınıf; kendi komşu-tablo satırı
   (`· Spec ·`→`· <hedef> ·`); "Skora katkı"/R1 satırı; "GTFS spec referansı" ve "Dış araç eşleşmesi"
   metninde YANLIŞ spec/parite iddialarını düzelt ("proje-özel Spec"→"proje-özel kalite/analitik" vb.).
   Komşu tablodaki BAŞKA kuralların satırına dokunma (onlar kendi sıralarında düzelir).
5. `cargo run --example gen_rules` (RULES.md×3 üretir) → `cargo run --example sync_cards` (kart `.rs#L` satır-ref
   günceller; "AGN_001 çözülemedi" uyarıları ÖNCEDEN VAR, önemsiz).
6. `cargo test --workspace` → **exit 0 olmalı** (card_consistency Sınıf/Görünürlük/başlık + gate + drift).
7. **commit** (İngilizce, imzasız — public repo hijyeni). Küçük partiler (3-6 kural) halinde.

## 5) ⚠️ KRİTİK UYARILAR
- **INTEROP ATAMADAN ÖNCE MD'Yİ DOĞRULA.** Ledger'ın `MOBILITYDATA_PARITY` default'ları GÜVENİLMEZ: FRQ_005/
  TFR_004/RCT_005 "MD paritesi var" sanılıp Interop yapılmış, gerçekte YOK → Quality'ye düzeltildi (`cc37aa2`).
  Her Interop adayı için `gtfs-validator.mobilitydata.org/rules.html`'i fetch et, **birebir MD notice adı** ara.
  Parite VARSA Interop (kartta tam MD adını yaz); YOKSA Quality. Kartın "Dış araç eşleşmesi" metnini oku — çoğu
  zaten "MD karşılığı tespit edilmedi" diyor (o zaman Quality).
- **round2=true** kurallar: hedef sınıf provisional (anchor/parite belirsiz). Emin değilsen güvenli iniş **Quality**;
  ASLA Spec bırakma. Açık anlaşmazlıklar (anchor avı gerek): ATR_001 (attribution_id spec'te Optional mı?),
  ATR_009 (phone format normu yok), PTH_011/014 (türetilmiş graf), STP_027 (stop_access).
- **ARC_029 = FatalCode** (zip-bomb koruması, `k1_parse.rs` fatal yolu). Normal notice değil; class değişiminin
  fatal-yolu etkisi belirsiz → **en sona bırak, dikkatli test et** (veya kullanıcıya sor).
- **JPN_*** (5 kural) → `RegionalProfile` authority + `Interop` sınıf (GTFS-JP resmi Schedule değil; enum'da
  `RegionalProfile` var). AUTHORITY tablosunda "REGIONAL_PROFILE(öneri)..." gibi kirli string OLABİLİR — temiz
  `RegionalProfile` enum'una çek.
- **R1 test:** `blocker_rules_have_r1` yeni değişmez = "R1 yalnız Spec+Kritik". Interop'u R1'den çıkardık. Tam
  kod-düzeyi R1 (is_pub_relevant/build_r1) = **Faz 4**.
- **Skor kayar** her partide (Spec 0.4→Quality 0.2/Interop 0.3/Analytics 0.1). **Kullanıcı canlıda doğrular.**
  Gerçek-feed/golden testi ÇALIŞTIRMA (kullanıcı yapar, token).
- **PUSH YAPMA.** Kullanıcı kararı: refactor tutarlı milestone'a (Faz 3 sonu/Faz 4) gelince toplu push +
  minor sürüm bump + release note ("eski↔yeni skor karşılaştırılamaz").
- Küçük-harf sonekli ID'ler var: **DQ_005b, DQ_005c** (regex kullanırsan `[A-Za-z0-9_]`).
- `docs/rules/README.md` amacı, `TEMPLATE.md` formatı; kural davranışı öğrenmenin ilk durağı ilgili karttır.

## 6) KALAN 28 WORKLIST (ledger hedefi; Interop olanları MUTLAKA MD-doğrula)
| ID | Hedef (ledger) | Not / MD-doğrula? |
|---|---|---|
| ARC_002 | Quality/GTFS_BEST_PRACTICE | UTF-8 "should" (spec fetch teyitli) |
| ARC_009 | Quality | boş dosya; round2 |
| ARC_015 | Quality | mükerrer başlık; round2 |
| ARC_019 | Quality | boş kolon adı; round2 |
| ARC_023 | Quality | nested zip; round2 |
| ARC_029 | Quality | **FatalCode — en sona, dikkat** |
| ATR_001 | Quality | attribution_id spec'te Optional? anchor-avı |
| ATR_009 | Quality | phone format normu yok |
| CLD_005 | Quality | makul-yıl heuristik |
| JPN_002/003/004/005/011 | Interop/**RegionalProfile** | GTFS-JP profil |
| PTH_011 | Quality | döngü (türetilmiş) |
| PTH_014 | Quality | istasyon sınırı (türetilmiş) |
| SHP_028 | Quality | aynı-dist farklı-koordinat |
| STM_023 | Quality | dosya satır sırası |
| STM_033 | Interop? | **MD-DOĞRULA** (`unusable_trip` WARNING var mı?) |
| STP_027 | Quality | stop_access; round2 |
| TRF_013/015/016/017/019 | Interop? | **MD-DOĞRULA** her biri (in-seat/transfer aile) |
| TRP_017 | Interop? | **MD-DOĞRULA** (freq trip stop_times) |
| TRP_019 | Quality | continuous+shape gerekliliği |
| XFL_002 | Interop? | **MD-DOĞRULA** (`missing_trip_edge`/trip no stop_times?) |

## 7) İLERLEME GÜNLÜĞÜ (buraya EKLE — her parti sonrası)
- 2026-07-09 · Claude · batch 1-4 + MD-doğrulama düzeltmesi (`cc37aa2`) · allowlist 45→28.
- 2026-07-09 · Codex · batch 5 ARC quality reclass (`ARC_002/009/015/019/023`) · allowlist 28→23 · fatal `ARC_029` sona bırakıldı.
- 2026-07-09 · Codex · batch 6 derived quality reclass (`CLD_005/PTH_011/PTH_014/SHP_028/STM_023/TRP_019`) · allowlist 23→17 · MD-doğrulama isteyenler sonraya bırakıldı.
- <tarih> · <ai> · <parti> · <commit> · <allowlist kaç kaldı> · <not>

## 8) SONRA (Faz 3 bitince)
- **Faz 4:** R1'i koda bağla (`is_pub_relevant`/`build_r1` → `authority==GtfsSpec`); `report_views` görünüm
  metadata'sı olur, R1 otoritesi olmaz. CI koşul #6 (Spec kuralı karta alan-düzeyi anchor `#field_x` zorunlu).
  allowlist boşalt → hard-fail. warn-mode kaldır.
- **Faz 5:** ~25 default-atanmış `MOBILITYDATA_PARITY` mevcut Interop kuralını MD'ye karşı tek tek doğrula
  (bkz Faz 5, plan). Doğrulanmayanı UNKNOWN/PROJECT_*'e çek.
