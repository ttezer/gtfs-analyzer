# Fifth full MobilityDatabase GTFS Schedule audit

Transient run trigger for the fifth full-catalog validation run.

- Product baseline: `c97698c775b941d7863e51f0d8aac50e3bff535d` from `main`
- Run date: `2026-08-19`
- `BENCH_DATE=20260819`
- `MD_DATE=2026-08-19`
- MobilityData validator: `8.0.1` (pinned; do not upgrade)
- Aggregate manifest health gate: enabled on `main`
- Instructions and predeclared expectations: `spec-audit/FULL_CATALOG_RUN.md`, especially sections 0, 3, 4, 5 and 6

This file exists only to create a `benchmark/audit_all/**` diff so the pull-request workflow runs from a fresh branch without changing product behavior. The run branch is not intended to be merged.
