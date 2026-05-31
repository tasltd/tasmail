// Added: Per-user IMAP configuration (BYOK webmail pivot — TASMail is a webmail UI
// for whatever IMAP server the user already uses).
// CONSTRAINTS: encrypted_password is base64(nonce + ciphertext) via AES-256-GCM, same
// derivation as smtp_configurations and ai_config (key derived from JWT secret).

use serde::{Deserialize, Serialize};
use sqlx::PgPool;
use uuid::Uuid;

use crate::models::ai_config::{decrypt_api_key, encrypt_api_key};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum ImapEncryption {
    None,
    Ssl,
    #[serde(rename = "starttls")]
    StartTls,
}

impl ImapEncryption {
    pub fn as_str(&self) -> &str {
        match self {
            ImapEncryption::None => "none",
            ImapEncryption::Ssl => "ssl",
            ImapEncryption::StartTls => "starttls",
        }
    }
    pub fn from_str(s: &str) -> Result<Self, String> {
        match s {
            "none" => Ok(ImapEncryption::None),
            "ssl" => Ok(ImapEncryption::Ssl),
            "starttls" => Ok(ImapEncryption::StartTls),
            other => Err(format!("Unknown IMAP encryption: {}", other)),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct ImapConfiguration {
    pub id: Uuid,
    pub user_id: Uuid,
    pub name: String,
    pub host: String,
    pub port: i32,
    pub username: String,
    pub encrypted_password: String,
    pub encryption: String,
    pub sent_folder: Option<String>,
    pub drafts_folder: Option<String>,
    pub trash_folder: Option<String>,
    pub spam_folder: Option<String>,
    pub archive_folder: Option<String>,
    pub is_default: bool,
    pub verified: bool,
    pub last_tested_at: Option<chrono::DateTime<chrono::Utc>>,
    pub last_error: Option<String>,
    pub created_at: Option<chrono::DateTime<chrono::Utc>>,
    pub updated_at: Option<chrono::DateTime<chrono::Utc>>,
}

#[derive(Debug, Deserialize)]
pub struct CreateImapConfigRequest {
    pub name: String,
    pub host: String,
    pub port: i32,
    pub username: String,
    pub password: String,
    pub encryption: ImapEncryption,
    #[serde(default)] pub sent_folder: Option<String>,
    #[serde(default)] pub drafts_folder: Option<String>,
    #[serde(default)] pub trash_folder: Option<String>,
    #[serde(default)] pub spam_folder: Option<String>,
    #[serde(default)] pub archive_folder: Option<String>,
    #[serde(default)] pub is_default: bool,
}

impl ImapConfiguration {
    pub async fn list_for_user(pool: &PgPool, user_id: Uuid) -> Result<Vec<Self>, sqlx::Error> {
        sqlx::query_as::<_, ImapConfiguration>(
            "SELECT * FROM imap_configurations WHERE user_id = $1 ORDER BY is_default DESC, name ASC",
        )
        .bind(user_id)
        .fetch_all(pool)
        .await
    }

    /// PURPOSE: Resolve the default IMAP config for a user — the one TASMail will
    /// use for foreground IMAP operations until per-account selection ships.
    pub async fn default_for_user(pool: &PgPool, user_id: Uuid) -> Result<Option<Self>, sqlx::Error> {
        sqlx::query_as::<_, ImapConfiguration>(
            "SELECT * FROM imap_configurations WHERE user_id = $1 AND is_default = true LIMIT 1",
        )
        .bind(user_id)
        .fetch_optional(pool)
        .await
    }

    pub fn decrypt_password(&self, key: &[u8; 32]) -> Result<String, String> {
        decrypt_api_key(&self.encrypted_password, key)
    }

    pub async fn create(
        pool: &PgPool,
        user_id: Uuid,
        req: &CreateImapConfigRequest,
        key: &[u8; 32],
    ) -> Result<Self, sqlx::Error> {
        let encrypted = encrypt_api_key(&req.password, key)
            .map_err(|e| sqlx::Error::Protocol(format!("encrypt failed: {}", e)))?;

        // If is_default is requested, clear other defaults for this user first
        if req.is_default {
            sqlx::query("UPDATE imap_configurations SET is_default = false WHERE user_id = $1")
                .bind(user_id)
                .execute(pool)
                .await?;
        }

        sqlx::query_as::<_, ImapConfiguration>(
            "INSERT INTO imap_configurations
                (user_id, name, host, port, username, encrypted_password, encryption,
                 sent_folder, drafts_folder, trash_folder, spam_folder, archive_folder, is_default)
             VALUES ($1,$2,$3,$4,$5,$6,$7, $8,$9,$10,$11,$12, $13)
             RETURNING *",
        )
        .bind(user_id)
        .bind(&req.name)
        .bind(&req.host)
        .bind(req.port)
        .bind(&req.username)
        .bind(&encrypted)
        .bind(req.encryption.as_str())
        .bind(&req.sent_folder)
        .bind(&req.drafts_folder)
        .bind(&req.trash_folder)
        .bind(&req.spam_folder)
        .bind(&req.archive_folder)
        .bind(req.is_default)
        .fetch_one(pool)
        .await
    }

    /// Added (TMAIL-380): Partial update of an existing IMAP config row.
    /// Every `Option` argument is COALESCE'd against the existing column so
    /// the BYOK settings form can leave the password field blank and keep
    /// the encrypted_password as-is. Returns the freshly-loaded row on
    /// success, or `None` when no row matches `(id, user_id)` — same shape
    /// as `SmtpConfiguration::update`.
    pub async fn update(
        pool: &PgPool,
        id: Uuid,
        user_id: Uuid,
        host: Option<&str>,
        port: Option<i32>,
        username: Option<&str>,
        encrypted_password: Option<&str>,
        encryption: Option<&str>,
    ) -> Result<Option<Self>, sqlx::Error> {
        sqlx::query_as::<_, ImapConfiguration>(
            "UPDATE imap_configurations SET \
                host = COALESCE($3, host), \
                port = COALESCE($4, port), \
                username = COALESCE($5, username), \
                encrypted_password = COALESCE($6, encrypted_password), \
                encryption = COALESCE($7, encryption), \
                updated_at = NOW() \
             WHERE id = $1 AND user_id = $2 RETURNING *",
        )
        .bind(id)
        .bind(user_id)
        .bind(host)
        .bind(port)
        .bind(username)
        .bind(encrypted_password)
        .bind(encryption)
        .fetch_optional(pool)
        .await
    }

    pub async fn delete(pool: &PgPool, user_id: Uuid, id: Uuid) -> Result<bool, sqlx::Error> {
        let result = sqlx::query("DELETE FROM imap_configurations WHERE id = $1 AND user_id = $2")
            .bind(id)
            .bind(user_id)
            .execute(pool)
            .await?;
        Ok(result.rows_affected() > 0)
    }

    pub async fn mark_tested(
        pool: &PgPool,
        id: Uuid,
        success: bool,
        error: Option<&str>,
    ) -> Result<(), sqlx::Error> {
        sqlx::query(
            "UPDATE imap_configurations
             SET verified = $1, last_tested_at = NOW(), last_error = $2
             WHERE id = $3",
        )
        .bind(success)
        .bind(error)
        .bind(id)
        .execute(pool)
        .await?;
        Ok(())
    }
}

/// PURPOSE: Public-facing summary that scrubs the encrypted password so it never
/// leaks into JSON responses (mirrors SmtpConfigSummary pattern).
#[derive(Debug, Serialize)]
pub struct ImapConfigSummary {
    pub id: Uuid,
    pub name: String,
    pub host: String,
    pub port: i32,
    pub username: String,
    pub encryption: String,
    pub sent_folder: Option<String>,
    pub drafts_folder: Option<String>,
    pub trash_folder: Option<String>,
    pub spam_folder: Option<String>,
    pub archive_folder: Option<String>,
    pub is_default: bool,
    pub verified: bool,
    pub last_tested_at: Option<chrono::DateTime<chrono::Utc>>,
    pub last_error: Option<String>,
}

impl From<ImapConfiguration> for ImapConfigSummary {
    fn from(c: ImapConfiguration) -> Self {
        Self {
            id: c.id,
            name: c.name,
            host: c.host,
            port: c.port,
            username: c.username,
            encryption: c.encryption,
            sent_folder: c.sent_folder,
            drafts_folder: c.drafts_folder,
            trash_folder: c.trash_folder,
            spam_folder: c.spam_folder,
            archive_folder: c.archive_folder,
            is_default: c.is_default,
            verified: c.verified,
            last_tested_at: c.last_tested_at,
            last_error: c.last_error,
        }
    }
}

/// PURPOSE: Curated provider presets for the onboarding wizard.
/// Mirrors what Thunderbird and macOS Mail's auto-discovery yield for popular providers.
pub fn provider_presets() -> Vec<serde_json::Value> {
    serde_json::from_str(r#"[
        {"name":"Gmail","domain":"gmail.com","imap":{"host":"imap.gmail.com","port":993,"encryption":"ssl"},"smtp":{"host":"smtp.gmail.com","port":587,"encryption":"starttls"},"hint":"Use a Google App Password, not your account password."},
        {"name":"Google Workspace","domain":"googlemail.com","imap":{"host":"imap.gmail.com","port":993,"encryption":"ssl"},"smtp":{"host":"smtp.gmail.com","port":587,"encryption":"starttls"},"hint":"Use an App Password (Workspace admin must enable IMAP)."},
        {"name":"Outlook / Hotmail","domain":"outlook.com","imap":{"host":"outlook.office365.com","port":993,"encryption":"ssl"},"smtp":{"host":"smtp.office365.com","port":587,"encryption":"starttls"},"hint":"Personal accounts may require enabling IMAP in Outlook.com settings."},
        {"name":"Office 365","domain":"office365.com","imap":{"host":"outlook.office365.com","port":993,"encryption":"ssl"},"smtp":{"host":"smtp.office365.com","port":587,"encryption":"starttls"},"hint":"Modern auth (OAuth) may be required by your tenant."},
        {"name":"Yahoo Mail","domain":"yahoo.com","imap":{"host":"imap.mail.yahoo.com","port":993,"encryption":"ssl"},"smtp":{"host":"smtp.mail.yahoo.com","port":465,"encryption":"ssl"},"hint":"Generate an App Password under Account Security."},
        {"name":"Zoho Mail","domain":"zoho.com","imap":{"host":"imap.zoho.com","port":993,"encryption":"ssl"},"smtp":{"host":"smtp.zoho.com","port":465,"encryption":"ssl"},"hint":"Use an App Password if 2FA is enabled."},
        {"name":"FastMail","domain":"fastmail.com","imap":{"host":"imap.fastmail.com","port":993,"encryption":"ssl"},"smtp":{"host":"smtp.fastmail.com","port":465,"encryption":"ssl"},"hint":"Generate an App Password under Settings → Privacy & Security."},
        {"name":"iCloud Mail","domain":"icloud.com","imap":{"host":"imap.mail.me.com","port":993,"encryption":"ssl"},"smtp":{"host":"smtp.mail.me.com","port":587,"encryption":"starttls"},"hint":"Use an App-Specific Password generated at appleid.apple.com."},
        {"name":"ProtonMail Bridge","domain":"protonmail.com","imap":{"host":"127.0.0.1","port":1143,"encryption":"starttls"},"smtp":{"host":"127.0.0.1","port":1025,"encryption":"starttls"},"hint":"Requires the locally-running ProtonMail Bridge app."},
        {"name":"Mail.ru","domain":"mail.ru","imap":{"host":"imap.mail.ru","port":993,"encryption":"ssl"},"smtp":{"host":"smtp.mail.ru","port":465,"encryption":"ssl"}},
        {"name":"GMX","domain":"gmx.com","imap":{"host":"imap.gmx.com","port":993,"encryption":"ssl"},"smtp":{"host":"mail.gmx.com","port":465,"encryption":"ssl"}}
    ]"#).unwrap_or_default()
}
