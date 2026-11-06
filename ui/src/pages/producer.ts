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
  return h > 0 ? t('producer.times.duration_hm', { h, m }) : t('producer.times.duration_m', { m });
}

/** "26:26:00" → seconds; NaN when the text is not h:mm:ss. */
function hmsSeconds(value?: string | null): number {
  const m = /^(\d+):(\d{2}):(\d{2})$/.exec(value ?? '');
  return m ? Number(m[1]) * 3600 + Number(m[2]) * 60 + Number(m[3]) : NaN;
}

/** A straight-line distance in metres as "350 m" or "8,4 km". */
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
          route_code: routeId || '—',
          service: serviceContext,
        }),
        what: t(geojsonLocation ? 'producer.card.STM_002.geojson_what' : 'producer.card.STM_002.what', { route_code: routeId || '—', service: serviceContext, time, time_range: timeRange }),
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
  const d = n.details;
  if (!d) return '';
  const name = (id?: string, fallback?: string) => (id ? result.name_index.stops?.[id] : undefined) || fallback || id || '—';
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
  if (['STM_037', 'STM_038', 'STM_039', 'STM_040'].includes(n.rule_id) && (d.location_id || d.location_group_id)) {
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
  } else if ((n.rule_id === 'STM_003' || n.rule_id === 'STM_004') && d.stop_id) {
  // The engine represents a malformed time as "?" and an empty field as ""; the faulty row's own value is observed.
    const time = (v?: string) => v === '?' ? t('producer.times.invalid') : v || '—';
    const raw = n.observed_value ?? '?';
    rows = [
      ...(d.prev_stop_id ? [[name(d.prev_stop_id), time(d.prev_arrival), time(d.prev_departure), false] as Row] : []),
      [name(d.stop_id), n.rule_id === 'STM_003' ? raw : time(d.arrival), n.rule_id === 'STM_004' ? raw : time(d.departure), true],
      ...(d.next_stop_id ? [[name(d.next_stop_id), time(d.next_arrival), time(d.next_departure), false] as Row] : []),
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
  if (['STM_037', 'STM_038', 'STM_039', 'STM_040'].includes(n.rule_id) && (d.location_id || d.location_group_id)) {
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

/** Haritayı seçili kayda göre kurar: sefer → kendi güzergâhı ve durakları, işaretli durak
 *  veya bölüm kırmızı ve ona yakınlaşılmış; durak → durağın kendisi; hat → hattın çizimleri.
 *  Konuma bağlı olmayan kayıtta ağın tamamı. */
export function plannerMap(n: Notice, result: ValidationResult): PlannerMap {
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
  const label = tripLabel || entityLabel(n, ni) || n.entity_id || '';
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
        || n.rule_id === 'STM_048' || n.rule_id === 'STM_049')
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
    } else if (['STM_032', 'STM_033', 'STM_034', 'STM_035', 'STM_041', 'STM_042', 'STM_045', 'STM_047', 'STM_048', 'STM_049'].includes(n.rule_id)) {
      // Stop-row findings: the route and direction with every stop, the faulty stop(s) in red,
      // the neighbours of a repeated stop in grey.
      const stopSequences = ni.trip_stop_sequences?.[n.entity_id] ?? [];
      const d = n.details ?? {};
      if (n.rule_id === 'STM_047') {
        for (let i = 0; d[`example_${i}_stop_id`]; i++) addStop(d[`example_${i}_stop_id`], true, d[`example_${i}_stop_sequence`]);
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
      for (const stopId of tripStops) addStop(stopId, false);
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
    if (n.rule_id === 'STM_014') {
      const sampleTripId = stm014SampleTripId(n, result);
      const tripStops = sampleTripId ? ni.trip_stops?.[sampleTripId] ?? [] : [];
      const stopSequences = sampleTripId ? ni.trip_stop_sequences?.[sampleTripId] ?? [] : [];
      const sampleShape = (sampleTripId ? ni.trip_shapes?.[sampleTripId] : undefined)
        || routeDirectionShapeId(ni, routeId, n.details?.direction_id);
      if (!addShape(sampleShape, color)) {
        const coords = tripStops.map(id => coord(id)).filter((p): p is [number, number] => p !== null);
        if (coords.length > 1) lines.push({ coords, color, weight: 4, arrows: true });
      }
      for (const [index, stopId] of tripStops.entries()) addStop(stopId, false, stopSequences[index]);
    } else {
      for (const shapeId of (ni.route_shapes?.[routeId] ?? []).slice(0, 4)) addShape(shapeId, color);
    }
  // Route-level speed findings (STM_014, STM_061) carry the faulty stop pair.
    const a = coord(n.details?.stop_a);
    const b = coord(n.details?.stop_b);
    if (a && b) {
      segments.push({ coords: [a, b], color: MAP_RED, weight: 7 });
      addStop(n.details?.stop_a, true);
      addStop(n.details?.stop_b, true);
    }
    if (n.rule_id === 'STM_014') {
      addStop(n.details?.prev_stop_id, false);
      addStop(n.details?.next_stop_id, false);
    }
  } else if (n.entity_type === 'Shape' && n.entity_id) {
    addShape(n.entity_id, routeColor(ni.shape_routes?.[n.entity_id]?.[0]?.[0]));
  }
  if (n.rule_id !== 'STM_018' && n.rule_id !== 'STM_019') addStop(n.details?.stop_id, true);

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
  const base = entityColumns(g, result);
  // For rules that do not provide their own evidence, each row carries its own observed value.
  const copy = plannerCardCopy(g, result);
  if ((copy && !copy.generic) || !g.notices.some(n => observedValue(n))) return base;
  return {
    head: [...base.head, t('producer.card.observed')],
    row: n => {
      const o = observedValue(n);
      const cell: Cell = o
        ? { html: `${escHtml(o.value)}${o.expected ? ` <span class="planner-expected">${escHtml(t('producer.card.expected_inline', { value: o.expected }))}</span>` : ''}` }
        : '—';
      return [...base.row(n), cell];
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
    ? { head: [...cols, t('producer.col.service')], row: (n: Notice) => [...row(n), serviceText(n.service_id, result)] }
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
              routeId ? { html: `${routeBadgeHtml(routeId, result)} <span class="planner-route-code">${escHtml(routeId)}</span>` } : '—',
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
      return {
        head: [t('producer.col.route'), t('producer.col.route_name'), t('producer.col.route_code'), ...(withSpot ? [t('producer.col.spot')] : [])],
        row: n => {
          const routeId = routeIdOfNotice(n);
          return [
            badge(routeId), dash(routeId ? ni.route_long_names?.[routeId] : null), dash(routeId),
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
          entityLabel(n, ni) || n.observed_value || t('producer.map.unknown_context'),
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
