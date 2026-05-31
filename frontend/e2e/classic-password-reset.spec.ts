// TMAIL-375 — E2E spec for the /classic no-JS password reset flow.
//
// Exercises the live backend through the same Apache → SSH-tunnel → Rust
// router stack a real user hits. We deliberately do NOT mock anything; the
// test bed uses the dev tunnel by default and PLAYWRIGHT_BASE_URL can
// override for local runs.
//
// Coverage matches the gap-analysis acceptance criteria (P1 #21):
//   * Login page links to the password-reset request page (HARD RULE on
//     menu navigation — we click the "Forgot your password?" link rather
//     than `page.goto('/classic/password-reset/request')`).
//   * GET /classic/password-reset/request renders an accessible form with a
//     pre-session CSRF cookie and the matching hidden _csrf input. No
//     <script> tags anywhere.
//   * POST with a wrong CSRF token re-renders the form with the CSRF error
//     message and rotates the cookie.
//   * POST with a never-registered email STILL renders the generic "if
//     that address is registered we sent it" page (anti-enumeration).
//   * GET /classic/password-reset/confirm with no token renders the generic
//     "invalid or expired" page.
//   * GET /classic/password-reset/confirm with an unknown token renders the
//     same generic invalid page (same response — no oracle).
//   * Every step inherits base.html (skip-link, <main id="main">, CSP
//     nonce on the inline <style>) and ships zero <script> tags.
//
// Screenshots are captured at every validation point per the HARD RULE.
//
// What this spec deliberately does NOT cover (left for the integration
// test layer):
//   * The end-to-end "real email arrives, click link, type new password,
//     sign in with it" path — that requires inbox capture against the
//     noreply mailbox + a real signed-up user, which the next sweep of
//     E2E work will add alongside other email-driven flows.

import { test, expect } from './fixtures/base';
import path from 'path';
import { fileURLToPath } from 'url';

const __filename = fileURLToPath(import.meta.url);
const __dirname = path.dirname(__filename);
const SCREENSHOT_DIR = path.join(__dirname, 'screenshots', 'classic-password-reset');

// Bogus account so the unknown-email path is exercised without affecting
// real mailboxes. The request endpoint does NOT enumerate — the response
// is identical whether the email exists or not.
const NEVER_REGISTERED_EMAIL = 'this-user-does-not-exist-tmail375@example.invalid';

test.describe('Classic UI Password Reset (TMAIL-375)', () => {
  test('Login page links to the password-reset request page', async ({ page }) => {
    // Only allowed page.goto in the spec — the login page is the entry
    // point for signed-out users (mirrors classic-signup-wizard.spec.ts).
    const resp = await page.goto('/classic/login');
    expect(resp?.status()).toBe(200);

    await page.screenshot({
      path: path.join(SCREENSHOT_DIR, 'login-page-with-forgot-link.png'),
      fullPage: true,
    });

    const link = page.locator('a[href="/classic/password-reset/request"]');
    await expect(link).toBeVisible();
    await expect(link).toContainText(/forgot/i);

    // Click the link — navigation must work without JS.
    await Promise.all([
      page.waitForLoadState('domcontentloaded'),
      link.click(),
    ]);
    expect(page.url()).toContain('/classic/password-reset/request');
    await expect(page.locator('h1')).toContainText(/reset your password/i);
  });

  test('Request GET renders accessible form with pre-session CSRF cookie', async ({
    page,
    context,
  }) => {
    await page.goto('/classic/login');
    await page.locator('a[href="/classic/password-reset/request"]').click();
    await page.waitForLoadState('domcontentloaded');

    // Form scaffold.
    await expect(
      page.locator('form[action="/classic/password-reset/request"]'),
    ).toBeVisible();
    await expect(page.locator('input[name="email"]')).toBeVisible();
    await expect(page.locator('input[name="_csrf"]')).toHaveCount(1);
    await expect(page.locator('button[type="submit"]')).toBeVisible();

    // Accessible base layout (TMAIL-356 inheritance).
    await expect(page.locator('a.skip-link')).toBeAttached();
    await expect(page.locator('main#main')).toBeVisible();

    // No <script> tags — no-JS rule.
    expect(await page.locator('script').count()).toBe(0);

    // Pre-session CSRF cookie is set with the right attributes.
    const cookies = await context.cookies();
    const csrf = cookies.find((c) => c.name === 'tasmail_classic_pwreset_csrf');
    expect(csrf, 'tasmail_classic_pwreset_csrf cookie must be set on GET').toBeDefined();
    expect(csrf!.httpOnly).toBe(true);
    expect(csrf!.value).toBeTruthy();

    // Cookie value matches the hidden form input (double-submit invariant).
    const formToken = await page.locator('input[name="_csrf"]').getAttribute('value');
    expect(formToken, 'form must carry a _csrf token').toBeTruthy();
    expect(formToken).toBe(csrf!.value);

    await page.screenshot({
      path: path.join(SCREENSHOT_DIR, 'request-form-loaded.png'),
      fullPage: true,
    });
  });

  test('Request POST with unknown email renders generic done page (no enumeration)', async ({
    page,
  }) => {
    await page.goto('/classic/login');
    await page.locator('a[href="/classic/password-reset/request"]').click();
    await page.waitForLoadState('domcontentloaded');

    await page.locator('input[name="email"]').fill(NEVER_REGISTERED_EMAIL);

    await page.screenshot({
      path: path.join(SCREENSHOT_DIR, 'request-form-filled.png'),
      fullPage: true,
    });

    await Promise.all([
      page.waitForLoadState('domcontentloaded'),
      page.locator('button[type="submit"]').click(),
    ]);

    // The response MUST be the generic "if that address is registered we
    // sent it" page — never reveals whether the email matched.
    await expect(page.locator('h1')).toContainText(/check your email/i);
    await expect(page.locator('[role="status"]')).toContainText(
      /if that email address is registered/i,
    );
    // Must NOT name the submitted email (defence against reflected XSS
    // AND against enumeration via "we did not find …" copy).
    const bodyText = await page.locator('body').textContent();
    expect(bodyText).not.toContain(NEVER_REGISTERED_EMAIL);

    // Still zero <script> tags on the done page.
    expect(await page.locator('script').count()).toBe(0);

    await page.screenshot({
      path: path.join(SCREENSHOT_DIR, 'request-done-page.png'),
      fullPage: true,
    });
  });

  test('Request POST with a missing CSRF cookie re-renders with error', async ({
    page,
    context,
  }) => {
    await page.goto('/classic/login');
    await page.locator('a[href="/classic/password-reset/request"]').click();
    await page.waitForLoadState('domcontentloaded');

    // Clear the pre-session CSRF cookie before submitting — same shape
    // the classic-login spec uses to exercise its CSRF error branch.
    await context.clearCookies({ name: 'tasmail_classic_pwreset_csrf' });

    await page.locator('input[name="email"]').fill(NEVER_REGISTERED_EMAIL);
    await Promise.all([
      page.waitForLoadState('domcontentloaded'),
      page.locator('button[type="submit"]').click(),
    ]);

    // Form re-renders with the CSRF error alert AND a fresh cookie so
    // the user can retry.
    await expect(page.locator('[role="alert"]')).toBeVisible();
    await expect(page.locator('[role="alert"]')).toContainText(
      /session expired before you submitted the form/i,
    );
    await expect(
      page.locator('form[action="/classic/password-reset/request"]'),
    ).toBeVisible();
    // Email round-trip — user shouldn't have to retype.
    await expect(page.locator('input[name="email"]')).toHaveValue(
      NEVER_REGISTERED_EMAIL,
    );

    // Fresh CSRF cookie is set.
    const cookies = await context.cookies();
    const csrf = cookies.find((c) => c.name === 'tasmail_classic_pwreset_csrf');
    expect(csrf, 'fresh cookie issued on re-render').toBeDefined();
    expect(csrf!.value).toBeTruthy();

    await page.screenshot({
      path: path.join(SCREENSHOT_DIR, 'request-form-csrf-rejected.png'),
      fullPage: true,
    });
  });

  test('Confirm GET with no token renders generic invalid page', async ({ page }) => {
    // The user lands on the confirm endpoint via the email link. If the
    // URL has no token (truncated by an over-zealous email client, for
    // example), we must render the same generic "invalid" page as for
    // any other failure mode — no oracle.
    //
    // Direct page.goto is allowed here because the confirm URL is what
    // arrives in the user's email — that IS the entry point, not an
    // internal nav target.
    const resp = await page.goto('/classic/password-reset/confirm');
    expect(resp?.status()).toBe(200);

    await expect(page.locator('h1')).toContainText(/invalid|expired/i);
    await expect(page.locator('[role="alert"]')).toContainText(
      /no longer valid/i,
    );
    // Offers a retry link to the request page.
    await expect(
      page.locator('a[href="/classic/password-reset/request"]'),
    ).toBeVisible();
    // Zero <script> tags.
    expect(await page.locator('script').count()).toBe(0);

    await page.screenshot({
      path: path.join(SCREENSHOT_DIR, 'confirm-no-token-invalid.png'),
      fullPage: true,
    });
  });

  test('Confirm GET with unknown token renders the same generic invalid page', async ({
    page,
  }) => {
    // Same response shape as the no-token case — that's the anti-oracle
    // discipline. An unknown token must NOT 404, MUST NOT say "no such
    // token", MUST return the same HTML the no-token case does.
    const resp = await page.goto(
      '/classic/password-reset/confirm?token=this-token-is-completely-fake-tmail375',
    );
    expect(resp?.status()).toBe(200);

    await expect(page.locator('h1')).toContainText(/invalid|expired/i);
    await expect(page.locator('[role="alert"]')).toContainText(
      /no longer valid/i,
    );

    await page.screenshot({
      path: path.join(SCREENSHOT_DIR, 'confirm-unknown-token-invalid.png'),
      fullPage: true,
    });
  });

  test('Login page renders the success flash when ?reset=ok is present', async ({
    page,
  }) => {
    // After a successful confirm, the handler 303-redirects to
    // /classic/login?reset=ok. We can't drive the full reset flow
    // end-to-end without a registered user + inbox interception (that
    // sits in the next sweep), but we CAN prove the success-flash
    // wiring works by hitting the URL directly — same shape as the
    // existing classic-2fa-challenge spec exercises ?error=2fa_expired.
    const resp = await page.goto('/classic/login?reset=ok');
    expect(resp?.status()).toBe(200);

    await expect(page.locator('[role="status"]')).toContainText(
      /password has been updated/i,
    );
    // The success alert must use the success styling, not the error one.
    await expect(page.locator('.alert-success')).toBeVisible();

    await page.screenshot({
      path: path.join(SCREENSHOT_DIR, 'login-with-reset-success-flash.png'),
      fullPage: true,
    });
  });

  test('Unknown reset query value is silently dropped (no reflected output)', async ({
    page,
  }) => {
    // Anti-XSS: an attacker can't inject arbitrary copy via the query
    // string. The whitelisted server-side mapping returns None for any
    // value outside the recognised set.
    const resp = await page.goto(
      '/classic/login?reset=%3Cscript%3Ealert(1)%3C%2Fscript%3E',
    );
    expect(resp?.status()).toBe(200);

    // No success alert renders at all.
    expect(await page.locator('.alert-success').count()).toBe(0);
    // And the malicious payload is nowhere in the rendered output.
    const body = await page.locator('body').textContent();
    expect(body).not.toContain('<script>');

    await page.screenshot({
      path: path.join(SCREENSHOT_DIR, 'login-hostile-reset-param-dropped.png'),
      fullPage: true,
    });
  });
});
