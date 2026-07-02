import { describe, expect, it } from 'vitest';
import {
  feedMismatch,
  dateRangeChanged,
  configChanged,
  normalizeConfig,
  noticeDensity,
} from '../pages/compare-helpers';
import type { GoldenRun } from '../golden';

function run(over: Partial<GoldenRun> = {}): GoldenRun {
  return {
    schema: 'gtfs-analyzer-golden/4', appVersion: 'x', generatedAt: null, validateDate: '',
    engineMode: 'unknown', feedName: 'feed.zip',
    files: {}, metrics: {},
    scores: { overall: 0, score: 0, pub_score: 0, spec: 0, interop: 0, quality: 0, analytics: 0 },
    severityCounts: {}, classCounts: {}, rules: {}, noticeTotal: 0, configDelta: '', legacy: false,
    ...over,
  };
}

describe('feedMismatch', () => {
  it('same feed name → false', () => {
    expect(feedMismatch(run({ feedName: 'a.zip' }), run({ feedName: 'a.zip' }))).toBe(false);
  });
  it('different feed names → true', () => {
    expect(feedMismatch(run({ feedName: 'a.zip' }), run({ feedName: 'b.zip' }))).toBe(true);
  });
  it('one name empty → false (cannot assert a mismatch)', () => {
    expect(feedMismatch(run({ feedName: '' }), run({ feedName: 'b.zip' }))).toBe(false);
    expect(feedMismatch(run({ feedName: 'a.zip' }), run({ feedName: '' }))).toBe(false);
  });
  it('both empty → false', () => {
    expect(feedMismatch(run({ feedName: '' }), run({ feedName: '' }))).toBe(false);
  });
});

describe('dateRangeChanged', () => {
  const base = {
    feed_start_date: 20260101, feed_end_date: 20261231,
    service_start_date: 20260101, service_end_date: 20261231,
  };
  it('identical ranges → false', () => {
    expect(dateRangeChanged(run({ metrics: { ...base } }), run({ metrics: { ...base } }))).toBe(false);
  });
  for (const key of ['feed_start_date', 'feed_end_date', 'service_start_date', 'service_end_date'] as const) {
    it(`${key} differs → true`, () => {
      expect(dateRangeChanged(
        run({ metrics: { ...base } }),
        run({ metrics: { ...base, [key]: 99999999 } }),
      )).toBe(true);
    });
  }
  it('unrelated metric differs → false', () => {
    expect(dateRangeChanged(
      run({ metrics: { ...base, trip_count: 100 } }),
      run({ metrics: { ...base, trip_count: 999 } }),
    )).toBe(false);
  });
});

describe('configChanged / normalizeConfig', () => {
  it('identical config → false', () => {
    expect(configChanged(run({ configDelta: '{"a":1,"b":2}' }), run({ configDelta: '{"a":1,"b":2}' }))).toBe(false);
  });
  it('reordered keys, same values → false (key order normalized)', () => {
    expect(configChanged(run({ configDelta: '{"a":1,"b":2}' }), run({ configDelta: '{"b":2,"a":1}' }))).toBe(false);
  });
  it('different value → true', () => {
    expect(configChanged(run({ configDelta: '{"a":1}' }), run({ configDelta: '{"a":2}' }))).toBe(true);
  });
  it('empty string vs empty object → false', () => {
    expect(configChanged(run({ configDelta: '' }), run({ configDelta: '{}' }))).toBe(false);
  });
  it('normalizeConfig sorts keys', () => {
    expect(normalizeConfig('{"b":2,"a":1}')).toBe('{"a":1,"b":2}');
  });
  it('normalizeConfig falls back to raw on invalid JSON', () => {
    expect(normalizeConfig('not json')).toBe('not json');
  });
});

describe('noticeDensity', () => {
  it('normalizes per 1,000 trips', () => {
    expect(noticeDensity(run({ noticeTotal: 1500 }), 3000, 1000)).toBe(500);
  });
  it('normalizes per 100,000 stop_times', () => {
    expect(noticeDensity(run({ noticeTotal: 200 }), 1_000_000, 100000)).toBe(20);
  });
  it('zero denominator → 0 (no division by zero)', () => {
    expect(noticeDensity(run({ noticeTotal: 100 }), 0, 1000)).toBe(0);
  });
});
