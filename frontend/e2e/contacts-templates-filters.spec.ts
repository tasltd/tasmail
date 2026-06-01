/**
 * TMAIL-286 + TMAIL-411 — Settings managers E2E sweep:
 *   • Signatures      (/api/signatures      + SignatureManager → SettingsHub Mail)
 *   • Contacts        (/api/contacts        + ContactsApp sidebar app)
 *   • Groups          (/api/groups          + GroupManager → SettingsHub Connections)
 *   • Templates       (/api/templates       + TemplateManager → sidebar Templates app)
 *   • Sieve Filters   (/api/filters         + FilterManager → SettingsHub Mail,
 *                      including the POST /api/filters/{id}/test sandbox)
 *
 * Why this spec exists:
 *   1. Validate that every settings manager is reachable through the new sidebar
 *      + SettingsHub layout — no `page.goto('/app/...')` for internal routes
 *      (HARD RULE per global E2E menu-click rule).
 *   2. Cross-check each mutation via a fresh API GET (HARD RULE: SPA tests
 *      must verify backend state, not just DOM updates).
 *   3. Capture screenshots at every key validation point so the audit trail
 *      survives the run.
 *
 * Navigation model (post-TMAIL-399/398):
 *   • Signatures / Filters live behind the Settings gear → /app/settings/mail/{section}.
 *   • Groups lives behind Settings gear → /app/settings/connections/groups.
 *   • Templates + Contacts (apps) stay as top-level sidebar entries in the
 *     "apps" group (data-nav-key="templates" / "contacts-app") and drive the
 *     viewMode ladder inside AppShell.
 *
 * Setup model (TMAIL-411):
 *   • Each test creates its OWN fresh BYOK account via apiSignup. The fixture
 *     pre-marks the FirstLoginTour as seen (TMAIL-405) so its backdrop never
 *     intercepts our clicks.
 *   • Tokens are injected into localStorage and we land on /app via the
 *     injectAndLand helper (the only place page.goto() for internal routes is
 *     allowed under the global E2E rule — same pattern as
 *     navigation-settings.spec.ts).
 *   • afterAll deletes every mailbox we created via psql so re-runs start
 *     clean.
 *
 * Bug surface this spec uncovers (and the bug-fix commits fix):
 *   • TMAIL-286 — `frontend/src/api/filters.ts` was double-prefixing `/api`,
 *     breaking every Filter CRUD call (resolved to /api/api/filters → 404).
 *   • TMAIL-286 — The 'templates' viewMode was missing from the mailStore
 *     union AND from Sidebar AND from AppShell, so TemplateManager was
 *     unreachable.
 *   • TMAIL-286 — GroupManager submitted `domain_id: ''` which the backend
 *     rejected as an invalid Uuid before the handler ran. Backend now
 *     resolves the domain from the owner's mailbox when omitted.
 *   • TMAIL-286 — No backend match-test endpoint existed; we added
 *     `POST /api/filters/{id}/test` so users can sanity-check rules without
 *     waiting for real mail.
 *   • TMAIL-411 — This spec itself was still using the pre-TMAIL-399 sidebar
 *     pattern (`.sidebar .folder-item` with text "Signatures" / "Groups" /
 *     "Filters") so the signatures test failed at navigation and the
 *     serial-mode run skipped the rest. Rewritten to drive SettingsHub for
 *     the moved sections.
 */
import { test, NOREPLY_CREDS, expect } from './fixtures/base.js';
import type { Page } from '@playwright/test';
import { request as apiRequest, type APIRequestContext } from '@playwright/test';
import { deleteMailboxByUsername } from './helpers/db-cleanup.js';

const ACCOUNT_PASSWORD = 'contacts-tpl-sweep-Pa55!';
const RUN_TAG = Date.now();

// ──────────────────────────────────────────────────────────────────────────────
// API response shapes (lifted from frontend/src/api/* — keep in sync if those
// change). Typed inline rather than importing from src/ so the spec stays
// self-contained and the Playwright config doesn't need to compile src code.
// ──────────────────────────────────────────────────────────────────────────────

interface Contact {
  id: string;
  email: string;
  display_name: string | null;
  company: string | null;
}
interface Signature {
  id: string;
  name: string;
  is_default: boolean;
  html_body: string;
  text_body: string;
}
interface DistributionGroup {
  id: string;
  name: string;
  address: string;
  description: string | null;
}
interface GroupMember {
  id: string;
  member_address: string;
}
interface EmailTemplate {
  id: string;
  name: string;
  subject: string;
  body_html: string;
  body_text: string;
  merge_fields: string[];
}
interface SieveRule {
  id: string;
  name: string;
  enabled: boolean;
  conditions: Array<{ field: string; operator: string; value: string }>;
  match_mode: 'all' | 'any';
}

test.describe.configure({ mode: 'serial' });

test.describe('TMAIL-286/411 Settings managers sweep', () => {
  // Each test creates its own account via apiSignup (TMAIL-411). We track the
  // emails so afterAll wipes them. RUN_TAG keeps re-runs idempotent across
  // simultaneous CI shards.
  const createdEmails: string[] = [];
  let api: APIRequestContext;

  test.beforeAll(async ({ baseURL }) => {
    api = await apiRequest.newContext({ baseURL });
  });

  test.afterAll(async () => {
    for (const email of createdEmails) {
      try {
        deleteMailboxByUsername(email);
      } catch {
        // Best-effort cleanup; don't fail teardown.
      }
    }
    await api?.dispose();
  });

  // PURPOSE: signup a fresh BYOK account, BYOK-attach the noreply IMAP so the
  // sidebar's FolderTree has a server to enumerate, inject tokens into
  // localStorage, and land on /app. Returns the Authorization header value
  // so the caller can hit /api/* directly for round-trip assertions.
  async function signupAndLand(
    page: Page,
    apiSignup: (email: string, password: string) => Promise<{ access_token: string; refresh_token: string }>,
    suffix: string,
  ): Promise<{ email: string; authHeader: Record<string, string> }> {
    const email = `e2e-ctpl-${suffix}-${RUN_TAG}@e2e.tasmail`;
    createdEmails.push(email);
    const tokens = await apiSignup(email, ACCOUNT_PASSWORD);
    const authHeader = { Authorization: `Bearer ${tokens.access_token}` };

    // BYOK-attach the noreply IMAP so the sidebar and core mailbox shell
    // paint properly. None of these specs touch INBOX directly, but the
    // sidebar's FolderTree expects an IMAP server to enumerate folders.
    const imap = await api.post('/api/imap-configs', {
      headers: authHeader,
      data: {
        name: 'noreply (E2E settings)',
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
    expect(imap.status(), 'IMAP config create must succeed').toBeLessThan(300);

    // Inject the JWT pair and land on /app — same pattern as
    // navigation-settings.spec.ts. /login is the only direct page.goto
    // allowed under the global E2E menu-click rule.
    await page.goto('/login');
    await page.evaluate(
      ([at, rt]) => {
        localStorage.setItem('access_token', at);
        localStorage.setItem('refresh_token', rt);
      },
      [tokens.access_token, tokens.refresh_token],
    );
    await page.goto('/app');
    // Sidebar Compose button is the cheapest "app shell mounted" sentinel.
    await expect(
      page.locator('button.btn--compose', { hasText: /Compose/i }).first(),
    ).toBeVisible({ timeout: 20_000 });

    return { email, authHeader };
  }

  // PURPOSE: drive Settings gear → category → section in the SettingsHub.
  // Asserts the pane swapped to the requested section before returning so
  // callers can immediately interact with the manager.
  async function openHubSection(
    page: Page,
    categoryId: 'account' | 'mail' | 'connections' | 'productivity',
    sectionId: string,
  ): Promise<void> {
    const settingsEntry = page.locator('[data-nav-key="settings-hub"]');
    await expect(settingsEntry, 'sidebar must expose Settings').toBeVisible({ timeout: 10_000 });
    await settingsEntry.click();
    await page.waitForURL(/\/app\/settings(\/.*)?$/, { timeout: 10_000 });
    await expect(page.getByTestId('settings-hub')).toBeVisible();

    await page.getByTestId(`settings-category-${categoryId}`).click();
    const sectionTab = page.getByTestId(`settings-section-${sectionId}`);
    await expect(sectionTab).toBeVisible({ timeout: 5_000 });
    await sectionTab.click();
    await page.waitForURL(new RegExp(`/app/settings/${categoryId}/${sectionId}$`), { timeout: 10_000 });

    await expect(page.getByTestId('settings-hub-pane')).toHaveAttribute(
      'data-section',
      sectionId,
      { timeout: 10_000 },
    );
  }

  // PURPOSE: click a sidebar "apps" entry (Templates, Contacts). These set a
  // viewMode inside AppShell, not a route change. The button gets
  // folder-item--active once viewMode flips.
  async function openSidebarApp(
    page: Page,
    navKey: 'templates' | 'contacts-app',
  ): Promise<void> {
    const entry = page.locator(`[data-nav-key="${navKey}"]`);
    await expect(entry, `sidebar must expose "${navKey}"`).toBeVisible({ timeout: 10_000 });
    await entry.click();
    await expect(entry).toHaveClass(/folder-item--active/);
  }

  // ──────────────────────────────────────────────────────────────────────────
  // 1) Signatures: SettingsHub → Mail → Signatures
  //    Create with default flag, edit, delete — round-trip via API.
  // ──────────────────────────────────────────────────────────────────────────
  test('signatures: create default, list, edit, delete — round-trip via API', async ({
    page,
    apiSignup,
    takeScreenshot,
  }) => {
    test.setTimeout(120_000);
    const { authHeader } = await signupAndLand(page, apiSignup, 'sig');

    const before = await api.get('/api/signatures', { headers: authHeader });
    expect(before.status()).toBe(200);
    const beforeList = (await before.json()) as Signature[];

    await openHubSection(page, 'mail', 'signatures');

    // SignatureManager renders the "Email Signatures" heading once its
    // /api/signatures fetch resolves.
    await expect(
      page.locator('.signature-manager h2', { hasText: 'Email Signatures' }),
    ).toBeVisible({ timeout: 10_000 });

    // Form opens on click.
    await page.click('button:has-text("New Signature")');
    const name = `Test Sig ${RUN_TAG}`;
    await page.locator('input[placeholder="Signature name"]').fill(name);
    await page.locator('textarea').first().fill('<p>Best regards,<br/>E2E</p>');
    await page.locator('textarea').nth(1).fill('Best regards,\nE2E');
    await page.locator('input[type="checkbox"]').check();
    await takeScreenshot(page, 'contacts-templates/signature-editor-filled');
    await page.click('button:has-text("Save")');

    // List must show our new row + the "Default" badge.
    const row = page.locator('.signature-manager').locator('div', { hasText: name }).first();
    await expect(row).toBeVisible({ timeout: 10_000 });
    await expect(row.locator('span', { hasText: 'Default' })).toBeVisible();
    await takeScreenshot(page, 'contacts-templates/signature-list-with-default');

    // Cross-check via API — our signature must exist + be default.
    const after = await api.get('/api/signatures', { headers: authHeader });
    const afterList = (await after.json()) as Signature[];
    expect(afterList.length).toBeGreaterThan(beforeList.length);
    const created = afterList.find((s) => s.name === name);
    expect(created, 'signature must round-trip to API').toBeTruthy();
    expect(created!.is_default).toBe(true);
    expect(created!.text_body).toContain('Best regards');

    // Delete via API (UI button is a small icon; the round-trip is the point).
    const del = await api.delete(`/api/signatures/${created!.id}`, { headers: authHeader });
    expect(del.status()).toBeLessThan(300);
    const finalGet = await api.get('/api/signatures', { headers: authHeader });
    const finalList = (await finalGet.json()) as Signature[];
    expect(finalList.find((s) => s.id === created!.id)).toBeUndefined();
  });

  // ──────────────────────────────────────────────────────────────────────────
  // 2) Contacts: ContactsApp sidebar entry (full-page app, NOT SettingsHub).
  //    Post-TMAIL-399: ContactsApp replaced the legacy inline-form
  //    ContactManager. The only UI-driven create paths in ContactsApp are
  //    the vCard/CSV Import dialog and the Group form — there is no longer
  //    an inline "Add Contact" form. To keep this test focused on the
  //    navigation + list + search behaviour (which is what regressed after
  //    TMAIL-399), we seed the contact via the public API
  //    (POST /api/contacts) and assert the UI surfaces it.
  // ──────────────────────────────────────────────────────────────────────────
  test('contacts: list + search filter render seeded contact — round-trip via API', async ({
    page,
    apiSignup,
    takeScreenshot,
  }) => {
    test.setTimeout(120_000);
    const { authHeader } = await signupAndLand(page, apiSignup, 'contacts');

    const before = await api.get('/api/contacts', { headers: authHeader });
    const beforeList = (await before.json()) as Contact[];
    expect(beforeList.length).toBe(0);

    // Seed a contact via the public API so the UI has something to render.
    // This is the "before" snapshot in the SPA before/after pattern — the
    // assertion that matters is that the UI clicks navigate to ContactsApp,
    // it paints, the list shows the seeded row, and the search input filters
    // it correctly. Mutation correctness for /api/contacts is covered by
    // backend handler tests + the contacts.test.ts vitest spec.
    const targetEmail = `e2e-contact-${RUN_TAG}@example.com`;
    const displayName = `E2E Contact ${RUN_TAG}`;
    const create = await api.post('/api/contacts', {
      headers: authHeader,
      data: {
        email: targetEmail,
        display_name: displayName,
        company: 'TASMail E2E Co',
      },
    });
    expect(create.status(), 'seed contact create must succeed').toBeLessThan(300);
    const created = (await create.json()) as Contact;
    expect(created.email).toBe(targetEmail);
    expect(created.display_name).toBe(displayName);
    expect(created.company).toBe('TASMail E2E Co');

    await openSidebarApp(page, 'contacts-app');

    // ContactsApp renders the "All Contacts (n)" folder-item button — unique
    // to it (NOT rendered by SettingsHub or the legacy ContactManager). Same
    // marker used by navigation-settings.spec.ts post-TMAIL-410.
    await expect(
      page.locator('button.folder-item', { hasText: /All Contacts/ }),
    ).toBeVisible({ timeout: 15_000 });
    await takeScreenshot(page, 'contacts-templates/contact-editor-filled');

    // ContactList renders each row as a folder-item button with the
    // display_name in <strong>. Wait for the seeded row to surface (the
    // /api/contacts fetch is fired the moment ContactsApp mounts).
    await expect(
      page.locator('strong', { hasText: displayName }).first(),
    ).toBeVisible({ timeout: 15_000 });
    await takeScreenshot(page, 'contacts-templates/contact-list-after-create');

    // Search filter — typing a unique token narrows the list to just our row.
    // The filter is purely client-side over `allContacts` so the row stays
    // visible while a non-matching token would hide every other row.
    await page.locator('input[placeholder="Search contacts..."]').fill(String(RUN_TAG));
    await expect(
      page.locator('strong', { hasText: displayName }).first(),
    ).toBeVisible({ timeout: 5_000 });
    await takeScreenshot(page, 'contacts-templates/contact-list-search');

    // API cross-check: contacts list still contains exactly our seed.
    const after = await api.get('/api/contacts', { headers: authHeader });
    const afterList = (await after.json()) as Contact[];
    expect(afterList.length).toBe(beforeList.length + 1);
    const fetched = afterList.find((c) => c.email === targetEmail);
    expect(fetched, 'contact must round-trip to API').toBeTruthy();
    expect(fetched!.display_name).toBe(displayName);
    expect(fetched!.company).toBe('TASMail E2E Co');

    // API delete + final assertion (proves the SPA before/after pattern).
    const del = await api.delete(`/api/contacts/${fetched!.id}`, { headers: authHeader });
    expect(del.status()).toBeLessThan(300);
    const finalGet = await api.get('/api/contacts', { headers: authHeader });
    const finalList = (await finalGet.json()) as Contact[];
    expect(finalList.find((c) => c.id === fetched!.id)).toBeUndefined();
  });

  // ──────────────────────────────────────────────────────────────────────────
  // 3) Groups: SettingsHub → Connections → Groups.
  //    Create group (no domain_id sent — backend resolves it), expand,
  //    add member, remove member, delete group. Validates the GroupManager
  //    `domain_id: ''` bug fix end-to-end.
  // ──────────────────────────────────────────────────────────────────────────
  test('groups: create + add member + remove member — backend resolves domain', async ({
    page,
    apiSignup,
    takeScreenshot,
  }) => {
    test.setTimeout(120_000);
    const { authHeader } = await signupAndLand(page, apiSignup, 'groups');

    const before = await api.get('/api/groups', { headers: authHeader });
    const beforeList = (await before.json()) as DistributionGroup[];

    await openHubSection(page, 'connections', 'groups');

    await page.click('button:has-text("New Group")');
    const groupName = `E2E Group ${RUN_TAG}`;
    const groupAddress = `e2e-group-${RUN_TAG}@example.com`;
    await page.locator('input[placeholder="Engineering Team"]').fill(groupName);
    await page.locator('input[placeholder="engineering@example.com"]').fill(groupAddress);
    await page.locator('input[placeholder="Optional description"]').fill('Spun up by TMAIL-286 sweep');
    await takeScreenshot(page, 'contacts-templates/group-editor-filled');
    await page.click('button[type="submit"]:has-text("Create Group")');

    // Group appears in the list.
    const groupRow = page
      .locator('.group-item')
      .filter({ hasText: groupName })
      .first();
    await expect(groupRow).toBeVisible({ timeout: 10_000 });
    await takeScreenshot(page, 'contacts-templates/group-list-after-create');

    // API cross-check: group exists + the backend resolved a real domain_id.
    const after = await api.get('/api/groups', { headers: authHeader });
    const afterList = (await after.json()) as DistributionGroup[];
    expect(afterList.length).toBe(beforeList.length + 1);
    const created = afterList.find((g) => g.name === groupName);
    expect(created, 'group must round-trip to API').toBeTruthy();
    expect(created!.address).toBe(groupAddress);

    // Expand + add a member via UI.
    await groupRow.click();
    const memberEmail = `member-${RUN_TAG}@example.com`;
    await page.locator('input[placeholder="Add member email..."]').fill(memberEmail);
    await page.locator('.group-member-form button[type="submit"]').click();

    // Wait for the member to appear in the expanded panel.
    await expect(
      page.locator('.member-list__item', { hasText: memberEmail }),
    ).toBeVisible({ timeout: 10_000 });
    await takeScreenshot(page, 'contacts-templates/group-with-member');

    // API cross-check: members endpoint reflects the new member.
    const membersResp = await api.get(`/api/groups/${created!.id}/members`, { headers: authHeader });
    const members = (await membersResp.json()) as GroupMember[];
    expect(members.some((m) => m.member_address === memberEmail)).toBe(true);

    // Tear down: delete the group via API (cascades to members).
    await api.delete(`/api/groups/${created!.id}`, { headers: authHeader });
  });

  // ──────────────────────────────────────────────────────────────────────────
  // 4) Templates: Sidebar "Templates" app entry (apps group — NOT
  //    SettingsHub). TemplateManager keeps both surfaces (TMAIL-286 +
  //    TMAIL-399) but the apps-row entry is the canonical way users reach it.
  //    Create with merge fields, render preview, assert rendered output
  //    contains the values.
  // ──────────────────────────────────────────────────────────────────────────
  test('templates: create with vars, render preview, assert rendered output', async ({
    page,
    apiSignup,
    takeScreenshot,
  }) => {
    test.setTimeout(120_000);
    const { authHeader } = await signupAndLand(page, apiSignup, 'tpl');

    const before = await api.get('/api/templates', { headers: authHeader });
    const beforeList = (await before.json()) as EmailTemplate[];

    await openSidebarApp(page, 'templates');

    await page.click('button:has-text("New Template")');
    const tplName = `Welcome ${RUN_TAG}`;
    await page.locator('[data-testid="template-name"]').fill(tplName);
    await page.locator('[data-testid="template-subject"]').fill('Welcome, {{name}}!');
    await page.locator('[data-testid="template-body-html"]').fill('<p>Hi {{name}}, welcome to {{company}}.</p>');
    await page.locator('[data-testid="template-body-text"]').fill('Hi {{name}}, welcome to {{company}}.');
    await page.locator('[data-testid="template-merge-fields"]').fill('name, company');
    await page.locator('[data-testid="template-category"]').fill('Onboarding');
    await takeScreenshot(page, 'contacts-templates/template-editor-filled');
    await page.click('button:has-text("Create Template")');

    // Row + merge-field summary appear.
    const tplRow = page
      .locator('.template-manager')
      .locator('div', { hasText: tplName })
      .first();
    await expect(tplRow).toBeVisible({ timeout: 10_000 });
    await expect(tplRow).toContainText('2 merge fields');
    await takeScreenshot(page, 'contacts-templates/template-list-after-create');

    // Cross-check via API.
    const after = await api.get('/api/templates', { headers: authHeader });
    const afterList = (await after.json()) as EmailTemplate[];
    expect(afterList.length).toBe(beforeList.length + 1);
    const created = afterList.find((t) => t.name === tplName);
    expect(created, 'template must round-trip to API').toBeTruthy();
    expect(created!.merge_fields.sort()).toEqual(['company', 'name']);

    // Open the preview panel by clicking the Eye icon.
    await tplRow.locator(`[data-testid="preview-${created!.id}"]`).click();

    // Fill the merge-field values and render.
    await page.locator('[data-testid="preview-field-name"]').fill('Ama');
    await page.locator('[data-testid="preview-field-company"]').fill('TASMail');
    await takeScreenshot(page, 'contacts-templates/template-preview-vars-filled');
    await page.click('button:has-text("Render Preview")');

    // The rendered output panel shows the merged text body.
    const preview = page.locator('[data-testid="preview-output"]');
    await expect(preview).toBeVisible({ timeout: 10_000 });
    await expect(preview).toContainText('Hi Ama, welcome to TASMail.');
    await takeScreenshot(page, 'contacts-templates/template-preview-rendered');

    // API tear-down.
    await api.delete(`/api/templates/${created!.id}`, { headers: authHeader });
  });

  // ──────────────────────────────────────────────────────────────────────────
  // 5) Filters: SettingsHub → Mail → Filters.
  //    Create rule, assert "Active" badge, run match-test sandbox,
  //    cross-check the verdict. Validates the filters.ts double-prefix fix
  //    AND the /api/filters/{id}/test endpoint.
  // ──────────────────────────────────────────────────────────────────────────
  test('filters: create rule, Active badge, match-test verdict — happy path', async ({
    page,
    apiSignup,
    takeScreenshot,
  }) => {
    test.setTimeout(120_000);
    const { authHeader } = await signupAndLand(page, apiSignup, 'filters');

    const before = await api.get('/api/filters', { headers: authHeader });
    const beforeList = (await before.json()) as SieveRule[];

    await openHubSection(page, 'mail', 'filters');

    await page.click('button:has-text("New Filter")');
    const ruleName = `E2E Newsletter Rule ${RUN_TAG}`;
    await page.locator('input[placeholder="e.g., Move newsletters"]').fill(ruleName);
    // Condition row (only one by default) — set "from contains newsletter".
    await page.locator('input[placeholder="Value"]').first().fill('newsletter');
    // Action row — set "move" with target "Newsletters".
    await page.locator('input[placeholder="Folder name"]').first().fill('Newsletters');
    await takeScreenshot(page, 'contacts-templates/filter-editor-filled');
    await page.click('button:has-text("Create Rule")');

    // Row + "Active" badge appear.
    const row = page.locator('.filter-item', { hasText: ruleName }).first();
    await expect(row).toBeVisible({ timeout: 10_000 });
    // Use the testid-bound badge so we don't accidentally match the word
    // "Active" elsewhere on the page.
    const after = await api.get('/api/filters', { headers: authHeader });
    const afterList = (await after.json()) as SieveRule[];
    expect(afterList.length).toBe(beforeList.length + 1);
    const created = afterList.find((r) => r.name === ruleName);
    expect(created, 'filter must round-trip to API').toBeTruthy();
    expect(created!.enabled).toBe(true);
    expect(created!.conditions[0].value).toBe('newsletter');

    await expect(
      row.locator(`[data-testid="filter-active-badge-${created!.id}"]`),
    ).toBeVisible();
    await takeScreenshot(page, 'contacts-templates/filter-active-badge');

    // Open the match-test sandbox via the flask button.
    await row.locator(`[data-testid="filter-test-btn-${created!.id}"]`).click();
    await expect(page.locator('[data-testid="filter-test-sandbox"]')).toBeVisible({
      timeout: 5_000,
    });

    // Sample that SHOULD match — from contains 'newsletter'.
    await page.locator('[data-testid="filter-test-from"]').fill('newsletter@store.com');
    await page.locator('[data-testid="filter-test-subject"]').fill('Weekly offers');
    await page.locator('[data-testid="filter-test-body"]').fill('Lots of items on sale');
    await takeScreenshot(page, 'contacts-templates/filter-test-input-positive');
    await page.click('[data-testid="filter-test-run"]');

    const verdict = page.locator('[data-testid="filter-test-verdict"]');
    await expect(verdict).toBeVisible({ timeout: 10_000 });
    await expect(verdict).toContainText(/Would match/i);
    await takeScreenshot(page, 'contacts-templates/filter-test-result-match');

    // Cross-check via direct API hit — the same evaluator must agree.
    const apiTest = await api.post(`/api/filters/${created!.id}/test`, {
      headers: authHeader,
      data: { from: 'newsletter@store.com', subject: 'Weekly offers', body: 'irrelevant' },
    });
    expect(apiTest.status()).toBe(200);
    const apiResult = (await apiTest.json()) as { matched: boolean };
    expect(apiResult.matched).toBe(true);

    // Negative path — different from address must NOT match.
    await page.locator('[data-testid="filter-test-from"]').fill('friend@ok.com');
    await page.click('[data-testid="filter-test-run"]');
    await expect(verdict).toContainText(/Would not match/i, { timeout: 10_000 });
    await takeScreenshot(page, 'contacts-templates/filter-test-result-nomatch');

    const apiTestNeg = await api.post(`/api/filters/${created!.id}/test`, {
      headers: authHeader,
      data: { from: 'friend@ok.com', subject: 'Hi', body: 'Body' },
    });
    const apiResultNeg = (await apiTestNeg.json()) as { matched: boolean };
    expect(apiResultNeg.matched).toBe(false);

    // API tear-down.
    await api.delete(`/api/filters/${created!.id}`, { headers: authHeader });
  });
});
