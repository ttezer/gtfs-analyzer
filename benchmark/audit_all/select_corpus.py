#!/usr/bin/env python3
import argparse,csv,hashlib,json,re,urllib.request
from collections import Counter
from pathlib import Path

CATALOG_URL="https://files.mobilitydatabase.org/feeds_v2.csv"

def norm(s):
    return re.sub(r'[^a-z0-9]+','',s.lower())

def pick_field(fields, exact=(), contains=()):
    nf={norm(f):f for f in fields}
    for e in exact:
        if norm(e) in nf: return nf[norm(e)]
    for f in fields:
        n=norm(f)
        if all(norm(x) in n for x in contains): return f
    return None

def clean_id(v):
    v=(v or '').strip()
    if v.isdigit(): return f"mdb-{v}"
    return v

def main():
    ap=argparse.ArgumentParser()
    ap.add_argument("--out",required=True)
    ap.add_argument("--catalog-out")
    args=ap.parse_args()

    raw=urllib.request.urlopen(CATALOG_URL,timeout=120).read()
    if args.catalog_out: Path(args.catalog_out).write_bytes(raw)
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
    if missing: raise SystemExit(f"Cannot identify required catalog fields {missing}; headers={fields}")

    feeds=[]; seen_url=set(); type_counts=Counter(); status_counts=Counter()
    for row in rd:
        typ=(row.get(fmap["type"]) or "").strip().lower()
        type_counts[typ or "(blank)"]+=1
        # MobilityDatabase uses data_type=gtfs for static GTFS Schedule.
        # GTFS Realtime records (e.g. gtfs_rt) are deliberately excluded here.
        if typ!="gtfs": continue
        status=(row.get(fmap["status"]) or "").strip().lower() if fmap["status"] else ""
        status_counts[status or "(blank)"]+=1
        if status and status!="active": continue
        url=(row.get(fmap["latest"]) or "").strip()
        if not url.startswith(("http://","https://")): continue
        if url in seen_url: continue
        fid=clean_id(row.get(fmap["id"]))
        if not fid: continue
        seen_url.add(url)
        feeds.append({
          "feed_id":fid,
          "provider":(row.get(fmap["provider"]) or "").strip() if fmap["provider"] else "",
          "country":(row.get(fmap["country"]) or "").strip() if fmap["country"] else "",
          "features":(row.get(fmap["features"]) or "").strip() if fmap["features"] else "",
          "official":(row.get(fmap["official"]) or "").strip() if fmap["official"] else "",
          "url":url,
          "mobilitydatabase_page":f"https://mobilitydatabase.org/feeds/gtfs/{fid}",
          "stored_md_report_url":url.rsplit("/",1)[0]+"/report_8.0.1.json",
        })

    feeds.sort(key=lambda x:(x["feed_id"],x["url"]))
    for i,x in enumerate(feeds): x["corpus_index"]=i
    out={
      "schema_version":2,
      "selection":{
        "catalog_url":CATALOG_URL,
        "catalog_sha256":hashlib.sha256(raw).hexdigest(),
        "eligible_count":len(feeds),
        "selected_count":len(feeds),
        "filter":"data_type=gtfs only (GTFS Schedule); status active or blank; public latest URL; URL deduplicated; GTFS Realtime excluded",
        "method":"exhaustive catalog selection; no sampling",
        "field_map":fmap,
        "catalog_data_type_counts":dict(type_counts),
        "schedule_status_counts":dict(status_counts),
        "country_counts":dict(Counter(x["country"] or "(unknown)" for x in feeds)),
      },
      "feeds":feeds,
    }
    Path(args.out).write_text(json.dumps(out,indent=2,ensure_ascii=False)+"\n",encoding="utf-8")
    print(json.dumps(out["selection"],indent=2,ensure_ascii=False))

if __name__=="__main__": main()
