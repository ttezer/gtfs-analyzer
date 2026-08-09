# Issue #113 — dependency-aware partial recovery audit

Date: 2026-08-09

PARTIAL raporlarında K4, K5 ve K6 artık tek bir global unavailable-file kapısı
kullanmaz. Her alt kontrol kendi dosya bağımlılıklarını değerlendirir; bozuk
`feed_info.txt` route/trip/stop cross-reference ve analytics kapsamını kapatmaz,
bozuk `shapes.txt` ise yalnız shape'e bağlı kontrolleri etkiler.

`locations.geojson` yalnızca okunabilir ve kullanılabilir geometri içeriyorsa
eksik `stops.txt` için koşullu gerekliliği karşılar. Geçersiz JSON, boş/geçersiz
geometri veya okunamayan ZIP girdisi `locations.geojson`'ı unavailable yapar ve
`stops.txt` için `ARC_004` korunur.

K6 veri kalitesi kontrolleri de aynı sözleşmeye bağlandı. `DQ_005` gerçek ve
okunabilir bir `calendar.txt` veya `calendar_dates.txt` kaynağı olmadan; `DQ_009`
ise `trips.txt` ve `stop_times.txt` birlikte kullanılabilir olmadan çalışmaz.
`STP_040/041`, `DQ_011/017/022`, route/agency kontrolleri ve `FIN_*` kontrolleri
de ilgili dosyanın envanterde bulunup okunabilir olmasını şart koşar. Böylece
okunamayan bir calendar veya stop_times dosyası, türetilmiş "aktif servis yok"
ya da "hiç stop_times yok" bulgusuna dönüşmez.

`FileAvailability` artık K1'in gerçek dosya envanterini de taşır. `available()`
okunabilirliği, `present()` varlığı, `present_and_available()` ise ikisini birlikte
ifade eder; takvim kaynağı gibi "en az bir dosya gerçekten var mı?" kararları son
metodu kullanır. Doğrudan bellek-içi K4/K5/K6 testleri için `complete()` geriye
dönük uyumluluk modunu korur.

Regression testleri:

- `malformed_feed_info_keeps_independent_cross_ref_findings`
- `invalid_geojson_does_not_make_missing_stops_optional`
- `arc004_missing_required_file_returns_partial_report`
- `unavailable_or_missing_calendar_does_not_produce_dq_005`
- `unavailable_stop_times_does_not_produce_dq_009`
- `unavailable_feed_info_does_not_produce_feed_expiry_findings`
- `inventory_separates_missing_from_unreadable_files`
