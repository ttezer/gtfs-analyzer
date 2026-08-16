# PR scope

This branch adds an exhaustive benchmark of MobilityDatabase GTFS Schedule feeds. It does not change production validation rules or application behavior.

The benchmark compares GTFS Analyzer with MobilityData gtfs-validator v8.0.1, isolates feeds into shards, records download/timeout/OOM/input failures as results, and produces aggregate parity/triage artifacts with an explicit corpus-completeness check.
