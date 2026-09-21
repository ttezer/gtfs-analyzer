# DQ_020 simple-loop istisnası araştırması

## 2026-09-21 sonucu

`DQ_020` zaten feed/alan seviyesinde tek notice üretir ve yalnızca
`trips.txt.trip_headsign` boşluk oranını ölçer. Mevcut kodda kapalı döngü için
özel bir muafiyet yoktur; ancak `TRP_011`, `trip_headsign` ve `trip_short_name`
ikisi de boş olduğunda yolcu tanımlayıcısı bulunmamasını ayrıca değerlendirir ve
route adı yolcuya yeterli tanımlayıcı sağlıyorsa bu yolu atlar.

Bu nedenle simple-loop istisnasını doğrudan DQ_020'ye eklemek erken olur:

- `trip_headsign` önerilen alan olarak boş kalabilir; route adı loop'u tanımlıyor
  olsa bile bu, mevcut DQ_020 metriğinin kapsamını değiştiren bir karardır.
- İstisna eklenirse aynı feed'lerde DQ_020 ve TRP_011 oranları birlikte
  karşılaştırılmalı; yalnız DQ_020'yi sessizce düşürmek ölçüm karşılaştırmasını
  bozar.
- Şimdilik mevcut feed-level aggregation korunuyor, runtime değişikliği yapılmıyor.

Sonraki adım: loop route/pattern, normal route ve boş headsign kümelerini ayrı
ölçen bir karşı-olgusal corpus koşumu.
