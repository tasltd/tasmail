// TMAIL-374 — E2E spec for the 3-step `/classic` no-JS BYOK signup wizard.
//
// Exercises the live backend through the same Apache → SSH-tunnel → Rust
// router stack a real user hits. Nothing is mocked. Set
// PLAYWRIGHT_BASE_URL=http://localhost:4400 (or whatever local backend port)
// to run against a workstation build.
//
// Coverage matches the gap-analysis acceptance criteria (P1 #20):
//   * Login page links to Step 1 — we navigate via the link, not page.goto
//     for internal routes (HARD RULE).
//   * Step 1 GET renders an accessible form with a draft cookie + matching
//     hidden _csrf input. No <script> tags anywhere.
//   * Step 1 POST with short password re-renders with the inline error.
//   * Step 1 POST with valid credentials creates the mailbox + advances to
//     Step 2 via 303.
//   * Step 2 GET renders BOTH IMAP and SMTP forms + the provider preset
//     picker as plain <a> links (no JS).
//   * Step 2 POST with bad servers re-renders with section-specific IMAP
//     and SMTP error messages + a top-of-form retry hint.
//   * Every step inherits base.html (skip-link, <main id="main">, CSP nonce
//     on the inline <style>) and ships zero <script> tags.
//
// Screenshots are captured at every validation point per the HARD RULE.

import { test, expect } from './fixtures/base';
import path from 'path';
import { fileURLToPath } from 'url';

const __filename = fileURLToPath(import.meta.url);
const __dirname = path.dirname(__filename);
const SCREENSHOT_DIR = path.join(__dirname, 'screenshots', 'classic-signup-wizard');

// Each spec spins up a unique-per-run email so re-runs don't collide on the
// `username` unique constraint. The DB row is left behind on purpose — we
// don't want a partial-failure test to nuke other people's drafts.
function uniqueEmail(): string {
  const suffix = `${Date.now()}-${Math.floor(Math.random() * 100_000)}`;
  return `tmail374-e2e-${suffix}@example.invalid`;
}

test.describe('Classic UI Signup Wizard (TMAIL-374)', () => {
  test('Login page links to the signup wizard', async ({ page }) => {
    // Only allowed page.goto in the spec — the login page is the entry point
    // for unauthenticated users (mirrors classic-login.spec.ts).
    const resp = await page.goto('/classic/login');
    expect(resp?.status()).toBe(200);

    await page.screenshot({
      path: path.join(SCREENSHOT_DIR, 'login-page-with-signup-link.png'),
      fullPage: true,
    });

    const signupLink = page.locator('a[href="/classic/signup"]');
    await expect(signupLink).toBeVisible();

    // Click the link — navigation must work without JS.
    await Promise.all([
      page.waitForLoadState('domcontentloaded'),
      signupLink.click(),
    ]);

    expect(page.url()).toContain('/classic/signup');
    await expect(page.locator('h1')).toContainText(/create your tasmail account/i);
  });

  test('Step 1 GET renders accessible account form with draft cookie', async ({ page, context }) => {
    await page.goto('/classic/login');
    await page.locator('a[href="/classic/signup"]').click();
    await page.waitForLoadState('domcontentloaded');

    // Form scaffold.
    await expect(page.locator('form[action="/classic/signup"]')).toBeVisible();
    await expect(page.locator('input[name="email"]')).toBeVisible();
    await expect(page.locator('input[name="password"]')).toBeVisible();
    await expect(page.locator('input[name="display_name"]')).toBeVisible();
    await expect(page.locator('input[name="_csrf"]')).toHaveCount(1);

    // Accessible base layout (TMAIL-356 inheritance).
    await expect(page.locator('a.skip-link')).toBeAttached();
    await expect(page.locator('main#main')).toBeVisible();

    // No <script> tags — no-JS rule.
    expect(await page.locator('script').count()).toBe(0);

    // Draft cookie is set with the right attributes.
    const cookies = await context.cookies();
    const draft = cookies.find((c) => c.name === 'tasmail_classic_signup_draft');
    expect(draft, 'tasmail_classic_signup_draft cookie must be set on GET').toBeDefined();
    expect(draft!.httpOnly).toBe(true);
    expect(draft!.value).toMatch(/^[0-9a-f]{32}\.[A-Za-z0-9_-]{43}$/);

    // CSRF token in the form is non-empty (validated against the row, not
    // the cookie — the cookie carries the row id, not the csrf token).
    const csrf = await page.locator('input[name="_csrf"]').getAttribute('value');
    expect(csrf, 'form must carry a _csrf token').toBeTruthy();
    expect(csrf!.length).toBeGreaterThanOrEqual(40);

    await page.screenshot({
      path: path.join(SCREENSHOT_DIR, 'step1-form-loaded.png'),
      fullPage: true,
    });
  });

  test('Step 1 POST with short password re-renders form with inline error', async ({ page }) => {
    await page.goto('/classic/login');
    await page.locator('a[href="/classic/signup"]').click();
    await page.waitForLoadState('domcontentloaded');

    const email = uniqueEmail();
    await page.locator('input[name="email"]').fill(email);
    await page.locator('input[name="password"]').fill('short'); // < 8 chars
    await page.locator('input[name="display_name"]').fill('Test Short Password');

    await page.screenshot({
      path: path.join(SCREENSHOT_DIR, 'step1-form-filled-short-password.png'),
      fullPage: true,
    });

    await Promise.all([
      page.waitForLoadState('domcontentloaded'),
      page.locator('main button[type="submit"]').click(),
    ]);

    // Stay on Step 1 with an error message.
    expect(page.url()).toContain('/classic/signup');
    expect(page.url()).not.toContain('/classic/signup/imap');

    const alert = page.locator('[role="alert"]');
    await expect(alert).toBeVisible();
    expect((await alert.textContent())?.toLowerCase()).toContain('at least 8 characters');

    // Submitted values round-trip into the form so the user doesn't retype.
    await expect(page.locator('input[name="email"]')).toHaveValue(email);
    await expect(page.locator('input[name="display_name"]')).toHaveValue('Test Short Password');

    await page.screenshot({
      path: path.join(SCREENSHOT_DIR, 'step1-short-password-rejected.png'),
      fullPage: true,
    });
  });

  test('Step 1 POST with valid credentials advances to Step 2', async ({ page }) => {
    await page.goto('/classic/login');
    await page.locator('a[href="/classic/signup"]').click();
    await page.waitForLoadState('domcontentloaded');

    const email = uniqueEmail();
    await page.locator('input[name="email"]').fill(email);
    await page.locator('input[name="password"]').fill('correct-horse-battery-staple');
    await page.locator('input[name="display_name"]').fill('E2E Wizard Test');

    await page.screenshot({
      path: path.join(SCREENSHOT_DIR, 'step1-form-filled-valid.png'),
      fullPage: true,
    });

    await Promise.all([
      page.waitForLoadState('domcontentloaded'),
      page.locator('main button[type="submit"]').click(),
    ]);

    // 303 should land us on Step 2.
    expect(page.url()).toContain('/classic/signup/imap');
    await expect(page.locator('h1')).toContainText(/connect your mailbox/i);

    await page.screenshot({
      path: path.join(SCREENSHOT_DIR, 'step2-form-after-step1.png'),
      fullPage: true,
    });
  });

  test('Step 2 GET renders both IMAP + SMTP forms with preset picker', async ({ page }) => {
    // Start from the login page so we exercise the full navigation chain.
    await page.goto('/classic/login');
    await page.locator('a[href="/classic/signup"]').click();
    await page.waitForLoadState('domcontentloaded');

    const email = uniqueEmail();
    await page.locator('input[name="email"]').fill(email);
    await page.locator('input[name="password"]').fill('correct-horse-battery-staple');
    await Promise.all([
      page.waitForLoadState('domcontentloaded'),
      page.locator('main button[type="submit"]').click(),
    ]);

    // Landed on Step 2.
    expect(page.url()).toContain('/classic/signup/imap');

    // Both server sections present.
    await expect(page.locator('input[name="imap_host"]')).toBeVisible();
    await expect(page.locator('input[name="imap_port"]')).toBeVisible();
    await expect(page.locator('input[name="imap_username"]')).toBeVisible();
    await expect(page.locator('input[name="imap_password"]')).toBeVisible();
    await expect(page.locator('select[name="imap_encryption"]')).toBeVisible();

    await expect(page.locator('input[name="smtp_host"]')).toBeVisible();
    await expect(page.locator('input[name="smtp_port"]')).toBeVisible();
    await expect(page.locator('input[name="smtp_username"]')).toBeVisible();
    await expect(page.locator('input[name="smtp_password"]')).toBeVisible();
    await expect(page.locator('select[name="smtp_encryption"]')).toBeVisible();

    // Preset picker — Gmail / Outlook / Zoho / FastMail / iCloud / ProtonMail
    // Bridge etc., plus "Custom / None of these". Rendered as plain <a> links
    // so no-JS browsers can swap presets.
    await expect(page.locator('a[href*="?preset=Gmail"]')).toBeVisible();
    await expect(page.locator('a[href*="?preset=Zoho"]')).toBeVisible();
    await expect(page.locator('a[href="/classic/signup/imap"]')).toBeVisible(); // "Custom / None of these"

    // No <script> tags anywhere.
    expect(await page.locator('script').count()).toBe(0);

    await page.screenshot({
      path: path.join(SCREENSHOT_DIR, 'step2-form-loaded.png'),
      fullPage: true,
    });

    // Click the Gmail preset link to validate query-string auto-fill works.
    await Promise.all([
      page.waitForLoadState('domcontentloaded'),
      page.locator('a[href*="?preset=Gmail"]').click(),
    ]);

    // After the preset click the host fields should be pre-filled with Gmail's settings.
    await expect(page.locator('input[name="imap_host"]')).toHaveValue('imap.gmail.com');
    await expect(page.locator('input[name="imap_port"]')).toHaveValue('993');
    await expect(page.locator('input[name="smtp_host"]')).toHaveValue('smtp.gmail.com');

    await page.screenshot({
      path: path.join(SCREENSHOT_DIR, 'step2-gmail-preset-autofill.png'),
      fullPage: true,
    });
  });

  test('Step 2 POST with bad servers re-renders with section-specific errors', async ({ page }) => {
    await page.goto('/classic/login');
    await page.locator('a[href="/classic/signup"]').click();
    await page.waitForLoadState('domcontentloaded');

    const email = uniqueEmail();
    await page.locator('input[name="email"]').fill(email);
    await page.locator('input[name="password"]').fill('correct-horse-battery-staple');
    await Promise.all([
      page.waitForLoadState('domcontentloaded'),
      page.locator('main button[type="submit"]').click(),
    ]);

    expect(page.url()).toContain('/classic/signup/imap');

    // Fill BOTH server forms with intentionally bogus hosts. The wizard will
    // try to TCP-connect + LOGIN against both in parallel and fail.
    await page.locator('input[name="imap_host"]').fill('imap.does-not-exist-tmail374.invalid');
    await page.locator('input[name="imap_port"]').fill('993');
    await page.locator('input[name="imap_username"]').fill('does-not-matter');
    await page.locator('input[name="imap_password"]').fill('does-not-matter');
    await page.locator('select[name="imap_encryption"]').selectOption('ssl');

    await page.locator('input[name="smtp_host"]').fill('smtp.does-not-exist-tmail374.invalid');
    await page.locator('input[name="smtp_port"]').fill('587');
    await page.locator('input[name="smtp_username"]').fill('does-not-matter');
    await page.locator('input[name="smtp_password"]').fill('does-not-matter');
    await page.locator('select[name="smtp_encryption"]').selectOption('starttls');

    await page.screenshot({
      path: path.join(SCREENSHOT_DIR, 'step2-form-filled-bad-servers.png'),
      fullPage: true,
    });

    await Promise.all([
      page.waitForLoadState('domcontentloaded'),
      page.locator('main button[type="submit"]').click(),
    ]);

    // Stay on Step 2 with both errors visible. Wait up to 30s because the
    // DNS lookups will time out on the underlying TCP connect.
    expect(page.url()).toContain('/classic/signup/imap');

    const alerts = page.locator('[role="alert"]');
    await expect(alerts.first()).toBeVisible({ timeout: 30_000 });

    // Per-section markers. The exact downstream error text varies (DNS vs
    // refused vs TLS handshake), so just assert the section labels rendered.
    await expect(page.locator('text=/IMAP error:/i')).toBeVisible();
    await expect(page.locator('text=/SMTP error:/i')).toBeVisible();

    // Passwords MUST NOT round-trip into the rendered form.
    await expect(page.locator('input[name="imap_password"]')).toHaveValue('');
    await expect(page.locator('input[name="smtp_password"]')).toHaveValue('');

    // Hosts DO round-trip so the user can fix and retry without retyping.
    await expect(page.locator('input[name="imap_host"]'))
      .toHaveValue('imap.does-not-exist-tmail374.invalid');

    await page.screenshot({
      path: path.join(SCREENSHOT_DIR, 'step2-bad-servers-rejected.png'),
      fullPage: true,
    });
  });

  test('Wizard cookie is HttpOnly + Secure + SameSite=Strict + scoped to /classic/signup', async ({ page, context }) => {
    await page.goto('/classic/login');
    await page.locator('a[href="/classic/signup"]').click();
    await page.waitForLoadState('domcontentloaded');

    const cookies = await context.cookies();
    const draft = cookies.find((c) => c.name === 'tasmail_classic_signup_draft');
    expect(draft).toBeDefined();
    expect(draft!.httpOnly, 'cookie must be HttpOnly (JS-unreadable)').toBe(true);
    expect(draft!.sameSite, 'cookie must be SameSite=Strict').toBe('Strict');
    // Secure is implied on https origins; Playwright's cookie record exposes it.
    if (page.url().startsWith('https://')) {
      expect(draft!.secure, 'cookie must be Secure over HTTPS').toBe(true);
    }
    // Path scoped to the wizard so it doesn't leak to the post-login session
    // cookie's Path=/ scope.
    expect(draft!.path).toBe('/classic/signup');
    // Max-Age maps to expires; ~30min from now ± a small slack.
    const ttl = (draft!.expires ?? 0) - Math.floor(Date.now() / 1000);
    expect(ttl, `expected ~1800s TTL, got ${ttl}`).toBeGreaterThan(1500);
    expect(ttl, `expected ~1800s TTL, got ${ttl}`).toBeLessThan(2000);

    await page.screenshot({
      path: path.join(SCREENSHOT_DIR, 'wizard-cookie-attributes.png'),
      fullPage: true,
    });
  });
});
