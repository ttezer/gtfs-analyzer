# GTFS Analyzer

→ [Türkçe sürüm](README.md)

An open-source, fully client-side GTFS (General Transit Feed Specification) validator that runs entirely in the browser. The uploaded zip file is never sent to a server — all processing happens in WebAssembly inside the user's browser.

Most existing GTFS validators only check specification compliance and produce an error list. GTFS Analyzer goes several steps further: it shows exactly which file and line number contains a problem, provides step-by-step remediation guidance for each issue, and marks geographic errors (route deviations, coordinate anomalies, unreachable stops, etc.) on an interactive map. Every finding is tagged with a file- and component-level rule code (`ARC_`, `STP_`, `STM_`...), one of four classes (Spec · Interop · Quality · Analytics), and one of five severity levels (Critical → Info) — making it easy to filter, prioritize, and automate across thousands of findings. The GTFS features used by a feed (Shapes, Transfers, Fares, Headsigns, Flex, etc.) are detected automatically and reflected in the report.

Beyond specification compliance, it also measures operational quality: frequency inconsistencies per route, anomalous speed segments, isolated stops, service pattern gaps, and network topology issues — across 445 rules. Results are summarized with two independent scores; the fix queue automatically answers "what should I fix first?" and shows each fix's contribution to the score.

**Who uses it:**
- Transit operators and municipalities — before publishing a feed
- GTFS integrators and consultants — to verify delivery quality
- Application developers — to assess the reliability of feeds they consume
- Researchers and analysts — to benchmark network quality

---

## Comparison with Other Tools

### Feature Matrix

| Feature | MobilityData | France Transport | GTFS Guru | GTFS Analyzer |
|---|:---:|:---:|:---:|:---:|
| Web interface | ✅ | ✅ | ✅ | ✅ |
| Data never leaves the browser | ❌ | ❌ | ✅ | ✅ |
| Spec compliance rules | ✅ | ✅ | ✅ | ✅ |
| Quality rules | ❌ | Partial | ❌ | ✅ |
| Operational analytics | ❌ | ❌ | ❌ | ✅ |
| Map visualization | ❌ | Stops | ❌ | Stops, routes, trips, lines, pathways |
| Feed score | ❌ | ❌ | ❌ | ✅ |
| Remediation guidance | Partial | ❌ | ❌ | ✅ |
| GTFS Flex support | Partial | ❌ | ❌ | ✅ |
| Output formats | HTML, JSON | Web (permalink) | HTML, JSON | HTML, CSV, JSON, PDF |
| Platform | Web | Web | Web, CLI, Desktop | Web *(CLI, Desktop planned)* |
| **Total rules** | **~120** | **~80** | **~120** | **445** |

### BART GTFS Feed Example

The BART (Bay Area Rapid Transit, San Francisco) feed was tested with four validators.  
Feed: `BART (San Francisco).zip` — version downloaded on 2026-05-25 (validity range: 2026-01-12–2026-08-07).  
Versions used: MobilityData gtfs-validator v8.0.1 · France Transport (transport.data.gouv.fr, May 2026) · GTFS Guru v0.1.0 · GTFS Analyzer v0.1.2.

| | MobilityData | France Transport | GTFS Guru | GTFS Analyzer |
|---|---:|---:|---:|---:|
| Total notices | 2,725 | 6 ⚠️ | 2,663 | 6,684 |
| Critical / Error | 2 | 1 | 1 | 3 |
| High / Warning | 2,655 | 0 | 2,655 | 5,128 |
| Medium | — | — | — | 554 |
| Low | — | — | — | 500 |
| Info | 68 | 5 | 7 | 499 |
| Distinct rule types triggered | 13 | 2 | 13 | **45** |
| Publish score | — | — | — | **79.4 / 100** |
| Quality score | — | — | — | **80.1 / 100** |

> ⚠️ France Transport could not complete validation due to a missing `rider_category_name` field.

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
| **Effort** | Fix effort: 1 = single field change, 5 = major data model revision |

The sum of all +Pub values equals `100 − current Publish Score`; the sum of all +Quality values equals `100 − current Quality Score`. Geographic issues show a map icon; clicking it displays the problem location and related shape/stop data on an interactive map.

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
│   ├── rules/      # Rule definitions and registry (445 rules, 36 groups)
│   └── wasm/       # wasm-bindgen WASM output
└── ui/             # Vite + TypeScript frontend
    ├── pkg/          # wasm-pack output (generated, committed)
    ├── src/
    │   └── pages/    # Application tabs (domain/fix/rules/export)
    └── tests/        # Playwright tests
```

## License

MIT — see [LICENSE](LICENSE) for details.
