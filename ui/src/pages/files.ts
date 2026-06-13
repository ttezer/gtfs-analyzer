import type { ValidationResult, Notice, FileInfo } from '../types';
import { SEVERITY_COLOR, SEVERITY_TR, t } from '../i18n';
import { setFixFileFilter, setPage } from '../state';

// GTFS zorunlu dosyalar (en az calendar.txt VEYA calendar_dates.txt gerekli)
const REQUIRED_FILES = [
  'agency.txt', 'stops.txt', 'routes.txt', 'trips.txt', 'stop_times.txt',
];
const CALENDAR_FILES = ['calendar.txt', 'calendar_dates.txt'];

// GTFS spec'te tanımlı tüm dosyalar (Rust KNOWN_FILES — k1_parse.rs:27 ile SENKRON tut).
// Spec'te OLMAYAN bir ada atıf yapan notice (ör. ARC_007 bilinmeyen dosya) feed_stats'e
// girmediği için "eksik" sanılıyordu; bu set onu "spec dışı/tanınmayan" olarak ayırır.
const GTFS_SPEC_FILES = new Set<string>([
  'agency.txt', 'stops.txt', 'routes.txt', 'trips.txt', 'stop_times.txt',
  'calendar.txt', 'calendar_dates.txt', 'shapes.txt', 'frequencies.txt',
  'transfers.txt', 'fare_attributes.txt', 'fare_rules.txt',
  'pathways.txt', 'levels.txt', 'feed_info.txt', 'translations.txt', 'attributions.txt',
  'route_networks.txt',
  // Fares v2
  'areas.txt', 'stop_areas.txt', 'networks.txt',
  'rider_categories.txt', 'fare_media.txt', 'fare_products.txt',
  'fare_leg_rules.txt', 'fare_transfer_rules.txt', 'timeframes.txt',
  // Flex
  'booking_rules.txt',
  // GTFS-JP uzantıları
  'agency_jp.txt', 'routes_jp.txt', 'office_jp.txt',
]);

const SEV_ORDER: Record<string, number> = {
  CRITICAL: 0, HIGH: 1, MEDIUM: 2, LOW: 3, INFO: 4,
};

interface FileRow {
  name: string;
  info: FileInfo | null;    // null → feed'de yok
  notices: Notice[];
  scoreDelta: number;
  missing: boolean;
}

export function renderFiles(root: HTMLElement, result: ValidationResult): void {
  const rows = buildRows(result);

  const presentNames = new Set(result.metrics.file_stats.map(f => f.name));
  const calendarMissing = !CALENDAR_FILES.some(f => presentNames.has(f));

  root.innerHTML = `
    <div class="files-page">
      <div class="files-summary-bar">
        <span class="files-summary-text">${t('files.summary', { count: rows.filter(r => !r.missing).length })}</span>
      </div>
      <div class="files-list">
        ${rows.map(r => renderFileRow(r, calendarMissing)).join('')}
      </div>
    </div>`;

  root.querySelectorAll<HTMLButtonElement>('.file-row-btn[data-file]').forEach(btn => {
    btn.addEventListener('click', () => {
      const file = btn.dataset['file']!;
      setFixFileFilter(file);
      setPage('fix');
      // main.ts'in render döngüsünü tetikle
      window.dispatchEvent(new CustomEvent('gtfs-navigate'));
    });
  });
}

function buildRows(result: ValidationResult): FileRow[] {
  const { notices, metrics, reports } = result;

  // notice.id → score_delta eşlemesi (R9 items'ten)
  const noticeDelta = new Map<string, number>();
  for (const item of reports.r9.items) {
    for (const nid of item.notice_ids) {
      noticeDelta.set(nid, (noticeDelta.get(nid) ?? 0) + item.score_delta);
    }
  }

  // dosya → notice'lar
  const fileNotices = new Map<string, Notice[]>();
  const generalNotices: Notice[] = [];
  for (const n of notices) {
    if (n.file) {
      if (!fileNotices.has(n.file)) fileNotices.set(n.file, []);
      fileNotices.get(n.file)!.push(n);
    } else {
      generalNotices.push(n);
    }
  }

  // file_stats map
  const statsMap = new Map<string, FileInfo>(
    metrics.file_stats.map(f => [f.name, f])
  );

  // tüm bilinen dosyaları topla (mevcut + notice olan + zorunlu)
  const allNames = new Set<string>([
    ...statsMap.keys(),
    ...fileNotices.keys(),
    ...REQUIRED_FILES,
    ...CALENDAR_FILES,
  ]);

  const presentSet = new Set(statsMap.keys());

  const rows: FileRow[] = [];
  for (const name of allNames) {
    const ns = fileNotices.get(name) ?? [];
    const delta = ns.reduce((sum, n) => sum + (noticeDelta.get(n.id) ?? 0), 0);
    rows.push({
      name,
      info: statsMap.get(name) ?? null,
      notices: ns,
      scoreDelta: delta,
      missing: !presentSet.has(name),
    });
  }

  // Sıralama: mevcut dosyalar (en ağır hata önce, temiz sonda), sonra eksikler
  const presentRows = rows.filter(r => !r.missing).sort((a, b) => {
    const aWorst = worstSev(a.notices);
    const bWorst = worstSev(b.notices);
    if (aWorst !== bWorst) return aWorst - bWorst;
    return b.notices.length - a.notices.length;
  });

  const missingRows = rows.filter(r => r.missing).sort((a, b) =>
    a.name.localeCompare(b.name)
  );

  // Genel notice'lar ayrı satır
  const generalDelta = generalNotices.reduce(
    (sum, n) => sum + (noticeDelta.get(n.id) ?? 0), 0
  );
  const generalRow: FileRow = {
    name: '__general__',
    info: null,
    notices: generalNotices,
    scoreDelta: generalDelta,
    missing: false,
  };

  return [...presentRows, ...(generalNotices.length > 0 ? [generalRow] : []), ...missingRows];
}

function worstSev(notices: Notice[]): number {
  if (notices.length === 0) return 99;
  return Math.min(...notices.map(n => SEV_ORDER[n.severity] ?? 99));
}

function renderFileRow(row: FileRow, calendarMissing: boolean): string {
  const isGeneral = row.name === '__general__';
  const isCalendar = CALENDAR_FILES.includes(row.name);
  const isRequired = REQUIRED_FILES.includes(row.name) || isCalendar;

  if (row.missing) {
    // Spec'te tanımlı OLMAYAN dosya (notice'tan gelmiş, ör. ARC_007): "eksik" değil,
    // "spec dışı/tanınmayan" olarak göster.
    if (!isCalendar && !GTFS_SPEC_FILES.has(row.name)) {
      return `
      <div class="file-row file-row-unknown">
        <div class="file-row-left">
          <code class="file-name">${escHtml(row.name)}</code>
          <span class="file-badge badge-unknown">${t('files.badge.unknown')}</span>
        </div>
        <div class="file-row-right">
          <span class="file-missing-hint">${t('files.hint.unknown')}</span>
        </div>
      </div>`;
    }
    // Takvim grubu için tek kayıp uyarısı yeter
    const showCalendarMissing = isCalendar && calendarMissing;
    if (isCalendar && !showCalendarMissing) return '';
    const label = isCalendar
      ? 'calendar.txt / calendar_dates.txt'
      : row.name;
    return `
      <div class="file-row file-row-missing">
        <div class="file-row-left">
          <code class="file-name">${escHtml(label)}</code>
          <span class="file-badge badge-missing">${t('files.badge.missing')}</span>
          ${isRequired ? `<span class="file-badge badge-required">${t('files.badge.required')}</span>` : ''}
        </div>
        <div class="file-row-right">
          <span class="file-missing-hint">${t('files.hint.missing')}</span>
        </div>
      </div>`;
  }

  const sevCounts: Record<string, number> = {};
  for (const n of row.notices) {
    sevCounts[n.severity] = (sevCounts[n.severity] ?? 0) + 1;
  }

  const sevBadges = (['CRITICAL', 'HIGH', 'MEDIUM', 'LOW', 'INFO'] as const)
    .filter(s => sevCounts[s])
    .map(s => `<span class="file-sev-badge" style="color:${SEVERITY_COLOR[s]}">${SEVERITY_TR[s]} ${sevCounts[s].toLocaleString('tr-TR')}</span>`)
    .join('');

  const clean = row.notices.length === 0;
  const deltaStr = row.scoreDelta < -0.005
    ? `<span class="file-delta-neg">${row.scoreDelta.toFixed(1)} puan</span>`
    : '';

  const infoStr = row.info
    ? `<span class="file-info">${row.info.rows.toLocaleString('tr-TR')} satır · ${formatBytes(row.info.bytes)}</span>`
    : '';

  const displayName = isGeneral ? t('files.general') : row.name;
  const clickable = !isGeneral && row.notices.length > 0;

  const inner = `
    <div class="file-row-left">
      <code class="file-name">${escHtml(displayName)}</code>
      ${infoStr}
    </div>
    <div class="file-row-right">
      ${clean
        ? `<span class="file-badge badge-clean">✓ ${t('files.badge.clean')}</span>`
        : sevBadges}
      ${deltaStr}
      ${clickable ? `<span class="file-row-arrow">→</span>` : ''}
    </div>`;

  if (clickable) {
    return `<button class="file-row file-row-btn" type="button" data-file="${escHtml(row.name)}">${inner}</button>`;
  }
  return `<div class="file-row ${clean ? 'file-row-clean' : ''}">${inner}</div>`;
}

function formatBytes(b: number): string {
  if (b < 1024) return `${b} B`;
  if (b < 1024 * 1024) return `${(b / 1024).toFixed(1)} KB`;
  return `${(b / 1024 / 1024).toFixed(1)} MB`;
}

function escHtml(s: string): string {
  return s.replace(/&/g, '&amp;').replace(/</g, '&lt;').replace(/>/g, '&gt;');
}
