/**
 * TMAIL-136: Admin can export the user list as CSV from the Bulk Import view.
 *
 * Companion to admin-users-flow.spec.ts (TMAIL-202) which covers import.
 * Navigates via the sidebar (no direct page.goto for app routes), clicks the
 * Export Users (CSV) button, intercepts the browser download, and confirms
 * the CSV body matches the live admin users API response (round-trip check).
 */
import { test, expect } from '../fixtures/base.js';
import { execFileSync } from 'node:child_process';

const PASSWORD = 'export-e2e-2026';
const DB_URL = process.env.TASMAIL_DB_URL ?? 'postgresql://alleina:alleina_dev_2026@127.0.0.1:5432/tasmail';

function setAdmin(email: string, isAdmin: boolean) {
  execFileSync('psql', [DB_URL, '-At', '-c', `UPDATE mailboxes SET is_admin = ${isAdmin} WHERE username = $$${email}$$;`], { encoding: 'utf8' });
}

function deleteUser(email: string) {
  execFileSync('psql', [DB_URL, '-At', '-c', `DELETE FROM mailboxes WHERE username = $$${email}$$;`], { encoding: 'utf8' });
}

test.describe('Bulk Import — Export Users (TMAIL-136)', () => {
  const ADMIN_EMAIL = `export-${Date.now()}@e2e.tasmail`;

  test.afterAll(() => {
    deleteUser(ADMIN_EMAIL);
  });

  test('admin can download the users CSV via the Export Users button', async ({
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

    // Seed the JWTs from localStorage so the SPA boots authenticated, then enter via /app.
    await page.goto('/login');
    await page.evaluate(([at, rt]) => {
      localStorage.setItem('access_token', at);
      localStorage.setItem('refresh_token', rt);
    }, [tokens.access_token, tokens.refresh_token]);
    await page.goto('/app');
    await expect(page.locator('aside.sidebar')).toBeVisible({ timeout: 10_000 });

    // Navigate to the Bulk Import view via the sidebar — menu clicks, not page.goto.
    await page.locator('aside.sidebar button:has-text("Bulk Import")').click();
    await expect(page.locator('h2', { hasText: 'Bulk User Import' })).toBeVisible({ timeout: 8_000 });
    await expect(page.getByTestId('export-users-button')).toBeVisible();
    await takeScreenshot(page, 'admin-users/06-bulk-import-loaded');

    // Capture the admin users list via API so we can compare with the CSV download.
    const apiResp = await fetch(`${baseURL}/api/admin/users`, {
      headers: { Authorization: `Bearer ${tokens.access_token}` },
    });
    expect(apiResp.status).toBe(200);
    const apiUsers = (await apiResp.json()) as Array<{ username: string }>;
    expect(apiUsers.find((u) => u.username === ADMIN_EMAIL)).toBeDefined();

    // Click the export button and intercept the browser download.
    const [download] = await Promise.all([
      page.waitForEvent('download', { timeout: 10_000 }),
      page.getByTestId('export-users-button').click(),
    ]);

    expect(download.suggestedFilename()).toBe('users-export.csv');

    const stream = await download.createReadStream();
    const chunks: Buffer[] = [];
    for await (const chunk of stream) {
      chunks.push(Buffer.from(chunk));
    }
    const csv = Buffer.concat(chunks).toString('utf8');

    // Header row must match the documented export columns and MUST NOT include
    // any password/totp fields (security boundary — see csv_processor tests).
    const firstLine = csv.split('\n', 1)[0];
    expect(firstLine).toBe('email,display_name,role,active,quota_bytes,created_at');
    expect(csv).not.toMatch(/password|totp/i);

    // Every API user must appear in the CSV — round-trip check.
    for (const u of apiUsers) {
      expect(csv).toContain(u.username);
    }
    await takeScreenshot(page, 'admin-users/07-after-export-click');
  });
});
