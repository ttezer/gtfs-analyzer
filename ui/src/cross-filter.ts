/** One row of the "Details and Fix" finding table as its filters see it. */
export interface CrossFilterRow { sev: string; cls: string; rule: string; file: string }

/** The table's filter state: severity and class are multi-select (empty = all). */
export interface CrossFilterState { sev: Set<string>; cls: Set<string>; rule: string; file: string }

export interface CrossFilterResult {
  /** Per row: shown under every filter. */
  visible: boolean[];
  /** Rule and file options, each counted under every OTHER active filter (its own pick excluded),
   *  as on the planner page: the number of rows the option would show. */
  rules: Map<string, number>;
  files: Map<string, number>;
}

/** Row visibility and the rule/file option counts, in one pass so they cannot disagree. */
export function crossFilter(rows: CrossFilterRow[], f: CrossFilterState): CrossFilterResult {
  const out: CrossFilterResult = { visible: [], rules: new Map(), files: new Map() };
  const add = (map: Map<string, number>, key: string) => map.set(key, (map.get(key) ?? 0) + 1);
  for (const row of rows) {
    const sev = f.sev.size === 0 || f.sev.has(row.sev);
    const cls = f.cls.size === 0 || f.cls.has(row.cls);
    const rule = !f.rule || row.rule === f.rule;
    const file = !f.file || row.file === f.file;
    out.visible.push(sev && cls && rule && file);
    if (sev && cls && file) add(out.rules, row.rule);
    if (sev && cls && rule && row.file) add(out.files, row.file);
  }
  return out;
}
