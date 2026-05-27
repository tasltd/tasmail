# IP Warm-up Runbook (TMAIL-17)

> Operational guide for warming up a new sending IP before public launch.
> Must complete the full 8-week schedule before opening signup to the public,
> or before pointing a high-volume customer's MX records at TASMail.

## Why warm-up matters

Mailbox providers (Gmail, Outlook, Yahoo, Apple) score new IPs as untrusted.
Spiking from 0 → 10k mail/day on a cold IP almost guarantees inbox filtering
and outright blocks. The remedy is a predictable ramp where bounce rates,
spam-complaint rates, and engagement signals can be tuned at each step.

## Schedule (8 weeks, 56 days)

The canonical numbers live in `backend/src/models/warmup.rs`
(`WARMUP_WEEKLY_LIMITS`) and `deploy/scripts/ip-warmup.sh`. Keep this table
in sync with both.

| Week | Daily Limit | Operational focus |
|------|-------------|-------------------|
| 1 | 50 | Initial warm-up — low volume, establish reputation |
| 2 | 100 | Gradual increase — monitor bounce rates |
| 3 | 250 | Moderate volume — **enroll in Google Postmaster Tools** and check spam placement |
| 4 | 500 | Steady growth — review engagement metrics in Google Postmaster Tools |
| 5 | 1,000 | Scaling up — maintain consistent sending patterns |
| 6 | 2,500 | High volume ramp — monitor deliverability scores |
| 7 | 5,000 | Near-full capacity — verify inbox placement rates |
| 8 | Unlimited | Warm-up complete — sending is unrestricted |

Week 8 stores `daily_limit = 0` to mean "no cap". The model treats this as
unlimited (`remaining_today = u32::MAX`).

## Pre-flight (do once, before Week 1)

1. SPF, DKIM (selector + 2048-bit key), and DMARC (`p=quarantine` initially,
   `rua=mailto:postmaster@<domain>`) configured. Run
   `deploy/scripts/test-deliverability.sh` to validate.
2. Reverse DNS (PTR) on the sending IP resolves to the mail host name and
   forward-confirms (`dig -x <ip>` matches `dig <hostname>`).
3. TLS certificate is valid and Postfix is configured for STARTTLS.
4. From-address uses a real, monitored domain — never a no-reply on a
   freshly registered domain.
5. List-Unsubscribe header is set on all bulk mail (RFC 8058 one-click).

## Week 3 — Google Postmaster Tools enrollment

This is the single highest-leverage operational step in the schedule.

1. Go to https://postmaster.google.com and sign in with a Google account
   that controls DNS for the sending domain.
2. Add the domain (e.g. `mail.techatscale.io`).
3. Verify ownership via the supplied TXT record (added to the domain's DNS).
4. Wait 24–48h for the first data to populate (you need ≥100/day of Gmail
   traffic, which the Week 3 ramp produces).
5. Bookmark the dashboards: **Spam Rate**, **IP Reputation**, **Domain
   Reputation**, **Authentication**, **Encryption**, **Delivery Errors**.

## Daily checks (every weekday during Weeks 1–8)

* Run `deploy/scripts/ip-warmup.sh --check` — shows today's cap and
  remaining quota for each tracked IP.
* Run `deploy/scripts/ip-warmup.sh --status` — full progress for every
  tracked IP.
* From Week 3 onward, open Google Postmaster Tools and confirm:
  * Spam rate < 0.10% (Gmail's published threshold is 0.30%, keep margin)
  * IP Reputation: High or Medium (never Low/Bad)
  * Domain Reputation: High or Medium
  * Authentication: 100% pass on SPF/DKIM/DMARC

If any metric degrades, **pause the ramp at the current week** until the
underlying issue is resolved — do not advance to the next week's cap.

## Admin API (programmatic control)

All endpoints are admin-gated (`auth_service::require_admin`).

| Method | Path | Purpose |
|--------|------|---------|
| `GET` | `/api/admin/warmup/schedule` | Returns the canonical 8-week schedule |
| `GET` | `/api/admin/warmup/status` | Returns warm-up state for every tracked IP |
| `POST` | `/api/admin/warmup/start` | Begins tracking a new IP at day 1 |

State lives in the `ip_warmup_tracking` table (migration
`053_ip_warmup.sql`). The standalone CLI script
`deploy/scripts/ip-warmup.sh` keeps its own JSON state file at
`${TASMAIL_STATE_DIR:-/var/lib/tasmail}/warmup-state.json` for operators
who prefer not to go through the API.

## When the ramp completes

After day 56, `WarmupStatus.completed = true` and the daily limit is
treated as unlimited. The IP is still being tracked — keep monitoring
Postmaster Tools weekly for the first 90 days to catch any late-onset
reputation degradation.

## References

* [Gmail bulk-sender guidelines](https://support.google.com/mail/answer/81126)
* [Google Postmaster Tools](https://postmaster.google.com)
* [Microsoft SNDS](https://sendersupport.olc.protection.outlook.com/snds/)
* [Yahoo Sender Hub](https://senders.yahooinc.com/)
* [M3AAWG sender best practices](https://www.m3aawg.org/published-documents)
