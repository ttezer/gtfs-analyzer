// Deneysel serial Memory64 build'i. wasm64 Rust'ta Tier 3 olduğu için nightly
// build-std gerekir; JS binding'i wasm-pack yerine doğrudan wasm-bindgen üretir.
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

run('cargo', [
  '+nightly', 'build',
  '-Z', 'build-std=std,panic_abort',
  '--target', 'wasm64-unknown-unknown',
  '-p', 'gtfs-wasm',
  '--release',
]);

rmSync(outDir, { recursive: true, force: true });
run('wasm-bindgen', [wasmPath, '--target', 'web', '--out-dir', outDir]);

// wasm-opt (opsiyonel): wasm-bindgen çıktısı optimize edilmemiştir ve wasm-pack'in
// bu paket için akışı yoktur. Binaryen >= ~119 memory64 çıktısını (64-bit tablo dahil)
// optimize edebilir; wasm-pack'in paketlediği eski v117 bunu parse EDEMEZ.
// Guarded: bir wasm-opt yoksa ya da parse edemezse optimize edilmemiş binary korunur;
// build hiçbir koşulda kırılmaz. Çıktı yalnız exit==0'da değiştirilir.
const bgWasm = resolve(outDir, 'gtfs_wasm_bg.wasm');
const optTmp = `${bgWasm}.opt`;
const optArgs = ['-O', '--enable-memory64', bgWasm, '-o', optTmp];
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

console.log(`[wasm64] hazır: ${outDir}`);
