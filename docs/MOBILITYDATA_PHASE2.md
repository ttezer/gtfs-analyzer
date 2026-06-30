# MobilityData Phase 2 Decisions

Bu faz, “kısmen var” görünen kuralların mevcut davranışını çalışan kod ve testlerle netleştirir.

| Issue | Sonuç | GTFS Analyzer kararı |
|---|---|---|
| #153 | Eşdeğere yakın | `STP_020`, `STP_029`, `STP_030` ayrı ve daha açıklayıcı sinyaller olarak korunur. |
| #154 | Eşdeğere yakın | `STP_016/017/021/029` stop-stop ve child-parent mesafe eksenlerini ayrı raporlar. |
| #159 | Tamamlandı | Eksik ve negatif değer `TRF_005`; aşırı büyük değer `TRF_010`. 24 saat sınırı testle kilitlendi. |
| #1275 | Bilinçli farklı | `ARC_020` genel Quality önerisidir; California zorunluluğu global Spec yapılmaz. |
| #1277 | Bilinçli farklı | Continuous servis `TRP_019`; genel shape eksikliği `RTS_017/ARC_020`. Her trip için global warning üretilmez. |
| #1280 | Bilinçli farklı | `FIN_018`, email veya contact URL kabul eder. California email-only profili global varsayılan değildir. |
| #1729 | Parçalı/tamam | Duplicate trip ID mevcut; coverage penceresi `upcoming_service_days` ile configurable. Belirsiz duplicate-trip-name analitiği Faz 4'e bırakıldı. |
| #1792 | Bekle | Genel `ARC_020` korunur. DRT istisnası, güvenilir feature detector olmadan eklenmez; MD PR #2123 acceptance sonucuna bağlıdır. |

Bu kararlar “eksik implementasyon” değil, global validator ile bölgesel profil kurallarını ayırma tercihidir.
