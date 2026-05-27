# AI Subsystem Assessment — May 2026

**Ticket:** TMAIL-249 (axis of TMAIL-241 backend modularisation review)
**Scope:** `services/ai_client.rs`, `services/embedding_service.rs`,
`services/ollama_client.rs`, `services/nlp_parser.rs`,
`handlers/ai_config.rs`, `handlers/semantic_search.rs`,
`handlers/nlp_search.rs`, `handlers/ollama.rs`,
`models/ai_config.rs`, `models/email_embedding.rs`,
`models/nlp_search.rs`, migrations `032_ai_providers.sql`,
`036_semantic_search.sql`, `047_ollama_config.sql`,
`070_email_summary_cache.sql`. Frontend AI surface was NOT in
scope for this pass.
**Method:** Static read of every in-scope file. No live calls to
OpenAI / Anthropic / Google / Ollama were made. Existing
`#[cfg(test)]` modules were sampled but not re-run.

---

## TL;DR

The AI plane works end-to-end as a competent BYOK shim — AES-256-GCM
at rest, JWT-derived encryption key, Redis-backed per-user rate
limit (TMAIL-102), Postgres-backed summary cache (TMAIL-103),
pgvector semantic search (TMAIL-106) — but it was built feature by
feature without a unifying client abstraction, and the seams show.

There are **four P0 findings** worth fixing before scale or marketing:

1. **AI rate limit is enforced on summarize / smart-reply / thread /
   compose / test, but NOT on `/api/search/semantic`,
   `/api/search/index`, or `/api/search/nlp`.** All three hit the
   same OpenAI/Google/Ollama embedding+chat APIs and burn the same
   BYOK quota. A SPA `useEffect` indexing the inbox or a tight
   semantic-search loop can drain the user's provider budget in
   seconds with no 429.
2. **Anthropic embeddings silently fall back to OpenAI with the
   Anthropic key.** `embedding_service.rs:99-108` rewrites the
   request to `api.openai.com/v1/embeddings` but keeps the
   Anthropic API key in the `Authorization: Bearer` header.
   Guaranteed 401, AND leaks the Anthropic key to OpenAI's logs.
3. **NLP search endpoint is a stub** — `handlers/nlp_search.rs:54-59`
   builds the IMAP search criteria from the AI-parsed query, then
   discards it and returns `results: []`. The endpoint is wired
   into the router and advertised in `CLAUDE.md` but never actually
   searches IMAP.
4. **Embedding dimension hardcoded to `vector(1536)`** in migration
   036. Users who pick Google `text-embedding-004` (768d),
   Ollama `nomic-embed-text` (768d), or OpenAI
   `text-embedding-3-large` (3072d) will hit a Postgres dimension
   mismatch on every `INSERT INTO email_embeddings`. No handler-level
   validation; failure mode is a raw sqlx error to the SPA.

Two structural concerns:

- **There is no `AiClient` trait or provider registry.** Each
  operation lives as a free function with 4-5 `match arm` switches
  on `AiProvider`. Adding a sixth provider edits ~8 spots in two
  files. Violates the "data-driven configuration over hardcoded
  logic" rule.
- **Every AI call is buffered, never streamed.** Ollama is
  hardcoded `"stream": false`; OpenAI/Anthropic/Google `stream`
  field is not set (defaults to false). For 1-3 paragraph
  compose/smart-reply the user waits 5-30 seconds on a black
  spinner before any token appears. Every modern provider
  (including local Ollama) supports SSE/chunked streaming.

Beyond those, the IVFFLAT index will need re-tuning past ~100K
embeddings (HNSW would be set-and-forget), the NLP parser embeds a
**stale literal "today's date (2026-04-14)"** in its system prompt,
embedding writes are single-row only (no batch endpoint to amortise
the AI round-trip), token usage is discarded so we can't bill or
detect prompt-injection cost spikes, and `reqwest::Client` is
constructed per request so every call pays the TLS handshake.

---

## What was checked

| Axis | Result |
|---|---|
| BYOK provider registry: enum + factory vs hardcoded `match` | ❌ Hardcoded — see #1 |
| Embedding writes batched | ❌ Single-row only — see #2 |
| Embedding index type (HNSW vs IVFFLAT) | ⚠️ IVFFLAT lists=100 — see #3 |
| Embedding dimension flexibility | ❌ Hardcoded 1536 — see #4 |
| LLM streaming responses | ❌ All buffered — see #5 |
| Per-user / per-provider rate limit | ⚠️ Inconsistent — see #6 |
| Shared `AiClient` trait | ❌ Free functions — see #1 |
| Graceful degrade on AI failure | ❌ Hard 4xx — see #7 |
| Anthropic embedding fallback safety | ❌ Silent key-mistargeting — see #8 |
| NLP search returns actual results | ❌ Stub — returns `[]` — see #9 |
| NLP parser "today" anchor | ❌ Hardcoded literal `2026-04-14` — see #10 |
| AI usage / token cost telemetry | ❌ Discarded — see #11 |
| `reqwest::Client` reuse | ❌ Per-call rebuild — see #12 |
| `base_url` trailing-slash normalisation | ❌ Concatenated raw — see #13 |
| Timeout per provider/model | ❌ Hardcoded 30s — see #14 |
| AES-256-GCM key derivation cached | ✅ Derived once per call from `state.config.jwt.secret` (acceptable, SHA-256 is cheap) |
| RLS on `ai_configurations` + `email_embeddings` | ✅ Both tables `ENABLE ROW LEVEL SECURITY` + `app.current_user_id` policy |
| Ollama admin endpoints gated on `is_admin` | ✅ All five `handlers/ollama.rs` actions call `require_admin` (TMAIL-210) |
| API key encrypted at rest | ✅ AES-256-GCM, JWT-secret-derived key, base64(nonce||cipher) |
| Summary cache invalidation | ✅ Cache key includes `hash_body` of the message body — edits auto-invalidate |
| Compose response parser fallback | ✅ `parse_compose_response` handles missing markers gracefully |

---

## Findings

### #1 — No `AiClient` trait, hardcoded provider `match` across two files (P1 structural)

**Where:** `services/ai_client.rs:95-114, 128-194, 198-228`, `services/embedding_service.rs:63-110, 113-152`, `models/ai_config.rs:19-30`.

**What:** Every operation is a free function that switches on
`AiProvider` for URL construction, request body shape, auth-header
selection, and response extraction. There are at minimum **six**
parallel `match arm` blocks for the same five providers:

| Concern | File:fn |
|---|---|
| Chat URL | `ai_client.rs::build_api_url` |
| Chat request body | `ai_client.rs::format_*_request` (4 fns) |
| Chat auth header | `ai_client.rs::call_ai_provider` inline match |
| Chat response extract | `ai_client.rs::extract_response_text` |
| Embedding URL+body | `embedding_service.rs::build_embedding_request` |
| Embedding response extract | `embedding_service.rs::extract_embedding_vector` |

Adding a sixth provider (e.g. Cohere, Mistral, Azure OpenAI,
DeepSeek) requires touching all six plus the `AiProvider` enum,
the `ai_provider` Postgres ENUM in migration 032, the SPA's
`AiProvider` TypeScript union, and the `AiConfigurationManager.tsx`
provider dropdown. None of this is hidden behind a registry.

**Fix sketch:** Define `pub trait AiClient { fn build_chat_url(...) -> String; fn build_chat_body(...) -> Value; fn auth_headers(...) -> Vec<(&str, String)>; fn parse_chat_response(v: &Value) -> Result<String>; fn build_embed_url(...); ... }` with one impl-block per provider. Move the enum to a registry-keyed `HashMap<AiProvider, Box<dyn AiClient>>` constructed once at `AppState` init. Adding a provider becomes one new file + one registry entry.

**Why:** Mirrors the `PaymentProviderConfig::resolve` lesson from
TMAIL-250: hardcoded switches across many files become a tax on
every future BYOK addition.

### #2 — Embedding writes are single-row only (P1 scale)

**Where:** `handlers/semantic_search.rs:66-124`, `models/email_embedding.rs::upsert`.

**What:** `POST /api/search/index` accepts one `{folder, uid,
subject, text}` per request. To index a 10,000-message mailbox the
SPA must make 10,000 round-trips, each running: AI config lookup
→ AES decrypt → external embedding API → pgvector INSERT. Even on
local Ollama that's 10K serialised HTTP calls. On OpenAI it also
burns 10K request-side rate-limit slots that could have fit ~5
batches of 2,048.

OpenAI's embeddings endpoint accepts `input: string[]` with up to
2,048 elements per request. Google `embedContent` has a
`batchEmbedContents` variant. Ollama 0.5+ accepts `input: string[]`
too. The current code calls the singular form every time.

**Fix sketch:** Add `POST /api/search/index/batch` that accepts
`{ items: [{folder, uid, subject, text}, ...] }`, splits into
provider-appropriate batch sizes (2048 for OpenAI, 100 for Google,
64 for Ollama by default), and does one `INSERT ... VALUES (...), (...), (...)`
per batch. Track per-user "background indexing" progress in a new
table or in Redis so the SPA can show a progress bar.

### #3 — pgvector index is IVFFLAT lists=100, will need re-tuning past 100K rows (P2)

**Where:** `migrations/036_semantic_search.sql:25`.

```sql
CREATE INDEX idx_email_embeddings_vector ON email_embeddings
  USING ivfflat (embedding vector_cosine_ops) WITH (lists = 100);
```

**What:** IVFFLAT's `lists` parameter is a one-shot decision at
index-creation time. Rule of thumb is `lists ≈ rows/1000` for
≤1M rows, then `≈ sqrt(rows)` past that. With `lists=100` the
index is well-tuned for ~100K rows. Past that, recall and latency
degrade until you `REINDEX` with a higher `lists`. The index
also needs `ANALYZE` after every large insert to update the
clustering — there's no documented runbook for this.

**Alternative:** HNSW (available since pgvector 0.5.0, mid-2023)
has no `lists` knob, no ANALYZE-after-insert requirement, and
generally faster recall at the cost of slightly slower build
time. Postgres 16 + pgvector ≥0.5 is the target stack — HNSW is
available. The pgvector README now treats HNSW as the default
recommendation.

**Fix sketch:** Migration to swap to HNSW:

```sql
DROP INDEX IF EXISTS idx_email_embeddings_vector;
CREATE INDEX idx_email_embeddings_vector ON email_embeddings
  USING hnsw (embedding vector_cosine_ops);
```

Document the migration in `MIGRATIONS-RUNBOOK` or similar — HNSW
build can take 5-30 min on a 1M-row table, so it's a maintenance
window operation.

### #4 — Embedding dimension hardcoded to 1536 (P0 correctness)

**Where:** `migrations/036_semantic_search.sql:18`,
`models/email_embedding.rs:3-4, 117`.

**What:** The column is declared `embedding vector(1536)` to match
OpenAI's `text-embedding-3-small`. The model name is user-selectable
in `ai_configurations.model_name`. If the user picks any of:

| Provider | Model | Dimension |
|---|---|---|
| Google | `text-embedding-004` | 768 |
| Google | `gemini-embedding-001` | 3072 |
| Ollama | `nomic-embed-text` | 768 |
| Ollama | `mxbai-embed-large` | 1024 |
| OpenAI | `text-embedding-3-large` | 3072 |

...then `EmailEmbedding::upsert` will pass a `vector(768)` or
`vector(3072)` literal into a `vector(1536)` column and Postgres
returns `expected 1536 dimensions, not 768` — surfaced to the SPA
as an opaque 500.

**Fix sketch:** Either (a) drop the dimension constraint and store
each row's dimension in `embedding_dim INTEGER NOT NULL`, build
separate ANN indexes per-dimension (HNSW supports this); or
(b) gate the model dropdown to only models that produce 1536d for
this iteration and document the constraint in `AiConfigurationManager.tsx`
+ `MODELS.md`. Option (a) is correct long-term; (b) buys time.

Until either lands, `semantic_search` / `index_email` should
**validate** at the handler level: hard-fail with a 400 if the
generated embedding is not 1536 dimensions, with a message that
names the active model and the dimension mismatch.

### #5 — All AI calls buffered, never streamed (P1 UX)

**Where:** `services/ai_client.rs:85` (Ollama explicit
`"stream": false`), `format_openai_request`, `format_anthropic_request`,
`format_google_request` all omit `stream` (defaults false).
Handlers return `Json<...>` so the response is fully materialised
in-memory before flushing.

**What:** A `gpt-4o-mini` compose for a long thread can take
5-15 seconds. A local 7B Ollama on a laptop CPU can take 30-60s.
The user sees a spinner until the entire response arrives. There
is no token-by-token streaming surface today.

**Fix sketch:** Two-step:

1. Add `services/ai_stream.rs` that uses
   `reqwest::Response::bytes_stream()` and parses provider-specific
   SSE / chunked-JSON formats (OpenAI `data: {...}\n\n`, Anthropic
   `event: content_block_delta`, Google
   `:`-delimited JSON, Ollama line-delimited JSON).
2. Add `POST /api/ai/smart-reply/stream`,
   `POST /api/ai/compose/stream`, `POST /api/ai/summarize/stream`
   that return `axum::response::sse::Sse<Stream<Item = ...>>`.
   Keep the existing non-streaming routes for backward compat.

The SPA's React 19 `useTransition` + `EventSource` consumer is a
straightforward swap.

### #6 — AI rate limit inconsistently applied (P0 abuse vector)

**Where:** `handlers/ai_config.rs:139, 188, 292, 367, 506` (all
gated) vs `handlers/semantic_search.rs:23-61, 66-124` and
`handlers/nlp_search.rs:23-81` (all UN-gated).

**What:** `CacheService::check_ai_rate_limit` correctly caps users
at 10 AI calls per 60s — but only on the five handlers in
`ai_config.rs`. The semantic-search index/query path and the NLP
parsing path both call the same external APIs (`generate_embedding`
hits OpenAI/Google/Ollama; `nlp_parser::parse_natural_query` calls
`ai_client::call_ai_provider`) and would each cost the BYOK user
real money — yet they bypass the gate completely.

**Concrete attack:** A misbehaving SPA `useEffect` (or a hostile
client) hitting `POST /api/search/index` in a tight loop indexes
every UID in the mailbox. On OpenAI at the embeddings price that's
$0.02 per 1M tokens × the entire mailbox size. The 429 budget
that protects the rest of the AI surface doesn't fire.

**Fix sketch:** Add `enforce_ai_rate_limit(&state, user_id).await?`
as the first line of `semantic_search`, `index_email`, and
`nlp_search`. (Don't gate `index_stats` — that's a pure DB read.)
Consider moving `enforce_ai_rate_limit` from `handlers/ai_config.rs`
to a `middleware::ai_rate_limit` layer applied to all `/api/ai/*`,
`/api/search/semantic`, `/api/search/index`, `/api/search/nlp`
routes via a typed-router merge, so future AI endpoints can't
forget the gate.

### #7 — No graceful degrade on AI failure (P2 UX)

**Where:** `handlers/ai_config.rs:240, 345, 455, 539`,
`handlers/semantic_search.rs:54, 100`.

**What:** Every AI-call failure is mapped to
`AppError::BadRequest(format!("AI summarization failed: {}", err))`
and surfaced as 400 to the SPA. There's no fallback (e.g. "your
provider is down, here are the first 500 chars of the email" or
"semantic search unavailable, falling back to keyword search").
The summary cache (TMAIL-103) partially mitigates re-reads, but
first-read failures are terminal.

**Fix sketch:** In the SPA-facing error envelope, classify the
failure as `transient | provider_auth | provider_quota | model_404 | unknown`
so the SPA can show actionable copy ("retry", "fix your key",
"upgrade your plan", "contact support"). Pair with #14 — a
proper error type lets us discriminate these cases.

### #8 — Anthropic embedding silently falls back to OpenAI with Anthropic key (P0 security)

**Where:** `services/embedding_service.rs:99-108`.

```rust
AiProvider::Anthropic => {
    // NOTE: Anthropic does not offer a native embedding API; fall back to OpenAI-compatible format
    let base = base_url.unwrap_or("https://api.openai.com/v1");
    let url = format!("{}/embeddings", base);
    let body = serde_json::json!({
        "model": "text-embedding-3-small",
        "input": text
    });
    (url, body, Some(("Authorization", format!("Bearer {}", api_key))))
}
```

**What:** The "fallback" rewrites the URL to `api.openai.com` but
keeps the Anthropic key in the `Authorization: Bearer` header.
The Anthropic key never authenticates against OpenAI — it will
401, AND the Anthropic key (`sk-ant-...`) is now in OpenAI's
access logs / proxy traces, which is **a credential leak to a
third party**.

Even if the user intended "use my OpenAI key for embeddings even
though I picked Anthropic for chat", the current code uses the
Anthropic key. There's no path to making this work as advertised.

**Fix sketch:** Hard-fail in `build_embedding_request` for
`AiProvider::Anthropic` with an error message that says:
"Anthropic does not offer a native embeddings API. To use
semantic search, configure a separate AI provider
(OpenAI / Google / Ollama / Custom) for embeddings — go to
Settings → AI Config and add a second provider." Surface this as
a 400 from `/api/search/semantic` and `/api/search/index`.

Even better: split the `AiConfiguration` row's responsibilities so
a user can mark one provider for chat and a different one for
embeddings. The schema already supports multiple rows per user.

### #9 — NLP search endpoint is a stub, never queries IMAP (P0 functional)

**Where:** `handlers/nlp_search.rs:54-59`.

```rust
let _imap_search = nlp_parser::build_imap_search(&parsed_params);

// NOTE: In a full implementation, we would execute the IMAP search here via imap_service.
// For now, return the parsed parameters so the frontend can display what the AI understood.
// The IMAP search execution will be connected when IMAP service supports programmatic search.
let results: Vec<NlpSearchResultItem> = Vec::new();
let result_count = results.len() as i32;
```

**What:** `POST /api/search/nlp` is wired into the router, listed
in `CLAUDE.md` API Route Structure as
"Natural-language search", and the SPA's NLP search component
calls it — but the endpoint always returns
`{ results: [], result_count: 0, parsed_params: {...} }`. The user
sees "0 results found" no matter what they search for.

`nlp_parser::build_imap_search` is fully implemented and tested
(15 unit tests), so the IMAP search command is constructed — just
never sent. The blocker is the comment claims `imap_service`
doesn't support programmatic search, but `services/imap_service.rs`
does have search-capable methods (used by other handlers).

**Fix sketch:** Wire `imap_service::search_with_criteria(folder, criteria)`
(or build it if missing) into `nlp_search`. The `folder` field of
`ParsedSearchParams` selects the mailbox; the IMAP search string
selects the messages; results map into `NlpSearchResultItem`.
Honor the existing `enforce_ai_rate_limit` (per #6 above).

Until this is wired up, **the endpoint should be hidden from the
SPA UI** — shipping "natural language search" that returns 0
results every time is worse than not shipping it.

### #10 — NLP parser system prompt embeds a literal stale date (P1 correctness)

**Where:** `services/nlp_parser.rs:28`.

```
- For relative dates like "last week", "yesterday", "this month",
  calculate from today's date (2026-04-14)
```

**What:** The string `2026-04-14` is hardcoded in the system
prompt. As of this assessment (2026-05-27) every "last week" /
"yesterday" / "this month" query is being anchored to mid-April
instead of today. The bug doesn't fail any tests because
`format_ai_prompt`'s tests only assert the prompt contains
"YYYY-MM-DD" — not that it's current.

**Fix sketch:**

```rust
pub fn format_ai_prompt(query: &str) -> String {
    let today = chrono::Utc::now().format("%Y-%m-%d");
    format!(
        r#"...calculate from today's date ({today})...
Query: {query}"#,
        today = today,
        query = query,
    )
}
```

Add a regression test that uses a mocked clock or asserts the
prompt contains `chrono::Utc::now().format("%Y-%m-%d").to_string()`.

### #11 — Token usage discarded; no AI cost telemetry (P2 billing readiness)

**Where:** `services/ai_client.rs::extract_response_text` ignores
`response["usage"]` entirely. Same for `embedding_service.rs`.

**What:** Every chat/embedding response from OpenAI/Anthropic/
Google carries `usage: { prompt_tokens, completion_tokens, total_tokens }`.
Ollama doesn't, but Ollama is local so it doesn't matter for
billing. We currently discard this.

**Consequences:**

- TASMail can't bill AI usage to the BYOK user (their bill is
  with OpenAI/Anthropic — we have no visibility), but ALSO can't
  show them in-app what their last 30 days of TASMail-AI usage
  cost.
- A prompt-injection that gets the model to enter a 4000-token
  reply burns the user's budget silently — we have no signal.
- No way to set a per-user monthly cap (a frequent enterprise ask).
- Compare with `migrations/058_usage_billing.sql` which already
  has the `billing_invoices.usage_amount` shape and would absorb
  AI cost rollups trivially.

**Fix sketch:** Add `ai_usage_log` table (`user_id`, `provider`,
`model`, `endpoint`, `prompt_tokens`, `completion_tokens`,
`occurred_at`). Update `call_ai_provider` to return
`(String, Option<Usage>)` and have handlers persist on success.
Aggregate by month into a `billing_invoices`-shaped output or
into a separate `ai_usage_invoices` table. Surface to the user
in Settings → AI Config → Usage tab.

### #12 — `reqwest::Client` rebuilt per call (P2 latency)

**Where:** `services/ai_client.rs:146`,
`services/embedding_service.rs:24`, `services/ollama_client.rs:26`
and the other helper fns in the same file.

**What:** Every AI call constructs a fresh
`reqwest::Client::builder().timeout(30s).build()?`. No connection
pooling, no TLS session resumption, no keep-alive. For local
Ollama this costs ~1ms; for OpenAI/Anthropic where the TLS
handshake is 50-200ms, every request pays that latency tax
again. At 10 calls/min/user × 1000 active users that's an extra
50-200s of cumulative wall-clock per minute, much of which is the
SPA spinner.

**Fix sketch:** Add `ai_client: Arc<reqwest::Client>` to
`AppState`, constructed once with `Client::builder().pool_idle_timeout(Duration::from_secs(90)).build()`.
Pass `&state.ai_client` (or `state.ai_client.clone()`) into every
AI call. Pair with #14 (configurable timeouts).

### #13 — `base_url` concatenation is not slash-normalised (P3 hygiene)

**Where:** `services/ai_client.rs::build_api_url` — all four
branches do `format!("{}/chat/completions", base)`. If the user
puts `https://my-proxy.example.com/v1/` (trailing slash) in
`ai_configurations.base_url`, the URL becomes
`https://my-proxy.example.com/v1//chat/completions`.

**Fix sketch:** Normalise once at config-write time, OR follow
`ollama_client.rs:25` and use `base_url.trim_end_matches('/')` at
every concat site.

### #14 — Timeout hardcoded to 30s everywhere (P2)

**Where:** `services/ai_client.rs:147`,
`services/embedding_service.rs:25`, `services/ollama_client.rs:27/61/106/155`.

**What:** A `gpt-4o` compose for a long thread can exceed 30s;
a local 7B Ollama on a slow CPU often does. `pull_model` rightly
uses 600s but the inference calls do not.

**Fix sketch:** Add `request_timeout_secs INTEGER NOT NULL DEFAULT 30`
to `ai_configurations`. Default to 30 for OpenAI/Anthropic/Google,
120 for Ollama, configurable per-row. Surface in
`AiConfigurationManager.tsx` for power users.

### #15 — No structured error type for AI failures (P2)

**Where:** All public AI functions return `Result<_, String>`.

**What:** Every failure mode collapses to a `String`. The handlers
then wrap as `AppError::BadRequest(format!("..."))` which becomes
a generic 400 to the SPA. The SPA has no way to distinguish
"your key is wrong" from "the provider is rate-limiting you" from
"the model name you picked doesn't exist anymore". This blocks #7
(graceful degrade).

**Fix sketch:**

```rust
pub enum AiError {
    Auth(String),         // 401/403 from provider
    Quota(String),        // 429 from provider
    Timeout,              // tokio::time::error::Elapsed
    Network(String),      // reqwest::Error
    ModelNotFound(String),// 404 / specific provider error
    Parse(String),        // response shape unexpected
    Other(String),
}
impl From<AiError> for AppError { ... }  // each variant -> right HTTP status
```

Map provider response codes / error JSON to the right variant in
`call_ai_provider` / `generate_embedding`. SPA can then key off
the typed error to render targeted help.

### #16 — Cache-persist failures observable only via tracing::warn (P3)

**Where:** `handlers/ai_config.rs:262-270, 477-484`.

**What:** When the summary-cache `upsert` fails post-AI-call, the
code correctly returns the result anyway and emits `tracing::warn!`.
But there's no metric (Prometheus counter or histogram) to alert on
"cache write failure rate spiked" — the only signal is reading
production logs.

**Fix sketch:** Add `ai_cache_write_failures_total` Counter (or
similar) to the existing Prometheus exposition. Same for #15's
typed errors (`ai_provider_calls_total{provider,outcome}`).

### #17 — Anthropic API version hardcoded (P3)

**Where:** `services/ai_client.rs:162` — `anthropic-version: 2023-06-01`.

**What:** Anthropic versions their API via a header. Locking
ourselves to 2023-06-01 means we miss any deprecation/feature
bumps until the value is edited in source. Should be a config
constant (or a per-provider header map in the future
`AiClient` trait per #1).

### #18 — `test_ai_config` counts against the user's inference budget (P3 design)

**Where:** `handlers/ai_config.rs:137-139`.

```rust
// Connection-test still calls the provider, so it counts
// against the 10/min/user AI budget like any other inference call.
enforce_ai_rate_limit(&state, user_id).await?;
```

**What:** Documented and intentional, but worth flagging. A user
trying to wire up a new provider can burn their 10/min budget on
config tests alone, then can't actually use AI for the rest of
the minute. Mitigation could be a separate, very-low-cap budget
for `/api/ai/config/*/test` so config testing doesn't share a
counter with inference.

---

## Cross-cutting recommendations

In priority order, here's what to do if you only have time for a
short slice of this list:

1. **Fix #8 (Anthropic embedding key leak)** — credential exposure,
   one-line fix, ship today.
2. **Fix #6 (rate-limit gaps)** — three handler edits, prevents
   abuse, ship this sprint.
3. **Fix #4 (embedding dimension)** — at minimum validate at the
   handler level so the failure is a 400 with a useful message,
   not an opaque 500.
4. **Fix #9 (NLP search stub)** — either wire it up to IMAP or
   hide it from the UI. Don't ship a "0 results, always" endpoint.
5. **Fix #10 (stale date in NLP prompt)** — five-line fix, removes
   a category of wrong answers.
6. **Add streaming (#5)** — biggest single UX win, mid-effort.
7. **Add the `AiClient` trait (#1)** — pre-requisite for ever
   adding Cohere / Mistral / Azure / DeepSeek without pain.
8. **Add `Arc<reqwest::Client>` reuse (#12)** — quiet latency win.
9. **Add token usage telemetry (#11)** — unblocks per-user AI billing.
10. **Migrate IVFFLAT → HNSW (#3)** — only urgent once any tenant
    crosses ~100K indexed emails.

The remaining findings (typed errors, timeouts, slash-normalisation,
Anthropic version header, test-config budget) are quality-of-life
and can ride along with the structural refactor in #1.

---

## Files reviewed

- `backend/src/services/ai_client.rs` (648 lines, 27 unit tests)
- `backend/src/services/embedding_service.rs` (373 lines, 13 unit tests)
- `backend/src/services/ollama_client.rs` (285 lines, 11 unit tests)
- `backend/src/services/nlp_parser.rs` (381 lines, 18 unit tests)
- `backend/src/services/cache_service.rs` (506 lines, AI rate-limit excerpt)
- `backend/src/handlers/ai_config.rs` (716 lines, 15 unit tests)
- `backend/src/handlers/semantic_search.rs` (206 lines, 4 unit tests)
- `backend/src/handlers/nlp_search.rs` (185 lines, 5 unit tests)
- `backend/src/handlers/ollama.rs` (`require_admin` gating only — full module not read)
- `backend/src/models/ai_config.rs` (742 lines, 32 unit tests)
- `backend/src/models/email_embedding.rs` (313 lines, 10 unit tests)
- `backend/src/router.rs` (AI route blocks at lines 563-651, 858-874)
- `backend/migrations/032_ai_providers.sql` (24 lines)
- `backend/migrations/036_semantic_search.sql` (34 lines)
- `backend/migrations/070_email_summary_cache.sql` (referenced, not opened)
