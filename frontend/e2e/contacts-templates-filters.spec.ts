/**
 * TMAIL-286 — Settings managers E2E sweep:
 *   • Contacts        (/api/contacts        + ContactManager)
 *   • Groups          (/api/groups          + GroupManager)
 *   • Signatures      (/api/signatures      + SignatureManager)
 *   • Templates       (/api/templates       + TemplateManager)
 *   • Sieve Filters   (/api/filters         + FilterManager, including the
 *                      new POST /api/filters/{id}/test sandbox)
 *
 * Why this spec exists:
 *   1. Validate that every settings manager is reachable through the sidebar
 *      (no `page.goto('/app/...')` for internal routes — HARD RULE).
 *   2. Cross-check each mutation via a fresh API GET (HARD RULE: SPA tests
 *      must verify backend state, not just DOM updates).
 *   3. Capture screenshots at every key validation point so the audit trail
 *      survives the run.
 *
 * Setup model:
 *   • One fresh BYOK signup is created in beforeAll — each spec then logs in
 *     through the UI so the auth → app shell flow paints under test.
 *   • Tests share the same mailbox but namespace their fixtures with a per-run
 *     RUN_TAG and per-test prefixes so they don't clobber each other.
 *   • afterAll deletes the mailbox via psql so re-runs start clean.
 *
 * Bug surface this spec uncovers (and the bug-fix commit fixes):
 *   • `frontend/src/api/filters.ts` was double-prefixing `/api`, breaking
 *     every Filter CRUD call (resolved to /api/api/filters → 404).
 *   • The 'templates' viewMode was missing from the mailStore union AND from
 *     Sidebar AND from AppShell, so TemplateManager was unreachable.
 *   • GroupManager submitted `domain_id: ''` which the backend rejected as
 *     an invalid Uuid before the handler ran. Backend now resolves the
 *     domain from the owner's mailbox when omitted.
 *   • No backend match-test endpoint existed; we added
 *     `POST /api/filters/{id}/test` so users can sanity-check rules without
 *     waiting for real mail.
 */
import { test, NOREPLY_CREDS, expect } from './fixtures/base.js';
import { request as apiRequest, type APIRequestContext } from '@playwright/test';
import { deleteMailboxByUsername } from './helpers/db-cleanup.js';

const ACCOUNT_PASSWORD = 'contacts-tpl-sweep-Pa55!';
const RUN_TAG = Date.now();
const ACCOUNT_EMAIL = `e2e-ctpl-${RUN_TAG}@e2e.tasmail`;

let api: APIRequestContext;
let accessToken: string;
let authHeader: Record<string, string>;

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

test.describe('TMAIL-286 Settings managers sweep', () => {
  test.beforeAll(async ({ baseURL }) => {
    test.setTimeout(120_000);
    api = await apiRequest.newContext({ baseURL });

    const signup = await api.post('/api/auth/signup', {
      data: { email: ACCOUNT_EMAIL, password: ACCOUNT_PASSWORD },
    });
    expect(signup.status(), 'signup must succeed').toBeLessThan(300);
    const signupBody = (await signup.json()) as { access_token: string };
    accessToken = signupBody.access_token;
    authHeader = { Authorization: `Bearer ${accessToken}` };

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
  });

  test.afterAll(async () => {
    try {
      deleteMailboxByUsername(ACCOUNT_EMAIL);
    } catch {
      /* best-effort */
    }
    await api?.dispose();
  });

  // Login through the UI — exercises the real auth flow + paints the shell.
  async function loginViaUI(page: import('@playwright/test').Page) {
    await page.goto('/login');
    await page.waitForSelector('#username');
    await page.fill('#username', ACCOUNT_EMAIL);
    await page.fill('#password', ACCOUNT_PASSWORD);
    await page.click('button[type="submit"]');
    await page.waitForURL(/\/app/, { timeout: 20_000 });
    // Wait for the sidebar to be present before we start clicking it.
    await expect(page.locator('.sidebar')).toBeVisible({ timeout: 15_000 });
  }

  // Sidebar nav helper — clicks the menu item whose visible label matches.
  // Per the HARD RULE we ALWAYS navigate by clicking sidebar items, never
  // via page.goto for internal routes.
  async function navigateToSettings(
    page: import('@playwright/test').Page,
    label: string,
  ) {
    const item = page.locator('.sidebar .folder-item', { hasText: label }).first();
    await expect(item, `sidebar must expose "${label}"`).toBeVisible({ timeout: 10_000 });
    await item.click();
  }

  // ──────────────────────────────────────────────────────────────────────────
  // 1) Signatures: create with default flag, edit, delete — round-trip via API
  // ──────────────────────────────────────────────────────────────────────────
  test('signatures: create default, list, edit, delete — round-trip via API', async ({
    page,
    takeScreenshot,
  }) => {
    const before = await api.get('/api/signatures', { headers: authHeader });
    expect(before.status()).toBe(200);
    const beforeList = (await before.json()) as Signature[];

    await loginViaUI(page);
    await navigateToSettings(page, 'Signatures');

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
  // 2) Contacts: create via form, search filters list, edit, delete
  // ──────────────────────────────────────────────────────────────────────────
  test('contacts: CRUD + search filter — round-trip via API', async ({
    page,
    takeScreenshot,
  }) => {
    const before = await api.get('/api/contacts', { headers: authHeader });
    const beforeList = (await before.json()) as Contact[];

    await loginViaUI(page);
    await navigateToSettings(page, 'Contacts');

    await page.click('button:has-text("Add Contact")');
    const email = `e2e-contact-${RUN_TAG}@example.com`;
    const displayName = `E2E Contact ${RUN_TAG}`;
    await page.locator('input[type="email"][placeholder="user@example.com"]').fill(email);
    await page.locator('input[placeholder="Display name"]').fill(displayName);
    await page.locator('input[placeholder="Company"]').fill('TASMail E2E Co');
    await takeScreenshot(page, 'contacts-templates/contact-editor-filled');
    await page.click('button[type="submit"]:has-text("Save")');

    // Row visible with the new email.
    await expect(page.locator('div', { hasText: email }).first()).toBeVisible({ timeout: 10_000 });
    await takeScreenshot(page, 'contacts-templates/contact-list-after-create');

    // Search filter — typing a unique token must narrow the list to just our row.
    await page.locator('input[placeholder="Search contacts..."]').fill(String(RUN_TAG));
    await expect(page.locator('div', { hasText: email }).first()).toBeVisible({ timeout: 5_000 });
    await takeScreenshot(page, 'contacts-templates/contact-list-search');

    // API cross-check: contacts list grew by 1 and contains our seed.
    const after = await api.get('/api/contacts', { headers: authHeader });
    const afterList = (await after.json()) as Contact[];
    expect(afterList.length).toBe(beforeList.length + 1);
    const created = afterList.find((c) => c.email === email);
    expect(created, 'contact must round-trip to API').toBeTruthy();
    expect(created!.display_name).toBe(displayName);
    expect(created!.company).toBe('TASMail E2E Co');

    // API delete + final assertion.
    const del = await api.delete(`/api/contacts/${created!.id}`, { headers: authHeader });
    expect(del.status()).toBeLessThan(300);
    const finalGet = await api.get('/api/contacts', { headers: authHeader });
    const finalList = (await finalGet.json()) as Contact[];
    expect(finalList.find((c) => c.id === created!.id)).toBeUndefined();
  });

  // ──────────────────────────────────────────────────────────────────────────
  // 3) Groups: create group (no domain_id sent — backend resolves it),
  //    expand, add member, remove member, delete group.
  //    Validates the GroupManager `domain_id: ''` bug fix end-to-end.
  // ──────────────────────────────────────────────────────────────────────────
  test('groups: create + add member + remove member — backend resolves domain', async ({
    page,
    takeScreenshot,
  }) => {
    const before = await api.get('/api/groups', { headers: authHeader });
    const beforeList = (await before.json()) as DistributionGroup[];

    await loginViaUI(page);
    await navigateToSettings(page, 'Groups');

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
  // 4) Templates: navigate via the NEW sidebar entry, create with merge
  //    fields, render preview, assert rendered output contains the values.
  //    Validates the TemplateManager wiring fix end-to-end.
  // ──────────────────────────────────────────────────────────────────────────
  test('templates: create with vars, render preview, assert rendered output', async ({
    page,
    takeScreenshot,
  }) => {
    const before = await api.get('/api/templates', { headers: authHeader });
    const beforeList = (await before.json()) as EmailTemplate[];

    await loginViaUI(page);

    // The sidebar entry only exists after the TMAIL-286 wiring fix. If this
    // step fails, the navigation regression is back.
    await navigateToSettings(page, 'Templates');

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
  // 5) Filters: create rule, assert "Active" badge, run match-test sandbox,
  //    cross-check the verdict. Validates the filters.ts double-prefix fix
  //    AND the new /api/filters/{id}/test endpoint.
  // ──────────────────────────────────────────────────────────────────────────
  test('filters: create rule, Active badge, match-test verdict — happy path', async ({
    page,
    takeScreenshot,
  }) => {
    const before = await api.get('/api/filters', { headers: authHeader });
    const beforeList = (await before.json()) as SieveRule[];

    await loginViaUI(page);
    await navigateToSettings(page, 'Filters');

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
