/**
 * TMAIL-348: alt-UI ("modern") EmailReader per-message comments thread.
 *
 * Coverage:
 *   1. Sign up + BYOK the noreply mailbox so the inbox has at least one
 *      real envelope
 *   2. Hop into /modern/ via the classic SPA's wand button
 *   3. Open the first envelope so the EmailReader pane mounts
 *   4. Assert the empty state of the comments section is rendered below the
 *      message body
 *   5. Post a comment via the textarea + "Add comment" button and verify it
 *      appears in the UI AND in a fresh GET /api/folders/INBOX/messages/{uid}
 *      /comments (SPA E2E HARD RULE: validate mutation via API state before/
 *      after, not UI-only assertions)
 *   6. Edit the comment via the Pencil action and verify the content change
 *      round-trips through the same API GET
 *   7. Delete the comment via the Trash action and verify the API GET returns
 *      an empty list again
 *
 * Screenshots: frontend/e2e/screenshots/reader-comments/<step>.png
 *
 * Build prerequisite (handled by the auto-fix runner): `npm run build:alt-ui`
 * so the bundle in `frontend/public/modern/` reflects this commit.
 *
 * Runs against the live tunnel/proxy by default (see playwright.config.ts).
 */
import { expect, NOREPLY_CREDS, test } from '../fixtures/base.js';
import { deleteMailboxByUsername } from '../helpers/db-cleanup.js';

const SCREENSHOT_DIR = 'reader-comments';
const PASSWORD = 'tmail-348-reader-comments-2026';

interface EnvelopeRow {
  uid: number;
}

interface CommentRow {
  id: string;
  content: string;
  author_email: string;
  message_uid: number;
}

async function fetchInbox(
  baseURL: string | undefined,
  auth: Record<string, string>,
): Promise<EnvelopeRow[]> {
  const resp = await fetch(
    `${baseURL}/api/folders/INBOX/messages?page=0&page_size=50`,
    { headers: auth },
  );
  if (!resp.ok) return [];
  const body = (await resp.json()) as { messages?: EnvelopeRow[] };
  return body.messages ?? [];
}

async function fetchComments(
  baseURL: string | undefined,
  auth: Record<string, string>,
  uid: number,
): Promise<CommentRow[]> {
  const resp = await fetch(
    `${baseURL}/api/folders/INBOX/messages/${uid}/comments`,
    { headers: auth },
  );
  if (!resp.ok) return [];
  const text = await resp.text();
  if (!text) return [];
  return JSON.parse(text) as CommentRow[];
}

test.describe('TMAIL-348 alt-UI EmailReader per-message comments thread', () => {
  test.beforeAll(() => deleteMailboxByUsername(NOREPLY_CREDS.email));
  test.afterAll(() => deleteMailboxByUsername(NOREPLY_CREDS.email));

  test('list → add → edit → delete a comment with live API round-trip', async ({
    page,
    apiSignup,
    takeScreenshot,
    baseURL,
  }) => {
    test.setTimeout(180_000);

    // ── 1. signup + BYOK so /api/folders/INBOX/messages has real rows ────
    const tokens = await apiSignup(NOREPLY_CREDS.email, PASSWORD);
    const auth = {
      Authorization: `Bearer ${tokens.access_token}`,
      'Content-Type': 'application/json',
    };
    const imapResp = await fetch(`${baseURL}/api/imap-configs`, {
      method: 'POST',
      headers: auth,
      body: JSON.stringify({
        name: 'tmail-348-imap',
        host: NOREPLY_CREDS.imap.host,
        port: NOREPLY_CREDS.imap.port,
        username: NOREPLY_CREDS.imap.username,
        password: NOREPLY_CREDS.imap.password,
        encryption: NOREPLY_CREDS.imap.encryption,
        is_default: true,
      }),
    });
    expect(imapResp.status, 'IMAP config').toBe(201);

    // ── 2. log in via localStorage + classic /app then hop to /modern/ ───
    await page.goto('/login');
    await page.evaluate(
      ([at, rt]) => {
        localStorage.setItem('access_token', at);
        localStorage.setItem('refresh_token', rt);
      },
      [tokens.access_token, tokens.refresh_token],
    );
    await page.goto('/app');
    await expect(
      page.locator('button, a', { hasText: /Compose/i }).first(),
    ).toBeVisible({ timeout: 20_000 });
    await page.locator('a[title="Try the modern UI"]').click();
    await page.waitForURL(/\/modern\/index\.html/i, { timeout: 15_000 });
    await page.waitForLoadState('domcontentloaded');
    await expect(page).toHaveTitle(/Modern UI/i);
    await expect(page.locator('h2', { hasText: /INBOX/i })).toBeVisible({
      timeout: 25_000,
    });
    await expect(page.locator('div.cursor-pointer').first()).toBeVisible({
      timeout: 25_000,
    });
    await takeScreenshot(page, `${SCREENSHOT_DIR}/01-inbox-rendered`);

    // ── 3. open the first envelope so EmailReader mounts ─────────────────
    const before = await fetchInbox(baseURL, auth);
    expect(before.length, 'inbox must contain at least one envelope').toBeGreaterThan(0);
    const targetUid = before[0].uid;

    await page
      .locator('div.cursor-pointer')
      .first()
      .locator('.text-sm.truncate')
      .first()
      .click();
    await expect(page.locator('h2.text-2xl').first()).toBeVisible({ timeout: 15_000 });
    await takeScreenshot(page, `${SCREENSHOT_DIR}/02-reader-opened`);

    // ── 4. empty state: no comments yet ──────────────────────────────────
    const initialList = await fetchComments(baseURL, auth, targetUid);
    expect(initialList.length, 'no comments before user posts one').toBe(0);

    const thread = page.locator('[data-testid="modern-comments-thread"]');
    await expect(thread, 'comments section must render below the body').toBeVisible({
      timeout: 10_000,
    });
    await expect(
      thread.locator('[data-testid="modern-comments-empty"]'),
    ).toBeVisible();
    await takeScreenshot(page, `${SCREENSHOT_DIR}/03-empty-state`);

    // ── 5. add a new comment via the UI + verify backend state changed ───
    const ORIGINAL = 'follow up with client tomorrow morning';
    await thread
      .locator('[data-testid="modern-comments-new-input"]')
      .fill(ORIGINAL);
    await thread.locator('[data-testid="modern-comments-submit"]').click();

    let afterCreate: CommentRow[] = [];
    for (let attempt = 0; attempt < 12; attempt++) {
      await page.waitForTimeout(500);
      afterCreate = await fetchComments(baseURL, auth, targetUid);
      if (afterCreate.length === 1) break;
    }
    expect(afterCreate.length, 'one comment after add').toBe(1);
    expect(afterCreate[0].content).toBe(ORIGINAL);
    expect(afterCreate[0].author_email).toBe(NOREPLY_CREDS.email);
    expect(afterCreate[0].message_uid).toBe(targetUid);

    // UI must surface the new comment too — fail fast if the cache wasn't
    // invalidated by the create mutation.
    const newRow = thread.locator('[data-testid="modern-comment-item"]').first();
    await expect(newRow).toBeVisible({ timeout: 10_000 });
    await expect(
      newRow.locator('[data-testid="modern-comment-content"]'),
    ).toHaveText(ORIGINAL);
    await takeScreenshot(page, `${SCREENSHOT_DIR}/04-comment-added`);

    // ── 6. edit the comment via the Pencil + verify backend ──────────────
    const EDITED = 'follow up with client TODAY — they pushed the deadline';
    await newRow.locator('[data-testid="modern-comment-edit"]').click();
    const editInput = newRow.locator('[data-testid="modern-comment-edit-input"]');
    await expect(editInput).toBeVisible({ timeout: 5_000 });
    await editInput.fill(EDITED);
    await newRow.locator('[data-testid="modern-comment-save"]').click();

    let afterEdit: CommentRow[] = [];
    for (let attempt = 0; attempt < 12; attempt++) {
      await page.waitForTimeout(500);
      afterEdit = await fetchComments(baseURL, auth, targetUid);
      if (afterEdit.length === 1 && afterEdit[0].content === EDITED) break;
    }
    expect(afterEdit.length, 'still one comment after edit').toBe(1);
    expect(afterEdit[0].content).toBe(EDITED);
    expect(afterEdit[0].id).toBe(afterCreate[0].id);

    await expect(
      newRow.locator('[data-testid="modern-comment-content"]'),
    ).toHaveText(EDITED);
    await takeScreenshot(page, `${SCREENSHOT_DIR}/05-comment-edited`);

    // ── 7. delete the comment via the Trash + verify backend cleanup ─────
    await newRow.locator('[data-testid="modern-comment-delete"]').click();

    let afterDelete: CommentRow[] = afterEdit;
    for (let attempt = 0; attempt < 12; attempt++) {
      await page.waitForTimeout(500);
      afterDelete = await fetchComments(baseURL, auth, targetUid);
      if (afterDelete.length === 0) break;
    }
    expect(afterDelete.length, 'no comments after delete').toBe(0);

    await expect(
      thread.locator('[data-testid="modern-comments-empty"]'),
      'empty state returns after the only comment is deleted',
    ).toBeVisible({ timeout: 10_000 });
    await takeScreenshot(page, `${SCREENSHOT_DIR}/06-comment-deleted`);
  });
});
