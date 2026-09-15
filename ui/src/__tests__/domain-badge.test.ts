import { describe, expect, it } from 'vitest';
import { gtfsJpBadgeKey } from '../gtfs-jp-badge';
import { GTFS_JP_AUTOMATED_COVERAGE_COMPLETE, GTFS_JP_AUTOMATED_PROVISIONS } from '../gtfs-jp-coverage.generated';

describe('GTFS-JP profile badge scope', () => {
  it('uses the generated MLIT provision inventory contract', () => {
    expect(GTFS_JP_AUTOMATED_PROVISIONS.length).toBeGreaterThan(20);
    expect(GTFS_JP_AUTOMATED_PROVISIONS.flatMap((p) => p.ruleIds)).toContain('JPN_006');
    expect(GTFS_JP_AUTOMATED_PROVISIONS.flatMap((p) => p.ruleIds)).toContain('JPN_007');
    expect(GTFS_JP_AUTOMATED_PROVISIONS.every((p) => p.ruleIds.length > 0)).toBe(true);
    expect(GTFS_JP_AUTOMATED_COVERAGE_COMPLETE).toBe(
      GTFS_JP_AUTOMATED_PROVISIONS.length > 0 &&
        GTFS_JP_AUTOMATED_PROVISIONS.every((p) => p.ruleIds.length > 0),
    );
    expect(gtfsJpBadgeKey('v3', true)).toBe('domain.gtfs_jp.coverage');
  });

  it('shows the 100% machine coverage badge only for detected complete V3/V4 analyses', () => {
    expect(gtfsJpBadgeKey('v3', true, true)).toBe('domain.gtfs_jp.coverage');
    expect(gtfsJpBadgeKey('V4', true, true)).toBe('domain.gtfs_jp.coverage');
    expect(gtfsJpBadgeKey('v3', false, true)).toBe('domain.gtfs_jp.profile');
    expect(gtfsJpBadgeKey('v4', true, false)).toBe('domain.gtfs_jp.profile');
  });

  it('keeps Auto and unknown profiles out of the version coverage claim', () => {
    expect(gtfsJpBadgeKey('auto', true, true)).toBe('domain.gtfs_jp.profile');
    expect(gtfsJpBadgeKey(undefined, true, true)).toBeNull();
    expect(gtfsJpBadgeKey('experimental', true, true)).toBeNull();
  });
});
