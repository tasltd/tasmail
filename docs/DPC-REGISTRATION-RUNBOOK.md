# DPC Data Controller Registration Runbook (TMAIL-44)

**Version:** 1.0
**Date:** 2026-05-28
**Owner:** Founder / Compliance Lead
**Status:** Procedure documented; portal filing pending company incorporation (TMAIL-43)
**Related:** `docs/research/dpc-registration-2026.md` (raw research + sources),
`docs/COMPANY-REGISTRATION-RUNBOOK.md` (incorporation must complete first),
`docs/BUSINESS-VALIDATION-GHANA.md` §4 (DPC as competitive moat),
`docs/GAP-ANALYSIS.md` GAP-B-047 (Compliance & Privacy)

---

## 1. Purpose

TASMail must be registered with the **Ghana Data Protection Commission (DPC)** as
a Data Controller under **Section 27(1) of the Data Protection Act, 2012 (Act 843)**
before it can:

- Take BYOK custody of customer IMAP/SMTP credentials in production at scale
  without exposure to Section 56 sanctions (up to 250 penalty units ≈ GHS 3,000
  and/or 2 years imprisonment).
- Display the **"DPC-Registered Data Controller"** badge that
  `BUSINESS-VALIDATION-GHANA.md` §4 identifies as the regulatory moat — none of
  Google Workspace, Microsoft 365, Zoho Mail, or HostAfrica are DPC-registered
  in Ghana.
- Bid for NGO and public-sector tenders where DPC compliance is a prequalification
  criterion (referenced in `BUSINESS-VALIDATION-GHANA.md` §6 and §8.3).
- Survive the **2026 enforcement drive** announced by the DPC in January 2026
  (B&FT, 28 Jan 2026), which targets unregistered processors of personal data.

The three prior auto-fix passes on TMAIL-44 produced no code changes because
this is a **regulatory filing**, not a software task. This runbook captures
everything the filing operator needs so the application is filed once, correctly.

---

## 2. Decision Summary

| Choice | Selection | Rationale |
|---|---|---|
| Controller vs Processor classification | **Hybrid: Controller for account/billing metadata, Processor for customer email content** | TASMail signs up users, holds Ghana Card / TIN / billing data → Controller. Customer email content lives on the customer's own IMAP server (BYOK); TASMail only proxies → Processor. Confirm interpretation with `compliance@dataprotection.org.gh` *before* submission. |
| Filing path | **Online portal at https://app.dataprotection.org.gh/** | Portal returns the sector-classified fee quote and a payment voucher. Walk-in at DPC Achimota office is the documented fallback. |
| Sector classification | **Information & Communication Technology (ICT) — Cloud / Email Service Provider** | Closest match in the DPC sector taxonomy; affects the fee tier. |
| Size tier | **Small** (initial filing) | Until headcount > 30 or annual turnover > the SME threshold, file as Small. Upgrade tier at first renewal. |
| Data Protection Supervisor (DPS) | **Appointed at filing** | Section 58 only mandates DPS for medium/large controllers, but appointing one as a Small filer (a) hardens the moat narrative, (b) avoids a forced amendment when TASMail crosses into Medium. Default: the founder, with one named alternate from the engineering team. |
| Validity period assumed | **2 years** | Official DPC guidance and DLA Piper both cite 2 years. Diary the 12-month internal compliance review regardless. |
| Indicative budget | **GHS 1,500–4,000 filing + GHS 0–18,000/yr DPS-as-a-Service (optional)** | Fee is portal-quoted; range derived from sector + size class. DPSaaS only if the founder cannot serve. |

---

## 3. Pre-Flight Checklist

Before opening the portal — gather everything below. Section 4 will be smooth
only if every line item is ticked.

- [ ] **Certificate of Incorporation** from ORC (output of TMAIL-43 runbook).
      The Cert number is the registration handle on the DPC portal.
- [ ] **Form 3** (incorporation form) certified copy.
- [ ] **Company TIN** certificate (issued at incorporation).
- [ ] **Registered office address** with GhanaPostGPS code (matches what was
      filed at ORC — mismatch triggers a query and a 14-day delay).
- [ ] **Designated DPS** — name, Ghana Card number, role title, email, phone.
      Default: Founder, with one engineering team-member as named alternate.
- [ ] **Privacy Policy** published at `/privacy` on `mail.techatscale.io`.
      The DPC will fetch the URL and reject the application if the policy
      does not cover: identity of controller, categories of data, lawful
      basis, retention, data subject rights, complaints route to the DPC.
      Verify rendering against the *Guidelines to Demonstrate Data Protection
      Compliance* checklist (see `docs/research/dpc-registration-2026.md` §6).
- [ ] **Records of Processing Activities (ROPA)** — short document covering:
  - Categories of data subjects (signup users, billing contacts, recipients of
    sent mail, calendar attendees, contact-book entries).
  - Categories of data per subject type (auth metadata, billing PII, email
    content held on customer-side IMAP, calendar event metadata, audit logs).
  - Retention defaults per category (auth logs 90 days, audit logs 1 year,
    deleted-account scrub T+30 days, billing records 7 years per tax law).
  - Per-tenant overrides — TASMail's enterprise tier allows custom retention,
    so the ROPA must say "tenant-configurable, defaults as above".
- [ ] **DPIA** for the two high-risk processing activities:
  - **AI subsystem** (Ollama / embeddings / phishing scanner — see
    `docs/assessments/ai-subsystem-2026-05.md`). DPIA must cover purpose,
    data flow, BYOK boundary, opt-out path, and risk-mitigation TOMs.
  - **eDiscovery compliance feature** (migration `069`). DPIA must cover
    legal hold scope, admin-only access, audit log of every export.
- [ ] **Technical & Organisational Measures (TOMs)** summary:
  - TLS-only transport (Apache + Let's Encrypt; HSTS).
  - Argon2id password hashing.
  - AES-256-GCM at-rest encryption for credentials (`services/encryption.rs`).
  - Row-Level Security in PostgreSQL enforced via `auth_middleware`.
  - Audit log retention + tamper-evident hash chain.
  - Per-account brute-force lockout (TMAIL-273, just shipped 4bcf2f6d).
  - Backup + DR procedure per `docs/BACKUP-RESTORE.md`.
- [ ] **Cross-border transfer list** — every IMAP/SMTP preset in
      `frontend/src/components/onboarding/OnboardingWizard.tsx` mapped to its
      provider's hosting jurisdiction:

  | Preset | Provider | Hosting jurisdiction |
  |---|---|---|
  | Gmail / Google Workspace | Google LLC | USA + EU |
  | Outlook / Microsoft 365 | Microsoft Corp. | USA + EU |
  | Yahoo Mail | Yahoo Inc. | USA |
  | Zoho Mail | Zoho Corp. | India + USA |
  | FastMail | FastMail Pty | Australia + USA |
  | iCloud Mail | Apple Inc. | USA |
  | ProtonMail Bridge | Proton AG | Switzerland |
  | TASMail self-hosted (optional Postfix/Dovecot) | Aveshost / Smart Infraco | **Ghana** |

  Section 18 of Act 843 governs cross-border transfers — file this list with
  the DPC at registration so it never becomes a post-filing query.
- [ ] **Funding** — GHS 4,000 in operator's account (covers the upper-bound
      fee + screenshot evidence of the quote attached to the PM ticket).
- [ ] **Email-monitoring rota** — DPC queries arrive at the email on the
      application; the operator MUST monitor it daily for the first 30 days
      after submission (query deadline is 14 days; missing one re-starts the
      application).

---

## 4. Filing Procedure — Online (Primary)

1. Go to <https://app.dataprotection.org.gh/> and create an applicant account
   using the company TIN as the primary identifier.
2. Verify the applicant email — DPC sends an OTP to the email on file. Use a
   shared compliance@techatscale.io alias rather than a personal mailbox so
   the account survives staff changes.
3. **Pre-check with compliance team** — open a ticket via
   `compliance@dataprotection.org.gh` asking the DPC to confirm TASMail's
   classification as **Controller for metadata + Processor for content under
   the BYOK model**. Attach the architecture one-pager from
   `docs/ARCHITECTURE.md` §1. Wait for the written reply (typically 3–5
   working days) — it goes into the application as evidence.
4. **Start the Registration form** in the portal. Fill, section-by-section:
   - **Identity** — company name, Cert of Inc number, TIN, registered office
     address + GhanaPostGPS code, contact email/phone.
   - **Sector & size** — ICT / Cloud / Email Service Provider, Small.
   - **DPS** — name, Ghana Card, role, contact details, alternate.
   - **Processing activities** — paste the ROPA summary (§3); upload the
     full ROPA as a PDF attachment.
   - **Categories of data subjects** — Customers (signup users), Recipients
     (external), Billing contacts, Calendar attendees, Contact book entries.
   - **Categories of personal data** — Identification (name, email, Ghana
     Card for KYC), Authentication (hashed password, TOTP secret, FIDO2 keys),
     Billing (TIN, billing address, payment provider tokens), Content
     (email body — held on customer's IMAP, **proxied not stored**), Metadata
     (IP, user agent, audit log entries).
   - **Special categories** — confirm "Yes" (email content can contain
     health / political / religious data the user chose to write).
   - **Purposes** — Webmail proxying, calendar, billing, audit, anti-abuse,
     optional AI assistance (opt-in).
   - **Data recipients** — Customer's chosen IMAP/SMTP server; Paystack /
     Mastercard MPGS / Cybersource (for payment processing); Hetzner /
     Smart Infraco (hosting); Ollama instance (only if user opts in to AI).
   - **Cross-border transfers** — paste the preset → jurisdiction table
     from §3. Confirm the legal basis for each (contract performance + user
     consent at onboarding).
   - **Security safeguards** — paste the TOMs summary from §3; upload the
     full TOMs document as a PDF attachment.
   - **Compliance documents** — attach Privacy Policy URL, ROPA PDF,
     DPIA PDFs (AI subsystem + eDiscovery), TOMs PDF, the
     `compliance@dataprotection.org.gh` reply on classification.
5. Portal computes the **sector-classified fee** and issues a payment
   voucher. **Screenshot the quote** and the voucher; attach both to the
   TMAIL-44 PM comment before paying.
6. Pay via the portal (card or MoMo). Keep the receipt PDF.
7. Submit. Portal returns an application reference number — record it on
   the TMAIL-44 PM comment.
8. **Daily monitoring** for 30 days — respond to any query within 14 days
   or the application lapses (re-application fee applies).
9. On approval, download from the portal:
   - **Certificate of Registration** (PDF, watermarked).
   - Registration number (format `DPC/REG/yyyy/xxxxx`).
   - Validity dates.

## 5. Filing Procedure — In-Person (Fallback)

Use when the portal is down or the §4.3 pre-check email goes unanswered for
> 10 working days.

1. Print the Registration Form (download from
   <https://dataprotection.org.gh/registration/>).
2. Complete by hand, sign every page; attach printed copies of every
   supporting document listed in §4.4.
3. Take to the DPC office near GIMPA, Achimota, Accra.
4. Pay at the in-house cash desk (cheque or MoMo voucher). Keep the
   stamped receipt — review will not begin without it.
5. Collect the certificate in person after 4–6 weeks (in-person path is
   slower than the portal).

## 6. Post-Registration Onboarding

Within **7 days** of receiving the Certificate of Registration:

- [ ] **Update Privacy Policy** at `/privacy` to include the DPC registration
      number and validity period. File path:
      `frontend/src/components/landing/PrivacyPolicy.tsx`.
- [ ] **Add the DPC badge** to:
  - Landing page footer (`frontend/src/components/landing/LandingPage.tsx`)
  - Pricing page footer (`frontend/src/components/landing/PricingPage.tsx`)
  - Settings → About panel (`frontend/src/components/settings/AboutPanel.tsx`)
- [ ] **Update meta tags** on `frontend/index.html` to include
      "DPC-registered Ghana data controller" in the meta description.
- [ ] **Publish the registration certificate** as a downloadable PDF at
      `/compliance/dpc-certificate.pdf` (route served by Vite static).
- [ ] **Update `BUSINESS-VALIDATION-GHANA.md` §4** — replace
      "DPC registration is a competitive moat" forward-looking statement
      with the registration number and date.
- [ ] **Brief sales channels** — NGO partners, B2G channels, agency resellers
      flagged in `BUSINESS-VALIDATION-GHANA.md` §8.2 (the cert unlocks them).
- [ ] **Diary the renewal calendar**:
  - T+12 months — internal Gap Analysis review.
  - T+21 months — start preparing the Compliance Report + Gap Analysis
    Report needed for renewal.
  - T+24 months minus 3 months — open renewal window; submit early.
  - T+24 months — registration expires; renewal must be in or grace
    ends 7 days later.

## 7. Indicative Timeline

| Day | Action |
|---|---|
| 0 | Pre-flight checklist complete (§3) |
| 1 | Send classification pre-check email to `compliance@dataprotection.org.gh` |
| 4–6 | DPC reply with classification confirmation |
| 7 | Open application on portal, complete sections, attach all documents |
| 7 | Screenshot fee quote → attach to TMAIL-44 PM comment → pay |
| 7 | Submit application; record reference number |
| 7–37 | Monitor application email daily; respond to any DPC query within 14 days |
| 30–45 | Approval window (varies by queue depth) |
| 45 | Receive Certificate of Registration; execute §6 onboarding within 7 days |

## 8. Failure Modes & Recovery

| Failure | Recovery |
|---|---|
| DPC rejects Controller-only classification on the BYOK model | Re-file as joint Controller + Processor (the §3 default position); the pre-check email in §4.3 should prevent this. |
| Sector dropdown has no "Email Service Provider" entry | Pick "Cloud Services" or "ICT — Other"; explain in the application notes and attach the architecture one-pager. |
| Fee quote exceeds GHS 4,000 budget | Request a Small-tier discretion via `compliance@dataprotection.org.gh`; attach headcount + turnover evidence. If denied, escalate to PM ticket and tap the §3 contingency budget. |
| Cross-border transfer list challenged on Section 18 grounds | Switch the affected user's onboarding to require explicit, granular consent at provider-add time (UI change in `OnboardingWizard.tsx`); resubmit with the consent-screen screenshot as evidence. |
| Privacy Policy URL rejected as non-compliant | Use the DPC's *Guidelines to Demonstrate Data Protection Compliance* (July 2025) as a checklist; rewrite and resubmit within 14 days. |
| DPS appointment rejected (e.g. founder also CEO conflict) | Replace with an external DPSaaS provider; engagement letter + Ghana Card of the assigned officer must be attached. |
| Query missed (14-day deadline lapsed) | Application lapses; re-file from scratch and pay the fee again. Mitigation: §3 email-monitoring rota. |
| 2026 enforcement notice received before registration completes | Reply citing the in-flight application reference number; the DPC's published practice is to pause sanctions for applications already under review. |

## 9. Cost Summary

| Line item | GHS |
|---|---|
| DPC registration fee (Small ICT — quoted on portal, indicative) | 1,500–4,000 |
| Privacy Policy legal review (one-off, optional) | 1,500–3,000 |
| DPSaaS engagement (optional — annual retainer) | 0–18,000 |
| Renewal Compliance Report drafting (T+21 months) | 0–5,000 (internal vs external) |
| Sanction avoided | up to GHS 3,000 (Section 56) per infraction |

Source for the fee range: sector + size table on the DPC portal (post-login);
the public-facing fee page is portal-gated. Operator MUST screenshot the
exact quote returned at §4.5 and attach it to the PM ticket.

## 10. Sources

See `docs/research/dpc-registration-2026.md` §10 for the full source list with
URLs (DPC official registration page, *Guidelines to Demonstrate Data Protection
Compliance* PDF, DLA Piper, Lexology, DataGuidance, InfoGov, B&FT 2026
enforcement announcement, trade.gov market intelligence).
