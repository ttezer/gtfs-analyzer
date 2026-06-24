import type { Severity } from './types';

// Canonical severity ordering for the whole UI (worst -> least severe).
// Import SEVERITY_ORDER for iteration/listing and SEVERITY_RANK for O(1)
// comparisons instead of re-declaring the order in each page.
export const SEVERITY_ORDER: readonly Severity[] = [
  'CRITICAL',
  'HIGH',
  'MEDIUM',
  'LOW',
  'INFO',
];

export const SEVERITY_RANK: Record<Severity, number> = Object.fromEntries(
  SEVERITY_ORDER.map((severity, index) => [severity, index]),
) as Record<Severity, number>;

export function compareSeverityThenCount(
  a: { severity: Severity; count: number; ruleId: string },
  b: { severity: Severity; count: number; ruleId: string },
): number {
  const severityOrder = SEVERITY_RANK[a.severity] - SEVERITY_RANK[b.severity];
  return severityOrder || b.count - a.count || a.ruleId.localeCompare(b.ruleId);
}
