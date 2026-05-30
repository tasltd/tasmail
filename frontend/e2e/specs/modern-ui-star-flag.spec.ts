/**
 * TMAIL-315: alt-UI ("modern") EmailList star button toggles the IMAP
 * \Flagged keyword via POST /api/folders/{folder}/messages/{uid}/flag.
 *
 * Coverage:
 *   1. Sign up + BYOK the noreply mailbox so the inbox has real envelopes
 *   2. Hop into /modern/ via the classic SPA's wand button
 *   3. Capture per-uid flags via GET /api/folders/INBOX/messages BEFORE the click
 *   4. Click the EmailList star button on the first envelope
 *   5. Capture flags AFTER — assert \Flagged was added (SPA E2E HARD RULE:
 *      validate mutation via API state before/after, not UI-only assertions)
 *   6. Click the same star again — assert \Flagged was removed (round-trip)
 *   7. Assert the button exposes aria-pressed so the toggle state is
 *      announced to assistive tech
 *
 * Screenshots: frontend/e2e/screenshots/star-flag/<step>.png
 *
 * Build prerequisite (handled by the auto-fix runner): `npm run build:alt-ui`
 * so the bundle in `frontend/public/modern/` reflects this commit.
 *
 * Runs against the live tunnel/proxy by default (see playwright.config.ts).
 */
import { expect, NOREPLY_CREDS, test } from '../fixtures/base.js';
import { deleteMailboxByUsername } from '../helpers/db-cleanup.js';

const SCREENSHOT_DIR = 'star-flag';
const PASSWORD = 'tmail-315-star-2026';

interface EnvelopeRow {
  uid: number;
  flags: string[];
}

function hasFlagged(flags: string[] | undefined): boolean {
  return (flags ?? []).some((f) => f.includes('Flagged'));
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

test.describe('TMAIL-315 alt-UI EmailList star button toggles \\Flagged', () => {
  test.beforeAll(() => deleteMailboxByUsername(NOREPLY_CREDS.email));
  test.afterAll(() => deleteMailboxByUsername(NOREPLY_CREDS.email));

  test('star button POSTs /flag and round-trips through the live IMAP', async ({
    page,
    apiSignup,
    takeScreenshot,
    baseURL,
  }) => {
    test.setTimeout(120_000);

    // ── 1. signup + BYOK so /api/folders/INBOX/messages has real rows ───
    const tokens = await apiSignup(NOREPLY_CREDS.email, PASSWORD);
    const auth = {
      Authorization: `Bearer ${tokens.access_token}`,
      'Content-Type': 'application/json',
    };
    const imapResp = await fetch(`${baseURL}/api/imap-configs`, {
      method: 'POST',
      headers: auth,
      body: JSON.stringify({
        name: 'tmail-315-imap',
        host: NOREPLY_CREDS.imap.host,
        port: NOREPLY_CREDS.imap.port,
        username: NOREPLY_CREDS.imap.username,
        password: NOREPLY_CREDS.imap.password,
        encryption: NOREPLY_CREDS.imap.encryption,
        is_default: true,
      }),
    });
    expect(imapResp.status, 'IMAP config').toBe(201);

    // ── 2. open classic /app and hop to /modern/ ────────────────────────
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

    // ── 3. snapshot per-uid flags from the live backend BEFORE clicking ─
    // (SPA E2E HARD RULE: capture API state before AND after the UI action.)
    const before = await fetchInbox(baseURL, auth);
    expect(before.length, 'inbox must contain at least one envelope').toBeGreaterThan(0);

    // EmailList renders rows in the same order as the API. The first
    // .cursor-pointer row corresponds to before[0], which is what we'll
    // click. Snapshot its starting flag state so we know whether the click
    // should add or remove \Flagged.
    const targetUid = before[0].uid;
    const startedStarred = hasFlagged(before[0].flags);

    // ── 4. click the star button on the first row ───────────────────────
    // The aria-label is the most stable hook — added by TMAIL-315.
    const firstRow = page.locator('div.cursor-pointer').first();
    const starButton = firstRow.locator('button[aria-label*="Star email from"], button[aria-label*="Unstar email from"]').first();
    await expect(starButton, 'star button is keyboard/AT discoverable').toBeVisible({
      timeout: 10_000,
    });
    // aria-pressed must reflect the starting flag state so screen readers
    // announce the toggle state correctly.
    await expect(starButton).toHaveAttribute('aria-pressed', String(startedStarred));
    await takeScreenshot(page, `${SCREENSHOT_DIR}/02-before-click`);
    await starButton.click();

    // ── 5. confirm the round-trip via the live backend ──────────────────
    // The IMAP STORE → FLAGS reply round-trip plus the TanStack invalidation
    // can take a beat. Poll for up to ~10s.
    let flippedOnce = false;
    let afterFirst: EnvelopeRow[] = [];
    for (let attempt = 0; attempt < 10 && !flippedOnce; attempt++) {
      await page.waitForTimeout(1000);
      afterFirst = await fetchInbox(baseURL, auth);
      const row = afterFirst.find((r) => r.uid === targetUid);
      if (row && hasFlagged(row.flags) !== startedStarred) {
        flippedOnce = true;
      }
    }
    expect(
      flippedOnce,
      `flag toggled on uid=${targetUid} (started starred=${startedStarred})`,
    ).toBe(true);
    await takeScreenshot(page, `${SCREENSHOT_DIR}/03-after-first-click`);

    // aria-pressed must now reflect the new state — read from the DOM, not
    // from our snapshot, so we're asserting what AT actually sees.
    await expect(starButton).toHaveAttribute(
      'aria-pressed',
      String(!startedStarred),
    );

    // ── 6. click again — assert the flag is removed (full round-trip) ───
    await starButton.click();
    let flippedBack = false;
    for (let attempt = 0; attempt < 10 && !flippedBack; attempt++) {
      await page.waitForTimeout(1000);
      const after = await fetchInbox(baseURL, auth);
      const row = after.find((r) => r.uid === targetUid);
      if (row && hasFlagged(row.flags) === startedStarred) {
        flippedBack = true;
      }
    }
    expect(
      flippedBack,
      `flag toggled back on uid=${targetUid} to starting state=${startedStarred}`,
    ).toBe(true);
    await takeScreenshot(page, `${SCREENSHOT_DIR}/04-after-second-click`);
    await expect(starButton).toHaveAttribute(
      'aria-pressed',
      String(startedStarred),
    );
  });
});
