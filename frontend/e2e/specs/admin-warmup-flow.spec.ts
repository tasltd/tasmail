/**
 * TMAIL-203: IP warm-up admin page.
 *
 * Promotes a fresh user to admin, opens /admin/warmup, asserts the 8-week
 * schedule renders, submits a synthetic IP via the form and confirms the
 * tracked-IPs table picks it up.
 */
import { test, expect } from '../fixtures/base.js';
import { execFileSync } from 'node:child_process';

const PASSWORD = 'warmup-e2e-2026';
const DB_URL = process.env.TASMAIL_DB_URL ?? 'postgresql://alleina:alleina_dev_2026@127.0.0.1:5432/tasmail';

function setAdmin(email: string, isAdmin: boolean) {
  execFileSync('psql', [DB_URL, '-At', '-c', `UPDATE mailboxes SET is_admin = ${isAdmin} WHERE username = $$${email}$$;`], { encoding: 'utf8' });
}
function deleteUser(email: string) {
  execFileSync('psql', [DB_URL, '-At', '-c', `DELETE FROM mailboxes WHERE username = $$${email}$$;`], { encoding: 'utf8' });
}
function deleteWarmup(ip: string) {
  execFileSync('psql', [DB_URL, '-At', '-c', `DELETE FROM ip_warmup_tracking WHERE ip_address = $$${ip}$$;`], { encoding: 'utf8' });
}

test.describe('WarmupManager (TMAIL-203)', () => {
  const ADMIN_EMAIL = `warmup-${Date.now()}@e2e.tasmail`;
  // 198.51.100.0/24 is RFC 5737 TEST-NET-2 — guaranteed not routable.
  const TEST_IP = `198.51.100.${Math.floor(Math.random() * 200) + 10}`;

  test.afterAll(() => {
    deleteUser(ADMIN_EMAIL);
    deleteWarmup(TEST_IP);
  });

  test('admin can view schedule and start tracking a new IP', async ({
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

    await page.goto('/admin/warmup');
    await expect(page.locator('h1', { hasText: 'IP warm-up' })).toBeVisible({ timeout: 10_000 });

    // 8-week schedule renders. "Week 1" / "Week 8" appear in multiple cells
    // (table row + week label inside the description). Anchor on the first
    // match per row to satisfy strict-mode locators.
    await expect(page.locator('tr td', { hasText: 'Week 1' }).first()).toBeVisible({ timeout: 8_000 });
    await expect(page.locator('tr td', { hasText: 'Week 8' }).first()).toBeVisible();
    await takeScreenshot(page, 'admin-warmup/01-loaded');

    // Start tracking a new IP.
    let startStatus = 0;
    page.on('response', (resp) => {
      if (resp.url().endsWith('/api/admin/warmup/start') && resp.request().method() === 'POST') {
        startStatus = resp.status();
      }
    });
    await page.locator('input[placeholder^="203.0.113"]').fill(TEST_IP);
    await page.locator('button', { hasText: 'Start warm-up' }).click();
    await expect.poll(() => startStatus, { timeout: 10_000 }).toBe(201);
    // Tracked IP row appears after the status query refetches.
    await expect(page.locator('tr', { hasText: TEST_IP })).toBeVisible({ timeout: 10_000 });
    await takeScreenshot(page, 'admin-warmup/02-tracking');
  });
});
