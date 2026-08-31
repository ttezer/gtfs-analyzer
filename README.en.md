# GTFS Validator & Analyzer

🇹🇷 [Türkçe](README.md) · 🇬🇧 **English** · 🇯🇵 [日本語](README.ja.md) · 🇫🇷 [Français](README.fr.md)

[![Open App](https://img.shields.io/badge/Open%20App-gtfs--analyzer-2ea44f?style=flat&logo=googlechrome&logoColor=white)](https://ttezer.github.io/gtfs-analyzer/)
[![GTFS-JP](https://img.shields.io/badge/GTFS--JP-v3%2Fv4%20supported-c8102e?style=flat)](https://www.gtfs.jp/)
[![Rule count](https://img.shields.io/badge/rules-611-blue?style=flat)](RULES.en.md)
![GTFS Spec coverage](https://img.shields.io/badge/GTFS%20Spec-97.2%25-007ec6?style=flat)
[![Corpus validation](https://img.shields.io/badge/corpus-4%2C318%20feeds%20%C3%97%2012%20runs-brightgreen?style=flat)](audit-results/)
[![crates.io](https://img.shields.io/crates/v/gtfs-analyzer?style=flat&label=crates.io)](https://crates.io/crates/gtfs-analyzer)
[![npm](https://img.shields.io/npm/v/gtfs-sdk?style=flat&label=npm)](https://www.npmjs.com/package/gtfs-sdk)
[![License MIT](https://img.shields.io/badge/license-MIT-yellow?style=flat)](LICENSE)

GTFS Validator & Analyzer is an open-source GTFS validator and feed quality analyzer. The uploaded `.zip` file is never sent to any server; all validation runs on the user's device via WebAssembly. It is available as a browser application, a CLI (`cargo install gtfs-analyzer`), a Rust library, a CI/CD gate, and the `gtfs-sdk` npm package.

The project covers **97.2% of the measurable GTFS Specification requirements** and anchors all 300 atoms in the field inventory to at least one Spec rule. Of its **611 rules**, **417** produced at least one finding in the most recent full 4,318-feed catalog run; the GTFS-JP additions were measured separately on a 585-feed profile run. Every rule is listed in [`RULES.en.md`](RULES.en.md).

Accuracy is tested against MobilityData's official `gtfs-validator` through **twelve full catalog runs**. Each run validates every testable GTFS Schedule feed in the catalogue — **4,318** as of the most recent run — with both validators on the same machine and date, using the actual Java `gtfs-validator v8.0.1`. The raw outputs are available in [`audit-results/`](audit-results/).

GTFS Validator & Analyzer does not merely check whether a file conforms to the specification; it also analyzes how reliable, consistent, and usable the feed is. It shows errors together with the relevant file and line number, provides remediation steps for each finding, and marks geographic issues — such as deviating routes, broken coordinates, or unreachable stops — on an interactive map.

Every finding is tagged with a rule code, an analysis class, and a severity level. Thanks to the Spec · Interop · Quality · Analytics classes and the Critical → Info severity levels, thousands of findings can be filtered, prioritized, and handled systematically. The tool also automatically detects the GTFS features used by the feed — Shapes, Transfers, Fares, Headsigns, Flex, and the like — and includes them in the report.

GTFS Validator & Analyzer extends specification validation with operational quality analysis. Frequency inconsistencies per route, anomalous speed segments, isolated stops, gaps in service patterns, and network topology problems are examined with 611 distinct validation and analysis rules. Results are summarized with scores for publishability and overall feed quality. The prioritized fix queue shows which issues should be addressed first and the likely impact of each fix on the score.

**Who is it for?**

- **Transit operators and municipalities** — To validate a feed and resolve quality issues before publishing.
- **GTFS integrators and consultants** — To document the technical and operational quality of delivered data.
- **Application developers** — To assess the reliability and integration risks of the feeds they consume.
- **Researchers and analysts** — To compare different transit networks in terms of data quality and structure.

---

## Comparison with Other Tools

### Feature Matrix

| Feature | MobilityData | GTFS Analyzer |
|---|:---:|:---:|
| Web interface | ✅ | ✅ |
| Data never leaves the browser | ❌ | ✅ |
| Spec compliance rules | ✅ | ✅ |
| Quality rules | ❌ | ✅ |
| Operational analytics | ❌ | ✅ |
| Map visualization | ❌ | Stops, routes, trips, lines, pathways |
| Feed score | ❌ | ✅ |
| Remediation guidance | Partial | ✅ |
| GTFS Flex support | Partial | ✅ |
| Fares v2 validation | Partial | ✅ |
| GTFS-JP profile validation | ❌ | ✅ |
| Output formats | HTML, JSON | HTML, CSV, JSON, PDF |
| Distribution | Web · desktop installers (msi/dmg/deb) · CLI JAR · Docker | Web · CLI binary · `cargo install` · npm SDK |
| Documented CI/CD integration | Not documented in the README (possible via Docker/CLI) | ✅ `--fail-on` + exit codes |
| npm package | ❌ | ✅ `gtfs-sdk` |
| crates.io package | — *(Java project)* | ✅ `gtfs-analyzer` |
| GTFS Spec coverage (measured) | — | **97.2%** · 300/300 field anchors |
| **Total rules** | **178** | **611** |

### Corpus Validation

Accuracy cannot be shown with a handful of feeds. Every release is run against the **entire MobilityDatabase GTFS Schedule catalogue** — **4,318 feeds** in the most recent run, over 640 parallel shards. On the other side is MobilityData's **`gtfs-validator` v8.0.1**, executed again on the same archive rather than read from its published reports, so the difference is "who found what", not "whose report was generated when".

From the most recent run (`32587015142`, both validators clean on 4,275 feeds):

| | GTFS Analyzer | MobilityData |
|---|---|---|
| Median wall time | **0.05 s** | 3.00 s |
| Median peak memory | **14 MB** | 329 MB |
| Feeds not completed | **1** | 10 |
| Facts MobilityData saw and we did not | **0** | — |

Raw output is under [`audit-results/`](audit-results/) — the first seven runs are committed, later ones are archived as an `audit-<run-id>` prerelease.

### Feed Analysis Examples

The figures below come from the latest corpus run: the same archive and the same analysis date (2026-08-20), with MobilityData running Java `gtfs-validator v8.0.1`.

#### BART (Bay Area Rapid Transit, San Francisco)

Feed: `mdb-53` · 14 routes, 287 stops, 4,417 trips · 0.9 MB

| | MobilityData | GTFS Analyzer |
|---|---:|---:|
| Total notices | 2,715 | 740 |
| Critical / Error | 2 | 2 |
| High / Warning | 2,654 | 1 |
| Medium | — | 11 |
| Low | — | 24 |
| Info | 59 | 702 |
| Distinct rule types triggered | 13 | **37** |
| Validation time | 3.43 s | **0.19 s** |
| Publish score | — | **92.6 / 100** |
| Overall score | — | **90.9 / 100** |

#### TriMet (Portland, Oregon)

Feed: `mdb-247` · 112 routes, 6,480 stops, 70,557 trips · 28.4 MB

| | MobilityData | GTFS Analyzer |
|---|---:|---:|
| Total notices | 51 | 3,099 |
| Critical / Error | 0 | 0 |
| High / Warning | 38 | 12 |
| Medium | — | 97 |
| Low | — | 497 |
| Info | 13 | 2,493 |
| Distinct rule types triggered | 8 | **49** |
| Validation time | 14.85 s | **5.46 s** |
| Publish score | — | **100 / 100** |
| Overall score | — | **90.0 / 100** |

> This is a specification-clean feed: both tools report zero Critical findings and a Publish Score of 100. The difference in rule counts reflects GTFS Analyzer's additional operational-quality analysis.

#### Tokyo Toei (Tokyo Metropolitan Bureau of Transportation)

Feed: `mdb-3175` · 151 routes, 5,370 stops, 68,817 trips · 8.6 MB · **GTFS-JP profile**

| | MobilityData | GTFS Analyzer |
|---|---:|---:|
| Total notices | 1,849 | 1,741 |
| Critical / Error | 0 | 0 |
| High / Warning | 268 | 12 |
| Medium | — | 809 |
| Low | — | 548 |
| Info | 1,581 | 372 |
| Distinct rule types triggered | 8 | **49** |
| Validation time | 5.94 s | **1.75 s** |
| Publish score | — | **100 / 100** |
| Overall score | — | **87.2 / 100** |

> The GTFS-JP profile produces no false positives on this real Japanese feed: it is specification-clean (0 Critical, Publish Score 100), and profile rules inspect only Japan-specific requirements.

#### VBB (Berlin-Brandenburg Transport Association)

Feed: `mdb-782` · 1,274 routes, 41,961 stops, 258,524 trips, 14,485 shapes · **~75 MB**

| | MobilityData | GTFS Analyzer |
|---|---:|---:|
| Total notices | 12,201 | 25,369 |
| Critical / Error | 0 | 0 |
| High / Warning | 11,486 | 1,307 |
| Medium | — | 7,440 |
| Low | — | 8,186 |
| Info | 715 | 8,436 |
| Distinct rule types triggered | 18 | **91** |
| Validation time | 45.16 s | **21.07 s** |
| Overall score | — | **78.4 / 100** |

> 🇩🇪 **Large feed:** MobilityData's hosted web validator cannot process a feed of this size. GTFS Analyzer validates it directly in the browser without sending the file to a server. More than half of MobilityData's total (`non_ascii_or_non_printable_char`) comes from valid German ü/ö/ä/ß characters; GTFS Analyzer does not flag valid Unicode letters. Core checks remain aligned.

---

## GTFS-JP Support

GTFS Analyzer automatically recognizes **GTFS-JP**, Japan's national GTFS profile (国土交通省 / MLIT standard), and enforces the requirements that GTFS-JP makes mandatory where standard GTFS leaves them optional. Because MLIT requires subsidized operators to publish GTFS-JP, hundreds of small operators must conform to this profile — yet mainstream validators do not check its profile-specific obligations.

**Automatic detection.** A feed is flagged as GTFS-JP — and a **GTFS-JP** badge appears in the report — when it contains the current GTFS-JP files (`agency_jp.txt`, `office_jp.txt`, `pattern_jp.txt`) or the legacy-compatible `routes_jp.txt`, when `feed_lang` starts with `ja`, or when `translations.txt` carries kana (`ja-Hrkt`) readings. `routes_jp.txt` is not a v3 file; it remains recognized only for legacy-feed compatibility. The default rule profile is **auto**; the web app, CLI, and WASM config can explicitly select `v3` or `v4`. Under v4, v3 extension files are reference data and their v3-specific JPN rules do not run. The profile rules activate only on GTFS-JP signals and stay silent on standard feeds.

**Selecting the profile for an analysis.** In the web app, open **Analysis Criteria** before choosing the ZIP and select `Auto`, `V3`, or `V4` under **GTFS-JP validation profile**. The current selection is committed when you choose a feed, before automatic validation starts; `Auto` is the default. For the CLI, use `--gtfs-jp-profile v3` or `--gtfs-jp-profile v4`. In the SDK, pass `config: { gtfs_jp_profile: 'v3' }` or `'v4'`. This selects the validation scope; it does not infer the feed's official GTFS-JP version. See the [GTFS-JP v3/v4 compatibility matrix](docs/gtfs-jp-v3-v4-matrix.md) for the detailed differences.

**Profile rules (JPN group).**

| Rule | Check |
|---|---|
| **JPN_001** | Kana reading (よみがな — `translations.txt`, `ja-Hrkt`) for stop names; required by GTFS-JP for voice announcements and search |
| **JPN_002** | `jp_office_id` (in `trips.txt` **or** `routes.txt`) must match an `office_id` defined in `office_jp.txt` (operating-office referential integrity) |
| **JPN_003** | `agency_jp.txt` `agency_id` must be defined in `agency.txt` (operator referential integrity) |
| **JPN_004** | `translations.txt` must be present — mandatory in GTFS-JP (notably for kana readings) |
| **JPN_005** | `office_name` (a required field) must be filled in `office_jp.txt` |
| **JPN_006** | `fare_attributes.txt` is required; `fare_rules.txt` is conditional when fare profiles differ |
| **JPN_007** | `feed_info.txt` must be present — mandatory in GTFS-JP |
| **JPN_008** | Kana (`ja-Hrkt`) reading for the route name (`route_long_name`) |
| **JPN_009** | Kana (`ja-Hrkt`) reading for `trip_headsign` |
| **JPN_010** | Kana (`ja-Hrkt`) reading for the operator name (`agency_name`) |
| **JPN_011** | `agency_id` is required even when the feed has only one agency |
| **JPN_012** | `agency_jp.agency_id` is required and must identify an `agency.txt` row |
| **JPN_013** | When present, `agency_zip_number` must contain exactly 7 ASCII digits |
| **JPN_014** | `office_jp.office_id` must be present and unique |
| **JPN_015** | Legacy `routes_jp.route_id` compatibility check; not a v3 file |
| **JPN_016** | `pattern_jp.route_update_date` and legacy `routes_jp.route_update_date` must be valid `YYYYMMDD` dates |
| **JPN_017** | `pattern_jp.jp_pattern_id` must be present and unique |
| **JPN_018** | When `pattern_jp.txt` exists, `trips.jp_pattern_id` must reference it |
| **JPN_019** | `ja-Hrkt` rows must use valid GTFS tables, fields, records, and stop-time sub-records |
| **JPN_020** | `office_url` and `office_phone` receive basic format quality checks |
| **JPN_021** | `ja-Hrkt` translations must be non-empty, consistent, and contain Japanese writing |
| **JPN_022** | GTFS-JP v4 requires `agency_lang`, `feed_start_date`, `feed_end_date`, and `feed_version` |

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

## Five Ways to Use It

The same validation core (`gtfs_pipeline::validate_bytes`) runs in five ways — all of them use the same 611 rules and produce the same result model:

| Path | Best for | Where the data goes |
|---|---|---|
| **Browser** ([app](https://ttezer.github.io/gtfs-analyzer/)) | Inspecting one feed with the map and report | **Nowhere** — on-device WebAssembly |
| **CLI** (`cargo install gtfs-analyzer`, or a prebuilt binary) | Batch validation, scripting, and Python integration | Nowhere — local binary |
| **Rust library** ([`gtfs-pipeline`](https://crates.io/crates/gtfs-pipeline)) | Embedding validation in your own Rust service | Nowhere — your own process |
| **CI/CD** (exit codes + `--fail-on`) | A release gate before publishing a feed | Nowhere — your own runner |
| **[`gtfs-sdk`](https://www.npmjs.com/package/gtfs-sdk) npm package** | Embedding validation in your web or Node application | Nowhere — local WASM |

The feed is never uploaded to a server in any of these modes. This makes it suitable for data that cannot leave your organization under policy or contract.

### CI/CD integration

The `--fail-on` flags fail the run only for the severity or class you choose, so Analytics findings do not break a release pipeline:

```yaml
# GitHub Actions — fail only on official GTFS Spec violations
- name: Validate GTFS feed
  run: |
    curl -sL https://github.com/ttezer/gtfs-analyzer/releases/latest/download/gtfs-analyzer-x86_64-linux.tar.gz | tar xz
    ./gtfs-analyzer validate feed.zip --fail-on-class spec --min-severity critical
```

Exit codes: `0` clean · `1` matching findings present · `2` feed/config/file error.

### Rust library

To embed validation in your own Rust service, use `gtfs-pipeline` directly — no CLI, no filesystem, no network:

```toml
[dependencies]
gtfs-pipeline = "0.11.1"
gtfs-config   = "0.11.1"
gtfs-core     = "0.11.1"
```

```rust
use gtfs_config::ValidatorConfig;
use gtfs_core::ValidateResult;
use gtfs_pipeline::validate_bytes;

let zip = std::fs::read("feed.zip")?;
let config = ValidatorConfig::default();

match validate_bytes(&zip, &config, 20_260_820) {
    ValidateResult::Ok(result) => {
        println!("notices: {}", result.notices.len());
        println!("publication score: {}", result.reports.r5.pub_score);
    }
    ValidateResult::Fatal(err) => eprintln!("fatal: {}", err.message),
}
```

`validate_bytes` takes bytes and returns a result carrying every report (`r1`–`r9`), the scores and the notices. Adjust `ValidatorConfig` fields to change thresholds, or apply a JSON delta with `merge_delta`.

⚠️ The library crates are the analyzer's **internals**. They are published so the binary can be built from the registry and they carry **no API stability guarantee**. If you need a stable surface, the CLI's JSON output or `gtfs-sdk` is the safer choice.

### `gtfs-sdk` npm package

`gtfs-sdk` exposes the v0.11.1 validation engine as a typed JavaScript/TypeScript API. The feed is validated with local WASM and never leaves the application:

```js
import { validateGtfs } from "gtfs-sdk";

const result = await validateGtfs(new Uint8Array(zipBytes), {
  today: "2026-08-20",
});
console.log(result.notices.length, result.reports.r5.score);
```

The public API includes `validateGtfs`, `getVersion`, and `createValidatorSession` for applications that need progress and cache events. The low-level `gtfs-wasm` binding is not part of the SDK contract; WASM64 and threaded engine selection remain internal to the first SDK package.

Package sources live under `sdk/`; the detailed usage, result model, and config reference are in [`sdk/README.md`](sdk/README.md). The WASM binding is generated from `crates/wasm` during the build.

---

## CLI (Terminal)

Besides the web UI, you can run the same validation core (`gtfs_pipeline::validate_bytes`) from a terminal — for Python/automation integration.

### Installation

With Rust installed, the shortest path:

```bash
cargo install gtfs-analyzer
gtfs-analyzer validate feed.zip
```

Without installing Rust: download the archive for your platform from [Releases](https://github.com/ttezer/gtfs-analyzer/releases) (`x86_64-linux`, `aarch64-macos`, `x86_64-windows`), unpack it and put the `gtfs-analyzer` binary on your `PATH`.

```bash
# Linux / macOS — latest release
curl -sL https://github.com/ttezer/gtfs-analyzer/releases/latest/download/gtfs-analyzer-x86_64-linux.tar.gz | tar xz
./gtfs-analyzer --version
```

To build from source:

```bash
cargo build --release -p gtfs-analyzer
target/release/gtfs-analyzer validate feed.zip --json

# or directly
cargo run -p gtfs-analyzer -- validate feed.zip --json
```

### `validate` — feed validation

| Flag | Description |
|---|---|
| `--json` | Writes the full result as JSON |
| `--summary` | Short summary: status, notice count, scores (default; cannot be combined with `--json`) |
| `--rule SHP_010` | Only notices for the given rule |
| `--severity critical` | Notices with exactly this severity (critical/high/medium/low/info) |
| `--min-severity high` | This severity and anything worse (critical is the worst) |
| `--class spec` | Only these rule classes — `spec,interop,quality,analytics`, comma separated |
| `--fail-on critical` | Exit 1 **only** when this severity or worse is present |
| `--fail-on-class spec` | Exit 1 only when a notice in these classes is present |
| `--pretty` | Indent the JSON output (requires `--json`) |
| `--include-name-index` | Include `name_index` (stop/route/shape lookup tables) in the JSON |
| `-o report.json` | Write the output to a file instead of stdout |
| `--lang en` | Language of the finding texts: `en` (default) / `tr` / `ja` / `fr` |
| `--config config.json` | Apply a JSON config delta (on top of `ValidatorConfig::default()`) |
| `--today 20260710` | Pin the analysis "today" (for calendar rules) |

**Filters only narrow what is displayed.** `notices` and the R2–R9 lists are filtered; **the R1 publish verdict and the R5 scores always describe the whole feed**. When a filter is active the JSON gains a `filtered` field and the summary a `filter:` line.

`name_index` is **omitted by default**: on large feeds the stop/shape coordinate tables dominate the payload. Pass `--include-name-index` when you need it.

Pass `-` instead of a path to read the ZIP from **stdin**: `curl -sL <url> | gtfs-analyzer validate - --json`. (The ZIP central directory lives at the end of the file, so the archive is buffered in memory rather than streamed.)

> **Counts differ from the web UI.** The browser caps how many findings it keeps per rule for performance (the real totals are reported in `capped_totals`). The CLI applies **no such cap** — the same feed yields more notices and unscaled R9 impact figures. This is expected; do not compare the two outputs count by count.

**Exit codes:** `0` no notices · `1` notices present or a `PARTIAL` report · `2` fatal or config/file error. A `PARTIAL` report safely skips unavailable inputs and continues independent checks; JSON exposes `status: "partial"`, `validation_status: "PARTIAL"`, and the `partial` scope. `partial.skipped_checks` lists the K4/K5/K6 check families and individual rules skipped because prerequisites were unavailable; `partial.skipped_stages` is retained for coarse stage metadata. With `--fail-on*`, `1` is returned only for a matching notice; other findings are still reported but do not fail the run. In JSON mode stdout is JSON only; errors go to stderr.

```bash
# CI gate: fail only on official GTFS Spec violations
gtfs-analyzer validate feed.zip --fail-on-class spec

# Report Spec findings only (scores still describe the whole feed)
gtfs-analyzer validate feed.zip --class spec --json --pretty -o spec.json
```

```python
import json, subprocess

proc = subprocess.run(
    ["target/release/gtfs-analyzer", "validate", "feed.zip", "--json"],
    text=True, capture_output=True,
)
# exit 1 means "has notices", not failure — do NOT use check=True
data = json.loads(proc.stdout)
if data["status"] == "fatal":
    raise SystemExit(f'{data["code"]}: {data["message"]}')
for n in data["notices"]:
    print(n["rule_id"], n["severity"], n["rule_class"])
```

### `rules` — rule registry

Lists the whole rule registry without running a validation — meant as the rule dictionary for integrating projects.

```bash
gtfs-analyzer rules --class spec --severity critical
gtfs-analyzer rules --rule STM_004 --json --pretty
```

Fields: `id`, `severity`, `class`, `authority_source`, `base_effort`, `blocks`, `title`.
The `--class` / `--severity` / `--min-severity` / `--rule` filters mean the same as in `validate`.
`--lang` applies here too (rule titles).

### Output language

The validation core emits its finding texts in Turkish; `--lang en` / `--lang ja` / `--lang fr` replace them using the **same translation dictionaries the web UI uses**. Rule ids, severities and classes (`CRITICAL`, `SPEC`) stay machine-readable in every language — only `title`, `message` and `remediation` are translated.

When a rule has no translation the chain is: requested language → English → Turkish (the core's own text), so the output is never blank.

The dictionaries are derived from `ui/src/locales/{en,ja}.ts` into `crates/cli/locales/*.json` by `npm run locales:export` and embedded in the CLI binary. If a locale is edited without re-running the export, `locale-parity.test.ts` fails in CI — the locale files remain the single source of truth.

---

## Analysis Thresholds

Validation thresholds can be customized from the **Analysis Thresholds** section on the upload screen. Changed values take effect on the next ZIP upload; the reset button restores defaults.

### Rule Classes and Authority Source

Every rule falls into one of four classes. The class reflects the finding's **authority source** (its basis of legitimacy), so a user can tell at a glance whether a finding is a real GTFS Spec violation or an interoperability/quality/analytics signal:

- **Spec** — only cases that the official **GTFS Schedule Reference** explicitly requires, forbids, or renders invalid (required / conditionally required / conditionally forbidden fields, enum values, foreign keys, uniqueness, format constraints). No other source produces `Spec`.
- **Interop** — compatibility signals with consumer/validator behavior such as MobilityData, Google Transit, or a regional profile (e.g., GTFS-JP).
- **Quality** — GTFS best-practice, data quality, readability, consistency, and production-quality checks.
- **Analytics** — statistical, operational, performance, or analysis-oriented signals.

Each rule also carries a machine-readable **authority source** (`authority_source`) field (`GTFS_SPEC`, `MOBILITYDATA_PARITY`, `REGIONAL_PROFILE`, `PROJECT_QUALITY`, etc.). Invariant: **the `Spec` class is legitimate only with `authority_source = GTFS_SPEC`**; parity with MobilityData/Guru/Google, best-practice, or project-specific heuristics is not on its own proof of Spec.

### Optional Profiles and Source URL

Setting `stop_name_best_practices=true` in the config delta enables the language-dependent `STP_040` and `STP_041` checks; they are disabled by default because of their false-positive risk. URL-based integrations may provide `source_url` metadata, allowing `ARC_028` to verify that the permanent publishing URL contains a `.zip` filename. Upload-only validation skips this check. The core engine never requests URLs found inside a feed; HTTP availability checks require a separate, explicitly opt-in online adapter.

### Coordinating shape distance fields

If a trip uses `shape_dist_traveled` in `stop_times.txt` but some points of its referenced shape lack the same field in `shapes.txt`, the analyzer emits `SHP_030` (Quality · Medium). Both fields are individually optional in GTFS, so this is not a Spec publish blocker; it is a shape-level compatibility signal that consumers may be unable to place stops reliably on the shape. The finding includes the affected-trip count and representative trip IDs.

A one-point shape that is actually referenced by a trip is reported as `SHP_006` at Low · Quality, with `shape_id` and `shape_point_count=1` in the details. A two-point straight segment is valid. An unused one-point shape is reported only by `SHP_018`. This is an intentional near-parity mapping for MobilityData's `single_shape_point`: Analyzer reports `SHP_006` only for a used shape.

### Far-stop speed parity

MobilityData's current rules page is internally inconsistent for `fast_travel_between_far_stops`: the main WARNING table shows it as active, while the notice-detail metadata says `Deprecated since undefined`, and the deprecated table omits it. The #115 audit sampled 20 positive feeds; the decision is based on the signal's mixed/noisy combination of cumulative distances over 10 km, non-consecutive stop pairs, and timing cascades—not on an assumed deprecation. Aliasing it to `STM_012` or `STM_014` was rejected; no new rule was added and the difference remains an intentional Analytics coverage gap.

### Stop URL specificity

`STP_034` and `STP_035` compare `stop_url` with agency and route URLs using a conservative syntactic identity and report low-priority Quality findings. Scheme/host case, the root `/`, and explicit HTTP 80/HTTPS 443 default ports are equivalent; query strings, fragments, path trailing slashes, and percent-encoding remain significant. Stops sharing one normalized URL are reported in a single aggregate finding with the affected-stop count and representative IDs in `details`.

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
| Feed Info Expiry Warning | 7 days | 1–60 | Default `FIN_019` horizon for `feed_info.feed_end_date`; the 30-day MobilityData parity is applicable when `feed_info_expiry_warning_days=30`, separate from `CAL_008` |
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

Severity combines the file/field requirement level (Required · Conditionally Required · Recommended · Optional) from the [GTFS Schedule Reference](https://gtfs.org/documentation/schedule/reference/#file-requirements) with the finding's semantic impact.

### Spec severity rubric

Spec severity combines requirement level and semantic impact; it is not based on whether MobilityData labels
the same finding `ERROR`, `WARNING`, or `INFO`:

- **Critical:** Required file/field, primary-key or foreign-key integrity, or core type/range violation; the feed cannot be consumed reliably and `Spec + Critical` is the publish gate.
- **High:** A direct normative violation that materially changes schedule, fare, accessibility, or Flex/pathway semantics even though the feed remains parseable.
- **Medium:** A localized or conditional normative violation while the main data model remains readable.
- **Low:** A narrow metadata or optional-field normative deviation; it does not block publication.
- **Info:** Not used for normative Spec violations; reserved for measurement or context signals.

Therefore no `Spec` rule may have `Info` severity. The 2026-08-09 audit reviewed all 307
Spec rules and raised the raw service-day rules `STM_048` and `STM_049` from Info to High.
See the complete [Spec severity audit](docs/audits/spec-severity-rubric-2026-08-09.md).

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
| **FLJ** | `fare_leg_join_rules.txt` | Rules that join two legs across a transfer into one effective fare leg (Fares v2) |
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

# Blocking lint for every workspace crate, test, and example target
cargo clippy --workspace --all-targets --all-features -- -D warnings

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
│   ├── rules/      # Rule definitions and registry (611 rules, 38 groups)
│   └── wasm/       # wasm-bindgen WASM output
├── spec-audit/     # Field table generated from the spec (anchor gate)
└── ui/             # Vite + TypeScript frontend
    ├── pkg/          # wasm-pack output (generated, committed)
    ├── src/
    │   └── pages/    # Application tabs (domain/fix/rules/export)
    └── tests/        # Playwright tests
```

## License

MIT — see [LICENSE](LICENSE) for details.
