// TMAIL-309: tower middleware that restores middleware-side defense-in-depth
// for Row-Level Security.
//
// Background — the gap this closes:
//   * Postgres RLS policies (see `backend/migrations/008_row_level_security.sql`
//     and ~30 others) filter rows on `current_setting('app.mailbox_id')` /
//     `current_setting('app.current_user_id')` / `current_setting('app.is_admin')`.
//   * Setting those vars on a pool connection only sticks for that connection.
//     The original auth middleware tried to `SET app.mailbox_id` on a connection
//     from the pool, but the handler's next `&state.db` query checked out a
//     DIFFERENT connection with no vars set — the SET vanished.
//   * After TMAIL-161 the auth middleware was downgraded to a no-op, leaving
//     tenant isolation entirely up to per-handler `WHERE mailbox_id = $N`
//     discipline. One forgotten WHERE = silent cross-tenant leak.
//
// What this middleware does:
//   * Runs immediately AFTER `auth_middleware` (which has already validated the
//     JWT and put `Claims` in request extensions).
//   * Clones `(AppState, Claims)` into a lightweight `RlsRequestContext` and
//     stores it in extensions.
//   * Acquisition of the actual connection is LAZY — it happens when a handler
//     extracts `RlsConn` (see `services::db_session`). Requests that never need
//     the DB never burn a pool slot.
//
// Why lazy and not eager:
//   * The pool defaults to 10 connections (DATABASE_MAX_CONNECTIONS=10). Many
//     handlers do slow I/O (IMAP, SMTP, attachment streaming) and would hold a
//     connection for seconds-to-minutes if we eagerly acquired in middleware.
//     Under moderate load that would starve the pool.
//   * Lazy acquisition keeps the pool semantics intact: a request that doesn't
//     touch the DB doesn't checkout a connection at all.
//
// Migration path for handlers (incremental):
//   1. Change handler signature from `axum::Extension<Claims>` + `&state.db`
//      to `RlsConn` (the extractor) — see `services::db_session::RlsConn`.
//   2. Replace `&state.db` with `&mut *rls.conn` (or `&mut *rls` thanks to Deref).
//   3. The `WHERE user_id = $N` filter can stay as defense in depth, but RLS
//      now backs it up at the DB level.

use axum::{
    extract::{Request, State},
    middleware::Next,
    response::Response,
};

use crate::error::AppError;
use crate::services::auth_service::Claims;
use crate::state::AppState;

/// Cheap, cloneable bundle of the things the `RlsConn` extractor needs to lazily
/// acquire a connection with RLS session vars set. Inserted into request
/// extensions by `rls_context_middleware`. Fields are read by
/// `services::db_session::RlsConn::from_request_parts` via
/// `parts.extensions.get::<RlsRequestContext>()`, which the compiler can't see
/// statically — so `#[allow(dead_code)]` keeps the warning off.
#[derive(Clone)]
#[allow(dead_code)]
pub struct RlsRequestContext {
    pub state: AppState,
    pub claims: Claims,
}

/// Tower middleware. Runs after `auth_middleware`. Reads the validated `Claims`
/// out of extensions and parks an `RlsRequestContext` next to them so the
/// `RlsConn` extractor can lazily acquire an RLS-primed connection later.
///
/// Returns 500 if it runs without `Claims` already in extensions — that would
/// indicate the layers are stacked in the wrong order (auth_middleware must
/// come first). This is a configuration error, not a runtime hazard.
pub async fn rls_context_middleware(
    State(state): State<AppState>,
    mut req: Request,
    next: Next,
) -> Result<Response, AppError> {
    let claims = req
        .extensions()
        .get::<Claims>()
        .cloned()
        .ok_or_else(|| {
            AppError::Internal(anyhow::anyhow!(
                "rls_context_middleware ran without Claims in extensions; \
                 wire auth_middleware BEFORE rls_context_middleware"
            ))
        })?;

    req.extensions_mut().insert(RlsRequestContext { state, claims });

    Ok(next.run(req).await)
}
