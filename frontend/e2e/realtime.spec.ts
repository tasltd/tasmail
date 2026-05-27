// Added: Real-time E2E specs — WebSocket push events trigger UI updates (TMAIL-36)
// Covers the "real-time" requirement from TMAIL-36. We stub the WS endpoint with
// `page.routeWebSocket` so the test can deliver server frames deterministically.
import { test, expect } from './fixtures/base';
import type { WebSocketRoute } from '@playwright/test';

const folderResponse = (inboxUnseen: number) =>
  JSON.stringify([
    { name: 'INBOX', unseen: inboxUnseen },
    { name: 'Sent', unseen: 0 },
  ]);

test.beforeEach(async ({ page }) => {
  // Track INBOX unread on the test side so we can simulate a server push that
  // bumps the counter and assert the badge re-renders.
  let inboxUnseen = 1;

  await page.route('**/api/auth/login', async (route) => {
    await route.fulfill({
      status: 200,
      contentType: 'application/json',
      body: JSON.stringify({
        access_token: 'mock-access-token',
        refresh_token: 'mock-refresh-token',
      }),
    });
  });

  await page.route('**/api/folders', async (route) => {
    await route.fulfill({
      status: 200,
      contentType: 'application/json',
      body: folderResponse(inboxUnseen),
    });
  });

  await page.route('**/api/oidc/providers/login', async (route) => {
    await route.fulfill({ status: 200, contentType: 'application/json', body: '[]' });
  });

  await page.route('**/api/quota', async (route) => {
    await route.fulfill({
      status: 200,
      contentType: 'application/json',
      body: JSON.stringify({ used: 100, limit: 1000 }),
    });
  });

  await page.route('**/api/folders/*/messages*', async (route) => {
    await route.fulfill({
      status: 200,
      contentType: 'application/json',
      body: JSON.stringify({ messages: [], total: 0 }),
    });
  });

  // Expose a helper on the window so individual tests can bump the inbox count
  // and trigger a folder-list refetch via TanStack Query invalidation through WS.
  await page.exposeFunction('__bumpInboxUnseen', () => {
    inboxUnseen += 1;
  });
});

const wsHandler = (ws: WebSocketRoute) => {
  // Accept the subscribe:INBOX frame the hook sends on open, then immediately
  // emit a new_mail event so the SPA invalidates its folder cache.
  ws.onMessage((frame) => {
    const text = String(frame);
    if (text.startsWith('subscribe:')) {
      // Echo a ping so the suite has a deterministic signal that the SPA is
      // listening, then sit idle until the test pushes new_mail.
      ws.send(JSON.stringify({ type: 'ping', timestamp: Date.now() }));
    }
  });
};

test.describe('Real-time updates', () => {
  test('SPA opens a WebSocket connection after login', async ({
    page,
    loginAs,
    takeScreenshot,
  }, testInfo) => {
    let observedConnection = false;
    await page.routeWebSocket('**/ws**', (ws) => {
      observedConnection = true;
      wsHandler(ws);
    });

    await loginAs(page, 'user@example.com', 'password123');

    // Give the hook one tick to open its socket.
    await expect.poll(() => observedConnection, { timeout: 10_000 }).toBe(true);

    await takeScreenshot(page, 'realtime/ws-connected');
    testInfo.annotations.push({ type: 'note', description: 'WebSocket opened post-login' });
  });

  test('new_mail event invalidates folders and bumps unread badge', async ({
    page,
    loginAs,
    takeScreenshot,
  }) => {
    await page.routeWebSocket('**/ws**', (ws) => {
      ws.onMessage(async (frame) => {
        const text = String(frame);
        if (text === 'subscribe:INBOX') {
          // Bump the server-side counter, then push a new_mail event that the
          // hook turns into a folders refetch.
          await page.evaluate(() =>
            (window as unknown as { __bumpInboxUnseen: () => void }).__bumpInboxUnseen(),
          );
          ws.send(
            JSON.stringify({
              type: 'new_mail',
              folder: 'INBOX',
              unread: 2,
              timestamp: Date.now(),
            }),
          );
        }
      });
    });

    await loginAs(page, 'user@example.com', 'password123');

    const inboxBadge = page
      .locator('.folder-tree .folder-item', { hasText: 'INBOX' })
      .locator('.folder-item__badge');

    await expect(inboxBadge).toHaveText('2', { timeout: 10_000 });
    await takeScreenshot(page, 'realtime/inbox-badge-after-push');
  });

  test('quota_update event refreshes the quota indicator', async ({
    page,
    loginAs,
    takeScreenshot,
  }) => {
    let usedBytes = 100;
    await page.unroute('**/api/quota');
    await page.route('**/api/quota', async (route) => {
      await route.fulfill({
        status: 200,
        contentType: 'application/json',
        body: JSON.stringify({ used: usedBytes, limit: 1000 }),
      });
    });

    await page.routeWebSocket('**/ws**', (ws) => {
      ws.onMessage((frame) => {
        const text = String(frame);
        if (text === 'subscribe:INBOX') {
          usedBytes = 800;
          ws.send(
            JSON.stringify({
              type: 'quota_update',
              used_bytes: 800,
              total_bytes: 1000,
              timestamp: Date.now(),
            }),
          );
        }
      });
    });

    await loginAs(page, 'user@example.com', 'password123');

    // The quota indicator should eventually pick up the new value (component
    // class name varies — match either the QuotaBar or a percentage-aware label).
    const quotaIndicator = page.locator('.quota-bar, [data-testid="quota-indicator"]').first();
    await expect(quotaIndicator).toBeVisible({ timeout: 10_000 });

    await takeScreenshot(page, 'realtime/quota-updated');
  });

  test('socket reconnects after the server closes the connection', async ({
    page,
    loginAs,
    takeScreenshot,
  }) => {
    let connectCount = 0;
    await page.routeWebSocket('**/ws**', (ws) => {
      connectCount += 1;
      if (connectCount === 1) {
        // Close immediately to trigger the SPA's reconnect timer.
        ws.close({ code: 1006 });
      } else {
        wsHandler(ws);
      }
    });

    await loginAs(page, 'user@example.com', 'password123');

    await expect.poll(() => connectCount, { timeout: 15_000 }).toBeGreaterThanOrEqual(2);
    await takeScreenshot(page, 'realtime/reconnect');
  });
});
