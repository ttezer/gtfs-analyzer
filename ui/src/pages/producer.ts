// Producer summary: collects the engine's existing result (R1 decision, R9 priority, NameIndex
// names) on one screen. It computes no new findings, scores, or root causes; it only groups and sorts.
import type { ValidationResult, Notice, Severity } from '../types';
import { t, tMsg, tRemediation, getLocale, intlLocale, severityForLocale, SEVERITY_COLOR, RULE_CLASS_TR } from '../i18n';
import { SEVERITY_ORDER } from '../severity-order';
import { escHtml } from '../escape';
import type { MapOptions, MapPin } from '../map-modal';
import {
  buildProducerModel, entityLabel, plannerArea, routeBadge,
  routeIdOfNotice,
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

/** STM_051/052/054/055: the boarding setting written on the Flex row and what it means; an
 *  empty cell the spec counts as 0 is shown as such. */
function flexSetting(n: Notice): { value: string; meaning: string } {
  const raw = n.observed_value ?? '';
  if (raw.startsWith('value=0; source=implicit_default')) {
    return { value: t('producer.card.flex_value_empty'), meaning: t('producer.card.flex_meaning.implicit') };
  }
  const kind = n.rule_id === 'STM_054' || n.rule_id === 'STM_055' ? 'continuous' : 'type';
  return { value: raw || '—', meaning: t(`producer.card.flex_meaning.${kind}_${raw}`) };
}

/** The Flex row's place: its zone, or its stop by name. */
function flexPlace(d: Record<string, string>, result: ValidationResult): string {
  const zone = d.location_id || d.location_group_id;
  if (zone) return t('producer.card.flex_place_zone', { zone });
  return d.stop_id ? t('producer.card.flex_place_stop', { stop: result.name_index.stops?.[d.stop_id] || d.stop_id }) : '';
}

/** STM_053: the example trip's rows (one before the run, the run, one after) in stop order. */
function stm053Rows(d: Record<string, string>): Array<{ stop: string; sequence: string; arrival: string; departure: string; run: boolean }> {
  const rows = [];
  for (let i = 0; d[`example_${i}_stop_id`]; i++) {
    const at = (key: string) => d[`example_${i}_${key}`] ?? '';
    rows.push({ stop: at('stop_id'), sequence: at('stop_sequence'), arrival: at('arrival'), departure: at('departure'), run: at('role') === 'run' });
  }
  return rows;
}

interface PlannerCardCopy {
  title: string;
  context?: string;
  what: string;
  action: string;
  observed?: string;
  usage?: string;
  /** Metin dil dosyasından geldi; kanıt (görülen değer) genel kalıpla gösterilir. */
  generic?: boolean;
}

function agencyLabel(n: Notice, result: ValidationResult): string {
  const agency = result.feed_summary?.agencies?.find(a => a.agency_id === n.entity_id);
  // A single agency often has no code; the engine then names it in details.agency_name.
  return agency?.agency_name || n.details?.agency_name || n.entity_id || t('producer.card.unnamed_agency');
}

/** Agency cards: the agency by name, then the value written in the field the rule checks. */
const AGN_AGENCY_RULES = new Set(['AGN_002', 'AGN_004', 'AGN_006', 'AGN_007', 'AGN_008', 'AGN_009', 'AGN_010', 'AGN_012', 'AGN_013', 'AGN_014', 'AGN_015', 'AGN_016', 'AGN_018']);
function agnText(n: Notice, result: ValidationResult): string {
  const value = visibleValue(n.observed_value ?? null);
  const parts = [t(`producer.card.${n.rule_id}.what`), t('producer.card.AGN.agency', { agency: agencyLabel(n, result) })];
  if (value) parts.push(t('producer.card.AGN.value', { value }));
  // AGN_015: the same address over https, which the planner can try first.
  if (n.rule_id === 'AGN_015' && /^http:\/\//i.test(value)) parts.push(t('producer.card.AGN_015.https', { url: value.replace(/^http:/i, 'https:') }));
  return parts.join(' ');
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

function plannerTime(value?: string | null): string | null {
  return value ? (value.endsWith(':00') ? value.slice(0, -3) : value) : null;
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

/** Great-circle distance in metres between two [lat, lon] points. */
function metersBetween(a: [number, number], b: [number, number]): number {
  const rad = Math.PI / 180;
  const dLat = (b[0] - a[0]) * rad;
  const dLon = (b[1] - a[1]) * rad;
  const h = Math.sin(dLat / 2) ** 2 + Math.cos(a[0] * rad) * Math.cos(b[0] * rad) * Math.sin(dLon / 2) ** 2;
  return 2 * 6_371_000 * Math.asin(Math.min(1, Math.sqrt(h)));
}

/** A coordinate as the feed would write it: dot decimal, at most six decimals. */
function coordText(value: number | undefined): string {
  return value === undefined || !Number.isFinite(value) ? '—' : String(Number(value.toFixed(6)));
}

/** Shapes the engine found drawn against the direction of travel (SHP_016), per result. */
const reversedShapeCache = new WeakMap<ValidationResult, Set<string>>();
function reversedShapes(result: ValidationResult): Set<string> {
  let shapes = reversedShapeCache.get(result);
  if (!shapes) {
    shapes = new Set(result.notices.filter(n => n.rule_id === 'SHP_016' && n.entity_id).map(n => n.entity_id!));
    reversedShapeCache.set(result, shapes);
  }
  return shapes;
}

/** Seconds as "26 sa 26 dk" / "45 dk" / "30 sn" in the page language. */
function durationText(seconds: number): string {
  if (!Number.isFinite(seconds)) return '—';
  if (seconds < 60) return t('producer.times.seconds', { n: Math.round(seconds) });
  const h = Math.floor(seconds / 3600);
  const m = Math.round((seconds % 3600) / 60);
  if (h > 0) return m === 0 ? t('producer.times.duration_h', { h }) : t('producer.times.duration_hm', { h, m });
  return t('producer.times.duration_m', { m });
}

/** "26:26:00" → seconds; NaN when the text is not h:mm:ss. */
function hmsSeconds(value?: string | null): number {
  const m = /^(\d+):(\d{2}):(\d{2})$/.exec(value ?? '');
  return m ? Number(m[1]) * 3600 + Number(m[2]) * 60 + Number(m[3]) : NaN;
}

/** A straight-line distance in metres as "350 m" or "8,4 km". */
/** A GTFS date (YYYYMMDD) in the reader's format; anything else as written. */
function ymdText(value?: string): string {
  if (!value) return '—';
  return /^\d{8}$/.test(value)
    ? new Intl.DateTimeFormat(intlLocale(), { day: '2-digit', month: '2-digit', year: 'numeric', timeZone: 'UTC' })
      .format(new Date(Date.UTC(Number(value.slice(0, 4)), Number(value.slice(4, 6)) - 1, Number(value.slice(6, 8)))))
    : value;
}

function metersText(value?: string): string {
  const v = Number(value);
  if (!Number.isFinite(v)) return '—';
  return v >= 1000 ? `${localizedNumber(String(v / 1000))} km` : `${localizedNumber(String(Math.round(v)))} m`;
}

/** STM_024: the distance unit a cumulative value matches when compared with the drawn shape
 *  length in metres. Only a value within ×1.25 of one unit counts (stops may end a little before
 *  the shape does); km and miles are 1.6× apart, so the bands never overlap. */
const DISTANCE_UNITS: ReadonlyArray<[string, number]> = [['m', 1], ['km', 0.001], ['ft', 3.28084], ['mi', 0.000621371]];
function distanceUnit(value: string | undefined, lengthMeters: string | undefined): string | null {
  const v = Number(value);
  const length = Number(lengthMeters);
  if (!(v > 0) || !(length > 0)) return null;
  const ratio = v / length;
  let best: [string, number] | null = null;
  for (const [unit, factor] of DISTANCE_UNITS) {
    const gap = Math.abs(Math.log(ratio / factor));
    if (!best || gap < best[1]) best = [unit, gap];
  }
  return best && best[1] <= Math.log(1.25) ? best[0] : null;
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

  // The engine's observed/expected values are not translated. The planner shows only
  // language-independent values (numbers, times, dates, units, coordinates) and data from the
  // feed's own text columns; engine additions such as "7 rows", "full", or "segment 8→9"
  // stay hidden so languages are not mixed.
const NEUTRAL_UNITS = ['km/h', 'm/s', 'min', 'km', 'm', 's', 'h', '%'] as const;
const NEUTRAL_PUNCTUATION = new Set(['<', '>', '≤', '≥', '=', '→', '/', '×', '(', ')', '[', ']', ',', ';', ':', '+', '-', '–', '.', '%', '·']);

/**
 * Nötr değerleri regex yerine soldan sağa tarar. Önceki regex, özellikle sayı ve
 * noktalama işaretlerini bir arada taşıyan motor özetlerinde üstel geri izleme
 * yaparak tek bir değeri dakikalarca işleyebiliyordu.
 */
function isNeutralValue(value: string): boolean {
  let i = 0;
  while (i < value.length) {
    const char = value[i];
    if (/\s/u.test(char) || NEUTRAL_PUNCTUATION.has(char)) {
      i += 1;
      continue;
    }

    if (char >= '0' && char <= '9') {
      i += 1;
      while (i < value.length && value[i] >= '0' && value[i] <= '9') i += 1;
      if ((value[i] === '.' || value[i] === ',') && value[i + 1] >= '0' && value[i + 1] <= '9') {
        i += 1;
        while (i < value.length && value[i] >= '0' && value[i] <= '9') i += 1;
      }
      const unit = NEUTRAL_UNITS.find(candidate => value.startsWith(candidate, i));
      if (unit) i += unit.length;
      continue;
    }

    const unit = NEUTRAL_UNITS.find(candidate => value.startsWith(candidate, i));
    if (unit) {
      i += unit.length;
      continue;
    }
    return false;
  }
  return value.length > 0;
}
const FEED_TEXT_FIELDS = new Set([
  'agency_id', 'agency_name', 'agency_url', 'agency_phone', 'agency_email', 'agency_fare_url',
  'stop_id', 'stop_code', 'stop_name', 'tts_stop_name', 'stop_desc', 'platform_code', 'zone_id',
  'from_stop_id', 'to_stop_id', 'parent_station', 'level_id', 'level_name',
  'route_id', 'route_short_name', 'route_long_name', 'route_desc', 'route_url',
  'trip_id', 'trip_headsign', 'trip_short_name', 'block_id', 'stop_headsign',
  'service_id', 'shape_id', 'fare_id', 'feed_publisher_name', 'feed_publisher_url',
]);

function plainValue(value: string | null | undefined, field: string | null | undefined): string | null {
  const v = value?.trim();
  if (!v || v === 'None') return null;
  if (isNeutralValue(v)) return v;
  const fromFeedText = (field ?? '').split('|').some(f => FEED_TEXT_FIELDS.has(f));
  // Engine summaries such as "7 rows", "1469min (hour-based …)", or "a=b; c" are not feed data.
  return fromFeedText && !/[=;]/.test(v) && !/^\d+(?:[.,]\d+)?\s*\p{L}{2,}(?![\p{L}\d])/u.test(v) && !/\(\s*\p{L}/u.test(v) ? v : null;
}

/** Bildirimin görülen değeri; beklenen değer yalnız dilden bağımsızsa eklenir. */
function observedValue(n: Notice): { value: string; expected: string | null } | null {
  const value = plainValue(n.observed_value, n.field);
  if (!value) return null;
  const expected = n.expected_value?.trim();
  return { value, expected: expected && expected !== 'None' && isNeutralValue(expected) ? expected : null };
}

function observedLine(n: Notice): string | undefined {
  const o = observedValue(n);
  if (!o) return undefined;
  return o.expected ? `${o.value} · ${t('producer.card.expected_inline', { value: o.expected })}` : o.value;
}

/** Kural kendi kanıtını yazmıyorsa (genel kalıp ya da metinsiz kural) seçilen kaydın görülen değeri. */
function evidenceFacts(copy: PlannerCardCopy | null, n: Notice): Pick<PlannerCardCopy, 'observed' | 'usage'> | null {
  if (copy && !copy.generic) return copy;
  const observed = observedLine(n);
  return observed ? { observed } : null;
}

/** DQ_021: what kind of record repeats in each file it checks, so the card speaks of stops,
 *  routes, calendar dates or fare rules rather than file and column names. */
const DQ021_KIND: Record<string, string> = {
  'stops.txt': 'stop', 'routes.txt': 'route', 'trips.txt': 'trip', 'calendar_dates.txt': 'dates',
  'attributions.txt': 'attribution', 'location_groups.txt': 'location_group', 'route_networks.txt': 'route_network',
  'location_group_stops.txt': 'group_member', 'stop_areas.txt': 'stop_area', 'fare_rules.txt': 'fare',
  'fare_leg_rules.txt': 'fare', 'fare_leg_join_rules.txt': 'fare', 'fare_transfer_rules.txt': 'fare',
};
/** Planner names for GTFS files ("Stop list") and columns ("Stop name"), so cards need not
 *  show file and column names. A column without a name keeps its own (the planner still has to
 *  find it); a file without one leaves the card to its general text. */
const PLANNER_LISTS = new Set(['agency', 'stops', 'routes', 'trips', 'stop_times', 'calendar', 'calendar_dates', 'shapes', 'frequencies', 'transfers', 'feed_info', 'fare_attributes', 'fare_rules', 'translations', 'attributions', 'pathways', 'levels', 'agency_jp', 'office_jp', 'routes_jp', 'pattern_jp']);
const PLANNER_FIELDS = new Set(['safe_duration_factor', 'safe_duration_offset', 'monday', 'tuesday', 'wednesday', 'thursday', 'friday', 'saturday', 'sunday', 'start_date', 'end_date', 'stop_id', 'stop_code', 'stop_name', 'stop_desc', 'stop_lat', 'stop_lon', 'zone_id', 'stop_url', 'parent_station', 'platform_code', 'tts_stop_name', 'route_id', 'route_short_name', 'route_long_name', 'route_desc', 'route_url', 'route_color', 'route_text_color', 'agency_id', 'agency_name', 'agency_url', 'agency_timezone', 'agency_phone', 'agency_lang', 'trip_id', 'service_id', 'trip_headsign', 'trip_short_name', 'block_id', 'shape_id', 'stop_headsign', 'arrival_time', 'departure_time', 'date', 'feed_publisher_name', 'feed_publisher_url', 'feed_lang', 'fare_id', 'location_type', 'feed_start_date', 'feed_end_date', 'feed_version', 'currency_type', 'route_type', 'agency_email', 'agency_fare_url', 'agency_cemv_support']);
function plannerListName(file?: string | null): string | null {
  const stem = file?.replace(/\.txt$/, '');
  return stem && PLANNER_LISTS.has(stem) ? t(`producer.list.${stem}`) : null;
}
function plannerFieldName(field: string): string {
  return PLANNER_FIELDS.has(field) ? t(`producer.field.${field}`) : field;
}

/** GTFS-JP kana rules (JPN_001/008/009/010): the Japanese name that lacks a reading, read from the
 *  name index (the engine puts it only in its Turkish message). */
function jpKanaName(n: Notice, result: ValidationResult): string | null {
  const ni = result.name_index;
  const id = n.entity_id;
  if (!id) return null;
  switch (n.rule_id) {
    case 'JPN_001': return ni.stops?.[id] ?? null;
    case 'JPN_008': return n.field === 'route_long_name' ? ni.route_long_names?.[id] || ni.routes?.[id] || null : ni.routes?.[id] ?? null;
    case 'JPN_009': return ni.trips?.[id] ?? null;
    case 'JPN_010': return result.feed_summary?.agencies?.find(a => a.agency_id === id || a.agency_name === id)?.agency_name ?? id;
    default: return null;
  }
}

/** GTFS-JP agency, office and pattern rules: one sentence per finding from the code or value it names. */
const JPN_RECORD_RULES = new Set(['JPN_002', 'JPN_003', 'JPN_005', 'JPN_011', 'JPN_012', 'JPN_013', 'JPN_014', 'JPN_015', 'JPN_016', 'JPN_017', 'JPN_018', 'JPN_020', 'JPN_032']);
function jpRecordText(n: Notice, result: ValidationResult): string | null {
  const d = n.details ?? {};
  const value = n.observed_value ?? '';
  const code = n.entity_id ?? value;
  const agency = (id?: string | null) => (id ? result.feed_summary?.agencies?.find(a => a.agency_id === id)?.agency_name : undefined) || id || '—';
  const line = n.line != null ? String(n.line) : '—';
  switch (n.rule_id) {
    case 'JPN_002':
      return n.entity_type === 'Trip'
        ? t('producer.card.JPN_002.what_trip', { trip: tripPhrase(n.entity_id, result), office: value })
        : t('producer.card.JPN_002.what_route', { route: n.entity_id ? result.name_index.routes?.[n.entity_id] || n.entity_id : '—', office: value });
    case 'JPN_018':
      return t('producer.card.JPN_018.what_detail', { trip: tripPhrase(n.entity_id, result), pattern: value });
    case 'JPN_003': {
      const known = (result.feed_summary?.agencies ?? []).map(a => a.agency_id ? `${a.agency_name} (${a.agency_id})` : a.agency_name);
      return [t('producer.card.JPN_003.what_detail', { code: value }), known.length ? t('producer.card.JPN_003.what_known', { agencies: known.join(', ') }) : ''].filter(Boolean).join(' ');
    }
    case 'JPN_005': return t('producer.card.JPN_005.what_detail', { code });
    case 'JPN_011': return n.file === 'routes.txt'
      ? t('producer.card.JPN_011.what_routes', { count: localizedNumber(value || '0') })
      : t('producer.card.JPN_011.what_agency', { count: localizedNumber(value || '0') });
    case 'JPN_012': return t('producer.card.JPN_012.what_detail', { line });
    case 'JPN_013': return t('producer.card.JPN_013.what_detail', { agency: agency(n.entity_id), value });
    case 'JPN_032': return t('producer.card.JPN_032.what_detail', { agency: agency(n.entity_id), value });
    case 'JPN_014':
    case 'JPN_015':
    case 'JPN_017': {
      const issue = d.issue;
      if (!issue || !['empty', 'duplicate', 'undefined'].includes(issue)) return null;
      const route = n.rule_id === 'JPN_015' && n.entity_id ? result.name_index.routes?.[n.entity_id] || n.entity_id : code;
      return t(`producer.card.${n.rule_id}.what_${issue}`, { code: route, line, first: d.first_line ?? '—' });
    }
    case 'JPN_016': return n.file === 'routes_jp.txt'
      ? t('producer.card.JPN_016.what_route', { route: n.entity_id ? result.name_index.routes?.[n.entity_id] || n.entity_id : '—', value })
      : t('producer.card.JPN_016.what_pattern', { code: n.entity_id || '—', value });
    case 'JPN_020': return n.field === 'office_phone'
      ? t('producer.card.JPN_020.what_phone', { code, value })
      : t('producer.card.JPN_020.what_url', { code, value });
    default: return null;
  }
}

/** XFL cross-file cards: which trip, route, calendar or shape, named the planner's way. */
function xflList(raw?: string | null): { ids: string[]; more: string } {
  const m = (raw ?? '').match(/^(.*?)(?: \(\+(\d+)\))?$/);
  const ids = (m?.[1] ?? '').split(/,\s*/).filter(v => v && v !== '—');
  return { ids, more: m?.[2] ? ` (+${localizedNumber(m[2])})` : '' };
}
function xflText(g: RuleGroup, n: Notice, result: ValidationResult): string | null {
  const d = n.details ?? {};
  const ni = result.name_index;
  switch (n.rule_id) {
    case 'XFL_002': {
      if (!n.entity_id) return null;
      const routes = new Set(g.notices.map(x => x.entity_id ? ni.trip_routes?.[x.entity_id] : undefined).filter(Boolean)).size;
      const detail = t('producer.card.XFL_002.what_detail', { trip: tripPhrase(n.entity_id, result), service: plannerServiceText(d.service_id, result) });
      return g.notices.length > 1 ? `${detail} ${t('producer.card.XFL_002.summary', { count: localizedNumber(String(g.count)), routes: localizedNumber(String(routes)) })}` : detail;
    }
    case 'XFL_006': {
      const { ids, more } = xflList(n.observed_value);
      return ids.length ? t('producer.card.XFL_006.what_detail', { services: ids.join(', ') + more }) : null;
    }
    case 'XFL_012': {
      const route = n.entity_id ? ni.routes?.[n.entity_id] || n.entity_id : null;
      return route && d.trip_count ? t('producer.card.XFL_012.what_detail', { route, count: localizedNumber(d.trip_count) }) : null;
    }
    case 'XFL_013': {
      const fwd = xflList(d.fwd_routes);
      const bwd = xflList(d.bwd_routes);
      if (!fwd.ids.length && !bwd.ids.length) return null;
      const deps = (raw?: string) => { const l = xflList(raw); return l.ids.join(', ') + l.more || '—'; };
      return t('producer.card.XFL_013.what_detail', {
        shape: shapeText(n.entity_id ?? undefined, [...new Set([...fwd.ids, ...bwd.ids])], result),
        fwd: (routeLabels(fwd.ids, result) || '—') + fwd.more, fwd_deps: deps(d.fwd_deps),
        bwd: (routeLabels(bwd.ids, result) || '—') + bwd.more, bwd_deps: deps(d.bwd_deps),
      });
    }
    case 'XFL_020':
    case 'XFL_021': {
      const pair = trfPairText(n, result);
      return pair ? `${t(`producer.card.${n.rule_id}.what`)} ${pair}` : null;
    }
    default:
      return null;
  }
}

/** Station pathway cards: the pathway by its kind, two ends and station (shared engine step). */
function pthMode(mode?: string): string {
  return mode && /^[1-7]$/.test(mode) ? t(`producer.pathway_mode.${mode}`) : t('producer.pathway_mode.unknown');
}
function pthPathway(n: Notice, result: ValidationResult): string | null {
  const d = n.details ?? {};
  if (!d.from_stop_id && !d.to_stop_id) return null;
  const name = (id?: string) => id ? result.name_index.stops?.[id] || id : '—';
  const args = { mode: pthMode(d.pathway_mode), from: name(d.from_stop_id), to: name(d.to_stop_id), station: name(d.station_id) };
  return t(d.station_id ? 'producer.card.PTH.pathway_station' : 'producer.card.PTH.pathway', args);
}
/** PTH summaries and stop-level PTH cards: count with named examples, or the station/node. */
function pthStopText(n: Notice, result: ValidationResult): string | null {
  const d = n.details ?? {};
  const name = (id?: string) => id ? result.name_index.stops?.[id] || id : '—';
  switch (n.rule_id) {
    case 'PTH_008': case 'PTH_009': case 'PTH_025': case 'PTH_029': {
      const examples: string[] = [];
      for (let i = 0; i < 5 && (d[`example_${i}_from_stop_id`] || d[`example_${i}_to_stop_id`]); i++) {
        examples.push(t('producer.card.PTH.example', { mode: pthMode(d[`example_${i}_pathway_mode`]), from: name(d[`example_${i}_from_stop_id`]), to: name(d[`example_${i}_to_stop_id`]) }));
      }
      const count = t('producer.card.PTH.summary', { count: localizedNumber(d.affected_pathways ?? n.observed_value ?? '1') });
      return [t(`producer.card.${n.rule_id}.what`), count, examples.length ? t('producer.card.PTH.examples', { examples: examples.join('; ') }) : ''].filter(Boolean).join(' ');
    }
    case 'PTH_012': {
      // A summary (even when the page has expanded it into one platform row) keeps its count.
      const listed = (d.example_platforms ?? '').split(/,\s*/).filter(Boolean);
      const ids = listed.length ? listed : n.entity_id ? [n.entity_id] : [];
      if (!ids.length) return null;
      const count = d.affected_platforms ? t('producer.card.PTH_012.count', { count: localizedNumber(d.affected_platforms) }) : '';
      return [t('producer.card.PTH_012.what'), count, t('producer.card.PTH_012.platforms', { platforms: ids.map(name).join(', ') })].filter(Boolean).join(' ');
    }
    case 'PTH_013':
      return n.entity_id ? `${t('producer.card.PTH_013.what')} ${t('producer.card.PTH.station', { station: name(n.entity_id) })}` : null;
    case 'PTH_019':
      return n.entity_id ? `${t('producer.card.PTH_019.what')} ${t('producer.card.PTH.node', { node: name(n.entity_id) })}` : null;
    default:
      return null;
  }
}
function pthText(n: Notice, result: ValidationResult): string | null {
  const stopLevel = pthStopText(n, result);
  if (stopLevel !== null || ['PTH_008', 'PTH_009', 'PTH_012', 'PTH_013', 'PTH_019', 'PTH_025', 'PTH_029'].includes(n.rule_id)) return stopLevel;
  const d = n.details ?? {};
  const pathway = pthPathway(n, result);
  if (!pathway) return null;
  switch (n.rule_id) {
    case 'PTH_002':
    case 'PTH_003':
      return `${t(`producer.card.${n.rule_id}.what_detail`, { code: n.observed_value ?? '—' })} ${pathway}`;
    case 'PTH_007': {
      const rows = Number(d.affected_rows);
      return [t('producer.card.PTH_007.what_detail', { value: d.traversal_time ?? '—' }), rows > 1 ? t('producer.card.PTH_007.rows', { rows: localizedNumber(String(rows)) }) : '', pathway].filter(Boolean).join(' ');
    }
    case 'PTH_015': {
      const length = Number(d.length);
      const seconds = Number(d.traversal_time);
      if (!(length > 0 && seconds > 0)) return null;
      return `${t('producer.card.PTH_015.what_detail', {
        length: metersText(d.length), seconds: localizedNumber(d.traversal_time ?? ''),
        speed: (length / seconds).toLocaleString(getLocale(), { maximumFractionDigits: 1 }),
        needed: localizedNumber(String(Math.ceil(length / 3))),
      })} ${pathway}`;
    }
    default:
      // The rule's own sentence, then which pathway it is.
      return `${t(`producer.card.${n.rule_id}.what`)} ${pathway}`;
  }
}

/** Frequency cards: the trip, its period and headway as written on the frequency row. */
function frqText(n: Notice, result: ValidationResult): string | null {
  const d = n.details ?? {};
  const trip = n.rule_id === 'FRQ_001'
    ? t('producer.card.FRQ.unknown_trip', { code: n.entity_id || n.observed_value || '—' })
    : tripPhrase(n.entity_id, result);
  const start = plannerTime(d.start_time) ?? '—';
  const end = plannerTime(d.end_time) ?? '—';
  const headway = d.headway_secs && Number.isFinite(Number(d.headway_secs)) ? durationText(Number(d.headway_secs)) : '—';
  // The page expands a feed summary into its example rows, so tell a summary by its keys.
  if (n.rule_id === 'FRQ_007' && (d.affected_trips || d.example_trips)) {
    const trips = (d.example_trips ?? n.entity_id ?? '').split(/,\s*/).filter(Boolean).map(id => tripShort(id, result));
    return t('producer.card.FRQ_007.what_summary', { count: localizedNumber(d.affected_trips ?? '1'), trips: trips.join('; ') || '—' });
  }
  if (!d.start_time && !d.end_time && !d.headway_secs) return null;
  const args = { trip, start, end, headway, until: plannerTime(d.overlap_until) ?? '—', value: n.observed_value ?? '—' };
  switch (n.rule_id) {
    case 'FRQ_001': case 'FRQ_005': case 'FRQ_006': case 'FRQ_008': case 'FRQ_009':
    case 'FRQ_010': case 'FRQ_011': case 'FRQ_012':
      return t(`producer.card.${n.rule_id}.what_detail`, args);
    default:
      // FRQ_002/003/004: the rule's own sentence, then the period it is on.
      return `${t(`producer.card.${n.rule_id}.what`)} ${t('producer.card.FRQ.period', args)}`;
  }
}

/** Transfer cards: the stop riders leave and the stop they board. The engine gives both in
 *  details; TRF_011 (one finding per stop pair) and older results carry them as "from|to". */
function trfStops(n: Notice): [string | undefined, string | undefined] {
  const d = n.details ?? {};
  if (d.from_stop_id || d.to_stop_id) return [d.from_stop_id, d.to_stop_id];
  const pair = (n.entity_type === 'Transfer' ? n.entity_id || n.scope_key : null)?.split('|');
  return pair && pair.length === 2 ? [pair[0], pair[1]] : [undefined, undefined];
}
function trfPairText(n: Notice, result: ValidationResult): string {
  const name = (id?: string) => id ? result.name_index.stops?.[id] || id : t('producer.card.unknown_stop');
  const [from, to] = trfStops(n);
  return from || to ? t('producer.card.TRF.pair', { from: name(from), to: name(to) }) : '';
}
function minutesText(seconds?: string | null): string {
  const s = Number(seconds);
  return Number.isFinite(s) && seconds ? localizedNumber(String(Math.round(s / 60))) : '—';
}
function trfText(n: Notice, result: ValidationResult): string | null {
  const d = n.details ?? {};
  const pair = trfPairText(n, result);
  const line = n.line != null ? String(n.line) : '—';
  switch (n.rule_id) {
    case 'TRF_010':
      return pair ? t('producer.card.TRF_010.what_detail', { pair, minutes: minutesText(d.min_transfer_time) }) : null;
    case 'TRF_020': {
      const speed = Number((n.observed_value ?? '').match(/^([\d.]+)/)?.[1]);
      const distance = Number(d.distance_m);
      if (!pair || !Number.isFinite(speed) || !Number.isFinite(distance)) return null;
      // At the rule's own threshold (2 m/s) the walk needs distance / 2 seconds.
      return t('producer.card.TRF_020.what_detail', {
        pair, distance: metersText(d.distance_m), seconds: localizedNumber(d.min_transfer_time ?? '—'),
        speed: speed.toLocaleString(getLocale(), { maximumFractionDigits: 1 }),
        needed: minutesText(String(Math.ceil(distance / 2))),
      });
    }
    case 'TRF_011': {
      const meters = (n.observed_value ?? '').match(/^(\d+)/)?.[1];
      return pair && meters ? t('producer.card.TRF_011.what_detail', { pair, distance: metersText(meters) }) : null;
    }
    case 'TRF_003':
      return pair ? t('producer.card.TRF_003.what_detail', { pair, code: n.observed_value ?? '—' }) : null;
    case 'TRF_006':
    case 'TRF_007':
    case 'TRF_008':
    case 'TRF_009':
      return pair ? t(`producer.card.${n.rule_id}.what_detail`, { pair, code: n.observed_value ?? '—' }) : null;
    case 'TRF_012':
    case 'TRF_016':
      return pair ? t(`producer.card.${n.rule_id}.what_detail`, { pair, first: d.first_line ?? '—', line }) : null;
    case 'TRF_005': {
      // A feed summary when several rows lack the time: how many, and the first few transfers.
      if (!d.affected_rows && !d.example_transfers) return pair ? `${t('producer.card.TRF_005.what')} ${pair}` : null;
      const name = (id: string) => result.name_index.stops?.[id] || id;
      const examples = (d.example_transfers ?? '').split(/,\s*/).filter(Boolean)
        .map(key => key.split('|')).filter(p => p.length === 2).map(([a, b]) => `${name(a)} → ${name(b)}`);
      return t('producer.card.TRF_005.what_summary', { rows: localizedNumber(d.affected_rows ?? '1'), examples: examples.join('; ') || '—' });
    }
    case 'TRF_018':
      return t('producer.card.TRF_018.what_detail', { trip: tripPhrase(n.entity_id || d.from_trip_id, result) });
    case 'TRF_022':
    case 'TRF_023':
      if (!d.trip_a || !d.trip_b) return null;
      return t(`producer.card.${n.rule_id}.what_detail`, {
        trip: tripPhrase(n.entity_id, result),
        a: tripPhrase(d.trip_a, result), b: tripPhrase(d.trip_b, result),
        service_a: plannerServiceText(d.service_a, result), service_b: plannerServiceText(d.service_b, result),
        day: ymdText(d.conflict_day),
      });
    default:
      // The other transfer rules keep their own sentence and name the transfer after it.
      return pair ? `${t(`producer.card.${n.rule_id}.what`)} ${pair}` : null;
  }
}

/** GTFS-JP feed-level and profile rules: the value written, how many records, and examples. */
function jpFeedText(n: Notice, result: ValidationResult): string | null {
  const d = n.details ?? {};
  const value = n.observed_value ?? '';
  const agency = (id?: string | null) => (id ? result.feed_summary?.agencies?.find(a => a.agency_id === id || a.agency_name === id)?.agency_name : undefined) || id || '—';
  const examples = (raw?: string) => (raw ?? '').split(/[;,]\s*/).filter(Boolean).slice(0, 5).join(', ') || '—';
  const count = localizedNumber(d.affected_records ?? '1');
  switch (n.rule_id) {
    case 'JPN_006': {
      const variant = d.message_variant;
      return variant && ['unusable', 'missing_rules', 'missing_review'].includes(variant) ? t(`producer.card.JPN_006.what_${variant}`) : null;
    }
    case 'JPN_022': {
      const main = t('producer.card.JPN_022.what_detail', { field: n.field ? plannerFieldName(n.field) : '—', list: plannerListName(n.file) || n.file || '—', count });
      return d.example_record_ids ? `${main} ${t('producer.card.JPN_022.examples', { examples: examples(d.example_record_ids) })}` : main;
    }
    case 'JPN_023': return t('producer.card.JPN_023.what_detail', { value });
    case 'JPN_024': return t('producer.card.JPN_024.what_detail', { agency: agency(n.entity_id), value });
    case 'JPN_025': return t('producer.card.JPN_025.what_detail', { agency: agency(n.entity_id), value });
    case 'JPN_026': return t('producer.card.JPN_026.what_detail', { value, count });
    case 'JPN_027': {
      const routes = (d.example_record_ids ?? '').split(/[;,]\s*/).filter(Boolean).slice(0, 5).map(id => result.name_index.routes?.[id] || id).join(', ') || '—';
      return t('producer.card.JPN_027.what_detail', { type: value ? routeTypeName(value) : '—', count, routes });
    }
    case 'JPN_031': {
      const routes = (d.example_route_ids ?? '').split(/,\s*/).filter(Boolean).map(id => result.name_index.routes?.[id] || id).join(', ') || '—';
      const stopId = n.entity_id;
      return t('producer.card.JPN_031.what_detail', { stop: stopId ? result.name_index.stops?.[stopId] || stopId : '—', routes });
    }
    case 'JPN_033':
      // The custom name IS the finding, so it is shown as written; its list in planner words.
      return n.field
        ? t('producer.card.JPN_033.what_field', { name: value || n.field, list: plannerListName(n.file) || n.file || '—' })
        : t('producer.card.JPN_033.what_file', { name: value || n.file || '—' });
    default: return null;
  }
}

/** JPN_019/021: what the kana row translates — its source text, else the record's own name, else its code. */
function jpTranslationTarget(n: Notice, result: ValidationResult): string {
  const d = n.details ?? {};
  const ni = result.name_index;
  const id = d.record_id;
  const named = id ? (d.table_name === 'stops' ? ni.stops?.[id] : d.table_name === 'routes' ? ni.routes?.[id] : d.table_name === 'trips' ? ni.trips?.[id] : undefined) : undefined;
  return d.field_value || named || id || t('producer.map.unknown_context');
}

/** File-level cards (DQ_016, DQ_021, CAL_020) name their file in the planner's words, not as a file name. */
function dqListLabel(n: Notice): string | null {
  // CAL_020 / OPR_014 / OPR_016 / OPR_025 are about a whole list: name it, not the engine's summary ("11 years").
  if (['DQ_016', 'CAL_020', 'OPR_014', 'OPR_016', 'OPR_025', 'TRP_015'].includes(n.rule_id)) return plannerListName(n.file);
  // RTS_031: the code-less row by the number or name written on it.
  if (n.rule_id === 'RTS_031') return n.details?.route_short_name || n.details?.route_long_name || plannerListName(n.file);
  // SHP_001: the code-less point is a row of the shape point list.
  if (n.rule_id === 'SHP_001') return plannerListName(n.file);
  // STP_002: the code-less row by the name written on it.
  if (n.rule_id === 'STP_002') return n.details?.stop_name || plannerListName(n.file);
  // TRP_001 on a code-less row: the row is in the trip list.
  if ((n.rule_id === 'TRP_001' || n.rule_id === 'TRP_031' || n.rule_id === 'TRP_035') && !n.entity_id) return plannerListName(n.file);
  if (['TRP_023', 'TRP_025', 'TRP_028', 'TRP_029', 'TRP_021'].includes(n.rule_id) && !n.entity_id) return plannerListName(n.file);
  // File-level ARC cards: the list in the planner's words; an unknown file keeps its own name.
  if (ARC_FILE_RULES.has(n.rule_id)) return plannerListName(n.file) || n.file || null;
  const kind = n.rule_id === 'DQ_021' && n.file ? DQ021_KIND[n.file] : undefined;
  return kind ? t(`producer.card.DQ_021.list_${kind}`) : null;
}
const ARC_FILE_RULES = new Set(['ARC_002', 'ARC_003', 'ARC_004', 'ARC_006', 'ARC_007', 'ARC_009', 'ARC_010', 'ARC_011', 'ARC_012', 'ARC_013', 'ARC_014', 'ARC_015', 'ARC_017', 'ARC_018', 'ARC_019', 'ARC_021', 'ARC_022', 'ARC_023', 'ARC_024', 'ARC_025', 'ARC_026', 'ARC_027', 'ARC_030', 'ARC_032', 'ARC_033', 'ARC_034', 'ARC_035', 'ARC_037']);

/** "12,3 KB" from ARC_011's "12345 bytes". */
function fileSizeText(observed?: string | null): string {
  const bytes = Number(observed?.match(/^(\d+)/)?.[1]);
  if (!Number.isFinite(bytes)) return observed || '—';
  const [value, unit] = bytes >= 1024 * 1024 ? [bytes / (1024 * 1024), 'MB'] : bytes >= 1024 ? [bytes / 1024, 'KB'] : [bytes, 'B'];
  return `${localizedNumber(String(Math.round(value * 10) / 10))} ${unit}`;
}

/** Kinds whose repeats are one code on several records (the rest repeat a whole row's meaning). */
const DQ021_CODE_KINDS = new Set(['stop', 'route', 'trip', 'attribution', 'location_group']);

/** The direction as a planner reads it: the trip's headsign, else "outbound (0)"/"inbound (1)". */
function oprDirectionName(d: Record<string, string>, result: ValidationResult, tripId?: string): string {
  const headsign = tripId ? result.name_index.trips?.[tripId] : undefined;
  if (headsign) return headsign;
  return d.direction_id ? t(`producer.card.opr_dir_name_${d.direction_id === '1' ? '1' : '0'}`) : '';
}

/** " „Merkez“ yönünde" for a sentence; empty when the direction is unknown. */
function oprDirection(d: Record<string, string>, result: ValidationResult, tripId?: string): string {
  const name = oprDirectionName(d, result, tripId);
  return name ? t('producer.card.opr_dir', { dir: name }) : '';
}

/** "1 hattının" — the route's planner name, or its code. */
function oprRoute(n: Notice, result: ValidationResult, routeId?: string | null): string {
  const id = routeId ?? routeIdOfNotice(n) ?? (n.entity_id ? result.name_index.trip_routes?.[n.entity_id] : undefined);
  return id ? result.name_index.routes?.[id] || id : '—';
}

/** Each OPR rule's own sentence in the planner's words; null keeps the card's general text. */
function oprRuleText(n: Notice, result: ValidationResult): string | null {
  const ni = result.name_index;
  const d = n.details ?? {};
  const route = oprRoute(n, result);
  const days = n.service_id ? plannerServiceText(n.service_id, result) : '';
  const stopName = (id?: string) => (id ? ni.stops?.[id] || id : '—');
  const tripStops = n.entity_type === 'Trip' && n.entity_id ? ni.trip_stops?.[n.entity_id] ?? [] : [];
  switch (n.rule_id) {
    case 'OPR_001':
      if (!d.gap_from || !d.gap_to || !n.observed_value) return null;
      return t('producer.card.OPR_001.what_detail', {
        route, dir: oprDirection(d, result, d.trip_before), days: days || '—', from: d.gap_from, to: d.gap_to,
        gap: durationText(Number(n.observed_value) * 60), limit: durationText(Number(/\d+/.exec(n.expected_value ?? '')?.[0]) * 60),
      });
    case 'OPR_003':
      if (!d.gap_from || !d.gap_to || !n.observed_value) return null;
      return t('producer.card.OPR_003.what_detail', {
        route, dir: oprDirection(d, result, d.trip_before), days: days || '—', stop: stopName(d.stop_id),
        from: d.gap_from, to: d.gap_to, gap: durationText(Math.max(60, Number(n.observed_value) * 60)),
      });
    case 'OPR_004':
      return t('producer.card.OPR_004.what_detail', { route });
    case 'OPR_005':
      if (!d.median_min || !n.observed_value) return null;
      return t(d.kind === 'dense' ? 'producer.card.OPR_005.what_dense' : 'producer.card.OPR_005.what_sparse', {
        route, dir: oprDirection(d, result), days: days || '—', hw: localizedNumber(n.observed_value), median: localizedNumber(d.median_min),
        first: d.first_departure || '—', last: d.last_departure || '—', n: localizedNumber(d.departures || '—'),
      });
    case 'OPR_006':
      return n.entity_id ? t('producer.card.OPR_006.what_detail', { trip: tripPhrase(n.entity_id, result), stop: stopName(tripStops[0]) }) : null;
    case 'OPR_007': {
      const times = /^(\d+)×/.exec(n.observed_value ?? '')?.[1];
      return n.entity_id && d.dup_stop && times
        ? t('producer.card.OPR_007.what_detail', { trip: tripPhrase(n.entity_id, result), stop: stopName(d.dup_stop), n: times })
        : null;
    }
    case 'OPR_008': {
      const max = /[\d.]+/.exec(n.observed_value ?? '')?.[0];
      const limit = /[\d.]+/.exec(n.expected_value ?? '')?.[0];
      return n.entity_id && max && limit
        ? t('producer.card.OPR_008.what_detail', {
          trip: tripPhrase(n.entity_id, result, 'gen'), n: localizedNumber(String(badSegments(d).length)),
          max: localizedNumber(String(Math.round(Number(max)))), limit: localizedNumber(limit),
        })
        : null;
    }
    case 'OPR_009':
      return d.night_trips
        ? t(d.first_departure === d.last_departure ? 'producer.card.OPR_009.what_one' : 'producer.card.OPR_009.what_detail', {
          route, n: localizedNumber(d.night_trips), first: d.first_departure || '—', last: d.last_departure || '—' })
        : null;
    case 'OPR_010':
      if (!d.yes_trips || !d.no_trips) return null;
      return t(n.field === 'bikes_allowed' ? 'producer.card.OPR_010.what_bikes' : 'producer.card.OPR_010.what_wheelchair', {
        route, yes: localizedNumber(d.yes_trips), no: localizedNumber(d.no_trips),
      });
    case 'OPR_011':
      return n.entity_id ? t('producer.card.OPR_011.what_detail', { svc: n.entity_id }) : null;
    case 'OPR_012':
      return n.entity_id && d.gap_start && d.gap_end
        ? t('producer.card.OPR_012.what_detail', { svc: n.entity_id, from: ymdText(d.gap_start), to: ymdText(d.gap_end), gap: localizedNumber(n.observed_value ?? '—') })
        : null;
    case 'OPR_013': {
      const dir = /direction_id=(\d)/.exec(n.observed_value ?? '')?.[1];
      return dir ? t('producer.card.OPR_013.what_detail', { route, dir: t(`producer.card.opr_dir_name_${dir === '1' ? '1' : '0'}`) }) : null;
    }
    case 'OPR_014':
      return d.transfer_count && d.average_seconds
        ? t('producer.card.OPR_014.what_detail', { n: localizedNumber(d.transfer_count), avg: durationText(Number(d.average_seconds)) })
        : null;
    case 'OPR_015':
      return d.shape_id ? t('producer.card.OPR_015.what_detail', { route, shape: d.shape_id }) : null;
    case 'OPR_016':
      return t('producer.card.OPR_016.what_detail');
    case 'OPR_017': {
      const metres = /\d+/.exec(n.observed_value ?? '')?.[0];
      return n.entity_id && metres
        ? t('producer.card.OPR_017.what_detail', {
          trip: tripPhrase(n.entity_id, result), m: localizedNumber(metres), from: stopName(tripStops[0]), to: stopName(tripStops[tripStops.length - 1]),
          n: localizedNumber(String(tripStops.length)),
        })
        : null;
    }
    case 'OPR_019':
    case 'OPR_020': {
      const dates = listOf(d.sample_dates?.replace(/,/g, ', ')).slice(0, 3).map(ymdText).join(', ');
      return d.conflict_day_count && dates
        ? t(`producer.card.${n.rule_id}.what_detail`, { route, n: localizedNumber(d.conflict_day_count), dates })
        : null;
    }
    case 'OPR_021':
    case 'OPR_022':
    case 'OPR_023': {
      const first = listOf(d.sample_dates)[0];
      return d.conflict_day_count && first
        ? t(`producer.card.${n.rule_id}.what_detail`, {
          route, n: localizedNumber(d.conflict_day_count), first: ymdText(first), base: d.base_services || '—', override: d.override_services || '—',
        })
        : null;
    }
    case 'OPR_024':
      return n.observed_value && n.service_id
        ? t('producer.card.OPR_024.what_detail', { route, svc: n.service_id, n: localizedNumber(n.observed_value), limit: localizedNumber(/\d+/.exec(n.expected_value ?? '')?.[0] ?? '500') })
        : null;
    case 'OPR_025':
      return n.observed_value ? t('producer.card.OPR_025.what_detail', { avg: localizedNumber(/\d+/.exec(n.observed_value)?.[0] ?? '—') }) : null;
    default:
      return null;
  }
}

/** OPR tables: the two departures around a gap, the yes/no trips, the longest transfers, the
 *  shortest trips, the conflicting dates, the route's calendars. */
function oprTable(n: Notice, result: ValidationResult): string {
  const ni = result.name_index;
  const d = n.details ?? {};
  const stopName = (id?: string) => (id ? (ni.stops?.[id] ? `${ni.stops[id]} (${id})` : id) : '—');
  const table = (caption: string, head: string[], rows: string[][], bad: (i: number) => boolean = () => false) =>
    `<p class="producer-card-label">${escHtml(caption)}</p><table class="planner-times planner-stop-times"><thead><tr>${head.map(h => `<th>${escHtml(h)}</th>`).join('')}</tr></thead><tbody>${
      rows.map((r, i) => `<tr${bad(i) ? ' class="bad"' : ''}>${r.map(c => `<td>${escHtml(c)}</td>`).join('')}</tr>`).join('')}</tbody></table>`;
  switch (n.rule_id) {
    case 'OPR_001':
    case 'OPR_003':
      if (!d.gap_from || !d.gap_to) return '';
      return table(t(n.rule_id === 'OPR_001' ? 'producer.times.opr_gap' : 'producer.times.opr_bunch'),
        [t('producer.col.departure'), t('producer.col.direction')],
        [[d.gap_from, (d.trip_before && ni.trips?.[d.trip_before]) || '—'],
          [d.gap_to, (d.trip_after && ni.trips?.[d.trip_after]) || '—']], () => true);
    case 'OPR_004': {
      const services = listOf(d.services);
      return services.length
        ? table(t('producer.times.opr_route_services'), [t('producer.col.service_code'), t('producer.col.service'), t('producer.times.validity')],
          services.slice(0, CAL_SERVICE_ROWS).map(svc => [svc, plannerServiceText(svc, result), serviceRangeText(svc, result)]))
        : '';
    }
    case 'OPR_010':
      if (!d.yes_trips || !d.no_trips) return '';
      return table(t('producer.times.opr_policy'), [t('producer.times.value'), t('producer.times.trip_count'), t('producer.times.example_trip')], [
        [t(n.field === 'bikes_allowed' ? 'producer.times.opr_bikes_yes' : 'producer.times.opr_wheelchair_yes'), localizedNumber(d.yes_trips), tripShort(d.yes_trip_id, result, false)],
        [t(n.field === 'bikes_allowed' ? 'producer.times.opr_bikes_no' : 'producer.times.opr_wheelchair_no'), localizedNumber(d.no_trips), tripShort(d.no_trip_id, result, false)],
      ]);
    case 'OPR_014': {
      const rows: string[][] = [];
      for (let i = 0; d[`example_${i}_seconds`]; i++) {
        rows.push([stopName(d[`example_${i}_from_stop_id`]), stopName(d[`example_${i}_to_stop_id`]), durationText(Number(d[`example_${i}_seconds`]))]);
      }
      return rows.length ? table(t('producer.times.opr_transfers'), [t('producer.times.opr_from'), t('producer.times.opr_to'), t('producer.times.opr_transfer_time')], rows) : '';
    }
    case 'OPR_025': {
      const rows: string[][] = [];
      for (let i = 0; d[`example_${i}_trip_id`]; i++) {
        const trip = d[`example_${i}_trip_id`]!;
        const routeId = ni.trip_routes?.[trip];
        rows.push([routeId ? routeBadge(routeId, ni).label : '—', tripShort(trip, result, false), durationText(Number(d[`example_${i}_seconds`]))]);
      }
      return rows.length ? table(t('producer.times.opr_short_trips'), [t('producer.col.route'), t('producer.col.trip'), t('producer.times.opr_duration')], rows, () => true) : '';
    }
    case 'OPR_019': case 'OPR_020': case 'OPR_021': case 'OPR_022': case 'OPR_023': {
      const dates = listOf(d.sample_dates?.replace(/,/g, ', '));
      const weekday = new Intl.DateTimeFormat(intlLocale(), { weekday: 'long', timeZone: 'UTC' });
      const rows = dates.map(v => [ymdText(v), /^\d{8}$/.test(v) ? weekday.format(new Date(Date.UTC(Number(v.slice(0, 4)), Number(v.slice(4, 6)) - 1, Number(v.slice(6, 8))))) : '—']);
      const more = Number(d.conflict_day_count) - dates.length;
      if (more > 0) rows.push([t('producer.times.more_days', { n: localizedNumber(String(more)) }), '']);
      return rows.length ? table(t('producer.times.opr_dates'), [t('producer.times.date'), t('producer.times.weekday')], rows, i => i < dates.length) : '';
    }
    default:
      return '';
  }
}

/** Each TRP rule's own sentence in the planner's words; trips named by route, direction and time. */
function trpRuleText(n: Notice, result: ValidationResult): string | null {
  const ni = result.name_index;
  const d = n.details ?? {};
  const trip = n.entity_type === 'Trip' ? n.entity_id ?? undefined : undefined;
  const value = (n.observed_value ?? '').trim();
  switch (n.rule_id) {
    case 'TRP_001':
      if (trip) {
        return d.first_line && n.line != null
          ? t('producer.card.TRP_001.what_dup', { code: trip, first: d.first_line, line: String(n.line) })
          : null;
      }
      if (n.line == null) return null;
      {
        // The code-less row named by its route, direction and calendar (from the row itself).
        const parts = [
          d.route_id ? t('producer.card.trp_row_route', { route: routeBadge(d.route_id, ni).label }) : '',
          d.trip_headsign ? t('producer.trip.dir', { headsign: d.trip_headsign }) : '',
          d.service_id ? plannerServiceText(d.service_id, result) : '',
        ].filter(Boolean);
        return t('producer.card.TRP_001.what_empty', { line: String(n.line), ctx: parts.length ? ` (${parts.join(', ')})` : '' });
      }
    case 'TRP_002':
      // The route is the missing thing: name the trip by direction and time only.
      return trip && value ? t('producer.card.TRP_002.what_detail', { trip: tripPhrase(trip, result, 'subj', false), route: value }) : null;
    case 'TRP_003':
      return d.affected_trips
        ? t('producer.card.TRP_003.what_detail', { n: localizedNumber(d.affected_trips), services: d.example_service_ids || '—' })
        : null;
    case 'TRP_004':
      return d.affected_trips && d.example_trip
        ? t('producer.card.TRP_004.what_detail', { shape: n.entity_id || value || '—', n: localizedNumber(d.affected_trips), trip: tripPhrase(d.example_trip, result) })
        : null;
    case 'TRP_005':
      return d.affected_trips
        ? t('producer.card.TRP_005.what_detail', { n: localizedNumber(d.affected_trips), values: (d.example_values || '—').split(' | ').map(v => `"${v}"`).join(', ') })
        : null;
    case 'TRP_006':
    case 'TRP_007':
      return trip && n.observed_value != null ? t(`producer.card.${n.rule_id}.what_detail`, { trip: tripPhrase(trip, result, 'gen'), value }) : null;
    case 'TRP_011':
      return trip ? t('producer.card.TRP_011.what_detail', { trip: tripPhrase(trip, result, 'gen') }) : null;
    case 'TRP_012':
      return n.observed_value
        ? t('producer.card.TRP_012.what_detail', { route: oprRoute(n, result), n: localizedNumber(n.observed_value) })
        : null;
    case 'TRP_013': {
      const one = d.example_trip_id || routeExampleTrip(ni, routeIdOfNotice(n));
      return one ? t('producer.card.TRP_013.what_detail', { route: oprRoute(n, result), trip: tripShort(one, result, false) }) : null;
    }
    case 'TRP_014':
      return trip && d.trip_short_name
        ? t('producer.card.TRP_014.what_detail', { trip: tripPhrase(trip, result, 'gen'), name: d.trip_short_name, n: localizedNumber(value) })
        : null;
    case 'TRP_015':
      return d.trips_with_block && n.observed_value
        ? t('producer.card.TRP_015.what_detail', { trips: localizedNumber(d.trips_with_block), blocks: localizedNumber(n.observed_value) })
        : null;
    case 'TRP_017':
      if (!trip) return null;
      return d.start_time && d.end_time && d.headway_secs
        ? t('producer.card.TRP_017.what_detail', {
          trip: tripPhrase(trip, result), start: d.start_time, end: d.end_time, headway: durationText(Number(d.headway_secs)),
        })
        : t('producer.card.TRP_017.what_plain', { trip: tripPhrase(trip, result) });
    case 'TRP_019':
      return trip ? t('producer.card.TRP_019.what_detail', { trip: tripPhrase(trip, result) }) : null;
    case 'TRP_020':
      return trip && value && d.stop_id
        ? t('producer.card.TRP_020.what_detail', {
          trip: tripPhrase(trip, result, 'gen'), headsign: value, stop: ni.stops?.[d.stop_id] || d.stop_id,
          seq: d.stop_sequence || '—', last: (d.last_stop_id && (ni.stops?.[d.last_stop_id] || d.last_stop_id)) || '—',
        })
        : null;
    case 'TRP_021':
    case 'TRP_025':
    case 'TRP_028':
    case 'TRP_029': {
      if (!d.missing_trips || !d.trip_count) return null;
      const all = d.missing_trips === d.trip_count;
      const kind = n.rule_id === 'TRP_021' ? 'bikes' : 'wheelchair';
      return t(`producer.card.trp_${kind}_${all ? 'all' : 'some'}`, {
        n: localizedNumber(d.missing_trips), total: localizedNumber(d.trip_count),
        pct: localizedNumber(String(Math.round(Number(d.missing_trips) * 100 / Math.max(1, Number(d.trip_count))))),
      });
    }
    case 'TRP_022':
      return trip && d.other_trip_id && d.block_id
        ? t('producer.card.TRP_022.what_detail', {
          trip: tripPhrase(trip, result), other: tripPhrase(d.other_trip_id, result), block: d.block_id,
          start: d.start || '—', end: d.end || '—', other_start: d.other_start || '—', other_end: d.other_end || '—',
        })
        : null;
    case 'TRP_023':
      return t(d.next_active ? 'producer.card.TRP_023.what_detail' : 'producer.card.TRP_023.what_none', { next: ymdText(d.next_active) });
    case 'TRP_024': {
      const types = [...new Set([/route_type=([\d/]+)/.exec(n.expected_value ?? '')?.[1], /route_type=([\d/]+)/.exec(n.observed_value ?? '')?.[1]]
        .filter((v): v is string => Boolean(v)).flatMap(v => v.split('/')))].map(routeTypeName);
      return d.block_id && types.length
        ? t('producer.card.TRP_024.what_detail', { block: d.block_id, types: types.join(', '), n: localizedNumber(String(Number(d.trip_count || '0') + 1)) })
        : null;
    }
    case 'TRP_026':
      return trip && value ? t('producer.card.TRP_026.what_detail', { trip: tripPhrase(trip, result), svc: value }) : null;
    case 'TRP_031':
    case 'TRP_035': {
      if (n.line == null) return null;
      const parts = [
        d.trip_id ? tripShort(d.trip_id, result, n.rule_id === 'TRP_035') : '',
        n.rule_id === 'TRP_031' && d.service_id ? plannerServiceText(d.service_id, result) : '',
      ].filter(v => v && v !== d.trip_id);
      if (!parts.length && d.trip_headsign) parts.push(t('producer.trip.dir', { headsign: d.trip_headsign }));
      return t(`producer.card.${n.rule_id}.what_detail`, { line: String(n.line), ctx: parts.length ? ` (${parts.join(', ')})` : '' });
    }
    case 'TRP_032':
      return trip && n.observed_value != null ? t('producer.card.TRP_032.what_detail', { trip: tripPhrase(trip, result, 'gen'), value }) : null;
    case 'TRP_034':
      return trip && n.field
        ? t('producer.card.TRP_034.what_detail', { trip: tripPhrase(trip, result, 'gen'), field: plannerFieldName(n.field).toLocaleLowerCase(intlLocale()), value })
        : null;
    default:
      return null;
  }
}

/** The core mode of a route type code; extended codes by their mode family. */
function routeBaseType(code: string): number {
  const c = Number(code);
  return !Number.isFinite(c) ? NaN
    : c < 100 ? c
      : c < 200 ? 2 : c < 300 ? 3 : c < 500 ? 1 : c < 800 ? 3 : c < 900 ? 11 : c < 1000 ? 0
        : c < 1200 ? 4 : c < 1400 ? 6 : c < 1500 ? 7 : NaN;
}

/** A route type code as a mode planners name ("Otobüs"); extended codes by their mode family. */
function routeTypeName(code: string): string {
  const base = routeBaseType(code);
  const key = `producer.route_type.${base}`;
  const text = t(key);
  return text === key ? code : `${text} (${code})`;
}

/** TRP tables: the trips without a direction, the blocks with their one trip. */
function trpTable(n: Notice, result: ValidationResult): string {
  const ni = result.name_index;
  const d = n.details ?? {};
  const stopName = (id?: string) => (id ? ni.stops?.[id] || id : '—');
  const route = (tripId: string) => {
    const stops = ni.trip_stops?.[tripId] ?? [];
    return stops.length ? `${stopName(stops[0])} → ${stopName(stops[stops.length - 1])}` : '—';
  };
  const table = (caption: string, head: string[], rows: string[][]) =>
    `<p class="producer-card-label">${escHtml(caption)}</p><table class="planner-times planner-stop-times"><thead><tr>${head.map(h => `<th>${escHtml(h)}</th>`).join('')}</tr></thead><tbody>${
      rows.map(r => `<tr class="bad">${r.map(c => `<td>${escHtml(c)}</td>`).join('')}</tr>`).join('')}</tbody></table>`;
  if (n.rule_id === 'TRP_012' && n.entity_type === 'Route') {
    const trips = listOf(d.example_trips);
    return trips.length
      ? table(t('producer.times.trp012_trips'), [t('producer.col.trip'), t('producer.times.trp_path')], trips.map(id => [tripShort(id, result, false), route(id)]))
      : '';
  }
  if (n.rule_id === 'TRP_022' && d.other_trip_id && n.entity_id) {
    return table(t('producer.times.trp022_trips'), [t('producer.col.trip'), t('producer.times.trp_start'), t('producer.times.trp_end')], [
      [tripShort(n.entity_id, result), d.start || '—', d.end || '—'],
      [tripShort(d.other_trip_id, result), d.other_start || '—', d.other_end || '—'],
    ]);
  }
  if (n.rule_id === 'TRP_024' && d.trips) {
    const trips = [d.first_trip, ...listOf(d.trips)].filter((id): id is string => Boolean(id));
    return table(t('producer.times.trp024_trips', { block: d.block_id || '—' }), [t('producer.col.trip')], [...new Set(trips)].map(id => [tripShort(id, result)]));
  }
  if (n.rule_id === 'TRP_017' && d.start_time && d.end_time) {
    return table(t('producer.times.trp017_frequency'), [t('producer.times.trp_start'), t('producer.times.trp_end'), t('producer.times.trp_headway')],
      [[d.start_time, d.end_time, d.headway_secs ? durationText(Number(d.headway_secs)) : '—']]);
  }
  if (n.rule_id === 'TRP_015') {
    const rows: string[][] = [];
    for (let i = 0; d[`example_${i}_block`]; i++) rows.push([d[`example_${i}_block`]!, tripShort(d[`example_${i}_trip_id`], result)]);
    return rows.length ? table(t('producer.times.trp015_blocks'), [t('producer.times.trp_block'), t('producer.col.trip')], rows) : '';
  }
  return '';
}

/** STP findings: the stop a row is about and its trip context — a summary row (STP_004/005)
 *  reads the context of its own example stop (`example_<i>_*`). */
function stpContext(n: Notice, side?: 'other' | 'nearest'): { stop?: string; trip?: string; prev?: string; next?: string; seq?: string; routes: string[]; trips?: string } {
  const d = n.details ?? {};
  let prefix = side ? `${side}_` : '';
  if (n.rule_id === 'STP_004' || n.rule_id === 'STP_005') {
    let i = 0;
    while (d[`example_${i}_stop_id`] && d[`example_${i}_stop_id`] !== n.entity_id) i++;
    if (!d[`example_${i}_stop_id`]) i = 0;
    prefix = `example_${i}_`;
  }
  const at = (key: string) => d[`${prefix}${key}`] || undefined;
  const stop = side === 'other' ? d.other_stop_id : side === 'nearest' ? d.nearest_used_stop_id
    : (prefix ? at('stop_id') : undefined) ?? n.entity_id ?? undefined;
  return {
    stop, trip: at('example_trip_id'), prev: at('prev_stop_id'),
    next: at('next_stop_id'), seq: at('stop_sequence'), routes: listOf(at('routes')), trips: at('trip_count'),
  };
}

/** "Bu durağa 1 hattın 12 seferi uğruyor (S8). Örneğin … Çarşı ile Liman arasında bu durağa uğruyor." */
function stpUseText(n: Notice, result: ValidationResult): string {
  const c = stpContext(n);
  if (!c.trips) return '';
  const name = (id?: string) => (id ? result.name_index.stops?.[id] || id : '—');
  const use = t('producer.card.stp_use', { n: localizedNumber(c.trips), routes: routeLabels(c.routes, result) });
  if (!c.trip) return use;
  const trip = tripPhrase(c.trip, result);
  const where = c.prev && c.next ? t('producer.card.stp_between', { trip, prev: name(c.prev), next: name(c.next) })
    : c.next ? t('producer.card.stp_first', { trip, next: name(c.next) })
      : c.prev ? t('producer.card.stp_last', { trip, prev: name(c.prev) }) : '';
  return [use, where].filter(Boolean).join(' ');
}

/** A location type code in the planner's words ("istasyon"). */
function locationTypeName(code?: string | null): string {
  const key = `producer.location_type.${code === '' || code == null ? '0' : code}`;
  const text = t(key);
  return text === key ? String(code) : text;
}

/** Each STP rule's own sentence: the stop by name, what is wrong with it, and where it is used. */
function stpRuleText(n: Notice, result: ValidationResult): string | null {
  const ni = result.name_index;
  const d = n.details ?? {};
  const stop = (id?: string | null) => (id ? ni.stops?.[id] || id : '—');
  const value = (n.observed_value ?? '').trim();
  const use = stpUseText(n, result);
  // The stretch is only claimed when the map can draw it: both neighbours have a position.
  const stretch = (c: ReturnType<typeof stpContext>) => Boolean(c.stop && !ni.stop_coords?.[c.stop]
    && c.prev && ni.stop_coords?.[c.prev] && c.next && ni.stop_coords?.[c.next]);
  const place = stretch(stpContext(n)) ? t('producer.card.stp_place') : '';
  const join = (...parts: string[]) => parts.filter(Boolean).join(' ');
  switch (n.rule_id) {
    case 'STP_001': {
      if (!n.entity_id || !d.first_line || n.line == null) return null;
      const dist = d.first_lat && d.lat
        ? metersBetween([Number(d.first_lat), Number(d.first_lon)], [Number(d.lat), Number(d.lon)])
        : NaN;
      return join(t('producer.card.STP_001.what_detail', {
        code: n.entity_id, first_line: d.first_line, line: String(n.line), first: d.first_name || '—', second: d.name || '—',
        dist: Number.isFinite(dist) ? t('producer.card.STP_001.dist', { dist: metersText(String(dist)) }) : '',
      }), use);
    }
    case 'STP_002': {
      if (n.line == null) return null;
      const ctx = [d.stop_name ? `„${d.stop_name}“` : '', d.stop_code ? t('producer.card.stp_code', { code: d.stop_code }) : ''].filter(Boolean).join(', ');
      return t('producer.card.STP_002.what_detail', { line: String(n.line), ctx: ctx ? ` (${ctx})` : '' });
    }
    case 'STP_003':
      if (!n.entity_id) return null;
      return n.field === 'stop_name'
        ? join(t('producer.card.STP_003.what_name', { code: n.entity_id }), use)
        : join(t('producer.card.STP_003.what_lat', { stop: stop(n.entity_id), value }), use, place);
    case 'STP_004':
    case 'STP_005': {
      const count = d.affected_stops || n.observed_value;
      if (!count) return null;
      const c = stpContext(n);
      return join(t(`producer.card.${n.rule_id}.what_detail`, { n: localizedNumber(count) }),
        c.stop ? t('producer.card.stp_example', { stop: stop(c.stop) }) : '', use, place);
    }
    case 'STP_006':
    case 'STP_007':
      return n.entity_id ? join(t(`producer.card.${n.rule_id}.what_detail`, { stop: stop(n.entity_id) }), use, place) : null;
    case 'STP_008':
      if (!n.entity_id) return null;
      return join(t('producer.card.STP_008.what_detail', { stop: stop(n.entity_id), value }), use,
        stpContext(n).trips ? t('producer.card.STP_008.what_used') : '');
    case 'STP_009':
      if (!n.entity_id || !value) return null;
      return join(t('producer.card.STP_009.what_detail', { stop: stop(n.entity_id), parent: value }),
        d.nearest_station_id ? t('producer.card.STP_009.what_near', { station: stop(d.nearest_station_id), dist: metersText(d.nearest_station_m) }) : '', use);
    case 'STP_011':
      if (!n.entity_id || !d.location_type) return null;
      return join(t('producer.card.STP_011.what_detail', { stop: stop(n.entity_id), type: locationTypeName(d.location_type) }),
        d.nearest_station_id ? t('producer.card.STP_009.what_near', { station: stop(d.nearest_station_id), dist: metersText(d.nearest_station_m) }) : '');
    case 'STP_012':
      if (!n.entity_id) return null;
      return join(t('producer.card.STP_012.what_detail', { stop: stop(n.entity_id), type: locationTypeName(n.observed_value) }), use,
        d.platforms ? t('producer.card.STP_012.what_platforms', { platforms: listOf(d.platforms).map(id => `„${stop(id)}“`).join(', ') }) : '');
    case 'STP_013':
      return n.entity_id ? join(t('producer.card.STP_013.what_detail', { stop: stop(n.entity_id), value }), use) : null;
    case 'STP_014':
      return n.entity_id && value
        ? join(t('producer.card.STP_014.what_detail', { stop: stop(n.entity_id), value }), d.agency_timezone ? t('producer.card.STP_014.what_agency', { tz: d.agency_timezone }) : '', use)
        : null;
    case 'STP_015': {
      if (!n.entity_id || !value) return null;
      const levels = (d.known_levels ?? '').split('|').filter(Boolean).map(v => { const [code, name] = v.split('='); return name ? `${name} (${code})` : code; });
      return join(t('producer.card.STP_015.what_detail', { stop: stop(n.entity_id), value }),
        levels.length ? t('producer.card.STP_015.what_levels', { levels: levels.join(', ') }) : t('producer.card.STP_015.what_nolevels'), use);
    }
    case 'STP_016':
    case 'STP_017': {
      if (!n.entity_id || !d.other_stop_id) return null;
      const a = stpContext(n);
      const b = stpContext(n, 'other');
      return join(t(`producer.card.${n.rule_id}.what_detail`, { stop: stop(n.entity_id), other: stop(d.other_stop_id), dist: metersText(d.distance_m) }),
        t('producer.card.stp_pair_use', {
          stop: stop(n.entity_id), other: stop(d.other_stop_id), n: localizedNumber(a.trips || '0'), m: localizedNumber(b.trips || '0'),
          routes: routeLabels(a.routes, result), oroutes: routeLabels(b.routes, result),
        }));
    }
    case 'STP_019':
      return n.entity_id ? join(t('producer.card.STP_019.what_detail', { stop: stop(n.entity_id), n: localizedNumber(value || '—') }), use) : null;
    case 'STP_020': {
      if (!n.entity_id) return null;
      const c = stpContext(n, 'nearest');
      return join(t('producer.card.STP_020.what_detail', { stop: stop(n.entity_id) }), d.nearest_used_stop_id
        ? t(c.trips ? 'producer.card.STP_020.what_near' : 'producer.card.STP_020.what_near_plain', {
          near: stop(d.nearest_used_stop_id), dist: metersText(d.nearest_used_m), n: localizedNumber(c.trips || '0'), routes: routeLabels(c.routes, result),
        })
        : '');
    }
    case 'STP_021':
      if (!n.entity_id || !d.parent_station) return null;
      return t('producer.card.STP_021.what_detail', { stop: stop(n.entity_id), parent: stop(d.parent_station), type: locationTypeName(d.parent_location_type) });
    case 'STP_022':
      if (Number(d.affected_stops) > 1) {
        return join(t('producer.card.STP_022.what_many', { n: localizedNumber(d.affected_stops!) }),
          n.entity_id ? t('producer.card.stp_example', { stop: stop(n.entity_id) }) : '');
      }
      return n.entity_id ? join(t('producer.card.STP_022.what_detail', { stop: stop(n.entity_id) }), use) : null;
    case 'STP_023':
      return n.entity_id && value ? join(t('producer.card.STP_023.what_detail', { stop: stop(n.entity_id), value }), use) : null;
    case 'STP_024':
    case 'STP_026':
      return n.entity_id ? join(t('producer.card.stp_access', { stop: stop(n.entity_id), value }), use) : null;
    case 'STP_028':
      return n.entity_id && d.stop_code
        ? join(t('producer.card.STP_028.what_detail', { stop: stop(n.entity_id), code: d.stop_code, n: localizedNumber(value || String(d.stop_code.length)) }), use)
        : null;
    case 'STP_029': {
      if (!n.entity_id || !d.parent_id) return null;
      const dist = /[\d.]+/.exec(value)?.[0];
      const limit = /[\d.]+/.exec(n.expected_value ?? '')?.[0];
      return join(t('producer.card.STP_029.what_detail', { stop: stop(n.entity_id), parent: stop(d.parent_id), dist: metersText(dist), limit: metersText(limit) }),
        d.nearest_station_id ? t('producer.card.STP_009.what_near', { station: stop(d.nearest_station_id), dist: metersText(d.nearest_station_m) }) : '', use);
    }
    case 'STP_030': {
      if (!n.entity_id) return null;
      const near = listOf(d.nearby_stops);
      const dists = listOf(d.nearby_m);
      return join(t('producer.card.STP_030.what_detail', { stop: stop(n.entity_id) }), near.length
        ? t('producer.card.STP_030.what_near', { stops: near.map((id, i) => `„${stop(id)}“ (${metersText(dists[i])})`).join(', ') })
        : '');
    }
    case 'STP_031':
      return n.entity_id ? join(t('producer.card.STP_031.what_detail', { stop: stop(n.entity_id), value: value || stop(n.entity_id) }), use) : null;
    case 'STP_033': {
      if (Number(d.affected_stops) > 1 || !n.entity_id) {
        const count = d.affected_stops || n.observed_value;
        if (!count) return null;
        return join(t('producer.card.STP_033.what_many', { n: localizedNumber(count) }),
          d.zoned_stops === '0' ? t('producer.card.STP_033.what_nozones') : d.zoned_stops ? t('producer.card.STP_033.what_zoned', { n: localizedNumber(d.zoned_stops) }) : '',
          n.entity_id ? t('producer.card.stp_example', { stop: stop(n.entity_id) }) : '');
      }
      const c = stpContext(n);
      const zones = [
        c.prev && d.prev_zone ? t('producer.card.STP_033.zone_prev', { stop: stop(c.prev), zone: d.prev_zone }) : '',
        c.next && d.next_zone ? t('producer.card.STP_033.zone_next', { stop: stop(c.next), zone: d.next_zone }) : '',
      ].filter(Boolean);
      return join(t('producer.card.STP_033.what_detail', { stop: stop(n.entity_id) }), use,
        zones.length ? t('producer.card.STP_033.what_zones', { zones: zones.join(t('producer.card.stp_and')) }) : '');
    }
    case 'STP_034':
    case 'STP_035': {
      if (!n.entity_id || !value) return null;
      const owner = n.rule_id === 'STP_034' ? t('producer.card.STP_034.owner')
        : t('producer.card.STP_035.owner', { routes: routeLabels(listOf(d.route_ids), result) });
      return t('producer.card.stp_url_same', { stop: stop(n.entity_id), owner, url: value, n: localizedNumber(d.stop_count || '1') });
    }
    case 'STP_036':
      if (!n.entity_id || !d.parent_station) return null;
      return join(t('producer.card.STP_036.what_detail', { stop: stop(n.entity_id), parent: stop(d.parent_station) }), use);
    case 'STP_037':
    case 'STP_038': {
      if (!d.missing_stops || !d.stop_count) return null;
      const calls = listOf(d.example_stop_calls);
      const busiest = listOf(d.example_stops).map((id, i) => t('producer.card.stp_calls', { stop: stop(id), n: localizedNumber(calls[i] || '0') }));
      return join(n.rule_id === 'STP_038'
        ? t('producer.card.STP_038.what_detail', { total: localizedNumber(d.stop_count) })
        : t('producer.card.STP_037.what_detail', {
          n: localizedNumber(d.missing_stops), total: localizedNumber(d.stop_count),
          pct: localizedNumber(String(Math.round(Number(d.missing_stops) * 100 / Math.max(1, Number(d.stop_count))))),
        }),
      busiest.length ? t('producer.card.stp_busiest', { stops: busiest.join(', ') }) : '');
    }
    case 'STP_039': {
      if (!n.entity_id || !d.other_stop_id || !value) return null;
      const a = stpContext(n);
      const b = stpContext(n, 'other');
      const p = ni.stop_coords?.[n.entity_id];
      const q = ni.stop_coords?.[d.other_stop_id];
      return join(t('producer.card.STP_039.what_detail', { code: value, stop: stop(n.entity_id), other: stop(d.other_stop_id) }),
        p && q ? t('producer.card.stp_apart', { dist: metersText(String(metersBetween(p, q))) }) : '',
        t('producer.card.stp_pair_use', {
          stop: stop(n.entity_id), other: stop(d.other_stop_id), n: localizedNumber(a.trips || '0'), m: localizedNumber(b.trips || '0'),
          routes: routeLabels(a.routes, result), oroutes: routeLabels(b.routes, result),
        }));
    }
    case 'STP_040': {
      if (!n.entity_id || !value) return null;
      const clean = value.replace(/\b(stop|station)\b/gi, '').replace(/\s{2,}/g, ' ').replace(/^[\s\-–,]+|[\s\-–,]+$/g, '');
      return join(t('producer.card.STP_040.what_detail', { name: value }), clean && clean !== value ? t('producer.card.stp_suggest', { value: clean }) : '', use);
    }
    case 'STP_041': {
      if (!n.entity_id || !d.parent_station || !value) return null;
      const parent = stop(d.parent_station);
      return join(t('producer.card.STP_041.what_detail', { name: value, parent }), t('producer.card.stp_suggest', { value: `${parent} ${value}` }), use);
    }
    case 'STP_042': {
      const count = d.affected_stops || n.observed_value;
      if (!count) return null;
      const url = n.entity_id ? d[`example_${listOf(d.example_stop_ids).indexOf(n.entity_id)}_url`] : undefined;
      return join(t('producer.card.STP_042.what_detail', { n: localizedNumber(count) }),
        n.entity_id && url ? t('producer.card.STP_042.what_url', { stop: stop(n.entity_id), url }) : '');
    }
    case 'STP_043': {
      if (!n.entity_id) return null;
      const reason = d.location_type && d.location_type !== '0'
        ? t('producer.card.STP_043.reason_type', { type: locationTypeName(d.location_type) })
        : t('producer.card.STP_043.reason_parent');
      return t('producer.card.STP_043.what_detail', { stop: stop(n.entity_id), value, reason });
    }
    case 'STP_044': {
      if (!n.entity_id || !value) return null;
      const clean = value.replace(/\b(platform|plattform|track|gleis|bahnsteig|voie|quai|spor|anden|andén|plataforma)\b/gi, '').replace(/\s{2,}/g, ' ').trim();
      return join(t('producer.card.STP_044.what_detail', { stop: stop(n.entity_id), value }), clean && clean !== value ? t('producer.card.stp_suggest', { value: clean }) : '', use);
    }
    case 'STP_045': {
      const count = d.affected_stops || n.observed_value;
      if (!count) return null;
      let i = 0;
      while (d[`example_${i}_stop_id`] && d[`example_${i}_stop_id`] !== n.entity_id) i++;
      const station = n.entity_id ? d[`example_${i}_nearest_station_id`] : undefined;
      return join(t('producer.card.STP_045.what_detail', { n: localizedNumber(count) }),
        n.entity_id ? t('producer.card.stp_example', { stop: stop(n.entity_id) }) : '',
        station ? t('producer.card.STP_009.what_near', { station: stop(station), dist: metersText(d[`example_${i}_nearest_station_m`]) }) : '');
    }
    case 'STP_032': {
      const count = d.affected_platforms || n.observed_value;
      if (!count) return null;
      let i = 0;
      while (d[`example_${i}_stop_id`] && d[`example_${i}_stop_id`] !== n.entity_id) i++;
      const station = n.entity_id ? d[`example_${i}_nearest_station_id`] : undefined;
      return join(t('producer.card.STP_032.what_detail', { n: localizedNumber(count) }),
        n.entity_id ? t('producer.card.stp_example', { stop: stop(n.entity_id) }) : '',
        station ? t('producer.card.STP_009.what_near', { station: stop(station), dist: metersText(d[`example_${i}_nearest_station_m`]) }) : '');
    }
    case 'STP_010':
      if (!n.entity_id || !d.parent_station) return null;
      return join(t('producer.card.STP_010.what_detail', { stop: stop(n.entity_id), parent: stop(d.parent_station), type: locationTypeName(n.observed_value) }), use);
    default:
      return null;
  }
}

/** STP tables: the two rows of a repeated code; the example stops of a summary with their trips. */
function stpTable(n: Notice, result: ValidationResult): string {
  const ni = result.name_index;
  const d = n.details ?? {};
  const stop = (id?: string | null) => (id ? ni.stops?.[id] || id : '—');
  const pos = (lat?: string, lon?: string) => (lat && lon ? `${lat}, ${lon}` : '—');
  const table = (caption: string, head: string[], rows: Array<{ cells: string[]; bad?: boolean }>) =>
    `<p class="producer-card-label">${escHtml(caption)}</p><table class="planner-times planner-stop-times"><thead><tr>${head.map(h => `<th>${escHtml(h)}</th>`).join('')}</tr></thead><tbody>${
      rows.map(r => `<tr${r.bad ? ' class="bad"' : ''}>${r.cells.map(c => `<td>${escHtml(c)}</td>`).join('')}</tr>`).join('')}</tbody></table>`;
  if (n.rule_id === 'STP_001' && d.first_line && n.line != null) {
    return table(t('producer.times.stp001_rows'), [t('producer.col.row'), t('producer.times.stp_name'), t('producer.times.stp_position')], [
      { cells: [d.first_line, d.first_name || '—', pos(d.first_lat, d.first_lon)], bad: true },
      { cells: [String(n.line), d.name || '—', pos(d.lat, d.lon)], bad: true },
    ]);
  }
  if ((n.rule_id === 'STP_034' || n.rule_id === 'STP_035') && d.representative_stop_ids) {
    const more = Number(d.additional_stop_count || '0');
    return table(t('producer.times.stp_url_stops', { url: n.observed_value || '—' }), [t('producer.col.stop'), t('producer.col.stop_code')],
      [...listOf(d.representative_stop_ids).map(id => ({ cells: [ni.stops?.[id] || '—', id], bad: true })),
        ...(more > 0 ? [{ cells: [t('producer.times.more_stops', { n: localizedNumber(String(more)) }), ''] }] : [])]);
  }
  if ((n.rule_id === 'STP_037' || n.rule_id === 'STP_038') && d.example_stops) {
    const calls = listOf(d.example_stop_calls);
    return table(t('producer.times.stp037_busiest'), [t('producer.col.stop'), t('producer.times.stp_calls')],
      listOf(d.example_stops).map((id, i) => ({ cells: [stop(id), localizedNumber(calls[i] || '0')], bad: id === n.entity_id })));
  }
  if (n.rule_id === 'STP_042' && d.example_stop_ids) {
    return table(t('producer.times.stp042_urls'), [t('producer.col.stop'), t('producer.times.stp_url')],
      listOf(d.example_stop_ids).map((id, i) => ({ cells: [stop(id), d[`example_${i}_url`] || '—'], bad: id === n.entity_id })));
  }
  if ((n.rule_id === 'STP_016' || n.rule_id === 'STP_017' || n.rule_id === 'STP_020' || n.rule_id === 'STP_039') && n.entity_id) {
    const otherId = n.rule_id === 'STP_020' ? d.nearest_used_stop_id : d.other_stop_id;
    if (!otherId) return '';
    const a = stpContext(n);
    const b = stpContext(n, n.rule_id === 'STP_020' ? 'nearest' : 'other');
    // STP_039 carries no distance: measured between the two stops' positions.
    const p = ni.stop_coords?.[n.entity_id];
    const q = ni.stop_coords?.[otherId];
    const dist = n.rule_id === 'STP_039' ? (p && q ? metersText(String(metersBetween(p, q))) : '—')
      : metersText(n.rule_id === 'STP_020' ? d.nearest_used_m : d.distance_m);
    return table(t(n.rule_id === 'STP_020' ? 'producer.times.stp020_pair' : 'producer.times.stp_pair'),
      [t('producer.col.stop'), t('producer.times.vat_distance'), t('producer.col.route'), t('producer.times.trip_count')], [
        { cells: [stop(n.entity_id), '—', routeLabels(a.routes, result), localizedNumber(a.trips || '0')], bad: true },
        { cells: [stop(otherId), dist, routeLabels(b.routes, result), localizedNumber(b.trips || '0')], bad: n.rule_id !== 'STP_020' },
      ]);
  }
  if (n.rule_id === 'STP_029' && d.parent_id && n.entity_id) {
    const rows = [{ cells: [stop(d.parent_id), t('producer.times.stp_parent'), metersText(/[\d.]+/.exec(n.observed_value ?? '')?.[0])], bad: true }];
    if (d.nearest_station_id) rows.push({ cells: [stop(d.nearest_station_id), t('producer.times.stp_nearer'), metersText(d.nearest_station_m)], bad: false });
    return table(t('producer.times.stp029_stations'), [t('producer.times.stp_station'), t('producer.times.stp_role'), t('producer.times.vat_distance')], rows);
  }
  if (n.rule_id === 'STP_030' && d.nearby_stops) {
    const dists = listOf(d.nearby_m);
    return table(t('producer.times.stp030_candidates'), [t('producer.col.stop'), t('producer.times.vat_distance')],
      listOf(d.nearby_stops).map((id, i) => ({ cells: [stop(id), metersText(dists[i])] })));
  }
  if ((n.rule_id === 'STP_032' || n.rule_id === 'STP_045') && d.example_0_stop_id) {
    const rows = [];
    for (let i = 0; d[`example_${i}_stop_id`]; i++) {
      const at = (key: string) => d[`example_${i}_${key}`];
      rows.push({ cells: [stop(at('stop_id')), at('nearest_station_id') ? `${stop(at('nearest_station_id'))} · ${metersText(at('nearest_station_m'))}` : '—'], bad: at('stop_id') === n.entity_id });
    }
    return table(t('producer.times.stp032_platforms'), [t('producer.times.stp_platform'), t('producer.times.stp_nearest_station')], rows);
  }
  if (n.rule_id === 'STP_012' && d.platforms) {
    return table(t('producer.times.stp012_platforms'), [t('producer.col.stop'), t('producer.col.stop_code')],
      listOf(d.platforms).map(id => ({ cells: [ni.stops?.[id] || '—', id] })));
  }
  if (n.rule_id === 'STP_015' && d.known_levels) {
    return table(t('producer.times.stp015_levels'), [t('producer.times.stp_level_code'), t('producer.times.stp_name')],
      d.known_levels.split('|').filter(Boolean).map(v => { const [code, name] = v.split('='); return { cells: [code, name || '—'] }; }));
  }
  if ((n.rule_id === 'STP_004' || n.rule_id === 'STP_005') && d.example_0_stop_id) {
    const rows = [];
    for (let i = 0; d[`example_${i}_stop_id`]; i++) {
      const at = (key: string) => d[`example_${i}_${key}`];
      rows.push({
        cells: [stop(at('stop_id')), routeLabels(listOf(at('routes')), result),
          at('prev_stop_id') || at('next_stop_id') ? `${stop(at('prev_stop_id'))} → ${stop(at('next_stop_id'))}` : '—'],
        bad: at('stop_id') === n.entity_id,
      });
    }
    return table(t('producer.times.stp_examples'), [t('producer.col.stop'), t('producer.col.route'), t('producer.times.stp_between')], rows);
  }
  return '';
}

/** WCAG contrast ratio of two 6-digit hex colours. */
function contrastRatio(a: string, b: string): number {
  const lum = (hex: string) => {
    const c = [0, 2, 4].map(i => parseInt(hex.slice(i, i + 2), 16) / 255).map(v => (v <= 0.03928 ? v / 12.92 : ((v + 0.055) / 1.055) ** 2.4));
    return 0.2126 * c[0] + 0.7152 * c[1] + 0.0722 * c[2];
  };
  const [x, y] = [lum(a), lum(b)].sort((p, q) => q - p);
  return (x + 0.05) / (y + 0.05);
}

/** The readable text colour on a route colour: black or white, whichever contrasts more. */
function readableTextColor(bg: string): string {
  return contrastRatio(bg, '000000') >= contrastRatio(bg, 'FFFFFF') ? '000000' : 'FFFFFF';
}

/** Each RTS rule's own sentence: the route by its number, what is wrong, how many trips it has. */
function rtsRuleText(n: Notice, result: ValidationResult): string | null {
  const ni = result.name_index;
  const d = n.details ?? {};
  // RTS_031: a row without a code, named by what it carries.
  if (n.rule_id === 'RTS_031') {
    if (n.line == null) return null;
    const ctx = [d.route_short_name, d.route_long_name].filter(Boolean).join(' · ');
    return t('producer.card.RTS_031.what_detail', { line: String(n.line), ctx: ctx ? ` („${ctx}“)` : '' });
  }
  const routeId = routeIdOfNotice(n);
  if (!routeId) return null;
  const route = routeBadge(routeId, ni).label;
  const value = (n.observed_value ?? '').trim();
  const join = (...parts: string[]) => parts.filter(Boolean).join(' ');
  const use = d.trip_count ? t('producer.card.rts_use', { n: localizedNumber(d.trip_count) }) : '';
  const label = (short?: string, long?: string) => [short, long].filter(Boolean).join(' · ') || '—';
  switch (n.rule_id) {
    case 'RTS_001':
      if (!d.first_line || n.line == null) return null;
      return join(t('producer.card.RTS_001.what_detail', {
        code: routeId, first_line: d.first_line, line: String(n.line), first: label(d.first_short_name, d.first_long_name), second: label(d.short_name, d.long_name),
      }), use);
    case 'RTS_002': {
      if (!value) return null;
      const known = (d.known_agencies ?? '').split('|').filter(Boolean).map(v => { const i = v.indexOf('='); return `${v.slice(i + 1)} (${v.slice(0, i)})`; });
      return join(t('producer.card.RTS_002.what_detail', { route, agency: value }), known.length ? t('producer.card.RTS_002.what_known', { agencies: known.join(', ') }) : '', use);
    }
    case 'RTS_003': {
      const trip = dqIndexKey(ni.trip_stops, d.example_trip_id);
      const stops = trip ? ni.trip_stops?.[trip] ?? [] : [];
      const name = (id?: string) => (id ? ni.stops?.[id] || id : '');
      const from = name(stops[0]);
      const to = name(stops[stops.length - 1]);
      return join(t('producer.card.RTS_003.what_detail', { code: routeId }), use, from && to && from !== to ? t('producer.card.RTS_003.what_suggest', { from, to }) : '');
    }
    case 'RTS_004': {
      const types = [0, 1, 2, 3, 4, 5, 6, 7, 11, 12].map(c => routeTypeName(String(c))).join(', ');
      return join(t(value ? 'producer.card.RTS_004.what_detail' : 'producer.card.RTS_004.what_empty', { route, value, types }), use);
    }
    case 'RTS_005':
    case 'RTS_006':
    case 'RTS_007':
      return value ? join(t(`producer.card.${n.rule_id}.what_detail`, { route, value }), use) : null;
    case 'RTS_008': {
      if (!d.color || !d.text_color || !value) return null;
      const better = readableTextColor(d.color);
      return join(t('producer.card.RTS_008.what_detail', { route, color: d.color, text: d.text_color, ratio: localizedNumber(Number(value).toFixed(1)) }),
        t('producer.card.RTS_008.what_suggest', { text: better, ratio: localizedNumber(contrastRatio(d.color, better).toFixed(1)) }), use);
    }
    case 'RTS_012':
      return join(t('producer.card.RTS_012.what_detail', { route }),
        d.same_name_routes ? t('producer.card.RTS_012.what_same', { codes: listOf(d.same_name_routes).join(', ') }) : '');
    case 'RTS_013':
    case 'RTS_018':
      return join(t(`producer.card.${n.rule_id}.what_detail`, { route, value: value || '—' }), use);
    case 'RTS_016':
      return t('producer.card.RTS_016.what_detail', { route, n: localizedNumber(d.trip_count || '0') });
    case 'RTS_017':
      return t('producer.card.RTS_017.what_detail', { route, n: localizedNumber(d.trip_count || '0') });
    case 'RTS_019':
      return d.conflicting_routes && value
        ? t('producer.card.RTS_019.what_detail', { name: value, n: localizedNumber(String(listOf(d.conflicting_routes).length)) })
        : null;
    case 'RTS_020':
      return value ? join(t('producer.card.RTS_020.what_detail', { route, value }), use) : null;
    case 'RTS_021':
      return d.name ? join(t('producer.card.RTS_021.what_detail', { route, n: localizedNumber(value || String([...d.name].length)) }), use) : null;
    case 'RTS_022': {
      if (!value) return null;
      const esc = route.replace(/[.*+?^${}()|[\]\\]/g, '\\$&');
      const clean = value.replace(new RegExp(esc, 'i'), '').replace(/\s{2,}/g, ' ').replace(/^[\s\-–:,]+|[\s\-–:,]+$/g, '');
      return join(t('producer.card.RTS_022.what_detail', { route, long: value }), clean && clean !== value ? t('producer.card.stp_suggest', { value: clean }) : '', use);
    }
    case 'RTS_023':
      return value
        ? join(t((d.matched_field ?? '').includes('route_long_name') ? 'producer.card.RTS_023.what_long' : 'producer.card.RTS_023.what_short', { route, value }), use)
        : null;
    case 'RTS_024':
    case 'RTS_029':
      return join(t(`producer.card.${n.rule_id}.what_detail`, { route, value: value || '—' }), use);
    case 'RTS_025':
      return join(t(d.agency_name ? 'producer.card.RTS_025.what_detail' : 'producer.card.RTS_025.what_plain', { route, agency: d.agency_name ?? '' }), use);
    case 'RTS_026':
    case 'RTS_027':
      return d.conflicting_routes && value
        ? t(`producer.card.${n.rule_id}.what_detail`, { name: value, n: localizedNumber(String(listOf(d.conflicting_routes).length)) })
        : null;
    case 'RTS_028':
      return d.flex_trip_id && value
        ? join(t(n.field === 'continuous_drop_off' ? 'producer.card.RTS_028.what_dropoff' : 'producer.card.RTS_028.what_pickup', { route, trip: tripShort(d.flex_trip_id, result, false), value }), use)
        : null;
    case 'RTS_030':
      return value
        ? join(t('producer.card.RTS_030.what_detail', { route, value, name: routeTypeName(value), base: routeTypeName(String(routeBaseType(value))) }), use)
        : null;
    case 'RTS_010':
    case 'RTS_011':
      return d.name ? join(t(`producer.card.${n.rule_id}.what_detail`, { route, name: d.name, n: localizedNumber(value || String([...d.name].length)) }), use) : null;
    default:
      return null;
  }
}

/** RTS tables: the two rows of a repeated code, the defined agencies, the badge preview. */
function rtsTable(n: Notice, result: ValidationResult): string {
  const d = n.details ?? {};
  const table = (caption: string, head: string[], rows: Array<{ cells: string[]; bad?: boolean }>) =>
    `<p class="producer-card-label">${escHtml(caption)}</p><table class="planner-times planner-stop-times"><thead><tr>${head.map(h => `<th>${escHtml(h)}</th>`).join('')}</tr></thead><tbody>${
      rows.map(r => `<tr${r.bad ? ' class="bad"' : ''}>${r.cells.map(c => `<td>${escHtml(c)}</td>`).join('')}</tr>`).join('')}</tbody></table>`;
  if (n.rule_id === 'RTS_001' && d.first_line && n.line != null) {
    const type = (v?: string) => (v ? routeTypeName(v) : '—');
    return table(t('producer.times.rts001_rows'), [t('producer.col.row'), t('producer.times.rts_short'), t('producer.col.route_name'), t('producer.times.rts_type')], [
      { cells: [d.first_line, d.first_short_name || '—', d.first_long_name || '—', type(d.first_type)], bad: true },
      { cells: [String(n.line), d.short_name || '—', d.long_name || '—', type(d.type)], bad: true },
    ]);
  }
  if (n.rule_id === 'RTS_016' && d.services) {
    return table(t('producer.times.rts016_services'), [t('producer.col.service_code'), t('producer.col.service')],
      listOf(d.services).map(id => ({ cells: [id, serviceText(id, result)], bad: true })));
  }
  if (['RTS_019', 'RTS_026', 'RTS_027'].includes(n.rule_id) && d.route_0_route_id) {
    const ni = result.name_index;
    const rows = [];
    for (let i = 0; d[`route_${i}_route_id`]; i++) {
      const id = d[`route_${i}_route_id`]!;
      rows.push({ cells: [id, ni.routes?.[id] || '—', ni.route_long_names?.[id] || '—', localizedNumber(d[`route_${i}_trips`] || '0'), tripShort(d[`route_${i}_example_trip_id`], result, false)], bad: true });
    }
    return table(t(n.rule_id === 'RTS_026' ? 'producer.times.rts026_routes' : 'producer.times.rts019_routes'), [t('producer.times.rts_record_code'), t('producer.times.rts_short'), t('producer.col.route_name'), t('producer.times.trip_count'), t('producer.times.example_trip')], rows);
  }
  if (n.rule_id === 'RTS_002' && d.known_agencies) {
    return table(t('producer.times.rts002_agencies'), [t('producer.times.rts_agency_code'), t('producer.times.stp_name')],
      d.known_agencies.split('|').filter(Boolean).map(v => { const i = v.indexOf('='); return { cells: [v.slice(0, i) || '—', v.slice(i + 1)] }; }));
  }
  if (n.rule_id === 'RTS_008' && d.color && d.text_color && /^[0-9a-f]{6}$/i.test(d.color) && /^[0-9a-f]{6}$/i.test(d.text_color)) {
    const badge = (fg: string, caption: string) =>
      `<span class="planner-route-badge" style="background:#${d.color};border-color:#${d.color};color:#${fg}">${escHtml(caption)}</span>`;
    const better = readableTextColor(d.color);
    return `<p class="producer-card-label">${escHtml(t('producer.times.rts008_preview'))}</p><p>${badge(d.text_color, t('producer.times.rts_now'))} → ${badge(better, t('producer.times.rts_suggested'))}</p>`;
  }
  return '';
}

/** SHP findings: the shape a row is about and its context; a summary row reads its own
 *  example's keys (`example_<i>_*`), SHP_001 the shape of the rows around it (`shape_*`). */
function shpContext(n: Notice): { shape?: string; at: (key: string) => string | undefined; routes: string[] } {
  const d = n.details ?? {};
  let prefix = '';
  // A trip or stop finding about a shape (SHP_017/022) names it in its details.
  let shape = (n.entity_type === 'Shape' ? n.entity_id : d.shape_id || n.entity_id) ?? undefined;
  if (d.example_0_shape_id) {
    let i = 0;
    while (d[`example_${i}_shape_id`] && d[`example_${i}_shape_id`] !== n.entity_id) i++;
    if (!d[`example_${i}_shape_id`]) i = 0;
    prefix = `example_${i}_`;
    shape = d[`example_${i}_shape_id`];
  } else if (n.rule_id === 'SHP_001') {
    prefix = 'shape_';
    shape = d.prev_shape_id || d.next_shape_id;
  }
  const at = (key: string) => d[`${prefix}${key}`] || undefined;
  return { shape, at, routes: listOf(at('routes')) };
}

/** "„SH1“ güzergâh çizimi (S8 hattı)". */
function shapeText(shape: string | undefined, routes: string[], result: ValidationResult): string {
  if (!shape) return '—';
  const ids = routes.length ? routes : [...new Set((result.name_index.shape_routes?.[shape] ?? []).map(([r]) => r))];
  return ids.length ? t('producer.card.shp_shape_routes', { shape, routes: routeLabels(ids, result) }) : t('producer.card.shp_shape', { shape });
}

/** Each SHP rule's own sentence: the shape by its routes, the point by its row, where it goes wrong. */
function shpRuleText(n: Notice, result: ValidationResult): string | null {
  const d = n.details ?? {};
  const c = shpContext(n);
  const shape = shapeText(c.shape, c.routes, result);
  const value = (n.observed_value ?? '').trim();
  const join = (...parts: string[]) => parts.filter(Boolean).join(' ');
  const between = d.prev_line && d.next_line ? t('producer.card.shp_between', { prev: d.prev_line, next: d.next_line }) : '';
  const meters = (a?: string, b?: string, x?: string, y?: string) => {
    const p = [Number(a), Number(b)] as [number, number];
    const q = [Number(x), Number(y)] as [number, number];
    return [...p, ...q].every(Number.isFinite) && a && x ? metersText(String(metersBetween(p, q))) : '';
  };
  switch (n.rule_id) {
    case 'SHP_001':
      if (n.line == null) return null;
      return join(t('producer.card.SHP_001.what_detail', { line: String(n.line) }),
        c.shape ? t('producer.card.SHP_001.what_near', { shape }) : '');
    case 'SHP_002':
    case 'SHP_003':
      if (n.line == null) return null;
      return join(t(`producer.card.${n.rule_id}.what_detail`, { shape, line: String(n.line), value: value || '—' }), between);
    case 'SHP_004':
      return n.line == null ? null : t('producer.card.SHP_004.what_detail', { shape, line: String(n.line), value: value || '—' });
    case 'SHP_005': {
      const count = d.affected_shapes || n.observed_value;
      if (!count) return null;
      const ex = c.at('dist') && c.at('prev_dist')
        ? t('producer.card.SHP_005.what_example', { shape, seq: c.at('shape_pt_sequence') || '—', prev: localizedNumber(c.at('prev_dist')!), dist: localizedNumber(c.at('dist')!) })
        : '';
      return join(t('producer.card.SHP_005.what_detail', { n: localizedNumber(count) }), ex);
    }
    case 'SHP_006':
      return t('producer.card.SHP_006.what_detail', { shape });
    case 'SHP_008': {
      if (n.line == null) return null;
      const dist = meters(d.first_lat, d.first_lon, d.lat, d.lon);
      return t('producer.card.SHP_008.what_detail', {
        shape, seq: value || '—', first: d.first_line || '—', line: String(n.line), dist: dist ? t('producer.card.shp_apart', { dist }) : '',
      });
    }
    case 'SHP_009':
      if (!n.entity_id && Number(d.affected_shapes) > 1) return t('producer.card.SHP_009.what_many', { n: localizedNumber(d.affected_shapes!) });
      return join(t('producer.card.SHP_009.what_detail', { shape }),
        d.repeated_shapes ? t('producer.card.SHP_009.what_repeat', { n: localizedNumber(d.repeated_shapes) }) : '');
    case 'SHP_010':
      if (Number(d.affected_shapes) > 1) {
        return join(t('producer.card.SHP_010.what_many', { n: localizedNumber(d.affected_shapes!) }), n.entity_id ? t('producer.card.shp_example', { shape }) : '');
      }
      return t('producer.card.SHP_010.what_detail', { shape, prev: d.prev_line || '—', line: String(n.line ?? '—') });
    case 'SHP_014': {
      if (!d.problem_stop) return null;
      const dist = /[\d.]+/.exec(value)?.[0];
      return t(d.endpoint === 'end' ? 'producer.card.SHP_014.what_end' : 'producer.card.SHP_014.what_start', {
        shape, stop: result.name_index.stops?.[d.problem_stop] || d.problem_stop, dist: metersText(dist),
      });
    }
    case 'SHP_015':
      return d.point_count && d.length_km
        ? t('producer.card.SHP_015.what_detail', { shape, km: localizedNumber(d.length_km), n: localizedNumber(d.point_count) })
        : null;
    case 'SHP_016':
      return d.first_stop ? t('producer.card.SHP_016.what_detail', { shape, stop: result.name_index.stops?.[d.first_stop] || d.first_stop }) : null;
    case 'SHP_017':
      return n.entity_id && d.bad_stop
        ? join(t('producer.card.SHP_017.what_detail', { trip: tripPhrase(n.entity_id, result), stop: result.name_index.stops?.[d.bad_stop] || d.bad_stop }),
          c.shape ? t('producer.card.SHP_017.what_shape', { shape }) : '')
        : null;
    case 'SHP_018': {
      const coords = c.shape ? result.name_index.shape_coords?.[c.shape] ?? [] : [];
      let m = 0;
      for (let i = 1; i < coords.length; i++) m += metersBetween(coords[i - 1], coords[i]);
      return join(t('producer.card.SHP_018.what_detail', { shape }),
        coords.length > 1 ? t('producer.card.SHP_018.what_size', { n: localizedNumber(String(coords.length)), dist: metersText(String(m)) }) : '');
    }
    case 'SHP_019':
      return t('producer.card.SHP_019.what_detail', { shape, n: localizedNumber(value || String(listOf(d.trip_ids).length)) });
    case 'SHP_020':
      if (!n.entity_id && Number(d.affected_shapes) > 1) return t('producer.card.SHP_020.what_many', { n: localizedNumber(d.affected_shapes!) });
      return join(d.first_point && d.second_point
        ? t('producer.card.SHP_020.what_detail', { shape, a: d.first_point, b: d.second_point })
        : t('producer.card.SHP_020.what_plain', { shape }),
      d.repeated_shapes ? t('producer.card.SHP_020.what_repeat', { n: localizedNumber(d.repeated_shapes) }) : '');
    case 'SHP_021':
      return n.line == null ? null : t('producer.card.SHP_021.what_detail', { shape, line: String(n.line), value: value || '—' });
    case 'SHP_022':
      return n.entity_id && c.shape
        ? t('producer.card.SHP_022.what_detail', { stop: result.name_index.stops?.[n.entity_id] || n.entity_id, shape, n: localizedNumber(d.parts || '2') })
        : null;
    case 'SHP_023':
      return t('producer.card.SHP_023.what_detail', { shape, dist: d.dist ? localizedNumber(d.dist) : '—' });
    case 'SHP_024':
      return n.entity_id && d.shape_dist_traveled && d.distance_m
        ? t('producer.card.SHP_024.what_detail', {
          stop: result.name_index.stops?.[n.entity_id] || n.entity_id, sdt: localizedNumber(d.shape_dist_traveled), dist: metersText(d.distance_m), shape,
        })
        : null;
    case 'SHP_025': {
      if (!n.entity_id || !d.trip_max || !d.shape_max) return null;
      const ratio = Number(d.trip_max) / Math.max(1e-9, Number(d.shape_max));
      return join(t('producer.card.SHP_025.what_detail', {
        trip: tripPhrase(n.entity_id, result, 'gen'), max: localizedNumber(d.trip_max), shape, total: localizedNumber(d.shape_max),
      }), ratio > 500 && ratio < 2000 ? t('producer.card.SHP_025.what_unit') : '');
    }
    case 'SHP_026':
      return d.point_count ? t('producer.card.SHP_026.what_detail', { shape, n: localizedNumber(d.point_count), limit: localizedNumber(d.limit || '5000') }) : null;
    case 'SHP_028':
    case 'SHP_029':
      return d.dist && d.apart_m
        // Under 10 m the decimal is the point (SHP_029 measures differences below 1.1 m).
        ? t(`producer.card.${n.rule_id}.what_detail`, {
          shape, dist: localizedNumber(d.dist),
          apart: Number(d.apart_m) < 10 ? `${localizedNumber(Number(d.apart_m).toFixed(1))} m` : metersText(d.apart_m),
        })
        : null;
    case 'SHP_030':
      return d.shape_point_count && d.shape_points_missing_dist
        ? t('producer.card.SHP_030.what_detail', {
          shape, k: localizedNumber(d.affected_trip_count || '1'), total: localizedNumber(d.shape_point_count), missing: localizedNumber(d.shape_points_missing_dist),
        })
        : null;
    case 'SHP_012': {
      const far = listOf(d.far_stops);
      if (!far.length) return null;
      const ms = listOf(d.far_stop_m).map(Number);
      let worst = 0;
      ms.forEach((m, i) => { if (m > (ms[worst] ?? 0)) worst = i; });
      const limit = /\d+m/.exec(value)?.[0]?.replace('m', '');
      return join(t('producer.card.SHP_012.what_detail', { shape, n: localizedNumber(/^\d+/.exec(value)?.[0] || String(far.length)), limit: metersText(limit) }),
        ms.length ? t('producer.card.SHP_012.what_worst', { stop: result.name_index.stops?.[far[worst]] || far[worst], dist: metersText(String(ms[worst])) }) : '');
    }
    default:
      return null;
  }
}

/** SHP tables: the far stops with their distance, the summary's shapes, the repeated order's rows. */
function shpTable(n: Notice, result: ValidationResult): string {
  const ni = result.name_index;
  const d = n.details ?? {};
  const table = (caption: string, head: string[], rows: Array<{ cells: string[]; bad?: boolean }>) =>
    `<p class="producer-card-label">${escHtml(caption)}</p><table class="planner-times planner-stop-times"><thead><tr>${head.map(h => `<th>${escHtml(h)}</th>`).join('')}</tr></thead><tbody>${
      rows.map(r => `<tr${r.bad ? ' class="bad"' : ''}>${r.cells.map(c => `<td>${escHtml(c)}</td>`).join('')}</tr>`).join('')}</tbody></table>`;
  if (n.rule_id === 'SHP_012' && d.far_stops) {
    const ms = listOf(d.far_stop_m);
    return table(t('producer.times.shp012_stops'), [t('producer.col.stop'), t('producer.times.shp_distance')],
      listOf(d.far_stops).map((id, i) => ({ cells: [ni.stops?.[id] ? `${ni.stops[id]} (${id})` : id, metersText(ms[i])], bad: true })));
  }
  if (n.rule_id === 'SHP_005' && d.example_0_shape_id) {
    const rows = [];
    for (let i = 0; d[`example_${i}_shape_id`]; i++) {
      const at = (key: string) => d[`example_${i}_${key}`];
      rows.push({
        cells: [at('shape_id')!, routeLabels(listOf(at('routes')), result), at('shape_pt_sequence') || '—',
          at('prev_dist') && at('dist') ? `${localizedNumber(at('prev_dist')!)} → ${localizedNumber(at('dist')!)}` : '—'],
        bad: at('shape_id') === n.entity_id,
      });
    }
    return table(t('producer.times.shp005_shapes'), [t('producer.col.shape_code'), t('producer.col.route'), t('producer.times.shp_seq'), t('producer.times.shp_dists')], rows);
  }
  if (n.rule_id === 'SHP_030' && d.representative_trip_ids) {
    return table(t('producer.times.shp030_trips'), [t('producer.col.trip'), t('producer.col.trip_code')],
      listOf(d.representative_trip_ids).map(id => ({ cells: [tripShort(id, result), id] })));
  }
  if (n.rule_id === 'SHP_019' && d.trip_ids) {
    return table(t('producer.times.shp019_trips'), [t('producer.col.trip'), t('producer.col.trip_code')],
      listOf(d.trip_ids).map(id => ({ cells: [tripShort(id, result), id], bad: true })));
  }
  if (n.rule_id === 'SHP_008' && d.first_line && n.line != null) {
    const pos = (a?: string, b?: string) => (a && b ? `${a}, ${b}` : '—');
    return table(t('producer.times.shp008_rows'), [t('producer.col.row'), t('producer.times.stp_position')], [
      { cells: [d.first_line, pos(d.first_lat, d.first_lon)], bad: true },
      { cells: [String(n.line), pos(d.lat, d.lon)], bad: true },
    ]);
  }
  return '';
}

/** GEO findings about one stop: the stop on an example trip (its routes and trips). */
const GEO_STOP_RULES = new Set(['GEO_002', 'GEO_009', 'GEO_012', 'GEO_015', 'GEO_016', 'GEO_019', 'GEO_022']);
/** GEO findings about one drawing: the line with its routes, an example trip and the faulty spot. */
const GEO_SHAPE_RULES = new Set(['GEO_006', 'GEO_007', 'GEO_017', 'GEO_020']);
/** GEO findings with planner text, tables and maps. */
const GEO_CONTEXT_RULES = new Set([...GEO_STOP_RULES, ...GEO_SHAPE_RULES, 'GEO_013', 'GEO_014', 'GEO_018', 'GEO_021']);
/** GEO stop findings whose stop belongs between its neighbours on the trip (no usable position). */
const GEO_PLACE_RULES = new Set(['GEO_016', 'GEO_019', 'GEO_022']);

/** A distance in km as planners read it: "7.919 km", "20,0 km", "0,2 km". */
function kmText(value?: string | number): string {
  const v = Number(value);
  if (value === undefined || value === '' || !Number.isFinite(v)) return '—';
  return `${localizedNumber(String(v >= 100 ? Math.round(v) : v))} km`;
}

/** A likely typing slip behind a stop placed far away: latitude and longitude swapped, or one
 *  sign flipped — the first correction that brings the stop where it belongs (`fits`). */
function geoSlip(p: [number, number] | undefined, fits: (q: [number, number]) => boolean): { kind: 'swap' | 'sign_lat' | 'sign_lon'; at: [number, number] } | null {
  if (!p || fits(p)) return null;
  const options: Array<['swap' | 'sign_lat' | 'sign_lon', [number, number]]> = [
    ['swap', [p[1], p[0]]], ['sign_lat', [-p[0], p[1]]], ['sign_lon', [p[0], -p[1]]],
  ];
  const hit = options.find(([, q]) => Math.abs(q[0]) <= 90 && Math.abs(q[1]) <= 180 && fits(q));
  return hit ? { kind: hit[0], at: hit[1] } : null;
}

/** GEO_002/015: the correction that brings the stop back — near the network's middle (within
 *  200 km), or inside Japan's bounds. */
function geoStopSlip(n: Notice, result: ValidationResult): ReturnType<typeof geoSlip> {
  const d = n.details ?? {};
  const p = n.entity_id ? result.name_index.stop_coords?.[n.entity_id] : undefined;
  if (n.rule_id === 'GEO_002' && d.med_lat && d.med_lon) {
    const mid: [number, number] = [Number(d.med_lat), Number(d.med_lon)];
    return geoSlip(p, q => metersBetween(q, mid) <= 200_000);
  }
  if (n.rule_id === 'GEO_015') return geoSlip(p, q => q[0] >= 20.25 && q[0] <= 45.33 && q[1] >= 122.56 && q[1] <= 153.59);
  return null;
}

/** The point on a drawn line closest to a stop (for the stop–line link on the map). */
function nearestOnLine(line: [number, number][], p: [number, number]): [number, number] | null {
  let best: [number, number] | null = null;
  let bestD = Infinity;
  for (let i = 1; i < line.length; i++) {
    const [a, b] = [line[i - 1], line[i]];
    const k = Math.cos(p[0] * Math.PI / 180);
    const ax = a[1] * k, ay = a[0], bx = b[1] * k, by = b[0], px = p[1] * k, py = p[0];
    const len = (bx - ax) ** 2 + (by - ay) ** 2;
    const t = len ? Math.max(0, Math.min(1, ((px - ax) * (bx - ax) + (py - ay) * (by - ay)) / len)) : 0;
    const q: [number, number] = [a[0] + t * (b[0] - a[0]), a[1] + t * (b[1] - a[1])];
    const dist = metersBetween(p, q);
    if (dist < bestD) { bestD = dist; best = q; }
  }
  return best;
}

/** Each GEO rule's own sentence: the stop by name and its use, the drawing by its routes. */
function geoRuleText(n: Notice, result: ValidationResult): string | null {
  const ni = result.name_index;
  const d = n.details ?? {};
  const stop = (id?: string | null) => (id ? ni.stops?.[id] || id : '—');
  const join = (...parts: string[]) => parts.filter(Boolean).join(' ');
  const use = GEO_STOP_RULES.has(n.rule_id) ? stpUseText(n, result) : '';
  const slip = geoStopSlip(n, result);
  const slipText = slip ? t(`producer.card.geo_slip_${slip.kind}`) : '';
  const pos = n.entity_id && ni.stop_coords?.[n.entity_id]
    ? `${coordText(ni.stop_coords[n.entity_id][0])}, ${coordText(ni.stop_coords[n.entity_id][1])}` : (n.observed_value ?? '—');
  switch (n.rule_id) {
    case 'GEO_002':
      if (!d.dist_km) return null;
      return join(t('producer.card.GEO_002.what_detail', { stop: stop(n.entity_id), km: kmText(d.dist_km) }), slipText, use);
    case 'GEO_015':
      return join(t('producer.card.GEO_015.what_detail', { stop: stop(n.entity_id), pos }), slipText, use);
    case 'GEO_016': case 'GEO_019': case 'GEO_022': {
      const c = stpContext(n);
      const place = c.prev && ni.stop_coords?.[c.prev] && c.next && ni.stop_coords?.[c.next] ? t('producer.card.stp_place') : '';
      const p = n.entity_id ? ni.stop_coords?.[n.entity_id] : undefined;
      return join(t(`producer.card.${n.rule_id}.what_detail`, {
        stop: stop(n.entity_id), pos, lat: p ? coordText(p[0]) : (n.observed_value ?? '—'),
      }), use, place);
    }
    case 'GEO_018':
      if (!d.stop_count) return null;
      return t('producer.card.GEO_018.what_detail', { n: localizedNumber(d.stop_count), m: metersText(d.span_m) });
    case 'GEO_021': {
      if (!d.shared_stops) return null;
      const groups: string[] = [];
      for (let i = 0; d[`group_${i}_count`]; i++) {
        groups.push(t('producer.card.GEO_021.group', { n: localizedNumber(d[`group_${i}_count`]!), stops: listOf(d[`group_${i}_stops`]).map(id => `„${stop(id)}“`).join(', ') }));
      }
      const pct = Math.round(Number(d.shared_stops) / Number(d.total_stops) * 100);
      return t('producer.card.GEO_021.what_detail', {
        total: localizedNumber(d.total_stops || '0'), n: localizedNumber(d.shared_stops), pct: localizedNumber(String(pct)), groups: groups.join('; ') || '—',
      });
    }
    case 'GEO_020': {
      if (!d.point_count) return null;
      const c = shpContext(n);
      return t('producer.card.GEO_020.what_detail', {
        shape: shapeText(c.shape, c.routes, result), n: localizedNumber(d.point_count), pos: `${coordText(Number(d.lat))}, ${coordText(Number(d.lon))}`,
      });
    }
    case 'GEO_009': {
      const m = /([\d.]+)\s*m/.exec(n.observed_value ?? '')?.[1];
      const limit = /([\d.]+)/.exec(n.expected_value ?? '')?.[1];
      if (!m || !d.shape_id) return null;
      return join(t('producer.card.GEO_009.what_detail', {
        stop: stop(n.entity_id), shape: shapeText(d.shape_id, [], result), dist: metersText(m), limit: metersText(limit),
      }), use);
    }
    case 'GEO_012': {
      const others: string[] = [];
      for (let i = 0; d[`example_${i}_stop_id`]; i++) others.push(stop(d[`example_${i}_stop_id`]));
      if (!others.length) return null;
      return join(t('producer.card.GEO_012.what_detail', {
        stop: stop(n.entity_id), m: metersText(d.cluster_m), n: localizedNumber(String(others.length)), stops: others.map(x => `„${x}“`).join(', '),
      }), use);
    }
    case 'GEO_013':
      if (!d.ns_km) return null;
      return t('producer.card.GEO_013.what_detail', { n: localizedNumber(n.observed_value ?? '0'), ns: kmText(d.ns_km), ew: kmText(d.ew_km) });
    case 'GEO_014': {
      if (!d.span_km) return null;
      const kms = listOf(d.example_km);
      const far = listOf(d.example_stop_ids).map((id, i) => t('producer.card.GEO_014.far', { stop: stop(id), km: kmText(kms[i]) }));
      return t('producer.card.GEO_014.what_detail', {
        ns: kmText(d.ns_km), ew: kmText(d.ew_km), far: far.join(', ') || '—',
      });
    }
    case 'GEO_006': case 'GEO_007': {
      if (!d.dist_km) return null;
      const c = shpContext(n);
      return t(`producer.card.${n.rule_id}.what_detail`, {
        shape: shapeText(c.shape, c.routes, result), from: d.from_seq || '—', to: d.to_seq || '—', km: kmText(d.dist_km), limit: kmText(d.threshold_km),
      });
    }
    case 'GEO_017': {
      const c = shpContext(n);
      const count = Number(d.null_point_count || '1');
      return join(t('producer.card.GEO_017.what_detail', { shape: shapeText(c.shape, c.routes, result), seq: d.shape_pt_sequence || '—' }),
        count > 1 ? t('producer.card.GEO_017.more', { n: localizedNumber(String(count)) }) : '');
    }
    default:
      return null;
  }
}

/** GEO tables: the stop with its position and use, a cluster's stops, the far stops, the two
 *  points of a long stretch, the 0,0 point between its neighbours. */
function geoTable(n: Notice, result: ValidationResult): string {
  const ni = result.name_index;
  const d = n.details ?? {};
  const table = (caption: string, head: string[], rows: Array<{ cells: string[]; bad?: boolean }>) =>
    `<p class="producer-card-label">${escHtml(caption)}</p><table class="planner-times planner-stop-times"><thead><tr>${head.map(h => `<th>${escHtml(h)}</th>`).join('')}</tr></thead><tbody>${
      rows.map(r => `<tr${r.bad ? ' class="bad"' : ''}>${r.cells.map(c => `<td>${escHtml(c)}</td>`).join('')}</tr>`).join('')}</tbody></table>`;
  const stopLabel = (id?: string) => (id ? (ni.stops?.[id] ? `${ni.stops[id]} (${id})` : id) : '—');
  const posOf = (id?: string) => {
    const p = id ? ni.stop_coords?.[id] : undefined;
    return p ? `${coordText(p[0])}, ${coordText(p[1])}` : '—';
  };
  const pt = (lat?: string, lon?: string) => (lat && lon ? `${coordText(Number(lat))}, ${coordText(Number(lon))}` : '—');
  if (n.rule_id === 'GEO_002' || n.rule_id === 'GEO_015' || GEO_PLACE_RULES.has(n.rule_id)) {
    const c = stpContext(n);
    const pos = n.rule_id === 'GEO_016' ? '0, 0' : posOf(n.entity_id ?? undefined);
    const head = [t('producer.col.stop'), t('producer.times.stp_position'),
      ...(n.rule_id === 'GEO_002' ? [t('producer.times.geo_middle_distance')] : []), t('producer.times.trip_count'), t('producer.col.route')];
    const rows = [{ cells: [stopLabel(n.entity_id ?? undefined), pos, ...(n.rule_id === 'GEO_002' ? [kmText(d.dist_km)] : []),
      localizedNumber(c.trips || '0'), routeLabels(c.routes, result)], bad: true }];
    const slip = geoStopSlip(n, result);
    if (slip) {
      rows.push({ cells: [t('producer.times.geo_slip_position'), `${coordText(slip.at[0])}, ${coordText(slip.at[1])}`,
        ...(n.rule_id === 'GEO_002' && d.med_lat ? [kmText(metersBetween(slip.at, [Number(d.med_lat), Number(d.med_lon)]) / 1000)] : []), '', ''], bad: false });
    }
    return table(t('producer.times.geo_stop'), head, rows);
  }
  if (n.rule_id === 'GEO_009' && d.shape_id) {
    const c = stpContext(n);
    const m = /([\d.]+)\s*m/.exec(n.observed_value ?? '')?.[1];
    return table(t('producer.times.geo_stop'), [t('producer.col.stop'), t('producer.times.geo_shape'), t('producer.times.shp_distance'), t('producer.col.route')],
      [{ cells: [stopLabel(n.entity_id ?? undefined), shapeText(d.shape_id, [], result), metersText(m), routeLabels(c.routes, result)], bad: true }]);
  }
  if (n.rule_id === 'GEO_012' && d.example_0_stop_id) {
    const here = n.entity_id ? ni.stop_coords?.[n.entity_id] : undefined;
    const ids = [n.entity_id ?? '', ...Array.from({ length: 20 }, (_, i) => d[`example_${i}_stop_id`]).filter((v): v is string => Boolean(v))];
    const ctxOf = (i: number) => (i === 0
      ? { trips: d.trip_count, routes: listOf(d.routes) }
      : { trips: d[`example_${i - 1}_trip_count`], routes: listOf(d[`example_${i - 1}_routes`]) });
    return table(t('producer.times.geo_cluster'), [t('producer.col.stop'), t('producer.times.stp_position'), t('producer.times.geo_first_distance'), t('producer.times.trip_count'), t('producer.col.route')],
      ids.map((id, i) => {
        const p = ni.stop_coords?.[id];
        const c = ctxOf(i);
        return { cells: [stopLabel(id), posOf(id), i && here && p ? metersText(String(metersBetween(here, p))) : '—', localizedNumber(c.trips || '0'), routeLabels(c.routes, result)], bad: true };
      }));
  }
  if (n.rule_id === 'GEO_014' && d.example_0_stop_id) {
    const kms = listOf(d.example_km);
    const rows = [];
    for (let i = 0; d[`example_${i}_stop_id`]; i++) {
      const id = d[`example_${i}_stop_id`]!;
      rows.push({ cells: [stopLabel(id), posOf(id), kmText(kms[i]), localizedNumber(d[`example_${i}_trip_count`] || '0'), routeLabels(listOf(d[`example_${i}_routes`]), result)], bad: i === 0 });
    }
    return table(t('producer.times.geo_far'), [t('producer.col.stop'), t('producer.times.stp_position'), t('producer.times.geo_middle_distance'), t('producer.times.trip_count'), t('producer.col.route')], rows);
  }
  if ((n.rule_id === 'GEO_006' || n.rule_id === 'GEO_007') && d.from_lat) {
    return table(t('producer.times.geo_jump'), [t('producer.times.shp_seq'), t('producer.col.row'), t('producer.times.stp_position'), t('producer.times.geo_from_previous')], [
      { cells: [d.from_seq || '—', d.from_line || '—', pt(d.from_lat, d.from_lon), '—'], bad: true },
      { cells: [d.to_seq || '—', d.to_line || '—', pt(d.to_lat, d.to_lon), kmText(d.dist_km)], bad: true },
    ]);
  }
  if (n.rule_id === 'GEO_018' && d.example_0_stop_id) {
    const rows = [];
    for (let i = 0; d[`example_${i}_stop_id`]; i++) {
      const id = d[`example_${i}_stop_id`]!;
      rows.push({ cells: [stopLabel(id), posOf(id), localizedNumber(d[`example_${i}_trip_count`] || '0'), routeLabels(listOf(d[`example_${i}_routes`]), result)] });
    }
    return table(t('producer.times.geo_area_stops'), [t('producer.col.stop'), t('producer.times.stp_position'), t('producer.times.trip_count'), t('producer.col.route')], rows);
  }
  if (n.rule_id === 'GEO_021' && d.group_0_count) {
    const rows = [];
    for (let i = 0; d[`group_${i}_count`]; i++) {
      const more = Number(d[`group_${i}_count`]) - listOf(d[`group_${i}_stops`]).length;
      rows.push({ cells: [pt(d[`group_${i}_lat`], d[`group_${i}_lon`]), localizedNumber(d[`group_${i}_count`]!),
        listOf(d[`group_${i}_stops`]).map(stopLabel).join(', ') + (more > 0 ? ` +${localizedNumber(String(more))}` : '')], bad: true });
    }
    return table(t('producer.times.geo_groups'), [t('producer.times.stp_position'), t('producer.times.geo_stop_count'), t('producer.times.geo_stops')], rows);
  }
  if (n.rule_id === 'GEO_020' && d.lat) {
    return table(t('producer.times.geo_shape_points'), [t('producer.times.geo_shape'), t('producer.times.stp_position'), t('producer.times.geo_point_count')],
      [{ cells: [shapeText(shpContext(n).shape, shpContext(n).routes, result), pt(d.lat, d.lon), localizedNumber(d.point_count || '—')], bad: true }]);
  }
  if (n.rule_id === 'GEO_017') {
    const rows = [];
    if (d.prev_line) rows.push({ cells: [d.prev_line, pt(d.prev_lat, d.prev_lon)] });
    rows.push({ cells: [String(n.line ?? '—'), '0, 0'], bad: true });
    if (d.next_line) rows.push({ cells: [d.next_line, pt(d.next_lat, d.next_lon)] });
    return table(t('producer.times.geo_null_rows'), [t('producer.col.row'), t('producer.times.stp_position')], rows);
  }
  return '';
}

/** Route numbers as riders know them, comma-separated ("S8, S9 +3"). */
function routeLabels(ids: string[], result: ValidationResult, max = 6): string {
  const shown = ids.slice(0, max).map(id => routeBadge(id, result.name_index).label);
  return shown.length ? shown.join(', ') + (ids.length > max ? ` +${localizedNumber(String(ids.length - max))}` : '') : '—';
}

/** Each VAT rule's own sentence in the planner's words: routes by number, stops by name, trips
 *  by route, direction and departure. */
function vatRuleText(n: Notice, result: ValidationResult): string | null {
  const ni = result.name_index;
  const d = n.details ?? {};
  const stop = (id?: string | null) => (id ? ni.stops?.[id] || id : '—');
  const label = (id?: string | null) => (id ? routeBadge(id, ni).label : '—');
  // The percentage next to its sign ("Jaccard=92%", "50/55 (%91)", "40% (4 routes)").
  const pctMatch = /%(\d+)|(\d+)%/.exec(n.observed_value ?? '');
  const pct = pctMatch?.[1] ?? pctMatch?.[2];
  switch (n.rule_id) {
    case 'VAT_001': {
      const [a, b] = listOf(d.routes);
      if (!a || !b || !d.shared_stops || !pct) return null;
      const shared = Number(d.shared_stops);
      const diff = Number(d.stops_a) - shared + Number(d.stops_b) - shared;
      return [
        t('producer.card.VAT_001.what_detail', {
          a: label(a), b: label(b), shared: localizedNumber(d.shared_stops), na: localizedNumber(d.stops_a || '—'),
          nb: localizedNumber(d.stops_b || '—'), pct: localizedNumber(pct),
        }),
        diff > 0 ? t('producer.card.VAT_001.what_diff', { n: localizedNumber(String(diff)) }) : t('producer.card.VAT_001.what_same'),
      ].join(' ');
    }
    case 'VAT_002':
    case 'VAT_007': {
      const routes = listOf(d.routes);
      if (!d.nearby_stop_id || !routes.length) return null;
      return t(`producer.card.${n.rule_id}.what_detail`, {
        stop: stop(n.entity_id), n: localizedNumber(String(routes.length)), routes: routeLabels(routes, result),
        near: stop(d.nearby_stop_id), dist: metersText(d.nearby_distance_m), nroutes: routeLabels(listOf(d.nearby_routes), result),
      });
    }
    case 'VAT_003': {
      if (!n.entity_id || !d.duration_min || !d.expected_min) return null;
      const dur = Number(d.duration_min);
      const exp = Number(d.expected_min);
      return [
        t(dur >= exp ? 'producer.card.VAT_003.what_long' : 'producer.card.VAT_003.what_short', {
          trip: tripPhrase(n.entity_id, result), dur: durationText(dur * 60), exp: durationText(exp * 60),
          diff: durationText(Math.abs(dur - exp) * 60), n: localizedNumber(d.pattern_trips || '—'),
        }),
        d.duplicate_trips ? t('producer.card.VAT_003.what_repeat', { n: localizedNumber(d.duplicate_trips) }) : '',
      ].filter(Boolean).join(' ');
    }
    case 'VAT_005': {
      // From the details: the summary opens into stop rows, which drop the observed value.
      const stops = String(listOf(d.isolated_stops).length);
      const clusters = d.cluster_count;
      if (!clusters || !d.cluster_0_stop_id) return null;
      return [
        t('producer.card.VAT_005.what_detail', { n: localizedNumber(clusters), stops: localizedNumber(stops) }),
        t(d.cluster_0_nearest_stop_id ? 'producer.card.VAT_005.what_example' : 'producer.card.VAT_005.what_example_plain', {
          stop: stop(d.cluster_0_stop_id), routes: routeLabels(listOf(d.cluster_0_routes), result),
          near: stop(d.cluster_0_nearest_stop_id), dist: metersText(d.cluster_0_nearest_m),
        }),
      ].join(' ');
    }
    case 'VAT_006':
      if (!d.trips || !d.total_trips || !pct) return null;
      return t('producer.card.VAT_006.what_detail', {
        route: label(routeIdOfNotice(n)), trips: localizedNumber(d.trips), total: localizedNumber(d.total_trips), pct: localizedNumber(pct),
        others: localizedNumber(String(Math.max(0, Number(d.route_count || '0') - 1))),
        rest: localizedNumber(String(Number(d.total_trips) - Number(d.trips))),
      });
    case 'VAT_008':
      if (!n.entity_id || !d.routes || !pct) return null;
      return t('producer.card.VAT_008.what_detail', {
        shape: n.entity_id, n: localizedNumber(d.route_count || String(listOf(d.routes).length)),
        routes: routeLabels(listOf(d.routes), result), total: localizedNumber(d.total_routes || '—'), pct: localizedNumber(pct),
      });
    default:
      return null;
  }
}

/** VAT tables: the stops only one route serves, the stops a transfer could link, the same path's
 *  trips with their durations, the detached clusters, the trip share per route, the routes on a shape. */
function vatTable(n: Notice, result: ValidationResult): string {
  const ni = result.name_index;
  const d = n.details ?? {};
  const stop = (id?: string | null) => (id ? (ni.stops?.[id] ? `${ni.stops[id]} (${id})` : id) : '—');
  const label = (id?: string | null) => (id ? routeBadge(id, ni).label : '—');
  const table = (caption: string, head: string[], rows: Array<{ cells: string[]; bad?: boolean }>) =>
    `<p class="producer-card-label">${escHtml(caption)}</p><table class="planner-times planner-stop-times"><thead><tr>${head.map(h => `<th>${escHtml(h)}</th>`).join('')}</tr></thead><tbody>${
      rows.map(r => `<tr${r.bad ? ' class="bad"' : ''}>${r.cells.map(c => `<td>${escHtml(c)}</td>`).join('')}</tr>`).join('')}</tbody></table>`;
  switch (n.rule_id) {
    case 'VAT_001': {
      const [a, b] = listOf(d.routes);
      const rows = [
        ...listOf(d.only_a).map(id => ({ cells: [stop(id), '✓', '—'], bad: true })),
        ...listOf(d.only_b).map(id => ({ cells: [stop(id), '—', '✓'], bad: true })),
      ];
      return a && b && rows.length ? table(t('producer.times.vat001_stops'), [t('producer.col.stop'), label(a), label(b)], rows) : '';
    }
    case 'VAT_002':
    case 'VAT_007':
      if (!d.nearby_stop_id || !n.entity_id) return '';
      return table(t('producer.times.vat_transfer_stops'), [t('producer.col.stop'), t('producer.times.vat_distance'), t('producer.col.route')], [
        { cells: [stop(n.entity_id), '—', routeLabels(listOf(d.routes), result)] },
        { cells: [stop(d.nearby_stop_id), metersText(d.nearby_distance_m), routeLabels(listOf(d.nearby_routes), result)], bad: true },
      ]);
    case 'VAT_003': {
      if (!n.entity_id || !d.duration_min) return '';
      const rows: Array<{ dep: string; cells: string[]; bad?: boolean }> = [
        { dep: d.departure || '', cells: [plannerTime(d.departure) || '—', durationText(Number(d.duration_min) * 60)], bad: true },
      ];
      for (let i = 0; d[`example_${i}_trip_id`]; i++) {
        const dep = d[`example_${i}_departure`] || '';
        rows.push({ dep, cells: [plannerTime(dep) || '—', durationText(Number(d[`example_${i}_minutes`]) * 60)] });
      }
      rows.sort((x, y) => x.dep.localeCompare(y.dep));
      return table(t('producer.times.vat003_trips'), [t('producer.col.departure'), t('producer.times.opr_duration')], rows);
    }
    case 'VAT_005': {
      const rows = [];
      for (let i = 0; d[`cluster_${i}_stop_id`]; i++) {
        const at = (key: string) => d[`cluster_${i}_${key}`];
        rows.push({
          cells: [stop(at('stop_id')), localizedNumber(at('stops') || '—'), routeLabels(listOf(at('routes')), result),
            at('nearest_stop_id') ? `${stop(at('nearest_stop_id'))} · ${metersText(at('nearest_m'))}` : '—'],
          bad: true,
        });
      }
      return rows.length
        ? table(t('producer.times.vat005_clusters'), [t('producer.times.vat005_example_stop'), t('producer.times.vat005_stop_count'), t('producer.col.route'), t('producer.times.vat005_nearest')], rows)
        : '';
    }
    case 'VAT_006': {
      const routeId = routeIdOfNotice(n);
      if (!routeId || !d.trips || !d.total_trips) return '';
      const total = Number(d.total_trips);
      const share = (v: string) => `%${localizedNumber(String(Math.round(Number(v) * 100 / Math.max(1, total))))}`;
      const rows = [{ cells: [label(routeId), localizedNumber(d.trips), share(d.trips)], bad: true }];
      for (let i = 0; d[`other_${i}_route_id`]; i++) {
        rows.push({ cells: [label(d[`other_${i}_route_id`]), localizedNumber(d[`other_${i}_trips`] || '0'), share(d[`other_${i}_trips`] || '0')], bad: false });
      }
      return table(t('producer.times.vat006_routes'), [t('producer.col.route'), t('producer.times.trip_count'), t('producer.times.vat006_share')], rows);
    }
    case 'VAT_008': {
      const routes = listOf(d.routes);
      if (!routes.length) return '';
      return table(t('producer.times.vat008_routes'), [t('producer.col.route'), t('producer.col.route_name'), t('producer.times.example_trip')],
        routes.map(id => ({ cells: [label(id), ni.route_long_names?.[id] || '—', tripShort(routeExampleTrip(ni, id), result, false)], bad: true })));
    }
    default:
      return '';
  }
}

/** CAL_020: do the calendars themselves run longer than the five-year limit the rule uses? */
function cal020LongCalendars(d: Record<string, string>): boolean {
  const ms = (v?: string) => (v && /^\d{8}$/.test(v) ? Date.UTC(Number(v.slice(0, 4)), Number(v.slice(4, 6)) - 1, Number(v.slice(6, 8))) : NaN);
  return (ms(d.service_last) - ms(d.service_first)) / 86_400_000 > 1825;
}

/** Each calendar rule's own sentence, in the planner's words; null when the finding lacks what
 *  the sentence needs (the card then keeps its general text). */
function calRuleText(n: Notice, result: ValidationResult): string | null {
  const d = n.details ?? {};
  const svc = n.entity_type === 'Service' || n.rule_id === 'CAL_025' ? n.entity_id ?? '' : '';
  const services = calServices(n);
  const count = localizedNumber(String(services.length));
  const value = (n.observed_value ?? '').trim();
  switch (n.rule_id) {
    case 'CAL_001':
      return svc ? t('producer.card.CAL_001.what_detail', { svc, first: d.first_line || '—', line: n.line != null ? String(n.line) : '—' }) : null;
    case 'CAL_002':
      return svc && n.field ? t('producer.card.CAL_002.what_detail', { svc, day: plannerFieldName(n.field), value: n.observed_value ?? '' }) : null;
    case 'CAL_003':
    case 'CAL_004':
      if (!svc) return null;
      return value ? t(`producer.card.${n.rule_id}.what_invalid`, { svc, value }) : t(`producer.card.${n.rule_id}.what_empty`, { svc });
    case 'CAL_005': {
      const start = /\d{8}/.exec(n.expected_value ?? '')?.[0];
      return svc && start && value ? t('producer.card.CAL_005.what_detail', { svc, start: ymdText(start), end: ymdText(value) }) : null;
    }
    case 'CAL_006': {
      if (!svc) return null;
      const exceptions = Number(result.name_index.service_days?.[svc]?.[3] ?? NaN);
      if (Number.isNaN(exceptions)) return null;
      return exceptions > 0
        ? t('producer.card.CAL_006.what_dates', { svc, n: localizedNumber(String(exceptions)) })
        : t('producer.card.CAL_006.what_never', { svc });
    }
    case 'CAL_007':
    case 'CAL_012': {
      const [from, to] = (n.entity_id ?? '').split('-');
      if (!from || !to || !services.length) return null;
      return t(`producer.card.${n.rule_id}.what_detail`, { n: count, from: ymdText(from), to: ymdText(to), gap: localizedNumber(n.observed_value ?? '—') });
    }
    case 'CAL_008': {
      // The summary opens into its calendars on the planner page; a calendar row has no observed
      // value, so its end date comes from the calendar itself.
      const date = n.observed_value || (svc ? result.name_index.service_days?.[svc]?.[2] : undefined);
      return d.affected_services && date
        ? t(d.days_left ? 'producer.card.CAL_008.what_detail' : 'producer.card.CAL_008.what_date', {
          n: localizedNumber(d.affected_services), date: ymdText(date), days: localizedNumber(d.days_left ?? '') })
        : null;
    }
    case 'CAL_009':
      return n.observed_value
        ? t(d.last_end_date ? 'producer.card.CAL_009.what_detail' : 'producer.card.CAL_009.what_count', {
          n: localizedNumber(n.observed_value), date: ymdText(d.last_end_date) })
        : null;
    case 'CAL_010': {
      if (!svc || !d.active_days) return null;
      const window = Number(d.feed_window_days);
      return t(window > 0 && window < 30 ? 'producer.card.CAL_010.what_window' : 'producer.card.CAL_010.what_detail', {
        svc, n: localizedNumber(d.active_days), first: ymdText(d.first_active), last: ymdText(d.last_active), window: localizedNumber(d.feed_window_days ?? ''),
      });
    }
    case 'CAL_011':
      return svc ? t('producer.card.CAL_011.what_detail', { svc, days: plannerServiceText(svc, result), range: serviceRangeText(svc, result) }) : null;
    case 'CAL_013':
      return services.length && n.observed_value
        ? t('producer.card.CAL_013.what_detail', { n: count, date: ymdText(n.observed_value) })
        : null;
    case 'CAL_014': {
      // The calendar's running days against the dataset's validity window, and how far it reaches out.
      const [first, last] = (n.observed_value ?? '').split('..');
      const [fs, fe] = (n.expected_value ?? '').split('..');
      const before = Number(d.days_before) || 0;
      const after = Number(d.days_after) || 0;
      if (!svc || !first || !last || !fs || !fe || (!before && !after)) return null;
      const key = before && after ? 'what_both' : before ? 'what_before' : 'what_after';
      return t(`producer.card.CAL_014.${key}`, {
        svc, first: ymdText(first), last: ymdText(last), fs: ymdText(fs), fe: ymdText(fe),
        before: localizedNumber(String(before)), after: localizedNumber(String(after)),
      });
    }
    case 'CAL_015': {
      const date = d.first_service_date || n.observed_value;
      if (!date) return null;
      return t(d.message_variant === 'future_only_jp' ? 'producer.card.CAL_015.what_jp' : 'producer.card.CAL_015.what_detail', {
        date: ymdText(date), days: localizedNumber(d.days_until ?? '—'),
      });
    }
    case 'CAL_016':
      return n.observed_value && services.length
        ? t('producer.card.CAL_016.what_detail', { date: ymdText(n.observed_value), n: count })
        : null;
    case 'CAL_017':
      return svc && n.observed_value ? t('producer.card.CAL_017.what_detail', { svc, date: ymdText(n.observed_value) }) : null;
    case 'CAL_018':
      return svc ? t('producer.card.CAL_018.what_detail', { svc }) : null;
    case 'CAL_020': {
      if (!d.feed_start_date || !d.feed_end_date || !d.span_days) return null;
      const written = t('producer.card.CAL_020.what_detail', {
        start: ymdText(d.feed_start_date), end: ymdText(d.feed_end_date), years: localizedNumber(String(Math.floor(Number(d.span_days) / 365))),
      });
      // The period the calendars really run, to set the window against — unless the calendars
      // themselves run as long (then the calendar setting the far end is the thing to fix first).
      if (!d.service_first || !d.service_last) return written;
      return `${written} ${t(cal020LongCalendars(d) ? 'producer.card.CAL_020.what_service_long' : 'producer.card.CAL_020.what_service', {
        first: ymdText(d.service_first), last: ymdText(d.service_last), svc: d.service_last_calendar || '—',
      })}`;
    }
    case 'CAL_021':
      return svc && d.window_days && d.next_active
        ? t('producer.card.CAL_021.what_detail', { svc, n: localizedNumber(d.window_days), next: ymdText(d.next_active) })
        : null;
    case 'CAL_022':
      return n.line != null ? t('producer.card.CAL_022.what_detail', { line: String(n.line) }) : null;
    case 'CAL_023':
      return svc && n.observed_value ? t('producer.card.CAL_023.what_detail', { svc, date: ymdText(n.observed_value) }) : null;
    case 'CAL_024':
      if (!svc || !n.observed_value) return null;
      return t(d.next_active ? 'producer.card.CAL_024.what_detail' : 'producer.card.CAL_024.what_none', {
        svc, n: localizedNumber(n.observed_value), next: ymdText(d.next_active),
      });
    case 'CAL_025':
      return svc && n.field ? t('producer.card.CAL_025.what_detail', { svc, day: plannerFieldName(n.field) }) : null;
    default:
      return null;
  }
}

function plannerCardCopy(g: RuleGroup, result: ValidationResult): PlannerCardCopy | null {
  const n = g.notices[0];
  switch (g.ruleId) {
    case 'XFL_002': case 'XFL_006': case 'XFL_012': case 'XFL_013': case 'XFL_020': case 'XFL_021': {
      const what = xflText(g, n, result);
      return what ? { title: t(`producer.card.${g.ruleId}.title`), what, action: t(`producer.card.${g.ruleId}.action`) } : null;
    }
    case 'PTH_001': case 'PTH_002': case 'PTH_003': case 'PTH_004': case 'PTH_005': case 'PTH_006':
    case 'PTH_007': case 'PTH_010': case 'PTH_011': case 'PTH_014': case 'PTH_015': case 'PTH_016':
    case 'PTH_017': case 'PTH_018': case 'PTH_026': case 'PTH_027': case 'PTH_028': case 'PTH_030':
    case 'PTH_031': case 'PTH_008': case 'PTH_009': case 'PTH_012': case 'PTH_013': case 'PTH_019':
    case 'PTH_020': case 'PTH_021': case 'PTH_022': case 'PTH_023': case 'PTH_024': case 'PTH_025':
    case 'PTH_029': {
      const what = pthText(n, result);
      return what ? { title: t(`producer.card.${g.ruleId}.title`), what, action: t(`producer.card.${g.ruleId}.action`) } : null;
    }
    case 'FRQ_007':
    case 'FRQ_001': case 'FRQ_002': case 'FRQ_003': case 'FRQ_004': case 'FRQ_005': case 'FRQ_006':
    case 'FRQ_008': case 'FRQ_009': case 'FRQ_010': case 'FRQ_011': case 'FRQ_012': {
      const what = frqText(n, result);
      return what ? { title: t(`producer.card.${g.ruleId}.title`), what, action: t(`producer.card.${g.ruleId}.action`) } : null;
    }
    case 'TRF_001': case 'TRF_002': case 'TRF_004': case 'TRF_005':
    case 'TRF_003': case 'TRF_006': case 'TRF_007': case 'TRF_008': case 'TRF_009': case 'TRF_010':
    case 'TRF_011': case 'TRF_012': case 'TRF_013': case 'TRF_014': case 'TRF_015': case 'TRF_016':
    case 'TRF_017': case 'TRF_018': case 'TRF_019': case 'TRF_020': case 'TRF_021': case 'TRF_022':
    case 'TRF_023': {
      const what = trfText(n, result);
      return what ? { title: t(`producer.card.${g.ruleId}.title`), what, action: t(`producer.card.${g.ruleId}.action`) } : null;
    }
    case 'JPN_001':
    case 'JPN_008':
    case 'JPN_009':
    case 'JPN_010': {
      const name = jpKanaName(n, result);
      if (!name) return null;
      // A reading added for the written name covers every record with the same spelling
      // (the engine accepts a translation matched by its source text), so count distinct names.
      const names = new Set(g.notices.map(x => jpKanaName(x, result)).filter(Boolean)).size;
      const routeId = n.rule_id === 'JPN_008' ? n.entity_id : n.rule_id === 'JPN_009' && n.entity_id ? result.name_index.trip_routes?.[n.entity_id] : undefined;
      const detail = t(`producer.card.${g.ruleId}.what_detail`, {
        name,
        kind: n.field ? plannerFieldName(n.field) : '—',
        route: routeId ? result.name_index.routes?.[routeId] || routeId : '—',
        trip: n.rule_id === 'JPN_009' ? tripPhrase(n.entity_id, result) : '—',
      });
      const summary = g.notices.length > 1 ? t('producer.card.JPN_kana.summary', { count: localizedNumber(String(g.count)), names: localizedNumber(String(names)) }) : '';
      return { title: t(`producer.card.${g.ruleId}.title`), what: [detail, summary].filter(Boolean).join(' '), action: t(`producer.card.${g.ruleId}.action`) };
    }
    case 'JPN_002':
    case 'JPN_003':
    case 'JPN_005':
    case 'JPN_011':
    case 'JPN_012':
    case 'JPN_013':
    case 'JPN_014':
    case 'JPN_015':
    case 'JPN_016':
    case 'JPN_017':
    case 'JPN_018':
    case 'JPN_020':
    case 'JPN_032': {
      const what = jpRecordText(n, result);
      return what ? { title: t(`producer.card.${g.ruleId}.title`), what, action: t(`producer.card.${g.ruleId}.action`) } : null;
    }
    case 'JPN_006':
    case 'JPN_022':
    case 'JPN_023':
    case 'JPN_024':
    case 'JPN_025':
    case 'JPN_026':
    case 'JPN_027':
    case 'JPN_031':
    case 'JPN_033': {
      const what = jpFeedText(n, result);
      return what ? { title: t(`producer.card.${g.ruleId}.title`), what, action: t(`producer.card.${g.ruleId}.action`) } : null;
    }
    case 'JPN_019': {
      const d = n.details ?? {};
      if (!d.table_name) return null;
      return {
        title: t('producer.card.JPN_019.title'),
        what: t('producer.card.JPN_019.what_detail', {
          list: plannerListName(d.table_name) || d.table_name,
          field: d.field_name ? plannerFieldName(d.field_name) : '—',
          target: jpTranslationTarget(n, result),
        }),
        action: t('producer.card.JPN_019.action'),
      };
    }
    case 'JPN_021': {
      const d = n.details ?? {};
      if (!d.issue || !['empty', 'conflict', 'no_kana'].includes(d.issue)) return null;
      return {
        title: t('producer.card.JPN_021.title'),
        what: t(`producer.card.JPN_021.what_${d.issue}`, {
          target: jpTranslationTarget(n, result),
          value: n.observed_value ?? '',
          other: n.expected_value ?? '',
          other_line: d.other_line ?? '—',
        }),
        action: t('producer.card.JPN_021.action'),
      };
    }
    case 'JPN_028':
    case 'JPN_029':
    case 'JPN_030': {
      const d = n.details ?? {};
      if (!d.source_value) return null;
      const both = g.ruleId === 'JPN_028' && d.message_variant === 'aggregate_both';
      return {
        title: t(`producer.card.${g.ruleId}.title`),
        what: t(`producer.card.${g.ruleId}.what_detail${both ? '_both' : ''}`, {
          value: d.source_value,
          field: n.field ? plannerFieldName(n.field) : '—',
          list: plannerListName(d.table_name) || d.table_name || '—',
          count: localizedNumber(d.affected_records ?? '1'),
        }),
        action: t(`producer.card.${g.ruleId}.action`),
      };
    }
    case 'STM_006': {
      const routeId = n.entity_id ? result.name_index.trip_routes?.[n.entity_id] : undefined;
      const route = routeId ? result.name_index.routes?.[routeId] || '—' : '—';
      const direction = n.entity_id ? result.name_index.trips?.[n.entity_id] : undefined;
      const service = plannerServiceText(n.service_id, result);
      const departure = n.entity_id ? result.name_index.trip_first_dep?.[n.entity_id] || '—' : '—';
      return {
        title: t('producer.card.STM_006.title'),
        context: t('producer.card.STM_006.context', { route, direction: direction || '—', departure, service }),
        what: t('producer.card.STM_006.what', {
          arrival: plannerTime(n.details?.arrival) || '—',
          departure: plannerTime(n.details?.departure) || '—',
        }),
        action: t('producer.card.STM_006.action'),
      };
    }
    case 'STM_001': {
      const detail = n.details ?? {};
      const stopName = (id?: string) => id ? result.name_index.stops?.[id] || id : t('producer.card.unknown_stop');
      return {
        title: t('producer.card.STM_001.title'),
        what: t('producer.card.STM_001.what', {
          first_stop: stopName(detail.first_stop_id),
          first_departure: plannerTime(detail.first_departure) || '—',
          last_stop: stopName(detail.last_stop_id),
          last_arrival: plannerTime(detail.last_arrival) || '—',
          stop_count: detail.stop_count || '—',
        }),
        action: t('producer.card.STM_001.action'),
      };
    }
    case 'STM_012': {
      const context = tripContext(n, result);
    // If one stop is at 0,0, the speed spikes for that reason; the location is what needs fixing.
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
    case 'STM_014': {
      const d = n.details ?? {};
      const name = (id?: string) => id ? result.name_index.stops?.[id] || id : '—';
      const speedMin = d.speed_min ? localizedNumber(d.speed_min) : '—';
      const speedMax = d.speed_max ? localizedNumber(d.speed_max) : speedMin;
      const distanceMin = d.distance_min_meters ? localizedNumber(d.distance_min_meters) : '—';
      const distanceMax = d.distance_max_meters ? localizedNumber(d.distance_max_meters) : distanceMin;
      return {
        title: t('producer.card.STM_014.title'),
        what: t('producer.card.STM_014.what', {
          from: name(d.stop_a), to: name(d.stop_b),
          speed: speedMin === speedMax ? speedMax : `${speedMin}–${speedMax}`,
          distance: distanceMin === distanceMax ? distanceMax : `${distanceMin}–${distanceMax}`,
          threshold: d.threshold_kmh ? localizedNumber(d.threshold_kmh) : '—',
        }),
        action: t('producer.card.STM_014.action'),
      };
    }
    case 'STP_001': case 'STP_002': case 'STP_003': case 'STP_004': case 'STP_005':
    case 'STP_006': case 'STP_007': case 'STP_008': case 'STP_009': case 'STP_010': case 'STP_011': case 'STP_012':
    case 'STP_013': case 'STP_014': case 'STP_015': case 'STP_016': case 'STP_017': case 'STP_019': case 'STP_020':
    case 'STP_021': case 'STP_022': case 'STP_023': case 'STP_024': case 'STP_026': case 'STP_028': case 'STP_029':
    case 'STP_030': case 'STP_031': case 'STP_032': case 'STP_033': case 'STP_034': case 'STP_035': case 'STP_036':
    case 'STP_037': case 'STP_038': case 'STP_039': case 'STP_040': case 'STP_041': case 'STP_042': case 'STP_043':
    case 'STP_044': case 'STP_045': {
      // STP_001/009 keep their context line (the stop code; the stop and the missing station's code);
      // without planner details the cards keep their general text.
      const stopName = n.entity_id ? result.name_index.stops?.[n.entity_id] || n.entity_id : '';
      const context = g.ruleId === 'STP_001' || g.ruleId === 'STP_012' ? t(`producer.card.${g.ruleId}.context`, { stop: n.entity_id || n.observed_value || '' })
        : g.ruleId === 'STP_009' ? t('producer.card.STP_009.context', { stop: stopName, parent: n.observed_value || '—' }) : undefined;
      const what = stpRuleText(n, result);
      if (!what && !context) return null;
      return {
        title: t(`producer.card.${g.ruleId}.title`), ...(context ? { context } : {}),
        what: what || t(`producer.card.${g.ruleId}.what`), action: t(`producer.card.${g.ruleId}.action`),
      };
    }
    case 'STM_015':
    case 'STM_016': {
      const context = tripContext(n, result);
      const stopId = n.details?.stop_id;
      const stop = stopId ? result.name_index.stops?.[stopId] || stopId : '';
    // If no stop in the trip has a time, the problem is not one endpoint but a trip imported without times.
      const untimed = n.details?.timed_stops === '0';
      const singleUntimedStop = untimed && n.details?.stop_count === '1' && stop;
      return {
        title: t(`producer.card.${g.ruleId}.title`),
        context: tripContextText(context),
        what: untimed
          ? singleUntimedStop
            ? t('producer.card.trip_untimed_at', { stop })
            : t('producer.card.trip_untimed', { n: n.details?.stop_count ?? '?' })
          : stop
            ? t(`producer.card.${g.ruleId}.what_at`, { stop })
            : t(`producer.card.${g.ruleId}.what`),
        action: untimed ? t('producer.card.trip_untimed_action') : t(`producer.card.${g.ruleId}.action`),
      };
    }
    case 'STM_017': {
      const d = n.details ?? {};
      const context = n.entity_type === 'Trip' ? tripContextText(tripContext(n, result)) : undefined;
      const summary = d.summary_mode === 'feed';
      return {
        title: t('producer.card.STM_017.title'),
        context,
        what: summary
          ? t('producer.card.STM_017.what_feed', {
            trips: d.affected_trip_count || g.count,
            shapes: d.affected_shape_count || '—',
          })
          : t('producer.card.STM_017.what_trip', {
            affected: d.affected_stop_count || '—',
            total: d.stop_count || '—',
          }),
        // Every stop missing: there are no other distances to remove, so say what applies.
        action: t(summary ? 'producer.card.STM_017.action_feed'
          : Number(d.affected_stop_count) > 0 && d.affected_stop_count === d.stop_count
            ? 'producer.card.STM_017.action_trip_all'
            : 'producer.card.STM_017.action_trip'),
      };
    }
    case 'STM_018':
    case 'STM_019': {
      const d = n.details ?? {};
      const name = (id?: string) => id ? result.name_index.stops?.[id] || id : t('producer.card.unknown_stop');
      const key = n.rule_id;
      return {
        title: t(`producer.card.${key}.title`),
        context: n.entity_type === 'Trip' ? tripContextText(tripContext(n, result)) : undefined,
        what: d.next_stop_id
          ? t(`producer.card.${key}.what_segment`, {
            from: name(d.stop_id), to: name(d.next_stop_id), value: visibleValue(n.observed_value),
          })
          : t(`producer.card.${key}.what`, {
            stop: name(d.stop_id), value: visibleValue(n.observed_value),
          }),
        action: t(`producer.card.${key}.action`),
      };
    }
    case 'STM_032':
    case 'STM_033':
    case 'STM_034':
    case 'STM_035': {
      const d = n.details ?? {};
      const name = (id?: string) => (id ? result.name_index.stops?.[id] || id : '—');
      const time = (arr?: string, dep?: string) => plannerTime(dep) || plannerTime(arr) || '—';
      let what: string | null = null;
      if (g.ruleId === 'STM_032' && d.first_stop_id && d.second_stop_id) {
        what = t('producer.card.STM_032.what_detail', {
          sequence: n.observed_value ?? '—',
          first: name(d.first_stop_id), first_time: time(d.first_arrival, d.first_departure),
          second: name(d.second_stop_id), second_time: time(d.second_arrival, d.second_departure),
        });
      } else if (g.ruleId === 'STM_033' && d.stop_id) {
        what = t('producer.card.STM_033.what_detail', {
          stop: name(d.stop_id), sequence: d.stop_sequence || '—', time: time(d.arrival, d.departure),
        });
      } else if (g.ruleId === 'STM_034' && d.stop_id) {
        // `field` is the side that is missing.
        const missingDeparture = n.field === 'departure_time';
        what = t(missingDeparture ? 'producer.card.STM_034.what_no_departure' : 'producer.card.STM_034.what_no_arrival', {
          stop: name(d.stop_id), sequence: d.stop_sequence || '—',
          time: plannerTime(missingDeparture ? d.arrival : d.departure) || '—',
        });
      } else if (g.ruleId === 'STM_035' && d.stop_id) {
        what = t('producer.card.STM_035.what_detail', {
          stop: name(d.stop_id),
          first_sequence: d.stop_a_sequence || '—', second_sequence: d.stop_b_sequence || '—',
          first_time: time(d.stop_a_arrival, d.stop_a_departure), second_time: time(d.stop_b_arrival, d.stop_b_departure),
        }) + (Number(d.affected_trips) > 1
          ? ` ${t('producer.card.STM_035.what_trips', { n: localizedNumber(d.affected_trips) })}`
          : '');
      }
      if (!what) return null;
      return {
        title: t(`producer.card.${g.ruleId}.title`),
        context: tripContextText(tripContext(n, result)),
        what,
        action: t(`producer.card.${g.ruleId}.action`),
      };
    }
    case 'STM_036': {
      const d = n.details ?? {};
      if (!d.kind) return null;
      const name = (id?: string) => (id ? result.name_index.stops?.[id] || id : '—');
      const what = d.kind === 'order'
        ? t('producer.card.STM_036.what_order', {
          current: name(d.curr_stop), current_sequence: d.curr_seq ?? '—',
          previous: name(d.prev_stop), previous_sequence: d.prev_seq ?? '—',
        })
        : t('producer.card.STM_036.what_scattered', { from: d.line_from ?? '—', to: d.line_to ?? '—' });
      const total = Number(d.affected_trips);
      const feed = total > 1
        ? ` ${t('producer.card.STM_036.what_feed', {
          n: localizedNumber(String(total)),
          order: localizedNumber(d.order_trips ?? '0'), scattered: localizedNumber(d.scattered_trips ?? '0'),
        })}`
        : '';
      return {
        title: t('producer.card.STM_036.title'),
        context: tripContextText(tripContext(n, result)),
        what: what + feed,
        action: t('producer.card.STM_036.action'),
      };
    }
    case 'STM_037':
    case 'STM_038':
    case 'STM_039':
    case 'STM_040': {
      const d = n.details ?? {};
      const zone = d.location_id || d.location_group_id;
      if (!zone) return null;
      const missing = n.field === 'end_pickup_drop_off_window' ? 'end' : 'start';
      return {
        title: t(`producer.card.${g.ruleId}.title`),
        context: tripContextText(tripContext(n, result)),
        what: t(`producer.card.${g.ruleId}.what_detail`, {
          zone,
          start: plannerTime(d.window_start) || d.window_start || '—',
          end: plannerTime(d.window_end) || d.window_end || '—',
          time: plannerTime(d.arrival) || plannerTime(d.departure) || '—',
          missing: t(`producer.card.STM_039.missing_${missing}`),
        }),
        action: t(`producer.card.${g.ruleId}.action`),
      };
    }
    case 'STM_041':
    case 'STM_042':
    case 'STM_045':
    case 'STM_046': {
      // Stop-row findings: the row's stop and time (STM_046 has no trip to name, so the stop,
      // sequence and time are what the planner searches by).
      const d = n.details ?? {};
      if (!d.stop_id) return null;
      const stop = result.name_index.stops?.[d.stop_id] || d.stop_id;
      const sequence = d.stop_sequence || '—';
      const time = plannerTime(d.departure) || plannerTime(d.arrival) || '—';
      let what: string;
      if (g.ruleId === 'STM_041') {
        const zone = d.location_id || d.location_group_id;
        if (!zone) return null;
        what = t('producer.card.STM_041.what_detail', { stop, zone, sequence });
      } else if (g.ruleId === 'STM_042') {
        if (!d.stop_headsign) return null;
        what = t('producer.card.STM_042.what_detail', { stop, sequence, headsign: d.stop_headsign, char: n.observed_value ?? '—' });
      } else if (g.ruleId === 'STM_045') {
        // The engine's own window end (≤27:00:00 by default; config and rail trips change it).
        const limit = /^≤(\d{2,3}:\d{2}:\d{2})$/.exec(n.expected_value ?? '')?.[1];
        what = t('producer.card.STM_045.what_detail', {
          stop, time: plannerTime(n.observed_value) || n.observed_value || '—', limit: plannerTime(limit) || '—',
        });
      } else {
        what = t('producer.card.STM_046.what_detail', { stop, sequence, time });
      }
      return {
        title: t(`producer.card.${g.ruleId}.title`),
        context: g.ruleId === 'STM_046' ? undefined : tripContextText(tripContext(n, result)),
        what,
        action: t(`producer.card.${g.ruleId}.action`),
      };
    }
    case 'STM_043': {
      const d = n.details ?? {};
      if (!d.first_stop_id || !d.last_stop_id) return null;
      const name = (id: string) => result.name_index.stops?.[id] || id;
      const stops = Number(d.stop_count);
      const distinct = Number(d.distinct_stops);
      // Only a measured repeat is named: fewer distinct stops than stop times.
      const repeated = distinct > 0 && distinct < stops
        ? ` ${t('producer.card.STM_043.repeated', { distinct: localizedNumber(d.distinct_stops) })}`
        : '';
      return {
        title: t('producer.card.STM_043.title'),
        context: tripContextText(tripContext(n, result)),
        what: t('producer.card.STM_043.what_detail', {
          first: name(d.first_stop_id), dep: plannerTime(d.first_departure) || '—',
          last: name(d.last_stop_id), arr: plannerTime(d.last_arrival) || '—',
          stops: localizedNumber(d.stop_count ?? '0'),
        }) + repeated,
        action: t('producer.card.STM_043.action'),
      };
    }
    case 'STM_044': {
      if (!n.observed_value || !/^\d+$/.test(n.observed_value)) return null;
      return {
        title: t('producer.card.STM_044.title'),
        what: t('producer.card.STM_044.what_detail', { count: localizedNumber(n.observed_value) }),
        action: t('producer.card.STM_044.action'),
      };
    }
    case 'STM_028':
    case 'STM_029': {
      const d = n.details ?? {};
      if (!d.first_stop_id || !d.last_stop_id) return null;
      const name = (id?: string) => (id ? result.name_index.stops?.[id] || id : '—');
      const common = {
        first: name(d.first_stop_id), dep: plannerTime(d.first_departure) || '—',
        last: name(d.last_stop_id), arr: plannerTime(d.last_arrival) || '—',
        stops: d.stop_count ?? '—',
      };
      if (g.ruleId === 'STM_029') {
        return {
          title: t('producer.card.STM_029.title'),
          context: tripContextText(tripContext(n, result)),
          what: t('producer.card.STM_029.what_detail', {
            ...common,
            duration: durationText(Number(d.duration_seconds)),
            distance: metersText(d.distance_meters),
          }),
          action: t('producer.card.STM_029.action_detail'),
        };
      }
      const gap = d.gap_stop_a && d.gap_stop_b
        ? ` ${t('producer.card.STM_028.what_gap', {
          from: name(d.gap_stop_a), from_time: plannerTime(d.gap_from) || '—',
          to: name(d.gap_stop_b), to_time: plannerTime(d.gap_to) || '—',
          gap: durationText(Number(d.gap_seconds)),
        })}`
        : '';
      const rail = d.rail === '1' ? ` ${t('producer.card.STM_028.what_rail')}` : '';
      return {
        title: t('producer.card.STM_028.title'),
        context: tripContextText(tripContext(n, result)),
        what: t('producer.card.STM_028.what_detail', {
          ...common,
          duration: durationText(hmsSeconds(n.observed_value)),
          threshold: durationText(hmsSeconds(n.expected_value)),
        }) + gap + rail,
        action: t('producer.card.STM_028.action_detail'),
      };
    }
    case 'STM_030': {
      const d = n.details ?? {};
      if (!d.stop_id) return null;
      return {
        title: t('producer.card.STM_030.title'),
        context: tripContextText(tripContext(n, result)),
        what: t('producer.card.STM_030.what_detail', {
          stop: result.name_index.stops?.[d.stop_id] || d.stop_id,
          sequence: d.stop_sequence || '—',
          value: n.observed_value ?? '—',
        }),
        action: t('producer.card.STM_030.action'),
      };
    }
    case 'STM_025':
    case 'STM_026': {
      const d = n.details ?? {};
      if (!d.stop_a || !d.stop_b || !d.distance_meters) return null;
      const name = (id: string) => result.name_index.stops?.[id] || id;
      const common = {
        from: name(d.stop_a), to: name(d.stop_b),
        from_sequence: d.stop_a_sequence || '—', to_sequence: d.stop_b_sequence || '—',
      };
      const what = g.ruleId === 'STM_025'
        ? t('producer.card.STM_025.what_detail', { ...common, duration: d.duration_seconds ?? '—', distance: localizedNumber(d.distance_meters) })
        : t('producer.card.STM_026.what_detail', {
          ...common,
          distance: localizedNumber(String(Number(d.distance_meters) / 1000)),
          threshold: d.threshold_km ?? '—',
        });
      return {
        title: t(`producer.card.${g.ruleId}.title`),
        context: tripContextText(tripContext(n, result)),
        what,
        action: t(`producer.card.${g.ruleId}.action`),
      };
    }
    case 'STM_024': {
      const d = n.details ?? {};
      if (!d.stop_times_max || !d.shape_max) return null;
      const unitText = (value?: string) => {
        const unit = distanceUnit(value, d.shape_length_m);
        return unit ? t(`producer.card.STM_024.unit_${unit}`) : t('producer.card.STM_024.unit_unknown');
      };
      const what = d.shape_length_m
        ? t('producer.card.STM_024.what_detail', {
          length: localizedNumber(String(Number(d.shape_length_m) / 1000)),
          shape_max: localizedNumber(d.shape_max), shape_unit: unitText(d.shape_max),
          stop_max: localizedNumber(d.stop_times_max), stop_unit: unitText(d.stop_times_max),
        })
        : t('producer.card.STM_024.what_no_length', {
          shape_max: localizedNumber(d.shape_max), stop_max: localizedNumber(d.stop_times_max),
        });
      const shapes = Number(d.affected_shapes);
      const feed = shapes > 1
        ? ` ${t('producer.card.STM_024.what_feed', { shapes: localizedNumber(String(shapes)), trips: localizedNumber(d.affected_trips ?? '—') })}`
        : '';
      return {
        title: t('producer.card.STM_024.title'),
        context: tripContextText(tripContext(n, result)),
        what: what + feed,
        action: t('producer.card.STM_024.action_detail'),
      };
    }
    case 'STM_022': {
      const d = n.details ?? {};
      if (!d.stop_id) return null;
      const stop = result.name_index.stops?.[d.stop_id] || d.stop_id;
      const value = n.observed_value ?? '—';
      // Name the pattern only when it is measured: one written value covers at least half of
      // the invalid rows, so a single export setting may be behind every row.
      const top = Number(d.value_0_count);
      const total = Number(d.affected_rows);
      const pattern = d.value_0 !== undefined && total > 1 && top / total >= 0.5
        ? ` ${t('producer.card.STM_022.what_pattern', { value: d.value_0, count: localizedNumber(String(top)), total: localizedNumber(String(total)) })}`
        : '';
      return {
        title: t('producer.card.STM_022.title'),
        context: tripContextText(tripContext(n, result)),
        what: t('producer.card.STM_022.what_detail', { stop, sequence: d.stop_sequence || '—', value }) + pattern,
        action: t('producer.card.STM_022.action_detail'),
      };
    }
    case 'STM_021': {
      const d = n.details ?? {};
      const name = (id?: string) => id ? result.name_index.stops?.[id] || id : t('producer.card.unknown_stop');
      const place = d.stop_a ? result.name_index.stop_coords?.[d.stop_a] : undefined;
      return {
        title: t('producer.card.STM_021.title'),
        context: tripContextText(tripContext(n, result)),
        what: t('producer.card.STM_021.what_detail', {
          from: name(d.stop_a), to: name(d.stop_b),
          from_sequence: d.stop_a_sequence || '—', to_sequence: d.stop_b_sequence || '—',
          lat: coordText(place?.[0]), lon: coordText(place?.[1]),
        }) + (Number(d.affected_trips) > 1
          ? ` ${t('producer.card.STM_021.what_trips', { n: localizedNumber(d.affected_trips) })}`
          : ''),
        action: t('producer.card.STM_021.action'),
      };
    }
    case 'STM_020': {
      const d = n.details ?? {};
      const name = (id?: string) => id ? result.name_index.stops?.[id] || id : t('producer.card.unknown_stop');
      return {
        title: t('producer.card.STM_020.title'),
        context: tripContextText(tripContext(n, result)),
        what: t('producer.card.STM_020.what_detail', {
          from: name(d.stop_a), to: name(d.stop_b),
          from_time: plannerTime(d.section_start_time) || '—',
          to_time: plannerTime(d.section_end_time) || '—',
          distance: d.distance_meters ? localizedNumber(d.distance_meters) : '—',
        }),
        action: t('producer.card.STM_020.action_detail'),
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
    case 'ARC_012': {
      // INFO = rows that leave out trailing optional fields (valid CSV); the rest have extra fields.
      const short = n.severity === 'INFO';
      return {
        title: t(short ? 'producer.card.ARC_012.title_short' : 'producer.card.ARC_012.title'),
        what: t(short ? 'producer.card.ARC_012.what_short' : 'producer.card.ARC_012.what', {
          file: dqListLabel(n) || n.file || t('producer.card.unknown_file'),
          n: localizedNumber(n.details?.affected_rows ?? '1'),
        }),
        action: t(short ? 'producer.card.ARC_012.action_short' : 'producer.card.ARC_012.action'),
      };
    }
    case 'ARC_020': {
      const both = g.notices.some(x => x.file?.includes('/'));
      const kind = both ? 'both' : n.file === 'shapes.txt' ? 'shapes' : 'feed_info';
      return {
        title: t('producer.card.ARC_020.title'),
        what: t(`producer.card.ARC_020.what_${kind}`),
        action: t(`producer.card.ARC_020.action_${kind}`),
      };
    }
    case 'ARC_025':
      return {
        title: t('producer.card.ARC_025.title'),
        what: t('producer.card.ARC_025.what', {
          file: dqListLabel(n) || n.file || t('producer.card.unknown_file'),
          field: n.field ? plannerFieldName(n.field) : t('producer.card.unknown_field'),
        }),
        action: t('producer.card.ARC_025.action'),
      };
    case 'ARC_035':
      return {
        title: t('producer.card.ARC_035.title'),
        what: t('producer.card.ARC_035.what', { file: dqListLabel(n) || n.file || t('producer.card.unknown_file') }),
        action: t('producer.card.ARC_035.action'),
      };
    case 'AGN_002': case 'AGN_004': case 'AGN_006': case 'AGN_007': case 'AGN_008': case 'AGN_009':
    case 'AGN_010': case 'AGN_012': case 'AGN_013': case 'AGN_014': case 'AGN_015': case 'AGN_016':
    case 'AGN_018':
      return { title: t(`producer.card.${g.ruleId}.title`), what: agnText(n, result), action: t(`producer.card.${g.ruleId}.action`) };
    case 'AGN_003':
      return {
        title: t('producer.card.AGN_003.title'),
        what: t('producer.card.AGN_003.what', { agency: agencyLabel(n, result) }),
        action: t('producer.card.AGN_003.action'),
        observed: visibleValue(n.observed_value),
      };
    case 'STM_002': {
      const details = n.details ?? {};
      const routeId = details.example_1_route_id;
      const route = routeId ? result.name_index.routes?.[routeId] || routeId : '—';
      const serviceId = details.example_1_service_id;
      const service = serviceText(serviceId, result);
      const serviceName = details.example_1_service_name;
      const serviceContext = serviceName
        ? t('producer.card.STM_002.service_context', { name: serviceName, days: service })
        : service;
      const time = details.example_1_departure
        || (details.example_1_window_start && details.example_1_window_end
          ? t('producer.card.pickup_window', { start: plannerTime(details.example_1_window_start) ?? '', end: plannerTime(details.example_1_window_end) ?? '' })
          : t('producer.card.no_scheduled_time'));
      const timeRange = details.example_1_window_start && details.example_1_window_end
        ? t('producer.card.time_range', { start: plannerTime(details.example_1_window_start) ?? '', end: plannerTime(details.example_1_window_end) ?? '' })
        : time;
      const geojsonLocation = details.geojson_location === 'true';
      return {
        title: t('producer.card.STM_002.title'),
        context: t('producer.card.STM_002.context', {
          route,
          service: serviceContext,
        }),
        // "Hat kodu" is the route's number riders know (route_short_name, as the badge), not route_id.
        what: t(geojsonLocation ? 'producer.card.STM_002.geojson_what' : 'producer.card.STM_002.what', {
          route_code: routeId ? routeBadge(routeId, result.name_index).label : '—', service: serviceContext, time, time_range: timeRange,
        }),
        action: t(geojsonLocation ? 'producer.card.STM_002.geojson_action' : 'producer.card.STM_002.action'),
      };
    }
    case 'STM_007': {
  // Feed-level summary: row and trip counts, the most common difference, and the most affected stops.
      const d = n.details ?? {};
      const rows = Number(d.affected_rows);
      const gapRows = Number(d.typical_gap_rows);
      const parts: string[] = [];
      if (rows > 1 && d.affected_trips) {
        parts.push(t('producer.card.STM_007.summary', {
          rows: rows.toLocaleString(getLocale()), trips: Number(d.affected_trips).toLocaleString(getLocale()),
        }));
      }
      if (rows > 1 && d.typical_gap_seconds && gapRows) {
        parts.push(t('producer.card.STM_007.gap', {
          share: new Intl.NumberFormat(getLocale(), { style: 'percent' }).format(gapRows / rows), gap: d.typical_gap_seconds,
        }));
  // If the same difference occurs in more than half the rows, it is not an isolated typo.
        if (gapRows / rows >= 0.5) parts.push(t('producer.card.STM_007.pattern'));
      }
      const top = listOf(d.top_stops).map(id => result.name_index.stops?.[id] || id);
      if (rows > 1 && top.length) parts.push(t('producer.card.STM_007.top_stops', { stops: top.join(', ') }));
      return {
        title: t('producer.card.STM_007.title'),
        context: parts.join(' ') || undefined,
        what: t('producer.card.STM_007.what'),
        action: t('producer.card.STM_007.action'),
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
      const d = n.details ?? {};
      const firstStop = d.example_0_stop_id;
      return {
        title: t('producer.card.STM_047.title'),
        context: contextParts.join(t('producer.card.context_separator')),
        what: firstStop
          ? t('producer.card.STM_047.what_detail', {
            n: localizedNumber(d.affected_rows ?? '1'),
            stop: result.name_index.stops?.[firstStop] || firstStop,
            sequence: d.example_0_stop_sequence || '—',
          })
          : t('producer.card.STM_047.what'),
        action: t('producer.card.STM_047.action'),
      };
    }
    case 'STM_051':
    case 'STM_052':
    case 'STM_054':
    case 'STM_055': {
      // A Flex row with a fixed boarding setting: the zone (or stop), its window and the value.
      const d = n.details ?? {};
      const zone = d.location_id || d.location_group_id;
      const place = zone
        ? t('producer.card.flex_place_zone', { zone })
        : d.stop_id ? t('producer.card.flex_place_stop', { stop: result.name_index.stops?.[d.stop_id] || d.stop_id }) : '';
      if (!place) return null;
      return {
        title: t(`producer.card.${g.ruleId}.title`),
        context: tripContextText(tripContext(n, result)),
        what: t(`producer.card.${g.ruleId}.what_detail`, {
          place,
          start: plannerTime(d.window_start) || '—',
          end: plannerTime(d.window_end) || '—',
          ...flexSetting(n),
        }),
        action: t(`producer.card.${g.ruleId}.action`),
      };
    }
    case 'STM_050': {
      // Feed summary: how many rows leave the mark empty, how many of them carry a time, and a
      // fix per kind (untimed rows are estimated, so 0; timed rows need the planner's call).
      const d = n.details ?? {};
      const empty = Number(d.empty_rows);
      const timed = Number(d.empty_rows_with_time);
      if (!(empty > 0) || !Number.isFinite(timed)) return null;
      const untimed = empty - timed;
      const steps = [
        untimed > 0 ? t('producer.card.STM_050.action_untimed', { n: localizedNumber(String(untimed)) }) : '',
        timed > 0 ? t('producer.card.STM_050.action_timed', { n: localizedNumber(String(timed)) }) : '',
      ].filter(Boolean);
      return {
        title: t('producer.card.STM_050.title'),
        what: t('producer.card.STM_050.what_detail', {
          n: localizedNumber(String(empty)), total: localizedNumber(d.total_rows ?? String(empty)),
          timed: localizedNumber(String(timed)), untimed: localizedNumber(String(untimed)),
        }),
        action: steps.join(' '),
      };
    }
    case 'STM_058': {
      const d = n.details ?? {};
      const place = flexPlace(d, result);
      if (!place) return null;
      return {
        title: t('producer.card.STM_058.title'),
        context: tripContextText(tripContext(n, result)),
        what: t('producer.card.STM_058.what_detail', {
          place,
          side: t(n.field === 'end_pickup_drop_off_window' ? 'producer.card.STM_039.missing_end' : 'producer.card.STM_039.missing_start'),
          value: visibleValue(n.observed_value) || '—',
        }),
        action: t('producer.card.STM_058.action'),
      };
    }
    case 'STM_059': {
      const d = n.details ?? {};
      const place = flexPlace(d, result);
      if (!place) return null;
      return {
        title: t('producer.card.STM_059.title'),
        context: tripContextText(tripContext(n, result)),
        what: t('producer.card.STM_059.what_detail', {
          place,
          sequence: d.stop_sequence || '—',
          side: t(n.field === 'drop_off_booking_rule_id' ? 'producer.card.STM_059.side_drop_off' : 'producer.card.STM_059.side_pickup'),
        }),
        action: t('producer.card.STM_059.action'),
      };
    }
    case 'STM_060': {
      // Two Flex records of one trip: both zones, their windows and the side both offer.
      const d = n.details ?? {};
      if (!d.zone_a_id || !d.zone_b_id) return null;
      return {
        title: t('producer.card.STM_060.title'),
        context: tripContextText(tripContext(n, result)),
        what: t('producer.card.STM_060.what_detail', {
          a: d.zone_a_id, a_sequence: d.zone_a_stop_sequence || '—',
          a_start: plannerTime(d.zone_a_window_start) || '—', a_end: plannerTime(d.zone_a_window_end) || '—',
          b: d.zone_b_id, b_sequence: d.zone_b_stop_sequence || '—',
          b_start: plannerTime(d.zone_b_window_start) || '—', b_end: plannerTime(d.zone_b_window_end) || '—',
          start: plannerTime(d.window_overlap_start) || '—', end: plannerTime(d.window_overlap_end) || '—',
          side: t(`producer.card.STM_060.side_${d.overlap === 'both' || d.overlap === 'drop_off' ? d.overlap : 'pickup'}`),
        }),
        action: t('producer.card.STM_060.action'),
      };
    }
    case 'STM_061': {
      // A distant stop pair: the distance, the fastest example trip's times and the speed they imply.
      const d = n.details ?? {};
      if (!d.stop_a || !d.stop_b || !d.stop_a_departure || !d.stop_b_arrival) return null;
      const name = (id: string) => result.name_index.stops?.[id] || id;
      const dep = hmsSeconds(d.stop_a_departure);
      const arr = hmsSeconds(d.stop_b_arrival);
      const minutes = Number.isFinite(arr - dep) ? Math.round((arr - dep) / 6) / 10 : null;
      const routeId = routeIdOfNotice(n) || d.route_id;
      const route = (routeId ? result.name_index.routes?.[routeId] : undefined) || d.route_label;
      const params = {
        from: name(d.stop_a), to: name(d.stop_b),
        a_sequence: d.stop_a_sequence || '—', b_sequence: d.stop_b_sequence || '—',
        distance: localizedNumber(d.dist_km ?? '—'),
        dep: plannerTime(d.stop_a_departure) || '—', arr: plannerTime(d.stop_b_arrival) || '—',
        minutes: minutes === null ? '—' : localizedNumber(String(minutes)),
        speed: localizedNumber(d.speed_max ?? '—'),
        threshold: localizedNumber(d.threshold_kmh ?? '—'),
        between: localizedNumber(d.between_stops ?? '0'),
      };
      return {
        title: t('producer.card.STM_061.title'),
        context: [
          route ? t('producer.card.route_context', { route }) : '',
          d.trip_count ? t('producer.card.trip_count_context', { count: localizedNumber(d.trip_count) }) : '',
        ].filter(Boolean).join(t('producer.card.context_separator')) || undefined,
        what: t('producer.card.STM_061.what_detail', params),
        action: t('producer.card.STM_061.action_detail', params),
      };
    }
    case 'STM_053': {
      // Route summary: how many trips repeat one time, and the example trip's run by name.
      const d = n.details ?? {};
      const run = stm053Rows(d).filter(row => row.run);
      if (!d.example_trip_id || run.length < 2) return null;
      const name = (id: string) => result.name_index.stops?.[id] || id;
      const trips = /^(\d+) trips/.exec(n.observed_value ?? '')?.[1] ?? '1';
      const after = stm053Rows(d).find((row, i, rows) => !row.run && i > 0 && rows[i - 1].run);
      const runParams = {
        first: name(run[0].stop),
        last: name(run[run.length - 1].stop),
        n: localizedNumber(d.run_length ?? String(run.length)),
        time: plannerTime(run[0].arrival) || plannerTime(run[0].departure) || '—',
      };
      // The fix names the stops to retime and the window they fit in: the run's time up to the
      // next stop's time. A run that ends the trip (or is cut at 50 rows) has no next stop.
      const next = after ? plannerTime(after.arrival) || plannerTime(after.departure) : null;
      return {
        title: t('producer.card.STM_053.title'),
        context: tripContextText(tripContext({ ...n, entity_id: d.example_trip_id }, result)),
        what: t('producer.card.STM_053.what_detail', { trips: localizedNumber(trips), ...runParams }),
        action: next
          ? t('producer.card.STM_053.action_detail', { ...runParams, next })
          : t('producer.card.STM_053.action_detail_end', runParams),
      };
    }
    case 'STM_056': {
      const d = n.details ?? {};
      const stopName = (id?: string) => id ? result.name_index.stops?.[id] || id : '';
      const context = tripContext(n, result);
      // A route line drawn backwards (SHP_016 on this trip's shape) is the usual cause: distances
      // measured along a reversed line cannot grow. Said only when the engine found it.
      const shapeId = n.entity_id ? result.name_index.trip_shapes?.[n.entity_id] : undefined;
      const reversed = Boolean(shapeId && reversedShapes(result).has(shapeId));
      const reversedText = reversed ? ` ${t('producer.card.STM_056.what_reversed')}` : '';
      const constantTrips = Number(d.constant_trips);
      const feed = constantTrips > 1
        ? ` ${t('producer.card.STM_056.what_constant_feed', { n: localizedNumber(String(constantTrips)) })}`
        : '';
      // Every stop of the trip has one value: the trip, not a stop pair, is what to fix.
      if (d.trip_constant_value) {
        return {
          title: t('producer.card.STM_056.title'),
          context: tripContextText(context),
          what: t('producer.card.STM_056.what_constant', {
            stops: d.trip_stop_count ?? '—',
            value: d.trip_constant_value,
          }) + reversedText + feed,
          action: t(reversed ? 'producer.card.STM_056.action_reversed' : 'producer.card.STM_056.action_constant'),
        };
      }
      // Only the neighbours that exist: the first stop has no stop before it.
      const order = [d.prev2_stop_id, d.prev_stop_id, d.stop_id, d.next_stop_id, d.next2_stop_id]
        .map(stopName)
        .filter(Boolean)
        .join(' → ')
        // The template ends the list with its own full stop; a name ending in "." doubled it.
        .replace(/\.+$/, '');
      return {
        title: t('producer.card.STM_056.title'),
        context: tripContextText(context),
        what: t('producer.card.STM_056.what_detail', {
          stops: order || '—',
          previous: stopName(d.prev_stop_id) || '—',
          current: stopName(d.stop_id) || '—',
          previous_distance: d.prev_shape_dist_traveled || '—',
          current_distance: d.shape_dist_traveled || n.observed_value || '—',
        }) + reversedText,
        action: t(reversed ? 'producer.card.STM_056.action_reversed' : 'producer.card.STM_056.action_detail'),
      };
    }
    case 'DQ_005': {
      // The last day any calendar runs and the calendar it belongs to; none when no calendar has a day.
      const d = n.details ?? {};
      if (!d.service_count) return null;
      const services = localizedNumber(d.service_count);
      return {
        title: t('producer.card.DQ_005.title'),
        what: d.last_active_date
          ? t('producer.card.DQ_005.what_detail', {
            services, date: ymdText(d.last_active_date), service: d.last_service_id || '—', today: ymdText(d.today),
          })
          : t('producer.card.DQ_005.what_none', { services }),
        action: t('producer.card.DQ_005.action'),
      };
    }
    case 'DQ_005b':
    case 'DQ_009': {
      const d = n.details ?? {};
      if (!d.trip_count) return null;
      return {
        title: t(`producer.card.${g.ruleId}.title`),
        what: t(`producer.card.${g.ruleId}.what_detail`, { n: localizedNumber(d.trip_count) }),
        action: t(`producer.card.${g.ruleId}.action`),
      };
    }
    case 'DQ_005c': {
      const d = n.details ?? {};
      if (!d.missing_count || !d.stop_count) return null;
      return {
        title: t('producer.card.DQ_005c.title'),
        what: t('producer.card.DQ_005c.what_detail', { n: localizedNumber(d.missing_count), total: localizedNumber(d.stop_count) }),
        action: t('producer.card.DQ_005c.action'),
      };
    }
    case 'CAL_001': case 'CAL_002': case 'CAL_003': case 'CAL_004': case 'CAL_005': case 'CAL_006':
    case 'CAL_007': case 'CAL_008': case 'CAL_009': case 'CAL_010': case 'CAL_011': case 'CAL_012': case 'CAL_013':
    case 'CAL_014': case 'CAL_015': case 'CAL_016': case 'CAL_017': case 'CAL_018': case 'CAL_020': case 'CAL_021':
    case 'CAL_022': case 'CAL_023': case 'CAL_024': case 'CAL_025': {
      const what = calRuleText(n, result);
      if (!what) return null;
      return {
        title: t(`producer.card.${g.ruleId}.title`),
        // CAL_011 has no trips; CAL_020/022 concern no calendar's trips.
        what: ['CAL_011', 'CAL_020', 'CAL_022'].includes(g.ruleId) ? what : calWhat(n, result, what),
        // CAL_020: set the window to the period the calendars run.
        action: g.ruleId === 'CAL_020' && n.details?.service_first && n.details?.service_last
          ? t(cal020LongCalendars(n.details) ? 'producer.card.CAL_020.action_long' : 'producer.card.CAL_020.action_detail', {
            first: ymdText(n.details.service_first), last: ymdText(n.details.service_last), svc: n.details.service_last_calendar || '—',
          })
          : t(`producer.card.${g.ruleId}.action`),
      };
    }
    case 'OPR_001': case 'OPR_003': case 'OPR_004': case 'OPR_005': case 'OPR_006': case 'OPR_007': case 'OPR_008':
    case 'OPR_009': case 'OPR_010': case 'OPR_011': case 'OPR_012': case 'OPR_013': case 'OPR_014': case 'OPR_015':
    case 'OPR_016': case 'OPR_017': case 'OPR_019': case 'OPR_020': case 'OPR_021': case 'OPR_022': case 'OPR_023':
    case 'OPR_024': case 'OPR_025': {
      const what = oprRuleText(n, result);
      if (!what) return null;
      // Calendar findings also name their trips and routes; the others the trip the map draws.
      const extra = CAL_USAGE_RULES.has(g.ruleId)
        ? calWhat(n, result, '').trim()
        : n.entity_type === 'Trip' ? '' : dqTripMapText(dqExampleTrip(n, result.name_index), result);
      return {
        title: t(`producer.card.${g.ruleId}.title`),
        what: [what, extra].filter(Boolean).join(' '),
        action: t(`producer.card.${g.ruleId}.action`),
      };
    }
    case 'RTS_001': case 'RTS_002': case 'RTS_003': case 'RTS_004': case 'RTS_005': case 'RTS_006': case 'RTS_007':
    case 'RTS_008': case 'RTS_010': case 'RTS_011': case 'RTS_012': case 'RTS_013': case 'RTS_016': case 'RTS_017':
    case 'RTS_018': case 'RTS_019': case 'RTS_020': case 'RTS_021': case 'RTS_022': case 'RTS_023': case 'RTS_024':
    case 'RTS_025': case 'RTS_026': case 'RTS_027': case 'RTS_028': case 'RTS_029': case 'RTS_030': case 'RTS_031': {
      const what = rtsRuleText(n, result);
      if (!what) return null;
      const mapText = dqTripMapText(dqExampleTrip(n, result.name_index), result);
      return { title: t(`producer.card.${g.ruleId}.title`), what: [what, mapText].filter(Boolean).join(' '), action: t(`producer.card.${g.ruleId}.action`) };
    }
    case 'SHP_001': case 'SHP_002': case 'SHP_003': case 'SHP_004': case 'SHP_005': case 'SHP_006': case 'SHP_008':
    case 'SHP_009': case 'SHP_010': case 'SHP_012': case 'SHP_014': case 'SHP_015': case 'SHP_016': case 'SHP_017':
    case 'SHP_018': case 'SHP_019': case 'SHP_020': case 'SHP_021': case 'SHP_022': case 'SHP_023': case 'SHP_024':
    case 'SHP_025': case 'SHP_026': case 'SHP_028': case 'SHP_029': case 'SHP_030': {
      const raw = shpRuleText(n, result);
      if (!raw) return null;
      // Sentences may open with the shape phrase ("the shape … of route 10"): capitalise.
      const what = raw.charAt(0).toLocaleUpperCase(intlLocale()) + raw.slice(1);
      // The shape's routes and the example trip the map draws its stops from.
      const trip = shpContext(n).at('example_trip_id');
      const mapText = trip ? dqTripMapText(dqIndexKey(result.name_index.trip_stops, trip), result) : '';
      return { title: t(`producer.card.${g.ruleId}.title`), what: [what, mapText].filter(Boolean).join(' '), action: t(`producer.card.${g.ruleId}.action`) };
    }
    case 'GEO_002': case 'GEO_006': case 'GEO_007': case 'GEO_009': case 'GEO_012': case 'GEO_013': case 'GEO_014':
    case 'GEO_015': case 'GEO_016': case 'GEO_017': case 'GEO_018': case 'GEO_019': case 'GEO_020': case 'GEO_021':
    case 'GEO_022': {
      const raw = geoRuleText(n, result);
      if (!raw) return null;
      const what = raw.charAt(0).toLocaleUpperCase(intlLocale()) + raw.slice(1);
      // Drawing findings: the example trip whose stops the map numbers.
      const trip = GEO_SHAPE_RULES.has(g.ruleId) ? shpContext(n).at('example_trip_id') : undefined;
      const mapText = trip ? dqTripMapText(dqIndexKey(result.name_index.trip_stops, trip), result) : '';
      return { title: t(`producer.card.${g.ruleId}.title`), what: [what, mapText].filter(Boolean).join(' '), action: t(`producer.card.${g.ruleId}.action`) };
    }
    case 'VAT_001': case 'VAT_002': case 'VAT_003': case 'VAT_005': case 'VAT_006': case 'VAT_007': case 'VAT_008': {
      const what = vatRuleText(n, result);
      if (!what) return null;
      const d = n.details ?? {};
      // VAT_002/007: the two stops a transfer could link, by name.
      const action = (g.ruleId === 'VAT_002' || g.ruleId === 'VAT_007') && d.nearby_stop_id && n.entity_id
        ? t('producer.card.vat_transfer_action', {
          stop: result.name_index.stops?.[n.entity_id] || n.entity_id, near: result.name_index.stops?.[d.nearby_stop_id] || d.nearby_stop_id,
        })
        : t(`producer.card.${g.ruleId}.action`);
      return { title: t(`producer.card.${g.ruleId}.title`), what, action };
    }
    case 'TRP_001': case 'TRP_002': case 'TRP_003': case 'TRP_004': case 'TRP_005': case 'TRP_006': case 'TRP_007':
    case 'TRP_011': case 'TRP_012': case 'TRP_013': case 'TRP_014': case 'TRP_015': case 'TRP_017': case 'TRP_019':
    case 'TRP_020': case 'TRP_021': case 'TRP_022': case 'TRP_023': case 'TRP_024': case 'TRP_025': case 'TRP_026':
    case 'TRP_028': case 'TRP_029': case 'TRP_031': case 'TRP_032': case 'TRP_034': case 'TRP_035': {
      const what = trpRuleText(n, result);
      if (!what) return null;
      // Summaries and route findings also say which trip the map draws; trip findings are that trip.
      // TRP_023/026 also name the trips and routes on the calendar(s).
      const usage = g.ruleId === 'TRP_023' || g.ruleId === 'TRP_026' ? calUsageText(n) : '';
      const mapText = n.entity_type === 'Trip' && n.entity_id && g.ruleId !== 'TRP_024' ? '' : dqTripMapText(dqExampleTrip(n, result.name_index), result);
      const extra = [usage, mapText].filter(Boolean).join(' ');
      // TRP_011: the trip's last stop as a headsign the planner can start from.
      const stops = g.ruleId === 'TRP_011' && n.entity_id ? result.name_index.trip_stops?.[n.entity_id] ?? [] : [];
      const last = stops.length ? result.name_index.stops?.[stops[stops.length - 1]] : undefined;
      return {
        title: t(`producer.card.${g.ruleId}.title`),
        what: [what, extra].filter(Boolean).join(' '),
        action: last ? t('producer.card.TRP_011.action_detail', { stop: last }) : t(`producer.card.${g.ruleId}.action`),
      };
    }
    case 'DQ_003':
    case 'DQ_004': {
      // The route by name, and the example trip the map draws (first and last stop).
      const routeId = routeIdOfNotice(n);
      if (!routeId) return null;
      return {
        title: t(`producer.card.${g.ruleId}.title`),
        what: [
          t(`producer.card.${g.ruleId}.what_detail`, { route: result.name_index.routes?.[routeId] || routeId }),
          dqTripMapText(dqExampleTrip(n, result.name_index), result),
        ].filter(Boolean).join(' '),
        action: t(`producer.card.${g.ruleId}.action`),
      };
    }
    case 'DQ_018':
    case 'DQ_019': {
      // Which name it is (stop, route, route description, trip or stop headsign), whose, and the value.
      const field = n.field ?? '';
      if (!['stop_name', 'route_long_name', 'route_desc', 'trip_headsign', 'stop_headsign'].includes(field) || n.observed_value == null) return null;
      const ni = result.name_index;
      const d = n.details ?? {};
      const routeId = routeIdOfNotice(n);
      const stopId = n.entity_type === 'Stop' ? n.entity_id : d.stop_id;
      const what = t(`producer.card.${g.ruleId}.what_${field}`, {
        value: n.observed_value,
        stop_id: stopId || '—',
        stop: stopId ? (ni.stops?.[stopId] ? `${ni.stops[stopId]} (${stopId})` : stopId) : '—',
        route: routeId ? ni.routes?.[routeId] || routeId : '—',
        trip: tripPhrase(n.entity_type === 'Trip' ? n.entity_id : d.trip_id, result, 'gen'),
      });
      return {
        title: t(`producer.card.${g.ruleId}.title`),
        what: [what, n.entity_type === 'Route' ? dqTripMapText(dqExampleTrip(n, ni), result) : ''].filter(Boolean).join(' '),
        action: t(`producer.card.${g.ruleId}.action`),
      };
    }
    case 'DQ_006': {
      const d = n.details ?? {};
      if (!d.shapeless_trips || !d.fixed_trips) return null;
      return {
        title: t('producer.card.DQ_006.title'),
        what: [
          t('producer.card.DQ_006.what_detail', {
            n: localizedNumber(d.shapeless_trips), total: localizedNumber(d.fixed_trips), routes: localizedNumber(d.route_count || '0'),
          }),
          dqTripMapText(d.example_trip_id, result),
        ].filter(Boolean).join(' '),
        action: t('producer.card.DQ_006.action'),
      };
    }
    case 'DQ_010': {
      const d = n.details ?? {};
      if (!d.route_count) return null;
      return {
        title: t('producer.card.DQ_010.title'),
        what: t('producer.card.DQ_010.what_detail', {
          agency: d.agency_name || n.entity_id || '—', routes: localizedNumber(d.route_count),
        }),
        action: t('producer.card.DQ_010.action'),
      };
    }
    case 'DQ_011': {
      const d = n.details ?? {};
      if (!d.stop_id) return null;
      const name = d.stop_name || result.name_index.stops?.[d.stop_id];
      return {
        title: t('producer.card.DQ_011.title'),
        what: t('producer.card.DQ_011.what_detail', { stop: name ? `${name} (${d.stop_id})` : d.stop_id }),
        action: t('producer.card.DQ_011.action'),
      };
    }
    case 'DQ_012': {
      const d = n.details ?? {};
      if (!d.agency_count) return null;
      return {
        title: t('producer.card.DQ_012.title'),
        what: t('producer.card.DQ_012.what_detail', { agencies: localizedNumber(d.agency_count), routes: localizedNumber(d.route_count || '0') }),
        action: t('producer.card.DQ_012.action'),
      };
    }
    case 'DQ_013': {
      const d = n.details ?? {};
      if (!d.trip_count) return null;
      return {
        title: t('producer.card.DQ_013.title'),
        what: t('producer.card.DQ_013.what_detail', { n: localizedNumber(d.trip_count) }),
        action: t('producer.card.DQ_013.action'),
      };
    }
    case 'DQ_016': {
      const d = n.details ?? {};
      if (!d.affected_rows || !n.file) return null;
      return {
        title: t('producer.card.DQ_016.title'),
        // The list in the planner's words; spaces in code fields break links to other lists.
        what: [
          plannerListName(n.file)
            ? t('producer.card.DQ_016.what_list', { list: plannerListName(n.file)!, n: localizedNumber(d.affected_rows) })
            : t('producer.card.DQ_016.what'),
          d.identity_whitespace_fields
            ? t('producer.card.DQ_016.what_identity', { fields: listOf(d.identity_whitespace_fields).map(plannerFieldName).join(', ') })
            : '',
        ].filter(Boolean).join(' '),
        action: t('producer.card.DQ_016.action'),
      };
    }
    case 'DQ_017': {
      const count = n.observed_value;
      if (!count || !n.details?.example_0_stop_id) return null;
      return {
        title: t('producer.card.DQ_017.title'),
        what: t('producer.card.DQ_017.what_detail', { n: localizedNumber(count) }),
        action: t('producer.card.DQ_017.action'),
      };
    }
    case 'DQ_020': {
      // The counts, and the example trip's last stop as a headsign the planner can start from.
      const d = n.details ?? {};
      if (!d.missing_trips || !d.trip_count) return null;
      const tripStops = d.example_trip_id ? result.name_index.trip_stops?.[d.example_trip_id] ?? [] : [];
      const lastStop = tripStops[tripStops.length - 1];
      const last = lastStop ? result.name_index.stops?.[lastStop] : undefined;
      return {
        title: t('producer.card.DQ_020.title'),
        what: [
          t('producer.card.DQ_020.what_detail', {
            n: localizedNumber(d.missing_trips), total: localizedNumber(d.trip_count), routes: localizedNumber(d.route_count || '0'),
          }),
          dqTripMapText(d.example_trip_id, result),
        ].filter(Boolean).join(' '),
        action: last
          ? t('producer.card.DQ_020.action_detail', { trip: tripPhrase(d.example_trip_id, result), stop: last })
          : t('producer.card.DQ_020.action'),
      };
    }
    case 'DQ_021': {
      const d = n.details ?? {};
      if (!d.affected_duplicates || !n.file) return null;
      return {
        title: t('producer.card.DQ_021.title'),
        // Said in the planner's words per kind of record; an unknown file keeps the general text.
        what: DQ021_KIND[n.file]
          ? t(`producer.card.DQ_021.what_${DQ021_KIND[n.file]}`, { n: localizedNumber(d.affected_duplicates) })
          : t('producer.card.DQ_021.what'),
        action: !DQ021_KIND[n.file] ? t('producer.card.DQ_021.action')
          : t(DQ021_CODE_KINDS.has(DQ021_KIND[n.file]) ? 'producer.card.DQ_021.action_code' : 'producer.card.DQ_021.action_repeat'),
      };
    }
    case 'DQ_022': {
      const d = n.details ?? {};
      if (!d.common_name || !d.common_count || !d.named_count) return null;
      return {
        title: t('producer.card.DQ_022.title'),
        what: t('producer.card.DQ_022.what_detail', {
          n: localizedNumber(d.common_count), total: localizedNumber(d.named_count), name: d.common_name,
        }),
        action: t('producer.card.DQ_022.action'),
      };
    }
    default: {
  // Rules that need no special context: from producer.card.<RULE>.* keys in the text dictionary.
      const base = `producer.card.${g.ruleId}`;
      const title = t(`${base}.title`);
      if (title === `${base}.title`) return null;
      return { title, what: t(`${base}.what`), action: t(`${base}.action`), generic: true };
    }
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
      ? routeIdOfNotice(n)
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
  // 2026-01-05 was a Monday; the mask is ordered Mon..Sun.
    text = [...mask].map((on, i) => on === '1' ? fmt.format(new Date(Date.UTC(2026, 0, 5 + i))) : '').filter(Boolean).join(', ');
  }
  return Number(exceptions) > 0 ? `${text} ${t('producer.days.exceptions', { n: exceptions })}` : text;
}

/** The calendar's validity range ("29.03.2026 – 31.03.2026"), or '—' when unknown. */
function serviceRangeText(serviceId: string | null | undefined, result: ValidationResult): string {
  const entry = serviceId ? result.name_index.service_days?.[serviceId] : undefined;
  if (!entry || (!entry[1] && !entry[2])) return '—';
  const ymd = (v: string) => (/^\d{8}$/.test(v)
    ? new Intl.DateTimeFormat(intlLocale(), { day: '2-digit', month: '2-digit', year: 'numeric', timeZone: 'UTC' })
      .format(new Date(Date.UTC(Number(v.slice(0, 4)), Number(v.slice(4, 6)) - 1, Number(v.slice(6, 8)))))
    : v);
  return `${ymd(entry[1])} – ${ymd(entry[2])}`;
}

function plannerServiceText(serviceId: string | null | undefined, result: ValidationResult): string {
  const text = serviceText(serviceId, result);
  return serviceId && text === serviceId ? t('producer.days.check') : text;
}

/** A trip as planners know it — route number, headsign, first departure — never its trip_id. */
function tripParts(tripId: string, result: ValidationResult): { route?: string; headsign?: string; dep?: string } {
  const ni = result.name_index;
  const routeId = ni.trip_routes?.[tripId];
  return {
    route: routeId ? routeBadge(routeId, ni).label : undefined,
    headsign: ni.trips?.[tripId] || undefined,
    dep: ni.trip_first_dep?.[tripId] || undefined,
  };
}

/** A notice's record for the planner: trips as "S8 · Liman yönü · 06:00", others by their name. */
function plannerEntityLabel(n: Notice, result: ValidationResult): string | null {
  return n.entity_type === 'Trip' && n.entity_id ? tripShort(n.entity_id, result) : entityLabel(n, result.name_index);
}

/** "S8 · Liman yönü · 06:00" for tables and the map line; the trip_id only when nothing else is known. */
function tripShort(tripId: string | undefined | null, result: ValidationResult, withRoute = true): string {
  if (!tripId) return '—';
  const p = tripParts(tripId, result);
  const parts = [withRoute ? p.route : undefined, p.headsign ? t('producer.trip.dir', { headsign: p.headsign }) : undefined, p.dep]
    .filter(Boolean);
  return parts.length ? parts.join(' · ') : tripId;
}

/** "S8 hattının „Liman“ yönündeki 06:00 kalkışlı seferi" for sentences (`gen`: "…seferinin"). */
function tripPhrase(tripId: string | undefined | null, result: ValidationResult, form: 'subj' | 'gen' = 'subj', withRoute = true): string {
  if (!tripId) return '—';
  const p = tripParts(tripId, result);
  if (!withRoute) p.route = undefined;
  const suffix = form === 'gen' ? '_gen' : '';
  if (!p.route && !p.headsign && !p.dep) return t(`producer.trip.code${suffix}`, { trip: tripId });
  const dir = p.headsign ? t('producer.trip.dir_part', { headsign: p.headsign }) : '';
  const dep = p.dep ? t('producer.trip.dep_part', { dep: p.dep }) : '';
  return t(`producer.trip.${p.route ? 'phrase' : 'phrase_noroute'}${suffix}`, { route: p.route ?? '', dir, dep }).trim();
}

/** A route's example trip for the planner: the first trip (by trip code) with a drawn line, else
 *  the first with two or more stops. Undefined when the name index carries no trip of the route
 *  (large feeds keep only the trips a finding names). */
function routeExampleTrip(ni: ValidationResult['name_index'], routeId?: string | null): string | undefined {
  if (!routeId) return undefined;
  let fallback: string | undefined;
  for (const [tripId, tripRoute] of Object.entries(ni.trip_routes ?? {})) {
    if (tripRoute !== routeId || (ni.trip_stops?.[tripId]?.length ?? 0) < 2) continue;
    if (ni.trip_shapes?.[tripId]) return tripId;
    fallback ??= tripId;
  }
  return fallback;
}

/** DQ_016 samples: the column, the raw value, its line and the record it sits on (the file's
 *  key column value, raw — a key with spaces is looked up trimmed too). */
function dq016Samples(d: Record<string, string>): Array<{ field: string; value: string; line?: string; record?: string }> {
  const out = [];
  for (let i = 0; d[`sample_${i}_field`]; i++) {
    out.push({ field: d[`sample_${i}_field`]!, value: d[`sample_${i}_value`] ?? '', line: d[`sample_${i}_line`], record: d[`sample_${i}_record`] });
  }
  return out;
}
function dqIndexKey<T>(map: Record<string, T> | undefined, key?: string): string | undefined {
  if (!key || !map) return undefined;
  return key in map ? key : key.trim() in map ? key.trim() : undefined;
}

/** Route findings whose cards draw the route's lines and an example trip with numbered stops. */
const RTS_CONTEXT_RULES = new Set(['RTS_001', 'RTS_002', 'RTS_003', 'RTS_004', 'RTS_005', 'RTS_006', 'RTS_007', 'RTS_008', 'RTS_010', 'RTS_011',
  'RTS_012', 'RTS_013', 'RTS_016', 'RTS_017', 'RTS_018', 'RTS_019', 'RTS_020', 'RTS_021', 'RTS_022', 'RTS_023',
  'RTS_024', 'RTS_025', 'RTS_026', 'RTS_027', 'RTS_028', 'RTS_029', 'RTS_030']);

/** Shape findings whose cards draw the shape with its routes, an example trip and the faulty spot. */
const SHP_CONTEXT_RULES = new Set(['SHP_001', 'SHP_002', 'SHP_003', 'SHP_004', 'SHP_005', 'SHP_006', 'SHP_008', 'SHP_009', 'SHP_010', 'SHP_012',
  'SHP_014', 'SHP_015', 'SHP_016', 'SHP_017', 'SHP_018', 'SHP_019', 'SHP_020', 'SHP_021', 'SHP_022', 'SHP_023',
  'SHP_024', 'SHP_025', 'SHP_026', 'SHP_028', 'SHP_029', 'SHP_030']);

/** Stop findings (STP_001–010) whose cards place the stop on an example trip. */
const STP_CONTEXT_RULES = new Set(['STP_001', 'STP_002', 'STP_003', 'STP_004', 'STP_005', 'STP_006', 'STP_007', 'STP_008', 'STP_009', 'STP_010',
  'STP_011', 'STP_012', 'STP_013', 'STP_014', 'STP_015', 'STP_016', 'STP_017', 'STP_019', 'STP_020',
  'STP_021', 'STP_022', 'STP_023', 'STP_024', 'STP_026', 'STP_028', 'STP_029', 'STP_030', 'STP_031', 'STP_032',
  'STP_033', 'STP_034', 'STP_035', 'STP_036', 'STP_037', 'STP_038', 'STP_039', 'STP_040', 'STP_041', 'STP_042', 'STP_043', 'STP_044', 'STP_045']);

/** Rule groups whose cards draw one example trip with numbered stops and list it stop by stop. */
function plannerTripRule(ruleId: string): boolean {
  return ruleId.startsWith('DQ_') || ruleId.startsWith('CAL_') || ruleId.startsWith('OPR_') || ruleId.startsWith('TRP_') || ruleId === 'VAT_003' || RTS_CONTEXT_RULES.has(ruleId);
}

/** CAL findings (but CAL_020/022) carry the trips and routes on their calendar(s); the map draws the example trip. */
const CAL_USAGE_RULES = new Set(['CAL_001', 'CAL_002', 'CAL_003', 'CAL_004', 'CAL_005', 'CAL_006', 'CAL_007',
  'CAL_008', 'CAL_009', 'CAL_010', 'CAL_011', 'CAL_012', 'CAL_013', 'CAL_014', 'CAL_015', 'CAL_016', 'CAL_017',
  'CAL_018', 'CAL_021', 'CAL_023', 'CAL_024', 'CAL_025', 'OPR_011', 'OPR_012', 'OPR_016']);

/** The trip a DQ card draws on its map and lists stop by stop: the route's example trip for a route
 *  record, the record's own trip for a headsign row, the example trip of a feed summary. */
function dqExampleTrip(n: Notice, ni: ValidationResult['name_index']): string | undefined {
  const d = n.details ?? {};
  switch (n.rule_id) {
    case 'DQ_003':
    case 'DQ_004':
      return n.entity_type === 'Route' ? routeExampleTrip(ni, routeIdOfNotice(n)) : undefined;
    case 'RTS_012':
      // No trips of its own: the trip of a route carrying the same number, if any.
      return n.entity_type === 'Route' ? routeExampleTrip(ni, listOf(d.same_name_routes)[0]) : undefined;
    case 'RTS_001': case 'RTS_002': case 'RTS_003': case 'RTS_004': case 'RTS_005':
    case 'RTS_006': case 'RTS_007': case 'RTS_008': case 'RTS_010': case 'RTS_011': case 'RTS_013': case 'RTS_016':
    case 'RTS_017': case 'RTS_018': case 'RTS_019': case 'RTS_020': case 'RTS_021': case 'RTS_022': case 'RTS_023':
    case 'RTS_024': case 'RTS_025': case 'RTS_026': case 'RTS_027': case 'RTS_028': case 'RTS_029': case 'RTS_030':
      return n.entity_type === 'Route' ? dqIndexKey(ni.trip_stops, d.example_trip_id) ?? routeExampleTrip(ni, routeIdOfNotice(n)) : undefined;
    case 'DQ_018':
    case 'DQ_019':
      if (n.entity_type === 'Route') return routeExampleTrip(ni, routeIdOfNotice(n));
      if (n.entity_type === 'Trip') return n.entity_id ?? undefined;
      return n.entity_type === 'Row' ? d.trip_id : undefined;
    case 'DQ_021':
      if (n.file === 'routes.txt') return routeExampleTrip(ni, d.example_0_key);
      return n.file === 'trips.txt' ? d.example_0_key : undefined;
    case 'DQ_006':
    case 'DQ_020':
      return n.entity_type !== 'Trip' ? d.example_trip_id : undefined;
    case 'DQ_013':
      return n.entity_type !== 'Trip' ? d.example_0_trip_id : undefined;
    case 'DQ_016': {
      // A trip-keyed list (trips, stop times, frequencies): the first sample's trip; a route list:
      // that route's example trip. Stop lists are pinned stop by stop instead.
      const records = dq016Samples(d).map(sample => sample.record);
      if (d.record_field === 'trip_id') return records.map(r => dqIndexKey(ni.trip_stops, r)).find(Boolean);
      if (d.record_field === 'route_id') {
        return records.map(r => routeExampleTrip(ni, r) || routeExampleTrip(ni, r?.trim())).find(Boolean);
      }
      return undefined;
    }
    // OPR: the trip before the gap (OPR_001), the first of the close pair (OPR_003), the night or
    // shortest trip, an accessible/inaccessible trip; route findings: the route's example trip.
    case 'OPR_001':
    case 'OPR_003':
      return dqIndexKey(ni.trip_stops, d.trip_before) ?? routeExampleTrip(ni, routeIdOfNotice(n));
    case 'OPR_006': case 'OPR_007': case 'OPR_008': case 'OPR_017':
    case 'TRP_001': case 'TRP_002': case 'TRP_006': case 'TRP_007': case 'TRP_011': case 'TRP_014':
    case 'TRP_019': case 'TRP_020': case 'TRP_022': case 'TRP_026': case 'TRP_032': case 'TRP_034': case 'VAT_003':
      // Trip findings: the trip itself (listed stop by stop; the map already draws it).
      return n.entity_type === 'Trip' ? dqIndexKey(ni.trip_stops, n.entity_id ?? undefined) : undefined;
    // TRP summaries: the row's own trip once opened into rows, else the first example.
    case 'TRP_003': case 'TRP_005': case 'TRP_012': case 'TRP_021': case 'TRP_025': case 'TRP_028': case 'TRP_029':
      return n.entity_type === 'Trip'
        ? dqIndexKey(ni.trip_stops, n.entity_id ?? undefined)
        : listOf(d.example_trips).map(id => dqIndexKey(ni.trip_stops, id)).find(Boolean);
    case 'TRP_004':
      return dqIndexKey(ni.trip_stops, d.example_trip);
    case 'TRP_013':
      return dqIndexKey(ni.trip_stops, d.example_trip_id) ?? routeExampleTrip(ni, routeIdOfNotice(n));
    case 'TRP_015':
      return dqIndexKey(ni.trip_stops, d.example_0_trip_id);
    case 'TRP_023':
      return dqIndexKey(ni.trip_stops, d.example_trip_id);
    case 'TRP_024':
      return dqIndexKey(ni.trip_stops, d.first_trip);
    case 'TRP_031':
    case 'TRP_035':
      return dqIndexKey(ni.trip_stops, d.trip_id);
    case 'OPR_009':
    case 'OPR_025':
      return dqIndexKey(ni.trip_stops, d.example_trip_id) ?? (n.entity_type === 'Route' ? routeExampleTrip(ni, routeIdOfNotice(n)) : undefined);
    case 'OPR_010':
      return dqIndexKey(ni.trip_stops, d.no_trip_id) ?? dqIndexKey(ni.trip_stops, d.yes_trip_id) ?? routeExampleTrip(ni, routeIdOfNotice(n));
    case 'OPR_004': case 'OPR_005': case 'OPR_013': case 'OPR_015': case 'OPR_019': case 'OPR_020':
    case 'OPR_021': case 'OPR_022': case 'OPR_023': case 'OPR_024':
      return n.entity_type === 'Route' ? routeExampleTrip(ni, routeIdOfNotice(n)) : undefined;
    default:
      // Calendar findings: a trip running on the calendar(s), when the index has its stops.
      return CAL_USAGE_RULES.has(n.rule_id) && d.example_trip_id && (ni.trip_stops?.[d.example_trip_id]?.length ?? 0) > 1
        ? d.example_trip_id : undefined;
  }
}

/** The calendar(s) a CAL finding is about: its own, a summary's list, or none. */
function calServices(n: Notice): string[] {
  if ((n.entity_type === 'Service' || n.rule_id === 'CAL_025') && n.entity_id) return [n.entity_id];
  return listOf(n.details?.services);
}

/** "Bu takvimle 2 hattın 14 seferi çalışıyor." — from the engine's calendar usage counts. */
function calUsageText(n: Notice): string {
  const d = n.details ?? {};
  if (d.trip_count == null) return '';
  if (d.trip_count === '0') return t('producer.card.cal_usage_none');
  // A summary's calendar row still carries the whole summary's counts.
  const many = n.rule_id === 'CAL_009' || n.rule_id === 'CAL_015' || calServices(n).length > 1 || Number(d.affected_services) > 1;
  return t(many ? 'producer.card.cal_usage_many' : 'producer.card.cal_usage', {
    routes: localizedNumber(d.route_count || '0'), trips: localizedNumber(d.trip_count),
  });
}

/** CAL card text: the rule's own sentence, the trips and routes on the calendar, the map's trip. */
function calWhat(n: Notice, result: ValidationResult, what: string): string {
  return [what, calUsageText(n), dqTripMapText(dqExampleTrip(n, result.name_index), result)].filter(Boolean).join(' ');
}

/** One sentence naming the trip the map draws, its first and last stop and how many stops it has;
 *  empty when fewer than two of its stops have a position (nothing to draw). */
function dqTripMapText(tripId: string | undefined, result: ValidationResult): string {
  const ni = result.name_index;
  const stops = tripId ? ni.trip_stops?.[tripId] ?? [] : [];
  if (!tripId || stops.filter(id => ni.stop_coords?.[id]).length < 2) return '';
  const stopName = (id: string) => ni.stops?.[id] || id;
  return t('producer.card.dq_trip_map', {
    trip: tripShort(tripId, result), from: stopName(stops[0]), to: stopName(stops[stops.length - 1]), n: localizedNumber(String(stops.length)),
  });
}

const ITINERARY_ROWS = 60;
const CAL_SERVICE_ROWS = 10;

/** The trip's stops in order (sequence and stop), the marked stop in red. */
function tripItineraryTable(tripId: string, result: ValidationResult, markStop?: string, markSequence?: string): string {
  const ni = result.name_index;
  const stops = ni.trip_stops?.[tripId] ?? [];
  if (stops.length < 2) return '';
  const sequences = ni.trip_stop_sequences?.[tripId] ?? [];
  const rows = stops.slice(0, ITINERARY_ROWS).map((stopId, index) => {
    const sequence = sequences[index];
    const shown = sequence == null || sequence === 4294967295 ? '—' : String(sequence);
    const bad = markStop === stopId && (!markSequence || markSequence === shown);
    const stop = ni.stops?.[stopId] ? `${ni.stops[stopId]} (${stopId})` : stopId;
    return `<tr${bad ? ' class="bad"' : ''}><td>${escHtml(shown)}</td><td>${escHtml(stop)}</td></tr>`;
  });
  if (stops.length > ITINERARY_ROWS) {
    rows.push(`<tr><td></td><td>${escHtml(t('producer.times.more_stops', { n: localizedNumber(String(stops.length - ITINERARY_ROWS)) }))}</td></tr>`);
  }
  const caption = t('producer.times.trip_itinerary', { trip: tripShort(tripId, result) });
  return `<p class="producer-card-label">${escHtml(caption)}</p><table class="planner-times planner-stop-times"><thead><tr><th>${t('producer.times.sequence')}</th><th>${t('producer.col.stop')}</th></tr></thead><tbody>${rows.join('')}</tbody></table>`;
}

/** The trips behind a finding grouped across trips (STM_021, STM_035): route, first departure,
 *  where the finding sits in each trip, service days and the calendar's validity range. Trip copies
 *  often differ only in that range (one per month), so it is a column of its own. Empty for one trip. */
function tripListTable(d: Record<string, string>, result: ValidationResult, place: (at: (key: string) => string | undefined) => string, captionKey = 'stm021_trips'): string {
  const ni = result.name_index;
  const trips: string[] = [];
  for (let i = 0; d[`example_${i}_trip`]; i++) {
    const at = (key: string) => d[`example_${i}_${key}`];
    const tripId = at('trip')!;
    const routeId = ni.trip_routes?.[tripId];
    const service = at('service_id');
    trips.push(`<tr><td>${routeId ? routeBadgeHtml(routeId, result) : '—'}</td><td>${escHtml(ni.trip_first_dep?.[tripId] || '—')}</td><td>${escHtml(place(at))}</td><td>${escHtml(plannerServiceText(service, result))}</td><td>${escHtml(serviceRangeText(service, result))}</td></tr>`);
  }
  if (trips.length < 2) return '';
  const total = Number(d.affected_trips) || trips.length;
  const caption = total > trips.length
    ? t(`producer.times.${captionKey}_some`, { n: localizedNumber(String(total)), shown: trips.length })
    : t(`producer.times.${captionKey}`, { n: localizedNumber(String(total)) });
  return `<p class="producer-card-label">${escHtml(caption)}</p><table class="planner-times planner-trip-list"><thead><tr><th>${t('producer.col.route')}</th><th>${t('producer.col.departure')}</th><th>${t('producer.times.stm021_order')}</th><th>${t('producer.col.service')}</th><th>${t('producer.times.validity')}</th></tr></thead><tbody>${trips.join('')}</tbody></table>`;
}

/** The segment rules (STM_021/025/026): previous stop, the two stops of the segment (marked) and
 *  the next stop, with sequence, times, coordinates and straight-line distance from the row above. */
type SegmentStopRow = { stop: string; sequence: string; arrival?: string; departure?: string; lat: string; lon: string; distance: string; bad: boolean };
function segmentStopRows(d: Record<string, string>, result: ValidationResult): SegmentStopRow[] {
  const coords = result.name_index.stop_coords ?? {};
  const name = (id: string) => result.name_index.stops?.[id] || id;
  const entries: Array<[string | undefined, string | undefined, string | undefined, string | undefined, boolean]> = [
    [d.prev_stop_id, d.prev_stop_sequence, d.prev_arrival, d.prev_departure, false],
    [d.stop_a, d.stop_a_sequence, d.stop_a_arrival, d.stop_a_departure, true],
    [d.stop_b, d.stop_b_sequence, d.stop_b_arrival, d.stop_b_departure, true],
    [d.next_stop_id, d.next_stop_sequence, d.next_arrival, d.next_departure, false],
  ];
  let previous: [number, number] | undefined;
  return entries.filter(([id]) => id).map(([id, sequence, arrival, departure, bad]) => {
    const p = coords[id!];
    const here: [number, number] | undefined = p ? [p[0], p[1]] : undefined;
    const distance = here && previous ? localizedNumber(String(Math.round(metersBetween(previous, here)))) : '—';
    previous = here;
    return { stop: name(id!), sequence: sequence || '—', arrival, departure, lat: coordText(here?.[0]), lon: coordText(here?.[1]), distance, bad };
  });
}

function segmentCoordTable(rows: SegmentStopRow[], cls: string): string {
  return `<table class="planner-times ${cls}"><thead><tr><th>${t('producer.col.stop')}</th><th>${t('producer.times.sequence')}</th><th>${t('producer.times.latitude')}</th><th>${t('producer.times.longitude')}</th><th>${t('producer.times.distance_from_previous')}</th></tr></thead><tbody>${
    rows.map(r => `<tr${r.bad ? ' class="bad"' : ''}><td>${escHtml(r.stop)}</td><td>${escHtml(r.sequence)}</td><td>${escHtml(r.lat)}</td><td>${escHtml(r.lon)}</td><td>${escHtml(r.distance)}</td></tr>`).join('')
  }</tbody></table>`;
}

/** Saat kurallarında seçili seferin ilgili durakları sırayla, saatleriyle; hatalı satır işaretli. */
function timeStrip(n: Notice, result: ValidationResult): string {
  const body = timeStripBody(n, result);
  // DQ cards: the stops of the trip on the map, in order (the headsign row's stop, or the
  // last stop a missing headsign could name, in red).
  // SHP cards: the example trip on the shape, stop by stop (SHP_012: its first far stop red).
  if (SHP_CONTEXT_RULES.has(n.rule_id)) {
    // SHP_017 lists its own trip (the stop out of order red); SHP_014/016 the trip's problem stop.
    const d = n.details ?? {};
    const own = n.entity_type === 'Trip' ? n.entity_id ?? undefined : ['SHP_014', 'SHP_016', 'SHP_024'].includes(n.rule_id) ? d.trip_id : undefined;
    const trip = dqIndexKey(result.name_index.trip_stops, own || shpContext(n).at('example_trip_id'));
    const marked = [...listOf(d.far_stops), d.bad_stop, d.problem_stop, d.first_stop, n.rule_id === 'SHP_024' ? n.entity_id : undefined]
      .filter((v): v is string => Boolean(v));
    const mark = (trip ? result.name_index.trip_stops?.[trip] ?? [] : []).find(id => marked.includes(id));
    return trip ? body + tripItineraryTable(trip, result, mark) : body;
  }
  // GEO cards: the stop's example trip (the stop red), a drawing's example trip.
  if (GEO_STOP_RULES.has(n.rule_id) || GEO_SHAPE_RULES.has(n.rule_id)) {
    const c = stpContext(n);
    const shapeTrip = GEO_SHAPE_RULES.has(n.rule_id) ? shpContext(n).at('example_trip_id') : undefined;
    const trip = dqIndexKey(result.name_index.trip_stops, shapeTrip ?? c.trip);
    return trip ? body + tripItineraryTable(trip, result, shapeTrip ? undefined : c.stop, shapeTrip ? undefined : c.seq) : body;
  }
  // STP cards: the example trip through the stop, the stop red in its stop list.
  if (STP_CONTEXT_RULES.has(n.rule_id)) {
    const c = stpContext(n, n.rule_id === 'STP_020' ? 'nearest' : undefined);
    const trip = dqIndexKey(result.name_index.trip_stops, c.trip);
    return trip ? body + tripItineraryTable(trip, result, c.stop, c.seq) : body;
  }
  const tripId = plannerTripRule(n.rule_id) ? dqExampleTrip(n, result.name_index) : undefined;
  if (!tripId) return body;
  const stops = result.name_index.trip_stops?.[tripId] ?? [];
  const lastStop = n.rule_id === 'DQ_020' || n.rule_id === 'TRP_011';
  const mark = lastStop ? stops[stops.length - 1] : n.rule_id === 'OPR_007' ? n.details?.dup_stop : n.details?.stop_id;
  return body + tripItineraryTable(tripId, result, mark, lastStop ? undefined : n.details?.stop_sequence);
}

function timeStripBody(n: Notice, result: ValidationResult): string {
  const d = n.details;
  if (!d) return '';
  const name = (id?: string, fallback?: string) => (id ? result.name_index.stops?.[id] : undefined) || fallback || id || '—';
  if ((n.rule_id === 'STM_003' || n.rule_id === 'STM_004') && d.stop_id) {
    // Previous / faulty / next row with stop sequence (same table as STM_045/048/049). The engine
    // represents a malformed time as "?" and an empty field as ""; the faulty row's own value is observed.
    const time = (v?: string) => v === '?' ? t('producer.times.invalid') : v || '—';
    const raw = n.observed_value ?? '?';
    type MalformedRow = [string, string, string, string, boolean];
    const rows: MalformedRow[] = [
      ...(d.prev_stop_id ? [[d.prev_stop_sequence || '—', name(d.prev_stop_id), time(d.prev_arrival), time(d.prev_departure), false] as MalformedRow] : []),
      [d.stop_sequence || '—', name(d.stop_id), n.rule_id === 'STM_003' ? raw : time(d.arrival), n.rule_id === 'STM_004' ? raw : time(d.departure), true],
      ...(d.next_stop_id ? [[d.next_stop_sequence || '—', name(d.next_stop_id), time(d.next_arrival), time(d.next_departure), false] as MalformedRow] : []),
    ];
    return `<table class="planner-times planner-neighbour-times"><thead><tr><th>${t('producer.times.sequence')}</th><th>${t('producer.col.stop')}</th><th>${t('producer.times.arrival')}</th><th>${t('producer.times.departure')}</th></tr></thead><tbody>${
      rows.map(([sequence, stop, arrival, departure, bad]) => `<tr${bad ? ' class="bad"' : ''}><td>${escHtml(sequence)}</td><td>${escHtml(stop)}</td><td>${escHtml(arrival)}</td><td>${escHtml(departure)}</td></tr>`).join('')
    }</tbody></table>`;
  }
  if (['STM_045', 'STM_048', 'STM_049'].includes(n.rule_id) && d.stop_id && (d.arrival || d.departure)) {
    // Previous / faulty / next row, as written in the feed. STM_048/049: the faulty time
    // (arrival for 048, departure for 049) also shows the value it should have.
    const fix = (value: string | undefined, side: 'arrival_time' | 'departure_time') => {
      const shown = plannerTime(value) || '—';
      return (n.rule_id === 'STM_048' || n.rule_id === 'STM_049') && n.field === side && d.expected_time
        ? `${shown} → ${plannerTime(d.expected_time)}`
        : shown;
    };
    type NeighbourRow = [string, string, string, string, boolean];
    const rows: NeighbourRow[] = [
      ...(d.prev_stop_id ? [[d.prev_stop_sequence || '—', name(d.prev_stop_id), plannerTime(d.prev_arrival) || '—', plannerTime(d.prev_departure) || '—', false] as NeighbourRow] : []),
      [d.stop_sequence || '—', name(d.stop_id), fix(d.arrival, 'arrival_time'), fix(d.departure, 'departure_time'), true],
      ...(d.next_stop_id ? [[d.next_stop_sequence || '—', name(d.next_stop_id), plannerTime(d.next_arrival) || '—', plannerTime(d.next_departure) || '—', false] as NeighbourRow] : []),
    ];
    return `<table class="planner-times planner-neighbour-times"><thead><tr><th>${t('producer.times.sequence')}</th><th>${t('producer.col.stop')}</th><th>${t('producer.times.arrival')}</th><th>${t('producer.times.departure')}</th></tr></thead><tbody>${
      rows.map(([sequence, stop, arrival, departure, bad]) => `<tr${bad ? ' class="bad"' : ''}><td>${escHtml(sequence)}</td><td>${escHtml(stop)}</td><td>${escHtml(arrival)}</td><td>${escHtml(departure)}</td></tr>`).join('')
    }</tbody></table>`;
  }
  if (n.rule_id === 'STM_056') {
    type DistanceRow = [string, string, string, string, string, boolean];
    const rows: DistanceRow[] = [];
    if (d.prev2_stop_id) rows.push([
      name(d.prev2_stop_id), d.prev2_stop_sequence || '—', d.prev2_arrival || '—',
      d.prev2_departure || '—', d.prev2_shape_dist_traveled || '—', false,
    ]);
    if (d.prev_stop_id) rows.push([
      name(d.prev_stop_id), d.prev_stop_sequence || '—', d.prev_arrival || '—',
      d.prev_departure || '—', d.prev_shape_dist_traveled || '—', false,
    ]);
    rows.push([
      name(d.stop_id), d.stop_sequence || '—', d.arrival || '—', d.departure || '—',
      d.shape_dist_traveled || n.observed_value || '—', true,
    ]);
    if (d.next_stop_id) rows.push([
      name(d.next_stop_id), d.next_stop_sequence || '—', d.next_arrival || '—',
      d.next_departure || '—', d.next_shape_dist_traveled || '—', false,
    ]);
    if (d.next2_stop_id) rows.push([
      name(d.next2_stop_id), d.next2_stop_sequence || '—', d.next2_arrival || '—',
      d.next2_departure || '—', d.next2_shape_dist_traveled || '—', false,
    ]);
    return `<table class="planner-times planner-stm056-times"><thead><tr><th>${t('producer.col.stop')}</th><th>${t('producer.times.stm056_sequence')}</th><th>${t('producer.times.arrival')}</th><th>${t('producer.times.departure')}</th><th>${t('producer.times.stm056_distance')}</th></tr></thead><tbody>${
      rows.map(([stop, sequence, arrival, departure, distance, bad]) => `<tr${bad ? ' class="bad"' : ''}><td>${escHtml(stop)}</td><td>${escHtml(sequence)}</td><td>${escHtml(arrival)}</td><td>${escHtml(departure)}</td><td>${escHtml(distance)}</td></tr>`).join('')
    }</tbody></table>`;
  }
  if (n.rule_id === 'STM_012') {
    type SegmentRow = [string, string, string, string, string, boolean];
    const rows: SegmentRow[] = [];
    if (d.prev_stop_id) rows.push([name(d.prev_stop_id), plannerTime(d.prev_arrival) || '—', plannerTime(d.prev_departure) || '—', '', '', false]);
    rows.push([name(d.stop_a), plannerTime(d.stop_a_arrival) || '—', plannerTime(d.stop_a_departure) || '—', '', '', false]);
    const distance = d.distance_meters
      ? localizedNumber(d.distance_meters)
      : d.distance_km ? localizedNumber(String(Number(d.distance_km) * 1000)) : '—';
    const rawSpeed = d.speed_kmh || d.speed_lower_bound_kmh;
    const speed = rawSpeed
      ? d.speed_lower_bound_kmh
        ? t('producer.times.speed_at_least', { speed: localizedNumber(rawSpeed) })
        : localizedNumber(rawSpeed)
      : '—';
    rows.push([name(d.stop_b), plannerTime(d.stop_b_arrival) || '—', plannerTime(d.stop_b_departure) || '—', distance, speed, true]);
    if (d.next_stop_id) rows.push([name(d.next_stop_id), plannerTime(d.next_arrival) || '—', plannerTime(d.next_departure) || '—', '', '', false]);
    return `<table class="planner-times"><thead><tr><th>${t('producer.col.stop')}</th><th>${t('producer.times.arrival')}</th><th>${t('producer.times.departure')}</th><th>${t('producer.times.distance_m')}</th><th>${t('producer.times.speed_kmh')}</th></tr></thead><tbody>${
      rows.map(([stop, arr, dep, meters, speedValue, bad]) => `<tr${bad ? ' class="bad"' : ''}><td>${escHtml(stop)}</td><td>${escHtml(arr)}</td><td>${escHtml(dep)}</td><td>${escHtml(meters)}</td><td>${escHtml(speedValue)}</td></tr>`).join('')
    }</tbody></table>`;
  }
  if (n.rule_id === 'STM_014') {
    type SegmentRow = [string, string, string, string, string, boolean];
    const rows: SegmentRow[] = [];
    if (d.prev_stop_id) rows.push([name(d.prev_stop_id), plannerTime(d.prev_arrival) || '—', plannerTime(d.prev_departure) || '—', '', '', false]);
    rows.push([name(d.stop_a), plannerTime(d.stop_a_arrival) || '—', plannerTime(d.stop_a_departure) || '—', '', '', false]);
    const range = (min?: string, max?: string) => {
      if (!min) return '—';
      const low = localizedNumber(min);
      const high = max ? localizedNumber(max) : low;
      return low === high ? low : `${low}–${high}`;
    };
    rows.push([name(d.stop_b), plannerTime(d.stop_b_arrival) || '—', plannerTime(d.stop_b_departure) || '—', range(d.distance_min_meters, d.distance_max_meters), range(d.speed_min, d.speed_max), true]);
    if (d.next_stop_id) rows.push([name(d.next_stop_id), plannerTime(d.next_arrival) || '—', plannerTime(d.next_departure) || '—', '', '', false]);
    return `<table class="planner-times"><thead><tr><th>${t('producer.col.stop')}</th><th>${t('producer.times.arrival')}</th><th>${t('producer.times.departure')}</th><th>${t('producer.times.stm014_distance_m')}</th><th>${t('producer.times.stm014_speed_kmh')}</th></tr></thead><tbody>${
      rows.map(([stop, arr, dep, meters, speedValue, bad]) => `<tr${bad ? ' class="bad"' : ''}><td>${escHtml(stop)}</td><td>${escHtml(arr)}</td><td>${escHtml(dep)}</td><td>${escHtml(meters)}</td><td>${escHtml(speedValue)}</td></tr>`).join('')
    }</tbody></table>`;
  }
  const missing = t('producer.times.missing');
  if (n.rule_id === 'STM_015') {
    type EdgeRow = [string, string, string, string, boolean];
    const rows: EdgeRow[] = [[
      name(d.stop_id), d.stop_sequence || '—', missing, d.departure ?? '—', true,
    ]];
    if (d.following_stop_id) rows.push([
      name(d.following_stop_id), d.following_stop_sequence || '—',
      d.following_arrival || '—', d.following_departure || '—', false,
    ]);
    if (d.following2_stop_id) rows.push([
      name(d.following2_stop_id), d.following2_stop_sequence || '—',
      d.following2_arrival || '—', d.following2_departure || '—', false,
    ]);
    // Older reports only carry the nearest timed following stop.
    if (rows.length === 1 && d.next_stop_id) rows.push([
      name(d.next_stop_id), '—', d.next_time ?? '—', '—', false,
    ]);
    return `<table class="planner-times planner-stm015-times"><thead><tr><th>${t('producer.col.stop')}</th><th>${t('producer.times.sequence')}</th><th>${t('producer.times.arrival')}</th><th>${t('producer.times.departure')}</th></tr></thead><tbody>${
      rows.map(([stop, sequence, arrival, departure, bad]) => `<tr${bad ? ' class="bad"' : ''}><td>${escHtml(stop)}</td><td>${escHtml(sequence)}</td><td>${escHtml(arrival)}</td><td>${escHtml(departure)}</td></tr>`).join('')
    }</tbody></table>`;
  }
  if (n.rule_id === 'STM_016') {
    type EdgeRow = [string, string, string, string, boolean];
    const rows: EdgeRow[] = [];
    if (d.preceding2_stop_id) rows.push([
      name(d.preceding2_stop_id), d.preceding2_stop_sequence || '—',
      d.preceding2_arrival || '—', d.preceding2_departure || '—', false,
    ]);
    if (d.preceding_stop_id) rows.push([
      name(d.preceding_stop_id), d.preceding_stop_sequence || '—',
      d.preceding_arrival || '—', d.preceding_departure || '—', false,
    ]);
    // Older reports only carry the nearest timed preceding stop.
    if (!rows.length && d.prev_stop_id) rows.push([
      name(d.prev_stop_id), '—', d.prev_time ?? '—', '—', false,
    ]);
    rows.push([
      name(d.stop_id), d.stop_sequence || '—', missing, d.departure ?? '—', true,
    ]);
    return `<table class="planner-times planner-stm016-times"><thead><tr><th>${t('producer.col.stop')}</th><th>${t('producer.times.sequence')}</th><th>${t('producer.times.arrival')}</th><th>${t('producer.times.departure')}</th></tr></thead><tbody>${
      rows.map(([stop, sequence, arrival, departure, bad]) => `<tr${bad ? ' class="bad"' : ''}><td>${escHtml(stop)}</td><td>${escHtml(sequence)}</td><td>${escHtml(arrival)}</td><td>${escHtml(departure)}</td></tr>`).join('')
    }</tbody></table>`;
  }
  if (n.rule_id === 'STM_017') {
    type DistanceRow = [string, string, string, string];
    const rows: DistanceRow[] = [];
    for (let i = 0; d[`missing_${i}_stop_id`]; i++) {
      rows.push([
        name(d[`missing_${i}_stop_id`]),
        d[`missing_${i}_stop_sequence`] || '—',
        d[`missing_${i}_arrival`] || '—',
        d[`missing_${i}_departure`] || '—',
      ]);
    }
    if (!rows.length) return '';
    return `<table class="planner-times planner-stm017-times"><thead><tr><th>${t('producer.col.stop')}</th><th>${t('producer.times.sequence')}</th><th>${t('producer.times.arrival')}</th><th>${t('producer.times.departure')}</th><th>${t('producer.times.cumulative_distance')}</th></tr></thead><tbody>${
      rows.map(([stop, sequence, arrival, departure]) => `<tr class="bad"><td>${escHtml(stop)}</td><td>${escHtml(sequence)}</td><td>${escHtml(arrival)}</td><td>${escHtml(departure)}</td><td>${t('producer.times.missing')}</td></tr>`).join('')
    }</tbody></table>`;
  }
  if ((n.rule_id === 'STM_018' || n.rule_id === 'STM_019') && d.stop_id) {
    type ContinuousServiceRow = [string, string, string, string, string, boolean];
    const rows: ContinuousServiceRow[] = [[
      name(d.stop_id), d.stop_sequence || '—', plannerTime(d.arrival ?? d.arr) || '—',
      plannerTime(d.departure ?? d.dep) || '—', visibleValue(n.observed_value) || '—', true,
    ]];
    if (d.next_stop_id) rows.push([
      name(d.next_stop_id), d.next_stop_sequence || '—', plannerTime(d.next_arrival) || '—',
      plannerTime(d.next_departure) || '—', t('producer.times.section_end'), false,
    ]);
    const settingKey = n.rule_id === 'STM_018' ? 'producer.times.continuous_pickup' : 'producer.times.continuous_drop_off';
    return `<table class="planner-times planner-continuous-times"><thead><tr><th>${t('producer.col.stop')}</th><th>${t('producer.times.sequence')}</th><th>${t('producer.times.arrival')}</th><th>${t('producer.times.departure')}</th><th>${t(settingKey)}</th></tr></thead><tbody>${
      rows.map(([stop, sequence, arrival, departure, setting, bad]) => `<tr${bad ? ' class="bad"' : ''}><td>${escHtml(stop)}</td><td>${escHtml(sequence)}</td><td>${escHtml(arrival)}</td><td>${escHtml(departure)}</td><td>${escHtml(setting)}</td></tr>`).join('')
    }</tbody></table>`;
  }
  if (n.rule_id === 'STM_024' && d.stop_times_max && d.shape_max) {
    const length = d.shape_length_m;
    const unitCell = (value?: string) => {
      const unit = distanceUnit(value, length);
      return unit ? t(`producer.card.STM_024.unit_${unit}`) : '—';
    };
    // Three significant digits: a km/m mismatch is a ratio like 0.000996, which one decimal hides.
    const ratioCell = (value?: string) => (length && Number(length) > 0 && value
      ? new Intl.NumberFormat(getLocale(), { maximumSignificantDigits: 3 }).format(Number(value) / Number(length))
      : '—');
    const rows: Array<[string, string, string, string, boolean]> = [
      [t('producer.times.stm024_shape'), localizedNumber(d.shape_max), ratioCell(d.shape_max), unitCell(d.shape_max), false],
      [t('producer.times.stm024_stop_times'), localizedNumber(d.stop_times_max), ratioCell(d.stop_times_max), unitCell(d.stop_times_max), false],
      ...(length ? [[t('producer.times.stm024_drawn'), localizedNumber(length), '1', t('producer.card.STM_024.unit_m'), false] as [string, string, string, string, boolean]] : []),
    ];
    const odd = rows.slice(0, 2).filter(([, , , unit]) => unit !== t('producer.card.STM_024.unit_m'));
    if (odd.length === 1) odd[0][4] = true;
    return `<table class="planner-times planner-stm024-times"><thead><tr><th>${t('producer.times.stm024_source')}</th><th>${t('producer.times.stm024_last')}</th><th>${t('producer.times.stm024_ratio')}</th><th>${t('producer.times.stm024_unit')}</th></tr></thead><tbody>${
      rows.map(([source, value, ratio, unit, bad]) => `<tr${bad ? ' class="bad"' : ''}><td>${escHtml(source)}</td><td>${escHtml(value)}</td><td>${escHtml(ratio)}</td><td>${escHtml(unit)}</td></tr>`).join('')
    }</tbody></table>`;
  }
  if (n.rule_id === 'STM_022' && d.stop_id) {
    const cells = [
      name(d.stop_id), d.stop_sequence || '—',
      plannerTime(d.arr ?? d.arrival) || '—', plannerTime(d.dep ?? d.departure) || '—',
      n.observed_value ?? '—',
    ];
    return `<table class="planner-times planner-stm022-times"><thead><tr><th>${t('producer.col.stop')}</th><th>${t('producer.times.sequence')}</th><th>${t('producer.times.arrival')}</th><th>${t('producer.times.departure')}</th><th>${t('producer.times.timepoint')}</th></tr></thead><tbody><tr class="bad">${
      cells.map(cell => `<td>${escHtml(cell)}</td>`).join('')
    }</tr></tbody></table>`;
  }
  if ((n.rule_id === 'STM_041' || n.rule_id === 'STM_042') && d.stop_id) {
    // The row's stop and what conflicts on it: the zone written next to it, or the headsign text.
    const columns: Array<[string, string]> = n.rule_id === 'STM_041'
      ? [[t('producer.col.stop'), name(d.stop_id)], [t('producer.times.flex_zone'), d.location_id || d.location_group_id || '—'],
        [t('producer.times.sequence'), d.stop_sequence || '—'], [t('producer.times.departure'), plannerTime(d.departure) || '—']]
      : [[t('producer.col.stop'), name(d.stop_id)], [t('producer.times.sequence'), d.stop_sequence || '—'],
        [t('producer.times.departure'), plannerTime(d.departure) || '—'], [t('producer.times.headsign'), d.stop_headsign || '—']];
    return `<table class="planner-times planner-stop-times"><thead><tr>${columns.map(([head]) => `<th>${escHtml(head)}</th>`).join('')}</tr></thead><tbody><tr class="bad">${
      columns.map(([, cell]) => `<td>${escHtml(cell)}</td>`).join('')
    }</tr></tbody></table>`;
  }
  if (n.rule_id === 'STM_050' && d.example_0_stop_id) {
    // The first rows with an empty mark, in file order: trip, stop with its sequence, times.
    const rows: string[] = [];
    for (let i = 0; d[`example_${i}_stop_id`]; i++) {
      const at = (key: string) => d[`example_${i}_${key}`];
      const stop = `${name(at('stop_id'))} (${at('stop_sequence') || '—'})`;
      rows.push(`<tr><td>${escHtml(tripShort(at('trip_id'), result))}</td><td>${escHtml(stop)}</td><td>${escHtml(plannerTime(at('arrival')) || '—')}</td><td>${escHtml(plannerTime(at('departure')) || '—')}</td></tr>`);
    }
    return `<p class="producer-card-label">${escHtml(t('producer.times.stm050_examples'))}</p><table class="planner-times planner-stop-times"><thead><tr><th>${t('producer.col.trip')}</th><th>${t('producer.times.stop_with_sequence')}</th><th>${t('producer.times.arrival')}</th><th>${t('producer.times.departure')}</th></tr></thead><tbody>${rows.join('')}</tbody></table>`;
  }
  if (n.rule_id.startsWith('OPR_')) {
    const table = oprTable(n, result);
    if (table) return table;
  }
  if (n.rule_id.startsWith('TRP_')) {
    const table = trpTable(n, result);
    if (table) return table;
  }
  if (n.rule_id.startsWith('VAT_')) {
    const table = vatTable(n, result);
    if (table) return table;
  }
  if (STP_CONTEXT_RULES.has(n.rule_id)) {
    const table = stpTable(n, result);
    if (table) return table;
  }
  if (GEO_CONTEXT_RULES.has(n.rule_id)) {
    const table = geoTable(n, result);
    if (table) return table;
  }
  if (SHP_CONTEXT_RULES.has(n.rule_id)) {
    const table = shpTable(n, result);
    if (table) return table;
  }
  if (RTS_CONTEXT_RULES.has(n.rule_id)) {
    const table = rtsTable(n, result);
    if (table) return table;
  }
  if (n.rule_id === 'CAL_020' && d.feed_start_date && d.feed_end_date) {
    // The window as written next to the period the calendars run (and the calendar setting each end).
    const by = (date?: string, svc?: string) => (date ? (svc ? t('producer.times.cal020_by', { date: ymdText(date), svc }) : ymdText(date)) : '—');
    const days = (a?: string, b?: string) => {
      if (!a || !b || !/^\d{8}$/.test(a) || !/^\d{8}$/.test(b)) return '—';
      const ms = (v: string) => Date.UTC(Number(v.slice(0, 4)), Number(v.slice(4, 6)) - 1, Number(v.slice(6, 8)));
      return localizedNumber(String(Math.round((ms(b) - ms(a)) / 86_400_000) + 1));
    };
    const rows = [
      `<tr class="bad"><td>${escHtml(t('producer.times.cal020_written'))}</td><td>${escHtml(ymdText(d.feed_start_date))}</td><td>${escHtml(ymdText(d.feed_end_date))}</td><td>${escHtml(days(d.feed_start_date, d.feed_end_date))}</td></tr>`,
      ...(d.service_first && d.service_last ? [`<tr><td>${escHtml(t('producer.times.cal020_calendars'))}</td><td>${escHtml(by(d.service_first, d.service_first_calendar))}</td><td>${escHtml(by(d.service_last, d.service_last_calendar))}</td><td>${escHtml(days(d.service_first, d.service_last))}</td></tr>`] : []),
    ];
    return `<p class="producer-card-label">${escHtml(t('producer.times.cal020_periods'))}</p><table class="planner-times planner-stop-times"><thead><tr><th>${t('producer.times.period')}</th><th>${t('producer.field.start_date')}</th><th>${t('producer.field.end_date')}</th><th>${t('producer.times.days')}</th></tr></thead><tbody>${rows.join('')}</tbody></table>`;
  }
  if (CAL_USAGE_RULES.has(n.rule_id)) {
    // The calendars of a summary (days and validity), then the routes running on them with
    // their trip counts and an example trip.
    const parts: string[] = [];
    const services = n.entity_type === 'Service' ? [] : calServices(n);
    if (services.length) {
      const rows = services.slice(0, CAL_SERVICE_ROWS).map(svc => `<tr><td>${escHtml(svc)}</td><td>${escHtml(plannerServiceText(svc, result))}</td><td>${escHtml(serviceRangeText(svc, result))}</td></tr>`);
      if (services.length > CAL_SERVICE_ROWS) rows.push(`<tr><td colspan="3">${escHtml(t('producer.times.more_services', { n: localizedNumber(String(services.length - CAL_SERVICE_ROWS)) }))}</td></tr>`);
      parts.push(`<p class="producer-card-label">${escHtml(t('producer.times.cal_services', { n: localizedNumber(String(services.length)) }))}</p><table class="planner-times planner-stop-times"><thead><tr><th>${t('producer.col.service_code')}</th><th>${t('producer.col.service')}</th><th>${t('producer.times.validity')}</th></tr></thead><tbody>${rows.join('')}</tbody></table>`);
    }
    if (d.route_0_route_id) {
      const rows: string[] = [];
      for (let i = 0; d[`route_${i}_route_id`]; i++) {
        const routeId = d[`route_${i}_route_id`]!;
        rows.push(`<tr><td>${routeBadgeHtml(routeId, result)}</td><td>${escHtml(localizedNumber(d[`route_${i}_trips`] || '0'))}</td><td>${escHtml(tripShort(d[`route_${i}_example_trip_id`], result, false))}</td></tr>`);
      }
      parts.push(`<p class="producer-card-label">${escHtml(t('producer.times.cal_routes', { routes: localizedNumber(d.route_count || '0'), trips: localizedNumber(d.trip_count || '0') }))}</p><table class="planner-times planner-stop-times"><thead><tr><th>${t('producer.col.route')}</th><th>${t('producer.times.trip_count')}</th><th>${t('producer.times.example_trip')}</th></tr></thead><tbody>${rows.join('')}</tbody></table>`);
    }
    return parts.join('');
  }
  if (n.rule_id === 'DQ_012' && d.example_0_agency_id != null) {
    // The first agencies in file order: none of them is named on a route.
    const rows: string[] = [];
    for (let i = 0; d[`example_${i}_agency_id`] != null; i++) {
      const id = d[`example_${i}_agency_id`];
      const agencyName = d[`example_${i}_agency_name`];
      const label = agencyName ? (id ? `${agencyName} (${id})` : agencyName) : id || '—';
      rows.push(`<tr><td>${escHtml(label)}</td></tr>`);
    }
    return `<p class="producer-card-label">${escHtml(t('producer.times.dq012_agencies'))}</p><table class="planner-times planner-stop-times"><thead><tr><th>${t('producer.times.agency')}</th></tr></thead><tbody>${rows.join('')}</tbody></table>`;
  }
  if (n.rule_id === 'DQ_016' && d.sample_0_field) {
    // One raw value per column, with the spaces made visible, the record it sits on (named when
    // the list is stops, routes or trips) and its row number in the file.
    const ni = result.name_index;
    const recordText = (record?: string): string => {
      if (!record) return '—';
      const stop = d.record_field === 'stop_id' ? dqIndexKey(ni.stops, record) : undefined;
      if (stop) return `${ni.stops[stop]} (${visibleValue(record)})`;
      const route = d.record_field === 'route_id' ? dqIndexKey(ni.routes, record) : undefined;
      if (route) return `${ni.routes[route]} (${visibleValue(record)})`;
      return visibleValue(record);
    };
    const samples = dq016Samples(d);
    const withRecord = samples.some(sample => sample.record);
    const withLine = samples.some(sample => sample.line);
    const rows = samples.map(sample => `<tr class="bad"><td>${escHtml(plannerFieldName(sample.field))}</td><td>${escHtml(visibleValue(sample.value) || '—')}</td>${
      withRecord ? `<td>${escHtml(recordText(sample.record))}</td>` : ''}${withLine ? `<td>${escHtml(sample.line || '—')}</td>` : ''}</tr>`);
    const recordHead = d.record_field && PLANNER_FIELDS.has(d.record_field) ? plannerFieldName(d.record_field) : t('producer.col.record');
    return `<p class="producer-card-label">${escHtml(t('producer.times.dq016_samples'))}</p><table class="planner-times planner-stop-times"><thead><tr><th>${t('producer.times.field')}</th><th>${t('producer.times.value')}</th>${
      withRecord ? `<th>${escHtml(recordHead)}</th>` : ''}${withLine ? `<th>${t('producer.col.row')}</th>` : ''}</tr></thead><tbody>${rows.join('')}</tbody></table>`;
  }
  if (n.rule_id === 'DQ_017' && d.example_0_stop_id) {
    // The stops near (0, 0) with the position as written.
    const rows: string[] = [];
    for (let i = 0; d[`example_${i}_stop_id`]; i++) {
      const at = (key: string) => d[`example_${i}_${key}`];
      const id = at('stop_id')!;
      const stop = at('stop_name') ? `${at('stop_name')} (${id})` : id;
      rows.push(`<tr class="bad"><td>${escHtml(stop)}</td><td>${escHtml(at('stop_lat') || '—')}</td><td>${escHtml(at('stop_lon') || '—')}</td></tr>`);
    }
    return `<p class="producer-card-label">${escHtml(t('producer.times.dq017_examples'))}</p><table class="planner-times planner-stop-times"><thead><tr><th>${t('producer.col.stop')}</th><th>${t('producer.times.latitude')}</th><th>${t('producer.times.longitude')}</th></tr></thead><tbody>${rows.join('')}</tbody></table>`;
  }
  if (n.rule_id === 'DQ_020' && d.route_0_route_id) {
    // The routes with the most trips without a headsign: missing / all trips, one example trip.
    const rows: string[] = [];
    for (let i = 0; d[`route_${i}_route_id`]; i++) {
      const at = (key: string) => d[`route_${i}_${key}`];
      const routeId = at('route_id') || '';
      const route = result.name_index.routes?.[routeId] || routeId;
      const share = `${localizedNumber(at('missing') || '0')} / ${localizedNumber(at('trips') || '0')}`;
      rows.push(`<tr><td>${escHtml(route)}</td><td>${escHtml(share)}</td><td>${escHtml(tripShort(at('example_trip_id'), result, false))}</td></tr>`);
    }
    return `<p class="producer-card-label">${escHtml(t('producer.times.dq020_routes'))}</p><table class="planner-times planner-stop-times"><thead><tr><th>${t('producer.col.route')}</th><th>${t('producer.times.headsign_missing')}</th><th>${t('producer.times.example_trip')}</th></tr></thead><tbody>${rows.join('')}</tbody></table>`;
  }
  if (n.rule_id === 'DQ_021' && d.example_0_key) {
    // Each repeated code and the row numbers in the file where it is written, when the engine
    // knows them. No name column: the name index keeps one name per code, which would hide that
    // the repeated records may carry different names.
    const kind = DQ021_KIND[n.file ?? ''] ?? 'other';
    const codeHead = ['stop', 'route', 'trip', 'dates'].includes(kind) ? `producer.times.dq021_code_${kind}` : 'producer.times.dq021_code_other';
    const hasLines = Object.keys(d).some(key => /^example_\d+_lines$/.test(key));
    const rows: string[] = [];
    for (let i = 0; d[`example_${i}_key`]; i++) {
      const key = d[`example_${i}_key`]!;
      const cells = [key, ...(hasLines ? [d[`example_${i}_lines`] || '—'] : [])];
      rows.push(`<tr class="bad">${cells.map(c => `<td>${escHtml(c)}</td>`).join('')}</tr>`);
    }
    const head = [t(codeHead), ...(hasLines ? [t('producer.times.lines')] : [])];
    return `<p class="producer-card-label">${escHtml(t('producer.times.dq021_keys'))}</p><table class="planner-times planner-stop-times"><thead><tr>${head.map(h => `<th>${escHtml(h)}</th>`).join('')}</tr></thead><tbody>${rows.join('')}</tbody></table>`;
  }
  if ((n.rule_id === 'DQ_005b' || n.rule_id === 'DQ_009' || n.rule_id === 'DQ_013') && d.example_0_trip_id) {
    // The first trips in file order: trip, route and calendar, none of which has a stop time.
    const rows: string[] = [];
    for (let i = 0; d[`example_${i}_trip_id`]; i++) {
      const at = (key: string) => d[`example_${i}_${key}`];
      const routeId = at('route_id');
      const route = routeId ? result.name_index.routes?.[routeId] || routeId : '—';
      rows.push(`<tr${n.rule_id === 'DQ_013' ? '' : ' class="bad"'}><td>${escHtml(tripShort(at('trip_id'), result, false))}</td><td>${escHtml(route)}</td><td>${escHtml(at('service_id') || '—')}</td></tr>`);
    }
    return `<p class="producer-card-label">${escHtml(t(n.rule_id === 'DQ_013' ? 'producer.times.dq013_trips' : 'producer.times.dq_trip_examples'))}</p><table class="planner-times planner-stop-times"><thead><tr><th>${t('producer.col.trip')}</th><th>${t('producer.col.route')}</th><th>${t('producer.col.service_code')}</th></tr></thead><tbody>${rows.join('')}</tbody></table>`;
  }
  if (n.rule_id === 'DQ_005c' && d.example_0_stop_id) {
    // The first stops without a position: name and code, type, and the station they belong to.
    const kind = (v?: string) => {
      const key = `producer.times.location_type_${v || '0'}`;
      const text = t(key);
      return text === key ? v || '—' : text;
    };
    const rows: string[] = [];
    for (let i = 0; d[`example_${i}_stop_id`]; i++) {
      const at = (key: string) => d[`example_${i}_${key}`];
      const id = at('stop_id') || '—';
      const stop = at('stop_name') ? `${at('stop_name')} (${id})` : id;
      const parent = at('parent_station') ? name(at('parent_station')) : '—';
      rows.push(`<tr class="bad"><td>${escHtml(stop)}</td><td>${escHtml(kind(at('location_type')))}</td><td>${escHtml(parent)}</td></tr>`);
    }
    return `<p class="producer-card-label">${escHtml(t('producer.times.dq005c_examples'))}</p><table class="planner-times planner-stop-times"><thead><tr><th>${t('producer.col.stop')}</th><th>${t('producer.times.location_type')}</th><th>${t('producer.times.parent_station')}</th></tr></thead><tbody>${rows.join('')}</tbody></table>`;
  }
  if (n.rule_id === 'DQ_006' && d.route_0_route_id) {
    // The routes with the most trips without a line: shapeless / all fixed trips, one example trip.
    const rows: string[] = [];
    for (let i = 0; d[`route_${i}_route_id`]; i++) {
      const at = (key: string) => d[`route_${i}_${key}`];
      const routeId = at('route_id') || '';
      const route = result.name_index.routes?.[routeId] || routeId;
      const share = `${localizedNumber(at('shapeless') || '0')} / ${localizedNumber(at('trips') || '0')}`;
      rows.push(`<tr><td>${escHtml(route)}</td><td>${escHtml(share)}</td><td>${escHtml(tripShort(at('example_trip_id'), result, false))}</td></tr>`);
    }
    return `<p class="producer-card-label">${escHtml(t('producer.times.dq006_routes'))}</p><table class="planner-times planner-stop-times"><thead><tr><th>${t('producer.col.route')}</th><th>${t('producer.times.shapeless_trips')}</th><th>${t('producer.times.example_trip')}</th></tr></thead><tbody>${rows.join('')}</tbody></table>`;
  }
  if (n.rule_id === 'DQ_010' && d.used_0_agency_id) {
    // The agencies the routes do point at, most routes first.
    const rows: string[] = [];
    for (let i = 0; d[`used_${i}_agency_id`]; i++) {
      const id = d[`used_${i}_agency_id`];
      const agencyName = result.feed_summary?.agencies?.find(a => a.agency_id === id)?.agency_name;
      const label = agencyName ? `${agencyName} (${id})` : id;
      rows.push(`<tr><td>${escHtml(label)}</td><td>${escHtml(localizedNumber(d[`used_${i}_routes`] || '0'))}</td></tr>`);
    }
    return `<p class="producer-card-label">${escHtml(t('producer.times.dq010_used'))}</p><table class="planner-times planner-stop-times"><thead><tr><th>${t('producer.times.agency')}</th><th>${t('producer.times.route_count')}</th></tr></thead><tbody>${rows.join('')}</tbody></table>`;
  }
  if (n.rule_id === 'STM_047' && d.example_0_stop_id) {
    // The trip's exact timepoints without a time, in stop order; the missing side reads "missing".
    const missing = t('producer.times.missing');
    const rows: string[] = [];
    for (let i = 0; d[`example_${i}_stop_id`]; i++) {
      const at = (key: string) => d[`example_${i}_${key}`];
      const side = at('missing');
      const arrival = side === 'arrival' || side === 'both' ? missing : plannerTime(at('arrival')) || '—';
      const departure = side === 'departure' || side === 'both' ? missing : plannerTime(at('departure')) || '—';
      rows.push(`<tr class="bad"><td>${escHtml(name(at('stop_id')))}</td><td>${escHtml(at('stop_sequence') || '—')}</td><td>${escHtml(arrival)}</td><td>${escHtml(departure)}</td></tr>`);
    }
    return `<p class="producer-card-label">${escHtml(t('producer.times.stm047_examples'))}</p><table class="planner-times planner-stop-times"><thead><tr><th>${t('producer.col.stop')}</th><th>${t('producer.times.sequence')}</th><th>${t('producer.times.arrival')}</th><th>${t('producer.times.departure')}</th></tr></thead><tbody>${rows.join('')}</tbody></table>`;
  }
  if ((n.rule_id === 'STM_032' && d.first_stop_id) || ((n.rule_id === 'STM_033' || n.rule_id === 'STM_034' || n.rule_id === 'STM_046') && d.stop_id)) {
    type StopRow = [string, string, string, string, boolean];
    const rows: StopRow[] = n.rule_id === 'STM_032'
      ? [
        [name(d.first_stop_id), n.observed_value ?? '—', plannerTime(d.first_arrival) || '—', plannerTime(d.first_departure) || '—', true],
        [name(d.second_stop_id), n.observed_value ?? '—', plannerTime(d.second_arrival) || '—', plannerTime(d.second_departure) || '—', true],
      ]
      : [[name(d.stop_id), d.stop_sequence || '—', plannerTime(d.arrival) || '—', plannerTime(d.departure) || '—', true]];
    return `<table class="planner-times planner-stop-times"><thead><tr><th>${t('producer.col.stop')}</th><th>${t('producer.times.sequence')}</th><th>${t('producer.times.arrival')}</th><th>${t('producer.times.departure')}</th></tr></thead><tbody>${
      rows.map(([stop, sequence, arrival, departure, bad]) => `<tr${bad ? ' class="bad"' : ''}><td>${escHtml(stop)}</td><td>${escHtml(sequence)}</td><td>${escHtml(arrival)}</td><td>${escHtml(departure)}</td></tr>`).join('')
    }</tbody></table>`;
  }
  if (n.rule_id === 'STM_035' && d.stop_id) {
    // Both rows are the same stop.
    const rows = segmentStopRows({ ...d, stop_a: d.stop_id, stop_b: d.stop_id }, result);
    return `<table class="planner-times planner-stm035-times"><thead><tr><th>${t('producer.col.stop')}</th><th>${t('producer.times.sequence')}</th><th>${t('producer.times.arrival')}</th><th>${t('producer.times.departure')}</th></tr></thead><tbody>${
      rows.map(r => `<tr${r.bad ? ' class="bad"' : ''}><td>${escHtml(r.stop)}</td><td>${escHtml(r.sequence)}</td><td>${escHtml(plannerTime(r.arrival) || '—')}</td><td>${escHtml(plannerTime(r.departure) || '—')}</td></tr>`).join('')
    }</tbody></table>` + tripListTable(d, result, at => `${at('stop_a_sequence') || '—'}–${at('stop_b_sequence') || '—'} · ${plannerTime(at('stop_a_departure')) || '—'} → ${plannerTime(at('stop_b_arrival')) || '—'}`, 'stm035_trips');
  }
  if (['STM_051', 'STM_052', 'STM_054', 'STM_055'].includes(n.rule_id) && (d.location_id || d.location_group_id || d.stop_id)) {
    // The Flex row: its zone (or stop), sequence, window and the boarding setting that conflicts.
    const zone = d.location_id || d.location_group_id;
    const settingKey = {
      STM_051: 'producer.times.pickup_type', STM_052: 'producer.times.drop_off_type',
      STM_054: 'producer.times.continuous_pickup', STM_055: 'producer.times.continuous_drop_off',
    }[n.rule_id]!;
    const { value, meaning } = flexSetting(n);
    const columns: Array<[string, string]> = [
      zone ? [t('producer.times.flex_zone'), zone] : [t('producer.col.stop'), name(d.stop_id)],
      [t('producer.times.sequence'), d.stop_sequence || '—'],
      [t('producer.times.window'), `${plannerTime(d.window_start) || '—'}–${plannerTime(d.window_end) || '—'}`],
      [t(settingKey), `${value} (${meaning})`],
    ];
    return `<table class="planner-times planner-flex-times"><thead><tr>${columns.map(([head]) => `<th>${escHtml(head)}</th>`).join('')}</tr></thead><tbody><tr class="bad">${
      columns.map(([, cell]) => `<td>${escHtml(cell)}</td>`).join('')
    }</tr></tbody></table>`;
  }
  if (n.rule_id === 'STM_053' && d.example_0_stop_id) {
    // The example trip's same-time run, with one row either side for contrast.
    const rows = stm053Rows(d);
    return `<table class="planner-times planner-neighbour-times"><thead><tr><th>${t('producer.times.sequence')}</th><th>${t('producer.col.stop')}</th><th>${t('producer.times.arrival')}</th><th>${t('producer.times.departure')}</th></tr></thead><tbody>${
      rows.map(r => `<tr${r.run ? ' class="bad"' : ''}><td>${escHtml(r.sequence || '—')}</td><td>${escHtml(name(r.stop))}</td><td>${escHtml(plannerTime(r.arrival) || '—')}</td><td>${escHtml(plannerTime(r.departure) || '—')}</td></tr>`).join('')
    }</tbody></table>`;
  }
  if (n.rule_id === 'STM_059' && (d.stop_id || d.location_id || d.location_group_id)) {
    // The row, the side set to "book by phone" and the booking rule it lacks.
    const zone = d.location_id || d.location_group_id;
    const dropOff = n.field === 'drop_off_booking_rule_id';
    const columns: Array<[string, string]> = [
      zone ? [t('producer.times.flex_zone'), zone] : [t('producer.col.stop'), name(d.stop_id)],
      [t('producer.times.sequence'), d.stop_sequence || '—'],
      [t(dropOff ? 'producer.times.drop_off_type' : 'producer.times.pickup_type'), (dropOff ? d.drop_off_type : d.pickup_type) || '2'],
      [t('producer.times.booking_rule'), t('producer.times.missing')],
    ];
    return `<table class="planner-times planner-flex-times"><thead><tr>${columns.map(([head]) => `<th>${escHtml(head)}</th>`).join('')}</tr></thead><tbody><tr class="bad">${
      columns.map(([, cell]) => `<td>${escHtml(cell)}</td>`).join('')
    }</tr></tbody></table>`;
  }
  if (n.rule_id === 'STM_060' && d.zone_a_id && d.zone_b_id) {
    // Both records: zone, sequence, window and which sides they offer.
    const sides = (prefix: string) => [d[`${prefix}_pickup`], d[`${prefix}_drop_off`]]
      .map(v => t(v === 'false' ? 'producer.times.closed' : 'producer.times.open')).join(' / ');
    const rows = ['zone_a', 'zone_b'].map(prefix => [
      d[`${prefix}_id`], d[`${prefix}_stop_sequence`] || '—',
      `${plannerTime(d[`${prefix}_window_start`]) || '—'}–${plannerTime(d[`${prefix}_window_end`]) || '—'}`, sides(prefix),
    ]);
    return `<table class="planner-times planner-flex-times"><thead><tr><th>${t('producer.times.flex_zone')}</th><th>${t('producer.times.sequence')}</th><th>${t('producer.times.window')}</th><th>${t('producer.times.pickup_drop_off')}</th></tr></thead><tbody>${
      rows.map(cells => `<tr class="bad">${cells.map(cell => `<td>${escHtml(cell)}</td>`).join('')}</tr>`).join('')
    }</tbody></table>`;
  }
  if (n.rule_id === 'STM_061' && d.stop_a && d.stop_b && (d.stop_a_departure || d.stop_b_arrival)) {
    // The example trip's departure from A, the stops between, its arrival at B.
    const between = Number(d.between_stops ?? '0');
    const rows = [
      `<tr class="bad"><td>${escHtml(d.stop_a_sequence || '—')}</td><td>${escHtml(name(d.stop_a))}</td><td>—</td><td>${escHtml(plannerTime(d.stop_a_departure) || '—')}</td></tr>`,
      ...(between > 0 ? [`<tr><td>…</td><td>${escHtml(t('producer.times.between_stops', { n: localizedNumber(String(between)) }))}</td><td></td><td></td></tr>`] : []),
      `<tr class="bad"><td>${escHtml(d.stop_b_sequence || '—')}</td><td>${escHtml(name(d.stop_b))}</td><td>${escHtml(plannerTime(d.stop_b_arrival) || '—')}</td><td>—</td></tr>`,
    ];
    return `<table class="planner-times planner-neighbour-times"><thead><tr><th>${t('producer.times.sequence')}</th><th>${t('producer.col.stop')}</th><th>${t('producer.times.arrival')}</th><th>${t('producer.times.departure')}</th></tr></thead><tbody>${rows.join('')}</tbody></table>`;
  }
  if (['STM_037', 'STM_038', 'STM_039', 'STM_040', 'STM_058'].includes(n.rule_id) && (d.location_id || d.location_group_id)) {
    // Only the columns the finding is about (at most four): a seven-column table did not fit the
    // card's text column and pushed past the card.
    const zone = d.location_id || d.location_group_id || '—';
    const window = `${plannerTime(d.window_start) || '—'}–${plannerTime(d.window_end) || '—'}`;
    const columns: Array<[string, string]> = [[t('producer.times.flex_zone'), zone], [t('producer.times.sequence'), d.stop_sequence || '—']];
    if (n.rule_id === 'STM_037') {
      columns.push([t('producer.times.window'), window],
        [t('producer.times.fixed_time'), `${plannerTime(d.arrival) || '—'} / ${plannerTime(d.departure) || '—'}`]);
    } else if (n.rule_id === 'STM_040') {
      columns.push([t('producer.times.window'), window], [t('producer.times.booking_rule'), d.pickup_booking_rule_id || d.drop_off_booking_rule_id || '—']);
    } else {
      columns.push([t('producer.times.window_start'), plannerTime(d.window_start) || '—'], [t('producer.times.window_end'), plannerTime(d.window_end) || '—']);
    }
    return `<table class="planner-times planner-flex-times"><thead><tr>${columns.map(([head]) => `<th>${escHtml(head)}</th>`).join('')}</tr></thead><tbody><tr class="bad">${
      columns.map(([, cell]) => `<td>${escHtml(cell)}</td>`).join('')
    }</tr></tbody></table>`;
  }
  if ((n.rule_id === 'STM_028' || n.rule_id === 'STM_029' || n.rule_id === 'STM_043') && d.first_stop_id && d.last_stop_id) {
    type SpanRow = [string, string, string, string, boolean];
    const rows: SpanRow[] = [[name(d.first_stop_id), d.first_stop_sequence || '—', '—', plannerTime(d.first_departure) || '—', n.rule_id === 'STM_029']];
    if (n.rule_id === 'STM_028' && d.gap_stop_a && d.gap_stop_b) {
      // Skip a gap end that is the first or last stop itself; that row is already listed.
      if (d.gap_a_sequence !== d.first_stop_sequence) rows.push([name(d.gap_stop_a), d.gap_a_sequence || '—', '—', plannerTime(d.gap_from) || '—', true]);
      else rows[0][4] = true;
      if (d.gap_b_sequence !== d.last_stop_sequence) rows.push([name(d.gap_stop_b), d.gap_b_sequence || '—', plannerTime(d.gap_to) || '—', '—', true]);
    }
    const lastBad = n.rule_id === 'STM_029' || d.gap_b_sequence === d.last_stop_sequence;
    rows.push([name(d.last_stop_id), d.last_stop_sequence || '—', plannerTime(d.last_arrival) || '—', '—', lastBad]);
    return `<table class="planner-times planner-span-times"><thead><tr><th>${t('producer.col.stop')}</th><th>${t('producer.times.sequence')}</th><th>${t('producer.times.arrival')}</th><th>${t('producer.times.departure')}</th></tr></thead><tbody>${
      rows.map(([stop, sequence, arrival, departure, bad]) => `<tr${bad ? ' class="bad"' : ''}><td>${escHtml(stop)}</td><td>${escHtml(sequence)}</td><td>${escHtml(arrival)}</td><td>${escHtml(departure)}</td></tr>`).join('')
    }</tbody></table>`;
  }
  if (n.rule_id === 'STM_030' && d.stop_id) {
    const rows: Array<[string, string, string, boolean]> = [
      ...(d.prev_stop_id ? [[name(d.prev_stop_id), d.prev_stop_sequence || '—', d.prev_shape_dist_traveled || '—', false] as [string, string, string, boolean]] : []),
      [name(d.stop_id), d.stop_sequence || '—', n.observed_value ?? '—', true],
      ...(d.next_stop_id ? [[name(d.next_stop_id), d.next_stop_sequence || '—', d.next_shape_dist_traveled || '—', false] as [string, string, string, boolean]] : []),
    ];
    return `<table class="planner-times planner-stm030-times"><thead><tr><th>${t('producer.col.stop')}</th><th>${t('producer.times.sequence')}</th><th>${t('producer.times.cumulative_distance')}</th></tr></thead><tbody>${
      rows.map(([stop, sequence, value, bad]) => `<tr${bad ? ' class="bad"' : ''}><td>${escHtml(stop)}</td><td>${escHtml(sequence)}</td><td>${escHtml(value)}</td></tr>`).join('')
    }</tbody></table>`;
  }
  if ((n.rule_id === 'STM_025' || n.rule_id === 'STM_026') && d.stop_a && d.stop_b) {
    const rows = segmentStopRows(d, result);
    if (n.rule_id === 'STM_025') {
      return `<table class="planner-times planner-stm025-times"><thead><tr><th>${t('producer.col.stop')}</th><th>${t('producer.times.sequence')}</th><th>${t('producer.times.arrival')}</th><th>${t('producer.times.departure')}</th><th>${t('producer.times.distance_from_previous')}</th></tr></thead><tbody>${
        rows.map(r => `<tr${r.bad ? ' class="bad"' : ''}><td>${escHtml(r.stop)}</td><td>${escHtml(r.sequence)}</td><td>${escHtml(plannerTime(r.arrival) || '—')}</td><td>${escHtml(plannerTime(r.departure) || '—')}</td><td>${escHtml(r.distance)}</td></tr>`).join('')
      }</tbody></table>`;
    }
    return segmentCoordTable(rows, 'planner-stm026-times');
  }
  if (n.rule_id === 'STM_021' && d.stop_a && d.stop_b) {
    const stopTable = segmentCoordTable(segmentStopRows(d, result), 'planner-stm021-times');
    const name2 = (id?: string) => (id ? result.name_index.stops?.[id] || id : '—');
    return stopTable + tripListTable(d, result,
      (at: (key: string) => string | undefined) => `${name2(at('stop_a'))} (${at('stop_a_sequence') || '—'}) → ${name2(at('stop_b'))} (${at('stop_b_sequence') || '—'})`);
  }
  if (n.rule_id === 'STM_020' && d.stop_a && d.stop_b) {
    type ZeroTravelRow = [string, string, string, string, string, boolean];
    const rows: ZeroTravelRow[] = [
      [name(d.stop_a), d.stop_a_sequence || '—', plannerTime(d.stop_a_arrival) || '—', plannerTime(d.stop_a_departure) || '—', '—', false],
      [name(d.stop_b), d.stop_b_sequence || '—', plannerTime(d.stop_b_arrival) || '—', plannerTime(d.stop_b_departure) || '—', d.distance_meters ? localizedNumber(d.distance_meters) : '—', true],
    ];
    return `<table class="planner-times planner-stm020-times"><thead><tr><th>${t('producer.col.stop')}</th><th>${t('producer.times.sequence')}</th><th>${t('producer.times.arrival')}</th><th>${t('producer.times.departure')}</th><th>${t('producer.times.distance_m')}</th></tr></thead><tbody>${
      rows.map(([stop, sequence, arrival, departure, distance, bad]) => `<tr${bad ? ' class="bad"' : ''}><td>${escHtml(stop)}</td><td>${escHtml(sequence)}</td><td>${escHtml(arrival)}</td><td>${escHtml(departure)}</td><td>${escHtml(distance)}</td></tr>`).join('')
    }</tbody></table>`;
  }
  type Row = [string, string, string, boolean];
  let rows: Row[] = [];
  if (n.rule_id === 'STM_008') {
    rows = [
      ...(d.prev_stop_id ? [[name(d.prev_stop_id), '', d.prev_time ?? '—', false] as Row] : []),
      [name(d.stop_a, d.stop_a_name), '', d.dep ?? '—', false],
      [name(d.stop_b, d.stop_b_name), d.arr ?? '—', '', true],
      ...(d.next_stop_id ? [[name(d.next_stop_id), d.next_time ?? '—', '', false] as Row] : []),
    ];
  } else if (n.rule_id === 'STM_007' && d.stop_id) {
    rows = [
      ...(d.prev_stop_id ? [[name(d.prev_stop_id), d.prev_arrival ?? '—', d.prev_departure ?? '—', false] as Row] : []),
      [name(d.stop_id), d.arr ?? '—', d.dep ?? '—', true],
      ...(d.next_stop_id ? [[name(d.next_stop_id), d.next_arrival ?? '—', d.next_departure ?? '—', false] as Row] : []),
    ];
  } else if ((n.rule_id === 'STM_009' || n.rule_id === 'STM_010') && d.stop_id) {
    const stop = result.name_index.stops?.[d.stop_id] || t('producer.card.unknown_stop');
    rows = [[stop, plannerTime(d.arrival) || '—', plannerTime(d.departure) || '—', true]];
  } else if (n.rule_id === 'STM_006') {
    const missingStop = t('producer.times.missing_stop');
    const stopName = (id?: string) => id ? name(id) : missingStop;
    const time = (value?: string) => value || '—';
    rows = [
      ...(d.prev_arrival !== undefined || d.prev_departure !== undefined
        ? [[stopName(d.prev_stop_id), time(d.prev_arrival), time(d.prev_departure), false] as Row]
        : []),
      [missingStop, time(d.arrival), time(d.departure), true],
      ...(d.next_arrival !== undefined || d.next_departure !== undefined
        ? [[stopName(d.next_stop_id), time(d.next_arrival), time(d.next_departure), false] as Row]
        : []),
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
  if (n.rule_id === 'STM_009' || n.rule_id === 'STM_010') {
    const stop = d.stop_id ? result.name_index.stops?.[d.stop_id] || t('producer.card.unknown_stop') : t('producer.card.unknown_stop');
    return t(n.rule_id === 'STM_009' ? 'producer.times.spot_pickup' : 'producer.times.spot_dropoff', {
      stop, sequence: d.stop_sequence || '—',
    });
  }
  if (n.rule_id === 'STM_006') {
    return t('producer.times.spot_missing_stop', {
      arrival: plannerTime(d.arrival) || '—', departure: plannerTime(d.departure) || '—',
    });
  }
  if ((n.rule_id === 'STM_015' || n.rule_id === 'STM_016') && d.timed_stops === '0') {
    if (d.stop_count === '1' && d.stop_id) {
      return t('producer.times.spot_untimed_at', { stop: name(d.stop_id) });
    }
    return t('producer.times.spot_untimed', { n: d.stop_count ?? '?' });
  }
  if (n.rule_id === 'STM_017' && d.missing_0_stop_id) {
    const count = Number(d.affected_stop_count || 1);
    return t(count > 1 ? 'producer.times.spot_stm017_many' : 'producer.times.spot_stm017_one', {
      stop: name(d.missing_0_stop_id), n: Number.isFinite(count) ? count : 1,
    });
  }
  if ((n.rule_id === 'STM_018' || n.rule_id === 'STM_019') && d.stop_id) {
    return d.next_stop_id ? `${name(d.stop_id)} → ${name(d.next_stop_id)}` : name(d.stop_id);
  }
  if (n.rule_id === 'STM_024' && d.stop_times_max && d.shape_max) {
    return t('producer.times.spot_stm024', {
      stop: name(d.stop_id), stop_max: localizedNumber(d.stop_times_max), shape_max: localizedNumber(d.shape_max),
    });
  }
  if (n.rule_id === 'STM_022' && d.stop_id) {
    return t('producer.times.spot_timepoint', { stop: name(d.stop_id), sequence: d.stop_sequence || '—', value: n.observed_value ?? '—' });
  }
  if (n.rule_id === 'STM_025' && d.stop_a && d.stop_b && d.duration_seconds) {
    return `${stopPair(d.stop_a, d.stop_b, result)} · ${t('producer.times.seconds', { n: d.duration_seconds })}`;
  }
  if (n.rule_id === 'STM_026' && d.stop_a && d.stop_b && d.distance_meters) {
    return `${stopPair(d.stop_a, d.stop_b, result)} · ${localizedNumber(String(Number(d.distance_meters) / 1000))} km`;
  }
  if (n.rule_id === 'STM_021' && d.stop_a && d.stop_b) return `${stopPair(d.stop_a, d.stop_b, result)} · 0 m`;
  if (n.rule_id === 'STM_020' && d.stop_a && d.stop_b) {
    const pair = stopPair(d.stop_a, d.stop_b, result);
    return d.distance_meters ? `${pair} · ${localizedNumber(d.distance_meters)} m` : pair;
  }
  if (n.rule_id === 'STM_015') return t('producer.times.spot_first', { stop: name(d.stop_id), time: d.departure ?? d.next_time ?? '—' });
  if (n.rule_id === 'STM_003' && d.stop_id) return t('producer.times.spot_arrival', { stop: name(d.stop_id), time: n.observed_value ?? '?' });
  if (n.rule_id === 'STM_007' && d.stop_id) return t('producer.times.spot_dwell', { stop: name(d.stop_id), arr: d.arr ?? '—', dep: d.dep ?? '—' });
  if (n.rule_id === 'STM_004' && d.stop_id) return t('producer.times.spot_first', { stop: name(d.stop_id), time: n.observed_value ?? '?' });
  if (n.rule_id === 'STM_016') return t('producer.times.spot_last', { stop: name(d.stop_id), time: d.prev_time ?? '—' });
  if (n.rule_id === 'STM_008') return `${name(d.stop_a, d.stop_a_name)} ${d.dep ?? '—'} → ${name(d.stop_b, d.stop_b_name)} ${d.arr ?? '—'}`;
  if (n.rule_id === 'STM_012' && (d.stop_a || d.stop_b)) {
    const distanceMeters = d.distance_meters
      ? localizedNumber(d.distance_meters)
      : d.distance_km ? localizedNumber(String(Number(d.distance_km) * 1000)) : null;
    const speed = d.speed_kmh
      ? localizedNumber(d.speed_kmh)
      : d.speed_lower_bound_kmh
        ? t('producer.times.speed_at_least', { speed: localizedNumber(d.speed_lower_bound_kmh) })
        : null;
    return `${name(d.stop_a)} → ${name(d.stop_b)}${distanceMeters ? ` · ${distanceMeters} m` : ''}${speed ? ` · ${speed} km/h` : ''}`;
  }
  if (n.rule_id === 'STM_014' && (d.stop_a || d.stop_b)) {
    const range = (min?: string, max?: string) => {
      if (!min) return null;
      const low = localizedNumber(min);
      const high = max ? localizedNumber(max) : low;
      return low === high ? low : `${low}–${high}`;
    };
    const distance = range(d.distance_min_meters, d.distance_max_meters);
    const speed = range(d.speed_min, d.speed_max);
    return `${name(d.stop_a)} → ${name(d.stop_b)}${distance ? ` · ${distance} m` : ''}${speed ? ` · ${speed} km/h` : ''}`;
  }
  // Midnight: the stop where time moves backward and the previous time (STM_048), or the arrival at the same stop (STM_049).
  if (n.rule_id === 'STM_048' && d.stop_id) return t('producer.times.spot_wrap', { stop: name(d.stop_id), prev: d.previous_time ?? '—' });
  if (n.rule_id === 'STM_049' && d.stop_id) return t('producer.times.spot_dwell', { stop: name(d.stop_id), arr: d.previous_time ?? '—', dep: d.observed_time ?? '—' });
  if ((n.rule_id === 'OPR_001' || n.rule_id === 'OPR_003') && d.gap_from && d.gap_to) {
    return n.rule_id === 'OPR_003' && d.stop_id ? `${name(d.stop_id)} ${d.gap_from} / ${d.gap_to}` : `${d.gap_from} → ${d.gap_to}`;
  }
  if (n.rule_id === 'OPR_005' && n.observed_value) return t('producer.times.spot_avg_headway', { n: n.observed_value });
  if (n.rule_id === 'OPR_009' && d.night_trips) {
    return d.first_departure === d.last_departure
      ? t('producer.times.spot_night_one', { n: d.night_trips, first: d.first_departure ?? '—' })
      : t('producer.times.spot_night', { n: d.night_trips, first: d.first_departure ?? '—', last: d.last_departure ?? '—' });
  }
  if (n.rule_id === 'OPR_010' && d.yes_trips && d.no_trips) return `${d.yes_trips} / ${d.no_trips}`;
  if (n.rule_id === 'OPR_024' && n.observed_value) return t('producer.times.spot_trips', { n: n.observed_value });
  if (['OPR_019', 'OPR_020', 'OPR_021', 'OPR_022', 'OPR_023'].includes(n.rule_id) && d.conflict_day_count) {
    return t('producer.times.spot_days', { n: d.conflict_day_count });
  }
  if (n.rule_id === 'OPR_008' && d.bad_seg_0_a) {
    const first = `${name(d.bad_seg_0_a)} → ${name(d.bad_seg_0_b)}`;
    const more = badSegments(d).length - 1;
    return more > 0 ? t('producer.times.spot_more_segments', { first, n: more }) : first;
  }
  if (n.rule_id === 'OPR_007' && d.dup_stop) {
    const times = /^(\d+)×/.exec(n.observed_value ?? '')?.[1];
    return times ? t('producer.times.spot_repeat', { stop: name(d.dup_stop), n: times }) : name(d.dup_stop);
  }
  if (n.rule_id === 'SHP_017' && d.bad_stop) return name(d.bad_stop);
  // Total duration is measured between trip endpoints: the first departure and last arrival stop times.
  if (n.rule_id === 'STM_028' && d.gap_stop_a && d.gap_stop_b) {
    return `${name(d.gap_stop_a)} ${plannerTime(d.gap_from) || '—'} → ${name(d.gap_stop_b)} ${plannerTime(d.gap_to) || '—'} · ${durationText(Number(d.gap_seconds))}`;
  }
  if ((n.rule_id === 'STM_028' || n.rule_id === 'STM_029') && d.first_stop_id && d.last_stop_id) {
    return `${name(d.first_stop_id)} ${d.first_departure ?? '—'} → ${name(d.last_stop_id)} ${d.last_arrival ?? '—'}`;
  }
  if (n.rule_id === 'STM_036' && d.kind) {
    return d.kind === 'order'
      ? t('producer.times.spot_stm036_order', { current: name(d.curr_stop), previous: name(d.prev_stop) })
      : t('producer.times.spot_stm036_scattered', { from: d.line_from ?? '—', to: d.line_to ?? '—' });
  }
  if (n.rule_id === 'STM_032' && d.first_stop_id && d.second_stop_id) {
    return `${name(d.first_stop_id)} / ${name(d.second_stop_id)} · ${t('producer.times.sequence')} ${n.observed_value ?? '—'}`;
  }
  if ((n.rule_id === 'STM_033' || n.rule_id === 'STM_034' || n.rule_id === 'STM_041' || n.rule_id === 'STM_042' || n.rule_id === 'STM_046') && d.stop_id) {
    return `${name(d.stop_id)} · ${t('producer.times.sequence')} ${d.stop_sequence || '—'}`;
  }
  if (n.rule_id === 'STM_047' && d.example_0_stop_id) {
    return `${name(d.example_0_stop_id)} · ${t('producer.times.sequence')} ${d.example_0_stop_sequence || '—'}`;
  }
  if (n.rule_id === 'STM_043' && d.first_stop_id && d.last_stop_id) {
    return `${name(d.first_stop_id)} ${plannerTime(d.first_departure) || '—'} → ${name(d.last_stop_id)} ${plannerTime(d.last_arrival) || '—'}`;
  }
  if (n.rule_id === 'STM_035' && d.stop_id) {
    return `${name(d.stop_id)} · ${t('producer.times.sequence')} ${d.stop_a_sequence || '—'}–${d.stop_b_sequence || '—'}`;
  }
  if (['STM_051', 'STM_052', 'STM_054', 'STM_055'].includes(n.rule_id) && !(d.location_id || d.location_group_id) && d.stop_id) {
    return `${name(d.stop_id)} · ${plannerTime(d.window_start) || '—'}–${plannerTime(d.window_end) || '—'}`;
  }
  if (n.rule_id === 'STM_053' && d.example_0_stop_id) {
    const run = stm053Rows(d).filter(row => row.run);
    if (run.length) return `${name(run[0].stop)} → ${name(run[run.length - 1].stop)} · ${plannerTime(run[0].arrival) || plannerTime(run[0].departure) || '—'}`;
  }
  if (n.rule_id === 'STM_060' && d.zone_a_id && d.zone_b_id) {
    return `${t('producer.times.flex_zone')} ${d.zone_a_id} / ${d.zone_b_id} · ${plannerTime(d.window_overlap_start) || '—'}–${plannerTime(d.window_overlap_end) || '—'}`;
  }
  if (n.rule_id === 'STM_059' && d.stop_id) return `${name(d.stop_id)} · ${t('producer.times.sequence')} ${d.stop_sequence || '—'}`;
  if (['STM_037', 'STM_038', 'STM_039', 'STM_040', 'STM_051', 'STM_052', 'STM_054', 'STM_055', 'STM_058', 'STM_059'].includes(n.rule_id) && (d.location_id || d.location_group_id)) {
    return `${t('producer.times.flex_zone')} ${d.location_id || d.location_group_id} · ${plannerTime(d.window_start) || '—'}–${plannerTime(d.window_end) || '—'}`;
  }
  if (n.rule_id === 'STM_030' && d.stop_id) {
    return t('producer.times.spot_stm030', { stop: name(d.stop_id), sequence: d.stop_sequence || '—', value: n.observed_value ?? '—' });
  }
  if (n.rule_id === 'STM_045' && d.stop_id) return t('producer.times.spot_first', { stop: name(d.stop_id), time: n.observed_value ?? '—' });
  // Other rules carrying a stop pair (STM_014, STM_020, STM_025, STM_026, STM_061): the two stop names.
  if (d.stop_a && d.stop_b) return stopPair(d.stop_a, d.stop_b, result);
  return null;
}

/** "A → B"; iki durağın adı aynıysa (karşılıklı duraklar) planlamacı ayırt edebilsin diye durak kodlarıyla. */
function stopPair(a: string, b: string, result: ValidationResult): string {
  const ni = result.name_index;
  const nameA = ni.stops?.[a] || a;
  const nameB = ni.stops?.[b] || b;
  if (nameA !== nameB) return `${nameA} → ${nameB}`;
  const code = (id: string) => ni.stop_codes?.[id] || id;
  return `${nameA} (${code(a)}) → ${nameB} (${code(b)})`;
}

/** OPR_008: motorun bildirdiği tüm hızlı bölümler (bad_seg_<i>_a → bad_seg_<i>_b). */
function badSegments(d: Record<string, string>): Array<[string, string]> {
  const out: Array<[string, string]> = [];
  for (let i = 0; d[`bad_seg_${i}_a`] && d[`bad_seg_${i}_b`]; i++) out.push([d[`bad_seg_${i}_a`], d[`bad_seg_${i}_b`]]);
  return out;
}

/** Takvim görünümü: haftanın hangi günleri çalıştığı ve geçerlilik aralığı. */
function serviceBlock(serviceId: string | null | undefined, result: ValidationResult, includeCode = true): string {
  if (!serviceId) return '';
  const entry = result.name_index.service_days?.[serviceId];
  if (!entry) return '';
  const [mask, start, end, exceptions] = entry;
  const fmt = new Intl.DateTimeFormat(intlLocale(), { weekday: 'short', timeZone: 'UTC' });
  const ymd = (v: string) => (/^\d{8}$/.test(v)
    ? new Intl.DateTimeFormat(intlLocale(), { day: '2-digit', month: '2-digit', year: 'numeric', timeZone: 'UTC' })
      .format(new Date(Date.UTC(Number(v.slice(0, 4)), Number(v.slice(4, 6)) - 1, Number(v.slice(6, 8)))))
    : v);
  const days = mask
    ? [...mask].map((on, i) => `<span class="planner-day${on === '1' ? ' on' : ''}">${escHtml(fmt.format(new Date(Date.UTC(2026, 0, 5 + i))))}</span>`).join('')
    : `<span class="planner-day-note">${escHtml(t('producer.days.dates'))}</span>`;
  const range = start || end ? `${ymd(start)} – ${ymd(end)}` : '';
  const extra = Number(exceptions) > 0 ? ` · ${t('producer.days.exceptions', { n: exceptions })}` : '';
  const title = includeCode
    ? escHtml(t('producer.calendar.title', { id: serviceId }))
    : escHtml(t('producer.calendar.generic_title'));
  return `<div class="planner-calendar"><span class="producer-card-label">${title}</span>
    <div class="planner-days">${days}</div><p class="planner-calendar-range">${escHtml(range + extra)}</p></div>`;
}

/** Yolcu uygulamasında hat aranınca: aynı adı taşıyan hatlar alt alta, ayırt edilemez. */
function searchView(n: Notice, result: ValidationResult): string {
  const routeId = routeIdOfNotice(n);
  if (!['RTS_019', 'RTS_026', 'RTS_027'].includes(n.rule_id) || !routeId) return '';
  const ni = result.name_index;
  const nameOf = (id: string) => n.rule_id === 'RTS_027' ? ni.route_long_names?.[id] || ni.routes?.[id] : ni.routes?.[id];
  const name = nameOf(routeId);
  if (!name) return '';
  const same = Object.keys(ni.routes ?? {}).filter(id => nameOf(id) === name).slice(0, 6);
  if (same.length < 2) return '';
  return `<div class="planner-search-view"><span class="producer-card-label">${t('producer.seen.search')}</span>
    <div class="planner-search-box">${escHtml(name)}</div>
    ${same.map(id => `<div class="planner-search-hit">${routeBadgeHtml(id, result)}<span>${escHtml(ni.routes?.[id] ?? id)}</span><span class="planner-search-kind">${escHtml(t('producer.filter.route'))}</span></div>`).join('')}
  </div>`;
}

/** "Ne yapmalı" cümlelerini numaralı adımlara böler. */
function stepsHtml(action: string): string {
  // A sentence ends at a full stop, but not at an abbreviation ("(ör. FF0000)", "e.g. 5").
  const parts = action.split(/(?<=[.。])(?<!(?:^|[\s(])(?:ör|örn|vb|e\.g|i\.e|ex|p\.ex)\.)\s+/).map(p => p.trim()).filter(Boolean);
  return parts.length > 1
    ? `<ol class="planner-steps">${parts.map(p => `<li>${escHtml(p)}</li>`).join('')}</ol>`
    : `<p class="producer-card-remediation">${escHtml(action)}</p>`;
}

/** Kartın sağ üstündeki sayının birimi: sefer, durak, hat... */
function unitOf(g: RuleGroup): string {
  const n = g.notices[0];
  if (n.rule_id === 'STM_006') return t('producer.unit.stop_time');
  // If the summary carries a row count (affected_rows), or the same trip occurs multiple times, the count is
  // rows rather than trips (STM_003: every malformed stop time in a trip; STM_007 summary: 60,010 stop times).
  if (n.details?.affected_rows && n.file === 'stop_times.txt') return t('producer.unit.stop_time');
  const trips = g.notices.filter(item => item.entity_type === 'Trip').map(item => item.entity_id);
  if (n.entity_type === 'Trip' && new Set(trips).size < trips.length) {
    return t(n.file === 'stop_times.txt' ? 'producer.unit.stop_time' : 'producer.card.affected_short');
  }
  // A group mixing routes, stops and rows (DQ_018/019 names) counts records, not the first one's kind.
  if (new Set(g.notices.map(item => item.entity_type)).size > 1) return t('producer.card.affected_short');
  const unit = n.entity_id ? ({ Trip: 'trip', Stop: 'stop', Route: 'route', Shape: 'shape', Service: 'service' } as Record<string, string>)[n.entity_type] : undefined;
  return t(unit ? `producer.unit.${unit}` : 'producer.card.affected_short');
}

interface PlannerMap { opts: MapOptions; caption: string; }

interface MissingStopPathIndex {
  byDirection: Map<string, string>;
  byRoute: Map<string, string[]>;
}
const missingStopPathCache = new WeakMap<object, MissingStopPathIndex>();

/** Find route-trip geometry; prefer the same passenger-facing direction when it is available. */
function representativeTripsForDirection(ni: ValidationResult['name_index'], tripId: string): string[] {
  const routeId = ni.trip_routes?.[tripId];
  if (!routeId) return [];
  const direction = ni.trips?.[tripId] || ni.trip_directions?.[tripId] || '';
  const departure = ni.trip_first_dep?.[tripId] || '';
  let index = missingStopPathCache.get(ni);
  if (!index) {
    const byDirection = new Map<string, string>();
    const byRoute = new Map<string, string[]>();
    const routePatterns = new Map<string, Set<string>>();
    for (const [candidateId, candidateRoute] of Object.entries(ni.trip_routes ?? {})) {
      const candidateDirection = ni.trips?.[candidateId] || ni.trip_directions?.[candidateId] || '';
      const hasShape = Boolean(ni.trip_shapes?.[candidateId] && (ni.map_data_deferred || ni.shape_coords?.[ni.trip_shapes[candidateId]]?.length));
      const stops = ni.trip_stops?.[candidateId] ?? [];
      const pathStops = stops.filter(stopId => Boolean(ni.stop_coords?.[stopId]));
      const hasPath = pathStops.length > 1;
      if (!hasShape && !hasPath) continue;
      const directionKey = `${candidateRoute}\u0000${candidateDirection}`;
      if (candidateDirection && !byDirection.has(directionKey)) byDirection.set(directionKey, candidateId);
      const patternKey = ni.trip_shapes?.[candidateId] || pathStops.join('|');
      const seen = routePatterns.get(candidateRoute) ?? new Set<string>();
      if (!seen.has(patternKey) && seen.size < 4) {
        seen.add(patternKey);
        routePatterns.set(candidateRoute, seen);
        const candidates = byRoute.get(candidateRoute) ?? [];
        candidates.push(candidateId);
        byRoute.set(candidateRoute, candidates);
      }
    }
    index = { byDirection, byRoute };
    missingStopPathCache.set(ni, index);
  }
  const exact = direction ? index.byDirection.get(`${routeId}\u0000${direction}`) : undefined;
  if (exact) return [exact];
  const candidates = index.byRoute.get(routeId) ?? [];
  if (candidates.length < 2 || !departure) return candidates.slice(0, 1);
  const toSeconds = (time: string) => {
    const parts = time.split(':').map(Number);
    return (parts[0] ?? 0) * 3600 + (parts[1] ?? 0) * 60 + (parts[2] ?? 0);
  };
  const targetTime = toSeconds(departure);
  const nearest = candidates
    .map(id => ({ id, difference: Math.abs(toSeconds(ni.trip_first_dep?.[id] || '00:00:00') - targetTime) }))
    .sort((a, b) => a.difference - b.difference)[0]?.id;
  return nearest ? [nearest] : [];
}

function routeDirectionShapeId(ni: ValidationResult['name_index'], routeId?: string, direction?: string): string | undefined {
  if (!routeId) return undefined;
  return (ni.route_shapes?.[routeId] ?? []).find(shapeId =>
    (ni.shape_routes?.[shapeId] ?? []).some(([shapeRoute, shapeDirection]) =>
      shapeRoute === routeId && (direction == null || direction === '' || shapeDirection === direction)));
}

function segmentOnShape(
  shape: [number, number][],
  from: [number, number],
  to: [number, number],
): [number, number][] {
  if (shape.length < 2) return [];
  const nearest = (point: [number, number]) => {
    let best = 0;
    let bestDistance = Number.POSITIVE_INFINITY;
    for (let i = 0; i < shape.length; i++) {
      const lat = shape[i][0] - point[0];
      const lon = shape[i][1] - point[1];
      const distance = lat * lat + lon * lon;
      if (distance < bestDistance) {
        best = i;
        bestDistance = distance;
      }
    }
    return best;
  };
  const a = nearest(from);
  const b = nearest(to);
  if (a === b) return [];
  return shape.slice(Math.min(a, b), Math.max(a, b) + 1);
}

function stm014SampleTripId(n: Notice, result: ValidationResult): string | undefined {
  return (n.details?.trips ?? '').split(',').map(id => id.trim()).find(id =>
    Boolean(id && (result.name_index.trip_shapes?.[id] || result.name_index.trip_stops?.[id]?.length)));
}

/** The drawings this notice's map reads that a large-feed result does not carry yet. The map itself
 *  is the list: running it against a recording index covers every line it would draw. */
export function missingMapShapes(n: Notice, result: ValidationResult): string[] {
  const ni = result.name_index;
  const missing = new Set<string>();
  const shape_coords = new Proxy(ni.shape_coords ?? {}, {
    get(target, key, receiver) {
      if (typeof key === 'string' && !(key in target)) missing.add(key);
      return Reflect.get(target, key, receiver);
    },
  });
  plannerMapRaw(n, { ...result, name_index: { ...ni, shape_coords } });
  return [...missing];
}

/** CAL cards: the other routes on the calendar(s) too, each as its example trip's line in the
 *  route's colour (thinner than the drawn trip), so the planner sees the whole affected network. */
function addCalendarRoutes(map: PlannerMap, n: Notice, result: ValidationResult, drawnTrip: string): void {
  const ni = result.name_index;
  const d = n.details ?? {};
  const lines = map.opts.extraPolylines ?? (map.opts.extraPolylines = []);
  for (let i = 0; d[`route_${i}_route_id`]; i++) {
    const tripId = d[`route_${i}_example_trip_id`];
    if (!tripId || tripId === drawnTrip) continue;
    const own = ni.trip_shapes?.[tripId] ? drawableLine(ni.shape_coords?.[ni.trip_shapes[tripId]]) : undefined;
    const sibling = routeDirectionShapeId(ni, ni.trip_routes?.[tripId], ni.trip_directions?.[tripId]);
    const shape = own && own.length > 1 ? own : sibling ? drawableLine(ni.shape_coords?.[sibling]) : undefined;
    const coords = shape && shape.length > 1
      ? shape
      : (ni.trip_stops?.[tripId] ?? []).map(id => ni.stop_coords?.[id]).filter((p): p is [number, number] => Boolean(p) && !isNullIsland(p!));
    if (coords.length < 2) continue;
    lines.push({ coords, color: routeBadge(d[`route_${i}_route_id`]!, ni).bg ?? MAP_GREY, weight: 3 });
  }
}

const NO_LINE_TRIP_RULES = new Set(['DQ_006', 'TRP_004', 'TRP_019']);

/** A drawing as the map shows it: its 0,0 points (GEO_017's finding) left out, so a line with a
 *  placeholder point does not run out to the ocean on every card. The same array comes back for
 *  the same drawing (lines are compared by identity). */
const drawableLines = new WeakMap<object, [number, number][]>();
function drawableLine(coords: [number, number][] | undefined): [number, number][] | undefined {
  if (!coords || !coords.some(p => isNullIsland(p))) return coords;
  let kept = drawableLines.get(coords);
  if (!kept) {
    kept = coords.filter(p => !isNullIsland(p));
    drawableLines.set(coords, kept);
  }
  return kept;
}

/** Haritayı seçili kayda göre kurar: sefer → kendi güzergâhı ve durakları, işaretli durak
 *  veya bölüm kırmızı ve ona yakınlaşılmış; durak → durağın kendisi; hat → hattın çizimleri.
 *  Konuma bağlı olmayan kayıtta ağın tamamı. */
/** Map for a planner card. Pin and legend labels are built as plain text (feed names, ids) and
 *  map-modal renders them as HTML (fix.ts relies on that for <strong>/<code>), so escape them here once. */
export function plannerMap(n: Notice, result: ValidationResult): PlannerMap {
  const map = plannerMapRaw(n, result);
  map.opts.pins = map.opts.pins.map(p => ({ ...p, label: escHtml(p.label) }));
  if (map.opts.legendItems) map.opts.legendItems = map.opts.legendItems.map(it => ({ ...it, label: escHtml(it.label) }));
  return map;
}

function plannerMapRaw(n: Notice, result: ValidationResult): PlannerMap {
  // STM_050 is a feed summary: its map is the first example row's trip, drawn as a trip finding.
  if (n.rule_id === 'STM_050' && n.entity_type !== 'Trip' && n.details?.example_0_trip_id) {
    return plannerMapRaw({ ...n, entity_type: 'Trip', entity_id: n.details.example_0_trip_id }, result);
  }
  // DQ cards draw one trip with its stops in order: a route record's example trip (DQ_003/004/
  // 018/019/021), a headsign row's own trip (its stop red), a summary's example trip (DQ_006
  // straight between its stops when it has no line; DQ_013; DQ_020 with its last stop — the
  // headsign candidate — red). A route whose trips the index lacks keeps the route view below.
  // TRP_024's record is a vehicle block (its code is the block's, not a trip's): draw its first trip.
  const dqTrip = plannerTripRule(n.rule_id) && (n.entity_type !== 'Trip' || n.rule_id === 'TRP_024') ? dqExampleTrip(n, result.name_index) : undefined;
  // (Once redrawn as its trip, a TRP_024 record is that trip: no further step.)
  if (dqTrip && dqTrip !== n.entity_id) {
    const tripStops = result.name_index.trip_stops?.[dqTrip] ?? [];
    const last = tripStops[tripStops.length - 1];
    const details = n.rule_id === 'DQ_021' || n.entity_type === 'Route' ? {}
      : n.rule_id === 'DQ_020' ? { ...n.details, ...(last ? { stop_id: last } : {}) }
        : n.details;
    const map = plannerMapRaw({ ...n, entity_type: 'Trip', entity_id: dqTrip, details }, result);
    if (CAL_USAGE_RULES.has(n.rule_id) || ['RTS_019', 'RTS_026', 'RTS_027'].includes(n.rule_id)) addCalendarRoutes(map, n, result, dqTrip);
    // RTS: the route's other lines (its other directions and variants) in its colour.
    if (RTS_CONTEXT_RULES.has(n.rule_id)) {
      const ni = result.name_index;
      const routeId = routeIdOfNotice(n);
      const color = routeId ? routeBadge(routeId, ni).bg ?? MAP_GREY : MAP_GREY;
      const drawn = map.opts.extraPolylines ?? [];
      for (const shapeId of (routeId ? ni.route_shapes?.[routeId] ?? [] : []).slice(0, 4)) {
        const coords = drawableLine(ni.shape_coords?.[shapeId]);
        if (coords && coords.length > 1 && !drawn.some(l => l.coords === coords)) drawn.push({ coords, color, weight: 3, arrows: true });
      }
      map.opts.extraPolylines = drawn;
    }
    return map;
  }
  const ni = result.name_index;
  const zeroStops = new Set<string>();
  const coord = (stopId?: string | null): [number, number] | null => {
    const p = stopId ? ni.stop_coords?.[stopId] : undefined;
    if (p && isNullIsland(p)) { zeroStops.add(stopId!); return null; }
    // Beyond ±85° the web map cannot show a point (GEO_022 is about such stops).
    if (p && Math.abs(p[0]) > 85) return null;
    return p ? [p[0], p[1]] : null;
  };
  const routeColor = (routeId?: string) => (routeId ? routeBadge(routeId, ni).bg : null) ?? MAP_GREY;
  const lines: NonNullable<MapOptions['extraPolylines']> = [];
  const segments: NonNullable<MapOptions['extraPolylines']> = [];
  const pins: MapPin[] = [];
  const highlights: NonNullable<MapOptions['highlights']> = [];
  const stops = new Set<string>();
  const pinByStop = new Map<string, MapPin>();
  let focus: [number, number] | undefined;
  let stopRing = MAP_GREY;

  const addShape = (shapeId: string | null | undefined, color: string, weight = 5) => {
    const coords = shapeId ? drawableLine(ni.shape_coords?.[shapeId]) : undefined;
    // A line already drawn (the stop's own routes and its example trip often share one) once.
    if (coords && coords.length > 1 && !lines.some(l => l.coords === coords)) lines.push({ coords, color, weight, arrows: true });
    return Boolean(coords && coords.length > 1);
  };
  // A trip's real line: its own drawing, else the drawing of its route in the same direction.
  // Straight between the stops only when the feed has no drawing for that route and direction.
  const addTripLine = (tripId: string | null | undefined, color: string, weight = 5) => {
    if (!tripId) return;
    if (addShape(ni.trip_shapes?.[tripId], color, weight)) return;
    if (addShape(routeDirectionShapeId(ni, ni.trip_routes?.[tripId], ni.trip_directions?.[tripId]), color, weight)) return;
    const coords = (ni.trip_stops?.[tripId] ?? []).map(id => coord(id)).filter((p): p is [number, number] => p !== null);
    if (coords.length > 1) lines.push({ coords, color, weight: Math.min(weight, 4), arrows: true });
  };
  // A stretch between two stops follows the drawn line; straight only when no line covers it.
  const alongLine = (a: [number, number], b: [number, number]): [number, number][] => {
    const stretch = lines.length ? segmentOnShape(lines[0].coords as [number, number][], a, b) : [];
    return stretch.length > 1 ? stretch : [a, b];
  };
  const addStop = (stopId: string | null | undefined, primary: boolean, sequence?: string | number | null) => {
    if (!stopId) return;
    const makeLabel = () => {
      const name = ni.stops?.[stopId];
      const base = name ? `${name} (${stopId})` : stopId;
      const value = sequence == null ? '' : String(sequence);
      return value && value !== '4294967295'
        ? `${base} · ${t('producer.map.stop_sequence', { sequence: value })}`
        : base;
    };
    if (stops.has(stopId)) {
      // A feed may repeat the same physical stop at adjacent stop_sequence values.
      // If the repeated record is the faulty one, promote its existing pin instead
      // of leaving the map focused on the earlier gray context pin.
      if (primary) {
        const pin = pinByStop.get(stopId);
        if (pin) {
          pin.primary = true;
          pin.small = false;
          pin.color = MAP_RED;
          pin.stroke = '#ffffff';
          // Relabel only with a sequence: a later call without one (the generic `stop_id` pass at
          // the end) must not erase the position an earlier, rule-specific call wrote.
          if (sequence != null) pin.label = makeLabel();
          if (!focus) focus = [pin.lat, pin.lon];
        }
      }
      return;
    }
    const p = coord(stopId);
    if (!p) return;
    stops.add(stopId);
    const pin: MapPin = {
      lat: p[0], lon: p[1], label: makeLabel(),
      primary, small: !primary, color: primary ? MAP_RED : '#ffffff', stroke: primary ? '#ffffff' : stopRing,
    };
    pins.push(pin);
    pinByStop.set(stopId, pin);
    if (primary && !focus) focus = p;
  };

  const routeIdForNotice = n.rule_id === 'STM_006' && n.entity_id ? ni.trip_routes?.[n.entity_id] : undefined;
  const tripLabel = n.rule_id === 'STM_006' && n.entity_id
    ? [routeIdForNotice ? ni.routes?.[routeIdForNotice] : '', ni.trips?.[n.entity_id], ni.trip_first_dep?.[n.entity_id]].filter(Boolean).join(' · ')
    : '';
  // A drawn trip is named as planners know it (route · direction · departure), also when a
  // list-level finding's map draws its example trip; otherwise the list or record name.
  const drawnTrip = n.entity_type === 'Trip' && n.entity_id && ni.trip_stops?.[n.entity_id] ? tripShort(n.entity_id, result) : '';
  // A shape finding names the shape with its routes ("S8 hattının „SA“ güzergâh çizimi").
  const shpShape = SHP_CONTEXT_RULES.has(n.rule_id) ? shpContext(n) : undefined;
  const shpLabel = shpShape?.shape ? shapeText(shpShape.shape, shpShape.routes, result) : '';
  const label = tripLabel || drawnTrip || shpLabel || dqListLabel(n) || plannerEntityLabel(n, result) || n.entity_id || '';
  const shown = label ? t('producer.map.shown', { record: label }) : '';
  const zeroCaption = () => zeroStops.size
    ? t('producer.map.null_island', { stops: [...zeroStops].slice(0, 3).map(id => ni.stops?.[id] || id).join(', ') })
    : '';
  const focusCaption = () => [
    zeroCaption(),
    shown,
    n.rule_id === 'STM_056'
      ? t('producer.map.stm056_focus', {
        from_sequence: n.details?.prev_stop_sequence || '—',
        sequence: n.details?.stop_sequence || '—',
      })
      : t('producer.map.caption_focus'),
  ].filter(Boolean).join(' · ');

  if (n.entity_type === 'Trip' && n.entity_id) {
    const tripStops = ni.trip_stops?.[n.entity_id] ?? [];
    const color = routeColor(ni.trip_routes?.[n.entity_id]);
    const directShape = ni.trip_shapes?.[n.entity_id];
    // Findings about trips without a drawing keep showing them as they are: straight between stops.
    const selectedShape = directShape
      || (NO_LINE_TRIP_RULES.has(n.rule_id) ? undefined : routeDirectionShapeId(ni, ni.trip_routes?.[n.entity_id], ni.trip_directions?.[n.entity_id]));
    const directPath = tripStops.map(id => ni.stop_coords?.[id]).filter(Boolean).length > 1;
    const pathTripIds = n.rule_id === 'STM_006' && !directShape && !directPath
      ? representativeTripsForDirection(ni, n.entity_id)
      : [n.entity_id];
    for (const pathTripId of pathTripIds) {
      const shapeId = pathTripId === n.entity_id ? selectedShape : ni.trip_shapes?.[pathTripId];
      // A code pointing at a missing drawing falls back to the route's own line too.
      const sibling = NO_LINE_TRIP_RULES.has(n.rule_id) ? undefined : routeDirectionShapeId(ni, ni.trip_routes?.[pathTripId], ni.trip_directions?.[pathTripId]);
      if (!addShape(shapeId, color) && !addShape(sibling, color)) {
        if (n.rule_id === 'STM_018' || n.rule_id === 'STM_019' || n.rule_id === 'STM_020') continue;
        const pathStops = ni.trip_stops?.[pathTripId] ?? [];
        const coords = pathStops.map(id => coord(id)).filter((p): p is [number, number] => p !== null);
        if (coords.length > 1) lines.push({ coords, color, weight: 4, arrows: true });
      }
    }
    stopRing = color;
    if (n.rule_id === 'STM_006') {
      addStop(n.details?.prev_stop_id, false);
      addStop(n.details?.next_stop_id, false);
    } else if (n.rule_id === 'STM_015' || n.rule_id === 'STM_016') {
      const stopSequences = ni.trip_stop_sequences?.[n.entity_id] ?? [];
      for (const [index, stopId] of tripStops.entries()) addStop(stopId, false, stopSequences[index]);
      if (n.rule_id === 'STM_015') {
        addStop(n.details?.stop_id || tripStops[0], true, n.details?.stop_sequence || stopSequences[0]);
        addStop(n.details?.following_stop_id, false, n.details?.following_stop_sequence);
        addStop(n.details?.following2_stop_id, false, n.details?.following2_stop_sequence);
      } else {
        const lastIndex = Math.max(0, tripStops.length - 1);
        addStop(n.details?.stop_id || tripStops[lastIndex], true, n.details?.stop_sequence || stopSequences[lastIndex]);
        addStop(n.details?.preceding_stop_id, false, n.details?.preceding_stop_sequence);
        addStop(n.details?.preceding2_stop_id, false, n.details?.preceding2_stop_sequence);
      }
    } else if (n.rule_id === 'STM_017') {
      const stopSequences = ni.trip_stop_sequences?.[n.entity_id] ?? [];
      for (const [index, stopId] of tripStops.entries()) addStop(stopId, false, stopSequences[index]);
      const allMissing = Number(n.details?.affected_stop_count) === Number(n.details?.stop_count)
        && Number(n.details?.stop_count) > 0;
      if (allMissing) {
        for (const [index, stopId] of tripStops.entries()) addStop(stopId, true, stopSequences[index]);
      } else {
        for (let i = 0; n.details?.[`missing_${i}_stop_id`]; i++) {
          addStop(n.details[`missing_${i}_stop_id`], true, n.details[`missing_${i}_stop_sequence`]);
        }
      }
    } else if (n.rule_id === 'STM_018' || n.rule_id === 'STM_019') {
      const stopSequences = ni.trip_stop_sequences?.[n.entity_id] ?? [];
      for (const [index, stopId] of tripStops.entries()) addStop(stopId, false, stopSequences[index]);
      addStop(n.details?.stop_id, true, n.details?.stop_sequence);
      addStop(n.details?.next_stop_id, false, n.details?.next_stop_sequence);
      const current = coord(n.details?.stop_id);
      const next = coord(n.details?.next_stop_id);
      const shape = selectedShape ? drawableLine(ni.shape_coords?.[selectedShape]) ?? [] : [];
      if (current && next) {
        const actualSegment = segmentOnShape(shape, current, next);
        if (actualSegment.length > 1) {
          segments.push({ coords: actualSegment, color: MAP_RED, weight: 8, zoomTo: true });
        }
        focus = undefined;
      }
    } else if (n.rule_id === 'STM_060') {
      // The two overlapping zones outlined in red (the first one zoomed to), the trip's stops grey.
      for (const [index, key] of ['zone_a_ring', 'zone_b_ring'].entries()) {
        const coords = (n.details?.[key] ?? '').split(';')
          .map(point => point.split(',').map(Number))
          .filter((p): p is [number, number] => p.length === 2 && p.every(Number.isFinite));
        if (coords.length > 1) segments.push({ coords, color: MAP_RED, weight: 4, zoomTo: index === 0 });
      }
      const stopSequences = ni.trip_stop_sequences?.[n.entity_id] ?? [];
      for (const [index, stopId] of tripStops.entries()) addStop(stopId, false, stopSequences[index]);
    } else if (['STM_032', 'STM_033', 'STM_034', 'STM_035', 'STM_041', 'STM_042', 'STM_045', 'STM_047', 'STM_050', 'STM_048', 'STM_049', 'STM_051', 'STM_052', 'STM_054', 'STM_055', 'STM_058', 'STM_059'].includes(n.rule_id)) {
      // STM_051/052/054/055 on a zone row have no stop to mark (zone shapes are not in the name
      // index): the route and its stops are drawn; a stop-based Flex row gets its red pin.
      // Stop-row findings: the route and direction with every stop, the faulty stop(s) in red,
      // the neighbours of a repeated stop in grey.
      const stopSequences = ni.trip_stop_sequences?.[n.entity_id] ?? [];
      const d = n.details ?? {};
      if (n.rule_id === 'STM_047') {
        for (let i = 0; d[`example_${i}_stop_id`]; i++) addStop(d[`example_${i}_stop_id`], true, d[`example_${i}_stop_sequence`]);
      } else if (n.rule_id === 'STM_050') {
        // Only this trip's example rows are red; other trips' examples are not on this line.
        for (let i = 0; d[`example_${i}_stop_id`]; i++) {
          if (d[`example_${i}_trip_id`] === n.entity_id) addStop(d[`example_${i}_stop_id`], true, d[`example_${i}_stop_sequence`]);
        }
      } else if (n.rule_id === 'STM_032') {
        addStop(d.first_stop_id, true, n.observed_value);
        addStop(d.second_stop_id, true, n.observed_value);
      } else if (n.rule_id === 'STM_035') {
        addStop(d.stop_id, true, d.stop_a_sequence);
        addStop(d.prev_stop_id, false, d.prev_stop_sequence);
        addStop(d.next_stop_id, false, d.next_stop_sequence);
      } else {
        addStop(d.stop_id, true, d.stop_sequence);
      }
      for (const [index, stopId] of tripStops.entries()) addStop(stopId, false, stopSequences[index]);
    } else if (n.rule_id === 'STM_028' || n.rule_id === 'STM_029' || n.rule_id === 'STM_030' || n.rule_id === 'STM_043') {
      // Whole-trip findings: the route and direction with every stop, the faulty place in red.
      const stopSequences = ni.trip_stop_sequences?.[n.entity_id] ?? [];
      const d = n.details ?? {};
      if (n.rule_id === 'STM_029' || n.rule_id === 'STM_043') {
        // STM_043 has no faulty stop: the trip's two ends mark where the long trip starts and ends.
        addStop(d.first_stop_id, true, d.first_stop_sequence);
        addStop(d.last_stop_id, true, d.last_stop_sequence);
      } else if (n.rule_id === 'STM_028') {
        addStop(d.gap_stop_a, true, d.gap_a_sequence);
        addStop(d.gap_stop_b, true, d.gap_b_sequence);
        const from = coord(d.gap_stop_a);
        const to = coord(d.gap_stop_b);
        if (from && to) {
          const shape = selectedShape ? drawableLine(ni.shape_coords?.[selectedShape]) ?? [] : [];
          const onShape = segmentOnShape(shape, from, to);
          segments.push({ coords: onShape.length > 1 ? onShape : [from, to], color: MAP_RED, weight: 8, zoomTo: true });
        }
      } else {
        addStop(d.stop_id, true, d.stop_sequence);
        addStop(d.prev_stop_id, false, d.prev_stop_sequence);
        addStop(d.next_stop_id, false, d.next_stop_sequence);
      }
      for (const [index, stopId] of tripStops.entries()) addStop(stopId, false, stopSequences[index]);
    } else if (n.rule_id === 'STM_021') {
      // Both stops sit on the same point: draw one red pin that names both, with the trip
      // and its neighbouring stops as context.
      const stopSequences = ni.trip_stop_sequences?.[n.entity_id] ?? [];
      const d = n.details ?? {};
      addStop(d.stop_a, true, d.stop_a_sequence);
      const pin = d.stop_a ? pinByStop.get(d.stop_a) : undefined;
      if (pin && d.stop_b) {
        pin.label = t('producer.map.stm021_same_place', {
          from: ni.stops?.[d.stop_a!] || d.stop_a!, to: ni.stops?.[d.stop_b] || d.stop_b,
          from_sequence: d.stop_a_sequence || '—', to_sequence: d.stop_b_sequence || '—',
        });
        stops.add(d.stop_b);
      } else {
        addStop(d.stop_b, true, d.stop_b_sequence);
      }
      addStop(d.prev_stop_id, false, d.prev_stop_sequence);
      addStop(d.next_stop_id, false, d.next_stop_sequence);
      for (const [index, stopId] of tripStops.entries()) addStop(stopId, false, stopSequences[index]);
    } else if (n.rule_id === 'STM_020') {
      const stopSequences = ni.trip_stop_sequences?.[n.entity_id] ?? [];
      for (const [index, stopId] of tripStops.entries()) addStop(stopId, false, stopSequences[index]);
      addStop(n.details?.stop_a, true, n.details?.stop_a_sequence);
      addStop(n.details?.stop_b, true, n.details?.stop_b_sequence);
      const from = coord(n.details?.stop_a);
      const to = coord(n.details?.stop_b);
      const shape = selectedShape ? drawableLine(ni.shape_coords?.[selectedShape]) ?? [] : [];
      if (from && to) {
        const actualSegment = segmentOnShape(shape, from, to);
        if (actualSegment.length > 1) {
          segments.push({ coords: actualSegment, color: MAP_RED, weight: 8, zoomTo: true });
          focus = undefined;
        }
      }
    } else if (n.rule_id === 'STM_056') {
      // STM_056 is a directional trip finding: the distance fails to grow on the step from the
      // previous stop to the faulty one, so both ends of that step are red and the step is drawn
      // along the route line. Two stops at (almost) one point leave a zero-length line, which is
      // why both pins carry the mark; the next stops stay grey context.
      const stopSequences = ni.trip_stop_sequences?.[n.entity_id] ?? [];
      addStop(n.details?.prev_stop_id, true, n.details?.prev_stop_sequence);
      addStop(n.details?.stop_id, true, n.details?.stop_sequence);
      addStop(n.details?.prev2_stop_id, false, n.details?.prev2_stop_sequence);
      addStop(n.details?.next_stop_id, false, n.details?.next_stop_sequence);
      addStop(n.details?.next2_stop_id, false, n.details?.next2_stop_sequence);
      for (const [index, stopId] of tripStops.entries()) addStop(stopId, false, stopSequences[index]);
      const previous = coord(n.details?.prev_stop_id);
      const current = coord(n.details?.stop_id);
      if (previous && current) {
        const shape = selectedShape ? drawableLine(ni.shape_coords?.[selectedShape]) ?? [] : [];
        const onShape = segmentOnShape(shape, previous, current);
        segments.push({ coords: onShape.length > 1 ? onShape : [previous, current], color: MAP_RED, weight: 8, zoomTo: true });
        // Circle the step: two stops at (almost) one point make the red line invisible.
        const half = metersBetween(previous, current) / 2;
        highlights.push({
          center: [(previous[0] + current[0]) / 2, (previous[1] + current[1]) / 2],
          radiusMeters: Math.max(60, half + 40),
          color: MAP_RED,
        });
      }
    } else {
  // Stop indicated by the rule: the endpoint when its time is missing, otherwise the notice's stops.
      if (n.rule_id === 'STM_015') addStop(tripStops[0], true);
      if (n.rule_id === 'STM_016') addStop(tripStops[tripStops.length - 1], true);
      addStop(n.details?.stop_id, true);
      addStop(n.details?.bad_stop, true);
      addStop(n.details?.first_stop_id, true);
      addStop(n.details?.second_stop_id, true);
      addStop(n.details?.last_stop_id, true);
      addStop(n.details?.dup_stop, true);
      const pairs: Array<[string | undefined, string | undefined]> = [[n.details?.stop_a, n.details?.stop_b], ...(n.details ? badSegments(n.details) : [])];
      for (const [idA, idB] of pairs) {
        const a = coord(idA);
        const b = coord(idB);
        if (!a || !b) continue;
        segments.push({ coords: alongLine(a, b), color: MAP_RED, weight: 7 });
        addStop(idA, true);
        addStop(idB, true);
      }
      // DQ cards list the trip stop by stop, so its pins carry the stop sequence too.
      const dqSequences = plannerTripRule(n.rule_id) ? ni.trip_stop_sequences?.[n.entity_id] ?? [] : [];
      for (const [index, stopId] of tripStops.entries()) addStop(stopId, false, dqSequences[index]);
    }
  } else if (n.entity_type === 'Stop' && n.entity_id) {
    addStop(n.entity_id, true);
    const here = coord(n.entity_id);
    if (here) {
  // Context: up to three routes using the stop and stops within 600 m.
      const routesHere = new Map<string, string>();
      for (const [tripId, tripStops] of Object.entries(ni.trip_stops ?? {})) {
        if (routesHere.size >= 3) break;
        const routeId = ni.trip_routes?.[tripId];
        if (!routeId || routesHere.has(routeId) || !tripStops.includes(n.entity_id)) continue;
        routesHere.set(routeId, tripId);
      }
      for (const [routeId, tripId] of routesHere) addShape(ni.trip_shapes?.[tripId] || routeDirectionShapeId(ni, routeId, ni.trip_directions?.[tripId]), routeColor(routeId));
      const near = Object.entries(ni.stop_coords ?? {})
        .filter(([, p]) => Math.abs(p[0] - here[0]) < 0.0055 && Math.abs(p[1] - here[1]) < 0.0075)
        .slice(0, 150);
      for (const [stopId] of near) addStop(stopId, false);
    }
  } else if (n.entity_type === 'Route' && routeIdOfNotice(n)) {
    const routeId = routeIdOfNotice(n)!;
    const color = routeColor(routeId);
    stopRing = color;
    if (n.rule_id === 'STM_014' || n.rule_id === 'STM_061') {
      // STM_061 draws its fastest example trip; STM_014 the first sample with a line.
      const sampleTripId = (n.rule_id === 'STM_061' ? n.details?.example_trip_id : undefined) || stm014SampleTripId(n, result);
      const tripStops = sampleTripId ? ni.trip_stops?.[sampleTripId] ?? [] : [];
      const stopSequences = sampleTripId ? ni.trip_stop_sequences?.[sampleTripId] ?? [] : [];
      const sampleShape = (sampleTripId ? ni.trip_shapes?.[sampleTripId] : undefined)
        || routeDirectionShapeId(ni, routeId, n.details?.direction_id);
      if (!addShape(sampleShape, color)) {
        const coords = tripStops.map(id => coord(id)).filter((p): p is [number, number] => p !== null);
        if (coords.length > 1) lines.push({ coords, color, weight: 4, arrows: true });
      }
      for (const [index, stopId] of tripStops.entries()) addStop(stopId, false, stopSequences[index]);
    } else if (n.rule_id === 'STM_053' && n.details?.example_trip_id) {
      // The example trip's line and stops in grey, the stops sharing one time in red.
      const tripId = n.details.example_trip_id;
      const tripStops = ni.trip_stops?.[tripId] ?? [];
      const stopSequences = ni.trip_stop_sequences?.[tripId] ?? [];
      const tripShape = ni.trip_shapes?.[tripId]
        || routeDirectionShapeId(ni, routeId, ni.trip_directions?.[tripId]);
      if (!addShape(tripShape, color)) {
        const coords = tripStops.map(id => coord(id)).filter((p): p is [number, number] => p !== null);
        if (coords.length > 1) lines.push({ coords, color, weight: 4, arrows: true });
      }
      for (const row of stm053Rows(n.details).filter(r => r.run)) addStop(row.stop, true, row.sequence);
      for (const [index, stopId] of tripStops.entries()) addStop(stopId, false, stopSequences[index]);
    } else {
      for (const shapeId of (ni.route_shapes?.[routeId] ?? []).slice(0, 4)) addShape(shapeId, color);
      if (!lines.length && (n.rule_id === 'DQ_003' || n.rule_id === 'DQ_004')) {
        // A route without a line: its first trip's stops, so the map still shows which route it is.
        const tripId = Object.keys(ni.trip_routes ?? {}).find(id => ni.trip_routes?.[id] === routeId && (ni.trip_stops?.[id]?.length ?? 0) > 1);
        const tripStops = tripId ? ni.trip_stops?.[tripId] ?? [] : [];
        const coords = tripStops.map(id => coord(id)).filter((p): p is [number, number] => p !== null);
        if (coords.length > 1) lines.push({ coords, color, weight: 4, arrows: true });
        for (const stopId of tripStops) addStop(stopId, false);
      }
    }
  // Route-level speed findings (STM_014, STM_061) carry the faulty stop pair.
    const a = coord(n.details?.stop_a);
    const b = coord(n.details?.stop_b);
    if (a && b) {
      segments.push({ coords: alongLine(a, b), color: MAP_RED, weight: 7 });
      addStop(n.details?.stop_a, true, n.details?.stop_a_sequence);
      addStop(n.details?.stop_b, true, n.details?.stop_b_sequence);
    }
    if (n.rule_id === 'STM_014') {
      addStop(n.details?.prev_stop_id, false);
      addStop(n.details?.next_stop_id, false);
    }
  } else if (n.entity_type === 'Shape' && n.entity_id) {
    addShape(n.entity_id, routeColor(ni.shape_routes?.[n.entity_id]?.[0]?.[0]));
  }
  if (n.rule_id !== 'STM_018' && n.rule_id !== 'STM_019') addStop(n.details?.stop_id, true);
  // DQ_020: the whole example trip stays in view; its red last stop is only the headsign candidate.
  if (n.rule_id === 'DQ_020') focus = undefined;
  // DQ_022: the stops sharing one name; DQ_021 on stops.txt: the repeated stop codes.
  if (n.rule_id === 'DQ_022') {
    for (let i = 0; n.details?.[`example_${i}_stop_id`]; i++) addStop(n.details[`example_${i}_stop_id`], true);
  }
  // TRP_022: the other trip of the block too, so both overlapping trips are on the map.
  if (n.rule_id === 'TRP_022' && n.details?.other_trip_id) {
    addTripLine(n.details.other_trip_id, MAP_RED, 3);
  }
  // SHP: the shape in its route's colour (SHP_001: the shape of the rows around the point), the
  // example trip's stops grey and numbered, and the faulty spot red — the point, the stretch a
  // point without a position belongs to (circled), the crossing (circled), the far stops.
  if (SHP_CONTEXT_RULES.has(n.rule_id)) {
    const d = n.details ?? {};
    const c = shpContext(n);
    const routeId = c.routes[0] || (c.shape ? ni.shape_routes?.[c.shape]?.[0]?.[0] : undefined);
    if (c.shape && !lines.some(l => l.coords === drawableLine(ni.shape_coords?.[c.shape!]))) addShape(c.shape, routeColor(routeId));
    const ownTrip = n.entity_type === 'Trip' ? n.entity_id ?? undefined : ['SHP_014', 'SHP_016', 'SHP_024'].includes(n.rule_id) ? d.trip_id : undefined;
    const trip = dqIndexKey(ni.trip_stops, ownTrip || c.at('example_trip_id'));
    if (trip) {
      const sequences = ni.trip_stop_sequences?.[trip] ?? [];
      for (const [index, stopId] of (ni.trip_stops?.[trip] ?? []).entries()) addStop(stopId, false, sequences[index]);
      if (!c.shape || !ni.shape_coords?.[c.shape]) addTripLine(trip, routeColor(ni.trip_routes?.[trip]), 3);
    }
    const pt = (lat?: string, lon?: string): [number, number] | null => {
      const p: [number, number] = [Number(lat), Number(lon)];
      return lat && lon && p.every(Number.isFinite) && Math.abs(p[0]) <= 90 && Math.abs(p[1]) <= 180 && !isNullIsland(p) ? p : null;
    };
    const pin = (p: [number, number] | null, label: string) => {
      if (p) pins.push({ lat: p[0], lon: p[1], label, primary: true, color: MAP_RED, stroke: '#ffffff' });
    };
    const circle = (p: [number, number] | null, radius: number) => {
      if (p) highlights.push({ center: p, radiusMeters: radius, color: MAP_RED });
    };
    const row = (line?: string | number | null) => t('producer.times.stp_row', { line: String(line ?? '—') });
    const here = pt(d.lat, d.lon);
    if (n.rule_id === 'SHP_002' || n.rule_id === 'SHP_003') {
      const a = pt(d.prev_lat, d.prev_lon);
      const b = pt(d.next_lat, d.next_lon);
      if (a && b) {
        segments.push({ coords: [a, b], color: MAP_RED, weight: 7, zoomTo: true });
        circle([(a[0] + b[0]) / 2, (a[1] + b[1]) / 2], Math.max(60, metersBetween(a, b) / 2 + 40));
      }
    } else if (n.rule_id === 'SHP_005') {
      const p = pt(c.at('lat'), c.at('lon'));
      const q = pt(c.at('prev_lat'), c.at('prev_lon'));
      pin(p, t('producer.times.shp_point', { seq: c.at('shape_pt_sequence') || '—' }));
      if (p && q) segments.push({ coords: [q, p], color: MAP_RED, weight: 7, zoomTo: true });
    } else if (n.rule_id === 'SHP_008') {
      const first = pt(d.first_lat, d.first_lon);
      pin(first, row(d.first_line));
      pin(here, row(n.line));
      if (first && here) segments.push({ coords: [first, here], color: MAP_RED, weight: 3 });
    } else if (n.rule_id === 'SHP_009') {
      const p = pt(d.cross_lat, d.cross_lon);
      circle(p, 80);
      pin(p, t('producer.times.shp_cross'));
    } else if (n.rule_id === 'SHP_012') {
      for (const id of listOf(d.far_stops)) addStop(id, true);
      focus = undefined;
    } else if (n.rule_id === 'SHP_014' || n.rule_id === 'SHP_016') {
      // The stop and the line's end it should meet (SHP_016: the line's start), linked in red.
      const shapePts = c.shape ? drawableLine(ni.shape_coords?.[c.shape]) ?? [] : [];
      const end = n.rule_id === 'SHP_014' && d.endpoint === 'end' ? shapePts[shapePts.length - 1] : shapePts[0];
      const stopId = n.rule_id === 'SHP_014' ? d.problem_stop : d.first_stop;
      addStop(stopId, true);
      const label = t(n.rule_id === 'SHP_014' && d.endpoint === 'end' ? 'producer.times.shp_line_end' : 'producer.times.shp_line_start');
      pin(end ? [end[0], end[1]] : null, label);
      const s = coord(stopId);
      if (end && s) segments.push({ coords: [[end[0], end[1]], s], color: MAP_RED, weight: 4 });
      focus = undefined;
    } else if (n.rule_id === 'SHP_015') {
      // The few points the line is made of.
      for (const p of (c.shape ? ni.shape_coords?.[c.shape] ?? [] : []).slice(0, 50)) {
        pins.push({ lat: p[0], lon: p[1], label: t('producer.times.shp_point_plain'), primary: false, small: true, color: MAP_RED, stroke: '#ffffff' });
      }
    } else if (n.rule_id === 'SHP_017') {
      addStop(d.bad_stop, true);
    } else if (n.rule_id === 'SHP_024') {
      // The stop and the place on the line its written distance points to, linked in red.
      const implied = pt(d.implied_lat, d.implied_lon);
      pin(implied, t('producer.times.shp_implied'));
      const s = coord(n.entity_id);
      if (implied && s) segments.push({ coords: [s, implied], color: MAP_RED, weight: 4 });
      focus = undefined;
    } else if (n.rule_id === 'SHP_028' || n.rule_id === 'SHP_029') {
      const a = pt(d.lat, d.lon);
      const b = pt(d.next_lat, d.next_lon);
      pin(a, t('producer.times.shp_same_dist'));
      pin(b, t('producer.times.shp_same_dist'));
      if (a && b) segments.push({ coords: [a, b], color: MAP_RED, weight: 6, zoomTo: true });
      circle(a, 40);
    } else if (n.rule_id === 'SHP_025' || n.rule_id === 'SHP_026' || n.rule_id === 'SHP_030') {
      // The shape and the trip's stops are the evidence.
    } else if (n.rule_id === 'SHP_020' || n.rule_id === 'SHP_023') {
      pin(here, t('producer.times.shp_repeat'));
      circle(here, 60);
    } else if (n.rule_id === 'SHP_018' || n.rule_id === 'SHP_019' || n.rule_id === 'SHP_022') {
      // The shape (and SHP_022's stop, drawn above) is the evidence.
    } else {
      // SHP_001/004/006/010: the point itself.
      pin(here, row(n.line));
      circle(here, 60);
    }
  }
  // STP: the stop on its example trip (line and grey stops); a stop without a usable position is
  // shown by the stretch between the stops before and after it (red, circled). A repeated code's
  // two positions, a code-less row's position, the nearest station or the parent are red too.
  if (STP_CONTEXT_RULES.has(n.rule_id)) {
    const d = n.details ?? {};
    const c = stpContext(n);
    const trip = dqIndexKey(ni.trip_stops, c.trip);
    if (trip) {
      const tripStops = ni.trip_stops?.[trip] ?? [];
      const sequences = ni.trip_stop_sequences?.[trip] ?? [];
      const color = routeColor(ni.trip_routes?.[trip]);
      addTripLine(trip, color);
      if (c.stop && coord(c.stop)) addStop(c.stop, true, c.seq);
      for (const [index, stopId] of tripStops.entries()) addStop(stopId, false, sequences[index]);
    }
    const a = coord(c.prev);
    const b = coord(c.next);
    if (c.stop && !coord(c.stop) && a && b) {
      segments.push({ coords: alongLine(a, b), color: MAP_RED, weight: 7, zoomTo: true });
      const half = metersBetween(a, b) / 2;
      highlights.push({ center: [(a[0] + b[0]) / 2, (a[1] + b[1]) / 2], radiusMeters: Math.max(80, half + 60), color: MAP_RED });
      focus = undefined;
    }
    const rowPin = (lat?: string, lon?: string, label?: string) => {
      const la = Number(lat);
      const lo = Number(lon);
      if (!lat || !lon || !Number.isFinite(la) || !Number.isFinite(lo) || Math.abs(la) > 90 || Math.abs(lo) > 180 || isNullIsland([la, lo])) return null;
      pins.push({ lat: la, lon: lo, label: label || '—', primary: true, color: MAP_RED, stroke: '#ffffff' });
      return [la, lo] as [number, number];
    };
    const drawTrip = (tripId?: string, mark?: string, weight = 3) => {
      const key = dqIndexKey(ni.trip_stops, tripId);
      if (!key) return;
      addTripLine(key, routeColor(ni.trip_routes?.[key]), weight);
      for (const stopId of ni.trip_stops?.[key] ?? []) addStop(stopId, stopId === mark);
    };
    const link = (from?: string | null, to?: string | null) => {
      const p = coord(from);
      const q = coord(to);
      if (p && q) segments.push({ coords: [p, q], color: MAP_RED, weight: 3 });
    };
    if (n.rule_id === 'STP_016' || n.rule_id === 'STP_017' || n.rule_id === 'STP_039') {
      // The other stop red with its own trip, both stops linked.
      addStop(d.other_stop_id, true);
      drawTrip(stpContext(n, 'other').trip);
      link(n.entity_id, d.other_stop_id);
      // Both stops in view (a repeated number may sit far apart).
      focus = undefined;
    } else if (n.rule_id === 'STP_020') {
      // The unused stop red; the used stop nearby red with its trip, both linked.
      addStop(n.entity_id, true);
      addStop(d.nearest_used_stop_id, true);
      drawTrip(stpContext(n, 'nearest').trip, undefined, 4);
      link(n.entity_id, d.nearest_used_stop_id);
      focus = undefined;
    } else if (n.rule_id === 'STP_011') {
      addStop(n.entity_id, true);
      addStop(d.nearest_station_id, true);
      link(n.entity_id, d.nearest_station_id);
    } else if (n.rule_id === 'STP_034' || n.rule_id === 'STP_035') {
      // Every stop repeating the address red; STP_035 the route whose address it is.
      for (const id of listOf(d.representative_stop_ids)) addStop(id, true);
      for (const routeId of listOf(d.route_ids).slice(0, 3)) drawTrip(routeExampleTrip(ni, routeId));
      focus = undefined;
    } else if (n.rule_id === 'STP_021' || n.rule_id === 'STP_036' || n.rule_id === 'STP_041') {
      addStop(n.entity_id, true);
      addStop(d.parent_station, true);
      link(n.entity_id, d.parent_station);
    } else if (n.rule_id === 'STP_029') {
      // The stop, its far parent and a nearer station, each linked to the stop.
      addStop(n.entity_id, true);
      addStop(d.parent_id, true);
      link(n.entity_id, d.parent_id);
      addStop(d.nearest_station_id, true);
      link(n.entity_id, d.nearest_station_id);
      focus = undefined;
    } else if (n.rule_id === 'STP_030') {
      // The station and the stops around it that belong to no station (likely its platforms).
      addStop(n.entity_id, true);
      for (const id of listOf(d.nearby_stops)) {
        addStop(id, false);
        link(n.entity_id, id);
      }
    } else if ((n.rule_id === 'STP_032' || n.rule_id === 'STP_045') && n.entity_id) {
      let i = 0;
      while (d[`example_${i}_stop_id`] && d[`example_${i}_stop_id`] !== n.entity_id) i++;
      addStop(n.entity_id, true);
      addStop(d[`example_${i}_nearest_station_id`], true);
      link(n.entity_id, d[`example_${i}_nearest_station_id`]);
    } else if (n.rule_id === 'STP_012') {
      // The station red, its platforms grey: where the trips should call instead.
      for (const id of listOf(d.platforms)) addStop(id, false);
    }
    if (n.rule_id === 'STP_001') {
      const p1 = rowPin(d.first_lat, d.first_lon, `${d.first_name || n.entity_id} · ${t('producer.times.stp_row', { line: d.first_line || '—' })}`);
      const p2 = rowPin(d.lat, d.lon, `${d.name || n.entity_id} · ${t('producer.times.stp_row', { line: String(n.line ?? '—') })}`);
      if (p1 && p2) segments.push({ coords: [p1, p2], color: MAP_RED, weight: 3 });
      focus = undefined;
    } else if (n.rule_id === 'STP_002') {
      if (rowPin(d.stop_lat, d.stop_lon, d.stop_name)) focus = undefined;
    } else if (n.rule_id === 'STP_009' || n.rule_id === 'STP_010') {
      const other = n.rule_id === 'STP_009' ? d.nearest_station_id : d.parent_station;
      addStop(other, true);
      const here = coord(n.entity_id);
      const there = coord(other);
      if (here && there) segments.push({ coords: [here, there], color: MAP_RED, weight: 3 });
    }
  }
  // GEO: a stop on its example trip (the network's middle and a likely correct position for a
  // far-off stop; the stretch where a 0,0 stop belongs; the nearest point of the line; the
  // cluster's stops), a drawing with its long stretch or its 0,0 point (left out of the line),
  // the network with its farthest stops.
  if (GEO_CONTEXT_RULES.has(n.rule_id)) {
    const d = n.details ?? {};
    const mark = (p: [number, number] | null, label: string, color = MAP_RED) => {
      if (p) pins.push({ lat: p[0], lon: p[1], label, primary: true, color, stroke: '#ffffff' });
    };
    const num = (a?: string, b?: string): [number, number] | null => {
      const p: [number, number] = [Number(a), Number(b)];
      return a && b && p.every(Number.isFinite) && !isNullIsland(p) ? p : null;
    };
    if (GEO_STOP_RULES.has(n.rule_id)) {
      const c = stpContext(n);
      const trip = dqIndexKey(ni.trip_stops, c.trip);
      if (trip) {
        addTripLine(trip, routeColor(ni.trip_routes?.[trip]));
        const sequences = ni.trip_stop_sequences?.[trip] ?? [];
        for (const [index, stopId] of (ni.trip_stops?.[trip] ?? []).entries()) addStop(stopId, stopId === c.stop, sequences[index]);
      }
      const here = coord(n.entity_id);
      const a = coord(c.prev);
      const b = coord(c.next);
      // A stop without a usable position (0,0, rounded, near the pole): its place on the trip.
      if (GEO_PLACE_RULES.has(n.rule_id) && a && b) {
        segments.push({ coords: alongLine(a, b), color: MAP_RED, weight: 7, zoomTo: true });
        highlights.push({ center: [(a[0] + b[0]) / 2, (a[1] + b[1]) / 2], radiusMeters: Math.max(80, metersBetween(a, b) / 2 + 60), color: MAP_RED });
        focus = undefined;
      }
      const middle = num(d.med_lat, d.med_lon);
      if (n.rule_id === 'GEO_002' && middle) {
        pins.push({ lat: middle[0], lon: middle[1], label: t('producer.map.geo_middle'), primary: false, color: MAP_GREY, stroke: '#ffffff' });
        focus = undefined;
      }
      const slip = geoStopSlip(n, result);
      if (slip) mark(slip.at, t('producer.map.geo_slip'), '#f59e0b');
      if (n.rule_id === 'GEO_009' && here && d.shape_id) {
        const routeId = ni.shape_routes?.[d.shape_id]?.[0]?.[0];
        addShape(d.shape_id, routeColor(routeId));
        const line = drawableLine(ni.shape_coords?.[d.shape_id]);
        const near = line ? nearestOnLine(line, here) : null;
        if (near) {
          segments.push({ coords: [here, near], color: MAP_RED, weight: 4 });
          mark(near, t('producer.map.geo_nearest'));
        }
      }
      if (n.rule_id === 'GEO_012') {
        for (let i = 0; d[`example_${i}_stop_id`]; i++) addStop(d[`example_${i}_stop_id`], true);
        // The stops sit metres apart: a ring shows where the cluster is.
        if (here) highlights.push({ center: here, radiusMeters: Math.max(25, Number(d.cluster_m || '5') * 4), color: MAP_RED });
      }
    }
    if (GEO_SHAPE_RULES.has(n.rule_id)) {
      const c = shpContext(n);
      const routeId = c.routes[0] || (c.shape ? ni.shape_routes?.[c.shape]?.[0]?.[0] : undefined);
      const color = routeColor(routeId);
      // (GEO_017: the line as it should run, without its 0,0 points — as on every card.)
      addShape(c.shape, color);
      const trip = dqIndexKey(ni.trip_stops, c.at('example_trip_id'));
      if (trip) {
        const sequences = ni.trip_stop_sequences?.[trip] ?? [];
        for (const [index, stopId] of (ni.trip_stops?.[trip] ?? []).entries()) addStop(stopId, false, sequences[index]);
      }
      const [p, q] = n.rule_id === 'GEO_017'
        ? [num(d.prev_lat, d.prev_lon), num(d.next_lat, d.next_lon)]
        : [num(d.from_lat, d.from_lon), num(d.to_lat, d.to_lon)];
      if (p && q) {
        segments.push({ coords: [p, q], color: MAP_RED, weight: 7, zoomTo: true });
        highlights.push({ center: [(p[0] + q[0]) / 2, (p[1] + q[1]) / 2], radiusMeters: Math.max(80, metersBetween(p, q) / 2 + 60), color: MAP_RED });
        focus = undefined;
      }
    }
    if (n.rule_id === 'GEO_020') {
      // All points in one place: that place red (the trip's stops stay grey around it).
      mark(num(d.lat, d.lon), t('producer.map.geo_shape_point'));
    }
    if (n.rule_id === 'GEO_018') {
      for (let i = 0; d[`example_${i}_stop_id`]; i++) addStop(d[`example_${i}_stop_id`], true);
      const mid = num(d.mid_lat, d.mid_lon);
      if (mid) highlights.push({ center: mid, radiusMeters: Math.max(30, Number(d.span_m || '0') / 2 + 20), color: MAP_RED });
      focus = undefined;
    }
    if (n.rule_id === 'GEO_021') {
      for (let i = 0; d[`group_${i}_count`]; i++) {
        const names = listOf(d[`group_${i}_stops`]).map(id => ni.stops?.[id] || id).join(', ');
        mark(num(d[`group_${i}_lat`], d[`group_${i}_lon`]), t('producer.map.geo_group', { n: localizedNumber(d[`group_${i}_count`]!), stops: names }));
      }
      focus = undefined;
    }
    if (n.rule_id === 'GEO_014') {
      for (const routeId of Object.keys(ni.route_shapes ?? {}).slice(0, 40)) addShape(ni.route_shapes[routeId][0], routeColor(routeId), 3);
      for (const id of listOf(d.example_stop_ids)) addStop(id, true);
      const middle = num(d.med_lat, d.med_lon);
      if (middle) pins.push({ lat: middle[0], lon: middle[1], label: t('producer.map.geo_middle'), primary: false, color: MAP_GREY, stroke: '#ffffff' });
      focus = undefined;
    }
  }
  // VAT: the routes in question in their own colours, the stops the finding is about in red.
  if (n.rule_id.startsWith('VAT_') && n.details) {
    const d = n.details;
    const drawTrip = (tripId: string | undefined, color: string, weight: number, mark: boolean | null) => {
      const tripStops = tripId ? ni.trip_stops?.[tripId] ?? [] : [];
      addTripLine(tripId, color, weight);
      if (mark !== null) for (const stopId of tripStops) addStop(stopId, mark);
    };
    // A route by its drawn lines (once each), else its example trip's stops; its stops grey.
    const drawRoute = (routeId: string | undefined, withStops: boolean) => {
      if (!routeId) return;
      const color = routeColor(routeId);
      let drawn = false;
      for (const shapeId of (ni.route_shapes?.[routeId] ?? []).slice(0, 2)) {
        const coords = drawableLine(ni.shape_coords?.[shapeId]);
        drawn = lines.some(l => l.coords === coords) || addShape(shapeId, color) || drawn;
      }
      const tripId = routeExampleTrip(ni, routeId);
      if (!drawn) drawTrip(tripId, color, 4, withStops ? false : null);
      else if (withStops) for (const stopId of (tripId ? ni.trip_stops?.[tripId] ?? [] : [])) addStop(stopId, false);
    };
    const link = (from?: string | null, to?: string | null) => {
      const a = coord(from);
      const b = coord(to);
      if (a && b) segments.push({ coords: [a, b], color: MAP_RED, weight: 3 });
    };
    if (n.rule_id === 'VAT_001') {
      for (const id of [...listOf(d.only_a), ...listOf(d.only_b)]) addStop(id, true);
      for (const routeId of listOf(d.routes)) drawRoute(routeId, true);
      focus = undefined;
    } else if (n.rule_id === 'VAT_002' || n.rule_id === 'VAT_007') {
      addStop(d.nearby_stop_id, true);
      link(n.entity_id, d.nearby_stop_id);
      for (const routeId of listOf(d.routes).slice(0, 4)) drawRoute(routeId, false);
      for (const routeId of listOf(d.nearby_routes).slice(0, 3)) drawRoute(routeId, false);
    } else if (n.rule_id === 'VAT_005') {
      for (const id of listOf(d.isolated_stops)) addStop(id, true);
      for (let i = 0; d[`cluster_${i}_stop_id`]; i++) {
        const tripId = d[`cluster_${i}_trip_id`];
        drawTrip(tripId, routeColor(tripId ? ni.trip_routes?.[tripId] : undefined), 4, true);
        addStop(d[`cluster_${i}_nearest_stop_id`], false);
        link(d[`cluster_${i}_stop_id`], d[`cluster_${i}_nearest_stop_id`]);
      }
      focus = undefined;
    } else if (n.rule_id === 'VAT_006') {
      drawRoute(routeIdOfNotice(n) ?? undefined, true);
      for (let i = 0; i < 3 && d[`other_${i}_route_id`]; i++) drawRoute(d[`other_${i}_route_id`], false);
    } else if (n.rule_id === 'VAT_008') {
      // Each route's own line, so a route that leaves the shared line shows.
      for (const routeId of listOf(d.routes).slice(0, 6)) drawTrip(routeExampleTrip(ni, routeId), routeColor(routeId), 3, false);
    }
  }
  // OPR_014: the stops of the longest transfers.
  if (n.rule_id === 'OPR_014' && n.details) {
    for (let i = 0; n.details[`example_${i}_seconds`]; i++) {
      addStop(n.details[`example_${i}_from_stop_id`], true);
      addStop(n.details[`example_${i}_to_stop_id`], true);
    }
  }
  // DQ_016 on the stop list: the stops whose values carry the spaces.
  if (n.rule_id === 'DQ_016' && n.details?.record_field === 'stop_id') {
    for (const sample of dq016Samples(n.details)) addStop(dqIndexKey(ni.stops, sample.record), true);
  }
  if (n.rule_id === 'DQ_021' && n.file === 'stops.txt') {
    for (let i = 0; n.details?.[`example_${i}_key`]; i++) addStop(n.details[`example_${i}_key`], true);
  }

  if (lines.length || segments.length || pins.length) {
    return {
      opts: {
        pins,
        extraPolylines: [...lines, ...segments],
        ...(highlights.length ? { highlights } : {}),
        focus,
        focusZoom: n.rule_id === 'STM_015' || n.rule_id === 'STM_016' ? 14 : n.rule_id === 'STM_021' || n.rule_id === 'GEO_012' ? 17 : n.rule_id === 'GEO_009' ? 16 : undefined,
      },
      caption: focusCaption(),
    };
  }

  if (n.rule_id === 'STM_006') {
    return { opts: { pins: [] }, caption: t('producer.map.no_evidence') };
  }

  // Location-independent record: the whole network.
  for (const routeId of Object.keys(ni.route_shapes ?? {}).slice(0, 40)) {
    addShape(ni.route_shapes[routeId][0], routeColor(routeId));
  }
  if (!lines.length) {
    for (const stopId of Object.keys(ni.stop_coords ?? {}).slice(0, 3000)) addStop(stopId, false);
  }
  if (!lines.length && !pins.length) return { opts: { pins: [] }, caption: t('producer.map.no_evidence') };
  return { opts: { pins, extraPolylines: lines }, caption: t('producer.map.caption_network') };
}

/** Görülen/beklenen değer: motorun çevrilmemiş açıklamaları ("monotonic trip chronology", "7 rows")
 *  gösterilmez; biçim kalıpları (HH:MM:SS, YYYYMMDD) dilden bağımsız sayılır. Sefer düzeyine toplanmış
 *  bildirimde (STM_008) görülen değer bir sayaçtır ve kendi adıyla yazılır. */
function technicalValues(n: Notice): Array<[string, string | null | undefined]> {
  const formatMask = (v: string) => /^[A-Z]+(?:[:/\- ][A-Z]+)*$/.test(v);
  const expected = n.expected_value?.trim();
  const shownExpected = expected && (isNeutralValue(expected) || formatMask(expected)) ? expected : null;
  if (n.details?.affected_segments && n.details.affected_segments === n.observed_value) {
    return [[t('producer.card.affected_segments'), n.observed_value]];
  }
  // STM_021's observed value is the stop-code pair, the expected one a distance; show the
  // measured distance so both sides compare the same thing (the codes are listed above).
  if (n.rule_id === 'STM_021') return [[t('producer.card.observed'), '0 m'], [t('producer.card.expected'), shownExpected]];
  return [
    [t('producer.card.observed'), plainValue(n.observed_value, n.field)],
    [t('producer.card.expected'), shownExpected],
  ];
}

/** Bilgi işlem ekibi için seçilen kaydın künyesi: kural, dosyadaki yeri, kayıt kodları ve ham değerler.
 *  Motorun iç anahtarları (prev_arrival …) planlamacı görünümünde zaten tabloda; burada tekrar edilmez. */
function technicalDetails(n: Notice, result: ValidationResult): string {
  const codeLabel: Record<string, string> = {
    Trip: 'producer.col.trip_code', Stop: 'producer.col.stop_code', Route: 'producer.col.route_code',
    Service: 'producer.col.service_code', Shape: 'producer.col.shape_code',
  };
  // "Hat kodu" is the route's number riders know (route_short_name, as the badges), not route_id.
  const routeId = n.entity_type === 'Route' ? routeIdOfNotice(n) : null;
  const entityId = n.entity_type !== 'Route' ? n.entity_id : routeId ? routeBadge(routeId, result.name_index).label : routeId;
  const stopId = n.entity_type === 'Stop' ? null : n.details?.stop_id;
  const location = n.rule_id === 'STM_056'
    ? [n.file, n.field].filter(Boolean).join(' · ')
    : where(n);
  const fields = [
    [t('producer.card.rule'), n.rule_id],
    [t('producer.card.location'), location],
    [t(codeLabel[n.entity_type] ?? 'producer.col.record'), entityId],
    [t('producer.col.stop_code'), stopId],
    ...technicalValues(n),
  ] as Array<[string, string | null | undefined]>;
  return fields
    .filter(([, value]) => value != null && value !== '')
    .map(([label, value]) => `<span>${escHtml(label)}</span><code>${escHtml(String(value))}</code>`)
    .join('');
}

/** Etkilenenler tablosu planlamacının tanıdığı sütunlarla: sefer → hat/yön/kalkış/günler,
 *  durak → ad/kod, takvim → günler. */
type Cell = string | { html: string };
function affectedColumns(g: RuleGroup, result: ValidationResult): { head: string[]; row: (n: Notice) => Cell[] } {
  const entity = entityColumns(g, result);
  // CAL cards: each row also names the routes running on its calendar(s) by the number riders
  // know (route_short_name, as the route badges; the code only when there is none), comma-separated
  // (the engine lists up to ten, most trips first; "+N" for the rest).
  const base = CAL_USAGE_RULES.has(g.ruleId)
    ? {
      head: [...entity.head, t('producer.col.route_code')],
      row: (n: Notice): Cell[] => {
        const d = n.details ?? {};
        const routes: string[] = [];
        for (let i = 0; d[`route_${i}_route_id`]; i++) routes.push(routeBadge(d[`route_${i}_route_id`]!, result.name_index).label);
        const more = Number(d.route_count) - routes.length;
        return [...entity.row(n), routes.length ? routes.join(', ') + (more > 0 ? ` +${localizedNumber(String(more))}` : '') : '—'];
      },
    }
    : entity;
  // For rules that do not provide their own evidence, each row carries its own observed value.
  // DQ_018/019 describe the selected name only; the table keeps each row's name.
  const copy = plannerCardCopy(g, result);
  const ownEvidence = copy && !copy.generic && g.ruleId !== 'DQ_018' && g.ruleId !== 'DQ_019';
  if (ownEvidence || !g.notices.some(n => observedValue(n))) return base;
  // DQ_018/019 mix stop names, route names and headsigns in one list: each row says which it is.
  const nameKind = g.ruleId === 'DQ_018' || g.ruleId === 'DQ_019';
  return {
    head: [...base.head, ...(nameKind ? [t('producer.times.name_kind')] : []), t('producer.card.observed')],
    row: n => {
      const o = observedValue(n);
      const cell: Cell = o
        ? { html: `${escHtml(o.value)}${o.expected ? ` <span class="planner-expected">${escHtml(t('producer.card.expected_inline', { value: o.expected }))}</span>` : ''}` }
        : '—';
      const kindKey = `producer.times.name_kind_${n.field ?? ''}`;
      const kind = t(kindKey);
      return [...base.row(n), ...(nameKind ? [kind === kindKey ? n.field || '—' : kind] : []), cell];
    },
  };
}

function entityColumns(g: RuleGroup, result: ValidationResult): { head: string[]; row: (n: Notice) => Cell[] } {
  const ni = result.name_index;
  const dash = (v?: string | null) => v || '—';
  const first = g.notices[0];
  // Saat sırası kartlarında takvim bağlamı özellikle görünür olmalı; veri yoksa
  // bile sütun korunur ve "—" gösterilir. Diğer sefer kartlarının mevcut düzeni
  // değişmez.
  const withDays = g.notices.some(n => n.service_id)
    || (first.entity_type === 'Trip' && ['STM_003', 'STM_048', 'STM_056'].includes(g.ruleId));
  const days = (cols: string[], row: (n: Notice) => Cell[]) => withDays
    ? {
      head: [...cols, t('producer.col.service')],
      // A trip row without a code (TRP_001/031/035) still names the calendar written on it.
      row: (n: Notice) => [...row(n), serviceText(n.service_id || (['TRP_001', 'TRP_031', 'TRP_035'].includes(n.rule_id) ? n.details?.service_id : undefined), result)],
    }
    : { head: cols, row };
  const badge = (routeId?: string | null, fallback?: string | null): Cell => routeId ? { html: routeBadgeHtml(routeId, result) } : dash(fallback);
  const stm006Columns = () => ({
    head: [t('producer.col.route'), t('producer.col.direction'), t('producer.col.departure'), t('producer.col.spot'), t('producer.col.service'), t('producer.col.stop_arrival'), t('producer.col.stop_departure')],
    row: (n: Notice): Cell[] => {
      const tripId = n.entity_id;
      const routeId = tripId ? ni.trip_routes?.[tripId] : undefined;
      return [
        routeId ? { html: routeBadgeHtml(routeId, result) } : '—',
        dash(tripId ? ni.trips?.[tripId] : null),
        dash(tripId ? ni.trip_first_dep?.[tripId] : null),
        dash(timeSpot(n, result)),
        plannerServiceText(n.service_id, result),
        dash(plannerTime(n.details?.arrival)),
        dash(plannerTime(n.details?.departure)),
      ];
    },
  });
  const continuousServiceColumns = () => ({
    head: [t('producer.col.route'), t('producer.col.direction'), t('producer.col.departure'), t('producer.col.service'), t('producer.col.spot'), t('producer.card.observed')],
    row: (n: Notice): Cell[] => {
      const tripId = n.entity_id;
      const routeId = tripId ? ni.trip_routes?.[tripId] : undefined;
      return [
        routeId ? { html: routeBadgeHtml(routeId, result) } : '—',
        dash(tripId ? ni.trips?.[tripId] : null),
        dash(tripId ? ni.trip_first_dep?.[tripId] : null),
        plannerServiceText(n.service_id, result),
        dash(timeSpot(n, result)),
        visibleValue(n.observed_value) || '—',
      ];
    },
  });
  const listLabel = (n: Notice) => dqListLabel(n) || n.file || '—';
  if (g.ruleId === 'ARC_011') {
    return { head: [t('producer.col.record'), t('producer.col.file_size')], row: n => [listLabel(n), fileSizeText(n.observed_value)] };
  }
  if (g.ruleId === 'ARC_012') {
    return {
      head: [t('producer.col.record'), t('producer.col.affected_rows'), t('producer.col.example_rows')],
      row: n => [listLabel(n), localizedNumber(n.details?.affected_rows ?? '1'), dash(n.details?.example_lines)],
    };
  }
  // Header findings: the column as written (spaces made visible); ARC_017's summary row counts the rest.
  if (g.ruleId === 'ARC_014' || g.ruleId === 'ARC_015' || g.ruleId === 'ARC_017') {
    return {
      head: [t('producer.col.record'), t('producer.col.column')],
      row: n => [
        listLabel(n),
        n.details?.message_variant === 'truncated'
          ? t('producer.card.ARC_017.more_columns', { n: localizedNumber(n.details.omitted_columns ?? '') })
          : visibleValue(n.field) || '—',
      ],
    };
  }
  if (g.ruleId === 'ARC_022') {
    return { head: [t('producer.col.record'), t('producer.col.row_count')], row: n => [listLabel(n), localizedNumber(n.observed_value ?? '')] };
  }
  // File summaries (one finding per file): how many rows, and the first value with its column.
  if (g.ruleId === 'ARC_032' || g.ruleId === 'ARC_037') {
    return {
      head: [t('producer.col.record'), t('producer.col.affected_rows'), t('producer.col.row'), t('producer.col.column'), t('producer.times.value')],
      row: n => [
        listLabel(n),
        dash(n.details?.affected_rows ? localizedNumber(n.details.affected_rows) : null),
        n.line != null ? String(n.line) : '—',
        dash(n.details?.example_field ? plannerFieldName(n.details.example_field) : null),
        visibleValue(n.details?.example_value ?? n.observed_value) || '—',
      ],
    };
  }
  if (g.ruleId === 'ARC_033') {
    return {
      head: [t('producer.col.record'), t('producer.col.affected_rows'), t('producer.col.row'), t('producer.times.value')],
      row: n => [
        listLabel(n),
        dash(n.details?.affected_rows ? localizedNumber(n.details.affected_rows) : null),
        n.line != null ? String(n.line) : '—',
        visibleValue(n.details?.example_value ?? null) || '—',
      ],
    };
  }
  if (g.ruleId === 'ARC_021' || g.ruleId === 'ARC_030') {
    return {
      head: [t('producer.col.record'), t('producer.col.row'), t('producer.col.column'), t('producer.times.value')],
      row: n => [
        listLabel(n),
        n.line != null ? String(n.line) : '—',
        dash(n.details?.field_name ? plannerFieldName(n.details.field_name) : null),
        dash(n.details?.value_excerpt ?? n.observed_value),
      ],
    };
  }
  // ARC_002/003: the first unreadable character, after the readable text just before it.
  if (g.ruleId === 'ARC_002' || g.ruleId === 'ARC_003') {
    return {
      head: [t('producer.col.record'), t('producer.col.row'), t('producer.col.broken_text')],
      row: n => {
        const d = n.details ?? {};
        return [
          listLabel(n),
          dash(d.invalid_line),
          d.invalid_bytes ? `…${d.text_before ?? ''} ⟨${d.invalid_bytes}⟩` : '—',
        ];
      },
    };
  }
  // Station pathways: station, kind, the two ends, length and time as written.
  if (g.ruleId.startsWith('PTH_') && (first.details?.from_stop_id || first.details?.to_stop_id)) {
    const name = (id?: string) => id ? ni.stops?.[id] || id : '—';
    return {
      head: [t('producer.col.station'), t('producer.col.pathway_mode'), t('producer.col.pathway_from'), t('producer.col.pathway_to'), t('producer.col.distance'), t('producer.col.pathway_time')],
      row: n => {
        const d = n.details ?? {};
        return [
          name(d.station_id), pthMode(d.pathway_mode), name(d.from_stop_id), name(d.to_stop_id),
          d.length && Number.isFinite(Number(d.length)) ? metersText(d.length) : dash(d.length),
          d.traversal_time && Number(d.traversal_time) > 0 ? durationText(Number(d.traversal_time)) : dash(d.traversal_time),
        ];
      },
    };
  }
  // Frequencies: the trip, the period and the headway as written, and the row.
  if (g.ruleId.startsWith('FRQ_') && first.entity_type === 'Trip' && first.details?.start_time !== undefined) {
    return {
      head: [t('producer.col.route'), t('producer.col.direction'), t('producer.col.frq_period'), t('producer.col.frq_headway'), t('producer.col.row')],
      row: n => {
        const d = n.details ?? {};
        const known = n.rule_id !== 'FRQ_001' && n.entity_id;
        return [
          known ? badge(ni.trip_routes?.[n.entity_id!]) : '—',
          known ? dash(ni.trips?.[n.entity_id!]) : dash(n.entity_id),
          d.start_time || d.end_time ? `${plannerTime(d.start_time) ?? '—'} – ${plannerTime(d.end_time) ?? '—'}` : '—',
          d.headway_secs && Number.isFinite(Number(d.headway_secs)) ? durationText(Number(d.headway_secs)) : dash(d.headway_secs),
          n.line != null ? String(n.line) : '—',
        ];
      },
    };
  }
  // Transfers: the two stops, the time entered and its row.
  if (AGN_AGENCY_RULES.has(g.ruleId) || g.ruleId === 'AGN_003') {
    return {
      head: [t('producer.times.agency'), t('producer.col.column'), t('producer.times.value'), t('producer.col.row')],
      row: n => [agencyLabel(n, result), dash(n.field ? plannerFieldName(n.field) : null), visibleValue(n.observed_value ?? null) || '—', n.line != null ? String(n.line) : '—'],
    };
  }
  if (g.ruleId === 'XFL_002') {
    return {
      head: [t('producer.col.route'), t('producer.col.direction'), t('producer.col.service')],
      row: n => [badge(n.entity_id ? ni.trip_routes?.[n.entity_id] : undefined), dash(n.entity_id ? ni.trips?.[n.entity_id] : null), plannerServiceText(n.details?.service_id, result)],
    };
  }
  if ((g.ruleId.startsWith('TRF_') || g.ruleId === 'XFL_020' || g.ruleId === 'XFL_021') && (first.entity_type === 'Transfer' || first.entity_type === 'Row') && trfStops(first).some(Boolean)) {
    const name = (id?: string) => id ? ni.stops?.[id] || id : '—';
    const withDistance = g.ruleId === 'TRF_020' || g.ruleId === 'TRF_011';
    return {
      head: [t('producer.col.transfer_from'), t('producer.col.transfer_to'), t('producer.col.transfer_time'), ...(withDistance ? [t('producer.col.distance')] : []), t('producer.col.row')],
      row: n => {
        const [from, to] = trfStops(n);
        const meters = n.details?.distance_m ?? (n.rule_id === 'TRF_011' ? (n.observed_value ?? '').match(/^(\d+)/)?.[1] : undefined);
        return [
          name(from), name(to),
          n.details?.min_transfer_time ? t('producer.card.TRF.minutes', { minutes: minutesText(n.details.min_transfer_time) }) : '—',
          ...(withDistance ? [meters ? metersText(meters) : '—'] : []),
          n.line != null ? String(n.line) : '—',
        ];
      },
    };
  }
  // GTFS-JP: route kana names say which name (number or long name); translation rows say what they translate.
  if (g.ruleId === 'JPN_008') {
    return {
      head: [t('producer.col.route'), t('producer.times.name_kind'), t('producer.times.value')],
      row: n => [badge(n.entity_id), dash(n.field ? plannerFieldName(n.field) : null), dash(jpKanaName(n, result))],
    };
  }
  // JPN agency/office/pattern rows: which list, the code or value written, its row.
  if (JPN_RECORD_RULES.has(g.ruleId) && !['JPN_002', 'JPN_018', 'JPN_011'].includes(g.ruleId)) {
    return {
      head: [t('producer.col.record'), t('producer.times.value'), t('producer.col.row')],
      row: n => [
        dash(plannerListName(n.file) || n.file),
        dash(n.observed_value || n.entity_id),
        n.line != null ? String(n.line) : '—',
      ],
    };
  }
  if (g.ruleId === 'JPN_022' || g.ruleId === 'JPN_026' || g.ruleId === 'JPN_027') {
    return {
      head: [t('producer.col.record'), t('producer.times.name_kind'), t('producer.times.value'), t('producer.col.affected_records')],
      row: n => [
        dash(plannerListName(n.file) || n.file),
        dash(n.field ? plannerFieldName(n.field) : null),
        dash(n.rule_id === 'JPN_027' && n.observed_value ? routeTypeName(n.observed_value) : n.observed_value),
        dash(n.details?.affected_records ? localizedNumber(n.details.affected_records) : null),
      ],
    };
  }
  if (g.ruleId === 'JPN_024' || g.ruleId === 'JPN_025') {
    return {
      head: [t('producer.times.agency'), t('producer.times.value')],
      row: n => [dash(result.feed_summary?.agencies?.find(a => a.agency_id === n.entity_id || a.agency_name === n.entity_id)?.agency_name || n.entity_id), dash(n.observed_value)],
    };
  }
  if (g.ruleId === 'JPN_033') {
    return {
      head: [t('producer.col.record'), t('producer.times.value')],
      row: n => [dash(plannerListName(n.file) || n.file), dash(n.observed_value)],
    };
  }
  if (g.ruleId === 'JPN_002' || g.ruleId === 'JPN_018') {
    // The trip or route as usual, plus the office or pattern code it points to.
    const base = first.entity_type === 'Trip'
      ? { head: [t('producer.col.route'), t('producer.col.direction'), t('producer.col.departure')], row: (n: Notice): Cell[] => [badge(n.entity_id ? ni.trip_routes?.[n.entity_id] : undefined), dash(n.entity_id ? ni.trips?.[n.entity_id] : null), dash(n.entity_id ? ni.trip_first_dep?.[n.entity_id] : null)] }
      : { head: [t('producer.col.route'), t('producer.col.route_name')], row: (n: Notice): Cell[] => [badge(n.entity_id), dash(n.entity_id ? ni.route_long_names?.[n.entity_id] : null)] };
    return { head: [...base.head, t('producer.times.value')], row: n => [...base.row(n), dash(n.observed_value)] };
  }
  if (g.ruleId === 'JPN_019' || g.ruleId === 'JPN_021') {
    return {
      head: [t('producer.col.record'), t('producer.times.name_kind'), t('producer.times.value'), t('producer.col.reading'), t('producer.col.row')],
      row: n => {
        const d = n.details ?? {};
        return [
          dash(plannerListName(d.table_name) || d.table_name),
          dash(d.field_name ? plannerFieldName(d.field_name) : null),
          jpTranslationTarget(n, result),
          n.rule_id === 'JPN_021' ? visibleValue(n.observed_value ?? null) || '—' : '—',
          n.line != null ? String(n.line) : '—',
        ];
      },
    };
  }
  if (g.ruleId === 'JPN_028' || g.ruleId === 'JPN_029' || g.ruleId === 'JPN_030') {
    return {
      head: [t('producer.col.record'), t('producer.times.name_kind'), t('producer.times.value'), t('producer.col.affected_records')],
      row: n => {
        const d = n.details ?? {};
        return [
          dash(plannerListName(d.table_name) || d.table_name),
          dash(n.field ? plannerFieldName(n.field) : null),
          dash(d.source_value),
          dash(d.affected_records ? localizedNumber(d.affected_records) : null),
        ];
      },
    };
  }
  // An identity-less notice (feed-level summary) does not fill entity columns; it falls into the general columns.
  switch (first.entity_id ? first.entity_type : 'Feed') {
    case 'Trip': {
      if (g.ruleId === 'STM_006') return stm006Columns();
      // TRP_024's record is a vehicle block, not a trip: its code and its first trip.
      if (g.ruleId === 'TRP_024') {
        return {
          head: [t('producer.times.trp_block'), t('producer.col.trip')],
          row: n => [dash(n.details?.block_id || n.entity_id), tripShort(n.details?.first_trip, result)],
        };
      }
      if (g.ruleId === 'STM_018' || g.ruleId === 'STM_019') return continuousServiceColumns();
      const withSpot = timeSpot(first, result) !== null;
      return days(
        [t('producer.col.route'), t('producer.col.direction'), t('producer.col.departure'), ...(withSpot ? [t('producer.col.spot')] : [])],
        n => {
          const routeId = n.entity_id ? ni.trip_routes?.[n.entity_id] : undefined;
          return [
            badge(routeId),
            dash(n.entity_id ? ni.trips?.[n.entity_id] : null),
            dash(n.rule_id === 'STM_001'
              ? plannerTime(n.details?.first_departure)
              : n.entity_id ? ni.trip_first_dep?.[n.entity_id] : null),
            ...(withSpot ? [dash(timeSpot(n, result))] : []),
          ];
        },
      );
    }
    case 'Stop':
      if (g.ruleId === 'STM_002') {
        return {
          head: [t('producer.col.route'), t('producer.col.direction'), t('producer.col.departure'), t('producer.col.service')],
          row: n => {
            const details = n.details ?? {};
            const routeId = details.example_1_route_id;
            const serviceName = details.example_1_service_name;
            const time = details.example_1_departure
              || (details.example_1_window_start && details.example_1_window_end
                ? t('producer.card.pickup_window', { start: plannerTime(details.example_1_window_start) ?? '', end: plannerTime(details.example_1_window_end) ?? '' })
                : null);
            return [
              routeId ? { html: routeBadgeHtml(routeId, result) } : '—',
              dash(details.example_1_trip_id ? ni.trips?.[details.example_1_trip_id] : null),
              dash(time),
              {
                html: `${escHtml(serviceName || '')}${serviceName ? ' · ' : ''}${escHtml(serviceText(details.example_1_service_id, result))}`,
              },
            ];
          },
        };
      }
      return {
        head: [t('producer.col.stop'), t('producer.col.stop_code')],
        row: n => [dash(n.entity_id ? ni.stops?.[n.entity_id] : null), dash(n.entity_id)],
      };
    case 'Route': {
      const withSpot = timeSpot(first, result) !== null;
      if (g.ruleId === 'STM_014') {
        return {
          head: [t('producer.col.route'), t('producer.col.direction'), t('producer.col.departure'), t('producer.col.spot'), t('producer.col.service')],
          row: n => {
            const routeId = routeIdOfNotice(n);
            const d = n.details ?? {};
            const tripId = listOf(d.trips)[0];
            const direction = (tripId ? ni.trips?.[tripId] : null)
              || [d.direction_from_stop_id || d.stop_a, d.direction_to_stop_id || d.stop_b]
                .map(id => id ? ni.stops?.[id] || id : null)
                .filter((name): name is string => Boolean(name))
                .join(' → ');
            return [
              badge(routeId),
              dash(direction),
              dash(plannerTime(d.sample_departure) || (tripId ? ni.trip_first_dep?.[tripId] : null)),
              dash(timeSpot(n, result)),
              plannerServiceText(d.sample_service_id ?? n.service_id, result),
            ];
          },
        };
      }
      // OPR route findings per direction and calendar: name both on each row.
      const perService = g.ruleId.startsWith('OPR_') && g.notices.some(n => n.service_id);
      return {
        head: [t('producer.col.route'), t('producer.col.route_name'),
          ...(perService ? [t('producer.col.direction'), t('producer.col.service')] : []), ...(withSpot ? [t('producer.col.spot')] : [])],
        row: n => {
          const routeId = routeIdOfNotice(n);
          const d = n.details ?? {};
          return [
            // The badge is the route's number riders know (route_short_name); a separate code column would repeat it.
            badge(routeId), dash(routeId ? ni.route_long_names?.[routeId] : null),
            ...(perService ? [dash(oprDirectionName(d, result, d.trip_before) || null), plannerServiceText(n.service_id, result)] : []),
            ...(withSpot ? [dash(timeSpot(n, result))] : []),
          ];
        },
      };
    }
    case 'Shape':
  // Shape drawing: the route(s) using it and the shape ID.
      return {
        head: [t('producer.col.route'), t('producer.col.shape_code')],
        row: n => {
          const indexed = [...new Set((n.entity_id ? ni.shape_routes?.[n.entity_id] ?? [] : []).map(([routeId]) => routeId))];
          // Large feeds index only the shapes a finding names; SHP findings carry their routes.
          const routeIds = indexed.length ? indexed : SHP_CONTEXT_RULES.has(n.rule_id) ? shpContext(n).routes : [];
          const routes: Cell = routeIds.length
            ? { html: routeIds.slice(0, 4).map(routeId => routeBadgeHtml(routeId, result)).join(' ') + (routeIds.length > 4 ? ` +${routeIds.length - 4}` : '') }
            : '—';
          return [routes, dash(n.entity_id)];
        },
      };
    case 'Service':
      return {
        head: [t('producer.col.service_code'), t('producer.col.service')],
        row: n => [dash(n.entity_id), serviceText(n.entity_id, result)],
      };
    default: {
  // Calendar-period notices (CAL_007/012/013) carry affected calendars as a list.
      const withServices = g.notices.some(n => n.details?.services);
      const servicesText = (n: Notice) => {
        const ids = listOf(n.details?.services);
        return ids.length ? ids.slice(0, 5).join(', ') + (ids.length > 5 ? ` (+${ids.length - 5})` : '') : '—';
      };
  // Records without a row number (feed/period level) do not show an empty "Row" column.
      const withLine = g.notices.some(n => n.line != null);
      return days(
        [t('producer.col.record'), ...(withLine ? [t('producer.col.row')] : []), ...(withServices ? [t('producer.col.service_code')] : [])],
        n => [
          dqListLabel(n) || plannerEntityLabel(n, result) || n.observed_value || t('producer.map.unknown_context'),
          ...(withLine ? [n.line != null ? String(n.line) : '—'] : []),
          ...(withServices ? [servicesText(n)] : []),
        ],
      );
    }
  }
}

function affectedRows(g: RuleGroup, result: ValidationResult): string {
  const cols = affectedColumns(g, result);
  const rowCap = ROWS_CAP;
  const visibleRows = g.notices.slice(0, rowCap);
  const rows = visibleRows.map((n, index) => `<tr class="planner-affected-row${index === 0 ? ' selected' : ''}" data-row="${index}" tabindex="0"${index >= ROWS_FIRST ? ' hidden' : ''}>
    ${cols.row(n).map(v => typeof v === 'string' ? `<td>${escHtml(v)}</td>` : `<td>${v.html}</td>`).join('')}
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
function factsHtml(planner: Pick<PlannerCardCopy, 'observed' | 'usage'> | null): string {
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
      <span class="planner-card-impact"><strong>${g.count.toLocaleString(getLocale())}</strong><span>${escHtml(unitOf(g))}</span></span>
    </button>
    <div class="planner-card-collapsible" hidden>
      ${context ? `<p class="producer-card-context">${escHtml(context)}</p>` : ''}
      <div class="producer-card-main">
        <figure class="planner-map-wrap"><div class="planner-map"><span>${t('producer.map.loading')}</span></div><figcaption class="planner-map-caption"></figcaption></figure>
        <div class="planner-card-text">
          <div class="planner-seen-host"></div>
          <div class="producer-card-evidence">
            <span class="producer-card-label">${t('producer.card.what')}</span>
            <p class="producer-card-message">${escHtml(what)}</p>
            <div class="planner-times-host">${timeStrip(n, result)}</div>
            <div class="producer-card-facts"${factsHtml(evidenceFacts(planner, n)) ? '' : ' hidden'}>${factsHtml(evidenceFacts(planner, n))}</div>
          </div>
          <div class="producer-card-action">
            <span class="producer-card-label">${t('producer.card.action')}</span>
            <div class="planner-steps-host">${stepsHtml(action)}</div>
          </div>
        </div>
      </div>
      ${affectedRows(g, result)}
      <details class="producer-card-details"><summary>${t('producer.card.technical')}</summary><dl>${technicalDetails(n, result)}</dl></details>
    </div>
  </article>`;
}

function coverageNotice(complete: boolean): string {
  if (complete) return '';
  return `<div class="producer-coverage">${t('producer.coverage_incomplete')}</div>`;
}

function feedTitle(result: ValidationResult, agencyIds: readonly string[]): string {
  const info = result.feed_summary?.feed_info;
  // When an agency is selected, the heading is its name (or their names).
  const picked = (result.agency_breakdown?.agencies ?? [])
    .filter(a => agencyIds.includes(a.agency_id))
    .map(a => a.agency_name || a.agency_id);
  const name = picked.length
    ? picked.join(', ')
    : info?.feed_publisher_name || result.feed_summary?.agencies?.[0]?.agency_name || t('producer.feed_fallback');
  const version = info?.feed_version ? `<span>${escHtml(t('producer.feed_version', { v: info.feed_version }))}</span>` : '';
  return `<header class="planner-top"><strong>${escHtml(name)}</strong>${version}<span class="planner-beta" title="${escHtml(t('producer.beta_hint'))}">BETA</span><span class="planner-beta-hint">${escHtml(t('producer.beta_hint'))}</span></header>`;
}

type EntityDim = 'route' | 'stop' | 'shape' | 'service';
const ENTITY_DIMS: EntityDim[] = ['route', 'stop', 'shape', 'service'];
type NoticeKeys = Record<EntityDim, string[]>;

/** One card as the planner filters see it. */
export interface FacetCard { area: string; sev: string; cls: string; rule: string; file: string; count: number; keys: NoticeKeys[] }
/** The planner page's filter state. Entity picks: OR within a dimension, AND across dimensions. */
export interface FacetFilter {
  area: string; sev: Set<string>; cls: Set<string>; rule: string; file: string;
  entity: Map<EntityDim, Set<string>>;
}
export interface Facets {
  /** Per card: shown or not, and the matching rows (null = every row, no entity pick). */
  visible: boolean[];
  matches: Array<Set<number> | null>;
  /** Options per filter, each counted under every OTHER active filter (faceted search): a rule
   *  is offered with the records it would show given the area, severity, class, file and entity
   *  picks, and so on. Counts are affected records (rows when entities are picked). */
  rules: Map<string, number>;
  files: Map<string, number>;
  entity: Record<EntityDim, Map<string, number>>;
}

/** The planner filters, computed once for visibility and for every option list so they agree. */
export function plannerFacets(cards: FacetCard[], f: FacetFilter): Facets {
  const cardPasses = (c: FacetCard, skip?: 'rule' | 'file') =>
    (f.area === 'all' || c.area === f.area)
    && (f.sev.size === 0 || f.sev.has(c.sev))
    && (f.cls.size === 0 || f.cls.has(c.cls))
    && (skip === 'rule' || !f.rule || c.rule === f.rule)
    && (skip === 'file' || !f.file || c.file === f.file);
  const picks = [...f.entity].filter(([, ids]) => ids.size > 0);
  const rowsOf = (c: FacetCard, skip?: EntityDim): Set<number> | null => {
    const active = picks.filter(([dim]) => dim !== skip);
    if (!active.length) return null;
    const rows = new Set<number>();
    c.keys.forEach((k, row) => {
      if (active.every(([dim, ids]) => k[dim].some(id => ids.has(id)))) rows.add(row);
    });
    return rows;
  };
  const countOf = (c: FacetCard, rows: Set<number> | null) => (rows ? rows.size : c.count);
  const add = (map: Map<string, number>, key: string, n: number) => map.set(key, (map.get(key) ?? 0) + n);
  const out: Facets = {
    visible: [], matches: [], rules: new Map(), files: new Map(),
    entity: Object.fromEntries(ENTITY_DIMS.map(dim => [dim, new Map<string, number>()])) as Record<EntityDim, Map<string, number>>,
  };
  for (const c of cards) {
    const rows = rowsOf(c);
    out.matches.push(rows);
    out.visible.push(cardPasses(c) && (!rows || rows.size > 0));
    const n = countOf(c, rows);
    if (n > 0 && cardPasses(c, 'rule')) add(out.rules, c.rule, n);
    if (n > 0 && c.file && cardPasses(c, 'file')) add(out.files, c.file, n);
    if (!cardPasses(c)) continue;
    for (const dim of ENTITY_DIMS) {
      const dimRows = rowsOf(c, dim);
      c.keys.forEach((k, row) => {
        if (dimRows && !dimRows.has(row)) return;
        for (const id of new Set(k[dim])) add(out.entity[dim], id, 1);
      });
    }
  }
  return out;
}

/** Bir kaydın dokunduğu hat, durak, güzergâh ve takvim kimlikleri (arama süzgeci için). */
function noticeKeys(n: Notice, result: ValidationResult): NoticeKeys {
  const ni = result.name_index;
  const d = n.details ?? {};
  const trip = n.entity_type === 'Trip' ? n.entity_id : null;
  const route = n.entity_type === 'Route' ? routeIdOfNotice(n) : trip ? ni.trip_routes?.[trip] : d.route_id;
  const shape = n.entity_type === 'Shape' ? n.entity_id : trip ? ni.trip_shapes?.[trip] : d.shape_id;
  const service = n.entity_type === 'Service' ? n.entity_id : n.service_id;
  const services = listOf(d.services);
  const stops = [n.entity_type === 'Stop' ? n.entity_id : null, d.stop_id, d.stop_a, d.stop_b, d.prev_stop_id, d.next_stop_id];
  const ids = (values: Array<string | null | undefined>) => [...new Set(values.filter((v): v is string => Boolean(v)))];
  return { route: ids([route]), stop: ids(stops), shape: ids([shape]), service: ids([service, ...services]) };
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
  // If GTFS has no shape name, the route and direction of the trip using it stand in for the name.
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

  // Feed-level summary notices carry the sample records supplied by the engine; the planner should show
  // these records (stop, trip, calendar, shape) row by row instead of one "Entire feed" row.
const EXAMPLE_LISTS: Array<[string, Notice['entity_type']]> = [
  ['example_stops', 'Stop'], ['isolated_stops', 'Stop'], ['example_platforms', 'Stop'],
  ['example_trips', 'Trip'], ['example_services', 'Service'], ['example_shapes', 'Shape'],
  ['example_pathways', 'Pathway'],
];

function listOf(value: string | undefined): string[] {
  return value ? value.split(/\s*,\s*/).filter(Boolean) : [];
}

/** Özet bildirimi örnek kayıtlarına açar; özetin sayısı (görülen değer) tek kayda ait olmadığı için taşınmaz. */
function exampleNotices(n: Notice): Notice[] | null {
  if (n.entity_id || !n.details) return null;
  const rows = exampleRows(n);
  if (rows) return rows;
  // STP_004/005: one row per example stop (they carry no position, so they are listed by name).
  if ((n.rule_id === 'STP_004' || n.rule_id === 'STP_005' || n.rule_id === 'STP_042') && n.details.example_stop_ids) {
    return listOf(n.details.example_stop_ids).map(entity_id => ({ ...n, entity_type: 'Stop' as const, entity_id, observed_value: null, expected_value: null }));
  }
  for (const [key, entity_type] of EXAMPLE_LISTS) {
    const ids = listOf(n.details[key]);
    if (ids.length) return ids.map(entity_id => ({ ...n, entity_type, entity_id, observed_value: null, expected_value: null }));
  }
  return null;
}

/** Özet bildirimindeki `example_<i>_*` anahtarlarını satır düzeyindeki bildirime çevirir. */
function exampleRows(n: Notice): Notice[] | null {
  const d = n.details ?? {};
  if (!d.example_0_trip) return null;
  const summary = Object.fromEntries(Object.entries(d).filter(([key]) => !key.startsWith('example_')));
  const rows: Notice[] = [];
  for (let i = 0; d[`example_${i}_trip`]; i++) {
    const at = (key: string) => d[`example_${i}_${key}`];
    const details: Record<string, string> = { ...summary };
    if (n.rule_id === 'STM_056' || n.rule_id === 'STM_024' || n.rule_id === 'STM_036') {
      // STM_056 / STM_024 / STM_036 examples carry the pinpoint row's details verbatim (prev2…next2 stops, times,
      // distances); copy them as-is so the card, time table and map read the same keys.
      const prefix = `example_${i}_`;
      for (const [key, value] of Object.entries(d)) {
        if (!key.startsWith(prefix)) continue;
        const name = key.slice(prefix.length);
        if (!['trip', 'line', 'observed', 'expected'].includes(name)) details[name] = value;
      }
    }
    const pairs: Array<[string, string]> = [
      ['stop_id', 'stop'], ['arr', 'arrival'], ['dep', 'departure'],
      ['prev2_stop_id', 'prev2_stop'], ['prev2_stop_sequence', 'prev2_stop_sequence'],
      ['prev2_shape_dist_traveled', 'prev2_shape_dist_traveled'],
      ['prev_stop_id', 'prev_stop'], ['prev_arrival', 'prev_arrival'], ['prev_departure', 'prev_departure'],
      ['next_stop_id', 'next_stop'], ['next_arrival', 'next_arrival'], ['next_departure', 'next_departure'],
      ['next2_stop_id', 'next2_stop'], ['next2_stop_sequence', 'next2_stop_sequence'],
      ['next2_shape_dist_traveled', 'next2_shape_dist_traveled'],
      ['prev_stop_sequence', 'prev_stop_sequence'], ['stop_sequence', 'stop_sequence'],
      ['next_stop_sequence', 'next_stop_sequence'],
      ['prev_shape_dist_traveled', 'prev_shape_dist_traveled'],
      ['shape_dist_traveled', 'shape_dist_traveled'],
      ['next_shape_dist_traveled', 'next_shape_dist_traveled'],
    ];
    for (const [to, from] of pairs) {
      const value = at(from) ?? at(to);
      if (value) details[to] = value;
    }
    for (let j = 0; at(`missing_${j}_stop_id`); j++) {
      for (const key of ['stop_id', 'stop_sequence', 'arrival', 'departure']) {
        const value = at(`missing_${j}_${key}`);
        if (value) details[`missing_${j}_${key}`] = value;
      }
    }
    for (const key of ['stop_count', 'affected_stop_count']) {
      const value = at(key);
      if (value) details[key] = value;
    }
    const line = Number(at('line'));
    rows.push({
      ...n, entity_type: 'Trip', entity_id: at('trip'), line: Number.isFinite(line) && at('line') ? line : null,
      service_id: at('service') ?? at('service_id') ?? n.service_id,
      observed_value: at('observed') ?? at('shape_dist_traveled') ?? null,
      expected_value: at('expected') ?? (at('prev_shape_dist_traveled') ? `> ${at('prev_shape_dist_traveled')}` : null),
      details,
    });
  }
  return rows;
}

/** Özetin bildirdiği toplam kayıt sayısı (affected_*); yoksa örnek sayısı. */
function exampleTotal(n: Notice, examples: number): number {
  const preferred = n.rule_id === 'STM_017' ? n.details?.affected_trip_count : undefined;
  const preferredValue = Number(preferred);
  if (Number.isFinite(preferredValue) && preferredValue > examples) return preferredValue;
  const total = Object.entries(n.details ?? {}).find(([key]) => key.startsWith('affected_'))?.[1];
  const value = Number(total);
  return Number.isFinite(value) && value > examples ? value : examples;
}

function expandExamples(g: RuleGroup): void {
  let total = 0;
  let expanded = false;
  g.notices = g.notices.flatMap(n => {
    const rows = exampleNotices(n);
    if (!rows) { total += 1; return [n]; }
    expanded = true;
    total += exampleTotal(n, rows.length);
    return rows;
  });
  if (expanded) g.count = Math.max(g.count, total);
}

/** Aynı satırda varış da kalkış da bozuksa planlamacı tek konu görsün: STM_003 kartının saat
 *  tablosu iki saati birlikte gösterir; STM_004'te yalnız varışı sağlam satırlar kalır. */
function foldDepartureIntoArrival(m: ReturnType<typeof buildProducerModel>): void {
  const arrival = [...m.blockers, ...m.improvements, ...m.infos].find(g => g.ruleId === 'STM_003');
  if (!arrival) return;
  const rows = new Set(arrival.notices.map(n => `${n.entity_id}\u0000${n.line}`));
  for (const list of [m.blockers, m.improvements, m.infos]) {
    const i = list.findIndex(g => g.ruleId === 'STM_004');
    if (i < 0) continue;
    const g = list[i];
    const rest = g.notices.filter(n => !rows.has(`${n.entity_id}\u0000${n.line}`));
    if (rest.length === g.notices.length) continue;
    if (rest.length === 0) list.splice(i, 1);
    else { g.notices = rest; g.count = rest.length; }
  }
}

export function renderProducer(root: HTMLElement, fullResult: ValidationResult): void {
  // The agency filter is shared with "Details and Fix": records narrow to selected agencies;
  // publishability is feed-wide (the engine does not provide it per agency).
  const agencyFilter = getState()?.agencyFilter ?? [];
  const filtered = filterNoticesByAgency(fullResult, agencyFilter);
  const result: ValidationResult = filtered ? { ...fullResult, notices: filtered } : fullResult;
  const m = buildProducerModel(result);
  if (filtered) {
    for (const g of [...m.blockers, ...m.improvements, ...m.infos]) g.count = g.notices.length;
  }
  foldDepartureIntoArrival(m);
  const ordered = [...m.blockers, ...m.improvements, ...m.infos];
  for (const g of ordered) expandExamples(g);
  const blockerRules = new Set(m.blockers.map(g => g.ruleId));
  // Topics with planner text and publication blockers come first; the rest use the engine's technical
  // text in a separate section so two languages are not mixed in one list.
  const primary = ordered.filter(g => blockerRules.has(g.ruleId) || plannerCardCopy(g, result) !== null);
  const technical = ordered.filter(g => !primary.includes(g));
  const groups = [...primary, ...technical];

  // Every row, not only the first ROWS_CAP shown: a filter must find a record past the shown rows.
  const keys = groups.map(g => g.notices.map(n => noticeKeys(n, result)));
  const labels = Object.fromEntries(ENTITY_DIMS.map(dim => [dim, new Map<string, string>()])) as Record<EntityDim, Map<string, string>>;
  // Options are only records defined in the dataset; details for some rules, such as route_id,
  // carry a pattern identity rather than a route ("00020:0:11223→11224").
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
  // Record count: the same unit as "records" in cards.
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
  let mapRequestId = 0;

  const showMap = async (card: HTMLElement, index: number, row: number) => {
    const requestId = ++mapRequestId;
    const mapHost = card.querySelector<HTMLElement>('.planner-map');
    const caption = card.querySelector<HTMLElement>('.planner-map-caption');
    const g = groups[index];
    const notice = g.notices[row] ?? g.notices[0];
    if (!mapHost) return;
    unmount(index);
    card.querySelectorAll('.planner-affected-row.selected').forEach(el => el.classList.remove('selected'));
    card.querySelector(`.planner-affected-row[data-row="${row}"]`)?.classList.add('selected');
  // The card's context line (route · direction · departure) follows the selected record so it does not contradict the map.
    const rowCopy = plannerCardCopy({ ...g, notices: [notice] }, result);
    const contextEl = card.querySelector<HTMLElement>('.producer-card-context');
    if (contextEl && rowCopy?.context) contextEl.textContent = rowCopy.context;
  // "What happened / What to do" uses the selected record's own text; if planner text is absent, use the engine text.
    const messageEl = card.querySelector<HTMLElement>('.producer-card-message');
    if (messageEl) messageEl.textContent = rowCopy?.what || tMsg(notice);
    const actionEl = card.querySelector<HTMLElement>('.planner-steps-host');
    if (actionEl) actionEl.innerHTML = stepsHtml(rowCopy?.action || tRemediation(notice));
    const seenEl = card.querySelector<HTMLElement>('.planner-seen-host');
    if (seenEl) {
      const serviceId = notice.entity_type === 'Service' ? notice.entity_id : notice.service_id;
      seenEl.innerHTML = searchView(notice, result) + serviceBlock(serviceId, result, notice.rule_id !== 'STM_006');
    }
    const factsEl = card.querySelector<HTMLElement>('.producer-card-facts');
    if (factsEl) { const facts = factsHtml(evidenceFacts(rowCopy, notice)); factsEl.innerHTML = facts; factsEl.hidden = !facts; }
    const technicalEl = card.querySelector<HTMLElement>('.producer-card-details dl');
    if (technicalEl) technicalEl.innerHTML = technicalDetails(notice, result);
    const strip = card.querySelector<HTMLElement>('.planner-times-host');
    if (strip) strip.innerHTML = timeStrip(notice, result);
    let map = plannerMap(notice, result);
    // Large-feed results deliberately omit shape geometry. Fetch the selected
    // trip's existing shape instead of drawing a synthetic stop-to-stop route.
    if (result.name_index.map_data_deferred) {
      // Every drawing the map uses (the trip, the route's other lines, the routes around it); a
      // fetched line can make the map read another, so ask again until nothing new is missing.
      const tried = new Set<string>();
      for (let pass = 0; pass < 3; pass++) {
        const wanted = missingMapShapes(notice, result).filter(id => !tried.has(id)).slice(0, 40);
        if (!wanted.length) break;
        const { requestShapeCoords } = await import('../validator-client');
        await Promise.all(wanted.map(async shapeId => {
          tried.add(shapeId);
          const coords = await requestShapeCoords(shapeId);
          if (coords.length > 1) result.name_index.shape_coords[shapeId] = coords;
        }));
      }
      map = plannerMap(notice, result);
    }
    if (requestId !== mapRequestId || openIndex !== index || !mapHost.isConnected) return;
    if (caption) caption.textContent = map.caption;
    if (!map.opts.extraPolylines?.length && map.opts.pins.length === 0) {
      mapHost.innerHTML = `<span>${t('producer.map.no_evidence')}</span>`;
      return;
    }
    void import('../map-modal').then(({ mountInlineMap }) => {
      if (requestId === mapRequestId && openIndex === index && mapHost.isConnected && !cleanups.has(index)) {
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

  // Filters: domain + severity + class + rule + file apply together.
  let area: AreaFilter = 'all';
  const sevActive = new Set<string>();
  const clsActive = new Set<string>();
  const ruleSelect = root.querySelector<HTMLSelectElement>('#planner-rule-filter');
  const fileSelect = root.querySelector<HTMLSelectElement>('#planner-file-filter');
  const limits = new Map<number, number>();
  const matchesByCard = new Map<number, Set<number> | null>();
  const facetCards: FacetCard[] = groups.map((g, index) => ({
    area: groupArea(g), sev: g.severity, cls: g.notices[0].rule_class, rule: g.ruleId,
    file: g.notices[0].file || '', count: g.count, keys: keys[index],
  }));
  // Option labels as first rendered; counts are appended on every filter change.
  const optionLabels = (select: HTMLSelectElement | null) => new Map(
    Array.from(select?.options ?? []).filter(o => o.value).map(o => [o.value, o.textContent ?? o.value]));
  const ruleLabels = optionLabels(ruleSelect);
  const fileLabels = optionLabels(fileSelect);
  let facets: Facets | null = null;
  const fillSelect = (select: HTMLSelectElement | null, labelsOf: Map<string, string>, counts: Map<string, number>) => {
    if (!select) return;
    const current = select.value;
    // An option with no record left disappears, except the one already chosen.
    const options = [...labelsOf].filter(([value]) => (counts.get(value) ?? 0) > 0 || value === current)
      .map(([value, label]) => `<option value="${escHtml(value)}">${escHtml(label)} (${localizedNumber(String(counts.get(value) ?? 0))})</option>`);
    select.innerHTML = `<option value="">${t('fix.filter.all')}</option>${options.join('')}`;
    select.value = current;
  };
  // Affected-record table: all matching rows when filtered, paginated otherwise (10, +50).
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
  // Multi-select lists: selected IDs per dimension; OR within a dimension, AND across dimensions.
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
    const counts = facets?.entity[dim];
    // Only records still reachable under the other filters; a chosen one always stays.
    const options = sortedOptions.get(dim)!.filter(([id]) => ids.has(id) || !counts || (counts.get(id) ?? 0) > 0);
    const hits = options.filter(([id, label]) => ids.has(id) || !search || label.toLocaleLowerCase(getLocale()).includes(search));
    hits.sort((a, b) => Number(ids.has(b[0])) - Number(ids.has(a[0])));
    const list = box.querySelector<HTMLElement>('.planner-pick-list');
    if (list) {
      list.innerHTML = hits.slice(0, PICK_LIMIT).map(([id, label]) =>
        `<label class="planner-pick-option"><input type="checkbox" value="${escHtml(id)}"${ids.has(id) ? ' checked' : ''}><span>${escHtml(label)}${counts ? ` (${localizedNumber(String(counts.get(id) ?? 0))})` : ''}</span></label>`).join('')
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
    facets = plannerFacets(facetCards, { area, sev: sevActive, cls: clsActive, rule, file, entity });
    let visible = 0;
    root.querySelectorAll<HTMLElement>('.producer-card').forEach(card => {
      const index = Number(card.dataset['cardIndex']);
      const matches = facets!.matches[index] ?? null;
      matchesByCard.set(index, matches);
      const show = facets!.visible[index] ?? false;
      card.hidden = !show;
      if (show) visible++;
      refreshRows(card, index);
  // If the selected row in an open card falls outside the filter, show the first match.
      if (show && openIndex === index && matches) {
        const selected = Number(card.querySelector<HTMLElement>('.planner-affected-row.selected')?.dataset['row'] ?? -1);
        if (!matches.has(selected)) showMap(card, index, Math.min(...matches));
      }
    });
    fillSelect(ruleSelect, ruleLabels, facets.rules);
    fillSelect(fileSelect, fileLabels, facets.files);
    pickBoxes.forEach(renderPick);
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
  // Counts and narrowed option lists from the first paint, not only after the first change.
  applyFilters();
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
