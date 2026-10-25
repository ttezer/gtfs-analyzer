// Üretici özeti için saf yardımcılar (render/state'ten bağımsız → doğrudan birim testlenebilir).
// Motorun sonucunu yalnız gruplar ve sıralar; yeni bulgu, skor ya da kök neden hesaplamaz.
import type { ValidationResult, Notice, NameIndex, R9Item, Severity } from '../types';

export interface RuleGroup {
  ruleId: string;
  severity: Severity;
  /** R9'un etkilenen örnek sayısı; R9 kalemi yoksa notice sayısı. */
  count: number;
  priority: number;
  notices: Notice[];
}

export interface ProducerModel {
  publishable: boolean;
  coverageComplete: boolean;
  blockers: RuleGroup[];
  improvements: RuleGroup[];
  infos: RuleGroup[];
}

export interface StopUsage { route: string; trips: number; }

function groupOf(ruleId: string, notices: Notice[], r9: Map<string, R9Item>): RuleGroup {
  const item = r9.get(ruleId);
  return {
    ruleId,
    severity: notices[0].severity,
    count: item?.affected_instance_count ?? notices.length,
    priority: item?.priority_score ?? 0,
    notices,
  };
}

/** Önceliğe göre azalan; eşitlikte motorun verdiği sıra korunur (Array.sort kararlı). */
function byPriority(a: RuleGroup, b: RuleGroup): number {
  return b.priority - a.priority;
}

export function buildProducerModel(result: ValidationResult): ProducerModel {
  const r1 = result.reports.r1;
  const r9 = new Map(result.reports.r9.items.map(i => [i.rule_id, i]));
  const byRule = new Map<string, Notice[]>();
  for (const n of result.notices) {
    const list = byRule.get(n.rule_id);
    if (list) list.push(n); else byRule.set(n.rule_id, [n]);
  }

  const blockerIds = new Set(r1.blocker_notice_ids);
  const blockerRules = new Set(result.notices.filter(n => blockerIds.has(n.id)).map(n => n.rule_id));
  const blockers = [...blockerRules]
    .map(id => groupOf(id, byRule.get(id)!.filter(n => blockerIds.has(n.id)), r9))
    .sort(byPriority);

  // Liste R9 kalemlerinden kurulur: motorun kök neden bastırması (blocks) burada zaten
  // uygulanmıştır. Ham notice'lardan gruplamak, bastırılan belirtileri geri getirirdi.
  const improvements: RuleGroup[] = [];
  const infos: RuleGroup[] = [];
  for (const item of result.reports.r9.items) {
    const notices = byRule.get(item.rule_id);
    if (!notices || blockerRules.has(item.rule_id)) continue;
    const g = groupOf(item.rule_id, notices, r9);
    (g.severity === 'INFO' ? infos : improvements).push(g);
  }
  improvements.sort(byPriority);
  infos.sort(byPriority);

  return { publishable: r1.publishable, coverageComplete: r1.coverage_complete, blockers, improvements, infos };
}

/** Bir durağı kullanan seferler, hat adına göre (NameIndex üzerinden okuma; yeni hesap değil). */
export function stopUsage(stopId: string, ni: NameIndex): StopUsage[] {
  const counts = new Map<string, number>();
  for (const [tripId, stops] of Object.entries(ni.trip_stops ?? {})) {
    if (!stops.includes(stopId)) continue;
    const routeId = ni.trip_routes?.[tripId];
    const route = (routeId && ni.routes?.[routeId]) || routeId || tripId;
    counts.set(route, (counts.get(route) ?? 0) + 1);
  }
  return [...counts].map(([route, trips]) => ({ route, trips }));
}

/** Notice'ın varlığını üreticinin tanıdığı adla söyler; ad yoksa kimliği döner. */
export function entityLabel(n: Notice, ni: NameIndex): string | null {
  const id = n.entity_id;
  if (!id) return null;
  switch (n.entity_type) {
    case 'Stop': {
      const name = ni.stops?.[id];
      return name ? `${name} (${id})` : id;
    }
    case 'Trip': {
      const routeId = ni.trip_routes?.[id];
      const route = routeId ? (ni.routes?.[routeId] || routeId) : null;
      const dep = ni.trip_first_dep?.[id];
      return [route, dep].filter(Boolean).join(' · ') || id;
    }
    case 'Route':
      return ni.routes?.[id] || id;
    default:
      return id;
  }
}
