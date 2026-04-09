use serde::Deserialize;
use std::path::Path;

#[derive(Debug, Deserialize, Clone)]
pub struct Config {
    pub server: ServerConfig,
    pub database: DatabaseConfig,
    pub imap: ImapConfig,
    pub smtp: SmtpConfig,
    pub jwt: JwtConfig,
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
                secret: std::env::var("JWT_SECRET")
                    .unwrap_or_else(|_| "dev-secret-change-in-production".to_string()),
                access_token_expiry_secs: std::env::var("JWT_ACCESS_EXPIRY")
                    .ok()
                    .and_then(|p| p.parse().ok())
                    .unwrap_or(900), // 15 minutes
                refresh_token_expiry_secs: std::env::var("JWT_REFRESH_EXPIRY")
                    .ok()
                    .and_then(|p| p.parse().ok())
                    .unwrap_or(604800), // 7 days
            },
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_config_from_env_loads() {
        // Verify config loads without error (actual values depend on env)
        let config = Config::from_env().unwrap();
        assert!(!config.server.host.is_empty());
        assert!(config.server.port > 0);
        assert!(config.database.max_connections > 0);
        assert!(config.jwt.access_token_expiry_secs > 0);
    }
}
