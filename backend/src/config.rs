use serde::Deserialize;
use std::path::Path;

#[derive(Debug, Deserialize, Clone)]
pub struct Config {
    pub server: ServerConfig,
    pub database: DatabaseConfig,
    pub imap: ImapConfig,
    pub smtp: SmtpConfig,
    pub jwt: JwtConfig,
    // Added: Attachment storage configuration for TMAIL-59
    #[serde(default)]
    pub storage: StorageConfig,
    // Added: Optional metrics bearer token for TMAIL-41
    #[serde(default)]
    pub metrics_token: Option<String>,
    // Added: Optional Rspamd URL and password for spam filter integration (TMAIL-15)
    #[serde(default)]
    pub rspamd_url: Option<String>,
    #[serde(default)]
    pub rspamd_password: Option<String>,
    // Added: Optional billing/payment configuration for Paystack and MoMo (TMAIL-46)
    #[serde(default)]
    pub billing: Option<BillingConfig>,
}

/// Added: Billing configuration for Paystack and MTN MoMo payment providers (TMAIL-46)
/// PURPOSE: Stores API keys for payment gateway integration; all fields optional to allow partial config
#[derive(Debug, Deserialize, Clone)]
pub struct BillingConfig {
    pub paystack_secret_key: Option<String>,
    pub paystack_public_key: Option<String>,
    pub momo_api_key: Option<String>,
    pub momo_api_user: Option<String>,
}

#[derive(Debug, Deserialize, Clone)]
pub struct ServerConfig {
    pub host: String,
    pub port: u16,
}

#[derive(Debug, Deserialize, Clone)]
pub struct DatabaseConfig {
    pub url: String,
    pub max_connections: u32,
}

#[derive(Debug, Deserialize, Clone)]
pub struct ImapConfig {
    pub host: String,
    pub port: u16,
    pub tls: bool,
    #[serde(default)]
    pub master_password: Option<String>,
}

#[derive(Debug, Deserialize, Clone)]
pub struct SmtpConfig {
    pub host: String,
    pub port: u16,
    pub tls: bool,
}

#[derive(Debug, Deserialize, Clone)]
pub struct JwtConfig {
    pub secret: String,
    pub access_token_expiry_secs: u64,
    pub refresh_token_expiry_secs: u64,
}

/// Added: Attachment storage and ClamAV scanning configuration for TMAIL-59
/// PURPOSE: Controls where attachments are stored on disk, max upload size, and ClamAV socket path
#[derive(Debug, Deserialize, Clone)]
pub struct StorageConfig {
    #[serde(default = "default_attachment_dir")]
    pub attachment_dir: String,
    #[serde(default = "default_max_file_size")]
    pub max_file_size: u64,
    #[serde(default)]
    pub clamav_socket: Option<String>,
}

fn default_attachment_dir() -> String {
    "./data/attachments".to_string()
}

fn default_max_file_size() -> u64 {
    25 * 1024 * 1024 // 25 MB
}

impl Default for StorageConfig {
    fn default() -> Self {
        Self {
            attachment_dir: default_attachment_dir(),
            max_file_size: default_max_file_size(),
            clamav_socket: None,
        }
    }
}

impl Config {
    pub fn load(path: &Path) -> anyhow::Result<Self> {
        let content = std::fs::read_to_string(path)?;
        let config: Config = toml::from_str(&content)?;
        Ok(config)
    }

    /// Load config from environment variables with defaults for development
    pub fn from_env() -> anyhow::Result<Self> {
        Ok(Config {
            server: ServerConfig {
                host: std::env::var("TASMAIL_HOST").unwrap_or_else(|_| "127.0.0.1".to_string()),
                port: std::env::var("TASMAIL_PORT")
                    .ok()
                    .and_then(|p| p.parse().ok())
                    .unwrap_or(3000),
            },
            database: DatabaseConfig {
                url: std::env::var("DATABASE_URL")
                    .unwrap_or_else(|_| "postgres://tasmail:tasmail@localhost/tasmail".to_string()),
                max_connections: std::env::var("DATABASE_MAX_CONNECTIONS")
                    .ok()
                    .and_then(|p| p.parse().ok())
                    .unwrap_or(10),
            },
            imap: ImapConfig {
                host: std::env::var("IMAP_HOST").unwrap_or_else(|_| "127.0.0.1".to_string()),
                port: std::env::var("IMAP_PORT")
                    .ok()
                    .and_then(|p| p.parse().ok())
                    .unwrap_or(993),
                tls: std::env::var("IMAP_TLS")
                    .map(|v| v != "false")
                    .unwrap_or(true),
                master_password: std::env::var("IMAP_MASTER_PASSWORD").ok(),
            },
            smtp: SmtpConfig {
                host: std::env::var("SMTP_HOST").unwrap_or_else(|_| "127.0.0.1".to_string()),
                port: std::env::var("SMTP_PORT")
                    .ok()
                    .and_then(|p| p.parse().ok())
                    .unwrap_or(587),
                tls: std::env::var("SMTP_TLS")
                    .map(|v| v != "false")
                    .unwrap_or(true),
            },
            jwt: JwtConfig {
                // Changed: Log warning when using default dev secret (TMAIL-37)
                secret: std::env::var("JWT_SECRET").unwrap_or_else(|_| {
                    eprintln!("WARNING: JWT_SECRET not set — using insecure default. Set JWT_SECRET env var in production!");
                    "dev-secret-change-in-production".to_string()
                }),
                access_token_expiry_secs: std::env::var("JWT_ACCESS_EXPIRY")
                    .ok()
                    .and_then(|p| p.parse().ok())
                    .unwrap_or(900), // 15 minutes
                refresh_token_expiry_secs: std::env::var("JWT_REFRESH_EXPIRY")
                    .ok()
                    .and_then(|p| p.parse().ok())
                    .unwrap_or(604800), // 7 days
            },
            // Added: Metrics token from env var for TMAIL-41
            metrics_token: std::env::var("METRICS_TOKEN").ok(),
            // Added: Rspamd config from env vars for TMAIL-15
            rspamd_url: std::env::var("RSPAMD_URL").ok(),
            rspamd_password: std::env::var("RSPAMD_PASSWORD").ok(),
            // Added: Storage config from env vars for TMAIL-59
            storage: StorageConfig {
                attachment_dir: std::env::var("ATTACHMENT_DIR")
                    .unwrap_or_else(|_| default_attachment_dir()),
                max_file_size: std::env::var("MAX_FILE_SIZE")
                    .ok()
                    .and_then(|p| p.parse().ok())
                    .unwrap_or_else(default_max_file_size),
                clamav_socket: std::env::var("CLAMAV_SOCKET").ok(),
            },
            // Added: Billing config from env vars for TMAIL-46
            billing: {
                let paystack_secret = std::env::var("PAYSTACK_SECRET_KEY").ok();
                let paystack_public = std::env::var("PAYSTACK_PUBLIC_KEY").ok();
                let momo_key = std::env::var("MOMO_API_KEY").ok();
                let momo_user = std::env::var("MOMO_API_USER").ok();
                if paystack_secret.is_some() || momo_key.is_some() {
                    Some(BillingConfig {
                        paystack_secret_key: paystack_secret,
                        paystack_public_key: paystack_public,
                        momo_api_key: momo_key,
                        momo_api_user: momo_user,
                    })
                } else {
                    None
                }
            },
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    #[test]
    fn test_config_from_env_loads() {
        let config = Config::from_env().unwrap();
        assert!(!config.server.host.is_empty());
        assert!(config.server.port > 0);
        assert!(config.database.max_connections > 0);
        assert!(config.jwt.access_token_expiry_secs > 0);
    }

    #[test]
    fn test_config_from_env_has_reasonable_values() {
        // NOTE: env vars may override defaults, so test for reasonable ranges
        let config = Config::from_env().unwrap();
        assert!(!config.server.host.is_empty());
        assert!(config.server.port > 0);
        assert!(config.database.max_connections > 0);
        assert!(config.imap.port > 0);
        assert!(config.smtp.port > 0);
        assert!(config.jwt.access_token_expiry_secs > 0);
        assert!(config.jwt.refresh_token_expiry_secs > config.jwt.access_token_expiry_secs);
    }

    #[test]
    fn test_config_from_toml() {
        let toml_content = r#"
[server]
host = "0.0.0.0"
port = 8080

[database]
url = "postgres://test:test@localhost/testdb"
max_connections = 5

[imap]
host = "imap.example.com"
port = 993
tls = true

[smtp]
host = "smtp.example.com"
port = 465
tls = true

[jwt]
secret = "test-secret-key"
access_token_expiry_secs = 300
refresh_token_expiry_secs = 86400
"#;
        // Write to a temp file
        let mut tmp = tempfile::NamedTempFile::new().unwrap();
        tmp.write_all(toml_content.as_bytes()).unwrap();
        tmp.flush().unwrap();

        let config = Config::load(tmp.path()).unwrap();
        assert_eq!(config.server.host, "0.0.0.0");
        assert_eq!(config.server.port, 8080);
        assert_eq!(config.database.url, "postgres://test:test@localhost/testdb");
        assert_eq!(config.database.max_connections, 5);
        assert_eq!(config.imap.host, "imap.example.com");
        assert_eq!(config.smtp.host, "smtp.example.com");
        assert_eq!(config.smtp.port, 465);
        assert_eq!(config.jwt.secret, "test-secret-key");
        assert_eq!(config.jwt.access_token_expiry_secs, 300);
        assert_eq!(config.jwt.refresh_token_expiry_secs, 86400);
        // Added: Storage config defaults when [storage] section omitted from TOML
        assert_eq!(config.storage.attachment_dir, "./data/attachments");
        assert_eq!(config.storage.max_file_size, 25 * 1024 * 1024);
        assert!(config.storage.clamav_socket.is_none());
    }

    #[test]
    fn test_config_invalid_toml() {
        let mut tmp = tempfile::NamedTempFile::new().unwrap();
        tmp.write_all(b"not valid toml {{{").unwrap();
        tmp.flush().unwrap();

        let result = Config::load(tmp.path());
        assert!(result.is_err());
    }

    #[test]
    fn test_config_storage_defaults() {
        // Added: Verify storage config defaults from env
        let config = Config::from_env().unwrap();
        assert_eq!(config.storage.attachment_dir, "./data/attachments");
        assert_eq!(config.storage.max_file_size, 25 * 1024 * 1024);
    }

    #[test]
    fn test_config_imap_master_password_optional() {
        let config = Config::from_env().unwrap();
        // Master password defaults to None from env
        assert!(config.imap.master_password.is_none());
    }
}
