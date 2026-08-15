#!/usr/bin/env python3
"""Aggregate the full runnable MobilityDatabase GTFS Schedule audit.

This is a population measurement, not a before/after comparison with the old
stratified corpus. Missing shard rows are explicit, duplicate downloaded content
is reported, and only COMPLETE/COMPLETE validator pairs enter parity counts.
"""
from __future__ import annotations

import argparse
import csv
import gzip
import importlib.util
import json
import math
import statistics
from collections import Counter, defaultdict
from pathlib import Path


def load_map(path: str):
    spec = importlib.util.spec_from_file_location("mdmap", path)
    mod = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(mod)
    return mod.MAP, getattr(mod, "AGG_RULES", set())


def md_counts(m):
    return {
        g.get("code"): int(g.get("totalNotices") or 0)
        for g in (m or {}).get("groups", [])
        if g.get("code")
    }


def md_severity(m, code):
    for g in (m or {}).get("groups", []):
        if g.get("code") == code:
            return (g.get("severity") or "").upper()
    return ""


def md_sample(m, code):
    for g in (m or {}).get("groups", []):
        if g.get("code") == code:
            return (g.get("sampleNotices") or [])[:2]
    return []


def percentile(values, q):
    xs = sorted(float(x) for x in values if x is not None)
    if not xs:
        return None
    pos = (len(xs) - 1) * q
    lo, hi = math.floor(pos), math.ceil(pos)
    if lo == hi:
        return xs[lo]
    return xs[lo] * (hi - pos) + xs[hi] * (pos - lo)


def perf(values):
    xs = [float(x) for x in values if x is not None]
    return {
        "n": len(xs),
        "median": statistics.median(xs) if xs else None,
        "p95": percentile(xs, 0.95),
        "maximum": max(xs) if xs else None,
    }


def priority(d):
    typ = d["type"]
    sev = (d.get("md_severity") or "").upper()
    if typ == "validator_state_asymmetry":
        return 110
    if typ == "md_mapped_missing" and sev == "ERROR":
        return 105
    if typ == "analyzer_spec_md_absent" and d.get("analyzer_severity") in ("CRITICAL", "KRİTİK"):
        return 100
    if typ == "analyzer_spec_unmapped":
        return 95
    if typ == "analyzer_spec_md_absent":
        return 92
    if typ == "md_unmapped" and sev == "ERROR":
        return 90
    if typ == "md_mapped_missing":
        return 85
    if typ == "analyzer_mapped_md_absent":
        return 80
    if typ in ("md_mapped_under", "md_mapped_over"):
        return 65
    if typ == "md_unmapped":
        return 55
    return 40


def write_csv(path, rows, fields):
    with Path(path).open("w", newline="", encoding="utf-8") as fh:
        w = csv.DictWriter(fh, fieldnames=fields, extrasaction="ignore")
        w.writeheader()
        for row in rows:
            x = dict(row)
            for k, v in list(x.items()):
                if isinstance(v, (dict, list, tuple)):
                    x[k] = json.dumps(v, ensure_ascii=False, separators=(",", ":"))
            w.writerow(x)


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--manifest", required=True)
    ap.add_argument("--results-dir", required=True)
    ap.add_argument("--map-file", required=True)
    ap.add_argument("--out-dir", required=True)
    ap.add_argument("--expected-analyzer-sha")
    args = ap.parse_args()

    out = Path(args.out_dir)
    out.mkdir(parents=True, exist_ok=True)
    manifest = json.loads(Path(args.manifest).read_text(encoding="utf-8"))
    expected = manifest.get("feeds") or []
    expected_by_id = {f["feed_id"]: f for f in expected}
    expected_ids = set(expected_by_id)

    rows = []
    malformed_lines = []
    for p in sorted(Path(args.results_dir).glob("shard-*.jsonl")):
        for line_no, line in enumerate(p.read_text(errors="replace").splitlines(), 1):
            if not line.strip():
                continue
            try:
                rows.append(json.loads(line))
            except Exception as exc:
                malformed_lines.append({"file": p.name, "line": line_no, "error": repr(exc)})

    rows.sort(key=lambda x: int((x.get("feed") or {}).get("corpus_index", 10**9)))
    with gzip.open(out / "all-results.json.gz", "wt", encoding="utf-8") as fh:
        json.dump(rows, fh, ensure_ascii=False, separators=(",", ":"))

    seen_ids = [((r.get("feed") or {}).get("feed_id") or "") for r in rows]
    attempted_counts = Counter(x for x in seen_ids if x)
    attempted_ids = set(attempted_counts)
    missing_ids = sorted(expected_ids - attempted_ids)
    unexpected_ids = sorted(attempted_ids - expected_ids)
    duplicate_result_rows = {k: v for k, v in attempted_counts.items() if v > 1}

    (out / "coverage-gaps.json").write_text(
        json.dumps(
            {
                "missing_manifest_feed_ids": missing_ids,
                "unexpected_feed_ids": unexpected_ids,
                "duplicate_result_rows": duplicate_result_rows,
                "malformed_jsonl_lines": malformed_lines,
            },
            indent=2,
            ensure_ascii=False,
        )
        + "\n",
        encoding="utf-8",
    )

    if args.expected_analyzer_sha:
        wrong = sorted(
            {
                (r.get("analyzer") or {}).get("commit_sha")
                for r in rows
                if (r.get("analyzer") or {}).get("commit_sha")
                and (r.get("analyzer") or {}).get("commit_sha") != args.expected_analyzer_sha
            }
        )
        if wrong:
            raise SystemExit(f"Analyzer SHA mismatch: {wrong}")

    MAP, AGG = load_map(args.map_file)
    inverse = defaultdict(list)
    for code, rule_ids in MAP.items():
        for rid in rule_ids:
            inverse[rid].append(code)

    feed_csv = []
    analyzer_rules = {}
    md_codes = {}
    divergences = []
    state_pairs = Counter()
    stored_disagreements = []
    sha_groups = defaultdict(list)

    for r in rows:
        feed = r.get("feed") or {}
        fid = feed.get("feed_id", "")
        dl = r.get("download") or {}
        a = r.get("analyzer") or {}
        m = r.get("mobilitydata") or {}
        ast = a.get("state", "NOT_RUN")
        mst = m.get("state", "NOT_RUN")
        state_pairs[(ast, mst)] += 1
        if dl.get("status") == "ok" and dl.get("sha256"):
            sha_groups[dl["sha256"]].append(fid)

        feed_csv.append(
            {
                "corpus_index": feed.get("corpus_index"),
                "feed_id": fid,
                "provider": feed.get("provider", ""),
                "country": feed.get("country", ""),
                "url": feed.get("url", ""),
                "download_status": dl.get("status", "missing"),
                "zip_bytes": dl.get("bytes"),
                "sha256": dl.get("sha256"),
                "analyzer_state": ast,
                "analyzer_exit": a.get("exit_code"),
                "analyzer_notices": a.get("notice_count"),
                "analyzer_wall_s": (a.get("timing") or {}).get("elapsed_seconds"),
                "analyzer_rss_kb": (a.get("timing") or {}).get("max_rss_kb"),
                "md_state": mst,
                "md_exit": m.get("exit_code"),
                "md_notices": m.get("notice_total"),
                "md_groups": m.get("notice_groups"),
                "md_wall_s": (m.get("timing") or {}).get("elapsed_seconds"),
                "md_rss_kb": (m.get("timing") or {}).get("max_rss_kb"),
                "md_validation_s": m.get("validation_time_seconds"),
                "runner_exception": r.get("runner_exception"),
            }
        )

        if dl.get("status") == "ok" and (ast, mst) != ("COMPLETE", "COMPLETE") and ast != mst:
            divergences.append(
                {
                    "type": "validator_state_asymmetry",
                    "feed_id": fid,
                    "provider": feed.get("provider", ""),
                    "country": feed.get("country", ""),
                    "url": feed.get("url", ""),
                    "analyzer_state": ast,
                    "md_state": mst,
                    "analyzer_fatal": a.get("fatal"),
                    "analyzer_partial": a.get("partial"),
                    "analyzer_stderr": a.get("stderr_head", ""),
                    "md_stderr": m.get("stderr_head", ""),
                }
            )

        for rid, count in (a.get("by_rule") or {}).items():
            ent = analyzer_rules.setdefault(
                rid,
                {
                    "rule_id": rid,
                    "feed_count": 0,
                    "notice_total": 0,
                    "affected_feeds": [],
                    "classes": Counter(),
                    "severities": Counter(),
                    "sample": None,
                },
            )
            ent["feed_count"] += 1
            ent["notice_total"] += int(count)
            ent["affected_feeds"].append(fid)
            sm = (a.get("samples") or {}).get(rid) or {}
            if sm.get("rule_class"):
                ent["classes"][sm["rule_class"]] += 1
            if sm.get("severity"):
                ent["severities"][sm["severity"]] += 1
            if ent["sample"] is None:
                ent["sample"] = {"feed_id": fid, "notice": sm}

        mc = md_counts(m)
        for code, count in mc.items():
            ent = md_codes.setdefault(
                code,
                {
                    "code": code,
                    "feed_count": 0,
                    "notice_total": 0,
                    "affected_feeds": [],
                    "severities": Counter(),
                    "sample": None,
                },
            )
            ent["feed_count"] += 1
            ent["notice_total"] += int(count)
            ent["affected_feeds"].append(fid)
            sev = md_severity(m, code)
            if sev:
                ent["severities"][sev] += 1
            if ent["sample"] is None:
                ent["sample"] = {"feed_id": fid, "sample": md_sample(m, code)}

        stored = r.get("mobilitydata_stored") or {}
        if mst == "COMPLETE" and stored.get("status") == "available":
            stored_counts = md_counts(stored)
            if mc != stored_counts:
                stored_disagreements.append({"feed_id": fid, "fresh": mc, "stored": stored_counts})

        if ast != "COMPLETE" or mst != "COMPLETE":
            continue

        ac = {k: int(v) for k, v in (a.get("by_rule") or {}).items()}
        for code, mdc in mc.items():
            rules = MAP.get(code)
            sev = md_severity(m, code)
            if not rules:
                divergences.append(
                    {
                        "type": "md_unmapped",
                        "feed_id": fid,
                        "provider": feed.get("provider", ""),
                        "country": feed.get("country", ""),
                        "url": feed.get("url", ""),
                        "md_code": code,
                        "md_count": mdc,
                        "md_severity": sev,
                        "md_samples": md_sample(m, code),
                    }
                )
                continue
            our = sum(ac.get(x, 0) for x in rules)
            agg = any(x in AGG for x in rules)
            if our == 0:
                typ = "md_mapped_missing"
            elif agg:
                typ = "mapped_aggregation_present"
            elif our < mdc:
                typ = "md_mapped_under"
            elif our > mdc:
                typ = "md_mapped_over"
            else:
                typ = "mapped_exact"
            if typ not in ("mapped_exact", "mapped_aggregation_present"):
                divergences.append(
                    {
                        "type": typ,
                        "feed_id": fid,
                        "provider": feed.get("provider", ""),
                        "country": feed.get("country", ""),
                        "url": feed.get("url", ""),
                        "md_code": code,
                        "md_count": mdc,
                        "md_severity": sev,
                        "mapped_analyzer_rules": rules,
                        "analyzer_mapped_count": our,
                        "analyzer_rule_counts": {x: ac.get(x, 0) for x in rules},
                        "md_samples": md_sample(m, code),
                        "analyzer_samples": {
                            x: (a.get("samples") or {}).get(x) for x in rules if ac.get(x, 0)
                        },
                    }
                )

        for rid, ourc in ac.items():
            sm = (a.get("samples") or {}).get(rid) or {}
            cls = (sm.get("rule_class") or "").upper()
            sev = (sm.get("severity") or "").upper()
            codes = inverse.get(rid, [])
            if codes:
                mt = sum(mc.get(code, 0) for code in codes)
                if mt == 0:
                    divergences.append(
                        {
                            "type": "analyzer_spec_md_absent" if cls == "SPEC" else "analyzer_mapped_md_absent",
                            "feed_id": fid,
                            "provider": feed.get("provider", ""),
                            "country": feed.get("country", ""),
                            "url": feed.get("url", ""),
                            "analyzer_rule": rid,
                            "analyzer_count": ourc,
                            "analyzer_class": cls,
                            "analyzer_severity": sev,
                            "mapped_md_codes": codes,
                            "analyzer_sample": sm,
                        }
                    )
            elif cls == "SPEC":
                divergences.append(
                    {
                        "type": "analyzer_spec_unmapped",
                        "feed_id": fid,
                        "provider": feed.get("provider", ""),
                        "country": feed.get("country", ""),
                        "url": feed.get("url", ""),
                        "analyzer_rule": rid,
                        "analyzer_count": ourc,
                        "analyzer_class": cls,
                        "analyzer_severity": sev,
                        "analyzer_sample": sm,
                    }
                )

    for d in divergences:
        d["priority"] = priority(d)
    divergences.sort(
        key=lambda d: (
            -d["priority"],
            d.get("feed_id", ""),
            d.get("md_code", ""),
            d.get("analyzer_rule", ""),
        )
    )

    duplicate_content = [
        {"sha256": sha, "feed_count": len(ids), "feed_ids": sorted(ids)}
        for sha, ids in sha_groups.items()
        if len(ids) > 1
    ]
    duplicate_content.sort(key=lambda x: (-x["feed_count"], x["sha256"]))
    (out / "duplicate-content-groups.json").write_text(
        json.dumps(duplicate_content, indent=2, ensure_ascii=False) + "\n",
        encoding="utf-8",
    )

    ar_list = []
    for rid, e in analyzer_rules.items():
        ar_list.append(
            {
                "rule_id": rid,
                "feed_count": e["feed_count"],
                "notice_total": e["notice_total"],
                "classes": dict(e["classes"]),
                "severities": dict(e["severities"]),
                "affected_feeds": e["affected_feeds"],
                "sample": e["sample"],
            }
        )
    ar_list.sort(key=lambda x: (-x["feed_count"], -x["notice_total"], x["rule_id"]))

    md_list = []
    for code, e in md_codes.items():
        md_list.append(
            {
                "code": code,
                "feed_count": e["feed_count"],
                "notice_total": e["notice_total"],
                "severities": dict(e["severities"]),
                "affected_feeds": e["affected_feeds"],
                "sample": e["sample"],
            }
        )
    md_list.sort(key=lambda x: (-x["feed_count"], -x["notice_total"], x["code"]))

    write_csv(out / "feed-summary.csv", feed_csv, list(feed_csv[0]) if feed_csv else ["feed_id"])
    dfields = [
        "priority",
        "type",
        "feed_id",
        "provider",
        "country",
        "md_code",
        "md_severity",
        "md_count",
        "mapped_analyzer_rules",
        "analyzer_mapped_count",
        "analyzer_rule",
        "analyzer_class",
        "analyzer_severity",
        "analyzer_count",
        "analyzer_state",
        "md_state",
        "url",
    ]
    write_csv(out / "divergence-candidates.csv", divergences, dfields)
    (out / "divergence-candidates-full.json").write_text(
        json.dumps(divergences, indent=2, ensure_ascii=False) + "\n", encoding="utf-8"
    )
    (out / "analyzer-rules-full.json").write_text(
        json.dumps(ar_list, indent=2, ensure_ascii=False) + "\n", encoding="utf-8"
    )
    (out / "md-codes-full.json").write_text(
        json.dumps(md_list, indent=2, ensure_ascii=False) + "\n", encoding="utf-8"
    )
    (out / "stored-vs-fresh-md-disagreements.json").write_text(
        json.dumps(stored_disagreements, indent=2, ensure_ascii=False) + "\n", encoding="utf-8"
    )

    reps = []
    seen = set()
    for d in divergences:
        if d["type"] == "validator_state_asymmetry":
            key = (d["type"], d.get("analyzer_state"), d.get("md_state"))
        elif d.get("md_code"):
            key = (d["type"], d.get("md_code"), tuple(d.get("mapped_analyzer_rules") or []))
        else:
            key = (d["type"], d.get("analyzer_rule"))
        if key in seen:
            continue
        seen.add(key)
        reps.append(d)
        if len(reps) >= 140:
            break
    (out / "triage-representatives.json").write_text(
        json.dumps(reps, indent=2, ensure_ascii=False) + "\n", encoding="utf-8"
    )

    types = Counter(d["type"] for d in divergences)
    downloaded = sum(1 for r in rows if (r.get("download") or {}).get("status") == "ok")
    both = sum(
        1
        for r in rows
        if (r.get("analyzer") or {}).get("state") == "COMPLETE"
        and (r.get("mobilitydata") or {}).get("state") == "COMPLETE"
    )
    astate = Counter((r.get("analyzer") or {}).get("state", "NOT_RUN") for r in rows)
    mstate = Counter((r.get("mobilitydata") or {}).get("state", "NOT_RUN") for r in rows)
    a_wall = [(r.get("analyzer") or {}).get("timing", {}).get("elapsed_seconds") for r in rows]
    m_wall = [(r.get("mobilitydata") or {}).get("timing", {}).get("elapsed_seconds") for r in rows]
    a_rss = [(r.get("analyzer") or {}).get("timing", {}).get("max_rss_kb") for r in rows]
    m_rss = [(r.get("mobilitydata") or {}).get("timing", {}).get("max_rss_kb") for r in rows]

    summary = {
        "scope": "MobilityDatabase runnable GTFS Schedule only",
        "gtfs_realtime_tested": 0,
        "manifest_selected_count": len(expected),
        "attempted_rows": len(rows),
        "attempted_unique_feeds": len(attempted_ids),
        "missing_manifest_feeds": len(missing_ids),
        "unexpected_feeds": len(unexpected_ids),
        "malformed_jsonl_lines": len(malformed_lines),
        "downloaded_ok": downloaded,
        "both_validators_complete": both,
        "analyzer_state_counts": dict(astate),
        "mobilitydata_state_counts": dict(mstate),
        "state_pairs": {f"{a} | {m}": n for (a, m), n in state_pairs.items()},
        "duplicate_content_groups": len(duplicate_content),
        "duplicate_content_rows": sum(x["feed_count"] for x in duplicate_content),
        "unique_analyzer_rules_seen": len(ar_list),
        "unique_md_codes_seen": len(md_list),
        "divergence_candidate_counts": dict(types),
        "fresh_vs_stored_md_report_count_different": len(stored_disagreements),
        "performance": {
            "analyzer_wall_seconds": perf(a_wall),
            "mobilitydata_wall_seconds": perf(m_wall),
            "analyzer_peak_rss_kb": perf(a_rss),
            "mobilitydata_peak_rss_kb": perf(m_rss),
        },
        "selection": manifest.get("selection") or {},
    }
    (out / "summary.json").write_text(
        json.dumps(summary, indent=2, ensure_ascii=False) + "\n", encoding="utf-8"
    )

    lines = [
        "# Full MobilityDatabase GTFS Schedule validator audit",
        "",
        "## Scope",
        "",
        "- GTFS-Realtime tested: **0**",
        f"- Schedule manifest: **{len(expected)}** feeds",
        f"- Attempted unique feeds: **{len(attempted_ids)}**",
        f"- Missing manifest feeds: **{len(missing_ids)}**",
        f"- Successful ZIP downloads: **{downloaded}**",
        f"- Both validators COMPLETE: **{both}**",
        f"- Explicit exclusions: `{json.dumps((manifest.get('selection') or {}).get('explicit_exclusions', {}), ensure_ascii=False)}`",
        "",
        "This full-catalog run is a new population measurement; it is not a code-regression diff against the old stratified corpus.",
        "",
        "## Validator state pairs",
        "",
        "| Analyzer | MobilityData | Feeds |",
        "|---|---|---:|",
    ]
    for (ast, mst), n in state_pairs.most_common():
        lines.append(f"| {ast} | {mst} | {n} |")
    lines += [
        "",
        "## Duplicate downloaded content",
        "",
        f"- SHA-256 duplicate groups: **{len(duplicate_content)}**",
        f"- Feed rows in duplicate groups: **{sum(x['feed_count'] for x in duplicate_content)}**",
        "",
        "## Divergence candidate classes",
        "",
        "Automated differences are triage candidates, not correctness verdicts.",
        "",
        "| Candidate class | Count |",
        "|---|---:|",
    ]
    for k, n in types.most_common():
        lines.append(f"| {k} | {n} |")
    lines += [
        "",
        "## Performance",
        "",
        f"- Analyzer wall: `{json.dumps(summary['performance']['analyzer_wall_seconds'])}`",
        f"- MobilityData wall: `{json.dumps(summary['performance']['mobilitydata_wall_seconds'])}`",
        f"- Analyzer peak RSS: `{json.dumps(summary['performance']['analyzer_peak_rss_kb'])}`",
        f"- MobilityData peak RSS: `{json.dumps(summary['performance']['mobilitydata_peak_rss_kb'])}`",
        "",
        "## Highest-priority candidates",
        "",
        "| Priority | Feed | Direction | MD code | Analyzer rule(s) | Counts |",
        "|---:|---|---|---|---|---|",
    ]
    for d in divergences[:80]:
        rules = d.get("mapped_analyzer_rules") or ([d["analyzer_rule"]] if d.get("analyzer_rule") else [])
        counts = ""
        if d.get("md_count") is not None:
            counts += f"MD {d.get('md_count')}"
        if d.get("analyzer_mapped_count") is not None:
            counts += f" / A {d.get('analyzer_mapped_count')}"
        elif d.get("analyzer_count") is not None:
            counts += f" / A {d.get('analyzer_count')}"
        lines.append(
            f"| {d['priority']} | {d.get('feed_id','')} | {d['type']} | {d.get('md_code','')} | {', '.join(rules)} | {counts} |"
        )
    (out / "AUDIT_SUMMARY.md").write_text("\n".join(lines) + "\n", encoding="utf-8")
    print(json.dumps(summary, indent=2, ensure_ascii=False))


if __name__ == "__main__":
    main()
