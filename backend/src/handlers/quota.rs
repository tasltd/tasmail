// Changed: Added Redis caching for quota data — cached for 60 sec, invalidated on sync
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
/// Changed: Checks Redis cache first to avoid repeated DB + IMAP calls
pub async fn get_quota(
    State(state): State<AppState>,
    axum::Extension(claims): axum::Extension<Claims>,
) -> Result<Json<QuotaStatus>, AppError> {
    let mailbox_id: uuid::Uuid = claims
        .sub
        .parse()
        .map_err(|_| AppError::Internal(anyhow::anyhow!("Invalid mailbox ID in token")))?;

    // Added: Check Redis cache first
    if let Some(cached) = state.cache.get_quota::<QuotaStatus>(&claims.sub).await {
        return Ok(Json(cached));
    }

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

    // Added: Cache the quota status
    state.cache.set_quota(&claims.sub, &status).await;

    Ok(Json(status))
}

/// POST /api/quota/sync — Sync quota from IMAP server (triggers GETQUOTAROOT)
/// Changed: TMAIL-156 — migrated to ImapService::for_user (BYOK). The old path
/// called ImapService::new(state.config.imap.clone()) and forwarded
/// claims.username + a global master password, which under BYOK hits the empty
/// global Dovecot host instead of the user's per-mailbox IMAP server.
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

    // Fetch quota from the user's BYOK IMAP server using their stored creds.
    let imap_service = crate::services::imap_service::ImapService::for_user(&state, mailbox_id).await?;
    let (imap_user, imap_pass) = imap_service
        .user_creds()
        .ok_or_else(|| AppError::Internal(anyhow::anyhow!("BYOK IMAP credentials missing")))?;

    let (used_bytes, message_count) = imap_service
        .get_quota(imap_user, imap_pass)
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

    // Added: Invalidate stale cache and set fresh data
    state.cache.invalidate_quota(&claims.sub).await;
    state.cache.set_quota(&claims.sub, &status).await;

    Ok(Json(status))
}
