/**
 * TMAIL-318: alt-UI ("modern") EmailReader Delete button. From a non-trash
 * folder it soft-deletes (backend's DELETE handler routes to the per-user
 * trash folder — Stalwart "Deleted Items"). From the trash folder itself it
 * permanently EXPUNGEs the message, guarded by a window.confirm() prompt.
 *
 * Sister-spec to modern-ui-reader-archive.spec.ts (TMAIL-317). Shares the
 * same noreply BYOK signup → hop into /modern/ → open first envelope shape.
 *
 * Coverage (SPA E2E HARD RULE: API state before AND after for every mutation):
 *
 *   PART A — soft delete from INBOX
 *     1. Sign up + BYOK the noreply mailbox so the inbox has real envelopes
 *     2. Hop into /modern/ via the classic SPA's wand button
 *     3. Snapshot INBOX UIDs + Deleted Items UIDs
 *     4. Open the first envelope so the EmailReader pane mounts
 *     5. Click the EmailReader header Delete button
 *     6. Poll until the target UID has LEFT INBOX and a new UID has APPEARED
 *        in Deleted Items
 *     7. Assert the reader pane cleared
 *
 *   PART B — permanent delete from Deleted Items
 *     8. Click the "Deleted Items" folder in the sidebar
 *     9. Snapshot the Deleted Items UIDs
 *    10. Open the soft-deleted message
 *    11. Register a window.confirm() handler that ACCEPTS (proves the
 *        confirmation gate fires before permanent EXPUNGE)
 *    12. Click the EmailReader header Delete button
 *    13. Poll until the target UID has LEFT Deleted Items (true expunge, not
 *        another move — the backend's delete_message resolves the active
 *        folder as the trash folder and routes to the EXPUNGE branch)
 *
 * Screenshots: frontend/e2e/screenshots/reader-delete/<step>.png
 *
 * Build prerequisite (handled by the auto-fix runner): `npm run build:alt-ui`
 * so the bundle in `frontend/public/modern/` reflects this commit.
 *
 * Runs against the live tunnel/proxy by default (see playwright.config.ts).
 */
import { expect, NOREPLY_CREDS, test } from '../fixtures/base.js';
import { deleteMailboxByUsername } from '../helpers/db-cleanup.js';

const SCREENSHOT_DIR = 'reader-delete';
const PASSWORD = 'tmail-318-reader-delete-2026';

// Stalwart provisions "Deleted Items" as the trash folder on signup. The
// frontend's TRASH_FOLDER_NAMES set in EmailClient.tsx must include this name
// for the window.confirm() gate to fire when the active folder is the trash
// folder. Hardcoding it here keeps the spec readable; if Stalwart's defaults
// change the precondition assertion below will surface the drift loudly.
const STALWART_TRASH_FOLDER = 'Deleted Items';

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

test.describe('TMAIL-318 alt-UI EmailReader Delete soft-deletes to trash and permanently expunges from trash', () => {
  test.beforeAll(() => deleteMailboxByUsername(NOREPLY_CREDS.email));
  test.afterAll(() => deleteMailboxByUsername(NOREPLY_CREDS.email));

  test('Delete from INBOX moves to "Deleted Items"; Delete from "Deleted Items" expunges after confirm', async ({
    page,
    apiSignup,
    takeScreenshot,
    baseURL,
  }) => {
    test.setTimeout(180_000);

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
        name: 'tmail-318-imap',
        host: NOREPLY_CREDS.imap.host,
        port: NOREPLY_CREDS.imap.port,
        username: NOREPLY_CREDS.imap.username,
        password: NOREPLY_CREDS.imap.password,
        encryption: NOREPLY_CREDS.imap.encryption,
        is_default: true,
        // TMAIL-283 / TMAIL-318: Stalwart pre-provisions "Deleted Items" as the
        // trash folder, NOT the Dovecot/legacy "Trash" default. The backend's
        // `delete_message` resolves the per-user `imap_configurations.trash_folder`
        // first and only falls back to "Trash" when null — so without this
        // override the soft-delete COPY would target a non-existent "Trash"
        // mailbox on Stalwart and the CREATE retry can't rescue it (Stalwart
        // does not let users provision a separate "Trash" folder while
        // "Deleted Items" is already serving that role). Setting it explicitly
        // here matches what the onboarding wizard's preset for Stalwart-flavoured
        // BYOK should set in production.
        trash_folder: 'Deleted Items',
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

    // ── 3. precondition + snapshot ──────────────────────────────────────
    const foldersBefore = await fetchFolderList(baseURL, auth);
    expect(
      foldersBefore.includes(STALWART_TRASH_FOLDER),
      `precondition: Stalwart pre-provisions "${STALWART_TRASH_FOLDER}" as the trash folder (folders=${foldersBefore.join(',')})`,
    ).toBe(true);

    const inboxBefore = await fetchFolderMessages(baseURL, auth, 'INBOX');
    expect(inboxBefore.length, 'inbox must contain at least one envelope').toBeGreaterThan(0);
    const targetInboxUid = inboxBefore[0].uid;

    const trashBefore = await fetchFolderMessages(baseURL, auth, STALWART_TRASH_FOLDER);
    const trashUidsBefore = new Set(trashBefore.map((r) => r.uid));

    // ── 4. open first INBOX envelope so EmailReader mounts ──────────────
    const firstRow = page.locator('div.cursor-pointer').first();
    await firstRow.locator('.text-sm.truncate').first().click();
    const readerHeading = page.locator('h2.text-2xl').first();
    await expect(readerHeading).toBeVisible({ timeout: 15_000 });
    await takeScreenshot(page, `${SCREENSHOT_DIR}/02-reader-opened-inbox`);

    // ── 5. find + click Delete (non-permanent label, since INBOX is not trash) ──
    // EmailReader renders aria-label "Delete email from <sender>" when the
    // active folder isn't the trash folder, and "Permanently delete email
    // from <sender>" when it is. Match the non-permanent label here to prove
    // the isPermanentDelete prop is correctly wired for INBOX.
    const softDeleteButton = page.locator(
      'button[aria-label^="Delete email from "]',
    );
    await expect(
      softDeleteButton,
      'reader Delete button on INBOX uses the non-permanent aria-label',
    ).toBeVisible({ timeout: 10_000 });
    await takeScreenshot(page, `${SCREENSHOT_DIR}/03-before-soft-delete`);
    await softDeleteButton.click();

    // ── 6. poll until UID left INBOX AND appeared in "Deleted Items" ────
    let movedOut = false;
    let movedIn = false;
    for (let attempt = 0; attempt < 20 && (!movedOut || !movedIn); attempt++) {
      await page.waitForTimeout(1000);
      const inboxAfter = await fetchFolderMessages(baseURL, auth, 'INBOX');
      movedOut = !inboxAfter.some((r) => r.uid === targetInboxUid);
      const trashAfter = await fetchFolderMessages(baseURL, auth, STALWART_TRASH_FOLDER);
      movedIn = trashAfter.some((r) => !trashUidsBefore.has(r.uid));
    }
    expect(movedOut, `inbox uid=${targetInboxUid} gone after soft delete`).toBe(true);
    expect(movedIn, '"Deleted Items" gained a new UID after soft delete').toBe(true);
    await takeScreenshot(page, `${SCREENSHOT_DIR}/04-after-soft-delete`);

    // ── 7. reader pane must clear (selectedUid reset in onMutate) ───────
    await expect(readerHeading).toHaveCount(0);
    await expect(
      page.getByText(/Select an email to read/i).first(),
    ).toBeVisible({ timeout: 5_000 });

    // ── 8. click the "Deleted Items" folder in the sidebar ──────────────
    // Sidebar renders each folder as <button> wrapping <Icon /> + <span>{name}</span>.
    // Filtering by visible text inside a button is the most robust selector.
    const trashFolderButton = page
      .locator('button')
      .filter({ hasText: STALWART_TRASH_FOLDER })
      .first();
    await expect(trashFolderButton, '"Deleted Items" appears in sidebar').toBeVisible({
      timeout: 10_000,
    });
    await trashFolderButton.click();
    // Folder header in the message list switches to the new active folder.
    await expect(
      page.locator('h2', { hasText: new RegExp(STALWART_TRASH_FOLDER, 'i') }),
    ).toBeVisible({ timeout: 15_000 });
    await takeScreenshot(page, `${SCREENSHOT_DIR}/05-trash-folder-active`);

    // ── 9. snapshot trash UIDs AFTER landing on the folder ──────────────
    // Re-fetch from the backend rather than reusing the post-soft-delete
    // value — by now any pending invalidations have settled and the cache
    // is authoritative. The UID we just soft-deleted is somewhere in here;
    // we'll identify it as "the one not present in trashUidsBefore".
    const trashOnLanding = await fetchFolderMessages(baseURL, auth, STALWART_TRASH_FOLDER);
    expect(
      trashOnLanding.length,
      'trash folder must contain at least the message just soft-deleted',
    ).toBeGreaterThan(0);
    const newlyArrivedTrash = trashOnLanding.find((r) => !trashUidsBefore.has(r.uid));
    expect(
      newlyArrivedTrash,
      'the soft-deleted message is identifiable as the new UID in trash',
    ).toBeDefined();
    const targetTrashUid = newlyArrivedTrash!.uid;

    // ── 10. open the soft-deleted envelope in the trash folder ──────────
    // The list might still be rendering the trash envelopes — wait for any
    // row before clicking. Stalwart only has the one message we just moved
    // in, so .first() is the soft-deleted target.
    const trashRow = page.locator('div.cursor-pointer').first();
    await expect(trashRow).toBeVisible({ timeout: 15_000 });
    await trashRow.locator('.text-sm.truncate').first().click();
    await expect(readerHeading).toBeVisible({ timeout: 15_000 });
    await takeScreenshot(page, `${SCREENSHOT_DIR}/06-reader-opened-trash`);

    // ── 11. register a window.confirm() handler BEFORE the click ────────
    // EmailClient gates permanent delete behind window.confirm(). Playwright
    // surfaces these as `dialog` events. Auto-accept ONCE so the mutation
    // proceeds; if the prompt never fires the test fails on the API poll.
    let confirmFired = false;
    page.once('dialog', async (dialog) => {
      confirmFired = true;
      expect(
        dialog.message(),
        'confirm prompt must mention that the action is irreversible',
      ).toMatch(/permanently|cannot be undone/i);
      await dialog.accept();
    });

    // ── 12. find + click Delete — aria-label must reflect permanence ────
    const permanentDeleteButton = page.locator(
      'button[aria-label^="Permanently delete email from "]',
    );
    await expect(
      permanentDeleteButton,
      'reader Delete button on trash folder uses the permanent aria-label',
    ).toBeVisible({ timeout: 10_000 });
    await takeScreenshot(page, `${SCREENSHOT_DIR}/07-before-permanent-delete`);
    await permanentDeleteButton.click();

    // ── 13. poll until UID has LEFT "Deleted Items" — that's a true EXPUNGE ──
    let expunged = false;
    for (let attempt = 0; attempt < 20 && !expunged; attempt++) {
      await page.waitForTimeout(1000);
      const trashAfter = await fetchFolderMessages(baseURL, auth, STALWART_TRASH_FOLDER);
      expunged = !trashAfter.some((r) => r.uid === targetTrashUid);
    }
    expect(confirmFired, 'window.confirm() prompt fired before permanent delete').toBe(true);
    expect(
      expunged,
      `trash uid=${targetTrashUid} EXPUNGED from "${STALWART_TRASH_FOLDER}" after permanent delete`,
    ).toBe(true);
    await takeScreenshot(page, `${SCREENSHOT_DIR}/08-after-permanent-delete`);

    // Reader pane must clear after the permanent delete too.
    await expect(readerHeading).toHaveCount(0);
    await expect(
      page.getByText(/Select an email to read/i).first(),
    ).toBeVisible({ timeout: 5_000 });
  });
});
