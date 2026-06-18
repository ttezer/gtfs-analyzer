import type { ValidationResult } from '../types';
import { SEVERITY_TR, RULE_CLASS_TR, t, tMsg } from '../i18n';
import { augmentRouteLabels } from './fix';

// Bayt → insan-okur boyut (yerel ondalık ayraçla). Tahmini dışa aktarım boyutu için.
function formatBytes(b: number): string {
  if (b < 1024) return `${b} B`;
  if (b < 1024 * 1024) return `${Math.round(b / 1024)} KB`;
  const mb = b / (1024 * 1024);
  return `${new Intl.NumberFormat(undefined, { maximumFractionDigits: 1 }).format(mb)} MB`;
}

const byteLen = (s: string): number => new TextEncoder().encode(s).length;

// Renkli daire içinde satır-ikonu (currentColor). Kart/stat başlıkları için.
const ICON: Record<string, string> = {
  doc:   '<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><path d="M14 2H6a2 2 0 0 0-2 2v16a2 2 0 0 0 2 2h12a2 2 0 0 0 2-2V8z"/><path d="M14 2v6h6"/><path d="M8 13h8M8 17h8"/></svg>',
  chart: '<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><path d="M3 3v18h18"/><rect x="7" y="11" width="3" height="6"/><rect x="12" y="7" width="3" height="10"/><rect x="17" y="13" width="3" height="4"/></svg>',
  shield:'<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><path d="M12 22s8-4 8-10V5l-8-3-8 3v7c0 6 8 10 8 10z"/></svg>',
  csv:   '<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><path d="M14 2H6a2 2 0 0 0-2 2v16a2 2 0 0 0 2 2h12a2 2 0 0 0 2-2V8z"/><path d="M14 2v6h6"/><path d="M8 13h2M8 17h2M14 13h2M14 17h2"/></svg>',
  braces:'<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><path d="M8 3H7a2 2 0 0 0-2 2v4a2 2 0 0 1-2 2 2 2 0 0 1 2 2v4a2 2 0 0 0 2 2h1"/><path d="M16 3h1a2 2 0 0 1 2 2v4a2 2 0 0 0 2 2 2 2 0 0 0-2 2v4a2 2 0 0 1-2 2h-1"/></svg>',
  print: '<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><path d="M6 9V2h12v7"/><path d="M6 18H4a2 2 0 0 1-2-2v-5a2 2 0 0 1 2-2h16a2 2 0 0 1 2 2v5a2 2 0 0 1-2 2h-2"/><rect x="6" y="14" width="12" height="8"/></svg>',
};

export function renderExport(
  root: HTMLElement,
  result: ValidationResult,
  fileName: string,
): void {
  // EN/JA parite: route-scoped bulgu mesajlarının {route_label}'ını doldur (R2 ile aynı).
  augmentRouteLabels(result.notices, result.name_index);

  // İçerikleri bir kez üret → hem tahmini boyut hem indirme bunları kullanır (çift iş yok).
  const htmlStr = buildReportHtml(result, fileName);
  const csvStr  = buildCsv(result);
  const jsonStr = JSON.stringify(result, null, 2);

  const { r5 } = result.reports;
  const fmtScore = (n: number) => new Intl.NumberFormat(undefined, { minimumFractionDigits: 1, maximumFractionDigits: 1 }).format(n);
  const total = new Intl.NumberFormat(undefined).format(result.notices.length);

  const card = (id: string, icon: string, cls: string, title: string, badge: string, desc: string, btn: string, primary: boolean, size: string) => `
    <div class="exp-card${primary ? ' exp-card-rec' : ''}">
      <div class="exp-card-head">
        <span class="exp-card-icon exp-icon-${cls}">${icon}</span>
        <span class="exp-card-title">${title}</span>
        ${badge ? `<span class="exp-rec-badge">${badge}</span>` : ''}
      </div>
      <p class="exp-card-desc">${desc}</p>
      <button id="${id}" class="btn ${primary ? 'btn-primary' : 'btn-secondary'} exp-card-btn">${btn}</button>
      <p class="exp-card-size"><span class="exp-i">ⓘ</span> ${t('export.est_size')} <strong>${size}</strong></p>
    </div>`;

  root.innerHTML = `
    <section class="export-page">
      <div class="card">
        <h2 class="exp-title">${t('export.title')}</h2>
        <p class="exp-source">${t('export.source')} <code>${escHtml(fileName)}</code></p>

        <div class="exp-stats">
          <div class="exp-stat">
            <span class="exp-stat-icon exp-icon-doc">${ICON.doc}</span>
            <span class="exp-stat-text"><span class="exp-stat-label">${t('export.html.total')}</span><span class="exp-stat-value">${total}</span></span>
          </div>
          <div class="exp-stat">
            <span class="exp-stat-icon exp-icon-green">${ICON.chart}</span>
            <span class="exp-stat-text"><span class="exp-stat-label">${t('export.html.pub_score')}</span><span class="exp-stat-value exp-val-pub">${fmtScore(r5.pub_score)}</span></span>
          </div>
          <div class="exp-stat">
            <span class="exp-stat-icon exp-icon-amber">${ICON.shield}</span>
            <span class="exp-stat-text"><span class="exp-stat-label">${t('export.html.qual_score')}</span><span class="exp-stat-value exp-val-qual">${fmtScore(r5.score)}</span></span>
          </div>
        </div>

        <h3 class="exp-formats-title">${t('export.formats.title')}</h3>
        <p class="exp-formats-sub">${t('export.formats.subtitle')}</p>

        <div class="exp-grid">
          ${card('btn-export-html', ICON.doc,   'doc', t('export.card.html.title'), t('export.recommended'), t('export.card.html.desc'), t('export.download'), true,  formatBytes(byteLen(htmlStr)))}
          ${card('btn-export-csv',  ICON.csv,   'csv', t('export.card.csv.title'),  '',                       t('export.card.csv.desc'),  t('export.download'), false, formatBytes(byteLen(csvStr)))}
          ${card('btn-export-json', ICON.braces,'json',t('export.card.json.title'), '',                       t('export.card.json.desc'), t('export.download'), false, formatBytes(byteLen(jsonStr)))}
          ${card('btn-export-pdf',  ICON.print, 'pdf', t('export.card.pdf.title'),  '',                       t('export.card.pdf.desc'),  t('export.print'),    false, t('export.size.browser'))}
        </div>

        <div class="exp-note"><span class="exp-i">ⓘ</span> <strong>${t('export.note.label')}</strong> ${t('export.note.text')}</div>
      </div>
    </section>`;

  const dl = (s: string, mime: string, ext: string) =>
    triggerDownload(new Blob([s], { type: mime }), fileName.replace(/\.zip$/i, ext));

  root.querySelector('#btn-export-html')!.addEventListener('click', () => dl(htmlStr, 'text/html; charset=utf-8', '-report.html'));
  root.querySelector('#btn-export-csv')!.addEventListener('click',  () => dl('﻿' + csvStr, 'text/csv; charset=utf-8', '-report.csv'));
  root.querySelector('#btn-export-json')!.addEventListener('click', () => dl(jsonStr, 'application/json', '-report.json'));
  root.querySelector('#btn-export-pdf')!.addEventListener('click', () => printHtml(htmlStr));
}

function csvCell(v: string | number | null | undefined): string {
  const s = v == null ? '' : String(v);
  const safe = /^[=+\-@]/.test(s) ? `'${s}` : s;
  return safe.includes(',') || safe.includes('"') || safe.includes('\n')
    ? `"${safe.replace(/"/g, '""')}"`
    : safe;
}

// CSV içeriğini üretir (BOM'suz; indirme sırasında BOM eklenir).
function buildCsv(result: ValidationResult): string {
  const header = [
    t('export.csv.rule'), t('export.csv.severity'), t('export.csv.class'),
    t('export.csv.message'), t('export.csv.entity_id'), t('export.csv.file'), t('export.csv.service'), t('export.csv.row'),
  ];
  const rows = result.notices.map(n => [
    n.rule_id,
    SEVERITY_TR[n.severity] ?? n.severity,
    RULE_CLASS_TR[n.rule_class] ?? n.rule_class,
    tMsg(n),
    n.entity_id ?? '',
    n.file ?? '',
    n.service_id ?? '',
    n.line != null ? String(n.line) : '',
  ].map(csvCell).join(','));

  return [header.map(csvCell).join(','), ...rows].join('\r\n');
}

function buildReportHtml(result: ValidationResult, fileName: string): string {
  const { r1, r5 } = result.reports;
  const publishLabel = r1.publishable
    ? (r1.conditional ? t('export.html.publishable_cond') : t('export.html.publishable_ok'))
    : t('export.html.publishable_blocked');

  const noticeRows = result.notices.map(n => `
    <tr>
      <td>${escHtml(n.rule_id)}</td>
      <td>${SEVERITY_TR[n.severity]}</td>
      <td>${RULE_CLASS_TR[n.rule_class]}</td>
      <td>${escHtml(tMsg(n))}</td>
      <td>${n.entity_id ? escHtml(n.entity_id) : ''}</td>
      <td>${n.file ? escHtml(n.file) : ''}</td>
      <td>${n.service_id ? escHtml(n.service_id) : ''}</td>
      <td>${n.line ?? ''}</td>
    </tr>`).join('');

  const breakdown = t('export.html.breakdown', {
    spec    : r5.spec_score.toFixed(1),
    interop : r5.interop_score.toFixed(1),
    quality : r5.quality_score.toFixed(1),
    analytics: r5.analytics_score.toFixed(1),
  });

  return `<!DOCTYPE html>
<html lang="tr">
<head>
  <meta charset="UTF-8"/>
  <title>${t('export.html.doc_title')} - ${escHtml(fileName)}</title>
  <style>
    body { font-family: system-ui, sans-serif; max-width: 1100px; margin: 0 auto; padding: 2rem; color: #1e293b; }
    h1 { font-size: 1.4rem; margin-bottom: .5rem; }
    .summary { display: flex; gap: 2rem; flex-wrap: wrap; margin: 1rem 0 1.5rem; }
    .kpi { display: flex; flex-direction: column; }
    .kpi-value { font-size: 1.8rem; font-weight: 700; }
    .kpi-label { font-size: .8rem; color: #64748b; }
    .score-breakdown { margin-top: .3rem; font-size: .8rem; color: #64748b; }
    h2 { font-size: 1rem; margin: 1.5rem 0 .5rem; }
    table { border-collapse: collapse; width: 100%; font-size: 0.82rem; }
    th, td { border: 1px solid #e2e8f0; padding: 0.35rem 0.6rem; text-align: left; vertical-align: top; }
    th { background: #f8fafc; font-weight: 600; }
    @media print { body { padding: 1rem; } }
  </style>
</head>
<body>
  <h1>${t('export.html.h1')}</h1>
  <p style="color:#64748b;font-size:.9rem">${t('export.html.source')} ${escHtml(fileName)}</p>

  <div class="summary">
    <div class="kpi">
      <span class="kpi-value">${publishLabel}</span>
      <span class="kpi-label">${t('export.html.publishability')}</span>
    </div>
    <div class="kpi">
      <span class="kpi-value">${r5.score.toFixed(1)}<span style="font-size:1rem;font-weight:400">/100</span></span>
      <span class="kpi-label">${t('export.html.qual_score')}</span>
      <span class="score-breakdown">${breakdown}</span>
    </div>
    <div class="kpi">
      <span class="kpi-value">${r5.pub_score.toFixed(1)}<span style="font-size:1rem;font-weight:400">/100</span></span>
      <span class="kpi-label">${t('export.html.pub_score')}</span>
    </div>
    <div class="kpi">
      <span class="kpi-value">${result.notices.length}</span>
      <span class="kpi-label">${t('export.html.total')}</span>
    </div>
  </div>

  <h2>${t('export.html.h2_findings')}</h2>
  <table>
    <thead><tr>
      <th>${t('export.html.th.rule')}</th>
      <th>${t('export.html.th.severity')}</th>
      <th>${t('export.html.th.class')}</th>
      <th>${t('export.html.th.message')}</th>
      <th>${t('export.html.th.entity_id')}</th>
      <th>${t('export.html.th.file')}</th>
      <th>${t('export.html.th.service')}</th>
      <th>${t('export.html.th.row')}</th>
    </tr></thead>
    <tbody>${noticeRows}</tbody>
  </table>

  <footer style="margin-top:2rem;padding-top:1rem;border-top:1px solid #e2e8f0;text-align:center;font-size:.78rem;color:#64748b">
    <a href="https://github.com/ttezer/gtfs-analyzer" style="color:#64748b">github.com/ttezer/gtfs-analyzer</a> · MIT License
  </footer>
</body>
</html>`;
}

// Önceden üretilmiş raporu yazdırma penceresinde açar (PDF = tarayıcı yazdırma diyaloğu).
function printHtml(html: string): void {
  const win = window.open('', '_blank');
  if (!win) { alert(t('export.popup_blocked')); return; }
  win.document.write(html);
  win.document.close();
  win.focus();
  win.print();
}

function triggerDownload(blob: Blob, name: string): void {
  const url = URL.createObjectURL(blob);
  const a = document.createElement('a');
  a.href = url;
  a.download = name;
  a.click();
  URL.revokeObjectURL(url);
}

function escHtml(s: string): string {
  return s.replace(/&/g, '&amp;').replace(/</g, '&lt;').replace(/>/g, '&gt;');
}
