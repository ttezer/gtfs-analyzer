# Seventh full MobilityDatabase GTFS Schedule audit

Transient PR trigger for the seventh full-catalog verification run.

- Base: `main` at `fd40642c`, CI green
- Validation date: `2026-08-20` (`BENCH_DATE=20260820`, `MD_DATE=2026-08-20`) — same as run 6, so date-sensitive rules must NOT move
- MobilityData validator: `8.0.1`, pinned
- Purpose: test seven product changes. Not a discovery run.

## Why this run is justified

One of the changes is in the CSV tokenizer, which affects how **all 4,271 feeds** are
parsed rather than what one rule reports. Four feeds were verified locally; whether the
change merges or shifts data anywhere else in the catalog can only be answered at corpus
scale. Shipping a release without that answer would hand the risk to users.

## Predictions

| change | feed | expected |
|---|---|---|
| CSV tokenizer | `tdg-80973` | `RTS_031` 7 → **0**, `ARC_012` 7 → **0** |
| | | 🔴 `ARC_033` **holds at 1** — the violation must still be reported |
| `ARC_034` streams | `mdb-1004` | `ARC_034` 5 → **8**, `TRP_002` 2 → **0** |
| | `mdb-1003` | `ARC_034` 5 → **6** |
| `TRN_011` | `mdb-2126` | 421 → **0**, 🔴 `TRN_001` **holds at 421** |
| | `jbda-shinjobankotsu…` | 1,044 → **0**, 🔴 `TRN_001` **holds at 1,056** |
| `DQ_018` threshold | `mdb-2389` | 2,394 → **0** |
| | `mdb-2653` | 🔴 **holds at 926** — five-letter-plus still caught |
| `TRP_024` aggregation | `tdg-83634` | 5,798 → **1,887** |
| | `tdg-81942` | 🔴 **holds at 1,870** |
| `STP_009` raw FK | `mdb-2003` | 2 → **0** |
| `STM_034`/`STM_047` | corpus-wide | **no change** — preventive fix, 26 STM_047 feeds carry no STM_003/004 |

Every row has a "must hold" half. If `ARC_033`, `TRN_001` or `STM_003` fall alongside the
drops, the fix silenced the correct rule and the headline improvement is hiding a
regression. That check is what made run 6 worth its hour.

## Must not change

Attempted feeds 4,271 · firing rules 422 · `md_unmapped` 0 · `STM_061` 331 feeds
(aggregation groups findings, it must not narrow coverage) · date-sensitive rules
(`CAL_013`, `CAL_024`, `FIN_010`, `FIN_019`, `TRP_023`) since the date is unchanged.

This file exists only to create a `benchmark/audit_all/**` diff and trigger the workflow.
Do not merge this branch.
