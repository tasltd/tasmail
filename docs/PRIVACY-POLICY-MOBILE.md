# TASMail Mobile — Privacy Policy

**Effective date:** 2026-05-28
**Last updated:** 2026-05-28
**Operator:** Tech at Scale Ltd, Accra, Ghana
**Contact:** privacy@techatscale.io
**Data Protection Officer:** dominic@techatscale.io
**Public URL:** https://mail.techatscale.io/privacy-policy/mobile

This page is the canonical Privacy Policy linked from Google Play Store and
Huawei AppGallery Connect listings for the **TASMail Mobile** application
(package `io.techatscale.tasmail_mobile`). The same policy text is rendered
under the public web path above so app-store reviewers can verify it.

It supplements — and is consistent with — the main TASMail web policy at
https://mail.techatscale.io/privacy-policy.

---

## 1. Who we are

Tech at Scale Ltd ("Tech at Scale", "we", "us") is a private limited company
registered in Ghana (RoC: see TMAIL-43). We operate the TASMail service at
`mail.techatscale.io` and publish the TASMail Mobile application on Google
Play Store and Huawei AppGallery. We are registered as a Data Controller with
the Ghana Data Protection Commission (DPC) under [registration number to be
filled when TMAIL-44 completes] and we comply with the Data Protection Act,
2012 (Act 843).

If you are reading this from the European Economic Area, the United Kingdom,
or California: we additionally treat your data in line with the GDPR, UK GDPR,
and the CCPA respectively, even though we are not headquartered in those
jurisdictions.

---

## 2. What TASMail Mobile is

TASMail Mobile is a **Bring-Your-Own-Key (BYOK) email client**. You connect
the credentials of an email account you already own (Gmail, Outlook, Yahoo,
Zoho, FastMail, iCloud, ProtonMail Bridge, or any IMAP/SMTP server) and
TASMail proxies the IMAP and SMTP traffic on your behalf. Your email is
delivered to and stored by your existing provider — **TASMail does not host
mailboxes, does not store the bodies of your messages, and does not read your
mail**.

---

## 3. What data we collect

### 3.1 Data you provide directly

| Category | Example | Why |
|---|---|---|
| Account info | Email address, display name, password (hashed) | To create your TASMail account |
| IMAP/SMTP credentials | Server hostname, port, username, password | To proxy mail on your behalf |
| Contact information | If you grant permission, names + emails from your device contacts | Only to populate the "To/CC/BCC" autocomplete; never uploaded to TASMail servers unless you explicitly save a contact |
| Mail you write | Drafts, outgoing messages | Held in memory while composing; written to your SMTP server when you tap Send |
| Mail you receive | Headers + bodies | Cached **locally on your device only** for offline access; encrypted on disk |

### 3.2 Data we generate automatically

| Category | Example | Why |
|---|---|---|
| Crash reports | Stack traces (via Sentry) | To diagnose app crashes. Stack traces are scrubbed of email content, recipient addresses, and IMAP credentials before transmission. |
| Diagnostic logs | API endpoint, HTTP status, latency | To measure performance. No mail content. |
| Usage analytics | Screen names visited, feature usage counts (via self-hosted Matomo) | To prioritise features. No content or recipient identifiers. We do not use Google Firebase Analytics. |
| Device identifier | A randomly generated UUID stored in app local storage | To deduplicate push notifications. Not linked to your phone's hardware IMEI/MEID. |

### 3.3 Data we do NOT collect

- We do not collect contents of your messages on our servers
- We do not scan, index, or train AI models on your mail
- We do not sell, rent, or share your data with advertisers
- We do not use Google Firebase Analytics, Crashlytics, or any other ad-funded SDK
- We do not collect precise location
- We do not access your microphone, camera (except when you tap "Attach photo"), or SMS

---

## 4. Permissions the app requests

| Android permission | Used for | When prompted |
|---|---|---|
| `INTERNET` | Connecting to your IMAP/SMTP server and to TASMail | Always — required for the app to work |
| `USE_BIOMETRIC`, `USE_FINGERPRINT` | Biometric unlock | First time you enable biometric unlock |
| `CAMERA` | Capturing a photo to attach to an outgoing email | First time you tap "Attach photo → Take photo" |
| `READ_CONTACTS` | Suggesting recipients from your phone contacts | First time you tap a contact picker (you can decline — autocomplete then only suggests from TASMail-saved contacts) |
| `READ_MEDIA_IMAGES`, `READ_MEDIA_VIDEO` | Attaching files you already have on the device | First time you tap "Attach → From device" |
| `POST_NOTIFICATIONS` (Android 13+) | New-mail notifications | First launch |

You can revoke any of these later in Android Settings → Apps → TASMail Mobile
→ Permissions. Revoking a permission disables the corresponding feature but
the rest of the app continues to work.

---

## 5. Where your data lives

| Type | Location | Encryption |
|---|---|---|
| Account email + hashed password | TASMail PostgreSQL on `tas-src-1`, Accra, Ghana | At rest: PostgreSQL TDE; in transit: TLS 1.3 |
| IMAP/SMTP password | TASMail PostgreSQL, AES-256-GCM with a per-user key derived from server-side `JWT_SECRET` | At rest: AES-256-GCM; in transit: TLS 1.3 |
| Mail bodies | Your IMAP provider's servers (Google, Microsoft, Zoho, your own…) — **never on TASMail servers**. A copy is cached on your device in a SQLite database encrypted with the Android keystore. | On device: SQLCipher + Android keystore; provider-side: their policy |
| Crash reports | Sentry (`tasltd` org, EU region) | TLS 1.3 in transit; Sentry's at-rest encryption |
| Usage analytics | Self-hosted Matomo on `tas-src-1`, Accra, Ghana | At rest: filesystem-level; in transit: TLS 1.3 |

We do **not** transfer your data to the United States or any other jurisdiction
outside Ghana **except** for Sentry crash reports (EU region, GDPR-compliant
data processor) and DNS resolution to your IMAP provider's servers (which is
inherent to email — we cannot proxy Gmail without contacting Google's
servers).

---

## 6. How long we keep your data

| Type | Retention |
|---|---|
| Account info | Until you delete your account or 24 months after last login, whichever is sooner |
| IMAP credentials | Until you remove the IMAP configuration or delete your account |
| Crash reports | 90 days (Sentry default) |
| Usage analytics | 12 months, then aggregated |
| On-device mail cache | Up to 90 days of mail by default — configurable in Settings → Storage → Offline cache window |

You can trigger immediate deletion in three ways:

1. **In app:** Settings → Account → Delete account. Triggers permanent deletion within 30 days.
2. **By email:** privacy@techatscale.io with subject "Delete my account". We respond within 5 business days.
3. **Uninstall the app only:** removes the on-device cache. Your TASMail account on our servers persists — use option 1 to fully delete.

---

## 7. Your rights

Under the Ghana Data Protection Act 2012, GDPR (where applicable), and CCPA
(where applicable), you have the right to:

- **Access** a machine-readable export of your TASMail data — Settings → Account → Export data
- **Correct** inaccurate information — Settings → Account → Profile
- **Delete** your account (see §6)
- **Restrict** processing — email privacy@techatscale.io
- **Object** to processing for direct marketing — we don't do direct marketing from this app, but the request is honoured if you make it
- **Data portability** — the export is in MBOX + JSON, importable into any other client
- **Withdraw consent** at any time, for processing that relies on consent
- **Lodge a complaint** with the Ghana Data Protection Commission (https://www.dataprotection.org.gh) or your local supervisory authority

Response window: **30 days** from the date we receive your verified request,
extendable once by another 30 days for complex requests with notice to you.

---

## 8. Children

TASMail Mobile is not directed at children under 13. We do not knowingly
collect data from children under 13. If you believe a child has registered,
email privacy@techatscale.io and we will delete the account.

---

## 9. Third-party services

We use the following third-party data processors, each under a written
Data Processing Agreement:

| Processor | Purpose | Jurisdiction | DPA URL |
|---|---|---|---|
| Sentry | Crash reporting | EU region (Frankfurt) | https://sentry.io/legal/dpa/ |
| Aveshost (beta) / Smart Infraco (production) | Hosting `tas-src-1` | Ghana | Contractual DPA on file |
| Google Play Console | App distribution | Global (Google LLC) | https://play.google.com/about/developer-distribution-agreement.html |
| Huawei AppGallery Connect | App distribution | Global (Huawei Services Hong Kong) | https://developer.huawei.com/consumer/en/doc/start/AppGallery-connect-policy |

We do **not** use:

- Google Firebase Analytics
- Google Crashlytics
- Facebook SDK
- Any advertising SDK
- Any third-party ad network

---

## 10. Changes to this policy

When we change this policy in a material way (e.g. start using a new processor,
collect a new category of data), we will:

1. Update the "Last updated" date at the top of this page
2. Show an in-app banner the next time you open TASMail Mobile
3. For changes that require renewed consent under Ghana Data Protection Act,
   block app access until you accept or decline

We will keep the previous version archived at
`https://mail.techatscale.io/privacy-policy/mobile/v/<date>`.

---

## 11. Contact us

| Reason | Contact |
|---|---|
| Privacy questions, data subject requests | privacy@techatscale.io |
| Security vulnerability reports | security@techatscale.io |
| Customer support | support@techatscale.io |
| Postal mail | Tech at Scale Ltd, Accra, Ghana (full address available on request — we don't publish for spam reasons) |

---

*This policy was authored in plain English with the goal that an ordinary
non-lawyer can understand what we do with their data. If anything reads as
ambiguous, that's a bug — email privacy@techatscale.io and we'll fix the
wording in the next revision.*
