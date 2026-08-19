# Sixth full MobilityDatabase GTFS Schedule audit

Transient PR trigger for the sixth full-catalog verification run.

- Base: `main` at `005dbba00ed5d40cf4d90b8ab24d0d3ac0e2ea38`
- Validation date: `2026-08-20` (`BENCH_DATE=20260820`, `MD_DATE=2026-08-20`)
- MobilityData validator: `8.0.1`, pinned
- Aggregate manifest gate: enabled
- Authoritative run instructions: `spec-audit/FULL_CATALOG_RUN.md`
- Purpose: test the five predeclared changes in §5. This is not a discovery run.

## What this run has to show

Product side, each naming its feed in advance:

- `STM_061` pair aggregation — `mdb-1003` 9,533 -> 11, `mdb-1004` 10,390 -> 20,
  `mdb-3401` 6,091 -> 43. The number of feeds firing must stay at 331: aggregation
  groups findings, it does not narrow coverage.
- `TIME_MALFORMED` — `mdb-3401` `STM_016` 10,884 -> 0 and `mdb-2727` `STM_015`/`STM_016`
  101,621 -> 0 each. **`STM_003` must hold at 218,087 on `mdb-3401`.** If it falls, the
  fix silenced the correct rule rather than the wrong one, and the headline drop would
  be hiding a regression.

Tool side, already verified locally by re-aggregating the fifth run's raw rows; the run
tests that the same result holds on real shard data:

- `analyzer_spec_unmapped` 327 -> ~9
- `md_mapped_missing` 133 -> ~12, with no `fast_travel_between_far_stops` row left
- `analyzer_mapped_md_absent` ~2,196

Date moves one day forward, so `CAL_013`, `CAL_024`, `FIN_010`, `FIN_019` and `TRP_023`
shift accordingly. That is expected and is not product movement — no rule under test is
date-dependent.

This file exists only to create a `benchmark/audit_all/**` diff and trigger the
pull-request workflow. Do not merge this run-trigger branch.
