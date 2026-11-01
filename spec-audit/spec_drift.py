#!/usr/bin/env python3
"""Detect spec drift — issue #73.

The catalogue is generated from a LIVE page. If gtfs.org publishes a revision, our 299 candidates
silently describe OLD text and the percentage in `EVIDENCE_BASE.md` no longer belongs to the current
spec.
Nothing currently detects this: `extract_provisions.py` is run manually, and CI has only
`badge_status.py`, which checks the ledger against the catalogue WE HAVE — NOT the spec.

## 🔴 Why a raw SHA-256 comparison is NOT ENOUGH BY ITSELF

The issue's first proposal was "compare the page SHA-256 with `_source.sha256` and open an issue
if different". It was MEASURED on 2026-08-07: the page SHA-256 was ALREADY different (295,559 →
295,827 bytes), but the catalogue was **exactly the same** — all 299 candidates, the same IDs, the same
`spec_revision`. The difference was in the site scaffold (mkdocs rebuild), not normative text.
A raw-SHA gate would false-alarm on its first run, exactly what the issue's own rationale
("trains people to ignore it") forbids.

Therefore measure and report three separate signals:

  `same`      → page bytes and catalogue are unchanged. Silent.
  `cosmetic`  → page bytes changed, CATALOGUE UNCHANGED. NOT NEWS; DO NOT OPEN an issue.
                🔴 **THIS IS THE EXPECTED STATE.** The page changes on every response (see below).
  `catalogue` → `provisions_sha256` changed (candidate added/removed or `spec_revision`). NEWS → issue.
  `upstream`  → catalogue unchanged but a new commit touched `reference.md`; the page may not have
                been rebuilt yet → NEWS, a separate class.
                This distinction avoids treating a delayed page rebuild as a catalogue change.

## 🔴 The page SHA-256 CANNOT BE REPRODUCED (2026-08-07 measurement)

The page was downloaded three times in succession and produced three DIFFERENT digests. The only
difference was two lines injected by Cloudflare: the `data-cfemail` token and `__CF$cv$params` ray ID.
Therefore a raw-SHA comparison triggers on every run — it is WORTHLESS as a gate and would produce
a weekly false alarm if it opened an issue.

The decision anchor is `_source.provisions_sha256`: computed from candidates, independent of the site
scaffold, and measured to be the SAME across two SEPARATE downloads.

## 🔴 Cannot be retrieved from gtfs.org CI — signal split in two (2026-08-07 measurement)

The first CI run got **HTTP 403**: gtfs.org is behind Cloudflare and blocks hosted runner
IPs (the browser User-Agent was not enough). The CATALOGUE signal therefore cannot be measured
from CI — this is an environment limitation, not an error.

But drift's actual question is "did the spec text change?", and CI CAN ANSWER that:
read the newest commit touching `reference.md` from the GitHub API. Therefore there are three modes:

  `page`     → download the page and regenerate/compare the CATALOGUE (local/developer)
  `upstream` → compare only the commit (runs in CI)
  `auto`     → try the page; if unavailable, FALL BACK to upstream **and state this explicitly**

⚠️ The fallback is NOT SILENT: the report prints which signal was NOT measured and why, and it
also enters the issue body. A silent fallback would be a blind check that appears green.

## Exit codes — "no drift" and "could not inspect" are SEPARATE

  0 → AT LEAST ONE signal was measured (result `same`/`cosmetic`/`catalogue`/`upstream`)
  2 → NO signal was measured (neither page nor commit). The job turns red, DELIBERATELY:
      a check that silently stays green is not a check (`zip_peek.py` teaches the same lesson —
      do not confuse "file absent" with "could not inspect").

Drift itself does not BREAK the build: a spec revision is news, not a regression in this
repository.

Çalıştır:
    python3 spec-audit/spec_drift.py                 # auto (sayfa → gerekirse upstream)
    python3 spec-audit/spec_drift.py spec.html       # yerel kopyadan (test)
    python3 spec-audit/spec_drift.py --mode upstream # commit only (CI runs this)
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
    """Load `extract_provisions.py` as a module — keep sentence-splitting logic in ONE place.
    Copying it would let the two sides diverge silently."""
    spec = importlib.util.spec_from_file_location("extract_provisions", HERE / "extract_provisions.py")
    mod = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(mod)
    return mod


def fetch(url: str) -> str:
    """Download the spec page and verify that it REALLY is the spec.

    ⚠️ Use `curl`, NOT `urllib` — the development machine has no Python certificate store.
    ⚠️ Browser User-Agent: gtfs.org is behind Cloudflare and may return an interstitial (bot protection)
    to CI runners with the default `curl/*` UA. Such a response looks like a fetched page but contains
    no provisions → it reduces the catalogue to 0 candidates. Therefore validate the response by CONTENT
    and report the problem separately: silently saying "no drift" would be the worst form of
    audit.
    """
    ua = ("Mozilla/5.0 (X11; Linux x86_64) AppleWebKit/537.36 (KHTML, like Gecko) "
          "Chrome/126.0 Safari/537.36 (+gtfs-analyzer spec-drift)")
    p = subprocess.run(
        ["curl", "-sSL", "--max-time", "90", "-A", ua,
         "-H", "Accept: text/html,application/xhtml+xml", "-w", "\n%{http_code}", url],
        capture_output=True,
    )
    if p.returncode != 0:
        raise RuntimeError(f"curl failed: {p.stderr.decode()[:160]}")
    body, _, code = p.stdout.decode("utf-8", "replace").rpartition("\n")
    if code.strip() != "200":
        raise RuntimeError(f"HTTP {code.strip()} — could not fetch page ({len(body)} bytes)")
    # Immutable anchor for the spec page. Without it, what we have is NOT THE SPEC (interstitial,
    # error page, redirect body…).
    if "field-definitions" not in body or "Revised" not in body:
        head = " ".join(body[:200].split())
        raise RuntimeError(
            f"response is not the spec page ({len(body)} bytes) — it may be bot protection "
            f"or a redirect. Head: {head!r}")
    return body


def catalogue_of(mod, doc: str) -> dict:
    """Belgeden kataloğu ÜRET ama diske yazma — `extract_provisions.main()` dosyaya
    yazar, o yüzden geçici bir dizine yönlendirilir."""
    with tempfile.TemporaryDirectory() as tmp:
        out = pathlib.Path(tmp) / "spec_provisions.json"
        saved_out_path, real_argv = mod.OUT, sys.argv
        mod.OUT = out
        # Catalogue comparison does NOT REQUIRE resolving a commit; avoiding a network request here
        # makes the measurement independent of the GitHub API quota.
        sys.argv = ["extract_provisions", "--no-upstream", "-"]
        try:
            mod.fetch = lambda argv: doc  # noqa: ARG005 — document is already in memory
            # The generator's progress lines and `--no-upstream` warning are EXPECTED here
            # (the temporary catalogue is deliberately not pinned) — do not pollute the report.
            with open(os.devnull, "w", encoding="utf-8") as null:
                saved_stdout, saved_stderr = sys.stdout, sys.stderr
                sys.stdout = sys.stderr = null
                try:
                    rc = mod.main()
                finally:
                    sys.stdout, sys.stderr = saved_stdout, saved_stderr
            if rc != 0:
                raise RuntimeError("extract_provisions.main() returned an error")
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
    # Primary anchor: the catalogue's own digest. The ID set enriches the issue body, but this
    # field makes the decision — it catches a corrected sentence even when its ID does not change.
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
        print(f"unknown --mode: {mode} (auto|page|upstream)", file=sys.stderr)
        return 2
    local = [a for a in argv if not a.startswith("--")]

    doc = live_cat = page_error = live_upstream = None
    try:
        mod = load_extractor()
        base = json.loads(baseline_path.read_text(encoding="utf-8"))

        if mode == "upstream":
            # ⚠️ English: this text enters the issue body, and the repository is public.
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
                # auto: FALL BACK to upstream, but not silently.
                print(f"WARNING: page signal NOT MEASURED ({page_error}) — comparing only the "
                      f"upstream commit.", file=sys.stderr)

        try:
            live_upstream = mod.upstream_commit()["upstream_commit"]
        except Exception as e:
            # Print the message too: on 2026-08-07 only the type was printed and the CI quota
            # error appeared as "RuntimeError", delaying diagnosis by one round.
            print(f"WARNING: could not resolve upstream commit ({type(e).__name__}: {e}) — "
                  f"the 'upstream' signal was NOT MEASURED in this run.", file=sys.stderr)
            live_upstream = None

        if live_cat is None and live_upstream is None:
            raise RuntimeError(f"no signal was measured (page: {page_error})")
        r = classify(base, doc, live_cat, live_upstream, page_error)
    except Exception as e:
        print(f"NOT MEASURED: {type(e).__name__}: {e}", file=sys.stderr)
        print("⚠️ This does NOT mean 'no drift' — the source could not be inspected.", file=sys.stderr)
        return 2

    print(f"status: {r['status']}")
    if not r["catalogue_measured"]:
        print(f"  ⚠️ CATALOGUE SIGNAL NOT MEASURED: {r['page_error']}")
        print(f"     (only the upstream commit was compared — 'same' does NOT mean 'the page "
              f"was inspected' here)")
    else:
        print(f"  catalogue sha  : {(r['catalogue_sha_recorded'] or '-')[:12]} -> "
              f"{(r['catalogue_sha_live'] or '-')[:12]}")
        print(f"  page sha equal : {r['page_same']}  (⚠️ page changes on every response — NOT a signal)")
        print(f"  candidates     : {r['candidates_recorded']} -> {r['candidates_live']}"
              f" (added {len(r['added'])}, removed {len(r['gone'])})")
        print(f"  spec_revision  : {r['revision_recorded']} -> {r['revision_live']}")
    print(f"  upstream commit: {(r['commit_recorded'] or '-')[:12]} -> "
          f"{(r['commit_live'] or 'not measured')[:12]}")

    if out := os.environ.get("GITHUB_OUTPUT"):
        with open(out, "a", encoding="utf-8") as fh:
            fh.write(f"status={r['status']}\n")
            # The deduplication key must be STABLE; it cannot be the page SHA. If the catalogue
            # was not measured (expected in CI), fall back to the upstream commit.
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
        f"| spec_revision | {r['revision_recorded']} | "
        f"{r['revision_live'] if r['catalogue_measured'] else 'not measured'} |",
        f"| upstream commit | {(r['commit_recorded'] or '-')[:12]} | "
        f"{(r['commit_live'] or 'not measured')[:12]} |",
        f"| catalogue sha | {(r['catalogue_sha_recorded'] or '-')[:12]} | "
        f"{(r['catalogue_sha_live'] or 'not measured')[:12]} |",
        # ⚠️ If the page was never fetched, writing `False` would read like "tried and failed".
        f"| page bytes | | " + ("not fetched" if not r["catalogue_measured"]
                                else f"{'same' if r['page_same'] else 'differ'} "
                                     "(they differ on every response — Cloudflare; "
                                     "not a signal)") + " |",
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
