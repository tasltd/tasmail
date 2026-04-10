use axum::{
    extract::State,
    Json,
};

use crate::error::AppError;
use crate::models::mailbox::Mailbox;
use crate::models::quota::{QuotaStatus, QuotaUsage};
use crate::services::auth_service::Claims;
use crate::state::AppState;

/// GET /api/quota — Get current user's quota status
pub async fn get_quota(
    State(state): State<AppState>,
    axum::Extension(claims): axum::Extension<Claims>,
) -> Result<Json<QuotaStatus>, AppError> {
    let mailbox_id: uuid::Uuid = claims
        .sub
        .parse()
        .map_err(|_| AppError::Internal(anyhow::anyhow!("Invalid mailbox ID in token")))?;

    let mailbox = Mailbox::find_by_id(&state.db, mailbox_id)
        .await?
        .ok_or_else(|| AppError::NotFound("Mailbox not found".to_string()))?;

    let usage = QuotaUsage::find_by_mailbox(&state.db, mailbox_id).await?;

    let status = QuotaUsage::to_status(
        usage.as_ref(),
        mailbox.quota_bytes,
        mailbox.quota_warn_percent,
        mailbox_id,
    );

    Ok(Json(status))
}

/// POST /api/quota/sync — Sync quota from IMAP server (triggers GETQUOTAROOT)
pub async fn sync_quota(
    State(state): State<AppState>,
    axum::Extension(claims): axum::Extension<Claims>,
) -> Result<Json<QuotaStatus>, AppError> {
    let mailbox_id: uuid::Uuid = claims
        .sub
        .parse()
        .map_err(|_| AppError::Internal(anyhow::anyhow!("Invalid mailbox ID in token")))?;

    let mailbox = Mailbox::find_by_id(&state.db, mailbox_id)
        .await?
        .ok_or_else(|| AppError::NotFound("Mailbox not found".to_string()))?;

    // Fetch quota from IMAP GETQUOTAROOT
    let imap_config = &state.config.imap;
    let imap_service = crate::services::imap_service::ImapService::new(imap_config.clone());

    let (used_bytes, message_count) = imap_service
        .get_quota(&claims.username, &get_imap_password(&state, mailbox_id).await?)
        .await
        .unwrap_or_else(|e| {
            tracing::warn!("Failed to fetch IMAP quota: {}", e);
            (0, 0)
        });

    // Update database
    let usage = QuotaUsage::upsert(&state.db, mailbox_id, used_bytes, message_count).await?;

    let status = QuotaUsage::to_status(
        Some(&usage),
        mailbox.quota_bytes,
        mailbox.quota_warn_percent,
        mailbox_id,
    );

    Ok(Json(status))
}

/// Retrieve the IMAP password for a mailbox from the session or stored credentials
/// NOTE: In production, this would use stored IMAP credentials or a master password
async fn get_imap_password(state: &AppState, _mailbox_id: uuid::Uuid) -> Result<String, AppError> {
    // For now, use a configured master password or service account
    // In a full implementation, passwords would be stored encrypted
    state.config.imap.master_password.clone().ok_or_else(|| {
        AppError::Internal(anyhow::anyhow!(
            "IMAP master password not configured for quota sync"
        ))
    })
}
