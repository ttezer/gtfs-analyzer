import { defineConfig, devices } from '@playwright/test';

export default defineConfig({
  testDir: './tests',
  timeout: 30_000,
  use: {
    baseURL: 'http://localhost:4173',
    headless: true,
  },
  projects: [{ name: 'chromium', use: { ...devices['Desktop Chrome'], channel: 'chrome' } }],
  webServer: {
    command: 'node node_modules/vite/bin/vite.js preview --port 4173',
    port: 4173,
    reuseExistingServer: false,
    timeout: 15_000,
  },
});
