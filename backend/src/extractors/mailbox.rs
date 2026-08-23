use axum::{
    extract::FromRequestParts,
    http::request::Parts,
};
use async_trait::async_trait;
use uuid::Uuid;

use crate::error::AppError;
use crate::models::mailbox::Mailbox as MailboxModel;
use crate::services::auth_service::Claims;
use crate::state::AppState;

/// An Axum extractor that retrieves the `Mailbox` associated with the current user's claims.

/// This centralizes the logic for:
/// 1. Extracting `Claims` from request extensions.
/// 2. Parsing the `mailbox_id` from the `sub` field.
/// 3. Fetching the `Mailbox` from the database.
///
/// This promotes DRYness and modularity by removing repetitive boilerplate from handlers.
pub struct MailboxExtractor(pub MailboxModel);

#[async_trait]
impl<S> FromRequestParts<S> for MailboxExtractor
where
    AppState: From<S>,
    S: Send + Sync + 'static,
{
    type Rejection = AppError;

    async fn from_request_parts(parts: &mut Parts, _state: &S) -> Result<Self, Self::Rejection> {
        // 1. Extract Claims from extensions (populated by auth_middleware)
        let claims = parts
            .extensions
            .get::<Claims>()
            .ok_or_else(|| AppError::Unauthorized("Missing authentication claims".to_string()))?;

        // 2. Parse mailbox_id from claims.sub
        let mailbox_id: Uuid = claims.sub.parse()
            .map_err(|_| AppError::Internal(anyhow::anyhow!("Invalid mailbox ID in claims")))?;

        // 3. Fetch Mailbox from DB - use a fresh connection via the model's pool
        // The model's find_by_id uses sqlx::query_as which needs a PoolConnection.
        // We'll acquire from the AppState's pool directly.
        let mailbox = MailboxModel::find_by_id(&AppState::default().db, mailbox_id)
            .await?
            .ok_or_else(|| AppError::NotFound("Mailbox not found".to_string()))?;

        Ok(MailboxExtractor(mailbox))
    }
}
