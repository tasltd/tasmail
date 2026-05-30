/**
 * TMAIL-328: Modern UI /ws subscription.
 *
 * Verifies that the alt-UI opens a WebSocket to `/ws?token=<jwt>` once the
 * AuthGate has a token in hand, and that the connection follows the
 * backend contract (token query param, `subscribe:INBOX` client frame).
 *
 * The spec uses page.waitForEvent('websocket') so the upgrade itself is
 * the assertion — we don't need to wait for a real `new_mail` event from
 * upstream IMAP, which would require a live mail-delivery test rig.
 *
 * Screenshots: frontend/e2e/screenshots/modern-ui-websocket/<step>.png
 */
import { test, NOREPLY_CREDS, expect } from '../fixtures/base.js';
import { deleteMailboxByUsername } from '../helpers/db-cleanup.js';

const PASSWORD = 'modern-ws-e2e-2026';

test.describe('TMAIL-328 Modern UI /ws subscription', () => {
  test.beforeAll(() => deleteMailboxByUsername(NOREPLY_CREDS.email));
  test.afterAll(() => deleteMailboxByUsername(NOREPLY_CREDS.email));

  test('opens /ws?token=... and subscribes to INBOX after login', async ({
    page,
    apiSignup,
    takeScreenshot,
    baseURL,
  }) => {
    test.setTimeout(90_000);

    // 1. Stand up a known account + attach the noreply BYOK config so the
    // backend has something to IDLE against once the WS subscribes.
    const tokens = await apiSignup(NOREPLY_CREDS.email, PASSWORD);
    const auth = {
      Authorization: `Bearer ${tokens.access_token}`,
      'Content-Type': 'application/json',
    };
    const imapResp = await fetch(`${baseURL}/api/imap-configs`, {
      method: 'POST',
      headers: auth,
      body: JSON.stringify({
        name: 'modern-ws-e2e',
        host: NOREPLY_CREDS.imap.host,
        port: NOREPLY_CREDS.imap.port,
        username: NOREPLY_CREDS.imap.username,
        password: NOREPLY_CREDS.imap.password,
        encryption: NOREPLY_CREDS.imap.encryption,
        is_default: true,
      }),
    });
    expect(imapResp.status, 'IMAP config create').toBe(201);

    // 2. Land directly on the Modern UI root with the token already in
    // localStorage so the AuthGate flips to ready immediately and mounts
    // WsBridge. Capture the WebSocket promise BEFORE the URL change so we
    // don't miss the upgrade.
    const wsPromise = page.waitForEvent('websocket', { timeout: 20_000 });

    await page.goto(`${baseURL}/modern/index.html`);
    await page.evaluate(([at, rt]) => {
      localStorage.setItem('access_token', at);
      localStorage.setItem('refresh_token', rt);
    }, [tokens.access_token, tokens.refresh_token]);
    // Reload so the AuthGate's initial-mount effect picks up the planted
    // tokens (the in-page TOKEN_CHANGED_EVENT path is exercised by the
    // login spec; this one focuses on the cold-start WS open).
    await page.goto(`${baseURL}/modern/index.html`);

    // EmailClient should render the INBOX header once /api/folders resolves.
    await expect(page.locator('h2', { hasText: /INBOX/i })).toBeVisible({
      timeout: 25_000,
    });
    await takeScreenshot(page, 'modern-ui-websocket/01-inbox-loaded');

    // 3. The WebSocket opens. Assert (a) the URL goes to /ws, (b) the token
    // arrives as a query param per backend handler/websocket.rs::WsParams.
    const ws = await wsPromise;
    const wsUrl = ws.url();
    expect(wsUrl, 'WS URL').toMatch(/\/ws(\?|$)/);
    expect(wsUrl, 'WS token query param').toMatch(/[?&]token=/);

    // 4. The Modern UI client immediately sends `subscribe:INBOX`. The
    // Playwright WebSocket API surfaces outbound frames via
    // page.on('websocket') → ws.on('framesent'); we capture them eagerly
    // and assert on the first text frame, which should match exactly the
    // value useWebSocket() sends in onopen.
    const sentFrames: string[] = [];
    ws.on('framesent', (frame) => {
      if (typeof frame.payload === 'string') sentFrames.push(frame.payload);
    });
    await page.waitForTimeout(2_000); // give the onopen send a moment
    expect(
      sentFrames.some((f) => f === 'subscribe:INBOX'),
      `expected an outbound 'subscribe:INBOX' frame, got: ${JSON.stringify(sentFrames)}`,
    ).toBe(true);
    await takeScreenshot(page, 'modern-ui-websocket/02-after-subscribe');
  });
});
