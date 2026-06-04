# Sapma Defteri (Divergence Ledger)

> **Amaç:** Bir feed'i MobilityData/GTFS Guru ile karşılaştırınca çıkan farkın **bizde bilinçli mi, bug mı** olduğunu kesin cevaplamak. `docs/RULE_TRIAGE.md` Adım 3'ün referansıdır.
>
> **Katman A (otomatik):** `MD karşılığı` + `Tür` sütunları kartların "Dış araç eşleşmesi" bölümünden üretilir — `cargo run -p gtfs-rules --example gen_divergence_ledger` ile yenilenir.
>
> **Katman B (elle):** `Bilinçli sapma?` + `Gerekçe / parite` sütunları boş doğar; bir fark araştırıldıkça doldurulur. Bir kez doldurulan satır, aynı fark tekrar çıktığında anında karar verir.
>
> **Tür değerleri:** `MD-eşleşme` (MD'de birebir/yakın karşılık var) · `proje-özel` (MD/Guru'da karşılık yok) · `İNCELE` (otomatik sınıflanamadı).


## ARC

| ID | Önem·Sınıf | MD karşılığı | Tür | Bilinçli sapma? | Gerekçe / parite |
|---|---|---|---|---|---|
| ARC_001 | Kritik·Spec | `csv_parsing_failed` | MD-eşleşme |  |  |
| ARC_002 | Kritik·Spec | `csv_parsing_failed` | MD-eşleşme |  |  |
| ARC_003 | Orta·Quality | `csv_parsing_failed` | MD-eşleşme |  |  |
| ARC_004 | Kritik·Spec | `csv_parsing_failed` | MD-eşleşme |  |  |
| ARC_006 | Bilgi·Quality | `csv_parsing_failed` | MD-eşleşme |  |  |
| ARC_007 | Bilgi·Quality | `csv_parsing_failed` | MD-eşleşme |  |  |
| ARC_008 | Kritik·Spec | `csv_parsing_failed` | MD-eşleşme |  |  |
| ARC_009 | Kritik·Spec | `csv_parsing_failed` | MD-eşleşme |  |  |
| ARC_010 | Orta·Interop | `csv_parsing_failed` | MD-eşleşme |  |  |
| ARC_011 | Bilgi·Analytics | `csv_parsing_failed` | MD-eşleşme |  |  |
| ARC_012 | Kritik·Spec | `csv_parsing_failed` | MD-eşleşme |  |  |
| ARC_013 | Kritik·Spec | `csv_parsing_failed` | MD-eşleşme |  |  |
| ARC_014 | Orta·Quality | `csv_parsing_failed` | MD-eşleşme |  |  |
| ARC_015 | Kritik·Spec | `csv_parsing_failed` | MD-eşleşme |  |  |
| ARC_016 | Yüksek·Spec | `csv_parsing_failed` | MD-eşleşme |  |  |
| ARC_017 | Bilgi·Quality | `jp_` | MD-eşleşme |  |  |
| ARC_018 | Orta·Quality | — | İNCELE |  |  |
| ARC_019 | Yüksek·Spec | `empty_column_name` | MD-eşleşme |  |  |
| ARC_020 | Düşük·Quality | `feed_info.txt` | MD-eşleşme |  |  |
| ARC_021 | Düşük·Quality | `invalid_character` | MD-eşleşme |  |  |
| ARC_022 | Düşük·Quality | — | proje-özel |  |  |
| ARC_023 | Orta·Spec | — | İNCELE |  |  |
| ARC_024 | Orta·Spec | `invalid_input_files_in_subfolder` | MD-eşleşme |  |  |

## BKR

| ID | Önem·Sınıf | MD karşılığı | Tür | Bilinçli sapma? | Gerekçe / parite |
|---|---|---|---|---|---|
| BKR_001 | Yüksek·Spec | `forbidden_real_time_booking_field_value` | MD-eşleşme |  |  |
| BKR_002 | Yüksek·Spec | `booking_rules.txt` | MD-eşleşme |  |  |
| BKR_003 | Yüksek·Spec | `forbidden_prior_notice_start_time` | MD-eşleşme |  |  |
| BKR_004 | Yüksek·Spec | `forbidden_real_time_booking_field_value` | MD-eşleşme |  |  |
| BKR_005 | Orta·Spec | `forbidden_prior_day_booking_field_value` | MD-eşleşme |  |  |
| BKR_006 | Yüksek·Spec | `invalid_prior_notice_duration_min` | MD-eşleşme |  |  |
| BKR_007 | Kritik·Spec | `missing_prior_notice_duration_min` | MD-eşleşme |  |  |
| BKR_008 | Kritik·Spec | `missing_prior_notice_last_day` | MD-eşleşme |  |  |
| BKR_009 | Kritik·Spec | `missing_prior_notice_last_time` | MD-eşleşme |  |  |
| BKR_010 | Yüksek·Spec | `missing_prior_notice_start_time` | MD-eşleşme |  |  |
| BKR_011 | Yüksek·Spec | `booking_rules.txt` | MD-eşleşme |  |  |

## AGN

| ID | Önem·Sınıf | MD karşılığı | Tür | Bilinçli sapma? | Gerekçe / parite |
|---|---|---|---|---|---|
| AGN_001 | Kritik·Spec | `missing_required_file` | MD-parite | evet | agency.txt zorunlu dosya; MD `missing_required_file` ile birebir (aşağıdaki grup-notice listesi diğer AGN alan kuralları içindir, AGN_001 dosya-eksik kuralıdır) |
| AGN_002 | Kritik·Spec | `missing_required_agency_id` | MD-eşleşme |  |  |
| AGN_003 | Yüksek·Spec | `missing_required_agency_id` | MD-eşleşme |  |  |
| AGN_004 | Kritik·Spec | `missing_required_agency_id` | MD-eşleşme |  |  |
| AGN_005 | Orta·Quality | `missing_required_agency_id` | MD-eşleşme |  |  |
| AGN_006 | Düşük·Spec | `missing_required_agency_id` | MD-eşleşme |  |  |
| AGN_007 | Düşük·Quality | `missing_required_agency_id` | MD-eşleşme |  |  |
| AGN_008 | Düşük·Spec | `missing_required_agency_id` | MD-eşleşme |  |  |
| AGN_009 | Düşük·Spec | `missing_required_agency_id` | MD-eşleşme |  |  |
| AGN_010 | Kritik·Spec | `missing_required_agency_id` | MD-eşleşme |  |  |
| AGN_011 | Kritik·Spec | `missing_required_agency_id` | MD-eşleşme |  |  |
| AGN_012 | Düşük·Quality | `missing_required_agency_id` | MD-eşleşme |  |  |
| AGN_013 | Düşük·Interop | `missing_required_agency_id` | MD-eşleşme |  |  |

## STP

| ID | Önem·Sınıf | MD karşılığı | Tür | Bilinçli sapma? | Gerekçe / parite |
|---|---|---|---|---|---|
| STP_001 | Kritik·Spec | `duplicate_key` | proje-özel (yakın MD var) |  |  |
| STP_002 | Yüksek·Spec | `duplicate_key` | proje-özel (yakın MD var) |  |  |
| STP_003 | Kritik·Spec | `duplicate_key` | proje-özel (yakın MD var) |  |  |
| STP_004 | Kritik·Spec | `duplicate_key` | proje-özel (yakın MD var) |  |  |
| STP_005 | Kritik·Quality | `duplicate_key` | proje-özel (yakın MD var) |  |  |
| STP_006 | Kritik·Spec | `duplicate_key` | proje-özel (yakın MD var) |  |  |
| STP_007 | Kritik·Spec | `duplicate_key` | proje-özel (yakın MD var) |  |  |
| STP_008 | Yüksek·Spec | `duplicate_key` | proje-özel (yakın MD var) |  |  |
| STP_009 | Kritik·Spec | `duplicate_key` | proje-özel (yakın MD var) |  |  |
| STP_010 | Yüksek·Spec | `duplicate_key` | proje-özel (yakın MD var) |  |  |
| STP_011 | Yüksek·Spec | `duplicate_key` | proje-özel (yakın MD var) |  |  |
| STP_012 | Kritik·Spec | `duplicate_key` | proje-özel (yakın MD var) |  |  |
| STP_013 | Düşük·Spec | `duplicate_key` | proje-özel (yakın MD var) |  |  |
| STP_014 | Orta·Spec | `duplicate_key` | proje-özel (yakın MD var) |  |  |
| STP_015 | Orta·Spec | `duplicate_key` | proje-özel (yakın MD var) |  |  |
| STP_016 | Orta·Quality | `duplicate_key` | proje-özel (yakın MD var) |  |  |
| STP_017 | Düşük·Quality | `duplicate_key` | proje-özel (yakın MD var) |  |  |
| STP_018 | Kritik·Spec | `duplicate_key` | proje-özel (yakın MD var) |  |  |
| STP_019 | Düşük·Quality | `duplicate_key` | proje-özel (yakın MD var) |  |  |
| STP_020 | Orta·Analytics | `duplicate_key` | proje-özel (yakın MD var) |  |  |
| STP_021 | Yüksek·Quality | `duplicate_key` | proje-özel (yakın MD var) |  |  |
| STP_022 | Orta·Quality | `duplicate_key` | proje-özel (yakın MD var) |  |  |
| STP_023 | Düşük·Quality | `duplicate_key` | proje-özel (yakın MD var) |  |  |
| STP_024 | Bilgi·Quality | `duplicate_key` | proje-özel (yakın MD var) |  |  |
| STP_025 | Orta·Quality | `duplicate_key` | proje-özel (yakın MD var) |  |  |
| STP_026 | Düşük·Spec | `duplicate_key` | proje-özel (yakın MD var) |  |  |
| STP_027 | Orta·Spec | `duplicate_key` | proje-özel (yakın MD var) |  |  |
| STP_028 | Bilgi·Quality | `duplicate_key` | proje-özel (yakın MD var) |  |  |
| STP_029 | Yüksek·Quality | `duplicate_key` | proje-özel (yakın MD var) |  |  |
| STP_030 | Orta·Quality | `duplicate_key` | proje-özel (yakın MD var) |  |  |
| STP_031 | Bilgi·Quality | `duplicate_key` | proje-özel (yakın MD var) |  |  |
| STP_032 | Orta·Quality | `duplicate_key` | proje-özel (yakın MD var) |  |  |
| STP_033 | Bilgi·Quality | `duplicate_key` | proje-özel (yakın MD var) |  |  |
| STP_034 | Bilgi·Quality | `duplicate_key` | proje-özel (yakın MD var) |  |  |
| STP_035 | Bilgi·Quality | `duplicate_key` | proje-özel (yakın MD var) |  |  |
| STP_036 | Düşük·Spec | `duplicate_key` | proje-özel (yakın MD var) |  |  |

## RTS

| ID | Önem·Sınıf | MD karşılığı | Tür | Bilinçli sapma? | Gerekçe / parite |
|---|---|---|---|---|---|
| RTS_001 | Kritik·Spec | `duplicate_key` | MD-eşleşme |  |  |
| RTS_002 | Kritik·Spec | `foreign_key_violation` | MD-eşleşme |  |  |
| RTS_003 | Kritik·Spec | `route_both_short_and_long_name_missing` | MD-eşleşme |  |  |
| RTS_004 | Kritik·Spec | `missing_required_field` | MD-eşleşme |  |  |
| RTS_005 | Orta·Spec | `invalid_url` | MD-eşleşme |  |  |
| RTS_006 | Orta·Spec | `invalid_color` | MD-eşleşme |  |  |
| RTS_007 | Düşük·Quality | `invalid_color` | MD-eşleşme |  |  |
| RTS_008 | Orta·Quality | `route_color_contrast` | MD-eşleşme |  |  |
| RTS_009 | Düşük·Spec | `route_long_name_contains_short_name` | proje-özel (yakın MD var) |  |  |
| RTS_010 | Düşük·Quality | `route_short_name_too_long` | MD-eşleşme |  |  |
| RTS_011 | Düşük·Quality | — | proje-özel |  |  |
| RTS_012 | Orta·Quality | — | proje-özel |  |  |
| RTS_013 | Düşük·Spec | `unexpected_enum_value` | MD-eşleşme |  |  |
| RTS_016 | Düşük·Quality | — | proje-özel |  |  |
| RTS_017 | Bilgi·Quality | — | proje-özel |  |  |
| RTS_018 | Düşük·Spec | `unexpected_enum_value` | MD-eşleşme |  |  |
| RTS_019 | Orta·Quality | `duplicate_route_name` | MD-eşleşme |  |  |
| RTS_020 | Düşük·Quality | — | proje-özel |  |  |
| RTS_021 | Düşük·Interop | — | proje-özel |  |  |
| RTS_022 | Düşük·Quality | `route_long_name_contains_short_name` | MD-eşleşme |  |  |
| RTS_023 | Bilgi·Quality | `same_name_and_description_for_route` | MD-eşleşme |  |  |

## TRP

| ID | Önem·Sınıf | MD karşılığı | Tür | Bilinçli sapma? | Gerekçe / parite |
|---|---|---|---|---|---|
| TRP_001 | Kritik·Spec | `missing_required_field` | MD-eşleşme |  |  |
| TRP_002 | Kritik·Spec | `foreign_key_violation` | MD-eşleşme |  |  |
| TRP_003 | Kritik·Spec | `foreign_key_violation` | MD-eşleşme |  |  |
| TRP_004 | Yüksek·Spec | `foreign_key_violation` | MD-eşleşme |  |  |
| TRP_005 | Orta·Spec | `unexpected_enum_value` | MD-eşleşme |  |  |
| TRP_006 | Düşük·Spec | `unexpected_enum_value` | MD-eşleşme |  |  |
| TRP_007 | Düşük·Spec | `unexpected_enum_value` | MD-eşleşme |  |  |
| TRP_009 | Yüksek·Quality | `unusable_trip` | proje-özel (yakın MD var) |  |  |
| TRP_011 | Yüksek·Quality | — | proje-özel |  |  |
| TRP_012 | Düşük·Quality | — | proje-özel |  |  |
| TRP_013 | Düşük·Quality | — | proje-özel |  |  |
| TRP_014 | Bilgi·Quality | `trip_short_name` | proje-özel (yakın MD var) |  |  |
| TRP_015 | Düşük·Quality | — | proje-özel |  |  |
| TRP_017 | Orta·Spec | `unusable_trip` | proje-özel (yakın MD var) |  |  |
| TRP_019 | Yüksek·Spec | — | proje-özel |  |  |
| TRP_020 | Düşük·Quality | — | proje-özel |  |  |
| TRP_021 | Bilgi·Quality | — | proje-özel |  |  |
| TRP_022 | Yüksek·Spec | `block_trips_with_overlapping_stop_times` | MD-eşleşme |  |  |
| TRP_023 | Düşük·Quality | `trip_coverage_not_active_for_next7_days` | MD-eşleşme |  |  |
| TRP_024 | Düşük·Interop | — | proje-özel |  |  |
| TRP_025 | Bilgi·Quality | — | proje-özel |  |  |
| TRP_026 | Orta·Analytics | — | proje-özel |  |  |
| TRP_028 | Orta·Quality | — | proje-özel |  |  |
| TRP_029 | Bilgi·Quality | — | proje-özel |  |  |
| TRP_030 | Düşük·Quality | `trip_coverage_not_active_for_next7_days` | MD-eşleşme |  |  |

## STM

| ID | Önem·Sınıf | MD karşılığı | Tür | Bilinçli sapma? | Gerekçe / parite |
|---|---|---|---|---|---|
| STM_001 | Kritik·Spec | `foreign_key_violation` | proje-özel (yakın MD var) |  |  |
| STM_002 | Kritik·Spec | `foreign_key_violation` | proje-özel (yakın MD var) |  |  |
| STM_003 | Kritik·Spec | `foreign_key_violation` | proje-özel (yakın MD var) |  |  |
| STM_004 | Kritik·Spec | `foreign_key_violation` | proje-özel (yakın MD var) |  |  |
| STM_005 | Kritik·Spec | `foreign_key_violation` | proje-özel (yakın MD var) |  |  |
| STM_006 | Kritik·Spec | `foreign_key_violation` | proje-özel (yakın MD var) |  |  |
| STM_007 | Yüksek·Spec | `foreign_key_violation` | proje-özel (yakın MD var) |  |  |
| STM_008 | Kritik·Spec | `foreign_key_violation` | proje-özel (yakın MD var) |  |  |
| STM_009 | Yüksek·Spec | `foreign_key_violation` | proje-özel (yakın MD var) |  |  |
| STM_010 | Yüksek·Spec | `foreign_key_violation` | proje-özel (yakın MD var) |  |  |
| STM_012 | Yüksek·Interop | `foreign_key_violation` | proje-özel (yakın MD var) |  |  |
| STM_013 | Yüksek·Quality | `foreign_key_violation` | proje-özel (yakın MD var) |  |  |
| STM_014 | Yüksek·Analytics | `foreign_key_violation` | proje-özel (yakın MD var) |  |  |
| STM_015 | Kritik·Spec | `foreign_key_violation` | proje-özel (yakın MD var) |  |  |
| STM_016 | Kritik·Spec | `foreign_key_violation` | proje-özel (yakın MD var) |  |  |
| STM_017 | Orta·Interop | `foreign_key_violation` | proje-özel (yakın MD var) |  |  |
| STM_018 | Orta·Spec | `foreign_key_violation` | proje-özel (yakın MD var) |  |  |
| STM_019 | Orta·Spec | `foreign_key_violation` | proje-özel (yakın MD var) |  |  |
| STM_020 | Yüksek·Quality | `foreign_key_violation` | proje-özel (yakın MD var) |  |  |
| STM_021 | Yüksek·Quality | `foreign_key_violation` | proje-özel (yakın MD var) |  |  |
| STM_022 | Orta·Spec | `foreign_key_violation` | proje-özel (yakın MD var) |  |  |
| STM_023 | Kritik·Spec | `foreign_key_violation` | proje-özel (yakın MD var) |  |  |
| STM_024 | Bilgi·Quality | `foreign_key_violation` | proje-özel (yakın MD var) |  |  |
| STM_025 | Orta·Quality | `foreign_key_violation` | proje-özel (yakın MD var) |  |  |
| STM_026 | Yüksek·Quality | `foreign_key_violation` | proje-özel (yakın MD var) |  |  |
| STM_027 | Yüksek·Interop | `foreign_key_violation` | proje-özel (yakın MD var) |  |  |
| STM_028 | Yüksek·Analytics | `foreign_key_violation` | proje-özel (yakın MD var) |  |  |
| STM_029 | Orta·Analytics | `foreign_key_violation` | proje-özel (yakın MD var) |  |  |
| STM_030 | Düşük·Spec | `foreign_key_violation` | proje-özel (yakın MD var) |  |  |
| STM_032 | Düşük·Quality | `foreign_key_violation` | proje-özel (yakın MD var) |  |  |
| STM_033 | Yüksek·Spec | `foreign_key_violation` | proje-özel (yakın MD var) |  |  |
| STM_034 | Orta·Spec | `foreign_key_violation` | proje-özel (yakın MD var) |  |  |
| STM_035 | Bilgi·Analytics | `foreign_key_violation` | proje-özel (yakın MD var) |  |  |
| STM_036 | Yüksek·Quality | `foreign_key_violation` | proje-özel (yakın MD var) |  |  |
| STM_037 | Yüksek·Spec | `foreign_key_violation` | proje-özel (yakın MD var) |  |  |
| STM_038 | Yüksek·Spec | `foreign_key_violation` | proje-özel (yakın MD var) |  |  |
| STM_039 | Kritik·Spec | `foreign_key_violation` | proje-özel (yakın MD var) |  |  |
| STM_040 | Yüksek·Spec | `foreign_key_violation` | proje-özel (yakın MD var) |  |  |
| STM_041 | Yüksek·Spec | `foreign_key_violation` | proje-özel (yakın MD var) |  |  |
| STM_042 | Düşük·Interop | `foreign_key_violation` | proje-özel (yakın MD var) |  |  |
| STM_043 | Bilgi·Analytics | `foreign_key_violation` | proje-özel (yakın MD var) |  |  |
| STM_044 | Bilgi·Analytics | `foreign_key_violation` | proje-özel (yakın MD var) |  |  |
| STM_045 | Orta·Quality | `foreign_key_violation` | proje-özel (yakın MD var) |  |  |

## PDW

| ID | Önem·Sınıf | MD karşılığı | Tür | Bilinçli sapma? | Gerekçe / parite |
|---|---|---|---|---|---|
| PDW_006 | Orta·Spec | — | İNCELE |  |  |

## LOC

| ID | Önem·Sınıf | MD karşılığı | Tür | Bilinçli sapma? | Gerekçe / parite |
|---|---|---|---|---|---|
| LOC_001 | Yüksek·Spec | `invalid_geometry` | MD-eşleşme |  |  |
| LOC_002 | Yüksek·Spec | — | İNCELE |  |  |
| LOC_003 | Yüksek·Spec | `duplicate_geography_id` | MD-eşleşme |  |  |
| LOC_004 | Orta·Spec | — | İNCELE |  |  |
| LOC_005 | Düşük·Quality | — | İNCELE |  |  |
| LOC_006 | Orta·Quality | — | İNCELE |  |  |
| LOC_007 | Orta·Spec | `duplicate_geography_id` | MD-eşleşme |  |  |

## CAL

| ID | Önem·Sınıf | MD karşılığı | Tür | Bilinçli sapma? | Gerekçe / parite |
|---|---|---|---|---|---|
| CAL_001 | Kritik·Spec | — | İNCELE |  |  |
| CAL_002 | Kritik·Spec | — | İNCELE |  |  |
| CAL_003 | Kritik·Spec | — | İNCELE |  |  |
| CAL_004 | Kritik·Spec | — | İNCELE |  |  |
| CAL_005 | Kritik·Spec | — | İNCELE |  |  |
| CAL_006 | Yüksek·Quality | — | İNCELE |  |  |
| CAL_007 | Orta·Analytics | — | İNCELE |  |  |
| CAL_008 | Yüksek·Analytics | — | İNCELE |  |  |
| CAL_009 | Kritik·Interop | `expired_calendar` | MD-eşleşme |  |  |
| CAL_010 | Orta·Analytics | — | İNCELE |  |  |
| CAL_011 | Düşük·Quality | — | İNCELE |  |  |
| CAL_012 | Yüksek·Analytics | — | İNCELE |  |  |
| CAL_013 | Bilgi·Analytics | — | İNCELE |  |  |
| CAL_014 | Düşük·Quality | — | İNCELE |  |  |
| CAL_015 | Düşük·Quality | — | İNCELE |  |  |
| CAL_016 | Bilgi·Quality | — | İNCELE |  |  |
| CAL_017 | Düşük·Quality | — | İNCELE |  |  |
| CAL_018 | Düşük·Quality | — | İNCELE |  |  |
| CAL_019 | Düşük·Quality | `service_window_outside_feed_period` | MD-eşleşme |  |  |
| CAL_020 | Düşük·Quality | — | İNCELE |  |  |

## CLD

| ID | Önem·Sınıf | MD karşılığı | Tür | Bilinçli sapma? | Gerekçe / parite |
|---|---|---|---|---|---|
| CLD_001 | Kritik·Spec | — | İNCELE |  |  |
| CLD_002 | Kritik·Spec | — | İNCELE |  |  |
| CLD_003 | Kritik·Spec | — | İNCELE |  |  |
| CLD_004 | Yüksek·Interop | — | İNCELE |  |  |
| CLD_005 | Kritik·Spec | — | İNCELE |  |  |
| CLD_006 | Orta·Quality | — | İNCELE |  |  |
| CLD_007 | Bilgi·Analytics | — | İNCELE |  |  |

## SHP

| ID | Önem·Sınıf | MD karşılığı | Tür | Bilinçli sapma? | Gerekçe / parite |
|---|---|---|---|---|---|
| SHP_001 | Düşük·Quality | `missing_required_field` | MD-eşleşme |  |  |
| SHP_002 | Kritik·Spec | `missing_required_field` | MD-eşleşme |  |  |
| SHP_003 | Kritik·Spec | `missing_required_field` | MD-eşleşme |  |  |
| SHP_004 | Kritik·Spec | `missing_required_field` | MD-eşleşme |  |  |
| SHP_005 | Kritik·Spec | `decreasing_shape_distance` | MD-eşleşme |  |  |
| SHP_006 | Kritik·Spec | — | proje-özel |  |  |
| SHP_007 | Kritik·Spec | — | proje-özel |  |  |
| SHP_008 | Kritik·Spec | `duplicate_key` | MD-eşleşme |  |  |
| SHP_009 | Bilgi·Analytics | — | proje-özel |  |  |
| SHP_010 | Düşük·Quality | — | proje-özel |  |  |
| SHP_011 | Orta·Analytics | — | proje-özel |  |  |
| SHP_012 | Yüksek·Analytics | `stop_too_far_from_shape` | MD-eşleşme |  |  |
| SHP_014 | Yüksek·Quality | `stop_too_far_from_shape` | MD-eşleşme |  |  |
| SHP_015 | Orta·Quality | — | proje-özel |  |  |
| SHP_016 | Yüksek·Interop | `stops_match_shape_out_of_order` | proje-özel (yakın MD var) |  |  |
| SHP_017 | Yüksek·Quality | `stops_match_shape_out_of_order` | MD-eşleşme |  |  |
| SHP_018 | Düşük·Quality | — | proje-özel |  |  |
| SHP_019 | Orta·Quality | `unusable_trip` | proje-özel (yakın MD var) |  |  |
| SHP_020 | Bilgi·Analytics | — | proje-özel |  |  |
| SHP_021 | Düşük·Quality | `number_out_of_range` | proje-özel (yakın MD var) |  |  |
| SHP_022 | Orta·Interop | — | proje-özel |  |  |
| SHP_023 | Orta·Quality | `equal_shape_distance_*` | proje-özel (yakın MD var) |  |  |
| SHP_024 | Orta·Quality | `stop_too_far_from_shape_using_user_distance` | MD-eşleşme |  |  |
| SHP_025 | Orta·Quality | — | proje-özel |  |  |
| SHP_026 | Bilgi·Analytics | — | proje-özel |  |  |
| SHP_027 | Bilgi·Analytics | — | proje-özel |  |  |

## FRQ

| ID | Önem·Sınıf | MD karşılığı | Tür | Bilinçli sapma? | Gerekçe / parite |
|---|---|---|---|---|---|
| FRQ_001 | Kritik·Spec | — | İNCELE |  |  |
| FRQ_002 | Kritik·Spec | — | İNCELE |  |  |
| FRQ_003 | Kritik·Spec | — | İNCELE |  |  |
| FRQ_004 | Kritik·Spec | — | İNCELE |  |  |
| FRQ_005 | Kritik·Spec | — | İNCELE |  |  |
| FRQ_006 | Orta·Analytics | — | İNCELE |  |  |
| FRQ_007 | Orta·Spec | — | İNCELE |  |  |
| FRQ_008 | Kritik·Spec | — | İNCELE |  |  |
| FRQ_009 | Orta·Quality | — | İNCELE |  |  |
| FRQ_010 | Bilgi·Analytics | — | İNCELE |  |  |

## TRF

| ID | Önem·Sınıf | MD karşılığı | Tür | Bilinçli sapma? | Gerekçe / parite |
|---|---|---|---|---|---|
| TRF_001 | Kritik·Spec | `missing_required_field` | MD-eşleşme |  |  |
| TRF_002 | Kritik·Spec | `missing_required_field` | MD-eşleşme |  |  |
| TRF_003 | Kritik·Spec | `foreign_key_violation` | MD-eşleşme |  |  |
| TRF_004 | Yüksek·Spec | `unexpected_enum_value` | MD-eşleşme |  |  |
| TRF_005 | Yüksek·Spec | — | proje-özel |  |  |
| TRF_006 | Kritik·Spec | `foreign_key_violation` | MD-eşleşme |  |  |
| TRF_007 | Kritik·Spec | `foreign_key_violation` | MD-eşleşme |  |  |
| TRF_008 | Kritik·Spec | `foreign_key_violation` | MD-eşleşme |  |  |
| TRF_009 | Kritik·Spec | `foreign_key_violation` | MD-eşleşme |  |  |
| TRF_010 | Orta·Analytics | — | proje-özel |  |  |
| TRF_011 | Bilgi·Quality | — | proje-özel |  |  |
| TRF_012 | Orta·Quality | `duplicate_key` | MD-eşleşme |  |  |
| TRF_013 | Kritik·Spec | — | proje-özel |  |  |
| TRF_014 | Yüksek·Spec | — | proje-özel |  |  |
| TRF_015 | Yüksek·Spec | — | proje-özel |  |  |
| TRF_016 | Orta·Spec | `duplicate_key` | MD-eşleşme |  |  |
| TRF_017 | Yüksek·Spec | — | proje-özel |  |  |
| TRF_018 | Yüksek·Spec | — | proje-özel |  |  |
| TRF_019 | Orta·Spec | — | İNCELE |  |  |

## GGL

| ID | Önem·Sınıf | MD karşılığı | Tür | Bilinçli sapma? | Gerekçe / parite |
|---|---|---|---|---|---|
| GGL_001 | Düşük·Interop | `transfers.txt.transfer_type` | MD-eşleşme |  |  |
| GGL_002 | Düşük·Interop | `fare_products.txt` | MD-eşleşme |  |  |

## FAR

| ID | Önem·Sınıf | MD karşılığı | Tür | Bilinçli sapma? | Gerekçe / parite |
|---|---|---|---|---|---|
| FAR_001 | Kritik·Spec | — | İNCELE |  |  |
| FAR_002 | Yüksek·Spec | `fare_attributes.txt` | MD-eşleşme |  |  |
| FAR_003 | Kritik·Spec | `currency_type` | MD-eşleşme |  |  |
| FAR_004 | Kritik·Spec | `payment_method` | MD-eşleşme |  |  |
| FAR_005 | Kritik·Spec | — | İNCELE |  |  |
| FAR_006 | Orta·Spec | `transfer_duration` | MD-eşleşme |  |  |
| FAR_008 | Kritik·Spec | `agency_id` | MD-eşleşme |  |  |
| FAR_009 | Düşük·Quality | `fare_attributes.txt` | MD-eşleşme |  |  |
| FAR_010 | Orta·Quality | `fare_rules.txt` | MD-eşleşme |  |  |

## FRL

| ID | Önem·Sınıf | MD karşılığı | Tür | Bilinçli sapma? | Gerekçe / parite |
|---|---|---|---|---|---|
| FRL_001 | Kritik·Spec | `fare_rules.fare_id` | MD-eşleşme |  |  |
| FRL_002 | Kritik·Spec | `route_id` | MD-eşleşme |  |  |
| FRL_003 | Kritik·Spec | `origin_id` | MD-eşleşme |  |  |
| FRL_004 | Kritik·Spec | `destination_id` | MD-eşleşme |  |  |
| FRL_005 | Kritik·Spec | `contains_id` | MD-eşleşme |  |  |
| FRL_006 | Bilgi·Quality | `fare_attributes.txt` | MD-eşleşme |  |  |
| FRL_007 | Orta·Quality | — | İNCELE |  |  |
| FRL_008 | Bilgi·Quality | — | İNCELE |  |  |

## RCT

| ID | Önem·Sınıf | MD karşılığı | Tür | Bilinçli sapma? | Gerekçe / parite |
|---|---|---|---|---|---|
| RCT_001 | Kritik·Spec | `duplicate_key` | MD-eşleşme |  |  |
| RCT_002 | Kritik·Spec | `missing_required_field` | MD-eşleşme |  |  |
| RCT_003 | Kritik·Spec | `unexpected_enum_value` | MD-eşleşme |  |  |
| RCT_004 | Orta·Spec | `number_out_of_range` | proje-özel (yakın MD var) |  |  |
| RCT_005 | Orta·Spec | — | proje-özel |  |  |
| RCT_006 | Orta·Spec | — | proje-özel |  |  |

## FMD

| ID | Önem·Sınıf | MD karşılığı | Tür | Bilinçli sapma? | Gerekçe / parite |
|---|---|---|---|---|---|
| FMD_001 | Kritik·Spec | `fare_media_id` | MD-eşleşme |  |  |
| FMD_002 | Kritik·Spec | `fare_media_type` | MD-eşleşme |  |  |
| FMD_003 | Düşük·Quality | — | İNCELE |  |  |

## FPD

| ID | Önem·Sınıf | MD karşılığı | Tür | Bilinçli sapma? | Gerekçe / parite |
|---|---|---|---|---|---|
| FPD_001 | Kritik·Spec | `fare_product_id` | MD-eşleşme |  |  |
| FPD_002 | Kritik·Spec | — | İNCELE |  |  |
| FPD_003 | Kritik·Spec | — | İNCELE |  |  |
| FPD_004 | Kritik·Spec | `fare_media_id` | MD-eşleşme |  |  |
| FPD_005 | Kritik·Spec | `rider_category_id` | MD-eşleşme |  |  |
| FPD_006 | Orta·Spec | — | İNCELE |  |  |

## FLG

| ID | Önem·Sınıf | MD karşılığı | Tür | Bilinçli sapma? | Gerekçe / parite |
|---|---|---|---|---|---|
| FLG_001 | Kritik·Spec | `fare_leg_rules.txt` | MD-eşleşme |  |  |
| FLG_002 | Kritik·Spec | `network_id` | MD-eşleşme |  |  |
| FLG_003 | Kritik·Spec | `from_area_id` | MD-eşleşme |  |  |
| FLG_004 | Kritik·Spec | `to_area_id` | MD-eşleşme |  |  |
| FLG_005 | Kritik·Spec | `from_timeframe_group_id` | MD-eşleşme |  |  |
| FLG_006 | Kritik·Spec | `to_timeframe_group_id` | MD-eşleşme |  |  |
| FLG_007 | Orta·Spec | `rule_priority` | MD-eşleşme |  |  |

## FTR

| ID | Önem·Sınıf | MD karşılığı | Tür | Bilinçli sapma? | Gerekçe / parite |
|---|---|---|---|---|---|
| FTR_001 | Kritik·Spec | `fare_transfer_rules.txt` | MD-eşleşme |  |  |
| FTR_002 | Kritik·Spec | `fare_transfer_rules.txt` | MD-eşleşme |  |  |
| FTR_003 | Kritik·Spec | `fare_transfer_rules.txt` | MD-eşleşme |  |  |
| FTR_004 | Kritik·Spec | `fare_transfer_rules.txt` | MD-eşleşme |  |  |
| FTR_005 | Kritik·Spec | `fare_transfer_rules.txt` | MD-eşleşme |  |  |
| FTR_006 | Orta·Spec | `fare_transfer_rules.txt` | MD-eşleşme |  |  |
| FTR_007 | Orta·Spec | `fare_transfer_rules.txt` | MD-eşleşme |  |  |
| FTR_008 | Orta·Spec | `fare_transfer_rules.txt` | MD-eşleşme |  |  |

## ARS

| ID | Önem·Sınıf | MD karşılığı | Tür | Bilinçli sapma? | Gerekçe / parite |
|---|---|---|---|---|---|
| ARS_001 | Kritik·Spec | `duplicate_key` | MD-eşleşme |  |  |

## SAR

| ID | Önem·Sınıf | MD karşılığı | Tür | Bilinçli sapma? | Gerekçe / parite |
|---|---|---|---|---|---|
| SAR_001 | Kritik·Spec | `foreign_key_violation` | MD-eşleşme |  |  |
| SAR_002 | Kritik·Spec | `foreign_key_violation` | MD-eşleşme |  |  |

## NET

| ID | Önem·Sınıf | MD karşılığı | Tür | Bilinçli sapma? | Gerekçe / parite |
|---|---|---|---|---|---|
| NET_001 | Kritik·Spec | — | İNCELE |  |  |

## TFR

| ID | Önem·Sınıf | MD karşılığı | Tür | Bilinçli sapma? | Gerekçe / parite |
|---|---|---|---|---|---|
| TFR_001 | Kritik·Spec | `missing_required_field` | MD-eşleşme |  |  |
| TFR_002 | Kritik·Spec | `foreign_key_violation` | MD-eşleşme |  |  |
| TFR_003 | Yüksek·Spec | `invalid_time` | MD-eşleşme |  |  |
| TFR_004 | Orta·Spec | — | proje-özel |  |  |
| TFR_005 | Orta·Spec | — | proje-özel |  |  |

## PTH

| ID | Önem·Sınıf | MD karşılığı | Tür | Bilinçli sapma? | Gerekçe / parite |
|---|---|---|---|---|---|
| PTH_001 | Kritik·Spec | — | İNCELE |  |  |
| PTH_002 | Kritik·Spec | — | İNCELE |  |  |
| PTH_003 | Kritik·Spec | — | İNCELE |  |  |
| PTH_004 | Kritik·Spec | — | İNCELE |  |  |
| PTH_005 | Kritik·Spec | — | İNCELE |  |  |
| PTH_006 | Orta·Spec | — | İNCELE |  |  |
| PTH_007 | Orta·Spec | — | İNCELE |  |  |
| PTH_008 | Düşük·Quality | — | İNCELE |  |  |
| PTH_009 | Düşük·Quality | — | İNCELE |  |  |
| PTH_010 | Düşük·Spec | — | İNCELE |  |  |
| PTH_011 | Yüksek·Spec | — | İNCELE |  |  |
| PTH_012 | Yüksek·Interop | — | İNCELE |  |  |
| PTH_013 | Bilgi·Analytics | — | İNCELE |  |  |
| PTH_014 | Kritik·Spec | — | İNCELE |  |  |
| PTH_015 | Orta·Analytics | — | İNCELE |  |  |
| PTH_016 | Yüksek·Spec | — | İNCELE |  |  |
| PTH_017 | Orta·Spec | — | İNCELE |  |  |
| PTH_018 | Düşük·Quality | — | İNCELE |  |  |
| PTH_019 | Orta·Quality | — | İNCELE |  |  |

## LVL

| ID | Önem·Sınıf | MD karşılığı | Tür | Bilinçli sapma? | Gerekçe / parite |
|---|---|---|---|---|---|
| LVL_001 | Kritik·Spec | — | İNCELE |  |  |
| LVL_002 | Kritik·Spec | `levels.txt.level_index` | MD-eşleşme |  |  |
| LVL_003 | Düşük·Quality | `level_name` | MD-eşleşme |  |  |
| LVL_004 | Düşük·Quality | — | İNCELE |  |  |
| LVL_005 | Orta·Quality | — | İNCELE |  |  |
| LVL_006 | Orta·Quality | `level_id` | MD-eşleşme |  |  |

## FIN

| ID | Önem·Sınıf | MD karşılığı | Tür | Bilinçli sapma? | Gerekçe / parite |
|---|---|---|---|---|---|
| FIN_001 | Kritik·Spec | `missing_required_field` | MD-eşleşme |  |  |
| FIN_002 | Kritik·Spec | `missing_required_field` | MD-eşleşme |  |  |
| FIN_003 | Kritik·Spec | `missing_required_field` | MD-eşleşme |  |  |
| FIN_004 | Orta·Spec | `invalid_language_code` | MD-eşleşme |  |  |
| FIN_005 | Orta·Spec | `invalid_date` | MD-eşleşme |  |  |
| FIN_006 | Yüksek·Spec | `invalid_date` | MD-eşleşme |  |  |
| FIN_007 | Düşük·Quality | `missing_recommended_field` | MD-eşleşme |  |  |
| FIN_008 | Düşük·Spec | `invalid_email` | MD-eşleşme |  |  |
| FIN_009 | Düşük·Spec | `invalid_url` | MD-eşleşme |  |  |
| FIN_010 | Yüksek·Analytics | `feed_expiration_date7_days` | MD-eşleşme |  |  |
| FIN_012 | Düşük·Quality | `start_and_end_range_out_of_order` | MD-eşleşme |  |  |
| FIN_013 | Bilgi·Quality | — | proje-özel |  |  |
| FIN_014 | Düşük·Quality | `missing_feed_info_date` | MD-eşleşme |  |  |
| FIN_015 | Orta·Quality | `more_than_one_entity` | MD-eşleşme |  |  |
| FIN_016 | Düşük·Quality | — | proje-özel |  |  |
| FIN_017 | Bilgi·Quality | — | proje-özel |  |  |
| FIN_018 | Düşük·Quality | `missing_recommended_field` | proje-özel (yakın MD var) |  |  |
| FIN_019 | Düşük·Quality | `feed_expiration_date7_days` | MD-eşleşme |  |  |
| FIN_020 | Orta·Quality | — | proje-özel |  |  |

## TRN

| ID | Önem·Sınıf | MD karşılığı | Tür | Bilinçli sapma? | Gerekçe / parite |
|---|---|---|---|---|---|
| TRN_001 | Kritik·Spec | `translation_unknown_table_name` | MD-eşleşme |  |  |
| TRN_002 | Kritik·Spec | — | proje-özel |  |  |
| TRN_003 | Orta·Spec | `invalid_language_code` | MD-eşleşme |  |  |
| TRN_004 | Yüksek·Spec | `translation_foreign_key_violation` | MD-eşleşme |  |  |
| TRN_005 | Kritik·Spec | `duplicate_key` | MD-eşleşme |  |  |
| TRN_006 | Kritik·Spec | `duplicate_key` | MD-eşleşme |  |  |
| TRN_007 | Düşük·Quality | — | proje-özel |  |  |
| TRN_008 | Bilgi·Quality | `missing_required_field` | proje-özel (yakın MD var) |  |  |
| TRN_009 | Yüksek·Spec | — | proje-özel |  |  |
| TRN_010 | Yüksek·Spec | — | proje-özel |  |  |
| TRN_011 | Yüksek·Spec | — | proje-özel |  |  |
| TRN_013 | Yüksek·Spec | — | proje-özel |  |  |
| TRN_014 | Yüksek·Spec | — | proje-özel |  |  |

## ATR

| ID | Önem·Sınıf | MD karşılığı | Tür | Bilinçli sapma? | Gerekçe / parite |
|---|---|---|---|---|---|
| ATR_001 | Yüksek·Spec | `missing_required_field` | MD-eşleşme |  |  |
| ATR_002 | Kritik·Spec | `missing_required_field` | MD-eşleşme |  |  |
| ATR_003 | Yüksek·Spec | — | İNCELE |  |  |
| ATR_004 | Kritik·Spec | — | İNCELE |  |  |
| ATR_005 | Kritik·Spec | — | İNCELE |  |  |
| ATR_006 | Kritik·Spec | `foreign_key_violation` | MD-eşleşme |  |  |
| ATR_007 | Kritik·Spec | `invalid_url` | MD-eşleşme |  |  |
| ATR_008 | Düşük·Spec | `invalid_email` | MD-eşleşme |  |  |
| ATR_009 | Yüksek·Spec | — | İNCELE |  |  |
| ATR_010 | Düşük·Spec | `foreign_key_violation` | MD-eşleşme |  |  |

## XFL

| ID | Önem·Sınıf | MD karşılığı | Tür | Bilinçli sapma? | Gerekçe / parite |
|---|---|---|---|---|---|
| XFL_001 | Kritik·Spec | `foreign_key_violation` | MD-eşleşme |  |  |
| XFL_002 | Yüksek·Spec | `unusable_trip` | MD-eşleşme |  |  |
| XFL_003 | Yüksek·Spec | `foreign_key_violation` | MD-eşleşme |  |  |
| XFL_004 | Kritik·Spec | `foreign_key_violation` | MD-eşleşme |  |  |
| XFL_005 | Kritik·Spec | `foreign_key_violation` | MD-eşleşme |  |  |
| XFL_006 | Orta·Spec | — | proje-özel |  |  |
| XFL_007 | Kritik·Spec | `foreign_key_violation` | MD-eşleşme |  |  |
| XFL_009 | Kritik·Spec | `foreign_key_violation` | MD-eşleşme |  |  |
| XFL_010 | Kritik·Spec | `foreign_key_violation` | MD-eşleşme |  |  |
| XFL_011 | Orta·Interop | `service_window_outside_feed_period` | MD-eşleşme |  |  |
| XFL_012 | Yüksek·Quality | `unusable_trip` | MD-eşleşme |  |  |
| XFL_013 | Yüksek·Interop | — | proje-özel |  |  |
| XFL_014 | Orta·Quality | `translation_foreign_key_violation` | MD-eşleşme |  |  |
| XFL_015 | Kritik·Spec | `foreign_key_violation` | MD-eşleşme |  |  |
| XFL_016 | Yüksek·Spec | `translation_foreign_key_violation` | MD-eşleşme |  |  |
| XFL_017 | Düşük·Quality | `route_cemv_support` | MD-eşleşme |  |  |
| XFL_018 | Orta·Quality | `missing_recommended_file` | MD-eşleşme |  |  |
| XFL_019 | Orta·Spec | `route_networks_specified_in_more_than_one_file` | MD-eşleşme |  |  |
| XFL_020 | Yüksek·Spec | — | proje-özel |  |  |
| XFL_021 | Yüksek·Spec | — | proje-özel |  |  |

## OPR

| ID | Önem·Sınıf | MD karşılığı | Tür | Bilinçli sapma? | Gerekçe / parite |
|---|---|---|---|---|---|
| OPR_001 | Orta·Analytics | — | İNCELE |  |  |
| OPR_003 | Düşük·Analytics | — | İNCELE |  |  |
| OPR_004 | Bilgi·Analytics | — | İNCELE |  |  |
| OPR_005 | Bilgi·Analytics | — | İNCELE |  |  |
| OPR_006 | Yüksek·Analytics | — | İNCELE |  |  |
| OPR_007 | Orta·Analytics | — | İNCELE |  |  |
| OPR_008 | Yüksek·Analytics | — | İNCELE |  |  |
| OPR_009 | Bilgi·Analytics | — | İNCELE |  |  |
| OPR_010 | Orta·Analytics | — | İNCELE |  |  |
| OPR_011 | Yüksek·Analytics | `service_id` | MD-eşleşme |  |  |
| OPR_012 | Orta·Analytics | `service_gap_days` | MD-eşleşme |  |  |
| OPR_013 | Bilgi·Analytics | — | İNCELE |  |  |
| OPR_014 | Orta·Analytics | — | İNCELE |  |  |
| OPR_015 | Bilgi·Analytics | — | İNCELE |  |  |
| OPR_016 | Bilgi·Analytics | — | İNCELE |  |  |
| OPR_017 | Orta·Analytics | — | İNCELE |  |  |
| OPR_018 | Orta·Analytics | — | İNCELE |  |  |
| OPR_019 | Bilgi·Analytics | — | İNCELE |  |  |
| OPR_020 | Yüksek·Analytics | — | İNCELE |  |  |
| OPR_021 | Yüksek·Analytics | — | İNCELE |  |  |
| OPR_022 | Yüksek·Analytics | — | İNCELE |  |  |
| OPR_023 | Orta·Analytics | — | İNCELE |  |  |
| OPR_024 | Bilgi·Analytics | — | İNCELE |  |  |
| OPR_025 | Yüksek·Analytics | — | İNCELE |  |  |

## GEO

| ID | Önem·Sınıf | MD karşılığı | Tür | Bilinçli sapma? | Gerekçe / parite |
|---|---|---|---|---|---|
| GEO_002 | Yüksek·Analytics | — | İNCELE |  |  |
| GEO_006 | Yüksek·Analytics | — | İNCELE |  |  |
| GEO_007 | Yüksek·Analytics | — | İNCELE |  |  |
| GEO_009 | Yüksek·Quality | — | İNCELE |  |  |
| GEO_012 | Orta·Analytics | — | İNCELE |  |  |
| GEO_013 | Bilgi·Analytics | — | İNCELE |  |  |
| GEO_014 | Bilgi·Analytics | — | İNCELE |  |  |
| GEO_015 | Orta·Quality | — | İNCELE |  |  |
| GEO_016 | Yüksek·Quality | — | İNCELE |  |  |
| GEO_017 | Yüksek·Quality | — | İNCELE |  |  |
| GEO_018 | Yüksek·Analytics | — | İNCELE |  |  |
| GEO_019 | Orta·Quality | — | İNCELE |  |  |
| GEO_020 | Yüksek·Quality | — | İNCELE |  |  |
| GEO_021 | Yüksek·Analytics | — | İNCELE |  |  |

## DQ

| ID | Önem·Sınıf | MD karşılığı | Tür | Bilinçli sapma? | Gerekçe / parite |
|---|---|---|---|---|---|
| DQ_001 | Düşük·Quality | `missing_required_field` | MD-eşleşme |  |  |
| DQ_002 | Düşük·Quality | `missing_required_field` | MD-eşleşme |  |  |
| DQ_003 | Bilgi·Quality | `missing_required_field` | MD-eşleşme |  |  |
| DQ_004 | Düşük·Quality | `missing_required_field` | MD-eşleşme |  |  |
| DQ_005 | Yüksek·Interop | `missing_required_field` | MD-eşleşme |  |  |
| DQ_005b | Yüksek·Interop | `missing_required_field` | MD-eşleşme |  |  |
| DQ_005c | Yüksek·Interop | `missing_required_field` | MD-eşleşme |  |  |
| DQ_006 | Yüksek·Quality | `missing_required_field` | MD-eşleşme |  |  |
| DQ_009 | Bilgi·Quality | `missing_required_field` | MD-eşleşme |  |  |
| DQ_010 | Bilgi·Quality | `missing_required_field` | MD-eşleşme |  |  |
| DQ_011 | Düşük·Quality | `missing_required_field` | MD-eşleşme |  |  |
| DQ_012 | Düşük·Quality | `missing_required_field` | MD-eşleşme |  |  |
| DQ_013 | Orta·Quality | `missing_required_field` | MD-eşleşme |  |  |
| DQ_016 | Orta·Quality | `missing_required_field` | MD-eşleşme |  |  |
| DQ_017 | Bilgi·Quality | `missing_required_field` | MD-eşleşme |  |  |
| DQ_018 | Orta·Quality | `missing_required_field` | MD-eşleşme |  |  |
| DQ_019 | Orta·Quality | `missing_required_field` | MD-eşleşme |  |  |
| DQ_020 | Düşük·Quality | `missing_required_field` | MD-eşleşme |  |  |
| DQ_021 | Yüksek·Spec | `missing_required_field` | MD-eşleşme |  |  |
| DQ_022 | Yüksek·Quality | `missing_required_field` | MD-eşleşme |  |  |

## VAT

| ID | Önem·Sınıf | MD karşılığı | Tür | Bilinçli sapma? | Gerekçe / parite |
|---|---|---|---|---|---|
| VAT_001 | Orta·Analytics | — | proje-özel |  |  |
| VAT_002 | Bilgi·Analytics | — | proje-özel |  |  |
| VAT_003 | Düşük·Analytics | — | proje-özel |  |  |
| VAT_004 | Bilgi·Analytics | — | proje-özel |  |  |
| VAT_005 | Orta·Analytics | — | proje-özel |  |  |
| VAT_006 | Bilgi·Analytics | — | proje-özel |  |  |
| VAT_007 | Bilgi·Analytics | — | proje-özel |  |  |
| VAT_008 | Bilgi·Analytics | — | proje-özel |  |  |
