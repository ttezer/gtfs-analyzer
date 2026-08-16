# Full MobilityDatabase GTFS Schedule audit

This benchmark audits the full eligible MobilityDatabase **GTFS Schedule** catalog against GTFS Analyzer and MobilityData gtfs-validator.

Scope is intentionally static GTFS only: catalog rows whose `data_type` is `gtfs`. GTFS Realtime (`gtfs_rt` and other realtime data types) is excluded.

The run is exhaustive rather than sampled. It uses shard isolation, per-feed download/validator timeouts, immediate JSONL flushing, and per-feed temporary-directory cleanup so one bad or oversized feed does not terminate the whole corpus.

Artifacts must retain the catalog snapshot and generated manifest so the exact corpus can be reconstructed.
