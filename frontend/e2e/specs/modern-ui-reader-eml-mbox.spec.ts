/**
 * TMAIL-349: alt-UI ("modern") EML export per-message + per-folder MBOX
 * export / EML import.
 *
 * Sister-spec to modern-ui-reader-download-attachment.spec.ts (TMAIL-320). The
 * EML export button on the reader and the MBOX / Import EML items in the
 * folder header dropdown all exchange binary bodies with the backend, so the
 * spec validates the full round-trip (snapshot API state → click UI → compare
 * downloaded bytes / re-read API state).
 *
 * Coverage:
 *   1. Sign up + BYOK the noreply mailbox so /api/folders/INBOX/messages
 *      becomes reachable
 *   2. Seed an inbox fixture via POST /api/folders/INBOX/import-eml so there
 *      is at least one message to open (this also smoke-tests the same
 *      endpoint the Import EML menu item calls)
 *   3. Hop into /modern/ via the classic SPA's wand button
 *   4. Open the seeded fixture and click "Export EML" — Playwright captures
 *      the download and we assert the saved bytes equal the
 *      `GET /eml` snapshot
 *   5. Open the folder-actions dropdown and click "Export folder as MBOX" —
 *      assert the download starts with the mboxo `From ` separator and
 *      contains the fixture's subject inside the embedded RFC822 body
 *   6. Click "Import EML into folder" and feed it a fresh fixture file —
 *      assert the new envelope appears in /api/folders/INBOX/messages after
 *      a brief poll (SPA E2E HARD RULE: backend state must change)
 *   7. Screenshot at each key validation point
 *
 * Screenshots: frontend/e2e/screenshots/reader-eml-mbox/<step>.png
 *
 * Build prerequisite: `npm run build:alt-ui` so the bundle in
 * `frontend/public/modern/` reflects this commit (the auto-fix runner does
 * this automatically; local runs must do it once).
 *
 * Runs against the live tunnel/proxy by default (see playwright.config.ts).
 */
import { promises as fs } from 'node:fs';
import path from 'node:path';
import os from 'node:os';
import { expect, NOREPLY_CREDS, test } from '../fixtures/base.js';
import { deleteMailboxByUsername } from '../helpers/db-cleanup.js';

const SCREENSHOT_DIR = 'reader-eml-mbox';
const PASSWORD = 'tmail-349-eml-mbox-2026';
const NOW = Date.now();
const SUBJECT_EXPORT = `TMAIL-349 export fixture ${NOW}`;
const SUBJECT_IMPORT = `TMAIL-349 import fixture ${NOW}`;
// Use a dedicated custom folder for the MBOX export test so the export is
// bounded to a single fixture message — the upstream noreply mailbox has
// 1000+ INBOX messages and fetching them all over IMAP for the mbox blob
// blows past Playwright's default 30s download timeout.
const TEST_FOLDER = `TMAIL349_${NOW}`;

interface EnvelopeRow {
  uid: number;
  subject: string | null;
}

function buildEml(subject: string, body: string): string {
  // Plain-text RFC822 message — CRLF line endings are mandatory for IMAP
  // APPEND on Stalwart (matches modern-ui-reader-download-attachment.spec.ts).
  return [
    `From: ${NOREPLY_CREDS.email}`,
    `To: ${NOREPLY_CREDS.email}`,
    `Subject: ${subject}`,
    `MIME-Version: 1.0`,
    `Content-Type: text/plain; charset="utf-8"`,
    `Content-Transfer-Encoding: 7bit`,
    '',
    body,
    '',
  ].join('\r\n');
}

async function fetchFolder(
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

async function waitForSubject(
  baseURL: string | undefined,
  auth: Record<string, string>,
  folder: string,
  subject: string,
  attempts = 15,
): Promise<number | undefined> {
  for (let i = 0; i < attempts; i++) {
    const rows = await fetchFolder(baseURL, auth, folder);
    const hit = rows.find((r) => (r.subject ?? '').includes(subject));
    if (hit) return hit.uid;
    await new Promise((r) => setTimeout(r, 1000));
  }
  return undefined;
}

test.describe('TMAIL-349 alt-UI EML export + folder MBOX/EML actions', () => {
  test.beforeAll(() => deleteMailboxByUsername(NOREPLY_CREDS.email));
  test.afterAll(() => deleteMailboxByUsername(NOREPLY_CREDS.email));

  test('reader Export EML downloads RFC822, folder MBOX/EML actions round-trip via API', async ({
    page,
    apiSignup,
    takeScreenshot,
    baseURL,
  }) => {
    test.setTimeout(180_000);

    // ── 1. signup + BYOK ───────────────────────────────────────────────────
    const tokens = await apiSignup(NOREPLY_CREDS.email, PASSWORD);
    const auth = {
      Authorization: `Bearer ${tokens.access_token}`,
      'Content-Type': 'application/json',
    };
    const authBin = { Authorization: `Bearer ${tokens.access_token}` };
    const imapResp = await fetch(`${baseURL}/api/imap-configs`, {
      method: 'POST',
      headers: auth,
      body: JSON.stringify({
        name: 'tmail-349-imap',
        host: NOREPLY_CREDS.imap.host,
        port: NOREPLY_CREDS.imap.port,
        username: NOREPLY_CREDS.imap.username,
        password: NOREPLY_CREDS.imap.password,
        encryption: NOREPLY_CREDS.imap.encryption,
        is_default: true,
      }),
    });
    expect(imapResp.status, 'IMAP config must be created').toBe(201);

    // ── 2. create a dedicated test folder + seed the export fixture into it
    //       via import-eml. The custom folder keeps MBOX export bounded to a
    //       single message — INBOX on the shared noreply mailbox has 1000+
    //       messages and the all-UIDs IMAP fetch blows past Playwright's 30s
    //       download timeout. The custom folder also smoke-tests the same
    //       POST /api/folders/{folder}/import-eml endpoint the Import EML
    //       menu item uses.
    const createFolderResp = await fetch(`${baseURL}/api/folders`, {
      method: 'POST',
      headers: auth,
      body: JSON.stringify({ name: TEST_FOLDER }),
    });
    expect(
      [200, 201].includes(createFolderResp.status),
      `create custom folder ${TEST_FOLDER} must succeed (got ${createFolderResp.status})`,
    ).toBe(true);

    const exportEml = buildEml(
      SUBJECT_EXPORT,
      'Fixture for the reader Export EML round-trip test.',
    );
    const seedResp = await fetch(
      `${baseURL}/api/folders/${encodeURIComponent(TEST_FOLDER)}/import-eml`,
      {
        method: 'POST',
        headers: {
          Authorization: `Bearer ${tokens.access_token}`,
          'Content-Type': 'message/rfc822',
        },
        body: exportEml,
      },
    );
    expect(seedResp.status, 'seed import-eml must succeed').toBe(201);

    const exportUid = await waitForSubject(
      baseURL,
      auth,
      TEST_FOLDER,
      SUBJECT_EXPORT,
    );
    expect(
      exportUid,
      `seed envelope "${SUBJECT_EXPORT}" must appear in ${TEST_FOLDER}`,
    ).toBeDefined();

    // SPA E2E HARD RULE: snapshot the EML the backend will emit BEFORE the
    // UI action so the post-click comparison is byte-exact.
    const emlSnapshotResp = await fetch(
      `${baseURL}/api/folders/${encodeURIComponent(TEST_FOLDER)}/messages/${exportUid}/eml`,
      { headers: authBin },
    );
    expect(emlSnapshotResp.status, 'GET .eml must succeed').toBe(200);
    expect(emlSnapshotResp.headers.get('content-type')).toContain('message/rfc822');
    const expectedEmlBytes = Buffer.from(await emlSnapshotResp.arrayBuffer());
    expect(expectedEmlBytes.length, 'EML snapshot must be non-empty').toBeGreaterThan(0);

    // ── 3. open classic /app and hop to /modern/ ───────────────────────────
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
    await takeScreenshot(page, `${SCREENSHOT_DIR}/01-inbox-rendered`);

    // Navigate to the dedicated test folder via the sidebar — menu clicks
    // only (E2E navigation hard rule). The folder appears in the sidebar
    // because GET /api/folders surfaces it after the custom-folder create.
    const sidebarFolderLink = page
      .locator('button, a', { hasText: new RegExp(`^${TEST_FOLDER}$`) })
      .first();
    await expect(
      sidebarFolderLink,
      `sidebar must surface the custom folder ${TEST_FOLDER}`,
    ).toBeVisible({ timeout: 15_000 });
    await sidebarFolderLink.click();
    await expect(
      page.locator('h2', { hasText: new RegExp(TEST_FOLDER, 'i') }),
    ).toBeVisible({ timeout: 15_000 });
    await expect(page.locator('div.cursor-pointer').first()).toBeVisible({
      timeout: 25_000,
    });

    // ── 4. open the seeded message and click Export EML ────────────────────
    const fixtureRow = page
      .locator('div.cursor-pointer', { hasText: SUBJECT_EXPORT })
      .first();
    await expect(fixtureRow, 'fixture row must be visible').toBeVisible({
      timeout: 25_000,
    });
    await fixtureRow.locator('.text-sm.truncate').first().click();
    const readerHeading = page.locator('h2.text-2xl').first();
    await expect(readerHeading).toContainText(SUBJECT_EXPORT, { timeout: 15_000 });
    await takeScreenshot(page, `${SCREENSHOT_DIR}/02-reader-opened`);

    const exportEmlBtn = page.locator('[data-testid="modern-export-eml"]');
    await expect(exportEmlBtn, 'reader Export EML button must render').toBeVisible({
      timeout: 10_000,
    });
    await takeScreenshot(page, `${SCREENSHOT_DIR}/03-before-export-eml-click`);

    const [emlDownload] = await Promise.all([
      page.waitForEvent('download', { timeout: 20_000 }),
      exportEmlBtn.click(),
    ]);
    expect(
      emlDownload.suggestedFilename(),
      'Export EML filename must follow the backend pattern',
    ).toBe(`message_${exportUid}.eml`);
    const emlSavedPath = await emlDownload.path();
    expect(emlSavedPath, 'Playwright must persist the EML download').toBeTruthy();
    const emlSavedBytes = await fs.readFile(emlSavedPath);
    expect(
      emlSavedBytes.equals(expectedEmlBytes),
      `EML download bytes (${emlSavedBytes.length}) must equal the snapshot (${expectedEmlBytes.length})`,
    ).toBe(true);
    await takeScreenshot(page, `${SCREENSHOT_DIR}/04-after-export-eml-click`);

    // ── 5. folder header → Export folder as MBOX ─────────────────────────
    const exportMboxBtn = page.locator('[data-testid="modern-export-mbox"]');
    await expect(
      exportMboxBtn,
      'folder header Export MBOX button must render',
    ).toBeVisible({ timeout: 10_000 });
    await takeScreenshot(page, `${SCREENSHOT_DIR}/05-folder-header-actions`);

    const [mboxDownload] = await Promise.all([
      page.waitForEvent('download', { timeout: 60_000 }),
      exportMboxBtn.click(),
    ]);
    expect(
      mboxDownload.suggestedFilename(),
      'MBOX download must be named after the folder',
    ).toBe(`${TEST_FOLDER}.mbox`);
    const mboxSavedPath = await mboxDownload.path();
    const mboxBytes = await fs.readFile(mboxSavedPath);
    const mboxText = mboxBytes.toString('utf8');
    expect(mboxBytes.length, 'MBOX export must be non-empty').toBeGreaterThan(0);
    expect(
      mboxText.startsWith('From '),
      'MBOX must begin with an mboxo "From " separator',
    ).toBe(true);
    expect(
      mboxText.includes(SUBJECT_EXPORT),
      'MBOX body must contain the seeded fixture subject',
    ).toBe(true);
    await takeScreenshot(page, `${SCREENSHOT_DIR}/06-after-mbox-download`);

    // ── 6. folder actions → Import EML into folder ─────────────────────────
    // Snapshot folder state BEFORE the import so we can prove the backend
    // changed (SPA E2E HARD RULE — no UI-only assertions).
    const beforeImport = await fetchFolder(baseURL, auth, TEST_FOLDER);
    const beforeCount = beforeImport.length;

    // Write the import fixture to a temp file because Playwright's
    // setInputFiles needs a real path on disk.
    const tmpDir = await fs.mkdtemp(path.join(os.tmpdir(), 'tmail-349-'));
    const importPath = path.join(tmpDir, 'tmail-349-import.eml');
    const importEml = buildEml(
      SUBJECT_IMPORT,
      'Fixture for the Import EML menu round-trip test.',
    );
    await fs.writeFile(importPath, importEml, 'utf8');

    const importBtn = page.locator('[data-testid="modern-import-eml-trigger"]');
    await expect(
      importBtn,
      'folder header Import EML button must render',
    ).toBeVisible({ timeout: 10_000 });
    await takeScreenshot(page, `${SCREENSHOT_DIR}/07-folder-header-import`);

    // Drive the hidden file input directly — clicking the button opens the
    // native file chooser which Playwright can't drive in Firefox without
    // `filechooser`. Setting the input value is the canonical pattern for
    // hidden inputs (https://playwright.dev/docs/api/class-locator#locator-set-input-files).
    const hiddenInput = page.locator('[data-testid="modern-import-eml-input"]');
    await hiddenInput.setInputFiles(importPath);

    // Backend state must reflect the new envelope. Poll for ~20s.
    const importedUid = await waitForSubject(
      baseURL,
      auth,
      TEST_FOLDER,
      SUBJECT_IMPORT,
      20,
    );
    expect(
      importedUid,
      `imported envelope "${SUBJECT_IMPORT}" must appear via /api/folders/${TEST_FOLDER}/messages`,
    ).toBeDefined();

    const afterImport = await fetchFolder(baseURL, auth, TEST_FOLDER);
    expect(
      afterImport.length,
      `folder count must grow after Import EML (was ${beforeCount}, now ${afterImport.length})`,
    ).toBeGreaterThan(beforeCount);
    await takeScreenshot(page, `${SCREENSHOT_DIR}/08-after-import`);

    // Cleanup the temp file the test wrote.
    await fs.rm(tmpDir, { recursive: true, force: true });
  });
});
