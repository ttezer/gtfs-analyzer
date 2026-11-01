// kk-date wrapper — the single point for all date formatting/arithmetic.
// kk-date (kentkartlab) is for GTFS calendar DATES (YYYYMMDD), not GTFS stop_times
// TIMES beyond 24:00 (those are handled on the Rust side).
// kk-date throws on invalid dates → a validation tool must not crash, so every call is
// guarded defensively and falls back to the raw value.
import KkDate from 'kk-date';
import { getLocale } from './i18n';
import type { Locale } from './i18n';

const LOCALE_TAG: Record<Locale, string> = { tr: 'tr', en: 'en', ja: 'ja', fr: 'fr' };
function tag(): string {
  return LOCALE_TAG[getLocale()] ?? 'en';
}

/** YYYYMMDD integer → localized long date (e.g. "23 August 2024"). Null-safe. */
export function fmtServiceDate(ymd: number | null | undefined, template = 'DD MMMM YYYY'): string | null {
  if (ymd == null) return null;
  try {
    return new KkDate(String(ymd), 'YYYYMMDD').config({ locale: tag() }).format(template);
  } catch {
    return String(ymd);
  }
}

/** Inclusive number of days between two YYYYMMDD values (including both endpoints). */
export function inclusiveDaySpan(startYmd: number, endYmd: number): number {
  try {
    const days = new KkDate(String(startYmd), 'YYYYMMDD').diff(new KkDate(String(endYmd), 'YYYYMMDD'), 'days');
    return Math.abs(days) + 1;
  } catch {
    return 0;
  }
}

/** Day offset between two YYYYMMDD values (absolute; for axis positioning). */
export function dayOffset(fromYmd: number, toYmd: number): number {
  try {
    const d = new KkDate(String(fromYmd), 'YYYYMMDD').diff(new KkDate(String(toYmd), 'YYYYMMDD'), 'days');
    return Math.abs(d);
  } catch {
    return 0;
  }
}

/** Date → localized date-time stamp (report creation time). */
export function fmtTimestamp(d: Date, template = 'DD MMM YYYY HH:mm'): string {
  try {
    return new KkDate(d).config({ locale: tag() }).format(template);
  } catch {
    return d.toLocaleString();
  }
}
