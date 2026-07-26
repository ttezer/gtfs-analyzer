# Changelog

All notable changes to this project are documented here.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Added
- **`--class` filter (CLI).** Notices can be narrowed to one or more rule classes
  (`spec,interop,quality,analytics`), so a consumer can ask for the official GTFS Spec
  violations alone without post-processing the JSON.
- **`--min-severity` (CLI).** `--severity` matches one severity exactly; the new flag keeps
  that severity and everything worse, which is what a "critical and high" sweep needs.
- **`--fail-on` / `--fail-on-class` (CLI).** Exit 1 previously meant "any notice at all",
  including INFO — true for essentially every real feed, which made the exit code useless as
  a CI gate. These flags scope the failure to what the pipeline is actually gating on.
- **`rules` subcommand (CLI).** Lists the rule registry (`id`, `severity`, `class`,
  `authority_source`, `base_effort`, `blocks`, `title`) as text or JSON, so an integrating
  project can build its rule dictionary without scraping the docs.
- **`--version`, `--pretty`, `-o/--output` (CLI).** The binary had no version flag at all.
- **`--lang tr|en|ja` (CLI).** The pipeline emits its finding texts in Turkish, so the CLI —
  unlike the web UI — had no English or Japanese output at all. Notice titles, messages and
  remediations are now translated through the very dictionaries the UI uses, with the UI's
  fallback chain (requested language → English → the pipeline's Turkish). Rule ids, severities
  and classes stay machine-readable in every language. The dictionaries are derived from
  `ui/src/locales/{en,ja}.ts` by `npm run locales:export` and embedded in the binary; the
  locale files remain the single source of truth and `locale-parity.test.ts` fails on drift.
- **CLI test suite.** The crate shipped without tests; exit codes, filter semantics, the JSON
  envelope and the registry listing are now covered.

### Changed
- **BREAKING — the `--json` envelope is flat.** Output was the raw serde form of the
  `ValidateResult` enum, so consumers had to unwrap `{"Ok":{…}}` or `{"Fatal":{…}}`. It is now
  a flat object tagged by a `status` field (`"ok"` / `"fatal"`), with the validation result's
  fields at the top level. The enum representation is unchanged at the WASM/UI boundary.
- **BREAKING — `name_index` is no longer included in `--json` by default.** The stop, route and
  shape lookup tables (including full shape geometry) dominated the payload on large feeds
  while most automation never reads them. Pass `--include-name-index` to restore it.
- **BREAKING — `--summary` and `--json` are now mutually exclusive.** Passing both used to
  silently ignore `--summary`.

### Fixed
- **CLI filters no longer flip the publish verdict.** `--rule` / `--severity` recomputed R1
  over the filtered subset, so hiding the critical SPEC notices reported `publishable: true`
  for a feed with publish blockers — while the R5 scores, left untouched, still showed the
  penalty. Filters are display-only now: R1 and R5 always describe the whole feed, and the
  active filter is disclosed in the output (`filtered` in JSON, `filter:` in the summary).

## [0.6.0] - 2026-07-17

> **Scores are not comparable to 0.5.0.** Several changes move a feed's numbers even though
> the feed is unchanged: false positives were removed from five rules and two rules were
> regrouped. Re-baseline any Golden snapshots after upgrading.

This release is the result of running 250 feeds sampled across 71 countries from the
MobilityData catalogue and comparing every finding against MobilityData's own report for the
same snapshot. Our test corpus had been three feeds — BART, Tokyo Toei, TriMet — and every
bug below is one that corpus could not surface: they need non-ASCII text, minute-rounded
times, a repeated stop name, a `", "` separator, or a multi-agency feed.

### Added
- **`validate_with_today` (WASM).** The browser evaluated calendar rules against the machine's
  clock, so a run could not be reproduced later and a diff mixed code changes with the date
  shift. The date can now be supplied explicitly, mirroring the CLI's `--today`. `validate`
  delegates to it, so there is one code path and no behaviour change.

### Fixed
- **Valid feeds were rejected outright when a header read split a UTF-8 character.** K1 reads
  only the first 8 KB of `trips.txt`, `stop_times.txt` and `calendar_dates.txt`; when that cut
  landed inside a multi-byte character, the truncated tail was read as invalid encoding and a
  required file turned it into a fatal error — the feed produced no analysis at all. Only feeds
  carrying non-ASCII text in those files were affected, and only when the byte offset fell
  unluckily. Found by running 250 feeds from the MobilityData catalogue: Japanese, Lithuanian
  and Thai feeds were rejected while MobilityData reported two of them with zero errors.
- **`FLG_002` reported false criticals for valid Fares v2 networks.** A `network_id` may be
  declared in `networks.txt`, in `routes.txt`, or in `route_networks.txt`; only the first was
  recognised, so feeds declaring networks the other way had every `fare_leg_rules` reference
  flagged as an undefined network. `route_networks.txt` is now parsed rather than merely
  detected. On TriMet this removed all 7 criticals and raised the Publication Score from 89.3
  to 100.
- **`STM_012` called a stop pair impossible on a fixed 1 km distance.** When two stops share a
  whole-minute timestamp the real travel time is unknown — minute rounding hides anything from
  0 to 59 seconds — yet a bus covering 1.1 km within that minute only needs 66 km/h. The
  distance must now be unreachable even if a full minute had elapsed
  (`max_speed_kmh(route_type) / 60`), so the check tightens for slow modes (cable car: 0.5 km)
  and relaxes for fast ones (rail: 5 km). One corpus feed drops from 42 findings to the single
  29.4 km jump that MobilityData also reports.
- **`TRP_020` flagged headsigns that correctly name the last stop.** The rule excluded the
  terminal by `stop_id` but matched candidates by name, so a trip was flagged whenever an
  earlier stop shared the terminal's name — separate platform records carry different ids. One
  feed drops from 9,162 findings to 171 (MobilityData: 19); on Tokyo Toei all 156 findings were
  this false positive and are now gone.
- **`RTS_019` treated different operators' route numbers as duplicates.** Two agencies each
  running a route "10", or a bus "10" beside a tram "10", are ordinary — a passenger tells them
  apart by operator or mode, and GTFS scopes name uniqueness to one `route_type` under one
  `agency_id`. Both are now part of the key; one corpus feed drops from 32 findings to 3,
  matching MobilityData exactly.
- **`STP_030` counted entrances as a station's children.** `parent_station` is also written by
  entrances and generic nodes, but a vehicle only stops at a platform, so a station holding six
  entrances and no platform was silently accepted. Only `location_type` 0 or empty now counts,
  and the message says the station has no platform instead of claiming nothing references it.
- **`RTS_023` ignored `route_desc` that repeats the short name.** It compared only against
  `route_long_name`; MobilityData checks both names. On one corpus feed all 436 of its findings
  were short-name matches and we reported none. The rule and its title in all three languages
  now say `route_desc` repeats *a route name*, and the message names the field that matched.

### Changed
- **`STM_014` findings are grouped per route, direction and segment.** A single bad segment
  produced one finding per trip crossing it — on TriMet, 497 findings all pointed at the same
  two segments. Each finding now carries the affected trip count, a trip sample and the speed
  range. Thresholds, severity and detection are unchanged; only presentation.
- **`DQ_016` reports one summary per file instead of one finding per row.** Leading and
  trailing whitespace is a single producer habit — most often writing `", "` as the separator —
  and it then appears on every row of every file. One corpus feed produced 2,361,873 findings,
  98.6% of its total, against MobilityData's 12, which is the volume that exhausts browser
  memory. The detection is unchanged and still correct (RFC 4180 makes the space part of the
  field; MobilityData's parser trims it): each finding now carries the affected row count and
  the columns involved. That feed drops to 7 findings and its total from 2.4M to 43k.

### Internal
- `RULES.md` / `RULES.en.md` / `RULES.ja.md` are generated from the rule registry, but nothing
  enforced regeneration and they had drifted. A test now ties both together.
- `ARC_029` (decompression guard) is proven end-to-end: a zip bomb reaching the validator
  returns the expected fatal, and a legitimate small high-ratio file does not trip the guard.

## [0.5.0] - 2026-07-11

> **Scores are not comparable to 0.4.0.** This release continues the authority-based
> reclassification started in 0.4.0: more rules moved between classes, so a feed's Overall
> and Publication scores can shift even though the feed is unchanged and detection is
> identical. Re-baseline any Golden snapshots after upgrading.

### Added
- **Command-line interface.** A new `gtfs-cli` crate ships the `gtfs-analyzer` binary:
  `gtfs-analyzer validate feed.zip` with `--json`, `--summary`, `--rule <ID>`,
  `--severity <level>`, `--config <file>`, `--today <YYYYMMDD>`. Exit codes: `0` no notices,
  `1` notices present, `2` fatal / config error. It runs the same validation core as the web
  app — no separate logic.

### Changed
- **Authority-based classification completed.** Every `Spec` rule card must now cite an
  explicit `gtfs.org` field anchor, enforced by a test gate (the earlier warn-mode was
  retired). Rules lacking an explicit GTFS Schedule normative basis were reclassified off
  `Spec`/`Interop`:
  - Interop rules matching only project-specific behaviour (no MobilityData / Google /
    regional parity) moved to `Quality` or `Analytics`.
  - `XFL_020` / `XFL_021` (transfers cross-file consistency): `Spec` → `Quality` — no explicit
    `transfers.txt` normative clause and no verified external parity.
  - `TRP_017` (frequencies trip missing `stop_times`): `Interop` → `Quality` — MobilityData's
    `unused_trip` is a different condition, so there is no exact-notice parity.
  - Google Transit checks are grouped under a dedicated `GoogleTransitInterop` authority.
  - Detection is unchanged throughout — the same issues are still reported; only class labels
    and their scoring weights moved.
- `ATR_009` was mislabelled "attribution_phone invalid"; it actually flags rows where more
  than one of `agency_id` / `route_id` / `trip_id` is set. Registry title, rule message, and
  the en/tr/ja locales were corrected (no behaviour change).
- Interop rules no longer appear in the R1 publishability report (view-metadata cleanup).

### Fixed
- On the file map, shape geometry for trip-context rules (e.g. STM_014, OPR_007, STM_017) is
  now fetched on demand in deferred (large-shape) mode, so the route line renders alongside
  the stop pins instead of pins alone.

### Internal
- Classification audit ledgers are kept out of the public repository.

## [0.4.0] - 2026-07-09

> **Scores are not comparable to 0.3.x.** This release recalibrates how rules are
> classified and how the Publication Score is computed. A feed's numbers can move
> substantially even though nothing about the feed itself changed. Re-baseline any
> Golden snapshots after upgrading.

### Changed
- Rule classification is now authority-based. The `Spec` class is reserved for cases the
  official GTFS Schedule Reference explicitly requires, forbids, or invalidates (required /
  conditionally required / conditionally forbidden fields, enum values, foreign keys,
  uniqueness, format constraints). Rules previously labelled `Spec` on weaker grounds
  (MobilityData parity, best practice, or derived/project-specific checks) were reclassified
  to `Interop`, `Quality`, or `Analytics` to match their real authority. Detection is
  unchanged — the same issues are still reported; only the class label and its scoring weight
  moved. The overall score (Spec 40% / Interop 30% / Quality 20% / Analytics 10%) shifts
  accordingly.
- The Publication Score and the R1 publishability decision now gate on `Spec + Critical`
  only — the official GTFS specification gate. `Interop` findings (even Critical/High) no
  longer block publication or lower the Publication Score; interoperability readiness is
  reported separately through the Interop Score and R8. This also removes the artificial
  Publication-Score drop that occurred when a rule moved between the `Spec` and `Interop`
  classes.

### Removed
- The "conditionally publishable" state (R1 conditional blocker / `ConditionalBlocker`
  label). A feed is now either spec-publishable or not; consumer-compatibility warnings are
  surfaced via the Interop Score, R8, and the R9 `interop` label.

## [0.3.1] - 2026-07-02

### Fixed
- SHP_005 (`shape_dist_traveled` decreasing) now evaluates values in `shape_pt_sequence`
  order instead of file-row order. GTFS does not require `shapes.txt` rows to be pre-sorted
  by sequence, so feeds that list shape points out of sequence order (valid) produced
  spurious CRITICAL errors — e.g. all 5,774 on the Athens feed (mdb-3220) were false
  positives (file order shows 5,774 decreases; sequence order shows 0, matching
  MobilityData). The check moved to the stage that already sorts shape points by sequence;
  no other rule was affected. On Athens: overall 79.2 → 84.8, publish/spec 86.2 → 100.

## [0.3.0] - 2026-07-02

### Added
- Load a GTFS feed directly from a URL (#45): the browser fetches the feed client-side —
  no backend, and the feed data stays on the device. Works for CORS-enabled hosts such as
  the Mobility Database catalog; other hosts fall back cleanly to download-and-drop.
- Run-to-run comparison page (#21): compare the currently analyzed feed with an older
  Golden JSON, including fixed/new/increased/decreased rules, score, severity, class,
  feed-size, date-range, and normalized notice-density deltas.
- Golden JSON v4 with an exact UTC `generated_at` timestamp, file row/byte counts,
  feed metrics, analysis settings, rule metadata, and severity/class totals. Golden
  v1–v3 files remain importable with clearly marked limitations.

### Changed
- The downloaded Golden snapshot is byte-deterministic again for git-tracked regression
  baselines (#42): it omits the per-second `generated_at` while keeping the day-granular
  `validate_date`; the in-app comparison keeps its live timestamp.
- CAL_006 (all weekday columns are 0) and RTS_020 (route_url equals agency_url) are now
  Info, matching MobilityData; CAL_006 is reframed as a dates-only hint rather than
  "service never runs".
- The Memory64 package (`pkg64`) is now optimized with `wasm-opt` (~2.37 MB → ~2.02 MB).

### Fixed
- STP_021 no longer flags valid Entrance/Exit stops. It now targets Boarding Areas
  (`location_type=4`) that lack a platform parent, removing high-severity false positives
  (e.g. 132 on the BART feed) that skewed the quality score.
- Calendar-analytics over-fire on feeds that model one operational calendar as many
  service-id variants (#30): CAL_007/CAL_012 gaps and CAL_013 expiries are aggregated by
  signature, so each is reported once with the affected services listed.

## [0.2.0] - 2026-06-28

### Added
- Automatic dual-runtime selection: browsers with WebAssembly Memory64 use the
  wasm64 serial engine; other browsers fall back to wasm32 threaded or serial mode.
- Active engine badge and diagnostic export metadata.
- Debug overrides: `?wasm32=1`, `?wasm64=1`, and `?serial=1`.

### Performance
- Large feeds can exceed the wasm32 4 GB linear-memory ceiling when the browser
  supports Memory64. A 250 MB feed with 34.5 million stop-times completed at about
  5.3 GB.
- wasm32 and wasm64 output parity is checked with a deterministic Golden snapshot.

## [0.1.4] - 2026-06-24

This release adds the **Interactive GTFS File Map** and substantially improves the
reliability of large feeds under WebAssembly. Before 0.1.4, VBB-class feeds (e.g.
VBB Berlin, ~282k trips / 6.3M stop_times) either ran close to the 4 GB wasm32
memory ceiling or failed during result serialization; 0.1.4 brings peak memory down
to roughly **2.74 GB**, an improvement of about **1.3 GB** of headroom.

### Added
- **Interactive GTFS File Map** that combines GTFS file relationships with the real
  analyzer findings for the loaded feed.
- Per-file finding count, severity, row count, and missing/clean status.
- Direct navigation from a file into the filtered Detail and Fix views.
- File-type icons, dark theme, and a mobile layout; non-spec files are shown
  separately.
- Turkish, English, and Japanese UI strings for the File Map.
- Diagnostic information showing the last completed pipeline stage on validation
  errors and timeouts.

### Performance
- Reduced WASM peak memory on the VBB Berlin test feed from roughly **4.0 GB (or a
  serialization failure)** to about **2.74 GB**.
- `CompactStopTime` row size reduced from **168 to 96 bytes** (`size_of`): dead flag
  fields were removed and `stop_headsign` is boxed.
- Removed about **454 MB** of unused capacity in the `stop_times` row buffer by
  reserving from the actual line count instead of `text.len() / 40`.
- Removed the second, borrowed copy of `trip_stop_set` that was built in the K4
  cross-reference stage.
- `trip_stop_set` is now freed immediately after K4 so K6 allocations can reuse that
  space, lowering peak memory.
- Added a large-feed mode to the name index: above a deterministic trip-count
  threshold, per-trip and map-only label fields are skipped (notices fall back to raw
  IDs) to avoid serialization OOM; small and normal feeds are unchanged.
- Reduced several high-volume intermediate notice emitters (TRP_020, SHP_022,
  STP_022, PTH_008).

### Fixed
- Large GTFS feeds that previously approached the wasm32 4 GB limit or ran out of
  memory can now be analyzed more safely.
- **STM_014** (fast travel) false positives on feeds using extended European
  `route_type` codes (S-Bahn, U-Bahn, tram, regional rail) — speed thresholds are now
  mapped to the correct vehicle category, so legitimate high-speed rail is no longer
  flagged.
- The File Map no longer disappears entirely when a severity filter is applied.
- File and group colors are now correct in the dark theme.
- Improved the readability of link and field labels in the GTFS file relationship view.

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

This release also closes the highest-impact validation coverage gaps — STM_050
(missing timepoint value), STM_051/052 (forbidden Flex pickup/drop-off
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
