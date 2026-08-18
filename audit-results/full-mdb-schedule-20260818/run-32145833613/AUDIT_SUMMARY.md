# Full MobilityDatabase GTFS Schedule — GTFS Analyzer vs MobilityData audit

## Corpus execution

- Attempted feeds: **4271**
- Successfully downloaded: **4239**
- Both validators completed cleanly: **4229**
- Analyzer median wall time (completed feeds): **0.05 s**
- MobilityData median wall time (completed feeds): **2.98 s**
- Analyzer rules observed: **421**
- MobilityData notice codes observed: **118**

These are automated divergence *candidates*, not correctness verdicts. A count difference can be caused by aggregation, thresholds, scope, or a true validator bug.

## Validator state pairs

| Analyzer | MobilityData | Feeds |
|---|---|---:|
| completed | completed | 4229 |
| not_run | not_run | 32 |
| completed | partial_internal | 6 |
| completed | no_report | 2 |
| completed | timeout | 2 |

## Divergence candidate classes

| Candidate class | Count |
|---|---:|
| analyzer_mapped_md_absent | 14938 |
| adjudicated_divergence | 9644 |
| analyzer_spec_unmapped | 1187 |
| md_mapped_over | 689 |
| md_mapped_under | 366 |
| analyzer_spec_md_absent | 254 |
| md_mapped_missing | 39 |
| context_unresolved | 16 |
| validator_state_asymmetry | 10 |
| md_unmapped | 5 |

## Highest-priority candidates

| Priority | Feed | Direction | MD code | Analyzer rule(s) | Counts |
|---:|---|---|---|---|---|
| 110 | mdb-1186 | validator_state_asymmetry |  |  |  |
| 110 | mdb-1960 | validator_state_asymmetry |  |  |  |
| 110 | mdb-2014 | validator_state_asymmetry |  |  |  |
| 110 | mdb-2607 | validator_state_asymmetry |  |  |  |
| 110 | mdb-2867 | validator_state_asymmetry |  |  |  |
| 110 | mdb-3215 | validator_state_asymmetry |  |  |  |
| 110 | mdb-357 | validator_state_asymmetry |  |  |  |
| 110 | mdb-784 | validator_state_asymmetry |  |  |  |
| 110 | tdg-81618 | validator_state_asymmetry |  |  |  |
| 110 | tdg-83019 | validator_state_asymmetry |  |  |  |
| 105 | mdb-1125 | md_mapped_missing | invalid_url | STP_042 | MD 1912 / A 0 |
| 105 | mdb-1136 | md_mapped_missing | invalid_url | STP_042 | MD 760 / A 0 |
| 105 | mdb-1292 | md_mapped_missing | invalid_url | AGN_003 | MD 1 / A 0 |
| 105 | mdb-1317 | md_mapped_missing | invalid_url | STP_042 | MD 1826 / A 0 |
| 105 | mdb-2366 | md_mapped_missing | stop_time_with_arrival_before_previous_departure_time | STM_008, STM_048 | MD 168 / A 0 |
| 105 | mdb-2386 | md_mapped_missing | invalid_url | AGN_003 | MD 2 / A 0 |
| 105 | mdb-2719 | md_mapped_missing | invalid_url | AGN_003 | MD 1 / A 0 |
| 105 | mdb-2741 | md_mapped_missing | invalid_pickup_drop_off_window | PDW_006 | MD 38 / A 0 |
| 105 | mdb-2905 | md_mapped_missing | invalid_url | AGN_003 | MD 1 / A 0 |
| 105 | mdb-767 | md_mapped_missing | pathway_unreachable_location | PTH_012 | MD 6 / A 0 |
| 105 | mdb-990 | md_mapped_missing | pathway_unreachable_location | PTH_012 | MD 1 / A 0 |
| 105 | ntd-60091 | md_mapped_missing | invalid_url | AGN_003 | MD 1 / A 0 |
| 105 | tdg-80960 | md_mapped_missing | point_near_origin | GEO_016 | MD 5 / A 0 |
| 105 | tdg-82321 | md_mapped_missing | invalid_url | AGN_003 | MD 2 / A 0 |
| 105 | tdg-82326 | md_mapped_missing | invalid_url | AGN_003 | MD 1 / A 0 |
| 105 | tdg-83744 | md_mapped_missing | invalid_url | AGN_003 | MD 1 / A 0 |
| 105 | tdg-83820 | md_mapped_missing | point_near_origin | GEO_016 | MD 2 / A 0 |
| 105 | tdg-83982 | md_mapped_missing | invalid_url | AGN_003 | MD 1 / A 0 |
| 105 | tfs-342 | md_mapped_missing | invalid_color | RTS_006, RTS_007 | MD 2 / A 0 |
| 105 | tfs-342 | md_mapped_missing | invalid_url | STP_042 | MD 1023 / A 0 |
| 105 | tfs-728 | md_mapped_missing | invalid_url | STP_042 | MD 1807 / A 0 |
| 105 | tfs-732 | md_mapped_missing | invalid_url | STP_042 | MD 855 / A 0 |
| 105 | tld-4327 | md_mapped_missing | missing_required_field | FIN_001, FIN_002 | MD 3 / A 0 |
| 105 | tld-7825 | md_mapped_missing | missing_required_field | RTS_031, RTS_004 | MD 8 / A 0 |
| 105 | tld-7876 | md_mapped_missing | missing_required_field | RTS_031, RTS_004 | MD 8 / A 0 |
| 105 | tld-7877 | md_mapped_missing | missing_required_field | RTS_031, RTS_004 | MD 8 / A 0 |
| 105 | tld-7929 | md_mapped_missing | block_trips_with_overlapping_stop_times | TRP_022 | MD 3 / A 0 |
| 105 | tld-829 | md_mapped_missing | invalid_url | STP_042 | MD 745 / A 0 |
| 100 | jbda-nishinomiyacity-guruttonamaze | analyzer_spec_md_absent |  | TRN_001 |  / A 1 |
| 100 | jbda-waketown-wakechoueibasu | analyzer_spec_md_absent |  | TRN_001 |  / A 13 |
| 100 | mdb-1003 | analyzer_spec_md_absent |  | RTS_004 |  / A 1 |
| 100 | mdb-1003 | analyzer_spec_md_absent |  | SHP_002 |  / A 1 |
| 100 | mdb-1003 | analyzer_spec_md_absent |  | SHP_003 |  / A 1 |
| 100 | mdb-1003 | analyzer_spec_md_absent |  | STP_005 |  / A 1 |
| 100 | mdb-1004 | analyzer_spec_md_absent |  | RTS_004 |  / A 1 |
| 100 | mdb-1004 | analyzer_spec_md_absent |  | SHP_002 |  / A 1 |
| 100 | mdb-1004 | analyzer_spec_md_absent |  | SHP_003 |  / A 1 |
| 100 | mdb-1004 | analyzer_spec_md_absent |  | STM_015 |  / A 1 |
| 100 | mdb-1004 | analyzer_spec_md_absent |  | STM_016 |  / A 1 |
| 100 | mdb-1004 | analyzer_spec_md_absent |  | STP_005 |  / A 1 |
| 100 | mdb-1019 | analyzer_spec_md_absent |  | ARC_025 |  / A 5 |
| 100 | mdb-1105 | analyzer_spec_md_absent |  | AGN_011 |  / A 1 |
| 100 | mdb-1185 | analyzer_spec_md_absent |  | STP_006 |  / A 4 |
| 100 | mdb-1185 | analyzer_spec_md_absent |  | STP_007 |  / A 4 |
| 100 | mdb-1187 | analyzer_spec_md_absent |  | AGN_003 |  / A 1 |
| 100 | mdb-1241 | analyzer_spec_md_absent |  | STM_002 |  / A 925 |
| 100 | mdb-1241 | analyzer_spec_md_absent |  | STP_003 |  / A 1 |
| 100 | mdb-1241 | analyzer_spec_md_absent |  | STP_006 |  / A 1 |
| 100 | mdb-1241 | analyzer_spec_md_absent |  | STP_007 |  / A 1 |
| 100 | mdb-1258 | analyzer_spec_md_absent |  | AGN_011 |  / A 1 |
