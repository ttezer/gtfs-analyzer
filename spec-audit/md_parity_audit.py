#!/usr/bin/env python3
"""
MD ↔ bizim golden parite denetimi — 20-feed kampanyası için KANONİK araç.

Kullanım:
    python3 md_parity_audit.py <BASE_DIR>
BASE_DIR altında her feed için bir alt klasör; her alt klasörde:
    - MD report json  (içinde 'notices' listesi, her biri {'code','totalNotices','severity','sampleNotices'})
    - bizim golden json (içinde 'validation' anahtarı; rule_counts + scores)
Dosyalar İÇERİĞE göre ayrılır (isim şablonundan bağımsız), böylece
'spelunca_report_8.0.1.json' de 'BART_MD_report_8.0.1.json' de çalışır.

Çıktı (BASE_DIR'e yazılır):
    parity_all.csv   — satır = (feed, kod); B bölümü kolonları
    parity_md_only.csv — MD emit ediyor, MAP karşılığı yok (KAPSAM EKSİĞİ review kuyruğu)
    parity_summary.md  — 20-feed AGREGAT + öne çıkanlar

NOT (sınır): golden AGREGAT snapshot'tır → bizim per-notice ÖRNEKLERİMİZ yoktur.
MD örnekleri vardır. Sapma çıkan kodlarda bizim tarafı incelemek için feed zip'ini sakla.
"""
import csv
import glob
import json
import os
import sys
from collections import Counter, defaultdict

from md_parity_mapping import BY_DESIGN, classify_unmapped, resolve_mapping

# ── KANONİK MAP: MD kodu → bizim kural(lar) ──────────────────────────────────
# Bu oturumda (2026-07-02/03) doğrulanan eşlemeler. MAP'te OLMAYAN MD kodu
# otomatik "MD-ONLY" kovasına düşer = kapsam-eksiği review kuyruğu (bilerek).
# Yeni eşleme eklerken: önce kuralın GERÇEKTEN aynı şeyi ölçtüğünü doğrula.
MAP = {
    # ── Archive / dosya-sütun yapısı ──
    "missing_required_column":  ["ARC_025"],
    "missing_recommended_file": ["ARC_020"],
    "unknown_file":             ["ARC_007"],   # NOT: GTFS-JP agency_jp/office_jp → biz tanırız, MD bilmez
    "unknown_column":           ["ARC_017"],   # NOT: jp_* sütunları → biz tanırız
    "invalid_row_length":        ["ARC_012"],
    "missing_required_file":     ["ARC_004"],
    "missing_calendar_and_calendar_date_files": ["ARC_008"],
    "invalid_input_files_in_subfolder": ["ARC_024"],
    # ── Routes ──
    "duplicate_route_name":     ["RTS_019"],   # HEM short HEM long aynı. Tek-alan → RTS_026/027 (US-only, MD'de yok)
    "same_route_and_agency_url":["RTS_020"],
    # ── Stops ──
    "same_name_and_description_for_stop": ["STP_031"],
    "stop_without_stop_time":   ["STP_020"],
    # ── Calendar ──
    "big_gap_in_service":       ["CAL_007", "CAL_012"],  # PER-SERVICE, eşik big_gap_days=14
    # CAL_013 servisin BİRLEŞİK aktif tarih kümesine bakar (calendar + calendar_dates);
    # MD ise calendar.txt satırının end_date'ine bakar. İstisna günleri ileri tarihliyse
    # birleşik küme geçmişte DEĞİLDİR ve CAL_013 haklı olarak susar. Vaka yine görülüyor:
    # koşumda bu koda düşen 6 feed'in 6'sı da CAL_024 ("önümüzdeki 7 günde aktif değil")
    # alıyor. İkisi birlikte eşlenir (#146).
    "expired_calendar":         ["CAL_013", "CAL_024"],
    # #123: whitespace-wrapped weekday zeros are parsed for the weekly decision while
    # DQ_016 remains the lexical root; mdb-2830 replay recovered MD's 12 services.
    "service_has_no_active_day_of_the_week": ["CAL_006"],
    "service_extends_far_in_the_future":     ["CAL_023"],  # EŞİK FARKI: biz max_calendar_future_years=3 (yıl-granüler, tutucu); MD ~1-2y daha agresif → 2028'i işaretler biz etmeyiz. Configurable, by-design.
    "service_window_outside_feed_period":    ["CAL_014", "CAL_019"],
    # ── Shapes ──
    "decreasing_shape_distance":            ["SHP_005"],   # sequence-sıralı monotonluk
    "stop_too_far_from_shape":              ["SHP_012"],   # geometrik en-yakın-nokta
    "stop_too_far_from_shape_using_user_distance": ["SHP_024"],  # DÜZELTİLDİ: shape_dist_traveled konumu (SHP_012 değil). 20-feed: SHP_024=479 vs MD 721 (~eşleşme)
    "stops_match_shape_out_of_order":       ["SHP_017"],
    "unused_shape":                         ["SHP_018"],
    "equal_shape_distance_same_coordinates":["SHP_023"],   # tekrar-nokta
    "equal_shape_distance_diff_coordinates":["SHP_028"],   # eşik ÜSTÜ = gerçek hata
    "equal_shape_distance_diff_coordinates_distance_below_threshold": ["SHP_029"],  # eşik ALTI = yuvarlama
    "trip_with_shape_dist_traveled_but_no_shape_distances": ["SHP_030"],
    "single_shape_point":          ["SHP_006"],  # near-parity: yalnız kullanılan shape'ler
    # fare_products.txt::amount para birimi ondalık hassasiyetine uymuyor. Eşlemesizdi ve
    # 11 feed "kuralımız yok" gibi görünüyordu; koşumda 11'in 11'i de FPD_007 alıyor (#146).
    "invalid_currency_amount":     ["FPD_007", "FAR_013"],  # FAR_013: price ISO 4217 ondalık basamak sayısını taşımıyor (#165)
    # Aşağıdaki altı kod eşlemesizdi ve "kuralımız yok" gibi sayılıyordu; her biri tam
    # katalog koşumunda DOĞRULANDI — adayın o feed'lerde gerçekten ateşlediği ölçüldü (#146).
    "missing_required_agency_id":       ["AGN_011"],  # 9/9 feed
    "missing_prior_notice_duration_min": ["BKR_007"],  # 10/10 feed
    "feed_info_lang_and_agency_lang_mismatch": ["AGN_013"],  # 12/12 feed
    "missing_trip_edge":                ["STM_015", "STM_016"],  # ilk/son durak arrival/departure
    "empty_file":                       ["ARC_009"],  # "dosyada veri satırı yok" — 8/8 feed
    "invalid_color":                    ["RTS_006", "RTS_007"],  # route_color / route_text_color
    # STP_003/STP_005 GEÇERSİZ değeri ölçer (parse edilebiliyorsa aralık kontrolü);
    # BOŞ koordinatın sahibi STP_006/STP_007'dir. 7/7 feed doğrulandı — boşluk YOKTU,
    # yalnız eşleme eksikti.
    "stop_without_location":            ["STP_006", "STP_007"],
    # ── #147 turu: eşlemesiz 26 kodun 13'ü, her biri korpusta DOĞRULANDI (#155).
    # Yöntem: aday kuralı ada göre değil, MD'nin raporladığı feed'lerde GERÇEKTEN
    # ateşleyip ateşlemediğine göre seç. Ateşlemiyorsa eşleme YAZILMAZ.
    "route_both_short_and_long_name_missing": ["RTS_003"],      # 2/2 feed
    "missing_stop_name":                ["STP_003"],            # 2/2
    "inconsistent_agency_timezone":     ["AGN_005"],            # 2/2
    "invalid_language_code":            ["AGN_006"],            # 1/1
    "fast_travel_between_far_stops":    ["STM_061"],  # >10 km ayrık, ardışık OLMAYAN çift — #168'de yazıldı
    "invalid_prior_notice_duration_min": ["BKR_006"],  # birebir (#165)
    "forbidden_prior_notice_start_day": ["BKR_024"],   # MD: "forbidden when prior_notice_duration_max is set" (#165)
    "unsupported_geometry_type":        ["LOC_001"],   # locations.geojson geometri tipi (#165)
    "fare_transfer_rule_duration_limit_without_type": ["FTR_011"],  # birebir (#165)
    "fare_product_with_multiple_default_rider_categories": ["RCT_006"],  # birebir (#165)
    "forbidden_prior_day_booking_field_value": ["BKR_001"],  # önceki gün rezervasyon alanı yasak (#165)
    "transfer_with_invalid_trip_and_route": ["TRF_017", "XFL_020"],  # XFL_020 aynı olgunun (trip,route) çifti (#165)
    "invalid_timezone":                 ["AGN_004"],            # 1/1
    "inconsistent_agency_lang":         ["AGN_017", "AGN_013"], # 4/4 ikisi de
    "new_line_in_value":                ["ARC_030"],            # 3/3 — sekme/CR/LF yasağı
    "translation_unknown_table_name":   ["TRN_001"],            # 2/2
    "translation_foreign_key_violation": ["TRN_016"],  # "field_value hiçbir kayıtla eşleşmiyor" — MD: record_id/record_sub_id ile varlık bulunamıyor (#165)
    "translation_unexpected_value":     ["TRN_013"],  # "bu bağlamda dolu olmamalı" — MD: "a field has value but must be empty" (#165)
    "stop_time_with_only_arrival_or_departure_time": ["STM_034"],  # 1/1
    "missing_prior_notice_last_time":   ["BKR_009"],            # 1/1
    "empty_column_name":                ["ARC_017"],            # 5/5 — boş başlık tanınmayan sütundur
    "missing_pickup_or_drop_off_window": ["STM_039", "PDW_006"],# 1/1 ikisi de
    "invalid_pickup_drop_off_window":   ["STM_038"],            # ters/sıfır pencere; PDW_006 ÖRTÜŞME ölçer, bu değil
    # İlk aday turunda "hiçbiri" çıkmıştı çünkü YANLIŞ aday seçmiştim (FTR_002 =
    # from_leg_group_id bulunamadı, FK kuralı). Doğru kurallar leg-grubu koşulunu
    # ölçenler: aynıysa transfer_count ZORUNLU, farklıysa YASAK.
    "fare_transfer_rule_without_transfer_count":       ["FTR_009"],  # 4/4
    "fare_transfer_rule_with_forbidden_transfer_count": ["FTR_010"], # 2/2
    "missing_level_id":                 ["LVL_006"],                 # 1/1 — asansör bağlantısı bağlamı
    # ÜÇÜNCÜ aday turu (#147). Yine "hiçbiri ateşlemiyor" sonucu ADAY listesinden
    # geliyordu: kurallar vardı, ben yanlış grupta ya da yanlış kimlikte aramıştım.
    "invalid_email":                    ["FIN_008"],   # 1/1 — feed_contact_email; FIN_017/AGN_009 denemiştim
    "attribution_without_role":         ["ATR_003"],   # 3/3 — "Attribution rolü tanımlanmamış"
    "invalid_geometry":                 ["LOC_011"],   # 2/2 — ring kendini kesiyor / delik dışarıda
    "bidirectional_exit_gate":          ["PTH_016"],   # 1/1
    # ── Trips / bloklar ──
    "trip_headsign_matches_intermediate_stop": ["TRP_020"],  # AGG: per-trip (MD per-occurrence)
    "block_trips_with_overlapping_stop_times": ["TRP_022"],  # MD ERROR; bizde ~eşleşir (aynı-gün blok)
    # ── Stop times ──
    "missing_timepoint_value":  ["STM_050"],   # AGG: feed-özeti (MD per-row)
    "decreasing_or_equal_stop_time_distance": ["STM_056"],  # MD ERROR ↔ stop_times.shape_dist_traveled monotonluğu
    # DÜZELTİLDİ 2026-07-17: MD'nin TEK kodunu bizim İKİ kuralımız 700 km/h
    # çizgisinden böler → STM_012 (>700, "fiziksel imkânsız", emit edip `continue`
    # ile STM_014'ü gölgeler) + STM_014 (eşik..700). Yalnız STM_014'e eşlemek
    # 963 km/h'lik segmentleri "kaçırdık" gösteriyordu. Kanıt: mdb-2015 bizim
    # STM_012=78 ↔ MD 78 (birebir), mdb-3108 8 ↔ 8.
    "fast_travel_between_consecutive_stops": ["STM_014", "STM_012"],
    # ── Quality / feed_info ──
    "missing_recommended_field":["RTS_025", "FIN_007", "FIN_013", "FIN_018"],  # Generic kod; routes/fare/feed_info alanı sample context ile çözülür.
    "mixed_case_recommended_field": ["DQ_018"],  # biz yalnız ALL-CAPS (bilinçli)
    "missing_feed_contact_email_and_url": ["FIN_018"],
    # ── test2 (2. corpus) eşlemeleri ──
    "duplicate_key":                  ["SHP_008"],   # shapes PK (shape_id,seq). BY-DESIGN agg: K2 6159 emit eder ama pipeline (rule,shape_id) başına dedup → 6 distinct shape (mdb-1935). MD per-row (6159). Bug DEĞİL.
    "foreign_key_violation":          ["STM_002"],   # stop_times.stop_id→stops. BY-DESIGN agg: STM_002 distinct stop_id (4), MD per-occurrence (2886).
    "same_name_and_description_for_route": ["RTS_023"],
    # ── 2026-07-03 20-feed taramasından eklenen eşlemeler ──
    "non_ascii_or_non_printable_char": ["ARC_021"],   # BİZ-DOĞRU: MD meşru UTF-8'i (Japonca vb.) işaretler; ARC_021 yalnız non-printable → 0 doğru
    "unsorted_stop_times":            ["STM_036"],
    "leading_or_trailing_whitespaces":["DQ_016"],
    "missing_bike_allowance":         ["TRP_021"],
    "route_short_name_too_long":      ["RTS_010"],     # RTS_010=MD birebir; RTS_021(Google 6-char) US-ONLY daha sıkı
    "route_long_name_contains_short_name": ["RTS_022"],
    "trip_coverage_not_active_for_next7_days": ["TRP_023"],
    "future_calendar":                ["CAL_017"],
    "future_feed":                    ["CAL_017"],
    # MD tek kodla İKİ hâli birden sayar: süresi DOLMUŞ ve yakında DOLACAK. Bizde ayrık
    # iki kural var (FIN_010 dolmuş · FIN_019 pencere içinde doluyor), o yüzden eşleme
    # ikisine birden gider. Tek başına FIN_019'a bağlamak 841 feed'i sahte MISS yapıyordu:
    # tam katalog koşumunda 841'in 837'si FIN_010 alıyor, 0'ı FIN_019 — MD'nin kendi
    # örneklerinde de 841'in 839'u ZATEN DOLMUŞ feed'ler (#146).
    "feed_expiration_date7_days":     ["FIN_010", "FIN_019"],
    "same_stop_and_agency_url":       ["STP_034"],
    "same_stop_and_route_url":        ["STP_035"],
    "platform_without_parent_station":["STP_032"],     # BİZ-TUTUCU: MD HER parentsiz lt=0 durağı INFO işaretler (mdb-981: 1632=tüm duraklar, gürültü). Biz yalnız pathway-bağlı platformu (STP_032) işaretleriz — standalone durak parentsiz NORMAL.
    # ── 20-feed 2. tur eşlemeleri (kuralımız VAR, MAP eksikti) ──
    "route_color_contrast":           ["RTS_008"],     # over: biz 132 vs MD 3 (kontrast eşiği farkı)
    # DÜZELTİLDİ 2026-07-17: MD'nin tek kodunu bizim İKİ kuralımız durak SAYISINA göre böler:
    # 0 durak → XFL_002 ("seferin stop_times kaydı yok"), 1 durak → STM_033 ("tek duraklı").
    # Yalnız STM_033'e eşlemek sıfır-duraklı seferleri "kaçırdık" gösteriyordu. Kanıt:
    # mdb-2015/2036 XFL_002=13 ↔ MD 13, mdb-2381 XFL_002=35 ↔ MD 35 — hepsi BİREBİR
    # (MD örneklerinin 13/13'ü gerçekten 0 stop_times'lı doğrulandı).
    "unusable_trip":                  ["STM_033", "XFL_002"],
    # DÜZELTİLDİ 2026-07-17: JENERİK kod → bizde alan başına AYRI kural var.
    # 250-feed kanıtı: mdb-3230 direction_id=255 → TRP_005=26 ↔ MD 26 BİREBİR;
    # mdb-2086 transfers=5 → FAR_005=6 ↔ MD 6 BİREBİR. Yalnız RTS_004'e eşlemek
    # bu ikisini "kaçırdık" gösteriyordu.
    "unexpected_enum_value":          ["RTS_004", "RTS_030", "TRP_005", "TRP_006", "TRP_007", "TRP_032", "FAR_005", "TRF_004", "RCT_003", "RTS_013", "RTS_018"],
    "stop_has_too_many_matches_for_shape": ["SHP_022"], # DISJOINT: SHP_022 yalnız shape_dist EKSİK trip'lerde (geometrik fallback); MD shape_dist VARKEN → mdb-8'de us=9300/MD=0 mis-comparison'dı, MD-parite bug DEĞİL. (SHP_022 ORTA×9300 kendi başına severity sorusu.)
    "unused_station":                 ["STP_030"],     # station without child ≈ unused_station (~19)
    "trip_distance_exceeds_shape_distance": ["SHP_025"],  # VAR ama by-design: SHP_025 eşiği >%0.1 (rounding tolere, SHP_029 felsefesi). mdb-1909: 170 aşımın 165'i <%0.1 rounding → biz 5 doğru, MD 170 hepsini basar.
    "trip_distance_exceeds_shape_distance_below_threshold": ["SHP_025"],
    # ── 2026-07-05 DELFI (mdb-3215, Almanya ulusal) taramasından: MAP eksikleri (kuralımız VAR) ──
    "number_out_of_range":            ["PTH_007", "STP_003", "STP_005", "SHP_002", "SHP_003", "RCT_004", "FRQ_008"],  # Generic kod; alan bağlamı olmadan yalnız aday kümesi.

    "inconsistent_route_type_for_block_id": ["TRP_024"],  # "Block içinde tutarsız rota tipi"
    "transfer_distance_above_2_km":   ["TRF_011"],   # "aktarma tanımlandı ama mesafe uzak" (MD INFO variant)
    "transfer_distance_too_large":    ["TRF_011"],   # aynı kural (MD WARNING variant)
    "pathway_loop":                   ["PTH_011"],   # from_stop_id==to_stop_id (k2/pathways.rs:91)
    "location_with_unexpected_stop_time": ["STP_012"],  # stop_times durağı location_type≠0 (k4_cross_ref.rs:536)
    "pathway_unreachable_location":   ["PTH_012"],
    "stop_without_zone_id":            ["STP_033"],
    # point_near_origin bağlama taşındı (stops → GEO_016, shapes → GEO_017) — md_parity_mapping.CONTEXT_MAPPINGS
    "point_near_pole":                  ["GEO_022"],   # 2/2 — |lat|>89; GEO_022 aynı iki feed'de ateşliyor
    "pathway_dangling_generic_node":    ["PTH_019"],   # kural VARDI, eşleme yoktu (#158)
    "transfer_with_invalid_trip_and_stop":  ["XFL_021"],  # kural VARDI (XFL grubunda), defter yanlış "genuine-gap" demişti
    "invalid_character":                ["ARC_021"],
    "missing_feed_info_date":           ["FIN_014"],
    "more_than_one_entity":             ["FIN_015"],
    "overlapping_frequency":            ["FRQ_011"],
    # MD tek kodla iki olguyu birden sayar. Gece yarısını aşan seferde saat `24:xx`
    # olarak devam etmelidir; feed `00:xx` yazarsa MD bunu "varış öncekinden erken"
    # görür. Bizde o HAM YAZIM ihlali `STM_048` (K2, Spec), `STM_008` ise yalnız
    # normalizasyonla açıklanamayan GERÇEK geri gidişleri ölçer — ayrım STM_008
    # kartında yazılı. Yalnız STM_008'e bağlamak 6 feed'i sahte MISS yapıyordu;
    # koşumda 6'nın 5'i STM_048 alıyor (#155).
    "stop_time_with_arrival_before_previous_departure_time": ["STM_008", "STM_048"],
    "stop_time_timepoint_without_times": ["STM_047"],
    "missing_pickup_drop_off_booking_rule_id": ["STM_059"],
    "invalid_date":                     ["FIN_005", "FIN_006", "CAL_003", "CAL_004", "CLD_002"],
    "invalid_integer":                  ["TRP_005"],
    "invalid_url":                      ["AGN_003", "RTS_005", "FIN_002"],
    "missing_required_field":           ["AGN_003", "FIN_002"],
    "start_and_end_range_out_of_order": ["CAL_005", "FIN_012", "STM_007"],
    "unknown_file":                   ["ARC_007"],   # 2026-07-05 FIX: ARC_007 artık .txt-olmayan kök dosyaları da işaretler (.pdf/.out)
    # ── KAPSAM BOŞLUĞU: bilerek MAP'TE DEĞİL → MD-ONLY'de görünsün ──
    # `fast_travel_between_far_stops` (250-feed: 32 feed) KALDIRILDI 2026-07-17.
    # Eskiden STM_012'ye eşliydi, YANLIŞTI: STM_012 ">700 km/h" mutlak kontrolü ve
    # ARDIŞIK çiftlere bakar. MD'ninki ise coğrafi olarak >10 km ayrı, ARDIŞIK
    # OLMAYAN durak çiftlerini karşılaştırır — kanıt: mdb-1229 örneği
    # stopSequence 1 → 19 (18 durak atlıyor), 10 km / 600 km/h.
    # Bizde ardışık-olmayan aralık kontrolü YOK → GERÇEK kapsam boşluğu.
    # MD eşiği route_type başına consecutive ile AYNI (otobüs 150); ayıran tek şey
    # 10 km mesafe. Kural eklenirse: yeni STM_* (ardışık-olmayan aralık) düşün.
    #
    # ── GERÇEKTEN kalan minik (düşük değer, ertelenebilir) ──
    # decreasing_or_equal_stop_time_distance (2): 1-2 vaka, düşük öncelik
}

# ── AGREGE kurallar: biz per-entity/feed-özeti raporlarız, MD per-occurrence.
# Bunlarda our_count << md_count NORMALDİR → UNDER sanma, "AGG" etiketle.
AGG_RULES = {
    "STM_050", "STM_017", "TRN_007", "STP_022", "RTS_017",  # feed-özeti (changelog 0.1.3)
    "TRP_020",                                              # per-trip
    "SHP_023", "SHP_028", "SHP_029",                        # per-shape (tür başına)
    "CAL_007", "CAL_012", "CAL_013",                        # per-service, imza-agregasyonu (#30)
    "STM_014",  # 2026-07-16: (hat, yön, segment) toplulaması (53ef3fb). MD per-occurrence
                # sayar → our_count << md_count ARTIK NORMAL. Eklenmezse her koşumda
                # sahte UNDER üretir (250-feed koşumunda 20 feed'de böyle çıktı).
    "RTS_019",  # (kısa,uzun,tür,ajans) GRUBU başına tek notice, çakışan hatlar listelenir;
                # MD ÇİFT-ÇİFT sayar (routeId1/routeId2) → 3 hatlık grup = MD C(3,2)=3.
                # mdb-867: biz 1 ↔ MD 3. Granülerlik, uyuşmazlık değil.
    "DQ_016",   # 2026-07-17: DOSYA başına tek özet (c95189e). MD alan-değeri başına sayar
                # → our_count << md_count ARTIK NORMAL. (Kendi dersimizin tekrarı: kuralı
                # agregat yapınca AGG_RULES'a da ekle — STM_014'te de unutulmuştu.)
    "STM_018", "STM_019", "STM_032", "STM_007", "STM_056",  # 2026-08-18: >50 stop_times satırında tek feed özeti (#151);
                            # MD satır-başına sayar → our_count << md_count NORMAL.
    "SHP_012",  # per-SHAPE özeti: shape başına TEK notice, observed="N durak >100m".
                # MD (sefer,durak) başına sayar → aynı shape N seferde kullanılırsa MD N katı.
                # mdb-2366: bizim 171 notice içinde 3.338 uzak durak, MD 1.794 occurrence
                # → granülerlik farklı, sayı kıyası anlamsız.
    "FRL_001", "FRL_003", "FRL_004", "FRL_005",
                # 2026-07-17: AYRIK id başına tek notice (7727456), satır sayısı mesajda.
                # MD fare_rules SATIRI başına foreign_key_violation sayar → mdb-2837'de
                # 1.128.840 satır / 486 ayrık fare_id ⇒ biz 486, MD 1.128.840. AGG, UNDER değil.
                # (STM_002 emsali: aynı FK vakasını zaten ayrık stop_id ile özetliyor.)
    "TRP_021",  # feed-özeti: k2/trips.rs bikes_allowed-boş seferleri SAYAR, tek notice +
                # ilk 5 örnek trip_id emit eder (kart: "feed genelinde tek özet").
                # MD per-trip sayar (mdb-2933: 118) → bizim 1'imiz UNDER değil AGG.
}


# ── JENERİK MD KODU TUZAĞI (kalıcı ders) ────────────────────────────────────
# MD'nin JENERİK kodları (unexpected_enum_value, number_out_of_range,
# missing_recommended_field) bizde ALAN BAŞINA ayrı kurala karşılık gelir → tek
# kuralımıza 1-1 eşlemek sahte MISS üretir. BY_DESIGN kod-bazlıdır, alan-bazlı
# ayrım YAPAMAZ; bu yüzden önce MAP'i tamamla (tüm alan kuralları), SONRA kalan
# farkı gerekçelendir.
# unexpected_enum_value bu yolla çözüldü (2026-07-17): MAP 4 kurala genişletildi
# → direction_id/transfers BİREBİR eşleşti; kalan route_type farkı BY_DESIGN'a indi.
# Karar vermeden ÖNCE örnekteki `fieldName` dağılımına bak — sayı tek başına aldatır.

OVER_RATIO = 3.0
UNDER_RATIO = 0.34


def load_json(path):
    try:
        with open(path, encoding="utf-8") as f:
            return json.load(f)
    except Exception:
        return None


def classify(d):
    """İçeriğe göre: 'md' | 'golden' | None."""
    if not isinstance(d, dict):
        return None
    if "validation" in d and isinstance(d["validation"], dict):
        return "golden"
    notices = d.get("notices")
    if isinstance(notices, list) and notices and isinstance(notices[0], dict) and "code" in notices[0]:
        return "md"
    # boş-notice MD raporu da olabilir
    if "notices" in d and "summary" in d:
        return "md"
    return None


def find_pair(folder):
    md = golden = None
    for p in glob.glob(os.path.join(folder, "*.json")):
        d = load_json(p)
        kind = classify(d)
        if kind == "md" and md is None:
            md = (p, d)
        elif kind == "golden" and golden is None:
            golden = (p, d)
    return md, golden


def samples_str(notice, n=3):
    ss = notice.get("sampleNotices", [])[:n]
    return " | ".join(json.dumps(s, ensure_ascii=False) for s in ss)


def main():
    if len(sys.argv) < 2:
        print("kullanım: python3 md_parity_audit.py <BASE_DIR>")
        sys.exit(1)
    base = sys.argv[1]
    folders = [f for f in sorted(glob.glob(os.path.join(base, "*"))) if os.path.isdir(f)]

    mapped_ours = set(x for lst in MAP.values() for x in lst)
    rows = []              # B: (feed, kod) satırları
    md_only_rows = []      # C1
    agg_md = defaultdict(int)   # D agregat
    agg_our = defaultdict(int)
    agg_feeds = defaultdict(int)
    agg_diverge = defaultdict(int)
    feed_meta = []

    for folder in folders:
        feed = os.path.basename(folder)
        md, golden = find_pair(folder)
        if md is None or golden is None:
            feed_meta.append((feed, "PARSE/DOSYA HATASI",
                              "MD var" if md else "MD YOK",
                              "golden var" if golden else "golden YOK"))
            continue
        _, mdd = md
        _, gd = golden
        val = gd.get("validation", {})
        rc = {r: v.get("count", 0) for r, v in val.get("rule_counts", {}).items()}
        scores = val.get("scores", {})
        feed_meta.append((feed, "OK",
                          f"overall={scores.get('overall')}",
                          f"publish={scores.get('publish', scores.get('pub_score'))}"))

        md_counts = {n["code"]: n for n in mdd.get("notices", [])}
        for code, notice in sorted(md_counts.items(), key=lambda x: -x[1].get("totalNotices", 0)):
            mc = notice.get("totalNotices", 0)
            ours = MAP.get(code)
            agg_md[code] += mc
            agg_feeds[code] += 1
            resolution = resolve_mapping(code, notice, ours or ())
            ours = list(resolution.analyzer_rules)
            if not ours:
                classification, rationale = classify_unmapped(code)
                md_only_rows.append([feed, code, mc, notice.get("severity", ""),
                                     classification, rationale, samples_str(notice)])
                agg_diverge[code] += 1
                continue
            oc = sum(rc.get(r, 0) for r in ours)
            agg_our[code] += oc
            our_rule = "|".join(ours)
            is_agg = any(r in AGG_RULES for r in ours)
            ratio = (oc / mc) if mc else 0
            # A single MD code can legitimately contain samples from multiple
            # fields.  The golden snapshot has no per-field count, so do not
            # turn that aggregate into a false OVER/UNDER claim.
            if (resolution.kind == "context-mixed"
                    or resolution.unresolved_samples
                    or not resolution.context_complete):
                status = "CONTEXT"
            elif oc == 0 and mc > 0:
                status = "AGG" if is_agg else "MISS"
            elif ratio >= OVER_RATIO:
                status = "OVER"
            elif ratio <= UNDER_RATIO:
                status = "AGG" if is_agg else "UNDER"
            else:
                status = "MATCH"
            # Adjudicate edilmiş sapmayı, sapma olarak SAYMA. MATCH'i ezmiyoruz:
            # zaten uyuşuyorsa "explained" demek bilgi kaybı olurdu.
            if code in BY_DESIGN and status not in ("MATCH", "CONTEXT"):
                status = "EXPLAINED"
            if status in ("OVER", "UNDER", "MISS"):
                agg_diverge[code] += 1
            gv = gd.get("validation", {}).get("rule_counts", {})
            our_sev = "|".join(gv.get(r, {}).get("severity", "") for r in ours if r in gv)
            our_cls = "|".join(sorted({gv.get(r, {}).get("rule_class", "") for r in ours if r in gv}))
            rows.append([feed, code, mc, notice.get("severity", ""), our_rule, oc,
                         our_sev, our_cls, "AGG" if is_agg else "", f"{ratio:.2f}",
                         status, samples_str(notice), resolution.kind,
                         "|".join(resolution.contexts),
                         "complete" if resolution.context_complete else "partial"])

        # US-ONLY: bizde ≥1, MAP'te MD karşılığı yok, sayı büyük
        for r, c in rc.items():
            if r not in mapped_ours and c > 0:
                pass  # US-ONLY ayrı raporlanabilir; şimdilik parity_all dışı

    # ── yaz ──
    with open(os.path.join(base, "parity_all.csv"), "w", newline="", encoding="utf-8") as f:
        w = csv.writer(f)
        w.writerow(["feed", "md_code", "md_count", "md_severity", "our_rule", "our_count",
                    "our_severity", "our_class", "agg", "ratio", "status", "md_samples",
                    "mapping_kind", "mapping_context", "sample_context"])
        w.writerows(rows)
    with open(os.path.join(base, "parity_md_only.csv"), "w", newline="", encoding="utf-8") as f:
        w = csv.writer(f)
        w.writerow(["feed", "md_code", "md_count", "md_severity", "classification",
                    "rationale", "md_samples"])
        w.writerows(md_only_rows)

    # ── ASIL ÇIKTI: açıklanamayan delta ──
    # parity_all her şeyi tutar (denetlenebilirlik), ama insan bunu okur:
    # MATCH gürültüsü ve adjudicate edilmiş sapmalar düşülmüş hâli.
    unexplained = [r for r in rows if r[10] in ("MISS", "OVER", "UNDER")]
    with open(os.path.join(base, "parity_unexplained.csv"), "w", newline="", encoding="utf-8") as f:
        w = csv.writer(f)
        w.writerow(["feed", "md_code", "md_count", "md_severity", "our_rule", "our_count",
                    "our_severity", "our_class", "agg", "ratio", "status", "md_samples",
                    "mapping_kind", "mapping_context", "sample_context"])
        w.writerows(unexplained)

    explained = sum(1 for r in rows if r[10] == "EXPLAINED")
    by_code = Counter((r[10], r[1]) for r in unexplained)

    with open(os.path.join(base, "parity_summary.md"), "w", encoding="utf-8") as f:
        f.write("# MD-parite agregat\n\n")
        f.write("## 🎯 Açıklanamayan delta (inceleme kuyruğu)\n\n")
        f.write(f"- Toplam satır: **{len(rows)}** · MATCH: **{sum(1 for r in rows if r[10]=='MATCH')}** "
                f"· AGG: **{sum(1 for r in rows if r[10]=='AGG')}** "
                f"· CONTEXT: **{sum(1 for r in rows if r[10]=='CONTEXT')}** "
                f"· adjudicate edilmiş (EXPLAINED): **{explained}**\n")
        f.write(f"- **AÇIKLANAMAYAN: {len(unexplained)}** → `parity_unexplained.csv`\n\n")
        f.write("EXPLAINED = `BY_DESIGN` tablosunda gerekçesi yazılı, daha önce yargılanmış sapma.\n"
                "Aşağıdaki liste kanıt DEĞİL, inceleme kuyruğudur.\n\n")
        f.write("| durum | md_code | satır |\n|---|---|--:|\n")
        for (st, code), n in by_code.most_common(25):
            f.write(f"| {st} | {code} | {n} |\n")
        f.write("\n## Feed durumu\n\n")
        f.write("| feed | durum | skor | | \n|---|---|---|---|\n")
        for m in feed_meta:
            f.write(f"| {m[0]} | {m[1]} | {m[2]} | {m[3]} |\n")
        f.write("\n## Kod agregat (sapma büyüklüğüne göre)\n\n")
        f.write("| md_code | Σmd | Σour | feed# | sapan# | eşleme |\n|---|--:|--:|--:|--:|---|\n")
        allcodes = set(agg_md) | set(agg_our)
        for code in sorted(allcodes, key=lambda c: -(agg_md[c])):
            ours = MAP.get(code)
            tag = "|".join(ours) if ours else "**MD-ONLY**"
            f.write(f"| {code} | {agg_md[code]} | {agg_our.get(code,'—')} | "
                    f"{agg_feeds[code]} | {agg_diverge[code]} | {tag} |\n")
    print(f"yazıldı: parity_all.csv ({len(rows)}) · parity_unexplained.csv ({len(unexplained)}) · "
          f"parity_md_only.csv ({len(md_only_rows)}) · parity_summary.md")
    print(f"  MATCH={sum(1 for r in rows if r[10]=='MATCH')} AGG={sum(1 for r in rows if r[10]=='AGG')} "
          f"EXPLAINED={explained} → AÇIKLANAMAYAN={len(unexplained)}")


if __name__ == "__main__":
    main()
