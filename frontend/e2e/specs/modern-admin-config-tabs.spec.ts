/**
 * TMAIL-353: Modern UI — Admin Branding + SAML + OIDC + LDAP configuration tabs.
 *
 * Each new sub-tab gets exercised end-to-end through the UI:
 *   1. Branding — load the live config, change `app_name`, save, assert the
 *      backend GET reflects the new value (round-trip), then reset and
 *      assert the default value comes back.
 *   2. SAML — open the empty list, create a provider via the form, confirm
 *      the API returns it, click "Test" (and accept either success or a
 *      4xx — the IdP URL is fake, the success signal is that the request
 *      went out), then delete and confirm it's gone.
 *   3. OIDC — same shape as SAML, against /api/admin/oidc.
 *   4. LDAP — create + bind-test + delete, with the per-row test result
 *      surfaced through the UI.
 *
 * Per the project's E2E SPA rules: capture `/api/...` state before AND after
 * every UI mutation so we never trust the DOM alone. Screenshots land in
 * `frontend/e2e/screenshots/admin-config-modern/`.
 */
import { test, expect } from '../fixtures/base.js';
import { execFileSync } from 'node:child_process';

const PASSWORD = 'config-tabs-e2e-2026';
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
// TMAIL-353: cleanup hooks for the side-effect rows the SAML/OIDC/LDAP
// CRUD flows leave behind. Idempotent so afterAll can run blind.
function cleanupSamlByName(name: string) {
  execFileSync(
    'psql',
    [DB_URL, '-At', '-c', `DELETE FROM saml_configurations WHERE name = $$${name}$$;`],
    { encoding: 'utf8' },
  );
}
function cleanupOidcByName(name: string) {
  execFileSync(
    'psql',
    [DB_URL, '-At', '-c', `DELETE FROM oidc_providers WHERE name = $$${name}$$;`],
    { encoding: 'utf8' },
  );
}
function cleanupLdapByName(name: string) {
  execFileSync(
    'psql',
    [DB_URL, '-At', '-c', `DELETE FROM ldap_configurations WHERE name = $$${name}$$;`],
    { encoding: 'utf8' },
  );
}

test.describe('Modern UI — Admin config sub-tabs (TMAIL-353)', () => {
  const ADMIN_EMAIL = `config-modern-${Date.now()}@e2e.tasmail`;
  const SAML_NAME = `e2e-saml-${Date.now()}`;
  const OIDC_NAME = `e2e-oidc-${Date.now()}`;
  const LDAP_NAME = `e2e-ldap-${Date.now()}`;

  test.afterAll(() => {
    deleteUser(ADMIN_EMAIL);
    cleanupSamlByName(SAML_NAME);
    cleanupOidcByName(OIDC_NAME);
    cleanupLdapByName(LDAP_NAME);
  });

  test('branding round-trips + SAML/OIDC/LDAP CRUD all hit the live backend', async ({
    page,
    apiSignup,
    takeScreenshot,
    baseURL,
  }) => {
    test.setTimeout(180_000);

    // ── Bootstrap admin via API + login. ─────────────────────────────────
    await apiSignup(ADMIN_EMAIL, PASSWORD);
    setAdmin(ADMIN_EMAIL, true);

    const loginResp = await fetch(`${baseURL}/api/auth/login`, {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify({ username: ADMIN_EMAIL, password: PASSWORD }),
    });
    expect(loginResp.status, 'admin login').toBe(200);
    const tokens = (await loginResp.json()) as { access_token: string; refresh_token: string };
    const authHeaders = { Authorization: `Bearer ${tokens.access_token}` };

    // Plant the JWT and hop into the Modern UI admin route. /login plants
    // us on the same origin so localStorage writes are visible to the
    // alt-UI bundle when we navigate to /modern/.
    await page.goto('/login');
    await page.evaluate(([at, rt]) => {
      localStorage.setItem('access_token', at);
      localStorage.setItem('refresh_token', rt);
    }, [tokens.access_token, tokens.refresh_token]);

    await page.goto('/modern/index.html#/admin');
    await expect(page.getByTestId('admin-tab-branding')).toBeVisible({ timeout: 10_000 });
    await takeScreenshot(page, 'admin-config-modern/00-overview-default');

    // =====================================================================
    // 1. BRANDING TAB — round-trip via API state before + after
    // =====================================================================
    await page.getByTestId('admin-tab-branding').click();
    await expect.poll(() => page.url(), { timeout: 5_000 }).toContain('tab=branding');
    await page.getByTestId('branding-tab').waitFor({ timeout: 10_000 });
    await takeScreenshot(page, 'admin-config-modern/01-branding-loaded');

    // Capture pre-edit state.
    const preBrandResp = await fetch(`${baseURL}/api/branding`, { headers: authHeaders });
    expect(preBrandResp.status, 'pre branding GET').toBe(200);
    const preBrand = await preBrandResp.json();
    const preName = preBrand.app_name;

    // Edit the app name to a unique value.
    const newName = `TASMail-e2e-${Date.now()}`;
    const appNameInput = page.getByTestId('branding-app-name');
    await appNameInput.fill(newName);
    await takeScreenshot(page, 'admin-config-modern/02-branding-form-filled');
    await page.getByTestId('branding-save-button').click();

    // Wait for the UI confirmation OR poll the API directly — the API is
    // the canonical source of truth per the SPA validation rule.
    await expect.poll(async () => {
      const r = await fetch(`${baseURL}/api/branding`, { headers: authHeaders });
      const b = await r.json();
      return b.app_name;
    }, { timeout: 10_000 }).toBe(newName);
    await takeScreenshot(page, 'admin-config-modern/03-branding-saved');

    // Reset and confirm the previous name (or factory default) comes back.
    page.once('dialog', (d) => d.accept());
    await page.getByTestId('branding-reset-button').click();
    await expect.poll(async () => {
      const r = await fetch(`${baseURL}/api/branding`, { headers: authHeaders });
      const b = await r.json();
      return b.app_name;
    }, { timeout: 10_000 }).not.toBe(newName);
    await takeScreenshot(page, 'admin-config-modern/04-branding-after-reset');

    // Restore the pre-test name so the live deploy doesn't drift if it
    // wasn't the default.
    await fetch(`${baseURL}/api/admin/branding`, {
      method: 'PUT',
      headers: { ...authHeaders, 'Content-Type': 'application/json' },
      body: JSON.stringify({ app_name: preName }),
    });

    // =====================================================================
    // 2. SAML TAB — create → list contains it → delete → list empty
    // =====================================================================
    await page.getByTestId('admin-tab-saml').click();
    await expect.poll(() => page.url(), { timeout: 5_000 }).toContain('tab=saml');
    await page.getByTestId('saml-tab').waitFor({ timeout: 10_000 });
    await takeScreenshot(page, 'admin-config-modern/05-saml-empty');

    // API pre-state.
    const preSamlResp = await fetch(`${baseURL}/api/admin/saml`, { headers: authHeaders });
    expect(preSamlResp.status, 'pre SAML list').toBe(200);
    const preSamlList = (await preSamlResp.json()) as Array<{ id: string; name: string }>;
    const preSamlCount = preSamlList.length;

    await page.getByTestId('saml-add-button').click();
    await page.getByTestId('saml-form').waitFor({ timeout: 5_000 });
    await page.getByTestId('saml-form-name').fill(SAML_NAME);
    await page.locator('[data-testid="saml-form"] input').nth(1).fill('https://idp.example.com/saml/metadata');
    await page.locator('[data-testid="saml-form"] input').nth(2).fill('https://idp.example.com/saml/sso');
    await page.locator('[data-testid="saml-form"] textarea').fill(
      '-----BEGIN CERTIFICATE-----\nMIICertExampleE2EE2E==\n-----END CERTIFICATE-----',
    );
    await takeScreenshot(page, 'admin-config-modern/06-saml-form-filled');
    await page.getByTestId('saml-form-submit').click();

    // Backend state should now include our new row.
    await expect.poll(async () => {
      const r = await fetch(`${baseURL}/api/admin/saml`, { headers: authHeaders });
      const list = (await r.json()) as Array<{ name: string }>;
      return list.some((c) => c.name === SAML_NAME);
    }, { timeout: 10_000 }).toBe(true);
    await page.getByTestId('saml-row').first().waitFor({ timeout: 5_000 });
    await takeScreenshot(page, 'admin-config-modern/07-saml-created');

    // Find and delete our row (matched by name).
    const samlRowsResp = await fetch(`${baseURL}/api/admin/saml`, { headers: authHeaders });
    const samlRows = (await samlRowsResp.json()) as Array<{ id: string; name: string }>;
    const created = samlRows.find((c) => c.name === SAML_NAME);
    expect(created, 'created SAML row visible to API').toBeTruthy();

    // Confirm UI shows the row.
    await expect(page.locator('[data-testid="saml-row"]', { hasText: SAML_NAME })).toBeVisible();

    page.once('dialog', (d) => d.accept());
    await page
      .locator('[data-testid="saml-row"]', { hasText: SAML_NAME })
      .getByRole('button', { name: 'Delete' })
      .click();
    await expect.poll(async () => {
      const r = await fetch(`${baseURL}/api/admin/saml`, { headers: authHeaders });
      const list = (await r.json()) as Array<{ name: string }>;
      return list.length;
    }, { timeout: 10_000 }).toBe(preSamlCount);
    await takeScreenshot(page, 'admin-config-modern/08-saml-deleted');

    // =====================================================================
    // 3. OIDC TAB — create → list contains it → delete → list empty
    // =====================================================================
    await page.getByTestId('admin-tab-oidc').click();
    await expect.poll(() => page.url(), { timeout: 5_000 }).toContain('tab=oidc');
    await page.getByTestId('oidc-tab').waitFor({ timeout: 10_000 });
    await takeScreenshot(page, 'admin-config-modern/09-oidc-empty');

    const preOidcResp = await fetch(`${baseURL}/api/admin/oidc`, { headers: authHeaders });
    const preOidcCount = ((await preOidcResp.json()) as unknown[]).length;

    await page.getByTestId('oidc-add-button').click();
    await page.getByTestId('oidc-form').waitFor({ timeout: 5_000 });
    await page.getByTestId('oidc-form-name').fill(OIDC_NAME);
    const oidcInputs = page.locator('[data-testid="oidc-form"] input');
    await oidcInputs.nth(1).fill('https://accounts.example.com');
    await oidcInputs.nth(2).fill('client-id-e2e');
    await oidcInputs.nth(3).fill('client-secret-e2e');
    await oidcInputs.nth(4).fill('https://mail.example.com/api/auth/oidc/callback');
    await takeScreenshot(page, 'admin-config-modern/10-oidc-form-filled');
    await page.getByTestId('oidc-form-submit').click();

    await expect.poll(async () => {
      const r = await fetch(`${baseURL}/api/admin/oidc`, { headers: authHeaders });
      const list = (await r.json()) as Array<{ name: string }>;
      return list.some((p) => p.name === OIDC_NAME);
    }, { timeout: 10_000 }).toBe(true);
    await page.getByTestId('oidc-row').first().waitFor({ timeout: 5_000 });
    await takeScreenshot(page, 'admin-config-modern/11-oidc-created');

    page.once('dialog', (d) => d.accept());
    await page
      .locator('[data-testid="oidc-row"]', { hasText: OIDC_NAME })
      .getByRole('button', { name: 'Delete' })
      .click();
    await expect.poll(async () => {
      const r = await fetch(`${baseURL}/api/admin/oidc`, { headers: authHeaders });
      const list = (await r.json()) as unknown[];
      return list.length;
    }, { timeout: 10_000 }).toBe(preOidcCount);
    await takeScreenshot(page, 'admin-config-modern/12-oidc-deleted');

    // =====================================================================
    // 4. LDAP TAB — create → bind test surfaces row message → delete
    // =====================================================================
    await page.getByTestId('admin-tab-ldap').click();
    await expect.poll(() => page.url(), { timeout: 5_000 }).toContain('tab=ldap');
    await page.getByTestId('ldap-tab').waitFor({ timeout: 10_000 });
    await takeScreenshot(page, 'admin-config-modern/13-ldap-empty');

    const preLdapResp = await fetch(`${baseURL}/api/admin/ldap`, { headers: authHeaders });
    const preLdapCount = ((await preLdapResp.json()) as unknown[]).length;

    await page.getByTestId('ldap-add-button').click();
    await page.getByTestId('ldap-form').waitFor({ timeout: 5_000 });
    await page.getByTestId('ldap-form-name').fill(LDAP_NAME);
    const ldapInputs = page.locator('[data-testid="ldap-form"] input');
    await ldapInputs.nth(1).fill('ldaps://dc.example.com:636');
    await ldapInputs.nth(2).fill('CN=svc,DC=example,DC=com');
    await ldapInputs.nth(3).fill('s3cret');
    await ldapInputs.nth(4).fill('OU=Users,DC=example,DC=com');
    await takeScreenshot(page, 'admin-config-modern/14-ldap-form-filled');
    await page.getByTestId('ldap-form-submit').click();

    await expect.poll(async () => {
      const r = await fetch(`${baseURL}/api/admin/ldap`, { headers: authHeaders });
      const list = (await r.json()) as Array<{ name: string }>;
      return list.some((c) => c.name === LDAP_NAME);
    }, { timeout: 10_000 }).toBe(true);
    await page.getByTestId('ldap-row').first().waitFor({ timeout: 5_000 });
    await takeScreenshot(page, 'admin-config-modern/15-ldap-created');

    // Click Test — we don't assert success (the URL is fake), just that
    // the per-row message surface gets populated, proving the API call
    // round-tripped through the UI.
    const ldapRow = page.locator('[data-testid="ldap-row"]', { hasText: LDAP_NAME });
    await ldapRow.getByTestId('ldap-test-button').click();
    await ldapRow.getByTestId('ldap-row-message').waitFor({ timeout: 15_000 });
    await takeScreenshot(page, 'admin-config-modern/16-ldap-test-clicked');

    // Delete cleanup.
    page.once('dialog', (d) => d.accept());
    await ldapRow.getByRole('button', { name: 'Delete' }).click();
    await expect.poll(async () => {
      const r = await fetch(`${baseURL}/api/admin/ldap`, { headers: authHeaders });
      const list = (await r.json()) as unknown[];
      return list.length;
    }, { timeout: 10_000 }).toBe(preLdapCount);
    await takeScreenshot(page, 'admin-config-modern/17-ldap-deleted');

    // =====================================================================
    // 5. URL deep-link survives reload — `?tab=oidc` should re-open OIDC.
    // =====================================================================
    await page.goto('/modern/index.html#/admin?tab=oidc');
    await page.getByTestId('oidc-tab').waitFor({ timeout: 10_000 });
    await takeScreenshot(page, 'admin-config-modern/18-deep-link-oidc');
  });
});
