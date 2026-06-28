import { readFileSync } from 'node:fs';
import { dirname, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';

const uiDir = resolve(dirname(fileURLToPath(import.meta.url)), '..');
const fixture = new Uint8Array(readFileSync(resolve(uiDir, 'tests/fixtures/minimal.zip')));
const wasm32 = await import('../pkg/gtfs_wasm.js');
const wasm64 = await import('../pkg64/gtfs_wasm.js');

await wasm32.default({ module_or_path: readFileSync(resolve(uiDir, 'pkg/gtfs_wasm_bg.wasm')) });
await wasm64.default({ module_or_path: readFileSync(resolve(uiDir, 'pkg64/gtfs_wasm_bg.wasm')) });

const result32 = JSON.parse(wasm32.validate(fixture, ''));
const result64 = JSON.parse(wasm64.validate(fixture, ''));

function canonical(value) {
  if (Array.isArray(value)) return value.map(canonical);
  if (value && typeof value === 'object') {
    return Object.fromEntries(Object.keys(value).sort().map(key => [key, canonical(value[key])]));
  }
  return value;
}

if (JSON.stringify(canonical(result32)) !== JSON.stringify(canonical(result64))) {
  console.error('[wasm64:parity] FAIL — wasm32 ve wasm64 sonuçları farklı');
  process.exit(1);
}

console.log(`[wasm64:parity] PASS — ${result64.Ok?.notices?.length ?? 0} notice birebir eşit`);
