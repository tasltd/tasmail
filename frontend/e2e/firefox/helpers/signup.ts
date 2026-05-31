// Added: Programmatic BYOK signup helper for Firefox E2E suite (TMAIL-388).
//
// Each spec gets a brand-new tenant via POST /api/auth/signup so suites can run
// in any order without sharing state. The synthetic byok.tasmail domain (added
// by migration 056) lets signups land without an admin-provisioned domain row.
//
// The endpoint contract (from backend/src/handlers/auth.rs::signup):
//   request:  { email, password, display_name? }
//   response: { access_token, refresh_token, expires_in }
//   201 on success, 409 if the email is taken.
//
// Per the project's E2E-validate-via-real-backend rule, this helper hits the
// running backend — no mocking. The default base URL is the firefox-test
// project's localhost:5273 Vite proxy; override with PLAYWRIGHT_TEST_BASE_URL.
import type { APIRequestContext } from '@playwright/test';

export interface SignupResult {
  email: string;
  password: string;
  accessToken: string;
  refreshToken: string;
}

export interface SignupOptions {
  /** Email override. Defaults to `e2e-modern-{Date.now()}@byok.tasmail`. */
  email?: string;
  /** Password override. Defaults to a 16-char fixed value (length > 8 satisfies the backend rule). */
  password?: string;
  /** Optional display name to send with the signup request. */
  displayName?: string;
  /** Base URL of the test backend. Defaults to PLAYWRIGHT_TEST_BASE_URL or localhost:5273. */
  baseURL?: string;
}

const DEFAULT_BASE_URL = process.env.PLAYWRIGHT_TEST_BASE_URL ?? 'http://localhost:5273';
const DEFAULT_PASSWORD = 'e2e-test-pass-2026';

function freshEmail(): string {
  // Date.now() + random suffix keeps two same-tick specs from colliding.
  const suffix = Math.random().toString(36).slice(2, 8);
  return `e2e-modern-${Date.now()}-${suffix}@byok.tasmail`;
}

/**
 * Programmatically signs up a fresh BYOK tenant.
 *
 * Usage from a spec:
 *   const user = await signupFreshUser(request);
 *   // user.email / user.password / user.accessToken are now usable.
 *
 * Pass `request` (Playwright `APIRequestContext`) so signup uses Playwright's
 * fetch — gives us baseURL handling, retry policy, and trace integration for
 * free. If you need a raw fetch (e.g. inside globalSetup before fixtures
 * exist), pass `{ baseURL }` and omit `request`.
 */
export async function signupFreshUser(
  request: APIRequestContext | null,
  options: SignupOptions = {},
): Promise<SignupResult> {
  const email = options.email ?? freshEmail();
  const password = options.password ?? DEFAULT_PASSWORD;
  const body: Record<string, string> = { email, password };
  if (options.displayName) body.display_name = options.displayName;

  const url = '/api/auth/signup';

  let status: number;
  let payload: unknown;

  if (request) {
    const resp = await request.post(url, {
      data: body,
      headers: { 'Content-Type': 'application/json' },
    });
    status = resp.status();
    payload = await resp.json().catch(() => null);
  } else {
    const baseURL = (options.baseURL ?? DEFAULT_BASE_URL).replace(/\/$/, '');
    const resp = await fetch(`${baseURL}${url}`, {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify(body),
    });
    status = resp.status;
    payload = await resp.json().catch(() => null);
  }

  if (status !== 200 && status !== 201) {
    throw new Error(
      `signupFreshUser(${email}) failed: HTTP ${status} body=${JSON.stringify(payload)}`,
    );
  }

  const tokens = payload as { access_token?: string; refresh_token?: string } | null;
  if (!tokens?.access_token || !tokens.refresh_token) {
    throw new Error(
      `signupFreshUser(${email}) did not return a token pair: ${JSON.stringify(payload)}`,
    );
  }

  return {
    email,
    password,
    accessToken: tokens.access_token,
    refreshToken: tokens.refresh_token,
  };
}
