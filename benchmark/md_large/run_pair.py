#!/usr/bin/env python3
"""Large-feed speed/memory comparison: GTFS Analyzer vs one MobilityData validator build.

One invocation = one feed x one MobilityData jar, on one runner:

* the analyzer runs once (wall time, peak RSS, exit status);
* MobilityData runs once at the comparison heap (default 12 GiB) for wall time, then
  at decreasing heaps until a run fails. The smallest heap that still completes is the
  memory figure we report, because peak RSS under a fixed -Xmx measures the ceiling the
  JVM was given rather than what it needed.

Every run is wrapped in `/usr/bin/time -v`. A MobilityData run counts as completed only
when it exits 0, writes report.json and reports no OutOfMemoryError in system_errors.json.
"""
import argparse
import hashlib
import json
import os
import platform
import re
import subprocess
import time
from pathlib import Path

FEED_URLS = {
    "nv_free": "https://download.gtfs.de/germany/nv_free/latest.zip",
}


def feed_url(feed_id):
    return FEED_URLS.get(feed_id, f"https://files.mobilitydatabase.org/{feed_id}/latest.zip")


def sha256(path):
    h = hashlib.sha256()
    with open(path, "rb") as f:
        for block in iter(lambda: f.read(1 << 20), b""):
            h.update(block)
    return h.hexdigest()


def parse_time(text):
    def grab(label):
        m = re.search(rf"{re.escape(label)}: (.+)", text)
        return m.group(1).strip() if m else None

    elapsed = grab("Elapsed (wall clock) time (h:mm:ss or m:ss)")
    wall = None
    if elapsed:
        parts = [float(p) for p in elapsed.split(":")]
        wall = sum(v * 60 ** i for i, v in enumerate(reversed(parts)))
    rss = grab("Maximum resident set size (kbytes)")
    user = grab("User time (seconds)")
    sys_ = grab("System time (seconds)")
    return {
        "wall_s": wall,
        "max_rss_kb": int(rss) if rss else None,
        "cpu_s": (float(user) + float(sys_)) if user and sys_ else None,
    }


def run_timed(cmd, workdir, timeout_s):
    workdir.mkdir(parents=True, exist_ok=True)
    timefile = workdir / "time.txt"
    full = ["/usr/bin/time", "-v", "-o", str(timefile),
            "timeout", "--signal=TERM", "--kill-after=10s", f"{timeout_s}s", *cmd]
    with open(workdir / "stdout.txt", "wb") as so, open(workdir / "stderr.txt", "wb") as se:
        code = subprocess.run(full, stdout=so, stderr=se).returncode
    result = {"exit": code, "timed_out": code in (124, 137)}
    if timefile.exists():
        result.update(parse_time(timefile.read_text(errors="replace")))
    return result


def md_completed(outdir, result):
    if result["exit"] != 0 or result["timed_out"]:
        return False, "exit"
    if not (outdir / "report.json").exists():
        return False, "no_report"
    errors = outdir / "system_errors.json"
    if errors.exists():
        text = errors.read_text(errors="replace").lower()
        if "outofmemoryerror" in text or "java heap space" in text:
            return False, "oom"
    return True, "ok"


def md_run(jar, zip_path, work, heap_gb, today, timeout_s):
    outdir = work / f"md-{heap_gb}g" / "out"
    res = run_timed(
        ["java", f"-Xmx{heap_gb}g", "-XX:+ExitOnOutOfMemoryError",
         "-jar", str(jar), "-i", str(zip_path), "-o", str(outdir), "-d", today],
        work / f"md-{heap_gb}g", timeout_s)
    stderr_text = (work / f"md-{heap_gb}g" / "stderr.txt").read_text(errors="replace").lower()
    ok, why = md_completed(outdir, res)
    if ok and "outofmemoryerror" in stderr_text:
        ok, why = False, "oom"
    res.update({"heap_gb": heap_gb, "completed": ok, "reason": why})
    report = outdir / "report.json"
    if report.exists():
        try:
            summary = json.loads(report.read_text()).get("summary") or {}
            res["md_summary"] = {k: summary.get(k) for k in ("validatorVersion", "validationTimeSeconds",
                                                               "memoryUsageRecords") if k in summary}
        except Exception as e:  # noqa: BLE001
            res["md_summary_error"] = str(e)[:200]
    return res


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--feed", required=True)
    ap.add_argument("--md-label", required=True)
    ap.add_argument("--md-jar", required=True)
    ap.add_argument("--analyzer", required=True)
    ap.add_argument("--today", required=True)
    ap.add_argument("--compare-heap", type=int, default=12)
    ap.add_argument("--search-heaps", default="10,8,6,4")
    ap.add_argument("--timeout", type=int, default=3600)
    ap.add_argument("--skip-analyzer", action="store_true")
    ap.add_argument("--out", required=True)
    args = ap.parse_args()

    work = Path("work") / args.feed
    feed_dir = work / "feed"
    feed_dir.mkdir(parents=True, exist_ok=True)
    zip_path = feed_dir / "latest.zip"
    url = feed_url(args.feed)
    subprocess.run(["curl", "-fsSL", "--retry", "3", "--retry-all-errors", "-o", str(zip_path), url], check=True)

    record = {
        "feed": args.feed, "url": url, "zip_bytes": zip_path.stat().st_size, "zip_sha256": sha256(zip_path),
        "md_label": args.md_label, "today": args.today,
        "runner": {"nproc": os.cpu_count(), "machine": platform.machine(),
                   "mem_total_kb": next((int(l.split()[1]) for l in open("/proc/meminfo") if l.startswith("MemTotal")), None)},
        "started": time.strftime("%Y-%m-%dT%H:%M:%SZ", time.gmtime()),
    }

    if not args.skip_analyzer:
        a = run_timed([args.analyzer, "validate", str(zip_path), "--summary", "--lang", "en",
                       "--today", args.today], work / "analyzer", args.timeout)
        a["summary"] = (work / "analyzer" / "stdout.txt").read_text(errors="replace")[:2000]
        record["analyzer"] = a
        print(f"[analyzer] {args.feed}: exit={a['exit']} wall={a.get('wall_s')} rss_kb={a.get('max_rss_kb')}", flush=True)

    jar = Path(args.md_jar)
    runs = [md_run(jar, zip_path, work, args.compare_heap, args.today, args.timeout)]
    print(f"[md {args.md_label}] {args.compare_heap}g: {runs[-1]['reason']} wall={runs[-1].get('wall_s')}", flush=True)
    min_ok = args.compare_heap if runs[-1]["completed"] else None
    if min_ok is not None:
        for heap in (int(h) for h in args.search_heaps.split(",") if h):
            if heap >= args.compare_heap:
                continue
            r = md_run(jar, zip_path, work, heap, args.today, args.timeout)
            runs.append(r)
            print(f"[md {args.md_label}] {heap}g: {r['reason']} wall={r.get('wall_s')}", flush=True)
            if not r["completed"]:
                break
            min_ok = heap
    record["md_runs"] = runs
    record["md_min_completed_heap_gb"] = min_ok
    record["finished"] = time.strftime("%Y-%m-%dT%H:%M:%SZ", time.gmtime())
    Path(args.out).parent.mkdir(parents=True, exist_ok=True)
    Path(args.out).write_text(json.dumps(record, indent=1))


if __name__ == "__main__":
    main()
