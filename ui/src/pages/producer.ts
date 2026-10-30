// Üretici özeti: motorun mevcut sonucunu (R1 kararı, R9 önceliği, NameIndex adları) tek
// ekranda toplar. Yeni bulgu, skor ya da kök neden hesaplamaz; yalnız gruplar ve sıralar.
import type { ValidationResult, Notice, Severity } from '../types';
import { t, tMsg, tRemediation, getLocale, intlLocale, severityForLocale, SEVERITY_COLOR, RULE_CLASS_TR } from '../i18n';
import { SEVERITY_ORDER } from '../severity-order';
import { escHtml } from '../escape';
import type { MapOptions, MapPin } from '../map-modal';
import {
  buildProducerModel, entityLabel, stopUsage, plannerArea, routeBadge,
  type RuleGroup, type PlannerArea,
} from './producer-helpers';
import { renderHiddenRulesBar, attachHiddenRuleListeners } from '../hidden-rules';
import { attachAgencyFilterListeners, filterNoticesByAgency, renderAgencyFilterBar } from '../agency-filter';
import { getState, setAgencyFilter } from '../state';

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

/** 0,0 (Gine Körfezi) — konum girilmemiş durakların yaygın değeri; gerçek bir durak değildir. */
function isNullIsland(p?: [number, number] | null): boolean {
  return Boolean(p && Math.abs(p[0]) < 0.01 && Math.abs(p[1]) < 0.01);
}

function plannerCardCopy(g: RuleGroup, result: ValidationResult): PlannerCardCopy | null {
  const n = g.notices[0];
  switch (g.ruleId) {
    case 'STM_012': {
      const context = tripContext(n, result);
      // Duraklardan biri 0,0 konumundaysa hız o yüzden uçuyor; asıl düzeltilecek şey konum.
      const zeroStop = [n.details?.stop_a, n.details?.stop_b].find(id => isNullIsland(result.name_index.stop_coords?.[id ?? '']));
      if (zeroStop) {
        return {
          title: t('producer.card.STM_012.title'),
          context: tripContextText(context),
          what: t('producer.card.null_island', { stop: result.name_index.stops?.[zeroStop] || zeroStop, id: zeroStop }),
          action: t('producer.card.null_island_action'),
          observed: stm012Observed(n),
        };
      }
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
    case 'STP_009': {
      const stop = n.entity_id ? result.name_index.stops?.[n.entity_id] || n.entity_id : '';
      return {
        title: t('producer.card.STP_009.title'),
        context: t('producer.card.STP_009.context', { stop, parent: n.observed_value || '—' }),
        what: t('producer.card.STP_009.what'),
        action: t('producer.card.STP_009.action'),
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
    case 'STM_015':
    case 'STM_016': {
      const context = tripContext(n, result);
      const stopId = n.details?.stop_id;
      const stop = stopId ? result.name_index.stops?.[stopId] || stopId : '';
      // Seferin hiçbir durağında saat yoksa sorun tek bir uç durak değil, saatsiz aktarılmış seferdir.
      const untimed = n.details?.timed_stops === '0';
      return {
        title: t(`producer.card.${g.ruleId}.title`),
        context: tripContextText(context),
        what: untimed
          ? t('producer.card.trip_untimed', { n: n.details?.stop_count ?? '?' })
          : stop
            ? t(`producer.card.${g.ruleId}.what_at`, { stop })
            : t(`producer.card.${g.ruleId}.what`),
        action: untimed ? t('producer.card.trip_untimed_action') : t(`producer.card.${g.ruleId}.action`),
      };
    }
    case 'STM_008': {
      const context = tripContext(n, result);
      const d = n.details ?? {};
      const name = (id?: string, fallback?: string) => (id ? result.name_index.stops?.[id] : undefined) || fallback || id || '';
      return {
        title: t('producer.card.STM_008.title'),
        context: tripContextText(context),
        what: t('producer.card.STM_008.what', {
          from: name(d.stop_a, d.stop_a_name), dep: d.dep ?? '—',
          to: name(d.stop_b, d.stop_b_name), arr: d.arr ?? '—',
        }),
        action: t('producer.card.STM_008.action'),
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

const AREAS: PlannerArea[] = ['tarife', 'durak', 'hat', 'takvim', 'guzergah', 'ucret', 'kurum', 'teknik'];
const CLASS_ORDER = ['SPEC', 'INTEROP', 'QUALITY', 'ANALYTICS'];
const MAP_RED = '#dc2626';
const MAP_GREY = '#64748b';
const ROWS_FIRST = 10;
const ROWS_STEP = 50;
const ROWS_CAP = 500;

function groupArea(g: RuleGroup): PlannerArea {
  return plannerArea(g.ruleId, g.notices[0]);
}

/** Konunun dokunduğu hatlar (route_id), bildirim sırasıyla. */
function routeIdsOf(g: RuleGroup, result: ValidationResult, limit = 8): string[] {
  const ni = result.name_index;
  const routes = new Set<string>();
  for (const n of g.notices) {
    const routeId = n.entity_type === 'Route'
      ? n.entity_id
      : n.entity_type === 'Trip' && n.entity_id ? ni.trip_routes?.[n.entity_id] : n.details?.route_id;
    if (routeId) routes.add(routeId);
    if (routes.size >= limit) break;
  }
  return [...routes];
}

function routeBadgeHtml(routeId: string, result: ValidationResult): string {
  const b = routeBadge(routeId, result.name_index);
  const style = b.bg ? ` style="background:${b.bg};border-color:${b.bg};color:${b.fg}"` : '';
  return `<span class="planner-route-badge"${style}>${escHtml(b.label)}</span>`;
}

function severityHtml(sev: Severity): string {
  return `<span class="planner-sev" style="--sev:${SEVERITY_COLOR[sev]}">${escHtml(severityForLocale(getLocale(), sev))}</span>`;
}

/** Çalışma günleri: calendar maskesini planlamacı diliyle söyler ("Hafta içi", "Ct, Pz"). */
function serviceText(serviceId: string | null | undefined, result: ValidationResult): string {
  if (!serviceId) return '—';
  const entry = result.name_index.service_days?.[serviceId];
  if (!entry) return serviceId;
  const [mask, , , exceptions] = entry;
  let text: string;
  if (!mask) text = t('producer.days.dates');
  else if (mask === '1111111') text = t('producer.days.everyday');
  else if (mask === '1111100') text = t('producer.days.weekdays');
  else if (mask === '0000011') text = t('producer.days.weekend');
  else if (mask === '0000000') text = t('producer.days.none');
  else {
    const fmt = new Intl.DateTimeFormat(intlLocale(), { weekday: 'short', timeZone: 'UTC' });
    // 2026-01-05 bir pazartesi; maske pzt..paz sırasında.
    text = [...mask].map((on, i) => on === '1' ? fmt.format(new Date(Date.UTC(2026, 0, 5 + i))) : '').filter(Boolean).join(', ');
  }
  return Number(exceptions) > 0 ? `${text} ${t('producer.days.exceptions', { n: exceptions })}` : text;
}

/** Saat kurallarında seçili seferin ilgili durakları sırayla, saatleriyle; hatalı satır işaretli. */
function timeStrip(n: Notice, result: ValidationResult): string {
  const d = n.details;
  if (!d) return '';
  const name = (id?: string, fallback?: string) => (id ? result.name_index.stops?.[id] : undefined) || fallback || id || '—';
  const missing = t('producer.times.missing');
  type Row = [string, string, string, boolean];
  let rows: Row[] = [];
  if ((n.rule_id === 'STM_015' || n.rule_id === 'STM_016') && d.timed_stops === '0') return '';
  if (n.rule_id === 'STM_015') {
    rows = [
      [name(d.stop_id), missing, d.departure ?? '—', true],
      ...(d.next_stop_id ? [[name(d.next_stop_id), d.next_time ?? '—', '', false] as Row] : []),
    ];
  } else if (n.rule_id === 'STM_016') {
    rows = [
      ...(d.prev_stop_id ? [[name(d.prev_stop_id), '', d.prev_time ?? '—', false] as Row] : []),
      [name(d.stop_id), missing, d.departure ?? '—', true],
    ];
  } else if (n.rule_id === 'STM_008') {
    rows = [
      ...(d.prev_stop_id ? [[name(d.prev_stop_id), '', d.prev_time ?? '—', false] as Row] : []),
      [name(d.stop_a, d.stop_a_name), '', d.dep ?? '—', false],
      [name(d.stop_b, d.stop_b_name), d.arr ?? '—', '', true],
      ...(d.next_stop_id ? [[name(d.next_stop_id), d.next_time ?? '—', '', false] as Row] : []),
    ];
  }
  if (!rows.length) return '';
  return `<table class="planner-times"><thead><tr><th>${t('producer.col.stop')}</th><th>${t('producer.times.arrival')}</th><th>${t('producer.times.departure')}</th></tr></thead><tbody>${
    rows.map(([stop, arr, dep, bad]) => `<tr${bad ? ' class="bad"' : ''}><td>${escHtml(stop)}</td><td>${escHtml(arr)}</td><td>${escHtml(dep)}</td></tr>`).join('')
  }</tbody></table>`;
}

/** Etkilenenler tablosunda saat kurallarının "hatalı yer" sütunu. */
function timeSpot(n: Notice, result: ValidationResult): string | null {
  const d = n.details;
  if (!d) return null;
  const name = (id?: string, fallback?: string) => (id ? result.name_index.stops?.[id] : undefined) || fallback || id || '—';
  if ((n.rule_id === 'STM_015' || n.rule_id === 'STM_016') && d.timed_stops === '0') {
    return t('producer.times.spot_untimed', { n: d.stop_count ?? '?' });
  }
  if (n.rule_id === 'STM_015') return t('producer.times.spot_first', { stop: name(d.stop_id), time: d.departure ?? d.next_time ?? '—' });
  if (n.rule_id === 'STM_016') return t('producer.times.spot_last', { stop: name(d.stop_id), time: d.prev_time ?? '—' });
  if (n.rule_id === 'STM_008') return `${name(d.stop_a, d.stop_a_name)} ${d.dep ?? '—'} → ${name(d.stop_b, d.stop_b_name)} ${d.arr ?? '—'}`;
  if (n.rule_id === 'STM_012' && (d.stop_a || d.stop_b)) {
    const speed = stm012Observed(n);
    return `${name(d.stop_a)} → ${name(d.stop_b)}${speed ? ` · ${speed}` : ''}`;
  }
  return null;
}

interface PlannerMap { opts: MapOptions; caption: string; }

/** Haritayı seçili kayda göre kurar: sefer → kendi güzergâhı ve durakları, işaretli durak
 *  veya bölüm kırmızı ve ona yakınlaşılmış; durak → durağın kendisi; hat → hattın çizimleri.
 *  Konuma bağlı olmayan kayıtta ağın tamamı. */
function plannerMap(n: Notice, result: ValidationResult): PlannerMap {
  const ni = result.name_index;
  const zeroStops = new Set<string>();
  const coord = (stopId?: string | null): [number, number] | null => {
    const p = stopId ? ni.stop_coords?.[stopId] : undefined;
    if (p && isNullIsland(p)) { zeroStops.add(stopId!); return null; }
    return p ? [p[0], p[1]] : null;
  };
  const routeColor = (routeId?: string) => (routeId ? routeBadge(routeId, ni).bg : null) ?? MAP_GREY;
  const lines: NonNullable<MapOptions['extraPolylines']> = [];
  const segments: NonNullable<MapOptions['extraPolylines']> = [];
  const pins: MapPin[] = [];
  const stops = new Set<string>();
  let focus: [number, number] | undefined;

  const addShape = (shapeId: string | null | undefined, color: string) => {
    const coords = shapeId ? ni.shape_coords?.[shapeId] : undefined;
    if (coords && coords.length > 1) lines.push({ coords, color, weight: 5 });
    return Boolean(coords && coords.length > 1);
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
    if (primary && !focus) focus = p;
  };

  const label = entityLabel(n, ni) || n.entity_id || '';
  const shown = label ? t('producer.map.shown', { record: label }) : '';
  const zeroCaption = () => zeroStops.size
    ? t('producer.map.null_island', { stops: [...zeroStops].slice(0, 3).map(id => ni.stops?.[id] || id).join(', ') })
    : '';
  const focusCaption = () => [zeroCaption(), shown, t('producer.map.caption_focus')].filter(Boolean).join(' · ');

  if (n.entity_type === 'Trip' && n.entity_id) {
    const tripStops = ni.trip_stops?.[n.entity_id] ?? [];
    const color = routeColor(ni.trip_routes?.[n.entity_id]);
    if (!addShape(ni.trip_shapes?.[n.entity_id], color)) {
      const coords = tripStops.map(id => coord(id)).filter((p): p is [number, number] => p !== null);
      if (coords.length > 1) lines.push({ coords, color, weight: 4 });
    }
    // Kuralın işaret ettiği durak: ilk/son durak saati eksikse o durak, aksi halde bildirimin durakları.
    if (n.rule_id === 'STM_015') addStop(tripStops[0], true);
    if (n.rule_id === 'STM_016') addStop(tripStops[tripStops.length - 1], true);
    addStop(n.details?.stop_id, true);
    const a = coord(n.details?.stop_a);
    const b = coord(n.details?.stop_b);
    if (a && b) {
      segments.push({ coords: [a, b], color: MAP_RED, weight: 7 });
      addStop(n.details?.stop_a, true);
      addStop(n.details?.stop_b, true);
    }
    for (const stopId of tripStops) addStop(stopId, false);
  } else if (n.entity_type === 'Stop' && n.entity_id) {
    addStop(n.entity_id, true);
    const here = coord(n.entity_id);
    if (here) {
      // Bağlam: durağı kullanan en fazla üç hattın çizimi ve 600 m içindeki duraklar.
      const routesHere = new Map<string, string>();
      for (const [tripId, tripStops] of Object.entries(ni.trip_stops ?? {})) {
        if (routesHere.size >= 3) break;
        const routeId = ni.trip_routes?.[tripId];
        if (!routeId || routesHere.has(routeId) || !tripStops.includes(n.entity_id)) continue;
        routesHere.set(routeId, tripId);
      }
      for (const [routeId, tripId] of routesHere) addShape(ni.trip_shapes?.[tripId], routeColor(routeId));
      const near = Object.entries(ni.stop_coords ?? {})
        .filter(([, p]) => Math.abs(p[0] - here[0]) < 0.0055 && Math.abs(p[1] - here[1]) < 0.0075)
        .slice(0, 150);
      for (const [stopId] of near) addStop(stopId, false);
    }
  } else if (n.entity_type === 'Route' && n.entity_id) {
    for (const shapeId of (ni.route_shapes?.[n.entity_id] ?? []).slice(0, 4)) addShape(shapeId, routeColor(n.entity_id));
  } else if (n.entity_type === 'Shape' && n.entity_id) {
    addShape(n.entity_id, routeColor(ni.shape_routes?.[n.entity_id]?.[0]?.[0]));
  }
  addStop(n.details?.stop_id, true);

  if (lines.length || segments.length || pins.length) {
    return { opts: { pins, extraPolylines: [...lines, ...segments], focus }, caption: focusCaption() };
  }

  // Konuma bağlı olmayan kayıt: ağın tamamı.
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

/** Etkilenenler tablosu planlamacının tanıdığı sütunlarla: sefer → hat/yön/kalkış/günler,
 *  durak → ad/kod, takvim → günler. */
function affectedColumns(g: RuleGroup, result: ValidationResult): { head: string[]; row: (n: Notice) => string[] } {
  const ni = result.name_index;
  const dash = (v?: string | null) => v || '—';
  const first = g.notices[0];
  const withDays = g.notices.some(n => n.service_id);
  const days = (cols: string[], row: (n: Notice) => string[]) => withDays
    ? { head: [...cols, t('producer.col.service')], row: (n: Notice) => [...row(n), serviceText(n.service_id, result)] }
    : { head: cols, row };
  // Kimliksiz bildirim (feed düzeyi özet) varlık sütunlarını doldurmaz; genel sütunlara düşer.
  switch (first.entity_id ? first.entity_type : 'Feed') {
    case 'Trip': {
      const withSpot = timeSpot(first, result) !== null;
      return days(
        [t('producer.col.route'), t('producer.col.direction'), t('producer.col.departure'), ...(withSpot ? [t('producer.col.spot')] : [])],
        n => {
          const routeId = n.entity_id ? ni.trip_routes?.[n.entity_id] : undefined;
          return [
            dash(routeId ? ni.routes?.[routeId] || routeId : n.entity_id),
            dash(n.entity_id ? ni.trips?.[n.entity_id] : null),
            dash(n.entity_id ? ni.trip_first_dep?.[n.entity_id] : null),
            ...(withSpot ? [dash(timeSpot(n, result))] : []),
          ];
        },
      );
    }
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
    case 'Service':
      return {
        head: [t('producer.col.service_code'), t('producer.col.service')],
        row: n => [dash(n.entity_id), serviceText(n.entity_id, result)],
      };
    default:
      return days(
        [t('producer.col.record'), t('producer.col.row')],
        n => [entityLabel(n, ni) || n.observed_value || t('producer.map.unknown_context'), n.line != null ? String(n.line) : '—'],
      );
  }
}

function affectedRows(g: RuleGroup, result: ValidationResult): string {
  const cols = affectedColumns(g, result);
  const visibleRows = g.notices.slice(0, ROWS_CAP);
  const rows = visibleRows.map((n, index) => `<tr class="planner-affected-row${index === 0 ? ' selected' : ''}" data-row="${index}" tabindex="0"${index >= ROWS_FIRST ? ' hidden' : ''}>
    ${cols.row(n).map(v => `<td>${escHtml(v)}</td>`).join('')}
  </tr>`).join('');
  const more = visibleRows.length > ROWS_FIRST
    ? `<button type="button" class="planner-more" data-more-step="${ROWS_STEP}">${t('producer.card.more', { n: Math.min(ROWS_STEP, visibleRows.length - ROWS_FIRST) })}</button>`
    : '';
  const cap = g.count > visibleRows.length
    ? `<p class="planner-capnote">${escHtml(t('producer.card.cap', { n: (g.count - visibleRows.length).toLocaleString(getLocale()) }))}</p>`
    : '';
  return `<div class="planner-affected">
    <div class="planner-subhead"><span>${t('producer.card.affected')}</span><span>${g.count.toLocaleString(getLocale())}</span><span class="planner-match-count"></span><span class="planner-subhead-hint">${t('producer.card.row_hint')}</span></div>
    <div class="planner-table-wrap"><table><thead><tr>${cols.head.map(h => `<th>${escHtml(h)}</th>`).join('')}</tr></thead><tbody>${rows || `<tr><td colspan="${cols.head.length}">${t('producer.card.no_rows')}</td></tr>`}</tbody></table></div>
    <div class="planner-affected-foot">${more}${cap}</div>
  </div>`;
}

/** Görülen değer ve kullanım satırları; seçilen kayda göre yeniden yazılır. */
function factsHtml(planner: PlannerCardCopy | null): string {
  const usage = planner?.usage
    ? `<p class="producer-card-usage"><span class="producer-card-label">${t('producer.card.usage')}:</span> ${escHtml(planner.usage)}</p>`
    : '';
  const observed = planner?.observed
    ? `<p class="producer-card-observed"><span class="producer-card-label">${t('producer.card.observed')}:</span> <code>${escHtml(planner.observed)}</code></p>`
    : '';
  return observed + usage;
}

function topicCard(g: RuleGroup, result: ValidationResult, index: number): string {
  const n = g.notices[0];
  const planner = plannerCardCopy(g, result);
  const title = planner?.title || t(`rule.${g.ruleId}`);
  const what = planner?.what || tMsg(n);
  const action = planner?.action || tRemediation(n);
  const context = planner?.context || (n.entity_type === 'Feed' ? t('producer.all_feed') : '');
  const routes = routeIdsOf(g, result);
  return `<article class="producer-card" data-rule="${escHtml(g.ruleId)}" data-area="${groupArea(g)}" data-sev="${g.severity}" data-cls="${escHtml(n.rule_class)}" data-file="${escHtml(n.file || '')}" data-card-index="${index}">
    <button type="button" class="planner-card-header" aria-expanded="false">
      <div>
        <h4 class="producer-card-title"><span class="planner-rule-id">${escHtml(g.ruleId)}</span>${escHtml(title)}</h4>
        <div class="producer-card-kicker">
          ${severityHtml(g.severity)}
          ${routes.map(routeId => routeBadgeHtml(routeId, result)).join('')}
        </div>
      </div>
      <span class="planner-card-impact"><strong>${g.count.toLocaleString(getLocale())}</strong><span>${t('producer.card.affected_short')}</span></span>
    </button>
    <div class="planner-card-collapsible" hidden>
      ${context ? `<p class="producer-card-context">${escHtml(context)}</p>` : ''}
      <div class="producer-card-main">
        <figure class="planner-map-wrap"><div class="planner-map"><span>${t('producer.map.loading')}</span></div><figcaption class="planner-map-caption"></figcaption></figure>
        <div class="planner-card-text">
          <div class="producer-card-evidence">
            <span class="producer-card-label">${t('producer.card.what')}</span>
            <p class="producer-card-message">${escHtml(what)}</p>
            <div class="planner-times-host"></div>
            <div class="producer-card-facts"${factsHtml(planner) ? '' : ' hidden'}>${factsHtml(planner)}</div>
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

function feedTitle(result: ValidationResult, agencyIds: readonly string[]): string {
  const info = result.feed_summary?.feed_info;
  // İşletmeci seçiliyse başlık o işletmecinin (işletmecilerin) adıdır.
  const picked = (result.agency_breakdown?.agencies ?? [])
    .filter(a => agencyIds.includes(a.agency_id))
    .map(a => a.agency_name || a.agency_id);
  const name = picked.length
    ? picked.join(', ')
    : info?.feed_publisher_name || result.feed_summary?.agencies?.[0]?.agency_name || t('producer.feed_fallback');
  const version = info?.feed_version ? `<span>${escHtml(t('producer.feed_version', { v: info.feed_version }))}</span>` : '';
  return `<header class="planner-top"><strong>${escHtml(name)}</strong>${version}</header>`;
}

type EntityDim = 'route' | 'stop' | 'shape' | 'service';
const ENTITY_DIMS: EntityDim[] = ['route', 'stop', 'shape', 'service'];
type NoticeKeys = Record<EntityDim, string[]>;

/** Bir kaydın dokunduğu hat, durak, güzergâh ve takvim kimlikleri (arama süzgeci için). */
function noticeKeys(n: Notice, result: ValidationResult): NoticeKeys {
  const ni = result.name_index;
  const d = n.details ?? {};
  const trip = n.entity_type === 'Trip' ? n.entity_id : null;
  const route = n.entity_type === 'Route' ? n.entity_id : trip ? ni.trip_routes?.[trip] : d.route_id;
  const shape = n.entity_type === 'Shape' ? n.entity_id : trip ? ni.trip_shapes?.[trip] : d.shape_id;
  const service = n.entity_type === 'Service' ? n.entity_id : n.service_id;
  const stops = [n.entity_type === 'Stop' ? n.entity_id : null, d.stop_id, d.stop_a, d.stop_b, d.prev_stop_id, d.next_stop_id];
  const ids = (values: Array<string | null | undefined>) => [...new Set(values.filter((v): v is string => Boolean(v)))];
  return { route: ids([route]), stop: ids(stops), shape: ids([shape]), service: ids([service]) };
}

function entityLabelOf(dim: EntityDim, id: string, result: ValidationResult): string {
  const ni = result.name_index;
  const tail = (text: string) => (text && text !== id ? `${text} (${id})` : id);
  const routeName = (routeId: string) =>
    [ni.routes?.[routeId], ni.route_long_names?.[routeId]].filter(Boolean).join(' · ');
  if (dim === 'route') return tail(routeName(id));
  if (dim === 'stop') {
    const code = ni.stop_codes?.[id];
    return tail([ni.stops?.[id], code ? t('producer.filter.stop_code', { code }) : ''].filter(Boolean).join(' · '));
  }
  if (dim === 'shape') {
    // GTFS'te güzergâhın adı yok: onu kullanan seferin hattı ve yönü ad yerine geçer.
    const routeId = ni.shape_routes?.[id]?.[0]?.[0];
    const tripId = ni.shape_trips?.[id];
    const headsign = tripId ? ni.trips?.[tripId] : undefined;
    return tail([routeId ? routeName(routeId) || routeId : '', headsign].filter(Boolean).join(' · '));
  }
  const days = serviceText(id, result);
  return days === id ? id : `${days} (${id})`;
}

/** Hat, durak, güzergâh ve takvim için seçmeli, aranabilir açılır listeler. */
function entityFilterBar(labels: Record<EntityDim, Map<string, string>>): string {
  const fields = ENTITY_DIMS.filter(dim => labels[dim].size > 0).map(dim => `
    <div class="planner-pick" data-dim="${dim}">
      <button type="button" class="planner-pick-btn" id="planner-pick-${dim}" aria-expanded="false">
        <span class="planner-pick-cap">${t(`producer.filter.${dim}`)}</span>
        <span class="planner-pick-value">${t('fix.filter.all')}</span>
      </button>
      <div class="planner-pick-panel" hidden>
        <div class="planner-pick-head">
          <input type="search" class="planner-pick-search" id="planner-pick-search-${dim}" placeholder="${escHtml(t('producer.filter.search'))}" autocomplete="off">
          <button type="button" class="planner-pick-clear">${t('producer.filter.clear')}</button>
        </div>
        <div class="planner-pick-list" role="listbox" aria-multiselectable="true"></div>
        <p class="planner-pick-more"></p>
      </div>
    </div>`).join('');
  return fields ? `<div class="filter-bar planner-filter-bar planner-entity-filters">${fields}</div>` : '';
}

/** Rapor sekmesindeki R2 süzgeçleriyle aynı: önem ve sınıf çoklu seçim, kural ve dosya listesi. */
function filterBar(groups: RuleGroup[], result: ValidationResult): string {
  const sevPresent = new Set(groups.map(g => g.severity));
  const clsPresent = new Set(groups.map(g => g.notices[0].rule_class));
  const files = [...new Set(groups.map(g => g.notices[0].file).filter((f): f is string => Boolean(f)))].sort();
  const sevChips = SEVERITY_ORDER.filter(s => sevPresent.has(s)).map(s =>
    `<button type="button" class="sev-chip" data-sev="${s}" style="--chip-color:${SEVERITY_COLOR[s]}">${escHtml(severityForLocale(getLocale(), s))}</button>`).join('');
  const clsChips = CLASS_ORDER.filter(c => clsPresent.has(c as Notice['rule_class'])).map(c =>
    `<button type="button" class="cls-chip" data-cls="${c}">${escHtml(RULE_CLASS_TR[c as keyof typeof RULE_CLASS_TR])}</button>`).join('');
  const ruleOptions = [...groups].sort((a, b) => a.ruleId.localeCompare(b.ruleId))
    .map(g => `<option value="${escHtml(g.ruleId)}">${escHtml(g.ruleId)} — ${escHtml(plannerCardCopy(g, result)?.title || t(`rule.${g.ruleId}`))}</option>`).join('');
  const all = t('fix.filter.all');
  return `<div class="filter-bar planner-filter-bar">
    ${sevPresent.size > 1 ? `<div class="filter-chip-group" id="planner-sev-filter"><span class="filter-cap">${t('fix.filter.severity')}</span>${sevChips}</div>` : ''}
    ${clsPresent.size > 1 ? `<div class="filter-chip-group" id="planner-cls-filter"><span class="filter-cap">${t('fix.filter.class')}</span>${clsChips}</div>` : ''}
    <label>${t('fix.filter.rule')} <select id="planner-rule-filter"><option value="">${all}</option>${ruleOptions}</select></label>
    ${files.length ? `<label>${t('fix.filter.file')} <select id="planner-file-filter"><option value="">${all}</option>${files.map(f => `<option value="${escHtml(f)}">${escHtml(f)}</option>`).join('')}</select></label>` : ''}
    <span class="filter-count" id="planner-filter-count"></span>
  </div>`;
}

export function renderProducer(root: HTMLElement, fullResult: ValidationResult): void {
  // İşletmeci süzgeci "Ayrıntı ve Düzeltme" ile ortaktır: kayıtlar seçili işletmecilerinkine
  // daralır; yayın kararı feed genelidir (motor onu işletmeci başına vermez).
  const agencyFilter = getState()?.agencyFilter ?? [];
  const filtered = filterNoticesByAgency(fullResult, agencyFilter);
  const result: ValidationResult = filtered ? { ...fullResult, notices: filtered } : fullResult;
  const m = buildProducerModel(result);
  if (filtered) {
    for (const g of [...m.blockers, ...m.improvements, ...m.infos]) g.count = g.notices.length;
  }
  const ordered = [...m.blockers, ...m.improvements, ...m.infos];
  const blockerRules = new Set(m.blockers.map(g => g.ruleId));
  // Planlamacı metni yazılmış konular ve yayını engelleyenler üstte; kalanlar motorun teknik
  // metniyle ayrı bölümde, iki dil aynı listede karışmasın diye.
  const primary = ordered.filter(g => blockerRules.has(g.ruleId) || plannerCardCopy(g, result) !== null);
  const technical = ordered.filter(g => !primary.includes(g));
  const groups = [...primary, ...technical];

  const keys = groups.map(g => g.notices.slice(0, ROWS_CAP).map(n => noticeKeys(n, result)));
  const labels = Object.fromEntries(ENTITY_DIMS.map(dim => [dim, new Map<string, string>()])) as Record<EntityDim, Map<string, string>>;
  // Seçenekler yalnız veri setinde tanımlı kayıtlardır; bazı kuralların route_id gibi
  // ayrıntıları hat değil desen kimliği taşır ("00020:0:11223→11224").
  const ni = result.name_index;
  const known: Record<EntityDim, (id: string) => boolean> = {
    route: id => id in (ni.routes ?? {}) || id in (ni.route_shapes ?? {}),
    stop: id => id in (ni.stops ?? {}) || id in (ni.stop_coords ?? {}),
    shape: id => id in (ni.shape_routes ?? {}) || id in (ni.shape_coords ?? {}),
    service: id => id in (ni.service_days ?? {}),
  };
  for (const rows of keys) for (const k of rows) for (const dim of ENTITY_DIMS) {
    for (const id of k[dim]) if (!labels[dim].has(id) && known[dim](id)) labels[dim].set(id, entityLabelOf(dim, id, result));
  }

  const verdict = m.publishable
    ? `<div class="planner-verdict ok"><h2>${t('domain.r1_ok_title')}</h2><p>${t('producer.planner_verdict', { n: groups.length })}</p></div>`
    : `<div class="planner-verdict bad"><h2>${t('domain.r1_blocked_title')}</h2><p>${t('producer.not_publishable', { n: m.blockers.length })} ${t('producer.not_publishable_desc')}</p></div>`;
  const sevCounts = SEVERITY_ORDER
    .map(s => [s, groups.filter(g => g.severity === s).length] as [Severity, number])
    .filter(([, count]) => count > 0)
    .map(([s, count]) => `<span class="planner-count">${severityHtml(s)}<strong>${count}</strong></span>`).join('');
  const areaNav = (['all', ...AREAS] as AreaFilter[]).map(id => {
    // Kayıt sayısı: kartlardaki "kayıt" ile aynı birim.
    const count = groups.filter(g => id === 'all' || groupArea(g) === id).reduce((sum, g) => sum + g.count, 0);
    return `<button type="button" class="planner-area" data-area-filter="${id}" aria-pressed="${id === 'all'}"><span>${t(`producer.area.${id}`)}</span><span class="planner-area-count${count === 0 ? ' zero' : ''}">${count ? count.toLocaleString(getLocale()) : t('producer.area.none')}</span></button>`;
  }).join('');
  const primaryCards = primary.map((g, i) => topicCard(g, result, i)).join('');
  const technicalCards = technical.map((g, i) => topicCard(g, result, primary.length + i)).join('');
  const technicalSection = technical.length
    ? `<details class="planner-technical"${primary.length ? '' : ' open'}><summary>${t('producer.technical.title', { n: technical.length })}</summary><p class="planner-technical-hint">${t('producer.technical.hint')}</p>${technicalCards}</details>`
    : '';

  root.innerHTML = `<section class="producer-page">
    ${renderAgencyFilterBar(fullResult, agencyFilter)}
    ${feedTitle(fullResult, agencyFilter)}
    ${verdict}<div class="planner-counts">${sevCounts}</div>
    ${coverageNotice(m.coverageComplete)}
    ${groups.length ? filterBar(groups, result) : ''}
    ${groups.length ? entityFilterBar(labels) : ''}
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
  attachAgencyFilterListeners(root, ids => {
    setAgencyFilter(ids);
    window.dispatchEvent(new CustomEvent('gtfs-navigate'));
  });
  let openIndex: number | null = null;
  const cleanups = new Map<number, () => void>();
  const cardAt = (index: number) => root.querySelector<HTMLElement>(`[data-card-index="${index}"]`);
  const unmount = (index: number) => { cleanups.get(index)?.(); cleanups.delete(index); };

  const showMap = (card: HTMLElement, index: number, row: number) => {
    const mapHost = card.querySelector<HTMLElement>('.planner-map');
    const caption = card.querySelector<HTMLElement>('.planner-map-caption');
    const g = groups[index];
    const notice = g.notices[row] ?? g.notices[0];
    if (!mapHost) return;
    unmount(index);
    card.querySelectorAll('.planner-affected-row.selected').forEach(el => el.classList.remove('selected'));
    card.querySelector(`.planner-affected-row[data-row="${row}"]`)?.classList.add('selected');
    // Kartın bağlam satırı (hat · yön · kalkış) seçilen kayda göre yazılır; harita ile çelişmesin.
    const rowCopy = plannerCardCopy({ ...g, notices: [notice] }, result);
    const contextEl = card.querySelector<HTMLElement>('.producer-card-context');
    if (contextEl && rowCopy?.context) contextEl.textContent = rowCopy.context;
    // "Ne olmuş / Ne yapmalı" seçilen kaydın kendi metnidir; planlamacı metni yoksa motorun metni.
    const messageEl = card.querySelector<HTMLElement>('.producer-card-message');
    if (messageEl) messageEl.textContent = rowCopy?.what || tMsg(notice);
    const actionEl = card.querySelector<HTMLElement>('.producer-card-remediation');
    if (actionEl) actionEl.textContent = rowCopy?.action || tRemediation(notice);
    const factsEl = card.querySelector<HTMLElement>('.producer-card-facts');
    if (factsEl) { const facts = factsHtml(rowCopy); factsEl.innerHTML = facts; factsEl.hidden = !facts; }
    const strip = card.querySelector<HTMLElement>('.planner-times-host');
    if (strip) strip.innerHTML = timeStrip(notice, result);
    const map = plannerMap(notice, result);
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
  const closeCard = (card: HTMLElement, index: number) => {
    card.querySelector<HTMLElement>('.planner-card-collapsible')!.hidden = true;
    card.querySelector<HTMLButtonElement>('.planner-card-header')!.setAttribute('aria-expanded', 'false');
    unmount(index);
  };
  const openCard = (card: HTMLElement, index: number) => {
    if (openIndex !== null && openIndex !== index) {
      const previous = cardAt(openIndex);
      if (previous) closeCard(previous, openIndex);
    }
    card.querySelector<HTMLElement>('.planner-card-collapsible')!.hidden = false;
    card.querySelector<HTMLButtonElement>('.planner-card-header')!.setAttribute('aria-expanded', 'true');
    openIndex = index;
    showMap(card, index, 0);
  };
  root.querySelectorAll<HTMLButtonElement>('.planner-card-header').forEach(header => {
    header.addEventListener('click', () => {
      const card = header.closest<HTMLElement>('.producer-card')!;
      const index = Number(card.dataset['cardIndex']);
      if (openIndex === index) { closeCard(card, index); openIndex = null; } else openCard(card, index);
    });
  });
  root.querySelectorAll<HTMLTableRowElement>('.planner-affected-row').forEach(rowEl => {
    const select = () => {
      const card = rowEl.closest<HTMLElement>('.producer-card')!;
      showMap(card, Number(card.dataset['cardIndex']), Number(rowEl.dataset['row']));
    };
    rowEl.addEventListener('click', select);
    rowEl.addEventListener('keydown', e => { if (e.key === 'Enter' || e.key === ' ') { e.preventDefault(); select(); } });
  });

  // Süzgeçler: iş alanı + önem + sınıf + kural + dosya birlikte uygulanır.
  let area: AreaFilter = 'all';
  const sevActive = new Set<string>();
  const clsActive = new Set<string>();
  const ruleSelect = root.querySelector<HTMLSelectElement>('#planner-rule-filter');
  const fileSelect = root.querySelector<HTMLSelectElement>('#planner-file-filter');
  const limits = new Map<number, number>();
  const matchesByCard = new Map<number, Set<number> | null>();
  // Etkilenenler tablosu: süzgeç varken eşleşen tüm satırlar, yokken sayfalı (10, +50).
  const refreshRows = (card: HTMLElement, index: number) => {
    const matches = matchesByCard.get(index) ?? null;
    const limit = limits.get(index) ?? ROWS_FIRST;
    const rows = card.querySelectorAll<HTMLTableRowElement>('.planner-affected-row');
    rows.forEach(row => {
      const i = Number(row.dataset['row']);
      row.hidden = matches ? !matches.has(i) : i >= limit;
    });
    const more = card.querySelector<HTMLButtonElement>('.planner-more');
    const remaining = rows.length - limit;
    if (more) {
      more.hidden = Boolean(matches) || remaining <= 0;
      more.textContent = t('producer.card.more', { n: Math.min(ROWS_STEP, Math.max(remaining, 0)) });
    }
    const counter = card.querySelector<HTMLElement>('.planner-match-count');
    if (counter) counter.textContent = matches ? t('producer.card.matching', { n: matches.size }) : '';
  };
  // Seçmeli listeler: her boyutta seçilen kimlikler; boyut içinde VEYA, boyutlar arası VE.
  const picked = new Map<EntityDim, Set<string>>(ENTITY_DIMS.map(dim => [dim, new Set<string>()]));
  const entityQuery = (): Map<EntityDim, Set<string>> =>
    new Map([...picked].filter(([, ids]) => ids.size > 0));
  const PICK_LIMIT = 300;
  const sortedOptions = new Map(ENTITY_DIMS.map(dim => [dim,
    [...labels[dim]].sort((a, b) => a[1].localeCompare(b[1], getLocale()))]));
  const renderPick = (box: HTMLElement) => {
    const dim = box.dataset['dim'] as EntityDim;
    const ids = picked.get(dim)!;
    const search = box.querySelector<HTMLInputElement>('.planner-pick-search')?.value.trim().toLocaleLowerCase(getLocale()) ?? '';
    const options = sortedOptions.get(dim)!;
    const hits = options.filter(([id, label]) => ids.has(id) || !search || label.toLocaleLowerCase(getLocale()).includes(search));
    hits.sort((a, b) => Number(ids.has(b[0])) - Number(ids.has(a[0])));
    const list = box.querySelector<HTMLElement>('.planner-pick-list');
    if (list) {
      list.innerHTML = hits.slice(0, PICK_LIMIT).map(([id, label]) =>
        `<label class="planner-pick-option"><input type="checkbox" value="${escHtml(id)}"${ids.has(id) ? ' checked' : ''}><span>${escHtml(label)}</span></label>`).join('')
        || `<p class="planner-pick-empty">${t('producer.filter.no_match')}</p>`;
    }
    const more = box.querySelector<HTMLElement>('.planner-pick-more');
    if (more) more.textContent = hits.length > PICK_LIMIT ? t('producer.filter.refine', { n: hits.length - PICK_LIMIT }) : '';
    const value = box.querySelector<HTMLElement>('.planner-pick-value');
    if (value) {
      value.textContent = ids.size === 0 ? t('fix.filter.all')
        : ids.size === 1 ? labels[dim].get([...ids][0]) ?? '' : t('producer.filter.selected', { n: ids.size });
    }
    box.classList.toggle('active', ids.size > 0);
  };
  const pickBoxes = Array.from(root.querySelectorAll<HTMLElement>('.planner-pick'));
  const closePicks = (except?: HTMLElement) => pickBoxes.forEach(box => {
    if (box === except) return;
    box.querySelector<HTMLElement>('.planner-pick-panel')!.hidden = true;
    box.querySelector<HTMLButtonElement>('.planner-pick-btn')!.setAttribute('aria-expanded', 'false');
  });
  const applyFilters = () => {
    const rule = ruleSelect?.value ?? '';
    const file = fileSelect?.value ?? '';
    const entity = entityQuery();
    let visible = 0;
    root.querySelectorAll<HTMLElement>('.producer-card').forEach(card => {
      const index = Number(card.dataset['cardIndex']);
      let matches: Set<number> | null = null;
      if (entity.size) {
        matches = new Set(keys[index].flatMap((k, row) =>
          [...entity].every(([dim, ids]) => k[dim].some(id => ids.has(id))) ? [row] : []));
      }
      matchesByCard.set(index, matches);
      const show = (area === 'all' || card.dataset['area'] === area)
        && (sevActive.size === 0 || sevActive.has(card.dataset['sev'] ?? ''))
        && (clsActive.size === 0 || clsActive.has(card.dataset['cls'] ?? ''))
        && (!rule || card.dataset['rule'] === rule)
        && (!file || card.dataset['file'] === file)
        && (!matches || matches.size > 0);
      card.hidden = !show;
      if (show) visible++;
      refreshRows(card, index);
      // Açık kartta seçili satır süzgeç dışında kaldıysa ilk eşleşeni göster.
      if (show && openIndex === index && matches) {
        const selected = Number(card.querySelector<HTMLElement>('.planner-affected-row.selected')?.dataset['row'] ?? -1);
        if (!matches.has(selected)) showMap(card, index, Math.min(...matches));
      }
    });
    const technicalBox = root.querySelector<HTMLDetailsElement>('.planner-technical');
    if (technicalBox) {
      technicalBox.hidden = !technicalBox.querySelector('.producer-card:not([hidden])');
      if (rule || file || entity.size) technicalBox.open = true;
    }
    const counter = root.querySelector<HTMLElement>('#planner-filter-count');
    const anyFilter = area !== 'all' || sevActive.size > 0 || clsActive.size > 0 || rule || file || entity.size > 0;
    if (counter) counter.textContent = anyFilter ? t('producer.filter.count', { visible, total: groups.length }) : '';
  };
  root.querySelectorAll<HTMLButtonElement>('.planner-area').forEach(btn => {
    btn.addEventListener('click', () => {
      area = btn.dataset['areaFilter'] as AreaFilter;
      root.querySelectorAll<HTMLButtonElement>('.planner-area').forEach(b => b.setAttribute('aria-pressed', String(b === btn)));
      applyFilters();
    });
  });
  const toggleChip = (chip: HTMLButtonElement, set: Set<string>, value: string) => {
    if (set.has(value)) { set.delete(value); chip.classList.remove('active'); } else { set.add(value); chip.classList.add('active'); }
    applyFilters();
  };
  root.querySelectorAll<HTMLButtonElement>('#planner-sev-filter .sev-chip').forEach(chip =>
    chip.addEventListener('click', () => toggleChip(chip, sevActive, chip.dataset['sev'] ?? '')));
  root.querySelectorAll<HTMLButtonElement>('#planner-cls-filter .cls-chip').forEach(chip =>
    chip.addEventListener('click', () => toggleChip(chip, clsActive, chip.dataset['cls'] ?? '')));
  ruleSelect?.addEventListener('change', applyFilters);
  fileSelect?.addEventListener('change', applyFilters);
  let typing: ReturnType<typeof setTimeout> | undefined;
  pickBoxes.forEach(box => {
    const dim = box.dataset['dim'] as EntityDim;
    const panel = box.querySelector<HTMLElement>('.planner-pick-panel')!;
    const btn = box.querySelector<HTMLButtonElement>('.planner-pick-btn')!;
    renderPick(box);
    btn.addEventListener('click', () => {
      const opening = panel.hidden;
      closePicks(box);
      panel.hidden = !opening;
      btn.setAttribute('aria-expanded', String(opening));
      if (opening) box.querySelector<HTMLInputElement>('.planner-pick-search')?.focus();
    });
    box.querySelector<HTMLInputElement>('.planner-pick-search')?.addEventListener('input', () => {
      clearTimeout(typing);
      typing = setTimeout(() => renderPick(box), 120);
    });
    box.querySelector<HTMLElement>('.planner-pick-list')?.addEventListener('change', e => {
      const input = e.target as HTMLInputElement;
      if (input.checked) picked.get(dim)!.add(input.value); else picked.get(dim)!.delete(input.value);
      renderPick(box);
      applyFilters();
    });
    box.querySelector<HTMLButtonElement>('.planner-pick-clear')?.addEventListener('click', () => {
      picked.get(dim)!.clear();
      const search = box.querySelector<HTMLInputElement>('.planner-pick-search');
      if (search) search.value = '';
      renderPick(box);
      applyFilters();
    });
  });
  if (pickBoxes.length && typeof document !== 'undefined') {
    document.addEventListener('click', e => {
      if (!root.isConnected) return;
      if (!pickBoxes.some(box => box.contains(e.target as Node))) closePicks();
    });
    document.addEventListener('keydown', e => { if (e.key === 'Escape') closePicks(); });
  }

  root.querySelectorAll<HTMLButtonElement>('.planner-more').forEach(btn => {
    btn.addEventListener('click', () => {
      const card = btn.closest<HTMLElement>('.producer-card');
      if (!card) return;
      const index = Number(card.dataset['cardIndex']);
      limits.set(index, Math.min((limits.get(index) ?? ROWS_FIRST) + ROWS_STEP, ROWS_CAP));
      refreshRows(card, index);
    });
  });

  const first = groups.length ? cardAt(0) : null;
  if (first) openCard(first, 0);
}
