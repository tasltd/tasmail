use axum::{
    extract::{Path, State},
    http::StatusCode,
    Json,
};
use uuid::Uuid;

use crate::error::AppError;
use crate::models::email_template::{CreateTemplate, EmailTemplate, RenderRequest, RenderResult, UpdateTemplate};
use crate::services::auth_service::Claims;
use crate::state::AppState;

/// Helper to parse mailbox_id from JWT claims
fn parse_mailbox_id(claims: &Claims) -> Result<Uuid, AppError> {
    claims
        .sub
        .parse()
        .map_err(|_| AppError::Internal(anyhow::anyhow!("Invalid mailbox ID")))
}

/// GET /api/templates — List all email templates for the current user
pub async fn list_templates(
    State(state): State<AppState>,
    axum::Extension(claims): axum::Extension<Claims>,
) -> Result<Json<Vec<EmailTemplate>>, AppError> {
    let mailbox_id = parse_mailbox_id(&claims)?;
    let templates = EmailTemplate::find_by_mailbox(&state.db, mailbox_id).await?;
    Ok(Json(templates))
}

/// POST /api/templates — Create a new email template
pub async fn create_template(
    State(state): State<AppState>,
    axum::Extension(claims): axum::Extension<Claims>,
    Json(body): Json<CreateTemplate>,
) -> Result<(StatusCode, Json<EmailTemplate>), AppError> {
    let mailbox_id = parse_mailbox_id(&claims)?;

    // Validate required fields are not empty
    if body.name.trim().is_empty() {
        return Err(AppError::BadRequest("Template name is required".to_string()));
    }
    if body.subject.trim().is_empty() {
        return Err(AppError::BadRequest("Template subject is required".to_string()));
    }

    let template = EmailTemplate::create(&state.db, mailbox_id, &body).await?;
    Ok((StatusCode::CREATED, Json(template)))
}

/// PUT /api/templates/{id} — Update an existing email template
pub async fn update_template(
    State(state): State<AppState>,
    axum::Extension(claims): axum::Extension<Claims>,
    Path(id): Path<Uuid>,
    Json(body): Json<UpdateTemplate>,
) -> Result<Json<EmailTemplate>, AppError> {
    let mailbox_id = parse_mailbox_id(&claims)?;

    let template = EmailTemplate::update(&state.db, id, mailbox_id, &body)
        .await?
        .ok_or_else(|| AppError::NotFound("Email template not found".to_string()))?;

    Ok(Json(template))
}

/// DELETE /api/templates/{id} — Delete an email template
pub async fn delete_template(
    State(state): State<AppState>,
    axum::Extension(claims): axum::Extension<Claims>,
    Path(id): Path<Uuid>,
) -> Result<StatusCode, AppError> {
    let mailbox_id = parse_mailbox_id(&claims)?;

    let deleted = EmailTemplate::delete(&state.db, id, mailbox_id).await?;
    if deleted {
        Ok(StatusCode::NO_CONTENT)
    } else {
        Err(AppError::NotFound("Email template not found".to_string()))
    }
}

/// POST /api/templates/{id}/render — Render a template with merge field values
pub async fn render_template(
    State(state): State<AppState>,
    axum::Extension(claims): axum::Extension<Claims>,
    Path(id): Path<Uuid>,
    Json(body): Json<RenderRequest>,
) -> Result<Json<RenderResult>, AppError> {
    let mailbox_id = parse_mailbox_id(&claims)?;

    let template = EmailTemplate::find_by_id(&state.db, id, mailbox_id)
        .await?
        .ok_or_else(|| AppError::NotFound("Email template not found".to_string()))?;

    let result = template.render(&body.fields);
    Ok(Json(result))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_create_template_request_full() {
        let json = r#"{
            "name": "Welcome Email",
            "subject": "Welcome {{first_name}} to {{company}}!",
            "body_html": "<h1>Hello {{first_name}}</h1><p>Welcome to {{company}}</p>",
            "body_text": "Hello {{first_name}}, Welcome to {{company}}",
            "merge_fields": ["first_name", "company"],
            "category": "Onboarding",
            "is_shared": true
        }"#;
        let req: CreateTemplate = serde_json::from_str(json).unwrap();
        assert_eq!(req.name, "Welcome Email");
        assert_eq!(req.merge_fields.as_ref().unwrap().len(), 2);
        assert_eq!(req.is_shared, Some(true));
    }

    #[test]
    fn test_create_template_request_minimal() {
        let json = r#"{
            "name": "Quick Reply",
            "subject": "Re: your inquiry",
            "body_html": "<p>Thank you for reaching out.</p>",
            "body_text": "Thank you for reaching out."
        }"#;
        let req: CreateTemplate = serde_json::from_str(json).unwrap();
        assert_eq!(req.name, "Quick Reply");
        assert!(req.merge_fields.is_none());
        assert!(req.category.is_none());
        assert!(req.is_shared.is_none());
    }

    #[test]
    fn test_update_template_request_partial() {
        let json = r#"{"name": "Updated Template Name"}"#;
        let req: UpdateTemplate = serde_json::from_str(json).unwrap();
        assert_eq!(req.name.as_deref(), Some("Updated Template Name"));
        assert!(req.subject.is_none());
        assert!(req.body_html.is_none());
        assert!(req.merge_fields.is_none());
    }

    #[test]
    fn test_update_template_request_multiple_fields() {
        let json = r#"{
            "subject": "New Subject",
            "category": "Marketing",
            "is_shared": false
        }"#;
        let req: UpdateTemplate = serde_json::from_str(json).unwrap();
        assert_eq!(req.subject.as_deref(), Some("New Subject"));
        assert_eq!(req.category.as_deref(), Some("Marketing"));
        assert_eq!(req.is_shared, Some(false));
        assert!(req.name.is_none());
    }

    #[test]
    fn test_render_request_deserialization() {
        let json = r#"{"fields": {"first_name": "Bob", "company": "TechCo"}}"#;
        let req: RenderRequest = serde_json::from_str(json).unwrap();
        assert_eq!(req.fields.len(), 2);
        assert_eq!(req.fields.get("first_name").unwrap(), "Bob");
        assert_eq!(req.fields.get("company").unwrap(), "TechCo");
    }

    #[test]
    fn test_render_request_empty_fields() {
        let json = r#"{"fields": {}}"#;
        let req: RenderRequest = serde_json::from_str(json).unwrap();
        assert!(req.fields.is_empty());
    }
}
