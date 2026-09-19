/** "dq_003, DQ_004 TRP_021" → ["DQ_003","DQ_004","TRP_021"]; virgül/boşluk ayırır, tekrarı atar. */
export function parseRuleIdList(raw: string): string[] {
  const ids = raw.split(/[\s,;]+/).map(v => v.trim().toUpperCase()).filter(v => v.length > 0);
  return [...new Set(ids)];
}

