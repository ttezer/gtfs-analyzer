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

assert.ok(rustBody, 'Rust KNOWN_CONFIG_KEYS list was not found.');
assert.ok(typesBody, 'SDK ValidatorConfigDelta interface was not found.');

const rustKeys = [...rustBody.matchAll(/"([a-z][a-z0-9_]*)"/g)].map(([, key]) => key);
const sdkKeys = [...typesBody.matchAll(/^\s*([a-z][a-z0-9_]*)\??:/gm)].map(([, key]) => key);

function assertUnique(label, keys) {
  assert.equal(new Set(keys).size, keys.length, `${label} list contains duplicate config keys.`);
}

function difference(left, right) {
  const rightSet = new Set(right);
  return [...new Set(left)].filter((key) => !rightSet.has(key)).sort();
}

assertUnique('Rust', rustKeys);
assertUnique('SDK', sdkKeys);
assert.deepEqual(difference(rustKeys, sdkKeys), [], 'Rust list contains keys not typed by the SDK.');
assert.deepEqual(difference(sdkKeys, rustKeys), [], 'SDK interface contains keys not accepted by Rust.');
assert.equal(rustKeys.length, 39, 'Expected Rust config key count changed; review the parity lists.');
assert.equal(sdkKeys.length, rustKeys.length, 'Rust and SDK config key counts differ.');

console.log(`config parity OK: ${rustKeys.length} Rust/SDK keys`);
