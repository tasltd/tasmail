use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    Json,
};
use serde::Deserialize;

use crate::error::AppError;
use crate::models::contact::{Contact, CreateContact, UpdateContact};
use crate::services::auth_service::Claims;
use crate::state::AppState;

#[derive(Debug, Deserialize)]
pub struct SearchQuery {
    pub q: Option<String>,
}

/// GET /api/contacts — list all contacts or search
pub async fn list_contacts(
    State(state): State<AppState>,
    axum::Extension(claims): axum::Extension<Claims>,
    Query(query): Query<SearchQuery>,
) -> Result<Json<Vec<Contact>>, AppError> {
    let mailbox_id = parse_mailbox_id(&claims)?;
    let contacts = if let Some(q) = &query.q {
        Contact::search(&state.db, mailbox_id, q).await?
    } else {
        Contact::find_by_mailbox(&state.db, mailbox_id).await?
    };
    Ok(Json(contacts))
}

/// POST /api/contacts — create a new contact
pub async fn create_contact(
    State(state): State<AppState>,
    axum::Extension(claims): axum::Extension<Claims>,
    Json(body): Json<CreateContact>,
) -> Result<(StatusCode, Json<Contact>), AppError> {
    let mailbox_id = parse_mailbox_id(&claims)?;
    let contact = Contact::create(&state.db, mailbox_id, &body).await?;
    Ok((StatusCode::CREATED, Json(contact)))
}

/// PUT /api/contacts/:id — update a contact
pub async fn update_contact(
    State(state): State<AppState>,
    axum::Extension(claims): axum::Extension<Claims>,
    Path(id): Path<uuid::Uuid>,
    Json(body): Json<UpdateContact>,
) -> Result<Json<Contact>, AppError> {
    let mailbox_id = parse_mailbox_id(&claims)?;
    let contact = Contact::update(&state.db, id, mailbox_id, &body)
        .await?
        .ok_or_else(|| AppError::NotFound("Contact not found".to_string()))?;
    Ok(Json(contact))
}

/// DELETE /api/contacts/:id — delete a contact
pub async fn delete_contact(
    State(state): State<AppState>,
    axum::Extension(claims): axum::Extension<Claims>,
    Path(id): Path<uuid::Uuid>,
) -> Result<StatusCode, AppError> {
    let mailbox_id = parse_mailbox_id(&claims)?;
    let deleted = Contact::delete(&state.db, id, mailbox_id).await?;
    if deleted {
        Ok(StatusCode::NO_CONTENT)
    } else {
        Err(AppError::NotFound("Contact not found".to_string()))
    }
}

fn parse_mailbox_id(claims: &Claims) -> Result<uuid::Uuid, AppError> {
    claims
        .sub
        .parse()
        .map_err(|_| AppError::Internal(anyhow::anyhow!("Invalid mailbox ID")))
}
