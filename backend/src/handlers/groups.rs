use axum::{
    extract::{Path, State},
    http::StatusCode,
    Json,
};
use uuid::Uuid;

use crate::error::AppError;
use crate::models::distribution_group::{
    AddMemberRequest, CreateGroupRequest, DistributionGroup, GroupMember,
    GroupWithCount, UpdateGroupRequest,
};
use crate::services::auth_service::Claims;
use crate::state::AppState;

fn parse_mailbox_id(claims: &Claims) -> Result<Uuid, AppError> {
    claims
        .sub
        .parse()
        .map_err(|_| AppError::Internal(anyhow::anyhow!("Invalid mailbox ID")))
}

/// GET /api/groups — List distribution groups owned by the current user
pub async fn list_groups(
    State(state): State<AppState>,
    axum::Extension(claims): axum::Extension<Claims>,
) -> Result<Json<Vec<DistributionGroup>>, AppError> {
    let mailbox_id = parse_mailbox_id(&claims)?;
    let groups = DistributionGroup::find_by_owner(&state.db, mailbox_id).await?;
    Ok(Json(groups))
}

/// POST /api/groups — Create a new distribution group
pub async fn create_group(
    State(state): State<AppState>,
    axum::Extension(claims): axum::Extension<Claims>,
    Json(body): Json<CreateGroupRequest>,
) -> Result<(StatusCode, Json<DistributionGroup>), AppError> {
    let mailbox_id = parse_mailbox_id(&claims)?;

    // Validate address format
    if !body.address.contains('@') {
        return Err(AppError::BadRequest("Invalid group address format".to_string()));
    }

    let group = DistributionGroup::create(&state.db, &body, mailbox_id).await?;
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
    axum::Extension(claims): axum::Extension<Claims>,
    Path(id): Path<Uuid>,
    Json(body): Json<UpdateGroupRequest>,
) -> Result<Json<DistributionGroup>, AppError> {
    let mailbox_id = parse_mailbox_id(&claims)?;

    // Verify ownership
    let existing = DistributionGroup::find_by_id(&state.db, id)
        .await?
        .ok_or_else(|| AppError::NotFound("Group not found".to_string()))?;

    if existing.owner_mailbox_id != mailbox_id && !claims.is_admin {
        return Err(AppError::Forbidden("Not the group owner".to_string()));
    }

    let group = DistributionGroup::update(&state.db, id, &body).await?;
    Ok(Json(group))
}

/// DELETE /api/groups/:id — Delete a distribution group
pub async fn delete_group(
    State(state): State<AppState>,
    axum::Extension(claims): axum::Extension<Claims>,
    Path(id): Path<Uuid>,
) -> Result<StatusCode, AppError> {
    let mailbox_id = parse_mailbox_id(&claims)?;

    let existing = DistributionGroup::find_by_id(&state.db, id)
        .await?
        .ok_or_else(|| AppError::NotFound("Group not found".to_string()))?;

    if existing.owner_mailbox_id != mailbox_id && !claims.is_admin {
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
    axum::Extension(claims): axum::Extension<Claims>,
    Path(id): Path<Uuid>,
    Json(body): Json<AddMemberRequest>,
) -> Result<(StatusCode, Json<GroupMember>), AppError> {
    let mailbox_id = parse_mailbox_id(&claims)?;

    let group = DistributionGroup::find_by_id(&state.db, id)
        .await?
        .ok_or_else(|| AppError::NotFound("Group not found".to_string()))?;

    if group.owner_mailbox_id != mailbox_id && !claims.is_admin {
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
    axum::Extension(claims): axum::Extension<Claims>,
    Path((id, address)): Path<(Uuid, String)>,
) -> Result<StatusCode, AppError> {
    let mailbox_id = parse_mailbox_id(&claims)?;

    let group = DistributionGroup::find_by_id(&state.db, id)
        .await?
        .ok_or_else(|| AppError::NotFound("Group not found".to_string()))?;

    if group.owner_mailbox_id != mailbox_id && !claims.is_admin {
        return Err(AppError::Forbidden("Not the group owner".to_string()));
    }

    GroupMember::remove(&state.db, id, &address).await?;
    Ok(StatusCode::NO_CONTENT)
}
