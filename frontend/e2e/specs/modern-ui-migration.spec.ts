/**
 * TMAIL-345: alt-UI ("modern") Settings → Import.
 *
 * Coverage:
 *   1. Sign up + BYOK the noreply mailbox so /api/migration/* has a real
 *      account to attach migration jobs to.
 *   2. Hop into /modern/ via the classic SPA's wand button and navigate
 *      Settings → Import via the side-nav (HARD RULE: menu clicks only).
 *   3. Sub-tab switch: confirm the IMAP / MBOX / PST sub-tabs all mount
 *      and only their own form is visible at a time.
 *   4. IMAP wizard — fill host/port/user/password, submit. Verify via
 *      GET /api/migration that exactly one new IMAP job exists with the
 *      submitted source_host / source_user. This is the SPA before/after
 *      API state assertion the HARD RULE demands.
 *   5. MBOX import — fill the file path, submit. GET /api/migration shows
 *      a second job with job_type='mbox' and the submitted path.
 *   6. Cancel: click the X on the freshly created IMAP job. GET confirms
 *      its status flipped to 'cancelled'.
 *   7. PST upload — drop a tiny synthetic .pst file via the hidden file
 *      input (multipart). GET /api/migration/pst confirms the row exists.
 *
 * Screenshots: frontend/e2e/screenshots/migration/<step>.png
 *
 * Build prerequisite: `npm run build:alt-ui` so frontend/public/modern/
 * reflects this commit. The auto-fix runner handles that.
 *
 * Default browser: Firefox (per the global E2E HARD RULE).
 */
import path from 'path';
import { fileURLToPath } from 'url';
import { expect, NOREPLY_CREDS, test } from '../fixtures/base.js';
import { deleteMailboxByUsername } from '../helpers/db-cleanup.js';

const __filename = fileURLToPath(import.meta.url);
const __dirname = path.dirname(__filename);

const SCREENSHOT_DIR = 'migration';
const PASSWORD = 'tmail-345-migration-2026';

interface MigrationJobRow {
  id: string;
  job_type: 'imap' | 'mbox';
  status: 'pending' | 'running' | 'completed' | 'failed' | 'cancelled';
  source_host: string | null;
  source_user: string | null;
  mbox_file_path: string | null;
  created_at: string;
}

interface PstImportRow {
  id: string;
  filename: string;
  status: 'pending' | 'processing' | 'completed' | 'failed';
  target_folder: string;
  created_at: string;
}

test.describe('TMAIL-345 alt-UI Import pane — IMAP / MBOX / PST migration wizard', () => {
  test.beforeAll(() => deleteMailboxByUsername(NOREPLY_CREDS.email));
  test.afterAll(() => deleteMailboxByUsername(NOREPLY_CREDS.email));

  test('Import tab routes via menu, IMAP/MBOX/PST jobs round-trip through the backend, cancel flips status', async ({
    page,
    apiSignup,
    takeScreenshot,
    baseURL,
  }) => {
    test.setTimeout(180_000);

    // ── 1. signup + BYOK ────────────────────────────────────────────────
    const tokens = await apiSignup(NOREPLY_CREDS.email, PASSWORD);
    const auth = {
      Authorization: `Bearer ${tokens.access_token}`,
      'Content-Type': 'application/json',
    };
    const imapResp = await fetch(`${baseURL}/api/imap-configs`, {
      method: 'POST',
      headers: auth,
      body: JSON.stringify({
        name: 'tmail-345-imap',
        host: NOREPLY_CREDS.imap.host,
        port: NOREPLY_CREDS.imap.port,
        username: NOREPLY_CREDS.imap.username,
        password: NOREPLY_CREDS.imap.password,
        encryption: NOREPLY_CREDS.imap.encryption,
        is_default: true,
      }),
    });
    expect(imapResp.status, 'IMAP config').toBe(201);

    // ── 2. open classic /app then hop to /modern/ ───────────────────────
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
    await takeScreenshot(page, `${SCREENSHOT_DIR}/01-modern-ui-loaded`);

    // ── 3. Navigate Settings → Import via the side-nav ──────────────────
    await page.getByTestId('navbar-settings-link').click();
    await page.waitForURL(/#\/settings\/profile/i, { timeout: 10_000 });
    await page.getByTestId('settings-tab-import').click();
    await page.waitForURL(/#\/settings\/import/i, { timeout: 10_000 });
    await expect(page.getByTestId('settings-tab-import-pane')).toBeVisible({
      timeout: 10_000,
    });
    // IMAP sub-tab is the default; its form should be the visible one.
    await expect(page.getByTestId('migration-imap-form')).toBeVisible();
    await expect(page.getByTestId('migration-mbox-form')).toHaveCount(0);
    await expect(page.getByTestId('migration-pst-form')).toHaveCount(0);
    await expect(page.getByTestId('migration-jobs-empty')).toBeVisible();
    await expect(page.getByTestId('pst-imports-empty')).toBeVisible();
    await takeScreenshot(page, `${SCREENSHOT_DIR}/02-import-pane-loaded`);

    // ── 4. Snapshot backend state BEFORE any migrations ─────────────────
    const before = await fetch(`${baseURL}/api/migration`, { headers: auth });
    expect(before.status, 'GET /api/migration before').toBe(200);
    expect(
      (await before.json()) as MigrationJobRow[],
      'no migration jobs at start',
    ).toHaveLength(0);

    // ── 5. IMAP wizard — fill + submit ──────────────────────────────────
    const IMAP_HOST = `imap.example-${Date.now()}.com`;
    const IMAP_USER = 'someone@example.com';
    await page.getByTestId('migration-imap-host').fill(IMAP_HOST);
    await page.getByTestId('migration-imap-port').fill('993');
    await page.getByTestId('migration-imap-user').fill(IMAP_USER);
    await page.getByTestId('migration-imap-password').fill('app-password-123');
    // SSL stays checked (the default).
    await takeScreenshot(page, `${SCREENSHOT_DIR}/03-imap-form-filled`);
    await page.getByTestId('migration-imap-submit').click();

    // Backend round-trip — poll until the IMAP job we just submitted shows
    // up in /api/migration, then capture the row for the cancel test below.
    let imapJob: MigrationJobRow | undefined;
    await expect
      .poll(
        async () => {
          const r = await fetch(`${baseURL}/api/migration`, { headers: auth });
          if (!r.ok) return null;
          const rows = (await r.json()) as MigrationJobRow[];
          imapJob = rows.find(
            (j) =>
              j.job_type === 'imap' &&
              j.source_host === IMAP_HOST &&
              j.source_user === IMAP_USER,
          );
          return imapJob ? 'found' : null;
        },
        { timeout: 10_000, message: 'IMAP migration job created' },
      )
      .toBe('found');
    expect(imapJob, 'IMAP job row captured').toBeTruthy();

    // UI shows the new job too.
    await expect(page.getByTestId('migration-jobs-list')).toBeVisible({
      timeout: 10_000,
    });
    await takeScreenshot(page, `${SCREENSHOT_DIR}/04-imap-job-created`);

    // ── 6. MBOX import sub-tab ──────────────────────────────────────────
    await page.getByTestId('migration-subtab-mbox').click();
    await expect(page.getByTestId('migration-mbox-form')).toBeVisible();
    await expect(page.getByTestId('migration-imap-form')).toHaveCount(0);

    const MBOX_PATH = `/srv/uploads/takeout-${Date.now()}.mbox`;
    await page.getByTestId('migration-mbox-path').fill(MBOX_PATH);
    await takeScreenshot(page, `${SCREENSHOT_DIR}/05-mbox-form-filled`);
    await page.getByTestId('migration-mbox-submit').click();

    await expect
      .poll(
        async () => {
          const r = await fetch(`${baseURL}/api/migration`, { headers: auth });
          if (!r.ok) return null;
          const rows = (await r.json()) as MigrationJobRow[];
          const found = rows.find(
            (j) =>
              j.job_type === 'mbox' && j.mbox_file_path === MBOX_PATH,
          );
          return found ? 'found' : null;
        },
        { timeout: 10_000, message: 'MBOX import job created' },
      )
      .toBe('found');
    await takeScreenshot(page, `${SCREENSHOT_DIR}/06-mbox-job-created`);

    // ── 7. Cancel the IMAP job — backend state flips to cancelled ──────
    await page
      .getByTestId(`migration-job-cancel-${imapJob!.id}`)
      .click({ timeout: 10_000 });
    await expect
      .poll(
        async () => {
          const r = await fetch(`${baseURL}/api/migration/${imapJob!.id}`, {
            headers: auth,
          });
          if (!r.ok) return null;
          const j = (await r.json()) as MigrationJobRow;
          return j.status;
        },
        { timeout: 10_000, message: 'IMAP job transitions to cancelled' },
      )
      .toBe('cancelled');
    await takeScreenshot(page, `${SCREENSHOT_DIR}/07-imap-job-cancelled`);

    // ── 8. PST sub-tab — multipart upload round-trip ────────────────────
    await page.getByTestId('migration-subtab-pst').click();
    await expect(page.getByTestId('migration-pst-form')).toBeVisible();
    await expect(page.getByTestId('migration-mbox-form')).toHaveCount(0);

    // Build a tiny synthetic .pst file on the fly so we don't ship a
    // multi-MB fixture binary in git. The backend validates the
    // *extension* only; the readpst worker will fail later in the queue,
    // but the upload + DB row creation succeed which is what this spec
    // is asserting.
    const pstFixture = path.join(__dirname, `__synthetic-${Date.now()}.pst`);
    const fs = await import('node:fs/promises');
    await fs.writeFile(pstFixture, Buffer.from('not a real pst'));

    try {
      await page
        .getByTestId('migration-pst-file-input')
        .setInputFiles(pstFixture);
      await expect(page.getByTestId('migration-pst-dropzone')).toContainText(
        /__synthetic-/,
        { timeout: 5_000 },
      );
      await takeScreenshot(page, `${SCREENSHOT_DIR}/08-pst-file-picked`);
      await page.getByTestId('migration-pst-submit').click();

      // Round-trip: GET /api/migration/pst shows the upload row.
      await expect
        .poll(
          async () => {
            const r = await fetch(`${baseURL}/api/migration/pst`, {
              headers: auth,
            });
            if (!r.ok) return null;
            const rows = (await r.json()) as PstImportRow[];
            const found = rows.find(
              (row) =>
                /__synthetic-/.test(row.filename) &&
                row.target_folder === 'INBOX',
            );
            return found ? 'found' : null;
          },
          { timeout: 15_000, message: 'PST import row created' },
        )
        .toBe('found');

      // UI shows the new row in the PST history.
      await expect(page.getByTestId('pst-imports-list')).toBeVisible({
        timeout: 10_000,
      });
      await takeScreenshot(page, `${SCREENSHOT_DIR}/09-pst-imported`);
    } finally {
      await fs.unlink(pstFixture).catch(() => undefined);
    }
  });
});
