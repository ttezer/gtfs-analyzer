#!/usr/bin/env python3
import argparse,csv,gzip,importlib.util,json,math,re,statistics
from collections import Counter,defaultdict
from pathlib import Path

# The context resolver and adjudication ledgers come from the CANONICAL module (bridge `md_parity_mapping.py`).
# Use the flat `MAP` only as a FALLBACK for codes without a context entry.
from md_parity_mapping import (CONTEXT_BY_CODE, classify_analyzer_divergence,
                               classify_divergence, resolve_mapping)


def load_map(path):
    spec=importlib.util.spec_from_file_location("mdmap",path)
    mod=importlib.util.module_from_spec(spec); spec.loader.exec_module(mod)
    return mod.MAP, getattr(mod,"AGG_RULES",set())

def elapsed_seconds(v):
    if not v:return None
    v=str(v).strip()
    try:
        parts=v.split(":")
        if len(parts)==1:return float(parts[0])
        if len(parts)==2:return float(parts[0])*60+float(parts[1])
        if len(parts)==3:return float(parts[0])*3600+float(parts[1])*60+float(parts[2])
    except: return None

def md_counts(m):
    return {g.get("code"):int(g.get("totalNotices") or 0) for g in (m or {}).get("groups",[]) if g.get("code")}

def md_severity(m,code):
    for g in (m or {}).get("groups",[]):
        if g.get("code")==code:return (g.get("severity") or "").upper()
    return ""

def md_sample(m,code):
    for g in (m or {}).get("groups",[]):
        if g.get("code")==code:return (g.get("sampleNotices") or [])[:2]
    return []

def priority(d):
    typ=d["type"]; sev=(d.get("md_severity") or "").upper()
    # An adjudicated divergence is NOT a finding; it must not occupy the top of the triage list.
    if typ=="adjudicated_divergence": return 10
    if typ=="context_unresolved": return 30
    if typ=="validator_state_asymmetry": return 110
    if typ=="md_mapped_missing" and sev=="ERROR": return 105
    if typ=="analyzer_spec_md_absent" and d.get("analyzer_severity") in ("CRITICAL","KRİTİK"): return 100
    if typ=="analyzer_spec_unmapped": return 95
    if typ=="analyzer_spec_md_absent": return 92
    if typ=="md_unmapped" and sev=="ERROR": return 90
    if typ=="md_mapped_missing": return 85
    if typ=="analyzer_mapped_md_absent": return 80
    if typ in ("md_mapped_under","md_mapped_over"): return 65
    if typ=="md_unmapped": return 55
    return 40

def percentile(values,q):
    """q-th percentile with linear interpolation. The median alone does not show a long tail."""
    xs=sorted(float(x) for x in values if x is not None)
    if not xs: return None
    pos=(len(xs)-1)*q
    lo,hi=math.floor(pos),math.ceil(pos)
    if lo==hi: return xs[lo]
    return xs[lo]*(hi-pos)+xs[hi]*(pos-lo)


def perf(values):
    """n / median / p95 / maximum — one median can hide a long-tailed distribution."""
    xs=[float(x) for x in values if x is not None]
    return {"n":len(xs),
            "median":statistics.median(xs) if xs else None,
            "p95":percentile(xs,0.95),
            "maximum":max(xs) if xs else None}


def require_measured(columns, attempted):
    """A wholly empty measurement column is an ERROR; printing a `null` median would hide it.

    In run-31934698855, `analyzer_wall_s` and `md_wall_s` were empty in all 4,259 rows.
    The summary silently printed `null`, wrote "median: None s" to `AUDIT_SUMMARY.md`,
    and the run was published—as if the speed axis existed when it did not (#149). The
    parser bug was one line; SILENT `None` kept it invisible across 4,259 rows.

    If `attempted == 0`, an empty column is expected: distinguish a measurement failure from an empty corpus.
    """
    if not attempted:
        return
    empty=sorted(name for name,values in columns.items() if not values)
    if empty:
        raise SystemExit(
            "measured column(s) came back wholly empty over "
            f"{attempted} attempted feeds: {', '.join(empty)}\n"
            "A median over zero samples is a broken measurement, not a null."
        )


# A row saying `completed` claims that the process really finished. The analyzer's
# normal exits are 0 (clean), 1 (findings), and 2; 124/137 are `timeout`/SIGKILL,
# so that row would present truncated output as clean.
#
# In run-32145833613, `mdb-2014` did exactly this: it was killed at 300 seconds, recorded as
# "completed" with 1,551,740 findings, and 11 divergence rows were derived from it. The bug
# was in `classify_analyzer`, and this gate COULD HAVE CAUGHT it; none of the five health gates
# checked classification ACCURACY.
ANALYZER_CLEAN_EXITS = (0, 1, 2)


def require_consistent_states(feed_rows):
    bad = [
        (r["feed_id"], r["analyzer_exit"])
        for r in feed_rows
        if r["analyzer_state"] == "completed"
        and r["analyzer_exit"] is not None
        and r["analyzer_exit"] not in ANALYZER_CLEAN_EXITS
    ]
    if bad:
        listed = ", ".join(f"{fid} (exit {code})" for fid, code in bad[:10])
        more = f" and {len(bad)-10} more" if len(bad) > 10 else ""
        raise SystemExit(
            f"{len(bad)} feed(s) recorded as analyzer_state=completed with an exit "
            f"code outside {ANALYZER_CLEAN_EXITS}: {listed}{more}\n"
            "A truncated run presented as a clean one corrupts every count derived "
            "from it. Fix the state classifier; do not publish the run."
        )

def summarize_analyzer_profiles(rows):
    """Summarize additive Auto/V3/V4 analyzer runs from shard rows."""
    profiles=defaultdict(lambda: {
        "attempted":0, "completed":0, "partial":0, "failed":0,
        "notice_total":0, "rules":defaultdict(lambda: {
            "feed_count":0, "notice_total":0, "affected_feeds":[]
        })
    })
    for row in rows:
        for profile, report in (row.get("analyzer_profiles") or {}).items():
            stats=profiles[profile]
            stats["attempted"]+=1
            state=report.get("state","not_run")
            if state=="completed": stats["completed"]+=1
            elif state.startswith("partial"): stats["partial"]+=1
            else: stats["failed"]+=1
            stats["notice_total"]+=int(report.get("notice_count") or 0)
            feed_id=(row.get("feed") or {}).get("feed_id","")
            for rule_id,count in (report.get("by_rule") or {}).items():
                entry=stats["rules"][rule_id]
                entry["feed_count"]+=1
                entry["notice_total"]+=int(count)
                if feed_id: entry["affected_feeds"].append(feed_id)
    serial_profiles={}
    serial_rules=[]
    for profile in sorted(profiles):
        stats=profiles[profile]
        rules={}
        for rule_id,entry in sorted(stats["rules"].items()):
            rules[rule_id]={
                "feed_count":entry["feed_count"],
                "notice_total":entry["notice_total"],
                "affected_feeds":entry["affected_feeds"],
            }
            serial_rules.append({
                "profile":profile,
                "rule_id":rule_id,
                **rules[rule_id],
            })
        serial_profiles[profile]={k:v for k,v in stats.items() if k!="rules"}
        serial_profiles[profile]["rules_seen"]=len(rules)
    serial_rules.sort(key=lambda x:(x["profile"],-x["feed_count"],-x["notice_total"],x["rule_id"]))
    return {"profiles":serial_profiles,"rules":serial_rules}

def load_result_rows(path):
    """Load a previous aggregate result for source-drift comparison."""
    path=Path(path)
    opener=gzip.open if path.suffix==".gz" else open
    with opener(path,"rt",encoding="utf-8") as f:
        data=json.load(f)
    if not isinstance(data,list):
        raise SystemExit(f"baseline results must contain a JSON array: {path}")
    return data

def compare_source_drift(rows, baseline_path):
    """Compare feed payload provenance with a previous run, without deduping."""
    if not baseline_path:
        return {
            "baseline_checked": False,
            "reason": "No --baseline-results was supplied; current payload hashes are still recorded.",
            "changed_feed_count": 0,
            "changed": [],
        }
    baseline=load_result_rows(baseline_path)
    current_by_id={((r.get("feed") or {}).get("feed_id") or ""):r for r in rows}
    baseline_by_id={((r.get("feed") or {}).get("feed_id") or ""):r for r in baseline}
    changed=[]
    for fid in sorted(set(current_by_id)&set(baseline_by_id)):
        current=(current_by_id[fid].get("download") or {})
        previous=(baseline_by_id[fid].get("download") or {})
        keys=("sha256","bytes","effective_url","http_status","content_type")
        differences={k:{"baseline":previous.get(k),"current":current.get(k)}
                     for k in keys if previous.get(k)!=current.get(k)}
        if differences:
            changed.append({"feed_id":fid,"differences":differences})
    return {
        "baseline_checked": True,
        "baseline_path": str(baseline_path),
        "baseline_feed_count": len(baseline_by_id),
        "current_feed_count": len(current_by_id),
        "compared_feed_count": len(set(current_by_id)&set(baseline_by_id)),
        "changed_feed_count": len(changed),
        "changed": changed,
        "missing_from_current": sorted(set(baseline_by_id)-set(current_by_id)),
        "new_in_current": sorted(set(current_by_id)-set(baseline_by_id)),
    }


def main():
    ap=argparse.ArgumentParser()
    ap.add_argument("--results-dir",required=True)
    ap.add_argument("--map-file",required=True)
    ap.add_argument("--out-dir",required=True)
    ap.add_argument("--manifest",help="corpus-manifest.json; verilirse eksik/beklenmeyen feed kontrolü yapılır")
    ap.add_argument("--baseline-results",help="previous all-results.json[.gz] for feed payload drift comparison")
    args=ap.parse_args()
    out=Path(args.out_dir); out.mkdir(parents=True,exist_ok=True)
    MAP,AGG=load_map(args.map_file)
    inv=defaultdict(list)
    # 🔴 REVERSE MAPPING IS BUILT FROM TWO SOURCES. Flat `MAP` is only HALF of the
    # rule→code relationship; the other half is in `CONTEXT_BY_CODE`, where 88 rules
    # do not appear in the flat table at all. Reading only `MAP` classifies those rules
    # as "unmapped to any MD code" and puts them in `analyzer_spec_unmapped`—exactly
    # what happened to 217 of 226 rows (65 rules) in run 32290410755. A direct repeat
    # of lesson #165: the mapping existed, but the consumer did not read it.
    for code,rules in MAP.items():
        for r in rules: inv[r].append(code)
    for code,entries in CONTEXT_BY_CODE.items():
        for e in entries:
            for r in e.analyzer_rules:
                if code not in inv[r]: inv[r].append(code)

    rows=[]
    for p in sorted(Path(args.results_dir).glob("shard-*.jsonl")):
        for line in p.read_text(errors="replace").splitlines():
            if line.strip():
                try: rows.append(json.loads(line))
                except Exception: pass
    rows.sort(key=lambda x:int((x.get("feed") or {}).get("corpus_index",10**9)))
    with gzip.open(out/"all-results.json.gz","wt",encoding="utf-8") as f:
        json.dump(rows,f,ensure_ascii=False,separators=(",",":"))
    profile_measurements=summarize_analyzer_profiles(rows)
    (out/"profile-summary.json").write_text(
        json.dumps(profile_measurements["profiles"],indent=2,ensure_ascii=False)+"\n"
    )
    (out/"profile-rules.json").write_text(
        json.dumps(profile_measurements["rules"],indent=2,ensure_ascii=False)+"\n"
    )

    # Coverage gaps (carried over from #135): a missing shard must not disappear SILENTLY.
    # With a manifest, compare against the expected feed set; without one, report only
    # duplicate result rows—do not CONFUSE "not checked" with "no gap".
    seen_ids=[((r.get("feed") or {}).get("feed_id") or "") for r in rows]
    attempted_counts=Counter(x for x in seen_ids if x)
    expected_ids=set()
    if args.manifest:
        man=json.loads(Path(args.manifest).read_text(encoding="utf-8"))
        expected_ids={f.get("feed_id","") for f in man.get("feeds",[]) if f.get("feed_id")}
    gaps={"manifest_checked":bool(args.manifest),
          "missing_manifest_feed_ids":sorted(expected_ids-set(attempted_counts)) if expected_ids else [],
          "unexpected_feed_ids":sorted(set(attempted_counts)-expected_ids) if expected_ids else [],
          "duplicate_result_rows":{k:v for k,v in attempted_counts.items() if v>1}}
    (out/"coverage-gaps.json").write_text(json.dumps(gaps,indent=2,ensure_ascii=False)+"\n")

    # Duplicate content (carried over from #135): feeds sharing a SHA-256 are NOT DEDUPLICATED;
    # they are reported. mdb-1241 and tfs-374 both produced 925 STM_002 findings; counting the
    # same content under two catalog IDs would turn one finding into two "independent proofs".
    sha_groups=defaultdict(set)
    for r in rows:
        sha=((r.get("download") or {}).get("sha256") or "")
        fid=((r.get("feed") or {}).get("feed_id") or "")
        if sha and fid: sha_groups[sha].add(fid)
    dupes=[{"sha256":s,"feed_count":len(i),"feed_ids":sorted(i)} for s,i in sha_groups.items() if len(i)>1]
    dupes.sort(key=lambda x:(-x["feed_count"],x["sha256"]))
    (out/"duplicate-content-groups.json").write_text(json.dumps(dupes,indent=2,ensure_ascii=False)+"\n")
    source_drift=compare_source_drift(rows,args.baseline_results)
    (out/"source-drift.json").write_text(json.dumps(source_drift,indent=2,ensure_ascii=False)+"\n")

    feed_csv=[]
    analyzer_rules={}
    md_codes={}
    divergences=[]
    state_pairs=Counter()
    stored_disagreements=[]

    for r in rows:
        feed=r.get("feed") or {}
        fid=feed.get("feed_id","")
        dl=r.get("download") or {}
        a=r.get("analyzer") or {}
        m=r.get("mobilitydata") or {}
        ast=a.get("state","not_run"); mst=m.get("state","not_run")
        state_pairs[(ast,mst)]+=1
        feed_csv.append({
          "corpus_index":feed.get("corpus_index"),"feed_id":fid,"provider":feed.get("provider",""),
          "country":feed.get("country",""),"url":feed.get("url",""),
          "download_status":dl.get("status","missing"),"zip_bytes":dl.get("bytes"),"sha256":dl.get("sha256"),
          "analyzer_state":ast,"analyzer_exit":a.get("exit_code"),"analyzer_notices":a.get("notice_count"),
          "analyzer_wall_s":elapsed_seconds((a.get("timing") or {}).get("elapsed")),
          "analyzer_rss_kb":(a.get("timing") or {}).get("max_rss_kb"),
          "md_state":mst,"md_exit":m.get("exit_code"),"md_notices":m.get("notice_total"),
          "md_groups":m.get("notice_groups"),"md_wall_s":elapsed_seconds((m.get("timing") or {}).get("elapsed")),
          "md_rss_kb":(m.get("timing") or {}).get("max_rss_kb"),
          "md_validation_s":m.get("validation_time_seconds"),
        })

        if ast!=mst and dl.get("status")=="ok":
            if ast!="completed" or mst!="completed":
                divergences.append({
                  "type":"validator_state_asymmetry","feed_id":fid,"provider":feed.get("provider",""),
                  "country":feed.get("country",""),"url":feed.get("url",""),
                  "analyzer_state":ast,"md_state":mst,
                  "analyzer_stderr":a.get("stderr_head",""),"md_stderr":m.get("stderr_head","")})

        for rid,c in (a.get("by_rule") or {}).items():
            ent=analyzer_rules.setdefault(rid,{"rule_id":rid,"feed_count":0,"notice_total":0,"affected_feeds":[],"classes":Counter(),"severities":Counter(),"sample":None})
            ent["feed_count"]+=1; ent["notice_total"]+=int(c); ent["affected_feeds"].append(fid)
            sm=(a.get("samples") or {}).get(rid) or {}
            if sm.get("rule_class"):ent["classes"][sm["rule_class"]]+=1
            if sm.get("severity"):ent["severities"][sm["severity"]]+=1
            if ent["sample"] is None:ent["sample"]={"feed_id":fid,"notice":sm}
        mc=md_counts(m)
        for code,c in mc.items():
            ent=md_codes.setdefault(code,{"code":code,"feed_count":0,"notice_total":0,"affected_feeds":[],"severities":Counter(),"sample":None})
            ent["feed_count"]+=1; ent["notice_total"]+=int(c); ent["affected_feeds"].append(fid)
            sev=md_severity(m,code)
            if sev:ent["severities"][sev]+=1
            if ent["sample"] is None:ent["sample"]={"feed_id":fid,"sample":md_sample(m,code)}

        stored=r.get("mobilitydata_stored") or {}
        if mst=="completed" and stored.get("status")=="available":
            sc=md_counts(stored)
            if mc!=sc: stored_disagreements.append({"feed_id":fid,"fresh":mc,"stored":sc})

        if ast!="completed" or mst not in ("completed","partial_internal","partial_oom"):
            continue
        ac={k:int(v) for k,v in (a.get("by_rule") or {}).items()}

        for code,mdc in mc.items():
            sev=md_severity(m,code)
            samples=md_sample(m,code)
            # MD generic codes are resolved by file/field context; the flat MAP is the fallback.
            # Aliasing to one rule had tied `missing_required_field` to only 2 of 17 fields.
            res=resolve_mapping(code,{"sampleNotices":samples},fallback_rules=MAP.get(code,()))
            rules=list(res.analyzer_rules)
            # Has the divergence already been adjudicated? BY_DESIGN/UNMAPPED/MAPPED_DIVERGENCE in one call.
            decision,_reason=classify_divergence(code)
            adjudicated=decision!="unreviewed"
            if not rules:
                divergences.append({
                  "type":"adjudicated_divergence" if adjudicated else ("context_unresolved" if not res.context_complete else "md_unmapped"),
                  "decision":decision,"feed_id":fid,"provider":feed.get("provider",""),"country":feed.get("country",""),
                  "url":feed.get("url",""),"md_code":code,"md_count":mdc,"md_severity":sev,"md_samples":samples})
                continue
            our=sum(ac.get(x,0) for x in rules)
            agg=any(x in AGG for x in rules)
            # ⚠️ CHECK AGGREGATION BEFORE `our==0`. The reverse promoted a known aggregation
            # difference to priority 105 and filled the "highest-priority candidate" table.
            if agg: typ="mapped_aggregation_present"
            elif our==0: typ="md_mapped_missing"
            elif our<mdc: typ="md_mapped_under"
            elif our>mdc: typ="md_mapped_over"
            else: typ="mapped_exact"
            if adjudicated and typ!="mapped_exact": typ="adjudicated_divergence"
            if typ not in ("mapped_exact","mapped_aggregation_present"):
                divergences.append({
                  "type":typ,"feed_id":fid,"provider":feed.get("provider",""),"country":feed.get("country",""),"url":feed.get("url",""),
                  "md_code":code,"md_count":mdc,"md_severity":sev,"mapped_analyzer_rules":rules,"analyzer_mapped_count":our,
                  "decision":decision,"mapping_kind":res.kind,"mapping_contexts":list(res.contexts),
                  "analyzer_rule_counts":{x:ac.get(x,0) for x in rules},"md_samples":md_sample(m,code),
                  "analyzer_samples":{x:(a.get("samples") or {}).get(x) for x in rules if ac.get(x,0)}})

        for rid,ourc in ac.items():
            sm=(a.get("samples") or {}).get(rid) or {}
            cls=(sm.get("rule_class") or "").upper(); sev=(sm.get("severity") or "").upper()
            codes=inv.get(rid,[])
            if codes:
                mt=sum(mc.get(code,0) for code in codes)
                if mt==0:
                    # Analyzer-side rulings live in `fp_adjudication.tsv` and are keyed by
                    # RULE ID—`classify_divergence` takes an MD code and does not apply here.
                    # This ledger had gone unread for years: all 251 rows looked fresh on every
                    # run even though they had already been adjudicated (#163).
                    adj,areason=classify_analyzer_divergence(rid)
                    base="analyzer_spec_md_absent" if cls=="SPEC" else "analyzer_mapped_md_absent"
                    divergences.append({
                      "type":"adjudicated_divergence" if adj!="unreviewed" else base,
                      "unadjudicated_type":base if adj!="unreviewed" else None,
                      "decision":adj,"reason":areason,
                      "feed_id":fid,"provider":feed.get("provider",""),"country":feed.get("country",""),"url":feed.get("url",""),
                      "analyzer_rule":rid,"analyzer_count":ourc,"analyzer_class":cls,"analyzer_severity":sev,
                      "mapped_md_codes":codes,"analyzer_sample":sm})
            elif cls=="SPEC":
                # 🔴 THIS BUCKET ALSO GOES THROUGH THE LEDGER. No mapping is not no ruling:
                # `NO_MD_EQUIVALENT` and `fp_adjudication.tsv` can adjudicate a rule without
                # an MD code. Until the lookup was added, ARC_033/ARC_032/FRQ_012/DQ_021 were
                # counted as fresh divergences in run 32290410755 despite being adjudicated.
                adj,areason=classify_analyzer_divergence(rid)
                divergences.append({
                  "type":"adjudicated_divergence" if adj!="unreviewed" else "analyzer_spec_unmapped",
                  "unadjudicated_type":"analyzer_spec_unmapped" if adj!="unreviewed" else None,
                  "decision":adj,"reason":areason,
                  "feed_id":fid,"provider":feed.get("provider",""),"country":feed.get("country",""),"url":feed.get("url",""),
                  "analyzer_rule":rid,"analyzer_count":ourc,"analyzer_class":cls,"analyzer_severity":sev,"analyzer_sample":sm})

    # 🔴 NO DIVERGENCE ROW MAY BE PRODUCED WITHOUT A LEDGER LOOKUP.
    # This invariant makes both defects from run 32290410755 STRUCTURALLY impossible.
    # There, `analyzer_spec_unmapped` emitted rows without any ledger query and nobody noticed—
    # the bucket silently looked correct. If a new bucket is added and its lookup is forgotten,
    # the run STOPS here instead of publishing a false count.
    # The only exception is `validator_state_asymmetry`: it is a WORK-STATE difference between
    # two validators, not a rule adjudication.
    LEDGERLESS_TYPES = {"validator_state_asymmetry"}
    undecided = sorted({d["type"] for d in divergences
                        if d["type"] not in LEDGERLESS_TYPES and not d.get("decision")})
    if undecided:
        raise SystemExit(
            f"Defter sorgusundan geçmeyen sapma türü: {', '.join(undecided)}. "
            f"Her sapma satırı `classify_divergence` ya da "
            f"`classify_analyzer_divergence` sonucunu `decision` alanında taşımalıdır; "
            f"taşımayan satır, yargılanmış bir kararı taze sapma gibi gösterir."
        )

    for d in divergences:d["priority"]=priority(d)
    divergences.sort(key=lambda d:(-d["priority"],d.get("feed_id",""),d.get("md_code",""),d.get("analyzer_rule","")))

    if feed_csv:
        with (out/"feed-summary.csv").open("w",newline="",encoding="utf-8") as f:
            w=csv.DictWriter(f,fieldnames=list(feed_csv[0]));w.writeheader();w.writerows(feed_csv)
    dfields=["priority","type","feed_id","provider","country","md_code","md_severity","md_count","mapped_analyzer_rules","analyzer_mapped_count","analyzer_rule","analyzer_class","analyzer_severity","analyzer_count","analyzer_state","md_state","url"]
    with (out/"divergence-candidates.csv").open("w",newline="",encoding="utf-8") as f:
        w=csv.DictWriter(f,fieldnames=dfields,extrasaction="ignore");w.writeheader()
        for d in divergences:
            x=dict(d)
            if isinstance(x.get("mapped_analyzer_rules"),list):x["mapped_analyzer_rules"]="|".join(x["mapped_analyzer_rules"])
            w.writerow(x)

    ar_list=[]
    for rid,e in analyzer_rules.items():
        ar_list.append({"rule_id":rid,"feed_count":e["feed_count"],"notice_total":e["notice_total"],"classes":dict(e["classes"]),"severities":dict(e["severities"]),"affected_feeds":e["affected_feeds"],"sample":e["sample"]})
    ar_list.sort(key=lambda x:(-x["feed_count"],-x["notice_total"],x["rule_id"]))
    (out/"analyzer-rules-full.json").write_text(json.dumps(ar_list,indent=2,ensure_ascii=False)+"\n")
    with (out/"analyzer-rules.csv").open("w",newline="",encoding="utf-8") as f:
        fields=["rule_id","feed_count","notice_total","classes","severities","affected_feeds"]
        w=csv.DictWriter(f,fieldnames=fields);w.writeheader()
        for e in ar_list:
            w.writerow({**{k:e[k] for k in ("rule_id","feed_count","notice_total")},"classes":json.dumps(e["classes"],ensure_ascii=False),"severities":json.dumps(e["severities"],ensure_ascii=False),"affected_feeds":"|".join(e["affected_feeds"])})

    mc_list=[]
    for code,e in md_codes.items():
        mc_list.append({"code":code,"feed_count":e["feed_count"],"notice_total":e["notice_total"],"severities":dict(e["severities"]),"affected_feeds":e["affected_feeds"],"sample":e["sample"]})
    mc_list.sort(key=lambda x:(-x["feed_count"],-x["notice_total"],x["code"]))
    (out/"md-codes-full.json").write_text(json.dumps(mc_list,indent=2,ensure_ascii=False)+"\n")
    (out/"divergence-candidates-full.json").write_text(json.dumps(divergences,indent=2,ensure_ascii=False)+"\n")
    (out/"stored-vs-fresh-md-disagreements.json").write_text(json.dumps(stored_disagreements,indent=2,ensure_ascii=False)+"\n")

    reps=[]; seen=set()
    for d in divergences:
        if d["type"]=="validator_state_asymmetry": key=(d["type"],d.get("analyzer_state"),d.get("md_state"))
        elif d.get("md_code"): key=(d["type"],d.get("md_code"),tuple(d.get("mapped_analyzer_rules") or []))
        else: key=(d["type"],d.get("analyzer_rule"))
        if key in seen:continue
        seen.add(key);reps.append(d)
        if len(reps)>=120:break
    (out/"triage-representatives.json").write_text(json.dumps(reps,indent=2,ensure_ascii=False)+"\n")

    attempted=len(rows)
    downloaded=sum(1 for r in rows if (r.get("download") or {}).get("status")=="ok")
    both=sum(1 for r in rows if (r.get("analyzer") or {}).get("state")=="completed" and (r.get("mobilitydata") or {}).get("state")=="completed")
    aclean=[x["analyzer_wall_s"] for x in feed_csv if x["analyzer_state"]=="completed" and x["analyzer_wall_s"] is not None]
    mclean=[x["md_wall_s"] for x in feed_csv if x["md_state"]=="completed" and x["md_wall_s"] is not None]
    arss=[x["analyzer_rss_kb"] for x in feed_csv if x["analyzer_state"]=="completed" and x["analyzer_rss_kb"]]
    mrss=[x["md_rss_kb"] for x in feed_csv if x["md_state"]=="completed" and x["md_rss_kb"]]
    # Validate measurement columns before summarizing them (#149).
    require_consistent_states(feed_csv)
    require_measured({"analyzer_wall_s":aclean,"md_wall_s":mclean,
                      "analyzer_peak_rss_kb":arss,"md_peak_rss_kb":mrss}, attempted)
    types=Counter(d["type"] for d in divergences)
    summary={
      "attempted":attempted,"downloaded":downloaded,"both_completed_cleanly":both,
      "state_pairs":{f"{a} | {m}":n for (a,m),n in state_pairs.items()},
      # Median + p95 + maximum (carried over from #135): one median hides a long tail.
      # `n` also says how MANY feeds support the measurement—if it is 0, the column is empty,
      # the `null` median is a MEASUREMENT FAILURE, not "no data" (#149).
      "analyzer_wall_s":perf(aclean),
      "md_wall_s":perf(mclean),
      "analyzer_peak_rss_kb":perf(arss),
      "md_peak_rss_kb":perf(mrss),
      "analyzer_wall_median_s":statistics.median(aclean) if aclean else None,
      "md_wall_median_s":statistics.median(mclean) if mclean else None,
      "analyzer_peak_rss_median_kb":statistics.median(arss) if arss else None,
      "md_peak_rss_median_kb":statistics.median(mrss) if mrss else None,
      "unique_analyzer_rules_seen":len(ar_list),"unique_md_codes_seen":len(mc_list),
      "analyzer_profiles":profile_measurements["profiles"],
      "divergence_candidate_counts":dict(types),
      "fresh_vs_stored_md_report_count_different":len(stored_disagreements),
      "source_drift":source_drift}
    (out/"summary.json").write_text(json.dumps(summary,indent=2,ensure_ascii=False)+"\n")

    top=divergences[:60]
    lines=["# 1000-feed GTFS Analyzer vs MobilityData audit","","## Corpus execution","",f"- Attempted feeds: **{attempted}**",f"- Successfully downloaded: **{downloaded}**",f"- Both validators completed cleanly: **{both}**",f"- Analyzer median wall time (completed feeds): **{summary['analyzer_wall_median_s']} s**",f"- MobilityData median wall time (completed feeds): **{summary['md_wall_median_s']} s**",f"- Analyzer rules observed: **{len(ar_list)}**",f"- MobilityData notice codes observed: **{len(mc_list)}**","","These are automated divergence *candidates*, not correctness verdicts. A count difference can be caused by aggregation, thresholds, scope, or a true validator bug.","","## Validator state pairs","","| Analyzer | MobilityData | Feeds |","|---|---|---:|"]
    for (a,m),n in state_pairs.most_common(): lines.append(f"| {a} | {m} | {n} |")
    lines += ["", "## Explicit GTFS-JP profile runs", "", "| Profile | Attempted | Completed | Partial | Failed | Notices | Rules |", "|---|---:|---:|---:|---:|---:|---:|"]
    for profile,stats in profile_measurements["profiles"].items():
        lines.append(f"| {profile} | {stats['attempted']} | {stats['completed']} | {stats['partial']} | {stats['failed']} | {stats['notice_total']} | {stats['rules_seen']} |")
    lines += ["","## Divergence candidate classes","","| Candidate class | Count |","|---|---:|"]
    for k,n in types.most_common(): lines.append(f"| {k} | {n} |")
    lines += ["","## Highest-priority candidates","","| Priority | Feed | Direction | MD code | Analyzer rule(s) | Counts |","|---:|---|---|---|---|---|"]
    for d in top:
        rules=d.get("mapped_analyzer_rules") or ([d["analyzer_rule"]] if d.get("analyzer_rule") else [])
        counts=""
        if d.get("md_count") is not None: counts+=f"MD {d.get('md_count')}"
        if d.get("analyzer_mapped_count") is not None: counts+=f" / A {d.get('analyzer_mapped_count')}"
        elif d.get("analyzer_count") is not None: counts+=f" / A {d.get('analyzer_count')}"
        lines.append(f"| {d['priority']} | {d.get('feed_id','')} | {d['type']} | {d.get('md_code','')} | {', '.join(rules)} | {counts} |")
    (out/"AUDIT_SUMMARY.md").write_text("\n".join(lines)+"\n")
    print(json.dumps(summary,indent=2))

if __name__=="__main__":
    main()
