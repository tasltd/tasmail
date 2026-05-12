/**
 * Landing page E2E
 *
 * Validates the public marketing surface at https://mail.techatscale.io/:
 *   - hero renders with the right title, badge, and CTAs
 *   - feature grid, pricing tiles, deploy snippet, and footer all paint
 *   - "Sign in" / "Get started" / "Create your account" CTAs route to /login and /signup
 *   - catch-all routes redirect back to "/"
 *
 * Screenshots are captured at every assertion point per the HARD RULE.
 */
import { test } from '../fixtures/base.js';
import { expect } from '@playwright/test';

test.describe('Landing page', () => {
  test('renders hero, features, pricing, deploy, and footer', async ({ page, takeScreenshot }) => {
    await page.goto('/');
    await page.waitForLoadState('networkidle');

    await expect(page.locator('.landing-hero__title')).toContainText('One');
    await expect(page.locator('.landing-hero__title')).toContainText('webmail UI');
    await expect(page.locator('.landing-hero__badge')).toContainText('Bring your own IMAP');
    await takeScreenshot(page, 'landing/01-hero-loaded');

    // Six feature cards (one per FeatureCard in LandingPage.tsx)
    await expect(page.locator('.landing-feature')).toHaveCount(6);
    await page.locator('#features').scrollIntoViewIfNeeded();
    await takeScreenshot(page, 'landing/02-features-grid');

    // Three pricing tiers
    await expect(page.locator('.landing-price-card')).toHaveCount(3);
    await page.locator('#pricing').scrollIntoViewIfNeeded();
    await takeScreenshot(page, 'landing/03-pricing-tiers');

    // Deploy section + footer
    await page.locator('#deploy').scrollIntoViewIfNeeded();
    await expect(page.locator('.landing-deploy__code')).toContainText('git clone');
    await takeScreenshot(page, 'landing/04-deploy-snippet');

    await page.locator('.landing-footer').scrollIntoViewIfNeeded();
    await expect(page.locator('.landing-footer__copy')).toContainText('Tech at Scale');
    await takeScreenshot(page, 'landing/05-footer');
  });

  test('"Get started" CTA navigates to /signup', async ({ page, takeScreenshot }) => {
    await page.goto('/');
    await page.locator('a.landing-btn--primary', { hasText: 'Create your account' }).first().click();
    await page.waitForURL(/\/signup$/);
    await expect(page.locator('#email')).toBeVisible();
    await takeScreenshot(page, 'landing/06-cta-to-signup');
  });

  test('"Sign in" CTA navigates to /login', async ({ page, takeScreenshot }) => {
    await page.goto('/');
    await page.locator('a.landing-btn--ghost', { hasText: 'Sign in' }).first().click();
    await page.waitForURL(/\/login$/);
    await expect(page.locator('#username')).toBeVisible();
    await takeScreenshot(page, 'landing/07-cta-to-login');
  });

  test('unknown route falls back to landing', async ({ page, takeScreenshot }) => {
    await page.goto('/this-route-does-not-exist');
    await page.waitForURL((url) => url.pathname === '/');
    await expect(page.locator('.landing-hero__title')).toBeVisible();
    await takeScreenshot(page, 'landing/08-catchall-redirect');
  });
});
