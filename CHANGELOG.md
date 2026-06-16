# Changelog

All notable changes to this project are documented here.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

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

[Unreleased]: https://github.com/ttezer/gtfs-analyzer/compare/v0.1.2...HEAD
[0.1.2]: https://github.com/ttezer/gtfs-analyzer/releases/tag/v0.1.2
