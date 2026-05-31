# Askama + Axum 0.8 Integration (research for TMAIL-355)

**Date:** 2026-05-31
**Context:** Scaffolding `/classic` no-JS surface uses Askama compile-time templates.
**Spec:** `docs/gap-analysis/classic-ui.md` P0 #1 selected Askama for compile-time
checking, no runtime parsing, no GPL dependency.

## Version notes

* `askama` 0.13+ removed the bundled `askama_axum` integration crate. The previous
  approach of `axum = "..."; askama_axum = "..."` no longer works.
* Two viable replacements:
  1. **`askama_web`** with the `axum-0.8` feature flag — re-derives `IntoResponse`
     so a template struct can be returned directly from a handler.
  2. **Manual render** — call `template.render()` (returns `Result<String, askama::Error>`)
     and wrap in `axum::response::Html(...)`. One extra line per handler.

## Decision

Use **manual render** + `axum::response::Html`. Rationale:

* One fewer transitive dependency to track.
* Explicit error propagation (lets us map `askama::Error` to `AppError::Internal`
  uniformly, instead of relying on a default 500 conversion).
* Keeps the template type free of axum-specific traits, so the same template
  can later be rendered for tests or for an SSR snapshot pipeline without
  pulling axum into the test harness.

## Sources

* [askama on crates.io](https://crates.io/crates/askama)
* [askama_web on crates.io](https://crates.io/crates/askama_web) — alternative integration
* [askama_axum 0.5.0+deprecated](https://docs.rs/crate/askama_axum/latest) — deprecated path, not used
* [Reconsider removing axum integration · askama#1119](https://github.com/askama-rs/askama-old/issues/1119) — explains why integration was dropped
