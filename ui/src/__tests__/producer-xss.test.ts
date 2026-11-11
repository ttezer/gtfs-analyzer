// Feed-sourced strings (stop_name, stop_id, service_id, route names, details values…) must never reach
// the planner page as live HTML. This test renders every planner card with hostile values everywhere,
// opens each card (the open path fills more innerHTML sinks) and builds each map, then asserts the raw
// payload never appears unescaped in any HTML sink.
import { describe, it, expect, vi } from 'vitest';
import { plannerMap, renderProducer } from '../pages/producer';
import type { ValidationResult, Notice, NameIndex, EntityType } from '../types';

vi.mock('../state', () => ({ setPage: vi.fn(), getState: () => ({ agencyFilter: [] }), setAgencyFilter: vi.fn() }));
vi.mock('../hidden-rules', () => ({ renderHiddenRulesBar: () => '', attachHiddenRuleListeners: () => undefined }));
vi.mock('../map-modal', () => ({ mountInlineMap: () => () => undefined }));

const MARK = '<b id=xss>';
const X = `X${MARK}"'`; // one id that exists in every name-index map
const NAME = `N${MARK}`;

const src = Object.values(import.meta.glob(['../pages/producer.ts', '../pages/producer-helpers.ts'],
  { query: '?raw', import: 'default', eager: true }) as Record<string, string>).join('\n');
const RULES = [...new Set(src.match(/\b[A-Z]{2,3}_\d{3}\b/g) ?? [])];
const DETAIL_KEYS = [...new Set([...src.matchAll(/\b(?:d|details|dd)\??\.([a-z_][a-z_0-9]*)\b/g)].map(m => m[1]))];

function nameIndex(): NameIndex {
  const coord: [number, number] = [41, 29];
  return {
    stops: { [X]: NAME }, routes: { [X]: NAME }, trips: { [X]: NAME }, trip_routes: { [X]: X },
    trip_directions: { [X]: '0' }, stop_coords: { [X]: coord }, trip_first_dep: { [X]: NAME },
    shape_routes: { [X]: [[X, '0']] }, shape_coords: { [X]: [coord, [41.01, 29.01]] }, trip_shapes: { [X]: X },
    trip_stops: { [X]: [X, X] }, trip_stop_sequences: { [X]: [1, 2] }, shape_trips: { [X]: X },
    route_shapes: { [X]: [X] }, route_colors: { [X]: [`FF0000${MARK}`, `FFFFFF${MARK}`] },
    service_days: { [X]: ['1111100', '20260101', '20261231', '2'] },
    route_long_names: { [X]: NAME }, stop_codes: { [X]: NAME }, map_data_deferred: false,
  };
}

const ENTITY_TYPES: EntityType[] = ['Trip', 'Stop', 'Route', 'Service', 'Shape', 'Feed', 'Row'];
const VARIANTS: ((k: string) => string)[] = [() => X, () => '3', () => '08:00:00', k => `${k}${MARK}`];

function notices(rule: string): Notice[] {
  const out: Notice[] = [];
  ENTITY_TYPES.forEach((entity_type, i) => VARIANTS.forEach((v, j) => {
    out.push({
      id: `${rule}-${i}-${j}`, rule_id: rule, severity: 'HIGH', rule_class: 'SPEC', entity_type, entity_id: X,
      scope_key: X, file: 'stop_times.txt', line: 2, field: `f${MARK}`, observed_value: X, expected_value: X,
      details: Object.fromEntries(DETAIL_KEYS.map(k => [k, v(k)])),
      title: '', message: `m${MARK}`, remediation: `r${MARK}`, blocks: [], base_effort: 1, service_id: X,
    } as Notice);
  }));
  return out;
}

// Minimal fake DOM: every element records the HTML written into it and the listeners attached to it.
const sinks: string[] = [];
type Fake = Record<string, unknown>;
function fakeEl(): Fake {
  const handlers: Record<string, ((e: unknown) => void)[]> = {};
  let html = '';
  const el: Fake = {
    dataset: new Proxy({}, { get: () => '0' }), hidden: false, textContent: '', value: '', isConnected: true,
    classList: { add() {}, remove() {}, toggle() {}, contains: () => false },
    style: {}, options: [],
    setAttribute() {}, getAttribute: () => null, focus() {},
    addEventListener: (type: string, fn: (e: unknown) => void) => { (handlers[type] ??= []).push(fn); },
    querySelector: () => fakeEl(), querySelectorAll: () => [], closest: () => fakeEl(),
    handlers,
  };
  Object.defineProperty(el, 'innerHTML', { get: () => html, set: (v: string) => { html = v; sinks.push(v); } });
  return el;
}

describe('planner page XSS', () => {
  it('collects rules and detail keys from the source', () => {
    expect(RULES.length).toBeGreaterThan(100);
    expect(DETAIL_KEYS.length).toBeGreaterThan(100);
  });

  it('never writes feed values as live HTML', async () => {
    const leaks = new Map<string, string>();
    const check = (where: string, html: string) => {
      const i = html.indexOf(MARK);
      if (i >= 0 && !leaks.has(where)) leaks.set(where, html.slice(Math.max(0, i - 120), i + 20));
    };
    for (const rule of RULES) {
      const ns = notices(rule);
      const result = {
        notices: ns, name_index: nameIndex(),
        reports: { r1: { publishable: true, coverage_complete: true, blocker_notice_ids: [] },
          r9: { items: [{ rule_id: rule, priority_score: 1, affected_instance_count: ns.length, labels: [] }] } },
      } as unknown as ValidationResult;

      const headers: Fake[] = [];
      const root = fakeEl();
      root.querySelectorAll = (sel: string) => {
        if (sel !== '.planner-card-header') return [];
        const h = fakeEl();
        h.closest = () => { const card = fakeEl(); card.dataset = { cardIndex: '0' }; return card; };
        headers.push(h);
        return [h];
      };
      sinks.length = 0;
      try { renderProducer(root as unknown as HTMLElement, result); } catch (e) { leaks.set(`${rule} render threw`, String(e)); continue; }
      check(`${rule} page`, root.innerHTML as string);
      for (const h of headers) for (const fn of (h.handlers as Record<string, ((e: unknown) => void)[]>)['click'] ?? []) fn({});
      await new Promise(r => setTimeout(r, 0));
      sinks.forEach(s => check(`${rule} card`, s));

      for (const n of ns) {
        let map;
        try { map = plannerMap(n, result); } catch { continue; }
        map.opts.pins.forEach(p => check(`${rule} map pin`, p.label));
        map.opts.legendItems?.forEach(l => check(`${rule} map legend`, l.label));
      }
    }
    expect([...leaks].map(([k, v]) => `${k}: …${v}`)).toEqual([]);
  }, 120_000);
});
