import initWasmModule, {
  validate as wasmValidate,
  prepare as wasmPrepare,
  rerun_k6_k7 as wasmRerun,
  list_zip_files as wasmListZipFiles,
  get_cached_file_stats as wasmGetCachedFileStats,
} from '../pkg/gtfs_wasm';
import type { CachedState } from '../pkg/gtfs_wasm';
import type { ValidateResult, ValidationResult, FatalError, FileInfo } from './types';

export type { CachedState };

let ready = false;

export async function initWasm(): Promise<void> {
  if (ready) return;
  await initWasmModule();
  ready = true;
}

export function runValidate(zip: Uint8Array, configDelta = ''): ValidateResult {
  return JSON.parse(wasmValidate(zip, configDelta)) as ValidateResult;
}

export function listZipFiles(zip: Uint8Array): Array<{ name: string; uncompressed_size: number }> {
  try {
    return JSON.parse(wasmListZipFiles(zip)) as Array<{ name: string; uncompressed_size: number }>;
  } catch {
    return [];
  }
}

export function getCachedFileStats(cache: CachedState): FileInfo[] {
  try {
    return JSON.parse(wasmGetCachedFileStats(cache)) as FileInfo[];
  } catch {
    return [];
  }
}

export function runPrepare(zip: Uint8Array, onStage: (name: string, elapsedMs: number) => void): CachedState {
  const cb = (name: string, elapsedMs: number) => onStage(name, elapsedMs);
  try {
    return wasmPrepare(zip, cb);
  } catch (thrown: unknown) {
    const raw = typeof thrown === 'string' ? thrown : String(thrown);
    const parsed = tryParseJson<ValidateResult>(raw);
    if (parsed && 'Fatal' in parsed) throw parsed.Fatal;
    // WASM çökmesi (OOM / unreachable / RuntimeError) → kullanıcı dostu mesaj
    const isWasmCrash = /unreachable|RuntimeError|out of memory|memory access/i.test(raw);
    const message = isWasmCrash
      ? `Feed tarayıcı için çok büyük — WASM bellek yetersiz. Daha küçük bir feed deneyin. (${raw})`
      : raw;
    throw { code: 'ZipUnreadable', message } satisfies FatalError;
  }
}

export function runRerun(
  cache: CachedState,
  configDelta = '',
  onStage: (name: string, elapsedMs: number) => void = () => {},
): ValidateResult {
  const cb = (name: string, elapsedMs: number) => onStage(name, elapsedMs);
  return JSON.parse(wasmRerun(cache, configDelta, cb)) as ValidateResult;
}

export function extractOk(result: ValidateResult): ValidationResult | null {
  if ('Ok' in result) return result.Ok;
  return null;
}

export function extractFatal(result: ValidateResult): FatalError | null {
  if ('Fatal' in result) return result.Fatal;
  return null;
}

function tryParseJson<T>(s: string): T | null {
  try { return JSON.parse(s) as T; } catch { return null; }
}
