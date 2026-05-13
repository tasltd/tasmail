/**
 * End-to-end BYOK validation using the real noreply@techatscale.io mailbox
 * AS the TASMail account (TMAIL-194).
 *
 * Unlike signup-byok-flow.spec.ts (which signs up a throwaway e2e-*@e2e.tasmail
 * account and only attaches the noreply mailbox as BYOK credentials), this
 * spec uses noreply@techatscale.io for *both* sides:
 *
 *   - the TASMail account email at signup
 *   - the BYOK IMAP/SMTP credentials in the onboarding wizard
 *
 * The flow it walks:
 *   0. Delete any pre-existing noreply@techatscale.io mailbox row so the test
 *      is idempotent (no 409 from /api/auth/signup).
 *   1. Sign up via the public /signup form using noreply@techatscale.io.
 *   2. Walk the onboarding wizard: Connect existing → Other/Custom → IMAP form.
 *   3. Test IMAP connection against swmail.techatscale.io:993 — backend must
 *      report "IMAP login succeeded".
 *   4. Save & continue, then fill the SMTP form (port 465 SSL).
 *   5. Test SMTP connection — backend must report a success result.
 *   6. Finish setup, land on /app.
 *   7. Confirm /api/folders returns a non-empty list including INBOX.
 *   8. Confirm /api/smtp-configs/test returns ok=true against the saved row.
 *
 * Screenshots land under e2e/screenshots/byok-noreply/.
 */
import { test, NOREPLY_CREDS, expect } from '../fixtures/base.js';
import { deleteMailboxByUsername } from '../helpers/db-cleanup.js';

const NOREPLY_TASMAIL_PASSWORD = 'noreply-tasmail-e2e-2026';

test.describe('BYOK end-to-end as noreply@techatscale.io', () => {
  test.beforeAll(async () => {
    const deleted = deleteMailboxByUsername(NOREPLY_CREDS.email);
    // 0 = first run; 1 = re-run cleanup. Both are fine.
    expect(deleted, 'pre-test cleanup of noreply mailbox').toBeGreaterThanOrEqual(0);
  });

  test('signup → wizard → /app proves IMAP + SMTP work against swmail', async ({
    page,
    signupAs,
    takeScreenshot,
    baseURL,
  }) => {
    test.setTimeout(120_000);

    // ─── 1. SIGN UP via the public form ────────────────────────────────────
    await signupAs(page, NOREPLY_CREDS.email, NOREPLY_TASMAIL_PASSWORD);
    await expect(page).toHaveURL(/\/onboarding/);
    await takeScreenshot(page, 'byok-noreply/01-signup-landed-on-onboarding');

    // ─── 2. Path picker (may auto-skip if only BYOK is enabled) ────────────
    const pathHeading = page.locator('h2:has-text("How do you want to use TASMail?")');
    if (await pathHeading.isVisible().catch(() => false)) {
      await page.locator('button.onboarding-path:has-text("Connect an existing account")').click();
      await takeScreenshot(page, 'byok-noreply/02a-path-byok-picked');
    }

    // ─── 3. Provider picker → Other/Custom ─────────────────────────────────
    await expect(page.locator('h2:has-text("Who hosts your email?")')).toBeVisible({ timeout: 10_000 });
    await takeScreenshot(page, 'byok-noreply/02b-provider-picker');
    await page.locator('button.onboarding-provider--custom').click();

    // ─── 4. IMAP form ──────────────────────────────────────────────────────
    await expect(page.locator('h2:has-text("IMAP server")')).toBeVisible();
    await page.locator('input[placeholder^="imap."]').fill(NOREPLY_CREDS.imap.host);
    await page.locator('input[type="number"]').fill(String(NOREPLY_CREDS.imap.port));
    await page.locator('select').first().selectOption(NOREPLY_CREDS.imap.encryption);
    await page.locator('input[placeholder*="full email"]').fill(NOREPLY_CREDS.imap.username);
    await page.locator('input[type="password"]').first().fill(NOREPLY_CREDS.imap.password);
    await takeScreenshot(page, 'byok-noreply/03-imap-form-filled');

    // Test the IMAP connection — this proves the backend can actually log in.
    await page.locator('button.btn--ghost', { hasText: 'Test connection' }).click();
    await expect(page.locator('.onboarding-test-result')).toBeVisible({ timeout: 20_000 });
    const imapResultText = await page.locator('.onboarding-test-result').textContent();
    await takeScreenshot(page, 'byok-noreply/04-imap-test-result');
    expect(imapResultText, `IMAP test against ${NOREPLY_CREDS.imap.host}`).toContain('IMAP login succeeded');

    await page.locator('button.btn--primary', { hasText: 'Save & continue' }).click();

    // ─── 5. SMTP form ──────────────────────────────────────────────────────
    await expect(page.locator('h2:has-text("SMTP server")')).toBeVisible({ timeout: 15_000 });
    await page.locator('input[placeholder^="smtp."]').fill(NOREPLY_CREDS.smtp.host);
    await page.locator('input[type="number"]').fill(String(NOREPLY_CREDS.smtp.port));
    await page.locator('select').first().selectOption(NOREPLY_CREDS.smtp.encryption);
    await page.locator('input[placeholder*="full email"]').fill(NOREPLY_CREDS.smtp.username);
    await page.locator('input[type="password"]').first().fill(NOREPLY_CREDS.smtp.password);
    await takeScreenshot(page, 'byok-noreply/05-smtp-form-filled');

    // SMTP also exposes Test connection. If the button is visible click it.
    const smtpTestBtn = page.locator('button.btn--ghost', { hasText: 'Test connection' });
    if (await smtpTestBtn.isVisible().catch(() => false)) {
      await smtpTestBtn.click();
      await expect(page.locator('.onboarding-test-result')).toBeVisible({ timeout: 20_000 });
      const smtpResultText = await page.locator('.onboarding-test-result').textContent();
      await takeScreenshot(page, 'byok-noreply/06-smtp-test-result');
      expect(smtpResultText?.toLowerCase(), `SMTP test against ${NOREPLY_CREDS.smtp.host}`).toMatch(/succeed|ok|connected/);
    }

    // ─── 6. Finish setup → /app ────────────────────────────────────────────
    await page.locator('button.btn--primary', { hasText: 'Finish setup' }).click();
    await page.waitForURL(/\/app/, { timeout: 30_000 });
    await expect(page.locator('button, a', { hasText: /Compose/i }).first())
      .toBeVisible({ timeout: 20_000 });
    await takeScreenshot(page, 'byok-noreply/07-app-shell-loaded');

    // ─── 7. Validate /api/folders returns a real list from swmail ──────────
    const accessToken = await page.evaluate(() => localStorage.getItem('access_token'));
    expect(accessToken, 'access token persisted to localStorage').toBeTruthy();
    const authHeaders = { Authorization: `Bearer ${accessToken}` };

    const foldersResp = await fetch(`${baseURL}/api/folders`, { headers: authHeaders });
    expect(foldersResp.status, '/api/folders status').toBe(200);
    const folders = (await foldersResp.json()) as Array<{ name: string }>;
    expect(folders.length, 'folder count from swmail').toBeGreaterThan(0);
    expect(folders.map((f) => f.name.toUpperCase()), 'INBOX must exist').toContain('INBOX');

    // ─── 8. Validate the SMTP config we saved actually authenticates ──────
    const smtpConfigsResp = await fetch(`${baseURL}/api/smtp-configs`, { headers: authHeaders });
    expect(smtpConfigsResp.status, '/api/smtp-configs list status').toBe(200);
    const smtpConfigs = (await smtpConfigsResp.json()) as Array<{ id: string; host: string }>;
    expect(smtpConfigs.length, 'at least one SMTP config saved').toBeGreaterThan(0);
    const smtpId = smtpConfigs[0].id;

    const smtpTestResp = await fetch(`${baseURL}/api/smtp-configs/${smtpId}/test`, {
      method: 'POST',
      headers: authHeaders,
    });
    expect(smtpTestResp.status, '/api/smtp-configs/:id/test status').toBe(200);
    const smtpTestBody = (await smtpTestResp.json()) as { ok?: boolean; success?: boolean; message?: string };
    const smtpOk = smtpTestBody.ok ?? smtpTestBody.success;
    expect(smtpOk, `SMTP test body: ${JSON.stringify(smtpTestBody)}`).toBe(true);

    // ─── 9. Click INBOX in the sidebar to render the message list ──────────
    const inboxLink = page.locator('button, a, li', { hasText: /INBOX/i }).first();
    await inboxLink.click().catch(() => null);
    await page.waitForTimeout(4000);
    await takeScreenshot(page, 'byok-noreply/08-inbox-rendered');
  });

  test.afterAll(async () => {
    // Leave the system in the state the next run expects — clean.
    deleteMailboxByUsername(NOREPLY_CREDS.email);
  });
});
