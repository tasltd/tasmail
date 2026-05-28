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
    // Added: Optional push notification configuration for FCM/APNs/Web Push (TMAIL-50)
    #[serde(default)]
    pub push: Option<PushConfig>,
    // Added: Redis cache configuration for session/branding/rate-limit caching
    #[serde(default)]
    pub redis: RedisConfig,
    // Added (TMAIL-273): Per-account brute-force lockout configuration. All
    // fields default so a deployment that doesn't set the env vars still
    // gets sane production behaviour (5 attempts / 15 min / 15 min lockout).
    #[serde(default)]
    pub lockout: LockoutConfig,
}

/// Added (TMAIL-273): Per-account brute-force lockout policy.
///
/// PURPOSE: Limits how many failed login attempts a single account can
/// accumulate before the auth service refuses to check passwords against
/// it for a cooldown period. Layered on top of the per-IP rate limit so
/// distributed attackers rotating IPs can't grind a single account.
///
/// All values are configurable via env vars / config.toml so operators
/// can tighten or relax the policy without a rebuild.
#[derive(Debug, Deserialize, Clone)]
pub struct LockoutConfig {
    /// Failed attempts within `window_secs` that triggers a lockout.
    /// Default: 5
    #[serde(default = "default_lockout_threshold")]
    pub threshold: i32,
    /// Rolling window for counting failed attempts. An attempt older
    /// than this resets the counter back to 1. Default: 900 (15 min).
    #[serde(default = "default_lockout_window_secs")]
    pub window_secs: i64,
    /// How long an account stays locked once the threshold is hit.
    /// Default: 900 (15 min).
    #[serde(default = "default_lockout_duration_secs")]
    pub duration_secs: i64,
}

fn default_lockout_threshold() -> i32 { 5 }
fn default_lockout_window_secs() -> i64 { 900 }
fn default_lockout_duration_secs() -> i64 { 900 }

impl Default for LockoutConfig {
    fn default() -> Self {
        Self {
            threshold: default_lockout_threshold(),
            window_secs: default_lockout_window_secs(),
            duration_secs: default_lockout_duration_secs(),
        }
    }
}

/// Added: Redis cache configuration
/// PURPOSE: Controls Redis connection URL and default TTLs for cached data categories
#[derive(Debug, Deserialize, Clone)]
pub struct RedisConfig {
    #[serde(default = "default_redis_url")]
    pub url: String,
    /// TTL in seconds for branding cache (default: 300 = 5 min)
    #[serde(default = "default_branding_ttl")]
    pub branding_ttl_secs: u64,
    /// TTL in seconds for quota cache (default: 60 = 1 min)
    #[serde(default = "default_quota_ttl")]
    pub quota_ttl_secs: u64,
    /// TTL in seconds for user session metadata cache (default: 900 = 15 min)
    #[serde(default = "default_session_ttl")]
    pub session_ttl_secs: u64,
    /// Rate limit window in seconds (default: 60)
    #[serde(default = "default_rate_limit_window")]
    pub rate_limit_window_secs: u64,
    /// Rate limit max requests per window (default: 100)
    #[serde(default = "default_rate_limit_max")]
    pub rate_limit_max_requests: u64,
}

fn default_redis_url() -> String {
    "redis://127.0.0.1:6379".to_string()
}
fn default_branding_ttl() -> u64 { 300 }
fn default_quota_ttl() -> u64 { 60 }
fn default_session_ttl() -> u64 { 900 }
fn default_rate_limit_window() -> u64 { 60 }
fn default_rate_limit_max() -> u64 { 100 }

impl Default for RedisConfig {
    fn default() -> Self {
        Self {
            url: default_redis_url(),
            branding_ttl_secs: default_branding_ttl(),
            quota_ttl_secs: default_quota_ttl(),
            session_ttl_secs: default_session_ttl(),
            rate_limit_window_secs: default_rate_limit_window(),
            rate_limit_max_requests: default_rate_limit_max(),
        }
    }
}

/// Added: Push notification configuration for FCM, APNs, and Web Push providers (TMAIL-50)
/// PURPOSE: Stores credentials and project IDs for push notification delivery
#[derive(Debug, Deserialize, Clone)]
pub struct PushConfig {
    pub fcm_project_id: Option<String>,
    pub fcm_service_account_key: Option<String>,
    pub apns_key_id: Option<String>,
    pub apns_team_id: Option<String>,
    pub apns_key_path: Option<String>,
}

/// Changed: Billing configuration now mirrors PayPro's 4 supported providers.
/// MTN MoMo removed — TASMail uses only the providers PayPro is already configured for:
/// Paystack, Mastercard MPGS, Cybersource invoicing, and manual Bank Transfer instructions.
/// All fields optional so partially-configured deployments keep working.
#[derive(Debug, Deserialize, Clone, Default)]
pub struct BillingConfig {
    // --- Paystack ---
    pub paystack_secret_key: Option<String>,
    pub paystack_public_key: Option<String>,
    #[serde(default)]
    pub paystack_base_url: Option<String>,

    // --- Mastercard Payment Gateway Services (MPGS) ---
    pub mastercard_merchant_id: Option<String>,
    pub mastercard_api_password: Option<String>,
    #[serde(default)]
    pub mastercard_base_url: Option<String>,
    #[serde(default)]
    pub mastercard_currency: Option<String>,
    #[serde(default)]
    pub mastercard_webhook_secret: Option<String>,

    // --- Cybersource Invoicing ---
    pub cybersource_merchant_id: Option<String>,
    pub cybersource_key_id: Option<String>,
    pub cybersource_shared_secret: Option<String>,
    #[serde(default)]
    pub cybersource_base_url: Option<String>,

    // --- Bank Transfer (manual) ---
    pub bank_name: Option<String>,
    pub bank_account_name: Option<String>,
    pub bank_account_number: Option<String>,
    #[serde(default)]
    pub bank_branch: Option<String>,
    #[serde(default)]
    pub bank_swift_code: Option<String>,
    #[serde(default)]
    pub bank_reference_prefix: Option<String>,
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
    // Added: System "noreply" sender for billing receipts, password resets, OTP, signup confirmations etc.
    // Per-user outgoing mail still authenticates as the user — these credentials are used only for
    // notifications originating from TASMail itself (not from a user mailbox).
    #[serde(default)]
    pub notification_from: Option<String>,
    #[serde(default)]
    pub notification_username: Option<String>,
    #[serde(default)]
    pub notification_password: Option<String>,
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
                // Added: System notification sender — defaults to noreply@techatscale.io per the
                // TASMail product convention. Override via SMTP_NOTIFICATION_FROM env var if needed.
                notification_from: Some(
                    std::env::var("SMTP_NOTIFICATION_FROM")
                        .unwrap_or_else(|_| "noreply@techatscale.io".to_string()),
                ),
                notification_username: std::env::var("SMTP_NOTIFICATION_USERNAME").ok(),
                notification_password: std::env::var("SMTP_NOTIFICATION_PASSWORD").ok(),
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
            // Changed: Billing config now spans the four PayPro providers (TMAIL-46).
            // MoMo dropped — TASMail mirrors PayPro, which uses Paystack / Mastercard / Cybersource / Bank Transfer.
            // Credentials are sourced from env vars at startup; production deployments mirror PayPro's
            // PaymentProviderConfig table by passing the same values through env.
            billing: {
                let cfg = BillingConfig {
                    paystack_secret_key: std::env::var("PAYSTACK_SECRET_KEY").ok(),
                    paystack_public_key: std::env::var("PAYSTACK_PUBLIC_KEY").ok(),
                    paystack_base_url: std::env::var("PAYSTACK_BASE_URL").ok(),

                    mastercard_merchant_id: std::env::var("MASTERCARD_MERCHANT_ID").ok(),
                    mastercard_api_password: std::env::var("MASTERCARD_API_PASSWORD").ok(),
                    mastercard_base_url: std::env::var("MASTERCARD_GATEWAY_URL").ok(),
                    mastercard_currency: std::env::var("MASTERCARD_CURRENCY").ok(),
                    mastercard_webhook_secret: std::env::var("MASTERCARD_WEBHOOK_SECRET").ok(),

                    cybersource_merchant_id: std::env::var("CYBERSOURCE_MERCHANT_ID").ok(),
                    cybersource_key_id: std::env::var("CYBERSOURCE_KEY_ID").ok(),
                    cybersource_shared_secret: std::env::var("CYBERSOURCE_SECRET_KEY").ok(),
                    cybersource_base_url: std::env::var("CYBERSOURCE_BASE_URL").ok(),

                    bank_name: std::env::var("BANK_NAME").ok(),
                    bank_account_name: std::env::var("BANK_ACCOUNT_NAME").ok(),
                    bank_account_number: std::env::var("BANK_ACCOUNT_NUMBER").ok(),
                    bank_branch: std::env::var("BANK_BRANCH").ok(),
                    bank_swift_code: std::env::var("BANK_SWIFT_CODE").ok(),
                    bank_reference_prefix: std::env::var("BANK_REFERENCE_PREFIX").ok(),
                };
                // Only attach billing if at least one provider has credentials configured.
                let has_any = cfg.paystack_secret_key.is_some()
                    || cfg.mastercard_merchant_id.is_some()
                    || cfg.cybersource_merchant_id.is_some()
                    || cfg.bank_account_number.is_some();
                if has_any { Some(cfg) } else { None }
            },
            // Added (TMAIL-273): Lockout policy from env vars. Falls back to
            // the defaults (5 / 15 min / 15 min) when unset.
            lockout: LockoutConfig {
                threshold: std::env::var("LOCKOUT_THRESHOLD")
                    .ok().and_then(|p| p.parse().ok())
                    .unwrap_or_else(default_lockout_threshold),
                window_secs: std::env::var("LOCKOUT_WINDOW_SECS")
                    .ok().and_then(|p| p.parse().ok())
                    .unwrap_or_else(default_lockout_window_secs),
                duration_secs: std::env::var("LOCKOUT_DURATION_SECS")
                    .ok().and_then(|p| p.parse().ok())
                    .unwrap_or_else(default_lockout_duration_secs),
            },
            // Added: Redis config from env vars
            redis: RedisConfig {
                url: std::env::var("REDIS_URL")
                    .unwrap_or_else(|_| default_redis_url()),
                branding_ttl_secs: std::env::var("REDIS_BRANDING_TTL")
                    .ok().and_then(|p| p.parse().ok())
                    .unwrap_or_else(default_branding_ttl),
                quota_ttl_secs: std::env::var("REDIS_QUOTA_TTL")
                    .ok().and_then(|p| p.parse().ok())
                    .unwrap_or_else(default_quota_ttl),
                session_ttl_secs: std::env::var("REDIS_SESSION_TTL")
                    .ok().and_then(|p| p.parse().ok())
                    .unwrap_or_else(default_session_ttl),
                rate_limit_window_secs: std::env::var("REDIS_RATE_LIMIT_WINDOW")
                    .ok().and_then(|p| p.parse().ok())
                    .unwrap_or_else(default_rate_limit_window),
                rate_limit_max_requests: std::env::var("REDIS_RATE_LIMIT_MAX")
                    .ok().and_then(|p| p.parse().ok())
                    .unwrap_or_else(default_rate_limit_max),
            },
            // Added: Push notification config from env vars for TMAIL-50
            push: {
                let fcm_project = std::env::var("FCM_PROJECT_ID").ok();
                let fcm_key = std::env::var("FCM_SERVICE_ACCOUNT_KEY").ok();
                let apns_key_id = std::env::var("APNS_KEY_ID").ok();
                let apns_team_id = std::env::var("APNS_TEAM_ID").ok();
                let apns_key_path = std::env::var("APNS_KEY_PATH").ok();
                if fcm_project.is_some() || apns_key_id.is_some() {
                    Some(PushConfig {
                        fcm_project_id: fcm_project,
                        fcm_service_account_key: fcm_key,
                        apns_key_id,
                        apns_team_id,
                        apns_key_path,
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
