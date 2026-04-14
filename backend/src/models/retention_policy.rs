// Added: Retention policy and legal hold models for TMAIL-109

use serde::{Deserialize, Serialize};
use sqlx::PgPool;
use uuid::Uuid;

/// PURPOSE: Represents an email retention policy defining how long emails are kept
/// CONSTRAINTS: retention_days must be > 0, folder_pattern NULL means all folders
#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct RetentionPolicy {
    pub id: Uuid,
    pub name: String,
    pub description: Option<String>,
    pub retention_days: i32,
    pub folder_pattern: Option<String>,
    pub apply_to_all: bool,
    pub created_at: chrono::DateTime<chrono::Utc>,
    pub updated_at: chrono::DateTime<chrono::Utc>,
}

/// PURPOSE: Represents a legal hold placed on a user preventing email deletion
/// NOTE: When active, user's emails bypass retention policy auto-deletion
#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct LegalHold {
    pub id: Uuid,
    pub user_id: Uuid,
    pub reason: String,
    pub placed_by: Uuid,
    pub active: bool,
    pub created_at: chrono::DateTime<chrono::Utc>,
    pub released_at: Option<chrono::DateTime<chrono::Utc>>,
}

#[derive(Debug, Deserialize)]
pub struct CreateRetentionPolicyRequest {
    pub name: String,
    pub description: Option<String>,
    pub retention_days: i32,
    pub folder_pattern: Option<String>,
    pub apply_to_all: Option<bool>,
}

#[derive(Debug, Deserialize)]
pub struct UpdateRetentionPolicyRequest {
    pub name: Option<String>,
    pub description: Option<String>,
    pub retention_days: Option<i32>,
    pub folder_pattern: Option<String>,
    pub apply_to_all: Option<bool>,
}

#[derive(Debug, Deserialize)]
pub struct CreateLegalHoldRequest {
    pub user_id: Uuid,
    pub reason: String,
}

impl RetentionPolicy {
    /// PURPOSE: List all retention policies ordered by creation date
    pub async fn find_all(pool: &PgPool) -> Result<Vec<RetentionPolicy>, sqlx::Error> {
        sqlx::query_as::<_, RetentionPolicy>(
            "SELECT * FROM retention_policies ORDER BY created_at DESC",
        )
        .fetch_all(pool)
        .await
    }

    /// PURPOSE: Get a single retention policy by ID
    pub async fn find_by_id(
        pool: &PgPool,
        id: Uuid,
    ) -> Result<Option<RetentionPolicy>, sqlx::Error> {
        sqlx::query_as::<_, RetentionPolicy>(
            "SELECT * FROM retention_policies WHERE id = $1",
        )
        .bind(id)
        .fetch_optional(pool)
        .await
    }

    /// PURPOSE: Create a new retention policy
    pub async fn create(
        pool: &PgPool,
        input: &CreateRetentionPolicyRequest,
    ) -> Result<RetentionPolicy, sqlx::Error> {
        sqlx::query_as::<_, RetentionPolicy>(
            "INSERT INTO retention_policies (name, description, retention_days, folder_pattern, apply_to_all) \
             VALUES ($1, $2, $3, $4, $5) RETURNING *",
        )
        .bind(&input.name)
        .bind(&input.description)
        .bind(input.retention_days)
        .bind(&input.folder_pattern)
        .bind(input.apply_to_all.unwrap_or(false))
        .fetch_one(pool)
        .await
    }

    /// PURPOSE: Update an existing retention policy
    pub async fn update(
        pool: &PgPool,
        id: Uuid,
        input: &UpdateRetentionPolicyRequest,
    ) -> Result<Option<RetentionPolicy>, sqlx::Error> {
        sqlx::query_as::<_, RetentionPolicy>(
            "UPDATE retention_policies SET \
                name = COALESCE($2, name), \
                description = COALESCE($3, description), \
                retention_days = COALESCE($4, retention_days), \
                folder_pattern = COALESCE($5, folder_pattern), \
                apply_to_all = COALESCE($6, apply_to_all), \
                updated_at = NOW() \
             WHERE id = $1 RETURNING *",
        )
        .bind(id)
        .bind(&input.name)
        .bind(&input.description)
        .bind(input.retention_days)
        .bind(&input.folder_pattern)
        .bind(input.apply_to_all)
        .fetch_optional(pool)
        .await
    }

    /// PURPOSE: Delete a retention policy by ID
    pub async fn delete(pool: &PgPool, id: Uuid) -> Result<bool, sqlx::Error> {
        let result = sqlx::query("DELETE FROM retention_policies WHERE id = $1")
            .bind(id)
            .execute(pool)
            .await?;
        Ok(result.rows_affected() > 0)
    }
}

impl LegalHold {
    /// PURPOSE: List all legal holds, optionally filtered by active status
    pub async fn find_all(pool: &PgPool) -> Result<Vec<LegalHold>, sqlx::Error> {
        sqlx::query_as::<_, LegalHold>(
            "SELECT * FROM legal_holds ORDER BY created_at DESC",
        )
        .fetch_all(pool)
        .await
    }

    /// PURPOSE: Find active legal holds for a specific user
    pub async fn find_active_for_user(
        pool: &PgPool,
        user_id: Uuid,
    ) -> Result<Vec<LegalHold>, sqlx::Error> {
        sqlx::query_as::<_, LegalHold>(
            "SELECT * FROM legal_holds WHERE user_id = $1 AND active = true ORDER BY created_at DESC",
        )
        .bind(user_id)
        .fetch_all(pool)
        .await
    }

    /// PURPOSE: Place a new legal hold on a user
    pub async fn create(
        pool: &PgPool,
        input: &CreateLegalHoldRequest,
        placed_by: Uuid,
    ) -> Result<LegalHold, sqlx::Error> {
        sqlx::query_as::<_, LegalHold>(
            "INSERT INTO legal_holds (user_id, reason, placed_by) \
             VALUES ($1, $2, $3) RETURNING *",
        )
        .bind(input.user_id)
        .bind(&input.reason)
        .bind(placed_by)
        .fetch_one(pool)
        .await
    }

    /// PURPOSE: Release a legal hold by setting active=false and recording release time
    pub async fn release(pool: &PgPool, id: Uuid) -> Result<Option<LegalHold>, sqlx::Error> {
        sqlx::query_as::<_, LegalHold>(
            "UPDATE legal_holds SET active = false, released_at = NOW() \
             WHERE id = $1 AND active = true RETURNING *",
        )
        .bind(id)
        .fetch_optional(pool)
        .await
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_retention_policy_serialization() {
        let policy = RetentionPolicy {
            id: Uuid::new_v4(),
            name: "Trash Cleanup".to_string(),
            description: Some("Delete trash after 30 days".to_string()),
            retention_days: 30,
            folder_pattern: Some("Trash".to_string()),
            apply_to_all: false,
            created_at: chrono::Utc::now(),
            updated_at: chrono::Utc::now(),
        };

        let json = serde_json::to_value(&policy).unwrap();
        assert_eq!(json["name"], "Trash Cleanup");
        assert_eq!(json["retention_days"], 30);
        assert_eq!(json["folder_pattern"], "Trash");
        assert_eq!(json["apply_to_all"], false);
    }

    #[test]
    fn test_retention_policy_roundtrip() {
        let policy = RetentionPolicy {
            id: Uuid::new_v4(),
            name: "Global 365-day retention".to_string(),
            description: None,
            retention_days: 365,
            folder_pattern: None,
            apply_to_all: true,
            created_at: chrono::Utc::now(),
            updated_at: chrono::Utc::now(),
        };

        let json = serde_json::to_string(&policy).unwrap();
        let deserialized: RetentionPolicy = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.id, policy.id);
        assert_eq!(deserialized.name, "Global 365-day retention");
        assert_eq!(deserialized.retention_days, 365);
        assert!(deserialized.folder_pattern.is_none());
        assert!(deserialized.apply_to_all);
    }

    #[test]
    fn test_legal_hold_serialization() {
        let hold = LegalHold {
            id: Uuid::new_v4(),
            user_id: Uuid::new_v4(),
            reason: "Ongoing litigation".to_string(),
            placed_by: Uuid::new_v4(),
            active: true,
            created_at: chrono::Utc::now(),
            released_at: None,
        };

        let json = serde_json::to_value(&hold).unwrap();
        assert_eq!(json["reason"], "Ongoing litigation");
        assert_eq!(json["active"], true);
        assert!(json["released_at"].is_null());
    }

    #[test]
    fn test_legal_hold_released() {
        let hold = LegalHold {
            id: Uuid::new_v4(),
            user_id: Uuid::new_v4(),
            reason: "Investigation".to_string(),
            placed_by: Uuid::new_v4(),
            active: false,
            created_at: chrono::Utc::now(),
            released_at: Some(chrono::Utc::now()),
        };

        let json = serde_json::to_value(&hold).unwrap();
        assert_eq!(json["active"], false);
        assert!(!json["released_at"].is_null());
    }

    #[test]
    fn test_legal_hold_roundtrip() {
        let hold = LegalHold {
            id: Uuid::new_v4(),
            user_id: Uuid::new_v4(),
            reason: "Compliance audit".to_string(),
            placed_by: Uuid::new_v4(),
            active: true,
            created_at: chrono::Utc::now(),
            released_at: None,
        };

        let json = serde_json::to_string(&hold).unwrap();
        let deserialized: LegalHold = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.id, hold.id);
        assert_eq!(deserialized.reason, "Compliance audit");
        assert!(deserialized.active);
    }

    #[test]
    fn test_create_retention_policy_request_deserialization() {
        let json = serde_json::json!({
            "name": "Spam cleanup",
            "description": "Remove spam after 7 days",
            "retention_days": 7,
            "folder_pattern": "Spam",
            "apply_to_all": false
        });

        let request: CreateRetentionPolicyRequest = serde_json::from_value(json).unwrap();
        assert_eq!(request.name, "Spam cleanup");
        assert_eq!(request.retention_days, 7);
        assert_eq!(request.folder_pattern.unwrap(), "Spam");
    }

    #[test]
    fn test_create_retention_policy_request_minimal() {
        let json = serde_json::json!({
            "name": "Default policy",
            "retention_days": 90
        });

        let request: CreateRetentionPolicyRequest = serde_json::from_value(json).unwrap();
        assert_eq!(request.name, "Default policy");
        assert_eq!(request.retention_days, 90);
        assert!(request.description.is_none());
        assert!(request.folder_pattern.is_none());
    }

    #[test]
    fn test_create_retention_policy_request_missing_required_fails() {
        let json = serde_json::json!({
            "name": "Missing days"
        });
        let result = serde_json::from_value::<CreateRetentionPolicyRequest>(json);
        assert!(result.is_err());
    }

    #[test]
    fn test_update_retention_policy_request_partial() {
        let json = serde_json::json!({
            "retention_days": 60
        });

        let update: UpdateRetentionPolicyRequest = serde_json::from_value(json).unwrap();
        assert!(update.name.is_none());
        assert_eq!(update.retention_days, Some(60));
        assert!(update.apply_to_all.is_none());
    }

    #[test]
    fn test_update_retention_policy_request_empty() {
        let json = serde_json::json!({});
        let update: UpdateRetentionPolicyRequest = serde_json::from_value(json).unwrap();
        assert!(update.name.is_none());
        assert!(update.retention_days.is_none());
    }

    #[test]
    fn test_create_legal_hold_request_deserialization() {
        let user_id = Uuid::new_v4();
        let json = serde_json::json!({
            "user_id": user_id.to_string(),
            "reason": "Court order #12345"
        });

        let request: CreateLegalHoldRequest = serde_json::from_value(json).unwrap();
        assert_eq!(request.user_id, user_id);
        assert_eq!(request.reason, "Court order #12345");
    }

    #[test]
    fn test_create_legal_hold_request_missing_reason_fails() {
        let json = serde_json::json!({
            "user_id": Uuid::new_v4().to_string()
        });
        let result = serde_json::from_value::<CreateLegalHoldRequest>(json);
        assert!(result.is_err());
    }
}
