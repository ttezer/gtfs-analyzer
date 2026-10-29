#!/usr/bin/env python3
"""Collect run_pair.py records into one JSON file and a Markdown table."""
import json
import statistics
import sys
from pathlib import Path


def gb(kb):
    return f"{kb / 2**20:.2f}" if kb else "–"


def main():
    src, out_json, out_md = Path(sys.argv[1]), Path(sys.argv[2]), Path(sys.argv[3])
    records = [json.loads(p.read_text()) for p in sorted(src.rglob("*.json"))]
    out_json.write_text(json.dumps(records, indent=1))

    lines = ["# Large-feed comparison: GTFS Analyzer vs MobilityData", "",
             "MobilityData memory = smallest -Xmx that still completed (the search stops at the first failure).",
             "Analyzer = median of its runs on this feed (one per job).", "",
             "| feed | ZIP MB | MD build | MD wall @compare heap (s) | MD peak RSS @compare heap (GB) | "
             "MD min completed heap (GB) | analyzer wall (s) | analyzer peak RSS (GB) | analyzer status |",
             "|---|---:|---|---:|---:|---:|---:|---:|---|"]
    by_feed = {}
    for r in records:
        by_feed.setdefault(r["feed"], []).append(r)
    for feed, rs in sorted(by_feed.items()):
        a_runs = [r["analyzer"] for r in rs if r.get("analyzer")]
        a_wall = statistics.median([a["wall_s"] for a in a_runs if a.get("wall_s")]) if a_runs else None
        a_rss = statistics.median([a["max_rss_kb"] for a in a_runs if a.get("max_rss_kb")]) if a_runs else None
        a_status = ",".join(sorted({"ok" if a["exit"] in (0, 1) else f"exit={a['exit']}" for a in a_runs}))
        for r in sorted(rs, key=lambda x: x["md_label"]):
            first = r["md_runs"][0]
            md_wall = f"{first['wall_s']:.1f}" if first.get("wall_s") and first["completed"] else first["reason"]
            lines.append(
                f"| {feed} | {r['zip_bytes'] / 1e6:.0f} | {r['md_label']} | {md_wall} | {gb(first.get('max_rss_kb'))} | "
                f"{r['md_min_completed_heap_gb'] if r['md_min_completed_heap_gb'] is not None else '–'} | "
                f"{a_wall:.1f} | {gb(a_rss)} | {a_status} |" if a_wall else
                f"| {feed} | {r['zip_bytes'] / 1e6:.0f} | {r['md_label']} | {md_wall} | {gb(first.get('max_rss_kb'))} | "
                f"{r['md_min_completed_heap_gb'] if r['md_min_completed_heap_gb'] is not None else '–'} | – | – | {a_status or '–'} |")
    out_md.write_text("\n".join(lines) + "\n")
    print("\n".join(lines))


if __name__ == "__main__":
    main()
