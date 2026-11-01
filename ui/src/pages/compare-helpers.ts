// Pure comparison helpers (independent of rendering/worker → directly unit-testable).
// Imports only the `GoldenRun` type (not a runtime import).
import type { GoldenRun } from '../golden';

/** Makes config delta independent of key order; returns the raw value for invalid JSON. */
export function normalizeConfig(value: string): string {
  try {
    const parsed = JSON.parse(value || '{}') as Record<string, unknown>;
    return JSON.stringify(Object.fromEntries(Object.entries(parsed).sort()));
  } catch {
    return value;
  }
}

/** True when both feed names are present and different (incompatible-feed warning). */
export function feedMismatch(before: GoldenRun, now: GoldenRun): boolean {
  return Boolean(before.feedName && now.feedName && before.feedName !== now.feedName);
}

/** True when any feed/service start or end metric changed. */
export function dateRangeChanged(before: GoldenRun, now: GoldenRun): boolean {
  return ['feed_start_date', 'feed_end_date', 'service_start_date', 'service_end_date']
    .some(key => before.metrics[key] !== now.metrics[key]);
}

/** True when config deltas differ (independent of key order). */
export function configChanged(before: GoldenRun, now: GoldenRun): boolean {
  return normalizeConfig(before.configDelta) !== normalizeConfig(now.configDelta);
}

/** Notice density: scaled by the denominator (per 1,000 trips / per 100k stop_times). Zero denominator → 0. */
export function noticeDensity(run: GoldenRun, denominator: number, scale: number): number {
  return denominator > 0 ? run.noticeTotal / denominator * scale : 0;
}
