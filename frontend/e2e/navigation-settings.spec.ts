// Added: Settings-area navigation E2E specs for TASMail sidebar (TMAIL-36)
// Split from navigation.spec.ts to keep each spec at <=8 tests.
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
        { name: 'INBOX', unseen: 4 },
        { name: 'Sent', unseen: 0 },
      ]),
    });
  });

  await page.route('**/api/oidc/providers/login', async (route) => {
    await route.fulfill({
      status: 200,
      contentType: 'application/json',
      body: JSON.stringify([]),
    });
  });

  await page.route('**/api/quota', async (route) => {
    await route.fulfill({
      status: 200,
      contentType: 'application/json',
      body: JSON.stringify({ used: 150, limit: 1000 }),
    });
  });

  await page.route('**/api/folders/*/messages*', async (route) => {
    await route.fulfill({
      status: 200,
      contentType: 'application/json',
      body: JSON.stringify({ messages: [], total: 0 }),
    });
  });

  // Settings API stubs (empty payloads — we only validate navigation, not CRUD).
  await page.route('**/api/signatures', async (route) => {
    await route.fulfill({ status: 200, contentType: 'application/json', body: '[]' });
  });
  await page.route('**/api/contacts*', async (route) => {
    await route.fulfill({ status: 200, contentType: 'application/json', body: '[]' });
  });
  await page.route('**/api/2fa/status', async (route) => {
    await route.fulfill({
      status: 200,
      contentType: 'application/json',
      body: JSON.stringify({ enabled: false }),
    });
  });
});

test.describe('Settings Navigation', () => {
  test('click Signatures in sidebar opens Signatures view', async ({
    page,
    loginAs,
    takeScreenshot,
  }) => {
    await loginAs(page, 'user@example.com', 'password123');

    const signaturesBtn = page.locator('.sidebar .folder-item', { hasText: 'Signatures' });
    await signaturesBtn.click();
    await expect(signaturesBtn).toHaveClass(/folder-item--active/);

    await takeScreenshot(page, 'navigation/signatures-view');
  });

  test('click Contacts in sidebar opens Contacts view', async ({
    page,
    loginAs,
    takeScreenshot,
  }) => {
    await loginAs(page, 'user@example.com', 'password123');

    const contactsBtn = page.locator('.sidebar .folder-item', { hasText: /^Contacts$/ });
    await contactsBtn.click();
    await expect(contactsBtn).toHaveClass(/folder-item--active/);

    await takeScreenshot(page, 'navigation/contacts-view');
  });

  test('click Security in sidebar opens Two-Factor view', async ({
    page,
    loginAs,
    takeScreenshot,
  }) => {
    await loginAs(page, 'user@example.com', 'password123');

    const securityBtn = page.locator('.sidebar .folder-item', { hasText: 'Security' });
    await securityBtn.click();
    await expect(securityBtn).toHaveClass(/folder-item--active/);

    await takeScreenshot(page, 'navigation/security-view');
  });

  test('navigating between settings preserves sidebar state', async ({
    page,
    loginAs,
    takeScreenshot,
  }) => {
    await loginAs(page, 'user@example.com', 'password123');

    const signaturesBtn = page.locator('.sidebar .folder-item', { hasText: 'Signatures' });
    await signaturesBtn.click();
    await expect(signaturesBtn).toHaveClass(/folder-item--active/);
    await takeScreenshot(page, 'navigation/settings-flow-signatures');

    const securityBtn = page.locator('.sidebar .folder-item', { hasText: 'Security' });
    await securityBtn.click();
    await expect(securityBtn).toHaveClass(/folder-item--active/);
    await takeScreenshot(page, 'navigation/settings-flow-security');

    const inboxFolder = page.locator('.folder-tree .folder-item', { hasText: 'INBOX' });
    await inboxFolder.click();
    await expect(inboxFolder).toHaveClass(/folder-item--active/);
    await takeScreenshot(page, 'navigation/settings-flow-back-to-inbox');
  });
});
