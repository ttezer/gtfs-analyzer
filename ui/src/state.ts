import type { ValidationResult } from './types';

export type AppPage = 'upload' | 'domain' | 'fix' | 'rules' | 'export';

export interface AppState {
  page: AppPage;
  result: ValidationResult | null;
  configDelta: string;
  fileName: string;
}

const state: AppState = {
  page: 'upload',
  result: null,
  configDelta: sessionStorage.getItem('gtfs-config-delta') ?? '',
  fileName: '',
};

export function getState(): Readonly<AppState> { return state; }

export function setResult(result: ValidationResult, fileName: string): void {
  state.result = result;
  state.fileName = fileName;
  state.page = 'domain';
}

export function setPage(page: AppPage): void {
  state.page = page;
}

export function setConfigDelta(delta: string): void {
  state.configDelta = delta;
  sessionStorage.setItem('gtfs-config-delta', delta);
}

export function updateResult(result: ValidationResult): void {
  state.result = result;
}
