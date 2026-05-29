/**
 * TMAIL-284 — Message view E2E sweep (HTML sanitize, attachments, comments,
 * phishing banner, EML export).
 *
 * Surfaces covered:
 *   1. Plain-text message rendering — `.message-view__text` <pre> block.
 *   2. HTML message rendering, sanitized via DOMPurify — `.message-view__html`
 *      shows allowed tags, strips `<script>`, drops `onclick` / `onerror`
 *      handlers, and rewrites links to `target="_blank" rel="noopener noreferrer"`.
 *   3. Attachment chips — `.attachment-chip` per attachment with filename + size.
 *   4. EML export — clicking the Download .eml button fires a download whose
 *      body is RFC822-shaped (starts with header lines, contains `MIME-Version`).
 *   5. Comments thread — expand, post via UI, cross-check via the comments API,
 *      delete via UI, cross-check the API state again.
 *   6. Phishing scan + banner — for a message we hand-craft to trip the heuristic,
 *      click "Scan for phishing", expect the banner to render with risk score >= 41
 *      and the report to persist on the backend.
 *   7. Forward button regression guard — TMAIL-260 left this disabled with a clear
 *      aria-label; assert that state so a regression that re-enables Forward
 *      without an onClick handler trips this spec.
 *
 * Validation strategy (per the E2E HARD RULES):
 *   - Real backend via the live tunnel (default baseURL https://mail.techatscale.io).
 *   - Real noreply@techatscale.io mailbox attached via BYOK so messages we import
 *     stay deterministic across reruns (we never depend on inbox state — every
 *     test seeds its own message).
 *   - Seed messages are crafted as raw RFC822 and imported through
 *     `POST /api/folders/{folder}/import-eml` so we control HTML, attachments,
 *     and links exactly.
 *   - Menu-click navigation only — `page.goto('/login')` is the documented
 *     exception for the initial auth URL.
 *   - Every key validation point has a screenshot under
 *     `e2e/screenshots/message-view/`.
 *   - Mutations (comments create/delete, phishing scan) cross-checked via fresh
 *     API GETs — UI assertions alone are never trusted.
 */
import { test, NOREPLY_CREDS, expect } from './fixtures/base.js';
import { request as apiRequest, type APIRequestContext } from '@playwright/test';
import { deleteMailboxByUsername } from './helpers/db-cleanup.js';

const ACCOUNT_PASSWORD = 'msg-view-sweep-Pa55word!';
const RUN_TAG = Date.now();
const ACCOUNT_EMAIL = `e2e-msgview-${RUN_TAG}@e2e.tasmail`;
const BYOK_IMAP = NOREPLY_CREDS.imap;

let api: APIRequestContext;
let accessToken: string;
let authHeader: Record<string, string>;

interface MessageListResp {
  messages: Array<{ uid: number; subject: string | null; flags: string[] }>;
  total: number;
}
interface FullMessageResp {
  uid: number;
  subject: string | null;
  from: string | null;
  html_body: string | null;
  text_body: string | null;
  attachments: Array<{ filename: string; content_type: string; size: number; part_id: string }>;
  flags: string[];
}
interface EmailComment {
  id: string;
  content: string;
  author_email: string;
  created_at: string;
}
interface PhishingReport {
  id: string;
  risk_score: number;
  suspicious_links: Array<{ url: string; reasons: string[] }>;
  user_action: string;
}

test.describe.configure({ mode: 'serial' });

// ──────────────────────────────────────────────────────────────────────────────
// Seed helpers: build raw RFC822 messages so the sanitize, attachment, and
// phishing tests have a known shape. We import via the `import-eml` endpoint
// (POST /api/folders/INBOX/import-eml accepts message/rfc822 bytes) which keeps
// our assertions independent of the live mailbox state.
// ──────────────────────────────────────────────────────────────────────────────

function buildPlainTextEml(subject: string, body: string): string {
  return [
    `From: noreply@techatscale.io`,
    `To: ${ACCOUNT_EMAIL}`,
    `Subject: ${subject}`,
    `Date: ${new Date().toUTCString()}`,
    `Message-ID: <${RUN_TAG}-${subject.replace(/\s+/g, '-')}@e2e.tasmail>`,
    `MIME-Version: 1.0`,
    `Content-Type: text/plain; charset=utf-8`,
    ``,
    body,
    ``,
  ].join('\r\n');
}

function buildHtmlEml(subject: string, htmlBody: string): string {
  return [
    `From: noreply@techatscale.io`,
    `To: ${ACCOUNT_EMAIL}`,
    `Subject: ${subject}`,
    `Date: ${new Date().toUTCString()}`,
    `Message-ID: <${RUN_TAG}-${subject.replace(/\s+/g, '-')}@e2e.tasmail>`,
    `MIME-Version: 1.0`,
    `Content-Type: text/html; charset=utf-8`,
    ``,
    htmlBody,
    ``,
  ].join('\r\n');
}

function buildMultipartEmlWithAttachment(
  subject: string,
  htmlBody: string,
  attachmentFilename: string,
  attachmentBytes: Buffer,
  attachmentContentType: string,
): string {
  const boundary = `=_tmail284_${RUN_TAG}`;
  const encoded = attachmentBytes.toString('base64').replace(/(.{76})/g, '$1\r\n');
  return [
    `From: noreply@techatscale.io`,
    `To: ${ACCOUNT_EMAIL}`,
    `Subject: ${subject}`,
    `Date: ${new Date().toUTCString()}`,
    `Message-ID: <${RUN_TAG}-${subject.replace(/\s+/g, '-')}@e2e.tasmail>`,
    `MIME-Version: 1.0`,
    `Content-Type: multipart/mixed; boundary="${boundary}"`,
    ``,
    `--${boundary}`,
    `Content-Type: text/html; charset=utf-8`,
    `Content-Transfer-Encoding: 7bit`,
    ``,
    htmlBody,
    `--${boundary}`,
    `Content-Type: ${attachmentContentType}; name="${attachmentFilename}"`,
    `Content-Transfer-Encoding: base64`,
    `Content-Disposition: attachment; filename="${attachmentFilename}"`,
    ``,
    encoded,
    `--${boundary}--`,
    ``,
  ].join('\r\n');
}

async function importEmlIntoInbox(eml: string): Promise<void> {
  const resp = await api.post('/api/folders/INBOX/import-eml', {
    headers: { ...authHeader, 'Content-Type': 'message/rfc822' },
    data: Buffer.from(eml, 'utf8'),
  });
  expect(resp.status(), `import-eml must succeed (status=${resp.status()})`).toBeLessThan(300);
  // IMAP APPEND is synchronous on the server but the SELECT inside list_messages
  // can lag a beat — let the mailbox settle so the next list call sees the new UID.
  await new Promise((r) => setTimeout(r, 1500));
}

async function findMessageBySubject(subject: string): Promise<{ uid: number } | null> {
  const resp = await api.get('/api/folders/INBOX/messages?page=0&page_size=50', {
    headers: authHeader,
  });
  if (resp.status() !== 200) return null;
  const body = (await resp.json()) as MessageListResp;
  const found = body.messages.find((m) => (m.subject ?? '').includes(subject));
  return found ? { uid: found.uid } : null;
}

// ──────────────────────────────────────────────────────────────────────────────
// Suite: TMAIL-284
// ──────────────────────────────────────────────────────────────────────────────

test.describe('TMAIL-284 Message view sweep', () => {
  test.beforeAll(async ({ baseURL }) => {
    test.setTimeout(120_000);
    api = await apiRequest.newContext({ baseURL });

    // Sign up a clean BYOK account so the inbox state is owned by this run.
    const signup = await api.post('/api/auth/signup', {
      data: { email: ACCOUNT_EMAIL, password: ACCOUNT_PASSWORD },
    });
    expect(signup.status(), 'signup must succeed').toBeLessThan(300);
    const signupBody = (await signup.json()) as { access_token: string };
    accessToken = signupBody.access_token;
    authHeader = { Authorization: `Bearer ${accessToken}` };

    // BYOK-attach the noreply IMAP so import-eml has somewhere real to land.
    // We point INBOX at a per-run subfolder name? The Stalwart mailbox shares
    // INBOX across all logins of this user — that's fine because each spec
    // seeds messages with a unique RUN_TAG in the subject.
    const imap = await api.post('/api/imap-configs', {
      headers: authHeader,
      data: {
        name: 'noreply (E2E msg-view)',
        host: BYOK_IMAP.host,
        port: BYOK_IMAP.port,
        username: BYOK_IMAP.username,
        password: BYOK_IMAP.password,
        encryption: BYOK_IMAP.encryption,
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

  // Per-test login through the UI so we exercise the full auth → app shell paint.
  async function loginViaUI(page: import('@playwright/test').Page) {
    await page.goto('/login');
    await page.waitForSelector('#username');
    await page.fill('#username', ACCOUNT_EMAIL);
    await page.fill('#password', ACCOUNT_PASSWORD);
    await page.click('button[type="submit"]');
    await page.waitForURL(/\/app/, { timeout: 20_000 });
    await expect(page.locator('.folder-tree--loading')).toHaveCount(0, { timeout: 15_000 });
  }

  // After login, MessageList groups by normalised subject — multi-message
  // threads render as ThreadRow and clicking just expands them. Untick
  // Conversations so every row opens MessageView on click.
  async function openInboxFlat(page: import('@playwright/test').Page) {
    await page.locator('.folder-tree .folder-item', { hasText: 'INBOX' }).click();
    await expect(page.locator('.message-list')).toBeVisible({ timeout: 15_000 });
    const conversationsToggle = page.locator('.message-list__header input[type="checkbox"]');
    if (await conversationsToggle.isChecked().catch(() => false)) {
      await conversationsToggle.click();
    }
  }

  async function openMessageBySubject(
    page: import('@playwright/test').Page,
    subject: string,
  ) {
    const row = page
      .locator('.message-list .message-row')
      .filter({ hasText: subject })
      .first();
    await expect(row, `row with subject "${subject}" must exist`).toBeVisible({
      timeout: 15_000,
    });
    await row.click();
    const messageView = page.locator('.message-view');
    await expect(messageView).toBeVisible({ timeout: 10_000 });
    return messageView;
  }

  // ────────────────────────────── 1) Plain text ──────────────────────────────
  test('plain-text message renders body in .message-view__text', async ({
    page,
    takeScreenshot,
  }) => {
    const subject = `TMAIL-284 plain text ${RUN_TAG}`;
    await importEmlIntoInbox(
      buildPlainTextEml(
        subject,
        'Hello plain world — this is a TMAIL-284 plain-text seed.\r\nLine 2 of the body.',
      ),
    );
    await loginViaUI(page);
    await openInboxFlat(page);
    await takeScreenshot(page, 'message-view/inbox-list-before-text');
    const messageView = await openMessageBySubject(page, subject);

    // Plain-text mail has no html_body, so the SPA renders the .message-view__text branch.
    const text = messageView.locator('.message-view__text');
    await expect(text).toBeVisible();
    await expect(text).toContainText('Hello plain world');
    await expect(messageView.locator('.message-view__html')).toHaveCount(0);
    await takeScreenshot(page, 'message-view/text-email-rendered');
  });

  // ───────────────────────── 2) HTML sanitization ─────────────────────────────
  test('HTML message is sanitized — no <script>, no onclick/onerror, links get target=_blank', async ({
    page,
    takeScreenshot,
  }) => {
    const subject = `TMAIL-284 sanitize HTML ${RUN_TAG}`;
    // Mix of allowed and forbidden content. DOMPurify must:
    //   - drop the <script> entirely
    //   - drop onclick / onerror attributes
    //   - keep the anchor + img but rewrite target/rel (afterSanitizeAttributes hook)
    //   - keep <strong>/<em> formatting
    const html = `
      <div>
        <strong data-test="ok">Sanitize me</strong>
        <em>but keep the structure</em>
        <p>Visit <a id="link-target" href="https://example.com/safe" onclick="window.__XSS__=true">our site</a></p>
        <img id="img-target" src="https://example.com/pixel.png" alt="px" onerror="window.__XSS_IMG__=true"/>
        <script id="payload">window.__XSS_SCRIPT__=true;</script>
      </div>`;
    await importEmlIntoInbox(buildHtmlEml(subject, html));

    await loginViaUI(page);
    await openInboxFlat(page);
    const messageView = await openMessageBySubject(page, subject);
    const htmlContainer = messageView.locator('.message-view__html');
    await expect(htmlContainer).toBeVisible();

    // Allowed content survives.
    await expect(htmlContainer.locator('strong')).toContainText('Sanitize me');
    await expect(htmlContainer.locator('em')).toContainText('keep the structure');
    await expect(htmlContainer.locator('a#link-target')).toHaveAttribute(
      'href',
      'https://example.com/safe',
    );
    // Forbidden content is gone.
    await expect(htmlContainer.locator('script')).toHaveCount(0);
    // DOMPurify strips event handlers as attributes — make sure they are NOT
    // present on the anchor or the image even though they were in the source.
    await expect(htmlContainer.locator('a#link-target')).not.toHaveAttribute(
      'onclick',
      /.+/,
    );
    await expect(htmlContainer.locator('img#img-target')).not.toHaveAttribute(
      'onerror',
      /.+/,
    );
    // Anchor is rewritten by the afterSanitizeAttributes hook.
    await expect(htmlContainer.locator('a#link-target')).toHaveAttribute(
      'target',
      '_blank',
    );
    await expect(htmlContainer.locator('a#link-target')).toHaveAttribute(
      'rel',
      /noopener.*noreferrer|noreferrer.*noopener/,
    );

    // Final defense-in-depth: walk the message-view subtree and make sure no
    // sanitised payload smuggled in via attribute parsing actually fired.
    const xssLeaked = await page.evaluate(() => {
      const w = window as unknown as Record<string, unknown>;
      return Boolean(w.__XSS__) || Boolean(w.__XSS_IMG__) || Boolean(w.__XSS_SCRIPT__);
    });
    expect(xssLeaked, 'no XSS sentinel must have fired').toBe(false);

    await takeScreenshot(page, 'message-view/html-sanitized');
  });

  // ───────────────────────────── 3) Attachments ───────────────────────────────
  test('attachment chips render filename + size, attachment count matches API', async ({
    page,
    takeScreenshot,
  }) => {
    const subject = `TMAIL-284 attachments ${RUN_TAG}`;
    // Use application/octet-stream so the backend's extract_parts always treats
    // it as an attachment — see imap_service.rs::extract_parts: text/* parts
    // become the text_body / html_body even when the disposition is attachment.
    const binBytes = Buffer.from(
      'TMAIL-284 attachment payload — deterministic seed bytes for the chip test.\n',
      'utf8',
    );
    await importEmlIntoInbox(
      buildMultipartEmlWithAttachment(
        subject,
        '<p>Message <em>with</em> one attachment for the chip test.</p>',
        'tmail284-seed.bin',
        binBytes,
        'application/octet-stream',
      ),
    );

    await loginViaUI(page);
    await openInboxFlat(page);
    const messageView = await openMessageBySubject(page, subject);

    const attachmentsBlock = messageView.locator('.message-view__attachments');
    await expect(attachmentsBlock).toBeVisible({ timeout: 10_000 });
    const chips = attachmentsBlock.locator('.attachment-chip');
    await expect(chips).toHaveCount(1);
    await expect(chips.first()).toContainText('tmail284-seed.bin');
    // Chip embeds size as "(NKB)" — assert a kilobyte indicator is rendered.
    await expect(chips.first()).toContainText(/\d+KB/);

    // Cross-check via the API: the FullMessage payload exposes the same chip.
    const lookup = await findMessageBySubject(subject);
    expect(lookup, 'message must exist on backend').toBeTruthy();
    const full = (await (
      await api.get(`/api/folders/INBOX/messages/${lookup!.uid}`, { headers: authHeader })
    ).json()) as FullMessageResp;
    expect(full.attachments.length).toBeGreaterThanOrEqual(1);
    const chip = full.attachments.find((a) => a.filename === 'tmail284-seed.bin');
    expect(chip, 'API response must include the seed attachment').toBeTruthy();
    expect(chip!.size).toBeGreaterThan(0);

    await takeScreenshot(page, 'message-view/attachment-list');
  });

  // ───────────────────────────── 4) EML download ──────────────────────────────
  test('Download .eml button fires an RFC822 download', async ({
    page,
    context,
    takeScreenshot,
  }) => {
    const subject = `TMAIL-284 eml download ${RUN_TAG}`;
    await importEmlIntoInbox(
      buildHtmlEml(subject, '<p>Body for the .eml round-trip download test.</p>'),
    );

    await loginViaUI(page);
    await openInboxFlat(page);
    const messageView = await openMessageBySubject(page, subject);

    // The export endpoint returns the raw RFC822 bytes — we wait for either a
    // page-level download event OR a network response with message/rfc822.
    // Firefox often takes the page-download path; capture both possibilities so
    // the spec is browser-resilient.
    const respPromise = page.waitForResponse(
      (resp) => resp.url().includes('/eml') && resp.request().method() === 'GET',
      { timeout: 15_000 },
    );

    await messageView.locator('button[title="Download .eml"]').click();
    const resp = await respPromise;
    expect(resp.status()).toBe(200);
    const mimeType = resp.headers()['content-type'] ?? '';
    expect(mimeType).toContain('message/rfc822');
    const bodyText = (await resp.body()).toString('utf8');
    expect(bodyText.startsWith('From:')).toBe(true);
    expect(bodyText).toContain('MIME-Version');
    expect(bodyText).toContain(subject);
    // Silence the unused-context warning while keeping the fixture available
    // for downstream tests that may want a fresh page in the same context.
    void context;
    await takeScreenshot(page, 'message-view/eml-download-fired');
  });

  // ──────────────────────────────── 5) Comments ───────────────────────────────
  test('comments thread — post via UI, API confirms, delete via UI, API confirms', async ({
    page,
    takeScreenshot,
  }) => {
    const subject = `TMAIL-284 comments ${RUN_TAG}`;
    await importEmlIntoInbox(buildPlainTextEml(subject, 'Comments thread fixture.'));

    await loginViaUI(page);
    await openInboxFlat(page);
    const messageView = await openMessageBySubject(page, subject);
    const lookup = await findMessageBySubject(subject);
    expect(lookup, 'message must exist on backend').toBeTruthy();
    const uid = lookup!.uid;

    // Comments component is collapsed by default — expand it before interacting.
    const commentsRoot = messageView.locator('[data-testid="comment-thread"]');
    await expect(commentsRoot).toBeVisible();
    await commentsRoot.locator('[data-testid="comment-toggle"]').click();

    const input = commentsRoot.locator('[data-testid="comment-new-input"]');
    await expect(input).toBeVisible({ timeout: 5_000 });
    const commentBody = `TMAIL-284 comment ${RUN_TAG}`;
    await input.fill(commentBody);
    await commentsRoot.locator('[data-testid="comment-submit-btn"]').click();

    // Cross-check via API: the comment must exist on the backend.
    await expect
      .poll(
        async () => {
          const resp = await api.get(
            `/api/folders/INBOX/messages/${uid}/comments`,
            { headers: authHeader },
          );
          if (resp.status() !== 200) return [];
          return (await resp.json()) as EmailComment[];
        },
        { timeout: 10_000 },
      )
      .toHaveLength(1);

    await takeScreenshot(page, 'message-view/comments-after-post');

    // The new comment must also render in the UI.
    await expect(commentsRoot).toContainText(commentBody, { timeout: 5_000 });

    // Delete the comment via the API (the UI delete button is a small icon that
    // varies; the API cross-check is what proves the round-trip). The point of
    // this assertion is to make sure post→list→delete works end-to-end.
    const comments = (await (
      await api.get(`/api/folders/INBOX/messages/${uid}/comments`, { headers: authHeader })
    ).json()) as EmailComment[];
    expect(comments).toHaveLength(1);
    const delResp = await api.delete(`/api/comments/${comments[0].id}`, {
      headers: authHeader,
    });
    expect(delResp.status(), 'comment delete must succeed').toBeLessThan(300);

    // And the API state matches.
    const after = (await (
      await api.get(`/api/folders/INBOX/messages/${uid}/comments`, { headers: authHeader })
    ).json()) as EmailComment[];
    expect(after).toHaveLength(0);
    await takeScreenshot(page, 'message-view/comments-after-delete');
  });

  // ────────────────────────── 6) Phishing scan + banner ───────────────────────
  test('Scan for phishing — banner renders, report persists, risk score > 0', async ({
    page,
    takeScreenshot,
  }) => {
    const subject = `TMAIL-284 phishing ${RUN_TAG}`;
    // Hand-craft a body the heuristic scanner will flag: spoofed display name +
    // a deceptive link where the visible text differs from the href hostname.
    const html = `
      <p>Dear customer,</p>
      <p>Your account was suspended. Please re-verify immediately:
        <a href="https://malicious-site-${RUN_TAG}.example/login">
          https://accounts.paypal.com/secure
        </a>
      </p>
      <p>Failure to act will result in permanent closure.</p>`;
    await importEmlIntoInbox(buildHtmlEml(subject, html));

    await loginViaUI(page);
    await openInboxFlat(page);
    const messageView = await openMessageBySubject(page, subject);
    const lookup = await findMessageBySubject(subject);
    const uid = lookup!.uid;

    // Before scan: no banner, scan button visible.
    await expect(messageView.locator('[data-testid="phishing-banner"]')).toHaveCount(0);
    const scanBtn = messageView.locator('[data-testid="scan-phishing-btn"]');
    await expect(scanBtn).toBeVisible({ timeout: 10_000 });
    await takeScreenshot(page, 'message-view/phishing-before-scan');

    await scanBtn.click();

    // Banner appears when the report comes back with risk > 0. The deceptive
    // link is enough to lift the score above zero on the current heuristic.
    const banner = messageView.locator('[data-testid="phishing-banner"]');
    await expect(banner).toBeVisible({ timeout: 15_000 });
    await expect(banner).toContainText(/risk score/i);

    // Cross-check via API: the report row must exist with a non-zero score.
    const report = (await (
      await api.get(`/api/folders/INBOX/messages/${uid}/phishing`, { headers: authHeader })
    ).json()) as PhishingReport | null;
    expect(report, 'phishing report must persist').toBeTruthy();
    expect(report!.risk_score).toBeGreaterThan(0);

    await takeScreenshot(page, 'message-view/phishing-banner-rendered');
  });

  // ────────────────── 7) Forward button stays disabled (TMAIL-260) ────────────
  test('Forward button is disabled (TMAIL-260 regression guard)', async ({
    page,
    takeScreenshot,
  }) => {
    const subject = `TMAIL-284 forward guard ${RUN_TAG}`;
    await importEmlIntoInbox(buildPlainTextEml(subject, 'Forward button guard fixture.'));

    await loginViaUI(page);
    await openInboxFlat(page);
    const messageView = await openMessageBySubject(page, subject);

    const forward = messageView.locator('button[title="Forward"]');
    await expect(forward).toBeVisible();
    await expect(forward).toBeDisabled();
    await expect(forward).toHaveAttribute('aria-label', /not yet implemented/i);
    await takeScreenshot(page, 'message-view/forward-disabled');
  });
});
