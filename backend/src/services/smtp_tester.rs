// Added: SMTP connection tester service for BYO-SMTP (TMAIL-48)
// PURPOSE: Tests external SMTP server connectivity, authentication, and optionally sends test email
// EXTERNAL: Uses lettre crate for SMTP transport construction and connection testing

use lettre::{
    transport::smtp::authentication::Credentials,
    AsyncSmtpTransport, AsyncTransport, Message, Tokio1Executor,
};
use std::time::Instant;

use crate::models::smtp_config::SmtpConfiguration;

/// PURPOSE: Result of an SMTP connection test
#[derive(Debug, serde::Serialize)]
pub struct SmtpTestResult {
    pub success: bool,
    pub message: String,
    pub latency_ms: u64,
}

/// PURPOSE: Build an async lettre SMTP transport from a user's SMTP configuration
/// CONSTRAINTS: Supports none, ssl, and starttls encryption modes
pub fn build_transport(
    config: &SmtpConfiguration,
    password: &str,
) -> Result<AsyncSmtpTransport<Tokio1Executor>, String> {
    let creds = Credentials::new(config.username.clone(), password.to_string());

    let transport = match config.encryption.as_str() {
        "ssl" => AsyncSmtpTransport::<Tokio1Executor>::relay(&config.host)
            .map_err(|e| format!("Failed to create SSL transport: {}", e))?
            .port(config.port as u16)
            .credentials(creds)
            .build(),
        "starttls" => AsyncSmtpTransport::<Tokio1Executor>::starttls_relay(&config.host)
            .map_err(|e| format!("Failed to create STARTTLS transport: {}", e))?
            .port(config.port as u16)
            .credentials(creds)
            .build(),
        "none" => AsyncSmtpTransport::<Tokio1Executor>::builder_dangerous(&config.host)
            .port(config.port as u16)
            .credentials(creds)
            .build(),
        other => {
            return Err(format!("Unknown encryption type: {}", other));
        }
    };

    Ok(transport)
}

/// PURPOSE: Test SMTP connection by authenticating and sending a test email to self
/// NOTE: Uses the config's from_address or username as both sender and recipient
pub async fn test_smtp_connection(
    config: &SmtpConfiguration,
    decrypted_password: &str,
) -> SmtpTestResult {
    let start = Instant::now();

    let transport = match build_transport(config, decrypted_password) {
        Ok(t) => t,
        Err(e) => {
            return SmtpTestResult {
                success: false,
                message: format!("Failed to build SMTP transport: {}", e),
                latency_ms: start.elapsed().as_millis() as u64,
            };
        }
    };

    // Added: Determine the from/to address for the test email
    let test_address = config
        .from_address
        .as_deref()
        .unwrap_or(&config.username);

    // Added: Build a simple test message
    let email = match Message::builder()
        .from(
            test_address
                .parse()
                .unwrap_or_else(|_| format!("<{}>", test_address).parse().unwrap_or_else(|_| {
                    "test@localhost".parse().expect("fallback address should parse")
                })),
        )
        .to(
            test_address
                .parse()
                .unwrap_or_else(|_| format!("<{}>", test_address).parse().unwrap_or_else(|_| {
                    "test@localhost".parse().expect("fallback address should parse")
                })),
        )
        .subject("TASMail SMTP Test")
        .body("This is a test email from TASMail to verify your SMTP configuration.".to_string())
    {
        Ok(msg) => msg,
        Err(e) => {
            return SmtpTestResult {
                success: false,
                message: format!("Failed to build test email: {}", e),
                latency_ms: start.elapsed().as_millis() as u64,
            };
        }
    };

    // Added: Attempt to send the test email
    match transport.send(email).await {
        Ok(_) => SmtpTestResult {
            success: true,
            message: format!(
                "SMTP connection successful. Test email sent to {}",
                test_address
            ),
            latency_ms: start.elapsed().as_millis() as u64,
        },
        Err(e) => SmtpTestResult {
            success: false,
            message: format!("SMTP test failed: {}", e),
            latency_ms: start.elapsed().as_millis() as u64,
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use uuid::Uuid;

    /// PURPOSE: Helper to create a test SmtpConfiguration with given encryption type
    fn make_test_config(encryption: &str) -> SmtpConfiguration {
        SmtpConfiguration {
            id: Uuid::new_v4(),
            user_id: Uuid::new_v4(),
            name: "Test Config".to_string(),
            host: "smtp.example.com".to_string(),
            port: 587,
            username: "user@example.com".to_string(),
            encrypted_password: "not-used-in-transport".to_string(),
            encryption: encryption.to_string(),
            from_address: Some("sender@example.com".to_string()),
            is_default: false,
            verified: false,
            last_tested_at: None,
            created_at: Some(chrono::Utc::now()),
            updated_at: Some(chrono::Utc::now()),
        }
    }

    #[test]
    fn test_build_transport_starttls() {
        let config = make_test_config("starttls");
        let result = build_transport(&config, "test-password");
        // NOTE: Transport creation should succeed (no actual connection yet)
        assert!(result.is_ok());
    }

    #[test]
    fn test_build_transport_ssl() {
        let config = make_test_config("ssl");
        let result = build_transport(&config, "test-password");
        assert!(result.is_ok());
    }

    #[test]
    fn test_build_transport_none() {
        let config = make_test_config("none");
        let result = build_transport(&config, "test-password");
        assert!(result.is_ok());
    }

    #[test]
    fn test_build_transport_unknown_encryption() {
        let config = make_test_config("unknown");
        let result = build_transport(&config, "test-password");
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("Unknown encryption type"));
    }

    #[test]
    fn test_smtp_test_result_serialization() {
        let result = SmtpTestResult {
            success: true,
            message: "Connection successful".to_string(),
            latency_ms: 123,
        };
        let json = serde_json::to_value(&result).unwrap();
        assert_eq!(json["success"], true);
        assert_eq!(json["message"], "Connection successful");
        assert_eq!(json["latency_ms"], 123);
    }

    #[test]
    fn test_smtp_test_result_failure_serialization() {
        let result = SmtpTestResult {
            success: false,
            message: "Authentication failed".to_string(),
            latency_ms: 500,
        };
        let json = serde_json::to_value(&result).unwrap();
        assert_eq!(json["success"], false);
        assert_eq!(json["message"], "Authentication failed");
    }

    #[test]
    fn test_build_transport_with_custom_port() {
        let mut config = make_test_config("starttls");
        config.port = 2525;
        let result = build_transport(&config, "password");
        assert!(result.is_ok());
    }

    #[test]
    fn test_build_transport_with_ssl_port() {
        let mut config = make_test_config("ssl");
        config.port = 465;
        let result = build_transport(&config, "password");
        assert!(result.is_ok());
    }
}
