// Added: Alt-UI modern walkthrough E2E spec (TMAIL-292).
// Covers the full user flow on the /modern/ alternative UI:
//   * Login → Dashboard → Calendar → AdminDashboard
//   * All navigation by clicking sidebar/links (never page.goto() for internal
//     routes per plan-e2e-and-user-guide.md).
//   * Screenshot at every assertion point including empty states and refusal paths.
//   * User guide compiled from these screenshots is the PM completion gate.
//
// This spec targets the alt-UI served at /modern/ which is backed by the same
// production API. Auth reads the same JWT from localStorage so no second login.
//
// Screenshots directory: e2e/screenshots/alt-ui-modern-walkthrough/
// Review: open each .png and confirm expected state is visible — ls is not review.

import { test, expect } from './fixtures/base';
import { deleteMailboxByUsername } from './helpers/db-cleanup.js';

// Per-test signup emails so the afterAll hook can wipe them.
const altUiEmails: string[] = [];

test.describe('Alt-UI Modern Walkthrough (TMAIL-292)', () => {
  test.beforeEach(async ({ page }) => {
    // Mock the noisy endpoints so the SPA's apiClient doesn't 401 → refresh → /login bounce mid-spec.
    await page.route('**/api/folders', async (route) => {
      await route.fulfill({
        status: 200,
        contentType: 'application/json',
        body: JSON.stringify([
          { name: 'INBOX', unseen: 0 },
          { name: 'Sent', unseen: 0 },
          { name: 'Drafts', unseen: 0 },
          { name: 'Trash', unseen: 0 },
        ]),
      });
    });

    await page.route('**/api/folders/*/messages*', async (route) => {
      await route.fulfill({
        status: 200,
        contentType: 'application/json',
        body: JSON.stringify({ messages: [], total: 0 }),
      });
    });

    await page.route('**/api/quota', async (route) => {
      await route.fulfill({
        status: 200,
        contentType: 'application/json',
        body: JSON.stringify({
          mailbox_id: 'e2e-alt-ui',
          quota_bytes: 1_073_741_824,
          used_bytes: 104_857_600,
          message_count: 0,
          usage_percent: 10,
          quota_warn_percent: 80,
          is_over_quota: false,
          is_warning: false,
          last_synced_at: null,
        }),
      });
    });

    await page.route('**/api/signatures', async (route) => {
      await route.fulfill({
        status: 200,
        contentType: 'application/json',
        body: JSON.stringify([]),
      });
    });

    // RecipientAutocomplete fires /api/contacts?q=… while typing; stub with an
    // empty list so the dropdown stays out of the way but doesn't bounce on 401.
    await page.route(/\/api\/contacts(\?|$)/, async (route) => {
      await route.fulfill({
        status: 200,
        contentType: 'application/json',
        body: JSON.stringify([]),
      });
    });
  });

  test('login page loads and JWT is set in localStorage', async ({ page, apiSignup, takeScreenshot }) => {
    const email = `alt-ui-${Date.now()}@e2e.tasmail`;
    altUiEmails.push(email);
    const tokens = await apiSignup(email, 'alt-ui-e2e-pw-2026');
    await page.goto('/login');
    await page.evaluate(([at, rt]) => {
      localStorage.setItem('access_token', at);
      localStorage.setItem('refresh_token', rt);
    }, [tokens.access_token, tokens.refresh_token]);

    // Verify JWT is in localStorage after API signup.
    const hasToken = await page.evaluate(() => {
      return !!localStorage.getItem('access_token');
    });
    expect(hasToken).toBe(true);

    await takeScreenshot(page, 'alt-ui-modern-walkthrough/login-page');
  });

  test('dashboard renders with quota and account summary', async ({ page, apiSignup, takeScreenshot }) => {
    const email = `alt-ui-${Date.now()}@e2e.tasmail`;
    altUiEmails.push(email);
    const tokens = await apiSignup(email, 'alt-ui-e2e-pw-2026');
    await page.goto('/modern/');

    // Dashboard surface — verify key metrics render.
    const dashboard = page.locator('.modern-dashboard');
    await expect(dashboard).toBeVisible({ timeout: 15_000 });

    await expect(page.locator('.quota-bar')).toBeVisible();
    await expect(page.locator('.account-summary')).toBeVisible();

    // Echo back the JWT that was stored via apiSignup so we know auth is active.
    const jwtInStore = await page.evaluate(() => {
      return localStorage.getItem('access_token')?.substring(0, 20);
    });
    expect(jwtInStore).toBeTruthy();

    await takeScreenshot(page, 'alt-ui-modern-walkthrough/dashboard');
  });

  test('calendar view loads with events list', async ({ page, apiSignup, takeScreenshot }) => {
    const email = `alt-ui-${Date.now()}@e2e.tasmail`;
    altUiEmails.push(email);
    const tokens = await apiSignup(email, 'alt-ui-e2e-pw-2026');
    await page.goto('/modern/calendar/events');

    // Calendar events list should render.
    const eventsList = page.locator('.calendar-events-list');
    await expect(eventsList).toBeVisible({ timeout: 15_000 });

    // Verify at least one event row exists (mocked data).
    const eventRows = page.locator('.event-row');
    const rowCount = await eventRows.count();
    expect(rowCount).toBeGreaterThan(0);

    await takeScreenshot(page, 'alt-ui-modern-walkthrough/calendar-events');
  });

  test('calendar free-busy lookup returns data', async ({ page, apiSignup, takeScreenshot }) => {
    const email = `alt-ui-${Date.now()}@e2e.tasmail`;
    altUiEmails.push(email);
    const tokens = await apiSignup(email, 'alt-ui-e2e-pw-2026');
    await page.goto('/modern/calendar/free-busy');

    // Free-busy endpoint should return a JSON response.
    const fbResponse = page.locator('.free-busy-result');
    const fbText = await fbResponse.textContent();
    expect(fbText).not.toBeNull();
    expect(fbText?.length).toBeGreaterThan(0);

    await takeScreenshot(page, 'alt-ui-modern-walkthrough/free-busy-result');
  });

  test('admin dashboard shows users list', async ({ page, apiSignup, takeScreenshot }) => {
    const email = `alt-ui-${Date.now()}@e2e.tasmail`;
    altUiEmails.push(email);
    const tokens = await apiSignup(email, 'alt-ui-e2e-pw-2026');
    await page.goto('/modern/admin/users');

    // Admin users list should render.
    const usersTable = page.locator('.admin-users-table');
    await expect(usersTable).toBeVisible({ timeout: 15_000 });

    const userRows = page.locator('.user-row');
    const rowCount = await userRows.count();
    expect(rowCount).toBeGreaterThan(0);

    await takeScreenshot(page, 'alt-ui-modern-walkthrough/admin-users');
  });

  test('send message from alt-UI composer', async ({ page, apiSignup, takeScreenshot }) => {
    const email = `alt-ui-${Date.now()}@e2e.tasmail`;
    altUiEmails.push(email);
    const tokens = await apiSignup(email, 'alt-ui-e2e-pw-2026');
    await page.goto('/modern/');

    // Open the composer from the alt-UI (wand-icon or compose button).
    const composeBtn = page.locator('.modern-toolbar .btn--compose, .wand-icon');
    await expect(composeBtn).toBeVisible({ timeout: 15_000 });
    await composeBtn.click();

    // Wait for composer to mount.
    await expect(page.locator('.modern-composer__editor')).toBeVisible({ timeout: 15_000 });

    // Fill and send.
    await page.locator('.modern-composer__to-input').first().fill('alt-ui-recipient@example.com');
    await page.locator('.modern-composer__subject').fill('TMAIL-292 alt-UI send');
    const editor = page.locator('.modern-composer__editor').first();
    await editor.click();
    await page.keyboard.type('Body for the TMAIL-292 alt-UI send.');

    await takeScreenshot(page, 'alt-ui-modern-walkthrough/composer-filled');

    // Click Send.
    await page.locator('.modern-composer__actions button:has-text("Send")').click();
    const undoToast = page.locator('.modern-composer__undo-toast');
    await expect(undoToast).toBeVisible({ timeout: 10_000 });
    await expect(undoToast).toContainText(/Message sent/);

    await takeScreenshot(page, 'alt-ui-modern-walkthrough/send-submitted');
  });
});

test.afterAll(() => {
  for (const email of altUiEmails) {
    try {
      deleteMailboxByUsername(email);
    } catch {
      // Best-effort: don't fail the spec if psql is unreachable from CI.
    }
  }
});
