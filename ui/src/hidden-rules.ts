import { getState, setConfigDelta, setResult, setPage } from './state';
import { renderApp } from './main';
import { parseHiddenRules, withHiddenRule, withoutHiddenRule } from './hidden-rules-core';
import { t } from './i18n';
import { escHtml } from './escape';
import { rerunValidation } from './validator-client';

/**
 * Rule IDs hidden by the user are stored in `configDelta`'s `disabled_rule_ids`
 * field — the same place written by the settings panel. The engine applies this list
 * BEFORE K7: a hidden rule appears neither in findings, scores, nor the fix queue.
 *
 * ⚠️ Spec-class rules CANNOT be added here: publication decisions depend on them, and the
 * engine (`gtfs_pipeline::check_rule_scope`) rejects such a request as a fatal error.
 * The UI therefore never shows a hide button for Spec rules.
 */
export function hiddenRules(): string[] {
  return parseHiddenRules(getState().configDelta);
}

export function isHidden(ruleId: string): boolean {
  return hiddenRules().includes(ruleId);
}

/** Hides a rule. No-op if already hidden; returns whether the list changed. */
export function hideRule(ruleId: string): boolean {
  const next = withHiddenRule(getState().configDelta, ruleId);
  if (next === null) return false;
  setConfigDelta(next);
  return true;
}

/** Makes a rule visible again. Returns whether the list changed. */
export function unhideRule(ruleId: string): boolean {
  const next = withoutHiddenRule(getState().configDelta, ruleId);
  if (next === null) return false;
  setConfigDelta(next);
  return true;
}

/**
 * Bar for hidden rules. Appears at the top of EVERY report page (Report and
 * Details/Fix): users must be able to see what is excluded from measurement and restore it
 * with one click, regardless of the page they are on.
 */
export function renderHiddenRulesBar(): string {
  const hidden = hiddenRules();
  if (hidden.length === 0) return '';
  const chips = hidden.map(id =>
    `<button class="hidden-rule-chip" data-rule="${escHtml(id)}" title="${t('fix.unhide_rule')}">${escHtml(id)} ↩</button>`
  ).join('');
  return `
    <div class="card hidden-rules-card">
      <p class="hint">${t('fix.hidden_note', { count: hidden.length })}</p>
      <div class="hidden-rule-chips">${chips}</div>
    </div>`;
}

/**
 * Hiding/showing follows one path: update the list → rerun K6+K7 from cache → refresh the
 * result. K1–K5 are NOT rerun, so this is immediate even on a large feed.
 * `setResult` returns the page to 'domain', so the user's original page is restored.
 */
// `main` is imported statically: `pages/upload.ts` does the same, so the module
// cycle already exists and is resolved by ESM. A dynamic import produced Vite's
// "both static and dynamic" warning.
export async function applyRuleVisibility(changed: boolean): Promise<void> {
  if (!changed) return;
  const state = getState();
  const page = state.page;
  // Rerendering rebuilds the page from scratch: the R9 accordion closes, the severity filter
  // selection disappears, and the page returns to the top. The user should stay where they were.
  // Class/file filters live in state and are preserved automatically; severity chips live only
  // in the DOM, so they are captured here.
  const r9Open = document.querySelector<HTMLDetailsElement>('.r9-card')?.open ?? false;
  const scrollY = window.scrollY;
  const activeSev = (id: string): string[] => {
    const chips = Array.from(document.querySelectorAll(`#${id} .sev-chip.active`)) as HTMLButtonElement[];
    return chips.map(chip => chip.dataset['sev'] ?? '').filter(Boolean);
  };
  const sevSelection = { r2: activeSev('r2-sev-filter'), r9: activeSev('r9-sev-filter') };
  try {
    const fresh = await rerunValidation(state.configDelta);
    setResult(fresh, state.fileName, state.fileSize ?? 0, state.reportDurationMs);
    setPage(page);
  } catch {
    // If the rerun fails, the setting is still saved and applies on the next run.
  }
  renderApp();
  const r9 = document.querySelector<HTMLDetailsElement>('.r9-card');
  if (r9 && r9Open) r9.open = true;
  // Clicking the chip restores both the 'active' marker and the filter; DUPLICATING the
  // filter logic here would create two copies.
  for (const [id, sevs] of [['r2-sev-filter', sevSelection.r2], ['r9-sev-filter', sevSelection.r9]] as const) {
    for (const sev of sevs) {
      document.querySelector<HTMLButtonElement>(`#${id} .sev-chip[data-sev="${sev}"]`)?.click();
    }
  }
  window.scrollTo({ top: scrollY });
}

/** Listeners for bar chips and hide buttons in finding rows. */
export function attachHiddenRuleListeners(root: HTMLElement): void {
  root.querySelectorAll<HTMLButtonElement>('.hide-rule-btn').forEach(btn => {
    btn.addEventListener('click', e => {
      e.stopPropagation();  // R9 row clicks open it; hiding must not trigger that.
      const rule = btn.dataset['rule'];
      if (rule) void applyRuleVisibility(hideRule(rule));
    });
  });
  root.querySelectorAll<HTMLButtonElement>('.hidden-rule-chip').forEach(chip => {
    chip.addEventListener('click', () => {
      const rule = chip.dataset['rule'];
      if (rule) void applyRuleVisibility(unhideRule(rule));
    });
  });
}
