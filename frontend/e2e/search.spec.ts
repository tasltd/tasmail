// Added: Search E2E specs — TopBar search bar, results page, empty + populated states (TMAIL-36)
// Covers the "search" requirement from TMAIL-36.
import { test, expect } from './fixtures/base';

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
        { name: 'INBOX', unseen: 0 },
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

test.describe('Mail search', () => {
  test('search input is visible in the top bar after login', async ({
    page,
    loginAs,
    takeScreenshot,
  }) => {
    await loginAs(page, 'user@example.com', 'password123');

    const searchInput = page.locator(
      '.topbar__search input[placeholder="Search emails..."]',
    );
    await expect(searchInput).toBeVisible();

    await takeScreenshot(page, 'search/topbar-search-visible');
  });

  test('submitting a query renders matching results', async ({
    page,
    loginAs,
    takeScreenshot,
  }) => {
    await loginAs(page, 'user@example.com', 'password123');

    await page.route('**/api/search**', async (route) => {
      await route.fulfill({
        status: 200,
        contentType: 'application/json',
        body: JSON.stringify({
          results: [
            {
              uid: 501,
              folder: 'INBOX',
              subject: 'Quarterly report Q1 2026',
              from: 'reports@techatscale.io',
              date: '2026-04-02T10:00:00Z',
              snippet: 'Attached is the Q1 2026 report …',
            },
            {
              uid: 502,
              folder: 'INBOX',
              subject: 'Q1 2026 budget review',
              from: 'finance@techatscale.io',
              date: '2026-04-04T11:30:00Z',
              snippet: 'Please review the Q1 2026 budget …',
            },
          ],
          total: 2,
        }),
      });
    });

    const searchInput = page.locator(
      '.topbar__search input[placeholder="Search emails..."]',
    );
    await searchInput.fill('Q1 2026');
    await searchInput.press('Enter');

    const results = page.locator('.search-results');
    await expect(results).toBeVisible();
    await expect(results.locator('.search-results__item').first()).toContainText('Q1 2026');
    await expect(results.locator('.search-results__item')).toHaveCount(2);

    await takeScreenshot(page, 'search/results-populated');
  });

  test('submitting a query with zero matches shows the empty state', async ({
    page,
    loginAs,
    takeScreenshot,
  }) => {
    await loginAs(page, 'user@example.com', 'password123');

    await page.route('**/api/search**', async (route) => {
      await route.fulfill({
        status: 200,
        contentType: 'application/json',
        body: JSON.stringify({ results: [], total: 0 }),
      });
    });

    const searchInput = page.locator(
      '.topbar__search input[placeholder="Search emails..."]',
    );
    await searchInput.fill('xyzzy-no-match');
    await searchInput.press('Enter');

    const empty = page.locator('.search-results__empty, .search-results .empty-state').first();
    await expect(empty).toBeVisible();

    await takeScreenshot(page, 'search/empty-state');
  });

  test('clearing the search input restores the folder view', async ({
    page,
    loginAs,
    takeScreenshot,
  }) => {
    await loginAs(page, 'user@example.com', 'password123');

    await page.route('**/api/search**', async (route) => {
      await route.fulfill({
        status: 200,
        contentType: 'application/json',
        body: JSON.stringify({ results: [], total: 0 }),
      });
    });

    const searchInput = page.locator(
      '.topbar__search input[placeholder="Search emails..."]',
    );
    await searchInput.fill('temporary query');
    await searchInput.press('Enter');
    await expect(page.locator('.search-results')).toBeVisible();
    await takeScreenshot(page, 'search/before-clear');

    await searchInput.fill('');
    await searchInput.press('Enter');

    await expect(page.locator('.message-list, .empty-folder')).toBeVisible();
    await takeScreenshot(page, 'search/after-clear');
  });
});
