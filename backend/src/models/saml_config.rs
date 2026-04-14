// Added: SAML 2.0 SSO configuration and session models for TMAIL-101
use serde::{Deserialize, Serialize};
use sqlx::PgPool;
use uuid::Uuid;

/// PURPOSE: Represents a SAML 2.0 IdP configuration for enterprise SSO
/// CONSTRAINTS: certificate must be a valid X.509 PEM; attribute_mapping must be valid JSON
/// EXTERNAL: Maps to saml_configurations table
#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct SamlConfiguration {
    pub id: Uuid,
    pub name: String,
    pub entity_id: String,
    pub sso_url: String,
    pub slo_url: Option<String>,
    pub certificate: String,
    pub name_id_format: String,
    pub attribute_mapping: serde_json::Value,
    pub active: bool,
    pub created_at: chrono::DateTime<chrono::Utc>,
    pub updated_at: chrono::DateTime<chrono::Utc>,
}

/// PURPOSE: Tracks active SAML sessions for SLO (Single Logout) support
/// EXTERNAL: Maps to saml_sessions table
#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct SamlSession {
    pub id: Uuid,
    pub saml_config_id: Uuid,
    pub user_id: Option<Uuid>,
    pub session_index: Option<String>,
    pub name_id: String,
    pub attributes: serde_json::Value,
    pub created_at: chrono::DateTime<chrono::Utc>,
    pub expires_at: chrono::DateTime<chrono::Utc>,
}

/// PURPOSE: Request payload for creating a new SAML IdP configuration
/// CONSTRAINTS: name, entity_id, sso_url, certificate are required
#[derive(Debug, Clone, Deserialize)]
pub struct CreateSamlConfigRequest {
    pub name: String,
    pub entity_id: String,
    pub sso_url: String,
    pub slo_url: Option<String>,
    pub certificate: String,
    pub name_id_format: Option<String>,
    pub attribute_mapping: Option<serde_json::Value>,
}

/// PURPOSE: Request payload for updating an existing SAML configuration
/// CONSTRAINTS: All fields optional — only provided fields are updated
#[derive(Debug, Clone, Deserialize)]
pub struct UpdateSamlConfigRequest {
    pub name: Option<String>,
    pub entity_id: Option<String>,
    pub sso_url: Option<String>,
    pub slo_url: Option<String>,
    pub certificate: Option<String>,
    pub name_id_format: Option<String>,
    pub attribute_mapping: Option<serde_json::Value>,
    pub active: Option<bool>,
}

impl SamlConfiguration {
    /// Fetch all SAML configurations
    pub async fn list(pool: &PgPool) -> Result<Vec<SamlConfiguration>, sqlx::Error> {
        sqlx::query_as::<_, SamlConfiguration>(
            "SELECT * FROM saml_configurations ORDER BY created_at DESC",
        )
        .fetch_all(pool)
        .await
    }

    /// Fetch a single SAML configuration by ID
    pub async fn get_by_id(pool: &PgPool, id: Uuid) -> Result<SamlConfiguration, sqlx::Error> {
        sqlx::query_as::<_, SamlConfiguration>(
            "SELECT * FROM saml_configurations WHERE id = $1",
        )
        .bind(id)
        .fetch_one(pool)
        .await
    }

    /// Create a new SAML configuration
    pub async fn create(
        pool: &PgPool,
        request: &CreateSamlConfigRequest,
    ) -> Result<SamlConfiguration, sqlx::Error> {
        sqlx::query_as::<_, SamlConfiguration>(
            "INSERT INTO saml_configurations (name, entity_id, sso_url, slo_url, certificate, name_id_format, attribute_mapping)
             VALUES ($1, $2, $3, $4, $5, $6, $7)
             RETURNING *",
        )
        .bind(&request.name)
        .bind(&request.entity_id)
        .bind(&request.sso_url)
        .bind(&request.slo_url)
        .bind(&request.certificate)
        .bind(
            request
                .name_id_format
                .as_deref()
                .unwrap_or("urn:oasis:names:tc:SAML:1.1:nameid-format:emailAddress"),
        )
        .bind(
            request
                .attribute_mapping
                .as_ref()
                .unwrap_or(&serde_json::json!({"email": "email", "name": "displayName"})),
        )
        .fetch_one(pool)
        .await
    }

    /// Update an existing SAML configuration with partial fields
    pub async fn update(
        pool: &PgPool,
        id: Uuid,
        request: &UpdateSamlConfigRequest,
    ) -> Result<SamlConfiguration, sqlx::Error> {
        sqlx::query_as::<_, SamlConfiguration>(
            "UPDATE saml_configurations SET
                name = COALESCE($2, name),
                entity_id = COALESCE($3, entity_id),
                sso_url = COALESCE($4, sso_url),
                slo_url = COALESCE($5, slo_url),
                certificate = COALESCE($6, certificate),
                name_id_format = COALESCE($7, name_id_format),
                attribute_mapping = COALESCE($8, attribute_mapping),
                active = COALESCE($9, active),
                updated_at = now()
             WHERE id = $1
             RETURNING *",
        )
        .bind(id)
        .bind(&request.name)
        .bind(&request.entity_id)
        .bind(&request.sso_url)
        .bind(&request.slo_url)
        .bind(&request.certificate)
        .bind(&request.name_id_format)
        .bind(&request.attribute_mapping)
        .bind(request.active)
        .fetch_one(pool)
        .await
    }

    /// Delete a SAML configuration by ID
    pub async fn delete(pool: &PgPool, id: Uuid) -> Result<(), sqlx::Error> {
        sqlx::query("DELETE FROM saml_configurations WHERE id = $1")
            .bind(id)
            .execute(pool)
            .await?;
        Ok(())
    }

    /// PURPOSE: Build a SAML AuthnRequest redirect URL for the given IdP
    /// CONSTRAINTS: Returns a URL with SAMLRequest query parameter (deflate + base64 encoded)
    /// NOTE: Simplified implementation — production should use a proper SAML library
    pub fn build_authn_request_url(&self, sp_entity_id: &str, acs_url: &str) -> String {
        // Added: Build minimal SAML AuthnRequest XML
        let request_id = format!("_req_{}", Uuid::new_v4());
        let issue_instant = chrono::Utc::now().format("%Y-%m-%dT%H:%M:%SZ").to_string();

        let authn_request = format!(
            r#"<samlp:AuthnRequest xmlns:samlp="urn:oasis:names:tc:SAML:2.0:protocol" xmlns:saml="urn:oasis:names:tc:SAML:2.0:assertion" ID="{request_id}" Version="2.0" IssueInstant="{issue_instant}" Destination="{destination}" AssertionConsumerServiceURL="{acs_url}" ProtocolBinding="urn:oasis:names:tc:SAML:2.0:bindings:HTTP-POST"><saml:Issuer>{sp_entity_id}</saml:Issuer><samlp:NameIDPolicy Format="{name_id_format}" AllowCreate="true"/></samlp:AuthnRequest>"#,
            destination = self.sso_url,
            name_id_format = self.name_id_format,
        );

        // Added: Deflate-compress and base64-encode the AuthnRequest
        use std::io::Write;
        let mut encoder =
            flate2::write::DeflateEncoder::new(Vec::new(), flate2::Compression::default());
        encoder.write_all(authn_request.as_bytes()).unwrap_or_default();
        let compressed = encoder.finish().unwrap_or_default();

        use base64::Engine;
        let encoded = base64::engine::general_purpose::STANDARD.encode(&compressed);
        let url_encoded = urlencoding::encode(&encoded);

        // Added: Construct redirect URL with SAMLRequest query param
        let separator = if self.sso_url.contains('?') { "&" } else { "?" };
        format!("{}{}SAMLRequest={}", self.sso_url, separator, url_encoded)
    }

    /// PURPOSE: Resolve an attribute value from SAML assertion attributes using the mapping
    /// CONSTRAINTS: Returns None if the mapped attribute key is not found
    pub fn resolve_attribute(
        &self,
        attributes: &serde_json::Value,
        logical_name: &str,
    ) -> Option<String> {
        // Added: Look up the IdP attribute name from the mapping, then find it in assertion attributes
        let mapped_key = self.attribute_mapping.get(logical_name)?.as_str()?;
        attributes.get(mapped_key)?.as_str().map(String::from)
    }
}

impl SamlSession {
    /// Create a new SAML session entry
    pub async fn create(
        pool: &PgPool,
        saml_config_id: Uuid,
        user_id: Option<Uuid>,
        session_index: Option<&str>,
        name_id: &str,
        attributes: &serde_json::Value,
        expires_at: chrono::DateTime<chrono::Utc>,
    ) -> Result<SamlSession, sqlx::Error> {
        sqlx::query_as::<_, SamlSession>(
            "INSERT INTO saml_sessions (saml_config_id, user_id, session_index, name_id, attributes, expires_at)
             VALUES ($1, $2, $3, $4, $5, $6)
             RETURNING *",
        )
        .bind(saml_config_id)
        .bind(user_id)
        .bind(session_index)
        .bind(name_id)
        .bind(attributes)
        .bind(expires_at)
        .fetch_one(pool)
        .await
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_create_saml_config_request_deserializes_minimal() {
        // Added: Verify minimal required fields deserialize correctly
        let json_str = r##"{
            "name": "Okta SSO",
            "entity_id": "https://okta.example.com/saml",
            "sso_url": "https://okta.example.com/sso/saml",
            "certificate": "MIICpDCCAYwCCQ..."
        }"##;
        let request: CreateSamlConfigRequest = serde_json::from_str(json_str).unwrap();
        assert_eq!(request.name, "Okta SSO");
        assert_eq!(request.entity_id, "https://okta.example.com/saml");
        assert_eq!(request.sso_url, "https://okta.example.com/sso/saml");
        assert_eq!(request.certificate, "MIICpDCCAYwCCQ...");
        assert!(request.slo_url.is_none());
        assert!(request.name_id_format.is_none());
        assert!(request.attribute_mapping.is_none());
    }

    #[test]
    fn test_create_saml_config_request_deserializes_all_fields() {
        // Added: Verify all fields including optional ones deserialize correctly
        let json_str = r##"{
            "name": "Azure AD",
            "entity_id": "https://sts.windows.net/tenant-id/",
            "sso_url": "https://login.microsoftonline.com/tenant-id/saml2",
            "slo_url": "https://login.microsoftonline.com/tenant-id/saml2/logout",
            "certificate": "MIICpDCCAYwCCQ...",
            "name_id_format": "urn:oasis:names:tc:SAML:1.1:nameid-format:unspecified",
            "attribute_mapping": {"email": "http://schemas.xmlsoap.org/ws/2005/05/identity/claims/emailaddress", "name": "http://schemas.xmlsoap.org/ws/2005/05/identity/claims/name"}
        }"##;
        let request: CreateSamlConfigRequest = serde_json::from_str(json_str).unwrap();
        assert_eq!(request.name, "Azure AD");
        assert_eq!(request.slo_url.as_deref(), Some("https://login.microsoftonline.com/tenant-id/saml2/logout"));
        assert_eq!(
            request.name_id_format.as_deref(),
            Some("urn:oasis:names:tc:SAML:1.1:nameid-format:unspecified")
        );
        assert!(request.attribute_mapping.is_some());
        let mapping = request.attribute_mapping.unwrap();
        assert_eq!(
            mapping["email"],
            "http://schemas.xmlsoap.org/ws/2005/05/identity/claims/emailaddress"
        );
    }

    #[test]
    fn test_update_saml_config_request_deserializes_partial() {
        // Added: Verify partial update with only some fields
        let json_str = r##"{"name": "Updated SSO", "active": false}"##;
        let request: UpdateSamlConfigRequest = serde_json::from_str(json_str).unwrap();
        assert_eq!(request.name.as_deref(), Some("Updated SSO"));
        assert_eq!(request.active, Some(false));
        assert!(request.entity_id.is_none());
        assert!(request.sso_url.is_none());
        assert!(request.certificate.is_none());
        assert!(request.name_id_format.is_none());
        assert!(request.attribute_mapping.is_none());
    }

    #[test]
    fn test_update_saml_config_request_deserializes_empty() {
        let json_str = "{}";
        let request: UpdateSamlConfigRequest = serde_json::from_str(json_str).unwrap();
        assert!(request.name.is_none());
        assert!(request.active.is_none());
        assert!(request.entity_id.is_none());
    }

    #[test]
    fn test_saml_configuration_serializes_correctly() {
        // Added: Verify all fields serialize as expected
        let config = SamlConfiguration {
            id: Uuid::new_v4(),
            name: "Okta SSO".to_string(),
            entity_id: "https://okta.example.com/saml".to_string(),
            sso_url: "https://okta.example.com/sso/saml".to_string(),
            slo_url: Some("https://okta.example.com/slo/saml".to_string()),
            certificate: "MIICpDCCAYwCCQ...".to_string(),
            name_id_format: "urn:oasis:names:tc:SAML:1.1:nameid-format:emailAddress".to_string(),
            attribute_mapping: serde_json::json!({"email": "email", "name": "displayName"}),
            active: true,
            created_at: chrono::Utc::now(),
            updated_at: chrono::Utc::now(),
        };

        let json_value = serde_json::to_value(&config).unwrap();
        assert_eq!(json_value["name"], "Okta SSO");
        assert_eq!(json_value["entity_id"], "https://okta.example.com/saml");
        assert_eq!(json_value["sso_url"], "https://okta.example.com/sso/saml");
        assert_eq!(json_value["slo_url"], "https://okta.example.com/slo/saml");
        assert_eq!(json_value["certificate"], "MIICpDCCAYwCCQ...");
        assert_eq!(json_value["active"], true);
        assert_eq!(json_value["attribute_mapping"]["email"], "email");
    }

    #[test]
    fn test_saml_configuration_serializes_without_slo_url() {
        // Added: Verify config without optional SLO URL serializes with null
        let config = SamlConfiguration {
            id: Uuid::new_v4(),
            name: "OneLogin".to_string(),
            entity_id: "https://onelogin.example.com".to_string(),
            sso_url: "https://onelogin.example.com/sso".to_string(),
            slo_url: None,
            certificate: "CERT_DATA".to_string(),
            name_id_format: "urn:oasis:names:tc:SAML:1.1:nameid-format:emailAddress".to_string(),
            attribute_mapping: serde_json::json!({"email": "User.Email"}),
            active: false,
            created_at: chrono::Utc::now(),
            updated_at: chrono::Utc::now(),
        };

        let json_value = serde_json::to_value(&config).unwrap();
        assert_eq!(json_value["name"], "OneLogin");
        assert!(json_value["slo_url"].is_null());
        assert_eq!(json_value["active"], false);
    }

    #[test]
    fn test_saml_session_serializes_correctly() {
        // Added: Verify SAML session serialization including attributes
        let session = SamlSession {
            id: Uuid::new_v4(),
            saml_config_id: Uuid::new_v4(),
            user_id: Some(Uuid::new_v4()),
            session_index: Some("_session_abc123".to_string()),
            name_id: "user@example.com".to_string(),
            attributes: serde_json::json!({"email": "user@example.com", "name": "Test User"}),
            created_at: chrono::Utc::now(),
            expires_at: chrono::Utc::now() + chrono::Duration::hours(8),
        };

        let json_value = serde_json::to_value(&session).unwrap();
        assert_eq!(json_value["name_id"], "user@example.com");
        assert_eq!(json_value["session_index"], "_session_abc123");
        assert_eq!(json_value["attributes"]["email"], "user@example.com");
        assert!(json_value["user_id"].is_string());
    }

    #[test]
    fn test_build_authn_request_url_contains_saml_request() {
        // Added: Verify AuthnRequest URL generation includes SAMLRequest parameter
        let config = SamlConfiguration {
            id: Uuid::new_v4(),
            name: "Test IdP".to_string(),
            entity_id: "https://idp.example.com".to_string(),
            sso_url: "https://idp.example.com/sso".to_string(),
            slo_url: None,
            certificate: "CERT".to_string(),
            name_id_format: "urn:oasis:names:tc:SAML:1.1:nameid-format:emailAddress".to_string(),
            attribute_mapping: serde_json::json!({"email": "email"}),
            active: true,
            created_at: chrono::Utc::now(),
            updated_at: chrono::Utc::now(),
        };

        let url = config.build_authn_request_url(
            "https://mail.example.com",
            "https://mail.example.com/api/auth/saml/callback",
        );

        assert!(url.starts_with("https://idp.example.com/sso?SAMLRequest="));
        // NOTE: URL should contain a base64-encoded deflated AuthnRequest
        assert!(url.len() > 60);
    }

    #[test]
    fn test_build_authn_request_url_appends_to_existing_query() {
        // Added: Verify URL with existing query params uses '&' separator
        let config = SamlConfiguration {
            id: Uuid::new_v4(),
            name: "Test".to_string(),
            entity_id: "https://idp.example.com".to_string(),
            sso_url: "https://idp.example.com/sso?tenant=abc".to_string(),
            slo_url: None,
            certificate: "CERT".to_string(),
            name_id_format: "urn:oasis:names:tc:SAML:1.1:nameid-format:emailAddress".to_string(),
            attribute_mapping: serde_json::json!({}),
            active: true,
            created_at: chrono::Utc::now(),
            updated_at: chrono::Utc::now(),
        };

        let url = config.build_authn_request_url("https://sp.example.com", "https://sp.example.com/callback");
        assert!(url.contains("?tenant=abc&SAMLRequest="));
    }

    #[test]
    fn test_resolve_attribute_finds_mapped_value() {
        // Added: Verify attribute resolution via mapping lookup
        let config = SamlConfiguration {
            id: Uuid::new_v4(),
            name: "Test".to_string(),
            entity_id: "https://idp.example.com".to_string(),
            sso_url: "https://idp.example.com/sso".to_string(),
            slo_url: None,
            certificate: "CERT".to_string(),
            name_id_format: "urn:oasis:names:tc:SAML:1.1:nameid-format:emailAddress".to_string(),
            attribute_mapping: serde_json::json!({
                "email": "http://schemas.xmlsoap.org/ws/2005/05/identity/claims/emailaddress",
                "name": "http://schemas.xmlsoap.org/ws/2005/05/identity/claims/name"
            }),
            active: true,
            created_at: chrono::Utc::now(),
            updated_at: chrono::Utc::now(),
        };

        let attributes = serde_json::json!({
            "http://schemas.xmlsoap.org/ws/2005/05/identity/claims/emailaddress": "user@corp.com",
            "http://schemas.xmlsoap.org/ws/2005/05/identity/claims/name": "Jane Doe"
        });

        assert_eq!(
            config.resolve_attribute(&attributes, "email"),
            Some("user@corp.com".to_string())
        );
        assert_eq!(
            config.resolve_attribute(&attributes, "name"),
            Some("Jane Doe".to_string())
        );
        // NOTE: Unmapped logical names return None
        assert_eq!(config.resolve_attribute(&attributes, "department"), None);
    }
}
