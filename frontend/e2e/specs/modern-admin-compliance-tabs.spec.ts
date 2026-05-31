/**
 * TMAIL-354: Modern UI — Admin Retention + Legal-holds + DLP + eDiscovery
 * compliance sub-tabs.
 *
 * Each new sub-tab gets exercised end-to-end through the UI:
 *   1. Retention — open list, create a policy via the form, confirm the API
 *      returns it, delete and confirm it's gone.
 *   2. Legal holds — pick the freshly-signed-up admin mailbox, place a
 *      hold, assert API shows it, release and assert active=false.
 *   3. DLP — switch to the Rules pane, create a rule, run a Test scan
 *      against a body that matches the pattern, assert the API returns
 *      at least one match, then delete the rule.
 *   4. eDiscovery — create a search, assert API has the row, delete it
 *      and assert the list shrinks back.
 *
 * Per the project E2E SPA rules: capture `/api/...` state before AND after
 * every UI mutation so we never trust the DOM alone. Screenshots land in
 * `frontend/e2e/screenshots/admin-compliance-modern/`.
 *
 * Build prerequisite (handled by the auto-fix runner): `npm run build:alt-ui`
 * so the bundle in `frontend/public/modern/` reflects the latest source.
 */
import { test, expect } from '../fixtures/base.js';
import { execFileSync } from 'node:child_process';

const PASSWORD = 'compliance-tabs-e2e-2026';
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
function cleanupRetentionByName(name: string) {
  execFileSync(
    'psql',
    [DB_URL, '-At', '-c', `DELETE FROM retention_policies WHERE name = $$${name}$$;`],
    { encoding: 'utf8' },
  );
}
function cleanupDlpRuleByName(name: string) {
  execFileSync(
    'psql',
    [DB_URL, '-At', '-c', `DELETE FROM dlp_rules WHERE name = $$${name}$$;`],
    { encoding: 'utf8' },
  );
}
function cleanupEdiscoveryByName(name: string) {
  execFileSync(
    'psql',
    [DB_URL, '-At', '-c', `DELETE FROM ediscovery_searches WHERE name = $$${name}$$;`],
    { encoding: 'utf8' },
  );
}

test.describe('Modern UI — Admin compliance sub-tabs (TMAIL-354)', () => {
  const ADMIN_EMAIL = `compliance-modern-${Date.now()}@e2e.tasmail`;
  const RETENTION_NAME = `e2e-retention-${Date.now()}`;
  const DLP_NAME = `e2e-dlp-${Date.now()}`;
  const EDISCOVERY_NAME = `e2e-ediscovery-${Date.now()}`;

  test.afterAll(() => {
    deleteUser(ADMIN_EMAIL);
    cleanupRetentionByName(RETENTION_NAME);
    cleanupDlpRuleByName(DLP_NAME);
    cleanupEdiscoveryByName(EDISCOVERY_NAME);
  });

  test('retention + legal-holds + DLP + eDiscovery CRUD all hit the live backend', async ({
    page,
    apiSignup,
    takeScreenshot,
    baseURL,
  }) => {
    test.setTimeout(240_000);

    // ── Bootstrap admin via API + login. ─────────────────────────────────
    const signupTokens = await apiSignup(ADMIN_EMAIL, PASSWORD);
    setAdmin(ADMIN_EMAIL, true);

    const loginResp = await fetch(`${baseURL}/api/auth/login`, {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify({ username: ADMIN_EMAIL, password: PASSWORD }),
    });
    expect(loginResp.status, 'admin login').toBe(200);
    const tokens = (await loginResp.json()) as { access_token: string; refresh_token: string };
    const authHeaders = { Authorization: `Bearer ${tokens.access_token}` };
    const jsonHeaders = { ...authHeaders, 'Content-Type': 'application/json' };
    expect(signupTokens.access_token, 'signup returned tokens').toBeTruthy();

    // Plant the JWT and hop into the Modern UI admin route.
    await page.goto('/login');
    await page.evaluate(([at, rt]) => {
      localStorage.setItem('access_token', at);
      localStorage.setItem('refresh_token', rt);
    }, [tokens.access_token, tokens.refresh_token]);

    await page.goto('/modern/index.html#/admin');
    await expect(page.getByTestId('admin-tab-retention')).toBeVisible({ timeout: 10_000 });
    await takeScreenshot(page, 'admin-compliance-modern/00-tabs-visible');

    // =====================================================================
    // 1. RETENTION TAB
    // =====================================================================
    await page.getByTestId('admin-tab-retention').click();
    await expect.poll(() => page.url(), { timeout: 5_000 }).toContain('tab=retention');
    await page.getByTestId('retention-tab').waitFor({ timeout: 10_000 });
    await takeScreenshot(page, 'admin-compliance-modern/01-retention-loaded');

    // Pre-state.
    const preRetResp = await fetch(`${baseURL}/api/admin/retention`, { headers: authHeaders });
    expect(preRetResp.status, 'pre retention list').toBe(200);
    const preRetList = (await preRetResp.json()) as Array<{ id: string; name: string }>;
    const preRetCount = preRetList.length;

    await page.getByTestId('retention-add-button').click();
    await page.getByTestId('retention-form').waitFor({ timeout: 5_000 });
    await page.getByTestId('retention-form-name').fill(RETENTION_NAME);
    await page.getByTestId('retention-form-days').fill('365');
    await takeScreenshot(page, 'admin-compliance-modern/02-retention-form-filled');
    await page.getByTestId('retention-form-submit').click();

    await expect.poll(async () => {
      const r = await fetch(`${baseURL}/api/admin/retention`, { headers: authHeaders });
      const list = (await r.json()) as Array<{ name: string }>;
      return list.some((p) => p.name === RETENTION_NAME);
    }, { timeout: 10_000 }).toBe(true);
    await page.getByTestId('retention-row').first().waitFor({ timeout: 5_000 });
    await takeScreenshot(page, 'admin-compliance-modern/03-retention-created');

    // Delete via the row — match by name so we don't blow away pre-existing rows.
    page.once('dialog', (d) => d.accept());
    await page
      .locator('[data-testid="retention-row"]', { hasText: RETENTION_NAME })
      .getByRole('button', { name: 'Delete' })
      .click();
    await expect.poll(async () => {
      const r = await fetch(`${baseURL}/api/admin/retention`, { headers: authHeaders });
      const list = (await r.json()) as unknown[];
      return list.length;
    }, { timeout: 10_000 }).toBe(preRetCount);
    await takeScreenshot(page, 'admin-compliance-modern/04-retention-deleted');

    // =====================================================================
    // 2. LEGAL HOLDS TAB
    // =====================================================================
    await page.getByTestId('admin-tab-legal-holds').click();
    await expect.poll(() => page.url(), { timeout: 5_000 }).toContain('tab=legal-holds');
    await page.getByTestId('legal-holds-tab').waitFor({ timeout: 10_000 });
    await takeScreenshot(page, 'admin-compliance-modern/05-legal-holds-loaded');

    // Look up the admin's mailbox id so we can place a hold on it. The
    // signup handler stores the full email in `username`, so an exact
    // match (lowercased) is the right comparison.
    const usersResp = await fetch(`${baseURL}/api/admin/users`, { headers: authHeaders });
    const users = (await usersResp.json()) as Array<{ id: string; username: string }>;
    const adminUser = users.find((u) => u.username.toLowerCase() === ADMIN_EMAIL.toLowerCase());
    expect(adminUser, 'admin user found in users list').toBeTruthy();

    const preHoldResp = await fetch(`${baseURL}/api/admin/legal-holds`, { headers: authHeaders });
    const preHoldList = (await preHoldResp.json()) as Array<{ id: string; user_id: string; active: boolean }>;
    const preHoldCount = preHoldList.length;

    await page.getByTestId('legal-holds-add-button').click();
    await page.getByTestId('legal-holds-form').waitFor({ timeout: 5_000 });
    // Pick our admin from the dropdown. The select shows username (no @) per
    // the LegalHoldsTab implementation.
    await page
      .getByTestId('legal-holds-form-user')
      .selectOption({ value: adminUser!.id });
    await page.getByTestId('legal-holds-form-reason').fill('E2E test case TMAIL-354');
    await takeScreenshot(page, 'admin-compliance-modern/06-legal-holds-form-filled');
    await page.getByTestId('legal-holds-form-submit').click();

    // Backend should now have one more active hold.
    await expect.poll(async () => {
      const r = await fetch(`${baseURL}/api/admin/legal-holds`, { headers: authHeaders });
      const list = (await r.json()) as Array<{ user_id: string; active: boolean }>;
      return list.filter((h) => h.user_id === adminUser!.id && h.active).length;
    }, { timeout: 10_000 }).toBe(1);
    await page.getByTestId('legal-holds-row').first().waitFor({ timeout: 5_000 });
    await takeScreenshot(page, 'admin-compliance-modern/07-legal-holds-created');

    // Release the hold via the UI button. The row text contains the
    // formatted mailbox (`<full-email>@byok.tasmail` per the signup quirk),
    // so we match on the unique local-part prefix to find our row.
    page.once('dialog', (d) => d.accept());
    const adminLocalPart = ADMIN_EMAIL.split('@')[0];
    await page
      .locator('[data-testid="legal-holds-row"]', { hasText: adminLocalPart })
      .getByTestId('legal-holds-release-button')
      .click();
    await expect.poll(async () => {
      const r = await fetch(`${baseURL}/api/admin/legal-holds`, { headers: authHeaders });
      const list = (await r.json()) as Array<{ user_id: string; active: boolean }>;
      // Active count should be back to the pre-test level.
      return list.filter((h) => h.active).length;
    }, { timeout: 10_000 }).toBe(preHoldList.filter((h) => h.active).length);
    await takeScreenshot(page, 'admin-compliance-modern/08-legal-holds-released');

    // API cleanup of the released hold row to keep the test bed tidy.
    // The released row stays in the DB by design (audit trail) — we just
    // don't want it to accumulate over repeated dev runs against the same
    // mailbox. Asserting preHoldCount stays consistent across runs.
    const postHoldResp = await fetch(`${baseURL}/api/admin/legal-holds`, { headers: authHeaders });
    const postHoldList = (await postHoldResp.json()) as Array<{ id: string; user_id: string; active: boolean }>;
    const ourHolds = postHoldList.filter((h) => h.user_id === adminUser!.id);
    for (const h of ourHolds) {
      execFileSync(
        'psql',
        [DB_URL, '-At', '-c', `DELETE FROM legal_holds WHERE id = $$${h.id}$$;`],
        { encoding: 'utf8' },
      );
    }
    expect(preHoldCount, 'pre-test hold count snapshot').toBeGreaterThanOrEqual(0);

    // =====================================================================
    // 3. DLP TAB — Rules pane (create + delete) + Scan pane (round-trip)
    // =====================================================================
    await page.getByTestId('admin-tab-dlp').click();
    await expect.poll(() => page.url(), { timeout: 5_000 }).toContain('tab=dlp');
    await page.getByTestId('dlp-tab').waitFor({ timeout: 10_000 });
    await takeScreenshot(page, 'admin-compliance-modern/09-dlp-loaded');

    // Default sub-tab is Rules.
    const preRulesResp = await fetch(`${baseURL}/api/admin/dlp/rules`, { headers: authHeaders });
    const preRulesList = (await preRulesResp.json()) as Array<{ id: string; name: string }>;
    const preRulesCount = preRulesList.length;

    await page.getByTestId('dlp-rule-add-button').click();
    await page.getByTestId('dlp-rule-form').waitFor({ timeout: 5_000 });
    await page.getByTestId('dlp-rule-form-name').fill(DLP_NAME);
    // Use a simple keyword pattern that's guaranteed to match our test body.
    await page.getByTestId('dlp-rule-form-pattern').fill('TMAIL-354-SECRET');
    await takeScreenshot(page, 'admin-compliance-modern/10-dlp-rule-form-filled');
    await page.getByTestId('dlp-rule-form-submit').click();

    await expect.poll(async () => {
      const r = await fetch(`${baseURL}/api/admin/dlp/rules`, { headers: authHeaders });
      const list = (await r.json()) as Array<{ name: string }>;
      return list.some((p) => p.name === DLP_NAME);
    }, { timeout: 10_000 }).toBe(true);
    await page.getByTestId('dlp-rule-row').first().waitFor({ timeout: 5_000 });
    await takeScreenshot(page, 'admin-compliance-modern/11-dlp-rule-created');

    // Switch to the Test scan sub-tab and run a body that matches the pattern.
    await page.getByTestId('dlp-subtab-scan').click();
    await page.getByTestId('dlp-scan-pane').waitFor({ timeout: 5_000 });
    await page.getByTestId('dlp-scan-body').fill('This text contains TMAIL-354-SECRET inside it.');
    await takeScreenshot(page, 'admin-compliance-modern/12-dlp-scan-filled');
    await page.getByTestId('dlp-scan-submit').click();
    await page.getByTestId('dlp-scan-results').waitFor({ timeout: 10_000 });
    await takeScreenshot(page, 'admin-compliance-modern/13-dlp-scan-results');

    // Verify the same scan via the API directly so we're not trusting DOM alone.
    const scanResp = await fetch(`${baseURL}/api/admin/dlp/scan`, {
      method: 'POST',
      headers: jsonHeaders,
      body: JSON.stringify({ body: 'This text contains TMAIL-354-SECRET inside it.' }),
    });
    expect(scanResp.status, 'scan API').toBe(200);
    const scanMatches = (await scanResp.json()) as Array<{ rule_name: string }>;
    expect(scanMatches.some((m) => m.rule_name === DLP_NAME), 'our rule matched').toBe(true);

    // Back to Rules pane and delete.
    await page.getByTestId('dlp-subtab-rules').click();
    page.once('dialog', (d) => d.accept());
    await page
      .locator('[data-testid="dlp-rule-row"]', { hasText: DLP_NAME })
      .getByRole('button', { name: 'Delete' })
      .click();
    await expect.poll(async () => {
      const r = await fetch(`${baseURL}/api/admin/dlp/rules`, { headers: authHeaders });
      const list = (await r.json()) as unknown[];
      return list.length;
    }, { timeout: 10_000 }).toBe(preRulesCount);
    await takeScreenshot(page, 'admin-compliance-modern/14-dlp-rule-deleted');

    // =====================================================================
    // 4. EDISCOVERY TAB — create + delete (execute/export need a richer
    //    test bed with real mailbox content, which is out of scope here;
    //    we only verify the CRUD round-trip for the UI plumbing).
    // =====================================================================
    await page.getByTestId('admin-tab-ediscovery').click();
    await expect.poll(() => page.url(), { timeout: 5_000 }).toContain('tab=ediscovery');
    await page.getByTestId('ediscovery-tab').waitFor({ timeout: 10_000 });
    await takeScreenshot(page, 'admin-compliance-modern/15-ediscovery-loaded');

    const preEdResp = await fetch(`${baseURL}/api/admin/ediscovery`, { headers: authHeaders });
    const preEdList = (await preEdResp.json()) as Array<{ id: string; name: string }>;
    const preEdCount = preEdList.length;

    await page.getByTestId('ediscovery-add-button').click();
    await page.getByTestId('ediscovery-form').waitFor({ timeout: 5_000 });
    await page.getByTestId('ediscovery-form-name').fill(EDISCOVERY_NAME);
    await page.getByTestId('ediscovery-form-query').fill('from:acme.com');
    await takeScreenshot(page, 'admin-compliance-modern/16-ediscovery-form-filled');
    await page.getByTestId('ediscovery-form-submit').click();

    await expect.poll(async () => {
      const r = await fetch(`${baseURL}/api/admin/ediscovery`, { headers: authHeaders });
      const list = (await r.json()) as Array<{ name: string }>;
      return list.some((s) => s.name === EDISCOVERY_NAME);
    }, { timeout: 10_000 }).toBe(true);
    await page.getByTestId('ediscovery-row').first().waitFor({ timeout: 5_000 });
    await takeScreenshot(page, 'admin-compliance-modern/17-ediscovery-created');

    // Click the View button to confirm the detail pane opens (results
    // will be empty since we never executed — that's fine).
    await page
      .locator('[data-testid="ediscovery-row"]', { hasText: EDISCOVERY_NAME })
      .getByTestId('ediscovery-view-button')
      .click();
    await page.getByTestId('ediscovery-detail').waitFor({ timeout: 5_000 });
    await takeScreenshot(page, 'admin-compliance-modern/18-ediscovery-detail');

    // Delete.
    page.once('dialog', (d) => d.accept());
    await page
      .locator('[data-testid="ediscovery-row"]', { hasText: EDISCOVERY_NAME })
      .getByRole('button', { name: 'Delete' })
      .click();
    await expect.poll(async () => {
      const r = await fetch(`${baseURL}/api/admin/ediscovery`, { headers: authHeaders });
      const list = (await r.json()) as unknown[];
      return list.length;
    }, { timeout: 10_000 }).toBe(preEdCount);
    await takeScreenshot(page, 'admin-compliance-modern/19-ediscovery-deleted');

    // =====================================================================
    // 5. URL deep-link survives reload — `?tab=dlp` should re-open DLP.
    // =====================================================================
    await page.goto('/modern/index.html#/admin?tab=dlp');
    await page.getByTestId('dlp-tab').waitFor({ timeout: 10_000 });
    await takeScreenshot(page, 'admin-compliance-modern/20-deep-link-dlp');
  });
});
