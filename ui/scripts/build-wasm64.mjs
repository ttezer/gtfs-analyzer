// Deneysel serial Memory64 build'i. wasm64 Rust'ta Tier 3 olduğu için nightly
// build-std gerekir; JS binding'i wasm-pack yerine doğrudan wasm-bindgen üretir.
import { rmSync } from 'node:fs';
import { dirname, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';
import { spawnSync } from 'node:child_process';

const uiDir = resolve(dirname(fileURLToPath(import.meta.url)), '..');
const repoDir = resolve(uiDir, '..');
const wasmPath = resolve(repoDir, 'target/wasm64-unknown-unknown/release/gtfs_wasm.wasm');
const outDir = resolve(uiDir, 'pkg64');

function run(command, args, cwd = repoDir) {
  console.log(`[wasm64] ${command} ${args.join(' ')}`);
  const result = spawnSync(command, args, { cwd, stdio: 'inherit' });
  if (result.error) throw result.error;
  if (result.status !== 0) process.exit(result.status ?? 1);
}

run('cargo', [
  '+nightly', 'build',
  '-Z', 'build-std=std,panic_abort',
  '--target', 'wasm64-unknown-unknown',
  '-p', 'gtfs-wasm',
  '--release',
]);

rmSync(outDir, { recursive: true, force: true });
run('wasm-bindgen', [wasmPath, '--target', 'web', '--out-dir', outDir]);
console.log(`[wasm64] hazır: ${outDir}`);
