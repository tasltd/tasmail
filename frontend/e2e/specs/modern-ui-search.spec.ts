/**
 * TMAIL-322: alt-UI ("modern") Navbar search bar submits to GET /api/search
 * and renders a results page. Each result row deep-links into the
 * EmailClient via `/?folder=...&uid=...` so the reader opens on click.
 *
 * Coverage:
 *   1. Sign up + BYOK the noreply mailbox so /api/folders/INBOX/messages
 *      has real envelopes to search
 *   2. Hop into /modern/ via the classic SPA's wand button
 *   3. Snapshot INBOX via the live API and derive a search term from the
 *      first envelope's subject (or the sender local-part as fallback) —
 *      guarantees the search has at least one match without hard-coding
 *      a string we don't control
 *   4. Type the term in the Navbar search input and assert:
 *      - the URL hash changes to `/search?q=<encoded>`
 *      - the SearchResultsPage renders
 *      - the result list contains at least one row matching the API result
 *   5. Independently call GET /api/search with the same term and assert the
 *      SPA row count matches the backend count (SPA E2E HARD RULE:
 *      validate mutation/state via API before AND after — here it's
 *      verifying the SPA's rendered list matches the live server response)
 *   6. Click a result row and assert:
 *      - the URL hash changes to `/?folder=INBOX&uid=<uid>`
 *      - EmailClient mounts with the message pre-selected (reader h2 visible)
 *
 * Screenshots: frontend/e2e/screenshots/search/<step>.png
 *
 * Build prerequisite (handled by the auto-fix runner): `npm run build:alt-ui`
 * so the bundle in `frontend/public/modern/` reflects this commit.
 *
 * Runs against the live tunnel/proxy by default (see playwright.config.ts).
 */
import { expect, NOREPLY_CREDS, test } from '../fixtures/base.js';
import { deleteMailboxByUsername } from '../helpers/db-cleanup.js';

const SCREENSHOT_DIR = 'search';
const PASSWORD = 'tmail-322-search-2026';

interface EnvelopeRow {
  uid: number;
  subject: string | null;
  from: string | null;
}

interface SearchResponseBody {
  messages: EnvelopeRow[];
  total: number;
  query: string;
  folder: string;
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

async function fetchSearch(
  baseURL: string | undefined,
  auth: Record<string, string>,
  query: string,
): Promise<SearchResponseBody | null> {
  const resp = await fetch(
    `${baseURL}/api/search?q=${encodeURIComponent(query)}&folder=INBOX`,
    { headers: auth },
  );
  if (!resp.ok) return null;
  return (await resp.json()) as SearchResponseBody;
}

/**
 * Pick a stable, IMAP-SEARCH-friendly term from the first envelope.
 * Subject preferred (more matches); falls back to the sender local-part.
 * Strips whitespace + non-word chars and requires ≥ 4 chars so generic
 * tokens like "RE:" or "FYI" don't collapse the search to half the inbox.
 */
function pickSearchTerm(envelope: EnvelopeRow): string {
  const candidates: string[] = [];
  if (envelope.subject) {
    for (const word of envelope.subject.split(/[\s,.;:!?()\[\]<>"'/]+/)) {
      const w = word.trim();
      if (w.length >= 4 && /^[A-Za-z0-9]+$/.test(w)) candidates.push(w);
    }
  }
  if (candidates.length > 0) return candidates[0];

  // Fallback: sender local-part (everything before the @, with display name stripped).
  if (envelope.from) {
    const angle = envelope.from.match(/<([^>]+)>/);
    const addr = angle ? angle[1] : envelope.from;
    const local = addr.split('@')[0]?.trim() ?? '';
    if (local.length >= 4 && /^[A-Za-z0-9._-]+$/.test(local)) return local;
  }

  // Last-resort: a generic token that should match almost any modern message.
  return 'the';
}

test.describe('TMAIL-322 alt-UI Navbar search submits to /api/search and renders results', () => {
  test.beforeAll(() => deleteMailboxByUsername(NOREPLY_CREDS.email));
  test.afterAll(() => deleteMailboxByUsername(NOREPLY_CREDS.email));

  test('typing in the search bar navigates to /search and a row click opens the reader', async ({
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
        name: 'tmail-322-imap',
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

    // ── 3. snapshot INBOX + pick a stable search term from the first row ──
    const inbox = await fetchInbox(baseURL, auth);
    expect(inbox.length, 'inbox must contain at least one envelope').toBeGreaterThan(0);
    const searchTerm = pickSearchTerm(inbox[0]);

    // Independently confirm the backend can answer this query with ≥ 1 hit
    // BEFORE we ask the SPA to render it (SPA E2E HARD RULE: capture API
    // state first so we know what the UI should show).
    const backendResult = await fetchSearch(baseURL, auth, searchTerm);
    expect(backendResult, '/api/search must respond 200').not.toBeNull();
    expect(
      backendResult!.total,
      `term="${searchTerm}" must return at least one hit from the live IMAP`,
    ).toBeGreaterThan(0);
    const expectedFirstUid = backendResult!.messages[0].uid;

    // ── 4. type into the Navbar search input ─────────────────────────────
    const searchInput = page.getByTestId('modern-ui-search-input');
    await expect(searchInput, 'navbar search input is mounted').toBeVisible({
      timeout: 10_000,
    });
    await searchInput.click();
    await searchInput.fill(searchTerm);
    await takeScreenshot(page, `${SCREENSHOT_DIR}/02-input-filled`);

    // Press Enter — fires the form submit path so we don't have to wait the
    // full 400 ms debounce window. The hash should update to /search?q=...
    await searchInput.press('Enter');

    // The HashRouter writes the query into location.hash, so URL assertions
    // must match against the hash fragment.
    await page.waitForURL(
      new RegExp(`/modern/index\\.html#/search\\?q=${encodeURIComponent(searchTerm)}`, 'i'),
      { timeout: 10_000 },
    );

    // ── 5. SearchResultsPage renders with at least one row ───────────────
    await expect(page.getByTestId('search-results-query')).toContainText(searchTerm, {
      timeout: 15_000,
    });
    const resultsList = page.getByTestId('search-results-list');
    await expect(resultsList, 'results list is rendered').toBeVisible({ timeout: 15_000 });

    const rows = page.getByTestId('search-result-row');
    const renderedCount = await rows.count();
    expect(
      renderedCount,
      `SPA rendered row count must match /api/search total (backend=${backendResult!.total})`,
    ).toBe(backendResult!.messages.length);
    await takeScreenshot(page, `${SCREENSHOT_DIR}/03-results-rendered`);

    // ── 6. click the first result row → EmailClient opens on that uid ───
    await rows.first().click();

    await page.waitForURL(
      new RegExp(`/modern/index\\.html#/\\?folder=INBOX&uid=${expectedFirstUid}`, 'i'),
      { timeout: 10_000 },
    );

    // EmailClient mounts and renders the reader for the deep-linked uid.
    // The reader pane's <h2.text-2xl> carries the subject — its presence is
    // the proof that EmailClient consumed the URL params and selected the
    // message. (Empty-state copy is plain text, not an h2.)
    await expect(page.locator('h2.text-2xl').first()).toBeVisible({ timeout: 20_000 });
    await takeScreenshot(page, `${SCREENSHOT_DIR}/04-reader-opened-from-result`);
  });
});
