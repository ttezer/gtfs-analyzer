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

# ── CANONICAL MAP: MD code → our rule(s) ──────────────────────────────────
# Mappings verified in this session (2026-07-02/03). An MD code NOT IN MAP
# automatically falls into the "MD-ONLY" bucket = coverage-gap review queue (deliberately).
# Before adding a mapping: first verify that the rule REALLY measures the same thing.
MAP = {
    # ── Archive / file-column structure ──
    "missing_required_column":  ["ARC_025"],
    "missing_recommended_file": ["ARC_020"],
    "unknown_file":             ["ARC_007"],   # NOTE: GTFS-JP agency_jp/office_jp → we know them, MD does not
    "unknown_column":           ["ARC_017"],   # NOTE: jp_* columns → we know them
    "invalid_row_length":        ["ARC_012"],
    "missing_required_file":     ["ARC_004"],
    "missing_calendar_and_calendar_date_files": ["ARC_008"],
    "invalid_input_files_in_subfolder": ["ARC_024"],
    # ── Routes ──
    "duplicate_route_name":     ["RTS_019"],   # BOTH short AND long are the same. Single-field → RTS_026/027 (US-only, absent in MD)
    "same_route_and_agency_url":["RTS_020"],
    # ── Stops ──
    "same_name_and_description_for_stop": ["STP_031"],
    "stop_without_stop_time":   ["STP_020"],
    # ── Calendar ──
    # CAL_007 is the exact-parity owner. CAL_012 is the near-future user-impact view of the
    # same gap; it must not be attached to the same MD code a second time.
    "big_gap_in_service":       ["CAL_007"],  # PER-SERVICE, threshold big_gap_days=14
    # CAL_013 examines the service's COMBINED active-date set (calendar + calendar_dates);
    # MD evaluates calendar.txt end_date and exception semantics at service level.
    # CAL_009 is only our feed-level calendar.txt summary; it is not the exact-parity owner.
    # CAL_024 asks "not active in the next 7 days"; its scope differs from MD's expired_calendar
    # code and its closest counterpart is TRP_023.
    "expired_calendar":         ["CAL_013"],
    # #123: whitespace-wrapped weekday zeros are parsed for the weekly decision while
    # DQ_016 remains the lexical root; mdb-2830 replay recovered MD's 12 services.
    "service_has_no_active_day_of_the_week": ["CAL_006"],
    "service_extends_far_in_the_future":     ["CAL_023"],  # THRESHOLD DIFFERENCE: ours max_calendar_future_years=3 (year-granular, conservative); MD ~1–2y more aggressive → marks 2028, ours does not. Configurable, by design.
    "service_window_outside_feed_period":    ["CAL_014"],  # 2026-10-11: CAL_019/XFL_011 retired; active-date semantics have one owner
    # ── Shapes ──
    "decreasing_shape_distance":            ["SHP_005"],   # sequence-ordered monotonicity
    "stop_too_far_from_shape":              ["SHP_012"],   # geometrically nearest point
    "stop_too_far_from_shape_using_user_distance": ["SHP_024"],  # FIXED: shape_dist_traveled position (not SHP_012). 20-feed: SHP_024=479 vs MD 721 (~match)
    "stops_match_shape_out_of_order":       ["SHP_017"],
    "unused_shape":                         ["SHP_018"],
    "equal_shape_distance_same_coordinates":["SHP_023"],   # repeated point
    "equal_shape_distance_diff_coordinates":["SHP_028"],   # ABOVE threshold = real error
    "equal_shape_distance_diff_coordinates_distance_below_threshold": ["SHP_029"],  # BELOW threshold = rounding
    "trip_with_shape_dist_traveled_but_no_shape_distances": ["SHP_030"],
    "single_shape_point":          ["SHP_006"],  # near-parity: used shapes only
    # fare_products.txt::amount does not follow the currency's decimal precision. It was unmapped,
    # making 11 feeds look like "we have no rule"; all 11 received FPD_007 in the run (#146).
    # ⚠️ FAR_013 was REMOVED FROM THIS MAPPING on 2026-09-11. MD's code checks ONLY
    # `fare_products.amount` (all samples from 11 feeds in run 16 did so), while `FAR_013` checks
    # `fare_attributes.price` — a field for which MD has no rule.
    # None of the 394 feeds has Fares V2, so MD's rule cannot fire structurally.
    # While the mapping remained, the audit emitted 394 false `..._md_absent` findings every run.
    "invalid_currency_amount":     ["FPD_007"],
    # The following six codes were unmapped and counted as "we have no rule"; each was VERIFIED
    # in the full-catalog run — the candidate was measured to fire on those feeds (#146).
    "missing_required_agency_id":       ["AGN_011"],  # 9/9 feed
    "missing_prior_notice_duration_min": ["BKR_007"],  # 10/10 feed
    "feed_info_lang_and_agency_lang_mismatch": ["AGN_013"],  # 12/12 feed
    "missing_trip_edge":                ["STM_015", "STM_016"],  # first/last stop arrival/departure
    "empty_file":                       ["ARC_009", "ARC_035"],  # required → ARC_035 (2026-08-31), optional → ARC_009
    "invalid_color":                    ["RTS_006", "RTS_007"],  # route_color / route_text_color
    # STP_003/STP_005 measure INVALID values (range checks when parseable);
    # STP_006/STP_007 own EMPTY coordinates. All 7/7 feeds were verified — there was no gap,
    # only a missing mapping.
    "stop_without_location":            ["STP_006", "STP_007"],
    # ── #147 round: 13 of 26 unmapped codes, each VERIFIED in the corpus (#155).
    # Method: select the candidate rule not by name but by whether it REALLY fires on the feeds
    # reported by MD. If it does not fire, DO NOT WRITE THE MAPPING.
    "route_both_short_and_long_name_missing": ["RTS_003"],      # 2/2 feed
    "missing_stop_name":                ["STP_003"],            # 2/2
    "inconsistent_agency_timezone":     ["AGN_005"],            # 2/2
    "invalid_language_code":            ["AGN_006"],            # 1/1
    "fast_travel_between_far_stops":    ["STM_061"],  # >10 km apart, NON-CONSECUTIVE pair — added in #168
    "invalid_prior_notice_duration_min": ["BKR_006"],  # birebir (#165)
    "forbidden_prior_notice_start_day": ["BKR_024"],   # MD: "forbidden when prior_notice_duration_max is set" (#165)
    "unsupported_geometry_type":        ["LOC_001"],   # locations.geojson geometri tipi (#165)
    "fare_transfer_rule_duration_limit_without_type": ["FTR_011"],  # birebir (#165)
    "fare_product_with_multiple_default_rider_categories": ["RCT_006"],  # birebir (#165)
    "forbidden_prior_day_booking_field_value": ["BKR_001"],  # prior-day booking field value forbidden (#165)
    # Corpus run 17 (2026-09-11): two codes remained in the `md_unmapped` bucket.
    # `mdb-2796`/`mdb-2926`: MD `forbidden_pickup_type` 5 ↔ our `STM_051` 5 — EXACT.
    "forbidden_pickup_type":            ["STM_051"],
    # ⚠️ The `forbidden_drop_off_type` mapping raises an OPEN QUESTION and is deliberately left so:
    # MD says 5 on the same two feeds, while `STM_052` says 0. The Spec distinction is: 0 AND 3
    # are forbidden for `pickup_type`, ONLY 0 for `drop_off_type` (`P56388e97` / `P4280b626`).
    # If the row value is 3, MD overclaims; if 0, we have a blind spot. Cannot decide without the feed.
    "forbidden_drop_off_type":          ["STM_052"],
    "transfer_with_invalid_trip_and_route": ["TRF_017", "XFL_020"],  # XFL_020 is the (trip,route) pair for the same phenomenon (#165)
    "invalid_timezone":                 ["AGN_004"],            # 1/1
    "inconsistent_agency_lang":         ["AGN_017", "AGN_013"], # 4/4 both
    "new_line_in_value":                ["ARC_030"],            # 3/3 — tab/CR/LF forbidden
    "translation_unknown_table_name":   ["TRN_001"],            # 2/2
    "translation_foreign_key_violation": ["TRN_016"],  # "field_value matches no record" — MD: entity cannot be found by record_id/record_sub_id (#165)
    "translation_unexpected_value":     ["TRN_013"],  # "must not be populated in this context" — MD: "a field has value but must be empty" (#165)
    "stop_time_with_only_arrival_or_departure_time": ["STM_034"],  # 1/1
    "missing_prior_notice_last_time":   ["BKR_009"],            # 1/1
    "empty_column_name":                ["ARC_017"],            # 5/5 — empty header is an unknown column
    # ⚠️ `PDW_006` was REMOVED FROM THIS MAPPING on 2026-09-12. MD's code measures a MISSING
    # window; `PDW_006` measures OVERLAPPING windows in the same trip+zone — a different phenomenon.
    # The correct counterpart is `STM_039` ("pickup/drop_off window missing in Flex context").
    # The contradiction was inside the script: the lower comment already said "PDW_006 measures
    # OVERLAP, not this", but the upper row still attached it to that code. In run 17 the mapping
    # produced 75 false `analyzer_mapped_md_absent` findings (24% of the bucket).
    "missing_pickup_or_drop_off_window": ["STM_039"],
    "invalid_pickup_drop_off_window":   ["STM_038"],            # reversed/zero window; PDW_006 measures OVERLAP, not this
    # The first candidate round returned "none" because I selected the WRONG candidate (FTR_002 =
    # from_leg_group_id missing, an FK rule). The correct rules measure the leg-group condition:
    # transfer_count is REQUIRED when equal, and FORBIDDEN when different.
    "fare_transfer_rule_without_transfer_count":       ["FTR_009"],  # 4/4
    "fare_transfer_rule_with_forbidden_transfer_count": ["FTR_010"], # 2/2
    "missing_level_id":                 ["LVL_006"],                 # 1/1 — elevator connection context
    # THIRD candidate round (#147). Again, the "none of them fires" result came from the
    # CANDIDATE list: the rules existed, but I searched the wrong group or identity.
    "invalid_email":                    ["FIN_008"],   # 1/1 — feed_contact_email; I had tried FIN_017/AGN_009
    "attribution_without_role":         ["ATR_003"],   # 3/3 — "Attribution role is undefined"
    "invalid_geometry":                 ["LOC_011"],   # 2/2 — ring self-intersects / hole is outside
    "bidirectional_exit_gate":          ["PTH_016"],   # 1/1
    # ── Trips / blocks ──
    "trip_headsign_matches_intermediate_stop": ["TRP_020"],  # AGG: per-trip (MD per-occurrence)
    "block_trips_with_overlapping_stop_times": ["TRP_022"],  # MD ERROR; approximately matches ours (same-day block)
    # ── Stop times ──
    "missing_timepoint_value":  ["STM_050"],   # AGG: feed summary (MD per-row)
    "decreasing_or_equal_stop_time_distance": ["STM_056"],  # MD ERROR ↔ stop_times.shape_dist_traveled monotonicity
    # FIXED 2026-07-17: our TWO rules split MD's ONE code at 700 km/h
    # → STM_012 (>700, "physically impossible", emits and `continue`s over STM_014)
    # + STM_014 (threshold..700). Mapping only to STM_014 made 963 km/h segments appear missed.
    # Evidence: mdb-2015 ours STM_012=78 ↔ MD 78 (exact), mdb-3108 8 ↔ 8.
    "fast_travel_between_consecutive_stops": ["STM_014", "STM_012"],
    # ── Quality / feed_info ──
    "missing_recommended_field":["RTS_025", "FIN_007", "FIN_013", "FIN_018"],  # Generic code; routes/fare/feed_info field resolved by sample context.
    "mixed_case_recommended_field": ["DQ_018"],  # we check ALL-CAPS only (deliberate)
    "missing_feed_contact_email_and_url": ["FIN_018"],
    # ── test2 (second corpus) mappings ──
    "duplicate_key":                  ["SHP_008"],   # shapes PK (shape_id,seq). BY-DESIGN agg: K2 emits 6159 but pipeline deduplicates by (rule,shape_id) → 6 distinct shapes (mdb-1935). MD per-row (6159). Not a bug.
    "foreign_key_violation":          ["STM_002"],   # stop_times.stop_id→stops. BY-DESIGN agg: STM_002 distinct stop_id (4), MD per-occurrence (2886).
    "same_name_and_description_for_route": ["RTS_023"],
    # ── mappings added from the 2026-07-03 20-feed scan ──
    "non_ascii_or_non_printable_char": ["ARC_021", "ARC_037"],   # ARC_021 non-printable, ARC_037 non-ASCII in ID (file summary)
    "unsorted_stop_times":            ["STM_036"],
    "leading_or_trailing_whitespaces":["DQ_016"],
    "missing_bike_allowance":         ["TRP_021"],
    "route_short_name_too_long":      ["RTS_010"],     # RTS_010=MD exact; RTS_021 (Google 6-char) is stricter and US-ONLY
    "route_long_name_contains_short_name": ["RTS_022"],
    "trip_coverage_not_active_for_next7_days": ["TRP_023"],
    "future_calendar":                ["CAL_017"],
    "future_feed":                    ["CAL_017"],
    # MD counts BOTH states under one code: already expired and expiring soon. We have separate
    # rules (FIN_010 expired · FIN_019 expiring within the window), so the mapping goes to both.
    # Mapping only to FIN_019 created a false MISS for 841 feeds: in the full-catalog run 837
    # of 841 receive FIN_010, 0 receive FIN_019 — and 839 of MD's own 841 samples are ALREADY EXPIRED (#146).
    "feed_expiration_date7_days":     ["FIN_010", "FIN_019"],
    "same_stop_and_agency_url":       ["STP_034"],
    "same_stop_and_route_url":        ["STP_035"],
    "platform_without_parent_station":["STP_032", "STP_045"],  # STP_045 platform_code+parent absent (feed summary); STP_032 pathway platform
    # ── 20-feed second-round mappings (our rule EXISTS, MAP was missing) ──
    "route_color_contrast":           ["RTS_008"],     # over: ours 132 vs MD 3 (contrast-threshold difference)
    # FIXED 2026-07-17: our TWO rules split MD's one code by stop COUNT:
    # 0 stops → XFL_002 ("trip has no stop_times record"), 1 stop → STM_033 ("single-stop").
    # Mapping only to STM_033 made zero-stop trips appear missed. Evidence:
    # mdb-2015/2036 XFL_002=13 ↔ MD 13, mdb-2381 XFL_002=35 ↔ MD 35 — all EXACT
    # (13/13 MD samples were verified to have 0 stop_times rows).
    "unusable_trip":                  ["STM_033", "XFL_002"],
    "unused_trip":                    ["XFL_002"],
    # FIXED 2026-07-17: GENERIC code → we have a SEPARATE rule per field.
    # 250-feed evidence: mdb-3230 direction_id=255 → TRP_005=26 ↔ MD 26 EXACT;
    # mdb-2086 transfers=5 → FAR_005=6 ↔ MD 6 EXACT. Mapping only to RTS_004 made
    # these two appear missed.
    "unexpected_enum_value":          ["RTS_004", "RTS_030", "TRP_005", "TRP_006", "TRP_007", "TRP_032", "FAR_005", "TRF_004", "RCT_003", "RTS_013", "RTS_018"],
    "unused_station":                 ["STP_030"],     # station without child ≈ unused_station (~19)
    "trip_distance_exceeds_shape_distance": ["SHP_025"],  # EXISTS but by design: SHP_025 threshold >0.1% (tolerates rounding, SHP_029 philosophy). mdb-1909: 165 of 170 exceedances are <0.1% rounding → ours 5 correct, MD emits all 170.
    "trip_distance_exceeds_shape_distance_below_threshold": ["SHP_025"],
    # ── 2026-07-05 DELFI (mdb-3215, Germany national) scan: MAP gaps (our rule EXISTS) ──
    "number_out_of_range":            ["PTH_007", "STP_003", "STP_005", "SHP_002", "SHP_003", "RCT_004", "FRQ_008"],  # Generic code; candidate set only without field context.

    "inconsistent_route_type_for_block_id": ["TRP_024"],  # "inconsistent route type within block"
    "transfer_distance_above_2_km":   ["TRF_011"],   # "transfer defined but distance is large" (MD INFO variant)
    "transfer_distance_too_large":    ["TRF_011"],   # same rule (MD WARNING variant)
    "pathway_loop":                   ["PTH_011"],   # from_stop_id==to_stop_id (k2/pathways.rs:91)
    "location_with_unexpected_stop_time": ["STP_012"],  # stop_times stop with location_type≠0 (k4_cross_ref.rs:536)
    "pathway_unreachable_location":   ["PTH_012"],
    "stop_without_zone_id":            ["STP_033"],
    # point_near_origin moved to contextual mapping (stops → GEO_016, shapes → GEO_017) — md_parity_mapping.CONTEXT_MAPPINGS
    "point_near_pole":                  ["GEO_022"],   # 2/2 — |lat|>89; GEO_022 fires on the same two feeds
    "pathway_dangling_generic_node":    ["PTH_019"],   # rule EXISTED, mapping was missing (#158)
    "transfer_with_invalid_trip_and_stop":  ["XFL_021"],  # rule EXISTED (XFL group), ledger incorrectly said "genuine-gap"
    "invalid_character":                ["ARC_021"],
    "missing_feed_info_date":           ["FIN_014"],
    "more_than_one_entity":             ["FIN_015"],
    "overlapping_frequency":            ["FRQ_011"],
    # MD counts two phenomena under one code. A trip crossing midnight must continue with `24:xx`;
    # if the feed writes `00:xx`, MD sees "arrival earlier than previous". Our RAW-WRITING violation
    # is `STM_048` (K2, Spec), while `STM_008` measures only REAL reversals unexplained by normalization
    # — the distinction is documented on the STM_008 card. Mapping only to STM_008 created false
    # MISS for 6 feeds; 5 of 6 received STM_048 in the run (#155).
    "stop_time_with_arrival_before_previous_departure_time": ["STM_008", "STM_048"],
    "stop_time_timepoint_without_times": ["STM_047"],
    "missing_pickup_drop_off_booking_rule_id": ["STM_059"],
    "invalid_date":                     ["FIN_005", "FIN_006", "CAL_003", "CAL_004", "CLD_002"],
    "invalid_integer":                  ["TRP_005"],
    "invalid_url":                      ["AGN_003", "RTS_005", "FIN_002"],
    "missing_required_field":           ["AGN_003", "FIN_002"],
    "start_and_end_range_out_of_order": ["CAL_005", "FIN_012", "STM_007"],
    "unknown_file":                   ["ARC_007"],   # 2026-07-05 FIX: ARC_007 now also flags non-.txt root files (.pdf/.out)
    # ── COVERAGE GAP: deliberately NOT IN MAP → let it appear in MD-ONLY ──
    # `fast_travel_between_far_stops` (250-feed: 32 feeds) was REMOVED on 2026-07-17.
    # It used to map to STM_012, which was WRONG: STM_012 is an absolute ">700 km/h" check
    # over CONSECUTIVE pairs. MD compares geographically separate, NON-CONSECUTIVE stop pairs
    # >10 km apart — evidence: mdb-1229 sample, stopSequence 1 → 19 (skips 18 stops), 10 km / 600 km/h.
    # We have NO non-consecutive gap check → a REAL coverage gap.
    # MD's threshold is the SAME per route_type as consecutive (bus 150); only the 10 km distance
    # separates them. If adding a rule: consider a new STM_* (non-consecutive gap).
    #
    # ── ACTUALLY remaining minor item (low value, deferrable) ──
    # decreasing_or_equal_stop_time_distance (2): 1–2 cases, low priority
}

# ── AGGREGATED rules: we report per-entity/feed summaries, MD per occurrence.
# our_count << md_count is NORMAL for these → do not call it UNDER; label it "AGG".
AGG_RULES = {
    "STM_050", "STM_017", "TRN_007", "STP_022", "RTS_017",  # feed summary (changelog 0.1.3)
    "TRP_020",                                              # per-trip
    "STP_045",                                              # feed summary; MD per stop
    "ARC_037",                                              # file summary; MD per value
    "SHP_023", "SHP_028", "SHP_029",                        # per-shape (per type)
    "CAL_007", "CAL_009", "CAL_012", "CAL_013",             # per-service/feed calendar aggregates; CAL_012 MD-only detail
    "STM_014",  # 2026-07-16: (route, direction, segment) aggregation (53ef3fb). MD counts per occurrence
                # → our_count << md_count is NOW NORMAL. If omitted, every run produces
                # a false UNDER (as happened on 20 feeds in the 250-feed run).
    "RTS_019",  # one notice per (short,long,type,agency) GROUP, listing overlapping routes;
                # MD counts PAIRS (routeId1/routeId2) → a 3-route group = MD C(3,2)=3.
                # mdb-867: ours 1 ↔ MD 3. Granularity, not divergence.
    "DQ_016",   # 2026-07-17: one FILE summary (c95189e). MD counts per field value
                # → our_count << md_count is NOW NORMAL. (Repeat of our own lesson: when a rule
                # becomes aggregate, also add it to AGG_RULES — it was forgotten for STM_014.)
    "STM_018", "STM_019", "STM_032", "STM_007", "STM_056",  # 2026-08-18: one feed summary for >50 stop_times rows (#151);
                            # MD counts per row → our_count << md_count is NORMAL.
    "STM_061",  # 2026-08-20: (route, direction, far-stop pair) aggregation. One finding per trip
                # was NOT ENOUGH — re-reporting the same physical pair per trip scaled the count
                # with schedule frequency: all 9,533 findings in mdb-1003 came from ONE pair (then 11),
                # and 10,390 -> 20 in mdb-1004.
                # MD counts per occurrence → our_count << md_count is NOW NORMAL.
    "TRP_024",  # 2026-08-20: one notice per BLOCK. The rule counted every overlapping trip and
                # the count scaled with block size (tdg-83634: 5,798 -> 1,887).
    "FIN_018", "FIN_007", "FIN_014",  # 2026-08-20: one finding per FEED (feed_info.txt has one
                # row). The count itself is evidence: 1,491 common feeds -> 1,491 findings,
                # 288 -> 288, 264 -> 264. MD counts per missing FIELD (55x / 12x / 7x).
                # Same omission as SHP_024; the rule was aggregate, but the list did not know.
    # 2026-09-01 (corpus run 14): some of the 68 `md_mapped_under` items were a TOOL DEFECT,
    # not product blindness. All eight rules below are already deduplicated at Feed or File level
    # in the registry — aggregate by design — but the list did not know, so every run produced
    # false UNDER. This was the FOURTH repeat of the same omission after STM_014 · DQ_016 · SHP_024.
    # 2026-09-12 (corpus run 17): the FIFTH, SIXTH, and SEVENTH repeat of the SAME omission.
    # All three are already deduplicated at Entity level in the registry — aggregate by design —
    # but the list did not know, so every run produced false `md_mapped_under`.
    "GEO_017",  # [Entity=shape_id] shape POINT near Null Island — one notice per SHAPE.
                # MD counts per point: tdg-81559/tdg-82291 MD 67 ↔ ours 19 (3.53×).
                # Its sibling `GEO_016` [Entity=stop_id] counts in the SAME unit as MD and matches —
                # the 1.80× mixed pair in `mdb-3108` comes only from the GEO_017 branch.
    "STM_059",  # [Entity=trip_id] booking_rule_id missing when pickup/drop_off=2 — one per TRIP.
                # MD counts per stop_times ROW: mdb-2884 MD 1,504 ↔ ours 140 (10.74×),
                # mdb-2447 MD 598 ↔ ours 79 (7.57×).
    "STM_038",  # [Entity=trip_id] start > end pickup/drop-off window — one per TRIP.
                # MD counts per row: mdb-2741 MD 38 ↔ ours 23.
    "SHP_002", "SHP_003",  # [Entity=shape] invalid shape_pt_lat/lon — one notice per SHAPE.
                # MD counts per POINT. Measured with tfs-70 (comma decimal separator): shapes.txt
                # has 36,491 points but 397 shapes → MD 72,982 (lat+lon), ours 794. The same feed's
                # stops side matches EXACTLY (STP_004 7,836 + STP_005 7,843 ↔ MD remainder),
                # so the difference is granularity, not scope. Emitting per point would mean 36,491 notices
                # — the same explosion as STM_050/DQ_016.
    "ARC_021",  # [File] non-printable/problematic character — one FILE summary. MD counts per field value:
                # mdb-1926 MD 777 ↔ ours 1, and we show the SAME row (45556) and character (U+FFFD).
                # Detection is exact, granularity differs.
    "ARC_030",  # [File] value containing tab/line break — one per file (mdb-501: MD 29 ↔ 2).
    "FAR_013",  # [File] price ISO 4217 decimal places — one per file (mdb-114: MD 31 ↔ 1).
    "FPD_007",  # [File] amount ISO 4217 decimal places — fare_products counterpart of FAR_013,
                # same ratio on the same feeds (mdb-114: MD 31 ↔ 1).
    "LOC_011",  # [File] invalid polygon (ring self-intersects / hole is outside) — one per file
                # (mdb-2164: MD 9 ↔ 1).
    "AGN_011",  # [Feed] agency_id missing in multiple agencies — one per FEED; MD counts every agency
                # row carrying the missing field (mdb-1134: MD 15 ↔ 1).
    "AGN_013",  # [Feed] feed language ↔ agency language mismatch — one per feed (mdb-2826: 25 ↔ 2).
    "AGN_017",  # [Feed] agency_lang inconsistency across agencies — same feeds as AGN_013,
                # same ratio; both are feed-level provisions (mdb-2832: 21 ↔ 2).
    "SHP_024",  # 2026-08-20: one notice per (stop_id, shape_id) PAIR (`seen_shp024`).
                # MD counts per (trip, stop) → if the same shape is used by N trips, MD gives N
                # times more; across 581 common feeds ours 122,401, theirs 486,637 (3.98x). The rule
                # DID deduplicate, but was not in AGG_RULES — the THIRD repeat after STM_014 and DQ_016.
    "SHP_012",  # per-SHAPE summary: ONE notice per shape, observed="N stops >100m".
                # MD counts per (trip,stop) → if the same shape is used by N trips, MD gives N times more.
                # mdb-2366: 3,338 far stops in our 171 notices, MD 1,794 occurrences
                # → different granularity, count comparison is meaningless.
    "FRL_001", "FRL_003", "FRL_004", "FRL_005",
                # 2026-07-17: one notice per DISTINCT ID (7727456), row count in the message.
                # MD counts foreign_key_violation per fare_rules ROW → in mdb-2837
                # 1,128,840 rows / 486 distinct fare_id ⇒ ours 486, MD 1,128,840. AGG, not UNDER.
                # (STM_002 precedent: it already summarizes the same FK finding by distinct stop_id.)
    "TRP_021",  # feed summary: k2/trips.rs COUNTS bikes_allowed-empty trips, one notice +
                # emits the first 5 sample trip_id values (card: "one feed-wide summary").
                # MD counts per trip (mdb-2933: 118) → our 1 is AGG, not UNDER.
    # 2026-09-21: direct MD mappings for Gate A aggregations. These rules carry feed/file/entity
    # summaries after orchestration, so the low ratio caused by MD's per-row count is not real UNDER.
    "STM_036", "ARC_012", "SHP_005", "STM_008", "STM_047",
    "TRN_001", "PTH_012", "STP_032", "TRP_005",
    # Entries coming through context mapping: when resolution selects a specific Analyzer rule,
    # that rule's own aggregation scope is also taken into account.
    "DQ_021", "TRP_003",
    "CLD_003", "FRQ_007", "PTH_007", "STM_022", "STP_004", "STP_005",
    "STP_042", "TRF_005",
}

# Aggregation labels are explanatory, not a scoring override.
# They describe why count ratios differ between validators.
# A mapped rule still needs a context-aware resolution.
# Unresolved context remains visible to parity consumers.
# Exact matches remain MATCH even when a rule is aggregated.
# This distinction prevents denominator errors from becoming findings.
# Keep these annotations aligned with the rule cards.


# ── GENERIC MD CODE TRAP (permanent lesson) ────────────────────────────────────
# MD's GENERIC codes (unexpected_enum_value, number_out_of_range,
# missing_recommended_field) correspond to a separate rule PER FIELD for us → mapping one-to-one
# to a single rule produces a false MISS. BY_DESIGN is code-based and CANNOT distinguish fields;
# therefore complete MAP first (all field rules), THEN explain the remaining difference.
# unexpected_enum_value was resolved this way (2026-07-17): MAP expanded to 4 rules
# → direction_id/transfers matched EXACTLY; the remaining route_type difference moved to BY_DESIGN.
# Before deciding, inspect the `fieldName` distribution in samples — the count alone misleads.

OVER_RATIO = 3.0
UNDER_RATIO = 0.34


def load_json(path):
    try:
        with open(path, encoding="utf-8") as f:
            return json.load(f)
    except Exception:
        return None


def classify(d):
    """Classify by content: 'md' | 'golden' | None."""
    if not isinstance(d, dict):
        return None
    if "validation" in d and isinstance(d["validation"], dict):
        return "golden"
    notices = d.get("notices")
    if isinstance(notices, list) and notices and isinstance(notices[0], dict) and "code" in notices[0]:
        return "md"
    # An empty-notice MD report is possible too.
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
        print("usage: python3 md_parity_audit.py <BASE_DIR>")
        sys.exit(1)
    base = sys.argv[1]
    folders = [f for f in sorted(glob.glob(os.path.join(base, "*"))) if os.path.isdir(f)]

    mapped_ours = set(x for lst in MAP.values() for x in lst)
    rows = []              # B: (feed, code) rows
    md_only_rows = []      # C1
    agg_md = defaultdict(int)   # D aggregate
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
            # Do not COUNT an adjudicated divergence as a divergence. Do not overwrite MATCH:
            # if it already matches, calling it "explained" would lose information.
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

        # US-ONLY: ours has ≥1, no MD counterpart in MAP, count is large
        for r, c in rc.items():
            if r not in mapped_ours and c > 0:
                pass  # US-ONLY can be reported separately; outside parity_all for now

    # ── write ──
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

    # ── PRIMARY OUTPUT: unexplained delta ──
    # parity_all keeps everything (auditability), but humans read this:
    # MATCH noise and adjudicated divergences removed.
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
        f.write("## 🎯 Unexplained delta (review queue)\n\n")
        f.write(f"- Total rows: **{len(rows)}** · MATCH: **{sum(1 for r in rows if r[10]=='MATCH')}** "
                f"· AGG: **{sum(1 for r in rows if r[10]=='AGG')}** "
                f"· CONTEXT: **{sum(1 for r in rows if r[10]=='CONTEXT')}** "
                f"· adjudicated (EXPLAINED): **{explained}**\n")
        f.write(f"- **UNEXPLAINED: {len(unexplained)}** → `parity_unexplained.csv`\n\n")
        f.write("EXPLAINED = a previously adjudicated divergence with a reason in `BY_DESIGN`.\n"
                "The list below is a review queue, not evidence.\n\n")
        f.write("| status | md_code | rows |\n|---|---|--:|\n")
        for (st, code), n in by_code.most_common(25):
            f.write(f"| {st} | {code} | {n} |\n")
        f.write("\n## Feed durumu\n\n")
        f.write("| feed | durum | skor | | \n|---|---|---|---|\n")
        for m in feed_meta:
            f.write(f"| {m[0]} | {m[1]} | {m[2]} | {m[3]} |\n")
        f.write("\n## Code aggregate (by divergence magnitude)\n\n")
        f.write("| md_code | Σmd | Σour | feed# | diverging# | mapping |\n|---|--:|--:|--:|--:|---|\n")
        allcodes = set(agg_md) | set(agg_our)
        for code in sorted(allcodes, key=lambda c: -(agg_md[c])):
            ours = MAP.get(code)
            tag = "|".join(ours) if ours else "**MD-ONLY**"
            f.write(f"| {code} | {agg_md[code]} | {agg_our.get(code,'—')} | "
                    f"{agg_feeds[code]} | {agg_diverge[code]} | {tag} |\n")
    print(f"wrote: parity_all.csv ({len(rows)}) · parity_unexplained.csv ({len(unexplained)}) · "
          f"parity_md_only.csv ({len(md_only_rows)}) · parity_summary.md")
    print(f"  MATCH={sum(1 for r in rows if r[10]=='MATCH')} AGG={sum(1 for r in rows if r[10]=='AGG')} "
          f"EXPLAINED={explained} → UNEXPLAINED={len(unexplained)}")


if __name__ == "__main__":
    main()
