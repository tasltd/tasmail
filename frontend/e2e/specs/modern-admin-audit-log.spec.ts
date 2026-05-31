/**
 * TMAIL-352: Modern UI — Admin Audit log viewer.
 *
 * Promotes a fresh signup to admin, hops to `/modern/index.html#/admin?tab=audit-log`,
 * asserts the new tab renders, applies the action-prefix + date-range filters, and
 * confirms the request actually hits `/api/admin/audit-log` with the expected
 * query params.
 *
 * Pre-existing classic-SPA coverage lives in `admin-audit-log-flow.spec.ts`
 * (TMAIL-198). This spec covers the Modern UI surface and adds explicit
 * pagination/total-count assertions that the classic viewer never had.
 *
 * Screenshots: frontend/e2e/screenshots/admin-audit-modern/
 */
import { test, expect } from '../fixtures/base.js';
import { execFileSync } from 'node:child_process';

const PASSWORD = 'audit-modern-e2e-2026';
const DB_URL = process.env.TASMAIL_DB_URL
  ?? 'postgresql://alleina:alleina_dev_2026@127.0.0.1:5432/tasmail';

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

test.describe('Modern UI — Admin Audit log (TMAIL-352)', () => {
  const ADMIN_EMAIL = `audit-modern-${Date.now()}@e2e.tasmail`;
  test.afterAll(() => deleteUser(ADMIN_EMAIL));

  test('admin tab renders, filters by action prefix, paginates, and surfaces X-Total-Count', async ({
    page,
    apiSignup,
    takeScreenshot,
    baseURL,
  }) => {
    test.setTimeout(90_000);

    // ── 1. Bootstrap admin via API + login. ─────────────────────────────
    await apiSignup(ADMIN_EMAIL, PASSWORD);
    setAdmin(ADMIN_EMAIL, true);

    // Re-login so the JWT carries is_admin=true (the signup token was
    // minted before the UPDATE).
    const loginResp = await fetch(`${baseURL}/api/auth/login`, {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify({ username: ADMIN_EMAIL, password: PASSWORD }),
    });
    expect(loginResp.status, 'admin login').toBe(200);
    const tokens = (await loginResp.json()) as { access_token: string; refresh_token: string };

    // The login flow itself records `auth.login` audit rows we'll use
    // below to assert the filter actually returns matching results.

    // ── 2. Plant the JWT and visit the Modern UI admin route directly. ─
    // The Modern UI is at /modern/index.html and uses HashRouter, so the
    // initial nav needs the file path with a hash deep-link.
    await page.goto('/login');
    await page.evaluate(([at, rt]) => {
      localStorage.setItem('access_token', at);
      localStorage.setItem('refresh_token', rt);
    }, [tokens.access_token, tokens.refresh_token]);

    await page.goto('/modern/index.html#/admin');
    // Tabs shell should render with the Overview tab active by default.
    const overviewTab = page.getByTestId('admin-tab-overview');
    const auditTab = page.getByTestId('admin-tab-audit-log');
    await expect(overviewTab).toBeVisible({ timeout: 10_000 });
    await expect(auditTab).toBeVisible();
    await takeScreenshot(page, 'admin-audit-modern/01-overview-default');

    // ── 3. Capture pre-action API state: count rows before clicking. ───
    const auditUrl = `${baseURL}/api/admin/audit-log?limit=50&action=auth.`;
    const preResp = await fetch(auditUrl, {
      headers: { Authorization: `Bearer ${tokens.access_token}` },
    });
    expect(preResp.status, 'pre-fetch audit-log').toBe(200);
    const preTotal = parseInt(preResp.headers.get('X-Total-Count') ?? '0', 10);
    expect(preTotal, 'X-Total-Count header present').toBeGreaterThan(0);

    // ── 4. Switch to Audit log tab. ────────────────────────────────────
    let lastAuditRequest = '';
    page.on('request', (req) => {
      if (req.url().includes('/api/admin/audit-log')) {
        lastAuditRequest = req.url();
      }
    });
    await auditTab.click();
    // URL syncs to ?tab=audit-log.
    await expect.poll(() => page.url(), { timeout: 5_000 }).toContain('tab=audit-log');
    await expect(page.getByTestId('audit-log-tab')).toBeVisible();

    // Wait for the initial fetch to land and render the table.
    await expect.poll(() => lastAuditRequest, { timeout: 10_000 }).toContain('/api/admin/audit-log');
    await page.getByTestId('audit-table').waitFor({ timeout: 10_000 });
    await takeScreenshot(page, 'admin-audit-modern/02-tab-loaded');

    // Pagination status should mention "Showing 1–N of TOTAL".
    await expect(page.getByTestId('audit-pagination-status')).toContainText(/Showing 1.\d+ of \d+/);

    // ── 5. Apply the auth.* prefix filter and confirm the request shape. ─
    await page.getByTestId('audit-filter-prefix').selectOption('auth.');
    await expect.poll(() => lastAuditRequest, { timeout: 8_000 }).toContain('action=auth.');
    // Page should reset to 0 — page label says "Page 1 of ...".
    await expect(page.getByTestId('audit-page-label')).toContainText(/Page 1 of \d+/);
    await takeScreenshot(page, 'admin-audit-modern/03-auth-prefix-filter');

    // Every visible row's Action column should start with auth.
    const actionCells = page.locator('[data-testid="audit-row"] td:nth-child(2)');
    const actionCount = await actionCells.count();
    expect(actionCount, 'rows present after filter').toBeGreaterThan(0);
    for (let i = 0; i < Math.min(actionCount, 5); i++) {
      await expect(actionCells.nth(i)).toContainText(/^auth\./);
    }

    // ── 6. Date-range filter — from = now, expect zero rows. ───────────
    // (Audit log rows were inserted before this `now`, so >= now matches
    // nothing.) Captures the wired-up datetime-local → ISO conversion.
    const future = new Date(Date.now() + 60_000).toISOString().slice(0, 16);
    await page.getByTestId('audit-filter-from').fill(future);
    await expect.poll(() => lastAuditRequest, { timeout: 8_000 }).toMatch(/from=.+/);
    await page.getByTestId('audit-empty').waitFor({ timeout: 10_000 });
    await takeScreenshot(page, 'admin-audit-modern/04-future-from-empty');

    // ── 7. Clear date filter, then test the action override input. ─────
    await page.getByTestId('audit-filter-from').fill('');
    await page.getByTestId('audit-filter-prefix').selectOption('');
    await page.getByTestId('audit-filter-action').fill('auth.login');
    await expect.poll(() => lastAuditRequest, { timeout: 8_000 }).toContain('action=auth.login');
    await page.getByTestId('audit-table').waitFor({ timeout: 10_000 });
    await takeScreenshot(page, 'admin-audit-modern/05-action-exact-match');

    // Confirm the resulting rows are all literally `auth.login`.
    const exactActionCells = page.locator('[data-testid="audit-row"] td:nth-child(2)');
    const exactCount = await exactActionCells.count();
    expect(exactCount).toBeGreaterThan(0);
    for (let i = 0; i < Math.min(exactCount, 5); i++) {
      await expect(exactActionCells.nth(i)).toHaveText('auth.login');
    }

    // ── 8. Verify API state matches what the UI shows. ─────────────────
    const postResp = await fetch(`${baseURL}/api/admin/audit-log?action=auth.login&limit=50`, {
      headers: { Authorization: `Bearer ${tokens.access_token}` },
    });
    expect(postResp.status).toBe(200);
    const apiTotal = parseInt(postResp.headers.get('X-Total-Count') ?? '0', 10);
    // The UI status line says "Showing 1-N of TOTAL" — pull out TOTAL and
    // compare to the API's X-Total-Count.
    const statusText = (await page.getByTestId('audit-pagination-status').textContent()) ?? '';
    const m = statusText.match(/of (\d+)/);
    expect(m, `pagination status: ${statusText}`).toBeTruthy();
    const uiTotal = parseInt(m![1], 10);
    expect(uiTotal, 'UI total matches API X-Total-Count').toBe(apiTotal);

    // ── 9. Drop back to Overview tab — assert the existing Mailboxes
    //         panel still renders so we know we didn't regress the
    //         pre-TMAIL-352 admin page. ──────────────────────────────────
    await overviewTab.click();
    await expect.poll(() => page.url(), { timeout: 5_000 }).not.toContain('tab=audit-log');
    await expect(page.getByRole('heading', { name: 'Mailboxes' })).toBeVisible();
    await takeScreenshot(page, 'admin-audit-modern/06-back-to-overview');
  });
});
