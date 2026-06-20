import type { NameIndex } from './types';
import { escHtml } from './escape';
import { t } from './i18n';

// Bir hattın farklı sefer DESENLERİNİ (sıralı durak dizisi) gösteren modal.
// Veri tamamen client-side: name_index.trip_routes + trip_stops (+ yön/sıklık için
// trip_directions + trip_first_dep). Backend'e ek alan: trip_directions.
// Desen anahtarı VAT_003 ile aynı (sıralı durak dizisi).

let overlay: HTMLElement | null = null;

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

// Desendeki seferlerin baskın yönü (direction_id 0 → Gidiş, 1 → Dönüş; yoksa "—").
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
  // Hatta ait sefer'ler → sıralı durak dizisine göre grupla (trip listesiyle).
  const groups = new Map<string, { stops: string[]; trips: string[] }>();
  for (const [tripId, rId] of Object.entries(ni.trip_routes)) {
    if (rId !== routeId) continue;
    const stops = ni.trip_stops[tripId] ?? [];
    if (stops.length === 0) continue;
    const key = stops.join('>');
    const e = groups.get(key);
    if (e) e.trips.push(tripId);
    else groups.set(key, { stops, trips: [tripId] });
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
      // Yön her satırda uniform (yoksa "—") — tutarsız görünmesin.
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
  return summary + rows;
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
