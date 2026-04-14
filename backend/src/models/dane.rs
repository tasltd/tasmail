// Added: DANE policy and verification models for TMAIL-125 — DNS-based Authentication of Named Entities

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::PgPool;
use uuid::Uuid;

/// PURPOSE: Represents a DANE policy configuration for a specific domain
/// CONSTRAINTS: domain must be unique; tlsa_records is a JSONB array of TLSA record objects
/// EXTERNAL: PostgreSQL dane_policies table
#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct DanePolicy {
    pub id: Uuid,
    pub domain: String,
    pub enforce: bool,
    pub last_checked_at: Option<DateTime<Utc>>,
    pub tlsa_records: serde_json::Value,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

/// PURPOSE: Records the DANE verification result for each outbound message
/// CONSTRAINTS: dane_status must be one of: verified, failed, no_tlsa, disabled
/// EXTERNAL: PostgreSQL dane_verifications table with RLS on user_id
#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct DaneVerification {
    pub id: Uuid,
    pub user_id: Uuid,
    pub message_id: String,
    pub recipient_domain: String,
    pub dane_status: String,
    pub checked_at: DateTime<Utc>,
}

/// Added: Request body for creating or updating a DANE policy
#[derive(Debug, Deserialize)]
pub struct CreateDanePolicyRequest {
    pub domain: String,
    pub enforce: Option<bool>,
}

/// Added: Request body for looking up TLSA records for a domain
#[derive(Debug, Deserialize)]
pub struct DaneLookupRequest {
    pub domain: String,
    pub port: Option<u16>,
}

/// Added: A single TLSA record parsed from DNS
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct TlsaRecord {
    pub usage: u8,
    pub selector: u8,
    pub matching_type: u8,
    pub cert_data: String,
}

/// Added: Result of a DANE verification check
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DaneResult {
    pub domain: String,
    pub status: String,
    pub tlsa_records: Vec<TlsaRecord>,
    pub message: String,
}

/// Added: Pagination query params for listing verifications
#[derive(Debug, Deserialize)]
pub struct VerificationListParams {
    pub limit: Option<i64>,
    pub offset: Option<i64>,
}

impl DanePolicy {
    /// Added: List all DANE policies ordered by domain
    pub async fn list_all(pool: &PgPool) -> Result<Vec<DanePolicy>, sqlx::Error> {
        sqlx::query_as::<_, DanePolicy>(
            "SELECT * FROM dane_policies ORDER BY domain ASC",
        )
        .fetch_all(pool)
        .await
    }

    /// Added: Get a DANE policy by domain
    pub async fn get_by_domain(pool: &PgPool, domain: &str) -> Result<Option<DanePolicy>, sqlx::Error> {
        sqlx::query_as::<_, DanePolicy>(
            "SELECT * FROM dane_policies WHERE domain = $1",
        )
        .bind(domain)
        .fetch_optional(pool)
        .await
    }

    /// Added: Create or update a DANE policy (upsert on domain)
    pub async fn upsert(
        pool: &PgPool,
        req: &CreateDanePolicyRequest,
    ) -> Result<DanePolicy, sqlx::Error> {
        let enforce = req.enforce.unwrap_or(false);
        sqlx::query_as::<_, DanePolicy>(
            "INSERT INTO dane_policies (domain, enforce)
             VALUES ($1, $2)
             ON CONFLICT (domain) DO UPDATE SET
                enforce = $2,
                updated_at = now()
             RETURNING *",
        )
        .bind(&req.domain)
        .bind(enforce)
        .fetch_one(pool)
        .await
    }

    /// Added: Update TLSA records and last_checked_at for a policy
    pub async fn update_tlsa(
        pool: &PgPool,
        id: Uuid,
        tlsa_records: &serde_json::Value,
    ) -> Result<Option<DanePolicy>, sqlx::Error> {
        sqlx::query_as::<_, DanePolicy>(
            "UPDATE dane_policies SET
                tlsa_records = $2,
                last_checked_at = now(),
                updated_at = now()
             WHERE id = $1
             RETURNING *",
        )
        .bind(id)
        .bind(tlsa_records)
        .fetch_optional(pool)
        .await
    }

    /// Added: Delete a DANE policy by ID
    pub async fn delete(pool: &PgPool, id: Uuid) -> Result<bool, sqlx::Error> {
        let result = sqlx::query("DELETE FROM dane_policies WHERE id = $1")
            .bind(id)
            .execute(pool)
            .await?;
        Ok(result.rows_affected() > 0)
    }
}

impl DaneVerification {
    /// Added: Record a DANE verification result for an outbound message
    pub async fn create(
        pool: &PgPool,
        user_id: Uuid,
        message_id: &str,
        recipient_domain: &str,
        dane_status: &str,
    ) -> Result<DaneVerification, sqlx::Error> {
        sqlx::query_as::<_, DaneVerification>(
            "INSERT INTO dane_verifications (user_id, message_id, recipient_domain, dane_status)
             VALUES ($1, $2, $3, $4)
             RETURNING *",
        )
        .bind(user_id)
        .bind(message_id)
        .bind(recipient_domain)
        .bind(dane_status)
        .fetch_one(pool)
        .await
    }

    /// Added: List verifications for a user with pagination, newest first
    pub async fn list_for_user(
        pool: &PgPool,
        user_id: Uuid,
        limit: i64,
        offset: i64,
    ) -> Result<Vec<DaneVerification>, sqlx::Error> {
        sqlx::query_as::<_, DaneVerification>(
            "SELECT * FROM dane_verifications WHERE user_id = $1 ORDER BY checked_at DESC LIMIT $2 OFFSET $3",
        )
        .bind(user_id)
        .bind(limit)
        .bind(offset)
        .fetch_all(pool)
        .await
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_dane_policy_deserialization() {
        // Added: Verify DanePolicy struct deserializes from JSON
        let json = r#"{
            "id": "550e8400-e29b-41d4-a716-446655440000",
            "domain": "example.com",
            "enforce": true,
            "last_checked_at": "2026-04-14T10:00:00Z",
            "tlsa_records": [{"usage": 3, "selector": 1, "matching_type": 1, "cert_data": "abcdef"}],
            "created_at": "2026-04-14T10:00:00Z",
            "updated_at": "2026-04-14T10:00:00Z"
        }"#;
        let policy: DanePolicy = serde_json::from_str(json).unwrap();
        assert_eq!(policy.domain, "example.com");
        assert!(policy.enforce);
        assert!(policy.last_checked_at.is_some());
    }

    #[test]
    fn test_dane_policy_serialization() {
        // Added: Verify DanePolicy serialization produces expected JSON shape
        let policy = DanePolicy {
            id: Uuid::new_v4(),
            domain: "mail.example.org".to_string(),
            enforce: false,
            last_checked_at: None,
            tlsa_records: serde_json::json!([]),
            created_at: Utc::now(),
            updated_at: Utc::now(),
        };
        let json = serde_json::to_string(&policy).unwrap();
        let parsed: serde_json::Value = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed["domain"], "mail.example.org");
        assert_eq!(parsed["enforce"], false);
    }

    #[test]
    fn test_dane_verification_deserialization() {
        // Added: Verify DaneVerification struct round-trips through JSON
        let json = r#"{
            "id": "550e8400-e29b-41d4-a716-446655440000",
            "user_id": "550e8400-e29b-41d4-a716-446655440001",
            "message_id": "<msg123@example.com>",
            "recipient_domain": "recipient.com",
            "dane_status": "verified",
            "checked_at": "2026-04-14T10:00:00Z"
        }"#;
        let verification: DaneVerification = serde_json::from_str(json).unwrap();
        assert_eq!(verification.dane_status, "verified");
        assert_eq!(verification.recipient_domain, "recipient.com");
    }

    #[test]
    fn test_dane_verification_serialization() {
        // Added: Verify DaneVerification serialization
        let verification = DaneVerification {
            id: Uuid::new_v4(),
            user_id: Uuid::new_v4(),
            message_id: "<test@example.com>".to_string(),
            recipient_domain: "secure.org".to_string(),
            dane_status: "failed".to_string(),
            checked_at: Utc::now(),
        };
        let json = serde_json::to_string(&verification).unwrap();
        let parsed: serde_json::Value = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed["dane_status"], "failed");
        assert_eq!(parsed["recipient_domain"], "secure.org");
    }

    #[test]
    fn test_create_dane_policy_request_minimal() {
        // Added: CreateDanePolicyRequest with only required field
        let json = r#"{"domain": "example.com"}"#;
        let req: CreateDanePolicyRequest = serde_json::from_str(json).unwrap();
        assert_eq!(req.domain, "example.com");
        assert!(req.enforce.is_none());
    }

    #[test]
    fn test_create_dane_policy_request_full() {
        // Added: CreateDanePolicyRequest with all fields
        let json = r#"{"domain": "secure.example.com", "enforce": true}"#;
        let req: CreateDanePolicyRequest = serde_json::from_str(json).unwrap();
        assert_eq!(req.domain, "secure.example.com");
        assert_eq!(req.enforce, Some(true));
    }

    #[test]
    fn test_tlsa_record_deserialization() {
        // Added: Verify TlsaRecord parses from JSON
        let json = r#"{"usage": 3, "selector": 1, "matching_type": 1, "cert_data": "a0b1c2d3"}"#;
        let record: TlsaRecord = serde_json::from_str(json).unwrap();
        assert_eq!(record.usage, 3);
        assert_eq!(record.selector, 1);
        assert_eq!(record.matching_type, 1);
        assert_eq!(record.cert_data, "a0b1c2d3");
    }

    #[test]
    fn test_tlsa_record_serialization() {
        // Added: Verify TlsaRecord round-trips through JSON
        let record = TlsaRecord {
            usage: 2,
            selector: 0,
            matching_type: 2,
            cert_data: "deadbeef".to_string(),
        };
        let json = serde_json::to_string(&record).unwrap();
        let parsed: TlsaRecord = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed, record);
    }

    #[test]
    fn test_dane_result_serialization() {
        // Added: Verify DaneResult output format
        let result = DaneResult {
            domain: "example.com".to_string(),
            status: "verified".to_string(),
            tlsa_records: vec![TlsaRecord {
                usage: 3,
                selector: 1,
                matching_type: 1,
                cert_data: "abcdef".to_string(),
            }],
            message: "DANE verification successful".to_string(),
        };
        let json = serde_json::to_string(&result).unwrap();
        let parsed: serde_json::Value = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed["domain"], "example.com");
        assert_eq!(parsed["status"], "verified");
        assert_eq!(parsed["tlsa_records"][0]["usage"], 3);
    }

    #[test]
    fn test_dane_lookup_request_minimal() {
        // Added: DaneLookupRequest with only domain
        let json = r#"{"domain": "example.com"}"#;
        let req: DaneLookupRequest = serde_json::from_str(json).unwrap();
        assert_eq!(req.domain, "example.com");
        assert!(req.port.is_none());
    }

    #[test]
    fn test_dane_lookup_request_with_port() {
        // Added: DaneLookupRequest with custom port
        let json = r#"{"domain": "example.com", "port": 465}"#;
        let req: DaneLookupRequest = serde_json::from_str(json).unwrap();
        assert_eq!(req.port, Some(465));
    }

    #[test]
    fn test_verification_list_params_defaults() {
        // Added: VerificationListParams with no values
        let json = r#"{}"#;
        let params: VerificationListParams = serde_json::from_str(json).unwrap();
        assert!(params.limit.is_none());
        assert!(params.offset.is_none());
    }

    #[test]
    fn test_verification_list_params_with_values() {
        // Added: VerificationListParams with explicit pagination
        let json = r#"{"limit": 25, "offset": 50}"#;
        let params: VerificationListParams = serde_json::from_str(json).unwrap();
        assert_eq!(params.limit, Some(25));
        assert_eq!(params.offset, Some(50));
    }

    #[test]
    fn test_dane_policy_null_last_checked() {
        // Added: DanePolicy with null last_checked_at
        let json = r#"{
            "id": "550e8400-e29b-41d4-a716-446655440000",
            "domain": "new-domain.com",
            "enforce": false,
            "last_checked_at": null,
            "tlsa_records": [],
            "created_at": "2026-04-14T10:00:00Z",
            "updated_at": "2026-04-14T10:00:00Z"
        }"#;
        let policy: DanePolicy = serde_json::from_str(json).unwrap();
        assert!(policy.last_checked_at.is_none());
        assert_eq!(policy.tlsa_records, serde_json::json!([]));
    }

    #[test]
    fn test_dane_status_values() {
        // Added: Verify all valid DANE status values deserialize correctly
        for status in ["verified", "failed", "no_tlsa", "disabled"] {
            let json = format!(
                r#"{{"id":"550e8400-e29b-41d4-a716-446655440000","user_id":"550e8400-e29b-41d4-a716-446655440001","message_id":"<msg@ex>","recipient_domain":"d.com","dane_status":"{}","checked_at":"2026-04-14T10:00:00Z"}}"#,
                status
            );
            let v: DaneVerification = serde_json::from_str(&json).unwrap();
            assert_eq!(v.dane_status, status);
        }
    }
}
