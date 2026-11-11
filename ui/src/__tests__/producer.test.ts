import { describe, it, expect, vi } from 'vitest';
import { buildProducerModel, stopUsage, entityLabel, plannerArea, routeBadge } from '../pages/producer-helpers';
import { missingMapShapes, plannerFacets, plannerMap, renderProducer } from '../pages/producer';
import type { FacetCard, FacetFilter } from '../pages/producer';
import type { ValidationResult, Notice, NameIndex } from '../types';
import { tMsgForLocale } from '../i18n';

vi.mock('../state', () => ({ setPage: vi.fn(), getState: () => ({ agencyFilter: [] }), setAgencyFilter: vi.fn() }));
vi.mock('../hidden-rules', () => ({
  renderHiddenRulesBar: () => '',
  attachHiddenRuleListeners: () => undefined,
}));

function notice(id: string, rule_id: string, severity: Notice['severity'], extra: Partial<Notice> = {}): Notice {
  return {
    id, rule_id, severity, rule_class: 'SPEC', entity_type: 'Feed', entity_id: null, scope_key: null,
    file: null, line: null, field: null, observed_value: null, expected_value: null, details: null,
    title: '', message: `${rule_id} message`, remediation: '', blocks: [], base_effort: 1,
    ...extra,
  } as Notice;
}

const emptyNameIndex: NameIndex = {
  stops: {}, routes: {}, trips: {}, trip_routes: {}, trip_directions: {}, stop_coords: {},
  trip_first_dep: {}, shape_routes: {}, shape_coords: {}, trip_shapes: {}, trip_stops: {},
  shape_trips: {}, route_shapes: {}, map_data_deferred: false,
};

function result(
  notices: Notice[],
  blockers: string[],
  r9: [string, number, number][],
  coverageComplete = true,
  name_index: NameIndex = emptyNameIndex,
  feed_summary?: ValidationResult['feed_summary'],
): ValidationResult {
  return {
    notices,
    name_index,
    feed_summary,
    reports: {
      r1: { publishable: blockers.length === 0, coverage_complete: coverageComplete, blocker_notice_ids: blockers },
      r9: { items: r9.map(([rule_id, priority_score, affected_instance_count]) => ({ rule_id, priority_score, affected_instance_count, labels: [] })) },
    },
  } as unknown as ValidationResult;
}

describe('buildProducerModel', () => {
  it('engelleri R1 blocker_notice_ids üzerinden kural başına gruplar', () => {
    const r = result(
      [notice('a', 'STM_002', 'CRITICAL'), notice('b', 'STP_018', 'CRITICAL'), notice('c', 'DQ_016', 'MEDIUM')],
      ['a', 'b'],
      [['STM_002', 4, 1], ['STP_018', 4, 1], ['DQ_016', 2, 1]],
    );
    const m = buildProducerModel(r);
    expect(m.publishable).toBe(false);
    expect(m.blockers.map(g => g.ruleId)).toEqual(['STM_002', 'STP_018']);
    expect(m.improvements.map(g => g.ruleId)).toEqual(['DQ_016']);
  });

  it('engel olmayanları R9 önceliğine göre sıralar, INFO kuralları ayrı tutar', () => {
    const r = result(
      [notice('a', 'CAL_014', 'INFO'), notice('b', 'ARC_020', 'LOW'), notice('c', 'ARC_024', 'MEDIUM'), notice('d', 'ARC_024', 'MEDIUM')],
      [],
      [['CAL_014', 0, 2], ['ARC_020', 1, 1], ['ARC_024', 2.21, 9]],
    );
    const m = buildProducerModel(r);
    expect(m.improvements.map(g => g.ruleId)).toEqual(['ARC_024', 'ARC_020']);
    expect(m.improvements[0].count).toBe(9); // R9 affected_instance_count, not the notice count
    expect(m.infos.map(g => g.ruleId)).toEqual(['CAL_014']);
  });

  it('R9 da olmayan (motorun bastırdığı) kuralları listelemez', () => {
    // CAL_024 is blocked by TRP_023: it is in the notice list but has no R9 item.
    const r = result(
      [notice('a', 'TRP_023', 'LOW'), notice('b', 'CAL_024', 'INFO')],
      [],
      [['TRP_023', 2, 1]],
    );
    const m = buildProducerModel(r);
    expect(m.improvements.map(g => g.ruleId)).toEqual(['TRP_023']);
    expect(m.infos).toEqual([]);
  });

  it('aynı kuralın engel olmayan notice ları engel kartına girmez', () => {
    const r = result(
      [notice('a', 'TRP_001', 'CRITICAL'), notice('b', 'TRP_001', 'CRITICAL')],
      ['a'],
      [['TRP_001', 4, 2]],
    );
    const m = buildProducerModel(r);
    expect(m.blockers[0].notices.map(n => n.id)).toEqual(['a']);
    expect(m.improvements).toEqual([]);
  });

  it('feed düzeyi notice grubunu korur ve kapsam kararını R1 den alır', () => {
    const r = result(
      [notice('a', 'ARC_024', 'MEDIUM', { entity_type: 'Feed' })],
      ['a'],
      [['ARC_024', 2, 1]],
      false,
    );
    const m = buildProducerModel(r);
    expect(m.blockers[0].notices[0].entity_type).toBe('Feed');
    expect(m.coverageComplete).toBe(false);
  });
});

describe('NameIndex okumaları', () => {
  const ni: NameIndex = {
    ...emptyNameIndex,
    routes: { R1: 'Dial-a-Ride' },
    trip_routes: { T1: 'R1', T2: 'R1', T3: 'R1' },
    trip_stops: { T1: ['area_803'], T2: ['area_803'], T3: ['S9'] },
    stops: { S9: 'Merkez' },
    trip_first_dep: { T3: '07:15' },
  };

  it('bir durağı kullanan seferleri hat adına göre sayar', () => {
    expect(stopUsage('area_803', ni)).toEqual([{ route: 'Dial-a-Ride', trips: 2 }]);
    expect(stopUsage('yok', ni)).toEqual([]);
  });

  it('varlığı üreticinin tanıdığı adla gösterir, ad yoksa kimliğe düşer', () => {
    expect(entityLabel(notice('x', 'STP_001', 'CRITICAL', { entity_type: 'Stop', entity_id: 'S9' }), ni)).toBe('Merkez (S9)');
    expect(entityLabel(notice('x', 'STM_002', 'CRITICAL', { entity_type: 'Stop', entity_id: 'area_803' }), ni)).toBe('area_803');
    expect(entityLabel(notice('x', 'TRP_011', 'HIGH', { entity_type: 'Trip', entity_id: 'T3' }), ni)).toBe('Dial-a-Ride · 07:15');
    expect(entityLabel(notice('x', 'ARC_020', 'LOW'), ni)).toBeNull();
  });
});

describe('Üretici kartı görünümü', () => {
  it('feed düzeyi satırda gerçekten tüm feed etiketini gösterir', () => {
    const r = result(
      [notice('a', 'ARC_024', 'MEDIUM', { entity_type: 'Feed' })],
      [],
      [['ARC_024', 2, 1]],
    );
    const root = { innerHTML: '', querySelectorAll: () => [], querySelector: () => null } as unknown as HTMLElement;
    renderProducer(root, r);
    expect(root.innerHTML).toContain('tüm feed');
  });

  it('kapsam nedeni için ham motor metnini yeniden göstermeyip yerelleştirilmiş uyarıyı korur', () => {
    const r = result([], [], [], false);
    r.partial = {
      root_structural_errors: ['ZIP kökü hatası — motor metni'],
      unavailable_files: [],
      skipped_stages: [],
    };
    const root = { innerHTML: '', querySelectorAll: () => [], querySelector: () => null } as unknown as HTMLElement;
    renderProducer(root, r);
    expect(root.innerHTML).toContain('Kontrol eksik kaldı');
    expect(root.innerHTML).not.toContain('motor metni');
  });

  it('STM_002 kartında teknik başlık yerine durak değeri ve kullanan hatları gösterir', () => {
    const ni: NameIndex = {
      ...emptyNameIndex,
      routes: { R1: 'Dial-a-Ride' },
      trip_routes: { T1: 'R1', T2: 'R1' },
      trips: { T1: 'Talep üzerine ulaşım' },
      service_days: { SVC1: ['1111100', '20220101', '20230101', '0'] },
      trip_stops: { T1: ['area_803'], T2: ['area_803'] },
    };
    const r = result(
      [notice('a', 'STM_002', 'CRITICAL', {
        entity_type: 'Stop', entity_id: 'area_803', details: {
          trip_count: '1', example_1_trip_id: 'T1', example_1_route_id: 'R1',
          example_1_service_id: 'SVC1', example_1_service_name: 'Year round (Weekday)',
          example_1_window_start: '06:00:00', example_1_window_end: '21:00:00',
          geojson_location: 'true',
        },
      })],
      ['a'],
      [['STM_002', 4, 1]],
      true,
      ni,
    );
    const root = { innerHTML: '', querySelectorAll: () => [], querySelector: () => null } as unknown as HTMLElement;
    renderProducer(root, r);
    expect(root.innerHTML).toContain('Seferde kullanılan durak bulunamadı');
    expect(root.innerHTML).not.toContain('Bu kodu kullanan hatlar');
    // The route is shown by the number riders know (here its name), never by route_id.
    expect(root.innerHTML).not.toContain('Hat kodu R1');
    expect(root.innerHTML).toContain('Year round (Weekday)');
    expect(root.innerHTML).not.toContain('planner-route-code');
    expect(root.innerHTML).toContain('Hafta içi');
    expect(root.innerHTML).toContain('alış-bırakış aralığı 06:00–21:00');
    expect(root.innerHTML).toContain('talebe bağlı hizmetin yolcu alma ve bırakma bölgesi; sefer bu bölgeyi 06:00–21:00 boyunca kullanıyor');
    expect(root.innerHTML).toContain('güncel GTFS-Flex biçimine');
    expect(root.innerHTML).not.toContain('takvim kodu');
    expect(root.innerHTML).toContain('<th>Hat</th><th>Yön</th><th>Sefer başlangıç saati</th><th>Çalışma günleri</th>');
    expect(root.innerHTML).toContain('Talep üzerine ulaşım');
    expect(root.innerHTML).not.toContain('stop_id bulunamadı');
  });

  it('STM_047 kartında hat ve ilk kalkış bağlamını gösterir', () => {
    const ni: NameIndex = {
      ...emptyNameIndex,
      routes: { R6: '6' },
      trip_routes: { T1: 'R6', T2: 'R6', T3: 'R6', T4: 'R6', T5: 'R6' },
      trip_first_dep: { T1: '07:15', T2: '07:45', T3: '08:15', T4: '08:45', T5: '09:15' },
    };
    const r = result(
      [
        notice('a', 'STM_047', 'CRITICAL', { entity_type: 'Trip', entity_id: 'T1' }),
        notice('b', 'STM_047', 'CRITICAL', { entity_type: 'Trip', entity_id: 'T2' }),
        notice('c', 'STM_047', 'CRITICAL', { entity_type: 'Trip', entity_id: 'T3' }),
        notice('d', 'STM_047', 'CRITICAL', { entity_type: 'Trip', entity_id: 'T4' }),
        notice('e', 'STM_047', 'CRITICAL', { entity_type: 'Trip', entity_id: 'T5' }),
      ],
      ['a', 'b', 'c', 'd', 'e'],
      [['STM_047', 5, 5]],
      true,
      ni,
    );
    const root = { innerHTML: '', querySelectorAll: () => [], querySelector: () => null } as unknown as HTMLElement;
    renderProducer(root, r);
    expect(root.innerHTML).toContain('Bazı seferlerde kesin durak saatleri eksik');
    expect(root.innerHTML).toContain('6 hattı · sefer başlangıcı 07:15, 07:45, 08:15… · 5 sefer');
    // The first three departures are summarized in the card context; the detailed table now lives inside the card.
    expect(root.innerHTML).toContain('07:15, 07:45, 08:15…');
  });

  it('STM_056 kartında hatalı durağın komşularını, yönü ve takvimini gösterir', () => {
    const ni: NameIndex = {
      ...emptyNameIndex,
      stops: { S0: 'Daha önceki', S1: 'Önceki', S2: 'Hatalı', S3: 'Sonraki', S4: 'Daha sonraki' },
      routes: { R1: '10' },
      trips: { T1: 'Merkez yönü' },
      trip_routes: { T1: 'R1' },
      trip_first_dep: { T1: '07:15' },
      trip_stops: { T1: ['S0', 'S1', 'S2', 'S3', 'S4'] },
      service_days: { WKD: ['1111100', '20260101', '20261231', '0'] },
      stop_coords: { S1: [41, 29], S2: [41.001, 29.001], S3: [41.002, 29.002] },
    };
    const r = result(
      [notice('a', 'STM_056', 'CRITICAL', {
        entity_type: 'Feed', file: 'stop_times.txt', field: 'shape_dist_traveled',
        observed_value: '51', details: {
          affected_rows: '51', example_trips: 'T1',
          example_0_trip: 'T1', example_0_line: '42',
          example_0_service_id: 'WKD',
          example_0_prev2_stop_id: 'S0', example_0_prev_stop_id: 'S1', example_0_stop_id: 'S2', example_0_next_stop_id: 'S3', example_0_next2_stop_id: 'S4',
          example_0_prev2_stop_sequence: '0', example_0_prev2_shape_dist_traveled: '90',
          example_0_prev_stop_sequence: '1', example_0_stop_sequence: '2', example_0_next_stop_sequence: '3',
          example_0_next2_stop_sequence: '4', example_0_prev_shape_dist_traveled: '100', example_0_shape_dist_traveled: '100', example_0_next_shape_dist_traveled: '120', example_0_next2_shape_dist_traveled: '130',
          example_0_prev2_departure: '07:58:30', example_0_arrival: '08:04:30', example_0_next2_arrival: '08:09:30',
        },
      })],
      ['a'], [['STM_056', 5, 51]], true, ni,
    );
    const root = { innerHTML: '', querySelectorAll: () => [], querySelector: () => null } as unknown as HTMLElement;
    renderProducer(root, r);
    expect(root.innerHTML).toContain('Sefer duraklarında güzergâh mesafesi artmıyor');
    expect(root.innerHTML).toContain('Önceki');
    expect(root.innerHTML).toContain('Hatalı');
    expect(root.innerHTML).toContain('Sonraki');
    expect(root.innerHTML).toContain('Daha önceki');
    expect(root.innerHTML).toContain('Daha sonraki');
    expect(root.innerHTML).toContain('Durak gerçekten yanlışsa durak sırasını düzeltin');
    expect(root.innerHTML).toContain('<th>Varış</th>');
    expect(root.innerHTML).toContain('<th>Kalkış</th>');
    expect(root.innerHTML).not.toContain('Kaynak satırı');
    expect(root.innerHTML).not.toContain('<th>Durum</th>');
    expect(root.innerHTML).toContain('<th>Çalışma günleri</th>');
    expect(root.innerHTML).toContain('Hafta içi');
    // Example rows keep every neighbour time, not only the keys of the generic example mapping.
    expect(root.innerHTML).toContain('07:58:30');
    expect(root.innerHTML).toContain('08:04:30');
    expect(root.innerHTML).toContain('08:09:30');
  });

  it('STM_024 kartı iki mesafeyi haritadaki uzunlukla karşılaştırıp birimi farklı olan tarafı işaretler', () => {
    const ni: NameIndex = {
      ...emptyNameIndex,
      stops: { S1: 'Garaj', S2: 'Terminal' },
      routes: { R1: '10' },
      trips: { T1: 'Terminal yönü', T2: 'Garaj yönü' },
      trip_routes: { T1: 'R1', T2: 'R1' },
      trip_first_dep: { T1: '08:00', T2: '09:00' },
      trip_stops: { T1: ['S1', 'S2'], T2: ['S2', 'S1'] },
      stop_coords: { S1: [41.0, 29.0], S2: [41.1, 29.1] },
    };
    const trip = notice('a', 'STM_024', 'INFO', {
      entity_type: 'Trip', entity_id: 'T1', file: 'stop_times.txt', field: 'shape_dist_traveled',
      observed_value: '13.90', expected_value: '13900.00',
      details: {
        shape_id: 'SH1', stop_times_max: '13.90', shape_max: '13900.00', ratio: '0.001',
        shape_length_m: '13950', stop_id: 'S2', stop_sequence: '2', affected_trips: '1',
      },
    });
    const r = result([trip], [], [['STM_024', 1, 1]], true, ni);
    const root = { innerHTML: '', querySelectorAll: () => [], querySelector: () => null } as unknown as HTMLElement;
    renderProducer(root, r);
    const html = root.innerHTML;
    expect(html).toContain('Bu seferin güzergâh çizgisi haritada yaklaşık 14 km');
    expect(html).toContain('(metreyle uyumlu)');
    expect(html).toContain('(kilometreyle uyumlu)');
    expect(html).toContain('<th>Haritadaki uzunluğa oranı</th>');
    // The stop-times row is the odd one out: it matches kilometres while the map length is metres.
    expect(html).toContain('<tr class="bad"><td>Durak saatleri</td><td>13,9</td><td>0,000996</td><td>kilometreyle uyumlu</td></tr>');
    expect(html).toContain('<tr><td>Güzergâh çizgisi</td>');
    expect(html).toContain('Terminal · durak saatlerinde 13,9, çizgide 13.900');
    const red = plannerMap(trip, r).opts?.pins.filter(pin => pin.primary) ?? [];
    expect(red.map(pin => pin.label)).toEqual([expect.stringContaining('Terminal')]);

    const summary = notice('b', 'STM_024', 'INFO', {
      entity_type: 'Feed', file: 'stop_times.txt', field: 'shape_dist_traveled', observed_value: '2',
      details: {
        message_variant: 'feed_summary', affected_shapes: '2', affected_trips: '3',
        example_0_trip: 'T1', example_0_shape_id: 'SH1', example_0_stop_times_max: '13.90', example_0_shape_max: '13900.00', example_0_shape_length_m: '13950', example_0_stop_id: 'S2', example_0_stop_sequence: '2',
        example_1_trip: 'T2', example_1_shape_id: 'SH2', example_1_stop_times_max: '13.90', example_1_shape_max: '13900.00', example_1_shape_length_m: '13950', example_1_stop_id: 'S1', example_1_stop_sequence: '2',
      },
    });
    expect(tMsgForLocale('en', summary)).toContain('2 shapes have an abnormal shape_dist_traveled ratio');
    expect(tMsgForLocale('en', summary)).toContain('(3 trips)');
    const rs = result([summary], [], [['STM_024', 1, 2]], true, ni);
    const root2 = { innerHTML: '', querySelectorAll: () => [], querySelector: () => null } as unknown as HTMLElement;
    renderProducer(root2, rs);
    expect(root2.innerHTML).toContain('Aynı fark 2 güzergâh çizgisinde (3 sefer) var');
    expect(root2.innerHTML).toContain('Garaj · durak saatlerinde 13,9, çizgide 13.900');
    expect(root2.innerHTML).toContain('Terminal · durak saatlerinde 13,9, çizgide 13.900');
  });

  it('STM_021 aynı durak çiftini kullanan seferleri tek satırda toplar ve geçerlilik tarihiyle listeler', () => {
    const ni: NameIndex = {
      ...emptyNameIndex,
      stops: { S1: 'Ferry Waypoint', S2: 'Ferry terminal' },
      routes: { R1: 'N1376' },
      trips: {},
      trip_routes: { T1: 'R1', T2: 'R1', T3: 'R1' },
      trip_first_dep: { T1: '18:00', T2: '18:00', T3: '17:45' },
      trip_stops: { T1: ['S1', 'S2'], T2: ['S1', 'S2'], T3: ['S2', 'S1'] },
      stop_coords: { S1: [54.655054, 11.354309], S2: [54.655054, 11.354309] },
      service_days: {
        APR: ['1111111', '20260401', '20260430', '0'],
        MAY: ['1111111', '20260501', '20260531', '0'],
        MAR: ['1100001', '20260329', '20260331', '0'],
      },
    };
    const n = notice('a', 'STM_021', 'LOW', {
      entity_type: 'Trip', entity_id: 'T1', field: 'stop_id', observed_value: 'S1 → S2', expected_value: '> 0 m', service_id: 'APR',
      details: {
        stop_a: 'S1', stop_b: 'S2', stop_a_sequence: '7', stop_b_sequence: '8', affected_trips: '3', affected_rows: '3',
        example_0_trip: 'T1', example_0_stop_a: 'S1', example_0_stop_b: 'S2', example_0_stop_a_sequence: '7', example_0_stop_b_sequence: '8', example_0_service_id: 'APR',
        example_1_trip: 'T2', example_1_stop_a: 'S1', example_1_stop_b: 'S2', example_1_stop_a_sequence: '7', example_1_stop_b_sequence: '8', example_1_service_id: 'MAY',
        example_2_trip: 'T3', example_2_stop_a: 'S2', example_2_stop_b: 'S1', example_2_stop_a_sequence: '6', example_2_stop_b_sequence: '7', example_2_service_id: 'MAR',
      },
    });
    const r = result([n], [], [['STM_021', 1, 1]], true, ni);
    const root = { innerHTML: '', querySelectorAll: () => [], querySelector: () => null } as unknown as HTMLElement;
    renderProducer(root, r);
    const html = root.innerHTML;
    // One finding, one row: the summary is not expanded back into its trips.
    expect((html.match(/<tr[^>]*data-row="/g) ?? []).length).toBe(1);
    expect(html).toContain('Bu iki durak 3 seferde ardışık kullanılıyor');
    expect(html).toContain('Bu durak çiftini kullanan seferler (3)');
    expect(html).toContain('<th>Geçerlilik</th>');
    expect(html).toContain('<td>18:00</td><td>Ferry Waypoint (7) → Ferry terminal (8)</td><td>Her gün</td><td>01.04.2026 – 30.04.2026</td>');
    expect(html).toContain('<td>18:00</td><td>Ferry Waypoint (7) → Ferry terminal (8)</td><td>Her gün</td><td>01.05.2026 – 31.05.2026</td>');
    expect(html).toContain('<td>17:45</td><td>Ferry terminal (6) → Ferry Waypoint (7)</td>');
    expect(html).toContain('29.03.2026 – 31.03.2026');
  });

  it('STM_025 ve STM_026 kartları bölümün duraklarını sıra, saat, koordinat ve ölçülen değerle gösterir', () => {
    const ni: NameIndex = {
      ...emptyNameIndex,
      stops: { S0: 'Başlangıç', S1: 'Çarşı', S2: 'Çarşı Karşı', S3: 'Uzak Kent' },
      routes: { R1: '10' },
      trips: { T1: 'Kent yönü' },
      trip_routes: { T1: 'R1' },
      trip_first_dep: { T1: '07:55' },
      trip_stops: { T1: ['S0', 'S1', 'S2', 'S3'] },
      stop_coords: { S0: [40.99, 29.0], S1: [41.0, 29.0], S2: [41.0005, 29.0], S3: [42.0, 29.0] },
    };
    const short = notice('a', 'STM_025', 'INFO', {
      entity_type: 'Trip', entity_id: 'T1', observed_value: '5s', expected_value: 'inceleyin',
      details: {
        stop_a: 'S1', stop_b: 'S2', duration_seconds: '5', distance_meters: '56',
        stop_a_sequence: '2', stop_a_arrival: '08:00:00', stop_a_departure: '08:00:00',
        stop_b_sequence: '3', stop_b_arrival: '08:00:05', stop_b_departure: '08:00:05',
        prev_stop_id: 'S0', prev_stop_sequence: '1', next_stop_id: 'S3', next_stop_sequence: '4',
      },
    });
    const root = { innerHTML: '', querySelectorAll: () => [], querySelector: () => null } as unknown as HTMLElement;
    renderProducer(root, result([short], [], [['STM_025', 1, 1]], true, ni));
    let html = root.innerHTML;
    expect(html).toContain('Çarşı (durak sırası 2) → Çarşı Karşı (durak sırası 3) arası planlı süre 5 sn; iki durak kuş uçuşu 56 m arayla');
    // Seconds stay visible: a 5-second gap is the finding.
    expect(html).toContain('<tr class="bad"><td>Çarşı Karşı</td><td>3</td><td>08:00:05</td><td>08:00:05</td><td>56</td></tr>');
    expect(html).toContain('<td>Başlangıç</td><td>1</td>');
    expect(html).toContain('Çarşı → Çarşı Karşı · 5 sn');

    const long = notice('b', 'STM_026', 'INFO', {
      entity_type: 'Trip', entity_id: 'T1', observed_value: '111.4 km', expected_value: '<= 50 km',
      details: {
        stop_a: 'S2', stop_b: 'S3', distance_meters: '111139', threshold_km: '50',
        stop_a_sequence: '3', stop_b_sequence: '4', prev_stop_id: 'S1', prev_stop_sequence: '2',
      },
    });
    renderProducer(root, result([long], [], [['STM_026', 1, 1]], true, ni));
    html = root.innerHTML;
    expect(html).toContain('Çarşı Karşı (durak sırası 3) ile hemen ardından gelen Uzak Kent (durak sırası 4) arası kuş uçuşu 111,1 km; bu hat türü için eşik 50 km');
    expect(html).toContain('<th>Enlem</th>');
    expect(html).toContain('<tr class="bad"><td>Uzak Kent</td><td>4</td><td>42</td><td>29</td>');
    expect(html).toContain('Çarşı Karşı → Uzak Kent · 111,1 km');
    const red = plannerMap(long, result([long], [], [['STM_026', 1, 1]], true, ni)).opts?.pins.filter(pin => pin.primary) ?? [];
    expect(red).toHaveLength(2);
  });

  it('STM_028, STM_029 ve STM_030 kartları yeri, ölçüyü ve seferi hat yönüyle gösterir', () => {
    const ni: NameIndex = {
      ...emptyNameIndex,
      stops: { S1: 'Ankara', S2: 'Sivas', S3: 'Erzincan', S4: 'Kars' },
      routes: { R1: 'Doğu' },
      trips: { T1: 'Kars yönü' },
      trip_routes: { T1: 'R1' },
      trip_first_dep: { T1: '08:00' },
      trip_stops: { T1: ['S1', 'S2', 'S3', 'S4'] },
      trip_stop_sequences: { T1: [1, 2, 3, 4] },
      stop_coords: { S1: [39.9, 32.8], S2: [39.7, 37.0], S3: [39.7, 39.5], S4: [40.6, 43.1] },
    };
    const render = (n: Notice) => {
      const root = { innerHTML: '', querySelectorAll: () => [], querySelector: () => null } as unknown as HTMLElement;
      const r = result([n], [], [[n.rule_id, 1, 1]], true, ni);
      renderProducer(root, r);
      return { html: root.innerHTML, map: plannerMap(n, r) };
    };
    const long = notice('a', 'STM_028', 'HIGH', {
      entity_type: 'Trip', entity_id: 'T1', observed_value: '26:26:00', expected_value: '24:00:00',
      details: {
        first_stop_id: 'S1', first_stop_sequence: '1', first_departure: '08:00:00',
        last_stop_id: 'S4', last_stop_sequence: '4', last_arrival: '34:26:00', stop_count: '4',
        gap_stop_a: 'S2', gap_stop_b: 'S3', gap_a_sequence: '2', gap_b_sequence: '3',
        gap_from: '14:10:00', gap_to: '38:05:00', gap_seconds: '86100',
      },
    });
    let out = render(long);
    expect(out.html).toContain('Ankara 08:00 → Kars 34:26: 4 duraklı sefer 26 sa 26 dk sürüyor; bu hat türü için üst sınır 24 sa');
    expect(out.html).toContain('En uzun ara Sivas 14:10 → Erzincan 38:05 (23 sa 55 dk)');
    expect(out.html).toContain('<tr class="bad"><td>Sivas</td><td>2</td><td>—</td><td>14:10</td></tr>');
    expect(out.html).toContain('Sivas 14:10 → Erzincan 38:05 · 23 sa 55 dk');
    // Route and direction: the trip path with arrows, the long step in red.
    expect(out.map.opts?.extraPolylines?.some(line => line.arrows)).toBe(true);
    expect(out.map.opts?.pins.filter(pin => pin.primary).map(pin => pin.label)).toEqual([
      expect.stringContaining('Sivas'), expect.stringContaining('Erzincan'),
    ]);

    const short = notice('b', 'STM_029', 'MEDIUM', {
      entity_type: 'Trip', entity_id: 'T1', observed_value: '0s', expected_value: '≥ 60s',
      details: {
        first_stop_id: 'S1', first_stop_sequence: '1', first_departure: '08:00:00',
        last_stop_id: 'S4', last_stop_sequence: '4', last_arrival: '08:00:00',
        stop_count: '4', duration_seconds: '0', distance_meters: '8400',
      },
    });
    out = render(short);
    expect(out.html).toContain('Ankara 08:00 → Kars 08:00: 4 duraklı sefer 0 sn sürüyor; ilk ve son durak kuş uçuşu 8,4 km uzakta');
    expect(out.map.opts?.pins.filter(pin => pin.primary)).toHaveLength(2);

    const negative = notice('c', 'STM_030', 'LOW', {
      entity_type: 'Trip', entity_id: 'T1', observed_value: '-12.5', expected_value: '>= 0',
      details: {
        stop_id: 'S2', stop_sequence: '2', prev_stop_id: 'S1', prev_stop_sequence: '1', prev_shape_dist_traveled: '0',
        next_stop_id: 'S3', next_stop_sequence: '3', next_shape_dist_traveled: '410',
      },
    });
    out = render(negative);
    expect(out.html).toContain('Kümülatif mesafe geçersiz');
    expect(out.html).toContain('Sivas durağında (durak sırası 2) güzergâh başından itibaren kümülatif mesafe “-12.5” yazılmış');
    expect(out.html).toContain('<tr class="bad"><td>Sivas</td><td>2</td><td>-12.5</td></tr>');
    expect(out.html).toContain('<td>Erzincan</td><td>3</td><td>410</td>');
    expect(out.map.opts?.pins.filter(pin => pin.primary).map(pin => pin.label)).toEqual([expect.stringContaining('Sivas')]);
  });

  it('STM_032–040 kartları durağı, sırayı, saati ve talebe bağlı bölgeyi gösterir', () => {
    const ni: NameIndex = {
      ...emptyNameIndex,
      stops: { S1: 'Hastane', S2: 'Okul' },
      routes: { R1: '10' }, trips: { T1: 'Merkez' }, trip_routes: { T1: 'R1' }, trip_first_dep: { T1: '08:00' },
      trip_stops: { T1: ['S1', 'S2'] }, stop_coords: { S1: [41, 29], S2: [41.01, 29.01] },
    };
    const render = (n: Notice) => {
      const root = { innerHTML: '', querySelectorAll: () => [], querySelector: () => null } as unknown as HTMLElement;
      const r = result([n], [], [[n.rule_id, 1, 1]], true, ni);
      renderProducer(root, r);
      return { html: root.innerHTML, map: plannerMap(n, r) };
    };
    const trip = (rule: string, extra: Partial<Notice>) => notice('a', rule, 'LOW', { entity_type: 'Trip', entity_id: 'T1', ...extra });

    let out = render(trip('STM_032', { observed_value: '1', field: 'stop_sequence', details: {
      first_stop_id: 'S1', first_arrival: '08:00:00', first_departure: '08:00:00',
      second_stop_id: 'S2', second_arrival: '08:10:00', second_departure: '08:10:00',
    } }));
    expect(out.html).toContain('Bu seferde durak sırası 1 iki kez kullanılmış: Hastane (08:00) ve Okul (08:10)');
    expect(out.html).toContain('Hastane / Okul · Durak sırası 1');
    expect(out.map.opts?.pins.filter(p => p.primary)).toHaveLength(2);

    out = render(trip('STM_033', { details: { stop_id: 'S2', stop_sequence: '7', departure: '08:00:00' } }));
    expect(out.html).toContain('Bu seferin yalnız bir durağı var: Okul (durak sırası 7, 08:00)');

    out = render(trip('STM_034', { field: 'departure_time', details: { stop_id: 'S2', stop_sequence: '2', arrival: '08:10:00' } }));
    expect(out.html).toContain('Okul durağında (durak sırası 2) varış 08:10 girilmiş, kalkış saati boş');
    expect(out.html).toContain('<tr class="bad"><td>Okul</td><td>2</td><td>08:10</td><td>—</td></tr>');

    out = render(trip('STM_035', { details: {
      stop_id: 'S2', stop_a_sequence: '2', stop_b_sequence: '3', stop_a_departure: '08:10:00', stop_b_arrival: '08:15:00',
      prev_stop_id: 'S1', prev_stop_sequence: '1',
    } }));
    expect(out.html).toContain('Okul durağı seferde arka arkaya iki kez geçiyor: durak sırası 2 (08:10) ve 3 (08:15)');
    expect(out.html).toContain('Okul · Durak sırası 2–3');

    const flex = { location_id: 'Z1', stop_sequence: '1', window_start: '10:00:00', window_end: '09:00:00', arrival: '08:00:00' };
    out = render(trip('STM_037', { field: 'arrival_time', details: flex }));
    expect(out.html).toContain('Bölge Z1 için talebe bağlı hizmet penceresi 10:00–09:00, ama aynı satıra sabit saat de girilmiş (08:00)');
    expect(out.html).toContain('<th>Sabit saat (varış / kalkış)</th>');
    expect(out.html).toContain('<td>10:00–09:00</td><td>08:00 / —</td>');
    // At most four columns: a wider table pushed past the card.
    const flexHeads = (html: string) => (html.match(/planner-flex-times"><thead><tr>(.*?)<\/tr>/)?.[1].match(/<th>/g) ?? []).length;
    expect(flexHeads(out.html)).toBeLessThanOrEqual(4);
    out = render(trip('STM_038', { field: 'start_pickup_drop_off_window', details: flex }));
    expect(out.html).toContain('hizmet penceresi 10:00 → 09:00: bitiş başlangıçtan sonra değil');
    expect(out.html).toContain('<th>Pencere başlangıcı</th><th>Pencere bitişi</th>');
    expect(flexHeads(out.html)).toBeLessThanOrEqual(4);
    out = render(trip('STM_039', { field: 'end_pickup_drop_off_window', details: { location_id: 'Z2', window_start: '11:00:00' } }));
    expect(out.html).toContain('Bölge Z2 için hizmet penceresinin bitiş saati girilmemiş');
    out = render(trip('STM_040', { field: 'pickup_booking_rule_id', details: flex }));
    expect(out.html).toContain('yolcunun nasıl rezervasyon yapacağı tanımlanmamış');
    expect(out.html).toContain('<th>Rezervasyon kuralı</th>');
    expect(flexHeads(out.html)).toBeLessThanOrEqual(4);
  });

  it('STM_035 hattın seferlerini listeler; STM_033–035 haritası hat yönü ve hatalı durakla çizilir', () => {
    const ni: NameIndex = {
      ...emptyNameIndex,
      stops: { S1: 'Hastane', S2: 'Okul', S3: 'Çarşı' },
      routes: { R1: '10' }, trips: { T1: 'Merkez', T2: 'Merkez' }, trip_routes: { T1: 'R1', T2: 'R1' },
      trip_first_dep: { T1: '08:00', T2: '09:00' },
      trip_stops: { T1: ['S1', 'S2', 'S3'], T2: ['S1', 'S2', 'S3'] },
      trip_stop_sequences: { T1: [1, 2, 4], T2: [1, 2, 4] },
      stop_coords: { S1: [41, 29], S2: [41.01, 29.01], S3: [41.02, 29.02] },
      service_days: { APR: ['1111111', '20260401', '20260430', '0'], MAY: ['1111111', '20260501', '20260531', '0'] },
    };
    const n035 = notice('a', 'STM_035', 'INFO', {
      entity_type: 'Trip', entity_id: 'T1', observed_value: 'S2', details: {
        stop_id: 'S2', route_id: 'R1', stop_a_sequence: '2', stop_b_sequence: '3', stop_a_departure: '08:10:00', stop_b_arrival: '08:15:00',
        prev_stop_id: 'S1', prev_stop_sequence: '1', next_stop_id: 'S3', next_stop_sequence: '4', affected_trips: '2', affected_rows: '2',
        example_0_trip: 'T1', example_0_stop_a_sequence: '2', example_0_stop_b_sequence: '3', example_0_stop_a_departure: '08:10:00', example_0_stop_b_arrival: '08:15:00', example_0_service_id: 'APR',
        example_1_trip: 'T2', example_1_stop_a_sequence: '2', example_1_stop_b_sequence: '3', example_1_stop_a_departure: '09:10:00', example_1_stop_b_arrival: '09:15:00', example_1_service_id: 'MAY',
      },
    });
    const root = { innerHTML: '', querySelectorAll: () => [], querySelector: () => null } as unknown as HTMLElement;
    const r = result([n035], [], [['STM_035', 1, 1]], true, ni);
    renderProducer(root, r);
    expect(root.innerHTML).toContain('Bu durak hattın 2 seferinde arka arkaya iki kez geçiyor');
    expect(root.innerHTML).toContain('Bu durağın arka arkaya geçtiği seferler (2)');
    expect(root.innerHTML).toContain('<td>2–3 · 09:10 → 09:15</td><td>Her gün</td><td>01.05.2026 – 31.05.2026</td>');
    const map = plannerMap(n035, r);
    // The red pin carries the stop's position in the trip, not only its name.
    expect(map.opts?.pins.filter(p => p.primary).map(p => p.label)).toEqual([expect.stringContaining('Okul (S2) · durak sırası 2')]);
    expect(map.opts?.pins.some(p => !p.primary && p.label.includes('Hastane'))).toBe(true);
    expect(map.opts?.extraPolylines?.some(line => line.arrows)).toBe(true);

    for (const rule of ['STM_033', 'STM_034']) {
      const n = notice('b', rule, 'LOW', { entity_type: 'Trip', entity_id: 'T1', details: { stop_id: 'S3', stop_sequence: '4' } });
      const m = plannerMap(n, result([n], [], [[rule, 1, 1]], true, ni));
      expect(m.opts?.pins.filter(p => p.primary).map(p => p.label), rule).toEqual([expect.stringContaining('Çarşı (S3) · durak sırası 4')]);
      expect(m.opts?.extraPolylines?.some(line => line.arrows), rule).toBe(true);
    }
  });

  it('STM_036 özeti her seferin hangi durumda olduğunu ve yerini söyler', () => {
    const ni: NameIndex = {
      ...emptyNameIndex,
      stops: { S1: 'Hastane', S2: 'Okul' }, routes: { R1: '10' },
      trips: { T1: 'Merkez', T2: 'Merkez' }, trip_routes: { T1: 'R1', T2: 'R1' }, trip_first_dep: { T1: '08:00', T2: '07:00' },
    };
    const summary = notice('a', 'STM_036', 'INFO', {
      entity_type: 'Feed', file: 'stop_times.txt', field: 'trip_id', observed_value: '2', details: {
        affected_trips: '2', example_trips: 'T1, T2', order_trips: '1', scattered_trips: '1',
        example_0_trip: 'T1', example_0_line: '4', example_0_kind: 'order', example_0_curr_stop: 'S1', example_0_curr_seq: '1', example_0_prev_stop: 'S2', example_0_prev_seq: '2',
        example_1_trip: 'T2', example_1_line: '2', example_1_kind: 'scattered', example_1_line_from: '2', example_1_line_to: '5',
      },
    });
    const root = { innerHTML: '', querySelectorAll: () => [], querySelector: () => null } as unknown as HTMLElement;
    renderProducer(root, result([summary], [], [['STM_036', 1, 2]], true, ni));
    const html = root.innerHTML;
    expect(html).toContain('Hastane (durak sırası 1) dosyada Okul (durak sırası 2) satırından sonra yazılmış');
    expect(html).toContain('2 sefer böyle: 1 seferde durak sırası geriye gidiyor, 1 seferde satırlar dağınık');
    expect(html).toContain('Hastane, Okul satırından sonra');
    expect(html).toContain('satır 2–5');
  });

  it('STM_017 kartı seferin bütün durakları eksikse kaldırılacak başka değer olmadığını söyler', () => {
    const ni: NameIndex = { ...emptyNameIndex, stops: { S1: 'Firals' }, routes: { R1: 'L1' }, trip_routes: { T1: 'R1' }, trip_stops: { T1: ['S1'] } };
    const n = notice('a', 'STM_017', 'MEDIUM', {
      entity_type: 'Trip', entity_id: 'T1', field: 'shape_dist_traveled',
      details: { stop_count: '44', affected_stop_count: '44', missing_0_stop_id: 'S1', missing_0_stop_sequence: '1' },
    });
    const root = { innerHTML: '', querySelectorAll: () => [], querySelector: () => null } as unknown as HTMLElement;
    renderProducer(root, result([n], [], [['STM_017', 2, 1]], true, ni));
    expect(root.innerHTML).toContain('Bu seferin hiçbir durağında kümülatif mesafe yok');
    expect(root.innerHTML).not.toContain('diğer mesafe değerlerini de kaldırın');
  });

  it('STM_056 haritası hatalı geçişin iki durağını kırmızı işaretler ve daire içine alır', () => {
    const ni: NameIndex = {
      ...emptyNameIndex,
      stops: { S40: '8849 PAZ', S41: 'GENERAL PAZ 7597', S42: '9061 PAZ', S43: 'GENERAL PAZ 7745' },
      routes: { R1: '28F' }, trips: { T1: 'Liniers' }, trip_routes: { T1: 'R1' },
      trip_stops: { T1: ['S40', 'S41', 'S42', 'S43'] }, trip_stop_sequences: { T1: [40, 41, 42, 43] },
      // 41 and 42 almost on one point: the red step alone would be invisible.
      stop_coords: { S40: [-34.6, -58.5], S41: [-34.61, -58.5], S42: [-34.61001, -58.50001], S43: [-34.62, -58.5] },
    };
    const n = notice('a', 'STM_056', 'CRITICAL', {
      entity_type: 'Trip', entity_id: 'T1', observed_value: '17550', details: {
        prev_stop_id: 'S41', prev_stop_sequence: '41', stop_id: 'S42', stop_sequence: '42',
        next_stop_id: 'S43', next_stop_sequence: '43', prev_shape_dist_traveled: '17550', shape_dist_traveled: '17550',
      },
    });
    const map = plannerMap(n, result([n], ['a'], [['STM_056', 5, 1]], true, ni));
    const red = map.opts?.pins.filter(p => p.primary).map(p => p.label) ?? [];
    expect(red).toEqual([expect.stringContaining('durak sırası 41'), expect.stringContaining('durak sırası 42')]);
    expect(map.opts?.extraPolylines?.some(line => line.color === '#f59e0b')).toBe(false);
    expect(map.opts?.highlights).toHaveLength(1);
    expect(map.opts?.highlights?.[0].radiusMeters).toBe(60);
    expect(map.caption).toContain('durak sırası 41 → 42');
  });

  it('STM_041–047 kartları durağı, sırayı, saati ve seferin uçlarını gösterir', () => {
    const ni: NameIndex = {
      ...emptyNameIndex,
      stops: { S1: 'Hastane', S2: 'Okul', S3: 'Çarşı' },
      routes: { R1: '10' }, trips: { T1: 'Merkez' }, trip_routes: { T1: 'R1' }, trip_first_dep: { T1: '08:00' },
      trip_stops: { T1: ['S1', 'S2', 'S3'] }, stop_coords: { S1: [41, 29], S2: [41.01, 29.01], S3: [41.02, 29.02] },
    };
    const render = (n: Notice) => {
      const root = { innerHTML: '', querySelectorAll: () => [], querySelector: () => null } as unknown as HTMLElement;
      const r = result([n], [], [[n.rule_id, 1, 1]], true, ni);
      renderProducer(root, r);
      return { html: root.innerHTML, map: plannerMap(n, r) };
    };
    const trip = (rule: string, extra: Partial<Notice>) => notice('a', rule, 'LOW', { entity_type: 'Trip', entity_id: 'T1', ...extra });

    let out = render(trip('STM_041', { details: { stop_id: 'S1', location_id: 'Z1', stop_sequence: '1', departure: '08:00:00' } }));
    expect(out.html).toContain('1. sıradaki satırda hem Hastane durağı hem Z1 talebe bağlı hizmet bölgesi seçilmiş');
    expect(out.html).toContain('<tr class="bad"><td>Hastane</td><td>Z1</td><td>1</td><td>08:00</td></tr>');
    expect(out.html).toContain('Hastane · Durak sırası 1');

    out = render(trip('STM_042', { observed_value: '*', details: { stop_id: 'S2', stop_sequence: '2', departure: '08:10:00', stop_headsign: 'Centre*' } }));
    expect(out.html).toContain('Okul durağında (2. sıra) gösterilen yön metni „Centre*” içinde * karakteri var');
    expect(out.html).toContain('<th>Yön metni</th>');

    out = render(trip('STM_043', { observed_value: '201', details: {
      first_stop_id: 'S1', first_stop_sequence: '1', first_departure: '06:00:00',
      last_stop_id: 'S3', last_stop_sequence: '201', last_arrival: '09:20:00', stop_count: '201', distinct_stops: '3',
    } }));
    expect(out.html).toContain('Sefer Hastane durağından 06:00 saatinde kalkıp Çarşı durağına 09:20 saatinde varıyor; 201 durak saati var.');
    expect(out.html).toContain('yalnız 3 farklı durak var');
    expect(out.html).toContain('Hastane 06:00 → Çarşı 09:20');
    expect(out.map.opts?.pins.filter(p => p.primary)).toHaveLength(2);
    // Every stop distinct: no merge sentence.
    out = render(trip('STM_043', { details: { first_stop_id: 'S1', last_stop_id: 'S3', stop_count: '201', distinct_stops: '201' } }));
    expect(out.html).not.toContain('farklı durak var');

    out = render(notice('a', 'STM_044', 'INFO', { entity_type: 'Feed', observed_value: '2100000' }));
    expect(out.html).toContain('Veri setinde 2.100.000 durak saati kaydı var');

    out = render(trip('STM_045', { observed_value: '28:30:00', details: { stop_id: 'S2' } }));
    expect(out.html).toContain('Sefer Okul durağından 28:30 saatinde kalkıyor');

    out = render(notice('a', 'STM_046', 'CRITICAL', { entity_type: 'Trip', details: { stop_id: 'S3', stop_sequence: '4', arrival: '08:20:00', departure: '08:21:00' } }));
    expect(out.html).toContain('Çarşı durağında 08:21 saatli (4. sıra) bir durak saati var');
    expect(out.html).toContain('<tr class="bad"><td>Çarşı</td><td>4</td><td>08:20</td><td>08:21</td></tr>');

    out = render(trip('STM_047', { details: {
      affected_rows: '2',
      example_0_stop_id: 'S2', example_0_stop_sequence: '2', example_0_missing: 'both',
      example_1_stop_id: 'S3', example_1_stop_sequence: '3', example_1_missing: 'arrival', example_1_departure: '08:12:00',
    } }));
    expect(out.html).toContain('Bu seferde 2 kesin zaman noktasında saat eksik; ilki Okul durağı (2. sıra).');
    expect(out.html).toContain('<tr class="bad"><td>Okul</td><td>2</td><td>eksik</td><td>eksik</td></tr>');
    expect(out.html).toContain('<tr class="bad"><td>Çarşı</td><td>3</td><td>eksik</td><td>08:12</td></tr>');
    expect(out.map.opts?.pins.filter(p => p.primary)).toHaveLength(2);
  });

  it('STM_045, STM_048 ve STM_049 önceki / hatalı / sonraki satırı durak sırasıyla gösterir', () => {
    const ni: NameIndex = {
      ...emptyNameIndex,
      stops: { S1: 'Hastane', S2: 'Okul', S3: 'Çarşı' },
      routes: { R1: '10' }, trips: { T1: 'Merkez' }, trip_routes: { T1: 'R1' }, trip_first_dep: { T1: '23:40' },
      trip_stops: { T1: ['S1', 'S2', 'S3'] }, stop_coords: { S1: [41, 29], S2: [41.01, 29.01], S3: [41.02, 29.02] },
    };
    const render = (n: Notice) => {
      const root = { innerHTML: '', querySelectorAll: () => [], querySelector: () => null } as unknown as HTMLElement;
      const r = result([n], [], [[n.rule_id, 1, 1]], true, ni);
      renderProducer(root, r);
      return { html: root.innerHTML, map: plannerMap(n, r) };
    };
    const rows = {
      prev_stop_id: 'S1', prev_stop_sequence: '1', prev_arrival: '23:40:00', prev_departure: '23:40:00',
      stop_id: 'S2', stop_sequence: '2',
      next_stop_id: 'S3', next_stop_sequence: '3', next_arrival: '00:20:00', next_departure: '00:20:00',
    };
    const trip = (rule: string, extra: Partial<Notice>) => notice('a', rule, 'HIGH', { entity_type: 'Trip', entity_id: 'T1', ...extra });

    let out = render(trip('STM_048', { field: 'arrival_time', observed_value: '00:10:00', details: {
      ...rows, arrival: '00:10:00', departure: '00:10:00', previous_time: '23:40:00', observed_time: '00:10:00', expected_time: '24:10:00',
    } }));
    expect(out.html).toContain('<th>Durak sırası</th><th>Durak</th><th>Varış</th><th>Kalkış</th>');
    expect(out.html).toContain('<tr><td>1</td><td>Hastane</td><td>23:40</td><td>23:40</td></tr>');
    expect(out.html).toContain('<tr class="bad"><td>2</td><td>Okul</td><td>00:10 → 24:10</td><td>00:10</td></tr>');
    expect(out.html).toContain('<tr><td>3</td><td>Çarşı</td><td>00:20</td><td>00:20</td></tr>');
    expect(out.map.opts?.pins.filter(p => p.primary)).toHaveLength(1);

    out = render(trip('STM_049', { field: 'departure_time', observed_value: '00:02:00', details: {
      ...rows, arrival: '23:58:00', departure: '00:02:00', previous_time: '23:58:00', observed_time: '00:02:00', expected_time: '24:02:00',
    } }));
    expect(out.html).toContain('<tr class="bad"><td>2</td><td>Okul</td><td>23:58</td><td>00:02 → 24:02</td></tr>');

    out = render(trip('STM_045', { field: 'departure_time', observed_value: '28:30:00', expected_value: '≤27:00:00', details: {
      ...rows, arrival: '28:20:00', departure: '28:30:00',
    } }));
    expect(out.html).toContain('Sefer Okul durağından 28:30 saatinde kalkıyor; servis gününün sonu 27:00, bu saat onu aşıyor.');
    // STM_045 has no expected time per row: the faulty row is shown as written.
    expect(out.html).toContain('<tr class="bad"><td>2</td><td>Okul</td><td>28:20</td><td>28:30</td></tr>');

    // STM_003/004: same table; the faulty cell is the raw value, "?" on a neighbour reads as invalid.
    out = render(trip('STM_003', { field: 'arrival_time', observed_value: '8:6O', details: {
      ...rows, arrival: '?', departure: '08:10:00', next_arrival: '?',
    } }));
    expect(out.html).toContain('<tr><td>1</td><td>Hastane</td><td>23:40:00</td><td>23:40:00</td></tr>');
    expect(out.html).toContain('<tr class="bad"><td>2</td><td>Okul</td><td>8:6O</td><td>08:10:00</td></tr>');
    expect(out.html).toContain('<tr><td>3</td><td>Çarşı</td><td>geçersiz</td><td>00:20:00</td></tr>');
    out = render(trip('STM_004', { field: 'departure_time', observed_value: '25:61:00', details: { ...rows, arrival: '08:10:00', departure: '?' } }));
    expect(out.html).toContain('<tr class="bad"><td>2</td><td>Okul</td><td>08:10:00</td><td>25:61:00</td></tr>');
  });

  it('STM_050 özeti boş satır sayısını, saatli/saatsiz dağılımı, ilk satırları ve örnek seferi gösterir', () => {
    const ni: NameIndex = {
      ...emptyNameIndex,
      stops: { S1: 'Hastane', S2: 'Okul', S3: 'Çarşı' },
      routes: { R1: '10' }, trips: { T1: 'Merkez' }, trip_routes: { T1: 'R1' }, trip_first_dep: { T1: '08:00' },
      trip_stops: { T1: ['S1', 'S2', 'S3'] }, trip_stop_sequences: { T1: [1, 2, 3] },
      stop_coords: { S1: [41, 29], S2: [41.01, 29.01], S3: [41.02, 29.02] },
    };
    const render = (n: Notice) => {
      const root = { innerHTML: '', querySelectorAll: () => [], querySelector: () => null } as unknown as HTMLElement;
      const r = result([n], [], [[n.rule_id, 1, 1]], true, ni);
      renderProducer(root, r);
      return { html: root.innerHTML, map: plannerMap(n, r) };
    };
    const examples = {
      example_0_trip_id: 'T1', example_0_stop_id: 'S2', example_0_stop_sequence: '2',
      example_1_trip_id: 'T1', example_1_stop_id: 'S3', example_1_stop_sequence: '3', example_1_arrival: '08:10:00', example_1_departure: '08:10:00',
      // Another trip's row: listed, but not red on T1's line.
      example_2_trip_id: 'T9', example_2_stop_id: 'S1', example_2_stop_sequence: '1',
    };
    let out = render(notice('a', 'STM_050', 'LOW', { entity_type: 'Feed', observed_value: '3', details: {
      empty_rows: '3', empty_rows_with_time: '1', total_rows: '1200', ...examples,
    } }));
    expect(out.html).toContain('3 durak saati satırında (toplam 1.200 satırdan) kesin/yaklaşık işareti boş');
    expect(out.html).toContain('Bunların 1 tanesinde saat yazılı, 2 tanesinde saat yok.');
    expect(out.html).toContain('<li>Saatsiz 2 satıra timepoint=0 yazın; bu durakların saati iki yandaki kesin saatlerden tahmin edilir.</li>');
    expect(out.html).toContain('<li>Saatli 1 satırdan tarifede gerçekten söz verilen saatlere timepoint=1, tahmini olanlara timepoint=0 yazın.</li>');
    // The trip as planners know it (route · direction · departure), not its trip_id.
    expect(out.html).toContain('<th>Sefer</th><th>Durak (sıra)</th>');
    expect(out.html).toContain('<tr><td>10 · Merkez yönü · 08:00</td><td>Okul (2)</td><td>—</td><td>—</td></tr>');
    expect(out.html).toContain('<tr><td>10 · Merkez yönü · 08:00</td><td>Çarşı (3)</td><td>08:10</td><td>08:10</td></tr>');
    const red = out.map.opts?.pins.filter(p => p.primary).map(p => p.label) ?? [];
    expect(red).toEqual([expect.stringContaining('durak sırası 2'), expect.stringContaining('durak sırası 3')]);
    expect(out.map.opts?.pins).toHaveLength(3);

    // Every empty row has a time: only the timed step, as a single paragraph.
    out = render(notice('a', 'STM_050', 'LOW', { entity_type: 'Feed', observed_value: '1', details: {
      empty_rows: '1', empty_rows_with_time: '1', total_rows: '10', example_0_trip_id: 'T1', example_0_stop_id: 'S3', example_0_stop_sequence: '3',
    } }));
    expect(out.html).not.toContain('Saatsiz');
    expect(out.html).toContain('Saatli 1 satırdan');
  });

  it('DQ_003–011 kartları takvimi, seferleri, durakları, hatları, işletmecileri ve haritayı gösterir', () => {
    const ni: NameIndex = {
      ...emptyNameIndex,
      stops: { S1: 'Hastane', S2: 'Okul', S3: 'Çarşı' },
      routes: { R1: '10', R2: '20' }, trips: { T1: 'Merkez', T2: 'Liman' }, trip_routes: { T1: 'R1', T2: 'R2' },
      trip_stops: { T1: ['S1', 'S2', 'S3'], T2: ['S3', 'S1'] }, trip_stop_sequences: { T1: [1, 2, 3], T2: [1, 2] },
      stop_coords: { S1: [41, 29], S2: [41.01, 29.01], S3: [41.02, 29.02] },
    };
    const render = (n: Notice) => {
      const root = { innerHTML: '', querySelectorAll: () => [], querySelector: () => null } as unknown as HTMLElement;
      const r = result([n], [], [[n.rule_id, 1, 1]], true, ni);
      renderProducer(root, r);
      return { html: root.innerHTML, map: plannerMap(n, r) };
    };

    let out = render(notice('a', 'DQ_005', 'HIGH', { entity_type: 'Feed', details: {
      service_count: '2', today: '20260515', last_active_date: '20250630', last_service_id: 'KIS',
    } }));
    expect(out.html).toContain('Veri setindeki 2 takvimin son çalışma günü 30.06.2025 (KIS takvimi); bugünden (15.05.2026) sonra çalışan takvim yok.');
    out = render(notice('a', 'DQ_005', 'HIGH', { entity_type: 'Feed', details: { service_count: '3' } }));
    expect(out.html).toContain('Veri setindeki 3 takvimin hiçbirinde çalışma günü yok');

    for (const rule of ['DQ_005b', 'DQ_009']) {
      out = render(notice('a', rule, 'HIGH', { entity_type: 'Feed', details: {
        trip_count: '1200', example_0_trip_id: 'T1', example_0_route_id: 'R1', example_0_service_id: 'HAFTAICI',
      } }));
      expect(out.html).toContain('1.200 sefer tanımlı');
      expect(out.html).toContain('<th>Sefer</th><th>Hat</th><th>Takvim kodu</th>');
      expect(out.html).toContain('<tr class="bad"><td>Merkez yönü</td><td>10</td><td>HAFTAICI</td></tr>');
    }

    out = render(notice('a', 'DQ_005c', 'HIGH', { entity_type: 'Feed', details: {
      missing_count: '2', stop_count: '3',
      example_0_stop_id: 'P1', example_0_stop_name: 'Peron 1', example_0_location_type: '0', example_0_parent_station: 'S1',
      example_1_stop_id: 'G1', example_1_location_type: '3',
    } }));
    expect(out.html).toContain('3 duraktan 2 tanesinin konumu girilmemiş');
    expect(out.html).toContain('<tr class="bad"><td>Peron 1 (P1)</td><td>Durak</td><td>Hastane</td></tr>');
    expect(out.html).toContain('<tr class="bad"><td>G1</td><td>Bağlantı noktası</td><td>—</td></tr>');

    // DQ_006: the route table and the example trip drawn straight between its stops.
    out = render(notice('a', 'DQ_006', 'HIGH', { entity_type: 'Feed', details: {
      shapeless_trips: '9', fixed_trips: '10', route_count: '1', example_trip_id: 'T1',
      route_0_route_id: 'R1', route_0_shapeless: '9', route_0_trips: '10', route_0_example_trip_id: 'T1',
    } }));
    expect(out.html).toContain('10 seferden 9 tanesi bir güzergâh çizimine bağlı değil (1 hat)');
    expect(out.html).toContain('<tr><td>10</td><td>9 / 10</td><td>Merkez yönü</td></tr>');
    expect(out.map.opts?.extraPolylines?.[0]?.coords).toEqual([[41, 29], [41.01, 29.01], [41.02, 29.02]]);
    expect(out.map.opts?.pins).toHaveLength(3);
    expect(out.html).toContain('Haritada: 10 · Merkez yönü (Hastane → Çarşı, 3 durak).');
    expect(out.html).toContain('<tr><td>1</td><td>Hastane (S1)</td></tr>');

    out = render(notice('a', 'DQ_010', 'INFO', { entity_type: 'Agency', entity_id: 'A2', details: {
      agency_name: 'Yedek', route_count: '5', used_0_agency_id: 'A1', used_0_routes: '5',
    } }));
    expect(out.html).toContain('Yedek işletmecisine hiçbir hat bağlı değil; veri setindeki 5 hattın tamamı başka işletmecilere bağlı.');
    expect(out.html).toContain('<tr><td>A1</td><td>5</td></tr>');

    // DQ_011: the one stop named and pinned red.
    out = render(notice('a', 'DQ_011', 'LOW', { entity_type: 'Feed', details: { stop_id: 'S2', stop_name: 'Okul' } }));
    expect(out.html).toContain('Veri setindeki tek durak Okul (S2)');
    expect(out.map.opts?.pins.filter(p => p.primary).map(p => p.label)).toEqual([expect.stringContaining('Okul')]);

    // DQ_003 on a route without a shape: its first trip's stops instead of the whole network.
    out = render(notice('a', 'DQ_003', 'INFO', { entity_type: 'Route', entity_id: 'R2', field: 'route_desc' }));
    expect(out.map.opts?.extraPolylines?.[0]?.coords).toEqual([[41.02, 29.02], [41, 29]]);
    expect(out.map.opts?.pins).toHaveLength(2);
    // The route named, the example trip on the map from its first to its last stop, listed in order.
    expect(out.html).toContain('20 hattında yolcuya ek bilgi veren açıklama boş. Haritada: 20 · Liman yönü (Çarşı → Hastane, 2 durak).');
    expect(out.html).toContain('<tr><td>1</td><td>Çarşı (S3)</td></tr><tr><td>2</td><td>Hastane (S1)</td></tr>');
    expect(out.html).not.toContain('<td>R2</td></tr>');
  });

  it('DQ_012–022 kartları işletmecileri, seferleri, boşlukları, kodları, adları ve haritayı gösterir', () => {
    const ni: NameIndex = {
      ...emptyNameIndex,
      stops: { S1: 'Hastane', S2: 'Okul', S3: 'Çarşı' },
      routes: { R1: '10' }, trips: { T1: 'Merkez' }, trip_routes: { T1: 'R1' },
      trip_stops: { T1: ['S1', 'S2', 'S3'] }, trip_stop_sequences: { T1: [1, 2, 3] },
      stop_coords: { S1: [41, 29], S2: [41.01, 29.01], S3: [41.02, 29.02] },
    };
    const render = (n: Notice) => {
      const root = { innerHTML: '', querySelectorAll: () => [], querySelector: () => null } as unknown as HTMLElement;
      const r = result([n], [], [[n.rule_id, 1, 1]], true, ni);
      renderProducer(root, r);
      return { html: root.innerHTML, map: plannerMap(n, r) };
    };
    const red = (out: ReturnType<typeof render>) => out.map.opts?.pins.filter(p => p.primary).map(p => p.label) ?? [];

    let out = render(notice('a', 'DQ_012', 'LOW', { entity_type: 'Feed', details: {
      agency_count: '6', route_count: '40', example_0_agency_id: 'A1', example_0_agency_name: 'Bir',
    } }));
    expect(out.html).toContain('Veri setinde 6 işletmeci var ama 40 hattın hiçbirinde');
    expect(out.html).toContain('<tr><td>Bir (A1)</td></tr>');

    out = render(notice('a', 'DQ_013', 'INFO', { entity_type: 'Feed', details: {
      trip_count: '1', example_0_trip_id: 'T1', example_0_route_id: 'R1', example_0_service_id: 'HER',
    } }));
    expect(out.html).toContain('Veri setinde yalnızca 1 sefer var.');
    expect(out.html).toContain('<tr><td>Merkez yönü</td><td>10</td><td>HER</td></tr>');
    expect(out.map.opts?.pins).toHaveLength(3);

    out = render(notice('a', 'DQ_016', 'INFO', { entity_type: 'File', entity_id: 'stops.txt', file: 'stops.txt', details: {
      affected_rows: '12', sample_0_field: 'stop_name', sample_0_value: ' Okul ', sample_0_line: '3', sample_0_record: 'S2',
      sample_1_field: 'wheelchair_note', sample_1_value: 'x ', sample_1_line: '4', sample_1_record: 'S3 ', record_field: 'stop_id',
      identity_whitespace_fields: 'stop_id, parent_station',
    } }));
    // The list and the columns in the planner's words; an unnamed column keeps its own name.
    expect(out.html).toContain('Durak listesi içinde 12 satırda bazı değerlerin başında ya da sonunda gereksiz boşluk var');
    expect(out.html).toContain('Boşluk kod alanlarında da var (Durak kodu, Bağlı istasyon)');
    expect(out.html).not.toContain('stops.txt dosyasında');
    // Each sample names its stop (a code with spaces is matched trimmed) and its row; the stops are pinned red.
    expect(out.html).toContain('<th>Alan</th><th>Değer</th><th>Durak kodu</th><th>Satır</th>');
    expect(out.html).toContain('<tr class="bad"><td>Durak adı</td><td>␠Okul␠</td><td>Okul (S2)</td><td>3</td></tr>');
    expect(out.html).toContain('<td>wheelchair_note</td><td>x␠</td><td>Çarşı (S3␠)</td><td>4</td>');
    expect(red(out)).toEqual([expect.stringContaining('Okul'), expect.stringContaining('Çarşı')]);
    expect(out.html).toContain('<td>Durak listesi</td>');
    out = render(notice('a', 'DQ_016', 'INFO', { entity_type: 'File', entity_id: 'x.txt', file: 'x.txt', details: { affected_rows: '2' } }));
    expect(out.html).toContain('Bazı ad, kod veya değerlerin başında ya da sonunda gereksiz boşluk var');

    out = render(notice('a', 'DQ_017', 'INFO', { entity_type: 'Feed', observed_value: '1', details: {
      example_0_stop_id: 'S9', example_0_stop_name: 'Yeni', example_0_stop_lat: '0.1', example_0_stop_lon: '0.2',
    } }));
    expect(out.html).toContain('1 durağın konumu 0,0 noktasının');
    expect(out.html).toContain('<tr class="bad"><td>Yeni (S9)</td><td>0.1</td><td>0.2</td></tr>');

    // DQ_020: the route table, the last stop named as a headsign and drawn red on the trip.
    out = render(notice('a', 'DQ_020', 'INFO', { entity_type: 'Feed', details: {
      missing_trips: '5', trip_count: '8', route_count: '1', example_trip_id: 'T1',
      route_0_route_id: 'R1', route_0_missing: '5', route_0_trips: '8', route_0_example_trip_id: 'T1',
    } }));
    expect(out.html).toContain('8 seferden 5 tanesinde yolcuya gösterilen yön adı boş (1 hat).');
    expect(out.html).toContain('Örneğin 10 hattının „Merkez“ yönündeki seferi Çarşı durağında bitiyor; yön adı olarak &quot;Çarşı&quot; yazılabilir.</li>');
    expect(out.html).toContain('<tr><td>10</td><td>5 / 8</td><td>Merkez yönü</td></tr>');
    expect(red(out)).toEqual([expect.stringContaining('Çarşı')]);
    expect(out.map.opts?.focus).toBeUndefined();
    expect(out.html).toContain('<tr class="bad"><td>3</td><td>Çarşı (S3)</td></tr>');

    // DQ_018 on a stop_headsign row: that row's trip, its stop in red.
    out = render(notice('a', 'DQ_018', 'INFO', { entity_type: 'Row', file: 'stop_times.txt', field: 'stop_headsign',
      observed_value: 'MERKEZ', details: { trip_id: 'T1', stop_id: 'S2', stop_sequence: '2' } }));
    expect(red(out)).toEqual([expect.stringContaining('Okul')]);
    expect(out.map.opts?.pins).toHaveLength(3);
    expect(out.html).toContain('10 hattının „Merkez“ yönündeki seferinin Okul (S2) durağındaki yön adı tamamen BÜYÜK HARFLE yazılmış: &quot;MERKEZ&quot;.');
    expect(out.html).toContain('<tr class="bad"><td>2</td><td>Okul (S2)</td></tr>');

    // DQ_018/019 on a route or stop name: which name, whose, and the route's example trip on the map.
    out = render(notice('a', 'DQ_018', 'INFO', { entity_type: 'Route', entity_id: 'R1', file: 'routes.txt', field: 'route_long_name', observed_value: 'MERKEZ - LIMAN' }));
    expect(out.html).toContain('10 hattının uzun adı tamamen BÜYÜK HARFLE yazılmış: &quot;MERKEZ - LIMAN&quot;. Haritada: 10 · Merkez yönü');
    expect(out.html).toContain('<td>MERKEZ - LIMAN</td>');
    expect(out.html).toContain('<th>Ad türü</th>');
    expect(out.html).toContain('<td>Hat uzun adı</td><td>MERKEZ - LIMAN</td>');
    expect(out.map.opts?.pins).toHaveLength(3);
    out = render(notice('a', 'DQ_019', 'INFO', { entity_type: 'Stop', entity_id: 'S2', file: 'stops.txt', field: 'stop_name', observed_value: 'okul' }));
    expect(out.html).toContain('S2 kodlu durağın adı tamamen küçük harfle yazılmış: &quot;okul&quot;.');

    out = render(notice('a', 'DQ_021', 'HIGH', { entity_type: 'File', entity_id: 'stops.txt', file: 'stops.txt', details: {
      affected_duplicates: '1', example_keys: 'S1', example_0_key: 'S1', example_0_lines: '2, 3',
    } }));
    // Said as stops, not as a file and column; the table names each code's stop and its row numbers.
    expect(out.html).toContain('Durak listesinde 1 durak kodu birden fazla durak kaydında kullanılmış.');
    expect(out.html).not.toContain('stops.txt dosyasında');
    expect(out.html).toContain('<th>Durak kodu</th><th>Dosyadaki satır numaraları</th>');
    expect(out.html).toContain('<tr class="bad"><td>S1</td><td>2, 3</td></tr>');
    expect(out.html).toContain('<td>Durak listesi</td>');
    expect(out.map.caption).toContain('Durak listesi');
    expect(out.html).toContain('<li>Her kayda ayrı bir kod verin.</li>');
    expect(red(out)).toEqual([expect.stringContaining('Hastane')]);
    out = render(notice('a', 'DQ_021', 'HIGH', { entity_type: 'File', entity_id: 'calendar_dates.txt', file: 'calendar_dates.txt', details: { affected_duplicates: '2' } }));
    expect(out.html).toContain('Takvim istisnalarında 2 takvim için aynı tarih birden fazla kez yazılmış');
    out = render(notice('a', 'DQ_021', 'HIGH', { entity_type: 'File', entity_id: 'fare_rules.txt', file: 'fare_rules.txt', details: { affected_duplicates: '3' } }));
    expect(out.html).toContain('Ücret kurallarında 3 kural aynı koşullarla birden fazla kez yazılmış');
    expect(out.html).toContain('<li>Tekrarlanan satırlardan yalnız birini bırakın.</li>');

    out = render(notice('a', 'DQ_022', 'MEDIUM', { entity_type: 'Feed', details: {
      common_name: 'Durak', common_count: '5', named_count: '6', example_0_stop_id: 'S1', example_1_stop_id: 'S3',
    } }));
    expect(out.html).toContain('Adı girilmiş 6 duraktan 5 tanesi &quot;Durak&quot; adını taşıyor');
    expect(red(out)).toHaveLength(2);
  });

  it('CAL_001–013 kartları takvimi, onu kullanan hatları ve seferleri, haritada güzergâhı gösterir', () => {
    const ni: NameIndex = {
      ...emptyNameIndex,
      stops: { S1: 'Hastane', S2: 'Okul', S3: 'Çarşı' },
      routes: { R1: '10', R2: '20' }, trips: { T1: 'Merkez', T2: 'Liman' }, trip_routes: { T1: 'R1', T2: 'R2' },
      trip_stops: { T1: ['S1', 'S2', 'S3'], T2: ['S3', 'S1'] }, trip_stop_sequences: { T1: [1, 2, 3], T2: [1, 2] },
      stop_coords: { S1: [41, 29], S2: [41.01, 29.01], S3: [41.02, 29.02] },
      service_days: { EXP: ['1111111', '20250101', '20250301', '0'], ZERO: ['0000000', '20250101', '20271231', '0'],
        DATES: ['0000000', '20250101', '20271231', '4'], SOON: ['1111111', '20250101', '20261120', '0'],
        UNUSED: ['1111100', '20250101', '20271231', '0'] },
    };
    const render = (n: Notice) => {
      const root = { innerHTML: '', querySelectorAll: () => [], querySelector: () => null } as unknown as HTMLElement;
      const r = result([n], [], [[n.rule_id, 1, 1]], true, ni);
      renderProducer(root, r);
      return { html: root.innerHTML, map: plannerMap(n, r) };
    };
    const usage = { trip_count: '3', route_count: '2', example_trip_id: 'T1',
      route_0_route_id: 'R1', route_0_trips: '2', route_0_example_trip_id: 'T1',
      route_1_route_id: 'R2', route_1_trips: '1', route_1_example_trip_id: 'T2' };

    // CAL_013 summary: the calendar, the routes on it, the example trip and every route on the map.
    let out = render(notice('a', 'CAL_013', 'INFO', { entity_type: 'Feed', entity_id: '20250301', observed_value: '20250301',
      details: { services: 'EXP', ...usage } }));
    expect(out.html).toContain('1 takvimin son çalışma günü 01.03.2025 geçti');
    expect(out.html).toContain('Bu takvimi 2 hattın 3 seferi kullanıyor. Haritada: 10 · Merkez yönü (Hastane → Çarşı, 3 durak).');
    expect(out.html).toContain('<td>EXP</td><td>Her gün</td><td>01.01.2025 – 01.03.2025</td>');
    expect(out.html).toContain('Bu takvimleri kullanan hatlar (2 hat, 3 sefer)');
    expect(out.html).toContain('<tr><td>1</td><td>Hastane (S1)</td></tr>');
    expect(out.map.opts?.extraPolylines).toHaveLength(2);
    // The affected record names the routes on its calendar, comma-separated.
    expect(out.html).toContain('<th>Hat kodu</th>');
    expect(out.html).toContain('<td>10, 20</td>');
    expect(out.map.opts?.extraPolylines?.[1]?.coords).toEqual([[41.02, 29.02], [41, 29]]);

    // CAL_002: the day and the value; CAL_005: both dates.
    out = render(notice('a', 'CAL_002', 'HIGH', { entity_type: 'Service', entity_id: 'EXP', field: 'monday', observed_value: '2', details: usage }));
    expect(out.html).toContain('EXP takviminin Pazartesi alanında &quot;2&quot; yazıyor');
    out = render(notice('a', 'CAL_005', 'HIGH', { entity_type: 'Service', entity_id: 'EXP', field: 'end_date', observed_value: '20260101', expected_value: '>= 20270101', details: usage }));
    expect(out.html).toContain('EXP takvimi 01.01.2027 tarihinde başlıyor ama 01.01.2026 tarihinde bitiyor');

    // CAL_006: no weekday — runs only on its exception dates, or never.
    out = render(notice('a', 'CAL_006', 'INFO', { entity_type: 'Service', entity_id: 'ZERO', details: usage }));
    expect(out.html).toContain('ZERO takviminde haftanın hiçbir günü işaretli değil ve istisna tarihi de yok; bu takvim hiç çalışmaz.');
    out = render(notice('a', 'CAL_006', 'INFO', { entity_type: 'Service', entity_id: 'DATES', details: usage }));
    expect(out.html).toContain('yalnız istisna listesindeki 4 tarihte çalışır');

    // CAL_008 opened into a calendar row (no observed value): the end date comes from the calendar.
    out = render(notice('a', 'CAL_008', 'MEDIUM', { entity_type: 'Service', entity_id: 'SOON', observed_value: null,
      details: { affected_services: '2', services: 'SOON,X', days_left: '11', ...usage } }));
    expect(out.html).toContain('2 takvim 20.11.2026 tarihinde bitiyor (11 gün kaldı)');
    expect(out.html).toContain('Bu takvimleri 2 hattın 3 seferi kullanıyor.');

    // CAL_011: no trip, so no route table and no example trip.
    out = render(notice('a', 'CAL_011', 'LOW', { entity_type: 'Service', entity_id: 'UNUSED', details: { trip_count: '0', route_count: '0' } }));
    expect(out.html).toContain('UNUSED takvimini hiçbir sefer kullanmıyor (Hafta içi, 01.01.2025 – 31.12.2027)');
    expect(out.html).not.toContain('Bu takvimleri kullanan hatlar');

    // CAL_014: the calendar's days against the dataset window and how far it reaches past it.
    out = render(notice('a', 'CAL_014', 'INFO', { entity_type: 'Service', entity_id: 'EXP', observed_value: '20250101..20250301',
      expected_value: '20250101..20250201', details: { days_before: '0', days_after: '28', ...usage } }));
    expect(out.html).toContain('EXP takvimi 01.01.2025 – 01.03.2025 arasında çalışıyor; veri seti bilgilerinde yazan geçerlilik dönemi ise 01.01.2025 – 01.02.2025. Takvim bu dönemin bitişinden 28 gün sonrasına uzanıyor');
    // CAL_015: the first service day; the JP variant says it is expected.
    out = render(notice('a', 'CAL_015', 'HIGH', { entity_type: 'Feed', observed_value: '20260601',
      details: { first_service_date: '20260601', days_until: '17', ...usage } }));
    expect(out.html).toContain('Veri setindeki ilk çalışma günü 01.06.2026 (17 gün sonra)');
    expect(out.html).toContain('Bu takvimleri 2 hattın 3 seferi kullanıyor.');
    out = render(notice('a', 'CAL_015', 'INFO', { entity_type: 'Feed', observed_value: '20260601',
      details: { first_service_date: '20260601', days_until: '17', message_variant: 'future_only_jp' } }));
    expect(out.html).toContain('Japonya (GTFS-JP) veri setindeki bütün takvimler 01.06.2026 tarihinde başlıyor');
    // CAL_020: the dataset window, no calendar trips; CAL_022: the row; CAL_025: the empty day.
    out = render(notice('a', 'CAL_020', 'LOW', { entity_type: 'Feed', file: 'feed_info.txt', observed_value: '9 years', details: { feed_start_date: '20200101', feed_end_date: '20290101', span_days: '3288',
      service_first: '20260101', service_last: '20261231', service_first_calendar: 'EXP', service_last_calendar: 'SOON' } }));
    expect(out.html).toContain('geçerlilik dönemi 01.01.2020 – 01.01.2029, yani yaklaşık 9 yıl; bu kadar uzun bir tarife dönemi gerçekçi değil. Takvimler ise yalnız 01.01.2026 – 31.12.2026 arasında çalışıyor.');
    expect(out.html).toContain('Geçerlilik dönemini takvimlerin çalıştığı döneme göre ayarlayın: başlangıç 01.01.2026, bitiş 31.12.2026.');
    expect(out.html).toContain('<tr class="bad"><td>Veri seti bilgilerinde yazan</td><td>01.01.2020</td><td>01.01.2029</td><td>3.289</td></tr>');
    expect(out.html).toContain('<tr><td>Takvimlerin çalıştığı</td><td>01.01.2026 (EXP takvimi)</td><td>31.12.2026 (SOON takvimi)</td><td>365</td></tr>');
    // Calendars as long as the window: the calendar setting the far end is fixed first.
    out = render(notice('a', 'CAL_020', 'LOW', { entity_type: 'Feed', file: 'feed_info.txt', observed_value: '9 years', details: {
      feed_start_date: '20200101', feed_end_date: '20290101', span_days: '3288',
      service_first: '20250101', service_last: '20990101', service_first_calendar: 'EXP', service_last_calendar: 'FAR' } }));
    expect(out.html).toContain('Takvimler de 01.01.2025 – 01.01.2099 arasında çalışıyor; bu da gerçekçi değil, en uzak tarihi FAR takvimi belirliyor.');
    expect(out.html).toContain('Önce FAR takviminin bitiş tarihini tarifenin gerçek sonuna çekin');
    expect(out.html).toContain('<td>Veri seti bilgileri</td>');
    out = render(notice('a', 'CAL_022', 'HIGH', { entity_type: 'Service', line: 4 }));
    expect(out.html).toContain('Takvim listesinin 4. satırında takvim kodu boş');
    out = render(notice('a', 'CAL_025', 'HIGH', { entity_type: 'Row', entity_id: 'EXP', field: 'sunday', details: usage }));
    expect(out.html).toContain('EXP takviminin Pazar alanı boş');
    expect(out.html).toContain('<td>10, 20</td>');
    // CAL_024: the next service day, or none.
    out = render(notice('a', 'CAL_024', 'LOW', { entity_type: 'Service', entity_id: 'EXP', observed_value: '3', details: { next_active: '20260701', ...usage } }));
    expect(out.html).toContain('bir sonraki çalışma günü 01.07.2026. Bu takvime bağlı 3 sefer');
    expect(out.map.opts?.extraPolylines).toHaveLength(2);
  });

  it('OPR kartları hattı, yönü, saatleri ve seferi planlamacı diliyle anlatır, haritada seferi çizer', () => {
    const ni: NameIndex = {
      ...emptyNameIndex,
      stops: { S1: 'Hastane', S2: 'Okul', S3: 'Çarşı' },
      routes: { R1: '10' }, trips: { T1: 'Çarşı', T2: 'Çarşı' }, trip_routes: { T1: 'R1', T2: 'R1' },
      trip_first_dep: { T1: '06:00', T2: '06:01' },
      trip_stops: { T1: ['S1', 'S2', 'S3', 'S2'], T2: ['S1', 'S2', 'S3'] }, trip_stop_sequences: { T1: [1, 2, 3, 4], T2: [1, 2, 3] },
      stop_coords: { S1: [41, 29], S2: [41.01, 29.01], S3: [41.02, 29.02] },
      service_days: { WK: ['1111100', '20250101', '20271231', '0'] },
    };
    const render = (n: Notice) => {
      const root = { innerHTML: '', querySelectorAll: () => [], querySelector: () => null } as unknown as HTMLElement;
      const r = result([n], [], [[n.rule_id, 1, 1]], true, ni);
      renderProducer(root, r);
      return { html: root.innerHTML, map: plannerMap(n, r) };
    };

    // OPR_001: the gap's two departures in the text, a table and the affected row; the trip before it on the map.
    let out = render(notice('a', 'OPR_001', 'MEDIUM', { entity_type: 'Route', entity_id: 'R1', service_id: 'WK', observed_value: '660', expected_value: '≤ 240',
      details: { gap_from: '12:30', gap_to: '23:30', trip_before: 'T1', trip_after: 'T2', departures: '6', direction_id: '0' } }));
    expect(out.html).toContain('10 hattının „Çarşı“ yönünde seferlerinde (Hafta içi) 12:30 kalkışından sonraki sefer ancak 23:30 kalkışlı; arada 11 sa boyunca sefer yok. Sınır 4 sa.');
    expect(out.html).toContain('<tr class="bad"><td>12:30</td><td>Çarşı</td></tr>');
    expect(out.html).toContain('<td>12:30 → 23:30</td>');
    // The badge is the route's number (route_short_name); no separate code column repeats it, and route_id R1 is not shown.
    expect(out.html).toContain('<th>Hat</th><th>Hat adı</th><th>Yön</th>');
    expect(out.html).toContain('<td><span class="planner-route-badge">10</span></td><td>—</td><td>Çarşı</td>');
    expect(out.html).not.toContain('<td>R1</td>');
    expect(out.map.opts?.pins).toHaveLength(3);

    // OPR_007: the repeated stop red in the trip's stop list.
    out = render(notice('a', 'OPR_007', 'LOW', { entity_type: 'Trip', entity_id: 'T1', observed_value: '2× repeat', details: { dup_stop: 'S2' } }));
    expect(out.html).toContain('10 hattının „Çarşı“ yönündeki 06:00 kalkışlı seferi Okul durağından 2 kez geçiyor');
    expect(out.html).toContain('<tr class="bad"><td>2</td><td>Okul (S2)</td></tr>');
    expect(out.html).toContain('<tr class="bad"><td>4</td><td>Okul (S2)</td></tr>');

    // OPR_010: how many trips say yes and no.
    out = render(notice('a', 'OPR_010', 'LOW', { entity_type: 'Route', entity_id: 'R1', field: 'wheelchair_accessible',
      details: { yes_trips: '3', yes_trip_id: 'T1', no_trips: '1', no_trip_id: 'T2' } }));
    expect(out.html).toContain('10 hattının 3 seferi tekerlekli sandalyeyle erişilebilir, 1 seferi erişilemez olarak işaretlenmiş');
    expect(out.html).toContain('<tr><td>Erişilemez (2)</td><td>1</td><td>Çarşı yönü · 06:01</td></tr>');

    // OPR_011: the calendar's trips "use" it — it never runs.
    out = render(notice('a', 'OPR_011', 'HIGH', { entity_type: 'Service', entity_id: 'WK', details: { trip_count: '2', route_count: '1', example_trip_id: 'T2' } }));
    expect(out.html).toContain('WK takviminin hiç çalışma günü yok; bu takvimi kullanan seferler hiç çalışmaz. Bu takvimi 1 hattın 2 seferi kullanıyor.');

    // OPR_014: the longest transfers in a table, their stops pinned, the record named as the list.
    out = render(notice('a', 'OPR_014', 'LOW', { entity_type: 'Feed', file: 'transfers.txt', observed_value: '1050s (2)',
      details: { transfer_count: '2', average_seconds: '1050', example_0_from_stop_id: 'S2', example_0_to_stop_id: 'S3', example_0_seconds: '1200' } }));
    expect(out.html).toContain('Aktarma kurallarındaki 2 zamanlı aktarmanın ortalama süresi 18 dk');
    expect(out.html).toContain('<tr><td>Okul (S2)</td><td>Çarşı (S3)</td><td>20 dk</td></tr>');
    expect(out.html).toContain('<td>Aktarma kuralları</td>');
    expect(out.map.opts?.pins.filter(p => p.primary)).toHaveLength(2);

    // OPR_021: the special day's calendars and the dates.
    out = render(notice('a', 'OPR_021', 'MEDIUM', { entity_type: 'Route', entity_id: 'R1',
      details: { conflict_day_count: '2', sample_dates: '20261229,20261230', base_services: 'WK', override_services: 'HOL' } }));
    expect(out.html).toContain('10 hattında 2 özel günde hem özel gün takvimi (HOL) hem normal takvim (WK) çalışıyor (ilk gün 29.12.2026)');
    expect(out.html).toContain('<tr class="bad"><td>29.12.2026</td><td>Salı</td></tr>');

    // OPR_025: the shortest trips.
    out = render(notice('a', 'OPR_025', 'MEDIUM', { entity_type: 'Feed', file: 'stop_times.txt', observed_value: '45s',
      details: { trip_count: '5', example_0_trip_id: 'T2', example_0_seconds: '20', example_trip_id: 'T2' } }));
    expect(out.html).toContain('Seferlerin ortalama süresi 45 saniye');
    expect(out.html).toContain('<tr class="bad"><td>10</td><td>Çarşı yönü · 06:01</td><td>20 sn</td></tr>');
  });

  it('TRP kartları seferi hat, yön ve kalkışla anlatır; trip_id yalnız hatalı değer olduğunda görünür', () => {
    const ni: NameIndex = {
      ...emptyNameIndex,
      stops: { S1: 'Hastane', S2: 'Okul', S3: 'Çarşı' },
      routes: { R1: '10' }, trips: { T1: 'Çarşı', T2: '' }, trip_routes: { T1: 'R1', T2: 'R1' },
      trip_first_dep: { T1: '06:00', T2: '07:00' },
      trip_stops: { T1: ['S1', 'S2', 'S3'], T2: ['S1', 'S2'] }, trip_stop_sequences: { T1: [1, 2, 3], T2: [1, 2] },
      stop_coords: { S1: [41, 29], S2: [41.01, 29.01], S3: [41.02, 29.02] },
    };
    const render = (n: Notice) => {
      const root = { innerHTML: '', querySelectorAll: () => [], querySelector: () => null } as unknown as HTMLElement;
      const r = result([n], [], [[n.rule_id, 1, 1]], true, ni);
      renderProducer(root, r);
      return { html: root.innerHTML, map: plannerMap(n, r) };
    };

    // TRP_001: a repeated code is itself the finding, so it is shown; its trip is drawn.
    let out = render(notice('a', 'TRP_001', 'HIGH', { entity_type: 'Trip', entity_id: 'T1', line: 5, details: { first_line: '2' } }));
    expect(out.html).toContain('Sefer listesinde &quot;T1&quot; sefer kodu iki kez yazılmış (ilk kez 2. satırda, yeniden 5. satırda)');
    expect(out.map.opts?.pins).toHaveLength(3);

    // TRP_002: the missing route code is the finding; the trip is named by direction and time.
    out = render(notice('a', 'TRP_002', 'HIGH', { entity_type: 'Trip', entity_id: 'T1', observed_value: 'R9' }));
    expect(out.html).toContain('„Çarşı“ yönündeki 06:00 kalkışlı sefer &quot;R9&quot; kodlu bir hatta bağlanmış');
    expect(out.html).not.toContain('10 hattının „Çarşı“ yönündeki 06:00 kalkışlı seferi &quot;R9&quot;');

    // TRP_006: the trip by route, direction and time — never "T1".
    out = render(notice('a', 'TRP_006', 'MEDIUM', { entity_type: 'Trip', entity_id: 'T1', observed_value: '5' }));
    expect(out.html).toContain('10 hattının „Çarşı“ yönündeki 06:00 kalkışlı seferinin tekerlekli sandalye bilgisi &quot;5&quot;');
    expect(out.html).not.toContain('T1 seferi');
    // The map caption names the trip the same way (route · direction · departure).
    expect(out.map.caption).toContain('Haritada: 10 · Çarşı yönü · 06:00');

    // TRP_011: no headsign; the last stop offered as one and red in the stop list.
    out = render(notice('a', 'TRP_011', 'LOW', { entity_type: 'Trip', entity_id: 'T2' }));
    expect(out.html).toContain('10 hattının 07:00 kalkışlı seferinin yön adı da sefer numarası da yok');
    expect(out.html).toContain('Örneğin bu sefer Okul durağında bitiyor');
    expect(out.html).toContain('<tr class="bad"><td>2</td><td>Okul (S2)</td></tr>');

    // TRP_012: the trips without a direction with their path.
    out = render(notice('a', 'TRP_012', 'MEDIUM', { entity_type: 'Route', entity_id: 'R1', observed_value: '1', details: { example_trips: 'T1' } }));
    expect(out.html).toContain('10 hattı iki yönlü çalışıyor ama 1 seferinde yön bilgisi boş');
    expect(out.html).toContain('<tr class="bad"><td>Çarşı yönü · 06:00</td><td>Hastane → Çarşı</td></tr>');

    // TRP_015: block examples with their trip, the record named as the list.
    out = render(notice('a', 'TRP_015', 'LOW', { entity_type: 'Feed', file: 'trips.txt', observed_value: '2',
      details: { trips_with_block: '2', example_0_block: 'B1', example_0_trip_id: 'T1' } }));
    expect(out.html).toContain('Araç bloğu kodu taşıyan 2 seferin her biri ayrı bir blokta');
    expect(out.html).toContain('<tr class="bad"><td>B1</td><td>10 · Çarşı yönü · 06:00</td></tr>');
    expect(out.html).toContain('<td>Sefer listesi</td>');
    // The map caption names the trip it draws, not the list.
    expect(out.map.caption).toContain('Haritada: 10 · Çarşı yönü · 06:00');
  });

  it('kalan TRP kartları seferleri, blokları, takvimi ve satırı planlamacı diliyle anlatır', () => {
    const ni: NameIndex = {
      ...emptyNameIndex,
      stops: { S1: 'Hastane', S2: 'Okul', S3: 'Çarşı' },
      routes: { R1: '10' }, trips: { T1: 'Okul', T2: 'Çarşı' }, trip_routes: { T1: 'R1', T2: 'R1' },
      trip_first_dep: { T1: '08:00', T2: '08:15' },
      trip_stops: { T1: ['S1', 'S2', 'S3'], T2: ['S1', 'S3'] }, trip_stop_sequences: { T1: [1, 2, 3], T2: [1, 2] },
      stop_coords: { S1: [41, 29], S2: [41.01, 29.01], S3: [41.02, 29.02] },
    };
    const render = (n: Notice) => {
      const root = { innerHTML: '', querySelectorAll: () => [], querySelector: () => null } as unknown as HTMLElement;
      const r = result([n], [], [[n.rule_id, 1, 1]], true, ni);
      renderProducer(root, r);
      return { html: root.innerHTML, map: plannerMap(n, r) };
    };

    // TRP_020: the headsign is an intermediate stop, red in the stop list; the real last stop named.
    let out = render(notice('a', 'TRP_020', 'LOW', { entity_type: 'Trip', entity_id: 'T1', observed_value: 'Okul',
      details: { stop_id: 'S2', stop_sequence: '2', last_stop_id: 'S3' } }));
    expect(out.html).toContain('10 hattının „Okul“ yönündeki 08:00 kalkışlı seferinin yön adı &quot;Okul&quot; seferin son durağı değil, yol üzerindeki bir ara durak (Okul, 2. sıra); sefer Çarşı durağında bitiyor.');
    expect(out.html).toContain('<tr class="bad"><td>2</td><td>Okul (S2)</td></tr>');

    // TRP_022: both trips with their times, the other trip also on the map.
    out = render(notice('a', 'TRP_022', 'MEDIUM', { entity_type: 'Trip', entity_id: 'T1',
      details: { block_id: 'B1', other_trip_id: 'T2', start: '08:00', end: '08:20', other_start: '08:15', other_end: '08:30' } }));
    expect(out.html).toContain('aynı araç bloğunda (B1) ama saatleri çakışıyor: biri 08:00–08:20, diğeri 08:15–08:30');
    expect(out.html).toContain('<tr class="bad"><td>10 · Çarşı yönü · 08:15</td><td>08:15</td><td>08:30</td></tr>');
    expect(out.map.opts?.extraPolylines?.some(l => l.color === '#dc2626' || l.coords.length === 2)).toBe(true);

    // TRP_028: some trips without wheelchair information.
    out = render(notice('a', 'TRP_028', 'LOW', { entity_type: 'Feed', file: 'trips.txt', observed_value: '1/2',
      details: { missing_trips: '1', trip_count: '2', example_trips: 'T2' } }));
    expect(out.html).toContain('2 seferin 1 tanesinde (%50) tekerlekli sandalye bilgisi yok');

    // TRP_023: the next service day; TRP_024: route types by name.
    out = render(notice('a', 'TRP_023', 'HIGH', { entity_type: 'Feed', file: 'calendar.txt', details: { next_active: '20260601' } }));
    expect(out.html).toContain('Önümüzdeki 7 günde hiçbir sefer çalışmıyor; ilk çalışma günü 01.06.2026.');
    out = render(notice('a', 'TRP_024', 'MEDIUM', { entity_type: 'Trip', entity_id: 'B1', observed_value: 'route_type=0', expected_value: 'route_type=3',
      details: { block_id: 'B1', trip_count: '1', first_trip: 'T1', trips: 'T2' } }));
    expect(out.html).toContain('B1 araç bloğundaki seferler farklı türde hatlara ait (Otobüs (3), Tramvay (0))');
    expect(out.html).toContain('<td>B1</td><td>10 · Okul yönü · 08:00</td>');
    // The record's code is the block's: the map draws the block's first trip and names it.
    expect(out.map.caption).toContain('Haritada: 10 · Okul yönü · 08:00');
    expect(out.map.opts?.pins.length).toBeGreaterThan(1);

    // TRP_031: the code-less route row named by its trip.
    out = render(notice('a', 'TRP_031', 'HIGH', { entity_type: 'Trip', file: 'trips.txt', line: 4, details: { trip_id: 'T2' } }));
    expect(out.html).toContain('Sefer listesinin 4. satırındaki seferin hattı boş (Çarşı yönü · 08:15)');
  });

  it('VAT kartları hatları, durakları ve seferleri planlamacı diliyle anlatır ve haritada çizer', () => {
    const ni: NameIndex = {
      ...emptyNameIndex,
      stops: { S1: 'Hastane', S2: 'Okul', S3: 'Çarşı', S4: 'Liman', N1: 'Peron 2', Z1: 'Ada', Z2: 'Köy' },
      routes: { R1: '10', R2: '20', R5: '50', R6: '60' }, trips: { T1: 'Çarşı', T2: 'Liman', T5: 'Liman', T6: 'Köy' },
      trip_routes: { T1: 'R1', T2: 'R2', T5: 'R5', T6: 'R6' },
      trip_first_dep: { T1: '06:00', T2: '07:00', T5: '08:00', T6: '09:00' },
      trip_stops: { T1: ['S1', 'S2', 'S3'], T2: ['S1', 'S2', 'S4'], T5: ['N1', 'S4'], T6: ['Z1', 'Z2'] },
      trip_stop_sequences: { T1: [1, 2, 3], T2: [1, 2, 3], T5: [1, 2], T6: [1, 2] },
      stop_coords: { S1: [41, 29], S2: [41.01, 29.01], S3: [41.02, 29.02], S4: [41.03, 29.0], N1: [41.0004, 29], Z1: [42, 30], Z2: [42.01, 30] },
    };
    const render = (n: Notice) => {
      const root = { innerHTML: '', querySelectorAll: () => [], querySelector: () => null } as unknown as HTMLElement;
      const r = result([n], [], [[n.rule_id, 1, 1]], true, ni);
      renderProducer(root, r);
      return { html: root.innerHTML, map: plannerMap(n, r) };
    };

    // VAT_001: both routes by number, the shared and the differing stops; both routes drawn.
    let out = render(notice('a', 'VAT_001', 'MEDIUM', { entity_type: 'Route', entity_id: 'R1', observed_value: 'Jaccard=50%',
      details: { routes: 'R1,R2', route_b: 'R2', shared_stops: '2', stops_a: '3', stops_b: '3', only_a: 'S3', only_b: 'S4' } }));
    expect(out.html).toContain('10 ve 20 hatları aynı türde ve durakları büyük ölçüde ortak: 2 durak iki hatta da var (10: 3 durak, 20: 3 durak; benzerlik %50).');
    expect(out.html).toContain('İki hat yalnız 2 durakta ayrılıyor');
    expect(out.html).toContain('<tr class="bad"><td>Çarşı (S3)</td><td>✓</td><td>—</td></tr>');
    expect(out.html).toContain('<tr class="bad"><td>Liman (S4)</td><td>—</td><td>✓</td></tr>');
    expect(out.map.opts?.extraPolylines).toHaveLength(2);
    expect(out.map.opts?.pins.filter(p => p.primary)).toHaveLength(2);

    // VAT_002: the nearby stop a transfer could link, its distance and routes; both stops red, linked.
    out = render(notice('a', 'VAT_002', 'INFO', { entity_type: 'Stop', entity_id: 'S1', observed_value: '2 routes',
      details: { routes: 'R1,R2', nearby_stop_id: 'N1', nearby_distance_m: '45', nearby_routes: 'R5' } }));
    expect(out.html).toContain('Hastane durağından 2 hat geçiyor (10, 20). 45 m ötedeki Peron 2 durağından ise bu durağa uğramayan hatlar geçiyor (50)');
    expect(out.html).toContain('Yolcular Hastane ile Peron 2 arasında yürüyerek aktarma yapıyorsa');
    expect(out.html).toContain('<tr class="bad"><td>Peron 2 (N1)</td><td>45 m</td><td>50</td></tr>');
    expect(out.map.opts?.pins.filter(p => p.primary)).toHaveLength(2);
    expect(out.map.opts?.extraPolylines?.length).toBeGreaterThanOrEqual(4);

    // VAT_003: the trip, its duration against the same path's trips, a table of them; the trip drawn.
    out = render(notice('a', 'VAT_003', 'LOW', { entity_type: 'Trip', entity_id: 'T1',
      details: { duration_min: '40', expected_min: '10', departure: '06:00:00', pattern_trips: '6', example_0_trip_id: 'X1', example_0_departure: '05:54:00', example_0_minutes: '10' } }));
    expect(out.html).toContain('10 hattının „Çarşı“ yönündeki 06:00 kalkışlı seferi 40 dk sürüyor; aynı güzergâhtaki seferler bu saatlerde yaklaşık 10 dk sürüyor, bu sefer 30 dk daha uzun.');
    expect(out.html).toContain('<tr><td>05:54</td><td>10 dk</td></tr><tr class="bad"><td>06:00</td><td>40 dk</td></tr>');
    expect(out.html).toContain('Haritadaki sefer: 10 · Çarşı yönü · 06:00 — durak sırası');

    // VAT_005: the detached clusters, their route and the nearest main-network stop; linked on the map.
    out = render(notice('a', 'VAT_005', 'MEDIUM', { entity_type: 'Feed', observed_value: '2 / 1',
      details: { isolated_stops: 'Z1,Z2', cluster_count: '1', cluster_0_stops: '2', cluster_0_stop_id: 'Z1', cluster_0_trip_id: 'T6',
        cluster_0_routes: 'R6', cluster_0_nearest_stop_id: 'S3', cluster_0_nearest_m: '140000' } }));
    expect(out.html).toContain('1 durak grubu (2 durak) ağın geri kalanına hiçbir seferle bağlanmıyor');
    expect(out.html).toContain('Örneğin Ada durağının grubunda çalışan hat(lar): 60; ana ağa en yakın durak Çarşı (140 km).');
    expect(out.html).toContain('<tr class="bad"><td>Ada (Z1)</td><td>2</td><td>60</td><td>Çarşı (S3) · 140 km</td></tr>');
    expect(out.map.opts?.pins.filter(p => p.primary).length).toBeGreaterThanOrEqual(2);

    // VAT_006: the route's share and the next busiest routes.
    out = render(notice('a', 'VAT_006', 'INFO', { entity_type: 'Route', entity_id: 'R1', observed_value: '50/55 (%91)',
      details: { trips: '50', total_trips: '55', route_count: '3', other_0_route_id: 'R2', other_0_trips: '3' } }));
    expect(out.html).toContain('10 hattı veri setindeki 55 seferin 50 tanesini (%91) yapıyor; diğer 2 hat kalan 5 seferi paylaşıyor.');
    expect(out.html).toContain('<tr><td>20</td><td>3</td><td>%5</td></tr>');

    // VAT_008: the routes on the shape, each route's own stops drawn.
    out = render(notice('a', 'VAT_008', 'LOW', { entity_type: 'Shape', entity_id: 'SH1', observed_value: '67% (2 routes)',
      details: { routes: 'R1,R2', route_count: '2', total_routes: '3' } }));
    expect(out.html).toContain('&quot;SH1&quot; güzergâh çizimi 2 hatta kullanılıyor (10, 20; 3 hattın 2 tanesi, %67)');
    expect(out.html).toContain('<tr class="bad"><td>20</td><td>—</td><td>Liman yönü · 07:00</td></tr>');
    expect(out.map.opts?.extraPolylines).toHaveLength(2);
  });

  it('STP_001–010 kartları durağı adıyla, seferdeki yeriyle anlatır; konumu olmayan durağın aralığını haritada gösterir', () => {
    const ni: NameIndex = {
      ...emptyNameIndex,
      stops: { S1: 'Hastane', S2: 'Okul', S3: 'Çarşı', X6: 'Pazar', ST: 'Gar', P1: 'Peron' },
      routes: { R1: '10' }, trips: { T1: 'Çarşı' }, trip_routes: { T1: 'R1' }, trip_first_dep: { T1: '06:00' },
      trip_stops: { T1: ['S1', 'X6', 'S2', 'S3'] }, trip_stop_sequences: { T1: [1, 2, 3, 4] },
      stop_coords: { S1: [41, 29], S2: [41.02, 29.02], S3: [41.03, 29.03], ST: [41.021, 29.02], P1: [41.0201, 29.02] },
    };
    const render = (n: Notice) => {
      const root = { innerHTML: '', querySelectorAll: () => [], querySelector: () => null } as unknown as HTMLElement;
      const r = result([n], [], [[n.rule_id, 1, 1]], true, ni);
      renderProducer(root, r);
      return { html: root.innerHTML, map: plannerMap(n, r) };
    };
    const ctx = { trip_count: '12', route_count: '1', routes: 'R1', example_trip_id: 'T1', prev_stop_id: 'S1', next_stop_id: 'S2', stop_sequence: '2' };

    // STP_006: no latitude — named, placed on its trip between two stops, the stretch red and circled.
    let out = render(notice('a', 'STP_006', 'CRITICAL', { entity_type: 'Stop', entity_id: 'X6', field: 'stop_lat', details: ctx }));
    expect(out.html).toContain('„Pazar“ durağının enlemi boş; durak haritada gösterilemiyor. Bu durağa 12 sefer uğruyor (10). Örneğin 10 hattının „Çarşı“ yönündeki 06:00 kalkışlı seferi Hastane ile Okul arasında bu durağa uğruyor. Durak haritada bu iki durak arasında olmalı');
    expect(out.html).toContain('<tr class="bad"><td>2</td><td>Pazar (X6)</td></tr>');
    expect(out.map.opts?.extraPolylines?.some(l => l.zoomTo)).toBe(true);
    expect(out.map.opts?.highlights).toHaveLength(1);
    // A neighbour without a position: no stretch can be drawn, so the card does not claim one.
    out = render(notice('a', 'STP_007', 'CRITICAL', { entity_type: 'Stop', entity_id: 'X6', field: 'stop_lon', details: { ...ctx, next_stop_id: 'X9' } }));
    expect(out.html).toContain('„Pazar“ durağının boylamı boş');
    expect(out.html).not.toContain('Durak haritada bu iki durak arasında olmalı');

    // STP_001: both rows with names and positions, both pinned.
    out = render(notice('a', 'STP_001', 'CRITICAL', { entity_type: 'Stop', entity_id: 'S1', line: 9,
      details: { first_line: '2', first_name: 'Hastane', first_lat: '41', first_lon: '29', name: 'Hastane B', lat: '41.01', lon: '29', ...ctx } }));
    expect(out.html).toContain('„S1“ durak kodu durak listesinde iki kez yazılmış: 2. satırda „Hastane“, 9. satırda „Hastane B“ (iki konum arası 1');
    expect(out.html).toContain('<tr class="bad"><td>9</td><td>Hastane B</td><td>41.01, 29</td></tr>');
    expect(out.map.opts?.pins.filter(p => p.primary).length).toBeGreaterThanOrEqual(2);

    // STP_002: the code-less row by its name and position.
    out = render(notice('a', 'STP_002', 'CRITICAL', { entity_type: 'Stop', line: 5, details: { stop_name: 'Yeni', stop_code: '77', stop_lat: '41.04', stop_lon: '29.04' } }));
    expect(out.html).toContain('Durak listesinin 5. satırındaki durağın kodu boş („Yeni“, durak numarası 77); seferler bu durağa bağlanamaz.');
    expect(out.map.opts?.pins.some(p => p.lat === 41.04 && p.primary)).toBe(true);

    // STP_004: the summary opens into its stops; each row reads its own stop's trip context.
    out = render(notice('a', 'STP_004', 'HIGH', { entity_type: 'Feed', observed_value: '1',
      details: { affected_stops: '1', example_stop_ids: 'X6', example_0_stop_id: 'X6', example_0_trip_count: '12', example_0_routes: 'R1',
        example_0_example_trip_id: 'T1', example_0_prev_stop_id: 'S1', example_0_next_stop_id: 'S2', example_0_stop_sequence: '2' } }));
    expect(out.html).toContain('1 durağın enlemi sayı olarak okunamıyor; bu duraklar haritada gösterilemiyor. Örneğin „Pazar“ durağı.');
    expect(out.html).toContain('<tr class="bad"><td>Pazar</td><td>10</td><td>Hastane → Okul</td></tr>');

    // STP_008: the value and, as trips call at it, the likely type.
    out = render(notice('a', 'STP_008', 'MEDIUM', { entity_type: 'Stop', entity_id: 'S2', observed_value: '9', details: ctx }));
    expect(out.html).toContain('„Okul“ kaydının türü „9“');
    expect(out.html).toContain('Seferler bu kayda uğradığına göre türü büyük olasılıkla durak/peron.');

    // STP_009: the missing station and the nearest one, linked on the map.
    out = render(notice('a', 'STP_009', 'CRITICAL', { entity_type: 'Stop', entity_id: 'S2', observed_value: 'PX', details: { nearest_station_id: 'ST', nearest_station_m: '111', ...ctx } }));
    expect(out.html).toContain('„Okul“ durağı „PX“ kodlu istasyona bağlı gösterilmiş ama durak listesinde böyle bir kayıt yok. 111 m ötedeki „Gar“ istasyonu kastedilmiş olabilir.');
    expect(out.map.opts?.pins.filter(p => p.primary)).toHaveLength(2);

    // STP_010: the parent and its type by name.
    out = render(notice('a', 'STP_010', 'HIGH', { entity_type: 'Stop', entity_id: 'S2', observed_value: '0', details: { parent_station: 'P1', ...ctx } }));
    expect(out.html).toContain('„Okul“ durağı „Peron“ kaydına bağlı gösterilmiş ama o kayıt bir istasyon değil, durak/peron.');
  });

  it('STP_011–020 kartları durağı, öbür durağı ve önerilen kaydı adıyla anlatır ve haritada bağlar', () => {
    const ni: NameIndex = {
      ...emptyNameIndex,
      stops: { S1: 'Hastane', S2: 'Okul', S3: 'Çarşı', ST: 'Gar', P1: 'Gar Peron 1', E1: 'Gar Giriş', U1: 'Eski Durak' },
      routes: { R1: '10', R2: '20' }, trips: { T1: 'Çarşı', T2: 'Okul' }, trip_routes: { T1: 'R1', T2: 'R2' }, trip_first_dep: { T1: '06:00', T2: '07:00' },
      trip_stops: { T1: ['S1', 'S2', 'S3'], T2: ['ST', 'S2'] }, trip_stop_sequences: { T1: [1, 2, 3], T2: [1, 2] },
      stop_coords: { S1: [41, 29], S2: [41.01, 29.01], S3: [41.02, 29.02], ST: [41.03, 29.03], P1: [41.0301, 29.03], E1: [41.0305, 29.031], U1: [41.0102, 29.01] },
    };
    const render = (n: Notice) => {
      const root = { innerHTML: '', querySelectorAll: () => [], querySelector: () => null } as unknown as HTMLElement;
      const r = result([n], [], [[n.rule_id, 1, 1]], true, ni);
      renderProducer(root, r);
      return { html: root.innerHTML, map: plannerMap(n, r) };
    };
    const ctx = { trip_count: '12', route_count: '1', routes: 'R1', example_trip_id: 'T1', prev_stop_id: 'S1', next_stop_id: 'S3', stop_sequence: '2' };

    // STP_011: the type by name and the nearest station, linked.
    let out = render(notice('a', 'STP_011', 'HIGH', { entity_type: 'Stop', entity_id: 'E1', details: { location_type: '2', nearest_station_id: 'ST', nearest_station_m: '60' } }));
    expect(out.html).toContain('„Gar Giriş“ bir giriş/çıkış ama hiçbir istasyona bağlı değil');
    expect(out.html).toContain('60 m ötedeki „Gar“ istasyonu kastedilmiş olabilir.');
    expect(out.map.opts?.pins.filter(p => p.primary)).toHaveLength(2);

    // STP_012: the station used by trips and its platforms.
    out = render(notice('a', 'STP_012', 'HIGH', { entity_type: 'Stop', entity_id: 'ST', observed_value: '1',
      details: { platforms: 'P1', platform_count: '1', trip_count: '3', routes: 'R2', example_trip_id: 'T2', next_stop_id: 'S2', stop_sequence: '1' } }));
    expect(out.html).toContain('„Gar“ bir istasyon kaydı ama seferler doğrudan ona uğruyor');
    expect(out.html).toContain('İstasyonun içindeki peronlar: „Gar Peron 1“.');
    expect(out.html).toContain('<tr><td>Gar Peron 1</td><td>P1</td></tr>');

    // STP_014: the feed's timezone offered.
    out = render(notice('a', 'STP_014', 'MEDIUM', { entity_type: 'Stop', entity_id: 'S2', observed_value: 'Europe/Istambul', details: { agency_timezone: 'Europe/Istanbul', ...ctx } }));
    expect(out.html).toContain('„Okul“ durağının saat dilimi „Europe/Istambul“ tanınmıyor');
    expect(out.html).toContain('Veri setinin saat dilimi „Europe/Istanbul“');

    // STP_015: the defined levels by name.
    out = render(notice('a', 'STP_015', 'MEDIUM', { entity_type: 'Stop', entity_id: 'S2', observed_value: 'L9', details: { known_levels: 'L0=Zemin|L1', ...ctx } }));
    expect(out.html).toContain('Tanımlı katlar: Zemin (L0), L1.');
    expect(out.html).toContain('<tr><td>L0</td><td>Zemin</td></tr>');

    // STP_017: both stops, their distance and their trips; both red and linked.
    out = render(notice('a', 'STP_017', 'LOW', { entity_type: 'Stop', entity_id: 'S2', details: { other_stop_id: 'U1', distance_m: '3.2', ...ctx,
      other_trip_count: '0' } }));
    expect(out.html).toContain('„Okul“ ile „Eski Durak“ arası yalnız 3 m.');
    expect(out.html).toContain('„Okul“ durağına 12 sefer (10), „Eski Durak“ durağına 0 sefer (—) uğruyor.');
    expect(out.map.opts?.pins.filter(p => p.primary).length).toBeGreaterThanOrEqual(2);

    // STP_020: the used stop nearby and its trips; its trip drawn, both linked.
    out = render(notice('a', 'STP_020', 'LOW', { entity_type: 'Stop', entity_id: 'U1', details: { nearest_used_stop_id: 'S2', nearest_used_m: '22',
      nearest_trip_count: '12', nearest_routes: 'R1', nearest_example_trip_id: 'T1', nearest_prev_stop_id: 'S1', nearest_next_stop_id: 'S3', nearest_stop_sequence: '2' } }));
    expect(out.html).toContain('„Eski Durak“ durağına hiçbir sefer uğramıyor. 22 m ötedeki „Okul“ durağına ise 12 sefer uğruyor (10)');
    expect(out.html).toContain('<tr class="bad"><td>2</td><td>Okul (S2)</td></tr>');
    expect(out.map.opts?.extraPolylines?.length).toBeGreaterThanOrEqual(2);
  });

  it('STP_021–032 kartları istasyon bağlarını ve değerleri planlamacı diliyle anlatır, önerilen istasyonu haritada bağlar', () => {
    const ni: NameIndex = {
      ...emptyNameIndex,
      stops: { S1: 'Hastane', S2: 'Okul', S3: 'Çarşı', ST: 'Gar', ST2: 'Yeni Gar', B1: 'Bekleme', N1: 'Peron' },
      routes: { R1: '10' }, trips: { T1: 'Çarşı' }, trip_routes: { T1: 'R1' }, trip_first_dep: { T1: '06:00' },
      trip_stops: { T1: ['S1', 'S2', 'S3'] }, trip_stop_sequences: { T1: [1, 2, 3] },
      stop_coords: { S1: [41, 29], S2: [41.01, 29.01], S3: [41.02, 29.02], ST: [41.03, 29.03], ST2: [41.011, 29.01], B1: [41.0301, 29.03], N1: [41.0302, 29.0302] },
    };
    const render = (n: Notice) => {
      const root = { innerHTML: '', querySelectorAll: () => [], querySelector: () => null } as unknown as HTMLElement;
      const r = result([n], [], [[n.rule_id, 1, 1]], true, ni);
      renderProducer(root, r);
      return { html: root.innerHTML, map: plannerMap(n, r) };
    };
    const ctx = { trip_count: '12', route_count: '1', routes: 'R1', example_trip_id: 'T1', prev_stop_id: 'S1', next_stop_id: 'S3', stop_sequence: '2' };

    // STP_021: the parent and its type, linked.
    let out = render(notice('a', 'STP_021', 'HIGH', { entity_type: 'Stop', entity_id: 'B1', details: { parent_station: 'ST', parent_location_type: '1' } }));
    expect(out.html).toContain('„Bekleme“ biniş alanı „Gar“ kaydına bağlı ama o kayıt bir peron değil, istasyon');
    expect(out.map.opts?.pins.filter(p => p.primary)).toHaveLength(2);

    // STP_024: the access value in plain words, with the trips through the stop.
    out = render(notice('a', 'STP_024', 'MEDIUM', { entity_type: 'Stop', entity_id: 'S2', observed_value: '7', details: ctx }));
    expect(out.html).toContain('„Okul“ durağının sokaktan erişim bilgisi „7“; bu alan yalnız 0 (istasyonun içinden erişilir) ya da 1 (doğrudan sokaktan erişilir) olabilir. Bu durağa 12 sefer uğruyor (10).');

    // STP_029: the far parent and a nearer station, in the text, the table and on the map.
    out = render(notice('a', 'STP_029', 'MEDIUM', { entity_type: 'Stop', entity_id: 'S2', observed_value: '2900m', expected_value: '≤ 150m',
      details: { parent_id: 'ST', nearest_station_id: 'ST2', nearest_station_m: '110', ...ctx } }));
    expect(out.html).toContain('„Okul“ durağı bağlı olduğu „Gar“ istasyonundan 2,9 km uzakta; bir istasyonun peronu bu kadar uzakta olmamalı (sınır 150 m). 110 m ötedeki „Yeni Gar“ istasyonu kastedilmiş olabilir.');
    expect(out.html).toContain('<tr><td>Yeni Gar</td><td>Daha yakın istasyon</td><td>110 m</td></tr>');
    expect(out.map.opts?.pins.filter(p => p.primary)).toHaveLength(3);

    // STP_030: the stops around a platform-less station, linked to it.
    out = render(notice('a', 'STP_030', 'LOW', { entity_type: 'Stop', entity_id: 'ST', details: { nearby_stops: 'N1', nearby_m: '25' } }));
    expect(out.html).toContain('„Gar“ istasyonuna bağlı hiçbir peron yok');
    expect(out.html).toContain('Yakındaki istasyona bağlı olmayan duraklar: „Peron“ (25 m)');

    // STP_032: the summary row of a platform names its nearest station.
    out = render(notice('a', 'STP_032', 'MEDIUM', { entity_type: 'Feed', observed_value: '1',
      details: { affected_platforms: '1', example_platforms: 'N1', example_0_stop_id: 'N1', example_0_nearest_station_id: 'ST', example_0_nearest_station_m: '30' } }));
    expect(out.html).toContain('1 peron yürüme yollarıyla bağlı ama hiçbir istasyona bağlı değil');
    expect(out.html).toContain('30 m ötedeki „Gar“ istasyonu kastedilmiş olabilir.');
    expect(out.html).toContain('<tr class="bad"><td>Peron</td><td>Gar · 30 m</td></tr>');
  });

  it('STP_033–045 kartları bölgeleri, adresleri, numaraları ve istasyon bağlarını planlamacı diliyle anlatır', () => {
    const ni: NameIndex = {
      ...emptyNameIndex,
      stops: { S1: 'Hastane', S2: 'Okul', S3: 'Çarşı', ST: 'Gar', P1: 'Peron 1' },
      routes: { R1: '10' }, trips: { T1: 'Çarşı' }, trip_routes: { T1: 'R1' }, trip_first_dep: { T1: '06:00' },
      trip_stops: { T1: ['S1', 'S2', 'S3'] }, trip_stop_sequences: { T1: [1, 2, 3] },
      stop_coords: { S1: [41, 29], S2: [41.01, 29.01], S3: [41.0101, 29.01], ST: [41.03, 29.03], P1: [41.0301, 29.03] },
    };
    const render = (n: Notice) => {
      const root = { innerHTML: '', querySelectorAll: () => [], querySelector: () => null } as unknown as HTMLElement;
      const r = result([n], [], [[n.rule_id, 1, 1]], true, ni);
      renderProducer(root, r);
      return { html: root.innerHTML, map: plannerMap(n, r) };
    };
    const ctx = { trip_count: '12', route_count: '1', routes: 'R1', example_trip_id: 'T1', prev_stop_id: 'S1', next_stop_id: 'S3', stop_sequence: '2' };

    // STP_033: the neighbours' zones.
    let out = render(notice('a', 'STP_033', 'LOW', { entity_type: 'Stop', entity_id: 'S2', details: { ...ctx, prev_zone: 'A', next_zone: 'B', affected_stops: '1' } }));
    expect(out.html).toContain('„Okul“ durağının ücret bölgesi yok');
    expect(out.html).toContain('Seferde önceki durak „Hastane“ A bölgesinde, sonraki durak „Çarşı“ B bölgesinde.');

    // STP_035: the route whose address the stops repeat, the stops listed and pinned.
    out = render(notice('a', 'STP_035', 'LOW', { entity_type: 'Stop', entity_id: 'S1', observed_value: 'http://r.example/10',
      details: { normalized_url: 'r.example/10', stop_count: '2', representative_stop_ids: 'S1,S2', route_ids: 'R1' } }));
    expect(out.html).toContain('„Hastane“ durağının web adresi 10 hattının web adresiyle aynı (http://r.example/10); 2 durak aynı adresi kullanıyor.');
    expect(out.html).toContain('<tr class="bad"><td>Okul</td><td>S2</td></tr>');
    expect(out.map.opts?.pins.filter(p => p.primary).length).toBeGreaterThanOrEqual(2);

    // STP_037: the busiest stops without the information.
    out = render(notice('a', 'STP_037', 'INFO', { entity_type: 'Feed', observed_value: '2/3',
      details: { missing_stops: '2', stop_count: '3', example_stops: 'S2,S3', example_stop_calls: '40,12' } }));
    expect(out.html).toContain('3 durağın 2 tanesinde (%67) tekerlekli sandalye bilgisi yok');
    expect(out.html).toContain('En çok seferin uğradığı bilgisiz duraklar: „Okul“ (40 uğrayış), „Çarşı“ (12 uğrayış).');

    // STP_039: both stops, their distance and trips.
    out = render(notice('a', 'STP_039', 'MEDIUM', { entity_type: 'Stop', entity_id: 'S3', observed_value: '100', details: { other_stop_id: 'S2', ...ctx, other_trip_count: '4' } }));
    expect(out.html).toContain('„100“ durak numarası hem „Okul“ hem „Çarşı“ durağında kullanılıyor');
    expect(out.html).toContain('İki durak arası 11 m.');
    expect(out.html).toContain('<tr class="bad"><td>Okul</td><td>11 m</td>');

    // STP_040/044: the cleaned value suggested.
    out = render(notice('a', 'STP_040', 'INFO', { entity_type: 'Stop', entity_id: 'S1', observed_value: 'Hastane Stop', details: ctx }));
    expect(out.html).toContain('Önerilen: „Hastane“.');
    out = render(notice('a', 'STP_044', 'INFO', { entity_type: 'Stop', entity_id: 'P1', observed_value: 'Gleis 5', details: ctx }));
    expect(out.html).toContain('„Peron 1“ durağının peron numarası „Gleis 5“');
    expect(out.html).toContain('Önerilen: „5“.');

    // STP_041: the station's name suggested in the name, the station linked.
    out = render(notice('a', 'STP_041', 'INFO', { entity_type: 'Stop', entity_id: 'P1', observed_value: 'Peron 1', details: { parent_station: 'ST' } }));
    expect(out.html).toContain('„Peron 1“ adı bağlı olduğu „Gar“ istasyonunun adını içermiyor');
    expect(out.html).toContain('Önerilen: „Gar Peron 1“.');
    expect(out.map.opts?.pins.filter(p => p.primary)).toHaveLength(2);

    // STP_042: the summary row shows its own written address.
    out = render(notice('a', 'STP_042', 'LOW', { entity_type: 'Feed', observed_value: '1', details: { affected_stops: '1', example_stop_ids: 'S2', example_0_url: 'not a url' } }));
    expect(out.html).toContain('„Okul“ durağında yazılan adres: „not a url“.');

    // STP_043: why the access value is out of place.
    out = render(notice('a', 'STP_043', 'LOW', { entity_type: 'Stop', entity_id: 'S2', observed_value: '1', details: { location_type: '0' } }));
    expect(out.html).toContain('bu kayıt hiçbir istasyona bağlı değil.');
  });

  it('SHP_001–012 kartları çizimi hattıyla anlatır, hatalı noktayı ya da bölümü haritada kırmızı gösterir', () => {
    const ni: NameIndex = {
      ...emptyNameIndex,
      stops: { S1: 'Hastane', S2: 'Okul', S3: 'Çarşı' },
      routes: { R1: '10' }, trips: { T1: 'Çarşı' }, trip_routes: { T1: 'R1' }, trip_first_dep: { T1: '06:00' },
      trip_stops: { T1: ['S1', 'S2', 'S3'] }, trip_stop_sequences: { T1: [1, 2, 3] },
      stop_coords: { S1: [41, 29], S2: [41.01, 29], S3: [41.02, 29.5] },
      shape_coords: { SA: [[41, 29], [41.01, 29], [41.02, 29]] }, shape_routes: { SA: [['R1', '0']] },
    };
    const render = (n: Notice) => {
      const root = { innerHTML: '', querySelectorAll: () => [], querySelector: () => null } as unknown as HTMLElement;
      const r = result([n], [], [[n.rule_id, 1, 1]], true, ni);
      renderProducer(root, r);
      return { html: root.innerHTML, map: plannerMap(n, r) };
    };
    const ctx = { trip_count: '3', route_count: '1', routes: 'R1', example_trip_id: 'T1' };

    // SHP_002: the point by its row, the stretch it belongs to red and circled; the trip's stops listed.
    let out = render(notice('a', 'SHP_002', 'CRITICAL', { entity_type: 'Shape', entity_id: 'SA', line: 4, observed_value: 'abc',
      details: { ...ctx, prev_lat: '41', prev_lon: '29', prev_line: '3', next_lat: '41.01', next_lon: '29', next_line: '5' } }));
    expect(out.html).toContain('10 hattının „SA“ güzergâh çizimi: 4. satırdaki noktanın enlemi „abc“, geçerli bir değer değil');
    expect(out.html).toContain('Nokta haritada 3. ve 5. satırdaki noktaların arasında olmalı');
    expect(out.html).toContain('Haritadaki sefer: 10 · Çarşı yönü · 06:00 — durak sırası');
    expect(out.map.opts?.highlights).toHaveLength(1);
    expect(out.map.opts?.extraPolylines?.some(l => l.zoomTo)).toBe(true);
    expect(out.map.caption).toContain('Haritada: 10 hattının „SA“ güzergâh çizimi');

    // SHP_001: the shape of the rows around the code-less point, the point pinned.
    out = render(notice('a', 'SHP_001', 'CRITICAL', { entity_type: 'Shape', line: 9, file: 'shapes.txt',
      details: { lat: '41.03', lon: '29', prev_shape_id: 'SA', shape_routes: 'R1', shape_trip_count: '3', shape_example_trip_id: 'T1' } }));
    expect(out.html).toContain('Güzergâh noktaları listesinin 9. satırındaki noktanın çizim kodu boş');
    expect(out.html).toContain('Çevresindeki satırlar şu çizime ait: 10 hattının „SA“ güzergâh çizimi');
    expect(out.map.opts?.pins.some(p => p.primary && p.lat === 41.03)).toBe(true);
    expect(out.html).toContain('<td>Güzergâh çizimleri</td>');

    // SHP_005: the summary row's decrease; the stretch going back red.
    out = render(notice('a', 'SHP_005', 'CRITICAL', { entity_type: 'Feed', observed_value: '1',
      details: { affected_shapes: '1', example_shapes: 'SA', example_0_shape_id: 'SA', example_0_routes: 'R1', example_0_example_trip_id: 'T1',
        example_0_shape_pt_sequence: '3', example_0_prev_dist: '1000', example_0_dist: '900', example_0_lat: '41.02', example_0_lon: '29',
        example_0_prev_lat: '41.01', example_0_prev_lon: '29' } }));
    expect(out.html).toContain('1 güzergâh çiziminde kat edilen mesafe bir noktada geriye gidiyor. Örneğin 10 hattının „SA“ güzergâh çizimi: 3. noktada mesafe 1.000 iken 900 oluyor');
    expect(out.map.opts?.extraPolylines?.some(l => l.zoomTo)).toBe(true);

    // SHP_009: the crossing circled.
    out = render(notice('a', 'SHP_009', 'LOW', { entity_type: 'Shape', entity_id: 'SA', details: { ...ctx, cross_lat: '41.01', cross_lon: '29' } }));
    expect(out.html).toContain('10 hattının „SA“ güzergâh çizimi kendi üzerinden geçiyor');
    expect(out.map.opts?.highlights).toHaveLength(1);

    // SHP_012: the far stops with their distance, red on the map and in the stop list.
    out = render(notice('a', 'SHP_012', 'MEDIUM', { entity_type: 'Shape', entity_id: 'SA', observed_value: '1 (>100m)', details: { ...ctx, far_stops: 'S3', far_stop_m: '41000' } }));
    expect(out.html).toContain('10 hattının „SA“ güzergâh çizimi, onu kullanan seferlerin 1 durağından 100 m üzerinde uzaktan geçiyor');
    expect(out.html).toContain('En uzak durak „Çarşı“ (41 km).');
    expect(out.html).toContain('<tr class="bad"><td>3</td><td>Çarşı (S3)</td></tr>');
    expect(out.map.opts?.pins.filter(p => p.primary)).toHaveLength(1);
  });

  it('SHP_014–023 kartları çizimin uçlarını, yönünü, noktalarını ve kullanımını haritada gösterir', () => {
    const ni: NameIndex = {
      ...emptyNameIndex,
      stops: { S1: 'Hastane', S2: 'Okul', S3: 'Çarşı' },
      routes: { R1: '10' }, trips: { T1: 'Çarşı' }, trip_routes: { T1: 'R1' }, trip_first_dep: { T1: '06:00' },
      trip_stops: { T1: ['S1', 'S2', 'S3'] }, trip_stop_sequences: { T1: [1, 2, 3] },
      stop_coords: { S1: [41, 29], S2: [41.01, 29], S3: [41.02, 29] },
      shape_coords: { SE: [[40.99, 29], [41.01, 29], [41.02, 29]] }, shape_routes: { SE: [['R1', '0']] },
    };
    const render = (n: Notice) => {
      const root = { innerHTML: '', querySelectorAll: () => [], querySelector: () => null } as unknown as HTMLElement;
      const r = result([n], [], [[n.rule_id, 1, 1]], true, ni);
      renderProducer(root, r);
      return { html: root.innerHTML, map: plannerMap(n, r) };
    };
    const ctx = { trip_count: '3', route_count: '1', routes: 'R1', example_trip_id: 'T1' };

    // SHP_014: the line's start and the first stop, linked in red.
    let out = render(notice('a', 'SHP_014', 'INFO', { entity_type: 'Shape', entity_id: 'SE', observed_value: '1112m',
      details: { ...ctx, problem_stop: 'S1', endpoint: 'start', trip_id: 'T1' } }));
    expect(out.html).toContain('10 hattının „SE“ güzergâh çizimi: çizginin başladığı nokta, seferin ilk durağı „Hastane“ konumundan 1,1 km uzakta');
    expect(out.map.opts?.pins.some(p => p.label === 'Çizginin başı')).toBe(true);
    expect(out.html).toContain('<tr class="bad"><td>1</td><td>Hastane (S1)</td></tr>');

    // SHP_016: drawn backwards, the first stop named.
    out = render(notice('a', 'SHP_016', 'MEDIUM', { entity_type: 'Shape', entity_id: 'SE', details: { ...ctx, first_stop: 'S1', trip_id: 'T1' } }));
    expect(out.html).toContain('10 hattının „SE“ güzergâh çizimi ters yönde çizilmiş: seferler „Hastane“ durağından başlıyor');

    // SHP_017: the trip and the stop out of order, red in its stop list.
    out = render(notice('a', 'SHP_017', 'HIGH', { entity_type: 'Trip', entity_id: 'T1', details: { shape_id: 'SE', bad_stop: 'S2', ...ctx } }));
    expect(out.html).toContain('10 hattının „Çarşı“ yönündeki 06:00 kalkışlı seferi „Okul“ durağına, çizgi boyunca önceki durağın gerisinde');
    expect(out.html).toContain('<tr class="bad"><td>2</td><td>Okul (S2)</td></tr>');

    // SHP_018: the unused shape's size from its points.
    out = render(notice('a', 'SHP_018', 'LOW', { entity_type: 'Shape', entity_id: 'SE' }));
    expect(out.html).toContain('„SE“ güzergâh çizimi hiçbir sefer tarafından kullanılmıyor. Çizimin 3 noktası var');

    // SHP_019: the trips without stop times.
    out = render(notice('a', 'SHP_019', 'MEDIUM', { entity_type: 'Shape', entity_id: 'SE', observed_value: '1', details: { ...ctx, trip_ids: 'T9' } }));
    expect(out.html).toContain('1 sefere atanmış ama bu seferlerin hiçbirinin durak saati yok');
    expect(out.html).toContain('<tr class="bad"><td>T9</td><td>T9</td></tr>');

    // SHP_020: the two points at one place, circled.
    out = render(notice('a', 'SHP_020', 'INFO', { entity_type: 'Shape', entity_id: 'SE', details: { ...ctx, lat: '41.01', lon: '29', first_point: '2', second_point: '5' } }));
    expect(out.html).toContain('çizgi 2. ve 5. noktada aynı yerden geçiyor');
    expect(out.map.opts?.highlights).toHaveLength(1);

    // SHP_022: the stop near two parts of the line.
    out = render(notice('a', 'SHP_022', 'LOW', { entity_type: 'Stop', entity_id: 'S2', details: { shape_id: 'SE', parts: '2', trip_id: 'T1', ...ctx } }));
    expect(out.html).toContain('„Okul“ durağı şu çizimin 2 ayrı bölümüne yakın: 10 hattının „SE“ güzergâh çizimi');
  });

  it('SHP_024–030 kartları mesafe sorunlarını çizim ve seferle anlatır, işaret edilen yeri haritada gösterir', () => {
    const ni: NameIndex = {
      ...emptyNameIndex,
      stops: { S1: 'Hastane', S2: 'Okul', S3: 'Çarşı' },
      routes: { R1: '10' }, trips: { T1: 'Çarşı' }, trip_routes: { T1: 'R1' }, trip_first_dep: { T1: '06:00' },
      trip_stops: { T1: ['S1', 'S2', 'S3'] }, trip_stop_sequences: { T1: [1, 2, 3] },
      stop_coords: { S1: [41, 29], S2: [41.01, 29], S3: [41.02, 29] },
      shape_coords: { SA: [[41, 29], [41.01, 29], [41.02, 29]] }, shape_routes: { SA: [['R1', '0']] },
    };
    const render = (n: Notice) => {
      const root = { innerHTML: '', querySelectorAll: () => [], querySelector: () => null } as unknown as HTMLElement;
      const r = result([n], [], [[n.rule_id, 1, 1]], true, ni);
      renderProducer(root, r);
      return { html: root.innerHTML, map: plannerMap(n, r) };
    };
    const ctx = { trip_count: '1', route_count: '1', routes: 'R1', example_trip_id: 'T1' };

    // SHP_024: the stop and the place its distance points to, linked; the stop red in its trip.
    let out = render(notice('a', 'SHP_024', 'MEDIUM', { entity_type: 'Stop', entity_id: 'S2',
      details: { ...ctx, shape_id: 'SA', trip_id: 'T1', shape_dist_traveled: '1500', distance_m: '556', implied_lat: '41.015', implied_lon: '29' } }));
    expect(out.html).toContain('„Okul“ durağının durak saatlerindeki mesafesi (1.500) çizimde bu duraktan 556 m uzaktaki bir noktayı gösteriyor (10 hattının „SA“ güzergâh çizimi)');
    expect(out.map.opts?.pins.some(p => p.label === 'Mesafenin gösterdiği yer')).toBe(true);
    expect(out.html).toContain('<tr class="bad"><td>2</td><td>Okul (S2)</td></tr>');

    // SHP_025: both distances and the unit hint.
    out = render(notice('a', 'SHP_025', 'MEDIUM', { entity_type: 'Trip', entity_id: 'T1', details: { ...ctx, shape_id: 'SA', trip_max: '2500', shape_max: '2.5' } }));
    expect(out.html).toContain('10 hattının „Çarşı“ yönündeki 06:00 kalkışlı seferinin durak saatlerinde mesafe 2.500 değerine kadar çıkıyor');
    expect(out.html).toContain('Fark yaklaşık 1.000 kat');

    // SHP_028: two points with one distance, pinned and linked.
    out = render(notice('a', 'SHP_028', 'LOW', { entity_type: 'Shape', entity_id: 'SA', details: { ...ctx, lat: '41', lon: '29', next_lat: '41.001', next_lon: '29', dist: '0', apart_m: '111.19' } }));
    expect(out.html).toContain('art arda iki nokta aynı mesafeyi (0) taşıyor ama aralarında 111 m var');
    expect(out.map.opts?.pins.filter(p => p.primary)).toHaveLength(2);
    out = render(notice('a', 'SHP_029', 'INFO', { entity_type: 'Shape', entity_id: 'SA', details: { ...ctx, lat: '41', lon: '29', next_lat: '41.000005', next_lon: '29', dist: '500', apart_m: '0.55' } }));
    expect(out.html).toContain('aralarında yalnız 0,6 m var');

    // SHP_030: the trips using distances listed.
    out = render(notice('a', 'SHP_030', 'MEDIUM', { entity_type: 'Shape', entity_id: 'SA',
      details: { ...ctx, shape_point_count: '3', shape_points_missing_dist: '1', affected_trip_count: '1', representative_trip_ids: 'T1' } }));
    expect(out.html).toContain('1 sefer durak saatlerinde mesafe kullanıyor ama çizimin 3 noktasından 1 tanesinde mesafe yok');
    expect(out.html).toContain('<tr><td>10 · Çarşı yönü · 06:00</td><td>T1</td></tr>');
  });

  it('RTS_001–011 kartları hattı numarasıyla anlatır, hattın çizimlerini ve örnek seferini haritada gösterir', () => {
    const ni: NameIndex = {
      ...emptyNameIndex,
      stops: { S1: 'Hastane', S2: 'Okul', S3: 'Çarşı' },
      routes: { R1: '10' }, trips: { T1: 'Çarşı' }, trip_routes: { T1: 'R1' }, trip_first_dep: { T1: '06:00' },
      trip_stops: { T1: ['S1', 'S2', 'S3'] }, trip_stop_sequences: { T1: [1, 2, 3] }, trip_shapes: { T1: 'SA' },
      stop_coords: { S1: [41, 29], S2: [41.01, 29], S3: [41.02, 29] },
      shape_coords: { SA: [[41, 29], [41.02, 29]], SB: [[41.02, 29], [41, 29]] }, route_shapes: { R1: ['SA', 'SB'] },
    };
    const render = (n: Notice) => {
      const root = { innerHTML: '', querySelectorAll: () => [], querySelector: () => null } as unknown as HTMLElement;
      const r = result([n], [], [[n.rule_id, 1, 1]], true, ni);
      renderProducer(root, r);
      return { html: root.innerHTML, map: plannerMap(n, r) };
    };
    const ctx = { trip_count: '12', example_trip_id: 'T1' };

    // RTS_001: both rows; the trip numbered on the map and both directions drawn.
    let out = render(notice('a', 'RTS_001', 'CRITICAL', { entity_type: 'Route', entity_id: 'R1', line: 5,
      details: { ...ctx, first_line: '2', first_short_name: '10', first_long_name: 'İlk', first_type: '3', short_name: '11', long_name: 'İkinci', type: '0' } }));
    expect(out.html).toContain('„R1“ hat kodu hat listesinde iki kez yazılmış: 2. satırda „10 · İlk“, 5. satırda „11 · İkinci“.');
    expect(out.html).toContain('<tr class="bad"><td>5</td><td>11</td><td>İkinci</td><td>Tramvay (0)</td></tr>');
    expect(out.html).toContain('Haritadaki sefer: 10 · Çarşı yönü · 06:00 — durak sırası');
    expect(out.map.opts?.extraPolylines).toHaveLength(2);
    expect(out.map.opts?.pins).toHaveLength(3);

    // RTS_002: the defined agencies.
    out = render(notice('a', 'RTS_002', 'CRITICAL', { entity_type: 'Route', entity_id: 'R1', observed_value: '99', details: { ...ctx, known_agencies: '1=Belediye' } }));
    expect(out.html).toContain('10 hattı „99“ kodlu işletmeciye bağlanmış');
    expect(out.html).toContain('Listedeki işletmeciler: Belediye (1).');

    // RTS_003: a name offered from the first and last stops.
    out = render(notice('a', 'RTS_003', 'CRITICAL', { entity_type: 'Route', entity_id: 'R1', details: ctx }));
    expect(out.html).toContain('hat adı olarak „Hastane - Çarşı“ yazılabilir.');

    // RTS_004: the valid types by name.
    out = render(notice('a', 'RTS_004', 'CRITICAL', { entity_type: 'Route', entity_id: 'R1', observed_value: '99', details: ctx }));
    expect(out.html).toContain('10 hattının araç türü „99“, tanımlı bir tür değil. Geçerli türler: Tramvay (0)');

    // RTS_008: the badge previewed now and with a readable text colour.
    out = render(notice('a', 'RTS_008', 'LOW', { entity_type: 'Route', entity_id: 'R1', observed_value: '1.07', details: { ...ctx, color: 'FFFF00', text_color: 'FFFFFF' } }));
    expect(out.html).toContain('zemin FFFF00, yazı FFFFFF; kontrast 1,1:1');
    expect(out.html).toContain('Yazı rengi 000000 yapılırsa kontrast 19,6:1 olur.');
    expect(out.html).toContain('background:#FFFF00;border-color:#FFFF00;color:#000000');
    // An abbreviation does not end a step: "(ör. koyu zemine beyaz yazı)" stays in its sentence.
    expect(out.html).toContain('açık seçin (ör. koyu zemine beyaz yazı).');
    expect(out.html).not.toContain('<li>koyu zemine beyaz yazı).</li>');

    // RTS_011: the name itself.
    out = render(notice('a', 'RTS_011', 'LOW', { entity_type: 'Route', entity_id: 'R1', observed_value: '101', details: { ...ctx, name: 'Uzun ad' } }));
    expect(out.html).toContain('10 hattının adı 101 karakter: „Uzun ad“.');
  });

  it('RTS_012–023 kartları seferleri, takvimleri ve aynı adlı hatları anlatır, hatları haritada çizer', () => {
    const ni: NameIndex = {
      ...emptyNameIndex,
      stops: { S1: 'Hastane', S2: 'Okul', S3: 'Çarşı' },
      routes: { R1: '10', R2: '10', R0: '10', R4: '44' }, trips: { T1: 'Çarşı', T2: 'Okul' }, trip_routes: { T1: 'R1', T2: 'R2' }, trip_first_dep: { T1: '06:00', T2: '07:00' },
      trip_stops: { T1: ['S1', 'S2', 'S3'], T2: ['S3', 'S2'] }, trip_stop_sequences: { T1: [1, 2, 3], T2: [1, 2] },
      stop_coords: { S1: [41, 29], S2: [41.01, 29], S3: [41.02, 29] },
    };
    const render = (n: Notice) => {
      const root = { innerHTML: '', querySelectorAll: () => [], querySelector: () => null } as unknown as HTMLElement;
      const r = result([n], [], [[n.rule_id, 1, 1]], true, ni);
      renderProducer(root, r);
      return { html: root.innerHTML, map: plannerMap(n, r) };
    };
    const ctx = { trip_count: '12', example_trip_id: 'T1' };

    // RTS_012: no trips; the record with the same number named and its trip drawn.
    let out = render(notice('a', 'RTS_012', 'MEDIUM', { entity_type: 'Route', entity_id: 'R0', details: { same_name_routes: 'R1' } }));
    expect(out.html).toContain('10 hattının hiç seferi yok');
    expect(out.html).toContain('Aynı numarayla seferi olan başka kayıt var (kod: R1)');
    expect(out.map.opts?.pins).toHaveLength(3);

    // RTS_016: the calendars listed.
    out = render(notice('a', 'RTS_016', 'HIGH', { entity_type: 'Route', entity_id: 'R1', details: { ...ctx, services: 'NONE' } }));
    expect(out.html).toContain('10 hattının 12 seferi var ama bağlı oldukları takvimlerin hiçbirinde çalışma günü yok');
    expect(out.html).toContain('<td>NONE</td>');

    // RTS_019: each route with its trips; every route drawn.
    out = render(notice('a', 'RTS_019', 'LOW', { entity_type: 'Route', entity_id: 'R1', observed_value: '10 — Aynı',
      details: { ...ctx, conflicting_routes: 'R1, R2', route_0_route_id: 'R1', route_0_trips: '5', route_0_example_trip_id: 'T1', route_1_route_id: 'R2', route_1_trips: '3', route_1_example_trip_id: 'T2' } }));
    expect(out.html).toContain('„10 — Aynı“ adı 2 hatta kullanılıyor');
    expect(out.html).toContain('<tr class="bad"><td>R2</td><td>10</td><td>—</td><td>3</td><td>Okul yönü · 07:00</td></tr>');
    expect(out.map.opts?.extraPolylines?.length).toBeGreaterThanOrEqual(2);

    // RTS_022: the number removed from the long name, offered.
    out = render(notice('a', 'RTS_022', 'LOW', { entity_type: 'Route', entity_id: 'R4', observed_value: '44 Merkez', details: { trip_count: '1' } }));
    expect(out.html).toContain('44 hattının uzun adı „44 Merkez“ numarasını da içeriyor');
    expect(out.html).toContain('Önerilen: „Merkez“.');

    // RTS_023: which name the description repeats.
    out = render(notice('a', 'RTS_023', 'LOW', { entity_type: 'Route', entity_id: 'R1', observed_value: 'Merkez', details: { ...ctx, matched_field: 'route_long_name' } }));
    expect(out.html).toContain('10 hattının açıklaması „Merkez“ uzun adıyla aynı');
  });

  it('GEO kartları durağı, çizimi ve ağı planlamacı diliyle anlatır', () => {
    const line: [number, number][] = [[35.6, 139.7], [35.61, 139.7], [35.62, 139.7], [35.8, 139.7]];
    const ni: NameIndex = {
      ...emptyNameIndex,
      stops: { S1: 'Bir', S2: 'İki', S3: 'Üç', FAR: 'Uzak', Z: 'Sıfır', C1: 'Küme A', C2: 'Küme B' },
      routes: { R1: '10' }, trips: { T1: 'Üç', T3: 'Uzak' }, trip_routes: { T1: 'R1', T3: 'R1' }, trip_directions: { T1: '0', T3: '0' },
      trip_first_dep: { T1: '08:00', T3: '10:00' },
      trip_stops: { T1: ['S1', 'S2', 'S3'], T3: ['S3', 'FAR'], T2: ['S1', 'Z', 'S3'] }, trip_stop_sequences: { T1: [1, 2, 3], T3: [1, 2], T2: [1, 2, 3] },
      stop_coords: { S1: [35.6, 139.7], S2: [35.61, 139.702], S3: [35.62, 139.7], FAR: [-35.6, 139.7], Z: [0, 0], C1: [35.63, 139.7], C2: [35.63001, 139.70001] },
      trip_shapes: { T1: 'SH1' }, shape_coords: { SH1: line }, route_shapes: { R1: ['SH1'] }, shape_routes: { SH1: [['R1', '0']] },
    };
    const render = (n: Notice) => {
      const root = { innerHTML: '', querySelectorAll: () => [], querySelector: () => null } as unknown as HTMLElement;
      const r = result([n], [], [[n.rule_id, 1, 1]], true, ni);
      renderProducer(root, r);
      return { html: root.innerHTML, map: plannerMap(n, r) };
    };
    const stopCtx = { trip_count: '1', route_count: '1', routes: 'R1' };

    // GEO_002: the distance, the sign slip, the use; the middle and the likely position on the map.
    let out = render(notice('a', 'GEO_002', 'HIGH', { entity_type: 'Stop', entity_id: 'FAR', details: { ...stopCtx, example_trip_id: 'T3', prev_stop_id: 'S3', med_lat: '35.61', med_lon: '139.7', dist_km: '7919.3' } }));
    expect(out.html).toContain('„Uzak“ durağı ağın orta noktasından 7.919 km uzakta görünüyor');
    expect(out.html).toContain('Enlemin işareti çevrilirse');
    expect(out.html).toContain('<td>Olası doğru konum</td><td>35.6, 139.7</td>');
    expect(out.map.opts.pins.some(p => p.label === 'Olası doğru konum')).toBe(true);
    expect(out.map.opts.pins.some(p => p.label === 'Ağın orta noktası')).toBe(true);

    // GEO_016: the 0,0 stop's place between its neighbours, red on the line.
    out = render(notice('a', 'GEO_016', 'HIGH', { entity_type: 'Stop', entity_id: 'Z', details: { ...stopCtx, example_trip_id: 'T2', prev_stop_id: 'S1', next_stop_id: 'S3' } }));
    expect(out.html).toContain('„Sıfır“ durağının konumu 0,0 girilmiş');
    expect(out.html).toContain('Durak haritada bu iki durak arasında olmalı');
    expect(out.map.opts.extraPolylines?.some(l => l.zoomTo && l.color === '#dc2626')).toBe(true);

    // GEO_009: the distance to the line, the nearest point linked in red.
    out = render(notice('a', 'GEO_009', 'HIGH', { entity_type: 'Stop', entity_id: 'S2', observed_value: "180.3m (shape 'SH1')", expected_value: '≤ 100m',
      details: { ...stopCtx, example_trip_id: 'T1', shape_id: 'SH1' } }));
    expect(out.html).toContain('„İki“ durağı güzergâh çizgisinden 180 m uzakta');
    expect(out.map.opts.pins.some(p => p.label === 'Çizginin en yakın noktası')).toBe(true);

    // GEO_012: the cluster's other stops by name, all red, with their distance.
    out = render(notice('a', 'GEO_012', 'MEDIUM', { entity_type: 'Stop', entity_id: 'C1', details: { cluster_m: '5', example_stop_ids: 'C2', example_0_stop_id: 'C2', example_0_trip_count: '0' } }));
    expect(out.html).toContain('„Küme A“ durağının 5 m yakınında 1 durak daha var: „Küme B“');
    expect(out.map.opts.pins.filter(p => p.color === '#dc2626').length).toBe(2);

    // GEO_006: the long stretch between two numbered points, red and circled.
    out = render(notice('a', 'GEO_006', 'HIGH', { entity_type: 'Shape', entity_id: 'SH1',
      details: { routes: 'R1', example_trip_id: 'T1', dist_km: '20.02', threshold_km: '10', from_seq: '3', to_seq: '4', from_lat: '35.62', from_lon: '139.7', to_lat: '35.8', to_lon: '139.7', from_line: '4', to_line: '5' } }));
    expect(out.html).toContain('10 hattının „SH1“ güzergâh çizimi: 3. ve 4. noktalar arasında çizgi 20 km boyunca düz gidiyor (sınır 10 km)');
    expect(out.map.opts.extraPolylines?.some(l => l.zoomTo && l.coords.length === 2)).toBe(true);
    expect(out.map.opts.highlights?.length).toBe(1);

    // GEO_017: the 0,0 point left out of the line; its neighbours' stretch red.
    const withZero = { ...ni, shape_coords: { SH1: [[35.6, 139.7], [0, 0], [35.61, 139.7]] as [number, number][] } };
    const r17 = result([], [], [], true, withZero);
    const n17 = notice('a', 'GEO_017', 'HIGH', { entity_type: 'Shape', entity_id: 'SH1', line: 8,
      details: { routes: 'R1', shape_pt_sequence: '2', null_point_count: '3', prev_lat: '35.6', prev_lon: '139.7', prev_line: '7', next_lat: '35.61', next_lon: '139.7', next_line: '9' } });
    const m17 = plannerMap(n17, r17);
    expect(m17.opts.extraPolylines?.[0].coords).toEqual([[35.6, 139.7], [35.61, 139.7]]);
    out = render(n17);
    expect(out.html).toContain('2. nokta 0,0 konumunda');
    expect(out.html).toContain('Çizimde toplam 3 nokta 0,0 konumunda.');

    // GEO_014: the extent and the farthest stops by name and distance.
    out = render(notice('a', 'GEO_014', 'INFO', { entity_type: 'Feed', details: { span_km: '7920', ns_km: '7918', ew_km: '0', med_lat: '35.61', med_lon: '139.7', example_stop_ids: 'FAR', example_km: '7919', example_0_stop_id: 'FAR', example_0_trip_count: '1', example_0_routes: 'R1' } }));
    expect(out.html).toContain('Ağın orta noktasına en uzak duraklar: „Uzak“ (7.919 km).');
    expect(out.map.opts.pins.some(p => p.label.startsWith('Uzak'))).toBe(true);

    // A drawing with a 0,0 point never runs out to the ocean, here nor on another card.
    const zeroLine = { ...ni, shape_coords: { SH1: [[35.6, 139.7], [0, 0], [35.61, 139.702], [35.62, 139.7]] as [number, number][] } };
    const n16 = notice('a', 'GEO_016', 'HIGH', { entity_type: 'Stop', entity_id: 'Z', details: { ...stopCtx, example_trip_id: 'T1', prev_stop_id: 'S1', next_stop_id: 'S3' } });
    const m16 = plannerMap(n16, result([], [], [], true, { ...zeroLine, trip_stops: { ...ni.trip_stops, T1: ['S1', 'Z', 'S3'] } }));
    expect(m16.opts.extraPolylines?.every(l => l.coords.every(p => Math.abs(p[0]) > 0.1 || Math.abs(p[1]) > 0.1))).toBe(true);

    // GEO_019: the rounded stop and the stretch where it belongs.
    const rounded = { ...ni, stop_coords: { ...ni.stop_coords, I: [36, 140] as [number, number] }, stops: { ...ni.stops, I: 'Yuvarlak' }, trip_stops: { ...ni.trip_stops, T1: ['S1', 'I', 'S3'] } };
    const n19 = notice('a', 'GEO_019', 'MEDIUM', { entity_type: 'Stop', entity_id: 'I', details: { ...stopCtx, example_trip_id: 'T1', prev_stop_id: 'S1', next_stop_id: 'S3' } });
    const root19 = { innerHTML: '', querySelectorAll: () => [], querySelector: () => null } as unknown as HTMLElement;
    renderProducer(root19, result([n19], [], [['GEO_019', 1, 1]], true, rounded));
    expect(root19.innerHTML).toContain('„Yuvarlak“ durağının konumu tam sayıya yuvarlanmış (36, 140)');
    expect(plannerMap(n19, result([], [], [], true, rounded)).opts.extraPolylines?.some(l => l.zoomTo)).toBe(true);

    // GEO_022: a stop near the pole is not pinned; its place on the trip is shown.
    const pole = { ...ni, stop_coords: { ...ni.stop_coords, P: [89.5, 139.7] as [number, number] }, stops: { ...ni.stops, P: 'Kutup' }, trip_stops: { ...ni.trip_stops, T1: ['S1', 'P', 'S3'] } };
    const m22 = plannerMap(notice('a', 'GEO_022', 'HIGH', { entity_type: 'Stop', entity_id: 'P', details: { ...stopCtx, example_trip_id: 'T1', prev_stop_id: 'S1', next_stop_id: 'S3' } }), result([], [], [], true, pole));
    expect(m22.opts.pins.every(p => Math.abs(p.lat) <= 85)).toBe(true);
    expect(m22.opts.extraPolylines?.some(l => l.zoomTo)).toBe(true);

    // GEO_021: the crowded positions with their stops, red on the map.
    out = render(notice('a', 'GEO_021', 'HIGH', { entity_type: 'Feed', details: { shared_stops: '4', total_stops: '5', group_0_lat: '35.6', group_0_lon: '139.7', group_0_count: '2', group_0_stops: 'S1,S2' } }));
    expect(out.html).toContain('5 duraktan 4 tanesi (%80) başka bir durakla aynı konumda');
    expect(out.html).toContain('2 durak („Bir“, „İki“)');
    expect(out.map.opts.pins.some(p => p.label === '2 durak aynı yerde: Bir, İki')).toBe(true);

    // GEO_018: all stops in a small area, ringed.
    out = render(notice('a', 'GEO_018', 'HIGH', { entity_type: 'Feed', details: { stop_count: '5', span_m: '25', mid_lat: '35.6', mid_lon: '139.7', example_stop_ids: 'S1', example_0_stop_id: 'S1', example_0_trip_count: '1', example_0_routes: 'R1' } }));
    expect(out.html).toContain('Veri setindeki 5 durağın hepsi 25 m genişliğinde bir alanın içinde');
    expect(out.map.opts.highlights?.length).toBe(1);

    // GEO_020: the one place of all points, red.
    out = render(notice('a', 'GEO_020', 'HIGH', { entity_type: 'Shape', entity_id: 'SH1', details: { routes: 'R1', example_trip_id: 'T1', point_count: '3', lat: '35.6', lon: '139.7' } }));
    expect(out.html).toContain('10 hattının „SH1“ güzergâh çizimi: 3 noktanın hepsi aynı yerde (35.6, 139.7)');
    expect(out.map.opts.pins.some(p => p.label === 'Çizimin bütün noktaları burada')).toBe(true);
  });

  it('harita seferleri kuş uçuşu değil gerçek güzergâhtan çizer', () => {
    const line: [number, number][] = [[41, 29], [41.005, 29.02], [41.01, 29], [41.015, 29.02], [41.02, 29]];
    const other: [number, number][] = [[41.02, 29], [41.03, 29.02], [41.04, 29]];
    const ni: NameIndex = {
      ...emptyNameIndex,
      stops: { S1: 'Hastane', S2: 'Okul', S3: 'Çarşı', S4: 'Liman' },
      routes: { R1: '10', R2: '20' }, trips: { T1: 'Çarşı', T2: 'Çarşı', T3: 'Liman' },
      trip_routes: { T1: 'R1', T2: 'R1', T3: 'R2' }, trip_directions: { T1: '0', T2: '0', T3: '0' }, trip_first_dep: { T1: '06:00', T2: '07:00', T3: '08:00' },
      trip_stops: { T1: ['S1', 'S2', 'S3'], T2: ['S1', 'S2', 'S3'], T3: ['S3', 'S4'] }, trip_stop_sequences: { T1: [1, 2, 3], T2: [1, 2, 3], T3: [1, 2] },
      stop_coords: { S1: [41, 29], S2: [41.01, 29], S3: [41.02, 29], S4: [41.04, 29] },
      trip_shapes: { T1: 'A', T3: 'B' }, shape_coords: { A: line, B: other }, route_shapes: { R1: ['A'], R2: ['B'] },
      shape_routes: { A: [['R1', '0']], B: [['R2', '0']] },
    };
    const r = result([], [], [], true, ni);
    const coords = (n: Notice) => plannerMap(n, r).opts.extraPolylines?.map(l => l.coords);

    // TRP_022: the other trip of the block on its own line.
    expect(coords(notice('a', 'TRP_022', 'LOW', { entity_type: 'Trip', entity_id: 'T1', details: { other_trip_id: 'T3' } }))).toContainEqual(other);
    // VAT_008: each route on its line.
    expect(coords(notice('a', 'VAT_008', 'LOW', { entity_type: 'Feed', details: { routes: 'R1, R2' } }))).toEqual(expect.arrayContaining([line, other]));
    // STP_016: the other stop's trip on its line.
    expect(coords(notice('a', 'STP_016', 'LOW', { entity_type: 'Stop', entity_id: 'S1', details: { other_stop_id: 'S4', other_example_trip_id: 'T3', example_trip_id: 'T1' } }))).toContainEqual(other);
    // A trip without its own drawing: its route's line in the same direction.
    expect(coords(notice('a', 'TRP_022', 'LOW', { entity_type: 'Trip', entity_id: 'T2' }))?.[0]).toEqual(line);
    // A red stretch between two stops follows the line.
    expect(coords(notice('a', 'OPR_008', 'LOW', { entity_type: 'Trip', entity_id: 'T1', details: { stop_a: 'S1', stop_b: 'S2' } }))).toContainEqual(line.slice(0, 3));
    // A finding about a trip without a drawing keeps its straight line.
    expect(coords(notice('a', 'DQ_006', 'LOW', { entity_type: 'Trip', entity_id: 'T2' }))?.[0]).toEqual([[41, 29], [41.01, 29], [41.02, 29]]);

    // Large feeds: every drawing the map reads is asked for, not just the selected trip's.
    const deferred = result([], [], [], true, { ...ni, shape_coords: {}, map_data_deferred: true });
    expect(missingMapShapes(notice('a', 'VAT_008', 'LOW', { entity_type: 'Feed', details: { routes: 'R1, R2' } }), deferred).sort()).toEqual(['A', 'B']);
    expect(missingMapShapes(notice('a', 'TRP_022', 'LOW', { entity_type: 'Trip', entity_id: 'T1', details: { other_trip_id: 'T3' } }), deferred).sort()).toEqual(['A', 'B']);
  });

  it('RTS_024–031 kartları değeri, işletmeciyi ve aynı numaralı hatları planlamacı diliyle anlatır', () => {
    const ni: NameIndex = {
      ...emptyNameIndex,
      stops: { S1: 'Hastane', S2: 'Okul', S3: 'Çarşı' },
      routes: { R1: '10', R2: '10' }, route_long_names: { R1: 'Birinci', R2: 'İkinci' }, trips: { T1: 'Çarşı', T2: 'Okul' }, trip_routes: { T1: 'R1', T2: 'R2' }, trip_first_dep: { T1: '06:00', T2: '07:00' },
      trip_stops: { T1: ['S1', 'S2', 'S3'], T2: ['S3', 'S2'] }, trip_stop_sequences: { T1: [1, 2, 3], T2: [1, 2] },
      stop_coords: { S1: [41, 29], S2: [41.01, 29], S3: [41.02, 29] },
    };
    const render = (n: Notice) => {
      const root = { innerHTML: '', querySelectorAll: () => [], querySelector: () => null } as unknown as HTMLElement;
      const r = result([n], [], [[n.rule_id, 1, 1]], true, ni);
      renderProducer(root, r);
      return { html: root.innerHTML, map: plannerMap(n, r) };
    };
    const ctx = { trip_count: '12', example_trip_id: 'T1' };

    // RTS_025: the only agency offered.
    let out = render(notice('a', 'RTS_025', 'INFO', { entity_type: 'Route', entity_id: 'R1', details: { ...ctx, agency_name: 'Belediye' } }));
    expect(out.html).toContain('veri setinde tek işletmeci var („Belediye“), hat ona atanabilir.');
    // Technical details: "Hat kodu" is the route's number, not its record code.
    expect(out.html).toContain('<span>Hat kodu</span><code>10</code>');
    expect(out.html).not.toContain('<span>Hat kodu</span><code>R1</code>');

    // RTS_026: the routes sharing a number with their names; both drawn.
    out = render(notice('a', 'RTS_026', 'LOW', { entity_type: 'Route', entity_id: 'R1', observed_value: '10',
      details: { ...ctx, conflicting_routes: 'R1, R2', route_0_route_id: 'R1', route_0_trips: '5', route_0_example_trip_id: 'T1', route_1_route_id: 'R2', route_1_trips: '3', route_1_example_trip_id: 'T2' } }));
    expect(out.html).toContain('„10“ numarası farklı adlı 2 hatta kullanılıyor');
    expect(out.html).toContain('<tr class="bad"><td>R2</td><td>10</td><td>İkinci</td><td>3</td><td>Okul yönü · 07:00</td></tr>');
    expect(out.map.opts?.extraPolylines?.length).toBeGreaterThanOrEqual(2);

    // RTS_028: the on-demand trip and which continuous setting is on.
    out = render(notice('a', 'RTS_028', 'HIGH', { entity_type: 'Route', entity_id: 'R1', field: 'continuous_drop_off', observed_value: '0', details: { ...ctx, flex_trip_id: 'T1' } }));
    expect(out.html).toContain('10 hattında talebe bağlı sefer var (Çarşı yönü · 06:00) ama yol boyunca iniş açık');

    // RTS_030: the extended code by name and its nearest basic type.
    out = render(notice('a', 'RTS_030', 'LOW', { entity_type: 'Route', entity_id: 'R1', observed_value: '700', details: ctx }));
    expect(out.html).toContain('genişletilmiş bir kodla yazılmış: Otobüs (700).');
    expect(out.html).toContain('en yakın temel tür Otobüs (3).');

    // RTS_031: the code-less row by what it carries.
    out = render(notice('a', 'RTS_031', 'CRITICAL', { entity_type: 'Route', line: 9, file: 'routes.txt', details: { route_short_name: '70', route_long_name: 'Adsız' } }));
    expect(out.html).toContain('Hat listesinin 9. satırındaki hattın kodu boş („70 · Adsız“)');
    expect(out.html).toContain('<td>70</td>');
  });

  it('STM_051–055 kartları Flex bölgesini, pencereyi, yazılan değeri ve aynı saatli durak serisini gösterir', () => {
    const ni: NameIndex = {
      ...emptyNameIndex,
      stops: { S1: 'Hastane', S2: 'Okul', S3: 'Çarşı', S4: 'Liman' },
      routes: { R1: '10' }, trips: { T1: 'Merkez' }, trip_routes: { T1: 'R1' }, trip_first_dep: { T1: '08:00' },
      trip_stops: { T1: ['S1', 'S2', 'S3', 'S4'] }, trip_stop_sequences: { T1: [1, 2, 3, 4] },
      stop_coords: { S1: [41, 29], S2: [41.01, 29.01], S3: [41.02, 29.02], S4: [41.03, 29.03] },
    };
    const render = (n: Notice) => {
      const root = { innerHTML: '', querySelectorAll: () => [], querySelector: () => null } as unknown as HTMLElement;
      const r = result([n], [], [[n.rule_id, 1, 1]], true, ni);
      renderProducer(root, r);
      return { html: root.innerHTML, map: plannerMap(n, r) };
    };
    const flex = { location_id: 'Z1', stop_sequence: '1', window_start: '08:00:00', window_end: '12:00:00' };
    const trip = (rule: string, extra: Partial<Notice>) => notice('a', rule, 'HIGH', { entity_type: 'Trip', entity_id: 'T1', details: flex, ...extra });

    let out = render(trip('STM_051', { field: 'pickup_type', observed_value: '3' }));
    expect(out.html).toContain('Bölge Z1 için talebe bağlı hizmet penceresi 08:00–12:00, ama biniş türü 3 (sürücüyle ayarlanır).');
    expect(out.html).toContain('<tr class="bad"><td>Z1</td><td>1</td><td>08:00–12:00</td><td>3 (sürücüyle ayarlanır)</td></tr>');
    expect(out.html).toContain('<th>Biniş türü</th>');
    expect(out.html).toContain('Bölge Z1 · 08:00–12:00');
    // A zone row has no stop to mark: the trip's stops are drawn grey.
    expect(out.map.opts?.pins.length).toBe(4);
    expect(out.map.opts?.pins.filter(p => p.primary)).toHaveLength(0);

    // The empty cell the spec counts as 0.
    out = render(trip('STM_052', { field: 'drop_off_type', observed_value: 'value=0; source=implicit_default' }));
    expect(out.html).toContain('ama iniş türü boş (0 sayılır, yani normal).');

    // A stop-based Flex row names and marks the stop.
    out = render(trip('STM_054', { field: 'continuous_pickup', observed_value: '0', details: { stop_id: 'S2', stop_sequence: '2', window_start: '08:00:00' } }));
    expect(out.html).toContain('Okul durağı için talebe bağlı hizmet penceresi 08:00–—, ama hat boyunca biniş ayarı 0 (duraklar arasında serbest).');
    expect(out.html).toContain('<tr class="bad"><td>Okul</td><td>2</td><td>08:00–—</td><td>0 (duraklar arasında serbest)</td></tr>');
    expect(out.map.opts?.pins.filter(p => p.primary)).toHaveLength(1);

    out = render(trip('STM_055', { field: 'continuous_drop_off', observed_value: '2' }));
    expect(out.html).toContain('hat boyunca iniş ayarı 2 (telefonla ayarlanır)');

    out = render(notice('a', 'STM_053', 'INFO', {
      entity_type: 'Route', entity_id: 'R1', observed_value: '4 trips; max 3 consecutive stops', details: {
        example_trip_id: 'T1', run_length: '3',
        example_0_stop_id: 'S1', example_0_stop_sequence: '1', example_0_arrival: '08:00:00', example_0_departure: '08:00:00', example_0_role: 'before',
        example_1_stop_id: 'S2', example_1_stop_sequence: '2', example_1_arrival: '08:10:00', example_1_departure: '08:10:00', example_1_role: 'run',
        example_2_stop_id: 'S3', example_2_stop_sequence: '3', example_2_arrival: '08:10:00', example_2_departure: '08:10:00', example_2_role: 'run',
        example_3_stop_id: 'S4', example_3_stop_sequence: '4', example_3_arrival: '08:10:00', example_3_departure: '08:10:00', example_3_role: 'run',
      },
    }));
    expect(out.html).toContain('4 seferde arka arkaya birçok durağa aynı saat yazılmış. Örnek seferde Okul ile Liman arasındaki 3 durağın hepsinde saat 08:10.');
    expect(out.html).toContain('10 hattı');
    expect(out.html).toContain('<tr><td>1</td><td>Hastane</td><td>08:00</td><td>08:00</td></tr>');
    expect(out.html).toContain('<tr class="bad"><td>3</td><td>Çarşı</td><td>08:10</td><td>08:10</td></tr>');
    expect(out.html).toContain('Okul → Liman · 08:10');
    // The run ends the example trip: no next stop to bound the times.
    expect(out.html).toContain('<li>Okul ile Liman arasındaki 3 durağın saatlerini gerçek geçiş süresine göre birbirinden ayırın; şu an hepsinde 08:10 yazıyor.</li>');
    expect(out.html).toContain('<li>Duraklar gerçekten bu kadar yakınsa ve aynı dakikaya düşüyorsa değişiklik gerekmez.</li>');
    expect(out.map.opts?.pins.filter(p => p.primary)).toHaveLength(3);
    expect(out.map.opts?.pins.length).toBe(4);

    out = render(notice('a', 'STM_053', 'INFO', {
      entity_type: 'Route', entity_id: 'R1', observed_value: '1 trips; max 2 consecutive stops', details: {
        example_trip_id: 'T1', run_length: '2',
        example_0_stop_id: 'S2', example_0_stop_sequence: '2', example_0_arrival: '08:10:00', example_0_role: 'run',
        example_1_stop_id: 'S3', example_1_stop_sequence: '3', example_1_arrival: '08:10:00', example_1_role: 'run',
        example_2_stop_id: 'S4', example_2_stop_sequence: '4', example_2_arrival: '08:20:00', example_2_role: 'after',
      },
    }));
    expect(out.html).toContain('<li>Okul ile Çarşı arasındaki 2 durağın saatlerini, 08:10 ile sonraki durağın saati 08:20 arasında gerçek geçiş süresine göre dağıtın.</li>');
    expect(out.html).toContain('<li>Süre bilinmiyorsa bu durakları yaklaşık olarak işaretleyin (timepoint=0).</li>');
  });

  it('STM_058–061 kartları pencereyi, rezervasyon tarafını, çakışan bölgeleri ve uzak durak çiftini gösterir', () => {
    const ni: NameIndex = {
      ...emptyNameIndex,
      stops: { S1: 'Hastane', S2: 'Okul', S3: 'Çarşı' },
      routes: { R1: '10' }, trips: { T1: 'Merkez' }, trip_routes: { T1: 'R1' }, trip_first_dep: { T1: '08:00' },
      trip_stops: { T1: ['S1', 'S2', 'S3'] }, trip_stop_sequences: { T1: [1, 2, 3] },
      stop_coords: { S1: [41, 29], S2: [41.01, 29.01], S3: [41.2, 29.2] },
    };
    const render = (n: Notice) => {
      const root = { innerHTML: '', querySelectorAll: () => [], querySelector: () => null } as unknown as HTMLElement;
      const r = result([n], [], [[n.rule_id, 1, 1]], true, ni);
      renderProducer(root, r);
      return { html: root.innerHTML, map: plannerMap(n, r) };
    };
    const trip = (rule: string, extra: Partial<Notice>) => notice('a', rule, 'HIGH', { entity_type: 'Trip', entity_id: 'T1', ...extra });

    let out = render(trip('STM_058', { field: 'start_pickup_drop_off_window', observed_value: '9am', details: {
      location_id: 'Z1', stop_sequence: '1', window_start: '9am', window_end: '12:00:00',
    } }));
    expect(out.html).toContain('Bölge Z1 için hizmet penceresinin başlangıç saati “9am” yazılmış; bu geçerli bir saat değil.');
    expect(out.html).toContain('<tr class="bad"><td>Z1</td><td>1</td><td>9am</td><td>12:00</td></tr>');

    out = render(trip('STM_059', { field: 'pickup_booking_rule_id', details: {
      stop_id: 'S2', stop_sequence: '2', pickup_type: '2', drop_off_type: '0',
    } }));
    expect(out.html).toContain('Okul durağı (2. sıra) için biniş “telefonla rezervasyon” (2) seçilmiş');
    expect(out.html).toContain('<tr class="bad"><td>Okul</td><td>2</td><td>2</td><td>eksik</td></tr>');
    expect(out.html).toContain('Okul · Durak sırası 2');
    expect(out.map.opts?.pins.filter(p => p.primary)).toHaveLength(1);

    out = render(trip('STM_060', { details: {
      zone_a_id: 'Z1', zone_a_stop_sequence: '1', zone_a_window_start: '08:00:00', zone_a_window_end: '10:00:00', zone_a_pickup: 'true', zone_a_drop_off: 'false',
      zone_b_id: 'Z2', zone_b_stop_sequence: '2', zone_b_window_start: '09:00:00', zone_b_window_end: '11:00:00', zone_b_pickup: 'true', zone_b_drop_off: 'true',
      overlap: 'pickup', window_overlap_start: '09:00:00', window_overlap_end: '10:00:00',
      zone_a_ring: '41,29;41,29.1;41.1,29.1;41,29', zone_b_ring: '41.05,29.05;41.05,29.2;41.2,29.2;41.05,29.05',
    } }));
    expect(out.html).toContain('Bu seferde Z1 (1. sıra, 08:00–10:00) ve Z2 (2. sıra, 09:00–11:00) bölgeleri haritada kesişiyor; pencereler 09:00–10:00 arasında çakışıyor ve ikisinde de biniş açık.');
    expect(out.html).toContain('<tr class="bad"><td>Z1</td><td>1</td><td>08:00–10:00</td><td>açık / kapalı</td></tr>');
    expect(out.html).toContain('Bölge Z1 / Z2 · 09:00–10:00');
    const rings = out.map.opts?.extraPolylines?.filter(line => line.color === '#dc2626') ?? [];
    expect(rings).toHaveLength(2);
    expect(rings[0].coords[1]).toEqual([41, 29.1]);
    expect(rings[0].zoomTo).toBe(true);

    out = render(notice('a', 'STM_061', 'HIGH', { entity_type: 'Route', entity_id: 'R1:0:S1→S3', details: {
      stop_a: 'S1', stop_b: 'S3', route_id: 'R1', route_label: '10', direction_id: '0', trip_count: '2',
      dist_km: '28.3', speed_min: '340', speed_max: '425', stop_a_sequence: '1', stop_b_sequence: '3', threshold_kmh: '120',
      example_trip_id: 'T1', stop_a_departure: '09:00:00', stop_b_arrival: '09:04:00', between_stops: '1', trips: 'T1',
    } }));
    expect(out.html).toContain('Hastane (1. sıra) ile Çarşı (3. sıra) arası kuş uçuşu 28,3 km. En hızlı örnek seferde 09:00 kalkış ile 09:04 varış arasında 4 dakika var; bu 425 km/h demek, bu hat türünün sınırı 120 km/h.');
    expect(out.html).toContain('<li>09:00 kalkış ve 09:04 varış saatlerini kontrol edin;');
    expect(out.html).toContain('10 hattı');
    expect(out.html).toContain('<tr class="bad"><td>1</td><td>Hastane</td><td>—</td><td>09:00</td></tr>');
    expect(out.html).toContain('<td>1 ara durak</td>');
    expect(out.html).toContain('<tr class="bad"><td>3</td><td>Çarşı</td><td>09:04</td><td>—</td></tr>');
    const red = out.map.opts?.pins.filter(p => p.primary).map(p => p.label) ?? [];
    expect(red).toEqual([expect.stringContaining('durak sırası 1'), expect.stringContaining('durak sırası 3')]);
    expect(out.map.opts?.pins).toHaveLength(3);
  });

  it('STM_022 özeti örnek satırlara açılır; kart durağı, yazılan değeri ve ölçülen deseni gösterir', () => {
    const ni: NameIndex = {
      ...emptyNameIndex,
      stops: { S1: 'Hastane', S2: 'Okul' },
      routes: { R1: '10' },
      trips: { T1: 'Merkez yönü', T2: 'Merkez yönü' },
      trip_routes: { T1: 'R1', T2: 'R1' },
      trip_first_dep: { T1: '08:00', T2: '09:00' },
      trip_stops: { T1: ['S1', 'S2'], T2: ['S1', 'S2'] },
      stop_coords: { S1: [41.0, 29.0], S2: [41.01, 29.01] },
    };
    const summary = notice('a', 'STM_022', 'MEDIUM', {
      entity_type: 'Feed', file: 'stop_times.txt', field: 'timepoint', observed_value: '3', expected_value: 'timepoint 0 or 1',
      details: {
        affected_rows: '3', example_trips: 'T1, T2', value_0: '2', value_0_count: '2', value_1: 'abc', value_1_count: '1',
        example_0_trip: 'T1', example_0_line: '2', example_0_observed: '2', example_0_stop_id: 'S1', example_0_stop_sequence: '1', example_0_arrival: '08:00:00', example_0_departure: '08:00:00',
        example_1_trip: 'T2', example_1_line: '4', example_1_observed: 'abc', example_1_stop_id: 'S1', example_1_stop_sequence: '1', example_1_arrival: '09:00:00', example_1_departure: '09:00:00',
        example_2_trip: 'T1', example_2_line: '3', example_2_observed: '2', example_2_stop_id: 'S2', example_2_stop_sequence: '2', example_2_arrival: '08:10:00', example_2_departure: '08:10:00',
      },
    });
    const r = result([summary], [], [['STM_022', 2, 3]], true, ni);
    const root = { innerHTML: '', querySelectorAll: () => [], querySelector: () => null } as unknown as HTMLElement;
    renderProducer(root, r);
    const html = root.innerHTML;
    expect(html).toContain('Kesin saat işareti geçersiz');
    expect(html).toContain('Hastane durağında (durak sırası 1) saatin kesin mi yaklaşık mı olduğunu belirten işaret “2” yazılmış');
    expect(html).toContain('Hatalı 3 durak saatinin 2 tanesinde aynı değer (“2”) var');
    expect(html).toContain('<th>Kesinlik işareti</th>');
    expect(html).toContain('<tr class="bad"><td>Hastane</td><td>1</td><td>08:00</td><td>08:00</td><td>2</td></tr>');
    expect(html).toContain('Okul · durak sırası 2 · işaret “2”');
    expect(html).toContain('Hastane · durak sırası 1 · işaret “abc”');
    // One summary expands to its three example rows.
    expect((html.match(/data-row="/g) ?? []).length).toBeGreaterThanOrEqual(3);
    const row = notice('b', 'STM_022', 'MEDIUM', {
      entity_type: 'Trip', entity_id: 'T1', observed_value: '2', details: { stop_id: 'S1', stop_sequence: '1' },
    });
    const red = plannerMap(row, r).opts?.pins.filter(pin => pin.primary) ?? [];
    expect(red.map(pin => pin.label)).toEqual([expect.stringContaining('Hastane')]);
  });

  it('STM_021 kartı aynı noktadaki iki durağı sırası ve koordinatıyla tabloda, haritada tek işaretle gösterir', () => {
    const ni: NameIndex = {
      ...emptyNameIndex,
      stops: { S0: 'Çarşı', S1: 'Hastane', S2: 'Hastane Karşı', S3: 'Okul' },
      routes: { R1: '10' },
      trips: { T1: 'Merkez yönü' },
      trip_routes: { T1: 'R1' },
      trip_first_dep: { T1: '07:15' },
      trip_stops: { T1: ['S0', 'S1', 'S2', 'S3'] },
      trip_stop_sequences: { T1: [4, 5, 6, 7] },
      stop_coords: { S0: [41.0, 29.0], S1: [41.0072, 29.0153], S2: [41.0072, 29.0153], S3: [41.01, 29.02] },
    };
    const n = notice('a', 'STM_021', 'LOW', {
      entity_type: 'Trip', entity_id: 'T1', file: 'stop_times.txt', field: 'stop_id', line: 12,
      observed_value: 'S1 → S2', expected_value: '> 0 m',
      details: {
        stop_a: 'S1', stop_b: 'S2', stop_a_sequence: '5', stop_b_sequence: '6',
        prev_stop_id: 'S0', prev_stop_sequence: '4', next_stop_id: 'S3', next_stop_sequence: '7',
      },
    });
    const r = result([n], [], [['STM_021', 1, 1]], true, ni);
    const root = { innerHTML: '', querySelectorAll: () => [], querySelector: () => null } as unknown as HTMLElement;
    renderProducer(root, r);
    const html = root.innerHTML;
    expect(html).toContain('Ardışık iki durak aynı yerde');
    expect(html).toContain('Hastane (durak sırası 5) ile hemen ardından gelen Hastane Karşı (durak sırası 6) aynı koordinatta (41.0072, 29.0153)');
    expect(html).toContain('<th>Enlem</th>');
    expect(html).toContain('<th>Boylam</th>');
    expect(html).toContain('<th>Önceki duraktan mesafe (m)</th>');
    expect(html).toContain('<tr class="bad"><td>Hastane Karşı</td><td>6</td><td>41.0072</td><td>29.0153</td><td>0</td></tr>');
    expect(html).toContain('<td>Çarşı</td><td>4</td>');
    expect(html).toContain('<td>Okul</td><td>7</td>');
    expect(html).toContain('Hastane → Hastane Karşı · 0 m');
    const map = plannerMap(n, r);
    const red = map.opts?.pins.filter(pin => pin.primary) ?? [];
    expect(red).toHaveLength(1);
    expect(red[0].label).toContain('Hastane (sıra 5) ve Hastane Karşı (sıra 6) aynı noktada');
    expect(map.opts?.focusZoom).toBe(17);
  });

  it('STM_056 kartı eksik komşuyu tireyle yazmaz, bütün durakları aynı değerdeki seferi sefer olarak anlatır', () => {
    const ni: NameIndex = {
      ...emptyNameIndex,
      stops: { S1: 'Natürel Park Sitesi', S2: 'Tivoli Park Sitesi', S3: 'Doğa Park Villaları', S4: 'Gökkuşağı Villaları' },
      routes: { R1: '131' },
      trips: { T1: 'Merkez' },
      trip_routes: { T1: 'R1' },
      trip_first_dep: { T1: '07:00' },
      trip_stops: { T1: ['S1', 'S2', 'S3', 'S4'] },
      trip_shapes: { T1: 'SH1' },
    };
    const render = (details: Record<string, string>, shapeReversed = false) => {
      const n = notice('a', 'STM_056', 'CRITICAL', {
        entity_type: 'Trip', entity_id: 'T1', field: 'shape_dist_traveled', observed_value: '25.644928', expected_value: '> 25.644928', details,
      });
      const shp016 = notice('s', 'SHP_016', 'HIGH', { entity_type: 'Shape', entity_id: 'SH1', field: 'shape_id' });
      const root = { innerHTML: '', querySelectorAll: () => [], querySelector: () => null } as unknown as HTMLElement;
      renderProducer(root, result(shapeReversed ? [n, shp016] : [n], ['a'], [['STM_056', 5, 1], ['SHP_016', 2, 1]], true, ni));
      return root.innerHTML;
    };
    const pair = {
      prev_stop_id: 'S1', stop_id: 'S2', next_stop_id: 'S3', next2_stop_id: 'S4',
      prev_stop_sequence: '1', stop_sequence: '2', prev_shape_dist_traveled: '10', shape_dist_traveled: '10',
    };
    let html = render(pair);
    expect(html).toContain('Durak sırası: Natürel Park Sitesi → Tivoli Park Sitesi → Doğa Park Villaları → Gökkuşağı Villaları.');
    expect(html).not.toContain('— →');
    expect(html).not.toContain('kuş uçuşu');

    html = render({ ...pair, trip_constant_value: '25.644928', trip_stop_count: '46', constant_trips: '2010' });
    expect(html).toContain('Bu seferin 46 durağının hepsinde güzergâh başından itibaren kümülatif mesafe aynı (25.644928)');
    expect(html).toContain('aynı durumdaki sefer sayısı 2.010.');
    expect(html).toContain('Dışa aktarma ayarını kontrol edin');
    expect(html).not.toContain('ters yönde çizilmiş');

    // SHP_016 on the trip's shape: the card names the reversed line and asks to fix it first.
    html = render({ ...pair, trip_constant_value: '25.644928', trip_stop_count: '46' }, true);
    expect(html).toContain('Bu seferin güzergâh çizgisi ters yönde çizilmiş (ilk durak çizginin sonunda)');
    expect(html).toContain('Önce güzergâh çizgisinin yönünü düzeltin');
    html = render(pair, true);
    expect(html).toContain('ters yönde çizilmiş');
  });

  it('AGN_003 kartında yalnız motorun bildirdiği geçersiz URL ve gözlenen değer gösterilir', () => {
    const r = result(
      [notice('a', 'AGN_003', 'CRITICAL', {
        entity_type: 'Agency', entity_id: 'A1', observed_value: ' http://www.wavetransit.com',
      })],
      ['a'],
      [['AGN_003', 4, 1]],
      true,
      emptyNameIndex,
      { agency_count: 1, agencies: [{ agency_id: 'A1', agency_name: 'Wave Transit' }] },
    );
    const root = { innerHTML: '', querySelectorAll: () => [], querySelector: () => null } as unknown as HTMLElement;
    renderProducer(root, r);
    expect(root.innerHTML).toContain("İşletmecinin web adresi geçersiz");
    expect(root.innerHTML).toContain('␠http://www.wavetransit.com');
    expect(root.innerHTML).not.toContain('başındaki boşluğu');
  });

  it('STP_001 ve STP_012 kartlarında planlamacı dili ve durak kodu gösterilir', () => {
    const r = result(
      [
        notice('a', 'STP_001', 'CRITICAL', { entity_type: 'Stop', entity_id: '71717' }),
        notice('b', 'STP_012', 'CRITICAL', { entity_type: 'Stop', entity_id: '71717' }),
      ],
      ['a', 'b'],
      [['STP_001', 5, 1], ['STP_012', 5, 1]],
    );
    const root = { innerHTML: '', querySelectorAll: () => [], querySelector: () => null } as unknown as HTMLElement;
    renderProducer(root, r);
    expect(root.innerHTML).toContain('Aynı durak kodu birden fazla kayıtta kullanılmış');
    expect(root.innerHTML).toContain('İstasyon veya giriş kaydı seferde kullanılmış');
    expect(root.innerHTML).toContain('Durak kodu: 71717');
    expect(root.innerHTML).not.toContain('top_id yineleniyor');
  });

  it('STM_012 kartında hızı ve duraklar arasındaki kontrol sırasını planlamacı diliyle gösterir', () => {
    const ni: NameIndex = {
      ...emptyNameIndex,
      routes: { R6: '6' },
      trips: { T1: 'Hastane' },
      trip_routes: { T1: 'R6' },
      trip_first_dep: { T1: '08:45' },
      stops: { S0: 'Önceki', S1: 'Merkez', S2: 'Hastane', S3: 'Sonraki' },
    };
    const r = result(
      [notice('a', 'STM_012', 'HIGH', {
        entity_type: 'Trip', entity_id: 'T1', observed_value: '768 km/h',
        details: {
          prev_stop_id: 'S0', prev_arrival: '08:40:00', prev_departure: '08:41:00',
          stop_a: 'S1', stop_a_arrival: '08:42:00', stop_a_departure: '08:43:00',
          stop_b: 'S2', stop_b_arrival: '08:45:20', stop_b_departure: '08:46:00',
          next_stop_id: 'S3', next_arrival: '08:50:00', next_departure: '08:51:00',
          distance_km: '29.4', distance_meters: '29400', scheduled_gap_seconds: '140', speed_kmh: '768.0',
        },
      })],
      [],
      [['STM_012', 3, 500]],
      true,
      ni,
    );
    const root = { innerHTML: '', querySelectorAll: () => [], querySelector: () => null } as unknown as HTMLElement;
    renderProducer(root, r);
    expect(root.innerHTML).toContain('Duraklar arasında gerçekçi olmayan hız');
    expect(root.innerHTML).toContain('6 hattı · Hastane yönü · sefer başlangıcı 08:45');
    expect(root.innerHTML).toContain('Görülen değer:</span> <code>768 km/h</code>');
    expect(root.innerHTML).toContain('Merkez ile Hastane arasında hesaplanan hız gerçekçi görünmüyor');
    expect(root.innerHTML).toContain('Merkez → Hastane · 29.400 m · 768 km/h');
    expect(root.innerHTML).not.toContain('768 km/h hız hesaplanmış');
    expect(root.innerHTML).toContain('Önce bu seferin iki durak arasındaki varış ve kalkış saatlerini kontrol edin');
    expect(root.innerHTML).not.toContain('trip_id');
  });

  it('STM_012 serbest gözlenen değeri ayrı gösterir ve eksik bağlamı cümleye katmaz', () => {
    const r = result(
      [notice('a', 'STM_012', 'HIGH', {
        entity_type: 'Trip', entity_id: 'T1', observed_value: 'sıfır süre, 29.4 km',
        details: { distance_km: '29.4', scheduled_gap_seconds: '0', speed_lower_bound_kmh: '1764.0' },
      })],
      [],
      [['STM_012', 3, 1]],
    );
    const root = { innerHTML: '', querySelectorAll: () => [], querySelector: () => null } as unknown as HTMLElement;
    renderProducer(root, r);
    expect(root.innerHTML).toContain('Görülen değer:</span> <code>kayıtlı süre: 0 saniye · mesafe: 29,4 km</code>');
    expect(root.innerHTML).toContain('iki ardışık durak arasında hesaplanan hız gerçekçi görünmüyor');
    expect(root.innerHTML).not.toContain('sıfır süre, 29.4 km hız');
    expect(root.innerHTML).not.toContain('belirtilmemiş hattı');
    expect(root.innerHTML).not.toContain('belirtilmemiş yönü');
  });

  it('STM_015 ve STM_016 kartlarında hat ve yolcuya gösterilen yön bağlamını gösterir', () => {
    const ni: NameIndex = {
      ...emptyNameIndex,
      routes: { R6: '6' },
      trips: { T1: 'Hastane' },
      trip_routes: { T1: 'R6' },
      trip_first_dep: { T1: '07:15' },
    };
    const r = result(
      [
        notice('a', 'STM_015', 'CRITICAL', { entity_type: 'Trip', entity_id: 'T1' }),
        notice('b', 'STM_016', 'CRITICAL', { entity_type: 'Trip', entity_id: 'T1' }),
      ],
      ['a', 'b'],
      [['STM_015', 5, 1], ['STM_016', 5, 1]],
      true,
      ni,
    );
    const root = { innerHTML: '', querySelectorAll: () => [], querySelector: () => null } as unknown as HTMLElement;
    renderProducer(root, r);
    expect(root.innerHTML).toContain('İlk durakta varış saati eksik');
    expect(root.innerHTML).toContain('Son durakta varış saati eksik');
    expect(root.innerHTML).toContain('6 hattı · Hastane yönü · sefer başlangıcı 07:15');
    expect(root.innerHTML).not.toContain('trip_id');
  });

  it('ARC_031 kartında veri seti bilgisi planlamacı diliyle gösterilir', () => {
    const r = result(
      [notice('a', 'ARC_031', 'CRITICAL', { entity_type: 'Feed' })],
      ['a'],
      [['ARC_031', 5, 1]],
    );
    const root = { innerHTML: '', querySelectorAll: () => [], querySelector: () => null } as unknown as HTMLElement;
    renderProducer(root, r);
    expect(root.innerHTML).toContain('Çeviri dosyası varken veri seti bilgisi eksik');
    expect(root.innerHTML).toContain('feed dilini belirtin');
    expect(root.innerHTML).not.toContain('translations.txt varken feed_info.txt eksik');
  });
});

describe('Planlamacı alanı ve hat rengi', () => {
  it('iş alanını kural önekine değil bildirimin dosyasına göre seçer', () => {
    expect(plannerArea('STM_002', notice('a', 'STM_002', 'CRITICAL', { file: 'stop_times.txt', entity_type: 'Stop' }))).toBe('tarife');
    expect(plannerArea('RTS_005', notice('a', 'RTS_005', 'MEDIUM', { file: 'routes.txt', entity_type: 'Route' }))).toBe('hat');
    expect(plannerArea('CAL_014', notice('a', 'CAL_014', 'MEDIUM', { entity_type: 'Service' }))).toBe('takvim');
    expect(plannerArea('ARC_025', notice('a', 'ARC_025', 'CRITICAL', { file: 'stops.txt', entity_type: 'File' }))).toBe('teknik');
    expect(plannerArea('DQ_016', notice('a', 'DQ_016', 'MEDIUM', { entity_type: 'Feed' }))).toBe('teknik');
  });

  it('hat rozetini tarifedeki renkle kurar, metin rengi yoksa okunur karşıtını seçer', () => {
    const ni: NameIndex = { ...emptyNameIndex, routes: { R1: '34', R2: '12', R3: '7' }, route_colors: { R1: ['B5121B', 'FFFFFF'], R2: ['FFE100', ''] } };
    expect(routeBadge('R1', ni)).toEqual({ label: '34', bg: '#B5121B', fg: '#FFFFFF' });
    expect(routeBadge('R2', ni)).toEqual({ label: '12', bg: '#FFE100', fg: '#000000' });
    expect(routeBadge('R3', ni)).toEqual({ label: '7', bg: null, fg: null });
  });
});

describe('Planlamacılar İçin sayfası', () => {
  const root = () => ({ innerHTML: '', querySelectorAll: () => [], querySelector: () => null }) as unknown as HTMLElement;

  it('planlamacı metni olmayan konuyu teknik bölüme ayırır, yazılmış olanı üstte tutar', () => {
    const r = result(
      [notice('a', 'STM_002', 'CRITICAL', { entity_type: 'Stop', entity_id: 'S1' }), notice('b', 'ZZZ_999', 'MEDIUM')],
      [],
      [['STM_002', 4, 1], ['ZZZ_999', 2, 1]],
    );
    const el = root();
    renderProducer(el, r);
    const technicalAt = el.innerHTML.indexOf('planner-technical');
    expect(technicalAt).toBeGreaterThan(0);
    expect(el.innerHTML.indexOf('data-rule="STM_002"')).toBeLessThan(technicalAt);
    expect(el.innerHTML.indexOf('data-rule="ZZZ_999"')).toBeGreaterThan(technicalAt);
  });

  it('mevcut önem derecesini ve kural kodunu kart başlığında gösterir, yayın kararını rapordaki dille söyler', () => {
    const r = result([notice('a', 'STM_015', 'CRITICAL', { entity_type: 'Trip', entity_id: 'T1' })], ['a'], [['STM_015', 4, 1]]);
    const el = root();
    renderProducer(el, r);
    expect(el.innerHTML).toContain('<span class="planner-rule-id">STM_015</span>');
    expect(el.innerHTML).toContain('KRİTİK');
    expect(el.innerHTML).toContain('Yayınlanması Tavsiye Edilmez');
    expect(el.innerHTML).not.toContain('Yayınlanamaz');
    expect(el.innerHTML).not.toContain('Düzeltilmeli');
  });

  it('sefer satırına takvimin çalışma günlerini yazar', () => {
    const ni: NameIndex = {
      ...emptyNameIndex, routes: { R1: '34' }, trip_routes: { T1: 'R1', T2: 'R1' },
      service_days: { HI: ['1111100', '20260101', '20261231', '0'], OZ: ['', '20260520', '20260521', '2'] },
    };
    const r = result([
      notice('a', 'STM_015', 'CRITICAL', { entity_type: 'Trip', entity_id: 'T1', service_id: 'HI' }),
      notice('b', 'STM_015', 'CRITICAL', { entity_type: 'Trip', entity_id: 'T2', service_id: 'OZ' }),
    ], ['a', 'b'], [['STM_015', 4, 2]], true, ni);
    const el = root();
    renderProducer(el, r);
    expect(el.innerHTML).toContain('<th>Çalışma günleri</th>');
    expect(el.innerHTML).toContain('<td>Hafta içi</td>');
    expect(el.innerHTML).toContain('<td>Belirli tarihler (+2 istisna)</td>');
  });

  it('STP_009 kartında alan adı yerine planlamacı dili kullanılır', () => {
    const ni: NameIndex = { ...emptyNameIndex, stops: { '10043': 'Onkoloji' } };
    const r = result([notice('a', 'STP_009', 'CRITICAL', { entity_type: 'Stop', entity_id: '10043', observed_value: '0' })], ['a'], [['STP_009', 4, 1]], true, ni);
    const el = root();
    renderProducer(el, r);
    expect(el.innerHTML).toContain('Durağın bağlı olduğu istasyon bulunamadı');
    expect(el.innerHTML).toContain('Durak: Onkoloji · bağlı gösterilen istasyon kodu: 0');
  });

  it('sefer konusunun tablosunda dosya/alan değil hat, yön ve kalkış sütunları vardır', () => {
    const ni: NameIndex = { ...emptyNameIndex, routes: { R1: '34' }, trip_routes: { T1: 'R1' }, trips: { T1: 'Üniversite' }, trip_first_dep: { T1: '07:15' } };
    const r = result([notice('a', 'STM_015', 'MEDIUM', { entity_type: 'Trip', entity_id: 'T1', file: 'stop_times.txt' })], [], [['STM_015', 2, 1]], true, ni);
    const el = root();
    renderProducer(el, r);
    expect(el.innerHTML).toContain('<th>Hat</th><th>Yön</th><th>Sefer başlangıç saati</th></tr>');
    expect(el.innerHTML).toContain('<td>Üniversite</td>');
    expect(el.innerHTML).not.toContain('<th>Dosya</th>');
  });

  it('STM_001 describes the missing trip from its stop-time evidence', () => {
    const tripId = '1323072442';
    const r = result([
      notice('a', 'STM_001', 'CRITICAL', {
        entity_type: 'Trip', entity_id: tripId, observed_value: tripId,
        file: 'stop_times.txt', field: 'trip_id', line: 42,
        details: {
          first_stop_id: 'S1', last_stop_id: 'S2', first_departure: '06:15:00',
          last_arrival: '07:02:00', stop_count: '23',
        },
      }),
    ], ['a'], [['STM_001', 4, 1]], true, {
      ...emptyNameIndex,
      stops: { S1: 'Otogar', S2: 'Şehir Merkezi' },
      trip_stops: { [tripId]: ['S1', 'S3', 'S2'] },
    });
    const el = root();
    renderProducer(el, r);
    expect(el.innerHTML).toContain('Otogar (kalkış 06:15) ile Şehir Merkezi (varış 07:02) arasında 23 duraklı bir seferin saatleri var');
    expect(el.innerHTML).toContain('Planlama yazılımında bu durak ve saatlerle çalışan seferi bulun');
    expect(el.innerHTML).not.toContain('Sefer kodu</th>');
    expect(el.innerHTML).not.toContain('Çalışma günleri</th>');
    expect(el.innerHTML).toContain('<th>Hat</th><th>Yön</th><th>Sefer başlangıç saati</th>');
    expect(el.innerHTML).toContain('<td>—</td><td>—</td><td>06:15</td>');
    expect(el.innerHTML).not.toContain(`<td>${tripId}</td>`);
  });

  it('STM_006 identifies the route, service days, and exact stop-time row', () => {
    const r = result([
      notice('a', 'STM_006', 'CRITICAL', {
        entity_type: 'Trip', entity_id: 'trip-technical', service_id: 'calendar-technical',
        details: {
          arrival: '05:45:00', departure: '05:45:00',
          prev_stop_id: 'S1', prev_arrival: '05:40:00', prev_departure: '05:40:00',
          next_stop_id: 'S3', next_arrival: '05:50:00', next_departure: '05:50:00',
        },
      }),
    ], ['a'], [['STM_006', 4, 1]], true, {
      ...emptyNameIndex,
      routes: { R1: 'SFS' }, stops: { S1: 'Önceki durak', S3: 'Sonraki durak' }, trip_routes: { 'trip-technical': 'R1' },
      trips: { 'trip-technical': 'Descendante' }, trip_first_dep: { 'trip-technical': '05:45' },
      service_days: { 'calendar-technical': ['', '20250915', '20250915', '1'] },
    });
    const el = root();
    renderProducer(el, r);
    expect(el.innerHTML).toContain('SFS hattı · Descendante yönü · Sefer başlangıç saati: 05:45 · Belirli tarihler (+1 istisna)');
    expect(el.innerHTML).toContain('Bu durak saatinde varış 05:45, kalkış 05:45; durak seçilmemiş.');
    expect(el.innerHTML).toContain('<th>Hat</th><th>Yön</th><th>Sefer başlangıç saati</th><th>Hatalı yer</th><th>Çalışma günleri</th><th>Durak varış saati</th><th>Durak kalkış saati</th>');
    expect(el.innerHTML).toContain('Durak seçilmemiş · varış 05:45 · kalkış 05:45');
    expect(el.innerHTML).toContain('<td>05:45</td><td>Durak seçilmemiş · varış 05:45 · kalkış 05:45</td><td>Belirli tarihler (+1 istisna)</td><td>05:45</td><td>05:45</td>');
    expect(el.innerHTML).toContain('<strong>1</strong><span>durak saati</span>');
    expect(el.innerHTML).toContain('<span>Sefer kodu</span><code>trip-technical</code>');
    expect(el.innerHTML).not.toContain('<td>trip-technical</td>');
  });

  it('STM_009 identifies the affected stop and explains pickup values 0–3', () => {
    const r = result([
      notice('a', 'STM_009', 'HIGH', {
        entity_type: 'Trip', entity_id: 'T1', file: 'stop_times.txt', field: 'pickup_type', line: 42,
        observed_value: 'invalid', expected_value: '0-3',
        details: { stop_id: 'S1', stop_sequence: '25', arrival: '06:40:00', departure: '06:40:00' },
      }),
    ], ['a'], [['STM_009', 3, 1]], true, {
      ...emptyNameIndex,
      stops: { S1: 'Central Library' }, routes: { R1: 'SFS' },
      trips: { T1: 'Downtown' }, trip_routes: { T1: 'R1' }, trip_first_dep: { T1: '06:00' },
    });
    const el = root();
    renderProducer(el, r);
    expect(el.innerHTML).toContain('Central Library · durak sırası 25 · biniş kuralı');
    expect(el.innerHTML).toContain('0 — normal biniş; 1 — biniş yok; 2 — biniş için işletmeyi telefonla arayın; 3 — biniş için sürücüyle önceden anlaşın.');
    expect(el.innerHTML).not.toContain('<td>S1</td>');
  });

  it('STM_010 identifies the affected stop and explains drop-off values 0–3', () => {
    const r = result([
      notice('a', 'STM_010', 'HIGH', {
        entity_type: 'Trip', entity_id: 'T1', file: 'stop_times.txt', field: 'drop_off_type', line: 42,
        observed_value: 'invalid', expected_value: '0-3',
        details: { stop_id: 'S2', stop_sequence: '7', arrival: '08:00:00', departure: '08:00:00' },
      }),
    ], ['a'], [['STM_010', 3, 1]], true, {
      ...emptyNameIndex,
      stops: { S2: 'City Hall' }, routes: { R1: 'SFS' },
      trips: { T1: 'Downtown' }, trip_routes: { T1: 'R1' }, trip_first_dep: { T1: '06:00' },
    });
    const el = root();
    renderProducer(el, r);
    expect(el.innerHTML).toContain('City Hall · durak sırası 7 · iniş kuralı');
    expect(el.innerHTML).toContain('0 — normal iniş; 1 — iniş yok; 2 — iniş için işletmeyi telefonla arayın; 3 — iniş için sürücüyle önceden anlaşın.');
  });
});

describe('Saat kuralları ve kayıt sayıları', () => {
  const root = () => ({ innerHTML: '', querySelectorAll: () => [], querySelector: () => null }) as unknown as HTMLElement;

  it('STM_015 için ilk durağı ve sonraki iki gerçek durağı, STM_008 için iki durağın saatini yazar', () => {
    const ni: NameIndex = { ...emptyNameIndex, stops: { S1: 'Onkoloji', S2: 'Binevler', S3: 'Otogar', A: 'Çarşı', B: 'Liman' }, trip_routes: { T1: 'R1', T2: 'R1' }, routes: { R1: 'S08' } };
    const r = result([
      notice('a', 'STM_015', 'CRITICAL', { entity_type: 'Trip', entity_id: 'T1', details: {
        stop_id: 'S1', stop_sequence: '1', departure: '06:15:00', next_stop_id: 'S2', next_time: '06:18:00',
        following_stop_id: 'S2', following_stop_sequence: '2', following_arrival: '06:18:00', following_departure: '06:18:00',
        following2_stop_id: 'S3', following2_stop_sequence: '3', following2_arrival: '06:22:00', following2_departure: '06:22:00',
      } }),
      notice('b', 'STM_008', 'CRITICAL', { entity_type: 'Trip', entity_id: 'T2', details: { stop_a: 'A', stop_b: 'B', dep: '08:00:00', arr: '07:55:00' } }),
    ], ['a', 'b'], [['STM_015', 4, 1], ['STM_008', 4, 1]], true, ni);
    const el = root();
    renderProducer(el, r);
    expect(el.innerHTML).toContain('Seferin ilk durağı olan Onkoloji için varış saati girilmemiş.');
    expect(el.innerHTML).toContain('<td>Onkoloji · kalkış 06:15:00</td>');
    expect(el.innerHTML).toContain('<td>Binevler</td><td>2</td><td>06:18:00</td><td>06:18:00</td>');
    expect(el.innerHTML).toContain('<td>Otogar</td><td>3</td><td>06:22:00</td><td>06:22:00</td>');
    expect(el.innerHTML).toContain('<td>Çarşı 08:00:00 → Liman 07:55:00</td>');
    expect(el.innerHTML).toContain('Çarşı durağından 08:00:00 kalkışından sonra Liman durağına 07:55:00 varışı yazılmış');
    expect(el.innerHTML).not.toContain('stop_times.txt zaman değerlerini');
  });

  it('STM_015 haritasında gerçek sefer çizgisini ve bütün durakları gösterip ilk durağa odaklanır', () => {
    const ni: NameIndex = {
      ...emptyNameIndex,
      stops: { S1: 'Onkoloji', S2: 'Binevler', S3: 'Otogar' },
      routes: { R1: 'S08' }, trip_routes: { T1: 'R1' },
      trip_shapes: { T1: 'SH1' },
      shape_coords: { SH1: [[37.01, 37.31], [37.02, 37.32], [37.03, 37.33]] },
      trip_stops: { T1: ['S1', 'S2', 'S3'] },
      trip_stop_sequences: { T1: [1, 2, 3] },
      stop_coords: { S1: [37.01, 37.31], S2: [37.02, 37.32], S3: [37.03, 37.33] },
    };
    const n = notice('a', 'STM_015', 'CRITICAL', {
      entity_type: 'Trip', entity_id: 'T1',
      details: { stop_id: 'S1', stop_sequence: '1', following_stop_id: 'S2', following_stop_sequence: '2' },
    });
    const map = plannerMap(n, result([n], ['a'], [['STM_015', 4, 1]], true, ni));
    expect(map.opts.extraPolylines?.[0].coords).toEqual(ni.shape_coords.SH1);
    expect(map.opts.pins).toHaveLength(3);
    expect(map.opts.pins.find(pin => pin.primary)?.label).toContain('Onkoloji');
    expect(map.opts.focus).toEqual(ni.stop_coords.S1);
    expect(map.opts.focusZoom).toBe(14);
  });

  it('STM_016 için önceki iki gerçek durağı gösterir ve haritada son durağa odaklanır', () => {
    const ni: NameIndex = {
      ...emptyNameIndex,
      stops: { S1: 'Onkoloji', S2: 'Binevler', S3: 'Otogar' },
      routes: { R1: 'S08' }, trip_routes: { T1: 'R1' },
      trip_shapes: { T1: 'SH1' },
      shape_coords: { SH1: [[37.01, 37.31], [37.02, 37.32], [37.03, 37.33]] },
      trip_stops: { T1: ['S1', 'S2', 'S3'] }, trip_stop_sequences: { T1: [1, 2, 3] },
      stop_coords: { S1: [37.01, 37.31], S2: [37.02, 37.32], S3: [37.03, 37.33] },
    };
    const n = notice('a', 'STM_016', 'CRITICAL', {
      entity_type: 'Trip', entity_id: 'T1', details: {
        stop_id: 'S3', stop_sequence: '3', departure: '06:22:00',
        preceding2_stop_id: 'S1', preceding2_stop_sequence: '1', preceding2_departure: '06:15:00',
        preceding_stop_id: 'S2', preceding_stop_sequence: '2', preceding_arrival: '06:18:00', preceding_departure: '06:18:00',
      },
    });
    const r = result([n], ['a'], [['STM_016', 4, 1]], true, ni);
    const el = root();
    renderProducer(el, r);
    expect(el.innerHTML).toContain('<td>Onkoloji</td><td>1</td><td>—</td><td>06:15:00</td>');
    expect(el.innerHTML).toContain('<td>Binevler</td><td>2</td><td>06:18:00</td><td>06:18:00</td>');
    expect(el.innerHTML).toContain('<tr class="bad"><td>Otogar</td><td>3</td><td>eksik</td><td>06:22:00</td>');
    const map = plannerMap(n, r);
    expect(map.opts.extraPolylines?.[0].coords).toEqual(ni.shape_coords.SH1);
    expect(map.opts.pins).toHaveLength(3);
    expect(map.opts.pins.find(pin => pin.primary)?.label).toContain('Otogar');
    expect(map.opts.focus).toEqual(ni.stop_coords.S3);
    expect(map.opts.focusZoom).toBe(14);
  });

  it('STM_018 çalışma takvimini, gerçek hat bölümünü ve geçersiz ayarı gösterir', () => {
    const ni: NameIndex = {
      ...emptyNameIndex,
      stops: { S1: 'Onkoloji', S2: 'Binevler', S3: 'Otogar' },
      routes: { R1: 'S08' }, trips: { T1: 'Otogar' }, trip_routes: { T1: 'R1' },
      trip_directions: { T1: '0' }, trip_first_dep: { T1: '06:15' },
      trip_shapes: { T1: 'SH1' }, route_shapes: { R1: ['SH1'] }, shape_routes: { SH1: [['R1', '0']] },
      shape_coords: { SH1: [[37.00, 37.30], [37.005, 37.305], [37.01, 37.31], [37.02, 37.32]] },
      trip_stops: { T1: ['S1', 'S2', 'S3'] }, trip_stop_sequences: { T1: [1, 2, 3] },
      stop_coords: { S1: [37.00, 37.30], S2: [37.01, 37.31], S3: [37.02, 37.32] },
      service_days: { HI: ['1111100', '20260101', '20261231', '0'] },
    };
    const n = notice('a', 'STM_018', 'MEDIUM', {
      entity_type: 'Trip', entity_id: 'T1', service_id: 'HI', observed_value: '9', expected_value: '0-3',
      details: {
        stop_id: 'S1', stop_sequence: '1', arrival: '06:15:00', departure: '06:15:00',
        next_stop_id: 'S2', next_stop_sequence: '2', next_arrival: '06:18:00', next_departure: '06:18:00',
      },
    });
    const r = result([n], [], [['STM_018', 2, 1]], true, ni);
    const el = root();
    renderProducer(el, r);
    expect(el.innerHTML).toContain('Hat bölümünde biniş ayarı geçersiz');
    expect(el.innerHTML).toContain('S08 hattı · Otogar yönü · sefer başlangıcı 06:15');
    expect(el.innerHTML).toContain('Onkoloji ile Binevler arasındaki hat bölümünde yolcu alma ayarı “9” olarak yazılmış.');
    expect(el.innerHTML).toContain('Hat genelindeki ayarı kullanacaksanız bu alanı boş bırakın.');
    expect(el.innerHTML).toContain('<th>Çalışma günleri</th>');
    expect(el.innerHTML).toContain('<td>Hafta içi</td>');
    expect(el.innerHTML).toContain('<tr class="bad"><td>Onkoloji</td><td>1</td><td>06:15</td><td>06:15</td><td>9</td>');
    expect(el.innerHTML).toContain('<td>Binevler</td><td>2</td><td>06:18</td><td>06:18</td><td>Bölümün sonu</td>');
    const map = plannerMap(n, r);
    expect(map.opts.extraPolylines?.[0].coords).toEqual(ni.shape_coords.SH1);
    expect(map.opts.extraPolylines?.[1].coords).toEqual(ni.shape_coords.SH1.slice(0, 3));
    expect(map.opts.extraPolylines?.[1].zoomTo).toBe(true);
    expect(map.opts.pins).toHaveLength(3);
    expect(map.opts.pins.find(pin => pin.primary)?.label).toContain('Onkoloji');
    expect(map.opts.focus).toBeUndefined();
  });

  it('STM_018 feed özetini gerçek sefer örneklerine açıp çalışma günlerini gösterir', () => {
    const ni: NameIndex = {
      ...emptyNameIndex,
      stops: { S1: 'Hospital', S2: 'Roosevelt' }, routes: { R1: 'Purple' },
      trips: { T1: 'Hospital' }, trip_routes: { T1: 'R1' }, trip_first_dep: { T1: '05:35' },
      service_days: { WEEKDAY: ['1111100', '20210601', '20240701', '0'] },
    };
    const summary = notice('a', 'STM_018', 'MEDIUM', {
      entity_type: 'Feed', file: 'stop_times.txt', observed_value: '1664',
      details: {
        affected_rows: '1664', example_0_trip: 'T1', example_0_line: '2',
        example_0_service_id: 'WEEKDAY', example_0_observed: '-999', example_0_expected: '0-3',
        example_0_stop_id: 'S1', example_0_stop_sequence: '1', example_0_arrival: '05:35:00', example_0_departure: '05:35:00',
        example_0_next_stop_id: 'S2', example_0_next_stop_sequence: '2', example_0_next_arrival: '05:42:00', example_0_next_departure: '05:42:00',
      },
    });
    const el = root();
    renderProducer(el, result([summary], [], [['STM_018', 2, 1664]], true, ni));
    expect(el.innerHTML).toContain('<strong>1.664</strong><span>durak saati</span>');
    expect(el.innerHTML).toContain('<th>Çalışma günleri</th>');
    expect(el.innerHTML).toContain('<td>Hafta içi</td>');
    expect(el.innerHTML).toContain('<td>Hospital → Roosevelt</td><td>-999</td>');
    expect(el.innerHTML).toContain('Hospital ile Roosevelt arasındaki hat bölümünde yolcu alma ayarı “-999” olarak yazılmış.');
  });

  it('STM_019 çalışma takvimini, gerçek hat bölümünü ve geçersiz ayarı gösterir', () => {
    const ni: NameIndex = {
      ...emptyNameIndex,
      stops: { S1: 'Onkoloji', S2: 'Binevler', S3: 'Otogar' },
      routes: { R1: 'S08' }, trips: { T1: 'Otogar' }, trip_routes: { T1: 'R1' },
      trip_directions: { T1: '0' }, trip_first_dep: { T1: '06:15' },
      trip_shapes: { T1: 'SH1' }, route_shapes: { R1: ['SH1'] }, shape_routes: { SH1: [['R1', '0']] },
      shape_coords: { SH1: [[37.00, 37.30], [37.005, 37.305], [37.01, 37.31], [37.02, 37.32]] },
      trip_stops: { T1: ['S1', 'S2', 'S3'] }, trip_stop_sequences: { T1: [1, 2, 3] },
      stop_coords: { S1: [37.00, 37.30], S2: [37.01, 37.31], S3: [37.02, 37.32] },
      service_days: { HI: ['1111100', '20260101', '20261231', '0'] },
    };
    const n = notice('a', 'STM_019', 'MEDIUM', {
      entity_type: 'Trip', entity_id: 'T1', service_id: 'HI', observed_value: '9', expected_value: '0-3',
      details: {
        stop_id: 'S1', stop_sequence: '1', arrival: '06:15:00', departure: '06:15:00',
        next_stop_id: 'S2', next_stop_sequence: '2', next_arrival: '06:18:00', next_departure: '06:18:00',
      },
    });
    const r = result([n], [], [['STM_019', 2, 1]], true, ni);
    const el = root();
    renderProducer(el, r);
    expect(el.innerHTML).toContain('Hat bölümünde iniş ayarı geçersiz');
    expect(el.innerHTML).toContain('S08 hattı · Otogar yönü · sefer başlangıcı 06:15');
    expect(el.innerHTML).toContain('Onkoloji ile Binevler arasındaki hat bölümünde yolcu indirme ayarı “9” olarak yazılmış.');
    expect(el.innerHTML).toContain('Hat genelindeki ayarı kullanacaksanız bu alanı boş bırakın.');
    expect(el.innerHTML).toContain('<th>Çalışma günleri</th>');
    expect(el.innerHTML).toContain('<td>Hafta içi</td>');
    expect(el.innerHTML).toContain('<th>Hat boyunca iniş ayarı</th>');
    expect(el.innerHTML).toContain('<tr class="bad"><td>Onkoloji</td><td>1</td><td>06:15</td><td>06:15</td><td>9</td>');
    expect(el.innerHTML).toContain('<td>Binevler</td><td>2</td><td>06:18</td><td>06:18</td><td>Bölümün sonu</td>');
    const map = plannerMap(n, r);
    expect(map.opts.extraPolylines?.[0].coords).toEqual(ni.shape_coords.SH1);
    expect(map.opts.extraPolylines?.[1].coords).toEqual(ni.shape_coords.SH1.slice(0, 3));
    expect(map.opts.extraPolylines?.[1].zoomTo).toBe(true);
    expect(map.opts.pins).toHaveLength(3);
    expect(map.opts.pins.find(pin => pin.primary)?.label).toContain('Onkoloji');
    expect(map.opts.focus).toBeUndefined();
  });

  it('STM_019 feed özetini gerçek sefer örneklerine açıp çalışma günlerini gösterir', () => {
    const ni: NameIndex = {
      ...emptyNameIndex,
      stops: { S1: 'Hospital', S2: 'Roosevelt' }, routes: { R1: 'Purple' },
      trips: { T1: 'Hospital' }, trip_routes: { T1: 'R1' }, trip_first_dep: { T1: '05:35' },
      service_days: { WEEKDAY: ['1111100', '20210601', '20240701', '0'] },
    };
    const summary = notice('a', 'STM_019', 'MEDIUM', {
      entity_type: 'Feed', file: 'stop_times.txt', observed_value: '1664',
      details: {
        affected_rows: '1664', example_0_trip: 'T1', example_0_line: '2',
        example_0_service_id: 'WEEKDAY', example_0_observed: '-999', example_0_expected: '0-3',
        example_0_stop_id: 'S1', example_0_stop_sequence: '1', example_0_arrival: '05:35:00', example_0_departure: '05:35:00',
        example_0_next_stop_id: 'S2', example_0_next_stop_sequence: '2', example_0_next_arrival: '05:42:00', example_0_next_departure: '05:42:00',
      },
    });
    const el = root();
    renderProducer(el, result([summary], [], [['STM_019', 2, 1664]], true, ni));
    expect(el.innerHTML).toContain('<strong>1.664</strong><span>durak saati</span>');
    expect(el.innerHTML).toContain('<th>Çalışma günleri</th>');
    expect(el.innerHTML).toContain('<td>Hafta içi</td>');
    expect(el.innerHTML).toContain('<td>Hospital → Roosevelt</td><td>-999</td>');
    expect(el.innerHTML).toContain('Hospital ile Roosevelt arasındaki hat bölümünde yolcu indirme ayarı “-999” olarak yazılmış.');
  });

  it('STM_020 sıfır süreli bölümün saatini, mesafesini, takvimini ve gerçek güzergâhını gösterir', () => {
    const ni: NameIndex = {
      ...emptyNameIndex,
      stops: { S1: 'Onkoloji', S2: 'Binevler', S3: 'Otogar' },
      routes: { R1: 'S08' }, trips: { T1: 'Otogar' }, trip_routes: { T1: 'R1' },
      trip_directions: { T1: '0' }, trip_first_dep: { T1: '08:00' },
      trip_shapes: { T1: 'SH1' }, route_shapes: { R1: ['SH1'] }, shape_routes: { SH1: [['R1', '0']] },
      shape_coords: { SH1: [[37.00, 37.30], [37.005, 37.305], [37.01, 37.31], [37.02, 37.32]] },
      trip_stops: { T1: ['S1', 'S2', 'S3'] }, trip_stop_sequences: { T1: [1, 2, 3] },
      stop_coords: { S1: [37.00, 37.30], S2: [37.01, 37.31], S3: [37.02, 37.32] },
      service_days: { HI: ['1111100', '20260101', '20261231', '0'] },
    };
    const n = notice('a', 'STM_020', 'HIGH', {
      entity_type: 'Trip', entity_id: 'T1', service_id: 'HI',
      observed_value: 'duration=0s; distance=640m', expected_value: '> 0s',
      details: {
        stop_a: 'S1', stop_b: 'S2', stop_a_sequence: '1', stop_b_sequence: '2',
        section_start_time: '08:00:30', section_end_time: '08:00:30', distance_meters: '640',
        intermediate_stop_count: '0',
        stop_a_arrival: '08:00:30', stop_a_departure: '08:00:30',
        stop_b_arrival: '08:00:30', stop_b_departure: '08:00:30',
      },
    });
    const r = result([n], [], [['STM_020', 3, 1]], true, ni);
    const el = root();
    renderProducer(el, r);
    expect(el.innerHTML).toContain('Uzak iki durak arasında süre sıfır');
    expect(el.innerHTML).toContain('S08 hattı · Otogar yönü · sefer başlangıcı 08:00');
    expect(el.innerHTML).toContain('Onkoloji durağındaki 08:00:30 ile Binevler durağındaki 08:00:30 aynı.');
    expect(el.innerHTML).toContain('yaklaşık 640 metre');
    expect(el.innerHTML).toContain('Saatler doğruysa iki durağın konumlarını kontrol edin.');
    expect(el.innerHTML).toContain('<th>Çalışma günleri</th>');
    expect(el.innerHTML).toContain('<td>Hafta içi</td>');
    expect(el.innerHTML).toContain('<td>Onkoloji → Binevler · 640 m</td>');
    expect(el.innerHTML).toContain('<td>Onkoloji</td><td>1</td><td>08:00:30</td><td>08:00:30</td><td>—</td>');
    expect(el.innerHTML).toContain('<tr class="bad"><td>Binevler</td><td>2</td><td>08:00:30</td><td>08:00:30</td><td>640</td>');
    const map = plannerMap(n, r);
    expect(map.opts.extraPolylines?.[0].coords).toEqual(ni.shape_coords.SH1);
    expect(map.opts.extraPolylines?.[1].coords).toEqual(ni.shape_coords.SH1.slice(0, 3));
    expect(map.opts.extraPolylines?.[1].zoomTo).toBe(true);
    expect(map.opts.pins).toHaveLength(3);
    expect(map.opts.pins.filter(pin => pin.primary)).toHaveLength(2);
    expect(map.opts.focus).toBeUndefined();
  });

  it('tek duraklı STM_015 seferinde aynı hat ve yönün gerçek güzergâhını ve durağı gösterir', () => {
    const ni: NameIndex = {
      ...emptyNameIndex,
      stops: { S1: 'Onkoloji' }, routes: { R1: 'S08' },
      trip_routes: { T1: 'R1' }, trip_directions: { T1: '0' },
      trip_stops: { T1: ['S1'] }, trip_stop_sequences: { T1: [1] },
      stop_coords: { S1: [37.01, 37.31] },
      route_shapes: { R1: ['SH1'] }, shape_routes: { SH1: [['R1', '0']] },
      shape_coords: { SH1: [[37.00, 37.30], [37.01, 37.31], [37.02, 37.32]] },
    };
    const n = notice('a', 'STM_015', 'CRITICAL', {
      entity_type: 'Trip', entity_id: 'T1',
      details: { stop_id: 'S1', stop_sequence: '1', stop_count: '1', timed_stops: '0' },
    });
    const map = plannerMap(n, result([n], ['a'], [['STM_015', 4, 1]], true, ni));
    expect(map.opts.extraPolylines?.[0].coords).toEqual(ni.shape_coords.SH1);
    expect(map.opts.pins).toHaveLength(1);
    expect(map.opts.pins[0].primary).toBe(true);
    expect(map.opts.pins[0].label).toContain('Onkoloji');
    expect(map.opts.focus).toEqual(ni.stop_coords.S1);
  });

  it('sol menü konu değil kayıt sayısını gösterir', () => {
    const r = result([notice('a', 'DQ_016', 'MEDIUM')], [], [['DQ_016', 2, 37]]);
    const el = root();
    renderProducer(el, r);
    expect(el.innerHTML).toMatch(/data-area-filter="all"[^>]*><span>Tüm konular<\/span><span class="planner-area-count">37<\/span>/);
  });
});

describe('Saatsiz sefer', () => {
  it('seferin hiçbir durağında saat yoksa bunu söyler, tek uç durak demez', () => {
    const r = result([
      notice('a', 'STM_015', 'CRITICAL', { entity_type: 'Trip', entity_id: 'T1', details: { stop_id: 'S1', stop_count: '85', timed_stops: '0' } }),
    ], ['a'], [['STM_015', 4, 1]]);
    const el = { innerHTML: '', querySelectorAll: () => [], querySelector: () => null } as unknown as HTMLElement;
    renderProducer(el, r);
    expect(el.innerHTML).toContain('Bu seferin 85 durağının hiçbirinde varış ya da kalkış saati yok');
    expect(el.innerHTML).toContain('<td>Seferde hiç saat yok (85 durak)</td>');
  });

  it('tek duraklı saatsiz seferde durak sayısı yerine durak adını yazar', () => {
    const n = notice('a', 'STM_015', 'CRITICAL', {
      entity_type: 'Trip', entity_id: 'T1',
      details: { stop_id: 'S1', stop_count: '1', timed_stops: '0' },
    });
    const r = result([n], ['a'], [['STM_015', 4, 1]], true, {
      ...emptyNameIndex, stops: { S1: 'Onkoloji' }, trip_stops: { T1: ['S1'] },
    });
    const el = { innerHTML: '', querySelectorAll: () => [], querySelector: () => null } as unknown as HTMLElement;
    renderProducer(el, r);
    expect(el.innerHTML).toContain('Bu seferin Onkoloji durağının varış ya da kalkış saati yok');
    expect(el.innerHTML).toContain('<td>Onkoloji durağında saat yok</td>');
    expect(el.innerHTML).not.toContain('Bu seferin 1 durağının');
  });

  it('tek duraklı STM_016 kartında da durak adını yazar', () => {
    const n = notice('a', 'STM_016', 'CRITICAL', {
      entity_type: 'Trip', entity_id: 'T1',
      details: { stop_id: 'S1', stop_sequence: '1', stop_count: '1', timed_stops: '0' },
    });
    const r = result([n], ['a'], [['STM_016', 4, 1]], true, {
      ...emptyNameIndex, stops: { S1: 'Onkoloji' }, trip_stops: { T1: ['S1'] },
    });
    const el = { innerHTML: '', querySelectorAll: () => [], querySelector: () => null } as unknown as HTMLElement;
    renderProducer(el, r);
    expect(el.innerHTML).toContain('Bu seferin Onkoloji durağının varış ya da kalkış saati yok');
    expect(el.innerHTML).toContain('<td>Onkoloji durağında saat yok</td>');
  });
});

describe('STM_017 planlamacı kanıtı', () => {
  const nameIndex: NameIndex = {
    ...emptyNameIndex,
    stops: { S1: 'Onkoloji', S2: 'Binevler', S3: 'Otogar' },
    routes: { R1: 'S08' }, trips: { T1: 'Otogar', T2: 'Onkoloji' },
    trip_routes: { T1: 'R1', T2: 'R1' }, trip_directions: { T1: '0', T2: '1' },
    trip_first_dep: { T1: '06:15', T2: '07:15' }, trip_shapes: { T1: 'SH1', T2: 'SH1' },
    trip_stops: { T1: ['S1', 'S2', 'S3'], T2: ['S3', 'S2', 'S1'] },
    trip_stop_sequences: { T1: [1, 2, 3], T2: [1, 2, 3] },
    stop_coords: { S1: [37.01, 37.31], S2: [37.02, 37.32], S3: [37.03, 37.33] },
    shape_coords: { SH1: [[37.01, 37.31], [37.02, 37.32], [37.03, 37.33]] },
    service_days: { WKD: ['1111100', '20260101', '20261231', '0'] },
  };

  it('sefer bulgusunda eksik durakları tablo ve haritada kırmızı gösterir', () => {
    const n = notice('a', 'STM_017', 'MEDIUM', {
      entity_type: 'Trip', entity_id: 'T1', service_id: 'WKD',
      details: {
        route_id: 'R1', shape_id: 'SH1', stop_count: '3', affected_stop_count: '2',
        missing_0_stop_id: 'S1', missing_0_stop_sequence: '1', missing_0_arrival: '06:15:00', missing_0_departure: '06:15:00',
        missing_1_stop_id: 'S3', missing_1_stop_sequence: '3', missing_1_arrival: '06:22:00', missing_1_departure: '06:22:00',
      },
    });
    const r = result([n], [], [['STM_017', 2, 1]], true, nameIndex);
    const el = { innerHTML: '', querySelectorAll: () => [], querySelector: () => null } as unknown as HTMLElement;
    renderProducer(el, r);
    expect(el.innerHTML).toContain('Bu seferin 3 durağından 2 tanesinde');
    expect(el.innerHTML).toContain('<td>Onkoloji</td><td>1</td><td>06:15:00</td><td>06:15:00</td><td>eksik</td>');
    expect(el.innerHTML).toContain('Onkoloji ile birlikte 2 durakta kümülatif mesafe eksik');
    expect(el.innerHTML).not.toContain('Güzergâh üzerindeki mesafe');
    expect(el.innerHTML).toContain('<th>Kümülatif mesafe</th>');
    // 2 of 3 stops missing: the trip still has a distance to keep or remove.
    expect(el.innerHTML).toContain('Kullanmıyorsanız bu seferdeki diğer mesafe değerlerini de kaldırın');
    expect(el.innerHTML).not.toContain('Bu seferin hiçbir durağında kümülatif mesafe yok');
    expect(el.innerHTML).toContain('<th>Çalışma günleri</th>');
    expect(el.innerHTML).toContain('<td>Hafta içi</td>');
    const map = plannerMap(n, r);
    expect(map.opts.extraPolylines?.[0].coords).toEqual(nameIndex.shape_coords.SH1);
    expect(map.opts.pins).toHaveLength(3);
    expect(map.opts.pins.filter(pin => pin.primary).map(pin => pin.label)).toEqual(expect.arrayContaining([
      expect.stringContaining('Onkoloji'), expect.stringContaining('Otogar'),
    ]));
    expect(map.opts.focus).toEqual(nameIndex.stop_coords.S1);
  });

  it('feed özetini gerçek sefer örneklerine açıp etkilenen sefer sayısını gösterir', () => {
    const n = notice('a', 'STM_017', 'MEDIUM', {
      entity_type: 'Feed', entity_id: null, observed_value: '3',
      details: {
        summary_mode: 'feed', affected_shape_count: '1', affected_trip_count: '3',
        example_0_trip: 'T1', example_0_service_id: 'WKD', example_0_stop_count: '3', example_0_affected_stop_count: '3',
        example_0_missing_0_stop_id: 'S1', example_0_missing_0_stop_sequence: '1',
        example_1_trip: 'T2', example_1_service_id: 'WKD', example_1_stop_count: '3', example_1_affected_stop_count: '3',
        example_1_missing_0_stop_id: 'S3', example_1_missing_0_stop_sequence: '1',
      },
    });
    const r = result([n], [], [['STM_017', 2, 1]], true, nameIndex);
    const el = { innerHTML: '', querySelectorAll: () => [], querySelector: () => null } as unknown as HTMLElement;
    renderProducer(el, r);
    expect(el.innerHTML).toContain('1 güzergâha ait 3 seferde');
    expect(el.innerHTML).toContain('<strong>3</strong><span>sefer</span>');
    expect(el.innerHTML).toContain('<td>Otogar</td><td>06:15</td>');
    expect(el.innerHTML).toContain('<td>Onkoloji</td><td>07:15</td>');
    expect(el.innerHTML).not.toContain('<strong>1</strong><span>kayıt</span>');
  });
});

describe('Hat, durak, güzergâh ve takvim seçmeli süzgeci', () => {
  it('her boyut için seçmeli liste kurar ve seçenek adlarını planlamacının tanıdığı biçimde yazar', () => {
    const ni: NameIndex = {
      ...emptyNameIndex, routes: { R1: 'K701' }, route_long_names: { R1: 'İstasyon - Hisar Köy' },
      stops: { A: 'Çarşı', B: 'Liman' }, stop_codes: { A: '1204' }, trip_routes: { T1: 'R1' }, trip_shapes: { T1: 'SH1' },
      shape_routes: { SH1: [['R1', '0']] }, shape_trips: { SH1: 'T1' }, trips: { T1: 'Hisar Köy' },
      service_days: { HI: ['1111100', '20260101', '20261231', '0'] },
    };
    const r = result([
      notice('a', 'STM_012', 'HIGH', { entity_type: 'Trip', entity_id: 'T1', service_id: 'HI', observed_value: '987 km/h', details: { stop_a: 'A', stop_b: 'B', speed_kmh: '987' } }),
    ], [], [['STM_012', 4, 1]], true, ni);
    const el = { innerHTML: '', querySelectorAll: () => [], querySelector: () => null } as unknown as HTMLElement;
    renderProducer(el, r);
    for (const dim of ['route', 'stop', 'shape', 'service']) expect(el.innerHTML).toContain(`data-dim="${dim}"`);
    expect(el.innerHTML).toContain('<td>Çarşı → Liman · 987 km/h</td>');
  });
});

describe('İşletmeci süzgeci', () => {
  it('iki ve daha fazla işletmecide süzgeci en üstte gösterir', () => {
    const r = result([notice('a', 'DQ_016', 'MEDIUM'), notice('b', 'DQ_016', 'MEDIUM')], [], [['DQ_016', 2, 2]]);
    r.agency_breakdown = { agencies: [
      { agency_id: 'A1', agency_name: 'Gaziulaş', notice_indices: [0] },
      { agency_id: 'A2', agency_name: 'Belediye Otobüsleri', notice_indices: [1] },
    ] } as unknown as ValidationResult['agency_breakdown'];
    const el = { innerHTML: '', querySelectorAll: () => [], querySelector: () => null } as unknown as HTMLElement;
    renderProducer(el, r);
    const html = el.innerHTML;
    expect(html.indexOf('agency-filter')).toBeGreaterThan(-1);
    expect(html.indexOf('agency-filter')).toBeLessThan(html.indexOf('planner-top'));
    expect(html).toContain('Gaziulaş');
    expect(html).toContain('Belediye Otobüsleri');
  });
});

describe('0,0 konumlu durak', () => {
  it('STM_012 hızını saat değil durak konumu sorunu olarak anlatır', () => {
    const ni: NameIndex = { ...emptyNameIndex, stops: { A: 'Osnabrück Altstadt', B: 'Hamburg-Harburg' }, stop_coords: { A: [52.28, 8.04], B: [0, 0] } };
    const r = result([notice('a', 'STM_012', 'HIGH', { entity_type: 'Trip', entity_id: 'T1', observed_value: '9999 km/h', details: { stop_a: 'A', stop_b: 'B' } })], [], [['STM_012', 4, 1]], true, ni);
    const el = { innerHTML: '', querySelectorAll: () => [], querySelector: () => null } as unknown as HTMLElement;
    renderProducer(el, r);
    expect(el.innerHTML).toContain('Hamburg-Harburg (B) durağının konumu 0,0 girilmiş');
  });
});

describe('Tasarım ayrıntıları', () => {
  it('sayıyı birimiyle, hat sütununu renkli kutucukla ve Beta etiketini gösterir', () => {
    const ni: NameIndex = { ...emptyNameIndex, routes: { R1: '34' }, trip_routes: { T1: 'R1' }, route_colors: { R1: ['B5121B', 'FFFFFF'] } };
    const r = result([notice('a', 'STM_015', 'CRITICAL', { entity_type: 'Trip', entity_id: 'T1' })], ['a'], [['STM_015', 4, 1]], true, ni);
    const el = { innerHTML: '', querySelectorAll: () => [], querySelector: () => null } as unknown as HTMLElement;
    renderProducer(el, r);
    expect(el.innerHTML).toContain('<strong>1</strong><span>sefer</span>');
    expect(el.innerHTML).toContain('<td><span class="planner-route-badge" style="background:#B5121B;border-color:#B5121B;color:#FFFFFF">34</span></td>');
    expect(el.innerHTML).toContain('>BETA</span>');
  });
});

describe('Genel kalıpta görülen değer', () => {
  const render = (notices: Notice[], rule: string) => {
    const r = result(notices, [], [[rule, 4, notices.length]]);
    const el = { innerHTML: '', querySelectorAll: () => [], querySelector: () => null } as unknown as HTMLElement;
    renderProducer(el, r);
    return el.innerHTML;
  };

  it('her satıra kendi görülen değerini ve dilden bağımsız beklenen değeri yazar', () => {
    const html = render([
      notice('a', 'STM_048', 'HIGH', { entity_type: 'Trip', entity_id: 'T1', field: 'arrival_time', observed_value: '00:10:00', expected_value: '24:10:00' }),
      notice('b', 'STM_048', 'HIGH', { entity_type: 'Trip', entity_id: 'T2', field: 'arrival_time', observed_value: '00:25:00', expected_value: '24:25:00' }),
    ], 'STM_048');
    expect(html).toContain('<th>Görülen değer</th>');
    expect(html).toContain('<td>00:10:00 <span class="planner-expected">beklenen 24:10:00</span></td>');
    expect(html).toContain('<td>00:25:00 <span class="planner-expected">beklenen 24:25:00</span></td>');
    expect(html).toContain('<code>00:10:00 · beklenen 24:10:00</code>');
  });

  it('feed metin sütunundaki veriyi gösterir, motorun dil içeren özetini ve beklenen metnini göstermez', () => {
    const name = render([notice('a', 'DQ_019', 'LOW', { entity_type: 'Trip', entity_id: 'T1', field: 'route_long_name', observed_value: 'test-test', expected_value: 'inceleyin' })], 'DQ_019');
    expect(name).toContain('<td>test-test</td>');
    expect(name).not.toContain('beklenen inceleyin');
    const engine = render([notice('a', 'DQ_016', 'LOW', { entity_type: 'Trip', entity_id: 'T1', field: 'stop_name', observed_value: '7 rows' })], 'DQ_016');
    expect(engine).not.toContain('<th>Görülen değer</th>');
    expect(engine).not.toContain('<td>7 rows');
    const glued = render([notice('a', 'VAT_003', 'LOW', { entity_type: 'Trip', entity_id: 'T1', field: 'trip_id', observed_value: '1469min (hour-based expected 29min)' })], 'VAT_003');
    expect(glued).not.toContain('<td>1469min');
    const ids = render([notice('a', 'STM_021', 'LOW', { entity_type: 'Trip', entity_id: 'T1', field: 'stop_id', observed_value: '43GX03 → 44GX02' })], 'STM_021');
    // STM_021 has its own card with a stop table, so the rows carry no separate observed/expected
    // column: neither the stop-code pair nor the inline "expected > 0 m" is shown there.
    expect(ids).not.toContain('<td>43GX03 → 44GX02</td>');
    expect(ids).not.toContain('planner-expected');
    expect(engine).toContain('class="producer-card-facts" hidden');
  });

  it('sayı ve noktalama içeren uzun motor özetinde geri izleme yapmaz', () => {
    const summary = render([notice('a', 'DQ_016', 'LOW', {
      entity_type: 'Stop', entity_id: 'S1', field: 'stop_lat|stop_lon',
      observed_value: "(52.603570904925725, 1.2203111959399426) == '29002900H555'",
    })], 'DQ_016');
    expect(summary).not.toContain('<th>Görülen değer</th>');
  });

  it('kendi kanıtını yazan kurala sütun eklemez', () => {
    const html = render([notice('a', 'STM_015', 'CRITICAL', { entity_type: 'Trip', entity_id: 'T1', field: 'arrival_time', expected_value: 'HH:MM:SS' })], 'STM_015');
    expect(html).not.toContain('<th>Görülen değer</th>');
  });
});

describe('Hatalı yer sütunu', () => {
  const ni: NameIndex = {
    ...emptyNameIndex,
    stops: { S1: 'Şehreküstü İtfaiyesi', S2: 'Çarşı', S3: 'Liman', S4: 'Otogar', S5: 'Garaj' },
    routes: { R1: 'S08' }, trip_routes: { T1: 'R1' },
  };
  const render = (n: Notice) => {
    const r = result([n], [], [[n.rule_id, 4, 1]], true, ni);
    const el = { innerHTML: '', querySelectorAll: () => [], querySelector: () => null } as unknown as HTMLElement;
    renderProducer(el, r);
    return el.innerHTML;
  };

  it('STM_048 saatin geriye döndüğü durağı ve önceki saati, STM_049 aynı duraktaki varış ve kalkışı yazar', () => {
    const wrap = render(notice('a', 'STM_048', 'HIGH', {
      entity_type: 'Trip', entity_id: 'T1', field: 'arrival_time', observed_value: '00:00:43', expected_value: '24:00:43',
      details: { stop_id: 'S1', previous_time: '23:58:51', observed_time: '00:00:43', expected_time: '24:00:43' },
    }));
    expect(wrap).toContain('<th>Hatalı yer</th>');
    expect(wrap).toContain('<td>Şehreküstü İtfaiyesi · önceki durak 23:58:51</td>');
    expect(wrap).toContain('Gece yarısından sonraki durak geçiş saatleri 00:xx yazılmış');
    const dwell = render(notice('a', 'STM_049', 'HIGH', {
      entity_type: 'Trip', entity_id: 'T1', field: 'departure_time', observed_value: '00:00:05', expected_value: '24:00:05',
      details: { stop_id: 'S1', previous_time: '23:59:45', observed_time: '00:00:05', expected_time: '24:00:05' },
    }));
    expect(dwell).toContain('<td>Şehreküstü İtfaiyesi · varış 23:59:45, kalkış 00:00:05</td>');
  });

  it('OPR_008 ilk hızlı bölümü ve kalan bölüm sayısını, OPR_007 tekrar eden durağı yazar', () => {
    const fast = render(notice('a', 'OPR_008', 'MEDIUM', {
      entity_type: 'Trip', entity_id: 'T1', observed_value: '318.8 km/h',
      details: { bad_seg_0_a: 'S2', bad_seg_0_b: 'S3', bad_seg_1_a: 'S4', bad_seg_1_b: 'S5', departure: '08:00' },
    }));
    expect(fast).toContain('<td>Çarşı → Liman (+1 bölüm daha)</td>');
    const repeat = render(notice('a', 'OPR_007', 'LOW', {
      entity_type: 'Trip', entity_id: 'T1', observed_value: '2× repeat', details: { dup_stop: 'S4', stops: 'S2,S4,S3,S4' },
    }));
    expect(repeat).toContain('<td>Otogar (2 kez)</td>');
  });

  it('STM_029 seferin uç duraklarını saatleriyle, STM_045 pencereyi aşan kalkışın durağını yazar', () => {
    const short = render(notice('a', 'STM_029', 'MEDIUM', {
      entity_type: 'Trip', entity_id: 'T1', observed_value: '30s', expected_value: '≥ 60s',
      details: { first_stop_id: 'S2', first_departure: '08:00:00', last_stop_id: 'S3', last_arrival: '08:00:30' },
    }));
    expect(short).toContain('<td>Çarşı 08:00:00 → Liman 08:00:30</td>');
    const late = render(notice('a', 'STM_045', 'MEDIUM', {
      entity_type: 'Trip', entity_id: 'T1', observed_value: '28:00:00', expected_value: '≤27:00:00', details: { stop_id: 'S4' },
    }));
    expect(late).toContain('<td>Otogar · kalkış 28:00:00</td>');
  });

  it('STM_003 bozuk varış saatini durak adıyla yazar', () => {
    const html = render(notice('a', 'STM_003', 'CRITICAL', {
      entity_type: 'Trip', entity_id: 'T1', field: 'arrival_time', observed_value: '-15:46:01', expected_value: 'HH:MM:SS',
      details: {
        stop_id: 'S2', arrival: '?', departure: '?',
        prev_stop_id: 'S1', prev_arrival: '09:44:16', prev_departure: '09:44:16',
        next_stop_id: 'S3', next_arrival: '?', next_departure: '?',
      },
    }));
    expect(html).toContain('<td>Çarşı · varış -15:46:01</td>');
  });

  it('varışı da bozuk satırların STM_004 tekrarını STM_003 kartına katar, birimi durak saati yazar', () => {
    const bad = (id: string, rule: string, line: number, extra: Partial<Notice> = {}) =>
      notice(id, rule, 'CRITICAL', { entity_type: 'Trip', entity_id: 'T1', file: 'stop_times.txt', line, observed_value: '-15:46:01', ...extra });
    const r = result([
      bad('a', 'STM_003', 10), bad('b', 'STM_003', 11),
      bad('c', 'STM_004', 10), bad('d', 'STM_004', 11), bad('e', 'STM_004', 12),
    ], ['a', 'b', 'c', 'd', 'e'], [['STM_003', 4, 2], ['STM_004', 4, 3]], true, ni);
    const el = { innerHTML: '', querySelectorAll: () => [], querySelector: () => null } as unknown as HTMLElement;
    renderProducer(el, r);
    expect(el.innerHTML).toContain('data-rule="STM_003"');
    expect(el.innerHTML).toContain('<strong>2</strong><span>durak saati</span>');
    // For STM_004, only the row with a valid arrival (12) remains.
    expect(el.innerHTML).toContain('data-rule="STM_004"');
    expect(el.innerHTML).toContain('<strong>1</strong><span>sefer</span>');
  });

  it('teknik ayrıntıda kayıt kodunu türüyle adlandırır, motorun iç anahtarlarını dökmez', () => {
    const html = render(notice('a', 'STM_003', 'CRITICAL', {
      entity_type: 'Trip', entity_id: '203536-0', file: 'stop_times.txt', line: 1232681, field: 'arrival_time',
      observed_value: '-15:46:01', expected_value: 'HH:MM:SS', details: { stop_id: 'S2', prev_arrival: '09:44:16' },
    }));
    expect(html).toContain('<span>Sefer kodu</span><code>203536-0</code>');
    expect(html).toContain('<span>Durak kodu</span><code>S2</code>');
    expect(html).not.toContain('<span>prev_arrival</span>');
    expect(html).not.toContain('Varlık');
  });

  it('STM_007 özetini örnek satırlara açar, deseni ve birimi doğru yazar', () => {
    const r = result([notice('a', 'STM_007', 'HIGH', {
      file: 'stop_times.txt', field: 'departure_time', observed_value: '60010',
      details: {
        affected_rows: '60010', affected_trips: '11586', example_trips: 'T1',
        typical_gap_seconds: '19', typical_gap_rows: '30997', top_stops: 'S2,S3',
        example_0_trip: 'T1', example_0_line: '50', example_0_stop: 'S2', example_0_arrival: '06:50:45', example_0_departure: '06:50:30',
        example_0_prev_stop: 'S1', example_0_prev_arrival: '06:50:20', example_0_prev_departure: '06:50:20',
      },
    })], [], [['STM_007', 4, 60010]], true, ni);
    const el = { innerHTML: '', querySelectorAll: () => [], querySelector: () => null } as unknown as HTMLElement;
    renderProducer(el, r);
    expect(el.innerHTML).toContain('<strong>60.010</strong><span>durak saati</span>');
    expect(el.innerHTML).toContain('<td>Çarşı · varış 06:50:45, kalkış 06:50:30</td>');
    expect(el.innerHTML).toContain('60.010 durak saatinde (11.586 sefer) kalkış varıştan önce.');
    expect(el.innerHTML).toContain('Bunların %52 kadarında fark aynı: kalkış varıştan 19 sn önce.');
    expect(el.innerHTML).toContain('saatleri üreten ayardan gelebilir');
    expect(el.innerHTML).toContain('En çok etkilenen duraklar: Çarşı, Liman.');
  });

  it('teknik ayrıntıda motorun İngilizce açıklamasını göstermez, STM_008 sayacını adıyla yazar', () => {
    const html = render(notice('a', 'STM_008', 'CRITICAL', {
      entity_type: 'Trip', entity_id: '450055-0', file: 'stop_times.txt', line: 226349, field: 'arrival_time',
      observed_value: '2', expected_value: 'monotonic trip chronology', details: { affected_segments: '2', stop_a: 'S1', stop_b: 'S2' },
    }));
    expect(html).toContain('<span>Geriye giden bölüm sayısı</span><code>2</code>');
    expect(html).not.toContain('monotonic trip chronology');
    const mask = render(notice('a', 'STM_003', 'CRITICAL', {
      entity_type: 'Trip', entity_id: 'T1', field: 'arrival_time', observed_value: '-15:46:01', expected_value: 'HH:MM:SS', details: { stop_id: 'S2' },
    }));
    expect(mask).toContain('<span>Beklenen değer</span><code>HH:MM:SS</code>');
  });

  it('hat düzeyindeki hız bulgusunda durak çiftini hat sütunlarının yanına yazar', () => {
    const html = render(notice('a', 'STM_014', 'MEDIUM', {
      entity_type: 'Route', entity_id: 'R1', observed_value: '468.0 km/h (S2 → S3)', expected_value: '≤ 120 km/h',
      details: { route_id: 'R1', stop_a: 'S2', stop_b: 'S3', speed_max: '468.0' },
    }));
    expect(html).toContain('<td>Çarşı → Liman</td>');
  });

  it('STM_014 tablosu hat, yön, sefer başlangıcı, hatalı yer ve çalışma günlerini yazar', () => {
    const local: NameIndex = {
      ...ni, routes: { R1: '34' }, trips: { T1: 'Otogar' }, trip_first_dep: { T1: '06:00' },
      service_days: { SVC: ['1111100', '20260101', '20261231', '0'] },
    };
    const r = result([notice('a', 'STM_014', 'HIGH', {
      entity_type: 'Route', entity_id: 'R1:0:S2→S3',
      details: {
        route_id: 'R1', stop_a: 'S2', stop_b: 'S3', prev_stop_id: 'S1', next_stop_id: 'S4',
        direction_from_stop_id: 'S1', direction_to_stop_id: 'S4',
        trips: 'T1', sample_departure: '06:00:00', sample_service_id: 'SVC',
        speed_min: '468.0', speed_max: '468.0', distance_min_meters: '1200', distance_max_meters: '1200', threshold_kmh: '120',
      },
    })], [], [['STM_014', 4, 1]], true, local);
    const el = { innerHTML: '', querySelectorAll: () => [], querySelector: () => null } as unknown as HTMLElement;
    renderProducer(el, r);
    expect(el.innerHTML).toContain('<span class="planner-route-badge">34</span>');
    expect(el.innerHTML).toContain('<th>Hat</th><th>Yön</th><th>Sefer başlangıç saati</th><th>Hatalı yer</th><th>Çalışma günleri</th>');
    expect(el.innerHTML).toContain('<td>Otogar</td><td>06:00</td>');
    expect(el.innerHTML).toContain('<td>Hafta içi</td>');
    expect(el.innerHTML).not.toContain('<th>Hat kodu</th>');
    expect(el.innerHTML).not.toContain('<td>R1</td>');
    expect(el.innerHTML).toContain('Çarşı → Liman · 1.200 m · 468 km/h');
    expect(el.innerHTML).toContain('bu hat türü için belirlenen 120 km/h sınırını aşıyor');
  });

  it('STM_014 haritasında örnek gerçek seferin güzergâhını ve bütün duraklarını gösterir', () => {
    const local: NameIndex = {
      ...emptyNameIndex,
      stops: { S1: 'Başlangıç', S2: 'Çarşı', S3: 'Liman', S4: 'Otogar' },
      routes: { R1: '34' }, trip_routes: { T1: 'R1' }, trip_shapes: { T1: 'SH1' },
      trip_stops: { T1: ['S1', 'S2', 'S3', 'S4'] }, trip_stop_sequences: { T1: [1, 2, 3, 4] },
      stop_coords: { S1: [41.00, 29.00], S2: [41.01, 29.01], S3: [41.02, 29.02], S4: [41.03, 29.03] },
      shape_coords: { SH1: [[41.00, 29.00], [41.015, 29.015], [41.03, 29.03]] },
    };
    const n = notice('a', 'STM_014', 'HIGH', {
      entity_type: 'Route', entity_id: 'R1:0:S2→S3',
      details: { route_id: 'R1', trips: 'T1', stop_a: 'S2', stop_b: 'S3', prev_stop_id: 'S1', next_stop_id: 'S4' },
    });
    const map = plannerMap(n, result([n], [], [['STM_014', 4, 1]], true, local));
    expect(map.opts.extraPolylines?.[0].coords).toEqual(local.shape_coords.SH1);
    expect(map.opts.pins).toHaveLength(4);
    expect(map.opts.pins.filter(pin => pin.primary).map(pin => pin.label)).toEqual(expect.arrayContaining([
      expect.stringContaining('Çarşı'), expect.stringContaining('Liman'),
    ]));
    expect(map.opts.focus).toEqual(local.stop_coords.S2);
  });

  it('aynı adlı iki durağı durak koduyla ayırır', () => {
    const local: NameIndex = { ...ni, stops: { ...ni.stops, K1: 'Kubilay Cad.', K2: 'Kubilay Cad.' }, stop_codes: { K1: '71508' } };
    const r = result([notice('a', 'STM_025', 'LOW', { entity_type: 'Trip', entity_id: 'T1', observed_value: '6s', details: { stop_a: 'K1', stop_b: 'K2' } })], [], [['STM_025', 4, 1]], true, local);
    const el = { innerHTML: '', querySelectorAll: () => [], querySelector: () => null } as unknown as HTMLElement;
    renderProducer(el, r);
    expect(el.innerHTML).toContain('<td>Kubilay Cad. (71508) → Kubilay Cad. (K2)</td>');
  });
});

describe('Feed düzeyi özetin örnek kayıtları', () => {
  const ni: NameIndex = { ...emptyNameIndex, stops: { P1: 'Çarşı', P2: 'Liman' }, stop_codes: { P1: '1204' } };
  const render = (notices: Notice[], rule: string) => {
    const r = result(notices, [], [[rule, 4, 1]], true, ni);
    const el = { innerHTML: '', querySelectorAll: () => [], querySelector: () => null } as unknown as HTMLElement;
    renderProducer(el, r);
    return el.innerHTML;
  };

  it('örnek durakları satır satır açar, toplamı motorun sayısından alır, özetin sayısını satıra yazmaz', () => {
    const html = render([notice('a', 'STP_022', 'LOW', {
      observed_value: '3537', details: { affected_stops: '3537', example_stops: 'P1, P2' },
    })], 'STP_022');
    expect(html).toContain('<td>Çarşı</td><td>P1</td>');
    expect(html).toContain('<td>Liman</td><td>P2</td>');
    expect(html).toContain('<strong>3.537</strong><span>durak</span>');
    expect(html).toContain('Kalan 3.535 kayıt listede yok');
    expect(html).not.toContain('<th>Görülen değer</th>');
  });

  it('örnek güzergâhları hat rozeti ve güzergâh koduyla açar', () => {
    const local: NameIndex = { ...ni, routes: { R1: '34' }, shape_routes: { SH1: [['R1', '0']] } };
    const r = result([notice('a', 'SHP_009', 'LOW', { observed_value: '2', details: { affected_shapes: '2', example_shapes: 'SH1, SH2' } })], [], [['SHP_009', 4, 1]], true, local);
    const el = { innerHTML: '', querySelectorAll: () => [], querySelector: () => null } as unknown as HTMLElement;
    renderProducer(el, r);
    expect(el.innerHTML).toContain('<th>Hat</th><th>Güzergâh kodu</th>');
    expect(el.innerHTML).toContain('>34</span></td><td>SH1</td>');
    expect(el.innerHTML).toContain('<td>—</td><td>SH2</td>');
  });

  it('takvim dönemi bildiriminde dönem satırını korur ve etkilenen takvim kodlarını yazar', () => {
    const html = render([notice('a', 'CAL_007', 'LOW', {
      entity_id: '20260812-20261002', observed_value: '50', expected_value: '< 14', details: { services: '287.328.1,287.65.1' },
    })], 'CAL_007');
    expect(html).toContain('<th>Takvim kodu</th>');
    expect(html).toContain('<td>287.328.1, 287.65.1</td>');
  });
});

describe('plannerFacets (bağlı filtreler)', () => {
  const k = (route: string[], stop: string[]) => ({ route, stop, shape: [], service: [] });
  const cards: FacetCard[] = [
    { area: 'stops', sev: 'LOW', cls: 'ANALYTICS', rule: 'STM_021', file: 'stop_times.txt', count: 2, keys: [k(['R1'], ['S1']), k(['R2'], ['S2'])] },
    { area: 'times', sev: 'MEDIUM', cls: 'SPEC', rule: 'STM_022', file: 'stop_times.txt', count: 1, keys: [k(['R1'], ['S3'])] },
    { area: 'agency', sev: 'HIGH', cls: 'SPEC', rule: 'AGN_003', file: 'agency.txt', count: 1, keys: [k([], [])] },
  ];
  const none = (): FacetFilter => ({ area: 'all', sev: new Set(), cls: new Set(), rule: '', file: '', entity: new Map() });

  it('filtre yokken her seçenek bütün kayıtlarla sayılır', () => {
    const f = plannerFacets(cards, none());
    expect(f.visible).toEqual([true, true, true]);
    expect([...f.rules]).toEqual([['STM_021', 2], ['STM_022', 1], ['AGN_003', 1]]);
    expect(f.entity.route.get('R1')).toBe(2);
    expect(f.entity.stop.get('S2')).toBe(1);
  });

  it('hat seçilince durak, kural ve dosya listeleri o hatta daralır', () => {
    const f = plannerFacets(cards, { ...none(), entity: new Map([['route', new Set(['R1'])]]) });
    expect(f.visible).toEqual([true, true, false]);
    expect([...f.matches[0]!]).toEqual([0]);
    expect([...f.entity.stop.keys()].sort()).toEqual(['S1', 'S3']);
    expect([...f.rules]).toEqual([['STM_021', 1], ['STM_022', 1]]);
    expect([...f.files]).toEqual([['stop_times.txt', 2]]);
    // The route list itself is not narrowed by its own pick, so another route can be added.
    expect(f.entity.route.get('R2')).toBe(1);
  });

  it('kural seçimi kendi listesini daraltmaz, diğerlerini daraltır', () => {
    const f = plannerFacets(cards, { ...none(), rule: 'STM_022' });
    expect(f.visible).toEqual([false, true, false]);
    expect([...f.rules.keys()]).toEqual(['STM_021', 'STM_022', 'AGN_003']);
    expect([...f.entity.route.keys()]).toEqual(['R1']);
    expect([...f.entity.stop.keys()]).toEqual(['S3']);
  });

  it('alan ve önem seçimi kural listesini daraltır', () => {
    const f = plannerFacets(cards, { ...none(), area: 'stops' });
    expect([...f.rules.keys()]).toEqual(['STM_021']);
    const g = plannerFacets(cards, { ...none(), sev: new Set(['MEDIUM', 'HIGH']) });
    expect([...g.rules.keys()]).toEqual(['STM_022', 'AGN_003']);
  });

  it('gösterilen 500 satırın ötesindeki kayıt da bulunur', () => {
    const rows = Array.from({ length: 600 }, (_, i) => k([i === 550 ? 'R9' : 'R1'], []));
    const big: FacetCard[] = [{ area: 'trips', sev: 'LOW', cls: 'QUALITY', rule: 'TRP_020', file: 'trips.txt', count: 600, keys: rows }];
    const f = plannerFacets(big, { ...none(), entity: new Map([['route', new Set(['R9'])]]) });
    expect(f.visible).toEqual([true]);
    expect([...f.matches[0]!]).toEqual([550]);
  });
});
