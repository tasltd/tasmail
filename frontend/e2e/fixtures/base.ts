// Added: Shared E2E test fixtures for TASMail (TMAIL-36)
import { test as base, type Page } from '@playwright/test';
import path from 'path';

// NOTE: Screenshots are enabled by default; set E2E_SCREENSHOTS=false to disable
const screenshotsEnabled = process.env.E2E_SCREENSHOTS !== 'false';

// Added: Extended test fixture with screenshot helper and login utility
export const test = base.extend<{
  screenshotDir: string;
  takeScreenshot: (page: Page, name: string) => Promise<void>;
  loginAs: (page: Page, email: string, password: string) => Promise<void>;
}>({
  // Added: Screenshot output directory fixture, resolved to e2e/screenshots/
  screenshotDir: async ({}, use) => {
    const dir = path.join(__dirname, '..', 'screenshots');
    await use(dir);
  },

  // Added: Conditional screenshot capture — respects E2E_SCREENSHOTS env var
  takeScreenshot: async ({screenshotDir}, use) => {
    const fn = async (page: Page, name: string) => {
      if (!screenshotsEnabled) return;
      await page.screenshot({
        path: path.join(screenshotDir, `${name}.png`),
        fullPage: false,
      });
    };
    await use(fn);
  },

  // Added: Login helper — navigates to login page, fills credentials, submits
  // NOTE: This is the ONLY place page.goto() is allowed (initial login URL)
  loginAs: async ({}, use) => {
    const fn = async (page: Page, email: string, password: string) => {
      await page.goto('/');
      // NOTE: Wait for login form to be visible before interacting
      await page.waitForSelector('#username');
      await page.fill('#username', email);
      await page.fill('#password', password);
      await page.click('button[type="submit"]');
      // Added: Wait for login to complete — sidebar appears after successful auth
      await page.waitForSelector('.sidebar', { timeout: 15_000 });
    };
    await use(fn);
  },
});

export { expect } from '@playwright/test';
