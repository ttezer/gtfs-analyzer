import assert from 'node:assert/strict';
import { readFile } from 'node:fs/promises';
import { readFileSync } from 'node:fs';
import { fileURLToPath } from 'node:url';

import {
  createValidatorSession,
  getVersion,
  validateGtfs,
  ValidationError,
} from '../dist/index.js';

const fixtureUrl = new URL('../../ui/tests/fixtures/minimal.zip', import.meta.url);
const fixture = new Uint8Array(await readFile(fileURLToPath(fixtureUrl)));

// Surum bilgisi package.json'dan okunur: bump sirasinda BES ayri yerde elle guncellenmesi
// gerekiyordu (package.json · SDK_VERSION · ENGINE_VERSION · README ornegi · BU SATIR) ve
// 0.12.0'da ucu unutuldu, CI iki kez ust uste kirmizi yandi. Testin iddiasi degismedi:
// getVersion() paketin ilan ettigi surumu ve gomulu motorun surumunu dogru bildirmeli.
const pkg = JSON.parse(readFileSync(new URL('../package.json', import.meta.url), 'utf8'));
const engineVersion = JSON.parse(
  readFileSync(new URL('../pkg/package.json', import.meta.url), 'utf8'),
).version;
assert.deepEqual(getVersion(), { sdk: pkg.version, engine: engineVersion });

const result = await validateGtfs(fixture, { today: '2026-08-20' });
assert.equal(result.validation_status, 'COMPLETE');
assert.ok(result.notices.length > 0);
assert.equal(typeof result.reports.r5.score, 'number');
for (const notice of result.notices) {
  assert.match(notice.title, /^[\x00-\x7F]*$/, `notice title is not English: ${notice.rule_id}`);
  assert.match(notice.message, /^[\x00-\x7F]*$/, `notice message is not English: ${notice.rule_id}`);
  assert.match(notice.remediation, /^[\x00-\x7F]*$/, `notice remediation is not English: ${notice.rule_id}`);
}

await assert.rejects(
  () => validateGtfs(new Uint8Array([1, 2, 3]), { today: '2026-08-20' }),
  (error) => error instanceof ValidationError
    && error.code === 'ZipUnreadable'
    && error.message === 'Could not read the GTFS ZIP archive.'
    && error.detail?.includes('Could not find EOCD'),
);

await assert.rejects(
  () => validateGtfs(fixture, { today: '2026-02-30' }),
  (error) => error instanceof ValidationError
    && error.code === 'InvalidInput'
    && error.message === 'Invalid SDK input or configuration.'
    && error.detail?.includes('Invalid today value'),
);

const circularConfig = {};
circularConfig.self = circularConfig;
await assert.rejects(
  () => validateGtfs(fixture, { config: circularConfig }),
  (error) => error instanceof ValidationError
    && error.code === 'InvalidInput'
    && error.message === 'Invalid SDK input or configuration.'
    && error.detail?.includes('circular'),
);

const stages = [];
const session = await createValidatorSession({ today: '2026-08-20' });
const sessionRun = await session.validate(fixture, {
  callbacks: { onStageDone: (stage) => stages.push(stage) },
});
assert.ok(sessionRun.files.length > 0);
assert.ok(sessionRun.fileStats.length > 0);
assert.deepEqual(stages, ['K1', 'K2', 'K3', 'K4', 'K5', 'K6', 'K7']);

const rerun = await session.rerun();
assert.equal(rerun.files.length, sessionRun.files.length);
assert.equal(rerun.result.reports.r5.score, sessionRun.result.reports.r5.score);
assert.deepEqual(session.getShapeCoords('missing-shape'), []);
session.dispose();

console.log(
  `gtfs-sdk smoke test passed: ${result.notices.length} notices, score ${result.reports.r5.score}, session rerun ok`,
);
