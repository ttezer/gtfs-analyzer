import { describe, expect, it } from 'vitest';
import tr, { trRemediationTexts } from '../locales/tr';
import { tRemediationForLocale } from '../i18n';

// Issue #201: the engine's source text moves to English step by step. Rules already moved
// show the Turkish dictionary text in the Turkish UI; the others still show the engine text.
describe('Turkish remediation dictionary', () => {
  const moved = Object.entries(tr.ruleRemediations).filter(([key]) => !key.includes('.'));

  it('has moved rules', () => {
    expect(moved.length).toBeGreaterThan(0);
  });

  it('shows the Turkish dictionary text for every moved rule, whatever the engine sent', () => {
    for (const [ruleId, text] of moved) {
      expect(tRemediationForLocale('tr', { rule_id: ruleId, remediation: 'English engine text' })).toBe(text);
    }
  });

  it('matches a branch by the engine remediation text', () => {
    const branches = Object.entries(trRemediationTexts)
      .flatMap(([ruleId, byText]) => Object.entries(byText).map(([engine, text]) => ({ ruleId, engine, text })));
    expect(branches.length).toBeGreaterThan(0);
    for (const { ruleId, engine, text } of branches) {
      expect(tRemediationForLocale('tr', { rule_id: ruleId, remediation: engine })).toBe(text);
    }
    // Every branch of one rule maps to its own Turkish text.
    for (const byText of Object.values(trRemediationTexts)) {
      expect(new Set(Object.values(byText)).size).toBe(Object.keys(byText).length);
    }
  });

  it('keeps the engine text for a rule that is not in the dictionary', () => {
    expect(tRemediationForLocale('tr', { rule_id: 'NOPE_999', remediation: 'motorun metni' })).toBe('motorun metni');
  });
});
