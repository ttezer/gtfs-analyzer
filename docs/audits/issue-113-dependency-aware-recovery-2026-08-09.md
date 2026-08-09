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

Regression testleri:

- `malformed_feed_info_keeps_independent_cross_ref_findings`
- `invalid_geojson_does_not_make_missing_stops_optional`
- `arc004_missing_required_file_returns_partial_report`
