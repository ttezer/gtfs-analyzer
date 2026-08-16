#!/usr/bin/env python3
import argparse,collections,hashlib,json,os,re,shutil,subprocess,time,zipfile
from pathlib import Path

try:
    import ijson
except Exception:
    ijson=None

ANALYZER_TIMEOUT=300
MD_TIMEOUT=420
DOWNLOAD_TIMEOUT=300
MAX_DOWNLOAD=1_200_000_000
BENCH_DATE="20260809"
MD_DATE="2026-08-09"

def slurp(p,limit=6000):
    p=Path(p)
    if not p.exists(): return ""
    s=p.read_text(errors="replace")
    return s[:limit]

def timing(p):
    p=Path(p)
    if not p.exists(): return {}
    s=p.read_text(errors="replace")
    def g(pat):
        m=re.search(pat,s,re.M)
        return m.group(1).strip() if m else None
    return {
      "elapsed":g(r"Elapsed \(wall clock\) time.*?:\s*(.+)$"),
      "user_s":g(r"User time \(seconds\):\s*(.+)$"),
      "system_s":g(r"System time \(seconds\):\s*(.+)$"),
      "max_rss_kb":int(g(r"Maximum resident set size \(kbytes\):\s*(\d+)") or 0),
      "cpu":g(r"Percent of CPU this job got:\s*(.+)$"),
    }

def run_timed(cmd,timefile,stdout,stderr,timeout_s,cwd=None):
    full=["/usr/bin/time","-v","-o",str(timefile),"timeout","--signal=TERM","--kill-after=10s",f"{timeout_s}s",*cmd]
    with open(stdout,"wb") as out, open(stderr,"wb") as err:
        p=subprocess.run(full,stdout=out,stderr=err,cwd=cwd)
    return p.returncode

def download(url,dest,max_time=DOWNLOAD_TIMEOUT,max_size=MAX_DOWNLOAD):
    cmd=["curl","--fail","--location","--silent","--show-error",
         "--retry","2","--retry-delay","2","--retry-all-errors",
         "--connect-timeout","20","--max-time",str(max_time),
         "--max-filesize",str(max_size),url,"-o",str(dest)]
    cmd[1:1]=["--write-out","%{url_effective}"]
    p=subprocess.run(cmd,stdout=subprocess.PIPE,stderr=subprocess.PIPE)
    return p.returncode,p.stderr.decode("utf-8","replace")[:3000],p.stdout.decode("utf-8","replace").strip()

def small_notice(n):
    keep=("id","rule_id","severity","rule_class","entity_type","entity_id","scope_key",
          "file","line","field","observed_value","expected_value","details","message","remediation")
    return {k:n.get(k) for k in keep if n.get(k) not in (None,{},[])}

def parse_analyzer(report):
    report=Path(report)
    out={"report_bytes":report.stat().st_size}
    by_rule=collections.Counter(); by_class=collections.Counter(); by_sev=collections.Counter()
    samples={}
    status=None
    if ijson is not None:
        try:
            with report.open("rb") as f:
                vals=ijson.items(f,"status")
                status=next(vals,None)
            with report.open("rb") as f:
                for n in ijson.items(f,"notices.item"):
                    rid=n.get("rule_id") or "(none)"
                    by_rule[rid]+=1
                    by_class[n.get("rule_class") or "(none)"]+=1
                    by_sev[n.get("severity") or "(none)"]+=1
                    if rid not in samples:
                        samples[rid]=small_notice(n)
        except Exception as e:
            out["parse_error"]=repr(e)
    else:
        try:
            d=json.loads(report.read_text())
            status=d.get("status")
            for n in d.get("notices",[]):
                rid=n.get("rule_id") or "(none)"
                by_rule[rid]+=1
                by_class[n.get("rule_class") or "(none)"]+=1
                by_sev[n.get("severity") or "(none)"]+=1
                if rid not in samples:
                    samples[rid]=small_notice(n)
        except Exception as e:
            out["parse_error"]=repr(e)
    out.update({
      "status":status,
      "notice_count":sum(by_rule.values()),
      "by_rule":dict(by_rule),
      "by_class":dict(by_class),
      "by_severity":dict(by_sev),
      "samples":samples,
    })
    return out

def parse_md_report(report):
    report=Path(report)
    d=json.loads(report.read_text())
    s=d.get("summary",{})
    groups=[]
    for x in d.get("notices",[]):
        groups.append({
          "code":x.get("code"),
          "severity":x.get("severity"),
          "totalNotices":x.get("totalNotices",0),
          "sampleNotices":(x.get("sampleNotices") or [])[:2],
        })
    return {
      "report_bytes":report.stat().st_size,
      "validator_version":s.get("validatorVersion"),
      "validation_time_seconds":s.get("validationTimeSeconds"),
      "max_memory_bytes":s.get("maxMemory"),
      "notice_groups":len(groups),
      "notice_total":sum(int(x.get("totalNotices") or 0) for x in groups),
      "groups":groups,
      "summary_counts":{k:v for k,v in s.items() if isinstance(v,(int,float,str,bool,type(None)))},
    }

def classify_analyzer(exit_code,report,stderr):
    low=stderr.lower()
    if report.exists():
        return "completed"
    if exit_code in (124,137) or "timed out" in low:
        return "timeout"
    if "out of memory" in low or "cannot allocate memory" in low:
        return "oom"
    if "fatal" in low or "zip" in low or "csv" in low:
        return "fatal_or_input_error"
    return "no_report"

def classify_md(exit_code,report,sys_errors,stderr):
    low=stderr.lower()
    has_sys=bool(sys_errors and sys_errors.get("notices"))
    oom="outofmemoryerror" in low or "java heap space" in low or "out of memory" in low
    if report.exists():
        if oom: return "partial_oom"
        if has_sys: return "partial_internal"
        if exit_code in (124,137): return "partial_timeout"
        return "completed"
    if exit_code in (124,137) or "timed out" in low: return "timeout"
    if oom: return "oom"
    if has_sys: return "internal_error"
    return "no_report"

def run_one(feed,root,analyzer,mdjar):
    fid=feed["feed_id"]
    work=root/fid.replace("/","_")
    shutil.rmtree(work,ignore_errors=True)
    (work/"analyzer").mkdir(parents=True)
    (work/"md").mkdir(parents=True)
    result={"feed":feed,"started_at_epoch":time.time()}
    z=work/"feed.zip"

    rc,err,effective=download(feed["url"],z)
    if rc!=0:
        result["download"]={"status":"failed","curl_exit":rc,"stderr":err,"effective_url":effective}
        result["finished_at_epoch"]=time.time()
        shutil.rmtree(work,ignore_errors=True)
        return result
    b=z.stat().st_size
    if b < 100_000_000:
        sha=hashlib.sha256(z.read_bytes()).hexdigest()
    else:
        h=hashlib.sha256()
        with z.open("rb") as f:
            for chunk in iter(lambda:f.read(8*1024*1024),b""): h.update(chunk)
        sha=h.hexdigest()
    result["download"]={"status":"ok","bytes":b,"sha256":sha,"is_zip":zipfile.is_zipfile(z),"effective_url":effective}
    if not result["download"]["is_zip"]:
        result["download"]["status"]="not_zip"
        result["finished_at_epoch"]=time.time()
        shutil.rmtree(work,ignore_errors=True)
        return result

    stored=work/"stored-md.json"
    stored_url=(effective.rsplit("/",1)[0]+"/report_8.0.1.json") if effective else feed["stored_md_report_url"]
    src,se,_=download(stored_url,stored,max_time=45,max_size=20_000_000)
    if src==0:
        try:
            result["mobilitydata_stored"]=parse_md_report(stored)
            result["mobilitydata_stored"]["status"]="available"
        except Exception as e:
            result["mobilitydata_stored"]={"status":"parse_error","error":repr(e)}
    else:
        result["mobilitydata_stored"]={"status":"unavailable","curl_exit":src}

    ar=work/"analyzer"/"report.json"
    aexit=run_timed(
      [str(analyzer),"validate",str(z),"--json","--lang","en","--today",BENCH_DATE,"--output",str(ar)],
      work/"analyzer"/"time.txt",work/"analyzer"/"stdout.txt",work/"analyzer"/"stderr.txt",ANALYZER_TIMEOUT)
    astderr=slurp(work/"analyzer"/"stderr.txt")
    a={"exit_code":aexit,"timing":timing(work/"analyzer"/"time.txt"),"stderr_head":astderr}
    if ar.exists():
        try:a.update(parse_analyzer(ar))
        except Exception as e:a["parse_error"]=repr(e)
    a["state"]=classify_analyzer(aexit,ar,astderr)
    result["analyzer"]=a

    mexit=run_timed(
      ["java","-Xmx12g","-jar",str(mdjar),"-i",str(z),"-o",str(work/"md"),"-d",MD_DATE],
      work/"md"/"time.txt",work/"md"/"stdout.txt",work/"md"/"stderr.txt",MD_TIMEOUT)
    mstderr=slurp(work/"md"/"stderr.txt")
    mr=work/"md"/"report.json"
    sysp=work/"md"/"system_errors.json"
    sys_errors=None
    if sysp.exists():
        try: sys_errors=json.loads(sysp.read_text())
        except Exception as e: sys_errors={"parse_error":repr(e)}
    m={"exit_code":mexit,"timing":timing(work/"md"/"time.txt"),"stderr_head":mstderr,"system_errors":sys_errors}
    if mr.exists():
        try:m.update(parse_md_report(mr))
        except Exception as e:m["parse_error"]=repr(e)
    m["state"]=classify_md(mexit,mr,sys_errors,mstderr)
    result["mobilitydata"]=m

    result["finished_at_epoch"]=time.time()
    shutil.rmtree(work,ignore_errors=True)
    return result

def main():
    ap=argparse.ArgumentParser()
    ap.add_argument("--manifest",required=True)
    ap.add_argument("--shard",type=int,required=True)
    ap.add_argument("--shards",type=int,required=True)
    ap.add_argument("--analyzer",required=True)
    ap.add_argument("--md-jar",required=True)
    ap.add_argument("--out",required=True)
    args=ap.parse_args()
    manifest=json.loads(Path(args.manifest).read_text())
    feeds=[f for f in manifest["feeds"] if int(f["corpus_index"]) % args.shards == args.shard]
    root=Path(os.environ.get("RUNNER_TEMP","/tmp"))/f"gtfs-audit-{args.shard:03d}"
    root.mkdir(parents=True,exist_ok=True)
    analyzer=Path(args.analyzer).resolve(); mdjar=Path(args.md_jar).resolve()
    outp=Path(args.out); outp.parent.mkdir(parents=True,exist_ok=True)
    with outp.open("w",encoding="utf-8") as fh:
        for i,feed in enumerate(feeds,1):
            print(f"[shard {args.shard}/{args.shards}] {i}/{len(feeds)} {feed['feed_id']} {feed.get('provider','')}",flush=True)
            try:
                r=run_one(feed,root,analyzer,mdjar)
            except Exception as e:
                r={"feed":feed,"runner_exception":repr(e),"finished_at_epoch":time.time()}
            fh.write(json.dumps(r,ensure_ascii=False,separators=(",",":"))+"\n")
            fh.flush()
    shutil.rmtree(root,ignore_errors=True)

if __name__=="__main__":
    main()
