import type { FatalError, ValidationResult } from './types';
import type { WorkerMsg } from './validator-worker';
import type { EngineMode } from './wasm';

// ── Dışa açık callback tipleri ────────────────────────────────────────────────

export type OnFileList  = (files: Array<{ name: string; uncompressed_size: number }>) => void;
export type OnFileDone  = (name: string, rows: number, bytes: number) => void;
export type OnStageDone = (stage: string, elapsed_ms: number) => void;
export type OnEngine    = (mode: EngineMode) => void;

export interface ValidateCallbacks {
  onFileList?:  OnFileList;
  onFileDone?:  OnFileDone;
  onStageDone?: OnStageDone;
  onEngine?:    OnEngine;
}

// ── İç tipler ─────────────────────────────────────────────────────────────────

type ValidateRequest = { id: number; type: 'validate'; buffer: ArrayBuffer; configDelta: string; forceSerial?: boolean; forceMemory64?: boolean; forceWasm32?: boolean };
type RerunRequest    = { id: number; type: 'rerun';    configDelta: string };

// Sayfa ?serial=1 ise WASM thread'lerini kapat (A/B ölçümü / hata-ayıklama).
const FORCE_SERIAL = typeof location !== 'undefined' && /[?&]serial=1\b/.test(location.search);
const FORCE_MEMORY64 = typeof location !== 'undefined' && /[?&]wasm64=1\b/.test(location.search);
const FORCE_WASM32 = typeof location !== 'undefined' && /[?&]wasm32=1\b/.test(location.search);

let lastEngineMode: EngineMode | null = null;
export function getLastEngineMode(): EngineMode | null { return lastEngineMode; }

type PendingEntry = {
  resolve:    (v: ValidationResult) => void;
  reject:     (e: FatalError) => void;
  callbacks?: ValidateCallbacks;
};

// ── Worker ────────────────────────────────────────────────────────────────────

const pending = new Map<number, PendingEntry>();
let   nextId  = 1;
let   worker  = createWorker();

// #15 teşhis: UI her aşama mesajını alıyor (onWorkerMessage 'stage'). Son tamamlanan
// aşamayı tut ki bir çökme/timeout olduğunda hata mesajına eklenebilsin — büyük feed'de
// WASM trap'i (memory access out of bounds) rayon alt-worker'ında olup JS'te temiz
// yakalanamıyor; en azından HANGİ aşamadan sonra öldüğü devtools'suz görünsün.
let lastStage = '';
function stageHint(): string {
  return lastStage ? ` (son tamamlanan aşama: ${lastStage})` : '';
}

function createWorker(): Worker {
  const w = new Worker(new URL('./validator-worker.ts', import.meta.url), { type: 'module' });
  w.onmessage = onWorkerMessage;
  w.onerror   = onWorkerError;
  return w;
}

function onWorkerMessage(event: MessageEvent<WorkerMsg>): void {
  const msg = event.data;
  const entry = pending.get(msg.id);
  if (!entry) return;

  if (msg.type === 'file-list') {
    entry.callbacks?.onFileList?.(msg.files);
    return;
  }
  if (msg.type === 'engine') {
    lastEngineMode = msg.mode;
    entry.callbacks?.onEngine?.(msg.mode);
    return;
  }
  if (msg.type === 'file-done') {
    entry.callbacks?.onFileDone?.(msg.name, msg.rows, msg.bytes);
    return;
  }
  if (msg.type === 'stage') {
    lastStage = msg.stage;
    entry.callbacks?.onStageDone?.(msg.stage, msg.elapsed_ms);
    return;
  }
  if (msg.type === 'result') {
    pending.delete(msg.id);
    if (msg.ok) entry.resolve(msg.result);
    else        entry.reject(msg.error);
  }
}

function onWorkerError(event: ErrorEvent): void {
  const error: FatalError = {
    code: 'ResourceLimit',
    message: (event.message || 'Arka plan doğrulama işçisi beklenmedik şekilde durdu.') + stageHint(),
  };
  for (const [, e] of pending) e.reject(error);
  pending.clear();
  worker.terminate();
  worker = createWorker();
}

// ── Public API ────────────────────────────────────────────────────────────────

// #15: bellek artık tavanı aşmıyor (W1-W3 + Mode A fix); çok büyük feed'lerde tek kalan
// sınır wall-clock SÜREsi. K2/K6 büyük feed'de dakikalar sürebildiğinden 5→15 dk.
const VALIDATE_TIMEOUT_MS = 15 * 60 * 1000; // 15 dakika
const VALIDATE_TIMEOUT_MIN = VALIDATE_TIMEOUT_MS / 60000;

function withTimeout(id: number, reject: (e: FatalError) => void): ReturnType<typeof setTimeout> {
  return setTimeout(() => {
    if (!pending.has(id)) return;
    pending.delete(id);
    reject({ code: 'ResourceLimit', message: `Doğrulama ${VALIDATE_TIMEOUT_MIN} dakika içinde tamamlanamadı. Daha küçük bir feed deneyin.${stageHint()}` });
  }, VALIDATE_TIMEOUT_MS);
}

export function validateFile(
  buffer: ArrayBuffer,
  configDelta: string,
  callbacks?: ValidateCallbacks,
): Promise<ValidationResult> {
  const id = nextId++;
  return new Promise<ValidationResult>((resolve, reject) => {
    const timer = withTimeout(id, reject);
    pending.set(id, {
      resolve: (v) => { clearTimeout(timer); resolve(v); },
      reject:  (e) => { clearTimeout(timer); reject(e); },
      callbacks,
    });
    worker.postMessage({
      id, type: 'validate', buffer, configDelta,
      forceSerial: FORCE_SERIAL || FORCE_MEMORY64,
      forceMemory64: FORCE_MEMORY64,
      forceWasm32: FORCE_WASM32,
    } satisfies ValidateRequest, [buffer]);
  });
}

export function rerunValidation(
  configDelta: string,
  callbacks?: ValidateCallbacks,
): Promise<ValidationResult> {
  const id = nextId++;
  return new Promise<ValidationResult>((resolve, reject) => {
    const timer = withTimeout(id, reject);
    pending.set(id, {
      resolve: (v) => { clearTimeout(timer); resolve(v); },
      reject:  (e) => { clearTimeout(timer); reject(e); },
      callbacks,
    });
    worker.postMessage({ id, type: 'rerun', configDelta } satisfies RerunRequest);
  });
}
