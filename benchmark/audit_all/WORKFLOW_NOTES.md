# Execution invariants

- Corpus scope is MobilityDatabase catalog rows with `data_type=gtfs` only.
- GTFS Realtime rows are never selected.
- Selection is exhaustive: no sampling and no URL deduplication.
- Schedule rows without a public HTTP(S) latest URL are retained in manifest exclusions and counted as untestable, not silently dropped.
- Each testable feed produces one JSONL result row even when download or validation fails.
- Validator failures are data, not shard failures; shard strategy is `fail-fast: false`.
- Final completeness compares attempted result rows against the manifest's testable Schedule count.
