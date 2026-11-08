/** Playwright config for the screenshot harness (`pnpm shots`): Chromium against the Vite dev server, mock mode only. */
import { defineConfig } from '@playwright/test';

export default defineConfig({
  testDir: 'tests',
  timeout: 60_000,
  workers: 1,
  outputDir: 'artifacts/test-results',
  reporter: 'list',
  use: { baseURL: 'http://localhost:1420', viewport: { width: 1280, height: 720 }, deviceScaleFactor: 1 },
  webServer: {
    command: 'pnpm exec vite --port 1420 --strictPort',
    url: 'http://localhost:1420',
    // Locally, reuse a dev server already on 1420 (e.g. one started for the desktop app); CI always starts its own.
    reuseExistingServer: !process.env.CI,
    timeout: 60_000,
  },
});
