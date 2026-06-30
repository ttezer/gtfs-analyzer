# MobilityData Phase 3 Decisions

| Issue | Sonuç |
|---|---|
| #158 | `TRF_020`: kuş uçuşu mesafe / min_transfer_time > 2 m/s ise Quality notice. |
| #178 | `STM_053`: üç veya daha fazla ardışık durağın event time değeri aynıysa trip-level notice. |
| #884 | Mevcut `STM_017/027` ve shape sıralama kuralları korunur. Güvenilir loop/lollipop sınıflandırıcısı corpus çalışmasına bırakıldı. |
| #1137 | Günlük trip histogramı kuruma özel threshold gerektirdiğinden global kural yapılmadı; profil/config tasarımına bırakıldı. |
| #1922 | `SHP_014` mevcut varyant-farkında başlangıç/bitiş mesafe testiyle korunur. |
| #1923 | `SHP_011` mevcut configurable shape jump eşiğiyle korunur. |

Faz 3 sonunda yeni eşikler corpus üzerinde ölçülmeden severity yükseltilmeyecektir.
