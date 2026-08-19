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

Açık denetim işi artık issue'larda: #162 · #163 · #164 · #165 · #166 · #167 · #168.
Bunların HİÇBİRİ yeni koşum GEREKTİRMEZ — hepsi eldeki
`audit-results/full-mdb-schedule-20260819/run-32197267205/` verisiyle çalışılır.

## 1. Hangi yığın

- `benchmark/audit_all/`
- `.github/workflows/benchmark-validator-audit-all-mdb.yml`

Eski `benchmark/auditall/` yığını #152'de emekliye ayrıldı — diriltme.
`agent/validator-audit-all-mdb-20260817` dalı **silinmez**.

## 2. Koşmadan önce güncellenecek şeyler

> ✅ **5. koşum için ikisi ZATEN HAZIR** (`0acf6129`): `--manifest` artık main'de kalıcı,
> `BENCH_DATE`/`MD_DATE` `20260819`'da. Koşum BAŞKA BİR GÜN yapılırsa tarihi çek.

2. Koşmadan önce güncellenecek üç şey

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

> Bu bölüm **her koşumdan sonra yeniden yazılır.** Aşağıdaki hâli **5. koşum** içindir;
> 4. koşumun (`32197267205`) kendi uyarı tablosu git geçmişinde `9e7746ba`'dan önceki
> sürümdedir.

**Bu koşumun tek gerekçesi: ALTI ürün değişikliğinin tahminini SINAMAK.** Keşif değil
(brief §0). Her satırın beklentisi önceden yazılıdır; tutmazsa düzeltme çalışmıyor demektir.

| değişiklik | commit | BEKLENEN |
|---|---|---|
| `TRP_019` continuous enum `{0,1}` → `{0,2,3}` | `983420f8` | **215.288 → çok daha az.** Yüklem 1'i (=sürekli servis YOK) dahil ediyordu. `tdg-83921` ve `tdg-81645` **0'a inmeli**, `ntd-50386` 24'te kalmalı. |
| `ARC_009` koşullu çift simetrisi | `39246a4a` | CRITICAL 104 → **~45**. Boş `calendar.txt` + dolu `calendar_dates.txt` olan 59 feed susmalı; `stop_times`/`routes`/`agency`/`stops` boş olanlar SUSMAMALI. |
| `STM_061` (YENİ, 600. kural) | `94cc4c06` | MD'nin `fast_travel_between_far_stops` verdiği ~372 feed'de ateşlemeli. `mdb-2946`≈116, `mdb-3235`≈36, `tld-4477`≈7. |
| `ARC_034` (YENİ) | `fcb19828` | `mdb-1003`/`mdb-1004`'te **6'şar**; aynı feed'lerde `AGN_004`/`AGN_005` **0'a inmeli**. |
| `BKR_025` (YENİ) | `a5860028` | `tdg-80694`, `tdg-84001` — `prior_notice` saatinde `00:00:00.0000000` gibi bozuk değer. |
| `BKR_002` Spec → Quality | `e7a1af8d` | Bulgu sayısı AYNI kalır; yalnız sınıf/önem değişir → R1 yayın engeli olmaktan çıkar. |

### Defter tarafı — aracın doğru okuduğunun sınavı

Bugün üç kovanın hükümleri yazıldı ve `aggregate.py` artık `fp_adjudication.tsv`'yi
okuyor (`706aed1d`). Beklenen:

| sınıf | 4. koşum | BEKLENEN |
|---|---:|---|
| `analyzer_spec_md_absent` | 251 | **~9** |
| `analyzer_spec_unmapped` | 1.187 | **~0** (95 eşleme + 18 `NO_MD_EQUIVALENT`) |
| `analyzer_mapped_md_absent` | 14.936 | **~4.900** |
| `md_mapped_over` + `md_mapped_under` | 1.054 | **~101** |
| `md_mapped_missing` | 36 | ~1 (`tfs-342` 403 veriyor) |
| `md_unmapped` | 0 | **0 kalmalı** |
| `adjudicated_divergence` | 9.641 | belirgin ARTAR |

🔴 **Bu düşüşlerin HİÇBİRİ ürün iyileşmesi DEĞİLDİR.** Yargılanmış sapmanın artık
yargılanmış görünmesidir. Raporda böyle yazılır.

⚠️ **`ARC_009` yine de `analyzer_mapped_md_absent`'ta görünecek** — `FALSE_POSITIVE_FIXED`
hükmü gerileme duyarlıdır ve koşum düzeltmeyi kanıtlayana kadar bilerek görünür kalır.
Görünmesi kusur DEĞİL, kapının çalıştığının işaretidir.

⚠️ **202 feed aynı SHA-256'yı paylaşıyor** (~%5). Her "N feed" rakamı bu kadar şişkindir.

## 6. Sonuçların yayımlanması

Ham sonuç setinin TAMAMI repoya commit edilir — depo sahibinin kararı, önceki iki koşumda
da böyle yapıldı. Küçültme, Release'e taşıma, dosya eleme YAPILMAZ.

- Hedef: `audit-results/full-mdb-schedule-<TARİH>/run-<RUN_ID>/`
- Mevcut run klasörleri **ezilmez**; yeni koşum yeni klasördür.
- `SOURCE.txt`: run ID · ürün commit SHA · `MD_VERSION` · koşum tarihi · katalog/manifest SHA.
- Ölçek: önceki koşum 17 dosya / 72,3 MB. Commit mesajında bunun bilinçli bir depo
  politikası olduğu tekrar belirtilir.

## 7. Referans — önceki koşumlar

| | run-31934698855 (08-16) | run-31981225727 (08-17) | run-32145833613 (08-18) |
|---|---:|---:|---:|
| denenen feed | 4.259 | 4.258 (`mdb-2904` dışlandı) | 4.271 / 4.476 katalog satırı |
| iki validatör de temiz | 4.215 | 4.214 | 4.229 |
| süre kapsaması | **0 / 4.259** 🔴 | 4.224/4.224 · 4.216/4.216 ✅ | 4.239/4.229 ✅ |
| `md_mapped_missing` | 4.706 | 43 | 39 |
| `md_unmapped` | 1.008 | 57 | 5 |
| `analyzer_spec_md_absent` | 134 | 141 | 254 (eşleme kaynaklı) |
| `adjudicated_divergence` | — | 9.595 | 9.644 |
| toplam Analyzer bulgusu | 45,5M | 46,9M | **40,3M** |
| medyan süre (Analyzer / MD) | ölçülmedi | 0,05 sn / 3,06 sn | 0,05 sn / 2,98 sn |
| p95 süre | ölçülmedi | 3,65 sn / 12,72 sn | 3,73 sn / 12,96 sn |
| aynı SHA-256 grubu | ölçülmedi | 101 grup / 206 feed | 99 grup / 202 feed |

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
