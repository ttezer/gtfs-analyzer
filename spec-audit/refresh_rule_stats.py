#!/usr/bin/env python3
"""Korpus kural istatistiğini TAZELE — 242 feed, tek binary, tek tarih.

Neden: `corpus-evidence/rule_stats.csv`'nin VERİSİ 2026-08-06 koşumundandır ve o tarihten
sonra kural eklendi/değişti. Bayat baseline bu turda İKİ KEZ yanlış sonuca yol açtı:
`POST_BASELINE` kovası (10 kural "sessiz" sanıldı, üçü aslında ateşliyordu — `RTS_030`
23 feed/16.804 bulgu) ve `fatals.csv`'den okunan "ARC_004 ateşledi" sonucu (o fatal yolu
üç gün sonra kaldırılmıştı).

🔴 **BİR BASELINE'IN DOSYA TARİHİ, VERİSİNİN TARİHİ DEĞİLDİR.** Çıktı bu yüzden koşumun
tarihini ve binary'nin commit'ini KENDİ İÇİNE yazar.

`corpus_batch.py`'nin `results/` dizinine DOKUNMAZ (o araç tamamlanmış feed'i atlar ve
eski koşum kanıt olarak korunmalıdır); bu betik kendi ham çıktısını ayrı tutar.

    cargo build --release -p gtfs-analyzer
    python3 spec-audit/refresh_rule_stats.py            # devam ettirilebilir
    python3 spec-audit/refresh_rule_stats.py --report   # eski baseline ile FARK
"""
import argparse
import collections
import csv
import json
import os
import subprocess
import sys

ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
ZIPS = os.path.join(ROOT, "notgit/corpus/zips")
EV = os.path.join(ROOT, "spec-audit/corpus-evidence")
RAW = os.path.join(ROOT, "notgit/corpus/rule_stats_raw.csv")
OUT = os.path.join(EV, "rule_stats.csv")
BINARY = os.path.join(ROOT, "target/release/gtfs-analyzer")
# 🔴 ESKİ BASELINE İLE AYNI GÜN OLMALI. İlk denemem 20260515 ile koştu ve kıyas
# ANLAMSIZ çıktı: takvime göreli kurallar patladı (CAL_017 +%135490, CAL_008 −%89).
# Projenin kendi kayıtlı tuzağı — farklı 'bugün'lerle kıyas kural semantiğini değil
# TARİH FARKINI ölçer.
TODAY = "20260717"

# 🚫 KULLANICI TALİMATI (2026-08-16): `mdb-2904` HİÇBİR ŞEKİLDE KOŞULMAZ.
# 85 MB, tek başına 5.374.292 bulgu üretiyor ve her toplu koşumu kilitliyor.
# Bu liste yorum değil KAPIDIR — feed listesinden çıkarılır, atlanma açıkça bildirilir.
EXCLUDED_FEEDS = {"mdb-2904"}



def run_all():
    feeds = sorted(f for f in os.listdir(ZIPS)
                if f.endswith(".zip") and f[:-4] not in EXCLUDED_FEEDS)
    if EXCLUDED_FEEDS:
        print(f"atlanan feed (kullanıcı talimatı): {sorted(EXCLUDED_FEEDS)}", flush=True)
    done = set()
    if os.path.exists(RAW):
        with open(RAW) as fh:
            done = {r["feed"] for r in csv.DictReader(fh)}
    new = not os.path.exists(RAW)
    with open(RAW, "a", newline="") as fh:
        w = csv.DictWriter(fh, fieldnames=["feed", "status", "rule_id", "count"],
                           lineterminator="\n")
        if new:
            w.writeheader()
        for i, fn in enumerate(feeds, 1):
            feed = fn[:-4]
            if feed in done:
                continue
            p = subprocess.run(
                [BINARY, "validate", os.path.join(ZIPS, fn), "--json", "--today", TODAY],
                capture_output=True, timeout=1800)
            try:
                d = json.loads(p.stdout)
            except Exception:
                w.writerow({"feed": feed, "status": "crash", "rule_id": "", "count": 0})
                fh.flush()
                continue
            if "Fatal" in d:
                w.writerow({"feed": feed, "status": "FATAL", "rule_id": "", "count": 0})
                fh.flush()
                continue
            v = d.get("Ok", d)
            c = collections.Counter(n["rule_id"] for n in v.get("notices", []))
            st = v.get("validation_status", "COMPLETE")
            if not c:
                w.writerow({"feed": feed, "status": st, "rule_id": "", "count": 0})
            for rid, n in sorted(c.items()):
                w.writerow({"feed": feed, "status": st, "rule_id": rid, "count": n})
            fh.flush()
            if i % 40 == 0:
                print(f"{i}/{len(feeds)}", flush=True)
    print("ham çıktı yazıldı:", RAW)


def aggregate():
    """Ham çıktıyı `rule_stats.csv` şemasına toplar (eski dosyayla aynı sütunlar)."""
    per_rule = collections.defaultdict(list)   # rule → [(feed, count)]
    with open(RAW) as fh:
        for r in csv.DictReader(fh):
            if r["rule_id"]:
                per_rule[r["rule_id"]].append((r["feed"], int(r["count"])))

    reg = open(os.path.join(ROOT, "crates/rules/src/registry.rs"), encoding="utf-8").read()
    import re
    meta = {m.group(1): (m.group(2), m.group(3)) for m in re.finditer(
        r'r!\(\s*"([A-Z]{2,4}_\d{3})"\s*,\s*(\w+)\s*,\s*(\w+)\s*,', reg)}
    SEV = {"Kritik": "CRITICAL", "Yuksek": "HIGH", "Orta": "MEDIUM",
           "Dusuk": "LOW", "Bilgi": "INFO"}
    CLS = {"Spec": "SPEC", "Interop": "INTEROP", "Quality": "QUALITY",
           "Analytics": "ANALYTICS"}

    feeds_total = len({r["feed"] for r in csv.DictReader(open(RAW))})
    rows = []
    for rid, hits in per_rule.items():
        counts = [n for _, n in hits]
        counts.sort()
        mid = counts[len(counts) // 2] if len(counts) % 2 else \
            (counts[len(counts) // 2 - 1] + counts[len(counts) // 2]) / 2
        top = max(hits, key=lambda x: x[1])
        sev, cls = meta.get(rid, ("?", "?"))
        rows.append({
            "rule_id": rid,
            "severity": SEV.get(sev, sev),
            "rule_class": CLS.get(cls, cls),
            "feeds_triggered": len(hits),
            "feeds_pct": round(100 * len(hits) / feeds_total, 1),
            "total_notices": sum(counts),
            "median_per_triggered_feed": round(sum(counts) / len(counts), 1),
            "max_in_one_feed": top[1],
            "max_feed": top[0],
        })
    rows.sort(key=lambda r: -r["total_notices"])
    return rows, feeds_total


def report():
    new_rows, feeds_total = aggregate()
    new = {r["rule_id"]: r for r in new_rows}
    old = {r["rule_id"]: r for r in csv.DictReader(open(OUT))}

    gained = sorted(set(new) - set(old))
    lost = sorted(set(old) - set(new))
    print(f"taze koşum: {feeds_total} feed · tetiklenen kural {len(new)} "
          f"(eski baseline {len(old)})")
    print(f"\nYENİ ateşleyen ({len(gained)}):")
    for r in gained:
        print(f"  {r:8} {new[r]['feeds_triggered']:>3} feed / {new[r]['total_notices']:>7} bulgu")
    print(f"\nARTIK ateşlemeyen ({len(lost)}):")
    for r in lost:
        print(f"  {r:8} (eski: {old[r]['feeds_triggered']} feed / {old[r]['total_notices']} bulgu)")

    moved = []
    for rid in sorted(set(new) & set(old)):
        a, b = int(old[rid]["total_notices"]), new[rid]["total_notices"]
        if a and abs(b - a) / a > 0.2:
            moved.append((rid, a, b))
    print(f"\nHACMİ %20'den ÇOK değişen ({len(moved)}):")
    for rid, a, b in sorted(moved, key=lambda x: -abs(x[2] - x[1]))[:15]:
        print(f"  {rid:8} {a:>8} → {b:<8} ({(b-a)/a*100:+.0f}%)")


def write_out():
    rows, _ = aggregate()
    with open(OUT, "w", newline="") as fh:
        w = csv.DictWriter(fh, fieldnames=list(rows[0].keys()), lineterminator="\n")
        w.writeheader()
        w.writerows(rows)
    print("yazıldı:", OUT)


if __name__ == "__main__":
    ap = argparse.ArgumentParser()
    ap.add_argument("--report", action="store_true")
    ap.add_argument("--write", action="store_true")
    a = ap.parse_args()
    if a.report:
        report()
    elif a.write:
        write_out()
    else:
        if not os.path.exists(BINARY):
            sys.exit("release binary yok: cargo build --release -p gtfs-analyzer")
        run_all()
    sys.exit(0)
