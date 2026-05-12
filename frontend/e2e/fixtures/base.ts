// Added: Shared E2E test fixtures for TASMail (TMAIL-36)
import { test as base, type Page } from '@playwright/test';
import path from 'path';
import { fileURLToPath } from 'url';

// Fix: Use import.meta.url for ESM compatibility instead of __dirname
const __filename = fileURLToPath(import.meta.url);
const __dirname = path.dirname(__filename);

// NOTE: Screenshots are enabled by default; set E2E_SCREENSHOTS=false to disable
const screenshotsEnabled = process.env.E2E_SCREENSHOTS !== 'false';

/**
 * Real-mailbox credentials for the test bed. The noreply@techatscale.io mailbox
 * is hosted on swmail.techatscale.io (Stalwart IMAP4rev2 + SMTP submissions on 465).
 *
 * The IMAP server expects the bare local-part as the username (`noreply`), not the
 * full email address — verified directly with `openssl s_client + IMAP LOGIN`.
 *
 * Override with NOREPLY_PASSWORD env var if the password rotates so the suite
 * does not need a code change.
 */
export const NOREPLY_CREDS = {
  email: 'noreply@techatscale.io',
  imap: {
    host: 'swmail.techatscale.io',
    port: 993,
    encryption: 'ssl' as const,
    username: 'noreply',
    password: process.env.NOREPLY_PASSWORD ?? 't@s.noreply@2025',
  },
  smtp: {
    host: 'swmail.techatscale.io',
    port: 465,
    encryption: 'ssl' as const,
    username: 'noreply',
    password: process.env.NOREPLY_PASSWORD ?? 't@s.noreply@2025',
  },
} as const;

// Added: Extended test fixture with screenshot helper, login + signup utilities.
export const test = base.extend<{
  screenshotDir: string;
  takeScreenshot: (page: Page, name: string) => Promise<void>;
  loginAs: (page: Page, email: string, password: string) => Promise<void>;
  signupAs: (page: Page, email: string, password: string) => Promise<void>;
  apiSignup: (email: string, password: string) => Promise<{ access_token: string; refresh_token: string }>;
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
      await page.goto('/login');
      await page.waitForSelector('#username');
      await page.fill('#username', email);
      await page.fill('#password', password);
      await page.click('button[type="submit"]');
      // After successful login the SPA routes to /app (sidebar lives there).
      await page.waitForURL(/\/app/, { timeout: 15_000 });
    };
    await use(fn);
  },

  // BYOK signup via the public /signup form. Drops the user at /onboarding.
  signupAs: async ({}, use) => {
    const fn = async (page: Page, email: string, password: string) => {
      await page.goto('/signup');
      await page.waitForSelector('#email');
      await page.fill('#email', email);
      await page.fill('#password', password);
      await page.fill('#confirm', password);
      await page.click('button[type="submit"]');
      await page.waitForURL(/\/onboarding/, { timeout: 15_000 });
    };
    await use(fn);
  },

  // Direct API signup — bypasses the form for tests that just need a token.
  // Returns the JWT pair so the caller can write it to localStorage and visit
  // protected pages directly without going through the UI signup flow.
  apiSignup: async ({ baseURL }, use) => {
    const fn = async (email: string, password: string) => {
      const url = `${baseURL?.replace(/\/$/, '') ?? ''}/api/auth/signup`;
      const resp = await fetch(url, {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({ email, password }),
      });
      if (!resp.ok) {
        throw new Error(`apiSignup failed: HTTP ${resp.status} ${await resp.text()}`);
      }
      return resp.json() as Promise<{ access_token: string; refresh_token: string }>;
    };
    await use(fn);
  },
});

export { expect } from '@playwright/test';
