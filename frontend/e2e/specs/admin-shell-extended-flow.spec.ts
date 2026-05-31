/**
 * TMAIL-400: extended AdminShell walk for the 18 admin managers that were
 * previously sitting in every user's sidebar via viewMode.
 *
 * Acceptance criteria from the ticket:
 *   1. Non-admin user: no Admin entry in the mailbox sidebar AND a role-gate
 *      page on direct /admin/* navigation.
 *   2. Admin user: Admin entry visible AND every one of the 18 new admin
 *      categories reachable via menu clicks (no direct page.goto into
 *      /admin/<slug> per the E2E menu-click HARD RULE).
 *
 * The 8 pre-400 admin managers stay covered by admin-shell-flow.spec.ts.
 * Split into a separate file so neither suite balloons past the ~8-test
 * batch budget.
 *
 * Screenshots: e2e/screenshots/admin-shell-extended/<step>.png — required
 * per the E2E Screenshots HARD RULE so visual regressions are caught.
 */
import { test, expect } from '../fixtures/base.js';
import { execFileSync } from 'node:child_process';

const PASSWORD = 'admin-ext-e2e-2026';
const DB_URL = process.env.TASMAIL_DB_URL ?? 'postgresql://alleina:alleina_dev_2026@127.0.0.1:5432/tasmail';

// The 18 managers introduced into the admin shell by TMAIL-400.
// `label` is what the rail renders; `slug` is the path the NavLink targets.
// Used by the menu-click walk below.
const NEW_ADMIN_MANAGERS: Array<{ slug: string; label: string }> = [
  { slug: 'branding', label: 'Branding' },
  { slug: 'hostnames', label: 'Hostnames' },
  { slug: 'bulk-import', label: 'Bulk import' },
  { slug: 'saml', label: 'SAML' },
  { slug: 'oidc', label: 'OIDC' },
  { slug: 'ldap', label: 'LDAP' },
  { slug: 'dlp', label: 'DLP' },
  { slug: 'ediscovery', label: 'eDiscovery' },
  { slug: 'dane', label: 'DANE' },
  { slug: 'retention', label: 'Retention' },
  { slug: 'archive', label: 'Archive' },
  { slug: 'deliverability', label: 'Deliverability' },
  { slug: 'activesync', label: 'ActiveSync' },
  { slug: 'shared-mailboxes', label: 'Shared mailboxes' },
  { slug: 'plugins', label: 'Plugins' },
  { slug: 'webhooks', label: 'Webhooks' },
  { slug: 'chat-integrations', label: 'Chat integrations' },
  { slug: 'billing', label: 'Billing' },
];

function setAdmin(email: string, isAdmin: boolean) {
  execFileSync(
    'psql',
    [DB_URL, '-At', '-c', `UPDATE mailboxes SET is_admin = ${isAdmin} WHERE username = $$${email}$$;`],
    { encoding: 'utf8' },
  );
}

function deleteUser(email: string) {
  execFileSync(
    'psql',
    [DB_URL, '-At', '-c', `DELETE FROM mailboxes WHERE username = $$${email}$$;`],
    { encoding: 'utf8' },
  );
}

test.describe('AdminShell extended (TMAIL-400) — admin gating + 18 new managers', () => {
  const NON_ADMIN_EMAIL = `admin-ext-na-${Date.now()}@e2e.tasmail`;
  const ADMIN_EMAIL = `admin-ext-yes-${Date.now()}@e2e.tasmail`;

  test.afterAll(() => {
    deleteUser(NON_ADMIN_EMAIL);
    deleteUser(ADMIN_EMAIL);
  });

  test('non-admin user has no Admin entry in the mailbox sidebar', async ({
    page,
    apiSignup,
    takeScreenshot,
  }) => {
    test.setTimeout(60_000);
    const tokens = await apiSignup(NON_ADMIN_EMAIL, PASSWORD);
    await page.goto('/login');
    await page.evaluate(([at, rt]) => {
      localStorage.setItem('access_token', at);
      localStorage.setItem('refresh_token', rt);
    }, [tokens.access_token, tokens.refresh_token]);

    // Land in the mailbox so the registry-driven Sidebar renders.
    await page.goto('/app');
    await expect(page.locator('aside.sidebar')).toBeVisible({ timeout: 10_000 });
    await takeScreenshot(page, 'admin-shell-extended/01-non-admin-sidebar');

    // The Admin nav entry MUST NOT render for a non-admin user.
    await expect(page.locator('aside.sidebar [data-nav-key="admin"]')).toHaveCount(0);

    // Direct nav to /admin must show the role-gate (not the shell).
    await page.goto('/admin');
    await expect(page.locator('h1', { hasText: 'Admin only' })).toBeVisible({ timeout: 10_000 });
    await expect(page.locator('.admin-shell__sidebar')).toHaveCount(0);
    await takeScreenshot(page, 'admin-shell-extended/02-non-admin-role-gate');
  });

  test('admin user clicks Admin in the sidebar and walks every new manager', async ({
    page,
    apiSignup,
    takeScreenshot,
  }) => {
    // 60s + ~1.5s per manager click — give it room.
    test.setTimeout(120_000);
    await apiSignup(ADMIN_EMAIL, PASSWORD);
    setAdmin(ADMIN_EMAIL, true);

    // Re-issue a JWT so the is_admin claim is fresh.
    const baseURL = test.info().project.use.baseURL ?? 'https://mail.techatscale.io';
    const loginResp = await fetch(`${baseURL}/api/auth/login`, {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify({ username: ADMIN_EMAIL, password: PASSWORD }),
    });
    expect(loginResp.status, 'admin re-login').toBe(200);
    const adminTokens = (await loginResp.json()) as { access_token: string; refresh_token: string };

    await page.goto('/login');
    await page.evaluate(([at, rt]) => {
      localStorage.setItem('access_token', at);
      localStorage.setItem('refresh_token', rt);
    }, [adminTokens.access_token, adminTokens.refresh_token]);

    await page.goto('/app');
    await expect(page.locator('aside.sidebar')).toBeVisible({ timeout: 10_000 });
    await takeScreenshot(page, 'admin-shell-extended/03-admin-sidebar');

    // Admin entry must exist — and the click is the only navigation we allow
    // per the menu-click HARD RULE.
    const adminEntry = page.locator('aside.sidebar [data-nav-key="admin"]');
    await expect(adminEntry).toHaveCount(1);
    await adminEntry.click();
    await expect(page).toHaveURL(/\/admin\/feature-flags$/, { timeout: 10_000 });
    await expect(page.locator('.admin-shell__sidebar')).toBeVisible({ timeout: 10_000 });

    // Surface the full registry — 26 entries grouped into 7 sections.
    await expect(page.locator('.admin-shell__nav-item')).toHaveCount(26);
    await expect(page.locator('.admin-shell__group')).toHaveCount(7);
    await takeScreenshot(page, 'admin-shell-extended/04-admin-shell-landed');

    // Walk every new manager via menu click. Each click MUST land on the
    // correct /admin/<slug> URL and the right pane must render (we assert
    // the admin shell's <main> region is still mounted — manager-internal
    // assertions live in each manager's own spec).
    for (const { slug, label } of NEW_ADMIN_MANAGERS) {
      const entry = page.locator('.admin-shell__nav-item', { hasText: label });
      await expect(entry, `nav entry for ${label}`).toHaveCount(1);
      await entry.click();
      await expect(page, `URL after clicking ${label}`).toHaveURL(
        new RegExp(`/admin/${slug.replace(/[-/]/g, '\\$&')}$`),
        { timeout: 10_000 },
      );
      // The pane should mount the manager — confirm the content area
      // exists and is non-empty. Manager-specific h1 assertions are out
      // of scope for this walk; some managers don't render an h1.
      await expect(page.locator('.admin-shell__content')).toBeVisible();
      await takeScreenshot(page, `admin-shell-extended/manager-${slug}`);
    }
  });
});
