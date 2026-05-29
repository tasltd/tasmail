/**
 * TMAIL-281 — Auth + Onboarding E2E sweep
 *
 * Surfaces covered:
 *   1. Landing page (`/`) — hero, CTAs route to /login + /signup
 *   2. Login page (`/login`) — pristine form, branding
 *   3. Signup page (`/signup`) — pristine, filled, mismatched-confirm rejected
 *   4. BYOK onboarding wizard (`/onboarding`) — signup lands here, provider picker
 *      visible, custom provider opens the IMAP form (full provisioning is covered
 *      separately by specs/signup-byok-flow.spec.ts so we don't re-burn the real
 *      swmail credentials here).
 *   5. Login success — fresh BYOK account signs in via the UI and reaches /app
 *      or /onboarding (both prove auth succeeded).
 *   6. Account lockout (TMAIL-273) — after the configured threshold of failed
 *      attempts the backend returns HTTP 423 Locked; even the correct password
 *      stays blocked while the lockout window is active.
 *
 * Validation strategy (per the E2E HARD RULES):
 *   - Navigate only via menu clicks / link clicks; page.goto() is used only for
 *     the initial public URLs (/, /signup, /login) which are the documented
 *     exception (no "menu" to click from outside).
 *   - Screenshots captured at every key validation point, written under
 *     e2e/screenshots/auth/ via the shared `takeScreenshot` fixture.
 *   - Mutations cross-checked against the API state:
 *       * after signup → GET /api/imap-configs returns [] (auth works, no BYOK yet)
 *       * after BYOK provisioning is OUT OF SCOPE here (covered by signup-byok-flow)
 *       * lockout → POST /api/auth/login with the correct password still 423
 *   - Per-IP auth rate-limit budget is 10 req / 60s. The lockout test alone needs
 *     1 signup + 5 wrong + 1 correct = 7 calls; we run it FIRST so the budget is
 *     fresh, and we keep the rest of the suite to ≤ 3 additional auth calls.
 *
 * NOTE: TMAIL-281's description references `GET /api/auth/me` and HTTP 429 for the
 * lockout response. Neither matches the actual backend — the project never shipped
 * an /api/auth/me route, and lockout uses HTTP 423 Locked (see error.rs and
 * services/auth_service.rs). This spec validates the real behaviour and the
 * assessment doc (docs/assessments/e2e-auth-2026-05.md) records the discrepancy.
 */
import { test, NOREPLY_CREDS as _NOREPLY_CREDS } from './fixtures/base.js';
import { expect, request as apiRequest } from '@playwright/test';
import { deleteMailboxByUsername } from './helpers/db-cleanup.js';

// Keep the import live so a future signup-with-noreply test can reuse it without a re-import churn.
void _NOREPLY_CREDS;

const ACCOUNT_PASSWORD = 'auth-sweep-Pa55word!';
const RUN_TAG = Date.now();

function freshEmail(label: string): string {
  const suffix = Math.floor(Math.random() * 9_999_999).toString(36);
  return `e2e-auth-${label}-${RUN_TAG}-${suffix}@e2e.tasmail`;
}

// Track every mailbox we sign up so the afterAll sweep can purge them from the
// live DB. Without this the unique-email constraint would slowly leak rows.
const CREATED_USERNAMES: string[] = [];

// Lockout test was running first because of the per-IP rate-limit budget;
// `mode: 'serial'` keeps Playwright from re-ordering tests when running with
// PLAYWRIGHT_PARALLEL=true.
test.describe.configure({ mode: 'serial' });

// The auth router rate-limit window. The live backend defaults to 10 req/60s
// per IP (see backend/src/router.rs::auth_rl_window / AUTH_RATE_LIMIT_WINDOW).
// Override with E2E_AUTH_RL_WINDOW_MS=0 for local dev backends where the limit
// is relaxed; default keeps CI reliable against mail.techatscale.io.
const AUTH_RL_WINDOW_MS = Number(process.env.E2E_AUTH_RL_WINDOW_MS ?? 65_000);

async function afterAllCleanup() {
  for (const username of CREATED_USERNAMES) {
    try {
      deleteMailboxByUsername(username);
    } catch {
      // Best-effort — if the local DB isn't reachable, the next run's
      // signup will still succeed because each email embeds RUN_TAG.
    }
  }
}

// Two describes so we can drain the auth rate-limit window between them.
// Behind a live Apache reverse-proxy + SSH tunnel every request lands on
// 127.0.0.1 as far as the backend's per-IP RateLimiter is concerned, so all
// auth calls in this suite share one bucket. The lockout test alone burns ~8
// of the 10 slots; running the rest immediately would 429.
test.describe('TMAIL-281 Auth + Onboarding sweep — lockout (burns the auth-RL budget)', () => {
  test.afterAll(afterAllCleanup);

  // ------------------------------------------------------------------
  // 1) Lockout first — it needs the freshest rate-limit budget.
  // ------------------------------------------------------------------
  test('account lockout: 5 failed attempts trigger 423, correct password stays blocked', async ({
    page,
    takeScreenshot,
    baseURL,
  }) => {
    const email = freshEmail('lockout-victim');
    CREATED_USERNAMES.push(email);

    const api = await apiRequest.newContext({ baseURL });

    // Brand-new account.
    const signup = await api.post('/api/auth/signup', {
      data: { email, password: ACCOUNT_PASSWORD },
    });
    expect(signup.status(), 'signup should succeed (201/200)').toBeLessThan(300);

    // First 4 wrong attempts via the API — fast, deterministic, no rendering cost.
    // The 5th is intentionally driven through the UI so we get a screenshot of
    // the lockout banner that real users see.
    for (let attempt = 1; attempt <= 4; attempt++) {
      const r = await api.post('/api/auth/login', {
        data: { username: email, password: 'definitely-wrong' },
      });
      expect(r.status(), `wrong-password attempt ${attempt} should be 401`).toBe(401);
    }

    // 5th wrong attempt via the UI — this is the one that crosses the threshold
    // and flips the account into the LOCKED state (HTTP 423).
    await page.goto('/login');
    await page.waitForSelector('#username');
    await page.fill('#username', email);
    await page.fill('#password', 'definitely-wrong');
    await takeScreenshot(page, 'auth/lockout-01-form-before-final-fail');
    await page.click('button[type="submit"]');

    const errBanner = page.locator('.login-card__error');
    await expect(errBanner).toBeVisible({ timeout: 10_000 });
    await expect(errBanner).toHaveText(/Account temporarily locked\. Try again later\./);
    await takeScreenshot(page, 'auth/lockout-02-banner-after-threshold');

    // Now try the CORRECT password — backend still returns 423 because the
    // lockout window is open. The frontend maps 423 → the same generic banner.
    await page.fill('#password', ACCOUNT_PASSWORD);
    await page.click('button[type="submit"]');
    await expect(errBanner).toHaveText(/Account temporarily locked\. Try again later\./);
    await takeScreenshot(page, 'auth/lockout-03-correct-password-still-blocked');

    // Cross-check via the API: confirm /api/auth/login is returning 423, not 401.
    const lockedResp = await api.post('/api/auth/login', {
      data: { username: email, password: ACCOUNT_PASSWORD },
    });
    expect(lockedResp.status(), 'login with correct password during lockout window').toBe(423);

    // And confirm the locked-out account can't reach a protected route either:
    // there's no valid session, so /api/imap-configs without a token still 401s.
    const protectedNoToken = await api.get('/api/imap-configs');
    expect(protectedNoToken.status()).toBe(401);
  });
});

test.describe('TMAIL-281 Auth + Onboarding sweep — UI surfaces (uses fresh auth-RL window)', () => {
  // Drain the shared per-IP auth rate-limit window before this block — the
  // lockout test above used most of the previous window's 10 slots. The default
  // beforeAll hook timeout is 30s; bump it to fit the rate-limit window.
  test.beforeAll(async () => {
    test.setTimeout(AUTH_RL_WINDOW_MS + 30_000);
    if (AUTH_RL_WINDOW_MS > 0) {
      await new Promise((resolve) => setTimeout(resolve, AUTH_RL_WINDOW_MS));
    }
  });
  test.afterAll(afterAllCleanup);

  // ------------------------------------------------------------------
  // 2) Landing page — hero, badge, both CTAs route correctly.
  // ------------------------------------------------------------------
  test('landing page renders hero + routes "Get started" to /signup and "Sign in" to /login', async ({
    page,
    takeScreenshot,
  }) => {
    await page.goto('/');
    await page.waitForLoadState('networkidle');

    await expect(page.locator('.landing-hero__title')).toBeVisible();
    await expect(page.locator('.landing-hero__title')).toContainText('webmail UI');
    await expect(page.locator('.landing-hero__badge')).toContainText('Bring your own IMAP');
    await takeScreenshot(page, 'auth/landing-01-hero');

    // CTA → /signup.
    await page
      .locator('a.landing-btn--primary', { hasText: 'Create your account' })
      .first()
      .click();
    await page.waitForURL(/\/signup$/);
    await expect(page.locator('#email')).toBeVisible();
    await takeScreenshot(page, 'auth/landing-02-cta-signup');

    // Go back to landing, then the "Sign in" CTA → /login.
    await page.goto('/');
    await page
      .locator('a.landing-btn--ghost', { hasText: 'Sign in' })
      .first()
      .click();
    await page.waitForURL(/\/login$/);
    await expect(page.locator('#username')).toBeVisible();
    await takeScreenshot(page, 'auth/landing-03-cta-login');
  });

  // ------------------------------------------------------------------
  // 3) Login page pristine.
  // ------------------------------------------------------------------
  test('login page renders branded form with TASMail header', async ({ page, takeScreenshot }) => {
    await page.goto('/login');
    await page.waitForSelector('#username');
    await expect(page.locator('.login-card__header h1')).toHaveText('TASMail');
    await expect(page.locator('.login-card__header p')).toHaveText('Webmail for any IMAP/SMTP server');
    await expect(page.locator('#password')).toBeVisible();
    await expect(page.locator('button[type="submit"]')).toHaveText('Sign In');
    await takeScreenshot(page, 'auth/login-01-pristine');
  });

  // ------------------------------------------------------------------
  // 4) Signup page pristine + filled + mismatch.
  // ------------------------------------------------------------------
  test('signup page: pristine → filled → mismatched-confirm shows inline error', async ({
    page,
    takeScreenshot,
  }) => {
    await page.goto('/signup');
    await page.waitForSelector('#email');
    await expect(page.locator('.login-card__header h1')).toHaveText('TASMail');
    await takeScreenshot(page, 'auth/signup-01-pristine');

    await page.fill('#email', freshEmail('mismatch'));
    await page.fill('#display_name', 'E2E Sweep User');
    await page.fill('#password', 'one-good-password-12');
    await page.fill('#confirm', 'a-different-password-13');
    await takeScreenshot(page, 'auth/signup-02-filled-with-mismatch');

    await page.click('button[type="submit"]');
    const error = page.locator('.login-card__error');
    await expect(error).toBeVisible();
    await expect(error).toContainText('do not match');
    await takeScreenshot(page, 'auth/signup-03-mismatch-error');

    // The browser should still be on /signup — submit was rejected client-side
    // before any /api/auth/signup call went out.
    await expect(page).toHaveURL(/\/signup$/);
  });

  // ------------------------------------------------------------------
  // 5) Signup happy path → lands on /onboarding.
  //    API cross-check: GET /api/imap-configs with the just-issued token returns [].
  //    (TMAIL-281 mentions /api/auth/me, which the backend never shipped —
  //    we use /api/imap-configs as the equivalent token-validating call.)
  // ------------------------------------------------------------------
  test('signup form submits successfully and lands on the onboarding wizard', async ({
    page,
    signupAs,
    takeScreenshot,
    baseURL,
  }) => {
    const email = freshEmail('signup-happy');
    CREATED_USERNAMES.push(email);

    await signupAs(page, email, ACCOUNT_PASSWORD);
    await expect(page).toHaveURL(/\/onboarding/);
    // Wait for the wizard to finish its initial preset + feature-flag fetch so
    // the screenshot captures the real first step instead of the "Loading…"
    // placeholder. Either the path picker (DNS-MX feature on) or the provider
    // picker (BYOK only) is the first stable heading.
    await expect(
      page.locator(
        'h2:has-text("Who hosts your email?"), h2:has-text("How do you want to use TASMail?")',
      ),
    ).toBeVisible({ timeout: 10_000 });
    await takeScreenshot(page, 'auth/signup-04-onboarding-landed');

    // Pull the access_token the SPA just stored, confirm the API accepts it
    // and that no IMAP config exists yet for the brand-new account.
    const accessToken = await page.evaluate(() => localStorage.getItem('access_token'));
    expect(accessToken, 'SPA should have stored an access_token after signup').toBeTruthy();

    const api = await apiRequest.newContext({ baseURL });
    const configs = await api.get('/api/imap-configs', {
      headers: { Authorization: `Bearer ${accessToken!}` },
    });
    expect(configs.status()).toBe(200);
    const rows = (await configs.json()) as unknown[];
    expect(Array.isArray(rows), '/api/imap-configs should return an array').toBe(true);
    expect(rows.length, 'brand-new account has zero BYOK rows').toBe(0);
  });

  // ------------------------------------------------------------------
  // 6) Onboarding wizard step navigation (no provisioning — covered by
  //    signup-byok-flow.spec.ts). We just verify the wizard renders each
  //    step correctly for the screenshot inspection log.
  // ------------------------------------------------------------------
  test('onboarding wizard exposes provider picker and IMAP step', async ({
    page,
    signupAs,
    takeScreenshot,
  }) => {
    const email = freshEmail('wizard-steps');
    CREATED_USERNAMES.push(email);
    await signupAs(page, email, ACCOUNT_PASSWORD);
    await expect(page).toHaveURL(/\/onboarding/);

    // The wizard may show a "path" step first when dns_mx_onboarding_enabled is
    // toggled on. Pick the BYOK path if so; otherwise jump straight to provider.
    if (
      await page
        .locator('h2:has-text("How do you want to use TASMail?")')
        .isVisible()
        .catch(() => false)
    ) {
      await page
        .locator('button.onboarding-path:has-text("Connect an existing account")')
        .click();
    }

    await expect(page.locator('h2:has-text("Who hosts your email?")')).toBeVisible();
    await takeScreenshot(page, 'auth/wizard-01-provider-picker');

    // The custom provider tile drops us on the IMAP form.
    await page.locator('button.onboarding-provider--custom').click();
    await expect(page.locator('h2:has-text("IMAP server")')).toBeVisible();
    await takeScreenshot(page, 'auth/wizard-02-imap-step-empty');
  });

  // ------------------------------------------------------------------
  // 7) Login success — fresh account signs in via the UI and reaches /app
  //    or /onboarding (both confirm the access_token was issued).
  // ------------------------------------------------------------------
  test('login flow signs an existing account in via the UI', async ({
    page,
    takeScreenshot,
    baseURL,
  }) => {
    const email = freshEmail('login-success');
    CREATED_USERNAMES.push(email);

    // Provision the account out-of-band via the API so this test isolates
    // "login UI works" from "signup UI works".
    const api = await apiRequest.newContext({ baseURL });
    const signup = await api.post('/api/auth/signup', {
      data: { email, password: ACCOUNT_PASSWORD },
    });
    expect(signup.status()).toBeLessThan(300);

    await page.goto('/login');
    await page.waitForSelector('#username');
    await page.fill('#username', email);
    await page.fill('#password', ACCOUNT_PASSWORD);
    await takeScreenshot(page, 'auth/login-02-filled-with-good-credentials');
    await page.click('button[type="submit"]');

    // BYOK-fresh accounts land on /onboarding; accounts that already finished
    // BYOK land on /app. Either resolves "logged in".
    await page.waitForURL(/\/(app|onboarding)/, { timeout: 20_000 });
    // Wait until the post-login surface has finished its first paint so the
    // screenshot doesn't catch "Loading folders…" or the wizard's loader.
    await expect(
      page.locator(
        '.sidebar, h2:has-text("Who hosts your email?"), h2:has-text("How do you want to use TASMail?")',
      ),
    ).toBeVisible({ timeout: 10_000 });
    // Folder list is fetched on /app; once the FolderTree's loading placeholder
    // disappears the sidebar tree is live. Skip on /onboarding (no FolderTree).
    if (/\/app/.test(page.url())) {
      await expect(page.locator('.folder-tree--loading')).toHaveCount(0, { timeout: 10_000 });
    }
    await takeScreenshot(page, 'auth/login-03-success-landed');

    // Cross-check: the access_token the SPA wrote actually works against the API.
    const accessToken = await page.evaluate(() => localStorage.getItem('access_token'));
    expect(accessToken).toBeTruthy();
    const me = await api.get('/api/imap-configs', {
      headers: { Authorization: `Bearer ${accessToken!}` },
    });
    expect(me.status(), 'login should issue a token that GET /api/imap-configs accepts').toBe(200);
  });
});
