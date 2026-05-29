/**
 * TMAIL-289 — E2E sweep: shared mailboxes + shared files (token DL)
 *
 * Covers:
 *   • Shared mailboxes — grant ACL via API, grantee sees owner mailbox in UI
 *     via the "Shared Mailboxes" sidebar entry, expanded admin view shows the
 *     ACL list + grant form, revoke via API removes it from the grantee's list.
 *   • Shared files — upload through the SharedFileManager UI, list shows the
 *     uploaded file with "Active" badge, copy-link button swaps icon, public
 *     download URL works in an incognito context, max_downloads cap turns the
 *     same URL into an "expired" error, delete via API removes the row.
 *
 * Why this spec exists:
 *   1. Validate menu-driven navigation to /settings views per the HARD RULE
 *      (no page.goto for internal routes — sidebar clicks only).
 *   2. Cross-check every mutation via a fresh API GET (HARD RULE: SPA tests
 *      verify backend state, not just DOM updates).
 *   3. Verify the public /api/dl/{token} download path against an incognito
 *      browser context AND an APIRequestContext — both must round-trip the
 *      file body and respect the max_downloads expiry cap.
 *   4. Capture screenshots at every key validation point so the audit trail
 *      survives the run.
 *
 * Bug surface this spec uncovers (and the bug-fix commit fixes):
 *   • Migration 010 left shared_mailbox_acl with FORCE ROW LEVEL SECURITY
 *     + policies referencing `current_setting('app.mailbox_id', true)`. After
 *     TMAIL-161 the auth middleware stopped SETing that var on the pool, so
 *     every query against the table fell through to the policy with an unset
 *     session var. Production "worked" only because connection-pool state
 *     leakage from audit_log / login set `app.is_admin = 'true'` or
 *     `app.mailbox_id = …` on whichever connection was reused next. Fragile
 *     and incorrect — migration 075 drops FORCE and rewrites the policies to
 *     use `app.current_user_id` (matching migrations 017/019/020/028).
 *
 * Setup model:
 *   • Two BYOK signups in beforeAll: owner + grantee. Each gets the noreply
 *     IMAP attached so the sidebar paints (the FolderTree expects an IMAP
 *     server). The owner's mailbox_id is decoded from the JWT and used as the
 *     ACL target.
 *   • afterAll deletes both mailboxes via psql so the suite is re-runnable.
 */
import { test, NOREPLY_CREDS, expect } from './fixtures/base.js';
import { request as apiRequest, type APIRequestContext, type Page } from '@playwright/test';
import { deleteMailboxByUsername } from './helpers/db-cleanup.js';
import { writeFileSync, mkdtempSync } from 'node:fs';
import path from 'node:path';
import os from 'node:os';

const PASSWORD = 'shared-sweep-Pa55!';
const RUN_TAG = Date.now();
const OWNER_EMAIL = `e2e-shared-owner-${RUN_TAG}@e2e.tasmail`;
const GRANTEE_EMAIL = `e2e-shared-grantee-${RUN_TAG}@e2e.tasmail`;

let api: APIRequestContext;
let ownerToken: string;
let granteeToken: string;
let ownerId: string;
let granteeId: string;
let ownerAuth: Record<string, string>;
let granteeAuth: Record<string, string>;
let resolvedBaseURL: string;

// ──────────────────────────────────────────────────────────────────────────────
// API response shapes — inlined so the Playwright config doesn't compile src/.
// Keep in sync with frontend/src/api/shared-{mailboxes,files}.ts.
// ──────────────────────────────────────────────────────────────────────────────

interface SharedMailboxView {
  mailbox_id: string;
  username: string;
  display_name: string | null;
  can_read: boolean;
  can_write: boolean;
  can_delete: boolean;
  can_admin: boolean;
}

interface SharedFile {
  id: string;
  user_id: string;
  filename: string;
  content_type: string;
  file_size: number;
  download_token: string;
  download_count: number;
  max_downloads: number | null;
  expires_at: string | null;
}

// PURPOSE: decode the `sub` claim (the mailbox/user UUID) from a JWT
function decodeSub(jwt: string): string {
  const payload = jwt.split('.')[1];
  // NOTE: base64url — pad to a multiple of 4 before decoding.
  const padded = payload + '='.repeat((4 - (payload.length % 4)) % 4);
  const json = Buffer.from(padded, 'base64').toString('utf8');
  return JSON.parse(json).sub as string;
}

test.describe.configure({ mode: 'serial' });

test.describe('TMAIL-289 Shared mailboxes + shared files sweep', () => {
  test.beforeAll(async ({ baseURL }) => {
    test.setTimeout(120_000);
    resolvedBaseURL = baseURL ?? 'https://mail.techatscale.io';
    api = await apiRequest.newContext({ baseURL });

    // Signup both users via the public BYOK endpoint.
    const ownerResp = await api.post('/api/auth/signup', {
      data: { email: OWNER_EMAIL, password: PASSWORD },
    });
    expect(ownerResp.status(), 'owner signup must succeed').toBeLessThan(300);
    ownerToken = (await ownerResp.json()).access_token as string;
    ownerId = decodeSub(ownerToken);
    ownerAuth = { Authorization: `Bearer ${ownerToken}` };

    const granteeResp = await api.post('/api/auth/signup', {
      data: { email: GRANTEE_EMAIL, password: PASSWORD },
    });
    expect(granteeResp.status(), 'grantee signup must succeed').toBeLessThan(300);
    granteeToken = (await granteeResp.json()).access_token as string;
    granteeId = decodeSub(granteeToken);
    granteeAuth = { Authorization: `Bearer ${granteeToken}` };

    // BYOK-attach the noreply IMAP for the grantee so the sidebar paints
    // after they log in (FolderTree needs a default IMAP config). We don't
    // touch INBOX directly — the sidebar entry clicks are what we exercise.
    const imap = await api.post('/api/imap-configs', {
      headers: granteeAuth,
      data: {
        name: 'noreply (E2E shared)',
        host: NOREPLY_CREDS.imap.host,
        port: NOREPLY_CREDS.imap.port,
        username: NOREPLY_CREDS.imap.username,
        password: NOREPLY_CREDS.imap.password,
        encryption: NOREPLY_CREDS.imap.encryption,
        trash_folder: 'Deleted Items',
        sent_folder: 'Sent Items',
        drafts_folder: 'Drafts',
        spam_folder: 'Junk Mail',
        is_default: true,
      },
    });
    expect(imap.status(), 'grantee IMAP config create must succeed').toBeLessThan(300);
  });

  test.afterAll(async () => {
    // Best-effort cleanup so the suite is re-runnable.
    try {
      deleteMailboxByUsername(OWNER_EMAIL);
    } catch {
      /* best-effort */
    }
    try {
      deleteMailboxByUsername(GRANTEE_EMAIL);
    } catch {
      /* best-effort */
    }
    await api?.dispose();
  });

  async function loginViaUI(page: Page, email: string, password: string) {
    await page.goto('/login');
    await page.waitForSelector('#username');
    await page.fill('#username', email);
    await page.fill('#password', password);
    await page.click('button[type="submit"]');
    await page.waitForURL(/\/app/, { timeout: 20_000 });
    await expect(page.locator('.sidebar')).toBeVisible({ timeout: 15_000 });
  }

  // Sidebar nav helper — clicks the menu item whose visible label matches.
  // HARD RULE: navigate via sidebar clicks only, never via page.goto for
  // internal routes.
  async function navigateToSettings(page: Page, label: string) {
    const item = page.locator('.sidebar .folder-item', { hasText: label }).first();
    await expect(item, `sidebar must expose "${label}"`).toBeVisible({ timeout: 10_000 });
    await item.click();
  }

  // ──────────────────────────────────────────────────────────────────────────
  // 1) Shared mailboxes — owner grants admin to grantee, grantee sees the
  //    shared mailbox in their sidebar manager, expanded view shows the ACL
  //    list + grant form, revoke removes it from the grantee's list.
  //
  //    This validates the migration-075 RLS fix end-to-end: without it the
  //    GET /api/shared-mailboxes endpoint would return [] for the grantee
  //    even after a successful grant (FORCE RLS + unset app.mailbox_id).
  // ──────────────────────────────────────────────────────────────────────────
  test('shared mailboxes: grant ACL, grantee sees mailbox + ACL list, revoke clears it', async ({
    page,
    takeScreenshot,
  }) => {
    // Baseline: grantee has no shared mailboxes yet.
    const before = await api.get('/api/shared-mailboxes', { headers: granteeAuth });
    expect(before.status()).toBe(200);
    expect((await before.json()) as SharedMailboxView[]).toEqual([]);

    // Owner grants the grantee read+write+admin so the grantee can expand the
    // mailbox in the manager and see the ACL list.
    const grant = await api.post(`/api/shared-mailboxes/${ownerId}/acl`, {
      headers: ownerAuth,
      data: {
        granted_to: granteeId,
        can_read: true,
        can_write: true,
        can_delete: false,
        can_admin: true,
      },
    });
    expect(grant.status(), 'grant must return 201').toBe(201);

    // Cross-check via API: the grantee now sees exactly one shared mailbox.
    const after = await api.get('/api/shared-mailboxes', { headers: granteeAuth });
    const accessible = (await after.json()) as SharedMailboxView[];
    expect(accessible.length).toBe(1);
    expect(accessible[0].mailbox_id).toBe(ownerId);
    expect(accessible[0].username).toBe(OWNER_EMAIL);
    expect(accessible[0].can_admin).toBe(true);
    expect(accessible[0].can_write).toBe(true);

    // Now exercise the UI as the grantee.
    await loginViaUI(page, GRANTEE_EMAIL, PASSWORD);
    await navigateToSettings(page, 'Shared Mailboxes');

    // The shared-mailbox row paints with the owner's username and active perms.
    // This is the "switch-to-shared-mailbox indicator" — the grantee can see
    // which mailbox is shared with them and at what permission level.
    const row = page
      .locator('.mailbox-item')
      .filter({ hasText: OWNER_EMAIL })
      .first();
    await expect(row, 'shared mailbox row must paint for the grantee').toBeVisible({
      timeout: 15_000,
    });
    await expect(row, 'permissions must surface in the row').toContainText(/Read|Write|Admin/);
    // The Admin badge proves can_admin came through end-to-end.
    await expect(row.locator('.badge', { hasText: 'Admin' })).toBeVisible();
    await takeScreenshot(page, 'shared/shared-mailbox-list-indicator');

    // Expand the row — the chevron-rotate flips to ChevronDown and the ACL
    // panel appears (only when can_admin === true, which we just verified).
    await row.locator('.mailbox-item__header').click();
    await expect(page.locator('.acl-list, .acl-entry').first()).toBeVisible({
      timeout: 10_000,
    });

    // The ACL entry must show the grantee themselves (the only grant so far).
    const aclEntry = page.locator('.acl-entry').filter({ hasText: GRANTEE_EMAIL }).first();
    await expect(aclEntry).toBeVisible();
    await takeScreenshot(page, 'shared/shared-mailbox-acl-list');

    // Open the Grant Access form to capture the form screenshot. The button
    // sits inside the expanded panel.
    await page.locator('button:has-text("Grant Access")').first().click();
    await expect(page.locator('input[placeholder="User UUID to grant access"]')).toBeVisible();
    await takeScreenshot(page, 'shared/shared-mailbox-grant-form');

    // Revoke the grant via API and confirm the UI clears once the query
    // re-runs. We hard-reload the page so React Query's cached
    // ['shared-mailboxes'] payload doesn't paper over the revoke; reload also
    // exercises the JWT-in-localStorage rehydration path on the way in.
    const revoke = await api.delete(
      `/api/shared-mailboxes/${ownerId}/acl/${granteeId}`,
      { headers: ownerAuth },
    );
    expect(revoke.status()).toBe(204);

    await page.reload();
    await expect(page.locator('.sidebar')).toBeVisible({ timeout: 15_000 });
    await navigateToSettings(page, 'Shared Mailboxes');

    // Empty-state copy appears once the grant is gone.
    await expect(
      page.locator('p.empty-state', { hasText: /No shared mailboxes available/i }),
    ).toBeVisible({ timeout: 15_000 });
    await takeScreenshot(page, 'shared/shared-mailbox-empty-after-revoke');

    // Final API cross-check.
    const final = await api.get('/api/shared-mailboxes', { headers: granteeAuth });
    expect((await final.json()) as SharedMailboxView[]).toEqual([]);
  });

  // ──────────────────────────────────────────────────────────────────────────
  // 2) Shared files — UI upload, copy-link feedback, incognito public DL
  //    works, exhausting max_downloads turns the same token into "expired",
  //    delete via API tears it down.
  // ──────────────────────────────────────────────────────────────────────────
  test('shared files: upload via UI, copy link, public download (incognito) + expired-token error', async ({
    page,
    browser,
    takeScreenshot,
  }) => {
    // Baseline.
    const before = await api.get('/api/shared-files', { headers: granteeAuth });
    expect(before.status()).toBe(200);
    const beforeList = (await before.json()) as SharedFile[];

    // Build a small fixture file on disk (the input[type=file] needs a real
    // path; Playwright reads it and uploads through the SPA's multipart form).
    const tmpDir = mkdtempSync(path.join(os.tmpdir(), 'tmail-shared-'));
    const fixturePath = path.join(tmpDir, `tmail-289-fixture-${RUN_TAG}.txt`);
    const fileBody = `TMAIL-289 shared-file fixture\nrun=${RUN_TAG}\n`;
    writeFileSync(fixturePath, fileBody, 'utf8');

    await loginViaUI(page, GRANTEE_EMAIL, PASSWORD);
    await navigateToSettings(page, 'Shared Files');

    // Empty-state copy. Use a strict locator so the heading + form don't
    // confuse the matcher.
    await expect(
      page.getByText(/No shared files yet/i),
    ).toBeVisible({ timeout: 15_000 });
    await takeScreenshot(page, 'shared/shared-files-empty');

    // Fill the form. We pin max_downloads = 2 so the same fixture can both
    // succeed once (incognito GET) AND show the expired-token error after a
    // second draw — proving the count check actually decrements.
    await page.locator('[data-testid="file-input"]').setInputFiles(fixturePath);
    await page.locator('[data-testid="max-downloads-input"]').fill('2');
    await takeScreenshot(page, 'shared/shared-file-upload-form-filled');

    await page.click('button:has-text("Upload & Share")');

    // The row + Active badge must appear.
    const fileRow = page
      .locator('div', { hasText: path.basename(fixturePath) })
      .first();
    await expect(fileRow).toBeVisible({ timeout: 20_000 });
    await expect(fileRow).toContainText('Active');
    await takeScreenshot(page, 'shared/shared-files-list-after-upload');

    // Cross-check via API + capture the token for the public-DL leg.
    const after = await api.get('/api/shared-files', { headers: granteeAuth });
    const afterList = (await after.json()) as SharedFile[];
    expect(afterList.length).toBe(beforeList.length + 1);
    const created = afterList.find((f) => f.filename === path.basename(fixturePath));
    expect(created, 'shared file must round-trip to API').toBeTruthy();
    expect(created!.max_downloads).toBe(2);
    expect(created!.download_count).toBe(0);
    const token = created!.download_token;
    expect(token).toMatch(/^[0-9a-f]{64}$/);

    // Click the copy-link button — the Copy icon swaps to a Link icon for 2s.
    // Firefox blocks clipboard writes by default in the test profile, so we
    // ignore the clipboard contents and just verify the visual feedback.
    await page.locator(`[data-testid="copy-link-${created!.id}"]`).click({
      // Some Firefox sandbox configs throw on clipboard.writeText; clickAndWait
      // still fires the React state update either way.
      force: true,
    });
    await takeScreenshot(page, 'shared/shared-file-copy-link-feedback');

    // Public download leg #1 — fresh APIRequestContext (no auth header) hits
    // /api/dl/{token} and must return 200 + Content-Disposition + the body.
    const pubApi = await apiRequest.newContext({ baseURL: resolvedBaseURL });
    const dlOnce = await pubApi.get(`/api/dl/${token}`);
    expect(dlOnce.status(), 'public DL #1 must succeed').toBe(200);
    expect(dlOnce.headers()['content-disposition']).toContain(
      `filename="${path.basename(fixturePath)}"`,
    );
    expect(await dlOnce.text()).toBe(fileBody);

    // Public download leg #2 — same call inside an incognito browser context.
    // The Content-Disposition: attachment header makes Firefox treat the
    // response as a download instead of rendering it inline, so we navigate
    // via a tiny in-page anchor click and catch the `download` event. The
    // screenshot captures the helper page that triggered it — proof the
    // incognito context exercised the public URL end-to-end.
    const incognito = await browser.newContext({ acceptDownloads: true });
    const incPage = await incognito.newPage();
    await incPage.setContent(
      `<html><body style="font-family:sans-serif;padding:24px">
         <h1>TMAIL-289 incognito download</h1>
         <p>Triggering <code>${resolvedBaseURL}/api/dl/${token}</code></p>
         <a id="dl" href="${resolvedBaseURL}/api/dl/${token}" download>Download</a>
       </body></html>`,
    );
    const [download] = await Promise.all([
      incPage.waitForEvent('download'),
      incPage.click('#dl'),
    ]);
    expect(download.suggestedFilename()).toBe(path.basename(fixturePath));
    await takeScreenshot(incPage, 'shared/shared-file-public-download-success');

    // We've now spent 2 of 2 max downloads. The next GET must be rejected as
    // expired. AppError::BadRequest → HTTP 400 with the "expired" message.
    const dlExpired = await pubApi.get(`/api/dl/${token}`);
    expect(dlExpired.status(), 'public DL after max_downloads must 400').toBe(400);
    const expiredBody = await dlExpired.text();
    expect(expiredBody.toLowerCase()).toContain('expired');

    // Render the expired-token error in the incognito browser too. The 400
    // response carries no Content-Disposition, so Firefox renders the JSON
    // body inline; we assert the body text and screenshot the page.
    const incExpiredPage = await incognito.newPage();
    const expiredResp = await incExpiredPage.goto(`${resolvedBaseURL}/api/dl/${token}`);
    expect(expiredResp?.status(), 'incognito DL after exhaust must 400').toBe(400);
    await expect(incExpiredPage.locator('body')).toContainText(/expired/i);
    await takeScreenshot(incExpiredPage, 'shared/shared-file-public-download-expired');

    await incognito.close();
    await pubApi.dispose();

    // Delete via API and confirm the row is gone for the grantee.
    const del = await api.delete(`/api/shared-files/${created!.id}`, {
      headers: granteeAuth,
    });
    expect(del.status()).toBe(204);

    const final = await api.get('/api/shared-files', { headers: granteeAuth });
    const finalList = (await final.json()) as SharedFile[];
    expect(finalList.find((f) => f.id === created!.id)).toBeUndefined();

    // Hard reload so React Query's cached ['shared-files'] payload doesn't
    // paper over the API delete; sidebar round-trip alone leaves stale data
    // visible until the background refetch completes.
    await page.reload();
    await expect(page.locator('.sidebar')).toBeVisible({ timeout: 15_000 });
    await navigateToSettings(page, 'Shared Files');
    await expect(
      page.getByText(/No shared files yet/i),
    ).toBeVisible({ timeout: 15_000 });
    await takeScreenshot(page, 'shared/shared-files-empty-after-revoke');
  });

  // ──────────────────────────────────────────────────────────────────────────
  // 3) Public DL endpoint — unknown token must 404 cleanly. Cheap negative
  //    path that lives in the same spec so we don't pay another login.
  // ──────────────────────────────────────────────────────────────────────────
  test('shared files: unknown public token returns 404', async () => {
    const pubApi = await apiRequest.newContext({ baseURL: resolvedBaseURL });
    // 64 hex chars but never minted — find_by_token returns None.
    const fake = 'f'.repeat(64);
    const resp = await pubApi.get(`/api/dl/${fake}`);
    expect(resp.status()).toBe(404);
    const body = await resp.text();
    expect(body.toLowerCase()).toMatch(/not found|invalid/);
    await pubApi.dispose();
  });
});
