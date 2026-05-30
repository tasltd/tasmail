/**
 * TMAIL-320: alt-UI ("modern") EmailReader Attachment Download button calls
 * GET /api/folders/{folder}/messages/{uid}/parts/{part_id} and triggers a
 * real browser download of the part's bytes.
 *
 * Sister-spec to modern-ui-reader-archive.spec.ts (TMAIL-317) and
 * modern-ui-reader-delete.spec.ts (TMAIL-318). Reuses the same noreply BYOK
 * signup → hop into /modern/ → open first envelope shape, with one
 * additional step before opening: we inject a multipart/mixed RFC822 message
 * that carries a known-bytes PDF attachment via POST /api/folders/INBOX/
 * import-eml. That guarantees the inbox has an attachment to click on
 * regardless of what real mail is in the upstream Stalwart mailbox.
 *
 * Coverage:
 *   1. Sign up + BYOK the noreply mailbox so /api/folders/INBOX/messages
 *      becomes reachable
 *   2. POST /api/folders/INBOX/import-eml with a multipart/mixed message that
 *      carries a known-bytes PDF attachment (`%PDF-1.4\n` after base64 decode)
 *   3. Hop into /modern/ via the classic SPA's wand button
 *   4. Find the seeded envelope in the inbox by subject and open it
 *   5. Snapshot the attachment's GET response (this is the source of truth
 *      for what we expect the browser download to contain — same SPA HARD
 *      RULE about API state validation, applied to the GET we're verifying)
 *   6. Click the Download button — Playwright captures the download event
 *      and we assert the saved file's bytes match the snapshot exactly
 *   7. Assert the Download button's aria-label spells out the filename for
 *      screen reader users
 *
 * Screenshots: frontend/e2e/screenshots/reader-download/<step>.png
 *
 * Build prerequisite (handled by the auto-fix runner): `npm run build:alt-ui`
 * so the bundle in `frontend/public/modern/` reflects this commit.
 *
 * Runs against the live tunnel/proxy by default (see playwright.config.ts).
 */
import { promises as fs } from 'node:fs';
import { expect, NOREPLY_CREDS, test } from '../fixtures/base.js';
import { deleteMailboxByUsername } from '../helpers/db-cleanup.js';

const SCREENSHOT_DIR = 'reader-download';
const PASSWORD = 'tmail-320-reader-download-2026';
const SUBJECT = `TMAIL-320 download fixture ${Date.now()}`;
// "%PDF-1.4\n" — minimal but valid PDF signature so the test asserts both
// the response Content-Type AND that the bytes are not mangled by transport.
const PDF_PLAINTEXT = '%PDF-1.4\n';
const PDF_FILENAME = 'tmail-320-fixture.pdf';

interface EnvelopeRow {
  uid: number;
  subject: string | null;
}

interface AttachmentRow {
  filename: string;
  content_type: string;
  size: number;
  part_id: string;
}

interface FullMessageRow {
  uid: number;
  subject: string | null;
  attachments: AttachmentRow[];
}

function buildEml(subject: string, base64Pdf: string): string {
  // Hand-crafted multipart/mixed with one text part and one base64 PDF
  // attachment. CRLF line endings are mandatory — IMAP APPEND will reject
  // bare LF on some servers and mailparse expects RFC822 CRLF too.
  return [
    `From: ${NOREPLY_CREDS.email}`,
    `To: ${NOREPLY_CREDS.email}`,
    `Subject: ${subject}`,
    `MIME-Version: 1.0`,
    `Content-Type: multipart/mixed; boundary="TMAIL320BOUNDARY"`,
    '',
    `--TMAIL320BOUNDARY`,
    `Content-Type: text/plain; charset="utf-8"`,
    `Content-Transfer-Encoding: 7bit`,
    '',
    `Fixture body for TMAIL-320 — see attached PDF.`,
    '',
    `--TMAIL320BOUNDARY`,
    `Content-Type: application/pdf; name="${PDF_FILENAME}"`,
    `Content-Disposition: attachment; filename="${PDF_FILENAME}"`,
    `Content-Transfer-Encoding: base64`,
    '',
    base64Pdf,
    '',
    `--TMAIL320BOUNDARY--`,
    '',
  ].join('\r\n');
}

async function fetchInbox(
  baseURL: string | undefined,
  auth: Record<string, string>,
): Promise<EnvelopeRow[]> {
  const resp = await fetch(`${baseURL}/api/folders/INBOX/messages?page=0&page_size=50`, {
    headers: auth,
  });
  if (!resp.ok) return [];
  const body = (await resp.json()) as { messages?: EnvelopeRow[] };
  return body.messages ?? [];
}

async function fetchFullMessage(
  baseURL: string | undefined,
  auth: Record<string, string>,
  uid: number,
): Promise<FullMessageRow> {
  const resp = await fetch(`${baseURL}/api/folders/INBOX/messages/${uid}`, {
    headers: auth,
  });
  expect(resp.ok, `GET /messages/${uid} must succeed`).toBe(true);
  return (await resp.json()) as FullMessageRow;
}

test.describe('TMAIL-320 alt-UI EmailReader attachment Download button', () => {
  test.beforeAll(() => deleteMailboxByUsername(NOREPLY_CREDS.email));
  test.afterAll(() => deleteMailboxByUsername(NOREPLY_CREDS.email));

  test('Download button fetches the MIME part bytes and saves them via blob URL', async ({
    page,
    apiSignup,
    takeScreenshot,
    baseURL,
  }) => {
    test.setTimeout(180_000);

    // ── 1. signup + BYOK so /api/folders/INBOX/messages becomes reachable ─
    const tokens = await apiSignup(NOREPLY_CREDS.email, PASSWORD);
    const auth = {
      Authorization: `Bearer ${tokens.access_token}`,
      'Content-Type': 'application/json',
    };
    const imapResp = await fetch(`${baseURL}/api/imap-configs`, {
      method: 'POST',
      headers: auth,
      body: JSON.stringify({
        name: 'tmail-320-imap',
        host: NOREPLY_CREDS.imap.host,
        port: NOREPLY_CREDS.imap.port,
        username: NOREPLY_CREDS.imap.username,
        password: NOREPLY_CREDS.imap.password,
        encryption: NOREPLY_CREDS.imap.encryption,
        is_default: true,
      }),
    });
    expect(imapResp.status, 'IMAP config').toBe(201);

    // ── 2. inject the fixture EML with a known PDF attachment ──────────────
    const base64Pdf = Buffer.from(PDF_PLAINTEXT, 'utf8').toString('base64');
    const eml = buildEml(SUBJECT, base64Pdf);
    const importResp = await fetch(`${baseURL}/api/folders/INBOX/import-eml`, {
      method: 'POST',
      headers: {
        Authorization: `Bearer ${tokens.access_token}`,
        'Content-Type': 'message/rfc822',
      },
      body: eml,
    });
    expect(importResp.status, 'EML import must succeed').toBe(201);

    // Wait for the fixture to show up in the inbox listing. The Stalwart
    // APPEND → list round-trip is usually instant but we poll for ~15s to
    // absorb any IMAP cache lag.
    let fixtureUid: number | undefined;
    for (let attempt = 0; attempt < 15 && fixtureUid == null; attempt++) {
      const rows = await fetchInbox(baseURL, auth);
      fixtureUid = rows.find((r) => (r.subject ?? '').includes(SUBJECT))?.uid;
      if (fixtureUid == null) await new Promise((r) => setTimeout(r, 1000));
    }
    expect(
      fixtureUid,
      `fixture envelope "${SUBJECT}" must appear in INBOX after import-eml`,
    ).toBeDefined();

    // SPA E2E HARD RULE: snapshot backend state *before* the UI action so we
    // can compare the browser download against it byte-for-byte.
    const full = await fetchFullMessage(baseURL, auth, fixtureUid!);
    const pdfAtt = full.attachments.find((a) => a.filename === PDF_FILENAME);
    expect(pdfAtt, `FullMessage.attachments must list ${PDF_FILENAME}`).toBeTruthy();
    expect(pdfAtt!.content_type).toBe('application/pdf');

    // Also pin the raw response from /parts/{part_id} so we can assert the
    // bytes the click handler is supposed to download.
    const partUrl = `${baseURL}/api/folders/INBOX/messages/${fixtureUid}/parts/${encodeURIComponent(pdfAtt!.part_id)}`;
    const partResp = await fetch(partUrl, {
      headers: { Authorization: `Bearer ${tokens.access_token}` },
    });
    expect(partResp.status, 'part download must succeed').toBe(200);
    expect(partResp.headers.get('content-type')).toContain('application/pdf');
    const expectedBytes = Buffer.from(await partResp.arrayBuffer());
    expect(expectedBytes.equals(Buffer.from(PDF_PLAINTEXT, 'utf8'))).toBe(true);

    // ── 3. open classic /app and hop to /modern/ ───────────────────────────
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
    await expect(page.locator('div.cursor-pointer').first()).toBeVisible({
      timeout: 25_000,
    });
    await takeScreenshot(page, `${SCREENSHOT_DIR}/01-inbox-rendered`);

    // ── 4. find and open the fixture row by subject ───────────────────────
    const fixtureRow = page
      .locator('div.cursor-pointer', { hasText: SUBJECT })
      .first();
    await expect(fixtureRow, `fixture row must be discoverable by subject`).toBeVisible({
      timeout: 25_000,
    });
    await fixtureRow.locator('.text-sm.truncate').first().click();
    const readerHeading = page.locator('h2.text-2xl').first();
    await expect(readerHeading).toContainText(SUBJECT, { timeout: 15_000 });
    await takeScreenshot(page, `${SCREENSHOT_DIR}/02-reader-opened`);

    // ── 5. assert the Download button is rendered for the PDF attachment ──
    // The button's accessible name is "Download attachment <filename>" — see
    // EmailReader.tsx where the aria-label is composed.
    const downloadButton = page.locator(
      `button[aria-label="Download attachment ${PDF_FILENAME}"]`,
    );
    await expect(
      downloadButton,
      'reader Download button is discoverable by filename',
    ).toBeVisible({ timeout: 10_000 });
    await takeScreenshot(page, `${SCREENSHOT_DIR}/03-before-click`);

    // ── 6. click — Playwright captures the blob-URL download ──────────────
    const [download] = await Promise.all([
      page.waitForEvent('download', { timeout: 20_000 }),
      downloadButton.click(),
    ]);
    expect(
      download.suggestedFilename(),
      'download filename must match the attachment filename',
    ).toBe(PDF_FILENAME);

    // Save the download into a temp file and compare bytes to the snapshot
    // taken before the UI action — full round-trip validation per the SPA
    // E2E HARD RULE (don't trust UI-only state changes).
    const savedPath = await download.path();
    expect(savedPath, 'Playwright must persist the download to disk').toBeTruthy();
    const savedBytes = await fs.readFile(savedPath);
    expect(
      savedBytes.equals(expectedBytes),
      `downloaded bytes (${savedBytes.length}) must equal the API snapshot (${expectedBytes.length})`,
    ).toBe(true);
    await takeScreenshot(page, `${SCREENSHOT_DIR}/04-after-click`);
  });
});
