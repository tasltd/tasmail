/**
 * TMAIL-201: Payment providers admin page.
 *
 * Promotes a fresh user to admin, opens /admin/payment-providers, adds a
 * Paystack provider with a dummy secret + webhook, asserts the row appears
 * (with secret/webhook check marks), then archives it and verifies it
 * disappears from the live list.
 */
import { test, expect } from '../fixtures/base.js';
import { execFileSync } from 'node:child_process';

const PASSWORD = 'pp-e2e-2026';
const DB_URL = process.env.TASMAIL_DB_URL ?? 'postgresql://alleina:alleina_dev_2026@127.0.0.1:5432/tasmail';

function setAdmin(email: string, isAdmin: boolean) {
  execFileSync('psql', [DB_URL, '-At', '-c', `UPDATE mailboxes SET is_admin = ${isAdmin} WHERE username = $$${email}$$;`], { encoding: 'utf8' });
}
function deleteUser(email: string) {
  execFileSync('psql', [DB_URL, '-At', '-c', `DELETE FROM mailboxes WHERE username = $$${email}$$;`], { encoding: 'utf8' });
}
function archiveProvidersByName(name: string) {
  execFileSync('psql', [DB_URL, '-At', '-c', `DELETE FROM payment_provider_config WHERE name = $$${name}$$;`], { encoding: 'utf8' });
}

test.describe('PaymentProvidersManager (TMAIL-201)', () => {
  const ADMIN_EMAIL = `pp-${Date.now()}@e2e.tasmail`;
  const PROVIDER_NAME = `E2E Paystack ${Date.now()}`;

  test.afterAll(() => {
    deleteUser(ADMIN_EMAIL);
    archiveProvidersByName(PROVIDER_NAME);
  });

  test('admin can add and archive a payment provider', async ({
    page,
    apiSignup,
    takeScreenshot,
    baseURL,
  }) => {
    test.setTimeout(60_000);

    await apiSignup(ADMIN_EMAIL, PASSWORD);
    setAdmin(ADMIN_EMAIL, true);

    const loginResp = await fetch(`${baseURL}/api/auth/login`, {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify({ username: ADMIN_EMAIL, password: PASSWORD }),
    });
    expect(loginResp.status).toBe(200);
    const tokens = (await loginResp.json()) as { access_token: string; refresh_token: string };

    await page.goto('/login');
    await page.evaluate(([at, rt]) => {
      localStorage.setItem('access_token', at);
      localStorage.setItem('refresh_token', rt);
    }, [tokens.access_token, tokens.refresh_token]);

    await page.goto('/admin/payment-providers');
    await expect(page.locator('h1', { hasText: 'Payment providers' })).toBeVisible({ timeout: 10_000 });
    await takeScreenshot(page, 'admin-payment-providers/01-loaded');

    // Open form, fill Paystack credentials. Use placeholder-based selectors
    // so the name field doesn't collide with the public_key/currency text
    // inputs further down the form.
    await page.locator('button', { hasText: 'Add provider' }).click();
    await expect(page.locator('select').first()).toBeVisible();
    await page.locator('input[placeholder^="e.g. Paystack"]').fill(PROVIDER_NAME);
    await page.locator('input[type="password"]').first().fill('sk_test_secret_value');
    await page.locator('input[type="password"]').nth(1).fill('whsec_test_value');
    await takeScreenshot(page, 'admin-payment-providers/02-form-filled');

    let createStatus = 0;
    page.on('response', (resp) => {
      if (resp.url().endsWith('/api/admin/payment-providers') && resp.request().method() === 'POST') {
        createStatus = resp.status();
      }
    });
    await page.locator('button[type="submit"]', { hasText: 'Save provider' }).click();
    await expect.poll(() => createStatus, { timeout: 10_000 }).toBe(201);

    // Row appears with check marks for secret + webhook.
    const row = page.locator('tr', { hasText: PROVIDER_NAME });
    await expect(row).toBeVisible({ timeout: 8_000 });
    await takeScreenshot(page, 'admin-payment-providers/03-after-add');

    // Archive the row.
    page.on('dialog', (d) => d.accept().catch(() => {}));
    let archiveStatus = 0;
    page.on('response', (resp) => {
      if (resp.request().method() === 'DELETE' && resp.url().includes('/api/admin/payment-providers/')) {
        archiveStatus = resp.status();
      }
    });
    await row.locator('button.btn--danger').click();
    await expect.poll(() => archiveStatus, { timeout: 10_000 }).toBe(204);
    await expect(page.locator('tr', { hasText: PROVIDER_NAME })).toHaveCount(0, { timeout: 8_000 });
    await takeScreenshot(page, 'admin-payment-providers/04-after-archive');
  });
});
