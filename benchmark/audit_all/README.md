# Full MobilityDatabase GTFS Schedule audit

This benchmark audits the full eligible MobilityDatabase **GTFS Schedule** catalog against GTFS Analyzer and MobilityData gtfs-validator.

Scope is intentionally static GTFS only: catalog rows whose `data_type` is `gtfs`. GTFS Realtime (`gtfs_rt` and other realtime data types) is excluded.

The run is exhaustive rather than sampled. It uses shard isolation, per-feed download/validator timeouts, immediate JSONL flushing, and per-feed temporary-directory cleanup so one bad or oversized feed does not terminate the whole corpus.

Artifacts must retain the catalog snapshot and generated manifest so the exact corpus can be reconstructed.
The workflow keeps its short-lived Actions artifacts for 90 days and publishes the final result bundle,
checksum and `provenance.json` as a GitHub prerelease tagged with the workflow run ID. Those audit tags
are archival records, not product release tags.

## Payload provenance

Each shard records the downloaded payload's SHA-256, byte count, HTTP status,
content type, effective URL and ZIP decision. `status=not_zip` means that the
catalog URL returned a non-ZIP payload; it does **not** prove that the catalog
record is not a GTFS feed. This distinction matters for deprecated entries whose
old redirect now returns an HTML/XML page while an official source still serves
the feed.

To compare a rerun with an earlier aggregate, pass its raw result array to the
aggregator:

```text
python3 benchmark/audit_all/aggregate.py \
  --results-dir benchmark/audit_all/all-shards \
  --map-file spec-audit/md_parity_audit.py \
  --manifest benchmark/audit_all/run/manifest.json \
  --baseline-results /path/to/previous/all-results.json.gz \
  --out-dir benchmark/audit_all/final
```

The run writes `source-drift.json` and includes the same result in
`summary.json`. A changed SHA, byte count, effective URL, HTTP status or content
type is reported per feed; feed IDs are never deduplicated.
