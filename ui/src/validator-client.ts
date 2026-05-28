import type { FatalError, ValidationResult } from './types';
import type { WorkerMsg } from './validator-worker';

// ── Dışa açık callback tipleri ────────────────────────────────────────────────

export type OnFileList  = (files: Array<{ name: string; uncompressed_size: number }>) => void;
export type OnFileDone  = (name: string, rows: number, bytes: number) => void;
export type OnStageDone = (stage: string, elapsed_ms: number) => void;

export interface ValidateCallbacks {
  onFileList?:  OnFileList;
  onFileDone?:  OnFileDone;
  onStageDone?: OnStageDone;
}

// ── İç tipler ─────────────────────────────────────────────────────────────────

type ValidateRequest = { id: number; type: 'validate'; buffer: ArrayBuffer; configDelta: string };
type RerunRequest    = { id: number; type: 'rerun';    configDelta: string };

type PendingEntry = {
  resolve:    (v: ValidationResult) => void;
  reject:     (e: FatalError) => void;
  callbacks?: ValidateCallbacks;
};

// ── Worker ────────────────────────────────────────────────────────────────────

const pending = new Map<number, PendingEntry>();
let   nextId  = 1;
let   worker  = createWorker();

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
  if (msg.type === 'file-done') {
    entry.callbacks?.onFileDone?.(msg.name, msg.rows, msg.bytes);
    return;
  }
  if (msg.type === 'stage') {
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
    message: event.message || 'Arka plan doğrulama işçisi beklenmedik şekilde durdu.',
  };
  for (const [, e] of pending) e.reject(error);
  pending.clear();
  worker.terminate();
  worker = createWorker();
}

// ── Public API ────────────────────────────────────────────────────────────────

const VALIDATE_TIMEOUT_MS = 5 * 60 * 1000; // 5 dakika

function withTimeout(id: number, reject: (e: FatalError) => void): ReturnType<typeof setTimeout> {
  return setTimeout(() => {
    if (!pending.has(id)) return;
    pending.delete(id);
    reject({ code: 'ResourceLimit', message: 'Doğrulama 5 dakika içinde tamamlanamadı. Daha küçük bir feed deneyin.' });
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
    worker.postMessage({ id, type: 'validate', buffer, configDelta } satisfies ValidateRequest, [buffer]);
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
