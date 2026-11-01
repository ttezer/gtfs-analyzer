// Experimental serial Memory64 build. Because wasm64 is Tier 3 in Rust, nightly
// build-std is required; wasm-bindgen generates the JS binding directly instead of wasm-pack.
import { rmSync, renameSync, existsSync } from 'node:fs';
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

// `RUST_NIGHTLY`: dated nightly in CI (setup-rust-wasm action); installed "nightly" locally.
run('cargo', [
  `+${process.env.RUST_NIGHTLY || 'nightly'}`, 'build',
  '-Z', 'build-std=std,panic_abort',
  '--target', 'wasm64-unknown-unknown',
  '-p', 'gtfs-wasm',
  '--release',
]);

rmSync(outDir, { recursive: true, force: true });
run('wasm-bindgen', [wasmPath, '--target', 'web', '--out-dir', outDir]);

// wasm-opt (optional): wasm-bindgen output is unoptimized, and wasm-pack has no
// flow for this package. Binaryen >= ~119 can optimize memory64 output (including
// the 64-bit table); the older v117 bundled by wasm-pack CANNOT parse it.
// Guarded: if wasm-opt is missing or cannot parse the binary, preserve the unoptimized
// binary; the build never fails. Replace the output only when exit==0.
const bgWasm = resolve(outDir, 'gtfs_wasm_bg.wasm');
const optTmp = `${bgWasm}.opt`;
const optArgs = ['-O', '--enable-memory64', bgWasm, '-o', optTmp];
// Prefer the binaryen devDependency (node-wrapped wasm-opt) → everyone, including CI,
// gets the same version with `npm ci`. Otherwise try the standalone wasm-opt on PATH.
const localWasmOpt = resolve(uiDir, 'node_modules/binaryen/bin/wasm-opt');
const [optCmd, optCmdArgs] = existsSync(localWasmOpt)
  ? [process.execPath, [localWasmOpt, ...optArgs]]
  : ['wasm-opt', optArgs];
const opt = spawnSync(optCmd, optCmdArgs, { cwd: repoDir, stdio: 'inherit' });
if (!opt.error && opt.status === 0 && existsSync(optTmp)) {
  renameSync(optTmp, bgWasm);
  console.log('[wasm64] wasm-opt applied');
} else {
  rmSync(optTmp, { force: true });
  console.warn('[wasm64] wasm-opt skipped (compatible binaryen unavailable or parsing failed) — using unoptimized pkg64');
}

console.log(`[wasm64] ready: ${outDir}`);
