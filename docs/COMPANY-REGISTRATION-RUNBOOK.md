# Company Registration Runbook (TMAIL-43)

**Version:** 1.0
**Date:** 2026-05-28
**Owner:** Founder / Platform Engineering
**Status:** Procedure documented; physical filing pending operator action at ORC
**Related:** `docs/research/company-registration-orc-2026.md` (raw research + sources),
`docs/BUSINESS-VALIDATION-GHANA.md`, `docs/PAYMENT-PROVIDER-MIGRATION.md` (downstream KYC),
`docs/HOSTING-PROCUREMENT.md` (sibling procurement runbook for TMAIL-18)

---

## 1. Purpose

TASMail must operate as a **registered Ghanaian limited company** before it can:

- Open a corporate bank account (mandatory for receiving Paystack / Mastercard MPGS settlements).
- Complete payment-provider KYC (Paystack, Mastercard MPGS, Cybersource) so the four
  providers wired in `handlers/billing.rs` can be activated against live payouts.
- Register as a **Data Controller** with the Data Protection Commission (per
  `BUSINESS-VALIDATION-GHANA.md` §6 and the deferred DPC ticket — required before
  taking BYOK custody of customer credentials in production).
- Pre-qualify for B2G tenders with NITA, MDA customers, or the public-sector channels
  in `BUSINESS-VALIDATION-GHANA.md` §8.3.
- Sign hosting / colocation contracts with Aveshost (Phase 1) and Smart Infraco (Phase 2)
  in the company's name rather than the founder's personal name.

The prior four auto-fix passes on TMAIL-43 produced no code changes because this is
a **physical-world filing**, not a software task. This runbook captures everything the
filing operator needs so the trip to ORC is a single visit, not three.

---

## 2. Decision Summary

| Choice | Selection | Rationale |
|---|---|---|
| Company structure | **Company Limited by Shares (Ltd)** under Companies Act 2019 (Act 992) | Required for equity raises, beneficial ownership clarity, and payment-provider KYC. Sole proprietorship cannot open the corporate accounts Paystack requires; Company Limited by Guarantee is for non-profits and would block billing. |
| Ownership | 100 % Ghanaian-owned at incorporation | Minimum stated capital GHS 500; preserves data-sovereignty marketing message in `BUSINESS-VALIDATION-GHANA.md`. Foreign investment can be added later via GIPC registration without re-incorporating. |
| Filing path | **e-Registrar portal** as primary, in-person fallback | Portal is faster when operational; in-person is the proven fallback when portal is down. |
| Speed tier | **Standard** filing (5–10 working days) | Prestige Service (48 h, +GHS 750) is reserved for time-critical scenarios — none of the downstream blockers in §1 are due inside two weeks. |
| Indicative budget | **GHS 500–550** (standard path, single applicant, default constitution) | See `docs/research/company-registration-orc-2026.md` §3 for line-item breakdown. |

---

## 3. Pre-Flight Checklist

Before filing — gather everything below. The single most common cause of multi-trip
filings is missing inputs in this list.

- [ ] **Three ranked company-name candidates** (e.g. "Tech at Scale Ltd", "TASMail Ghana Ltd",
      "Tech at Scale Mail Ltd"). Run a free name search on the ORC portal first.
- [ ] **Ghana Card** for every director, shareholder, and the company secretary.
- [ ] **TIN** for every director, shareholder, and the company secretary. Apply at the
      nearest GRA office or via the GRA eServices portal — issued in 24–48 hours, no charge.
- [ ] **Registered office address** with **GhanaPostGPS code** (download GhanaPostGPS app
      → stand at the office front door → record the code, e.g. `GA-123-4567`).
- [ ] **Stated capital** decision. Default: **GHS 5,000** (10× the legal minimum — gives
      headroom for share allotments without re-stamping later).
- [ ] **Share split** decision. Default: 100 % to the founder at incorporation; record
      future cap-table moves in board minutes, not in the constitution.
- [ ] **Objects of the company** — short list of permitted activities. Suggested set:
  1. Provision of email hosting, messaging, and collaboration services.
  2. Provision of cloud-based software and software-as-a-service.
  3. Provision of related professional, advisory, and consultancy services in
     information technology.
  4. All other lawful activities incidental or conducive to the above.
- [ ] **Company secretary** — must satisfy §211 of Act 992. If no internal qualifying
      person, retain an external ICSA / chartered-accountant secretary for the first
      filing (typical fee GHS 500–1,500/year — line-item separate from ORC fees).
- [ ] **Auditor consent letter** — must be ICAG-licensed. Engage and obtain consent before
      filing; the form asks for the auditor's name and licence number.
- [ ] **Beneficial Ownership list** — every individual with ≥10 % ownership or effective
      control. For a 100 %-founder structure: founder only.
- [ ] **Funding for fees** — GHS 600 in the operator's account (covers fees + ~GHS 50
      buffer for printing certified copies).

---

## 4. Filing Procedure — Online (e-Registrar Portal)

1. Go to <https://orc.gov.gh/> and click "e-Registrar" / online services.
2. Create an applicant account using Ghana Card verification.
3. **Name search** — submit ranked candidates. On approval, the name is reserved for
   2 months (pay the GHS 120 name-search-and-reservation fee).
4. Start the **Form 3** (Incorporation) flow. Fill:
   - Company name (the reserved one).
   - Type: Company Limited by Shares.
   - Registered office address + GhanaPostGPS code.
   - Stated capital, number of shares, par value.
   - Directors (min 2 under Act 992; if solo-founder, appoint a trusted second director
     — board member, advisor, or co-founder).
   - Company secretary (with §211 qualification proof).
   - Auditor (with ICAG licence number).
   - Objects (paste the list from §3).
5. Submit the **Beneficial Ownership Form** (required by AML Act 2020 Act 1044).
6. Submit **Form 4** (Notice of Commencement of Business) — can file alongside Form 3
   or within 28 days of incorporation; bundling now saves a second trip.
7. Pay all fees online (card or MoMo). Reference fee schedule:
   `docs/research/company-registration-orc-2026.md` §3.
8. Wait 5–10 working days. ORC reviews; queries (if any) arrive by email — respond
   within 14 days or the application lapses.
9. On approval, download from the portal:
   - Certificate of Incorporation.
   - Certified Form 3.
   - Certified Form 4.
   - Company TIN (issued by GRA via the integrated flow).
   - Beneficial Ownership Certificate.

## 5. Filing Procedure — In-Person (Fallback)

Use when the portal is down or face-to-face KYC is preferred.

1. Print Form 3, Form 4, and BO Form (free download from `orc.gov.gh/forms-fees`).
2. Complete by hand, sign every page, attach photocopies of Ghana Cards and TIN
   certificates.
3. Take to ORC Head Office (Adabraka, Accra) or nearest regional office (Kumasi,
   Takoradi, Tamale, Ho, Sunyani, Koforidua, Wa, Bolgatanga).
4. Pay at the in-house GCB Bank branch — keep the receipts; review will not start
   without the bank-stamped pay-in slip stapled to the forms.
5. Collect the certified outputs in person after 5–10 working days (the bundle is the
   same as §4.9).

## 6. Post-Incorporation Onboarding

Within **30 days** of receiving the Certificate of Incorporation:

- [ ] **GRA tax registration** — register for VAT, NHIL, GETFund, COVID-19 Health Recovery
      Levy, and PAYE (the latter only once the first employee is hired). The company
      TIN issued at incorporation is the registration handle.
- [ ] **SSNIT employer registration** — required before the first payroll cycle.
- [ ] **Open the corporate bank account** — take 2 certified copies of the Certificate
      of Incorporation, 2 of Form 3, 1 of Form 4, BO Certificate, board resolution
      naming the account signatories, and the directors' Ghana Cards. Recommended
      banks for fintech receipts: GCB, Stanbic, Absa (all accept Paystack settlements).
- [ ] **Update domain registrar (`techatscale.io`) WHOIS** to the company name and
      registered office.
- [ ] **Update GitHub org (`tasltd`) billing entity** to the company.
- [ ] **Apply for Data Protection Commission (DPC) Data Controller registration** —
      separately tracked in the deferred DPC ticket (cannot start without the
      Certificate of Incorporation from this runbook).
- [ ] **Start payment-provider KYC**:
  - **Paystack Ghana** — submit Cert of Inc, Form 3, BO Cert, bank statement, founder
    Ghana Card. See `docs/PAYMENT-PROVIDER-MIGRATION.md` for the credential-rotation
    runbook once Paystack approves and issues live API keys.
  - **Mastercard MPGS** — typically routed through a Ghanaian acquiring bank; start
    after the corporate bank account is open.
  - **Cybersource** — for invoice-based enterprise clients only; defer until first
    enterprise quote signs.
- [ ] **Sign Aveshost hosting contract** in the company name (see
      `docs/HOSTING-PROCUREMENT.md` §4).

## 7. Indicative Timeline

| Day | Action |
|---|---|
| 0 | Pre-flight checklist complete, name candidates ranked, all TINs in hand |
| 1 | Submit online filing (name search + Form 3 + Form 4 + BO + payment) |
| 6–10 | ORC review window |
| 10 | Receive certificates; print 3 certified copies of each via portal |
| 11 | Open corporate bank account (single morning if all docs in hand) |
| 12 | Submit Paystack KYC, GRA tax registration, SSNIT employer registration |
| 14 | DPC application submitted (separate ticket) |
| 28 | Form 4 absolute filing deadline (already filed in §4.6) |
| 30 | All §6 post-incorporation items closed |

## 8. Failure Modes & Recovery

| Failure | Recovery |
|---|---|
| All three name candidates rejected | Submit three more (no extra fee within the same application window if the portal flow supports it; otherwise pay another GHS 120). Common cause: matches a struck-off entity not visible to public search. |
| Secretary appointment rejected on §211 grounds | Replace with an ICSA member or licensed lawyer; resubmit Form 3 amendment within the 14-day query window. |
| Stamp-duty understated | Pay top-up via the same payment voucher reference; do not start a new filing. |
| Beneficial ownership query | Provide full chain of control (indirect shareholders too) within 14 days. |
| e-Registrar portal unavailable | Switch to in-person path (§5) without restarting; the name reservation carries over. |
| Bank rejects Form 4 as "not yet commenced" | Show the certified Form 4 issued at incorporation; explain Act 992 allows simultaneous filing. Escalate to bank branch manager if needed. |

## 9. Cost Summary

Standard path (no advisory):

| Line item | GHS |
|---|---|
| Name search & reservation | 120 |
| Filing fee (Company Limited by Shares) | 230 |
| Stamp duty (on GHS 5,000 stated capital) | 25 |
| Beneficial Ownership filing | 50 |
| Certified copies (Cert of Inc + Form 3 + Form 4 × 2 each) | 300 |
| **Sub-total ORC** | **~725** |
| External company secretary (annual retainer, optional) | 500–1,500 |
| GRA TIN issuance | 0 |
| **Total filing-day spend** | **~725 GHS** (≈ USD 50 at 2026-05 rate) |

Prestige Service (48 h) adds GHS 750 — use only if a downstream contract has a
hard deadline inside two weeks.

## 10. Sources

See `docs/research/company-registration-orc-2026.md` §10 for the full source list with
URLs (ORC official forms-and-fees PDF, Companies Act 2019 full text, Trade.gov, Firmus
Advisory, Lexology, Mondaq, DennislawGH, JS Morlu, PISTIS, Pulse Ghana).
