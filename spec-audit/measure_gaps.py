#!/usr/bin/env python3
"""Corpus measurement for T5's 9 gaps + 2 false positives (static scan).

The validator is NOT RUN — ZIP+CSV files are read, with ~0 token cost.

⚠️ HEADER TRAP: real feed headers may be quoted ("from_stop_id") or have leading
spaces (" agency_id"). The pipeline normalizes them; do not use a naive dictionary lookup.
This trap produced false results TWICE on 2026-08-02. Therefore read headers with csv.reader
and strip whitespace and quotes.

⚠️ This scan APPROXIMATES the rule predicates. "0 matches" may be wrong if the real predicate
differs; when they conflict, trust the validator run, not the scan.
"""
import csv, io, json, re, sys, zipfile, pathlib, collections

# The corpus is NOT IN THE REPO (notgit/ is gitignored). The path is relative to the user's local copy.
CORPUS = pathlib.Path("notgit/corpus/zips")
csv.field_size_limit(10_000_000)

# ISO 4217 minor units — default 2, with the exceptions below.
MINOR = {**{c: 0 for c in "BIF CLP DJF GNF ISK JPY KMF KRW PYG RWF UGX UYI VND VUV XAF XOF XPF".split()},
         **{c: 3 for c in "BHD IQD JOD KWD LYD OMR TND".split()}}

URL_FIELDS = {
    "agency.txt": ["agency_url", "agency_fare_url"], "stops.txt": ["stop_url"],
    "routes.txt": ["route_url"], "feed_info.txt": ["feed_publisher_url", "feed_contact_url"],
    "attributions.txt": ["attribution_url"], "booking_rules.txt": ["booking_url", "info_url"],
    "rider_categories.txt": ["eligibility_url"],
}
# HTML scan: small/medium files carrying text. stop_times is deliberately EXCLUDED (volume).
HTML_FILES = ["agency.txt", "stops.txt", "routes.txt", "trips.txt", "feed_info.txt",
              "attributions.txt", "levels.txt", "pathways.txt", "translations.txt"]
# The first measurement was too loose: "K.A.Wheel&Tire;" was treated as an entity, and
# "St Sauvant >< St Césaire" as a tag → 1,991 false matches. Count only REAL HTML tag names
# and KNOWN entities.
TAGS = "a|b|i|u|p|br|hr|em|strong|span|div|img|font|ul|ol|li|table|tr|td|th|h1|h2|h3|small|sub|sup"
HTML_RE = re.compile(rf"<\s*/?\s*({TAGS})\b[^<>]*>|<!--|&(nbsp|amp|lt|gt|quot|apos|#\d{{2,5}}|#x[0-9a-fA-F]{{2,4}});", re.I)


def rows(zf, name):
    """(Header-normalized) dictionary rows. Empty when the file is absent."""
    try:
        raw = zf.read(name)
    except KeyError:
        # Wrapped root: feed nested under one top-level directory.
        cand = [n for n in zf.namelist() if n.endswith("/" + name)]
        if not cand:
            return
        raw = zf.read(cand[0])
    text = raw.decode("utf-8-sig", errors="replace")
    rdr = csv.reader(io.StringIO(text))
    try:
        hdr = next(rdr)
    except StopIteration:
        return
    hdr = [h.strip().strip('"').strip() for h in hdr]
    for r in rdr:
        yield dict(zip(hdr, [v.strip() for v in r]))


def get(row, key):
    return (row.get(key) or "").strip()


def main():
    hit = collections.defaultdict(lambda: collections.defaultdict(int))   # gap -> feed -> count
    sample = collections.defaultdict(list)
    zips = sorted(CORPUS.glob("*.zip"))
    for i, z in enumerate(zips, 1):
        fid = z.stem
        try:
            zf = zipfile.ZipFile(z)
        except Exception:
            continue
        try:
            # ── 1. URL scheme ────────────────────────────────────────────────
            for fname, fields in URL_FIELDS.items():
                for row in rows(zf, fname):
                    for f in fields:
                        v = get(row, f)
                        # Values without a scheme or with a malformed value ('1', '-') already
                        # fail looks_like_url and fire the existing rule; they produce no new finding.
                        # Measure values carrying a valid SCHEME but not http/https.
                        if v and re.match(r"^[a-zA-Z][a-zA-Z0-9+.\-]*:", v) \
                                and not v.lower().startswith(("http://", "https://")):
                            hit["1_url_scheme"][fid] += 1
                            if len(sample["1_url_scheme"]) < 25:
                                sample["1_url_scheme"].append(f"{fid} {fname}.{f}={v[:60]}")

            # ── 2. HTML tag / entity ─────────────────────────────────────────
            for fname in HTML_FILES:
                for row in rows(zf, fname):
                    for k, v in row.items():
                        if v and HTML_RE.search(v):
                            hit["2_html"][fid] += 1
                            if len(sample["2_html"]) < 25:
                                sample["2_html"].append(f"{fid} {fname}.{k}={v[:60]}")

            # ── 3. record_sub_id requirement direction ──────────────────────
            for row in rows(zf, "translations.txt"):
                if get(row, "table_name") == "stop_times" and get(row, "record_id") \
                        and not get(row, "record_sub_id"):
                    hit["3_record_sub_id"][fid] += 1

            # ── 4. start_day prohibition (booking_type=1 + duration_max) ─────
            for row in rows(zf, "booking_rules.txt"):
                if get(row, "booking_type") == "1" and get(row, "prior_notice_duration_max") \
                        and get(row, "prior_notice_start_day"):
                    hit["4_start_day"][fid] += 1

            # ── 5. ISO 4217 decimal places ───────────────────────────────────
            for fname, amt, cur in (("fare_products.txt", "amount", "currency"),
                                    ("fare_attributes.txt", "price", "currency_type")):
                for row in rows(zf, fname):
                    a, c = get(row, amt), get(row, cur).upper()
                    if not a or not c or len(c) != 3:
                        continue
                    want = MINOR.get(c, 2)
                    dec = len(a.split(".", 1)[1]) if "." in a else 0
                    # 'CZK 39' (whole number without decimals) is universal in the corpus and unambiguous;
                    # the first measurement treated it as a violation and produced 2 million matches.
                    # The meaningful violation is TOO MANY decimal places: 'JPY 100.00' (yen minor unit is 0).
                    if dec > want:
                        hit["5_iso4217"][fid] += 1
                        if len(sample["5_iso4217"]) < 25:
                            sample["5_iso4217"].append(f"{fid} {fname} {c} {a} (beklenen {want} basamak)")

            # ── 6. pathway to a platform when a boarding area exists ─────────
            parents_with_ba, ptypes = set(), {}
            for row in rows(zf, "stops.txt"):
                sid, lt = get(row, "stop_id"), get(row, "location_type")
                ptypes[sid] = lt
                if lt == "4" and get(row, "parent_station"):
                    parents_with_ba.add(get(row, "parent_station"))
            if parents_with_ba:
                for row in rows(zf, "pathways.txt"):
                    for f in ("from_stop_id", "to_stop_id"):
                        if get(row, f) in parents_with_ba:
                            hit["6_platform_pathway"][fid] += 1

            # ── 8. traversal_time recommendation (mode 3/4/5) ────────────────
            for row in rows(zf, "pathways.txt"):
                if get(row, "pathway_mode") in ("3", "4", "5") and not get(row, "traversal_time"):
                    hit["8_traversal_time"][fid] += 1

            # ── 9. "platform"/"track" in platform_code ───────────────────────
            for row in rows(zf, "stops.txt"):
                pc = get(row, "platform_code").lower()
                if pc and re.search(r"\b(platform|track|peron|gleis|voie|quai|binario|via|anden)\b", pc):
                    hit["9_platform_code"][fid] += 1
                    if len(sample["9_platform_code"]) < 25:
                        sample["9_platform_code"].append(f"{fid} platform_code={pc[:40]}")

            # ── 10. Does translations.field_value actually exist at the target? ─
            # Spec: "The field must have exactly the value defined in field_value."
            # `field_value` matches only by VALUE; when no row matches, the translation
            # silently has no effect.
            tr_rows = [r for r in rows(zf, "translations.txt")
                       if get(r, "field_value") and not get(r, "record_id")]
            if tr_rows:
                cache = {}
                for row in tr_rows:
                    tbl, fld = get(row, "table_name"), get(row, "field_name")
                    want = get(row, "field_value")
                    if not tbl or not fld:
                        continue
                    fname = f"{tbl}.txt"
                    if fname not in cache:
                        cache[fname] = list(rows(zf, fname))
                    if not cache[fname]:
                        continue          # no file → another rule (XFL_016) covers it
                    # Is `field_name` REALLY a column in that file? If not, it is the
                    # `TRN_002` case ("field_name is invalid for this table"), and counting it
                    # here reports the same fact twice. The first measurement therefore produced
                    # 2,546 false findings for `routes.long_name` (correct:
                    # `route_long_name`).
                    if fld not in cache[fname][0]:
                        continue
                    if not any(get(r, fld) == want for r in cache[fname]):
                        hit["10_field_value"][fid] += 1
                        if len(sample["10_field_value"]) < 25:
                            sample["10_field_value"].append(f"{fid} {tbl}.{fld}={want[:40]}")

            # ── 11. Is end_time on the headway boundary when exact_times=1? ──
            # Spec: end_time must be GREATER than the last trip start but LESS than start+headway
            # → (end-start) must not be an EXACT MULTIPLE of headway.
            def _secs(v):
                p = v.split(":")
                return int(p[0]) * 3600 + int(p[1]) * 60 + int(p[2]) if len(p) == 3 else None
            for row in rows(zf, "frequencies.txt"):
                if get(row, "exact_times") != "1":
                    continue
                try:
                    s, e = _secs(get(row, "start_time")), _secs(get(row, "end_time"))
                    h = int(get(row, "headway_secs"))
                except (ValueError, IndexError):
                    continue
                if s is None or e is None or h <= 0 or e <= s:
                    continue              # covered by FRQ_005/FRQ_004
                if (e - s) % h == 0:
                    hit["11_exact_times"][fid] += 1
                    if len(sample["11_exact_times"]) < 25:
                        sample["11_exact_times"].append(
                            f"{fid} start={get(row,'start_time')} end={get(row,'end_time')} headway={h}")

            # ── 12. Recommend a booking rule when pickup_type=2 / drop_off_type=2 ─
            # Spec: "Recommended when pickup_type=2" (and the drop_off counterpart). STM_040 instead
            # looks for a booking rule when a FLEX WINDOW exists — a different population.
            # ⚠️ stop_times can be large; inspect only the required columns.
            # ⚠️ The recommendation cannot be applied when `booking_rules.txt` is ABSENT:
            # writing `pickup_booking_rule_id` would break the FK (BKR_017). The first measurement
            # lacked this gate, returned 97,355 rows, and none of the four largest feeds had booking_rules.txt.
            # ⚠️ FILE EXISTENCE IS NOT ENOUGH; records must exist: two feeds carry only the
            # `booking_rules.txt` HEADER. The rule checks `records.booking_rules.is_empty()` and behaves
            # correctly (a recommendation cannot apply when there is no rule to reference); the first
            # measurement counted those feeds by file existence and contradicted the rule.
            has_booking = any(True for _ in rows(zf, "booking_rules.txt"))
            for row in (rows(zf, "stop_times.txt") if has_booking else []):
                if get(row, "pickup_type") == "2" and not get(row, "pickup_booking_rule_id"):
                    hit["12_pickup_type2"][fid] += 1
                if get(row, "drop_off_type") == "2" and not get(row, "drop_off_booking_rule_id"):
                    hit["12_pickup_type2"][fid] += 1
                # When the same row also has a Flex window, STM_040 already covers it — count separately.
                if (get(row, "pickup_type") == "2" or get(row, "drop_off_type") == "2") and (
                        get(row, "start_pickup_drop_off_window") or get(row, "end_pickup_drop_off_window")):
                    hit["12b_overlaps_stm040"][fid] += 1

            # ── FP-A. agency_phone vanity (ALLOWED by spec, rejected by us) ───
            for row in rows(zf, "agency.txt"):
                ph = get(row, "agency_phone")
                if ph and any(c.isalpha() for c in ph):
                    hit["FPa_vanity_phone"][fid] += 1
                    if len(sample["FPa_vanity_phone"]) < 25:
                        sample["FPa_vanity_phone"].append(f"{fid} agency_phone={ph[:50]}")
        except Exception as e:
            print(f"  ! {fid}: {type(e).__name__} {e}", file=sys.stderr)
        finally:
            zf.close()
        if i % 40 == 0:
            print(f"  … {i}/{len(zips)}", file=sys.stderr)

    out = {}
    for gap, feeds in sorted(hit.items()):
        out[gap] = {"feeds": len(feeds), "total": sum(feeds.values()),
                    "max_in_one_feed": max(feeds.values()),
                    "top": sorted(feeds.items(), key=lambda kv: -kv[1])[:5],
                    "samples": sample.get(gap, [])}
    print(json.dumps({"corpus_size": len(zips), "results": out}, ensure_ascii=False, indent=2))


if __name__ == "__main__":
    main()
