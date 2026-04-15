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
  // NOTE: Run tests sequentially in CI for stability; parallel locally
  fullyParallel: true,
  forbidOnly: !!process.env.CI,
  retries: process.env.CI ? 2 : 0,
  workers: process.env.CI ? 1 : undefined,
  reporter: 'html',
  use: {
    // Added: Vite dev server base URL
    baseURL: 'http://localhost:5173',
    // Added: 60s navigation timeout for slow page loads
    navigationTimeout: 60_000,
    trace: 'on-first-retry',
    // Added: Capture screenshot on failure automatically
    screenshot: screenshotsEnabled ? 'on' : 'off',
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
