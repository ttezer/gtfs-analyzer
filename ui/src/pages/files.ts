import type { ValidationResult } from '../types';
import { SEVERITY_COLOR, SEVERITY_TR, t } from '../i18n';
import { setFixFileFilter, setPage } from '../state';
import {
  CALENDAR_FILES,
  buildFileSummaries,
  formatFileBytes,
  type FileSummary,
} from '../file-summary';
import { escHtml } from '../escape';

export function renderFiles(root: HTMLElement, result: ValidationResult): void {
  const rows = buildFileSummaries(result);
  const presentNames = new Set(result.metrics.file_stats.map((file) => file.name));
  const calendarMissing = !CALENDAR_FILES.some((file) => presentNames.has(file));

  root.innerHTML = `
    <div class="files-page">
      <div class="files-summary-bar">
        <span class="files-summary-text">${t('files.summary', {
          count: rows.filter((row) => !row.missing).length,
        })}</span>
      </div>
      <div class="files-list">
        ${rows.map((row) => renderFileRow(row, calendarMissing)).join('')}
      </div>
    </div>`;

  root.querySelectorAll<HTMLButtonElement>('.file-row-btn[data-file]').forEach((button) => {
    button.addEventListener('click', () => {
      setFixFileFilter(button.dataset['file'] ?? '');
      setPage('fix');
      window.dispatchEvent(new CustomEvent('gtfs-navigate'));
    });
  });
}

function renderFileRow(row: FileSummary, calendarMissing: boolean): string {
  const isCalendar = CALENDAR_FILES.some((file) => file === row.name);

  if (row.unknown) {
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

  if (row.missing) {
    if (isCalendar && !calendarMissing) return '';
    const label = isCalendar
      ? 'calendar.txt / calendar_dates.txt'
      : row.name;
    return `
      <div class="file-row file-row-missing">
        <div class="file-row-left">
          <code class="file-name">${escHtml(label)}</code>
          <span class="file-badge badge-missing">${t('files.badge.missing')}</span>
          ${row.required ? `<span class="file-badge badge-required">${t('files.badge.required')}</span>` : ''}
        </div>
        <div class="file-row-right">
          <span class="file-missing-hint">${t('files.hint.missing')}</span>
        </div>
      </div>`;
  }

  const severityBadges = (['CRITICAL', 'HIGH', 'MEDIUM', 'LOW', 'INFO'] as const)
    .filter((severity) => row.severityCounts[severity] > 0)
    .map((severity) => `
      <span class="file-sev-badge" style="color:${SEVERITY_COLOR[severity]}">
        ${SEVERITY_TR[severity]} ${row.severityCounts[severity].toLocaleString('tr-TR')}
      </span>`)
    .join('');

  const clean = row.notices.length === 0;
  const scoreDelta = row.scoreDelta < -0.005
    ? `<span class="file-delta-neg">${row.scoreDelta.toFixed(1)} puan</span>`
    : '';
  const fileInfo = row.info
    ? `<span class="file-info">${row.info.rows.toLocaleString('tr-TR')} satir - ${formatFileBytes(row.info.bytes)}</span>`
    : '';
  const displayName = row.general ? t('files.general') : row.name;
  const clickable = !row.general && row.notices.length > 0;

  const content = `
    <div class="file-row-left">
      <code class="file-name">${escHtml(displayName)}</code>
      ${fileInfo}
    </div>
    <div class="file-row-right">
      ${clean
        ? `<span class="file-badge badge-clean">${t('files.badge.clean')}</span>`
        : severityBadges}
      ${scoreDelta}
      ${clickable ? '<span class="file-row-arrow">&rarr;</span>' : ''}
    </div>`;

  if (clickable) {
    return `<button class="file-row file-row-btn" type="button" data-file="${escHtml(row.name)}">${content}</button>`;
  }
  return `<div class="file-row ${clean ? 'file-row-clean' : ''}">${content}</div>`;
}
