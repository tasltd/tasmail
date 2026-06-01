/**
 * TMAIL-288 — Tasks + Auto-reply + Quota bar E2E sweep
 *
 * Surfaces covered (3 separate tests, fresh BYOK account each):
 *   1. Tasks app  — sidebar nav-key "tasks" → AppShell viewMode='tasks' →
 *                   TaskManager. Creates task via UI, toggles completion via
 *                   UI, deletes via UI. Round-trips every mutation through
 *                   GET /api/tasks (SPA validation rule: API state before AND
 *                   after, never UI-only).
 *   2. Auto-reply — Sidebar Settings gear → SettingsHub → Mail → Vacation
 *                   Responder. Enables the responder, fills subject/body/
 *                   start+end date, saves, and asserts GET /api/auto-reply
 *                   reflects the active config.
 *   3. Quota bar  — Sidebar always renders the QuotaBar footer. Asserts the
 *                   bar paints, "used / total" labels are non-empty, no
 *                   "NaN" leaks (TMAIL-417 regression), and GET /api/quota
 *                   returns a coherent QuotaStatus payload.
 *
 * Navigation pattern (HARD RULE — menu clicks only, never page.goto for
 * internal routes): every section is reached by clicking sidebar entries or
 * SettingsHub category/section testids — same model as
 * contacts-templates-filters.spec.ts.
 *
 * Setup model (TMAIL-411 pattern):
 *   • Each test creates its own fresh BYOK account via apiSignup. The fixture
 *     pre-marks the FirstLoginTour as seen (TMAIL-405) so its backdrop never
 *     intercepts our clicks.
 *   • Tokens are injected into localStorage and we land on /app via the
 *     signupAndLand helper (the only place a direct page.goto for internal
 *     routes is allowed — to seed localStorage on the /login page first).
 *   • afterAll deletes every mailbox we created via psql so re-runs start
 *     clean.
 *
 * Screenshots:
 *   Captured under e2e/screenshots/tasks-autoreply-quota/* at every key
 *   validation point per the HARD RULE. Names follow {feature}-{action}.png.
 */
import { test, NOREPLY_CREDS, expect } from './fixtures/base.js';
import type { Page } from '@playwright/test';
import { request as apiRequest, type APIRequestContext } from '@playwright/test';
import { deleteMailboxByUsername } from './helpers/db-cleanup.js';

const ACCOUNT_PASSWORD = 'tasks-autoreply-quota-Pa55!';
const RUN_TAG = Date.now();

// ──────────────────────────────────────────────────────────────────────────
// API response shapes — lifted from frontend/src/api/{tasks,auto-reply,quota}.ts.
// Kept inline so the spec stays self-contained.
// ──────────────────────────────────────────────────────────────────────────

interface Task {
  id: string;
  user_id: string;
  title: string;
  description: string | null;
  due_date: string | null;
  completed: boolean;
  completed_at: string | null;
  priority: string;
  linked_folder: string | null;
  linked_uid: number | null;
  linked_subject: string | null;
  created_at: string;
  updated_at: string;
}

interface AutoReplyRule {
  id: string;
  mailbox_id: string;
  enabled: boolean;
  subject: string;
  body_text: string;
  body_html: string | null;
  start_date: string | null;
  end_date: string | null;
  reply_to_all: boolean;
  exclude_lists: boolean;
  created_at: string;
  updated_at: string;
}

interface QuotaStatus {
  mailbox_id: string;
  quota_bytes: number;
  used_bytes: number;
  message_count: number;
  usage_percent: number;
  quota_warn_percent: number;
  is_over_quota: boolean;
  is_warning: boolean;
  last_synced_at: string | null;
}

test.describe.configure({ mode: 'serial' });

test.describe('TMAIL-288 Tasks + Auto-reply + Quota sweep', () => {
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

  // PURPOSE: signup a fresh BYOK account, attach the noreply IMAP so the
  // sidebar's FolderTree has a server to enumerate, inject tokens into
  // localStorage, and land on /app. Returns the email + Authorization header
  // so the caller can hit /api/* for round-trip assertions.
  async function signupAndLand(
    page: Page,
    apiSignup: (email: string, password: string) => Promise<{ access_token: string; refresh_token: string }>,
    suffix: string,
  ): Promise<{ email: string; authHeader: Record<string, string> }> {
    const email = `e2e-tau-${suffix}-${RUN_TAG}@e2e.tasmail`;
    createdEmails.push(email);
    const tokens = await apiSignup(email, ACCOUNT_PASSWORD);
    const authHeader = { Authorization: `Bearer ${tokens.access_token}` };

    // BYOK-attach the noreply IMAP so the sidebar and core mailbox shell
    // paint properly. None of these specs touch INBOX directly, but the
    // sidebar's FolderTree expects an IMAP server to enumerate folders and
    // the QuotaBar's /api/quota call expects a mailbox row with quota_bytes.
    const imap = await api.post('/api/imap-configs', {
      headers: authHeader,
      data: {
        name: 'noreply (E2E tau)',
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

    // /login is the ONLY direct page.goto allowed under the global E2E rule;
    // use it to seed localStorage before landing on /app.
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
      page.locator('button.btn--compose', { hasText: /Compose/i }).first(),
    ).toBeVisible({ timeout: 20_000 });

    return { email, authHeader };
  }

  // PURPOSE: click a sidebar "apps" entry (Tasks, Templates, Contacts) — these
  // set a viewMode inside AppShell, not a route change. The entry gets the
  // folder-item--active class once the store flips, which we wait for.
  async function openSidebarApp(page: Page, navKey: 'tasks'): Promise<void> {
    const entry = page.locator(`[data-nav-key="${navKey}"]`);
    await expect(entry, `sidebar must expose "${navKey}"`).toBeVisible({ timeout: 10_000 });
    await entry.click();
    await expect(entry).toHaveClass(/folder-item--active/);
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

  // ──────────────────────────────────────────────────────────────────────────
  // 1) Tasks: sidebar "Tasks" entry → TaskManager
  //    Create via inline form, toggle complete, delete — round-trip via API.
  // ──────────────────────────────────────────────────────────────────────────
  test('tasks: create via form, toggle complete, delete — round-trip via API', async ({
    page,
    apiSignup,
    takeScreenshot,
  }) => {
    test.setTimeout(120_000);
    const { authHeader } = await signupAndLand(page, apiSignup, 'tasks');

    // Baseline: a brand new account has no tasks.
    const before = await api.get('/api/tasks', { headers: authHeader });
    expect(before.status()).toBe(200);
    const beforeList = (await before.json()) as Task[];
    expect(beforeList.length).toBe(0);

    await openSidebarApp(page, 'tasks');

    // TaskManager renders the "Tasks" h2 + "Add Task" button once mounted.
    await expect(page.locator('h2', { hasText: /^Tasks$/ })).toBeVisible({ timeout: 10_000 });
    await expect(
      page.getByText('No tasks yet. Add one to get started.', { exact: false }),
    ).toBeVisible();
    await takeScreenshot(page, 'tasks-autoreply-quota/tasks-empty-state');

    // Open the inline TaskForm by clicking "Add Task" (top-right primary button).
    await page.click('button:has-text("Add Task")');
    await expect(page.locator('h3', { hasText: 'New Task' })).toBeVisible();

    const title = `E2E Task ${RUN_TAG}`;
    await page.locator('input[placeholder="Task title"]').fill(title);
    // Description textarea is the first textarea in the form.
    await page
      .locator('textarea[placeholder="Optional description"]')
      .fill('Review Q4 proposal pricing section before EOD');
    // Priority select — TaskForm uses a native <select> with low/normal/high/urgent.
    await page.locator('select').selectOption('high');
    await takeScreenshot(page, 'tasks-autoreply-quota/tasks-form-filled');

    // Submit via the form's "Add Task" button (the one inside .composer__actions —
    // distinguished from the toolbar "Add Task" by being inside the form).
    await page.locator('form button[type="submit"]:has-text("Add Task")').click();

    // Wait for the row to appear in the list. TaskManager toggles isCreating
    // back to false on mutation success, so the form unmounts and the row
    // renders inside the list.
    const taskRow = page.locator('div').filter({ hasText: title }).first();
    await expect(taskRow).toBeVisible({ timeout: 10_000 });
    await takeScreenshot(page, 'tasks-autoreply-quota/tasks-list-after-create');

    // Cross-check via API — backend now has the task with the correct priority.
    const afterCreate = await api.get('/api/tasks', { headers: authHeader });
    const afterCreateList = (await afterCreate.json()) as Task[];
    expect(afterCreateList.length).toBe(1);
    const created = afterCreateList[0];
    expect(created.title).toBe(title);
    expect(created.priority).toBe('high');
    expect(created.completed).toBe(false);

    // Toggle completion via the checkbox button (the Square / CheckSquare icon
    // sits inside a button with the title attribute "Mark complete").
    await page.locator('button[title="Mark complete"]').click();
    // The button title flips to "Mark incomplete" once the task is done.
    await expect(page.locator('button[title="Mark incomplete"]')).toBeVisible({ timeout: 10_000 });
    await takeScreenshot(page, 'tasks-autoreply-quota/tasks-after-toggle-complete');

    // Cross-check via API — completed flag flipped.
    const afterToggle = await api.get('/api/tasks?completed=true', { headers: authHeader });
    const afterToggleList = (await afterToggle.json()) as Task[];
    expect(afterToggleList.length).toBe(1);
    expect(afterToggleList[0].completed).toBe(true);
    expect(afterToggleList[0].completed_at).not.toBeNull();

    // Filter tabs — click "active" tab, the completed task should disappear.
    await page.locator('button:has-text("active")').first().click();
    await expect(page.getByText('No active tasks.', { exact: false })).toBeVisible({
      timeout: 10_000,
    });
    await takeScreenshot(page, 'tasks-autoreply-quota/tasks-active-filter-empty');

    // Switch to "completed" tab — our task should be there.
    await page.locator('button:has-text("completed")').first().click();
    await expect(page.locator('div').filter({ hasText: title }).first()).toBeVisible({
      timeout: 10_000,
    });
    await takeScreenshot(page, 'tasks-autoreply-quota/tasks-completed-filter');

    // Delete via the trash icon (button with title "Delete task").
    await page.locator('button[title="Delete task"]').click();
    // After delete + invalidate, the completed tab is empty again.
    await expect(page.getByText('No completed tasks.', { exact: false })).toBeVisible({
      timeout: 10_000,
    });

    // Final API check — task is gone.
    const afterDelete = await api.get('/api/tasks', { headers: authHeader });
    const afterDeleteList = (await afterDelete.json()) as Task[];
    expect(afterDeleteList.length).toBe(0);
  });

  // ──────────────────────────────────────────────────────────────────────────
  // 2) Auto-reply: SettingsHub → Mail → Vacation Responder
  //    Enable + fill + save — round-trip via GET /api/auto-reply.
  // ──────────────────────────────────────────────────────────────────────────
  test('auto-reply: enable + fill + save — round-trip via API', async ({
    page,
    apiSignup,
    takeScreenshot,
  }) => {
    test.setTimeout(120_000);
    const { authHeader } = await signupAndLand(page, apiSignup, 'autoreply');

    // Baseline: a brand new account has no auto-reply rule.
    const before = await api.get('/api/auto-reply', { headers: authHeader });
    expect(before.status()).toBe(200);
    const beforeBody = await before.json();
    expect(beforeBody).toBeNull();

    await openHubSection(page, 'mail', 'vacation');

    // VacationResponder renders an h2 "Vacation Responder" once mounted.
    // Scope EVERY interaction to the SettingsHub pane — the AppShell has a
    // global search input + Compose textarea live in DOM that would otherwise
    // steal the .first() selectors and silently send our subject/body to the
    // wrong inputs.
    const pane = page.getByTestId('settings-hub-pane');
    await expect(pane.locator('h2', { hasText: 'Vacation Responder' })).toBeVisible({
      timeout: 10_000,
    });
    await takeScreenshot(page, 'tasks-autoreply-quota/autoreply-pristine');

    // Enable the responder via its "Enable vacation responder" checkbox.
    const enableCheckbox = pane
      .locator('label', { hasText: 'Enable vacation responder' })
      .locator('input[type="checkbox"]');
    await enableCheckbox.check();
    await expect(enableCheckbox).toBeChecked();

    // Fill the form.
    const subject = `On Holiday — TMAIL-288 ${RUN_TAG}`;
    const bodyText =
      "I'm out of the office until further notice. For urgent matters please contact ops@techatscale.io.";
    await pane.locator('input[type="text"]').first().fill(subject);
    await pane.locator('textarea').fill(bodyText);

    // Set start + end dates — datetime-local inputs accept "YYYY-MM-DDTHH:MM".
    // Pick start = now-ish (an hour from now), end = 7 days later.
    const now = new Date();
    const start = new Date(now.getTime() + 60 * 60 * 1000);
    const end = new Date(now.getTime() + 7 * 24 * 60 * 60 * 1000);
    const toLocalInput = (d: Date) =>
      `${d.getFullYear()}-${String(d.getMonth() + 1).padStart(2, '0')}-${String(d.getDate()).padStart(2, '0')}T${String(d.getHours()).padStart(2, '0')}:${String(d.getMinutes()).padStart(2, '0')}`;
    const startStr = toLocalInput(start);
    const endStr = toLocalInput(end);
    await pane.locator('input[type="datetime-local"]').nth(0).fill(startStr);
    await pane.locator('input[type="datetime-local"]').nth(1).fill(endStr);

    await takeScreenshot(page, 'tasks-autoreply-quota/autoreply-form-filled');

    // Save — wait on the actual PUT /api/auto-reply response so we know the
    // mutation hit the backend before doing the GET round-trip. Relying on
    // the "Saved!" label flicker is fragile because the label reverts after
    // 2s and the React-Query invalidate-then-refetch can race the assertion.
    const savePromise = page.waitForResponse(
      (resp) => resp.url().includes('/api/auto-reply') && resp.request().method() === 'PUT',
      { timeout: 15_000 },
    );
    await pane.locator('button:has-text("Save Settings")').click();
    const saveResp = await savePromise;
    expect(saveResp.status(), 'PUT /api/auto-reply must succeed').toBeLessThan(300);
    await takeScreenshot(page, 'tasks-autoreply-quota/autoreply-after-save');

    // Cross-check via API — backend now has an active auto-reply rule that
    // mirrors what we typed.
    const after = await api.get('/api/auto-reply', { headers: authHeader });
    expect(after.status()).toBe(200);
    const rule = (await after.json()) as AutoReplyRule | null;
    expect(rule, 'auto-reply must round-trip to API').not.toBeNull();
    expect(rule!.enabled).toBe(true);
    expect(rule!.subject).toBe(subject);
    expect(rule!.body_text).toBe(bodyText);
    expect(rule!.start_date).not.toBeNull();
    expect(rule!.end_date).not.toBeNull();
    // ISO timestamps round-trip — backend stores UTC. The exact instant must
    // match the local-time strings we typed (modulo timezone), so we just
    // assert both endpoints are valid Date instances ordered start < end.
    const apiStart = new Date(rule!.start_date!);
    const apiEnd = new Date(rule!.end_date!);
    expect(apiStart.getTime()).toBeLessThan(apiEnd.getTime());

    // Composer banner gap: per the issue spec, an "auto-reply active banner"
    // should surface in the Composer when a rule is active. The current
    // Composer doesn't render one as of TMAIL-288 (documented in the
    // assessment doc as a follow-up). To capture the gap without coupling
    // this assertion to product behavior that hasn't shipped yet, we just
    // re-screenshot the saved form so the audit trail shows the responder
    // is enabled end-to-end.
    await expect(enableCheckbox, 'enable toggle stays checked after save').toBeChecked();
    await takeScreenshot(page, 'tasks-autoreply-quota/autoreply-active-state');
  });

  // ──────────────────────────────────────────────────────────────────────────
  // 3) Quota bar: Sidebar footer always renders QuotaBar — assert paint,
  //    GET /api/quota shape, and no NaN regression (TMAIL-417).
  // ──────────────────────────────────────────────────────────────────────────
  test('quota: sidebar bar renders + /api/quota returns coherent state', async ({
    page,
    apiSignup,
    takeScreenshot,
  }) => {
    test.setTimeout(120_000);
    const { authHeader } = await signupAndLand(page, apiSignup, 'quota');

    // Fetch /api/quota directly — the SPA refetches every 5 min but the
    // sidebar paints on first response. A brand-new mailbox should have a
    // non-zero quota_bytes (provisioned on signup) and 0 used_bytes.
    const quotaResp = await api.get('/api/quota', { headers: authHeader });
    expect(quotaResp.status()).toBe(200);
    const quota = (await quotaResp.json()) as QuotaStatus;
    expect(Number.isFinite(quota.quota_bytes)).toBe(true);
    expect(Number.isFinite(quota.used_bytes)).toBe(true);
    expect(Number.isFinite(quota.usage_percent)).toBe(true);
    expect(quota.quota_bytes).toBeGreaterThan(0);
    expect(quota.used_bytes).toBeGreaterThanOrEqual(0);
    expect(quota.is_over_quota).toBe(false);

    // The sidebar's QuotaBar mounts on its own React-Query fetch. Wait for
    // the bar to appear — it renders a <div class="quota-bar"> wrapper.
    const quotaBar = page.locator('.quota-bar');
    await expect(quotaBar).toBeVisible({ timeout: 15_000 });

    // "used" + total labels must paint without NaN / undefined — TMAIL-417
    // regression guard.
    await expect(quotaBar.locator('span', { hasText: /used$/ })).toBeVisible();
    const barText = (await quotaBar.textContent()) ?? '';
    expect(barText).not.toMatch(/NaN/);
    expect(barText).not.toMatch(/undefined/);
    await takeScreenshot(page, 'tasks-autoreply-quota/quota-bar-baseline');

    // Trigger an explicit sync via POST /api/quota/sync to confirm the
    // backend path also returns coherent state. The IMAP fetch may fail
    // (BYOK server might not support QUOTA) — backend handles it gracefully
    // and returns 0 bytes used. Either way, the HTTP status must be 200.
    const sync = await api.post('/api/quota/sync', { headers: authHeader });
    expect(sync.status()).toBe(200);
    const synced = (await sync.json()) as QuotaStatus;
    expect(Number.isFinite(synced.usage_percent)).toBe(true);
    expect(synced.quota_bytes).toBeGreaterThan(0);

    // Reload the sidebar so the bar picks up the synced state (the SPA also
    // refetches on staleTime but we want a deterministic screenshot).
    await page.reload();
    await expect(page.locator('.quota-bar')).toBeVisible({ timeout: 15_000 });
    await takeScreenshot(page, 'tasks-autoreply-quota/quota-bar-after-sync');

    // Capture the threshold-state screenshot context. Forcing a warning /
    // over-quota state without filling the real mailbox would require
    // injecting fake usage rows directly into Postgres — which the assessment
    // documents as a follow-up (TMAIL-288 audit notes). We assert the
    // is_warning / is_over_quota *predicates* are well-typed booleans so
    // any future state simulation has a stable contract to test against.
    expect(typeof quota.is_warning).toBe('boolean');
    expect(typeof quota.is_over_quota).toBe('boolean');
  });
});
