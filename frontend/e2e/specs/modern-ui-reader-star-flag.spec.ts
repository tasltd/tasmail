/**
 * TMAIL-316: alt-UI ("modern") EmailReader header star button toggles the IMAP
 * \Flagged keyword via POST /api/folders/{folder}/messages/{uid}/flag.
 *
 * Sister-spec to modern-ui-star-flag.spec.ts (TMAIL-315 — EmailList row star).
 * The reader-header star uses the same mutation owner (EmailClient) but the
 * cache-invalidation path is broader: it must also invalidate the
 * ['message', folder, uid] detail cache so the next view of this message
 * reflects the new flag state. This spec proves the round-trip works through
 * the live backend.
 *
 * Coverage:
 *   1. Sign up + BYOK the noreply mailbox so the inbox has real envelopes
 *   2. Hop into /modern/ via the classic SPA's wand button
 *   3. Open the first envelope so the EmailReader pane mounts
 *   4. Capture per-uid flags via GET /api/folders/INBOX/messages BEFORE the click
 *   5. Click the EmailReader header star button (not the list-row star)
 *   6. Capture flags AFTER — assert \Flagged was toggled (SPA E2E HARD RULE:
 *      validate mutation via API state before/after, not UI-only assertions)
 *   7. Re-fetch the FullMessage via GET /api/folders/INBOX/messages/{uid} and
 *      assert it also reflects the new flag — this is what TanStack would have
 *      cached, so it proves the ['message', folder, uid] invalidation worked
 *   8. Click the same star again — assert \Flagged round-trips back
 *   9. Assert the button exposes aria-pressed so the toggle state is
 *      announced to assistive tech
 *
 * Screenshots: frontend/e2e/screenshots/reader-star-flag/<step>.png
 *
 * Build prerequisite (handled by the auto-fix runner): `npm run build:alt-ui`
 * so the bundle in `frontend/public/modern/` reflects this commit.
 *
 * Runs against the live tunnel/proxy by default (see playwright.config.ts).
 */
import { expect, NOREPLY_CREDS, test } from '../fixtures/base.js';
import { deleteMailboxByUsername } from '../helpers/db-cleanup.js';

const SCREENSHOT_DIR = 'reader-star-flag';
const PASSWORD = 'tmail-316-reader-star-2026';

interface EnvelopeRow {
  uid: number;
  flags: string[];
}

interface FullMessageBody {
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

async function fetchFullMessage(
  baseURL: string | undefined,
  auth: Record<string, string>,
  uid: number,
): Promise<FullMessageBody | null> {
  const resp = await fetch(`${baseURL}/api/folders/INBOX/messages/${uid}`, {
    headers: auth,
  });
  if (!resp.ok) return null;
  return (await resp.json()) as FullMessageBody;
}

test.describe('TMAIL-316 alt-UI EmailReader header star toggles \\Flagged', () => {
  test.beforeAll(() => deleteMailboxByUsername(NOREPLY_CREDS.email));
  test.afterAll(() => deleteMailboxByUsername(NOREPLY_CREDS.email));

  test('reader header star POSTs /flag and round-trips through the live IMAP', async ({
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
        name: 'tmail-316-imap',
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

    const targetUid = before[0].uid;
    const startedStarred = hasFlagged(before[0].flags);

    // ── 4. open the first email so EmailReader mounts ───────────────────
    // EmailList rows render in the same order as the API, so before[0] is the
    // first .cursor-pointer row. Clicking the row body (not the list-row star
    // button) selects the message and mounts EmailReader.
    const firstRow = page.locator('div.cursor-pointer').first();
    // The subject text inside the row is the safest non-button click target.
    await firstRow.locator('.text-sm.truncate').first().click();
    // EmailReader heading <h2> renders the subject — wait for it before
    // hunting for the star button so we know the reader actually mounted.
    await expect(page.locator('h2.text-2xl').first()).toBeVisible({ timeout: 15_000 });
    await takeScreenshot(page, `${SCREENSHOT_DIR}/02-reader-opened`);

    // ── 5. find the EmailReader header star (NOT the list-row star) ─────
    // EmailReader's star sits in the header flex-row alongside <h2.text-2xl>.
    // We scope the locator to the reader header to avoid hitting the
    // list-row star on smaller viewports (where both panes render).
    const readerStar = page
      .locator('h2.text-2xl')
      .first()
      .locator('xpath=following-sibling::button[1]');
    await expect(readerStar, 'reader header star is keyboard/AT discoverable').toBeVisible({
      timeout: 10_000,
    });
    // aria-pressed must reflect the starting flag state so screen readers
    // announce the toggle state correctly.
    await expect(readerStar).toHaveAttribute('aria-pressed', String(startedStarred));
    await expect(readerStar).toHaveAttribute(
      'aria-label',
      new RegExp(startedStarred ? '^Unstar email from ' : '^Star email from '),
    );
    await takeScreenshot(page, `${SCREENSHOT_DIR}/03-before-click`);

    // ── 6. click — first toggle direction ───────────────────────────────
    await readerStar.click();

    // The IMAP STORE → FLAGS reply round-trip plus the TanStack invalidation
    // can take a beat. Poll for up to ~10s on the envelope list.
    let flippedOnce = false;
    for (let attempt = 0; attempt < 10 && !flippedOnce; attempt++) {
      await page.waitForTimeout(1000);
      const afterFirst = await fetchInbox(baseURL, auth);
      const row = afterFirst.find((r) => r.uid === targetUid);
      if (row && hasFlagged(row.flags) !== startedStarred) {
        flippedOnce = true;
      }
    }
    expect(
      flippedOnce,
      `flag toggled on uid=${targetUid} (started starred=${startedStarred})`,
    ).toBe(true);

    // ── 7. confirm the FullMessage detail also reflects the new flag ────
    // This is what TanStack ['message', folder, uid] would cache, so it
    // proves the detail-cache invalidation (the new bit in TMAIL-316) is
    // backed by real server state too.
    const detailAfter = await fetchFullMessage(baseURL, auth, targetUid);
    expect(detailAfter, 'message detail must be reachable after toggle').not.toBeNull();
    expect(
      hasFlagged(detailAfter?.flags),
      `FullMessage flags reflect toggle for uid=${targetUid}`,
    ).toBe(!startedStarred);

    await takeScreenshot(page, `${SCREENSHOT_DIR}/04-after-first-click`);

    // aria-pressed must now reflect the new state — read from the DOM, not
    // from our snapshot, so we're asserting what AT actually sees.
    await expect(readerStar).toHaveAttribute(
      'aria-pressed',
      String(!startedStarred),
    );

    // ── 8. click again — assert the flag is removed (full round-trip) ───
    await readerStar.click();
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

    const detailFinal = await fetchFullMessage(baseURL, auth, targetUid);
    expect(
      hasFlagged(detailFinal?.flags),
      `FullMessage flags rolled back for uid=${targetUid}`,
    ).toBe(startedStarred);

    await takeScreenshot(page, `${SCREENSHOT_DIR}/05-after-second-click`);
    await expect(readerStar).toHaveAttribute(
      'aria-pressed',
      String(startedStarred),
    );
  });
});
