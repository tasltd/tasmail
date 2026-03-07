# Project Management Plan (PMBOK 7 Aligned)
# RustMail — Self-Hosted Web Email Client on Linux

**Version:** 1.0
**Date:** 2026-03-07
**Standard:** PMBOK Guide 7th Edition (2021) — Performance Domains
**Project Duration:** 12-16 weeks (MVP)

---

## 1. Brief Project Description

RustMail is a privacy-focused, self-hosted web email system running on Linux. Users access a modern Gmail-like browser interface for custom-domain email (compose, send/receive, folders, search, attachments). The system is built from open-source components: **Postfix** (SMTP mail transfer), **Dovecot** (IMAP mail delivery/access), **Rspamd** (spam filtering), and a custom **React 19 SPA** frontend connected to a high-performance **Rust (Axum)** backend. The project delivers full data control, TLS encryption, DKIM/SPF/DMARC email authentication, real-time push notifications via IMAP IDLE, and an admin interface for multi-domain management.

**Target Users:** Privacy-conscious individuals, small teams (5-50 users), and organizations seeking independence from commercial email providers (Gmail, Outlook).

**Technology Justification vs All-in-One Stacks:**

| Criterion | Mailcow/Modoboa | RustMail (Custom) |
|-----------|-----------------|-------------------|
| RAM usage | 4-8 GB (Docker) | 1-2 GB (native) |
| UI quality | SOGo (AngularJS, 2015-era) | React 19 SPA (modern) |
| Real-time push | Polling (seconds delay) | IMAP IDLE → WebSocket (< 3s) |
| Backend safety | PHP/Python (memory unsafe) | Rust (memory-safe, zero CVEs) |
| Customizability | Limited (Docker layer) | Total control |
| Setup time | 4-8 hours | 12-16 weeks development |

---

## 2. Project Integration — Planning Domain (PMBOK 7)

*Ensures project cohesion by aligning objectives, stakeholders, timeline, constraints, and assumptions into a unified plan.*

### 2.1 Project Charter

| Element | Detail |
|---------|--------|
| **Project Name** | RustMail — Self-Hosted Email Service |
| **Project Manager** | TAS Engineering |
| **Sponsor** | Project Owner |
| **Start Date** | 2026-03-10 |
| **Target Completion** | 2026-06-30 (16 weeks) |
| **Budget** | VPS: $10-20/month; Domain: $10-15/year; Development: team time |

### 2.2 Project Objectives and Success Criteria

| Objective | Success Criterion | Measurement |
|-----------|-------------------|-------------|
| O1: Functional webmail | Send/receive emails via browser UI for custom domain | End-to-end test: compose → send → receive → read |
| O2: Email deliverability | Emails reach Gmail/Outlook inboxes (not spam) | mail-tester.com score ≥ 8/10; Google Postmaster Tools pass |
| O3: Security | TLS everywhere, DKIM/SPF/DMARC aligned | MXToolbox full pass; zero critical vulnerabilities in pen test |
| O4: Real-time notifications | New emails appear in < 3 seconds | WebSocket latency test from LMTP delivery to browser |
| O5: Admin management | Create/manage domains, users, aliases via web UI | Admin flow E2E test |
| O6: Performance | < 200ms API response; < 100MB backend RAM at idle | Load test with 50 concurrent users |
| O7: Deployability | Reproducible deployment in < 30 minutes | Fresh server deployment test |

### 2.3 Stakeholder Identification and Roles

| Stakeholder | Role | Responsibility | Interest Level |
|-------------|------|----------------|----------------|
| Project Owner | Sponsor / Decision-maker | Approves scope, budget, milestones | High |
| Developer/Sysadmin | Builder | Design, implement, test, deploy | High |
| End Users | Testers / Recipients | Validate UX, report bugs, provide feedback | Medium |
| Domain Registrar | Service Provider | DNS record management | Low |
| VPS Provider | Infrastructure | Server hosting, rDNS configuration | Low |

### 2.4 Timeline with Major Milestones

| # | Milestone | Weeks | Deliverable | Acceptance Criteria |
|---|-----------|-------|-------------|---------------------|
| M0 | Project Kickoff | 0 | Charter, scope statement, WBS | Approved by sponsor |
| M1 | Infrastructure Ready | 1-2 | VPS provisioned, Postfix+Dovecot configured, DNS set | Test email delivery via CLI (swaks) |
| M2 | Backend API Complete | 3-6 | Rust backend: auth, folders, messages, search, WebSocket | All API integration tests pass |
| M3 | Frontend MVP | 7-10 | React SPA: login, inbox, compose, reply, search | E2E Playwright tests pass |
| M4 | Admin Panel + Security | 11-13 | Admin UI, DKIM signing, rate limiting, pen test | Admin E2E tests + security scan |
| M5 | IP Warm-up + Beta | 14-16 | Production deployment, IP warm-up started, 5+ beta users | mail-tester.com ≥ 8/10; no spam flags |

**Critical Path:** M1 → M2 → M3 (infrastructure blocks backend blocks frontend).
**IP Warm-up** runs parallel to M4-M5 (4-8 weeks minimum for Gmail/Outlook reputation).

```
Week:  1  2  3  4  5  6  7  8  9  10  11  12  13  14  15  16
M0:    ██
M1:    ██ ██
M2:          ██ ██ ██ ██
M3:                      ██ ██ ██ ██
M4:                                  ██  ██  ██
M5:                                              ██  ██  ██
Warm:                 ██ ██ ██ ██ ██  ██  ██  ██  ██  ██  ██
```

### 2.5 Key Constraints

| Constraint | Impact | Mitigation |
|------------|--------|------------|
| **Time:** 16-week target for MVP | Limits feature scope to P0 features | Strict scope control; no P2 features in MVP |
| **Budget:** Low-cost VPS ($10-20/mo) | Limits RAM to 2-4 GB; single server | Rust's low memory footprint; no Docker overhead |
| **Resources:** Solo/small team | Development bottleneck | Focus on highest-value features first; leverage existing crates |
| **IP Reputation:** 4-8 week warm-up | Cannot send bulk email immediately | Start warm-up at M2; use throttling; monitor blacklists |
| **Deliverability Enforcement:** Google/Microsoft 2025 rules | Emails rejected if SPF/DKIM/DMARC not aligned | Configure from day 1; test with mail-tester.com weekly |

### 2.6 Assumptions

| # | Assumption | Risk if Wrong |
|---|-----------|---------------|
| A1 | Developer has intermediate Rust and React experience | Timeline extends by 4-8 weeks for learning |
| A2 | Domain is already registered and accessible | Delays DNS configuration by 1-3 days |
| A3 | VPS provider supports rDNS/PTR configuration | **Deployment blocker** — must verify before purchase |
| A4 | VPS IP is not on email blacklists | Must check mxtoolbox.com before committing to provider |
| A5 | Target scale is < 100 users (no clustering needed) | Architecture redesign required for 100+ |
| A6 | PostgreSQL, Postfix, Dovecot are available in OS repos | Must verify package versions on chosen distro |

---

## 3. Project Scope — Delivery Domain (PMBOK 7)

*Defines what the project produces and prevents scope creep through clear boundaries and formal change control.*

### 3.1 Detailed Scope Statement

**Included (In-Scope):**

| # | Feature | Priority |
|---|---------|----------|
| 1 | Custom domain email send/receive via web browser interface | P0 |
| 2 | Modern webmail client (compose, inbox, folders, search, attachments) | P0 |
| 3 | TLS encryption on all connections (HTTPS, IMAPS, SMTPS, STARTTLS) | P0 |
| 4 | Email authentication: SPF, DKIM (2048-bit), DMARC | P0 |
| 5 | Basic spam/virus protection via Rspamd | P0 |
| 6 | User account management (create/delete, quotas, passwords) | P0 |
| 7 | Multi-domain support (10+ domains from single installation) | P0 |
| 8 | IMAP/SMTP support for external clients (Thunderbird, mobile apps) | P0 |
| 9 | Real-time push notifications (IMAP IDLE → WebSocket) | P0 |
| 10 | Admin web interface (domains, users, aliases, dashboard) | P1 |
| 11 | JWT-based authentication with RS256 signing | P0 |
| 12 | Responsive design (desktop + mobile browsers) | P1 |
| 13 | Dark mode toggle | P2 |
| 14 | Keyboard shortcuts (Gmail-like navigation) | P2 |
| 15 | Single-binary Rust backend deployment | P0 |
| 16 | Deployment on single Linux server (no Docker required) | P0 |
| 17 | Setup documentation and configuration guides | P0 |
| 18 | Backup/restore procedure | P1 |

**Excluded (Out-of-Scope) — unless added via change request:**

| # | Feature | Rationale |
|---|---------|-----------|
| 1 | CalDAV/CardDAV (calendar/contacts) | Separate concern; can run Radicale/Nextcloud alongside |
| 2 | Native mobile apps | React PWA covers mobile; native is v2.0+ |
| 3 | POP3 support | IMAP is the modern standard |
| 4 | End-to-end encryption (PGP/S/MIME) | Complex; deferred to v2 |
| 5 | Email migration from Gmail/Outlook | Separate tool; deferred |
| 6 | High-availability clustering | Single-server MVP; HA is v2 |
| 7 | Advanced groupware (shared tasks, file sharing) | Out of project scope |
| 8 | Multi-tenant SaaS hosting | Single-installation only |
| 9 | Built-in antivirus (ClamAV) | Rspamd sufficient for MVP; ClamAV is resource-heavy |
| 10 | BIMI (Brand Indicators) | Optional; requires trademark registration; deferred |
| 11 | ActiveSync for mobile clients | Complex protocol; IMAP sufficient |

### 3.2 Work Breakdown Structure (WBS)

```
1.0 Project Initiation
├── 1.1 Project charter & scope statement
├── 1.2 Technology stack finalization
├── 1.3 VPS selection & rDNS/blacklist verification
└── 1.4 Domain/DNS planning (MX, SPF, DKIM, DMARC, MTA-STS)

2.0 Infrastructure Setup
├── 2.1 VPS provisioning & OS hardening
├── 2.2 PostgreSQL installation & schema deployment
├── 2.3 Postfix installation & configuration
│   ├── 2.3.1 main.cf + master.cf
│   ├── 2.3.2 PostgreSQL virtual maps
│   └── 2.3.3 Submission port (587) with SASL
├── 2.4 Dovecot installation & configuration
│   ├── 2.4.1 IMAP + LMTP services
│   ├── 2.4.2 PostgreSQL auth backend
│   ├── 2.4.3 SASL socket for Postfix
│   └── 2.4.4 Sieve filtering
├── 2.5 Rspamd installation & milter integration
├── 2.6 DNS records (MX, A/AAAA, SPF, DKIM, DMARC, MTA-STS)
├── 2.7 TLS certificates (Let's Encrypt + certbot)
├── 2.8 Nginx reverse proxy configuration
└── 2.9 Firewall (UFW) + Fail2ban

3.0 Rust Backend Development
├── 3.1 Project scaffolding (Axum, tokio, sqlx)
├── 3.2 Configuration system (TOML)
├── 3.3 Authentication service
│   ├── 3.3.1 Argon2id password hashing
│   ├── 3.3.2 JWT (RS256) token management
│   └── 3.3.3 Session management (refresh tokens)
├── 3.4 IMAP service
│   ├── 3.4.1 Connection pool (per-user)
│   ├── 3.4.2 Folder operations
│   ├── 3.4.3 Message operations (list, fetch, flags)
│   ├── 3.4.4 Search (IMAP SEARCH / FTS)
│   └── 3.4.5 IDLE session manager
├── 3.5 SMTP service (lettre)
│   ├── 3.5.1 Message composition (MIME builder)
│   └── 3.5.2 Authenticated send via port 587
├── 3.6 WebSocket hub (IMAP IDLE → browser push)
├── 3.7 Admin API (domains, users, aliases, stats)
├── 3.8 Middleware (CORS, rate limiting, logging)
└── 3.9 Database migrations

4.0 React Frontend Development
├── 4.1 Project scaffolding (Vite + TypeScript)
├── 4.2 Authentication flow (login/logout/refresh)
├── 4.3 Folder navigation (recursive IMAP tree)
├── 4.4 Message list (virtualized, paginated)
├── 4.5 Message viewer (sanitized HTML rendering)
├── 4.6 Composer (TipTap rich text editor)
├── 4.7 Reply/Forward flow
├── 4.8 Attachment upload/download
├── 4.9 Search interface
├── 4.10 WebSocket notifications (real-time badge updates)
├── 4.11 Admin panel (domain/user/alias management)
└── 4.12 Responsive design + dark mode

5.0 Testing & Security Hardening
├── 5.1 Unit tests (Rust + React)
├── 5.2 Integration tests (IMAP/SMTP flows)
├── 5.3 E2E tests (Playwright)
├── 5.4 HTML email XSS testing (crafted payloads)
├── 5.5 Security audit (port scan, header check, rate limiting)
├── 5.6 Deliverability testing
│   ├── 5.6.1 mail-tester.com score verification
│   ├── 5.6.2 Google Postmaster Tools enrollment
│   ├── 5.6.3 MXToolbox blacklist check
│   └── 5.6.4 Send to Gmail/Outlook/iCloud — verify inbox placement
└── 5.7 Load testing (50 concurrent users)

6.0 Deployment & IP Warm-up
├── 6.1 Production deployment (systemd + Nginx)
├── 6.2 IP warm-up schedule (4-8 weeks)
│   ├── 6.2.1 Week 1: 100-500 emails/day (trusted recipients)
│   ├── 6.2.2 Week 2: 500-1,500 emails/day
│   ├── 6.2.3 Weeks 3-4: 2,000-5,000 emails/day
│   └── 6.2.4 Weeks 5-8: Full volume
├── 6.3 Monitoring setup (logs, health checks)
├── 6.4 Backup/restore procedure
└── 6.5 Beta user onboarding (5+ users)

7.0 Documentation & Closeout
├── 7.1 Setup & installation guide
├── 7.2 Configuration reference
├── 7.3 Admin manual
├── 7.4 User guide
├── 7.5 Security documentation
├── 7.6 Test report with screenshots
├── 7.7 Backup/recovery procedure
└── 7.8 Lessons learned
```

### 3.3 Key Project Requirements

| # | Requirement | Category | Verification |
|---|-------------|----------|-------------|
| R1 | Web access via HTTPS with valid TLS certificate | Functional | Browser loads without warnings |
| R2 | Send/receive emails on custom domain(s) | Functional | Successful cross-provider test (Gmail, Outlook, iCloud) |
| R3 | Anti-spam filtering with Rspamd (score-based, DNSBL) | Functional | Known spam samples are quarantined |
| R4 | Responsive UI for desktop (1024px+) and mobile (375px+) | Non-functional | Lighthouse mobile score ≥ 90 |
| R5 | Backup/restore procedure (database + Maildir) | Operational | Tested restore from backup succeeds |
| R6 | Real-time email notifications via WebSocket | Functional | New email appears in < 3 seconds |
| R7 | API response time < 200ms (p95) for message list | Performance | Load test verification |
| R8 | Argon2id password hashing with JWT RS256 authentication | Security | Security audit pass |
| R9 | DOMPurify HTML sanitization for email rendering | Security | XSS test payloads are blocked |
| R10 | DKIM/SPF/DMARC aligned and passing (mail-tester.com ≥ 8/10) | Deliverability | Automated weekly verification |

### 3.4 Process for Managing Scope Changes

1. **Request:** Submit written change request describing the change, business reason, and estimated impact on timeline/scope/budget.
2. **Assess:** Developer evaluates technical feasibility, effort (story points or hours), and dependencies.
3. **Approve:** Project owner reviews and approves/rejects. Approved changes are documented in the Change Log.
4. **Implement:** Approved changes are added to the backlog and scheduled into the next available milestone.
5. **Verify:** Change is tested and validated against the original request.
6. **Log:** All change requests (approved and rejected) are recorded in the Change Log with date, description, decision, and rationale.

**Scope Change Triggers:**
- Any feature not listed in section 3.1 "Included"
- Any timeline extension beyond ±2 weeks per milestone
- Any new technology/dependency not in the approved tech stack
- Any change to deployment architecture (e.g., adding Docker, clustering)

---

## 4. Risk Register — Uncertainty Domain (PMBOK 7)

| # | Risk | Probability | Impact | Mitigation | Owner |
|---|------|-------------|--------|------------|-------|
| R1 | VPS IP on email blacklists | Medium | Critical | Check mxtoolbox.com BEFORE purchasing VPS; choose reputable providers | Developer |
| R2 | VPS provider doesn't support rDNS/PTR | Medium | Critical | Verify rDNS capability before purchase; test with alternate provider | Developer |
| R3 | Gmail/Outlook flags emails as spam | High | High | IP warm-up (4-8 weeks); perfect SPF/DKIM/DMARC; monitor Postmaster Tools | Developer |
| R4 | IMAP protocol complexity causes bugs | Medium | High | Use battle-tested `async-imap` crate; test against real Dovecot; reference Stalwart source | Developer |
| R5 | HTML email XSS vulnerability | Medium | Critical | DOMPurify with strict allowlist; sandboxed iframe; automated XSS test suite | Developer |
| R6 | Concurrent IMAP connections exhaust server resources | Medium | Medium | Pool max 3 connections/user; 200 total; idle timeout 5 min | Developer |
| R7 | DNS host truncates DKIM TXT records | Low | High | Use split-record quoting; verify record with `dig TXT` after creation | Developer |
| R8 | Let's Encrypt cert renewal fails silently | Low | High | certbot timer + monitoring check; alert on expiry < 14 days | Developer |
| R9 | Brute force attacks on SMTP/IMAP ports | High | Medium | Fail2ban; Postfix rate limiting; Dovecot auth penalty | Developer |
| R10 | Scope creep (CalDAV, PGP, clustering requests) | Medium | Medium | Formal change request process; strict in/out scope boundaries | Owner |

---

## 5. Change Log — Measurement Domain (PMBOK 7)

| # | Date | Description | Requester | Impact | Decision | Notes |
|---|------|-------------|-----------|--------|----------|-------|
| — | — | No changes recorded yet | — | — | — | — |

*This log is updated whenever a scope change is requested, whether approved or rejected.*

---

## 6. Test Plan Summary — Delivery Domain (PMBOK 7)

| Test Type | Scope | Tool | When | Pass Criteria |
|-----------|-------|------|------|---------------|
| Unit (Backend) | Rust modules: auth, MIME, IMAP parsing | `cargo test` | Every commit | > 80% coverage |
| Unit (Frontend) | React components, hooks, stores | Vitest + RTL | Every commit | > 80% coverage |
| Integration | IMAP/SMTP flows against real Dovecot/Postfix | Rust integration tests | Per milestone | All flows pass |
| E2E | Full user flows: login, compose, send, receive | Playwright | Per milestone | All scenarios pass |
| Security | XSS, SQL injection, JWT tampering, rate limits | Manual + automated | M4 | Zero critical findings |
| Deliverability | SPF/DKIM/DMARC alignment, inbox placement | mail-tester.com | Weekly from M2 | Score ≥ 8/10 |
| Load | 50 concurrent users, 1000 messages/hour | Custom load script | M5 | < 200ms p95 response |

---

## 7. How Integration (Planning Domain) Ensures Project Cohesion

The Planning domain integrates all project aspects into a unified approach:

- **Charter** binds objectives, constraints, and stakeholders into a single authorizing document
- **Milestone timeline** creates checkpoints where all streams (infra, backend, frontend, security) must converge
- **Risk register** forces proactive identification of cross-cutting concerns (IP reputation affects infrastructure AND testing timelines)
- **Assumptions log** surfaces hidden dependencies before they become blockers
- **Integrated change control** ensures any deviation is assessed for impact across ALL project dimensions (timeline, scope, budget, quality)

Without this integration, the project risks: infrastructure deployed before backend is ready, frontend built before API is stable, or emails going live before IP warm-up is complete.

---

## 8. How Scope Management (Delivery Domain) Prevents Scope Creep

The Delivery domain defines precisely what is produced:

- **Scope statement** draws a clear line between included and excluded features — "CalDAV" or "mobile apps" are explicitly out, preventing ambiguous requests
- **WBS** decomposes work into 7 major work packages with traceable subtasks — nothing is built unless it appears in the WBS
- **Requirements list** prioritizes 10 specific, measurable requirements — each has a verification method, making "done" unambiguous
- **Change request process** adds friction to scope additions — requiring written justification, impact assessment, and approval prevents casual feature creep
- **Change log** creates an audit trail — rejected requests are documented so the same discussion doesn't repeat

Without scope management, the typical failure mode is: "Let's also add calendar sync" → "And contacts import" → "And PGP encryption" → project never ships.

---

## 9. PMBOK 7 Performance Domain Mapping

| PMBOK 7 Domain | Project Application |
|----------------|---------------------|
| **Stakeholders** | Section 2.3 — identified with roles and interest levels |
| **Team** | Solo/small team; roles defined in stakeholder matrix |
| **Development Approach** | Iterative; milestone-based with 2-week sprints within each phase |
| **Planning** | Sections 2.1-2.6 — charter, timeline, constraints, assumptions |
| **Project Work** | WBS (Section 3.2) — 7 work packages, 40+ subtasks |
| **Delivery** | Scope statement (3.1), requirements (3.3), test plan (6) |
| **Measurement** | Success criteria (2.2), change log (5), weekly deliverability checks |
| **Uncertainty** | Risk register (4) — 10 identified risks with mitigations |

---

## 10. Presentation Structure (5 Minutes)

| Segment | Duration | Content |
|---------|----------|---------|
| **Intro** | 30s | "We're building a self-hosted Gmail alternative — modern React UI, Rust backend, Postfix+Dovecot infrastructure. Full privacy, zero vendor lock-in." |
| **Planning Domain** | 1.5 min | Show charter summary → 6-milestone timeline → constraints table. Key message: "Planning integrates all streams so infrastructure, backend, frontend, and security converge at each checkpoint." |
| **Delivery Domain** | 2 min | Show scope in/out table → WBS diagram → 10 requirements. Key message: "Clear boundaries prevent scope creep. Every feature is either in the WBS or requires a formal change request." |
| **Risk Highlight** | 30s | "The #1 risk isn't code — it's IP reputation. Gmail hard-rejects unauthenticated email since November 2025. Our 4-8 week warm-up plan starts at milestone 2, running parallel to development." |
| **Close** | 30s | "Expected outcome: secure, deployable webmail in 16 weeks. Questions?" |

---

## 11. Document References

| Document | Location | Purpose |
|----------|----------|---------|
| Product Requirements (PRD) | `docs/PRD.md` | Features, personas, goals |
| System Requirements (SRS) | `docs/SSR.md` | Functional/non-functional requirements |
| Architecture | `docs/ARCHITECTURE.md` | System design, component diagrams |
| API Specification | `docs/API-SPECIFICATION.md` | REST/WebSocket endpoint reference |
| Development Setup | `docs/DEVELOPMENT-SETUP.md` | Dev environment configuration |
| Deployment Guide | `docs/DEPLOYMENT-GUIDE.md` | Production deployment steps |
| Security Documentation | `docs/SECURITY.md` | Security architecture, hardening |
| Grok Conversation (Original) | `docs/grok-conversation-raw.md` | Initial project planning discussion |

---

## 12. Sources and References

### PMBOK & Project Management
- [PMBOK 7: 8 Performance Domains — MPUG](https://mpug.com/8-planning-and-delivery-performance-domains)
- [PMBOK 7: 12 Principles — SPOCLearn](https://www.spoclearn.com/blog/principles-project-management-pmbok-edition/)
- [PMBOK Artifacts Guide — DeepProjectManager](https://deeprojectmanager.com/project-management-artifacts/)
- [PMI PMBOK Guide (Official)](https://www.pmi.org/standards/pmbok)

### Email Deliverability (2025-2026)
- [Google Sender Guidelines (Official)](https://support.google.com/a/answer/81126)
- [Google November 2025 DMARC Enforcement — Ironscales](https://ironscales.com/blog/googles-november-2025-dmarc-crackdown-what-security-and-marketing-leaders-need-to-know)
- [Microsoft Outlook Sender Requirements (May 2025)](https://techcommunity.microsoft.com/blog/microsoftdefenderforoffice365blog/strengthening-email-ecosystem-outlook%E2%80%99s-new-requirements-for-high%E2%80%90volume-senders/4399730)
- [Apple Postmaster Info](https://support.apple.com/en-us/102322)
- [BIMI 2025 Adoption Analysis — URIports](https://www.uriports.com/blog/bimi-2025-update/)
- [IP Warm-up Guide — Litmus](https://www.litmus.com/blog/ip-domain-email-warm-up)
- [PowerDMARC: Gmail Enforcement 2025](https://powerdmarc.com/gmail-enforcement-email-rejection/)

### Self-Hosted Email
- [DCHost: Building Mail Server with IP Warm-up](https://www.dchost.com/blog/en/i-built-my-own-mail-server-postfix-dovecot-rspamd-and-the-calm-path-to-deliverability-with-ip-warm%E2%80%91up/)
- [Vadosware: Self-Hosting Email Challenges](https://vadosware.io/post/its-never-been-easier-or-harder-to-self-host-email)
- [RunCloud: Best Self-Hosted Email 2025](https://runcloud.io/blog/best-self-hosted-email-server)
- [WeHaveServers: Email Deliverability DNS](https://wehaveservers.com/blog/dev-use-cases/email-deliverability-on-your-own-server-dns-spf-dkim-dmarc/)

### Technology
- [Stalwart Mail Server (Rust Reference)](https://stalw.art/)
- [Axum Framework](https://github.com/tokio-rs/axum)
- [lettre SMTP Crate](https://docs.rs/lettre/)
- [async-imap Crate](https://docs.rs/async-imap/)
- [Mailcow Documentation](https://docs.mailcow.email/)
