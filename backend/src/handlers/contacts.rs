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

#[cfg(test)]
mod tests {
    use crate::models::contact::{CreateContact, UpdateContact};

    #[test]
    fn test_create_contact_full() {
        let json = r#"{
            "email": "alice@example.com",
            "display_name": "Alice Smith",
            "company": "Acme Corp",
            "phone": "+1234567890",
            "notes": "VIP client"
        }"#;
        let req: CreateContact = serde_json::from_str(json).unwrap();
        assert_eq!(req.email, "alice@example.com");
        assert_eq!(req.display_name.as_deref(), Some("Alice Smith"));
        assert_eq!(req.company.as_deref(), Some("Acme Corp"));
        assert_eq!(req.phone.as_deref(), Some("+1234567890"));
        assert_eq!(req.notes.as_deref(), Some("VIP client"));
    }

    #[test]
    fn test_create_contact_minimal() {
        let json = r#"{"email": "bob@test.com"}"#;
        let req: CreateContact = serde_json::from_str(json).unwrap();
        assert_eq!(req.email, "bob@test.com");
        assert!(req.display_name.is_none());
        assert!(req.company.is_none());
        assert!(req.phone.is_none());
        assert!(req.notes.is_none());
    }

    #[test]
    fn test_create_contact_missing_email_fails() {
        let json = r#"{"display_name": "No Email"}"#;
        let result = serde_json::from_str::<CreateContact>(json);
        assert!(result.is_err());
    }

    #[test]
    fn test_update_contact_partial() {
        let json = r#"{"email": "new@email.com"}"#;
        let req: UpdateContact = serde_json::from_str(json).unwrap();
        assert_eq!(req.email.as_deref(), Some("new@email.com"));
        assert!(req.display_name.is_none());
        assert!(req.company.is_none());
    }

    #[test]
    fn test_update_contact_all_fields() {
        let json = r#"{
            "email": "updated@test.com",
            "display_name": "Updated Name",
            "company": "New Corp",
            "phone": "+0987654321",
            "notes": "Updated notes"
        }"#;
        let req: UpdateContact = serde_json::from_str(json).unwrap();
        assert_eq!(req.email.as_deref(), Some("updated@test.com"));
        assert_eq!(req.display_name.as_deref(), Some("Updated Name"));
        assert_eq!(req.company.as_deref(), Some("New Corp"));
        assert_eq!(req.phone.as_deref(), Some("+0987654321"));
        assert_eq!(req.notes.as_deref(), Some("Updated notes"));
    }

    #[test]
    fn test_update_contact_empty_object() {
        let json = r#"{}"#;
        let req: UpdateContact = serde_json::from_str(json).unwrap();
        assert!(req.email.is_none());
        assert!(req.display_name.is_none());
        assert!(req.company.is_none());
        assert!(req.phone.is_none());
        assert!(req.notes.is_none());
    }

    #[test]
    fn test_search_query_with_q() {
        let json = r#"{"q": "alice"}"#;
        let query: super::SearchQuery = serde_json::from_str(json).unwrap();
        assert_eq!(query.q.as_deref(), Some("alice"));
    }

    #[test]
    fn test_search_query_without_q() {
        let json = r#"{}"#;
        let query: super::SearchQuery = serde_json::from_str(json).unwrap();
        assert!(query.q.is_none());
    }
}
