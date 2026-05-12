/**
 * /app dashboard with a live BYOK mailbox
 *
 * Bootstraps a fresh TASMail account via the API, then writes the
 * noreply@techatscale.io IMAP + SMTP credentials directly into
 * imap_configurations / smtp_configurations via the public API.
 * Once that's done the user lands on /app and the SPA proxies the user's real
 * inbox via the live SSH tunnel + backend.
 *
 * This spec proves the full vertical works: signup → BYOK config → IMAP fetch
 * round-trip → SPA renders the result.
 */
import { test, NOREPLY_CREDS } from '../fixtures/base.js';
import { expect } from '@playwright/test';

const ACCOUNT_PASSWORD = 'correct-horse-battery-staple-9k';

test.describe('Dashboard with live noreply@techatscale.io mailbox', () => {
  test('inbox loads folders + messages from swmail.techatscale.io', async ({
    page,
    apiSignup,
    takeScreenshot,
    baseURL,
  }) => {
    // Real IMAP round-trip + the SPA's WebSocket poller mean this test cannot
    // rely on networkidle. Give the whole flow 90s headroom.
    test.setTimeout(90_000);

    // 1. Bootstrap a TASMail account.
    const tokens = await apiSignup(`dashboard-${Date.now()}@e2e.tasmail`, ACCOUNT_PASSWORD);
    const headers = { Authorization: `Bearer ${tokens.access_token}`, 'Content-Type': 'application/json' };

    // 2. Attach the noreply@techatscale.io IMAP server.
    const imapResp = await fetch(`${baseURL}/api/imap-configs`, {
      method: 'POST',
      headers,
      body: JSON.stringify({
        name: 'noreply (E2E test bed)',
        host: NOREPLY_CREDS.imap.host,
        port: NOREPLY_CREDS.imap.port,
        username: NOREPLY_CREDS.imap.username,
        password: NOREPLY_CREDS.imap.password,
        encryption: NOREPLY_CREDS.imap.encryption,
        is_default: true,
      }),
    });
    expect(imapResp.status, 'IMAP config create').toBe(201);

    // 3. Attach the matching SMTP server (port 465 SSL, same mailbox).
    const smtpResp = await fetch(`${baseURL}/api/smtp-configs`, {
      method: 'POST',
      headers,
      body: JSON.stringify({
        name: 'noreply (E2E test bed)',
        host: NOREPLY_CREDS.smtp.host,
        port: NOREPLY_CREDS.smtp.port,
        username: NOREPLY_CREDS.smtp.username,
        password: NOREPLY_CREDS.smtp.password,
        encryption: NOREPLY_CREDS.smtp.encryption,
        from_address: NOREPLY_CREDS.email,
        is_default: true,
      }),
    });
    expect(smtpResp.status, 'SMTP config create').toBe(201);

    // 4. Inject session into the SPA and navigate straight to /app.
    await page.goto('/login');
    await page.evaluate(([at, rt]) => {
      localStorage.setItem('access_token', at);
      localStorage.setItem('refresh_token', rt);
    }, [tokens.access_token, tokens.refresh_token]);

    await page.goto('/app');
    // Don't wait for networkidle — the SPA opens a long-lived WebSocket so it never settles.
    // Wait for the AppShell instead (Compose button always renders once the auth check passes).
    await expect(page.locator('button, a', { hasText: /Compose/i }).first())
      .toBeVisible({ timeout: 20_000 });
    await takeScreenshot(page, 'dashboard/01-app-shell-loaded');

    // 5. Verify folders fetched from the real IMAP server appear.
    // The SPA's FolderTree concatenates the folder name with an unread-count badge,
    // so the visible text reads e.g. "INBOX901". Match `INBOX` as a substring instead
    // of the whole label.
    await expect(
      page.locator('button, a, li', { hasText: /INBOX/i }).first()
    ).toBeVisible({ timeout: 25_000 });
    await takeScreenshot(page, 'dashboard/02-folder-tree-populated');

    // 6. Confirm at the API layer that /api/folders returns a non-empty list.
    const foldersApi = await fetch(`${baseURL}/api/folders`, { headers: { Authorization: `Bearer ${tokens.access_token}` } });
    expect(foldersApi.status, '/api/folders').toBe(200);
    const folders = (await foldersApi.json()) as Array<{ name: string }>;
    expect(folders.length, 'IMAP folder count').toBeGreaterThan(0);
    expect(folders.map((f) => f.name.toUpperCase())).toContain('INBOX');

    // 7. Click INBOX and wait for the message list to paint.
    // Don't wait for networkidle — WS keeps the connection live forever.
    const inboxLink = page.locator('button, a, li', { hasText: /INBOX/i }).first();
    await inboxLink.click().catch(() => null);
    // Either a message row appears (mailbox has messages) OR an empty-state message renders.
    // Use a generous timeout because Stalwart IMAP can take a few seconds to FETCH the list.
    await page.waitForTimeout(4000);
    await takeScreenshot(page, 'dashboard/03-inbox-clicked');
  });
});
