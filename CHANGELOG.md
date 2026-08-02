# Changelog

All notable changes to this project are documented here.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

> **Scores move on feeds with a missing stop.** A `stop_id` used in `stop_times.txt` but absent
> from `stops.txt` was penalised twice; it is now counted once. Feeds without that error are
> unaffected. Re-baseline Golden snapshots that contain it.

### Added
- **A rule id used for two unrelated checks is now reported.** ATR_006 emitted both the
  `is_authority` enum check and the `attributions.route_id` foreign-key check, and ATR_007 did
  the same for `attribution_url` and `trip_id`; the registry describes one of each, so a feed
  with a dangling route reference received a finding titled "is_authority geçersiz" whose
  remediation told it to fix a route id (fixed below). Nothing could see this: the emit proof asks whether a
  rule can fire, not whether what it emits is what the rule says it is, and a second emit site
  added to an existing id was invisible to every check.

  `emit_identity.rs` scans the pipeline source and records, per rule id, the field names each
  emit site mentions. Sites that share no field at all are the signal: a rule with branches for
  a missing and an invalid value names the same field twice, while ATR_006 names
  `is_authority` in one place and `route_id` in another. Overlapping sets are ignored, which
  takes the list from 32 candidates to 12, and those twelve are adjudicated in the test's own
  documentation — nine are legitimate, two are the collision above, and STP_003 declares its
  double duty in its own title. A second check, comparing the registry title against the fields
  the emit site names, was built and discarded: it produced 47 findings, almost all noise from
  filenames in titles and neighbouring code, and caught nothing the field-set check missed.
- **The gap between what the specification requires and what we check is now measured and
  frozen in a ledger.** The two gates added before this one ask whether our findings are
  *right*; neither asks whether they are *enough*. `spec_fields.json` now carries each field's
  Type and Presence, plus the file's primary key, and those two columns are the specification's
  own vocabulary for its normative provisions: `Required` and `Conditionally Required` state
  presence, `Enum` states a domain, `Foreign ID` states referential integrity, `Unique ID`
  states uniqueness, and the format and numeric types state a shape. `Optional` and
  `Recommended` state nothing and are counted as no provision — the distinction STM_040 turned
  on. Of the 218 fields, 192 carry at least one provision, and 32 of those have no Spec notice
  anchored to them anywhere in the emit-proof corpus. `spec_coverage_ledger.txt` records them,
  so a field added by a future revision of the specification, or a gap opened by removing a
  rule, shows up as a failing test instead of going unnoticed.

  The ledger is deliberately labelled as a lower bound rather than a list of missing rules. A
  rule that checks several fields registers only the one its fixture breaks — CAL_002 validates
  all seven day columns but the fixture corrupts `monday` — and a rule that reports no field at
  all, like RTS_003 for "one of route_short_name or route_long_name", anchors nowhere. Entries
  are marked `[yalnız: …]` when a rule does measure the field but is not classed `Spec` —
  `agency_phone` is validated by AGN_007, which is deliberately Quality — because those are not
  gaps at all but classification questions, and eleven of the thirty-two entries are of that
  kind. Entries are marked `[denetim-yok]` when the field name appears in no check stage at all,
  which is the subset that is certainly uncovered: `booking_rules.txt` `booking_url`,
  `info_url` and `phone_number`, and `fare_leg_join_rules.txt` `from_network_id` and
  `to_network_id`. `k1_parse.rs` is excluded from that search because its `known_columns` lists
  every official column by name, which would mark every field as covered.

  Three limits are stated wherever the numbers appear, because the numbers are easy to
  overstate. The measurement is a lower bound, so the honest sentence is that these fields carry
  a provision no Spec notice anchors to in the corpus, not that this many rules are missing. The
  provisions are derived from the Presence, Type and primary-key columns alone, so prose
  conditions — "only when B=1", "one of the two is required", "A and B must be given together",
  cross-file conditions, consistency within a trip, GeoJSON nesting — are outside what is
  counted, and the total is not the specification's total. And `[yalnız: …]` proves only that a
  rule emitted on the field: `route_text_color` carries RTS_007 for hex format and RTS_008 for
  contrast, and the contrast rule is legitimate Quality work that does not satisfy the Color
  type's requirement. CI now writes the partial-coverage report to a `spec-audit-report`
  artifact on every run, since a test marked `#[ignore]` is otherwise easy to forget; it stays a
  report, not a gate.
- **A `Spec` rule must now anchor to a field the specification actually defines.** RCT_004
  validated `min_age` and `max_age`, which appear nowhere in the Schedule Reference, and its
  Spec class fed the publish gate — a misclassification no test could see. The new gate reads
  `spec-audit/spec_fields.json`, a field table generated from the reference itself by
  `spec-audit/extract_fields.py`, and checks every notice emitted by a rule whose authority is
  `GtfsSpec`: the `file` and `field` it reports must exist in that table. The anchor comes from
  the rule's own output rather than a hand-kept list, so there is nothing to keep in sync.
  Files the reference documents in prose instead of a field table — `locations.geojson` is the
  only one — are exempt by name, so an unrecognised file fails the gate rather than slipping
  through it. Verified against the tree before the RCT_004 fix: it reports `min_age`. The
  extractor parses the reference's HTML tables with a real parser, since field descriptions
  embed their own value tables and a regex silently truncated `fare_transfer_rules.txt` at
  `fare_transfer_type`.
- **A valid feed must now stay silent in the `Spec` class, and CI enforces it.**
  `crates/pipeline/tests/spec_conformance.rs` is the mirror of the emit proof: the emit proof
  asks whether every rule *can* fire, this asks whether valid data keeps quiet. Eight
  spec-conformant feeds — `stop_times` keyed by `stop_id` only, by `location_id` only, by
  `location_group_id` only, a Flex-only feed with no `stops.txt`, a negative fare amount, a
  lone `routes.network_id`, a service defined only through `calendar_dates.txt`, and a rail
  trip departing at 38:10 — must produce no `Spec` notice and no `CRITICAL` notice. Each feed
  carries a canary: the same feed with a deliberate violation must produce a named rule, so a
  fixture cannot pass green because its file was silently skipped. Three of the eight cases
  would have caught the FPD_002, STM_006 and ARC_004 defects fixed earlier today; the gate
  found two more on its first run, both recorded below.

- **LOC_008, LOC_009, LOC_010 — the required members of a `locations.geojson` feature are now
  checked.** The spec's field table marks `type`, `properties` and `geometry.coordinates` as
  Required for every feature, and none of the three was verified: a feature missing them was
  skipped in silence, so the file appeared validated while its geometry had never been read.
  A missing `coordinates` array is critical, since the zone cannot be resolved at all; the
  other two are medium. An empty `properties` object stays valid, as `stop_name` and
  `stop_desc` are optional.
- **XFL_031 — IDs must be unique across `stops.txt`, `locations.geojson` and
  `location_groups.txt`.** The spec states that `location_group_id` "must be unique across all
  `stops.stop_id`, `locations.geojson` id, and `location_groups.location_group_id` values": the
  three files share one namespace. Nothing checked it, so a feed could name a location group
  after an existing stop and leave every `stop_times.location_id` or `location_group_id`
  reference pointing at two different things. One notice per clashing ID, naming the source it
  collides with.

### Added
- **The specification's file-level provisions were never extracted, and one of them was being
  reported as a preference.** Every file section of the reference carries a `File: …` line, and
  seven of them are conditional — five `Conditionally Required`, two `Conditionally Forbidden`.
  `extract_fields.py` read only the field tables, so that whole class sat outside both ledgers.
  It now captures `presence` per file, and all seven were adjudicated by hand against the
  reference text; the result is written down in `spec-audit/FILE_LEVEL_PROVISIONS.md`, because
  the conditions are prose ("if demand-responsive zones are defined in `locations.geojson`") and
  no measurement can match them to rules automatically.

  Six of the seven were already covered, two of them in ways worth recording: `stops.txt` is
  `Conditionally Required` and `ARC_004` already exempts it when `locations.geojson` is present,
  so a pure-Flex feed is not falsely rejected; and `levels.txt` is covered indirectly but
  completely, by `LVL_006` together with the `level_id` foreign key.

  The seventh was a gap. The reference says of `feed_info.txt`: *"Required if `translations.txt`
  is provided. Recommended otherwise."* Two sentences, two classes — and both were being reported
  by `ARC_020` as a missing *recommended* file, Düşük·Quality. A norm was being reported as a
  preference. `ARC_031` now covers the first sentence at Kritik·Spec, following `ARC_008`, which
  is the same shape of provision; `ARC_020` keeps the second and its comment now records the
  boundary. The practical reason it is Kritik: a translation's meaning depends on
  `feed_info.feed_lang`, so without that file the default language is unknown and `TRN_007`
  cannot work at all.

  This is the mirror of the `PTH_017` error found the same day. There a recommendation was
  enforced as a norm, which can reject a valid feed; here a norm was reported as a
  recommendation, which only under-warns. Both are class-authority failures, and the asymmetry
  is worth stating rather than treating them as equivalent.

  Measured: 34 of the 239 corpus feeds ship `translations.txt` and two of those lack
  `feed_info.txt`. **No publish verdict changes** — both were already unpublishable, one of them
  carrying 42,881 blocking notices before this rule added its one.

- **A fifth gate asks the question the other four do not: is every `Spec` claim backed by a
  provision?** The coverage ledger asks whether every provision is measured. That is the cheaper
  of the two failures — a gap tells a publisher less than it could. The expensive failure is the
  opposite: asserting that the specification requires something it does not, because the R1
  publish gate is exactly `Spec ∧ Kritik` and 170 of the 266 `Spec` rules can block publication.
  Nothing measured that direction.

  `spec_claims_without_a_provision_match_ledger` reports a `Spec`-class rule anchored to a field
  on which the reference's field tables impose nothing. Its first run returned one row, and the
  row was real — see `PTH_017` below. The ledger is empty now, and its header states the two
  ways it is a lower bound, mirroring the coverage side: rules emitting `field: None` are
  invisible, and so is a rule that enforces the *wrong* provision on a field that has one.

### Changed
- **`PTH_017` asserted a recommendation as a requirement, and mislabelled a type error while
  doing it.** The rule carried two facts: `max_slope` not being a number, and `max_slope` being
  used with a `pathway_mode` other than 1 or 3. The first is a real specification claim — the
  field is typed `Float`. The second is not: the reference says this field *"**should** only be
  used with walkways and moving sidewalks"*, and its Presence column is a plain `Optional`. That
  document expresses its actual prohibitions by writing `Conditionally Forbidden` in the Presence
  column, which it does for 14 fields; here it deliberately did not.

  The context branch becomes `PTH_028`, **Quality** — the same judgement as `STM_040` and
  `AGN_007`. `PTH_017` keeps the type check and is retitled accordingly: a feed with
  `max_slope = abc` used to receive a finding titled "invalid context", which is the same
  mislabelling fixed in `ATR_006` this week.
- **`stop_access` had an unchecked prohibition and accepted a value the specification does not
  define.** Reading the reference for the four fields whose provision looked unclassified turned
  one of them into two plain defects rather than a classification question.

  The reference states a condition that nothing enforced: *"Forbidden for locations which are
  stations (`location_type=1`), entrances (2), generic nodes (3) or boarding areas (4). Forbidden
  if `parent_station` is empty."* `STP_026` checks only the enum and `STP_027` is a separate
  quality heuristic. `STP_043` now covers it — a rule of its own, because an enum error and a
  context prohibition are different facts and merging them is the mislabelling fixed in
  `ATR_006`.

  And `STP_026` accepted `0`, `1` or `2` while the reference defines only `0` and `1`, with no
  comment explaining the third. A `stop_access=2` passed silently. Narrowed.

  No corpus feed is affected either way.

- **291 of the 302 measurable provisions become 296, by exercising fixtures that only ever tested
  one branch.** Five of the eleven unproven provisions were not gaps in the rules but gaps in
  what the fixtures triggered: `BKR_023` breaks one of its four integer fields, `TFR_007` one of
  its two directions, `STM_039` one of its two windows. Each fixture now exercises every branch,
  and the dedup level is why one row was never enough — the lesson `CAL_002` taught first.
  `STM_047` also now names both time fields: the reference writes "Required for timepoint=1" for
  `arrival_time` and `departure_time` alike, and the notice named only the first.

  Six remain, and they are two different problems. Two — `FLJ_003` and `FLJ_004` — are a limit of
  the measurement itself: one rule genuinely implements both the conditional presence and the
  foreign key, and counting rules per field cannot express that. Closing them needs rules to
  declare which provision they implement, which is a schema change, not a fixture.

  The other four are a classification question rather than a coverage one. The reference marks
  `routes.continuous_pickup`/`continuous_drop_off` and `stops.stop_access` *Conditionally
  Forbidden* and `trips.shape_id` *Conditionally Required*, and the rules that enforce those
  conditions are `RTS_028` (Interop), `STP_027` and `TRP_019` (Quality). If the provision is
  normative the rule should be `Spec`; that is the same judgement made for `ARC_020` this week,
  and it is left open deliberately rather than changed in passing.

- **The specification requires `agency_id` in `fare_attributes.txt` too, and only `routes.txt`
  was checked.** The reference attaches the same clause to both — *"Required if multiple agencies
  are defined in agency.txt"* — and `AGN_011` enforced it for routes alone. It now covers both
  files, one rule, as `DQ_021` does for primary keys: the fact is identical and the notice's
  `file` distinguishes them.

  One corpus feed is affected and it was already unpublishable. A second looked affected and was
  not: its `fare_attributes.txt` header reads `" agency_id"` with a leading space, which the
  pipeline normalises and a naive scan does not. That is the second time in a day a cheap static
  scan produced a false alarm from header quoting or spacing, so the technique now carries a
  written caveat.

- **`STM_041` named the wrong field when the conflict involved a location group.** The rule
  catches `stop_id` used together with `location_id` or `location_group_id`, and always reported
  `field: location_id` — so a feed whose conflict was with a location group was pointed at a
  field it had not set. It now names all three with the pipe convention and reports whichever
  location field is actually populated.

- **Twelve places where a value that is not a number produced no finding at all.** The pattern
  `Err(_) => None` sat at twelve parse sites in K2, and eleven had the field's own rule
  immediately beside them — so `location_type = 9` drew `STP_008` while `location_type = abc`
  disappeared, along with the value. The same pattern had been fixed individually five times this
  week (`STM_058`, `PTH_027`, `RTS_029`, `TRP_034`) before it was recognised as one pattern.

  Nine sites reuse the neighbouring rule unchanged, because those rules say "invalid" and a
  non-numeric value is invalid for that field. Three could not:

  `STM_030` and `SHP_021` said "negative", and a value that is not a number is not negative.
  Both are retitled to name the type violation they now cover — the same judgement made for
  `PTH_017`.

  The four `booking_rules` integer fields needed a rule of their own, `BKR_023`. Reusing
  `BKR_001`/`BKR_002`/`BKR_005` was the obvious move and it was wrong: those are *context* rules
  ("valid only with booking_type=1"), so a malformed number would have been reported under a
  title about booking types — the mislabelling fixed in `ATR_006` this week, about to be
  reintroduced. The emit-identity gate caught the attempt, which is what it exists for.

  Measured across the corpus: no new findings anywhere. One feed carries NUL bytes in
  `shapes.txt`, but that file is rejected earlier and never reaches `SHP_021`.

- **The atom-level blind spot shrank from 62 fields to 13, by correcting the measurement rather
  than the rules.** A field carrying two provisions but anchored by one rule was reported as
  partially covered, and 46 of the 62 were `presence:required` plus a type — where the presence
  is enforced by `ARC_025`, which anchors the column it reports. Since that rule's list is now
  locked to the specification, membership in it *is* proof the provision is checked; the report
  counts it. One more closed by strengthening `STM_058`'s fixture to break both window fields
  rather than one, the same dedup lesson `CAL_002` taught.

  The remaining thirteen are triaged in the report itself, in three kinds: measurement blindness
  where one rule genuinely covers both atoms (`FLJ_003`/`FLJ_004`, `STM_058`↔`STM_039`); a second
  rule covering the provision without anchoring the field (`RTS_028` for the continuous-pickup
  prohibition, `STP_027` for `stop_access`); and seven that need the specification text read
  before anyone can say whether they are gaps. That reading is the remaining work, and it is
  named rather than implied.

  Worth stating plainly: this makes the tooling more honest, not the validator more capable. The
  original framing — declare, per rule, which provision it implements, across all 563 — turned
  out to be the wrong shape for the problem.

- **The required-column list is now locked to the specification.** `ARC_025`'s list is
  maintained by hand and the rule is Kritik·Spec, so a wrong entry rejects a valid feed and a
  missing one leaves a provision unenforced — both happened in the same file. A test now compares
  the list against `spec_fields.json`, naming which direction each disagreement goes and what it
  costs. It was verified by breaking it: re-adding `from_stop_id` fails the build with exactly
  the message a future maintainer needs.

- **`ARC_025` required `transfers.from_stop_id`, which the specification makes conditional.** The
  rule reports a required column missing from a header, at Kritik·Spec, so it blocks publication.
  Its list of required columns is maintained by hand, and comparing it against the reference
  found three disagreements in one place. `from_stop_id` and `to_stop_id` are *Conditionally*
  Required — *"Required if transfer_type is empty, 0, 1, 2, or 3. Optional if transfer_type is 4
  or 5"* — and 4 and 5 are the in-seat transfers that use `from_trip_id`/`to_trip_id` instead. A
  valid feed carrying only those was being rejected. Meanwhile `transfer_type`, which the
  reference makes unconditionally Required, was absent from the list and therefore unenforced.

  Also added: `route_networks.txt` (both fields Required, the file only became known this week)
  and `attributions.organization_name`, whose list was empty.

  No corpus feed is affected in either direction — the false positive is latent, which is why
  nothing caught it. It would have surfaced the first time someone validated a trip-to-trip
  transfers file.

- **`AGN_001` is removed: it could never fire.** The rule claimed to report a missing
  `agency.txt`, was classed Kritik·Spec, had a card and three locale entries — and emitted
  nothing, by any path. Not a notice, not a fatal code. Its id appeared in no production code;
  only in the registry and in one unit test that used it as a dummy value. A missing
  `agency.txt` is reported by `ARC_004` as `FatalCode::NoRequiredFiles`, and always was.

  It was kept deliberately, to document parity with MobilityData's `missing_required_file`.
  That purpose is legitimate; the place was not. A sentence in `ARC_004`'s card documents the
  parity without inventing a rule, and the cost of the invention was concrete: being Kritik·Spec,
  it inflated every count of Spec rules and R1 blockers by one — including the numbers used to
  reason about the publish gate this week. Three independent mechanisms had been reporting it
  for months: `emit_coverage`'s dynamic allowlist, `emit_proof`'s allowlist, and `sync_cards`
  printing "AGN_001: could not resolve" on every run.

  563 rules. The id is retired and recorded in `removed_ids`, so it cannot be reused.

- **`FatalCode::NoRequiredFiles` had no test.** Auditing the emit-proof allowlist — the rules
  exempted from having to prove they can fire — showed that an exemption is only as good as the
  proof it points at. `ARC_001` and `ARC_029` name real tests. `ARC_004` said it was
  "structurally unprovable in this harness", which is true, and implied the proof lived
  elsewhere; it did not. The fatal path a feed takes when a required file is missing was
  untested. It now has `arc004_missing_required_file_returns_fatal_no_required_files`, and the
  allowlist records where each exemption's proof lives so the next reader can check.

- **Six more rules name their fields, and the remaining thirty are adjudicated and frozen.**
  Working through the Entity/Row-level rules that always emitted `field: None` separated them
  into two kinds. Six were describing a violation of specific fields and now name them:
  `ATR_009` (`agency_id|route_id|trip_id`), `CAL_006` and `CAL_018` (all seven day columns),
  `FRL_007` (`route_id|origin_id|destination_id|contains_id`), `GEO_020`
  (`shape_pt_lat|shape_pt_lon`) and `PTH_011` (`from_stop_id|to_stop_id`).

  The other thirty are correct as they are, for three distinct reasons now written down in the
  gate itself. Most report a **derived fact** — "a stop no trip serves" is not a wrong value in
  `stops.txt`, it comes from a relationship with `stop_times`, and naming a field would send the
  reader to the wrong place. `ARC_012` and `ARC_018` describe the **shape of a row** rather than
  any field. `RCT_006` is **cross-file**: its violation lives in
  `rider_categories.is_default_fare_category` while the notice points at `fare_products.txt`, so
  naming the field would fail the anchor gate, correctly.

  The report becomes a gate: `field_none_emitters_match_ledger`, thirty entries. A new row now
  fails the build and asks the question that matters — is this a derived fact, or a violation of
  specific fields that should use the pipe convention?

- **Eight more rules name the fields their finding is about.** Measuring the blind spot both
  specification ledgers share turned up 91 rules that always emit `field: None`; 47 are file- or
  feed-level, where naming a single field would be wrong, and of the rest ten were `Spec` class —
  invisible to both gates while being able to block publication. Eight now use the pipe
  convention: `ATR_003` (`is_producer|is_operator|is_authority`), `PTH_016`
  (`pathway_mode|is_bidirectional`), `TRF_012`, `TRF_016`, `TRN_005`, `TRN_006`, `TRN_009`
  (`record_id|field_value`) and `TRN_013`. Several already wrote the field names into
  `observed_value` — `PTH_016` reported "pathway_mode=7, is_bidirectional=1" — so the
  information was there and simply not in the column that shows it.

  **Two are deliberately left as `None`.** `ARC_012` reports an inconsistent column count, which
  is a property of the row's shape rather than of any field. `RCT_006` reports a
  `fare_product_id` bound to more than one default rider category; the offending value lives in
  `rider_categories.is_default_fare_category` while the notice points at `fare_products.txt`, and
  writing a field that does not belong to the named file would fail the anchor gate — correctly.

  One caveat worth recording: a broad anchor makes the coverage ledger less sensitive, since a
  field counts as covered once any `Spec` rule names it. `TRN_005` now anchors six fields it
  does not individually check. That is the atom-blindness already documented in the ledger
  header, slightly widened in exchange for findings that say which fields they are about.
- **`RTS_003` now names the two fields it is about.** A route with neither `route_short_name`
  nor `route_long_name` is a Kritik finding that keeps the feed from being published, and the
  "Field" column of the R2 report was blank for it, because the notice carried `field: None`.
  It now carries `route_short_name|route_long_name` — the pipe convention the codebase already
  uses in 32 places for a notice about two fields jointly (`stop_lat|stop_lon`,
  `start_date|end_date`, `from_stop_id|to_stop_id`). Naming only one of them would have been
  wrong: the provision is that *both* are empty.

  This is a visible output change — Golden snapshots gain a value where the field column was
  empty — but not a behavioural one: `field` is consumed only inside per-rule sort and dedup
  keys, and a constant value shifts nothing relative to anything.

  It also closes the last two lines of the coverage ledger, though that was the side effect
  rather than the reason. Had the ledger been the only argument, the rule should have been left
  alone; a measurement is not a reason to change production output.

### Added
- **`trips.safe_duration_offset` was read as an unsigned integer, and the specification types it
  `Float`.** A valid fractional offset — `12.5` seconds — failed to parse and was discarded
  without a word, along with any parse failure on `safe_duration_factor`, both dropped by
  `.ok().flatten()`. That is the fourth instance of the same swallow found in two days, after the
  Flex windows, `stair_count` and `route_sort_order`; at four it is the idiom that needs
  attention, not the fields. Both now parse as `f64` and report failures as `TRP_034`.

  This one was found by the measurement rather than by reading. `provision_atoms` now derives an
  atom from the plain scalar types `Float` and `Integer`, which impose parseability even though
  they impose no range. `Text`, `ID` and `Text or URL or Email or Phone number` deliberately
  derive nothing — the reference lets those be any string, so there is no checkable provision,
  the same reasoning that removed the invented `Phone number` atom. Making the measurement finer
  immediately opened two ledger lines that had been invisible, which is the measurement working.

- **The specification-coverage ledger is empty.** It opened at 32 lines and closes at zero, which
  means every normative provision derivable from the reference's field tables now has a `Spec`
  notice anchored to it in the emit-proof corpus. The claim is deliberately narrow, and the
  ledger header says so where the number is read: prose conditions, cross-file conditions,
  within-trip consistency and GeoJSON structure remain outside what this measurement can see.

  The last six lines to close were never gaps at all, and finding that out took correcting the
  measurement rather than the rules. `CAL_002` checks all seven `calendar.txt` day columns, but
  its fixture only broke `monday`, so six columns looked unanchored. Breaking all seven in one
  row was not enough either — the rule dedups at `Entity` level on `service_id`, so a single
  service yields a single notice no matter how many days are wrong. The fixture now uses seven
  services, one broken day each.
- **`fare_leg_join_rules.txt` is now a file the validator knows about** (issue #59). It is part
  of Fares v2, and the pipeline had never heard of it: absent from `KNOWN_FILES`, so a feed
  shipping it was told it had an unknown file; absent from `required_fields` and
  `known_columns`, so its columns were unknown columns; no parser, no rules. The UI's file map
  meanwhile already drew the node and its edge to `networks.txt` — that inconsistency is
  resolved by the pipeline catching up rather than the UI being trimmed.

  A new `FLJ` group covers all four fields, one rule each, checking presence and resolution
  together as `NET_002`/`NET_003` do. The two halves of the file behave differently and the
  specification is explicit about both:

  - `from_network_id` and `to_network_id` are unconditionally Required and resolve against the
    **union** — *"referencing routes.network_id **or** networks.network_id"*. This is the
    opposite of `NET_002`, which must resolve against `networks.txt` alone; the difference is in
    the specification text, not a preference.
  - `from_stop_id` and `to_stop_id` are **mutually** conditional — *"Required if to_stop_id is
    defined. Optional otherwise."* and its mirror — so one filled and the other empty is a
    violation, while both empty is explicitly valid. The target must also be a stop
    (`location_type` 0 or empty) or a station (`location_type=1`); entrances, nodes and boarding
    areas are not.

  The file's primary key is all four fields together, so `DQ_021` gains it alongside the other
  composite keys. A blank value is part of the key rather than an absence, since the
  specification treats a blank predicate field as "ignored for matching" rather than missing.

  **These four rules have no real-data evidence.** No feed in the 239-feed corpus ships the
  file, so unlike the `booking_rules.txt` batch they rest on unit fixtures alone. Worth
  recording rather than glossing: a feed that does ship it will be the first real exercise.

  The coverage ledger drops from 12 open lines to **8**, and all eight that remain are known
  measurement artifacts rather than gaps — the six `calendar.txt` day columns that `CAL_002`
  does check, and `routes.route_short_name`/`route_long_name` that `RTS_003` checks but emits
  with `field=None`.
- **The three contact fields of `booking_rules.txt` were declared but never read** (issue #60).
  `booking_url`, `info_url` and `phone_number` were listed in `known_columns`, so a feed
  carrying them drew no "unknown column" warning, and no validation stage looked at the values —
  in a file whose entire purpose is telling riders how to book. `BKR_020` and `BKR_021` check the
  two URLs, `BKR_022` the phone number.

  `booking_url` is Orta while `info_url` is Düşük: the first is where the rider actually books,
  the second is supplementary. `BKR_022` is **Quality, not Spec**, following `AGN_007` — the
  specification's `Phone number` type prescribes no grammar (its definition is "A phone number"),
  so a strict check would reject valid international formats; the rule only asks whether enough
  digits are present.

  Better evidence than the previous batch: all eight Flex feeds in the corpus populate these
  fields — 23 booking rules with contact details between them — and none is malformed. The rules
  are exercised by real data and stay correctly silent, rather than being silent because the
  field is absent everywhere.

  The coverage ledger drops from 14 open lines to 12. Two, not three: the issue predates the
  removal of the invented `Phone number` format atom, so `phone_number` was never a ledger line.
- **`route_networks.txt` was parsed but never validated** — the file had no rules at all. Both of
  its fields are `Required` and both are foreign keys, so a row pointing at a route or a network
  that does not exist passed silently, and the route dropped out of its network without a word;
  fare rules then do not apply to it. `NET_002` covers `network_id`, `NET_003` covers `route_id`,
  each checking presence and resolution together, since an empty value is also a value that does
  not resolve.

  `network_id` is resolved against `networks.txt` **only**, not against `EntityMap::network_ids` —
  that set also contains ids contributed by `routes.network_id` and by `route_networks.txt`'s own
  rows (`FLG_002` wants the full set), so checking against it would be circular and could never
  fire. The specification is explicit: *Foreign ID referencing `networks.network_id`*.

  The file's primary key is `route_id`, and the reference states the consequence directly — *"A
  route_id can only be defined in one network_id"* — so `DQ_021` gains `route_networks.txt`
  alongside the `attributions.txt` entry above.
- **Four remaining provisions from issue #61**, closing the ledger lines that the issue's body
  never described. `RCT_007` gives `rider_categories.eligibility_url` the URL format check every
  sibling URL field already had. `RTS_029` covers `routes.route_sort_order`, typed `Non-negative
  integer`, whose parse failure was being dropped silently — the third instance of that same
  swallow found today, after the Flex windows and `stair_count`. `TRN_015` closes the last of the
  three provisions the specification attaches to `translations.field_value`: two were already
  covered (`TRN_013` for `feed_info`, `TRN_009` for use alongside `record_id`), and the missing
  one was *"Required if record_id is empty"* — with both empty, a translation row never says
  which record it translates and can never be applied.

  `TRN_015` is deliberately narrow, and measuring showed why. In its first form it fired
  **101,872 times on one corpus feed**, an Israeli dataset still using the legacy Google
  `trans_id,lang,translation` layout, which has none of the specification's columns. Those rows
  already draw `TRN_001`, `TRN_002`, `TRN_003`, `TRN_006` and `TRN_011` — saying the same thing a
  sixth time is noise, not coverage. The rule now stays silent when `table_name` is not a
  supported table, since `TRN_001` already rejects the row and the question "which record does
  this translate" no longer means anything. Across the corpus it now matches nothing.

  Scanned across all 239 corpus feeds, none of the five new rules matches a single row.

  **Issue #61 is fully closed:** the coverage ledger drops from 19 open lines to 14.
- **Three more provisions from issue #61 are now checked**, and one of them needed no new rule.

  `attributions.attribution_id` is typed `Unique ID` and is the file's primary key, but nothing
  checked that two attributions do not share one — `ATR_001` reports the field being *absent*,
  which is a project choice since the field is Optional. `DQ_021` already enforces single-field
  primary keys for `stops.txt`, `routes.txt` and `trips.txt`, so `attributions.txt` joins that
  list rather than getting a rule of its own.

  `PTH_027` covers `pathways.stair_count`, typed `Non-null integer`. Zero is the one value the
  type excludes, and the specification says why: a positive count means the rider walks up from
  `from_stop_id` to `to_stop_id`, a negative one that they walk down, and zero says neither. The
  same rule catches a value that is not an integer at all — that error was being dropped
  silently, the same swallow as the Flex windows above.

  `STP_042` covers `stops.stop_url`, typed `URL`. Every sibling field already had a format rule
  — `agency_url`, `agency_fare_url`, `route_url`, `feed_publisher_url`, `attribution_url` — while
  `stop_url` had only `STP_034`/`STP_035`, which report that it duplicates another URL and say
  nothing about its shape.

  All three are guards rather than new noise: scanned across the 239 corpus feeds, none of them
  matches a single row, so no score moves. The evidence for `PTH_027` is thin by nature — only
  six corpus feeds ship `pathways.txt` at all.

  The coverage ledger drops from 22 open lines to 19.

### Fixed
- **A malformed Flex booking window produced no finding at all** (`STM_058`, issue #61).
  `start_pickup_drop_off_window` and `end_pickup_drop_off_window` are typed `Time`, and they
  were parsed with the same helper as `arrival_time` — but where `arrival_time` turns a parse
  failure into `STM_003`, these two discarded it with `.ok().flatten()`. The consequence was
  worse than a missing check: the value was gone, so `STM_038` had nothing left to compare,
  and the row still counted as having a window for the presence rules. A feed carrying
  `start_pickup_drop_off_window = 9am` therefore validated silently while nobody could book
  the trip.

  One rule covers both fields rather than two mirroring `STM_003`/`STM_004`. The fact is
  single — a `Time` field that does not parse — and the notice names the field, so nothing is
  mislabelled; the identity ledger records the adjudication. It blocks `STM_038`, since
  "start is after end" is derived and misleading when the format is broken in the first place.

  Verified against the eight real Flex feeds now available: **zero firings, and not one of the
  eight changed** — including the Swiss national feed, where all 104 window-bearing rows parse
  cleanly. The rule is a guard, not a new source of noise. This closes the
  `stop_times.txt:start_pickup_drop_off_window` line of the coverage ledger (23 open lines
  become 22) and the first of the nine provisions in issue #61.
- **Every stop in a demand-responsive feed was reported as unused.** STP_020 built its set of
  served stops from `stop_times.stop_id` alone. A GTFS-Flex feed reaches its stops through
  `location_group_id` instead, leaving `stop_id` empty on those rows, so the rule saw a
  `stops.txt` full of stops that no trip touches. Measured on the Swiss national Flex feed:
  **1259 of 1259 stops flagged, 85% of everything the validator said about that feed.** Stops
  that belong to a location group now count as served.

  The fix is narrow by construction, and that was verified rather than assumed: across the
  seven other Flex feeds, all of which ship an empty `location_groups.txt`, the STP_020 counts
  are unchanged (23, 17, 2, 2, 1, 1, 1) — no genuine finding was lost. Note the deliberate
  tolerance: group membership shows a stop *can* be served; whether a trip actually uses that
  group is not asked.
- **LOC_006 measured a bounding box and called it an area.** Flex service zones follow
  administrative boundaries and are rarely convex, so the box systematically overstates them —
  by 1.4x to 5.6x across the eight feeds measured. Three of the rule's seven firings existed
  only because of that inflation (Columbia County twice, and Pueblo at 954 km² against a real
  245 km²); those feeds are now silent, and the areas the rule reports are real ones. The area
  comes from a shoelace sum in an equirectangular projection, with inner rings subtracted from
  the outer ring, which the box could not do either.

  What the fix does *not* settle is the 500 km² threshold. Of the four firings that survive on
  real area, at least one is provably legitimate: the Dolores County zone measures 2751 km²
  against the county's actual 2696 km², so the polygon is the county. Rural demand-responsive
  service is county-scale, and the threshold is calibrated for urban zones; the rule's own card
  now says so where the number is read.
- **A dangling `route_id` in `attributions.txt` was reported as an invalid `is_authority`.** The
  identity check added above found two rule ids doing double duty, and this is what a user saw
  because of it: ATR_006 carried both the `is_authority` enum check in K2 and the
  `attributions.route_id` foreign-key check in K4, so a broken route reference produced a
  finding titled "is_authority geçersiz" — and ATR_007 did the same for `attribution_url` and
  `trip_id`. The two foreign-key paths move to **ATR_011** (`route_id`) and **ATR_012**
  (`trip_id`), leaving ATR_006 and ATR_007 measuring one fact each.

  The new rules are Düşük·Spec, matching **ATR_010**, which has checked the third reference
  field of the same file, `agency_id`, at that severity all along; the old Kritik came from the
  enum and URL rules whose weight the foreign-key paths happened to share, not from a judgement
  about dangling references. No publish gate is lost: **XFL_015** (Kritik, `VS_K`) summarises
  all three reference fields at feed level, so a feed with a broken attribution reference still
  fails R1 exactly as before — which is already how `agency_id` behaves.

  Two residues of the same history were fixed alongside: the remediation text for ATR_005
  (`is_operator` invalid) told users to "use a valid agency_id" — that check moved to ATR_010
  long ago and the text never followed — and ATR_006's told them to use a valid `route_id`.
  Both now describe the enum they actually check. `RTS_001`'s `blocks` list and the K4 section
  comment, which still named ATR_005-007, were updated to match.

  This closes two lines of the specification-coverage ledger: `attributions.txt:route_id` and
  `:trip_id` were listed as foreign-key provisions with no Spec anchor, because the rules that
  checked them were anchored to a different field. 25 open lines become 23.
- **The zip guard's compression-ratio cap had less headroom than its own comment claimed.** It
  was calibrated in July against a 419 MB aggregate feed whose worst entry compressed 17.8:1,
  leaving what the code described as 4.5x of margin. A calibration run over the Entur feed
  measured `calendar_dates.txt` at **32.8:1** — that file compresses extremely well, since it
  repeats dates and service ids — which is above the 16 MiB ratio floor and therefore subject to
  the check. Entur passed, but on 2.4x of margin rather than 4.5x, and a legitimate feed with a
  more repetitive calendar would have been rejected outright with a fatal ARC_029, the harshest
  false positive the validator can produce. The cap moves from 80 to 100, which keeps roughly
  3x of headroom over the worst measured feed while staying an order of magnitude below the
  ~1032:1 ceiling a single DEFLATE stream can reach; the absolute entry and total caps bound the
  damage regardless of ratio. The comment now records Entur as the calibration basis.
- **`rule_priority` is a `fare_leg_rules.txt` field, and K1 accepted it in
  `fare_transfer_rules.txt` too.** The spec defines it only for fare leg rules, where FLG_007
  validates it; the transfer rules table does not list it. Because K1 held it among the known
  columns of both files, a producer that put the column in the wrong file got no ARC_017 and no
  hint that nothing would ever read it. Found by the new field-anchor gate while cross-checking
  the generated spec table against the parser's own column lists.
- **DQ_021 now checks composite primary keys.** It looked only for repeated `stop_id`,
  `route_id` and `trip_id`, so the Fares v2 and Flex files — where most keys are composite —
  could repeat a key freely. Duplicates are now detected on the six-field key of
  `fare_leg_rules.txt`, the five-field key of `fare_transfer_rules.txt`, `location_group_id` in
  `location_groups.txt`, and the whole row in `location_group_stops.txt`, whose key the spec
  writes as `(*)`. Field lists are taken verbatim from the specification, and a blank value
  counts as part of the key, as the spec treats blanks as meaningful.
- **XFL_019 missed the networks.txt half of its rule.** The spec forbids `routes.network_id`
  when *either* `route_networks.txt` or `networks.txt` exists; only the first was checked, so a
  feed defining networks twice through `networks.txt` went unreported. Both files now trigger
  it, and the message names the one that clashed. Using `routes.network_id` with neither file
  present remains valid.
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
- **XFL_019 pointed at the wrong file.** The rule reports that `routes.network_id` is used while
  `networks.txt` or `route_networks.txt` also exists, which the spec forbids. It anchored the
  finding to the network file — the one that is perfectly legal — rather than to the column that
  must go, sending anyone who follows the report to the wrong place. The message already named
  the conflicting file, so nothing is lost by anchoring where the fix belongs. The coverage
  ledger had recorded the consequence without anyone noticing: `routes.txt:network_id` looked
  unchecked, and `route_networks.txt:network_id` looked checked. Correcting the anchor swapped
  both, which is how the second one turned out to be a genuine gap — nothing validates
  `route_networks.txt` at all, although its rows are parsed.
- **AGN_012, RTS_024 and TRN_008 were classed as quality problems, not specification errors.**
  Adjudicating every ledger entry whose field is measured only by a non-Spec rule turned up
  three of the same kind as RTS_007. `AGN_012` and `RTS_024` check that `cemv_support` holds one
  of the enumerated values, and the reference defines `Enum` as "an option from a set of
  predefined constants defined in the Description column", so a value outside the set violates
  the specification. Both keep low severity, matching RTS_013 and TRP_006. `TRN_008` reports an
  empty `translation`, a field the reference marks Required; an empty required value is a
  missing required value, which this project has always treated as critical and Spec —
  FIN_001, ATR_002 and LVL_007 all do. TRN_008 therefore becomes critical and Spec, **which
  makes it a publish blocker**, since the publish gate takes exactly the critical Spec findings.
  The file being optional does not soften it: `attributions.txt` and `levels.txt` are optional
  too.

  Two of the ten entries needed no rule change at all. `route_short_name` and `route_long_name`
  appeared uncovered only because RTS_003, which is critical and Spec, enforces their mutual
  requirement while reporting no field name — precisely the lower-bound artefact the ledger
  header warns about.
- **The coverage measurement no longer invents a provision for phone numbers.** It counted
  `Phone number` as a format constraint, but the reference defines that type in full as "A
  phone number", with no grammar to violate. Three entries — `agency_phone`,
  `attributions.attribution_phone` and `booking_rules.phone_number` — were listed as
  uncovered provisions that do not exist, and AGN_007 was being implicitly blamed for being
  Quality when Quality is the right class for it. The ledger drops from 31 entries to 25.
- **RTS_007 is a specification error, like its twin RTS_006.** Both rules check that a colour
  is a six-digit hex value, with the same helper and the same handling of an empty value, but
  `route_color` was classed Spec at medium severity and `route_text_color` Quality at low. The
  reference types both fields as `Color` and defines the type normatively: "A color encoded as
  a six-digit hexadecimal number … the leading '#' must not be included". A field being
  Optional does not make its type advisory — if a value is given, the format is required. There
  was no substantive reason for the split, so RTS_007 becomes medium severity and Spec, with
  its authority set to `GtfsSpec`. The contrast requirement is a separate matter and the
  reference words it with "should", so RTS_008 stays Quality, which is correct.
  This is what the coverage ledger is for: `routes.txt:route_text_color` was listed there as
  carrying a provision with no Spec rule anchored, and closing the gap removed the line.
- **STM_040 is no longer a specification error.** The rule reports a Flex row that defines a
  pickup or drop-off window without either booking rule ID. The 27 April 2026 Schedule
  Reference marks both fields **Optional**, and only adds "Recommended when `pickup_type=2`".
  A recommendation is not a norm, so a valid Flex feed was losing the publish gate, which is
  exactly `Spec ∧ CRITICAL`. The check is unchanged and moves to Quality at medium severity,
  with its authority set to `ProjectQuality`: the trigger it uses — any window present and both
  IDs blank — is the project's own tightening, not the spec's conditional wording. Narrowing it
  to `pickup_type=2` / `drop_off_type=2` is recorded on the card as separate work.
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
- **STM_057, which read a conditional sentence of the spec as an unconditional one.** It
  required two `stop_times.txt` records for every `location_id` and every `location_group_id`
  in a trip. The spec says something narrower: "Travel **within the same** location group or
  GeoJSON location requires two records in `stop_times.txt` with the same `location_group_id`
  or `location_id`." Two records describe a journey that begins and ends inside one zone. An
  ordinary point-to-point Flex trip visits each zone once, and the rule rejected it as a
  critical specification error — a publish blocker on correct data. Its card claimed no false
  positive was possible. Nothing falsifiable remains once the conditional is respected: a trip
  whose single record is a Flex location is already `STM_033`, "trip with one stop". Removal
  covers the registry entry, the AUTHORITY row, the `removed_ids` guards, the K6 emit site,
  both integration tests, the emit-proof fixture, the generated RULES files, the UI and CLI
  locales in three languages, the card, and the rule count in the three READMEs. 545 → 544.

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
