// Exports the rule dictionaries from the TypeScript locales into JSON the Rust
// CLI can embed (`crates/cli/locales/`).
//
// The locales stay the single source of truth: this script only derives from
// them, and `src/__tests__/locale-export-drift.test.ts` fails the build if the
// derived JSON drifts. Run `npm run locales:export` after touching en.ts/ja.ts.
//
// TR is not exported: Turkish message/remediation text is what the Rust
// pipeline emits natively, so the CLI already has it.

import { writeFileSync, readFileSync, existsSync, mkdirSync } from 'node:fs';
import { fileURLToPath } from 'node:url';

const OUT_DIR = fileURLToPath(new URL('../../crates/cli/locales/', import.meta.url));
const LANGS = ['en', 'ja', 'fr'];

/** Shape embedded by the CLI. Keep in sync with `crates/cli/src/i18n.rs`. */
function payloadOf(locale) {
  return {
    messages: locale.ruleMessages ?? {},
    remediations: locale.ruleRemediations ?? {},
    titles: locale.ruleTitles ?? {},
  };
}

export function serialize(payload) {
  return JSON.stringify(payload, null, 2) + '\n';
}

export async function loadPayload(lang) {
  const mod = await import(`../src/locales/${lang}.ts`);
  return payloadOf(mod.default);
}

const check = process.argv.includes('--check');
let drifted = false;

mkdirSync(OUT_DIR, { recursive: true });

for (const lang of LANGS) {
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
  console.log(`wrote crates/cli/locales/${lang}.json (${counts})`);
}

if (drifted) process.exit(1);
