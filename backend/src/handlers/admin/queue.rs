// TMAIL-58 admin endpoint for the global email queue.
//
//   GET /api/admin/queue-stats — aggregate counts by status across ALL mailboxes
//
// The per-user equivalent is GET /api/queue/stats (handlers/queue.rs).
// Admin stats include the same shape (pending, sending, sent, failed, dead_letter, bounced)
// but counted globally so ops can monitor backlog and bounce rates across the fleet.

use axum::{extract::State, Json};

use crate::error::AppError;
use crate::models::email_queue::{EmailQueueItem, QueueStats};
use crate::services::auth_service::{self, Claims};
use crate::state::AppState;

/// GET /api/admin/queue-stats — global queue statistics (admin only)
pub async fn admin_queue_stats(
    State(state): State<AppState>,
    axum::Extension(claims): axum::Extension<Claims>,
) -> Result<Json<QueueStats>, AppError> {
    auth_service::require_admin(&claims)?;
    let stats = EmailQueueItem::queue_stats(&state.db).await?;
    Ok(Json(stats))
}

#[cfg(test)]
mod tests {
    use crate::services::auth_service::{self, Claims};

    fn make_claims(is_admin: bool) -> Claims {
        Claims {
            sub: "00000000-0000-0000-0000-000000000001".into(),
            username: "user@example.com".into(),
            is_admin,
            is_compliance_officer: false,
            exp: 9_999_999_999,
            iat: 0,
        }
    }

    // Added: TMAIL-58 — non-admins must be rejected from /api/admin/queue-stats
    #[test]
    fn require_admin_rejects_non_admin_claims() {
        let claims = make_claims(false);
        let err = auth_service::require_admin(&claims).expect_err("non-admin should be forbidden");
        // AppError::Forbidden carries the message — we just confirm the rejection path fires
        let msg = format!("{}", err);
        assert!(msg.to_lowercase().contains("admin") || msg.to_lowercase().contains("forbid"),
            "expected forbidden-style error, got: {}", msg);
    }

    // Added: TMAIL-58 — admin claims pass the gate
    #[test]
    fn require_admin_accepts_admin_claims() {
        let claims = make_claims(true);
        auth_service::require_admin(&claims).expect("admin should pass gate");
    }
}
