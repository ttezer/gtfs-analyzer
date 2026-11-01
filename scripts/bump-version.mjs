#!/usr/bin/env node
// Bump the engine version with one command: node scripts/bump-version.mjs X.Y.Z
//
// The single source is the root Cargo.toml `[workspace.package] version`; seven crates and the Python package
// (maturin, dynamic version) read it from there. This script writes only manually maintained copies and verifies
// all of them with sdk/scripts/check-version.mjs:
//   - root Cargo.toml: workspace version + four internal crate pins for crates.io
//   - sdk/src/index.ts: ENGINE_VERSION
//   - ui/package.json + ui/package-lock.json (the UI's __APP_VERSION__)
//   - READMEs (four languages) and version examples in sdk/README.md
//   - Cargo.lock (cargo update --workspace)
//
// INTENTIONALLY UNCHANGED: the SDK's own version (sdk/package.json, SDK_VERSION),
// CHANGELOG and documentation, and the tag. Create the tag only AFTER CI is green.
import { execFileSync } from 'node:child_process';
import { readFileSync, writeFileSync } from 'node:fs';
import { dirname, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';

const root = resolve(dirname(fileURLToPath(import.meta.url)), '..');
const next = process.argv[2];
if (!/^\d+\.\d+\.\d+$/.test(next ?? '')) {
  console.error('usage: node scripts/bump-version.mjs X.Y.Z');
  process.exit(2);
}

const path = (p) => resolve(root, p);
const cargoToml = readFileSync(path('Cargo.toml'), 'utf8');
const current = cargoToml.match(/^\[workspace\.package\][^[]*?^version\s*=\s*"([^"]+)"/ms)?.[1];
if (!current) {
  console.error('The root Cargo.toml has no [workspace.package] version.');
  process.exit(1);
}
if (current === next) {
  console.error(`Version is already ${next}.`);
  process.exit(1);
}

// Every replacement must match the EXACT expected count; zero or extra matches indicate
// a silently partial bump, so stop before writing any file.
function replaceExact(file, pattern, replacement, expected) {
  const text = readFileSync(path(file), 'utf8');
  const hits = text.match(new RegExp(pattern.source, pattern.flags.includes('g') ? pattern.flags : `${pattern.flags}g`))?.length ?? 0;
  if (hits !== expected) {
    throw new Error(`${file}: expected ${expected} matches for ${pattern}, found ${hits}.`);
  }
  return text.replace(pattern, replacement);
}

const esc = current.replaceAll('.', '\\.');

function setJsonVersion(file, targets) {
  const json = JSON.parse(readFileSync(path(file), 'utf8'));
  for (const node of targets(json)) {
    if (node?.version !== current) {
      throw new Error(`${file}: version is ${node?.version}, expected ${current}.`);
    }
    node.version = next;
  }
  return `${JSON.stringify(json, null, 2)}\n`;
}
const edits = [
  ['Cargo.toml', replaceExact('Cargo.toml',
    new RegExp(`^(version\\s*=\\s*")${esc}(")|^(gtfs-[a-z]+\\s*=\\s*\\{\\s*version\\s*=\\s*")${esc}(")`, 'gm'),
    (_, a, b, c, d) => (a ? `${a}${next}${b}` : `${c}${next}${d}`), 5)],
  ['sdk/src/index.ts', replaceExact('sdk/src/index.ts',
    new RegExp(`(const ENGINE_VERSION = ')${esc}(')`), `$1${next}$2`, 1)],
  ['ui/package.json', setJsonVersion('ui/package.json', (j) => [j])],
  // The root package version appears twice in the lockfile: at the top and in packages[""].
  // Use JSON rather than a regex so a dependency with the same version is not changed by mistake.
  // npm writes these files as JSON.stringify(_, null, 2) + "\n"; preserve that formatting.
  ['ui/package-lock.json', setJsonVersion('ui/package-lock.json', (j) => [j, j.packages?.['']])],
  // Installation examples in the READMEs (three crate lines) and the SDK introduction's `vX.Y.Z`.
  ...['README.md', 'README.en.md', 'README.fr.md', 'README.ja.md'].map((file) => [file, replaceExact(file,
    new RegExp(`^(gtfs-(?:pipeline|config|core)\\s*=\\s*")${esc}(")|v${esc}(?=\\D)`, 'gm'),
    (m, a, b) => (a ? `${a}${next}${b}` : `v${next}`), 4)]),
  ['sdk/README.md', replaceExact('sdk/README.md',
    new RegExp(`(\`)${esc}(\` validator engine)|(engine: ')${esc}(')`, 'g'),
    (m, a, b, c, d) => (a ? `${a}${next}${b}` : `${c}${next}${d}`), 2)],
];

for (const [file, text] of edits) writeFileSync(path(file), text);
console.log(`[bump] ${current} → ${next}: ${edits.map(([f]) => f).join(', ')}`);

execFileSync('cargo', ['update', '--workspace', '--offline'], { cwd: root, stdio: 'inherit' });
execFileSync(process.execPath, ['sdk/scripts/check-version.mjs'], { cwd: root, stdio: 'inherit' });

console.log(`
[bump] Done. Remaining manual steps:
  1. Add a ${next} section to CHANGELOG.md.
  2. If the SDK changed, update sdk/package.json + SDK_VERSION separately.
  3. Commit + push; create the v${next} tag only AFTER CI is green.`);
