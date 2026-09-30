// Native <-> WASM parite kapisi (SDK tarafi).
//
// tests/fixtures/wasm-parity/cases.json vakalarini WASM'da (validateGtfs, oturum validate
// ve rerun) kosar ve expected.json ile kiyaslar. expected.json'i native CLI uretir ve
// crates/cli/tests/wasm_parity.rs ayni dosyaya karsi dogrular; ikisi de eslesirse native
// ve WASM birbirine esittir. 2026-09-30'da 22 feed-level toplulama yalniz native'de
// calisiyordu ve native kosan korpus denetimi bunu goremedi; bu kapi onu yakalar.
//
// Karsilastirma anahtar sirasindan bagimsizdir (kanonik JSON, coklukume): Rust ile JS
// nesne anahtarlarini farkli siralar, ayni icerik ayni sayilmalidir.

import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';

import { createValidatorSession, validateGtfs } from '../dist/index.js';

const fixtureDir = new URL('../../tests/fixtures/wasm-parity/', import.meta.url);
const cases = JSON.parse(readFileSync(new URL('cases.json', fixtureDir), 'utf8'));
const expected = JSON.parse(readFileSync(new URL('expected.json', fixtureDir), 'utf8'));

// crates/cli/tests/wasm_parity.rs FIELDS ile ayni liste.
const FIELDS = [
  'rule_id', 'severity', 'rule_class', 'entity_type', 'entity_id', 'scope_key', 'file',
  'line', 'field', 'observed_value', 'expected_value', 'details', 'service_id', 'title',
  'message', 'remediation',
];

// ── Minimal STORED zip yazici (bagimlilik eklememek icin) ──────────────────────
const CRC_TABLE = Array.from({ length: 256 }, (_, n) => {
  let c = n;
  for (let k = 0; k < 8; k += 1) c = c & 1 ? 0xedb88320 ^ (c >>> 1) : c >>> 1;
  return c >>> 0;
});
function crc32(bytes) {
  let c = 0xffffffff;
  for (const b of bytes) c = CRC_TABLE[(c ^ b) & 0xff] ^ (c >>> 8);
  return (c ^ 0xffffffff) >>> 0;
}
function makeZip(files) {
  const encoder = new TextEncoder();
  const locals = [];
  const centrals = [];
  let offset = 0;
  for (const [name, content] of Object.entries(files)) {
    const nameBytes = encoder.encode(name);
    const data = encoder.encode(content);
    const crc = crc32(data);
    const local = Buffer.alloc(30);
    local.writeUInt32LE(0x04034b50, 0);
    local.writeUInt16LE(20, 4);
    local.writeUInt16LE(0x0800, 6); // UTF-8 adlar
    local.writeUInt32LE(crc, 14);
    local.writeUInt32LE(data.length, 18);
    local.writeUInt32LE(data.length, 22);
    local.writeUInt16LE(nameBytes.length, 26);
    const central = Buffer.alloc(46);
    central.writeUInt32LE(0x02014b50, 0);
    central.writeUInt16LE(20, 4);
    central.writeUInt16LE(20, 6);
    central.writeUInt16LE(0x0800, 8);
    central.writeUInt32LE(crc, 16);
    central.writeUInt32LE(data.length, 20);
    central.writeUInt32LE(data.length, 24);
    central.writeUInt16LE(nameBytes.length, 28);
    central.writeUInt32LE(offset, 42);
    locals.push(local, nameBytes, data);
    centrals.push(central, nameBytes);
    offset += local.length + nameBytes.length + data.length;
  }
  const centralSize = centrals.reduce((sum, part) => sum + part.length, 0);
  const end = Buffer.alloc(22);
  end.writeUInt32LE(0x06054b50, 0);
  end.writeUInt16LE(Object.keys(files).length, 8);
  end.writeUInt16LE(Object.keys(files).length, 10);
  end.writeUInt32LE(centralSize, 12);
  end.writeUInt32LE(offset, 16);
  return new Uint8Array(Buffer.concat([...locals, ...centrals, end]));
}

// ── Kanonik karsilastirma ─────────────────────────────────────────────────────
function canonical(value) {
  if (Array.isArray(value)) return `[${value.map(canonical).join(',')}]`;
  if (value && typeof value === 'object') {
    return `{${Object.keys(value).sort().map((k) => `${JSON.stringify(k)}:${canonical(value[k])}`).join(',')}}`;
  }
  return JSON.stringify(value ?? null);
}
const multiset = (items) => items.map(canonical).sort();

function project(result) {
  return {
    publishable: result.reports.r1.publishable,
    score: result.reports.r5.score,
    notices: result.notices.map((n) => Object.fromEntries(FIELDS.map((f) => [f, n[f] ?? null]))),
  };
}

function diff(caseResult, base) {
  const remaining = base.notices.map(canonical);
  const added = [];
  for (const notice of caseResult.notices) {
    const i = remaining.indexOf(canonical(notice));
    if (i >= 0) remaining.splice(i, 1);
    else added.push(notice);
  }
  return { publishable: caseResult.publishable, score: caseResult.score, added, removed: remaining };
}

function assertSameDiff(actual, want, label) {
  assert.equal(actual.publishable, want.publishable, `${label}: publishable`);
  assert.equal(actual.score, want.score, `${label}: score`);
  assert.deepEqual(multiset(actual.added), multiset(want.added), `${label}: eklenen notice'lar`);
  assert.deepEqual(actual.removed.sort(), multiset(want.removed), `${label}: kaybolan notice'lar`);
}

// Uc SDK yolu: tam dogrulama, oturum (onbellekli) dogrulama, onbellekten rerun.
async function runAllPaths(bytes) {
  const today = cases.today;
  const session = await createValidatorSession({ today });
  try {
    return {
      validateGtfs: project(await validateGtfs(bytes, { today })),
      'session.validate': project((await session.validate(bytes)).result),
      'session.rerun': project((await session.rerun()).result),
    };
  } finally {
    session.dispose();
  }
}

const baseRuns = await runAllPaths(makeZip(cases.base));
for (const [path, base] of Object.entries(baseRuns)) {
  assert.equal(base.publishable, expected.base.publishable, `base/${path}: publishable`);
  assert.equal(base.score, expected.base.score, `base/${path}: score`);
  assert.deepEqual(multiset(base.notices), multiset(expected.base.notices), `base/${path}: notice'lar`);
}

for (const testCase of cases.cases) {
  const runs = await runAllPaths(makeZip({ ...cases.base, ...testCase.files }));
  const want = expected.cases[testCase.name];
  assert.ok(want, `${testCase.name}: expected.json'da yok`);
  for (const [path, projected] of Object.entries(runs)) {
    assertSameDiff(diff(projected, baseRuns[path]), want, `${testCase.name}/${path}`);
  }
}

console.log(`gtfs-sdk WASM parity passed: base + ${cases.cases.length} cases x 3 paths`);
