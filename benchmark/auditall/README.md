# Full MobilityDatabase GTFS Schedule audit

This benchmark runs the pinned GTFS Analyzer CLI and MobilityData gtfs-validator v8.0.1 over the full runnable GTFS Schedule population exposed by the MobilityDatabase catalog snapshot.

## Scope

The corpus selector accepts only rows satisfying all of these conditions:

- `data_type == "gtfs"`
- `status` is `active` or blank
- no authentication is required
- `urls.latest` is present
- `redirect.id` is blank

GTFS-Realtime rows (`data_type == "gtfs-rt"`) are excluded before the manifest is constructed and are never passed to either validator. Static references attached to Realtime rows are not followed because the corresponding Schedule feeds already exist as their own catalog rows.

`mdb-2904` is explicitly excluded as documented in `spec-audit/FULL_CATALOG_RUN.md` because its pathological finding volume would dominate the measurement.

This is a full-population measurement, not a regression comparison with the older stratified corpus. Duplicate feed rows are all executed, but the aggregator groups identical downloaded ZIPs by SHA-256 so mirror duplication is visible and can be excluded from downstream statistics when appropriate.

## Crash isolation

The workflow uses 128 deterministic shards with `fail-fast: false`. Each feed is isolated by download, Analyzer and MobilityData timeouts; runner exceptions are recorded as a result row and the shard continues. Every completed feed row is flushed immediately to JSONL, and every shard uploads its checkpoint with `if: always()`.

The aggregate job runs even when individual shard jobs fail. It compares the received feed IDs with the manifest and writes `coverage-gaps.json`, so a missing shard or job-level failure cannot silently disappear from the final report.

## Fixed validators

- Analyzer product source: `155bb06037a9caca339e4bd893619e5c3ab5fafb`
- MobilityData validator: `8.0.1`
- Analyzer validation date: `20260809`
- MobilityData validation date: `2026-08-09`

The validation date is intentionally fixed so calendar-relative rules do not drift during a multi-job run.

## Outputs

The final artifact contains the corpus manifest, compact per-feed status/performance data, Analyzer rule totals, MobilityData notice-code totals, parity divergence candidates, duplicate-content groups, coverage gaps, representative raw evidence, and the gzipped row-level audit results. Automated divergences are triage candidates, not correctness verdicts.
