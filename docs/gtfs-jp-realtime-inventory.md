# GTFS-JP / GTFS-RT envanteri

Bu belge GTFS-RT’yi sonraki iş paketi olarak kaydeder. Bu sprintte GTFS-RT feed’i parse edilmez, doğrulama kuralları çalıştırılmaz ve statik GTFS-JP v3 kapsamına dahil edilmez.

## Envanter

| GTFS-RT akışı | Statik GTFS bağlantısı | Sonraki doğrulama başlıkları |
|---|---|---|
| `TripUpdate` | `trip_id`, `route_id`, `stop_id`, `stop_sequence` | Statik referans bütünlüğü, zaman sıralaması, güncel servis eşleşmesi |
| `VehiclePosition` | `vehicle.id`, `trip.trip_id`, `route_id`, konum | Trip/vehicle referansı, konum aralığı, timestamp tazeliği |
| `Alert` | `informed_entity` içindeki route/stop/trip | Etkilenen varlıkların statik feed’de bulunması, zaman aralığı ve metin dili |

## Planlanan sınırlar

- GTFS-RT protobuf çözümleme ve feed türü tespiti ayrı bir pipeline aşaması olacaktır.
- Statik GTFS ile gerçek zamanlı akışın aynı yayıncıya ait olduğu otomatik varsayılmayacaktır; yayıncı/URL eşleştirmesi sonraki tasarım kararıdır.
- GTFS-RT için JPN kural kimlikleri bu sprintte ayrılmamıştır. Önce `TripUpdate`, `VehiclePosition` ve `Alert` fixture’ları hazırlanacak, ardından kural sınıfı ve skor etkisi belirlenecektir.
- GTFS-JP v3/v4 sürüm iddiası ile GTFS-RT desteği birbirinden bağımsız raporlanacaktır.

## Kaynaklar

- [GTFS Realtime reference](https://gtfs.org/documentation/realtime/reference/)
- [GTFS Realtime best practices](https://gtfs.org/documentation/realtime/realtime-best-practices/)
- [GTFS-JP format referansı](https://www.gtfs.jp/developpers-guide/format-reference.html)
