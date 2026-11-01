#!/usr/bin/env python3
"""Verify the corpus evidence as a THIRD PARTY.

⚠️ WHY THIS EXISTS: the 2026-08-06 external audit correctly noted that the evidence document's
"reproduction" commands pointed to files under `notgit/`, and that directory is in `.gitignore`.
The corpus numbers therefore could not be verified externally; they were a developer assertion.

IN THE REPOSITORY: feed IDs + each ZIP's SHA-256 + download URL + the URL's last measured
status (`url_status`/`url_checked`) + run outputs.
NOT IN THE REPOSITORY: the ZIPs themselves (2.33 GB).

Usage:
    python3 spec-audit/corpus-evidence/verify_corpus.py --zips <directory>       # verify hashes
    python3 spec-audit/corpus-evidence/verify_corpus.py --download <directory>   # download missing files

Full chain:
    1) fetch feeds with --download (from the manifest URLs)
    2) verify SHA-256 values with --zips → are these the same BYTES we ran?
    3) python3 spec-audit/corpus_batch.py validate --dir <dizin> --today 20260717
    4) python3 spec-audit/corpus_batch.py report --dir <dizin>
    5) compare the generated rule_stats.csv with corpus-evidence/rule_stats.csv

⚠️ Feeds come from LIVE URLs; the hash will not match if a publisher updates the file. This is
not a failure, but evidence that the corpus is a SNAPSHOT — mismatching feeds are reported
separately and are not considered "broken".

⚠️ A DEAD URL is a separate fact and MUST NOT BE CONFUSED with hash drift (measured for 14 feeds
on 2026-08-12). A feed with a dead URL WAS in the corpus, entered the measurement, and its sha256
remains in the manifest; deleting the row retroactively changes the measured set and destroys byte identity.
Therefore the row REMAINS, with only its `url_status` field changed to `dead_404`.
"""
import argparse, csv, hashlib, os, subprocess, sys

HERE = os.path.dirname(os.path.abspath(__file__))
MANIFEST = os.path.join(HERE, "corpus_manifest.csv")


def sha256(path):
    h = hashlib.sha256()
    with open(path, "rb") as f:
        for chunk in iter(lambda: f.read(1 << 20), b""):
            h.update(chunk)
    return h.hexdigest()


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--zips", help="zip dizini — hash doğrula")
    ap.add_argument("--download", help="zip dizini — eksikleri indir")
    a = ap.parse_args()
    rows = list(csv.DictReader(open(MANIFEST, encoding="utf-8")))
    print(f"manifest: {len(rows)} feed")

    if a.download:
        os.makedirs(a.download, exist_ok=True)
        got = miss = 0
        dead, failed = [], []
        for r in rows:
            dst = os.path.join(a.download, r["feed"] + ".zip")
            if os.path.exists(dst):
                continue
            if not r["download_url"]:
                miss += 1
                continue  # the manifest generator already reports this with exit 1
            if r.get("url_status") == "dead_404":
                dead.append(r["feed"])
                continue
            # ⚠️ `-f` is REQUIRED. The 2026-08-12 measurement showed that without `-f`, curl
            # also returned EXIT 0 for a 404 and wrote the error body (92-byte JSON) to disk as `.zip`.
            # That fake ZIP then entered hash verification and was reported as "publisher updated — NOT an error";
            # a dead URL therefore looked like a refreshed feed.
            rc = subprocess.run(["curl", "-fsSL", "--max-time", "180", "-o", dst,
                                 r["download_url"]]).returncode
            if rc == 0:
                got += 1
            else:
                failed.append(r["feed"])
                if os.path.exists(dst):
                    os.remove(dst)  # do not let a partial/invalid body mislead hash verification
        print(f"indirilen {got} · URL'siz {miss} · manifeste göre ölü {len(dead)} · "
              f"indirilemeyen {len(failed)}")
        if miss:
            # 2026-08-07 audit: feeds without URLs were skipped SILENTLY → a third party could not
            # download all 242/242 but did not notice. This is now a loud failure.
            print(f"🔴 {miss} feed'in URL'i YOK — bu manifestle tam korpus kurulamaz.")
        if dead:
            print(f"🔴 {len(dead)} feed'in URL'i {rows[0].get('url_checked', '?')} "
                  f"itibarıyla 404: {', '.join(dead)}")
            print("  → Bu feed'ler korpusta VARDI ve ölçüme girdi; sha256'ları manifestte "
                  "durur. URL'in ölmesi ölçümü geçersiz kılmaz, YENİDEN KURULMASINI engeller.")
        if failed:
            print(f"🔴 manifestte 'ok' işaretli ama İNMEYEN {len(failed)} feed: "
                  f"{', '.join(failed)}")
            print("  → url_status bayat olabilir; yeniden ölçüp manifesti güncelleyin.")
        return 1 if (miss or dead or failed) else 0

    if not a.zips:
        ap.print_help()
        return 2

    ok = drift = absent = 0
    drifted = []
    for r in rows:
        p = os.path.join(a.zips, r["feed"] + ".zip")
        if not os.path.exists(p):
            absent += 1
            continue
        if sha256(p) == r["sha256"]:
            ok += 1
        else:
            drift += 1
            drifted.append(r["feed"])
    print(f"\nBİREBİR AYNI: {ok} · DEĞİŞMİŞ: {drift} · YEREL'DE YOK: {absent}")
    if drifted:
        print("değişenler (yayıncı güncellemiş — hata DEĞİL):")
        print("  " + ", ".join(drifted[:20]) + (" …" if len(drifted) > 20 else ""))
    print("\n⚠️ Yalnız BİREBİR AYNI olan feed'ler için üretilen sayılar bizimkiyle "
          "karşılaştırılabilir.")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
