# gtfs-analyzer

Validate a [GTFS Schedule](https://gtfs.org/documentation/schedule/reference/) feed from the command line.

```sh
cargo install gtfs-analyzer
gtfs-analyzer validate feed.zip
```

## What it does

- **600+ rules** across four classes — Spec conformance, interoperability, data quality and analytics.
- **Notices carry remediation**: every finding says what is wrong, where, and what to change.
- **Scores**: a weighted overall score plus a publication score built only from blocker-eligible findings.
- **JSON or human-readable output**, selectable severity and rule filters.
- **Streams large archives**: feeds with millions of `stop_times` rows are validated without loading the file into memory.

```sh
gtfs-analyzer validate feed.zip --json --lang en
gtfs-analyzer validate feed.zip --today 20260820
```

Exit code is `0` when the feed is clean, `1` when notices were produced, and `2` on a fatal error.

## Accuracy

Every release is measured against the **entire MobilityDatabase GTFS Schedule catalogue** — over 4,300 real feeds — and compared feed by feed with the reference implementation, `MobilityData/gtfs-validator`. Divergences are adjudicated and recorded rather than silently accepted.

## Library crates

`gtfs-core`, `gtfs-config`, `gtfs-rules` and `gtfs-pipeline` are published so this binary can be built from crates.io. They are internal to the analyzer and their APIs carry **no stability guarantee** — depend on them only if you are prepared for breaking changes in patch releases.

## License

MIT. Source and issue tracker: <https://github.com/ttezer/gtfs-analyzer>
