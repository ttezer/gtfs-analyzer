import type { CachedState } from '../pkg/gtfs_wasm';
import type { ValidateResult, ValidationResult, FatalError, FileInfo } from './types';

export type { CachedState };

// İki WASM paketi de AYNI API'yi dışa verir; yalnızca threaded paket ek `initThreadPool` taşır.
// Seri paket = pkg/ (stable, tek thread). Threaded paket = pkg-threads/ (nightly+atomics, rayon).
type SerialModule = typeof import('../pkg/gtfs_wasm');
type ThreadedModule = SerialModule & { initThreadPool: (n: number) => Promise<void> };

let ready = false;
let mod: SerialModule | null = null;
let usingThreads = false;

/** Doğrulamanın gerçek thread'lerle mi (paralel K6) yoksa seri mi çalıştığını döner. */
export function isThreaded(): boolean {
  return usingThreads;
}

export async function initWasm(forceSerial = false): Promise<void> {
  if (ready) return;

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
