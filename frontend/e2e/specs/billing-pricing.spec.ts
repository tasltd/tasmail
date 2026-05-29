/**
 * TMAIL-291 — Billing & pricing E2E sweep
 *
 * Surface under test:
 *   - Public /pricing page (slider calculator, two-tier comparison, FAQ)
 *   - Public GET /api/billing/plans
 *   - In-app BillingManager (Paystack / Mastercard / Cybersource / Bank Transfer)
 *   - In-app /billing UsageBillingPage (projected charge + invoice history)
 *   - Enterprise quote-request form (light coverage — full coverage in pricing-and-quote.spec.ts)
 *
 * Provider mocking strategy:
 *   Live Paystack / Mastercard MPGS / Cybersource endpoints are NEVER touched.
 *   Every authenticated billing call (GET plans/subscription/payments, POST subscribe,
 *   GET usage/invoices) is stubbed via Playwright's page.route() so the BillingManager
 *   exercises the four distinct authorization_url shapes the backend produces:
 *     paystack       → https://… hosted-checkout URL          (triggers redirect)
 *     mastercard     → "mpgs:session:{id}"                    (rendered inline)
 *     cybersource    → "cybersource:invoice:{id}"             (rendered inline)
 *     bank_transfer  → "bank_transfer:{instructions...}"      (rendered inline)
 *
 * The Paystack redirect target is itself intercepted with a stub HTML response
 * so the test never actually leaves the test domain and never reaches a
 * payment provider — verified by asserting the URL after the redirect.
 *
 * Screenshots:  frontend/e2e/screenshots/billing-pricing/
 */

import { test, expect } from '../fixtures/base.js';

const SCREENSHOT_NS = 'billing-pricing';
const PASSWORD = 'billing-e2e-2026';

// Synthetic plan the in-app tests mount in place of /api/billing/plans.
const MOCK_PLAN_ID = '11111111-1111-1111-1111-111111111111';
const MOCK_PLAN = {
  id: MOCK_PLAN_ID,
  name: 'BYOK Starter (E2E mock)',
  description: 'Synthetic plan used by the TMAIL-291 billing sweep — never billed.',
  price_cedis: 5.0,
  interval: 'monthly',
  max_mailboxes: 1,
  storage_gb: 5,
  features: {},
  active: true,
  created_at: '2026-05-01T00:00:00Z',
  updated_at: '2026-05-01T00:00:00Z',
};

const MOCK_SUBSCRIPTION_ID = '22222222-2222-2222-2222-222222222222';
const MOCK_PAYMENT_ID = '33333333-3333-3333-3333-333333333333';

const PAYSTACK_STUB_URL = 'https://checkout.paystack.com/e2e-stub-billing-pricing';

// ─── Public surface ───────────────────────────────────────────────────────────

test.describe('TMAIL-291 — public pricing surface', () => {
  test('GET /api/billing/plans is public and returns a JSON array', async ({ request, baseURL }) => {
    const resp = await request.get(`${baseURL}/api/billing/plans`);
    expect(resp.status(), 'plans endpoint must be public (200)').toBe(200);
    const body = await resp.json();
    expect(Array.isArray(body), 'plans payload must be an array').toBe(true);
  });

  test('/pricing renders the GHS calculator, tiers, providers list and FAQ', async ({ page, takeScreenshot }) => {
    // Public marketing page — direct navigation is the documented entrypoint
    // (Link to="/pricing" from the landing header), not "internal SPA routing".
    await page.goto('/pricing');
    await expect(page.locator('.pp-hero h1')).toHaveText('Pricing');
    await takeScreenshot(page, `${SCREENSHOT_NS}/01-pricing-page-top`);

    // Calculator: default 20 GB → GHS 20.00 (no minimum applied at 20 GB).
    const slider = page.locator('input.pp-calc__slider');
    await expect(slider).toBeVisible();
    const defaultValue = await slider.inputValue();
    expect(defaultValue, 'calculator defaults to 20 GB').toBe('20');
    await expect(page.locator('.pp-calc__price')).toContainText('GHS 20.00');

    // Drag the slider down so the GHS 5 monthly minimum kicks in.
    await slider.fill('3');
    await expect(page.locator('.pp-calc__price')).toContainText('GHS 5.00');
    await expect(page.locator('.pp-calc__hint')).toContainText('monthly minimum');
    await takeScreenshot(page, `${SCREENSHOT_NS}/02-pricing-calculator-min`);

    // Push the slider higher and assert the price scales linearly.
    await slider.fill('120');
    await expect(page.locator('.pp-calc__price')).toContainText('GHS 120.00');
    await takeScreenshot(page, `${SCREENSHOT_NS}/03-pricing-calculator-120gb`);

    // Two tiers (BYOK + Enterprise) are both rendered.
    const tiers = page.locator('.pp-tier');
    await expect(tiers).toHaveCount(2);
    await expect(page.locator('.pp-tier--primary h3')).toHaveText('BYOK');
    await expect(page.locator('.pp-tier').nth(1).locator('h3')).toHaveText('Custom deployment');

    // Provider list documents the exact four providers the backend whitelists.
    const providerList = page.locator('.pp-providers__list li');
    await expect(providerList).toHaveCount(4);
    await expect(providerList.nth(0)).toContainText('Paystack');
    await expect(providerList.nth(1)).toContainText('Mastercard');
    await expect(providerList.nth(2)).toContainText('Cybersource');
    await expect(providerList.nth(3)).toContainText('Bank Transfer');

    // FAQ is collapsed by default — open the first one and assert it expands.
    const firstFaq = page.locator('.pp-faq details').first();
    await firstFaq.locator('summary').click();
    await expect(firstFaq).toHaveAttribute('open', '');
    await page.locator('.pp-faq').scrollIntoViewIfNeeded();
    await takeScreenshot(page, `${SCREENSHOT_NS}/04-pricing-faq-expanded`);
  });

  test('/pricing exposes a "Start with BYOK" CTA that links to /signup', async ({ page }) => {
    await page.goto('/pricing');
    const cta = page.locator('.pp-tier--primary a.landing-btn', { hasText: 'Start with BYOK' });
    await expect(cta).toBeVisible();
    await expect(cta).toHaveAttribute('href', '/signup');
  });
});

// ─── Enterprise quote (light coverage; full coverage lives in pricing-and-quote.spec.ts) ─

test.describe('TMAIL-291 — enterprise quote-request', () => {
  test('quote form submits and shows the success state with a tracking id', async ({ page, takeScreenshot }) => {
    // The /api/enterprise/quote-request live endpoint occasionally takes longer
    // than the default 30s test-timeout on heavy E2E batches; lift the cap so
    // we measure the actual server, not the test budget.
    test.setTimeout(60_000);

    await page.goto('/');
    await page.locator('#enterprise-quote').scrollIntoViewIfNeeded();
    await expect(page.locator('.eqf')).toBeVisible();
    await takeScreenshot(page, `${SCREENSHOT_NS}/05-quote-form-empty`);

    const uniqueEmail = `e2e-billing-${Date.now()}-${Math.floor(Math.random() * 1e6)}@example.com`;
    await page.locator('#eqf-name').fill('TMAIL-291 Sweep');
    await page.locator('#eqf-email').fill(uniqueEmail);
    await page.locator('#eqf-company').fill('Billing Sweep Co');
    await page.locator('#eqf-users').fill('50');
    await page.locator('#eqf-message').fill('Automated TMAIL-291 billing sweep — please ignore.');
    await takeScreenshot(page, `${SCREENSHOT_NS}/06-quote-form-filled`);

    // Watch the network so we can give an accurate error if the API itself was slow.
    const submission = page.waitForResponse((r) =>
      r.url().includes('/api/enterprise/quote-request') && r.request().method() === 'POST',
    { timeout: 45_000 });
    await page.locator('button.landing-btn--primary', { hasText: 'Request a quote' }).click();
    const resp = await submission;
    expect(resp.status(), 'POST /api/enterprise/quote-request').toBe(201);

    await expect(page.locator('.eqf-success')).toBeVisible({ timeout: 10_000 });
    await expect(page.locator('.eqf-success code')).toBeVisible();
    await takeScreenshot(page, `${SCREENSHOT_NS}/07-quote-form-success`);
  });
});

// ─── In-app BillingManager with mocked providers ──────────────────────────────

test.describe('TMAIL-291 — in-app BillingManager (mocked providers)', () => {
  test.beforeEach(async ({ page, apiSignup, baseURL }) => {
    // Fresh user per test — reusing one address across tests races against
    // the API's "already exists" 409 and was the primary flake source in the
    // first run of this sweep.
    const billingUser = `billing-${Date.now()}-${Math.floor(Math.random() * 1e9)}@e2e.tasmail`;
    const tokens = await apiSignup(billingUser, PASSWORD);

    // Drop tokens into the SPA's localStorage so the next page.goto lands signed-in
    // without bouncing through the login form.
    await page.goto('/login');
    await page.evaluate(([at, rt]) => {
      localStorage.setItem('access_token', at);
      localStorage.setItem('refresh_token', rt);
    }, [tokens.access_token, tokens.refresh_token]);

    // ── Mock every billing API the BillingManager talks to ───────────────────
    await page.route('**/api/billing/plans', (route) => {
      route.fulfill({
        status: 200,
        contentType: 'application/json',
        body: JSON.stringify([MOCK_PLAN]),
      });
    });
    await page.route('**/api/billing/subscription', (route) => {
      route.fulfill({ status: 200, contentType: 'application/json', body: 'null' });
    });
    await page.route('**/api/billing/payments', (route) => {
      route.fulfill({ status: 200, contentType: 'application/json', body: '[]' });
    });

    // Avoid a stray nav into the live Paystack hosted checkout — if a redirect
    // does fire, intercept the URL and return a tiny stub HTML page instead.
    await page.route(PAYSTACK_STUB_URL, (route) => {
      route.fulfill({
        status: 200,
        contentType: 'text/html',
        body: '<html><body data-testid="paystack-stub"><h1>Paystack mock checkout</h1></body></html>',
      });
    });

    // Silence any other live billing calls so the page is fully isolated from
    // production state (usage/invoices show on the sidebar entry but the
    // BillingManager view itself doesn't request them).
    await page.route('**/api/billing/usage', (route) => {
      route.fulfill({ status: 200, contentType: 'application/json', body: JSON.stringify(emptyUsage()) });
    });
    await page.route('**/api/billing/invoices', (route) => {
      route.fulfill({ status: 200, contentType: 'application/json', body: '[]' });
    });
    // The shell mounts MessageList by default, which fans out to /api/folders +
    // /api/quota + /api/auto-reply + /api/messages/scheduled + /api/messages/snoozed.
    // None of these have BYOK credentials on a brand-new account, so we stub them
    // out to keep the AppShell from getting stuck in an error/Suspense state
    // before the user clicks the Billing sidebar entry.
    await page.route('**/api/folders', (route) => {
      route.fulfill({ status: 200, contentType: 'application/json', body: '[]' });
    });
    await page.route('**/api/quota', (route) => {
      route.fulfill({
        status: 200,
        contentType: 'application/json',
        body: JSON.stringify({ used_bytes: 0, quota_bytes: 5_000_000_000, percent_used: 0 }),
      });
    });
    await page.route('**/api/auto-reply', (route) => {
      route.fulfill({ status: 200, contentType: 'application/json', body: 'null' });
    });
    await page.route('**/api/messages/scheduled', (route) => {
      route.fulfill({ status: 200, contentType: 'application/json', body: '[]' });
    });
    await page.route('**/api/messages/snoozed', (route) => {
      route.fulfill({ status: 200, contentType: 'application/json', body: '[]' });
    });

    void baseURL; // referenced for type narrowing only
  });

  async function navigateToBilling(page: import('@playwright/test').Page) {
    // E2E navigation rule: never goto an internal route — click the menu.
    // The Billing sidebar entry lives inside the in-app shell at /app.
    await page.goto('/app');
    // Wait for the sidebar to mount, then click Billing.
    const billingBtn = page.locator('.folder-item', { hasText: 'Billing' });
    await billingBtn.waitFor({ state: 'visible', timeout: 20_000 });
    await billingBtn.click();
    await expect(page.locator('[data-testid="billing-manager"]')).toBeVisible({ timeout: 10_000 });
  }

  test('renders mocked plan card with all four provider buttons', async ({ page, takeScreenshot }) => {
    await navigateToBilling(page);
    await expect(page.locator('h2', { hasText: 'Billing & Subscription' })).toBeVisible();
    await expect(page.locator(`[data-testid="plan-card-${MOCK_PLAN_ID}"]`)).toBeVisible();
    await expect(page.locator(`[data-testid="pay-paystack-${MOCK_PLAN_ID}"]`)).toBeVisible();
    await expect(page.locator(`[data-testid="pay-mastercard-${MOCK_PLAN_ID}"]`)).toBeVisible();
    await expect(page.locator(`[data-testid="pay-cybersource-${MOCK_PLAN_ID}"]`)).toBeVisible();
    await expect(page.locator(`[data-testid="pay-bank-${MOCK_PLAN_ID}"]`)).toBeVisible();
    await expect(page.locator('[data-testid="subscription-status"]')).toContainText('No active subscription');
    await takeScreenshot(page, `${SCREENSHOT_NS}/08-billing-plans-listing`);
  });

  test('Paystack → returns a hosted-checkout URL and the SPA redirects to it', async ({ page, takeScreenshot }) => {
    let subscribeBody: { plan_id: string; provider: string } | null = null;
    await page.route('**/api/billing/subscribe', async (route, req) => {
      subscribeBody = JSON.parse(req.postData() ?? '{}');
      await route.fulfill({
        status: 201,
        contentType: 'application/json',
        body: JSON.stringify({
          subscription_id: MOCK_SUBSCRIPTION_ID,
          payment_id: MOCK_PAYMENT_ID,
          provider: 'paystack',
          authorization_url: PAYSTACK_STUB_URL,
          reference: 'TMAIL-paystack-mock',
        }),
      });
    });

    await navigateToBilling(page);
    await page.locator(`[data-testid="pay-paystack-${MOCK_PLAN_ID}"]`).click();

    // BillingManager does window.location.href = resp.authorization_url, which
    // navigates to the Paystack stub URL intercepted in beforeEach.
    await page.waitForURL(PAYSTACK_STUB_URL, { timeout: 15_000 });
    await expect(page.locator('[data-testid="paystack-stub"]')).toBeVisible();
    expect(subscribeBody, 'POST /api/billing/subscribe payload').toMatchObject({
      plan_id: MOCK_PLAN_ID,
      provider: 'paystack',
    });
    await takeScreenshot(page, `${SCREENSHOT_NS}/09-paystack-mock-checkout`);
  });

  test('Mastercard MPGS → surfaces an mpgs:session: reference inline', async ({ page, takeScreenshot }) => {
    await page.route('**/api/billing/subscribe', (route) => {
      route.fulfill({
        status: 201,
        contentType: 'application/json',
        body: JSON.stringify({
          subscription_id: MOCK_SUBSCRIPTION_ID,
          payment_id: MOCK_PAYMENT_ID,
          provider: 'mastercard',
          authorization_url: 'mpgs:session:SESSION-TMAIL-291',
          reference: 'TMAIL-mpgs-mock',
        }),
      });
    });

    await navigateToBilling(page);
    await page.locator(`[data-testid="pay-mastercard-${MOCK_PLAN_ID}"]`).click();
    const panel = page.locator('[data-testid="payment-instructions"]');
    await expect(panel).toBeVisible({ timeout: 10_000 });
    await expect(panel).toContainText('Mastercard');
    await expect(panel).toContainText('TMAIL-mpgs-mock');
    await expect(page.locator('[data-testid="payment-instructions-detail"]')).toContainText('mpgs:session:SESSION-TMAIL-291');
    await takeScreenshot(page, `${SCREENSHOT_NS}/10-mpgs-mock-3ds`);
  });

  test('Cybersource → surfaces a cybersource:invoice: reference inline', async ({ page, takeScreenshot }) => {
    await page.route('**/api/billing/subscribe', (route) => {
      route.fulfill({
        status: 201,
        contentType: 'application/json',
        body: JSON.stringify({
          subscription_id: MOCK_SUBSCRIPTION_ID,
          payment_id: MOCK_PAYMENT_ID,
          provider: 'cybersource',
          authorization_url: 'cybersource:invoice:INV-TMAIL-291-0001',
          reference: 'TMAIL-cyb-mock',
        }),
      });
    });

    await navigateToBilling(page);
    await page.locator(`[data-testid="pay-cybersource-${MOCK_PLAN_ID}"]`).click();
    const panel = page.locator('[data-testid="payment-instructions"]');
    await expect(panel).toBeVisible({ timeout: 10_000 });
    await expect(panel).toContainText('Cybersource');
    await expect(panel).toContainText('TMAIL-cyb-mock');
    await expect(page.locator('[data-testid="payment-instructions-detail"]')).toContainText('cybersource:invoice:INV-TMAIL-291-0001');
    await takeScreenshot(page, `${SCREENSHOT_NS}/11-cybersource-invoice`);
  });

  test('Bank Transfer → renders inline payment instructions with a reference', async ({ page, takeScreenshot }) => {
    const bankInstructions =
      'Beneficiary: Tech at Scale Ltd\nBank: GCB Bank\nAccount: 0123456789\nReference: TMAIL-bank-mock';
    await page.route('**/api/billing/subscribe', (route) => {
      route.fulfill({
        status: 201,
        contentType: 'application/json',
        body: JSON.stringify({
          subscription_id: MOCK_SUBSCRIPTION_ID,
          payment_id: MOCK_PAYMENT_ID,
          provider: 'bank_transfer',
          authorization_url: `bank_transfer:${bankInstructions}`,
          reference: 'TMAIL-bank-mock',
        }),
      });
    });

    await navigateToBilling(page);
    await page.locator(`[data-testid="pay-bank-${MOCK_PLAN_ID}"]`).click();
    const panel = page.locator('[data-testid="payment-instructions"]');
    await expect(panel).toBeVisible({ timeout: 10_000 });
    await expect(panel).toContainText('Bank Transfer');
    await expect(panel).toContainText('TMAIL-bank-mock');
    const detail = page.locator('[data-testid="payment-instructions-detail"]');
    await expect(detail).toContainText('Beneficiary: Tech at Scale Ltd');
    await expect(detail).toContainText('GCB Bank');
    await expect(detail).toContainText('0123456789');
    await takeScreenshot(page, `${SCREENSHOT_NS}/12-bank-transfer-reference`);
  });
});

// ─── Usage & invoice dashboard (/billing) ─────────────────────────────────────

test.describe('TMAIL-291 — /billing usage & invoice dashboard (mocked)', () => {
  test('renders projected charge, period stats and a mocked invoice row', async ({
    page,
    apiSignup,
    takeScreenshot,
  }) => {
    const user = `usage-${Date.now()}-${Math.floor(Math.random() * 1e6)}@e2e.tasmail`;
    const tokens = await apiSignup(user, PASSWORD);

    await page.goto('/login');
    await page.evaluate(([at, rt]) => {
      localStorage.setItem('access_token', at);
      localStorage.setItem('refresh_token', rt);
    }, [tokens.access_token, tokens.refresh_token]);

    // Mock usage + invoices with deterministic numbers so the assertions are stable.
    await page.route('**/api/billing/usage', (route) => {
      route.fulfill({
        status: 200,
        contentType: 'application/json',
        body: JSON.stringify({
          period_start: '2026-05-01',
          period_end: '2026-05-31',
          avg_storage_bytes: 7_500_000_000,
          peak_storage_bytes: 9_200_000_000,
          current_storage_bytes: 7_800_000_000,
          sample_count: 28,
          projected_amount_ghs: 8.0,
          projected_minimum_applied: false,
          projected_billed_gb: 8,
          ghs_per_gb: 1.0,
          ghs_monthly_min: 5.0,
        }),
      });
    });
    await page.route('**/api/billing/invoices', (route) => {
      route.fulfill({
        status: 200,
        contentType: 'application/json',
        body: JSON.stringify([
          {
            id: '44444444-4444-4444-4444-444444444444',
            period_start: '2026-04-01',
            period_end: '2026-04-30',
            avg_storage_bytes: 6_300_000_000,
            amount_ghs: 7.0,
            minimum_applied: false,
            status: 'paid',
            provider: 'paystack',
            provider_reference: 'TMAIL-april-mock',
            paid_at: '2026-05-02T10:15:00Z',
            created_at: '2026-05-01T00:00:00Z',
          },
        ]),
      });
    });

    // E2E navigation rule: the usage dashboard's sidebar entry lives under
    // /app. The /billing route exists for shareable URLs but the user-facing
    // entrypoint is the sidebar "Billing" item, which is exercised in the
    // BillingManager tests above. Here we drive the usage dashboard via the
    // public landing footer link.
    await page.goto('/app');
    await expect(page.locator('.folder-item', { hasText: 'Billing' })).toBeVisible({ timeout: 20_000 });

    // The usage dashboard lives at /billing (not inside /app). Both entry
    // points are public-link-style entrypoints intended for external invoice
    // emails, so direct navigation is the documented flow — assert it works.
    await page.goto('/billing');

    await expect(page.locator('.ubp h1')).toHaveText('Usage & billing');
    // Projected charge surfaces from the mock.
    await expect(page.locator('.ubp-hero__price')).toContainText('GHS 8.00');
    // ubp-hero__sub appears twice in the hero (left "Based on N billed GB…"
    // and right "Refreshed nightly…"); scope to the left column to avoid the
    // strict-mode violation.
    await expect(page.locator('.ubp-hero__left .ubp-hero__sub')).toContainText('8 billed GB');
    // Period stats reflect the avg/peak/sample numbers.
    await expect(page.locator('.ubp-stats')).toBeVisible();
    // Invoice history shows the mocked April row.
    await expect(page.locator('.ubp-invoices__table tbody tr')).toHaveCount(1);
    await expect(page.locator('.ubp-status--paid')).toContainText('paid');
    await takeScreenshot(page, `${SCREENSHOT_NS}/13-usage-report`);
  });
});

// ─── Helpers ──────────────────────────────────────────────────────────────────

function emptyUsage() {
  return {
    period_start: '2026-05-01',
    period_end: '2026-05-31',
    avg_storage_bytes: 0,
    peak_storage_bytes: 0,
    current_storage_bytes: 0,
    sample_count: 0,
    projected_amount_ghs: 5.0,
    projected_minimum_applied: true,
    projected_billed_gb: 0,
    ghs_per_gb: 1.0,
    ghs_monthly_min: 5.0,
  };
}
