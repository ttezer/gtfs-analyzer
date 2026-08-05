#!/usr/bin/env python3
"""12 presence boşluğu korpusta ne kadar gerçekleşiyor?

Her (dosya, alan) için: sütun VARSA ve değer BOŞSA say. Sütun hiç yoksa ayrı sayılır —
o durum `ARC_025` (zorunlu sütun başlıkta yok) alanıdır, bu boşlukların değil.
"""
import csv, io, zipfile, pathlib, collections, json

FIELDS = [
    ("areas.txt", "area_id"),
    *[("calendar.txt", d) for d in
      ("monday","tuesday","wednesday","thursday","friday","saturday","sunday")],
    ("fare_media.txt", "fare_media_id"),
    ("networks.txt", "network_id"),
    ("rider_categories.txt", "rider_category_id"),
    ("location_groups.txt", "location_group_id"),
    ("location_group_stops.txt", "location_group_id"),
    ("location_group_stops.txt", "stop_id"),
    ("stop_areas.txt", "area_id"),
    ("stop_areas.txt", "stop_id"),
    ("timeframes.txt", "service_id"),
    ("transfers.txt", "transfer_type"),
    ("trips.txt", "service_id"),
]
csv.field_size_limit(10_000_000)
empty = collections.Counter()          # (file,field) -> boş satır
feeds = collections.defaultdict(set)   # (file,field) -> feed kümesi
present = collections.Counter()        # (file,field) -> dosyayı taşıyan feed sayısı

for z in sorted(pathlib.Path("notgit/corpus/zips").glob("*.zip")):
    try: zf = zipfile.ZipFile(z)
    except Exception: continue
    cache = {}
    for fname, field in FIELDS:
        if fname not in cache:
            names = [n for n in zf.namelist() if n.split("/")[-1] == fname]
            if not names: cache[fname] = None
            else:
                try:
                    txt = zf.read(names[0]).decode("utf-8-sig", errors="replace")
                    r = csv.reader(io.StringIO(txt))
                    hdr = [h.strip().strip('"') for h in next(r)]
                    cache[fname] = (hdr, list(r))
                except Exception: cache[fname] = None
        if not cache[fname]: continue
        hdr, rows = cache[fname]
        if field not in hdr: continue          # sütun yok → ARC_025'in alanı
        present[(fname, field)] += 1
        i = hdr.index(field)
        n = sum(1 for row in rows if i >= len(row) or not row[i].strip())
        if n:
            empty[(fname, field)] += n
            feeds[(fname, field)].add(z.stem)
    zf.close()

print(f"{'dosya.alan':44s} {'sütunu olan feed':>16s} {'boş satır':>10s} {'feed':>6s}")
for key in FIELDS:
    f, fl = key
    print(f"{f+'.'+fl:44s} {present[key]:16d} {empty[key]:10d} {len(feeds[key]):6d}")
print("\nen yoğun örnekler:")
for key, n in empty.most_common(5):
    print(f"  {key[0]}.{key[1]}: {n} satır · feed: {sorted(feeds[key])[:4]}")
