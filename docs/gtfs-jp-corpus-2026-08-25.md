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
| ZIP olmayan payload | 2 (`mdb-1057`, `mdb-874`) |
| GTFS-JP rozeti | 585 |
| GTFS-JP bulgusu üreten feed | 543 / 585 |
| Toplam JPN bulgusu | 99.057 |
| JPN Quality bulguları | 98.825 |
| JPN Interop bulguları | 232 |
| Ortalama genel skor (585 feed) | 93,506 |
| Ortalama Quality skoru (585 feed) | 74,405 |

JPN bulgusu olan 543 feed'in ortalama genel skoru `93,415`, JPN bulgusu olmayan 42 feed'in ortalaması `94,686` oldu. Bu karşılaştırma nedensel etki iddiası değildir; gruplar aynı zamanda diğer Quality/Interop bulgularını da içerir.

## Mutable `latest.zip` follow-up

Manifestteki 592 feed kimliği aynı bırakılarak, aynı release binary ve aynı
`--today 20260820` parametresiyle yeniden indirilen payload'lar karşılaştırıldı.
Üç feed'in arşiv SHA-256'sı/byte sayısı değişmiş ve toplam notice/skor farkı
yalnızca aşağıdaki kural değişimlerinden oluşmuştur:

| Feed | Arşiv boyutu eski → yeni | Notice eski → yeni | Skor eski → yeni | Değişen kurallar |
|---|---:|---:|---:|---|
| `jbda-isecity-communitybus` | 69.188 → 69.206 B | 279 → 274 | 93,3 → 94,4 | CAL_008: 3→0; CAL_014: 1→0; OPR_012: 1→0 |
| `jbda-komonotown-communitybus` | 34.834 → 34.852 B | 154 → 153 | 90,7 → 91,3 | CAL_007: 2→5; CAL_008: 3→0; CAL_014: 1→0 |
| `mdb-3175` | 8.416.859 → 8.416.574 B | 1.857 → 1.858 | 86,6 → 86,6 | CAL_024: 57→58 |

Bu üç feed'de JPN kural dağılımı değişmemiştir. Ayrıntılı tekrar koşusu
[`three-feed-recheck-20260825/summary.json`](/Users/tacettintezer/GTFS/run13-artifacts/three-feed-recheck-20260825/summary.json)
altındadır.

## `not_zip` kayıtlarının resmî kaynakla doğrulanması

`mdb-1057` ve `mdb-874` MobilityDatabase kayıtları gerçek GTFS Schedule
feed'leridir; katalogdaki eski `latest.zip` yolları sırasıyla HTML 404 ve
taşınmış bir HTML sayfası döndürmüştür. Bu nedenle ilk koşudaki iki satır
“feed değil” olarak yorumlanmamalıdır. Resmî kaynaklardan ayrı bir doğrulama
koşusu yapıldı:

| Feed | Resmî payload | Toplam notice | JPN notice | Skor |
|---|---|---:|---:|---:|
| `mdb-1057` | [Fukuoka Municipal Ferry GTFS](https://www.city.fukuoka.lg.jp/kowan/kyakusen/shisei/shieitosen_opendata.html) | 607 | 155 | 78,0 |
| `mdb-874` | [Aomori City Bus GTFS-JP](https://aomoricitybus.com/opendata/index.html) | 6.838 | 5.006 | 88,2 |

Bu iki resmî kaynak sonucu ilk 592'lik aggregate'a geriye dönük olarak
eklenmemiştir; farklı URL/snapshot olduğu için ayrı follow-up olarak tutulur.
Ham çıktı:
[`mdb-1057-mdb-874-official-20260825.json`](/Users/tacettintezer/GTFS/run13-artifacts/mdb-1057-mdb-874-official-20260825.json).

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

## Açık V4 profil follow-up'ı

Yukarıdaki aggregate varsayılan `auto`/v3-legacy kapsamıdır. Profilin varsayılan
olarak V4 yapılmasının etkisini ölçmek için aynı 592 feed, bu kez açık
`--gtfs-jp-profile v4` seçimiyle ayrıca koşturuldu. Sonuç özeti:

| Ölçüm | Sonuç |
|---|---:|
| `ok` sonuç | 588 |
| `fatal` sonuç | 2 |
| Kısmi çıktı | 2 |
| GTFS-JP rozeti | 583 |
| JPN_019 | 1 feed / 1 bulgu |
| JPN_022 | 16 feed / 2.473 bulgu |

Ham follow-up özeti:
[`jp-v4-fixed-20260825.summary.json`](/Users/tacettintezer/GTFS/run14-artifacts/jp-v4-fixed-20260825.summary.json).
V4 açıkça seçilmediğinde bu sonuçlar uygulanmaz; araç feed içeriğinden v3/v4
çıkarımı yapmaz.

## Sınırlar

- Manifest feed kimlikleri aynıdır; `latest.zip` URL'leri yeniden indirildiği için arşiv byte'larının tamamının aynı olduğu varsayılmaz. Ürün karşılaştırmaları payload SHA-256 drift'i ile birlikte okunmalıdır.
- Bu ölçüm yalnız GTFS Analyzer sonuçlarını yeniler; MobilityData Validator yeniden koşturulmamıştır.
- ZIP olmayan iki payload ilk aggregate'ta skor veya kural dağılımına dahil edilmemiştir; gerçek feed oldukları resmî kaynak follow-up'ında doğrulanmıştır.
- v4 runtime kuralı uygulanmamıştır; sonuçlar GTFS-JP v3 kapsamındaki mevcut kuralların ölçümüdür.
