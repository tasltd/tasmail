# Enabling the DNS-MX onboarding path

> **Audience:** TASMail operators who want to offer a *secondary* signup option
> where new users get a real mailbox on a TASMail-managed domain (Postfix +
> Dovecot), instead of bringing their own IMAP/SMTP credentials.
>
> **Default state:** disabled. BYOK is the primary path.
>
> **Status:** the feature flag, signup-wizard branch, and managed-mailbox UI
> step ship today. The provisioning endpoint that actually creates the Dovecot
> mailbox is tracked in **TMAIL-167** and intentionally returns a placeholder
> until a real mail server is wired in. The steps below describe the full
> end-to-end setup; do them in order.

---

## Prerequisites

| Item | Where |
|---|---|
| TASMail backend running and healthy | `systemctl --user status tasmail-backend.service` |
| Postfix + Dovecot installed on a **dedicated** VPS (NOT the proxy host) | `docs/SELF-HOST-MAIL-SERVERS.md` |
| Public domain you control (DNS access) | e.g. `mail.yourdomain.com` |
| Outbound port 25 reachable from the mail VPS | `nc -zv aspmx.l.google.com 25` |
| `doveadm` and `postmap` accessible to the TASMail backend user | SSH key from TASMail host → mail VPS |

---

## Step 1: install Postfix/Dovecot on a dedicated VPS

Follow `docs/SELF-HOST-MAIL-SERVERS.md`. Briefly:

```bash
git clone https://github.com/tasltd/tasmail
cd tasmail/deploy/scripts
./setup-all.sh \
    --domain yourdomain.com \
    --hostname mail.yourdomain.com
```

This installs Postfix + Dovecot + Rspamd + OpenDKIM, requests a Let's Encrypt
cert for `mail.yourdomain.com`, generates a DKIM key, and prints the DNS
records you need to add.

---

## Step 2: add the DNS records the installer printed

| Record | Type | Value |
|---|---|---|
| `mail.yourdomain.com` | A | _your mail VPS IP_ |
| `yourdomain.com` | MX 10 | `mail.yourdomain.com.` |
| `yourdomain.com` | TXT | `v=spf1 mx -all` |
| `default._domainkey.yourdomain.com` | TXT | _printed by installer_ |
| `_dmarc.yourdomain.com` | TXT | `v=DMARC1; p=quarantine; rua=mailto:dmarc@yourdomain.com` |

Reverse DNS (PTR) for the VPS IP must resolve to `mail.yourdomain.com`. Most
providers expose this in their console.

Wait for the records to propagate (15–60 minutes) and verify:

```bash
dig +short yourdomain.com MX
dig +short mail.yourdomain.com A
dig +short default._domainkey.yourdomain.com TXT
```

---

## Step 3: tell TASMail about the managed mail server

Currently this is two env vars on the TASMail host (full DB-backed config will
ship when TMAIL-167 lands):

```bash
sudo systemctl --user edit tasmail-backend.service
```

Add an override:

```ini
[Service]
Environment=TASMAIL_MANAGED_DOMAIN=yourdomain.com
Environment=TASMAIL_MANAGED_DOVECOT_HOST=mail.yourdomain.com
```

Reload and restart:

```bash
systemctl --user daemon-reload
systemctl --user restart tasmail-backend.service
```

---

## Step 4: turn the feature flag on

Either via the admin dashboard at `https://mail.techatscale.io/admin/feature-flags`
(toggle **DNS-MX onboarding**), or via the API:

```bash
curl -X PATCH https://mail.techatscale.io/api/admin/feature-flags/dns_mx_onboarding_enabled \
    -H "Authorization: Bearer $TASMAIL_ADMIN_TOKEN" \
    -H "Content-Type: application/json" \
    -d '{"enabled": true}'
```

The change is visible to authenticated callers immediately and to public callers
within 60 seconds (Redis cache TTL — see TMAIL-162).

---

## Step 5: verify the wizard

1. Visit `https://mail.techatscale.io/signup` in a private window
2. Sign up with a fresh email/password
3. The onboarding wizard should now show **two tiles**:
   - "Connect an existing account" (BYOK, recommended)
   - "Get a new mailbox on this server" (the new DNS-MX path)
4. Pick the second tile — you should see the local-part picker

When TMAIL-167 lands, "Provision mailbox" will:

- POST to `/api/admin/mailbox-provision` with the chosen local-part
- Backend `doveadm user add yourname@yourdomain.com` on the mail VPS
- Insert an `imap_configurations` row pointing at `mail.yourdomain.com:993`
  with the user's TASMail account password (so the user doesn't have to enter
  IMAP creds — they're managed)
- Return the user to `/app` with their new mailbox loaded

Until then, the button surfaces a "TMAIL-167 not yet implemented" message.

---

## Disabling the path

Toggle the flag back off in the admin dashboard, or:

```bash
curl -X PATCH https://mail.techatscale.io/api/admin/feature-flags/dns_mx_onboarding_enabled \
    -H "Authorization: Bearer $TASMAIL_ADMIN_TOKEN" \
    -H "Content-Type: application/json" \
    -d '{"enabled": false}'
```

The mail server keeps running; only the new-mailbox UI tile disappears. Existing
users who already have a managed mailbox keep working — TASMail still talks to
their `imap_configurations` row pointing at the managed server.

---

## Troubleshooting

| Symptom | Fix |
|---|---|
| Wizard shows only the BYOK tile after toggling the flag | Wait 60s for the Redis cache TTL, or invalidate manually |
| `dig +short yourdomain.com MX` returns the wrong host | Update the MX record at your DNS provider; TTL must elapse |
| Inbound test mail bounces with `relay access denied` | Check `mynetworks` in `/etc/postfix/main.cf` on the mail VPS |
| Outbound mail lands in spam | Verify SPF, DKIM, DMARC; check IP reputation at `mail-tester.com` |
| TMAIL-167 endpoint returns 500 | Check that `TASMAIL_MANAGED_DOMAIN` is set and the mail VPS SSH key is in `~/.ssh/authorized_keys` on the mail host |

---

## Related docs

- `docs/SELF-HOST-MAIL-SERVERS.md` — installs Postfix + Dovecot
- `docs/DEPLOYMENT-GUIDE.md` — overall production deployment
- `docs/SECURITY.md` — DKIM/SPF/DMARC posture, IP reputation guidance
- TMAIL-165 — feature_flags backend
- TMAIL-166 — admin dashboard
- TMAIL-167 — provisioning endpoint (planned)
- TMAIL-168 — wizard branching (shipped)
