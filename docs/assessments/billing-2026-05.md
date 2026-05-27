# Billing & Payment Providers Assessment — May 2026

**Ticket:** TMAIL-250 (axis of TMAIL-241 backend modularisation review)
**Scope:** `handlers/billing.rs`, `handlers/admin/payment_providers.rs`,
`handlers/usage_billing.rs`, `handlers/enterprise_quote.rs`,
`models/payment_provider_config.rs`, `models/billing.rs`,
`services/payment_service.rs`, `services/billing_rollup.rs`,
`services/billing_math.rs`, `services/encryption.rs`, migrations
`054_payment_provider_config.sql`, `058_usage_billing.sql`,
`059_enterprise_quote_requests.sql`. Frontend: `components/settings/BillingManager.tsx`,
`components/billing/UsageBillingPage.tsx`, `components/admin/PaymentProvidersManager.tsx`,
`components/landing/PricingPage.tsx`, `components/landing/EnterpriseQuoteForm.tsx`.
**Method:** Static read of every in-scope file. No live payment provider
calls were made (per the "no live financials" HARD RULE). Test files
were sampled but not re-run.

---

## TL;DR

The billing pipeline mirrors PayPro's four-provider model competently —
`PaymentProviderConfig` is a clean port with AES-256-GCM at rest, the
`EncryptionService` derives its key once at startup, the rollup loop is
metric-instrumented, and the immutable `billing_invoices` design (freezes
`ghs_per_gb` / `ghs_monthly_min` at close time) prevents rate-change
rewrites of history. Pricing math is fully unit-tested in pure Rust
(`billing_math.rs`, 9 tests).

There are **three P0 findings** worth fixing before scale:

1. **Webhook signature comparison is NOT constant-time** despite the
   comment claiming so. `expected == signature` in `verify_paystack_signature`
   short-circuits on first byte-mismatch — timing-attack viable.
   Same defect in `verify_mastercard_webhook` (`Vec<u8> == Vec<u8>`).
2. **No idempotency on webhook event handling.** Paystack and Mastercard
   both retry webhooks on non-2xx, and may also redeliver after a 2xx if
   the platform isn't sure the receiver got it. `Subscription::activate`
   is called unconditionally on every `charge.success` / `PAYMENT_SUCCESS`
   delivery — two retries = `current_period_end` extended twice.
3. **No auto-charge worker for `billing_invoices.pending`.** Migration
   058 documents one ("auto-charge worker will try the user's default
   provider"), but no such service exists. Invoices accumulate in
   `pending` forever and the usage-based pipeline doesn't actually bill.

Two structural concerns:

- **Provider registry is NOT data-driven.** Adding a fifth provider
  requires edits across ~8 files (migration CHECK constraint, payment
  service client, two `match` arms in `subscribe()`, admin whitelist,
  webhook route, frontend buttons, TypeScript union). Violates the
  Scalability rule's "data-driven configuration over hardcoded logic".
- **Webhook endpoints are public, unrate-limited, with no body cap.**
  Each call triggers a DB lookup + 7 AES-GCM decrypts via `load_provider`.
  Easy CPU-amplification DoS vector against the credentials lookup path.

Beyond those, the rollup re-aggregates the full month on every tick
(not incremental), Cybersource has no webhook handler at all (so its
payments never auto-activate subscriptions), the 30-day subscription
period is hard-coded regardless of plan interval, and the frontend
`PricingPage` hard-codes the GHS rate constants rather than reading
them from the backend.

---

## What was checked

| Axis | Result |
|---|---|
| `PaymentProviderConfig::resolve` cached or per-request decrypt | ⚠️ Per-request — see #4 |
| Webhook handlers: idempotency keys | ❌ Missing — see #2 |
| Webhook handlers: signature verification typed correctly | ⚠️ Wrong primitive — see #1 |
| Usage-billing aggregation (TMAIL-058): incremental or full-scan | ⚠️ Full-month scan per tick — see #7 |
| Provider registry — edits to add a 5th provider | ❌ ~8 files — see #5 |
| AES-256-GCM key derivation cached | ✅ Derived once at startup — see #4 |
| Rate / TLS on webhook endpoints | ❌ No rate limit, no body cap — see #6 |
| Constant-time HMAC verification | ❌ String/Vec equality — see #1 |
| `admin/payment_providers` `is_admin` gated | ✅ All three handlers call `require_admin` |
| `payment_provider_config` UPDATE / rotation path | ⚠️ Insert + archive only — see #11 |
| Immutable invoice inputs (`ghs_per_gb` frozen) | ✅ Migration 058 stores per-invoice |
| `billing_math` unit tests | ✅ 9 tests, all edge cases |
| Cybersource webhook | ❌ Does not exist — see #8 |
| Auto-charge worker for `billing_invoices.pending` | ❌ Does not exist — see #3 |
| Public quote-request rate limit (IP-keyed) | ✅ Honours X-Forwarded-For |
| Public quote-request body validation | ✅ Length + email + range checks |
| Frontend `BillingManager` modularity | ⚠️ 299 lines, hardcoded providers |
| Frontend `PricingPage` reads rate from backend | ❌ Hardcoded constants — see #14 |
| Frontend `PaymentProvidersManager` field registry | ✅ `CREDENTIAL_FIELDS` map |
| Webhook secret column semantics consistent across providers | ⚠️ Paystack uses `secret_key`; Mastercard uses `webhook_secret` |

---

## Backend findings

### 1. Webhook HMAC verification is NOT constant-time — P0

`services/payment_service.rs:121-129`:

```rust
pub fn verify_paystack_signature(secret: &str, body: &[u8], signature: &str) -> bool {
    let Ok(mut mac) = Hmac::<Sha512>::new_from_slice(secret.as_bytes()) else {
        return false;
    };
    mac.update(body);
    let expected = hex::encode(mac.finalize().into_bytes());
    // Added: Constant-time comparison to prevent timing attacks
    expected == signature
}
```

The comment claims constant-time but `String == &str` (and `Vec<u8> == &[u8]`)
short-circuit on first mismatch. Same defect on Mastercard
(`services/payment_service.rs:259-262`):

```rust
match base64::engine::general_purpose::STANDARD.decode(signature_b64) {
    Ok(sig_bytes) => sig_bytes.as_slice() == expected.as_slice(),
    Err(_) => false,
}
```

**Risk:** MEDIUM. Practical timing-attack on a webhook signature is
hard over the public internet (jitter dominates), but trivial from a
co-located attacker or a compromised LAN egress. Both `hmac::Mac`
expose a `verify_slice()` helper that IS constant-time — use it.

**Fix:** Replace both with the typed API:

```rust
// Paystack
let mut mac = Hmac::<Sha512>::new_from_slice(secret.as_bytes()).ok()?;
mac.update(body);
mac.verify_slice(&hex::decode(signature).ok()?).is_ok()
```

Or use the `subtle::ConstantTimeEq` trait. Either fix is ~3 lines.

### 2. No webhook idempotency — P0

`handlers/billing.rs:279-295` (Paystack `charge.success`):

```rust
if event.event == "charge.success" {
    if let Some(reference) = event.data.get("reference").and_then(|v| v.as_str()) {
        let new_status = "success";
        if let Some(payment) =
            Payment::update_status(&state.db, reference, new_status, event.data.clone()).await?
        {
            if let Some(sub_id) = payment.subscription_id {
                let now = chrono::Utc::now();
                // NOTE: Default to 30-day period; adjust based on plan interval
                let period_end = now + chrono::Duration::days(30);
                let _ = Subscription::activate(&state.db, sub_id, None, now, period_end).await;
            }
        }
    }
}
```

`Payment::update_status` is idempotent on the status column (always
sets `'success'`) but `Subscription::activate` (`models/billing.rs:131-148`)
sets `current_period_end = period_end` *unconditionally*. Paystack and
Mastercard both retry webhooks on non-2xx; both can also redeliver after
2xx on platform error. Two deliveries of the same event ⇒ subscription
extended by 60 days, not 30.

Same gap in `webhook_mastercard` (`handlers/billing.rs:339-349`).

**Fix sketch:**

- Add `provider_event_id TEXT UNIQUE` column to `payments` (or a new
  `webhook_deliveries` table) and `INSERT ... ON CONFLICT DO NOTHING`
  before activating.
- Guard `Subscription::activate` with `WHERE current_period_end IS NULL
  OR current_period_end < $4` so re-delivering the same event is a no-op.

### 3. No auto-charge worker for usage invoices — P0

Migration `058_usage_billing.sql:60-63` documents the contract:

```sql
-- status flow:
--   pending   – computed but not yet paid; auto-charge worker will try the user's default provider
--   paid      – payment provider returned success
--   failed    – provider rejected; keeps queueing retries until manual intervention
```

`services/billing_rollup.rs:160-184` only ever writes `status='pending'`.
Grep for `update billing_invoices set status='paid'` / `set_status` /
`charge_invoice`: **no matches**. `main.rs:71-105` spawns exactly three
background services (`EmailScheduler`, `QueueProcessor`, `BillingRollup`)
— no charge worker.

**Consequence:** The usage-based billing pipeline (TMAIL-176 → TMAIL-180)
computes invoices nightly and writes them to the DB, but the actual
charge step is missing. Every invoice sits in `pending` indefinitely.
For BYOK customers at GHS 1/GB this is the *headline* product feature
and it isn't wired through to a provider call.

**Fix sketch:** Add `services/invoice_charger.rs` modeled on the rollup
loop:

```rust
SELECT * FROM billing_invoices
WHERE status = 'pending' AND created_at < now() - interval '1 day'
LIMIT 50;
// For each: look up mailbox owner's preferred provider,
// call PaystackClient::initialize_transaction / equivalent,
// move to 'paid' on webhook success, 'failed' on provider error.
```

Plus a partial index on `(status) WHERE status='pending'` to avoid
sequential scanning the invoices table as it grows.

### 4. `PaymentProviderConfig::resolve` is per-request — P1

`handlers/billing.rs:25-38`:

```rust
async fn load_provider(state: &AppState, provider: &str)
    -> Result<DecryptedProviderConfig, AppError> {
    let row = PaymentProviderConfig::resolve(&state.db, provider, None).await?...;
    row.decrypt_with(&state.encryption)
}
```

Called from every `subscribe()` call AND every webhook delivery.
`resolve()` does one DB SELECT (uses the partial index
`idx_ppc_provider_tenant`, so it's cheap) and `decrypt_with()` runs 7
AES-256-GCM decrypts per call (`secret_key`, `public_key`,
`webhook_secret`, `merchant_id`, `api_password`, `key_id`,
`shared_secret_key`). AES-GCM in software is ~1 GB/s on modern x86 with
AES-NI, so 7 short ciphertexts is sub-millisecond — but it's
unnecessary work on every webhook.

**The encryption key itself is cached correctly.** `EncryptionService::from_jwt_secret`
runs SHA-256 of `JWT_SECRET` once at startup and stores the 32-byte
result in the struct (`services/encryption.rs:14-17`). `AppState` holds
the `EncryptionService` (`state.rs:21`) so every request reuses the same
derived key. **Key derivation is NOT done per-request** — this part is
correct and the most expensive operation is already cached.

**Risk:** LOW for AES-GCM (cheap), LOW for DB SELECT (indexed). MEDIUM
once tenant_id-scoped lookups become common — the partial index keys on
`(provider, tenant_id) WHERE enabled AND NOT archived` so each
combination is O(1), but a busy webhook endpoint making N decrypts per
delivery still wastes CPU.

**Fix:** Wrap `PaymentProviderConfig::resolve(...)` in a
`tokio::sync::RwLock<HashMap<(String, Option<Uuid>), DecryptedProviderConfig>>`
in `AppState` with a 5-minute TTL and explicit invalidation on
`admin/payment_providers::create_provider` / `archive_provider`.
Matches the existing `cache_service` pattern (branding cache, finding 4
in the admin assessment).

### 5. Provider registry is not data-driven — P1 (structural)

Counting the edits required to add a fifth provider (say, Stripe):

| File | Change |
|---|---|
| `migrations/NNN_add_stripe.sql` | `ALTER TABLE payment_provider_config DROP CONSTRAINT ... CHECK ... ADD ... 'STRIPE'` |
| `services/payment_service.rs` | New `StripeClient` struct (~80 lines) + verify_stripe_webhook |
| `handlers/billing.rs` line 82 | Add `"stripe"` to the `allowed` whitelist |
| `handlers/billing.rs` lines 112-234 | New `match` arm with credential extraction + client call (~25 lines) |
| `handlers/billing.rs` | New `webhook_stripe` handler (~50 lines) |
| `handlers/admin/payment_providers.rs:115` | Add `"STRIPE"` to the admin `allowed` whitelist |
| `router.rs` line ~103 | Register `POST /api/billing/webhook/stripe` |
| `frontend/src/components/settings/BillingManager.tsx` lines 182-219 | Add fifth provider button |
| `frontend/src/components/admin/PaymentProvidersManager.tsx:18-31` | Add `STRIPE` to `PROVIDER_OPTIONS` and `CREDENTIAL_FIELDS` map |
| `frontend/src/api/billing.ts` | Extend `BillingProvider` union |

**Roughly 8-10 edits across 7 files.** The frontend
`PaymentProvidersManager` (`CREDENTIAL_FIELDS` map at lines 26-31) is the
*only* data-driven layer. Everything else is `match` / `if` /
whitelist arrays.

**Fix sketch:** Define a single `ProviderDefinition` registry — a `HashMap<String, ProviderHandlers>`
where each entry binds a provider code to (a) the list of required
credential fields, (b) the `initialize_payment` closure, (c) the
webhook verifier closure, (d) the response-to-`authorization_url`
mapper. Then `subscribe()` and webhook routes look up the entry by
provider code. The CHECK constraint becomes a soft assertion (`provider
IN (select provider from provider_definitions)`) or a TEXT column with
the validation moving fully into the application layer. Even without
the full refactor, extracting the `subscribe()` `match` body into a
trait + `Box<dyn PaymentProvider>` map shrinks the per-provider edit
to one or two files.

### 6. Webhook endpoints lack rate-limit + body cap — P1

`router.rs:101-104` registers:

```rust
.route("/api/billing/webhook/paystack", post(handlers::billing::webhook_paystack))
.route("/api/billing/webhook/mastercard", post(handlers::billing::webhook_mastercard))
```

— inside the **public** route blob (no auth middleware). Both handlers
accept `body: axum::body::Bytes` with no `ContentLengthLimit` /
`DefaultBodyLimit` layer. There's no per-IP rate-limit middleware on
these endpoints either (the rate-limit middleware exists in
`middleware/rate_limit.rs` but isn't applied to public routes here).

The handler chain on every (forged) request is:
1. Decode request body (unbounded)
2. `load_provider` → 1 DB SELECT + 7 AES-256-GCM decrypts
3. HMAC-SHA512 / HMAC-SHA256 compute (non-constant-time, finding 1)
4. JSON parse, look up payment by reference

An attacker firing 1000 RPS of garbage bodies forces the server through
steps 1–3 for every request. Steps 2 is the most expensive and is
already a hot-path concern (finding 4).

**Risk:** MEDIUM. CPU-amplification + an oracle for the timing-attack
in finding 1.

**Fix:** Apply `DefaultBodyLimit::max(64 * 1024)` (webhook payloads are
typically <10 KB) to the two webhook routes, and a per-IP token-bucket
limiter (10 req/s burst 50). Once finding 4's cache lands the decrypt
cost vanishes too.

### 7. Rollup aggregation is full-month scan, not incremental — P1

`services/billing_rollup.rs:115-149`:

```rust
sqlx::query(
    "WITH latest AS (
        SELECT DISTINCT ON (mailbox_id) mailbox_id, used_bytes
        FROM quota_usage
     )
     INSERT INTO billing_periods (...)
     SELECT mailbox_id, $1, $2, used_bytes, used_bytes, 1, 'open'
     FROM latest
     ON CONFLICT (mailbox_id, period_start) DO UPDATE
     SET avg_storage_bytes = (
            SELECT COALESCE(AVG(used_bytes)::bigint, 0)
            FROM usage_samples us
            WHERE us.mailbox_id = billing_periods.mailbox_id
              AND us.sampled_at >= billing_periods.period_start
              AND us.sampled_at <  billing_periods.period_end + interval '1 day'
         ),
         peak_storage_bytes = (
            SELECT COALESCE(MAX(used_bytes), 0)
            FROM usage_samples us
            WHERE ...same scan...
         ),
         sample_count = (SELECT COUNT(*) FROM usage_samples us WHERE ...same scan...)",
)
```

Each rollup tick re-scans `usage_samples` for the entire current month,
three times per mailbox (AVG, MAX, COUNT — three correlated subqueries
inside the same INSERT…ON CONFLICT). For N mailboxes with ~30 samples
in-month, this is O(N × 30 × 3) PostgreSQL row reads per tick. With
the default `poll_interval_secs` (env-driven, typically 86400 = once
per day) this is fine — at 1000 mailboxes / 30 samples = 90 000 row
reads per tick, sub-second.

If the operator drops `poll_interval_secs` (TMAIL-176 mentions hourly
runs for tighter quota enforcement), the constant factor multiplies.

**Risk:** LOW today, MEDIUM at 10× mailbox count *and* sub-hourly
ticks.

**Fix sketch:** Maintain a running sum/count/peak in `billing_periods`
itself, updated incrementally per inserted sample. Sample insertion in
step 1 already returns the affected mailboxes — extend that CTE to
also `UPDATE billing_periods SET running_sum = running_sum + new.used_bytes,
sample_count = sample_count + 1, peak_storage_bytes = GREATEST(...)`.
Avg is then `running_sum / sample_count` on read.

### 8. Cybersource has no webhook handler — P1

`handlers/billing.rs` defines webhooks only for Paystack and Mastercard.
The Cybersource branch of `subscribe()` (`handlers/billing.rs:173-213`)
creates an invoice via `CybersourceClient::initialize_payment` and
returns `"cybersource:invoice:{invoice_id}"` to the SPA — but there is
no `/api/billing/webhook/cybersource` route and no polling task that
queries `CybersourceClient::verify_payment` (which IS implemented at
`services/payment_service.rs:422-456` but never called outside tests).

**Consequence:** Cybersource invoice payments NEVER auto-activate the
linked subscription. The `payments` row stays at `'pending'` forever
unless an admin manually reconciles.

**Risk:** HIGH for any tenant using Cybersource — billing succeeds, but
service access is never granted via the standard flow.

**Fix:** Either (a) add a `services/cybersource_poller.rs` background
task that periodically calls `verify_payment` for invoices >24h old
and not yet paid, OR (b) implement Cybersource's webhook (their
"Notifications" API) and register it at
`/api/billing/webhook/cybersource`. Option (a) is simpler and more
robust given Cybersource's invoicing flow is typically days-to-weeks.

### 9. 30-day subscription period ignores plan interval — P2

`handlers/billing.rs:289-291`:

```rust
let now = chrono::Utc::now();
// NOTE: Default to 30-day period; adjust based on plan interval
let period_end = now + chrono::Duration::days(30);
```

`models/billing.rs:18` shows `BillingPlan.interval: String` exists in
the schema and is presumably `monthly | annual | weekly`, but it's
ignored — every successful charge always sets a 30-day period. Annual
plans get charged the full annual price and given 30 days of access.

The NOTE in the code already flags this, but it's been in place since
TMAIL-46.

**Fix:** Pull `plan.interval` and map to a `chrono::Duration`. Three
lines. Worth bundling with the auto-charge worker (finding 3).

### 10. Inconsistent webhook secret column semantics — P2

`handlers/billing.rs:264`:

```rust
let pcfg = load_provider(&state, "PAYSTACK").await?;
let paystack_key = pcfg.secret_key.ok_or_else(|| { ... })?;
if !verify_paystack_signature(&paystack_key, &body, signature) { ... }
```

But `handlers/billing.rs:314-317` (Mastercard):

```rust
let pcfg = load_provider(&state, "MASTERCARD").await?;
let secret = pcfg.webhook_secret.ok_or_else(|| { ... })?;
if !verify_mastercard_webhook(&secret, &body, signature) { ... }
```

Paystack uses **`secret_key`** for HMAC verification (which is correct
per Paystack docs — they sign with the merchant secret key). Mastercard
uses **`webhook_secret`** — a separate column. The schema has both, but
which provider uses which is convention rather than schema.

**Risk:** LOW — works correctly today. MEDIUM the day an operator
rotates the Paystack `secret_key` but leaves the same `webhook_secret`
in the DB and assumes the webhook will use the new value (it does, but
the inverse mistake — putting the Paystack signing secret in
`webhook_secret` — would silently fail signature verification with no
clear error).

**Fix:** Add a comment block to `migrations/054_payment_provider_config.sql`
documenting which column each provider uses for HMAC verification, OR
collapse the two into a single `signing_secret` column per provider
in a follow-up migration.

### 11. No UPDATE / rotation path for `payment_provider_config` — P2

`models/payment_provider_config.rs` exposes `resolve`, `list_all`,
`decrypt_with`, `insert`, but no `update`. `handlers/admin/payment_providers.rs`
exposes `list_providers`, `create_provider`, `archive_provider` — no
PUT/PATCH. To rotate a Paystack secret, an admin must:

1. POST a new row (archives nothing automatically).
2. DELETE the old row (soft-archive).

Between steps 1 and 2 the partial index `idx_ppc_provider_tenant`
`WHERE enabled AND NOT archived` allows two enabled rows for the same
`(provider, tenant_id)` pair, and `resolve()`'s `ORDER BY updated_at DESC
LIMIT 1` picks the newer one — so functionally fine — but the API
surface implies "delete the old, then create the new" which is the
opposite order from what an operator would naturally do.

**Risk:** LOW. The frontend (`PaymentProvidersManager.tsx`) only
exposes create + delete, so there's no false "edit" UI to mislead an
admin. Worth documenting in `docs/PAYMENT-PROVIDER-MIGRATION.md` (which
already exists per CLAUDE.md) that rotation is via "create-then-delete"
not "edit".

### 12. Webhook handler swallows activation errors silently — P2

`handlers/billing.rs:291`:

```rust
let _ = Subscription::activate(&state.db, sub_id, None, now, period_end).await;
```

The `let _ =` suppresses any error — DB conn drop, RLS misconfig, FK
violation — and still returns `200 OK` to the provider. Paystack
considers that a successful delivery and won't retry. If the activation
silently failed, the user paid but their subscription never went
active.

**Risk:** LOW (failure mode is rare) but the silent-error pattern
violates the Agent-friendly-code rule and makes debugging post-hoc
support tickets harder.

**Fix:** Log the error explicitly, optionally enqueue a retry into the
existing `email_queue` / a new `subscription_activation_retries` table.

### 13. Plan/Payment `provider` column not enum-constrained at DB level — P2

`models/billing.rs:30,46` define `Subscription.provider: String` and
`Payment.provider: String`. Migration 049 (not in scope but adjacent)
presumably has a CHECK or enum constraint. Worth confirming the same
four-provider whitelist holds across `subscriptions.provider` and
`payments.provider` as in `payment_provider_config.provider` (migration
054), and that `MoMo`-era rows (which the code comments confirm
previously existed) have been migrated or deleted.

### 14. Quote-request rate-limit key includes only IP — P2

`handlers/enterprise_quote.rs:80`:

```rust
if !state.cache.check_rate_limit(&format!("eqr:{}", client_ip)).await { ... }
```

Sane default, but an attacker behind a single egress IP (corporate NAT,
office wifi) starves legitimate co-workers. The cache key is also
unbounded — if `X-Forwarded-For` is spoofed by a misconfigured proxy
the cache grows by attacker-controlled keys. The check honours
X-Forwarded-For only when behind Apache (the production setup), which
is the right call.

**Fix:** None needed for current scale, but in a future iteration key
by `(IP, email-domain)` so an attacker has to also vary email domains
to keep getting through, and TTL-evict the cache entries after the
window expires.

### 15. Cybersource HTTP-Signature build silently returns `String::new()` on key decode failure — P2

`services/payment_service.rs:367-372`:

```rust
let secret_bytes = base64::engine::general_purpose::STANDARD
    .decode(&self.shared_secret_key).unwrap_or_default();
let mut mac = match Hmac::<sha2::Sha256>::new_from_slice(&secret_bytes) {
    Ok(m) => m,
    Err(_) => return String::new(),
};
```

If the operator pastes an invalid base64 `shared_secret_key` in
`payment_provider_config`, `build_signature` returns the empty string
— the resulting HTTP request to Cybersource gets the header
`signature: keyid="...", algorithm="HmacSHA256", headers="...", signature=""`
and Cybersource rejects it. The error surfaces as a generic "Cybersource
provider unavailable" 500 with no actionable detail.

**Fix:** Return `Result<String, anyhow::Error>` from `build_signature`
and bubble the decode failure all the way to a 503 with a clear "invalid
shared_secret_key — must be base64" message. Two-line refactor.

---

## Frontend findings

### 1. `BillingManager.tsx` — 4 hardcoded provider buttons — P2

`components/settings/BillingManager.tsx:182-219` repeats the same JSX
shape (button + icon + onClick + testid) four times — once per
provider. Adding/removing a provider edits this file plus the
backend whitelist (finding 5). Should be driven by a registry mirroring
`PaymentProvidersManager.tsx`'s `PROVIDER_OPTIONS`:

```ts
const SUBSCRIBE_PROVIDERS = [
  { id: 'paystack', label: 'Pay with Card', icon: CreditCard, variant: 'primary' },
  { id: 'mastercard', label: 'Mastercard', icon: CreditCard, variant: 'secondary' },
  { id: 'cybersource', label: 'Invoice', icon: FileText, variant: 'secondary' },
  { id: 'bank_transfer', label: 'Bank Transfer', icon: Landmark, variant: 'ghost' },
];
// ...then .map() the buttons.
```

`PaymentProvidersManager.tsx` already proves the pattern (lines 18-31)
— extend the same idea to `BillingManager`.

### 2. `PricingPage.tsx` hard-codes rate constants — P1

`components/landing/PricingPage.tsx:12-14`:

```ts
const GHS_PER_GB = 1.00;
const MONTHLY_MIN = 5.00;
const GHS_TO_USD = 0.067;
```

These mirror the backend `TASMAIL_GHS_PER_GB` / `TASMAIL_GHS_MONTHLY_MIN`
env vars (`services/billing_rollup.rs:43-51` and
`handlers/usage_billing.rs:38-40`). If an operator bumps the rate, the
calculator drifts from the actual bill. Same drift risk in
`UsageBillingPage.tsx` (`GHS_TO_USD = 0.067`).

**Fix:** Expose a public `GET /api/billing/rates` endpoint that returns
`{ ghs_per_gb, ghs_monthly_min, ghs_to_usd_indicative }` — PricingPage
fetches once, UsageBillingPage reads the same. Backend reads from the
same env vars it uses for the math.

### 3. `UsageBillingPage` is not gated by a feature flag for non-BYOK plans — P2

`components/billing/UsageBillingPage.tsx` renders the usage-based
billing dashboard unconditionally. Customers on the legacy
flat-priced billing plans (`BillingPlan.price_cedis` from migration
049, not migration 058's per-GB model) will see a "projected GHS X"
that doesn't match their bill. Worth either deprecating the legacy
plans or gating the dashboard behind a `subscription.is_usage_based`
flag.

### 4. `EnterpriseQuoteForm` — clean — ✅

Validates name/email/message client-side, posts via
`quoteRequestsApi.submit`, surfaces backend error JSON correctly,
returns a tracking id on success. 150 lines, single responsibility,
no findings.

### 5. `PaymentProvidersManager` — registry-driven, no edit path — ✅ with caveat

`components/admin/PaymentProvidersManager.tsx:26-31` correctly drives
the form fields off the `CREDENTIAL_FIELDS` map per provider type —
this is the data-driven pattern the rest of the surface should adopt.
Caveat: there's no edit UI; rotation is via create-then-delete (per
backend finding 11). Acceptable for now but should be documented inline
near the trash-icon button.

### 6. Component sizes — within budget

| File | Lines | Status |
|---|---|---|
| `EnterpriseQuoteForm.tsx` | 150 | ✅ |
| `UsageBillingPage.tsx` | 157 | ✅ (partial read — file may be longer) |
| `PricingPage.tsx` | 185 | ✅ |
| `BillingManager.tsx` | 299 | ⚠️ over the 250-line guideline — split provider buttons + payment-history table into siblings |

---

## Migration / schema findings

| Migration | Finding |
|---|---|
| `054_payment_provider_config.sql` | ✅ Correct CHECK constraint on provider; ✅ Tenant-id partial index; ✅ Auto-update trigger on `updated_at`. ⚠️ No `payment_provider_config_history` table — credential changes are not auditable. Worth adding before any compliance push (GDPR, PCI-DSS Level 4). |
| `058_usage_billing.sql` | ✅ RLS enabled, ✅ immutable invoice fields (`ghs_per_gb`, `ghs_monthly_min` frozen at close). ⚠️ `idx_billing_invoices_status` is on `(status, period_end)` — works, but a partial index `WHERE status='pending'` would scan faster when the auto-charge worker (finding 3) lands. |
| `059_enterprise_quote_requests.sql` | ✅ Three good indexes (status, created_at, lower(email)); ✅ `source_ip` captured for abuse forensics; ✅ Auto-update trigger. ⚠️ `internal_notes` is a single TEXT column — multi-rep collaboration would benefit from a separate `quote_request_comments` table (defer until needed). |

---

## Test coverage

| Area | Tests | Status |
|---|---|---|
| `billing_math::compute_invoice_ghs` | 9 unit tests covering empty mailbox, 1 GB exact, just-over-1 GB rounding, 9.5 GB ceiling, 50 GB, negative bytes, alternative rate, min-clamping, cents rounding | ✅ Solid |
| `payment_service.rs::verify_paystack_signature` | 4 tests (valid, tampered, wrong secret, empty body) | ⚠️ Missing: timing-attack regression test (would fail before fix #1) |
| `payment_service.rs::verify_mastercard_webhook` | 1 test (roundtrip + tampered + wrong secret in one assertion block) | ✅ |
| `payment_service.rs::PaystackClient::initialize_transaction` | None — only request-struct serialization test | ⚠️ Untested HTTP path |
| `payment_service.rs::MastercardClient`, `CybersourceClient` | New + signature-format unit tests, no end-to-end integration | ⚠️ |
| `payment_service.rs::BankInstructionConfig` | 1 test | ✅ |
| `payment_provider_config::resolve` + `decrypt_with` | None visible | ❌ Worth adding — tenant-priority lookup is critical |
| `encryption::EncryptionService` | 2 tests (roundtrip + wrong-secret fail) | ✅ |
| `billing_rollup::tick` | None — requires Postgres | ⚠️ Pure-SQL logic worth covering with sqlx-test |
| Frontend `BillingManager` | `BillingManager.test.tsx` exists | Sampled but not re-run |

**No tests trigger live provider HTTP** — confirmed by reading every
`#[cfg(test)]` block. Assessment compliant with the no-live-financials
HARD RULE.

---

## Recommendations

### P0 — Security / correctness

1. **Replace string/byte equality with `Mac::verify_slice`** in
   `verify_paystack_signature` (`payment_service.rs:128`) and
   `verify_mastercard_webhook` (`payment_service.rs:260`). 3-line fix
   per function. Add a regression test that compares partial-prefix
   signatures byte-by-byte to confirm constant-time behaviour.

2. **Add idempotency to webhook handlers.** Either:
   - Add `provider_event_id TEXT UNIQUE` to `payments` (or a new
     `webhook_deliveries` table) and `INSERT ... ON CONFLICT DO NOTHING`
     before activating, OR
   - Guard `Subscription::activate` with `WHERE current_period_end IS
     NULL OR current_period_end < $4` so repeated delivery is a no-op.
   Apply to both Paystack and Mastercard paths
   (`handlers/billing.rs:285` and `:339`).

3. **Implement the auto-charge worker** (`services/invoice_charger.rs`).
   Sibling to `BillingRollup` in `main.rs`. Reads
   `billing_invoices WHERE status='pending'`, calls the user's preferred
   provider, transitions status on webhook ack. Without this, the
   entire TMAIL-176 usage-based billing pipeline is dead-letter.

### P1 — Scalability / robustness

4. **Cache `PaymentProviderConfig::resolve` results** in `AppState`
   with explicit invalidation on
   `admin/payment_providers::create_provider` and `archive_provider`.
   Mirror the branding cache (5-minute TTL or until-mutation). Cuts
   per-webhook decrypt overhead from 7×AES-GCM to 0.

5. **Make the provider registry data-driven.** Extract
   `payment_service.rs`'s four client types behind a `trait
   PaymentProvider { initialize, verify, parse_webhook }` and register
   them in a `HashMap<&str, Box<dyn PaymentProvider>>` in `AppState`.
   `handlers/billing.rs:subscribe()` becomes a one-line lookup. Adding
   a fifth provider = one new file + one map entry.

6. **Rate-limit + body-cap public webhook routes**
   (`router.rs:101-104`). `DefaultBodyLimit::max(64 * 1024)` per webhook
   route, plus an IP-keyed token-bucket via the existing
   `cache_service::check_rate_limit` (10 req/s, burst 50).

7. **Switch billing rollup to incremental aggregation.** Maintain
   running sum/count/peak columns updated on each `usage_samples`
   insert, replacing the three correlated subqueries in
   `billing_rollup.rs:124-144`. Avg becomes `running_sum / sample_count`
   on read.

8. **Add a Cybersource verification poller**
   (`services/cybersource_poller.rs`) or the equivalent webhook
   endpoint. Without it, Cybersource invoice payments never activate
   the linked subscription.

### P2 — Hygiene / documentation

9. **Map `BillingPlan.interval` to subscription period length** —
   replace the hard-coded 30-day extension in `handlers/billing.rs:289`
   and `:344`. Three lines per webhook.

10. **Document the `secret_key` vs `webhook_secret` split** in
    `migrations/054_payment_provider_config.sql` so the next operator
    rotating a credential doesn't put it in the wrong column.

11. **Drop a `payment_provider_config_history` table** (or use the
    existing audit-log writer) so credential rotations are traceable
    for compliance.

12. **Document the create-then-delete rotation flow** in
    `docs/PAYMENT-PROVIDER-MIGRATION.md` and add an inline tooltip to
    `PaymentProvidersManager.tsx`'s trash icon.

13. **Stop swallowing `Subscription::activate` errors silently**
    (`handlers/billing.rs:291,346`). At minimum log + alert; ideally
    enqueue a retry.

14. **Surface `ghs_per_gb` / `ghs_monthly_min` / `ghs_to_usd_indicative`
    via `GET /api/billing/rates`** so `PricingPage.tsx` and
    `UsageBillingPage.tsx` stop drifting from the backend env vars.

15. **Bubble Cybersource shared-secret base64-decode errors**
    (`payment_service.rs:368`) instead of returning an empty signature.

16. **Split `BillingManager.tsx` (299 lines) into**
    `BillingPlanCards.tsx` + `PaymentHistoryTable.tsx` +
    `BillingManager.tsx` (orchestrator). Drive the provider buttons off
    a `SUBSCRIBE_PROVIDERS` registry.

17. **Gate `UsageBillingPage` behind `subscription.is_usage_based`** or
    deprecate the legacy flat-priced plans entirely.

18. **Add a partial index `WHERE status='pending'` on
    `billing_invoices`** before the auto-charge worker (recommendation 3)
    ships. Avoids a sequential scan once the table has many closed
    months.

19. **Backfill model tests** for `PaymentProviderConfig::resolve`
    (tenant-priority logic) and `decrypt_with` (round-trip).

---

## Action item summary

| # | Priority | Area | One-line fix |
|---|---|---|---|
| 1 | P0 | backend/security | Replace `==` with `Mac::verify_slice` in both webhook verifiers |
| 2 | P0 | backend/correctness | Idempotency guard before `Subscription::activate` |
| 3 | P0 | backend/correctness | Implement `services/invoice_charger.rs` background task |
| 4 | P1 | backend/perf | Cache `PaymentProviderConfig::resolve` in AppState with invalidation |
| 5 | P1 | backend/structure | Extract `trait PaymentProvider` + registry; remove `match` arms |
| 6 | P1 | backend/security | DefaultBodyLimit(64KB) + IP rate-limit on public webhook routes |
| 7 | P1 | backend/perf | Incremental rollup aggregation |
| 8 | P1 | backend/correctness | Cybersource webhook handler or verify-poller |
| 9 | P2 | backend/correctness | Map `plan.interval` to subscription period length |
| 10 | P2 | docs | Document `secret_key` vs `webhook_secret` per provider |
| 11 | P2 | backend/compliance | `payment_provider_config_history` audit table |
| 12 | P2 | docs | Document create-then-delete rotation in payment-provider migration doc |
| 13 | P2 | backend/observability | Log `Subscription::activate` errors instead of `let _ =` |
| 14 | P2 | frontend/consistency | `GET /api/billing/rates` for PricingPage + UsageBillingPage |
| 15 | P2 | backend/observability | Bubble Cybersource base64-decode failures |
| 16 | P2 | frontend/modularity | Split `BillingManager.tsx` + registry-driven provider buttons |
| 17 | P2 | frontend/UX | Gate `UsageBillingPage` to usage-based subscriptions |
| 18 | P2 | backend/perf | Partial index on `billing_invoices WHERE status='pending'` |
| 19 | P2 | tests | Add `PaymentProviderConfig::resolve` + `decrypt_with` unit tests |

These should be filed as scoped TMAIL tasks against this assessment.

---

## Method notes (no-live-financials compliance)

This assessment was conducted entirely via static reads. No file under
`backend/src/services/payment_service.rs` was executed against a live
provider endpoint. No `POST /api/billing/subscribe`,
`POST /api/billing/webhook/*`, or admin payment-provider mutation was
issued during this review. All quoted code paths were read from the
checked-in tree on `main` at commit `abca9f1`. The single shell
operation that touched billing data (`rg` / `wc -l`) was read-only.
