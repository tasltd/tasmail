// TMAIL-381 — E2E spec for the /classic/login/2fa/sms SMS-OTP challenge gate.
//
// Coverage matches the gap-analysis P1 #27 acceptance criteria:
//   * GET /classic/login/2fa/sms without a pending cookie bounces to
//     /classic/login?error=2fa_expired AND the login page flashes the
//     "verification session expired" alert in a role="alert" block.
//   * GET /classic/login/2fa/sms with a forged signature bounces the same
//     way (defends against UUID-guess + signature-swap attacks).
//   * POST /classic/login/2fa/sms without a pending cookie bounces.
//   * POST /classic/login/2fa/sms with action=resend without a pending
//     cookie also bounces (same shape — both branches of the dispatcher
//     share the cookie-resolution preamble).
//
// We deliberately do NOT exercise the success path (real code accepted)
// here — that requires a real SMS-OTP-enrolled mailbox AND a working SMS
// provider (or TASMAIL_SMS_TEST_MODE on the backend) plus the matching
// pending_2fa_tokens row. The backend unit tests pin the verify / resend
// branches; this spec is the visual / user-flow proof for the gate /
// bounce paths.
//
// Per the project's E2E navigation rule, direct page.goto() calls into
// /classic/login/2fa/sms are acceptable here because:
//   * /classic/login is the canonical entry point (initial login URL
//     exception).
//   * /classic/login/2fa/sms is reached BY a redirect from /classic/login
//     after an SMS-OTP-enrolled password check — the test bed can't drive
//     that without a real user with sms_otp_enabled=true, so we exercise
//     the gate via the bounce paths that don't require a session.

import { test, expect } from './fixtures/base';
import path from 'path';
import { fileURLToPath } from 'url';

const __filename = fileURLToPath(import.meta.url);
const __dirname = path.dirname(__filename);
const SCREENSHOT_DIR = path.join(__dirname, 'screenshots', 'classic-2fa-sms-challenge');

test.describe('Classic UI SMS-OTP 2FA Challenge (TMAIL-381)', () => {
  test('GET /classic/login/2fa/sms without cookie bounces to login with expired flash', async ({ page }) => {
    // Direct navigation without the pending cookie — the handler must
    // 303-redirect to /classic/login?error=2fa_expired. Playwright will
    // follow the redirect by default; we end up on the login page.
    await page.goto('/classic/login/2fa/sms');

    // After the redirect we should be on /classic/login with the flash
    // visible. The URL retains the query param so a refresh re-renders
    // the same flash.
    await expect(page).toHaveURL(/\/classic\/login\?error=2fa_expired/);

    // The role="alert" carries the expired-session copy.
    const alert = page.locator('[role="alert"]');
    await expect(alert).toBeVisible();
    const text = (await alert.textContent())?.toLowerCase() ?? '';
    expect(text).toContain('verification session expired');

    // The login form is still rendered so the user can re-enter creds.
    await expect(page.locator('form[action="/classic/login"]')).toBeVisible();
    await expect(page.locator('input[name="email"]')).toBeVisible();
    await expect(page.locator('input[name="password"]')).toBeVisible();

    // No-JS hard rule still holds for the bounced page.
    const scriptCount = await page.locator('script').count();
    expect(scriptCount).toBe(0);

    await page.screenshot({
      path: path.join(SCREENSHOT_DIR, '01-bounce-no-cookie-to-login-expired.png'),
      fullPage: true,
    });
  });

  test('GET /classic/login/2fa/sms with forged signature bounces same way', async ({ page, context, baseURL }) => {
    // Navigate to the login page first so the page context has a real URL
    // — Playwright's `addCookies({ domain })` requires a non-empty hostname,
    // and `page.url()` before any navigation is "about:blank".
    await page.goto('/classic/login');

    // Plant a well-shaped cookie (uuid.sig) with a junk signature. The
    // HMAC verify must fail and the user must land on the same bounce
    // as the missing-cookie case — no oracle for "UUID exists vs not".
    const targetUrl = baseURL ?? page.url();
    await context.addCookies([
      {
        name: 'tasmail_classic_pending_2fa',
        value: 'deadbeefdeadbeefdeadbeefdeadbeef.forgedsig123',
        domain: new URL(targetUrl).hostname,
        path: '/classic/login',
        httpOnly: false, // can't be HttpOnly when set via Playwright
        secure: targetUrl.startsWith('https://'),
        sameSite: 'Strict',
      },
    ]);

    await page.goto('/classic/login/2fa/sms');

    await expect(page).toHaveURL(/\/classic\/login\?error=2fa_expired/);
    const alert = page.locator('[role="alert"]');
    await expect(alert).toBeVisible();
    const text = (await alert.textContent())?.toLowerCase() ?? '';
    expect(text).toContain('verification session expired');

    await page.screenshot({
      path: path.join(SCREENSHOT_DIR, '02-bounce-forged-signature.png'),
      fullPage: true,
    });
  });

  test('POST /classic/login/2fa/sms (verify) without cookie bounces to login', async ({ request }) => {
    // Verify the POST half of the route is wired and that the verify
    // dispatch branch also runs the cookie-resolution preamble. A state-
    // changing submission without a pending cookie must NOT 200 (which
    // would imply the route is missing and got matched by a wildcard)
    // and must NOT 500. We expect a 303 redirect to /classic/login.
    const resp = await request.post('/classic/login/2fa/sms', {
      headers: { 'Content-Type': 'application/x-www-form-urlencoded' },
      data: '_csrf=anything&action=verify&code=123456',
      maxRedirects: 0,
      failOnStatusCode: false,
    });

    expect(resp.status()).toBe(303);
    const location = resp.headers()['location'];
    expect(location).toMatch(/\/classic\/login(\?|$)/);
    expect(location).toContain('error=2fa_expired');

    // Set-Cookie must include a Max-Age=0 clear for the pending cookie.
    const setCookieRaw = resp.headers()['set-cookie'] ?? '';
    const setCookie = Array.isArray(setCookieRaw) ? setCookieRaw.join('; ') : setCookieRaw;
    expect(setCookie).toContain('tasmail_classic_pending_2fa=');
    expect(setCookie).toContain('Max-Age=0');
  });

  test('POST /classic/login/2fa/sms (resend) without cookie also bounces', async ({ request }) => {
    // The resend branch of the dispatcher MUST share the same cookie
    // preamble as verify — a missing cookie cannot be allowed to drop
    // through to the SMS-provider call. This nails that branch is
    // gated identically.
    const resp = await request.post('/classic/login/2fa/sms', {
      headers: { 'Content-Type': 'application/x-www-form-urlencoded' },
      data: '_csrf=anything&action=resend',
      maxRedirects: 0,
      failOnStatusCode: false,
    });

    expect(resp.status()).toBe(303);
    const location = resp.headers()['location'];
    expect(location).toMatch(/\/classic\/login(\?|$)/);
    expect(location).toContain('error=2fa_expired');

    const setCookieRaw = resp.headers()['set-cookie'] ?? '';
    const setCookie = Array.isArray(setCookieRaw) ? setCookieRaw.join('; ') : setCookieRaw;
    expect(setCookie).toContain('tasmail_classic_pending_2fa=');
    expect(setCookie).toContain('Max-Age=0');
  });
});
