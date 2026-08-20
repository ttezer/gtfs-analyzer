/// <reference path="./node-shim.d.ts" />

import type { CachedState } from '../pkg/gtfs_wasm.js';

type BundledWasm = typeof import('../pkg/gtfs_wasm.js');
import type {
  EngineMode,
  EngineResult,
  FatalError,
  FileInfo,
  SdkVersion,
  SessionResult,
  SessionRunOptions,
  Today,
  ValidateOptions,
  ValidatorCache,
  ValidatorEngine,
  ValidatorSessionOptions,
  ValidationCallbacks,
  ValidationStage,
  ValidationResult,
  ZipFileInfo,
} from './types.js';

export type {
  EntityType,
  EngineMode,
  EngineResult,
  FatalError,
  SdkVersion,
  FatalCode,
  FeedMetrics,
  FileInfo,
  NameIndex,
  Notice,
  PartialReport,
  R1Report,
  R2Report,
  R3Report,
  R4Report,
  R5Report,
  R7Report,
  R8Report,
  R9Item,
  R9Report,
  R9Label,
  ReportId,
  ReportItem,
  ReportSet,
  RuleClass,
  Severity,
  SessionResult,
  SessionRunOptions,
  Today,
  ValidateOptions,
  ValidatorCache,
  ValidatorEngine,
  ValidatorSessionOptions,
  ValidationCallbacks,
  ValidationStage,
  ValidationResult,
  ZipFileInfo,
} from './types.js';

const SDK_VERSION = '0.1.3';
const ENGINE_VERSION = '0.9.7';

let initialization: Promise<void> | undefined;
let bundledWasmModule: BundledWasm | undefined;
let bundledWasmLoading: Promise<BundledWasm> | undefined;

/** Returns the public SDK version and the validator engine version it embeds. */
export function getVersion(): SdkVersion {
  return { sdk: SDK_VERSION, engine: ENGINE_VERSION };
}

/** Initializes the bundled WASM engine once. Safe to call more than once. */
export function initialize(): Promise<void> {
  initialization ??= initializeWasm();
  return initialization;
}

const bundledEngine: ValidatorEngine = {
  mode: 'wasm32-serial',
  initialize,
  listZipFiles: (input) => parseJson<ZipFileInfo[]>(bundledWasm().list_zip_files(input), []),
  prepare: (input, configDelta, onStage, today) => bundledWasm().prepare_with_today(input, configDelta, onStage, today),
  rerun: (cache, configDelta, onStage, today) => bundledWasm().rerun_k6_k7_with_today(cache as CachedState, configDelta, onStage, today),
  getCachedFileStats: (cache) => parseJson<FileInfo[]>(bundledWasm().get_cached_file_stats(cache as CachedState), []),
  getShapeCoords: (cache, shapeId) => parseJson<[number, number][]>(bundledWasm().shape_coords_of(cache as CachedState, shapeId), []),
};

/** Validates a GTFS ZIP with the same engine used by the Analyzer application. */
export async function validateGtfs(
  input: Uint8Array | ArrayBuffer,
  options: ValidateOptions = {},
): Promise<ValidationResult> {
  await initialize();

  const bytes = input instanceof Uint8Array ? input : new Uint8Array(input);
  const today = normalizeToday(options.today);
  const configDelta = options.config === undefined ? '' : JSON.stringify(options.config);
  const result = JSON.parse(bundledWasm().validate_with_today(bytes, configDelta, today)) as EngineResult;

  if ('Fatal' in result) {
    throw new ValidationError(toPublicFatal(result.Fatal));
  }
  return result.Ok;
}

/** Creates a stateful validator for progress callbacks and cache-backed reruns. */
export async function createValidatorSession(options: ValidatorSessionOptions = {}): Promise<ValidatorSession> {
  const engine = options.engine ?? bundledEngine;
  await engine.initialize();
  return new ValidatorSession(options, engine);
}

/**
 * Stateful validation facade. The session owns the WASM cache until `dispose()`.
 * The bundled package uses serial WASM by default, while applications can provide
 * a host-selected engine adapter for threaded or memory64 builds.
 */
export class ValidatorSession {
  readonly engineMode: EngineMode;

  private readonly today: number;
  private readonly defaultConfig?: Record<string, unknown>;
  private readonly engine: ValidatorEngine;
  private cache: ValidatorCache | undefined;
  private lastFiles: ZipFileInfo[] = [];
  private closed = false;

  constructor(options: ValidatorSessionOptions = {}, engine = options.engine ?? bundledEngine) {
    this.today = normalizeToday(options.today);
    this.defaultConfig = options.config;
    this.engine = engine;
    this.engineMode = engine.mode;
  }

  async validate(
    input: Uint8Array | ArrayBuffer,
    options: SessionRunOptions = {},
  ): Promise<SessionResult> {
    this.ensureOpen();
    await this.engine.initialize();

    this.releaseCache();
    const bytes = input instanceof Uint8Array ? input : new Uint8Array(input);
    const config = options.config ?? this.defaultConfig;
    const configDelta = serializeConfig(config);
    const files = this.engine.listZipFiles(bytes);
    options.callbacks?.onFileList?.(files);

    const onStage = makeStageCallback(options.callbacks);
    try {
      const cache = this.engine.prepare(bytes, configDelta, onStage, this.today);
      this.cache = cache;
      this.lastFiles = files;
    } catch (error: unknown) {
      throw new ValidationError(normalizeFatalError(error));
    }

    const fileStats = this.engine.getCachedFileStats(this.cache);
    for (const file of fileStats) options.callbacks?.onFileDone?.(file);

    return {
      result: this.runCached(configDelta, onStage),
      files,
      fileStats,
      engineMode: this.engineMode,
    };
  }

  /** Re-runs K6–K7 without reparsing the ZIP or rebuilding the cache. */
  async rerun(options: SessionRunOptions = {}): Promise<SessionResult> {
    this.ensureOpen();
    await this.engine.initialize();
    if (!this.cache) {
      throw new ValidationError({
        code: 'InvalidInput',
        message: 'No prepared cache. Call validate() first.',
      });
    }

    const config = options.config ?? this.defaultConfig;
    const configDelta = serializeConfig(config);
    const onStage = makeStageCallback(options.callbacks);
    return {
      result: this.runCached(configDelta, onStage),
      files: this.lastFiles,
      fileStats: this.engine.getCachedFileStats(this.cache),
      engineMode: this.engineMode,
    };
  }

  /** Gets one shape's coordinates from the live WASM cache. */
  getShapeCoords(shapeId: string): [number, number][] {
    this.ensureOpen();
    if (!this.cache) return [];
    return this.engine.getShapeCoords(this.cache, shapeId);
  }

  /** Releases the WASM cache. Safe to call more than once. */
  dispose(): void {
    if (this.closed) return;
    this.releaseCache();
    this.closed = true;
  }

  private runCached(configDelta: string, onStage: (stage: ValidationStage, elapsedMs: number) => void): ValidationResult {
    if (!this.cache) {
      throw new ValidationError({ code: 'InvalidInput', message: 'No prepared cache. Call validate() first.' });
    }
    try {
      return unwrapEngineResult(this.engine.rerun(this.cache, configDelta, onStage, this.today));
    } catch (error: unknown) {
      if (error instanceof ValidationError) throw error;
      throw new ValidationError(normalizeFatalError(error));
    }
  }

  private releaseCache(): void {
    this.cache?.free();
    this.cache = undefined;
  }

  private ensureOpen(): void {
    if (this.closed) throw new Error('ValidatorSession has been disposed. Create a new session.');
  }
}

export class ValidationError extends Error {
  readonly code: FatalError['code'];

  constructor(error: FatalError) {
    super(error.message);
    this.name = 'ValidationError';
    this.code = error.code;
  }
}

function unwrapEngineResult(raw: unknown): ValidationResult {
  const result = typeof raw === 'string'
    ? parseJson<EngineResult | null>(raw, null)
    : raw as EngineResult | null;
  if (!result || typeof result !== 'object') {
    throw new ValidationError({ code: 'InvalidInput', message: 'Unexpected validator result.' });
  }
  if ('Fatal' in result) throw new ValidationError(toPublicFatal(result.Fatal));
  if (!('Ok' in result)) {
    throw new ValidationError({ code: 'InvalidInput', message: 'Unexpected validator result.' });
  }
  return result.Ok;
}

function makeStageCallback(callbacks: ValidationCallbacks | undefined): (stage: ValidationStage, elapsedMs: number) => void {
  return (stage, elapsedMs) => callbacks?.onStageDone?.(stage, elapsedMs);
}

function serializeConfig(config: Record<string, unknown> | undefined): string {
  if (config === undefined) return '';
  try {
    return JSON.stringify(config);
  } catch (error: unknown) {
    throw new ValidationError({
      code: 'InvalidInput',
      message: `Config could not be serialized to JSON: ${error instanceof Error ? error.message : String(error)}`,
    });
  }
}

function normalizeFatalError(error: unknown): FatalError {
  if (error instanceof ValidationError) return toPublicFatal({ code: error.code, message: error.message });
  if (error !== null && typeof error === 'object' && 'code' in error) {
    const candidate = error as { code: FatalError['code']; message?: unknown };
    return toPublicFatal({ code: candidate.code, message: error instanceof Error ? error.message : String(candidate.message ?? error) });
  }

  const raw = typeof error === 'string' ? error : error instanceof Error ? error.message : String(error);
  const parsed = parseJson<EngineResult | FatalError | null>(raw, null);
  if (parsed && 'Fatal' in parsed) return toPublicFatal(parsed.Fatal);
  if (parsed && 'code' in parsed && 'message' in parsed) return toPublicFatal(parsed);

  const code: FatalError['code'] = /unreachable|RuntimeError|out of memory|memory access/i.test(raw)
    ? 'ResourceLimit'
    : 'ZipUnreadable';
  return toPublicFatal({ code, message: raw });
}

function toPublicFatal(error: FatalError): FatalError {
  return { code: error.code, message: publicFatalMessage(error.code) };
}

function publicFatalMessage(code: FatalError['code']): string {
  switch (code) {
    case 'ZipUnreadable': return 'Could not read the GTFS ZIP archive.';
    case 'Utf8Critical': return 'A critical GTFS text file is not valid UTF-8.';
    case 'NoRequiredFiles': return 'Required GTFS files are missing.';
    case 'CsvMalformed': return 'A GTFS CSV file is malformed.';
    case 'DecompressionLimit': return 'ZIP decompression exceeded the safety limit.';
    case 'ResourceLimit': return 'A memory, size, or runtime safety limit was reached.';
    case 'InvalidInput': return 'Invalid SDK input or configuration.';
  }
}

function parseJson<T>(raw: unknown, fallback: T): T {
  if (typeof raw !== 'string') return raw as T;
  try { return JSON.parse(raw) as T; } catch { return fallback; }
}

async function initializeWasm(): Promise<void> {
  bundledWasmLoading ??= loadAndInitializeWasm();
  bundledWasmModule = await bundledWasmLoading;
}

async function loadAndInitializeWasm(): Promise<BundledWasm> {
  const module = await import('../pkg/gtfs_wasm.js');
  const wasmUrl = new URL('../pkg/gtfs_wasm_bg.wasm', import.meta.url);
  if (isNodeRuntime()) {
    const nodeFsPromises = 'node:fs/promises';
    const { readFile } = await import(/* @vite-ignore */ nodeFsPromises);
    await module.default({ module_or_path: new Uint8Array(await readFile(wasmUrl)) });
  } else {
    await module.default(wasmUrl);
  }
  return module;
}

function bundledWasm(): BundledWasm {
  if (!bundledWasmModule) {
    throw new Error('WASM engine is not initialized. Call initialize() first.');
  }
  return bundledWasmModule;
}

function isNodeRuntime(): boolean {
  const runtimeProcess = (globalThis as typeof globalThis & {
    process?: { versions?: { node?: string } };
  }).process;
  return runtimeProcess?.versions?.node !== undefined;
}

function normalizeToday(value: Today | undefined): number {
  if (value === undefined) {
    const now = new Date();
    return now.getFullYear() * 10000 + (now.getMonth() + 1) * 100 + now.getDate();
  }

  const digits = typeof value === 'number'
    ? String(value)
    : value.replaceAll('-', '');
  if (!/^\d{8}$/.test(digits)) {
    throw new ValidationError({
      code: 'InvalidInput',
      message: `Invalid today value: ${value}. Expected YYYYMMDD or YYYY-MM-DD.`,
    });
  }

  const year = Number(digits.slice(0, 4));
  const month = Number(digits.slice(4, 6));
  const day = Number(digits.slice(6, 8));
  const date = new Date(Date.UTC(year, month - 1, day));
  if (
    year < 1970 || year > 9999
    || date.getUTCFullYear() !== year
    || date.getUTCMonth() !== month - 1
    || date.getUTCDate() !== day
  ) {
    throw new ValidationError({
      code: 'InvalidInput',
      message: `Invalid today value: ${value}. Expected a real calendar date.`,
    });
  }
  return year * 10000 + month * 100 + day;
}
