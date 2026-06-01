// Added: Search E2E specs — TopBar search bar, results page, empty + populated states (TMAIL-36)
// Covers the "search" requirement from TMAIL-36.
import { test, expect } from './fixtures/base';
// Fix (TMAIL-412): per-test signup emails need DB cleanup so re-runs stay
// idempotent and the e2e.tasmail accounts don't accumulate forever.
import { deleteMailboxByUsername } from './helpers/db-cleanup.js';

// Fix (TMAIL-412): collect every per-test signup email so the afterAll hook
// can wipe them from the DB. Replaces the dead hardcoded loginAs path.
const searchEmails: string[] = [];

test.afterAll(() => {
  for (const email of searchEmails) {
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
  const email = `search-${slug}-${Date.now()}@e2e.tasmail`;
  searchEmails.push(email);
  const tokens = await apiSignup(email, 'search-pw-2026');
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
        { name: 'INBOX', unseen: 0 },
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

test.describe('Mail search', () => {
  test('search input is visible in the top bar after login', async ({
    page,
    apiSignup,
    takeScreenshot,
  }) => {
    await authenticate(page, apiSignup, 'input-visible');

    const searchInput = page.locator(
      '.topbar__search input[placeholder="Search emails..."]',
    );
    await expect(searchInput).toBeVisible();

    await takeScreenshot(page, 'search/topbar-search-visible');
  });

  test('submitting a query renders matching results', async ({
    page,
    apiSignup,
    takeScreenshot,
  }) => {
    await authenticate(page, apiSignup, 'results');

    // Fix (TMAIL-418): mock must match SearchResponse shape — { messages, total,
    // query, folder } — and each MessageEnvelope needs `flags`/`size` because
    // the SearchRow component reads `message.flags.some(...)`.
    await page.route('**/api/search**', async (route) => {
      await route.fulfill({
        status: 200,
        contentType: 'application/json',
        body: JSON.stringify({
          messages: [
            {
              uid: 501,
              subject: 'Quarterly report Q1 2026',
              from: 'reports@techatscale.io',
              date: '2026-04-02T10:00:00Z',
              flags: [],
              size: 1024,
            },
            {
              uid: 502,
              subject: 'Q1 2026 budget review',
              from: 'finance@techatscale.io',
              date: '2026-04-04T11:30:00Z',
              flags: ['\\Seen'],
              size: 2048,
            },
          ],
          total: 2,
          query: 'Q1 2026',
          folder: 'INBOX',
        }),
      });
    });

    const searchInput = page.locator(
      '.topbar__search input[placeholder="Search emails..."]',
    );
    await searchInput.fill('Q1 2026');
    await searchInput.press('Enter');

    // Fix (TMAIL-418): SearchResults renders as `.message-list` with `.message-row`
    // children (consistent with the regular folder list) — the legacy
    // `.search-results` markup was dropped during a refactor.
    const results = page.locator('.message-list');
    await expect(results).toBeVisible();
    await expect(results.locator('.message-row').first()).toContainText('Q1 2026');
    await expect(results.locator('.message-row')).toHaveCount(2);

    await takeScreenshot(page, 'search/results-populated');
  });

  test('submitting a query with zero matches shows the empty state', async ({
    page,
    apiSignup,
    takeScreenshot,
  }) => {
    await authenticate(page, apiSignup, 'empty');

    // Fix (TMAIL-418): SearchResponse shape — { messages, total, query, folder }.
    await page.route('**/api/search**', async (route) => {
      await route.fulfill({
        status: 200,
        contentType: 'application/json',
        body: JSON.stringify({
          messages: [],
          total: 0,
          query: 'xyzzy-no-match',
          folder: 'INBOX',
        }),
      });
    });

    const searchInput = page.locator(
      '.topbar__search input[placeholder="Search emails..."]',
    );
    await searchInput.fill('xyzzy-no-match');
    await searchInput.press('Enter');

    // Fix (TMAIL-418): SearchResults renders "No messages match your search"
    // inside `<div className="message-list__empty">` — the legacy
    // `.search-results__empty` / `.empty-state` markup was dropped.
    const empty = page.locator('.message-list__empty');
    await expect(empty).toBeVisible();
    await expect(empty).toContainText('No messages match');

    await takeScreenshot(page, 'search/empty-state');
  });

  test('clearing the search input restores the folder view', async ({
    page,
    apiSignup,
    takeScreenshot,
  }) => {
    await authenticate(page, apiSignup, 'clear');

    // Fix (TMAIL-418): SearchResponse shape — { messages, total, query, folder }.
    await page.route('**/api/search**', async (route) => {
      await route.fulfill({
        status: 200,
        contentType: 'application/json',
        body: JSON.stringify({
          messages: [],
          total: 0,
          query: 'temporary query',
          folder: 'INBOX',
        }),
      });
    });

    const searchInput = page.locator(
      '.topbar__search input[placeholder="Search emails..."]',
    );
    await searchInput.fill('temporary query');
    await searchInput.press('Enter');

    // Fix (TMAIL-418): SearchResults rendered list uses `.message-list` and the
    // header line tells us we are looking at search ("N results for ...").
    const resultsList = page.locator('.message-list');
    await expect(resultsList).toBeVisible();
    await expect(resultsList.locator('.message-list__header')).toContainText(
      'results for "temporary query"',
    );
    await takeScreenshot(page, 'search/before-clear');

    // Fix (TMAIL-418): the explicit "Clear search" button in the SearchResults
    // header is the user-facing way to restore the folder view (the TopBar
    // `handleSearch` ignores empty submissions, so pressing Enter on an empty
    // input is a no-op). Click the X to actually reset searchQuery + viewMode.
    await page.locator('button[title="Clear search"]').click();

    // After clearing, viewMode flips back to 'list' so MessageList renders.
    // The `**/api/folders/*/messages*` route in beforeEach returns
    // `{ messages: [], total: 0 }`, and INBOX with zero messages renders the
    // EmptyInboxState (TMAIL-401) with data-testid="empty-inbox-state".
    await expect(page.locator('[data-testid="empty-inbox-state"]')).toBeVisible();
    // And the search header copy must be gone.
    await expect(page.locator('.message-list__header')).toHaveCount(0);
    await takeScreenshot(page, 'search/after-clear');
  });
});
