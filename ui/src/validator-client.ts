import type { FatalError, ValidationResult } from './types';
import type { WorkerMsg } from './validator-worker';
import type { EngineMode } from './wasm';

// ── Public callback types ─────────────────────────────────────────────────────

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

// ── Internal types ─────────────────────────────────────────────────────────────

type ValidateRequest = { id: number; type: 'validate'; buffer: ArrayBuffer; configDelta: string; forceSerial?: boolean; forceMemory64?: boolean; forceWasm32?: boolean };
type RerunRequest    = { id: number; type: 'rerun';    configDelta: string };

// Disable WASM threads when the page has ?serial=1 (A/B measurement / debugging).
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
// On-demand shape request for a map icon — lightweight, with no callback/timeout required.
const shapePending = new Map<number, (coords: [number, number][]) => void>();
let   nextId  = 1;
let   worker  = createWorker();

// #15 diagnostics: the UI receives every stage message (onWorkerMessage 'stage'). Keep the
// last completed stage so it can be added to an error on crash/timeout — on large feeds,
// the WASM trap (memory access out of bounds) may occur in a rayon sub-worker and cannot be
// caught cleanly in JS; at least show which stage had completed without requiring devtools.
let lastStage = '';
function stageHint(): string {
  return lastStage ? ` (last completed stage: ${lastStage})` : '';
}
/** Variant + params for a ResourceLimit error; the `_at_stage` variant carries the last stage. */
function withStage(variant: string, params: Record<string, string>): Record<string, string> {
  return lastStage ? { ...params, variant: `${variant}_at_stage`, stage: lastStage } : { ...params, variant };
}

function createWorker(): Worker {
  const w = new Worker(new URL('./validator-worker.ts', import.meta.url), { type: 'module' });
  w.onmessage = onWorkerMessage;
  w.onerror   = onWorkerError;
  return w;
}

function onWorkerMessage(event: MessageEvent<WorkerMsg>): void {
  const msg = event.data;

  if (msg.type === 'shape-coords-result') {
    const resolve = shapePending.get(msg.id);
    if (resolve) { shapePending.delete(msg.id); resolve(msg.coords); }
    return;
  }

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
  const detail = event.message || '';
  const error: FatalError = {
    code: 'ResourceLimit',
    message: `The background validation worker stopped unexpectedly. ${detail}`.trimEnd() + stageHint(),
    params: withStage('worker_crash', { detail }),
  };
  for (const [, e] of pending) e.reject(error);
  pending.clear();
  worker.terminate();
  worker = createWorker();
}

// ── Public API ────────────────────────────────────────────────────────────────

// #15: memory no longer exceeds the cap (W1-W3 + Mode A fix); the only remaining
// limit for very large feeds is wall-clock TIME. K2/K6 can take minutes on large feeds, so 5→15 min.
const VALIDATE_TIMEOUT_MS = 15 * 60 * 1000; // 15 minutes
const VALIDATE_TIMEOUT_MIN = VALIDATE_TIMEOUT_MS / 60000;

function withTimeout(id: number, reject: (e: FatalError) => void): ReturnType<typeof setTimeout> {
  return setTimeout(() => {
    if (!pending.has(id)) return;
    pending.delete(id);
    reject({
      code: 'ResourceLimit',
      message: `Validation did not finish within ${VALIDATE_TIMEOUT_MIN} minutes. Try a smaller feed.${stageHint()}`,
      params: withStage('timeout', { minutes: String(VALIDATE_TIMEOUT_MIN) }),
    });
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

// In large-feed mode, fetch one shape's points from the worker for the map icon.
// Return an empty array when there is no cache or the shape is missing (the map opens without a shape).
export function requestShapeCoords(shapeId: string): Promise<[number, number][]> {
  const id = nextId++;
  return new Promise<[number, number][]>((resolve) => {
    shapePending.set(id, resolve);
    worker.postMessage({ id, type: 'shape-coords', shapeId });
    // Do not hang if the worker dies unexpectedly — return empty after 10 seconds.
    setTimeout(() => {
      if (shapePending.has(id)) { shapePending.delete(id); resolve([]); }
    }, 10_000);
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
