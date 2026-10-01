import { describe, expect, it } from 'vitest';
import { filterNoticesByAgency, narrowR9Items } from '../agency-filter';
import type { ValidationResult } from '../types';

const notice = (id: string) => ({ id, rule_id: id.split('#')[0] }) as unknown as ValidationResult['notices'][number];

const result = {
  notices: [notice('TRP_005#1'), notice('DQ_003#1'), notice('DQ_003#2'), notice('ARC_011#1')],
  agency_breakdown: {
    complete: true,
    agencies: [
      { agency_id: 'A', agency_name: 'Alpha', trip_count: 1, notice_indices: [0, 1] },
      { agency_id: 'B', agency_name: 'Beta', trip_count: 1, notice_indices: [0, 2] },
    ],
    rules: {},
  },
} as unknown as ValidationResult;

describe('filterNoticesByAgency', () => {
  it('returns null when nothing is selected', () => {
    expect(filterNoticesByAgency(result, [])).toBeNull();
  });
  it('keeps the selected agency and shared summaries', () => {
    expect(filterNoticesByAgency(result, ['B'])!.map(n => n.id)).toEqual(['TRP_005#1', 'DQ_003#2']);
  });
  it('unions several agencies without duplicating shared notices, in result order', () => {
    expect(filterNoticesByAgency(result, ['A', 'B'])!.map(n => n.id)).toEqual(['TRP_005#1', 'DQ_003#1', 'DQ_003#2']);
  });
  it('does not filter single-agency feeds', () => {
    const single = { ...result, agency_breakdown: { ...result.agency_breakdown!, agencies: [result.agency_breakdown!.agencies[0]] } };
    expect(filterNoticesByAgency(single, ['A'])).toBeNull();
  });
});

describe('narrowR9Items', () => {
  const item = (rule_id: string, notice_ids: string[]) =>
    ({ rule_id, notice_ids, score_delta: 1 }) as unknown as Parameters<typeof narrowR9Items>[0][number];

  it('puts a notice that survived the filter first, so the row keeps its severity and title', () => {
    const items = [item('DQ_003', ['DQ_003#1', 'DQ_003#2'])];
    const kept = [notice('DQ_003#2')];
    expect(narrowR9Items(items, kept)[0].notice_ids[0]).toBe('DQ_003#2');
  });
  it('falls back to the first filtered notice of the rule when none of its ids survived', () => {
    const items = [item('DQ_003', ['DQ_003#1'])];
    const kept = [notice('DQ_003#9')];
    expect(narrowR9Items(items, kept)[0].notice_ids[0]).toBe('DQ_003#9');
  });
  it('drops rules without a visible notice and keeps the rest of the item', () => {
    const items = [item('DQ_003', ['DQ_003#1']), item('TRP_005', ['TRP_005#1'])];
    const out = narrowR9Items(items, [notice('TRP_005#1')]);
    expect(out.map(i => i.rule_id)).toEqual(['TRP_005']);
    expect((out[0] as unknown as { score_delta: number }).score_delta).toBe(1);
  });
});
