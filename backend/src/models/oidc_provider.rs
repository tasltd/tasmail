// Added: OIDC provider and user link models for TMAIL-99
use serde::{Deserialize, Serialize};
use sqlx::PgPool;
use uuid::Uuid;

/// PURPOSE: Represents an OIDC identity provider configuration (Google, Microsoft, etc.)
/// CONSTRAINTS: client_secret_encrypted should be encrypted at rest — handler layer encrypts before storage
/// EXTERNAL: Maps to oidc_providers table
#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct OidcProvider {
    pub id: Uuid,
    pub name: String,
    pub issuer_url: String,
    pub client_id: String,
    #[serde(skip_serializing)]
    pub client_secret_encrypted: String,
    pub scopes: String,
    pub redirect_uri: String,
    pub auto_create_users: bool,
    pub default_role: String,
    pub active: bool,
    pub icon_url: Option<String>,
    pub button_label: Option<String>,
    pub created_at: chrono::DateTime<chrono::Utc>,
    pub updated_at: chrono::DateTime<chrono::Utc>,
}

/// PURPOSE: Links a local user account to an OIDC provider identity
/// EXTERNAL: Maps to oidc_user_links table
#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct OidcUserLink {
    pub id: Uuid,
    pub user_id: Uuid,
    pub provider_id: Uuid,
    pub subject: String,
    pub email: String,
    pub linked_at: chrono::DateTime<chrono::Utc>,
}

/// PURPOSE: Request payload for creating a new OIDC provider
/// CONSTRAINTS: name, issuer_url, client_id, client_secret, redirect_uri are required
#[derive(Debug, Clone, Deserialize)]
pub struct CreateOidcProviderRequest {
    pub name: String,
    pub issuer_url: String,
    pub client_id: String,
    pub client_secret: String,
    pub scopes: Option<String>,
    pub redirect_uri: String,
    pub auto_create_users: Option<bool>,
    pub default_role: Option<String>,
    pub icon_url: Option<String>,
    pub button_label: Option<String>,
}

/// PURPOSE: Request payload for updating an existing OIDC provider
/// CONSTRAINTS: All fields optional — only provided fields are updated
#[derive(Debug, Clone, Deserialize)]
pub struct UpdateOidcProviderRequest {
    pub name: Option<String>,
    pub issuer_url: Option<String>,
    pub client_id: Option<String>,
    pub client_secret: Option<String>,
    pub scopes: Option<String>,
    pub redirect_uri: Option<String>,
    pub auto_create_users: Option<bool>,
    pub default_role: Option<String>,
    pub active: Option<bool>,
    pub icon_url: Option<String>,
    pub button_label: Option<String>,
}

/// PURPOSE: Public-facing OIDC provider info shown on the login page
/// CONSTRAINTS: Must NOT expose client_secret or internal config details
#[derive(Debug, Clone, Serialize)]
pub struct OidcLoginProvider {
    pub id: Uuid,
    pub name: String,
    pub icon_url: Option<String>,
    pub button_label: Option<String>,
}

/// PURPOSE: Request payload for OIDC callback — code exchange
/// CONSTRAINTS:
/// - `code` and `state` come straight from the IdP redirect query string.
/// - `provider_id` is required so the backend can look up the IdP config
///   (token endpoint, client_secret, etc.). The SPA stores it in
///   sessionStorage alongside `state` when calling /api/auth/oidc/{id}/authorize
///   and re-sends it on the callback POST. Same shape as SAML's RelayState
///   carrying the SAML config UUID (TMAIL-303).
#[derive(Debug, Clone, Deserialize)]
pub struct OidcCallbackRequest {
    pub code: String,
    pub state: String,
    // Added (TMAIL-304): provider id so we can resolve client_secret + token_endpoint.
    #[serde(default)]
    pub provider_id: Option<Uuid>,
}

/// PURPOSE: Subset of the OIDC discovery document we care about
/// (RFC 8414 / OpenID Connect Discovery 1.0).
/// EXTERNAL: GET {issuer_url}/.well-known/openid-configuration
///
/// We pin the fields the token-exchange + JWKS validation paths need;
/// extra fields are tolerated by serde so we don't break when providers
/// add new discovery keys.
#[derive(Debug, Clone, Deserialize)]
pub struct OidcDiscovery {
    pub issuer: String,
    pub token_endpoint: String,
    pub jwks_uri: String,
    #[serde(default)]
    pub userinfo_endpoint: Option<String>,
    #[serde(default)]
    pub id_token_signing_alg_values_supported: Option<Vec<String>>,
}

/// PURPOSE: Token-endpoint response per RFC 6749 §5.1 + OIDC Core §3.1.3.3.
/// CONSTRAINTS: `id_token` is the load-bearing field — `access_token` /
/// `refresh_token` aren't used by the current flow (we issue our own JWTs).
#[derive(Debug, Clone, Deserialize)]
pub struct OidcTokenResponse {
    #[serde(default)]
    pub access_token: Option<String>,
    pub id_token: String,
    #[serde(default)]
    pub refresh_token: Option<String>,
    #[serde(default)]
    pub token_type: Option<String>,
    #[serde(default)]
    pub expires_in: Option<i64>,
    #[serde(default)]
    pub scope: Option<String>,
}

/// PURPOSE: Claims we extract from a verified id_token (OIDC Core §2).
/// CONSTRAINTS:
/// - `sub` is the stable external identifier — that's what `oidc_user_links.subject` stores.
/// - `email` is preferred but optional in the spec; we also fall back to
///   `preferred_username` when it looks like an email.
/// - Extra claims (aud, iss, iat, exp, etc.) are validated by jsonwebtoken
///   via the Validation struct, so we don't need to capture them here.
#[derive(Debug, Clone, Deserialize)]
pub struct OidcIdTokenClaims {
    pub sub: String,
    #[serde(default)]
    pub email: Option<String>,
    #[serde(default)]
    pub email_verified: Option<bool>,
    #[serde(default)]
    pub name: Option<String>,
    #[serde(default)]
    pub preferred_username: Option<String>,
    #[serde(default)]
    pub given_name: Option<String>,
    #[serde(default)]
    pub family_name: Option<String>,
}

impl OidcIdTokenClaims {
    /// Added (TMAIL-304): Pure helper resolving the email we'll provision
    /// the local mailbox under. Priority:
    ///   1. `email` claim, if it contains '@'
    ///   2. `preferred_username` claim, if it contains '@'
    /// Both are trimmed + lowercased to match `Mailbox::find_by_username`.
    /// Returns None when neither claim yields an email-shaped value.
    pub fn resolve_email(&self) -> Option<String> {
        self.email
            .as_deref()
            .or(self.preferred_username.as_deref())
            .map(str::trim)
            .filter(|s| !s.is_empty() && s.contains('@'))
            .map(str::to_lowercase)
    }

    /// Added (TMAIL-304): Display name preference for auto-provisioned mailboxes.
    /// Priority: `name` > `given_name family_name` > preferred_username > None.
    pub fn resolve_display_name(&self) -> Option<String> {
        if let Some(n) = self.name.as_deref().map(str::trim).filter(|s| !s.is_empty()) {
            return Some(n.to_string());
        }
        match (
            self.given_name.as_deref().map(str::trim).filter(|s| !s.is_empty()),
            self.family_name.as_deref().map(str::trim).filter(|s| !s.is_empty()),
        ) {
            (Some(g), Some(f)) => Some(format!("{g} {f}")),
            (Some(g), None) => Some(g.to_string()),
            (None, Some(f)) => Some(f.to_string()),
            (None, None) => self
                .preferred_username
                .as_deref()
                .map(str::trim)
                .filter(|s| !s.is_empty())
                .map(str::to_string),
        }
    }
}

impl OidcProvider {
    /// Fetch all OIDC providers
    pub async fn list(pool: &PgPool) -> Result<Vec<OidcProvider>, sqlx::Error> {
        sqlx::query_as::<_, OidcProvider>(
            "SELECT * FROM oidc_providers ORDER BY created_at DESC",
        )
        .fetch_all(pool)
        .await
    }

    /// Fetch only active OIDC providers (for login page display)
    pub async fn list_active(pool: &PgPool) -> Result<Vec<OidcProvider>, sqlx::Error> {
        sqlx::query_as::<_, OidcProvider>(
            "SELECT * FROM oidc_providers WHERE active = true ORDER BY name ASC",
        )
        .fetch_all(pool)
        .await
    }

    /// Fetch a single OIDC provider by ID
    pub async fn get_by_id(pool: &PgPool, id: Uuid) -> Result<OidcProvider, sqlx::Error> {
        sqlx::query_as::<_, OidcProvider>(
            "SELECT * FROM oidc_providers WHERE id = $1",
        )
        .bind(id)
        .fetch_one(pool)
        .await
    }

    /// Create a new OIDC provider
    pub async fn create(
        pool: &PgPool,
        request: &CreateOidcProviderRequest,
        encrypted_secret: &str,
    ) -> Result<OidcProvider, sqlx::Error> {
        sqlx::query_as::<_, OidcProvider>(
            "INSERT INTO oidc_providers (name, issuer_url, client_id, client_secret_encrypted, scopes, redirect_uri, auto_create_users, default_role, icon_url, button_label)
             VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10)
             RETURNING *",
        )
        .bind(&request.name)
        .bind(&request.issuer_url)
        .bind(&request.client_id)
        .bind(encrypted_secret)
        .bind(request.scopes.as_deref().unwrap_or("openid email profile"))
        .bind(&request.redirect_uri)
        .bind(request.auto_create_users.unwrap_or(false))
        .bind(request.default_role.as_deref().unwrap_or("user"))
        .bind(&request.icon_url)
        .bind(&request.button_label)
        .fetch_one(pool)
        .await
    }

    /// Update an existing OIDC provider with partial fields
    pub async fn update(
        pool: &PgPool,
        id: Uuid,
        request: &UpdateOidcProviderRequest,
        encrypted_secret: Option<&str>,
    ) -> Result<OidcProvider, sqlx::Error> {
        sqlx::query_as::<_, OidcProvider>(
            "UPDATE oidc_providers SET
                name = COALESCE($2, name),
                issuer_url = COALESCE($3, issuer_url),
                client_id = COALESCE($4, client_id),
                client_secret_encrypted = COALESCE($5, client_secret_encrypted),
                scopes = COALESCE($6, scopes),
                redirect_uri = COALESCE($7, redirect_uri),
                auto_create_users = COALESCE($8, auto_create_users),
                default_role = COALESCE($9, default_role),
                active = COALESCE($10, active),
                icon_url = COALESCE($11, icon_url),
                button_label = COALESCE($12, button_label),
                updated_at = now()
             WHERE id = $1
             RETURNING *",
        )
        .bind(id)
        .bind(&request.name)
        .bind(&request.issuer_url)
        .bind(&request.client_id)
        .bind(encrypted_secret)
        .bind(&request.scopes)
        .bind(&request.redirect_uri)
        .bind(request.auto_create_users)
        .bind(&request.default_role)
        .bind(request.active)
        .bind(&request.icon_url)
        .bind(&request.button_label)
        .fetch_one(pool)
        .await
    }

    /// Delete an OIDC provider by ID
    pub async fn delete(pool: &PgPool, id: Uuid) -> Result<(), sqlx::Error> {
        sqlx::query("DELETE FROM oidc_providers WHERE id = $1")
            .bind(id)
            .execute(pool)
            .await?;
        Ok(())
    }

    /// Build the authorization URL for initiating OIDC login flow
    /// NOTE: Returns the full URL the frontend should redirect the user to
    pub fn build_authorize_url(&self, state: &str) -> String {
        // Added: Construct standard OIDC authorization endpoint URL
        let issuer = self.issuer_url.trim_end_matches('/');
        format!(
            "{}/authorize?client_id={}&redirect_uri={}&response_type=code&scope={}&state={}",
            issuer,
            urlencoding::encode(&self.client_id),
            urlencoding::encode(&self.redirect_uri),
            urlencoding::encode(&self.scopes),
            urlencoding::encode(state),
        )
    }

    /// Added (TMAIL-304): OIDC Discovery 1.0 well-known URL.
    /// Strips trailing '/' to avoid the `//` double-slash that breaks some IdPs.
    pub fn discovery_url(&self) -> String {
        format!(
            "{}/.well-known/openid-configuration",
            self.issuer_url.trim_end_matches('/')
        )
    }
}

impl OidcUserLink {
    /// Find a user link by provider and subject (external user ID)
    pub async fn find_by_provider_subject(
        pool: &PgPool,
        provider_id: Uuid,
        subject: &str,
    ) -> Result<Option<OidcUserLink>, sqlx::Error> {
        sqlx::query_as::<_, OidcUserLink>(
            "SELECT * FROM oidc_user_links WHERE provider_id = $1 AND subject = $2",
        )
        .bind(provider_id)
        .bind(subject)
        .fetch_optional(pool)
        .await
    }

    /// Create a new user-provider link
    pub async fn create(
        pool: &PgPool,
        user_id: Uuid,
        provider_id: Uuid,
        subject: &str,
        email: &str,
    ) -> Result<OidcUserLink, sqlx::Error> {
        sqlx::query_as::<_, OidcUserLink>(
            "INSERT INTO oidc_user_links (user_id, provider_id, subject, email) VALUES ($1, $2, $3, $4) RETURNING *",
        )
        .bind(user_id)
        .bind(provider_id)
        .bind(subject)
        .bind(email)
        .fetch_one(pool)
        .await
    }

    /// List all OIDC links for a user
    pub async fn list_by_user(
        pool: &PgPool,
        user_id: Uuid,
    ) -> Result<Vec<OidcUserLink>, sqlx::Error> {
        sqlx::query_as::<_, OidcUserLink>(
            "SELECT * FROM oidc_user_links WHERE user_id = $1 ORDER BY linked_at DESC",
        )
        .bind(user_id)
        .fetch_all(pool)
        .await
    }
}

/// Added: URL encoding utility for building authorization URLs
mod urlencoding {
    /// PURPOSE: Percent-encode a string for use in URL query parameters
    /// CONSTRAINTS: Encodes all characters except unreserved ones (RFC 3986)
    pub fn encode(input: &str) -> String {
        let mut encoded = String::with_capacity(input.len() * 2);
        for byte in input.bytes() {
            match byte {
                b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                    encoded.push(byte as char);
                }
                _ => {
                    encoded.push_str(&format!("%{:02X}", byte));
                }
            }
        }
        encoded
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_create_oidc_provider_request_deserializes_minimal() {
        // Added: Verify minimal required fields deserialize correctly
        let json_str = r##"{
            "name": "Google",
            "issuer_url": "https://accounts.google.com",
            "client_id": "123456.apps.googleusercontent.com",
            "client_secret": "GOCSPX-secret",
            "redirect_uri": "https://mail.example.com/api/auth/oidc/callback"
        }"##;
        let request: CreateOidcProviderRequest = serde_json::from_str(json_str).unwrap();
        assert_eq!(request.name, "Google");
        assert_eq!(request.issuer_url, "https://accounts.google.com");
        assert_eq!(request.client_id, "123456.apps.googleusercontent.com");
        assert_eq!(request.client_secret, "GOCSPX-secret");
        assert_eq!(request.redirect_uri, "https://mail.example.com/api/auth/oidc/callback");
        assert!(request.scopes.is_none());
        assert!(request.auto_create_users.is_none());
        assert!(request.default_role.is_none());
        assert!(request.icon_url.is_none());
        assert!(request.button_label.is_none());
    }

    #[test]
    fn test_create_oidc_provider_request_deserializes_all_fields() {
        // Added: Verify all fields including optional ones deserialize correctly
        let json_str = r##"{
            "name": "Microsoft",
            "issuer_url": "https://login.microsoftonline.com/common/v2.0",
            "client_id": "ms-client-id",
            "client_secret": "ms-secret",
            "scopes": "openid email profile User.Read",
            "redirect_uri": "https://mail.example.com/api/auth/oidc/callback",
            "auto_create_users": true,
            "default_role": "user",
            "icon_url": "https://cdn.example.com/ms-icon.svg",
            "button_label": "Sign in with Microsoft"
        }"##;
        let request: CreateOidcProviderRequest = serde_json::from_str(json_str).unwrap();
        assert_eq!(request.name, "Microsoft");
        assert_eq!(request.scopes.as_deref(), Some("openid email profile User.Read"));
        assert_eq!(request.auto_create_users, Some(true));
        assert_eq!(request.default_role.as_deref(), Some("user"));
        assert_eq!(request.icon_url.as_deref(), Some("https://cdn.example.com/ms-icon.svg"));
        assert_eq!(request.button_label.as_deref(), Some("Sign in with Microsoft"));
    }

    #[test]
    fn test_update_oidc_provider_request_deserializes_partial() {
        // Added: Verify partial update with only some fields
        let json_str = r##"{"name": "Updated Google", "active": false}"##;
        let request: UpdateOidcProviderRequest = serde_json::from_str(json_str).unwrap();
        assert_eq!(request.name.as_deref(), Some("Updated Google"));
        assert_eq!(request.active, Some(false));
        assert!(request.issuer_url.is_none());
        assert!(request.client_id.is_none());
        assert!(request.client_secret.is_none());
        assert!(request.scopes.is_none());
        assert!(request.redirect_uri.is_none());
    }

    #[test]
    fn test_update_oidc_provider_request_deserializes_empty() {
        let json_str = "{}";
        let request: UpdateOidcProviderRequest = serde_json::from_str(json_str).unwrap();
        assert!(request.name.is_none());
        assert!(request.active.is_none());
        assert!(request.issuer_url.is_none());
    }

    #[test]
    fn test_oidc_provider_serializes_without_secret() {
        // Added: Verify client_secret_encrypted is skipped in serialization
        let provider = OidcProvider {
            id: Uuid::new_v4(),
            name: "Google".to_string(),
            issuer_url: "https://accounts.google.com".to_string(),
            client_id: "123456.apps.googleusercontent.com".to_string(),
            client_secret_encrypted: "encrypted_secret".to_string(),
            scopes: "openid email profile".to_string(),
            redirect_uri: "https://mail.example.com/callback".to_string(),
            auto_create_users: false,
            default_role: "user".to_string(),
            active: true,
            icon_url: Some("https://cdn.example.com/google.svg".to_string()),
            button_label: Some("Sign in with Google".to_string()),
            created_at: chrono::Utc::now(),
            updated_at: chrono::Utc::now(),
        };

        let json_value = serde_json::to_value(&provider).unwrap();
        assert_eq!(json_value["name"], "Google");
        assert_eq!(json_value["issuer_url"], "https://accounts.google.com");
        assert_eq!(json_value["client_id"], "123456.apps.googleusercontent.com");
        assert_eq!(json_value["scopes"], "openid email profile");
        assert_eq!(json_value["auto_create_users"], false);
        assert_eq!(json_value["active"], true);
        assert_eq!(json_value["icon_url"], "https://cdn.example.com/google.svg");
        assert_eq!(json_value["button_label"], "Sign in with Google");
        // NOTE: client_secret_encrypted should NOT appear in serialized output
        assert!(json_value.get("client_secret_encrypted").is_none());
    }

    #[test]
    fn test_oidc_provider_serializes_with_null_optional_fields() {
        // Added: Verify null optional fields serialize correctly
        let provider = OidcProvider {
            id: Uuid::new_v4(),
            name: "Basic Provider".to_string(),
            issuer_url: "https://idp.example.com".to_string(),
            client_id: "client-id".to_string(),
            client_secret_encrypted: "enc".to_string(),
            scopes: "openid email".to_string(),
            redirect_uri: "https://mail.example.com/cb".to_string(),
            auto_create_users: true,
            default_role: "user".to_string(),
            active: false,
            icon_url: None,
            button_label: None,
            created_at: chrono::Utc::now(),
            updated_at: chrono::Utc::now(),
        };

        let json_value = serde_json::to_value(&provider).unwrap();
        assert_eq!(json_value["name"], "Basic Provider");
        assert_eq!(json_value["auto_create_users"], true);
        assert_eq!(json_value["active"], false);
        assert!(json_value["icon_url"].is_null());
        assert!(json_value["button_label"].is_null());
    }

    #[test]
    fn test_oidc_user_link_serializes_correctly() {
        // Added: Verify user link serialization
        let link = OidcUserLink {
            id: Uuid::new_v4(),
            user_id: Uuid::new_v4(),
            provider_id: Uuid::new_v4(),
            subject: "google-sub-12345".to_string(),
            email: "user@gmail.com".to_string(),
            linked_at: chrono::Utc::now(),
        };

        let json_value = serde_json::to_value(&link).unwrap();
        assert_eq!(json_value["subject"], "google-sub-12345");
        assert_eq!(json_value["email"], "user@gmail.com");
        assert!(json_value["user_id"].is_string());
        assert!(json_value["provider_id"].is_string());
    }

    #[test]
    fn test_oidc_login_provider_serializes_public_fields_only() {
        // Added: Verify public login provider response contains only safe fields
        let login_provider = OidcLoginProvider {
            id: Uuid::new_v4(),
            name: "Google".to_string(),
            icon_url: Some("https://cdn.example.com/google.svg".to_string()),
            button_label: Some("Sign in with Google".to_string()),
        };

        let json_value = serde_json::to_value(&login_provider).unwrap();
        assert_eq!(json_value["name"], "Google");
        assert_eq!(json_value["icon_url"], "https://cdn.example.com/google.svg");
        assert_eq!(json_value["button_label"], "Sign in with Google");
        // NOTE: Should not contain any secret or config fields
        assert!(json_value.get("client_id").is_none());
        assert!(json_value.get("client_secret_encrypted").is_none());
        assert!(json_value.get("issuer_url").is_none());
    }

    #[test]
    fn test_oidc_callback_request_deserializes() {
        // Added: Verify callback request from OIDC authorization server
        let json_str = r##"{"code": "4/0AX4XfWh...", "state": "random-state-token"}"##;
        let request: OidcCallbackRequest = serde_json::from_str(json_str).unwrap();
        assert_eq!(request.code, "4/0AX4XfWh...");
        assert_eq!(request.state, "random-state-token");
        // Added (TMAIL-304): provider_id is optional in the wire format so
        // legacy clients keep parsing; the handler enforces it as required.
        assert!(request.provider_id.is_none());
    }

    // -----------------------------------------------------------------
    // Added (TMAIL-304): coverage for the new payload shapes introduced
    // by the OIDC callback implementation. Pure-function tests only —
    // the HTTP-layer suite in tests/oidc_test.rs pins the early-return
    // guards against the actual router.
    // -----------------------------------------------------------------

    #[test]
    fn callback_request_round_trips_provider_id() {
        let pid = Uuid::new_v4();
        let json_str = format!(
            r##"{{"code":"abc","state":"xyz","provider_id":"{}"}}"##,
            pid
        );
        let req: OidcCallbackRequest = serde_json::from_str(&json_str).unwrap();
        assert_eq!(req.provider_id, Some(pid));
    }

    #[test]
    fn discovery_document_parses_minimal_required_fields() {
        // Real Google discovery has 30+ fields; we only need three.
        let json_str = r##"{
            "issuer": "https://accounts.google.com",
            "authorization_endpoint": "https://accounts.google.com/o/oauth2/v2/auth",
            "token_endpoint": "https://oauth2.googleapis.com/token",
            "jwks_uri": "https://www.googleapis.com/oauth2/v3/certs",
            "userinfo_endpoint": "https://openidconnect.googleapis.com/v1/userinfo",
            "response_types_supported": ["code","token","id_token","code token","code id_token","token id_token","code token id_token","none"],
            "id_token_signing_alg_values_supported": ["RS256"]
        }"##;
        let disc: OidcDiscovery = serde_json::from_str(json_str).unwrap();
        assert_eq!(disc.issuer, "https://accounts.google.com");
        assert_eq!(disc.token_endpoint, "https://oauth2.googleapis.com/token");
        assert_eq!(disc.jwks_uri, "https://www.googleapis.com/oauth2/v3/certs");
        assert_eq!(
            disc.userinfo_endpoint.as_deref(),
            Some("https://openidconnect.googleapis.com/v1/userinfo")
        );
        assert_eq!(
            disc.id_token_signing_alg_values_supported.unwrap(),
            vec!["RS256".to_string()]
        );
    }

    #[test]
    fn token_response_parses_oidc_payload() {
        let json_str = r##"{
            "access_token": "ya29.a0...",
            "expires_in": 3599,
            "scope": "openid email profile",
            "token_type": "Bearer",
            "id_token": "eyJhbGciOiJSUzI1NiIsImtpZCI6IkFC...",
            "refresh_token": "1//0e..."
        }"##;
        let tok: OidcTokenResponse = serde_json::from_str(json_str).unwrap();
        assert!(tok.id_token.starts_with("eyJ"));
        assert_eq!(tok.access_token.as_deref(), Some("ya29.a0..."));
        assert_eq!(tok.refresh_token.as_deref(), Some("1//0e..."));
        assert_eq!(tok.token_type.as_deref(), Some("Bearer"));
        assert_eq!(tok.expires_in, Some(3599));
    }

    #[test]
    fn token_response_tolerates_id_token_only() {
        // Some IdPs (or custom configs) only return id_token + token_type.
        let json_str = r##"{"id_token":"eyJ...","token_type":"Bearer"}"##;
        let tok: OidcTokenResponse = serde_json::from_str(json_str).unwrap();
        assert_eq!(tok.id_token, "eyJ...");
        assert!(tok.access_token.is_none());
        assert!(tok.refresh_token.is_none());
        assert!(tok.expires_in.is_none());
    }

    #[test]
    fn id_token_claims_parse_typical_google_payload() {
        // Sanitised structure from a real Google id_token.
        let json_str = r##"{
            "iss": "https://accounts.google.com",
            "azp": "client-id",
            "aud": "client-id",
            "sub": "1093427812345",
            "email": "User@Example.com",
            "email_verified": true,
            "name": "Jane Doe",
            "given_name": "Jane",
            "family_name": "Doe",
            "iat": 1700000000,
            "exp": 1700003600
        }"##;
        let c: OidcIdTokenClaims = serde_json::from_str(json_str).unwrap();
        assert_eq!(c.sub, "1093427812345");
        assert_eq!(c.email_verified, Some(true));
        assert_eq!(c.resolve_email().as_deref(), Some("user@example.com"));
        assert_eq!(c.resolve_display_name().as_deref(), Some("Jane Doe"));
    }

    #[test]
    fn id_token_resolve_email_falls_back_to_preferred_username() {
        let c = OidcIdTokenClaims {
            sub: "abc".into(),
            email: None,
            email_verified: None,
            name: None,
            preferred_username: Some("  USER@CORP.io  ".into()),
            given_name: None,
            family_name: None,
        };
        assert_eq!(c.resolve_email().as_deref(), Some("user@corp.io"));
    }

    #[test]
    fn id_token_resolve_email_rejects_non_email_subject() {
        // Microsoft sometimes uses an opaque oid for preferred_username.
        let c = OidcIdTokenClaims {
            sub: "abc".into(),
            email: None,
            email_verified: None,
            name: None,
            preferred_username: Some("opaque-oid-no-at-sign".into()),
            given_name: None,
            family_name: None,
        };
        assert_eq!(c.resolve_email(), None);
    }

    #[test]
    fn id_token_resolve_email_returns_none_when_both_claims_absent() {
        let c = OidcIdTokenClaims {
            sub: "abc".into(),
            email: None,
            email_verified: None,
            name: None,
            preferred_username: None,
            given_name: None,
            family_name: None,
        };
        assert_eq!(c.resolve_email(), None);
    }

    #[test]
    fn id_token_resolve_display_name_concatenates_given_family() {
        let c = OidcIdTokenClaims {
            sub: "abc".into(),
            email: Some("u@x.com".into()),
            email_verified: None,
            name: None,
            preferred_username: None,
            given_name: Some("Jane".into()),
            family_name: Some("Doe".into()),
        };
        assert_eq!(c.resolve_display_name().as_deref(), Some("Jane Doe"));
    }

    #[test]
    fn id_token_resolve_display_name_returns_none_when_all_absent() {
        let c = OidcIdTokenClaims {
            sub: "abc".into(),
            email: Some("u@x.com".into()),
            email_verified: None,
            name: None,
            preferred_username: None,
            given_name: None,
            family_name: None,
        };
        assert_eq!(c.resolve_display_name(), None);
    }

    #[test]
    fn discovery_url_trims_trailing_slash() {
        let provider = OidcProvider {
            id: Uuid::new_v4(),
            name: "Microsoft".to_string(),
            issuer_url: "https://login.microsoftonline.com/common/v2.0/".to_string(),
            client_id: "x".to_string(),
            client_secret_encrypted: "y".to_string(),
            scopes: "openid email profile".to_string(),
            redirect_uri: "https://mail.example.com/cb".to_string(),
            auto_create_users: true,
            default_role: "user".to_string(),
            active: true,
            icon_url: None,
            button_label: None,
            created_at: chrono::Utc::now(),
            updated_at: chrono::Utc::now(),
        };
        assert_eq!(
            provider.discovery_url(),
            "https://login.microsoftonline.com/common/v2.0/.well-known/openid-configuration"
        );
    }

    #[test]
    fn discovery_url_works_without_trailing_slash() {
        let provider = OidcProvider {
            id: Uuid::new_v4(),
            name: "Google".to_string(),
            issuer_url: "https://accounts.google.com".to_string(),
            client_id: "x".to_string(),
            client_secret_encrypted: "y".to_string(),
            scopes: "openid email profile".to_string(),
            redirect_uri: "https://mail.example.com/cb".to_string(),
            auto_create_users: true,
            default_role: "user".to_string(),
            active: true,
            icon_url: None,
            button_label: None,
            created_at: chrono::Utc::now(),
            updated_at: chrono::Utc::now(),
        };
        assert_eq!(
            provider.discovery_url(),
            "https://accounts.google.com/.well-known/openid-configuration"
        );
    }

    #[test]
    fn test_build_authorize_url_google() {
        // Added: Verify authorization URL construction for Google provider
        let provider = OidcProvider {
            id: Uuid::new_v4(),
            name: "Google".to_string(),
            issuer_url: "https://accounts.google.com".to_string(),
            client_id: "123456.apps.googleusercontent.com".to_string(),
            client_secret_encrypted: "enc".to_string(),
            scopes: "openid email profile".to_string(),
            redirect_uri: "https://mail.example.com/api/auth/oidc/callback".to_string(),
            auto_create_users: false,
            default_role: "user".to_string(),
            active: true,
            icon_url: None,
            button_label: None,
            created_at: chrono::Utc::now(),
            updated_at: chrono::Utc::now(),
        };

        let url = provider.build_authorize_url("test-state-123");
        assert!(url.starts_with("https://accounts.google.com/authorize?"));
        assert!(url.contains("client_id=123456.apps.googleusercontent.com"));
        assert!(url.contains("response_type=code"));
        assert!(url.contains("scope=openid%20email%20profile"));
        assert!(url.contains("state=test-state-123"));
        assert!(url.contains("redirect_uri=https%3A%2F%2Fmail.example.com%2Fapi%2Fauth%2Foidc%2Fcallback"));
    }

    #[test]
    fn test_build_authorize_url_microsoft() {
        // Added: Verify authorization URL construction for Microsoft provider with trailing slash
        let provider = OidcProvider {
            id: Uuid::new_v4(),
            name: "Microsoft".to_string(),
            issuer_url: "https://login.microsoftonline.com/common/v2.0/".to_string(),
            client_id: "ms-client-id".to_string(),
            client_secret_encrypted: "enc".to_string(),
            scopes: "openid email profile User.Read".to_string(),
            redirect_uri: "https://mail.example.com/api/auth/oidc/callback".to_string(),
            auto_create_users: true,
            default_role: "user".to_string(),
            active: true,
            icon_url: None,
            button_label: None,
            created_at: chrono::Utc::now(),
            updated_at: chrono::Utc::now(),
        };

        let url = provider.build_authorize_url("ms-state-456");
        // NOTE: Trailing slash on issuer_url should be trimmed
        assert!(url.starts_with("https://login.microsoftonline.com/common/v2.0/authorize?"));
        assert!(url.contains("client_id=ms-client-id"));
        assert!(url.contains("scope=openid%20email%20profile%20User.Read"));
        assert!(url.contains("state=ms-state-456"));
    }

    #[test]
    fn test_urlencoding_encode() {
        // Added: Verify URL encoding helper
        assert_eq!(urlencoding::encode("hello"), "hello");
        assert_eq!(urlencoding::encode("hello world"), "hello%20world");
        assert_eq!(urlencoding::encode("a+b=c&d"), "a%2Bb%3Dc%26d");
        assert_eq!(urlencoding::encode("https://example.com/path"), "https%3A%2F%2Fexample.com%2Fpath");
    }
}
