import { describe, expect, it } from 'vitest';
import { gtfsJpBadgeKey } from '../gtfs-jp-badge';

describe('GTFS-JP profile badge scope', () => {
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
