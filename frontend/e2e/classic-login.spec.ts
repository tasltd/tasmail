// TMAIL-359 — E2E spec for the /classic no-JS login form.
//
// Exercises the live backend through the same Apache → SSH-tunnel → Rust
// router stack a real user hits. We deliberately do NOT mock anything here;
// the test bed uses the dev tunnel by default and PLAYWRIGHT_BASE_URL can
// override for local runs.
//
// Coverage matches the gap-analysis acceptance criteria (P0 #5):
//   * GET /classic/login renders an accessible form with a pre-session CSRF
//     cookie and the matching hidden _csrf input.
//   * POST /classic/login with a missing pre-session cookie re-renders the
//     form with the CSRF-specific error message.
//   * POST /classic/login with bad credentials re-renders with the generic
//     "incorrect email or password" copy and rotates the CSRF token.
//   * Screenshots: page load, submitted form, CSRF rejection, bad-cred
//     rejection.

import { test, expect } from './fixtures/base';
import path from 'path';
import { fileURLToPath } from 'url';

const __filename = fileURLToPath(import.meta.url);
const __dirname = path.dirname(__filename);
const SCREENSHOT_DIR = path.join(__dirname, 'screenshots', 'classic-login');

// Bogus account so the bad-credential path is exercised without affecting
// real mailboxes. The login endpoint does NOT enumerate — the response is
// identical whether the email exists or not.
const FAKE_EMAIL = 'this-user-does-not-exist-tmail359@example.invalid';
const FAKE_PASSWORD = 'wrong-password-tmail359';

test.describe('Classic UI Login (TMAIL-359)', () => {
  test('GET /classic/login renders the no-JS login form', async ({ page }) => {
    const resp = await page.goto('/classic/login');
    expect(resp?.status()).toBe(200);

    // Form scaffold.
    await expect(page.locator('form[action="/classic/login"]')).toBeVisible();
    await expect(page.locator('input[name="email"]')).toBeVisible();
    await expect(page.locator('input[name="password"]')).toBeVisible();
    await expect(page.locator('input[name="_csrf"]')).toHaveCount(1);
    await expect(page.locator('button[type="submit"]')).toBeVisible();

    // Accessible base layout (TMAIL-356 inheritance).
    await expect(page.locator('a.skip-link')).toBeAttached();
    await expect(page.locator('main#main')).toBeVisible();

    // No script tags (no-JS surface).
    const scriptCount = await page.locator('script').count();
    expect(scriptCount).toBe(0);

    await page.screenshot({
      path: path.join(SCREENSHOT_DIR, 'login-page-loaded.png'),
      fullPage: true,
    });
  });

  test('GET /classic/login sets a pre-session CSRF cookie', async ({ page, context }) => {
    await page.goto('/classic/login');
    const cookies = await context.cookies();
    const csrfCookie = cookies.find((c) => c.name === 'tasmail_classic_login_csrf');
    expect(csrfCookie, 'pre-session CSRF cookie must be set on GET').toBeDefined();
    expect(csrfCookie!.value, 'cookie value non-empty').toBeTruthy();
    expect(csrfCookie!.httpOnly).toBe(true);
    // The token in the cookie MUST match the hidden form input — that's
    // the double-submit-cookie invariant.
    const formToken = await page.locator('input[name="_csrf"]').getAttribute('value');
    expect(formToken, 'form must carry a _csrf token').toBeTruthy();
    expect(formToken).toBe(csrfCookie!.value);

    await page.screenshot({
      path: path.join(SCREENSHOT_DIR, 'login-page-with-csrf-cookie.png'),
      fullPage: true,
    });
  });

  test('POST with bad credentials re-renders form with generic error', async ({ page }) => {
    await page.goto('/classic/login');

    // Fill + submit with wrong credentials.
    await page.locator('input[name="email"]').fill(FAKE_EMAIL);
    await page.locator('input[name="password"]').fill(FAKE_PASSWORD);

    await page.screenshot({
      path: path.join(SCREENSHOT_DIR, 'login-form-filled.png'),
      fullPage: true,
    });

    // Submit the form — Playwright waits for navigation.
    await Promise.all([
      page.waitForLoadState('domcontentloaded'),
      page.locator('button[type="submit"]').click(),
    ]);

    // We should land back on the same URL with the form re-rendered.
    expect(page.url()).toContain('/classic/login');

    // Error alert visible with the generic copy. Must NOT mention "locked",
    // account existence, or anything that would leak the lookup result.
    const alert = page.locator('[role="alert"]');
    await expect(alert).toBeVisible();
    const alertText = (await alert.textContent())?.toLowerCase() ?? '';
    expect(alertText).toContain('incorrect email or password');
    expect(alertText).not.toContain('locked');
    expect(alertText).not.toContain('does not exist');

    // The submitted email must round-trip into the form so the user
    // doesn't have to retype.
    await expect(page.locator('input[name="email"]')).toHaveValue(FAKE_EMAIL);

    await page.screenshot({
      path: path.join(SCREENSHOT_DIR, 'login-bad-credentials-rejected.png'),
      fullPage: true,
    });
  });

  test('POST with stripped CSRF cookie shows distinct CSRF error', async ({ page, context }) => {
    await page.goto('/classic/login');
    // Pull the form token BEFORE clearing the cookie so the form input
    // still carries something to submit.
    const formToken = await page.locator('input[name="_csrf"]').getAttribute('value');
    expect(formToken).toBeTruthy();

    // Simulate an extension that strips the cookie.
    await context.clearCookies({ name: 'tasmail_classic_login_csrf' });

    await page.locator('input[name="email"]').fill(FAKE_EMAIL);
    await page.locator('input[name="password"]').fill(FAKE_PASSWORD);

    await Promise.all([
      page.waitForLoadState('domcontentloaded'),
      page.locator('button[type="submit"]').click(),
    ]);

    expect(page.url()).toContain('/classic/login');

    const alert = page.locator('[role="alert"]');
    await expect(alert).toBeVisible();
    const alertText = (await alert.textContent())?.toLowerCase() ?? '';
    // Distinct error message from the credential path so a user whose
    // cookie was stripped by an extension can self-diagnose.
    expect(alertText).toContain('session expired');

    await page.screenshot({
      path: path.join(SCREENSHOT_DIR, 'login-csrf-cookie-stripped.png'),
      fullPage: true,
    });
  });

  test('failed POST rotates the pre-session CSRF cookie', async ({ page, context }) => {
    await page.goto('/classic/login');
    const before = (await context.cookies()).find((c) => c.name === 'tasmail_classic_login_csrf');
    expect(before).toBeDefined();

    await page.locator('input[name="email"]').fill(FAKE_EMAIL);
    await page.locator('input[name="password"]').fill(FAKE_PASSWORD);
    await Promise.all([
      page.waitForLoadState('domcontentloaded'),
      page.locator('button[type="submit"]').click(),
    ]);

    const after = (await context.cookies()).find((c) => c.name === 'tasmail_classic_login_csrf');
    expect(after).toBeDefined();
    expect(after!.value, 'CSRF cookie must rotate after a failed POST so a replay is impossible')
      .not.toBe(before!.value);

    await page.screenshot({
      path: path.join(SCREENSHOT_DIR, 'login-csrf-cookie-rotated.png'),
      fullPage: true,
    });
  });
});
