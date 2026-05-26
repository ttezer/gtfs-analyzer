import type { ValidationResult, R5Report, FeedMetrics } from '../types';

export function renderDomain(root: HTMLElement, result: ValidationResult): void {
  const { r1, r5 } = result.reports;
  const metrics = result.metrics;

  const sevCounts = countBySeverity(result);

  root.innerHTML = `
    <div class="report-page">
      ${renderScoreRow(r5, sevCounts)}
      ${renderR1Card(r1, result)}
      ${renderSubScores(r5)}
      ${renderMetrics(metrics)}
    </div>`;
}

// ── İki büyük skor kartı ──────────────────────────────────────────────────────

function renderScoreRow(r5: R5Report, c: SevCount): string {
  const pubColor   = scoreColor(r5.pub_score);
  const qualColor  = scoreColor(r5.score);

  return `
    <div class="rpt-score-row">
      <div class="rpt-score-card">
        <span class="rpt-score-label">Yayın Skoru</span>
        <span class="rpt-score-val" style="color:${pubColor}">${r5.pub_score.toFixed(1)}<span class="rpt-score-max">/100</span></span>
        <div class="rpt-score-track"><div class="rpt-score-fill" style="width:${r5.pub_score.toFixed(1)}%;background:${pubColor}"></div></div>
        <span class="rpt-score-hint">SPEC + INTEROP blocker temizliği</span>
      </div>
      <div class="rpt-score-card">
        <span class="rpt-score-label">Kalite Skoru</span>
        <span class="rpt-score-val" style="color:${qualColor}">${r5.score.toFixed(1)}<span class="rpt-score-max">/100</span></span>
        <div class="rpt-score-track"><div class="rpt-score-fill" style="width:${r5.score.toFixed(1)}%;background:${qualColor}"></div></div>
        <span class="rpt-score-hint">Tüm sınıfların ağırlıklı ortalaması</span>
      </div>
      ${renderPieCard(c)}
    </div>`;
}

function scoreColor(v: number): string {
  return v >= 80 ? '#16a34a' : v >= 60 ? '#d97706' : '#dc2626';
}

// ── R1 yayınlanabilirlik kartı ─────────────────────────────────────────────────

function renderR1Card(r1: ValidationResult['reports']['r1'], result: ValidationResult): string {
  const noticeMap = new Map(result.notices.map(n => [n.id, n]));

  if (!r1.publishable) {
    const blockers = r1.blocker_notice_ids
      .slice(0, 5)
      .map(id => {
        const n = noticeMap.get(id);
        return n ? `<li><code>${escHtml(n.rule_id)}</code> — ${escHtml(n.title || n.message.slice(0, 80))}</li>` : '';
      }).join('');
    return `
      <div class="rpt-r1 rpt-r1-blocked">
        <div class="rpt-r1-icon">✕</div>
        <div class="rpt-r1-body">
          <strong>Yayınlanması Tavsiye Edilmez</strong>
          <span>${r1.blocker_notice_ids.length} kritik engelleyici bulgu mevcut — feed yayına alınamaz.</span>
          ${blockers ? `<ul class="rpt-r1-list">${blockers}</ul>` : ''}
        </div>
      </div>`;
  }

  if (r1.conditional) {
    const conds = r1.conditional_blocker_notice_ids
      .slice(0, 5)
      .map(id => {
        const n = noticeMap.get(id);
        return n ? `<li><code>${escHtml(n.rule_id)}</code> — ${escHtml(n.title || n.message.slice(0, 80))}</li>` : '';
      }).join('');
    return `
      <div class="rpt-r1 rpt-r1-conditional">
        <div class="rpt-r1-icon">⚠</div>
        <div class="rpt-r1-body">
          <strong>Koşullu Yayına Uygun</strong>
          <span>${r1.conditional_blocker_notice_ids.length} koşullu engelleyici mevcut — bazı uygulamalar bu feed'i reddedebilir.</span>
          ${conds ? `<ul class="rpt-r1-list">${conds}</ul>` : ''}
        </div>
      </div>`;
  }

  return `
    <div class="rpt-r1 rpt-r1-ok">
      <div class="rpt-r1-icon">✓</div>
      <div class="rpt-r1-body">
        <strong>Yayına Uygun</strong>
        <span>Engelleyici bulgu yok.</span>
      </div>
    </div>`;
}

// ── Alt skor breakdown ────────────────────────────────────────────────────────

function renderSubScores(r5: R5Report): string {
  const cats = [
    { label: 'GTFS Geçerliliği',  weight: '%40', value: r5.spec_score,      color: '#1d4ed8', bg: '#eff6ff' },
    { label: 'GTFS Uyumluluğu',   weight: '%30', value: r5.interop_score,   color: '#b45309', bg: '#fef3c7' },
    { label: 'GTFS Kalitesi',     weight: '%20', value: r5.quality_score,   color: '#15803d', bg: '#f0fdf4' },
    { label: 'GTFS Analitiği',    weight: '%10', value: r5.analytics_score, color: '#6d28d9', bg: '#f5f3ff' },
  ];

  const chips = cats.map(c => `
    <div class="rpt-sub-chip" style="background:${c.bg};color:${c.color}">
      <div class="rpt-sub-top">
        <span class="rpt-sub-label">${c.label}</span>
        <span class="rpt-sub-weight">${c.weight}</span>
      </div>
      <span class="rpt-sub-val">${c.value.toFixed(1)}</span>
    </div>`).join('');

  return `
    <div>
      <div class="rpt-section-title" style="margin-bottom:.4rem">Kalite Skoru Bileşenleri</div>
      <div class="rpt-sub-row">${chips}</div>
    </div>`;
}

// ── Pasta grafiği (donut) ─────────────────────────────────────────────────────

type SevCount = { CRITICAL: number; HIGH: number; MEDIUM: number; LOW: number; INFO: number };

const PIE_SEGS = [
  { key: 'CRITICAL', label: 'Kritik',  color: '#dc2626' },
  { key: 'HIGH',     label: 'Yüksek',  color: '#d97706' },
  { key: 'MEDIUM',   label: 'Orta',    color: '#2563eb' },
  { key: 'LOW',      label: 'Düşük',   color: '#16a34a' },
  { key: 'INFO',     label: 'Bilgi',   color: '#9ca3af' },
] as const;

function renderPieCard(c: SevCount): string {
  const total = c.CRITICAL + c.HIGH + c.MEDIUM + c.LOW + c.INFO;
  if (total === 0) return '';

  // r=15.9155 → circumference≈100, stroke-dasharray percentages map 1:1
  const R = 15.9155;
  let cumPct = 0;
  const circles = PIE_SEGS
    .filter(s => (c as Record<string, number>)[s.key] > 0)
    .map(s => {
      const pct = (c as Record<string, number>)[s.key] / total * 100;
      const dashOffset = 25 - cumPct; // 25 = start at 12 o'clock
      cumPct += pct;
      return `<circle cx="18" cy="18" r="${R}" fill="none" stroke="${s.color}" stroke-width="3.5"` +
             ` stroke-dasharray="${pct.toFixed(3)} ${(100 - pct).toFixed(3)}" stroke-dashoffset="${dashOffset.toFixed(3)}"/>`;
    }).join('');

  const legend = PIE_SEGS
    .filter(s => (c as Record<string, number>)[s.key] > 0)
    .map(s => {
      const count = (c as Record<string, number>)[s.key];
      const pct   = (count / total * 100).toFixed(1);
      return `<div class="pie-leg-row">` +
               `<span class="pie-leg-dot" style="background:${s.color}"></span>` +
               `<span class="pie-leg-label">${s.label}</span>` +
               `<span class="pie-leg-count">${count.toLocaleString('tr-TR')}</span>` +
               `<span class="pie-leg-pct">${pct}%</span>` +
             `</div>`;
    }).join('');

  return `
    <div class="rpt-score-card rpt-pie-card">
      <span class="rpt-score-label">Hata Dağılımı <span class="rpt-score-total">${total.toLocaleString('tr-TR')}</span></span>
      <div class="rpt-pie-body">
        <svg viewBox="0 0 36 36" class="rpt-pie-svg" aria-hidden="true">
          <circle cx="18" cy="18" r="${R}" fill="none" class="pie-track" stroke-width="3.5"/>
          ${circles}
          <text x="18" y="16" text-anchor="middle" dominant-baseline="middle"
            font-size="5.5" font-weight="800" class="pie-center-num">${total.toLocaleString('tr-TR')}</text>
          <text x="18" y="22" text-anchor="middle" dominant-baseline="middle"
            font-size="2.8" class="pie-center-txt">hata</text>
        </svg>
        <div class="rpt-pie-legend">${legend}</div>
      </div>
    </div>`;
}

function countBySeverity(result: ValidationResult): SevCount {
  const c: SevCount = { CRITICAL: 0, HIGH: 0, MEDIUM: 0, LOW: 0, INFO: 0 };
  for (const n of result.notices) {
    if (n.severity in c) (c as any)[n.severity]++;
  }
  return c;
}


// ── Feed metrikleri ───────────────────────────────────────────────────────────

function renderMetrics(m: FeedMetrics): string {
  const items = [
    { label: 'Durak',           value: m.stop_count.toLocaleString('tr-TR') },
    { label: 'Hat',             value: m.route_count.toLocaleString('tr-TR') },
    { label: 'Sefer',           value: m.trip_count.toLocaleString('tr-TR') },
    { label: 'Güzergah',        value: m.shape_count.toLocaleString('tr-TR') },
    { label: 'Aktif Servis Günü', value: m.active_service_days.toLocaleString('tr-TR') },
    { label: 'Ort. Günlük Sefer', value: m.avg_daily_trips.toFixed(0) },
  ];

  const metricCards = items.map(it => `
    <div class="rpt-metric">
      <span class="rpt-metric-val">${it.value}</span>
      <span class="rpt-metric-label">${it.label}</span>
    </div>`).join('');

  const fileRows = m.file_stats.map(f => `
    <tr>
      <td><code>${escHtml(f.name)}</code></td>
      <td class="num">${f.rows.toLocaleString('tr-TR')}</td>
      <td class="num">${formatBytes(f.bytes)}</td>
    </tr>`).join('');

  return `
    <div class="card">
      <h3 class="rpt-section-title">Feed Metrikleri</h3>
      <div class="rpt-metrics-grid">${metricCards}</div>
      ${fileRows ? `
        <div class="table-scroll rpt-files">
          <table class="data-table">
            <thead><tr><th>Dosya</th><th class="num">Satır</th><th class="num">Boyut</th></tr></thead>
            <tbody>${fileRows}</tbody>
          </table>
        </div>` : ''}
    </div>`;
}

function formatBytes(b: number): string {
  if (b < 1024) return `${b} B`;
  if (b < 1024 * 1024) return `${(b / 1024).toFixed(1)} KB`;
  return `${(b / 1024 / 1024).toFixed(1)} MB`;
}

function escHtml(s: string): string {
  return s.replace(/&/g, '&amp;').replace(/</g, '&lt;').replace(/>/g, '&gt;');
}
