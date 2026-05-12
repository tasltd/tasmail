/**
 * BYOK signup → onboarding → /app E2E flow
 *
 * Real test bed: signs up a brand-new TASMail account, then attaches the
 * noreply@techatscale.io mailbox (Stalwart IMAP/SMTP at swmail.techatscale.io)
 * via the onboarding wizard. By the time the test reaches /app the backend has
 * proven it can actually reach the user's IMAP server.
 *
 * Each TASMail account is unique-per-test (timestamp suffix) so reruns don't
 * collide on the unique-email constraint.
 */
import { test, NOREPLY_CREDS } from '../fixtures/base.js';
import { expect } from '@playwright/test';

const ACCOUNT_PASSWORD = 'correct-horse-battery-staple-9k';

function freshEmail(label: string): string {
  // Use the noreply local-part as the @-prefix hint so the wizard auto-detects
  // the right preset isn't possible (the email domain controls that). Instead
  // we route the BYOK creds in manually after picking "Other / Custom".
  return `e2e-${label}-${Date.now()}@e2e.tasmail`;
}

test.describe('BYOK signup → onboarding → mailbox', () => {
  test('signup form rejects mismatched password confirmation', async ({ page, takeScreenshot }) => {
    await page.goto('/signup');
    await page.fill('#email', freshEmail('mismatch'));
    await page.fill('#password', 'one-password-here');
    await page.fill('#confirm', 'a-different-password');
    await page.click('button[type="submit"]');
    await expect(page.locator('.login-card__error')).toContainText('do not match');
    await takeScreenshot(page, 'byok/00-signup-password-mismatch');
  });

  test('signup creates account and lands on /onboarding', async ({ page, signupAs, takeScreenshot }) => {
    const email = freshEmail('signup');
    await signupAs(page, email, ACCOUNT_PASSWORD);
    await expect(page).toHaveURL(/\/onboarding/);
    // First step in the default (BYOK only) configuration is the provider picker.
    await expect(page.locator('h2', { hasText: /Who hosts your email|How do you want/ })).toBeVisible();
    await takeScreenshot(page, 'byok/01-onboarding-landed');
  });

  test('onboarding wizard provisions noreply@techatscale.io mailbox end-to-end', async ({
    page,
    signupAs,
    takeScreenshot,
  }) => {
    const email = freshEmail('byok-real');
    await signupAs(page, email, ACCOUNT_PASSWORD);

    // -------- Step 1: pick "Other / Custom" so we can punch in the real swmail host --------
    // (Auto-detect would only fire if our TASMail signup email matched a known provider domain.)
    if (await page.locator('h2:has-text("How do you want to use TASMail?")').isVisible().catch(() => false)) {
      // Path step is showing because dns_mx_onboarding_enabled was previously toggled.
      await page.locator('button.onboarding-path:has-text("Connect an existing account")').click();
    }
    await expect(page.locator('h2:has-text("Who hosts your email?")')).toBeVisible();
    await takeScreenshot(page, 'byok/02-provider-picker');

    await page.locator('button.onboarding-provider--custom').click();
    await expect(page.locator('h2:has-text("IMAP server")')).toBeVisible();
    await takeScreenshot(page, 'byok/03-imap-step-empty');

    // -------- Step 2: IMAP form using the real swmail.techatscale.io credentials --------
    const imapHost = page.locator('input[placeholder^="imap."]');
    await imapHost.fill(NOREPLY_CREDS.imap.host);
    await page.locator('input[type="number"]').fill(String(NOREPLY_CREDS.imap.port));
    await page.locator('select').first().selectOption(NOREPLY_CREDS.imap.encryption);
    await page.locator('input[placeholder*="full email"]').fill(NOREPLY_CREDS.imap.username);
    await page.locator('input[type="password"]').first().fill(NOREPLY_CREDS.imap.password);
    await takeScreenshot(page, 'byok/04-imap-step-filled');

    // Test connection — verifies the backend can actually reach swmail and LOGIN.
    await page.locator('button.btn--ghost', { hasText: 'Test connection' }).click();
    await expect(page.locator('.onboarding-test-result')).toBeVisible({ timeout: 15_000 });
    const testResultText = await page.locator('.onboarding-test-result').textContent();
    await takeScreenshot(page, 'byok/05-imap-test-result');
    expect(testResultText, `IMAP test against ${NOREPLY_CREDS.imap.host} should succeed`).toContain('IMAP login succeeded');

    // Save and continue to the SMTP step.
    await page.locator('button.btn--primary', { hasText: 'Save & continue' }).click();
    await expect(page.locator('h2:has-text("SMTP server")')).toBeVisible({ timeout: 10_000 });
    await takeScreenshot(page, 'byok/06-smtp-step-empty');

    // -------- Step 3: SMTP form, same mailbox (port 465 SSL) --------
    await page.locator('input[placeholder^="smtp."]').fill(NOREPLY_CREDS.smtp.host);
    await page.locator('input[type="number"]').fill(String(NOREPLY_CREDS.smtp.port));
    await page.locator('select').first().selectOption(NOREPLY_CREDS.smtp.encryption);
    await page.locator('input[placeholder*="full email"]').fill(NOREPLY_CREDS.smtp.username);
    await page.locator('input[type="password"]').first().fill(NOREPLY_CREDS.smtp.password);
    await takeScreenshot(page, 'byok/07-smtp-step-filled');

    // Finish setup — backend writes smtp_configurations row and the wizard transitions to /app.
    await page.locator('button.btn--primary', { hasText: 'Finish setup' }).click();
    await page.waitForURL(/\/app/, { timeout: 20_000 });
    await page.waitForLoadState('networkidle');
    await takeScreenshot(page, 'byok/08-arrived-at-app');
  });
});
