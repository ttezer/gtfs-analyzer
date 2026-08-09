# Partial validation

GTFS Analyzer has three observable validation outcomes:

- `COMPLETE`: the archive was readable and every pipeline stage ran.
- `PARTIAL`: the archive was readable, but one or more files could not be safely
  typed or a deterministic recovery path was used. Independent checks still run;
  coarse stage metadata remains in `partial.skipped_stages`, while each
  prerequisite-gated K4/K5/K6 check that could not run is listed in
  `partial.skipped_checks`.
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
    "skipped_stages": [],
    "skipped_checks": [
      "K4::routes",
      "K6::route_headway",
      "K6::speed_and_duration"
    ]
  }
}
```

The recovery boundary is deliberately conservative. Missing, invalid-UTF-8 or
CSV-malformed files are not converted lossily into typed records. K1/K2 findings
from files that remain readable are preserved; only cross-reference, derived,
analytics, or individual data-quality checks whose required inputs are
unavailable are skipped, so they cannot invent findings from an absent
prerequisite. Corrupt ZIP bytes and decompression-limit failures remain fatal.
