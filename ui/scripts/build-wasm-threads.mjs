// Cross-platform threaded (rayon/atomics) WASM build (#19).
//
// Fixes two Windows traps:
//  1) `rustup run nightly` resolved to the MSVC host (link.exe) on Windows and failed while
//     linking build-std host build scripts (compiler_builtins/rustversion) when MSVC C++ build
//     tools were absent. Explicitly select the GNU nightly (gcc linker) on Windows.
//  2) Global `RUSTFLAGS` (atomics + wasm link args) leaked into host build scripts and broke
//     them. Apply the flags only to the wasm target:
//     CARGO_TARGET_WASM32_UNKNOWN_UNKNOWN_RUSTFLAGS.
//
// Behavior on CI/Linux is unchanged (toolchain = "nightly").
//
// When `RUST_NIGHTLY` is set (CI, setup-rust-wasm action), use the dated nightly;
// locally it is unset and the installed "nightly" is used.
import { spawnSync } from 'node:child_process';

const rustflags = [
  '-C target-feature=+atomics,+bulk-memory,+mutable-globals',
  '-A warnings',              // atomics nightly-only unstable feature — suppress the expected warning
  '-C link-arg=--shared-memory',
  '-C link-arg=--max-memory=4294967296',
  '-C link-arg=--import-memory',
  '-C link-arg=--export=__heap_base',
  '-C link-arg=--export=__wasm_init_tls',
  '-C link-arg=--export=__tls_size',
  '-C link-arg=--export=__tls_align',
  '-C link-arg=--export=__tls_base',
].join(' ');

const isWin = process.platform === 'win32';
const nightly = process.env.RUST_NIGHTLY || 'nightly';
const toolchain = isWin ? `${nightly}-x86_64-pc-windows-gnu` : nightly;

const env = {
  ...process.env,
  CARGO_TARGET_WASM32_UNKNOWN_UNKNOWN_RUSTFLAGS: rustflags,
  CARGO_UNSTABLE_BUILD_STD: 'panic_abort,std',
};

const args = [
  'run', toolchain,
  'wasm-pack', 'build', '../crates/wasm',
  '--target', 'web',
  '--out-dir', '../../ui/pkg-threads',
  '--release',
  '--features', 'threads',
];

console.log(`[wasm:threads] toolchain=${toolchain} platform=${process.platform}`);
const res = spawnSync('rustup', args, { stdio: 'inherit', env, shell: isWin });
process.exit(res.status ?? 1);
