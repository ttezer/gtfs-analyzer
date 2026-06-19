import type { NameIndex } from './types';
import { escHtml } from './escape';
import { t } from './i18n';

// Bir hattın farklı sefer DESENLERİNİ (sıralı durak dizisi) gösteren modal.
// Veri tamamen client-side: name_index.trip_routes + trip_stops. Backend değişmez.
// VAT_003 gruplaması ile aynı mantık (durak dizisi = desen).

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

function renderPatterns(routeId: string, ni: NameIndex): string {
  // Hatta ait sefer'ler → sıralı durak dizisine göre grupla.
  const groups = new Map<string, { stops: string[]; count: number }>();
  for (const [tripId, rId] of Object.entries(ni.trip_routes)) {
    if (rId !== routeId) continue;
    const stops = ni.trip_stops[tripId] ?? [];
    if (stops.length === 0) continue;
    const key = stops.join('>');
    const e = groups.get(key);
    if (e) e.count++;
    else groups.set(key, { stops, count: 1 });
  }
  if (groups.size === 0) {
    return `<p class="pm-empty">${t('fix.pattern.none')}</p>`;
  }
  const patterns = Array.from(groups.values()).sort((a, b) => b.count - a.count);
  const totalTrips = patterns.reduce((s, p) => s + p.count, 0);

  const rows = patterns.map((p, i) => {
    const firstId = p.stops[0];
    const lastId = p.stops[p.stops.length - 1];
    const first = ni.stops[firstId] ?? firstId;
    const last = ni.stops[lastId] ?? lastId;
    const pct = Math.round((p.count / totalTrips) * 100);
    const chips = p.stops
      .map(sid => `<span class="pm-chip">${escHtml(ni.stops[sid] ?? sid)}</span>`)
      .join('');
    const head =
      `<strong>${t('fix.pattern.pat_n', { n: String(i + 1) })}</strong> · ` +
      `${t('fix.pattern.trips', { n: String(p.count) })} (${pct}%) · ` +
      `${t('fix.pattern.stops_n', { n: String(p.stops.length) })} · ` +
      `${escHtml(first)} → ${escHtml(last)}`;
    return `<details class="pm-pat"${i === 0 ? ' open' : ''}>
      <summary>${head}</summary>
      <div class="pm-chips">${chips}</div>
    </details>`;
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
