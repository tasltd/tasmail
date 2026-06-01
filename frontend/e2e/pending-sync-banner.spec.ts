// Added (TMAIL-88): E2E spec for the PendingSyncBanner + offline action queue.
// Verifies that:
//   1. With no queued actions, no banner is shown.
//   2. Seeding the tasmail-sync IndexedDB with actions makes the banner appear
//      with the correct count.
//   3. Toggling browser offline state changes the banner copy to "Offline —
//      N actions queued" and hides the Retry button.
//   4. Coming back online surfaces the "Syncing N actions…" + Retry button.
//   5. Clicking Retry drains the queue (banner disappears).
//
// Login is via the standard auth mock used by the rest of the suite so we
// reach AppShell without any backend dependency.
import { test, expect } from './fixtures/base';
// Fix (TMAIL-412): per-test signup emails need DB cleanup so re-runs stay
// idempotent and the e2e.tasmail accounts don't accumulate forever.
import { deleteMailboxByUsername } from './helpers/db-cleanup.js';

const SCREENSHOT_DIR = 'pending-sync';

// Fix (TMAIL-412): collect every per-test signup email so the afterAll hook
// can wipe them from the DB. Replaces the dead hardcoded loginAs path.
const pendingSyncEmails: string[] = [];

test.afterAll(() => {
  for (const email of pendingSyncEmails) {
    try {
      deleteMailboxByUsername(email);
    } catch {
      // Best-effort cleanup — don't fail the spec if the DB isn't reachable.
    }
  }
});

// Fix (TMAIL-412): provision a real BYOK account and inject its JWT pair so
// /app loads without bouncing on the first unmocked endpoint.
async function authenticate(
  page: import('@playwright/test').Page,
  apiSignup: (email: string, password: string) => Promise<{ access_token: string; refresh_token: string }>,
  slug: string,
): Promise<void> {
  const email = `pending-sync-${slug}-${Date.now()}@e2e.tasmail`;
  pendingSyncEmails.push(email);
  const tokens = await apiSignup(email, 'pending-sync-pw-2026');
  await page.goto('/login');
  await page.evaluate(([at, rt]) => {
    localStorage.setItem('access_token', at);
    localStorage.setItem('refresh_token', rt);
  }, [tokens.access_token, tokens.refresh_token]);
  await page.goto('/app');
}

// Helpers: Seed and drain the tasmail-sync IndexedDB queue from inside the page
// context. Executed via page.evaluate so they hit the same DB the production
// background-sync module writes to.
//
// Fix (TMAIL-415): the previous helpers opened the DB without any safety net.
// On Firefox, indexedDB.open() occasionally never fires onsuccess/onerror when
// another connection (e.g. PendingSyncBanner's own openSyncDB()) is mid-flight
// during page load. The Promise then hangs for the full 30s Playwright test
// timeout. Per MDN IndexedDB best practices we now:
//   1. Wire req.onblocked so a version/clear conflict rejects fast instead of
//      hanging silently.
//   2. Add a 5s safety timeout — IDB ops against a tiny test queue are sub-ms,
//      so 5s is plenty of slack while still failing fast on the Firefox hang.
//   3. Call db.close() once the transaction commits so the page's own
//      background-sync connections can re-read the queue without cascading
//      blocked-open events on subsequent helpers.
const seedQueue = (page: import('@playwright/test').Page, count: number) =>
  page.evaluate((n) => {
    return new Promise<void>((resolve, reject) => {
      const timer = setTimeout(
        () => reject(new Error('IDB seedQueue timeout after 5s — open() never resolved (TMAIL-415)')),
        5000,
      );
      const settle = (err?: unknown) => {
        clearTimeout(timer);
        if (err) reject(err);
        else resolve();
      };
      const req = indexedDB.open('tasmail-sync', 1);
      req.onblocked = () =>
        settle(new Error('IDB seedQueue blocked — another connection holds the DB (TMAIL-415)'));
      req.onupgradeneeded = () => {
        const db = req.result;
        if (!db.objectStoreNames.contains('pending-actions')) {
          db.createObjectStore('pending-actions', { keyPath: 'id', autoIncrement: true });
        }
      };
      req.onsuccess = () => {
        const db = req.result;
        try {
          const tx = db.transaction('pending-actions', 'readwrite');
          const store = tx.objectStore('pending-actions');
          for (let i = 0; i < n; i++) {
            store.add({
              type: 'send',
              payload: { to: [`user${i}@example.com`], subject: `Queued ${i}` },
              createdAt: Date.now() - (n - i) * 1000,
              retries: 0,
            });
          }
          tx.oncomplete = () => {
            db.close();
            settle();
          };
          tx.onerror = () => {
            db.close();
            settle(tx.error);
          };
        } catch (err) {
          db.close();
          settle(err);
        }
      };
      req.onerror = () => settle(req.error);
    });
  }, count);

const clearQueue = (page: import('@playwright/test').Page) =>
  page.evaluate(() => {
    return new Promise<void>((resolve, reject) => {
      const timer = setTimeout(
        () => reject(new Error('IDB clearQueue timeout after 5s (TMAIL-415)')),
        5000,
      );
      const settle = (err?: unknown) => {
        clearTimeout(timer);
        if (err) reject(err);
        else resolve();
      };
      const req = indexedDB.open('tasmail-sync', 1);
      req.onblocked = () =>
        settle(new Error('IDB clearQueue blocked — another connection holds the DB (TMAIL-415)'));
      req.onupgradeneeded = () => {
        const db = req.result;
        if (!db.objectStoreNames.contains('pending-actions')) {
          db.createObjectStore('pending-actions', { keyPath: 'id', autoIncrement: true });
        }
      };
      req.onsuccess = () => {
        const db = req.result;
        if (!db.objectStoreNames.contains('pending-actions')) {
          db.close();
          settle();
          return;
        }
        try {
          const tx = db.transaction('pending-actions', 'readwrite');
          tx.objectStore('pending-actions').clear();
          tx.oncomplete = () => {
            db.close();
            settle();
          };
          tx.onerror = () => {
            db.close();
            settle(tx.error);
          };
        } catch (err) {
          db.close();
          settle(err);
        }
      };
      req.onerror = () => settle(req.error);
    });
  });

// Fix (TMAIL-415): inline count helper used by the retry test. Same hardening
// as seedQueue/clearQueue so a stuck open() fails in 5s instead of 30s.
const countQueue = (page: import('@playwright/test').Page) =>
  page.evaluate(
    () =>
      new Promise<number>((resolve, reject) => {
        const timer = setTimeout(
          () => reject(new Error('IDB countQueue timeout after 5s (TMAIL-415)')),
          5000,
        );
        const settle = (val: number | undefined, err?: unknown) => {
          clearTimeout(timer);
          if (err) reject(err);
          else resolve(val as number);
        };
        const req = indexedDB.open('tasmail-sync', 1);
        req.onblocked = () =>
          settle(undefined, new Error('IDB countQueue blocked (TMAIL-415)'));
        req.onupgradeneeded = () => {
          const db = req.result;
          if (!db.objectStoreNames.contains('pending-actions')) {
            db.createObjectStore('pending-actions', { keyPath: 'id', autoIncrement: true });
          }
        };
        req.onsuccess = () => {
          const db = req.result;
          if (!db.objectStoreNames.contains('pending-actions')) {
            db.close();
            settle(0);
            return;
          }
          try {
            const tx = db.transaction('pending-actions', 'readonly');
            const c = tx.objectStore('pending-actions').count();
            c.onsuccess = () => {
              db.close();
              settle(c.result);
            };
            c.onerror = () => {
              db.close();
              settle(undefined, c.error);
            };
          } catch (err) {
            db.close();
            settle(undefined, err);
          }
        };
        req.onerror = () => settle(undefined, req.error);
      }),
  );

test.beforeEach(async ({ page }) => {
  // Mock the routes AppShell needs so we can reach the banner without a real
  // backend. These mirror the compose.spec.ts patterns.
  await page.route('**/api/auth/login', async (route) => {
    await route.fulfill({
      status: 200,
      contentType: 'application/json',
      body: JSON.stringify({ access_token: 'mock-access', refresh_token: 'mock-refresh' }),
    });
  });
  await page.route('**/api/folders', async (route) => {
    await route.fulfill({
      status: 200,
      contentType: 'application/json',
      body: JSON.stringify([{ name: 'INBOX', unseen: 0 }]),
    });
  });
  await page.route('**/api/oidc/providers/login', async (route) => {
    await route.fulfill({ status: 200, contentType: 'application/json', body: '[]' });
  });
  await page.route('**/api/quota', async (route) => {
    await route.fulfill({
      status: 200,
      contentType: 'application/json',
      body: JSON.stringify({ used: 0, limit: 1000 }),
    });
  });
  await page.route('**/api/folders/*/messages*', async (route) => {
    await route.fulfill({
      status: 200,
      contentType: 'application/json',
      body: JSON.stringify({ messages: [], total: 0 }),
    });
  });
  await page.route('**/api/messages/schedule', async (route) => {
    await route.fulfill({
      status: 200,
      contentType: 'application/json',
      body: JSON.stringify({ id: 'sched-1', cancel_token: 'tok' }),
    });
  });
});

test.describe('PendingSyncBanner (TMAIL-88)', () => {
  test('hides when queue is empty', async ({ page, apiSignup, takeScreenshot }) => {
    await authenticate(page, apiSignup, 'empty');
    await clearQueue(page);
    // NOTE: Force a re-render by toggling a no-op route so the banner subscription
    // sees the cleared queue; in practice the banner is mounted at AppShell load.
    await page.waitForTimeout(300);

    await takeScreenshot(page, `${SCREENSHOT_DIR}/empty-queue-no-banner`);
    expect(await page.locator('[data-testid="pending-sync-banner"]').count()).toBe(0);
  });

  test('shows "Syncing N actions" banner with Retry button when online with queue', async ({
    page,
    apiSignup,
    takeScreenshot,
  }) => {
    await authenticate(page, apiSignup, 'syncing');
    await clearQueue(page);

    await seedQueue(page, 3);
    // Trigger a re-render so the subscription picks up the seed (production code
    // wires this through enqueue → emitChange; the manual seed bypasses that).
    await page.reload();
    await page.waitForURL(/\/app/, { timeout: 15_000 });

    const banner = page.locator('[data-testid="pending-sync-banner"]');
    await expect(banner).toBeVisible();
    await expect(banner).toContainText('Pending sync:');
    await expect(banner).toContainText('Syncing 3 actions');
    await expect(page.locator('[data-testid="pending-sync-retry"]')).toBeVisible();

    await takeScreenshot(page, `${SCREENSHOT_DIR}/online-3-actions-syncing`);
  });

  test('shows "Offline — N actions queued" banner without Retry button when offline', async ({
    page,
    context,
    apiSignup,
    takeScreenshot,
  }) => {
    // Fix (TMAIL-415): override the default /api/messages/schedule mock so it
    // returns 503 for this test only. Otherwise the post-reload auto-replay
    // (useOnlineStatus -> processPending) drains the seeded action before we
    // can flip the context offline, and the banner never mounts. Returning 503
    // keeps the queued action in IndexedDB (executeAction throws, retry count
    // is incremented, action stays) so the banner stays at count=1 across the
    // online→offline flip.
    await page.route('**/api/messages/schedule', async (route) => {
      await route.fulfill({
        status: 503,
        contentType: 'application/json',
        body: JSON.stringify({ error: 'unavailable' }),
      });
    });

    await authenticate(page, apiSignup, 'offline');
    await clearQueue(page);

    await seedQueue(page, 1);
    // Fix (TMAIL-415): reload BEFORE going offline. Firefox refuses to satisfy
    // page.reload() while the context is offline (NS_ERROR_OFFLINE). The banner
    // re-renders reactively via useOnlineStatus (window 'offline' event) so no
    // reload is needed to flip the copy from "Syncing" to "Offline".
    await page.reload();
    await page.waitForURL(/\/app/, { timeout: 15_000 });
    // Wait until the banner has actually picked up the seeded count (online
    // copy first), so the subsequent setOffline triggers a re-render rather
    // than racing against the initial mount.
    const banner = page.locator('[data-testid="pending-sync-banner"]');
    await expect(banner).toBeVisible({ timeout: 10_000 });
    await expect(banner).toContainText('Syncing 1 action');
    await context.setOffline(true);

    await expect(banner).toContainText('Offline');
    await expect(banner).toContainText('1 action queued');
    expect(await page.locator('[data-testid="pending-sync-retry"]').count()).toBe(0);

    await takeScreenshot(page, `${SCREENSHOT_DIR}/offline-1-action-queued`);

    // Cleanup: restore online so the next test isn't poisoned
    await context.setOffline(false);
  });

  test('shows singular "1 action" copy (not "1 actions")', async ({
    page,
    apiSignup,
    takeScreenshot,
  }) => {
    await authenticate(page, apiSignup, 'singular');
    await clearQueue(page);
    await seedQueue(page, 1);
    await page.reload();
    await page.waitForURL(/\/app/, { timeout: 15_000 });

    const banner = page.locator('[data-testid="pending-sync-banner"]');
    await expect(banner).toBeVisible();
    await expect(banner).toContainText('Syncing 1 action');
    // Explicitly assert no plural-s on the count
    await expect(banner).not.toContainText('Syncing 1 actions');

    await takeScreenshot(page, `${SCREENSHOT_DIR}/singular-action-copy`);
  });

  test('Retry button drains the queue and hides the banner', async ({
    page,
    apiSignup,
    takeScreenshot,
  }) => {
    await authenticate(page, apiSignup, 'retry');
    await clearQueue(page);
    await seedQueue(page, 2);
    await page.reload();
    await page.waitForURL(/\/app/, { timeout: 15_000 });

    const banner = page.locator('[data-testid="pending-sync-banner"]');
    await expect(banner).toBeVisible();
    await expect(banner).toContainText('Syncing 2 actions');
    await takeScreenshot(page, `${SCREENSHOT_DIR}/retry-before-click`);

    // Capture pre-action queue size (TMAIL-415: use hardened countQueue helper)
    const pre = await countQueue(page);
    expect(pre).toBe(2);

    await page.locator('[data-testid="pending-sync-retry"]').click();
    await takeScreenshot(page, `${SCREENSHOT_DIR}/retry-clicked`);

    // Banner should disappear once queue drains (the mocked /api/messages/schedule
    // returns 200, so both actions succeed and are removed).
    await expect(banner).toBeHidden({ timeout: 10_000 });

    const post = await countQueue(page);
    expect(post).toBe(0);

    await takeScreenshot(page, `${SCREENSHOT_DIR}/retry-after-drain`);
  });
});
