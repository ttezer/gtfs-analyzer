#!/usr/bin/env python3
"""Korpus kanıtını ÜÇÜNCÜ TARAF olarak doğrula.

⚠️ NEDEN VAR: 2026-08-06 dış denetimi haklı olarak şunu söyledi — kanıt belgesindeki
"yeniden üretim" komutları `notgit/` altındaki dosyalara işaret ediyordu ve o klasör
`.gitignore`'da. Yani korpus rakamları dışarıdan doğrulanamıyordu, geliştirici beyanıydı.

Depoya ALINAN: feed kimlikleri + her zip'in SHA-256'sı + indirme URL'i + koşum çıktıları.
Depoya ALINMAYAN: zip'lerin kendisi (2,33 GB).

Kullanım:
    python3 spec-audit/corpus-evidence/verify_corpus.py --zips <dizin>       # hash doğrula
    python3 spec-audit/corpus-evidence/verify_corpus.py --download <dizin>   # eksikleri indir

Tam zincir:
    1) --download ile feed'leri çek (manifest'teki URL'lerden)
    2) --zips ile SHA-256'ları doğrula → bizim koştuğumuz BAYTLARIN aynısı mı
    3) python3 spec-audit/corpus_batch.py validate --dir <dizin> --today 20260717
    4) python3 spec-audit/corpus_batch.py report --dir <dizin>
    5) üretilen rule_stats.csv'yi corpus-evidence/rule_stats.csv ile karşılaştır

⚠️ Feed'ler CANLI URL'lerden gelir; yayıncı dosyayı güncellerse hash TUTMAZ. Bu bir
başarısızlık değil, korpusun bir ANLIK GÖRÜNTÜ olduğunun kanıtıdır — tutmayan feed'ler
ayrı raporlanır ve "bozuk" sayılmaz.
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
        for r in rows:
            dst = os.path.join(a.download, r["feed"] + ".zip")
            if os.path.exists(dst):
                continue
            if not r["download_url"]:
                miss += 1
                continue
            rc = subprocess.run(["curl", "-sSL", "--max-time", "180", "-o", dst,
                                 r["download_url"]]).returncode
            got += (rc == 0)
        print(f"indirilen {got} · URL'siz {miss}")
        return 0

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
