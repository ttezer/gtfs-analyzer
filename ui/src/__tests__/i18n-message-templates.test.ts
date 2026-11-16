import { describe, expect, it } from 'vitest';
import { matchTemplate, tMsgForLocale, tRemediationForLocale } from '../i18n';
import { trMessageTemplates, trRemediationTemplates } from '../locales/tr';
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

  it('translates a value remediation for every Turkish remediation template', () => {
    for (const [ruleId, templates] of Object.entries(trRemediationTemplates)) {
      for (const [engine, turkish] of Object.entries(templates)) {
        const holes = [...engine.matchAll(/\{(\w+)\}/g)].map((m) => m[1]);
        const sample = Object.fromEntries(holes.map((h, i) => [h, `v${i}x`]));
        const remediation = engine.replace(/\{(\w+)\}/g, (_, k: string) => sample[k]);
        const want = turkish.replace(/\{(\w+)\}/g, (_, k: string) => sample[k] ?? '');
        expect(tRemediationForLocale('tr', { rule_id: ruleId, remediation }), `${ruleId}: ${engine}`).toBe(want);
      }
    }
  });

  it('translates fragment holes through the fragment dictionary', () => {
    // STM_043: "Trip '{trip_id}'{f_dep_suffix} has {count} stops — …" with the shared fragment
    // " departing {t}" → " {t} kalkışlı"; an empty fragment stays empty.
    const withDep = tMsgForLocale('tr', {
      rule_id: 'STM_043',
      message: "Trip 'T1' departing 08:15 has 60 stops — a possible data merge error.",
    });
    expect(withDep).toBe("'T1' 08:15 kalkışlı seferinde 60 durak var — olası veri birleştirme hatası.");
    const withoutDep = tMsgForLocale('tr', {
      rule_id: 'STM_043',
      message: "Trip 'T1' has 60 stops — a possible data merge error.",
    });
    expect(withoutDep).toBe("'T1' seferinde 60 durak var — olası veri birleştirme hatası.");
    // A fragment inside a fragment: GEO_009's reference "from the shape of route 'R'{f_dir}".
    const nested = tMsgForLocale('tr', {
      rule_id: 'GEO_009',
      message: "Stop with code 'S1' ('Main') is 120.0m away from the shape of route 'R1' (outbound) (threshold: 100m).",
    });
    expect(nested).toBe("'S1' kodlu 'Main' durağı 'R1' hattının gidiş yönü güzergahından 120.0m uzakta (eşik: 100m).");
  });

  it('keeps an unmatched engine message as it is', () => {
    expect(tMsgForLocale('tr', { rule_id: 'NOPE_999', message: 'whatever 42' })).toBe('whatever 42');
  });
});
