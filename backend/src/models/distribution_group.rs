use serde::{Deserialize, Serialize};
use sqlx::{FromRow, PgPool};
use uuid::Uuid;

/// Distribution group (email list) backed by Postfix virtual aliases.
#[derive(Debug, Clone, Serialize, FromRow)]
pub struct DistributionGroup {
    pub id: Uuid,
    pub domain_id: Uuid,
    pub name: String,
    pub address: String,
    pub description: Option<String>,
    pub owner_mailbox_id: Uuid,
    pub allow_external: bool,
    pub active: bool,
    pub created_at: chrono::DateTime<chrono::Utc>,
    pub updated_at: chrono::DateTime<chrono::Utc>,
}

#[derive(Debug, Clone, Serialize, FromRow)]
pub struct GroupMember {
    pub id: Uuid,
    pub group_id: Uuid,
    pub member_address: String,
    pub mailbox_id: Option<Uuid>,
    pub added_at: chrono::DateTime<chrono::Utc>,
}

#[derive(Debug, Deserialize)]
pub struct CreateGroupRequest {
    pub name: String,
    pub address: String,
    pub domain_id: Uuid,
    pub description: Option<String>,
    pub allow_external: Option<bool>,
}

#[derive(Debug, Deserialize)]
pub struct UpdateGroupRequest {
    pub name: Option<String>,
    pub description: Option<String>,
    pub allow_external: Option<bool>,
    pub active: Option<bool>,
}

#[derive(Debug, Deserialize)]
pub struct AddMemberRequest {
    pub member_address: String,
    pub mailbox_id: Option<Uuid>,
}

// Added: Group with member count for list responses
#[derive(Debug, Serialize)]
pub struct GroupWithCount {
    #[serde(flatten)]
    pub group: DistributionGroup,
    pub member_count: i64,
}

impl DistributionGroup {
    pub async fn create(
        pool: &PgPool,
        req: &CreateGroupRequest,
        owner_id: Uuid,
    ) -> Result<DistributionGroup, sqlx::Error> {
        sqlx::query_as::<_, DistributionGroup>(
            "INSERT INTO distribution_groups (domain_id, name, address, description, owner_mailbox_id, allow_external)
             VALUES ($1, $2, $3, $4, $5, $6)
             RETURNING *"
        )
        .bind(req.domain_id)
        .bind(&req.name)
        .bind(&req.address)
        .bind(&req.description)
        .bind(owner_id)
        .bind(req.allow_external.unwrap_or(false))
        .fetch_one(pool)
        .await
    }

    pub async fn find_by_owner(pool: &PgPool, owner_id: Uuid) -> Result<Vec<DistributionGroup>, sqlx::Error> {
        sqlx::query_as::<_, DistributionGroup>(
            "SELECT * FROM distribution_groups WHERE owner_mailbox_id = $1 ORDER BY name"
        )
        .bind(owner_id)
        .fetch_all(pool)
        .await
    }

    pub async fn find_by_id(pool: &PgPool, id: Uuid) -> Result<Option<DistributionGroup>, sqlx::Error> {
        sqlx::query_as::<_, DistributionGroup>(
            "SELECT * FROM distribution_groups WHERE id = $1"
        )
        .bind(id)
        .fetch_optional(pool)
        .await
    }

    pub async fn update(
        pool: &PgPool,
        id: Uuid,
        req: &UpdateGroupRequest,
    ) -> Result<DistributionGroup, sqlx::Error> {
        sqlx::query_as::<_, DistributionGroup>(
            "UPDATE distribution_groups SET
                name = COALESCE($2, name),
                description = COALESCE($3, description),
                allow_external = COALESCE($4, allow_external),
                active = COALESCE($5, active),
                updated_at = NOW()
             WHERE id = $1
             RETURNING *"
        )
        .bind(id)
        .bind(&req.name)
        .bind(&req.description)
        .bind(req.allow_external)
        .bind(req.active)
        .fetch_one(pool)
        .await
    }

    pub async fn delete(pool: &PgPool, id: Uuid) -> Result<(), sqlx::Error> {
        sqlx::query("DELETE FROM distribution_groups WHERE id = $1")
            .bind(id)
            .execute(pool)
            .await?;
        Ok(())
    }
}

impl GroupMember {
    pub async fn add(
        pool: &PgPool,
        group_id: Uuid,
        req: &AddMemberRequest,
    ) -> Result<GroupMember, sqlx::Error> {
        sqlx::query_as::<_, GroupMember>(
            "INSERT INTO group_members (group_id, member_address, mailbox_id)
             VALUES ($1, $2, $3)
             ON CONFLICT (group_id, member_address) DO NOTHING
             RETURNING *"
        )
        .bind(group_id)
        .bind(&req.member_address)
        .bind(req.mailbox_id)
        .fetch_one(pool)
        .await
    }

    pub async fn list_by_group(pool: &PgPool, group_id: Uuid) -> Result<Vec<GroupMember>, sqlx::Error> {
        sqlx::query_as::<_, GroupMember>(
            "SELECT * FROM group_members WHERE group_id = $1 ORDER BY member_address"
        )
        .bind(group_id)
        .fetch_all(pool)
        .await
    }

    pub async fn remove(pool: &PgPool, group_id: Uuid, member_address: &str) -> Result<(), sqlx::Error> {
        sqlx::query("DELETE FROM group_members WHERE group_id = $1 AND member_address = $2")
            .bind(group_id)
            .bind(member_address)
            .execute(pool)
            .await?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_create_group_request_deserialization() {
        let json = r#"{"name":"Engineering","address":"eng@example.com","domain_id":"00000000-0000-0000-0000-000000000001"}"#;
        let req: CreateGroupRequest = serde_json::from_str(json).unwrap();
        assert_eq!(req.name, "Engineering");
        assert_eq!(req.address, "eng@example.com");
        assert!(req.description.is_none());
        assert!(req.allow_external.is_none());
    }

    #[test]
    fn test_update_group_request_partial() {
        let json = r#"{"name":"New Name"}"#;
        let req: UpdateGroupRequest = serde_json::from_str(json).unwrap();
        assert_eq!(req.name, Some("New Name".to_string()));
        assert!(req.description.is_none());
        assert!(req.allow_external.is_none());
        assert!(req.active.is_none());
    }

    #[test]
    fn test_add_member_request_with_mailbox() {
        let json = r#"{"member_address":"user@example.com","mailbox_id":"00000000-0000-0000-0000-000000000002"}"#;
        let req: AddMemberRequest = serde_json::from_str(json).unwrap();
        assert_eq!(req.member_address, "user@example.com");
        assert!(req.mailbox_id.is_some());
    }

    #[test]
    fn test_add_member_request_external() {
        let json = r#"{"member_address":"external@gmail.com"}"#;
        let req: AddMemberRequest = serde_json::from_str(json).unwrap();
        assert_eq!(req.member_address, "external@gmail.com");
        assert!(req.mailbox_id.is_none());
    }
}
