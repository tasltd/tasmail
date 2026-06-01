// Changed (TMAIL-409): rewritten for the SettingsHub layout introduced in
// TMAIL-399. Signatures / Two-Factor (Security) / Vacation / Filters /
// Templates / Spam Filter no longer live as top-level sidebar entries — they
// moved behind the Settings gear → /app/settings/{category}/{section}.
// Contacts moved into the "apps" group (data-nav-key="contacts-app").
//
// Also drops the mocked /api/auth/login flow in favour of real apiSignup so
// /app actually renders (TMAIL-408 cascade — loginAs('user@example.com') just
// 401'd because no such user exists in the BYOK DB).
import { test, expect } from './fixtures/base';
import type { Page } from '@playwright/test';
import { deleteMailboxByUsername } from './helpers/db-cleanup.js';

const ACCOUNT_PASSWORD = 'nav-settings-tmail409-2026';

// PURPOSE: inject the JWT pair and land on /app — only direct goto allowed by
// the E2E navigation rule (same pattern as byok-first-time-walkthrough.spec.ts).
async function injectAndLand(
  page: Page,
  tokens: { access_token: string; refresh_token: string },
): Promise<void> {
  await page.goto('/login');
  await page.evaluate(
    ([at, rt]) => {
      localStorage.setItem('access_token', at);
      localStorage.setItem('refresh_token', rt);
    },
    [tokens.access_token, tokens.refresh_token],
  );
  await page.goto('/app');
  // Sidebar Compose button is the cheapest "app shell is mounted" sentinel.
  await expect(
    page.locator('button.btn--compose', { hasText: /Compose/i }).first(),
  ).toBeVisible({ timeout: 20_000 });
}

test.describe('Settings Navigation (SettingsHub — TMAIL-399/409)', () => {
  // Track every email we create so afterAll wipes them — runId keeps re-runs
  // idempotent without coordination.
  const runId = Date.now();
  const createdEmails: string[] = [];

  test.afterAll(async () => {
    for (const email of createdEmails) {
      try {
        deleteMailboxByUsername(email);
      } catch {
        // Best-effort cleanup; don't fail teardown.
      }
    }
  });

  test('Settings gear opens SettingsHub on Mail → Signatures', async ({
    page,
    apiSignup,
    takeScreenshot,
  }) => {
    test.setTimeout(90_000);
    const email = `nav-sig-${runId}@e2e.tasmail`;
    createdEmails.push(email);
    const tokens = await apiSignup(email, ACCOUNT_PASSWORD);
    await injectAndLand(page, tokens);
    await takeScreenshot(page, 'navigation/settings-app-landing');

    // ── Click the Sidebar Settings entry (registry-driven; TMAIL-398).
    const settingsEntry = page.locator('[data-nav-key="settings-hub"]');
    await expect(settingsEntry).toBeVisible({ timeout: 10_000 });
    await settingsEntry.click();
    await page.waitForURL(/\/app\/settings(\/.*)?$/, { timeout: 10_000 });

    // ── SettingsHub is mounted, then drive Mail → Signatures.
    await expect(page.getByTestId('settings-hub')).toBeVisible();
    await page.getByTestId('settings-category-mail').click();
    const signaturesSection = page.getByTestId('settings-section-signatures');
    await expect(signaturesSection).toBeVisible({ timeout: 5_000 });
    await signaturesSection.click();
    await page.waitForURL(/\/app\/settings\/mail\/signatures$/, { timeout: 10_000 });

    // ── Pane swaps to the Signatures section (lazy chunk loaded).
    await expect(page.getByTestId('settings-hub-pane')).toHaveAttribute(
      'data-section',
      'signatures',
      { timeout: 10_000 },
    );
    // SignatureManager renders the "Email Signatures" heading once its
    // /api/signatures fetch resolves (empty array on a fresh account is fine).
    await expect(
      page.locator('.signature-manager h2', { hasText: 'Email Signatures' }),
    ).toBeVisible({ timeout: 10_000 });
    await takeScreenshot(page, 'navigation/settings-mail-signatures');
  });

  test('Apps row Contacts entry opens the Contacts app', async ({
    page,
    apiSignup,
    takeScreenshot,
  }) => {
    test.setTimeout(90_000);
    const email = `nav-contacts-${runId}@e2e.tasmail`;
    createdEmails.push(email);
    const tokens = await apiSignup(email, ACCOUNT_PASSWORD);
    await injectAndLand(page, tokens);

    // ── Click the Contacts entry in the apps group (TMAIL-398 registry).
    //    Contacts is NOT inside SettingsHub — it's a full-page app.
    const contactsEntry = page.locator('[data-nav-key="contacts-app"]');
    await expect(contactsEntry).toBeVisible({ timeout: 10_000 });
    await contactsEntry.click();
    // viewMode-driven nav item — sets the contacts-app viewMode inside
    // AppShell. The button gets folder-item--active and the contacts pane
    // renders in-place (no route change).
    await expect(contactsEntry).toHaveClass(/folder-item--active/);
    // ContactsApp renders the "Contacts" h3 heading + "All Contacts" button
    // once /api/contacts resolves. Both are always-visible (the import
    // textarea only appears after clicking Import — TMAIL-119).
    await expect(
      page.locator('.settings-panel h3', { hasText: /^Contacts$/ }),
    ).toBeVisible({ timeout: 15_000 });
    await expect(
      page.locator('button.folder-item', { hasText: /All Contacts/ }),
    ).toBeVisible();
    await takeScreenshot(page, 'navigation/contacts-view');
  });

  test('Settings → Account & Security renders Two-Factor pane', async ({
    page,
    apiSignup,
    takeScreenshot,
  }) => {
    test.setTimeout(90_000);
    const email = `nav-sec-${runId}@e2e.tasmail`;
    createdEmails.push(email);
    const tokens = await apiSignup(email, ACCOUNT_PASSWORD);
    await injectAndLand(page, tokens);

    // Click Settings gear → /app/settings/account/security (Account & Security
    // is the first category and Two-Factor is its first section, so the hub
    // lands there by default).
    await page.locator('[data-nav-key="settings-hub"]').click();
    await page.waitForURL(/\/app\/settings(\/.*)?$/, { timeout: 10_000 });

    await page.getByTestId('settings-category-account').click();
    const securitySection = page.getByTestId('settings-section-security');
    await expect(securitySection).toBeVisible({ timeout: 5_000 });
    await securitySection.click();
    await page.waitForURL(/\/app\/settings\/account\/security$/, { timeout: 10_000 });

    await expect(page.getByTestId('settings-hub-pane')).toHaveAttribute(
      'data-section',
      'security',
      { timeout: 10_000 },
    );
    // TwoFactorManager renders an h2 with the "Two-Factor Authentication"
    // heading at the top of its pane.
    await expect(
      page.locator('h2', { hasText: 'Two-Factor Authentication' }),
    ).toBeVisible({ timeout: 10_000 });
    await takeScreenshot(page, 'navigation/security-view');
  });

  test('deep-link /app/settings/mail/filters survives reload', async ({
    page,
    apiSignup,
    takeScreenshot,
  }) => {
    test.setTimeout(90_000);
    const email = `nav-flow-${runId}@e2e.tasmail`;
    createdEmails.push(email);
    const tokens = await apiSignup(email, ACCOUNT_PASSWORD);
    await injectAndLand(page, tokens);

    // ── Navigate to Mail → Filters via the menu (no direct page.goto for
    //    internal routes — that's the deep-link feature we're validating).
    await page.locator('[data-nav-key="settings-hub"]').click();
    await page.waitForURL(/\/app\/settings(\/.*)?$/, { timeout: 10_000 });
    await page.getByTestId('settings-category-mail').click();
    const filtersSection = page.getByTestId('settings-section-filters');
    await expect(filtersSection).toBeVisible({ timeout: 5_000 });
    await filtersSection.click();
    await page.waitForURL(/\/app\/settings\/mail\/filters$/, { timeout: 10_000 });
    await expect(page.getByTestId('settings-hub-pane')).toHaveAttribute(
      'data-section',
      'filters',
      { timeout: 10_000 },
    );
    await takeScreenshot(page, 'navigation/settings-flow-filters');

    // ── Reload — the SettingsHub parses :category/:section out of the URL,
    //    so a page refresh must keep us on the same Filters pane (this is
    //    what makes deep-linkable settings useful at all).
    await page.reload();
    await expect(page).toHaveURL(/\/app\/settings\/mail\/filters$/);
    await expect(page.getByTestId('settings-hub-pane')).toHaveAttribute(
      'data-section',
      'filters',
      { timeout: 10_000 },
    );
    // The Mail category tab should still be the active one in the left rail.
    await expect(page.getByTestId('settings-category-mail')).toHaveAttribute(
      'data-active',
      'true',
    );
    await takeScreenshot(page, 'navigation/settings-flow-filters-after-reload');

    // ── Switch to a different category (Account & Security) to prove the
    //    SettingsHub left rail still drives navigation after a reload.
    //    Fresh BYOK users have no /api/folders payload yet, so clicking the
    //    INBOX row isn't reliable here — staying inside SettingsHub keeps
    //    the assertion crisp.
    await page.getByTestId('settings-category-account').click();
    await page.waitForURL(/\/app\/settings\/account\/security$/, { timeout: 10_000 });
    await expect(page.getByTestId('settings-hub-pane')).toHaveAttribute(
      'data-section',
      'security',
      { timeout: 10_000 },
    );
    await takeScreenshot(page, 'navigation/settings-flow-back-to-security');
  });
});
