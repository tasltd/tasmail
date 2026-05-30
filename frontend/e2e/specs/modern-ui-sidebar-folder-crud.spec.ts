/**
 * TMAIL-324: alt-UI ("modern") Sidebar folder add/delete must persist to the
 * backend via POST/DELETE /api/folders — no more local-only `extraLocalFolders`
 * state that evaporated on reload.
 *
 * Coverage (SPA E2E HARD RULE: validate via API state before AND after):
 *   1. Signup + BYOK the noreply mailbox so /api/folders has real data
 *   2. Hop into /modern/ via the classic SPA's wand button
 *   3. Capture folder list via GET /api/folders BEFORE the UI action
 *   4. Click "New folder" → type a unique name → blur to submit
 *   5. Capture folder list AFTER — assert the new folder is present on the
 *      live IMAP server (not just in React state)
 *   6. Reload the page — assert the new folder is STILL there (this is what
 *      broke with the local-only state)
 *   7. Hover the new folder and click the delete (×) button
 *   8. Capture folder list AFTER delete — assert the folder is gone from the
 *      live IMAP server
 *   9. Reload — assert the folder remains deleted
 *
 * Screenshots: frontend/e2e/screenshots/sidebar-folder-crud/<step>.png
 *
 * Runs against the live tunnel/proxy by default (see playwright.config.ts).
 */
import { expect, NOREPLY_CREDS, test } from '../fixtures/base.js';
import { deleteMailboxByUsername } from '../helpers/db-cleanup.js';

const SCREENSHOT_DIR = 'sidebar-folder-crud';
const PASSWORD = 'tmail-324-folder-crud-2026';

interface FolderRow {
  name: string;
  delimiter: string;
  messages: number | null;
  unseen: number | null;
}

async function fetchFolders(
  baseURL: string | undefined,
  auth: Record<string, string>,
): Promise<FolderRow[]> {
  const resp = await fetch(`${baseURL}/api/folders`, { headers: auth });
  if (!resp.ok) return [];
  return (await resp.json()) as FolderRow[];
}

function uniqueFolderName(): string {
  // Stalwart accepts UTF-8 mailbox names; we stay ASCII so the test does not
  // accidentally fail on a less permissive server. The timestamp keeps the
  // name distinct across overlapping test runs.
  return `Projects_TMAIL324_${Date.now()}`;
}

test.describe('TMAIL-324 alt-UI Sidebar folder add/delete persists to backend', () => {
  test.beforeAll(() => deleteMailboxByUsername(NOREPLY_CREDS.email));
  test.afterAll(() => deleteMailboxByUsername(NOREPLY_CREDS.email));

  test('add folder + delete folder round-trips through the live IMAP server', async ({
    page,
    apiSignup,
    takeScreenshot,
    baseURL,
  }) => {
    test.setTimeout(150_000);

    // ── 1. signup + BYOK so /api/folders returns real rows ─────────────────
    const tokens = await apiSignup(NOREPLY_CREDS.email, PASSWORD);
    const auth = {
      Authorization: `Bearer ${tokens.access_token}`,
      'Content-Type': 'application/json',
    };
    const imapResp = await fetch(`${baseURL}/api/imap-configs`, {
      method: 'POST',
      headers: auth,
      body: JSON.stringify({
        name: 'tmail-324-imap',
        host: NOREPLY_CREDS.imap.host,
        port: NOREPLY_CREDS.imap.port,
        username: NOREPLY_CREDS.imap.username,
        password: NOREPLY_CREDS.imap.password,
        encryption: NOREPLY_CREDS.imap.encryption,
        is_default: true,
      }),
    });
    expect(imapResp.status, 'IMAP config').toBe(201);

    // ── 2. open classic /app and hop to /modern/ via the wand button ──────
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
    await takeScreenshot(page, `${SCREENSHOT_DIR}/01-modern-loaded`);

    // ── 3. capture folder list BEFORE so we can prove the mutation worked ─
    const newName = uniqueFolderName();
    const foldersBefore = await fetchFolders(baseURL, auth);
    expect(
      foldersBefore.some((f) => f.name === newName),
      `${newName} must not exist before we create it`,
    ).toBe(false);
    await takeScreenshot(page, `${SCREENSHOT_DIR}/02-before-add`);

    // ── 4. click "New folder", type the name, blur to submit ───────────────
    await page.getByTestId('new-folder-button').click();
    const input = page.getByTestId('new-folder-input');
    await expect(input).toBeVisible({ timeout: 5_000 });
    await input.fill(newName);
    await takeScreenshot(page, `${SCREENSHOT_DIR}/03-name-typed`);
    // Press Enter — onKeyDown=Enter calls handleAddFolder which fires the
    // POST /api/folders mutation and invalidates the ['folders'] query.
    await input.press('Enter');

    // ── 5. assert backend state changed (SPA E2E HARD RULE) ────────────────
    let appeared = false;
    let foldersAfterAdd: FolderRow[] = [];
    for (let attempt = 0; attempt < 15 && !appeared; attempt++) {
      await page.waitForTimeout(1000);
      foldersAfterAdd = await fetchFolders(baseURL, auth);
      if (foldersAfterAdd.some((f) => f.name === newName)) {
        appeared = true;
      }
    }
    expect(
      appeared,
      `new folder ${newName} must appear in GET /api/folders after the UI submit`,
    ).toBe(true);

    // The new folder must also render in the sidebar — the ['folders'] query
    // re-fetches after invalidation and the sidebar re-renders.
    await expect(
      page.locator(`button:has-text("${newName}")`).first(),
    ).toBeVisible({ timeout: 10_000 });
    await takeScreenshot(page, `${SCREENSHOT_DIR}/04-after-add`);

    // ── 6. reload and confirm persistence (this is what broke before) ──────
    await page.reload();
    await expect(page.locator('h2', { hasText: /INBOX/i })).toBeVisible({
      timeout: 25_000,
    });
    await expect(
      page.locator(`button:has-text("${newName}")`).first(),
    ).toBeVisible({ timeout: 15_000 });
    await takeScreenshot(page, `${SCREENSHOT_DIR}/05-after-reload-still-there`);

    // ── 7. hover the new folder and click the delete (×) button ────────────
    // The delete button is opacity-0 group-hover:opacity-100 so we hover
    // first; on Firefox a programmatic click() still fires even without a
    // visible hover state, so the hover is belt-and-braces.
    const folderRow = page.locator(`button:has-text("${newName}")`).first();
    await folderRow.hover();
    const deleteBtn = page.getByTestId(`delete-folder-${newName}`);
    await expect(deleteBtn).toBeAttached({ timeout: 5_000 });
    await deleteBtn.click({ force: true });

    // ── 8. assert backend state changed (folder gone from live IMAP) ───────
    let removed = false;
    for (let attempt = 0; attempt < 15 && !removed; attempt++) {
      await page.waitForTimeout(1000);
      const foldersAfterDelete = await fetchFolders(baseURL, auth);
      if (!foldersAfterDelete.some((f) => f.name === newName)) {
        removed = true;
      }
    }
    expect(
      removed,
      `folder ${newName} must be gone from GET /api/folders after the UI delete`,
    ).toBe(true);
    await takeScreenshot(page, `${SCREENSHOT_DIR}/06-after-delete`);

    // ── 9. reload and confirm the delete persists ──────────────────────────
    await page.reload();
    await expect(page.locator('h2', { hasText: /INBOX/i })).toBeVisible({
      timeout: 25_000,
    });
    await expect(
      page.locator(`button:has-text("${newName}")`),
    ).toHaveCount(0);
    await takeScreenshot(page, `${SCREENSHOT_DIR}/07-after-reload-still-gone`);
  });
});
