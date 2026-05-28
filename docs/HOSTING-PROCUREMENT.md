# Hosting Procurement Runbook (TMAIL-18)

**Version:** 1.0
**Date:** 2026-05-28
**Owner:** Platform Engineering
**Status:** Decision recorded; procurement actions pending operator sign-off
**Related:** `docs/BUSINESS-VALIDATION-GHANA.md` §4 (data sovereignty), `docs/DEPLOYMENT-GUIDE.md` §1 (server prerequisites), `docs/IP-WARMUP-RUNBOOK.md` (IP reputation), `docs/research/ghana-business-validation.md` §4 (raw research)

---

## 1. Purpose

TASMail is currently served from the workstation (`tas-src-1`) via SSH reverse tunnel to a proxy
host (`140.82.32.141`) — see `CLAUDE.md` → "Live deployment". That setup is fine for the closed beta
but is unacceptable for paying customers because:

- The workstation is not in a carrier-neutral data centre and is exposed to dumsor (load-shedding)
  and consumer-grade ISP downtime.
- Data sovereignty marketing (a core TASMail USP per `BUSINESS-VALIDATION-GHANA.md`) requires
  customer-visible Ghana hosting, not "Ghana workstation behind a proxy".
- TLS / reverse DNS / IP reputation can only be properly owned on infrastructure we control.

This runbook converts the strategic recommendation in `BUSINESS-VALIDATION-GHANA.md` ("Aveshost VPS
for beta → Smart Infraco colocation for production") into concrete procurement steps,
specs validation, cost projection, contact list, and cutover plan.

---

## 2. Decision Summary

| Phase | Provider | Form | Use case | Status |
|-------|----------|------|----------|--------|
| **Phase 0** (today) | Workstation + SSH tunnel | Self-host | Closed beta on `mail.techatscale.io` | Live |
| **Phase 1** (beta launch) | **Aveshost** (Accra) | VPS | Public beta, first 100–500 paying users | Procure now |
| **Phase 2** (production) | **Smart Infraco** (Accra) | Colocation or managed rack | Production, multi-tenant scale, SLA-bound | RFQ in parallel; cut over when Phase 1 is saturated |
| **Phase 2b** (DR / hot standby) | **Equinix AC1** or **AVANETCO** | Cross-DC standby | Disaster recovery target after submarine cable / dumsor incidents | Defer until Phase 2 is live |

Decision rationale: Aveshost wins Phase 1 on speed-to-procure (self-serve, card or MoMo, online
provisioning in minutes), GHS pricing transparency, and Ghana-located DC. Smart Infraco wins Phase 2
on power redundancy (1.7 MW installed IT capacity), direct access to the 2Africa subsea cable,
carrier-neutral peering, and NITA partnership (a credibility marker for government/MDA customers).

---

## 3. Specs Validation Against TMAIL-18 Brief

The TMAIL-18 brief specifies a **minimum** of 4 vCPU / 8 GB RAM / 100 GB SSD, Ghana-located. Validating
against the actual TASMail workload:

| Workload component | Resource demand | Notes |
|---|---|---|
| Rust/Axum backend (`tasmail-backend`) | ~150 MB RSS idle, scales with concurrent IMAP sessions | Single process; async-imap holds one TCP connection per active mailbox |
| PostgreSQL (metadata only — no mail bodies) | 1–2 GB working set for first 1,000 users | RLS context, contacts, signatures, signatures, billing, scheduled emails, ICS UIDs |
| Vite SPA (dev mode in current setup) | ~300 MB RSS | Replace with static build behind nginx for Phase 1 — see §7.2 |
| nginx + TLS termination | ~50 MB | Plus Let's Encrypt renewal cron |
| Rspamd + Redis (only if self-host mail) | 0 MB for BYOK | TASMail is BYOK by default (`docs/SELF-HOST-MAIL-SERVERS.md`) — skip Postfix/Dovecot/Rspamd unless an operator opts into self-hosted MX |
| OS, logs, headroom | ~1 GB | journald, fail2ban, pg_dump retention |

**Verdict:** 4 vCPU / 8 GB / 100 GB **comfortably covers the first ~1,000 BYOK users**. The cap will
be reached on (a) PostgreSQL connection pool when concurrent sessions exceed ~200, or (b) disk when
attachment storage + per-user signatures + scheduled emails passes ~60 GB. Both are vertical-scale
events answered by an in-place plan upgrade, not a re-architecture — Aveshost supports up to 16
vCPU / 64 GB which buys roughly 10× headroom before the Phase 2 colocation cutover becomes mandatory.

**Sizing override for Phase 2 (colocation):** order **2 × physical 1U servers**, each 8 vCPU / 32 GB
/ 2 × 480 GB SSD (RAID 1), one as primary, one as warm-standby for PostgreSQL streaming replication
and a manual TASMail backend failover. This sets us up for the `BACKUP-RESTORE.md` daily pg_dump +
incremental Maildir rsync workflow without resizing again before 5,000 users.

---

## 4. Phase 1 — Aveshost VPS Procurement

### 4.1 Plan to order

| Field | Value | Source |
|---|---|---|
| Provider | Aveshost Ghana | [aveshost.com/vps-ghana](https://www.aveshost.com/vps-ghana) |
| Plan family | Linux VPS Ghana (NOT Windows — Windows VPS starts at $85.50/mo per [aveshost.com/windows-vps-ghana](https://www.aveshost.com/windows-vps-ghana) and we don't need a Windows licence) |
| vCPU | 4 | TMAIL-18 minimum |
| RAM | 8 GB | TMAIL-18 minimum |
| Storage | 100 GB SSD | TMAIL-18 minimum |
| OS | Ubuntu 24.04 LTS | matches `DEPLOYMENT-GUIDE.md` `apt`-based instructions |
| Static IPv4 | 1 (required) | needed for PTR + rDNS + SPF |
| IPv6 | request if available | future-proofing |
| Datacentre | Accra | confirm at order time — Aveshost has both Accra DC and reseller arrangements |
| Bandwidth | unmetered up to 500 GB/mo soft cap | per Aveshost ToS; sufficient for proxy IMAP/SMTP traffic |
| Budget envelope (Phase 1) | **GHS 180 – GHS 600 / month** | TMAIL-18 brief lists GHS 180; current public Linux pricing not displayed on the public page — see §4.2 |

### 4.2 Pricing discrepancy — flag for operator

The TMAIL-18 ticket quotes **GHS 180/month**. Aveshost's public site shows:

- **Windows VPS Ghana**: from **$85.50/mo** (≈ GHS 1,070 at GHS 12.5/USD) for 2 vCPU / 4 GB / 100 GB
- **Linux VPS Ghana**: pricing not exposed on the public page; only visible after starting a quote
  on [my.aveshost.com/store/vps-ghana](https://my.aveshost.com/store/vps-ghana)
- **Dedicated server Ghana**: from **GHS 10,000/mo** per [my.aveshost.com/store/dedicated-servers-ghana](https://my.aveshost.com/store/dedicated-servers-ghana)

The GHS 180 number in TMAIL-18 likely refers to a **promotional Linux VPS starter tier** or an older
quote. **Before placing the order, the operator must request a fresh quote** for the Linux 4/8/100
configuration and reconcile against the GHS 180 figure. If actual pricing is materially above the
budget envelope, escalate to TMAIL-18 owner before ordering — do not silently upgrade the budget.

### 4.3 Order checklist

```text
[ ] Get fresh quote for Linux Ubuntu 24.04 / 4 vCPU / 8 GB / 100 GB SSD / 1 static IPv4
[ ] Confirm DC is Accra (not Lagos / Joburg / Frankfurt reseller IP)
[ ] Confirm reverse DNS (PTR) is editable from the customer panel — non-editable PTR is a hard blocker
[ ] Confirm port 25 outbound is open (or at least openable on request) — critical for any future MX
[ ] Confirm SSH key upload at provisioning time (no root password emailed in plaintext)
[ ] Pay invoice via MoMo / card / bank transfer
[ ] Receive provisioning email with IP, root credentials, hostname
[ ] Replace emailed credentials with SSH key + disable password auth before any service starts
```

### 4.4 Post-provisioning hardening (Day 0)

Run the deploy/scripts hardening sequence — `DEPLOYMENT-GUIDE.md` §1 covers the firewall + hostname
+ PTR + blacklist baseline. In short:

1. `ssh-copy-id`, then `sed -i 's/^#*PasswordAuthentication.*/PasswordAuthentication no/' /etc/ssh/sshd_config`
2. `ufw default deny incoming && ufw allow OpenSSH && ufw allow 80,443/tcp && ufw enable`
3. `hostnamectl set-hostname mail.techatscale.io` (or whichever production hostname applies)
4. PTR: open Aveshost panel → set IP → reverse to `mail.techatscale.io`
5. `mxtoolbox.com/blacklists.aspx` — verify IP is clean. If on Spamhaus PBL/SBL, request delisting before any SMTP traffic.
6. Install: `nginx`, `postgresql-16`, `certbot`, `fail2ban`, `unattended-upgrades`, `prometheus-node-exporter`.

---

## 5. Phase 2 — Smart Infraco Colocation Procurement

### 5.1 Contact + RFQ

| Field | Value | Source |
|---|---|---|
| Legal entity | Smart Infraco Ltd (SPV of Ascend Digital Solutions; Technical Partner of NITA) | [smartinfraco.com](https://smartinfraco.com/data-centre-solutions/), [ascenddigitalsol.com/smart-infraco](https://ascenddigitalsol.com/smart-infraco/) |
| Physical address | Marina Mall Building, 6th Floor, Airport City, Accra, Ghana | [datacentermap.com/c/smart-infraco-ltd](https://www.datacentermap.com/c/smart-infraco-ltd/) |
| Phone | (+233) 0307000701 | |
| Email | info@ascenddigitalsol.com | |
| Facility scale | 1.7 MW installed IT capacity | largest of its kind in Ghana |
| Subsea cable | Direct access to 2Africa | |
| Model | Carrier-neutral colocation; redundant power/cooling/network; 24×7 manned | |

### 5.2 RFQ template (send via email)

```
Subject: Colocation RFQ — TASMail (mail.techatscale.io)

Hi Smart Infraco team,

We are TASMail (operated by Tech at Scale Limited), a Ghana-based webmail
SaaS at https://mail.techatscale.io. We are planning to move off VPS hosting
to a colocation footprint with a Ghana-based provider, and Smart Infraco is
our preferred option based on your 2Africa connectivity and NITA partnership.

Please provide a quote for:

1. 1U colocation (single rack unit) — initial deployment, primary
2. Additional 1U slot in the same cabinet — warm standby (PostgreSQL streaming replica)
3. Cross-connect to the 2Africa cable / Ghana Internet Exchange (GIX) peering
4. Dual feed 230 V / 16 A power (≤ 200 W draw per node)
5. /29 IPv4 allocation (or two static /32 from your range) + IPv6 /64
6. Remote hands SLA (response time for tier-1 incidents)
7. Optional: managed PDU, smart hands quote per hour, KVM-over-IP

Hardware will be customer-supplied (Dell R250 or equivalent 1U short-depth).

We expect to move ~2 TB/month traffic at peak, predominantly outbound SMTP
relay + IMAP responses (Phase 2). Please share your standard SLA terms and
any onboarding lead time.

Best,
Dominic Dottey — operator, TASMail
dfdnusenu@gmail.com
```

### 5.3 What we need back before signing

- Quoted monthly recurring + one-off install fee (in GHS, with USD reference if applicable)
- Lead time from contract to live (target: ≤ 4 weeks)
- Audited SLA percentage (target: ≥ 99.99% facility uptime)
- Confirmation that we keep ownership/admin of the hardware (so we can wipe and ship if we churn)
- Confirmation that **/29 IPv4 reverse DNS** is delegable to us (otherwise PTR records are blocked)
- Network egress pricing tier — if metered, what's the inclusive bundle?

### 5.4 Alternatives if Smart Infraco RFQ fails or is overpriced

| Provider | DC | Why consider it | Source |
|---|---|---|---|
| **Equinix AC1** | Accra (Airport City) | Tier-1 colocation, Equinix Internet Exchange peering, enterprise pricing | [equinix.com/data-centers/europe-colocation/ghana-colocation/accra-data-centers](https://www.equinix.com/data-centers/europe-colocation/ghana-colocation/accra-data-centers) |
| **Digital Realty ACR2** | Accra | Tier-1, listed Accra colocation alternative | [digitalrealty.com/data-centers/emea/accra/acr2](https://www.digitalrealty.com/data-centers/emea/accra/acr2) |
| **MainOne MDXi Accra** | Accra | West African data centre operator (Equinix-owned since 2022) | [baxtel.com/data-center/accra](https://baxtel.com/data-center/accra) |
| **PAIX (Pan African IXP)** | Accra | GIX-adjacent, peering-friendly | `docs/research/ghana-business-validation.md` §4.3 |
| **AVANETCO** | Accra | Hybrid VPS + colocation, 99.9% uptime, 1 Gbps port, accepts crypto | [avanetco.com/ghana-vps-hosting](https://www.avanetco.com/ghana-vps-hosting/) |

Equinix and Digital Realty are likely 2–3× the Smart Infraco price; consider them only if Smart
Infraco can't deliver redundant power within the lead time.

---

## 6. Cost Projection (Indicative)

All figures GHS / month. USD reference at GHS 12.5 / USD. Treat as planning numbers — actual
quotes will replace these once §4.2 and §5.2 are completed.

### 6.1 Phase 1 (Aveshost VPS, first 12 months)

| Line item | Monthly (GHS) | Annual (GHS) | Notes |
|---|---|---|---|
| VPS 4/8/100 (target) | 180 – 600 | 2,160 – 7,200 | range per §4.2 — confirm with Aveshost quote |
| Backup storage (off-site, S3-compatible Ghana or rsync.net) | 60 | 720 | per `BACKUP-RESTORE.md` |
| Domain renewal (techatscale.io) | 12 | 145 | amortised |
| TLS (Let's Encrypt) | 0 | 0 | free |
| Monitoring (Grafana Cloud free tier or self-host on the VPS) | 0 | 0 | self-hosted |
| **Phase 1 total** | **252 – 672** | **3,025 – 8,065** | |

### 6.2 Phase 2 (Smart Infraco colocation, steady state)

| Line item | Monthly (GHS) | Annual (GHS) | Notes |
|---|---|---|---|
| 1U colocation slot, primary | TBD (estimate 1,500 – 3,000) | TBD | RFQ pending |
| 1U colocation slot, warm standby | TBD (estimate 1,500 – 3,000) | TBD | RFQ pending |
| Cross-connect / peering | TBD (estimate 400 – 800) | TBD | per active cross-connect |
| /29 IPv4 + IPv6 | TBD (estimate 200) | TBD | per RFQ |
| Hardware capex (2 × Dell R250 or equivalent) | — | ~50,000 one-off | amortise over 36 months → 1,400/mo |
| Remote hands (per-incident) | variable | ~3,000 | budget 2 incidents / quarter |
| Backup off-site (Ghana + AWS Frankfurt) | 200 | 2,400 | dual-region for resilience |
| **Phase 2 monthly target** | **5,000 – 9,000** | **60,000 – 108,000** | excludes capex amortisation |

### 6.3 Break-even check

Phase 2 monthly cost ÷ TASMail BYOK price (GHS 1 / GB · month, GHS 5 minimum per `CLAUDE.md`):
- GHS 5,000/mo breaks even at **1,000 minimum-tier users** OR **5,000 GB billable storage**
- GHS 9,000/mo breaks even at **1,800 users** OR **9,000 GB billable storage**

Phase 1 to Phase 2 cutover should not be triggered before the user base reaches ~600 paying users
(comfortably covers Phase 2 cost at minimum tier). If usage tilts heavier than minimum, cutover can
happen earlier. Source: `BUSINESS-VALIDATION-GHANA.md` pricing ladder.

---

## 7. Cutover Plans

### 7.1 Phase 0 → Phase 1 (workstation → Aveshost VPS)

Prerequisite: §4.4 hardening done on the new VPS.

```text
T-7d  Order Aveshost VPS, complete §4.4 hardening, install nginx + postgres-16 + certbot.
T-5d  Provision SSL via certbot for mail.techatscale.io (DNS pre-pointing — see below).
T-3d  Build production binary on workstation:
        cd backend && cargo build --release
      Scp release binary + frontend dist + migrations dir to VPS:/opt/tasmail/
T-3d  Build static frontend:
        cd frontend && npm run build  # output in dist/
      Configure nginx to serve dist/ + reverse-proxy /api and /ws to backend.
T-2d  Restore latest pg_dump from workstation onto the VPS Postgres (`BACKUP-RESTORE.md` §3).
T-1d  Run smoke tests against https://mail.techatscale.io.staging-vps (a staging hostname):
        - GET /api/health → 200
        - login + send a test email via BYOK SMTP → reaches recipient
        - WS connection stays open
        - billing webhook idempotency check
T+0   Cutover window (low-traffic hour, ~03:00 GMT):
        1. Put workstation backend into read-only mode (refuse writes for 5 min).
        2. Run final delta pg_dump and apply to VPS.
        3. Update DNS A record for mail.techatscale.io to VPS public IP (TTL pre-lowered to 60s).
        4. Stop SSH reverse tunnel on the proxy (`140.82.32.141`); remove Apache vhost.
        5. Watch logs on VPS for 30 min; smoke-test signup + login + send.
T+1d  Decommission workstation backend service (keep workstation Postgres for 30 days as DR).
T+7d  Tear down workstation Postgres if no rollback was needed.
```

### 7.2 Replace Vite dev server with built SPA

The current live setup (per `CLAUDE.md` → "Live deployment") runs `vite dev` on the workstation.
That's fine for the closed beta but unacceptable for VPS production:

- Vite dev is HMR-enabled, ~300 MB RSS, recompiles on demand.
- Built SPA is ~1 MB gzip, served as static files by nginx (~5 MB RSS), and cacheable at the CDN
  edge later.

In the T-3d step above, swap `tasmail-vite.service` for a one-shot `npm run build` baked into the
deploy script, then point nginx at `frontend/dist/`. Delete the systemd unit after cutover.

### 7.3 Phase 1 → Phase 2 (Aveshost → Smart Infraco colocation)

Same overall shape as §7.1 but more involved because we're moving across providers, IPs change,
and we're adding a warm standby. Add these steps:

- **Streaming replication** from Aveshost primary → Smart Infraco standby for at least 1 week
  before the cutover (catch up + verify lag stays under 1 s).
- **IP warmup** of the new Smart Infraco /29 — re-run the 8-week ramp in `IP-WARMUP-RUNBOOK.md`
  before any outbound MX traffic flows. For BYOK-only traffic (IMAP/SMTP proxy) the warmup is much
  shorter (~1 week) because we're not the originating MTA.
- **PTR delegation** must be confirmed in writing from Smart Infraco before cutover (Phase 2 has
  no fallback PTR if delegation fails).
- **Backup off-site target** must be the *other* DC (Aveshost-side or AWS Frankfurt), not the new
  Smart Infraco rack — otherwise a DC-level event takes out primary + backup simultaneously.

---

## 8. Risk Register

| Risk | Likelihood | Impact | Mitigation |
|---|---|---|---|
| Submarine cable cut (e.g. March 2024 incident, 8-week MainOne outage) | Medium | High | Smart Infraco's 2Africa direct access mitigates; cross-region backup to AWS Frankfurt as DR |
| Dumsor (load-shedding) at consumer ISPs | High | Low at DC | Both Aveshost and Smart Infraco have redundant generators; the risk is at our operator workstation, not the DCs |
| Aveshost PTR delegation refused | Medium | High | Confirm BEFORE ordering (§4.3); reject the order and switch to AVANETCO or HostAfrica if refused |
| Port 25 blocked on Aveshost outbound | Medium | High for self-host MX, Low for BYOK | TASMail is BYOK by default — port 25 is irrelevant unless an operator opts in to `setup-all.sh`. If self-host MX becomes needed, file ticket with Aveshost to unblock 25 |
| Smart Infraco quote exceeds budget | Medium | Medium | Phase 2 alternatives in §5.4 (Equinix, Digital Realty, MainOne, AVANETCO) |
| IP reputation poor on new IP range | High | High | Run `IP-WARMUP-RUNBOOK.md` 8-week ramp before any MX traffic; for BYOK proxy, 1-week ramp |
| Vendor lock-in to a single Ghana provider | Medium | Medium | Phase 2b warm standby in a *different* DC (Equinix or AVANETCO) |
| Smart Infraco staffing churn (small provider) | Low | Medium | Contractual SLA with credits + 30-day notice clause |
| Workstation rollback window too short (T+7d) | Low | High | Extend to T+30d for Phase 1 cutover; archive workstation pg_dump to off-site for 90d |

---

## 9. Sources (procurement)

All URLs verified 2026-05-28. Group by topic for easy refresh.

### Aveshost (Phase 1)
- [VPS Ghana — Aveshost](https://www.aveshost.com/vps-ghana)
- [Windows VPS Ghana — Aveshost](https://www.aveshost.com/windows-vps-ghana) (USD pricing reference)
- [VPS Ghana store / cart](https://my.aveshost.com/store/vps-ghana)
- [Dedicated servers Ghana](https://my.aveshost.com/store/dedicated-servers-ghana) (escalation tier — GHS 10,000+/mo)
- [Self-managed VPS plans](https://www.aveshost.com/self-managed-vps-hosting)
- [Aveshost pricing index](https://www.aveshost.com/pricing)
- [How to Buy Web Hosting in Ghana 2026 — Aveshost blog](https://blog.aveshost.com/web-hosting-in-ghana/)

### Smart Infraco (Phase 2)
- [Data Centre Solutions — Smart Infraco](https://smartinfraco.com/data-centre-solutions/)
- [Smart InfraCo Ltd profile — datacentermap.com](https://www.datacentermap.com/c/smart-infraco-ltd/)
- [Smart Infraco overview — Ascend Digital Solutions](https://ascenddigitalsol.com/smart-infraco/)
- Office: Marina Mall Building, 6th Floor, Airport City, Accra | Phone: (+233) 0307000701 | Email: info@ascenddigitalsol.com

### Phase 2 alternatives
- [Equinix AC1 (Accra) colocation](https://www.equinix.com/data-centers/europe-colocation/ghana-colocation/accra-data-centers)
- [Digital Realty ACR2 (Accra)](https://www.digitalrealty.com/data-centers/emea/accra/acr2)
- [Accra Data Centers & Colocation — baxtel.com](https://baxtel.com/data-center/accra)
- [Ghana Data Centers & Colocation — baxtel.com](https://baxtel.com/data-center/ghana)
- [AVANETCO VPS Ghana (1 Gbps, 99.9% uptime, crypto-friendly)](https://www.avanetco.com/ghana-vps-hosting/)
- [HostAfrica Ghana (Shared / VPS / Cloud / Dedicated)](https://www.hostafrica.com.gh/)
- [NetActuate Accra Data Center](https://netactuate.com/data-centers/accra-data-center)
- [HostAdvice — 5 Best Ghana VPS Hosting Services (2026)](https://hostadvice.com/vps/ghana/)
- [Best Web Hosting Providers In Ghana 2026 — Faciotech](https://blog.faciotech.com/best-web-hosting-providers-in-ghana/)
- [WHTop — VPS providers in Ghana directory](https://www.whtop.com/directory/country/gh/category/vps)
- [Best Web Hosting Companies in Ghana — Goodfirms 2026](https://www.goodfirms.co/web-hosting-companies/ghana)

### Peering / resilience context
- [GIXA — Ghana Internet eXchange Association](http://www.gixa.org.gh/)
- [GISPA — March 2024 internet disruption analysis](https://gispa.org.gh/internet-disruption-a-look-into-the-role-of-the-ghana-internet-exchange/)
- [GNA — Ghana needs robust internet connectivity after March 2024](https://gna.org.gh/2024/06/ghana-needs-robust-resilient-internet-connectivity-to-avoid-march-2024-service-disruption/)

### Internal references
- `docs/BUSINESS-VALIDATION-GHANA.md` — strategy + provider shortlist
- `docs/research/ghana-business-validation.md` §4 — raw research with 80+ sources
- `docs/DEPLOYMENT-GUIDE.md` §1 — server prerequisites checklist
- `docs/IP-WARMUP-RUNBOOK.md` — 8-week sender reputation ramp
- `docs/BACKUP-RESTORE.md` — pg_dump + Maildir rsync routine
- `docs/SELF-HOST-MAIL-SERVERS.md` — why Postfix/Dovecot are deferred (informs Phase 1 sizing)
- `CLAUDE.md` → "Live deployment" — current workstation + tunnel setup

---

## 10. Next Actions (operator)

These are the only steps that move TMAIL-18 from "decision recorded" to "procured":

1. Request a **written quote** from Aveshost for the Phase 1 spec (§4.1). Reconcile with the
   GHS 180 budget (§4.2). If actual is materially higher, escalate before ordering.
2. Send the **Smart Infraco RFQ** from §5.2 in parallel — lead time to live is likely 4–6 weeks
   so the RFQ has to be in flight while Phase 1 is being provisioned.
3. Once the Aveshost quote is in and budget approved, run §4.3 → §4.4 → §7.1.
4. When the Smart Infraco quote is in, decide whether to sign or fall back to §5.4 alternatives.
5. Schedule the Phase 1 → Phase 2 cutover (§7.3) once the user base passes ~600 paying users
   (per §6.3 break-even check).

This document does **not** authorise payment or provisioning on its own — it is the input to that
decision, not the decision itself.
