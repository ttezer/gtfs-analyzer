#!/usr/bin/env python3
"""Faz 0 + Faz 1 — sessiz kuralların ULAŞILABİLİRLİK ve SEBEP triyajı.

Faz 0 (ölü kural): registry'de ilan edilen her kural, üretim kodunda (test modülleri
HARİÇ) TIRNAK İÇİNDE geçiyor mu? Mevcut CI kapısı `crates/rules/tests/emit_coverage.rs`
aynı soruyu sorar ama TOKEN tarar — bir kural id'si yorum satırında geçse "canlı"
sayılır. Bu tarayıcı daha sıkıdır; farkı raporlar.

Faz 1 (sessizlik sebebi): korpusta hiç çıkmayan kurallar neden susuyor? Etiketler
KANIT DURUMUNU belgeler; hiçbiri "kuralı kaldır" demez. Otorite spec'tir, korpus değil.

⚠️ Ölçüm tuzakları bu betikte bilinçli olarak kapalı:
  1. Zip taramasında `.txt` filtresi `locations.geojson`'ı gizler (2026-08-14'te
     6 feed "0" olarak raporlandı).
  2. `fatals.csv` BAYAT — bkz. aşağıdaki FATAL_RULES notu. Bir zamanlar buradan
     "ARC_004 ateşledi" sonucu çıkarılmıştı; o fatal yolu artık YOK.
  3. Bir `FatalCode` varyantının TANIMLI olması ÜRETİLDİĞİ anlamına gelmez.
"""
import csv
import os
import re
import sys
import zipfile
from collections import Counter
from concurrent.futures import ThreadPoolExecutor

ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
REGISTRY = os.path.join(ROOT, "crates/rules/src/registry.rs")
PIPELINE = os.path.join(ROOT, "crates/pipeline/src")
ZIPS = os.path.join(ROOT, "notgit/corpus/zips")
EV = os.path.join(ROOT, "spec-audit/corpus-evidence")

RULE_RE = re.compile(r'"([A-Z]{2,4}_\d{3})"')
BARE_RE = re.compile(r'\b([A-Z]{2,4}_\d{3})\b')

# Fatal yolla üretilen kurallar Notice değildir; rule_id kaynakta literal geçmez ve
# rule_stats.csv'ye girmez.
#
# 🔴 BU HARİTA 2026-08-15'te DARALTILDI. İlk hâli `core/src/enums.rs`'teki `FatalCode`
# YORUMLARINDAN türetilmişti ve beş kural içeriyordu. Ölçüm gösterdi ki enum'un yedi
# varyantından DÖRDÜ üretim kodunda HİÇ KURULMUYOR: `Utf8Critical`, `NoRequiredFiles`,
# `CsvMalformed`, `ResourceLimit`. Yani ARC_002/ARC_004/ARC_013 fatal DEĞİL NOTICE
# üretiyor (üçü de `real_feed_mutation.rs`'te gerçek feed mutasyonuyla doğrulandı).
#
# ⚠️ Bir enum varyantının VARLIĞI onun ÜRETİLDİĞİ anlamına gelmez — `AGN_001` (ölü
# registry kaydı) ve #125'in ulaşılamaz K7 dalıyla aynı sınıf.
FATAL_RULES = {
    "ARC_001": "ZipUnreadable",
    "ARC_029": "DecompressionLimit",
}

# 🔴 `corpus-evidence/fatals.csv` ARTIK KANIT OLARAK KULLANILMIYOR. O dosya 2026-08-06
# koşumundan `NoRequiredFiles` fatal'ini 4 feed'de kaydeder, ama `66e009c2`
# ("recover and report partial feeds", 2026-08-09) o yolu KALDIRDI: bugün zorunlu
# dosyalar eksikken Fatal yok, `ARC_004` notice olarak çıkıyor. Bayat bir dosyadan
# "ateşledi" sonucu çıkarmak, ölçüm gibi görünen bir tahmindir.

# k2 modül adı → GTFS dosyası (modül-başına-dosya tasarımı). Dosyaya karşılık
# gelmeyen yardımcı modüller dışarıda.
K2_NOT_A_FILE = {"common", "mod", "bcp47_grandfathered", "iso4217_generated",
                 "agency_jp", "office_jp"}


def strip_test_modules(src: str) -> str:
    """`#[cfg(test)]` bloklarını sil; brace sayarken string ve yorumu atla."""
    out, i = [], 0
    while True:
        m = re.compile(r"#\[cfg\(test\)\]").search(src, i)
        if not m:
            out.append(src[i:])
            return "".join(out)
        out.append(src[i:m.start()])
        j = src.find("{", m.end())
        if j < 0:
            return "".join(out)
        depth, k, n = 0, j, len(src)
        while k < n:
            c = src[k]
            if c == '"':
                k += 1
                while k < n and src[k] != '"':
                    k += 2 if src[k] == "\\" else 1
            elif c == "/" and k + 1 < n and src[k + 1] == "/":
                nl = src.find("\n", k)
                if nl < 0:
                    break
                k = nl
            elif c == "{":
                depth += 1
            elif c == "}":
                depth -= 1
                if depth == 0:
                    k += 1
                    break
            k += 1
        i = k


def strip_line_comments(src: str) -> str:
    """`//` yorumlarını sil (string içindeki `//` korunur)."""
    out, i, n = [], 0, len(src)
    while i < n:
        c = src[i]
        if c == '"':
            j = i + 1
            while j < n and src[j] != '"':
                j += 2 if src[j] == "\\" else 1
            out.append(src[i:j + 1])
            i = j + 1
        elif c == "/" and i + 1 < n and src[i + 1] == "/":
            nl = src.find("\n", i)
            i = n if nl < 0 else nl
        else:
            out.append(c)
            i += 1
    return "".join(out)


def scan_pipeline():
    """rule_id -> (tırnaklı emit sayısı, yalnız-yorum mu, k2 dosyaları, modüller)."""
    quoted, bare, mods = Counter(), Counter(), {}
    for dirpath, _, names in os.walk(PIPELINE):
        for name in sorted(names):
            if not name.endswith(".rs"):
                continue
            path = os.path.join(dirpath, name)
            raw = strip_test_modules(open(path, encoding="utf-8").read())
            code = strip_line_comments(raw)
            rel = os.path.relpath(path, ROOT)
            for rid in RULE_RE.findall(code):
                quoted[rid] += 1
                mods.setdefault(rid, set()).add(rel)
            for rid in BARE_RE.findall(raw):
                bare[rid] += 1
    return quoted, bare, mods


def k2_files_for(rid, mods):
    """Kuralın DOĞRUDAN dosya çapası: k2 modül adı → <ad>.txt."""
    files = set()
    for rel in mods.get(rid, ()):
        base = os.path.basename(rel)[:-3]
        if "/k2/" in rel.replace(os.sep, "/") and base not in K2_NOT_A_FILE:
            files.add("locations.geojson" if base == "locations"
                      else f"{base}.txt")
    return sorted(files)


def group_home_files(meta, mods):
    """Grup öneki → o grubun EV dosyası, mekanik türetilir.

    `SAR_*` gibi grupların emit'i k4'te durabilir; o zaman kuralın kendi modülü
    dosya söylemez. Ama grubun kuralları topluca bakıldığında çoğunluğun geçtiği
    k2 modülü grubun ev dosyasıdır. Elle tablo yazmak yerine sayarak bulunur —
    elle yazılan liste bayatlar.
    """
    votes = {}
    for rid in meta:
        grp = rid.split("_")[0]
        for f in k2_files_for(rid, mods):
            votes.setdefault(grp, Counter())[f] += 1
    return {g: c.most_common(1)[0][0] for g, c in votes.items()}


def corpus_file_presence():
    if not os.path.isdir(ZIPS):
        return None, 0
    feeds = sorted(f for f in os.listdir(ZIPS) if f.endswith(".zip"))

    def names(f):
        try:
            with zipfile.ZipFile(os.path.join(ZIPS, f)) as z:
                return {os.path.basename(n) for n in z.namelist()
                        if n.lower().endswith((".txt", ".geojson"))}
        except Exception:
            return set()

    c = Counter()
    with ThreadPoolExecutor(16) as ex:
        for s in ex.map(names, feeds):
            c.update(s)
    return c, len(feeds)


def registry_meta():
    s = open(REGISTRY, encoding="utf-8").read()
    meta = {}
    for m in re.finditer(
            r'r!\(\s*"([A-Z]{2,4}_\d{3})"\s*,\s*(\w+)\s*,\s*(\w+)\s*,'
            r'(.{0,500}?)\),\s*\n', s, re.S):
        msgs = re.findall(r'"([^"]{6,})"', m.group(4))
        meta[m.group(1)] = {
            "severity": m.group(2), "class": m.group(3),
            "message": msgs[-1] if msgs else "",
        }
    return meta


REQUIREMENT_RE = re.compile(r"eksik|bulunamad|boş|yok\b|zorunlu", re.I)


# rule_stats.csv'nin VERİSİ 2026-08-06 koşumundandır (dosyanın commit tarihi değil —
# "bir baseline'ın dosya tarihi, verisinin tarihi değildir"). Bu tarihten sonra
# eklenen kurallar o koşumda YOKTU; "hiç ateşlemedi" onlar için bir ÖLÇÜM DEĞİL,
# ölçüm YOKLUĞUDUR. Kanıt: RTS_030 burada "şüpheli" görünür, oysa eklenirken 19
# feed'de ateşlediği ölçülmüştü.
CORPUS_RUN_DATE = "2026-08-06"


def rules_added_after_baseline():
    import subprocess
    try:
        out = subprocess.run(
            ["git", "log", f"--since={CORPUS_RUN_DATE}", "-p", "--",
             "crates/rules/src/registry.rs"],
            cwd=ROOT, capture_output=True, text=True, check=True).stdout
    except Exception:
        return set()
    return {m.group(1) for m in
            re.finditer(r'^\+\s+r!\(\s*"([A-Z]{2,4}_\d{3})"', out, re.M)}


def carried_evidence():
    """Önceki tsv'deki `evidence` sütununu taşır.

    Bu sütun ÖLÇÜMDÜR, türetilemez: mutasyon testinin sonucudur (issue #127). Betik her
    koşuşunda tabloyu yeniden ürettiği için, taşınmazsa kanıt sessizce silinirdi —
    `build_manifest.py`'nin `url_status`'ü taşıması ile aynı sebep.
    """
    path = os.path.join(ROOT, "spec-audit/silent_rules.tsv")
    if not os.path.exists(path):
        return {}
    with open(path, newline="") as fh:
        return {r["rule_id"]: r.get("evidence", "")
                for r in csv.DictReader(fh, delimiter="\t")
                if r.get("evidence")}


def suppression_map():
    """kural -> onu bastırabilen KÖK kurallar (registry `blocks` alanı).

    ⚠️ Bu YALNIZ registry'de ilan edilen kaskadı modeller. İkinci bir mekanizma
    daha var ve burada ÖLÇÜLMÜYOR: K7'nin whitespace-türev bastırması (#112),
    `whitespace_derived` bayrağıyla çalışır ve registry'de görünmez. Canlı örnek
    `mdb-2691`: `stop_lon=" -4.732529"` STP_005'i DOĞURUYOR, DQ_016 kökü onu
    bastırıyor ve bulgu istatistiğe girmiyor. Yani `suppressed_by` boş olması
    "bastırılmadı" DEMEK DEĞİLDİR.
    """
    s = open(REGISTRY, encoding="utf-8").read()
    victims = {}
    for m in re.finditer(
            r'r!\(\s*"([A-Z]{2,4}_\d{3})"\s*,\s*\w+\s*,\s*\w+\s*,\s*\d+\s*,'
            r'\s*&\[([^\]]*)\]', s):
        for vic in re.findall(r'"([A-Z]{2,4}_\d{3})"', m.group(2)):
            victims.setdefault(vic, []).append(m.group(1))
    return victims


def main():
    meta = registry_meta()
    quoted, bare, mods = scan_pipeline()
    presence, nfeeds = corpus_file_presence()

    fired = {r["rule_id"] for r in csv.DictReader(open(f"{EV}/rule_stats.csv"))}

    # ---------- Faz 0 ----------
    print("=== FAZ 0 · ulaşılabilirlik ===")
    no_quote = sorted(r for r in meta if quoted.get(r, 0) == 0)
    truly_dead, fatal_only, comment_only = [], [], []
    for r in no_quote:
        if r in FATAL_RULES:
            fatal_only.append(r)
        elif bare.get(r, 0) > 0:
            comment_only.append(r)
        else:
            truly_dead.append(r)
    print(f"  registry {len(meta)} kural · tırnaklı emit noktası yok: {len(no_quote)}")
    print(f"    fatal yolla üretilen (ölü DEĞİL): {fatal_only}")
    print(f"    YALNIZ yorum satırında geçen    : {comment_only}")
    print(f"    hiçbir yerde geçmeyen (ÖLÜ)     : {truly_dead}")
    bad = [r for r in truly_dead if r in fired]
    if bad:
        print(f"  🔴 TARAYICI HATASI: {bad} ateşlemiş ama emit bulunamadı")

    # ---------- Faz 1 ----------
    home = group_home_files(meta, mods)
    victims = suppression_map()
    evidence = carried_evidence()
    post_baseline = rules_added_after_baseline()
    silent = sorted(r for r in meta if r not in fired)
    rows = []
    for r in silent:
        files = k2_files_for(r, mods)
        anchor = "modül"
        if not files:                     # emit k4/k6'da → grubun ev dosyası
            hf = home.get(r.split("_")[0])
            if hf:
                files, anchor = [hf], "grup"
        best, nf = "", ""
        if files and presence:
            best = max(files, key=lambda f: presence.get(f, 0))
            nf = presence.get(best, 0)
        is_req = bool(REQUIREMENT_RE.search(meta[r]["message"]))
        if r in truly_dead:
            label = "DEAD"
        elif r in post_baseline:
            label = "POST_BASELINE"
        elif r in FATAL_RULES:
            label = "FATAL_PATH"
        elif not files:
            label = "CROSS_FILE"
        elif nf == 0:
            label = "NO_FEATURE"
        elif nf <= 5:
            label = "THIN_FEATURE"
        elif is_req:
            label = "SELECTION_BIAS"
        else:
            label = "PREDICATE_SUSPECT"
        rows.append({
            "rule_id": r, "evidence": evidence.get(r, ""),
            "severity": meta[r]["severity"],
            "class": meta[r]["class"], "label": label,
            "gtfs_file": best, "corpus_feeds_with_file": nf,
            "file_anchor": anchor if files else "",
            "suppressed_by": ",".join(sorted(victims.get(r, []))),
            "suppressor_fired": "yes" if any(v in fired for v in victims.get(r, []))
                                else ("no" if r in victims else ""),
            "emit_sites": quoted.get(r, 0),
            "message": meta[r]["message"],
        })

    out = os.path.join(ROOT, "spec-audit/silent_rules.tsv")
    with open(out, "w", newline="") as fh:
        w = csv.DictWriter(fh, fieldnames=list(rows[0].keys()),
                           delimiter="\t", lineterminator="\n")
        w.writeheader()
        w.writerows(rows)

    print(f"\n=== FAZ 1 · sessizlik sebebi === ({nfeeds} feed tarandı)")
    print(f"  korpusta hiç çıkmayan: {len(rows)} / {len(meta)}")
    gate = [x for x in rows if (x["severity"], x["class"]) == ("Kritik", "Spec")]
    print(f"  {'etiket':20} {'hepsi':>6} {'Kritik∧Spec':>12}")
    g = Counter(x["label"] for x in gate)
    for k, v in Counter(x["label"] for x in rows).most_common():
        print(f"  {k:20} {v:>6} {g.get(k,0):>12}")
    print(f"  {'TOPLAM':20} {len(rows):>6} {len(gate):>12}")
    ev = [x for x in rows if x["evidence"]]
    print(f"\n  ✅ MUTASYONLA ÖLÇÜLMÜŞ (etiket artık sezgisel değil): {len(ev)}")
    for x in ev:
        print(f"     {x['rule_id']:8} {x['evidence']}")
    amb = [x for x in rows if x["suppressor_fired"] == "yes"]
    print(f"\n  ⚠️ sessizliği BELİRSİZ (bastıran kök korpusta ateşledi): {len(amb)}")
    print("     bunlar 'hiç doğmadı' ile 'doğdu ve bastırıldı' arasında ayrılmamıştır")
    print(f"\n  → {out}")
    return 0


if __name__ == "__main__":
    sys.exit(main())
