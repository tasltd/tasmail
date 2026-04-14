// Added: Contact group and vCard import/export/merge handlers for TMAIL-119
use axum::{
    extract::{Path, State},
    http::StatusCode,
    Json,
};
use serde::Deserialize;
use uuid::Uuid;

use crate::error::AppError;
use crate::models::contact::Contact;
use crate::models::contact_group::{
    AddMemberRequest, ContactGroup, ContactGroupMember, CreateContactGroup, UpdateContactGroup,
};
use crate::services::auth_service::Claims;
use crate::services::vcard_service::{self, ContactExport};
use crate::state::AppState;

fn parse_user_id(claims: &Claims) -> Result<Uuid, AppError> {
    claims
        .sub
        .parse()
        .map_err(|_| AppError::Internal(anyhow::anyhow!("Invalid user ID")))
}

/// GET /api/contact-groups — List all contact groups for the current user
pub async fn list_groups(
    State(state): State<AppState>,
    axum::Extension(claims): axum::Extension<Claims>,
) -> Result<Json<Vec<ContactGroup>>, AppError> {
    let user_id = parse_user_id(&claims)?;
    let groups = ContactGroup::list_by_user(&state.db, user_id).await?;
    Ok(Json(groups))
}

/// POST /api/contact-groups — Create a new contact group
pub async fn create_group(
    State(state): State<AppState>,
    axum::Extension(claims): axum::Extension<Claims>,
    Json(body): Json<CreateContactGroup>,
) -> Result<(StatusCode, Json<ContactGroup>), AppError> {
    let user_id = parse_user_id(&claims)?;

    if body.name.trim().is_empty() {
        return Err(AppError::BadRequest("Group name cannot be empty".to_string()));
    }

    let group = ContactGroup::create(&state.db, user_id, &body).await?;
    Ok((StatusCode::CREATED, Json(group)))
}

/// PUT /api/contact-groups/{id} — Update a contact group
pub async fn update_group(
    State(state): State<AppState>,
    axum::Extension(claims): axum::Extension<Claims>,
    Path(id): Path<Uuid>,
    Json(body): Json<UpdateContactGroup>,
) -> Result<Json<ContactGroup>, AppError> {
    let user_id = parse_user_id(&claims)?;
    let group = ContactGroup::update(&state.db, id, user_id, &body)
        .await?
        .ok_or_else(|| AppError::NotFound("Contact group not found".to_string()))?;
    Ok(Json(group))
}

/// DELETE /api/contact-groups/{id} — Delete a contact group
pub async fn delete_group(
    State(state): State<AppState>,
    axum::Extension(claims): axum::Extension<Claims>,
    Path(id): Path<Uuid>,
) -> Result<StatusCode, AppError> {
    let user_id = parse_user_id(&claims)?;
    let deleted = ContactGroup::delete(&state.db, id, user_id).await?;
    if deleted {
        Ok(StatusCode::NO_CONTENT)
    } else {
        Err(AppError::NotFound("Contact group not found".to_string()))
    }
}

/// POST /api/contact-groups/{id}/members — Add a contact to a group
pub async fn add_member(
    State(state): State<AppState>,
    axum::Extension(claims): axum::Extension<Claims>,
    Path(group_id): Path<Uuid>,
    Json(body): Json<AddMemberRequest>,
) -> Result<(StatusCode, Json<ContactGroupMember>), AppError> {
    let user_id = parse_user_id(&claims)?;

    // NOTE: Verify the group belongs to the user
    ContactGroup::find_by_id(&state.db, group_id, user_id)
        .await?
        .ok_or_else(|| AppError::NotFound("Contact group not found".to_string()))?;

    let member = ContactGroupMember::add_to_group(&state.db, group_id, body.contact_id).await?;
    Ok((StatusCode::CREATED, Json(member)))
}

/// DELETE /api/contact-groups/{id}/members/{contact_id} — Remove a contact from a group
pub async fn remove_member(
    State(state): State<AppState>,
    axum::Extension(claims): axum::Extension<Claims>,
    Path((group_id, contact_id)): Path<(Uuid, Uuid)>,
) -> Result<StatusCode, AppError> {
    let user_id = parse_user_id(&claims)?;

    // NOTE: Verify the group belongs to the user
    ContactGroup::find_by_id(&state.db, group_id, user_id)
        .await?
        .ok_or_else(|| AppError::NotFound("Contact group not found".to_string()))?;

    let removed = ContactGroupMember::remove_from_group(&state.db, group_id, contact_id).await?;
    if removed {
        Ok(StatusCode::NO_CONTENT)
    } else {
        Err(AppError::NotFound("Member not found in group".to_string()))
    }
}

/// GET /api/contact-groups/{id}/contacts — List contacts in a group
pub async fn list_group_contacts(
    State(state): State<AppState>,
    axum::Extension(claims): axum::Extension<Claims>,
    Path(group_id): Path<Uuid>,
) -> Result<Json<Vec<Contact>>, AppError> {
    let user_id = parse_user_id(&claims)?;

    // NOTE: Verify the group belongs to the user
    ContactGroup::find_by_id(&state.db, group_id, user_id)
        .await?
        .ok_or_else(|| AppError::NotFound("Contact group not found".to_string()))?;

    let contact_ids = ContactGroupMember::list_contact_ids_in_group(&state.db, group_id).await?;

    // NOTE: Fetch each contact — only return those belonging to the user
    let mut contacts = Vec::new();
    for cid in contact_ids {
        if let Some(c) = Contact::find_by_id(&state.db, cid, user_id).await? {
            contacts.push(c);
        }
    }
    Ok(Json(contacts))
}

// PURPOSE: Request body for vCard import
#[derive(Debug, Deserialize)]
pub struct ImportVcardRequest {
    pub vcard_text: String,
}

/// POST /api/contacts/import-vcard — Import contacts from vCard text
pub async fn import_vcard(
    State(state): State<AppState>,
    axum::Extension(claims): axum::Extension<Claims>,
    Json(body): Json<ImportVcardRequest>,
) -> Result<(StatusCode, Json<Vec<Contact>>), AppError> {
    let mailbox_id = parse_user_id(&claims)?;
    let parsed = vcard_service::parse_vcard(&body.vcard_text);

    if parsed.is_empty() {
        return Err(AppError::BadRequest("No valid contacts found in vCard data".to_string()));
    }

    let mut imported = Vec::new();
    for ci in parsed {
        let create = crate::models::contact::CreateContact {
            email: ci.email,
            display_name: Some(ci.name),
            company: ci.company,
            phone: ci.phone,
            notes: None,
        };
        // NOTE: Skip duplicates — if email already exists for this user, skip silently
        match Contact::create(&state.db, mailbox_id, &create).await {
            Ok(contact) => imported.push(contact),
            Err(_) => continue,
        }
    }

    Ok((StatusCode::CREATED, Json(imported)))
}

/// GET /api/contacts/export-vcard — Export all contacts as vCard text
pub async fn export_vcard(
    State(state): State<AppState>,
    axum::Extension(claims): axum::Extension<Claims>,
) -> Result<(StatusCode, [(axum::http::header::HeaderName, &'static str); 2], String), AppError> {
    let mailbox_id = parse_user_id(&claims)?;
    let contacts = Contact::find_by_mailbox(&state.db, mailbox_id).await?;

    let exports: Vec<ContactExport> = contacts
        .into_iter()
        .map(|c| ContactExport {
            email: c.email,
            display_name: c.display_name,
            phone: c.phone,
            company: c.company,
        })
        .collect();

    let vcard_text = vcard_service::export_vcard(&exports);

    Ok((
        StatusCode::OK,
        [
            (axum::http::header::CONTENT_TYPE, "text/vcard; charset=utf-8"),
            (axum::http::header::CONTENT_DISPOSITION, "attachment; filename=\"contacts.vcf\""),
        ],
        vcard_text,
    ))
}

// PURPOSE: Request body for merging duplicate contacts
#[derive(Debug, Deserialize)]
pub struct MergeContactsRequest {
    pub contact_ids: Vec<Uuid>,
}

/// POST /api/contacts/merge — Merge duplicate contacts (keep first, delete rest)
pub async fn merge_contacts(
    State(state): State<AppState>,
    axum::Extension(claims): axum::Extension<Claims>,
    Json(body): Json<MergeContactsRequest>,
) -> Result<Json<Contact>, AppError> {
    let mailbox_id = parse_user_id(&claims)?;

    if body.contact_ids.len() < 2 {
        return Err(AppError::BadRequest("At least 2 contact IDs required to merge".to_string()));
    }

    // NOTE: The first ID is the primary (kept), rest are deleted
    let primary_id = body.contact_ids[0];

    let primary = Contact::find_by_id(&state.db, primary_id, mailbox_id)
        .await?
        .ok_or_else(|| AppError::NotFound("Primary contact not found".to_string()))?;

    // NOTE: Delete all secondary contacts
    for &cid in &body.contact_ids[1..] {
        Contact::delete(&state.db, cid, mailbox_id).await?;
    }

    Ok(Json(primary))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_import_vcard_request_deserialization() {
        let json = serde_json::json!({
            "vcard_text": "BEGIN:VCARD\nVERSION:3.0\nFN:Test\nEMAIL:test@test.com\nEND:VCARD"
        });

        let req: ImportVcardRequest = serde_json::from_value(json).unwrap();
        assert!(req.vcard_text.contains("BEGIN:VCARD"));
    }

    #[test]
    fn test_merge_contacts_request_deserialization() {
        let id1 = Uuid::new_v4();
        let id2 = Uuid::new_v4();
        let json = serde_json::json!({
            "contact_ids": [id1.to_string(), id2.to_string()]
        });

        let req: MergeContactsRequest = serde_json::from_value(json).unwrap();
        assert_eq!(req.contact_ids.len(), 2);
        assert_eq!(req.contact_ids[0], id1);
        assert_eq!(req.contact_ids[1], id2);
    }

    #[test]
    fn test_merge_contacts_request_empty() {
        let json = serde_json::json!({
            "contact_ids": []
        });

        let req: MergeContactsRequest = serde_json::from_value(json).unwrap();
        assert!(req.contact_ids.is_empty());
    }

    #[test]
    fn test_import_vcard_request_empty_text() {
        let json = serde_json::json!({
            "vcard_text": ""
        });

        let req: ImportVcardRequest = serde_json::from_value(json).unwrap();
        assert!(req.vcard_text.is_empty());
    }
}
