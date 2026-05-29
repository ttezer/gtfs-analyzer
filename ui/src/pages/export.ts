import type { ValidationResult } from '../types';
import { SEVERITY_TR, RULE_CLASS_TR, t, tMsg } from '../i18n';

export function renderExport(
  root: HTMLElement,
  result: ValidationResult,
  fileName: string,
): void {
  root.innerHTML = `
    <section class="page-export-wide">
      <div class="card">
        <h2>${t('export.title')}</h2>
        <p class="export-filename">${t('export.source')} <code>${escHtml(fileName)}</code></p>
        <div class="export-actions">
          <button id="btn-export-html" class="btn btn-primary">${t('export.html')}</button>
          <button id="btn-export-csv"  class="btn btn-secondary">${t('export.csv')}</button>
          <button id="btn-export-json" class="btn btn-secondary">${t('export.json')}</button>
          <button id="btn-export-pdf"  class="btn btn-secondary">${t('export.pdf')}</button>
        </div>
      </div>
    </section>`;

  root.querySelector('#btn-export-html')!.addEventListener('click', () => exportHtml(result, fileName));
  root.querySelector('#btn-export-csv')!.addEventListener('click',  () => exportCsv(result, fileName));
  root.querySelector('#btn-export-json')!.addEventListener('click', () => exportJson(result, fileName));
  root.querySelector('#btn-export-pdf')!.addEventListener('click', () => exportPdf(result, fileName));
}

function csvCell(v: string | number | null | undefined): string {
  const s = v == null ? '' : String(v);
  const safe = /^[=+\-@]/.test(s) ? `'${s}` : s;
  return safe.includes(',') || safe.includes('"') || safe.includes('\n')
    ? `"${safe.replace(/"/g, '""')}"`
    : safe;
}

function exportCsv(result: ValidationResult, fileName: string): void {
  const header = [
    t('export.csv.rule'), t('export.csv.severity'), t('export.csv.class'),
    t('export.csv.message'), t('export.csv.entity_id'), t('export.csv.file'), t('export.csv.row'),
  ];
  const rows = result.notices.map(n => [
    n.rule_id,
    SEVERITY_TR[n.severity] ?? n.severity,
    RULE_CLASS_TR[n.rule_class] ?? n.rule_class,
    tMsg(n),
    n.entity_id ?? '',
    n.file ?? '',
    n.line != null ? String(n.line) : '',
  ].map(csvCell).join(','));

  const bom = '﻿';
  const csv = bom + [header.map(csvCell).join(','), ...rows].join('\r\n');
  const blob = new Blob([csv], { type: 'text/csv; charset=utf-8' });
  triggerDownload(blob, fileName.replace(/\.zip$/i, '-report.csv'));
}

function exportJson(result: ValidationResult, fileName: string): void {
  const blob = new Blob([JSON.stringify(result, null, 2)], { type: 'application/json' });
  triggerDownload(blob, fileName.replace(/\.zip$/i, '-report.json'));
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
      <th>${t('export.html.th.row')}</th>
    </tr></thead>
    <tbody>${noticeRows}</tbody>
  </table>
</body>
</html>`;
}

function exportHtml(result: ValidationResult, fileName: string): void {
  const html = buildReportHtml(result, fileName);
  const blob = new Blob([html], { type: 'text/html; charset=utf-8' });
  triggerDownload(blob, fileName.replace(/\.zip$/i, '-report.html'));
}

function exportPdf(result: ValidationResult, fileName: string): void {
  const html = buildReportHtml(result, fileName);
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
