# Eight Ways to Use It

🇬🇧 **English** · [Back to README](../README.en.md)

The 621-rule GTFS Validator & Analyzer engine can be reached in eight ways. This page covers installing and using each of them.

## 1. Browser

GTFS Validator & Analyzer is a web application — no installation required. Open the live version in your browser and upload your GTFS zip file.

The runtime is selected automatically from browser capabilities: browsers with Memory64 use **WASM64**, all others use **WASM32**; there is no fixed feed size limit. The active engine is shown on the upload screen. For diagnostics, use `?wasm32=1`, `?wasm64=1`, or `?serial=1`.

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

> To self-host or set up a development environment, see [Developer Setup](../README.en.md#developer-setup).

## 2. CLI

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
| `--compact-json` | Emit the canonical compact JSON projection for machine consumers; add R9 root-cause samples with `--samples N` |
| `--summary` | Short summary: status, notice count, scores (default; cannot be combined with `--json`) |
| `--rule SHP_010` | Only notices for the given rule |
| `--severity critical` | Notices with exactly this severity (critical/high/medium/low/info) |
| `--min-severity high` | This severity and anything worse (critical is the worst) |
| `--class spec` | Only these rule classes — `spec,interop,quality,analytics`, comma separated |
| `--disable-rule DQ_004` | Removes the listed rules from notices, scores, and the R9 queue; comma separated |
| `--disable-rule-gtfs-jp DQ_004` | Removes these rules only for feeds detected as GTFS-JP |
| `--fail-on critical` | Exit 1 **only** when this severity or worse is present |
| `--fail-on-class spec` | Exit 1 only when a notice in these classes is present |
| `--pretty` | Indent the JSON output (requires `--json`) |
| `--include-name-index` | Include `name_index` (stop/route/shape lookup tables) in the JSON |
| `--max-notices-per-rule 20` | Keep at most N notices per rule in the JSON; real counts of truncated rules go to `capped_totals`, R1/R5/R9 are still computed over every notice (requires `--json`) |
| `--samples N` | With `--compact-json`, add the first N root-cause findings per R9 rule (`entity_type`, `entity_id`, `file`, `line`, `field`, `observed_value`, `expected_value`) |
| `-o report.json` | Write the output to a file instead of stdout |
| `--lang en` | Language of the finding texts: `en` (default) / `tr` / `ja` / `fr` / `es` (`es`: planner cards only) |
| `--config config.json` | Apply a JSON config delta (on top of `ValidatorConfig::default()`) |
| `--today 20260710` | Pin the analysis "today" (for calendar rules) |

**Filters only narrow what is displayed.** `notices` and the R2–R9 lists are filtered; **the R1 publish verdict and the R5 scores always describe the whole feed**. When a filter is active the JSON gains a `filtered` field and the summary a `filter:` line.

`name_index` is **omitted by default**: on large feeds the stop/shape coordinate tables dominate the payload. Pass `--include-name-index` when you need it.

Pass `-` instead of a path to read the ZIP from **stdin**: `curl -sL <url> | gtfs-analyzer validate - --json`. (The ZIP central directory lives at the end of the file, so the archive is buffered in memory rather than streamed.)

> **Counts differ from the web UI.** The browser caps how many findings it keeps per rule for performance (the real totals are reported in `capped_totals`). The CLI applies **no such cap** — the same feed yields more notices and unscaled R9 impact figures. This is expected; do not compare the two outputs count by count.

### Compact JSON and the web cap

`--compact-json` is the canonical machine-consumer projection: it omits the individual
`notices` list and `name_index` tables from the full `--json` report. Its top level
contains `status`, `engine`, `engine_version`, `engine_commit`,
`validation_date`, `partial`, `analysis`, `reports.r1`, `reports.r5`,
`reports.r9`, `metrics`, `notice_counts`, and `triggered_rule_count`.
Each `reports.r9.items` entry contains rule title, severity, class, message, remediation,
priority, impact, and score-delta fields, plus a plain-language `card` (`title`, `what`,
`action`) in the `--lang` language; `--samples N` adds up to N root-cause
examples to each item.

`--compact-json` is not the web UI payload and does not apply a per-rule sample cap by
itself. To approximate the web UI's visible notice list in full CLI JSON, use
`--json --max-notices-per-rule 500`; R1/R5/R9 are still computed over the complete
notice list and truncated rules are reported in `capped_totals`.

**Exit codes:** `0` no notices · `1` notices present or a `PARTIAL` report · `2` fatal or config/file error. A `PARTIAL` report safely skips unavailable inputs and continues independent checks; JSON exposes `status: "partial"`, `validation_status: "PARTIAL"`, and the `partial` scope. `partial.skipped_checks` lists the K4/K5/K6 check families and individual rules skipped because prerequisites were unavailable; `partial.skipped_stages` is retained for coarse stage metadata. With `--fail-on*`, `1` is returned only for a matching notice; other findings are still reported but do not fail the run. In JSON mode stdout is JSON only; errors go to stderr.

```bash
# CI gate: fail only on official GTFS Spec violations
gtfs-analyzer validate feed.zip --fail-on-class spec

# Report Spec findings only (scores still describe the whole feed)
gtfs-analyzer validate feed.zip --class spec --json --pretty -o spec.json
```

```python
import json, pathlib, subprocess

for feed in sorted(pathlib.Path("feeds").glob("*.zip")):
    proc = subprocess.run(
        ["gtfs-analyzer", "validate", str(feed), "--json"],
        text=True, capture_output=True,
    )
    # exit 1 means "has notices", not failure — do NOT use check=True
    data = json.loads(proc.stdout)
    if data["status"] == "fatal":
        print(feed.name, "FATAL", data["code"], data["message"])
        continue
    print(feed.name, data["reports"]["r5"]["pub_score"], len(data["notices"]))
```

### `rules` — rule registry

Lists the whole rule registry without running a validation — meant as the rule dictionary for integrating projects.

```bash
gtfs-analyzer rules --class spec --severity critical
gtfs-analyzer rules --rule STM_004 --json --pretty
```

Fields: `id`, `severity`, `class`, `authority_source`, `base_effort`, `blocks`, `title`, `message`, `remediation`, `card`.
The `--class` / `--severity` / `--min-severity` / `--rule` filters mean the same as in `validate`.
`--lang` applies here too (rule titles, messages and remediations).

### Output language

The validation core emits its finding texts in Turkish; `--lang en` / `--lang ja` / `--lang fr` replace them using the **same translation dictionaries the web UI uses**. Rule ids, severities and classes (`CRITICAL`, `SPEC`) stay machine-readable in every language — only `title`, `message` and `remediation` are translated.

When a rule has no translation the chain is: requested language → English → Turkish (the core's own text), so the output is never blank.

Rule-level outputs (`--compact-json` R9 items and `rules --json`) also carry `card`: the plain-language title, what happened (`what`) and what to do (`action`) of the web UI's planner view, in `tr`, `en`, `ja`, `fr` and `es`. `es` is a cards-only language: its other texts fall back to English. `what` is left out when the card text needs per-finding values.

The dictionaries are derived from `ui/src/locales/{en,ja}.ts` into `crates/core/locales/*.json` by `npm run locales:export` and embedded in the CLI binary and the Python package. If a locale is edited without re-running the export, `locale-parity.test.ts` fails in CI — the locale files remain the single source of truth.

## 3. Rust library

To embed validation in your own Rust service, use `gtfs-pipeline` directly — no CLI, no filesystem, no network:

```toml
[dependencies]
gtfs-pipeline = "0.22.0"
gtfs-config   = "0.22.0"
gtfs-core     = "0.22.0"
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

## 4. CI/CD

The `--fail-on` flags fail the run only for the severity or class you choose, so Analytics findings do not break a release pipeline:

```yaml
# GitHub Actions — fail only on official GTFS Spec violations
- name: Validate GTFS feed
  run: |
    curl -sL https://github.com/ttezer/gtfs-analyzer/releases/latest/download/gtfs-analyzer-x86_64-linux.tar.gz | tar xz
    ./gtfs-analyzer validate feed.zip --fail-on-class spec --min-severity critical
```

Exit codes: `0` clean · `1` matching findings present · `2` feed/config/file error.

## 5. `gtfs-sdk` npm package

`gtfs-sdk` exposes the v0.22.0 validation engine as a typed JavaScript/TypeScript API. The feed is validated with local WASM and never leaves the application:

```js
import { validateGtfs } from "gtfs-sdk";

const result = await validateGtfs(new Uint8Array(zipBytes), {
  today: "2026-08-20",
});
console.log(result.notices.length, result.reports.r5.score);
```

The public API includes `validateGtfs`, `getVersion`, and `createValidatorSession` for applications that need progress and cache events. The low-level `gtfs-wasm` binding is not part of the SDK contract; WASM64 and threaded engine selection remain internal to the first SDK package.

Package sources live under `sdk/`; the detailed usage, result model, and config reference are in [`sdk/README.md`](../sdk/README.md). The WASM binding is generated from `crates/wasm` during the build.

## 6. Python package

Install from [PyPI](https://pypi.org/project/gtfs-analyzer/) (CPython 3.9+):

```bash
pip install gtfs-analyzer
```

The package runs the same Rust pipeline through a PyO3 native module; no Cargo
or CLI install is needed. Pass the ZIP as a path or as bytes:

```python
from gtfs_analyzer import validate_gtfs

result = validate_gtfs("feed.zip", today="2026-09-30")
print(result["validation_status"])
print(len(result["notices"]))
```

`result` carries the same core result as the npm SDK and the CLI JSON output.
Parameters, the result structure, and error types are described in
[`python/README.md`](../python/README.md).

## 7–8. ChatGPT and Claude

Installation and usage are in the [ChatGPT and Claude section of the README](../README.en.md).

## Community bindings

| Language | Package | Maintained by |
|---|---|---|
| Kotlin / JVM | [`io.github.transittrack:gtfs-analyzer`](https://central.sonatype.com/artifact/io.github.transittrack/gtfs-analyzer) | Its authors, outside this repository |

Community bindings are not tested in this repository and do not ship from its release
process. Report engine findings here and binding-specific issues to the package's own
repository.

---
