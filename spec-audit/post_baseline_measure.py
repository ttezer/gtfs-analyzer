#!/usr/bin/env python3
"""REAL corpus count for rules added after the baseline (continuation of issue #131).

Problem: the DATA in `corpus-evidence/rule_stats.csv` comes from the 2026-08-06 run. Rules added after
that date were ABSENT from that run, so "never appeared in the corpus" for them is NOT A MEASUREMENT,
but the ABSENCE of a measurement. `RTS_030` proves this: it looks silent in the table, although it was
measured firing on 19 feeds when added.

This script runs the current binary on all 242 feeds and writes feed/finding counts PER RULE.
It does NOT TOUCH `corpus_batch.py` results — that tool skips completed feeds, and the old run's
output must be preserved.

    cargo build --release -p gtfs-analyzer
    python3 spec-audit/post_baseline_measure.py            # resumable
    python3 spec-audit/post_baseline_measure.py --report    # summary
"""
import argparse
import collections
import csv
import json
import os
import subprocess
import sys
import time

ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
ZIPS = os.path.join(ROOT, "notgit/corpus/zips")
OUT = os.path.join(ROOT, "notgit/corpus/postbaseline_counts.csv")
BINARY = os.path.join(ROOT, "target/release/gtfs-analyzer")

# Rules tagged POST_BASELINE in `silent_rules.tsv` (2026-08-15).
WATCH = [
    "ARC_026", "ARC_033", "FIN_012", "RTS_030", "SHP_030",
    "STM_049", "STM_060", "STP_034", "TFR_005", "TRF_017",
]
# Fixed date — keep date-relative rules deterministic.
TODAY = "20260515"

# 🚫 USER INSTRUCTION (2026-08-16): NEVER RUN `mdb-2904`.
# At 85 MB it produces 5,374,292 findings by itself and locks every batch run.
# This list is not commentary but a GATE — remove it from the feed list and report the skip explicitly.
EXCLUDED_FEEDS = {"mdb-2904"}



def done_feeds():
    if not os.path.exists(OUT):
        return set()
    with open(OUT) as fh:
        return {r["feed"] for r in csv.DictReader(fh)}


def run_one(feed, path):
    t0 = time.time()
    # exit 1 = "notice exists", NOT AN ERROR; do not use check=True.
    p = subprocess.run(
        [BINARY, "validate", path, "--json", "--today", TODAY],
        capture_output=True, timeout=1800,
    )
    secs = time.time() - t0
    if p.returncode not in (0, 1, 2) or not p.stdout:
        return {"feed": feed, "status": f"crash rc={p.returncode}", "secs": round(secs, 1)}
    try:
        d = json.loads(p.stdout)
    except Exception as e:
        return {"feed": feed, "status": f"parse {e}"[:60], "secs": round(secs, 1)}
    if "Fatal" in d:
        return {"feed": feed, "status": "FATAL:" + str(d["Fatal"].get("code")),
                "secs": round(secs, 1)}
    notices = d.get("Ok", d).get("notices", [])
    c = collections.Counter(n["rule_id"] for n in notices)
    row = {"feed": feed, "status": "ok", "secs": round(secs, 1),
           "notice_total": len(notices)}
    row.update({r: c.get(r, 0) for r in WATCH})
    return row


def report():
    with open(OUT) as fh:
        rows = list(csv.DictReader(fh))
    ok = [r for r in rows if r["status"] == "ok"]
    print(f"koşulan feed: {len(rows)} (ok={len(ok)})")
    print(f"\n{'kural':10} {'feed':>6} {'bulgu':>10}   ilk 3 feed")
    for rule in WATCH:
        hits = [(r["feed"], int(r[rule])) for r in ok if int(r.get(rule, 0)) > 0]
        total = sum(n for _, n in hits)
        sample = ", ".join(f"{f}({n})" for f, n in sorted(hits, key=lambda x: -x[1])[:3])
        print(f"{rule:10} {len(hits):>6} {total:>10}   {sample}")


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--report", action="store_true")
    args = ap.parse_args()
    if args.report:
        report()
        return 0

    if not os.path.exists(BINARY):
        sys.exit("release binary yok: cargo build --release -p gtfs-analyzer")
    feeds = sorted(f for f in os.listdir(ZIPS)
                if f.endswith(".zip") and f[:-4] not in EXCLUDED_FEEDS)
    if EXCLUDED_FEEDS:
        print(f"atlanan feed (kullanıcı talimatı): {sorted(EXCLUDED_FEEDS)}", flush=True)
    already = done_feeds()
    fields = ["feed", "status", "secs", "notice_total"] + WATCH
    new = not os.path.exists(OUT)
    with open(OUT, "a", newline="") as fh:
        w = csv.DictWriter(fh, fieldnames=fields, extrasaction="ignore",
                           lineterminator="\n")
        if new:
            w.writeheader()
        for i, fn in enumerate(feeds, 1):
            feed = fn[:-4]
            if feed in already:
                continue
            row = run_one(feed, os.path.join(ZIPS, fn))
            w.writerow(row)
            fh.flush()
            print(f"[{i}/{len(feeds)}] {feed:12} {row['status']:12} {row['secs']:7}s",
                  flush=True)
    return 0


if __name__ == "__main__":
    sys.exit(main())
