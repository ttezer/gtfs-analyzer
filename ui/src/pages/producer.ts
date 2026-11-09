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
const PLANNER_LISTS = new Set(['agency', 'stops', 'routes', 'trips', 'stop_times', 'calendar', 'calendar_dates', 'shapes', 'frequencies', 'transfers', 'feed_info', 'fare_attributes', 'fare_rules', 'translations', 'attributions', 'pathways', 'levels']);
const PLANNER_FIELDS = new Set(['safe_duration_factor', 'safe_duration_offset', 'monday', 'tuesday', 'wednesday', 'thursday', 'friday', 'saturday', 'sunday', 'start_date', 'end_date', 'stop_id', 'stop_code', 'stop_name', 'stop_desc', 'stop_lat', 'stop_lon', 'zone_id', 'stop_url', 'parent_station', 'platform_code', 'tts_stop_name', 'route_id', 'route_short_name', 'route_long_name', 'route_desc', 'route_url', 'route_color', 'route_text_color', 'agency_id', 'agency_name', 'agency_url', 'agency_timezone', 'agency_phone', 'agency_lang', 'trip_id', 'service_id', 'trip_headsign', 'trip_short_name', 'block_id', 'shape_id', 'stop_headsign', 'arrival_time', 'departure_time', 'date', 'feed_publisher_name', 'feed_publisher_url', 'feed_lang', 'fare_id']);
function plannerListName(file?: string | null): string | null {
  const stem = file?.replace(/\.txt$/, '');
  return stem && PLANNER_LISTS.has(stem) ? t(`producer.list.${stem}`) : null;
}
function plannerFieldName(field: string): string {
  return PLANNER_FIELDS.has(field) ? t(`producer.field.${field}`) : field;
}

/** File-level cards (DQ_016, DQ_021, CAL_020) name their file in the planner's words, not as a file name. */
function dqListLabel(n: Notice): string | null {
  // CAL_020 / OPR_014 / OPR_016 / OPR_025 are about a whole list: name it, not the engine's summary ("11 years").
  if (['DQ_016', 'CAL_020', 'OPR_014', 'OPR_016', 'OPR_025', 'TRP_015'].includes(n.rule_id)) return plannerListName(n.file);
  // STP_002: the code-less row by the name written on it.
  if (n.rule_id === 'STP_002') return n.details?.stop_name || plannerListName(n.file);
  // TRP_001 on a code-less row: the row is in the trip list.
  if ((n.rule_id === 'TRP_001' || n.rule_id === 'TRP_031' || n.rule_id === 'TRP_035') && !n.entity_id) return plannerListName(n.file);
  if (['TRP_023', 'TRP_025', 'TRP_028', 'TRP_029', 'TRP_021'].includes(n.rule_id) && !n.entity_id) return plannerListName(n.file);
  const kind = n.rule_id === 'DQ_021' && n.file ? DQ021_KIND[n.file] : undefined;
  return kind ? t(`producer.card.DQ_021.list_${kind}`) : null;
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

/** A route type code as a mode planners name ("Otobüs"); extended codes by their mode family. */
function routeTypeName(code: string): string {
  const c = Number(code);
  const base = !Number.isFinite(c) ? NaN
    : c < 100 ? c
      : c < 200 ? 2 : c < 300 ? 3 : c < 500 ? 1 : c < 800 ? 3 : c < 900 ? 11 : c < 1000 ? 0
        : c < 1200 ? 4 : c < 1400 ? 6 : c < 1500 ? 7 : NaN;
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
  if ((n.rule_id === 'STP_016' || n.rule_id === 'STP_017' || n.rule_id === 'STP_020') && n.entity_id) {
    const otherId = n.rule_id === 'STP_020' ? d.nearest_used_stop_id : d.other_stop_id;
    if (!otherId) return '';
    const a = stpContext(n);
    const b = stpContext(n, n.rule_id === 'STP_020' ? 'nearest' : 'other');
    const dist = metersText(n.rule_id === 'STP_020' ? d.nearest_used_m : d.distance_m);
    return table(t(n.rule_id === 'STP_020' ? 'producer.times.stp020_pair' : 'producer.times.stp_pair'),
      [t('producer.col.stop'), t('producer.times.vat_distance'), t('producer.col.route'), t('producer.times.trip_count')], [
        { cells: [stop(n.entity_id), '—', routeLabels(a.routes, result), localizedNumber(a.trips || '0')], bad: true },
        { cells: [stop(otherId), dist, routeLabels(b.routes, result), localizedNumber(b.trips || '0')], bad: n.rule_id !== 'STP_020' },
      ]);
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
    case 'STP_013': case 'STP_014': case 'STP_015': case 'STP_016': case 'STP_017': case 'STP_019': case 'STP_020': {
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

/** Stop findings (STP_001–010) whose cards place the stop on an example trip. */
const STP_CONTEXT_RULES = new Set(['STP_001', 'STP_002', 'STP_003', 'STP_004', 'STP_005', 'STP_006', 'STP_007', 'STP_008', 'STP_009', 'STP_010',
  'STP_011', 'STP_012', 'STP_013', 'STP_014', 'STP_015', 'STP_016', 'STP_017', 'STP_019', 'STP_020']);

/** Rule groups whose cards draw one example trip with numbered stops and list it stop by stop. */
function plannerTripRule(ruleId: string): boolean {
  return ruleId.startsWith('DQ_') || ruleId.startsWith('CAL_') || ruleId.startsWith('OPR_') || ruleId.startsWith('TRP_') || ruleId === 'VAT_003';
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
    ? t('producer.calendar.title', { id: serviceId })
    : t('producer.calendar.generic_title');
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
  const parts = action.split(/(?<=[.。])\s+/).map(p => p.trim()).filter(Boolean);
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
      const hasShape = Boolean(ni.trip_shapes?.[candidateId] && ni.shape_coords?.[ni.trip_shapes[candidateId]]?.length);
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

function plannerShapeId(n: Notice, result: ValidationResult): string | undefined {
  const ni = result.name_index;
  const dqTrip = plannerTripRule(n.rule_id) && n.entity_type !== 'Trip' ? dqExampleTrip(n, ni) : undefined;
  if (dqTrip) return ni.trip_shapes?.[dqTrip];
  if (n.entity_type === 'Trip' && n.entity_id) {
    return ni.trip_shapes?.[n.entity_id]
      || ((n.rule_id === 'STM_015' || n.rule_id === 'STM_016' || n.rule_id === 'STM_018' || n.rule_id === 'STM_019' || n.rule_id === 'STM_020')
        ? routeDirectionShapeId(ni, ni.trip_routes?.[n.entity_id], ni.trip_directions?.[n.entity_id])
        : undefined);
  }
  if (n.rule_id === 'STM_014') {
    const tripId = stm014SampleTripId(n, result);
    return (tripId ? ni.trip_shapes?.[tripId] : undefined)
      || routeDirectionShapeId(ni, routeIdOfNotice(n) ?? undefined, n.details?.direction_id);
  }
  return undefined;
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
    const shape = ni.trip_shapes?.[tripId] ? ni.shape_coords?.[ni.trip_shapes[tripId]] : undefined;
    const coords = shape && shape.length > 1
      ? shape
      : (ni.trip_stops?.[tripId] ?? []).map(id => ni.stop_coords?.[id]).filter((p): p is [number, number] => Boolean(p) && !isNullIsland(p!));
    if (coords.length < 2) continue;
    lines.push({ coords, color: routeBadge(d[`route_${i}_route_id`]!, ni).bg ?? MAP_GREY, weight: 3 });
  }
}

/** Haritayı seçili kayda göre kurar: sefer → kendi güzergâhı ve durakları, işaretli durak
 *  veya bölüm kırmızı ve ona yakınlaşılmış; durak → durağın kendisi; hat → hattın çizimleri.
 *  Konuma bağlı olmayan kayıtta ağın tamamı. */
export function plannerMap(n: Notice, result: ValidationResult): PlannerMap {
  // STM_050 is a feed summary: its map is the first example row's trip, drawn as a trip finding.
  if (n.rule_id === 'STM_050' && n.entity_type !== 'Trip' && n.details?.example_0_trip_id) {
    return plannerMap({ ...n, entity_type: 'Trip', entity_id: n.details.example_0_trip_id }, result);
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
    const map = plannerMap({ ...n, entity_type: 'Trip', entity_id: dqTrip, details }, result);
    if (CAL_USAGE_RULES.has(n.rule_id)) addCalendarRoutes(map, n, result, dqTrip);
    return map;
  }
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
  const highlights: NonNullable<MapOptions['highlights']> = [];
  const stops = new Set<string>();
  const pinByStop = new Map<string, MapPin>();
  let focus: [number, number] | undefined;
  let stopRing = MAP_GREY;

  const addShape = (shapeId: string | null | undefined, color: string) => {
    const coords = shapeId ? ni.shape_coords?.[shapeId] : undefined;
    if (coords && coords.length > 1) lines.push({ coords, color, weight: 5, arrows: true });
    return Boolean(coords && coords.length > 1);
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
  const label = tripLabel || drawnTrip || dqListLabel(n) || plannerEntityLabel(n, result) || n.entity_id || '';
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
    const selectedShape = directShape
      || ((n.rule_id === 'STM_015' || n.rule_id === 'STM_016' || n.rule_id === 'STM_018' || n.rule_id === 'STM_019' || n.rule_id === 'STM_020' || n.rule_id === 'STM_021' || n.rule_id === 'STM_028' || n.rule_id === 'STM_029' || n.rule_id === 'STM_030' || n.rule_id === 'STM_032' || n.rule_id === 'STM_033' || n.rule_id === 'STM_034' || n.rule_id === 'STM_035'
        || n.rule_id === 'STM_041' || n.rule_id === 'STM_042' || n.rule_id === 'STM_043' || n.rule_id === 'STM_045' || n.rule_id === 'STM_047'
        || n.rule_id === 'STM_048' || n.rule_id === 'STM_049' || n.rule_id === 'STM_051' || n.rule_id === 'STM_052' || n.rule_id === 'STM_054' || n.rule_id === 'STM_055'
        || n.rule_id === 'STM_058' || n.rule_id === 'STM_059' || n.rule_id === 'STM_060' || n.rule_id === 'STM_050')
        ? routeDirectionShapeId(ni, ni.trip_routes?.[n.entity_id], ni.trip_directions?.[n.entity_id])
        : undefined);
    const directPath = tripStops.map(id => ni.stop_coords?.[id]).filter(Boolean).length > 1;
    const pathTripIds = n.rule_id === 'STM_006' && !directShape && !directPath
      ? representativeTripsForDirection(ni, n.entity_id)
      : [n.entity_id];
    for (const pathTripId of pathTripIds) {
      const shapeId = pathTripId === n.entity_id ? selectedShape : ni.trip_shapes?.[pathTripId];
      if (!addShape(shapeId, color)) {
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
      const shape = selectedShape ? ni.shape_coords?.[selectedShape] ?? [] : [];
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
          const shape = selectedShape ? ni.shape_coords?.[selectedShape] ?? [] : [];
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
      const shape = selectedShape ? ni.shape_coords?.[selectedShape] ?? [] : [];
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
        const shape = selectedShape ? ni.shape_coords?.[selectedShape] ?? [] : [];
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
        segments.push({ coords: [a, b], color: MAP_RED, weight: 7 });
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
      for (const [routeId, tripId] of routesHere) addShape(ni.trip_shapes?.[tripId], routeColor(routeId));
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
      segments.push({ coords: [a, b], color: MAP_RED, weight: 7 });
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
    const other = n.details.other_trip_id;
    const coords = (ni.trip_stops?.[other] ?? []).map(id => coord(id)).filter((p): p is [number, number] => p !== null);
    if (coords.length > 1) lines.push({ coords, color: MAP_RED, weight: 3, arrows: true });
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
      if (!addShape(ni.trip_shapes?.[trip], color)) {
        const coords = tripStops.map(id => coord(id)).filter((p): p is [number, number] => p !== null);
        if (coords.length > 1) lines.push({ coords, color, weight: 4, arrows: true });
      }
      if (c.stop && coord(c.stop)) addStop(c.stop, true, c.seq);
      for (const [index, stopId] of tripStops.entries()) addStop(stopId, false, sequences[index]);
    }
    const a = coord(c.prev);
    const b = coord(c.next);
    if (c.stop && !coord(c.stop) && a && b) {
      segments.push({ coords: [a, b], color: MAP_RED, weight: 7, zoomTo: true });
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
      const color = routeColor(ni.trip_routes?.[key]);
      const coords = (ni.trip_stops?.[key] ?? []).map(id => coord(id)).filter((p): p is [number, number] => p !== null);
      if (coords.length > 1) lines.push({ coords, color, weight, arrows: true });
      for (const stopId of ni.trip_stops?.[key] ?? []) addStop(stopId, stopId === mark);
    };
    const link = (from?: string | null, to?: string | null) => {
      const p = coord(from);
      const q = coord(to);
      if (p && q) segments.push({ coords: [p, q], color: MAP_RED, weight: 3 });
    };
    if (n.rule_id === 'STP_016' || n.rule_id === 'STP_017') {
      // The other stop red with its own trip, both stops linked.
      addStop(d.other_stop_id, true);
      drawTrip(stpContext(n, 'other').trip);
      link(n.entity_id, d.other_stop_id);
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
  // VAT: the routes in question in their own colours, the stops the finding is about in red.
  if (n.rule_id.startsWith('VAT_') && n.details) {
    const d = n.details;
    const drawTrip = (tripId: string | undefined, color: string, weight: number, mark: boolean | null) => {
      const tripStops = tripId ? ni.trip_stops?.[tripId] ?? [] : [];
      const coords = tripStops.map(id => coord(id)).filter((p): p is [number, number] => p !== null);
      if (coords.length > 1) lines.push({ coords, color, weight, arrows: true });
      if (mark !== null) for (const stopId of tripStops) addStop(stopId, mark);
    };
    // A route by its drawn lines (once each), else its example trip's stops; its stops grey.
    const drawRoute = (routeId: string | undefined, withStops: boolean) => {
      if (!routeId) return;
      const color = routeColor(routeId);
      let drawn = false;
      for (const shapeId of (ni.route_shapes?.[routeId] ?? []).slice(0, 2)) {
        const coords = ni.shape_coords?.[shapeId];
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
      // Each route's own stops, straight between them, so a route that leaves the shared line shows.
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
        focusZoom: n.rule_id === 'STM_015' || n.rule_id === 'STM_016' ? 14 : n.rule_id === 'STM_021' ? 17 : undefined,
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
function technicalDetails(n: Notice): string {
  const codeLabel: Record<string, string> = {
    Trip: 'producer.col.trip_code', Stop: 'producer.col.stop_code', Route: 'producer.col.route_code',
    Service: 'producer.col.service_code', Shape: 'producer.col.shape_code',
  };
  const entityId = n.entity_type === 'Route' ? routeIdOfNotice(n) : n.entity_id;
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
          const routeIds = [...new Set((n.entity_id ? ni.shape_routes?.[n.entity_id] ?? [] : []).map(([routeId]) => routeId))];
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
      <details class="producer-card-details"><summary>${t('producer.card.technical')}</summary><dl>${technicalDetails(n)}</dl></details>
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
  if ((n.rule_id === 'STP_004' || n.rule_id === 'STP_005') && n.details.example_stop_ids) {
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
    if (technicalEl) technicalEl.innerHTML = technicalDetails(notice);
    const strip = card.querySelector<HTMLElement>('.planner-times-host');
    if (strip) strip.innerHTML = timeStrip(notice, result);
    let map = plannerMap(notice, result);
    // Large-feed results deliberately omit shape geometry. Fetch the selected
    // trip's existing shape instead of drawing a synthetic stop-to-stop route.
    if (result.name_index.map_data_deferred) {
      const shapeId = plannerShapeId(notice, result);
      if (shapeId && !(shapeId in (result.name_index.shape_coords ?? {}))) {
      const { requestShapeCoords } = await import('../validator-client');
      const coords = await requestShapeCoords(shapeId);
        if (coords.length > 1) {
          result.name_index.shape_coords[shapeId] = coords;
          map = plannerMap(notice, result);
        }
      }
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
