import { describe, expect, it } from 'vitest';
import { compareGolden, parseGolden, type GoldenRun } from '../golden';

function run(rules: Record<string, number>): GoldenRun {
  return { schema: 'gtfs-analyzer-golden/4', appVersion: 'x', generatedAt: null, validateDate: '', feedName: 'feed.zip',
    files: {}, metrics: {}, scores: { overall: 0, score: 0, pub_score: 0, spec: 0, interop: 0, quality: 0, analytics: 0 },
    severityCounts: {}, classCounts: {}, rules: Object.fromEntries(Object.entries(rules).map(([id, count]) => [id, { count }])),
    noticeTotal: 0, configDelta: '', legacy: false };
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
});
