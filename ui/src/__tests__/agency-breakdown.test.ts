import { describe, expect, it } from 'vitest';
import { agencyRows, findingDensity } from '../pages/agency-breakdown';
import type { AgencyBreakdown, Severity } from '../types';

const breakdown: AgencyBreakdown = {
  complete: true,
  agencies: [{ agency_id: 'A', agency_name: 'Alpha', trip_count: 200 }, { agency_id: 'B', agency_name: '', trip_count: 0 }],
  rules: {
    TRP_005: {
      finding_count: 1, affected_entity_count: 3, displayed_sample_count: 1,
      agency_sets: [
        { agency_ids: ['A'], affected_entity_count: 1, trim_fallback_count: 0 },
        { agency_ids: ['B'], affected_entity_count: 1, trim_fallback_count: 0 },
      ],
      unattributed: { UnknownTrip: 1 }, unsupported: 0, not_applicable: 0,
    },
    ARC_011: {
      finding_count: 2, affected_entity_count: 2, displayed_sample_count: 2,
      agency_sets: [], unattributed: {}, unsupported: 0, not_applicable: 2,
    },
  },
};

describe('agencyRows', () => {
  it('splits affected records by agency and severity, keeping the rest visible', () => {
    const severity = new Map<string, Severity>([['TRP_005', 'MEDIUM'], ['ARC_011', 'INFO']]);
    const [a, b, unattributed, unsupported, notApplicable] = agencyRows(breakdown, severity);
    expect([a.label, a.agencyId, a.total, a.bySeverity.MEDIUM]).toEqual(['Alpha', 'A', 1, 1]);
    // Ad yoksa kimlik gösterilir.
    expect([b.label, b.total]).toEqual(['B', 1]);
    expect(unattributed.total).toBe(1);
    expect(unsupported.total).toBe(0);
    expect([notApplicable.total, notApplicable.bySeverity.INFO]).toEqual([2, 2]);
    // Satırların toplamı dökümün etkilenen kayıt toplamına eşittir.
    const total = [a, b, unattributed, unsupported, notApplicable].reduce((s, r) => s + r.total, 0);
    expect(total).toBe(5);
    expect(a.rules.get('TRP_005')).toBe(1);
    // Yoğunluk: 1.000 sefer başına; seferi olmayan agency ve agency dışı satırlar için yok.
    expect(findingDensity(a.total, a.tripCount)).toBe(5);
    expect(findingDensity(b.total, b.tripCount)).toBeNull();
    expect(findingDensity(notApplicable.total, notApplicable.tripCount)).toBeNull();
  });
});
