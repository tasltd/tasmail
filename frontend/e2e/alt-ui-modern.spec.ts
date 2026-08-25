/**
 * TMAIL-292: focused E2E sweep of the alt-UI ("modern") theme.
 *
 * Covers:
 *   1. Classic TopBar wand-icon hop  →  /modern/index.html
 *   2. AuthGate JWT reuse from localStorage (no second login)
 *   3. EmailClient + EmailList render the same INBOX backed by /api/folders
 *   4. EmailReader hydrates the message body via /api/folders/{f}/messages/{uid}
 *   5. ComposeModal opens and sends via scheduledApi (delay_seconds=0)
 *      — backend state verified by /api/messages/queue or the Sent folder delta
 *   6. ← Classic link in the alt-UI Navbar drops the user back at /app
 *
 * Build prerequisite (handled by the auto-fix runner): `npm run build:alt-ui`
 * so the bundle in `frontend/public/modern/` is the latest source.
 *
 * Screenshots: frontend/e2e/screenshots/alt-ui/<step>.png
 *
 * Runs against the live tunnel/proxy by default (see playwright.config.ts).
 */
import { test, NOREPLY_CREDS, expect } from './fixtures/base.js';
import { deleteMailboxByUsername } from './helpers/db-cleanup.js';

const SCREENSHOT_DIR = 'alt-ui';
const PASSWORD = 'alt-ui-modern-2026';

test.describe('TMAIL-292 alt-UI modern theme sweep', () => {
  test.beforeAll(() => deleteMailboxByUsername(NOREPLY_CREDS.email));
  test.afterAll(() => deleteMailboxByUsername(NOREPLY_CREDS.email));

  test('classic → wand hop → list/read/compose → ← Classic', async ({
    page,
    apiSignup,
    takeScreenshot,
    baseURL,
  }) => {
    test.setTimeout(120_000);

    // ── 1. BYOK: sign up + attach the noreply mailbox so the alt-UI has
    //         something real to render through /api/folders. ───────────
    const tokens = await apiSignup(NOREPLY_CREDS.email, PASSWORD);
    const auth = {
      Authorization: `Bearer ${tokens.access_token}`,
      'Content-Type': 'application/json',
    };
    const imapResp = await fetch(`${baseURL}/api/imap-configs`, {
      method: 'POST',
      headers: auth,
      body: JSON.stringify({
        name: 'tmail-292-imap',
        host: NOREPLY_CREDS.imap.host,
        port: NOREPLY_CREDS.imap.port,
        username: NOREPLY_CREDS.imap.username,
        password: NOREPLY_CREDS.imap.password,
        encryption: NOREPLY_CREDS.imap.encryption,
        is_default: true,
      }),
    });
    expect(imapResp.status, 'IMAP config').toBe(201);
    const smtpResp = await fetch(`${baseURL}/api/smtp-configs`, {
      method: 'POST',
      headers: auth,
      body: JSON.stringify({
        name: 'tmail-292-smtp',
        host: NOREPLY_CREDS.smtp.host,
        port: NOREPLY_CREDS.smtp.port,
        username: NOREPLY_CREDS.smtp.username,
        password: NOREPLY_CREDS.smtp.password,
        encryption: NOREPLY_CREDS.smtp.encryption,
        from_address: NOREPLY_CREDS.email,
        is_default: true,
      }),
    });
    expect(smtpResp.status, 'SMTP config').toBe(201);

    // ── 2. Inject the JWT pair and open the classic /app. ────────────
    await page.goto('/login');
    await page.evaluate(([at, rt]) => {
      localStorage.setItem('access_token', at);
      localStorage.setItem('refresh_token', rt);
    }, [tokens.access_token, tokens.refresh_token]);
    await page.goto('/app');
    await expect(
      page.locator('text=New Message').first(),
    ).toBeVisible({ timeout: 20_000 });
    await expect(
      page.locator('button, a, li', { hasText: /INBOX/i }).first(),
    ).toBeVisible({ timeout: 25_000 });

    // The wand-icon anchor is in the classic TopBar. Screenshot it before
    // clicking so the assessment doc can show the entry point.
    const wand = page.locator('a[title="Try the modern UI"]');
    await expect(wand, 'wand-icon hop button visible in classic TopBar').toBeVisible();
    await takeScreenshot(page, `${SCREENSHOT_DIR}/01-classic-topbar-wand`);

    // ── 3. Hop to /modern/ — full-page nav, JWT pair survives because
    //         localStorage is same-origin. ───────────────────────────
    await wand.click();
    await page.waitForURL(/\/modern\/index\.html/i, { timeout: 15_000 });
    await page.waitForLoadState('domcontentloaded');
    await expect(page).toHaveTitle(/Modern UI/i);

    // Prove the AuthGate did NOT bounce back to /login: the URL must still
    // be /modern/, and the access_token must still be in localStorage.
    const tokenStillThere = await page.evaluate(() => localStorage.getItem('access_token'));
    expect(tokenStillThere, 'JWT survives the hop').toBe(tokens.access_token);
    expect(page.url()).toContain('/modern/index.html');
    await takeScreenshot(page, `${SCREENSHOT_DIR}/02-alt-ui-after-hop`);

    // ── 4. EmailList renders the real INBOX from /api/folders. ───────
    await expect(
      page.locator('h2', { hasText: /INBOX/i }),
      'INBOX header rendered',
    ).toBeVisible({ timeout: 25_000 });
    // At least one envelope row from the noreply mailbox.
    await expect(
      page
        .locator('text=/Mail Delivery|TASMail SMTP Test|Failed to deliver|Re:|Fwd:/')
        .first(),
      'real message row visible',
    ).toBeVisible({ timeout: 25_000 });
    // Sidebar's folder list populated (folders query resolved).
    await expect(page.locator('.px-2 > .group').first()).toBeVisible({
      timeout: 10_000,
    });
    await takeScreenshot(page, `${SCREENSHOT_DIR}/03-emaillist-rendered`);

    // ── 5. EmailReader hydrates body via /api/folders/{f}/messages/{uid}.
    const firstRow = page.locator('div.cursor-pointer').first();
    await firstRow.waitFor({ state: 'visible', timeout: 15_000 });
    await firstRow.click();
    // Reader pane has its own <h2> with the subject; wait for it to render.
    await expect(page.locator('h2').nth(1)).toBeVisible({ timeout: 15_000 });
    // Avatar fallback initials only appear in the reader header, not the list.
    await expect(page.locator('[class*="rounded-full"]').first()).toBeVisible({
      timeout: 10_000,
    });
    await takeScreenshot(page, `${SCREENSHOT_DIR}/04-emailreader-open`);

    // ── 6. ComposeModal — open, fill, send via scheduledApi. ─────────
    // Validate API state before/after per the SPA E2E HARD RULE.
    const sentBefore = await fetch(
      `${baseURL}/api/folders/Sent/messages?page=0&page_size=10`,
      { headers: auth },
    );
    const sentBeforeJson = sentBefore.ok
      ? await sentBefore.json()
      : { messages: [] };
    const sentBeforeCount = (sentBeforeJson.messages ?? []).length;

    await page.locator('text=New Message').first().click();
    await expect(page.locator('text=New Message')).toBeVisible({
      timeout: 5_000,
    });
    await page
      .locator('input[placeholder*="alice"]')
      .first()
      .fill(NOREPLY_CREDS.email);
    await page
      .locator('input[placeholder="Subject"]')
      .fill('TMAIL-292 alt-UI sweep');
    // TMAIL-330: composer body is a TipTap ProseMirror editor (no <textarea>).
    // Click into the contenteditable then type the body via the keyboard.
    const composeBody = page.locator('[data-testid="compose-rte-editor"]');
    await composeBody.click();
    await page.keyboard.type('Sent from the alt-UI ComposeModal via scheduledApi.');
    await takeScreenshot(page, `${SCREENSHOT_DIR}/05-composemodal-filled`);

    // ComposeModal "Send" → scheduledApi.scheduleSend({ delay_seconds: 0 }).
    await page.locator('button', { hasText: /^Send$/ }).first().click();
    // Modal closes once the mutation resolves.
    await expect(page.locator('text=New Message')).toBeHidden({
      timeout: 15_000,
    });
    await takeScreenshot(page, `${SCREENSHOT_DIR}/06-compose-send-confirmation`);

    // Backend round-trip — scheduledApi posts to /api/messages/schedule.
    // The handler clamps delay_seconds to a minimum of 5s, so the row first
    // lands in `scheduled_emails` (immediate) and the email-scheduler worker
    // moves it to Sent ~5–15s later. Accept either as proof of round-trip;
    // either side is conclusive evidence the compose path is live.
    let sendConfirmed = false;
    for (let attempt = 0; attempt < 12 && !sendConfirmed; attempt++) {
      await page.waitForTimeout(1500);
      const schedResp = await fetch(`${baseURL}/api/messages/scheduled`, {
        headers: auth,
      });
      if (schedResp.ok) {
        const rows = (await schedResp.json()) as Array<{ subject?: string }>;
        if (rows.some((r) => r.subject?.includes('TMAIL-292 alt-UI sweep'))) {
          sendConfirmed = true;
          break;
        }
      }
      const sentAfter = await fetch(
        `${baseURL}/api/folders/Sent/messages?page=0&page_size=10`,
        { headers: auth },
      );
      if (sentAfter.ok) {
        const sentAfterJson = await sentAfter.json();
        if ((sentAfterJson.messages ?? []).length > sentBeforeCount) {
          sendConfirmed = true;
          break;
        }
      }
    }
    expect(
      sendConfirmed,
      'compose round-trip — message reached scheduled_emails or Sent folder',
    ).toBe(true);

    // ── 7. ← Classic link drops the user back at /app. ───────────────
    // Navbar lives at the top of the alt-UI viewport — hit the Compose
    // close path first so the modal isn't blocking the click.
    await expect(page.locator('text=Classic')).toBeVisible({ timeout: 8_000 });
    const classicLink = page.locator('a', { hasText: /Classic/ }).first();
    await Promise.all([
      page.waitForURL(/\/app/i, { timeout: 15_000 }),
      classicLink.click(),
    ]);
    await expect(
      page.locator('text=New Message').first(),
      'back at classic SPA',
    ).toBeVisible({ timeout: 15_000 });
    // JWT must still be there — proves we never bounced through /login.
    const tokenAfterHopBack = await page.evaluate(() =>
      localStorage.getItem('access_token'),
    );
    expect(tokenAfterHopBack, 'JWT persisted through both hops').toBe(
      tokens.access_token,
    );
    await takeScreenshot(page, `${SCREENSHOT_DIR}/07-back-to-classic`);
  });
});
