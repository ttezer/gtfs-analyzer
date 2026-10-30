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
    expect(m.improvements[0].count).toBe(9); // R9 affected_instance_count, notice sayısı değil
    expect(m.infos.map(g => g.ruleId)).toEqual(['CAL_014']);
  });

  it('R9 da olmayan (motorun bastırdığı) kuralları listelemez', () => {
    // CAL_024, TRP_023 tarafından bloke edilir: notice listesinde vardır ama R9 kalemi yoktur.
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
      trip_stops: { T1: ['area_803'], T2: ['area_803'] },
    };
    const r = result(
      [notice('a', 'STM_002', 'CRITICAL', { entity_type: 'Stop', entity_id: 'area_803' })],
      ['a'],
      [['STM_002', 4, 1]],
      true,
      ni,
    );
    const root = { innerHTML: '', querySelectorAll: () => [], querySelector: () => null } as unknown as HTMLElement;
    renderProducer(root, r);
    expect(root.innerHTML).toContain('Seferde kullanılan durak bulunamadı');
    expect(root.innerHTML).toContain('Dial-a-Ride · 2 sefer');
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
    expect(root.innerHTML).toContain('6 hattı · 07:15, 07:45, 08:15… kalkış · 5 sefer');
    // İlk üç kalkış kart bağlamında özetlenir; ayrıntılı tablo artık kartın içinde tutulur.
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
    expect(root.innerHTML).toContain('Kullanılan durak kodu: 71717');
    expect(root.innerHTML).not.toContain('top_id yineleniyor');
  });

  it('STM_012 kartında hızı ve duraklar arasındaki kontrol sırasını planlamacı diliyle gösterir', () => {
    const ni: NameIndex = {
      ...emptyNameIndex,
      routes: { R6: '6' },
      trips: { T1: 'Hastane' },
      trip_routes: { T1: 'R6' },
      trip_first_dep: { T1: '08:45' },
      stops: { S1: 'Merkez', S2: 'Hastane' },
    };
    const r = result(
      [notice('a', 'STM_012', 'HIGH', {
        entity_type: 'Trip', entity_id: 'T1', observed_value: '768 km/h',
        details: { stop_a: 'S1', stop_b: 'S2', distance_km: '29.4', scheduled_gap_seconds: '140', speed_kmh: '768.0' },
      })],
      [],
      [['STM_012', 3, 500]],
      true,
      ni,
    );
    const root = { innerHTML: '', querySelectorAll: () => [], querySelector: () => null } as unknown as HTMLElement;
    renderProducer(root, r);
    expect(root.innerHTML).toContain('Duraklar arasında gerçekçi olmayan hız');
    expect(root.innerHTML).toContain('6 hattı · Hastane yönü · 08:45 kalkışlı sefer');
    expect(root.innerHTML).toContain('Görülen değer:</span> <code>768 km/h</code>');
    expect(root.innerHTML).toContain('Merkez ile Hastane arasında hesaplanan hız gerçekçi görünmüyor');
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
    expect(root.innerHTML).toContain('6 hattı · Hastane yönü · 07:15 kalkışlı sefer');
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
      [notice('a', 'STM_002', 'CRITICAL', { entity_type: 'Stop', entity_id: 'S1' }), notice('b', 'DQ_016', 'MEDIUM')],
      [],
      [['STM_002', 4, 1], ['DQ_016', 2, 1]],
    );
    const el = root();
    renderProducer(el, r);
    const technicalAt = el.innerHTML.indexOf('planner-technical');
    expect(technicalAt).toBeGreaterThan(0);
    expect(el.innerHTML.indexOf('data-rule="STM_002"')).toBeLessThan(technicalAt);
    expect(el.innerHTML.indexOf('data-rule="DQ_016"')).toBeGreaterThan(technicalAt);
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
    expect(el.innerHTML).toContain('<th>Hat</th><th>Yön</th><th>İlk kalkış</th></tr>');
    expect(el.innerHTML).toContain('<td>Üniversite</td>');
    expect(el.innerHTML).not.toContain('<th>Dosya</th>');
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
