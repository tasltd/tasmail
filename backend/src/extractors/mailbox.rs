use axum::{
    extract::{FromRef, FromRequestParts},
    http::request::Parts,
};
use uuid::Uuid;

use crate::error::AppError;
use crate::models::mailbox::Mailbox as MailboxModel;
use crate::services::auth_service::Claims;
use crate::state::AppState;

/// An Axum extractor that retrieves the `Mailbox` associated with the current user's claims.

/// This centralizes the logic for:
/// 1. Extracting `Claims` from request extensions.
/// 2. Parsing the `mailbox_id` from the `sub` field.
/// 3. Fetching the `Mailbox` from the database using the app state's pool.
///
/// Promotes DRYness and modularity by removing repetitive boilerplate from handlers.
pub struct MailboxExtractor(pub MailboxModel);

// Alias so both the tuple-struct name the handlers destructure with
// (`Mailbox(mailbox): Mailbox`) and the newer `MailboxExtractor` spelling
// resolve to the same extractor (TMAIL-311 refactor split).
pub use MailboxExtractor as Mailbox;

impl<S> FromRequestParts<S> for MailboxExtractor
where
    S: Send + Sync,
    AppState: FromRef<S>,
{
    type Rejection = AppError;

    async fn from_request_parts(parts: &mut Parts, state: &S) -> Result<Self, Self::Rejection> {
        // 1. Extract Claims from extensions (populated by auth_middleware)
        let claims = parts
            .extensions
            .get::<Claims>()
            .ok_or_else(|| AppError::Unauthorized("Missing authentication claims".to_string()))?;

        // 2-3. Fix: read AppState from the router state. It was read from
        // request extensions, which nothing populates, so every route using
        // this extractor returned 500 "Missing AppState in extensions".
        let app_state = AppState::from_ref(state);
        Ok(MailboxExtractor(load_mailbox(&app_state, claims).await?))
    }
}

/// Parse the mailbox id from `claims.sub` and load the mailbox.
///
/// Added: handlers that must reject hostile input before touching the
/// database (TMAIL-37: send, draft, search) take `Claims`, validate, then
/// call this. The extractor runs before the body is parsed, so it cannot
/// preserve that validate-first order.
pub async fn load_mailbox(state: &AppState, claims: &Claims) -> Result<MailboxModel, AppError> {
    let mailbox_id: Uuid = claims
        .sub
        .parse()
        .map_err(|_| AppError::Internal(anyhow::anyhow!("Invalid mailbox ID in claims")))?;

    MailboxModel::find_by_id(&state.db, mailbox_id)
        .await?
        .ok_or_else(|| AppError::NotFound("Mailbox not found".to_string()))
}
