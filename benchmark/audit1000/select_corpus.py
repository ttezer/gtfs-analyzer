#!/usr/bin/env python3
import argparse,csv,hashlib,json,re,urllib.request
from collections import Counter

CATALOG_URL="https://files.mobilitydatabase.org/feeds_v2.csv"

def norm(s):
    return re.sub(r'[^a-z0-9]+','',s.lower())

def pick_field(fields, exact=(), contains=()):
    nf={norm(f):f for f in fields}
    for e in exact:
        if norm(e) in nf: return nf[norm(e)]
    for f in fields:
        n=norm(f)
        if all(norm(x) in n for x in contains):
            return f
    return None

def clean_id(v):
    v=(v or '').strip()
    if v.isdigit(): return f"mdb-{v}"
    return v

def main():
    ap=argparse.ArgumentParser()
    ap.add_argument("--count",type=int,default=1000)
    ap.add_argument("--seed",default="gtfs-analyzer-audit-1000-v1")
    ap.add_argument("--out",required=True)
    ap.add_argument("--catalog-out")
    args=ap.parse_args()

    raw=urllib.request.urlopen(CATALOG_URL,timeout=120).read()
    if args.catalog_out:
        Path(args.catalog_out).write_bytes(raw)
    text=raw.decode("utf-8-sig")
    rd=csv.DictReader(text.splitlines())
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

    candidates=[]
    seen_url=set()
    for row in rd:
        typ=(row.get(fmap["type"]) or "").strip().lower()
        if typ!="gtfs": continue
        status=(row.get(fmap["status"]) or "").strip().lower() if fmap["status"] else ""
        if status and status!="active": continue
        url=(row.get(fmap["latest"]) or "").strip()
        if not url.startswith(("http://","https://")): continue
        if url in seen_url: continue
        seen_url.add(url)
        fid=clean_id(row.get(fmap["id"]))
        if not fid: continue
        provider=(row.get(fmap["provider"]) or "").strip() if fmap["provider"] else ""
        country=(row.get(fmap["country"]) or "").strip() if fmap["country"] else ""
        features=(row.get(fmap["features"]) or "").strip() if fmap["features"] else ""
        official=(row.get(fmap["official"]) or "").strip() if fmap["official"] else ""
        score=hashlib.sha256(f"{args.seed}\0{fid}\0{url}".encode()).hexdigest()
        candidates.append({
          "feed_id":fid,"provider":provider,"country":country,"features":features,
          "official":official,"url":url,"sample_hash":score,
          "mobilitydatabase_page":f"https://mobilitydatabase.org/feeds/gtfs/{fid}",
          "stored_md_report_url":url.rsplit("/",1)[0]+"/report_8.0.1.json",
        })
    candidates.sort(key=lambda x:(x["sample_hash"],x["feed_id"]))
    if len(candidates)<args.count:
        raise SystemExit(f"Only {len(candidates)} eligible active GTFS feeds, need {args.count}")
    selected=candidates[:args.count]
    for i,x in enumerate(selected):
        x["corpus_index"]=i

    out={
      "schema_version":1,
      "selection":{
        "catalog_url":CATALOG_URL,
        "catalog_sha256":hashlib.sha256(raw).hexdigest(),
        "seed":args.seed,
        "eligible_count":len(candidates),
        "selected_count":len(selected),
        "filter":"data_type=gtfs; status active or blank; public latest URL; URL deduplicated",
        "method":"sort by sha256(seed + feed_id + latest_url), take first N; deterministic/non-cherry-picked",
        "field_map":fmap,
        "country_counts":dict(Counter(x["country"] or "(unknown)" for x in selected)),
      },
      "feeds":selected,
    }
    Path(args.out).write_text(json.dumps(out,indent=2,ensure_ascii=False)+"\n")
    print(json.dumps(out["selection"],indent=2,ensure_ascii=False))

if __name__=="__main__":
    from pathlib import Path
    main()
