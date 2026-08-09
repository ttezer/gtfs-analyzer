# Issue #116 — single-point shape parity audit

Date: 2026-08-09

`single_shape_point` artık `SHP_006` için intentional-difference / near-parity
olarak kayıtlıdır. Analyzer yalnızca trip tarafından kullanılan tek noktalı
shape'i `SHP_006` (Düşük · Quality) olarak raporlar; orphan tek noktalı shape
`SHP_018` kapsamında kalır. Böylece MobilityData'nın generic sinyali, orphan
shape'leri yanlış biçimde kullanılan shape bulgusuna dönüştürmez.

Kod regression matrisi kullanılan tek nokta, kullanılmayan tek nokta ve iki
noktalı shape ayrımını kapsar; parity testleri mapping kararını sabitler.

Issue kabul kriterindeki 7-feed / 17-warning tam corpus artifact'ı bu
workspace'te bulunmadığından yeniden koşturulmuş gibi gösterilmemiştir. Artifact
geri geldiğinde corpus karşılaştırması ayrıca eklenmelidir.
