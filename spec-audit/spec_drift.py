#!/usr/bin/env python3
"""Spec sürüklenmesi (drift) tespiti — issue #73.

Katalog CANLI bir sayfadan üretiliyor. gtfs.org bir revizyon yayımlarsa 299 adayımız
sessizce ESKİ metni anlatır ve `EVIDENCE_BASE.md`'deki yüzde artık güncel spec'e ait
olmaz. Bunu bugün hiçbir şey tespit etmiyor: `extract_provisions.py` elle koşuyor,
CI'da yalnız `badge_status.py` var ve o, defteri ELİMİZDEKİ kataloğa karşı denetliyor —
kataloğu spec'e karşı DEĞİL.

## 🔴 Neden ham SHA-256 karşılaştırması TEK BAŞINA yetmez

Issue'nun ilk önerisi "sayfanın SHA-256'sını `_source.sha256` ile karşılaştır, farklıysa
issue aç" idi. 2026-08-07'de ÖLÇÜLDÜ: sayfanın sha256'sı ZATEN farklıydı (295.559 →
295.827 bayt) ama katalog **birebir aynı** çıktı — 299 adayın 299'u, aynı kimlikler, aynı
`spec_revision`. Fark site iskeletindeydi (mkdocs yeniden derlemesi), normatif metinde
değil. Ham sha kapısı ilk koşusunda yanlış alarm verirdi ve issue'nun kendi gerekçesi
("insanları görmezden gelmeye alıştırır") tam da bunu yasaklıyor.

Bu yüzden üç ayrı sinyal ölçülür ve AYRI raporlanır:

  `same`      → sayfa baytları da katalog da aynı. Sessiz.
  `cosmetic`  → sayfa baytları değişmiş, KATALOG AYNI. Haber DEĞİL; issue AÇILMAZ.
                🔴 **BEKLENEN DURUM BUDUR.** Sayfa her yanıtta değişir (aşağıya bak).
  `catalogue` → `provisions_sha256` değişmiş (aday eklendi/kayboldu ya da `spec_revision`).
                HABER → issue.
  `upstream`  → katalog aynı ama `reference.md`'ye yeni commit girmiş; sayfa henüz
                yeniden derlenmemiş olabilir → HABER, ayrı sınıf.

## 🔴 Sayfanın SHA-256'sı YENİDEN ÜRETİLEMEZ (2026-08-07 ölçümü)

Sayfa arka arkaya üç kez indirildi, üç FARKLI özet çıktı. Fark yalnız Cloudflare'in
enjekte ettiği İKİ satırdı: `data-cfemail` token'ı ve `__CF$cv$params` ray id'si.
Yani ham sha karşılaştırması her koşuda tetiklenir — kapı olarak DEĞERSİZDİR ve issue
açsaydı haftada bir yanlış alarm üretirdi.

Kararın dayandığı çapa `_source.provisions_sha256`: adaylardan hesaplanır, site
iskeletinden bağımsızdır ve iki AYRI indirmede AYNI çıktığı ölçülmüştür.

## 🔴 gtfs.org CI'dan ALINAMIYOR — sinyal ikiye ayrıldı (2026-08-07 ölçümü)

İlk koşum CI'da **HTTP 403** aldı: gtfs.org Cloudflare arkasında ve barındırılan koşucu
IP'lerini engelliyor (tarayıcı User-Agent'ı da yetmedi). Yani KATALOG sinyali CI'dan
ölçülemez — bu bir hata değil, ortamın sınırı.

Ama sürüklenmenin asıl sorusu "spec metni değişti mi", ve o soru CI'dan YANITLANABİLİR:
`reference.md`'ye dokunan en yeni commit GitHub API'sinden okunur. Bu yüzden üç mod var:

  `page`     → sayfayı indirir, KATALOĞU yeniden üretip karşılaştırır (yerel/geliştirici)
  `upstream` → yalnız commit karşılaştırır (CI'dan çalışır)
  `auto`     → sayfayı dener; alınamazsa upstream'e DÜŞER **ve bunu açıkça yazar**

⚠️ Düşüş SESSİZ DEĞİLDİR: rapor hangi sinyalin ölçülMEDİĞİNİ ve nedenini basar, issue
gövdesine de girer. Sessiz düşüş, yeşil görünen kör bir denetim demek olurdu.

## Çıkış kodları — "sürüklenme yok" ile "bakılamadı" AYRI

  0 → EN AZ BİR sinyal ölçüldü (sonuç `same`/`cosmetic`/`catalogue`/`upstream`)
  2 → HİÇBİR sinyal ölçülemedi (ne sayfa ne commit). İş kırmızı olur ve bu BİLİNÇLİDİR:
      sessizce yeşil kalan bir denetim, denetim değildir (`zip_peek.py` ile aynı ders —
      "dosya yok" ile "bakılamadı" karıştırılmaz).

Sürüklenmenin kendisi build'i KIRMAZ: spec revizyonu bu depoda bir regresyon değil,
haberdir.

Çalıştır:
    python3 spec-audit/spec_drift.py                 # auto (sayfa → gerekirse upstream)
    python3 spec-audit/spec_drift.py spec.html       # yerel kopyadan (test)
    python3 spec-audit/spec_drift.py --mode upstream # yalnız commit (CI bunu koşar)
    python3 spec-audit/spec_drift.py --baseline X.json spec.html
"""
import hashlib
import importlib.util
import json
import os
import pathlib
import subprocess
import sys
import tempfile

HERE = pathlib.Path(__file__).resolve().parent
BASELINE = HERE / "spec_provisions.json"
SPEC_URL = "https://gtfs.org/documentation/schedule/reference/"


def load_extractor():
    """`extract_provisions.py`'yi modül olarak yükle — cümle bölme mantığı TEK yerde
    kalsın. Kopyalamak, iki tarafın sessizce ayrışması demek olurdu."""
    spec = importlib.util.spec_from_file_location("extract_provisions", HERE / "extract_provisions.py")
    mod = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(mod)
    return mod


def fetch(url: str) -> str:
    """Spec sayfasını indir ve GERÇEKTEN spec olduğunu doğrula.

    ⚠️ `urllib` DEĞİL `curl` — geliştirme makinesinde Python'un sertifika deposu yok.
    ⚠️ Tarayıcı User-Agent'ı: gtfs.org Cloudflare arkasında ve varsayılan `curl/*` UA'sı
    ile CI koşucularına ara sayfa (bot koruması) dönebiliyor. Böyle bir yanıt "sayfa
    alındı" gibi görünür ama içinde tek bir hüküm yoktur → kataloğu 0 adaya düşürür.
    Bu yüzden yanıt İÇERİK olarak doğrulanır ve sorun ayrı bir mesajla raporlanır:
    sessizce "sürüklenme yok" demek, denetimin en kötü hâli olurdu.
    """
    ua = ("Mozilla/5.0 (X11; Linux x86_64) AppleWebKit/537.36 (KHTML, like Gecko) "
          "Chrome/126.0 Safari/537.36 (+gtfs-analyzer spec-drift)")
    p = subprocess.run(
        ["curl", "-sSL", "--max-time", "90", "-A", ua,
         "-H", "Accept: text/html,application/xhtml+xml", "-w", "\n%{http_code}", url],
        capture_output=True,
    )
    if p.returncode != 0:
        raise RuntimeError(f"curl hatası: {p.stderr.decode()[:160]}")
    body, _, code = p.stdout.decode("utf-8", "replace").rpartition("\n")
    if code.strip() != "200":
        raise RuntimeError(f"HTTP {code.strip()} — sayfa alınamadı ({len(body)} bayt)")
    # Spec sayfasının değişmez çapası. Yoksa elimizdeki şey spec DEĞİL (ara sayfa,
    # hata sayfası, yönlendirme gövdesi…).
    if "field-definitions" not in body or "Revised" not in body:
        head = " ".join(body[:200].split())
        raise RuntimeError(
            f"gelen belge spec sayfası değil ({len(body)} bayt) — bot koruması ya da "
            f"yönlendirme olabilir. Baş: {head!r}")
    return body


def catalogue_of(mod, doc: str) -> dict:
    """Belgeden kataloğu ÜRET ama diske yazma — `extract_provisions.main()` dosyaya
    yazar, o yüzden geçici bir dizine yönlendirilir."""
    with tempfile.TemporaryDirectory() as tmp:
        out = pathlib.Path(tmp) / "spec_provisions.json"
        saved_out_path, real_argv = mod.OUT, sys.argv
        mod.OUT = out
        # Katalog karşılaştırması commit çözmeyi GEREKTİRMEZ; ağ isteğini burada
        # yapmamak, ölçümü GitHub API kotasından bağımsız kılar.
        sys.argv = ["extract_provisions", "--no-upstream", "-"]
        try:
            mod.fetch = lambda argv: doc  # noqa: ARG005 — belge zaten elimizde
            # Üreticinin ilerleme satırları ve `--no-upstream` uyarısı bu bağlamda
            # BEKLENEN çıktıdır (geçici katalog kasten pinlenmez) — raporu kirletmesin.
            with open(os.devnull, "w", encoding="utf-8") as null:
                saved_stdout, saved_stderr = sys.stdout, sys.stderr
                sys.stdout = sys.stderr = null
                try:
                    rc = mod.main()
                finally:
                    sys.stdout, sys.stderr = saved_stdout, saved_stderr
            if rc != 0:
                raise RuntimeError("extract_provisions.main() hata döndü")
            return json.loads(out.read_text(encoding="utf-8"))
        finally:
            mod.OUT, sys.argv = saved_out_path, real_argv


def classify(base: dict, live_doc: str | None, live_cat: dict | None,
             live_upstream: str | None, page_error: str | None) -> dict:
    b_src = base["_source"]
    l_src = live_cat["_source"] if live_cat else {}
    page_same = (live_doc is not None
                 and b_src["sha256"] == hashlib.sha256(live_doc.encode("utf-8")).hexdigest())

    b_ids = {p["id"] for p in base["provisions"]}
    l_ids = {p["id"] for p in live_cat["provisions"]} if live_cat else set()
    added = sorted(l_ids - b_ids) if live_cat else []
    gone = sorted(b_ids - l_ids) if live_cat else []
    revision_changed = bool(live_cat) and b_src.get("spec_revision") != l_src.get("spec_revision")
    # Asıl çapa: kataloğun kendi özeti. Kimlik kümesi issue gövdesini zenginleştirir ama
    # kararı bu alan verir — cümle METNİ kimliği değiştirmeden düzelirse bile yakalar.
    catalogue_changed = (bool(live_cat)
                         and b_src.get("provisions_sha256") != l_src.get("provisions_sha256"))

    recorded_commit = b_src.get("upstream_commit")
    commit_moved = bool(live_upstream and recorded_commit and live_upstream != recorded_commit)

    if catalogue_changed or added or gone or revision_changed:
        status = "catalogue"
    elif commit_moved:
        status = "upstream"
    elif live_cat and not page_same:
        status = "cosmetic"
    else:
        status = "same"

    return {
        "status": status,
        "page_same": page_same,
        "page_error": page_error,
        "catalogue_measured": bool(live_cat),
        "added": added,
        "gone": gone,
        "revision_recorded": b_src.get("spec_revision"),
        "revision_live": l_src.get("spec_revision"),
        "commit_recorded": recorded_commit,
        "commit_live": live_upstream,
        "candidates_recorded": len(b_ids),
        "candidates_live": len(l_ids) if live_cat else None,
        "catalogue_sha_recorded": b_src.get("provisions_sha256"),
        "catalogue_sha_live": l_src.get("provisions_sha256"),
    }


def main() -> int:
    argv = sys.argv[1:]
    baseline_path = BASELINE
    if "--baseline" in argv:
        i = argv.index("--baseline")
        baseline_path = pathlib.Path(argv[i + 1])
        del argv[i:i + 2]
    mode = "auto"
    if "--mode" in argv:
        i = argv.index("--mode")
        mode = argv[i + 1]
        del argv[i:i + 2]
    if mode not in ("auto", "page", "upstream"):
        print(f"bilinmeyen --mode: {mode} (auto|page|upstream)", file=sys.stderr)
        return 2
    local = [a for a in argv if not a.startswith("--")]

    doc = live_cat = page_error = live_upstream = None
    try:
        mod = load_extractor()
        base = json.loads(baseline_path.read_text(encoding="utf-8"))

        if mode == "upstream":
            # ⚠️ İngilizce: bu metin issue gövdesine giriyor, depo herkese açık.
            page_error = "mode=upstream (page deliberately not fetched; CI gets 403)"
        else:
            try:
                doc = pathlib.Path(local[0]).read_text(encoding="utf-8") if local else fetch(SPEC_URL)
                live_cat = catalogue_of(mod, doc)
            except Exception as e:
                page_error = f"{type(e).__name__}: {e}"
                doc = None
                if mode == "page":
                    raise
                # auto: upstream'e DÜŞ ama sessizce değil.
                print(f"UYARI: sayfa sinyali ÖLÇÜLEMEDİ ({page_error}) — yalnız upstream "
                      f"commit karşılaştırılacak.", file=sys.stderr)

        try:
            live_upstream = mod.upstream_commit()["upstream_commit"]
        except Exception as e:
            # Mesajı da bas: 2026-08-07'de yalnız tip basılıyordu ve CI'daki kota
            # hatası "RuntimeError" olarak göründü, teşhis bir tur geciktirdi.
            print(f"UYARI: upstream commit çözülemedi ({type(e).__name__}: {e}) — "
                  f"'upstream' sinyali bu koşuda ÖLÇÜLMEDİ.", file=sys.stderr)
            live_upstream = None

        if live_cat is None and live_upstream is None:
            raise RuntimeError(f"hiçbir sinyal ölçülemedi (sayfa: {page_error})")
        r = classify(base, doc, live_cat, live_upstream, page_error)
    except Exception as e:
        print(f"ÖLÇÜLEMEDİ: {type(e).__name__}: {e}", file=sys.stderr)
        print("⚠️ Bu 'sürüklenme yok' DEĞİLDİR — bakılamadı.", file=sys.stderr)
        return 2

    print(f"durum: {r['status']}")
    if not r["catalogue_measured"]:
        print(f"  ⚠️ KATALOG SİNYALİ ÖLÇÜLMEDİ: {r['page_error']}")
        print(f"     (yalnız upstream commit karşılaştırıldı — 'same' burada 'sayfa "
              f"denetlendi' DEMEK DEĞİLDİR)")
    else:
        print(f"  katalog sha    : {(r['catalogue_sha_recorded'] or '-')[:12]} -> "
              f"{(r['catalogue_sha_live'] or '-')[:12]}")
        print(f"  sayfa sha eşit : {r['page_same']}  (⚠️ sayfa her yanıtta değişir — sinyal DEĞİL)")
        print(f"  aday sayısı    : {r['candidates_recorded']} -> {r['candidates_live']}"
              f" (eklenen {len(r['added'])}, kaybolan {len(r['gone'])})")
        print(f"  spec_revision  : {r['revision_recorded']} -> {r['revision_live']}")
    print(f"  upstream commit: {(r['commit_recorded'] or '-')[:12]} -> "
          f"{(r['commit_live'] or 'ölçülmedi')[:12]}")

    if out := os.environ.get("GITHUB_OUTPUT"):
        with open(out, "a", encoding="utf-8") as fh:
            fh.write(f"status={r['status']}\n")
            # Tekilleştirme anahtarı KARARLI olmalı; sayfa sha'sı olamaz. Katalog
            # ölçülemediyse (CI'da beklenen hâl) anahtar upstream commit'e düşer.
            key = (r["catalogue_sha_live"] or r["commit_live"] or "?")[:12]
            fh.write(f"fingerprint={(r['revision_live'] or r['revision_recorded'] or '?')} / {key}\n")
            fh.write("body<<EOF\n" + issue_body(r) + "\nEOF\n")
    return 0


def issue_body(r: dict) -> str:
    lines = [
        f"`spec_drift.py` reports **{r['status']}**.",
        "",
        f"| | recorded | live |",
        f"|---|---|---|",
        f"| candidates | {r['candidates_recorded']} | "
        f"{r['candidates_live'] if r['catalogue_measured'] else 'not measured'} |",
        f"| spec_revision | {r['revision_recorded']} | {r['revision_live']} |",
        f"| upstream commit | {(r['commit_recorded'] or '-')[:12]} | "
        f"{(r['commit_live'] or 'not measured')[:12]} |",
        f"| catalogue sha | {(r['catalogue_sha_recorded'] or '-')[:12]} | "
        f"{(r['catalogue_sha_live'] or '-')[:12]} |",
        f"| page sha equal | | {r['page_same']} (page bytes change on every response — "
        f"Cloudflare; not a signal) |",
        "",
    ]
    if r["added"]:
        lines += [f"Added candidates ({len(r['added'])}): " + ", ".join(f"`{i}`" for i in r["added"][:20]), ""]
    if r["gone"]:
        lines += [f"Missing candidates ({len(r['gone'])}): " + ", ".join(f"`{i}`" for i in r["gone"][:20]), ""]
    if not r["catalogue_measured"]:
        lines += [
            "⚠️ **The catalogue signal was not measured in this run** — only the upstream "
            f"commit was compared. Reason: `{r['page_error']}`. gtfs.org sits behind "
            "Cloudflare and returns 403 to hosted runners, so the page-level comparison "
            "runs locally, not in CI. Re-run `python3 spec-audit/spec_drift.py` on a "
            "workstation to see the candidate diff.",
            "",
        ]
    lines += [
        "A spec revision is not a regression here, it is news. What it does mean: the "
        "percentage in `spec-audit/EVIDENCE_BASE.md` describes the recorded catalogue, "
        "not the current specification, until the new candidates are adjudicated in "
        "`spec-audit/PROVISION_TRIAGE.md`.",
        "",
        "Next: `python3 spec-audit/extract_provisions.py`, adjudicate the diff, then "
        "`python3 spec-audit/badge_status.py`.",
    ]
    return "\n".join(lines)


if __name__ == "__main__":
    raise SystemExit(main())
