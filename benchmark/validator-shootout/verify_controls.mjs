import fs from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import { GtfsValidator } from '@tmlmobilidade/gtfs-validator';

const HERE = path.dirname(fileURLToPath(import.meta.url));
const FIXTURES = path.join(HERE, 'fixtures');
const OUT = path.join(HERE, 'results');

const CASES = [
  ['control_valid_location_type', 'valid location_type values'],
  ['invalid_location_type', 'invalid location_type=9'],
  ['control_valid_frequency', 'valid frequencies.txt with headway_secs=600'],
  ['frequency_headway_zero', 'same frequencies.txt shape with headway_secs=0'],
];

function serialError(error) {
  return {
    name: error?.name ?? null,
    message: error?.message ?? String(error),
    code: error?.code ?? null,
    stderr: error?.stderr ?? null,
  };
}

const results = [];
for (const [id, description] of CASES) {
  const feed = path.join(FIXTURES, `${id}.zip`);
  try {
    const result = await GtfsValidator(feed, { lang: 'en', timeout: 120_000 });
    results.push({ id, description, status: 'OK', summary: result?.summary ?? null, error: null });
  } catch (error) {
    const e = serialError(error);
    const crash = /panic:|runtime error|index out of range|invalid memory address|reflect:/i.test(`${e.message}\n${e.stderr ?? ''}`);
    results.push({ id, description, status: crash ? 'CRASH' : 'ERROR', summary: null, error: e });
  }
}

fs.writeFileSync(path.join(OUT, 'crash-controls.json'), JSON.stringify(results, null, 2) + '\n');
console.log('\nCrash-attribution controls:');
for (const r of results) console.log(`${r.id}: ${r.status}`);
