import { execFileSync } from 'node:child_process';
import process from 'node:process';

const cargoEnv = {
  ...process.env,
  // The SDK ships the WASM binary inside an npm tarball. Keep the application
  // and CLI release profile at opt-level=3, while the SDK gets a size-oriented
  // release build with enough headroom for future English dictionary entries.
  CARGO_PROFILE_RELEASE_OPT_LEVEL: process.env.WASM_CARGO_OPT_LEVEL ?? 's',
};

const wasmPackBin = process.env.WASM_PACK_BIN ?? 'wasm-pack';
const expectedWasmPackVersion = process.env.WASM_PACK_VERSION ?? '0.15.0';
const installedWasmPackVersion = execFileSync(wasmPackBin, ['--version'], { encoding: 'utf8' }).trim();
if (installedWasmPackVersion !== `wasm-pack ${expectedWasmPackVersion}`) {
  throw new Error(
    `Expected wasm-pack ${expectedWasmPackVersion}, got ${installedWasmPackVersion}. `
      + 'Set WASM_PACK_BIN/WASM_PACK_VERSION explicitly when using a pinned tool.',
  );
}

execFileSync(wasmPackBin, [
  'build',
  '../crates/wasm',
  '--target',
  'web',
  '--out-dir',
  '../../sdk/pkg',
  '--release',
  '--',
  '--features',
  'quiet,sdk-en',
], { env: cargoEnv, stdio: 'inherit' });

// The SDK is the size-sensitive artifact. Keep the application build's O3
// profile unchanged, then run the size-oriented pass only on this npm binary.
execFileSync('wasm-opt', [
  '-Oz',
  '--strip-producers',
  '--strip-target-features',
  '--strip-toolchain-annotations',
  '--strip-debug',
  'pkg/gtfs_wasm_bg.wasm',
  '-o',
  'pkg/gtfs_wasm_bg.wasm',
], { stdio: 'inherit' });

execFileSync(process.execPath, ['scripts/prepare-wasm-package.mjs'], {
  env: process.env,
  stdio: 'inherit',
});
