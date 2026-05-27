use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::FromRow;
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct Mailbox {
    pub id: Uuid,
    pub domain_id: Uuid,
    pub username: String,
    pub password_hash: String,
    pub display_name: Option<String>,
    pub quota_bytes: i64,
    pub quota_warn_percent: i32,
    pub active: bool,
    pub is_admin: bool,
    // Added: TMAIL-137 — dedicated compliance officer role for eDiscovery.
    // Default false on existing rows via migration 069.
    #[serde(default)]
    pub is_compliance_officer: bool,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub totp_secret: Option<String>,
    pub totp_enabled: bool,
    pub totp_verified_at: Option<DateTime<Utc>>,
}

/// Safe representation without password hash
#[derive(Debug, Clone, Serialize)]
pub struct MailboxInfo {
    pub id: Uuid,
    pub domain_id: Uuid,
    pub username: String,
    pub display_name: Option<String>,
    pub quota_bytes: i64,
    pub quota_warn_percent: i32,
    pub active: bool,
    pub is_admin: bool,
    // Added: TMAIL-137 — exposed so admin UI can show / set the role.
    pub is_compliance_officer: bool,
    pub created_at: DateTime<Utc>,
}

impl From<Mailbox> for MailboxInfo {
    fn from(m: Mailbox) -> Self {
        MailboxInfo {
            id: m.id,
            domain_id: m.domain_id,
            username: m.username,
            display_name: m.display_name,
            quota_bytes: m.quota_bytes,
            quota_warn_percent: m.quota_warn_percent,
            active: m.active,
            is_admin: m.is_admin,
            is_compliance_officer: m.is_compliance_officer,
            created_at: m.created_at,
        }
    }
}

#[derive(Debug, Deserialize)]
pub struct CreateMailbox {
    pub username: String,
    pub password: String,
    pub domain_id: Uuid,
    pub display_name: Option<String>,
    pub quota_bytes: Option<i64>,
}

impl Mailbox {
    pub async fn find_by_username(
        pool: &sqlx::PgPool,
        username: &str,
    ) -> Result<Option<Mailbox>, sqlx::Error> {
        sqlx::query_as::<_, Mailbox>("SELECT * FROM mailboxes WHERE username = $1 AND active = true")
            .bind(username)
            .fetch_optional(pool)
            .await
    }

    pub async fn find_by_id(
        pool: &sqlx::PgPool,
        id: Uuid,
    ) -> Result<Option<Mailbox>, sqlx::Error> {
        sqlx::query_as::<_, Mailbox>("SELECT * FROM mailboxes WHERE id = $1")
            .bind(id)
            .fetch_optional(pool)
            .await
    }

    pub async fn find_by_domain(
        pool: &sqlx::PgPool,
        domain_id: Uuid,
    ) -> Result<Vec<Mailbox>, sqlx::Error> {
        sqlx::query_as::<_, Mailbox>(
            "SELECT * FROM mailboxes WHERE domain_id = $1 ORDER BY username",
        )
        .bind(domain_id)
        .fetch_all(pool)
        .await
    }

    pub async fn create(
        pool: &sqlx::PgPool,
        username: &str,
        password_hash: &str,
        domain_id: Uuid,
        display_name: Option<&str>,
        quota_bytes: i64,
    ) -> Result<Mailbox, sqlx::Error> {
        sqlx::query_as::<_, Mailbox>(
            "INSERT INTO mailboxes (id, domain_id, username, password_hash, display_name, quota_bytes, active, is_admin, created_at, updated_at)
             VALUES ($1, $2, $3, $4, $5, $6, true, false, NOW(), NOW())
             RETURNING *",
        )
        .bind(Uuid::new_v4())
        .bind(domain_id)
        .bind(username)
        .bind(password_hash)
        .bind(display_name)
        .bind(quota_bytes)
        .fetch_one(pool)
        .await
    }

    pub async fn update_password(
        pool: &sqlx::PgPool,
        id: Uuid,
        password_hash: &str,
    ) -> Result<bool, sqlx::Error> {
        let result = sqlx::query(
            "UPDATE mailboxes SET password_hash = $1, updated_at = NOW() WHERE id = $2",
        )
        .bind(password_hash)
        .bind(id)
        .execute(pool)
        .await?;
        Ok(result.rows_affected() > 0)
    }

    pub async fn delete(pool: &sqlx::PgPool, id: Uuid) -> Result<bool, sqlx::Error> {
        let result = sqlx::query("DELETE FROM mailboxes WHERE id = $1")
            .bind(id)
            .execute(pool)
            .await?;
        Ok(result.rows_affected() > 0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_mailbox() -> Mailbox {
        Mailbox {
            id: Uuid::new_v4(),
            domain_id: Uuid::new_v4(),
            username: "kwame@tasmail.gh".to_string(),
            password_hash: "$argon2id$v=19$m=65536,t=3,p=4$secrethash".to_string(),
            display_name: Some("Kwame Mensah".to_string()),
            quota_bytes: 1_073_741_824,
            quota_warn_percent: 80,
            active: true,
            is_admin: false,
            is_compliance_officer: false,
            created_at: Utc::now(),
            updated_at: Utc::now(),
            totp_secret: Some("JBSWY3DPEHPK3PXP".to_string()),
            totp_enabled: true,
            totp_verified_at: Some(Utc::now()),
        }
    }

    #[test]
    fn test_mailbox_serialization() {
        let mailbox = sample_mailbox();
        let id = mailbox.id;

        let json = serde_json::to_value(&mailbox).unwrap();
        assert_eq!(json["id"], id.to_string());
        assert_eq!(json["username"], "kwame@tasmail.gh");
        assert_eq!(json["display_name"], "Kwame Mensah");
        assert_eq!(json["quota_bytes"], 1_073_741_824);
        assert_eq!(json["quota_warn_percent"], 80);
        assert_eq!(json["active"], true);
        assert_eq!(json["is_admin"], false);
        assert_eq!(json["totp_enabled"], true);
        // password_hash IS present in Mailbox serialization
        assert!(json["password_hash"].is_string());
    }

    #[test]
    fn test_mailbox_info_from_mailbox_excludes_sensitive_fields() {
        let mailbox = sample_mailbox();
        let id = mailbox.id;
        let domain_id = mailbox.domain_id;

        let info: MailboxInfo = mailbox.into();

        let json = serde_json::to_value(&info).unwrap();
        // Verify expected fields are present
        assert_eq!(json["id"], id.to_string());
        assert_eq!(json["domain_id"], domain_id.to_string());
        assert_eq!(json["username"], "kwame@tasmail.gh");
        assert_eq!(json["display_name"], "Kwame Mensah");
        assert_eq!(json["quota_bytes"], 1_073_741_824);
        assert_eq!(json["quota_warn_percent"], 80);
        assert_eq!(json["active"], true);
        assert_eq!(json["is_admin"], false);

        // Verify sensitive fields are NOT present
        assert!(json.get("password_hash").is_none());
        assert!(json.get("totp_secret").is_none());
        assert!(json.get("totp_enabled").is_none());
        assert!(json.get("totp_verified_at").is_none());
        assert!(json.get("updated_at").is_none());
    }

    #[test]
    fn test_mailbox_info_from_mailbox_with_no_display_name() {
        let mut mailbox = sample_mailbox();
        mailbox.display_name = None;

        let info: MailboxInfo = mailbox.into();
        let json = serde_json::to_value(&info).unwrap();
        assert!(json["display_name"].is_null());
    }

    #[test]
    fn test_create_mailbox_deserialization() {
        let domain_id = Uuid::new_v4();
        let json = serde_json::json!({
            "username": "ama@tasmail.gh",
            "password": "strongP@ss123",
            "domain_id": domain_id,
            "display_name": "Ama Darko",
            "quota_bytes": 5_368_709_120_i64
        });

        let create: CreateMailbox = serde_json::from_value(json).unwrap();
        assert_eq!(create.username, "ama@tasmail.gh");
        assert_eq!(create.password, "strongP@ss123");
        assert_eq!(create.domain_id, domain_id);
        assert_eq!(create.display_name.unwrap(), "Ama Darko");
        assert_eq!(create.quota_bytes.unwrap(), 5_368_709_120);
    }

    #[test]
    fn test_create_mailbox_deserialization_minimal() {
        let domain_id = Uuid::new_v4();
        let json = serde_json::json!({
            "username": "kofi@tasmail.gh",
            "password": "secret",
            "domain_id": domain_id
        });

        let create: CreateMailbox = serde_json::from_value(json).unwrap();
        assert_eq!(create.username, "kofi@tasmail.gh");
        assert!(create.display_name.is_none());
        assert!(create.quota_bytes.is_none());
    }

    #[test]
    fn test_create_mailbox_missing_required_field_fails() {
        let json = serde_json::json!({
            "username": "test@tasmail.gh"
        });
        let result = serde_json::from_value::<CreateMailbox>(json);
        assert!(result.is_err());
    }
}
