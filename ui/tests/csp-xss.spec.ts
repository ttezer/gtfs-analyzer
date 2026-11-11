import { test, expect, type Page, type BrowserContext } from '@playwright/test';
import { fileURLToPath } from 'url';
import * as path from 'path';

// A feed whose ids and names carry `X<img src=x onerror=__hit('<field>')>`. Every page, every
// planner card (and its map) and both report tabs are opened against the production build.
// Two independent failures are caught:
//   - an escaping slip: the payload reaches the DOM as markup and the CSP reports the blocked
//     inline handler (securitypolicyviolation) — or, without a CSP, __hit actually runs;
//   - a CSP that is missing, or one that blocks the app's own scripts (report print scripts).
const PAYLOAD_ZIP = path.join(path.dirname(fileURLToPath(import.meta.url)), 'fixtures', 'xss-payload.zip');

async function instrument(context: BrowserContext): Promise<void> {
  // Runs in every document of the context, report blob tabs included.
  await context.addInitScript(() => {
    const w = window as unknown as Record<string, unknown>;
    w.__hits = [];
    w.__hit = (field: string) => (w.__hits as string[]).push(field);
    w.__violations = [];
    document.addEventListener('securitypolicyviolation', e =>
      (w.__violations as string[]).push(`${e.violatedDirective} ${e.sample || e.blockedURI}`));
    w.__prints = 0;
    window.print = () => { w.__prints = (w.__prints as number) + 1; };
  });
}

const state = (page: Page) => page.evaluate(() => {
  const w = window as unknown as Record<string, unknown>;
  return { hits: w.__hits as string[], violations: w.__violations as string[], prints: w.__prints as number };
});

test('feed values never run as code; the CSP is in place and report printing works', async ({ page, context }) => {
  test.setTimeout(120_000);
  await instrument(context);
  await page.goto('./');
  expect(await page.locator('meta[http-equiv="Content-Security-Policy"]').count()).toBe(1);
  // WASM threads need cross-origin isolation; on GitHub Pages coi-serviceworker provides it,
  // so this also proves the CSP lets the service worker register.
  await expect.poll(() => page.evaluate(() => crossOriginIsolated)).toBe(true);

  await page.locator('#file-input').setInputFiles(PAYLOAD_ZIP);
  await expect(page.locator('#drop-zone.loading')).not.toBeVisible({ timeout: 25_000 });
  await page.locator('#btn-report').click();

  for (const id of ['domain', 'fix', 'files', 'file-map', 'export', 'producer']) {
    const nav = page.locator(`.nav-btn[data-page="${id}"]`);
    if (await nav.count()) { await nav.click(); await page.waitForTimeout(300); }
  }
  // Planner: open every card; opening fills its texts and draws its map (permanent labels).
  const headers = page.locator('.planner-card-header');
  const cards = await headers.count();
  expect(cards).toBeGreaterThan(0);
  for (let i = 0; i < cards; i++) {
    await headers.nth(i).click();
    await page.waitForTimeout(150);
  }
  await page.waitForTimeout(500);
  const app = await state(page);
  expect(app.hits, 'payload executed').toEqual([]);
  expect(app.violations, 'payload reached the DOM as markup (blocked by CSP)').toEqual([]);

  // Reports open as noopener blob tabs and inherit the CSP.
  await page.locator('.nav-btn[data-page="export"]').click();
  const [classic] = await Promise.all([context.waitForEvent('page'), page.locator('#btn-export-pdf').click()]);
  await classic.waitForLoadState('load');
  await expect.poll(async () => (await state(classic)).prints, { message: 'auto-print did not run' }).toBe(1);
  expect((await state(classic)).violations).toEqual([]);

  const [exec] = await Promise.all([context.waitForEvent('page'), page.locator('#btn-export-executive').click()]);
  await exec.waitForLoadState('load');
  await exec.locator('.print-action').click();
  await expect.poll(async () => (await state(exec)).prints, { message: 'print button did not work' }).toBe(1);
  const report = await state(exec);
  expect(report.hits).toEqual([]);
  expect(report.violations).toEqual([]);
});
