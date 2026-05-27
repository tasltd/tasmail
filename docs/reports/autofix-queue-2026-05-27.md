# TMAIL Auto-Fix Queue Report — 2026-05-27

Queued via TASCIM PM `trigger_work_item_auto_fix` MCP tool against project
`TMAIL` (`5ac920c6-94e7-4c91-98a8-ca14cd45ab83`). Every active work item
(states `Todo` + `In Progress`) was enqueued — **80 items in total** across
4 priority buckets.

## How the auto-fix prompt is built

Each call to `trigger_work_item_auto_fix(project_id, work_item_id)` spawns
a fresh headless Claude session on the backend. The session is **not** sent
a free-form prompt — instead the auto-fixer assembles its own prompt from
the work item record, roughly:

1. **System context** — project CLAUDE.md + global rules, plus the
   auto-fix agent's standing instructions (investigate → plan →
   implement → verify → comment back on PM).
2. **Task brief** — the work item's `identifier`, `title`, `description`,
   `priority`, `work_item_type`, plus any comments / parent links.
3. **Action contract** — "find the relevant code, write the fix, run the
   project's tests, push a branch + PR, update this work item with files
   touched and verification steps, then transition to Done."

So the "prompt" for, say, TMAIL-156 is effectively: *"You are working on
TMAIL-156 — 'Migrate remaining IMAP handlers to ImapService::for_user()'.
Read the task description, locate the remaining handlers under
`backend/src/handlers/` that still use the global `ImapService::new`, port
them to `for_user(state, user_id)` per the pattern already used in
`handlers/folders.rs`, run `cargo test` + `cargo clippy`, push a branch
referencing TMAIL-156, comment progress on the PM ticket, and transition
to Done."*

Each session is queued with `--dangerously-skip-permissions`-equivalent
auto-fix scopes and uses the project's default model routing.

## Run summary

| Bucket | Count | Status |
|--------|-------|--------|
| Urgent | 20 | ✅ all queued |
| High | 37 | ✅ all queued (1 transient 502 on TMAIL-54 — retried, queued) |
| Medium | 21 | ✅ all queued (1 transient 502 on TMAIL-104 — retried, queued) |
| Low | 2 | ✅ all queued |
| **Total** | **80** | **80/80 queued, 0 failed** |

The two 502 Proxy Errors hit the upstream pm-api gateway, not the auto-fix
backend — both were re-issued and succeeded on the next attempt.

## Notes / caveats worth flagging

| Concern | Items | Why it matters |
|---------|-------|-----------------|
| **Infrastructure tasks the AI can't physically execute** | TMAIL-11 (Postfix install), TMAIL-12 (Dovecot install), TMAIL-13 (DNS), TMAIL-15 (Rspamd install), TMAIL-16 (Let's Encrypt cert), TMAIL-17 (8-week IP warm-up), TMAIL-18 (procure VPS), TMAIL-40 (Nginx prod deploy), TMAIL-42 (backups), TMAIL-43/44 (company + DPC registration), TMAIL-45/47 (recruit beta customers), TMAIL-117 (Radicale deploy), TMAIL-102 (Ollama deploy) | These require operator action on the live host or off-keyboard tasks. The auto-fix session will most likely produce a runbook / script + PM comment rather than a real "Done" transition. Expect operator review. |
| **Duplicate** | TMAIL-159 ≡ TMAIL-163 ("Insert PayPro production credentials into payment_provider_config") | Both queued. First one to finish will likely close both via comment; the other can be marked duplicate manually. |
| **Epic queued** | TMAIL-155 ("BYOK webmail follow-ups (post-launch)") | Epics aren't usually auto-fixable as a single unit. The session will probably decompose it into child tasks rather than ship a PR. |
| **Live financial code** | TMAIL-46 (Paystack/MTN MoMo), TMAIL-159/163 (PayPro prod credentials) | Per global rules, the auto-fixer will NOT execute live transactions against production gateways. Code changes only; the credential rows must be inserted manually by you. |
| **Already In Progress** | TMAIL-154, TMAIL-194 | Auto-fix re-queues these — the new session will read existing comments and resume from where the previous one left off. |

## Sessions queued (priority-ordered)

Session names follow the pattern `fix-tmail-<num>-<HHMMSS>-<4-char-suffix>`.
You can stream live output with `mcp__tascim-pm__get_session_output` or
`tmux attach -t <session_name>` on the orchestrator host.

### Urgent (20)
| # | Work item | Title | Session |
|---|-----------|-------|---------|
| 1 | TMAIL-156 | Migrate remaining IMAP handlers to ImapService::for_user() | `fix-tmail-156-064927-jlup` |
| 2 | TMAIL-59 | Attachment storage strategy with ClamAV scanning | `fix-tmail-59-064936-xjpe` |
| 3 | TMAIL-58 | Email queue management with retry logic | `fix-tmail-58-064938-pxag` |
| 4 | TMAIL-52 | Mobile-optimized API endpoints | `fix-tmail-52-064940-hnsk` |
| 5 | TMAIL-51 | Offline-first sync protocol | `fix-tmail-51-064942-lxga` |
| 6 | TMAIL-50 | FCM/APNs push notification service | `fix-tmail-50-064943-biyz` |
| 7 | TMAIL-49 | Mobile platform decision: Flutter for Ghana market | `fix-tmail-49-064945-womy` |
| 8 | TMAIL-48 | Implement BYO-SMTP configuration flow | `fix-tmail-48-064947-nrzz` |
| 9 | TMAIL-46 | Integrate Paystack and MTN MoMo payment gateway | `fix-tmail-46-064948-jevl` |
| 10 | TMAIL-42 | Set up automated backups | `fix-tmail-42-064950-gabf` |
| 11 | TMAIL-40 | Production deployment with Nginx and systemd | `fix-tmail-40-064952-bsjb` |
| 12 | TMAIL-39 | Email deliverability testing | `fix-tmail-39-064953-yvsn` |
| 13 | TMAIL-37 | Security audit: XSS, CSRF, injection | `fix-tmail-37-064955-yewc` |
| 14 | TMAIL-36 | Write E2E tests with Playwright | `fix-tmail-36-064957-uvyv` |
| 15 | TMAIL-35 | Write integration tests for Postfix/Dovecot/PostgreSQL | `fix-tmail-35-064959-qjik` |
| 16 | TMAIL-17 | Begin IP warm-up protocol (8-week schedule) | `fix-tmail-17-065003-jnog` |
| 17 | TMAIL-16 | Configure TLS certificates with Let's Encrypt | `fix-tmail-16-065005-payo` |
| 18 | TMAIL-13 | Configure DNS records (MX, SPF, DKIM, DMARC, autoconfig) | `fix-tmail-13-065009-xjdl` |
| 19 | TMAIL-12 | Install and configure Dovecot IMAP server | `fix-tmail-12-065027-wief` |
| 20 | TMAIL-11 | Install and configure Postfix MTA with virtual mailbox domains | `fix-tmail-11-065056-mckl` |

### High (37)
| # | Work item | Title | Session |
|---|-----------|-------|---------|
| 21 | TMAIL-158 | Cache per-user IMAP/SMTP config in Redis | `fix-tmail-158-065214-ymye` |
| 22 | TMAIL-155 | BYOK webmail follow-ups (post-launch) *(epic)* | `fix-tmail-155-065215-lird` |
| 23 | TMAIL-138 | Implement large file sharing via cloud storage links | `fix-tmail-138-065216-jfsb` |
| 24 | TMAIL-136 | Implement bulk user provisioning and CSV import | `fix-tmail-136-065216-yukv` |
| 25 | TMAIL-131 | Implement outbound webhooks for third-party integrations | `fix-tmail-131-065217-kedx` |
| 26 | TMAIL-130 | Implement ActiveSync protocol support | `fix-tmail-130-065218-frcr` |
| 27 | TMAIL-126 | Implement tasks and to-do list | `fix-tmail-126-065219-ajnm` |
| 28 | TMAIL-124 | Implement phishing protection with link scanning and warning banners | `fix-tmail-124-065220-zufx` |
| 29 | TMAIL-122 | Implement drag-and-drop for messages and folders | `fix-tmail-122-065221-wuru` |
| 30 | TMAIL-121 | Implement Gmail-like keyboard shortcuts | `fix-tmail-121-065222-efcr` |
| 31 | TMAIL-119 | Build contacts management app | `fix-tmail-119-065222-napm` |
| 32 | TMAIL-118 | Build calendar UI with FullCalendar | `fix-tmail-118-065223-dkgd` |
| 33 | TMAIL-117 | Deploy and integrate Radicale CalDAV/CardDAV server | `fix-tmail-117-065224-lpyv` |
| 34 | TMAIL-115 | Build PST import for Outlook migration | `fix-tmail-115-065224-vufd` |
| 35 | TMAIL-112 | Implement custom SMTP/IMAP hostnames per tenant (SNI) | `fix-tmail-112-065225-tjky` |
| 36 | TMAIL-111 | Implement white-label branding (logo, colors, domain) | `fix-tmail-111-065226-dbuo` |
| 37 | TMAIL-105 | Implement BYOK AI integration (Bring Your Own Key) | `fix-tmail-105-065227-vbdp` |
| 38 | TMAIL-100 | Implement LDAP/Active Directory user sync | `fix-tmail-100-065227-cwku` |
| 39 | TMAIL-99 | Implement OIDC (Sign in with Google/Microsoft) | `fix-tmail-99-065228-kkhy` |
| 40 | TMAIL-97 | Implement email delegation (send-as / send-on-behalf) | `fix-tmail-97-065229-bdld` |
| 41 | TMAIL-94 | Implement email templates with merge fields | `fix-tmail-94-065256-uaqx` |
| 42 | TMAIL-83 | Implement WebAuthn/FIDO2 passkeys | `fix-tmail-83-065257-aigp` |
| 43 | TMAIL-68 | Email import/export (MBOX, EML) | `fix-tmail-68-065259-ciiq` |
| 44 | TMAIL-56 | App store distribution (Play Store + Huawei AppGallery) | `fix-tmail-56-065259-ttzz` |
| 45 | TMAIL-55 | Native OS integrations (share sheet, camera, contacts) | `fix-tmail-55-065300-asvl` |
| 46 | TMAIL-54 | Mobile UX: swipe gestures, bottom nav, FAB compose *(retried after 502)* | `fix-tmail-54-065347-tsbt` |
| 47 | TMAIL-53 | Biometric authentication and secure storage | `fix-tmail-53-065302-nxer` |
| 48 | TMAIL-47 | Launch beta program with 10 initial customers | `fix-tmail-47-065302-rvhx` |
| 49 | TMAIL-45 | Recruit 10 beta customers | `fix-tmail-45-065304-ozci` |
| 50 | TMAIL-44 | Register as Data Controller with DPC (Act 843) | `fix-tmail-44-065305-jdzj` |
| 51 | TMAIL-43 | Register company with Office of the Registrar of Companies | `fix-tmail-43-065306-devf` |
| 52 | TMAIL-41 | Set up monitoring (Prometheus + Grafana) | `fix-tmail-41-065307-lnzv` |
| 53 | TMAIL-38 | Performance benchmarking | `fix-tmail-38-065308-daho` |
| 54 | TMAIL-33 | Implement responsive design and mobile optimization | `fix-tmail-33-065310-ypsi` |
| 55 | TMAIL-32 | Build search interface with advanced filters | `fix-tmail-32-065310-yqoo` |
| 56 | TMAIL-18 | Procure VPS hosting in Ghana | `fix-tmail-18-065312-welx` |
| 57 | TMAIL-15 | Install and configure Rspamd spam filter | `fix-tmail-15-065313-otwd` |

### Medium (21)
| # | Work item | Title | Session |
|---|-----------|-------|---------|
| 58 | TMAIL-194 | Locate noreply account credentials and existing BYOK E2E tests *(was In Progress)* | `fix-tmail-194-065349-pfex` |
| 59 | TMAIL-192 | NotebookLM critique notebook + apply feedback | `fix-tmail-192-065350-eyma` |
| 60 | TMAIL-163 | Insert PayPro production credentials into payment_provider_config | `fix-tmail-163-065351-bnkz` |
| 61 | TMAIL-159 | Insert PayPro production credentials into payment_provider_config *(dup of 163)* | `fix-tmail-159-065352-wjop` |
| 62 | TMAIL-154 | Explore tascim, tasservat, cloudy-tas for mail deployment context *(was In Progress)* | `fix-tmail-154-065353-ropp` |
| 63 | TMAIL-137 | Implement eDiscovery search across email archives | `fix-tmail-137-065355-kqgw` |
| 64 | TMAIL-135 | Implement AI-powered NLP search queries | `fix-tmail-135-065356-tssw` |
| 65 | TMAIL-134 | Implement AI compose assist (full draft generation) | `fix-tmail-134-065357-bwzd` |
| 66 | TMAIL-133 | Add POP3 support via Dovecot | `fix-tmail-133-065358-ftui` |
| 67 | TMAIL-128 | Implement internal comments on emails (Streams-like) | `fix-tmail-128-065359-vnlc` |
| 68 | TMAIL-127 | Implement meeting scheduling with calendar integration | `fix-tmail-127-065400-bbdm` |
| 69 | TMAIL-125 | Implement DANE (DNS-based Authentication of Named Entities) | `fix-tmail-125-065401-wcgp` |
| 70 | TMAIL-109 | Implement retention policies and legal hold | `fix-tmail-109-065402-upyl` |
| 71 | TMAIL-108 | Implement DLP (Data Loss Prevention) milter | `fix-tmail-108-065404-pmpv` |
| 72 | TMAIL-107 | Integrate email archiving with Piler | `fix-tmail-107-065405-lmkj` |
| 73 | TMAIL-106 | Implement semantic search with pgvector | `fix-tmail-106-065405-kooq` |
| 74 | TMAIL-104 | Implement smart reply suggestions *(retried after 502)* | `fix-tmail-104-065418-wjpk` |
| 75 | TMAIL-103 | Implement email summarization | `fix-tmail-103-065408-ejpq` |
| 76 | TMAIL-102 | Set up Ollama local LLM inference server | `fix-tmail-102-065409-lfok` |
| 77 | TMAIL-101 | Implement SAML 2.0 SSO (enterprise IdPs) | `fix-tmail-101-065410-gmkz` |
| 78 | TMAIL-57 | Mobile localization for Ghana | `fix-tmail-57-065411-aooj` |

### Low (2)
| # | Work item | Title | Session |
|---|-----------|-------|---------|
| 79 | TMAIL-132 | Design plugin/extension architecture | `fix-tmail-132-065412-poqy` |
| 80 | TMAIL-129 | Implement team chat integration (webhook-based) | `fix-tmail-129-065413-gtjj` |

## What to do next

1. **Watch the queue drain** — visit the PM UI's Auto-Fix Queue panel or
   call `get_auto_fix_queue_status` to see in-flight / waiting counts.
2. **Spot-check infrastructure items** — TMAIL-11/12/13/15/16/40 etc.
   will likely produce a runbook + open questions rather than a PR.
   Review their PM comments and decide which to close vs. retain.
3. **Handle the duplicate** — once TMAIL-163 or TMAIL-159 ships, mark
   the other as Duplicate.
4. **Manual credential inserts** — for TMAIL-46/159/163, the auto-fixer
   produces code/SQL; you'll still need to run the credential INSERT
   yourself against the prod DB (per the no-live-financials rule).

— generated by Claude Opus 4.7 via `tascim-pm` MCP, 2026-05-27.
