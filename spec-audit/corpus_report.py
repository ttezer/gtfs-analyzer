#!/usr/bin/env python3
"""Korpus koşumu → DÖRT KÜÇÜK DOSYA (ham bulgular değil).

    cargo build --release -p gtfs-cli
    python3 spec-audit/corpus_report.py            # koşar, devam ettirilebilir
    python3 spec-audit/corpus_report.py --report   # eski baseline ile FARK

Neden bu dört dosya — her biri bu projede yaşanmış bir hatanın karşılığı:

1. `provenance.json` — binary commit'i, `--today`, feed kümesi, hariç tutulanlar.
   🔴 **BİR BASELINE'IN DOSYA TARİHİ, VERİSİNİN TARİHİ DEĞİLDİR.** 2026-08-15'te eski
   baseline `--today 20260717` ile üretilmişken 20260515 ile kıyaslandı ve 35 kural
   "değişmiş" göründü (CAL_017 +%135490). O kıyas KODU değil TARİHİ ölçüyordu.
2. `rule_stats.csv` — kural bazında toplam; diff'lenebilir tek artefakt.
3. `per_feed_rules.tsv` — feed × kural sayımı. Bir değişimi HANGİ FEED'in sürüklediğini
   ancak bu gösterir (`mdb-992` böyle bulundu: 6523 sayısı beş kuralda tekrar ediyordu).
4. `samples.jsonl` — kural başına en çok 3 bulgu (kısaltılmış). Örnek yoksa adjudikasyon
   için feed'i yeniden koşmak gerekir; *"sayıya değil İLK 5 ÖRNEĞE bak"* bu projenin
   kayıtlı dersidir.

⚠️ Tam JSON çıktıları SAKLANMAZ — korpus 11,6M bulgu üretiyor, okunamaz ve taşınamaz.
"""
import argparse
import collections
import csv
import json
import os
import re
import subprocess
import sys
import time

ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
ZIPS = os.path.join(ROOT, "notgit/corpus/zips")
OUTDIR = os.path.join(ROOT, "notgit/corpus/report")
BINARY = os.path.join(ROOT, "target/release/gtfs-analyzer")
BASELINE = os.path.join(ROOT, "spec-audit/corpus-evidence/rule_stats.csv")

# 🚫 KULLANICI TALİMATI (2026-08-16): `mdb-2904` hiçbir şekilde koşulmaz.
EXCLUDED_FEEDS = {"mdb-2904"}

# Eski baseline ile kıyas ancak AYNI GÜNDE anlamlıdır.
TODAY = "20260717"
SAMPLES_PER_RULE = 3
RAW = os.path.join(OUTDIR, "per_feed_rules.tsv")


def sh(*a):
    return subprocess.run(a, cwd=ROOT, capture_output=True, text=True).stdout.strip()


def write_provenance(feeds):
    os.makedirs(OUTDIR, exist_ok=True)
    prov = {
        "generated_at": time.strftime("%Y-%m-%dT%H:%M:%S%z"),
        "binary_commit": sh("git", "rev-parse", "HEAD"),
        "binary_dirty": bool(sh("git", "status", "--porcelain")),
        "today": TODAY,
        "feeds_run": len(feeds),
        "feeds_excluded": sorted(EXCLUDED_FEEDS),
        "note": "today MUST match the baseline being compared against; "
                "a different day measures the calendar, not the code.",
    }
    with open(os.path.join(OUTDIR, "provenance.json"), "w") as fh:
        json.dump(prov, fh, indent=2)
    return prov


def run():
    if not os.path.exists(BINARY):
        sys.exit("release binary yok: cargo build --release -p gtfs-cli")
    feeds = sorted(f for f in os.listdir(ZIPS)
                   if f.endswith(".zip") and f[:-4] not in EXCLUDED_FEEDS)
    prov = write_provenance(feeds)
    print(f"künye: commit={prov['binary_commit'][:8]} today={TODAY} "
          f"feed={len(feeds)} hariç={sorted(EXCLUDED_FEEDS)}", flush=True)

    done = set()
    if os.path.exists(RAW):
        with open(RAW) as fh:
            done = {r["feed"] for r in csv.DictReader(fh, delimiter="\t")}
    samples = collections.defaultdict(list)
    spath = os.path.join(OUTDIR, "samples.jsonl")
    new = not os.path.exists(RAW)

    with open(RAW, "a", newline="") as fh, open(spath, "a") as sfh:
        w = csv.DictWriter(fh, delimiter="\t", lineterminator="\n",
                           fieldnames=["feed", "status", "coverage_complete",
                                       "overall_score", "rule_id", "count"])
        if new:
            w.writeheader()
        for i, fn in enumerate(feeds, 1):
            feed = fn[:-4]
            if feed in done:
                continue
            p = subprocess.run([BINARY, "validate", os.path.join(ZIPS, fn),
                                "--json", "--today", TODAY],
                               capture_output=True, timeout=1800)
            try:
                d = json.loads(p.stdout)
            except Exception:
                w.writerow({"feed": feed, "status": "crash"})
                fh.flush()
                continue
            if "Fatal" in d:
                w.writerow({"feed": feed, "status": "FATAL:" + str(d["Fatal"].get("code"))})
                fh.flush()
                continue
            v = d.get("Ok", d)
            ns = v.get("notices", [])
            base = {
                "feed": feed,
                "status": v.get("validation_status", "COMPLETE"),
                "coverage_complete": v.get("reports", {}).get("r1", {}).get("coverage_complete"),
                "overall_score": v.get("metrics", {}).get("overall_score"),
            }
            for rid, n in sorted(collections.Counter(x["rule_id"] for x in ns).items()):
                w.writerow({**base, "rule_id": rid, "count": n})
            if not ns:
                w.writerow(base)
            for x in ns:
                rid = x["rule_id"]
                if len(samples[rid]) >= SAMPLES_PER_RULE:
                    continue
                samples[rid].append(1)
                sfh.write(json.dumps({
                    "rule_id": rid, "feed": feed, "file": x.get("file"),
                    "field": x.get("field"),
                    "observed": (x.get("observed_value") or "")[:80],
                    "message": (x.get("message") or "")[:160],
                }, ensure_ascii=False) + "\n")
            fh.flush()
            sfh.flush()
            print(f"[{i}/{len(feeds)}] {feed:12} {base['status']:9} {len(ns):>7} bulgu",
                  flush=True)
    aggregate()
    print("\nÇIKTI:", OUTDIR)
    for f in ("provenance.json", "rule_stats.csv", "per_feed_rules.tsv", "samples.jsonl"):
        p2 = os.path.join(OUTDIR, f)
        if os.path.exists(p2):
            print(f"  {f:22} {os.path.getsize(p2)/1024:8.1f} KB")


def aggregate():
    per = collections.defaultdict(list)
    feeds = set()
    with open(RAW) as fh:
        for r in csv.DictReader(fh, delimiter="\t"):
            feeds.add(r["feed"])
            if r.get("rule_id"):
                per[r["rule_id"]].append((r["feed"], int(r["count"])))
    reg = open(os.path.join(ROOT, "crates/rules/src/registry.rs"), encoding="utf-8").read()
    meta = {m.group(1): (m.group(2), m.group(3)) for m in re.finditer(
        r'r!\(\s*"([A-Z]{2,4}_\d{3})"\s*,\s*(\w+)\s*,\s*(\w+)\s*,', reg)}
    SEV = {"Kritik": "CRITICAL", "Yuksek": "HIGH", "Orta": "MEDIUM",
           "Dusuk": "LOW", "Bilgi": "INFO"}
    CLS = {"Spec": "SPEC", "Interop": "INTEROP", "Quality": "QUALITY",
           "Analytics": "ANALYTICS"}
    rows = []
    for rid, hits in per.items():
        counts = [n for _, n in hits]
        top = max(hits, key=lambda x: x[1])
        sev, cls = meta.get(rid, ("?", "?"))
        rows.append({
            "rule_id": rid, "severity": SEV.get(sev, sev), "rule_class": CLS.get(cls, cls),
            "feeds_triggered": len(hits),
            "feeds_pct": round(100 * len(hits) / len(feeds), 1),
            "total_notices": sum(counts),
            "median_per_triggered_feed": round(sum(counts) / len(counts), 1),
            "max_in_one_feed": top[1], "max_feed": top[0],
        })
    rows.sort(key=lambda r: -r["total_notices"])
    with open(os.path.join(OUTDIR, "rule_stats.csv"), "w", newline="") as fh:
        w = csv.DictWriter(fh, fieldnames=list(rows[0].keys()), lineterminator="\n")
        w.writeheader()
        w.writerows(rows)


def report():
    prov = json.load(open(os.path.join(OUTDIR, "provenance.json")))
    print(f"künye: commit={prov['binary_commit'][:8]} today={prov['today']} "
          f"feed={prov['feeds_run']} hariç={prov['feeds_excluded']}")
    if prov.get("binary_dirty"):
        print("  ⚠️ AĞAÇ KİRLİYDİ — bu koşum bir commit'e karşılık GELMİYOR.")
    new = {r["rule_id"]: r for r in csv.DictReader(open(os.path.join(OUTDIR, "rule_stats.csv")))}
    old = {r["rule_id"]: r for r in csv.DictReader(open(BASELINE))}
    print(f"\ntetiklenen {len(new)} (baseline {len(old)})")
    print(f"YENİ: {sorted(set(new) - set(old))}")
    print(f"SUSAN: {sorted(set(old) - set(new))}")
    moved = []
    for rid in set(new) & set(old):
        a, b = int(old[rid]["total_notices"]), int(new[rid]["total_notices"])
        if a and abs(b - a) / a > 0.2:
            moved.append((rid, a, b, new[rid]["max_feed"]))
    print(f"\n%20'den ÇOK değişen ({len(moved)}) — sürükleyen feed ile:")
    for rid, a, b, f in sorted(moved, key=lambda x: -abs(x[2] - x[1]))[:20]:
        print(f"  {rid:8} {a:>9,} → {b:<9,} ({(b-a)/a*100:+7.0f}%)  {f}")


if __name__ == "__main__":
    ap = argparse.ArgumentParser()
    ap.add_argument("--report", action="store_true")
    a = ap.parse_args()
    report() if a.report else run()
    sys.exit(0)
