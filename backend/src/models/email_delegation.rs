use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::PgPool;
use uuid::Uuid;

// Added: Email delegation model for send-as and send-on-behalf delegation
#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct EmailDelegation {
    pub id: Uuid,
    pub grantor_id: Uuid,
    pub delegate_id: Uuid,
    pub delegation_type: String,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Deserialize)]
pub struct CreateDelegation {
    pub grantor_id: Uuid,
    pub delegate_id: Uuid,
    pub delegation_type: String,
}

impl EmailDelegation {
    /// PURPOSE: Grant a new email delegation from grantor to delegate
    /// CONSTRAINTS: delegation_type should be 'send_as' or 'send_on_behalf'
    pub async fn grant(pool: &PgPool, data: &CreateDelegation) -> Result<EmailDelegation, sqlx::Error> {
        sqlx::query_as::<_, EmailDelegation>(
            "INSERT INTO email_delegations (grantor_id, delegate_id, delegation_type)
             VALUES ($1, $2, $3)
             RETURNING *"
        )
        .bind(data.grantor_id)
        .bind(data.delegate_id)
        .bind(&data.delegation_type)
        .fetch_one(pool)
        .await
    }

    /// PURPOSE: Revoke a delegation by id, only if the grantor owns it
    pub async fn revoke(pool: &PgPool, id: Uuid, grantor_id: Uuid) -> Result<bool, sqlx::Error> {
        let result = sqlx::query("DELETE FROM email_delegations WHERE id = $1 AND grantor_id = $2")
            .bind(id)
            .bind(grantor_id)
            .execute(pool)
            .await?;
        Ok(result.rows_affected() > 0)
    }

    /// PURPOSE: List delegations granted TO this user (where user is the delegate)
    pub async fn list_for_delegate(pool: &PgPool, delegate_id: Uuid) -> Result<Vec<EmailDelegation>, sqlx::Error> {
        sqlx::query_as::<_, EmailDelegation>(
            "SELECT * FROM email_delegations WHERE delegate_id = $1 ORDER BY created_at DESC"
        )
        .bind(delegate_id)
        .fetch_all(pool)
        .await
    }

    /// PURPOSE: List delegations this user has granted to others
    pub async fn list_for_grantor(pool: &PgPool, grantor_id: Uuid) -> Result<Vec<EmailDelegation>, sqlx::Error> {
        sqlx::query_as::<_, EmailDelegation>(
            "SELECT * FROM email_delegations WHERE grantor_id = $1 ORDER BY created_at DESC"
        )
        .bind(grantor_id)
        .fetch_all(pool)
        .await
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_create_delegation_deserialization() {
        let json = r#"{
            "grantor_id": "550e8400-e29b-41d4-a716-446655440000",
            "delegate_id": "660e8400-e29b-41d4-a716-446655440001",
            "delegation_type": "send_as"
        }"#;
        let req: CreateDelegation = serde_json::from_str(json).unwrap();
        assert_eq!(req.delegation_type, "send_as");
        assert_eq!(req.grantor_id.to_string(), "550e8400-e29b-41d4-a716-446655440000");
        assert_eq!(req.delegate_id.to_string(), "660e8400-e29b-41d4-a716-446655440001");
    }

    #[test]
    fn test_create_delegation_send_on_behalf() {
        let json = r#"{
            "grantor_id": "550e8400-e29b-41d4-a716-446655440000",
            "delegate_id": "660e8400-e29b-41d4-a716-446655440001",
            "delegation_type": "send_on_behalf"
        }"#;
        let req: CreateDelegation = serde_json::from_str(json).unwrap();
        assert_eq!(req.delegation_type, "send_on_behalf");
    }

    #[test]
    fn test_email_delegation_serialization() {
        let delegation = EmailDelegation {
            id: Uuid::new_v4(),
            grantor_id: Uuid::new_v4(),
            delegate_id: Uuid::new_v4(),
            delegation_type: "send_as".to_string(),
            created_at: Utc::now(),
        };
        let json = serde_json::to_string(&delegation).unwrap();
        assert!(json.contains("\"delegation_type\":\"send_as\""));
        assert!(json.contains("\"grantor_id\""));
        assert!(json.contains("\"delegate_id\""));
    }

    #[test]
    fn test_email_delegation_roundtrip() {
        let delegation = EmailDelegation {
            id: Uuid::new_v4(),
            grantor_id: Uuid::new_v4(),
            delegate_id: Uuid::new_v4(),
            delegation_type: "send_on_behalf".to_string(),
            created_at: Utc::now(),
        };
        let json = serde_json::to_string(&delegation).unwrap();
        let parsed: serde_json::Value = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed["delegation_type"], "send_on_behalf");
    }
}
