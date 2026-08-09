# 1000-feed GTFS Analyzer vs MobilityData rerun audit

This is a temporary benchmark/audit harness, not production code and not a marketing benchmark.
The rerun is designed primarily as a **before/after** measurement against workflow run
`31284358651` while also re-running MobilityData `gtfs-validator` v8.0.1 on the same newly
downloaded bytes used by GTFS Analyzer.

## Pinned Analyzer source

The Analyzer binary is built from the latest GREEN `main` commit pinned in the workflow as
`ANALYZER_SHA`. Benchmark files are checked out separately. No validator, parity, or product code
is copied from the historical audit branch.

The result for every feed records:

- Analyzer product commit SHA;
- benchmark branch/commit SHA;
- Analyzer config: `ValidatorConfig::default()` (no threshold override);
- Analyzer validation date: `20260809`;
- MobilityData version: `8.0.1`;
- MobilityData validation date: `2026-08-09`.

## Corpus provenance and input identity

The corpus is **not reselected** from a new MobilityDatabase catalog. The workflow downloads the
historical `audit1000-corpus` artifact (ID `9029347252`) from run `31284358651` and reuses its exact
1000-feed manifest and seed `gtfs-analyzer-audit-1000-v1`.

The historical `validator-audit-1000-final` artifact (ID `9029535391`) is downloaded read-only as
the baseline. Its `all-results.json.gz` provides the SHA-256 of the ZIP bytes used for every feed in
the old run. The old artifact is never modified or uploaded under the same name.

Because the manifest points at `latest.zip`, each rerun feed is classified as:

- `SAME_INPUT` — new ZIP SHA-256 equals the historical ZIP SHA-256;
- `INPUT_DRIFT` — a ZIP was downloaded but its SHA-256 differs (or no historical SHA can prove identity);
- `DOWNLOAD_FAILED` — the new input could not be obtained as a valid ZIP.

Only `SAME_INPUT` feeds are interpreted as direct old-vs-new Analyzer regressions/improvements.
Failed feeds are never replaced.

## Validator execution

The exact same local `feed.zip` bytes are passed to both validators.

Per-feed resource controls are unchanged from the historical run:

- 1000 feeds;
- 50 deterministic shards × 20 feeds;
- matrix `max-parallel: 20`;
- download timeout: 300 s;
- maximum compressed download: 1.2 GB;
- GTFS Analyzer timeout: 300 s;
- MobilityData timeout: 420 s;
- MobilityData Java heap: 12 GB.

Validation dates are deliberately held at the historical date to avoid calendar/expiry deltas caused
only by running one day later. Analyzer remains on default config; in particular FIN_019 is not
changed to mimic MobilityData's 30-day threshold. The current main parity ledger's
`config-dependent` decision is used instead.

## Analyzer result model

The historical shard runner treated any existing report file as `completed`, which hid fatal JSON
reports. The rerun parses the current CLI envelope explicitly:

- `status=ok` + `validation_status=COMPLETE` → `COMPLETE`;
- `status=partial` + `validation_status=PARTIAL` → `PARTIAL`;
- `status=fatal` → `FATAL`.

It also retains exit code, `partial.root_structural_errors`, `partial.unavailable_files`,
`partial.skipped_stages`, `partial.skipped_checks`, notice counts by rule/class/severity, one
representative notice per rule, `capped_totals`, timing and RSS. This allows issue #113's
recover-and-report behavior to be measured rather than collapsed into a generic completed state.

`/usr/bin/time -v` elapsed output is parsed after the final format-description delimiter (not the
colons inside `h:mm:ss or m:ss`). Aggregation reports median, p95 and maximum wall time, plus median
and p95 peak RSS for both validators.

## Preflight gate

The full 1000-feed matrix depends on a preflight job that performs:

1. Python syntax/compile checks for the benchmark scripts;
2. baseline manifest/results preparation smoke;
3. release build of `gtfs-cli` from pinned Analyzer `main`;
4. current CLI JSON parser test against actual COMPLETE, PARTIAL and FATAL fixtures;
5. a one-feed Analyzer + MobilityData smoke using the reused corpus;
6. aggregation smoke using current `spec-audit/md_parity_audit.py` and `md_parity_mapping.py`;
7. an assertion that parsed wall times are numeric.

If preflight fails, the full corpus jobs do not start.

## Final analyses

The final artifact contains two separate analyses.

### 1. New Analyzer vs MobilityData v8.0.1

Uses the current `main` parity code and reports exact/near/config-dependent/intentional-gap
classification, MD-only candidates, Analyzer-only candidates, count divergences, validator-state
asymmetries, and high-priority raw evidence. Automated differences are triage candidates, not
correctness verdicts.

### 2. Historical Analyzer vs new Analyzer

Direct regression/improvement language is restricted to `SAME_INPUT` feeds. The report tracks:

- old rule no longer emits / new rule emits;
- finding count increases/decreases;
- class or severity changes;
- old FATAL → new PARTIAL/COMPLETE improvements;
- old COMPLETE → new PARTIAL/FATAL red regressions;
- targeted corpus impact for whitespace-related changes, translations cascade suppression,
  midnight rules, fare products, shape rules, and recovery.

`INPUT_DRIFT` and `DOWNLOAD_FAILED` feeds are listed separately with old/new SHA-256 where available.

The preflight also captures `gtfs-analyzer rules --json` from the pinned Analyzer binary. The
before/after stage uses that registry to distinguish a finding that merely stopped emitting from an
old observed rule ID that has actually been removed, and to compare class/severity metadata without
mistaking a missing per-feed sample for a metadata change. Full MD-code parity observations are
retained separately from the divergence-candidate queue.
