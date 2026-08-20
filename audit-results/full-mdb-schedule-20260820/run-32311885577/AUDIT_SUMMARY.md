# Full MobilityDatabase GTFS Schedule — GTFS Analyzer vs MobilityData audit

## Corpus execution

- Attempted feeds: **4271**
- Successfully downloaded: **4239**
- Both validators completed cleanly: **4229**
- Analyzer median wall time (completed feeds): **0.05 s**
- MobilityData median wall time (completed feeds): **3.0 s**
- Analyzer rules observed: **422**
- MobilityData notice codes observed: **118**

These are automated divergence *candidates*, not correctness verdicts. A count difference can be caused by aggregation, thresholds, scope, or a true validator bug.

## Validator state pairs

| Analyzer | MobilityData | Feeds |
|---|---|---:|
| completed | completed | 4229 |
| not_run | not_run | 32 |
| completed | partial_internal | 5 |
| completed | no_report | 2 |
| completed | timeout | 2 |
| timeout | timeout | 1 |

## Divergence candidate classes

| Candidate class | Count |
|---|---:|
| adjudicated_divergence | 23353 |
| analyzer_mapped_md_absent | 2196 |
| analyzer_spec_md_absent | 565 |
| md_mapped_under | 72 |
| md_mapped_over | 24 |
| md_mapped_missing | 12 |
| validator_state_asymmetry | 9 |
| analyzer_spec_unmapped | 8 |
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
| 100 | mdb-100 | analyzer_spec_md_absent |  | STP_018 |  / A 1 |
| 100 | mdb-1003 | analyzer_spec_md_absent |  | CAL_001 |  / A 4 |
| 100 | mdb-1004 | analyzer_spec_md_absent |  | CAL_001 |  / A 4 |
| 100 | mdb-1004 | analyzer_spec_md_absent |  | TRP_002 |  / A 1 |
| 100 | mdb-1037 | analyzer_spec_md_absent |  | STP_018 |  / A 1 |
| 100 | mdb-1067 | analyzer_spec_md_absent |  | TRP_003 |  / A 207 |
| 100 | mdb-1105 | analyzer_spec_md_absent |  | FIN_003 |  / A 1 |
| 100 | mdb-1134 | analyzer_spec_md_absent |  | ARC_031 |  / A 1 |
| 100 | mdb-1161 | analyzer_spec_md_absent |  | ARC_031 |  / A 1 |
| 100 | mdb-1260 | analyzer_spec_md_absent |  | STP_018 |  / A 1 |
| 100 | mdb-1808 | analyzer_spec_md_absent |  | RTS_002 |  / A 15 |
| 100 | mdb-1812 | analyzer_spec_md_absent |  | FAR_003 |  / A 103 |
| 100 | mdb-1818 | analyzer_spec_md_absent |  | STP_018 |  / A 1 |
| 100 | mdb-1831 | analyzer_spec_md_absent |  | AGN_010 |  / A 2 |
| 100 | mdb-1891 | analyzer_spec_md_absent |  | STP_018 |  / A 1 |
| 100 | mdb-1899 | analyzer_spec_md_absent |  | STP_018 |  / A 1 |
| 100 | mdb-1961 | analyzer_spec_md_absent |  | FRL_002 |  / A 9 |
| 100 | mdb-2003 | analyzer_spec_md_absent |  | STP_009 |  / A 2 |
| 100 | mdb-2018 | analyzer_spec_md_absent |  | STP_018 |  / A 1 |
| 100 | mdb-2137 | analyzer_spec_md_absent |  | TRP_003 |  / A 5052 |
| 100 | mdb-2187 | analyzer_spec_md_absent |  | STP_018 |  / A 1 |
| 100 | mdb-2237 | analyzer_spec_md_absent |  | STP_018 |  / A 1 |
| 100 | mdb-2437 | analyzer_spec_md_absent |  | STP_018 |  / A 1 |
| 100 | mdb-2519 | analyzer_spec_md_absent |  | ARC_031 |  / A 1 |
| 100 | mdb-26 | analyzer_spec_md_absent |  | FTR_001 |  / A 6 |
| 100 | mdb-2653 | analyzer_spec_md_absent |  | TRF_005 |  / A 21 |
| 100 | mdb-2653 | analyzer_spec_md_absent |  | TRF_008 |  / A 15 |
| 100 | mdb-2653 | analyzer_spec_md_absent |  | TRF_009 |  / A 15 |
| 100 | mdb-2838 | analyzer_spec_md_absent |  | FRL_001 |  / A 90 |
| 100 | mdb-2838 | analyzer_spec_md_absent |  | FRL_005 |  / A 2 |
| 100 | mdb-2838 | analyzer_spec_md_absent |  | STP_009 |  / A 9 |
| 100 | mdb-3105 | analyzer_spec_md_absent |  | STP_018 |  / A 1 |
| 100 | mdb-3360 | analyzer_spec_md_absent |  | RTS_002 |  / A 3 |
| 100 | mdb-3360 | analyzer_spec_md_absent |  | TRP_002 |  / A 9 |
| 100 | mdb-3360 | analyzer_spec_md_absent |  | TRP_003 |  / A 9 |
| 100 | mdb-392 | analyzer_spec_md_absent |  | STP_018 |  / A 1 |
| 100 | mdb-817 | analyzer_spec_md_absent |  | FLG_003 |  / A 50 |
| 100 | mdb-817 | analyzer_spec_md_absent |  | FLG_004 |  / A 50 |
| 100 | mdb-836 | analyzer_spec_md_absent |  | STP_018 |  / A 1 |
