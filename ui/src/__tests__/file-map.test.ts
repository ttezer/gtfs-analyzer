import { describe, expect, it } from 'vitest';
import { buildFileSummaries } from '../file-summary';
import {
  CORE_FILE_IDS,
  FILE_MAP_EDGES,
  FILE_MAP_NODES,
  getBaseVisibleFileIds,
  getDirectlyRelatedFileIds,
  getVisibleFileIdsForSelection,
  validateFileMapSchema,
} from '../file-map-schema';
import type { Notice, ValidationResult } from '../types';
import { compareSeverityThenCount } from '../severity-order';

function notice(
  id: string,
  ruleId: string,
  file: string,
  severity: Notice['severity'],
): Notice {
  return {
    id,
    rule_id: ruleId,
    severity,
    rule_class: 'SPEC',
    entity_type: 'File',
    entity_id: file,
    scope_key: null,
    file,
    line: null,
    field: null,
    observed_value: null,
    expected_value: null,
    details: null,
    title: ruleId,
    message: `${ruleId} message`,
    remediation: '',
    blocks: [],
    base_effort: 1,
  };
}

function validationResult(notices: Notice[]): ValidationResult {
  return {
    notices,
    capped_totals: {},
    name_index: {
      stops: {},
      routes: {},
      trips: {},
      trip_routes: {},
      trip_directions: {},
      stop_coords: {},
      trip_first_dep: {},
      shape_routes: {},
      shape_coords: {},
      trip_shapes: {},
      trip_stops: {},
      shape_trips: {},
      route_shapes: {},
      map_data_deferred: false,
    },
    metrics: {
      stop_count: 0,
      route_count: 0,
      trip_count: 0,
      shape_count: 0,
      active_service_days: 0,
      avg_daily_trips: 0,
      feed_start_date: null,
      feed_end_date: null,
      service_start_date: null,
      service_end_date: null,
      spec_notice_count: notices.length,
      interop_notice_count: 0,
      quality_notice_count: 0,
      analytics_notice_count: 0,
      overall_score: 100,
      file_stats: [
        { name: 'calendar.txt', rows: 12, bytes: 240 },
        { name: 'stops.txt', rows: 3, bytes: 120 },
      ],
    },
    reports: {
      r1: {
        publishable: true,
        blocker_notice_ids: [],
      },
      r2: { items: [] },
      r3: { items: [] },
      r4: { items: [] },
      r5: {
        score: 100,
        pub_score: 100,
        spec_score: 100,
        interop_score: 100,
        quality_score: 100,
        analytics_score: 100,
      },
      r7: { items: [] },
      r8: { items: [] },
      r9: { items: [] },
    },
  };
}

describe('GTFS file map schema', () => {
  it('contains exactly 32 unique file nodes with valid edges', () => {
    expect(FILE_MAP_NODES).toHaveLength(32);
    expect(new Set(FILE_MAP_NODES.map((node) => node.id)).size).toBe(32);
    expect(FILE_MAP_EDGES.length).toBeGreaterThan(20);
    expect(validateFileMapSchema()).toEqual([]);
  });

  it('derives gtfs.org spec anchors by dropping the dot but keeping underscores', () => {
    const urlFor = (id: string) =>
      FILE_MAP_NODES.find((node) => node.id === id)?.specUrl;
    const base = 'https://gtfs.org/documentation/schedule/reference/';
    expect(urlFor('agency.txt')).toBe(`${base}#agencytxt`);
    expect(urlFor('stop_times.txt')).toBe(`${base}#stop_timestxt`);
    expect(urlFor('calendar_dates.txt')).toBe(`${base}#calendar_datestxt`);
    expect(urlFor('locations.geojson')).toBe(`${base}#locationsgeojson`);
    expect(urlFor('fare_leg_join_rules.txt')).toBe(`${base}#fare_leg_join_rulestxt`);
  });

  it('defines the seven-file core view and direct trip relationships', () => {
    expect(CORE_FILE_IDS).toEqual([
      'agency.txt',
      'routes.txt',
      'trips.txt',
      'stop_times.txt',
      'stops.txt',
      'calendar.txt',
      'calendar_dates.txt',
    ]);
    const related = getDirectlyRelatedFileIds('trips.txt');
    expect(related.has('routes.txt')).toBe(true);
    expect(related.has('stop_times.txt')).toBe(true);
    expect(related.has('calendar.txt')).toBe(true);
    expect(related.has('calendar_dates.txt')).toBe(true);
    expect(related.has('shapes.txt')).toBe(true);
    expect(related.has('fare_products.txt')).toBe(false);
  });

  it('shows only core files plus related non-core files that have findings', () => {
    const base = getBaseVisibleFileIds([
      { id: 'feed_info.txt', hasFindings: true, unknown: false },
      { id: 'assignments.txt', hasFindings: true, unknown: true },
      { id: 'shapes.txt', hasFindings: false, unknown: false },
    ]);
    const visible = getVisibleFileIdsForSelection(
      'trips.txt',
      base,
      (fileId) => fileId === 'shapes.txt',
    );
    expect(visible.has('feed_info.txt')).toBe(true);
    expect(visible.has('assignments.txt')).toBe(true);
    expect(visible.has('shapes.txt')).toBe(true);
    expect(visible.has('frequencies.txt')).toBe(false);
    expect(visible.has('attributions.txt')).toBe(false);
    expect(visible.has('routes.txt')).toBe(true);
    expect(visible.has('calendar.txt')).toBe(true);
  });
});

describe('file summaries', () => {
  it('aggregates severity counts and marks absent optional files as missing for the map', () => {
    const result = validationResult([
      notice('n1', 'CAL_006', 'calendar.txt', 'LOW'),
      notice('n2', 'CAL_012', 'calendar.txt', 'MEDIUM'),
      notice('n3', 'CAL_012', 'calendar.txt', 'MEDIUM'),
    ]);
    const summaries = buildFileSummaries(result, {
      includeAllSpec: true,
      includeGeneral: false,
    });
    const calendar = summaries.find((summary) => summary.name === 'calendar.txt');
    const shapes = summaries.find((summary) => summary.name === 'shapes.txt');

    expect(calendar?.notices).toHaveLength(3);
    expect(calendar?.severityCounts.MEDIUM).toBe(2);
    expect(calendar?.severityCounts.LOW).toBe(1);
    expect(calendar?.worstSeverity).toBe('MEDIUM');
    expect(calendar?.missing).toBe(false);
    expect(shapes?.missing).toBe(true);
  });

  it('orders rule groups by severity before occurrence count', () => {
    const grouped = [
      { ruleId: 'LOW_MANY', severity: 'LOW' as const, count: 20 },
      { ruleId: 'INFO_RULE', severity: 'INFO' as const, count: 100 },
      { ruleId: 'CRITICAL_RULE', severity: 'CRITICAL' as const, count: 1 },
      { ruleId: 'HIGH_RULE', severity: 'HIGH' as const, count: 2 },
      { ruleId: 'MEDIUM_RULE', severity: 'MEDIUM' as const, count: 3 },
    ].sort(compareSeverityThenCount);

    expect(grouped.map((group) => group.severity)).toEqual([
      'CRITICAL',
      'HIGH',
      'MEDIUM',
      'LOW',
      'INFO',
    ]);
  });
});
