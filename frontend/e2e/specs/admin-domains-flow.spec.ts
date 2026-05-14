/**
 * TMAIL-200: Domains admin page.
 *
 * Promotes a fresh user to admin, opens /admin/domains, asserts byok.tasmail
 * is listed and marked protected, adds a new test domain via the form,
 * confirms it appears in the list, then deletes it and confirms removal.
 */
import { test, expect } from '../fixtures/base.js';
import { execFileSync } from 'node:child_process';

const PASSWORD = 'domains-e2e-2026';
const DB_URL = process.env.TASMAIL_DB_URL ?? 'postgresql://alleina:alleina_dev_2026@127.0.0.1:5432/tasmail';

function setAdmin(email: string, isAdmin: boolean) {
  execFileSync('psql', [DB_URL, '-At', '-c', `UPDATE mailboxes SET is_admin = ${isAdmin} WHERE username = $$${email}$$;`], { encoding: 'utf8' });
}
function deleteUser(email: string) {
  execFileSync('psql', [DB_URL, '-At', '-c', `DELETE FROM mailboxes WHERE username = $$${email}$$;`], { encoding: 'utf8' });
}
function deleteDomain(name: string) {
  execFileSync('psql', [DB_URL, '-At', '-c', `DELETE FROM domains WHERE name = $$${name}$$;`], { encoding: 'utf8' });
}

test.describe('DomainsManager (TMAIL-200)', () => {
  const ADMIN_EMAIL = `domains-${Date.now()}@e2e.tasmail`;
  const TEST_DOMAIN = `e2e-${Date.now()}.example`;

  test.afterAll(() => {
    deleteUser(ADMIN_EMAIL);
    deleteDomain(TEST_DOMAIN);
  });

  test('admin can list, add, and delete domains', async ({
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

    await page.goto('/admin/domains');
    await expect(page.locator('h1', { hasText: 'Domains' })).toBeVisible({ timeout: 10_000 });

    // byok.tasmail is always present and marked protected.
    const byokRow = page.locator('tr', { hasText: 'byok.tasmail' });
    await expect(byokRow).toBeVisible();
    await expect(byokRow.locator('text=protected')).toBeVisible();
    await takeScreenshot(page, 'admin-domains/01-list');

    // Add new domain.
    await page.locator('button', { hasText: 'Add domain' }).click();
    await page.locator('input[placeholder="example.com"]').fill(TEST_DOMAIN);
    await takeScreenshot(page, 'admin-domains/02-add-form-filled');
    await page.locator('button[type="submit"]', { hasText: 'Save' }).click();
    await expect(page.locator('tr', { hasText: TEST_DOMAIN })).toBeVisible({ timeout: 8_000 });
    await takeScreenshot(page, 'admin-domains/03-after-add');

    // Delete the new domain — accept confirm.
    page.on('dialog', (d) => d.accept().catch(() => {}));
    let deleteStatus = 0;
    page.on('response', (resp) => {
      if (resp.request().method() === 'DELETE' && resp.url().includes('/api/admin/domains/')) {
        deleteStatus = resp.status();
      }
    });
    await page.locator('tr', { hasText: TEST_DOMAIN }).locator('button.btn--danger').click();
    await expect.poll(() => deleteStatus, { timeout: 10_000 }).toBe(204);
    await expect(page.locator('tr', { hasText: TEST_DOMAIN })).toHaveCount(0, { timeout: 8_000 });
    await takeScreenshot(page, 'admin-domains/04-after-delete');
  });
});
