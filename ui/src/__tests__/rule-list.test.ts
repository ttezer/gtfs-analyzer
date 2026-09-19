import { describe, expect, it } from 'vitest';
import { parseRuleIdList } from '../pages/rule-list';

describe('parseRuleIdList', () => {
  it('splits on commas, spaces and semicolons, uppercases and dedupes', () => {
    expect(parseRuleIdList(' dq_003, DQ_004 TRP_021;DQ_004 ')).toEqual(['DQ_003', 'DQ_004', 'TRP_021']);
  });
  it('returns an empty list for blank input', () => {
    expect(parseRuleIdList('  , ; ')).toEqual([]);
  });
});
