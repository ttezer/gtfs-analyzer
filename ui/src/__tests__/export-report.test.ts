import { describe, expect, it, vi } from 'vitest';
import type { Notice, ValidationResult } from '../types';

// export.ts only needs this helper from fix.ts; mock the page module so this
// pure report-builder test does not initialize Leaflet in the Node test runtime.
vi.mock('../pages/fix', () => ({ augmentRouteLabels: vi.fn() }));
vi.mock('../state', () => ({ getState: () => ({ configDelta: '', fileSize: 0, generatedAt: null }) }));
vi.mock('../validator-client', () => ({ getLastEngineMode: () => 'test' }));
vi.mock('../debug-buffer', () => ({ getLogs: () => [], getActions: () => [] }));
vi.mock('../golden', () => ({ buildGoldenSnapshot: vi.fn() }));

import { buildReportHtml } from '../pages/export';

function notice(id: string, entityType: Notice['entity_type'] = 'Row', entityId = 'row-1'): Notice {
  return {
    id,
    rule_id: 'DQ_003',
    severity: 'MEDIUM',
    rule_class: 'QUALITY',
    entity_type: entityType,
    entity_id: entityId,
    scope_key: 'route:10',
    file: 'routes.txt',
    line: 2,
    field: 'route_long_name',
    observed_value: '<missing>',
    expected_value: 'non-empty',
    details: { route_id: '10', reason: 'value is absent' },
    title: 'Route description',
    message: 'The route has no description.',
    remediation: 'Add route_desc to routes.txt.',
    blocks: ['R5'],
    base_effort: 2,
  };
}

function result(notices: Notice[]): ValidationResult {
  return {
    validation_status: 'COMPLETE',
    notices,
    reports: {
      r1: { publishable: true, coverage_complete: true, blocker_notice_ids: [] },
      r2: { items: [] }, r3: { items: [] }, r4: { items: [] },
      r5: { score: 94.7, pub_score: 100, spec_score: 100, interop_score: 100, quality_score: 78.1, analytics_score: 90.9 },
      r7: { items: [] }, r8: { items: [] }, r9: { items: [] },
    },
    metrics: {
      stop_count: 1, route_count: 1, trip_count: 1, shape_count: 0,
      active_service_days: 1, avg_daily_trips: 1,
      feed_start_date: 20260801, feed_end_date: 20260831,
      service_start_date: 20260801, service_end_date: 20260831,
      spec_notice_count: 0, interop_notice_count: 0, quality_notice_count: notices.length, analytics_notice_count: 0,
      overall_score: 94.7,
      file_stats: [{ name: 'routes.txt', rows: 2, bytes: 100 }],
      is_gtfs_jp: true,
      gtfs_jp_profile: 'auto',
    },
    name_index: {
      stops: {}, routes: {}, trips: {}, trip_routes: {}, trip_directions: {},
      stop_coords: { 'stop-1': [35.1, 136.7] }, trip_first_dep: {},
      shape_routes: {}, shape_coords: {}, trip_shapes: {}, trip_stops: {}, shape_trips: {}, route_shapes: {},
      map_data_deferred: false,
    },
    capped_totals: {},
  };
}

describe('detailed export report', () => {
  it('includes actionable evidence in the downloaded HTML report', () => {
    const html = buildReportHtml(result([notice('n-1')]), 'feed.zip', '2026-08-31 12:00:00');

    expect(html).toContain('route_long_name');
    expect(html).toContain('&lt;missing&gt;');
    expect(html).toContain('non-empty');
    expect(html).toContain('Add route_desc to routes.txt.');
    expect(html).toContain('value is absent');
    expect(html).toContain('GTFS-JP profili');
    expect(html).not.toContain('<missing>');
  });

  it('keeps the full list in HTML but bounds large-feed print output by rule', () => {
    const full = buildReportHtml(result([notice('n-1'), notice('n-2')]), 'feed.zip', 'now');
    const print = buildReportHtml(result([notice('n-1'), notice('n-2')]), 'feed.zip', 'now', 1);

    expect((full.match(/class="notice-card"/g) ?? []).length).toBe(2);
    expect((print.match(/class="notice-card"/g) ?? []).length).toBe(1);
    expect(print).toContain('2 bulgu');
    expect(print).toContain('1 örnek');
  });

  it('adds a map link when the finding points to a known stop', () => {
    const html = buildReportHtml(result([notice('n-1', 'Stop', 'stop-1')]), 'feed.zip', 'now');

    expect(html).toContain('https://www.google.com/maps/search/?api=1&amp;query=35.1%2C136.7');
    expect(html).toContain('Haritada aç');
  });
});
