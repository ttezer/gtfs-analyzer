import { defineConfig } from 'vitest/config';

export default defineConfig({
  test: {
    include: ['src/__tests__/**/*.test.ts'],
    environment: 'node',
    // CI'da JUnit çıktısı da üret (artefakt olarak saklanır); yerelde sade çıktı.
    reporters: process.env.CI
      ? ['default', ['junit', { outputFile: './test-results/vitest-junit.xml' }]]
      : ['default'],
    coverage: {
      provider: 'v8',
      reporter: ['text-summary', 'html', 'lcov', 'json-summary'],
      reportsDirectory: './coverage',
      all: true,
      include: ['src/**/*.ts'],
      // The gate measures unit-testable logic only. Two categories are excluded
      // so the percentage isn't dominated by things vitest can't meaningfully
      // cover:
      //   1. Translation DATA (src/locales/**): ~5000 lines that score 100%
      //      merely by being imported — pure noise in the numerator.
      //   2. Browser-only layers exercised by Playwright e2e, not vitest:
      //      every page renderer, the modals, the WASM/worker glue and the
      //      DOM-coupled i18n/main entry points.
      // What stays in the gate: pure logic modules (dates, state, debug-buffer,
      // escape, file-summary, file-map-schema, severity-order). Some are still
      // 0% (dates/state/debug-buffer) — kept on purpose so the gate reflects
      // real untested logic instead of hiding it.
      exclude: [
        'src/__tests__/**',
        'src/**/*.d.ts',
        'src/**/types.ts',
        'src/locales/**',
        'src/pages/**',
        'src/main.ts',
        'src/i18n.ts',
        'src/wasm.ts',
        'src/validator-client.ts',
        'src/validator-worker.ts',
        'src/map-modal.ts',
        'src/pattern-modal.ts',
      ],
      // Threshold sits just under the current honest logic-coverage; raise it
      // as dates/state/debug-buffer get unit tests. functions/branches stay
      // ungated (noisy under v8 + all:true).
      thresholds: {
        lines: 65,
        statements: 65,
      },
    },
  },
});
