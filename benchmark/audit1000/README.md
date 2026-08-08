# 1000-feed GTFS Analyzer vs MobilityData audit

This benchmark is an external-validator audit corpus, not a marketing benchmark.

## Corpus

- Source catalog: MobilityDatabase `feeds_v2.csv`.
- Feed type: GTFS Schedule.
- Status: active or blank.
- A public `latest` dataset URL must exist.
- Duplicate dataset URLs are removed.
- Selection is deterministic: candidates are sorted by
  `SHA256(seed + feed_id + latest_url)` and the first 1000 are selected.
- Seed: `gtfs-analyzer-audit-1000-v1`.
- The catalog SHA-256 and the complete 1000-feed manifest are retained.

The selection deliberately does not filter feeds by expected validator result.

## Validators

Every successfully downloaded ZIP is passed to both:

1. GTFS Analyzer, release native CLI built from the PR head.
2. MobilityData `gtfs-validator` v8.0.1, Java 21, `-Xmx12g`.

The exact downloaded ZIP SHA-256 is recorded. The same local ZIP bytes are used for both validators.

MobilityDatabase's stored v8.0.1 report for the same resolved snapshot is also fetched when available. It is used only as an independent sanity check; the main comparison uses the fresh MobilityData run.

## Resource controls

Per feed:

- Download timeout: 300 s.
- Maximum compressed download: 1.2 GB.
- GTFS Analyzer timeout: 300 s.
- MobilityData timeout: 420 s.
- MobilityData maximum Java heap: 12 GB.

A timeout, OOM, invalid download, parser failure, or internal validator error is part of the result. A failed feed is not silently replaced with another feed.

The workload is divided into 50 deterministic shards with 20 feeds each and up to 20 shards running concurrently.

## Result retention

Full per-feed reports are not retained because some Analyzer reports can be hundreds of megabytes. Each shard retains a structured summary containing:

- feed ID/provider/country/source URL;
- effective downloaded URL;
- ZIP size and SHA-256;
- validator state and exit code;
- wall time and peak RSS;
- Analyzer notice counts by rule/class/severity;
- one representative Analyzer notice per emitted rule;
- MobilityData counts and samples per notice code;
- MobilityData `system_errors.json` contents when present.

The aggregate stage retains the full list of affected feed IDs for every Analyzer rule and every MobilityData notice code.

## Comparison semantics

`spec-audit/md_parity_audit.py` is the canonical mapping from MobilityData notice codes to GTFS Analyzer rule IDs.

A raw count difference is **not** automatically a validator bug. Differences can come from:

- per-row vs per-entity/per-feed aggregation;
- different thresholds;
- different semantic scope;
- cascade suppression;
- parser behavior;
- an actual false positive or false negative.

The aggregate therefore produces *divergence candidates*. The highest-priority unique divergence categories are followed by a raw-evidence extraction pass that re-downloads the pinned snapshot and captures the source rows referenced by validator samples.

Correctness verdicts require reviewing the raw GTFS evidence and the normative GTFS text; automated parity alone is not treated as proof.
