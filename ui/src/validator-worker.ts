/// <reference lib="webworker" />

import {
  createValidatorSession,
  type ValidatorSession,
} from 'gtfs-sdk';
import {
  currentToday,
  createSdkEngine,
  initWasm,
  engineMode,
  type EngineMode,
} from './wasm';
import type { FatalError, ValidationResult } from './types';

// ── İstek tipleri ─────────────────────────────────────────────────────────────

type ValidateRequest   = { id: number; type: 'validate'; buffer: ArrayBuffer; configDelta: string; forceSerial?: boolean; forceMemory64?: boolean; forceWasm32?: boolean };
type RerunRequest      = { id: number; type: 'rerun';    configDelta: string };
type ShapeCoordsRequest = { id: number; type: 'shape-coords'; shapeId: string };
type WorkerRequest     = ValidateRequest | RerunRequest | ShapeCoordsRequest;

// ── Yanıt tipleri ─────────────────────────────────────────────────────────────

export type FileListMsg  = { id: number; type: 'file-list'; files: Array<{ name: string; uncompressed_size: number }> };
export type FileDoneMsg  = { id: number; type: 'file-done'; name: string; rows: number; bytes: number };
export type StageDoneMsg = { id: number; type: 'stage';     stage: string; elapsed_ms: number };
export type EngineMsg    = { id: number; type: 'engine';    mode: EngineMode };
export type ResultMsg    = { id: number; type: 'result'; ok: true;  result: ValidationResult }
                         | { id: number; type: 'result'; ok: false; error: FatalError };
export type ShapeCoordsMsg = { id: number; type: 'shape-coords-result'; coords: [number, number][] };

export type WorkerMsg = FileListMsg | FileDoneMsg | StageDoneMsg | EngineMsg | ResultMsg | ShapeCoordsMsg;

// ── Durum ─────────────────────────────────────────────────────────────────────

let session: ValidatorSession | null = null;

// ── Ana mesaj dinleyici ───────────────────────────────────────────────────────

self.onmessage = async (event: MessageEvent<WorkerRequest>) => {
  const req = event.data;

  try {
    await initWasm(
      req.type === 'validate' ? (req.forceSerial ?? false) : false,
      req.type === 'validate' ? (req.forceMemory64 ?? false) : false,
      req.type === 'validate' ? (req.forceWasm32 ?? false) : false,
    );

    if (!session || session.engineMode !== engineMode()) {
      session?.dispose();
      session = await createValidatorSession({ today: currentToday(), engine: createSdkEngine() });
    }

    // Harita ikonu: büyük feed'de shape geometrisini on-demand çek (cache canlı).
    if (req.type === 'shape-coords') {
      const coords = session.getShapeCoords(req.shapeId);
      send<ShapeCoordsMsg>({ id: req.id, type: 'shape-coords-result', coords });
      return;
    }

    if (req.type === 'validate') {
      send<EngineMsg>({ id: req.id, type: 'engine', mode: session.engineMode });
      const bytes = new Uint8Array(req.buffer);
      const tStart = performance.now();
      const stageHandler = (stage: string, elapsed_ms: number) => {
        send<StageDoneMsg>({ id: req.id, type: 'stage', stage, elapsed_ms });
        console.log(`[time] ${stage}: +${elapsed_ms}ms — ${((performance.now() - tStart) / 1000).toFixed(1)}s toplam`);
      };
      const run = await session.validate(bytes, {
        config: parseConfigDelta(req.configDelta),
        callbacks: {
          onFileList: (files) => send<FileListMsg>({ id: req.id, type: 'file-list', files }),
          onFileDone: (file) => send<FileDoneMsg>({ id: req.id, type: 'file-done', name: file.name, rows: file.rows, bytes: file.bytes }),
          onStageDone: stageHandler,
        },
      });
      respondWithValidationResult(req.id, run.result);
      return;
    }

    // Rerun (config değişikliği)
    const tStart = performance.now();
    const stageHandler = (stage: string, elapsed_ms: number) => {
      send<StageDoneMsg>({ id: req.id, type: 'stage', stage, elapsed_ms });
      console.log(`[time] ${stage}: +${elapsed_ms}ms — ${((performance.now() - tStart) / 1000).toFixed(1)}s toplam`);
    };
    const run = await session.rerun({
      config: parseConfigDelta(req.configDelta),
      callbacks: { onStageDone: stageHandler },
    });
    respondWithValidationResult(req.id, run.result);

  } catch (error: unknown) {
    // Error subclass'larının custom alanları Worker structured clone'da kaybolabilir;
    // UI'ya her zaman düz FatalError nesnesi gönder.
    if (error !== null && typeof error === 'object' && 'code' in error) {
      const candidate = error as { code: FatalError['code']; message?: unknown };
      send<ResultMsg>({
        id: req.id,
        type: 'result',
        ok: false,
        error: {
          code: candidate.code,
          message: error instanceof Error ? error.message : String(candidate.message ?? error),
        },
      });
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

function respondWithValidationResult(id: number, result: ValidationResult): void {
  send<ResultMsg>({ id, type: 'result', ok: true, result });
}

function parseConfigDelta(configDelta: string): Record<string, unknown> | undefined {
  if (!configDelta || configDelta === '{}') return undefined;
  try {
    const parsed: unknown = JSON.parse(configDelta);
    if (parsed === null || typeof parsed !== 'object' || Array.isArray(parsed)) throw new Error('Config nesne olmalı.');
    return parsed as Record<string, unknown>;
  } catch (error: unknown) {
    throw {
      code: 'InvalidInput',
      message: `Config parse hatası: ${error instanceof Error ? error.message : String(error)}`,
    } satisfies FatalError;
  }
}

export {};
