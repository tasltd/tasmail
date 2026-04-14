// Added: Phishing scan and report handlers for TMAIL-124

use axum::{
    extract::{Path, State},
    http::StatusCode,
    Json,
};
use uuid::Uuid;

use crate::error::AppError;
use crate::models::phishing_report::{PhishingReport, UpdatePhishingAction};
use crate::services::auth_service::Claims;
use crate::services::phishing_scanner;
use crate::state::AppState;

/// Added: Request body for triggering a phishing scan on a message
#[derive(Debug, serde::Deserialize)]
pub struct ScanRequest {
    pub html_body: String,
    pub sender_display_name: String,
    pub sender_email: String,
}

/// GET /api/folders/{folder}/messages/{uid}/phishing — Get phishing report for a message
pub async fn get_phishing_report(
    State(state): State<AppState>,
    axum::Extension(claims): axum::Extension<Claims>,
    Path((folder, uid)): Path<(String, i32)>,
) -> Result<Json<Option<PhishingReport>>, AppError> {
    let mailbox_id: Uuid = claims
        .sub
        .parse()
        .map_err(|_| AppError::Internal(anyhow::anyhow!("Invalid mailbox ID in JWT claims")))?;

    let report = PhishingReport::find_for_message(&state.db, mailbox_id, &folder, uid).await?;
    Ok(Json(report))
}

/// POST /api/folders/{folder}/messages/{uid}/phishing/scan — Trigger phishing scan on a message
pub async fn scan_message(
    State(state): State<AppState>,
    axum::Extension(claims): axum::Extension<Claims>,
    Path((folder, uid)): Path<(String, i32)>,
    Json(body): Json<ScanRequest>,
) -> Result<(StatusCode, Json<PhishingReport>), AppError> {
    let mailbox_id: Uuid = claims
        .sub
        .parse()
        .map_err(|_| AppError::Internal(anyhow::anyhow!("Invalid mailbox ID in JWT claims")))?;

    // Added: Run the heuristic phishing scanner on the provided email body
    let scan_result = phishing_scanner::scan_email(
        &body.html_body,
        &body.sender_display_name,
        &body.sender_email,
    );

    let suspicious_links_json = serde_json::to_value(&scan_result.suspicious_links)
        .map_err(|e| AppError::Internal(anyhow::anyhow!("Failed to serialize suspicious links: {}", e)))?;

    // Added: Persist the scan result to the database
    let report = PhishingReport::create(
        &state.db,
        mailbox_id,
        uid,
        &folder,
        suspicious_links_json,
        scan_result.suspicious_sender,
        scan_result.spoofed_display_name,
        scan_result.risk_score,
    )
    .await?;

    Ok((StatusCode::CREATED, Json(report)))
}

/// PUT /api/phishing/{id}/action — Update user action on a phishing report
pub async fn update_action(
    State(state): State<AppState>,
    axum::Extension(_claims): axum::Extension<Claims>,
    Path(id): Path<Uuid>,
    Json(body): Json<UpdatePhishingAction>,
) -> Result<StatusCode, AppError> {
    // Added: Validate user action value before updating
    let valid_actions = ["none", "dismissed", "reported", "confirmed_safe"];
    if !valid_actions.contains(&body.action.as_str()) {
        return Err(AppError::BadRequest(format!(
            "Invalid action '{}'. Must be one of: {}",
            body.action,
            valid_actions.join(", ")
        )));
    }

    let updated = PhishingReport::update_user_action(&state.db, id, &body.action).await?;
    if updated {
        Ok(StatusCode::NO_CONTENT)
    } else {
        Err(AppError::NotFound("Phishing report not found".to_string()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_scan_request_deserialization() {
        let json = r#"{
            "html_body": "<p>Hello <a href=\"https://evil.com\">paypal.com</a></p>",
            "sender_display_name": "PayPal",
            "sender_email": "scam@evil.com"
        }"#;
        let req: ScanRequest = serde_json::from_str(json).unwrap();
        assert_eq!(req.sender_display_name, "PayPal");
        assert_eq!(req.sender_email, "scam@evil.com");
        assert!(req.html_body.contains("evil.com"));
    }

    #[test]
    fn test_valid_action_values() {
        let valid_actions = ["none", "dismissed", "reported", "confirmed_safe"];
        for action_str in &valid_actions {
            let json = format!(r#"{{"action": "{}"}}"#, action_str);
            let action: UpdatePhishingAction = serde_json::from_str(&json).unwrap();
            assert_eq!(action.action, *action_str);
        }
    }

    #[test]
    fn test_invalid_action_is_parseable_but_would_be_rejected() {
        // NOTE: Deserialization succeeds — validation happens at handler level
        let json = r#"{"action": "delete_everything"}"#;
        let action: UpdatePhishingAction = serde_json::from_str(json).unwrap();
        let valid_actions = ["none", "dismissed", "reported", "confirmed_safe"];
        assert!(!valid_actions.contains(&action.action.as_str()));
    }
}
