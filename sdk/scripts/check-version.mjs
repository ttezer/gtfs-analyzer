import { readFile } from 'node:fs/promises';

// Motor sürümünün tek kaynağı kök Cargo.toml `[workspace.package] version`.
// Elle tutulan kopyalar (iç pinler, SDK ENGINE_VERSION, ui/package.json) burada ona
// karşı denetlenir; hepsini `node scripts/bump-version.mjs X.Y.Z` yazar.
// SDK_VERSION / sdk/package.json SDK'nın KENDİ sürümüdür, motorla eşit olmak zorunda değil.
const read = (path) => readFile(new URL(`../../${path}`, import.meta.url), 'utf8');

const packageJson = JSON.parse(await read('sdk/package.json'));
const indexSource = await read('sdk/src/index.ts');
const rootCargo = await read('Cargo.toml');
const pyproject = await read('pyproject.toml');
const uiPackage = JSON.parse(await read('ui/package.json'));
const uiLock = JSON.parse(await read('ui/package-lock.json'));

const sdkVersion = indexSource.match(/const SDK_VERSION = '([^']+)'/)?.[1];
const engineVersion = indexSource.match(/const ENGINE_VERSION = '([^']+)'/)?.[1];
const workspaceVersion = rootCargo.match(/^\[workspace\.package\][^[]*?^version\s*=\s*"([^"]+)"/ms)?.[1];

if (!sdkVersion || !engineVersion || !workspaceVersion) {
  throw new Error('SDK, engine veya workspace sürümü kaynaklardan okunamadı.');
}

const errors = [];
if (packageJson.version !== sdkVersion) {
  errors.push(`SDK package sürümü ${packageJson.version}, getVersion SDK sürümü ${sdkVersion}.`);
}
if (engineVersion !== workspaceVersion) {
  errors.push(`getVersion engine sürümü ${engineVersion}, Cargo workspace sürümü ${workspaceVersion}.`);
}

// Her crate sürümü workspace'ten almalı; biri kendi sürümünü yazarsa tek kaynak bozulur.
for (const crate of ['core', 'config', 'rules', 'pipeline', 'cli', 'wasm', 'python']) {
  const manifest = await read(`crates/${crate}/Cargo.toml`);
  if (!/^version\.workspace\s*=\s*true\s*$/m.test(manifest)) {
    errors.push(`crates/${crate}/Cargo.toml sürümü workspace'ten almıyor (version.workspace = true bekleniyor).`);
  }
}

// crates.io'da yayında path yerine bu pin kullanılır; workspace sürümünden ayrışırsa
// yayınlanan crate eski iç crate'e bağlanır.
const pins = [...rootCargo.matchAll(/^(gtfs-[a-z]+)\s*=\s*\{\s*version\s*=\s*"([^"]+)"/gm)];
if (pins.length !== 4) {
  errors.push(`Kök Cargo.toml'da 4 iç crate pini bekleniyordu, ${pins.length} bulundu.`);
}
for (const [, name, version] of pins) {
  if (version !== workspaceVersion) {
    errors.push(`İç pin ${name} = ${version}, workspace sürümü ${workspaceVersion}.`);
  }
}

if (!/^dynamic\s*=\s*\[\s*"version"\s*\]/m.test(pyproject) || /^version\s*=/m.test(pyproject)) {
  errors.push('pyproject.toml sürümü Cargo\'dan almalı (dynamic = ["version"], sabit version yok).');
}

for (const [label, version] of [
  ['ui/package.json', uiPackage.version],
  ['ui/package-lock.json', uiLock.version],
  ['ui/package-lock.json packages[""]', uiLock.packages?.['']?.version],
]) {
  if (version !== workspaceVersion) {
    errors.push(`${label} sürümü ${version}, Cargo workspace sürümü ${workspaceVersion}.`);
  }
}

if (errors.length) {
  throw new Error(`Sürüm uyuşmazlığı (düzeltmek için: node scripts/bump-version.mjs ${workspaceVersion}):\n- ${errors.join('\n- ')}`);
}

console.log(`version check ok: gtfs-sdk ${sdkVersion}, engine ${engineVersion}`);
