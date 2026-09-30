#!/usr/bin/env node
// Motor sürümünü tek komutla yükseltir:  node scripts/bump-version.mjs X.Y.Z
//
// Tek kaynak kök Cargo.toml `[workspace.package] version`; yedi crate ve Python paketi
// (maturin, dynamic version) oradan okur. Bu betik yalnız ELLE tutulmak zorunda kalan
// kopyaları yazar ve sonunda sdk/scripts/check-version.mjs ile hepsini doğrular:
//   - kök Cargo.toml: workspace sürümü + crates.io için dört iç crate pini
//   - sdk/src/index.ts: ENGINE_VERSION
//   - ui/package.json + ui/package-lock.json (arayüzdeki __APP_VERSION__)
//   - Cargo.lock (cargo update --workspace)
//
// DOKUNMADIKLARI (bilinçli): SDK'nın kendi sürümü (sdk/package.json, SDK_VERSION),
// CHANGELOG ve belgeler, tag. Tag CI yeşil olduktan SONRA atılır.
import { execFileSync } from 'node:child_process';
import { readFileSync, writeFileSync } from 'node:fs';
import { dirname, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';

const root = resolve(dirname(fileURLToPath(import.meta.url)), '..');
const next = process.argv[2];
if (!/^\d+\.\d+\.\d+$/.test(next ?? '')) {
  console.error('kullanım: node scripts/bump-version.mjs X.Y.Z');
  process.exit(2);
}

const path = (p) => resolve(root, p);
const cargoToml = readFileSync(path('Cargo.toml'), 'utf8');
const current = cargoToml.match(/^\[workspace\.package\][^[]*?^version\s*=\s*"([^"]+)"/ms)?.[1];
if (!current) {
  console.error('Kök Cargo.toml içinde [workspace.package] version bulunamadı.');
  process.exit(1);
}
if (current === next) {
  console.error(`Sürüm zaten ${next}.`);
  process.exit(1);
}

// Her değiştirme TAM beklenen sayıda eşleşmeli; sıfır ya da fazla eşleşme sessiz
// yarım bump demektir, o yüzden dosya yazılmadan önce durulur.
function replaceExact(file, pattern, replacement, expected) {
  const text = readFileSync(path(file), 'utf8');
  const hits = text.match(new RegExp(pattern.source, pattern.flags.includes('g') ? pattern.flags : `${pattern.flags}g`))?.length ?? 0;
  if (hits !== expected) {
    throw new Error(`${file}: ${pattern} için ${expected} eşleşme bekleniyordu, ${hits} bulundu.`);
  }
  return text.replace(pattern, replacement);
}

const esc = current.replaceAll('.', '\\.');

function setJsonVersion(file, targets) {
  const json = JSON.parse(readFileSync(path(file), 'utf8'));
  for (const node of targets(json)) {
    if (node?.version !== current) {
      throw new Error(`${file}: sürüm ${node?.version}, beklenen ${current}.`);
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
  // Lock'ta kök paket sürümü iki yerde durur: dosya başı ve packages[""]. Regex yerine
  // JSON: aynı sürüm numarasını taşıyan bir bağımlılık yanlışlıkla değişmesin. npm bu
  // dosyaları JSON.stringify(_, null, 2) + "\n" biçiminde yazar; biçim korunur.
  ['ui/package-lock.json', setJsonVersion('ui/package-lock.json', (j) => [j, j.packages?.['']])],
];

for (const [file, text] of edits) writeFileSync(path(file), text);
console.log(`[bump] ${current} → ${next}: ${edits.map(([f]) => f).join(', ')}`);

execFileSync('cargo', ['update', '--workspace', '--offline'], { cwd: root, stdio: 'inherit' });
execFileSync(process.execPath, ['sdk/scripts/check-version.mjs'], { cwd: root, stdio: 'inherit' });

console.log(`
[bump] Tamam. Kalan elle adımlar:
  1. CHANGELOG.md'ye ${next} bölümünü yaz.
  2. SDK değiştiyse sdk/package.json + SDK_VERSION'ı ayrıca yükselt.
  3. Commit + push; CI yeşil olduktan SONRA v${next} tag'ini at.`);
