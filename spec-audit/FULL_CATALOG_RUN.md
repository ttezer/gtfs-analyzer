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

> Bu bölüm **her koşumdan sonra yeniden yazılır.** Aşağıdaki hâli 4. koşum içindir;
> 3. koşumun (`32145833613`) kendi uyarı tablosu git geçmişinde `98f09bdf`'ten önceki
> sürümdedir.

3. koşumdan (`32145833613`) bu yana üründe **kural davranışı değiştiren bir şey yok**;
değişenler araç tarafında. Kıyas yaparken:

| değişiklik | beklenen etki |
|---|---|
| `classify_analyzer` artık 124/137'yi `partial_timeout` sayıyor (`a791a0d4`) | `mdb-2014` **`completed` olmaktan çıkar**; `both_completed_cleanly` 1 azalır ve o feed sapma kıyasından DÜŞER. Bu bir gerileme değil, önceki koşumun kesik çıktıyı temiz sayması kusurunun kapanmasıdır. |
| `point_near_pole` → `GEO_022` eşlendi (`d8b2820b`) | `md_unmapped` **5 → 3**'e iner; `analyzer_spec_md_absent` en fazla `GEO_022`'nin 4 feed'i kadar artabilir. Kapsam değişmedi, yalnız kitap düzeldi. |

⚠️ **`analyzer_spec_md_absent` (FP kovası) 254'ten geldi ve İÇİ YARGILANMADI.** 3. koşumda
141 → 254 çıkışının tamamı #147'nin eşlemelerinden geliyordu (`analyzer_spec_unmapped`
aynı anda 1.294 → 1.187 düştü). **Bu kovanın sayısı TEK BAŞINA asla ürün hükmü değildir —
komşu kovanın karşılık gelen düşüşüyle birlikte okunur.**

⚠️ **Agregasyon artık DOĞRULANMIŞ durumda (#151 kapandı).** `STM_056` ~1.200,
`STM_018` 9, `STM_019` 5, `STM_032` 45, `STM_007` 225 seviyelerinde OLMALI. Bunlar
milyonlara geri çıkarsa agregasyon kırılmış demektir.

⚠️ **202 feed aynı SHA-256'yı paylaşıyor** (korpusun ~%5'i). Her "N feed" rakamı bu kadar
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
