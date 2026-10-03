# GTFS Validator Phase 3 Benchmark — 2026-10-03

Bu ölçüm, gerçek MCP akışına yakın olarak ZIP'i parça parça stdin'e aktararak
`gtfs-analyzer validate - --compact-json --today 20260515` komutuyla yapıldı.
Değerler tek koşumluk yerel macOS ölçümüdür; production kapasite garantisi
değildir.

| Feed | Sıkıştırılmış ZIP | Stop | Route | Trip | Notice | Kural | Süre | Peak RSS | Compact çıktı | Exit/status |
|---|---:|---:|---:|---:|---:|---:|---:|---:|---:|---|
| `ui/tests/fixtures/minimal.zip` | 1,056 B | 2 | 1 | 1 | 19 | 14 | 0.014 s | 16.4 MiB | 5,864 B | 1 / ok |
| `notgit/corpus/zips/mdb-1832.zip` | 1,016,865 B | 925 | 360 | 2,608 | 3,544 | 49 | 1.255 s | 61.2 MiB | 14,657 B | 1 / ok |
| `notgit/corpus/zips/mdb-2933.zip` | 18,875,148 B | 14,710 | 1,626 | 226,347 | 17,045 | 74 | 14.740 s | 503.5 MiB | 21,634 B | 1 / ok |

## Çok büyük feed gözlemi

`notgit/corpus/zips/mdb-2519.zip` (171,413,597 B) aynı stdin yolunda 180 saniyeyi
aştı ve sonuç üretmedi. Koşum kontrollü olarak durduruldu; bu bir başarılı
validation sonucu değildir.

## Sonuç

- Compact response boyutu küçük kaldı; en büyük tamamlanan koşumda 21,634 bayt.
- ZIP boyutu tek başına RAM riskini tahmin etmiyor; 18.9 MB ZIP yaklaşık 503.5 MiB
  peak RSS kullandı.
- `GTFS_TOTAL_TIMEOUT_SECONDS=105` ve ayrı Analyzer timeout’u production için
  gerekli; büyük feed'ler kontrollü `ANALYSIS_TIMEOUT` veya ileride doğrulanmış
  `RESOURCE_LIMIT` sonucuna dönmelidir.
- Bu ölçümden sonra production compressed-size limiti, runtime ve peak RSS
  birlikte değerlendirilmeden artırılmamalıdır.
