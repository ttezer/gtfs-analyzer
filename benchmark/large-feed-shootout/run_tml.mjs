import fs from 'node:fs';
import path from 'node:path';
import { GtfsValidator, getValidatorInfo } from '@tmlmobilidade/gtfs-validator';

const [feed, outDir] = process.argv.slice(2);
if (!feed || !outDir) {
  console.error('usage: node run_tml.mjs <feed.zip> <out-dir>');
  process.exit(64);
}

fs.mkdirSync(outDir, { recursive: true });
const report = path.join(outDir, 'report.json');
const wrapper = path.join(outDir, 'wrapper.json');
const errorFile = path.join(outDir, 'error.json');

try {
  const info = await getValidatorInfo();
  fs.writeFileSync(path.join(outDir, 'validator-info.json'), JSON.stringify(info, null, 2) + '\n');
  const result = await GtfsValidator(feed, {
    lang: 'en',
    timeout: 1_100_000,
    out_file: report,
  });
  fs.writeFileSync(wrapper, JSON.stringify(result, (_k, v) => typeof v === 'bigint' ? v.toString() : v, 2) + '\n');
  process.exit(0);
} catch (error) {
  const serial = {
    name: error?.name ?? null,
    message: error?.message ?? String(error),
    code: error?.code ?? null,
    stdout: error?.stdout ?? null,
    stderr: error?.stderr ?? null,
    stack: error?.stack ?? null,
  };
  fs.writeFileSync(errorFile, JSON.stringify(serial, null, 2) + '\n');
  console.error(serial.message);
  if (serial.stderr) console.error(serial.stderr);
  process.exit(70);
}
