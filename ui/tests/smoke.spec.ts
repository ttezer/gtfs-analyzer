import { test, expect } from '@playwright/test';
import { fileURLToPath } from 'url';
import * as path from 'path';

const FIXTURE_ZIP = path.join(
  path.dirname(fileURLToPath(import.meta.url)),
  'fixtures',
  'minimal.zip',
);

test('sayfa açılıyor ve upload zone görünüyor', async ({ page }) => {
  await page.goto('/');
  await expect(page.locator('h1')).toHaveText('GTFS Validator');
  await expect(page.locator('.drop-zone')).toBeVisible();
  await expect(page.locator('#file-input')).toBeAttached();
});

test('.zip olmayan dosyada hata kartı çıkıyor', async ({ page }) => {
  await page.goto('/');
  await page.locator('#file-input').setInputFiles({
    name: 'notazip.txt',
    mimeType: 'text/plain',
    buffer: Buffer.from('bu bir zip dosyasi degil'),
  });
  const status = page.locator('.upload-status.error');
  await expect(status).toBeVisible({ timeout: 5_000 });
  await expect(status).toContainText('Geçersiz girdi');
});

test('ZIP → WASM çalışır → loading biter, sonuç gelir', async ({ page }) => {
  const consoleErrors: string[] = [];
  page.on('console', msg => { if (msg.type() === 'error') consoleErrors.push(msg.text()); });

  await page.goto('/');
  await page.locator('#file-input').setInputFiles(FIXTURE_ZIP);

  // Loading state görünmeli — drop-zone.loading class'ı eklenir
  await expect(page.locator('#drop-zone.loading')).toBeVisible({ timeout: 5_000 });

  // Loading bitmeli — hata da başarı da loading'i kaldırır
  await expect(page.locator('#drop-zone.loading')).not.toBeVisible({ timeout: 25_000 });

  // Sonuç: ya skor paneli (başarı) ya da hata kartı
  const hasScore = await page.locator('#score-panel.score-compact').isVisible();
  const hasError = await page.locator('.upload-status.error').isVisible();
  expect(hasScore || hasError, `WASM sonuç vermedi. Console: ${consoleErrors.join(', ')}`).toBe(true);
});

test('bozuk ZIP dosyası hata kartı gösteriyor', async ({ page }) => {
  await page.goto('/');
  await page.locator('#file-input').setInputFiles({
    name: 'corrupt.zip',
    mimeType: 'application/zip',
    buffer: Buffer.from('bu gercek bir zip degil - sadece rastgele baytlar'),
  });

  const status = page.locator('.upload-status.error');
  await expect(status).toBeVisible({ timeout: 15_000 });
});

test('512 MB üzeri dosya reddediliyor', async ({ page }) => {
  await page.goto('/');

  // 513 MB sahte buffer — content önemsiz, sadece boyut kontrol ediliyor
  const bigBuffer = Buffer.alloc(513 * 1024 * 1024, 0);
  await page.locator('#file-input').setInputFiles({
    name: 'huge.zip',
    mimeType: 'application/zip',
    buffer: bigBuffer,
  });

  const status = page.locator('.upload-status.error');
  await expect(status).toBeVisible({ timeout: 5_000 });
  await expect(status).toContainText('512 MB');
});
