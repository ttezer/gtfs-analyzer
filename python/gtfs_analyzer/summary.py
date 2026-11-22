"""Validate a GTFS ZIP and print a compact JSON summary.

Usage: python -m gtfs_analyzer.summary <feed.zip> [--lang en|tr|fr|ja] [--today YYYYMMDD]

The engine result can hold hundreds of thousands of notices, so this command keeps the
engine's verdict, scores and R9 priority order as returned and adds only counts and a few
example notices per prioritized rule. It never recomputes scores or publishability.

Exit codes (stdout is always one JSON object):
  0  summary printed
  2  FATAL: the engine could not analyze the feed (for example an unreadable ZIP)
  4  INPUT: bad arguments, unreadable path, or invalid --today
"""

from __future__ import annotations

import argparse
import json
import sys
from collections import Counter
from typing import Any, NoReturn, Optional, Sequence

LANGS = ("en", "tr", "fr", "ja")

EXAMPLE_FIELDS = ("file", "line", "field", "entity_id", "observed_value", "expected_value", "message")

EXIT_OK = 0
EXIT_FATAL = 2
EXIT_INPUT = 4

__all__ = ["summarize", "main"]


def _emit(payload: dict, code: int = EXIT_OK) -> NoReturn:
    json.dump(payload, sys.stdout, ensure_ascii=False, indent=1)
    sys.stdout.write("\n")
    sys.exit(code)


class _JsonArgumentParser(argparse.ArgumentParser):
    """Report usage errors as an INPUT JSON object instead of argparse's exit code 2."""

    def error(self, message: str) -> NoReturn:
        _emit({"error": "INPUT", "message": message}, EXIT_INPUT)


def _non_negative_int(text: str) -> int:
    value = int(text)
    if value < 0:
        raise argparse.ArgumentTypeError("must be 0 or greater")
    return value


def _notice_brief(notice: dict) -> dict:
    return {k: notice[k] for k in EXAMPLE_FIELDS if notice.get(k) is not None}


def summarize(result: dict[str, Any], max_priorities: int = 15, examples: int = 3) -> dict[str, Any]:
    """Reduce a ``validate_gtfs`` result to verdict, scores, priorities and counts."""
    notices = result["notices"]
    by_id = {n["id"]: n for n in notices}
    reports = result["reports"]

    rules: dict[str, dict] = {}
    for n in notices:
        entry = rules.get(n["rule_id"])
        if entry is None:
            rules[n["rule_id"]] = {
                "rule_id": n["rule_id"],
                "severity": n["severity"],
                "rule_class": n["rule_class"],
                "title": n["title"],
                "count": 1,
            }
        else:
            entry["count"] += 1

    priorities = []
    for item in reports["r9"]["items"][:max_priorities]:
        ids = item.get("notice_ids", [])
        first = by_id.get(ids[0]) if ids else None
        priorities.append({
            "rule_id": item["rule_id"],
            "title": first["title"] if first else None,
            "severity": first["severity"] if first else None,
            "rule_class": first["rule_class"] if first else None,
            "labels": item.get("labels"),
            "priority_score": item.get("priority_score"),
            "score_delta": item.get("score_delta"),
            "pub_score_delta": item.get("pub_score_delta"),
            "affected_instance_count": item.get("affected_instance_count"),
            "fix_effort": item.get("fix_effort"),
            "remediation": first["remediation"] if first else None,
            "examples": [_notice_brief(by_id[i]) for i in ids[:examples] if i in by_id],
        })

    blockers = [by_id[i] for i in reports["r1"].get("blocker_notice_ids", []) if i in by_id]
    metrics = {k: v for k, v in result["metrics"].items() if k != "file_stats"}

    summary = {
        "validation_status": result["validation_status"],
        "publishability": reports["r1"],
        "scores": reports["r5"],
        "metrics": metrics,
        "severity_counts": dict(Counter(n["severity"] for n in notices)),
        "total_notices": len(notices),
        "capped_totals": result.get("capped_totals") or {},
        "blockers": [
            dict(_notice_brief(n), rule_id=n["rule_id"], title=n["title"])
            for n in blockers[:examples * 4]
        ],
        "blocker_total": len(blockers),
        "r9_priorities": priorities,
        "r9_total": len(reports["r9"]["items"]),
        "rules": sorted(rules.values(), key=lambda r: -r["count"]),
        "agency_count": (result.get("feed_summary") or {}).get("agency_count"),
    }
    if "partial" in result:
        summary["partial"] = result["partial"]
    return summary


def main(argv: Optional[Sequence[str]] = None) -> NoReturn:
    from . import ValidationError, __version__, validate_gtfs

    parser = _JsonArgumentParser(
        prog="python -m gtfs_analyzer.summary",
        description=__doc__,
        formatter_class=argparse.RawDescriptionHelpFormatter,
    )
    parser.add_argument("feed", help="path to a GTFS ZIP file")
    parser.add_argument("--lang", default="en", choices=LANGS)
    parser.add_argument("--today", help="validation date, YYYYMMDD or YYYY-MM-DD (defaults to today)")
    parser.add_argument("--max-priorities", type=_non_negative_int, default=15)
    parser.add_argument("--examples", type=_non_negative_int, default=3)
    parser.add_argument(
        "--expect-version",
        help="engine version the caller was written for; a mismatch adds version_warning",
    )
    args = parser.parse_args(argv)

    kwargs: dict[str, Any] = {"lang": args.lang}
    if args.today:
        kwargs["today"] = args.today
    try:
        result = validate_gtfs(args.feed, **kwargs)
    except ValidationError as err:
        _emit({"error": "FATAL", "message": str(err), "engine_version": __version__}, EXIT_FATAL)
    except (OSError, ValueError, TypeError) as err:
        _emit({"error": "INPUT", "message": str(err)}, EXIT_INPUT)

    summary = summarize(result, args.max_priorities, args.examples)
    summary["engine_version"] = __version__
    if args.expect_version and args.expect_version != __version__:
        summary["version_warning"] = f"expected gtfs-analyzer {args.expect_version}"
    _emit(summary)


if __name__ == "__main__":
    main()
