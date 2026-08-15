#!/usr/bin/env python3
import argparse,json,re,subprocess,tempfile,zipfile
from pathlib import Path

MAX_FEEDS=100
MAX_DOWNLOAD=1_200_000_000

def download(url,dest):
    p=subprocess.run(["curl","--fail","--location","--silent","--show-error","--retry","2","--retry-all-errors","--connect-timeout","20","--max-time","300","--max-filesize",str(MAX_DOWNLOAD),url,"-o",str(dest)],stdout=subprocess.PIPE,stderr=subprocess.PIPE)
    return p.returncode,p.stderr.decode("utf-8","replace")[:2000]

def refs_from_obj(obj):
    refs=[]
    def walk(x):
        if isinstance(x,dict):
            fn=None; line=None
            for k,v in x.items():
                nk=re.sub(r'[^a-z0-9]','',k.lower())
                if nk in ("file","filename","csvfile","tablename") and isinstance(v,str) and v.endswith(".txt"):
                    fn=v
                if nk in ("line","linenumber","csvrownumber","rownumber","row") and isinstance(v,(int,float,str)):
                    try: line=int(v)
                    except: pass
            if fn:refs.append((fn,line))
            for v in x.values():walk(v)
        elif isinstance(x,list):
            for v in x:walk(v)
    walk(obj)
    out=[];seen=set()
    for x in refs:
        if x not in seen:seen.add(x);out.append(x)
    return out[:12]

def extract_context(zf,filename,line):
    names=zf.namelist()
    target=filename if filename in names else next((n for n in names if n.rsplit("/",1)[-1]==filename),None)
    if not target:return {"file":filename,"error":"not found"}
    try:
        with zf.open(target) as f:
            if line is None:
                arr=[]
                for i,b in enumerate(f,1):
                    arr.append({"line":i,"text":b.decode("utf-8-sig","replace").rstrip("\r\n")[:1000]})
                    if i>=6:break
                return {"file":target,"requested_line":None,"context":arr}
            lo=max(1,line-2); hi=line+2; arr=[]
            for i,b in enumerate(f,1):
                if i<lo:continue
                if i>hi:break
                arr.append({"line":i,"text":b.decode("utf-8-sig","replace").rstrip("\r\n")[:1500]})
            return {"file":target,"requested_line":line,"context":arr}
    except Exception as e:
        return {"file":target,"requested_line":line,"error":repr(e)}

def main():
    ap=argparse.ArgumentParser()
    ap.add_argument("--reps",required=True)
    ap.add_argument("--out",required=True)
    ap.add_argument("--max",type=int,default=MAX_FEEDS)
    args=ap.parse_args()
    reps=json.loads(Path(args.reps).read_text())
    selected=[r for r in reps if int(r.get("priority",0))>=80][:args.max]
    out=[]
    with tempfile.TemporaryDirectory() as td:
        td=Path(td)
        for i,r in enumerate(selected,1):
            fid=r.get("feed_id","unknown")
            print(f"[evidence] {i}/{len(selected)} {fid} {r.get('type')}",flush=True)
            z=td/"feed.zip"
            if z.exists():z.unlink()
            rc,err=download(r["url"],z)
            item={"candidate":r,"download_exit":rc}
            if rc!=0:
                item["download_error"]=err;out.append(item);continue
            try:
                with zipfile.ZipFile(z) as zf:
                    refs=refs_from_obj({"analyzer_sample":r.get("analyzer_sample"),"analyzer_samples":r.get("analyzer_samples"),"md_samples":r.get("md_samples")})
                    item["archive_files"]=zf.namelist()[:100]
                    item["refs"]=refs
                    item["contexts"]=[extract_context(zf,*x) for x in refs]
            except Exception as e:
                item["zip_error"]=repr(e)
            out.append(item)
    Path(args.out).write_text(json.dumps(out,indent=2,ensure_ascii=False)+"\n")

if __name__=="__main__":
    main()
