import type { ValidationResult } from '../types';
import { SEVERITY_TR, RULE_CLASS_TR } from '../i18n';

export function renderExport(
  root: HTMLElement,
  result: ValidationResult,
  fileName: string,
): void {
  root.innerHTML = `
    <section class="page-export-wide">
      <div class="card">
        <h2>Dışa Aktar</h2>
        <p class="export-filename">Kaynak: <code>${escHtml(fileName)}</code></p>
        <div class="export-actions">
          <button id="btn-export-html" class="btn btn-primary">HTML Rapor İndir</button>
          <button id="btn-export-csv"  class="btn btn-secondary">CSV İndir</button>
          <button id="btn-export-json" class="btn btn-secondary">JSON Ham Veri İndir</button>
          <button id="btn-export-pdf"  class="btn btn-secondary">PDF Olarak Yazdır</button>
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
  return s.includes(',') || s.includes('"') || s.includes('\n')
    ? `"${s.replace(/"/g, '""')}"`
    : s;
}

function exportCsv(result: ValidationResult, fileName: string): void {
  const header = ['Kural', 'Önem', 'Sınıf', 'Mesaj', 'Varlık ID', 'Dosya', 'Satır'];
  const rows = result.notices.map(n => [
    n.rule_id,
    SEVERITY_TR[n.severity] ?? n.severity,
    RULE_CLASS_TR[n.rule_class] ?? n.rule_class,
    n.message,
    n.entity_id ?? '',
    n.file ?? '',
    n.line != null ? String(n.line) : '',
  ].map(csvCell).join(','));

  const bom = '﻿'; // Excel UTF-8 BOM
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
    ? (r1.conditional ? 'Koşullu Yayına Uygun' : 'Yayına Uygun')
    : 'Yayınlanması Tavsiye Edilmez';

  const noticeRows = result.notices.map(n => `
    <tr>
      <td>${escHtml(n.rule_id)}</td>
      <td>${SEVERITY_TR[n.severity]}</td>
      <td>${RULE_CLASS_TR[n.rule_class]}</td>
      <td>${escHtml(n.message)}</td>
      <td>${n.entity_id ? escHtml(n.entity_id) : ''}</td>
      <td>${n.file ? escHtml(n.file) : ''}</td>
      <td>${n.line ?? ''}</td>
    </tr>`).join('');

  return `<!DOCTYPE html>
<html lang="tr">
<head>
  <meta charset="UTF-8"/>
  <title>GTFS Doğrulama Raporu - ${escHtml(fileName)}</title>
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
  <h1>GTFS Doğrulama Raporu</h1>
  <p style="color:#64748b;font-size:.9rem">Kaynak: ${escHtml(fileName)}</p>

  <div class="summary">
    <div class="kpi">
      <span class="kpi-value">${publishLabel}</span>
      <span class="kpi-label">Yayınlanabilirlik</span>
    </div>
    <div class="kpi">
      <span class="kpi-value">${r5.score.toFixed(1)}<span style="font-size:1rem;font-weight:400">/100</span></span>
      <span class="kpi-label">Kalite Skoru</span>
      <span class="score-breakdown">GTFS Geçerliliği ${r5.spec_score.toFixed(1)} · GTFS Uyumluluğu ${r5.interop_score.toFixed(1)} · GTFS Kalitesi ${r5.quality_score.toFixed(1)} · GTFS Analitiği ${r5.analytics_score.toFixed(1)}</span>
    </div>
    <div class="kpi">
      <span class="kpi-value">${r5.pub_score.toFixed(1)}<span style="font-size:1rem;font-weight:400">/100</span></span>
      <span class="kpi-label">Yayın Skoru</span>
    </div>
    <div class="kpi">
      <span class="kpi-value">${result.notices.length}</span>
      <span class="kpi-label">Toplam Bulgu</span>
    </div>
  </div>

  <h2>Bulgular</h2>
  <table>
    <thead><tr>
      <th>Kural</th><th>Önem</th><th>Sınıf</th><th>Mesaj</th>
      <th>Varlık ID</th><th>Dosya</th><th>Satır</th>
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
  if (!win) { alert('Açılır pencere engellendi. Tarayıcı ayarlarını kontrol edin.'); return; }
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
