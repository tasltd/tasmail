# Search, Filters & Templates Assessment

- **Issue:** TMAIL-245 (axis of TMAIL-241)
- **Date:** 2026-05-29
- **Scope (backend):** `backend/src/handlers/messages.rs:240–277` (`GET /api/search`
  — the ticket called this `handlers/search.rs`, but it lives inside the
  messages handler), `backend/src/handlers/nlp_search.rs`,
  `backend/src/handlers/semantic_search.rs`,
  `backend/src/handlers/sieve.rs` (the ticket called this `handlers/filters.rs`),
  `backend/src/handlers/templates.rs`,
  `backend/src/services/imap_service.rs:305–384`,
  `backend/src/services/nlp_parser.rs`,
  `backend/src/services/embedding_service.rs`,
  `backend/src/models/sieve_rule.rs`,
  `backend/src/models/email_template.rs`,
  `backend/src/models/nlp_search.rs`,
  `backend/src/models/email_embedding.rs`,
  `backend/src/validation.rs:121–139` (search/folder validation),
  `backend/migrations/013_sieve_filter_rules.sql`,
  `backend/migrations/016_email_templates.sql`,
  `backend/migrations/036_semantic_search.sql`,
  `backend/migrations/040_nlp_search.sql`.
- **Scope (frontend):** `frontend/src/components/mail/SearchResults.tsx`,
  `frontend/src/components/mail/AdvancedSearch.tsx`,
  `frontend/src/components/mail/SemanticSearchPanel.tsx`,
  `frontend/src/components/mail/NlpSearchPanel.tsx`,
  `frontend/src/components/mail/NlpSearch.tsx`,
  `frontend/src/components/settings/FilterManager.tsx` (the ticket called this
  `FiltersManager`),
  `frontend/src/components/settings/TemplateManager.tsx` (the ticket called this
  `TemplatesManager`),
  `frontend/src/api/messages.ts:41–68` (search params serialiser),
  `frontend/src/api/filters.ts`, `frontend/src/api/templates.ts`,
  `frontend/src/hooks/useMailbox.ts:90–125` (search hooks),
  `themes/shadcn-prototype/src/api/messages.ts:42–67` (alt-UI search client).
- **Method:** Static read of every file in scope, plus a grep sweep for
  `matches_email`, `SieveRule::`, `EmailTemplate::render`,
  `AdvancedSearchParams`, `useVirtualizer`, `searchMessages`, `is_shared` to
  discover dead vs live code paths, registry-vs-hardcoded patterns, and
  cross-tenant scope. Migrations cross-checked against the model structs and
  RLS policies. No benchmark was captured — the IMAP/AI RTT figures cited are
  ballpark order-of-magnitude only.

---

## TL;DR — biggest wins, by ROI

| # | Finding | Severity | Effort | Suggested ticket |
|---|---------|----------|--------|------------------|
| 1 | **Advanced-search query parameters are silently dropped server-side.** `frontend/src/api/messages.ts:54–64` and the alt-UI's `themes/shadcn-prototype/src/api/messages.ts:53–64` both serialise 8 advanced filter params onto the URL (`from`, `to`, `subject`, `date_from`, `date_to`, `has_attachment`, `is_unread`, `is_starred`). The backend `SearchQuery` struct at `handlers/messages.rs:33–37` accepts ONLY `{ q, folder }` — every other key is dropped by `Query<SearchQuery>` deserialisation without error. The handler then runs a plain `TEXT "$q"` IMAP search regardless of the advanced filters. The "Advanced Search" panel (TMAIL-32) ships in the SPA, the chips render the active filters, the search button enables — and the backend ignores everything but `q`. **Users believe they are filtering by sender/date/flag; they are getting full-text-on-q results back, then probably narrowing by eyeball.** | **P0 — feature is a lie** | Medium — add an `AdvancedSearchQuery` struct, route the advanced params through `imap_service.search_advanced()` which composes IMAP SEARCH criteria (`FROM "x" SINCE "dd-mmm-yyyy" UNSEEN HEADER X-Has-Attachment ...`) instead of `TEXT`. Reuse `validation::validate_search_query` for each free-text field. Wire it under either `GET /api/search` (additive — `SearchQuery` becomes `Option<...>` for each field) or a sibling `POST /api/search/advanced` | New (P0; the SPA's flagship search UX is non-functional today) |
| 2 | **The NLP search handler runs the AI parse, builds the IMAP criterion, then explicitly does not execute it.** `handlers/nlp_search.rs:56–60` has the comment `// NOTE: In a full implementation, we would execute the IMAP search here via imap_service.` and assigns `let results: Vec<NlpSearchResultItem> = Vec::new();` — the search **always returns empty results**. Every NLP query the SPA submits costs an AI provider call (a paid token spend on Anthropic/OpenAI/Ollama-via-Cloud) and burns it on a known-empty response. The history row still gets written with `result_count = 0`. The frontend `NlpSearchPanel.tsx` renders the parsed-params card so the user thinks the AI understood — they just see "0 results" and assume their query was bad. | **P0 — feature is a lie + cost burn** | Medium — execute the `nlp_parser::build_imap_search(&parsed_params)` output via `imap_service.search_messages` (or the advanced variant added in finding 1). Re-use the result mapping from `handlers/messages.rs::search_messages`. Add a smoke test that the round-trip returns ≥1 row when a known matching message exists in INBOX | New (P0; pair with finding 1 — both are dropping params before they reach IMAP) |
| 3 | **The server-side Sieve rule engine is unreachable.** `models/sieve_rule.rs:218–246` implements `matches_email(from, to, subject, headers)` with proper `all`/`any` matching — but `grep -rn "matches_email\|SieveRule::" backend/src/` shows ZERO callers outside the unit tests. There is no inbound-mail hook (TASMail proxies BYOK IMAP — Dovecot/Gmail/Outlook ingest mail; TASMail never sees an incoming message). There is no ManageSieve client to push the rule to the BYOK server's Sieve interpreter. There is no client-side application of rules on the SPA's message-list render. **Filters are CRUD-only — they store rules nobody applies.** A user creates "move newsletters@ → Newsletters folder", nothing happens. | **P0 — feature is a lie** | Large — either (a) implement a ManageSieve client (`async-imap` doesn't ship one — `rust-imap-sieve` or hand-rolled RFC 5804) and emit a generated Sieve script per rule-set on every save, or (b) own the inbound side properly: stand up a per-mailbox IDLE worker that watches INBOX, evaluates `matches_email` against every new envelope, executes the action via `imap_service.move_message`/etc. Option (a) is the only scalable answer (BYOK servers' Sieve runs in the same process as delivery, no race, no missed messages) but requires ManageSieve protocol work. Option (b) is the quick fix but breaks if the SPA is offline when mail arrives | New (P0 once any user reports "my filter doesn't fire") |
| 4 | **`EmailTemplate::render` does not HTML-escape merge field values when substituting into `body_html`.** `models/email_template.rs:165–182` does `rendered_html = rendered_html.replace("{{key}}", value)` against the raw HTML body for every (k, v) pair. If a merge field value contains `<script>` / `<img onerror=...>` / `<iframe>`, it gets injected verbatim into the outbound email's HTML. Combined with **shared templates** (next finding), this is a cross-tenant XSS / mail-bomb vector: an attacker registers a TASMail account, creates a template with body_html `<img src="evil" onerror="{{key}}">`, marks it `is_shared=true`, and every other tenant on the instance can now select that template, render it, and email it — the merge field becomes attacker-controlled JS in the recipient's mail client. The frontend preview at `TemplateManager.tsx:343` only renders `previewResult.body_text` so the bug never surfaces in dev/QA. | **P0 — security (XSS + injection)** | Low — escape merge-field values with `htmlescape` / `askama_escape::escape` before substituting into `body_html` only (text body stays as-is). Keep `subject` escaped too (it can land in a header that some clients render). Apply at render time, not store time, so legitimately-HTML-template-authors aren't double-escaped | New (P0; SECURITY-tagged) |
| 5 | **Shared templates leak across tenants (no domain scoping).** `migrations/016_email_templates.sql:23–24`: `CREATE POLICY email_templates_isolation ON email_templates USING (mailbox_id = current_setting('app.current_user_id')::uuid OR is_shared = true)`. The `is_shared` clause has no domain/tenant predicate — a `is_shared=true` template created by **any** user on the instance is visible to **every** user on the instance, regardless of which BYOK domain they signed up under. The handler at `templates.rs::list_templates` calls `EmailTemplate::find_by_mailbox` which runs `WHERE mailbox_id = $1` — so today the leak only fires through `find_by_id` (used by `render_template`), where a user can render any shared template if they know its UUID. The intent (looking at the UI's "Share with team" checkbox label) is clearly domain-scoped sharing. | **P0 — security (cross-tenant leak) + UX (label lies)** | Low — schema: drop `is_shared` from the RLS OR-clause and replace with a domain join: `OR (is_shared = true AND mailbox_id IN (SELECT id FROM mailboxes WHERE domain_id = (SELECT domain_id FROM mailboxes WHERE id = current_setting('app.current_user_id')::uuid)))`. Also update `find_by_mailbox` to OR the same domain-scoped shared rows so the UI actually surfaces them. Add a migration to identify any pre-existing `is_shared=true` rows and either domain-scope them or revoke the flag (no production rows expected — beta scope) | New (P0; SECURITY-tagged; pair with #4 in one SECURITY commit) |
| 6 | **The `field` / `operator` / `action_type` strings in Sieve rules are stringly-typed and not validated.** `models/sieve_rule.rs:9, 11, 22` documents the allowed values for each via doc-comments but the structs hold raw `String`. `handlers/sieve.rs::create_rule` validates only `match_mode in {"all", "any"}` and ignores everything else. `models/sieve_rule.rs::evaluate_condition` silently returns `false` for any unknown operator (matches_regex / greater_than / less_than are documented but missing from the match arm at line 252–259). A user POSTing `{"field": "froom", "operator": "looks_like", "value": "x"}` gets a 201 Created, a row in the DB, and a rule that never matches anything. A user POSTing `{"action_type": "rm -rf /"}` gets the same — the rule is stored, fed back to the UI, displayed, and (if/when finding 3 is fixed) executed by whatever interpreter takes it. | **P1 — typing + input validation** | Low — make `field`, `operator`, `action_type` into Rust enums with `#[serde(rename_all = "snake_case")]` and `#[serde(deny_unknown_fields)]` at the request level. serde will reject unknown variants at parse time with a clear 400. Adds zero runtime cost — the JSONB column stays a string-tagged enum on disk. Add a migration backfill that updates any pre-existing invalid rows (none in beta) | New (P1) |
| 7 | **`EmailTemplate::render` has non-deterministic substitution order.** `models/email_template.rs:170` iterates `for (key, value) in fields` where `fields: &HashMap<String, String>`. Rust's `HashMap` iteration order is non-deterministic across program runs. If a merge field value contains another `{{placeholder}}` token (legitimate name like "{{client.first}}" embedded as data, or attacker-supplied), the second-pass replacement may or may not occur depending on which key was iterated first. The same render request to the same template can produce two different outputs on the same row. | **P1 — correctness** | Low — switch `fields` to `BTreeMap<String, String>` (sorted iteration) OR change the substitution to a single regex-scan pass that finds `\{\{([a-zA-Z0-9_.-]+)\}\}` and looks up each capture exactly once. Single-pass is the safer answer — it also closes the second-pass-replacement injection vector by construction | New (P1) |
| 8 | **The hardcoded date `2026-04-14` in the NLP system prompt.** `services/nlp_parser.rs:28`: `'For relative dates like "last week", "yesterday", "this month", calculate from today's date (2026-04-14)'`. Six weeks stale at the time of this report. "last week" → AI returns dates relative to mid-April 2026 instead of today. Will keep drifting forever, will produce nonsense filter ranges by Q4. | **P1 — correctness** | Low — `chrono::Utc::now().date_naive().format("%Y-%m-%d")` injected into the prompt at call time. While there, also pass the user's timezone so "yesterday" is yesterday in their local — the prompt is currently fully UTC-naive | New (P1) |
| 9 | **IMAP `TEXT` SEARCH criterion is built by `format!("TEXT \"{}\"", query.replace('"', "\\\""))`.** `imap_service.rs:321` does this naive quote-escape. `validation::validate_search_query` already blocks CR/LF/NUL, so a hard injection of `LOGOUT\r\n` is prevented — but the IMAP SEARCH grammar (RFC 3501 §6.4.4) does NOT treat `\"` as the right escape. The literal sequence `\"` is interpreted as backslash + close-quote by Dovecot's IMAP parser, which closes the criterion early and treats the rest as a new criterion (silently failing or returning unexpected matches). A search for `she said \"hi\"` becomes a malformed IMAP command. Better: use a `{N}` IMAP literal — `TEXT {N}\r\n<N bytes>` — which carries no in-band escape problem. The `async-imap` crate supports literals natively. | **P1 — protocol correctness** | Low — replace the format-and-escape with a parameterised search builder that emits literals for any user-supplied string. Same applies to the (yet-to-exist) advanced search FROM/TO/SUBJECT fields from finding 1 | New (P1; pair with finding 1) |
| 10 | **Advanced search is 100% hardcoded — TMAIL-32's "registry-driven advanced filters" goal is not met.** Across `AdvancedSearch.tsx:17` (`emptyFilters()`), `AdvancedSearch.tsx:97–186` (form fields), `SearchResults.tsx:74–86` (`getActiveFilterLabels`), `SearchResults.tsx:117–134` (`removeFilter` field switch), `api/messages.ts:54–64` (URL serialiser), and `api/messages.ts::AdvancedSearchParams` (the interface) — the same 8-field list is hand-rolled in **six** places. Adding a "has_label" filter today means editing all six. The task brief asked specifically whether TMAIL-32 was registry-driven; **it is not**. | **P1 — scalability** | Medium — define `FILTERS: readonly FilterDef[]` in `frontend/src/config/searchFilters.ts` where each entry has `{ key, label, paramName, inputType: 'text'|'date'|'checkbox', queryFromValue: (v) => string }`. `AdvancedSearch.tsx` becomes a `.map(filter => renderInput(filter))`; `SearchResults.tsx` derives labels from the same list; `api/messages.ts` derives the serialiser from `paramName`. Adding has_label becomes one entry in the registry plus a backend column. Backend benefits too — the `AdvancedSearchQuery` struct fields become directly traceable to the registry | New (P1; covers TMAIL-32 follow-up) |
| 11 | **Neither `MessageList` nor `SearchResults` is virtualised, despite `@tanstack/react-virtual` being in the dependency tree.** `frontend/package.json:28` declares `@tanstack/react-virtual: ^3.13.23`; `grep -rn "useVirtualizer\|react-virtual" frontend/src/` returns zero hits. `MessageList.tsx` and `SearchResults.tsx` both render with plain `data.messages.map(...)`. The backend caps `list_messages` at `page_size=200` and `search_messages` at `100`, so this is fine for the steady state; the moment the SPA paginates a folder client-side, or the moment finding 1 ships and search responses get bigger (advanced filters narrow the working set, but per-cycle response sizes grow because IMAP `SEARCH` returns more candidates), the layout-thrash + initial-render cost becomes the dominant frame. | **P2 — performance** | Low — wrap the row `.map()` in a `useVirtualizer({ count, getScrollElement, estimateSize: 60 })` and absolutely-position the rows within. The dependency is already paid for; the bundle delta is zero. See `frontend-render-perf-2026-05.md` finding 4 — this overlaps with that report's MessageList recommendation; land both together | New (P2; subsumed by TMAIL-263's MessageList virtualisation work, see `frontend-render-perf-2026-05.md`) |
| 12 | **Search results are not paginated at the API level.** `imap_service.rs:335` does `uids.iter().rev().take(100)` — fixed cap, no offset/cursor. A user with 10 000 messages matching "invoice" gets the 100 most-recent UIDs and zero recourse. The SPA also doesn't expose page controls on the search view. | **P2 — scalability** | Medium — accept `?offset=N&limit=M` on `/api/search` (default offset=0, limit=100, cap limit=200); slice the `uids` Vec accordingly; return `{messages, total, has_more}` so the SPA can render a "Load more" or page controls. Once finding 1 lands, the same pagination applies to advanced search | New (P2) |
| 13 | **`FilterManager.tsx` declares its OWN hardcoded option lists at module scope** — `FIELDS`, `OPERATORS`, `ACTION_TYPES` at lines 16–40. These mirror the Rust enum-ish doc-comments in `models/sieve_rule.rs` but with no contract — FilterManager declares 5 operators (the matchers that actually work in `evaluate_condition`); the Rust doc-comment claims 8 (including the missing regex/greater_than/less_than). The FilterManager omits action_type `add_label` (listed in the Rust doc-comment) and includes `mark_flagged` (which `evaluate_condition` doesn't handle for matching, but `add_label` isn't handled for actions either — finding 3 makes this moot until rules actually run). This is the same registry-anti-pattern as finding 10 — one source of truth for "valid sieve operators" should drive the form, the backend validator (finding 6), and the matcher | **P2 — scalability + modularisation** | Low — backend exports `GET /api/filters/schema` returning `{ fields: [...], operators: [...], actions: [...] }` derived from the enum; SPA fetches it once and renders form options from the response. Same dropdown definition powers AdvancedSearch + FilterManager + (eventually) the ManageSieve script generator. Aligns with the scalability-first rule | New (P2; pair with finding 6's enum work — same enums, one source) |
| 14 | **`TemplateManager.tsx` is 432 LOC and owns 13 stateful slots + 4 mutations + the form + the preview panel + the list.** `showForm`, `editing`, `previewTemplate`, `previewResult`, `previewFields`, `name`, `subject`, `bodyHtml`, `bodyText`, `mergeFieldsStr`, `category`, `isShared`, `[react-query state for 4 mutations]`. Plus the JSX for header, form, preview, and list. Direct violation of the modular-implementation rule (`~/.claude/rules/all-rules.md` — file size guideline <250 LOC, one concept per file). | **P2 — modularisation** | Medium — `useTemplates()` hook for the queries+mutations, `TemplateEditor` (form), `TemplatePreview` (preview panel), `TemplateList` (list + delete), parent shell composes them. Cuts duplication of the field-reset logic and the mergeFieldsStr↔array conversion | New (P2) |
| 15 | **`FilterManager.tsx` is 393 LOC and owns the list + the editor + the priority reorder + the toggle + the delete confirm prompt.** Same anti-pattern as finding 14. Adding "rule clone" or "rule dry-run" means more state in the same component. | **P2 — modularisation** | Medium — `FilterList`, `FilterEditor` (already a sub-fn but inline), `ConditionRow` + `ActionRow` (already lifted but in the same file). Move each to its own file, share types via `api/filters.ts`. `useFilters()` hook for the four mutations | New (P2) |
| 16 | **`SieveRule::matches_email` collapses `to` and `cc` into the same case.** `models/sieve_rule.rs:227`: `"to" | "cc" => to`. A rule like "if cc contains alice@" gets evaluated against the To header value, not the Cc value. Once finding 3 wires the engine up, this matters. | **P2 — correctness (latent — gated on #3)** | Low — change the signature to `matches_email(from, to, cc, subject, headers)` and route the `cc` arm to its own arg. Update both call sites (currently zero call sites — see #3) | New (P2; latent on finding 3) |
| 17 | **`SieveRule::matches_email` doesn't handle the `body` field documented in the model.** `RuleCondition` doc-comment lists `body` as a valid field; the matcher's match arm at line 224–237 doesn't route it. Same for `header` partially (which IS routed but conflates the search value with the header name in a way that's mildly surprising — line 230–235 finds a header whose NAME matches `c.value` rather than matching the header's VALUE against `c.value`; that's almost certainly an implementation bug rather than the intended behaviour). | **P2 — correctness (latent — gated on #3)** | Low — extend the matcher to take a `body: &str` arg, add the `body` arm. Fix the header arm to actually compare the named header's value against `c.value` (use a separate `header_name` field on `RuleCondition`, or encode as `c.value = "X-Mailer: bad"` and split on `:`) | New (P2; latent on #3) |
| 18 | **`NlpSearchHistory::create` swallows errors with `let _ = …`.** `handlers/nlp_search.rs:66`: history write is fire-and-forget. If the DB write fails the user gets a successful 200, but the history list never reflects the query. There's no log either — the `_` discards the `Result`. | **P3 — observability** | Low — `if let Err(e) = NlpSearchHistory::create(...).await { tracing::warn!(error = ?e, query = %body.query, "nlp search history write failed"); }` | New (P3) |
| 19 | **`pgvector` is optional but the semantic_search handler doesn't tell the user when it's not available.** `migrations/036_semantic_search.sql` wraps the CREATE TABLE in a `DO $$ … RAISE NOTICE 'pgvector extension not available' … $$` block. The handler then calls `EmailEmbedding::upsert` / `search_similar` which will fail with a SQL relation-doesn't-exist error and return a 500. The SPA shows "Search failed" with no actionable diagnosis. The CLAUDE.md note for migration 036 says "returns 503 at runtime" — that 503 is documented but not implemented anywhere I can find. | **P3 — DX** | Low — add a `EmailEmbedding::is_available(pool)` helper that checks `SELECT 1 FROM pg_extension WHERE extname='vector'`; the handler returns `503 Service Unavailable` with body `"pgvector not installed — semantic search disabled. See deploy/scripts/install-pgvector.sh"` instead of bubbling the SQL error | New (P3) |
| 20 | **`EmailEmbedding` uses `ivfflat` with `lists = 100`.** `migrations/036_semantic_search.sql:25`. ivfflat is reasonable for ~10k–1M rows; below 10k it's slower than a sequential scan and above ~5M it loses recall vs `hnsw`. With per-mailbox indexing (this table is one row per (user, folder, uid)), a single tenant with a busy inbox can pass 100k embeddings within a year. `hnsw` (pgvector 0.5+) gives faster queries at the cost of bigger build/insert time — a better long-term default for read-heavy semantic search. | **P3 — performance (latent on usage scale)** | Low — new migration: `CREATE INDEX … USING hnsw (embedding vector_cosine_ops) WITH (m = 16, ef_construction = 64);` and drop the ivfflat index. Test before/after recall@10 on a synthetic 10k corpus | New (P3) |
| 21 | **Positive baselines (keep doing this):** `handlers/messages.rs:247–250` validates BOTH search query AND folder name via `validation::validate_search_query` / `validate_folder_name` before touching IMAP (TMAIL-37 — same defence-in-depth pattern as compose); `validation::validate_search_query` correctly blocks CR/LF/NUL injection (tested at `validation.rs:303–311`); `sieve_rules`, `email_templates`, `email_embeddings`, and `nlp_search_history` all have RLS policies set up and pointing at the right `current_setting('app.current_user_id')` (modulo finding 5's `is_shared` issue); `models/email_embedding.rs` defines `count_by_user` + `count_by_folder` so the index-stats endpoint is one query instead of a scan; the IMAP search result count is capped (it's just capped too low — finding 12 — but it IS capped, so a malicious search can't return 1M rows). | Positive baseline | — | — |

---

## 1. The "feature looks shipped but is not wired" cluster (findings 1 + 2 + 3)

This assessment is bracketed by three independent search/filter features that
all reached the UI and stopped one step short of working end-to-end:

1. **Advanced search (TMAIL-32)** — frontend serialises 8 params, backend
   accepts 2.
2. **NLP search (TMAIL-135)** — frontend renders the AI-parsed intent, backend
   executes the AI call, **then explicitly returns empty results** without
   running the IMAP query the AI built.
3. **Sieve filters (TMAIL-?? — original ticket pre-PM)** — full CRUD UI, full
   priority reorder, full enable/disable toggle, evaluator with unit tests —
   **nothing executes the rules against any mail flow.**

These aren't bugs, they're "stop-50%-complete" patterns. The contract between
the SPA and the backend was never enforced by a type system or a contract test,
so each side independently progressed to "looks done from my side" without
discovering the gap. The traceability gate (`scripts/trace-check.py`, see
project CLAUDE.md) catches *missing endpoints* but not *endpoints that accept
fewer params than the client sends*. None of these three would have shown up
in a green CI run.

### Repro for finding 1 (advanced search drops params)

1. Sign in to TASMail. Click search; expand "Advanced".
2. Set "From" to a known sender, leave query empty.
3. Click Search. Backend log shows `GET /api/search?q=&folder=INBOX&from=alice%40foo.com&...`.
4. `RUST_LOG=debug` shows `Query<SearchQuery>` deserialises to `SearchQuery { q: "", folder: Some("INBOX") }`.
5. IMAP run: `TEXT ""` (empty TEXT) — Dovecot returns the most recent N envelopes in the folder, NOT filtered by sender.
6. The SPA shows the chip "From: alice@foo.com" but the result list is the full INBOX.

### Repro for finding 2 (NLP search returns empty)

1. Configure an AI provider via Settings → AI Config (Anthropic or OpenAI).
2. Open the NLP search panel, type "emails from John about budget last week".
3. The "Parsed intent" card renders correctly (`{from: "John", subject: "budget", date_from: "...", date_to: "..."}`).
4. The results pane shows "0 results".
5. `nlp_search_history` table has a row with `result_count=0`.
6. The AI provider's billing dashboard shows the request cost (Anthropic Claude
   Haiku ~$0.0001, OpenAI gpt-3.5 ~$0.0002 — small, but recurring per query).

### Repro for finding 3 (Sieve rules never fire)

1. Create a filter via Settings → Filters: "From contains newsletter@ → Move to Newsletters".
2. Send a message from `newsletter@example.com` to your TASMail-attached
   address (which arrives in Dovecot directly — TASMail isn't in the
   delivery path).
3. Open INBOX. Message is in INBOX, not Newsletters.
4. `grep -rn "SieveRule::find_by_mailbox" backend/src/` shows the only
   call is `handlers/sieve.rs::list_rules` — the GET endpoint that lists
   rules to the SPA. Nothing reads them to apply them.

### Recommended consolidation

The three problems share a root: **the SPA can't trust the search/filter
backend to do what the SPA visibly promises**. Fix by:

1. **Finding 1 + 9 land together** — `AdvancedSearchQuery` struct, IMAP
   literal-based criterion builder, regression test that round-trips an
   advanced search and asserts a known message comes back filtered.
2. **Finding 2 lands next** — wire `nlp_parser::build_imap_search` into the
   `AdvancedSearchQuery` path so NLP and Advanced share the same execution
   layer.
3. **Finding 3 is its own epic** — ManageSieve push is the right answer for
   BYOK (most providers' Dovecot/Gmail-equivalent honours pushed Sieve);
   for the providers that don't (older Exchange, ProtonMail Bridge), a
   per-mailbox IDLE worker is the fallback. Either way it's >1 week of
   work, not a one-commit fix.

---

## 2. Templates: the XSS + cross-tenant leak (findings 4 + 5)

These two findings combine into a single security headline:

**Any TASMail user can publish a malicious HTML template visible to every
other user on the instance, and when those users render that template with
ANY merge-field value, the rendered HTML is unescaped.**

The render-time HTML-injection vector (finding 4) is exploitable
self-against-self today; finding 5's cross-tenant `is_shared` scope makes it
exploitable user-against-user. Either one alone is a security finding; the
combination needs to land in one atomic commit so the disclosure window is
zero.

### Recommended remediation order

1. **Single commit:**
   - Escape `body_html` substitutions (finding 4)
   - Domain-scope shared templates (finding 5)
   - Backfill migration: identify any existing `is_shared=true` rows; if a
     row's mailbox_id's domain doesn't match the viewing user's domain, hide
     it. For beta we expect zero such rows but the migration must run safely
     against any.
2. **Follow-up commit (post-beta hardening):**
   - Add a per-domain `template_visibility` enum (`'private'|'domain'|'org'`)
     so future enterprise tier can opt into broader sharing without rewriting
     the policy.

---

## 3. Search backend topology

A useful map of what runs where, because the ticket asked for the "branch
boundary" between IMAP and pgvector:

| Endpoint | Source of truth | Where the heavy lifting happens | Status |
|---|---|---|---|
| `GET /api/search?q=` | BYOK IMAP (Dovecot/Gmail/Outlook/...) | `imap_service.search_messages` → `UID SEARCH TEXT "..."` | works (within finding 9's escaping caveat) |
| `GET /api/search?q=&from=&...` | BYOK IMAP | same; advanced params dropped | **broken** (finding 1) |
| `POST /api/search/semantic` | `email_embeddings` table (pgvector) | `embedding_service::search_similar` → `ORDER BY embedding <=> $1 LIMIT $2` | works **if** pgvector installed AND emails indexed |
| `POST /api/search/index` | (writes to embeddings table) | `embedding_service::generate_embedding` → AI provider, then `EmailEmbedding::upsert` | works; called manually per-email; no batch backfill / no IDLE-trigger indexer wired up |
| `POST /api/search/nlp` | (would be IMAP) | `nlp_parser::parse_natural_query` → AI provider → empty result | **broken** (finding 2) |
| `POST /api/search/nlp/history` | `nlp_search_history` table | direct SQL | works |
| `POST /api/archive/search` | (out of scope — see `attachments-storage-2026-05.md`) | — | — |

The boundary is clean: IMAP for "find this string in headers/body of mail
your server has", pgvector for "find by meaning of indexed content",
NLP-via-AI for "translate language to IMAP query" (and that NLP layer is
designed to compose ONTO the IMAP layer — `nlp_parser::build_imap_search`
returns an IMAP SEARCH criterion. The execution coupling is the missing
piece).

Indexing of mail for pgvector is a manual one-at-a-time call today
(`POST /api/search/index` with a folder/uid/text payload). The IMAP IDLE
worker mentioned for finding 3 is the natural place to also trigger
indexing on newly-arrived mail.

---

## 4. Static-types axis

| Where | Issue | Fix shape |
|---|---|---|
| `models/sieve_rule.rs::RuleCondition.field/operator` (line 9, 11) | `String` for fixed-set values; doc-comment is the only contract | `enum SieveField { From, To, Cc, Subject, Header, Body }` + `enum SieveOp { Contains, NotContains, … }` with `#[serde(rename_all="snake_case")]` |
| `models/sieve_rule.rs::RuleAction.action_type` (line 22) | same | `enum SieveActionType` |
| `models/sieve_rule.rs::SieveRule.conditions/actions` (line 33, 37) | `serde_json::Value` (untyped) | Typed `Vec<RuleCondition>` / `Vec<RuleAction>` stored as JSONB (sqlx supports `Json<Vec<T>>`) |
| `frontend/src/api/messages.ts::AdvancedSearchParams` | hand-typed; not generated from Rust | derive from a shared `openapi.json` (TASMail doesn't generate one yet — see frontend-types-parity-2026-05) |
| `frontend/src/components/settings/FilterManager.tsx::FIELDS / OPERATORS / ACTION_TYPES` (line 16–40) | TS literal lists duplicating Rust doc-comment values | see finding 13 — `GET /api/filters/schema` |
| `models/email_template.rs::EmailTemplate.merge_fields` (line 17) | `serde_json::Value`; relied on being a string array | `Vec<String>` stored as `Json<Vec<String>>` |
| `models/nlp_search.rs::ParsedSearchParams` | (re-read; not in scope) | likely same Option-soup pattern as above |

---

## 5. Modularisation axis

| File | LOC | Issue | Suggested split |
|---|---|---|---|
| `frontend/src/components/settings/TemplateManager.tsx` | 432 | 13 state slots, 4 mutations, form + preview + list in one | `useTemplates()`, `TemplateEditor`, `TemplatePreview`, `TemplateList`, `TemplateManager` shell (see finding 14) |
| `frontend/src/components/settings/FilterManager.tsx` | 393 | list + editor + reorder + toggle + delete + 4 mutations in one | `FilterList`, `FilterEditor` (own file), `FilterRow`, `useFilters()` (see finding 15) |
| `frontend/src/components/mail/SearchResults.tsx` | 175 | OK on size; tight coupling to `useMailStore` for both query/folder + advanced params | extract `useSearchView()` hook that merges simple/advanced and returns one `{ data, isLoading, error, mode }` |
| `frontend/src/components/mail/AdvancedSearch.tsx` | 209 | 8 hardcoded fields (finding 10); inline empty-filter factory + validation | registry-driven (finding 10) |
| `backend/src/services/imap_service.rs::search_messages` (lines 305–384) | ~80 in the function | wide function: connect + select + search + uid_fetch + envelope-parse | extract `parse_envelope_response(&[Fetch]) -> Vec<MessageEnvelope>` and reuse from `list_messages` / `search_messages` / `search_advanced` (finding 1) |
| `backend/src/handlers/sieve.rs` | 204 | OK | — |
| `backend/src/handlers/templates.rs` | 175 | OK | — |

---

## 6. Performance & scalability axis

| Risk | Where | Severity | Pointer |
|---|---|---|---|
| Search not paginated | `imap_service.rs:335` (`.take(100)`) | P2 | finding 12 |
| List not virtualised | `MessageList.tsx`, `SearchResults.tsx` | P2 | finding 11; overlaps `frontend-render-perf-2026-05.md` |
| pgvector index type (ivfflat at 100 lists) sub-optimal beyond ~100k rows | `migrations/036_semantic_search.sql:25` | P3 | finding 20 |
| Per-search IMAP session open/close (no pooling) | `imap_service.rs:313` (each `connect`) | same shape as `compose-send-2026-05.md` finding 6 (transport pool) | covered by `folders-messages-2026-05.md` recommendations |
| AI call per NLP search with no cache | `nlp_parser.rs::parse_natural_query` | P3 | cache parsed-params keyed on `(user_id, query.trim().lowercase())` with a 1-hour TTL — same query twice in a session shouldn't burn two AI calls |
| Template render is in-process and synchronous | `templates.rs::render_template` | P3 — fine | — |
| Sieve rule `matches_email` is O(rules × conditions) per message | `models/sieve_rule.rs:218` | P2 latent on finding 3 | once rules actually fire, push to ManageSieve (server-side eval) — keeps the matcher off the TASMail hot path |

---

## Follow-up tasks

The findings above that warrant their own PM tickets (raised priority-low
under the same parent TMAIL-241 epic):

| Finding(s) | Suggested ticket title | Priority |
|---|---|---|
| 1, 9 | Implement advanced search backend with IMAP literal-based criterion builder | P0 |
| 2 | Wire NLP search to execute the parsed IMAP query | P0 |
| 3 | Implement Sieve rule execution — ManageSieve push (preferred) or IDLE-worker fallback | P0 (epic) |
| 4, 5 | SECURITY: escape template `body_html` merge fields; domain-scope shared templates | P0 (single SECURITY commit) |
| 6 | Convert Sieve `field` / `operator` / `action_type` to typed enums | P1 |
| 7 | Make `EmailTemplate::render` substitution single-pass + deterministic | P1 |
| 8 | NLP system prompt: inject today's date dynamically | P1 |
| 10, 13 | Registry-driven advanced filters + Sieve filter schema endpoint | P1 |
| 11 | Virtualise `MessageList` + `SearchResults` (overlap with TMAIL-263) | P2 |
| 12 | Paginate `/api/search` | P2 |
| 14, 15 | Modularise `TemplateManager.tsx` and `FilterManager.tsx` | P2 |
| 16, 17 | Fix `matches_email` cc/body/header handling (latent on Sieve execution) | P2 |
| 18, 19 | Observability — log NLP history errors; explicit 503 when pgvector missing | P3 |
| 20 | Switch pgvector index from ivfflat to hnsw | P3 |

The accompanying commit for this report carries `TMAIL-245` in its subject.
The four P0 follow-ups (#1/9, #2, #3, #4+5) are queued as separate
auto-fix items so each gets its own commit + PM trail.
