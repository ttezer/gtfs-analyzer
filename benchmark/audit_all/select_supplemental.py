#!/usr/bin/env python3
"""Build a supplemental corpus from the same full-audit catalog snapshot.

The primary run covered direct, public, active Schedule feeds. This selector adds
all direct/public non-active Schedule rows (inactive/future/development/deprecated)
without mixing them into the primary population statistics. Redirect rows are
aliases, not independent payloads; auth/no-URL rows are accounted for but cannot
be directly validated.
"""
from __future__ import annotations

import argparse
import csv
import hashlib
import json
import sys
from collections import Counter, defaultdict
from pathlib import Path

# Tek tanım: ana selektörün dışlama listesi burada da geçerlidir. Ayrı bir kopya
# tutmak, iki listenin sessizce ayrışması demekti — #150 tam olarak öyle oldu
# (kapı `spec-audit/`'teydi, `benchmark/` bilmiyordu).
sys.path.insert(0, str(Path(__file__).resolve().parent))
from select_corpus import EXCLUDED_FEEDS


def yes_noauth(row):
    return (row.get("urls.authentication_type") or "").strip() in ("", "0")


def feed_id(row):
    return (row.get("id") or "").strip()


def main():
    ap=argparse.ArgumentParser()
    ap.add_argument("--catalog", required=True)
    ap.add_argument("--out", required=True)
    args=ap.parse_args()

    raw=Path(args.catalog).read_bytes()
    rows=list(csv.DictReader(raw.decode("utf-8-sig").splitlines()))
    schedule=[r for r in rows if (r.get("data_type") or "").strip()=="gtfs"]

    accounting=Counter()
    accounting_by_status=defaultdict(Counter)
    direct=[]
    for r in schedule:
        status=(r.get("status") or "").strip() or "(blank)"
        if (r.get("redirect.id") or "").strip():
            cat="redirect_alias"
        elif not yes_noauth(r):
            cat="auth_required"
        elif not (r.get("urls.latest") or "").strip():
            cat="no_latest_url"
        else:
            cat="direct_runnable"
            direct.append(r)
        accounting[cat]+=1
        accounting_by_status[cat][status]+=1

    supplemental=[r for r in direct if (r.get("status") or "").strip() not in ("active", "")]
    # Talimatla dışlananlar burada da koşulmaz; SESSİZCE düşmesin diye sayılır.
    excluded_by_instruction=sorted({feed_id(r) for r in supplemental} & EXCLUDED_FEEDS)
    supplemental=[r for r in supplemental if feed_id(r) not in EXCLUDED_FEEDS]
    supplemental.sort(key=lambda r: feed_id(r))
    ids=[feed_id(r) for r in supplemental]
    if any(not x for x in ids):
        raise SystemExit("blank canonical id in supplemental corpus")
    if len(ids)!=len(set(ids)):
        raise SystemExit("duplicate canonical id in supplemental corpus")

    feeds=[]
    for i,r in enumerate(supplemental):
        fid=feed_id(r)
        url=(r.get("urls.latest") or "").strip()
        feeds.append({
            "corpus_index":i,
            "feed_id":fid,
            "data_type":"gtfs",
            "provider":(r.get("provider") or "").strip(),
            "country":(r.get("location.country_code") or "").strip(),
            "status":(r.get("status") or "").strip(),
            "authentication_type":(r.get("urls.authentication_type") or "0").strip() or "0",
            "features":(r.get("features") or "").strip(),
            "official":(r.get("is_official") or "").strip(),
            "url":url,
            "mobilitydatabase_page":f"https://mobilitydatabase.org/feeds/gtfs/{fid}",
            "stored_md_report_url":url.rsplit("/",1)[0]+"/report_8.0.1.json",
        })

    leaked=sorted({f["feed_id"] for f in feeds} & EXCLUDED_FEEDS)
    if leaked:
        raise SystemExit(f"excluded feed(s) reached the supplemental manifest: {', '.join(leaked)}")

    out={
        "schema_version":1,
        "selection":{
            "excluded_by_instruction":excluded_by_instruction,
            "catalog_sha256":hashlib.sha256(raw).hexdigest(),
            "scope":"supplemental direct/public non-active GTFS Schedule rows from primary snapshot",
            "gtfs_realtime_policy":"excluded by exact data_type=gtfs before all classification",
            "schedule_catalog_rows":len(schedule),
            "accounting":dict(accounting),
            "accounting_by_status":{k:dict(v) for k,v in accounting_by_status.items()},
            "direct_runnable_total":len(direct),
            "primary_active_direct_expected":sum((r.get("status") or "").strip() in ("active", "") for r in direct),
            "supplemental_selected_count":len(feeds),
        },
        "feeds":feeds,
    }
    Path(args.out).write_text(json.dumps(out,indent=2,ensure_ascii=False)+"\n",encoding="utf-8")
    print(json.dumps(out["selection"],indent=2,ensure_ascii=False))

if __name__=="__main__":
    main()
