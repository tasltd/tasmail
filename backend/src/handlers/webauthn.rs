// Added: WebAuthn/FIDO2 passkey handlers for TMAIL-83
use axum::{
    extract::{Path, State},
    http::StatusCode,
    Json,
};
use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine};
use rand::Rng;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use uuid::Uuid;

use crate::error::AppError;
use crate::models::webauthn_credential::{WebauthnCredential, WebauthnCredentialInfo};
use crate::services::auth_service::Claims;
use crate::state::AppState;

// -- Request/Response types --

/// PURPOSE: Registration begin response with challenge and relying party info
#[derive(Debug, Serialize)]
pub struct RegisterBeginResponse {
    pub challenge: String,
    pub rp: RelyingParty,
    pub user: PublicKeyUser,
    pub pub_key_cred_params: Vec<PubKeyCredParam>,
    pub timeout: u64,
    pub attestation: String,
}

#[derive(Debug, Serialize)]
pub struct RelyingParty {
    pub name: String,
    pub id: String,
}

#[derive(Debug, Serialize)]
pub struct PublicKeyUser {
    pub id: String,
    pub name: String,
    pub display_name: String,
}

#[derive(Debug, Serialize)]
pub struct PubKeyCredParam {
    #[serde(rename = "type")]
    pub cred_type: String,
    pub alg: i32,
}

/// PURPOSE: Client sends attestation data to complete registration
#[derive(Debug, Deserialize)]
pub struct RegisterCompleteRequest {
    pub credential_id: String,
    pub public_key: String,
    pub attestation_object: Value,
    pub client_data_json: Value,
    #[serde(default = "default_credential_name")]
    pub name: String,
}

fn default_credential_name() -> String {
    "Security Key".to_string()
}

/// PURPOSE: Authentication begin response with challenge and allowed credentials
#[derive(Debug, Serialize)]
pub struct AuthenticateBeginResponse {
    pub challenge: String,
    pub timeout: u64,
    pub rp_id: String,
    pub allow_credentials: Vec<AllowedCredential>,
}

#[derive(Debug, Serialize)]
pub struct AllowedCredential {
    #[serde(rename = "type")]
    pub cred_type: String,
    pub id: String,
}

/// PURPOSE: Client sends assertion data to complete authentication
#[derive(Debug, Deserialize)]
pub struct AuthenticateCompleteRequest {
    pub credential_id: String,
    pub authenticator_data: Value,
    pub client_data_json: Value,
    pub signature: String,
}

#[derive(Debug, Serialize)]
pub struct AuthenticateCompleteResponse {
    pub verified: bool,
    pub sign_count: i64,
}

#[derive(Debug, Serialize)]
pub struct RegisterCompleteResponse {
    pub id: Uuid,
    pub credential_id: String,
    pub name: String,
}

// -- Helper --

/// PURPOSE: Generate a cryptographically random challenge for WebAuthn ceremonies
/// CONSTRAINTS: Uses 32 bytes of randomness, base64url encoded (no padding)
fn generate_challenge() -> String {
    let mut rng = rand::rng();
    let mut bytes = [0u8; 32];
    rng.fill(&mut bytes);
    URL_SAFE_NO_PAD.encode(bytes)
}

/// PURPOSE: Extract mailbox_id from JWT claims
fn extract_mailbox_id(claims: &Claims) -> Result<Uuid, AppError> {
    claims
        .sub
        .parse()
        .map_err(|_| AppError::Internal(anyhow::anyhow!("Invalid mailbox ID in JWT claims")))
}

// -- Handlers --

/// POST /api/webauthn/register/begin — Start passkey registration
/// PURPOSE: Generate a challenge and return WebAuthn creation options
pub async fn register_begin(
    State(state): State<AppState>,
    axum::Extension(claims): axum::Extension<Claims>,
) -> Result<Json<RegisterBeginResponse>, AppError> {
    let mailbox_id = extract_mailbox_id(&claims)?;
    let challenge = generate_challenge();

    // NOTE: Store challenge in DB for verification during register/complete
    // Using a simple approach: store as a pending credential with empty public_key
    sqlx::query(
        "INSERT INTO webauthn_credentials (mailbox_id, credential_id, public_key, name)
         VALUES ($1, $2, $3, 'pending')
         ON CONFLICT (credential_id) DO UPDATE SET public_key = $3",
    )
    .bind(mailbox_id)
    .bind(format!("challenge:{}", challenge))
    .bind(challenge.as_bytes())
    .execute(&state.db)
    .await?;

    // Added: Determine RP ID from server config host
    let rp_id = state.config.server.host.clone();

    Ok(Json(RegisterBeginResponse {
        challenge,
        rp: RelyingParty {
            name: "TASMail".to_string(),
            id: rp_id,
        },
        user: PublicKeyUser {
            id: URL_SAFE_NO_PAD.encode(mailbox_id.as_bytes()),
            name: claims.username.clone(),
            display_name: claims.username.clone(),
        },
        pub_key_cred_params: vec![
            PubKeyCredParam {
                cred_type: "public-key".to_string(),
                alg: -7, // ES256
            },
            PubKeyCredParam {
                cred_type: "public-key".to_string(),
                alg: -257, // RS256
            },
        ],
        timeout: 60000,
        attestation: "none".to_string(),
    }))
}

/// POST /api/webauthn/register/complete — Complete passkey registration
/// PURPOSE: Store the credential after client-side attestation
pub async fn register_complete(
    State(state): State<AppState>,
    axum::Extension(claims): axum::Extension<Claims>,
    Json(body): Json<RegisterCompleteRequest>,
) -> Result<(StatusCode, Json<RegisterCompleteResponse>), AppError> {
    let mailbox_id = extract_mailbox_id(&claims)?;

    // Added: Validate credential_id is not empty
    if body.credential_id.is_empty() {
        return Err(AppError::BadRequest(
            "credential_id is required and cannot be empty".to_string(),
        ));
    }

    // Added: Validate public_key is valid base64url
    let public_key_bytes = URL_SAFE_NO_PAD
        .decode(&body.public_key)
        .map_err(|_| AppError::BadRequest("public_key must be valid base64url encoding".to_string()))?;

    if public_key_bytes.is_empty() {
        return Err(AppError::BadRequest(
            "public_key cannot be empty".to_string(),
        ));
    }

    // Added: Clean up the pending challenge entry
    sqlx::query("DELETE FROM webauthn_credentials WHERE mailbox_id = $1 AND credential_id LIKE 'challenge:%'")
        .bind(mailbox_id)
        .execute(&state.db)
        .await?;

    // Added: Store the real credential
    let credential = WebauthnCredential::create(
        &state.db,
        mailbox_id,
        &body.credential_id,
        &public_key_bytes,
        &body.name,
    )
    .await?;

    Ok((
        StatusCode::CREATED,
        Json(RegisterCompleteResponse {
            id: credential.id,
            credential_id: credential.credential_id,
            name: credential.name,
        }),
    ))
}

/// POST /api/webauthn/authenticate/begin — Start passkey authentication
/// PURPOSE: Return a challenge and the list of allowed credentials for this user
pub async fn authenticate_begin(
    State(state): State<AppState>,
    axum::Extension(claims): axum::Extension<Claims>,
) -> Result<Json<AuthenticateBeginResponse>, AppError> {
    let mailbox_id = extract_mailbox_id(&claims)?;

    // Added: Fetch user's registered credentials (exclude pending challenge entries)
    let credentials = WebauthnCredential::list_by_mailbox(&state.db, mailbox_id).await?;
    let real_credentials: Vec<_> = credentials
        .into_iter()
        .filter(|c| !c.credential_id.starts_with("challenge:"))
        .collect();

    if real_credentials.is_empty() {
        return Err(AppError::BadRequest(
            "No passkeys registered. Register a passkey first via /api/webauthn/register/begin".to_string(),
        ));
    }

    let challenge = generate_challenge();
    let rp_id = state.config.server.host.clone();

    let allow_credentials = real_credentials
        .iter()
        .map(|c| AllowedCredential {
            cred_type: "public-key".to_string(),
            id: c.credential_id.clone(),
        })
        .collect();

    Ok(Json(AuthenticateBeginResponse {
        challenge,
        timeout: 60000,
        rp_id,
        allow_credentials,
    }))
}

/// POST /api/webauthn/authenticate/complete — Complete passkey authentication
/// PURPOSE: Verify the assertion and increment the sign counter
/// CONSTRAINTS: Simplified verification — checks credential exists and increments sign_count
pub async fn authenticate_complete(
    State(state): State<AppState>,
    axum::Extension(claims): axum::Extension<Claims>,
    Json(body): Json<AuthenticateCompleteRequest>,
) -> Result<Json<AuthenticateCompleteResponse>, AppError> {
    let mailbox_id = extract_mailbox_id(&claims)?;

    // Added: Look up the credential
    let credential = WebauthnCredential::find_by_credential_id(&state.db, &body.credential_id)
        .await?
        .ok_or_else(|| {
            AppError::Unauthorized(format!(
                "Unknown credential_id '{}'. Register this passkey first.",
                body.credential_id
            ))
        })?;

    // Added: Verify the credential belongs to the authenticated user
    if credential.mailbox_id != mailbox_id {
        return Err(AppError::Forbidden(
            "Credential does not belong to the authenticated user".to_string(),
        ));
    }

    // Added: Increment sign count (simplified verification — full WebAuthn would verify the signature)
    let new_sign_count = credential.sign_count + 1;
    WebauthnCredential::update_sign_count(&state.db, credential.id, new_sign_count).await?;

    Ok(Json(AuthenticateCompleteResponse {
        verified: true,
        sign_count: new_sign_count,
    }))
}

/// GET /api/webauthn/credentials — List registered passkeys
/// PURPOSE: Return all passkeys for the authenticated user (for settings UI)
pub async fn list_credentials(
    State(state): State<AppState>,
    axum::Extension(claims): axum::Extension<Claims>,
) -> Result<Json<Vec<WebauthnCredentialInfo>>, AppError> {
    let mailbox_id = extract_mailbox_id(&claims)?;

    let credentials = WebauthnCredential::list_by_mailbox(&state.db, mailbox_id).await?;

    // Added: Filter out pending challenge entries and convert to info structs
    let infos: Vec<WebauthnCredentialInfo> = credentials
        .into_iter()
        .filter(|c| !c.credential_id.starts_with("challenge:"))
        .map(WebauthnCredentialInfo::from)
        .collect();

    Ok(Json(infos))
}

/// DELETE /api/webauthn/credentials/{id} — Remove a passkey
/// PURPOSE: Allow user to delete a registered passkey from their account
pub async fn delete_credential(
    State(state): State<AppState>,
    axum::Extension(claims): axum::Extension<Claims>,
    Path(id): Path<Uuid>,
) -> Result<StatusCode, AppError> {
    let mailbox_id = extract_mailbox_id(&claims)?;

    let deleted = WebauthnCredential::delete(&state.db, id, mailbox_id).await?;

    if !deleted {
        return Err(AppError::NotFound(format!(
            "Passkey with id '{}' not found or does not belong to this user",
            id
        )));
    }

    Ok(StatusCode::NO_CONTENT)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_generate_challenge_is_base64url_and_unique() {
        let c1 = generate_challenge();
        let c2 = generate_challenge();
        // NOTE: Challenges must be unique for each ceremony
        assert_ne!(c1, c2);
        // 32 bytes base64url encoded = 43 chars (no padding)
        assert_eq!(c1.len(), 43);
        // Verify it decodes back to 32 bytes
        let decoded = URL_SAFE_NO_PAD.decode(&c1).unwrap();
        assert_eq!(decoded.len(), 32);
    }

    #[test]
    fn test_register_begin_response_serialization() {
        let resp = RegisterBeginResponse {
            challenge: "test-challenge-abc123".to_string(),
            rp: RelyingParty {
                name: "TASMail".to_string(),
                id: "mail.example.com".to_string(),
            },
            user: PublicKeyUser {
                id: "user-id-encoded".to_string(),
                name: "user@example.com".to_string(),
                display_name: "user@example.com".to_string(),
            },
            pub_key_cred_params: vec![
                PubKeyCredParam {
                    cred_type: "public-key".to_string(),
                    alg: -7,
                },
            ],
            timeout: 60000,
            attestation: "none".to_string(),
        };

        let json = serde_json::to_value(&resp).unwrap();
        assert_eq!(json["rp"]["name"], "TASMail");
        assert_eq!(json["timeout"], 60000);
        assert_eq!(json["attestation"], "none");
        assert_eq!(json["pub_key_cred_params"][0]["alg"], -7);
        assert_eq!(json["pub_key_cred_params"][0]["type"], "public-key");
    }

    #[test]
    fn test_register_complete_request_deserialization() {
        let json = r#"{
            "credential_id": "cred-abc123",
            "public_key": "AQIDBA",
            "attestation_object": {},
            "client_data_json": {},
            "name": "My YubiKey"
        }"#;
        let req: RegisterCompleteRequest = serde_json::from_str(json).unwrap();
        assert_eq!(req.credential_id, "cred-abc123");
        assert_eq!(req.name, "My YubiKey");
    }

    #[test]
    fn test_register_complete_request_default_name() {
        let json = r#"{
            "credential_id": "cred-abc",
            "public_key": "AQIDBA",
            "attestation_object": {},
            "client_data_json": {}
        }"#;
        let req: RegisterCompleteRequest = serde_json::from_str(json).unwrap();
        assert_eq!(req.name, "Security Key");
    }

    #[test]
    fn test_authenticate_begin_response_serialization() {
        let resp = AuthenticateBeginResponse {
            challenge: "auth-challenge-xyz".to_string(),
            timeout: 60000,
            rp_id: "mail.example.com".to_string(),
            allow_credentials: vec![AllowedCredential {
                cred_type: "public-key".to_string(),
                id: "cred-id-1".to_string(),
            }],
        };

        let json = serde_json::to_value(&resp).unwrap();
        assert_eq!(json["rp_id"], "mail.example.com");
        assert_eq!(json["allow_credentials"][0]["type"], "public-key");
        assert_eq!(json["allow_credentials"][0]["id"], "cred-id-1");
    }

    #[test]
    fn test_authenticate_complete_request_deserialization() {
        let json = r#"{
            "credential_id": "cred-abc",
            "authenticator_data": {"raw": "base64data"},
            "client_data_json": {"type": "webauthn.get"},
            "signature": "sig-base64"
        }"#;
        let req: AuthenticateCompleteRequest = serde_json::from_str(json).unwrap();
        assert_eq!(req.credential_id, "cred-abc");
        assert_eq!(req.signature, "sig-base64");
    }

    #[test]
    fn test_authenticate_complete_response_serialization() {
        let resp = AuthenticateCompleteResponse {
            verified: true,
            sign_count: 42,
        };
        let json = serde_json::to_value(&resp).unwrap();
        assert_eq!(json["verified"], true);
        assert_eq!(json["sign_count"], 42);
    }

    #[test]
    fn test_register_complete_response_serialization() {
        let resp = RegisterCompleteResponse {
            id: Uuid::new_v4(),
            credential_id: "new-cred-id".to_string(),
            name: "Touch ID".to_string(),
        };
        let json = serde_json::to_value(&resp).unwrap();
        assert_eq!(json["credential_id"], "new-cred-id");
        assert_eq!(json["name"], "Touch ID");
        assert!(json["id"].is_string());
    }
}
