import { describe, expect, it } from 'vitest';
import { buildExecutiveReportHtml, buildExecutiveReportModel } from '../executive-report';
import type { Notice, R9Item, ValidationResult } from '../types';

function notice(ruleId: string, severity: Notice['severity'] = 'MEDIUM', index = 1, ruleClass?: Notice['rule_class']): Notice {
  return {
    id: `${ruleId}-${index}`,
    rule_id: ruleId,
    severity,
    rule_class: ruleClass ?? (severity === 'CRITICAL' ? 'SPEC' : 'QUALITY'),
    entity_type: 'Row',
    entity_id: `entity-${index}`,
    scope_key: null,
    file: 'routes.txt',
    line: index,
    field: 'route_id',
    observed_value: '<script>alert(1)</script>',
    expected_value: 'valid',
    details: null,
    title: `Title ${ruleId}`,
    message: `Unsafe <img src=x onerror=alert(1)> ${ruleId}`,
    remediation: `Fix ${ruleId} & verify`,
    blocks: [],
    base_effort: 2,
  };
}

function priority(ruleId: string, labels: R9Item['labels'] = [], count = 1): R9Item {
  return {
    rule_id: ruleId,
    labels,
    priority_score: count,
    score_delta: 1.5,
    pub_score_delta: labels.includes('blocker') ? 2 : 0,
    affected_instance_count: count,
    realized_dependent_count: 0,
    base_effort: 2,
    fix_effort: 2,
    notice_ids: [`${ruleId}-1`],
  };
}

function result(): ValidationResult {
  const notices = [notice('RTS_001', 'CRITICAL'), notice('DQ_003', 'HIGH'), notice('DQ_004')];
  return {
    notices,
    reports: {
      r1: { publishable: false, coverage_complete: true, blocker_notice_ids: ['RTS_001-1'] },
      r2: { items: [] }, r3: { items: [] }, r4: { items: [] },
      r5: { score: 72.4, pub_score: 61.2, spec_score: 68, interop_score: 71, quality_score: 79, analytics_score: 83 },
      r7: { items: [] }, r8: { items: [] },
      r9: { items: [priority('DQ_003', ['high-impact'], 700), priority('RTS_001', ['blocker'], 3), priority('DQ_004', [], 2)] },
    },
    metrics: {
      stop_count: 100, route_count: 2_000, trip_count: 2_000, shape_count: 0,
      active_service_days: 30, avg_daily_trips: 120.5,
      feed_start_date: 20260701, feed_end_date: 20261231,
      service_start_date: null, service_end_date: null,
      spec_notice_count: 1, interop_notice_count: 0, quality_notice_count: 2, analytics_notice_count: 0,
      overall_score: 72.4,
      file_stats: Array.from({ length: 30 }, (_, index) => ({ name: `file-${index}.txt`, rows: 30 - index, bytes: 100 })),
    },
    name_index: {
      stops: {}, routes: {}, trips: {}, trip_routes: {}, trip_directions: {}, stop_coords: {}, trip_first_dep: {},
      shape_routes: {}, shape_coords: {}, trip_shapes: {}, trip_stops: {}, shape_trips: {}, route_shapes: {}, map_data_deferred: false,
    },
    capped_totals: { DQ_003: 25_000 },
  };
}

const build = (locale: 'tr' | 'en' | 'ja') => buildExecutiveReportModel({
  result: result(),
  fileName: 'unsafe <feed>.zip',
  generatedAt: '2026-07-21 12:00:00',
  locale,
  appVersion: '0.6.0',
  engineMode: 'wasm64-serial',
});

describe('executive report', () => {
  it('uses actual capped totals and assigns deterministic priorities', () => {
    const model = build('en');
    expect(model.totals.displayed).toBe(3);
    expect(model.totals.actual).toBe(25_002);
    expect(model.totals.capped).toBe(true);
    expect(model.findings.map(finding => [finding.ruleId, finding.priority])).toEqual([
      ['RTS_001', 'P0'], ['DQ_003', 'P1'], ['DQ_004', 'P2'],
    ]);
  });

  it('keeps the generated document bounded for large feeds', () => {
    const large = result();
    large.notices = Array.from({ length: 30 }, (_, index) => notice(`RULE_${index}`, 'MEDIUM', index));
    large.reports.r1.blocker_notice_ids = [];
    large.reports.r9.items = large.notices.map((item, index) => priority(item.rule_id, [], 100 - index));
    const model = buildExecutiveReportModel({ result: large, fileName: 'large.zip', generatedAt: 'now', locale: 'en', appVersion: 'dev', engineMode: 'test' });
    expect(model.findings).toHaveLength(8);
    expect(model.topRules).toHaveLength(8);
    expect(model.fileStats).toHaveLength(20);
  });

  it('keeps a critical quality finding out of P0 when R1 has no blocker', () => {
    const vbb = result();
    vbb.notices = [notice('PTH_014', 'CRITICAL', 1, 'QUALITY')];
    vbb.reports.r1 = { publishable: true, coverage_complete: true, blocker_notice_ids: [] };
    vbb.reports.r5 = { score: 55.4, pub_score: 100, spec_score: 100, interop_score: 88.5, quality_score: 32.8, analytics_score: 37.7 };
    vbb.reports.r9.items = [priority('PTH_014', ['quality', 'high-impact'], 1_043)];
    vbb.capped_totals = { PTH_014: 1_043 };

    const model = buildExecutiveReportModel({ result: vbb, fileName: 'vbb.zip', generatedAt: 'now', locale: 'tr', appVersion: 'dev', engineMode: 'test' });
    const finding = model.findings[0];
    expect(finding?.priority).toBe('P1');
    expect(finding?.impact).toContain('tek başına yayın engeli değildir');
    expect(finding?.impact).not.toContain('doğrudan engeller');
    expect(model.plan.find(phase => phase.priority === 'P0')).toMatchObject({ empty: true, ruleCount: 0, findingCount: 0 });
    expect(model.plan.find(phase => phase.priority === 'P1')).toMatchObject({ empty: false, ruleCount: 1, findingCount: 1_043 });
    expect(model.summary).toContain('Yayın engeli bulunmadı');
  });

  it('labels class totals as finding counts and renders a feed-specific plan', () => {
    const vbb = result();
    vbb.notices = [notice('PTH_014', 'CRITICAL', 1, 'QUALITY')];
    vbb.reports.r1 = { publishable: true, coverage_complete: true, blocker_notice_ids: [] };
    vbb.reports.r5.pub_score = 100;
    vbb.reports.r5.spec_score = 100;
    vbb.reports.r9.items = [priority('PTH_014', ['quality', 'high-impact'], 1_043)];
    vbb.capped_totals = { PTH_014: 1_043 };

    const html = buildExecutiveReportHtml(buildExecutiveReportModel({ result: vbb, fileName: 'vbb.zip', generatedAt: 'now', locale: 'tr', appVersion: 'dev', engineMode: 'test' }));
    expect(html).toContain('değerler skor değil');
    expect(html).toContain('0 <small>bulgu</small>');
    expect(html).toContain('Bulgu yok');
    expect(html).toContain('1.043 bulgu');
    expect(html).toContain('PTH_014');
    expect(html).toContain('R1 yayın engeli bulunmadı; P0 aksiyonu gerekmiyor.');
    expect(html).not.toContain('Kritik yapısal ve zorunlu alan sorunlarını giderin.');
  });

  it('renders independent Turkish, English, and Japanese reports', () => {
    const tr = buildExecutiveReportHtml(build('tr'));
    const en = buildExecutiveReportHtml(build('en'));
    const ja = buildExecutiveReportHtml(build('ja'));
    expect(tr).toContain('Yönetici Özeti');
    expect(en).toContain('Executive Summary');
    expect(ja).toContain('エグゼクティブサマリー');
    expect(tr).toContain('Feed’e Özel İyileştirme Planı');
    expect(en).toContain('Feed-Specific Remediation Plan');
    expect(ja).toContain('フィード固有の改善計画');
  });

  it('escapes feed-derived content and contains no external-validator branding', () => {
    const html = buildExecutiveReportHtml(build('tr'));
    expect(html).toContain('unsafe &lt;feed&gt;.zip');
    expect(html).toContain('Unsafe &lt;img src=x onerror=alert(1)&gt;');
    expect(html).not.toContain('<img src=x onerror=alert(1)>');
    expect(html).not.toMatch(/MobilityData|GTFS Guru/i);
    expect(html).toContain('harici bir API gerekmez');
  });

  // Bir CSS sözdizimi hatası (kapanmayan kural) ikinci bir <style> bloğu tarafından
  // maskelenmişti: hata sessizce kuralları yutuyordu ama görünüm bozulmuyordu. Tek blok
  // + denge kontrolü bu sınıfı yakalar.
  it('emits exactly one stylesheet with balanced braces', () => {
    const html = buildExecutiveReportHtml(build('en'));
    const styles = html.match(/<style[^>]*>[\s\S]*?<\/style>/g) ?? [];
    expect(styles).toHaveLength(1);
    const css = styles[0] ?? '';
    expect(css.split('{').length).toBe(css.split('}').length);
  });

  it('numbers every page section distinctly', () => {
    const eyebrows = (buildExecutiveReportHtml(build('en')).match(/class="eyebrow">(\d\d)(?!\s*·)/g) ?? []);
    expect(new Set(eyebrows).size).toBe(eyebrows.length);
  });

  it('uses an independent print stylesheet, colored cards, and no fragile tables', () => {
    const html = buildExecutiveReportHtml(build('en'));
    expect(html).toContain('id="executive-print-layout"');
    expect(html).toContain('print-color-adjust: exact');
    expect(html).toContain('score score-green');
    expect(html).toContain('finding finding-p0');
    expect(html).toContain('class="plan-cards"');
    expect(html).toContain('class="rule-list"');
    expect(html).not.toContain('<table');
  });
});
