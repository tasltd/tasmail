// Added: Mobile-responsive E2E specs (TMAIL-36)
// Covers the "mobile responsive" requirement: sidebar overlay behaviour,
// hamburger toggle, and navigation auto-close on small viewports.
// See AppShell.tsx + Sidebar.tsx (TMAIL-33) for the implementation under test.
import { test, expect } from './fixtures/base';

// iPhone-ish portrait viewport — well below the desktop breakpoint used by
// useResponsive, so the SPA renders in mobile mode.
const MOBILE_VIEWPORT = { width: 390, height: 844 };

test.use({ viewport: MOBILE_VIEWPORT });

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
});

test.describe('Mobile responsive layout', () => {
  test('app shell renders on mobile width without horizontal overflow', async ({
    page,
    loginAs,
    takeScreenshot,
  }) => {
    await loginAs(page, 'user@example.com', 'password123');

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
    loginAs,
    takeScreenshot,
  }) => {
    await loginAs(page, 'user@example.com', 'password123');

    const sidebarToggle = page.locator('[data-testid="sidebar-toggle"]');
    await expect(sidebarToggle).toBeVisible();

    await takeScreenshot(page, 'mobile/topbar-with-toggle');
  });

  test('tapping a folder in the mobile sidebar closes the overlay', async ({
    page,
    loginAs,
    takeScreenshot,
  }) => {
    await loginAs(page, 'user@example.com', 'password123');

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
    loginAs,
    takeScreenshot,
  }) => {
    await loginAs(page, 'user@example.com', 'password123');

    const overlay = page.locator('[data-testid="sidebar-overlay"]');
    if (!(await overlay.isVisible().catch(() => false))) {
      await page.locator('[data-testid="sidebar-toggle"]').click();
    }
    await expect(overlay).toBeVisible();

    await overlay.click({ position: { x: 5, y: 5 } });
    await expect(overlay).toHaveCount(0);

    await takeScreenshot(page, 'mobile/sidebar-dismissed-via-backdrop');
  });
});
