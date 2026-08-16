#!/usr/bin/env python3
import argparse,csv,hashlib,json,re,urllib.request
from collections import Counter
from pathlib import Path

CATALOG_URL="https://files.mobilitydatabase.org/feeds_v2.csv"

# 🚫 Feeds that must never enter a corpus run, by standing instruction.
#
# `mdb-2904` is 85 MB and on its own produced 5,374,302 findings and 14.4 GB of peak
# RSS in run-31934698855 -- roughly a tenth of that run's entire finding volume from
# one feed. The gate already existed in `spec-audit/` (post_baseline_measure.py,
# refresh_rule_stats.py, corpus_report.py) but this selector is a separate code path
# and never learned about it, so the feed entered the full-catalog corpus anyway (#150).
#
# ⚠️ Excluded feeds are RECORDED, not silently dropped: they appear in the exclusions
# list with an explicit reason, so "not run by instruction" never reads as "not found".
EXCLUDED_FEEDS = {"mdb-2904"}


def norm(s):
    return re.sub(r'[^a-z0-9]+','',s.lower())


def pick_field(fields, exact=(), contains=()):
    nf={norm(f):f for f in fields}
    for e in exact:
        if norm(e) in nf:
            return nf[norm(e)]
    for f in fields:
        n=norm(f)
        if all(norm(x) in n for x in contains):
            return f
    return None


def clean_id(v):
    v=(v or '').strip()
    if v.isdigit():
        return f"mdb-{v}"
    return v


def main():
    ap=argparse.ArgumentParser()
    ap.add_argument("--out",required=True)
    ap.add_argument("--catalog-out")
    args=ap.parse_args()

    raw=urllib.request.urlopen(CATALOG_URL,timeout=120).read()
    if args.catalog_out:
        Path(args.catalog_out).write_bytes(raw)
    rd=csv.DictReader(raw.decode("utf-8-sig").splitlines())
    fields=rd.fieldnames or []
    fmap={
      "id":pick_field(fields,("mdb_source_id","source_id","id"),("mdb","source","id")),
      "type":pick_field(fields,("data_type","datatype"),("data","type")),
      "status":pick_field(fields,("status",),("status",)),
      "provider":pick_field(fields,("provider",),("provider",)),
      "country":pick_field(fields,("location.country_code","country_code","country"),("country","code")),
      "features":pick_field(fields,("features",),("features",)),
      "official":pick_field(fields,("is_official","official"),("official",)),
      "latest":pick_field(fields,("urls.latest","latest.url","latest_url","latest"),("latest",)),
    }
    missing=[k for k in ("id","type","latest") if not fmap[k]]
    if missing:
        raise SystemExit(f"Cannot identify required catalog fields {missing}; headers={fields}")

    feeds=[]
    excluded=[]
    seen_ids=set()
    type_counts=Counter()
    status_counts=Counter()
    schedule_rows=0

    for row_index,row in enumerate(rd,2):
        typ=(row.get(fmap["type"]) or "").strip().lower()
        type_counts[typ or "(blank)"]+=1
        # Static Schedule only. Realtime rows such as gtfs_rt never enter this corpus.
        if typ!="gtfs":
            continue

        schedule_rows+=1
        status=(row.get(fmap["status"]) or "").strip().lower() if fmap["status"] else ""
        status_counts[status or "(blank)"]+=1
        fid=clean_id(row.get(fmap["id"]))
        url=(row.get(fmap["latest"]) or "").strip()

        if not fid:
            excluded.append({"catalog_row":row_index,"reason":"missing_feed_id","status":status,"url":url})
            continue
        if fid in seen_ids:
            raise SystemExit(f"duplicate GTFS Schedule feed_id in catalog: {fid}")
        seen_ids.add(fid)
        if fid in EXCLUDED_FEEDS:
            excluded.append({"catalog_row":row_index,"feed_id":fid,"reason":"excluded_by_instruction","status":status,"url":url})
            continue
        if not url.startswith(("http://","https://")):
            excluded.append({"catalog_row":row_index,"feed_id":fid,"reason":"no_public_latest_url","status":status,"url":url})
            continue

        feeds.append({
          "feed_id":fid,
          "data_type":"gtfs",
          "catalog_status":status,
          "provider":(row.get(fmap["provider"]) or "").strip() if fmap["provider"] else "",
          "country":(row.get(fmap["country"]) or "").strip() if fmap["country"] else "",
          "features":(row.get(fmap["features"]) or "").strip() if fmap["features"] else "",
          "official":(row.get(fmap["official"]) or "").strip() if fmap["official"] else "",
          "url":url,
          "mobilitydatabase_page":f"https://mobilitydatabase.org/feeds/gtfs/{fid}",
          "stored_md_report_url":url.rsplit("/",1)[0]+"/report_8.0.1.json",
        })

    feeds.sort(key=lambda x:(x["feed_id"],x["url"]))
    for i,x in enumerate(feeds):
        x["corpus_index"]=i

    # Installed gate, not just a written one: if an excluded feed reached the manifest
    # the run must not start. The filter above can be bypassed by a future edit, a
    # different code path, or a hand-built manifest -- this is the check that actually
    # holds. Proven by putting mdb-2904 back in and watching it exit 2 (#150).
    leaked=sorted({x["feed_id"] for x in feeds} & EXCLUDED_FEEDS)
    if leaked:
        raise SystemExit(
            f"excluded feed(s) reached the corpus manifest: {', '.join(leaked)}\n"
            "These are excluded by standing instruction and must never be run."
        )

    excluded_counts=Counter(x["reason"] for x in excluded)
    out={
      "schema_version":3,
      "selection":{
        "excluded_by_instruction":sorted(EXCLUDED_FEEDS),
        "catalog_url":CATALOG_URL,
        "catalog_sha256":hashlib.sha256(raw).hexdigest(),
        "catalog_gtfs_schedule_rows":schedule_rows,
        "testable_schedule_count":len(feeds),
        "selected_count":len(feeds),
        "excluded_schedule_count":len(excluded),
        "excluded_reason_counts":dict(excluded_counts),
        "filter":"all catalog rows with data_type=gtfs (GTFS Schedule); any catalog status; public HTTP(S) latest URL required to execute; no URL deduplication; GTFS Realtime excluded",
        "method":"exhaustive GTFS Schedule catalog coverage; no sampling and no URL deduplication",
        "field_map":fmap,
        "catalog_data_type_counts":dict(type_counts),
        "schedule_status_counts":dict(status_counts),
        "country_counts":dict(Counter(x["country"] or "(unknown)" for x in feeds)),
      },
      "feeds":feeds,
      "excluded_schedule_rows":excluded,
    }
    Path(args.out).write_text(json.dumps(out,indent=2,ensure_ascii=False)+"\n",encoding="utf-8")
    print(json.dumps(out["selection"],indent=2,ensure_ascii=False))


if __name__=="__main__":
    main()
