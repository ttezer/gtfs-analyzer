#!/usr/bin/env node
// Write the rule lists into the Claude plugin as a few small files per language:
//   node scripts/split-plugin-rules.mjs <plugin-repo> [git-ref]
//
// The plugin directory's policy review skips files that are too large or too many, so each
// ~50 KB RULES*.md file is packed, whole groups at a time, into a few files of at most
// MAX_BYTES: skills/gtfs/references/<lang>/part-<n>.md. With a git ref
// (for example v0.22.0) the lists are read from that tag; otherwise from the working tree.
// Every rule row of the source must be written under its own group, or the script exits 1.
import { execFileSync } from 'node:child_process';
import { existsSync, mkdirSync, readFileSync, rmSync, writeFileSync } from 'node:fs';
import { dirname, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';

const root = resolve(dirname(fileURLToPath(import.meta.url)), '..');
const [pluginDir, ref] = process.argv.slice(2);
if (!pluginDir) {
  console.error('usage: node scripts/split-plugin-rules.mjs <plugin-repo> [git-ref]');
  process.exit(2);
}
const references = resolve(pluginDir, 'skills/gtfs/references');
if (!existsSync(resolve(pluginDir, '.claude-plugin/plugin.json'))) {
  console.error(`not a plugin repository: ${pluginDir}`);
  process.exit(2);
}

const SOURCES = { en: 'RULES.en.md', tr: 'RULES.md', fr: 'RULES.fr.md', ja: 'RULES.ja.md' };
const RULE_ROW = /^\| ([A-Z]+)_\d+ \|/;
const MAX_BYTES = 12_000;

function read(file) {
  return ref
    ? execFileSync('git', ['show', `${ref}:${file}`], { cwd: root, encoding: 'utf8' })
    : readFileSync(resolve(root, file), 'utf8');
}

let failed = false;
for (const [lang, file] of Object.entries(SOURCES)) {
  const lines = read(file).split('\n');
  const sourceRows = lines.filter((l) => RULE_ROW.test(l)).length;

  const groups = new Map();
  let current = null;
  for (const line of lines) {
    const heading = line.match(/^## ([A-Z]+)\b/);
    if (heading) {
      current = heading[1];
      if (groups.has(current)) throw new Error(`${file}: group ${current} appears twice`);
      groups.set(current, []);
    }
    if (current) groups.get(current).push(line);
  }

  let written = 0;
  for (const [group, body] of groups) {
    for (const line of body) {
      const m = line.match(RULE_ROW);
      if (!m) continue;
      if (m[1] !== group) {
        console.error(`${file}: ${line.split('|')[1].trim()} is under ## ${group}`);
        failed = true;
      }
      written += 1;
    }
  }
  if (written !== sourceRows) {
    console.error(`${file}: ${sourceRows} rule rows, ${written} written`);
    failed = true;
  }
  if (failed) continue;

  const dir = resolve(references, lang);
  rmSync(dir, { recursive: true, force: true });
  mkdirSync(dir, { recursive: true });
  const parts = [];
  for (const body of groups.values()) {
    const text = body.join('\n').replace(/\n+---\s*\n*$/, '').trimEnd() + '\n';
    const last = parts.at(-1);
    if (last && Buffer.byteLength(last + '\n' + text) <= MAX_BYTES) parts[parts.length - 1] = last + '\n' + text;
    else parts.push(text);
  }
  parts.forEach((text, i) => writeFileSync(resolve(dir, `part-${i + 1}.md`), text));
  rmSync(resolve(references, `rules.${lang}.md`), { force: true });
  console.log(`${lang}: ${groups.size} groups, ${written} rules, ${parts.length} files -> ${dir}`);
}
process.exit(failed ? 1 : 0);
