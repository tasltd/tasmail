/**
 * TMAIL-331: alt-UI ("modern") Settings → Signatures.
 *
 * Coverage:
 *   1. Sign up + BYOK the noreply mailbox so /api/signatures has a real
 *      account row to attach signatures to.
 *   2. Hop into /modern/ via the classic SPA's wand button.
 *   3. Open Navbar → Settings → Signatures pane and assert it ships the
 *      real CRUD UI (not the "Coming soon" placeholder).
 *   4. CREATE: fill the editor with name/HTML/text body, mark default,
 *      save. Round-trip check: GET /api/signatures returns the new row
 *      with is_default=true.
 *   5. COMPOSE INSERTION: open Compose. Assert the editor body contains
 *      the signature HTML — proves the modal seeds new messages with the
 *      default signature.
 *   6. EDIT: rename via the Edit button, save, GET /api/signatures
 *      confirms the rename.
 *   7. SET DEFAULT toggle: create a second signature with default=false,
 *      then click its star — GET confirms the new row is now the default
 *      and the first is no longer.
 *   8. DELETE: confirm dialog → click Delete on the second signature →
 *      GET /api/signatures shows it's gone.
 *
 * Screenshots: frontend/e2e/screenshots/signatures/<step>.png
 *
 * Build prerequisite: `npm run build:alt-ui` so frontend/public/modern/
 * reflects this commit. The auto-fix runner handles that.
 *
 * Default browser: Firefox (per the global E2E HARD RULE). Configured in
 * playwright.config.ts; no per-test override needed.
 */
import { expect, NOREPLY_CREDS, test } from '../fixtures/base.js';
import { deleteMailboxByUsername } from '../helpers/db-cleanup.js';

const SCREENSHOT_DIR = 'signatures';
const PASSWORD = 'tmail-331-signatures-2026';

interface SignatureRow {
  id: string;
  name: string;
  html_body: string;
  text_body: string;
  is_default: boolean;
}

test.describe('TMAIL-331 alt-UI Signatures CRUD + default insertion on compose', () => {
  test.beforeAll(() => deleteMailboxByUsername(NOREPLY_CREDS.email));
  test.afterAll(() => deleteMailboxByUsername(NOREPLY_CREDS.email));

  test('full signature lifecycle (create → default → compose insert → edit → toggle → delete)', async ({
    page,
    apiSignup,
    takeScreenshot,
    baseURL,
  }) => {
    test.setTimeout(180_000);

    // ── 1. signup + BYOK ────────────────────────────────────────────────
    const tokens = await apiSignup(NOREPLY_CREDS.email, PASSWORD);
    const auth = {
      Authorization: `Bearer ${tokens.access_token}`,
      'Content-Type': 'application/json',
    };
    const imapResp = await fetch(`${baseURL}/api/imap-configs`, {
      method: 'POST',
      headers: auth,
      body: JSON.stringify({
        name: 'tmail-331-imap',
        host: NOREPLY_CREDS.imap.host,
        port: NOREPLY_CREDS.imap.port,
        username: NOREPLY_CREDS.imap.username,
        password: NOREPLY_CREDS.imap.password,
        encryption: NOREPLY_CREDS.imap.encryption,
        is_default: true,
      }),
    });
    expect(imapResp.status, 'IMAP config').toBe(201);

    // ── 2. open classic /app then hop to /modern/ ───────────────────────
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
    await takeScreenshot(page, `${SCREENSHOT_DIR}/01-modern-ui-loaded`);

    // ── 3. Navigate Settings → Signatures via the side-nav ──────────────
    // Per HARD RULE: navigate by clicking menu, never by direct goto.
    await page.getByTestId('navbar-settings-link').click();
    await page.waitForURL(/#\/settings\/profile/i, { timeout: 10_000 });
    await page.getByTestId('settings-tab-signatures').click();
    await page.waitForURL(/#\/settings\/signatures/i, { timeout: 10_000 });
    await expect(page.getByTestId('settings-tab-signatures-pane')).toBeVisible({
      timeout: 10_000,
    });
    // The real pane shows the New signature CTA — not the placeholder.
    await expect(page.getByTestId('signatures-new-button')).toBeVisible();
    await expect(page.getByTestId('signatures-empty')).toBeVisible();
    await takeScreenshot(page, `${SCREENSHOT_DIR}/02-signatures-empty`);

    // ── 4. Snapshot API state BEFORE create ─────────────────────────────
    const beforeResp = await fetch(`${baseURL}/api/signatures`, {
      headers: auth,
    });
    expect(beforeResp.status, 'GET /api/signatures before').toBe(200);
    const beforeList = (await beforeResp.json()) as SignatureRow[];
    expect(beforeList, 'no signatures before').toHaveLength(0);

    // ── 5. CREATE: fill the editor, tick default, save ──────────────────
    await page.getByTestId('signatures-new-button').click();
    await expect(page.getByTestId('signature-editor')).toBeVisible();

    const SIG_NAME = `Work signature ${Date.now()}`;
    const SIG_TEXT = 'Best,\nNoreply\nTASMail QA';

    await page.getByTestId('signature-name-input').fill(SIG_NAME);

    // Type into the TipTap editor — the placeholder text disappears as soon
    // as we focus + type. We exercise the Bold button to confirm the rich-
    // text toolbar is wired (mirrors TMAIL-330).
    const editor = page.locator('[data-testid="signature-rte-editor"]');
    await editor.click();
    await page.getByTestId('signature-rte-bold').click();
    await page.keyboard.type('Noreply Bot');
    await page.getByTestId('signature-rte-bold').click();
    await page.keyboard.press('Enter');
    await page.keyboard.type('TASMail QA');

    await page.getByTestId('signature-text-input').fill(SIG_TEXT);
    await page.getByTestId('signature-default-checkbox').check();
    await takeScreenshot(page, `${SCREENSHOT_DIR}/03-signature-filled`);
    await page.getByTestId('signature-save-button').click();

    // Editor closes once the mutation settles.
    await expect(page.getByTestId('signature-editor')).toBeHidden({
      timeout: 10_000,
    });

    // ── 6. Verify backend state via GET /api/signatures ─────────────────
    const afterCreateResp = await fetch(`${baseURL}/api/signatures`, {
      headers: auth,
    });
    expect(afterCreateResp.status, 'GET /api/signatures after create').toBe(200);
    const afterCreateList = (await afterCreateResp.json()) as SignatureRow[];
    expect(afterCreateList, 'one signature exists').toHaveLength(1);
    const firstSig = afterCreateList[0];
    expect(firstSig.name).toBe(SIG_NAME);
    expect(firstSig.is_default, 'first signature is the default').toBe(true);
    expect(firstSig.html_body, 'html body has the bold mark').toMatch(
      /<strong>[^<]*Noreply Bot[^<]*<\/strong>/i,
    );
    expect(firstSig.text_body, 'text body persisted').toContain('Noreply');

    // The row should show up with the Default badge in the UI.
    await expect(
      page.getByTestId(`signature-default-badge-${firstSig.id}`),
    ).toBeVisible();
    await takeScreenshot(page, `${SCREENSHOT_DIR}/04-signature-created`);

    // ── 7. COMPOSE INSERTION: default signature seeds the editor ────────
    // Hop back to inbox (via the side-nav Back arrow) then open Compose.
    await page.getByRole('link', { name: /back to inbox/i }).click();
    await expect(page.locator('h2', { hasText: /INBOX/i })).toBeVisible({
      timeout: 15_000,
    });
    await page.locator('button', { hasText: 'Compose' }).first().click();
    await expect(page.locator('text=New Message')).toBeVisible({
      timeout: 10_000,
    });
    const composeEditor = page.locator('[data-testid="compose-rte-editor"]');
    // The seed effect renders after the editor mounts + the query resolves —
    // poll up to 10s for the signature mark to appear.
    await expect(composeEditor).toContainText(/Noreply Bot/i, {
      timeout: 10_000,
    });
    await expect(composeEditor.locator('strong')).toBeVisible();
    await takeScreenshot(page, `${SCREENSHOT_DIR}/05-compose-with-signature`);
    // Close the compose modal without sending.
    await page.locator('button', { hasText: /Discard/i }).click();

    // ── 8. EDIT: rename the signature and re-verify via API ─────────────
    await page.getByTestId('navbar-settings-link').click();
    await page.getByTestId('settings-tab-signatures').click();
    await page.getByTestId(`signature-edit-${firstSig.id}`).click();
    await expect(page.getByTestId('signature-editor')).toBeVisible();

    const RENAMED = `${SIG_NAME} renamed`;
    const nameInput = page.getByTestId('signature-name-input');
    await nameInput.fill(RENAMED);
    await takeScreenshot(page, `${SCREENSHOT_DIR}/06-signature-renaming`);
    await page.getByTestId('signature-save-button').click();
    await expect(page.getByTestId('signature-editor')).toBeHidden({
      timeout: 10_000,
    });

    const afterRenameResp = await fetch(`${baseURL}/api/signatures`, {
      headers: auth,
    });
    const afterRenameList =
      (await afterRenameResp.json()) as SignatureRow[];
    const renamed = afterRenameList.find((s) => s.id === firstSig.id);
    expect(renamed, 'signature still exists after rename').toBeTruthy();
    expect(renamed!.name).toBe(RENAMED);
    expect(renamed!.is_default).toBe(true);

    // ── 9. SET DEFAULT toggle: create second signature, then star it ────
    await page.getByTestId('signatures-new-button').click();
    await page.getByTestId('signature-name-input').fill('Personal');
    await page.locator('[data-testid="signature-rte-editor"]').click();
    await page.keyboard.type('Sent from my phone');
    await page.getByTestId('signature-text-input').fill('Sent from my phone');
    // Leave the default checkbox UN-checked.
    await page.getByTestId('signature-save-button').click();
    await expect(page.getByTestId('signature-editor')).toBeHidden({
      timeout: 10_000,
    });

    const twoResp = await fetch(`${baseURL}/api/signatures`, { headers: auth });
    const twoList = (await twoResp.json()) as SignatureRow[];
    expect(twoList, 'two signatures exist').toHaveLength(2);
    const second = twoList.find((s) => s.id !== firstSig.id);
    expect(second, 'second signature row').toBeTruthy();
    expect(second!.is_default, 'second is NOT default at create').toBe(false);

    await takeScreenshot(page, `${SCREENSHOT_DIR}/07-two-signatures`);

    // Click the StarOff button on the second row to promote it to default.
    await page.getByTestId(`signature-set-default-${second!.id}`).click();
    // Default badge moves to the second row.
    await expect(
      page.getByTestId(`signature-default-badge-${second!.id}`),
    ).toBeVisible({ timeout: 10_000 });

    const promotedResp = await fetch(`${baseURL}/api/signatures`, {
      headers: auth,
    });
    const promotedList = (await promotedResp.json()) as SignatureRow[];
    const newDefault = promotedList.find((s) => s.id === second!.id);
    const oldDefault = promotedList.find((s) => s.id === firstSig.id);
    expect(newDefault!.is_default, 'second is now the default').toBe(true);
    expect(oldDefault!.is_default, 'first is no longer the default').toBe(false);
    await takeScreenshot(page, `${SCREENSHOT_DIR}/08-default-flipped`);

    // ── 10. DELETE the second signature ─────────────────────────────────
    // Auto-accept the confirm() dialog.
    page.once('dialog', (d) => d.accept());
    await page.getByTestId(`signature-delete-${second!.id}`).click();
    await expect(
      page.getByTestId(`signature-row-${second!.id}`),
    ).toBeHidden({ timeout: 10_000 });

    const finalResp = await fetch(`${baseURL}/api/signatures`, {
      headers: auth,
    });
    const finalList = (await finalResp.json()) as SignatureRow[];
    expect(finalList, 'only the first signature remains').toHaveLength(1);
    expect(finalList[0].id).toBe(firstSig.id);
    await takeScreenshot(page, `${SCREENSHOT_DIR}/09-after-delete`);
  });
});
