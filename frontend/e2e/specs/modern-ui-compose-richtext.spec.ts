/**
 * TMAIL-330: alt-UI ("modern") ComposeModal must back its body with a real
 * rich-text editor (TipTap) and send html_body alongside text_body.
 *
 * Coverage:
 *   1. Sign up + BYOK the noreply mailbox so /api/drafts has a live IMAP
 *      backend to append to.
 *   2. Hop into /modern/ via the classic SPA's wand button.
 *   3. Open the Compose modal, type into the TipTap editor, then exercise
 *      each toolbar button (Bold, Italic, Link, Bullet list).
 *   4. Save Draft → wait for the modal to close → re-fetch the Drafts
 *      folder and pull the newest draft's full message.
 *   5. Assert the persisted draft's html_body contains real HTML markup
 *      (`<strong>`, `<em>`, `<a href="...">`, `<ul>`/`<li>`) — proves the
 *      toolbar buttons are wired to the editor and the body is being sent
 *      as `html_body` rather than just `text_body`.
 *
 * Screenshots: frontend/e2e/screenshots/compose-richtext/<step>.png
 *
 * Build prerequisite: `npm run build:alt-ui` so the bundle in
 * `frontend/public/modern/` reflects this commit.
 *
 * Runs against the live tunnel/proxy by default (see playwright.config.ts).
 */
import { expect, NOREPLY_CREDS, test } from '../fixtures/base.js';
import { deleteMailboxByUsername } from '../helpers/db-cleanup.js';

const SCREENSHOT_DIR = 'compose-richtext';
const PASSWORD = 'tmail-330-richtext-2026';

interface DraftEnvelope {
  uid: number;
  subject: string | null;
}

interface FullMessage {
  uid: number;
  subject: string | null;
  html_body: string | null;
  text_body: string | null;
}

test.describe('TMAIL-330 alt-UI Compose body is a real rich-text editor and sends html_body', () => {
  test.beforeAll(() => deleteMailboxByUsername(NOREPLY_CREDS.email));
  test.afterAll(() => deleteMailboxByUsername(NOREPLY_CREDS.email));

  test('typing + toolbar clicks produce a draft with HTML markup', async ({
    page,
    apiSignup,
    takeScreenshot,
    baseURL,
  }) => {
    test.setTimeout(150_000);

    // ── 1. signup + BYOK so /api/drafts has a real IMAP backend ─────────
    const tokens = await apiSignup(NOREPLY_CREDS.email, PASSWORD);
    const auth = {
      Authorization: `Bearer ${tokens.access_token}`,
      'Content-Type': 'application/json',
    };
    const imapResp = await fetch(`${baseURL}/api/imap-configs`, {
      method: 'POST',
      headers: auth,
      body: JSON.stringify({
        name: 'tmail-330-imap',
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
        name: 'tmail-330-smtp',
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

    // ── 2. classic /app then hop into /modern/ ──────────────────────────
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
    await takeScreenshot(page, `${SCREENSHOT_DIR}/01-modern-ui-loaded`);

    // ── 3. capture Drafts baseline ──────────────────────────────────────
    // SPA E2E HARD RULE: snapshot the resource via API before AND after
    // the UI action so we can prove the backend actually changed.
    const draftsBeforeResp = await fetch(
      `${baseURL}/api/folders/Drafts/messages?page=0&page_size=20`,
      { headers: auth },
    );
    const draftsBefore = draftsBeforeResp.ok
      ? ((await draftsBeforeResp.json()) as { messages?: DraftEnvelope[] })
      : { messages: [] };
    const beforeUids = new Set((draftsBefore.messages ?? []).map((m) => m.uid));

    // ── 4. open the compose modal and exercise the rich-text editor ─────
    await page.locator('button', { hasText: 'Compose' }).first().click();
    await expect(page.locator('text=New Message')).toBeVisible({
      timeout: 5_000,
    });
    await page
      .locator('input[placeholder*="alice"]')
      .first()
      .fill(NOREPLY_CREDS.email);
    const SUBJECT = `TMAIL-330 rich text ${Date.now()}`;
    await page.locator('input[placeholder="Subject"]').fill(SUBJECT);
    await takeScreenshot(page, `${SCREENSHOT_DIR}/02-compose-empty`);

    // The editor body is the contenteditable rendered by EditorContent —
    // located via the test-id baked into editorProps.attributes.
    const editor = page.locator('[data-testid="compose-rte-editor"]');
    await expect(editor).toBeVisible({ timeout: 10_000 });
    await editor.click();

    // Bold segment.
    await page.locator('[data-testid="compose-rte-bold"]').click();
    await page.keyboard.type('Bolded ');
    await page.locator('[data-testid="compose-rte-bold"]').click();

    // Italic segment.
    await page.locator('[data-testid="compose-rte-italic"]').click();
    await page.keyboard.type('italics');
    await page.locator('[data-testid="compose-rte-italic"]').click();

    // Trailing space, then a link (uses the native prompt — intercept it).
    await page.keyboard.type(' ');

    const LINK_HREF = 'https://example.com/tmail-330';

    // Stub window.prompt BEFORE clicking so the URL prompt resolves
    // synchronously with our test URL.
    await page.evaluate((href) => {
      // eslint-disable-next-line @typescript-eslint/no-explicit-any
      (window as any).prompt = () => href;
    }, LINK_HREF);
    await page.locator('[data-testid="compose-rte-link"]').click();
    // With no selection the toolbar inserts the URL itself as both href and
    // display text (mirrors Gmail / Outlook behaviour). The cursor lands
    // after the </a>, so the next keystrokes land in plain text.
    await page.keyboard.type(' suffix');

    // New line then bullet list.
    await page.keyboard.press('Enter');
    await page.locator('[data-testid="compose-rte-bullet-list"]').click();
    await page.keyboard.type('first item');
    await page.keyboard.press('Enter');
    await page.keyboard.type('second item');

    await takeScreenshot(page, `${SCREENSHOT_DIR}/03-compose-formatted`);

    // ── 5. save draft and wait for the modal to close ───────────────────
    await page.locator('button', { hasText: /Save Draft|Save/ }).first().click();
    await expect(page.locator('text=New Message')).toBeHidden({
      timeout: 10_000,
    });
    await takeScreenshot(page, `${SCREENSHOT_DIR}/04-compose-saved`);

    // ── 6. confirm a new Drafts row exists and pull its html_body ───────
    let newDraft: DraftEnvelope | null = null;
    for (let attempt = 0; attempt < 8 && newDraft === null; attempt++) {
      await page.waitForTimeout(1500);
      const after = await fetch(
        `${baseURL}/api/folders/Drafts/messages?page=0&page_size=20`,
        { headers: auth },
      );
      if (!after.ok) continue;
      const body = (await after.json()) as { messages?: DraftEnvelope[] };
      for (const row of body.messages ?? []) {
        if (!beforeUids.has(row.uid) && (row.subject ?? '').includes(SUBJECT)) {
          newDraft = row;
          break;
        }
      }
    }
    expect(newDraft, 'new draft was appended to the Drafts folder').not.toBeNull();

    const fullResp = await fetch(
      `${baseURL}/api/folders/Drafts/messages/${newDraft!.uid}`,
      { headers: auth },
    );
    expect(fullResp.status, 'full draft fetch').toBe(200);
    const full = (await fullResp.json()) as FullMessage;

    // The backend stores BOTH html_body and text_body — exactly the round-trip
    // TMAIL-330 was filed about. The plain-text body alone is not enough.
    expect(full.html_body, 'draft has an html_body part').toBeTruthy();
    const html = full.html_body ?? '';

    // Each assertion below maps to one toolbar button — failing any one of
    // them means that button is still dead.
    expect(html, 'Bold button produced a <strong>').toMatch(/<strong>[^<]*Bolded[^<]*<\/strong>/i);
    expect(html, 'Italic button produced an <em>').toMatch(/<em>[^<]*italics[^<]*<\/em>/i);
    expect(html, 'Link button produced an <a href="…">').toMatch(
      new RegExp(`<a [^>]*href="${LINK_HREF.replace(/[-/\\^$*+?.()|[\]{}]/g, '\\$&')}"[^>]*>[^<]*</a>`, 'i'),
    );
    expect(html, 'Bullet list button produced a <ul><li>').toMatch(/<ul>[\s\S]*<li>[\s\S]*first item[\s\S]*<\/li>[\s\S]*<\/ul>/i);

    // Plain-text fallback should still carry the same content as a stripped
    // version — proves we are sending text_body alongside, not instead of.
    expect(full.text_body ?? '', 'text_body fallback non-empty').toContain('Bolded');
    expect(full.text_body ?? '', 'text_body fallback non-empty').toContain('italics');
  });
});
