# GTFS-JP korpus ölçümü — 2026-08-25

Bu ölçüm, 2026-08-20 tarihli tam MobilityDatabase manifestindeki `country=JP` satırlarını yeniden kullanır. Küme 592 feed kimliğinden oluşur; analyzer güncel çalışma ağacındaki release binary ile, karşılaştırılabilir olması için `--today 20260820` kullanılarak çalıştırılmıştır. GTFS-RT kapsam dışıdır.

Ham JSONL çıktı repo dışındadır:

`/Users/tacettintezer/GTFS/run13-artifacts/jp-analyzer-only-20260825.jsonl`

Özet dosyası:

`/Users/tacettintezer/GTFS/run13-artifacts/jp-analyzer-only-20260825.summary.json`

## Koşum sonucu

| Ölçüm | Sonuç |
|---|---:|
| Seçilen JP feed | 592 |
| JSON raporu üreten feed | 590 |
| ZIP olmayan URL | 2 (`mdb-1057`, `mdb-874`) |
| GTFS-JP rozeti | 585 |
| GTFS-JP bulgusu üreten feed | 543 / 585 |
| Toplam JPN bulgusu | 99.057 |
| JPN Quality bulguları | 98.825 |
| JPN Interop bulguları | 232 |
| Ortalama genel skor (585 feed) | 93,506 |
| Ortalama Quality skoru (585 feed) | 74,405 |

JPN bulgusu olan 543 feed'in ortalama genel skoru `93,415`, JPN bulgusu olmayan 42 feed'in ortalaması `94,686` oldu. Bu karşılaştırma nedensel etki iddiası değildir; gruplar aynı zamanda diğer Quality/Interop bulgularını da içerir.

## JPN kural dağılımı

| Kural | Feed sayısı | Bulgu sayısı |
|---|---:|---:|
| JPN_001 | 543 | 12.824 |
| JPN_002 | 216 | 216 |
| JPN_003 | 4 | 4 |
| JPN_004 | 7 | 7 |
| JPN_006 | 15 | 15 |
| JPN_008 | 543 | 5.162 |
| JPN_009 | 543 | 80.646 |
| JPN_010 | 148 | 148 |
| JPN_011 | 1 | 1 |
| JPN_013 | 13 | 13 |
| JPN_016 | 3 | 17 |
| JPN_018 | 1 | 3 |
| JPN_019 | 1 | 1 |

Bu koşumda JPN_012, JPN_005, JPN_007, JPN_014, JPN_015, JPN_017, JPN_020 ve JPN_021 için bulgu oluşmadı. JPN_018 sonucu önceki adjudication ile aynıdır: `1 feed / 3 bulgu`. JPN_019 da `1 feed / 1 bulgu` olarak kalmıştır; bu korpusta bilinmeyen `table_name` türevi görülmemiştir.

## Sınırlar

- Manifest feed kimlikleri aynıdır; `latest.zip` URL'leri yeniden indirildiği için arşiv byte'larının tamamının aynı olduğu varsayılmaz.
- Bu ölçüm yalnız GTFS Analyzer sonuçlarını yeniler; MobilityData Validator yeniden koşturulmamıştır.
- ZIP olmayan iki satır skor veya kural dağılımına dahil edilmemiştir.
- v4 runtime kuralı uygulanmamıştır; sonuçlar GTFS-JP v3 kapsamındaki mevcut kuralların ölçümüdür.
