/**
 * TMAIL-197: AdminShell scaffold + RequireAdmin guard.
 *
 * Two passes:
 *   - non-admin user lands on /admin → role-gate page renders, no sidebar.
 *   - admin user (manually toggled via DB) sees the AdminShell sidebar with
 *     all nine entries, can navigate between feature-flags / quote-requests
 *     (real managers) and audit-log / cache / domains / payment-providers /
 *     users / warmup (placeholders for TMAIL-198..203).
 */
import { test, expect } from '../fixtures/base.js';
import { execFileSync } from 'node:child_process';

const PASSWORD = 'admin-shell-e2e-2026';
const DB_URL = process.env.TASMAIL_DB_URL ?? 'postgresql://alleina:alleina_dev_2026@127.0.0.1:5432/tasmail';

function setAdmin(email: string, isAdmin: boolean) {
  execFileSync(
    'psql',
    [DB_URL, '-At', '-c', `UPDATE mailboxes SET is_admin = ${isAdmin} WHERE username = $$${email}$$;`],
    { encoding: 'utf8' },
  );
}

function deleteUser(email: string) {
  execFileSync(
    'psql',
    [DB_URL, '-At', '-c', `DELETE FROM mailboxes WHERE username = $$${email}$$;`],
    { encoding: 'utf8' },
  );
}

test.describe('AdminShell + RequireAdmin (TMAIL-197)', () => {
  const NON_ADMIN_EMAIL = `admin-shell-na-${Date.now()}@e2e.tasmail`;
  const ADMIN_EMAIL = `admin-shell-yes-${Date.now()}@e2e.tasmail`;

  test.afterAll(() => {
    deleteUser(NON_ADMIN_EMAIL);
    deleteUser(ADMIN_EMAIL);
  });

  test('non-admin user gets the role-gate screen at /admin', async ({ page, apiSignup, takeScreenshot }) => {
    test.setTimeout(60_000);
    const tokens = await apiSignup(NON_ADMIN_EMAIL, PASSWORD);
    await page.goto('/login');
    await page.evaluate(([at, rt]) => {
      localStorage.setItem('access_token', at);
      localStorage.setItem('refresh_token', rt);
    }, [tokens.access_token, tokens.refresh_token]);
    await page.goto('/admin');
    // Role gate visible, sidebar absent.
    await expect(page.locator('h1', { hasText: 'Admin only' })).toBeVisible({ timeout: 10_000 });
    await expect(page.locator('.admin-shell__sidebar')).toHaveCount(0);
    await takeScreenshot(page, 'admin-shell/01-role-gate');
  });

  test('admin user sees the shell with all nav entries and can switch pages', async ({
    page,
    apiSignup,
    takeScreenshot,
  }) => {
    test.setTimeout(60_000);
    // Sign up, then promote in the DB. Re-issue tokens so the JWT reflects is_admin=true.
    await apiSignup(ADMIN_EMAIL, PASSWORD);
    setAdmin(ADMIN_EMAIL, true);

    // Re-login through the public login API to mint a fresh JWT with the
    // admin claim. Same shape apiSignup uses.
    const baseURL = test.info().project.use.baseURL ?? 'https://mail.techatscale.io';
    const loginResp = await fetch(`${baseURL}/api/auth/login`, {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify({ username: ADMIN_EMAIL, password: PASSWORD }),
    });
    expect(loginResp.status, 'admin re-login').toBe(200);
    const adminTokens = (await loginResp.json()) as { access_token: string; refresh_token: string };

    await page.goto('/login');
    await page.evaluate(([at, rt]) => {
      localStorage.setItem('access_token', at);
      localStorage.setItem('refresh_token', rt);
    }, [adminTokens.access_token, adminTokens.refresh_token]);

    await page.goto('/admin');
    // Index redirect lands on feature-flags; sidebar with all nine entries is visible.
    await expect(page).toHaveURL(/\/admin\/feature-flags$/);
    await expect(page.locator('.admin-shell__sidebar')).toBeVisible({ timeout: 10_000 });
    const navItems = page.locator('.admin-shell__nav-item');
    await expect(navItems).toHaveCount(8);
    await takeScreenshot(page, 'admin-shell/02-feature-flags-active');

    // Changed: TMAIL-198..203 replaced the placeholder routes with real
    // manager pages. Walk each one and assert its h1 renders.
    const realManagers: Array<string> = [
      'Audit log', 'Cache', 'Domains', 'Payment providers', 'Users', 'IP warm-up',
    ];
    for (const label of realManagers) {
      await page.locator('.admin-shell__nav-item', { hasText: label }).click();
      await expect(page.locator('h1', { hasText: label })).toBeVisible({ timeout: 8_000 });
    }
    await takeScreenshot(page, 'admin-shell/03-managers-walked');

    // Real manager: switch to Quote requests and verify the existing manager mounts
    // (it owns its own header; we look for the QuoteRequestsManager title).
    await page.locator('.admin-shell__nav-item', { hasText: 'Quote requests' }).click();
    await expect(page).toHaveURL(/\/admin\/quote-requests$/);
    await takeScreenshot(page, 'admin-shell/04-quote-requests-active');
  });
});
