// Added: Custom hostname model for per-tenant SNI configuration (TMAIL-112)
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::PgPool;
use uuid::Uuid;

/// PURPOSE: Represents a custom SMTP/IMAP hostname configuration for a domain
/// CONSTRAINTS: Each domain can have at most one custom hostname config (unique index on domain_id)
/// EXTERNAL: Actual SNI routing is handled by Postfix/Dovecot — this is the management layer
#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct CustomHostname {
    pub id: Uuid,
    pub domain_id: Uuid,
    pub smtp_hostname: String,
    pub imap_hostname: String,
    pub webmail_hostname: Option<String>,
    pub autodiscover_hostname: Option<String>,
    pub tls_cert_path: Option<String>,
    pub tls_key_path: Option<String>,
    pub verified: bool,
    pub verified_at: Option<DateTime<Utc>>,
    pub dns_verification_token: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

/// PURPOSE: Request payload for creating a new custom hostname config
/// CONSTRAINTS: domain_id, smtp_hostname, and imap_hostname are required
#[derive(Debug, Clone, Deserialize)]
pub struct CreateHostnameRequest {
    pub domain_id: Uuid,
    pub smtp_hostname: String,
    pub imap_hostname: String,
    pub webmail_hostname: Option<String>,
    pub autodiscover_hostname: Option<String>,
    pub tls_cert_path: Option<String>,
    pub tls_key_path: Option<String>,
}

/// PURPOSE: Request payload for updating an existing custom hostname config
/// CONSTRAINTS: All fields optional — only provided fields are updated
#[derive(Debug, Clone, Deserialize)]
pub struct UpdateHostnameRequest {
    pub smtp_hostname: Option<String>,
    pub imap_hostname: Option<String>,
    pub webmail_hostname: Option<String>,
    pub autodiscover_hostname: Option<String>,
    pub tls_cert_path: Option<String>,
    pub tls_key_path: Option<String>,
}

impl CustomHostname {
    /// List all custom hostname configurations
    pub async fn find_all(pool: &PgPool) -> Result<Vec<CustomHostname>, sqlx::Error> {
        sqlx::query_as::<_, CustomHostname>(
            "SELECT * FROM custom_hostnames ORDER BY created_at DESC",
        )
        .fetch_all(pool)
        .await
    }

    /// Find a custom hostname config by ID
    pub async fn find_by_id(
        pool: &PgPool,
        id: Uuid,
    ) -> Result<Option<CustomHostname>, sqlx::Error> {
        sqlx::query_as::<_, CustomHostname>("SELECT * FROM custom_hostnames WHERE id = $1")
            .bind(id)
            .fetch_optional(pool)
            .await
    }

    /// Create a new custom hostname configuration for a domain
    pub async fn create(
        pool: &PgPool,
        request: &CreateHostnameRequest,
    ) -> Result<CustomHostname, sqlx::Error> {
        // Added: Generate a DNS verification token on creation
        let verification_token = Uuid::new_v4().to_string();

        sqlx::query_as::<_, CustomHostname>(
            "INSERT INTO custom_hostnames (domain_id, smtp_hostname, imap_hostname, webmail_hostname, autodiscover_hostname, tls_cert_path, tls_key_path, dns_verification_token)
             VALUES ($1, $2, $3, $4, $5, $6, $7, $8)
             RETURNING *",
        )
        .bind(request.domain_id)
        .bind(&request.smtp_hostname)
        .bind(&request.imap_hostname)
        .bind(&request.webmail_hostname)
        .bind(&request.autodiscover_hostname)
        .bind(&request.tls_cert_path)
        .bind(&request.tls_key_path)
        .bind(&verification_token)
        .fetch_one(pool)
        .await
    }

    /// Update an existing custom hostname configuration using COALESCE for partial updates
    pub async fn update(
        pool: &PgPool,
        id: Uuid,
        request: &UpdateHostnameRequest,
    ) -> Result<Option<CustomHostname>, sqlx::Error> {
        sqlx::query_as::<_, CustomHostname>(
            "UPDATE custom_hostnames SET
                smtp_hostname = COALESCE($2, smtp_hostname),
                imap_hostname = COALESCE($3, imap_hostname),
                webmail_hostname = COALESCE($4, webmail_hostname),
                autodiscover_hostname = COALESCE($5, autodiscover_hostname),
                tls_cert_path = COALESCE($6, tls_cert_path),
                tls_key_path = COALESCE($7, tls_key_path),
                updated_at = now()
             WHERE id = $1
             RETURNING *",
        )
        .bind(id)
        .bind(&request.smtp_hostname)
        .bind(&request.imap_hostname)
        .bind(&request.webmail_hostname)
        .bind(&request.autodiscover_hostname)
        .bind(&request.tls_cert_path)
        .bind(&request.tls_key_path)
        .fetch_optional(pool)
        .await
    }

    /// Delete a custom hostname configuration
    pub async fn delete(pool: &PgPool, id: Uuid) -> Result<bool, sqlx::Error> {
        let result = sqlx::query("DELETE FROM custom_hostnames WHERE id = $1")
            .bind(id)
            .execute(pool)
            .await?;
        Ok(result.rows_affected() > 0)
    }

    /// Mark a hostname config as verified after DNS check passes
    pub async fn mark_verified(
        pool: &PgPool,
        id: Uuid,
    ) -> Result<Option<CustomHostname>, sqlx::Error> {
        sqlx::query_as::<_, CustomHostname>(
            "UPDATE custom_hostnames SET verified = true, verified_at = now(), updated_at = now()
             WHERE id = $1
             RETURNING *",
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
    fn test_create_hostname_request_deserializes_required_only() {
        let json_str = r##"{
            "domain_id": "550e8400-e29b-41d4-a716-446655440000",
            "smtp_hostname": "smtp.acme.com",
            "imap_hostname": "imap.acme.com"
        }"##;
        let request: CreateHostnameRequest = serde_json::from_str(json_str).unwrap();
        assert_eq!(request.smtp_hostname, "smtp.acme.com");
        assert_eq!(request.imap_hostname, "imap.acme.com");
        assert!(request.webmail_hostname.is_none());
        assert!(request.autodiscover_hostname.is_none());
        assert!(request.tls_cert_path.is_none());
        assert!(request.tls_key_path.is_none());
    }

    #[test]
    fn test_create_hostname_request_deserializes_all_fields() {
        let json_str = r##"{
            "domain_id": "550e8400-e29b-41d4-a716-446655440000",
            "smtp_hostname": "smtp.acme.com",
            "imap_hostname": "imap.acme.com",
            "webmail_hostname": "mail.acme.com",
            "autodiscover_hostname": "autodiscover.acme.com",
            "tls_cert_path": "/etc/ssl/acme.com.crt",
            "tls_key_path": "/etc/ssl/acme.com.key"
        }"##;
        let request: CreateHostnameRequest = serde_json::from_str(json_str).unwrap();
        assert_eq!(request.smtp_hostname, "smtp.acme.com");
        assert_eq!(request.imap_hostname, "imap.acme.com");
        assert_eq!(request.webmail_hostname.as_deref(), Some("mail.acme.com"));
        assert_eq!(
            request.autodiscover_hostname.as_deref(),
            Some("autodiscover.acme.com")
        );
        assert_eq!(
            request.tls_cert_path.as_deref(),
            Some("/etc/ssl/acme.com.crt")
        );
        assert_eq!(
            request.tls_key_path.as_deref(),
            Some("/etc/ssl/acme.com.key")
        );
    }

    #[test]
    fn test_create_hostname_request_missing_required_fails() {
        // NOTE: Missing smtp_hostname should fail deserialization
        let json_str = r##"{
            "domain_id": "550e8400-e29b-41d4-a716-446655440000",
            "imap_hostname": "imap.acme.com"
        }"##;
        let result = serde_json::from_str::<CreateHostnameRequest>(json_str);
        assert!(result.is_err());
    }

    #[test]
    fn test_update_hostname_request_deserializes_partial() {
        let json_str = r##"{"smtp_hostname": "new-smtp.acme.com"}"##;
        let request: UpdateHostnameRequest = serde_json::from_str(json_str).unwrap();
        assert_eq!(request.smtp_hostname.as_deref(), Some("new-smtp.acme.com"));
        assert!(request.imap_hostname.is_none());
        assert!(request.webmail_hostname.is_none());
    }

    #[test]
    fn test_update_hostname_request_deserializes_empty() {
        let json_str = "{}";
        let request: UpdateHostnameRequest = serde_json::from_str(json_str).unwrap();
        assert!(request.smtp_hostname.is_none());
        assert!(request.imap_hostname.is_none());
        assert!(request.webmail_hostname.is_none());
        assert!(request.autodiscover_hostname.is_none());
        assert!(request.tls_cert_path.is_none());
        assert!(request.tls_key_path.is_none());
    }

    #[test]
    fn test_custom_hostname_serializes_correctly() {
        let hostname = CustomHostname {
            id: Uuid::new_v4(),
            domain_id: Uuid::new_v4(),
            smtp_hostname: "smtp.acme.com".to_string(),
            imap_hostname: "imap.acme.com".to_string(),
            webmail_hostname: Some("mail.acme.com".to_string()),
            autodiscover_hostname: None,
            tls_cert_path: Some("/etc/ssl/acme.crt".to_string()),
            tls_key_path: None,
            verified: false,
            verified_at: None,
            dns_verification_token: Some("test-token".to_string()),
            created_at: Utc::now(),
            updated_at: Utc::now(),
        };

        let json_value = serde_json::to_value(&hostname).unwrap();
        assert_eq!(json_value["smtp_hostname"], "smtp.acme.com");
        assert_eq!(json_value["imap_hostname"], "imap.acme.com");
        assert_eq!(json_value["webmail_hostname"], "mail.acme.com");
        assert!(json_value["autodiscover_hostname"].is_null());
        assert_eq!(json_value["verified"], false);
        assert_eq!(json_value["dns_verification_token"], "test-token");
    }

    #[test]
    fn test_custom_hostname_verified_state() {
        let now = Utc::now();
        let hostname = CustomHostname {
            id: Uuid::new_v4(),
            domain_id: Uuid::new_v4(),
            smtp_hostname: "smtp.verified.com".to_string(),
            imap_hostname: "imap.verified.com".to_string(),
            webmail_hostname: None,
            autodiscover_hostname: None,
            tls_cert_path: None,
            tls_key_path: None,
            verified: true,
            verified_at: Some(now),
            dns_verification_token: Some("token-123".to_string()),
            created_at: now,
            updated_at: now,
        };

        assert!(hostname.verified);
        assert!(hostname.verified_at.is_some());
        let json_value = serde_json::to_value(&hostname).unwrap();
        assert_eq!(json_value["verified"], true);
    }

    #[test]
    fn test_custom_hostname_roundtrip() {
        let hostname = CustomHostname {
            id: Uuid::new_v4(),
            domain_id: Uuid::new_v4(),
            smtp_hostname: "smtp.roundtrip.com".to_string(),
            imap_hostname: "imap.roundtrip.com".to_string(),
            webmail_hostname: Some("mail.roundtrip.com".to_string()),
            autodiscover_hostname: Some("autodiscover.roundtrip.com".to_string()),
            tls_cert_path: None,
            tls_key_path: None,
            verified: false,
            verified_at: None,
            dns_verification_token: None,
            created_at: Utc::now(),
            updated_at: Utc::now(),
        };

        let json = serde_json::to_string(&hostname).unwrap();
        let deserialized: CustomHostname = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.id, hostname.id);
        assert_eq!(deserialized.smtp_hostname, "smtp.roundtrip.com");
        assert_eq!(deserialized.imap_hostname, "imap.roundtrip.com");
        assert_eq!(
            deserialized.webmail_hostname.as_deref(),
            Some("mail.roundtrip.com")
        );
    }
}
