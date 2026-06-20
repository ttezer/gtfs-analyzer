import type { ValidationResult } from './types';
import { logAction } from './debug-buffer';

export type AppPage = 'upload' | 'domain' | 'fix' | 'files' | 'export';

export interface AppState {
  page: AppPage;
  result: ValidationResult | null;
  configDelta: string;
  fileName: string;
  fileSize: number;
  fixFileFilter: string; // files sayfasından fix'e filtreli geçiş için
  fixClassFilter: string; // skor bileşeni kartından fix R2'ye sınıf-filtreli geçiş için
  generatedAt: Date | null; // raporun hesaplandığı an (validasyon/yeniden çalıştırma)
}

const state: AppState = {
  page: 'upload',
  result: null,
  configDelta: sessionStorage.getItem('gtfs-config-delta') ?? '',
  fileName: '',
  fileSize: 0,
  fixFileFilter: '',
  fixClassFilter: '',
  generatedAt: null,
};

export function getState(): Readonly<AppState> { return state; }

export function setResult(result: ValidationResult, fileName: string, fileSize = 0): void {
  state.result = result;
  state.fileName = fileName;
  state.fileSize = fileSize;
  state.page = 'domain';
  state.generatedAt = new Date();
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

export function setConfigDelta(delta: string): void {
  state.configDelta = delta;
  sessionStorage.setItem('gtfs-config-delta', delta);
  logAction('config', delta ? `${delta.length} char delta` : 'cleared');
}

export function updateResult(result: ValidationResult): void {
  state.result = result;
  state.generatedAt = new Date(); // yeniden çalıştırma yeni bir rapor üretir
  logAction('rerun', `${result.notices.length} notice`);
}
