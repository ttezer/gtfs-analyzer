# Full MobilityDatabase GTFS Schedule — GTFS Analyzer vs MobilityData audit

## Corpus execution

- Attempted feeds: **4271**
- Successfully downloaded: **4232**
- Both validators completed cleanly: **4222**
- Analyzer median wall time (completed feeds): **0.05 s**
- MobilityData median wall time (completed feeds): **2.99 s**
- Analyzer rules observed: **422**
- MobilityData notice codes observed: **118**

These are automated divergence *candidates*, not correctness verdicts. A count difference can be caused by aggregation, thresholds, scope, or a true validator bug.

## Validator state pairs

| Analyzer | MobilityData | Feeds |
|---|---|---:|
| completed | completed | 4222 |
| not_run | not_run | 39 |
| completed | partial_internal | 5 |
| completed | no_report | 2 |
| completed | timeout | 2 |
| timeout | timeout | 1 |

## Divergence candidate classes

| Candidate class | Count |
|---|---:|
| analyzer_mapped_md_absent | 14980 |
| adjudicated_divergence | 10210 |
| analyzer_spec_md_absent | 718 |
| analyzer_spec_unmapped | 690 |
| md_mapped_under | 179 |
| md_mapped_missing | 133 |
| md_mapped_over | 78 |
| validator_state_asymmetry | 9 |
| context_unresolved | 6 |

## Highest-priority candidates

| Priority | Feed | Direction | MD code | Analyzer rule(s) | Counts |
|---:|---|---|---|---|---|
| 110 | mdb-1186 | validator_state_asymmetry |  |  |  |
| 110 | mdb-1960 | validator_state_asymmetry |  |  |  |
| 110 | mdb-2607 | validator_state_asymmetry |  |  |  |
| 110 | mdb-2867 | validator_state_asymmetry |  |  |  |
| 110 | mdb-3215 | validator_state_asymmetry |  |  |  |
| 110 | mdb-357 | validator_state_asymmetry |  |  |  |
| 110 | mdb-784 | validator_state_asymmetry |  |  |  |
| 110 | tdg-81618 | validator_state_asymmetry |  |  |  |
| 110 | tdg-83019 | validator_state_asymmetry |  |  |  |
| 105 | mdb-1003 | md_mapped_missing | invalid_date | CAL_003, CAL_004 | MD 5 / A 0 |
| 105 | mdb-1003 | md_mapped_missing | invalid_integer | CAL_002 | MD 11 / A 0 |
| 105 | mdb-1003 | md_mapped_missing | invalid_timezone | AGN_004 | MD 1 / A 0 |
| 105 | mdb-1004 | md_mapped_missing | invalid_date | CAL_003, CAL_004 | MD 5 / A 0 |
| 105 | mdb-1004 | md_mapped_missing | invalid_integer | CAL_002 | MD 15 / A 0 |
| 105 | mdb-1004 | md_mapped_missing | invalid_timezone | AGN_004 | MD 1 / A 0 |
| 105 | ntd-60089 | md_mapped_missing | empty_file | STP_018 | MD 7 / A 0 |
| 105 | tld-4456 | md_mapped_missing | empty_file | STP_018 | MD 7 / A 0 |
| 105 | tld-4458 | md_mapped_missing | empty_file | STP_018 | MD 7 / A 0 |
| 105 | tld-4461 | md_mapped_missing | empty_file | STP_018 | MD 7 / A 0 |
| 105 | tld-4466 | md_mapped_missing | empty_file | STP_018 | MD 7 / A 0 |
| 105 | tld-478 | md_mapped_missing | empty_file | STP_018 | MD 7 / A 0 |
| 100 | jbda-nishinomiyacity-guruttonamaze | analyzer_spec_md_absent |  | TRN_001 |  / A 1 |
| 100 | jbda-waketown-wakechoueibasu | analyzer_spec_md_absent |  | TRN_001 |  / A 13 |
| 100 | mdb-1003 | analyzer_spec_md_absent |  | SHP_002 |  / A 1 |
| 100 | mdb-1003 | analyzer_spec_md_absent |  | SHP_003 |  / A 1 |
| 100 | mdb-1004 | analyzer_spec_md_absent |  | SHP_002 |  / A 1 |
| 100 | mdb-1004 | analyzer_spec_md_absent |  | SHP_003 |  / A 1 |
| 100 | mdb-1004 | analyzer_spec_md_absent |  | STM_002 |  / A 1 |
| 100 | mdb-1004 | analyzer_spec_md_absent |  | STM_015 |  / A 1 |
| 100 | mdb-1004 | analyzer_spec_md_absent |  | STM_016 |  / A 1 |
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
| 100 | mdb-1258 | analyzer_spec_md_absent |  | RTS_004 |  / A 2 |
| 100 | mdb-1271 | analyzer_spec_md_absent |  | STP_005 |  / A 2 |
| 100 | mdb-1271 | analyzer_spec_md_absent |  | STP_006 |  / A 5 |
| 100 | mdb-1271 | analyzer_spec_md_absent |  | STP_007 |  / A 5 |
| 100 | mdb-1299 | analyzer_spec_md_absent |  | STP_006 |  / A 2 |
| 100 | mdb-1299 | analyzer_spec_md_absent |  | STP_007 |  / A 2 |
| 100 | mdb-1808 | analyzer_spec_md_absent |  | RTS_003 |  / A 15 |
| 100 | mdb-1808 | analyzer_spec_md_absent |  | RTS_004 |  / A 30 |
| 100 | mdb-1814 | analyzer_spec_md_absent |  | AGN_003 |  / A 1 |
| 100 | mdb-1861 | analyzer_spec_md_absent |  | AGN_003 |  / A 1 |
| 100 | mdb-1862 | analyzer_spec_md_absent |  | AGN_003 |  / A 1 |
| 100 | mdb-1891 | analyzer_spec_md_absent |  | AGN_004 |  / A 1 |
| 100 | mdb-1899 | analyzer_spec_md_absent |  | AGN_004 |  / A 1 |
| 100 | mdb-1961 | analyzer_spec_md_absent |  | AGN_004 |  / A 3 |
| 100 | mdb-2000 | analyzer_spec_md_absent |  | STP_006 |  / A 2 |
| 100 | mdb-2000 | analyzer_spec_md_absent |  | STP_007 |  / A 2 |
| 100 | mdb-2026 | analyzer_spec_md_absent |  | AGN_003 |  / A 1 |
| 100 | mdb-2119 | analyzer_spec_md_absent |  | AGN_003 |  / A 2 |
| 100 | mdb-2119 | analyzer_spec_md_absent |  | AGN_004 |  / A 2 |
| 100 | mdb-2147 | analyzer_spec_md_absent |  | AGN_003 |  / A 1 |
