/**
 * TMAIL-226: classic SPA → modern UI hop.
 *
 * Sign up + attach the noreply BYOK config via the API, open the classic
 * /app dashboard, click the new "Try the modern UI" button in the TopBar,
 * and assert the alt-UI bundle loads and renders the same INBOX from
 * swmail.techatscale.io. Token survives the full-page nav because both
 * UIs share localStorage on the same origin.
 */
import { test, NOREPLY_CREDS, expect } from '../fixtures/base.js';
import { deleteMailboxByUsername } from '../helpers/db-cleanup.js';

const PASSWORD = 'modern-ui-e2e-2026';

test.describe('Modern UI alt-theme (TMAIL-226)', () => {
  test.beforeAll(() => deleteMailboxByUsername(NOREPLY_CREDS.email));
  test.afterAll(() => deleteMailboxByUsername(NOREPLY_CREDS.email));

  test('classic dashboard → /modern/ shows the same inbox', async ({
    page,
    apiSignup,
    takeScreenshot,
    baseURL,
  }) => {
    test.setTimeout(90_000);

    // 1. Bootstrap a BYOK account targeting the noreply mailbox.
    const tokens = await apiSignup(NOREPLY_CREDS.email, PASSWORD);
    const auth = { Authorization: `Bearer ${tokens.access_token}`, 'Content-Type': 'application/json' };
    const imapResp = await fetch(`${baseURL}/api/imap-configs`, {
      method: 'POST',
      headers: auth,
      body: JSON.stringify({
        name: 'modern-ui-e2e',
        host: NOREPLY_CREDS.imap.host,
        port: NOREPLY_CREDS.imap.port,
        username: NOREPLY_CREDS.imap.username,
        password: NOREPLY_CREDS.imap.password,
        encryption: NOREPLY_CREDS.imap.encryption,
        is_default: true,
      }),
    });
    expect(imapResp.status, 'IMAP config create').toBe(201);

    // 2. Inject session and open the classic /app.
    await page.goto('/login');
    await page.evaluate(([at, rt]) => {
      localStorage.setItem('access_token', at);
      localStorage.setItem('refresh_token', rt);
    }, [tokens.access_token, tokens.refresh_token]);
    await page.goto('/app');
    await expect(page.locator('button, a', { hasText: /Compose/i }).first())
      .toBeVisible({ timeout: 20_000 });
    // Wait for the folder tree so we know the IMAP config landed.
    await expect(page.locator('button, a, li', { hasText: /INBOX/i }).first())
      .toBeVisible({ timeout: 25_000 });
    await takeScreenshot(page, 'modern-ui/01-classic-dashboard');

    // 3. Click the new "Try the modern UI" anchor in the TopBar.
    const switcher = page.locator('a[title="Try the modern UI"]');
    await expect(switcher).toBeVisible();
    await switcher.click();

    // 4. Full-page nav — wait for the alt-UI bundle to take over.
    await page.waitForURL(/\/modern\/index\.html/i, { timeout: 15_000 });
    await page.waitForLoadState('domcontentloaded');
    await page.waitForLoadState('networkidle').catch(() => null);
    // Title was set in TMAIL-222.
    await expect(page).toHaveTitle(/Modern UI/i);
    await takeScreenshot(page, 'modern-ui/02-alt-ui-loaded');

    // 5. The active folder header reads "INBOX" once the auth gate clears
    // and the folders query resolves.
    await expect(page.locator('h2', { hasText: /INBOX/i })).toBeVisible({ timeout: 25_000 });
    // At least one message row from the real inbox should render.
    await expect(page.locator('text=/Mail Delivery|Action Required|Re:|Fwd:|TASMail SMTP Test/').first())
      .toBeVisible({ timeout: 25_000 });
    await takeScreenshot(page, 'modern-ui/03-inbox-rendered');
  });
});
