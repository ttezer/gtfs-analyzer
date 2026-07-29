# GTFS Validator & Analyzer — Rule List

🇹🇷 [Türkçe](RULES.md) · 🇬🇧 **English** · 🇯🇵 [日本語](RULES.ja.md)

542 rules, 37 groups. Each rule is identified by a unique ID, severity level, and class.
Severity levels: **CRITICAL** (publish blocker) · **HIGH** · **MEDIUM** · **LOW** · **INFO**
Classes: **Spec** (GTFS Validity) · **Interop** (GTFS Interoperability) · **Quality** (GTFS Quality) · **Analytics** (GTFS Analytics)

---

## ARC — Archive / File Level

| Rule | Title | Severity | Class |
|---|---|---|---|
| ARC_001 | ZIP archive could not be opened | CRITICAL | Spec |
| ARC_002 | File cannot be read as UTF-8 | CRITICAL | Quality |
| ARC_003 | UTF-8 encoding error in optional file | MEDIUM | Quality |
| ARC_004 | Required file missing | CRITICAL | Spec |
| ARC_006 | Optional GTFS file present | INFO | Quality |
| ARC_007 | Unrecognized non-GTFS file | INFO | Quality |
| ARC_008 | Calendar file missing (calendar.txt and calendar_dates.txt) | CRITICAL | Spec |
| ARC_009 | File has no data rows | CRITICAL | Quality |
| ARC_010 | File contains UTF-8 BOM | MEDIUM | Quality |
| ARC_011 | File size (info) | INFO | Analytics |
| ARC_012 | Row column count does not match header | CRITICAL | Spec |
| ARC_013 | CSV parse error | CRITICAL | Spec |
| ARC_014 | Leading/trailing whitespace in header | MEDIUM | Quality |
| ARC_015 | Duplicate header column | CRITICAL | Interop |
| ARC_025 | Required column missing from header | CRITICAL | Spec |
| ARC_017 | Unknown column (not defined in GTFS specification) | INFO | Quality |
| ARC_018 | Empty data row | MEDIUM | Quality |
| ARC_019 | Empty column name in header | HIGH | Quality |
| ARC_020 | Recommended GTFS file missing (shapes.txt or feed_info.txt) | LOW | Quality |
| ARC_021 | Non-printable or problematic character in field | LOW | Quality |
| ARC_022 | File row count exceeds 1,000,000 limit | LOW | Quality |
| ARC_023 | Nested ZIP file inside GTFS archive | MEDIUM | Quality |
| ARC_024 | GTFS .txt file in subdirectory (will not be parsed) | MEDIUM | Spec |
| ARC_026 | Malformed end-of-line characters | MEDIUM | Quality |
| ARC_027 | ZIP entry lacks user read permission | INFO | Quality |
| ARC_028 | GTFS publishing URL does not end with a .zip filename | LOW | Quality |
| ARC_029 | Zip-bomb protection triggered | CRITICAL | Quality |
| ARC_030 | Tab or line break in a field value | HIGH | Spec |

## BKR — Booking Rules

| Rule | Title | Severity | Class |
|---|---|---|---|
| BKR_001 | Prior-day booking field set in prohibited context | HIGH | Spec |
| BKR_002 | prior_notice_start_day only valid with prior_notice_last_day | HIGH | Spec |
| BKR_003 | prior_notice_start_time only valid with prior_notice_start_day | HIGH | Spec |
| BKR_004 | prior_notice fields prohibited for real-time booking | HIGH | Spec |
| BKR_005 | prior_notice_duration_max only valid with booking_type=1 | MEDIUM | Spec |
| BKR_006 | prior_notice_duration_min invalid (≤ 0 or non-numeric) | HIGH | Spec |
| BKR_007 | prior_notice_duration_min required for booking_type=1 | CRITICAL | Spec |
| BKR_008 | prior_notice_last_day required for booking_type=2 | CRITICAL | Spec |
| BKR_009 | prior_notice_last_time required for booking_type=2 | CRITICAL | Spec |
| BKR_010 | prior_notice_start_time required when prior_notice_start_day is set | HIGH | Spec |
| BKR_011 | prior_notice_last_day > prior_notice_start_day: invalid booking window | HIGH | Interop |
| BKR_012 | prior_notice_duration_min forbidden with booking_type=2 | MEDIUM | Spec |
| BKR_013 | prior_notice_last_time only valid with prior_notice_last_day | HIGH | Spec |
| BKR_014 | prior_notice_service_id only valid with booking_type=2 | HIGH | Spec |
| BKR_015 | prior_notice_service_id not found (calendar/calendar_dates) | CRITICAL | Spec |
| BKR_016 | booking_type missing or invalid | CRITICAL | Spec |
| BKR_017 | pickup_booking_rule_id not found (booking_rules) | CRITICAL | Spec |
| BKR_018 | drop_off_booking_rule_id not found (booking_rules) | CRITICAL | Spec |
| BKR_019 | booking_rule_id missing or duplicated | CRITICAL | Spec |

## AGN — Agency

| Rule | Title | Severity | Class |
|---|---|---|---|
| AGN_001 | agency.txt file missing | CRITICAL | Spec |
| AGN_002 | agency_name missing | CRITICAL | Spec |
| AGN_003 | agency_url missing or invalid | CRITICAL | Spec |
| AGN_004 | agency_timezone missing or invalid | CRITICAL | Spec |
| AGN_005 | Timezone inconsistency across agencies | MEDIUM | Quality |
| AGN_006 | agency_lang invalid | LOW | Spec |
| AGN_007 | agency_phone invalid | LOW | Quality |
| AGN_008 | agency_fare_url invalid | LOW | Spec |
| AGN_009 | agency_email invalid | LOW | Spec |
| AGN_010 | Duplicate agency_id | CRITICAL | Spec |
| AGN_011 | Multiple agencies with no agency_id | CRITICAL | Spec |
| AGN_012 | cemv_support invalid (agency) | LOW | Quality |
| AGN_013 | Feed language and agency language mismatch | LOW | Interop |
| AGN_014 | Multiple agencies but agency.txt agency_id missing | CRITICAL | Spec |
| AGN_015 | agency_url uses insecure http | INFO | Quality |
| AGN_016 | agency_phone suspect/placeholder | INFO | Quality |
| AGN_017 | Inconsistent agency_lang across agencies | LOW | Interop |

## STP — Stops

| Rule | Title | Severity | Class |
|---|---|---|---|
| STP_001 | Duplicate stop_id | CRITICAL | Spec |
| STP_002 | stop_id is empty | CRITICAL | Spec |
| STP_003 | stop_name missing or stop_lat out of range | CRITICAL | Spec |
| STP_004 | stop_lat is not numeric | CRITICAL | Spec |
| STP_005 | stop_lon invalid or out of range | CRITICAL | Spec |
| STP_006 | stop_lat missing | CRITICAL | Spec |
| STP_007 | stop_lon missing | CRITICAL | Spec |
| STP_008 | location_type invalid | HIGH | Spec |
| STP_009 | parent_station not found | CRITICAL | Spec |
| STP_010 | parent_station is not location_type=1 | HIGH | Spec |
| STP_011 | parent_station required for entrance/exit/boarding area | CRITICAL | Spec |
| STP_012 | Station or entrance used in stop_times | CRITICAL | Spec |
| STP_013 | wheelchair_boarding invalid | LOW | Spec |
| STP_014 | stop_timezone invalid | MEDIUM | Spec |
| STP_015 | level_id not found | CRITICAL | Spec |
| STP_016 | Two stops at identical coordinates | MEDIUM | Quality |
| STP_017 | Two stops too close together | LOW | Quality |
| STP_018 | No stops found | CRITICAL | Spec |
| STP_019 | stop_name too long | LOW | Quality |
| STP_020 | Stop with no trips | MEDIUM | Analytics |
| STP_021 | Boarding area parent is not a platform | HIGH | Quality |
| STP_022 | stop_code missing | MEDIUM | Quality |
| STP_023 | tts_stop_name invalid | LOW | Quality |
| STP_024 | stop_access invalid | INFO | Quality |
| STP_025 | stop_name has leading or trailing whitespace | MEDIUM | Quality |
| STP_026 | stop_access invalid value | LOW | Spec |
| STP_027 | stop_access not set on pathway station | MEDIUM | Quality |
| STP_028 | stop_code too long | INFO | Quality |
| STP_029 | Stop inside station but coordinate too far | MEDIUM | Quality |
| STP_030 | Parent station has no child stops | MEDIUM | Quality |
| STP_031 | Stop name and description are identical | INFO | Quality |
| STP_032 | parent_station missing for pathway-connected platform | MEDIUM | Quality |
| STP_033 | Stop zone_id missing (required for fare calculation) | INFO | Quality |
| STP_034 | stop_url matches agency URL | INFO | Quality |
| STP_035 | stop_url matches route URL | INFO | Quality |
| STP_036 | Station has parent_station (invalid) | LOW | Spec |
| STP_037 | Some stops have not set wheelchair boarding | MEDIUM | Quality |
| STP_038 | No stops report wheelchair boarding | INFO | Quality |
| STP_039 | stop_code is duplicated | LOW | Quality |
| STP_040 | Stop name contains a redundant stop/station word | LOW | Quality |
| STP_041 | Child stop name does not include the parent station name | LOW | Quality |

## RTS — Routes

| Rule | Title | Severity | Class |
|---|---|---|---|
| RTS_001 | Duplicate route_id | CRITICAL | Spec |
| RTS_002 | agency_id not found | CRITICAL | Spec |
| RTS_003 | Both route_short_name and route_long_name missing | CRITICAL | Spec |
| RTS_004 | route_type missing or invalid | CRITICAL | Spec |
| RTS_005 | route_url invalid | MEDIUM | Spec |
| RTS_006 | route_color invalid hex color | MEDIUM | Spec |
| RTS_007 | route_text_color invalid hex color | LOW | Quality |
| RTS_008 | Route color and text color have low contrast | MEDIUM | Quality |
| RTS_010 | route_short_name too long | LOW | Quality |
| RTS_011 | route_long_name too long | LOW | Quality |
| RTS_012 | Route with no trips | MEDIUM | Quality |
| RTS_013 | continuous_pickup invalid | LOW | Spec |
| RTS_016 | Route with no active service days | LOW | Quality |
| RTS_017 | Route with no shape defined | INFO | Quality |
| RTS_018 | continuous_drop_off invalid | LOW | Spec |
| RTS_019 | Duplicate route name | MEDIUM | Quality |
| RTS_020 | Route and agency share the same URL | INFO | Quality |
| RTS_021 | route_short_name exceeds Google Transit limit (6 characters) | LOW | Quality |
| RTS_022 | route_long_name contains route_short_name | LOW | Quality |
| RTS_023 | route_desc repeats a route name | INFO | Quality |
| RTS_024 | cemv_support invalid (route) | LOW | Quality |
| RTS_025 | routes.txt agency_id empty (recommended) | INFO | Quality |
| RTS_026 | Duplicate short route name | INFO | Quality |
| RTS_027 | Duplicate long route name | INFO | Quality |
| RTS_028 | continuous_pickup/drop_off forbidden on a Flex route | HIGH | Interop |

## TRP — Trips

| Rule | Title | Severity | Class |
|---|---|---|---|
| TRP_001 | trip_id missing or duplicate | CRITICAL | Spec |
| TRP_002 | route_id not found | CRITICAL | Spec |
| TRP_003 | service_id not found | CRITICAL | Spec |
| TRP_004 | shape_id not found | HIGH | Spec |
| TRP_005 | direction_id invalid | MEDIUM | Spec |
| TRP_006 | wheelchair_accessible invalid | LOW | Spec |
| TRP_007 | bikes_allowed invalid | LOW | Spec |
| TRP_032 | cars_allowed invalid | LOW | Spec |
| TRP_011 | Trip headsign not set | HIGH | Quality |
| TRP_012 | direction_id missing on bidirectional route | LOW | Quality |
| TRP_013 | Route has only one trip | LOW | Quality |
| TRP_014 | trip_short_name too long | INFO | Quality |
| TRP_015 | Single trip in block_id group | LOW | Quality |
| TRP_017 | Frequency-based trip missing from stop_times | MEDIUM | Quality |
| TRP_019 | shape_id missing with continuous service active | HIGH | Quality |
| TRP_020 | trip_headsign matches intermediate stop name | INFO | Analytics |
| TRP_021 | Bicycle allowance (bikes_allowed) not specified | INFO | Quality |
| TRP_022 | Overlapping trip times within block | HIGH | Interop |
| TRP_023 | No active trips in the next 7 days | LOW | Quality |
| TRP_024 | Inconsistent route type within block | LOW | Quality |
| TRP_025 | High proportion of trips without wheelchair accessibility information | INFO | Quality |
| TRP_026 | Trip with empty active-date set | MEDIUM | Analytics |
| TRP_028 | Some trips have not set wheelchair accessibility | MEDIUM | Quality |
| TRP_029 | No trips report wheelchair accessibility | INFO | Quality |
| TRP_031 | route_id missing | CRITICAL | Spec |
| TRP_033 | Trips sharing a block_id carry different route types | MEDIUM | Quality |

## STM — Stop Times

| Rule | Title | Severity | Class |
|---|---|---|---|
| STM_001 | trip_id not found | CRITICAL | Spec |
| STM_002 | stop_id not found | CRITICAL | Spec |
| STM_003 | arrival_time invalid format | CRITICAL | Spec |
| STM_004 | departure_time invalid format | CRITICAL | Spec |
| STM_005 | stop_sequence missing or invalid | CRITICAL | Spec |
| STM_006 | stop_id missing (stop_times) | CRITICAL | Spec |
| STM_007 | Departure time before arrival time | HIGH | Interop |
| STM_008 | Time decreases between stops | CRITICAL | Interop |
| STM_009 | pickup_type invalid | HIGH | Spec |
| STM_010 | drop_off_type invalid | HIGH | Spec |
| STM_012 | Unrealistic speed between stops | HIGH | Interop |
| STM_013 | Mixed arrival/departure times | HIGH | Quality |
| STM_014 | Excessive speed in segment | HIGH | Analytics |
| STM_015 | First stop is missing a required time | CRITICAL | Spec |
| STM_016 | Last stop is missing a required time | CRITICAL | Spec |
| STM_017 | Shape distance missing in stop times | MEDIUM | Quality |
| STM_018 | continuous_pickup invalid (stop_times) | MEDIUM | Spec |
| STM_019 | continuous_drop_off invalid (stop_times) | MEDIUM | Spec |
| STM_020 | Zero travel time (distance > 200m) | HIGH | Quality |
| STM_021 | Distance between stops is zero or negative | HIGH | Quality |
| STM_022 | timepoint invalid | MEDIUM | Spec |
| STM_024 | shape_dist_traveled unit inconsistency | INFO | Quality |
| STM_025 | Short segment timing | INFO | Analytics |
| STM_026 | Excessive distance between stops | HIGH | Quality |
| STM_028 | Trip duration too long | HIGH | Analytics |
| STM_029 | Trip duration too short | MEDIUM | Analytics |
| STM_030 | shape_dist_traveled is negative | LOW | Spec |
| STM_032 | Duplicate stop_sequence within trip | LOW | Quality |
| STM_033 | Single-stop trip (unusable) | HIGH | Interop |
| STM_034 | Only one of arrival or departure time defined | MEDIUM | Interop |
| STM_035 | Same stop visited twice consecutively (terminal/loop) | INFO | Analytics |
| STM_036 | stop_times not sorted by trip_id + stop_sequence (unsorted_stop_times) | INFO | Quality |
| STM_037 | arrival_time/departure_time prohibited in Flex window | HIGH | Spec |
| STM_038 | start_pickup_drop_off_window > end_pickup_drop_off_window | HIGH | Interop |
| STM_039 | Pickup/drop-off window missing in Flex context | CRITICAL | Spec |
| STM_040 | pickup/drop_off_booking_rule_id missing in Flex stop_times | CRITICAL | Spec |
| STM_041 | stop_id and location_id/group_id cannot be used together | HIGH | Spec |
| STM_042 | stop_headsign contains characters unsupported by Google Transit | LOW | Interop |
| STM_043 | Trip has extreme stop count (>200) | INFO | Analytics |
| STM_044 | Feed stop_times exceeds 2,000,000 rows (WASM performance warning) | INFO | Analytics |
| STM_045 | Trip departure time exceeds the service day window | MEDIUM | Quality |
| STM_046 | trip_id missing | CRITICAL | Spec |
| STM_047 | timepoint=1 without arrival/departure time | CRITICAL | Spec |
| STM_048 | After-midnight times written as 00:xx (between stops) | INFO | Quality |
| STM_049 | After-midnight departure written as 00:xx (same row) | INFO | Quality |
| STM_050 | timepoint column present but value empty | LOW | Quality |
| STM_051 | pickup_type 0/3 forbidden with Flex window | HIGH | Spec |
| STM_052 | drop_off_type 0 forbidden with Flex window | HIGH | Spec |
| STM_053 | Many consecutive stops have the same time | MEDIUM | Quality |
| STM_054 | continuous_pickup forbidden with Flex window | HIGH | Spec |
| STM_055 | continuous_drop_off forbidden with Flex window | HIGH | Spec |
| STM_057 | Only one stop_times record for a Flex location | CRITICAL | Spec |
| STM_056 | shape_dist_traveled does not increase | CRITICAL | Spec |

## PDW — Pickup/Drop-off Window

| Rule | Title | Severity | Class |
|---|---|---|---|
| PDW_006 | Overlapping pickup/drop-off window for same trip+zone | MEDIUM | Analytics |

## LOC — locations.geojson

| Rule | Title | Severity | Class |
|---|---|---|---|
| LOC_001 | Unknown or invalid geometry type in locations.geojson | HIGH | Spec |
| LOC_002 | Feature has null or missing geometry | CRITICAL | Spec |
| LOC_003 | Feature missing required 'id' property | CRITICAL | Spec |
| LOC_004 | Polygon ring is not closed | MEDIUM | Spec |
| LOC_005 | FeatureCollection has no features | LOW | Quality |
| LOC_006 | Polygon bounding box exceeds 500km² | MEDIUM | Quality |
| LOC_007 | Duplicate feature 'id' in FeatureCollection | MEDIUM | Spec |

## CAL — Calendar

| Rule | Title | Severity | Class |
|---|---|---|---|
| CAL_001 | Duplicate service_id | CRITICAL | Spec |
| CAL_002 | Calendar day field invalid value | CRITICAL | Spec |
| CAL_003 | start_date invalid | CRITICAL | Spec |
| CAL_004 | end_date invalid | CRITICAL | Spec |
| CAL_005 | start_date is after end_date | CRITICAL | Interop |
| CAL_006 | Weekly schedule has all days disabled | INFO | Quality |
| CAL_007 | Gap in service period | MEDIUM | Analytics |
| CAL_008 | Service expires soon | HIGH | Analytics |
| CAL_009 | All feed services have expired | CRITICAL | Quality |
| CAL_010 | Service has too few active days | MEDIUM | Analytics |
| CAL_011 | Unused service | LOW | Quality |
| CAL_012 | Service gap in the near future | INFO | Analytics |
| CAL_013 | Expired service period | INFO | Analytics |
| CAL_014 | Service dates outside feed_info validity range | LOW | Quality |
| CAL_015 | All calendar dates in the future (no active trips today) | LOW | Quality |
| CAL_016 | Service extends to a very distant future date | INFO | Quality |
| CAL_017 | Calendar has not yet started (all active dates in the future) | LOW | Quality |
| CAL_018 | Service has no active weekdays (all days 0, none overridden by calendar_dates) | LOW | Quality |
| CAL_019 | Service calendar dates outside feed_info validity window | LOW | Quality |
| CAL_020 | Feed validity window exceeds 5 years | LOW | Quality |
| CAL_021 | Active today but no service in the upcoming days | INFO | Analytics |
| CAL_022 | service_id missing | CRITICAL | Spec |
| CAL_023 | Calendar end_date far in the future (suspect) | MEDIUM | Quality |
| CAL_024 | Calendar inactive in the next 7 days | LOW | Quality |

## CLD — Calendar Dates

| Rule | Title | Severity | Class |
|---|---|---|---|
| CLD_001 | service_id missing | CRITICAL | Spec |
| CLD_002 | date invalid format | CRITICAL | Spec |
| CLD_003 | exception_type missing or invalid | CRITICAL | Spec |
| CLD_004 | Calendar-dates-only service has no active dates | HIGH | Quality |
| CLD_005 | Date out of range | CRITICAL | Quality |
| CLD_006 | Too many exception days | MEDIUM | Quality |
| CLD_007 | Excessive calendar exceptions | INFO | Analytics |

## SHP — Shapes

| Rule | Title | Severity | Class |
|---|---|---|---|
| SHP_001 | shape_id missing | CRITICAL | Spec |
| SHP_002 | shape_pt_lat missing or invalid | CRITICAL | Spec |
| SHP_003 | shape_pt_lon missing or invalid | CRITICAL | Spec |
| SHP_004 | shape_pt_sequence missing or invalid | CRITICAL | Spec |
| SHP_005 | shape_dist_traveled decreases | CRITICAL | Spec |
| SHP_006 | Shape consists of a single point only | MEDIUM | Quality |
| SHP_008 | Duplicate shape_pt_sequence | CRITICAL | Spec |
| SHP_009 | Shape self-intersects | INFO | Analytics |
| SHP_010 | Repeated shape point (consecutive identical coordinates) | LOW | Quality |
| SHP_012 | Shape too far from trip stops | HIGH | Analytics |
| SHP_014 | First or last stop far from shape endpoint | INFO | Analytics |
| SHP_015 | Shape has statistically too few points | MEDIUM | Quality |
| SHP_016 | Shape direction inconsistent with trip direction | HIGH | Quality |
| SHP_017 | Stop sequence conflicts with shape | INFO | Analytics |
| SHP_018 | Shape not referenced by any trip | LOW | Quality |
| SHP_019 | Shape's trips have no stop times | MEDIUM | Quality |
| SHP_020 | Repeated point in shape | INFO | Analytics |
| SHP_021 | shape_dist_traveled negative value | LOW | Quality |
| SHP_022 | Stop position ambiguous on shape | HIGH | Quality |
| SHP_023 | Consecutive points with same shape_dist_traveled at same coordinates | MEDIUM | Quality |
| SHP_024 | Stop-to-shape distance inconsistent with shape_dist_traveled | MEDIUM | Quality |
| SHP_025 | Trip stop_times distance exceeds total shape distance | MEDIUM | Quality |
| SHP_026 | Shape has extreme point count (>5,000) | INFO | Analytics |
| SHP_027 | Shape assigned to multiple stop patterns | INFO | Analytics |
| SHP_028 | Same shape_dist_traveled with different coordinates | HIGH | Quality |
| SHP_029 | Same shape_dist_traveled, tiny coordinate difference | INFO | Quality |

## FRQ — Frequencies

| Rule | Title | Severity | Class |
|---|---|---|---|
| FRQ_001 | trip_id not found | CRITICAL | Spec |
| FRQ_002 | start_time invalid | CRITICAL | Spec |
| FRQ_003 | end_time invalid | CRITICAL | Spec |
| FRQ_004 | headway_secs missing or invalid | CRITICAL | Spec |
| FRQ_005 | end_time is before start_time | CRITICAL | Quality |
| FRQ_006 | headway_secs too long | MEDIUM | Analytics |
| FRQ_007 | exact_times invalid | MEDIUM | Spec |
| FRQ_008 | headway_secs is zero (invalid frequency) | CRITICAL | Spec |
| FRQ_009 | Frequency interval too short | MEDIUM | Quality |
| FRQ_010 | Very high frequency (bunching risk) | INFO | Analytics |
| FRQ_011 | Overlapping frequency periods | HIGH | Interop |

## TRF — Transfers

| Rule | Title | Severity | Class |
|---|---|---|---|
| TRF_001 | from_stop_id missing | CRITICAL | Spec |
| TRF_002 | to_stop_id missing | CRITICAL | Spec |
| TRF_003 | from_stop_id or to_stop_id not found | CRITICAL | Spec |
| TRF_004 | transfer_type invalid | HIGH | Spec |
| TRF_005 | min_transfer_time missing | CRITICAL | Spec |
| TRF_006 | from_trip_id not found | CRITICAL | Spec |
| TRF_007 | to_trip_id not found | CRITICAL | Spec |
| TRF_008 | from_route_id not found | CRITICAL | Spec |
| TRF_009 | to_route_id not found | CRITICAL | Spec |
| TRF_010 | Transfer time too long | MEDIUM | Analytics |
| TRF_011 | Transfer defined but distance is far | INFO | Quality |
| TRF_012 | Duplicate transfer record | CRITICAL | Spec |
| TRF_013 | Transfer type inconsistent with context | CRITICAL | Quality |
| TRF_014 | No trip for in-seat transfer | HIGH | Spec |
| TRF_015 | in-seat transfer invalid | HIGH | Quality |
| TRF_016 | Transfer condition conflicting | CRITICAL | Spec |
| TRF_017 | Trip transfer on wrong route | HIGH | Interop |
| TRF_018 | Trip transfer references the same trip | MEDIUM | Quality |
| TRF_019 | Different route_type in in-seat transfer | MEDIUM | Interop |
| TRF_020 | Transfer walking speed is too fast | MEDIUM | Quality |
| TRF_021 | Transfer endpoint is not a stop or station | CRITICAL | Spec |

## GGL — Google Transit Compatibility

| Rule | Title | Severity | Class |
|---|---|---|---|
| GGL_001 | transfer_type=4/5 not supported by Google Transit | LOW | Interop |
| GGL_002 | ic_price (Google-specific) invalid value | LOW | Interop |

## FAR — Fare Attributes

| Rule | Title | Severity | Class |
|---|---|---|---|
| FAR_001 | Duplicate fare_id | CRITICAL | Spec |
| FAR_002 | price missing or invalid | CRITICAL | Spec |
| FAR_003 | currency_type missing | CRITICAL | Spec |
| FAR_004 | payment_method invalid | CRITICAL | Spec |
| FAR_005 | transfers invalid | CRITICAL | Spec |
| FAR_006 | transfer_duration invalid | MEDIUM | Spec |
| FAR_008 | agency_id not found | CRITICAL | Spec |
| FAR_009 | Fare has no route rules | LOW | Quality |
| FAR_010 | Overlapping fare rules | MEDIUM | Quality |
| FAR_011 | payment_method missing | CRITICAL | Spec |
| FAR_012 | fare_id missing | CRITICAL | Spec |

## FRL — Fare Rules

| Rule | Title | Severity | Class |
|---|---|---|---|
| FRL_001 | fare_id not found | CRITICAL | Spec |
| FRL_002 | route_id not found | CRITICAL | Spec |
| FRL_003 | origin_id invalid | CRITICAL | Spec |
| FRL_004 | destination_id invalid | CRITICAL | Spec |
| FRL_005 | contains_id invalid | CRITICAL | Spec |
| FRL_006 | No fare rules defined | INFO | Quality |
| FRL_007 | Fare rule logical inconsistency | MEDIUM | Quality |
| FRL_008 | No fare defined for all routes | INFO | Quality |

## RCT — Rider Categories (Fares v2)

| Rule | Title | Severity | Class |
|---|---|---|---|
| RCT_001 | Duplicate rider_category_id | CRITICAL | Spec |
| RCT_002 | rider_category_name missing | CRITICAL | Spec |
| RCT_003 | is_default_fare_category invalid | CRITICAL | Spec |
| RCT_004 | min_age or max_age invalid | MEDIUM | Spec |
| RCT_005 | max_age less than min_age | MEDIUM | Quality |
| RCT_006 | Multiple default rider categories per fare_product | MEDIUM | Spec |

## FMD — Fare Media (Fares v2)

| Rule | Title | Severity | Class |
|---|---|---|---|
| FMD_001 | Duplicate fare_media_id | CRITICAL | Spec |
| FMD_002 | fare_media_type missing or invalid | CRITICAL | Spec |
| FMD_003 | fare_media_name recommended for TransitCard/MobileApp | LOW | Quality |

## FPD — Fare Products (Fares v2)

| Rule | Title | Severity | Class |
|---|---|---|---|
| FPD_001 | Duplicate fare_product_id | CRITICAL | Spec |
| FPD_002 | amount missing or negative | CRITICAL | Spec |
| FPD_003 | currency invalid ISO 4217 code | CRITICAL | Spec |
| FPD_004 | fare_media_id not found | CRITICAL | Spec |
| FPD_005 | rider_category_id not found | CRITICAL | Spec |
| FPD_006 | Multiple default rider categories for one fare_product | MEDIUM | Spec |

## FLG — Fare Leg Rules (Fares v2)

| Rule | Title | Severity | Class |
|---|---|---|---|
| FLG_001 | fare_product_id not found | CRITICAL | Spec |
| FLG_002 | network_id not found | CRITICAL | Spec |
| FLG_003 | from_area_id not found | CRITICAL | Spec |
| FLG_004 | to_area_id not found | CRITICAL | Spec |
| FLG_005 | from_timeframe_group_id not found | CRITICAL | Spec |
| FLG_006 | to_timeframe_group_id not found | CRITICAL | Spec |
| FLG_007 | rule_priority invalid | MEDIUM | Spec |

## FTR — Fare Transfer Rules (Fares v2)

| Rule | Title | Severity | Class |
|---|---|---|---|
| FTR_001 | fare_transfer_type missing or invalid | CRITICAL | Spec |
| FTR_002 | from_leg_group_id not found | CRITICAL | Spec |
| FTR_003 | to_leg_group_id not found | CRITICAL | Spec |
| FTR_004 | fare_product_id not found | CRITICAL | Spec |
| FTR_005 | duration_limit_type invalid | CRITICAL | Spec |
| FTR_006 | duration_limit invalid | MEDIUM | Spec |
| FTR_007 | duration_limit_type defined without duration_limit | MEDIUM | Spec |
| FTR_008 | transfer_count invalid | MEDIUM | Spec |
| FTR_009 | transfer_count required when leg groups equal | MEDIUM | Spec |
| FTR_010 | transfer_count forbidden when leg groups differ | MEDIUM | Spec |
| FTR_011 | duration_limit set but duration_limit_type missing | MEDIUM | Spec |

## ARS — Areas (Fares v2)

| Rule | Title | Severity | Class |
|---|---|---|---|
| ARS_001 | Duplicate area_id | CRITICAL | Spec |

## SAR — Stop Areas (Fares v2)

| Rule | Title | Severity | Class |
|---|---|---|---|
| SAR_001 | area_id not found | CRITICAL | Spec |
| SAR_002 | stop_id not found | CRITICAL | Spec |

## NET — Networks (Fares v2)

| Rule | Title | Severity | Class |
|---|---|---|---|
| NET_001 | Duplicate network_id | CRITICAL | Spec |

## TFR — Timeframes (Fares v2)

| Rule | Title | Severity | Class |
|---|---|---|---|
| TFR_001 | timeframe_group_id missing | CRITICAL | Spec |
| TFR_002 | service_id not found | CRITICAL | Spec |
| TFR_003 | start_time or end_time format error | HIGH | Spec |
| TFR_004 | end_time less than start_time | MEDIUM | Quality |
| TFR_005 | Overlapping time ranges within same group and service_id | MEDIUM | Interop |
| TFR_006 | start_time or end_time greater than 24:00:00 | CRITICAL | Spec |
| TFR_007 | Only one of start_time and end_time is specified | CRITICAL | Spec |

## PTH — Pathways

| Rule | Title | Severity | Class |
|---|---|---|---|
| PTH_001 | Duplicate pathway_id | CRITICAL | Spec |
| PTH_002 | from_stop_id not found | CRITICAL | Spec |
| PTH_003 | to_stop_id not found | CRITICAL | Spec |
| PTH_004 | pathway_mode missing or invalid | CRITICAL | Spec |
| PTH_005 | is_bidirectional missing | CRITICAL | Spec |
| PTH_006 | length invalid | MEDIUM | Spec |
| PTH_007 | traversal_time invalid | MEDIUM | Spec |
| PTH_008 | stair_count missing | LOW | Quality |
| PTH_009 | max_slope missing | LOW | Quality |
| PTH_010 | min_width invalid | LOW | Spec |
| PTH_011 | Pathway forms a loop | HIGH | Quality |
| PTH_012 | No accessible pathway to station | HIGH | Interop |
| PTH_013 | Accessible pathway analysis | INFO | Analytics |
| PTH_014 | Pathway crosses station boundary | CRITICAL | Quality |
| PTH_015 | Pathway leads to an unreachable stop | MEDIUM | Analytics |
| PTH_016 | Exit defined as bidirectional | HIGH | Spec |
| PTH_017 | max_slope invalid context | MEDIUM | Spec |
| PTH_018 | signposted_as too long | LOW | Quality |
| PTH_019 | Generic node connected to single pathway (dead-end) | MEDIUM | Quality |
| PTH_020 | pathway_id missing | CRITICAL | Spec |
| PTH_021 | from_stop_id missing | CRITICAL | Spec |
| PTH_022 | to_stop_id missing | CRITICAL | Spec |
| PTH_023 | pathway_mode missing | CRITICAL | Spec |
| PTH_024 | is_bidirectional missing | CRITICAL | Spec |
| PTH_025 | Recommended pathway length missing | LOW | Quality |
| PTH_026 | Pathway endpoint is a station | CRITICAL | Spec |

## LVL — Levels

| Rule | Title | Severity | Class |
|---|---|---|---|
| LVL_001 | Duplicate level_id | CRITICAL | Spec |
| LVL_002 | level_index invalid | CRITICAL | Spec |
| LVL_003 | level_name missing | LOW | Quality |
| LVL_004 | Unused level | LOW | Quality |
| LVL_005 | level_name too long | MEDIUM | Quality |
| LVL_006 | level_id missing on elevator-connected stop | MEDIUM | Quality |
| LVL_007 | level_index missing | CRITICAL | Spec |
| LVL_008 | level_id missing | CRITICAL | Spec |

## FIN — Feed Info

| Rule | Title | Severity | Class |
|---|---|---|---|
| FIN_001 | feed_publisher_name missing | CRITICAL | Spec |
| FIN_002 | feed_publisher_url missing or invalid | CRITICAL | Spec |
| FIN_003 | feed_lang missing | CRITICAL | Spec |
| FIN_004 | default_lang invalid | MEDIUM | Spec |
| FIN_005 | feed_start_date invalid | MEDIUM | Spec |
| FIN_006 | feed_end_date invalid or past | HIGH | Spec |
| FIN_007 | feed_version missing | LOW | Quality |
| FIN_008 | feed_contact_email invalid | LOW | Spec |
| FIN_009 | feed_contact_url invalid | LOW | Spec |
| FIN_010 | Feed validity expired | HIGH | Analytics |
| FIN_012 | feed_start_date is after feed_end_date | LOW | Quality |
| FIN_013 | fare_attributes.agency_id recommended but missing | INFO | Quality |
| FIN_014 | Feed validity dates (feed_start_date/feed_end_date) missing | LOW | Quality |
| FIN_015 | Multiple feed_info records | MEDIUM | Quality |
| FIN_016 | feed_start_date in the future (feed not yet active) | LOW | Quality |
| FIN_017 | Feed expires in the very distant future | INFO | Quality |
| FIN_018 | Both feed_contact_email and feed_contact_url missing | LOW | Quality |
| FIN_019 | Feed validity expires within 7 days | LOW | Quality |
| FIN_020 | Feed validity window shorter than 7 days | MEDIUM | Quality |

## TRN — Translations

| Rule | Title | Severity | Class |
|---|---|---|---|
| TRN_001 | table_name invalid value | CRITICAL | Spec |
| TRN_002 | field_name invalid for this table | CRITICAL | Spec |
| TRN_003 | language invalid | MEDIUM | Spec |
| TRN_004 | record_id not found | HIGH | Spec |
| TRN_005 | Duplicate translation | CRITICAL | Spec |
| TRN_006 | Translation record conflicting | CRITICAL | Spec |
| TRN_007 | Translation in same language as feed_lang | LOW | Quality |
| TRN_008 | translation value is empty | INFO | Quality |
| TRN_009 | record_id and field_value cannot be used together | HIGH | Spec |
| TRN_010 | record_sub_id invalid | HIGH | Spec |
| TRN_011 | field_name is not translatable | HIGH | Spec |
| TRN_013 | Identity field cannot be used in feed_info translation | HIGH | Spec |
| TRN_014 | record_sub_id only valid for stop_times | HIGH | Spec |

## ATR — Attributions

| Rule | Title | Severity | Class |
|---|---|---|---|
| ATR_001 | attribution_id missing | HIGH | Quality |
| ATR_002 | organization_name missing | CRITICAL | Spec |
| ATR_003 | Attribution role not defined | HIGH | Spec |
| ATR_004 | is_producer invalid | CRITICAL | Spec |
| ATR_005 | is_operator invalid | CRITICAL | Spec |
| ATR_006 | is_authority invalid | CRITICAL | Spec |
| ATR_007 | attribution_url invalid | CRITICAL | Spec |
| ATR_008 | attribution_email invalid | LOW | Spec |
| ATR_009 | multiple attribution reference fields set | HIGH | Quality |
| ATR_010 | agency_id not found | LOW | Spec |

## XFL — Cross-File / Semantic

| Rule | Title | Severity | Class |
|---|---|---|---|
| XFL_002 | Trip has no stop_times records | HIGH | Interop |
| XFL_006 | service_id contains only cancellation exceptions (no active days) | MEDIUM | Analytics |
| XFL_011 | Calendar dates outside feed_info range | MEDIUM | Interop |
| XFL_012 | Route with no operational trips | HIGH | Quality |
| XFL_013 | shape_id used in multiple directions | HIGH | Quality |
| XFL_014 | Invalid translation reference (source record not found) | MEDIUM | Quality |
| XFL_015 | Invalid reference in attribution | CRITICAL | Spec |
| XFL_016 | Translation references feed_info but feed_info.txt is missing | HIGH | Spec |
| XFL_017 | route_cemv_support conflicts with agency_cemv_support | LOW | Quality |
| XFL_019 | Network defined in two separate files (routes.network_id + route_networks.txt) | MEDIUM | Spec |
| XFL_020 | Invalid (from_trip_id/to_trip_id, route_id) pair in transfers | CRITICAL | Spec |
| XFL_021 | Invalid (from_trip_id/to_trip_id, stop_id) pair in transfers | HIGH | Interop |
| XFL_022 | location_group_id not found | CRITICAL | Spec |
| XFL_023 | stop_id not found (location_group_stops) | CRITICAL | Spec |
| XFL_024 | location_group_id not found (stop_times) | CRITICAL | Spec |
| XFL_025 | location_id not found (locations.geojson) | CRITICAL | Spec |
| XFL_026 | route cemv=1 but no applicable contactless product | MEDIUM | Quality |
| XFL_027 | route cemv=2 but applicable contactless product | MEDIUM | Quality |
| XFL_028 | agency cemv=1 but no contactless media | INFO | Quality |
| XFL_029 | route cemv=1 but no contactless media | INFO | Quality |
| XFL_030 | contactless media but no cemv=1 | INFO | Quality |

## OPR — Operational Consistency

| Rule | Title | Severity | Class |
|---|---|---|---|
| OPR_001 | Route headway too long | MEDIUM | Analytics |
| OPR_003 | Trip bunching (minimum headway too small) | LOW | Analytics |
| OPR_004 | No weekend service | INFO | Analytics |
| OPR_005 | Unusual service frequency | INFO | Analytics |
| OPR_006 | Trip has too few stops (not functional) | HIGH | Analytics |
| OPR_007 | Repeated stop pattern within trip | INFO | Analytics |
| OPR_008 | Excessive speed in multiple segments | HIGH | Analytics |
| OPR_009 | Night trip start time too late | INFO | Analytics |
| OPR_010 | Route accessibility or bicycle policy conflicts | MEDIUM | Analytics |
| OPR_011 | Service has no active days | HIGH | Analytics |
| OPR_012 | Service gap | MEDIUM | Analytics |
| OPR_013 | Route operates in one direction only (no return direction) | INFO | Analytics |
| OPR_014 | Average transfer time too long | MEDIUM | Analytics |
| OPR_015 | Route operates with a single shape only | INFO | Analytics |
| OPR_016 | No active service across the entire feed | INFO | Analytics |
| OPR_017 | Trip distance too short | MEDIUM | Analytics |
| OPR_019 | Route calendar overlap (multiple services on same day) | INFO | Analytics |
| OPR_020 | Route exception day overlap | LOW | Analytics |
| OPR_021 | Calendar override conflict: override and base simultaneously active | HIGH | Analytics |
| OPR_022 | Calendar override not applied: base service running on override day | HIGH | Analytics |
| OPR_023 | Calendar override gap: no service active within window | MEDIUM | Analytics |
| OPR_024 | Route has an extreme trip count | INFO | Analytics |
| OPR_025 | Feed average trip duration under 60 seconds | HIGH | Analytics |

## GEO — Geographic / Spatial

| Rule | Title | Severity | Class |
|---|---|---|---|
| GEO_002 | Stop too far from feed median | HIGH | Analytics |
| GEO_006 | Large jump in shape | HIGH | Analytics |
| GEO_007 | Critical jump in shape (3× threshold) | HIGH | Analytics |
| GEO_009 | Stop too far from shape route | HIGH | Quality |
| GEO_012 | Stop cluster (stops too close together) | MEDIUM | Analytics |
| GEO_013 | Feed geographic coverage summary | INFO | Analytics |
| GEO_014 | Feed geographic coverage too wide | INFO | Analytics |
| GEO_015 | Stop coordinates outside Japan bounds (feed_lang: ja) | MEDIUM | Quality |
| GEO_016 | Stop at or near Null Island (\|lat\|<0.1 and \|lon\|<0.1) | HIGH | Quality |
| GEO_017 | Shape point at or near Null Island | HIGH | Quality |
| GEO_018 | All feed stops within 200m radius (possible test data) | HIGH | Analytics |
| GEO_019 | Stop has integer (zero-precision) coordinates | MEDIUM | Quality |
| GEO_020 | Shape is degenerate — all points at the same location | HIGH | Quality |
| GEO_021 | More than 30% of stops share coordinates (systematic issue) | HIGH | Analytics |
| GEO_022 | Stop latitude near a pole (\|lat\| > 89) | HIGH | Quality |

## DQ — Data Quality / User Experience

| Rule | Title | Severity | Class |
|---|---|---|---|
| DQ_003 | Route description missing | INFO | Quality |
| DQ_004 | Route URL missing | INFO | Quality |
| DQ_005 | No valid service period | HIGH | Quality |
| DQ_005b | No trip has stop times | HIGH | Quality |
| DQ_005c | High proportion of stops without coordinates | HIGH | Quality |
| DQ_006 | High proportion of trips without shapes | HIGH | Quality |
| DQ_009 | No stop times in trips | INFO | Quality |
| DQ_010 | Agency not used by any route | INFO | Quality |
| DQ_011 | Only one stop exists | LOW | Quality |
| DQ_012 | Too many agencies, agency_id not used | LOW | Quality |
| DQ_013 | Too few trips | MEDIUM | Quality |
| DQ_016 | Extra whitespace in field value | MEDIUM | Quality |
| DQ_017 | Suspicious coordinate value | INFO | Quality |
| DQ_018 | All-caps value in recommended field | MEDIUM | Quality |
| DQ_019 | All-lowercase value in recommended field | MEDIUM | Quality |
| DQ_020 | Recommended field missing or empty | LOW | Quality |
| DQ_021 | Primary key duplicate (duplicate_key) | HIGH | Spec |
| DQ_022 | More than 80% of stops share the same stop_name | HIGH | Quality |

## VAT — Entity Analytics Detection

| Rule | Title | Severity | Class |
|---|---|---|---|
| VAT_001 | Route shape similarity (likely duplicate route) | MEDIUM | Analytics |
| VAT_002 | Transfer hub undefined — many routes pass but no transfers defined | INFO | Analytics |
| VAT_003 | Trip duration statistical outlier | LOW | Analytics |
| VAT_005 | Isolated stop cluster — stops disconnected from main network component | MEDIUM | Analytics |
| VAT_006 | Service density imbalance — single route accounts for large proportion of feed trips | INFO | Analytics |
| VAT_007 | Terminal transfer missing — another route serves the terminus but no transfer defined | INFO | Analytics |
| VAT_008 | Same shape used by more than 30% of routes | INFO | Analytics |

## JPN

| Rule | Title | Severity | Class |
|---|---|---|---|
| JPN_001 | GTFS-JP: stop name missing kana (ja-Hrkt) reading | MEDIUM | Quality |
| JPN_002 | GTFS-JP: jp_office_id not defined in office_jp.txt | HIGH | Interop |
| JPN_003 | GTFS-JP: agency_jp agency_id not defined in agency.txt | HIGH | Interop |
| JPN_004 | GTFS-JP: translations.txt missing | HIGH | Interop |
| JPN_005 | GTFS-JP: office_jp office_name empty | HIGH | Interop |
| JPN_006 | GTFS-JP: fare files missing | MEDIUM | Quality |
| JPN_007 | GTFS-JP: feed_info.txt missing | MEDIUM | Quality |
| JPN_008 | GTFS-JP: route_long_name kana reading missing | MEDIUM | Quality |
| JPN_009 | GTFS-JP: trip_headsign kana reading missing | MEDIUM | Quality |
| JPN_010 | GTFS-JP: agency_name kana reading missing | MEDIUM | Quality |
| JPN_011 | GTFS-JP: agency_id required even with one agency | HIGH | Interop |
