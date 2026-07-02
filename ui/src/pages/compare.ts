import type { ValidationResult } from '../types';
import { buildGoldenSnapshot, compareGolden, parseGolden, type ChangeKind, type GoldenRun, type RuleChange } from '../golden';
import { t } from '../i18n';
import { getLastEngineMode } from '../validator-client';
import { feedMismatch, dateRangeChanged, configChanged, noticeDensity } from './compare-helpers';

const nf = new Intl.NumberFormat();
const sf = new Intl.NumberFormat(undefined, { minimumFractionDigits: 1, maximumFractionDigits: 1, signDisplay: 'always' });
const SCORE_KEYS = ['pub_score', 'overall', 'spec', 'interop', 'quality', 'analytics'] as const;

interface ComparisonState { currentKey: string; before: GoldenRun }
let comparisonState: ComparisonState | null = null;

function esc(s: string): string { return s.replace(/[&<>"']/g, c => ({ '&': '&amp;', '<': '&lt;', '>': '&gt;', '"': '&quot;', "'": '&#39;' })[c]!); }
function n(v: unknown): number { return typeof v === 'number' && Number.isFinite(v) ? v : 0; }
function dateLabel(run: GoldenRun): string {
  if (run.generatedAt) {
    const date = new Date(run.generatedAt);
    if (!Number.isNaN(date.getTime())) return new Intl.DateTimeFormat(undefined, { dateStyle: 'medium', timeStyle: 'short' }).format(date);
  }
  return run.validateDate || t('compare.unknown_date');
}
function currentRun(result: ValidationResult, fileName: string, generatedAt: Date, configDelta: string): GoldenRun {
  return parseGolden(buildGoldenSnapshot(result, fileName, generatedAt, configDelta, getLastEngineMode() ?? 'unknown'));
}
function deltaClass(delta: number): string { return delta > 0 ? 'bad' : delta < 0 ? 'good' : 'same'; }
function scoreDelta(delta: number): string { return `${delta >= 0 ? '+' : ''}${delta.toFixed(1)}`; }

export function renderCompare(root: HTMLElement, result: ValidationResult, fileName: string, generatedAt: Date, configDelta: string): void {
  const now = currentRun(result, fileName, generatedAt, configDelta);
  const currentKey = `${now.generatedAt ?? now.validateDate}|${now.feedName}|${now.noticeTotal}`;
  if (comparisonState?.currentKey !== currentKey) comparisonState = null;
  root.innerHTML = `<section class="compare-page">
    <div class="compare-head"><div><h2>${t('compare.title')}</h2><p>${t('compare.subtitle')}</p></div>
      <label class="compare-upload btn btn-primary" for="compare-golden-input">${t('compare.select_old')}</label>
      <input id="compare-golden-input" class="visually-hidden" type="file" accept=".json,application/json"></div>
    <div id="compare-error" class="upload-status hidden"></div>
    <div id="compare-content" class="compare-empty"><strong>${t('compare.empty_title')}</strong><span>${t('compare.empty_desc')}</span></div>
  </section>`;
  root.querySelector<HTMLInputElement>('#compare-golden-input')!.addEventListener('change', async e => {
    const input = e.currentTarget as HTMLInputElement;
    const file = input.files?.[0]; if (!file) return;
    const error = root.querySelector<HTMLElement>('#compare-error')!;
    try {
      const before = parseGolden(await file.text());
      comparisonState = { currentKey, before };
      renderReport(root.querySelector<HTMLElement>('#compare-content')!, before, now);
      error.className = 'upload-status hidden';
    } catch (err) {
      error.className = 'upload-status error'; error.textContent = err instanceof Error ? err.message : String(err);
    } finally {
      // Aynı dosya dahil her yeni seçimde change olayı yeniden çalışsın.
      input.value = '';
    }
  });
  if (comparisonState) renderReport(root.querySelector<HTMLElement>('#compare-content')!, comparisonState.before, now);
}

function renderReport(host: HTMLElement, before: GoldenRun, now: GoldenRun): void {
  const changes = compareGolden(before, now);
  const counts = (kind: ChangeKind) => changes.filter(c => c.kind === kind).length;
  const totalDelta = now.noticeTotal - before.noticeTotal;
  const cards = [
    ['compare.pub_score', n(before.scores.pub_score), n(now.scores.pub_score)],
    ['compare.overall_score', n(before.scores.overall ?? before.scores.score), n(now.scores.overall ?? now.scores.score)],
  ];
  host.className = '';
  host.innerHTML = `
    ${feedMismatch(before, now) ? `<div class="compare-warning">${t('compare.feed_warning')}: <strong>${esc(before.feedName)}</strong> ≠ <strong>${esc(now.feedName)}</strong></div>` : ''}
    ${dateRangeChanged(before, now) ? `<div class="compare-warning">${t('compare.date_warning')}</div>` : ''}
    ${configChanged(before, now) ? `<div class="compare-warning">${t('compare.config_warning')}</div>` : ''}
    ${before.legacy ? `<div class="compare-warning">${t('compare.legacy_warning')}</div>` : ''}
    <div class="compare-runs"><span class="run-dot old"></span><strong>${t('compare.before')}</strong> · v${esc(before.appVersion)} · ${esc(dateLabel(before))}<span class="run-vs">vs</span><span class="run-dot now"></span><strong>${t('compare.now')}</strong> · v${esc(now.appVersion)} · ${esc(dateLabel(now))}</div>
    <div class="compare-summary">
      ${cards.map(([key, old, cur]) => { const d = Number(cur) - Number(old); return `<div class="compare-card"><span>${t(String(key))}</span><strong>${Number(old).toFixed(1)} <i>→</i> ${Number(cur).toFixed(1)}</strong><em class="${deltaClass(-d)}">${scoreDelta(d)}</em></div>`; }).join('')}
      <div class="compare-card"><span>${t('compare.total_notice')}</span><strong>${nf.format(before.noticeTotal)} <i>→</i> ${nf.format(now.noticeTotal)}</strong><em class="${deltaClass(totalDelta)}">${totalDelta > 0 ? '+' : ''}${nf.format(totalDelta)}</em></div>
      <div class="compare-statuses">${status('fixed', counts('fixed'))}${status('decreased', counts('decreased'))}${status('new', counts('new'))}${status('increased', counts('increased'))}</div>
    </div>
    <section class="compare-section"><div class="compare-section-title">${t('compare.scores')}</div><div class="score-deltas">
      ${SCORE_KEYS.map(key => scoreBar(key, before, now)).join('')}
    </div></section>
    ${distributionSection(before, now)}
    <section class="compare-section"><div class="compare-table-head"><div><strong>${t('compare.rule_changes')}</strong><div class="compare-filters">
      ${(['all','fixed','decreased','increased','new','same'] as const).map((k, i) => `<button class="compare-filter${i === 0 ? ' active' : ''}" data-filter="${k}">${t(`compare.${k}`)}</button>`).join('')}
      </div></div><input id="compare-search" type="search" placeholder="${t('compare.search')}"></div>
      <div class="compare-table-wrap"><table class="compare-table"><thead><tr><th>${t('compare.status')}</th><th>${t('compare.rule')}</th><th>${t('compare.description')}</th><th>${t('compare.before')}</th><th>${t('compare.now')}</th><th>${t('compare.delta')}</th><th>${t('compare.change')}</th></tr></thead><tbody id="compare-rows"></tbody></table></div>
    </section>
    ${feedStructure(before, now)}
    <div class="compare-actions"><button id="compare-csv" class="btn btn-secondary">${t('compare.download_csv')}</button></div>`;

  let filter = 'all'; let search = '';
  const draw = () => drawRows(host.querySelector<HTMLElement>('#compare-rows')!, changes.filter(c => (filter === 'all' || c.kind === filter) && `${c.id} ${c.title ?? ''}`.toLowerCase().includes(search)));
  draw();
  host.querySelectorAll<HTMLButtonElement>('.compare-filter').forEach(btn => btn.addEventListener('click', () => {
    host.querySelectorAll('.compare-filter').forEach(b => b.classList.remove('active')); btn.classList.add('active'); filter = btn.dataset['filter']!; draw();
  }));
  host.querySelector<HTMLInputElement>('#compare-search')!.addEventListener('input', e => { search = (e.currentTarget as HTMLInputElement).value.toLowerCase(); draw(); });
  host.querySelector('#compare-csv')!.addEventListener('click', () => downloadCsv(changes, before, now));
}

function status(kind: ChangeKind, count: number): string {
  const icon: Record<ChangeKind, string> = { fixed: '✓', decreased: '↓', increased: '↑', new: '+', same: '−' };
  return `<div class="status-${kind}"><b>${icon[kind]}</b><strong>${count}</strong><span>${t(`compare.${kind}`)}</span></div>`;
}
// 'overall' alanı Golden /3'te eklendi; legacy /2'de yalnız 'score' var → alias.
function scoreVal(run: GoldenRun, key: typeof SCORE_KEYS[number]): number {
  return key === 'overall' ? n(run.scores.overall ?? run.scores.score) : n(run.scores[key]);
}
function scoreBar(key: typeof SCORE_KEYS[number], before: GoldenRun, now: GoldenRun): string {
  const old = scoreVal(before, key); const cur = scoreVal(now, key); const d = cur - old;
  return `<div class="score-delta"><strong>${t(`compare.score.${key}`)}</strong><div><span style="width:${Math.max(1, old)}%"></span><em>${old.toFixed(1)}</em></div><div class="current"><span style="width:${Math.max(1, cur)}%"></span><em>${cur.toFixed(1)}</em></div><small class="${deltaClass(-d)}">${scoreDelta(d)}</small></div>`;
}
function drawRows(body: HTMLElement, changes: RuleChange[]): void {
  body.innerHTML = changes.map(c => {
    const localized = t(`rule.${c.id}`); const title = localized === c.id ? (c.title ?? t('compare.no_metadata')) : localized;
    return `<tr><td><span class="change-pill ${c.kind}">${t(`compare.${c.kind}`)}</span></td><td><code>${esc(c.id)}</code></td><td>${esc(title)}</td><td>${nf.format(c.before)}</td><td>${nf.format(c.now)}</td><td class="${deltaClass(c.delta)}">${c.delta > 0 ? '+' : ''}${nf.format(c.delta)}</td><td class="${deltaClass(c.delta)}">${c.percent == null ? t('compare.new') : sf.format(c.percent) + '%'}</td></tr>`;
  }).join('') || `<tr><td colspan="7" class="compare-no-rows">${t('compare.no_rows')}</td></tr>`;
}
function distributionSection(before: GoldenRun, now: GoldenRun): string {
  // beforeKnown=false → "Karşılaştırılan" verisi yok (legacy Golden sınıf kırılımı saklamaz):
  // sayım yerine "—" gösterilir, böylece her sınıf yanlışlıkla "yeni artmış" gibi görünmez.
  const head = `<thead><tr><th></th><th>${t('compare.before')}</th><th></th><th>${t('compare.now')}</th><th>${t('compare.delta')}</th></tr></thead>`;
  const table = (title: string, keys: string[], oldMap: Record<string, number>, newMap: Record<string, number>, beforeKnown: boolean) => `<div><strong>${title}</strong><table>${head}<tbody>${keys.map(key => {
    const cur = newMap[key] ?? 0;
    if (!beforeKnown) return `<tr><td>${key}</td><td class="same">—</td><td>→</td><td>${nf.format(cur)}</td><td class="same">—</td></tr>`;
    const old = oldMap[key] ?? 0; const d = cur - old;
    return `<tr><td>${key}</td><td>${nf.format(old)}</td><td>→</td><td>${nf.format(cur)}</td><td class="${deltaClass(d)}">${d > 0 ? '+' : ''}${nf.format(d)}</td></tr>`;
  }).join('')}</tbody></table></div>`;
  return `<section class="compare-section"><div class="compare-section-title">${t('compare.distribution')}</div>${before.legacy ? `<p class="compare-note">ⓘ ${t('compare.legacy_class_note')}</p>` : ''}<div class="compare-distributions">
    ${table(t('compare.by_severity'), ['CRITICAL','HIGH','MEDIUM','LOW','INFO'], before.severityCounts, now.severityCounts, true)}
    ${table(t('compare.by_class'), ['SPEC','INTEROP','QUALITY','ANALYTICS'], before.classCounts, now.classCounts, !before.legacy)}
  </div></section>`;
}
function feedStructure(before: GoldenRun, now: GoldenRun): string {
  const metrics = [['trip_count','Trips'],['stop_count','Stops']];
  const files = [['stop_times.txt','Stop Times'],['calendar_dates.txt','Calendar Dates']];
  const items = [...metrics.map(([k,l]) => [l, n(before.metrics[k]), n(now.metrics[k])]), ...files.map(([k,l]) => [l, before.files[k]?.rows ?? 0, now.files[k]?.rows ?? 0])];
  const range = (run: GoldenRun, prefix: string) => `${formatYmd(run.metrics[`${prefix}_start_date`])} → ${formatYmd(run.metrics[`${prefix}_end_date`])}`;
  const density = noticeDensity;
  const oldTrips = n(before.metrics.trip_count), newTrips = n(now.metrics.trip_count);
  const oldSt = before.files['stop_times.txt']?.rows ?? 0, newSt = now.files['stop_times.txt']?.rows ?? 0;
  return `<section class="compare-section"><div class="compare-section-title">${t('compare.feed_structure')}</div><div class="feed-deltas">${items.map(([label, old, cur]) => { const d = Number(cur)-Number(old); return `<div><span>${label}</span><strong>${nf.format(Number(old))} → ${nf.format(Number(cur))}</strong><em class="${deltaClass(-d)}">${d > 0 ? '+' : ''}${nf.format(d)}</em></div>`; }).join('')}</div>
    <div class="date-deltas"><div><span>${t('compare.feed_range')}</span><strong>${range(before, 'feed')}</strong><i>→</i><strong>${range(now, 'feed')}</strong></div><div><span>${t('compare.service_range')}</span><strong>${range(before, 'service')}</strong><i>→</i><strong>${range(now, 'service')}</strong></div></div>
    <div class="density-deltas"><div><span>${t('compare.per_1000_trips')}</span><strong>${density(before,oldTrips,1000).toFixed(1)} → ${density(now,newTrips,1000).toFixed(1)}</strong></div><div><span>${t('compare.per_100k_stop_times')}</span><strong>${density(before,oldSt,100000).toFixed(1)} → ${density(now,newSt,100000).toFixed(1)}</strong></div></div>
    <p class="compare-note">ⓘ ${t('compare.feed_note')}</p></section>`;
}
function formatYmd(value: unknown): string {
  const s = String(value ?? ''); return /^\d{8}$/.test(s) ? `${s.slice(0,4)}-${s.slice(4,6)}-${s.slice(6)}` : '—';
}
function downloadCsv(changes: RuleChange[], before: GoldenRun, now: GoldenRun): void {
  const q = (v: unknown) => `"${String(v).replace(/"/g, '""')}"`;
  const rows = [['status','rule_id','title','before','now','delta','percent'], ...changes.map(c => [c.kind,c.id,c.title ?? '',c.before,c.now,c.delta,c.percent ?? ''])];
  const a = document.createElement('a'); a.href = URL.createObjectURL(new Blob(['\ufeff' + rows.map(r => r.map(q).join(',')).join('\r\n')], { type: 'text/csv' }));
  a.download = `${before.feedName || now.feedName || 'gtfs'}-comparison.csv`; a.click(); URL.revokeObjectURL(a.href);
}
