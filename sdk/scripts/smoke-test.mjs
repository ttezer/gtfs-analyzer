import assert from 'node:assert/strict';
import { readFile } from 'node:fs/promises';
import { fileURLToPath } from 'node:url';

import {
  createValidatorSession,
  getVersion,
  validateGtfs,
  ValidationError,
} from '../dist/index.js';

const fixtureUrl = new URL('../../ui/tests/fixtures/minimal.zip', import.meta.url);
const fixture = new Uint8Array(await readFile(fileURLToPath(fixtureUrl)));

assert.deepEqual(getVersion(), { sdk: '0.1.0', engine: '0.9.7' });

const result = await validateGtfs(fixture, { today: '2026-08-20' });
assert.equal(result.validation_status, 'COMPLETE');
assert.ok(result.notices.length > 0);
assert.equal(typeof result.reports.r5.score, 'number');

await assert.rejects(
  () => validateGtfs(fixture, { today: '2026-02-30' }),
  (error) => error instanceof ValidationError && error.code === 'InvalidInput',
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
