import type { FileInfo, Notice, Severity, ValidationResult } from './types';

export const REQUIRED_FILES = [
  'agency.txt', 'stops.txt', 'routes.txt', 'trips.txt', 'stop_times.txt',
] as const;

export const CALENDAR_FILES = ['calendar.txt', 'calendar_dates.txt'] as const;

export const GTFS_SPEC_FILES = [
  'agency.txt', 'stops.txt', 'routes.txt', 'trips.txt', 'stop_times.txt',
  'calendar.txt', 'calendar_dates.txt',
  'fare_attributes.txt', 'fare_rules.txt',
  'timeframes.txt', 'rider_categories.txt', 'fare_media.txt', 'fare_products.txt',
  'fare_leg_rules.txt', 'fare_leg_join_rules.txt', 'fare_transfer_rules.txt',
  'areas.txt', 'stop_areas.txt', 'networks.txt', 'route_networks.txt',
  'shapes.txt', 'frequencies.txt', 'transfers.txt', 'pathways.txt', 'levels.txt',
  'location_groups.txt', 'location_group_stops.txt', 'locations.geojson',
  'booking_rules.txt', 'translations.txt', 'feed_info.txt', 'attributions.txt',
] as const;

const GTFS_SPEC_FILE_SET = new Set<string>(GTFS_SPEC_FILES);
const REQUIRED_FILE_SET = new Set<string>(REQUIRED_FILES);
const CALENDAR_FILE_SET = new Set<string>(CALENDAR_FILES);

export const SEVERITY_ORDER: Record<Severity, number> = {
  CRITICAL: 0,
  HIGH: 1,
  MEDIUM: 2,
  LOW: 3,
  INFO: 4,
};

export interface FileSummary {
  name: string;
  info: FileInfo | null;
  notices: Notice[];
  severityCounts: Record<Severity, number>;
  worstSeverity: Severity | null;
  scoreDelta: number;
  missing: boolean;
  required: boolean;
  unknown: boolean;
  general: boolean;
}

interface BuildFileSummaryOptions {
  includeAllSpec?: boolean;
  includeGeneral?: boolean;
}

export function buildFileSummaries(
  result: ValidationResult,
  options: BuildFileSummaryOptions = {},
): FileSummary[] {
  const { notices, metrics, reports } = result;
  const includeAllSpec = options.includeAllSpec ?? false;
  const includeGeneral = options.includeGeneral ?? true;

  const noticeDelta = new Map<string, number>();
  for (const item of reports.r9.items) {
    for (const noticeId of item.notice_ids) {
      noticeDelta.set(noticeId, (noticeDelta.get(noticeId) ?? 0) + item.score_delta);
    }
  }

  const fileNotices = new Map<string, Notice[]>();
  const generalNotices: Notice[] = [];
  for (const notice of notices) {
    if (notice.file) {
      const current = fileNotices.get(notice.file) ?? [];
      current.push(notice);
      fileNotices.set(notice.file, current);
    } else {
      generalNotices.push(notice);
    }
  }

  const statsMap = new Map<string, FileInfo>(
    metrics.file_stats.map((file) => [file.name, file]),
  );
  const presentSet = new Set(statsMap.keys());
  const allNames = new Set<string>([
    ...statsMap.keys(),
    ...fileNotices.keys(),
    ...REQUIRED_FILES,
    ...CALENDAR_FILES,
    ...(includeAllSpec ? GTFS_SPEC_FILES : []),
  ]);

  const summaries: FileSummary[] = [];
  for (const name of allNames) {
    const fileNoticesForName = fileNotices.get(name) ?? [];
    const present = presentSet.has(name);
    const isSpec = GTFS_SPEC_FILE_SET.has(name);
    summaries.push({
      name,
      info: statsMap.get(name) ?? null,
      notices: fileNoticesForName,
      severityCounts: countSeverities(fileNoticesForName),
      worstSeverity: findWorstSeverity(fileNoticesForName),
      scoreDelta: fileNoticesForName.reduce(
        (sum, notice) => sum + (noticeDelta.get(notice.id) ?? 0),
        0,
      ),
      missing: !present && isSpec,
      required: REQUIRED_FILE_SET.has(name) || CALENDAR_FILE_SET.has(name),
      unknown: !present && !isSpec,
      general: false,
    });
  }

  const present = summaries
    .filter((summary) => !summary.missing && !summary.unknown)
    .sort(compareFileSummaries);
  const unknown = summaries
    .filter((summary) => summary.unknown)
    .sort((a, b) => a.name.localeCompare(b.name));
  const missing = summaries
    .filter((summary) => summary.missing)
    .sort((a, b) => a.name.localeCompare(b.name));

  if (!includeGeneral || generalNotices.length === 0) {
    return [...present, ...unknown, ...missing];
  }

  const general: FileSummary = {
    name: '__general__',
    info: null,
    notices: generalNotices,
    severityCounts: countSeverities(generalNotices),
    worstSeverity: findWorstSeverity(generalNotices),
    scoreDelta: generalNotices.reduce(
      (sum, notice) => sum + (noticeDelta.get(notice.id) ?? 0),
      0,
    ),
    missing: false,
    required: false,
    unknown: false,
    general: true,
  };
  return [...present, ...unknown, general, ...missing];
}

export function formatFileBytes(bytes: number): string {
  if (bytes < 1024) return `${bytes} B`;
  if (bytes < 1024 * 1024) return `${(bytes / 1024).toFixed(1)} KB`;
  return `${(bytes / 1024 / 1024).toFixed(1)} MB`;
}

function countSeverities(notices: Notice[]): Record<Severity, number> {
  const counts: Record<Severity, number> = {
    CRITICAL: 0,
    HIGH: 0,
    MEDIUM: 0,
    LOW: 0,
    INFO: 0,
  };
  for (const notice of notices) counts[notice.severity] += 1;
  return counts;
}

function findWorstSeverity(notices: Notice[]): Severity | null {
  let worst: Severity | null = null;
  for (const notice of notices) {
    if (worst === null || SEVERITY_ORDER[notice.severity] < SEVERITY_ORDER[worst]) {
      worst = notice.severity;
    }
  }
  return worst;
}

function compareFileSummaries(a: FileSummary, b: FileSummary): number {
  const aOrder = a.worstSeverity ? SEVERITY_ORDER[a.worstSeverity] : 99;
  const bOrder = b.worstSeverity ? SEVERITY_ORDER[b.worstSeverity] : 99;
  if (aOrder !== bOrder) return aOrder - bOrder;
  return b.notices.length - a.notices.length;
}
