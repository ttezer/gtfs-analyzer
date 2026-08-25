import assert from 'node:assert/strict';
import { readFile } from 'node:fs/promises';

const rustSource = await readFile(new URL('../../crates/config/src/lib.rs', import.meta.url), 'utf8');
const typesSource = await readFile(new URL('../src/types.ts', import.meta.url), 'utf8');

const rustBody = rustSource.match(
  /pub const KNOWN_CONFIG_KEYS: &\[&str\] = &\[(?<body>[\s\S]*?)\];/,
)?.groups?.body;
const typesBody = typesSource.match(
  /export interface ValidatorConfigDelta \{(?<body>[\s\S]*?)\n\}/,
)?.groups?.body;

assert.ok(rustBody, 'Rust KNOWN_CONFIG_KEYS listesi bulunamadı.');
assert.ok(typesBody, 'SDK ValidatorConfigDelta interface\'i bulunamadı.');

const rustKeys = [...rustBody.matchAll(/"([a-z][a-z0-9_]*)"/g)].map(([, key]) => key);
const sdkKeys = [...typesBody.matchAll(/^\s*([a-z][a-z0-9_]*)\??:/gm)].map(([, key]) => key);

function assertUnique(label, keys) {
  assert.equal(new Set(keys).size, keys.length, `${label} listesinde duplicate config anahtarı var.`);
}

function difference(left, right) {
  const rightSet = new Set(right);
  return [...new Set(left)].filter((key) => !rightSet.has(key)).sort();
}

assertUnique('Rust', rustKeys);
assertUnique('SDK', sdkKeys);
assert.deepEqual(difference(rustKeys, sdkKeys), [], 'Rust listesinde SDK tarafından typed edilmeyen anahtarlar var.');
assert.deepEqual(difference(sdkKeys, rustKeys), [], 'SDK interface\'inde Rust tarafından kabul edilmeyen anahtarlar var.');
assert.equal(rustKeys.length, 37, 'Beklenen Rust config anahtarı sayısı değişti; parity listesini gözden geçirin.');
assert.equal(sdkKeys.length, rustKeys.length, 'Rust ve SDK config anahtar sayıları eşit değil.');

console.log(`config parity ok: ${rustKeys.length} Rust/SDK anahtarı`);
