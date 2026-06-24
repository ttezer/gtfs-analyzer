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
      exclude: [
        'src/__tests__/**',
        'src/**/*.d.ts',
        'src/**/types.ts',
        // Render-only sayfa modülü: saf mantığı file-summary.ts / file-map-schema.ts /
        // severity-order.ts'e çıkarıldı (unit test'li); Cytoscape/DOM çizim katmanı
        // tests/file-map.spec.ts (Playwright e2e) ile kapsanıyor, unit coverage'a girmez.
        'src/pages/file-map.ts',
      ],
      // Gerçek baseline (ilk CI koşusu): lines/statements ~%64.5.
      // Eşik mevcut oranın hemen altına (60) konumlandı; tests eklendikçe ~%80'e yükseltilecek.
      // functions/branches v8 + all:true ile gürültülü (payda ~13: import edilmeyen dosyalar
      // için v8 branch/function noktası üretmiyor) olduğundan gate'lenmiyor — yanıltıcı olur.
      thresholds: {
        lines: 60,
        statements: 60,
      },
    },
  },
});
