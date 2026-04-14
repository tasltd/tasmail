// Added: LDAP/Active Directory configuration and sync log models for TMAIL-100
use serde::{Deserialize, Serialize};
use sqlx::PgPool;
use uuid::Uuid;

/// PURPOSE: Represents an LDAP/AD directory configuration for user sync
/// CONSTRAINTS: bind_password_encrypted should be encrypted at rest — handler layer encrypts before storage
/// EXTERNAL: Maps to ldap_configurations table
#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct LdapConfiguration {
    pub id: Uuid,
    pub name: String,
    pub server_url: String,
    pub bind_dn: String,
    #[serde(skip_serializing)]
    pub bind_password_encrypted: String,
    pub search_base: String,
    pub search_filter: String,
    pub email_attribute: String,
    pub name_attribute: String,
    pub group_filter: Option<String>,
    pub sync_interval_minutes: i32,
    pub active: bool,
    pub last_sync_at: Option<chrono::DateTime<chrono::Utc>>,
    pub last_sync_status: Option<String>,
    pub users_synced: Option<i32>,
    pub created_at: chrono::DateTime<chrono::Utc>,
    pub updated_at: chrono::DateTime<chrono::Utc>,
}

/// PURPOSE: Represents a single sync run log entry
/// EXTERNAL: Maps to ldap_sync_logs table
#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct LdapSyncLog {
    pub id: Uuid,
    pub config_id: Uuid,
    pub started_at: chrono::DateTime<chrono::Utc>,
    pub completed_at: Option<chrono::DateTime<chrono::Utc>>,
    pub users_created: i32,
    pub users_updated: i32,
    pub users_disabled: i32,
    pub errors: serde_json::Value,
    pub status: String,
}

/// PURPOSE: Request payload for creating a new LDAP configuration
/// CONSTRAINTS: name, server_url, bind_dn, bind_password, search_base are required
#[derive(Debug, Clone, Deserialize)]
pub struct CreateLdapConfigRequest {
    pub name: String,
    pub server_url: String,
    pub bind_dn: String,
    pub bind_password: String,
    pub search_base: String,
    pub search_filter: Option<String>,
    pub email_attribute: Option<String>,
    pub name_attribute: Option<String>,
    pub group_filter: Option<String>,
    pub sync_interval_minutes: Option<i32>,
}

/// PURPOSE: Request payload for updating an existing LDAP configuration
/// CONSTRAINTS: All fields optional — only provided fields are updated
#[derive(Debug, Clone, Deserialize)]
pub struct UpdateLdapConfigRequest {
    pub name: Option<String>,
    pub server_url: Option<String>,
    pub bind_dn: Option<String>,
    pub bind_password: Option<String>,
    pub search_base: Option<String>,
    pub search_filter: Option<String>,
    pub email_attribute: Option<String>,
    pub name_attribute: Option<String>,
    pub group_filter: Option<String>,
    pub sync_interval_minutes: Option<i32>,
    pub active: Option<bool>,
}

impl LdapConfiguration {
    /// Fetch all LDAP configurations
    pub async fn list(pool: &PgPool) -> Result<Vec<LdapConfiguration>, sqlx::Error> {
        sqlx::query_as::<_, LdapConfiguration>(
            "SELECT * FROM ldap_configurations ORDER BY created_at DESC",
        )
        .fetch_all(pool)
        .await
    }

    /// Fetch a single LDAP configuration by ID
    pub async fn get_by_id(pool: &PgPool, id: Uuid) -> Result<LdapConfiguration, sqlx::Error> {
        sqlx::query_as::<_, LdapConfiguration>(
            "SELECT * FROM ldap_configurations WHERE id = $1",
        )
        .bind(id)
        .fetch_one(pool)
        .await
    }

    /// Create a new LDAP configuration
    pub async fn create(
        pool: &PgPool,
        request: &CreateLdapConfigRequest,
        encrypted_password: &str,
    ) -> Result<LdapConfiguration, sqlx::Error> {
        sqlx::query_as::<_, LdapConfiguration>(
            "INSERT INTO ldap_configurations (name, server_url, bind_dn, bind_password_encrypted, search_base, search_filter, email_attribute, name_attribute, group_filter, sync_interval_minutes)
             VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10)
             RETURNING *",
        )
        .bind(&request.name)
        .bind(&request.server_url)
        .bind(&request.bind_dn)
        .bind(encrypted_password)
        .bind(&request.search_base)
        .bind(request.search_filter.as_deref().unwrap_or("(objectClass=person)"))
        .bind(request.email_attribute.as_deref().unwrap_or("mail"))
        .bind(request.name_attribute.as_deref().unwrap_or("displayName"))
        .bind(&request.group_filter)
        .bind(request.sync_interval_minutes.unwrap_or(60))
        .fetch_one(pool)
        .await
    }

    /// Update an existing LDAP configuration with partial fields
    pub async fn update(
        pool: &PgPool,
        id: Uuid,
        request: &UpdateLdapConfigRequest,
        encrypted_password: Option<&str>,
    ) -> Result<LdapConfiguration, sqlx::Error> {
        sqlx::query_as::<_, LdapConfiguration>(
            "UPDATE ldap_configurations SET
                name = COALESCE($2, name),
                server_url = COALESCE($3, server_url),
                bind_dn = COALESCE($4, bind_dn),
                bind_password_encrypted = COALESCE($5, bind_password_encrypted),
                search_base = COALESCE($6, search_base),
                search_filter = COALESCE($7, search_filter),
                email_attribute = COALESCE($8, email_attribute),
                name_attribute = COALESCE($9, name_attribute),
                group_filter = COALESCE($10, group_filter),
                sync_interval_minutes = COALESCE($11, sync_interval_minutes),
                active = COALESCE($12, active),
                updated_at = now()
             WHERE id = $1
             RETURNING *",
        )
        .bind(id)
        .bind(&request.name)
        .bind(&request.server_url)
        .bind(&request.bind_dn)
        .bind(encrypted_password)
        .bind(&request.search_base)
        .bind(&request.search_filter)
        .bind(&request.email_attribute)
        .bind(&request.name_attribute)
        .bind(&request.group_filter)
        .bind(request.sync_interval_minutes)
        .bind(request.active)
        .fetch_one(pool)
        .await
    }

    /// Delete an LDAP configuration by ID (cascade deletes sync logs)
    pub async fn delete(pool: &PgPool, id: Uuid) -> Result<(), sqlx::Error> {
        sqlx::query("DELETE FROM ldap_configurations WHERE id = $1")
            .bind(id)
            .execute(pool)
            .await?;
        Ok(())
    }

    /// Update sync status after a sync run completes
    pub async fn update_sync_status(
        pool: &PgPool,
        id: Uuid,
        status: &str,
        users_synced: i32,
    ) -> Result<(), sqlx::Error> {
        sqlx::query(
            "UPDATE ldap_configurations SET last_sync_at = now(), last_sync_status = $2, users_synced = $3, updated_at = now() WHERE id = $1",
        )
        .bind(id)
        .bind(status)
        .bind(users_synced)
        .execute(pool)
        .await?;
        Ok(())
    }
}

impl LdapSyncLog {
    /// Fetch sync logs for a specific LDAP configuration, most recent first
    pub async fn list_by_config(
        pool: &PgPool,
        config_id: Uuid,
    ) -> Result<Vec<LdapSyncLog>, sqlx::Error> {
        sqlx::query_as::<_, LdapSyncLog>(
            "SELECT * FROM ldap_sync_logs WHERE config_id = $1 ORDER BY started_at DESC LIMIT 50",
        )
        .bind(config_id)
        .fetch_all(pool)
        .await
    }

    /// Create a new sync log entry (status='running')
    pub async fn create(pool: &PgPool, config_id: Uuid) -> Result<LdapSyncLog, sqlx::Error> {
        sqlx::query_as::<_, LdapSyncLog>(
            "INSERT INTO ldap_sync_logs (config_id) VALUES ($1) RETURNING *",
        )
        .bind(config_id)
        .fetch_one(pool)
        .await
    }

    /// Mark a sync log as completed with results
    pub async fn complete(
        pool: &PgPool,
        id: Uuid,
        status: &str,
        users_created: i32,
        users_updated: i32,
        users_disabled: i32,
        errors: &serde_json::Value,
    ) -> Result<LdapSyncLog, sqlx::Error> {
        sqlx::query_as::<_, LdapSyncLog>(
            "UPDATE ldap_sync_logs SET completed_at = now(), status = $2, users_created = $3, users_updated = $4, users_disabled = $5, errors = $6 WHERE id = $1 RETURNING *",
        )
        .bind(id)
        .bind(status)
        .bind(users_created)
        .bind(users_updated)
        .bind(users_disabled)
        .bind(errors)
        .fetch_one(pool)
        .await
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_create_ldap_config_request_deserializes_minimal() {
        // Added: Verify minimal required fields deserialize correctly
        let json_str = r##"{
            "name": "Corporate AD",
            "server_url": "ldaps://ad.example.com:636",
            "bind_dn": "cn=admin,dc=example,dc=com",
            "bind_password": "secret123",
            "search_base": "ou=Users,dc=example,dc=com"
        }"##;
        let request: CreateLdapConfigRequest = serde_json::from_str(json_str).unwrap();
        assert_eq!(request.name, "Corporate AD");
        assert_eq!(request.server_url, "ldaps://ad.example.com:636");
        assert_eq!(request.bind_dn, "cn=admin,dc=example,dc=com");
        assert_eq!(request.bind_password, "secret123");
        assert_eq!(request.search_base, "ou=Users,dc=example,dc=com");
        assert!(request.search_filter.is_none());
        assert!(request.email_attribute.is_none());
        assert!(request.name_attribute.is_none());
        assert!(request.group_filter.is_none());
        assert!(request.sync_interval_minutes.is_none());
    }

    #[test]
    fn test_create_ldap_config_request_deserializes_all_fields() {
        // Added: Verify all fields including optional ones deserialize correctly
        let json_str = r##"{
            "name": "Corporate AD",
            "server_url": "ldaps://ad.example.com:636",
            "bind_dn": "cn=admin,dc=example,dc=com",
            "bind_password": "secret123",
            "search_base": "ou=Users,dc=example,dc=com",
            "search_filter": "(&(objectClass=user)(mail=*))",
            "email_attribute": "userPrincipalName",
            "name_attribute": "cn",
            "group_filter": "(memberOf=CN=MailUsers,OU=Groups,DC=example,DC=com)",
            "sync_interval_minutes": 30
        }"##;
        let request: CreateLdapConfigRequest = serde_json::from_str(json_str).unwrap();
        assert_eq!(request.name, "Corporate AD");
        assert_eq!(request.search_filter.as_deref(), Some("(&(objectClass=user)(mail=*))"));
        assert_eq!(request.email_attribute.as_deref(), Some("userPrincipalName"));
        assert_eq!(request.name_attribute.as_deref(), Some("cn"));
        assert_eq!(
            request.group_filter.as_deref(),
            Some("(memberOf=CN=MailUsers,OU=Groups,DC=example,DC=com)")
        );
        assert_eq!(request.sync_interval_minutes, Some(30));
    }

    #[test]
    fn test_update_ldap_config_request_deserializes_partial() {
        // Added: Verify partial update with only some fields
        let json_str = r##"{"name": "Updated AD", "active": false}"##;
        let request: UpdateLdapConfigRequest = serde_json::from_str(json_str).unwrap();
        assert_eq!(request.name.as_deref(), Some("Updated AD"));
        assert_eq!(request.active, Some(false));
        assert!(request.server_url.is_none());
        assert!(request.bind_dn.is_none());
        assert!(request.bind_password.is_none());
        assert!(request.search_base.is_none());
        assert!(request.sync_interval_minutes.is_none());
    }

    #[test]
    fn test_update_ldap_config_request_deserializes_empty() {
        let json_str = "{}";
        let request: UpdateLdapConfigRequest = serde_json::from_str(json_str).unwrap();
        assert!(request.name.is_none());
        assert!(request.active.is_none());
        assert!(request.server_url.is_none());
    }

    #[test]
    fn test_ldap_configuration_serializes_without_password() {
        // Added: Verify bind_password_encrypted is skipped in serialization
        let config = LdapConfiguration {
            id: Uuid::new_v4(),
            name: "Test LDAP".to_string(),
            server_url: "ldaps://ldap.example.com:636".to_string(),
            bind_dn: "cn=admin,dc=example,dc=com".to_string(),
            bind_password_encrypted: "encrypted_secret".to_string(),
            search_base: "ou=Users,dc=example,dc=com".to_string(),
            search_filter: "(objectClass=person)".to_string(),
            email_attribute: "mail".to_string(),
            name_attribute: "displayName".to_string(),
            group_filter: None,
            sync_interval_minutes: 60,
            active: true,
            last_sync_at: None,
            last_sync_status: None,
            users_synced: Some(0),
            created_at: chrono::Utc::now(),
            updated_at: chrono::Utc::now(),
        };

        let json_value = serde_json::to_value(&config).unwrap();
        assert_eq!(json_value["name"], "Test LDAP");
        assert_eq!(json_value["server_url"], "ldaps://ldap.example.com:636");
        assert_eq!(json_value["search_filter"], "(objectClass=person)");
        assert_eq!(json_value["email_attribute"], "mail");
        assert_eq!(json_value["active"], true);
        // NOTE: bind_password_encrypted should NOT appear in serialized output
        assert!(json_value.get("bind_password_encrypted").is_none());
    }

    #[test]
    fn test_ldap_configuration_serializes_with_sync_data() {
        // Added: Verify sync metadata fields serialize correctly
        let now = chrono::Utc::now();
        let config = LdapConfiguration {
            id: Uuid::new_v4(),
            name: "Active LDAP".to_string(),
            server_url: "ldaps://ad.corp.com:636".to_string(),
            bind_dn: "cn=svc,dc=corp,dc=com".to_string(),
            bind_password_encrypted: "enc_pw".to_string(),
            search_base: "dc=corp,dc=com".to_string(),
            search_filter: "(&(objectClass=user)(mail=*))".to_string(),
            email_attribute: "mail".to_string(),
            name_attribute: "displayName".to_string(),
            group_filter: Some("(memberOf=CN=MailUsers,DC=corp,DC=com)".to_string()),
            sync_interval_minutes: 30,
            active: true,
            last_sync_at: Some(now),
            last_sync_status: Some("success".to_string()),
            users_synced: Some(42),
            created_at: now,
            updated_at: now,
        };

        let json_value = serde_json::to_value(&config).unwrap();
        assert_eq!(json_value["name"], "Active LDAP");
        assert_eq!(json_value["sync_interval_minutes"], 30);
        assert_eq!(json_value["last_sync_status"], "success");
        assert_eq!(json_value["users_synced"], 42);
        assert!(json_value["group_filter"].is_string());
    }

    #[test]
    fn test_ldap_sync_log_serializes_correctly() {
        // Added: Verify sync log serialization including error details
        let log = LdapSyncLog {
            id: Uuid::new_v4(),
            config_id: Uuid::new_v4(),
            started_at: chrono::Utc::now(),
            completed_at: Some(chrono::Utc::now()),
            users_created: 5,
            users_updated: 10,
            users_disabled: 2,
            errors: serde_json::json!([{"email": "bad@example.com", "error": "duplicate"}]),
            status: "completed".to_string(),
        };

        let json_value = serde_json::to_value(&log).unwrap();
        assert_eq!(json_value["users_created"], 5);
        assert_eq!(json_value["users_updated"], 10);
        assert_eq!(json_value["users_disabled"], 2);
        assert_eq!(json_value["status"], "completed");
        assert!(json_value["errors"].is_array());
        assert_eq!(json_value["errors"][0]["email"], "bad@example.com");
    }

    #[test]
    fn test_ldap_sync_log_serializes_running_state() {
        // Added: Verify in-progress sync log without completion data
        let log = LdapSyncLog {
            id: Uuid::new_v4(),
            config_id: Uuid::new_v4(),
            started_at: chrono::Utc::now(),
            completed_at: None,
            users_created: 0,
            users_updated: 0,
            users_disabled: 0,
            errors: serde_json::json!([]),
            status: "running".to_string(),
        };

        let json_value = serde_json::to_value(&log).unwrap();
        assert_eq!(json_value["status"], "running");
        assert!(json_value["completed_at"].is_null());
        assert_eq!(json_value["users_created"], 0);
    }
}
