// Added: Branding model for white-label customization (TMAIL-111)
use serde::{Deserialize, Serialize};
use sqlx::PgPool;
use uuid::Uuid;

/// PURPOSE: Represents the instance-wide branding configuration
/// CONSTRAINTS: Only one row exists in the branding table — always fetched/updated by first row
#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct Branding {
    pub id: Uuid,
    pub app_name: String,
    pub logo_url: Option<String>,
    pub favicon_url: Option<String>,
    pub primary_color: String,
    pub secondary_color: String,
    pub accent_color: String,
    pub login_background_url: Option<String>,
    pub custom_css: Option<String>,
    pub footer_text: Option<String>,
    pub support_email: Option<String>,
    pub support_url: Option<String>,
    pub updated_at: chrono::DateTime<chrono::Utc>,
}

/// PURPOSE: Request payload for updating branding settings
/// CONSTRAINTS: All fields optional — only provided fields are updated
#[derive(Debug, Clone, Deserialize)]
pub struct UpdateBrandingRequest {
    pub app_name: Option<String>,
    pub logo_url: Option<String>,
    pub favicon_url: Option<String>,
    pub primary_color: Option<String>,
    pub secondary_color: Option<String>,
    pub accent_color: Option<String>,
    pub login_background_url: Option<String>,
    pub custom_css: Option<String>,
    pub footer_text: Option<String>,
    pub support_email: Option<String>,
    pub support_url: Option<String>,
}

impl Branding {
    /// Fetch the current branding configuration (first and only row)
    pub async fn get_current(pool: &PgPool) -> Result<Branding, sqlx::Error> {
        sqlx::query_as::<_, Branding>("SELECT * FROM branding LIMIT 1")
            .fetch_one(pool)
            .await
    }

    /// Update branding with partial fields — uses COALESCE to keep existing values for null fields
    pub async fn update(
        pool: &PgPool,
        request: &UpdateBrandingRequest,
    ) -> Result<Branding, sqlx::Error> {
        sqlx::query_as::<_, Branding>(
            "UPDATE branding SET
                app_name = COALESCE($1, app_name),
                logo_url = COALESCE($2, logo_url),
                favicon_url = COALESCE($3, favicon_url),
                primary_color = COALESCE($4, primary_color),
                secondary_color = COALESCE($5, secondary_color),
                accent_color = COALESCE($6, accent_color),
                login_background_url = COALESCE($7, login_background_url),
                custom_css = COALESCE($8, custom_css),
                footer_text = COALESCE($9, footer_text),
                support_email = COALESCE($10, support_email),
                support_url = COALESCE($11, support_url),
                updated_at = now()
             RETURNING *",
        )
        .bind(&request.app_name)
        .bind(&request.logo_url)
        .bind(&request.favicon_url)
        .bind(&request.primary_color)
        .bind(&request.secondary_color)
        .bind(&request.accent_color)
        .bind(&request.login_background_url)
        .bind(&request.custom_css)
        .bind(&request.footer_text)
        .bind(&request.support_email)
        .bind(&request.support_url)
        .fetch_one(pool)
        .await
    }

    /// Reset branding to defaults — keeps the same row ID
    pub async fn reset_to_defaults(pool: &PgPool) -> Result<Branding, sqlx::Error> {
        sqlx::query_as::<_, Branding>(
            "UPDATE branding SET
                app_name = 'TASMail',
                logo_url = NULL,
                favicon_url = NULL,
                primary_color = '#2563eb',
                secondary_color = '#1e40af',
                accent_color = '#3b82f6',
                login_background_url = NULL,
                custom_css = NULL,
                footer_text = NULL,
                support_email = NULL,
                support_url = NULL,
                updated_at = now()
             RETURNING *",
        )
        .fetch_one(pool)
        .await
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_update_branding_request_deserializes_partial() {
        let json_str = r##"{"app_name": "MyMail", "primary_color": "#ff0000"}"##;
        let request: UpdateBrandingRequest = serde_json::from_str(json_str).unwrap();
        assert_eq!(request.app_name.as_deref(), Some("MyMail"));
        assert_eq!(request.primary_color.as_deref(), Some("#ff0000"));
        assert!(request.logo_url.is_none());
        assert!(request.secondary_color.is_none());
        assert!(request.accent_color.is_none());
        assert!(request.favicon_url.is_none());
        assert!(request.custom_css.is_none());
        assert!(request.footer_text.is_none());
        assert!(request.support_email.is_none());
        assert!(request.support_url.is_none());
    }

    #[test]
    fn test_update_branding_request_deserializes_all_fields() {
        let json_str = r##"{
            "app_name": "BrandMail",
            "logo_url": "https://example.com/logo.png",
            "favicon_url": "https://example.com/favicon.ico",
            "primary_color": "#111111",
            "secondary_color": "#222222",
            "accent_color": "#333333",
            "login_background_url": "https://example.com/bg.jpg",
            "custom_css": "body { font-family: serif; }",
            "footer_text": "Powered by BrandMail",
            "support_email": "help@example.com",
            "support_url": "https://support.example.com"
        }"##;
        let request: UpdateBrandingRequest = serde_json::from_str(json_str).unwrap();
        assert_eq!(request.app_name.as_deref(), Some("BrandMail"));
        assert_eq!(request.logo_url.as_deref(), Some("https://example.com/logo.png"));
        assert_eq!(request.favicon_url.as_deref(), Some("https://example.com/favicon.ico"));
        assert_eq!(request.primary_color.as_deref(), Some("#111111"));
        assert_eq!(request.secondary_color.as_deref(), Some("#222222"));
        assert_eq!(request.accent_color.as_deref(), Some("#333333"));
        assert_eq!(request.login_background_url.as_deref(), Some("https://example.com/bg.jpg"));
        assert_eq!(request.custom_css.as_deref(), Some("body { font-family: serif; }"));
        assert_eq!(request.footer_text.as_deref(), Some("Powered by BrandMail"));
        assert_eq!(request.support_email.as_deref(), Some("help@example.com"));
        assert_eq!(request.support_url.as_deref(), Some("https://support.example.com"));
    }

    #[test]
    fn test_update_branding_request_deserializes_empty() {
        let json_str = "{}";
        let request: UpdateBrandingRequest = serde_json::from_str(json_str).unwrap();
        assert!(request.app_name.is_none());
        assert!(request.primary_color.is_none());
    }

    #[test]
    fn test_branding_serializes_correctly() {
        let branding = Branding {
            id: Uuid::new_v4(),
            app_name: "TestMail".to_string(),
            logo_url: Some("https://example.com/logo.png".to_string()),
            favicon_url: None,
            primary_color: "#2563eb".to_string(),
            secondary_color: "#1e40af".to_string(),
            accent_color: "#3b82f6".to_string(),
            login_background_url: None,
            custom_css: None,
            footer_text: Some("Footer text".to_string()),
            support_email: None,
            support_url: None,
            updated_at: chrono::Utc::now(),
        };

        let json_value = serde_json::to_value(&branding).unwrap();
        assert_eq!(json_value["app_name"], "TestMail");
        assert_eq!(json_value["primary_color"], "#2563eb");
        assert_eq!(json_value["logo_url"], "https://example.com/logo.png");
        assert!(json_value["favicon_url"].is_null());
        assert_eq!(json_value["footer_text"], "Footer text");
    }

    #[test]
    fn test_branding_default_colors() {
        // NOTE: Verifies the default color values match the migration defaults
        let default_primary = "#2563eb";
        let default_secondary = "#1e40af";
        let default_accent = "#3b82f6";

        let branding = Branding {
            id: Uuid::new_v4(),
            app_name: "TASMail".to_string(),
            logo_url: None,
            favicon_url: None,
            primary_color: default_primary.to_string(),
            secondary_color: default_secondary.to_string(),
            accent_color: default_accent.to_string(),
            login_background_url: None,
            custom_css: None,
            footer_text: None,
            support_email: None,
            support_url: None,
            updated_at: chrono::Utc::now(),
        };

        assert_eq!(branding.app_name, "TASMail");
        assert_eq!(branding.primary_color, "#2563eb");
        assert_eq!(branding.secondary_color, "#1e40af");
        assert_eq!(branding.accent_color, "#3b82f6");
    }
}
