#!/usr/bin/env python3
"""Aggregate rerun parity plus SAME_INPUT old/new comparison."""
from __future__ import annotations
import argparse,csv,gzip,importlib.util,json,math,statistics,sys
from collections import Counter,defaultdict
from pathlib import Path
NEAR={"single_shape_point"}; MIDNIGHT={"STM_007","STM_008","STM_048","STM_049"}

def load(path):
    p=Path(path)
    if p.suffix==".gz":
        with gzip.open(p,"rt",encoding="utf-8") as f:return json.load(f)
    return json.loads(p.read_text())
def parity_mod(path):
    p=Path(path).resolve(); sys.path.insert(0,str(p.parent)); s=importlib.util.spec_from_file_location("audit_md",p); m=importlib.util.module_from_spec(s); s.loader.exec_module(m)
    return m.MAP,getattr(m,"AGG_RULES",set()),getattr(m,"resolve_mapping",None),getattr(m,"classify_unmapped",None)
def mdgroups(m):return {g.get("code"):g for g in (m or {}).get("groups",[]) if g.get("code")}
def mdcounts(m):return {k:int(v.get("totalNotices") or 0) for k,v in mdgroups(m).items()}
def oldstate(r):
    a=(r or {}).get("analyzer") or {}; s=a.get("status"); v=a.get("validation_status")
    if s=="fatal":return "FATAL"
    if s=="partial" or v=="PARTIAL":return "PARTIAL"
    if s=="ok" or v=="COMPLETE":return "COMPLETE"
    x=str(a.get("state") or "").upper(); return "COMPLETE" if x=="COMPLETED" else x or "UNKNOWN"
def pct(v,p):
    x=sorted(float(n) for n in v if n is not None)
    if not x:return None
    z=(len(x)-1)*p; lo=math.floor(z); hi=math.ceil(z); return x[lo] if lo==hi else x[lo]*(hi-z)+x[hi]*(z-lo)
def perf(v):
    x=[float(n) for n in v if n is not None]; return {"n":len(x),"median":statistics.median(x) if x else None,"p95":pct(x,.95),"maximum":max(x) if x else None}
def meta(a,r,reg=None):
    s=((a or {}).get("samples") or {}).get(r) or {}; reg=reg or {}; return {"class":s.get("rule_class") or reg.get("class"),"severity":s.get("severity") or reg.get("severity"),"sample":s}
def mdiff(a,b):return bool(a.get("class") and a.get("severity") and b.get("class") and b.get("severity") and (a["class"],a["severity"])!=(b["class"],b["severity"]))
def impact(rule,oa,na):
    if rule=="DQ_016":return "whitespace_cascade"
    if rule.startswith("TRN_"):return "translations_cascade"
    if rule in MIDNIGHT:return "midnight_normalization"
    if rule.startswith("FPD_"):return "fare_products"
    if rule.startswith("SHP_"):return "shape_rules"
    if "DQ_016" in ((oa or {}).get("by_rule") or {}) or "DQ_016" in ((na or {}).get("by_rule") or {}):return "whitespace_related_feed"

def before_after(old,new,registry):
    oldby={(r.get("feed") or {}).get("feed_id"):r for r in old}; ev=[]; trans=Counter(); groups=defaultdict(lambda:{"feeds":set(),"events":0,"delta":0,"rules":Counter()}); red=[]; imp=[]; retired=set(); n=0
    for r in new:
        if r.get("input_classification")!="SAME_INPUT":continue
        fid=(r.get("feed") or {}).get("feed_id"); o=oldby.get(fid)
        if not o:continue
        n+=1; oa=o.get("analyzer") or {}; na=r.get("analyzer") or {}; os=oldstate(o); ns=na.get("state","NOT_RUN"); trans[(os,ns)]+=1
        if os!=ns:
            e={"feed_id":fid,"type":"STATE_TRANSITION","old_state":os,"new_state":ns}; ev.append(e)
            if os=="FATAL" and ns in ("PARTIAL","COMPLETE"):e["assessment"]="IMPROVEMENT"; imp.append(e)
            elif os=="COMPLETE" and ns in ("PARTIAL","FATAL"):e["assessment"]="REGRESSION_RED"; red.append(e)
            groups["recovery"]["feeds"].add(fid);groups["recovery"]["events"]+=1
        oc={k:int(v) for k,v in (oa.get("by_rule") or {}).items()}; nc={k:int(v) for k,v in (na.get("by_rule") or {}).items()}
        for rule in sorted(set(oc)|set(nc)):
            x,y=oc.get(rule,0),nc.get(rule,0); om=meta(oa,rule); nm=meta(na,rule,registry.get(rule))
            if x==y:
                if x and mdiff(om,nm):ev.append({"feed_id":fid,"type":"RULE_METADATA_CHANGED","rule_id":rule,"old_count":x,"new_count":y,"old_class":om["class"],"new_class":nm["class"],"old_severity":om["severity"],"new_severity":nm["severity"]})
                continue
            if x and not y:
                typ="OLD_RULE_REMOVED_FROM_REGISTRY" if registry and rule not in registry else "OLD_RULE_NO_LONGER_EMITS"; retired.add(rule) if typ.endswith("REGISTRY") else None
            elif y and not x:typ="NEW_RULE_EMITS"
            else:typ="FINDING_COUNT_INCREASED" if y>x else "FINDING_COUNT_DECREASED"
            e={"feed_id":fid,"type":typ,"rule_id":rule,"old_count":x,"new_count":y,"delta":y-x,"old_class":om["class"],"new_class":nm["class"],"old_severity":om["severity"],"new_severity":nm["severity"]}
            if mdiff(om,nm):e["metadata_changed"]=True
            ev.append(e); g=impact(rule,oa,na)
            if g:groups[g]["feeds"].add(fid);groups[g]["events"]+=1;groups[g]["delta"]+=y-x;groups[g]["rules"][rule]+=y-x
    gj={k:{"feed_count":len(v["feeds"]),"feeds":sorted(v["feeds"]),"event_count":v["events"],"net_notice_delta":v["delta"],"rule_deltas":dict(v["rules"].most_common())} for k,v in groups.items()}
    return ev,{"same_input_feeds_compared":n,"state_transitions":{f"{a} -> {b}":c for (a,b),c in trans.items()},"event_counts":dict(Counter(x["type"] for x in ev)),"red_regressions":red,"improvements":imp,"retired_observed_rules":sorted(retired),"change_impact":gj}

def category(code,mr,classify):
    rules=tuple(getattr(mr,"analyzer_rules",()) or ()) if mr else ()
    if code in NEAR and rules:return "near"
    if rules:
        if getattr(mr,"kind","static") in ("context-dependent","context-mixed","fallback") or not getattr(mr,"context_complete",True) or getattr(mr,"unresolved_samples",0):return "near"
        return "exact"
    if mr and getattr(mr,"kind","")=="unresolved-context":return "near"
    d=classify(code)[0] if classify else "unreviewed"
    if d=="config-dependent":return "config-dependent"
    if d in ("genuine-gap","intentional-difference","deprecated-md-only"):return "intentional-gap"
    if d=="context-dependent":return "near"
    return "unreviewed-gap"

def parity(rows,pm):
    MAP,AGG,resolve,classify=pm; inv=defaultdict(list)
    for c,rs in MAP.items():
        for r in rs:inv[r].append(c)
    cand=[]; obs=[]; cats=Counter(); states=Counter(); ar={}; mcodes={}
    for row in rows:
        feed=row.get("feed") or {}; fid=feed.get("feed_id",""); a=row.get("analyzer") or {}; m=row.get("mobilitydata") or {}; ast=a.get("state","NOT_RUN"); mst=m.get("state","NOT_RUN"); states[(ast,mst)]+=1
        for rid,n in (a.get("by_rule") or {}).items():
            e=ar.setdefault(rid,{"rule_id":rid,"feed_count":0,"notice_total":0,"affected_feeds":[],"classes":Counter(),"severities":Counter(),"sample":None});e["feed_count"]+=1;e["notice_total"]+=int(n);e["affected_feeds"].append(fid);s=(a.get("samples") or {}).get(rid) or {};e["classes"][s.get("rule_class")]+=1 if s.get("rule_class") else 0;e["severities"][s.get("severity")]+=1 if s.get("severity") else 0;e["sample"]=e["sample"] or {"feed_id":fid,"notice":s}
        gs=mdgroups(m)
        for code,g in gs.items():
            e=mcodes.setdefault(code,{"code":code,"feed_count":0,"notice_total":0,"affected_feeds":[],"severities":Counter(),"sample":None});e["feed_count"]+=1;e["notice_total"]+=int(g.get("totalNotices") or 0);e["affected_feeds"].append(fid);e["severities"][str(g.get("severity")).upper()]+=1 if g.get("severity") else 0;e["sample"]=e["sample"] or {"feed_id":fid,"sample":(g.get("sampleNotices") or [])[:2]}
        if ast!="COMPLETE" or mst!="COMPLETE":
            if (row.get("download") or {}).get("status")=="ok" and ast!=mst:cand.append({"priority":120,"type":"validator-state-asymmetry","feed_id":fid,"url":feed.get("url",""),"input_classification":row.get("input_classification"),"analyzer_state":ast,"md_state":mst,"analyzer_fatal":a.get("fatal"),"partial":a.get("partial")})
            continue
        ac={k:int(v) for k,v in (a.get("by_rule") or {}).items()}; mc=mdcounts(m)
        for code,mn in mc.items():
            g=gs[code]; fallback=MAP.get(code,[]); mr=resolve(code,g,fallback) if resolve else None; rules=tuple(getattr(mr,"analyzer_rules",()) or fallback); cat=category(code,mr,classify);cats[cat]+=1;decision,note=classify(code) if classify and not rules else (None,None)
            base={"feed_id":fid,"input_classification":row.get("input_classification"),"md_code":code,"md_count":mn,"md_severity":g.get("severity"),"parity_category":cat,"decision":decision,"decision_note":note,"mapped_analyzer_rules":list(rules),"mapping_kind":getattr(mr,"kind","static") if mr else "unmapped","mapping_context_complete":getattr(mr,"context_complete",True) if mr else True,"mapping_unresolved_samples":getattr(mr,"unresolved_samples",0) if mr else 0}
            if not rules:
                obs.append({**base,"analyzer_mapped_count":0,"count_relation":"MD_ONLY_UNMAPPED"});cand.append({**base,"priority":92 if cat=="unreviewed-gap" else 70,"type":"md-only-candidate","url":feed.get("url",""),"md_samples":(g.get("sampleNotices") or [])[:2]});continue
            our=sum(ac.get(r,0) for r in rules);agg=any(r in AGG for r in rules);rel="MD_ONLY_MAPPED" if our==0 else "COUNT_EXACT" if our==mn else "COUNT_DIVERGENCE_AGGREGATION_EXPECTED" if agg else "COUNT_DIVERGENCE";obs.append({**base,"analyzer_mapped_count":our,"analyzer_rule_counts":{r:ac.get(r,0) for r in rules},"aggregation_expected":agg,"count_relation":rel})
            if our==0:cand.append({**base,"priority":108 if str(g.get("severity","")).upper()=="ERROR" else 90,"type":"md-only-mapped-candidate","url":feed.get("url",""),"analyzer_mapped_count":0,"md_samples":(g.get("sampleNotices") or [])[:2]})
            elif our!=mn:cand.append({**base,"priority":65 if agg else 86 if str(g.get("severity","")).upper()=="ERROR" else 78,"type":"count-divergence","url":feed.get("url",""),"analyzer_mapped_count":our,"analyzer_rule_counts":{r:ac.get(r,0) for r in rules},"aggregation_expected":agg,"md_samples":(g.get("sampleNotices") or [])[:2],"analyzer_samples":{r:(a.get("samples") or {}).get(r) for r in rules if ac.get(r)}})
        for rid,n in ac.items():
            codes=inv.get(rid,[]);s=(a.get("samples") or {}).get(rid) or {}
            if not codes:
                cl=str(s.get("rule_class","")).upper();sv=str(s.get("severity","")).upper();cand.append({"priority":96 if cl=="SPEC" and sv=="CRITICAL" else 84 if cl=="SPEC" else 48,"type":"analyzer-only-candidate","feed_id":fid,"url":feed.get("url",""),"analyzer_rule":rid,"analyzer_count":n,"analyzer_class":s.get("rule_class"),"analyzer_severity":s.get("severity"),"analyzer_sample":s})
            elif sum(mc.get(c,0) for c in codes)==0:cand.append({"priority":82,"type":"analyzer-only-mapped-candidate","feed_id":fid,"url":feed.get("url",""),"analyzer_rule":rid,"analyzer_count":n,"mapped_md_codes":codes,"analyzer_class":s.get("rule_class"),"analyzer_severity":s.get("severity"),"analyzer_sample":s})
    cand.sort(key=lambda d:(-int(d.get("priority",0)),d.get("feed_id",""),d.get("md_code",""),d.get("analyzer_rule","")))
    return {"candidates":cand,"observations":obs,"category_counts":dict(cats),"state_pairs":{f"{a} | {m}":n for (a,m),n in states.items()},"analyzer_rules":ar,"md_codes":mcodes}
def serial(items):
    out=[{**{k:v for k,v in e.items() if k not in ("classes","severities")},"classes":dict(e.get("classes",{})),"severities":dict(e.get("severities",{}))} for e in items.values()]; key="rule_id" if out and "rule_id" in out[0] else "code";out.sort(key=lambda x:(-x.get("feed_count",0),-x.get("notice_total",0),x.get(key,"")));return out
def csvwrite(path,rows,fields):
    with Path(path).open("w",newline="",encoding="utf-8") as f:
        w=csv.DictWriter(f,fieldnames=fields,extrasaction="ignore");w.writeheader()
        for r in rows:w.writerow({k:json.dumps(v,ensure_ascii=False,separators=(",",":")) if isinstance(v,(list,dict)) else v for k,v in r.items()})

def main():
    p=argparse.ArgumentParser();p.add_argument("--results-dir",required=True);p.add_argument("--map-file",required=True);p.add_argument("--baseline-results",required=True);p.add_argument("--out-dir",required=True);p.add_argument("--expected-analyzer-sha");p.add_argument("--current-rule-registry");a=p.parse_args();out=Path(a.out_dir);out.mkdir(parents=True,exist_ok=True);rows=[]
    for f in sorted(Path(a.results_dir).glob("shard-*.jsonl")):
        for line in f.read_text(errors="replace").splitlines():
            try:rows.append(json.loads(line)) if line.strip() else None
            except json.JSONDecodeError:pass
    rows.sort(key=lambda x:int((x.get("feed") or {}).get("corpus_index",10**9)));old=load(a.baseline_results)
    with gzip.open(out/"all-results.json.gz","wt",encoding="utf-8") as f:json.dump(rows,f,ensure_ascii=False,separators=(",",":"))
    if a.expected_analyzer_sha:
        bad={x for x in ((r.get("analyzer") or {}).get("commit_sha") for r in rows if r.get("analyzer")) if x and x!=a.expected_analyzer_sha}
        if bad:raise SystemExit(f"Analyzer SHA mismatch: {bad}")
    reg={}
    if a.current_rule_registry:reg={x["id"]:x for x in load(a.current_rule_registry) if isinstance(x,dict) and x.get("id")}
    pa=parity(rows,parity_mod(a.map_file));events,ba=before_after(old,rows,reg);ic=Counter(r.get("input_classification","UNKNOWN") for r in rows)
    drift=[{"corpus_index":(r.get("feed") or {}).get("corpus_index"),"feed_id":(r.get("feed") or {}).get("feed_id"),"provider":(r.get("feed") or {}).get("provider",""),"url":(r.get("feed") or {}).get("url",""),"classification":r.get("input_classification"),"baseline_sha256":(r.get("download") or {}).get("baseline_sha256") or (r.get("feed") or {}).get("baseline_download_sha256"),"new_sha256":(r.get("download") or {}).get("sha256"),"download_status":(r.get("download") or {}).get("status")} for r in rows if r.get("input_classification")!="SAME_INPUT"]
    fs=[]
    for r in rows:
        f=r.get("feed") or {};d=r.get("download") or {};x=r.get("analyzer") or {};m=r.get("mobilitydata") or {};fs.append({"corpus_index":f.get("corpus_index"),"feed_id":f.get("feed_id"),"provider":f.get("provider",""),"country":f.get("country",""),"url":f.get("url",""),"input_classification":r.get("input_classification"),"download_status":d.get("status"),"baseline_sha256":d.get("baseline_sha256") or f.get("baseline_download_sha256"),"new_sha256":d.get("sha256"),"analyzer_state":x.get("state"),"analyzer_exit":x.get("exit_code"),"analyzer_notices":x.get("notice_count"),"analyzer_wall_s":(x.get("timing") or {}).get("elapsed_seconds"),"analyzer_rss_kb":(x.get("timing") or {}).get("max_rss_kb"),"md_state":m.get("state"),"md_exit":m.get("exit_code"),"md_notices":m.get("notice_total"),"md_wall_s":(m.get("timing") or {}).get("elapsed_seconds"),"md_rss_kb":(m.get("timing") or {}).get("max_rss_kb")})
    cs={}
    for o in pa["observations"]:
        e=cs.setdefault(o["md_code"],{"md_code":o["md_code"],"occurrences":0,"parity_categories":Counter(),"count_relations":Counter(),"mapped_analyzer_rules":set()});e["occurrences"]+=1;e["parity_categories"][o["parity_category"]]+=1;e["count_relations"][o["count_relation"]]+=1;e["mapped_analyzer_rules"].update(o.get("mapped_analyzer_rules") or [])
    csr=[{"md_code":k,"occurrences":v["occurrences"],"parity_categories":dict(v["parity_categories"]),"count_relations":dict(v["count_relations"]),"mapped_analyzer_rules":sorted(v["mapped_analyzer_rules"])} for k,v in cs.items()];csr.sort(key=lambda x:(-x["occurrences"],x["md_code"]))
    summary={"attempted":len(rows),"input_classification_counts":dict(ic),"analyzer_state_counts":dict(Counter((r.get("analyzer") or {}).get("state","NOT_RUN") for r in rows)),"mobilitydata_state_counts":dict(Counter((r.get("mobilitydata") or {}).get("state","NOT_RUN") for r in rows)),"parity_category_counts":pa["category_counts"],"validator_state_pairs":pa["state_pairs"],"candidate_counts":dict(Counter(c["type"] for c in pa["candidates"])),"performance":{"analyzer_wall_seconds":perf([x["analyzer_wall_s"] for x in fs]),"mobilitydata_wall_seconds":perf([x["md_wall_s"] for x in fs]),"analyzer_peak_rss_kb":perf([x["analyzer_rss_kb"] for x in fs if x["analyzer_rss_kb"]]),"mobilitydata_peak_rss_kb":perf([x["md_rss_kb"] for x in fs if x["md_rss_kb"]])},"before_after":ba}
    (out/"summary.json").write_text(json.dumps(summary,indent=2,ensure_ascii=False)+"\n");(out/"parity-candidates-full.json").write_text(json.dumps(pa["candidates"],indent=2,ensure_ascii=False)+"\n");(out/"parity-code-summary.json").write_text(json.dumps(csr,indent=2,ensure_ascii=False)+"\n");(out/"before-after-events.json").write_text(json.dumps(events,indent=2,ensure_ascii=False)+"\n");(out/"before-after-summary.json").write_text(json.dumps(ba,indent=2,ensure_ascii=False)+"\n");(out/"input-drift.json").write_text(json.dumps(drift,indent=2,ensure_ascii=False)+"\n");(out/"analyzer-rules-full.json").write_text(json.dumps(serial(pa["analyzer_rules"]),indent=2,ensure_ascii=False)+"\n");(out/"md-codes-full.json").write_text(json.dumps(serial(pa["md_codes"]),indent=2,ensure_ascii=False)+"\n")
    with gzip.open(out/"parity-observations.json.gz","wt",encoding="utf-8") as f:json.dump(pa["observations"],f,ensure_ascii=False,separators=(",",":"))
    csvwrite(out/"feed-summary.csv",fs,list(fs[0]) if fs else ["feed_id"]);csvwrite(out/"input-drift.csv",drift,["corpus_index","feed_id","provider","url","classification","baseline_sha256","new_sha256","download_status"]);csvwrite(out/"before-after-events.csv",events,["feed_id","type","rule_id","old_state","new_state","old_count","new_count","delta","old_class","new_class","old_severity","new_severity","assessment"]);csvwrite(out/"parity-candidates.csv",pa["candidates"],["priority","type","parity_category","feed_id","md_code","md_severity","md_count","mapped_analyzer_rules","analyzer_mapped_count","analyzer_rule","analyzer_class","analyzer_severity","analyzer_count","analyzer_state","md_state","input_classification","url"])
    reps=[];seen=set()
    for c in pa["candidates"]:
        k=(c.get("type"),c.get("md_code"),c.get("analyzer_rule"),tuple(c.get("mapped_analyzer_rules") or []),c.get("analyzer_state"),c.get("md_state"))
        if k in seen:continue
        seen.add(k);reps.append(c)
        if len(reps)>=140:break
    (out/"triage-representatives.json").write_text(json.dumps(reps,indent=2,ensure_ascii=False)+"\n")
    lines=["# 1000-feed GTFS Analyzer rerun audit","","## Corpus/input identity","",f"- Attempted: **{len(rows)}**",f"- SAME_INPUT: **{ic.get('SAME_INPUT',0)}**",f"- INPUT_DRIFT: **{ic.get('INPUT_DRIFT',0)}**",f"- DOWNLOAD_FAILED: **{ic.get('DOWNLOAD_FAILED',0)}**","","Only SAME_INPUT feeds receive direct regression/improvement attribution.","","## New Analyzer vs MobilityData v8.0.1","",f"- Parity categories: `{json.dumps(pa['category_counts'])}`",f"- Candidate classes: `{json.dumps(dict(Counter(c['type'] for c in pa['candidates'])))}`","","## Old vs new Analyzer — SAME_INPUT only","",f"- Comparable feeds: **{ba['same_input_feeds_compared']}**",f"- State transitions: `{json.dumps(ba['state_transitions'])}`",f"- Red COMPLETE→PARTIAL/FATAL: **{len(ba['red_regressions'])}**",f"- Improvements FATAL→PARTIAL/COMPLETE: **{len(ba['improvements'])}**",f"- Removed observed rule IDs: **{len(ba['retired_observed_rules'])}**","","## Performance","",f"- Analyzer wall: `{json.dumps(summary['performance']['analyzer_wall_seconds'])}`",f"- MD wall: `{json.dumps(summary['performance']['mobilitydata_wall_seconds'])}`",f"- Analyzer peak RSS: `{json.dumps(summary['performance']['analyzer_peak_rss_kb'])}`",f"- MD peak RSS: `{json.dumps(summary['performance']['mobilitydata_peak_rss_kb'])}`","","## Targeted change impact",""]
    for k,v in ba["change_impact"].items():lines.append(f"- **{k}**: {v['feed_count']} feeds, {v['event_count']} events, net notice delta {v['net_notice_delta']}")
    lines += ["","Automated differences are triage candidates, not correctness verdicts."];(out/"AUDIT_SUMMARY.md").write_text("\n".join(lines)+"\n");print(json.dumps(summary,indent=2))
if __name__=="__main__":main()
