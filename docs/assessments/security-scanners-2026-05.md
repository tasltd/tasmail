# Mail-Security Scanners Assessment — May 2026

**Ticket:** TMAIL-248 (axis of TMAIL-241 backend modularisation review)
**Scope:** `backend/src/services/phishing_scanner.rs`,
`backend/src/services/dlp_scanner.rs`,
`backend/src/services/rspamd_client.rs`,
`backend/src/handlers/{phishing,dlp,spam}.rs`,
`backend/src/bin/dlp_milter.rs`, plus the migrations that back them
(`020_phishing_reports.sql`, `037_dlp_rules.sql`, `050_rspamd.sql`,
`068_phishing_dangerous_attachments.sql`).
**Method:** static read of every file above, the route registrations in
`router.rs`, the `phishing_report` model, the rspamd_client call-graph,
and the milter EOM path.

---

## TL;DR

The three scanners work, but they were built one-at-a-time and have never
been refactored as a unified subsystem. The result is three independent
verdict shapes, a heavy regex/connection cost per message, no cross-user
URL caching, and a milter that re-reads the rule set from Postgres on
every single outbound message.

Top findings (ordered by impact):

1. **No shared `ScanVerdict` enum** — phishing returns `risk_score:i32`,
   DLP returns `Vec<DlpScanMatch>` with `DlpAction`, rspamd returns
   `action: String`. Three vocabularies. No common gate. Composing
   "phishing AND DLP AND spam say block" requires three string/score
   comparisons in every consumer. **TMAIL-241 axis.**
2. **DLP milter loads ALL active rules from Postgres on EVERY message**
   (`bin/dlp_milter.rs:298`). Each rule's regex is then **recompiled per
   message** (`dlp_scanner.rs:122`). At 10k msgs/hr with 50 active rules
   that is 500k regex compiles/hr. No in-memory rule cache, no
   compiled-regex cache, no TTL.
3. **`RspamdClient` builds a fresh `reqwest::Client` on every call**
   (`rspamd_client.rs:56`). No connection pool, no TCP keep-alive reuse.
   Each `check_message` / `learn_*` opens a new HTTPS connection to
   rspamd. The `Client` should be created once in `::new()` and cloned
   (it is internally `Arc`-shared).
4. **Phishing scanner has no URL-verdict cache** — the same shortener,
   IP-URL or homograph domain is re-checked for every recipient. The
   per-message verdict IS cached in `phishing_reports`, but the per-URL
   verdict (the expensive piece if we ever add WebFetch reputation
   lookups in TMAIL-124) has nowhere to live.
5. **The phishing scanner's check chain is a hardcoded if/else cascade**
   inside `analyze_url_reasons()` (lines 178–187). Adding a 7th check
   (e.g. typosquatting, punycode) means editing the function rather
   than registering a rule. Same story for the brand list, TLD list,
   and shortener list — all `const &[&str]` literals.
6. **`check_message` is never wired** — `handlers/spam.rs::learn_message`
   constructs an `RspamdClient` then drops it without calling
   `check_message` or `learn_spam` (line 92, prefixed with `_client` so
   the compiler stays quiet). The handler logs the request and returns
   200. No outbound rspamd integration exists in the active code path;
   only the milter (which is **disabled** on `mail.techatscale.io` per
   the BYOK pivot).
7. **No per-rule timing budget** — neither the DLP milter nor the
   phishing scanner records per-rule execution time. The `regex` crate's
   linear-time guarantee makes this safe by construction, but operators
   have no way to see which rule is the slowest under load.
8. **DLP milter is BLOCKING by design** — correct, because Postfix waits
   for the verdict on the milter socket. But there is no overall EOM
   timeout in the indymilter callback, and Postfix's
   `milter_default_action = accept` (fail-open) is the only safety net
   when the DB lookup times out.

---

## What was checked

| Check | Result |
|---|---|
| Scan chain is registry vs hardcoded if-else | ⚠️ Phishing = hardcoded cascade; DLP = DB registry; Rspamd = external (registry inside rspamd config) |
| DLP milter blocking vs async, per-rule timing | ⚠️ Blocking (correct) but **no per-rule budget**, no EOM timeout |
| Rspamd connection pooled, bypass on timeout | ❌ **New `reqwest::Client` per call**, no bypass — caller gets `Err` |
| Phishing link scanner caches verdicts cross-user | ❌ Per-message DB cache only; no per-URL cache |
| All three scanners share `ScanVerdict` enum | ❌ Three independent verdict shapes (i32 / Vec\<DlpScanMatch\> / String action) |
| Rules / patterns extensible at runtime | ⚠️ DLP = yes (DB); phishing/rspamd-symbols = no (const slices / rspamd.conf) |
| Pre-compiled regexes via `LazyLock` | ⚠️ Mostly yes; `strip_html_tags()` re-compiles `<[^>]+>` per call |
| Built-in DLP patterns iterated efficiently | ⚠️ `get_builtin_patterns()` reallocates the Vec on every scan |
| Catastrophic-backtracking safety | ✅ `regex` crate is linear-time by construction |
| Outbound spam check (`check_message`) wired | ❌ Never called; `learn_message` handler logs only |
| Milter `fail_open` semantics match Postfix default | ✅ `TASMAIL_DLP_FAIL_OPEN=true` → `Status::Continue` on DB error |
| `dlp_violations` persisted with FK safety | ✅ Built-in / attachment matches skip persistence (`rule_id = nil`) |
| Phishing report stores `dangerous_attachments` (TMAIL-068) | ✅ JSONB column + serde default for legacy rows |
| `mailboxes` lookup in milter cached | ❌ One `SELECT id FROM mailboxes WHERE LOWER(email) = ...` per violation |

---

## Findings

### 1. No shared `ScanVerdict` enum (P0 — TMAIL-241 axis)

The three scanners speak three different languages:

| Scanner | Return shape | "Block" signal |
|---|---|---|
| `phishing_scanner::scan_email_with_attachments` | `ScanResult { risk_score: i32 (0-100), suspicious_links, ... }` | `risk_score >= 60` (only asserted in tests, not enforced anywhere) |
| `dlp_scanner::scan_content` + `scan_attachments` | `Vec<DlpScanMatch>` each with `DlpAction::{Block,Quarantine,Warn,Log}` | `matches.iter().any(\|m\| m.action == DlpAction::Block)` |
| `rspamd_client::check_message` | `SpamCheckResult { score: f64, required_score: f64, action: String }` | `action == "reject"` (string compare) |

Consequences:

- The DLP milter builds its own `decide()` function (`bin/dlp_milter.rs:273`)
  to translate `DlpAction` to indymilter `Status`. The phishing reports
  table stores `risk_score` raw — no enum. The spam quarantine table
  stores `action_taken` as a string.
- The frontend has to know all three vocabularies to render unified
  "this message was flagged" UI in the message view.
- The pricing/quota subsystem cannot ask a single question like
  "did any scanner say BLOCK?" — it would have to know how to interpret
  each scanner's return type.

**Recommendation:** introduce `services::scan_verdict::ScanVerdict`:

```rust
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum ScanVerdict { Pass, Warn, Quarantine, Block }

pub trait Scanner {
    fn name(&self) -> &'static str;
    async fn scan(&self, input: &ScanInput) -> ScanReport;
}

pub struct ScanReport {
    pub verdict: ScanVerdict,
    pub confidence: u8,           // 0-100
    pub reasons: Vec<String>,
    pub details: serde_json::Value, // scanner-specific payload
}
```

Then the milter pipeline becomes `verdict.max(phishing).max(dlp).max(spam)`
and the frontend has one column to render.

Migration path: start with a thin shim — give each existing scanner an
`fn verdict(&self) -> ScanVerdict` helper that maps risk_score / DlpAction
/ rspamd action into the enum. Existing consumers keep working.

---

### 2. DLP rules + regexes recompiled per message (P0 — perf)

Three layers compound:

**Layer A — milter EOM** (`bin/dlp_milter.rs:229`):
```rust
let rules = match load_active_rules(&cfg.pool).await { ... };
```
This issues `SELECT * FROM dlp_rules WHERE active = true` on every
single outbound message. There is no in-memory cache. With 50 active
rules and 10k msgs/hr that is 10k DB roundtrips/hr plus 50 × 10k =
500k row materialisations.

**Layer B — per-rule regex compile** (`dlp_scanner.rs:122`):
```rust
if let Ok(regex) = Regex::new(&rule.pattern) {
    scan_with_regex(&regex, rule, subject, body, &mut matches);
}
```
Every rule's `pattern` string is recompiled from scratch on every
`scan_content()` call. The compile cost is small individually (~100 µs
for typical patterns) but pays a fixed allocation per scan.

**Layer C — built-in patterns vec reallocation** (`dlp_scanner.rs:146`):
```rust
for builtin in get_builtin_patterns() {
    scan_with_builtin_regex(&builtin, subject, body, &mut matches);
}
```
`get_builtin_patterns()` constructs a fresh `Vec<BuiltinPattern>` (with
3 entries today) on every scan. The regexes inside are already `LazyLock`
statics so they don't recompile, but the wrapper struct and Vec allocate
each time. Should be a `static BUILTIN_PATTERNS: LazyLock<Vec<...>>`.

**Recommendation:**

```rust
// In dlp_scanner.rs — keep an Arc<Vec<(DlpRule, Regex)>> behind RwLock,
// reloaded on rule create/update/delete via a pg_notify subscription
// or a short TTL (60s).
pub struct CompiledRuleSet {
    rules: Vec<(DlpRule, CompiledPattern)>,
    loaded_at: Instant,
}

enum CompiledPattern { Regex(Regex), Keyword(String), Dictionary(Vec<String>) }
```

Add an explicit cache invalidation call from the `handlers/dlp.rs`
`create_rule`/`update_rule`/`delete_rule` paths so admin edits take
effect immediately rather than waiting for TTL.

**Also fix:** `scan_with_regex` uses `regex.find()` (line 163, 178) —
**a body with 5 credit cards reports only 1 match**. Use `find_iter()`
and emit one `DlpScanMatch` per occurrence (capped at, say, 25 per
rule to bound memory).

---

### 3. `RspamdClient` builds a new `reqwest::Client` per call (P1 — perf)

```rust
// rspamd_client.rs:56
fn build_client(&self, timeout_secs: u64) -> Result<reqwest::Client, String> {
    reqwest::Client::builder()
        .timeout(...)
        .build()
        ...
}
```

Every call to `check_message` / `learn_spam` / `learn_ham` / `get_stat`
runs `Client::builder().build()` which allocates a fresh hyper connection
pool and a new TLS context (rspamd is usually HTTP over localhost, so
TLS doesn't apply — but the pool still has no chance to keep an open
TCP keep-alive across calls).

`reqwest::Client` is `Clone` and internally `Arc` — it's designed to be
built once and shared.

**Recommendation:**

```rust
pub struct RspamdClient {
    base_url: String,
    password: Option<String>,
    http: reqwest::Client,        // built once
}

impl RspamdClient {
    pub fn new(base_url: String, password: Option<String>) -> Self {
        let http = reqwest::Client::builder()
            .timeout(Duration::from_secs(30))
            .pool_max_idle_per_host(8)
            .build()
            .expect("reqwest client init must not fail");
        Self { base_url, password, http }
    }
}
```

Also add a `bypass_on_timeout: bool` so callers can opt into "if rspamd
is slow, treat as no-action" rather than failing the whole send. Today
a 30s rspamd hang stalls the calling handler for 30s before returning
500.

---

### 4. Phishing scanner has no cross-message URL cache (P1 — perf)

`phishing_scanner::scan_email_with_attachments` is pure-CPU heuristics
today, so the cost is small. **But** TMAIL-124 explicitly leaves the
door open for future external reputation lookups ("This is the first
pass — no API calls"). The moment we add Google Safe Browsing, PhishTank,
or VirusTotal URL lookups, every recipient of the same phishing campaign
will issue a separate API call for the same URL — quota burn and slow.

The existing `phishing_reports` table caches the **whole-message verdict**
(keyed on `mailbox_id + folder + message_uid`), so two users who both
receive the same campaign each pay the full scan cost. There is no
URL-level cache.

**Recommendation:** before TMAIL-124's external-reputation phase lands,
add a `phishing_url_verdicts` table:

```sql
CREATE TABLE phishing_url_verdicts (
    url_hash BYTEA PRIMARY KEY,           -- SHA-256(normalised_url)
    url TEXT NOT NULL,
    verdict TEXT NOT NULL,                -- 'safe' | 'suspicious' | 'malicious'
    reasons JSONB NOT NULL,
    source TEXT NOT NULL,                 -- 'heuristic' | 'safebrowsing' | 'phishtank' | ...
    scanned_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    expires_at TIMESTAMPTZ NOT NULL
);
CREATE INDEX ON phishing_url_verdicts (expires_at);
```

Plus a thin LRU in-memory cache (e.g. `moka`) in front of it for
hot URLs. Cross-user — no `mailbox_id` column on this table by design.

---

### 5. Phishing scan chain is hardcoded (P1 — extensibility)

```rust
// phishing_scanner.rs:178
fn analyze_url_reasons(href: &str, display_text: &str) -> Vec<String> {
    let mut reasons = Vec::new();
    check_display_mismatch(href, display_text, &mut reasons);
    check_suspicious_tld(href, &mut reasons);
    check_ip_url(href, &mut reasons);
    check_url_shortener(href, &mut reasons);
    check_homograph(href, &mut reasons);
    check_excessive_subdomains(href, &mut reasons);
    reasons
}
```

Adding a check (e.g. typosquatting against a whitelist of known brand
domains, or punycode detection) is a code change, not a config change.
Same for the data behind the checks:

```rust
const SUSPICIOUS_TLDS: &[&str] = &[".tk", ".ml", ".ga", ".cf", ".gq", ".xyz", ".top", ".buzz"];
const URL_SHORTENERS: &[&str] = &["bit.ly", "tinyurl.com", "t.co", ...];
const KNOWN_BRANDS: &[&str] = &["paypal", "apple", "google", ...];
const DANGEROUS_EXTENSIONS: &[&str] = &["exe", "bat", ...];
```

These are correct as defaults but should be **overridable per tenant**:
a finance firm might want to add `.zip` as a dangerous extension, a
brand might want to add itself to the spoofing list.

**Recommendation:** registry pattern, mirroring the DLP design:

```rust
pub trait UrlCheck: Send + Sync {
    fn name(&self) -> &'static str;
    fn run(&self, url: &str, display: &str) -> Option<String>;  // Some(reason) or None
}

pub struct UrlScanChain {
    checks: Vec<Box<dyn UrlCheck>>,
}
```

Then ship the six built-in checks as registry entries and let the
chain be extended by feeding in DB-backed checks (similar to DLP rules).
Also lift the TLD / shortener / brand lists into a `phishing_indicators`
table that operators can edit through the admin UI.

**Quick wins regardless of registry:**
- `strip_html_tags` (`phishing_scanner.rs:240`) builds a new `Regex`
  on every call. Move to `static STRIP_TAGS: LazyLock<Regex>`.
- Risk-score weights (15 per suspicious link, 25 for suspicious sender,
  30 per dangerous attachment) are magic numbers in `calculate_risk_score`.
  Hoist into `pub const PHISHING_SCORE_WEIGHTS: ...` so tuning doesn't
  require touching scanner logic.

---

### 6. `check_message` not wired into the production code path (P1 — gap)

`handlers/spam.rs:83-114` (`learn_message`):

```rust
let rspamd_url = state.config.rspamd_url.as_deref().unwrap_or("http://localhost:11333");
let _client = crate::services::rspamd_client::RspamdClient::new(
    rspamd_url.to_string(),
    state.config.rspamd_password.clone(),
);
// ...
tracing::info!(...);
Ok(StatusCode::OK)
```

The handler:
- Constructs an `RspamdClient` then prefixes it with `_` so the
  compiler doesn't complain about an unused variable.
- Logs the request.
- Returns `200 OK` without calling `learn_spam`, `learn_ham`, or
  `check_message`.

The comment on line 88 acknowledges this: *"In production, fetch the
raw email from IMAP using message_id + folder, then call
rspamd_client.learn_spam() or learn_ham() with the raw bytes."*

So the only thing that ever calls into rspamd is the DLP milter — and
that's disabled on the live `mail.techatscale.io` deployment per the
BYOK pivot (`docs/SELF-HOST-MAIL-SERVERS.md`).

**Status:** rspamd is effectively dead code on the BYOK path. The self-host
operators who run the DLP milter benefit from it indirectly via the
`X-Spam-*` headers Postfix writes. But the TASMail backend never calls
the rspamd API itself, the `/api/spam/learn` endpoint is a no-op, and
the stats panel in the admin UI shows what `SpamQuarantine::stats()`
counts in our own table — not what rspamd reports.

**Recommendation:** either (a) actually wire `learn_message` to fetch
the raw EML from IMAP and call `learn_spam`/`learn_ham`, or (b) mark
the endpoint deprecated and remove the `RspamdClient` allocation
from the handler. The current "log and pretend" state is dishonest to
the UI.

---

### 7. No per-rule timing budget, no EOM timeout in milter (P2 — observability)

The DLP milter's `evaluate_and_decide` (line 225) runs `scan_content`
and `scan_attachments` synchronously and produces a verdict. There is
no per-rule timing instrumentation, no overall EOM timeout, and no
backpressure if the DB pool is saturated.

Today this is fine because the `regex` crate is linear-time. If we ever
swap in PCRE2 for `regex`-incompatible patterns, or add an external
reputation lookup inside a DLP rule, a single slow rule will stall
every outbound message.

**Recommendation:**
- Add `tracing::info_span!` around each rule's scan with `rule_id` and
  `rule_name`. Emit duration on close.
- Add an overall EOM deadline (e.g. `tokio::time::timeout(5s, ...)`)
  that returns `Status::Continue` on miss when `fail_open=true` and
  `Status::Tempfail` otherwise.
- Cache the `mailboxes` lookup in `persist_violations` (currently a
  full lookup per violation, line 315).

---

### 8. Layout / location nits

- `DEFAULT_BLOCKED_EXTENSIONS` lives in `dlp_scanner.rs` but the
  phishing scanner has its own `DANGEROUS_EXTENSIONS` const with a
  largely overlapping list. Two sources of truth. Consolidate into a
  single `services::extension_blocklist` module that both consume.
- `DECEPTIVE_DOUBLE_EXTENSIONS` is phishing-only — that's fine, but
  worth a doc comment explaining why DLP doesn't care about double
  extensions (because DLP fires on the **last** extension regardless of
  intent, while phishing distinguishes "looks like a PDF but is an EXE").
- The phishing handler `handlers/phishing.rs:62-66` serialises
  `suspicious_links` and `dangerous_attachments` into JSONB via two
  separate `serde_json::to_value()` calls. Use the standard
  `sqlx::types::Json<T>` wrapper to skip the manual serialise step.
- `handlers/spam.rs` `get_settings`, `update_settings`, `list_quarantine`,
  `release_quarantine`, `delete_quarantine`, `learn_message` and
  `get_stats` all discard `claims` (prefixed `_claims`). None of them
  check `is_admin`, even though spam threshold tuning is clearly an
  admin-only operation. **Mirrors the ActiveSync gap flagged in the
  admin-surface assessment (`docs/assessments/admin-surface-2026-05.md`).**

---

## Prioritised follow-ups

| # | Item | Effort | Priority |
|---|---|---|---|
| 1 | Introduce `ScanVerdict` enum + shim conversions for all three scanners | M (1d) | P0 |
| 2 | Cache compiled DLP rule set in milter; invalidate on rule CRUD | M (1d) | P0 |
| 3 | Build `RspamdClient::http: reqwest::Client` once in `::new`, drop `build_client` | S (1h) | P1 |
| 4 | `scan_with_regex` → `find_iter` with cap; capture every match | S (2h) | P1 |
| 5 | Lift phishing's `strip_html_tags` regex into `LazyLock`, builtin_patterns into static | S (1h) | P1 |
| 6 | Wire `learn_message` to actually call rspamd or mark deprecated | M (4h) | P1 |
| 7 | Add `is_admin` gate to spam settings/quarantine handlers | S (2h) | P1 |
| 8 | Phishing URL-check registry + tenant-overridable indicator lists | L (2d) | P2 (blocks TMAIL-124 phase 2) |
| 9 | `phishing_url_verdicts` table + LRU for cross-user URL caching | M (1d) | P2 (blocks external reputation lookups) |
| 10 | Per-rule timing spans in DLP milter; overall EOM timeout | S (3h) | P2 |
| 11 | Consolidate `DEFAULT_BLOCKED_EXTENSIONS` and `DANGEROUS_EXTENSIONS` | S (1h) | P3 |

---

## Files audited

```
backend/src/services/phishing_scanner.rs    657 lines
backend/src/services/dlp_scanner.rs         544 lines
backend/src/services/rspamd_client.rs       344 lines
backend/src/handlers/phishing.rs            166 lines
backend/src/handlers/dlp.rs                 177 lines
backend/src/handlers/spam.rs                192 lines
backend/src/bin/dlp_milter.rs               420 lines
backend/src/models/phishing_report.rs       ~150 lines (relevant section)
backend/src/config.rs                       (rspamd_url, rspamd_password)
backend/migrations/020_phishing_reports.sql
backend/migrations/037_dlp_rules.sql
backend/migrations/050_rspamd.sql
backend/migrations/068_phishing_dangerous_attachments.sql
```

Total: ~2,500 lines of scanner code, none of it sharing a verdict
shape or a configuration registry.
