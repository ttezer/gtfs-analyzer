// Threaded (rayon/atomics) WASM derlemesi — cross-platform (#19).
//
// İki Windows tuzağını çözer:
//  1) `rustup run nightly` Windows'ta MSVC host'a (link.exe) çözülüp, MSVC C++ build
//     tools yoksa build-std'nin host build-script'lerini (compiler_builtins/rustversion)
//     linklerken patlıyordu. Windows'ta GNU nightly'yi (gcc linker) AÇIKÇA seçiyoruz.
//  2) Global `RUSTFLAGS` (atomics + wasm link-arg'ları) host build-script'lerine sızıp
//     onları bozuyordu. Bayrakları yalnız wasm hedefine uygula:
//     CARGO_TARGET_WASM32_UNKNOWN_UNKNOWN_RUSTFLAGS.
//
// CI/linux'ta davranış öncekiyle aynı (toolchain = "nightly").
import { spawnSync } from 'node:child_process';

const rustflags = [
  '-C target-feature=+atomics,+bulk-memory,+mutable-globals',
  '-A warnings',              // atomics nightly-only unstable feature — beklenen uyarıyı bastır
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
const toolchain = isWin ? 'nightly-x86_64-pc-windows-gnu' : 'nightly';

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
