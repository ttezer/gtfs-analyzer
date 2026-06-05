# Changelog

All notable changes to this project are documented here.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

Unreleased working state — no version has been tagged yet. Prior history is available
in the git log; entries below describe the current state of the application.

### Features
- Fully client-side GTFS validation and analysis in the browser (WebAssembly); uploaded
  feeds never leave the user's device.
- 474 validation and analysis rules across spec compliance, interoperability, quality,
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

[Unreleased]: https://github.com/ttezer/gtfs-analyzer/compare/main...HEAD
