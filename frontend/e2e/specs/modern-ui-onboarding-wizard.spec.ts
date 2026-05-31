/**
 * TMAIL-346: Native Modern UI onboarding wizard for BYOK IMAP/SMTP.
 *
 * Before this ticket the Modern UI's SignupPage bounced the user out to the
 * classic SPA's /onboarding URL to attach an IMAP/SMTP server. After this
 * ticket the wizard lives inside /modern/index.html#/onboarding so the
 * standalone-Modern-UI signup flow (TMAIL-298 P0 #13) never leaves the
 * Modern UI shell.
 *
 * Coverage:
 *   1. Native /#/signup → wizard at /#/onboarding (no full-page nav to the
 *      classic SPA).
 *   2. Provider step lists presets from /api/imap-configs/presets and offers
 *      the "Other / Custom" fallback.
 *   3. IMAP step accepts custom values, calls POST /api/imap-configs/test
 *      against the real swmail.techatscale.io, and shows the success result.
 *   4. Saving IMAP via POST /api/imap-configs persists a row visible to
 *      GET /api/imap-configs.
 *   5. SMTP step submits POST /api/smtp-configs and the row is visible to
 *      GET /api/smtp-configs.
 *   6. Wizard finishes on the "done" screen and routes the user to the
 *      Modern UI inbox at /#/.
 *
 * Screenshots: frontend/e2e/screenshots/modern-ui-onboarding-wizard/<step>.png
 */
import { test, NOREPLY_CREDS, expect } from '../fixtures/base.js';
import { deleteMailboxByUsername } from '../helpers/db-cleanup.js';

const PASSWORD = 'modern-ui-onboarding-2026';
const SCREENSHOT_DIR = 'modern-ui-onboarding-wizard';

test.describe('TMAIL-346 Modern UI BYOK onboarding wizard', () => {
  // Each test creates the noreply mailbox via the public signup endpoint, so
  // we wipe the row before AND after every test to keep the suite idempotent.
  // afterAll catches the last test's row so a fresh run starts clean too.
  test.beforeEach(() => deleteMailboxByUsername(NOREPLY_CREDS.email));
  test.afterAll(() => deleteMailboxByUsername(NOREPLY_CREDS.email));

  test('signup → wizard (provider → imap → smtp → done) → inbox', async ({
    page,
    takeScreenshot,
    baseURL,
  }) => {
    // Each step talks to the real backend (test conn, save IMAP, save SMTP)
    // so we need plenty of headroom over the 30s default.
    test.setTimeout(180_000);

    // ─── 1. Wipe any pre-existing session and visit the native signup ─────
    await page.goto(`${baseURL}/modern/index.html#/signup`);
    await page.evaluate(() => {
      localStorage.clear();
      sessionStorage.clear();
    });
    await page.reload();
    await page.waitForFunction(
      () => window.location.hash.startsWith('#/signup'),
      null,
      { timeout: 15_000 },
    );

    // Pre-state: GET /api/imap-configs unauthenticated must 401 — proves we
    // really started from a clean session.
    const preProbe = await page.evaluate(async () => {
      const r = await fetch('/api/imap-configs', { credentials: 'omit' });
      return r.status;
    });
    expect(preProbe, 'pre-signup /api/imap-configs unauthenticated').toBe(401);

    await expect(
      page.locator('h1', { hasText: 'Create your TASMail account' }),
    ).toBeVisible();
    await page.fill('#email', NOREPLY_CREDS.email);
    await page.fill('#password', PASSWORD);
    await page.fill('#confirm', PASSWORD);
    await takeScreenshot(page, `${SCREENSHOT_DIR}/01-signup-filled`);
    await page.click('button[type="submit"]:has-text("Create account")');

    // ─── 2. We should land on the native wizard, NOT the classic SPA ──────
    await page.waitForFunction(
      () => window.location.hash.startsWith('#/onboarding'),
      null,
      { timeout: 30_000 },
    );
    expect(page.url(), 'wizard URL').toContain('/modern/index.html');
    expect(page.url(), 'wizard URL').toContain('#/onboarding');

    await expect(
      page.locator('h1', { hasText: 'Connect your mailbox' }),
    ).toBeVisible();
    await expect(
      page.locator('h2', { hasText: 'Who hosts your email?' }),
    ).toBeVisible({ timeout: 15_000 });

    // Sanity-check: the preset grid actually rendered something from
    // /api/imap-configs/presets (backend ships ~11 presets — wait for at
    // least Gmail to render before counting). The first call returns 401
    // because AuthGate's setToken effect races with the wizard's mount, so
    // apiClient transparently refreshes + retries; presets appear ~500ms
    // after mount.
    await expect(page.locator('[data-testid="provider-gmail.com"]')).toBeVisible({
      timeout: 15_000,
    });
    await takeScreenshot(page, `${SCREENSHOT_DIR}/02-provider-step`);
    // `data-testid^="provider-"` matches both the per-preset buttons AND the
    // Other/Custom fallback button — assert ≥ 4 so we're sure we got the
    // real presets and not just the fallback.
    const presetCount = await page.locator('[data-testid^="provider-"]').count();
    expect(presetCount, 'preset count from /api/imap-configs/presets').toBeGreaterThanOrEqual(4);

    // ─── 3. Pick "Other / Custom" so we can drive the noreply IMAP creds ─
    await page.click('[data-testid="provider-custom"]');

    await expect(
      page.locator('h2', { hasText: 'IMAP server (incoming mail)' }),
    ).toBeVisible({ timeout: 10_000 });
    await page.fill('#imap-host', NOREPLY_CREDS.imap.host);
    await page.fill('#imap-port', String(NOREPLY_CREDS.imap.port));
    // The Encryption Select is a Radix combobox; click then pick by role.
    await page.click('#imap-encryption');
    await page.getByRole('option', { name: 'SSL/TLS' }).click();
    await page.fill('#imap-username', NOREPLY_CREDS.imap.username);
    await page.fill('#imap-password', NOREPLY_CREDS.imap.password);
    await takeScreenshot(page, `${SCREENSHOT_DIR}/03-imap-form-filled`);

    // ─── 4. Test IMAP connection — backend must return ok=true ────────────
    await page.click('[data-testid="imap-test-button"]');
    await expect(page.locator('[data-testid="imap-test-result"]')).toBeVisible({
      timeout: 30_000,
    });
    const imapResult = await page
      .locator('[data-testid="imap-test-result"]')
      .textContent();
    await takeScreenshot(page, `${SCREENSHOT_DIR}/04-imap-test-success`);
    expect(imapResult, `IMAP test against ${NOREPLY_CREDS.imap.host}`).toContain(
      'IMAP login succeeded',
    );

    // ─── 5. Save IMAP — confirm backend state changed (BEFORE/AFTER) ──────
    const imapBefore = await page.evaluate(async () => {
      const token = localStorage.getItem('access_token');
      const r = await fetch('/api/imap-configs', {
        headers: token ? { Authorization: `Bearer ${token}` } : {},
      });
      return r.ok ? ((await r.json()) as unknown[]).length : -1;
    });
    expect(imapBefore, 'IMAP rows before save').toBe(0);

    await page.click('[data-testid="imap-continue-button"]');

    await expect(
      page.locator('h2', { hasText: 'SMTP server (outgoing mail)' }),
    ).toBeVisible({ timeout: 30_000 });

    const imapAfter = await page.evaluate(async () => {
      const token = localStorage.getItem('access_token');
      const r = await fetch('/api/imap-configs', {
        headers: token ? { Authorization: `Bearer ${token}` } : {},
      });
      return r.ok ? ((await r.json()) as unknown[]).length : -1;
    });
    expect(imapAfter, 'IMAP rows after save').toBe(1);

    // ─── 6. SMTP step — fill, save, confirm DB row appears ────────────────
    await page.fill('#smtp-host', NOREPLY_CREDS.smtp.host);
    await page.fill('#smtp-port', String(NOREPLY_CREDS.smtp.port));
    await page.click('#smtp-encryption');
    await page.getByRole('option', { name: 'SSL/TLS' }).click();
    await page.fill('#smtp-username', NOREPLY_CREDS.smtp.username);
    await page.fill('#smtp-password', NOREPLY_CREDS.smtp.password);
    await takeScreenshot(page, `${SCREENSHOT_DIR}/05-smtp-form-filled`);

    const smtpBefore = await page.evaluate(async () => {
      const token = localStorage.getItem('access_token');
      const r = await fetch('/api/smtp-configs', {
        headers: token ? { Authorization: `Bearer ${token}` } : {},
      });
      return r.ok ? ((await r.json()) as unknown[]).length : -1;
    });
    expect(smtpBefore, 'SMTP rows before save').toBe(0);

    await page.click('[data-testid="smtp-continue-button"]');

    // ─── 7. Done screen + auto-redirect to inbox ──────────────────────────
    await expect(page.locator('[data-testid="onboarding-done"]')).toBeVisible({
      timeout: 30_000,
    });
    await takeScreenshot(page, `${SCREENSHOT_DIR}/06-done-screen`);

    const smtpAfter = await page.evaluate(async () => {
      const token = localStorage.getItem('access_token');
      const r = await fetch('/api/smtp-configs', {
        headers: token ? { Authorization: `Bearer ${token}` } : {},
      });
      return r.ok ? ((await r.json()) as unknown[]).length : -1;
    });
    expect(smtpAfter, 'SMTP rows after save').toBe(1);

    // The wizard sets a 1.2s timeout to navigate('/', { replace: true }) so
    // the user lands at the Modern UI inbox.
    await page.waitForFunction(
      () => window.location.hash === '#/' || window.location.hash === '',
      null,
      { timeout: 15_000 },
    );
    expect(page.url(), 'post-wizard URL').toContain('/modern/index.html');
    await takeScreenshot(page, `${SCREENSHOT_DIR}/07-landed-on-inbox`);
  });

  test('provider step renders presets and shows back-from-imap recovery', async ({
    page,
    apiSignup,
    takeScreenshot,
    baseURL,
  }) => {
    // A second pass that doesn't go all the way through — just exercises the
    // back/forward edges of the wizard so a regression in step navigation
    // gets caught without re-running the full IMAP+SMTP round trip.
    test.setTimeout(90_000);

    // Pre-create the account through the API so we skip the signup form and
    // land directly inside the wizard.
    const tokens = await apiSignup(NOREPLY_CREDS.email, PASSWORD);
    await page.goto(`${baseURL}/modern/index.html#/login`);
    await page.evaluate(
      ({ access, refresh }) => {
        localStorage.setItem('access_token', access);
        localStorage.setItem('refresh_token', refresh);
      },
      { access: tokens.access_token, refresh: tokens.refresh_token },
    );
    await page.goto(`${baseURL}/modern/index.html#/onboarding`);

    await expect(
      page.locator('h2', { hasText: 'Who hosts your email?' }),
    ).toBeVisible({ timeout: 15_000 });
    // Wait for the real presets to render (not just the custom fallback).
    await expect(page.locator('[data-testid="provider-gmail.com"]')).toBeVisible({
      timeout: 15_000,
    });
    await takeScreenshot(page, `${SCREENSHOT_DIR}/08-provider-direct-visit`);

    // Pick the Gmail preset — its pre-fill confirms onPick wires through
    // the preset's host/port/encryption into the IMAP form.
    await page.click('[data-testid="provider-gmail.com"]');
    await expect(page.locator('#imap-host')).toBeVisible({ timeout: 10_000 });
    await expect(page.locator('#imap-host')).toHaveValue('imap.gmail.com');
    await expect(page.locator('#imap-port')).toHaveValue('993');

    // …then click Back to confirm we land on the provider step again with
    // no errors.
    await page.click('button:has-text("Back")');
    await expect(
      page.locator('h2', { hasText: 'Who hosts your email?' }),
    ).toBeVisible({ timeout: 10_000 });
    await takeScreenshot(page, `${SCREENSHOT_DIR}/09-back-to-provider`);
  });
});
