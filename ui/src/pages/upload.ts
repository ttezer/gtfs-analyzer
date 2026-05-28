import { setResult, getState, setConfigDelta, setPage } from '../state';
import { FATAL_CODE_TR } from '../i18n';
import type { FatalError, ValidationResult, FileInfo } from '../types';
import { renderApp } from '../main';
import { validateFile } from '../validator-client';

const STAGE_LABELS: Record<string, string> = {
  K1: 'K1 — Dosya Ayrıştırma',
  K2: 'K2 — Kural Doğrulama',
  K3: 'K3 — Varlık Haritası',
  K4: 'K4 — Çapraz Kontrol',
  K5: 'K5 — Türetilmiş Veri',
  K6: 'K6 — İstatistik Analizi',
  K7: 'K7 — Rapor Oluşturma',
};

const STAGE_ORDER = ['K1', 'K2', 'K3', 'K4', 'K5', 'K6', 'K7'];

const CONFIG_FIELDS: Array<{
  key: string; label: string; unit: string; type: 'float' | 'int';
  def: number; min: number; max: number; desc: string;
}> = [
  { key: 'max_speed_bus_kmh',        label: 'Maks. Otobüs Hızı',           unit: 'km/s',  type: 'float', def: 120,  min: 60,   max: 200,  desc: 'Otobüs seferleri için maksimum izin verilen hız' },
  { key: 'max_speed_tram_kmh',       label: 'Maks. Tramvay Hızı',          unit: 'km/s',  type: 'float', def: 100,  min: 40,   max: 160,  desc: 'Tramvay seferleri için maksimum izin verilen hız' },
  { key: 'max_speed_metro_kmh',      label: 'Maks. Metro Hızı',            unit: 'km/s',  type: 'float', def: 150,  min: 80,   max: 250,  desc: 'Metro seferleri için maksimum izin verilen hız' },
  { key: 'max_speed_rail_kmh',       label: 'Maks. Demiryolu Hızı',        unit: 'km/s',  type: 'float', def: 300,  min: 100,  max: 400,  desc: 'Demiryolu seferleri için maksimum izin verilen hız' },
  { key: 'max_speed_ferry_kmh',      label: 'Maks. Feribot Hızı',          unit: 'km/s',  type: 'float', def: 80,   min: 20,   max: 150,  desc: 'Feribot seferleri için maksimum izin verilen hız' },
  { key: 'max_speed_cablecar_kmh',   label: 'Maks. Teleferik Hızı',        unit: 'km/s',  type: 'float', def: 30,   min: 10,   max: 60,   desc: 'Teleferik/füniküler için maksimum izin verilen hız' },
  { key: 'min_transfer_time_sec',    label: 'Min. Aktarma Süresi',          unit: 'sn',    type: 'int',   def: 180,  min: 30,   max: 1800, desc: 'Transferler için minimum bağlantı süresi' },
  { key: 'max_transfer_distance_m',  label: 'Maks. Aktarma Mesafesi',      unit: 'm',     type: 'float', def: 500,  min: 50,   max: 2000, desc: 'Transfer geçerli sayılmak için maksimum mesafe' },
  { key: 'max_shape_jump_km',        label: 'Maks. Güzergah Sıçraması',    unit: 'km',    type: 'float', def: 10,   min: 1,    max: 50,   desc: 'Art arda güzergah noktaları arasındaki maksimum mesafe' },
  { key: 'stop_too_close_m',         label: 'Çok Yakın Durak Eşiği',       unit: 'm',     type: 'float', def: 5,    min: 1,    max: 20,   desc: 'Bu mesafeden yakın duraklar tekrar sayılır' },
  { key: 'stop_far_from_shape_m',    label: "Durağın Güzergah'a Uzaklığı", unit: 'm',     type: 'float', def: 100,  min: 20,   max: 500,  desc: 'Durağın güzergahından en fazla bu kadar uzakta olabilir' },
  { key: 'stop_far_from_parent_m',  label: 'Üst İstasyona Uzaklık Eşiği', unit: 'm',     type: 'float', def: 100,  min: 10,   max: 1000, desc: 'Durak, üst istasyonundan en fazla bu kadar uzakta olabilir' },
  { key: 'feed_expiry_warning_days', label: 'Son Kullanma Uyarısı',         unit: 'gün',   type: 'int',   def: 30,   min: 1,    max: 60,   desc: 'Feed bu kadar günden az kalmışsa uyarı üretilir' },
  { key: 'service_gap_days',         label: 'Servis Boşluğu Eşiği',        unit: 'gün',   type: 'int',   def: 7,    min: 3,    max: 30,   desc: 'Bu günden uzun servis kesintisi işaretlenir' },
  { key: 'max_trip_duration_hours',  label: 'Maks. Sefer Süresi',           unit: 'saat',  type: 'float', def: 24,   min: 8,    max: 72,   desc: 'Tek bir seferin maksimum süresi' },
  { key: 'min_trip_duration_sec',    label: 'Min. Sefer Süresi',            unit: 'sn',    type: 'int',   def: 60,   min: 10,   max: 300,  desc: 'Tek bir seferin minimum süresi' },
  { key: 'max_headway_warning_min',  label: 'Maks. Sefer Aralığı',          unit: 'dk',    type: 'int',   def: 240,  min: 60,   max: 720,  desc: 'Bu dakikadan uzun aralık uyarı üretir' },
  { key: 'bunching_threshold_min',   label: 'Sıkışma Eşiği',               unit: 'dk',    type: 'int',   def: 2,    min: 1,    max: 10,   desc: 'Bu dakikadan kısa aralık sıkışma sayılır' },
];

const FILE_ROW_LABEL: Record<string, string> = {
  'stops.txt':          'durak',
  'routes.txt':         'hat',
  'trips.txt':          'sefer',
  'stop_times.txt':     'geçiş zamanı',
  'shapes.txt':         'güzergah',
  'agency.txt':         'işletici',
  'calendar.txt':       'takvim',
  'calendar_dates.txt': 'özel gün',
  'frequencies.txt':    'frekans',
  'transfers.txt':      'aktarma',
  'pathways.txt':       'geçit',
  'levels.txt':         'kat',
  'attributions.txt':   'atıf',
  'translations.txt':   'çeviri',
};

export function renderUpload(root: HTMLElement): void {
  const state = getState();
  const hasResult = state.result !== null;
  const btnLabel = hasResult ? 'Farklı GTFS Yükle' : 'GTFS Yükle';

  /* Grid düzeni:
     Sol sütun  → up-files (üst) + up-stages (alt)
     Sağ sütun  → up-drop  (üst) + up-score  (alt)
     CSS grid-template-areas ile sıra bağımsız yerleşim sağlanır. */
  root.innerHTML = `
    <div class="up-grid">

      <!-- Sol üst: GTFS Dosyaları -->
      <div class="up-cell up-files">
        <div class="lp-section-title">GTFS Dosyaları</div>
        <div id="file-rows" class="lp-rows">
          ${hasResult && state.result!.metrics.file_stats.length > 0
            ? renderFileStatRows(state.result!.metrics.file_stats)
            : '<div class="lp-placeholder">Henüz dosya yüklenmedi</div>'}
        </div>
      </div>

      <!-- Sağ üst: ZIP Yükle -->
      <div class="up-cell up-drop">
        <div id="drop-zone" class="drop-zone" role="button" tabindex="0" aria-label="ZIP dosyası yükle">
          <svg class="upload-icon-sm" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
            <path d="M3 16.5v2.25A2.25 2.25 0 005.25 21h13.5A2.25 2.25 0 0021 18.75V16.5m-13.5-9L12 3m0 0l4.5 4.5M12 3v13.5"/>
          </svg>
          <span class="drop-primary">ZIP sürükleyin</span>
          <label id="upload-btn-label" class="btn btn-primary btn-sm" for="file-input">${escHtml(btnLabel)}</label>
          <input type="file" id="file-input" accept=".zip" class="visually-hidden" aria-label="ZIP dosyası seç"/>
        </div>
        <div id="upload-error" class="upload-status hidden"></div>
      </div>

      <!-- Sol alt: İşlem Aşamaları -->
      <div class="up-cell up-stages">
        <div class="lp-section-title">İşlem Aşamaları</div>
        <div id="stage-rows" class="lp-rows">
          ${STAGE_ORDER.map(s => `
            <div class="lp-stage-row ${hasResult ? 'done' : 'waiting'}" data-stage="${s}">
              <span class="lp-icon">${hasResult ? '✓' : '○'}</span>
              <span class="lp-label">${escHtml(STAGE_LABELS[s] ?? s)}</span>
              <span class="lp-status">${hasResult ? '' : '—'}</span>
            </div>`).join('')}
        </div>
      </div>

      <!-- Sağ alt: Kalite Skoru + Rapor butonu -->
      <div class="up-cell up-score">
        <div id="score-panel" class="score-compact${hasResult ? '' : ' hidden'}">
          ${hasResult ? renderScoreCompact(state.result!) : ''}
        </div>
        <button id="btn-report" class="btn btn-primary btn-report" ${hasResult ? '' : 'disabled'}>
          Raporu Göster →
        </button>
      </div>

    </div>

    <!-- Analiz Kriterleri (collapse) -->
    <div class="settings-wrap">
      <button class="settings-toggle" id="settings-toggle" type="button">
        <span class="settings-arrow" id="settings-arrow">▶</span>
        Analiz Kriterleri
        ${state.configDelta && state.configDelta !== '{}'
          ? '<span class="settings-badge">Değiştirildi</span>' : ''}
      </button>
      <div class="settings-body hidden" id="settings-body">
        <div class="settings-grid" id="settings-grid">
          ${renderSettingsFields(parseConfigDelta(state.configDelta))}
        </div>
        <div class="settings-actions">
          <button id="btn-apply-settings" class="btn btn-primary btn-sm">Uygula</button>
          <button id="btn-reset-settings" class="btn btn-ghost btn-sm">Sıfırla</button>
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
    const label = FILE_ROW_LABEL[f.name] ?? 'satır';
    return `
      <div class="lp-file-row done">
        <span class="lp-icon">✓</span>
        <span class="lp-label">${escHtml(f.name)}</span>
        <span class="lp-status">${f.rows.toLocaleString('tr-TR')} ${label} · ${formatSize(kb)}</span>
      </div>`;
  }).join('');
}

// ── Skor kompakt panel ───────────────────────────────────────────────────────

function renderScoreCompact(result: ValidationResult): string {
  const { r5 } = result.reports;
  const cats = [
    { label: 'GTFS Geçerliliği', weight: '×40%', value: r5.spec_score,      color: '#2563eb', bg: '#eff6ff' },
    { label: 'GTFS Uyumluluğu', weight: '×30%', value: r5.interop_score,   color: '#92400e', bg: '#fef3c7' },
    { label: 'GTFS Kalitesi',   weight: '×20%', value: r5.quality_score,   color: '#15803d', bg: '#f0fdf4' },
    { label: 'GTFS Analitiği',  weight: '×10%', value: r5.analytics_score, color: '#6d28d9', bg: '#f5f3ff' },
  ];
  const qualColor = r5.score     >= 80 ? '#15803d' : r5.score     >= 60 ? '#92400e' : '#dc2626';
  const pubColor  = r5.pub_score >= 80 ? '#15803d' : r5.pub_score >= 60 ? '#d97706' : '#dc2626';

  const chips = cats.map(c =>
    `<div class="sc-chip" style="background:${c.bg};color:${c.color}">
      <div class="sc-chip-left">
        <span class="sc-chip-label">${c.label}</span>
        <span class="sc-weight">${c.weight}</span>
      </div>
      <span class="sc-chip-val">${c.value.toFixed(1)}</span>
    </div>`
  ).join('');

  return `
    <div class="sc-dual-header">
      <div class="sc-dual-item">
        <span class="sc-title">Yayın Skoru</span>
        <span class="sc-overall" style="color:${pubColor}">${r5.pub_score.toFixed(1)}<span class="sc-max">/100</span></span>
      </div>
      <div class="sc-dual-sep"></div>
      <div class="sc-dual-item">
        <span class="sc-title">Kalite Skoru</span>
        <span class="sc-overall" style="color:${qualColor}">${r5.score.toFixed(1)}<span class="sc-max">/100</span></span>
      </div>
    </div>
    <div class="sc-chips">${chips}</div>`;
}

// ── Ayarlar alanları ─────────────────────────────────────────────────────────

function renderSettingsFields(overrides: Record<string, number>): string {
  return CONFIG_FIELDS.map(f => {
    const val = overrides[f.key] !== undefined ? overrides[f.key] : f.def;
    const isOverridden = overrides[f.key] !== undefined;
    return `
      <div class="sf-row${isOverridden ? ' sf-overridden' : ''}">
        <label class="sf-label" title="${escHtml(f.desc)}">${escHtml(f.label)}</label>
        <div class="sf-control">
          <input class="sf-input" type="number" data-key="${f.key}" data-def="${f.def}"
            data-type="${f.type}" min="${f.min}" max="${f.max}"
            step="${f.type === 'int' ? 1 : 0.1}" value="${val}"/>
          <span class="sf-unit">${f.unit}</span>
          <button class="sf-reset btn btn-ghost" data-key="${f.key}" title="Sıfırla"
            ${!isOverridden ? 'style="visibility:hidden"' : ''}>✕</button>
        </div>
        <div class="sf-desc">${escHtml(f.desc)} (varsayılan: ${f.def})</div>
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

  root.querySelector('#btn-report')?.addEventListener('click', () => { setPage('domain'); renderApp(); });

  // Analiz Kriterleri collapse
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
  statusEl.textContent = 'Ayarlar kaydedildi. Yeni ZIP yükleyince uygulanır.';
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
  statusEl.textContent = 'Varsayılanlara döndürüldü.';
  statusEl.className = 'settings-status ok';
  setTimeout(() => { statusEl.className = 'settings-status hidden'; }, 3000);
}

// ── Dosya işleme ─────────────────────────────────────────────────────────────

const MAX_FILE_BYTES = 512 * 1024 * 1024; // 512 MB

async function handleFile(file: File, root: HTMLElement, errorEl: HTMLElement): Promise<void> {
  if (!file.name.endsWith('.zip')) {
    showError(errorEl, { code: 'InvalidInput', message: 'Yalnızca .zip uzantılı dosyalar kabul edilir.' });
    return;
  }
  if (file.size > MAX_FILE_BYTES) {
    showError(errorEl, {
      code: 'InvalidInput',
      message: `Dosya çok büyük (${(file.size / 1_048_576).toFixed(1)} MB). Maksimum izin verilen boyut: 512 MB.`,
    });
    return;
  }

  errorEl.className = 'upload-status hidden';
  setLoading(root, file.name, file.size);

  try {
    const buffer = await file.arrayBuffer();
    const result = await validateFile(buffer, getState().configDelta, {
      onFileList: (files) => {
        const container = root.querySelector<HTMLElement>('#file-rows');
        if (!container) return;
        container.innerHTML = '';
        files.forEach(f => {
          const row = document.createElement('div');
          row.className = 'lp-file-row reading';
          row.dataset['file'] = f.name;
          const sizeKb = Math.round(f.uncompressed_size / 1024);
          row.innerHTML = `
            <span class="lp-icon">○</span>
            <span class="lp-label">${escHtml(f.name)}</span>
            <span class="lp-status">${formatSize(sizeKb)} okunuyor…</span>`;
          container.appendChild(row);
        });
      },
      onFileDone: (name, rows, bytes) => {
        const container = root.querySelector<HTMLElement>('#file-rows');
        const row = container?.querySelector<HTMLElement>(`[data-file="${CSS.escape(name)}"]`);
        if (!row) return;
        row.className = 'lp-file-row done';
        row.querySelector('.lp-icon')!.textContent = '✓';
        const label = FILE_ROW_LABEL[name] ?? 'satır';
        row.querySelector<HTMLElement>('.lp-status')!.textContent =
          `${rows.toLocaleString('tr-TR')} ${label} · ${formatSize(Math.round(bytes / 1024))}`;
      },
      onStageDone: (stage, elapsed_ms) => {
        const stageEl = root.querySelector<HTMLElement>(`[data-stage="${CSS.escape(stage)}"]`);
        if (!stageEl) return;
        stageEl.className = 'lp-stage-row done';
        stageEl.querySelector('.lp-icon')!.textContent = '✓';
        stageEl.querySelector<HTMLElement>('.lp-status')!.textContent =
          elapsed_ms < 1000 ? `${elapsed_ms} ms` : `${(elapsed_ms / 1000).toFixed(1)} sn`;
        const idx = STAGE_ORDER.indexOf(stage);
        if (idx >= 0 && idx < STAGE_ORDER.length - 1) {
          const nextEl = root.querySelector<HTMLElement>(
            `[data-stage="${CSS.escape(STAGE_ORDER[idx + 1])}"]`);
          if (nextEl?.classList.contains('waiting')) {
            nextEl.className = 'lp-stage-row running';
            nextEl.querySelector('.lp-icon')!.textContent = '⋯';
            nextEl.querySelector<HTMLElement>('.lp-status')!.textContent = 'çalışıyor…';
          }
        }
      },
    });

    setResult(result, file.name);
    activateResult(root, result);
  } catch (err: unknown) {
    clearLoading(root);
    showError(errorEl, err as FatalError);
  }
}

function setLoading(root: HTMLElement, fileName: string, fileSize: number): void {
  const dropZone  = root.querySelector<HTMLElement>('#drop-zone')!;
  const fileInput = root.querySelector<HTMLInputElement>('#file-input')!;
  const btnLabel  = root.querySelector<HTMLElement>('#upload-btn-label')!;

  // Eski sonuç skoru yükleme sırasında görünmesin
  const scorePanel = root.querySelector<HTMLElement>('#score-panel');
  if (scorePanel) { scorePanel.classList.add('hidden'); scorePanel.innerHTML = ''; }
  const btnReport = root.querySelector<HTMLButtonElement>('#btn-report');
  if (btnReport) btnReport.disabled = true;

  dropZone.classList.add('loading');
  fileInput.disabled = true;
  btnLabel.setAttribute('aria-disabled', 'true');
  btnLabel.style.opacity = '0.5';
  btnLabel.style.pointerEvents = 'none';

  const sizeMb = (fileSize / 1_048_576).toFixed(1);
  const dropPrimary = dropZone.querySelector('.drop-primary');
  if (dropPrimary) dropPrimary.textContent = `${fileName} (${sizeMb} MB) işleniyor…`;

  const stageRows = root.querySelector<HTMLElement>('#stage-rows');
  if (stageRows) {
    stageRows.innerHTML = STAGE_ORDER.map((s, i) => `
      <div class="lp-stage-row ${i === 0 ? 'running' : 'waiting'}" data-stage="${s}">
        <span class="lp-icon">${i === 0 ? '⋯' : '○'}</span>
        <span class="lp-label">${escHtml(STAGE_LABELS[s] ?? s)}</span>
        <span class="lp-status">${i === 0 ? 'çalışıyor…' : 'bekliyor'}</span>
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
}

function activateResult(root: HTMLElement, result: ValidationResult): void {
  clearLoading(root);
  const btnLabel = root.querySelector<HTMLElement>('#upload-btn-label')!;
  btnLabel.textContent = 'Farklı GTFS Yükle';

  const scorePanel = root.querySelector<HTMLElement>('#score-panel')!;
  scorePanel.className = 'score-compact';
  scorePanel.innerHTML = renderScoreCompact(result);

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

function escHtml(s: string): string {
  return s.replace(/&/g, '&amp;').replace(/</g, '&lt;').replace(/>/g, '&gt;');
}
