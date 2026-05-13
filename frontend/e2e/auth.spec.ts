// Added: Authentication E2E specs for TASMail (TMAIL-36)
import { test, expect } from './fixtures/base';

test.describe('Login Page', () => {
  test('renders login form with branding', async ({ page, takeScreenshot }) => {
    // Changed: post-BYOK pivot `/` serves LandingPage. The actual login form lives at /login.
    await page.goto('/login');

    // Added: Verify login card renders with TASMail branding
    await expect(page.locator('.login-card')).toBeVisible();
    await expect(page.locator('.login-card__header h1')).toHaveText('TASMail');
    // Changed: post-BYOK pivot — subtitle now reflects "webmail for any IMAP/SMTP".
    await expect(page.locator('.login-card__header p')).toHaveText('Webmail for any IMAP/SMTP server');

    // Added: Verify form fields are present
    await expect(page.locator('#username')).toBeVisible();
    await expect(page.locator('#password')).toBeVisible();
    await expect(page.locator('button[type="submit"]')).toBeVisible();
    await expect(page.locator('button[type="submit"]')).toHaveText('Sign In');

    await takeScreenshot(page, 'auth/login-page-rendered');
  });

  test('shows validation error for empty credentials', async ({ page, takeScreenshot }) => {
    await page.goto('/login');
    await page.waitForSelector('#username');

    // Added: Click submit without filling any fields
    await page.click('button[type="submit"]');

    // Added: Verify error message appears for empty credentials
    await expect(page.locator('.login-card__error')).toBeVisible();
    await expect(page.locator('.login-card__error')).toHaveText('Username and password are required');

    await takeScreenshot(page, 'auth/login-validation-error');
  });

  test('shows validation error for missing password', async ({ page, takeScreenshot }) => {
    await page.goto('/login');
    await page.waitForSelector('#username');

    // Added: Fill only email, leave password empty
    await page.fill('#username', 'user@example.com');
    await page.click('button[type="submit"]');

    await expect(page.locator('.login-card__error')).toBeVisible();
    await expect(page.locator('.login-card__error')).toHaveText('Username and password are required');

    await takeScreenshot(page, 'auth/login-missing-password');
  });

  test('shows loading state during login attempt', async ({ page, takeScreenshot }) => {
    await page.goto('/login');
    await page.waitForSelector('#username');

    await page.fill('#username', 'user@example.com');
    await page.fill('#password', 'password123');

    // Added: Intercept the login API call to observe loading state
    await page.route('**/api/auth/login', async (route) => {
      // NOTE: Delay response to capture the loading state screenshot
      await new Promise((resolve) => setTimeout(resolve, 500));
      await route.fulfill({
        status: 200,
        contentType: 'application/json',
        body: JSON.stringify({
          access_token: 'mock-token',
          refresh_token: 'mock-refresh',
        }),
      });
    });

    await page.click('button[type="submit"]');

    // Added: Verify button shows loading text
    await expect(page.locator('button[type="submit"]')).toHaveText('Signing in...');
    await expect(page.locator('button[type="submit"]')).toBeDisabled();

    await takeScreenshot(page, 'auth/login-loading-state');
  });

  test('successful login redirects to mailbox', async ({ page, loginAs, takeScreenshot }) => {
    // Added: Mock login API to return valid tokens
    await page.route('**/api/auth/login', async (route) => {
      await route.fulfill({
        status: 200,
        contentType: 'application/json',
        body: JSON.stringify({
          access_token: 'mock-access-token',
          refresh_token: 'mock-refresh-token',
        }),
      });
    });

    // Added: Mock folders API so sidebar loads after login
    await page.route('**/api/folders', async (route) => {
      await route.fulfill({
        status: 200,
        contentType: 'application/json',
        body: JSON.stringify([
          { name: 'INBOX', unseen: 5 },
          { name: 'Sent', unseen: 0 },
          { name: 'Drafts', unseen: 0 },
          { name: 'Trash', unseen: 0 },
          { name: 'Junk', unseen: 2 },
        ]),
      });
    });

    // Added: Mock OIDC providers to prevent errors on login page load
    await page.route('**/api/oidc/providers/login', async (route) => {
      await route.fulfill({
        status: 200,
        contentType: 'application/json',
        body: JSON.stringify([]),
      });
    });

    // Added: Mock quota API for sidebar QuotaBar
    await page.route('**/api/quota', async (route) => {
      await route.fulfill({
        status: 200,
        contentType: 'application/json',
        body: JSON.stringify({ used: 100, limit: 1000 }),
      });
    });

    await loginAs(page, 'user@example.com', 'password123');

    // Added: Verify sidebar is visible after login (confirms successful auth)
    await expect(page.locator('.sidebar')).toBeVisible();
    await expect(page.locator('.topbar')).toBeVisible();

    await takeScreenshot(page, 'auth/login-success-mailbox');
  });

  test('displays login error on failed authentication', async ({ page, takeScreenshot }) => {
    // Added: Mock login API to return 401
    await page.route('**/api/auth/login', async (route) => {
      await route.fulfill({
        status: 401,
        contentType: 'application/json',
        body: JSON.stringify({ error: 'Invalid credentials' }),
      });
    });

    // Added: Mock OIDC providers
    await page.route('**/api/oidc/providers/login', async (route) => {
      await route.fulfill({
        status: 200,
        contentType: 'application/json',
        body: JSON.stringify([]),
      });
    });

    await page.goto('/login');
    await page.waitForSelector('#username');

    await page.fill('#username', 'wrong@example.com');
    await page.fill('#password', 'badpassword');
    await page.click('button[type="submit"]');

    // Added: Verify error message is displayed
    await expect(page.locator('.login-card__error')).toBeVisible({ timeout: 5_000 });

    await takeScreenshot(page, 'auth/login-failed-error');
  });
});

test.describe('Logout', () => {
  test('logout returns to login page', async ({ page, loginAs, takeScreenshot }) => {
    // Added: Set up route mocks for authenticated session
    await page.route('**/api/auth/login', async (route) => {
      await route.fulfill({
        status: 200,
        contentType: 'application/json',
        body: JSON.stringify({
          access_token: 'mock-access-token',
          refresh_token: 'mock-refresh-token',
        }),
      });
    });

    await page.route('**/api/folders', async (route) => {
      await route.fulfill({
        status: 200,
        contentType: 'application/json',
        body: JSON.stringify([
          { name: 'INBOX', unseen: 3 },
          { name: 'Sent', unseen: 0 },
          { name: 'Drafts', unseen: 1 },
          { name: 'Trash', unseen: 0 },
        ]),
      });
    });

    await page.route('**/api/oidc/providers/login', async (route) => {
      await route.fulfill({
        status: 200,
        contentType: 'application/json',
        body: JSON.stringify([]),
      });
    });

    await page.route('**/api/quota', async (route) => {
      await route.fulfill({
        status: 200,
        contentType: 'application/json',
        body: JSON.stringify({ used: 50, limit: 1000 }),
      });
    });

    // Added: Mock /api/auth/logout so the client's `await apiClient.post('/auth/logout')`
    // succeeds. Without this, the request 401s, client.ts triggers its
    // "Session expired" hard-redirect to /login, and AppRoute's onLogout
    // navigate('/') never fires — landing-page assertion below would fail.
    await page.route('**/api/auth/logout', async (route) => {
      await route.fulfill({ status: 204, body: '' });
    });

    await loginAs(page, 'user@example.com', 'password123');
    await takeScreenshot(page, 'auth/logout-before');

    // Added: Click the logout button in the top bar (navigating via UI, not goto)
    await page.click('.topbar button[title="Logout"]');

    // Changed: post-BYOK pivot — logout drops the user on the public landing page (/), not the login form.
    // The landing hero is the stable signal that the SPA navigated away from /app.
    await expect(page.locator('.landing-header__brand')).toBeVisible({ timeout: 5_000 });
    await expect(page.locator('.landing-hero__title')).toBeVisible();

    await takeScreenshot(page, 'auth/logout-after');
  });
});
