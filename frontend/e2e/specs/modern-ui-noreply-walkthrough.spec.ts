/**
 * TMAIL-228: full alt-UI walk-through as noreply@techatscale.io.
 *
 * Signs up the noreply mailbox, attaches its own swmail IMAP/SMTP config,
 * opens the classic dashboard, hops into the alt-UI via the TopBar
 * switcher, and walks every screen the user can reach. Captures a
 * screenshot at each step so the reviewer can scan the gallery.
 *
 * Coverage targets:
 *   1. Classic /app loaded
 *   2. Click "Try the modern UI" → alt-UI loads
 *   3. Inbox renders with real swmail messages
 *   4. Click a message — reader pane shows the real body
 *   5. Switch to a different folder (Sent / Junk / Drafts)
 *   6. Open compose modal
 *   7. Toggle to admin route via gear icon
 *   8. Toggle to calendar route via sidebar
 *   9. Click ← Classic to return to the production SPA
 */
import { test, NOREPLY_CREDS, expect } from '../fixtures/base.js';
import { deleteMailboxByUsername } from '../helpers/db-cleanup.js';

const PASSWORD = 'modern-walkthrough-2026';

test.describe('Modern UI walk-through as noreply (TMAIL-228)', () => {
  test.beforeAll(() => deleteMailboxByUsername(NOREPLY_CREDS.email));
  test.afterAll(() => deleteMailboxByUsername(NOREPLY_CREDS.email));

  test('every alt-UI screen renders against the real noreply mailbox', async ({
    page,
    apiSignup,
    takeScreenshot,
    baseURL,
  }) => {
    test.setTimeout(150_000);

    // ── 1. signup + BYOK config ──────────────────────────────────────
    const tokens = await apiSignup(NOREPLY_CREDS.email, PASSWORD);
    const auth = { Authorization: `Bearer ${tokens.access_token}`, 'Content-Type': 'application/json' };
    const imapResp = await fetch(`${baseURL}/api/imap-configs`, {
      method: 'POST',
      headers: auth,
      body: JSON.stringify({
        name: 'walkthrough-imap',
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
        name: 'walkthrough-smtp',
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

    // ── 2. classic /app loaded ───────────────────────────────────────
    await page.goto('/login');
    await page.evaluate(([at, rt]) => {
      localStorage.setItem('access_token', at);
      localStorage.setItem('refresh_token', rt);
    }, [tokens.access_token, tokens.refresh_token]);
    await page.goto('/app');
    await expect(page.locator('button, a', { hasText: /Compose/i }).first()).toBeVisible({ timeout: 20_000 });
    await expect(page.locator('button, a, li', { hasText: /INBOX/i }).first()).toBeVisible({ timeout: 25_000 });
    await takeScreenshot(page, 'modern-walkthrough/01-classic-app-loaded');

    // ── 3. hop to alt-UI ─────────────────────────────────────────────
    await page.locator('a[title="Try the modern UI"]').click();
    await page.waitForURL(/\/modern\/index\.html/i, { timeout: 15_000 });
    await page.waitForLoadState('domcontentloaded');
    await expect(page).toHaveTitle(/Modern UI/i);
    await takeScreenshot(page, 'modern-walkthrough/02-alt-ui-after-hop');

    // ── 4. inbox with real swmail messages ──────────────────────────
    await expect(page.locator('h2', { hasText: /INBOX/i })).toBeVisible({ timeout: 20_000 });
    await expect(page.locator('text=/Mail Delivery|TASMail SMTP Test|Failed to deliver|Re:|Fwd:/').first())
      .toBeVisible({ timeout: 25_000 });
    // Wait for the sidebar's real folder list to populate (folders query
    // can lag the messages query by one React tick — without this wait the
    // screenshot races and the sidebar shows only Compose / Calendar).
    await expect(page.locator('.px-2 > .group').first()).toBeVisible({ timeout: 10_000 });
    await takeScreenshot(page, 'modern-walkthrough/03-inbox-rendered');

    // ── 5. click first message → reader hydrates ────────────────────
    // EmailList renders each message as <div className="border-b ... cursor-pointer">.
    // The cursor-pointer Tailwind class is the most reliable shadcn-side hook.
    const firstRow = page.locator('div.cursor-pointer').first();
    await firstRow.waitFor({ state: 'visible', timeout: 15_000 });
    await firstRow.click();
    // Reader pane shows the message subject in <h2>; wait for it to be visible.
    // The "Select an email to read" placeholder lives in a different <div>;
    // once we click, the reader swaps and renders an h2 (loading state has
    // subject="(loading…)" then real subject).
    await expect(page.locator('h2', { hasText: /.+/ }).nth(1)).toBeVisible({ timeout: 15_000 });
    await page.waitForTimeout(2000);
    await takeScreenshot(page, 'modern-walkthrough/04-message-opened');

    // ── 6. switch to another folder (Sent Items / Junk / etc.) ──────
    // Pick the second non-INBOX folder shown in the sidebar.
    const altFolder = page.locator('button').filter({ hasText: /Sent|Junk|Drafts|Trash|Deleted/ }).first();
    if (await altFolder.isVisible().catch(() => false)) {
      await altFolder.click();
      await page.waitForTimeout(1500);
      await takeScreenshot(page, 'modern-walkthrough/05-other-folder');
    }

    // ── 7. open compose modal ───────────────────────────────────────
    await page.locator('button', { hasText: 'Compose' }).first().click();
    await expect(page.locator('text=New Message')).toBeVisible({ timeout: 5_000 });
    await page.locator('input[placeholder*="alice"]').first().fill('noreply@techatscale.io');
    await page.locator('input[placeholder="Subject"]').fill('Modern UI walk-through');
    // TMAIL-330: the composer body is now a TipTap ProseMirror editor — not a
    // <textarea>. Locate it via the wrapper test-id and type into the
    // contenteditable child rather than fill()ing a form control.
    const composeBody = page.locator('[data-testid="compose-rte-editor"]');
    await composeBody.click();
    await page.keyboard.type('Hello from the alt-UI');
    await takeScreenshot(page, 'modern-walkthrough/06-compose-filled');
    // TMAIL-238: click Save Draft (now wired) instead of Discard. Verify the
    // backend POST hit /api/drafts and produced a row in the Drafts folder.
    const draftsBefore = await fetch(`${baseURL}/api/folders/Drafts/messages?page=0&page_size=10`, { headers: auth });
    const draftsBeforeJson = draftsBefore.ok ? await draftsBefore.json() : { messages: [] };
    const beforeCount = (draftsBeforeJson.messages ?? []).length;
    await page.locator('button', { hasText: /Save Draft|Save/ }).first().click();
    // Modal closes on success.
    await expect(page.locator('text=New Message')).toBeHidden({ timeout: 8_000 });
    // Confirm a new draft row exists.
    let draftConfirmed = false;
    for (let attempt = 0; attempt < 5 && !draftConfirmed; attempt++) {
      await page.waitForTimeout(1200);
      const after = await fetch(`${baseURL}/api/folders/Drafts/messages?page=0&page_size=10`, { headers: auth });
      if (after.ok) {
        const afterJson = await after.json();
        if ((afterJson.messages ?? []).length > beforeCount) {
          draftConfirmed = true;
        }
      }
    }
    expect(draftConfirmed, 'draft was created on the live backend').toBe(true);
    await takeScreenshot(page, 'modern-walkthrough/06b-draft-saved');

    // ── 8. admin route ──────────────────────────────────────────────
    // Gear icon next to the inbox header opens admin via internal Link.
    const gear = page.locator('a[href*="admin"], button[title="Admin Dashboard"]').first();
    if (await gear.isVisible().catch(() => false)) {
      await gear.click();
      await page.waitForTimeout(1500);
      await takeScreenshot(page, 'modern-walkthrough/07-admin-route');
    }

    // ── 9. calendar route — sidebar entry, then create a real event ─
    // TMAIL-237: the sidebar Calendar link routes to /calendar inside the
    // hash router. Pull-down the New Event modal, fill it, save, and verify
    // the event reaches /api/calendar/events on the backend.
    await page.goto('/modern/index.html#/calendar');
    await page.waitForLoadState('domcontentloaded');
    await expect(page.locator('h2', { hasText: 'Calendar' })).toBeVisible({ timeout: 8_000 });
    await takeScreenshot(page, 'modern-walkthrough/08-calendar-route');
    const eventsBefore = await fetch(`${baseURL}/api/calendar/events`, { headers: auth });
    const eventsBeforeCount = eventsBefore.ok ? (await eventsBefore.json() as unknown[]).length : 0;
    await page.locator('button', { hasText: 'New Event' }).first().click();
    await page.locator('input[placeholder="Event title"]').fill('Walk-through smoke event');
    await page.locator('button', { hasText: 'Save Event' }).click();
    let eventCreated = false;
    for (let attempt = 0; attempt < 5 && !eventCreated; attempt++) {
      await page.waitForTimeout(1200);
      const after = await fetch(`${baseURL}/api/calendar/events`, { headers: auth });
      if (after.ok && (await after.json() as unknown[]).length > eventsBeforeCount) {
        eventCreated = true;
      }
    }
    expect(eventCreated, 'calendar event was created on the live backend').toBe(true);
    await takeScreenshot(page, 'modern-walkthrough/08b-calendar-event-created');

    // ── 10. back to classic via the ← Classic link ──────────────────
    // Need to be back on the email view (Navbar shows the Classic link there).
    await page.goto('/modern/index.html');
    await expect(page.locator('text=Classic')).toBeVisible({ timeout: 8_000 });
    const classicLink = page.locator('a', { hasText: /Classic/ }).first();
    await Promise.all([
      page.waitForURL(/\/app/i, { timeout: 15_000 }),
      classicLink.click(),
    ]);
    await expect(page.locator('button, a', { hasText: /Compose/i }).first()).toBeVisible({ timeout: 15_000 });
    await takeScreenshot(page, 'modern-walkthrough/09-back-to-classic');
  });
});
