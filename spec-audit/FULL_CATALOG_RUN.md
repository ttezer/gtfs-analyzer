# Tam MobilityData kataloğu koşumu — koşum brief'i

Bu belge **koşumu yapacak olana verilir**. Her maddesi bir koşumda hataya mal oldu;
hiçbiri teorik değil.

> 📌 **Bu dosya iki kez yeniden yazıldı.** İlk hâli koşum ÖNCESİ bir plandı ve sayıları
> (3.358 katalog → 1.479 koşulabilir) gerçek katalogla tutmadı. İkinci hâli iki koşumun
> özetiydi. Bu hâli **üçüncü koşum için talimat**tır. Geçmiş koşumların sayıları aşağıda
> "Referans" bölümünde; **güncel sayı için koşum artefaktlarına bak, bu dosyaya değil.**

## 0. 🛑 NE ZAMAN KOŞULUR — VE NE ZAMAN KOŞULMAZ

**Karar (2026-08-19, kullanıcı): korpus artık KEŞİF için koşulmaz, yalnız DOĞRULAMA için koşulur.**

Dört koşum sonunda katalog bize söyleyeceğini söyledi. Sınıf sayıları ikinci ve üçüncü
koşumdan beri neredeyse sabit; hareket eden her şeyin sebebi ya ARAÇTI ya da bizim o tur
yaptığımız düzeltmeydi. 4. koşum bunun kanıtı: yedi düzeltmenin yedisi de önceden adı
verilen feed'de gerçekleşti ve başka hiçbir şey kıpırdamadı (gerçek ürün değişimi
4.271 feed'de −461 bulgu).

**Koşum ŞART olan tek durum:** elde, hangi feed'de ne değişeceğini ÖNCEDEN söyleyebildiğimiz
bir düzeltme yığını var ve bu tahmin sınanacak.

**Koşum GEREKMEYEN durumlar:** "bakalım ne çıkacak" · defter/eşleme değişikliği (bunlar artık
`LedgerDriftGate` ve `test_timing.py` ile YERELDE sınanır) · rapor tazeleme.

⚠️ **Bir koşum ~1 saat, 640 shard ve 79 MB ham veridir.** Keşif amaçlı koşum, o maliyeti
sıfır bilgi için ödemektir — ve daha kötüsü, ürün değişmemişken oynayan bir sayı
"iyileşme" ya da "gerileme" diye okunur. Dördüncü koşumda −%3,9'luk düşüşün tamamının
TEK BİR FEED'in doğru sınıflandırılmasından geldiğini hatırla.

**5. koşum bu kuralın hem kanıtı hem uyarısıdır.** Altı tahminin beşi adı verilen feed'de
isabet etti — koşum işini gördü. Ama defter sayıları OKUNAMADI (§5) ve bu, koşumdan önce
yerel bir testle yakalanabilirdi. 🔴 **Koşum tetiklemeden önce
`python3 benchmark/audit_all/test_timing.py` YEŞİL olmalıdır.**

Koşumdan sonra açık kalan denetim işi §5'in sonundadır ve **hiçbiri yeni koşum
GEREKTİRMEZ** — hepsi eldeki `run-32290410755/` verisiyle (defter tarafı için
`reaggregated/` ile) çalışılır.

## 1. Hangi yığın

- `benchmark/audit_all/`
- `.github/workflows/benchmark-validator-audit-all-mdb.yml`

Eski `benchmark/auditall/` yığını #152'de emekliye ayrıldı — diriltme.

> ✅ **`agent/validator-audit-all-mdb-20260817` koruma notu KALDIRILDI.** O dal, 3. koşumun
> (`31981225727`) ham verisini TEK BAŞINA taşıdığı için korunuyordu — sonuçlar main'e hiç
> alınmamıştı ve dal silinseydi 62 MB'lık koşum verisi kaybolacaktı. Veri artık
> `audit-results/full-mdb-schedule-20260817/` altında, diğer altı koşumla aynı yerde;
> dal silindi. **Ders: bir dalı "silme" diye işaretlemek, içindeki veriyi kalıcı yapmaz.**

## 2. Koşmadan önce güncellenecek şeyler

> ⚠️ `--manifest` `0acf6129`'dan beri main'de kalıcıdır ve 5. koşumda çalıştı. Ama
> `BENCH_DATE`/`MD_DATE` hâlâ `20260819`'dadır: **6. koşum başka bir gün yapılırsa tarih
> ÇEKİLMEZSE hata vermez, sessizce yanlış verdikt üretir.**

1. **`benchmark/audit_all/run_shard.py` → `BENCH_DATE` ve `MD_DATE`.**
   Gerçek koşum tarihine çek. `BENCH_DATE` Analyzer'ın `--today` bayrağına, `MD_DATE`
   MobilityData'nın `-d` bayrağına gider. Tarihe bağlı kurallar (`CAL_013`, `CAL_024`,
   `FIN_010`, `FIN_019`, `TRP_023`) bu değere göre ölçer.
   🔴 **Bayat tarih HATA VERMEZ, sessizce yanlış verdikt üretir.** Elle bakımlı bir sabit
   olduğu için tek koruması insan dikkatidir.
2. **Aggregate çağrısına `--manifest benchmark/audit_all/run/manifest.json` ekle.**
   Yoksa `coverage-gaps.json` `manifest_checked: false` der ve düşen bir shard fark edilmez.
3. **`MD_VERSION` `8.0.1`'de kalır.** Yükseltilirse sonuç önceki koşumlarla
   **kıyaslanamaz** ve rapor başlığında öyle yazılır.

## 3. Dokunulmayacaklar

- **`require_measured` (aggregate.py).** `measured column(s) came back wholly empty` ile
  patlarsa süre yakalama bozulmuş demektir → **yakalamayı düzelt, koşumu yayımlama.**
  O kapı, bir önceki koşumun 4.259 feed'i hiç süre verisi olmadan yayımlaması yüzünden var.
- **`EXCLUDED_FEEDS` + manifest sızıntı denetimi (`select_corpus.py`).** `mdb-2904`
  kalıcı talimatla dışlanır. Filtre silinirse denetim `SystemExit` ile patlar; bu kasıtlı.
- **Mükerrer feed'ler tekilleştirilmez.** SHA-256 ile tespit edilir ve RAPORLANIR.

## 4. Koşum sonrası beş sağlık kapısı — bu sırayla

1. **`coverage-gaps.json`** → `missing_manifest_feed_ids` BOŞ, `manifest_checked: true`.
2. **`summary.json`** → `analyzer_wall_s.n` ve `md_wall_s.n`, `both_completed_cleanly`
   değerine YAKIN olmalı. Fark %1'i geçerse süre yakalama kısmen bozuktur.
3. **`duplicate-content-groups.json`** → grup ve feed sayısı raporlanır.
4. **`divergence_candidate_counts`** → `md_mapped_missing` binlerdeyse parity eşlemesi ya da
   defter sorgusu bozulmuştur; bulguları triyaj etmeden ÖNCE araştır.
5. **`mdb-2014`** → `download_status=ok` (1,2 GB sınırı 2,5 GB'a çıkarıldı, özellikle bunun için).

Bu beşi geçmeden tek tek sapmalar için issue AÇILMAZ. Bozuk bir koşum, ikna edici görünen
sahte bulgular üretir.

## 5. 🔴 BU KOŞUMA ÖZEL — ÖNCEKİYLE KIYASLARKEN

> **7. koşumun SONUCU** (`32344419636`). Tahmin tablosu git geçmişinde `55c38f7c`'dedir.

### 🔴 EN ÖNEMLİ BULGU: MANİFEST AYNI ≠ FEED'LER AYNI

Altı koşumdur "manifest bayt bayt aynı, dolayısıyla korpus değişmedi" diye yazıldı.
**Bu yanlış.** Manifest aynı feed KİMLİKLERİNİN seçildiğini söyler; indirilen ARŞİVLERİN
aynı olduğunu söylemez. 6. ve 7. koşum arasında:

| | feed |
|---|---:|
| arşiv SHA-256 aynı | **~3.662** |
| arşiv yeniden yayımlanmış | **~577** |
| iki koşumda da indirilemeyen | 32 |

**Her regresyon iddiası aynı-arşiv altkümesine karşı yapılmalıdır.** Global toplamlar ürün
değişikliğiyle katalog kaymasını karıştırır. Önceki altı koşumun "değişmeyen korpus"
ifadeleri bu gözle yeniden okunmalıdır.

Örnek: `STM_061` feed sayısı global 331 → 332 görünüyor, ama aynı-arşiv altkümesinde
235 → 235 ve bulgu 3.579 → 3.579. O +1 ürün değil katalog.

### Tahminler — 20 tuttu, 2 tutmadı, 1'i yarı

| değişiklik | feed | tahmin | ölçüm |
|---|---|---|---|
| CSV tokenizer | `tdg-80973` | `RTS_031` 0 · `ARC_012` 0 · 🔴 `ARC_033` **1** | 0 · 0 · **1** ✅ |
| `ARC_034` | `mdb-1004` / `mdb-1003` | 8 / 6 | **9 / 7** ❌ |
| | `mdb-1004` `TRP_002` | 0 | **0** ✅ |
| `TRN_011` | `mdb-2126` | 0, 🔴 `TRN_001` **421** | 0 · **421** ✅ |
| | `jbda-shinjobankotsu…` | 0, 🔴 `TRN_001` **1.056** | 0 · **1.056** ✅ |
| `DQ_018` | `mdb-2389` / `mdb-2653` | 0 / **926** | 0 / **926** ✅ |
| `TRP_024` | `tdg-83634` / `tdg-81942` | 1.887 / **1.870** | 1.887 / **1.870** ✅ |
| `STP_009` | `mdb-2003` | 0 | **0** ✅ |
| `STM_034` | korpus | **değişmemeli** | **2.113 → 0** ❌ |
| ateşleyen kural | — | 422 | **417** ❌ |

🔑 **"SABİT kalmalı" satırlarının hepsi tuttu.** `ARC_033` `ARC_012` çökerken kıpırdamadı,
`TRN_001` `TRN_011` sıfırlanırken kıpırdamadı, `STM_003` `STM_034` sıfırlanırken 218.087'de
kaldı. Her vakada yanlış kural sustu, doğru kural konuşmaya devam etti.

**`ARC_034` 9/7:** tahmin `calendar_dates.txt` dördüncü stream yolu olarak bağlanmadan önce
yazılmıştı. Dosya düzeyinde doğrulandı — o dosya gerçekten başlık tekrarı taşıyor.

**`STM_034` 2.113 → 0:** "önleyici, korpusta etki yok" tahmini ÖLÇÜLMEDEN yazılmıştı.
Düzeltme doğru çalışıyor; tahmin yanlıştı.

**417 ateşleyen kural:** altı kuralın dördü bu turun kendi düzeltmeleri (`TRN_011`,
`TRN_015` ve `ARC_034` türevleri `STM_005`, `TRP_006`), ikisi feed kayması (`mdb-1127`).
Yeni ateşleyen: `STP_036` — aşağıya bakın.

### 🔴 BU KOŞUMUN YAKALADIĞI REGRESYON

`tfs-535`'te `STP_036` **0 → 14**, arşiv SHA'sı DEĞİŞMEDEN. Sebep: `STP_009` düzeltmesi
`parent` değişkenini ham değere çevirdi, ama aynı bloktaki **üç `is_empty()` kontrolü
çevrilmedi**; tek boşluk taşıyan `parent_station` "dolu" sayıldı.

Düzeltildi: **DOLULUK daima `trim()`, KİMLİK daima ham.** `tfs-535` aynı SHA'da 14 → 0,
`mdb-2003` hâlâ 0. İki regresyon testi iki yönü de sabitliyor.

⚠️ Bu, koşumun tek başına haklı çıkardığı sonuçtur: dört feed'de yerel doğrulama bunu
görmedi, çünkü hiçbirinde boşluklu `parent_station` yoktu.

### Toplam: −101.639 ve bu sayı KULLANILAMAZ

38.345.713 → 38.244.074. Ayrıştırınca: aynı-arşiv feed'lerde **−222.802**, yeniden
yayımlanmış feed'lerde **+121.163**. Net rakamı "101 bin hata azalttık" diye okumak
her iki yönde de yanlıştır.

## 6. Sonuçların yayımlanması

Ham sonuç setinin TAMAMI repoya commit edilir — depo sahibinin kararı, önceki iki koşumda
da böyle yapıldı. Küçültme, Release'e taşıma, dosya eleme YAPILMAZ.

- Hedef: `audit-results/full-mdb-schedule-<TARİH>/run-<RUN_ID>/`
- Mevcut run klasörleri **ezilmez**; yeni koşum yeni klasördür.
- `SOURCE.txt`: run ID · ürün commit SHA · `MD_VERSION` · koşum tarihi · katalog/manifest SHA.
- Ölçek: önceki koşum 17 dosya / 72,3 MB. Commit mesajında bunun bilinçli bir depo
  politikası olduğu tekrar belirtilir.

## 7. Referans — önceki koşumlar

| | run-32145833613 (08-18) | run-32197267205 (08-19) | run-32290410755 (08-19) | run-32311885577 (08-20) |
|---|---:|---:|---:|---:|
| denenen feed | 4.271 | 4.271 | 4.271 | 4.271 |
| iki validatör de temiz | 4.229 | 4.228 | 4.222 | 4.229 |
| süre kapsaması | 4.239/4.229 ✅ | ✅ | 4.231/4.222 ✅ | 4.238/4.229 ✅ |
| `md_mapped_missing` | 39 | 36 | 133 → 12 † | **12** |
| `md_unmapped` | 5 | 0 | 0 | **0** |
| `analyzer_spec_unmapped` | 1.187 | 1.187 | 327 † | **8** |
| `analyzer_spec_md_absent` | 254 | 251 | 573 † | 565 |
| `analyzer_mapped_md_absent` | — | 14.936 | 924 † | **2.196** (hükümlerle 922) |
| `adjudicated_divergence` | 9.644 | 9.641 | 24.598 † | **23.353** |
| toplam Analyzer bulgusu | 40,3M | 38,70M | 38,62M | **38,35M** |
| medyan süre (Analyzer / MD) | 0,05 sn / 2,98 sn | 0,05 sn / 3,02 sn | 0,05 sn / 2,99 sn | ölçüldü |
| aynı SHA-256 grubu | 99 grup / 202 feed | 99 grup / 202 feed | 100 grup / 204 feed | 101 grup / 206 feed |
| tahmin isabeti | — | 7/7 | 5/6 | **17/17** |

† 5. koşumun defter sayıları yayımlandığı hâliyle yanlıştı ve buradaki değerler o günün
sonunda yazılan hükümlerle YENİDEN TOPLANMIŞ hâlidir — koşum tekrarlanmadı. Bu yüzden
5. ve 6. koşum defter sütunları doğrudan kıyaslanamaz: 5. sütun bugünün defteriyle,
6. sütun koşum ANINDAKİ defterle hesaplanmıştır.

🔑 **4. koşumun dersi: YEDİ düzeltMENİN YEDİSİ de önceden adı verilen feed'de isabet etti.**
Toplam 40,3M → 38,7M (−%14,1 değil, −%3,9) bir kalite ölçüsü DEĞİLDİR: düşüşün TAMAMI
`mdb-2014`'ün doğru sınıflandırılmasıdır (o feed 3. koşumda `completed`+1,55M bulgu
sayılıyordu). 4.271 feed'de gerçek ürün değişimi **−461**.

🔑 **3. koşumun dersi: agregasyon ÖLÇÜLDÜ ve tahminler feed başına birebir tuttu.** Toplam
46,9M → 40,3M (−%14,1) bir kalite ölçüsü DEĞİLDİR: agregasyonun eksilttiği (−8,93M) artı
zaman aşımından kurtulan iki feed'in eklediğidir (+2.002.996). `mdb-2727` 300 sn → 7,92 sn,
`mdb-3401` 300 sn → 11,75 sn. Aynı içerikli 589 feed'de medyan süre kayması −%0,5.

🔑 **İki koşum arasında katalog neredeyse değişmedi** — ortak 4.258 feed'in 4.185'i bayt
bayt aynıydı. MobilityData kontrol grubudur (kodu sabit): toplamı düz kaldı, bizimki
45,5M → 46,9M çıktı. Fark ürün değişikliğidir, katalog kayması değil.

⚠️ **4.706 → 43 düşüşünde ÜRÜN neredeyse hiç pay sahibi değildi.** Fark, aracın bağlam
çözebilmesi ve karar defterlerini okuyabilmesiydi (#146, #148). "Körlüğümüz" diye
raporlananın %74'ü zaten yargılanmıştı.

## 8. İlgili dosyalar

- Kural bazlı hükümler: `spec-audit/fp_adjudication.tsv` (`run_id` kolonuyla ayrık)
- Karar defterleri: `spec-audit/md_parity_mapping.py` → `classify_divergence()` TEK çağrı
- Artefaktlar: `audit-results/full-mdb-schedule-*/run-*/`
