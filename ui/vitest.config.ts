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
      exclude: ['src/__tests__/**', 'src/**/*.d.ts', 'src/**/types.ts'],
      // BASELINE eşikleri: UI mantığının bir kısmı yalnızca Playwright E2E ile test edildiğinden
      // vitest (birim) coverage'ı bilinçli olarak düşük başlar. İlk CI raporundaki gerçek oran
      // görüldükten sonra bu değerler kademeli olarak ~%80'e yükseltilecek (ratchet).
      thresholds: {
        lines: 25,
        functions: 25,
        branches: 60,
        statements: 25,
      },
    },
  },
});
