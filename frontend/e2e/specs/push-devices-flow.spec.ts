/**
 * TMAIL-204: Push notification device manager.
 *
 * Bootstraps a fresh user, registers two synthetic devices via the public
 * /api/push/register endpoint (one FCM, one APNs — what the Flutter app would
 * do at install time), then opens /app and clicks Sidebar → Notifications.
 * Verifies the manager renders both rows, the test-notification button calls
 * /api/push/test, and unregister removes a row.
 */
import { test, NOREPLY_CREDS, expect } from '../fixtures/base.js';
import { deleteMailboxByUsername } from '../helpers/db-cleanup.js';

const PASSWORD = 'push-e2e-2026';

test.describe('PushDevicesManager (TMAIL-204)', () => {
  test.beforeAll(() => deleteMailboxByUsername(NOREPLY_CREDS.email));
  test.afterAll(() => deleteMailboxByUsername(NOREPLY_CREDS.email));

  test('lists devices, fires a test, unregisters', async ({
    page,
    apiSignup,
    takeScreenshot,
    baseURL,
  }) => {
    test.setTimeout(60_000);

    const tokens = await apiSignup(NOREPLY_CREDS.email, PASSWORD);
    const auth = { Authorization: `Bearer ${tokens.access_token}`, 'Content-Type': 'application/json' };

    // Register two synthetic devices the way the mobile app would.
    for (const platform of ['fcm', 'apns'] as const) {
      const resp = await fetch(`${baseURL}/api/push/register`, {
        method: 'POST',
        headers: auth,
        body: JSON.stringify({
          platform,
          device_token: `e2e-${platform}-${Date.now()}`,
          device_name: `E2E ${platform.toUpperCase()} test device`,
          app_version: '1.0.0',
        }),
      });
      expect(resp.status, `${platform} register status`).toBe(201);
    }

    // Open /app + navigate to Notifications via the sidebar.
    await page.goto('/login');
    await page.evaluate(([at, rt]) => {
      localStorage.setItem('access_token', at);
      localStorage.setItem('refresh_token', rt);
    }, [tokens.access_token, tokens.refresh_token]);
    await page.goto('/app');
    await expect(page.locator('button, a', { hasText: /Compose/i }).first()).toBeVisible({ timeout: 20_000 });
    await page.locator('.sidebar button:has-text("Notifications")').click();
    await expect(page.locator('h2', { hasText: 'Notifications' })).toBeVisible({ timeout: 10_000 });
    await takeScreenshot(page, 'push-devices/01-manager-loaded');

    // Both devices visible.
    const rows = page.locator('.settings-table tbody tr');
    await expect(rows).toHaveCount(2);
    await expect(page.locator('text=E2E FCM test device')).toBeVisible();
    await expect(page.locator('text=E2E APNs test device')).toBeVisible();

    // Watch /api/push/test for the response.
    let testStatus = 0;
    page.on('response', async (resp) => {
      if (resp.url().endsWith('/api/push/test') && resp.request().method() === 'POST') {
        testStatus = resp.status();
      }
    });

    // Send test notification.
    await page.locator('button', { hasText: 'Send test notification' }).click();
    await expect.poll(() => testStatus, { timeout: 10_000 }).toBe(200);
    // Result banner appears.
    await expect(page.locator('text=/Sent test to 2 device/')).toBeVisible({ timeout: 5_000 });
    await takeScreenshot(page, 'push-devices/02-test-sent');

    // Unregister the FCM device. Auto-accept the confirm() dialog. Use the
    // persistent .on listener (not .once) so the click can fire before the
    // listener is wired without losing the event.
    page.on('dialog', (dialog) => dialog.accept().catch(() => {}));
    let unregisterStatus = 0;
    page.on('response', async (resp) => {
      if (resp.request().method() === 'DELETE' && /\/api\/push\/devices\//.test(resp.url())) {
        unregisterStatus = resp.status();
      }
    });
    await page.locator('.settings-table tbody tr').filter({ hasText: 'FCM' })
      .locator('button[title="Unregister"]').click();
    await expect.poll(() => unregisterStatus, { timeout: 10_000 }).toBe(204);
    await expect(rows).toHaveCount(1, { timeout: 8_000 });
    await takeScreenshot(page, 'push-devices/03-after-unregister');
  });
});
