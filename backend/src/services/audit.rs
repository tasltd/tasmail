// Added (TMAIL-307): Centralised admin-action audit helper. Every state-changing
// admin endpoint should call `audit_admin_action` at the end of a successful
// mutation so the audit_log table accumulates a complete compliance trail
// (delete user, rotate payment provider key, release legal hold, update branding,
// toggle feature flag, etc.).
//
// Semantics:
// * Fire-and-forget — a DB failure on the audit insert MUST NOT break the
//   admin action that already succeeded. We log at warn! and move on.
// * Actor is derived from `claims.sub` (mailbox uuid). If the sub doesn't
//   parse as a uuid we still record the row with mailbox_id = NULL so the
//   action itself is preserved.
// * `AuditLog::record` already pins app.is_admin = true on its connection so
//   the audit_log RLS policy admits the insert regardless of request context
//   (TMAIL-198).

use sqlx::PgPool;
use uuid::Uuid;

use crate::models::audit_log::AuditLog;
use crate::services::auth_service::Claims;

/// Record an admin state-change in the audit_log table.
///
/// `action`  — dotted verb, e.g. `domain.delete`, `user.create`, `feature_flag.update`.
/// `resource_type` / `resource_id` — what the action mutated (optional for actions
/// that don't target a single row, e.g. `branding.reset`).
/// `details` — JSON blob with diff / before-after / extra context.
pub async fn audit_admin_action(
    pool: &PgPool,
    claims: &Claims,
    action: &str,
    resource_type: Option<&str>,
    resource_id: Option<&str>,
    details: Option<serde_json::Value>,
) {
    let actor = Uuid::parse_str(&claims.sub).ok();
    if let Err(e) = AuditLog::record(
        pool,
        actor,
        action,
        resource_type,
        resource_id,
        details,
        None,
        None,
    )
    .await
    {
        tracing::warn!(
            error = %e,
            action = action,
            actor = ?actor,
            resource_type = ?resource_type,
            resource_id = ?resource_id,
            "TMAIL-307: failed to record admin audit log entry",
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::services::auth_service::Claims;

    fn admin_claims_with_sub(sub: &str) -> Claims {
        Claims {
            sub: sub.to_string(),
            username: "admin@example.com".into(),
            is_admin: true,
            is_compliance_officer: false,
            exp: 0,
            iat: 0,
        }
    }

    #[test]
    fn parses_valid_uuid_sub_into_actor() {
        // The actor parse path: a real uuid in claims.sub yields Some(uuid).
        let uid = Uuid::new_v4();
        let claims = admin_claims_with_sub(&uid.to_string());
        let parsed = Uuid::parse_str(&claims.sub).ok();
        assert_eq!(parsed, Some(uid));
    }

    #[test]
    fn invalid_uuid_sub_yields_none_actor() {
        // Defensive: if a token somehow carries a non-uuid sub, we record
        // the audit row with mailbox_id = NULL rather than dropping the entry.
        let claims = admin_claims_with_sub("not-a-uuid");
        let parsed = Uuid::parse_str(&claims.sub).ok();
        assert!(parsed.is_none());
    }
}
