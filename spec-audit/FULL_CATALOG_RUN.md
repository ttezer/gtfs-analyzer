# Tam MobilityData kataloğu koşumu — koşum brief'i

Bu belge **koşumu yapacak olana verilir**. Her maddesi bir koşumda hataya mal oldu;
hiçbiri teorik değil.

> 📌 **Bu dosya iki kez yeniden yazıldı.** İlk hâli koşum ÖNCESİ bir plandı ve sayıları
> (3.358 katalog → 1.479 koşulabilir) gerçek katalogla tutmadı. İkinci hâli iki koşumun
> özetiydi. Bu hâli **üçüncü koşum için talimat**tır. Geçmiş koşumların sayıları aşağıda
> "Referans" bölümünde; **güncel sayı için koşum artefaktlarına bak, bu dosyaya değil.**

## 1. Hangi yığın

- `benchmark/audit_all/`
- `.github/workflows/benchmark-validator-audit-all-mdb.yml`

Eski `benchmark/auditall/` yığını #152'de emekliye ayrıldı — diriltme.
`agent/validator-audit-all-mdb-20260817` dalı **silinmez**.

## 2. Koşmadan önce güncellenecek üç şey

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

Önceki koşumdan (`31981225727`) bu yana ürün ciddi değişti. **Bulgu sayısındaki düşüş ürün
iyileşmesi DEĞİLDİR**, aşağıdakilerin toplamıdır:

| değişiklik | beklenen etki |
|---|---|
| 5 kural agregat oldu (`STM_007/018/019/032/056`) | **~%12 toplam bulgu düşüşü** |
| ↳ `STM_056` | 3.297.317 → **~1.251** olmalı |
| ↳ `STM_018`/`STM_019` | 1.743.530 / 1.743.526 → **9 / 5** |
| K7 indeksleme | `mdb-2727` ve `mdb-3401` artık zaman aşımına UĞRAMAMALI |
| `STM_015`/`STM_016` kaskad bastırma | R9 item sayısı düşer, ham bulgu DEĞİŞMEZ |
| `RTS_031`, `FLG_008` (yeni kural) | küçük artış |
| IDN kabulü, koordinat kurtarma, `FIN_019` sınırı | karışık |

⚠️ **Agregasyonun çalıştığı İLK KEZ bu koşumda ölçülecek.** Etkilenen feed'lerin URL'leri
403 verdiği için şu ana kadar hiç gözlenemedi; eldeki R5 rakamları kayıtlı veriden
HESAPLANDI. `STM_056` beklenen seviyeye inmezse agregasyon çalışmıyor demektir (#151).

⚠️ **206 feed aynı SHA-256'yı paylaşıyor** (korpusun ~%5'i). Her "N feed" rakamı bu kadar
şişkindir; kıyas yaparken belirt.

## 6. Sonuçların yayımlanması

Ham sonuç setinin TAMAMI repoya commit edilir — depo sahibinin kararı, önceki iki koşumda
da böyle yapıldı. Küçültme, Release'e taşıma, dosya eleme YAPILMAZ.

- Hedef: `audit-results/full-mdb-schedule-<TARİH>/run-<RUN_ID>/`
- Mevcut run klasörleri **ezilmez**; yeni koşum yeni klasördür.
- `SOURCE.txt`: run ID · ürün commit SHA · `MD_VERSION` · koşum tarihi · katalog/manifest SHA.
- Ölçek: önceki koşum 17 dosya / 72,3 MB. Commit mesajında bunun bilinçli bir depo
  politikası olduğu tekrar belirtilir.

## 7. Referans — önceki koşumlar

| | run-31934698855 (08-16) | run-31981225727 (08-17) |
|---|---:|---:|
| denenen feed | 4.259 | 4.258 (`mdb-2904` dışlandı) |
| iki validatör de temiz | 4.215 | 4.214 |
| süre kapsaması | **0 / 4.259** 🔴 | 4.224/4.224 · 4.216/4.216 ✅ |
| `md_mapped_missing` | 4.706 | 43 |
| `md_unmapped` | 1.008 | 57 |
| `adjudicated_divergence` | — | 9.595 |
| medyan süre (Analyzer / MD) | ölçülmedi | 0,05 sn / 3,06 sn |
| p95 süre | ölçülmedi | 3,65 sn / 12,72 sn |
| aynı SHA-256 grubu | ölçülmedi | 101 grup / 206 feed |

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
