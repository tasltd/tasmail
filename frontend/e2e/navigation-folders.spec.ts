// Added: Folder-tree navigation E2E specs for TASMail sidebar (TMAIL-36)
// Split from navigation.spec.ts so each spec stays at <=8 tests per the
// "small focused specs" hard rule in ~/.claude/rules/all-rules.md.
import { test, expect } from './fixtures/base';

// Added: Shared route mocks so each test starts in a known logged-in state.
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

  await page.route('**/api/folders/*/messages*', async (route) => {
    await route.fulfill({
      status: 200,
      contentType: 'application/json',
      body: JSON.stringify({ messages: [], total: 0 }),
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

    const sidebar = page.locator('.sidebar');
    await expect(sidebar).toBeVisible();
    await expect(sidebar.locator('.btn--compose')).toBeVisible();
    await expect(sidebar.locator('.btn--compose')).toContainText('Compose');
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
    await expect(folderTree.locator('.folder-item__name', { hasText: 'INBOX' })).toBeVisible();
    await expect(folderTree.locator('.folder-item__name', { hasText: 'Sent' })).toBeVisible();
    await expect(folderTree.locator('.folder-item__name', { hasText: 'Drafts' })).toBeVisible();
    await expect(folderTree.locator('.folder-item__name', { hasText: 'Trash' })).toBeVisible();
    await expect(folderTree.locator('.folder-item__name', { hasText: 'Junk' })).toBeVisible();

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

    const inboxFolder = page.locator('.folder-tree .folder-item', { hasText: 'INBOX' });
    await inboxFolder.click();
    await expect(inboxFolder).toHaveClass(/folder-item--active/);

    await takeScreenshot(page, 'navigation/inbox-selected');
  });

  test('click Sent folder switches view', async ({
    page,
    loginAs,
    takeScreenshot,
  }) => {
    await loginAs(page, 'user@example.com', 'password123');

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

  test('click Junk folder switches view', async ({
    page,
    loginAs,
    takeScreenshot,
  }) => {
    await loginAs(page, 'user@example.com', 'password123');

    const junkFolder = page.locator('.folder-tree .folder-item', { hasText: 'Junk' });
    await junkFolder.click();
    await expect(junkFolder).toHaveClass(/folder-item--active/);

    await takeScreenshot(page, 'navigation/junk-selected');
  });
});
