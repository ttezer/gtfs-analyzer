import { test, expect } from '@playwright/test';
import { fileURLToPath } from 'url';
import * as path from 'path';

const FIXTURE_ZIP = path.join(
  path.dirname(fileURLToPath(import.meta.url)),
  'fixtures',
  'minimal.zip',
);

// WASM threads ↔ seri fallback: izole ortamda (vite preview COOP/COEP) thread'ler
// aktif olmalı; izole değilse seri fallback DEVREYE girip AYNI sonucu üretmeli.
// Test her iki yolda da yeşil kalır (CI tarayıcısı SAB sağlamasa bile), ama izole
// iken threading'in gerçekten devreye girdiğini doğrular (regression guard).
test('threaded WASM (izole) veya seri fallback — her durumda doğru sonuç, hatasız', async ({ page }) => {
  const logs: string[] = [];
  const errors: string[] = [];
  page.on('console', (m) => logs.push(`[${m.type()}] ${m.text()}`));
  page.on('pageerror', (e) => errors.push(String(e)));

  await page.goto('/');
  const isolated = await page.evaluate(
    () => (globalThis as { crossOriginIsolated?: boolean }).crossOriginIsolated === true,
  );

  await page.locator('#file-input').setInputFiles(FIXTURE_ZIP);
  await expect(page.locator('#drop-zone.loading')).toBeVisible({ timeout: 5_000 });
  await expect(page.locator('#drop-zone.loading')).not.toBeVisible({ timeout: 25_000 });

  // Her durumda: doğrulama başarılı (skor paneli) + runtime WASM/worker hatası YOK.
  expect(
    await page.locator('#score-panel.score-compact').isVisible(),
    `Skor paneli gelmedi. errors: ${errors.join('; ')}`,
  ).toBe(true);
  const fatal = logs.filter((l) => /RuntimeError|unreachable|memory access|threaded init başarısız/i.test(l));
  expect(fatal, `WASM/worker runtime hatası: ${fatal.join(' | ')}`).toHaveLength(0);

  const mode = logs.find((l) => l.includes('[WASM]') && /mode|mod/.test(l)) ?? '(mode log yok)';
  console.log(`crossOriginIsolated=${isolated}  →  ${mode}`);

  // İzole ise threading GERÇEKTEN aktif olmalı (yoksa thread kurulumu bozulmuş demektir).
  if (isolated) expect(mode, 'izole ortamda threaded mode bekleniyordu').toMatch(/threaded mode/);
});
