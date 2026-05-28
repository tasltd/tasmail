# Beta Launch Runbook — 4-Week Closed Beta with 10 Customers (TMAIL-47)

**Version:** 1.0
**Date:** 2026-05-28
**Owner:** Founder (Dominic Dodzi)
**Status:** Procedure documented; launch pending operator action
**Related:**
- `docs/BETA-CUSTOMER-RECRUITMENT-RUNBOOK.md` (TMAIL-45) — upstream: how the 10 customers were recruited
- `docs/IP-WARMUP-RUNBOOK.md` (TMAIL-17) — beta traffic is the warm-up trickle; weekly volumes from §6.1 here must stay within the warm-up envelope
- `docs/BUSINESS-VALIDATION-GHANA.md` §8.1 (Phase 1 GTM)
- `docs/PROJECT-MANAGEMENT-PLAN.md` (sprint cadence, post-beta milestones)
- `docs/BACKUP-RESTORE.md` (TMAIL-42) — nightly snapshots are a hard pre-launch dependency
- `docs/DNS-MX-ONBOARDING.md` (Full Hosted onboarding flow used during Week 1)
- `docs/SELF-HOST-MAIL-SERVERS.md` (Postfix/Dovecot provisioning for the 7 Full Hosted slots)
- `docs/PAYMENT-PROVIDER-MIGRATION.md` (TMAIL-163) — payment providers are NOT enabled during beta; this runbook intentionally bypasses billing
- `docs/SECURITY.md` and `docs/DPC-REGISTRATION-RUNBOOK.md` (privacy posture surfaced to beta users)

---

## 1. Purpose

TMAIL-47 launches and operates the **4-week closed beta** with the 10 customers
recruited under TMAIL-45. The beta is the first time TASMail will carry production
mail for non-founder users, so this runbook exists to make the launch a managed,
measurable, reversible operation rather than a "we flipped the switch" event.

The beta has four explicit success goals; everything in this runbook serves at
least one of them:

1. **Deliverability** — outbound mail from beta accounts lands in primary inboxes
   at Gmail, Outlook, Yahoo and Apple Mail. Target: ≥ 95 % inbox placement at
   Gmail by Week 3 per Google Postmaster Tools (GPT) once the domain hits GPT's
   ~100-msg/day visibility floor — see `IP-WARMUP-RUNBOOK.md` §3.
2. **Performance** — p95 IMAP folder-list < 800 ms, p95 SPA "open thread" < 1.5 s
   on Accra 4G, p95 send-to-Sent-folder reflection < 3 s, end-to-end push
   notification latency < 10 s. Measured continuously from Week 1.
3. **UX** — at least 8 of 10 customers complete the onboarding wizard without
   founder intervention; ≤ 2 critical blockers reported per week by Week 3;
   ≥ 7 of 10 say "I would recommend this to a peer" in the Week-4 exit survey.
4. **Reference customers** — 2–3 written testimonials and 1 anchor reference
   customer agreed in writing for the public launch (`BUSINESS-VALIDATION-GHANA.md`
   §10 critical success factor #6).

Like TMAIL-45, the three prior auto-fix passes on TMAIL-47 produced no code
changes because **launching a beta is human operations work, not software work**.
This runbook captures every operational input needed so the 28-day window runs
on a planned cadence instead of reactive firefighting.

---

## 2. Decision Summary

| Choice | Selection | Rationale |
|---|---|---|
| Beta window | **4 weeks (28 days)** — Mon-start, Sun-end | Matches PM plan; long enough to see a full week-over-week deliverability trend at GPT, short enough to keep founder energy and customer attention high. |
| Cohort | **10 customers** (3 BYO-SMTP + 7 Full Hosted) | As recruited under TMAIL-45 §2. If recruitment lands 8–9, **proceed**; if < 8, **delay launch by ≤ 1 week** to fill — do not launch under-cohort. |
| Pricing during beta | **Free** | Billing intentionally NOT enabled (matches recruitment runbook §2 and avoids Paystack KYC gating). Beta agreement includes 20 % founder discount for 12 months post-beta. |
| Feedback channel | **Single shared WhatsApp group + in-app feedback button + 1-on-1 weekly check-ins** | WhatsApp is the Ghana B2B working channel; in-app captures bug context; 1-on-1s surface qualitative signal the group will not. |
| WhatsApp group structure | **One shared group, founder-admin, 10 customers + 1 founder + 1 collaborator backup** | Single group beats two (B vs F) because cross-cohort visibility builds confidence and surfaces shared issues fast. Size (12) is below WhatsApp's social-noise threshold. |
| Beta SLA | **Critical: 2 hrs; High: next business day; Medium: 3 business days; Low: best-effort by end of beta** | Honest about being a small team; differentiates real outages from feature requests. |
| Status cadence | **Weekly Friday digest email + WhatsApp ping** | Predictable, low-noise, asynchronous. Avoids ad-hoc reactive updates. |
| Monitoring stack | **Prometheus + Grafana (perf), Sentry (errors), Google Postmaster Tools (deliverability), pg-boss queue depth, log-based UX funnel** | Each metric has a named owner (founder for all four, but the dashboard is single-pane-of-glass). |
| Graduation criteria | **All 4 §1 goals met OR explicit decision to extend by 2 weeks** | No silent slip into open beta. Week-4 review is a hard gate. |
| Exit interviews | **30-min 1-on-1 with every customer in Week 4**, written summary per customer | Quantitative survey alone is insufficient at n=10. |
| Communications archive | **All WhatsApp + email threads exported weekly to `docs/beta/communications/wk{N}.md`** (gitignored — PII) | Operator memory aid; basis for §11 exit synthesis. |

---

## 3. T-7 Pre-Launch Checklist

The Monday-morning launch only happens once every item below is **green or
explicitly waived in writing**. Treat any red item as a launch blocker.

### 3.1 Product readiness (re-verify TMAIL-45 §3.1)

- [ ] `https://mail.techatscale.io/signup` accepts a fresh signup → IMAP attach → first folder list, end-to-end, on a clean browser profile.
- [ ] All 11 IMAP presets validated against a live account (Gmail App Password, Outlook OAuth, Zoho personal, FastMail, ProtonMail Bridge, iCloud App Password as a minimum).
- [ ] Postfix/Dovecot Full Hosted stack provisioned for at least **3 of 7** Full Hosted customers (the remainder can be staggered through Week 1; tracked in §4.1).
- [ ] Mobile Flutter APK build available + sideload instructions documented for Android customers.
- [ ] In-app feedback button live (or `mailto:beta@techatscale.io` fallback explicitly accepted as the Week-1 substitute).
- [ ] **Per-account brute-force lockout active** (TMAIL-273, already merged) — beta accounts get the same protection as the founder account.
- [ ] WebSocket push (`/ws`) tested for at least one beta account — verified message arrival within 10 s of an external send.

### 3.2 Operational readiness

- [ ] Sentry org `tasltd` ingesting backend errors with `release` tag set to the current beta build SHA (so a Week-2 regression is bisectable against a single deploy).
- [ ] Nightly pg_dump + Maildir rsync verified working for **3 consecutive nights** with a successful restore drill on the third (per `BACKUP-RESTORE.md`). No exceptions — losing a beta customer's mail is the single worst non-security outcome possible.
- [ ] Prometheus scrape of `/metrics` confirmed; Grafana board (§5.2) renders all four monitoring panels with non-zero data from the founder's own mailbox.
- [ ] systemd units `tasmail-{backend,vite,tunnel}.service` configured with `Restart=always` and `RestartSec=10s`; tested by killing the backend process and observing a < 30 s recovery.
- [ ] Google Postmaster Tools enrollment **submitted** and the DNS TXT challenge published (full data appears once the domain reaches ~100 msgs/day; submitting now means data will be available by Week 2).
- [ ] DMARC policy in `p=none` reporting mode with `rua=` pointing to a monitored mailbox — `p=quarantine` is deferred until Week 4 (`IP-WARMUP-RUNBOOK.md` §4).
- [ ] DNS for the 7 Full Hosted domains validated: MX, SPF, DKIM, DMARC, A/AAAA for webmail subdomain, all resolving from a non-cached resolver (`dig +trace`).
- [ ] **Founder phone + work email + WhatsApp Business** all monitored; out-of-office auto-reply describes the beta SLA tiers from §2.

### 3.3 Legal / commercial readiness

- [ ] Beta agreement countersigned by **all 10 customers** before they receive credentials. No verbal-only customers in the cohort.
- [ ] Privacy notice at `/legal/privacy` reviewed by collaborator; matches the data-handling description in the beta agreement (no contradictions between the two documents).
- [ ] DPC application acknowledgement filed in `docs/beta/legal/dpc-ack.pdf` (private) so it can be produced on request.
- [ ] Beta exit-survey form ready (Google Form or in-product) with the 8 questions from §11.2 pre-loaded.

### 3.4 Comms assets ready

- [ ] WhatsApp group created + named **"TASMail Beta (May–Jun 2026)"** + group-icon set to TASMail mark.
- [ ] Day-0 welcome message (template §10.1) finalised.
- [ ] Weekly digest email template (§10.4) saved as a draft in the founder's mailbox.
- [ ] Critical-incident communication template (§10.5) saved as a draft.
- [ ] Status page (lightweight markdown at `https://mail.techatscale.io/status` or a pinned WhatsApp message) ready with the four-tier status definition: Operational / Degraded / Partial outage / Full outage.

---

## 4. Phase Plan — 28-Day Cadence

The beta runs as four 7-day phases with named outcomes. Phase boundaries are
hard checkpoints: if a phase's exit criteria fail, that is the trigger to slow
down or replan, not to silently roll into the next phase.

### 4.1 Week 1 — "Land" (Mon Day 1 → Sun Day 7)

**Goal:** every beta customer has a working mailbox they can send and receive
from on at least one device.

**Day 0 (Sunday before):** founder confirms all §3 items are green. Any red
item → push launch by 1 week.

**Day 1 (Mon):**
- 09:00 — send Day-0 welcome WhatsApp + email (template §10.1).
- 09:30 — WhatsApp group created and all 10 customers added (the +1 collaborator
  joins as observer/backup).
- 10:00–17:00 — **first 3 customers onboarded** via scheduled 20-minute calls
  (highest-stake or highest-tech first so any onboarding-wizard bugs surface
  immediately, not on Day 4 with the least-technical customer).
- 17:30 — daily standup-by-self: founder writes 5 lines into
  `docs/beta/communications/wk1.md` covering issues seen, fixes shipped, blockers.

**Day 2–4:** onboard the remaining 7 customers, 2–3 per day. Stagger Full Hosted
domain provisioning across these days (don't batch DNS cutovers).

**Day 5–7:** all 10 customers receiving and sending. By Friday night, every
customer must have:
- sent at least 1 outbound message,
- received at least 3 inbound (founder will send "are you receiving this?" pings),
- opened the SPA on desktop and at least attempted the mobile app or PWA.

**Exit criteria (Sun Day 7):**
- [ ] ≥ 9 of 10 customers active (logged in in the last 48 h).
- [ ] 0 P0 incidents open (P0 = customer can't receive at all).
- [ ] ≤ 2 P1 incidents open (P1 = degraded send/receive or repeated errors).
- [ ] Outbound volume ≥ 50 messages/day on at least 2 days (warm-up trickle floor).

If any criterion fails: **freeze new feature ships, reallocate Week 2 to
remediation, do NOT recruit replacement customers mid-cycle**.

### 4.2 Week 2 — "Watch" (Mon Day 8 → Sun Day 14)

**Goal:** observe natural usage, surface bugs at the boundary the test suite
cannot reach (provider quirks, device variety, real attachments, real
contacts).

**Daily cadence:**
- 09:00 — founder reviews overnight Sentry + Postmaster + Grafana (§5.2) and
  posts a 1-line "morning weather report" to the WhatsApp group ("All green",
  "Gmail delivery flaky last hour, investigating", etc.). This single discipline
  is the highest-leverage trust builder of the beta.
- During the day — issues triaged per §7 SLA.
- 17:00 — `docs/beta/communications/wk2.md` updated.

**Mid-week (Wed):** founder runs the **deliverability probe** — sends a
controlled batch of 20 outbound messages from 3 beta accounts to mail-tester.com
and to seed inboxes at Gmail/Outlook/Yahoo. Records scores in
`docs/beta/deliverability/wk2.md`. Threshold: mail-tester ≥ 9.0/10; any seed
inbox landing in spam → P1 incident.

**Friday:** first weekly digest email (template §10.4). One per cohort, BCC.

**Exit criteria (Sun Day 14):**
- [ ] All 10 customers still active (no churn).
- [ ] Sentry error rate (errors/100 reqs) ≤ 2× the pre-launch baseline.
- [ ] Postmaster Tools showing data (i.e., daily volume cleared the visibility
      floor at least once).
- [ ] At least 5 customer-reported issues logged in the issue tracker (TMAIL).
      Zero reported issues by Week 2 means customers aren't actually using
      the product, not that the product is perfect.

### 4.3 Week 3 — "Iterate" (Mon Day 15 → Sun Day 21)

**Goal:** ship fixes for the issues raised in Weeks 1–2; expand mobile usage;
start preparing exit conversations.

**Cadence:** same daily morning report; ship 1–3 small fixes per day rather
than batching a big drop. Every shipped fix gets a WhatsApp ping naming the
fix and the customer(s) it affects.

**Mid-week (Wed):** founder schedules the 10 exit-interview slots for Week 4.
Calendar invites go out by Wed evening — do not leave scheduling until Week 4.

**Friday:** second weekly digest. Include a "what we shipped this week" section
with linked TMAIL ticket numbers.

**Exit criteria (Sun Day 21):**
- [ ] Gmail inbox placement ≥ 95 % per Postmaster (if Postmaster has 7+ days of
      data; otherwise mail-tester score ≥ 9.5 averaged across 5 probes).
- [ ] p95 IMAP folder-list < 800 ms over the rolling 24 h.
- [ ] All P0/P1 issues from Weeks 1–2 closed.
- [ ] ≥ 8 of 10 exit interviews confirmed scheduled.

### 4.4 Week 4 — "Land and Decide" (Mon Day 22 → Sun Day 28)

**Goal:** run exit interviews, collect testimonials, make the public-launch /
extend-beta decision.

**Mon–Thu:** 10 × 30-min exit interviews (§11.1 script). Each writeup committed
to `docs/beta/exit/<BC-code>.md` (gitignored — references the private identity
map).

**Wed:** exit survey (§11.2) sent to all 10 by email; results due by Fri 17:00.

**Thu:** founder + 1 collaborator hold a 90-min **graduation review** working
session. Inputs: §1 goals, §11 exit data, monitoring data. Output: a written
decision in `docs/beta/exit/decision.md` of one of:
1. **Graduate** — open beta in 2 weeks per `PROJECT-MANAGEMENT-PLAN.md`.
2. **Extend** — run 2 more weeks against named remediation items.
3. **Hold** — public launch deferred to next quarter; specific blockers named.

**Fri:** founder publishes graduation decision to the WhatsApp group + email.
For graduating customers: 20 % founder discount confirmed, payment provider
flipped on for them only, beta agreement ends, paid agreement (or
month-to-month with no contract) begins.

**Sun Day 28:** WhatsApp group archived (not deleted — kept read-only for
6 months as a customer-relationship channel). Beta period formally ends.

**Exit criteria:**
- [ ] All 10 exit interviews completed.
- [ ] ≥ 7 of 10 net-positive responses on "would recommend".
- [ ] Written testimonials from ≥ 2 customers (text + permission-to-publish).
- [ ] ≥ 1 customer confirmed as the public-launch anchor reference.
- [ ] Decision document committed.

---

## 5. Monitoring Plan

### 5.1 What we monitor (and why)

| Dimension | Metric(s) | Source | Threshold | If breached |
|---|---|---|---|---|
| **Deliverability — Gmail** | Inbox-vs-spam placement, IP reputation, domain reputation, spam-rate, feedback-loop complaints | Google Postmaster Tools (`https://postmaster.google.com`) | Inbox placement ≥ 95 % by end of Week 3; spam rate < 0.1 %; domain reputation ≥ Medium | P1 if placement drops below 90 %; freeze new sending volume, investigate per `IP-WARMUP-RUNBOOK.md` §6 |
| **Deliverability — Outlook/Yahoo/Apple** | Probe scores (no public dashboard at these providers) | `mail-tester.com` weekly probe + seed-inbox checks | mail-tester ≥ 9.0/10; seed inboxes always primary | P1 if any provider routes seed to spam |
| **Performance — backend** | p50/p95/p99 for `/api/folders/{f}/messages`, `/api/messages/send`, `/api/auth/login` | Prometheus + Grafana panel "TASMail RED" | p95 folder-list < 800 ms; p95 send < 3 s; p99 login < 1.5 s | P2 if breached for > 30 min; investigate via Sentry + slow-query log |
| **Performance — frontend** | Page-load, TTI, INP for `/inbox`, `/compose`, `/calendar` | Browser RUM (`web-vitals` library reporting to `/api/metrics/vitals` — add if not present, otherwise weekly Lighthouse run on the 3 routes) | p95 INP < 200 ms; p95 inbox load < 2.5 s on Accra 4G | P2 if INP > 500 ms; record a profile, file bug |
| **Performance — push** | Time from inbound IMAP arrival → WebSocket frame at SPA | Synthetic probe: send from a control account every 5 min, log first-paint timestamp | < 10 s end-to-end | P2 if > 30 s sustained |
| **Errors** | Sentry error rate per release | Sentry org `tasltd` | ≤ 2× pre-launch baseline (baseline captured Day 0) | P1 if 5× baseline; auto-rollback to previous release SHA |
| **Queue** | pg-boss queue depth, retry-count, dead-letter count | Prometheus exporter on the queue tables | Queue depth < 100; dead-letter rate < 1 % | P1 if dead-letter > 5 % (silent message loss is the worst beta outcome) |
| **Capacity** | DB connections (HikariCP-equivalent in `sqlx`), disk free on Maildir mount, RSS of `tasmail-backend` | `node_exporter` + Postgres exporter | Disk free > 30 %; RSS stable over rolling 24 h | P2 if disk < 20 %; expand or trim per `BACKUP-RESTORE.md` retention |
| **UX funnel** | Onboarding wizard completion rate per step (signup → IMAP test → first folder load → first send) | Structured log events emitted by SPA, aggregated daily | ≥ 80 % step-to-step pass-through | P2 if any step drops below 60 % — that step is broken |

### 5.2 Single-pane Grafana board

A single board named **"TASMail Beta — Wk N"** with the following panels in
this order (so the morning report is read top-to-bottom):

1. **Status banner** — Operational / Degraded / Partial outage / Full outage,
   set by a simple Grafana alert combining the three "P0-critical" conditions:
   send-success-rate < 95 % over 5 min, IMAP-list error rate > 10 % over 5 min,
   backend 5xx rate > 5 % over 5 min.
2. **Outbound volume** (last 7 days) overlaid against the warm-up envelope from
   `IP-WARMUP-RUNBOOK.md` §3.
3. **Sentry error rate** vs baseline (annotation lines mark each deploy).
4. **p95 latency** for the four hot endpoints (folder-list, message-get,
   send, login).
5. **Queue depth + dead-letter rate** (single panel, two series).
6. **Customer activity heatmap** — one row per beta customer, one column per
   hour of the day, intensity = number of authenticated requests. Reveals
   inactivity and broken-account patterns immediately.
7. **Postmaster snapshot** (manual update, screenshot pasted weekly — GPT has
   no public API).

### 5.3 Synthetic probes

Three continuous probes run from a control account (NOT a beta customer):

| Probe | Frequency | What it does | Owner |
|---|---|---|---|
| **Send-and-check** | Every 15 min | Sends a timestamped message to a Gmail seed inbox; reads back via IMAP; alerts if round-trip > 60 s or fails | founder |
| **Login + folder list** | Every 5 min | Authenticates against the API, lists `INBOX`, asserts non-empty response | founder |
| **Webhook + push** | Every 5 min | Sends an inbound message to a control account; asserts the WebSocket frame arrives within 10 s in a headless Chromium tab | founder (Playwright `tests/probes/push-probe.spec.ts` to be added if not present) |

Probe results write to a Postgres table `probe_results` (or are scraped by
Prometheus) so the dashboard panel "Probe success rate" is the single most
honest signal of "is TASMail actually working right now".

### 5.4 Privacy boundary — what we do NOT monitor

- We do NOT read message bodies or subjects in monitoring. All metric labels
  use account IDs and folder paths only.
- We do NOT log full URLs from the SPA; PII may live in query strings.
- We do NOT screenshot beta inboxes for the runbook (screenshots use seed/control
  accounts only). This protects customer trust, the DPC posture, and the
  privacy-notice promise.

---

## 6. WhatsApp Group Operating Norms

### 6.1 Volume guardrails

- **Founder posts** ≤ 5 messages/day on average. The morning weather report is 1
  of those; any incident comms are extra and unbudgeted.
- **Customers post freely** — there is no rate norm imposed on them. Their job
  in the group is to talk.
- A single message from a customer triggers the §7 SLA clock; the founder must
  acknowledge within the SLA tier even if the resolution comes later.

### 6.2 Topic rules

- ✅ Bugs, feature requests, "is anyone else seeing this", praise, "feature
  X is great", quick "how do I…" questions.
- ✅ Outage acknowledgements + ETAs from the founder.
- ❌ No customer-vs-customer disputes — escalate to private DM.
- ❌ No marketing pitches to other customers from inside the group.
- ❌ No PII or message bodies from real mail — if a customer wants to debug a
  specific message they share its message-ID or screenshot the SPA with body
  masked. The founder pins this rule on Day 1.

### 6.3 Permission set

- Founder = admin.
- Collaborator backup = admin (so a sick-day on the founder's side does not
  silence the group).
- Customers = members.
- New members CANNOT be added by customers (admin-only adds — prevents an
  enthusiastic customer accidentally inviting their CTO before launch comms
  are ready).

### 6.4 Daily morning weather report — required template

> 🌤️ TASMail beta — Day {N} of 28
> Status: {Operational / Degraded / Partial outage}
> Overnight: {1 sentence — quietest night, or top issue}
> Today's plan: {1 sentence — what's shipping, what's being investigated}
> Open issues: {count by severity, e.g. "P1×0, P2×2, P3×4"}
> Need from you: {prompt for feedback or "nothing — keep using"}

Posting this every weekday morning is non-negotiable. Missed days erode the
trust-by-routine that makes the cohort tolerant of bugs.

---

## 7. Incident Severity & SLA

| Severity | Definition | Acknowledge | Update cadence | Resolve target | Customer comms |
|---|---|---|---|---|---|
| **P0 Critical** | At least one customer cannot read or send mail at all, or any data-loss event | 30 min | Every 30 min until resolved | 4 h | Immediate WhatsApp + email; status page flipped to Full outage |
| **P1 High** | Send or receive is degraded; deliverability dips below thresholds; broken auth; broken push for > 1 h | 2 h (business hours) / 4 h (after-hours) | Daily | 24 h | WhatsApp on first occurrence + in daily morning report |
| **P2 Medium** | Slow performance not breaching p95 floors, cosmetic SPA bugs, mobile-only issues affecting < 30 % of cohort | Next business day | Per weekly digest | 5 business days | In weekly digest |
| **P3 Low** | Feature requests, polish, edge-case behaviours, "nice to have" | Next business day (acknowledgement only) | Per weekly digest | Best-effort by Day 28; otherwise filed for post-beta | In weekly digest |

**After-hours definition:** weekdays before 08:00 or after 20:00 GMT, weekends
all day. The founder is not on call for P2/P3 outside business hours.

**Escalation:** if a P0 is open > 2 h with no resolution path, founder pages
the collaborator backup via WhatsApp + phone call. The backup's job is not to
fix the bug — it is to be the second pair of eyes and to handle customer comms
so the founder can focus on the fix.

---

## 8. Feedback Intake & Triage

### 8.1 Channels (priority order)

1. **In-app feedback button** (preferred — captures user agent, current route,
   account ID automatically). Feedback lands as a TMAIL ticket with label
   `beta-feedback`.
2. **WhatsApp group** — anything posted that looks like a bug is mirrored by
   the founder into a TMAIL ticket within 1 business hour; the ticket number
   is replied as a thread in the group ("filed as TMAIL-{N}").
3. **Direct WhatsApp DM** — same mirroring discipline as the group; preferred
   for issues with PII context.
4. **Email to `beta@techatscale.io`** — same mirroring discipline.
5. **1-on-1 weekly check-in** — qualitative feedback captured in the
   `docs/beta/communications/wk{N}.md` file.

### 8.2 Triage taxonomy

Every TMAIL ticket created from beta feedback gets exactly one severity (per §7)
and one category label:
- `cat:onboarding` — signup, IMAP attach, first-folder load
- `cat:read` — inbox list, thread view, attachments
- `cat:compose-send` — composer, drafts, send, scheduled send
- `cat:mobile` — Flutter app specifically
- `cat:performance`
- `cat:deliverability`
- `cat:billing-future` — feature requests around billing (deferred but logged)
- `cat:ux-polish` — non-blocking visual or interaction polish
- `cat:other`

This taxonomy feeds the §11 exit synthesis directly — at exit we report
"x P1s by category" and the categories light up the priority list for the
post-beta hardening sprint.

### 8.3 Closing the loop

Every closed ticket gets a WhatsApp ping naming the customer(s) who reported
it. "Closed TMAIL-{N} (the Gmail draft-restore bug) in build {SHA} — thanks
@{customer}, please confirm it's gone for you." This single discipline is
the most reliable testimonial generator.

---

## 9. Roles & RACI

| Activity | Founder | Collaborator backup | Customer |
|---|---|---|---|
| Daily morning report | **R/A** | C (writes if founder unavailable) | I |
| Onboarding calls | **R/A** | C (joins first 3 calls as observer) | R (attends) |
| Incident response | **R/A** for P0/P1 | C (comms during P0) | I |
| Weekly digest | **R/A** | I | I |
| Exit interviews | **R/A** | C (joins 2 as observer) | R (attends) |
| Graduation decision | A | C (must sign off) | C (input via survey + interview) |
| Deliverability monitoring | **R/A** | I | I |
| Backup verification | **R/A** | C (does the Week-2 restore drill) | I |
| Comms archive | **R/A** | I | I |

R = Responsible, A = Accountable, C = Consulted, I = Informed.

---

## 10. Comms Templates

### 10.1 Day-0 welcome (email + WhatsApp)

> Subject: Welcome to the TASMail beta — let's get you set up this week
>
> Hi {first name},
>
> Thank you for joining the closed beta. You're one of 10 customers helping
> shape TASMail before public launch.
>
> **What happens now**
> 1. You'll get a WhatsApp invite to "TASMail Beta (May–Jun 2026)" — that's
>    our shared room for questions, bugs and the daily morning update.
> 2. Pick a 20-minute slot for your kickoff call here: {Calendly link}.
>    We'll get your mailbox attached and your first messages flowing on
>    that call.
> 3. After the call, use the app like you would normally. The point of the
>    beta is to find what we missed.
>
> **What to expect from us**
> - A short status update on WhatsApp every weekday morning.
> - A Friday digest email summarising the week.
> - Same-day response to anything that breaks send/receive; next-business-day
>   for everything else.
>
> **Beta agreement** — countersigned PDF attached. The beta is free for
> 3 months; after that you get a 20 % founder discount for the first year if
> you stay.
>
> Talk soon,
> Dominic

### 10.2 Onboarding-call confirmation (email)

> Subject: Your TASMail setup call — {date} at {time}
>
> Hi {first name},
>
> Confirming our 20-minute call on {date} at {time} GMT — link: {Meet/Zoom URL}.
>
> Before we hop on, please have ready:
> 1. The email address you want to use with TASMail (and the existing IMAP
>    server it lives on — Gmail, Outlook, Zoho, etc.).
> 2. For Gmail/Yahoo/iCloud: a freshly generated **App Password** (we'll walk
>    through this on the call if you haven't done it before).
> 3. Your phone, for the mobile app sideload if you're on Android.
>
> We'll get you onboarded, send a couple of test messages, and you're off.

### 10.3 Daily morning weather report (WhatsApp)

See §6.4 for the required template.

### 10.4 Friday weekly digest (email — BCC to all 10)

> Subject: TASMail beta — Week {N} of 4 digest
>
> Hi all,
>
> Quick summary of the week.
>
> **This week's headline:** {1 sentence}
>
> **Deliverability:**
> - Gmail placement: {%}
> - mail-tester probe score: {X/10}
> - Outlook/Yahoo/Apple seed: {Primary / Promotions / Spam}
>
> **Performance:**
> - p95 inbox load: {ms}
> - p95 send: {ms}
> - Push latency p95: {s}
>
> **What we shipped:**
> - TMAIL-{n} — {title} (thanks @{customer})
> - TMAIL-{n} — {title}
> - {…}
>
> **Open issues:**
> - P1: {count} — {brief list}
> - P2: {count}
>
> **Coming next week:**
> - {1–3 bullets}
>
> Reply to this email or ping the WhatsApp group with anything I missed.
>
> Thanks for keeping us honest this week.
> Dominic

### 10.5 Critical incident comms (WhatsApp + email)

> Subject: [TASMail] Service degraded — {service area}
>
> {Timestamp GMT}
>
> We're seeing {symptom} affecting {scope — all customers / X customers /
> mobile only / etc.}.
>
> We are: {investigating / mitigating / monitoring after fix}.
> Next update by: {time}.
>
> You can keep working on {what still works}. If you hit a hard block, reply
> here or call/WhatsApp +233-{number} directly.
>
> — Dominic

### 10.6 Resolved incident comms

> {Timestamp GMT} — RESOLVED: {service area}
>
> Root cause: {1 sentence in plain English}.
> Fix shipped: {build SHA}.
> Customer impact: {duration + scope}.
> What we're doing so it doesn't happen again: {1–2 bullets}.
>
> Full writeup: TMAIL-{n}.

---

## 11. Exit & Graduation

### 11.1 Exit interview script (30 min, 1-on-1)

5 sections × ~5 min each, last 5 min for the customer's own questions.

1. **The before-state** — "Before TASMail, what were you using for email and
   what bugged you about it?"
2. **The onboarding** — "Walk me through what the first day with TASMail felt
   like. Where did you get stuck, if anywhere?"
3. **Daily use** — "What's the one thing you do every day that TASMail makes
   easier? What's the one thing it makes harder?"
4. **Trust** — "On a scale of 1–10, how comfortable are you trusting TASMail
   with your work mail in 3 months? What would have to be true to move that
   number up by 2?"
5. **Future** — "Would you recommend TASMail to a peer today? Would you give
   me a one-paragraph quote we can use publicly? Would you be willing to be
   named as a reference customer?"

Notes go straight into `docs/beta/exit/<BC-code>.md` (gitignored — PII).

### 11.2 Exit survey (8 questions, 5-min form)

1. How satisfied are you with TASMail overall? (1–10)
2. How likely are you to keep using TASMail after the beta? (1–10)
3. How likely are you to recommend TASMail to a peer (NPS)? (1–10)
4. Which feature do you use most often? (free text)
5. Which feature do you find most frustrating? (free text)
6. Compared to your previous email setup, TASMail is: (much worse / worse / same / better / much better)
7. After the beta ends, your plan is to: (continue paid / continue if free / stop using / undecided)
8. Anything else? (free text)

### 11.3 Graduation decision matrix

| Condition | Decision |
|---|---|
| All 4 §1 goals met + ≥ 7 net-positive on survey Q3 | **Graduate** to open beta |
| 3 of 4 §1 goals met + ≥ 5 net-positive | **Extend** by 2 weeks against named remediation list |
| ≤ 2 §1 goals met OR < 5 net-positive | **Hold** — public launch deferred; debrief required |

The decision is **written into `docs/beta/exit/decision.md`** with the
specific evidence supporting it. No verbal-only graduations.

### 11.4 Post-beta customer transition

For each graduating customer:
- [ ] Beta agreement formally ends (email confirmation).
- [ ] 20 % founder discount applied to their account on the public price
      (`PAYMENT-PROVIDER-MIGRATION.md` covers the provider flip).
- [ ] Payment provider enabled for their account only (others stay on free
      until public launch).
- [ ] If they agreed to be a reference customer, their quote goes into
      `marketing/testimonials.md` with the written permission attached.
- [ ] If they declined to continue, perform a `backup-restore` export of
      their data per `BACKUP-RESTORE.md` §7 and email it to them within 7
      days; then schedule account deletion 30 days later per the DPC posture.

---

## 12. Rollback & Stop Criteria

The beta is reversible. The following conditions trigger an immediate **pause**
(no new sends, no new signups, no new feature deploys) while the founder
decides whether to continue, extend or terminate the beta:

| Condition | Decision window | Outcome options |
|---|---|---|
| Any data-loss event (message lost, account corrupted) affecting ≥ 1 customer | 4 h | Restore from backup → continue, OR terminate beta and rebuild |
| Sentry error rate > 5× baseline for > 1 h | 1 h | Roll back to previous release SHA; resume only after RCA |
| Gmail domain reputation drops to "Low" or "Bad" | 24 h | Halt outbound from that domain; resume only after `IP-WARMUP-RUNBOOK.md` §6 mitigation |
| 3+ customers churning in the same week | 48 h | Pause; conduct churn interviews before continuing |
| Founder unable to maintain daily ops (illness, emergency) for > 3 consecutive days with no collaborator backup available | Immediate | Pause WhatsApp group with a frank update; resume on a defined date or terminate beta cleanly |
| Legal/regulatory event (DPC notice, ISP abuse complaint, takedown) | Immediate | Cease relevant activity; consult; only resume after written all-clear |

Terminating a beta cleanly is far better than letting it limp. If the
decision is to terminate, customers get a written notice, a data export per
§11.4, and the option of a 12-month free tier on the eventual public launch
as goodwill.

---

## 13. Open Questions for Operator Review

These do not block the runbook from being committed, but the operator should
decide them before §3 sign-off:

1. **Mobile sideload vs Play Store internal track** — sideloading APKs is
   faster but feels less professional. Decide if Week 3 should attempt
   internal-track distribution for the Android customers.
2. **Status page hosting** — pinned WhatsApp message vs lightweight
   `/status` route. The runbook assumes WhatsApp pin for cost reasons; a
   public `/status` page costs nothing if it lives on the same Apache vhost
   as the SPA.
3. **Probe account ownership** — the synthetic probes need a control
   mailbox that is NOT one of the beta customers. Decide whether to spin up
   `probe@techatscale.io` or use the founder's own mailbox (the latter
   contaminates founder-account metrics).
4. **Exit-survey hosting** — Google Forms is fast; a self-hosted survey
   preserves the data-sovereignty narrative. Pick one before Week 4.
5. **Founder discount on what plan exactly** — 20 % off which base price?
   `BUSINESS-VALIDATION-GHANA.md` §3 has the BYOK price; clarify if it
   applies to Full Hosted too or only BYOK.

---

## 14. Acceptance Criteria for This Runbook

This runbook is "done" (i.e., TMAIL-47 can move to In Review) when:
- [x] Every pre-flight item in §3 has a check box and an unambiguous pass/fail
      definition.
- [x] All 4 weeks have entry conditions, daily cadence, exit criteria.
- [x] All 4 §1 goals (deliverability, performance, UX, references) have at
      least one named monitoring metric and at least one Week-4 exit signal.
- [x] All comms templates are present and ready to copy-paste.
- [x] Severity tiers are defined with concrete examples and SLA numbers.
- [x] Rollback / stop conditions are explicit.
- [x] Privacy boundary on monitoring is explicit.
- [x] The runbook references its upstream (TMAIL-45) and downstream
      (public launch) so it can be re-found from either direction.

Operator pickup: read §3 first, then §4 week 1, then §6 and §7. The rest is
reference while the beta runs.
