/**
 * Admin feature-flags dashboard E2E
 *
 * Validates that the runtime-toggle dashboard at /admin/feature-flags renders
 * all 6 seeded flags, that toggling one persists via PATCH, and that the public
 * /api/feature-flags endpoint reflects the change.
 *
 * Always restores the dns_mx_onboarding_enabled flag back to false after the
 * mutation test so other specs see a clean default.
 */
import { test } from '../fixtures/base.js';
import { expect } from '@playwright/test';

const ACCOUNT_PASSWORD = 'correct-horse-battery-staple-9k';

test.describe('Admin feature-flags dashboard', () => {
  test('renders six seeded flags with name, key, description, badge, toggle', async ({
    page,
    apiSignup,
    takeScreenshot,
    baseURL,
  }) => {
    // Bootstrap an authenticated session via the API so the page can render
    // without going through the login form (login form would also work but
    // adds noise to this spec).
    const tokens = await apiSignup(`flagsui-${Date.now()}@e2e.tasmail`, ACCOUNT_PASSWORD);
    await page.goto('/login'); // any same-origin page so localStorage writes apply
    await page.evaluate(([at, rt]) => {
      localStorage.setItem('access_token', at);
      localStorage.setItem('refresh_token', rt);
    }, [tokens.access_token, tokens.refresh_token]);

    await page.goto('/admin/feature-flags');
    await page.waitForLoadState('networkidle');
    await expect(page.locator('h1', { hasText: 'Feature flags' })).toBeVisible();
    await expect(page.locator('.ff-row')).toHaveCount(6);
    await takeScreenshot(page, 'admin-flags/01-dashboard-loaded');

    // The DNS-MX flag should have a "public" badge (it's marked is_public=true in seed data).
    const dnsRow = page.locator('.ff-row', {
      has: page.locator('code', { hasText: 'dns_mx_onboarding_enabled' }),
    });
    await expect(dnsRow.locator('.ff-badge--public')).toBeVisible();
    await takeScreenshot(page, 'admin-flags/02-dns-mx-row');

    // baseURL is exposed by Playwright; just sanity-check we're hitting prod.
    expect(baseURL).toContain('mail.techatscale.io');
  });

  test('toggling a flag persists and is visible on /api/feature-flags', async ({
    page,
    apiSignup,
    takeScreenshot,
    baseURL,
  }) => {
    const tokens = await apiSignup(`flagstoggle-${Date.now()}@e2e.tasmail`, ACCOUNT_PASSWORD);
    await page.goto('/login');
    await page.evaluate(([at, rt]) => {
      localStorage.setItem('access_token', at);
      localStorage.setItem('refresh_token', rt);
    }, [tokens.access_token, tokens.refresh_token]);
    await page.goto('/admin/feature-flags');
    await page.waitForLoadState('networkidle');

    const dnsRow = page.locator('.ff-row', {
      has: page.locator('code', { hasText: 'dns_mx_onboarding_enabled' }),
    });
    const checkbox = dnsRow.locator('input[type="checkbox"]');
    const initial = await checkbox.isChecked();

    await dnsRow.locator('.ff-switch').click();
    await page.waitForTimeout(800); // allow the optimistic + PATCH round-trip
    await takeScreenshot(page, 'admin-flags/03-after-toggle');
    expect(await checkbox.isChecked()).toBe(!initial);

    // Validate the change is visible on the public endpoint (cache invalidation works).
    const publicResp = await fetch(`${baseURL}/api/feature-flags`);
    const flags = (await publicResp.json()) as Array<{ key: string; enabled: boolean }>;
    const dns = flags.find((f) => f.key === 'dns_mx_onboarding_enabled');
    expect(dns?.enabled).toBe(!initial);

    // Restore so other specs and dev users see the default state.
    await dnsRow.locator('.ff-switch').click();
    await page.waitForTimeout(500);
    expect(await checkbox.isChecked()).toBe(initial);
    await takeScreenshot(page, 'admin-flags/04-restored');
  });
});
