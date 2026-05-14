/**
 * TMAIL-198: Audit log viewer.
 *
 * Promotes a fresh user to admin, navigates to /admin/audit-log, asserts the
 * table renders, and switches the action-prefix filter to confirm the request
 * round-trips through /api/admin/audit-log with the expected query string.
 */
import { test, expect } from '../fixtures/base.js';
import { execFileSync } from 'node:child_process';

const PASSWORD = 'audit-e2e-2026';
const DB_URL = process.env.TASMAIL_DB_URL ?? 'postgresql://alleina:alleina_dev_2026@127.0.0.1:5432/tasmail';

function setAdmin(email: string, isAdmin: boolean) {
  execFileSync('psql', [DB_URL, '-At', '-c', `UPDATE mailboxes SET is_admin = ${isAdmin} WHERE username = $$${email}$$;`], { encoding: 'utf8' });
}
function deleteUser(email: string) {
  execFileSync('psql', [DB_URL, '-At', '-c', `DELETE FROM mailboxes WHERE username = $$${email}$$;`], { encoding: 'utf8' });
}

test.describe('AuditLogManager (TMAIL-198)', () => {
  const ADMIN_EMAIL = `audit-${Date.now()}@e2e.tasmail`;
  test.afterAll(() => deleteUser(ADMIN_EMAIL));

  test('admin can list audit-log entries and filter by action prefix', async ({
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
    await page.locator('.admin-shell__nav-item', { hasText: 'Audit log' }).click();
    await expect(page).toHaveURL(/\/admin\/audit-log$/);
    await expect(page.locator('h1', { hasText: 'Audit log' })).toBeVisible();
    await page.locator('table.audit-table tbody').waitFor({ timeout: 10_000 });
    await takeScreenshot(page, 'admin-audit/01-loaded');

    // Apply prefix filter — should re-fetch with action=auth.
    let lastAuditRequest = '';
    page.on('request', (req) => {
      if (req.url().includes('/api/admin/audit-log')) {
        lastAuditRequest = req.url();
      }
    });
    await page.locator('select').first().selectOption('auth.');
    await expect.poll(() => lastAuditRequest, { timeout: 8_000 }).toContain('action=auth.');
    await takeScreenshot(page, 'admin-audit/02-auth-prefix-filter');

    // The audit log is non-empty for this account because login + signup are
    // both audited. Confirm at least one row contains 'auth.'.
    await expect(page.locator('table.audit-table tbody tr')).not.toHaveCount(0);
    await expect(page.locator('table.audit-table tbody tr td:nth-child(2)').first())
      .toContainText('auth.');
  });
});
