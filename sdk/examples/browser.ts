import { validateGtfs, type ValidationResult } from 'gtfs-sdk';

export async function validateFile(file: File): Promise<ValidationResult> {
  const bytes = new Uint8Array(await file.arrayBuffer());
  return validateGtfs(bytes, { today: '2026-08-20' });
}

export function noticesForRule(result: ValidationResult, ruleId: string) {
  return result.notices.filter((notice) => notice.rule_id === ruleId);
}
