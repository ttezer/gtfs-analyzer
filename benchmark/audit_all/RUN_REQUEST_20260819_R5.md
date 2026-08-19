# Fifth full MobilityDatabase GTFS Schedule audit

Transient PR trigger for the fifth full-catalog verification run.

- Base: `main` at `c97698c775b941d7863e51f0d8aac50e3bff535d`
- Validation date: `2026-08-19` (`BENCH_DATE=20260819`, `MD_DATE=2026-08-19`)
- MobilityData validator: `8.0.1`, pinned
- Aggregate manifest gate: enabled
- Authoritative run instructions: `spec-audit/FULL_CATALOG_RUN.md`
- Purpose: verify the six predeclared product changes in §5; this is not a discovery run

This file exists only to create a `benchmark/audit_all/**` diff and trigger the pull-request workflow. Do not merge this run-trigger branch.
