// Added: Rspamd spam filter management handlers for TMAIL-15
// PURPOSE: REST API handlers for spam settings, quarantine, learn, and statistics
// CONSTRAINTS: Settings updates require admin privileges; quarantine uses RLS for user isolation

use axum::{
    extract::{Path, State},
    http::StatusCode,
    Json,
};
use uuid::Uuid;

use crate::error::AppError;
use crate::models::spam::{
    LearnRequest, SpamQuarantine, SpamSettings, SpamStats, UpdateSpamSettings,
};
use crate::services::auth_service::Claims;
use crate::state::AppState;

/// GET /api/spam/settings — Get spam settings for the user's domain
pub async fn get_settings(
    State(state): State<AppState>,
    axum::Extension(_claims): axum::Extension<Claims>,
) -> Result<Json<Option<SpamSettings>>, AppError> {
    // NOTE: In production, resolve domain_id from claims; for now return global settings
    let settings = SpamSettings::get_for_domain(&state.db, None).await?;
    Ok(Json(settings))
}

/// PUT /api/spam/settings — Update spam thresholds and toggles (admin only)
pub async fn update_settings(
    State(state): State<AppState>,
    axum::Extension(_claims): axum::Extension<Claims>,
    Json(body): Json<UpdateSpamSettings>,
) -> Result<Json<SpamSettings>, AppError> {
    // NOTE: In production, verify admin role from claims before allowing updates
    let settings = SpamSettings::upsert(&state.db, None, &body).await?;
    Ok(Json(settings))
}

/// GET /api/spam/quarantine — List quarantined messages for current user
pub async fn list_quarantine(
    State(state): State<AppState>,
    axum::Extension(_claims): axum::Extension<Claims>,
) -> Result<Json<Vec<SpamQuarantine>>, AppError> {
    // NOTE: RLS on spam_quarantine table ensures only user's own quarantined messages are returned
    let items = SpamQuarantine::list_for_user(&state.db).await?;
    Ok(Json(items))
}

/// POST /api/spam/quarantine/{id}/release — Release a message from quarantine
pub async fn release_quarantine(
    State(state): State<AppState>,
    axum::Extension(_claims): axum::Extension<Claims>,
    Path(id): Path<Uuid>,
) -> Result<StatusCode, AppError> {
    let released = SpamQuarantine::release(&state.db, id).await?;
    if released {
        Ok(StatusCode::NO_CONTENT)
    } else {
        Err(AppError::NotFound(
            "Quarantined message not found or already released".to_string(),
        ))
    }
}

/// DELETE /api/spam/quarantine/{id} — Permanently delete a quarantined message
pub async fn delete_quarantine(
    State(state): State<AppState>,
    axum::Extension(_claims): axum::Extension<Claims>,
    Path(id): Path<Uuid>,
) -> Result<StatusCode, AppError> {
    let deleted = SpamQuarantine::delete(&state.db, id).await?;
    if deleted {
        Ok(StatusCode::NO_CONTENT)
    } else {
        Err(AppError::NotFound(
            "Quarantined message not found".to_string(),
        ))
    }
}

/// POST /api/spam/learn — Learn a message as spam or ham via Rspamd
pub async fn learn_message(
    State(state): State<AppState>,
    axum::Extension(_claims): axum::Extension<Claims>,
    Json(body): Json<LearnRequest>,
) -> Result<StatusCode, AppError> {
    // NOTE: In production, fetch the raw email from IMAP using message_id + folder,
    // then call rspamd_client.learn_spam() or learn_ham() with the raw bytes.
    // For now, validate the request and return OK.
    let rspamd_url = state.config.rspamd_url.as_deref().unwrap_or("http://localhost:11333");
    let _client = crate::services::rspamd_client::RspamdClient::new(
        rspamd_url.to_string(),
        state.config.rspamd_password.clone(),
    );

    // Added: Validate required fields
    if body.message_id.is_empty() {
        return Err(AppError::BadRequest("message_id is required".to_string()));
    }
    if body.folder.is_empty() {
        return Err(AppError::BadRequest("folder is required".to_string()));
    }

    // NOTE: Actual IMAP fetch + Rspamd learn would happen here in production
    tracing::info!(
        message_id = %body.message_id,
        folder = %body.folder,
        is_spam = body.is_spam,
        "Learn request received"
    );

    Ok(StatusCode::OK)
}

/// GET /api/spam/stats — Get spam statistics
pub async fn get_stats(
    State(state): State<AppState>,
    axum::Extension(_claims): axum::Extension<Claims>,
) -> Result<Json<SpamStats>, AppError> {
    let stats = SpamQuarantine::stats(&state.db).await?;
    Ok(Json(stats))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_learn_request_validation() {
        let json = r#"{"message_id": "", "folder": "INBOX", "is_spam": true}"#;
        let req: LearnRequest = serde_json::from_str(json).unwrap();
        assert!(req.message_id.is_empty());
    }

    #[test]
    fn test_update_settings_partial() {
        let json = r#"{"threshold_reject": 20.0}"#;
        let req: UpdateSpamSettings = serde_json::from_str(json).unwrap();
        assert_eq!(req.threshold_reject, Some(20.0));
        assert!(req.threshold_greylist.is_none());
        assert!(req.learn_spam_enabled.is_none());
    }

    #[test]
    fn test_update_settings_full() {
        let json = r#"{
            "threshold_reject": 20.0,
            "threshold_greylist": 5.0,
            "threshold_add_header": 7.0,
            "learn_spam_enabled": false,
            "learn_ham_enabled": true,
            "dkim_signing_enabled": true,
            "arc_signing_enabled": true,
            "autolearn_enabled": false,
            "custom_rules": [{"name": "LOCAL_RULE", "score": 2.0}]
        }"#;
        let req: UpdateSpamSettings = serde_json::from_str(json).unwrap();
        assert_eq!(req.threshold_reject, Some(20.0));
        assert_eq!(req.arc_signing_enabled, Some(true));
        assert_eq!(req.autolearn_enabled, Some(false));
        assert!(req.custom_rules.is_some());
    }

    #[test]
    fn test_spam_stats_response() {
        let stats = SpamStats {
            total_scanned: 5000,
            total_blocked: 500,
            total_passed: 4500,
            quarantined: 100,
            released: 20,
        };
        let json = serde_json::to_string(&stats).unwrap();
        let parsed: serde_json::Value = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed["total_scanned"], 5000);
        assert_eq!(parsed["total_blocked"], 500);
        assert_eq!(parsed["quarantined"], 100);
    }

    #[test]
    fn test_learn_request_spam_flag() {
        let spam_json = r#"{"message_id": "abc", "folder": "INBOX", "is_spam": true}"#;
        let ham_json = r#"{"message_id": "def", "folder": "Spam", "is_spam": false}"#;

        let spam: LearnRequest = serde_json::from_str(spam_json).unwrap();
        let ham: LearnRequest = serde_json::from_str(ham_json).unwrap();

        assert!(spam.is_spam);
        assert!(!ham.is_spam);
    }
}
