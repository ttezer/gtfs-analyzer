#!/usr/bin/env python3
"""How often do the 12 presence gaps occur in the corpus?

For each (file, field): count when the column EXISTS and the value is EMPTY. Count a missing column separately — that is the domain of `ARC_025` (required column absent from the header), not a presence gap.
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
empty = collections.Counter()          # (file,field) -> empty rows
feeds = collections.defaultdict(set)   # (file,field) -> feed set
present = collections.Counter()        # (file,field) -> feeds carrying the file

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
        if field not in hdr: continue          # column absent → ARC_025's domain
        present[(fname, field)] += 1
        i = hdr.index(field)
        # ⚠️ DO NOT COUNT A COMPLETELY EMPTY ROW. The first measurement did so and reported
        # results such as "trips.service_id 24 rows"; those were actually trailing empty CSV rows.
        # The pipeline filters them with `ARC_018` and emits no record, so presence rules never see them.
        # The measurement must count what the rule sees.
        n = sum(1 for row in rows
                if any(c.strip() for c in row)                # not an empty row
                and (i >= len(row) or not row[i].strip()))
        if n:
            empty[(fname, field)] += n
            feeds[(fname, field)].add(z.stem)
    zf.close()

print(f"{'file.field':44s} {'feeds with field':>16s} {'empty rows':>10s} {'feeds':>6s}")
for key in FIELDS:
    f, fl = key
    print(f"{f+'.'+fl:44s} {present[key]:16d} {empty[key]:10d} {len(feeds[key]):6d}")
print("\nmost frequent examples:")
for key, n in empty.most_common(5):
    print(f"  {key[0]}.{key[1]}: {n} rows · feeds: {sorted(feeds[key])[:4]}")
