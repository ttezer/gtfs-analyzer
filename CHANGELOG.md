# Changelog

All notable changes to this project are documented here.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [0.1.3] - 2026-06-22

This release focuses on real-feed accuracy: several rules that produced large numbers
of false positives on production feeds (TriMet, Tokyo Toei, BART) were corrected, and
a deterministic Golden JSON snapshot export was added for version-to-version
regression comparison. The rule set grew from 518 to 526.

### Added
- **Golden JSON snapshot export** on the Export tab — a deterministic per-rule
  aggregate (scores, severity counts, per-rule counts, feed metrics) intended for
  comparing validator output across versions. The download is named
  `<feed>_<date>_<version>.json` and embeds `app_version`; feed contents are never
  included.
- New rules (518 → 526): **CAL_024** (block trips with overlapping service, moved from
  TRP_030), **GEO_022** (stop coordinate near a pole), **AGN_017** (inconsistent
  `agency_lang`), **JPN_011** (`agency_id` required even for single-agency GTFS-JP
  feeds), **STP_037 / STP_038** (`wheelchair_boarding` completeness), **FTR_009 /
  FTR_010 / FTR_011** (conditional `transfer_count` / `duration_limit_type`).
- Debug bundle export, a route trip-pattern summary modal, and an Export Summary Table
  card (copyable + PNG). R2 rule codes now link to the GTFS specification.

### Changed
- **Breaking (rule IDs):** `TRP_030` is renamed to `CAL_024` and moved to the CAL group.
- Large false-positive reductions on real feeds:
  - **TRP_022** — cross-calendar block overlaps are no longer flagged; only services
    active on the same day can conflict (TriMet: ~201k → ~900).
  - **TRP_011** — skipped when the route itself is named (TriMet: ~52k → 0).
  - **VAT_001** — temporal route splits (same short name, disjoint calendars) are
    suppressed; both route IDs are named in the message.
  - **VAT_003** — trip durations are deseasonalized by time-of-day band and grouped by
    stop pattern before the robust outlier test.
  - **TRP_020** — consecutive duplicate stops (dwell/timepoint doubling) are no longer
    treated as turnarounds.
  - **OPR_005** — now a relative service-frequency outlier per `route_type` baseline.
  - Also: STP_016, TRN_010, FPD_001.
- **CAL_007 / CAL_012** no longer double-report the same calendar gap.
- High-volume notices are aggregated to feed-level summaries where appropriate:
  STM_017, STM_050, TRN_007, STP_022, RTS_017.
- STP_029 threshold raised to 150 m, severity lowered to Medium. ARC_009 empty optional
  files downgraded from critical to info. OPR_015 fires only for bidirectional routes.
  STM_045 now respects `service_day_start_hour`.
- Rule messages enriched with `route_short_name` and trip departure time (EN/JA parity).
  `RULES.md` / `RULES.en.md` / `RULES.ja.md` regenerated (526 rules).

### Fixed
- Build warning on `service_day_start_hour` (useless comparison).
- `wasm:threads` build on Windows.
- Archive validation: ARC_017 coverage and an ARC_009 false positive.
- CI: `npm audit` blocking for production dependencies; tarpaulin LLVM engine to avoid
  a ptrace segfault.

### Internal
- Runtime emit-proof harness with a `coverage_debt` ledger proving every emittable rule
  actually fires, plus a static emit-coverage test that every canonical rule ID is
  referenced in production code (#5).

## [0.1.2] - 2026-06-16

⭐ **GTFS-JP (Japan profile) support.** Profile detection with a GTFS-JP badge, plus
10 GTFS-JP rules (JPN_001–010): ja-Hrkt (kana) readings for stop / route / trip /
agency names; `office_jp` / `agency_jp` / `routes_jp` foreign-key checks; and the
profile's mandatory `translations.txt`, fare files and `feed_info.txt`.

This release also closes the highest-impact coverage gaps found against MobilityData
v8 — STM_050 (missing timepoint value), STM_051/052 (forbidden Flex pickup/drop-off
types), FRQ_011 (overlapping frequency), SHP_028/029 (equal shape distance with
differing coordinates) — and adds a generator that keeps `RULES.md` / `RULES.en.md` /
`RULES.ja.md` in sync with the rule registry so the documented rule list can no longer
drift. The Fix page gains a collapsible rule-score summary (R9) and multi-select chip
filters (R2).

### Features
- Fully client-side GTFS validation and analysis in the browser (WebAssembly); uploaded
  feeds never leave the user's device.
- 518 validation and analysis rules across spec compliance, interoperability, quality,
  and operational analytics, each tagged with a rule code, class, and severity.
- Feed Calendar report section: feed validity window (from `feed_info`), actual service
  coverage with a coverage figure and a proportional timeline, and a report generation
  timestamp.
- Two independent scores (Publish / Quality) and a prioritized fix queue.
- Interactive map visualization for geographic findings (deviating routes, broken
  coordinates, unreachable stops, pathways, etc.).
- GTFS Flex and Fares v2 validation.
- Multilingual UI and reports: Turkish, English, Japanese.
- Export to HTML, CSV, JSON, PDF.

### Tooling / CI
- CI with Rust + TypeScript unit tests, E2E (Playwright), coverage gating, and a security
  audit (`cargo audit` blocking; `npm audit` reported, non-blocking).
- GitHub Pages deploy builds from source to guarantee the live site matches `HEAD`.

[Unreleased]: https://github.com/ttezer/gtfs-analyzer/compare/v0.1.3...HEAD
[0.1.3]: https://github.com/ttezer/gtfs-analyzer/compare/v0.1.2...v0.1.3
[0.1.2]: https://github.com/ttezer/gtfs-analyzer/releases/tag/v0.1.2
