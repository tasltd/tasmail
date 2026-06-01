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
const seedQueue = (page: import('@playwright/test').Page, count: number) =>
  page.evaluate((n) => {
    return new Promise<void>((resolve, reject) => {
      const req = indexedDB.open('tasmail-sync', 1);
      req.onupgradeneeded = () => {
        const db = req.result;
        if (!db.objectStoreNames.contains('pending-actions')) {
          db.createObjectStore('pending-actions', { keyPath: 'id', autoIncrement: true });
        }
      };
      req.onsuccess = () => {
        const db = req.result;
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
        tx.oncomplete = () => resolve();
        tx.onerror = () => reject(tx.error);
      };
      req.onerror = () => reject(req.error);
    });
  }, count);

const clearQueue = (page: import('@playwright/test').Page) =>
  page.evaluate(() => {
    return new Promise<void>((resolve, reject) => {
      const req = indexedDB.open('tasmail-sync', 1);
      req.onsuccess = () => {
        const db = req.result;
        if (!db.objectStoreNames.contains('pending-actions')) {
          resolve();
          return;
        }
        const tx = db.transaction('pending-actions', 'readwrite');
        tx.objectStore('pending-actions').clear();
        tx.oncomplete = () => resolve();
        tx.onerror = () => reject(tx.error);
      };
      req.onerror = () => reject(req.error);
    });
  });

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
    await authenticate(page, apiSignup, 'offline');
    await clearQueue(page);

    await seedQueue(page, 1);
    await context.setOffline(true);
    await page.reload();
    await page.waitForURL(/\/app/, { timeout: 15_000 }).catch(() => {});

    const banner = page.locator('[data-testid="pending-sync-banner"]');
    await expect(banner).toBeVisible({ timeout: 10_000 });
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

    // Capture pre-action queue size
    const pre = await page.evaluate(() => {
      return new Promise<number>((resolve, reject) => {
        const req = indexedDB.open('tasmail-sync', 1);
        req.onsuccess = () => {
          const tx = req.result.transaction('pending-actions', 'readonly');
          const c = tx.objectStore('pending-actions').count();
          c.onsuccess = () => resolve(c.result);
          c.onerror = () => reject(c.error);
        };
      });
    });
    expect(pre).toBe(2);

    await page.locator('[data-testid="pending-sync-retry"]').click();
    await takeScreenshot(page, `${SCREENSHOT_DIR}/retry-clicked`);

    // Banner should disappear once queue drains (the mocked /api/messages/schedule
    // returns 200, so both actions succeed and are removed).
    await expect(banner).toBeHidden({ timeout: 10_000 });

    const post = await page.evaluate(() => {
      return new Promise<number>((resolve, reject) => {
        const req = indexedDB.open('tasmail-sync', 1);
        req.onsuccess = () => {
          const tx = req.result.transaction('pending-actions', 'readonly');
          const c = tx.objectStore('pending-actions').count();
          c.onsuccess = () => resolve(c.result);
          c.onerror = () => reject(c.error);
        };
      });
    });
    expect(post).toBe(0);

    await takeScreenshot(page, `${SCREENSHOT_DIR}/retry-after-drain`);
  });
});
