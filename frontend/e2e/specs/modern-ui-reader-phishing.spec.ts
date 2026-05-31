/**
 * TMAIL-347: alt-UI ("modern") EmailReader phishing detection banner and
 * Mark-safe / Report / Dismiss actions.
 *
 * Coverage:
 *   1. Sign up + BYOK the noreply mailbox so the INBOX renders a real envelope
 *   2. Hop into /modern/ via the classic SPA's wand button
 *   3. Open the first envelope so EmailReader mounts
 *   4. Confirm the manual "Scan for phishing" button appears when no report exists
 *   5. Drive a scan directly via POST /api/folders/INBOX/messages/{uid}/phishing/scan
 *      with a synthetic html_body that contains an IP-URL link spoof. This
 *      forces the heuristic scanner to produce risk_score > 0 (independent of
 *      whatever real mail happens to be in the seeded inbox).
 *   6. Re-open the message in the modern UI — assert the banner renders with
 *      the expected severity class and risk-score text
 *   7. Click "Mark safe" — assert the PUT /api/phishing/{id}/action persisted
 *      `user_action='confirmed_safe'` via a fresh GET (SPA E2E HARD RULE:
 *      validate via API state before/after, not UI-only assertions)
 *   8. Confirm the banner disappears from the reader after the action
 *
 * Screenshots: frontend/e2e/screenshots/reader-phishing/<step>.png
 *
 * Build prerequisite (handled by the auto-fix runner): `npm run build:alt-ui`
 * so the bundle in `frontend/public/modern/` reflects this commit.
 */
import { expect, NOREPLY_CREDS, test } from '../fixtures/base.js';
import { deleteMailboxByUsername } from '../helpers/db-cleanup.js';

const SCREENSHOT_DIR = 'reader-phishing';
const PASSWORD = 'tmail-347-phishing-2026';

interface EnvelopeRow {
  uid: number;
  subject: string | null;
  from: string | null;
}

interface PhishingReport {
  id: string;
  message_uid: number;
  folder: string;
  risk_score: number;
  user_action: string;
  suspicious_links: Array<{ url: string; reasons: string[] }>;
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

async function fetchPhishingReport(
  baseURL: string | undefined,
  auth: Record<string, string>,
  uid: number,
): Promise<PhishingReport | null> {
  const resp = await fetch(
    `${baseURL}/api/folders/INBOX/messages/${uid}/phishing`,
    { headers: auth },
  );
  if (!resp.ok) return null;
  const txt = await resp.text();
  if (!txt) return null;
  return JSON.parse(txt) as PhishingReport;
}

test.describe('TMAIL-347 alt-UI EmailReader phishing banner + Report action', () => {
  test.beforeAll(() => deleteMailboxByUsername(NOREPLY_CREDS.email));
  test.afterAll(() => deleteMailboxByUsername(NOREPLY_CREDS.email));

  test('renders severity banner from a real /phishing/scan and persists user action', async ({
    page,
    apiSignup,
    takeScreenshot,
    baseURL,
  }) => {
    test.setTimeout(180_000);

    // ── 1. signup + BYOK so /api/folders/INBOX has at least one envelope ──
    const tokens = await apiSignup(NOREPLY_CREDS.email, PASSWORD);
    const auth = {
      Authorization: `Bearer ${tokens.access_token}`,
      'Content-Type': 'application/json',
    };
    const imapResp = await fetch(`${baseURL}/api/imap-configs`, {
      method: 'POST',
      headers: auth,
      body: JSON.stringify({
        name: 'tmail-347-imap',
        host: NOREPLY_CREDS.imap.host,
        port: NOREPLY_CREDS.imap.port,
        username: NOREPLY_CREDS.imap.username,
        password: NOREPLY_CREDS.imap.password,
        encryption: NOREPLY_CREDS.imap.encryption,
        is_default: true,
      }),
    });
    expect(imapResp.status, 'IMAP config').toBe(201);

    // ── 2. log in via localStorage + open classic /app then hop to /modern/
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

    // ── 3. pick a real uid and open the message ──────────────────────────
    const before = await fetchInbox(baseURL, auth);
    expect(before.length, 'inbox must contain at least one envelope').toBeGreaterThan(0);
    const targetUid = before[0].uid;

    const firstRow = page.locator('div.cursor-pointer').first();
    await firstRow.locator('.text-sm.truncate').first().click();
    await expect(page.locator('h2.text-2xl').first()).toBeVisible({ timeout: 15_000 });
    await takeScreenshot(page, `${SCREENSHOT_DIR}/02-reader-opened`);

    // ── 4. fresh state: no report yet → "Scan for phishing" button shown ─
    const initialReport = await fetchPhishingReport(baseURL, auth, targetUid);
    expect(initialReport, 'no phishing report should exist yet').toBeNull();
    await expect(
      page.locator('[data-testid="modern-phishing-scan"]'),
      'manual scan button must surface when no report exists',
    ).toBeVisible({ timeout: 5_000 });
    await takeScreenshot(page, `${SCREENSHOT_DIR}/03-scan-button-visible`);

    // ── 5. force a high-risk scan via direct POST. The heuristic scanner
    //      flags display-vs-href mismatches and IP-URL links — both conditions
    //      are present in this synthetic body, so risk_score will be > 0 even
    //      if the real seeded message body wouldn't trip the heuristic.
    const scanResp = await fetch(
      `${baseURL}/api/folders/INBOX/messages/${targetUid}/phishing/scan`,
      {
        method: 'POST',
        headers: auth,
        body: JSON.stringify({
          html_body:
            '<p>Dear customer, <a href="http://198.51.100.42/login">paypal.com</a> ' +
            'has detected unusual activity. <a href="http://203.0.113.7/verify">verify now</a>.</p>',
          sender_display_name: 'PayPal Security',
          sender_email: 'support@phishy-totally-not-paypal.example',
        }),
      },
    );
    expect(scanResp.status, 'phishing scan must return 201').toBe(201);
    const scanned = (await scanResp.json()) as PhishingReport;
    expect(scanned.risk_score, 'scanner must flag risk > 0').toBeGreaterThan(0);
    expect(scanned.user_action).toBe('none');

    // ── 6. re-render the reader so the new report flows through TanStack ──
    // The phishing query is keyed by ['phishing', folder, uid] and has a 60s
    // staleTime — easiest re-trigger is to navigate away and back into the
    // same message. We click back to the list (via the chevron / sidebar nav
    // isn't always visible at narrower viewports) by re-clicking another row
    // then ours. Simpler: reload the page — the modern UI uses hash-router
    // and AuthGate rehydrates from localStorage, so we land back on the
    // INBOX with the same env.
    await page.reload();
    await expect(page.locator('div.cursor-pointer').first()).toBeVisible({
      timeout: 25_000,
    });
    await page.locator('div.cursor-pointer').first().locator('.text-sm.truncate').first().click();
    await expect(page.locator('h2.text-2xl').first()).toBeVisible({ timeout: 15_000 });

    const banner = page.locator('[data-testid="modern-phishing-banner"]');
    await expect(banner, 'phishing banner must render after scan').toBeVisible({
      timeout: 15_000,
    });
    await expect(banner).toContainText(/Risk score:/);
    await expect(banner).toHaveAttribute(
      'data-severity',
      /(low|medium|high)/,
    );
    await takeScreenshot(page, `${SCREENSHOT_DIR}/04-banner-visible`);

    // ── 7. click "Mark safe" and assert backend state changed ────────────
    await page.locator('[data-testid="modern-phishing-mark-safe"]').click();

    let actionPersisted = false;
    for (let attempt = 0; attempt < 10 && !actionPersisted; attempt++) {
      await page.waitForTimeout(750);
      const report = await fetchPhishingReport(baseURL, auth, targetUid);
      if (report && report.user_action === 'confirmed_safe') {
        actionPersisted = true;
      }
    }
    expect(
      actionPersisted,
      'PUT /api/phishing/{id}/action must persist confirmed_safe',
    ).toBe(true);

    // ── 8. banner must disappear after the action (user_action != 'none') ─
    await expect(
      page.locator('[data-testid="modern-phishing-banner"]'),
      'banner hides once the user has acted',
    ).toBeHidden({ timeout: 10_000 });
    await takeScreenshot(page, `${SCREENSHOT_DIR}/05-banner-dismissed`);
  });
});
