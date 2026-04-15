// Added: Navigation E2E specs for TASMail sidebar and folder switching (TMAIL-36)
import { test, expect } from './fixtures/base';

// Added: Shared route mocks for authenticated session used across all navigation tests
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
        { name: 'INBOX', unseen: 12 },
        { name: 'Sent', unseen: 0 },
        { name: 'Drafts', unseen: 3 },
        { name: 'Trash', unseen: 0 },
        { name: 'Junk', unseen: 5 },
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
      body: JSON.stringify({ used: 250, limit: 1000 }),
    });
  });

  // Added: Mock messages API for folder content loading
  await page.route('**/api/folders/*/messages*', async (route) => {
    await route.fulfill({
      status: 200,
      contentType: 'application/json',
      body: JSON.stringify({
        messages: [
          {
            uid: 1,
            subject: 'Test Email',
            from: 'sender@example.com',
            date: '2026-04-15T10:00:00Z',
            flags: [],
          },
        ],
        total: 1,
      }),
    });
  });
});

test.describe('Sidebar Navigation', () => {
  test('sidebar renders with folder tree and settings items', async ({
    page,
    loginAs,
    takeScreenshot,
  }) => {
    await loginAs(page, 'user@example.com', 'password123');

    // Added: Verify sidebar is visible with key structural elements
    const sidebar = page.locator('.sidebar');
    await expect(sidebar).toBeVisible();

    // Added: Verify Compose button exists
    await expect(sidebar.locator('.btn--compose')).toBeVisible();
    await expect(sidebar.locator('.btn--compose')).toContainText('Compose');

    // Added: Verify folder tree is present
    await expect(sidebar.locator('.folder-tree')).toBeVisible();

    await takeScreenshot(page, 'navigation/sidebar-rendered');
  });

  test('mail folders display with unread badges', async ({
    page,
    loginAs,
    takeScreenshot,
  }) => {
    await loginAs(page, 'user@example.com', 'password123');

    const folderTree = page.locator('.folder-tree');

    // Added: Verify all standard mail folders are rendered
    await expect(folderTree.locator('.folder-item__name', { hasText: 'INBOX' })).toBeVisible();
    await expect(folderTree.locator('.folder-item__name', { hasText: 'Sent' })).toBeVisible();
    await expect(folderTree.locator('.folder-item__name', { hasText: 'Drafts' })).toBeVisible();
    await expect(folderTree.locator('.folder-item__name', { hasText: 'Trash' })).toBeVisible();
    await expect(folderTree.locator('.folder-item__name', { hasText: 'Junk' })).toBeVisible();

    // Added: Verify unread badges are displayed for folders with unseen > 0
    const inboxItem = folderTree.locator('.folder-item', { hasText: 'INBOX' });
    await expect(inboxItem.locator('.folder-item__badge')).toHaveText('12');

    const draftsItem = folderTree.locator('.folder-item', { hasText: 'Drafts' });
    await expect(draftsItem.locator('.folder-item__badge')).toHaveText('3');

    await takeScreenshot(page, 'navigation/folder-badges');
  });
});

test.describe('Folder Navigation', () => {
  test('click INBOX folder loads inbox content', async ({
    page,
    loginAs,
    takeScreenshot,
  }) => {
    await loginAs(page, 'user@example.com', 'password123');

    // Added: Click INBOX folder in the folder tree (menu navigation, not goto)
    const inboxFolder = page.locator('.folder-tree .folder-item', { hasText: 'INBOX' });
    await inboxFolder.click();

    // Added: Verify the folder item becomes active
    await expect(inboxFolder).toHaveClass(/folder-item--active/);

    await takeScreenshot(page, 'navigation/inbox-selected');
  });

  test('click Sent folder switches view', async ({
    page,
    loginAs,
    takeScreenshot,
  }) => {
    await loginAs(page, 'user@example.com', 'password123');

    // Added: Navigate to Sent folder via sidebar click
    const sentFolder = page.locator('.folder-tree .folder-item', { hasText: 'Sent' });
    await sentFolder.click();

    await expect(sentFolder).toHaveClass(/folder-item--active/);

    await takeScreenshot(page, 'navigation/sent-selected');
  });

  test('click Drafts folder switches view', async ({
    page,
    loginAs,
    takeScreenshot,
  }) => {
    await loginAs(page, 'user@example.com', 'password123');

    const draftsFolder = page.locator('.folder-tree .folder-item', { hasText: 'Drafts' });
    await draftsFolder.click();

    await expect(draftsFolder).toHaveClass(/folder-item--active/);

    await takeScreenshot(page, 'navigation/drafts-selected');
  });

  test('click Trash folder switches view', async ({
    page,
    loginAs,
    takeScreenshot,
  }) => {
    await loginAs(page, 'user@example.com', 'password123');

    const trashFolder = page.locator('.folder-tree .folder-item', { hasText: 'Trash' });
    await trashFolder.click();

    await expect(trashFolder).toHaveClass(/folder-item--active/);

    await takeScreenshot(page, 'navigation/trash-selected');
  });
});

test.describe('Settings Navigation', () => {
  test('click Signatures in sidebar opens Signatures view', async ({
    page,
    loginAs,
    takeScreenshot,
  }) => {
    await loginAs(page, 'user@example.com', 'password123');

    // Added: Mock signatures API for the settings view
    await page.route('**/api/signatures', async (route) => {
      await route.fulfill({
        status: 200,
        contentType: 'application/json',
        body: JSON.stringify([]),
      });
    });

    // Added: Click Signatures button in sidebar settings section (menu nav only)
    const signaturesBtn = page.locator('.sidebar .folder-item', { hasText: 'Signatures' });
    await signaturesBtn.click();

    // Added: Verify the button shows active state
    await expect(signaturesBtn).toHaveClass(/folder-item--active/);

    await takeScreenshot(page, 'navigation/signatures-view');
  });

  test('click Contacts in sidebar opens Contacts view', async ({
    page,
    loginAs,
    takeScreenshot,
  }) => {
    await loginAs(page, 'user@example.com', 'password123');

    await page.route('**/api/contacts*', async (route) => {
      await route.fulfill({
        status: 200,
        contentType: 'application/json',
        body: JSON.stringify([]),
      });
    });

    // Added: Click Contacts in sidebar (not Contacts App — different viewMode)
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

    await page.route('**/api/2fa/status', async (route) => {
      await route.fulfill({
        status: 200,
        contentType: 'application/json',
        body: JSON.stringify({ enabled: false }),
      });
    });

    // Added: Click Security settings via sidebar menu click
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

    // Added: Mock APIs for settings views
    await page.route('**/api/signatures', async (route) => {
      await route.fulfill({
        status: 200,
        contentType: 'application/json',
        body: JSON.stringify([]),
      });
    });

    await page.route('**/api/2fa/status', async (route) => {
      await route.fulfill({
        status: 200,
        contentType: 'application/json',
        body: JSON.stringify({ enabled: false }),
      });
    });

    // Added: Navigate Signatures -> Security -> back to INBOX via sidebar clicks
    const signaturesBtn = page.locator('.sidebar .folder-item', { hasText: 'Signatures' });
    await signaturesBtn.click();
    await expect(signaturesBtn).toHaveClass(/folder-item--active/);

    await takeScreenshot(page, 'navigation/settings-flow-signatures');

    const securityBtn = page.locator('.sidebar .folder-item', { hasText: 'Security' });
    await securityBtn.click();
    await expect(securityBtn).toHaveClass(/folder-item--active/);

    await takeScreenshot(page, 'navigation/settings-flow-security');

    // Added: Return to INBOX from settings via sidebar folder click
    const inboxFolder = page.locator('.folder-tree .folder-item', { hasText: 'INBOX' });
    await inboxFolder.click();
    await expect(inboxFolder).toHaveClass(/folder-item--active/);

    await takeScreenshot(page, 'navigation/settings-flow-back-to-inbox');
  });
});
