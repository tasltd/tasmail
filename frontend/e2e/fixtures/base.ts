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

// TMAIL-405: helper that marks the FirstLoginTour as already seen for a given
// access token. The tour mounts on /app for every fresh account and its
// backdrop / popover would otherwise intercept downstream Playwright clicks.
// Tests that explicitly need the tour to appear (e.g. an auth-onboarding spec
// that validates the tour itself) should NOT call this — see signupAsTourVisible.
async function markFirstLoginTourSeenViaApi(baseURL: string | undefined, accessToken: string): Promise<void> {
  const url = `${baseURL?.replace(/\/$/, '') ?? ''}/api/me/preferences/first-login-tour-seen`;
  const resp = await fetch(url, {
    method: 'PATCH',
    headers: {
      Authorization: `Bearer ${accessToken}`,
      'Content-Type': 'application/json',
    },
    body: '{}',
  });
  if (!resp.ok) {
    // Don't hard-fail the test — older backends without this endpoint should
    // still let the rest of the fixtures run. Surface the warning so it's
    // visible in CI logs.
    console.warn(`markFirstLoginTourSeenViaApi: PATCH returned HTTP ${resp.status}`);
  }
}

// Added: Extended test fixture with screenshot helper, login + signup utilities.
export const test = base.extend<{
  screenshotDir: string;
  takeScreenshot: (page: Page, name: string) => Promise<void>;
  loginAs: (page: Page, email: string, password: string) => Promise<void>;
  signupAs: (page: Page, email: string, password: string) => Promise<void>;
  apiSignup: (email: string, password: string) => Promise<{ access_token: string; refresh_token: string }>;
  apiSignupTourVisible: (email: string, password: string) => Promise<{ access_token: string; refresh_token: string }>;
  markTourSeen: (accessToken: string) => Promise<void>;
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
  //
  // TMAIL-405: After signup we also PATCH /api/me/preferences/first-login-tour-seen
  // so the FirstLoginTour overlay (added in TMAIL-401) doesn't render on /app and
  // intercept downstream Playwright interactions. Tests that need to drive the
  // tour itself should use `apiSignupTourVisible` instead.
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
      const tokens = (await resp.json()) as { access_token: string; refresh_token: string };
      await markFirstLoginTourSeenViaApi(baseURL, tokens.access_token);
      return tokens;
    };
    await use(fn);
  },

  // TMAIL-405: variant of apiSignup that does NOT pre-mark the tour as seen.
  // Use this in specs that explicitly validate the FirstLoginTour UI.
  apiSignupTourVisible: async ({ baseURL }, use) => {
    const fn = async (email: string, password: string) => {
      const url = `${baseURL?.replace(/\/$/, '') ?? ''}/api/auth/signup`;
      const resp = await fetch(url, {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({ email, password }),
      });
      if (!resp.ok) {
        throw new Error(`apiSignupTourVisible failed: HTTP ${resp.status} ${await resp.text()}`);
      }
      return resp.json() as Promise<{ access_token: string; refresh_token: string }>;
    };
    await use(fn);
  },

  // TMAIL-405: exposed so specs that call the raw /api/auth/signup endpoint
  // directly (e.g. folder-messagelist.spec.ts's beforeAll) can still benefit
  // from the tour-seen pre-mark without going through apiSignup.
  markTourSeen: async ({ baseURL }, use) => {
    const fn = async (accessToken: string) => {
      await markFirstLoginTourSeenViaApi(baseURL, accessToken);
    };
    await use(fn);
  },
});

export { expect } from '@playwright/test';
