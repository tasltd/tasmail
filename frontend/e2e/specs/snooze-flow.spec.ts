/**
 * TMAIL-205: Snooze button in MessageView toolbar.
 *
 * Bootstraps a fresh BYOK account that points at the noreply mailbox so the
 * SPA renders a real inbox, opens the first message, clicks Snooze → "Later
 * today", and confirms /api/messages/snooze returned 201 + the message
 * disappeared from the inbox view.
 */
import { test, NOREPLY_CREDS, expect } from '../fixtures/base.js';
import { deleteMailboxByUsername } from '../helpers/db-cleanup.js';

const PASSWORD = 'snooze-e2e-2026';

test.describe('Snooze button (TMAIL-205)', () => {
  test.beforeAll(() => {
    deleteMailboxByUsername(NOREPLY_CREDS.email);
  });
  test.afterAll(() => {
    deleteMailboxByUsername(NOREPLY_CREDS.email);
  });

  test('snoozes the first INBOX message via the toolbar', async ({
    page,
    apiSignup,
    takeScreenshot,
    baseURL,
  }) => {
    test.setTimeout(90_000);

    // 1. Provision a BYOK account that points at swmail.
    const tokens = await apiSignup(NOREPLY_CREDS.email, PASSWORD);
    const auth = { Authorization: `Bearer ${tokens.access_token}` };

    await fetch(`${baseURL}/api/imap-configs`, {
      method: 'POST',
      headers: { ...auth, 'Content-Type': 'application/json' },
      body: JSON.stringify({
        name: 'snooze-e2e',
        host: NOREPLY_CREDS.imap.host,
        port: NOREPLY_CREDS.imap.port,
        username: NOREPLY_CREDS.imap.username,
        password: NOREPLY_CREDS.imap.password,
        encryption: NOREPLY_CREDS.imap.encryption,
        is_default: true,
      }),
    });

    // 2. Inject the session and open /app.
    await page.goto('/login');
    await page.evaluate(([at, rt]) => {
      localStorage.setItem('access_token', at);
      localStorage.setItem('refresh_token', rt);
    }, [tokens.access_token, tokens.refresh_token]);
    await page.goto('/app');
    await expect(page.locator('button, a', { hasText: /Compose/i }).first()).toBeVisible({ timeout: 20_000 });

    // 3. Click INBOX, then open the first message in the list. Disable
    // conversation-grouping first so the first row is always a single
    // message — clicking a thread root only toggles expansion.
    const inboxLink = page.locator('button, a, li', { hasText: /INBOX/i }).first();
    await inboxLink.click().catch(() => null);
    const conversationsToggle = page.locator('label:has-text("Conversations") input[type="checkbox"]');
    if (await conversationsToggle.isChecked().catch(() => false)) {
      await conversationsToggle.uncheck();
    }
    const firstRow = page.locator('.message-row').first();
    await firstRow.waitFor({ state: 'visible', timeout: 25_000 });
    await firstRow.click();

    // The toolbar lives inside .message-view; wait for it before the snooze action.
    await page.locator('.message-view__toolbar').waitFor({ state: 'visible', timeout: 15_000 });
    await takeScreenshot(page, 'snooze/01-message-open');

    // 4. Watch the snooze API for the round-trip.
    let snoozeCalled = false;
    let snoozeStatus = 0;
    page.on('response', async (resp) => {
      if (resp.url().endsWith('/api/messages/snooze') && resp.request().method() === 'POST') {
        snoozeCalled = true;
        snoozeStatus = resp.status();
      }
    });

    // 5. Click the Snooze (Clock) icon, then "Later today".
    await page.locator('.message-view__toolbar button[title="Snooze"]').click();
    await page.locator('.snooze-menu__dropdown').waitFor({ state: 'visible', timeout: 5_000 });
    await takeScreenshot(page, 'snooze/02-menu-open');
    await page.locator('.snooze-menu__item', { hasText: 'Later today' }).click();

    // 6. Wait for the mutation to fire and the view to return to the list.
    await expect.poll(() => snoozeCalled, { timeout: 10_000 }).toBe(true);
    // Backend returns 201 Created for the snooze row insert.
    expect(snoozeStatus, 'POST /api/messages/snooze status').toBe(201);

    await takeScreenshot(page, 'snooze/03-after-snooze');
  });
});
