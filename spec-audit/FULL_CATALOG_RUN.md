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
`agent/validator-audit-all-mdb-20260817` dalı **silinmez**.

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

> Bu bölüm **her koşumdan sonra yeniden yazılır.** Aşağıdaki hâli **6. koşumun
> SONUCUDUR** (`32311885577`). Koşum öncesi tahmin tablosu git geçmişinde `ce321611`'dedir.

**ON YEDİ TAHMİNİN ON YEDİSİ TUTTU.** Doğrulama elle değil `verify_r6.py` ile yapıldı;
her satır artefakta karşı otomatik sınandı.

| değişiklik | tahmin | gerçekleşen |
|---|---|---|
| `STM_061` `mdb-1003` | 11 | **11** ✅ |
| `STM_061` `mdb-1004` | 20 | **20** ✅ |
| `STM_061` `mdb-3401` | 43 | **43** ✅ |
| `STM_061` ateşleyen feed | 331 (daralmamalı) | **331** ✅ |
| `STM_016` `mdb-3401` | 0 | **0** ✅ |
| `STM_015`/`STM_016` `mdb-2727` | 0 / 0 | **0 / 0** ✅ |
| 🔴 `STM_003` `mdb-3401` | 218.087 SABİT | **218.087** ✅ |
| 🔴 `STM_003` `mdb-2727` | 203.242 SABİT | **203.242** ✅ |
| `analyzer_spec_unmapped` | ~9 | **8** ✅ |
| `md_mapped_missing` | ~12 | **12** ✅ |
| `analyzer_mapped_md_absent` | ~2.196 | **2.196** ✅ |
| `md_unmapped` · kural · feed | 0 · 422 · 4.271 | aynı ✅ |

🔑 **İki `STM_003` satırı bu tablonun en önemli kısmıdır.** `TIME_MALFORMED` düzeltmesi
`STM_015/016`'yı, değeri DOLU ama `HH:MM` yazılmış satırlarda susturur; aynı satırlarda
gerçek kusuru `STM_003` bildirir. `STM_003` de düşseydi, 214.000 bulguluk azalma
"iyileşme" diye okunurken aslında DOĞRU kuralın susturulmasını gizlerdi. **Tek bulgu
oynamadı.** `STM_015` 195, `STM_016` 150 bulgu KORUDU — bunlar gerçekten boş alanlar.

### Toplam: −274.835 ve hiçbiri kalite ölçüsü DEĞİL

38.620.548 → 38.345.713, korpus değişmedi (manifest bayt bayt aynı).

| kaynak | değişim | ne anlama geliyor |
|---|---:|---|
| `STM_015` + `STM_016` | **−214.346** | doğru olmayan bir iddianın geri çekilmesi |
| `STM_061` | **−78.429** | aynı kusurun sefer başına değil çift başına sayılması |
| tarih (bir gün ileri) + feed içeriği | ~+17.900 | `DQ_018` +10.067, `STM_036` +1.748, `VAT_002` +1.468 |

**Feed sayıları düşmedi** — `STM_061` 331 feed'de kaldı, ateşleyen kural 422'de.

### Kalan sapma — üçü de küçük ve adı belli

- `analyzer_spec_unmapped` **8 satır**: yalnız `TRN_011` (6) ve `TRN_015` (2), ikisi de
  açık nöbet.
- `md_mapped_missing` **12 satır**: `empty_file` 6, `invalid_date`/`invalid_integer`/
  `invalid_timezone` 2'şer. `fast_travel_between_far_stops` TAMAMEN GİTTİ.
- `analyzer_mapped_md_absent` **2.196** → koşum sonrası hükümlerle **922**
  (`reaggregated/`).

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
