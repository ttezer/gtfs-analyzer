import { setResult, getState, setConfigDelta, setPage } from '../state';
import { FATAL_CODE_TR, t, getLocale } from '../i18n';
import type { FatalError, ValidationResult, FileInfo } from '../types';
import { renderApp } from '../main';
import { validateFile } from '../validator-client';
import type { EngineMode } from '../wasm';
import { validateFeedUrl, urlFileName, isZipSignature } from './url-helpers';
import { escHtml } from '../escape';

const STAGE_ORDER = ['K1', 'K2', 'K3', 'K4', 'K5', 'K6', 'K7'];
let activeEngine: EngineMode | null = null;

function engineBadge(): string {
  return activeEngine ? `<span class="engine-badge">${escHtml(t(`engine.${activeEngine}`))}</span>` : '';
}

const CONFIG_KEYS: Array<{ key: string; type: 'float' | 'int'; def: number; min: number; max: number }> = [
  { key: 'max_speed_bus_kmh',        type: 'float', def: 120,  min: 60,   max: 200  },
  { key: 'max_speed_tram_kmh',       type: 'float', def: 100,  min: 40,   max: 160  },
  { key: 'max_speed_metro_kmh',      type: 'float', def: 150,  min: 80,   max: 250  },
  { key: 'max_speed_rail_kmh',       type: 'float', def: 300,  min: 100,  max: 400  },
  { key: 'max_speed_ferry_kmh',      type: 'float', def: 80,   min: 20,   max: 150  },
  { key: 'max_speed_cablecar_kmh',   type: 'float', def: 30,   min: 10,   max: 60   },
  { key: 'min_transfer_time_sec',    type: 'int',   def: 180,  min: 30,   max: 1800 },
  { key: 'max_transfer_distance_m',  type: 'float', def: 500,  min: 50,   max: 2000 },
  { key: 'max_shape_jump_km',        type: 'float', def: 10,   min: 1,    max: 50   },
  { key: 'stop_too_close_m',         type: 'float', def: 5,    min: 1,    max: 20   },
  { key: 'stop_far_from_shape_m',    type: 'float', def: 100,  min: 20,   max: 500  },
  { key: 'stop_far_from_parent_m',   type: 'float', def: 100,  min: 10,   max: 1000 },
  { key: 'feed_expiry_warning_days', type: 'int',   def: 30,   min: 1,    max: 60   },
  { key: 'service_gap_days',         type: 'int',   def: 7,    min: 3,    max: 30   },
  { key: 'big_gap_days',             type: 'int',   def: 14,   min: 7,    max: 60   },
  { key: 'upcoming_service_days',    type: 'int',   def: 7,    min: 1,    max: 90   },
  { key: 'max_trip_duration_hours',  type: 'float', def: 24,   min: 8,    max: 72   },
  { key: 'min_trip_duration_sec',    type: 'int',   def: 60,   min: 10,   max: 300  },
  { key: 'max_headway_warning_min',  type: 'int',   def: 240,  min: 60,   max: 720  },
  { key: 'bunching_threshold_min',   type: 'int',   def: 2,    min: 1,    max: 10   },
  { key: 'service_day_start_hour',   type: 'int',   def: 3,    min: 0,    max: 6    },
  { key: 'max_calendar_future_years', type: 'int',  def: 3,    min: 1,    max: 50   },
];

export function renderUpload(root: HTMLElement): void {
  const state = getState();
  const hasResult = state.result !== null;
  const btnLabel = hasResult ? t('upload.load_another') : t('upload.load');

  root.innerHTML = `
    <div class="up-grid">

      <div class="up-cell up-url-bar">
        <div class="up-url-row">
          <input type="url" id="url-input" class="up-url-input" placeholder="${escHtml(t('upload.url_placeholder'))}" aria-label="${escHtml(t('upload.url_aria'))}"/>
          <button id="url-btn" class="btn btn-secondary btn-sm">${escHtml(t('upload.url_load'))}</button>
        </div>
        <span class="up-url-hint">${escHtml(t('upload.url_hint'))}</span>
      </div>

      <div class="up-cell up-files">
        <div class="lp-section-title">${t('upload.files_section')}</div>
        <div id="file-rows" class="lp-rows">
          ${hasResult && state.result!.metrics.file_stats.length > 0
            ? renderFileStatRows(state.result!.metrics.file_stats)
            : `<div class="lp-placeholder">${t('upload.no_files')}</div>`}
        </div>
      </div>

      <div class="up-cell up-drop">
        <div id="drop-zone" class="drop-zone" role="button" tabindex="0" aria-label="${escHtml(t('upload.drop_aria'))}">
          <svg class="upload-icon-sm" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
            <path d="M3 16.5v2.25A2.25 2.25 0 005.25 21h13.5A2.25 2.25 0 0021 18.75V16.5m-13.5-9L12 3m0 0l4.5 4.5M12 3v13.5"/>
          </svg>
          <span class="drop-primary">${t('upload.drag')}</span>
          <label id="upload-btn-label" class="btn btn-primary btn-sm" for="file-input">${escHtml(btnLabel)}</label>
          <input type="file" id="file-input" accept=".zip" class="visually-hidden" aria-label="${escHtml(t('upload.file_input_aria'))}"/>
        </div>
        <div id="uploaded-name" class="uploaded-name${hasResult && state.fileName ? '' : ' hidden'}">
          ${hasResult && state.fileName ? `${t('upload.loaded_file')} <strong>${escHtml(state.fileName)}</strong> ${engineBadge()}` : ''}
        </div>
        <div id="upload-error" class="upload-status hidden"></div>
      </div>

      <div class="up-cell up-stages">
        <div class="lp-section-title">${t('upload.stages_section')}</div>
        <div id="stage-rows" class="lp-rows">
          ${STAGE_ORDER.map(s => `
            <div class="lp-stage-row ${hasResult ? 'done' : 'waiting'}" data-stage="${s}">
              <span class="lp-icon">${hasResult ? '✓' : '○'}</span>
              <span class="lp-label">${escHtml(t(`stage.${s}`))}</span>
              <span class="lp-status">${hasResult ? '' : '—'}</span>
            </div>`).join('')}
        </div>
      </div>

      <div class="up-cell up-score">
        <div id="score-panel" class="score-compact${hasResult ? '' : ' hidden'}">
          ${hasResult ? renderScoreCompact(state.result!) : ''}
        </div>
        <div id="report-duration" class="report-duration${hasResult && state.reportDurationMs != null ? '' : ' hidden'}">
          ${hasResult && state.reportDurationMs != null ? escHtml(t('upload.report_ready', { duration: formatReportDuration(state.reportDurationMs) })) : ''}
        </div>
        <button id="btn-report" class="btn btn-primary btn-report" ${hasResult ? '' : 'disabled'}>
          ${t('upload.show_report')}
        </button>
      </div>

    </div>

    <div class="settings-wrap">
      <button class="settings-toggle" id="settings-toggle" type="button">
        <span class="settings-arrow" id="settings-arrow">▶</span>
        ${t('upload.settings_title')}
        ${state.configDelta && state.configDelta !== '{}'
          ? `<span class="settings-badge">${t('upload.settings_modified')}</span>` : ''}
      </button>
      <div class="settings-body hidden" id="settings-body">
        <div class="settings-grid" id="settings-grid">
          ${renderSettingsFields(parseConfigDelta(state.configDelta))}
        </div>
        <div class="settings-actions">
          <button id="btn-apply-settings" class="btn btn-primary btn-sm">${t('upload.apply')}</button>
          <button id="btn-reset-settings" class="btn btn-ghost btn-sm">${t('upload.reset')}</button>
          <span id="settings-status" class="settings-status hidden"></span>
        </div>
      </div>
    </div>`;

  attachUploadListeners(root);
}

// ── Dosya satırları ──────────────────────────────────────────────────────────

function renderFileStatRows(files: FileInfo[]): string {
  return files.map(f => {
    const kb = Math.round(f.bytes / 1024);
    const base = t(`file.${f.name}`) !== `file.${f.name}` ? t(`file.${f.name}`) : t('upload.row_unit');
    const label = pluralUnit(base, f.rows);
    return `
      <div class="lp-file-row done">
        <span class="lp-icon">✓</span>
        <span class="lp-label">${escHtml(f.name)}</span>
        <span class="lp-status">${formatCount(f.rows)} ${label} · ${formatSize(kb)}</span>
      </div>`;
  }).join('');
}

// ── Skor kompakt panel ───────────────────────────────────────────────────────

function renderScoreCompact(result: ValidationResult): string {
  const { r5 } = result.reports;
  const cats = [
    { labelKey: 'class.SPEC',      weight: '×40%', value: r5.spec_score,      color: '#2563eb', bg: '#eff6ff' },
    { labelKey: 'class.INTEROP',   weight: '×30%', value: r5.interop_score,   color: '#92400e', bg: '#fef3c7' },
    { labelKey: 'class.QUALITY',   weight: '×20%', value: r5.quality_score,   color: '#15803d', bg: '#f0fdf4' },
    { labelKey: 'class.ANALYTICS', weight: '×10%', value: r5.analytics_score, color: '#6d28d9', bg: '#f5f3ff' },
  ];
  const qualColor = r5.score     >= 80 ? '#15803d' : r5.score     >= 60 ? '#92400e' : '#dc2626';
  const pubColor  = r5.pub_score >= 80 ? '#15803d' : r5.pub_score >= 60 ? '#d97706' : '#dc2626';

  const chips = cats.map(c =>
    `<div class="sc-chip" style="background:${c.bg};color:${c.color}">
      <div class="sc-chip-left">
        <span class="sc-chip-label">${t(c.labelKey)}</span>
        <span class="sc-weight">${c.weight}</span>
      </div>
      <span class="sc-chip-val">${c.value.toFixed(1)}</span>
    </div>`
  ).join('');

  return `
    <div class="sc-dual-header">
      <div class="sc-dual-item">
        <span class="sc-title">${t('score.pub')}</span>
        <span class="sc-overall" style="color:${pubColor}">${r5.pub_score.toFixed(1)}<span class="sc-max">/100</span></span>
      </div>
      <div class="sc-dual-sep"></div>
      <div class="sc-dual-item">
        <span class="sc-title">${t('score.overall')}</span>
        <span class="sc-overall" style="color:${qualColor}">${r5.score.toFixed(1)}<span class="sc-max">/100</span></span>
      </div>
    </div>
    <div class="sc-chips">${chips}</div>`;
}

// ── Ayarlar alanları ─────────────────────────────────────────────────────────

function renderSettingsFields(overrides: Record<string, number>): string {
  return CONFIG_KEYS.map(f => {
    const val = overrides[f.key] !== undefined ? overrides[f.key] : f.def;
    const isOverridden = overrides[f.key] !== undefined;
    const label = t(`cfg.${f.key}.label`);
    const unit  = t(`cfg.${f.key}.unit`);
    const desc  = t(`cfg.${f.key}.desc`);
    return `
      <div class="sf-row${isOverridden ? ' sf-overridden' : ''}">
        <label class="sf-label" title="${escHtml(desc)}">${escHtml(label)}</label>
        <div class="sf-control">
          <input class="sf-input" type="number" data-key="${f.key}" data-def="${f.def}"
            data-type="${f.type}" min="${f.min}" max="${f.max}"
            step="${f.type === 'int' ? 1 : 0.1}" value="${val}"/>
          <span class="sf-unit">${unit}</span>
          <button class="sf-reset btn btn-ghost" data-key="${f.key}" title="${t('upload.reset')}"
            ${!isOverridden ? 'style="visibility:hidden"' : ''}>✕</button>
        </div>
        <div class="sf-desc">${escHtml(desc)} (${t('upload.default_for', { val: f.def })})</div>
      </div>`;
  }).join('');
}

function parseConfigDelta(delta: string): Record<string, number> {
  if (!delta || delta === '{}') return {};
  try {
    const parsed: unknown = JSON.parse(delta);
    if (typeof parsed !== 'object' || parsed === null || Array.isArray(parsed)) return {};
    const result: Record<string, number> = {};
    for (const [key, value] of Object.entries(parsed as Record<string, unknown>)) {
      if (typeof value === 'number' && isFinite(value)) result[key] = value;
    }
    return result;
  } catch { return {}; }
}

// ── Olay dinleyicileri ───────────────────────────────────────────────────────

function attachUploadListeners(root: HTMLElement): void {
  const dropZone  = root.querySelector<HTMLElement>('#drop-zone')!;
  const fileInput = root.querySelector<HTMLInputElement>('#file-input')!;
  const errorEl   = root.querySelector<HTMLElement>('#upload-error')!;

  fileInput.addEventListener('change', () => {
    const file = fileInput.files?.[0];
    if (file) void handleFile(file, root, errorEl);
  });

  dropZone.addEventListener('dragover', (e) => {
    e.preventDefault();
    if (!dropZone.classList.contains('loading')) dropZone.classList.add('drag-over');
  });
  dropZone.addEventListener('dragleave', () => dropZone.classList.remove('drag-over'));
  dropZone.addEventListener('drop', (e) => {
    e.preventDefault();
    dropZone.classList.remove('drag-over');
    if (dropZone.classList.contains('loading')) return;
    const file = e.dataTransfer?.files[0];
    if (file) void handleFile(file, root, errorEl);
  });
  dropZone.addEventListener('keydown', (e) => {
    if ((e.key === 'Enter' || e.key === ' ') && !dropZone.classList.contains('loading'))
      fileInput.click();
  });

  // #45: URL'den yükleme (client-side fetch).
  const urlInput = root.querySelector<HTMLInputElement>('#url-input');
  const urlBtn   = root.querySelector<HTMLButtonElement>('#url-btn');
  const loadUrl = () => {
    if (dropZone.classList.contains('loading')) return;
    const v = urlInput?.value.trim() ?? '';
    if (v) void handleUrl(v, root, errorEl);
  };
  urlBtn?.addEventListener('click', loadUrl);
  urlInput?.addEventListener('keydown', (e) => {
    if (e.key === 'Enter') { e.preventDefault(); loadUrl(); }
  });

  root.querySelector('#btn-report')?.addEventListener('click', () => { setPage('domain'); renderApp(); });

  const settingsToggle = root.querySelector<HTMLButtonElement>('#settings-toggle')!;
  const settingsBody   = root.querySelector<HTMLElement>('#settings-body')!;
  const settingsArrow  = root.querySelector<HTMLElement>('#settings-arrow')!;

  settingsToggle.addEventListener('click', () => {
    const isOpen = !settingsBody.classList.contains('hidden');
    settingsBody.classList.toggle('hidden', isOpen);
    settingsArrow.textContent = isOpen ? '▶' : '▼';
  });

  root.querySelector('#btn-apply-settings')?.addEventListener('click', () => applySettings(root));
  root.querySelector('#btn-reset-settings')?.addEventListener('click', () => resetSettings(root));

  root.querySelectorAll<HTMLButtonElement>('.sf-reset').forEach(btn => {
    btn.addEventListener('click', () => {
      const key = btn.dataset['key']!;
      const input = root.querySelector<HTMLInputElement>(`.sf-input[data-key="${CSS.escape(key)}"]`);
      if (input) {
        input.value = input.dataset['def']!;
        input.closest('.sf-row')?.classList.remove('sf-overridden');
        btn.style.visibility = 'hidden';
      }
    });
  });

  root.querySelectorAll<HTMLInputElement>('.sf-input').forEach(input => {
    input.addEventListener('change', () => {
      const btn = root.querySelector<HTMLButtonElement>(
        `.sf-reset[data-key="${CSS.escape(input.dataset['key']!)}"]`);
      if (btn) btn.style.visibility = 'visible';
      input.closest('.sf-row')?.classList.add('sf-overridden');
    });
  });
}

function applySettings(root: HTMLElement): void {
  const delta: Record<string, number> = {};
  root.querySelectorAll<HTMLInputElement>('.sf-input').forEach(input => {
    const key = input.dataset['key']!;
    const def = parseFloat(input.dataset['def']!);
    const val = input.dataset['type'] === 'int'
      ? parseInt(input.value, 10)
      : parseFloat(input.value);
    if (!isNaN(val) && val !== def) delta[key] = val;
  });
  const deltaStr = Object.keys(delta).length > 0 ? JSON.stringify(delta) : '';
  setConfigDelta(deltaStr);

  const statusEl = root.querySelector<HTMLElement>('#settings-status')!;
  statusEl.textContent = t('upload.settings_saved');
  statusEl.className = 'settings-status ok';
  setTimeout(() => { statusEl.className = 'settings-status hidden'; }, 3000);
}

function resetSettings(root: HTMLElement): void {
  root.querySelectorAll<HTMLInputElement>('.sf-input').forEach(input => {
    input.value = input.dataset['def']!;
    input.closest('.sf-row')?.classList.remove('sf-overridden');
    const btn = root.querySelector<HTMLButtonElement>(
      `.sf-reset[data-key="${CSS.escape(input.dataset['key']!)}"]`);
    if (btn) btn.style.visibility = 'hidden';
  });
  setConfigDelta('');

  const statusEl = root.querySelector<HTMLElement>('#settings-status')!;
  statusEl.textContent = t('upload.settings_reset');
  statusEl.className = 'settings-status ok';
  setTimeout(() => { statusEl.className = 'settings-status hidden'; }, 3000);
}

// ── Dosya işleme ─────────────────────────────────────────────────────────────

const MAX_FILE_BYTES = 1024 * 1024 * 1024;

async function handleFile(file: File, root: HTMLElement, errorEl: HTMLElement): Promise<void> {
  if (!file.name.endsWith('.zip')) {
    showError(errorEl, { code: 'InvalidInput', message: t('upload.error_zip') });
    return;
  }
  if (file.size > MAX_FILE_BYTES) {
    showError(errorEl, {
      code: 'InvalidInput',
      message: t('upload.error_size', { mb: (file.size / 1_048_576).toFixed(1) }),
    });
    return;
  }
  let buffer: ArrayBuffer;
  try { buffer = await file.arrayBuffer(); }
  catch (err) { showError(errorEl, err as FatalError); return; }
  await handleBuffer(buffer, file.name, file.size, root, errorEl);
}

// #45: feed'i bir URL'den CLIENT-SIDE indirir. CORS-açık host'larda (ör. Mobility Database)
// çalışır; CORS'suz host'ta tarayıcı fetch'i engeller → net mesaj + indir-sürükle fallback.
// Çekirdek/WASM internet isteği YAPMAZ; indirme yalnız bu UI katmanındadır.
async function handleUrl(rawUrl: string, root: HTMLElement, errorEl: HTMLElement): Promise<void> {
  if (!rawUrl.trim()) return;

  // 1) Ucuz client-side doğrulama (ağ yok): https zorunlu, gömülü kimlik bilgisi reddedilir.
  const check = validateFeedUrl(rawUrl);
  if (!check.ok) {
    const key = check.reason === 'https' ? 'upload.url_https'
      : check.reason === 'credentials' ? 'upload.url_credentials' : 'upload.url_invalid';
    showError(errorEl, { code: 'InvalidInput', message: t(key) });
    return;
  }
  const url = check.url;
  const name = urlFileName(url);
  errorEl.className = 'upload-status hidden';
  // İNDİRME aşaması: K1–K7 satırlarına, dosya listesine ve skor paneline DOKUNMA.
  // Gerçek setLoading() ZIP indirilip imzası doğrulandıktan sonra handleBuffer'da çağrılır.
  // İndirme hata verirse clearLoading bu hafif durumu geri alır → önceki sonuç korunur.
  setUrlDownloading(root, name);

  // 2) Header fetch — CORS/DNS/TLS/offline burada ayırt edilemez TypeError olur.
  let resp: Response;
  try {
    resp = await fetch(url.href, {
      credentials: 'omit', referrerPolicy: 'no-referrer', redirect: 'follow',
      signal: AbortSignal.timeout(30_000),
    });
  } catch (err) {
    clearLoading(root);
    if (err instanceof DOMException && err.name === 'TimeoutError') {
      showError(errorEl, { code: 'InvalidInput', message: t('upload.url_timeout') });
    } else {
      showUrlBlocked(errorEl, url); // CORS/ağ ortak mesajı + gerçek tıklanır link fallback
    }
    return;
  }
  // 3) CORS izinliyse HTTP hataları okunur.
  if (!resp.ok) {
    clearLoading(root);
    showError(errorEl, { code: 'InvalidInput', message: t('upload.url_http', { status: String(resp.status) }) });
    return;
  }
  // 4) Content-Length erken-ret (opsiyonel, yanlış olabilir → asıl sınır stream'de).
  const declared = Number(resp.headers.get('content-length'));
  if (declared && declared > MAX_FILE_BYTES) {
    clearLoading(root);
    showError(errorEl, { code: 'InvalidInput', message: t('upload.error_size', { mb: (declared / 1_048_576).toFixed(1) }) });
    return;
  }
  // 5) Sınırlı stream indirme (Content-Length yalanlarına karşı gerçek bayt sayımı) + body-hata aşaması.
  let buffer: ArrayBuffer;
  try { buffer = await readCapped(resp, MAX_FILE_BYTES); }
  catch (err) {
    clearLoading(root);
    const msg = err instanceof RangeError
      ? t('upload.error_size', { mb: (MAX_FILE_BYTES / 1_048_576).toFixed(0) })
      : t('upload.url_body');
    showError(errorEl, { code: 'InvalidInput', message: msg });
    return;
  }
  // 6) ZIP imzası (PK\x03\x04) — uzantı yerine otoriter içerik kontrolü.
  if (!isZipSignature(new Uint8Array(buffer, 0, Math.min(4, buffer.byteLength)))) {
    clearLoading(root);
    showError(errorEl, { code: 'InvalidInput', message: t('upload.url_not_zip') });
    return;
  }

  await handleBuffer(buffer, name, buffer.byteLength, root, errorEl);
}

// Response gövdesini bayt sınırıyla akış-okur; sınır aşılırsa RangeError, indirme kesilir.
async function readCapped(resp: Response, maxBytes: number): Promise<ArrayBuffer> {
  if (!resp.body) {
    const buf = await resp.arrayBuffer();
    if (buf.byteLength > maxBytes) throw new RangeError('exceeds limit');
    return buf;
  }
  const reader = resp.body.getReader();
  const chunks: Uint8Array[] = [];
  let total = 0;
  for (;;) {
    const { done, value } = await reader.read();
    if (done) break;
    total += value.byteLength;
    if (total > maxBytes) { await reader.cancel(); throw new RangeError('exceeds limit'); }
    chunks.push(value);
  }
  const out = new Uint8Array(total);
  let off = 0;
  for (const c of chunks) { out.set(c, off); off += c.byteLength; }
  return out.buffer;
}

// CORS/ağ hatası: tarayıcı sebebi gizler → dürüst ortak mesaj + gerçek tıklanır link.
// Cross-origin `download` attribute'una GÜVENİLMEZ; sade bir link kullanıcının indirmesini sağlar.
function showUrlBlocked(el: HTMLElement, url: URL): void {
  el.className = 'upload-status error';
  el.innerHTML = `${escHtml(t('upload.url_blocked'))} `
    + `<a href="${escHtml(url.href)}" target="_blank" rel="noopener noreferrer">${escHtml(t('upload.url_open_link'))}</a> `
    + escHtml(t('upload.url_then_drop'));
}

async function handleBuffer(buffer: ArrayBuffer, fileName: string, fileSize: number, root: HTMLElement, errorEl: HTMLElement): Promise<void> {
  errorEl.className = 'upload-status hidden';
  setLoading(root, fileName, fileSize);
  const startedAt = performance.now();

  try {
    const result = await validateFile(buffer, getState().configDelta, {
      onEngine: (mode) => {
        activeEngine = mode;
        const name = root.querySelector<HTMLElement>('#uploaded-name');
        if (name) {
          name.classList.remove('hidden');
          name.innerHTML = `<strong>${escHtml(fileName)}</strong> ${engineBadge()}`;
        }
      },
      onFileList: (files) => {
        const container = root.querySelector<HTMLElement>('#file-rows');
        if (!container) return;
        container.innerHTML = '';
        files.forEach(f => {
          const row = document.createElement('div');
          row.className = 'lp-file-row reading';
          row.dataset['file'] = f.name;
          const sizeKb = Math.round(f.uncompressed_size / 1024);
          const readingLabel = `${formatSize(sizeKb)} ${t('upload.running')}`;
          row.innerHTML = `
            <span class="lp-icon">○</span>
            <span class="lp-label">${escHtml(f.name)}</span>
            <span class="lp-status">${readingLabel}</span>`;
          container.appendChild(row);
        });
      },
      onFileDone: (name, rows, bytes) => {
        const container = root.querySelector<HTMLElement>('#file-rows');
        const row = container?.querySelector<HTMLElement>(`[data-file="${CSS.escape(name)}"]`);
        if (!row) return;
        row.className = 'lp-file-row done';
        row.querySelector('.lp-icon')!.textContent = '✓';
        const base = t(`file.${name}`) !== `file.${name}` ? t(`file.${name}`) : t('upload.row_unit');
        const label = pluralUnit(base, rows);
        row.querySelector<HTMLElement>('.lp-status')!.textContent =
          `${formatCount(rows)} ${label} · ${formatSize(Math.round(bytes / 1024))}`;
      },
      onStageDone: (stage, elapsed_ms) => {
        const stageEl = root.querySelector<HTMLElement>(`[data-stage="${CSS.escape(stage)}"]`);
        if (!stageEl) return;
        stageEl.className = 'lp-stage-row done';
        stageEl.querySelector('.lp-icon')!.textContent = '✓';
        stageEl.querySelector<HTMLElement>('.lp-status')!.textContent =
          elapsed_ms < 1000
            ? `${elapsed_ms} ${t('upload.unit_ms')}`
            : `${(elapsed_ms / 1000).toLocaleString(numLocale(), { minimumFractionDigits: 1, maximumFractionDigits: 1 })} ${t('upload.unit_sec')}`;
        const idx = STAGE_ORDER.indexOf(stage);
        if (idx >= 0 && idx < STAGE_ORDER.length - 1) {
          const nextEl = root.querySelector<HTMLElement>(
            `[data-stage="${CSS.escape(STAGE_ORDER[idx + 1])}"]`);
          if (nextEl?.classList.contains('waiting')) {
            nextEl.className = 'lp-stage-row running';
            nextEl.querySelector('.lp-icon')!.textContent = '⋯';
            nextEl.querySelector<HTMLElement>('.lp-status')!.textContent = t('upload.running');
          }
        }
      },
    });

    setResult(result, fileName, fileSize, performance.now() - startedAt);
    activateResult(root, result);
  } catch (err: unknown) {
    clearLoading(root);
    resetProgressRows(root);
    showError(errorEl, err as FatalError);
  }
}

// #45: URL indirme aşaması göstergesi. setLoading'in AKSİNE aşama satırlarını (K1–K7),
// dosya listesini ve skor panelini YENİDEN KURMAZ → indirme başarısız olursa clearLoading
// ile temizlenir ve varsa önceki başarılı sonuç ekranda korunur. Yalnız drop alanını kilitler
// ve "<dosya> indiriliyor…" metnini gösterir. Eşzamanlılık koruması dropZone.loading sınıfıdır.
function setUrlDownloading(root: HTMLElement, fileName: string): void {
  const dropZone  = root.querySelector<HTMLElement>('#drop-zone')!;
  const fileInput = root.querySelector<HTMLInputElement>('#file-input')!;
  const btnLabel  = root.querySelector<HTMLElement>('#upload-btn-label')!;
  dropZone.classList.add('loading');
  fileInput.disabled = true;
  btnLabel.setAttribute('aria-disabled', 'true');
  btnLabel.style.opacity = '0.5';
  btnLabel.style.pointerEvents = 'none';
  const dropPrimary = dropZone.querySelector('.drop-primary');
  if (dropPrimary) dropPrimary.textContent = t('upload.url_downloading', { name: fileName });
}

function setLoading(root: HTMLElement, fileName: string, fileSize: number): void {
  const dropZone  = root.querySelector<HTMLElement>('#drop-zone')!;
  const fileInput = root.querySelector<HTMLInputElement>('#file-input')!;
  const btnLabel  = root.querySelector<HTMLElement>('#upload-btn-label')!;

  const scorePanel = root.querySelector<HTMLElement>('#score-panel');
  if (scorePanel) { scorePanel.classList.add('hidden'); scorePanel.innerHTML = ''; }
  const reportDuration = root.querySelector<HTMLElement>('#report-duration');
  if (reportDuration) { reportDuration.classList.add('hidden'); reportDuration.textContent = ''; }
  const btnReport = root.querySelector<HTMLButtonElement>('#btn-report');
  if (btnReport) btnReport.disabled = true;

  const uploadedName = root.querySelector<HTMLElement>('#uploaded-name');
  if (uploadedName) { uploadedName.classList.add('hidden'); uploadedName.innerHTML = ''; }

  dropZone.classList.add('loading');
  fileInput.disabled = true;
  btnLabel.setAttribute('aria-disabled', 'true');
  btnLabel.style.opacity = '0.5';
  btnLabel.style.pointerEvents = 'none';

  const sizeMb = (fileSize / 1_048_576).toFixed(1);
  const dropPrimary = dropZone.querySelector('.drop-primary');
  if (dropPrimary) dropPrimary.textContent = `${fileName} (${sizeMb} MB) ${t('upload.running')}`;

  const stageRows = root.querySelector<HTMLElement>('#stage-rows');
  if (stageRows) {
    stageRows.innerHTML = STAGE_ORDER.map((s, i) => `
      <div class="lp-stage-row ${i === 0 ? 'running' : 'waiting'}" data-stage="${s}">
        <span class="lp-icon">${i === 0 ? '⋯' : '○'}</span>
        <span class="lp-label">${escHtml(t(`stage.${s}`))}</span>
        <span class="lp-status">${i === 0 ? t('upload.running') : t('upload.waiting')}</span>
      </div>`).join('');
  }
}

function clearLoading(root: HTMLElement): void {
  const dropZone  = root.querySelector<HTMLElement>('#drop-zone')!;
  const fileInput = root.querySelector<HTMLInputElement>('#file-input')!;
  const btnLabel  = root.querySelector<HTMLElement>('#upload-btn-label')!;
  dropZone.classList.remove('loading');
  fileInput.disabled = false;
  btnLabel.removeAttribute('aria-disabled');
  btnLabel.style.opacity = '';
  btnLabel.style.pointerEvents = '';

  // Yükleme bitti — drop alanındaki "<dosya> çalışıyor…" metnini sıfırla.
  const dropPrimary = dropZone.querySelector('.drop-primary');
  if (dropPrimary) dropPrimary.textContent = t('upload.drag');
}

// Fatal hatadan sonra dosya + aşama satırlarını boştaki haline döndürür.
// Hata yolunda onFileDone / onStageDone tetiklenmediği için bu satırlar
// aksi halde "çalışıyor…" göstergesinde takılı kalır.
function resetProgressRows(root: HTMLElement): void {
  const fileRows = root.querySelector<HTMLElement>('#file-rows');
  if (fileRows) {
    fileRows.innerHTML = `<div class="lp-placeholder">${t('upload.no_files')}</div>`;
  }
  const stageRows = root.querySelector<HTMLElement>('#stage-rows');
  if (stageRows) {
    stageRows.innerHTML = STAGE_ORDER.map(s => `
      <div class="lp-stage-row waiting" data-stage="${s}">
        <span class="lp-icon">○</span>
        <span class="lp-label">${escHtml(t(`stage.${s}`))}</span>
        <span class="lp-status">—</span>
      </div>`).join('');
  }
}

function activateResult(root: HTMLElement, result: ValidationResult): void {
  clearLoading(root);
  const btnLabel = root.querySelector<HTMLElement>('#upload-btn-label')!;
  btnLabel.textContent = t('upload.load_another');

  const uploadedName = root.querySelector<HTMLElement>('#uploaded-name');
  if (uploadedName) {
    const fileName = getState().fileName;
    if (fileName) {
      uploadedName.classList.remove('hidden');
      uploadedName.innerHTML = `${t('upload.loaded_file')} <strong>${escHtml(fileName)}</strong> ${engineBadge()}`;
    }
  }

  const scorePanel = root.querySelector<HTMLElement>('#score-panel')!;
  scorePanel.className = 'score-compact';
  scorePanel.innerHTML = renderScoreCompact(result);

  const reportDuration = root.querySelector<HTMLElement>('#report-duration');
  if (reportDuration && getState().reportDurationMs != null) {
    reportDuration.classList.remove('hidden');
    reportDuration.textContent = t('upload.report_ready', { duration: formatReportDuration(getState().reportDurationMs!) });
  }

  const btn = root.querySelector<HTMLButtonElement>('#btn-report')!;
  btn.disabled = false;
  btn.addEventListener('click', () => { setPage('domain'); renderApp(); }, { once: true });

  const fileRows = root.querySelector<HTMLElement>('#file-rows');
  if (fileRows && result.metrics.file_stats.length > 0) {
    fileRows.innerHTML = renderFileStatRows(result.metrics.file_stats);
  }
}

function showError(el: HTMLElement, error: FatalError): void {
  const label = FATAL_CODE_TR[error.code] ?? error.code;
  el.className = 'upload-status error';
  el.innerHTML = `<strong>${escHtml(label)}</strong><p>${escHtml(error.message)}</p>`;
}

function formatSize(kb: number): string {
  return kb >= 1024 ? `${(kb / 1024).toFixed(1)} MB` : `${kb} KB`;
}

function formatReportDuration(ms: number): string {
  return new Intl.NumberFormat(numLocale(), { minimumFractionDigits: 1, maximumFractionDigits: 1 }).format(ms / 1000);
}

// App diline karşılık gelen BCP-47 etiketi — sayı biçimlendirme için (tr virgül, en/ja nokta).
function numLocale(): string {
  const l = getLocale();
  return l === 'tr' ? 'tr-TR' : l === 'ja' ? 'ja-JP' : 'en-US';
}

// Tam sayıyı app diline göre binlik ayraçla biçimler.
function formatCount(n: number): string {
  return n.toLocaleString(numLocale());
}

// Dosya birimi locale'de tekil tutulur; İngilizcede sayıya göre çoğullanır (stop → stops).
// tr/ja adı sayıya göre çekmez → değişmez. Mevcut tüm İngilizce birimler düzenli (+s).
function pluralUnit(label: string, count: number): string {
  if (getLocale() !== 'en') return label;
  return new Intl.PluralRules('en').select(count) === 'one' ? label : `${label}s`;
}
