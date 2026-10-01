import { describe, expect, it } from 'vitest';
import { filterNoticesByAgency } from '../agency-filter';
import type { ValidationResult } from '../types';

const notice = (id: string) => ({ id, rule_id: id.split('#')[0] }) as unknown as ValidationResult['notices'][number];

const result = {
  notices: [notice('TRP_005#1'), notice('DQ_003#1'), notice('DQ_003#2'), notice('ARC_011#1')],
  agency_breakdown: {
    complete: true,
    agencies: [
      { agency_id: 'A', agency_name: 'Alpha', trip_count: 1, affected_trip_count: 0, notice_indices: [0, 1] },
      { agency_id: 'B', agency_name: 'Beta', trip_count: 1, affected_trip_count: 0, notice_indices: [0, 2] },
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
