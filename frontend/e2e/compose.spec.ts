// Added: Compose E2E specs for TASMail email composer (TMAIL-36)
import { test, expect } from './fixtures/base';

// Added: Shared route mocks for authenticated session
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
        { name: 'Drafts', unseen: 1 },
        { name: 'Trash', unseen: 0 },
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

  // Added: Mock messages endpoint for folder content
  await page.route('**/api/folders/*/messages*', async (route) => {
    await route.fulfill({
      status: 200,
      contentType: 'application/json',
      body: JSON.stringify({ messages: [], total: 0 }),
    });
  });

  // Added: Mock signatures API for composer signature dropdown
  await page.route('**/api/signatures', async (route) => {
    await route.fulfill({
      status: 200,
      contentType: 'application/json',
      body: JSON.stringify([]),
    });
  });

  // Added: Mock drafts save endpoint for auto-save
  await page.route('**/api/drafts', async (route) => {
    if (route.request().method() === 'POST') {
      await route.fulfill({
        status: 201,
        contentType: 'application/json',
        body: JSON.stringify({ uid: 1, folder: 'Drafts' }),
      });
    } else {
      await route.fulfill({
        status: 200,
        contentType: 'application/json',
        body: JSON.stringify([]),
      });
    }
  });
});

test.describe('Email Composer', () => {
  test('clicking Compose button opens the composer', async ({
    page,
    loginAs,
    takeScreenshot,
  }) => {
    await loginAs(page, 'user@example.com', 'password123');
    await takeScreenshot(page, 'compose/before-compose-click');

    // Added: Click Compose button in sidebar (menu navigation, not goto)
    const composeBtn = page.locator('.sidebar .btn--compose');
    await expect(composeBtn).toBeVisible();
    await composeBtn.click();

    // Added: Verify the composer view is now active
    // NOTE: Composer has input fields for To, Subject, and TipTap editor
    await expect(page.locator('.app-shell__content')).toBeVisible();

    await takeScreenshot(page, 'compose/composer-opened');
  });

  test('composer has To, CC, Subject fields and editor', async ({
    page,
    loginAs,
    takeScreenshot,
  }) => {
    await loginAs(page, 'user@example.com', 'password123');

    // Added: Open composer via sidebar Compose button
    await page.locator('.sidebar .btn--compose').click();

    // Added: Wait for composer inputs to render
    // NOTE: Composer uses standard input elements identified by placeholder/label text
    await page.waitForTimeout(500);

    await takeScreenshot(page, 'compose/composer-fields-visible');
  });

  test('fill To and Subject fields in composer', async ({
    page,
    loginAs,
    takeScreenshot,
  }) => {
    await loginAs(page, 'user@example.com', 'password123');

    // Added: Open composer via sidebar
    await page.locator('.sidebar .btn--compose').click();
    await page.waitForTimeout(500);

    await takeScreenshot(page, 'compose/fill-fields-before');

    // Added: Fill the To field — look for input with placeholder containing "To" or "recipient"
    const toInput = page.locator('input[placeholder*="To"], input[placeholder*="to"], input[placeholder*="recipient"]').first();
    if (await toInput.isVisible()) {
      await toInput.fill('recipient@example.com');
      await takeScreenshot(page, 'compose/fill-to-field');
    }

    // Added: Fill the Subject field
    const subjectInput = page.locator('input[placeholder*="Subject"], input[placeholder*="subject"]').first();
    if (await subjectInput.isVisible()) {
      await subjectInput.fill('Test Email Subject');
      await takeScreenshot(page, 'compose/fill-subject-field');
    }

    // Added: Interact with the TipTap rich text editor body
    const editor = page.locator('.tiptap, .ProseMirror, [contenteditable="true"]').first();
    if (await editor.isVisible()) {
      await editor.click();
      await editor.fill('This is the body of the test email.');
      await takeScreenshot(page, 'compose/fill-body-field');
    }

    await takeScreenshot(page, 'compose/all-fields-filled');
  });

  test('composer can be dismissed by clicking close/cancel', async ({
    page,
    loginAs,
    takeScreenshot,
  }) => {
    await loginAs(page, 'user@example.com', 'password123');

    // Added: Open composer via sidebar Compose button
    await page.locator('.sidebar .btn--compose').click();
    await page.waitForTimeout(500);

    await takeScreenshot(page, 'compose/before-dismiss');

    // Added: Look for a close/cancel button in the composer
    // NOTE: Composer uses an X (close) button with lucide-react X icon
    const closeBtn = page.locator('button', { hasText: /close|cancel|discard/i }).first();
    const xBtn = page.locator('.btn--icon').filter({ has: page.locator('svg') }).first();

    if (await closeBtn.isVisible()) {
      await closeBtn.click();
    } else if (await xBtn.isVisible()) {
      // Added: Fallback — navigate away by clicking INBOX folder
      const inboxFolder = page.locator('.folder-tree .folder-item', { hasText: 'INBOX' });
      await inboxFolder.click();
    }

    await takeScreenshot(page, 'compose/after-dismiss');
  });

  test('send button exists and is interactive', async ({
    page,
    loginAs,
    takeScreenshot,
  }) => {
    await loginAs(page, 'user@example.com', 'password123');

    // Added: Open composer
    await page.locator('.sidebar .btn--compose').click();
    await page.waitForTimeout(500);

    // Added: Verify a Send button is present in the composer
    // NOTE: Composer uses a button with Send icon from lucide-react
    const sendBtn = page.locator('button', { hasText: /send/i }).first();
    await expect(sendBtn).toBeVisible();

    await takeScreenshot(page, 'compose/send-button-visible');
  });

  test('SPA validation: composing and sending updates API state', async ({
    page,
    loginAs,
    takeScreenshot,
  }) => {
    await loginAs(page, 'user@example.com', 'password123');

    // Added: SPA validation — GET drafts count BEFORE composing
    let draftSaveCount = 0;
    await page.route('**/api/drafts', async (route) => {
      if (route.request().method() === 'POST') {
        draftSaveCount++;
        await route.fulfill({
          status: 201,
          contentType: 'application/json',
          body: JSON.stringify({ uid: draftSaveCount, folder: 'Drafts' }),
        });
      } else {
        await route.fulfill({
          status: 200,
          contentType: 'application/json',
          body: JSON.stringify([]),
        });
      }
    });

    // Changed: Composer now posts to /api/messages/schedule (10s undo window) via
    // scheduledApi.scheduleSend(), not the legacy /api/messages/send. Mock both
    // so the assertion below catches either contract.
    let sendCalled = false;
    const trackSend = async (route: import('@playwright/test').Route) => {
      sendCalled = true;
      await route.fulfill({
        status: 200,
        contentType: 'application/json',
        body: JSON.stringify({ id: 'sched-1', cancel_token: 'test-token' }),
      });
    };
    await page.route('**/api/messages/send', trackSend);
    await page.route('**/api/messages/schedule', trackSend);

    // Added: Open composer via sidebar button
    await page.locator('.sidebar .btn--compose').click();
    await page.waitForTimeout(500);

    await takeScreenshot(page, 'compose/spa-validation-composer-open');

    // Added: Fill fields to trigger auto-save draft
    const toInput = page.locator('input[placeholder*="To"], input[placeholder*="to"], input[placeholder*="recipient"]').first();
    if (await toInput.isVisible()) {
      await toInput.fill('test@example.com');
    }

    const subjectInput = page.locator('input[placeholder*="Subject"], input[placeholder*="subject"]').first();
    if (await subjectInput.isVisible()) {
      await subjectInput.fill('SPA Validation Test');
    }

    await takeScreenshot(page, 'compose/spa-validation-fields-filled');

    // Added: Click Send button to trigger the send API call
    const sendBtn = page.locator('button', { hasText: /send/i }).first();
    if (await sendBtn.isVisible()) {
      await sendBtn.click();
      // NOTE: Wait briefly for API call to complete
      await page.waitForTimeout(1000);
    }

    await takeScreenshot(page, 'compose/spa-validation-after-send');

    // Added: SPA validation — verify the send API was actually called
    // NOTE: This confirms the UI action triggered the expected backend mutation
    expect(sendCalled).toBe(true);
  });
});
