// TMAIL-361 — E2E spec for the /classic/login/2fa TOTP challenge gate.
//
// Coverage matches the gap-analysis P0 #7 acceptance criteria:
//   * GET /classic/login/2fa without a pending cookie bounces to
//     /classic/login?error=2fa_expired AND the login page flashes the
//     "verification session expired" alert in a role="alert" block.
//   * GET /classic/login/2fa with a forged signature bounces the same way
//     (defends against UUID-guess + signature-swap attacks).
//   * POST /classic/login/2fa without a pending cookie bounces.
//   * GET /classic/login?error=2fa_too_many flashes the "too many incorrect
//     codes" alert.
//   * GET /classic/login?error=<unrecognised> renders NO flash (whitelist
//     defence — anything we don't recognise is dropped).
//
// We deliberately do NOT exercise the success path (real code accepted) here
// — that requires a real TOTP-enrolled mailbox + working TOTP secret. The
// backend integration tests + unit tests pin those branches; this spec is
// the visual / user-flow proof for the gate / bounce paths.
//
// Per the project's E2E navigation rule, direct page.goto() calls into
// /classic/login and its variants are acceptable here because:
//   * /classic/login is the canonical entry point (initial login URL
//     exception).
//   * /classic/login/2fa is reached BY a redirect from /classic/login
//     after a TOTP-enrolled password check — the test bed can't drive
//     that without a real user, so we exercise the gate via the bounce
//     paths that don't require a session.

import { test, expect } from './fixtures/base';
import path from 'path';
import { fileURLToPath } from 'url';

const __filename = fileURLToPath(import.meta.url);
const __dirname = path.dirname(__filename);
const SCREENSHOT_DIR = path.join(__dirname, 'screenshots', 'classic-2fa-challenge');

test.describe('Classic UI 2FA Challenge (TMAIL-361)', () => {
  test('GET /classic/login/2fa without cookie bounces to login with expired flash', async ({ page }) => {
    // Direct navigation without the pending cookie — the handler must
    // 303-redirect to /classic/login?error=2fa_expired. Playwright will
    // follow the redirect by default; we end up on the login page.
    await page.goto('/classic/login/2fa');

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

  test('GET /classic/login/2fa with forged signature bounces same way', async ({ page, context }) => {
    // Plant a well-shaped cookie (uuid.sig) with a junk signature. The
    // HMAC verify must fail and the user must land on the same bounce
    // as the missing-cookie case — no oracle for "UUID exists vs not".
    await context.addCookies([
      {
        name: 'tasmail_classic_pending_2fa',
        value: 'deadbeefdeadbeefdeadbeefdeadbeef.forgedsig123',
        domain: new URL(page.url() || 'https://mail.techatscale.io').hostname,
        path: '/classic/login',
        httpOnly: false, // can't be HttpOnly when set via Playwright
        secure: true,
        sameSite: 'Strict',
      },
    ]);

    await page.goto('/classic/login/2fa');

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

  test('GET /classic/login?error=2fa_too_many flashes too-many-codes copy', async ({ page }) => {
    // The bounce path used after MAX_FAILED_ATTEMPTS wrong-code submissions.
    // We exercise the FLASH directly because triggering it for real would
    // require a TOTP-enrolled live mailbox and five wrong-code submissions.
    await page.goto('/classic/login?error=2fa_too_many');

    const alert = page.locator('[role="alert"]');
    await expect(alert).toBeVisible();
    const text = (await alert.textContent())?.toLowerCase() ?? '';
    expect(text).toContain('too many incorrect');
    expect(text).toContain('verification codes');
    // Must NOT leak account state (per gap analysis acceptance criteria).
    expect(text).not.toContain('account');
    expect(text).not.toContain('locked');

    // Login form still rendered so the user can re-enter credentials.
    await expect(page.locator('form[action="/classic/login"]')).toBeVisible();

    await page.screenshot({
      path: path.join(SCREENSHOT_DIR, '03-flash-too-many-codes.png'),
      fullPage: true,
    });
  });

  test('GET /classic/login?error=<unrecognised> renders no flash (whitelist)', async ({ page }) => {
    // Whitelist defence: an attacker who manages to land an attacker-
    // controlled query string in front of the user must not be able to
    // surface arbitrary text on the login page. The mapping is strict
    // and unknown values are silently dropped.
    await page.goto('/classic/login?error=%3Cscript%3Ealert(1)%3C/script%3E');

    // No alert visible.
    const alertCount = await page.locator('[role="alert"]').count();
    expect(alertCount).toBe(0);

    // Defence in depth: even if the alert HAD rendered, Askama's
    // auto-escape keeps script content inert.
    const html = await page.content();
    expect(html).not.toContain('<script>alert(1)</script>');

    await page.screenshot({
      path: path.join(SCREENSHOT_DIR, '04-unknown-error-no-flash.png'),
      fullPage: true,
    });
  });

  test('POST /classic/login/2fa without cookie bounces to login', async ({ request }) => {
    // Verify the POST half of the route is wired too — a state-changing
    // submission without a pending cookie must NOT 200 (which would
    // imply the route is missing and got matched by a wildcard) and
    // must NOT 500. We expect a 303 redirect to /classic/login.
    //
    // Disable redirect following so we can inspect the 303 + Set-Cookie
    // headers directly.
    const resp = await request.post('/classic/login/2fa', {
      headers: { 'Content-Type': 'application/x-www-form-urlencoded' },
      data: '_csrf=anything&code=123456',
      maxRedirects: 0,
      failOnStatusCode: false,
    });

    expect(resp.status()).toBe(303);
    const location = resp.headers()['location'];
    expect(location).toMatch(/\/classic\/login(\?|$)/);
    expect(location).toContain('error=2fa_expired');

    // Set-Cookie must include a Max-Age=0 clear for the pending cookie.
    // Playwright APIResponse exposes set-cookie under headers (it may be
    // joined or returned as an array depending on the transport — accept
    // both shapes).
    const setCookieRaw = resp.headers()['set-cookie'] ?? '';
    const setCookie = Array.isArray(setCookieRaw) ? setCookieRaw.join('; ') : setCookieRaw;
    expect(setCookie).toContain('tasmail_classic_pending_2fa=');
    expect(setCookie).toContain('Max-Age=0');
  });
});
