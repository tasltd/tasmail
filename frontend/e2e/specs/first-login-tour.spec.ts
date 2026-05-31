/**
 * TMAIL-401 — first-login product tour + empty-inbox state copy.
 *
 * Round-trip:
 *   1. apiSignup boots a fresh TASMail account → the mailbox row starts
 *      with first_login_tour_seen = false (migration 085 default).
 *   2. Attach a BYOK IMAP config so the empty-inbox state has a real
 *      username + host to render.
 *   3. Inject the JWT into localStorage and land on /app.
 *   4. Tour popover appears → screenshot every step.
 *   5. Dismiss → PATCH fires → backend flag flips to true → on reload
 *      the tour does NOT reappear.
 *   6. Empty INBOX shows the IMAP user@host pulled from the BYOK config.
 *
 * Backend state is asserted via /api/me/preferences/first-login-tour-seen
 * before AND after the UI dismiss to satisfy the SPA validation rule
 * (the rendered UI is corroborated by an API GET on each side).
 */
import { test, NOREPLY_CREDS, expect } from '../fixtures/base.js';

const ACCOUNT_PASSWORD = 'correct-horse-battery-staple-9k';
const SCREENSHOT_PREFIX = 'first-login-tour';

test.describe('First-login tour (TMAIL-401)', () => {
  test('new BYOK user sees 3-step tour, dismisses, never sees it again', async ({
    page,
    apiSignup,
    takeScreenshot,
    baseURL,
  }) => {
    test.setTimeout(90_000);

    // 1. Bootstrap a fresh account.
    const email = `tour-${Date.now()}@e2e.tasmail`;
    const tokens = await apiSignup(email, ACCOUNT_PASSWORD);
    const authHeader = { Authorization: `Bearer ${tokens.access_token}` };

    // 2. Attach the noreply BYOK IMAP server so the empty-inbox state has a
    //    real host/username to render.
    const imapResp = await fetch(`${baseURL}/api/imap-configs`, {
      method: 'POST',
      headers: { ...authHeader, 'Content-Type': 'application/json' },
      body: JSON.stringify({
        name: 'noreply (TMAIL-401 test)',
        host: NOREPLY_CREDS.imap.host,
        port: NOREPLY_CREDS.imap.port,
        username: NOREPLY_CREDS.imap.username,
        password: NOREPLY_CREDS.imap.password,
        encryption: NOREPLY_CREDS.imap.encryption,
        is_default: true,
      }),
    });
    expect(imapResp.status, 'IMAP config create').toBe(201);

    // 3. SPA validation — capture the backend flag BEFORE the UI action.
    const beforeResp = await fetch(
      `${baseURL}/api/me/preferences/first-login-tour-seen`,
      { headers: authHeader },
    );
    expect(beforeResp.status).toBe(200);
    const beforeBody = (await beforeResp.json()) as { seen: boolean };
    expect(beforeBody.seen, 'tour-seen flag starts false').toBe(false);

    // 4. Inject session + land on /app. (Per E2E rules the only direct goto
    //    allowed is the initial entry — same pattern as dashboard-byok.)
    await page.goto('/login');
    await page.evaluate(([at, rt]) => {
      localStorage.setItem('access_token', at);
      localStorage.setItem('refresh_token', rt);
    }, [tokens.access_token, tokens.refresh_token]);
    await page.goto('/app');

    // 5. Tour appears.
    const tour = page.getByTestId('first-login-tour');
    await expect(tour).toBeVisible({ timeout: 15_000 });
    await expect(page.getByText('Step 1 of 3')).toBeVisible();
    await takeScreenshot(page, `${SCREENSHOT_PREFIX}/01-tour-step-1-compose`);

    // 6. Advance through Step 2 (Inbox).
    await page.getByTestId('first-login-tour-next').click();
    await expect(page.getByText('Step 2 of 3')).toBeVisible();
    await expect(page.getByText('Your inbox')).toBeVisible();
    await takeScreenshot(page, `${SCREENSHOT_PREFIX}/02-tour-step-2-inbox`);

    // 7. Step 3 (Settings) — last step's CTA reads "Got it".
    await page.getByTestId('first-login-tour-next').click();
    await expect(page.getByText('Step 3 of 3')).toBeVisible();
    await expect(page.getByText('Everything else')).toBeVisible();
    await expect(page.getByTestId('first-login-tour-next')).toHaveText('Got it');
    await takeScreenshot(page, `${SCREENSHOT_PREFIX}/03-tour-step-3-settings`);

    // 8. Dismiss → PATCH fires, tour unmounts.
    await page.getByTestId('first-login-tour-next').click();
    await expect(tour).toBeHidden({ timeout: 10_000 });
    await takeScreenshot(page, `${SCREENSHOT_PREFIX}/04-after-dismiss`);

    // 9. SPA validation — backend flag flipped to true.
    const afterResp = await fetch(
      `${baseURL}/api/me/preferences/first-login-tour-seen`,
      { headers: authHeader },
    );
    expect(afterResp.status).toBe(200);
    const afterBody = (await afterResp.json()) as { seen: boolean };
    expect(afterBody.seen, 'PATCH set seen=true').toBe(true);

    // 10. Reload — tour stays dismissed.
    await page.reload();
    // AppShell has loaded once the Compose button is on screen.
    await expect(
      page.locator('button, a', { hasText: /Compose/i }).first(),
    ).toBeVisible({ timeout: 20_000 });
    // Give the lazy chunk + query a moment to settle, then assert nothing renders.
    await page.waitForTimeout(800);
    await expect(page.getByTestId('first-login-tour')).toHaveCount(0);
    await takeScreenshot(page, `${SCREENSHOT_PREFIX}/05-reload-no-tour`);
  });

  test('empty INBOX renders the user\'s configured IMAP address', async ({
    page,
    apiSignup,
    takeScreenshot,
    baseURL,
  }) => {
    test.setTimeout(60_000);

    // 1. Fresh account.
    const email = `empty-${Date.now()}@e2e.tasmail`;
    const tokens = await apiSignup(email, ACCOUNT_PASSWORD);
    const authHeader = { Authorization: `Bearer ${tokens.access_token}` };

    // 2. Attach a BYOK IMAP server with a distinctive username so we can
    //    assert it round-trips into the empty-state copy.
    const distinctUsername = `tmail401-${Date.now()}`;
    const imapResp = await fetch(`${baseURL}/api/imap-configs`, {
      method: 'POST',
      headers: { ...authHeader, 'Content-Type': 'application/json' },
      body: JSON.stringify({
        name: 'TMAIL-401 distinctive',
        host: 'imap.distinctive.test',
        port: 993,
        username: distinctUsername,
        password: 'unused-test-password',
        encryption: 'ssl',
        is_default: true,
      }),
    });
    expect(imapResp.status, 'IMAP config create').toBe(201);

    // 3. Mark the tour seen via API so it doesn't cover the empty state.
    await fetch(`${baseURL}/api/me/preferences/first-login-tour-seen`, {
      method: 'PATCH',
      headers: authHeader,
    });

    // 4. Land on /app.
    await page.goto('/login');
    await page.evaluate(([at, rt]) => {
      localStorage.setItem('access_token', at);
      localStorage.setItem('refresh_token', rt);
    }, [tokens.access_token, tokens.refresh_token]);
    await page.goto('/app');

    // 5. The selected folder defaults to INBOX. Wait for the EmptyInboxState
    //    to render (or any message-list state to clear loading).
    const emptyState = page.getByTestId('empty-inbox-state');
    await expect(emptyState).toBeVisible({ timeout: 25_000 });
    await takeScreenshot(page, `${SCREENSHOT_PREFIX}/06-empty-inbox-loaded`);

    // 6. The displayed address must match the BYOK config we just wrote.
    const expectedAddress = `${distinctUsername}@imap.distinctive.test`;
    await expect(page.getByTestId('empty-inbox-state__address')).toHaveText(
      expectedAddress,
    );
    await expect(emptyState).toContainText('Your inbox is empty');
    await takeScreenshot(
      page,
      `${SCREENSHOT_PREFIX}/07-empty-inbox-shows-imap-address`,
    );
  });
});
