import { test, expect } from '@playwright/test';
import { fileURLToPath } from 'url';
import * as path from 'path';

const FIXTURE_ZIP = path.join(
  path.dirname(fileURLToPath(import.meta.url)),
  'fixtures',
  'minimal.zip',
);

// WASM32 threads ↔ seri fallback: izole ortamda (vite preview COOP/COEP) thread'ler
// aktif olmalı; izole değilse seri fallback DEVREYE girip AYNI sonucu üretmeli.
// Test her iki yolda da yeşil kalır (CI tarayıcısı SAB sağlamasa bile), ama izole
// iken threading'in gerçekten devreye girdiğini doğrular (regression guard).
// v0.2.0'da varsayılan Memory64 seri olduğundan bu test WASM32'yi bilinçli zorlar.
test('threaded WASM (izole) veya seri fallback — her durumda doğru sonuç, hatasız', async ({ page }) => {
  const logs: string[] = [];
  const errors: string[] = [];
  page.on('console', (m) => logs.push(`[${m.type()}] ${m.text()}`));
  page.on('pageerror', (e) => errors.push(String(e)));

  await page.goto('/?wasm32=1');
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

test('Memory64 destekleniyorsa büyük-feed varsayılanı güvenli serial WASM64', async ({ page }) => {
  const logs: string[] = [];
  page.on('console', (m) => logs.push(`[${m.type()}] ${m.text()}`));

  await page.goto('/');
  const memory64 = await page.evaluate(() => {
    try {
      new WebAssembly.Memory({ address: 'i64', initial: 1n, maximum: 1n });
      return true;
    } catch {
      return false;
    }
  });
  test.skip(!memory64, 'Tarayıcı Memory64 desteklemiyor');

  await page.locator('#file-input').setInputFiles(FIXTURE_ZIP);
  await expect(page.locator('#drop-zone.loading')).not.toBeVisible({ timeout: 25_000 });
  expect(await page.locator('#score-panel.score-compact').isVisible()).toBe(true);
  expect(logs.some((line) => line.includes('[WASM] wasm64 serial mode'))).toBe(true);
  expect(logs.some((line) => /DataCloneError/i.test(line))).toBe(false);
});
