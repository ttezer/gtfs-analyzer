# GTFS Validator & Analyzer — Kural Listesi

🇹🇷 **Türkçe** · 🇬🇧 [English](RULES.en.md) · 🇯🇵 [日本語](RULES.ja.md)

545 kural, 37 grup. Her kural benzersiz bir ID, önem seviyesi ve sınıf ile tanımlanır.
Önem seviyeleri: **KRİTİK** (yayın engelleyici) · **YÜKSEK** · **ORTA** · **DÜŞÜK** · **BİLGİ**
Sınıflar: **Spec** (GTFS Geçerliliği) · **Interop** (GTFS Uyumluluğu) · **Quality** (GTFS Kalitesi) · **Analytics** (GTFS Analitiği)

---

## ARC — Arşiv / Dosya Seviyesi

| Kural | Başlık | Önem | Sınıf |
|---|---|---|---|
| ARC_001 | ZIP arşivi açılamadı | KRİTİK | Spec |
| ARC_002 | Dosya UTF-8 ile okunamıyor | KRİTİK | Quality |
| ARC_003 | İsteğe bağlı dosyada UTF-8 kodlama hatası | ORTA | Quality |
| ARC_004 | Zorunlu dosya eksik | KRİTİK | Spec |
| ARC_006 | İsteğe bağlı GTFS dosyası mevcut | BİLGİ | Quality |
| ARC_007 | GTFS dışı tanınmayan dosya | BİLGİ | Quality |
| ARC_008 | Takvim dosyası eksik (calendar.txt ve calendar_dates.txt) | KRİTİK | Spec |
| ARC_009 | Dosyada veri satırı yok | KRİTİK | Quality |
| ARC_010 | Dosya UTF-8 BOM içeriyor | ORTA | Quality |
| ARC_011 | Dosya boyutu (bilgi) | BİLGİ | Analytics |
| ARC_012 | Satır sütun sayısı başlıkla uyuşmuyor | KRİTİK | Spec |
| ARC_013 | CSV ayrıştırma hatası | KRİTİK | Spec |
| ARC_014 | Başlıkta baştaki/sondaki boşluk | ORTA | Quality |
| ARC_015 | Yinelenen başlık sütunu | KRİTİK | Interop |
| ARC_025 | Zorunlu sütun başlıkta hiç yok (header'da eksik) | KRİTİK | Spec |
| ARC_017 | Bilinmeyen sütun (GTFS spesifikasyonunda tanımlı değil) | BİLGİ | Quality |
| ARC_018 | Boş veri satırı | ORTA | Quality |
| ARC_019 | Başlıkta boş sütun adı | YÜKSEK | Quality |
| ARC_020 | Önerilen GTFS dosyası eksik (shapes.txt veya feed_info.txt) | DÜŞÜK | Quality |
| ARC_021 | Alanda yazdırılamaz veya sorunlu karakter | DÜŞÜK | Quality |
| ARC_022 | Dosya satır sayısı 1.000.000 sınırını aşıyor | DÜŞÜK | Quality |
| ARC_023 | ZIP içinde nested ZIP dosyası — GTFS formatında desteklenmez | ORTA | Quality |
| ARC_024 | GTFS .txt dosyası ZIP içinde alt dizinde — standart parser'lar tarafından atlanır | ORTA | Spec |
| ARC_026 | Dosyada hatalı satır sonu karakteri | ORTA | Quality |
| ARC_027 | ZIP girdisinde kullanıcı okuma izni yok | BİLGİ | Quality |
| ARC_028 | GTFS yayın URL'si .zip dosya adıyla bitmiyor | DÜŞÜK | Quality |
| ARC_029 | Sıkıştırma koruması: arşiv zip-bomb sınırını aştı | KRİTİK | Quality |
| ARC_030 | Alan değerinde sekme veya satır sonu karakteri | YÜKSEK | Spec |

## BKR — Booking Rules (Rezervasyon Kuralları)

| Kural | Başlık | Önem | Sınıf |
|---|---|---|---|
| BKR_001 | Önceki gün rezervasyon alanı yasak bağlamda dolu | YÜKSEK | Spec |
| BKR_002 | prior_notice_start_day yalnızca prior_notice_last_day ile kullanılabilir | YÜKSEK | Spec |
| BKR_003 | prior_notice_start_time yalnızca prior_notice_start_day ile kullanılabilir | YÜKSEK | Spec |
| BKR_004 | Anlık rezervasyonda prior_notice alanları yasak | YÜKSEK | Spec |
| BKR_005 | prior_notice_duration_max yalnızca booking_type=1 ile geçerli (booking_type=0/2 ile yasak) | ORTA | Spec |
| BKR_006 | prior_notice_duration_min geçersiz (≤ 0 veya sayısal değil) | YÜKSEK | Spec |
| BKR_007 | booking_type=1 için prior_notice_duration_min zorunlu | KRİTİK | Spec |
| BKR_008 | booking_type=2 için prior_notice_last_day zorunlu | KRİTİK | Spec |
| BKR_009 | booking_type=2 için prior_notice_last_time zorunlu | KRİTİK | Spec |
| BKR_010 | prior_notice_start_day belirtilmişse prior_notice_start_time zorunlu | YÜKSEK | Spec |
| BKR_011 | prior_notice_last_day > prior_notice_start_day: rezervasyon penceresi geçersiz | YÜKSEK | Interop |
| BKR_012 | booking_type=2 iken prior_notice_duration_min yasak | ORTA | Spec |
| BKR_013 | prior_notice_last_time yalnızca prior_notice_last_day ile kullanılabilir | YÜKSEK | Spec |
| BKR_014 | prior_notice_service_id yalnızca booking_type=2 ile kullanılabilir | YÜKSEK | Spec |
| BKR_015 | prior_notice_service_id bulunamadı (calendar/calendar_dates) | KRİTİK | Spec |
| BKR_016 | booking_type eksik veya geçersiz | KRİTİK | Spec |
| BKR_017 | pickup_booking_rule_id bulunamadı (booking_rules) | KRİTİK | Spec |
| BKR_018 | drop_off_booking_rule_id bulunamadı (booking_rules) | KRİTİK | Spec |
| BKR_019 | booking_rule_id eksik veya yineleniyor | KRİTİK | Spec |

## AGN — Agency (İşletici)

| Kural | Başlık | Önem | Sınıf |
|---|---|---|---|
| AGN_001 | agency.txt dosyası eksik | KRİTİK | Spec |
| AGN_002 | agency_name eksik | KRİTİK | Spec |
| AGN_003 | agency_url eksik veya geçersiz | KRİTİK | Spec |
| AGN_004 | agency_timezone eksik veya geçersiz | KRİTİK | Spec |
| AGN_005 | Kuruluşlar arası saat dilimi tutarsızlığı | ORTA | Quality |
| AGN_006 | agency_lang geçersiz | DÜŞÜK | Spec |
| AGN_007 | agency_phone geçersiz | DÜŞÜK | Quality |
| AGN_008 | agency_fare_url geçersiz | DÜŞÜK | Spec |
| AGN_009 | agency_email geçersiz | DÜŞÜK | Spec |
| AGN_010 | agency_id yineleniyor | KRİTİK | Spec |
| AGN_011 | Birden fazla kuruluşta agency_id yok | KRİTİK | Spec |
| AGN_012 | agency_cemv_support geçersiz | DÜŞÜK | Quality |
| AGN_013 | Feed dili ve ajans dili uyuşmuyor | DÜŞÜK | Interop |
| AGN_014 | Birden fazla kuruluş var ama agency.txt'de agency_id eksik | KRİTİK | Spec |
| AGN_015 | agency_url güvensiz http kullanıyor (https önerilir) | BİLGİ | Quality |
| AGN_016 | agency_phone bilinen yer-tutucu/şüpheli numara | BİLGİ | Quality |
| AGN_017 | Birden çok agency farklı agency_lang taşıyor — diller arası tutarsızlık | DÜŞÜK | Interop |

## STP — Stops (Duraklar)

| Kural | Başlık | Önem | Sınıf |
|---|---|---|---|
| STP_001 | stop_id yineleniyor | KRİTİK | Spec |
| STP_002 | stop_id boş | KRİTİK | Spec |
| STP_003 | stop_name eksik veya stop_lat/stop_lon aralık dışı (aynı ID altında iki ayrı koşul) | KRİTİK | Spec |
| STP_004 | stop_lat sayısal değil | KRİTİK | Spec |
| STP_005 | stop_lon geçersiz veya aralık dışı | KRİTİK | Spec |
| STP_006 | stop_lat eksik | KRİTİK | Spec |
| STP_007 | stop_lon eksik | KRİTİK | Spec |
| STP_008 | location_type geçersiz | YÜKSEK | Spec |
| STP_009 | parent_station bulunamadı | KRİTİK | Spec |
| STP_010 | parent_station location_type=1 değil | YÜKSEK | Spec |
| STP_011 | Giriş/çıkış/boarding için parent_station zorunlu | KRİTİK | Spec |
| STP_012 | stop_times'ta istasyon veya giriş kullanılmış | KRİTİK | Spec |
| STP_013 | wheelchair_boarding geçersiz | DÜŞÜK | Spec |
| STP_014 | stop_timezone geçersiz | ORTA | Spec |
| STP_015 | level_id bulunamadı | KRİTİK | Spec |
| STP_016 | İki durak tam aynı koordinatta | ORTA | Quality |
| STP_017 | İki durak birbirine çok yakın | DÜŞÜK | Quality |
| STP_018 | Hiç durak yok | KRİTİK | Spec |
| STP_019 | stop_name çok uzun | DÜŞÜK | Quality |
| STP_020 | Hiç sefer geçmeyen durak | ORTA | Analytics |
| STP_021 | Boarding area parent'ı platform değil | YÜKSEK | Quality |
| STP_022 | stop_code eksik | ORTA | Quality |
| STP_023 | tts_stop_name geçersiz | DÜŞÜK | Quality |
| STP_024 | stop_access enum aralığı dışında değer (K2 ham alan kontrolü) | BİLGİ | Quality |
| STP_025 | stop_name baştaki veya sondaki boşluk içeriyor | ORTA | Quality |
| STP_026 | stop_access geçersiz değer | DÜŞÜK | Spec |
| STP_027 | Pathway istasyonunda stop_access belirtilmemiş | ORTA | Quality |
| STP_028 | stop_code çok uzun | BİLGİ | Quality |
| STP_029 | Durak istasyon içinde ama koordinat çok uzakta | ORTA | Quality |
| STP_030 | Üst istasyonun alt durağı yok | ORTA | Quality |
| STP_031 | Durak adı ve açıklaması aynı | BİLGİ | Quality |
| STP_032 | Pathway bağlantılı platform için parent_station eksik | ORTA | Quality |
| STP_033 | Durak zone_id eksik (ücret hesabı için gerekli) | BİLGİ | Quality |
| STP_034 | stop_url acente URL'siyle aynı | BİLGİ | Quality |
| STP_035 | stop_url hat URL'siyle aynı | BİLGİ | Quality |
| STP_036 | İstasyonun (location_type=1) parent_station'ı var | DÜŞÜK | Spec |
| STP_037 | Bazı duraklar tekerlekli sandalye erişilebilirliği (wheelchair_boarding) bildirmemiş | ORTA | Quality |
| STP_038 | Hiçbir durak tekerlekli sandalye erişilebilirliği (wheelchair_boarding) bildirmemiş | BİLGİ | Quality |
| STP_039 | stop_code birden fazla durakta kullanılıyor | DÜŞÜK | Quality |
| STP_040 | Durak adı gereksiz genel stop/station sözcüğü içeriyor | DÜŞÜK | Quality |
| STP_041 | Alt durak adı üst istasyon adını içermiyor | DÜŞÜK | Quality |

## RTS — Routes (Hatlar)

| Kural | Başlık | Önem | Sınıf |
|---|---|---|---|
| RTS_001 | route_id yineleniyor | KRİTİK | Spec |
| RTS_002 | agency_id bulunamadı | KRİTİK | Spec |
| RTS_003 | route_short_name ve route_long_name ikisi de eksik | KRİTİK | Spec |
| RTS_004 | route_type eksik veya geçersiz | KRİTİK | Spec |
| RTS_005 | route_url geçersiz | ORTA | Spec |
| RTS_006 | route_color geçersiz hex renk | ORTA | Spec |
| RTS_007 | route_text_color geçersiz hex renk | DÜŞÜK | Quality |
| RTS_008 | Hat rengi ve metin rengi kontrast düşük | ORTA | Quality |
| RTS_010 | route_short_name çok uzun | DÜŞÜK | Quality |
| RTS_011 | route_long_name çok uzun | DÜŞÜK | Quality |
| RTS_012 | Seferi olmayan hat | ORTA | Quality |
| RTS_013 | continuous_pickup geçersiz | DÜŞÜK | Spec |
| RTS_016 | Hiçbir aktif servis günü olmayan hat | DÜŞÜK | Quality |
| RTS_017 | Shape tanımlı olmayan hat | BİLGİ | Quality |
| RTS_018 | continuous_drop_off geçersiz | DÜŞÜK | Spec |
| RTS_019 | Yinelenen hat adı | ORTA | Quality |
| RTS_020 | Hat ve acente aynı URL'yi paylaşıyor | BİLGİ | Quality |
| RTS_021 | Kısa hat adı Google Transit eşiğini (6 karakter) aşıyor | DÜŞÜK | Quality |
| RTS_022 | Uzun hat adı kısa adı içeriyor | DÜŞÜK | Quality |
| RTS_023 | Hat açıklaması hat adının kopyası | BİLGİ | Quality |
| RTS_024 | route_cemv_support geçersiz | DÜŞÜK | Quality |
| RTS_025 | routes.txt'te agency_id boş (önerilen alan) | BİLGİ | Quality |
| RTS_026 | Yinelenen kısa hat adı | BİLGİ | Quality |
| RTS_027 | Yinelenen uzun hat adı | BİLGİ | Quality |
| RTS_028 | Flex pencereli rotada continuous_pickup/drop_off yasak | YÜKSEK | Interop |

## TRP — Trips (Seferler)

| Kural | Başlık | Önem | Sınıf |
|---|---|---|---|
| TRP_001 | trip_id eksik veya yineleniyor | KRİTİK | Spec |
| TRP_002 | route_id bulunamadı | KRİTİK | Spec |
| TRP_003 | service_id bulunamadı | KRİTİK | Spec |
| TRP_004 | shape_id bulunamadı | YÜKSEK | Spec |
| TRP_005 | direction_id geçersiz | ORTA | Spec |
| TRP_006 | wheelchair_accessible geçersiz | DÜŞÜK | Spec |
| TRP_007 | bikes_allowed geçersiz | DÜŞÜK | Spec |
| TRP_032 | cars_allowed geçersiz | DÜŞÜK | Spec |
| TRP_011 | Sefer yön adı girilmemiş | YÜKSEK | Quality |
| TRP_012 | Çift yönlü rotada direction_id eksik | DÜŞÜK | Quality |
| TRP_013 | Hat tek seferlik | DÜŞÜK | Quality |
| TRP_014 | trip_short_name çok uzun | BİLGİ | Quality |
| TRP_015 | block_id grubunda tek sefer | DÜŞÜK | Quality |
| TRP_017 | Frekans tabanlı sefer stop_times'ta eksik | ORTA | Quality |
| TRP_019 | Continuous servis aktifken shape_id eksik | YÜKSEK | Quality |
| TRP_020 | trip_headsign ara durak adıyla eşleşiyor | BİLGİ | Analytics |
| TRP_021 | Bisiklet izni (bikes_allowed) belirtilmemiş | BİLGİ | Quality |
| TRP_022 | Block içinde çakışan sefer saatleri | YÜKSEK | Interop |
| TRP_023 | Önümüzdeki 7 günde aktif sefer yok | DÜŞÜK | Quality |
| TRP_024 | Block içinde tutarsız rota tipi | DÜŞÜK | Quality |
| TRP_025 | Tekerlekli sandalye erişilebilirlik bilgisi eksik seferlerin oranı yüksek | BİLGİ | Quality |
| TRP_026 | Aktif tarihi olmayan sefer (service_id geçerli ama aktif gün seti boş) | ORTA | Analytics |
| TRP_028 | Bazı seferler tekerlekli sandalye erişilebilirliği işaretlememiş | ORTA | Quality |
| TRP_029 | Hiçbir sefer tekerlekli sandalye erişilebilirliği bildirmemiş | BİLGİ | Quality |
| TRP_031 | route_id eksik | KRİTİK | Spec |
| TRP_033 | Aynı block_id'yi paylaşan seferler farklı route_type taşıyor | ORTA | Quality |

## STM — Stop Times (Geçiş Zamanları)

| Kural | Başlık | Önem | Sınıf |
|---|---|---|---|
| STM_001 | trip_id bulunamadı | KRİTİK | Spec |
| STM_002 | stop_id bulunamadı | KRİTİK | Spec |
| STM_003 | arrival_time geçersiz format | KRİTİK | Spec |
| STM_004 | departure_time geçersiz format | KRİTİK | Spec |
| STM_005 | stop_sequence eksik veya geçersiz | KRİTİK | Spec |
| STM_006 | stop_id eksik (stop_times) | KRİTİK | Spec |
| STM_007 | Kalkış saati varış saatinden önce (departure_time < arrival_time) | YÜKSEK | Interop |
| STM_008 | Duraklar arası zaman geriye gidiyor | KRİTİK | Interop |
| STM_009 | pickup_type geçersiz | YÜKSEK | Spec |
| STM_010 | drop_off_type geçersiz | YÜKSEK | Spec |
| STM_012 | Duraklar arası hız gerçekçi değil | YÜKSEK | Interop |
| STM_013 | Karışık varış/kalkış zamanları | YÜKSEK | Quality |
| STM_014 | Segmentte aşırı hız | YÜKSEK | Analytics |
| STM_015 | İlk durakta arrival_time eksik | KRİTİK | Spec |
| STM_016 | Son durakta arrival_time eksik | KRİTİK | Spec |
| STM_017 | Sefer saatlerinde güzergah mesafesi eksik | ORTA | Quality |
| STM_018 | continuous_pickup geçersiz (stop_times) | ORTA | Spec |
| STM_019 | continuous_drop_off geçersiz (stop_times) | ORTA | Spec |
| STM_020 | Sıfır geçiş süresi (mesafe > 200m) | YÜKSEK | Quality |
| STM_021 | Duraklar arası mesafe sıfır veya negatif | YÜKSEK | Quality |
| STM_022 | timepoint geçersiz | ORTA | Spec |
| STM_024 | shape_dist_traveled birim tutarsızlığı | BİLGİ | Quality |
| STM_025 | Kısa segment zamanlaması | BİLGİ | Analytics |
| STM_026 | Durak arası mesafe aşırı uzun | YÜKSEK | Quality |
| STM_028 | Sefer süresi çok uzun | YÜKSEK | Analytics |
| STM_029 | Sefer süresi çok kısa | ORTA | Analytics |
| STM_030 | shape_dist_traveled negatif | DÜŞÜK | Spec |
| STM_032 | Aynı seferde yinelenen stop_sequence değeri | DÜŞÜK | Quality |
| STM_033 | Tek duraklı sefer (kullanılamaz) | YÜKSEK | Interop |
| STM_034 | Varış veya kalkış zamanından yalnızca biri tanımlı | ORTA | Interop |
| STM_035 | Aynı durak ardışık iki kez ziyaret ediliyor (terminal/döngü) | BİLGİ | Analytics |
| STM_036 | stop_times trip_id + stop_sequence'a göre sıralı değil (unsorted_stop_times) | BİLGİ | Quality |
| STM_037 | Flex penceresinde arrival_time/departure_time yasak | YÜKSEK | Spec |
| STM_038 | start_pickup_drop_off_window > end_pickup_drop_off_window | YÜKSEK | Interop |
| STM_039 | Flex bağlamında pickup/drop_off penceresi eksik | KRİTİK | Spec |
| STM_040 | Flex stop_times'ta pickup/drop_off_booking_rule_id eksik (spec'te Optional) | ORTA | Quality |
| STM_041 | stop_id ile location_id/group_id aynı anda kullanılamaz | YÜKSEK | Spec |
| STM_042 | stop_headsign Google Transit tarafından desteklenmeyen karakter içeriyor | DÜŞÜK | Interop |
| STM_043 | Sefer aşırı fazla durağa sahip (>200) — olası veri birleştirme hatası | BİLGİ | Analytics |
| STM_044 | Feed stop_times satır sayısı 2 milyonu aşıyor — WASM bellek/performans uyarısı | BİLGİ | Analytics |
| STM_045 | Seferin hareket saati servis günü penceresini aşıyor — olası veri anomalisi | ORTA | Quality |
| STM_046 | trip_id eksik | KRİTİK | Spec |
| STM_047 | Kesin zaman noktasında (timepoint=1) arrival_time/departure_time eksik | KRİTİK | Spec |
| STM_048 | Gece yarısı sonrası saatler 00:xx yazılmış (24:xx önerilir, duraklar arası) | BİLGİ | Quality |
| STM_049 | Gece yarısı sonrası kalkış 00:xx yazılmış (24:xx önerilir, aynı satır) | BİLGİ | Quality |
| STM_050 | timepoint sütunu mevcut ama satırda boş — değer açıkça belirtilmeli (0 yaklaşık / 1 kesin) | DÜŞÜK | Quality |
| STM_051 | Flex penceresi tanımlıyken pickup_type=0/3 yasak (talep-üzerine değerler gerekli) | YÜKSEK | Spec |
| STM_052 | Flex penceresi tanımlıyken drop_off_type=0 yasak (talep-üzerine değer gerekli) | YÜKSEK | Spec |
| STM_053 | Çok sayıda ardışık durakta aynı zaman | ORTA | Quality |
| STM_054 | Flex penceresi tanımlıyken continuous_pickup 1/boş dışında yasak | YÜKSEK | Spec |
| STM_055 | Flex penceresi tanımlıyken continuous_drop_off 1/boş dışında yasak | YÜKSEK | Spec |
| STM_057 | Flex konumu için tek stop_times kaydı | KRİTİK | Spec |
| STM_056 | shape_dist_traveled artmıyor | KRİTİK | Spec |

## PDW — Pickup/Drop-off Window

| Kural | Başlık | Önem | Sınıf |
|---|---|---|---|
| PDW_006 | Aynı trip+zone'da örtüşen pickup/drop-off penceresi | ORTA | Analytics |

## LOC — locations.geojson

| Kural | Başlık | Önem | Sınıf |
|---|---|---|---|
| LOC_001 | locations.geojson'da bilinmeyen veya geçersiz geometri tipi | YÜKSEK | Spec |
| LOC_002 | Feature'da geometry null veya eksik — GTFS Flex gerektiriyor | KRİTİK | Spec |
| LOC_003 | Feature'da 'id' property eksik — stop_times çapraz referansı için zorunlu | KRİTİK | Spec |
| LOC_004 | Polygon ring kapalı değil (ilk != son nokta) | ORTA | Spec |
| LOC_005 | FeatureCollection boş — hiç feature yok | DÜŞÜK | Quality |
| LOC_006 | Polygon yaklaşık kapsamı 500km²'yi aşıyor — gerçekçi olmayan Flex bölge (bounding-box tahmini) | ORTA | Quality |
| LOC_007 | FeatureCollection içinde yinelenen 'id' değeri — Flex referansı belirsizleşir | ORTA | Spec |
| LOC_008 | Feature 'type' alanı eksik veya "Feature" değil | ORTA | Spec |
| LOC_009 | Feature 'properties' nesnesi eksik | ORTA | Spec |
| LOC_010 | Geometry 'coordinates' eksik veya dizi değil — bölge geometrisi çözümlenemez | KRİTİK | Spec |

## CAL — Calendar (Takvim)

| Kural | Başlık | Önem | Sınıf |
|---|---|---|---|
| CAL_001 | service_id yineleniyor | KRİTİK | Spec |
| CAL_002 | Takvim gün alanı geçersiz değer | KRİTİK | Spec |
| CAL_003 | start_date eksik veya geçersiz format | KRİTİK | Spec |
| CAL_004 | end_date eksik veya geçersiz format | KRİTİK | Spec |
| CAL_005 | start_date end_date'den sonra | KRİTİK | Interop |
| CAL_006 | Haftalık bazda tüm günler pasif (calendar_dates ile override mümkün) | BİLGİ | Quality |
| CAL_007 | Servis döneminde boşluk | ORTA | Analytics |
| CAL_008 | Servis tarihi yakında sona eriyor | YÜKSEK | Analytics |
| CAL_009 | Feed'deki tüm takvim dönemleri sona ermiş | KRİTİK | Quality |
| CAL_010 | Serviste aktif gün sayısı çok az | ORTA | Analytics |
| CAL_011 | Kullanılmayan servis | DÜŞÜK | Quality |
| CAL_012 | Yakın gelecekte servis boşluğu var | BİLGİ | Analytics |
| CAL_013 | Geçmiş tarihli servis dönemi | BİLGİ | Analytics |
| CAL_014 | Servis tarihleri feed_info geçerlilik aralığı dışında | DÜŞÜK | Quality |
| CAL_015 | Tüm takvim tarihleri gelecekte (bugün aktif sefer yok) | DÜŞÜK | Quality |
| CAL_016 | Servis çok uzak bir gelecek tarihine kadar uzanıyor | BİLGİ | Quality |
| CAL_017 | Takvim henüz başlamamış (tüm aktif tarihler gelecekte) | DÜŞÜK | Quality |
| CAL_018 | Servisin aktif haftanın günü yok (tüm günler 0, calendar_dates ile geçersiz kılınan yok) | DÜŞÜK | Quality |
| CAL_019 | Ham takvim aralığı feed_info geçerlilik penceresini aşıyor | DÜŞÜK | Quality |
| CAL_020 | Feed geçerlilik penceresi 5 yılı aşıyor — gerçekçi olmayan zaman dilimi | DÜŞÜK | Quality |
| CAL_021 | Servis bugünü kapsıyor ama yakın günlerde aktif sefer yok | BİLGİ | Analytics |
| CAL_022 | service_id eksik | KRİTİK | Spec |
| CAL_023 | end_date çok ileri (şüpheli uzak-gelecek tarih) | ORTA | Quality |
| CAL_024 | Takvim önümüzdeki 7 günde aktif değil | DÜŞÜK | Quality |

## CLD — Calendar Dates

| Kural | Başlık | Önem | Sınıf |
|---|---|---|---|
| CLD_001 | service_id eksik | KRİTİK | Spec |
| CLD_002 | date eksik veya geçersiz format | KRİTİK | Spec |
| CLD_003 | exception_type eksik veya geçersiz | KRİTİK | Spec |
| CLD_004 | calendar_dates-only serviste aktif gün (exception_type=1) tanımlı değil | YÜKSEK | Quality |
| CLD_005 | Tarih makul yıl aralığı dışında | KRİTİK | Quality |
| CLD_006 | Çok fazla istisna günü | ORTA | Quality |
| CLD_007 | Aşırı takvim istisnası | BİLGİ | Analytics |

## SHP — Shapes (Güzergah Şekilleri)

| Kural | Başlık | Önem | Sınıf |
|---|---|---|---|
| SHP_001 | shape_id eksik | KRİTİK | Spec |
| SHP_002 | shape_pt_lat eksik veya geçersiz | KRİTİK | Spec |
| SHP_003 | shape_pt_lon eksik veya geçersiz | KRİTİK | Spec |
| SHP_004 | shape_pt_sequence eksik veya geçersiz | KRİTİK | Spec |
| SHP_005 | shape_dist_traveled geriye gidiyor | KRİTİK | Spec |
| SHP_006 | Güzergah şekli yalnızca tek noktadan oluşuyor | ORTA | Quality |
| SHP_008 | shape_pt_sequence yineleniyor | KRİTİK | Spec |
| SHP_009 | Güzergah şekli kendisiyle kesişiyor | BİLGİ | Analytics |
| SHP_010 | Tekrarlanan shape noktası (ardışık özdeş koordinat) | DÜŞÜK | Quality |
| SHP_012 | Güzergah şekli sefer duraklarından çok uzak | YÜKSEK | Analytics |
| SHP_014 | İlk veya son durak güzergah ucundan uzakta | BİLGİ | Analytics |
| SHP_015 | Güzergah şekli istatistiksel olarak çok az nokta | ORTA | Quality |
| SHP_016 | Güzergah şekli yön bilgisiyle uyumsuz | YÜKSEK | Quality |
| SHP_017 | Durak sırası güzergah şekliyle çelişiyor | BİLGİ | Analytics |
| SHP_018 | Güzergah şekli sefer tarafından referanslanmıyor | DÜŞÜK | Quality |
| SHP_019 | Güzergah şeklinin seferleri durak zamanı içermiyor | ORTA | Quality |
| SHP_020 | Güzergah şeklinde tekrarlayan nokta | BİLGİ | Analytics |
| SHP_021 | shape_dist_traveled negatif değer | DÜŞÜK | Quality |
| SHP_022 | Durak güzergah şeklinde belirsiz konumda | YÜKSEK | Quality |
| SHP_023 | shape_dist_traveled aynı değere sahip art arda iki nokta aynı koordinatta | ORTA | Quality |
| SHP_024 | Duraktan şekle mesafe shape_dist_traveled ile tutarsız | ORTA | Quality |
| SHP_025 | Sefer stop_times mesafesi şeklin toplam mesafesini aşıyor | ORTA | Quality |
| SHP_026 | Shape aşırı fazla noktaya sahip (>5000) — tüketici render performansını etkiler | BİLGİ | Analytics |
| SHP_028 | Ardışık iki shape noktası aynı shape_dist_traveled ama farklı koordinat (mesafe artmadan konum değişmiş) | YÜKSEK | Quality |
| SHP_029 | Aynı shape_dist_traveled, farklı ama çok yakın koordinatlı ardışık shape noktaları (eşik altı) | BİLGİ | Quality |

## FRQ — Frequencies (Frekanslar)

| Kural | Başlık | Önem | Sınıf |
|---|---|---|---|
| FRQ_001 | trip_id bulunamadı | KRİTİK | Spec |
| FRQ_002 | start_time geçersiz | KRİTİK | Spec |
| FRQ_003 | end_time geçersiz | KRİTİK | Spec |
| FRQ_004 | headway_secs eksik veya geçersiz | KRİTİK | Spec |
| FRQ_005 | end_time start_time'dan önce | KRİTİK | Quality |
| FRQ_006 | headway_secs çok uzun | ORTA | Analytics |
| FRQ_007 | exact_times geçersiz | ORTA | Spec |
| FRQ_008 | headway_secs sıfır (geçersiz frekans) | KRİTİK | Spec |
| FRQ_009 | Frekans aralığı çok kısa | ORTA | Quality |
| FRQ_010 | Çok sık frekans (sıkışma riski) | BİLGİ | Analytics |
| FRQ_011 | Aynı trip için frequencies dönemleri zaman aralığı çakışıyor | YÜKSEK | Interop |

## TRF — Transfers (Aktarmalar)

| Kural | Başlık | Önem | Sınıf |
|---|---|---|---|
| TRF_001 | from_stop_id eksik | KRİTİK | Spec |
| TRF_002 | to_stop_id eksik | KRİTİK | Spec |
| TRF_003 | from_stop_id veya to_stop_id bulunamadı | KRİTİK | Spec |
| TRF_004 | transfer_type geçersiz | YÜKSEK | Spec |
| TRF_005 | min_transfer_time eksik | KRİTİK | Spec |
| TRF_006 | from_trip_id bulunamadı | KRİTİK | Spec |
| TRF_007 | to_trip_id bulunamadı | KRİTİK | Spec |
| TRF_008 | from_route_id bulunamadı | KRİTİK | Spec |
| TRF_009 | to_route_id bulunamadı | KRİTİK | Spec |
| TRF_010 | Aktarma süresi çok uzun | ORTA | Analytics |
| TRF_011 | Aktarma tanımlandı ama mesafe uzak | BİLGİ | Quality |
| TRF_012 | Yinelenen aktarma kaydı | KRİTİK | Spec |
| TRF_013 | Aktarma türü bağlamla uyumsuz | KRİTİK | Quality |
| TRF_014 | in-seat aktarma için sefer yok | YÜKSEK | Spec |
| TRF_015 | in-seat aktarma geçersiz | YÜKSEK | Quality |
| TRF_016 | Aktarma koşulu çelişkili | KRİTİK | Spec |
| TRF_017 | Sefer aktarması yanlış hat | YÜKSEK | Interop |
| TRF_018 | Sefer aktarması aynı seferi gösteriyor | ORTA | Quality |
| TRF_019 | In-seat aktarmada farklı route_type | ORTA | Interop |
| TRF_020 | Aktarma için gereken yürüme hızı çok yüksek | ORTA | Quality |
| TRF_021 | Aktarma uç noktası durak veya istasyon değil | KRİTİK | Spec |

## GGL — Google Transit Uyumluluğu

| Kural | Başlık | Önem | Sınıf |
|---|---|---|---|
| GGL_001 | transfer_type=4/5 Google Transit tarafından desteklenmiyor | DÜŞÜK | Interop |
| GGL_002 | ic_price (Google-özel) geçersiz değer | DÜŞÜK | Interop |

## FAR — Fare Attributes (Ücret Özellikleri)

| Kural | Başlık | Önem | Sınıf |
|---|---|---|---|
| FAR_001 | fare_id yineleniyor | KRİTİK | Spec |
| FAR_002 | price eksik veya geçersiz | KRİTİK | Spec |
| FAR_003 | currency_type eksik | KRİTİK | Spec |
| FAR_004 | payment_method geçersiz | KRİTİK | Spec |
| FAR_005 | transfers geçersiz | KRİTİK | Spec |
| FAR_006 | transfer_duration geçersiz | ORTA | Spec |
| FAR_008 | agency_id bulunamadı | KRİTİK | Spec |
| FAR_009 | Ücrete ait hat kuralı yok | DÜŞÜK | Quality |
| FAR_010 | Çakışan ücret kuralları | ORTA | Quality |
| FAR_011 | payment_method eksik | KRİTİK | Spec |
| FAR_012 | fare_id eksik | KRİTİK | Spec |

## FRL — Fare Rules (Ücret Kuralları)

| Kural | Başlık | Önem | Sınıf |
|---|---|---|---|
| FRL_001 | fare_id bulunamadı | KRİTİK | Spec |
| FRL_002 | route_id bulunamadı | KRİTİK | Spec |
| FRL_003 | origin_id geçersiz | KRİTİK | Spec |
| FRL_004 | destination_id geçersiz | KRİTİK | Spec |
| FRL_005 | contains_id geçersiz | KRİTİK | Spec |
| FRL_006 | Ücret kuralı tanımlı değil | BİLGİ | Quality |
| FRL_007 | Ücret kuralı mantıksal tutarsızlık | ORTA | Quality |
| FRL_008 | Tüm hatlar için ücret tanımlı değil | BİLGİ | Quality |

## RCT — Rider Categories (Yolcu Kategorileri, Fares v2)

| Kural | Başlık | Önem | Sınıf |
|---|---|---|---|
| RCT_001 | rider_category_id yineleniyor | KRİTİK | Spec |
| RCT_002 | rider_category_name eksik | KRİTİK | Spec |
| RCT_003 | is_default_fare_category geçersiz | KRİTİK | Spec |
| RCT_004 | min_age veya max_age geçersiz (GTFS uzantı alanı — resmî spec'te yok) | ORTA | Quality |
| RCT_005 | max_age min_age'den küçük | ORTA | Quality |
| RCT_006 | fare_product başına birden fazla varsayılan yolcu kategorisi | ORTA | Spec |

## FMD — Fare Media (Ücret Medyası, Fares v2)

| Kural | Başlık | Önem | Sınıf |
|---|---|---|---|
| FMD_001 | fare_media_id yineleniyor | KRİTİK | Spec |
| FMD_002 | fare_media_type eksik veya geçersiz | KRİTİK | Spec |
| FMD_003 | TransitCard/MobileApp için fare_media_name tavsiye edilir | DÜŞÜK | Quality |

## FPD — Fare Products (Ücret Ürünleri, Fares v2)

| Kural | Başlık | Önem | Sınıf |
|---|---|---|---|
| FPD_001 | fare_product_id yineleniyor | KRİTİK | Spec |
| FPD_002 | amount eksik veya negatif | KRİTİK | Spec |
| FPD_003 | currency geçersiz ISO 4217 kodu | KRİTİK | Spec |
| FPD_004 | fare_media_id bulunamadı | KRİTİK | Spec |
| FPD_005 | rider_category_id bulunamadı | KRİTİK | Spec |
| FPD_006 | Bir fare_product için birden fazla varsayılan rider category | ORTA | Spec |

## FLG — Fare Leg Rules (Fares v2)

| Kural | Başlık | Önem | Sınıf |
|---|---|---|---|
| FLG_001 | fare_product_id bulunamadı | KRİTİK | Spec |
| FLG_002 | network_id bulunamadı | KRİTİK | Spec |
| FLG_003 | from_area_id bulunamadı | KRİTİK | Spec |
| FLG_004 | to_area_id bulunamadı | KRİTİK | Spec |
| FLG_005 | from_timeframe_group_id bulunamadı | KRİTİK | Spec |
| FLG_006 | to_timeframe_group_id bulunamadı | KRİTİK | Spec |
| FLG_007 | rule_priority geçersiz | ORTA | Spec |

## FTR — Fare Transfer Rules (Fares v2)

| Kural | Başlık | Önem | Sınıf |
|---|---|---|---|
| FTR_001 | fare_transfer_type eksik veya geçersiz | KRİTİK | Spec |
| FTR_002 | from_leg_group_id bulunamadı | KRİTİK | Spec |
| FTR_003 | to_leg_group_id bulunamadı | KRİTİK | Spec |
| FTR_004 | fare_product_id bulunamadı | KRİTİK | Spec |
| FTR_005 | duration_limit_type geçersiz | KRİTİK | Spec |
| FTR_006 | duration_limit geçersiz | ORTA | Spec |
| FTR_007 | duration_limit_type duration_limit olmadan tanımlı | ORTA | Spec |
| FTR_008 | transfer_count geçersiz | ORTA | Spec |
| FTR_009 | leg grupları aynıyken transfer_count zorunlu | ORTA | Spec |
| FTR_010 | leg grupları farklıyken transfer_count tanımlanamaz | ORTA | Spec |
| FTR_011 | duration_limit tanımlı ama duration_limit_type eksik | ORTA | Spec |

## ARS — Areas (Bölgeler, Fares v2)

| Kural | Başlık | Önem | Sınıf |
|---|---|---|---|
| ARS_001 | area_id yineleniyor | KRİTİK | Spec |

## SAR — Stop Areas (Durak Bölgeleri, Fares v2)

| Kural | Başlık | Önem | Sınıf |
|---|---|---|---|
| SAR_001 | area_id bulunamadı | KRİTİK | Spec |
| SAR_002 | stop_id bulunamadı | KRİTİK | Spec |

## NET — Networks (Ağlar, Fares v2)

| Kural | Başlık | Önem | Sınıf |
|---|---|---|---|
| NET_001 | network_id yineleniyor | KRİTİK | Spec |

## TFR — Timeframes (Zaman Dilimleri, Fares v2)

| Kural | Başlık | Önem | Sınıf |
|---|---|---|---|
| TFR_001 | timeframe_group_id eksik | KRİTİK | Spec |
| TFR_002 | service_id bulunamadı | KRİTİK | Spec |
| TFR_003 | start_time veya end_time format hatası | YÜKSEK | Spec |
| TFR_004 | end_time start_time'dan küçük | ORTA | Quality |
| TFR_005 | Aynı grup ve service_id içinde örtüşen zaman aralıkları | ORTA | Interop |
| TFR_006 | start_time veya end_time 24:00:00'dan büyük | KRİTİK | Spec |
| TFR_007 | start_time ve end_time yalnızca biri tanımlı | KRİTİK | Spec |

## PTH — Pathways (Geçitler)

| Kural | Başlık | Önem | Sınıf |
|---|---|---|---|
| PTH_001 | pathway_id yineleniyor | KRİTİK | Spec |
| PTH_002 | from_stop_id bulunamadı | KRİTİK | Spec |
| PTH_003 | to_stop_id bulunamadı | KRİTİK | Spec |
| PTH_004 | pathway_mode eksik veya geçersiz | KRİTİK | Spec |
| PTH_005 | is_bidirectional eksik | KRİTİK | Spec |
| PTH_006 | length geçersiz | ORTA | Spec |
| PTH_007 | traversal_time geçersiz | ORTA | Spec |
| PTH_008 | stair_count eksik | DÜŞÜK | Quality |
| PTH_009 | max_slope eksik | DÜŞÜK | Quality |
| PTH_010 | min_width geçersiz | DÜŞÜK | Spec |
| PTH_011 | Geçit döngü oluşturuyor | YÜKSEK | Quality |
| PTH_012 | İstasyona erişilebilir yol yok | YÜKSEK | Interop |
| PTH_013 | Erişilebilir yol analizi | BİLGİ | Analytics |
| PTH_014 | Geçit istasyon sınırını aşıyor | KRİTİK | Quality |
| PTH_015 | Geçit hedefi erişilemeyen durakta | ORTA | Analytics |
| PTH_016 | Çıkış kapısı çift yönlü tanımlanmış | YÜKSEK | Spec |
| PTH_017 | max_slope geçersiz bağlam | ORTA | Spec |
| PTH_018 | signposted_as çok uzun | DÜŞÜK | Quality |
| PTH_019 | Generic node tek pathway'e bağlı (dead-end) | ORTA | Quality |
| PTH_020 | pathway_id eksik | KRİTİK | Spec |
| PTH_021 | from_stop_id eksik | KRİTİK | Spec |
| PTH_022 | to_stop_id eksik | KRİTİK | Spec |
| PTH_023 | pathway_mode eksik | KRİTİK | Spec |
| PTH_024 | is_bidirectional eksik | KRİTİK | Spec |
| PTH_025 | Önerilen pathway length bilgisi eksik | DÜŞÜK | Quality |
| PTH_026 | Pathway uç noktası istasyon | KRİTİK | Spec |

## LVL — Levels (Katlar)

| Kural | Başlık | Önem | Sınıf |
|---|---|---|---|
| LVL_001 | level_id yineleniyor | KRİTİK | Spec |
| LVL_002 | level_index geçersiz | KRİTİK | Spec |
| LVL_003 | level_name eksik | DÜŞÜK | Quality |
| LVL_004 | Kullanılmayan level | DÜŞÜK | Quality |
| LVL_005 | level_name çok uzun | ORTA | Quality |
| LVL_006 | Asansör bağlantısındaki durakta level_id eksik | ORTA | Quality |
| LVL_007 | level_index eksik | KRİTİK | Spec |
| LVL_008 | level_id eksik | KRİTİK | Spec |

## FIN — Feed Info

| Kural | Başlık | Önem | Sınıf |
|---|---|---|---|
| FIN_001 | feed_publisher_name eksik | KRİTİK | Spec |
| FIN_002 | feed_publisher_url eksik veya geçersiz | KRİTİK | Spec |
| FIN_003 | feed_lang eksik | KRİTİK | Spec |
| FIN_004 | default_lang geçersiz | ORTA | Spec |
| FIN_005 | feed_start_date geçersiz | ORTA | Spec |
| FIN_006 | feed_end_date geçersiz format (geçmiş tarih için FIN_010 kullanın) | YÜKSEK | Spec |
| FIN_007 | feed_version eksik | DÜŞÜK | Quality |
| FIN_008 | feed_contact_email geçersiz | DÜŞÜK | Spec |
| FIN_009 | feed_contact_url geçersiz | DÜŞÜK | Spec |
| FIN_010 | Feed geçerlilik süresi dolmuş | YÜKSEK | Analytics |
| FIN_012 | feed_start_date feed_end_date'den sonra | DÜŞÜK | Quality |
| FIN_013 | fare_attributes.agency_id önerilen ama eksik | BİLGİ | Quality |
| FIN_014 | Feed geçerlilik tarihleri (feed_start_date/feed_end_date) eksik | DÜŞÜK | Quality |
| FIN_015 | Birden fazla feed_info kaydı | ORTA | Quality |
| FIN_016 | feed_start_date gelecekte (feed henüz aktif değil) | DÜŞÜK | Quality |
| FIN_017 | Feed çok uzak gelecekte sona eriyor | BİLGİ | Quality |
| FIN_018 | feed_contact_email ve feed_contact_url ikisi de eksik | DÜŞÜK | Quality |
| FIN_019 | Feed'in geçerlilik süresi 7 gün içinde dolacak | DÜŞÜK | Quality |
| FIN_020 | Feed geçerlilik penceresi 7 günden kısa — operasyonel kullanım için çok kısa | ORTA | Quality |

## TRN — Translations (Çeviriler)

| Kural | Başlık | Önem | Sınıf |
|---|---|---|---|
| TRN_001 | table_name geçersiz değer | KRİTİK | Spec |
| TRN_002 | field_name bu tablo için geçersiz | KRİTİK | Spec |
| TRN_003 | language geçersiz | ORTA | Spec |
| TRN_004 | record_id bulunamadı | YÜKSEK | Spec |
| TRN_005 | Çeviri yineleniyor | KRİTİK | Spec |
| TRN_006 | Çeviri kaydı çelişkili | KRİTİK | Spec |
| TRN_007 | Çeviri feed_lang ile aynı dilde | DÜŞÜK | Quality |
| TRN_008 | translation değeri boş | BİLGİ | Quality |
| TRN_009 | record_id ve field_value aynı anda kullanılamaz | YÜKSEK | Spec |
| TRN_010 | record_sub_id geçersiz | YÜKSEK | Spec |
| TRN_011 | field_name çevrilebilir değil | YÜKSEK | Spec |
| TRN_013 | feed_info çevirisinde kimlik alanı kullanılamaz | YÜKSEK | Spec |
| TRN_014 | record_sub_id yalnızca stop_times için geçerli | YÜKSEK | Spec |

## ATR — Attributions (Atıflar)

| Kural | Başlık | Önem | Sınıf |
|---|---|---|---|
| ATR_001 | attribution_id eksik | YÜKSEK | Quality |
| ATR_002 | organization_name eksik | KRİTİK | Spec |
| ATR_003 | Attribution rolü tanımlanmamış | YÜKSEK | Spec |
| ATR_004 | is_producer geçersiz | KRİTİK | Spec |
| ATR_005 | is_operator geçersiz | KRİTİK | Spec |
| ATR_006 | is_authority geçersiz | KRİTİK | Spec |
| ATR_007 | attribution_url geçersiz | KRİTİK | Spec |
| ATR_008 | attribution_email geçersiz | DÜŞÜK | Spec |
| ATR_009 | attribution hedef alanları (agency/route/trip) birlikte kullanılmış | YÜKSEK | Quality |
| ATR_010 | agency_id bulunamadı | DÜŞÜK | Spec |

## XFL — Cross-File / Semantik

| Kural | Başlık | Önem | Sınıf |
|---|---|---|---|
| XFL_002 | Seferin stop_times kaydı yok | YÜKSEK | Interop |
| XFL_006 | service_id yalnızca iptal istisnası içeriyor (aktif gün yok) | ORTA | Analytics |
| XFL_011 | Takvim tarihleri feed_info aralığı dışında | ORTA | Interop |
| XFL_012 | Çalıştırılabilir seferi olmayan hat (stop_times veya aktif servis bağlamı eksik) | YÜKSEK | Quality |
| XFL_013 | shape_id birden fazla yönde kullanılıyor | YÜKSEK | Quality |
| XFL_014 | Geçersiz çeviri referansı (kaynak kayıt bulunamadı) | ORTA | Quality |
| XFL_015 | Attribution'da geçersiz referans | KRİTİK | Spec |
| XFL_016 | Çeviri feed_info'ya referans veriyor ama feed_info.txt eksik | YÜKSEK | Spec |
| XFL_017 | route_cemv_support ile agency_cemv_support çelişiyor | DÜŞÜK | Quality |
| XFL_019 | Ağ tanımı iki ayrı dosyada (routes.network_id + route_networks.txt) | ORTA | Spec |
| XFL_020 | Transfers'de geçersiz (from_trip_id/to_trip_id, route_id) çifti | KRİTİK | Spec |
| XFL_021 | Transfers'de geçersiz (from_trip_id/to_trip_id, stop_id) çifti | YÜKSEK | Interop |
| XFL_022 | location_group_id bulunamadı (location_group_stops) | KRİTİK | Spec |
| XFL_023 | stop_id bulunamadı (location_group_stops) | KRİTİK | Spec |
| XFL_024 | location_group_id bulunamadı (stop_times) | KRİTİK | Spec |
| XFL_025 | location_id bulunamadı (locations.geojson) | KRİTİK | Spec |
| XFL_031 | Kimlik çakışması: stop_id / locations.geojson id / location_group_id ortak isim alanını paylaşır | KRİTİK | Spec |
| XFL_026 | route cemv_support=1 ama uygulanabilir contactless fare product yok | ORTA | Quality |
| XFL_027 | route cemv_support=2 ama uygulanabilir contactless fare product var | ORTA | Quality |
| XFL_028 | agency cemv_support=1 ama Fares v2'de contactless media yok | BİLGİ | Quality |
| XFL_029 | route cemv_support=1 ama Fares v2'de contactless media yok | BİLGİ | Quality |
| XFL_030 | contactless fare media var ama hiç cemv_support=1 yok | BİLGİ | Quality |

## OPR — Operasyonel Tutarlılık

| Kural | Başlık | Önem | Sınıf |
|---|---|---|---|
| OPR_001 | Hat sefer sıklığı düşük | ORTA | Analytics |
| OPR_003 | Sefer sıkışması (minimum aralık çok küçük) | DÜŞÜK | Analytics |
| OPR_004 | Hafta sonu sefer yok | BİLGİ | Analytics |
| OPR_005 | Sıradışı sefer sıklığı | BİLGİ | Analytics |
| OPR_006 | Seferde çok az durak (işlevsel değil) | YÜKSEK | Analytics |
| OPR_007 | Sefer içinde tekrarlayan durak deseni | BİLGİ | Analytics |
| OPR_008 | Birden fazla segmentte aşırı hız | YÜKSEK | Analytics |
| OPR_009 | Gece seferi başlangıç saati çok geç | BİLGİ | Analytics |
| OPR_010 | Hatta erişilebilirlik veya bisiklet politikası çelişiyor | ORTA | Analytics |
| OPR_011 | Serviste aktif gün yok | YÜKSEK | Analytics |
| OPR_012 | Servis boşluğu | ORTA | Analytics |
| OPR_013 | Hat tek yönde işliyor (karşı yön tanımlı değil) | BİLGİ | Analytics |
| OPR_014 | Ortalama aktarma süresi uzun | ORTA | Analytics |
| OPR_015 | Hat yalnızca tek güzergahla işliyor | BİLGİ | Analytics |
| OPR_016 | Feed genelinde aktif servis yok | BİLGİ | Analytics |
| OPR_017 | Sefer çok kısa mesafe | ORTA | Analytics |
| OPR_019 | Rota takvim çakışması (aynı günde birden fazla servis) | BİLGİ | Analytics |
| OPR_020 | Rota exception günü çakışması | DÜŞÜK | Analytics |
| OPR_021 | Takvim override çakışması: override ve base eş zamanlı aktif | YÜKSEK | Analytics |
| OPR_022 | Takvim override uygulanmamış: override gününde base servis çalışıyor | YÜKSEK | Analytics |
| OPR_023 | Takvim override boşluğu: pencere içinde hiçbir servis aktif değil | ORTA | Analytics |
| OPR_024 | Hat çok fazla sefer içeriyor — olası veri birleştirme sorunu | BİLGİ | Analytics |
| OPR_025 | Feed genelinde ortalama sefer süresi 60 saniyeden kısa — veri kalitesi sorunu | YÜKSEK | Analytics |

## GEO — Coğrafi / Uzamsal

| Kural | Başlık | Önem | Sınıf |
|---|---|---|---|
| GEO_002 | Durak feed medianından çok uzakta | YÜKSEK | Analytics |
| GEO_006 | Güzergah şeklinde büyük atlama | YÜKSEK | Analytics |
| GEO_007 | Güzergah şeklinde kritik atlama (3× eşik) | YÜKSEK | Analytics |
| GEO_009 | Durak shape güzergahından çok uzakta | YÜKSEK | Quality |
| GEO_012 | Duraksallar kümelenmesi (çok yakın duraklar) | ORTA | Analytics |
| GEO_013 | Feed coğrafi kapsam özeti | BİLGİ | Analytics |
| GEO_014 | Feed coğrafi kapsamı çok geniş | BİLGİ | Analytics |
| GEO_015 | Durak koordinatları Japonya sınırları dışında (feed_lang: ja) | ORTA | Quality |
| GEO_016 | Durak Null Island yakınında (\|lat\|<1 VE \|lon\|<1) — olası koordinat hatası | YÜKSEK | Quality |
| GEO_017 | Shape noktası Null Island yakınında — GPS verisi hatası | YÜKSEK | Quality |
| GEO_018 | Tüm feed durağları 200m yarıçap içinde — test/yer tutucu veri | YÜKSEK | Analytics |
| GEO_019 | Durak koordinatları tam sayı (ondalık basamak yok) — düşük hassasiyetli veya yer tutucu | ORTA | Quality |
| GEO_020 | Shape'in tüm noktaları aynı koordinatta — dejenere geometri | YÜKSEK | Quality |
| GEO_021 | Durakların %30'undan fazlası koordinatlarını başka bir durakla paylaşıyor — sistematik hata | YÜKSEK | Analytics |
| GEO_022 | Durak enlemi kutba aşırı yakın (\|lat\|>89) — olası koordinat hatası | YÜKSEK | Quality |

## DQ — Veri Kalitesi / Kullanıcı Deneyimi

| Kural | Başlık | Önem | Sınıf |
|---|---|---|---|
| DQ_003 | Hat açıklaması eksik | BİLGİ | Quality |
| DQ_004 | Hat URL'si eksik | BİLGİ | Quality |
| DQ_005 | Geçerli servis dönemi yok | YÜKSEK | Quality |
| DQ_005b | Hiçbir seferin durak zamanı yok | YÜKSEK | Quality |
| DQ_005c | Koordinatsız durak oranı çok yüksek | YÜKSEK | Quality |
| DQ_006 | Şekil olmayan sefer oranı çok yüksek | YÜKSEK | Quality |
| DQ_009 | Seferlerde durak zamanı yok | BİLGİ | Quality |
| DQ_010 | Acente hiçbir hatta kullanılmıyor | BİLGİ | Quality |
| DQ_011 | Yalnızca bir durak var | DÜŞÜK | Quality |
| DQ_012 | Çok fazla acente, agency_id kullanılmıyor | DÜŞÜK | Quality |
| DQ_013 | Çok az sefer | ORTA | Quality |
| DQ_016 | Değerde fazladan boşluk karakteri | ORTA | Quality |
| DQ_017 | Şüpheli koordinat değeri | BİLGİ | Quality |
| DQ_018 | Önerilen alanda tamamen büyük harf (all-caps) | ORTA | Quality |
| DQ_019 | Önerilen alanda tamamen küçük harf (all-lowercase) | ORTA | Quality |
| DQ_020 | Önerilen alan eksik veya boş | DÜŞÜK | Quality |
| DQ_021 | Birincil anahtar yineleniyor — genel ikincil sinyal (STP_001/RTS_001 gibi entity-level kurallarla örtüşebilir) | YÜKSEK | Spec |
| DQ_022 | Durakların %80'inden fazlası aynı stop_name değerini paylaşıyor — yer tutucu/test verisi | YÜKSEK | Quality |

## VAT — Varlık Analitik Tespiti

| Kural | Başlık | Önem | Sınıf |
|---|---|---|---|
| VAT_001 | Hat güzergah benzerliği (muhtemel kopya hat) | ORTA | Analytics |
| VAT_002 | Aktarma merkezi tanımsız — çok sayıda hat geçiyor ama aktarma yok | BİLGİ | Analytics |
| VAT_003 | Sefer süresi istatistiksel aykırı değer | DÜŞÜK | Analytics |
| VAT_005 | İzole durak kümesi — ağ grafiğinde ana bileşenden kopuk duraklar | ORTA | Analytics |
| VAT_006 | Hizmet yoğunluğu dengesizliği — tek hat feed sefer sayısının büyük bölümünü oluşturuyor | BİLGİ | Analytics |
| VAT_007 | Terminus aktarma eksikliği — terminal durağa başka hat geliyor ama aktarma tanımlı değil | BİLGİ | Analytics |
| VAT_008 | Aynı shape feed hatlarının %30'undan fazlasında kullanılıyor — olası yanlış shape ataması | BİLGİ | Analytics |

## JPN

| Kural | Başlık | Önem | Sınıf |
|---|---|---|---|
| JPN_001 | GTFS-JP: durak adının kana (ja-Hrkt) okuması eksik | ORTA | Quality |
| JPN_002 | GTFS-JP: jp_office_id office_jp.txt'te tanımlı değil | YÜKSEK | Interop |
| JPN_003 | GTFS-JP: agency_jp.agency_id agency.txt'te tanımlı değil | YÜKSEK | Interop |
| JPN_004 | GTFS-JP: translations.txt eksik (profil zorunlu kılar) | YÜKSEK | Interop |
| JPN_005 | GTFS-JP: office_jp.office_name boş (zorunlu alan) | YÜKSEK | Interop |
| JPN_006 | GTFS-JP: fare_attributes/fare_rules eksik (profil zorunlu kılar) | ORTA | Quality |
| JPN_007 | GTFS-JP: feed_info.txt eksik (profil zorunlu kılar) | ORTA | Quality |
| JPN_008 | GTFS-JP: hat adının (route_long_name) kana (ja-Hrkt) okuması eksik | ORTA | Quality |
| JPN_009 | GTFS-JP: trip_headsign kana (ja-Hrkt) okuması eksik | ORTA | Quality |
| JPN_010 | GTFS-JP: işletici adının (agency_name) kana (ja-Hrkt) okuması eksik | ORTA | Quality |
| JPN_011 | GTFS-JP: tek işletici olsa bile agency_id zorunlu | YÜKSEK | Interop |
