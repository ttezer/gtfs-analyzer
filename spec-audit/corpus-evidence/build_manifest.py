#!/usr/bin/env python3
"""Korpus manifestini ÜRET — feed kimliği + SHA-256 + indirme URL'i.

⚠️ Manifest ELLE yazılmaz; bu betiğin çıktısıdır. 2026-08-07 dış denetimi manifestin
İKİ feed'inde `download_url` boş olduğunu ve `verify_corpus.py --download`'ın onları
sessizce atladığını gösterdi — yani üçüncü taraf 242/242'yi indiremiyordu.

URL kaynakları, sırayla:
  1. `catalog.csv` → `urls.latest` (MobilityData aynası; en kararlı)
  2. `catalog.csv` → `urls.direct_download` (yayıncının kendi adresi)
  3. `plan.csv`    → `zip_url` (katalog dışı, elle eklenmiş feed'ler)
  4. EXTRA_URLS    → yukarıdakilerin hiçbirinde olmayanlar, kaynağı yorumda yazılı

Kullanım:  python3 spec-audit/corpus-evidence/build_manifest.py [--zips <dizin>]
"""
import argparse, csv, hashlib, os, pathlib

HERE = pathlib.Path(__file__).resolve().parent
REPO = HERE.parent.parent

# Katalogda ve plan.csv'de bulunmayan feed'ler. Her satırın kaynağı YAZILI olmalı —
# kaynağı yazılamayan bir URL manifeste girmez.
EXTRA_URLS = {
    # mdb-767 (Prazska integrovana doprava) `urls.direct_download` alanı; zip adı bu
    # dosyadan geldiği için korpusta feed kimliği `PID_GTFS` olarak duruyor.
    "PID_GTFS": "http://data.pid.cz/PID_GTFS.zip",
}


def url_index(corpus: pathlib.Path) -> dict:
    urls = {}
    cat = corpus / "catalog.csv"
    if cat.exists():
        for r in csv.DictReader(cat.open(encoding="utf-8")):
            u = (r.get("urls.latest") or r.get("urls.direct_download") or "").strip()
            if u:
                urls[f"mdb-{r['mdb_source_id']}"] = u
    plan = corpus / "plan.csv"
    if plan.exists():
        for r in csv.DictReader(plan.open(encoding="utf-8")):
            u = (r.get("zip_url") or "").strip()
            if u:
                urls.setdefault(r["id"], u)
    urls.update(EXTRA_URLS)
    return urls


def main() -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument("--zips", default=str(REPO / "notgit/corpus/zips"))
    a = ap.parse_args()
    corpus = pathlib.Path(a.zips).parent
    urls = url_index(corpus)

    rows, missing = [], []
    for f in sorted(os.listdir(a.zips)):
        if not f.endswith(".zip"):
            continue
        p = os.path.join(a.zips, f)
        h = hashlib.sha256()
        with open(p, "rb") as fh:
            for chunk in iter(lambda: fh.read(1 << 20), b""):
                h.update(chunk)
        feed = f[:-4]
        u = urls.get(feed, "")
        if not u:
            missing.append(feed)
        rows.append({"feed": feed, "bytes": os.path.getsize(p),
                     "sha256": h.hexdigest(), "download_url": u})

    out = HERE / "corpus_manifest.csv"
    with out.open("w", newline="", encoding="utf-8") as fh:
        w = csv.DictWriter(fh, fieldnames=["feed", "bytes", "sha256", "download_url"])
        w.writeheader()
        w.writerows(rows)
    print(f"{out}: {len(rows)} feed · URL'li {len(rows)-len(missing)} · URL'siz {len(missing)}")
    if missing:
        print(f"🔴 URL'si OLMAYAN feed'ler — üçüncü taraf bunları indiremez: {missing}")
        print("  → EXTRA_URLS'e kaynağıyla birlikte ekleyin, yoksa kanıt zinciri eksiktir.")
        return 1
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
