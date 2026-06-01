/**
 * TMAIL-283 — Folder tree + paginated message list + unread + multi-select E2E sweep
 *
 * Surfaces covered:
 *   1. Folder tree (`/api/folders`) — sidebar renders system folders with badges,
 *      INBOX is the default selection, clicking other folders switches the view.
 *   2. Paginated message list (`/api/folders/{folder}/messages`) — INBOX renders
 *      its envelopes; backend honours `?page=0&page_size=N` parameters; the SPA
 *      hardcodes (page=0, size=50) via `useCurrentMessages()` so the assessment
 *      records the UI gap (see TMAIL-263 finding #4).
 *   3. Unread badge — `Folder.unseen` drives the count, opening an unread message
 *      flips `\\Seen` and `/api/folders` reports a decremented `unseen`.
 *   4. Flag toggle (Star) — `POST /messages/:uid/flag {\\Flagged, add: true}`,
 *      cross-checked via `GET /messages/:uid` returning the new flag set.
 *   5. Move to folder — `POST /messages/:uid/move {to_folder}`, cross-checked via
 *      source-folder envelope count before/after.
 *   6. Delete — `DELETE /messages/:uid`, cross-checked via INBOX total before/after.
 *   7. Empty folder state — `.message-list__empty` rendered for a folder with no
 *      messages (TASMail's Drafts is empty for a fresh BYOK account).
 *   8. Multi-select gap — the production UI has no row checkbox / bulk action bar;
 *      the assessment doc records this so it can be split into its own ticket.
 *
 * Validation strategy (per the E2E HARD RULES):
 *   - All UI navigation is menu-click driven; `page.goto()` is only used for the
 *     initial `/login` URL (the documented exception).
 *   - Screenshots captured at every key validation point under
 *     `e2e/screenshots/folder-messagelist/` via the shared `takeScreenshot` fixture.
 *   - Every mutation cross-checks the backend state via a fresh API GET — UI-only
 *     assertions (toasts, DOM updates) are never trusted on their own.
 *
 * Test bed:
 *   - The same noreply@techatscale.io mailbox the BYOK end-to-end spec uses.
 *     Sign-up + BYOK provisioning happen via the API in `beforeAll` so each
 *     test starts inside `/app` without re-walking the onboarding wizard.
 *   - If the INBOX is empty when the suite starts (first-ever run against a
 *     freshly-purged mailbox), we self-send three seed messages via the saved
 *     SMTP config so the mutation tests have something to act on. This makes
 *     the suite idempotent across re-runs.
 */
import { test, NOREPLY_CREDS, expect } from './fixtures/base.js';
import { request as apiRequest, type APIRequestContext } from '@playwright/test';
import { deleteMailboxByUsername } from './helpers/db-cleanup.js';

const ACCOUNT_PASSWORD = 'folder-sweep-Pa55word!';
const RUN_TAG = Date.now();
const ACCOUNT_EMAIL = `e2e-folder-${RUN_TAG}@e2e.tasmail`;

// We BYOK-attach the real noreply mailbox so the message list isn't talking to a
// stub. The mailbox already has historical mail; if empty we seed at setup time.
const BYOK_IMAP = NOREPLY_CREDS.imap;
const BYOK_SMTP = NOREPLY_CREDS.smtp;

let api: APIRequestContext;
let accessToken: string;
let authHeader: Record<string, string>;
let seededMessageCount = 0;

interface Folder { name: string; unseen: number; }
interface MessageListResp { messages: Array<{ uid: number; subject: string | null; flags: string[] }>; total: number; }
interface FullMessageResp { uid: number; subject: string | null; flags: string[]; body_text: string | null; }

test.describe.configure({ mode: 'serial' });

test.describe('TMAIL-283 Folder tree + message list sweep', () => {
  // -------------------- Suite-wide setup --------------------
  test.beforeAll(async ({ baseURL }) => {
    test.setTimeout(120_000);
    api = await apiRequest.newContext({ baseURL });

    // Sign up the test account via the API — bypasses the UI signup form which
    // is already covered by TMAIL-281.
    const signup = await api.post('/api/auth/signup', {
      data: { email: ACCOUNT_EMAIL, password: ACCOUNT_PASSWORD },
    });
    expect(signup.status(), 'signup must succeed').toBeLessThan(300);
    const signupBody = (await signup.json()) as { access_token: string };
    accessToken = signupBody.access_token;
    authHeader = { Authorization: `Bearer ${accessToken}` };

    // TMAIL-405: pre-mark the FirstLoginTour as seen so the overlay doesn't
    // intercept clicks once the test navigates to /app. Without this every
    // INBOX / folder click times out behind the tour backdrop.
    await api.patch('/api/me/preferences/first-login-tour-seen', {
      headers: authHeader,
      data: {},
    });

    // Attach the noreply IMAP server as the default — the same shape the wizard
    // would POST after the user clicks Save & continue. We set `trash_folder`
    // to the Stalwart special-use name so TMAIL-283's delete flow can resolve
    // the right destination folder (see folders-messages-2026-05.md finding #7
    // and the matching imap_service.rs::trash_folder fix in this commit).
    const imap = await api.post('/api/imap-configs', {
      headers: authHeader,
      data: {
        name: 'noreply (E2E)',
        host: BYOK_IMAP.host,
        port: BYOK_IMAP.port,
        username: BYOK_IMAP.username,
        password: BYOK_IMAP.password,
        encryption: BYOK_IMAP.encryption,
        trash_folder: 'Deleted Items',
        sent_folder: 'Sent Items',
        drafts_folder: 'Drafts',
        spam_folder: 'Junk Mail',
        is_default: true,
      },
    });
    expect(imap.status(), 'IMAP config create must succeed').toBeLessThan(300);

    // Attach SMTP too so the seeding step (if needed) has something to send through.
    const smtp = await api.post('/api/smtp-configs', {
      headers: authHeader,
      data: {
        name: 'noreply SMTP (E2E)',
        host: BYOK_SMTP.host,
        port: BYOK_SMTP.port,
        username: BYOK_SMTP.username,
        password: BYOK_SMTP.password,
        encryption: BYOK_SMTP.encryption,
        from_address: NOREPLY_CREDS.email,
      },
    });
    expect(smtp.status(), 'SMTP config create must succeed').toBeLessThan(300);

    // If INBOX is empty (a brand-new run after a purge), seed 3 messages so the
    // mutation tests have material. We send noreply→noreply so the local IMAP
    // mailbox is the source of truth.
    const initialList = await api.get('/api/folders/INBOX/messages?page=0&page_size=10', {
      headers: authHeader,
    });
    if (initialList.status() === 200) {
      const body = (await initialList.json()) as MessageListResp;
      if (body.total < 3) {
        for (let i = body.total; i < 3; i++) {
          const send = await api.post('/api/messages/send', {
            headers: authHeader,
            data: {
              to: [NOREPLY_CREDS.email],
              subject: `TMAIL-283 seed message ${i + 1} ${RUN_TAG}`,
              body_text: `Seed message ${i + 1} from TMAIL-283 folder/messagelist sweep. Run tag ${RUN_TAG}.`,
            },
          });
          // Best-effort — if send fails we still want the read-only tests to run.
          if (send.ok()) seededMessageCount++;
        }
        // Give IMAP a beat to deliver self-sent mail; the SMTP loopback through
        // Stalwart usually shows up within a couple of seconds.
        if (seededMessageCount > 0) await new Promise((r) => setTimeout(r, 4_000));
      }
    }
  });

  test.afterAll(async () => {
    try { deleteMailboxByUsername(ACCOUNT_EMAIL); } catch { /* best-effort */ }
    await api?.dispose();
  });

  // Each test logs in fresh through the UI so we exercise the full auth → app
  // shell paint path. Cookies/localStorage are wiped between tests by Playwright.
  async function loginViaUI(page: import('@playwright/test').Page) {
    await page.goto('/login');
    await page.waitForSelector('#username');
    await page.fill('#username', ACCOUNT_EMAIL);
    await page.fill('#password', ACCOUNT_PASSWORD);
    await page.click('button[type="submit"]');
    await page.waitForURL(/\/app/, { timeout: 20_000 });
    // Folder tree's loading placeholder disappears once `/api/folders` resolves.
    await expect(page.locator('.folder-tree--loading')).toHaveCount(0, { timeout: 15_000 });
  }

  // After clicking INBOX, MessageList groups messages by normalised subject —
  // multi-message threads render as a ThreadRow whose click just toggles
  // expansion. The mutation tests need every row to open MessageView on click,
  // so we untick the "Conversations" toggle to force flat rendering.
  async function openInboxFlat(page: import('@playwright/test').Page) {
    await page.locator('.folder-tree .folder-item', { hasText: 'INBOX' }).click();
    await expect(page.locator('.message-list')).toBeVisible({ timeout: 15_000 });
    const conversationsToggle = page.locator('.message-list__header input[type="checkbox"]');
    if (await conversationsToggle.isChecked().catch(() => false)) {
      await conversationsToggle.click();
    }
  }

  // -------------------- 1) Folder tree render + counts --------------------
  test('folder tree renders system folders with unread badges', async ({ page, takeScreenshot }) => {
    await loginViaUI(page);

    const folderTree = page.locator('.folder-tree');
    await expect(folderTree).toBeVisible();
    // Cross-check the rendered names against the live /api/folders payload.
    const foldersResp = await api.get('/api/folders', { headers: authHeader });
    expect(foldersResp.status()).toBe(200);
    const folders = (await foldersResp.json()) as Folder[];
    expect(folders.length, 'BYOK mailbox should expose at least INBOX').toBeGreaterThan(0);
    const inbox = folders.find((f) => f.name.toUpperCase() === 'INBOX');
    expect(inbox, 'INBOX must be in the folder list').toBeTruthy();

    // The SPA capitalises INBOX as-is from the IMAP `LIST` response; the rest
    // of the system folders depend on Stalwart's special-use mapping for the
    // noreply mailbox (`Sent`, `Trash`, `Drafts`, `Junk`/`Spam`).
    await expect(folderTree.locator('.folder-item__name', { hasText: 'INBOX' })).toBeVisible();
    await takeScreenshot(page, 'folder-messagelist/folder-tree-expanded');

    // If the API says INBOX has unread, the SPA must surface a non-zero badge.
    if ((inbox?.unseen ?? 0) > 0) {
      const inboxBadge = folderTree
        .locator('.folder-item', { hasText: 'INBOX' })
        .locator('.folder-item__badge');
      await expect(inboxBadge).toHaveText(String(inbox!.unseen));
      await takeScreenshot(page, 'folder-messagelist/unread-badge');
    }
  });

  // -------------------- 2) INBOX message list renders page 0 --------------------
  test('clicking INBOX renders the paginated message list (page 0 default)', async ({ page, takeScreenshot }) => {
    await loginViaUI(page);

    const inboxFolder = page.locator('.folder-tree .folder-item', { hasText: 'INBOX' });
    await inboxFolder.click();
    await expect(inboxFolder).toHaveClass(/folder-item--active/);

    // INBOX should have ≥3 envelopes thanks to the seeding step (or pre-existing
    // mail). The SPA loads page 0 size 50 — verify via the API call shape.
    const listResp = await api.get('/api/folders/INBOX/messages?page=0&page_size=50', {
      headers: authHeader,
    });
    expect(listResp.status()).toBe(200);
    const list = (await listResp.json()) as MessageListResp;
    expect(list.total, 'INBOX must have at least one message for the sweep').toBeGreaterThan(0);

    // The SPA renders the rows inside `.message-list__items`. With <50 messages
    // every row should be in the DOM (no virtualisation yet — TMAIL-263).
    const messageList = page.locator('.message-list');
    await expect(messageList).toBeVisible();
    const renderedRows = page.locator('.message-list .message-row');
    await expect(renderedRows.first()).toBeVisible({ timeout: 15_000 });
    await takeScreenshot(page, 'folder-messagelist/inbox-page-1');

    // Pagination contract — backend MUST support page=1 even though the SPA
    // does not currently expose a "load more" button (TMAIL-263 finding #4).
    const page2Resp = await api.get('/api/folders/INBOX/messages?page=1&page_size=10', {
      headers: authHeader,
    });
    expect(page2Resp.status(), 'page=1 must not 500 — backend must paginate').toBe(200);
    const page2 = (await page2Resp.json()) as MessageListResp;
    // total is page-invariant — same total regardless of which page we fetch.
    expect(page2.total).toBe(list.total);

    // Document the UI gap with a screenshot: scroll the list to the bottom, no
    // "Load more" button exists. The screenshot becomes the assessment evidence.
    await page.locator('.message-list__items').evaluate((el) => el.scrollTo(0, el.scrollHeight));
    await takeScreenshot(page, 'folder-messagelist/inbox-page-2-attempt');
  });

  // -------------------- 3) Opening an unread message decrements the badge --------------------
  test('opening an unread message flips \\Seen and decrements the unread badge', async ({ page, takeScreenshot }) => {
    await loginViaUI(page);

    // Find an unread UID via the API. If everything is already read, mark one
    // as unread via the flag API so the test has something to exercise.
    let listResp = await api.get('/api/folders/INBOX/messages?page=0&page_size=20', {
      headers: authHeader,
    });
    let list = (await listResp.json()) as MessageListResp;
    let unread = list.messages.find((m) => !m.flags.some((f) => f.includes('Seen')));
    if (!unread) {
      const target = list.messages[0];
      expect(target, 'INBOX has no messages to test').toBeTruthy();
      const setUnread = await api.post(`/api/folders/INBOX/messages/${target.uid}/flag`, {
        headers: authHeader,
        data: { flag: '\\Seen', add: false },
      });
      expect(setUnread.status(), 'priming an unread message must succeed').toBeLessThan(300);
      listResp = await api.get('/api/folders/INBOX/messages?page=0&page_size=20', {
        headers: authHeader,
      });
      list = (await listResp.json()) as MessageListResp;
      unread = list.messages.find((m) => m.uid === target.uid);
      expect(unread, 'primed message must reappear as unread').toBeTruthy();
    }

    const foldersBefore = (await (await api.get('/api/folders', { headers: authHeader })).json()) as Folder[];
    const inboxUnseenBefore = foldersBefore.find((f) => f.name.toUpperCase() === 'INBOX')?.unseen ?? 0;

    await openInboxFlat(page);
    const row = page
      .locator('.message-list .message-row')
      .filter({ hasText: unread!.subject ?? '' })
      .first();
    await expect(row).toBeVisible({ timeout: 15_000 });
    await takeScreenshot(page, 'folder-messagelist/before-mark-read');

    await row.click();
    const messageView = page.locator('.message-view');
    // Fix (TMAIL-425): MessageView only renders its .message-view root once
    // useCurrentMessage() resolves the real IMAP body — under broader-batch
    // load that fetch can comfortably exceed the default 5s expect timeout.
    await expect(messageView).toBeVisible({ timeout: 15_000 });
    await takeScreenshot(page, 'folder-messagelist/message-opened');

    // API cross-check: this UID's flags now include \\Seen.
    const opened = (await (await api.get(`/api/folders/INBOX/messages/${unread!.uid}`, {
      headers: authHeader,
    })).json()) as FullMessageResp;
    expect(opened.flags.some((f) => f.includes('Seen')), 'opening must flip \\Seen on the IMAP server').toBe(true);

    // And the folder-level unseen count drops by one.
    const foldersAfter = (await (await api.get('/api/folders', { headers: authHeader })).json()) as Folder[];
    const inboxUnseenAfter = foldersAfter.find((f) => f.name.toUpperCase() === 'INBOX')?.unseen ?? 0;
    expect(inboxUnseenAfter, 'unread badge must decrement after marking read').toBeLessThan(inboxUnseenBefore);
  });

  // -------------------- 4) Star (flag) toggle persists to IMAP --------------------
  test('starring a message persists \\Flagged to IMAP', async ({ page, takeScreenshot }) => {
    await loginViaUI(page);
    await openInboxFlat(page);

    // Pick a message to act on — first row in the rendered list.
    const firstRow = page.locator('.message-list .message-row').first();
    await expect(firstRow).toBeVisible({ timeout: 15_000 });
    const subject = await firstRow.locator('.message-row__subject').innerText();
    await firstRow.click();
    const messageView = page.locator('.message-view');
    // Fix (TMAIL-425): real IMAP body fetch can take >5s under batch load.
    await expect(messageView).toBeVisible({ timeout: 15_000 });

    // API state before — pull the UID out of the listing then GET flags.
    const list = (await (await api.get('/api/folders/INBOX/messages?page=0&page_size=20', {
      headers: authHeader,
    })).json()) as MessageListResp;
    const target = list.messages.find((m) => (m.subject ?? '').includes(subject.trim())) ?? list.messages[0];
    const flagsBefore = (
      (await (await api.get(`/api/folders/INBOX/messages/${target.uid}`, { headers: authHeader })).json()) as FullMessageResp
    ).flags;
    const wasFlagged = flagsBefore.some((f) => f.includes('Flagged'));
    await takeScreenshot(page, 'folder-messagelist/before-star-toggle');

    // Click whichever button is currently displayed — Unstar if already flagged,
    // otherwise Star — to land in the OPPOSITE state.
    await messageView
      .locator(`button[title="${wasFlagged ? 'Unstar' : 'Star'}"]`)
      .click();

    // Poll the API: flag state must flip.
    await expect.poll(async () => {
      const after = (await (await api.get(`/api/folders/INBOX/messages/${target.uid}`, {
        headers: authHeader,
      })).json()) as FullMessageResp;
      return after.flags.some((f) => f.includes('Flagged'));
    }, { timeout: 10_000 }).toBe(!wasFlagged);

    await takeScreenshot(page, 'folder-messagelist/after-star-toggle');

    // Reset the flag to its original state so the next test starts clean.
    await api.post(`/api/folders/INBOX/messages/${target.uid}/flag`, {
      headers: authHeader,
      data: { flag: '\\Flagged', add: wasFlagged },
    });
  });

  // -------------------- 5) Move changes source folder count --------------------
  test('moving a message decrements the source folder envelope count', async ({ page, takeScreenshot }) => {
    await loginViaUI(page);
    await openInboxFlat(page);

    // Pick the folder we'll move INTO — prefer Trash, fall back to any non-INBOX.
    const folders = (await (await api.get('/api/folders', { headers: authHeader })).json()) as Folder[];
    const destFolder =
      folders.find((f) => /trash/i.test(f.name))?.name ??
      folders.find((f) => f.name.toUpperCase() !== 'INBOX')?.name;
    expect(destFolder, 'a destination folder for the move must exist').toBeTruthy();

    const inboxBefore = (await (await api.get('/api/folders/INBOX/messages?page=0&page_size=50', {
      headers: authHeader,
    })).json()) as MessageListResp;
    expect(inboxBefore.total).toBeGreaterThan(0);
    const target = inboxBefore.messages[inboxBefore.messages.length - 1]; // oldest visible
    await takeScreenshot(page, 'folder-messagelist/before-move');

    // Drive the move via the API — the production UI uses window.prompt(),
    // which Playwright's dialog handler covers but the cross-check is what
    // proves the move worked, not the prompt UI itself.
    page.on('dialog', (dialog) => dialog.accept(destFolder!));
    const targetRow = page.locator('.message-list .message-row').filter({
      hasText: target.subject ?? '',
    }).first();
    await targetRow.click();
    await page.locator('.message-view button[title="Move to folder"]').click();

    // Cross-check: source folder total drops by exactly one.
    await expect.poll(async () => {
      const after = (await (await api.get('/api/folders/INBOX/messages?page=0&page_size=50', {
        headers: authHeader,
      })).json()) as MessageListResp;
      return after.total;
    }, { timeout: 15_000 }).toBe(inboxBefore.total - 1);

    await takeScreenshot(page, 'folder-messagelist/after-move');
  });

  // -------------------- 6) Delete reduces INBOX total --------------------
  test('deleting a message reduces the INBOX total', async ({ page, takeScreenshot }) => {
    await loginViaUI(page);
    await openInboxFlat(page);

    const inboxBefore = (await (await api.get('/api/folders/INBOX/messages?page=0&page_size=50', {
      headers: authHeader,
    })).json()) as MessageListResp;
    if (inboxBefore.total === 0) {
      // Seed one so this test has something to delete — keeps the suite resilient.
      const send = await api.post('/api/messages/send', {
        headers: authHeader,
        data: {
          to: [NOREPLY_CREDS.email],
          subject: `TMAIL-283 delete-target ${RUN_TAG}`,
          body_text: 'Throwaway for the delete cross-check.',
        },
      });
      expect(send.ok(), 'seeding a delete target must succeed').toBe(true);
      await new Promise((r) => setTimeout(r, 4_000));
    }
    const before = (await (await api.get('/api/folders/INBOX/messages?page=0&page_size=50', {
      headers: authHeader,
    })).json()) as MessageListResp;
    const target = before.messages[0];
    await takeScreenshot(page, 'folder-messagelist/before-delete');

    const targetRow = page.locator('.message-list .message-row').filter({
      hasText: target.subject ?? '',
    }).first();
    await targetRow.click();
    await page.locator('.message-view button[title="Delete"]').click();

    await expect.poll(async () => {
      const after = (await (await api.get('/api/folders/INBOX/messages?page=0&page_size=50', {
        headers: authHeader,
      })).json()) as MessageListResp;
      return after.total;
    }, { timeout: 15_000 }).toBe(before.total - 1);

    await takeScreenshot(page, 'folder-messagelist/after-delete');
  });

  // -------------------- 7) Empty folder state --------------------
  test('an empty folder renders the "no messages" empty state', async ({ page, takeScreenshot }) => {
    await loginViaUI(page);

    // Drafts is the most reliably-empty folder for a fresh BYOK account that has
    // never composed anything. If it's somehow not empty, fall back to any
    // folder the API reports with `unseen: 0` AND a zero-total message list.
    const folders = (await (await api.get('/api/folders', { headers: authHeader })).json()) as Folder[];
    let emptyFolder: string | undefined;
    for (const f of folders) {
      const r = await api.get(`/api/folders/${encodeURIComponent(f.name)}/messages?page=0&page_size=10`, {
        headers: authHeader,
      });
      if (r.status() === 200) {
        const body = (await r.json()) as MessageListResp;
        if (body.total === 0) { emptyFolder = f.name; break; }
      }
    }
    if (!emptyFolder) {
      // Mailbox has no empty folder — skip rather than fabricate a false negative.
      test.skip(true, 'no empty folder available on this mailbox to validate empty state');
      return;
    }

    const folderItem = page.locator('.folder-tree .folder-item', { hasText: emptyFolder });
    await folderItem.click();
    await expect(page.locator('.message-list__empty')).toBeVisible({ timeout: 15_000 });
    await expect(page.locator('.message-list__empty')).toContainText(/no messages/i);
    await takeScreenshot(page, 'folder-messagelist/empty-folder');
  });

  // -------------------- 8) Multi-select UI gap --------------------
  test('multi-select / bulk action bar — gap captured for follow-up ticket', async ({ page, takeScreenshot }) => {
    await loginViaUI(page);
    await openInboxFlat(page);
    await expect(page.locator('.message-list .message-row').first()).toBeVisible({ timeout: 15_000 });

    // The production MessageList component does not render row checkboxes nor a
    // sticky action bar — clicking a row opens MessageView. We assert the
    // absence so a regression that ships multi-select without docs would force
    // an explicit update to this spec + assessment.
    await expect(page.locator('.message-list .message-row input[type="checkbox"]')).toHaveCount(0);
    await expect(page.locator('.message-list__bulk-actions, .message-list__action-bar')).toHaveCount(0);
    await takeScreenshot(page, 'folder-messagelist/multi-select-gap');
  });
});
