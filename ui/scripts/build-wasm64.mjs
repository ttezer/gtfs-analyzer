// Memory64 build'i. wasm64 Rust'ta Tier 3 olduğu için nightly build-std gerekir;
// JS binding'i wasm-pack yerine doğrudan wasm-bindgen üretir. `--threads`
// artifact'i gelecekteki wasm64 atomics/rayon denemesi için ayrıca paketlenir;
// mevcut tarayıcı runtime'ı bunu seçmez.
import { rmSync, renameSync, existsSync, writeFileSync } from 'node:fs';
import { dirname, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';
import { spawnSync } from 'node:child_process';

const uiDir = resolve(dirname(fileURLToPath(import.meta.url)), '..');
const repoDir = resolve(uiDir, '..');
const wasmPath = resolve(repoDir, 'target/wasm64-unknown-unknown/release/gtfs_wasm.wasm');
const threaded = process.env.WASM64_THREADS === '1' || process.argv.includes('--threads');
const outDir = resolve(uiDir, threaded ? 'pkg64-threads' : 'pkg64');

function run(command, args, cwd = repoDir, env = process.env) {
  console.log(`[wasm64] ${command} ${args.join(' ')}`);
  const result = spawnSync(command, args, { cwd, stdio: 'inherit', env });
  if (result.error) throw result.error;
  if (result.status !== 0) process.exit(result.status ?? 1);
}

// `RUST_NIGHTLY`: CI'da tarihli nightly (setup-rust-wasm action'ı); yerelde kurulu "nightly".
const buildArgs = [
  `+${process.env.RUST_NIGHTLY || 'nightly'}`, 'build',
  '-Z', 'build-std=std,panic_abort',
  '--target', 'wasm64-unknown-unknown',
  '-p', 'gtfs-wasm',
  '--release',
];
if (threaded) {
  // Experimental artifact: the current js-sys/wasm-bindgen-rayon toolchain
  // does not yet produce a browser-usable shared wasm64 memory.
  buildArgs.push('--features', 'threads');
}
run('cargo', buildArgs);

rmSync(outDir, { recursive: true, force: true });
run('wasm-bindgen', [wasmPath, '--target', 'web', '--out-dir', outDir]);

// wasm-bindgen does not emit package.json when invoked directly. Rayon’s
// workerHelpers.js resolves the main module through `../../..`, so Vite needs
// this small package manifest to resolve that directory import during the UI
// build (wasm-pack emits the same manifest for the wasm32 threaded package).
if (threaded) {
  writeFileSync(resolve(outDir, 'package.json'), JSON.stringify({
    name: 'gtfs-wasm-memory64-threaded',
    type: 'module',
    version: '0.17.0',
    license: 'MIT',
    main: 'gtfs_wasm.js',
    types: 'gtfs_wasm.d.ts',
    sideEffects: ['./snippets/*'],
  }, null, 2) + '\n');
}

// wasm-opt (opsiyonel): wasm-bindgen çıktısı optimize edilmemiştir ve wasm-pack'in
// bu paket için akışı yoktur. Binaryen >= ~119 memory64 çıktısını (64-bit tablo dahil)
// optimize edebilir; wasm-pack'in paketlediği eski v117 bunu parse EDEMEZ.
// Guarded: bir wasm-opt yoksa ya da parse edemezse optimize edilmemiş binary korunur;
// build hiçbir koşulda kırılmaz. Çıktı yalnız exit==0'da değiştirilir.
const bgWasm = resolve(outDir, 'gtfs_wasm_bg.wasm');
const optTmp = `${bgWasm}.opt`;
const optArgs = ['-O', '--enable-memory64'];
if (threaded) optArgs.push('--enable-threads');
optArgs.push(bgWasm, '-o', optTmp);
// binaryen devDependency'si (node-sarmalı wasm-opt) tercih edilir → CI dahil herkes
// `npm ci` ile aynı sürümü alır. Yoksa PATH'teki standalone wasm-opt denenir.
const localWasmOpt = resolve(uiDir, 'node_modules/binaryen/bin/wasm-opt');
const [optCmd, optCmdArgs] = existsSync(localWasmOpt)
  ? [process.execPath, [localWasmOpt, ...optArgs]]
  : ['wasm-opt', optArgs];
const opt = spawnSync(optCmd, optCmdArgs, { cwd: repoDir, stdio: 'inherit' });
if (!opt.error && opt.status === 0 && existsSync(optTmp)) {
  renameSync(optTmp, bgWasm);
  console.log('[wasm64] wasm-opt uygulandı');
} else {
  rmSync(optTmp, { force: true });
  console.warn('[wasm64] wasm-opt atlandı (uyumlu binaryen yok veya parse edemedi) — optimize edilmemiş pkg64 kullanılıyor');
}

console.log(`[wasm64${threaded ? '-threads' : ''}] hazır: ${outDir}`);
