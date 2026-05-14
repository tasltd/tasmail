/**
 * TMAIL-199: Cache admin page.
 *
 * Promotes a fresh user to admin, navigates to /admin/cache, asserts the
 * status section reads Connected against the live Redis, then triggers the
 * confirm + flush flow and verifies POST /api/admin/cache/flush returns 200.
 */
import { test, expect } from '../fixtures/base.js';
import { execFileSync } from 'node:child_process';

const PASSWORD = 'cache-e2e-2026';
const DB_URL = process.env.TASMAIL_DB_URL ?? 'postgresql://alleina:alleina_dev_2026@127.0.0.1:5432/tasmail';

function setAdmin(email: string, isAdmin: boolean) {
  execFileSync('psql', [DB_URL, '-At', '-c', `UPDATE mailboxes SET is_admin = ${isAdmin} WHERE username = $$${email}$$;`], { encoding: 'utf8' });
}
function deleteUser(email: string) {
  execFileSync('psql', [DB_URL, '-At', '-c', `DELETE FROM mailboxes WHERE username = $$${email}$$;`], { encoding: 'utf8' });
}

test.describe('CacheManager (TMAIL-199)', () => {
  const ADMIN_EMAIL = `cache-${Date.now()}@e2e.tasmail`;
  test.afterAll(() => deleteUser(ADMIN_EMAIL));

  test('admin can read cache status, INFO, and flush', async ({
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

    await page.goto('/admin');
    await page.locator('.admin-shell__nav-item', { hasText: 'Cache' }).click();
    await expect(page).toHaveURL(/\/admin\/cache$/);
    await expect(page.locator('h1', { hasText: 'Cache' })).toBeVisible();

    // Status reads connected (live tasmail-backend has Redis at 127.0.0.1:6379).
    await expect(page.locator('text=Connected')).toBeVisible({ timeout: 10_000 });
    await takeScreenshot(page, 'admin-cache/01-loaded');

    // Trigger the destructive flow.
    let flushStatus = 0;
    page.on('response', (resp) => {
      if (resp.url().endsWith('/api/admin/cache/flush') && resp.request().method() === 'POST') {
        flushStatus = resp.status();
      }
    });
    await page.locator('button', { hasText: 'Flush all cache keys' }).click();
    await expect(page.locator('[role="alertdialog"]')).toBeVisible();
    await takeScreenshot(page, 'admin-cache/02-confirm');
    await page.locator('button', { hasText: 'Yes, flush' }).click();
    await expect.poll(() => flushStatus, { timeout: 10_000 }).toBe(200);
    // Result banner appears.
    await expect(page.locator('[role="status"]')).toBeVisible({ timeout: 5_000 });
    await takeScreenshot(page, 'admin-cache/03-flushed');
  });
});
