# Payment Provider Credential Migration (PayPro → TASMail)

**Work item:** [TMAIL-163](https://cim.techatscale.io/projects/TMAIL/issues) —
*Insert PayPro production credentials into `payment_provider_config`.*

**Status:** Blocked until PayPro production DB credentials (or an exported credential
dump) are handed off. This document is the runbook for the operator who unblocks it.

---

## 1. What this migration is for

TASMail mirrors PayPro's billing topology — same four providers, same DB-backed
encrypted credential store. To take TASMail live on real-money payments we have
to copy the four production rows from PayPro's `payment_provider_config` table
into TASMail's `payment_provider_config` table (schema in
[migration `054_payment_provider_config.sql`](../backend/migrations/054_payment_provider_config.sql)).

The four rows that must exist (one per provider) are:

| Provider | Purpose | Required credential fields |
| --- | --- | --- |
| `PAYSTACK` | Mobile money + card (online) | `secret_key`, `public_key` (and `webhook_secret` if signing webhooks) |
| `MASTERCARD` | Mastercard Payment Gateway Services (MPGS) | `merchant_id`, `api_password` |
| `CYBERSOURCE` | Invoice-based payments | `merchant_id`, `key_id`, `shared_secret_key` |
| `BANK_TRANSFER` | Manual bank transfer instructions | `bank_details` JSON (no encrypted fields) |

Source of truth for the required-fields rule:
[`PaymentProviderConfig.hasRequiredCredentials()`](https://github.com/tasltd/paypro-oms/blob/main/grails-app/domain/cloud/paypro/oms/billing/PaymentProviderConfig.groovy)
in PayPro.

Until those four rows are inserted, `handlers/billing.rs` returns **HTTP 503** with
an actionable message on any billing call (see
[`backend/src/handlers/billing.rs`](../backend/src/handlers/billing.rs) — `load_provider(...)`).

---

## 2. Why this is blocked

PayPro production runs in a different environment owned by a different team. The
TASMail repo and the engineer running this migration **do not have**:

- Read access to PayPro's MariaDB `payment_provider_config` table, or
- The PayPro `EncryptionService` key needed to decrypt PayPro's ciphertext.

That means the migration has to be performed by **someone with PayPro production
access**. The operator picks one of the two paths in §3 to unblock.

> **Important:** PayPro encrypts with its own AES-256-GCM key (Grails
> `cloud.paypro.oms.security.EncryptionService`). TASMail encrypts with a key
> derived from its own `JWT_SECRET` (see
> [`backend/src/services/encryption.rs`](../backend/src/services/encryption.rs)).
> The two keys are **not** interchangeable. You cannot copy ciphertext across —
> you must decrypt with PayPro's key, then POST the **plaintext** to TASMail's
> admin endpoint, which re-encrypts under TASMail's key.

---

## 3. Two unblock paths

### Path A — Operator runs the export themselves (recommended)

This is the safest path. PayPro credentials never leave the PayPro-admin's
machine in plaintext on disk.

**Prerequisites on operator's machine:**

- Network access to TASMail backend at `https://mail.techatscale.io` (or
  `http://127.0.0.1:3300` if running on `tas-src-1`).
- A TASMail admin JWT bearer token (issued via `/api/auth/login` with an
  account whose `role = 'admin'`, gated by `auth_service::require_admin`).
- PayPro admin login — they need to read each provider's plaintext via PayPro's
  Admin Payment Provider UI (`/adminPaymentProvider/show/{id}`) which decrypts
  with PayPro's own key and renders the masked + plaintext view.

**Steps:**

1. In PayPro Admin UI, open each of the four provider rows. Note the plaintext
   values for the credential fields listed in §1.
2. For each provider, POST to TASMail's admin endpoint. The handler is
   [`handlers/admin/payment_providers.rs::create_provider`](../backend/src/handlers/admin/payment_providers.rs).
   See §5 for ready-to-edit curl templates.
3. Verify each insertion via `GET /api/admin/payment-providers` (sensitive fields
   come back as `has_secret_key: true` flags — never decrypted in this response).
4. Smoke-test billing by hitting `GET /api/billing/plans` from the SPA. A 200
   with the plan list means `PaymentProviderConfig::resolve(...)` is happy.
5. Hand the work item back to engineering to flip TMAIL-163 → Done.

### Path B — DB credential handover + scripted migration

Use only when Path A is impractical (e.g. operator can't run curl).

1. PayPro admin opens a **time-limited** read-only PostgreSQL/MariaDB user
   against the PayPro `payment_provider_config` table.
2. PayPro admin shares **the PayPro `EncryptionService` key** with the TASMail
   engineer through 1Password / Bitwarden / Vault (never email or chat).
3. TASMail engineer runs a one-off Python script that:
   a. Connects to PayPro DB and SELECTs the four rows.
   b. Decrypts each ciphertext column using PayPro's key (AES-256-GCM, same
      algorithm — port the decrypt logic from
      `cloud.paypro.oms.security.EncryptionService`).
   c. POSTs each row to TASMail's `/api/admin/payment-providers` using the
      plaintext (TASMail re-encrypts under its own key).
   d. Verifies via `GET /api/admin/payment-providers`.
4. PayPro admin **revokes** the read-only DB user the moment migration is done.
5. The script is deleted (do **not** commit it — it contains hardcoded admin
   tokens and DB DSNs during the run).

If Path B is taken, append a short audit note to this document (date, operator,
which rows were migrated, environment) — see §7.

---

## 4. Field mapping (PayPro → TASMail)

PayPro is Grails/Groovy (camelCase domain fields). TASMail is Rust + serde
(snake_case JSON). The columns line up 1:1.

| PayPro domain field | TASMail JSON field | Encrypted? | Notes |
| --- | --- | --- | --- |
| `provider` | `provider` | no | `PAYSTACK` / `MASTERCARD` / `CYBERSOURCE` / `BANK_TRANSFER` |
| `tenantId` (String UUID) | `tenant_id` (UUID, optional) | no | `null` ⇒ global config; non-null ⇒ tenant-scoped override |
| `name` | `name` | no | Display name e.g. `"Production Paystack"` |
| `description` | `description` | no | |
| `decryptedSecretKey` | `secret_key` | **yes** | Paystack secret, generic API secret |
| `decryptedPublicKey` | `public_key` | **yes** | Paystack public key |
| `decryptedWebhookSecret` | `webhook_secret` | **yes** | HMAC secret for webhook verification |
| `decryptedMerchantId` | `merchant_id` | **yes** | Mastercard / Cybersource merchant ID |
| `decryptedApiPassword` | `api_password` | **yes** | Mastercard MPGS API password |
| `decryptedKeyId` | `key_id` | **yes** | Cybersource HTTP-Signature key id |
| `decryptedSharedSecretKey` | `shared_secret_key` | **yes** | Cybersource HMAC shared secret (base64) |
| `decryptedKeyFilePath` | `key_file_path` | **yes** | Cybersource P12 key file path |
| `baseUrl` | `base_url` | no | Provider API base URL |
| `callbackUrl` | `callback_url` | no | Payment callback URL |
| `currency` | `currency` | no | Default `GHS` |
| `environment` | `environment` | no | `sandbox` or `production` |
| `bankDetails` (String, JSON-encoded) | `bank_details` (JSON object) | no | **Parse PayPro's string to a JSON object before POSTing.** TASMail stores this as `JSONB`. |
| `splitCode` | `split_code` | no | Paystack split code, e.g. `SPL_xxx` |
| `notes` | `notes` | no | |

**Always pull from `decrypted*` getters, never raw `secretKey` etc.** — the raw
fields hold PayPro ciphertext that is meaningless to TASMail.

---

## 5. Curl templates (Path A)

Set these up front:

```bash
export TASMAIL_BASE="https://mail.techatscale.io"   # or http://127.0.0.1:3300 on tas-src-1
export TASMAIL_TOKEN="<admin JWT from POST /api/auth/login>"
```

> **Faster alternative — use the operator helper script.** Instead of hand-crafting
> four curl payloads, copy
> [`deploy/scripts/payment-providers.example.json`](../deploy/scripts/payment-providers.example.json)
> to `deploy/scripts/payment-providers.local.json` (gitignored), fill in plaintext
> values, then run:
>
> ```bash
> # Dry-run first — validates required fields and prints redacted payloads
> ./deploy/scripts/migrate-payment-providers.sh \
>   --file deploy/scripts/payment-providers.local.json --dry-run
>
> # Real run — POSTs all four, prints audit-log rows, runs verification
> ./deploy/scripts/migrate-payment-providers.sh \
>   --file deploy/scripts/payment-providers.local.json \
>   --base-url "$TASMAIL_BASE" --token "$TASMAIL_TOKEN"
>
> # Verify only (no file needed) — useful after a manual run
> ./deploy/scripts/migrate-payment-providers.sh \
>   --base-url "$TASMAIL_BASE" --token "$TASMAIL_TOKEN" --verify-only
> ```
>
> The script enforces the same required-field rules from §1, redacts sensitive
> values in dry-run output, and prints a ready-to-paste audit-log row per
> provider. Delete the filled-in JSON file the moment migration is done.

The raw curl templates below remain available for operators who prefer to issue
each request manually.

### Paystack

```bash
curl -fsS -X POST "$TASMAIL_BASE/api/admin/payment-providers" \
  -H "Authorization: Bearer $TASMAIL_TOKEN" \
  -H "Content-Type: application/json" \
  -d '{
    "provider": "PAYSTACK",
    "tenant_id": null,
    "name": "Production Paystack",
    "description": "Migrated from PayPro production payment_provider_config",
    "secret_key": "<paystack secret key — starts with sk_live_>",
    "public_key": "<paystack public key — starts with pk_live_>",
    "webhook_secret": "<paystack webhook signing secret>",
    "base_url": "https://api.paystack.co",
    "callback_url": "https://mail.techatscale.io/api/billing/webhook/paystack",
    "currency": "GHS",
    "environment": "production",
    "split_code": "SPL_xxxxxxxxxx"
  }'
```

### Mastercard MPGS

```bash
curl -fsS -X POST "$TASMAIL_BASE/api/admin/payment-providers" \
  -H "Authorization: Bearer $TASMAIL_TOKEN" \
  -H "Content-Type: application/json" \
  -d '{
    "provider": "MASTERCARD",
    "tenant_id": null,
    "name": "Production Mastercard MPGS",
    "merchant_id": "TEST<merchantId>",
    "api_password": "<api password>",
    "base_url": "https://eu-gateway.mastercard.com/api/rest/version/72",
    "callback_url": "https://mail.techatscale.io/api/billing/webhook/mastercard",
    "currency": "GHS",
    "environment": "production"
  }'
```

### Cybersource

```bash
curl -fsS -X POST "$TASMAIL_BASE/api/admin/payment-providers" \
  -H "Authorization: Bearer $TASMAIL_TOKEN" \
  -H "Content-Type: application/json" \
  -d '{
    "provider": "CYBERSOURCE",
    "tenant_id": null,
    "name": "Production Cybersource",
    "merchant_id": "<merchant id>",
    "key_id": "<http-signature key id>",
    "shared_secret_key": "<base64 hmac shared secret>",
    "key_file_path": "/etc/tasmail/cybersource.p12",
    "base_url": "https://apitest.cybersource.com",
    "currency": "GHS",
    "environment": "production"
  }'
```

> If `key_file_path` is set, you must also copy the P12 file to the TASMail
> backend host at the same path and ensure the systemd unit user can read it.

### Bank Transfer

```bash
curl -fsS -X POST "$TASMAIL_BASE/api/admin/payment-providers" \
  -H "Authorization: Bearer $TASMAIL_TOKEN" \
  -H "Content-Type: application/json" \
  -d '{
    "provider": "BANK_TRANSFER",
    "tenant_id": null,
    "name": "Manual Bank Transfer",
    "currency": "GHS",
    "environment": "production",
    "bank_details": {
      "account_name": "Tech at Scale Ltd",
      "account_number": "1234567890",
      "bank": "GCB Bank",
      "branch": "Accra Main",
      "swift": "GHCBGHACXXX",
      "reference_prefix": "TASMAIL-"
    }
  }'
```

Each successful POST returns HTTP 201 and a `ProviderSummary` JSON body where
`has_secret_key`, `has_merchant_id`, etc. confirm what was stored. The
ciphertext is **never** returned to the client.

---

## 6. Verification

After all four POSTs:

```bash
# 1. Confirm four enabled, non-archived rows exist
curl -fsS "$TASMAIL_BASE/api/admin/payment-providers" \
  -H "Authorization: Bearer $TASMAIL_TOKEN" | jq '
    [.[] | select(.archived == false and .enabled == true) | .provider] | sort
  '
# Expected: ["BANK_TRANSFER","CYBERSOURCE","MASTERCARD","PAYSTACK"]

# 2. Confirm billing endpoint no longer returns 503
curl -fsS "$TASMAIL_BASE/api/billing/plans" | jq '.plans | length'
# Expected: integer > 0 (was 503 "provider config missing" beforehand)
```

If either check fails, look at backend logs:

```bash
journalctl --user -u tasmail-backend.service -f
```

Common failure modes:

| Symptom | Cause | Fix |
| --- | --- | --- |
| `provider must be one of: PAYSTACK, MASTERCARD, CYBERSOURCE, BANK_TRANSFER` | Typo in `provider` field | Resend with the exact whitelisted string |
| 503 from `/api/billing/plans` after insert | Row was inserted with `enabled = false` (default is true — only happens if you also archived it) | `DELETE /api/admin/payment-providers/{id}` then re-POST |
| `Failed to create provider config: encryption error` | `JWT_SECRET` env var on backend differs between insert and read (i.e. backend was restarted with a new secret) | Ensure `JWT_SECRET` is stable and re-POST |

---

## 7. Audit log

Each row migrated should be logged here once Path A or Path B completes.

| Date | Operator | Provider | Tenant | Environment | Notes |
| --- | --- | --- | --- | --- | --- |
| _pending_ | | PAYSTACK | global | production | |
| _pending_ | | MASTERCARD | global | production | |
| _pending_ | | CYBERSOURCE | global | production | |
| _pending_ | | BANK_TRANSFER | global | production | |

---

## 8. Related code

- Schema: [`backend/migrations/054_payment_provider_config.sql`](../backend/migrations/054_payment_provider_config.sql)
- Model + `resolve()` priority logic: [`backend/src/models/payment_provider_config.rs`](../backend/src/models/payment_provider_config.rs)
- Admin endpoint (POST / GET / DELETE): [`backend/src/handlers/admin/payment_providers.rs`](../backend/src/handlers/admin/payment_providers.rs)
- Encryption service (AES-256-GCM, key derived from `JWT_SECRET`): [`backend/src/services/encryption.rs`](../backend/src/services/encryption.rs)
- Billing handler that consumes resolved configs: [`backend/src/handlers/billing.rs`](../backend/src/handlers/billing.rs)
- Path A operator helper: [`deploy/scripts/migrate-payment-providers.sh`](../deploy/scripts/migrate-payment-providers.sh) (+ [`payment-providers.example.json`](../deploy/scripts/payment-providers.example.json))
- PayPro source-of-truth domain: `cloud.paypro.oms.billing.PaymentProviderConfig` in the `paypro-oms` repo
