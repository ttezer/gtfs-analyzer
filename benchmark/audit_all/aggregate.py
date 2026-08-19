#!/usr/bin/env python3
import argparse,csv,gzip,importlib.util,json,math,re,statistics
from collections import Counter,defaultdict
from pathlib import Path

# Bağlam çözücü ve karar defterleri KANONİK modülden gelir (bridge `md_parity_mapping.py`).
# Düz `MAP` yalnız GERİ DÜŞÜŞTÜR: bağlam entry'si olmayan kodlar için.
from md_parity_mapping import classify_analyzer_divergence, classify_divergence, resolve_mapping


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
    # Yargılanmış sapma bir bulgu DEĞİLDİR; triyaj listesinin tepesini işgal etmemeli.
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
    """q'ıncı yüzdelik, doğrusal ara değerle. Medyan tek başına bir kuyruk göstermez."""
    xs=sorted(float(x) for x in values if x is not None)
    if not xs: return None
    pos=(len(xs)-1)*q
    lo,hi=math.floor(pos),math.ceil(pos)
    if lo==hi: return xs[lo]
    return xs[lo]*(hi-pos)+xs[hi]*(pos-lo)


def perf(values):
    """n / medyan / p95 / azami — tek bir medyan, uzun kuyruklu bir dağılımı gizler."""
    xs=[float(x) for x in values if x is not None]
    return {"n":len(xs),
            "median":statistics.median(xs) if xs else None,
            "p95":percentile(xs,0.95),
            "maximum":max(xs) if xs else None}


def require_measured(columns, attempted):
    """Tamamen boş bir ölçüm kolonu ARIZADIR; `null` medyan basıp geçmek onu gizler.

    run-31934698855'te `analyzer_wall_s` ve `md_wall_s` 4.259 satırın 4.259'unda boştu.
    Özet sessizce `null` bastı, `AUDIT_SUMMARY.md` "median: None s" yazdı ve koşum
    yayımlandı — hız ekseni yokken var sanıldı (#149). Ayrıştırıcı hatası tek satırlıktı;
    onu 4.259 satır boyunca görünmez kılan şey SESSİZ `None` idi.

    `attempted == 0` ise boş kolon beklenendir: ölçüm arızası ile boş korpus ayrılır.
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


# `completed` diyen bir satır, sürecin gerçekten bittiğini iddia eder. Analyzer'ın
# normal çıkışları 0 (temiz), 1 (bulgu var) ve 2'dir; 124/137 `timeout`/SIGKILL'dir
# ve o satır KESİK bir çıktıyı temiz gibi sunuyor demektir.
#
# run-32145833613'te `mdb-2014` tam bunu yaptı: 300 sn'de öldürüldü, "completed" +
# 1.551.740 bulgu olarak kaydedildi ve buradan 11 sapma satırı türetildi. Kusur
# `classify_analyzer`'daydı ve bu kapı onu YAKALAYABİLİRDİ; beş sağlık kapısının
# hiçbiri sınıflandırmanın DOĞRULUĞUNA bakmıyordu.
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


def main():
    ap=argparse.ArgumentParser()
    ap.add_argument("--results-dir",required=True)
    ap.add_argument("--map-file",required=True)
    ap.add_argument("--out-dir",required=True)
    ap.add_argument("--manifest",help="corpus-manifest.json; verilirse eksik/beklenmeyen feed kontrolü yapılır")
    args=ap.parse_args()
    out=Path(args.out_dir); out.mkdir(parents=True,exist_ok=True)
    MAP,AGG=load_map(args.map_file)
    inv=defaultdict(list)
    for code,rules in MAP.items():
        for r in rules: inv[r].append(code)

    rows=[]
    for p in sorted(Path(args.results_dir).glob("shard-*.jsonl")):
        for line in p.read_text(errors="replace").splitlines():
            if line.strip():
                try: rows.append(json.loads(line))
                except Exception: pass
    rows.sort(key=lambda x:int((x.get("feed") or {}).get("corpus_index",10**9)))
    with gzip.open(out/"all-results.json.gz","wt",encoding="utf-8") as f:
        json.dump(rows,f,ensure_ascii=False,separators=(",",":"))

    # Kapsam boşlukları (#135'ten taşındı): eksik bir shard SESSİZCE kaybolmamalı.
    # Manifest verilmişse beklenen feed kümesiyle karşılaştırılır; verilmemişse yalnız
    # mükerrer sonuç satırları raporlanır — "bakılmadı" ile "boşluk yok" KARIŞTIRILMAZ.
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

    # Mükerrer içerik (#135'ten taşındı): aynı SHA-256'yı paylaşan feed'ler DEDUP EDİLMEZ,
    # raporlanır. mdb-1241 ve tfs-374 ikisi de 925 STM_002 bulgusu veriyordu; aynı içeriğin
    # iki katalog kimliği altında sayılması bir bulguyu iki kez "bağımsız kanıt" yapar.
    sha_groups=defaultdict(set)
    for r in rows:
        sha=((r.get("download") or {}).get("sha256") or "")
        fid=((r.get("feed") or {}).get("feed_id") or "")
        if sha and fid: sha_groups[sha].add(fid)
    dupes=[{"sha256":s,"feed_count":len(i),"feed_ids":sorted(i)} for s,i in sha_groups.items() if len(i)>1]
    dupes.sort(key=lambda x:(-x["feed_count"],x["sha256"]))
    (out/"duplicate-content-groups.json").write_text(json.dumps(dupes,indent=2,ensure_ascii=False)+"\n")

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
            # MD jenerik kodları dosya/alan bağlamıyla çözülür; düz MAP geri düşüştür.
            # Tek kurala aliaslamak `missing_required_field`'i 17 alandan 2'sine bağlıyordu.
            res=resolve_mapping(code,{"sampleNotices":samples},fallback_rules=MAP.get(code,()))
            rules=list(res.analyzer_rules)
            # Sapma zaten yargılanmış mı? BY_DESIGN/UNMAPPED/MAPPED_DIVERGENCE tek çağrıda.
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
            # ⚠️ AGREGASYON KONTROLÜ `our==0`'DAN ÖNCE. Tersi, bilinen bir agregasyon farkını
            # priority 105'e çıkarıp "en yüksek öncelikli aday" tablosunu dolduruyordu.
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
                    # Analyzer tarafındaki hükümler `fp_adjudication.tsv`'de durur ve
                    # KURAL KİMLİĞİYLE anahtarlanır — `classify_divergence` MD kodu alır,
                    # buraya uymaz. Bu defter yıllardır hiç okunmuyordu: 251 satırın hepsi
                    # yargılanmış olduğu hâlde her koşumda taze görünüyordu (#163).
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
                # 🔴 Bu kova da defterden GEÇER. Eşleme yokluğu, hüküm yokluğu DEĞİLDİR:
                # `NO_MD_EQUIVALENT` ve `fp_adjudication.tsv` bir kuralı MD kodu olmadan
                # da yargılar. Sorgu eklenene kadar run 32290410755'te ARC_033/ARC_032/
                # FRQ_012/DQ_021 yargılanmış oldukları hâlde taze sapma sayıldı.
                adj,areason=classify_analyzer_divergence(rid)
                divergences.append({
                  "type":"adjudicated_divergence" if adj!="unreviewed" else "analyzer_spec_unmapped",
                  "unadjudicated_type":"analyzer_spec_unmapped" if adj!="unreviewed" else None,
                  "decision":adj,"reason":areason,
                  "feed_id":fid,"provider":feed.get("provider",""),"country":feed.get("country",""),"url":feed.get("url",""),
                  "analyzer_rule":rid,"analyzer_count":ourc,"analyzer_class":cls,"analyzer_severity":sev,"analyzer_sample":sm})

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
    # Ölçüm kolonları özetlenmeden önce denetlenir (#149).
    require_consistent_states(feed_csv)
    require_measured({"analyzer_wall_s":aclean,"md_wall_s":mclean,
                      "analyzer_peak_rss_kb":arss,"md_peak_rss_kb":mrss}, attempted)
    types=Counter(d["type"] for d in divergences)
    summary={
      "attempted":attempted,"downloaded":downloaded,"both_completed_cleanly":both,
      "state_pairs":{f"{a} | {m}":n for (a,m),n in state_pairs.items()},
      # Medyan + p95 + azami (#135'ten taşındı): tek bir medyan uzun kuyruğu gizler.
      # `n` ayrıca ölçümün KAÇ feed'e dayandığını söyler — 0 ise kolon boştur, medyan
      # `null` görünür ve bu bir ÖLÇÜM ARIZASIDIR, "veri yok" değil (#149).
      "analyzer_wall_s":perf(aclean),
      "md_wall_s":perf(mclean),
      "analyzer_peak_rss_kb":perf(arss),
      "md_peak_rss_kb":perf(mrss),
      "analyzer_wall_median_s":statistics.median(aclean) if aclean else None,
      "md_wall_median_s":statistics.median(mclean) if mclean else None,
      "analyzer_peak_rss_median_kb":statistics.median(arss) if arss else None,
      "md_peak_rss_median_kb":statistics.median(mrss) if mrss else None,
      "unique_analyzer_rules_seen":len(ar_list),"unique_md_codes_seen":len(mc_list),
      "divergence_candidate_counts":dict(types),
      "fresh_vs_stored_md_report_count_different":len(stored_disagreements)}
    (out/"summary.json").write_text(json.dumps(summary,indent=2,ensure_ascii=False)+"\n")

    top=divergences[:60]
    lines=["# 1000-feed GTFS Analyzer vs MobilityData audit","","## Corpus execution","",f"- Attempted feeds: **{attempted}**",f"- Successfully downloaded: **{downloaded}**",f"- Both validators completed cleanly: **{both}**",f"- Analyzer median wall time (completed feeds): **{summary['analyzer_wall_median_s']} s**",f"- MobilityData median wall time (completed feeds): **{summary['md_wall_median_s']} s**",f"- Analyzer rules observed: **{len(ar_list)}**",f"- MobilityData notice codes observed: **{len(mc_list)}**","","These are automated divergence *candidates*, not correctness verdicts. A count difference can be caused by aggregation, thresholds, scope, or a true validator bug.","","## Validator state pairs","","| Analyzer | MobilityData | Feeds |","|---|---|---:|"]
    for (a,m),n in state_pairs.most_common(): lines.append(f"| {a} | {m} | {n} |")
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
