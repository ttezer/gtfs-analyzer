import { readFile } from 'node:fs/promises';

// The root Cargo.toml `[workspace.package] version` is the single source for the engine version.
// Manually maintained copies (internal pins, SDK ENGINE_VERSION, ui/package.json) are checked
// against it here; `node scripts/bump-version.mjs X.Y.Z` writes all of them.
// SDK_VERSION / sdk/package.json is the SDK's OWN version and need not equal the engine version.
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
  throw new Error('Could not read the SDK, engine, or workspace version from the sources.');
}

const errors = [];
if (packageJson.version !== sdkVersion) {
  errors.push(`SDK package version ${packageJson.version}, getVersion SDK version ${sdkVersion}.`);
}
if (engineVersion !== workspaceVersion) {
  errors.push(`getVersion engine version ${engineVersion}, Cargo workspace version ${workspaceVersion}.`);
}

// Every crate must take its version from the workspace; a local version would break the single source.
for (const crate of ['core', 'config', 'rules', 'pipeline', 'cli', 'wasm', 'python']) {
  const manifest = await read(`crates/${crate}/Cargo.toml`);
  if (!/^version\.workspace\s*=\s*true\s*$/m.test(manifest)) {
    errors.push(`crates/${crate}/Cargo.toml does not use the workspace version (version.workspace = true expected).`);
  }
}

// crates.io uses this pin instead of the path; if it diverges from the workspace version,
// the published crate links to an older internal crate.
const pins = [...rootCargo.matchAll(/^(gtfs-[a-z]+)\s*=\s*\{\s*version\s*=\s*"([^"]+)"/gm)];
if (pins.length !== 4) {
  errors.push(`Root Cargo.toml expected 4 internal crate pins, found ${pins.length}.`);
}
for (const [, name, version] of pins) {
  if (version !== workspaceVersion) {
    errors.push(`Internal pin ${name} = ${version}, workspace version ${workspaceVersion}.`);
  }
}

if (!/^dynamic\s*=\s*\[\s*"version"\s*\]/m.test(pyproject) || /^version\s*=/m.test(pyproject)) {
  errors.push('pyproject.toml must get its version from Cargo (dynamic = ["version"], no fixed version).');
}

for (const [label, version] of [
  ['ui/package.json', uiPackage.version],
  ['ui/package-lock.json', uiLock.version],
  ['ui/package-lock.json packages[""]', uiLock.packages?.['']?.version],
]) {
  if (version !== workspaceVersion) {
    errors.push(`${label} version ${version}, Cargo workspace version ${workspaceVersion}.`);
  }
}

if (errors.length) {
  throw new Error(`Version mismatch (to fix: node scripts/bump-version.mjs ${workspaceVersion}):\n- ${errors.join('\n- ')}`);
}

console.log(`version check ok: gtfs-sdk ${sdkVersion}, engine ${engineVersion}`);
