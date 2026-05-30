/**
 * TMAIL-323: alt-UI ("modern") Settings shell.
 *
 * Coverage:
 *   1. Sign up + BYOK the noreply mailbox so the EmailClient mounts cleanly
 *   2. Hop into /modern/ via the classic SPA's wand button
 *   3. Click the Navbar Settings icon and assert:
 *      - URL hash routes to `/settings/profile` (default-tab redirect)
 *      - The settings shell mounts (`settings-page` + `settings-sidenav`)
 *      - All eight expected tabs are present in the side-nav
 *      - The Profile pane is the initially active one
 *   4. Click each remaining tab (Identities, Signatures, Vacation, Filters,
 *      MFA, Theme, IMAP/SMTP) and assert:
 *      - The URL hash updates to /settings/<slug>
 *      - The corresponding pane (data-testid `<tab>-pane`) renders
 *      - The "Coming soon" placeholder renders inside the pane (proves
 *        the shell is wired end-to-end — when P1 swaps the placeholder for
 *        the real component this assertion changes in the same task)
 *   5. Reload the page on /settings/mfa and assert the MFA pane is still
 *      active (deep-link survives reload — proves the URL is the source of
 *      truth, not in-memory state)
 *
 * Screenshots: frontend/e2e/screenshots/settings/<step>.png
 *
 * Build prerequisite (handled by the auto-fix runner): `npm run build:alt-ui`
 * so the bundle in `frontend/public/modern/` reflects this commit.
 */
import { expect, NOREPLY_CREDS, test } from '../fixtures/base.js';
import { deleteMailboxByUsername } from '../helpers/db-cleanup.js';

const SCREENSHOT_DIR = 'settings';
const PASSWORD = 'tmail-323-settings-2026';

// Mirrors themes/shadcn-prototype/src/features/settings/tabs.ts. Kept here
// (and asserted on) so any drift between the registry and what the user
// actually sees fails this spec loudly.
const EXPECTED_TABS: Array<{ slug: string; label: string; testId: string }> = [
  { slug: 'profile', label: 'Profile', testId: 'settings-tab-profile' },
  { slug: 'identities', label: 'Identities', testId: 'settings-tab-identities' },
  { slug: 'signatures', label: 'Signatures', testId: 'settings-tab-signatures' },
  { slug: 'vacation', label: 'Vacation', testId: 'settings-tab-vacation' },
  { slug: 'filters', label: 'Filters', testId: 'settings-tab-filters' },
  { slug: 'mfa', label: 'MFA', testId: 'settings-tab-mfa' },
  { slug: 'theme', label: 'Theme', testId: 'settings-tab-theme' },
  { slug: 'imap-smtp', label: 'IMAP / SMTP', testId: 'settings-tab-imap-smtp' },
];

test.describe('TMAIL-323 alt-UI Settings shell — Navbar button + side-tab navigation', () => {
  test.beforeAll(() => deleteMailboxByUsername(NOREPLY_CREDS.email));
  test.afterAll(() => deleteMailboxByUsername(NOREPLY_CREDS.email));

  test('Navbar Settings button opens /settings shell with all eight tabs and deep-links survive reload', async ({
    page,
    apiSignup,
    takeScreenshot,
    baseURL,
  }) => {
    test.setTimeout(120_000);

    // ── 1. signup + BYOK so EmailClient mounts cleanly behind the auth gate ──
    const tokens = await apiSignup(NOREPLY_CREDS.email, PASSWORD);
    const auth = {
      Authorization: `Bearer ${tokens.access_token}`,
      'Content-Type': 'application/json',
    };
    const imapResp = await fetch(`${baseURL}/api/imap-configs`, {
      method: 'POST',
      headers: auth,
      body: JSON.stringify({
        name: 'tmail-323-imap',
        host: NOREPLY_CREDS.imap.host,
        port: NOREPLY_CREDS.imap.port,
        username: NOREPLY_CREDS.imap.username,
        password: NOREPLY_CREDS.imap.password,
        encryption: NOREPLY_CREDS.imap.encryption,
        is_default: true,
      }),
    });
    expect(imapResp.status, 'IMAP config create').toBe(201);

    // ── 2. open classic /app and hop to /modern/ ───────────────────────────
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

    // ── 3. click the Navbar Settings icon ──────────────────────────────────
    const settingsLink = page.getByTestId('navbar-settings-link');
    await expect(settingsLink, 'Navbar Settings link is mounted').toBeVisible({
      timeout: 10_000,
    });
    await settingsLink.click();

    // Bare /settings → /settings/profile (default-tab redirect via loader).
    await page.waitForURL(/\/modern\/index\.html#\/settings\/profile/i, {
      timeout: 10_000,
    });
    await expect(page.getByTestId('settings-page')).toBeVisible({
      timeout: 10_000,
    });
    await expect(page.getByTestId('settings-sidenav')).toBeVisible();
    await takeScreenshot(page, `${SCREENSHOT_DIR}/02-settings-opened-profile`);

    // ── 4. assert all eight tabs are present + Profile pane is active ──────
    for (const t of EXPECTED_TABS) {
      await expect(
        page.getByTestId(t.testId),
        `tab "${t.label}" must be in the side-nav`,
      ).toBeVisible();
    }
    await expect(
      page.getByTestId('settings-tab-profile-pane'),
      'Profile pane must be the initial active pane',
    ).toBeVisible();
    await expect(
      page.getByTestId('settings-tab-profile-coming-soon'),
    ).toBeVisible();

    // ── 5. click through every non-default tab ─────────────────────────────
    // Skip 'profile' (already active). For each remaining tab, click it,
    // assert URL + pane + placeholder, then snapshot.
    for (const t of EXPECTED_TABS.slice(1)) {
      await page.getByTestId(t.testId).click();

      // URL hash update (HashRouter writes the path into location.hash).
      await page.waitForURL(
        new RegExp(`/modern/index\\.html#/settings/${t.slug}`, 'i'),
        { timeout: 10_000 },
      );

      // Pane swap.
      await expect(
        page.getByTestId(`${t.testId}-pane`),
        `${t.label} pane must render after clicking its tab`,
      ).toBeVisible({ timeout: 10_000 });
      await expect(
        page.getByTestId(`${t.testId}-coming-soon`),
        `${t.label} pane must render its "Coming soon" placeholder`,
      ).toBeVisible();

      await takeScreenshot(page, `${SCREENSHOT_DIR}/03-tab-${t.slug}`);
    }

    // ── 6. deep-link survives reload (URL is the source of truth) ──────────
    // Already on /settings/imap-smtp after the loop. Reload and assert the
    // same pane is still active — proves we read state from the URL, not
    // from a transient in-memory `useState`.
    await page.reload();
    await page.waitForLoadState('domcontentloaded');
    await expect(
      page.getByTestId('settings-tab-imap-smtp-pane'),
      'IMAP/SMTP pane must survive a full-page reload',
    ).toBeVisible({ timeout: 15_000 });
    await takeScreenshot(page, `${SCREENSHOT_DIR}/04-reload-survives`);
  });
});
