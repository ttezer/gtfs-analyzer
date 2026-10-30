import { execFileSync } from 'node:child_process';
import { rmSync } from 'node:fs';
import process from 'node:process';

const cargoEnv = {
  ...process.env,
  // The SDK ships the WASM binary inside an npm tarball. Keep the application
  // and CLI release profile at opt-level=3, while the SDK gets a size-oriented
  // release build with enough headroom for future English dictionary entries.
  CARGO_PROFILE_RELEASE_OPT_LEVEL: process.env.WASM_CARGO_OPT_LEVEL ?? 's',
};

// Both engines by default. CI builds them in separate parallel jobs
// (SDK_WASM_TARGETS=wasm32 / wasm64) so the wasm64 build does not lengthen the
// SDK job on the path to the UI build and deploy.
const targets = new Set((process.env.SDK_WASM_TARGETS ?? 'wasm32,wasm64').split(',').map((t) => t.trim()));
for (const target of targets) {
  if (target !== 'wasm32' && target !== 'wasm64') {
    throw new Error(`Unknown SDK_WASM_TARGETS entry: ${target}. Use wasm32 and/or wasm64.`);
  }
}

if (targets.has('wasm32')) {
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
}

if (targets.has('wasm64')) {
  // Memory64 engine for large feeds. wasm32 caps the engine at 4 GiB, which
  // feeds above ~30M stop_times rows exceed; Node 24 and Memory64 browsers run
  // this build instead. wasm64 is Tier 3 in Rust: nightly build-std, and
  // wasm-bindgen directly because wasm-pack has no wasm64 flow (same path as
  // ui/scripts/build-wasm64.mjs). `RUST_NIGHTLY`: dated nightly in CI.
  execFileSync('cargo', [
    `+${process.env.RUST_NIGHTLY || 'nightly'}`,
    'build',
    '-Z',
    'build-std=std,panic_abort',
    '--target',
    'wasm64-unknown-unknown',
    '-p',
    'gtfs-wasm',
    '--release',
    '--features',
    'quiet,sdk-en',
  ], { cwd: '..', env: cargoEnv, stdio: 'inherit' });

  rmSync('pkg64', { recursive: true, force: true });
  execFileSync(process.env.WASM_BINDGEN_BIN ?? 'wasm-bindgen', [
    '../target/wasm64-unknown-unknown/release/gtfs_wasm.wasm',
    '--target',
    'web',
    '--out-dir',
    'pkg64',
  ], { stdio: 'inherit' });

  execFileSync('wasm-opt', [
    '-Oz',
    '--enable-memory64',
    '--strip-producers',
    '--strip-target-features',
    '--strip-toolchain-annotations',
    '--strip-debug',
    'pkg64/gtfs_wasm_bg.wasm',
    '-o',
    'pkg64/gtfs_wasm_bg.wasm',
  ], { stdio: 'inherit' });
}

execFileSync(process.execPath, ['scripts/prepare-wasm-package.mjs'], {
  env: process.env,
  stdio: 'inherit',
});
