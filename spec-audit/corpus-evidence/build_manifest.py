#!/usr/bin/env python3
"""BUILD the corpus manifest — feed ID + SHA-256 + download URL.

⚠️ Do not write the manifest MANUALLY; it is this script's output. The 2026-08-07 external audit
showed that `download_url` was empty for TWO feeds and `verify_corpus.py --download` skipped them
silently — so a third party could not download all 242/242.

URL kaynakları, sırayla:
  1. `catalog.csv` → `urls.latest` (MobilityData aynası; en kararlı)
  2. `catalog.csv` → `urls.direct_download` (yayıncının kendi adresi)
  3. `plan.csv`    → `zip_url` (katalog dışı, elle eklenmiş feed'ler)
  4. EXTRA_URLS    → yukarıdakilerin hiçbirinde olmayanlar, kaynağı yorumda yazılı

Usage:  python3 spec-audit/corpus-evidence/build_manifest.py [--zips <directory>]
"""
import argparse, csv, hashlib, os, pathlib

HERE = pathlib.Path(__file__).resolve().parent
REPO = HERE.parent.parent

# Feeds absent from the catalogue and plan.csv. The source of every row must be WRITTEN DOWN —
# a URL whose source cannot be documented does not enter the manifest.
EXTRA_URLS = {
    # `urls.direct_download` for mdb-767 (Prazska integrovana doprava); because the ZIP name
    # came from this file, the feed ID remains `PID_GTFS` in the corpus.
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

    out = HERE / "corpus_manifest.csv"
    # URL STATUS IS MEASURED, NOT DERIVED: this script inspects ZIPs, not the network. Preserve
    # existing `url_status`/`url_checked` values; otherwise regeneration would silently erase
    # the 14 dead-URL markers measured on 2026-08-12.
    prior = {}
    if out.exists():
        for r in csv.DictReader(out.open(encoding="utf-8")):
            prior[r["feed"]] = (r.get("url_status", ""), r.get("url_checked", ""))

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
        status, checked = prior.get(feed, ("", ""))
        rows.append({"feed": feed, "bytes": os.path.getsize(p),
                     "sha256": h.hexdigest(), "download_url": u,
                     "url_status": status, "url_checked": checked})

    with out.open("w", newline="", encoding="utf-8") as fh:
        # lineterminator: CSV defaults to CRLF; the repository manifest uses LF, and a line-ending
        # difference would put the entire file into the diff.
        w = csv.DictWriter(fh, lineterminator="\n",
                           fieldnames=["feed", "bytes", "sha256", "download_url",
                                       "url_status", "url_checked"])
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
