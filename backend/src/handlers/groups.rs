use axum::{
    extract::{Path, State},
    http::StatusCode,
    Json,
    Extension,
};
use uuid::Uuid;

use crate::error::AppError;
use crate::extractors::Mailbox;
use crate::models::distribution_group::{
    AddMemberRequest, CreateGroupRequest, DistributionGroup, GroupMember,
    UpdateGroupRequest,
};
use crate::services::auth_service::Claims;
use crate::state::AppState;

/// GET /api/groups — List distribution groups owned by the current user
pub async fn list_groups(
    State(state): State<AppState>,
    mailbox: Mailbox,
) -> Result<Json<Vec<DistributionGroup>>, AppError> {
    let groups = DistributionGroup::find_by_owner(&state.db, mailbox.0.id).await?;
    Ok(Json(groups))
}

/// POST /api/groups — Create a new distribution group
pub async fn create_group(
    State(state): State<AppState>,
    mailbox: Mailbox,
    Json(body): Json<CreateGroupRequest>,
) -> Result<(StatusCode, Json<DistributionGroup>), AppError> {
    // Validate address format
    if !body.address.contains('@') {
        return Err(AppError::BadRequest("Invalid group address format".to_string()));
    }

    // Fix (TMAIL-286): the SPA GroupManager had no way to look up the user's
    // domain so it was sending an empty string. Falling back to the owner's
    // mailbox.domain_id matches single-domain BYOK reality and avoids hitting
    // the FK constraint with garbage. Admins on multi-domain deployments
    // can still pin a domain explicitly in the request body.
    let resolved_domain_id = body.domain_id.unwrap_or(mailbox.0.domain_id);

    let group = DistributionGroup::create(&state.db, &body, mailbox.0.id, resolved_domain_id).await?;
    Ok((StatusCode::CREATED, Json(group)))
}

/// GET /api/groups/:id — Get a specific distribution group
pub async fn get_group(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
) -> Result<Json<DistributionGroup>, AppError> {
    let group = DistributionGroup::find_by_id(&state.db, id)
        .await?
        .ok_or_else(|| AppError::NotFound("Group not found".to_string()))?;
    Ok(Json(group))
}

/// PUT /api/groups/:id — Update a distribution group
pub async fn update_group(
    State(state): State<AppState>,
    mailbox: Mailbox,
    Extension(claims): Extension<Claims>,
    Path(id): Path<Uuid>,
    Json(body): Json<UpdateGroupRequest>,
) -> Result<Json<DistributionGroup>, AppError> {
    // Verify ownership
    let existing = DistributionGroup::find_by_id(&state.db, id)
        .await?
        .ok_or_else(|| AppError::NotFound("Group not found".to_string()))?;

    if existing.owner_mailbox_id != mailbox.0.id && !claims.is_admin {
        return Err(AppError::Forbidden("Not the group owner".to_string()));
    }

    let group = DistributionGroup::update(&state.db, id, &body).await?;
    Ok(Json(group))
}

/// DELETE /api/groups/:id — Delete a distribution group
pub async fn delete_group(
    State(state): State<AppState>,
    mailbox: Mailbox,
    Extension(claims): Extension<Claims>,
    Path(id): Path<Uuid>,
) -> Result<StatusCode, AppError> {
    let existing = DistributionGroup::find_by_id(&state.db, id)
        .await?
        .ok_or_else(|| AppError::NotFound("Group not found".to_string()))?;

    if existing.owner_mailbox_id != mailbox.0.id && !claims.is_admin {
        return Err(AppError::Forbidden("Not the group owner".to_string()));
    }

    DistributionGroup::delete(&state.db, id).await?;
    Ok(StatusCode::NO_CONTENT)
}

/// GET /api/groups/:id/members — List group members
pub async fn list_members(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
) -> Result<Json<Vec<GroupMember>>, AppError> {
    // Verify group exists (RLS will enforce access)
    DistributionGroup::find_by_id(&state.db, id)
        .await?
        .ok_or_else(|| AppError::NotFound("Group not found".to_string()))?;

    let members = GroupMember::list_by_group(&state.db, id).await?;
    Ok(Json(members))
}

/// POST /api/groups/:id/members — Add a member to a group
pub async fn add_member(
    State(state): State<AppState>,
    mailbox: Mailbox,
    Extension(claims): Extension<Claims>,
    Path(id): Path<Uuid>,
    Json(body): Json<AddMemberRequest>,
) -> Result<(StatusCode, Json<GroupMember>), AppError> {
    let group = DistributionGroup::find_by_id(&state.db, id)
        .await?
        .ok_or_else(|| AppError::NotFound("Group not found".to_string()))?;

    if group.owner_mailbox_id != mailbox.0.id && !claims.is_admin {
        return Err(AppError::Forbidden("Not the group owner".to_string()));
    }

    // Validate member address
    if !body.member_address.contains('@') {
        return Err(AppError::BadRequest("Invalid member email address".to_string()));
    }

    let member = GroupMember::add(&state.db, id, &body).await?;
    Ok((StatusCode::CREATED, Json(member)))
}

/// DELETE /api/groups/:id/members/:address — Remove a member from a group
pub async fn remove_member(
    State(state): State<AppState>,
    mailbox: Mailbox,
    Extension(claims): Extension<Claims>,
    Path((id, address)): Path<(Uuid, String)>,
) -> Result<StatusCode, AppError> {
    let group = DistributionGroup::find_by_id(&state.db, id)
        .await?
        .ok_or_else(|| AppError::NotFound("Group not found".to_string()))?;

    if group.owner_mailbox_id != mailbox.0.id && !claims.is_admin {
        return Err(AppError::Forbidden("Not the group owner".to_string()));
    }

    GroupMember::remove(&state.db, id, &address).await?;
    Ok(StatusCode::NO_CONTENT)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::services::auth_service::Claims;

    #[test]
    fn test_create_group_request_deserialization() {
        let json = r#"{"name": "Team", "address": "team@example.com", "domain_id": "550e8400-e29b-41d4-a716-446655440000", "description": "Dev team"}"#;
        let req: CreateGroupRequest = serde_json::from_str(json).unwrap();
        assert_eq!(req.name, "Team");
        assert_eq!(req.address, "team@example.com");
        assert_eq!(req.description, Some("Dev team".to_string()));
        assert!(req.domain_id.is_some());
    }

    #[test]
    fn test_create_group_request_without_domain_id_deserialization() {
        let json = r#"{"name": "Team", "address": "team@example.com"}"#;
        let req: CreateGroupRequest = serde_json::from_str(json).unwrap();
        assert_eq!(req.name, "Team");
        assert!(req.domain_id.is_none());
    }

    #[test]
    fn test_add_member_request_deserialization() {
        let json = r#"{"member_address": "alice@example.com"}"#;
        let req: AddMemberRequest = serde_json::from_str(json).unwrap();
        assert_eq!(req.member_address, "alice@example.com");
        assert!(req.mailbox_id.is_none());
    }

    #[test]
    fn test_update_group_request_all_fields() {
        let json = r#"{"name": "Updated", "description": "New desc"}"#;
        let req: UpdateGroupRequest = serde_json::from_str(json).unwrap();
        assert_eq!(req.name, Some("Updated".to_string()));
        assert_eq!(req.description, Some("New desc".to_string()));
    }

    #[test]
    fn test_update_group_request_partial() {
        let json = r#"{"name": "Just Name"}"#;
        let req: UpdateGroupRequest = serde_json::from_str(json).unwrap();
        assert_eq!(req.name, Some("Just Name".to_string()));
        assert!(req.description.is_none());
    }
}
