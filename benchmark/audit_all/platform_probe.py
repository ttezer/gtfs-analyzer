#!/usr/bin/env python3
"""Extract the six known cross-platform K6 comparison points.

The full audit already runs on Ubuntu. This small artifact makes the current
run's four feed / six rule observations explicit, so a later comparison cannot
mistake an old Linux corpus result for a fresh measurement of today's binary.
"""

import argparse
import gzip
import json
import platform
import os
from pathlib import Path


PROBE_CASES = (
    ("jbda-kagaminotown-kagaminotownbus", "SHP_017"),
    ("jbda-kimitsucity-Local_buses_via_Kimitsu_City", "SHP_017"),
    ("jbda-tokushima-miyoshicity-miyoshicitybus", "SHP_017"),
    ("jbda-nantocity-nanbus", "STM_014"),
    ("jbda-tokushima-miyoshicity-miyoshicitybus", "STM_014"),
    ("jbda-nantocity-nanbus", "OPR_008"),
)


def load_rows(path):
    path = Path(path)
    opener = gzip.open if path.suffix == ".gz" else open
    with opener(path, "rt", encoding="utf-8") as fh:
        rows = json.load(fh)
    if not isinstance(rows, list):
        raise ValueError("all-results must contain a JSON array")
    return rows


def extract_probe(rows, profile="auto"):
    by_feed = {}
    for row in rows:
        feed_id = (row.get("feed") or {}).get("feed_id")
        if feed_id:
            if feed_id in by_feed:
                raise ValueError(f"duplicate result row for {feed_id}")
            by_feed[feed_id] = row

    cases = []
    missing = []
    for feed_id, rule_id in PROBE_CASES:
        row = by_feed.get(feed_id)
        if row is None:
            missing.append({"feed_id": feed_id, "rule_id": rule_id})
            continue
        report = (row.get("analyzer_profiles") or {}).get(profile)
        if report is None:
            report = row.get("analyzer") or {}
        cases.append({
            "feed_id": feed_id,
            "rule_id": rule_id,
            "state": report.get("state", "not_run"),
            "exit_code": report.get("exit_code"),
            "notice_count": int((report.get("by_rule") or {}).get(rule_id, 0)),
            "wall_time": (report.get("timing") or {}).get("elapsed"),
        })
    return {"profile": profile, "cases": cases, "missing": missing}


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--all-results", required=True)
    ap.add_argument("--out", required=True)
    ap.add_argument("--profile", default="auto")
    args = ap.parse_args()
    probe = extract_probe(load_rows(args.all_results), args.profile)
    if probe["missing"]:
        raise SystemExit(
            "platform probe feed(s) missing: "
            + ", ".join(x["feed_id"] for x in probe["missing"])
        )
    probe.update({
        "schema_version": 1,
        "runner": {
            "platform": platform.platform(),
            "machine": platform.machine(),
            "github_sha": os.environ.get("GITHUB_SHA"),
            "github_run_id": os.environ.get("GITHUB_RUN_ID"),
        },
    })
    Path(args.out).write_text(json.dumps(probe, indent=2, ensure_ascii=False) + "\n")
    print(json.dumps(probe, indent=2, ensure_ascii=False))


if __name__ == "__main__":
    main()
