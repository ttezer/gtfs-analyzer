import { describe, expect, it, vi } from 'vitest';
import { renderMetrics } from '../pages/domain';
import type { FeedMetrics } from '../types';
import { getLocale, setLocale, t } from '../i18n';

vi.mock('../state', () => ({ getState: () => ({}), setPage: vi.fn(), setFixClassFilter: vi.fn() }));
vi.mock('../validator-client', () => ({}));
vi.mock('../pages/feed-summary', () => ({ renderFeedSummary: () => '' }));
vi.mock('../pages/agency-breakdown', () => ({ renderAgencyBreakdown: () => '', attachAgencyBreakdownListeners: () => undefined }));
vi.mock('../hidden-rules', () => ({ renderHiddenRulesBar: () => '', attachHiddenRuleListeners: () => undefined }));

const metrics = {
  stop_count: 2, route_count: 1, trip_count: 4, shape_count: 0,
  active_service_days: 1, avg_daily_trips: 4,
  feed_start_date: null, feed_end_date: null, service_start_date: null, service_end_date: null,
  spec_notice_count: 0, interop_notice_count: 0, quality_notice_count: 0,
  analytics_notice_count: 0, overall_score: 100, file_stats: [],
} satisfies FeedMetrics;

describe('headway evaluation coverage', () => {
  it('shows frequency-template exclusions in the feed summary', () => {
    const html = renderMetrics({ ...metrics, headway_coverage: {
      route_direction_profiles: 3, pattern_profiles: 5, merged_departure_rows: 20,
      largest_merged_departures: 8, opr_001_skipped_profiles: 2,
      opr_001_skipped_days: 4, opr_003_skipped_pattern_profiles: 1,
    } });
    expect(html).toContain('rpt-coverage-note');
    expect(html).toContain('OPR_001');
    expect(html).toContain('OPR_003');
    expect(html).toContain('2 hat/yön profili (4 farklı tarih)');
  });

  it('does not show an exclusion note for fully evaluated profiles', () => {
    expect(renderMetrics(metrics)).not.toContain('rpt-coverage-note');
  });

  it('has profile and coverage copy in all four supported languages', () => {
    const previous = getLocale();
    try {
      for (const locale of ['tr', 'en', 'ja', 'fr'] as const) {
        setLocale(locale);
        expect(t('producer.card.opr_profile_one', { date: '2026-11-30', service_count: 2, services: 'A, B' })).toContain('A, B');
        expect(t('producer.card.opr_profile_many', { date: '2026-11-30', count: 2, service_count: 2, services: 'A, B' })).toContain('2026-11-30');
        expect(t('producer.card.OPR_001.what_profile', { route: 'R', dir: '', from: '08:00', to: '13:00', gap: '5h', limit: '4h', context: 'profile context' })).toContain('profile context');
        expect(t('producer.card.OPR_003.what_profile', { route: 'R', dir: '', stop: 'X', from: '08:00', to: '08:01', gap: '1m', context: 'profile context' })).toContain('profile context');
        expect(t('domain.headway_coverage', { profiles: 1, days: 2, patterns: 3 })).toContain('OPR_001');
      }
    } finally {
      setLocale(previous);
    }
  });
});
