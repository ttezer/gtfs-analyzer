#!/usr/bin/env python3
"""T5'in 9 boşluğu + 2 yanlış pozitifi için korpus ölçümü (statik tarama).

Doğrulayıcı KOŞULMAZ — zip+csv okunur, token maliyeti ~0.

⚠️ BAŞLIK TUZAĞI: gerçek feed başlıkları tırnaklı ("from_stop_id") ya da baştan
boşluklu (" agency_id") olabilir. Pipeline normalize eder, naif bir sözlük sorgusu
etmez. 2026-08-02'de bu tuzak İKİ KEZ uydurma sonuç üretti. Bu yüzden başlık
csv.reader ile okunur ve strip+tırnak temizliği yapılır.

⚠️ Bu tarama kural predicate'ini YAKLAŞIK taklit eder. "0 eşleşme" gerçek predicate
farklıysa yanlış olabilir; çelişki hâlinde taramaya değil doğrulayıcı koşusuna güven.
"""
import csv, io, json, re, sys, zipfile, pathlib, collections

# Korpus repoda DEĞİL (notgit/ gitignore). Yol kullanıcının yerel kopyasına göre.
CORPUS = pathlib.Path("notgit/corpus/zips")
csv.field_size_limit(10_000_000)

# ISO 4217 minor units — varsayılan 2, aşağıdakiler ayrıksı.
MINOR = {**{c: 0 for c in "BIF CLP DJF GNF ISK JPY KMF KRW PYG RWF UGX UYI VND VUV XAF XOF XPF".split()},
         **{c: 3 for c in "BHD IQD JOD KWD LYD OMR TND".split()}}

URL_FIELDS = {
    "agency.txt": ["agency_url", "agency_fare_url"], "stops.txt": ["stop_url"],
    "routes.txt": ["route_url"], "feed_info.txt": ["feed_publisher_url", "feed_contact_url"],
    "attributions.txt": ["attribution_url"], "booking_rules.txt": ["booking_url", "info_url"],
    "rider_categories.txt": ["eligibility_url"],
}
# HTML taraması: metin taşıyan küçük/orta dosyalar. stop_times bilinçli DIŞARIDA (hacim).
HTML_FILES = ["agency.txt", "stops.txt", "routes.txt", "trips.txt", "feed_info.txt",
              "attributions.txt", "levels.txt", "pathways.txt", "translations.txt"]
# İlk ölçüm çok gevşekti: "K.A.Wheel&Tire;" entity sanıldı, "St Sauvant >< St Césaire"
# etiket sanıldı → 1991 uydurma eşleşme. Yalnız GERÇEK HTML etiket adları ve BİLİNEN
# entity'ler sayılır.
TAGS = "a|b|i|u|p|br|hr|em|strong|span|div|img|font|ul|ol|li|table|tr|td|th|h1|h2|h3|small|sub|sup"
HTML_RE = re.compile(rf"<\s*/?\s*({TAGS})\b[^<>]*>|<!--|&(nbsp|amp|lt|gt|quot|apos|#\d{{2,5}}|#x[0-9a-fA-F]{{2,4}});", re.I)


def rows(zf, name):
    """(başlık-normalize edilmiş) sözlük satırları. Dosya yoksa boş."""
    try:
        raw = zf.read(name)
    except KeyError:
        # sarılı kök: tek üst klasöre gömülü feed
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
            # ── 1. URL şeması ────────────────────────────────────────────────
            for fname, fields in URL_FIELDS.items():
                for row in rows(zf, fname):
                    for f in fields:
                        v = get(row, f)
                        # Şemasız veya bozuk değer ('1', '-') zaten looks_like_url'den
                        # düşer ve mevcut kural ateşler; yeni bulgu üretmez. Ölçülen şey:
                        # geçerli bir ŞEMA taşıyan ama http/https OLMAYAN değer.
                        if v and re.match(r"^[a-zA-Z][a-zA-Z0-9+.\-]*:", v) \
                                and not v.lower().startswith(("http://", "https://")):
                            hit["1_url_scheme"][fid] += 1
                            if len(sample["1_url_scheme"]) < 25:
                                sample["1_url_scheme"].append(f"{fid} {fname}.{f}={v[:60]}")

            # ── 2. HTML etiketi / kaçış dizisi ───────────────────────────────
            for fname in HTML_FILES:
                for row in rows(zf, fname):
                    for k, v in row.items():
                        if v and HTML_RE.search(v):
                            hit["2_html"][fid] += 1
                            if len(sample["2_html"]) < 25:
                                sample["2_html"].append(f"{fid} {fname}.{k}={v[:60]}")

            # ── 3. record_sub_id gereklilik yönü ─────────────────────────────
            for row in rows(zf, "translations.txt"):
                if get(row, "table_name") == "stop_times" and get(row, "record_id") \
                        and not get(row, "record_sub_id"):
                    hit["3_record_sub_id"][fid] += 1

            # ── 4. start_day yasağı (booking_type=1 + duration_max) ──────────
            for row in rows(zf, "booking_rules.txt"):
                if get(row, "booking_type") == "1" and get(row, "prior_notice_duration_max") \
                        and get(row, "prior_notice_start_day"):
                    hit["4_start_day"][fid] += 1

            # ── 5. ISO 4217 ondalık basamak ──────────────────────────────────
            for fname, amt, cur in (("fare_products.txt", "amount", "currency"),
                                    ("fare_attributes.txt", "price", "currency_type")):
                for row in rows(zf, fname):
                    a, c = get(row, amt), get(row, cur).upper()
                    if not a or not c or len(c) != 3:
                        continue
                    want = MINOR.get(c, 2)
                    dec = len(a.split(".", 1)[1]) if "." in a else 0
                    # 'CZK 39' (ondalıksız tam sayı) korpusta evrensel ve okunuşu
                    # belirsiz değil; ilk ölçüm bunu ihlal sayıp 2 milyon eşleşme verdi.
                    # Anlamlı ihlal FAZLA basamaktır: 'JPY 100.00' (yen minor unit'i 0).
                    if dec > want:
                        hit["5_iso4217"][fid] += 1
                        if len(sample["5_iso4217"]) < 25:
                            sample["5_iso4217"].append(f"{fid} {fname} {c} {a} (beklenen {want} basamak)")

            # ── 6. boarding area varken platforma pathway ────────────────────
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

            # ── 8. traversal_time tavsiyesi (mode 3/4/5) ─────────────────────
            for row in rows(zf, "pathways.txt"):
                if get(row, "pathway_mode") in ("3", "4", "5") and not get(row, "traversal_time"):
                    hit["8_traversal_time"][fid] += 1

            # ── 9. platform_code'da "platform"/"track" ───────────────────────
            for row in rows(zf, "stops.txt"):
                pc = get(row, "platform_code").lower()
                if pc and re.search(r"\b(platform|track|peron|gleis|voie|quai|binario|via|anden)\b", pc):
                    hit["9_platform_code"][fid] += 1
                    if len(sample["9_platform_code"]) < 25:
                        sample["9_platform_code"].append(f"{fid} platform_code={pc[:40]}")

            # ── 10. translations.field_value hedefte gerçekten var mı ────────
            # Spec: "The field must have exactly the value defined in field_value."
            # `field_value` biçimi yalnız DEĞERE göre eşleşir; eşleşen satır yoksa çeviri
            # sessizce etkisizdir.
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
                        continue          # dosya yok → başka kural (XFL_016) ilgilenir
                    # `field_name` o dosyada GERÇEKTEN bir sütun mu? Değilse ihlal
                    # `TRN_002`'nin ("field_name bu tablo için geçersiz") alanıdır ve
                    # burada sayılırsa aynı olgu iki kez raporlanır. İlk ölçümde
                    # `routes.long_name` (doğrusu `route_long_name`) bu yüzden 2546
                    # sahte bulgu üretti.
                    if fld not in cache[fname][0]:
                        continue
                    if not any(get(r, fld) == want for r in cache[fname]):
                        hit["10_field_value"][fid] += 1
                        if len(sample["10_field_value"]) < 25:
                            sample["10_field_value"].append(f"{fid} {tbl}.{fld}={want[:40]}")

            # ── 11. exact_times=1 iken end_time headway sınırında mı ─────────
            # Spec: end_time son sefer başlangıcından BÜYÜK ama başlangıç+headway'den
            # KÜÇÜK olmalı → (end-start) headway'in TAM KATI olmamalı.
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
                    continue              # FRQ_005/FRQ_004'ün alanı
                if (e - s) % h == 0:
                    hit["11_exact_times"][fid] += 1
                    if len(sample["11_exact_times"]) < 25:
                        sample["11_exact_times"].append(
                            f"{fid} start={get(row,'start_time')} end={get(row,'end_time')} headway={h}")

            # ── 12. pickup_type=2 / drop_off_type=2 iken booking rule önerilir ─
            # Spec: "Recommended when pickup_type=2" (ve drop_off ikizi). STM_040 bunun
            # yerine FLEX PENCERESİ varken booking rule arar — farklı popülasyon.
            # ⚠️ stop_times büyük olabilir; yalnız gerekli sütunlara bakılır.
            # ⚠️ `booking_rules.txt` YOKSA tavsiye uygulanamaz: `pickup_booking_rule_id`
            # yazılsa FK kırılır (BKR_017). İlk ölçüm bu kapıyı koymayınca 97.355 satır
            # verdi ve en yoğun dört feed'in hiçbirinde booking_rules.txt yoktu.
            # ⚠️ DOSYA VARLIĞI YETMEZ, KAYIT olmalı: iki feed `booking_rules.txt`'i yalnız
            # BAŞLIK satırıyla taşıyor. Kural `records.booking_rules.is_empty()`'e bakar ve
            # doğru davranır (bağlanacak kural yoksa tavsiye uygulanamaz); dosya varlığına
            # bakan ilk ölçüm o feed'leri sayıp kuralla çelişmişti.
            has_booking = any(True for _ in rows(zf, "booking_rules.txt"))
            for row in (rows(zf, "stop_times.txt") if has_booking else []):
                if get(row, "pickup_type") == "2" and not get(row, "pickup_booking_rule_id"):
                    hit["12_pickup_type2"][fid] += 1
                if get(row, "drop_off_type") == "2" and not get(row, "drop_off_booking_rule_id"):
                    hit["12_pickup_type2"][fid] += 1
                # Aynı satırda Flex penceresi de varsa STM_040 zaten konuşuyor — ayrı say.
                if (get(row, "pickup_type") == "2" or get(row, "drop_off_type") == "2") and (
                        get(row, "start_pickup_drop_off_window") or get(row, "end_pickup_drop_off_window")):
                    hit["12b_overlaps_stm040"][fid] += 1

            # ── FP-A. agency_phone vanity (spec'te İZİNLİ, biz reddediyoruz) ─
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
