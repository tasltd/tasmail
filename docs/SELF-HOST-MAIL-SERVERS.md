# Optional: Self-host Postfix + Dovecot for TASMail

> **You don't need this for TASMail itself.** TASMail is a webmail UI that works
> against any IMAP/SMTP server (Gmail, Outlook, Zoho, FastMail, your corporate
> Exchange, an existing Dovecot, …). Users sign up, then attach their own server
> credentials in the onboarding wizard.
>
> This guide is for operators who want to *also* offer their own mail server
> alongside TASMail (e.g., to host `@example.com` mailboxes on the same VPS).
> The TASMail backend ships with `deploy/scripts/setup-all.sh` for this scenario,
> but it is **not** wired into the workstation/proxy environment that powers
> `mail.techatscale.io`.

---

## Status on `mail.techatscale.io` today

| Component | State |
|---|---|
| TASMail React SPA | ✅ Live at `https://mail.techatscale.io/` |
| TASMail Rust backend (REST + WS) | ✅ Live behind the SSH tunnel + Apache vhost |
| `payment_provider_config` (DB-backed credentials) | ✅ Schema applied; rows can be created via `POST /api/admin/payment-providers` |
| BYOK onboarding (signup → IMAP + SMTP wizard) | ✅ End-to-end |
| Postfix / Dovecot / Rspamd | ❌ **Not installed.** Intentionally deferred — TASMail's identity is webmail-for-other-servers, not a mail server itself. |
| MX record for `techatscale.io` | Still points at `swmail.techatscale.io` (legacy, unrelated) |
| SPF for `techatscale.io` | Includes `_spf.elasticemail.com` (legacy, unrelated) |

---

## When you would install the optional mail server

* You want TASMail to be a *full* email solution for a small team — accounts on `@yourdomain.com`, plus the webmail UI.
* You want to test TASMail's BYOK flow against a known-good local Dovecot rather than a real provider.
* You want one VPS that does both webmail AND mail transport.

If none of these apply, **skip this guide entirely.** Sign up to TASMail, point it at Gmail or your existing IMAP server, and you're done.

---

## Where to install (NOT on the proxy host)

The proxy at `140.82.32.141` (which serves `cim.techatscale.io`, `servat.techatscale.io`, `cloudy.techatscale.io`, `mail.techatscale.io`, `swmail.techatscale.io`, etc.) is **shared infrastructure**. Do NOT install Postfix/Dovecot there — it would compete with the legacy `swmail` setup that currently terminates `@techatscale.io` mail and break unrelated services.

Install on:

* The TASMail workstation/host (`tas-src-1`), if you only need a single-user/test setup. Keep ports 25/465/587/993/143 bound to `127.0.0.1` so the server is reachable only via local TASMail.
* A dedicated VPS (provision via `cloudy-tas` if you want to use Tech-at-Scale's cloud manager). This is the production answer.

---

## Install (one-shot, dedicated VPS)

The repo includes a turnkey installer. From a fresh Ubuntu 22.04+ VPS as root:

```bash
git clone https://github.com/tasltd/tasmail
cd tasmail/deploy/scripts
./setup-all.sh \
    --domain example.com \
    --hostname mail.example.com
```

What `setup-all.sh` does:

1. Installs Postfix, Dovecot, Rspamd, OpenDKIM, certbot
2. Generates an Ed25519 DKIM key for `example.com`
3. Requests a Let's Encrypt cert for `mail.example.com` (HTTP-01)
4. Configures Postfix with the right `smtpd_*_restrictions` for inbound + submission
5. Configures Dovecot with the SQL backend pointed at TASMail's `mailboxes` table (so a TASMail account *can* double as a real mailbox if you want)
6. Prints the DNS records you need to add (A, MX, SPF, DKIM, DMARC)
7. Runs `mail-tester` and reports the score

You then add the printed DNS records, wait for TTL, and verify with:

```bash
./test-deliverability.sh --domain example.com
```

---

## DNS records the installer prints

For `example.com` you will need:

| Record | Type | Value |
|---|---|---|
| `mail.example.com` | A | _your VPS IP_ |
| `example.com` | MX 10 | `mail.example.com.` |
| `example.com` | TXT | `v=spf1 mx -all` |
| `default._domainkey.example.com` | TXT | _printed by installer_ |
| `_dmarc.example.com` | TXT | `v=DMARC1; p=quarantine; rua=mailto:dmarc@example.com` |

Reverse DNS (PTR) for the VPS IP must also resolve to `mail.example.com`. Most cloud providers expose this in their console.

---

## Pointing TASMail at the new server

After Postfix/Dovecot is up, a TASMail user signs up the normal way and enters
their server credentials in the onboarding wizard:

```
IMAP: imap.example.com:993 (SSL/TLS)
SMTP: smtp.example.com:587 (STARTTLS)
Username: alice@example.com
Password: their_mailbox_password
```

That's the same flow as connecting to Gmail or Outlook — TASMail doesn't care
that the server is "yours".

---

## Remaining IMAP-handler migration to BYOK

The `folders` handler now reads the per-user `imap_configurations` row. The
following handlers still use the global `state.config.imap` and will be
migrated to `ImapService::for_user(state, user_id)` next:

* `handlers/messages.rs` (15 call sites)
* `handlers/mobile.rs` (12 call sites)
* `handlers/eml.rs` (11 call sites)
* `handlers/quota.rs` (2 call sites)
* `handlers/nlp_search.rs` (1 call site)

Until they migrate, TASMail can list folders against the user's IMAP server but
falls back to the global Dovecot host (often unconfigured) for message reads.
Track this work in TMAIL PM and migrate one handler per PR.

---

## Don't blindly run `setup-all.sh` on this workstation

`setup-all.sh` will try to bind ports 25, 465, 587, 993, 143 globally and to
modify `/etc/postfix/main.cf`, `/etc/dovecot/dovecot.conf`, and the system's
Let's Encrypt store. On the TASMail workstation that hosts the dev SPA, this
is overkill and would conflict with the `byok.tasmail` BYOK signup pivot.

If you really want a local Dovecot to test the BYOK wizard against, run:

```bash
sudo apt install dovecot-imapd
sudo cat >/etc/dovecot/local.conf <<'EOF'
listen = 127.0.0.1
protocols = imap
mail_location = maildir:~/Maildir
ssl = no
disable_plaintext_auth = no
EOF
sudo systemctl restart dovecot
```

Then in TASMail's onboarding wizard pick **Other / Custom** and enter
`127.0.0.1:143` with encryption `none`. The wizard will let you LOGIN as your
local Unix account and you can test message listing without leaving the box.
