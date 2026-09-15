export type GtfsJpBadgeKey = 'domain.gtfs_jp.coverage' | 'domain.gtfs_jp.profile';

/**
 * Static product contract: the scoped V3/V4 JPN rule inventory covers every
 * machine-verifiable MLIT provision; human review and external verification are
 * excluded. Feed-level R1 coverage is intentionally unrelated to this claim.
 * See docs/gtfs-jp-automated-coverage.md.
 */
export const GTFS_JP_AUTOMATED_COVERAGE_COMPLETE = true as const;

/** Selects the claim a UI badge may make without inferring a feed version. */
export function gtfsJpBadgeKey(
  profile: string | null | undefined,
  detected: boolean,
  automatedCoverageComplete: boolean = GTFS_JP_AUTOMATED_COVERAGE_COMPLETE,
): GtfsJpBadgeKey | null {
  const normalized = profile?.toLowerCase();
  if (!normalized || !['auto', 'v3', 'v4'].includes(normalized)) return null;
  if (detected && automatedCoverageComplete && ['v3', 'v4'].includes(normalized)) {
    return 'domain.gtfs_jp.coverage';
  }
  return 'domain.gtfs_jp.profile';
}
