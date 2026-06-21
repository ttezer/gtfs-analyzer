import type { ValidationResult } from '../types';
import { SEVERITY_TR, RULE_CLASS_TR, t, tMsg, getLocale } from '../i18n';
import { augmentRouteLabels } from './fix';
import { getState } from '../state';
import { isThreaded } from '../wasm';
import { getLogs, getActions } from '../debug-buffer';

// Bayt → insan-okur boyut (yerel ondalık ayraçla). Tahmini dışa aktarım boyutu için.
function formatBytes(b: number): string {
  if (b < 1024) return `${b} B`;
  if (b < 1024 * 1024) return `${Math.round(b / 1024)} KB`;
  const mb = b / (1024 * 1024);
  return `${new Intl.NumberFormat(undefined, { maximumFractionDigits: 1 }).format(mb)} MB`;
}

const byteLen = (s: string): number => new TextEncoder().encode(s).length;

// Locale-duyarlı sayı biçimi (binlik ayraç tr "2.935.811" / en "2,935,811" / ja "2,935,811").
const fmtInt = (n: number): string => new Intl.NumberFormat(undefined).format(n);
const fmtDec1 = (n: number): string => new Intl.NumberFormat(undefined, { minimumFractionDigits: 1, maximumFractionDigits: 1 }).format(n);

// Renkli daire içinde satır-ikonu (currentColor). Kart/stat başlıkları için.
const ICON: Record<string, string> = {
  doc:   '<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><path d="M14 2H6a2 2 0 0 0-2 2v16a2 2 0 0 0 2 2h12a2 2 0 0 0 2-2V8z"/><path d="M14 2v6h6"/><path d="M8 13h8M8 17h8"/></svg>',
  chart: '<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><path d="M3 3v18h18"/><rect x="7" y="11" width="3" height="6"/><rect x="12" y="7" width="3" height="10"/><rect x="17" y="13" width="3" height="4"/></svg>',
  shield:'<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><path d="M12 22s8-4 8-10V5l-8-3-8 3v7c0 6 8 10 8 10z"/></svg>',
  csv:   '<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><path d="M14 2H6a2 2 0 0 0-2 2v16a2 2 0 0 0 2 2h12a2 2 0 0 0 2-2V8z"/><path d="M14 2v6h6"/><path d="M8 13h2M8 17h2M14 13h2M14 17h2"/></svg>',
  braces:'<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><path d="M8 3H7a2 2 0 0 0-2 2v4a2 2 0 0 1-2 2 2 2 0 0 1 2 2v4a2 2 0 0 0 2 2h1"/><path d="M16 3h1a2 2 0 0 1 2 2v4a2 2 0 0 0 2 2 2 2 0 0 0-2 2v4a2 2 0 0 1-2 2h-1"/></svg>',
  print: '<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><path d="M6 9V2h12v7"/><path d="M6 18H4a2 2 0 0 1-2-2v-5a2 2 0 0 1 2-2h16a2 2 0 0 1 2 2v5a2 2 0 0 1-2 2h-2"/><rect x="6" y="14" width="12" height="8"/></svg>',
};

interface SummaryData {
  file: string; ts: string; ruleTypes: number; pubScore: number; qualScore: number;
  // disp = gösterilen (kural başına 500 cap'li); real = gerçek (capped_totals ile düzeltilmiş).
  disp: { total: number; critical: number; high: number; medium: number; low: number; info: number };
  real: { total: number; critical: number; high: number; medium: number; low: number; info: number };
  capped: boolean;  // herhangi bir kural cap'e takıldı mı (real > disp)
  ruleCounts: Array<[string, number]>;  // [rule_id, gerçek sayı] — #21 diff birimi
}

// Özet snapshot. notices kural başına 500'de cap'li (WASM PER_RULE_CAP); gerçek totaller
// result.capped_totals'tan alınır → hem cap'li hem gerçek sütun verilir.
function buildSummaryData(result: ValidationResult, fileName: string, ts: string): SummaryData {
  const sevDisp: Record<string, number> = {};
  const rcDisp: Record<string, number> = {};
  const ruleSev: Record<string, string> = {};
  for (const n of result.notices) {
    sevDisp[n.severity] = (sevDisp[n.severity] ?? 0) + 1;
    rcDisp[n.rule_id] = (rcDisp[n.rule_id] ?? 0) + 1;
    ruleSev[n.rule_id] = n.severity;
  }
  const cap = result.capped_totals ?? {};
  const sevReal: Record<string, number> = {};
  const rcReal: Record<string, number> = {};
  for (const rid of Object.keys(rcDisp)) {
    const real = cap[rid] ?? rcDisp[rid];
    rcReal[rid] = real;
    sevReal[ruleSev[rid]] = (sevReal[ruleSev[rid]] ?? 0) + real;
  }
  const g = (o: Record<string, number>, k: string) => o[k] ?? 0;
  const dispTotal = result.notices.length;
  const realTotal = Object.values(rcReal).reduce((a, b) => a + b, 0);
  const { r5 } = result.reports;
  return {
    file: fileName, ts, ruleTypes: Object.keys(rcDisp).length,
    pubScore: r5.pub_score, qualScore: r5.score,
    disp: { total: dispTotal, critical: g(sevDisp, 'CRITICAL'), high: g(sevDisp, 'HIGH'), medium: g(sevDisp, 'MEDIUM'), low: g(sevDisp, 'LOW'), info: g(sevDisp, 'INFO') },
    real: { total: realTotal, critical: g(sevReal, 'CRITICAL'), high: g(sevReal, 'HIGH'), medium: g(sevReal, 'MEDIUM'), low: g(sevReal, 'LOW'), info: g(sevReal, 'INFO') },
    capped: realTotal > dispTotal,
    ruleCounts: Object.entries(rcReal).sort((a, b) => b[1] - a[1]),
  };
}

// Teşhis paketi (#14): YALNIZCA whitelist alanlar. Feed içeriği (durak adı/koordinat,
// notice mesajları) ASLA dahil edilmez — yalnızca metadata + agregat sayılar.
function buildDebugBundle(result: ValidationResult, fileName: string, fileSize: number, ts: string): string {
  const s = buildSummaryData(result, fileName, ts);
  const nav = navigator as Navigator & { hardwareConcurrency?: number };
  const bundle = {
    schema: 'gtfs-analyzer-debug/1',
    generated_at: ts,
    app: {
      version: typeof __APP_VERSION__ !== 'undefined' ? __APP_VERSION__ : 'dev',
      wasm_mode: isThreaded() ? 'threaded' : 'serial',
      user_agent: navigator.userAgent,
      language: getLocale(),
      hardware_concurrency: nav.hardwareConcurrency ?? null,
    },
    feed: { file_name: fileName, size_bytes: fileSize },
    config_delta: getState().configDelta || null,
    result_summary: {
      scores: {
        score: s.qualScore, pub_score: s.pubScore,
        spec: result.reports.r5.spec_score, interop: result.reports.r5.interop_score,
        quality: result.reports.r5.quality_score, analytics: result.reports.r5.analytics_score,
      },
      rule_types: s.ruleTypes,
      notice_total_capped: s.disp.total,
      notice_total_actual: s.real.total,
      by_severity_actual: { critical: s.real.critical, high: s.real.high, medium: s.real.medium, low: s.real.low, info: s.real.info },
      rule_counts_actual: Object.fromEntries(s.ruleCounts),
      capped_rules: Object.keys(result.capped_totals ?? {}),
    },
    actions: getActions(),
    logs: getLogs(),
  };
  return JSON.stringify(bundle, null, 2);
}

// Golden snapshot (sürüm-diff / regresyon): SADECE deterministik agregat — logs/actions/
// timestamp/user-agent YOK. rule_id ALFABETİK sıralı (count değişse de satır sırası sabit →
// temiz `git diff`). Feed içeriği dahil edilmez (Teşhis Paketi ile aynı gizlilik garantisi).
function buildGoldenSnapshot(result: ValidationResult, fileName: string): string {
  const s = buildSummaryData(result, fileName, '');
  const ruleCounts = Object.fromEntries(
    [...s.ruleCounts].sort((a, b) => (a[0] < b[0] ? -1 : a[0] > b[0] ? 1 : 0)),
  );
  const r5 = result.reports.r5;
  const golden = {
    schema: 'gtfs-analyzer-golden/1',
    app_version: typeof __APP_VERSION__ !== 'undefined' ? __APP_VERSION__ : 'dev',
    feed: fileName,
    validate_date: new Date().toISOString().slice(0, 10),  // tarih-bağımlı kuralları yorumlamak için
    scores: {
      score: s.qualScore, pub_score: s.pubScore,
      spec: r5.spec_score, interop: r5.interop_score,
      quality: r5.quality_score, analytics: r5.analytics_score,
    },
    notice_total_actual: s.real.total,
    by_severity_actual: { critical: s.real.critical, high: s.real.high, medium: s.real.medium, low: s.real.low, info: s.real.info },
    rule_counts_actual: ruleCounts,
    capped_rules: Object.keys(result.capped_totals ?? {}).sort(),
  };
  return JSON.stringify(golden, null, 2);
}

// Severity satırları: [etiket, disp, real, renk]
function summaryRows(d: SummaryData): Array<[string, number, number, string | undefined]> {
  return [
    [t('export.html.total'), d.disp.total, d.real.total, undefined],
    [SEVERITY_TR['CRITICAL'], d.disp.critical, d.real.critical, '#dc2626'],
    [SEVERITY_TR['HIGH'], d.disp.high, d.real.high, '#ea580c'],
    [SEVERITY_TR['MEDIUM'], d.disp.medium, d.real.medium, '#ca8a04'],
    [SEVERITY_TR['LOW'], d.disp.low, d.real.low, '#16a34a'],
    [SEVERITY_TR['INFO'], d.disp.info, d.real.info, '#2563eb'],
  ];
}

// Kopyalanabilir Markdown: meta + Cap'li/Gerçek 2-sütunlu sayım tablosu + #21 için kural-sayım JSON.
function summaryMarkdown(d: SummaryData): string {
  const meta = [
    `**${d.file} — ${t('export.summary.title')}**`,
    `${d.ts} · ${t('export.html.pub_score')} ${fmtDec1(d.pubScore)} / ${t('export.html.qual_score')} ${fmtDec1(d.qualScore)} · ${fmtInt(d.ruleTypes)} ${t('export.summary.rule_types')}`,
  ].join('  \n');
  const head = `| ${t('export.summary.metric')} | ${t('export.summary.capped')} | ${t('export.summary.real')} |\n|---|--:|--:|`;
  const body = summaryRows(d).map(([k, disp, real]) => `| ${k} | ${fmtInt(disp)} | ${fmtInt(real)} |`).join('\n');
  const ruleJson = JSON.stringify({ file: d.file, ts: d.ts, scores: { pub: d.pubScore, quality: d.qualScore }, rule_counts: Object.fromEntries(d.ruleCounts) });
  return `${meta}\n\n${head}\n${body}\n\n<!-- rule_counts gerçek sayılar (#21 diff) -->\n\`\`\`json\n${ruleJson}\n\`\`\`\n`;
}

// Özetin PNG render'ı (kullanıcı kart içinde okur). Cap'li / Gerçek iki sütun.
function summaryPng(d: SummaryData): string {
  const rows = summaryRows(d);
  const dpr = window.devicePixelRatio || 1;
  const W = 520, rowH = 30, top = 70, bot = 16;
  const H = top + (rows.length + 1) * rowH + bot;  // +1 başlık satırı
  const cv = document.createElement('canvas');
  cv.width = W * dpr; cv.height = H * dpr;
  const x = cv.getContext('2d')!;
  x.scale(dpr, dpr);
  x.fillStyle = '#ffffff'; x.fillRect(0, 0, W, H);
  const colCap = W - 150, colReal = W - 22;  // sağ-hizalı sütun x'leri
  // Başlık: "<feed.zip> — Özet Tablo"
  const suffix = ` — ${t('export.summary.title')}`;
  x.font = '700 15px system-ui, sans-serif'; x.textAlign = 'left';
  let f = d.file;
  while (f.length > 4 && x.measureText(f + suffix).width > W - 36) f = f.slice(0, -1);
  if (f.length < d.file.length) f = f.slice(0, -1) + '…';
  x.fillStyle = '#1e293b'; x.fillText(f + suffix, 18, 28);
  // Meta satırı
  x.fillStyle = '#64748b'; x.font = '12px system-ui, sans-serif';
  x.fillText(`${d.ts} · ${t('export.html.pub_score')} ${fmtDec1(d.pubScore)} · ${t('export.html.qual_score')} ${fmtDec1(d.qualScore)} · ${fmtInt(d.ruleTypes)} ${t('export.summary.rule_types')}`, 18, 48);
  // Sütun başlıkları
  let y = top;
  x.fillStyle = '#94a3b8'; x.font = '600 11px system-ui, sans-serif';
  x.textAlign = 'right'; x.fillText(t('export.summary.capped').toUpperCase(), colCap, y); x.fillText(t('export.summary.real').toUpperCase(), colReal, y);
  y += rowH;
  for (const [label, disp, real, color] of rows) {
    x.strokeStyle = '#eef2f7'; x.beginPath(); x.moveTo(18, y - 19); x.lineTo(W - 18, y - 19); x.stroke();
    x.fillStyle = '#475569'; x.font = '13px system-ui, sans-serif'; x.textAlign = 'left';
    x.fillText(label, 22, y);
    x.font = '700 13px system-ui, sans-serif'; x.textAlign = 'right';
    x.fillStyle = '#94a3b8'; x.fillText(fmtInt(disp), colCap, y);
    x.fillStyle = color ?? '#1e293b'; x.fillText(fmtInt(real), colReal, y);
    y += rowH;
  }
  return cv.toDataURL('image/png');
}

export function renderExport(
  root: HTMLElement,
  result: ValidationResult,
  fileName: string,
): void {
  // EN/JA parite: route-scoped bulgu mesajlarının {route_label}'ını doldur (R2 ile aynı).
  augmentRouteLabels(result.notices, result.name_index);

  // Tarih-zaman damgası: karşılaştırma (#21) için her çıktıya yazılır.
  const now = new Date().toISOString().slice(0, 19).replace('T', ' ');
  const fnTs = now.replace(/[: ]/g, '-');

  // İçerikleri bir kez üret → hem tahmini boyut hem indirme bunları kullanır (çift iş yok).
  const htmlStr = buildReportHtml(result, fileName, now);
  const csvStr  = buildCsv(result);
  const jsonStr = JSON.stringify(result, null, 2);
  const summary = buildSummaryData(result, fileName, now);
  const summaryMd = summaryMarkdown(summary);
  const summaryImg = summaryPng(summary);
  const debugStr = buildDebugBundle(result, fileName, getState().fileSize, now);
  const goldenStr = buildGoldenSnapshot(result, fileName);

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

        <div class="exp-summary-card">
          <div class="exp-summary-head">
            <span class="exp-card-icon exp-icon-doc">${ICON.chart}</span>
            <span class="exp-card-title">${t('export.summary.title')}</span>
            <button id="btn-summary-copy" class="btn btn-secondary exp-summary-copy">${t('export.summary.copy')}</button>
            <button id="btn-summary-png" class="btn btn-secondary exp-summary-copy">PNG</button>
          </div>
          <p class="exp-card-desc">${t('export.summary.desc')}</p>
          <img class="exp-summary-img" src="${summaryImg}" alt="${t('export.summary.title')}" />
        </div>

        <div class="exp-summary-card">
          <div class="exp-summary-head">
            <span class="exp-card-icon exp-icon-amber">${ICON.shield}</span>
            <span class="exp-card-title">${t('export.debug.title')}</span>
            <button id="btn-debug-json" class="btn btn-secondary exp-summary-copy">${t('export.download')}</button>
            <button id="btn-golden-json" class="btn btn-secondary exp-summary-copy">Golden JSON</button>
          </div>
          <p class="exp-card-desc">${t('export.debug.desc')}</p>
          <p class="exp-card-desc exp-debug-privacy"><span class="exp-i">ⓘ</span> ${t('export.debug.privacy')}</p>
        </div>

        <div class="exp-note"><span class="exp-i">ⓘ</span> <strong>${t('export.note.label')}</strong> ${t('export.note.text')}</div>
      </div>
    </section>`;

  const dl = (s: string, mime: string, ext: string) =>
    triggerDownload(new Blob([s], { type: mime }), fileName.replace(/\.zip$/i, `-report-${fnTs}.${ext}`));

  root.querySelector('#btn-export-html')!.addEventListener('click', () => dl(htmlStr, 'text/html; charset=utf-8', 'html'));
  root.querySelector('#btn-export-csv')!.addEventListener('click',  () => dl('﻿' + csvStr, 'text/csv; charset=utf-8', 'csv'));
  root.querySelector('#btn-export-json')!.addEventListener('click', () => dl(jsonStr, 'application/json', 'json'));
  root.querySelector('#btn-export-pdf')!.addEventListener('click', () => printHtml(htmlStr));
  root.querySelector('#btn-debug-json')!.addEventListener('click', () =>
    triggerDownload(new Blob([debugStr], { type: 'application/json' }), fileName.replace(/\.zip$/i, `-debug-${fnTs}.json`)));
  root.querySelector('#btn-golden-json')!.addEventListener('click', () => {
    const v = typeof __APP_VERSION__ !== 'undefined' ? __APP_VERSION__ : 'dev';
    triggerDownload(new Blob([goldenStr], { type: 'application/json' }), fileName.replace(/\.zip$/i, `-golden-${v}.json`));
  });

  const copyBtn = root.querySelector<HTMLButtonElement>('#btn-summary-copy')!;
  copyBtn.addEventListener('click', async () => {
    try {
      await navigator.clipboard.writeText(summaryMd);
      const old = copyBtn.textContent;
      copyBtn.textContent = t('export.summary.copied');
      setTimeout(() => { copyBtn.textContent = old; }, 1500);
    } catch { alert(t('export.summary.copy_failed')); }
  });
  root.querySelector('#btn-summary-png')!.addEventListener('click', () => {
    const a = document.createElement('a');
    a.href = summaryImg;
    a.download = fileName.replace(/\.zip$/i, `-summary-${fnTs}.png`);
    a.click();
  });
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

function buildReportHtml(result: ValidationResult, fileName: string, ts: string): string {
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
  <p style="color:#64748b;font-size:.82rem">${t('export.summary.datetime')}: ${escHtml(ts)}</p>

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
