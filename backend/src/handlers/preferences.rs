// Added (TMAIL-401): per-user preference handlers. Currently only the
// first-login-tour-seen flag — kept as its own module so future
// preference flags (digest cadence, density mode, etc.) can land here
// without touching the auth or auto-reply modules.

use axum::{extract::State, http::StatusCode, Json};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::error::AppError;
use crate::services::auth_service::Claims;
use crate::state::AppState;

#[derive(Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct TourSeenResponse {
    pub seen: bool,
}

fn mailbox_id_from_claims(claims: &Claims) -> Result<Uuid, AppError> {
    claims
        .sub
        .parse()
        .map_err(|_| AppError::Internal(anyhow::anyhow!("Invalid mailbox ID in JWT subject")))
}

/// GET /api/me/preferences/first-login-tour-seen — returns whether the
/// authenticated user has already dismissed the first-login tour.
pub async fn get_first_login_tour_seen(
    State(state): State<AppState>,
    axum::Extension(claims): axum::Extension<Claims>,
) -> Result<Json<TourSeenResponse>, AppError> {
    let mailbox_id = mailbox_id_from_claims(&claims)?;

    let row: Option<(bool,)> = sqlx::query_as(
        "SELECT first_login_tour_seen FROM mailboxes WHERE id = $1",
    )
    .bind(mailbox_id)
    .fetch_optional(&state.db)
    .await?;

    let seen = row.map(|(v,)| v).unwrap_or(false);
    Ok(Json(TourSeenResponse { seen }))
}

/// PATCH /api/me/preferences/first-login-tour-seen — marks the tour as
/// seen for the authenticated user. Always idempotent — repeat calls
/// just leave the flag at true.
pub async fn mark_first_login_tour_seen(
    State(state): State<AppState>,
    axum::Extension(claims): axum::Extension<Claims>,
) -> Result<(StatusCode, Json<TourSeenResponse>), AppError> {
    let mailbox_id = mailbox_id_from_claims(&claims)?;

    let result = sqlx::query("UPDATE mailboxes SET first_login_tour_seen = true WHERE id = $1")
        .bind(mailbox_id)
        .execute(&state.db)
        .await?;

    if result.rows_affected() == 0 {
        return Err(AppError::NotFound(format!("mailbox {}", mailbox_id)));
    }

    Ok((StatusCode::OK, Json(TourSeenResponse { seen: true })))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::services::auth_service::Claims;

    fn claims_for(sub: &str) -> Claims {
        Claims {
            sub: sub.to_string(),
            username: "tour@example.test".to_string(),
            is_admin: false,
            is_compliance_officer: false,
            exp: 9_999_999_999,
            iat: 0,
        }
    }

    #[test]
    fn invalid_uuid_in_claims_returns_internal_error() {
        let bad = claims_for("not-a-uuid");
        let err = mailbox_id_from_claims(&bad).unwrap_err();
        match err {
            AppError::Internal(_) => {}
            other => panic!("expected Internal error, got {:?}", other),
        }
    }

    #[test]
    fn valid_uuid_in_claims_parses_cleanly() {
        let uuid = Uuid::new_v4();
        let good = claims_for(&uuid.to_string());
        assert_eq!(mailbox_id_from_claims(&good).unwrap(), uuid);
    }

    #[test]
    fn tour_seen_response_serializes_camel_safe() {
        let body = TourSeenResponse { seen: true };
        let json = serde_json::to_string(&body).unwrap();
        assert_eq!(json, "{\"seen\":true}");
    }
}
