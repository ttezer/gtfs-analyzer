/** Generated from the audited MLIT provision inventory; see the source link. */
import { GTFS_JP_AUTOMATED_COVERAGE_COMPLETE } from './gtfs-jp-coverage.generated';
export {
  GTFS_JP_AUTOMATED_PROVISIONS,
  GTFS_JP_AUTOMATED_COVERAGE_COMPLETE,
  GTFS_JP_UNMATCHED_STRONG_PROVISIONS,
} from './gtfs-jp-coverage.generated';

export type GtfsJpBadgeKey = 'domain.gtfs_jp.coverage' | 'domain.gtfs_jp.profile';

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
