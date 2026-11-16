import { describe, expect, it } from 'vitest';
import { matchTemplate, tMsgForLocale } from '../i18n';
import { trMessageTemplates } from '../locales/tr';
import sharedCases from '../../../tests/fixtures/i18n/template_match_cases.json';

type Case = { name: string; template: string; text: string; expect: Record<string, string> | null };

// Same cases as the Rust test `template_matcher_follows_the_shared_cases` (issue #201, F3).
const cases = sharedCases as Case[];

describe('message template matcher', () => {
  it('follows the shared cases', () => {
    expect(cases.length).toBeGreaterThanOrEqual(10);
    for (const c of cases) {
      expect(matchTemplate(c.template, c.text), c.name).toEqual(c.expect);
    }
  });

  it('translates a value message for every Turkish template', () => {
    for (const [ruleId, templates] of Object.entries(trMessageTemplates)) {
      for (const [engine, turkish] of Object.entries(templates)) {
        const holes = [...engine.matchAll(/\{(\w+)\}/g)].map((m) => m[1]);
        const sample = Object.fromEntries(holes.map((h, i) => [h, `v${i}x`]));
        const message = engine.replace(/\{(\w+)\}/g, (_, k: string) => sample[k]);
        const want = turkish.replace(/\{(\w+)\}/g, (_, k: string) => sample[k] ?? '');
        expect(tMsgForLocale('tr', { rule_id: ruleId, message }), `${ruleId}: ${engine}`).toBe(want);
      }
    }
  });

  it('keeps an unmatched engine message as it is', () => {
    expect(tMsgForLocale('tr', { rule_id: 'NOPE_999', message: 'whatever 42' })).toBe('whatever 42');
  });
});
