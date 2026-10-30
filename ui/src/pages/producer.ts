// Üretici özeti: motorun mevcut sonucunu (R1 kararı, R9 önceliği, NameIndex adları) tek
// ekranda toplar. Yeni bulgu, skor ya da kök neden hesaplamaz; yalnız gruplar ve sıralar.
import type { ValidationResult, Notice } from '../types';
import { t, tMsg, tRemediation, getLocale } from '../i18n';
import { escHtml } from '../escape';
import type { MapOptions, MapPin } from '../map-modal';
import {
  buildProducerModel, entityLabel, stopUsage, plannerArea, routeBadge,
  type RuleGroup, type PlannerArea,
} from './producer-helpers';
import { renderHiddenRulesBar, attachHiddenRuleListeners } from '../hidden-rules';

function where(n: Notice): string {
  const parts = [n.file, n.line != null ? t('producer.line', { n: n.line }) : null, n.field].filter(Boolean);
  return parts.join(' · ');
}

function visibleValue(value: string | null): string {
  if (!value) return '';
  return value.replace(/ /g, '␠').replace(/\t/g, '⇥').replace(/\r?\n/g, '↵');
}

interface PlannerCardCopy {
  title: string;
  context?: string;
  what: string;
  action: string;
  observed?: string;
  usage?: string;
}

function agencyLabel(n: Notice, result: ValidationResult): string {
  const agency = result.feed_summary?.agencies?.find(a => a.agency_id === n.entity_id);
  return agency?.agency_name || n.entity_id || t('producer.card.unnamed_agency');
}

function tripContext(n: Notice, result: ValidationResult): { route?: string; headsign?: string; departure?: string } {
  const tripId = n.entity_id;
  const routeId = tripId ? result.name_index.trip_routes?.[tripId] : undefined;
  return {
    route: routeId ? result.name_index.routes?.[routeId] : undefined,
    headsign: tripId ? result.name_index.trips?.[tripId] : undefined,
    departure: tripId ? result.name_index.trip_first_dep?.[tripId] : undefined,
  };
}

function tripContextText(context: ReturnType<typeof tripContext>, includeHeadsign = true): string {
  const parts = [
    context.route ? t('producer.card.route_context', { route: context.route }) : '',
    includeHeadsign && context.headsign ? t('producer.card.headsign_context', { headsign: context.headsign }) : '',
    context.departure ? t('producer.card.departure_context', { departure: context.departure }) : '',
  ].filter(Boolean);
  return parts.join(t('producer.card.context_separator'));
}

function speedStopPair(n: Notice, result: ValidationResult): string {
  const stopA = n.details?.stop_a ? result.name_index.stops?.[n.details.stop_a] : undefined;
  const stopB = n.details?.stop_b ? result.name_index.stops?.[n.details.stop_b] : undefined;
  return stopA && stopB
    ? t('producer.card.stop_pair', { from: stopA, to: stopB })
    : t('producer.card.adjacent_stops');
}

function localizedNumber(value: string): string {
  const number = Number(value);
  return Number.isFinite(number)
    ? new Intl.NumberFormat(getLocale(), { maximumFractionDigits: 1 }).format(number)
    : value;
}

function stm012Observed(n: Notice): string | undefined {
  const details = n.details;
  const distance = details?.distance_km;
  const duration = details?.scheduled_gap_seconds;
  if (distance && duration === '0') {
    return t('producer.card.zero_duration_value', {
      distance: localizedNumber(distance),
    });
  }
  const speed = details?.speed_kmh;
  if (speed) {
    return t('producer.card.speed_value', { speed: localizedNumber(speed) });
  }
  return /^\s*\d+(?:\.\d+)?\s+km\/h\s*$/i.test(n.observed_value || '')
    ? n.observed_value || undefined
    : undefined;
}

function plannerCardCopy(g: RuleGroup, result: ValidationResult): PlannerCardCopy | null {
  const n = g.notices[0];
  switch (g.ruleId) {
    case 'STM_012': {
      const context = tripContext(n, result);
      return {
        title: t('producer.card.STM_012.title'),
        context: tripContextText(context),
        what: t('producer.card.STM_012.what', {
          stops: speedStopPair(n, result),
        }),
        action: t('producer.card.STM_012.action'),
        observed: stm012Observed(n),
      };
    }
    case 'STP_001': {
      const stop = n.entity_id || n.observed_value || '';
      return {
        title: t('producer.card.STP_001.title'),
        context: t('producer.card.STP_001.context', { stop }),
        what: t('producer.card.STP_001.what'),
        action: t('producer.card.STP_001.action'),
      };
    }
    case 'STP_012': {
      const stop = n.entity_id || n.observed_value || '';
      return {
        title: t('producer.card.STP_012.title'),
        context: t('producer.card.STP_012.context', { stop }),
        what: t('producer.card.STP_012.what'),
        action: t('producer.card.STP_012.action'),
      };
    }
    case 'STM_015': {
      const context = tripContext(n, result);
      return {
        title: t('producer.card.STM_015.title'),
        context: tripContextText(context),
        what: t('producer.card.STM_015.what'),
        action: t('producer.card.STM_015.action'),
      };
    }
    case 'STM_016': {
      const context = tripContext(n, result);
      return {
        title: t('producer.card.STM_016.title'),
        context: tripContextText(context),
        what: t('producer.card.STM_016.what'),
        action: t('producer.card.STM_016.action'),
      };
    }
    case 'ARC_031':
      return {
        title: t('producer.card.ARC_031.title'),
        what: t('producer.card.ARC_031.what'),
        action: t('producer.card.ARC_031.action'),
      };
    case 'ARC_004':
      return {
        title: t('producer.card.ARC_004.title'),
        what: t('producer.card.ARC_004.what'),
        action: t('producer.card.ARC_004.action'),
      };
    case 'ARC_008':
      return {
        title: t('producer.card.ARC_008.title'),
        what: t('producer.card.ARC_008.what'),
        action: t('producer.card.ARC_008.action'),
      };
    case 'ARC_012':
      return {
        title: t('producer.card.ARC_012.title'),
        what: t('producer.card.ARC_012.what', { file: n.file || t('producer.card.unknown_file') }),
        action: t('producer.card.ARC_012.action'),
      };
    case 'ARC_025':
      return {
        title: t('producer.card.ARC_025.title'),
        what: t('producer.card.ARC_025.what', {
          file: n.file || t('producer.card.unknown_file'),
          field: n.field || t('producer.card.unknown_field'),
        }),
        action: t('producer.card.ARC_025.action'),
      };
    case 'ARC_035':
      return {
        title: t('producer.card.ARC_035.title'),
        what: t('producer.card.ARC_035.what', { file: n.file || t('producer.card.unknown_file') }),
        action: t('producer.card.ARC_035.action'),
      };
    case 'AGN_003':
      return {
        title: t('producer.card.AGN_003.title'),
        what: t('producer.card.AGN_003.what', { agency: agencyLabel(n, result) }),
        action: t('producer.card.AGN_003.action'),
        observed: visibleValue(n.observed_value),
      };
    case 'STM_002': {
      const stop = n.details?.stop_id || n.entity_id || n.observed_value || '';
      const usage = stop
        ? stopUsage(stop, result.name_index)
          .map(u => t('producer.card.route_usage', { route: u.route, trips: u.trips }))
          .join(', ')
        : '';
      return {
        title: t('producer.card.STM_002.title'),
        context: t('producer.card.STM_002.context', { stop }),
        what: t('producer.card.STM_002.what'),
        action: t('producer.card.STM_002.action'),
        usage,
      };
    }
    case 'STM_047': {
      const routeLabels = [...new Set(g.notices
        .map(item => item.entity_id ? result.name_index.trip_routes?.[item.entity_id] : null)
        .filter((routeId): routeId is string => Boolean(routeId))
        .map(routeId => result.name_index.routes?.[routeId] || routeId))];
      const departures = [...new Set(g.notices
        .map(item => item.entity_id ? result.name_index.trip_first_dep?.[item.entity_id] : null)
        .filter((departure): departure is string => Boolean(departure)))];
      const contextParts = [
        routeLabels.length ? t('producer.card.route_context', { route: routeLabels.join(', ') }) : '',
        departures.length
          ? t('producer.card.departure_list_context', {
            departure: departures.slice(0, 3).join(', ') + (departures.length > 3 ? '…' : ''),
          })
          : '',
        t('producer.card.trip_count_context', { count: g.count }),
      ].filter(Boolean);
      return {
        title: t('producer.card.STM_047.title'),
        context: contextParts.join(t('producer.card.context_separator')),
        what: t('producer.card.STM_047.what'),
        action: t('producer.card.STM_047.action'),
      };
    }
    default:
      return null;
  }
}

type AreaFilter = 'all' | PlannerArea;
type Tier = 'blocker' | 'fix' | 'info';

const AREAS: PlannerArea[] = ['tarife', 'durak', 'hat', 'takvim', 'guzergah', 'ucret', 'kurum', 'teknik'];
const TIER_CLASS: Record<Tier, string> = { blocker: 'critical', fix: 'high', info: 'info' };
const MAP_RED = '#dc2626';
const MAP_GREY = '#64748b';
const MAP_ROUTE_LIMIT = 12;
const MAP_SHAPE_LIMIT = 24;
const ROWS_FIRST = 10;
const ROWS_STEP = 50;
const ROWS_CAP = 500;

function groupArea(g: RuleGroup): PlannerArea {
  return plannerArea(g.ruleId, g.notices[0]);
}

/** Konunun dokunduğu hatlar (route_id), bildirim sırasıyla. */
function routeIdsOf(g: RuleGroup, result: ValidationResult): string[] {
  const ni = result.name_index;
  const routes = new Set<string>();
  for (const n of g.notices) {
    const routeId = n.entity_type === 'Route'
      ? n.entity_id
      : n.entity_type === 'Trip' && n.entity_id ? ni.trip_routes?.[n.entity_id] : n.details?.route_id;
    if (routeId) routes.add(routeId);
    if (routes.size >= MAP_ROUTE_LIMIT) break;
  }
  return [...routes];
}

function routeBadgeHtml(routeId: string, result: ValidationResult): string {
  const b = routeBadge(routeId, result.name_index);
  const style = b.bg ? ` style="background:${b.bg};border-color:${b.bg};color:${b.fg}"` : '';
  return `<span class="planner-route-badge"${style}>${escHtml(b.label)}</span>`;
}

interface PlannerMap { opts: MapOptions; caption: string; }

/** Harita her kartta vardır: konunun işaret ettiği durak/bölüm kırmızı, etkilenen hatlar kendi
 *  renginde. Konuma bağlı olmayan konuda ağın tamamı gösterilir. */
function plannerMap(g: RuleGroup, result: ValidationResult): PlannerMap {
  const ni = result.name_index;
  const coord = (stopId?: string | null): [number, number] | null => {
    const p = stopId ? ni.stop_coords?.[stopId] : undefined;
    return p ? [p[0], p[1]] : null;
  };
  const routeColor = (routeId?: string) => (routeId ? routeBadge(routeId, ni).bg : null) ?? MAP_GREY;
  const lines: NonNullable<MapOptions['extraPolylines']> = [];
  const segments: NonNullable<MapOptions['extraPolylines']> = [];
  const pins: MapPin[] = [];
  const shapes = new Set<string>();
  const stops = new Set<string>();

  const addShape = (shapeId: string | null | undefined, color: string) => {
    if (!shapeId || shapes.has(shapeId) || shapes.size >= MAP_SHAPE_LIMIT) return;
    const coords = ni.shape_coords?.[shapeId];
    if (!coords || coords.length < 2) return;
    shapes.add(shapeId);
    lines.push({ coords, color, weight: 4 });
  };
  const addStop = (stopId: string | null | undefined, primary: boolean) => {
    if (!stopId || stops.has(stopId)) return;
    const p = coord(stopId);
    if (!p) return;
    stops.add(stopId);
    const name = ni.stops?.[stopId];
    pins.push({
      lat: p[0], lon: p[1], label: name ? `${name} (${stopId})` : stopId,
      primary, small: !primary, color: primary ? MAP_RED : MAP_GREY,
    });
  };

  const sample = g.notices.slice(0, ROWS_CAP);
  for (const n of sample) {
    if (n.entity_type === 'Stop') addStop(n.entity_id, true);
    addStop(n.details?.stop_id, true);
    const a = coord(n.details?.stop_a);
    const b = coord(n.details?.stop_b);
    if (a && b && segments.length < 50) {
      segments.push({ coords: [a, b], color: MAP_RED, weight: 7, zoomTo: segments.length === 0 });
      addStop(n.details?.stop_a, true);
      addStop(n.details?.stop_b, true);
    }
  }

  for (const n of sample) {
    if (n.entity_type === 'Trip' && n.entity_id) {
      addShape(ni.trip_shapes?.[n.entity_id], routeColor(ni.trip_routes?.[n.entity_id]));
    }
    const shapeId = n.entity_type === 'Shape' ? n.entity_id : n.details?.shape_id;
    if (shapeId) addShape(shapeId, routeColor(ni.shape_routes?.[shapeId]?.[0]?.[0]));
  }
  const routeIds = routeIdsOf(g, result);
  for (const routeId of routeIds) {
    for (const shapeId of (ni.route_shapes?.[routeId] ?? []).slice(0, 2)) addShape(shapeId, routeColor(routeId));
  }

  // Güzergâh çizimi olmayan seferde durak sırası çizilir; ilk seferin durakları bağlam olarak gri.
  const firstTrip = sample.find(n => n.entity_type === 'Trip' && n.entity_id)?.entity_id;
  if (firstTrip) {
    const tripStops = ni.trip_stops?.[firstTrip] ?? [];
    if (!ni.trip_shapes?.[firstTrip]) {
      const coords = tripStops.map(id => coord(id)).filter((p): p is [number, number] => p !== null);
      if (coords.length > 1) lines.push({ coords, color: routeColor(ni.trip_routes?.[firstTrip]), weight: 3 });
    }
    for (const stopId of tripStops.slice(0, 120)) addStop(stopId, false);
  }

  if (lines.length || segments.length || pins.length) {
    return { opts: { pins, extraPolylines: [...lines, ...segments] }, caption: t('producer.map.caption_focus') };
  }

  // Konuma bağlı olmayan konu: ağın tamamı.
  for (const routeId of Object.keys(ni.route_shapes ?? {}).slice(0, 40)) {
    addShape(ni.route_shapes[routeId][0], routeColor(routeId));
  }
  if (!lines.length) {
    for (const stopId of Object.keys(ni.stop_coords ?? {}).slice(0, 3000)) addStop(stopId, false);
  }
  if (!lines.length && !pins.length) return { opts: { pins: [] }, caption: t('producer.map.no_evidence') };
  return { opts: { pins, extraPolylines: lines }, caption: t('producer.map.caption_network') };
}

function technicalDetails(g: RuleGroup): string {
  const n = g.notices[0];
  const fields = [
    [t('producer.card.rule'), g.ruleId],
    [t('producer.card.location'), where(n)],
    [t('producer.card.entity'), n.entity_id],
    [t('producer.card.observed'), n.observed_value],
    [t('producer.card.expected'), n.expected_value],
  ] as Array<[string, string | null]>;
  const extra = n.details
    ? Object.entries(n.details).slice(0, 8).map(([key, value]) => [key, value] as [string, string])
    : [];
  return [...fields, ...extra]
    .map(([label, value]) => `<span>${escHtml(label)}</span><code>${escHtml(String(value))}</code>`)
    .join('');
}

/** Etkilenenler tablosu planlamacının tanıdığı sütunlarla: sefer → hat/yön/kalkış, durak → ad/kod. */
function affectedColumns(g: RuleGroup, result: ValidationResult): { head: string[]; row: (n: Notice) => string[] } {
  const ni = result.name_index;
  const dash = (v?: string | null) => v || '—';
  const first = g.notices[0];
  // Kimliksiz bildirim (feed düzeyi özet) varlık sütunlarını doldurmaz; genel sütunlara düşer.
  switch (first.entity_id ? first.entity_type : 'Feed') {
    case 'Trip':
      return {
        head: [t('producer.col.route'), t('producer.col.direction'), t('producer.col.departure')],
        row: n => {
          const routeId = n.entity_id ? ni.trip_routes?.[n.entity_id] : undefined;
          return [
            dash(routeId ? ni.routes?.[routeId] || routeId : n.entity_id),
            dash(n.entity_id ? ni.trips?.[n.entity_id] : null),
            dash(n.entity_id ? ni.trip_first_dep?.[n.entity_id] : null),
          ];
        },
      };
    case 'Stop':
      return {
        head: [t('producer.col.stop'), t('producer.col.stop_code')],
        row: n => [dash(n.entity_id ? ni.stops?.[n.entity_id] : null), dash(n.entity_id)],
      };
    case 'Route':
      return {
        head: [t('producer.col.route'), t('producer.col.route_code')],
        row: n => [dash(n.entity_id ? ni.routes?.[n.entity_id] : null), dash(n.entity_id)],
      };
    default:
      return {
        head: [t('producer.col.record'), t('producer.col.row')],
        row: n => [
          entityLabel(n, ni) || n.observed_value || t('producer.map.unknown_context'),
          n.line != null ? String(n.line) : '—',
        ],
      };
  }
}

function affectedRows(g: RuleGroup, result: ValidationResult): string {
  const cols = affectedColumns(g, result);
  const visibleRows = g.notices.slice(0, ROWS_CAP);
  const rows = visibleRows.map((n, index) => `<tr class="planner-affected-row"${index >= ROWS_FIRST ? ' hidden' : ''}>
    ${cols.row(n).map(v => `<td>${escHtml(v)}</td>`).join('')}
  </tr>`).join('');
  const more = visibleRows.length > ROWS_FIRST
    ? `<button type="button" class="planner-more" data-more-step="${ROWS_STEP}">${t('producer.card.more', { n: Math.min(ROWS_STEP, visibleRows.length - ROWS_FIRST) })}</button>`
    : '';
  const cap = g.count > visibleRows.length
    ? `<p class="planner-capnote">${escHtml(t('producer.card.cap', { n: (g.count - visibleRows.length).toLocaleString(getLocale()) }))}</p>`
    : '';
  return `<div class="planner-affected">
    <div class="planner-subhead"><span>${t('producer.card.affected')}</span><span>${g.count.toLocaleString(getLocale())}</span></div>
    <div class="planner-table-wrap"><table><thead><tr>${cols.head.map(h => `<th>${escHtml(h)}</th>`).join('')}</tr></thead><tbody>${rows || `<tr><td colspan="${cols.head.length}">${t('producer.card.no_rows')}</td></tr>`}</tbody></table></div>
    <div class="planner-affected-foot">${more}${cap}</div>
  </div>`;
}

function topicCard(g: RuleGroup, tier: Tier, result: ValidationResult, index: number): string {
  const n = g.notices[0];
  const planner = plannerCardCopy(g, result);
  const title = planner?.title || t(`rule.${g.ruleId}`);
  const what = planner?.what || tMsg(n);
  const action = planner?.action || tRemediation(n);
  const context = planner?.context || (n.entity_type === 'Feed' ? t('producer.all_feed') : '');
  const usage = planner?.usage
    ? `<p class="producer-card-usage"><span class="producer-card-label">${t('producer.card.usage')}:</span> ${escHtml(planner.usage)}</p>`
    : '';
  const observed = planner?.observed
    ? `<p class="producer-card-observed"><span class="producer-card-label">${t('producer.card.observed')}:</span> <code>${escHtml(planner.observed)}</code></p>`
    : '';
  const routes = routeIdsOf(g, result).slice(0, 8);
  return `<article class="producer-card producer-card-${TIER_CLASS[tier]}" data-rule="${escHtml(g.ruleId)}" data-area="${groupArea(g)}" data-card-index="${index}">
    <button type="button" class="planner-card-header" aria-expanded="false">
      <div>
        <h4 class="producer-card-title">${escHtml(title)}</h4>
        <div class="producer-card-kicker">
          <span class="planner-tier planner-tier-${TIER_CLASS[tier]}">${escHtml(t(`producer.tier.${tier}`))}</span>
          ${routes.map(routeId => routeBadgeHtml(routeId, result)).join('')}
        </div>
      </div>
      <span class="planner-card-impact"><strong>${g.count.toLocaleString(getLocale())}</strong><span>${t('producer.card.affected_short')}</span></span>
    </button>
    <div class="planner-card-collapsible" hidden>
      ${context ? `<p class="producer-card-context">${escHtml(context)}</p>` : ''}
      <div class="producer-card-main">
        <figure class="planner-map-wrap"><div class="planner-map" data-planner-map="${index}"><span>${t('producer.map.loading')}</span></div><figcaption class="planner-map-caption"></figcaption></figure>
        <div class="planner-card-text">
          <div class="producer-card-evidence">
            <span class="producer-card-label">${t('producer.card.what')}</span>
            <p class="producer-card-message">${escHtml(what)}</p>
            ${observed || usage ? `<div class="producer-card-facts">${observed}${usage}</div>` : ''}
          </div>
          <div class="producer-card-action">
            <span class="producer-card-label">${t('producer.card.action')}</span>
            <p class="producer-card-remediation">${escHtml(action)}</p>
          </div>
        </div>
      </div>
      ${affectedRows(g, result)}
      <details class="producer-card-details"><summary>${t('producer.card.technical')}</summary><dl>${technicalDetails(g)}</dl></details>
    </div>
  </article>`;
}

function coverageNotice(complete: boolean): string {
  if (complete) return '';
  return `<div class="producer-coverage">${t('producer.coverage_incomplete')}</div>`;
}

function feedTitle(result: ValidationResult): string {
  const info = result.feed_summary?.feed_info;
  const name = info?.feed_publisher_name || result.feed_summary?.agencies?.[0]?.agency_name || t('producer.feed_fallback');
  const version = info?.feed_version ? `<span>${escHtml(t('producer.feed_version', { v: info.feed_version }))}</span>` : '';
  return `<header class="planner-top"><strong>${escHtml(name)}</strong>${version}</header>`;
}

export function renderProducer(root: HTMLElement, result: ValidationResult): void {
  const m = buildProducerModel(result);
  const tiered: Array<[RuleGroup, Tier]> = [
    ...m.blockers.map(g => [g, 'blocker'] as [RuleGroup, Tier]),
    ...m.improvements.map(g => [g, 'fix'] as [RuleGroup, Tier]),
    ...m.infos.map(g => [g, 'info'] as [RuleGroup, Tier]),
  ];
  // Planlamacı metni yazılmış konular ve yayını durduranlar üstte; kalanlar motorun teknik
  // metniyle ayrı bölümde, iki dil aynı listede karışmasın diye.
  const primary = tiered.filter(([g, tier]) => tier === 'blocker' || plannerCardCopy(g, result) !== null);
  const technical = tiered.filter(entry => !primary.includes(entry));
  const ordered = [...primary, ...technical];
  const groups = ordered.map(([g]) => g);

  const verdict = m.publishable
    ? `<div class="planner-verdict ok"><h2>${t('producer.publishable')}</h2><p>${t('producer.planner_verdict', { fix: m.improvements.length, info: m.infos.length })}</p></div>`
    : `<div class="planner-verdict bad"><h2>${t('producer.not_publishable', { n: m.blockers.length })}</h2><p>${t('producer.not_publishable_desc')}</p></div>`;
  const counts = `<div class="planner-counts">
    <span class="planner-tier planner-tier-critical">${t('producer.count.blockers', { n: m.blockers.length })}</span>
    <span class="planner-tier planner-tier-high">${t('producer.count.passenger', { n: m.improvements.length })}</span>
    <span class="planner-tier planner-tier-info">${t('producer.count.improvement', { n: m.infos.length })}</span>
  </div>`;
  const areaNav = (['all', ...AREAS] as AreaFilter[]).map(id => {
    const count = id === 'all' ? groups.length : groups.filter(g => groupArea(g) === id).length;
    return `<button type="button" class="planner-area" data-area-filter="${id}" aria-pressed="${id === 'all'}"><span>${t(`producer.area.${id}`)}</span><span class="planner-area-count${count === 0 ? ' zero' : ''}">${count || t('producer.area.none')}</span></button>`;
  }).join('');
  const primaryCards = primary.map(([g, tier], i) => topicCard(g, tier, result, i)).join('');
  const technicalCards = technical.map(([g, tier], i) => topicCard(g, tier, result, primary.length + i)).join('');
  const technicalSection = technical.length
    ? `<details class="planner-technical"${primary.length ? '' : ' open'}><summary>${t('producer.technical.title', { n: technical.length })}</summary><p class="planner-technical-hint">${t('producer.technical.hint')}</p>${technicalCards}</details>`
    : '';

  root.innerHTML = `<section class="producer-page">
    ${feedTitle(result)}
    ${verdict}${counts}
    ${coverageNotice(m.coverageComplete)}
    <div class="planner-body">
      <aside class="planner-areas"><h3>${t('producer.where_to_fix')}</h3>${areaNav}</aside>
      <main class="planner-list">
        ${primaryCards}
        ${technicalSection}
        ${groups.length ? '' : `<div class="planner-empty">${t('producer.no_findings')}</div>`}
      </main>
    </div>
    ${renderHiddenRulesBar()}
    <p class="producer-foot">${t('producer.footer')}</p>
  </section>`;

  attachHiddenRuleListeners(root);
  let openIndex: number | null = null;
  const cleanups = new Map<number, () => void>();
  const cardAt = (index: number) => root.querySelector<HTMLElement>(`[data-card-index="${index}"]`);
  const closeCard = (card: HTMLElement, index: number) => {
    card.querySelector<HTMLElement>('.planner-card-collapsible')!.hidden = true;
    card.querySelector<HTMLButtonElement>('.planner-card-header')!.setAttribute('aria-expanded', 'false');
    cleanups.get(index)?.();
    cleanups.delete(index);
  };
  const openCard = (card: HTMLElement, index: number) => {
    if (openIndex !== null && openIndex !== index) {
      const previous = cardAt(openIndex);
      if (previous) closeCard(previous, openIndex);
    }
    card.querySelector<HTMLElement>('.planner-card-collapsible')!.hidden = false;
    card.querySelector<HTMLButtonElement>('.planner-card-header')!.setAttribute('aria-expanded', 'true');
    openIndex = index;
    const mapHost = card.querySelector<HTMLElement>('.planner-map');
    const caption = card.querySelector<HTMLElement>('.planner-map-caption');
    if (!mapHost) return;
    const map = plannerMap(groups[index], result);
    if (caption) caption.textContent = map.caption;
    if (!map.opts.extraPolylines?.length && map.opts.pins.length === 0) {
      mapHost.innerHTML = `<span>${t('producer.map.no_evidence')}</span>`;
      return;
    }
    void import('../map-modal').then(({ mountInlineMap }) => {
      if (openIndex === index && mapHost.isConnected && !cleanups.has(index)) {
        cleanups.set(index, mountInlineMap(mapHost, map.opts));
      }
    });
  };
  root.querySelectorAll<HTMLButtonElement>('.planner-card-header').forEach(header => {
    header.addEventListener('click', () => {
      const card = header.closest<HTMLElement>('.producer-card')!;
      const index = Number(card.dataset['cardIndex']);
      if (openIndex === index) { closeCard(card, index); openIndex = null; } else openCard(card, index);
    });
  });
  root.querySelectorAll<HTMLButtonElement>('.planner-area').forEach(btn => {
    btn.addEventListener('click', () => {
      const filter = btn.dataset['areaFilter']!;
      root.querySelectorAll<HTMLButtonElement>('.planner-area').forEach(b => b.setAttribute('aria-pressed', String(b === btn)));
      root.querySelectorAll<HTMLElement>('.producer-card').forEach(card => {
        card.hidden = filter !== 'all' && card.dataset['area'] !== filter;
      });
      const technicalBox = root.querySelector<HTMLDetailsElement>('.planner-technical');
      if (technicalBox) {
        technicalBox.hidden = !technicalBox.querySelector('.producer-card:not([hidden])');
      }
    });
  });
  root.querySelectorAll<HTMLButtonElement>('.planner-more').forEach(btn => {
    btn.addEventListener('click', () => {
      const affected = btn.closest<HTMLElement>('.planner-affected');
      if (!affected) return;
      const hiddenRows = Array.from(affected.querySelectorAll<HTMLTableRowElement>('.planner-affected-row[hidden]'));
      hiddenRows.slice(0, Number(btn.dataset['moreStep']) || ROWS_STEP).forEach(row => { row.hidden = false; });
      const remaining = affected.querySelectorAll('.planner-affected-row[hidden]').length;
      if (remaining === 0) btn.remove();
      else btn.textContent = t('producer.card.more', { n: Math.min(ROWS_STEP, remaining) });
    });
  });

  const first = groups.length ? cardAt(0) : null;
  if (first) openCard(first, 0);
}
