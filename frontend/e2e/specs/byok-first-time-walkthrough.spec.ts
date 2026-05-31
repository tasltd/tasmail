/**
 * TMAIL-402 — BYOK first-time-user walkthrough (Firefox).
 *
 * Locks the post-TMAIL-398/399/400/401 structure with a Playwright spec
 * that exercises the new grouped Sidebar, SettingsHub left-rail, and
 * admin-only gating end-to-end as a fresh BYOK user.
 *
 * Split into three focused test cases so each one can use the IMAP shape
 * it needs without contradicting the others:
 *
 *   1) "grouped sidebar + admin gating + Settings deep-link"
 *      Real noreply BYOK so /api/folders returns a populated tree and
 *      the INBOX row actually renders. Asserts ≤ 8 top-level entries,
 *      INBOX has the dominant `folder-item--primary` class, no Admin
 *      entry, then clicks Settings → /app/settings/... → all 4
 *      category tabs render → clicks Mail → Filters →
 *      /app/settings/mail/filters → FilterManager pane is mounted.
 *
 *   2) "empty-inbox state renders the user's BYOK address"
 *      Fake BYOK host (imap.tmail402.test) so the messages query
 *      drops straight into the empty-INBOX branch and EmptyInboxState
 *      echoes the configured user@host back.
 *
 *   3) "admin elevation flips the Admin sidebar entry on"
 *      Signs up a second fresh user, flips `is_admin = true` in the DB,
 *      re-logs in to refresh the JWT claim, and verifies the Admin
 *      sidebar entry now renders. Skips if psql isn't reachable so the
 *      suite still works on CI runners without a local DB tunnel.
 *
 * Screenshots land under e2e/screenshots/byok-walkthrough/.
 */
import { test, expect, NOREPLY_CREDS } from '../fixtures/base.js';
import {
  deleteMailboxByUsername,
  setMailboxAdmin,
} from '../helpers/db-cleanup.js';

const ACCOUNT_PASSWORD = 'byok-walkthrough-tmail402-2026';
const SCREENSHOT_PREFIX = 'byok-walkthrough';
// Fake BYOK host so the messages query falls into the empty-INBOX branch.
const FAKE_IMAP = {
  host: 'imap.tmail402.test',
  port: 993,
  encryption: 'ssl' as const,
} as const;

// PURPOSE: best-effort PATCH to mark the first-login tour as seen so it
// doesn't sit over the sidebar/empty-inbox surfaces under test. Tolerant
// of 404/405 — pre-TMAIL-401 backends don't yet expose the endpoint, but
// in that world the FirstLoginTour query also bails on the same 404 so
// the tour stays hidden anyway.
async function markTourSeen(baseURL: string, accessToken: string): Promise<void> {
  const resp = await fetch(
    `${baseURL}/api/me/preferences/first-login-tour-seen`,
    {
      method: 'PATCH',
      headers: { Authorization: `Bearer ${accessToken}` },
    },
  );
  const ok = resp.status < 400 || resp.status === 404 || resp.status === 405;
  expect(
    ok,
    `mark first-login tour seen — got ${resp.status} (accepts <400 or 404/405 on pre-TMAIL-401 backends)`,
  ).toBe(true);
}

// PURPOSE: attach a BYOK IMAP config for the freshly-signed-up user.
async function attachImap(
  baseURL: string,
  accessToken: string,
  cfg: { host: string; port: number; username: string; password: string; encryption: 'ssl' | 'starttls' | 'none' },
): Promise<void> {
  const resp = await fetch(`${baseURL}/api/imap-configs`, {
    method: 'POST',
    headers: {
      Authorization: `Bearer ${accessToken}`,
      'Content-Type': 'application/json',
    },
    body: JSON.stringify({
      name: 'TMAIL-402 BYOK walkthrough',
      host: cfg.host,
      port: cfg.port,
      username: cfg.username,
      password: cfg.password,
      encryption: cfg.encryption,
      is_default: true,
    }),
  });
  expect(resp.status, `IMAP config create for ${cfg.host}`).toBe(201);
}

// PURPOSE: inject the JWT pair and visit /app — the only direct goto the
// E2E navigation rule permits (same pattern as first-login-tour.spec.ts).
async function injectAndLand(
  page: import('@playwright/test').Page,
  tokens: { access_token: string; refresh_token: string },
): Promise<void> {
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
}

test.describe('BYOK first-time-user walkthrough (TMAIL-402)', () => {
  // Hold onto every email we create so afterAll can wipe them — the unique
  // runId keeps re-runs idempotent without coordination.
  const runId = Date.now();
  const createdEmails: string[] = [];

  test.afterAll(async () => {
    for (const email of createdEmails) {
      try {
        deleteMailboxByUsername(email);
      } catch {
        // Best-effort cleanup; don't fail teardown.
      }
    }
  });

  test('grouped sidebar + Settings deep-link + non-admin gating', async ({
    page,
    apiSignup,
    takeScreenshot,
    baseURL,
  }) => {
    test.setTimeout(120_000);
    const email = `byok-nav-${runId}@e2e.tasmail`;
    createdEmails.push(email);

    const tokens = await apiSignup(email, ACCOUNT_PASSWORD);
    // Real noreply BYOK so /api/folders returns a populated tree (INBOX in
    // particular must exist for the dominant-row assertion to pass).
    await attachImap(baseURL!, tokens.access_token, {
      host: NOREPLY_CREDS.imap.host,
      port: NOREPLY_CREDS.imap.port,
      username: NOREPLY_CREDS.imap.username,
      password: NOREPLY_CREDS.imap.password,
      encryption: NOREPLY_CREDS.imap.encryption,
    });
    await markTourSeen(baseURL!, tokens.access_token);
    await injectAndLand(page, tokens);
    await takeScreenshot(page, `${SCREENSHOT_PREFIX}/01-app-landing`);

    // ── Sidebar ≤ 8 top-level entries (Compose + per-group nav buttons).
    //    FolderTree items live in their own block and aren't counted as
    //    top-level surfaces — the registry covers the nav groups only.
    const sidebar = page.locator('aside.sidebar');
    await expect(sidebar).toBeVisible({ timeout: 10_000 });

    const composeButtons = sidebar.locator('button.btn--compose');
    const groupNavButtons = sidebar.locator('.sidebar__group .folder-item');
    const topLevelCount =
      (await composeButtons.count()) + (await groupNavButtons.count());
    expect(
      topLevelCount,
      `top-level sidebar entries (Compose + nav groups) — got ${topLevelCount}`,
    ).toBeLessThanOrEqual(8);
    // Non-admin baseline = Compose (1) + Calendar/Contacts/Tasks/Templates (4)
    // + Settings (1) = 6. Floor catches silent removals.
    expect(
      topLevelCount,
      'top-level sidebar entries floor',
    ).toBeGreaterThanOrEqual(5);

    // ── INBOX row uses the dominant treatment (folder-item--primary set
    //    by FolderTree for the INBOX entry — see TMAIL-398).
    const inboxRow = sidebar.locator('button.folder-item--primary', {
      hasText: /INBOX/i,
    });
    await expect(
      inboxRow,
      'INBOX must carry the visually-dominant primary class',
    ).toBeVisible({ timeout: 20_000 });

    // ── Non-admin sidebar must NOT contain the Admin entry.
    const adminEntry = sidebar.locator('[data-nav-key="admin"]');
    await expect(
      adminEntry,
      'non-admin must not see the Admin sidebar entry',
    ).toHaveCount(0);
    await takeScreenshot(page, `${SCREENSHOT_PREFIX}/05-no-admin-entry`);

    // ── Click the Settings entry (cog/gear icon in the sidebar's settings
    //    group). Per the E2E navigation rule we click the menu entry, not
    //    goto('/app/settings').
    const settingsEntry = sidebar.locator('[data-nav-key="settings-hub"]');
    await expect(settingsEntry).toBeVisible();
    await settingsEntry.click();
    await page.waitForURL(/\/app\/settings(\/.*)?$/, { timeout: 10_000 });

    // ── SettingsHub renders all 4 category tabs from settings-hub-registry.
    const settingsHub = page.getByTestId('settings-hub');
    await expect(settingsHub).toBeVisible({ timeout: 10_000 });
    for (const catId of ['account', 'mail', 'connections', 'productivity']) {
      await expect(
        page.getByTestId(`settings-category-${catId}`),
        `category tab "${catId}" must render in SettingsHub left rail`,
      ).toBeVisible();
    }
    await takeScreenshot(page, `${SCREENSHOT_PREFIX}/03-settings-hub`);

    // ── Click "Mail" → "Filters" → /app/settings/mail/filters.
    await page.getByTestId('settings-category-mail').click();
    const filtersSection = page.getByTestId('settings-section-filters');
    await expect(filtersSection).toBeVisible({ timeout: 5_000 });
    await filtersSection.click();
    await page.waitForURL(/\/app\/settings\/mail\/filters$/, { timeout: 10_000 });

    // Pane is keyed by section id — assert it switched so we know the lazy
    // chunk swapped (not just the URL).
    await expect(
      page.getByTestId('settings-hub-pane'),
    ).toHaveAttribute('data-section', 'filters', { timeout: 10_000 });
    await takeScreenshot(page, `${SCREENSHOT_PREFIX}/04-settings-mail-filters`);
  });

  test('empty-inbox state renders the BYOK address', async ({
    page,
    apiSignup,
    takeScreenshot,
    baseURL,
  }) => {
    test.setTimeout(90_000);
    const email = `byok-empty-${runId}@e2e.tasmail`;
    createdEmails.push(email);
    const distinctUsername = `byok-empty-${runId}`;

    const tokens = await apiSignup(email, ACCOUNT_PASSWORD);
    // Deliberately fake host — the message fetch fails fast and
    // MessageList renders EmptyInboxState for the INBOX branch with our
    // configured user@host echoed back.
    await attachImap(baseURL!, tokens.access_token, {
      host: FAKE_IMAP.host,
      port: FAKE_IMAP.port,
      username: distinctUsername,
      password: 'unused-test-password',
      encryption: FAKE_IMAP.encryption,
    });
    await markTourSeen(baseURL!, tokens.access_token);
    await injectAndLand(page, tokens);

    const emptyState = page.getByTestId('empty-inbox-state');
    await expect(emptyState).toBeVisible({ timeout: 25_000 });
    const expectedAddress = `${distinctUsername}@${FAKE_IMAP.host}`;
    await expect(
      page.getByTestId('empty-inbox-state__address'),
      'empty-inbox state must render the BYOK address',
    ).toHaveText(expectedAddress);
    await expect(emptyState).toContainText('Your inbox is empty');
    await takeScreenshot(page, `${SCREENSHOT_PREFIX}/02-empty-inbox`);
  });

  test('admin elevation flips the Admin sidebar entry on', async ({
    page,
    apiSignup,
    loginAs,
    takeScreenshot,
    baseURL,
  }) => {
    test.setTimeout(120_000);
    const email = `byok-admin-${runId}@e2e.tasmail`;
    createdEmails.push(email);

    const tokens = await apiSignup(email, ACCOUNT_PASSWORD);
    // Real noreply BYOK so the sidebar is fully populated and the Admin
    // entry vs no-Admin diff is meaningful.
    await attachImap(baseURL!, tokens.access_token, {
      host: NOREPLY_CREDS.imap.host,
      port: NOREPLY_CREDS.imap.port,
      username: NOREPLY_CREDS.imap.username,
      password: NOREPLY_CREDS.imap.password,
      encryption: NOREPLY_CREDS.imap.encryption,
    });
    await markTourSeen(baseURL!, tokens.access_token);

    // Elevate via psql. Skip the rest if psql isn't reachable (e.g. a CI
    // runner without the local DB tunnel) — the other two tests still
    // prove the non-admin half of the gating.
    let elevated = false;
    try {
      const updated = setMailboxAdmin(email, true);
      elevated = updated > 0;
    } catch (err) {
      // eslint-disable-next-line no-console
      console.warn(
        `[TMAIL-402] admin elevation skipped: ${(err as Error).message}`,
      );
    }
    test.skip(!elevated, 'psql not reachable — skipping admin elevation step');

    // The is_admin claim only refreshes on a fresh JWT, so re-login through
    // the form (loginAs is the only fixture entry point that may visit
    // /login directly — see the navigation rule).
    await loginAs(page, email, ACCOUNT_PASSWORD);

    const sidebar = page.locator('aside.sidebar');
    await expect(sidebar).toBeVisible({ timeout: 10_000 });
    const adminEntryAfter = sidebar.locator('[data-nav-key="admin"]');
    await expect(
      adminEntryAfter,
      'admin user MUST see the Admin sidebar entry after re-login',
    ).toBeVisible({ timeout: 10_000 });
    await takeScreenshot(page, `${SCREENSHOT_PREFIX}/06-admin-entry-visible`);
  });
});
