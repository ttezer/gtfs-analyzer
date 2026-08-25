export type Severity = 'CRITICAL' | 'HIGH' | 'MEDIUM' | 'LOW' | 'INFO';
export type RuleClass = 'SPEC' | 'INTEROP' | 'QUALITY' | 'ANALYTICS';
export type EntityType =
  | 'Feed' | 'File' | 'Agency' | 'Stop' | 'Route' | 'Trip'
  | 'Shape' | 'Service' | 'Fare' | 'Transfer' | 'Pathway'
  | 'Level' | 'Translation' | 'Attribution' | 'Row';
export type ReportId = 'R1' | 'R2' | 'R3' | 'R4' | 'R5' | 'R7' | 'R8' | 'R9';
export type R9Label =
  | 'blocker' | 'interop' | 'propagation' | 'quick-win'
  | 'quality' | 'widespread' | 'analytics' | 'hard' | 'single' | 'high-impact';
export type FatalCode =
  | 'ZipUnreadable' | 'Utf8Critical' | 'NoRequiredFiles'
  | 'CsvMalformed' | 'DecompressionLimit' | 'ResourceLimit' | 'InvalidInput';

export interface Notice {
  id: string;
  rule_id: string;
  severity: Severity;
  rule_class: RuleClass;
  entity_type: EntityType;
  entity_id: string | null;
  scope_key: string | null;
  file: string | null;
  line: number | null;
  field: string | null;
  observed_value: string | null;
  expected_value: string | null;
  details: Record<string, string> | null;
  title: string;
  message: string;
  remediation: string;
  blocks: string[];
  base_effort: number;
  service_id?: string | null;
}

export interface ReportItem {
  notice_id: string;
  report_id: ReportId;
  display_label: string;
  is_primary: boolean;
}

export interface R1Report {
  publishable: boolean;
  coverage_complete: boolean;
  blocker_notice_ids: string[];
}
export interface R2Report { items: ReportItem[]; }
export interface R3Report { items: ReportItem[]; }
export interface R4Report { items: ReportItem[]; }
export interface R5Report {
  score: number;
  pub_score: number;
  spec_score: number;
  interop_score: number;
  quality_score: number;
  analytics_score: number;
}
export interface R7Report { items: ReportItem[]; }
export interface R8Report { items: ReportItem[]; }
export interface R9Item {
  rule_id: string;
  labels: R9Label[];
  priority_score: number;
  score_delta: number;
  pub_score_delta: number;
  affected_instance_count: number;
  realized_dependent_count: number;
  base_effort: number;
  fix_effort: number;
  notice_ids: string[];
}
export interface R9Report { items: R9Item[]; }

export interface ReportSet {
  r1: R1Report;
  r2: R2Report;
  r3: R3Report;
  r4: R4Report;
  r5: R5Report;
  r7: R7Report;
  r8: R8Report;
  r9: R9Report;
}

export interface FileInfo {
  name: string;
  rows: number;
  bytes: number;
}

export interface FeedMetrics {
  stop_count: number;
  route_count: number;
  trip_count: number;
  shape_count: number;
  active_service_days: number;
  avg_daily_trips: number;
  feed_start_date: number | null;
  feed_end_date: number | null;
  service_start_date: number | null;
  service_end_date: number | null;
  spec_notice_count: number;
  interop_notice_count: number;
  quality_notice_count: number;
  analytics_notice_count: number;
  overall_score: number;
  coverage_complete?: boolean;
  file_stats: FileInfo[];
  is_gtfs_jp?: boolean;
  /** Selected validation profile; this does not infer the feed's official v3/v4 version. */
  gtfs_jp_profile?: 'auto' | 'v3' | 'v4' | string | null;
}

export interface NameIndex {
  stops: Record<string, string>;
  routes: Record<string, string>;
  trips: Record<string, string>;
  trip_routes: Record<string, string>;
  trip_directions: Record<string, string>;
  stop_coords: Record<string, [number, number]>;
  trip_first_dep: Record<string, string>;
  shape_routes: Record<string, [string, string][]>;
  shape_coords: Record<string, [number, number][]>;
  trip_shapes: Record<string, string>;
  trip_stops: Record<string, string[]>;
  shape_trips: Record<string, string>;
  route_shapes: Record<string, string[]>;
  map_data_deferred: boolean;
}

export interface PartialReport {
  root_structural_errors: string[];
  unavailable_files: string[];
  skipped_stages: string[];
  skipped_checks?: string[];
}

export interface ValidationResult {
  validation_status?: 'COMPLETE' | 'PARTIAL';
  partial?: PartialReport;
  notices: Notice[];
  reports: ReportSet;
  metrics: FeedMetrics;
  name_index: NameIndex;
  capped_totals: Record<string, number>;
}

export interface FatalError {
  code: FatalCode;
  message: string;
  /** Optional engine diagnostic retained separately from the stable public message. */
  detail?: string;
}

export type EngineResult =
  | { Ok: ValidationResult }
  | { Fatal: FatalError };

export type Today = number | `${number}-${number}-${number}` | `${number}${number}${number}${number}${number}${number}${number}${number}`;

/** Explicit GTFS-JP validation scope; this never claims the feed's official version. */
export type GtfsJpProfile = 'auto' | 'v3' | 'v4';

/** Calendar base/override relationship used by OPR_021/022/023. */
export interface CalendarOverrideRule {
  route_id: string;
  base_service_ids: string[];
  override_service_ids: string[];
  start_date: number;
  end_date: number;
}

/** All configuration keys accepted by the Rust/WASM engine. */
export interface ValidatorConfigDelta {
  source_url?: string | null;
  gtfs_jp_profile?: GtfsJpProfile;
  stop_name_best_practices?: boolean;
  max_speed_bus_kmh?: number;
  max_speed_tram_kmh?: number;
  max_speed_metro_kmh?: number;
  max_speed_rail_kmh?: number;
  max_speed_ferry_kmh?: number;
  max_speed_cablecar_kmh?: number;
  min_transfer_time_sec?: number;
  max_transfer_distance_m?: number;
  max_shape_jump_km?: number;
  max_shape_jump_km_rail?: number;
  stop_too_close_m?: number;
  stop_far_from_shape_m?: number;
  stop_far_from_shape_m_rail?: number;
  stop_far_from_parent_m?: number;
  feed_expiry_warning_days?: number;
  feed_info_expiry_warning_days?: number;
  service_gap_days?: number;
  big_gap_days?: number;
  upcoming_service_days?: number;
  max_trip_duration_hours?: number;
  max_trip_duration_hours_rail?: number;
  min_trip_duration_sec?: number;
  max_headway_warning_min?: number;
  max_headway_warning_min_rail?: number;
  service_day_window_hours_rail?: number;
  bunching_threshold_min?: number;
  rail_stop_distance_km?: number;
  max_trips_per_route?: number;
  duration_outlier_sigma?: number;
  headway_outlier_sigma?: number;
  service_day_start_hour?: number;
  max_calendar_future_years?: number;
  rural_route_ids?: string[];
  calendar_override_rules?: CalendarOverrideRule[];
}

export interface ValidateOptions {
  /** Deterministic validation date: YYYYMMDD or YYYY-MM-DD. */
  today?: Today;
  /** Validator config delta; unknown keys are rejected by the engine. */
  config?: ValidatorConfigDelta;
}

export type EngineMode = 'wasm32-serial' | 'wasm32-threaded' | 'wasm64-serial';
export type ValidationStage = 'K1' | 'K2' | 'K3' | 'K4' | 'K5' | 'K6' | 'K7';

export interface ZipFileInfo {
  name: string;
  uncompressed_size: number;
}

export interface ValidationCallbacks {
  onFileList?: (files: ZipFileInfo[]) => void;
  onFileDone?: (file: FileInfo) => void;
  onStageDone?: (stage: ValidationStage, elapsedMs: number) => void;
}

export interface ValidatorCache {
  free(): void;
}

/** WASM engine contract used by ValidatorSession. The bundled engine is the default. */
export interface ValidatorEngine {
  readonly mode: EngineMode;
  initialize(): Promise<void>;
  listZipFiles(input: Uint8Array): ZipFileInfo[];
  prepare(
    input: Uint8Array,
    configDelta: string,
    onStage: (stage: ValidationStage, elapsedMs: number) => void,
    today: number,
  ): ValidatorCache;
  rerun(
    cache: ValidatorCache,
    configDelta: string,
    onStage: (stage: ValidationStage, elapsedMs: number) => void,
    today: number,
  ): unknown;
  getCachedFileStats(cache: ValidatorCache): FileInfo[];
  getShapeCoords(cache: ValidatorCache, shapeId: string): [number, number][];
}

export interface ValidatorSessionOptions extends ValidateOptions {
  /** Optional host-selected engine; omitted uses the bundled serial engine. */
  engine?: ValidatorEngine;
}

export interface SessionRunOptions {
  /** Validator config delta for this run. */
  config?: ValidatorConfigDelta;
  callbacks?: ValidationCallbacks;
}

export interface SessionResult {
  result: ValidationResult;
  files: ZipFileInfo[];
  fileStats: FileInfo[];
  engineMode: EngineMode;
}

export interface SdkVersion {
  sdk: string;
  engine: string;
}
