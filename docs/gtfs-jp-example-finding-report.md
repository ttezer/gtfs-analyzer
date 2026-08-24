# GTFS-JP örnek bulgu raporu

Bu örnek, Nishizawa-san’a gönderilecek teknik değerlendirme paketinde kullanılmak üzere hazırlanmıştır. Feed sentetiktir; gerçek bir işletmecinin verisini temsil etmez. Analyzer sürüm iddiası üretmez, yalnızca GTFS-JP sinyali ve bulgu kapsamını raporlar.

## Örnek giriş

| Dosya | Satır | Alan | Değer |
|---|---:|---|---|
| `agency_jp.txt` | 3 | `agency_zip_number` | `123-4567` |
| `routes_jp.txt` | 8 | `route_id` | `route-missing` |
| `pattern_jp.txt` | 4 | `jp_pattern_id` | *(boş)* |
| `translations.txt` | 6 | `record_id` | `stop-missing` |

## Beklenen bulgular

| Rule ID | Sınıf / önem | Varlık | Kanıt | Öneri |
|---|---|---|---|---|
| `JPN_013` | Quality / Orta | `agency_id=A1` | `agency_zip_number` yedi ASCII rakam değil | Tireyi kaldırıp `1234567` biçimine getirin |
| `JPN_015` | Interop / Yüksek | `route_id=route-missing` | `routes_jp.txt` kimliği `routes.txt` içinde yok | Mevcut route kimliğine bağlayın veya satırı kaldırın |
| `JPN_017` | Interop / Yüksek | satır 4 | Dosya mevcutken `jp_pattern_id` boş | Benzersiz bir pattern kimliği verin |
| `JPN_019` | Interop / Orta | `record_id=stop-missing` | `ja-Hrkt` satırı mevcut stop kaydını göstermiyor | `record_id` değerini `stops.txt` ile eşleştirin |

## Yorumlama

- `pattern_jp.txt` dosyasının hiç bulunmaması tek başına hata değildir; mevcut olduğunda `jp_pattern_id` zorunlu hale gelir.
- `origin_stop`, `via_stop` ve `destination_stop` açıklayıcı metinlerdir; stop ID foreign key bulgusu üretilmez.
- `jp_parent_route_id`, `jp_trip_desc` ve `jp_trip_desc_symbol` için spesifikasyonda olmayan regex veya otomatik foreign key kuralı uygulanmaz.
- Tarayıcı analizinde GTFS dosyası dış sunucuya yüklenmez; doğrulama kullanıcının cihazında çalışır.

## Nishizawa-san için değerlendirme soruları

1. JPN_012–JPN_021 kapsamındaki zorunluluk ve referans ayrımları GTFS-JP v3 rehberiyle uyumlu mu?
2. `office_url`/`office_phone` kalite kontrolleri fazla katı veya eksik mi?
3. v4’te yeniden sınıflandırılan alanlar için hangi kurallar v4 profilinde tutulmalı?

Kaynak: [GTFS-JP v3/v4 uyumluluk matrisi](gtfs-jp-v3-v4-matrix.md).
