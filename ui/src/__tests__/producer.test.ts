import { describe, it, expect, vi } from 'vitest';
import { buildProducerModel, stopUsage, entityLabel } from '../pages/producer-helpers';
import { renderProducer } from '../pages/producer';
import type { ValidationResult, Notice, NameIndex } from '../types';

vi.mock('../state', () => ({ setPage: vi.fn() }));
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
): ValidationResult {
  return {
    notices,
    name_index: emptyNameIndex,
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
    const root = { innerHTML: '', querySelectorAll: () => [] } as unknown as HTMLElement;
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
    const root = { innerHTML: '', querySelectorAll: () => [] } as unknown as HTMLElement;
    renderProducer(root, r);
    expect(root.innerHTML).toContain('Kontrol eksik kaldı');
    expect(root.innerHTML).not.toContain('motor metni');
  });
});
