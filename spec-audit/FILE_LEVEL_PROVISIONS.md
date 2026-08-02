# Dosya düzeyi hükümlerin adjudikasyonu

**Tarih:** 2026-08-02 · **Spec sürümü:** 27 Nisan 2026 Schedule Reference

## Neden bu dosya var

Kapsam defteri (`spec_coverage_ledger.txt`) ve iddia defteri (`spec_claims_ledger.txt`)
**alan tablosundan** türeyen hükümleri ölçer. Spec'in normatif içeriği bununla bitmiyor:
her dosyanın başlığında bir `File: …` satırı var ve bunların yedisi koşulludur.

`extract_fields.py` 2026-08-02'ye kadar bu satırı hiç okumuyordu. Artık okuyor
(`files.<dosya>.presence`), ama **otomatik bir kapıya bağlanamıyor**: koşullar düzyazıdadır
("locations.geojson varsa", "translations.txt varsa") ve hangi kuralın hangi koşulu
karşıladığını makine eşleştiremez. Bu yüzden adjudikasyon elle yapıldı ve **buraya** yazıldı —
bir sonraki spec revizyonunda yeniden okunması gereken yer burasıdır.

## Dağılım (31 dosya)

| Presence | adet |
|---|---|
| Required | 4 |
| Optional | 20 |
| Conditionally Required | 5 |
| Conditionally Forbidden | 2 |

`Required` ve `Optional` olanlar koşulsuzdur: ilki `ARC_004`, ikincisi hiçbir hüküm taşımaz.
Aşağıdaki yedisi koşulludur ve tek tek incelenmiştir.

## Yedi koşullu hüküm

| # | Dosya | Spec koşulu (birebir) | Karşılayan | Karar |
|---|---|---|---|---|
| 1 | `stops.txt` | *"Optional if demand-responsive zones are defined in locations.geojson. Required otherwise."* | `ARC_004` | ✅ Muafiyet kodda mevcut (`k1_parse.rs`, `has_locations_geojson` filtresi). Saf Flex feed'i yanlış reddedilmez. |
| 2 | `calendar.txt` | *"Required unless all dates of service are defined in calendar_dates.txt."* | `ARC_008` | ✅ İkisi de yoksa Kritik. |
| 3 | `calendar_dates.txt` | *"Required if calendar.txt is omitted."* | `ARC_008` | ✅ Aynı kural, aynı olgu. |
| 4 | `levels.txt` | *"Required when describing pathways with elevators (pathway_mode=5)."* | `LVL_006` + `level_id` FK'sı | ✅ **Dolaylı ama tam:** asansör geçidine bağlı durakta `level_id` yoksa LVL_006; `level_id` var ama `levels.txt` yoksa FK ihlali. Dosya düzeyinde ayrı bir kural gerekmiyor. |
| 5 | `feed_info.txt` | *"Required if translations.txt is provided. Recommended otherwise."* | **`ARC_031`** (norm) + `ARC_020` (tavsiye) | 🔧 **BOŞLUK VARDI, KAPATILDI.** İki hâl de `ARC_020` (Düşük·Quality) sayılıyordu — bir norm tavsiye diye raporlanıyordu. |
| 6 | `networks.txt` | *"Forbidden if network_id exists in routes.txt."* | `XFL_019` | ✅ Aynı çakışma routes tarafından raporlanır; tek bulgu yeterlidir. |
| 7 | `route_networks.txt` | *"Forbidden if network_id exists in routes.txt."* | `XFL_019` | ✅ 6 ile aynı. |

## Yedi'den çıkan tek boşluk

`feed_info.txt`. Spec'in cümlesi **ikiye ayrılıyor** ve iki farklı sınıf gerektiriyor:

- *"Required if translations.txt is provided"* → **norm** → `ARC_031`, Kritik·Spec
- *"Recommended otherwise"* → **tavsiye** → `ARC_020`, Düşük·Quality

Aynı gün `PTH_017`'de bunun **tam tersi** bulundu: spec'in "should" dediği bir tavsiye Spec
sınıfında dayatılıyordu. İki hata da sınıf otoritesinin ihlali; yönleri farklı:

| yön | sonuç | maliyet |
|---|---|---|
| tavsiyeyi norm saymak (`PTH_017`) | geçerli feed yayından alıkonabilir | **ağır** |
| normu tavsiye saymak (`ARC_020`) | kullanıcı eksik uyarılır | hafif |

## Ölçülen etki

Korpus (239 feed): 34 feed `translations.txt` taşıyor, **2'sinde** `feed_info.txt` yok
(`mdb-2519`, `mdb-2933`). `ARC_031` bu ikisinde ateşler ve ikisi R1'de düşer.

## Bir sonraki spec revizyonunda ne yapılmalı

1. `python3 spec-audit/extract_fields.py` ile `spec_fields.json`'u yeniden üret.
2. `presence` değeri `Conditionally *` olan dosyaları listele.
3. Bu tabloyla karşılaştır: **yeni bir koşullu dosya çıkarsa** ya da mevcut bir koşulun
   **metni değişirse**, o satırı yeniden adjudike et.
4. Bu ölçüm otomatik DEĞİLDİR — kimse bu adımı hatırlatmaz. `spec_fields.json`'daki
   `presence` alanının varlığı yalnız veriyi hazır tutar, kararı vermez.
