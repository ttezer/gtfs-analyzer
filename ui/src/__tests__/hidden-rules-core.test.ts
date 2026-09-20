import { describe, expect, it } from 'vitest';
import { parseHiddenRules, withHiddenRule, withoutHiddenRule } from '../hidden-rules-core';

describe('parseHiddenRules', () => {
  it('boş delta için boş liste verir', () => {
    expect(parseHiddenRules('')).toEqual([]);
    expect(parseHiddenRules('{}')).toEqual([]);
  });
  it('listeyi okur', () => {
    expect(parseHiddenRules('{"disabled_rule_ids":["DQ_003","DQ_004"]}')).toEqual(['DQ_003', 'DQ_004']);
  });
  it('bozuk JSON çökmez, boş liste verir', () => {
    expect(parseHiddenRules('{bozuk')).toEqual([]);
  });
  it('dizi olmayan değeri yok sayar ve string olmayan öğeleri atar', () => {
    expect(parseHiddenRules('{"disabled_rule_ids":"DQ_003"}')).toEqual([]);
    expect(parseHiddenRules('{"disabled_rule_ids":["DQ_003",7,null]}')).toEqual(['DQ_003']);
  });
});

describe('withHiddenRule', () => {
  it('boş deltaya ilk kuralı ekler', () => {
    expect(withHiddenRule('', 'DQ_004')).toBe('{"disabled_rule_ids":["DQ_004"]}');
  });
  it('listeyi sıralı tutar', () => {
    expect(withHiddenRule('{"disabled_rule_ids":["TRP_021"]}', 'DQ_004'))
      .toBe('{"disabled_rule_ids":["DQ_004","TRP_021"]}');
  });
  it('zaten gizliyse değişiklik yok (null)', () => {
    expect(withHiddenRule('{"disabled_rule_ids":["DQ_004"]}', 'DQ_004')).toBeNull();
  });
  /** Ayar panelinin yazdığı eşikler ve GTFS-JP profili kaybolmamalı. */
  it('diğer config anahtarlarını korur', () => {
    const next = withHiddenRule('{"gtfs_jp_profile":"v4","max_speed_kmh":120}', 'DQ_004');
    expect(JSON.parse(next!)).toEqual({
      gtfs_jp_profile: 'v4',
      max_speed_kmh: 120,
      disabled_rule_ids: ['DQ_004'],
    });
  });
});

describe('withoutHiddenRule', () => {
  it('kuralı çıkarır ve diğerini bırakır', () => {
    expect(withoutHiddenRule('{"disabled_rule_ids":["DQ_003","DQ_004"]}', 'DQ_003'))
      .toBe('{"disabled_rule_ids":["DQ_004"]}');
  });
  it('listede yoksa değişiklik yok (null)', () => {
    expect(withoutHiddenRule('{"disabled_rule_ids":["DQ_004"]}', 'TRP_021')).toBeNull();
    expect(withoutHiddenRule('', 'DQ_004')).toBeNull();
  });
  /** Liste boşalınca anahtar düşmeli; delta da boşaldıysa motor için '' olmalı. */
  it('son kural çıkınca deltayı tamamen temizler', () => {
    expect(withoutHiddenRule('{"disabled_rule_ids":["DQ_004"]}', 'DQ_004')).toBe('');
  });
  it('son kural çıksa da diğer ayarlar durur', () => {
    expect(withoutHiddenRule('{"gtfs_jp_profile":"v3","disabled_rule_ids":["DQ_004"]}', 'DQ_004'))
      .toBe('{"gtfs_jp_profile":"v3"}');
  });
});

describe('gidiş-dönüş', () => {
  it('ekle sonra çıkar başlangıç deltasını geri verir', () => {
    const start = '{"gtfs_jp_profile":"v4"}';
    const added = withHiddenRule(start, 'DQ_004')!;
    expect(withoutHiddenRule(added, 'DQ_004')).toBe(start);
  });
});
