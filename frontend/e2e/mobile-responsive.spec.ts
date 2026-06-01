// Added: Mobile-responsive E2E specs (TMAIL-36)
// Covers the "mobile responsive" requirement: sidebar overlay behaviour,
// hamburger toggle, and navigation auto-close on small viewports.
// See AppShell.tsx + Sidebar.tsx (TMAIL-33) for the implementation under test.
import { test, expect } from './fixtures/base';
// Fix (TMAIL-412): per-test signup emails need DB cleanup so re-runs stay
// idempotent and the e2e.tasmail accounts don't accumulate forever.
import { deleteMailboxByUsername } from './helpers/db-cleanup.js';

// iPhone-ish portrait viewport — well below the desktop breakpoint used by
// useResponsive, so the SPA renders in mobile mode.
const MOBILE_VIEWPORT = { width: 390, height: 844 };

test.use({ viewport: MOBILE_VIEWPORT });

// Fix (TMAIL-412): collect every per-test signup email so the afterAll hook
// can wipe them from the DB. Replaces the dead hardcoded loginAs path.
const mobileEmails: string[] = [];

test.afterAll(() => {
  for (const email of mobileEmails) {
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
  const email = `mobile-${slug}-${Date.now()}@e2e.tasmail`;
  mobileEmails.push(email);
  const tokens = await apiSignup(email, 'mobile-test-pw-2026');
  await page.goto('/login');
  await page.evaluate(([at, rt]) => {
    localStorage.setItem('access_token', at);
    localStorage.setItem('refresh_token', rt);
  }, [tokens.access_token, tokens.refresh_token]);
  await page.goto('/app');
}

test.beforeEach(async ({ page }) => {
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
      body: JSON.stringify([
        { name: 'INBOX', unseen: 3 },
        { name: 'Sent', unseen: 0 },
      ]),
    });
  });

  await page.route('**/api/oidc/providers/login', async (route) => {
    await route.fulfill({ status: 200, contentType: 'application/json', body: '[]' });
  });

  await page.route('**/api/quota', async (route) => {
    // Fix (TMAIL-417): real QuotaStatus shape so QuotaBar doesn't render "NaN".
    await route.fulfill({
      status: 200,
      contentType: 'application/json',
      body: JSON.stringify({
        mailbox_id: 'e2e-mailbox',
        quota_bytes: 1073741824,
        used_bytes: 104857600,
        message_count: 0,
        usage_percent: 10,
        quota_warn_percent: 80,
        is_over_quota: false,
        is_warning: false,
        last_synced_at: null,
      }),
    });
  });

  await page.route('**/api/folders/*/messages*', async (route) => {
    await route.fulfill({
      status: 200,
      contentType: 'application/json',
      body: JSON.stringify({ messages: [], total: 0 }),
    });
  });
});

test.describe('Mobile responsive layout', () => {
  test('app shell renders on mobile width without horizontal overflow', async ({
    page,
    apiSignup,
    takeScreenshot,
  }) => {
    await authenticate(page, apiSignup, 'overflow');

    const appShell = page.locator('.app-shell');
    await expect(appShell).toBeVisible();

    // No content should extend beyond the viewport width on mobile.
    const overflow = await page.evaluate(
      () => document.documentElement.scrollWidth - document.documentElement.clientWidth,
    );
    expect(overflow).toBeLessThanOrEqual(1);

    await takeScreenshot(page, 'mobile/app-shell-mobile-width');
  });

  test('topbar sidebar-toggle button is visible on mobile', async ({
    page,
    apiSignup,
    takeScreenshot,
  }) => {
    await authenticate(page, apiSignup, 'toggle');

    const sidebarToggle = page.locator('[data-testid="sidebar-toggle"]');
    await expect(sidebarToggle).toBeVisible();

    await takeScreenshot(page, 'mobile/topbar-with-toggle');
  });

  test('tapping a folder in the mobile sidebar closes the overlay', async ({
    page,
    apiSignup,
    takeScreenshot,
  }) => {
    await authenticate(page, apiSignup, 'fold-tap');

    // Open the sidebar overlay if it is not already open on mobile.
    const overlay = page.locator('[data-testid="sidebar-overlay"]');
    if (!(await overlay.isVisible().catch(() => false))) {
      await page.locator('[data-testid="sidebar-toggle"]').click();
    }
    await expect(overlay).toBeVisible();
    await takeScreenshot(page, 'mobile/sidebar-overlay-open');

    // Tapping a folder should navigate AND close the overlay (TMAIL-33 behaviour).
    await page
      .locator('.folder-tree .folder-item', { hasText: 'Sent' })
      .click();
    await expect(overlay).toHaveCount(0);

    await takeScreenshot(page, 'mobile/sidebar-overlay-closed-after-nav');
  });

  test('tapping the overlay backdrop dismisses the sidebar', async ({
    page,
    apiSignup,
    takeScreenshot,
  }) => {
    await authenticate(page, apiSignup, 'backdrop');

    const overlay = page.locator('[data-testid="sidebar-overlay"]');
    if (!(await overlay.isVisible().catch(() => false))) {
      await page.locator('[data-testid="sidebar-toggle"]').click();
    }
    await expect(overlay).toBeVisible();

    // Fix (TMAIL-414): the sidebar drawer occupies the left ~300px of the
    // viewport on mobile (80% width, max 300px). Clicking the overlay at
    // (5,5) lands inside the sidebar's bounding box and is intercepted by
    // <aside class="sidebar"> instead of the backdrop. Click well to the
    // right of the drawer to hit the real backdrop region.
    await overlay.click({ position: { x: 360, y: 100 } });
    await expect(overlay).toHaveCount(0);

    await takeScreenshot(page, 'mobile/sidebar-dismissed-via-backdrop');
  });
});
