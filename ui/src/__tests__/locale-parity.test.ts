import { describe, it, expect } from 'vitest';
import tr from '../locales/tr';
import en from '../locales/en';
import ja from '../locales/ja';
import fr from '../locales/fr';
import es from '../locales/es';
import enExport from '../../../crates/core/locales/en.json';
import jaExport from '../../../crates/core/locales/ja.json';
import frExport from '../../../crates/core/locales/fr.json';
import trExport from '../../../crates/core/locales/tr.json';
import esExport from '../../../crates/core/locales/es.json';

const keys = (o: Record<string, string>): Set<string> => new Set(Object.keys(o));
const minus = (a: Set<string>, b: Set<string>): string[] => [...a].filter((x) => !b.has(x));

describe('locale parity (registry kural anahtarları)', () => {
  it('ruleTitles anahtarları tr/en/ja arasında birebir aynı', () => {
    const t = keys(tr.ruleTitles);
    const e = keys(en.ruleTitles);
    const j = keys(ja.ruleTitles);
    expect(minus(t, e)).toEqual([]); // tr'de olup en'de olmayan
    expect(minus(e, t)).toEqual([]); // en'de olup tr'de olmayan
    expect(minus(e, j)).toEqual([]); // en'de olup ja'da olmayan
    expect(minus(j, e)).toEqual([]); // present in ja but absent in en (orphan → RTS_015 class)
    const r = keys(fr.ruleTitles);
    expect(minus(e, r)).toEqual([]); // en'de olup fr'de olmayan
    expect(minus(r, e)).toEqual([]); // fr'de olup en'de olmayan (orphan)
  });

  it('ruleMessages içinde ja anahtarları en tarafından kapsanır (tr bilinçli boş)', () => {
    expect(Object.keys(tr.ruleMessages)).toHaveLength(0);
    const e = keys(en.ruleMessages);
    const j = keys(ja.ruleMessages);
    // Present in en but absent in ja: ALLOWED. When the `ja` dictionary lacks a translation,
    // it falls back to English (crates/core/src/i18n.rs `fallback`), so this is translation
    // debt, not Turkish leakage. The Rust-side `every_registered_rule_resolves_in_every_dictionary`
    // guard enforces `en` completeness.
    expect(minus(j, e)).toEqual([]);
  });

  it('ruleRemediations içinde ja ve tr anahtarları en tarafından kapsanır', () => {
    const e = keys(en.ruleRemediations);
    // tr is filled step by step (issue #201): every Turkish key's rule must exist in en.
    // `RULE.variant` keys name a branch the English text does not split; English falls back to RULE.
    const trRules = new Set(Object.keys(tr.ruleRemediations).map((k) => k.split('.')[0]));
    expect(minus(trRules, e)).toEqual([]);
    const j = keys(ja.ruleRemediations);
    // Present in en but absent in ja: ALLOWED. When the `ja` dictionary lacks a translation,
    // it falls back to English (crates/core/src/i18n.rs `fallback`), so this is translation
    // debt, not Turkish leakage. The Rust-side `every_registered_rule_resolves_in_every_dictionary`
    // guard enforces `en` completeness.
    expect(minus(j, e)).toEqual([]); // ja'da olup en'de olmayan (orphan)
  });
});

// The Rust CLI's `--lang` output is DERIVED from these locales: `npm run locales:export`
// generates crates/core/locales/{en,ja}.json, which the CLI embeds with include_str!. If the
// locale is updated without running export, the CLI publishes stale text — this test catches it.
describe('CLI locale export (crates/core/locales)', () => {
  it.each([
    ['en', en, enExport],
    ['ja', ja, jaExport],
    ['fr', fr, frExport],
  ] as const)('%s.json locale ile aynı (bayat ise: npm run locales:export)', (_lang, locale, exported) => {
    expect(exported.messages).toEqual(locale.ruleMessages);
    expect(exported.remediations).toEqual(locale.ruleRemediations);
    expect(exported.titles).toEqual(locale.ruleTitles);
  });

  it.each([
    ['tr', tr.ui, trExport],
    ['en', en.ui, enExport],
    ['ja', ja.ui, jaExport],
    ['fr', fr.ui, frExport],
    ['es', es.ui, esExport],
  ] as const)('%s.json kartları locale ile aynı (bayat ise: npm run locales:export)', (_lang, ui, exported) => {
    expect(exported.cards).toEqual(cardsOf(ui));
  });
});

// ── planner cards (producer.card.<RULE>.{title,what,action}) ────────────────
// The CLI publishes these per rule (`card` in --compact-json and `rules --json`), so every
// language must carry the same rules and fields; `es` is a cards-only locale.

const CARD_KEY = /^producer\.card\.([A-Z]+_\d+[a-z]*)\.(title|what|action)$/;

function cardsOf(ui: Record<string, string>): Record<string, Record<string, string>> {
  const cards: Record<string, Record<string, string>> = {};
  for (const [key, text] of Object.entries(ui)) {
    const match = key.match(CARD_KEY);
    if (!match) continue;
    (cards[match[1]] ??= {})[match[2]] = text;
  }
  return cards;
}

const cardKeys = (ui: Record<string, string>): Set<string> =>
  new Set(Object.keys(ui).filter((key) => CARD_KEY.test(key)));

describe('planner card parity', () => {
  it.each([
    ['tr', tr.ui],
    ['ja', ja.ui],
    ['fr', fr.ui],
    ['es', es.ui],
  ] as const)('%s kart anahtarları en ile birebir aynı', (_lang, ui) => {
    const e = cardKeys(en.ui);
    const other = cardKeys(ui);
    expect(minus(e, other)).toEqual([]);
    expect(minus(other, e)).toEqual([]);
  });

  // The planner page falls back to English when a key is missing, so a fr/ja planner would
  // silently see English text. Every `producer.*` key must exist in all four page languages.
  it.each([
    ['tr', tr.ui],
    ['ja', ja.ui],
    ['fr', fr.ui],
  ] as const)('%s planlamacı sayfası anahtarları (producer.*) en ile birebir aynı', (_lang, ui) => {
    const producerKeys = (o: Record<string, string>): Set<string> =>
      new Set(Object.keys(o).filter((key) => key.startsWith('producer.')));
    const e = producerKeys(en.ui);
    const other = producerKeys(ui);
    expect(minus(e, other)).toEqual([]);
    expect(minus(other, e)).toEqual([]);
  });

  it('es yalnız kart anahtarı taşır', () => {
    expect(Object.keys(es.ui).filter((key) => !CARD_KEY.test(key))).toEqual([]);
  });

  it("es kartlarında Türkçe'ye özgü harf yok ve yer tutucular en ile aynı", () => {
    const problems: string[] = [];
    for (const [key, text] of Object.entries(es.ui)) {
      if (TR_ONLY.test(text)) problems.push(`${key}: Türkçe harf`);
      const enText = en.ui[key as keyof typeof en.ui] as string | undefined;
      if (enText !== undefined && holders(text).join() !== holders(enText).join()) {
        problems.push(`${key}: es=${holders(text).join()} en=${holders(enText).join()}`);
      }
    }
    expect(problems).toEqual([]);
  });
});

// ── fr guards (WP1) ──────────────────────────────────────────────────────────
// The `fr` dictionary is being filled gradually; a missing key falls back to English
// (i18n.ts:61,63,106,120), so a gap is translation debt. These two guards catch not gaps but
// WRONG FILLING — both automatically cover new entries as the dictionary grows.

/** Fransızca'da bulunmayan Türkçe harfler — tr.ts'ten kopyala-yapıştırın imzası. */
const TR_ONLY = /[şğİıŞĞ]/;

const FR_SECTIONS: [string, Record<string, string>][] = [
  ['ui', fr.ui],
  ['severity', fr.severity],
  ['ruleClass', fr.ruleClass],
  ['fatalCode', fr.fatalCode],
  ['ruleTitles', fr.ruleTitles],
  ['ruleMessages', fr.ruleMessages],
  ['ruleRemediations', fr.ruleRemediations],
];

describe('fr locale — Türkçe sızıntısı', () => {
  it.each(FR_SECTIONS)("%s içinde Türkçe'ye özgü harf yok", (_name, table) => {
    const leaked = Object.entries(table)
      .filter(([, v]) => TR_ONLY.test(v))
      .map(([k, v]) => `${k}: ${v}`);
    expect(leaked).toEqual([]);
  });
});

/** `{placeholder}` kümesi — sıra önemsiz, tekrar önemsiz. */
const holders = (s: string): string[] =>
  [...new Set(s.match(/\{(\w+)\}/g) ?? [])].sort();

describe('fr locale — yer tutucu paritesi', () => {
  it.each([
    ['ui', fr.ui, en.ui],
    ['ruleTitles', fr.ruleTitles, en.ruleTitles],
    ['ruleMessages', fr.ruleMessages, en.ruleMessages],
    ['ruleRemediations', fr.ruleRemediations, en.ruleRemediations],
  ] as const)('%s yer tutucuları en ile aynı', (_name, frTable, enTable) => {
    const drift: string[] = [];
    for (const [key, frText] of Object.entries(frTable)) {
      const enText = enTable[key];
      if (enText === undefined) continue; // orphan key: handled by the ruleTitles guard
      const [a, b] = [holders(frText), holders(enText)];
      if (a.join() !== b.join()) drift.push(`${key}: fr=${a.join()} en=${b.join()}`);
    }
    expect(drift).toEqual([]);
  });
});
