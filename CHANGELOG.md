# Changelog

All notable changes to this project are documented here.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

> **Scores move on feeds with a missing stop.** A `stop_id` used in `stop_times.txt` but absent
> from `stops.txt` was penalised twice; it is now counted once. Feeds without that error are
> unaffected. Re-baseline Golden snapshots that contain it.

### Fixed
- **rider_categories: `eligibility_url` was read under a name that does not exist.** The parser
  looked for `rider_category_eligibility_url`, which appears nowhere in the spec, so every valid
  `eligibility_url` value was silently dropped. K1 already knew the correct name, so the two
  stages disagreed. The field is now read and stored under its official name.
- **RCT_004 is no longer a Spec rule.** `min_age` and `max_age` are not part of
  `rider_categories.txt`; the official file carries only `rider_category_id`,
  `rider_category_name`, `is_default_fare_category` and `eligibility_url`. Some producers emit
  the two age fields as an extension and reading them is harmless — treating them as a
  specification violation was not, since the Spec class feeds the publish-score gate. The rule
  keeps its checks but moves to Quality, with its authority set to ProjectQuality.
- **Valid GTFS-Flex stop_times rows were reported as broken.** The spec lets a row identify its
  location with `stop_id`, `location_group_id` or `location_id`, requiring `stop_id` only when
  the other two are absent. K1 required `stop_id` unconditionally, listed none of the six
  official Flex columns as known, and STM_006 fired on every empty `stop_id` — while the K2
  parser had been reading those columns all along, so the two stages disagreed about the same
  row. A correct Flex feed collected ARC_017, ARC_025 and STM_006 on data the spec endorses.
  All three now follow the conditional rule.
- **FPD_002 no longer rejects negative fare amounts.** The spec says of
  `fare_products.amount`: "May be negative to represent transfer discounts. May be zero to
  represent a fare product that is free." Every negative value was reported as a critical Spec
  error, so a feed modelling transfer discounts correctly lost publish score for valid data.
  The rule now checks only that the field is present and numeric.
- **A Flex-only feed could not be opened at all.** `stops.txt` sat in the unconditional required
  list, so a feed that defines its service through `locations.geojson` zones — which the spec
  explicitly allows, calling `stops.txt` "Optional if demand-responsive zones are defined in
  locations.geojson" — was rejected with a fatal ARC_004 and produced no findings whatsoever.
  The requirement is now conditional. A feed carrying neither `stops.txt` nor `locations.geojson`
  is still fatal, since nothing then says where service is provided.

### Changed
- **Validation output is now byte-identical across runs.** The July 27 work fixed the part
  that changed *content* — which finding survived deduplication — but the serialized JSON still
  differed run to run, because `Notice.details`, the thirteen `NameIndex` lookups and
  `capped_totals` were `HashMap`s and Rust seeds each one differently per process. Only key
  order moved, never a value, so scores and reports were already stable; but it meant a golden
  snapshot could not be compared with `cmp` or a checksum, and no "output is bit-identical"
  gate could be built for future parallel work. All of them are now `BTreeMap`. Verified on
  five feeds: five consecutive runs of TCDD and three each of BART, TriMet, Tokyo and VBB are
  byte-identical, with content unchanged against the golden snapshots. Measured cost on VBB:
  median wall clock 7.12 → 7.17 s and peak RSS 2,317 → 2,340 MB. The determinism test now
  compares the whole serialized result rather than a subset of notice fields.
- **OPR_019 and OPR_020 now identify the trip, not just the calendar.** Both treated a route
  with two or more services active on the same date as an override conflict. That is normal
  operation: trains running in opposite directions use separate `service_id` values, and extra
  workings on the same route carry different train numbers. A conflict is now reported only
  when the *same* operational trip is active from two services on one day, identified by
  `direction_id`, `trip_short_name`, first departure time and the ordered stop sequence — the
  case the rules were written for, where a base calendar and its override both produce the same
  service. Signatures are computed only for routes with more than one service.
- **SHP_009 now reports each crossing location once, not once per shape.** Same cause as
  SHP_020: a switch or station approach appears in both directional shapes of a line, so
  emitting per shape counted one physical crossing several times. Measured on a rail feed,
  9 notices were 4 locations. Grouped by coordinate, represented by the smallest `shape_id`,
  with the count in `details.repeated_shapes`; the saturation threshold counts locations.
- **SHP_020 now reports each repeated location once, not once per shape.** Station throats and
  switch geometry appear in both the outbound and inbound shape of the same line, so emitting
  per shape counted the same physical point over and over: one rail feed showed 28 notices that
  were only 8 distinct coordinates. Findings are now grouped by coordinate, represented by the
  lexicographically smallest `shape_id`, with the repeat count in the message and in
  `details.repeated_shapes`. The saturation threshold counts distinct coordinates too.
- **CLD_007 no longer fires on feeds that only use calendar_dates.txt.** A feed may define its
  calendar entirely through `calendar_dates.txt`; GTFS allows this explicitly. In that shape
  every active day is an exception, so the rule's "more exceptions than half the active days"
  test was mathematically guaranteed to trigger and reported nothing about deviation from a
  base schedule — it simply read back the producer's modelling choice. The rule now requires
  `calendar.txt` to exist, matching the guard ARC_009 already uses. A missing base schedule is
  still reported by ARC_008 and the CAL rules.
- **CAL_010 and CAL_017 now read the feed's own context.** Both judged a service against an
  absolute rule that ignores what the feed is publishing. CAL_010 warned whenever a service
  ran seven days or fewer, which is meaningless in a feed covering only seventeen days: a
  train that runs on five of them is a normal timetable, not a stub. The day threshold now
  applies only when the feed publishes a window of 30 days or more; below that the test
  becomes a ratio, and a service active on less than 20% of the window is reported. CAL_017
  warned about every service starting in the future, so a valid timetable beginning in two
  days was an error while the rest of the feed ran normally. It now fires only when *no*
  service in the feed has started yet, which is the case it was written for. A service that
  has already expired counts as started; staleness is CAL_013's job. CAL_017 also emitted in
  hash order and is now sorted by `service_id`.
- **STM_045 and OPR_001 now read route_type.** Both applied a threshold calibrated for
  street-running transit to every mode. STM_045 rejected departure times past
  `24 + service_day_start_hour`, but long-distance rail legitimately writes 30:37, 33:56 or
  38:10 — GTFS encodes times after midnight as 24:xx and later, and folding them back into a
  24-hour day breaks the chronology of the trip. Rail trips now use
  `service_day_window_hours_rail` (default 48); genuine typos such as 99:00 are still caught,
  and STM_028 measures total trip length separately. OPR_001 warned about any gap over 240
  minutes, which is meaningless on intercity rail: nobody expects an hourly train between
  Ankara and Sivas, where a 580-minute gap is the timetable working as intended. Rail routes
  now use `max_headway_warning_min_rail` (default 720). OPR_005 is deliberately untouched — it
  already compares each route against the median and MAD of its own route_type and has no
  fixed threshold. Notices report the threshold that was applied.
- **VAT_002 and VAT_007 no longer ask for transfers that GTFS already implies.** Both rules
  suggested adding `transfers.txt` records for any stop several routes touch. But a transfer at
  the same `stop_id` is implicit — consumers already know passengers can change there, and
  `transfers.txt` exists to describe connections *between* stops, minimum times, or forbidden
  transfers. At a terminal where every train uses one `stop_id`, there was nothing to declare.
  Both rules now require a genuine gap: another stop within `max_transfer_distance_m` serving a
  route this stop does not, with no transfer recorded. Stops with a `parent_station` are also
  skipped, since the complex is already modelled — previously only the station itself was
  excluded, not its platforms. Both remain INFO and neither affects any score.
  VAT_007 additionally listed its routes in hash order and truncated to five *before* sorting,
  so which five appeared could change between runs; the list is now sorted first.
- **SHP_027 has been removed** (rule count 542 → 541). It reported a shape serving more than
  one stop pattern as a possible misassignment, but that reads the GTFS model backwards:
  `shapes.txt` describes the physical alignment while `stop_times.txt` describes where a train
  stops. Trains sharing a corridor and calling at different stations are the normal case, not
  an error, and narrowing the rule to mutually divergent patterns still flagged them. The one
  genuine signal underneath — a stop that is not on the shape it was attached to — is measured
  directly and better by GEO_009 and SHP_012. Our own measurement had already shown the rule
  was mostly firing on legitimate feeds: across 14,084 VBB shapes, 83% of SHP_012 findings had
  no SHP_027, and multi-pattern shapes had no more distant stops than single-pattern ones.
  Consumers keying on this id should drop it; nothing replaces it.
- **VAT_003 now counts timetables, not trip records.** Some producers write the same timetable
  as a separate `trip_id` for every operating date. VAT_003 emitted one notice per trip, so
  four real timetables surfaced as forty findings — and the damage was not only cosmetic: ten
  identical copies made up a quarter of their group and inflated the MAD, which masked the
  outliers the rule exists to find. Records sharing a pattern, a departure time and a duration
  are now collapsed into one timetable **before** the statistics run, and one notice is emitted
  per outlier timetable, carrying `details.duplicate_trips` and naming the repeat count in the
  message. The representative record is the lexicographically smallest `trip_id`.
  The ≥5 gate now counts distinct timetables, so a pattern with forty trips but only four
  timetables produces nothing: four observations cannot support a robust-z claim, and the old
  behaviour manufactured confidence by counting the same evidence ten times. Feeds that do not
  date-expand are unaffected.
- **Shape geometry rules now use wider thresholds on rail routes.** Six rules — GEO_006,
  GEO_007, GEO_009, SHP_012, SHP_014 and SHP_024 — measured every mode against thresholds
  calibrated for street-running transit, and flagged legitimate rail geometry as an error.
  Two assumptions break on rail. A station's `stop_lat`/`stop_lon` marks the platform or
  building centre while the shape follows a single track centreline, so a 100 m tolerance
  rejects a correct feed; and intercity track is legitimately simplified over long straight
  runs, where 13.4 km between consecutive shape points is real geometry rather than a gap.
  Rail shapes now use two new settings, `stop_far_from_shape_m_rail` (default 200 m) and
  `max_shape_jump_km_rail` (default 30 km); the street-running defaults are unchanged at
  100 m and 10 km. A shape counts as rail when **any** route using it is rail
  (`route_type` 2, 12 or 100–117, the same definition `max_speed_rail_kmh` and
  `max_trip_duration_hours_rail` already use), because a false positive here — calling
  correct geometry an error — costs more than a missed one. Notice messages report the
  threshold that was actually applied. Feeds containing rail routes will see fewer notices
  from these six rules; re-baseline Golden snapshots for them.
- **stop_times is now parsed in parallel on native targets.** It was the single largest stage
  left — 2.8 s of a 8.4 s run on VBB, with most cores idle while it ran. The ZIP entry is now
  decompressed once, split at line boundaries that are then aligned to a trip change, and the
  pieces are parsed concurrently before being merged. Aligning to trip boundaries matters:
  stop_times is grouped by trip, so no trip spans two pieces and STM_036's ordering state never
  has to be reconciled across a split. Measured on VBB (5.68M rows): K2::stop_times 2,820 →
  ~1,405 ms (2.0×), K2-validate 4,665 → ~2,467 ms, wall clock ~8.4 → ~6.9 s, peak RSS 2.50 →
  2.73 GB. Output is byte-identical to the serial path. WebAssembly keeps streaming row by row,
  as before.

### Fixed
- **Validation output was not deterministic.** The same binary, given the same feed and the same
  `--today`, produced different results from one run to the next: on VBB, 829 notices differed
  between two runs of the identical build. This predates the parallel work — it reproduced on a
  fully serial commit — and it quietly undermined golden snapshots, MobilityData parity
  measurements and any A/B comparison.

  The chain: several rules emit from a `HashMap`, whose iteration order differs per process.
  `dedup` then picked its keep-first representative from that order, using `sort_unstable_by` —
  whose own comment claimed to sort "stably" — over a key that could not tell two findings apart
  (`GEO_006` puts the segment only in `observed_value`; `line` is `None`). So which of a shape's
  two jump segments survived was decided by hash seeding.

  Fixed at each link: `dedup` sorts stably; notices are renumbered *after* dedup so ids follow
  the sorted order rather than emission order; `OPR_007` selects its duplicate stop by smallest
  `stop_id` instead of `HashMap::find`; and the `OPR_001`, `OPR_003`, `OPR_024` and `SHP_010`
  emission sites sort their keys before iterating. Verified on three feeds: VBB, TCDD and TriMet
  now produce byte-identical JSON across runs. A regression test validates the same feed four
  times in one process, which is stricter than separate runs because each `HashMap` instance is
  seeded independently.

  Two serialisation-order gaps remain, tracked separately: `Notice.details` and `NameIndex` are
  `HashMap`s, so serde writes their keys in varying order. That affects byte output only, not
  which findings are produced.

### Changed
- **`STM_028` gets a separate, higher threshold for rail — and reports it as info.** A single
  24-hour limit was wrong for long-distance rail: on the TCDD feed it fired six times and was
  wrong every time. Ankara–Kars runs 26:26, Ankara–Tatvan 26:37 and Kurtalan–Ankara 27:25 — real
  timetables, confirmed by the feed's author. Rail route types (2, 12 and the extended 100–117
  range) now use `max_trip_duration_hours_rail`, defaulting to 48 hours, and the notice drops to
  Info severity there: a train exceeding a day is worth seeing, not worth alarming over. Urban
  types keep the 24-hour limit at High, where that duration really does indicate a data error —
  verified in the same measurement: TCDD went 6 → 0 while VBB's single non-rail 27:25 finding
  stayed High. The remediation text is now one string covering both cases, because the locale
  dictionaries hold a single remediation per rule and a conditional variant could not be
  expressed under `--lang en/ja`.
- **CSV fields are borrowed instead of copied into owned `String`s.** Each parsed row built a
  `Vec<String>`, copying every field, and then wrapped those in a `Vec<Cow<str>>` — twelve
  allocations per row where one suffices. The comment justified it with "borrowing is not
  possible because raw_fields is reused on the next iteration", but the borrow ends when
  `process` returns, well before the buffer is refilled. Fields are now `Cow::Borrowed` on the
  common path, falling back to owned only for the rare invalid-UTF-8 line. Measured on VBB over
  three warm runs: K2::stop_times ~3,577 → ~2,916 ms, wall clock ~9.2 → ~8.7 s. Output
  byte-identical.
- **The CSV reader no longer calls `read` once per byte.** `ZipCsvReader::next_byte` pulled a
  single byte at a time through `BufReader::read(&mut [0u8; 1])` — roughly 402 million calls for
  stop_times.txt alone on VBB. The reader now owns its 64 KB window and advances an index,
  touching the underlying stream once per window. The CSV state machine is untouched. Measured
  on VBB across three warm runs: K2::stop_times 3,903 → ~3,577 ms, K2-validate 5,117 → ~4,665 ms,
  wall clock ~10.2 → ~9.2-9.5 s. Output byte-identical.
- **The CSV reader reuses its field buffers.** `ZipCsvReader::next_record` allocated a fresh
  `Vec` per field and dropped its capacity through `mem::take` on every delimiter — about 57
  million allocations on a feed the size of VBB (5.68M rows × ~10 fields). The buffers are now
  cleared and refilled in place. Measured on VBB: K2::stop_times 4,859 ms → 3,903 ms, wall clock
  11.14 s → 10.23 s, and peak RSS drops 2.85 → 2.55 GB. Field contents, field count and order
  are unchanged; the notice output is byte-identical.
- **K6 analytics now runs in parallel by default on native targets.** The machinery had been in
  place but behind an off-by-default feature flag, so every CLI run evaluated the fifteen
  independent K6 checks on a single core. Measured on VBB (5.68M stop_times, 602 MB expanded):
  K6 4,889 ms → 2,546 ms (1.92×), wall clock 13.07 s → 11.14 s. The notice output is
  byte-identical — the checks already merged in canonical order and renumbered against a single
  counter, which is what made the flag safe to flip. Peak RSS rises 2.39 → 2.85 GB.
  WebAssembly is unaffected: `crates/wasm` now pins `default-features = false`, so the browser
  keeps its serial path and its own `threads` feature governs parallelism there. On small feeds
  the change is invisible (TriMet: 4.76 → 4.68 s) because K6 is not the bottleneck at that size.
- **`TRP_025` no longer fires when `TRP_029` does.** TRP_029 reports that *no* trip declares
  `wheelchair_accessible`; TRP_025 reports that *over 80%* of them don't. 100% is above 80%, so
  a feed with no accessibility data at all produced both findings for one fact. TRP_025 is now
  scoped to 80–99% and TRP_029 keeps the 100% case, so each threshold reports once.

### Removed
- **Five more `XFL_*` rules that restated a foreign-key violation another rule already
  reported.** `XFL_005` turned out not to be an isolated mistake but a family pattern: of the
  nine XFL rules emitting a feed-level summary, six were summarising a set that a per-row rule
  had already flagged, under the same severity and class.

  | Removed | Kept | Shared condition |
  |---|---|---|
  | `XFL_001` | `TRP_003` | `trips.service_id` not in calendar/calendar_dates |
  | `XFL_003` | `TRP_004` | `trips.shape_id` not in shapes |
  | `XFL_004` | `FRL_002` | `fare_rules.route_id` not in routes |
  | `XFL_007` | `RTS_002` | `routes.agency_id` not in agency |
  | `XFL_009` | `STP_015` | `stops.level_id` not in levels |
  | `XFL_010` | `FRQ_001` | `frequencies.trip_id` not in trips |

  `XFL_007` and `RTS_002` even carried the same title. On a feed holding one of each violation
  the duplication cost 4 spurious publish blockers and 13.3 points of publication score
  (58.1 → 71.4, measured). The per-row rule survives in every case: it reports the offending
  line and names the entity, where the XFL summary only restated the same set feed-wide.

  `STP_015` is raised from Medium to Critical: `XFL_009` reported the same violation as
  Critical, so leaving `STP_015` at Medium would have quietly dropped `level_id` integrity out
  of the R1 publish gate. Blocks entries move to the surviving twin (`OPR_016` to `TRP_003`), and
  the blocks lists naming a removed rule now name its twin. Rule count: 542 → 536.
- **`XFL_005` — it reported the same violation as `STM_002`.** Both fired on a `stop_id` used in
  `stop_times.txt` but missing from `stops.txt`, and not by coincidence: the `bad_stop_ids` set
  was built inside the `STM_002` loop and handed to `check_xfl`, so XFL_005's input *was*
  STM_002's output. Both were Critical/Spec, so one missing stop produced two R1 publish
  blockers, two penalties against the publication score, and two separate entries in the R9 fix
  queue — while a single edit to `stops.txt` closed both. On a real feed with one missing stop
  this cost 6.4 points of publication score. `STM_002` is the surviving rule: it carries the
  line number and aggregates per distinct `stop_id`, whereas XFL_005 only summarised the same
  set feed-wide. XFL_005's `blocks` entries (`STP_012`, `DQ_005c`) moved to `STM_002`, and the
  `ARC_004` blocks list now names `STM_002` in its place. Rule count: 543 → 542.

## [0.7.0] - 2026-07-26

> **Scores are unchanged.** No rule, threshold or pipeline stage was touched in this release:
> `core`, `rules`, `pipeline`, `config` and `wasm` are byte-for-byte the 0.6.0 logic. A feed
> validated with 0.7.0 produces exactly the 0.6.0 numbers, so no Golden re-baseline is needed.

This release is about the command-line interface. The CLI existed as a thin wrapper — one
subcommand, six flags, no tests — which was enough to run a validation by hand but not enough
to build on: its exit code could not gate CI, its findings could not be narrowed to the
official GTFS Spec, its JSON forced consumers to unwrap an enum, and it had no English or
Japanese output at all. The web UI, meanwhile, had all of that.

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
- **Prebuilt CLI binaries.** Building from source was the only way to get the CLI, which rules
  it out for anyone without a Rust toolchain. Pushing a `v*` tag now builds Linux, macOS and
  Windows binaries and attaches them to a GitHub Release. The release is assembled as a draft
  and only published once every platform succeeds, and each binary must report the tag's
  version before it is uploaded.
- **Feed from stdin (CLI).** `validate -` reads the ZIP from standard input, so a feed can be
  piped straight from `curl` without a temporary file.
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
