export type GtfsJpBadgeKey = 'domain.gtfs_jp.coverage' | 'domain.gtfs_jp.profile';

/** Selects the claim a UI badge may make without inferring a feed version. */
export function gtfsJpBadgeKey(
  profile: string | null | undefined,
  detected: boolean,
  coverageComplete: boolean,
): GtfsJpBadgeKey | null {
  const normalized = profile?.toLowerCase();
  if (!normalized || !['auto', 'v3', 'v4'].includes(normalized)) return null;
  if (detected && coverageComplete && ['v3', 'v4'].includes(normalized)) {
    return 'domain.gtfs_jp.coverage';
  }
  return 'domain.gtfs_jp.profile';
}
