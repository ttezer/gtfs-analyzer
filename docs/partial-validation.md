# Partial validation

GTFS Analyzer has three observable validation outcomes:

- `COMPLETE`: the archive was readable and every pipeline stage ran.
- `PARTIAL`: the archive was readable, but one or more files could not be safely
  typed or a deterministic recovery path was used. Independent checks still run;
  stages that require unavailable inputs are listed in `partial.skipped_stages`.
- `FATAL`: the ZIP/archive itself could not be opened or the decompression guard
  rejected it. No validation report is produced.

`PARTIAL` is intentionally not a clean result. The CLI returns exit code `1` for
both notices and partial coverage, and `2` for fatal/config/input errors. With
`--fail-on*`, a partial run still returns `1`; a caller must not treat reduced
coverage as a complete successful validation.

The JSON envelope exposes:

```json
{
  "status": "partial",
  "validation_status": "PARTIAL",
  "partial": {
    "root_structural_errors": [],
    "unavailable_files": ["routes.txt"],
    "skipped_stages": ["K4-cross-ref", "K5-derived", "K6-analytics"]
  }
}
```

The recovery boundary is deliberately conservative. Missing, invalid-UTF-8 or
CSV-malformed files are not converted lossily into typed records. K1/K2 findings
from files that remain readable are preserved; cross-reference and derived
stages are skipped when any required input is unavailable so they cannot invent
findings from an absent prerequisite. Corrupt ZIP bytes and decompression-limit
failures remain fatal.
