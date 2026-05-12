// Per-request DB session helpers — production-grade RLS enforcement.
//
// Background: TASMail uses Postgres row-level security policies that filter on
// `current_setting('app.mailbox_id')`. The auth middleware was previously calling
// `SET app.mailbox_id = …` on a connection from the pool, but the next handler query
// got a *different* connection (a fresh acquire), so the SET vars never carried.
// Result: RLS evaluated `current_setting()` as NULL on every protected query, the
// policies silently failed, and handlers only worked because they ALSO included
// explicit `WHERE user_id = $N` filters.
//
// Fix: hold a single PgConnection for the lifetime of an RLS-sensitive operation
// and run all queries against it. This module exposes `acquire_with_rls(state, claims)`
// which acquires a connection, runs SET on it, and returns the connection ready for use.
//
// Usage in a handler:
//
//     let mut conn = acquire_with_rls(&state, &claims).await?;
//     let rows = sqlx::query_as::<_, Foo>("SELECT * FROM foo")
//         .fetch_all(&mut *conn)
//         .await?;
//
// The connection is released to the pool when `conn` is dropped.

use sqlx::pool::PoolConnection;
use sqlx::Postgres;

use crate::error::AppError;
use crate::services::auth_service::Claims;
use crate::state::AppState;

/// PURPOSE: Acquire a connection from the pool and prime it with the request-scoped
/// RLS session variables. All queries run against the returned connection see the
/// RLS policies evaluate to the correct user_id.
///
/// CONSTRAINTS: The caller MUST run all subsequent queries against this connection
/// (via `&mut *conn`). Acquiring a fresh `&state.db` connection in the same handler
/// will silently bypass RLS.
pub async fn acquire_with_rls(
    state: &AppState,
    claims: &Claims,
) -> Result<PoolConnection<Postgres>, AppError> {
    let mut conn = state.db.acquire().await?;

    sqlx::query("SELECT set_config('app.mailbox_id', $1, false)")
        .bind(&claims.sub)
        .execute(&mut *conn)
        .await?;

    sqlx::query("SELECT set_config('app.is_admin', $1, false)")
        .bind(claims.is_admin.to_string())
        .execute(&mut *conn)
        .await?;

    Ok(conn)
}
