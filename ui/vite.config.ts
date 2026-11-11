import { defineConfig } from 'vite';
import wasm from 'vite-plugin-wasm';
import { readFileSync } from 'node:fs';
import { fileURLToPath } from 'node:url';
import { createHash } from 'node:crypto';
import type { Plugin } from 'vite';
import { REPORT_INLINE_SCRIPTS } from './src/print-doc';

// Build-time sabiti: package.json sürümü (debug bundle / about için).
const pkgVersion = JSON.parse(readFileSync(new URL('./package.json', import.meta.url), 'utf8')).version;
const sdkSource = fileURLToPath(new URL('../sdk/src/index.ts', import.meta.url));

// WASM threads (SharedArrayBuffer) crossOriginIsolated gerektirir. Prod'da bunu
// coi-serviceworker sağlar; dev/preview'de aşağıdaki başlıklarla sağlanır.
const coopCoep = {
  'Cross-Origin-Opener-Policy': 'same-origin',
  'Cross-Origin-Embedder-Policy': 'require-corp',
};

// Content-Security-Policy for the production build (GitHub Pages cannot send headers, so it goes
// in a <meta>). It blocks inline scripts and on* handlers: an escaping slip that writes feed text
// as HTML (planner XSS, 2026-11) can no longer run code. Build only: the Vite dev server injects
// its own inline scripts. Report tabs are blob: documents that inherit this policy, so their two
// inline scripts are allowed by hash, computed here from print-doc.ts. frame-ancestors is
// ignored in a <meta> and left out. connect-src https: is needed by "Load from URL".
const sha256 = (s: string) => `'sha256-${createHash('sha256').update(s, 'utf8').digest('base64')}'`;
export const CSP = [
  "default-src 'self'",
  `script-src 'self' 'wasm-unsafe-eval' https://www.googletagmanager.com ${REPORT_INLINE_SCRIPTS.map(sha256).join(' ')}`,
  "style-src 'self' 'unsafe-inline'",
  "img-src 'self' data: blob: https://tile.openstreetmap.org https://*.google-analytics.com https://*.googletagmanager.com",
  "connect-src 'self' https:",
  "worker-src 'self' blob:",
  "font-src 'self' data:",
  "object-src 'none'",
  "base-uri 'self'",
  "form-action 'none'",
].join('; ');

function cspMeta(): Plugin {
  return {
    name: 'csp-meta',
    apply: 'build',
    transformIndexHtml: () => [{ tag: 'meta', attrs: { 'http-equiv': 'Content-Security-Policy', content: CSP }, injectTo: 'head-prepend' }],
  };
}

export default defineConfig({
  plugins: [wasm(), cspMeta()],
  base: './',
  define: { __APP_VERSION__: JSON.stringify(pkgVersion) },
  resolve: { alias: { 'gtfs-sdk': sdkSource } },
  server: { headers: coopCoep },
  preview: { headers: coopCoep },
  // Worker'lar ES modülü olarak paketlenmeli: validator-worker içinde threaded WASM
  // paketi dinamik import edilir ve wasm-bindgen-rayon iç içe worker'lar açar; bunlar
  // code-splitting gerektirir (varsayılan 'iife' desteklemez). wasm + topLevelAwait
  // plugin'leri worker derlemesinde de gerekli (worker içindeki .wasm importu için).
  worker: {
    format: 'es',
    plugins: () => [wasm()],
  },
  build: {
    target: 'esnext',
    outDir: 'dist',
    chunkSizeWarningLimit: 700,
    rollupOptions: {
      output: { manualChunks: { leaflet: ['leaflet'] } },
    },
  },
});
