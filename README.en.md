# GTFS Analyzer

🇹🇷 [Türkçe](README.md) · 🇬🇧 **English** · 🇯🇵 [日本語](README.ja.md)

[![Open App](https://img.shields.io/badge/Open%20App-gtfs--analyzer-2ea44f?style=flat&logo=googlechrome&logoColor=white)](https://ttezer.github.io/gtfs-analyzer/)
[![GTFS-JP](https://img.shields.io/badge/GTFS--JP-supported-c8102e?style=flat)](https://www.gtfs.jp/)
[![GTFS Spec](https://img.shields.io/badge/GTFS-Spec-007ec6?style=flat)](https://gtfs.org/)
[![License MIT](https://img.shields.io/badge/license-MIT-yellow?style=flat)](LICENSE)

GTFS Analyzer is an open-source tool that validates and analyzes GTFS files directly in the browser. The uploaded .zip file is never sent to any server; all processing is performed on the user's device via WebAssembly.

GTFS Analyzer does not merely check whether a file conforms to the specification; it also analyzes how reliable, consistent, and usable the feed is. It shows errors together with the relevant file and line number, provides remediation steps for each finding, and marks geographic issues — such as deviating routes, broken coordinates, or unreachable stops — on an interactive map.

Every finding is tagged with a rule code, an analysis class, and a severity level. Thanks to the Spec · Interop · Quality · Analytics classes and the Critical → Info severity levels, thousands of findings can be filtered, prioritized, and handled systematically. The tool also automatically detects the GTFS features used by the feed — Shapes, Transfers, Fares, Headsigns, Flex, and the like — and includes them in the report.

GTFS Analyzer extends specification validation with operational quality analysis. Frequency inconsistencies per route, anomalous speed segments, isolated stops, gaps in service patterns, andnetwork topology problems are examined with 526 distinct validation and analysis rules. Results are summarized with separate scores that evaluate compliance and quality independently. The prioritized fix queue shows which issues should be addressed first and the likely impact of each fix on the score.

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
| Platform | Web | Web, CLI, Desktop | Web *(CLI, Desktop planned)* |
| **Total rules** | **178** | **~120** | **520** |

### Feed Analysis Examples

The same feeds were compared with three validators: MobilityData gtfs-validator v8.0.1 · GTFS Guru v0.1.0 · GTFS Analyzer v0.1.2. (GTFS Analyzer figures are a snapshot from an analysis run on 2026-06-15; because some rules are date-dependent, running on a different day may produce small deviations.)

#### BART (Bay Area Rapid Transit, San Francisco)

Feed: `BART (San Francisco).zip` — version downloaded on 2026-05-25 (validity range: 2026-01-12–2026-08-07) · 14 routes, 287 stops, 4,455 trips.

| | MobilityData | GTFS Guru | GTFS Analyzer |
|---|---:|---:|---:|
| Total notices | 2,725 | 2,663 | 3,227 |
| Critical / Error | 2 | 1 | 2 |
| High / Warning | 2,655 | 2,655 | 141 |
| Medium | — | — | 1,058 |
| Low | — | — | 987 |
| Info | 68 | 7 | 1,039 |
| Distinct rule types triggered | 13 | 10 | **47** |
| Publish score | — | — | **92.6 / 100** |
| Quality score | — | — | **82.6 / 100** |

#### TriMet (Portland, Oregon)

Feed: `trimet.zip` — version downloaded on 2026-06-04 (validity range: 2026-04-26–2026-08-22) · 89 routes, 6,395 stops, 48,146 trips.

| | MobilityData | GTFS Guru | GTFS Analyzer |
|---|---:|---:|---:|
| Total notices | 48 | 116 | 4,966 |
| Critical / Error | 0 | 0 | 36 |
| High / Warning | 39 | 107 | 1,655 |
| Medium | — | — | 1,379 |
| Low | — | — | 1,018 |
| Info | 9 | 9 | 878 |
| Distinct rule types triggered | 7 | 7 | **57** |
| Publish score | — | — | **80.6 / 100** |
| Quality score | — | — | **73.9 / 100** |

> ⚠️ **Fares v2:** GTFS Analyzer reports Fares v2 referential-integrity problems as critical — for example, a `network_id` in `fare_leg_rules.txt` not defined in `networks.txt` (detailed coverage via the FAR/FPD/FLG/FTR/RCT/FMD groups). MobilityData also validates Fares v2 (schema + referential integrity + fare_transfer/products/media/timeframes rules), though coverage and severity classification differ.
>
> ℹ️ The critical count in the table above predates a **2026-06-18 fix** of a false positive related to the `fare_products` composite primary key (fare_product_id + rider_category_id + fare_media_id) and does not reflect the current code; it will be updated when the comparison is re-run.

#### Tokyo Toei (Tokyo Metropolitan Bureau of Transportation)

Feed: `tokyo_toei_bus.zip` — feed_version 2026-06-06 (validity range: 2026-06-06–2029-06-05) · 150 routes, 5,367 stops, 54,315 trips.

| | MobilityData | GTFS Guru | GTFS Analyzer |
|---|---:|---:|---:|
| Total notices | 1,637 | 4,137 | 7,231 |
| Critical / Error | 0 | 0 | 0 |
| High / Warning | 265 | 4,128 | 1,060 |
| Medium | — | — | 3,324 |
| Low | — | — | 1,968 |
| Info | 1,372 | 9 | 879 |
| Distinct rule types triggered | 9 | 6 | **58** |
| Publish score | — | — | **94.3 / 100** |
| Quality score | — | — | **72.5 / 100** |

> 🗾 **Spec-clean but operationally dense:** All three tools report 0 critical — the feed is specification-clean. The difference is in the analytics layer: most of GTFS Analyzer's medium/low findings are operational signals from the three-year validity window (2026–2029) and dense network/shape patterns, which MobilityData and GTFS Guru largely summarize as warnings/info.

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

GTFS Analyzer is a web application — no installation required. Open the live version in your browser and upload your GTFS zip file.

**→ [https://ttezer.github.io/gtfs-analyzer/](https://ttezer.github.io/gtfs-analyzer/)**

1. Drag and drop your GTFS zip file, or use the file picker.
2. Validation starts automatically; progress is shown step by step on screen.
3. When complete, the Publish and Quality scores appear alongside four tabs: **Report**, **Detail & Fix**, **By Category**, **Export**.

> To self-host or set up a development environment, see [Developer Setup](#developer-setup).

---

## Analysis Thresholds

Validation thresholds can be customized from the **Analysis Thresholds** section on the upload screen. Changed values take effect on the next ZIP upload; the reset button restores defaults.

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

Measures how consumable the feed is by transit applications. The score **starts at 100**; each blocker issue deducts a penalty proportional to the rule's weight and remediation cost.

**How the score is computed:**
- Only `Spec` and `Interop` class issues at `Critical` and `High` severity affect the Publish Score.
- If the same rule fires multiple times, the penalty is capped at **2×**; a single issue cannot drive the score to zero.
- **0–40:** Feed is likely unusable. Blocker errors present.
- **40–70:** Partial issues; some applications may reject the feed.
- **70–90:** Usable, but attention is needed.
- **90–100:** Ready to publish.

### Quality Score (0–100)

Measures data quality and adherence to best practices beyond specification compliance. A feed can be publishable while still having a low Quality Score.

**How the score is computed:**
- `Quality` and `Analytics` class issues affect this score.
- Missing optional fields, inconsistent service patterns, and accessibility gaps are reflected here.
- **0–60:** Significant quality issues; passenger experience may be affected.
- **60–80:** Moderate quality; improvements recommended.
- **80–100:** Good data quality.

> **Note:** The two scores are independent. A feed with a high Publish Score but a low Quality Score works technically, but issues such as missing accessibility information or incorrect route names affect passengers.

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
| **+Quality** | Quality Score gain if this rule is fixed |
| **Dependent** | How many other active rules close automatically if this one is fixed |
| **Effort** | Fix effort: 1 = single field change, 2 = limited cross-file, 3 = structural / data model revision |

The sum of all +Pub values equals `100 − current Publish Score`; the sum of all +Quality values equals `100 − current Quality Score`. Geographic issues show a map icon; clicking it displays the problem location and related shape/stop data on an interactive map. Clicking the **rule code** opens the relevant GTFS specification section in a new tab — the reference page of the file the finding most affects (gtfs.jp for GTFS-JP rules).

### 3. By Category
All rule violations listed by group and class. Each row shows the rule code, title, affected record count, severity, and remediation guidance. Filtering and sorting are supported.

### 4. Export
Download the report as HTML, CSV, or JSON. The PDF option opens the browser's print dialog — use "Save as PDF" to export.

---

## Rule Classes

| Class | What it measures | Affects |
|---|---|---|
| **Spec** | Deviations from the GTFS specification — missing required fields, invalid values, referential integrity errors | Publish Score |
| **Interop** | Spec-compliant but rejected or misinterpreted by common consumers (Google Maps, Apple Maps, etc.) | Publish Score |
| **Quality** | Missing optional but expected fields, inconsistencies, deviations from best practices | Quality Score |
| **Analytics** | Service pattern analysis — bunching, sparse service, expired service | Quality Score |

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
- **Node.js** 18+

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
│   ├── rules/      # Rule definitions and registry (526 rules, 37 groups)
│   └── wasm/       # wasm-bindgen WASM output
└── ui/             # Vite + TypeScript frontend
    ├── pkg/          # wasm-pack output (generated, committed)
    ├── src/
    │   └── pages/    # Application tabs (domain/fix/rules/export)
    └── tests/        # Playwright tests
```

## License

MIT — see [LICENSE](LICENSE) for details.
