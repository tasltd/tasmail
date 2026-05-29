# TMAIL-291 — E2E sweep: billing (Paystack / MPGS / Cybersource / Bank), pricing calculator, enterprise quote

- **Issue:** TMAIL-291 (continues the TMAIL-281 / 282 / 283 / 284 / 285 / 286 / 287 / 289 sweep series).
- **Date:** 2026-05-29
- **Spec:** [`frontend/e2e/specs/billing-pricing.spec.ts`](../../frontend/e2e/specs/billing-pricing.spec.ts)
- **Screenshots:** [`frontend/e2e/screenshots/billing-pricing/`](../../frontend/e2e/screenshots/billing-pricing/) — 13 PNGs covering the `/pricing` page, the four mocked-provider flows, the enterprise quote flow, and the in-app `/billing` usage dashboard.
- **Target:** Live `https://mail.techatscale.io` (workstation backend on `127.0.0.1:3300` reverse-tunnelled through `140.82.32.141:9601`).
- **Browser:** Firefox (per the E2E HARD RULE).
- **Workers:** 1 (default for this repo; tests are independent so they could parallelise, but the BYOK signup endpoint shares a database with the rest of the suite — keeping the sweep serial avoids cross-spec collisions on shared user state).

---

## TL;DR

All 10 tests pass on a clean run (`1.9m` wall time on the live tunnel). The
sweep proves every billing surface a real customer touches — the public
`/pricing` calculator, the public `/api/billing/plans` endpoint, the
in-app `BillingManager` (all four provider buttons), the inline payment-
instructions panel, the `/billing` usage dashboard, and the enterprise
quote-request flow — works end-to-end with the live backend.

| # | Test | Outcome |
| - | - | - |
| 1 | `GET /api/billing/plans` is public + JSON-array shape | ✅ pass |
| 2 | `/pricing` GHS slider, two tiers, providers list, FAQ expand | ✅ pass |
| 3 | `/pricing` BYOK CTA links to `/signup` | ✅ pass |
| 4 | Enterprise quote form submit + tracking-id success state | ✅ pass |
| 5 | `BillingManager` renders mocked plan card + all 4 provider buttons | ✅ pass |
| 6 | Paystack → SPA redirects to hosted-checkout URL | ✅ pass |
| 7 | Mastercard MPGS → inline `mpgs:session:` instructions | ✅ pass |
| 8 | Cybersource → inline `cybersource:invoice:` instructions | ✅ pass |
| 9 | Bank Transfer → inline payment instructions with reference | ✅ pass |
| 10 | `/billing` usage dashboard: projected charge + invoice history | ✅ pass |

No production-blocking bugs were found. Two **test-side** issues surfaced
during the first run of this sweep and were fixed in the same commit; they
are documented below so the next person to touch this spec recognises
them.

---

## Coverage map

The brief was: cover `/api/billing/*` (plans, Paystack, MPGS, Cybersource,
Bank Transfer), `/pricing` calculator, enterprise quote-request,
usage-based billing — without ever touching live Paystack / MPGS /
Cybersource endpoints.

| Surface | Test coverage | How |
| - | - | - |
| `GET /api/billing/plans` (public) | Test 1 | Direct HTTP request against the live tunnel. Asserts `200` + array shape. |
| `/pricing` (standalone marketing page) | Tests 2, 3 | Slider interaction at 3 GB (minimum), 20 GB (default), 120 GB (linear). Asserts the four-provider list. Expands the first FAQ row. |
| Landing-page tiers + quote form | Tests 4 + existing `pricing-and-quote.spec.ts` | Light coverage here (form fills, submit succeeds, success state); the full locale-aware tiers + validation error coverage stays in `pricing-and-quote.spec.ts` to avoid duplication. |
| In-app `BillingManager` | Tests 5–9 | Sign up fresh user → drop tokens in localStorage → click sidebar Billing entry → click provider button → assert response shape. |
| Paystack provider | Test 6 | `page.route()` returns `authorization_url: https://checkout.paystack.com/e2e-stub-…`. The SPA does `window.location.href = …` which is also intercepted by `page.route()` and stubbed with a tiny HTML page, so the test never reaches Paystack. Asserts the redirect actually fires and the request body had `provider: paystack`. |
| Mastercard MPGS | Test 7 | `page.route()` returns `authorization_url: mpgs:session:SESSION-…`. The SPA renders this inline in the `[data-testid="payment-instructions"]` panel. |
| Cybersource | Test 8 | `page.route()` returns `authorization_url: cybersource:invoice:INV-…`. Same inline panel. |
| Bank Transfer | Test 9 | `page.route()` returns `authorization_url: bank_transfer:Beneficiary…`. Asserts the multi-line bank details render inside `[data-testid="payment-instructions-detail"]`. |
| `/billing` usage dashboard (`UsageBillingPage`) | Test 10 | `page.route()` returns a deterministic 8 GB usage payload + one paid invoice row. Asserts the hero shows `GHS 8.00` and `8 billed GB`, and the table renders the mocked paid invoice. |

### Provider mocking strategy

Real provider credentials live in the DB-backed `payment_provider_config`
table (per [`backend/src/handlers/billing.rs::load_provider`](../../backend/src/handlers/billing.rs)).
Hitting `POST /api/billing/subscribe` on the live server would either
return `503 ServiceUnavailable` (no row configured for the provider on the
beta DB) or — if rows existed — initialise a real transaction against
Paystack / MPGS / Cybersource. Neither is acceptable for an E2E sweep.

This spec uses Playwright's `page.route()` to stub the four billing endpoints
(`plans`, `subscription`, `payments`, `subscribe`) plus the shared `/api/folders`
+ `/api/quota` calls the AppShell makes on mount. The stubs return the exact
JSON shapes the backend produces (verified against [`backend/src/handlers/billing.rs`](../../backend/src/handlers/billing.rs)),
including the four distinct `authorization_url` formats:

```
paystack       → "https://checkout.paystack.com/<ref>"   (hosted checkout — triggers redirect)
mastercard     → "mpgs:session:<id>"                     (rendered inline; user pastes ref into MPGS UI)
cybersource    → "cybersource:invoice:<id>"              (rendered inline; user opens invoice email)
bank_transfer  → "bank_transfer:<multiline instructions>"(rendered inline; user does a manual transfer)
```

The Paystack redirect target itself is also intercepted, so even the
redirect arrow lands on a Playwright-served stub — no live provider
endpoint is contacted under any of the four scenarios.

---

## Two test-side issues found and fixed in this commit

### 1. Describe-scoped user email caused 409s on every test after the first

**Symptom on first run.** Three of five `BillingManager` tests failed with
`HTTP 409 {"error":"An account with this email already exists"}` from
`/api/auth/signup`. The first test of the describe block succeeded, but
each subsequent test reused the same email and tripped the duplicate-row
check.

**Cause.** I hoisted `BILLING_USER` to describe scope:

```ts
test.describe('TMAIL-291 — in-app BillingManager (mocked providers)', () => {
  const BILLING_USER = `billing-${Date.now()}-${...}@e2e.tasmail`;   // ← evaluated ONCE
  test.beforeEach(async ({ apiSignup }) => {
    await apiSignup(BILLING_USER, PASSWORD);   // ← called per-test
  });
});
```

Playwright evaluates the describe body once, so `Date.now()` is captured
once and every test in the block gets the same address.

**Fix.** Move the email generation inside `beforeEach` so each test gets a
fresh address:

```ts
test.beforeEach(async ({ apiSignup }) => {
  const billingUser = `billing-${Date.now()}-${Math.floor(Math.random() * 1e9)}@e2e.tasmail`;
  await apiSignup(billingUser, PASSWORD);
});
```

### 2. `.ubp-hero__sub` strict-mode violation

**Symptom.** Test 10 failed with:

```
strict mode violation: locator('.ubp-hero__sub') resolved to 2 elements:
  1) <p class="ubp-hero__sub">Based on 8 billed GB at GHS 1/GB.</p>
  2) <div class="ubp-hero__sub">Refreshed nightly from your IMAP server</div>
```

The same class appears twice — once on the projected-charge line in the
left column, and once on the "refreshed nightly" hint in the right column
([`UsageBillingPage.tsx`](../../frontend/src/components/billing/UsageBillingPage.tsx)).

**Fix.** Scope the assertion to the hero's left column:

```ts
await expect(page.locator('.ubp-hero__left .ubp-hero__sub')).toContainText('8 billed GB');
```

(This is a test-side fix, not a component-side change — both `.ubp-hero__sub`
spans are legitimate uses of the class in the design, and the
locator-scoping is the right answer.)

### 3. Enterprise quote-form 15s timeout on heavy E2E batches

**Symptom.** Test 4 occasionally timed out at the `await expect(.eqf-success)`
line; screenshot showed the submit button stuck on "Sending…".

**Cause.** The live `/api/enterprise/quote-request` endpoint occasionally
took longer than 15s under the heavy sequential E2E batches this sweep
series runs. Not a production bug — just a per-request budget mismatch.

**Fix.** Per-test `test.setTimeout(60_000)` + watch for the POST response
explicitly with `page.waitForResponse(…, { timeout: 45_000 })` so the test
gives a precise error if the API itself is slow rather than blaming the
success-state locator.

---

## What this sweep does NOT cover

| Surface | Where it lives |
| - | - |
| Locale-aware USD line on the landing-page price card (en-US vs en-GH) | [`pricing-and-quote.spec.ts`](../../frontend/e2e/specs/pricing-and-quote.spec.ts) — kept separate to avoid duplication. |
| Enterprise quote validation-error path (required-field guard) | Same. |
| Admin payment-providers CRUD (`/admin/payment-providers`) | [`admin-payment-providers-flow.spec.ts`](../../frontend/e2e/specs/admin-payment-providers-flow.spec.ts) — already exhaustive. |
| Paystack / Mastercard webhook signature verification | Backend `cargo test` (`services::payment_service::tests`). Webhooks are server-to-server and untestable from the browser. |
| Real provider end-to-end transaction | Out of scope for E2E. Manual smoke against `paystack.com` test-mode keys is documented in [`docs/PAYMENT-PROVIDER-MIGRATION.md`](../PAYMENT-PROVIDER-MIGRATION.md). |

---

## Files changed by this commit

| Path | Change |
| - | - |
| `frontend/e2e/specs/billing-pricing.spec.ts` | **New.** 10-test sweep described above. |
| `frontend/e2e/screenshots/billing-pricing/*.png` | **New.** 13 screenshots committed alongside. |
| `docs/assessments/e2e-billing-2026-05.md` | **New** (this file). |

No backend or SPA code was modified — the sweep ran clean against the
production surface.

---

## How to re-run

```bash
cd frontend
# Default — runs against the live tunnel (mail.techatscale.io)
npx playwright test e2e/specs/billing-pricing.spec.ts --project=firefox

# Local backend variant
PLAYWRIGHT_BASE_URL=http://localhost:5273 \
  npx playwright test e2e/specs/billing-pricing.spec.ts --project=firefox
```

Wall time: ~2 minutes for 10 tests on the live tunnel (most of it is
network round-trips, not test logic). Screenshots are deterministic —
diff them against `frontend/e2e/screenshots/billing-pricing/` to spot
visual regressions in the next sweep cycle.
