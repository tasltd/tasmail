/**
 * TMAIL-202: Users admin page + bulk import.
 *
 * Promotes a fresh user to admin, opens /admin/users, asserts the table
 * lists at least the admin's own row, creates a new user via the form,
 * verifies the new row appears, then deletes it. Bulk import is exercised
 * with a small in-memory CSV.
 */
import { test, expect } from '../fixtures/base.js';
import { execFileSync } from 'node:child_process';

const PASSWORD = 'users-e2e-2026';
const DB_URL = process.env.TASMAIL_DB_URL ?? 'postgresql://alleina:alleina_dev_2026@127.0.0.1:5432/tasmail';

function setAdmin(email: string, isAdmin: boolean) {
  execFileSync('psql', [DB_URL, '-At', '-c', `UPDATE mailboxes SET is_admin = ${isAdmin} WHERE username = $$${email}$$;`], { encoding: 'utf8' });
}
function deleteUser(email: string) {
  execFileSync('psql', [DB_URL, '-At', '-c', `DELETE FROM mailboxes WHERE username = $$${email}$$;`], { encoding: 'utf8' });
}

test.describe('UsersManager (TMAIL-202)', () => {
  const ADMIN_EMAIL = `users-${Date.now()}@e2e.tasmail`;
  const NEW_EMAIL = `users-new-${Date.now()}@byok.tasmail`;
  const BULK1 = `bulk1-${Date.now()}@byok.tasmail`;
  const BULK2 = `bulk2-${Date.now()}@byok.tasmail`;

  test.afterAll(() => {
    deleteUser(ADMIN_EMAIL);
    deleteUser(NEW_EMAIL);
    deleteUser(BULK1);
    deleteUser(BULK2);
  });

  test('admin can list, create, delete, and bulk-import users', async ({
    page,
    apiSignup,
    takeScreenshot,
    baseURL,
  }) => {
    test.setTimeout(75_000);

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

    await page.goto('/admin/users');
    await expect(page.locator('h1', { hasText: 'Users' })).toBeVisible({ timeout: 10_000 });
    // Admin's own row is in the list.
    await expect(page.locator('tr', { hasText: ADMIN_EMAIL })).toBeVisible({ timeout: 8_000 });
    await takeScreenshot(page, 'admin-users/01-loaded');

    // Add user.
    await page.locator('button', { hasText: 'Add user' }).click();
    await page.locator('input[placeholder="alice@example.com"]').fill(NEW_EMAIL);
    await page.locator('select').first().selectOption({ label: 'byok.tasmail' });
    await page.locator('input[type="password"]').first().fill('newuser-pass-123');
    await takeScreenshot(page, 'admin-users/02-create-form');

    let createStatus = 0;
    page.on('response', (resp) => {
      if (resp.url().endsWith('/api/admin/users') && resp.request().method() === 'POST') {
        createStatus = resp.status();
      }
    });
    await page.locator('button[type="submit"]', { hasText: 'Create user' }).click();
    await expect.poll(() => createStatus, { timeout: 10_000 }).toBe(201);
    await expect(page.locator('tr', { hasText: NEW_EMAIL })).toBeVisible({ timeout: 8_000 });
    await takeScreenshot(page, 'admin-users/03-after-create');

    // Bulk import — set up the file via setInputFiles with an in-memory CSV.
    const csvBody = `email,display_name,password,role\n${BULK1},Bulk One,bulk-pass-1,user\n${BULK2},Bulk Two,bulk-pass-2,user\n`;
    await page.locator('input[type="file"]').setInputFiles({
      name: 'bulk.csv',
      mimeType: 'text/csv',
      buffer: Buffer.from(csvBody),
    });
    await expect(page.locator('[role="status"]', { hasText: /Bulk import/ })).toBeVisible({ timeout: 15_000 });
    await expect(page.locator('tr', { hasText: BULK1 })).toBeVisible({ timeout: 8_000 });
    await expect(page.locator('tr', { hasText: BULK2 })).toBeVisible();
    await takeScreenshot(page, 'admin-users/04-after-bulk');

    // Delete the manually-created user.
    page.on('dialog', (d) => d.accept().catch(() => {}));
    let deleteStatus = 0;
    page.on('response', (resp) => {
      if (resp.request().method() === 'DELETE' && resp.url().includes('/api/admin/users/')) {
        deleteStatus = resp.status();
      }
    });
    await page.locator('tr', { hasText: NEW_EMAIL }).locator('button.btn--danger').click();
    await expect.poll(() => deleteStatus, { timeout: 10_000 }).toBe(204);
    await expect(page.locator('tr', { hasText: NEW_EMAIL })).toHaveCount(0, { timeout: 8_000 });
    await takeScreenshot(page, 'admin-users/05-after-delete');
  });
});
