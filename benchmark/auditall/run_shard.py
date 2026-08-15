#!/usr/bin/env python3
"""Run one deterministic shard of the 1000-feed rerun."""
from __future__ import annotations
import argparse, collections, hashlib, json, os, re, shutil, subprocess, tempfile, time, zipfile
from pathlib import Path
try: import ijson
except Exception: ijson=None

ANALYZER_TIMEOUT=300; MD_TIMEOUT=420; DOWNLOAD_TIMEOUT=300; MAX_DOWNLOAD=1_200_000_000
BENCH_DATE="20260809"; MD_DATE="2026-08-09"; MD_VERSION="8.0.1"
ANALYZER_CONFIG="ValidatorConfig::default()"

def slurp(p,n=6000):
    p=Path(p); return p.read_text(errors="replace")[:n] if p.exists() else ""

def elapsed_seconds(v):
    if not v:return None
    try:
        p=str(v).strip().split(":")
        return float(p[0]) if len(p)==1 else float(p[0])*60+float(p[1]) if len(p)==2 else float(p[0])*3600+float(p[1])*60+float(p[2]) if len(p)==3 else None
    except (TypeError,ValueError): return None

def timing(p):
    p=Path(p)
    if not p.exists(): return {}
    s=p.read_text(errors="replace")
    def g(x):
        m=re.search(x,s,re.M); return m.group(1).strip() if m else None
    e=g(r"^\s*Elapsed \(wall clock\) time.*\):\s*([0-9:.]+)\s*$")
    u=g(r"^\s*User time \(seconds\):\s*([0-9.]+)\s*$"); y=g(r"^\s*System time \(seconds\):\s*([0-9.]+)\s*$")
    r=g(r"^\s*Maximum resident set size \(kbytes\):\s*(\d+)\s*$"); c=g(r"^\s*Percent of CPU this job got:\s*(.+?)\s*$")
    return {"elapsed_raw":e,"elapsed_seconds":elapsed_seconds(e),"user_seconds":float(u) if u else None,"system_seconds":float(y) if y else None,"max_rss_kb":int(r) if r else None,"cpu":c}

def run_timed(cmd,timefile,stdout,stderr,secs):
    full=["/usr/bin/time","-v","-o",str(timefile),"timeout","--signal=TERM","--kill-after=10s",f"{secs}s",*cmd]
    with open(stdout,"wb") as o,open(stderr,"wb") as e: return subprocess.run(full,stdout=o,stderr=e).returncode

def download(url,dest,max_time=DOWNLOAD_TIMEOUT,max_size=MAX_DOWNLOAD):
    cmd=["curl","--fail","--location","--silent","--show-error","--retry","2","--retry-delay","2","--retry-all-errors","--connect-timeout","20","--max-time",str(max_time),"--max-filesize",str(max_size),"--write-out","%{url_effective}",url,"-o",str(dest)]
    p=subprocess.run(cmd,stdout=subprocess.PIPE,stderr=subprocess.PIPE)
    return p.returncode,p.stderr.decode("utf-8","replace")[:3000],p.stdout.decode("utf-8","replace").strip()

def sha256_file(p):
    h=hashlib.sha256()
    with Path(p).open("rb") as f:
        for b in iter(lambda:f.read(8*1024*1024),b""): h.update(b)
    return h.hexdigest()

def small_notice(n):
    keys=("id","rule_id","severity","rule_class","authority_source","entity_type","entity_id","scope_key","file","line","field","observed_value","expected_value","details","message","remediation")
    return {k:n.get(k) for k in keys if n.get(k) not in (None,{},[])}

def _one(p,key):
    with p.open("rb") as f:return next(ijson.items(f,key),None)
def _list(p,key):
    with p.open("rb") as f:return list(ijson.items(f,key))

def parse_analyzer(report):
    p=Path(report); out={"report_bytes":p.stat().st_size}; br=collections.Counter(); bc=collections.Counter(); bs=collections.Counter(); samples={}
    try:
        if ijson:
            out["status"]=_one(p,"status"); out["validation_status"]=_one(p,"validation_status")
            if out["status"]=="fatal":
                out.update({"fatal":{"code":_one(p,"code"),"message":_one(p,"message")},"partial":None,"notice_count":0,"by_rule":{},"by_class":{},"by_severity":{},"samples":{},"capped_totals":{}}); return out
            partial={k:_list(p,f"partial.{k}.item") for k in ("root_structural_errors","unavailable_files","skipped_stages","skipped_checks")}
            out["partial"]=partial if any(partial.values()) or out["validation_status"]=="PARTIAL" else None
            with p.open("rb") as f:
                for n in ijson.items(f,"notices.item"):
                    rid=n.get("rule_id") or "(none)"; br[rid]+=1; bc[n.get("rule_class") or "(none)"]+=1; bs[n.get("severity") or "(none)"]+=1; samples.setdefault(rid,small_notice(n))
            cap={}
            with p.open("rb") as f:
                for k,v in ijson.kvitems(f,"capped_totals"):
                    try: cap[k]=int(v)
                    except (TypeError,ValueError):cap[k]=v
            out["capped_totals"]=cap
        else:
            d=json.loads(p.read_text()); out["status"]=d.get("status"); out["validation_status"]=d.get("validation_status")
            if out["status"]=="fatal":
                out.update({"fatal":{"code":d.get("code"),"message":d.get("message")},"partial":None,"notice_count":0,"by_rule":{},"by_class":{},"by_severity":{},"samples":{},"capped_totals":{}}); return out
            out["partial"]=d.get("partial"); out["capped_totals"]=d.get("capped_totals") or {}
            for n in d.get("notices",[]):
                rid=n.get("rule_id") or "(none)"; br[rid]+=1; bc[n.get("rule_class") or "(none)"]+=1; bs[n.get("severity") or "(none)"]+=1; samples.setdefault(rid,small_notice(n))
    except Exception as e: out["parse_error"]=repr(e)
    out.update({"notice_count":sum(br.values()),"by_rule":dict(br),"by_class":dict(bc),"by_severity":dict(bs),"samples":samples}); return out

def classify_analyzer(rc,a,exists,stderr):
    if a.get("status")=="fatal":return "FATAL"
    if a.get("status")=="partial" and a.get("validation_status")=="PARTIAL":return "PARTIAL"
    if a.get("status")=="ok" and a.get("validation_status")=="COMPLETE":return "COMPLETE"
    if exists:return "REPORT_PARSE_ERROR" if a.get("parse_error") else "REPORT_STATE_INVALID"
    low=stderr.lower()
    if rc in (124,137) or "timed out" in low:return "TIMEOUT"
    if "out of memory" in low or "cannot allocate memory" in low:return "OOM"
    return "NO_REPORT"

def parse_md(report):
    p=Path(report); d=json.loads(p.read_text()); s=d.get("summary",{}); groups=[{"code":x.get("code"),"severity":x.get("severity"),"totalNotices":x.get("totalNotices",0),"sampleNotices":(x.get("sampleNotices") or [])[:2]} for x in d.get("notices",[])]
    return {"report_bytes":p.stat().st_size,"validator_version":s.get("validatorVersion"),"validation_time_seconds":s.get("validationTimeSeconds"),"max_memory_bytes":s.get("maxMemory"),"notice_groups":len(groups),"notice_total":sum(int(x.get("totalNotices") or 0) for x in groups),"groups":groups,"summary_counts":{k:v for k,v in s.items() if isinstance(v,(int,float,str,bool,type(None)))}}

def classify_md(rc,report,sys_errors,stderr):
    low=stderr.lower(); hs=bool(sys_errors and sys_errors.get("notices")); oom=any(x in low for x in ("outofmemoryerror","java heap space","out of memory"))
    if report.exists(): return "PARTIAL_OOM" if oom else "PARTIAL_INTERNAL" if hs else "PARTIAL_TIMEOUT" if rc in (124,137) else "COMPLETE"
    return "TIMEOUT" if rc in (124,137) or "timed out" in low else "OOM" if oom else "INTERNAL_ERROR" if hs else "NO_REPORT"

def input_classification(feed,status,sha):
    if status!="ok":return "DOWNLOAD_FAILED"
    return "SAME_INPUT" if feed.get("baseline_download_sha256") and sha==feed["baseline_download_sha256"] else "INPUT_DRIFT"

def provenance(analyzer_sha,benchmark_sha):
    return {"analyzer_commit_sha":analyzer_sha,"benchmark_commit_sha":benchmark_sha,"analyzer_config":ANALYZER_CONFIG,"analyzer_validation_date":BENCH_DATE,"mobilitydata_version":MD_VERSION,"mobilitydata_validation_date":MD_DATE}

def run_one(feed,root,analyzer,mdjar,analyzer_sha,benchmark_sha):
    work=root/feed["feed_id"].replace("/","_"); shutil.rmtree(work,ignore_errors=True); (work/"analyzer").mkdir(parents=True); (work/"md").mkdir()
    r={"feed":feed,"started_at_epoch":time.time(),"provenance":provenance(analyzer_sha,benchmark_sha)}; z=work/"feed.zip"
    rc,err,eff=download(feed["url"],z)
    if rc or not z.exists():
        r.update({"download":{"status":"failed","curl_exit":rc,"stderr":err,"effective_url":eff,"baseline_sha256":feed.get("baseline_download_sha256")},"input_classification":"DOWNLOAD_FAILED","finished_at_epoch":time.time()}); shutil.rmtree(work,ignore_errors=True); return r
    size=z.stat().st_size; sha=sha256_file(z); iszip=zipfile.is_zipfile(z); ds="ok" if iszip else "not_zip"
    r["download"]={"status":ds,"bytes":size,"sha256":sha,"baseline_sha256":feed.get("baseline_download_sha256"),"is_zip":iszip,"effective_url":eff}; r["input_classification"]=input_classification(feed,ds,sha)
    if size>MAX_DOWNLOAD or not iszip:
        if size>MAX_DOWNLOAD:r["download"]["status"]="too_large"; r["input_classification"]="DOWNLOAD_FAILED"
        r["finished_at_epoch"]=time.time(); shutil.rmtree(work,ignore_errors=True); return r
    stored=work/"stored-md.json"; surl=(eff.rsplit("/",1)[0]+"/report_8.0.1.json") if eff else feed.get("stored_md_report_url","")
    if surl:
        x,_,_=download(surl,stored,45,20_000_000)
        try:r["mobilitydata_stored"]={**parse_md(stored),"status":"available"} if x==0 else {"status":"unavailable","curl_exit":x}
        except Exception as e:r["mobilitydata_stored"]={"status":"parse_error","error":repr(e)}
    ar=work/"analyzer/report.json"; ae=run_timed([str(analyzer),"validate",str(z),"--json","--lang","en","--today",BENCH_DATE,"--output",str(ar)],work/"analyzer/time.txt",work/"analyzer/stdout.txt",work/"analyzer/stderr.txt",ANALYZER_TIMEOUT); ase=slurp(work/"analyzer/stderr.txt"); a=parse_analyzer(ar) if ar.exists() else {}
    a={"exit_code":ae,"timing":timing(work/"analyzer/time.txt"),"stderr_head":ase,**a}; a["state"]=classify_analyzer(ae,a,ar.exists(),ase); a.update({"commit_sha":analyzer_sha,"config":ANALYZER_CONFIG,"validation_date":BENCH_DATE}); r["analyzer"]=a
    me=run_timed(["java","-Xmx12g","-jar",str(mdjar),"-i",str(z),"-o",str(work/"md"),"-d",MD_DATE],work/"md/time.txt",work/"md/stdout.txt",work/"md/stderr.txt",MD_TIMEOUT); mse=slurp(work/"md/stderr.txt"); mr=work/"md/report.json"; sp=work/"md/system_errors.json"; se=None
    if sp.exists():
        try:se=json.loads(sp.read_text())
        except Exception as e:se={"parse_error":repr(e)}
    m={"exit_code":me,"timing":timing(work/"md/time.txt"),"stderr_head":mse,"system_errors":se,"validator_version_expected":MD_VERSION,"validation_date":MD_DATE}
    if mr.exists():
        try:m.update(parse_md(mr))
        except Exception as e:m["parse_error"]=repr(e)
    m["state"]=classify_md(me,mr,se,mse); r["mobilitydata"]=m; r["finished_at_epoch"]=time.time(); shutil.rmtree(work,ignore_errors=True); return r

def _fixture(path,routes=True):
    fs={"agency.txt":"agency_id,agency_name,agency_url,agency_timezone\n1,Test,http://test.example,UTC\n","stops.txt":"stop_id,stop_name,stop_lat,stop_lon\nS1,Stop1,41,29\nS2,Stop2,41.1,29.1\n","routes.txt":"route_id,agency_id,route_short_name,route_type\nR1,1,101,3\n","trips.txt":"route_id,service_id,trip_id\nR1,SVC1,T1\n","stop_times.txt":"trip_id,arrival_time,departure_time,stop_id,stop_sequence\nT1,08:00:00,08:00:00,S1,1\nT1,08:10:00,08:10:00,S2,2\n","calendar.txt":"service_id,monday,tuesday,wednesday,thursday,friday,saturday,sunday,start_date,end_date\nSVC1,1,1,1,1,1,0,0,20250101,20271231\n"}
    if not routes:fs.pop("routes.txt")
    with zipfile.ZipFile(path,"w",zipfile.ZIP_DEFLATED) as zf:
        for n,b in fs.items():zf.writestr(n,b)

def parser_preflight(analyzer):
    with tempfile.TemporaryDirectory() as t:
        t=Path(t); c=t/"complete.zip"; p=t/"partial.zip"; f=t/"fatal.zip"; _fixture(c); _fixture(p,False); f.write_bytes(b"not a zip")
        out=[]
        for name,feed,want in (("COMPLETE",c,"COMPLETE"),("PARTIAL",p,"PARTIAL"),("FATAL",f,"FATAL")):
            report=t/f"{name}.json"; x=subprocess.run([str(Path(analyzer).resolve()),"validate",str(feed),"--json","--lang","en","--today",BENCH_DATE,"--output",str(report)],stdout=subprocess.PIPE,stderr=subprocess.PIPE); a=parse_analyzer(report) if report.exists() else {}; state=classify_analyzer(x.returncode,a,report.exists(),x.stderr.decode(errors="replace")); out.append({"case":name,"exit_code":x.returncode,"state":state,"validation_status":a.get("validation_status")})
            if state!=want:raise SystemExit(f"parser preflight {name}: expected {want}, got {state}: {a}")
        print(json.dumps({"parser_preflight":"PASS","cases":out},indent=2))

def main():
    ap=argparse.ArgumentParser(); ap.add_argument("--manifest"); ap.add_argument("--shard",type=int); ap.add_argument("--shards",type=int); ap.add_argument("--analyzer",required=True); ap.add_argument("--md-jar"); ap.add_argument("--out"); ap.add_argument("--analyzer-sha",default=os.getenv("ANALYZER_SHA","unknown")); ap.add_argument("--benchmark-sha",default=os.getenv("GITHUB_SHA","unknown")); ap.add_argument("--limit",type=int); ap.add_argument("--parser-preflight",action="store_true"); a=ap.parse_args()
    if a.parser_preflight:return parser_preflight(a.analyzer)
    if any(x is None for x in (a.manifest,a.shard,a.shards,a.md_jar,a.out)):raise SystemExit("manifest/shard/shards/md-jar/out required")
    d=json.loads(Path(a.manifest).read_text()); feeds=[f for f in d["feeds"] if int(f["corpus_index"])%a.shards==a.shard]; feeds=feeds[:a.limit] if a.limit is not None else feeds; root=Path(os.getenv("RUNNER_TEMP","/tmp"))/f"gtfs-audit-{a.shard:03d}"; root.mkdir(parents=True,exist_ok=True); out=Path(a.out); out.parent.mkdir(parents=True,exist_ok=True)
    with out.open("w") as h:
        for i,feed in enumerate(feeds,1):
            print(f"[shard {a.shard}/{a.shards}] {i}/{len(feeds)} {feed['feed_id']}",flush=True)
            try:r=run_one(feed,root,Path(a.analyzer).resolve(),Path(a.md_jar).resolve(),a.analyzer_sha,a.benchmark_sha)
            except Exception as e:
                work=root/feed["feed_id"].replace("/","_"); z=work/"feed.zip"; dl={"status":"failed","baseline_sha256":feed.get("baseline_download_sha256")}; cls="DOWNLOAD_FAILED"
                if z.exists():
                    try:sha=sha256_file(z); iz=zipfile.is_zipfile(z); st="ok" if iz else "not_zip"; dl.update({"status":st,"bytes":z.stat().st_size,"sha256":sha,"is_zip":iz}); cls=input_classification(feed,st,sha)
                    except Exception as q:dl["identity_error"]=repr(q)
                r={"feed":feed,"download":dl,"runner_exception":repr(e),"finished_at_epoch":time.time(),"input_classification":cls,"provenance":provenance(a.analyzer_sha,a.benchmark_sha)}; shutil.rmtree(work,ignore_errors=True)
            h.write(json.dumps(r,ensure_ascii=False,separators=(",",":"))+"\n"); h.flush()
    shutil.rmtree(root,ignore_errors=True)
if __name__=="__main__":main()
