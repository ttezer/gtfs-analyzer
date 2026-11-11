// Exports the rule dictionaries from the TypeScript locales into JSON the Rust
// CLI and Python package embed (`crates/core/locales/`, `gtfs_core::i18n`).
//
// The locales stay the single source of truth: this script only derives from
// them, and `src/__tests__/locale-export-drift.test.ts` fails the build if the
// derived JSON drifts. Run `npm run locales:export` after touching any locale.
//
// TR exports its rule titles and planner cards: Turkish message/remediation text
// is still what the Rust pipeline emits natively (issue #201 moves it step by
// step). ES is a cards-only locale whose other texts fall back to English.

import { writeFileSync, readFileSync, existsSync, mkdirSync } from 'node:fs';
import { fileURLToPath } from 'node:url';

const OUT_DIR = fileURLToPath(new URL('../../crates/core/locales/', import.meta.url));
const LANGS = ['en', 'ja', 'fr'];
const CARD_ONLY_LANGS = ['tr', 'es'];
const CARD_KEY = /^producer\.card\.([A-Z]+_\d+[a-z]*)\.(title|what|action)$/;

/** `producer.card.<RULE>.{title,what,action}` keys → `{ RULE: { title, what, action } }`. */
export function cardsOf(locale) {
  const cards = {};
  for (const [key, text] of Object.entries(locale.ui ?? {})) {
    const match = key.match(CARD_KEY);
    if (!match) continue;
    const [, ruleId, field] = match;
    cards[ruleId] ??= {};
    cards[ruleId][field] = text;
  }
  return Object.fromEntries(Object.keys(cards).sort().map((ruleId) => [ruleId, cards[ruleId]]));
}

/** Shape embedded by the CLI. Keep in sync with `crates/core/src/i18n.rs`. */
function payloadOf(locale, cardsOnly, lang) {
  if (lang === 'tr') return { titles: locale.ruleTitles ?? {}, cards: cardsOf(locale) };
  if (cardsOnly) return { cards: cardsOf(locale) };
  return {
    messages: locale.ruleMessages ?? {},
    remediations: locale.ruleRemediations ?? {},
    titles: locale.ruleTitles ?? {},
    fatal_messages: locale.fatalMessages ?? {},
    cards: cardsOf(locale),
  };
}

export function serialize(payload) {
  return JSON.stringify(payload, null, 2) + '\n';
}

export async function loadPayload(lang) {
  const mod = await import(`../src/locales/${lang}.ts`);
  return payloadOf(mod.default, CARD_ONLY_LANGS.includes(lang), lang);
}

const check = process.argv.includes('--check');
let drifted = false;

mkdirSync(OUT_DIR, { recursive: true });

for (const lang of [...LANGS, ...CARD_ONLY_LANGS]) {
  const path = `${OUT_DIR}${lang}.json`;
  const next = serialize(await loadPayload(lang));

  if (check) {
    const current = existsSync(path) ? readFileSync(path, 'utf8') : '';
    if (current !== next) {
      console.error(`drift: ${lang}.json is stale — run \`npm run locales:export\``);
      drifted = true;
    }
    continue;
  }

  writeFileSync(path, next);
  const counts = Object.entries(JSON.parse(next))
    .map(([k, v]) => `${k}=${Object.keys(v).length}`)
    .join(' ');
  console.log(`wrote crates/core/locales/${lang}.json (${counts})`);
}

if (drifted) process.exit(1);
