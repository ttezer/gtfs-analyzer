import { test, expect } from '@playwright/test';
import { fileURLToPath } from 'url';
import * as path from 'path';

const FIXTURE_ZIP = path.join(
  path.dirname(fileURLToPath(import.meta.url)),
  'fixtures',
  'minimal.zip',
);

test('file map starts with the core view and expands only direct relationships', async ({ page }) => {
  await page.goto('/');
  await page.locator('#file-input').setInputFiles(FIXTURE_ZIP);
  await expect(page.locator('#drop-zone.loading')).not.toBeVisible({ timeout: 25_000 });
  await expect(page.locator('#btn-report')).toBeEnabled();
  await page.locator('#btn-report').click();

  await page.locator('.nav-btn[data-page="file-map"]').click();
  await expect(page.locator('#file-map-canvas')).toBeVisible();
  await expect(page.locator('.file-map-panel h3')).toHaveText('Standart GTFS Dosyaları');
  await expect(page.locator('#file-map-jump')).toHaveValue('');
  await expect(page.locator('#file-map-jump option[value="shapes.txt"]')).toHaveCount(0);
  await expect(page.locator('.file-map-scope-note')).toContainText('7');

  await page.locator('#file-map-severity').selectOption('CRITICAL');
  await expect(page.locator('#file-map-canvas')).toBeVisible();
  await expect(page.locator('.file-map-panel h3')).toHaveText('Standart GTFS Dosyaları');
  await page.locator('#file-map-severity').selectOption('all');

  await page.locator('#file-map-jump').selectOption('trips.txt');
  await expect(page.locator('.file-map-panel h3')).toContainText('trips.txt');
  await expect(page.locator('#file-map-jump option[value="shapes.txt"]')).toHaveCount(0);
  await expect(page.locator('#file-map-jump option[value="frequencies.txt"]')).toHaveCount(0);
  const external = page.locator('.file-map-external');
  await expect(external).toHaveAttribute('target', '_blank');
  await expect(external).toHaveAttribute('href', /#tripstxt$/);

  await page.locator('#file-map-reset').click();
  await expect(page.locator('.file-map-panel h3')).toHaveText('Standart GTFS Dosyaları');
  await expect(page.locator('#file-map-jump option[value="shapes.txt"]')).toHaveCount(0);

  await page.locator('#file-map-jump').selectOption('calendar.txt');
  await expect(page.locator('.file-map-panel h3')).toContainText('calendar.txt');
  await expect(page.locator('.file-map-panel')).toContainText(/satır|bulunmuyor/);

  const openFindings = page.locator('#file-map-open-findings');
  if (await openFindings.isEnabled()) {
    await openFindings.click();
    await expect(page.locator('.page-fix')).toBeVisible();
    await expect(page.locator('.nav-btn[data-page="fix"]')).toHaveClass(/active/);
  }
});

test('file map uses stacked layout on a narrow viewport', async ({ page }) => {
  await page.setViewportSize({ width: 430, height: 900 });
  await page.goto('/');
  await page.locator('#file-input').setInputFiles(FIXTURE_ZIP);
  await expect(page.locator('#drop-zone.loading')).not.toBeVisible({ timeout: 25_000 });
  await page.locator('#btn-report').click();
  await page.locator('.nav-btn[data-page="file-map"]').click();

  const canvasBox = await page.locator('.file-map-canvas-wrap').boundingBox();
  const panelBox = await page.locator('.file-map-panel').boundingBox();
  expect(canvasBox).not.toBeNull();
  expect(panelBox).not.toBeNull();
  expect(panelBox!.y).toBeGreaterThanOrEqual(canvasBox!.y + canvasBox!.height - 2);
});
