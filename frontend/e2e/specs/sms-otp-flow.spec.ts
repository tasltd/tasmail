/**
 * TMAIL-209: SMS OTP enrollment in TwoFactorManager.
 *
 * Sign up, navigate via the sidebar to Security, fill the SMS OTP form,
 * submit, read the test_code rendered by the backend (TASMAIL_SMS_TEST_MODE
 * surfaces it inside the form's hint text), verify, assert "SMS codes
 * enabled" appears, then disable.
 */
import { test, expect } from '../fixtures/base.js';
import { execFileSync } from 'node:child_process';

const PASSWORD = 'sms-otp-e2e-2026';
const DB_URL = process.env.TASMAIL_DB_URL ?? 'postgresql://alleina:alleina_dev_2026@127.0.0.1:5432/tasmail';
const PHONE = '+233241234567';

function deleteUser(email: string) {
  execFileSync('psql', [DB_URL, '-At', '-c', `DELETE FROM mailboxes WHERE username = $$${email}$$;`], { encoding: 'utf8' });
}

test.describe('SMS OTP (TMAIL-209)', () => {
  const EMAIL = `sms-otp-${Date.now()}@e2e.tasmail`;

  test.afterAll(() => deleteUser(EMAIL));

  test('user can enroll, verify, and disable SMS OTP', async ({
    page,
    apiSignup,
    takeScreenshot,
    baseURL,
  }) => {
    test.setTimeout(60_000);

    const tokens = await apiSignup(EMAIL, PASSWORD);
    await page.goto('/login');
    await page.evaluate(([at, rt]) => {
      localStorage.setItem('access_token', at);
      localStorage.setItem('refresh_token', rt);
    }, [tokens.access_token, tokens.refresh_token]);

    // Skip the BYOK wizard by going straight to /app — it'll show the empty
    // state, but we navigate to Security via the sidebar.
    await page.goto('/app');
    await expect(page.locator('button, a', { hasText: /Compose/i }).first()).toBeVisible({ timeout: 20_000 });
    await page.locator('.sidebar button:has-text("Security")').click();
    await expect(page.locator('h2', { hasText: 'SMS one-time codes' })).toBeVisible({ timeout: 10_000 });
    await takeScreenshot(page, 'sms-otp/01-section-loaded');

    // Watch the enroll request to grab the test_code from the response.
    let testCode: string | null = null;
    page.on('response', async (resp) => {
      if (resp.url().endsWith('/api/sms-otp/enroll') && resp.request().method() === 'POST') {
        try {
          const body = await resp.json();
          if (body?.test_code) testCode = String(body.test_code);
        } catch { /* ignore */ }
      }
    });

    await page.locator('input[type="tel"]').fill(PHONE);
    await page.locator('button', { hasText: 'Send code' }).click();
    await expect.poll(() => testCode, { timeout: 10_000 }).not.toBeNull();
    await takeScreenshot(page, 'sms-otp/02-code-sent');

    // The component pre-fills the verify input with the test code in test mode.
    // The verify button enables when length === 6.
    await expect(page.locator('text=/Test mode: code is/')).toBeVisible({ timeout: 5_000 });
    await page.locator('button', { hasText: 'Verify & enable' }).click();
    await expect(page.locator('text=SMS codes enabled')).toBeVisible({ timeout: 8_000 });
    await takeScreenshot(page, 'sms-otp/03-enabled');

    // Cross-check the masked phone via the API.
    const statusResp = await fetch(`${baseURL}/api/sms-otp/status`, {
      headers: { Authorization: `Bearer ${tokens.access_token}` },
    });
    const statusBody = (await statusResp.json()) as { enabled: boolean; phone_number: string | null };
    expect(statusBody.enabled).toBe(true);
    expect(statusBody.phone_number).toContain('***');

    // Disable.
    page.on('dialog', (d) => d.accept().catch(() => {}));
    await page.locator('button', { hasText: 'Disable SMS OTP' }).click();
    await expect(page.locator('text=SMS codes enabled')).toHaveCount(0, { timeout: 8_000 });
    await takeScreenshot(page, 'sms-otp/04-disabled');
  });
});
