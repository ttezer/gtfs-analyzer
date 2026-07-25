# GTFS Validator & Analyzer

🇹🇷 [Türkçe](README.md) · 🇬🇧 **English** · 🇯🇵 [日本語](README.ja.md)

[![Open App](https://img.shields.io/badge/Open%20App-gtfs--analyzer-2ea44f?style=flat&logo=googlechrome&logoColor=white)](https://ttezer.github.io/gtfs-analyzer/)
[![GTFS-JP](https://img.shields.io/badge/GTFS--JP-supported-c8102e?style=flat)](https://www.gtfs.jp/)
[![GTFS Spec](https://img.shields.io/badge/GTFS-Spec-007ec6?style=flat)](https://gtfs.org/)
[![License MIT](https://img.shields.io/badge/license-MIT-yellow?style=flat)](LICENSE)

GTFS Validator & Analyzer is an open-source, browser-based GTFS validator and feed quality analyzer. The uploaded .zip file is never sent to any server; all processing is performed on the user's device via WebAssembly.

GTFS Validator & Analyzer does not merely check whether a file conforms to the specification; it also analyzes how reliable, consistent, and usable the feed is. It shows errors together with the relevant file and line number, provides remediation steps for each finding, and marks geographic issues — such as deviating routes, broken coordinates, or unreachable stops — on an interactive map.

Every finding is tagged with a rule code, an analysis class, and a severity level. Thanks to the Spec · Interop · Quality · Analytics classes and the Critical → Info severity levels, thousands of findings can be filtered, prioritized, and handled systematically. The tool also automatically detects the GTFS features used by the feed — Shapes, Transfers, Fares, Headsigns, Flex, and the like — and includes them in the report.

GTFS Validator & Analyzer extends specification validation with operational quality analysis. Frequency inconsistencies per route, anomalous speed segments, isolated stops, gaps in service patterns, andnetwork topology problems are examined with 539 distinct validation and analysis rules. Results are summarized with two scores — the Publish Score (blocking issues only) and the Overall Score (weighted average of all four classes) — computed with different formulas for different purposes. The prioritized fix queue shows which issues should be addressed first and the likely impact of each fix on the score.

**Who is it for?**

- **Transit operators and municipalities** — To validate a feed and resolve quality issues before publishing.
- **GTFS integrators and consultants** — To document the technical and operational quality of delivered data.
- **Application developers** — To assess the reliability and integration risks of the feeds they consume.
- **Researchers and analysts** — To compare different transit networks in terms of data quality and structure.

---

## Comparison with Other Tools

### Feature Matrix

| Feature | MobilityData | GTFS Guru | GTFS Analyzer |
|---|:---:|:---:|:---:|
| Web interface | ✅ | ✅ | ✅ |
| Data never leaves the browser | ❌ | ✅ | ✅ |
| Spec compliance rules | ✅ | ✅ | ✅ |
| Quality rules | ❌ | ❌ | ✅ |
| Operational analytics | ❌ | ❌ | ✅ |
| Map visualization | ❌ | ❌ | Stops, routes, trips, lines, pathways |
| Feed score | ❌ | ❌ | ✅ |
| Remediation guidance | Partial | ❌ | ✅ |
| GTFS Flex support | Partial | ❌ | ✅ |
| Fares v2 validation | Partial | ❌ | ✅ |
| GTFS-JP profile validation | ❌ | ❌ | ✅ |
| Output formats | HTML, JSON | HTML, JSON | HTML, CSV, JSON, PDF |
| Platform | Web | Web, CLI, Desktop | Web, CLI *(Desktop planned)* |
| **Total rules** | **178** | **~120** | **539** |

### Feed Analysis Examples

The same feeds were compared with two validators: MobilityData gtfs-validator v8.0.1 · GTFS Analyzer v0.6.0. (GTFS Analyzer figures are a snapshot from a run on 2026-07-24; because some rules are date-dependent, running on a different day may produce small deviations.)

#### BART (Bay Area Rapid Transit, San Francisco)

Feed: `mdb-53` (MobilityDatabase, 2026-07-15 snapshot; validity range: 2026-01-12–2026-08-30) · 14 routes, 287 stops, 7,036 trips.

| | MobilityData | GTFS Analyzer |
|---|---:|---:|
| Total notices | 2,745 | 1,227 |
| Critical / Error | 2 | 2 |
| High / Warning | 2,656 | 4 |
| Medium | — | 21 |
| Low | — | 65 |
| Info | 87 | 1,135 |
| Distinct rule types triggered | 12 | **45** |
| Publish score | — | **92.6 / 100** |
| Overall score | — | **90.6 / 100** |

#### TriMet (Portland, Oregon)

Feed: `mdb-247` (MobilityDatabase, 2026-07-15 snapshot; validity range: 2026-07-05–2026-11-28) · 112 routes, 6,480 stops, 70,557 trips.

| | MobilityData | GTFS Analyzer |
|---|---:|---:|
| Total notices | 970 | 6,356 |
| Critical / Error | 908 | 0 |
| High / Warning | 49 | 795 |
| Medium | — | 117 |
| Low | — | 1,908 |
| Info | 13 | 3,536 |
| Distinct rule types triggered | 9 | **54** |
| Publish score | — | **100 / 100** |
| Overall score | — | **84.1 / 100** |

> ⚠️ **Overlapping block trips:** This feed's dominant finding is trips that overlap in time within the same block (908 *errors* in MobilityData). GTFS Analyzer catches the same issue with TRP_022; both tools count a conflict only for services active on the same day (calendar intersection). The difference is the counting unit: MobilityData reports one notice per overlapping trip **pair** (908), while GTFS Analyzer collapses repeated overlaps of the same trip into a single record (770), suppressing repetition within busy blocks. Severity classification also differs (not critical in Analyzer).
>
> ⚠️ **Fares v2:** This feed assigns networks via the `network_id` column in `routes.txt` (there is no `networks.txt` — a valid GTFS Fares v2 method). GTFS Analyzer resolves `network_id` references in `fare_leg_rules.txt` against all three sources (`networks.txt`, `routes.txt`, `route_networks.txt`), so it raises no false critical for valid definitions (0 critical on this feed). Genuinely undefined `network_id` values and similar Fares v2 referential-integrity problems are still reported as critical (FAR/FPD/FLG/FTR/RCT/FMD groups). MobilityData also validates Fares v2 (schema + referential integrity + fare_transfer/products/media/timeframes rules), though coverage and severity classification differ.

#### Tokyo Toei (Tokyo Metropolitan Bureau of Transportation)

Feed: `mdb-3175` (MobilityDatabase, 2026-07-24 snapshot; validity range: 2026-07-24–2029-07-23) · 151 routes, 5,370 stops, 67,661 trips.

| | MobilityData | GTFS Analyzer |
|---|---:|---:|
| Total notices | 2,458 | 2,773 |
| Critical / Error | 0 | 0 |
| High / Warning | 330 | 22 |
| Medium | — | 964 |
| Low | — | 1,120 |
| Info | 2,128 | 667 |
| Distinct rule types triggered | 9 | **55** |
| Publish score | — | **100 / 100** |
| Overall score | — | **84.2 / 100** |

> 🗾 **Spec-clean but operationally dense:** Both tools report 0 critical — the feed is specification-clean. The difference is in the analytics layer: most of GTFS Analyzer's medium/low findings are operational signals from the three-year validity window (2026–2029) and dense network/shape patterns, which MobilityData largely summarizes as warnings/info.

#### VBB (Berlin-Brandenburg Transport Association)

Feed: `mdb-782` (MobilityDatabase, 2026-07-23 snapshot; validity range: 2026-07-21–2026-12-12) · 1,262 routes, 41,949 stops, 253,494 trips, 14,084 shapes · ~75 MB. This feed is too large for MobilityData's hosted web validator; the MobilityData figures come from a report produced with its desktop application. GTFS Analyzer validates the feed directly in the browser (~15 s).

| | MobilityData | GTFS Analyzer |
|---|---:|---:|
| Total notices | 11,912 | 28,380 |
| Critical / Error | 0 | 0 |
| High / Warning | 11,193 | 2,436 |
| Medium | — | 7,439 |
| Low | — | 8,530 |
| Info | 719 | 9,975 |
| Distinct rule types triggered | 19 | **97** |
| Publish score | — | **100 / 100** |
| Overall score | — | **77.8 / 100** |

> 🇩🇪 **Large feed, different focus:** Both tools report 0 critical — the feed is specification-clean. More than half of MobilityData's total (`non_ascii_or_non_printable_char`, 6,810) is the legitimate ü/ö/ä/ß characters in the feed's German text; GTFS Analyzer does not flag valid Unicode letters, only non-printable/control characters. GTFS Analyzer's volume instead comes from operational/geometric analytics MobilityData does not have (shape, stop, statistical duration). On core checks the two align: `stop_without_stop_time` (STP_020) and `service_has_no_active_day_of_the_week` (CAL_006) match exactly at 1,411 and 991.

---

## GTFS-JP Support

GTFS Analyzer automatically recognizes **GTFS-JP**, Japan's national GTFS profile (国土交通省 / MLIT standard), and enforces the requirements that GTFS-JP makes mandatory where standard GTFS leaves them optional. Because MLIT requires subsidized operators to publish GTFS-JP, hundreds of small operators must conform to this profile — yet mainstream validators do not check its profile-specific obligations.

**Automatic detection.** A feed is flagged as GTFS-JP — and a **GTFS-JP** badge appears in the report — when it contains `*_jp.txt` files (`agency_jp.txt`, `office_jp.txt`, `routes_jp.txt`), when `feed_lang` starts with `ja`, or when `translations.txt` carries kana (`ja-Hrkt`) readings. The profile rules activate only on such feeds and stay silent on standard feeds.

**Profile rules (JPN group).**

| Rule | Check |
|---|---|
| **JPN_001** | Kana reading (よみがな — `translations.txt`, `ja-Hrkt`) for stop names; required by GTFS-JP for voice announcements and search |
| **JPN_002** | `jp_office_id` (in `trips.txt` **or** `routes.txt`) must match an `office_id` defined in `office_jp.txt` (operating-office referential integrity) |
| **JPN_003** | `agency_jp.txt` `agency_id` must be defined in `agency.txt` (operator referential integrity) |
| **JPN_004** | `translations.txt` must be present — mandatory in GTFS-JP (notably for kana readings) |
| **JPN_005** | `office_name` (a required field) must be filled in `office_jp.txt` |
| **JPN_006** | `fare_attributes.txt` + `fare_rules.txt` must be present — mandatory in GTFS-JP |
| **JPN_007** | `feed_info.txt` must be present — mandatory in GTFS-JP |
| **JPN_008** | Kana (`ja-Hrkt`) reading for the route name (`route_long_name`) |
| **JPN_009** | Kana (`ja-Hrkt`) reading for `trip_headsign` |
| **JPN_010** | Kana (`ja-Hrkt`) reading for the operator name (`agency_name`) |

The **Tokyo Toei** comparison above shows how the profile behaves on a real GTFS-JP feed: the feed is specification-clean (0 critical), and the profile rules produce no false positives on correctly referenced data.

---

## Usage

GTFS Validator & Analyzer is a web application — no installation required. Open the live version in your browser and upload your GTFS zip file.

The runtime is selected automatically from browser capabilities: browsers with Memory64 use
**WASM64** for feeds that need more than 4 GB; all others use **WASM32**. The active engine is
shown on the upload screen. For diagnostics, use `?wasm32=1`, `?wasm64=1`, or `?serial=1`.

**→ [https://ttezer.github.io/gtfs-analyzer/](https://ttezer.github.io/gtfs-analyzer/)**

1. Drag and drop your GTFS zip file, or use the file picker.
2. Validation starts automatically; progress is shown step by step on screen.
3. When complete, the Publish and Overall scores appear with the detailed report tabs.
4. To compare an earlier analysis, open **Compare** and upload its Golden JSON. Fixed, new, decreased, and increased rules are shown alongside score, feed-date, and normalized notice-density changes.
5. For a shareable deliverable, open **Export → Executive PDF Report**, choose the report language, and use **Print / Save PDF** in the preview.

### Executive PDF Report

The **Executive PDF Report** turns detailed validation results into a readable, color-coded, A4-ready document for decision-makers and feed producers. It is generated exclusively from **GTFS Analyzer** results and does not include output from another validator or an external comparison.

The report includes:

- publication status, Publish Score, Overall Score, and the Spec · Interop · Quality · Analytics components;
- a feed profile covering stops, routes, trips, shapes, service days, and date ranges;
- deduplicated **P0 / P1 / P2** actions that combine R1 publication blockers with the R9 impact/effort ranking;
- evidence, impact, recommended remediation, actual affected-instance count, and potential score gain for every priority finding;
- feed-specific structural insights, a phased remediation plan, severity/class distributions, and a technical appendix.

Even when the UI retains a limited number of finding examples for performance, the report uses **actual aggregate counts** from `capped_totals` when available. The document can be generated in Turkish, English, or Japanese, independently of the UI language. Generation and printing happen entirely in the browser: the GTFS data is not uploaded to a server, and no external API is required.

> Report scores assess the uploaded GTFS feed; they do not rate the performance or accuracy of GTFS Analyzer itself.

> To self-host or set up a development environment, see [Developer Setup](#developer-setup).

---

## CLI (Terminal)

Besides the web UI, you can run the same validation core (`gtfs_pipeline::validate_bytes`) from a terminal — for Python/automation integration.

```bash
# From source
cargo run -p gtfs-cli -- validate feed.zip --json

# Release binary
cargo build --release -p gtfs-cli
target/release/gtfs-analyzer validate feed.zip --json
```

| Flag | Description |
|---|---|
| `--json` | Writes the full `ValidateResult` as JSON to stdout |
| `--summary` | Short summary: status, notice count, scores (default) |
| `--rule SHP_010` | Only notices for the given rule |
| `--severity critical` | Filter by severity (critical/high/medium/low/info) |
| `--config config.json` | Apply a JSON config delta (on top of `ValidatorConfig::default()`) |
| `--today 20260710` | Pin the analysis "today" (for calendar rules) |

**Exit codes:** `0` no notices · `1` notices present · `2` fatal or config/file error. In JSON mode stdout is JSON only; errors go to stderr.

```python
import json, subprocess

proc = subprocess.run(
    ["target/release/gtfs-analyzer", "validate", "feed.zip", "--json"],
    text=True, capture_output=True,
)
# exit 1 means "has notices", not failure — do NOT use check=True
data = json.loads(proc.stdout)
result = data["Ok"] if "Ok" in data else data["Fatal"]  # top key is the enum variant
```

---

## Analysis Thresholds

Validation thresholds can be customized from the **Analysis Thresholds** section on the upload screen. Changed values take effect on the next ZIP upload; the reset button restores defaults.

### Rule Classes and Authority Source

Every rule falls into one of four classes. The class reflects the finding's **authority source** (its basis of legitimacy), so a user can tell at a glance whether a finding is a real GTFS Spec violation or an interoperability/quality/analytics signal:

- **Spec** — only cases that the official **GTFS Schedule Reference** explicitly requires, forbids, or renders invalid (required / conditionally required / conditionally forbidden fields, enum values, foreign keys, uniqueness, format constraints). No other source produces `Spec`.
- **Interop** — compatibility signals with consumer/validator behavior such as MobilityData, GTFS Guru, Google Transit, or a regional profile (e.g., GTFS-JP).
- **Quality** — GTFS best-practice, data quality, readability, consistency, and production-quality checks.
- **Analytics** — statistical, operational, performance, or analysis-oriented signals.

Each rule also carries a machine-readable **authority source** (`authority_source`) field (`GTFS_SPEC`, `MOBILITYDATA_PARITY`, `REGIONAL_PROFILE`, `PROJECT_QUALITY`, etc.). Invariant: **the `Spec` class is legitimate only with `authority_source = GTFS_SPEC`**; parity with MobilityData/Guru/Google, best-practice, or project-specific heuristics is not on its own proof of Spec.

### Optional Profiles and Source URL

Setting `stop_name_best_practices=true` in the config delta enables the language-dependent `STP_040` and `STP_041` checks; they are disabled by default because of their false-positive risk. URL-based integrations may provide `source_url` metadata, allowing `ARC_028` to verify that the permanent publishing URL contains a `.zip` filename. Upload-only validation skips this check. The core engine never requests URLs found inside a feed; HTTP availability checks require a separate, explicitly opt-in online adapter.

### Speed Thresholds

| Parameter | Default | Range | Description |
|---|---:|---|---|
| Max Bus Speed | 120 km/h | 60–200 | Maximum allowed speed for bus trips |
| Max Tram Speed | 100 km/h | 40–160 | Maximum allowed speed for tram trips |
| Max Metro Speed | 150 km/h | 80–250 | Maximum allowed speed for metro trips |
| Max Rail Speed | 300 km/h | 100–400 | Maximum allowed speed for rail trips |
| Max Ferry Speed | 80 km/h | 20–150 | Maximum allowed speed for ferry trips |
| Max Cable Car Speed | 30 km/h | 10–60 | Maximum allowed speed for cable car / funicular |

### Geographic and Transfer Thresholds

| Parameter | Default | Range | Description |
|---|---:|---|---|
| Min Transfer Time | 180 s | 30–1800 | Minimum connection time for transfers |
| Max Transfer Distance | 500 m | 50–2000 | Maximum distance for a transfer to be considered valid |
| Max Route Jump | 10 km | 1–50 | Maximum distance between consecutive shape points |
| Nearby Stop Threshold | 5 m | 1–20 | Stops closer than this are flagged as duplicates |
| Stop-to-Shape Distance | 100 m | 20–500 | Maximum distance a stop may be from its shape |
| Parent Station Distance | 100 m | 10–1000 | Maximum distance a stop may be from its parent station |

### Service and Operational Thresholds

| Parameter | Default | Range | Description |
|---|---:|---|---|
| Expiry Warning | 30 days | 1–60 | Warning generated if feed expires within this many days |
| Service Gap Threshold | 7 days | 3–30 | Service gaps longer than this are flagged |
| Max Trip Duration | 24 h | 8–72 | Maximum duration for a single trip |
| Min Trip Duration | 60 s | 10–300 | Minimum duration for a single trip |
| Max Headway | 240 min | 60–720 | Headways longer than this generate a warning |
| Bunching Threshold | 2 min | 1–10 | Headways shorter than this are flagged as bunching |

---

## Scores

### Publish Score (0–100)

Measures whether the feed is publishable per the official GTFS Schedule Reference. The score **starts at 100**; each publication-blocking issue deducts a penalty proportional to the rule's weight and remediation cost.

**How the score is computed:**
- Only `Spec` class issues at `Critical` severity (the official GTFS spec gate) affect the Publish Score. `Interop` compatibility signals are reported separately (Interop Score / R8).
- If the same rule fires multiple times, the penalty is capped at **2×**; a single issue cannot drive the score to zero.
- **0–40:** Feed is likely unusable. Blocker errors present.
- **40–70:** Partial issues; some applications may reject the feed.
- **70–90:** Usable, but attention is needed.
- **90–100:** Ready to publish.

### Overall Score (0–100)

Weighted average of all four analysis classes: Spec×40% + Interop×30% + Quality×20% + Analytics×10%. Reflects both specification compliance and operational data quality. A feed can be publishable while still having a low Overall Score.

**How the score is computed:**
- Issues from all four classes (Spec, Interop, Quality, Analytics) affect this score according to their weights.
- Missing optional fields, inconsistent service patterns, and accessibility gaps are reflected here via the Quality and Analytics components.
- **0–60:** Significant quality issues; passenger experience may be affected.
- **60–80:** Moderate quality; improvements recommended.
- **80–100:** Good data quality.

> **Note:** The Publish Score and the Overall Score serve different purposes and are computed with different formulas. A feed with a high Publish Score but a low Overall Score works technically, but issues such as missing accessibility information or incorrect route names affect passengers.

---

## Report Tabs

### 1. Report
Summary overview: both scores, feed metrics (route count, trip count, date range, etc.), and a notice distribution chart.

### 2. Detail & Fix
Issues are presented as a prioritized fix queue, sorted by priority score. Each row contains:

| Column | Description |
|---|---|
| **Score** | Priority score — computed as `Severity × (1 + Dependent) × log₂(1 + Count) / Effort`; higher = fix first |
| **+Pub** | Publish Score gain if this rule is fixed |
| **+Score** | Overall Score gain if this rule is fixed |
| **Dependent** | How many other active rules close automatically if this one is fixed |
| **Effort** | Fix effort: 1 = single field change, 2 = limited cross-file, 3 = structural / data model revision |

The sum of all +Pub values equals `100 − current Publish Score`; the sum of all +Score values equals `100 − current Overall Score`. Geographic issues show a map icon; clicking it displays the problem location and related shape/stop data on an interactive map. Clicking the **rule code** opens the relevant GTFS specification section in a new tab — the reference page of the file the finding most affects (gtfs.jp for GTFS-JP rules).

### 3. By Category
All rule violations listed by group and class. Each row shows the rule code, title, affected record count, severity, and remediation guidance. Filtering and sorting are supported.

### 4. Export
Download the report as HTML, CSV, or JSON. The PDF option opens the browser's print dialog — use "Save as PDF" to export.

---

## Interactive GTFS File Map

GTFS Validator & Analyzer includes an interactive File Map that combines the GTFS data structure with the real validation findings of the analyzed feed.

This view is not a static schema. It shows the files present in the feed, the missing ones, the findings, and the validated file relationships based on the analysis result.

### Features

- Shows the seven core GTFS files in the **Calendar** and **Core Service** groups
- Shows non-core standard files only when the analyzer reports a finding for them
- Lists non-spec files found in the feed in a separate group
- Visualizes validated GTFS relationships such as `route_id`, `trip_id`, `stop_id`, `service_id`, and `shape_id`
- Colors files by their highest finding severity
- Distinguishes missing, clean, and problematic files
- Shows row count, file size, finding count, and severity distribution
- Lists findings by rule, always ordered **Critical → High → Medium → Low → Info**
- Opens all findings of the selected file in a filtered Detail & Fix view
- Provides file-presence and severity filters
- Supports zoom, fit-to-screen, dark theme, and a mobile layout

When a file is selected, only its validated and related GTFS connections expand. Non-spec files stay visible, but no unvalidated relationships are drawn.

Analysis and visualization run entirely in the browser. GTFS files are never uploaded to any server.

![GTFS File Map](docs/images/gtfs-file-map.png)

---

## Run-to-Run Comparison

GTFS Validator & Analyzer can compare two analyses of the same feed (before/after) to show what a fix round improved and what it regressed. Open **Compare** and upload the **Golden JSON** you downloaded from an earlier analysis; the diff is computed against the current run.

### Features

- Shows the before/after change in Publish, Overall, and sub-scores (Spec, Interop, Quality, Analytics)
- Classifies each rule as **Fixed, Decreased, Increased, New, or Same**, with filtering and search
- Shows the change in severity (Critical → Info) and class (Spec/Interop/Quality/Analytics) distribution
- Compares feed structure (trip, stop, `stop_times` and `calendar_dates` row counts) and feed/service date ranges
- Normalizes notice density **per 1,000 trips** and **per 100,000 stop_times**, so feeds of different sizes are comparable
- Warns when the two runs differ in feed name, date range, or configuration, so a misleading delta is not misread
- Exports the comparison as CSV
- Also reads legacy Golden schemas (v1–v3)

The comparison runs entirely in the browser. The Golden JSON is parsed locally; nothing is uploaded to any server.

---

## Rule Classes

| Class | What it measures | Affects |
|---|---|---|
| **Spec** | Deviations from the GTFS specification — missing required fields, invalid values, referential integrity errors | Publish Score |
| **Interop** | Spec-compliant but rejected or misinterpreted by common consumers (Google Maps, Apple Maps, etc.) | Publish Score |
| **Quality** | Missing optional but expected fields, inconsistencies, deviations from best practices | Overall Score |
| **Analytics** | Service pattern analysis — bunching, sparse service, expired service | Overall Score |

---

## Severity Levels

| Level | Meaning |
|---|---|
| **Critical** | Makes the feed unusable or causes data loss |
| **High** | Significant functional issue; strongly recommended to fix |
| **Medium** | Inconsistency that warrants attention |
| **Low** | Minor deviation from best practice |
| **Info** | Informational; action may not be required |

Severity levels are based on the file and field requirement levels (Required · Conditionally Required · Recommended · Optional) defined in the [GTFS Schedule Reference](https://gtfs.org/documentation/schedule/reference/#file-requirements).

For GTFS-JP feeds, the **JPN** group rules are based on the official [GTFS-JP specification](https://www.gtfs.jp/) (gtfs.jp).

---

## Notice Limits

In large feeds the same rule can fire thousands of times. Unlimited notice lists strain browser memory and reduce readability. A two-tier cap is applied:

| Limit | Value | Scope |
|---|---|---|
| Per rule (default) | 500 | All rules |
| Per rule (high) | 2,000 | `TRP_020`, `OPR_007`, `STP_016`, `STP_017` |
| Total (all rules) | 100,000 | Feed-wide — validation stops if exceeded |

Rules in the high-cap list naturally produce large counts in real feeds (e.g., one headway record per trip). When a rule hits its cap, the actual violation count appears in the **Total** column of the Fix Queue; on the All Findings tab, selecting that rule filter shows a yellow warning banner.

---

## Rule Groups

Each rule is coded as `GROUP_NNN`. Groups follow GTFS file and component boundaries.

| Group | GTFS Component | Description |
|---|---|---|
| **ARC** | Archive / file level | ZIP extraction, file format, required file presence, character encoding |
| **AGN** | `agency.txt` | Agency information and multi-agency consistency |
| **CAL** | `calendar.txt` | Service calendars and weekly day patterns |
| **CLD** | `calendar_dates.txt` | Service exception dates and date validity |
| **STP** | `stops.txt` | Stop locations, hierarchy, and accessibility information |
| **RTS** | `routes.txt` | Route definitions, route type, color, and naming |
| **TRP** | `trips.txt` | Trip definitions, block and shape associations |
| **STM** | `stop_times.txt` | Stop timings, speed, sequence, and timing consistency |
| **SHP** | `shapes.txt` | Route shapes, point order, and stop alignment |
| **FRQ** | `frequencies.txt` | Frequency-based trips and headway values |
| **TRF** | `transfers.txt` | Transfer definitions, types, and duration validity |
| **FAR** | `fare_attributes.txt` | Fare definitions, currency, and payment method |
| **FRL** | `fare_rules.txt` | Route- and zone-based fare rules |
| **FIN** | `feed_info.txt` | Feed publisher information, language, validity dates |
| **PTH** | `pathways.txt` | In-station pathway network and accessibility connections |
| **LVL** | `levels.txt` | Station floors and elevator/stairway relationships |
| **TRN** | `translations.txt` | Field translations and language consistency |
| **ATR** | `attributions.txt` | Data source and attribution information |
| **XFL** | Cross-file | Cross-file referential integrity and consistency |
| **GEO** | Geographic analysis | Coordinate consistency, outlier detection, clustering |
| **OPR** | Operational analysis | Inter-trip wait times, route density, stop repetition |
| **VAT** | Network topology | Isolated stops, disconnected routes, network accessibility |
| **DQ** | Feed-wide quality | General data quality metrics and threshold checks |
| **RCT** | `rider_categories.txt` | Rider categories, age ranges, and default category (Fares v2) |
| **FMD** | `fare_media.txt` | Payment media: physical card, mobile app, EMV, etc. (Fares v2) |
| **FPD** | `fare_products.txt` | Fare products, amount, currency, and media/category associations (Fares v2) |
| **FLG** | `fare_leg_rules.txt` | Per-leg fare rules and priority (Fares v2) |
| **FTR** | `fare_transfer_rules.txt` | Transfer fare rules and time limits (Fares v2) |
| **ARS** | `areas.txt` | Geographic area definitions (Fares v2) |
| **SAR** | `stop_areas.txt` | Stop–area mappings (Fares v2) |
| **NET** | `networks.txt` | Network definitions (Fares v2) |
| **TFR** | `timeframes.txt` | Time frame groups and service calendar associations (Fares v2) |
| **BKR** | `booking_rules.txt` | Demand-responsive booking rules, advance notice windows, and booking types (GTFS Flex) |
| **PDW** | Flex window rules | Demand-responsive pickup/drop-off time window consistency in `stop_times.txt` (GTFS Flex) |
| **LOC** | `locations.geojson` | Geometry and format validation of flexible service zones (GTFS Flex) |
| **GGL** | Google Transit specific | Rules additionally required or restricted by Google Maps and Google Transit |
| **JPN** | GTFS-JP profile | Japan national GTFS-JP profile rules — kana readings, `office_jp.txt`/`agency_jp.txt` referential integrity (GTFS-JP feeds only) |

---

## Developer Setup

### Requirements

- **Rust** — GNU toolchain (`stable-x86_64-pc-windows-gnu`), MinGW gcc
- **wasm-pack** — WASM build tool
- **Node.js** — a maintained LTS release (exact range in `ui/package.json` > `engines`)

> **Windows note:** The GNU toolchain is required instead of MSVC. During the WASM build, `wasm-opt` is downloaded and this step is incompatible with the MSVC linker. MinGW `gcc` must be on the PATH.

```powershell
# Rust GNU toolchain (once)
rustup toolchain install stable-x86_64-pc-windows-gnu
rustup override set stable-x86_64-pc-windows-gnu
```

### Build

```powershell
# 1. Install dependencies
cd ui
npm install

# 2. Compile WASM
npm run wasm

# 3. Compile UI
npm run build
# Output: ui/dist/
```

### Dev Server

```powershell
cd ui
npm install
npm run dev
```

### Tests

```powershell
# Rust unit and integration tests
cargo test

# Playwright smoke tests
cd ui
npx playwright test
```

## Project Structure

```
gtfs-validator/
├── crates/
│   ├── config/     # Configuration types
│   ├── core/       # Shared data structures and result model
│   ├── pipeline/   # Validation pipeline (k1–k7 stages)
│   ├── rules/      # Rule definitions and registry (539 rules, 37 groups)
│   └── wasm/       # wasm-bindgen WASM output
└── ui/             # Vite + TypeScript frontend
    ├── pkg/          # wasm-pack output (generated, committed)
    ├── src/
    │   └── pages/    # Application tabs (domain/fix/rules/export)
    └── tests/        # Playwright tests
```

## License

MIT — see [LICENSE](LICENSE) for details.
