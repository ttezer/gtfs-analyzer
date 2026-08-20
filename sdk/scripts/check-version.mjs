import { readFile } from 'node:fs/promises';

const packageJson = JSON.parse(await readFile(new URL('../package.json', import.meta.url), 'utf8'));
const indexSource = await readFile(new URL('../src/index.ts', import.meta.url), 'utf8');
const cargoSource = await readFile(new URL('../../crates/wasm/Cargo.toml', import.meta.url), 'utf8');

const sdkVersion = indexSource.match(/const SDK_VERSION = '([^']+)'/)?.[1];
const engineVersion = indexSource.match(/const ENGINE_VERSION = '([^']+)'/)?.[1];
const cargoVersion = cargoSource.match(/^version\s*=\s*"([^"]+)"/m)?.[1];

if (!sdkVersion || !engineVersion || !cargoVersion) {
  throw new Error('SDK veya engine sürümü kaynaklardan okunamadı.');
}
if (packageJson.version !== sdkVersion) {
  throw new Error(`SDK package sürümü ${packageJson.version}, getVersion SDK sürümü ${sdkVersion}.`);
}
if (engineVersion !== cargoVersion) {
  throw new Error(`getVersion engine sürümü ${engineVersion}, crates/wasm sürümü ${cargoVersion}.`);
}

console.log(`version check ok: gtfs-sdk ${sdkVersion}, engine ${engineVersion}`);
