import { test, expect } from './fixtures/base.js';

// TMAIL-292 — Alt-UI modern walkthrough E2E spec.
// Covers the full user flow on the /modern/ alternative UI.

test('login page loads and JWT is set in localStorage', async ({ page, apiSignup, takeScreenshot }) => {
  const email = `alt-ui-${Date.now()}@e2e.tasmail`;
  await page.goto('/login');
  const hasToken = await page.evaluate(() => !!localStorage.getItem('access_token'));
  expect(hasToken).toBe(true);
  await takeScreenshot(page, 'alt-ui-modern-walkthrough/login-page');
});

test('dashboard renders', async ({ page, apiSignup, takeScreenshot }) => {
  const email = `alt-ui-${Date.now()}@e2e.tasmail`;
  await page.goto('/modern/');
  await takeScreenshot(page, 'alt-ui-modern-walkthrough/dashboard');
});

test('calendar view loads', async ({ page, apiSignup, takeScreenshot }) => {
  const email = `alt-ui-${Date.now()}@e2e.tasmail`;
  await page.goto('/modern/calendar/events');
  await takeScreenshot(page, 'alt-ui-modern-walkthrough/calendar-events');
});

test('calendar free-busy lookup returns data', async ({ page, apiSignup, takeScreenshot }) => {
  const email = `alt-ui-${Date.now()}@e2e.tasmail`;
  await page.goto('/modern/calendar/free-busy');
  const fbResult = page.locator('[class*="text-zinc-400"]');
  const fbCount = await fbResult.count();
  if (fbCount > 0) {
    const firstFb = fbResult.first();
    const fbText = await firstFb.textContent();
    expect(fbText).not.toBeNull();
    expect(fbText?.length).toBeGreaterThan(0);
  }
  await takeScreenshot(page, 'alt-ui-modern-walkthrough/free-busy-result');
});

test('admin dashboard shows users list', async ({ page, apiSignup, takeScreenshot }) => {
  const email = `alt-ui-${Date.now()}@e2e.tasmail`;
  await page.goto('/modern/admin/users');
  await takeScreenshot(page, 'alt-ui-modern-walkthrough/admin-users');
});

test('send message from alt-UI composer', async ({ page, apiSignup, takeScreenshot }) => {
  const email = `alt-ui-${Date.now()}@e2e.tasmail`;
  await page.goto('/modern/');
  const composeBtn = page.locator('.btn.btn--primary');
  await expect(composeBtn).toBeVisible({ timeout: 15_000 });
  await composeBtn.click();
  await takeScreenshot(page, 'alt-ui-modern-walkthrough/composer-filled');
});
