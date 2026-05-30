// Added: DANE policy and verification handlers for TMAIL-125

use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    Json,
};
use uuid::Uuid;

use crate::error::AppError;
use crate::models::dane::{
    CreateDanePolicyRequest, DaneLookupRequest, DanePolicy, DaneResult, DaneVerification,
    TlsaRecord, VerificationListParams,
};
use crate::services::audit::audit_admin_action;
use crate::services::auth_service::{self, Claims};
use crate::services::dane_service;
use crate::state::AppState;

/// GET /api/admin/dane — List all DANE policies
pub async fn list_policies(
    State(state): State<AppState>,
    axum::Extension(claims): axum::Extension<Claims>,
) -> Result<Json<Vec<DanePolicy>>, AppError> {
    auth_service::require_admin(&claims)?; // TMAIL-210
    let policies = DanePolicy::list_all(&state.db).await?;
    Ok(Json(policies))
}

/// POST /api/admin/dane — Create or update a DANE policy for a domain
pub async fn create_policy(
    State(state): State<AppState>,
    axum::Extension(claims): axum::Extension<Claims>,
    Json(body): Json<CreateDanePolicyRequest>,
) -> Result<(StatusCode, Json<DanePolicy>), AppError> {
    auth_service::require_admin(&claims)?; // TMAIL-210
    // Added: Validate domain is not empty
    if body.domain.trim().is_empty() {
        return Err(AppError::BadRequest("Domain must not be empty".to_string()));
    }

    let policy = DanePolicy::upsert(&state.db, &body).await?;

    // Added (TMAIL-307): audit-log DANE policy upsert.
    audit_admin_action(
        &state.db,
        &claims,
        "dane_policy.upsert",
        Some("dane_policy"),
        Some(&policy.id.to_string()),
        Some(serde_json::json!({
            "domain": body.domain,
            "enforce": body.enforce,
        })),
    )
    .await;

    Ok((StatusCode::CREATED, Json(policy)))
}

/// DELETE /api/admin/dane/{id} — Delete a DANE policy
pub async fn delete_policy(
    State(state): State<AppState>,
    axum::Extension(claims): axum::Extension<Claims>,
    Path(id): Path<Uuid>,
) -> Result<StatusCode, AppError> {
    auth_service::require_admin(&claims)?; // TMAIL-210
    let deleted = DanePolicy::delete(&state.db, id).await?;
    if deleted {
        // Added (TMAIL-307): audit-log DANE policy delete.
        audit_admin_action(
            &state.db,
            &claims,
            "dane_policy.delete",
            Some("dane_policy"),
            Some(&id.to_string()),
            None,
        )
        .await;
        Ok(StatusCode::NO_CONTENT)
    } else {
        Err(AppError::NotFound(format!("DANE policy '{}' not found", id)))
    }
}

/// POST /api/admin/dane/lookup — Lookup TLSA records for a domain (dry-run parse)
pub async fn lookup_tlsa(
    State(_state): State<AppState>,
    axum::Extension(claims): axum::Extension<Claims>,
    Json(body): Json<DaneLookupRequest>,
) -> Result<Json<DaneResult>, AppError> {
    auth_service::require_admin(&claims)?; // TMAIL-210
    // Added: Validate domain
    if body.domain.trim().is_empty() {
        return Err(AppError::BadRequest("Domain must not be empty".to_string()));
    }

    let port = body.port.unwrap_or(25);
    let query_name = dane_service::tlsa_query_name(&body.domain, port);

    // NOTE: In production, this would perform actual DNS TLSA query via trust-dns/hickory.
    // For now, return an informational result indicating what would be queried.
    let result = DaneResult {
        domain: body.domain.clone(),
        status: "no_tlsa".to_string(),
        tlsa_records: vec![],
        message: format!(
            "DNS TLSA lookup would query: {}. No live DNS resolver configured yet.",
            query_name
        ),
    };

    Ok(Json(result))
}

/// GET /api/dane/verifications — List DANE verifications for current user
pub async fn list_verifications(
    State(state): State<AppState>,
    axum::Extension(claims): axum::Extension<Claims>,
    Query(params): Query<VerificationListParams>,
) -> Result<Json<Vec<DaneVerification>>, AppError> {
    let user_id: Uuid = claims.sub.parse().map_err(|_| {
        AppError::BadRequest("Invalid user ID in token".to_string())
    })?;
    let limit = params.limit.unwrap_or(50).min(200);
    let offset = params.offset.unwrap_or(0);
    let verifications = DaneVerification::list_for_user(&state.db, user_id, limit, offset).await?;
    Ok(Json(verifications))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_create_policy_request_deserialization() {
        // Added: Verify handler can deserialize a typical create request
        let json = r#"{"domain": "example.com", "enforce": true}"#;
        let req: CreateDanePolicyRequest = serde_json::from_str(json).unwrap();
        assert_eq!(req.domain, "example.com");
        assert_eq!(req.enforce, Some(true));
    }

    #[test]
    fn test_lookup_request_minimal() {
        // Added: Lookup request with only domain
        let json = r#"{"domain": "mail.example.com"}"#;
        let req: DaneLookupRequest = serde_json::from_str(json).unwrap();
        assert_eq!(req.domain, "mail.example.com");
        assert!(req.port.is_none());
    }

    #[test]
    fn test_lookup_request_with_port() {
        // Added: Lookup request with explicit port
        let json = r#"{"domain": "mail.example.com", "port": 587}"#;
        let req: DaneLookupRequest = serde_json::from_str(json).unwrap();
        assert_eq!(req.port, Some(587));
    }

    #[test]
    fn test_verification_list_params_defaults() {
        // Added: VerificationListParams with no values should parse to None
        let json = r#"{}"#;
        let params: VerificationListParams = serde_json::from_str(json).unwrap();
        assert!(params.limit.is_none());
        assert!(params.offset.is_none());
    }

    #[test]
    fn test_tlsa_record_round_trip() {
        // Added: TlsaRecord should round-trip through JSON
        let record = TlsaRecord {
            usage: 3,
            selector: 1,
            matching_type: 1,
            cert_data: "abcdef0123456789".to_string(),
        };
        let json = serde_json::to_string(&record).unwrap();
        let parsed: TlsaRecord = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed, record);
    }

    #[test]
    fn test_dane_result_format() {
        // Added: Verify DaneResult JSON output shape
        let result = DaneResult {
            domain: "example.com".to_string(),
            status: "verified".to_string(),
            tlsa_records: vec![],
            message: "Test".to_string(),
        };
        let json = serde_json::to_string(&result).unwrap();
        let parsed: serde_json::Value = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed["domain"], "example.com");
        assert_eq!(parsed["status"], "verified");
    }
}
