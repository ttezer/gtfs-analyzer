#!/usr/bin/env python3
"""Reuse the exact historical 1000-feed manifest and attach baseline ZIP hashes.

This rerun intentionally has no fresh-catalog selection path. The only accepted
corpus source is the manifest artifact from workflow run 31284358651 together
with that run's all-results.json.gz.
"""
from __future__ import annotations

import argparse
import gzip
import hashlib
import json
from pathlib import Path

DEFAULT_SEED = "gtfs-analyzer-audit-1000-v1"
BASELINE_RUN_ID = 31284358651
BASELINE_CORPUS_ARTIFACT_ID = 9029347252
BASELINE_FINAL_ARTIFACT_ID = 9029535391
BASELINE_ANALYZER_BRANCH_HEAD = "0c2ea5f1b769f9eb92aa7010234a35c6335f5752"


def load_json(path: Path):
    if path.suffix == ".gz":
        with gzip.open(path, "rt", encoding="utf-8") as fh:
            return json.load(fh)
    return json.loads(path.read_text(encoding="utf-8"))


def reuse_manifest(manifest_path: Path, baseline_results_path: Path, out_path: Path) -> dict:
    manifest = load_json(manifest_path)
    rows = load_json(baseline_results_path)
    feeds = manifest.get("feeds") or []
    if len(feeds) != 1000:
        raise SystemExit(f"baseline manifest must contain exactly 1000 feeds, found {len(feeds)}")
    if len(rows) != 1000:
        raise SystemExit(f"baseline results must contain exactly 1000 rows, found {len(rows)}")

    old_by_id = {}
    for row in rows:
        feed = row.get("feed") or {}
        fid = feed.get("feed_id")
        if fid:
            if fid in old_by_id:
                raise SystemExit(f"duplicate baseline feed_id in results: {fid}")
            old_by_id[fid] = row

    seen_ids = set()
    missing = []
    no_sha = []
    for feed in feeds:
        fid = feed.get("feed_id")
        if not fid:
            raise SystemExit("baseline manifest contains a feed without feed_id")
        if fid in seen_ids:
            raise SystemExit(f"duplicate feed_id in baseline manifest: {fid}")
        seen_ids.add(fid)
        old = old_by_id.get(fid)
        if not old:
            missing.append(fid)
            continue
        dl = old.get("download") or {}
        feed["baseline_download_status"] = dl.get("status")
        feed["baseline_download_sha256"] = dl.get("sha256")
        feed["baseline_download_bytes"] = dl.get("bytes")
        if not dl.get("sha256"):
            no_sha.append(fid)

    if missing:
        raise SystemExit(f"baseline results missing {len(missing)} manifest feeds; first={missing[:10]}")
    if no_sha:
        raise SystemExit(f"baseline results missing ZIP SHA-256 for {len(no_sha)} feeds; first={no_sha[:10]}")

    selection = manifest.setdefault("selection", {})
    if selection.get("seed") != DEFAULT_SEED:
        raise SystemExit(
            f"unexpected baseline seed {selection.get('seed')!r}; expected {DEFAULT_SEED!r}"
        )
    if int(selection.get("selected_count") or 0) != 1000:
        raise SystemExit(f"baseline selection metadata is not 1000 feeds: {selection.get('selected_count')!r}")

    manifest["rerun_baseline"] = {
        "source_workflow_run": BASELINE_RUN_ID,
        "corpus_artifact": "audit1000-corpus",
        "corpus_artifact_id": BASELINE_CORPUS_ARTIFACT_ID,
        "final_artifact": "validator-audit-1000-final",
        "final_artifact_id": BASELINE_FINAL_ARTIFACT_ID,
        "baseline_analyzer_branch_head": BASELINE_ANALYZER_BRANCH_HEAD,
        "mobilitydata_version": "8.0.1",
        "analyzer_validation_date": "20260809",
        "mobilitydata_validation_date": "2026-08-09",
        "manifest_sha256": hashlib.sha256(manifest_path.read_bytes()).hexdigest(),
        "baseline_results_sha256": hashlib.sha256(baseline_results_path.read_bytes()).hexdigest(),
    }
    out_path.parent.mkdir(parents=True, exist_ok=True)
    out_path.write_text(json.dumps(manifest, indent=2, ensure_ascii=False) + "\n", encoding="utf-8")
    summary = {
        "selected_count": len(feeds),
        "seed": selection.get("seed"),
        "manifest_sha256": manifest["rerun_baseline"]["manifest_sha256"],
        "baseline_results_sha256": manifest["rerun_baseline"]["baseline_results_sha256"],
        "feeds_with_baseline_sha256": sum(bool(f.get("baseline_download_sha256")) for f in feeds),
    }
    print(json.dumps(summary, indent=2))
    return manifest


def main() -> None:
    ap = argparse.ArgumentParser()
    ap.add_argument("--reuse-manifest", required=True)
    ap.add_argument("--baseline-results", required=True)
    ap.add_argument("--out", required=True)
    args = ap.parse_args()
    reuse_manifest(Path(args.reuse_manifest), Path(args.baseline_results), Path(args.out))


if __name__ == "__main__":
    main()
