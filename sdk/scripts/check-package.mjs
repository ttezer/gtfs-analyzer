import { execFileSync } from 'node:child_process';
import { readFile } from 'node:fs/promises';
import process from 'node:process';
import { fileURLToPath } from 'node:url';

const sdkRoot = fileURLToPath(new URL('..', import.meta.url));
const baseline = JSON.parse(await readFile(new URL('../package-size-baseline.json', import.meta.url), 'utf8'));

const npmArgs = ['pack', '--json', '--dry-run', '--ignore-scripts'];
const npmResult = process.env.npm_execpath
  ? execFileSync(process.execPath, [process.env.npm_execpath, ...npmArgs], { cwd: sdkRoot, encoding: 'utf8' })
  : execFileSync('npm', npmArgs, { cwd: sdkRoot, encoding: 'utf8' });
const pack = JSON.parse(npmResult)[0];

const expectedFiles = new Set([
  'LICENSE',
  'README.md',
  'package.json',
  'dist/index.d.ts',
  'dist/index.js',
  'dist/types.d.ts',
  'dist/types.js',
  'pkg/gtfs_wasm.js',
  'pkg/gtfs_wasm_bg.wasm',
]);
const actualFiles = pack.files.map(({ path }) => path).sort();
const unexpectedFiles = actualFiles.filter((path) => !expectedFiles.has(path));
const missingFiles = [...expectedFiles].filter((path) => !actualFiles.includes(path));
const failures = [];

if (unexpectedFiles.length > 0) {
  failures.push(`unexpected files: ${unexpectedFiles.join(', ')}`);
}
if (missingFiles.length > 0) {
  failures.push(`missing expected files: ${missingFiles.join(', ')}`);
}
if (pack.unpackedSize > baseline.maxUnpackedBytes) {
  failures.push(`unpacked size ${pack.unpackedSize} exceeds ${baseline.maxUnpackedBytes}`);
}
if (pack.size > baseline.maxPackedBytes) {
  failures.push(`packed size ${pack.size} exceeds ${baseline.maxPackedBytes}`);
}

if (failures.length > 0) {
  console.error('gtfs-sdk package gate failed:');
  for (const failure of failures) console.error(`- ${failure}`);
  process.exit(1);
}

console.log(
  `gtfs-sdk package gate passed: ${actualFiles.length} files, `
  + `${pack.unpackedSize} bytes unpacked, ${pack.size} bytes packed`,
);
