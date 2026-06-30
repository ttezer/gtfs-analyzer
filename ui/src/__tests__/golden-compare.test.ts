import { describe, expect, it } from 'vitest';
import { buildGoldenSnapshot, compareGolden, parseGolden, type GoldenRun } from '../golden';
import type { ValidationResult } from '../types';

function run(rules: Record<string, number>): GoldenRun {
  return { schema: 'gtfs-analyzer-golden/4', appVersion: 'x', generatedAt: null, validateDate: '', feedName: 'feed.zip',
    engineMode: 'unknown',
    files: {}, metrics: {}, scores: { overall: 0, score: 0, pub_score: 0, spec: 0, interop: 0, quality: 0, analytics: 0 },
    severityCounts: {}, classCounts: {}, rules: Object.fromEntries(Object.entries(rules).map(([id, count]) => [id, { count }])),
    noticeTotal: 0, configDelta: '', legacy: false };
}

// buildGoldenSnapshot'ın okuduğu alanları taşıyan minimal mock; geri kalan
// ValidationResult alanları snapshot tarafından kullanılmaz.
function mockResult(): ValidationResult {
  const notice = (rule_id: string, severity: string, rule_class: string, title: string) =>
    ({ rule_id, severity, rule_class, title });
  return {
    notices: [
      notice('STM_014', 'HIGH', 'QUALITY', 'Şüpheli hız'),
      notice('STM_014', 'HIGH', 'QUALITY', 'Şüpheli hız'),
      notice('CAL_013', 'MEDIUM', 'SPEC', 'Süresi geçmiş servis'),
      notice('ARC_007', 'LOW', 'INTEROP', 'Eksik alan'),
    ],
    capped_totals: { STM_014: 500 }, // gerçek toplam görünür sayımı (2) geçersiz kılmalı
    reports: { r5: { score: 83.6, pub_score: 92.6, spec_score: 92.6, interop_score: 90.1, quality_score: 65.8, analytics_score: 63.3 } },
    metrics: {
      route_count: 10, stop_count: 200, trip_count: 1500, shape_count: 12,
      active_service_days: 90, avg_daily_trips: 300,
      feed_start_date: 20260101, feed_end_date: 20261231,
      service_start_date: 20260105, service_end_date: 20261220, is_gtfs_jp: false,
      file_stats: [
        { name: 'stop_times.txt', rows: 60000, bytes: 4_000_000 },
        { name: 'calendar_dates.txt', rows: 1200, bytes: 30_000 },
      ],
    },
  } as unknown as ValidationResult;
}

describe('Golden comparison', () => {
  it('classifies fixed, decreased, increased, new and same rules', () => {
    const changes = compareGolden(run({ A: 10, B: 10, C: 10, E: 4 }), run({ B: 5, C: 15, D: 2, E: 4 }));
    expect(Object.fromEntries(changes.map(c => [c.id, c.kind]))).toEqual({ A: 'fixed', B: 'decreased', C: 'increased', D: 'new', E: 'same' });
  });

  it('reads legacy golden/1 through golden/3', () => {
    for (const version of [1, 2, 3]) {
      const parsed = parseGolden(JSON.stringify({ schema: `gtfs-analyzer-golden/${version}`, app_version: '0.1.4', feed: 'x.zip',
        validate_date: '2026-01-01', scores: { overall: 80 }, notice_total_actual: 7, rule_counts_actual: { STM_014: 7 } }));
      expect(parsed.legacy).toBe(true);
      expect(parsed.rules.STM_014.count).toBe(7);
    }
  });

  it('snapshot /5 round-trips through parseGolden with engine metadata', () => {
    const json = buildGoldenSnapshot(mockResult(), 'BART.zip', new Date('2026-06-28T10:00:00Z'), '{"foo":1}', 'wasm32-threaded');
    const parsed = parseGolden(json);

    expect(parsed.schema).toBe('gtfs-analyzer-golden/5');
    expect(parsed.legacy).toBe(false);
    expect(parsed.feedName).toBe('BART.zip');
    expect(parsed.configDelta).toBe('{"foo":1}');
    expect(parsed.generatedAt).toBe('2026-06-28T10:00:00.000Z');
    expect(parsed.validateDate).toBe('2026-06-28');
    expect(parsed.engineMode).toBe('wasm32-threaded');

    // capped_totals görünür sayımı geçersiz kılar (2 görünür → 500 gerçek).
    expect(parsed.rules.STM_014.count).toBe(500);
    expect(parsed.rules.CAL_013.count).toBe(1);
    expect(parsed.rules.STM_014.rule_class).toBe('QUALITY');

    // notice_total_actual = sayımların toplamı (500 + 1 + 1).
    expect(parsed.noticeTotal).toBe(502);

    // Severity/class dağılımı capped sayımları kullanır.
    expect(parsed.severityCounts.HIGH).toBe(500);
    expect(parsed.severityCounts.MEDIUM).toBe(1);
    expect(parsed.classCounts.QUALITY).toBe(500);
    expect(parsed.classCounts.SPEC).toBe(1);

    // Feed yapısı + metrikler taşınır.
    expect(parsed.files['stop_times.txt'].rows).toBe(60000);
    expect(parsed.metrics.trip_count).toBe(1500);
    expect(parsed.scores.overall).toBe(83.6);
    expect(parsed.scores.quality).toBe(65.8);
  });

  it('reads golden/4 without engine metadata as unknown', () => {
    const parsed = parseGolden(JSON.stringify({
      schema: 'gtfs-analyzer-golden/4', app_version: '0.2.0', generated_at: '2026-06-28T10:00:00.000Z',
      validate_date: '2026-06-28', feed: { file_name: 'BART.zip', files: {}, metrics: {} },
      validation: { config_delta: '', scores: {}, notice_total_actual: 0, severity_counts: {}, class_counts: {}, rule_counts: {} },
    }));
    expect(parsed.legacy).toBe(false);
    expect(parsed.engineMode).toBe('unknown');
  });
});
