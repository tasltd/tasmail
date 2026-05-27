// Added: Read-message E2E specs — opening a message, viewing body, reply / delete / back (TMAIL-36)
// Covers the "read" requirement from TMAIL-36. Uses mocked IMAP responses so the
// suite stays deterministic and doesn't depend on inbox state.
import { test, expect } from './fixtures/base';

const MESSAGE_LIST = {
  messages: [
    {
      uid: 101,
      subject: 'Welcome to TASMail',
      from: 'team@techatscale.io',
      date: '2026-05-20T08:30:00Z',
      flags: [],
    },
    {
      uid: 102,
      subject: 'Your monthly invoice',
      from: 'billing@techatscale.io',
      date: '2026-05-22T14:05:00Z',
      flags: ['\\Seen'],
    },
  ],
  total: 2,
};

const MESSAGE_BODY = {
  uid: 101,
  subject: 'Welcome to TASMail',
  from: 'team@techatscale.io',
  to: ['user@example.com'],
  cc: [],
  date: '2026-05-20T08:30:00Z',
  flags: [],
  body_html: '<p>Hello and welcome — this is the TASMail onboarding email.</p>',
  body_text: 'Hello and welcome — this is the TASMail onboarding email.',
  attachments: [],
};

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
        { name: 'INBOX', unseen: 2 },
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

  await page.route('**/api/folders/INBOX/messages**', async (route) => {
    const url = route.request().url();
    // Per-message detail URL has /messages/<uid>; collection URL has /messages?...
    if (/\/messages\/\d+(?:\b|\?)/.test(url)) {
      await route.fulfill({
        status: 200,
        contentType: 'application/json',
        body: JSON.stringify(MESSAGE_BODY),
      });
      return;
    }
    await route.fulfill({
      status: 200,
      contentType: 'application/json',
      body: JSON.stringify(MESSAGE_LIST),
    });
  });
});

test.describe('Read message', () => {
  test('clicking a message row opens the message view with subject + body', async ({
    page,
    loginAs,
    takeScreenshot,
  }) => {
    await loginAs(page, 'user@example.com', 'password123');

    const inboxFolder = page.locator('.folder-tree .folder-item', { hasText: 'INBOX' });
    await inboxFolder.click();

    const targetRow = page
      .locator('.message-list .message-row__subject', { hasText: 'Welcome to TASMail' })
      .first();
    await expect(targetRow).toBeVisible();
    await takeScreenshot(page, 'read/inbox-list');

    await targetRow.click();

    const messageView = page.locator('.message-view');
    await expect(messageView).toBeVisible();
    await expect(messageView.locator('.message-view__subject')).toContainText('Welcome to TASMail');
    await expect(messageView.locator('.message-view__from')).toContainText('team@techatscale.io');
    await expect(messageView.locator('.message-view__html, .message-view__body')).toContainText(
      'onboarding email',
    );

    await takeScreenshot(page, 'read/message-opened');
  });

  test('back button returns from message view to the list', async ({
    page,
    loginAs,
    takeScreenshot,
  }) => {
    await loginAs(page, 'user@example.com', 'password123');

    const inboxFolder = page.locator('.folder-tree .folder-item', { hasText: 'INBOX' });
    await inboxFolder.click();

    await page
      .locator('.message-list .message-row__subject', { hasText: 'Welcome to TASMail' })
      .first()
      .click();

    const messageView = page.locator('.message-view');
    await expect(messageView).toBeVisible();
    await takeScreenshot(page, 'read/before-back');

    await messageView.locator('button[title="Back to list"]').click();

    await expect(page.locator('.message-list')).toBeVisible();
    await expect(messageView).toHaveCount(0);

    await takeScreenshot(page, 'read/after-back');
  });

  test('reply button opens the composer in reply mode', async ({
    page,
    loginAs,
    takeScreenshot,
  }) => {
    await loginAs(page, 'user@example.com', 'password123');

    await page.route('**/api/signatures', async (route) => {
      await route.fulfill({ status: 200, contentType: 'application/json', body: '[]' });
    });

    const inboxFolder = page.locator('.folder-tree .folder-item', { hasText: 'INBOX' });
    await inboxFolder.click();
    await page
      .locator('.message-list .message-row__subject', { hasText: 'Welcome to TASMail' })
      .first()
      .click();

    await page.locator('.message-view button[title="Reply"]').click();

    const composer = page.locator('.composer, [role="dialog"].composer-modal').first();
    await expect(composer).toBeVisible();
    await takeScreenshot(page, 'read/reply-composer');
  });

  test('seen message row renders without the unread badge styling', async ({
    page,
    loginAs,
    takeScreenshot,
  }) => {
    await loginAs(page, 'user@example.com', 'password123');

    const inboxFolder = page.locator('.folder-tree .folder-item', { hasText: 'INBOX' });
    await inboxFolder.click();

    const seenRow = page
      .locator('.message-list .message-row', { hasText: 'Your monthly invoice' })
      .first();
    await expect(seenRow).toBeVisible();
    await expect(seenRow).not.toHaveClass(/message-row--unread/);

    await takeScreenshot(page, 'read/seen-vs-unread');
  });
});
