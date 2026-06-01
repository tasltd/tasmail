// Added (TMAIL-422): Single source of truth for sidebar + SettingsHub navigation
// inside E2E specs.
//
// Why this file exists:
//   TMAIL-398 swapped the flat 41-button sidebar for a registry-driven layout
//   (mail → apps → settings → admin) and TMAIL-399 moved every settings panel
//   behind a Gmail-style SettingsHub at /app/settings/:category/:section. The
//   "Security" entry that the MFA + Passkey specs used to click (a top-level
//   `.folder-item:has-text("Security")`) no longer exists — Security now lives
//   under Settings → Account & Security → Two-Factor Auth.
//
//   Per the E2E HARD RULES navigation must happen through menu clicks, never
//   `page.goto('/app/settings/...')` for internal routes. Encoding the menu
//   path here means future sidebar renames (TMAIL-398 cascade) hit ONE file
//   instead of every spec that touches Security.
//
// Selectors used:
//   * Sidebar Settings entry:
//       `.folder-item[data-nav-key="settings-hub"]`
//     (set in Sidebar.tsx via the nav-registry; also carries data-tour="settings"
//     for the first-login tour). The label-based fallback
//     `.folder-item:has-text("Settings")` works too but couples specs to copy.
//   * SettingsHub category tab + section button:
//       `[data-testid="settings-category-account"]`
//       `[data-testid="settings-section-security"]`
//     (set in SettingsHub.tsx — these are the durable test hooks.)
//
// Both helpers wait for the Two-Factor Authentication heading because that's
// the first thing the TwoFactorManager renders inside the Security section,
// and the spec assertions immediately downstream rely on it being visible.

import { expect, type Page } from '@playwright/test';

// Sidebar nav-key for the Settings entry in nav-registry.ts. Centralising it
// here keeps a future rename of the registry key from breaking every spec.
export const SETTINGS_NAV_KEY = 'settings-hub';

// SettingsHub testid prefixes. Match the data-testid attributes set in
// SettingsHub.tsx and settings-hub-registry.ts.
export const SETTINGS_CATEGORY_TESTID = (id: string) => `settings-category-${id}`;
export const SETTINGS_SECTION_TESTID = (id: string) => `settings-section-${id}`;

/**
 * Click the Settings entry in the sidebar and wait for the SettingsHub to mount.
 *
 * Use this when a spec only needs to land on /app/settings — the default
 * category (account) + default section (security) will auto-select.
 */
export async function openSettingsHub(page: Page): Promise<void> {
  // Sidebar must be present — caller is responsible for getting to /app first
  // (signupAndStashToken / loginViaUI helpers seed the token + navigate).
  await page.waitForSelector('.sidebar', { timeout: 15_000 });

  // data-nav-key is the durable hook; visible label "Settings" is a fallback.
  const settingsEntry = page
    .locator(`.folder-item[data-nav-key="${SETTINGS_NAV_KEY}"]`)
    .or(page.locator('.folder-item:has-text("Settings")'));
  await expect(settingsEntry).toBeVisible({ timeout: 10_000 });
  await settingsEntry.first().click();

  // SettingsHub mounts with the rail visible — wait for the rail testid so we
  // don't race the lazy-loaded section component.
  await expect(page.getByTestId('settings-hub-rail')).toBeVisible({ timeout: 10_000 });
}

/**
 * Navigate Sidebar Settings → Account & Security → Two-Factor Auth (Security panel).
 *
 * Returns once the Two-Factor Authentication h2 is visible, which is the
 * canonical "ready to assert" signal for MFA / passkey / SMS-OTP specs.
 *
 * Defensive: the SettingsHub already defaults to account/security, so the
 * category/section clicks are idempotent — but issuing them explicitly keeps
 * the test resilient if a future change re-orders the registry.
 */
export async function gotoSettingsSecurity(page: Page): Promise<void> {
  await openSettingsHub(page);

  // Pick the Account & Security category, then the Two-Factor Auth section.
  // The hub will already have these selected by default, but clicking them
  // explicitly defends against a future re-ordering of the registry.
  const accountTab = page.getByTestId(SETTINGS_CATEGORY_TESTID('account'));
  await expect(accountTab).toBeVisible({ timeout: 10_000 });
  await accountTab.click();

  const securitySection = page.getByTestId(SETTINGS_SECTION_TESTID('security'));
  await expect(securitySection).toBeVisible({ timeout: 10_000 });
  await securitySection.click();

  // TwoFactorManager mounts with its h2 — wait for it before returning so
  // downstream `.locator('button:has-text("Enable 2FA")')` calls don't race
  // the lazy import.
  await expect(page.locator('h2', { hasText: 'Two-Factor Authentication' })).toBeVisible({
    timeout: 15_000,
  });
}
