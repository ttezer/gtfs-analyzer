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

/** Planlamacının düzeltmeyi yaptığı iş alanı. Kural önekine değil, bildirimin dosyasına bakar;
 *  arşiv/dosya yapısı (ARC) her zaman bilgi işlem konusudur. */
export type PlannerArea = 'tarife' | 'durak' | 'hat' | 'takvim' | 'guzergah' | 'ucret' | 'kurum' | 'teknik';

const FILE_AREA: Record<string, PlannerArea> = {
  'stop_times.txt': 'tarife', 'trips.txt': 'tarife', 'frequencies.txt': 'tarife', 'transfers.txt': 'tarife',
  'booking_rules.txt': 'tarife', 'location_groups.txt': 'tarife', 'location_group_stops.txt': 'tarife',
  'stops.txt': 'durak', 'pathways.txt': 'durak', 'levels.txt': 'durak', 'locations.geojson': 'durak',
  'routes.txt': 'hat',
  'calendar.txt': 'takvim', 'calendar_dates.txt': 'takvim',
  'shapes.txt': 'guzergah',
  'fare_attributes.txt': 'ucret', 'fare_rules.txt': 'ucret', 'fare_media.txt': 'ucret', 'fare_products.txt': 'ucret',
  'fare_leg_rules.txt': 'ucret', 'fare_leg_join_rules.txt': 'ucret', 'fare_transfer_rules.txt': 'ucret',
  'rider_categories.txt': 'ucret', 'timeframes.txt': 'ucret', 'areas.txt': 'ucret', 'stop_areas.txt': 'ucret',
  'networks.txt': 'ucret', 'route_networks.txt': 'ucret',
  'agency.txt': 'kurum', 'feed_info.txt': 'kurum', 'attributions.txt': 'kurum', 'translations.txt': 'kurum',
};

const ENTITY_AREA: Partial<Record<Notice['entity_type'], PlannerArea>> = {
  Stop: 'durak', Pathway: 'durak', Level: 'durak', Trip: 'tarife', Transfer: 'tarife', Route: 'hat',
  Service: 'takvim', Shape: 'guzergah', Fare: 'ucret', Agency: 'kurum', Attribution: 'kurum', Translation: 'kurum',
};

export function plannerArea(ruleId: string, n: Notice): PlannerArea {
  if (ruleId.startsWith('ARC_')) return 'teknik';
  const file = n.file?.split('/').pop()?.toLowerCase();
  if (file && FILE_AREA[file]) return FILE_AREA[file];
  return ENTITY_AREA[n.entity_type] ?? 'teknik';
}

export interface RouteBadge { label: string; bg: string | null; fg: string | null; }

const HEX6 = /^[0-9a-f]{6}$/i;

/** Hattın tarifedeki rengini okur; metin rengi yoksa arka plana göre siyah/beyaz seçer. */
export function routeBadge(routeId: string, ni: NameIndex): RouteBadge {
  const label = ni.routes?.[routeId] || routeId;
  const [bg, fg] = ni.route_colors?.[routeId] ?? ['', ''];
  if (!HEX6.test(bg)) return { label, bg: null, fg: null };
  if (HEX6.test(fg)) return { label, bg: `#${bg}`, fg: `#${fg}` };
  const [r, g, b] = [0, 2, 4].map(i => parseInt(bg.slice(i, i + 2), 16));
  const luminance = (0.299 * r + 0.587 * g + 0.114 * b) / 255;
  return { label, bg: `#${bg}`, fg: luminance > 0.6 ? '#000000' : '#ffffff' };
}
