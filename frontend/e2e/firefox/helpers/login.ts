// Added: Page-object-style login helper for Firefox E2E suite (TMAIL-388).
//
// Enforces the global E2E navigation rule: `page.goto('/')` is the ONLY
// allowed direct URL — everything else inside the app must be reached by
// clicking menu items / links. We also dismiss the post-login overlays
// ("What's New", onboarding modals) so individual specs don't have to
// repeat the same dismissal dance.
import type { Page, Locator } from '@playwright/test';
import { expect } from '@playwright/test';

export interface LoginOptions {
  /** Maximum wait for the post-login redirect. */
  timeoutMs?: number;
}

/**
 * Walks the public login form and asserts the user lands on the dashboard.
 *
 *   await login(page, user.email, user.password);
 *
 * Throws (via expect) if the dashboard never loads — fail loud rather than
 * letting downstream selectors flake.
 */
export async function login(
  page: Page,
  email: string,
  password: string,
  options: LoginOptions = {},
): Promise<void> {
  const timeout = options.timeoutMs ?? 15_000;

  // The ONLY page.goto allowed per the E2E navigation rule — the landing /
  // login URL is the entry point. Everything after this is menu clicks.
  await page.goto('/');

  // Some builds land on `/` (landing) instead of `/login`; if so, click into
  // login via the visible "Sign in" link rather than another goto.
  const onLogin = await page.locator('#username, input[name="username"], input[type="email"]').first().isVisible().catch(() => false);
  if (!onLogin) {
    const signInLink = page.getByRole('link', { name: /sign in|log in/i }).first();
    if (await signInLink.isVisible().catch(() => false)) {
      await signInLink.click();
    }
  }

  const usernameField = page.locator('#username, input[name="username"], input[type="email"]').first();
  const passwordField = page.locator('#password, input[name="password"], input[type="password"]').first();
  await expect(usernameField).toBeVisible({ timeout });
  await usernameField.fill(email);
  await passwordField.fill(password);

  await page.locator('button[type="submit"]').first().click();

  // After successful login the SPA routes into the authenticated shell. We
  // accept either /app, /onboarding (BYOK first-time wizard), or /modern as
  // valid post-login destinations.
  await page.waitForURL(/\/(app|onboarding|modern)\b/, { timeout });

  await dismissOverlays(page);
}

/**
 * Dismisses any onboarding / "What's New" overlays that may obscure menu items
 * after login. Safe to call even if no overlay is present — selectors that
 * miss simply no-op.
 */
export async function dismissOverlays(page: Page): Promise<void> {
  const candidates: Locator[] = [
    page.getByRole('button', { name: /^(skip|got it|dismiss|close|maybe later|no thanks)$/i }),
    page.locator('[data-overlay-dismiss], [data-onboarding-skip]'),
    page.getByRole('button', { name: /what's new/i }),
  ];

  for (const candidate of candidates) {
    const count = await candidate.count().catch(() => 0);
    for (let i = 0; i < count; i++) {
      const item = candidate.nth(i);
      if (await item.isVisible().catch(() => false)) {
        await item.click({ timeout: 2_000 }).catch(() => {
          // overlay may have auto-closed between visible-check and click
        });
      }
    }
  }
}
