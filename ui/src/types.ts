/* Rust WASM JSON sözleşmesinin TypeScript karşılıkları.
 * Kaynak: crates/core/src/{enums,notice,reports,metrics,result}.rs
 * serde rename değerleri: Severity → İngilizce sabit kodlar, R9Label → snake_case.
 */

export type Severity   = 'CRITICAL' | 'HIGH' | 'MEDIUM' | 'LOW' | 'INFO';
export type RuleClass  = 'SPEC' | 'INTEROP' | 'QUALITY' | 'ANALYTICS';
export type EntityType =
  | 'Feed' | 'File' | 'Agency' | 'Stop' | 'Route' | 'Trip'
  | 'Shape' | 'Service' | 'Fare' | 'Transfer' | 'Pathway'
  | 'Level' | 'Translation' | 'Attribution' | 'Row';
export type DedupLevel = 'Feed' | 'File' | 'Entity' | 'Row' | 'Field';
export type FatalCode  =
  | 'ZipUnreadable' | 'Utf8Critical' | 'NoRequiredFiles'
  | 'CsvMalformed'  | 'ResourceLimit' | 'InvalidInput';
export type R9Label    =
  | 'blocker' | 'conditional_blocker' | 'interop' | 'propagation' | 'quick-win'
  | 'quality' | 'widespread';
export type ReportId   = 'R1' | 'R2' | 'R3' | 'R4' | 'R5' | 'R7' | 'R8' | 'R9';

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
}

export interface ReportItem {
  notice_id: string;
  report_id: ReportId;
  display_label: string;
  is_primary: boolean;
}

export interface R1Report {
  publishable: boolean;
  conditional: boolean;
  blocker_notice_ids: string[];
  conditional_blocker_notice_ids: string[];
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
  spec_notice_count: number;
  interop_notice_count: number;
  quality_notice_count: number;
  analytics_notice_count: number;
  quality_score: number;
  file_stats: FileInfo[];
}

export interface NameIndex {
  stops: Record<string, string>;                   // stop_id → stop_name
  routes: Record<string, string>;                  // route_id → short_name or long_name
  trips: Record<string, string>;                   // trip_id → trip_headsign
  trip_routes: Record<string, string>;             // trip_id → route_id
  stop_coords: Record<string, [number, number]>;    // stop_id → [lat, lon]
  trip_first_dep: Record<string, string>;           // trip_id → "HH:MM"
  shape_routes: Record<string, [string, string][]>; // shape_id → [[route_id, yön]]
  shape_coords: Record<string, [number, number][]>; // shape_id → [[lat, lon], ...] sıralı
  trip_shapes: Record<string, string>;              // trip_id → shape_id
  trip_stops: Record<string, string[]>;             // trip_id → [stop_id, ...] sıralı
  shape_trips: Record<string, string>;              // shape_id → ilk trip_id
  route_shapes: Record<string, string[]>;           // route_id → [shape_id, ...]
}

export interface ValidationResult {
  notices: Notice[];
  reports: ReportSet;
  metrics: FeedMetrics;
  name_index: NameIndex;
}

export interface FatalError {
  code: FatalCode;
  message: string;
}

export type ValidateResult =
  | { Ok: ValidationResult }
  | { Fatal: FatalError };
