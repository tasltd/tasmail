# TASMail vs Competitors: Feature Comparison

**Date:** 2026-03-22
**Competitors:** Thunderbird, Zoho Mail, Gmail, Microsoft Outlook

---

## 1. Email Basics

| Feature | TASMail (v1.0) | Thunderbird | Zoho Mail | Gmail | Outlook |
|---------|---------------|-------------|-----------|-------|---------|
| Rich text compose | TipTap editor | Built-in | Yes | Yes | Yes |
| HTML + plain text read | DOMPurify sanitized | Yes | Yes | Yes | Yes |
| Reply/Forward | Yes | Yes | Yes | Yes | Yes |
| Attachments | 25 MB limit | No practical limit | 250 MB–1 GB | 25 MB (Drive for larger) | 25 MB (OneDrive) |
| Folders/Labels | IMAP folders | Folders + virtual folders + tags | Folders + labels + tags | Labels, nested labels, categories | Folders + categories + Focused Inbox |
| Search | IMAP SEARCH + Dovecot FTS | Full-text, advanced filters | Full-text, advanced | NLP + AI search overview | Full-text + AI Immersive Search |
| Threading | In-Reply-To/References | Threaded view | Threaded | Native conversation view | Conversation + Focused/Other |
| Drafts auto-save | Not documented | Yes | Yes | Yes | Yes |
| Signatures | Per-account (P2) | Multiple | Yes, HTML | Multiple per account | Multiple |
| Keyboard shortcuts | Gmail-like (P2) | Extensive | Yes | Extensive | Extensive |
| Email templates | Not planned | Via add-ons | Yes | Yes | Yes |
| Snooze/Schedule send | Not planned | Add-ons only | Schedule send | Snooze + schedule send | Schedule send |
| Email recall | Not planned | No | Yes | Undo send (30s) | Recall within org |
| Sieve filter rules | UI for Sieve (P2) | Message filters | Yes | Filters (extensive) | Rules + Sweep |
| Real-time push | IMAP IDLE → WebSocket (<3s) | IMAP IDLE | Push notifications | Push (near instant) | Push (near instant) |

---

## 2. Security & Privacy

| Feature | TASMail | Thunderbird | Zoho Mail | Gmail | Outlook |
|---------|---------|-------------|-----------|-------|---------|
| TLS in-transit | TLS 1.2+ everywhere | Yes | Yes | Yes | Yes |
| Encryption at rest | Server-level Maildir | Local storage | AES-256 | AES-256 | AES-256 |
| End-to-end encryption | Deferred to v2 | OpenPGP built-in, S/MIME | S/MIME (Premium) | CSE (Workspace) | S/MIME, Purview OME |
| 2FA/MFA | Not documented v1 | Via provider | MFA, hardware keys | 2FA, passkeys, keys | MFA, authenticator, passkeys |
| Spam filtering | Rspamd milter | Via provider | Built-in | 99.9%+ ML detection | Exchange Online Protection |
| DKIM/SPF/DMARC | 2048-bit RSA DKIM | N/A (client) | Yes | Yes | Yes |
| Phishing protection | DOMPurify sanitization | Warning banners | Advanced threat protection | ML-based detection | Safe Links, Safe Attachments |
| Rate limiting | 100 req/min, 10 login/min | N/A | Yes | Yes | Yes |
| Brute force protection | Fail2ban + lockout (20 failures) | N/A | Yes | CAPTCHA + lockout | Yes |
| DLP | Not planned | No | Yes (Premium) | Yes (Workspace) | Yes (Purview DLP) |
| Password hashing | Argon2id (best-in-class) | N/A | Not disclosed | Not disclosed | Not disclosed |
| DANE support | Mentioned in architecture | N/A | Not documented | Not documented | Not documented |
| **Data sovereignty** | **YES (self-hosted)** | **YES (local)** | No | No | No |

---

## 3. Calendar & Contacts

| Feature | TASMail | Thunderbird | Zoho Mail | Gmail | Outlook |
|---------|---------|-------------|-----------|-------|---------|
| Calendar | **NOT in v1** | Built-in (Lightning) | Built-in | Google Calendar | Outlook Calendar |
| Contacts app | Autocomplete only | Built-in address book | Built-in | Google Contacts | People/Contacts |
| Tasks/To-do | Not planned | Built-in | Built-in | Google Tasks | Microsoft To Do |
| Meeting scheduling | Not planned | Not built-in | Yes | AI-assisted scheduling | FindTime, Copilot |
| CalDAV/CardDAV | Deferred post-v1 | Yes (native) | Yes | Proprietary + CalDAV | Exchange ActiveSync |

**Assessment:** Largest feature gap. Every competitor has integrated calendar/contacts/tasks.

---

## 4. Collaboration

| Feature | TASMail | Thunderbird | Zoho Mail | Gmail | Outlook |
|---------|---------|-------------|-----------|-------|---------|
| Shared mailboxes | Not planned | N/A | Yes | Yes (Groups, delegation) | Yes (full delegation) |
| Email delegation | Not planned | N/A | Yes | Yes (up to 25 delegates) | Yes (send-on-behalf) |
| Internal comments | Not planned | No | Yes (Streams) | No | No (Teams integration) |
| Team chat | Not planned | IRC/XMPP/Matrix | Zoho Cliq | Google Chat | Microsoft Teams |
| Doc collaboration | Not planned | No | Zoho Writer/Sheet/Show | Docs/Sheets/Slides | Word/Excel/PowerPoint |
| Video conferencing | Not planned | No | Zoho Meeting | Google Meet | Teams |

---

## 5. Mobile Support

| Feature | TASMail | Thunderbird | Zoho Mail | Gmail | Outlook |
|---------|---------|-------------|-----------|-------|---------|
| iOS app | No (v2 Flutter) | In dev (2026 beta) | Yes | Yes | Yes |
| Android app | No (v2 Flutter) | Yes (K-9 Mail) | Yes | Yes | Yes |
| PWA/responsive | PWA mentioned, 375px+ | No (desktop only) | Web mobile | Yes | Yes |
| Push notifications | Via PWA | K-9/native | Yes | Yes | Yes |
| ActiveSync | Deferred v2 | No | Yes | No (proprietary) | Yes (native) |
| Low-bandwidth mode | Gap identified (GAP-M) | No | No | Yes (Lite) | Yes (Lite) |

---

## 6. Offline Access

| Feature | TASMail | Thunderbird | Zoho Mail | Gmail | Outlook |
|---------|---------|-------------|-----------|-------|---------|
| Offline reading | Not documented | Yes (full) | Yes | Yes (Chrome) | Yes |
| Offline compose | Not documented | Yes | Yes | Yes | Yes |
| Offline search | Not documented | Yes (local index) | Limited | Yes (cached) | Yes (cached) |
| Reconnect sync | Not documented | Automatic | Automatic | Automatic | Automatic |

---

## 7. Integrations & Extensibility

| Feature | TASMail | Thunderbird | Zoho Mail | Gmail | Outlook |
|---------|---------|-------------|-----------|-------|---------|
| API access | REST + WebSocket | N/A (client) | APIs | Gmail API | Microsoft Graph |
| Extension ecosystem | Not planned | Add-ons marketplace | Zoho Marketplace (40+ apps) | Workspace Marketplace (1000s) | AppSource (1000s) |
| Webhooks | WebSocket events | No | Zoho Flow | Pub/Sub | Power Automate |
| IMAP/POP | IMAP (Dovecot); no POP3 | Both | Yes (paid) | Yes | Yes |
| SSO/SAML/OIDC | Not documented | N/A | Yes | Yes | Yes (Azure AD) |
| LDAP | Not documented | N/A | Yes | Google Directory | Active Directory |
| Exchange support | Not planned | Yes (native, 2025) | No | No | Native |

---

## 8. AI Features

| Feature | TASMail | Thunderbird | Zoho Mail | Gmail | Outlook |
|---------|---------|-------------|-----------|-------|---------|
| AI compose/drafting | Not planned | No | Zia AI | Gemini 3 "Help Me Write" | Copilot |
| Email summarization | Not planned | No | Zia AI | Gemini AI Overview | Copilot summaries |
| Smart replies | Not planned | No | Zia suggestions | Smart Reply (contextual) | Copilot replies |
| AI search | Not planned | No | No | NLP + AI Overview | Immersive Search |
| BYOK AI | Not planned | No | **Yes (GPT, Gemini, Claude, Cohere)** | No (Gemini only) | No (Copilot only) |

---

## 9. Storage & Pricing

| Aspect | TASMail | Thunderbird | Zoho Mail | Gmail | Outlook |
|--------|---------|-------------|-----------|-------|---------|
| Storage | Self-hosted (server disk) | Local disk | 5–50+ GB | 15 GB–5 TB | 15–100 GB + 1 TB OneDrive |
| Free tier | Self-hosted (infra cost) | Free (open source) | Free (5 users, 5 GB/user) | Free (15 GB) | Free (15 GB) |
| Entry paid | GHS 15–25/user/mo (BYO) | Free | $1/user/mo | $7/user/mo | ~$6/user/mo |
| Enterprise | GHS 110/user/mo | N/A | $7/user/mo | $26.40/user/mo | ~$22/user/mo |
| Attachment limit | 25 MB | Provider-dependent | 250 MB–1 GB | 25 MB | 25 MB |
| Multi-domain | Yes (100+) | N/A | Yes | Yes | Yes |

---

## 10. Admin & Business

| Feature | TASMail | Thunderbird | Zoho Mail | Gmail | Outlook |
|---------|---------|-------------|-----------|-------|---------|
| Domain management | Yes | N/A | Yes | Yes | Yes |
| User provisioning | Yes | N/A | Yes + bulk | Yes + directory sync | Yes + Azure AD |
| Quota management | Yes (per-user) | N/A | Yes | Yes (pooled) | Yes |
| Admin dashboard | Yes (health, sessions) | N/A | Extensive console | Admin Console | 365 Admin |
| RBAC | user/domain_admin/super_admin | N/A | Yes | Multiple admin roles | Granular roles |
| Email archiving | Not planned | N/A | Yes (eDiscovery) | Google Vault | In-Place Archive |
| Retention policies | Not planned | N/A | Yes | Yes | Yes |
| DLP policies | Not planned | N/A | Yes | Yes | Yes (Purview) |
| Migration tools | Not documented | N/A | POP/IMAP/PST | Data migration service | Hybrid/PST/IMAP |
| White labeling | Not planned | N/A | Yes (Premium) | No | No |

---

## Competitive Position Summary

### Where TASMail MATCHES
- Core email operations (compose, read, reply, forward, search, folders)
- Email authentication (DKIM, SPF, DMARC) — matches or exceeds
- Real-time notifications (WebSocket push)
- Basic admin (domains, users, quotas, aliases, RBAC)
- Multi-domain support
- HTML sanitization security
- Transport encryption

### Where TASMail EXCEEDS
- **Data sovereignty** — true self-hosted; critical for Ghana DPA Act 843
- **Resource efficiency** — <100 MB RAM; single VPS deployable
- **Deployment simplicity** — single Rust binary
- **GHS-denominated pricing** — eliminates currency risk vs Google/Microsoft
- **Performance targets** — sub-200ms UI, <150ms API p95
- **Memory safety** — Rust backend vs PHP/Java/Node alternatives
- **BYO-SMTP model** — unique: keep existing email infra, upgrade UI only
- **Argon2id password hashing** — best-in-class (undisclosed by competitors)
- **DANE support** — mentioned in architecture; rare among competitors

### Where TASMail FALLS SHORT
- **Calendar/Contacts/Tasks** — completely absent (every competitor has this)
- **Mobile apps** — no native apps, no ActiveSync (critical for Ghana)
- **AI features** — zero planned (Gmail/Outlook deeply AI-powered in 2026)
- **2FA/MFA** — not documented for v1 (serious security gap)
- **E2EE** — deferred to v2 (Thunderbird, Zoho, Gmail, Outlook have E2EE)
- **Collaboration** — no shared mailboxes, delegation, team features
- **Offline access** — not documented (important for Ghana connectivity)
- **Enterprise governance** — no archiving, eDiscovery, DLP, retention
- **Integration ecosystem** — no SSO/SAML, LDAP, add-on marketplace
- **Migration tools** — no import/migration path documented
- **Schedule send/Snooze/Recall** — standard in Gmail/Outlook, missing here

---

## Sources

### Thunderbird
- https://blog.thunderbird.net/2025/11/thunderbird-pro-november-2025-update/
- https://www.heise.de/en/news/What-Thunderbird-users-can-expect-in-2026-database-overhaul-and-iOS-app-11120751.html
- https://www.phoronix.com/news/Thunderbird-2026-Plans
- https://www.webpronews.com/thunderbirds-ambitious-2026-roadmap-bets-big-on-a-future-beyond-email/
- https://blog.thunderbird.net/2025/12/state-of-the-thunder-14-the-2026-mobile-roadmap/
- https://blog.thunderbird.net/2025/11/thunderbird-adds-native-microsoft-exchange-email-support/
- https://blog.thunderbird.net/2025/12/thunderbird-2025-review-building-stronger-for-the-future/
- https://clean.email/blog/email-clients/thunderbird-email-review
- https://dockshare.io/apps/thunderbird

### Zoho Mail
- https://www.zoho.com/mail/
- https://research.com/software/reviews/zoho-mail
- https://clean.email/blog/email-clients/zoho-review
- https://www.zoho.com/mail/zohomail-pricing.html
- https://www.zoho.com/workplace/plan-comparison.html
- https://www.topadvisor.com/products/zoho-mail/pricing
- https://www.neo.space/blog/review-of-zoho-mail-pricing-features-set-up-process
- https://www.zoho.com/mail/help/zia-artificial-intelligence.html
- https://www.zoho.com/mail/zia-with-openai.html

### Gmail / Google Workspace
- https://workspace.google.com/pricing
- https://www.emailtooltester.com/en/blog/google-workspace-pricing/
- https://blog.google/products-and-platforms/products/workspace/google-workspace-gemini-may-2025-updates/
- https://decrypt.co/353987/google-just-overhauled-gmail-gemini-3-ai-assistant
- https://fortune.com/2026/01/08/google-ai-inbox-gmail-gemini-3-integration-email-new-view-features/
- https://workspace.google.com/blog/product-announcements/gmail-and-calendar-client-side-encryption
- https://workspace.google.com/blog/identity-and-security/gmail-easy-end-to-end-encryption-all-businesses
- https://support.google.com/mail/answer/138350?hl=en
- https://www.devoteam.com/expert-view/what-is-new-in-google-workspace/

### Microsoft Outlook / 365
- https://www.microsoft.com/en-us/microsoft-365/blog/2025/12/04/advancing-microsoft-365-new-capabilities-and-pricing-update/
- https://support.microsoft.com/en-us/office/what-s-new-in-new-outlook-for-windows-c4c33813-1e9a-4304-8499-90fe7f164bd1
- https://www.trustedtechteam.com/blogs/news/microsoft-365-plan-updates-new-features-2026
- https://tminus365.com/microsoft-365-pricing-changes-in-2026-what-you-really-need-to-know/
- https://www.hbs.net/blog/major-microsoft-365-pricing-change-2026
- https://support.microsoft.com/en-us/office/feature-comparison-between-new-outlook-and-classic-outlook-de453583-1e76-48bf-975a-2e9cd2ee16dd
- https://support.microsoft.com/en-us/office/focused-inbox-for-outlook-f445ad7f-02f4-4294-a82e-71d8964e3978
- https://support.microsoft.com/en-us/office/set-up-outlook-to-use-s-mime-encryption-2e57e4bd-4cc2-4531-9a39-426e7c873e26
- https://learn.microsoft.com/en-us/microsoft-365/admin/email/about-shared-mailboxes

### General
- https://booboone.com/which-email-platform-wins-in-2026/
