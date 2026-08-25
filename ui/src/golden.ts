import type { R5Report, RuleClass, Severity, ValidationResult } from './types';

export const GOLDEN_SCHEMA = 'gtfs-analyzer-golden/5';

export interface GoldenRule { count: number; severity?: Severity; rule_class?: RuleClass; title?: string }
export interface GoldenRun {
  schema: string;
  appVersion: string;
  generatedAt: string | null;
  validateDate: string;
  engineMode: string;
  feedName: string;
  files: Record<string, { rows: number; bytes: number }>;
  metrics: Record<string, number | string | boolean | null>;
  scores: ReturnType<typeof buildGoldenScores>;
  severityCounts: Record<string, number>;
  classCounts: Record<string, number>;
  rules: Record<string, GoldenRule>;
  noticeTotal: number;
  configDelta: string;
  legacy: boolean;
}

export type ChangeKind = 'fixed' | 'decreased' | 'increased' | 'new' | 'same';
export interface RuleChange extends GoldenRule {
  id: string; before: number; now: number; delta: number; percent: number | null; kind: ChangeKind;
}

export function buildGoldenScores(r5: R5Report) {
  return {
    overall: r5.score, score: r5.score, pub_score: r5.pub_score,
    spec: r5.spec_score, interop: r5.interop_score,
    quality: r5.quality_score, analytics: r5.analytics_score,
  };
}

function actualRuleCounts(result: ValidationResult): Record<string, GoldenRule> {
  const visible = new Map<string, number>();
  const meta = new Map<string, GoldenRule>();
  for (const n of result.notices) {
    visible.set(n.rule_id, (visible.get(n.rule_id) ?? 0) + 1);
    if (!meta.has(n.rule_id)) meta.set(n.rule_id, { count: 0, severity: n.severity, rule_class: n.rule_class, title: n.title });
  }
  const ids = [...visible.keys()].sort();
  return Object.fromEntries(ids.map(id => [id, { ...meta.get(id), count: result.capped_totals[id] ?? visible.get(id)! }]));
}

export function buildGoldenSnapshot(
  result: ValidationResult, fileName: string, generatedAt: Date, configDelta: string, engineMode = 'unknown',
  opts: { deterministic?: boolean } = {},
): string {
  const rules = actualRuleCounts(result);
  const severityCounts: Record<string, number> = {};
  const classCounts: Record<string, number> = {};
  for (const rule of Object.values(rules)) {
    if (rule.severity) severityCounts[rule.severity] = (severityCounts[rule.severity] ?? 0) + rule.count;
    if (rule.rule_class) classCounts[rule.rule_class] = (classCounts[rule.rule_class] ?? 0) + rule.count;
  }
  const m = result.metrics;
  const files = Object.fromEntries([...m.file_stats].sort((a, b) => a.name.localeCompare(b.name))
    .map(f => [f.name, { rows: f.rows, bytes: f.bytes }]));
  const metrics = {
    route_count: m.route_count, stop_count: m.stop_count, trip_count: m.trip_count,
    shape_count: m.shape_count, active_service_days: m.active_service_days,
    avg_daily_trips: m.avg_daily_trips, feed_start_date: m.feed_start_date,
    feed_end_date: m.feed_end_date, service_start_date: m.service_start_date,
    service_end_date: m.service_end_date, is_gtfs_jp: m.is_gtfs_jp ?? false,
    gtfs_jp_profile: m.gtfs_jp_profile ?? null,
  };
  const iso = generatedAt.toISOString();
  // deterministic=true (git-tracked regression export): per-second `generated_at` atlanır ki
  // aynı feed+sürüm aynı gün birebir aynı byte'ları versin (temiz regresyon diff'i, #42).
  // `validate_date` (gün granülü) kalır; in-app Compare (deterministic=false) tam timestamp'i tutar.
  const golden = {
    schema: GOLDEN_SCHEMA,
    app_version: typeof __APP_VERSION__ !== 'undefined' ? __APP_VERSION__ : 'dev',
    ...(opts.deterministic ? {} : { generated_at: iso }),
    validate_date: iso.slice(0, 10),
    engine: { runtime: 'browser', mode: engineMode },
    feed: { file_name: fileName, files, metrics },
    validation: {
      config_delta: configDelta || '', scores: buildGoldenScores(result.reports.r5),
      notice_total_actual: Object.values(rules).reduce((sum, r) => sum + r.count, 0),
      severity_counts: severityCounts, class_counts: classCounts, rule_counts: rules,
    },
  };
  return JSON.stringify(golden, null, 2);
}

function num(v: unknown, fallback = 0): number { return typeof v === 'number' && Number.isFinite(v) ? v : fallback; }
function obj(v: unknown): Record<string, unknown> { return v && typeof v === 'object' && !Array.isArray(v) ? v as Record<string, unknown> : {}; }

export function parseGolden(text: string): GoldenRun {
  const root = obj(JSON.parse(text));
  const schema = String(root.schema ?? '');
  if (schema === GOLDEN_SCHEMA || schema === 'gtfs-analyzer-golden/4') {
    const feed = obj(root.feed); const validation = obj(root.validation);
    const engine = obj(root.engine);
    const rawRules = obj(validation.rule_counts);
    const rules: Record<string, GoldenRule> = {};
    for (const [id, value] of Object.entries(rawRules)) {
      const r = obj(value);
      rules[id] = { count: num(r.count), severity: r.severity as Severity | undefined,
        rule_class: r.rule_class as RuleClass | undefined, title: typeof r.title === 'string' ? r.title : undefined };
    }
    return {
      schema, appVersion: String(root.app_version ?? '?'), generatedAt: typeof root.generated_at === 'string' ? root.generated_at : null,
      validateDate: String(root.validate_date ?? ''), engineMode: String(engine.mode ?? 'unknown'), feedName: String(feed.file_name ?? ''),
      files: obj(feed.files) as unknown as GoldenRun['files'], metrics: obj(feed.metrics) as GoldenRun['metrics'],
      scores: obj(validation.scores) as unknown as GoldenRun['scores'],
      severityCounts: Object.fromEntries(Object.entries(obj(validation.severity_counts)).map(([k, v]) => [k, num(v)])),
      classCounts: Object.fromEntries(Object.entries(obj(validation.class_counts)).map(([k, v]) => [k, num(v)])),
      rules, noticeTotal: num(validation.notice_total_actual), configDelta: String(validation.config_delta ?? ''), legacy: false,
    };
  }
  if (schema === 'gtfs-analyzer-golden/3' || schema === 'gtfs-analyzer-golden/2' || schema === 'gtfs-analyzer-golden/1') {
    const counts = obj(root.rule_counts_actual);
    const rules = Object.fromEntries(Object.entries(counts).map(([id, count]) => [id, { count: num(count) }]));
    return {
      schema, appVersion: String(root.app_version ?? '?'), generatedAt: null, validateDate: String(root.validate_date ?? ''),
      engineMode: 'unknown', feedName: String(root.feed ?? ''), files: {}, metrics: obj(root.feed_metrics) as GoldenRun['metrics'],
      scores: obj(root.scores) as unknown as GoldenRun['scores'],
      severityCounts: Object.fromEntries(Object.entries(obj(root.by_severity_actual)).map(([k, v]) => [k.toUpperCase(), num(v)])),
      classCounts: {}, rules, noticeTotal: num(root.notice_total_actual), configDelta: '', legacy: true,
    };
  }
  throw new Error('Desteklenmeyen Golden JSON şeması.');
}

export function compareGolden(before: GoldenRun, now: GoldenRun): RuleChange[] {
  const ids = new Set([...Object.keys(before.rules), ...Object.keys(now.rules)]);
  return [...ids].map(id => {
    const oldRule = before.rules[id]; const newRule = now.rules[id];
    const oldCount = oldRule?.count ?? 0; const newCount = newRule?.count ?? 0; const delta = newCount - oldCount;
    const kind: ChangeKind = oldCount > 0 && newCount === 0 ? 'fixed' : oldCount === 0 && newCount > 0 ? 'new'
      : delta < 0 ? 'decreased' : delta > 0 ? 'increased' : 'same';
    return { id, before: oldCount, now: newCount, delta, percent: oldCount ? delta / oldCount * 100 : null,
      kind, count: newCount, severity: newRule?.severity ?? oldRule?.severity,
      rule_class: newRule?.rule_class ?? oldRule?.rule_class, title: newRule?.title ?? oldRule?.title };
  }).sort((a, b) => {
    const order: Record<ChangeKind, number> = { fixed: 0, decreased: 1, increased: 2, new: 3, same: 4 };
    return order[a.kind] - order[b.kind] || Math.abs(b.delta) - Math.abs(a.delta) || a.id.localeCompare(b.id);
  });
}
