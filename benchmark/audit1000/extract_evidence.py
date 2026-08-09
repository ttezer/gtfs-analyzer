#!/usr/bin/env python3
"""Re-download high-priority candidate feeds and capture source-row evidence.

The re-download is accepted only if its SHA-256 matches the ZIP used by the audit run.
This prevents evidence from silently referring to a later latest.zip snapshot.
"""
from __future__ import annotations

import argparse
import gzip
import hashlib
import json
import re
import subprocess
import tempfile
import zipfile
from pathlib import Path

MAX_FEEDS = 100
MAX_DOWNLOAD = 1_200_000_000


def load_json(path):
    path = Path(path)
    if path.suffix == ".gz":
        with gzip.open(path, "rt", encoding="utf-8") as fh:
            return json.load(fh)
    return json.loads(path.read_text(encoding="utf-8"))


def download(url, dest):
    proc = subprocess.run([
        "curl", "--fail", "--location", "--silent", "--show-error",
        "--retry", "2", "--retry-all-errors", "--connect-timeout", "20",
        "--max-time", "300", "--max-filesize", str(MAX_DOWNLOAD),
        url, "-o", str(dest),
    ], stdout=subprocess.PIPE, stderr=subprocess.PIPE)
    return proc.returncode, proc.stderr.decode("utf-8", "replace")[:2000]


def sha256_file(path):
    h = hashlib.sha256()
    with Path(path).open("rb") as fh:
        for chunk in iter(lambda: fh.read(8 * 1024 * 1024), b""):
            h.update(chunk)
    return h.hexdigest()


def refs_from_obj(obj):
    refs = []

    def walk(value):
        if isinstance(value, dict):
            filename = None
            line = None
            for key, item in value.items():
                norm = re.sub(r"[^a-z0-9]", "", key.lower())
                if norm in ("file", "filename", "csvfile", "tablename") and isinstance(item, str) and item.endswith(".txt"):
                    filename = item
                if norm in ("line", "linenumber", "csvrownumber", "rownumber", "row") and isinstance(item, (int, float, str)):
                    try:
                        line = int(item)
                    except (TypeError, ValueError):
                        pass
            if filename:
                refs.append((filename, line))
            for item in value.values():
                walk(item)
        elif isinstance(value, list):
            for item in value:
                walk(item)

    walk(obj)
    out, seen = [], set()
    for ref in refs:
        if ref not in seen:
            seen.add(ref)
            out.append(ref)
    return out[:12]


def extract_context(zf, filename, line):
    names = zf.namelist()
    target = filename if filename in names else next((n for n in names if n.rsplit("/", 1)[-1] == filename), None)
    if not target:
        return {"file": filename, "error": "not found"}
    try:
        with zf.open(target) as fh:
            if line is None:
                arr = []
                for idx, raw in enumerate(fh, 1):
                    arr.append({"line": idx, "text": raw.decode("utf-8-sig", "replace").rstrip("\r\n")[:1000]})
                    if idx >= 6:
                        break
                return {"file": target, "requested_line": None, "context": arr}
            lo, hi = max(1, line - 2), line + 2
            arr = []
            for idx, raw in enumerate(fh, 1):
                if idx < lo:
                    continue
                if idx > hi:
                    break
                arr.append({"line": idx, "text": raw.decode("utf-8-sig", "replace").rstrip("\r\n")[:1500]})
            return {"file": target, "requested_line": line, "context": arr}
    except Exception as exc:
        return {"file": target, "requested_line": line, "error": repr(exc)}


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--reps", required=True)
    ap.add_argument("--all-results", required=True)
    ap.add_argument("--out", required=True)
    ap.add_argument("--max", type=int, default=MAX_FEEDS)
    args = ap.parse_args()

    reps = load_json(args.reps)
    rows = load_json(args.all_results)
    run_by_id = {(r.get("feed") or {}).get("feed_id"): r for r in rows}
    selected = [r for r in reps if int(r.get("priority", 0)) >= 80][:args.max]
    output = []

    with tempfile.TemporaryDirectory() as td:
        td = Path(td)
        for idx, candidate in enumerate(selected, 1):
            fid = candidate.get("feed_id", "unknown")
            print(f"[evidence] {idx}/{len(selected)} {fid} {candidate.get('type')}", flush=True)
            run = run_by_id.get(fid) or {}
            feed = run.get("feed") or {}
            dl = run.get("download") or {}
            expected_sha = dl.get("sha256")
            url = feed.get("url") or candidate.get("url")
            item = {
                "candidate": candidate,
                "audit_input_classification": run.get("input_classification"),
                "audit_zip_sha256": expected_sha,
                "url": url,
            }
            if not url or not expected_sha:
                item["evidence_status"] = "NO_PINNED_AUDIT_INPUT"
                output.append(item)
                continue
            zpath = td / "feed.zip"
            if zpath.exists():
                zpath.unlink()
            rc, err = download(url, zpath)
            item["download_exit"] = rc
            if rc != 0:
                item["evidence_status"] = "DOWNLOAD_FAILED"
                item["download_error"] = err
                output.append(item)
                continue
            got_sha = sha256_file(zpath)
            item["redownload_sha256"] = got_sha
            if got_sha != expected_sha:
                item["evidence_status"] = "EVIDENCE_INPUT_DRIFT"
                output.append(item)
                continue
            item["evidence_status"] = "SAME_AS_AUDIT_INPUT"
            try:
                with zipfile.ZipFile(zpath) as zf:
                    refs = refs_from_obj({
                        "analyzer_sample": candidate.get("analyzer_sample"),
                        "analyzer_samples": candidate.get("analyzer_samples"),
                        "md_samples": candidate.get("md_samples"),
                    })
                    item["archive_files"] = zf.namelist()[:100]
                    item["refs"] = refs
                    item["contexts"] = [extract_context(zf, *ref) for ref in refs]
            except Exception as exc:
                item["evidence_status"] = "ZIP_ERROR"
                item["zip_error"] = repr(exc)
            output.append(item)

    Path(args.out).write_text(json.dumps(output, indent=2, ensure_ascii=False) + "\n", encoding="utf-8")


if __name__ == "__main__":
    main()
