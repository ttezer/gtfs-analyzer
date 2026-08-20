# Full MobilityDatabase GTFS Schedule — GTFS Analyzer vs MobilityData audit

## Corpus execution

- Attempted feeds: **4271**
- Successfully downloaded: **4239**
- Both validators completed cleanly: **4229**
- Analyzer median wall time (completed feeds): **0.05 s**
- MobilityData median wall time (completed feeds): **3.02 s**
- Analyzer rules observed: **417**
- MobilityData notice codes observed: **115**

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
| adjudicated_divergence | 26645 |
| analyzer_spec_md_absent | 509 |
| analyzer_mapped_md_absent | 270 |
| md_mapped_under | 69 |
| md_mapped_over | 21 |
| md_mapped_missing | 15 |
| validator_state_asymmetry | 9 |
| context_unresolved | 6 |
| analyzer_spec_unmapped | 2 |

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
| 105 | mdb-1003 | md_mapped_missing | invalid_float | SHP_002, SHP_003 | MD 4 / A 0 |
| 105 | mdb-1003 | md_mapped_missing | invalid_integer | CAL_002 | MD 11 / A 0 |
| 105 | mdb-1003 | md_mapped_missing | invalid_timezone | AGN_004 | MD 1 / A 0 |
| 105 | mdb-1004 | md_mapped_missing | invalid_date | CAL_003, CAL_004 | MD 5 / A 0 |
| 105 | mdb-1004 | md_mapped_missing | invalid_float | SHP_002, SHP_003 | MD 4 / A 0 |
| 105 | mdb-1004 | md_mapped_missing | invalid_integer | CAL_002 | MD 15 / A 0 |
| 105 | mdb-1004 | md_mapped_missing | invalid_time | STM_003, STM_004 | MD 2 / A 0 |
| 105 | mdb-1004 | md_mapped_missing | invalid_timezone | AGN_004 | MD 1 / A 0 |
| 105 | ntd-60089 | md_mapped_missing | empty_file | STP_018 | MD 7 / A 0 |
| 105 | tld-4456 | md_mapped_missing | empty_file | STP_018 | MD 7 / A 0 |
| 105 | tld-4458 | md_mapped_missing | empty_file | STP_018 | MD 7 / A 0 |
| 105 | tld-4461 | md_mapped_missing | empty_file | STP_018 | MD 7 / A 0 |
| 105 | tld-4466 | md_mapped_missing | empty_file | STP_018 | MD 7 / A 0 |
| 105 | tld-478 | md_mapped_missing | empty_file | STP_018 | MD 7 / A 0 |
| 100 | mdb-1252 | analyzer_spec_md_absent |  | STP_009 |  / A 1 |
| 100 | mdb-2838 | analyzer_spec_md_absent |  | STP_009 |  / A 9 |
| 100 | mdb-3360 | analyzer_spec_md_absent |  | TRP_002 |  / A 9 |
| 95 | mdb-865 | analyzer_spec_unmapped |  | STP_036 |  / A 122 |
| 95 | tfs-535 | analyzer_spec_unmapped |  | STP_036 |  / A 14 |
| 92 | jbda-awajicity-awaji-jenova-line-akashi-iwaya | analyzer_spec_md_absent |  | TRN_013 |  / A 1 |
| 92 | jbda-chitetsu-chitetsubus | analyzer_spec_md_absent |  | TRN_016 |  / A 1 |
| 92 | jbda-ichinosekicity-Ichinosekibus | analyzer_spec_md_absent |  | TRN_016 |  / A 1 |
| 92 | jbda-imizucity-imizushi | analyzer_spec_md_absent |  | TRN_016 |  / A 1 |
| 92 | jbda-joetsucity-joetsu | analyzer_spec_md_absent |  | TRN_016 |  / A 1 |
| 92 | jbda-kakogawacity-kakobuskakobusmini | analyzer_spec_md_absent |  | TRN_016 |  / A 1 |
| 92 | jbda-kamacity-KamaCityBus | analyzer_spec_md_absent |  | TRN_016 |  / A 1 |
| 92 | jbda-kandatown-KandaTownCommunityBus | analyzer_spec_md_absent |  | TRN_016 |  / A 1 |
| 92 | jbda-kanonjicity-kanonjishi-noriaibus | analyzer_spec_md_absent |  | TRN_016 |  / A 1 |
| 92 | jbda-kitakyushucity-tashiro-kawachi | analyzer_spec_md_absent |  | TRP_004 |  / A 3 |
| 92 | jbda-kochi-seinan-kotsu-GTFS-Seinantraffic_Localbus | analyzer_spec_md_absent |  | TRN_016 |  / A 1 |
| 92 | jbda-kosugevillage-kosugesoneibus | analyzer_spec_md_absent |  | TRN_016 |  / A 1 |
| 92 | jbda-kotosan-kotohira-local | analyzer_spec_md_absent |  | TRN_016 |  / A 1 |
| 92 | jbda-kusakarukotsu-kusakarubus | analyzer_spec_md_absent |  | TRN_016 |  / A 1 |
| 92 | jbda-mie-kotsu-sancoiga | analyzer_spec_md_absent |  | TRN_016 |  / A 1 |
| 92 | jbda-mie-kotsu-sancoise | analyzer_spec_md_absent |  | TRN_016 |  / A 1 |
| 92 | jbda-mie-kotsu-sancokuwana | analyzer_spec_md_absent |  | TRN_016 |  / A 1 |
| 92 | jbda-mie-kotsu-sancomatsusaka | analyzer_spec_md_absent |  | TRN_016 |  / A 1 |
| 92 | jbda-mie-kotsu-sancoshima | analyzer_spec_md_absent |  | TRN_016 |  / A 1 |
| 92 | jbda-mie-kotsu-sancoyokkaichi | analyzer_spec_md_absent |  | TRN_016 |  / A 1 |
| 92 | jbda-miyawakacity-MiyawakaMunakataLine | analyzer_spec_md_absent |  | TRN_013 |  / A 2 |
| 92 | jbda-nagadenbus-nagadenbus-nagano | analyzer_spec_md_absent |  | TRN_016 |  / A 1 |
| 92 | jbda-nagadenbus-nagadenbus-nakano | analyzer_spec_md_absent |  | TRN_016 |  / A 1 |
| 92 | jbda-nagai-unyu-Nagaibus | analyzer_spec_md_absent |  | TRN_016 |  / A 1 |
| 92 | jbda-nogatacity-NogataCityCommunityBus | analyzer_spec_md_absent |  | TRN_016 |  / A 1 |
| 92 | jbda-ongatown-OngaTownCommunityBus | analyzer_spec_md_absent |  | TRN_016 |  / A 1 |
| 92 | jbda-sakatacity-SakataCity | analyzer_spec_md_absent |  | AGN_009 |  / A 1 |
| 92 | jbda-setocity-setocitybus | analyzer_spec_md_absent |  | TRN_016 |  / A 1 |
| 92 | jbda-shichinohetown-shichinohe_community-bus | analyzer_spec_md_absent |  | TRN_016 |  / A 1 |
| 92 | jbda-shimanto-kotsu-GTFS-Shimantotraffic_Localbus | analyzer_spec_md_absent |  | TRN_016 |  / A 1 |
| 92 | jbda-shinjobankotsu-SHINJOBANKOTSU | analyzer_spec_md_absent |  | TRN_013 |  / A 6 |
