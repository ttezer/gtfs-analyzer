#!/usr/bin/env python3
"""
Geniş corpus toplu koşumu — MobilityData kataloğundan N feed indir, bizim
validator ile koş, güvenilirlik sinyallerini raporla.

NEDEN: Güvenilirlik Sprint'i kapanışı "2. dalga kural seçimi, yeni ve daha
çeşitli corpus hacmi görülmeden yapılmamalıdır" diyor. Mevcut corpus 3 feed
(BART/Tokyo/TriMet) → aday sıralaması bu 3 feed'in gürültüsünü yansıtır.
Bu script o corpus'u genişletir.

AŞAMALAR (her biri ayrı çalışır, hepsi DEVAM ETTİRİLEBİLİR):
    python3 notgit/corpus_batch.py sample   --n 250 --seed 42
    python3 notgit/corpus_batch.py resolve                    # MD API: snapshot + rapor URL'i
    python3 notgit/corpus_batch.py fetch                       # zip + MD raporu
    python3 notgit/corpus_batch.py validate --today 20260717
    python3 notgit/corpus_batch.py report

Varsayılan çalışma dizini: notgit/corpus/ (--dir ile değiştir).
Her aşama kaldığı yerden devam eder: inen zip yeniden inmez, koşan feed
yeniden koşmaz. Yarıda kesmek güvenli (Ctrl-C).

MD PARİTESİ: `resolve` MD API'sinden her feed'in SON snapshot'ını çözer →
zip ve MD raporu AYNI snapshot dizininden gelir (rapor gerçekten o zip'e ait).
Gerekli: refresh token → notgit/.md_token (gitignore, mod 600) veya
MD_REFRESH_TOKEN env. `resolve` atlanırsa katalog urls.latest'e düşülür:
çalışır ama BAYAT olabilir ve MD raporu eşleştirilemez.
Karşılaştırma: notgit/md_parity_audit.py (MAP/AGG_RULES hazır) — bu script
onun beklediği golden şeklini üretir.
"""

import argparse
import csv
import json
import os
import random
import re
import subprocess
import sys
import time
from collections import defaultdict

REPO = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
CATALOG_URL = "https://bit.ly/catalogs-csv"
UA = "gtfs-analyzer-corpus-batch/1.0 (+research; contact via repo)"

# Katalog 3.358 satır; bunların bir kısmı gtfs-rt / auth'lu / ölü.
# Bu filtreler memory'deki ölçümle uyumlu (~2.191 auth'suz, ~1.478 auth'suz+aktif).
def eligible(row):
    return (
        row.get("data_type") == "gtfs"
        and row.get("status") in ("active", "")
        and row.get("urls.authentication_type", "") in ("", "0")
        and row.get("urls.latest")
        and not row.get("redirect.id")
    )


# ── Aşama 1: örnekleme ────────────────────────────────────────────────────────
def cmd_sample(args):
    raw = fetch_bytes(CATALOG_URL, cache=os.path.join(args.dir, "catalog.csv"))
    rows = list(csv.DictReader(raw.decode("utf-8-sig").splitlines()))
    pool = [r for r in rows if eligible(r)]
    print(f"katalog {len(rows)} satır → uygun {len(pool)}")

    # KATMANLI örnekleme: ülkeye göre grupla, ülke başına tavan uygula.
    # Kör rastgele seçim kataloğun kendi dağılımını verir (ABD/Avrupa ağırlıklı)
    # ve amacı ıskalar — amaç HACİM değil ÇEŞİTLİLİK (farklı üretici araçları,
    # farklı profiller, farklı dil/coğrafya desenleri).
    rng = random.Random(args.seed)
    by_country = defaultdict(list)
    for r in pool:
        by_country[r.get("location.country_code") or "??"].append(r)
    for v in by_country.values():
        rng.shuffle(v)

    picked, countries = [], sorted(by_country)
    rng.shuffle(countries)
    # Tur tur dağıt: her turda her ülkeden 1 tane → küçük ülkeler temsil edilir,
    # büyük ülkeler kotayı tek başına yemez.
    depth = 0
    while len(picked) < args.n and depth < args.per_country:
        added = False
        for c in countries:
            if len(picked) >= args.n:
                break
            if depth < len(by_country[c]):
                picked.append(by_country[c][depth])
                added = True
        if not added:
            break
        depth += 1

    os.makedirs(args.dir, exist_ok=True)
    plan = os.path.join(args.dir, "plan.csv")
    with open(plan, "w", newline="") as f:
        w = csv.writer(f)
        w.writerow(["id", "country", "provider", "zip_url"])
        for r in picked:
            w.writerow([
                f"mdb-{r['mdb_source_id']}",
                r.get("location.country_code") or "??",
                (r.get("provider") or "")[:60],
                r["urls.latest"],
            ])
    dist = defaultdict(int)
    for r in picked:
        dist[r.get("location.country_code") or "??"] += 1
    print(f"seçildi {len(picked)} feed / {len(dist)} ülke → {plan}")
    print("en çok: " + ", ".join(f"{c}={n}" for c, n in sorted(dist.items(), key=lambda x: -x[1])[:8]))
    # --n istenen ama tavan yüzünden ulaşılamayan sayı SESSİZCE düşerdi ("500 istedim,
    # 276 geldi") → açıkça söyle ve tavanın ne demek olduğunu hatırlat.
    if len(picked) < args.n:
        reach = sum(min(len(v), args.per_country) for v in by_country.values())
        print(
            f"\n⚠️  {args.n} istendi, {len(picked)} seçilebildi: --per-country {args.per_country} "
            f"tavanı üst sınırı {reach} yapıyor.\n"
            f"    Daha fazlası için --per-country yükseltin — AMA fazladan feed'ler büyük "
            f"ülkelerden gelir\n    (havuzda US={len(by_country.get('US', []))}/{len(pool)}), "
            f"yani ÇEŞİTLİLİK değil HACİM eklersiniz."
        )
    print(f"\nsonraki: python3 notgit/corpus_batch.py fetch --dir {args.dir}")


# ── Aşama 1b: snapshot çözümleme (MD API) ─────────────────────────────────────
TOKEN_FILE = os.path.join(REPO, "notgit", ".md_token")
API = "https://api.mobilitydatabase.org"


def access_token():
    """refresh token → kısa ömürlü access token.

    Refresh token `notgit/.md_token` dosyasından okunur (notgit gitignore'da,
    mod 600) ya da MD_REFRESH_TOKEN env'inden. Token'ı ASLA argümana/loga koyma.
    """
    tok = os.environ.get("MD_REFRESH_TOKEN")
    if not tok and os.path.exists(TOKEN_FILE):
        with open(TOKEN_FILE) as f:
            tok = f.read().strip()
    if not tok:
        sys.exit(
            f"MD refresh token yok. {TOKEN_FILE} dosyasına yazın ya da MD_REFRESH_TOKEN\n"
            f"env'ini set edin (mobilitydatabase.org hesabından alınır)."
        )
    p = subprocess.run(
        ["curl", "-sS", "--fail", "-X", "POST", f"{API}/v1/tokens",
         "-H", "Content-Type: application/json",
         "--data-binary", "@-"],
        input=json.dumps({"refresh_token": tok}).encode(), capture_output=True,
    )
    if p.returncode != 0:
        sys.exit(f"token takası başarısız: {p.stderr.decode()[:200]}")
    return json.loads(p.stdout)["access_token"]


def cmd_resolve(args):
    """Her feed için SON snapshot'ı çöz: zip + MD raporu AYNI dizinden.

    Bu, katalogdaki `urls.latest`'in iki kusurunu birden çözer: (1) timestamp
    vermiyor → MD raporu eşleştirilemiyordu, (2) bayat (BART aynası 06-04 iken
    gerçek snapshot 06-25) ve bazen 404.
    """
    plan = read_plan(args.dir)
    at = access_token()
    out = os.path.join(args.dir, "resolved.csv")
    have = set()
    if os.path.exists(out):  # devam ettirilebilir
        with open(out) as f:
            have = {r["id"] for r in csv.DictReader(f)}
    new = not os.path.exists(out)
    with open(out, "a", newline="") as f:
        w = csv.writer(f)
        if new:
            # api_* sütunları MD API'sinin ÖZET sayıları — ön-eleme için pratik
            # (indirmeden "MD burada error görmüş mü?").
            # ⚠️ BUNLAR RAPORLA ÇELİŞEBİLİR: mdb-2077'de API total_warning=316
            # derken aynı koşumun (validated_at birebir aynı) report_8.0.0.json'u
            # 63 warning listeliyor. PARİTEDE RAPOR JSON'U ESAS AL — validator'ın
            # gerçek çıktısı o, md_parity_audit.py da onu okur.
            w.writerow(["id", "country", "snapshot", "zip_url", "report_url",
                        "validator_version", "zip_mb", "api_error", "api_warning", "api_info"])
        for i, row in enumerate(plan, 1):
            if row["id"] in have:
                continue
            try:
                d = api_get(at, f"/v1/gtfs_feeds/{row['id']}/datasets?latest=true")
                if not d:
                    raise ValueError("dataset yok")
                d = d[0] if isinstance(d, list) else d
                vr = d.get("validation_report") or {}
                w.writerow([
                    row["id"], row["country"], d["id"], d["hosted_url"],
                    vr.get("url_json", ""), vr.get("validator_version", ""),
                    d.get("zipped_folder_size_mb", ""),
                    vr.get("total_error", ""), vr.get("total_warning", ""), vr.get("total_info", ""),
                ])
                f.flush()
                print(f"[{i}/{len(plan)}] {row['id']:12} {d['id']:26} "
                      f"MD:{vr.get('validator_version','?'):5} rapor={'✅' if vr.get('url_json') else '—'}")
            except Exception as e:
                note(args.dir, "resolve_failed.csv", [row["id"], str(e)[:150]])
                print(f"[{i}/{len(plan)}] {row['id']:12} ÇÖZÜLEMEDİ: {str(e)[:60]}")
            time.sleep(args.delay)
    print(f"\n→ {out}\nsonraki: python3 notgit/corpus_batch.py fetch --dir {args.dir}")


def api_get(at, path):
    p = subprocess.run(
        ["curl", "-sS", "--fail", "-H", f"Authorization: Bearer {at}",
         "--max-time", "60", f"{API}{path}"],
        capture_output=True,
    )
    if p.returncode != 0:
        raise RuntimeError(p.stderr.decode()[:120])
    return json.loads(p.stdout)


# ── Aşama 2: indirme ──────────────────────────────────────────────────────────
def cmd_fetch(args):
    # resolve çalıştıysa onun URL'lerini kullan (snapshot'lı, MD raporuyla eşleşen);
    # yoksa plan.csv'deki katalog `urls.latest`'ine düş (rapor eşleşmesi OLMAZ).
    resolved = os.path.join(args.dir, "resolved.csv")
    reports = os.path.join(args.dir, "reports")
    if os.path.exists(resolved):
        with open(resolved) as f:
            plan = list(csv.DictReader(f))
        os.makedirs(reports, exist_ok=True)
        print(f"resolved.csv kullanılıyor ({len(plan)} feed) → MD raporları da inecek")
    else:
        plan = read_plan(args.dir)
        print("⚠️  resolved.csv yok → katalog urls.latest (bayat olabilir, MD raporu YOK).\n"
              "    Parite istiyorsanız önce: corpus_batch.py resolve")
    zips = os.path.join(args.dir, "zips")
    os.makedirs(zips, exist_ok=True)
    skipped = failed = ok = 0

    for i, row in enumerate(plan, 1):
        # MD raporu (varsa) — küçük dosya, zip atlansa bile indirmeye değmez → zip'le birlikte
        rurl = row.get("report_url")
        if rurl:
            rdest = os.path.join(reports, f"{row['id']}.report.json")
            if not os.path.exists(rdest):
                try:
                    curl(rurl, rdest, timeout=120)
                except Exception as e:
                    note(args.dir, "fetch_skipped.csv", [row["id"], "report_error", str(e)[:100]])

        dest = os.path.join(zips, f"{row['id']}.zip")
        if os.path.exists(dest) and os.path.getsize(dest) > 0:
            skipped += 1
            continue
        # resolve boyutu verdiyse İNDİRMEDEN ele: 580 MB'ı yarıya kadar çekip
        # curl'ün hata metnine bakmak hem yavaş hem kırılgan.
        known_mb = parse_float(row.get("zip_mb"))
        if known_mb and known_mb > args.max_mb:
            failed += 1
            note(args.dir, "fetch_skipped.csv", [row["id"], "too_big", f"{known_mb:.0f}MB (API)"])
            print(f"[{i}/{len(plan)}] {row['id']:12} ATLANDI ({known_mb:.0f}MB > {args.max_mb}MB)")
            continue
        try:
            size = download(row["zip_url"], dest, args.max_mb)
            ok += 1
            print(f"[{i}/{len(plan)}] {row['id']:12} {size/1e6:7.1f} MB  {row['country']}")
        except SkipTooBig as e:
            failed += 1
            note(args.dir, "fetch_skipped.csv", [row["id"], "too_big", str(e)])
            print(f"[{i}/{len(plan)}] {row['id']:12} ATLANDI ({e})")
        except Exception as e:
            failed += 1
            note(args.dir, "fetch_skipped.csv", [row["id"], "error", str(e)[:120]])
            print(f"[{i}/{len(plan)}] {row['id']:12} HATA: {str(e)[:70]}")
        time.sleep(args.delay)  # nazik ol: ortak altyapı

    print(f"\nindi={ok} zaten-vardı={skipped} başarısız={failed}")
    print(f"sonraki: python3 notgit/corpus_batch.py validate --dir {args.dir} --today YYYYMMDD")


# ── Aşama 3: bizim validator ──────────────────────────────────────────────────
def cmd_validate(args):
    zips = os.path.join(args.dir, "zips")
    out = os.path.join(args.dir, "results")
    os.makedirs(out, exist_ok=True)
    files = sorted(f for f in os.listdir(zips) if f.endswith(".zip"))
    if not files:
        sys.exit(f"{zips} boş — önce fetch çalıştırın")
    # --only: sabit bir --today ile YENİ feed eklerken şart. Sabit tarih verildiğinde
    # `md` modunda atlanmış her feed (MD raporu olmayanlar) birden koşmaya uygun hale
    # gelir — korpus istemeden büyür ve rule_stats tabanı kayar. Filtre eklenen feed'i
    # yalıtır.
    if args.only:
        want = set(args.only.split(","))
        files = [f for f in files if f[:-4] in want]
        missing = want - {f[:-4] for f in files}
        if missing:
            sys.exit(f"--only: bu zip'ler yok: {sorted(missing)}")

    print(f"{len(files)} feed · today={args.today} · release binary derleniyor…")
    subprocess.run(["cargo", "build", "--release", "-p", "gtfs-analyzer"], cwd=REPO, check=True)
    binary = os.path.join(REPO, "target", "release", "gtfs-analyzer")

    # `--today md` → her feed KENDİ MD raporunun doğrulama gününde koşulur.
    # NEDEN: tarihe göreli kurallar (TRP_023 "7 günde sefer yok", feed_expiration_*,
    # future_calendar, service_extends_far…) "bugün"e görelidir. MD raporlarının
    # yarısı 2026-05'ten, sabit koşumumuz Temmuz'dan → farklı "bugün"lerle kıyas
    # ANLAMSIZ; mevsimlik bir feed Mayıs'ta "sefer yok" verir, Temmuz'da vermez.
    # Aynı güne hizalayınca fark yalnız KURAL semantiğinden gelir.
    md_days = {}
    if args.today == "md":
        reports = os.path.join(args.dir, "reports")
        for fn in files:
            fid = fn[:-4]
            try:
                with open(os.path.join(reports, f"{fid}.report.json")) as f:
                    v = json.load(f)["summary"]["validatedAt"]  # 2026-05-07T16:33:55Z
                md_days[fid] = int(v[:10].replace("-", ""))
            except Exception:
                pass
        if not md_days:
            sys.exit("--today md: MD raporu bulunamadı → önce resolve + fetch çalıştırın")
        print(f"  MD tarihleri çözüldü: {len(md_days)}/{len(files)} feed "
              f"({min(md_days.values())}–{max(md_days.values())})")

    done = fatal = crash = 0
    for i, fn in enumerate(files, 1):
        fid = fn[:-4]
        dest = os.path.join(out, f"{fid}.json")
        if os.path.exists(dest):
            done += 1
            continue
        today = md_days.get(fid) if args.today == "md" else args.today
        if today is None:  # md modunda raporu olmayan feed → kıyaslanamaz, atla
            note(args.dir, "validate_skipped.csv", [fid, "md raporu yok → today çözülemedi"])
            continue
        t0 = time.time()
        # exit 0 = notice yok · 1 = notice VAR · 2 = fatal/config hatası.
        # 1 bir HATA DEĞİL → check=True KULLANMA (bkz. project_cli memory).
        p = subprocess.run(
            [binary, "validate", os.path.join(zips, fn), "--json", "--today", str(today)],
            capture_output=True, timeout=args.timeout,
        )
        secs = time.time() - t0
        if p.returncode not in (0, 1, 2) or not p.stdout:
            crash += 1
            note(args.dir, "validate_crashes.csv", [fid, p.returncode, p.stderr.decode()[:200]])
            print(f"[{i}/{len(files)}] {fid:12} ÇÖKME rc={p.returncode}")
            continue
        try:
            summary = to_golden(json.loads(p.stdout), fid, today, secs)
        except Exception as e:
            crash += 1
            note(args.dir, "validate_crashes.csv", [fid, "parse", str(e)[:200]])
            continue
        with open(dest, "w") as f:
            json.dump(summary, f, indent=2)
        done += 1
        if summary["fatal"]:
            fatal += 1
        tag = f"FATAL {summary['fatal']}" if summary["fatal"] else f"{summary['validation']['notice_total_actual']:>7} notice"
        print(f"[{i}/{len(files)}] {fid:12} {tag}  {secs:5.1f}s")

    print(f"\nkoşuldu={done} fatal={fatal} çökme={crash}")
    print(f"sonraki: python3 notgit/corpus_batch.py report --dir {args.dir}")


def to_golden(parsed, fid, today, secs):
    """CLI çıktısı → golden'a benzer şekil.

    CLI `--json` artık DÜZ ZARF verir: {"status":"ok", "notices":[…], "reports":{…}}
    veya {"status":"fatal", "code":…}. Eskiden ham `ValidateResult` enum'uydu
    ({"Ok":{…}}/{"Fatal":{…}}) ve bu fonksiyon onu bekliyordu — 2026-08-01'de iki Flex
    feed'i eklenirken ikisi de "çökme" olarak düştü. Eski şekil, 07-17 korpus koşumundan
    kalan sonuçları yeniden üretmek gerekirse diye tolere edilmeye devam ediyor.
    rule_counts golden ile AYNI formülle kurulur: capped_totals[id] ?? görünür sayım.
    """
    base = {"feed": fid, "validate_date": today, "elapsed_sec": round(secs, 2), "fatal": None}
    if parsed.get("status") == "fatal":
        base["fatal"] = parsed.get("code", "?")
        base["fatal_message"] = str(parsed.get("message", ""))[:300]
        base["validation"] = {"rule_counts": {}, "scores": {}, "notice_total_actual": 0}
        return base
    if "Fatal" in parsed:  # eski enum şekli
        base["fatal"] = parsed["Fatal"].get("code", "?")
        base["fatal_message"] = parsed["Fatal"].get("message", "")[:300]
        base["validation"] = {"rule_counts": {}, "scores": {}, "notice_total_actual": 0}
        return base

    vr = parsed["Ok"] if "Ok" in parsed else parsed
    visible, meta = defaultdict(int), {}
    for n in vr["notices"]:
        visible[n["rule_id"]] += 1
        meta.setdefault(n["rule_id"], {"severity": n["severity"], "rule_class": n["rule_class"]})
    capped = vr.get("capped_totals") or {}
    rules = {
        rid: {**meta[rid], "count": capped.get(rid, visible[rid])}
        for rid in sorted(visible)
    }
    r5 = (vr.get("reports") or {}).get("r5") or {}
    m = vr.get("metrics") or {}
    base["validation"] = {
        "rule_counts": rules,
        "notice_total_actual": sum(r["count"] for r in rules.values()),
        "scores": {
            "overall": r5.get("score"), "pub_score": r5.get("pub_score"),
            "spec": r5.get("spec_score"), "interop": r5.get("interop_score"),
            "quality": r5.get("quality_score"), "analytics": r5.get("analytics_score"),
        },
    }
    base["feed_metrics"] = {k: m.get(k) for k in ("route_count", "stop_count", "trip_count")}
    return base


# ── Aşama 4: rapor ────────────────────────────────────────────────────────────
def cmd_report(args):
    out = os.path.join(args.dir, "results")
    res = []
    for fn in sorted(os.listdir(out)):
        if fn.endswith(".json"):
            with open(os.path.join(out, fn)) as f:
                res.append(json.load(f))
    if not res:
        sys.exit("sonuç yok — önce validate çalıştırın")

    ok = [r for r in res if not r["fatal"]]
    fatals = [r for r in res if r["fatal"]]

    # Kural bazında: kaç feed'de çıktı, toplam, tek feed'deki en yüksek.
    feeds_with = defaultdict(int)
    totals = defaultdict(int)
    worst = {}
    sev = {}
    for r in ok:
        for rid, d in r["validation"]["rule_counts"].items():
            feeds_with[rid] += 1
            totals[rid] += d["count"]
            sev[rid] = (d["severity"], d["rule_class"])
            if rid not in worst or d["count"] > worst[rid][0]:
                worst[rid] = (d["count"], r["feed"])

    rows = []
    for rid in sorted(feeds_with, key=lambda x: -totals[x]):
        s, c = sev[rid]
        rows.append({
            "rule_id": rid, "severity": s, "rule_class": c,
            "feeds_triggered": feeds_with[rid],
            "feeds_pct": round(100 * feeds_with[rid] / max(len(ok), 1), 1),
            "total_notices": totals[rid],
            "median_per_triggered_feed": round(totals[rid] / feeds_with[rid], 1),
            "max_in_one_feed": worst[rid][0], "max_feed": worst[rid][1],
        })
    write_csv(os.path.join(args.dir, "rule_stats.csv"), rows)

    # Bizim registry'de olup corpus'ta HİÇ çıkmayan kurallar.
    try:
        allr = subprocess.run(
            ["cargo", "run", "-q", "-p", "gtfs-rules", "--example", "gen_rules"],
            cwd=REPO, capture_output=True, timeout=180,
        )
        del allr
    except Exception:
        pass
    never = sorted(set(known_rule_ids()) - set(feeds_with))

    write_csv(os.path.join(args.dir, "fatals.csv"),
              [{"feed": r["feed"], "code": r["fatal"], "message": r.get("fatal_message", "")} for r in fatals])

    slow = sorted(ok, key=lambda r: -r["elapsed_sec"])[:10]

    lines = [
        "# Corpus toplu koşum raporu",
        "",
        f"- Feed: **{len(res)}** (temiz {len(ok)}, fatal {len(fatals)})",
        f"- Tetiklenen kural: **{len(feeds_with)}** · corpus'ta hiç çıkmayan: **{len(never)}**",
        f"- Toplam notice: **{sum(totals.values()):,}**",
        "",
        "## 🔴 Fatal (en yüksek sinyal — MD açabildiyse bizde bug)",
        "",
    ]
    if fatals:
        lines += ["| feed | kod | mesaj |", "|---|---|---|"]
        lines += [f"| {r['feed']} | `{r['fatal']}` | {r.get('fatal_message','')[:80]} |" for r in fatals[:25]]
    else:
        lines.append("Yok — hiçbir feed pipeline'ı durdurmadı.")

    lines += ["", "## ⚠️ Aşırı-tetikleme adayları (tek feed'de en yüksek)", "",
              "Bir feed'de binlerce kez çıkan kural = ya gerçek sistemik hata ya FP ya da",
              "agregasyon adayı (STM_014 emsali). Sıralama kanıt değil, **triyaj sinyali**.", "",
              "| kural | sınıf | önem | max/feed | feed | toplam | feed% |", "|---|---|---|---:|---|---:|---:|"]
    for r in sorted(rows, key=lambda x: -x["max_in_one_feed"])[:20]:
        lines.append(f"| {r['rule_id']} | {r['rule_class']} | {r['severity']} | {r['max_in_one_feed']:,} | "
                     f"{r['max_feed']} | {r['total_notices']:,} | {r['feeds_pct']} |")

    lines += ["", "## 🌱 Corpus'ta hiç tetiklenmeyen kurallar", "",
              "Ölü kural değil demek DEĞİL (dar kapsam/nadir profil olabilir) — ama",
              "emit-proof'u olmayanlar için kanıt boşluğu sinyali.", "",
              "```", ", ".join(never) if never else "(yok)", "```"]

    lines += ["", "## ⏱ En yavaş 10 feed", "", "| feed | sn | trip |", "|---|---:|---:|"]
    for r in slow:
        lines.append(f"| {r['feed']} | {r['elapsed_sec']} | {(r.get('feed_metrics') or {}).get('trip_count')} |")

    # ── MD kaba karşılaştırma (varsa) ──
    # DİKKAT: bu YALNIZCA kaba bir hacim kontrolü. Kural-bazlı gerçek parite
    # md_parity_audit.py'nin işi (MAP + AGG_RULES + MATCH/OVER/MISS/AGG).
    # MD skor üretmez → skor karşılaştırması ANLAMSIZ ([[feedback_md_no_scoring]]).
    md_rows = md_compare(args.dir, ok)
    if md_rows:
        write_csv(os.path.join(args.dir, "md_volume.csv"), md_rows)
        big = [r for r in md_rows if r["ratio"] and (r["ratio"] >= 3 or r["ratio"] <= 0.34)]
        lines += ["", "## 🔬 MD hacim kıyası (kaba — kural paritesi DEĞİL)", "",
                  f"MD raporu eşleşen feed: **{len(md_rows)}**; hacim sapması (≥3× veya ≤0.34×): **{len(big)}**.",
                  "",
                  "MD total = error+warning+info. Bizde fazlalık NORMAL (Quality/Analytics/JP/Flex",
                  "kurallarımızın MD karşılığı yok) — bu tablo **triyaj sinyali**, kanıt değil.",
                  "Gerçek parite: `python3 notgit/md_parity_audit.py <dir>` (kural eşlemeli).", "",
                  "| feed | biz | MD | oran | MD ver |", "|---|---:|---:|---:|---|"]
        for r in sorted(big, key=lambda x: -(x["ratio"] or 0))[:15]:
            lines.append(f"| {r['feed']} | {r['ours']:,} | {r['md_total']:,} | {r['ratio']}× | {r['md_version']} |")
        lines += ["", f"Parite için hazır: `{os.path.join(args.dir, 'pairs')}` "
                      f"(her feed = 1 MD raporu + 1 golden) → md_parity_audit.py", ""]
    else:
        lines += ["", "## MD karşılaştırması yok", "",
                  "`resolve` çalıştırılmadı ya da rapor inmedi → MD raporu eşleşmemiş.",
                  "Parite için: `corpus_batch.py resolve` (notgit/.md_token gerekir), sonra fetch.", ""]

    path = os.path.join(args.dir, "report.md")
    with open(path, "w") as f:
        f.write("\n".join(lines) + "\n")
    print(f"yazıldı: {path}")
    print(f"        {os.path.join(args.dir, 'rule_stats.csv')}")
    print(f"        {os.path.join(args.dir, 'fatals.csv')}")
    print(f"\nfeed={len(res)} fatal={len(fatals)} tetiklenen_kural={len(feeds_with)} hiç_çıkmayan={len(never)}")


def md_compare(d, ours):
    """MD raporu ile bizim toplam notice hacmini kıyasla + parite için pairs/ kur.

    md_parity_audit.py BASE_DIR/<feed>/ altında 1 MD raporu + 1 golden bekler →
    burada o düzeni symlink ile kuruyoruz (kopya değil: 250 feed'de disk israfı).
    """
    reports = os.path.join(d, "reports")
    results = os.path.join(d, "results")
    if not os.path.isdir(reports):
        return []
    pairs = os.path.join(d, "pairs")
    rows = []
    for r in ours:
        rp = os.path.join(reports, f"{r['feed']}.report.json")
        if not os.path.exists(rp):
            continue
        try:
            with open(rp) as f:
                md = json.load(f)
        except Exception:
            continue
        md_total = sum(n.get("totalNotices", 0) for n in md.get("notices", []))
        our_total = r["validation"]["notice_total_actual"]
        rows.append({
            "feed": r["feed"], "ours": our_total, "md_total": md_total,
            "ratio": round(our_total / md_total, 2) if md_total else None,
            "md_version": (md.get("summary") or {}).get("validatorVersion", "?"),
        })
        # parite aracı için ikili klasör
        pd_ = os.path.join(pairs, r["feed"])
        os.makedirs(pd_, exist_ok=True)
        for src, name in ((rp, "md_report.json"), (os.path.join(results, f"{r['feed']}.json"), "golden.json")):
            dst = os.path.join(pd_, name)
            if not os.path.exists(dst) and os.path.exists(src):
                os.symlink(os.path.relpath(src, pd_), dst)
    return rows


# ── yardımcılar ───────────────────────────────────────────────────────────────
class SkipTooBig(Exception):
    pass


# İndirme curl'e devredildi (urllib DEĞİL): python.org kurulumlarında CA bundle
# gelmiyor ve urllib CERTIFICATE_VERIFY_FAILED veriyor. curl sistemin sertifika
# deposunu kullanır, yönlendirme/timeout/retry'ı hazır çözer, yeni bağımlılık yok.
def curl(url, dest, max_mb=None, timeout=180):
    cmd = [
        "curl", "-sSL", "--fail", "-A", UA,
        "--max-time", str(timeout), "--retry", "2", "--retry-delay", "2",
        "-o", dest, url,
    ]
    if max_mb:  # curl akışta kendi keser → dev feed indirme boşa gitmez
        cmd += ["--max-filesize", str(int(max_mb * 1e6))]
    p = subprocess.run(cmd, capture_output=True)
    if p.returncode != 0:
        if os.path.exists(dest):
            os.remove(dest)
        err = p.stderr.decode()[:200].strip()
        # Boyut sınırı: curl boyutu ÖNCEDEN bilirse 63, akış sırasında keserse 56
        # döner. Yalnız 63'e bakmak "çok büyük"ü "ağ hatası" diye raporlar →
        # 250 feed'lik koşumda sahte arıza avı demek. Metin curl sürümüne göre
        # değişiyor ("Exceeded the maximum allowed file size" / "Maximum file size
        # exceeded") → gevşek eşleştir.
        if p.returncode == 63 or "file size" in err.lower():
            raise SkipTooBig(f">{max_mb}MB")
        raise RuntimeError(f"curl rc={p.returncode}: {err[:100]}")


def fetch_bytes(url, cache=None):
    if cache and os.path.exists(cache) and os.path.getsize(cache) > 0:
        with open(cache, "rb") as f:
            return f.read()
    tmp = (cache or os.path.join("/tmp", "corpus_dl")) + ".part"
    os.makedirs(os.path.dirname(tmp), exist_ok=True)
    curl(url, tmp, timeout=120)
    with open(tmp, "rb") as f:
        data = f.read()
    if cache:
        os.replace(tmp, cache)
    else:
        os.remove(tmp)
    return data


def download(url, dest, max_mb):
    tmp = dest + ".part"
    curl(url, tmp, max_mb=max_mb)
    size = os.path.getsize(tmp)
    with open(tmp, "rb") as f:
        if f.read(2) != b"PK":  # ZIP magic: HTML hata sayfası inmiş olabilir
            os.remove(tmp)
            raise ValueError("ZIP değil (muhtemelen hata sayfası)")
    os.replace(tmp, dest)
    return size


def parse_float(v):
    """API alanı boş/eksik olabilir → None (skip mantığı sessizce çökmesin)."""
    try:
        return float(v)
    except (TypeError, ValueError):
        return None


def read_plan(d):
    p = os.path.join(d, "plan.csv")
    if not os.path.exists(p):
        sys.exit(f"{p} yok — önce sample çalıştırın")
    with open(p) as f:
        return list(csv.DictReader(f))


def note(d, fn, row):
    with open(os.path.join(d, fn), "a", newline="") as f:
        csv.writer(f).writerow(row)


def write_csv(path, rows):
    if not rows:
        open(path, "w").close()
        return
    with open(path, "w", newline="") as f:
        w = csv.DictWriter(f, fieldnames=list(rows[0]))
        w.writeheader()
        w.writerows(rows)


def known_rule_ids():
    """RULES.md tablosundan kural ID'leri (registry'nin yayınlanmış hâli).
    rules_doc_drift testi bunun registry ile eşitliğini garanti ediyor."""
    ids = []
    with open(os.path.join(REPO, "RULES.md")) as f:
        for line in f:
            t = line.strip()
            if t.startswith("|"):
                first = t.strip("|").split("|")[0].strip()
                if "_" in first and first.split("_")[0].isupper():
                    ids.append(first)
    return ids


def main():
    ap = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    # --dir her alt komutta geçerli olsun (alt komuttan sonra da yazılabilsin) →
    # parent parser. Yalnız üst seviyede tanımlansa `sample --dir x` hata verirdi.
    common = argparse.ArgumentParser(add_help=False)
    common.add_argument("--dir", default=os.path.join(REPO, "notgit", "corpus"),
                        help="çalışma dizini (varsayılan notgit/corpus)")
    ap.add_argument("--dir", default=os.path.join(REPO, "notgit", "corpus"), help=argparse.SUPPRESS)
    sub = ap.add_subparsers(dest="cmd", required=True)

    s = sub.add_parser("sample", parents=[common], help="katalogdan katmanlı N feed seç")
    s.add_argument("--n", type=int, default=250)
    s.add_argument("--seed", type=int, default=42, help="tekrarlanabilirlik için sabit")
    s.add_argument("--per-country", type=int, default=8, help="ülke başına tavan (çeşitlilik)")
    s.set_defaults(fn=cmd_sample)

    rs = sub.add_parser("resolve", parents=[common],
                        help="MD API ile snapshot + rapor URL'i çöz (token gerekir)")
    rs.add_argument("--delay", type=float, default=0.2, help="API çağrıları arası saniye")
    rs.set_defaults(fn=cmd_resolve)

    f = sub.add_parser("fetch", parents=[common], help="zip + MD raporu indir (devam ettirilebilir)")
    f.add_argument("--max-mb", type=int, default=200, help="bundan büyük feed atlanır")
    f.add_argument("--delay", type=float, default=1.0, help="indirmeler arası saniye")
    f.set_defaults(fn=cmd_fetch)

    v = sub.add_parser("validate", parents=[common], help="bizim CLI ile koş (devam ettirilebilir)")
    v.add_argument("--today", required=True,
                   help="YYYYMMDD (sabit, tekrarlanabilir) VEYA 'md' = her feed kendi MD "
                        "raporunun doğrulama gününde koşulur (tarihe göreli kural pariteleri için ŞART)")
    v.add_argument("--timeout", type=int, default=900)
    v.add_argument("--only", help="virgülle ayrılmış feed id listesi — YALNIZ bunları koş "
                                  "(yeni feed eklerken korpusun geri kalanını kilitler)")
    v.set_defaults(fn=cmd_validate)

    r = sub.add_parser("report", parents=[common], help="rule_stats.csv + fatals.csv + report.md")
    r.set_defaults(fn=cmd_report)

    args = ap.parse_args()
    if getattr(args, "today", None) is not None and args.today != "md":
        if not re.fullmatch(r"\d{8}", str(args.today)):
            ap.error("--today: YYYYMMDD ya da 'md' olmalı")
        args.today = int(args.today)
    os.makedirs(args.dir, exist_ok=True)
    args.fn(args)


if __name__ == "__main__":
    main()
