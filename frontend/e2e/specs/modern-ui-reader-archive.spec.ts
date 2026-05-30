/**
 * TMAIL-317: alt-UI ("modern") EmailReader header Archive button moves the
 * open message to the IMAP "Archive" folder via POST /api/folders/INBOX/
 * messages/{uid}/move.
 *
 * Sister-spec to modern-ui-reader-star-flag.spec.ts (TMAIL-316). Shares the
 * same noreply BYOK signup → hop into /modern/ → open first envelope shape.
 *
 * Coverage:
 *   1. Sign up + BYOK the noreply mailbox so the inbox has real envelopes
 *   2. Hop into /modern/ via the classic SPA's wand button
 *   3. Snapshot INBOX UIDs and confirm "Archive" is not yet present on the
 *      mailbox (proves the backend CREATE-on-first-move retry actually fires)
 *   4. Open the first envelope so the EmailReader pane mounts
 *   5. Click the EmailReader header Archive button
 *   6. Poll the live backend until the target UID is gone from INBOX AND a
 *      new UID appears under the Archive folder (SPA E2E HARD RULE: validate
 *      mutation via API state before/after, not UI-only assertions)
 *   7. Assert the reader pane cleared (selectedUid reset → no h2 subject)
 *   8. Assert the "Archive" folder now appears in GET /api/folders so the
 *      backend CREATE retry path is exercised end-to-end
 *
 * Screenshots: frontend/e2e/screenshots/reader-archive/<step>.png
 *
 * Build prerequisite (handled by the auto-fix runner): `npm run build:alt-ui`
 * so the bundle in `frontend/public/modern/` reflects this commit.
 *
 * Runs against the live tunnel/proxy by default (see playwright.config.ts).
 */
import { expect, NOREPLY_CREDS, test } from '../fixtures/base.js';
import { deleteMailboxByUsername } from '../helpers/db-cleanup.js';

const SCREENSHOT_DIR = 'reader-archive';
const PASSWORD = 'tmail-317-reader-archive-2026';

interface EnvelopeRow {
  uid: number;
}

interface FolderRow {
  name: string;
}

async function fetchFolderMessages(
  baseURL: string | undefined,
  auth: Record<string, string>,
  folder: string,
): Promise<EnvelopeRow[]> {
  const resp = await fetch(
    `${baseURL}/api/folders/${encodeURIComponent(folder)}/messages?page=0&page_size=50`,
    { headers: auth },
  );
  if (!resp.ok) return [];
  const body = (await resp.json()) as { messages?: EnvelopeRow[] };
  return body.messages ?? [];
}

async function fetchFolderList(
  baseURL: string | undefined,
  auth: Record<string, string>,
): Promise<string[]> {
  const resp = await fetch(`${baseURL}/api/folders`, { headers: auth });
  if (!resp.ok) return [];
  const folders = (await resp.json()) as FolderRow[];
  return folders.map((f) => f.name);
}

test.describe('TMAIL-317 alt-UI EmailReader header Archive moves to Archive folder', () => {
  test.beforeAll(() => deleteMailboxByUsername(NOREPLY_CREDS.email));
  test.afterAll(() => deleteMailboxByUsername(NOREPLY_CREDS.email));

  test('reader header Archive POSTs /move and round-trips through the live IMAP', async ({
    page,
    apiSignup,
    takeScreenshot,
    baseURL,
  }) => {
    test.setTimeout(120_000);

    // ── 1. signup + BYOK so /api/folders/INBOX/messages has real rows ───
    const tokens = await apiSignup(NOREPLY_CREDS.email, PASSWORD);
    const auth = {
      Authorization: `Bearer ${tokens.access_token}`,
      'Content-Type': 'application/json',
    };
    const imapResp = await fetch(`${baseURL}/api/imap-configs`, {
      method: 'POST',
      headers: auth,
      body: JSON.stringify({
        name: 'tmail-317-imap',
        host: NOREPLY_CREDS.imap.host,
        port: NOREPLY_CREDS.imap.port,
        username: NOREPLY_CREDS.imap.username,
        password: NOREPLY_CREDS.imap.password,
        encryption: NOREPLY_CREDS.imap.encryption,
        is_default: true,
      }),
    });
    expect(imapResp.status, 'IMAP config').toBe(201);

    // ── 2. open classic /app and hop to /modern/ ────────────────────────
    await page.goto('/login');
    await page.evaluate(([at, rt]) => {
      localStorage.setItem('access_token', at);
      localStorage.setItem('refresh_token', rt);
    }, [tokens.access_token, tokens.refresh_token]);
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

    // ── 3. snapshot INBOX UIDs + folder list BEFORE clicking ────────────
    // (SPA E2E HARD RULE: capture API state before AND after the UI action.)
    const inboxBefore = await fetchFolderMessages(baseURL, auth, 'INBOX');
    expect(inboxBefore.length, 'inbox must contain at least one envelope').toBeGreaterThan(0);
    const targetUid = inboxBefore[0].uid;

    const foldersBefore = await fetchFolderList(baseURL, auth);
    // Stalwart provisions a small set of system folders on signup but does
    // NOT create "Archive" — so the first move must trigger the backend
    // CREATE retry path (TMAIL-317 backend change). Confirm preconditions.
    expect(
      foldersBefore.includes('Archive'),
      `precondition: "Archive" must not exist yet (folders=${foldersBefore.join(',')})`,
    ).toBe(false);

    // ── 4. open the first email so EmailReader mounts ───────────────────
    // EmailList rows render in the same order as the API, so inboxBefore[0]
    // is the first .cursor-pointer row. Clicking the row body (not the star
    // button) selects the message and mounts EmailReader.
    const firstRow = page.locator('div.cursor-pointer').first();
    await firstRow.locator('.text-sm.truncate').first().click();
    // EmailReader heading <h2.text-2xl> renders the subject — wait for it
    // so we know the reader mounted before we hunt for the Archive button.
    const readerHeading = page.locator('h2.text-2xl').first();
    await expect(readerHeading).toBeVisible({ timeout: 15_000 });
    await takeScreenshot(page, `${SCREENSHOT_DIR}/02-reader-opened`);

    // ── 5. find the EmailReader header Archive button ───────────────────
    // The Archive button lives in the reader toolbar (Reply / Reply All /
    // Forward / [spacer] / Archive / Delete) and is the only one whose
    // accessible name starts with "Archive email from ".
    const archiveButton = page.locator('button[aria-label^="Archive email from "]');
    await expect(archiveButton, 'reader Archive button is discoverable').toBeVisible({
      timeout: 10_000,
    });
    await takeScreenshot(page, `${SCREENSHOT_DIR}/03-before-click`);

    // ── 6. click — message must leave INBOX and land in Archive ─────────
    await archiveButton.click();

    // The IMAP COPY + STORE \Deleted + EXPUNGE + (potential CREATE retry)
    // round-trip plus the TanStack invalidation can take a beat. Poll the
    // live backend for up to ~15s.
    let movedOut = false;
    let movedIn = false;
    for (let attempt = 0; attempt < 15 && (!movedOut || !movedIn); attempt++) {
      await page.waitForTimeout(1000);
      const inboxAfter = await fetchFolderMessages(baseURL, auth, 'INBOX');
      movedOut = !inboxAfter.some((r) => r.uid === targetUid);
      const archiveAfter = await fetchFolderMessages(baseURL, auth, 'Archive');
      movedIn = archiveAfter.length > 0;
    }
    expect(movedOut, `uid=${targetUid} no longer present in INBOX`).toBe(true);
    expect(movedIn, 'Archive folder contains at least one message after click').toBe(true);
    await takeScreenshot(page, `${SCREENSHOT_DIR}/04-after-click`);

    // ── 7. reader pane must clear (selectedUid was reset in onMutate) ───
    // The EmailReader returns the empty-state copy when uid==null. We assert
    // the previous subject heading is gone — the empty placeholder uses
    // plain text ("Select an email to read"), not an h2.
    await expect(readerHeading).toHaveCount(0);
    await expect(
      page.getByText(/Select an email to read/i).first(),
    ).toBeVisible({ timeout: 5_000 });

    // ── 8. assert backend now lists "Archive" — proves the CREATE retry
    //       path (TMAIL-317 backend change) fired end-to-end. Without it,
    //       the COPY against a non-existent destination would have failed
    //       and the message would still be in INBOX.
    const foldersAfter = await fetchFolderList(baseURL, auth);
    expect(
      foldersAfter.includes('Archive'),
      `Archive folder appears in /api/folders after first archive (folders=${foldersAfter.join(',')})`,
    ).toBe(true);
    await takeScreenshot(page, `${SCREENSHOT_DIR}/05-folder-list-includes-archive`);
  });
});
