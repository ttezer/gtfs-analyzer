import type { NameIndex } from './types';
import { escHtml } from './escape';
import { t } from './i18n';

// Modal showing a route's different trip PATTERNS (ordered stop sequences).
// Data is entirely client-side: name_index.trip_routes + trip_stops (+
// trip_directions + trip_first_dep for direction/frequency). Extra backend field: trip_directions.
// The pattern key matches VAT_003 (ordered stop sequence).

let overlay: HTMLElement | null = null;

const MAX_PATTERN_GROUPS = 500;
const MAX_PATTERN_TRIPS = 100_000;
const MAX_PATTERN_TRIPS_PER_GROUP = 5_000;

export function openPatternModal(routeId: string, routeLabel: string, nameIndex: NameIndex): void {
  ensureOverlay();
  overlay!.querySelector<HTMLElement>('.mm-title')!.textContent =
    `${routeLabel} — ${t('fix.pattern.title')}`;
  overlay!.querySelector<HTMLElement>('.pm-body')!.innerHTML = renderPatterns(routeId, nameIndex);
  overlay!.classList.remove('mm-hidden');
}

export function closePatternModal(): void {
  overlay?.classList.add('mm-hidden');
}

// Dominant direction of the trips in a pattern (direction_id 0 → outbound, 1 → inbound; "—" if absent).
function directionLabel(tripIds: string[], ni: NameIndex): string {
  const counts: Record<string, number> = {};
  for (const tid of tripIds) {
    const d = ni.trip_directions[tid];
    if (d !== undefined) counts[d] = (counts[d] ?? 0) + 1;
  }
  const keys = Object.keys(counts);
  if (keys.length === 0) return '—';
  keys.sort((a, b) => counts[b] - counts[a]);
  const d = keys[0];
  if (d === '0') return t('fix.pattern.dir_out');
  if (d === '1') return t('fix.pattern.dir_in');
  return '—';
}

function renderPatterns(routeId: string, ni: NameIndex): string {
  // Group the route's trips by ordered stop sequence (with the trip list).
  const groups = new Map<string, { stops: string[]; trips: string[] }>();
  let omittedTrips = 0;
  let storedTrips = 0;
  for (const [tripId, rId] of Object.entries(ni.trip_routes)) {
    if (rId !== routeId) continue;
    if (groups.size >= MAX_PATTERN_GROUPS && !groups.has((ni.trip_stops[tripId] ?? []).join('>'))) {
      omittedTrips++;
      continue;
    }
    const stops = ni.trip_stops[tripId] ?? [];
    if (stops.length === 0) continue;
    const key = stops.join('>');
    const e = groups.get(key);
    if (e) {
      if (e.trips.length < MAX_PATTERN_TRIPS_PER_GROUP && storedTrips < MAX_PATTERN_TRIPS) {
        e.trips.push(tripId);
        storedTrips++;
      } else {
        omittedTrips++;
      }
    } else if (storedTrips < MAX_PATTERN_TRIPS) {
      groups.set(key, { stops, trips: [tripId] });
      storedTrips++;
    } else {
      omittedTrips++;
    }
  }
  if (groups.size === 0) {
    return `<p class="pm-empty">${t('fix.pattern.none')}</p>`;
  }
  const patterns = Array.from(groups.values()).sort((a, b) => b.trips.length - a.trips.length);
  const totalTrips = patterns.reduce((s, p) => s + p.trips.length, 0);

  const rows = patterns.map((p, i) => {
    const firstId = p.stops[0];
    const lastId = p.stops[p.stops.length - 1];
    const first = ni.stops[firstId] ?? firstId;
    const last = ni.stops[lastId] ?? lastId;
    const pct = Math.round((p.trips.length / totalTrips) * 100);

    const parts: string[] = [
      `<strong>${t('fix.pattern.pat_n', { n: String(i + 1) })}</strong>`,
      `${t('fix.pattern.trips', { n: String(p.trips.length) })} (${pct}%)`,
      // Keep the direction uniform within each row (or "—") so it does not look inconsistent.
      `<span class="pm-dir">${directionLabel(p.trips, ni)}</span>`,
      `${escHtml(first)} → ${escHtml(last)}`,
      t('fix.pattern.stops_n', { n: String(p.stops.length) }),
    ];

    return `<div class="pm-pat-row">${parts.join(' · ')}</div>`;
  }).join('');

  const summary = `<p class="pm-summary">${t('fix.pattern.summary', {
    p: String(patterns.length),
    t: String(totalTrips),
  })}</p>`;
  const limitNote = omittedTrips > 0
    ? `<p class="pm-summary">${t('fix.pattern.limited', { n: String(omittedTrips) })}</p>`
    : '';
  return summary + limitNote + rows;
}

function ensureOverlay(): void {
  if (overlay) return;
  overlay = document.createElement('div');
  overlay.className = 'mm-overlay mm-hidden';
  overlay.innerHTML = `
    <div class="mm-box" role="dialog" aria-modal="true">
      <div class="mm-header">
        <span class="mm-title"></span>
        <button class="mm-close" title="${t('fix.pattern.close')}" aria-label="${t('fix.pattern.close')}">✕</button>
      </div>
      <div class="pm-body"></div>
    </div>`;
  document.body.appendChild(overlay);

  overlay.querySelector('.mm-close')!.addEventListener('click', closePatternModal);
  overlay.addEventListener('click', e => { if (e.target === overlay) closePatternModal(); });
  document.addEventListener('keydown', e => { if (e.key === 'Escape') closePatternModal(); });
}
