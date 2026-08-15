#!/usr/bin/env python3
"""Select every runnable GTFS Schedule feed from the MobilityDatabase catalog.

Only exact data_type=gtfs rows enter the corpus. Every other data type — including
GTFS-Realtime if present in the exported catalog — is rejected before validation.
This is not a sample: every active/open Schedule row with a public latest URL is
retained, except for explicitly documented feed exclusions.
"""
from __future__ import annotations

import argparse
import csv
import hashlib
import json
import urllib.request
from collections import Counter
from pathlib import Path

CATALOG_URL = "https://files.mobilitydatabase.org/feeds_v2.csv"
EXCLUDED_FEEDS = {
    "mdb-2904": "documented corpus exclusion: pathological finding volume",
}


def feed_id(row: dict[str, str]) -> str:
    raw = (row.get("mdb_source_id") or "").strip()
    if not raw:
        return ""
    return f"mdb-{raw}" if raw.isdigit() else raw


def eligible(row: dict[str, str]) -> bool:
    return (
        (row.get("data_type") or "").strip() == "gtfs"
        and (row.get("status") or "").strip() in ("active", "")
        and (row.get("urls.authentication_type") or "").strip() in ("", "0")
        and bool((row.get("urls.latest") or "").strip())
        and not (row.get("redirect.id") or "").strip()
    )


def sort_key(row: dict[str, str]):
    fid = feed_id(row)
    tail = fid.removeprefix("mdb-")
    return (0, int(tail)) if tail.isdigit() else (1, fid)


def main() -> None:
    ap = argparse.ArgumentParser()
    ap.add_argument("--out", required=True)
    ap.add_argument("--catalog-out")
    ap.add_argument("--min-count", type=int, default=1400)
    args = ap.parse_args()

    raw = urllib.request.urlopen(CATALOG_URL, timeout=120).read()
    if args.catalog_out:
        Path(args.catalog_out).write_bytes(raw)

    rows = list(csv.DictReader(raw.decode("utf-8-sig").splitlines()))
    data_type_counts = Counter((r.get("data_type") or "").strip() or "(blank)" for r in rows)
    funnel = Counter()
    funnel["catalog_total"] = len(rows)
    funnel["gtfs_schedule"] = data_type_counts.get("gtfs", 0)

    stage = [r for r in rows if (r.get("data_type") or "").strip() == "gtfs"]
    funnel["schedule_active_or_blank"] = sum((r.get("status") or "").strip() in ("active", "") for r in stage)
    stage = [r for r in stage if (r.get("status") or "").strip() in ("active", "")]
    funnel["schedule_no_auth"] = sum((r.get("urls.authentication_type") or "").strip() in ("", "0") for r in stage)
    stage = [r for r in stage if (r.get("urls.authentication_type") or "").strip() in ("", "0")]
    funnel["schedule_has_latest"] = sum(bool((r.get("urls.latest") or "").strip()) for r in stage)
    stage = [r for r in stage if (r.get("urls.latest") or "").strip()]
    funnel["schedule_not_redirect"] = sum(not (r.get("redirect.id") or "").strip() for r in stage)

    pool = [r for r in rows if eligible(r)]
    before_explicit = len(pool)
    pool = [r for r in pool if feed_id(r) not in EXCLUDED_FEEDS]
    pool.sort(key=sort_key)

    if len(pool) < args.min_count:
        raise SystemExit(
            f"eligible Schedule corpus unexpectedly small: {len(pool)} < {args.min_count}; "
            f"funnel={dict(funnel)} data_types={dict(data_type_counts)}"
        )

    feeds = []
    for index, row in enumerate(pool):
        fid = feed_id(row)
        url = (row.get("urls.latest") or "").strip()
        feeds.append(
            {
                "corpus_index": index,
                "feed_id": fid,
                "data_type": "gtfs",
                "provider": (row.get("provider") or "").strip(),
                "country": (row.get("location.country_code") or "").strip(),
                "status": (row.get("status") or "active").strip() or "active",
                "authentication_type": (row.get("urls.authentication_type") or "0").strip() or "0",
                "features": (row.get("features") or "").strip(),
                "official": (row.get("is_official") or "").strip(),
                "url": url,
                "mobilitydatabase_page": f"https://mobilitydatabase.org/feeds/gtfs/{fid}",
                "stored_md_report_url": url.rsplit("/", 1)[0] + "/report_8.0.1.json",
            }
        )

    if any(f["data_type"] != "gtfs" for f in feeds):
        raise SystemExit("non-Schedule feed entered corpus")
    if any(f["feed_id"] in EXCLUDED_FEEDS for f in feeds):
        raise SystemExit("explicitly excluded feed entered corpus")

    urls = Counter(f["url"] for f in feeds)
    duplicate_url_groups = {u: n for u, n in urls.items() if n > 1}
    country_counts = Counter(f["country"] or "(unknown)" for f in feeds)

    out = {
        "schema_version": 2,
        "selection": {
            "catalog_url": CATALOG_URL,
            "catalog_sha256": hashlib.sha256(raw).hexdigest(),
            "catalog_data_type_counts": dict(data_type_counts),
            "method": "all runnable MobilityDatabase GTFS Schedule rows; no sampling and no URL deduplication",
            "filter": "data_type=gtfs; status active/blank; authentication none; urls.latest present; redirect.id blank",
            "gtfs_realtime_policy": "only exact data_type=gtfs is admitted; all other data types, including GTFS-Realtime, are excluded before validation",
            "funnel": dict(funnel),
            "eligible_before_explicit_exclusions": before_explicit,
            "explicit_exclusions": EXCLUDED_FEEDS,
            "selected_count": len(feeds),
            "country_counts": dict(country_counts),
            "duplicate_latest_url_groups": len(duplicate_url_groups),
            "duplicate_latest_url_rows": sum(duplicate_url_groups.values()),
        },
        "feeds": feeds,
    }
    Path(args.out).write_text(json.dumps(out, indent=2, ensure_ascii=False) + "\n", encoding="utf-8")
    print(json.dumps(out["selection"], indent=2, ensure_ascii=False))


if __name__ == "__main__":
    main()
