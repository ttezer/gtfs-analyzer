import type { CachedState } from '../pkg/gtfs_wasm';
import type { ValidateResult, ValidationResult, FatalError, FileInfo } from './types';

export type { CachedState };

// İki WASM paketi de AYNI API'yi dışa verir; yalnızca threaded paket ek `initThreadPool` taşır.
// Seri paket = pkg/ (stable, tek thread). Threaded paket = pkg-threads/ (nightly+atomics, rayon).
type SerialModule = typeof import('../pkg/gtfs_wasm');
type ThreadedModule = SerialModule & { initThreadPool: (n: number) => Promise<void> };
type Memory64Module = typeof import('../pkg64/gtfs_wasm');

let ready = false;
let mod: SerialModule | null = null;
let usingThreads = false;
let usingMemory64 = false;

export type EngineMode = 'wasm64-serial' | 'wasm32-threaded' | 'wasm32-serial';

/** Doğrulamanın gerçek thread'lerle mi (paralel K6) yoksa seri mi çalıştığını döner. */
export function isThreaded(): boolean {
  return usingThreads;
}

/** Memory64 paketinin gerçekten seçili olup olmadığını döner. */
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

  // Memory64 destekleyen tarayıcıda varsayılan motor budur. Paket yüklenemezse
  // uygulama kullanılabilir kalmak için aşağıdaki wasm32 yoluna döner.
  if (!forceWasm32 && (forceMemory64 || supportsMemory64())) {
    try {
      const m64 = await import('../pkg64/gtfs_wasm') as Memory64Module;
      await m64.default();
      mod = m64 as unknown as SerialModule;
      usingThreads = false;
      usingMemory64 = true;
      ready = true;
      console.info('[WASM] wasm64 serial mode');
      return;
    } catch (err) {
      console.warn('[WASM] wasm64 init başarısız → wasm32 fallback', err);
    }
  }

  // Gerçek thread'ler yalnızca crossOriginIsolated + SharedArrayBuffer varken kurulur.
  // GitHub Pages'te bunu coi-serviceworker, dev/preview'de vite COOP/COEP başlıkları sağlar.
  // Yoksa SERİ paket yüklenir → çıktı birebir aynı (yalnızca yavaş). Bu, sistemin
  // her koşulda çalışmasını garanti eden geri-dönüş (fallback) yoludur.
  // forceSerial: A/B ölçümü/hata-ayıklama için thread'leri kapatır (sayfa ?serial=1).
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
      console.info(`[WASM] threaded mode (${n} iş parçacığı)`);
    } catch (err) {
      console.warn('[WASM] threaded init başarısız → seri fallback', err);
      mod = null;
    }
  }

  if (!mod) {
    const s = await import('../pkg/gtfs_wasm');
    await s.default();
    mod = s;
    usingThreads = false;
    usingMemory64 = false;
    console.info(canThread ? '[WASM] seri mod (threaded yüklenemedi)' : '[WASM] seri mod (cross-origin isolated değil)');
  }
  ready = true;
}

function loaded(): SerialModule {
  if (!mod) throw new Error('WASM başlatılmadı (önce initWasm çağrılmalı).');
  return mod;
}

export function runValidate(zip: Uint8Array, configDelta = ''): ValidateResult {
  return JSON.parse(loaded().validate(zip, configDelta)) as ValidateResult;
}

export function listZipFiles(zip: Uint8Array): Array<{ name: string; uncompressed_size: number }> {
  try {
    return JSON.parse(loaded().list_zip_files(zip)) as Array<{ name: string; uncompressed_size: number }>;
  } catch (err) {
    console.error('[WASM] listZipFiles başarısız:', err);
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

export function runPrepare(zip: Uint8Array, configDelta: string, onStage: (name: string, elapsedMs: number) => void): CachedState {
  const cb = (name: string, elapsedMs: number) => onStage(name, elapsedMs);
  try {
    return loaded().prepare(zip, configDelta, cb);
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
  return JSON.parse(loaded().rerun_k6_k7(cache, configDelta, cb)) as ValidateResult;
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
