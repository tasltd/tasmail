# Beta Customer Recruitment Runbook (TMAIL-45)

**Version:** 1.0
**Date:** 2026-05-28
**Owner:** Founder (Dominic Dodzi)
**Status:** Procedure documented; outreach pending operator action
**Related:** `docs/BUSINESS-VALIDATION-GHANA.md` §8.1 (Phase 1 GTM — "10 beta customers from
personal network"), `docs/IP-WARMUP-RUNBOOK.md` (beta traffic feeds the warm-up trickle),
`docs/DNS-MX-ONBOARDING.md` (Full Hosted onboarding), `docs/PRD.md` and
`BUSINESS-VALIDATION-GHANA.md` §3 (BYOK positioning + pricing), `docs/PROJECT-MANAGEMENT-PLAN.md`
(project milestones)

---

## 1. Purpose

TASMail's Phase-1 go-to-market plan calls for **10 beta customers recruited from the
founder's personal and professional network** before public launch. The beta cohort
serves four concrete functions:

1. **Validate the BYOK product end-to-end** — onboarding wizard, IMAP/SMTP attach,
   message read/send/compose, mobile companion — on real user mailboxes, not test fixtures.
2. **Generate the initial mail-sending trickle** the IP-warmup runbook needs (week-1
   target is ~50 messages/day; 10 mailboxes × 5 outbound/day clears that floor).
3. **Surface real-world bugs and UX gaps** that the unit/integration/E2E suites cannot
   reach — provider quirks (Gmail App Passwords, Outlook OAuth, ProtonMail Bridge ports),
   client/device variety, real attachment sizes.
4. **Produce 2–3 written testimonials and 1 anchor reference customer** for the public
   launch — see `BUSINESS-VALIDATION-GHANA.md` §10 critical success factor #6.

The three prior auto-fix passes on TMAIL-45 produced no code changes because this is a
**human outreach task**, not a software task. This runbook captures every input the
outreach operator needs so the recruitment cycle runs in a single sprint, not three.

---

## 2. Decision Summary

| Choice | Selection | Rationale |
|---|---|---|
| Cohort size | **10 customers** | Matches PM plan; small enough to onboard hand-held in 1–2 weeks, large enough to surface ~80 % of provider/device quirks. |
| Product mix | **3 BYO-SMTP + 7 Full Hosted** | BYO-SMTP exercises the BYOK wizard against external providers (Gmail/Outlook/Zoho); Full Hosted exercises the optional Postfix/Dovecot path for operators who want it. Mix is weighted to the primary BYOK story but covers the secondary self-host path. |
| Pricing for beta | **Free for 3 months** | Removes payment-provider gating during beta (Paystack KYC may not be complete yet — see `PAYMENT-PROVIDER-MIGRATION.md`). Post-beta: 20 % founder discount for 12 months as a retention incentive. |
| Recruitment channels | **Direct outreach only** (no public form, no ads) | "Personal/professional network" per TMAIL-45 spec. Founder + 6 collaborators each contribute outreach via warm intros. |
| Selection criteria | **Mixed but Ghana-weighted** | At least 7 of 10 must be Ghana-based (matches BYO target market and data-sovereignty narrative). Up to 3 can be diaspora / regional for variety. |
| Outreach platform | **WhatsApp + Email + LinkedIn DM** (in that order) | Ghana B2B reality: WhatsApp is the working channel; email is the receipt; LinkedIn is the formal handoff. |
| Beta agreement form | **Lightweight one-pager**, not a SaaS MSA | Reduces friction. See §7 for template. Real MSA is reserved for post-beta paying contracts. |
| Tracker | **`docs/beta/pipeline.csv` (git-tracked, no PII in the open repo)** | Use codes (e.g. `BC-01`) in the CSV; map codes → identities in a private `.gitignore`'d file. See §6.3. |

---

## 3. Pre-Flight Checklist

Before sending the first outreach message — gather everything below. The single most
common cause of an outreach campaign stalling is a half-built product behind it.

### 3.1 Product readiness

- [ ] **Signup + onboarding wizard live at `https://mail.techatscale.io/signup`** —
      verify a fresh signup + IMAP attach with a test Gmail account works end-to-end.
- [ ] **The 11 IMAP presets** in `GET /api/imap-configs/presets` all return correct
      hostnames/ports (Gmail, Outlook, Yahoo, Zoho, FastMail, iCloud, ProtonMail Bridge,
      etc.) — these are what beta users will click.
- [ ] **`POST /api/imap-configs/test`** passes against a real Gmail App-Password account
      and a real Outlook account before outreach starts.
- [ ] **Full Hosted setup** — at least one Postfix/Dovecot instance staged per
      `docs/SELF-HOST-MAIL-SERVERS.md` for the 7 Full Hosted slots, OR a holding answer
      ("we'll provision your hosted domain in week 2 of the beta") agreed with operator.
- [ ] **Mobile companion** — Flutter app installable APK available for sideload (see
      `MOBILE-PLATFORM-DECISION.md`). Beta users on Android get a build link; iOS users
      defer to the web PWA.
- [ ] **In-app feedback button** — Composer-adjacent "Send beta feedback" link that
      opens a prefilled email to `beta@techatscale.io` (or an in-app form posting to
      a feedback endpoint). If not built, log a follow-up ticket and use the email path
      via `mailto:` for the beta window.
- [ ] **Sentry / error tracking** wired so beta failures surface to the founder's
      Sentry org `tasltd` without the user needing to file a bug.
- [ ] **Backup runbook ready** — beta customers will trust the service more if they
      know `docs/BACKUP-RESTORE.md` exists and snapshots run nightly.

### 3.2 Legal / commercial readiness

- [ ] **Beta agreement one-pager finalised** (template in §7) — founder-signed copy
      ready to share as a PDF.
- [ ] **DPC application at least submitted** — even if not yet approved, the
      "registration pending under Act 843" line in the agreement is honest and
      protective. See `DPC-REGISTRATION-RUNBOOK.md`.
- [ ] **Privacy notice published at `/legal/privacy`** — beta users sign the agreement
      after reading the notice; both must exist before the first signature.
- [ ] **Founder contactable on at least two channels** — WhatsApp Business + work email,
      both monitored, both with auto-reply describing beta SLA (next-business-day
      response, not 24×7).

### 3.3 Outreach assets

- [ ] **Outreach scripts written** (§5).
- [ ] **Pipeline tracker created** at `docs/beta/pipeline.csv` (§6).
- [ ] **Identity map** at `docs/beta/identities.private.md` (gitignored — see §6.3).
- [ ] **Calendly / scheduling link** for the 20-minute kick-off call (template in §8).
- [ ] **One-pager PDF** (or Notion page) with screenshots of the SPA, sample pricing
      post-beta, founder bio.

---

## 4. Target List — Personas and Ranked Buckets

Rank candidates against the buckets below. Aim to fill ~14 names across the buckets,
expecting a ~70 % accept rate to land on 10.

### 4.1 Bucket A — Form B (Full Hosted, 7 slots)

These are organisations that need a custom-domain mailbox, today are paying for or
struggling with Google Workspace / Zoho / Roundcube. **Ghana-based, ≤ 25 mailboxes.**

| Sub-bucket | Examples | Why they fit |
|---|---|---|
| **A1. UG-adjacent labs / departments** | Research groups at UG Legon, KNUST, GIMPA — anyone the founder knows from coursework or supervisor relationships | Lowest friction: shared institutional context, founder credibility, easy in-person follow-up. Cap at 2 to avoid an all-academic cohort. |
| **A2. Small Ghana SMEs (3–15 staff)** | Architecture studios, accountancy practices, design agencies, law-firm boutiques in Accra/Tema/Kumasi | Highest commercial signal: they buy email, they have a .com.gh domain, GHS pricing matters to them. Target 3 of the 7. |
| **A3. NGOs / non-profits** | Local NGOs the founder or collaborators have volunteered with; faith-based orgs; advocacy groups | DPC-compliance story resonates. Free beta + cheap post-beta tier is genuinely budget-relieving. Target 1–2. |
| **A4. Tech-startup peers** | Founders the founder met at MEST / Impact Hub / Devcongress / GhanaTech meetups; Y-Combinator Ghana cohort if reachable | High-signal feedback (they will file good bugs), good word-of-mouth. Target 1–2. |

### 4.2 Bucket B — Form A (BYO-SMTP, 3 slots)

These are individuals or tiny teams who already have email working (Gmail / Outlook /
Zoho) but want a better UI, faster mobile experience, or a unified inbox.

| Sub-bucket | Examples | Why they fit |
|---|---|---|
| **B1. Power-user developers / freelancers** | The founder's collaborators (the 6 UG teammates in `PROJECT-MEMBERS.md`), other technical friends with Gmail-on-custom-domain | Will exercise IMAP edge cases (App Passwords, OAuth, 2FA), filter rules, search. |
| **B2. Diaspora users with Ghana ties** | Family / extended network in UK / US / Canada with multiple email accounts | Exercises latency from outside Ghana, validates the BYOK story for non-Ghana hosting too. |
| **B3. Multi-mailbox sole traders** | Anyone juggling 2+ work mailboxes (a personal Gmail + a business Outlook) and tired of switching tabs | Validates the "unified inbox" Quality of Life argument. |

### 4.3 Anti-selection criteria

Do **not** invite as beta:

- Anyone the founder cannot push back on (would say "yes" but never log in).
- Customers who require an SLA, MSA, or compliance certification today — those go to
  the enterprise quote pipeline (`migrations/059_enterprise_quote_requests.sql`),
  not beta.
- Anyone on a domain the founder cannot DNS-administer (for Full Hosted) and is
  unwilling to share registrar access for.
- Anyone whose primary objection is price — beta is free, but they will churn at GHS 65
  post-beta and contaminate the retention metric.

---

## 5. Outreach Playbook

### 5.1 Channel order

For each target:

1. **WhatsApp first** — most Ghana B2B conversations happen here. 1–2 sentence opener.
2. **Email second** (within 30 min of a positive WhatsApp reply) — formal one-pager
   + agreement + Calendly link.
3. **LinkedIn DM** as the third channel only if WhatsApp is unavailable or the contact
   is diaspora.
4. **In-person / call** for the kick-off, 20 minutes max.

### 5.2 WhatsApp opener template

```
Hi [First name], hope you're well. Quick one — I've been
building a Ghana-hosted email service called TASMail
(modern UI, GHS pricing, DPC-registered).

I'm picking 10 beta users from people I know and trust
before opening to the public. Beta is free for 3 months
and I'd really value your feedback.

Would you be open to a 20-min call this week to see if
it's a fit for [their org / their personal setup]?
```

### 5.3 Email follow-up template (after a positive reply)

```
Subject: TASMail beta — short kick-off (Ghana-hosted email)

Hi [Name],

Thanks for the quick reply on WhatsApp. As promised, the
short version:

• What it is: a modern webmail for any IMAP server
  (Gmail/Outlook/Zoho/your own Postfix). Web + Flutter
  mobile, real-time push, BYOK credentials.
• Why now: closed beta — 10 users from my network, free
  for 3 months. Goal is to surface bugs and shape the
  roadmap before public launch.
• Two flavours:
   - BYO-SMTP: keep your existing email, just get a
     better UI. (3 slots)
   - Full Hosted: yourname@yourcompany.com.gh with
     hosting on our infrastructure in Accra. (7 slots)
• Compliance: DPC registration in progress, data hosted
  in Ghana, end-to-end TLS, nightly backups.

Attached:
  - Beta agreement (one-pager — sign on Doc, no MSA)
  - Privacy notice (one click: https://mail.techatscale.io/legal/privacy)
  - Calendly to grab a 20-min slot: [link]

If it's not a fit, no offence taken — just tell me.

Best,
Dominic
TASMail • https://mail.techatscale.io
```

### 5.4 Collaborator outreach scripts

Each of the 6 GitHub collaborators (`PROJECT-MEMBERS.md`) is asked to nominate **2
candidates each** from their own network. That's 12 nominations — comfortably above
the 14-name target. Collaborator brief (paste into team channel):

```
Team — please nominate 2 people each for TASMail beta.

Filter:
  • Ghana-based preferred, diaspora OK if Ghana ties.
  • They have either a real .com.gh domain (Full Hosted)
    OR an existing Gmail/Outlook they'd love a better
    UI for (BYO-SMTP).
  • They're someone you can chase a reply from in 48h
    without it being weird.

For each nominee, drop in this channel:
  • Name
  • Org / role
  • Channel you'd use to reach them
  • BYO or Full Hosted bucket
  • 1-line "why they fit"

Deadline: end of day [date]. I'll handle outreach
once we have the list.
```

### 5.5 Cadence & SLA

| Step | Target SLA |
|---|---|
| Send WhatsApp opener | Day 0 |
| Reminder if no read receipt | Day 2 |
| Drop if no reply by | Day 5 (mark `dropped` in tracker) |
| Send email follow-up after positive reply | Within 30 min |
| Kick-off call booked | Within 5 days of email follow-up |
| Onboarding session | Within 3 days of kick-off call |
| First active week of usage | Within 7 days of onboarding |

---

## 6. Pipeline Tracking

### 6.1 States

Linear states the tracker walks through:

`nominated → contacted → replied → kickoff_booked → kickoff_done →
onboarded → active → testimonial_collected | churned | dropped`

Drop / churn reasons must be recorded — they are the most useful artefact of the beta.

### 6.2 Tracker schema (`docs/beta/pipeline.csv`)

```csv
code,bucket,form,nominated_by,channel,state,state_date,blocker,notes
BC-01,A1,full_hosted,founder,whatsapp,onboarded,2026-06-04,,UG lab, 4 mailboxes
BC-02,A2,full_hosted,collab_mhandev,whatsapp,replied,2026-06-02,domain_not_owned,Architecture studio, 8 mailboxes
BC-03,B1,byo_smtp,founder,direct,active,2026-06-08,,Solo dev, Gmail App Password
...
```

Columns:

- `code` — `BC-NN` running identifier. **Never put real names here** — that file is in
  the public repo.
- `bucket` — A1 / A2 / A3 / A4 / B1 / B2 / B3 (§4).
- `form` — `full_hosted` or `byo_smtp`.
- `nominated_by` — `founder` or `collab_<github_username>`.
- `channel` — `whatsapp` / `email` / `linkedin` / `in_person`.
- `state` — current state from §6.1.
- `state_date` — ISO date the state was last updated.
- `blocker` — short tag if `state = dropped` or `churned`: `no_reply`, `price_objection`,
  `domain_not_owned`, `compliance_objection`, `feature_gap`, `provider_quirk`,
  `other`.
- `notes` — one-liner. **No PII**.

### 6.3 Identity map (`docs/beta/identities.private.md`)

```markdown
# Beta identity map — NOT FOR COMMIT

> Add `docs/beta/identities.private.md` to `.gitignore` before populating.

| Code  | Name             | Org              | Phone / email           |
|-------|------------------|------------------|-------------------------|
| BC-01 | [Real name]      | [Org]            | +233 xx xxx xxxx        |
| BC-02 | [Real name]      | [Org]            | name@company.com.gh     |
```

Verify the gitignore line exists before adding any real names:

```bash
grep -q '^docs/beta/identities.private.md$' .gitignore || \
  echo 'docs/beta/identities.private.md' >> .gitignore
```

---

## 7. Beta Agreement One-Pager (Template)

Title: **TASMail Closed Beta Programme — Participant Agreement**

```
Between: Tech at Scale Ltd (in formation), Accra, Ghana
         ("TASMail")
And:     [Participant name + org] ("Participant")

1. Scope
   TASMail is providing the Participant with free access
   to the TASMail email service during a closed-beta
   period of three (3) calendar months starting on the
   onboarding date below.

2. Service
   Either:
   (a) BYO-SMTP — webmail UI for the Participant's
       existing IMAP/SMTP credentials, OR
   (b) Full Hosted — mailbox(es) on TASMail-operated
       Postfix + Dovecot infrastructure under a
       Participant-controlled domain.

3. Participant obligations
   - Use the service in good faith for normal business
     or personal email.
   - Report bugs and feedback via in-app feedback or
     beta@techatscale.io.
   - Maintain ownership of the Participant's IMAP/SMTP
     credentials (BYO-SMTP) or domain (Full Hosted).

4. TASMail obligations
   - Operate the service on best-effort basis with
     no SLA during beta.
   - Backups: nightly snapshots, 14-day retention.
   - Security: end-to-end TLS, AES-256-GCM encryption
     of stored credentials, DPC registration pending
     under Act 843.

5. Data handling
   - TASMail never stores email body content for
     BYO-SMTP users; messages are proxied via the
     Participant's chosen IMAP server.
   - For Full Hosted, mail data is stored in the
     Participant's Maildir on TASMail infrastructure
     hosted in Accra, Ghana.
   - Privacy notice: https://mail.techatscale.io/legal/privacy

6. Post-beta
   - On day 90, TASMail will offer the Participant a
     20% founder-discount tier for 12 months on the
     standard GHS 1.00 / GB-month rate (BYO-SMTP) or
     the relevant Full Hosted plan.
   - If the Participant declines, TASMail will provide
     a full export (BYO: re-use existing IMAP server;
     Full Hosted: mbox export within 14 days) and
     delete the account.

7. Confidentiality
   - The Participant agrees not to publicly share
     specific bugs, vulnerabilities, or unreleased
     features observed during beta until 30 days after
     public launch.
   - High-level testimonials are encouraged.

8. Termination
   Either party may exit beta with 7 days' notice. No
   fees apply during beta.

Signed:                          Date:

TASMail (Dominic Dodzi)          ____________________

Participant                      ____________________
```

Use a Google Doc or DocuSign-free alternative (e.g. `signaturit.com`, `dropboxsign.com`)
to collect signatures. Store countersigned PDFs in `docs/beta/agreements/` (gitignored).

---

## 8. Onboarding Procedure

### 8.1 Kick-off call (20 min, recorded with consent)

| Min | Topic |
|---|---|
| 0–2 | Greeting + confirm bucket + which form (A or B). |
| 2–7 | Demo the SPA against a sandbox account. |
| 7–12 | Share the agreement; walk through clauses 4 and 5 (the security ones). |
| 12–17 | If BYO-SMTP: collect provider name and confirm App Password / OAuth path. If Full Hosted: collect domain name and check WHOIS + DNS access. |
| 17–20 | Schedule onboarding session (≤ 3 days out); send Calendly invite live. |

### 8.2 BYO-SMTP onboarding (30 min, screen-shared)

1. Send signup link `https://mail.techatscale.io/signup`.
2. Participant signs up with their preferred login email.
3. Walk through onboarding wizard:
   - Pick provider preset (Gmail / Outlook / Zoho / etc.).
   - Generate App Password if Gmail or Yahoo (founder shares 30-second
     screenshot walkthrough).
   - Test connection (`POST /api/imap-configs/test`) — must return 200.
4. Send one test email from a new TASMail Composer → verify receipt in
   participant's existing inbox.
5. Receive one test email from external → verify it appears in TASMail.
6. Install Android APK (sideload, document permission steps in chat).
7. Set up in-app feedback shortcut on home screen.
8. Confirm `state = active` in `pipeline.csv`.

### 8.3 Full Hosted onboarding (60 min, async DNS lag means split into 2 sessions)

**Session 1** (live, 20 min):

1. Verify participant owns the domain (WHOIS or registrar screenshot).
2. Provision mailbox(es) on TASMail Postfix/Dovecot per
   `docs/SELF-HOST-MAIL-SERVERS.md`.
3. Generate DNS records (MX, SPF, DKIM, DMARC) per `docs/DNS-MX-ONBOARDING.md`.
4. Participant logs into registrar and applies DNS records (founder watches via
   screen-share).

**24–48 h DNS propagation gap.**

**Session 2** (live, 30 min):

5. Verify propagation: `dig MX participant-domain` shows TASMail MX records.
6. Send test email from external account → verify receipt.
7. Send test email from TASMail → verify external receipt + SPF/DKIM/DMARC pass via
   `mail-tester.com`.
8. Install Android APK; configure mobile push.
9. Confirm `state = active` in `pipeline.csv`.

### 8.4 First-week check-ins

Day 1, Day 3, Day 7 — short WhatsApp ping ("everything OK? any issues?"). Log every
response in `notes`. Promote `state = active` only after Day 7 if the participant has
sent ≥ 1 outbound email and received ≥ 5 inbound through TASMail.

---

## 9. Feedback Collection & Success Metrics

### 9.1 Quantitative metrics (per-participant, weekly)

| Metric | Source | Target by week 4 |
|---|---|---|
| Outbound messages sent via TASMail | `email_queue` table count | ≥ 5/week |
| Inbound messages read in TASMail | client telemetry (read receipts) | ≥ 50 % of new mail |
| 7-day active days | session log | ≥ 5 of 7 |
| Errors per session | Sentry | < 1 |
| Mobile app sessions | mobile telemetry | ≥ 3/week |

### 9.2 Qualitative feedback

- **Week 1 survey** — 5 questions, ≤ 3 min completion time, prefilled link.
- **Week 4 interview** — 30-min recorded conversation; founder asks the same 7
  questions to every participant for cross-comparison.
- **Week 8 testimonial ask** — only for participants whose `state = active` and
  who returned positive feedback at week 4.

### 9.3 Beta success criteria (exit gate)

The beta is successful if **by day 90**:

- ≥ 8 of 10 participants are still `active`.
- ≥ 3 written testimonials collected.
- ≥ 1 anchor reference customer (preferably NGO or public-sector adjacent — supports
  the `BUSINESS-VALIDATION-GHANA.md` GTM moat).
- ≥ 70 % of participants accept the post-beta paid tier.
- Sentry shows zero unresolved P0 errors attributable to the beta cohort.

If fewer than 8 are active, **do not** open public launch — extend beta by 30 days
and replace churned slots from the standby list (§10.3).

---

## 10. Failure Modes & Recovery

| Failure | Recovery |
|---|---|
| Nomination pipeline returns < 14 names | Re-ask collaborators with a more permissive bucket list (e.g. open up to "any technical friend with two mailboxes"). Founder personally taps 2–3 advisors. |
| > 50 % no-reply rate on WhatsApp opener | Rewrite opener — most likely cause is opener is too "sales-y". Drop to one sentence + question. |
| Onboarding session reveals product blocker (e.g. Gmail OAuth flow fails) | Pause that bucket's outreach; file a ticket; re-open within 7 days. Do not power through onboarding with a known broken path — beta feedback gets contaminated. |
| Full Hosted DNS propagation fails (TTL too high at registrar) | Document the registrar in the tracker `notes`; coach the participant to ask their registrar to lower TTL to 300 s; reschedule Session 2. |
| Participant uses TASMail for ≤ 1 week then goes silent | Single chase at Day 14; if still silent at Day 21 → `state = churned`, blocker = `no_reply`. Do not push further. |
| Participant requests SLA or formal MSA during beta | Politely redirect to enterprise quote pipeline; pull them out of beta. Beta participants who escalate are not beta material. |
| Participant on shared IP affected by a deliverability problem from another beta user | Apply incident response from `IP-WARMUP-RUNBOOK.md`; communicate proactively to the affected participant; offer 1 extra month free as compensation. |
| Founder runs out of bandwidth to onboard 10 in parallel | Cap concurrent onboardings at 3/week. Stagger over 4 weeks. Do not compress the timeline at the cost of half-onboarded users. |

### 10.3 Standby list

Maintain 4 extra nominations beyond the 10 active slots in
`pipeline.csv` (`state = nominated`). When a slot churns, promote the next standby in
the same bucket to keep the A/B mix balanced.

---

## 11. Indicative Timeline

Assumes pre-flight checklist (§3) complete on Day 0.

| Day | Action |
|---|---|
| 0 | Pre-flight checklist complete; collaborator nomination brief sent; tracker + identity map files created. |
| 3 | Nomination deadline; 12+ names collected. Founder finalises 14-name target list. |
| 4 | First batch of 5 WhatsApp openers sent (Bucket A first, then B). |
| 5–9 | Second batch of 9 openers; first kick-off calls. |
| 10–17 | First wave of onboarding (3 BYO-SMTP simpler — onboard first; Full Hosted needs DNS lag). |
| 18–28 | Second wave; complete remaining Full Hosted onboardings. |
| 30 | Week-4 survey sent to first wave; second wave finishing onboarding. |
| 45 | First wave 4-week interview; check active-user count vs Day-90 target. |
| 60 | Second wave 4-week interview. |
| 75 | Week-8 testimonial asks to top performers; start drafting 2026-06-launch comms. |
| 90 | Beta exit gate (§9.3). If pass → convert participants + open public launch. If fail → 30-day extension. |

---

## 12. Cost Summary

The beta itself has minimal direct cost — the operator's time is the binding
resource.

| Line item | GHS | Notes |
|---|---|---|
| 10 Full-Hosted mailboxes × 3 months × marginal cost | ~150 | Storage + bandwidth — absorbed by founder. |
| 1 Calendly Free plan | 0 | Free tier covers the 20-min call type. |
| 1 e-sign tool free tier (Dropbox Sign / SignWell) | 0 | 3 signatures/month free; cohort is sequenced over 4 weeks. |
| WhatsApp Business app | 0 | Free. |
| Android APK signing (debug keystore) | 0 | Free; production keystore deferred to public launch. |
| **Total cash outlay** | **~150 GHS** | Roughly the cost of one mid-tier VPS-month at Aveshost. |
| Founder time | ~40 h over 90 days | ~30 min/day average. |

---

## 13. Cross-Project Hooks

- **TMAIL-17 (IP warm-up):** Beta outbound traffic must hit the warm-up schedule in
  `IP-WARMUP-RUNBOOK.md`. If beta send-rate exceeds the week-1 ramp ceiling (50/day),
  throttle via the `email_queue` priority column (`migration 066`) rather than turning
  participants away.
- **TMAIL-42 (Backups):** Verify a restore-from-snapshot drill against one Full Hosted
  beta mailbox before day 30 — proves `BACKUP-RESTORE.md` for a real account, not just
  fixtures.
- **TMAIL-44 (DPC registration):** Participants ask "are you DPC-registered?" in the
  kick-off call; the agreement (§7 clause 4) honestly says "in progress under Act 843".
  Once approved, push a 1-line email update to all participants — that is the cheapest
  trust-builder available.
- **TMAIL-18 (Hosting procurement):** Until Smart Infraco colocation is live, beta runs
  on Aveshost (per `HOSTING-PROCUREMENT.md` Phase 1). Mention the upgrade path to
  participants who ask about scaling.
- **TMAIL-43 (Company registration):** The agreement is signed "Tech at Scale Ltd (in
  formation)" until ORC issues the Certificate. Replace with the post-incorporation
  entity name once available — no participant signature needs to be redone, just attach
  an addendum referencing the Certificate.

---

## 14. Post-Beta Conversion Plan

Day 90 actions, per participant:

1. **Active + positive feedback** → send 20 %-discount post-beta offer + Paystack
   payment link. Convert in `pipeline.csv` (`state = converted`).
2. **Active + neutral feedback** → schedule 15-min retention call before sending the
   offer; understand the gap.
3. **Inactive** → send a "graceful exit" email with mbox export instructions; thank
   them; do not push.
4. **Churned (recorded blocker)** → file the blocker as a TMAIL ticket; if the blocker
   is fixed before public launch, re-invite at standard pricing.

Target conversion: **7 of 10 participants** continue into year 1 at the discount tier
— matches the BUSINESS-VALIDATION §7 break-even math (30 paying customers needed; 7
from beta is the seed pool).

---

## 15. Sources

Internal:

- `docs/BUSINESS-VALIDATION-GHANA.md` §8.1 — Phase 1 GTM specifies "10 beta customers
  from personal network (free)".
- `docs/IP-WARMUP-RUNBOOK.md` — beta send-volume floor and ramp schedule.
- `docs/DNS-MX-ONBOARDING.md` — Full-Hosted DNS records and propagation guidance.
- `docs/PROJECT-MEMBERS.md` — six collaborators each nominating 2 contributes 12 names
  to the funnel.
- `docs/PRD.md` and `BUSINESS-VALIDATION-GHANA.md` §3 — BYOK product positioning and
  post-beta tier (GHS 1.00 / GB-month, GHS 5 monthly minimum).
- `docs/SELF-HOST-MAIL-SERVERS.md` — Full-Hosted Postfix/Dovecot provisioning steps.
- `docs/BACKUP-RESTORE.md` — beta retention promise (nightly snapshots, 14-day).
- `docs/DPC-REGISTRATION-RUNBOOK.md` — compliance language in §7 clause 4.
- `docs/HOSTING-PROCUREMENT.md` — Aveshost Phase-1 infrastructure footing during beta.

External (high-level — no new web research was required for this runbook; all data
points derive from already-cited sources in `docs/research/ghana-business-validation.md`):

- DataReportal "Digital 2025: Ghana" — WhatsApp / LinkedIn penetration figures used
  in §5.1 channel ranking.
- Ghana Data Protection Commission "Act 843" — compliance phrasing in §7.
- Calendly free tier — used for kick-off scheduling (no URL hard-coded; choose any
  equivalent free scheduler).
