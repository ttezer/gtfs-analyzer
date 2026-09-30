# gtfs-analyzer

GTFS Schedule validation and feed-quality analysis for Python, powered by the
Rust engine of [GTFS Analyzer](https://github.com/ttezer/gtfs-analyzer). The
package runs the same pipeline as the web app, the CLI, the Rust crates, and the
`gtfs-sdk` npm package, so a feed gets the same findings and scores everywhere.
Validation happens in-process; the feed is never uploaded.

- 618 rules across GTFS Schedule, GTFS-Flex, Fares v2, and GTFS-JP
- Spec / Interop / Quality / Analytics classification, per-feed scores, and a
  publishability verdict
- One abi3 wheel per platform for CPython 3.9+ (Linux x86_64, macOS arm64,
  Windows x86_64); other platforms build from the source distribution, which
  needs a Rust toolchain

## Installation

```bash
pip install gtfs-analyzer
```

## Usage

```python
from gtfs_analyzer import validate_gtfs

result = validate_gtfs("feed.zip")

print(result["validation_status"])            # "COMPLETE" or "PARTIAL"
print(result["reports"]["r1"]["publishable"])  # publishability verdict
print(result["metrics"]["overall_score"])

for notice in result["notices"]:
    print(notice["rule_id"], notice["severity"], notice["file"], notice["line"])
```

`validate_gtfs(feed, *, today=None, config=None, include_name_index=False, lang="en")`

| Argument | Meaning |
|---|---|
| `feed` | Path to a GTFS ZIP (`str` or `pathlib.Path`) or the ZIP as `bytes` |
| `today` | Reference date for calendar checks: `YYYYMMDD` int or `"YYYY-MM-DD"` / `"YYYYMMDD"` string. Defaults to the local date. Pin it for reproducible results. |
| `config` | Optional mapping of engine settings, e.g. `{"disabled_rule_ids": ["TRP_020"]}`. Unknown keys are rejected. |
| `include_name_index` | Include the stop/route name lookup table (large; omitted by default) |
| `lang` | Language of `title`, `message`, and `remediation`: `"en"` (default), `"fr"`, `"ja"`, or `"tr"`, as the CLI's `--lang`. Missing entries fall back to English. |

The returned `dict` is the engine's result object:

| Key | Content |
|---|---|
| `validation_status` | `COMPLETE`, or `PARTIAL` when a limit stopped part of the analysis (see `partial`) |
| `notices` | Findings with `rule_id`, `severity`, `rule_class`, `file`, `line`, `field`, `observed_value`, `expected_value`, `message`, `remediation` |
| `reports` | Report views `r1`–`r9`; `r1` holds the publishability verdict |
| `metrics` | Scores and feed statistics (service window, trip counts, notice counts per class) |
| `capped_totals` | True counts for rules whose notices were capped |

Texts follow `lang`; use `rule_id`, not the message text, for program logic.
The [rule reference](https://github.com/ttezer/gtfs-analyzer/blob/main/RULES.en.md)
describes every rule.

### Errors

- `gtfs_analyzer.ValidationError`: the feed could not be validated at all, for
  example an unreadable ZIP. The message starts with the error code
  (`ZipUnreadable: ...`) and follows `lang`.
- `ValueError`: invalid `today`, `config`, or `lang`.
- `TypeError`: `feed`, `today`, `config`, or `lang` has the wrong type.

A feed with errors is not an exception: its findings are in `notices`.

## Versioning

The package version is the engine version. `gtfs_analyzer.__version__`, the
CLI, and the Rust crates of the same version produce the same results.

## Links

- Web app: <https://ttezer.github.io/gtfs-analyzer/>
- Source and issues: <https://github.com/ttezer/gtfs-analyzer>
- Rule reference: <https://github.com/ttezer/gtfs-analyzer/blob/main/RULES.en.md>
- Changelog: <https://github.com/ttezer/gtfs-analyzer/blob/main/CHANGELOG.md>

MIT licensed.
