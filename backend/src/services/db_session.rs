// Per-request DB session helpers — production-grade RLS enforcement.
//
// Background: TASMail uses Postgres row-level security policies that filter on
// `current_setting('app.mailbox_id')` and `current_setting('app.current_user_id')`.
// The auth middleware was previously calling `SET app.mailbox_id = …` on a
// connection from the pool, but the next handler query got a *different*
// connection (a fresh acquire), so the SET vars never carried. Result: RLS
// evaluated `current_setting()` as NULL on every protected query, the policies
// silently failed, and handlers only worked because they ALSO included
// explicit `WHERE user_id = $N` filters.
//
// Fix: hold a single PgConnection for the lifetime of an RLS-sensitive operation
// and run all queries against it. This module exposes:
//
//   * `acquire_with_rls(state, claims)` — low-level helper that acquires a
//     connection and primes it with the three RLS session vars.
//   * `RlsConn` — an Axum extractor handlers can declare directly in their
//     signature; the extractor reads claims+state from request extensions
//     (populated by the `rls_context_middleware` tower layer) and calls
//     `acquire_with_rls` on demand.
//
// Usage in a handler:
//
//     pub async fn list_contacts(rls: RlsConn) -> Result<...> {
//         let mut conn = rls;          // RlsConn derefs to PoolConnection
//         let rows = sqlx::query_as::<_, Foo>("SELECT * FROM foo")
//             .fetch_all(&mut *conn).await?;
//     }
//
// The connection is released to the pool when `RlsConn` is dropped — i.e. when
// the handler returns. Acquisition is lazy: a handler that never asks for the
// connection pays nothing beyond inserting the cheap `RlsRequestContext` into
// extensions.
//
// TMAIL-309: previously this helper only set `app.mailbox_id` and `app.is_admin`,
// but ~38 RLS policies in `backend/migrations/*.sql` filter on
// `app.current_user_id` (push_devices, payments, subscriptions, quarantine,
// calendar grants, shared_mailbox_acl, etc). Without setting that var those
// policies evaluated NULL and silently denied every row. We now set all three
// vars from the same `claims.sub` UUID so every policy sees the right value.

use axum::extract::FromRequestParts;
use axum::http::request::Parts;
use sqlx::pool::PoolConnection;
use sqlx::Postgres;

use crate::error::AppError;
use crate::middleware::rls_context::RlsRequestContext;
use crate::services::auth_service::Claims;
use crate::state::AppState;

/// Acquire a connection from the pool and prime it with the request-scoped
/// RLS session variables. All queries run against the returned connection see
/// the RLS policies evaluate to the correct user_id / mailbox_id / admin flag.
///
/// Callers MUST run all subsequent queries against this connection (via
/// `&mut *conn`). Acquiring a fresh `&state.db` connection in the same handler
/// will silently bypass RLS.
///
/// Variables set (matching the RLS policies in `backend/migrations/`):
///   * `app.current_user_id`  — claims.sub (uuid)
///   * `app.mailbox_id`       — claims.sub (uuid); alias used by older migrations
///   * `app.is_admin`         — "true" or "false"
pub async fn acquire_with_rls(
    state: &AppState,
    claims: &Claims,
) -> Result<PoolConnection<Postgres>, AppError> {
    // Validate the claim sub is a real UUID before letting it anywhere near SQL.
    // set_config() is parameterised so injection is not the concern; we want to
    // fail loud rather than silently set RLS to "00000000-..." or empty.
    uuid::Uuid::parse_str(&claims.sub).map_err(|_| {
        AppError::Internal(anyhow::anyhow!(
            "Invalid mailbox UUID in JWT claims; refusing to set RLS context"
        ))
    })?;

    let mut conn = state.db.acquire().await?;

    sqlx::query("SELECT set_config('app.current_user_id', $1, false)")
        .bind(&claims.sub)
        .execute(&mut *conn)
        .await?;

    sqlx::query("SELECT set_config('app.mailbox_id', $1, false)")
        .bind(&claims.sub)
        .execute(&mut *conn)
        .await?;

    sqlx::query("SELECT set_config('app.is_admin', $1, false)")
        .bind(if claims.is_admin { "true" } else { "false" })
        .execute(&mut *conn)
        .await?;

    Ok(conn)
}

/// Axum extractor that yields a pooled Postgres connection with RLS context
/// already set. Wraps `acquire_with_rls` so handlers can write:
///
///     pub async fn handler(rls: RlsConn, ...) -> Result<..., AppError> {
///         let rows = sqlx::query_as!(...).fetch_all(&mut *rls.conn).await?;
///     }
///
/// Requires `rls_context_middleware` to have run earlier in the request
/// pipeline (which is wired in `router::create_router` immediately after
/// `auth_middleware`).
pub struct RlsConn {
    pub conn: PoolConnection<Postgres>,
}

impl std::ops::Deref for RlsConn {
    type Target = PoolConnection<Postgres>;
    fn deref(&self) -> &Self::Target {
        &self.conn
    }
}

impl std::ops::DerefMut for RlsConn {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.conn
    }
}

impl<S> FromRequestParts<S> for RlsConn
where
    S: Send + Sync,
{
    type Rejection = AppError;

    async fn from_request_parts(parts: &mut Parts, _state: &S) -> Result<Self, Self::Rejection> {
        let ctx = parts
            .extensions
            .get::<RlsRequestContext>()
            .cloned()
            .ok_or_else(|| {
                AppError::Internal(anyhow::anyhow!(
                    "RlsConn extracted before rls_context_middleware ran. \
                     Make sure protected_routes layer rls_context_middleware after auth_middleware."
                ))
            })?;
        let conn = acquire_with_rls(&ctx.state, &ctx.claims).await?;
        Ok(RlsConn { conn })
    }
}
