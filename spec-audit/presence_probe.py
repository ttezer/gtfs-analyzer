#!/usr/bin/env python3
"""Her şüpheli alan için: değeri BOŞ bırakıldığında çapalayan kural ateşliyor mu?

`presence:required` atomu ancak boş değerde bir notice çıkarsa GERÇEKTEN ölçülüyordur.
grep ile `is_empty()` aramak yetmez — yönü söylemez (ARS_001'de "boşsa atla" çıktı).
"""
import json, subprocess, sys, zipfile, pathlib, tempfile

BASE = {
 "agency.txt":"agency_id,agency_name,agency_url,agency_timezone\nA1,T,https://a.com,Europe/Istanbul\n",
 "stops.txt":"stop_id,stop_name,stop_lat,stop_lon\nS1,A,41.0,29.0\nS2,B,41.1,29.1\n",
 "routes.txt":"route_id,agency_id,route_short_name,route_type\nR1,A1,1,3\n",
 "trips.txt":"route_id,service_id,trip_id\nR1,SVC,T1\n",
 "stop_times.txt":"trip_id,arrival_time,departure_time,stop_id,stop_sequence\nT1,08:00:00,08:00:00,S1,1\nT1,08:10:00,08:10:00,S2,2\n",
 "calendar.txt":"service_id,monday,tuesday,wednesday,thursday,friday,saturday,sunday,start_date,end_date\nSVC,1,1,1,1,1,0,0,20250101,20261231\n",
}
# (kural, dosya, ek/değiştirilen içerik) — hedef alan BOŞ
CASES = [
 ("ARS_001","areas.txt","area_id,area_name\n,Bolge\n"),
 ("CAL_002","calendar.txt","service_id,monday,tuesday,wednesday,thursday,friday,saturday,sunday,start_date,end_date\nSVC,,1,1,1,1,0,0,20250101,20261231\n"),
 ("CLD_001","calendar_dates.txt","service_id,date,exception_type\n,20250115,2\n"),
 ("FAR_003","fare_attributes.txt","fare_id,price,currency_type,payment_method,transfers\nF1,2.50,,0,0\n"),
 ("FMD_001","fare_media.txt","fare_media_id,fare_media_name,fare_media_type\n,Kart,2\n"),
 ("FPD_003","fare_products.txt","fare_product_id,fare_product_name,amount,currency\nP1,Bilet,2.50,\n"),
 ("FIN_003","feed_info.txt","feed_publisher_name,feed_publisher_url,feed_lang\nT,https://a.com,\n"),
 ("FRQ_002","frequencies.txt","trip_id,start_time,end_time,headway_secs\nT1,,10:00:00,600\n"),
 ("FRQ_001","frequencies.txt","trip_id,start_time,end_time,headway_secs\n,08:00:00,10:00:00,600\n"),
 ("XFL_022","location_group_stops.txt","location_group_id,stop_id\n,S1\n"),
 ("XFL_023","location_group_stops.txt","location_group_id,stop_id\nLG1,\n"),
 ("XFL_031","location_groups.txt","location_group_id,location_group_name\n,Grup\n"),
 ("NET_001","networks.txt","network_id,network_name\n,Ag\n"),
 ("RCT_001","rider_categories.txt","rider_category_id,rider_category_name\n,Yetiskin\n"),
 ("RTS_018","routes.txt","route_id,agency_id,route_short_name,route_type,continuous_drop_off\nR1,A1,1,3,\n"),
 ("SAR_001","stop_areas.txt","area_id,stop_id\n,S1\n"),
 ("SAR_002","stop_areas.txt","area_id,stop_id\nA1,\n"),
 ("TFR_002","timeframes.txt","timeframe_group_id,start_time,end_time,service_id\nTG1,08:00:00,10:00:00,\n"),
 ("TRF_004","transfers.txt","from_stop_id,to_stop_id,transfer_type\nS1,S2,\n"),
 ("TRP_003","trips.txt","route_id,service_id,trip_id\nR1,,T1\n"),
]
BIN="./target/release/gtfs-analyzer"
out=[]
with tempfile.TemporaryDirectory() as td:
    for rule, fname, content in CASES:
        files=dict(BASE); files[fname]=content
        z=pathlib.Path(td)/f"{rule}.zip"
        with zipfile.ZipFile(z,"w") as zf:
            for n,c in files.items(): zf.writestr(n,c)
        r=subprocess.run([BIN,"validate",str(z),"--json","--today","20250601"],
                         capture_output=True,text=True)
        try:
            d=json.loads(r.stdout); d=d.get("Ok",d)
            ids={n["rule_id"] for n in d.get("notices",[])}
        except Exception:
            out.append((rule,"?","koşturulamadı")); continue
        out.append((rule, "ATEŞLER ✅" if rule in ids else "SESSİZ 🔴",
                    ",".join(sorted(i for i in ids if i.split("_")[0]==rule.split("_")[0]))[:56]))
print(f"{'kural':9s} {'boş değerde':12s}  aynı gruptan ateşleyenler")
for r,s,extra in out: print(f"{r:9s} {s:12s}  {extra}")
