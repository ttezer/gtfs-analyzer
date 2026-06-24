import type { Severity } from './types';

const SEVERITY_ORDER: readonly Severity[] = [
  'CRITICAL',
  'HIGH',
  'MEDIUM',
  'LOW',
  'INFO',
];

export function compareSeverityThenCount(
  a: { severity: Severity; count: number; ruleId: string },
  b: { severity: Severity; count: number; ruleId: string },
): number {
  const severityOrder = SEVERITY_ORDER.indexOf(a.severity)
    - SEVERITY_ORDER.indexOf(b.severity);
  return severityOrder || b.count - a.count || a.ruleId.localeCompare(b.ruleId);
}
