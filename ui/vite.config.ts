import { defineConfig } from 'vite';
import wasm from 'vite-plugin-wasm';
import topLevelAwait from 'vite-plugin-top-level-await';
import { readFileSync } from 'node:fs';

// Build-time sabiti: package.json sürümü (debug bundle / about için).
const pkgVersion = JSON.parse(readFileSync(new URL('./package.json', import.meta.url), 'utf8')).version;

// WASM threads (SharedArrayBuffer) crossOriginIsolated gerektirir. Prod'da bunu
// coi-serviceworker sağlar; dev/preview'de aşağıdaki başlıklarla sağlanır.
const coopCoep = {
  'Cross-Origin-Opener-Policy': 'same-origin',
  'Cross-Origin-Embedder-Policy': 'require-corp',
};

export default defineConfig({
  plugins: [wasm(), topLevelAwait()],
  base: './',
  define: { __APP_VERSION__: JSON.stringify(pkgVersion) },
  server: { headers: coopCoep },
  preview: { headers: coopCoep },
  // Worker'lar ES modülü olarak paketlenmeli: validator-worker içinde threaded WASM
  // paketi dinamik import edilir ve wasm-bindgen-rayon iç içe worker'lar açar; bunlar
  // code-splitting gerektirir (varsayılan 'iife' desteklemez). wasm + topLevelAwait
  // plugin'leri worker derlemesinde de gerekli (worker içindeki .wasm importu için).
  worker: {
    format: 'es',
    plugins: () => [wasm(), topLevelAwait()],
  },
  build: {
    target: 'esnext',
    outDir: 'dist',
    chunkSizeWarningLimit: 700,
  },
});
