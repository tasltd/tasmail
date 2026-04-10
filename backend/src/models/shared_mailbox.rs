use serde::{Deserialize, Serialize};
use sqlx::{FromRow, PgPool};
use uuid::Uuid;

/// ACL entry granting a user access to a shared mailbox
#[derive(Debug, Clone, Serialize, FromRow)]
pub struct SharedMailboxAcl {
    pub id: Uuid,
    pub mailbox_id: Uuid,
    pub granted_to: Uuid,
    pub can_read: bool,
    pub can_write: bool,
    pub can_delete: bool,
    pub can_admin: bool,
    pub granted_at: chrono::DateTime<chrono::Utc>,
    pub granted_by: Option<Uuid>,
}

// Added: ACL entry with the username of the grantee for display
#[derive(Debug, Clone, Serialize, FromRow)]
pub struct SharedMailboxAclWithUser {
    pub id: Uuid,
    pub mailbox_id: Uuid,
    pub granted_to: Uuid,
    pub granted_to_username: String,
    pub can_read: bool,
    pub can_write: bool,
    pub can_delete: bool,
    pub can_admin: bool,
    pub granted_at: chrono::DateTime<chrono::Utc>,
}

// Added: Shared mailbox visible to a user with permission info
#[derive(Debug, Clone, Serialize, FromRow)]
pub struct SharedMailboxView {
    pub mailbox_id: Uuid,
    pub username: String,
    pub display_name: Option<String>,
    pub can_read: bool,
    pub can_write: bool,
    pub can_delete: bool,
    pub can_admin: bool,
}

#[derive(Debug, Deserialize)]
pub struct GrantAccessRequest {
    pub granted_to: Uuid,
    pub can_read: Option<bool>,
    pub can_write: Option<bool>,
    pub can_delete: Option<bool>,
    pub can_admin: Option<bool>,
}

#[derive(Debug, Deserialize)]
pub struct UpdateAclRequest {
    pub can_read: Option<bool>,
    pub can_write: Option<bool>,
    pub can_delete: Option<bool>,
    pub can_admin: Option<bool>,
}

impl SharedMailboxAcl {
    /// Grant access to a shared mailbox
    pub async fn grant(
        pool: &PgPool,
        mailbox_id: Uuid,
        req: &GrantAccessRequest,
        granted_by: Uuid,
    ) -> Result<SharedMailboxAcl, sqlx::Error> {
        // Ensure mailbox is marked as shared
        sqlx::query("UPDATE mailboxes SET is_shared = true WHERE id = $1")
            .bind(mailbox_id)
            .execute(pool)
            .await?;

        sqlx::query_as::<_, SharedMailboxAcl>(
            "INSERT INTO shared_mailbox_acl (mailbox_id, granted_to, can_read, can_write, can_delete, can_admin, granted_by)
             VALUES ($1, $2, $3, $4, $5, $6, $7)
             ON CONFLICT (mailbox_id, granted_to) DO UPDATE SET
                can_read = EXCLUDED.can_read,
                can_write = EXCLUDED.can_write,
                can_delete = EXCLUDED.can_delete,
                can_admin = EXCLUDED.can_admin,
                granted_by = EXCLUDED.granted_by
             RETURNING *"
        )
        .bind(mailbox_id)
        .bind(req.granted_to)
        .bind(req.can_read.unwrap_or(true))
        .bind(req.can_write.unwrap_or(false))
        .bind(req.can_delete.unwrap_or(false))
        .bind(req.can_admin.unwrap_or(false))
        .bind(granted_by)
        .fetch_one(pool)
        .await
    }

    /// List ACL entries for a mailbox (who has access)
    pub async fn list_for_mailbox(
        pool: &PgPool,
        mailbox_id: Uuid,
    ) -> Result<Vec<SharedMailboxAclWithUser>, sqlx::Error> {
        sqlx::query_as::<_, SharedMailboxAclWithUser>(
            "SELECT a.id, a.mailbox_id, a.granted_to, m.username as granted_to_username,
                    a.can_read, a.can_write, a.can_delete, a.can_admin, a.granted_at
             FROM shared_mailbox_acl a
             JOIN mailboxes m ON m.id = a.granted_to
             WHERE a.mailbox_id = $1
             ORDER BY m.username"
        )
        .bind(mailbox_id)
        .fetch_all(pool)
        .await
    }

    /// List shared mailboxes accessible by a user
    pub async fn list_accessible(
        pool: &PgPool,
        user_id: Uuid,
    ) -> Result<Vec<SharedMailboxView>, sqlx::Error> {
        sqlx::query_as::<_, SharedMailboxView>(
            "SELECT a.mailbox_id, m.username, m.display_name,
                    a.can_read, a.can_write, a.can_delete, a.can_admin
             FROM shared_mailbox_acl a
             JOIN mailboxes m ON m.id = a.mailbox_id
             WHERE a.granted_to = $1 AND m.active = true
             ORDER BY m.username"
        )
        .bind(user_id)
        .fetch_all(pool)
        .await
    }

    /// Revoke access to a shared mailbox
    pub async fn revoke(
        pool: &PgPool,
        mailbox_id: Uuid,
        granted_to: Uuid,
    ) -> Result<(), sqlx::Error> {
        sqlx::query("DELETE FROM shared_mailbox_acl WHERE mailbox_id = $1 AND granted_to = $2")
            .bind(mailbox_id)
            .bind(granted_to)
            .execute(pool)
            .await?;

        // Check if any ACL entries remain; if not, unmark as shared
        let count: (i64,) = sqlx::query_as(
            "SELECT COUNT(*) FROM shared_mailbox_acl WHERE mailbox_id = $1"
        )
        .bind(mailbox_id)
        .fetch_one(pool)
        .await?;

        if count.0 == 0 {
            sqlx::query("UPDATE mailboxes SET is_shared = false WHERE id = $1")
                .bind(mailbox_id)
                .execute(pool)
                .await?;
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_grant_request_deserialization() {
        let json = r#"{"granted_to":"00000000-0000-0000-0000-000000000001","can_read":true,"can_write":true}"#;
        let req: GrantAccessRequest = serde_json::from_str(json).unwrap();
        assert_eq!(req.can_read, Some(true));
        assert_eq!(req.can_write, Some(true));
        assert!(req.can_delete.is_none());
        assert!(req.can_admin.is_none());
    }

    #[test]
    fn test_update_acl_partial() {
        let json = r#"{"can_write":true}"#;
        let req: UpdateAclRequest = serde_json::from_str(json).unwrap();
        assert!(req.can_read.is_none());
        assert_eq!(req.can_write, Some(true));
    }

    #[test]
    fn test_grant_defaults() {
        let json = r#"{"granted_to":"00000000-0000-0000-0000-000000000001"}"#;
        let req: GrantAccessRequest = serde_json::from_str(json).unwrap();
        // Defaults are applied in the grant() method, not in deserialization
        assert!(req.can_read.is_none());
        assert!(req.can_write.is_none());
    }
}
