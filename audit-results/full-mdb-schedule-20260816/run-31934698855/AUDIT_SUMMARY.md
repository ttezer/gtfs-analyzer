# Full MobilityDatabase GTFS Schedule — GTFS Analyzer vs MobilityData audit

## Corpus execution

- Attempted feeds: **4259**
- Successfully downloaded: **4226**
- Both validators completed cleanly: **4215**
- Analyzer median wall time (completed feeds): **None s**
- MobilityData median wall time (completed feeds): **None s**
- Analyzer rules observed: **414**
- MobilityData notice codes observed: **117**

These are automated divergence *candidates*, not correctness verdicts. A count difference can be caused by aggregation, thresholds, scope, or a true validator bug.

## Validator state pairs

| Analyzer | MobilityData | Feeds |
|---|---|---:|
| completed | completed | 4215 |
| not_run | not_run | 33 |
| completed | partial_internal | 5 |
| completed | no_report | 2 |
| timeout | completed | 2 |
| completed | timeout | 2 |

## Divergence candidate classes

| Candidate class | Count |
|---|---:|
| analyzer_mapped_md_absent | 12290 |
| md_mapped_missing | 4706 |
| md_mapped_over | 2707 |
| md_mapped_under | 2037 |
| analyzer_spec_unmapped | 1809 |
| md_unmapped | 1008 |
| analyzer_spec_md_absent | 134 |
| validator_state_asymmetry | 11 |

## Highest-priority candidates

| Priority | Feed | Direction | MD code | Analyzer rule(s) | Counts |
|---:|---|---|---|---|---|
| 110 | mdb-1186 | validator_state_asymmetry |  |  |  |
| 110 | mdb-1960 | validator_state_asymmetry |  |  |  |
| 110 | mdb-2607 | validator_state_asymmetry |  |  |  |
| 110 | mdb-2727 | validator_state_asymmetry |  |  |  |
| 110 | mdb-2867 | validator_state_asymmetry |  |  |  |
| 110 | mdb-3215 | validator_state_asymmetry |  |  |  |
| 110 | mdb-3401 | validator_state_asymmetry |  |  |  |
| 110 | mdb-357 | validator_state_asymmetry |  |  |  |
| 110 | mdb-784 | validator_state_asymmetry |  |  |  |
| 110 | tdg-81618 | validator_state_asymmetry |  |  |  |
| 110 | tdg-83019 | validator_state_asymmetry |  |  |  |
| 105 | jbda-ashikagacity-Ashikaga-route-bus | md_mapped_missing | duplicate_key | SHP_008 | MD 1 / A 0 |
| 105 | jbda-chitetsu-chitetsubus | md_mapped_missing | duplicate_key | SHP_008 | MD 2 / A 0 |
| 105 | jbda-kurobecity-ishida-aimoto | md_mapped_missing | duplicate_key | SHP_008 | MD 380 / A 0 |
| 105 | jbda-kurobecity-shinkansenikuji-ishida-aimoto-minibus | md_mapped_missing | foreign_key_violation | STM_002 | MD 2 / A 0 |
| 105 | jbda-kuroishicity-kuroishi | md_mapped_missing | duplicate_key | SHP_008 | MD 1 / A 0 |
| 105 | jbda-mie-kotsu-sancochusei | md_mapped_missing | duplicate_key | SHP_008 | MD 52 / A 0 |
| 105 | jbda-minamishinshu-Minamishinshu-Nanbu-1 | md_mapped_missing | duplicate_key | SHP_008 | MD 121 / A 0 |
| 105 | jbda-minamishinshu-Minamishinshu-takamori-1 | md_mapped_missing | foreign_key_violation | STM_002 | MD 15 / A 0 |
| 105 | jbda-nagahamacity-nishiazaiodekakewagon | md_mapped_missing | foreign_key_violation | STM_002 | MD 15 / A 0 |
| 105 | jbda-nagano-kawakamivillage-kawakamibus | md_mapped_missing | duplicate_key | SHP_008 | MD 51 / A 0 |
| 105 | jbda-namerikawacity-norumycar | md_mapped_missing | duplicate_key | SHP_008 | MD 2605 / A 0 |
| 105 | jbda-nantocity-nanbus | md_mapped_missing | foreign_key_violation | STM_002 | MD 4 / A 0 |
| 105 | jbda-nikkocity-nikkocity | md_mapped_missing | missing_required_field | AGN_003, FIN_002 | MD 200 / A 0 |
| 105 | jbda-sankobus-sankobus | md_mapped_missing | foreign_key_violation | STM_002 | MD 136 / A 0 |
| 105 | jbda-shinonsentown-yumetsubame | md_mapped_missing | foreign_key_violation | STM_002 | MD 132 / A 0 |
| 105 | jbda-tanabecity-Jumin-Bus | md_mapped_missing | foreign_key_violation | STM_002 | MD 1 / A 0 |
| 105 | jbda-tochigi-nakagawatown-nakagawamaticommunitybus | md_mapped_missing | invalid_url | AGN_003, RTS_005, FIN_002 | MD 1 / A 0 |
| 105 | jbda-waketown-wakechoueibasu | md_mapped_missing | duplicate_key | SHP_008 | MD 1 / A 0 |
| 105 | mdb-1 | md_mapped_missing | trip_distance_exceeds_shape_distance | SHP_025 | MD 189 / A 0 |
| 105 | mdb-1000 | md_mapped_missing | foreign_key_violation | STM_002 | MD 1481 / A 0 |
| 105 | mdb-1000 | md_mapped_missing | missing_required_field | AGN_003, FIN_002 | MD 267 / A 0 |
| 105 | mdb-1002 | md_mapped_missing | foreign_key_violation | STM_002 | MD 579 / A 0 |
| 105 | mdb-1011 | md_mapped_missing | foreign_key_violation | STM_002 | MD 4 / A 0 |
| 105 | mdb-1016 | md_mapped_missing | duplicate_key | SHP_008 | MD 8 / A 0 |
| 105 | mdb-1019 | md_mapped_missing | missing_calendar_and_calendar_date_files | ARC_008 | MD 1 / A 0 |
| 105 | mdb-1019 | md_mapped_missing | missing_required_file | ARC_004 | MD 5 / A 0 |
| 105 | mdb-102 | md_mapped_missing | duplicate_key | SHP_008 | MD 5 / A 0 |
| 105 | mdb-102 | md_mapped_missing | missing_required_field | AGN_003, FIN_002 | MD 6 / A 0 |
| 105 | mdb-1028 | md_mapped_missing | missing_calendar_and_calendar_date_files | ARC_008 | MD 1 / A 0 |
| 105 | mdb-1028 | md_mapped_missing | missing_required_file | ARC_004 | MD 5 / A 0 |
| 105 | mdb-1074 | md_mapped_missing | duplicate_key | SHP_008 | MD 231 / A 0 |
| 105 | mdb-1077 | md_mapped_missing | duplicate_key | SHP_008 | MD 1 / A 0 |
| 105 | mdb-1078 | md_mapped_missing | duplicate_key | SHP_008 | MD 5 / A 0 |
| 105 | mdb-1105 | md_mapped_missing | missing_calendar_and_calendar_date_files | ARC_008 | MD 1 / A 0 |
| 105 | mdb-1105 | md_mapped_missing | missing_required_file | ARC_004 | MD 5 / A 0 |
| 105 | mdb-1106 | md_mapped_missing | missing_calendar_and_calendar_date_files | ARC_008 | MD 1 / A 0 |
| 105 | mdb-1106 | md_mapped_missing | missing_required_file | ARC_004 | MD 5 / A 0 |
| 105 | mdb-1125 | md_mapped_missing | invalid_url | AGN_003, RTS_005, FIN_002 | MD 1912 / A 0 |
| 105 | mdb-1130 | md_mapped_missing | invalid_url | AGN_003, RTS_005, FIN_002 | MD 935 / A 0 |
| 105 | mdb-1134 | md_mapped_missing | foreign_key_violation | STM_002 | MD 9110 / A 0 |
| 105 | mdb-1136 | md_mapped_missing | invalid_url | AGN_003, RTS_005, FIN_002 | MD 760 / A 0 |
| 105 | mdb-114 | md_mapped_missing | duplicate_key | SHP_008 | MD 1 / A 0 |
| 105 | mdb-114 | md_mapped_missing | foreign_key_violation | STM_002 | MD 524 / A 0 |
| 105 | mdb-1150 | md_mapped_missing | duplicate_key | SHP_008 | MD 121 / A 0 |
| 105 | mdb-1186 | md_mapped_missing | foreign_key_violation | STM_002 | MD 1292 / A 0 |
| 105 | mdb-1194 | md_mapped_missing | duplicate_key | SHP_008 | MD 62484 / A 0 |
| 105 | mdb-1223 | md_mapped_missing | trip_distance_exceeds_shape_distance | SHP_025 | MD 51 / A 0 |
| 105 | mdb-1229 | md_mapped_missing | duplicate_key | SHP_008 | MD 170885 / A 0 |
| 105 | mdb-1229 | md_mapped_missing | stop_time_with_arrival_before_previous_departure_time | STM_008 | MD 1195 / A 0 |
