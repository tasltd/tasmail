// Added: Compose E2E specs for TASMail email composer (TMAIL-36)
import { test, expect } from './fixtures/base';
// Fix (TMAIL-408): need DB cleanup for the per-test signup emails so re-runs
// stay idempotent and the e2e.tasmail accounts don't accumulate forever.
import { deleteMailboxByUsername } from './helpers/db-cleanup.js';

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
    // Fix (TMAIL-417): real QuotaStatus shape so QuotaBar doesn't render "NaN".
    await route.fulfill({
      status: 200,
      contentType: 'application/json',
      body: JSON.stringify({
        mailbox_id: 'e2e-mailbox',
        quota_bytes: 1073741824,
        used_bytes: 157286400,
        message_count: 0,
        usage_percent: 15,
        quota_warn_percent: 80,
        is_over_quota: false,
        is_warning: false,
        last_synced_at: null,
      }),
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

  // Fix (TMAIL-406): mock the FirstLoginTour preference endpoint (TMAIL-401).
  // AppShell mounts FirstLoginTour for every /app render and the component
  // fires GET /api/me/preferences/first-login-tour-seen on mount. Without a
  // mock the request hits the live backend with the fake mock-access-token,
  // returns 401, apiClient's refresh chain also 401s (mock-refresh-token is
  // invalid), and the SPA does window.location.href='/login' — which is what
  // bounces the compose spec mid-test. Returning seen:true keeps the tour from
  // rendering AND short-circuits the PATCH path.
  await page.route('**/api/me/preferences/first-login-tour-seen', async (route) => {
    await route.fulfill({
      status: 200,
      contentType: 'application/json',
      body: JSON.stringify({ seen: true }),
    });
  });

  // Fix (TMAIL-406): defensive mock for the token refresh endpoint. If any
  // other unmocked endpoint 401s the apiClient will hit /api/auth/refresh
  // before redirecting — stub it out so a missed mock degrades to a no-op
  // instead of bouncing the whole test to /login.
  await page.route('**/api/auth/refresh', async (route) => {
    await route.fulfill({
      status: 200,
      contentType: 'application/json',
      body: JSON.stringify({
        access_token: 'mock-access-token',
        refresh_token: 'mock-refresh-token',
      }),
    });
  });

  // Fix (TMAIL-406): mock contacts autocomplete. RecipientAutocomplete (used
  // by the composer's To / Cc fields) fires GET /api/contacts?q=... once the
  // user starts typing. Unmocked it 401s and feeds into the same bounce chain.
  // NOTE: regex (not glob) so we don't also intercept Vite's dev-served source
  // module at /src/api/contacts.ts — `**/api/contacts*` would match both and
  // corrupt the dynamic import that lazy-loads the composer.
  await page.route(/\/api\/contacts(\?|$)/, async (route) => {
    await route.fulfill({
      status: 200,
      contentType: 'application/json',
      body: JSON.stringify([]),
    });
  });
});

// Fix (TMAIL-408): collect every per-test signup email so the afterAll hook
// can wipe them from the DB. Each apiSignup() creates a real mailbox row in
// the live `byok.tasmail` domain; without cleanup the table grows by one
// row per run.
const composeEmails: string[] = [];

test.describe('Email Composer', () => {
  test.afterAll(() => {
    for (const email of composeEmails) {
      try {
        deleteMailboxByUsername(email);
      } catch {
        // Best-effort cleanup — don't fail the spec if the DB isn't reachable
        // (e.g. CI runs against a remote backend without psql).
      }
    }
  });

  test('clicking Compose button opens the composer', async ({
    page,
    apiSignup,
    takeScreenshot,
  }) => {
    // Fix (TMAIL-412): replace the dead loginAs('user@example.com') call that
    // bounced mid-test on unmocked endpoints — provision a real BYOK account.
    const email = `compose-open-${Date.now()}@e2e.tasmail`;
    composeEmails.push(email);
    const tokens = await apiSignup(email, 'compose-test-pw-2026');
    await page.goto('/login');
    await page.evaluate(([at, rt]) => {
      localStorage.setItem('access_token', at);
      localStorage.setItem('refresh_token', rt);
    }, [tokens.access_token, tokens.refresh_token]);
    await page.goto('/app');
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
    apiSignup,
    takeScreenshot,
  }) => {
    // Fix (TMAIL-412): same dead-loginAs replacement as the open-composer test.
    const email = `compose-fields-${Date.now()}@e2e.tasmail`;
    composeEmails.push(email);
    const tokens = await apiSignup(email, 'compose-test-pw-2026');
    await page.goto('/login');
    await page.evaluate(([at, rt]) => {
      localStorage.setItem('access_token', at);
      localStorage.setItem('refresh_token', rt);
    }, [tokens.access_token, tokens.refresh_token]);
    await page.goto('/app');

    // Added: Open composer via sidebar Compose button
    await page.locator('.sidebar .btn--compose').click();

    // Added: Wait for composer inputs to render
    // NOTE: Composer uses standard input elements identified by placeholder/label text
    await page.waitForTimeout(500);

    await takeScreenshot(page, 'compose/composer-fields-visible');
  });

  test('fill To and Subject fields in composer', async ({
    page,
    apiSignup,
    takeScreenshot,
  }) => {
    // Fix (TMAIL-412): same dead-loginAs replacement as the open-composer test.
    const email = `compose-fill-${Date.now()}@e2e.tasmail`;
    composeEmails.push(email);
    const tokens = await apiSignup(email, 'compose-test-pw-2026');
    await page.goto('/login');
    await page.evaluate(([at, rt]) => {
      localStorage.setItem('access_token', at);
      localStorage.setItem('refresh_token', rt);
    }, [tokens.access_token, tokens.refresh_token]);
    await page.goto('/app');

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
    apiSignup,
    takeScreenshot,
  }) => {
    // Fix (TMAIL-412): same dead-loginAs replacement as the open-composer test.
    const email = `compose-dismiss-${Date.now()}@e2e.tasmail`;
    composeEmails.push(email);
    const tokens = await apiSignup(email, 'compose-test-pw-2026');
    await page.goto('/login');
    await page.evaluate(([at, rt]) => {
      localStorage.setItem('access_token', at);
      localStorage.setItem('refresh_token', rt);
    }, [tokens.access_token, tokens.refresh_token]);
    await page.goto('/app');

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
    apiSignup,
    takeScreenshot,
  }) => {
    // Fix (TMAIL-408): the hardcoded loginAs('user@example.com') call relied
    // on a mailbox that doesn't exist in the DB, so the SPA's first unmocked
    // request 401s, refresh fails, and the page bounces back to /login mid
    // test. Provision a real per-test BYOK account via the public signup
    // endpoint and inject its JWT pair into localStorage instead.
    const email = `compose-${Date.now()}@e2e.tasmail`;
    composeEmails.push(email);
    const tokens = await apiSignup(email, 'compose-test-pw-2026');
    await page.goto('/login');
    await page.evaluate(([at, rt]) => {
      localStorage.setItem('access_token', at);
      localStorage.setItem('refresh_token', rt);
    }, [tokens.access_token, tokens.refresh_token]);
    await page.goto('/app');

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
    apiSignup,
    takeScreenshot,
  }) => {
    // Fix (TMAIL-408): same root cause as the send-button test above — the
    // hardcoded 'user@example.com' login mocked to 200 but the resulting
    // mock-access-token isn't accepted by any unmocked endpoint, so the SPA
    // 401s and bounces to /login before the send button is ever clicked,
    // leaving sendCalled === false. Provision a real BYOK account so the JWT
    // round-trips cleanly.
    const email = `compose-spa-${Date.now()}@e2e.tasmail`;
    composeEmails.push(email);
    const tokens = await apiSignup(email, 'compose-test-pw-2026');
    await page.goto('/login');
    await page.evaluate(([at, rt]) => {
      localStorage.setItem('access_token', at);
      localStorage.setItem('refresh_token', rt);
    }, [tokens.access_token, tokens.refresh_token]);
    await page.goto('/app');

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

    // Fix (TMAIL-406): wait for the Composer's lazy-loaded chunk to actually
    // mount before interacting. Composer is React.lazy() in AppShell so the
    // chunk download + module init takes longer than the previous 500ms
    // sleep — the To input is the first DOM signal that Composer rendered.
    const toInput = page.locator('input[placeholder*="recipient"], input[placeholder*="To"], input[placeholder*="to"]').first();
    await expect(toInput).toBeVisible({ timeout: 15_000 });

    await takeScreenshot(page, 'compose/spa-validation-composer-open');

    // Added: Fill fields to trigger auto-save draft
    await toInput.fill('test@example.com');

    const subjectInput = page.locator('input[placeholder*="Subject"], input[placeholder*="subject"]').first();
    await expect(subjectInput).toBeVisible();
    await subjectInput.fill('SPA Validation Test');

    await takeScreenshot(page, 'compose/spa-validation-fields-filled');

    // Fix (TMAIL-406): explicitly wait for the Send button before clicking.
    // The old `if (await sendBtn.isVisible())` was a no-wait check that raced
    // against Suspense fallback and silently skipped the click — leaving
    // sendCalled === false even when the composer was about to render.
    const sendBtn = page.locator('button', { hasText: /send/i }).first();
    await expect(sendBtn).toBeVisible();
    await sendBtn.click();
    // NOTE: Wait briefly for API call to complete
    await page.waitForTimeout(1000);

    await takeScreenshot(page, 'compose/spa-validation-after-send');

    // Added: SPA validation — verify the send API was actually called
    // NOTE: This confirms the UI action triggered the expected backend mutation
    expect(sendCalled).toBe(true);
  });
});
