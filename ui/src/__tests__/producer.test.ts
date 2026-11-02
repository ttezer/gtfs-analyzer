import { describe, it, expect, vi } from 'vitest';
import { buildProducerModel, stopUsage, entityLabel, plannerArea, routeBadge } from '../pages/producer-helpers';
import { renderProducer } from '../pages/producer';
import type { ValidationResult, Notice, NameIndex } from '../types';

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
    expect(root.innerHTML).toContain('Hat kodu R1');
    expect(root.innerHTML).toContain('Year round (Weekday)');
    expect(root.innerHTML).toContain('planner-route-code">R1</span>');
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

  it('STM_015 için hatalı yeri durak adı ve kalkış saatiyle, STM_008 için iki durağın saatiyle yazar', () => {
    const ni: NameIndex = { ...emptyNameIndex, stops: { S1: 'Onkoloji', S2: 'Binevler', A: 'Çarşı', B: 'Liman' }, trip_routes: { T1: 'R1', T2: 'R1' }, routes: { R1: 'S08' } };
    const r = result([
      notice('a', 'STM_015', 'CRITICAL', { entity_type: 'Trip', entity_id: 'T1', details: { stop_id: 'S1', departure: '06:15:00', next_stop_id: 'S2', next_time: '06:18:00' } }),
      notice('b', 'STM_008', 'CRITICAL', { entity_type: 'Trip', entity_id: 'T2', details: { stop_a: 'A', stop_b: 'B', dep: '08:00:00', arr: '07:55:00' } }),
    ], ['a', 'b'], [['STM_015', 4, 1], ['STM_008', 4, 1]], true, ni);
    const el = root();
    renderProducer(el, r);
    expect(el.innerHTML).toContain('Seferin ilk durağı olan Onkoloji için varış saati girilmemiş.');
    expect(el.innerHTML).toContain('<td>Onkoloji · kalkış 06:15:00</td>');
    expect(el.innerHTML).toContain('<td>Çarşı 08:00:00 → Liman 07:55:00</td>');
    expect(el.innerHTML).toContain('Çarşı durağından 08:00:00 kalkışından sonra Liman durağına 07:55:00 varışı yazılmış');
    expect(el.innerHTML).not.toContain('stop_times.txt zaman değerlerini');
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
    expect(ids).toContain('<td>43GX03 → 44GX02</td>');
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
