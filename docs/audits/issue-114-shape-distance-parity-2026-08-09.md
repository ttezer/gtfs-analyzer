# Issue #114 — shape distance parity audit

Date: 2026-08-09

`trip_with_shape_dist_traveled_but_no_shape_distances` artık `SHP_030` ile
eşlenir. Kural, `stop_times.txt` mesafesi kullanan trip'lerin referans shape'inde
mesafe alanı eksik olan shape'leri shape başına aggregate eder; `SHP_017` ile
karıştırılmaz.

Kod regression'ı `shp030_aggregates_and_does_not_duplicate_related_findings`
fixture'ı ile kilitlidir. Parity defteri de doğrudan `SHP_030` mapping'i ve
ilgili testle korunur.

Issue kabul kriterindeki 31-feed / 142.819-notice tam corpus artifact'ı bu
workspace'te bulunmadığından yeniden koşturulmuş gibi gösterilmemiştir. Artifact
geri geldiğinde corpus karşılaştırması ayrıca eklenmelidir.
