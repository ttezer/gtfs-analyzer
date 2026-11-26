import csv, io, zipfile, pathlib, subprocess, json, sys
p=pathlib.Path(sys.argv[1] if len(sys.argv)>1 else '/tmp/gtfs-headway-calendar-fixtures')
p.mkdir(parents=True, exist_ok=True)
M='20261130'; T='20261201'; S='20261205'
def fixture(name, groups, dates, exceptions=(), frequencies=(), peers=0, peer_deps=('08:00','08:15','08:30')):
    files={}
    def table(name, header, rows):
        f=io.StringIO(); w=csv.writer(f); w.writerow(header.split(',')); w.writerows(rows); files[name]=f.getvalue()
    routes=[]; trips=[]; times=[]; fs=[]
    all_groups=[('R', *g) for g in groups]+[(f'P{i}', 'A', peer_deps, 'X','Y') for i in range(peers)]
    for i,(route, svc, deps, start, stop) in enumerate(all_groups):
        if route not in [r[0] for r in routes]: routes.append((route,route,'',3))
        for j,dep in enumerate(deps):
            tid=f't{i}_{j}'; trips.append((route,svc,tid,0))
            h,m=map(int,dep.split(':')); a=h*60+m+5
            times += [(tid,dep+':00',dep+':00',start,1),(tid,f'{a//60:02}:{a%60:02}:00',f'{a//60:02}:{a%60:02}:00',stop,2)]
            for fi, fj, st,en,hw,exact in frequencies:
                if (fi,fj)==(i,j): fs.append((tid,st,en,hw,exact))
    table('agency','agency_id,agency_name,agency_url,agency_timezone',[('A','Test Transit','https://example.org','America/Fortaleza')])
    table('stops','stop_id,stop_name,stop_lat,stop_lon',[(s,s,-3.75+j*.005,-38.54) for j,s in enumerate('XYZW') if any(t[3]==s for t in times)])
    table('routes','route_id,route_short_name,route_long_name,route_type',routes)
    table('trips','route_id,service_id,trip_id,direction_id',trips)
    table('stop_times','trip_id,arrival_time,departure_time,stop_id,stop_sequence',times)
    if dates: table('calendar','service_id,monday,tuesday,wednesday,thursday,friday,saturday,sunday,start_date,end_date',[(s,*flags,start,end) for s,flags,start,end in dates])
    if exceptions: table('calendar_dates','service_id,date,exception_type',exceptions)
    if fs: table('frequencies','trip_id,start_time,end_time,headway_secs,exact_times',fs)
    with zipfile.ZipFile(p/(name+'.zip'),'w',zipfile.ZIP_DEFLATED) as z:
        for n,v in files.items(): z.writestr(n+'.txt',v)
def g(s,*t): return s,t,'X','Y'
def cal(s,start=M,end=M,flags=(1,1,1,1,1,1,1)): return s,flags,start,end
ab=[cal('A'),cal('B')]
nel=[g('A','08:00','08:30','09:00'),g('B','08:15','08:45')]
close=[g('A','08:00','08:30'),g('B','08:01','08:45')]
gap=[g('A','08:00','08:15','16:30'),g('B','10:00','12:00','14:00','16:00')]
for name,grp,dt,ex in [
 ('nelson',nel,ab,[]),('nelson_merged',[g('A','08:00','08:15','08:30','08:45','09:00')],[cal('A')],[]),
 ('weekend',close,[cal('A',M,S,(1,1,1,1,1,0,0)),cal('B',M,S,(0,0,0,0,0,1,1))],[]),
 ('partial',close,[cal('A',M,T),cal('B',T,S)],[]),
 ('add',close,[cal('A'),cal('B',S,S)],[('B',M,1)]),
 ('remove',close,[cal('A'),cal('B',M,S)],[('B',M,2)]),
 ('dates_only',close,[],[('A',M,1),('B',M,1)]),
 ('close',close,ab,[]),
 ('close_merged',[g('A','08:00','08:01','08:30','08:45')],[cal('A')],[]),
 ('same_time',[g('A','08:00','08:30'),g('B','08:00','08:30')],ab,[]),
 ('midnight',[g('A','23:59'),g('B','24:00')],ab,[]),
 ('midnight_merged',[g('A','23:59','24:00')],[cal('A')],[]),
 ('terminals',[g('A','08:00','08:30'),('A',('08:01','08:45'),'Z','Y')],[cal('A')],[]),
 ('patterns',[g('A','08:00','08:30'),('A',('08:01','08:45'),'X','W')],[cal('A')],[]),
 ('gap',gap,ab,[]),('gap_merged',[g('A',*(gap[0][1]+gap[1][1]))],[cal('A')],[]),
 ('gap_cancelled',gap,ab,[('A',M,2)]),
 ('gap_split_single',[g('A','08:00'),g('B','08:15','16:30')],ab,[]),
 ('gap_split_single_merged',[g('A','08:00','08:15','16:30')],[cal('A')],[]),
 ('close_cancelled',[g('A','08:00','08:01'),g('B','09:00')],ab,[('A',M,2)]),
 ]: fixture(name,grp,dt,ex)
# Service U has calendar days but no trips; it must not dilute the daily average.
fixture('unused_service',[g('A','08:00','08:30')],[cal('A'),cal('U',M,S)])
for exact in (0,1):
    fixture(f'freq{exact}',[g('A','00:00','08:01','08:15','08:29')],[cal('A')],frequencies=[(0,0,'08:00:00','08:20:00',420,exact)])
fixture('freq_mixed',[g('A','00:00','08:01','08:15','08:29'),('A',('09:00','09:01'),'Z','W')],[cal('A')],frequencies=[(0,0,'08:00:00','08:20:00',420,0)])
fixture('freq_window_gap',[g('A','00:00','16:00','16:15','23:30')],[cal('A')],frequencies=[(0,0,'08:00:00','16:00:00',900,0)])
fixture('freq_boundary_after',[g('A','00:00','16:00')],[cal('A')],frequencies=[(0,0,'07:00:00','09:00:00',600,0)])
fixture('freq_boundary_before',[g('A','05:00','00:00')],[cal('A')],frequencies=[(0,1,'12:00:00','14:00:00',600,0)])
fixture('freq_boundary_windows',[g('A','00:00')],[cal('A')],frequencies=[(0,0,'07:00:00','09:00:00',600,0),(0,0,'17:00:00','19:00:00',600,0)])
fixture('freq_continuous_windows',[g('A','00:00')],[cal('A')],frequencies=[(0,0,'07:00:00','09:00:00',600,0),(0,0,'09:00:00','11:00:00',600,0)])
fixture('freq_window_bunch',[g('A','00:00','08:01','08:02')],[cal('A')],frequencies=[(0,0,'08:00:00','09:00:00',900,0)])
fixture('freq_pure',[g('A','00:00')],[cal('A')],frequencies=[(0,0,'08:00:00','10:00:00',3600,0)],peers=6)
fixture('freq_split_peers',[g('A','00:00')],[cal('A')],frequencies=[(0,0,'07:00:00','09:00:00',600,0),(0,0,'19:00:00','21:00:00',600,0)],peers=6)
fixture('freq_gap_peers',[g('A','00:00')],[cal('A')],frequencies=[(0,0,'07:00:00','09:00:00',600,0),(0,0,'17:00:00','19:00:00',600,0)],peers=6,peer_deps=('08:00','08:10','08:20'))
fixture('freq_overlap',[g('A','00:00','00:00')],[cal('A')],frequencies=[(0,0,'08:00:00','10:00:00',600,0),(0,1,'08:00:00','10:00:00',600,0)],peers=6)
fixture('freq_mixed_peers',[g('A','00:00','08:05')],[cal('A')],frequencies=[(0,0,'08:00:00','10:00:00',3600,0)],peers=6)
fixture('freq_exact_peers',[g('A','00:00')],[cal('A')],frequencies=[(0,0,'08:00:00','10:00:00',3600,1)],peers=6)
fixture('freq_midnight_exact',[g('A','00:00','24:11')],[cal('A')],frequencies=[(0,0,'23:50:00','24:20:00',600,1)])
fixture('freq_midnight_window',[g('A','00:00','24:11')],[cal('A')],frequencies=[(0,0,'23:50:00','24:20:00',600,0)])
fixture('close_window',close,[cal('A',M,T),cal('B',M,T)])
with zipfile.ZipFile(p/'close_window.zip') as z:
    close_window_files={n:z.read(n) for n in z.namelist()}
close_window_files['feed_info.txt']=(
    'feed_publisher_name,feed_publisher_url,feed_lang,feed_start_date,feed_end_date\n'
    'Test Transit,https://example.org,en,20261201,20261201\n'
).encode()
with zipfile.ZipFile(p/'close_window.zip','w',zipfile.ZIP_DEFLATED) as z:
    for name,body in close_window_files.items(): z.writestr(name,body)
for name, grp in [('outlier_fp',[g('A','08:00','09:00'),g('B','08:15','08:30','08:45')]),('outlier_fp_merged',[g('A','08:00','08:15','08:30','08:45','09:00')]),('outlier_fn',[g('A','08:00','08:15','08:30'),g('B','08:05','08:20','08:35'),g('C','08:10','08:25','08:40')]),('outlier_fn_merged',[g('A','08:00','08:05','08:10','08:15','08:20','08:25','08:30','08:35','08:40')])]:
    fixture(name,grp,[cal(s) for s,*_ in grp],peers=6)
outlier_split=[g('A','08:00','09:00'),g('B','08:15','08:30','08:45')]
fixture('outlier_add',outlier_split,[cal('A'),cal('B',S,S)],[('B',M,1)],peers=6)
fixture('outlier_remove',outlier_split,ab,[('B',M,2)],peers=6)
fixture('outlier_dates_only',outlier_split,[],[('A',M,1),('B',M,1)],peers=6)
fixture('outlier_weekend',outlier_split,[cal('A',M,S,(1,1,1,1,1,0,0)),cal('B',M,S,(0,0,0,0,0,1,1))],peers=6)
departures=tuple(f'{8+i//60:02}:{i%60:02}' for i in range(60))
fixture('route_volume',[g('A',*departures[::2]),g('B',*departures[1::2])],ab)
fixture('route_volume_merged',[g('A',*departures)],[cal('A')])
fixture('route_volume_cancelled',[g('A',*departures),g('B','09:00')],ab,[('A',M,2)])
(p/'volume-config.json').write_text('{"max_trips_per_route":50}')
fixture('unused_calendar',[g('A','08:00')],[cal('A'),cal('U',S,S)])
fixture('dedup',[g('A','08:00','08:01'),g('B','09:00','09:01')],[cal('A'),cal('B',S,S)])
fixture('freq1_expanded',[g('A','08:00','08:07','08:14','08:01','08:15','08:29')],[cal('A')])
def block_variant(source):
    with zipfile.ZipFile(p/(source+'.zip')) as z:
        files={n:z.read(n) for n in z.namelist()}
    lines=files['trips.txt'].decode().splitlines()
    files['trips.txt']=('\n'.join([lines[0]+',block_id',*[line+',V' for line in lines[1:]]])+'\n').encode()
    with zipfile.ZipFile(p/('block_'+source+'.zip'),'w',zipfile.ZIP_DEFLATED) as z:
        for n,v in files.items(): z.writestr(n,v)
for source in ('close','weekend','add','remove','dates_only','close_cancelled'):
    block_variant(source)
duplicate=[g('A','08:00'),g('B','08:00')]
fixture('exception_duplicate',duplicate,[cal('A'),cal('B',S,S)],[('B',M,1)])
fixture('override_config',close,[cal('A',M,'20261203'),cal('B',M,T)],
        [('A',T,2),('A','20261203',2)])
(p/'override-config.json').write_text(json.dumps({'calendar_override_rules':[
    {'route_id':'R','base_service_ids':['A'],'override_service_ids':['B'],
     'start_date':20261130,'end_date':20261203}]}))
if len(sys.argv)>2:
    for z in sorted(p.glob('*.zip')):
        data=json.loads(subprocess.run([sys.argv[2],'validate',str(z),'--json','--today','20261126','--include-name-index',*(['--config',str(p/'volume-config.json')] if z.stem.startswith('route_volume') else ['--config',str(p/'override-config.json')] if z.stem=='override_config' else [])],capture_output=True).stdout)
        (p/(z.stem+'.json')).write_text(json.dumps(data,indent=2))
        print(z.stem,[(n['rule_id'],n.get('entity_id'),n.get('service_id'),n.get('observed_value')) for n in data['notices'] if n['rule_id'] in {'OPR_001','OPR_003','OPR_005','OPR_011','TRP_022','OPR_019','OPR_020','OPR_024','OPR_021','OPR_022','OPR_023','OPR_009'}],{k:data['metrics'][k] for k in ('trip_count','active_service_days','avg_daily_trips')}, [n['rule_id'] for n in data['notices'] if n['rule_class'].lower()=='spec'])
