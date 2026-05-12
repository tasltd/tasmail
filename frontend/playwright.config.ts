// Added: Playwright E2E configuration for TASMail frontend (TMAIL-36)
import { defineConfig, devices } from '@playwright/test';

// NOTE: Screenshots are enabled by default; set E2E_SCREENSHOTS=false to disable
const screenshotsEnabled = process.env.E2E_SCREENSHOTS !== 'false';

export default defineConfig({
  testDir: './e2e',
  // Added: 30s timeout per test, 60s for navigation actions
  timeout: 30_000,
  expect: {
    timeout: 5_000,
  },
  // BYOK signup specs share a database — keep them serial so user-collision races
  // can't flake the suite. Set PLAYWRIGHT_PARALLEL=true for local-only smoke runs.
  fullyParallel: process.env.PLAYWRIGHT_PARALLEL === 'true',
  workers: process.env.PLAYWRIGHT_PARALLEL === 'true' ? undefined : 1,
  forbidOnly: !!process.env.CI,
  retries: process.env.CI ? 2 : 0,
  reporter: 'html',
  use: {
    // Default to the live tunnel/proxy URL so specs run end-to-end through the same
    // Apache → SSH tunnel → backend stack a real user hits. Override with
    // PLAYWRIGHT_BASE_URL=http://localhost:5273 for local dev runs.
    baseURL: process.env.PLAYWRIGHT_BASE_URL ?? 'https://mail.techatscale.io',
    // Added: 60s navigation timeout for slow page loads
    navigationTimeout: 60_000,
    trace: 'on-first-retry',
    // Added: Capture screenshot on failure automatically
    screenshot: screenshotsEnabled ? 'on' : 'off',
    // Production URL has Let's Encrypt cert; localhost dev cert won't be valid.
    ignoreHTTPSErrors: true,
  },
  projects: [
    {
      // Added: Firefox-only project per HARD RULE
      name: 'firefox',
      use: { ...devices['Desktop Firefox'] },
    },
  ],
  // Added: Screenshot and report output directories
  outputDir: 'test-results/',
});
