import type { ValidationResult } from './types';
import { logAction } from './debug-buffer';

export type AppPage = 'upload' | 'domain' | 'producer' | 'fix' | 'files' | 'file-map' | 'export' | 'compare';

export interface AppState {
  page: AppPage;
  result: ValidationResult | null;
  configDelta: string;
  fileName: string;
  fileSize: number;
  fixFileFilter: string; // filtered navigation from the files page to fix
  fixClassFilter: string; // class-filtered navigation from a score-component card to fix R2
  agencyFilter: string[]; // selected agency_ids on the fix page (empty = all)
  generatedAt: Date | null; // when the report was computed (validation/rerun)
  reportDurationMs: number | null; // wall-clock time for reading, analyzing, and generating the report
}

const state: AppState = {
  page: 'upload',
  result: null,
  configDelta: sessionStorage.getItem('gtfs-config-delta') ?? '',
  fileName: '',
  fileSize: 0,
  fixFileFilter: '',
  fixClassFilter: '',
  agencyFilter: [],
  generatedAt: null,
  reportDurationMs: null,
};

export function getState(): Readonly<AppState> { return state; }

export function setResult(result: ValidationResult, fileName: string, fileSize = 0, reportDurationMs: number | null = null): void {
  state.result = result;
  state.fileName = fileName;
  state.fileSize = fileSize;
  state.page = 'domain';
  state.generatedAt = new Date();
  state.reportDurationMs = reportDurationMs;
  state.agencyFilter = [];
  logAction('validate', `${fileName} (${fileSize} B) → ${result.notices.length} notice`);
}

export function setPage(page: AppPage): void {
  state.page = page;
  logAction('page', page);
}

export function setFixFileFilter(file: string): void {
  state.fixFileFilter = file;
}

export function setFixClassFilter(cls: string): void {
  state.fixClassFilter = cls;
}

export function setAgencyFilter(ids: string[]): void {
  state.agencyFilter = ids;
}

export function setConfigDelta(delta: string): void {
  state.configDelta = delta;
  sessionStorage.setItem('gtfs-config-delta', delta);
  logAction('config', delta ? `${delta.length} char delta` : 'cleared');
}

export function updateResult(result: ValidationResult): void {
  state.result = result;
  state.generatedAt = new Date(); // a rerun produces a new report
  logAction('rerun', `${result.notices.length} notice`);
}
