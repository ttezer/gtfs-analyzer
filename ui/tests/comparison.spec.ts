import { test, expect } from '@playwright/test';
import { fileURLToPath } from 'url';
import * as path from 'path';

const FIXTURE_ZIP = path.join(path.dirname(fileURLToPath(import.meta.url)), 'fixtures', 'minimal.zip');

test('analyzed feed can be compared with an older Golden v4 snapshot', async ({ page }) => {
  await page.goto('/?wasm32=1&serial=1');
  await page.locator('#file-input').setInputFiles(FIXTURE_ZIP);
  await expect(page.locator('#drop-zone.loading')).not.toBeVisible({ timeout: 25_000 });
  await expect(page.locator('#btn-report')).toBeEnabled();
  await page.locator('#btn-report').click();
  await page.locator('[data-page="compare"]').click();
  await expect(page.getByText('Sürüm Karşılaştırması', { exact: true })).toBeVisible();

  const old = {
    schema: 'gtfs-analyzer-golden/4', app_version: '0.2.0', generated_at: '2026-06-20T10:00:00.000Z', validate_date: '2026-06-20',
    feed: { file_name: 'minimal.zip', files: { 'stop_times.txt': { rows: 3, bytes: 100 } }, metrics: {
      trip_count: 1, stop_count: 2, feed_start_date: 20260101, feed_end_date: 20261231,
      service_start_date: 20260101, service_end_date: 20261231,
    } },
    validation: { config_delta: '', scores: { overall: 70, score: 70, pub_score: 80, spec: 75, interop: 70, quality: 65, analytics: 60 },
      notice_total_actual: 12, severity_counts: { HIGH: 12 }, class_counts: { QUALITY: 12 },
      rule_counts: { STM_014: { count: 12, severity: 'HIGH', rule_class: 'QUALITY', title: 'Anormal hız segmenti' } } },
  };
  await page.locator('#compare-golden-input').setInputFiles({ name: 'old.json', mimeType: 'application/json', buffer: Buffer.from(JSON.stringify(old)) });
  await expect(page.locator('.compare-table')).toBeVisible();
  await expect(page.getByText('STM_014', { exact: true })).toBeVisible();
  await expect(page.locator('#compare-rows').getByText('Düzeltildi', { exact: true })).toBeVisible();
  await expect(page.getByText('Feed veya servis tarih aralığı değişmiş.', { exact: false })).toBeVisible();

  // İkinci Golden seçimi mevcut karşılaştırmayı değiştirmeli; input sıfırlanarak
  // aynı dosyanın yeniden seçilmesine de izin vermeli.
  const second = { ...old, app_version: '0.3.0', generated_at: '2026-06-21T10:00:00.000Z' };
  await page.locator('#compare-golden-input').setInputFiles({ name: 'second.json', mimeType: 'application/json', buffer: Buffer.from(JSON.stringify(second)) });
  await expect(page.locator('.compare-runs')).toContainText('v0.3.0');
  await expect(page.locator('#compare-golden-input')).toHaveValue('');

  // Başka sekmeye gidip dönünce yüklenen karşılaştırma kaybolmamalı.
  await page.locator('[data-page="export"]').click();
  await page.locator('[data-page="compare"]').click();
  await expect(page.locator('.compare-table')).toBeVisible();
  await expect(page.locator('.compare-runs')).toContainText('v0.3.0');
});
