# Ghana DPC Data Controller Registration — Raw Research

**Date captured:** 2026-05-28
**Captured for:** TMAIL-44 (Register as Data Controller with DPC under Act 843)
**Status:** Research complete; runbook produced at `docs/DPC-REGISTRATION-RUNBOOK.md`

This file holds the raw findings, quotes, and source URLs gathered for the DPC
registration runbook. Keep it side-by-side with the runbook so the next operator
can verify any fact against its original source. Do not rewrite this file when
the runbook changes — append a new dated section instead.

---

## 1. Legal Basis

- **Statute:** Data Protection Act, 2012 (Act 843)
- **Registration trigger:** Section 27(1) — every data controller processing
  personal data in Ghana must apply in writing to the Data Protection Commission
  for registration. Foreign companies processing data of Ghanaian residents are
  caught too.
- **Sanction for non-registration:** Section 56 — fine of **up to 250 penalty
  units** (1 penalty unit = GHS 12 under the Fines (Penalty Units) Act 2000
  (Act 572), so ≈ **GHS 3,000**), or imprisonment up to 2 years, or both.
- **DPS appointment:** Section 58 — medium and large data controllers MUST
  appoint a Data Protection Supervisor. Small controllers are exempt from the
  hard requirement but the DPC strongly encourages it.

## 2. Enforcement Posture (2026)

- 2026-01 announcement: the DPC has flagged 2026 as the start of "full-scale
  enforcement" against unregistered processors. Public + private institutions
  in scope. Source: B&FT, 28 Jan 2026.
- Registration is therefore not a future-quarter problem — it is the
  competitive-moat opening **and** the audit-risk floor at the same time.

## 3. Registration Inputs

### Mandatory data fields (per DPC official registration page)

- Business name + registered office address
- Description of personal data to be processed
- Categories of data subjects whose data will be collected
- Purposes of processing
- Data recipients / disclosure plans
- International transfer destinations (this is the critical field for TASMail
  because BYOK customers may use IMAP/SMTP servers outside Ghana)
- Security safeguards implemented (TLS, at-rest encryption, RLS, audit logs)
- Sensitive (special) data confirmation — TASMail handles email content,
  which is treated as special-category data in practice

### Supporting documents

- Certificate of Incorporation (from ORC — comes out of `COMPANY-REGISTRATION-RUNBOOK.md`)
- Business operating licence (where applicable — software/IT services typically
  satisfied by Cert of Inc + GIPC registration if foreign equity)
- TIN certificate (company TIN, issued at incorporation)
- Proof of payment of the prescribed sector-based fee

## 4. Fee Schedule

The DPC website **does not publish a fee table publicly**. Fees are determined
by sector classification and organisation size and are quoted on the portal
after the applicant selects their classification.

- Sectors are classified via the DPC's internal taxonomy (mirrors the Ghana
  Standard Industrial Classification — finance, telecoms, ICT, health, education,
  NGO, public sector, etc.).
- Size tiers are typically small / medium / large, measured by headcount,
  turnover, and volume of data subjects processed.
- Confirm exact fee for TASMail by:
  1. Logging into the portal at https://app.dataprotection.org.gh/
  2. Selecting sector = **Information & Communication Technology (ICT) — Cloud /
     Email Service Provider**, size = **Small** (until headcount > 30 or
     turnover > the SME threshold)
  3. The portal returns the fee + payment voucher
- Operator MUST screenshot the fee quote and attach it to the TMAIL-44 PM
  comment when filed.

## 5. Validity & Renewal

- **Validity period:** Sources diverge — DLA Piper and InfoGov both state
  **two (2) years**; one secondary source (Lexology) cites one (1) year.
  Treat as **2 years** (the directly-cited statutory interpretation), but
  diary the first-year mark for an internal compliance check.
- **Renewal window:** 3 months before expiration → 7 days after.
- **Renewal requires:** Compliance Report covering the past period AND, per
  DPC practice, a Gap Analysis Report showing remediation steps for any
  identified gaps.

## 6. Pre-Registration Compliance Workstream

The DPC's *Guidelines to Demonstrate Data Protection Compliance* (July 2025
revision) makes clear that registration is the *output* of a compliance
programme, not a one-form submission. Before filing, TASMail needs:

- Designated DPS (named person + alternate)
- Privacy Policy (published — already exists at `/privacy` on the landing page;
  needs DPC-compliance review)
- Records of Processing Activities (ROPA) — bridges TASMail's
  multi-tenant data model: per-tenant data subject categories, per-tenant
  retention defaults, per-tenant SMTP/IMAP transfer destinations
- DPIAs for high-risk processing — for TASMail, the AI subsystem
  (TMAIL-249 assessment) and the eDiscovery compliance feature (migration 069)
  both qualify
- Technical + organisational measures (TOMs) documentation — TLS-only,
  Argon2id password hashing, AES-256-GCM at-rest for credentials, RLS, audit
  log retention policy

## 7. Online Portal & Contacts

- **Portal:** https://app.dataprotection.org.gh/
- **Phone:** +233 256 301 533 (general), 0256301533 (registration line)
- **Email:** info@dataprotection.org.gh (general), support@dataprotection.org.gh
  (registration support), compliance@dataprotection.org.gh (compliance queries)
- **WhatsApp:** 0506177975
- **Office:** Data Protection Commission, near GIMPA, Achimota, Accra
- **Ministry parent:** Ministry of Communication, Digital Technology and
  Innovations (DPC sits under MoCDTI per the 2025 machinery-of-government
  reshuffle).

## 8. Why This Is a Moat (Marketing Use)

Existing references in the project already lean on DPC registration as a
differentiator:

- `BUSINESS-VALIDATION-GHANA.md` §4 — "DPC registration is a competitive moat
  — most international SaaS providers are not DPC-registered."
- `BUSINESS-VALIDATION-GHANA.md` §5 competitor table — Google Workspace,
  Microsoft 365, Zoho all flagged as "no DPC registration".
- `BUSINESS-VALIDATION-GHANA.md` §8 critical success factors — "DPC
  registration from Day 1 — this is the moat".
- `GAP-ANALYSIS.md` GAP-B-014 (Audit Logging) and GAP-B-047 (Compliance &
  Privacy) both flag DPC registration as the P0 trigger.

Marketing payload, once the certificate lands:
- Display the DPC certificate number on the landing page (`/`) footer
- Add a "Ghana DPC Registered Data Controller — Certificate #XXX" badge on
  the pricing page (`/pricing`)
- Update the public meta description on `frontend/index.html` to include
  "DPC-registered Ghana data controller"

## 9. Outstanding Unknowns

- Exact filing fee in GHS (sector + size dependent, only quoted post-portal-login)
- Whether the DPC accepts the BYOK model cleanly — TASMail must position itself
  as the **data processor** for the customer's email content and the **data
  controller** only for account metadata (signup details, billing). Confirm
  with DPC compliance email before filing.
- Whether the cross-border transfer reporting in Section 18 applies when the
  customer's IMAP/SMTP server is hosted outside Ghana (likely yes — list each
  preset provider's hosting country in the registration form).

## 10. Sources

### Official

- [Data Protection Commission — Registration page](https://dataprotection.org.gh/registration/)
- [Data Protection Commission — Data Protection Act page](https://www.dataprotection.org.gh/data-protection-act)
- [Data Protection Commission — Fees & Charges (portal-gated)](https://www.dataprotection.org.gh/data-protection/fees-charges)
- [DPC — Guidelines to Demonstrate Data Protection Compliance (July 2025 PDF)](https://dataprotection.org.gh/wp-content/uploads/2025/07/GUIDELINES-TO-DEMONSTRATE-DATA-PROTECTION-COMPLIANCE.pdf)
- [DPC Online Registration Portal](https://app.dataprotection.org.gh/)
- [Ministry of Communication, Digital Technology and Innovations — DPC page](https://moc.gov.gh/dpc/)
- [Ghana.GOV — Data Protection Commission MDA page](https://www.ghana.gov.gh/mdas/e1eca9de96/)

### Legal commentary / law firm analyses

- [O-Laryea Law — An Overview of Data Protection Law in Ghana](https://olaryealaw.com/an-overview-of-data-protection-law-in-ghana/)
- [Lexology — Understanding Ghana's Data Protection Laws: What Businesses Need to Do](https://www.lexology.com/library/detail.aspx?g=98999f8e-d0c4-480d-b345-d9090b953c31)
- [DLA Piper — Ghana data protection enforcement summary](https://dlapiperdataprotection.com/?c=GH&t=enforcement)
- [DLA Piper — Ghana data protection registration summary](https://www.dlapiperdataprotection.com/index.html?t=registration&c=GH)
- [DataGuidance — Ghana Data Protection Overview](https://www.dataguidance.com/notes/ghana-data-protection-overview)
- [Aosphere — Ghana data privacy obligations](https://www.aosphere.com/products/data-privacy/rulefinder-data-privacy/data-privacy-ghana/)
- [Regulations.AI — Ghana Data Protection Act 843 text](https://regulations.ai/regulations/RAI-GH-NA-ACT8430-2012)
- [InfoGov Solutions — DPC Registration walkthrough](https://infogovs.com/registration/)
- [InfoGov Solutions — Data Protection services overview](https://www.infogovgh.com/data-protection/)
- [InfoGov Solutions — For Organisations (DPS as a Service)](https://infogovs.com/for-organisations/)

### Press / market signal

- [B&FT — DPC to sanction non-compliant institutions (28 Jan 2026)](https://thebftonline.com/2026/01/28/dpc-to-sanction-non-compliant-institutions/)
- [GhanaWeb — DPC pledges action against Act 843 breaches](https://www.ghanaweb.com/GhanaHomePage/business/Data-Protection-Commission-pledges-action-against-Act-843-breaches-2019640)
- [trade.gov — Ghana Information Technologies Data Protection market intelligence](https://www.trade.gov/market-intelligence/ghana-information-technologies-data-protection)
- [ITLawCo — Ghana's Data Protection Act 2012 (Act 843)](https://itlawco.com/focus-areas/data-protection-and-privacy/ghanas-data-protection-act-2012-act-843/) (already cited in `BUSINESS-VALIDATION-GHANA.md` §10)

### Adjacent service providers (for DPS-as-a-Service quotes, NOT endorsements)

- [Desk Multi Tech — Data Protection & Business Continuity](https://dmtctech.com/services/data-protection-business-continuity/)
