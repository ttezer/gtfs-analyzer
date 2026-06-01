/// <reference lib="webworker" />

import {
  initWasm,
  listZipFiles,
  runPrepare,
  runRerun,
  getCachedFileStats,
  extractOk,
  extractFatal,
  type CachedState,
} from './wasm';
import type { FatalError, ValidationResult } from './types';

// ── İstek tipleri ─────────────────────────────────────────────────────────────

type ValidateRequest = { id: number; type: 'validate'; buffer: ArrayBuffer; configDelta: string; forceSerial?: boolean };
type RerunRequest    = { id: number; type: 'rerun';    configDelta: string };
type WorkerRequest   = ValidateRequest | RerunRequest;

// ── Yanıt tipleri ─────────────────────────────────────────────────────────────

export type FileListMsg  = { id: number; type: 'file-list'; files: Array<{ name: string; uncompressed_size: number }> };
export type FileDoneMsg  = { id: number; type: 'file-done'; name: string; rows: number; bytes: number };
export type StageDoneMsg = { id: number; type: 'stage';     stage: string; elapsed_ms: number };
export type ResultMsg    = { id: number; type: 'result'; ok: true;  result: ValidationResult }
                         | { id: number; type: 'result'; ok: false; error: FatalError };

export type WorkerMsg = FileListMsg | FileDoneMsg | StageDoneMsg | ResultMsg;

// ── Durum ─────────────────────────────────────────────────────────────────────

let cache: CachedState | null = null;

// ── Ana mesaj dinleyici ───────────────────────────────────────────────────────

self.onmessage = async (event: MessageEvent<WorkerRequest>) => {
  const req = event.data;

  try {
    await initWasm(req.type === 'validate' ? (req.forceSerial ?? false) : false);

    if (req.type === 'validate') {
      const bytes = new Uint8Array(req.buffer);

      // 1. ZIP dosya listesi — anında
      const files = listZipFiles(bytes);
      send<FileListMsg>({ id: req.id, type: 'file-list', files });

      // 2. K1–K5 — her aşama sonrası callback ateşlenir
      const stageHandler = (stage: string, elapsed_ms: number) => {
        send<StageDoneMsg>({ id: req.id, type: 'stage', stage, elapsed_ms });
      };

      const newCache = runPrepare(bytes, stageHandler);
      cache = newCache;

      // 3. K1 bittikten sonra dosya istatistiklerini gönder
      const fileStats = getCachedFileStats(cache);
      for (const f of fileStats) {
        send<FileDoneMsg>({ id: req.id, type: 'file-done', name: f.name, rows: f.rows, bytes: f.bytes });
      }

      // 4. K6–K7
      const rawResult = runRerun(cache, req.configDelta, stageHandler);
      respondWithResult(req.id, rawResult);
      return;
    }

    // Rerun (config değişikliği)
    if (!cache) {
      send<ResultMsg>({
        id: req.id, type: 'result', ok: false,
        error: { code: 'InvalidInput', message: 'Hazır cache yok. ZIP dosyasını yeniden yükleyin.' },
      });
      return;
    }

    const stageHandler = (stage: string, elapsed_ms: number) => {
      send<StageDoneMsg>({ id: req.id, type: 'stage', stage, elapsed_ms });
    };
    respondWithResult(req.id, runRerun(cache, req.configDelta, stageHandler));

  } catch (error: unknown) {
    // runPrepare zaten FatalError objesi throw eder — ikinci kez sarma
    if (error !== null && typeof error === 'object' && 'code' in error) {
      send<ResultMsg>({ id: req.id, type: 'result', ok: false, error: error as FatalError });
      return;
    }
    const message = error instanceof Error ? error.message : String(error);
    send<ResultMsg>({ id: req.id, type: 'result', ok: false, error: { code: 'ZipUnreadable', message } });
  }
};

// ── Yardımcılar ───────────────────────────────────────────────────────────────

function send<T extends WorkerMsg>(msg: T): void {
  postMessage(msg);
}

function respondWithResult(id: number, rawResult: ReturnType<typeof runRerun>): void {
  const fatal = extractFatal(rawResult);
  if (fatal) { send<ResultMsg>({ id, type: 'result', ok: false, error: fatal }); return; }

  const ok = extractOk(rawResult);
  if (!ok) {
    send<ResultMsg>({ id, type: 'result', ok: false, error: { code: 'InvalidInput', message: 'Beklenmedik sonuç biçimi.' } });
    return;
  }
  send<ResultMsg>({ id, type: 'result', ok: true, result: ok });
}

export {};
