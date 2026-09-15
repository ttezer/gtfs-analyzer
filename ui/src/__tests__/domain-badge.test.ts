import { describe, expect, it } from 'vitest';
import { gtfsJpBadgeKey, GTFS_JP_AUTOMATED_COVERAGE_COMPLETE } from '../gtfs-jp-badge';

describe('GTFS-JP profile badge scope', () => {
  it('uses the static automated-coverage catalog contract', () => {
    expect(GTFS_JP_AUTOMATED_COVERAGE_COMPLETE).toBe(true);
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
