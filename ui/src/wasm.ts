import type { CachedState } from '../pkg/gtfs_wasm';
import type { ValidatorEngine, ValidationStage } from 'gtfs-sdk';
import type { ValidateResult, ValidationResult, FatalError, FileInfo } from './types';

export type { CachedState };

// Both WASM packages export the SAME API; only the threaded package also exposes `initThreadPool`.
// Serial package = pkg/ (stable, one thread). Threaded package = pkg-threads/ (nightly+atomics, rayon).
type SerialModule = typeof import('../pkg/gtfs_wasm');
type DeterministicModule = SerialModule & {
  prepare_with_today: (
    zip: Uint8Array,
    configDelta: string,
    onStage: Function,
    today: number,
  ) => CachedState;
  rerun_k6_k7_with_today: (
    cache: CachedState,
    configDelta: string,
    onStage: Function,
    today: number,
  ) => string;
};
type ThreadedModule = DeterministicModule & { initThreadPool: (n: number) => Promise<void> };
type Memory64Module = DeterministicModule;

let ready = false;
let mod: DeterministicModule | null = null;
let usingThreads = false;
let usingMemory64 = false;

export type EngineMode = 'wasm64-serial' | 'wasm32-threaded' | 'wasm32-serial';

// Large feeds must reach the WASM64 path; 512 MiB rejected valid feeds before
// the analyzer could inspect their ZIP directory. Keep a finite safety bound
// while allowing the 1.28 GiB mdb-2014 feed and similar inputs.
export const MAX_INPUT_BYTES = 2 * 1024 * 1024 * 1024;

function assertInputLimit(zip: Uint8Array): void {
  if (zip.byteLength > MAX_INPUT_BYTES) {
    throw {
      code: 'ResourceLimit',
      message: 'Input ZIP exceeds the SDK safety limit.',
    } satisfies FatalError;
  }
}

/** Return whether validation runs with real threads (parallel K6) or serially. */
export function isThreaded(): boolean {
  return usingThreads;
}

/** Return whether the Memory64 package is actually selected. */
export function isMemory64(): boolean {
  return usingMemory64;
}

export function engineMode(): EngineMode {
  if (usingMemory64) return 'wasm64-serial';
  return usingThreads ? 'wasm32-threaded' : 'wasm32-serial';
}

function supportsMemory64(): boolean {
  try {
    const descriptor = { address: 'i64', initial: 1n, maximum: 1n };
    new WebAssembly.Memory(descriptor as unknown as WebAssembly.MemoryDescriptor);
    return true;
  } catch {
    return false;
  }
}

export async function initWasm(forceSerial = false, forceMemory64 = false, forceWasm32 = false): Promise<void> {
  if (ready) return;

  // This is the default engine in a browser that supports Memory64. If the package
  // cannot be loaded, fall back to the wasm32 path below so the app remains usable.
  if (!forceWasm32 && (forceMemory64 || supportsMemory64())) {
    try {
      const m64 = await import('../pkg64/gtfs_wasm') as Memory64Module;
      await m64.default();
      mod = m64;
      usingThreads = false;
      usingMemory64 = true;
      ready = true;
      console.info('[WASM] wasm64 serial mode');
      return;
    } catch (err) {
      console.warn('[WASM] wasm64 init failed → wasm32 fallback', err);
    }
  }

  // Real threads are enabled only when crossOriginIsolated + SharedArrayBuffer are available.
  // On GitHub Pages, coi-serviceworker provides this; Vite COOP/COEP headers do so in dev/preview.
  // Otherwise load the SERIAL package → output is byte-for-byte identical (only slower). This
  // fallback guarantees that the system works under all conditions.
  // forceSerial: disables threads for A/B measurement/debugging (page ?serial=1).
  const isolated = (globalThis as unknown as { crossOriginIsolated?: boolean }).crossOriginIsolated === true;
  const canThread = !forceSerial && typeof SharedArrayBuffer !== 'undefined' && isolated;

  if (canThread) {
    try {
      const t = (await import('../pkg-threads/gtfs_wasm')) as unknown as ThreadedModule;
      await t.default();
      const hw = (navigator as Navigator & { hardwareConcurrency?: number }).hardwareConcurrency || 4;
      const n = Math.max(1, Math.min(hw, 8));
      await t.initThreadPool(n);
      mod = t;
      usingThreads = true;
      usingMemory64 = false;
      console.info(`[WASM] threaded mode (${n} threads)`);
    } catch (err) {
      console.warn('[WASM] threaded init failed → serial fallback', err);
      mod = null;
    }
  }

  if (!mod) {
    const s = await import('../pkg/gtfs_wasm') as unknown as DeterministicModule;
    await s.default();
    mod = s;
    usingThreads = false;
    usingMemory64 = false;
    console.info(canThread ? '[WASM] serial mode (threaded build failed to load)' : '[WASM] serial mode (not cross-origin isolated)');
  }
  ready = true;
}

function loaded(): DeterministicModule {
  if (!mod) throw new Error('WASM is not initialized (call initWasm first).');
  return mod;
}

export function currentToday(): number {
  const now = new Date();
  return now.getFullYear() * 10000 + (now.getMonth() + 1) * 100 + now.getDate();
}

export function runValidate(zip: Uint8Array, configDelta = ''): ValidateResult {
  assertInputLimit(zip);
  return JSON.parse(loaded().validate(zip, configDelta)) as ValidateResult;
}

export function listZipFiles(zip: Uint8Array): Array<{ name: string; uncompressed_size: number }> {
  assertInputLimit(zip);
  try {
    return JSON.parse(loaded().list_zip_files(zip)) as Array<{ name: string; uncompressed_size: number }>;
  } catch (err) {
    console.error('[WASM] listZipFiles failed:', err);
    return [];
  }
}

export function getCachedFileStats(cache: CachedState): FileInfo[] {
  try {
    return JSON.parse(loaded().get_cached_file_stats(cache)) as FileInfo[];
  } catch {
    return [];
  }
}

// In large-feed mode, fetch one shape's [[lat, lon]] points on demand.
export function shapeCoordsOf(cache: CachedState, shapeId: string): [number, number][] {
  try {
    return JSON.parse(loaded().shape_coords_of(cache, shapeId)) as [number, number][];
  } catch {
    return [];
  }
}

export function runPrepare(zip: Uint8Array, configDelta: string, onStage: (name: string, elapsedMs: number) => void): CachedState {
  assertInputLimit(zip);
  const cb = (name: string, elapsedMs: number) => onStage(name, elapsedMs);
  try {
    return loaded().prepare(zip, configDelta, cb);
  } catch (thrown: unknown) {
    const raw = typeof thrown === 'string' ? thrown : String(thrown);
    const parsed = tryParseJson<ValidateResult>(raw);
    if (parsed && 'Fatal' in parsed) throw parsed.Fatal;
    // WASM crash (OOM / unreachable / RuntimeError) → user-friendly message
    const isWasmCrash = /unreachable|RuntimeError|out of memory|memory access/i.test(raw);
    const message = isWasmCrash
      ? `Feed tarayıcı için çok büyük — WASM bellek yetersiz. Daha küçük bir feed deneyin. (${raw})`
      : raw;
    throw { code: 'ZipUnreadable', message } satisfies FatalError;
  }
}

export function runPrepareWithToday(
  zip: Uint8Array,
  configDelta: string,
  onStage: (stage: ValidationStage, elapsedMs: number) => void,
  today: number,
): CachedState {
  assertInputLimit(zip);
  const cb = (name: string, elapsedMs: number) => onStage(name as ValidationStage, elapsedMs);
  try {
    return loaded().prepare_with_today(zip, configDelta, cb, today);
  } catch (thrown: unknown) {
    const raw = typeof thrown === 'string' ? thrown : String(thrown);
    const parsed = tryParseJson<ValidateResult>(raw);
    if (parsed && 'Fatal' in parsed) throw parsed.Fatal;
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
  return JSON.parse(loaded().rerun_k6_k7(cache, configDelta, cb)) as ValidateResult;
}

export function runRerunWithToday(
  cache: CachedState,
  configDelta: string,
  onStage: (stage: ValidationStage, elapsedMs: number) => void,
  today: number,
): ValidateResult {
  const cb = (name: string, elapsedMs: number) => onStage(name as ValidationStage, elapsedMs);
  return JSON.parse(loaded().rerun_k6_k7_with_today(cache, configDelta, cb, today)) as ValidateResult;
}

/** Adapts the UI's selected WASM variant to the public SDK session contract. */
export function createSdkEngine(): ValidatorEngine {
  return {
    mode: engineMode(),
    initialize: () => initWasm(),
    listZipFiles,
    prepare: runPrepareWithToday,
    rerun: runRerunWithToday,
    getCachedFileStats,
    getShapeCoords: shapeCoordsOf,
  };
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
