import { execFileSync } from 'node:child_process';
import process from 'node:process';

const cargoEnv = {
  ...process.env,
  // The SDK ships the WASM binary inside an npm tarball. Keep the application
  // and CLI release profile at opt-level=3, while the SDK gets a size-oriented
  // release build with enough headroom for future English dictionary entries.
  CARGO_PROFILE_RELEASE_OPT_LEVEL: '2',
};

execFileSync('wasm-pack', [
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

execFileSync(process.execPath, ['scripts/prepare-wasm-package.mjs'], {
  env: process.env,
  stdio: 'inherit',
});
