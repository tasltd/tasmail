use lettre::{
    message::{header::ContentType, Mailbox as LettreMailbox, MultiPart, SinglePart},
    transport::smtp::authentication::Credentials,
    AsyncSmtpTransport, AsyncTransport, Message, Tokio1Executor,
};

use crate::config::SmtpConfig;
use crate::error::AppError;

/// Request to send an email
#[derive(Debug, serde::Deserialize)]
pub struct SendRequest {
    pub to: Vec<String>,
    pub cc: Option<Vec<String>>,
    pub bcc: Option<Vec<String>>,
    pub subject: String,
    pub text_body: Option<String>,
    pub html_body: Option<String>,
}

/// SMTP service for sending emails via Postfix
pub struct SmtpService {
    config: SmtpConfig,
}

impl SmtpService {
    pub fn new(config: SmtpConfig) -> Self {
        Self { config }
    }

    /// Send an email message
    pub async fn send(
        &self,
        from_address: &str,
        from_password: &str,
        request: &SendRequest,
    ) -> Result<(), AppError> {
        let from: LettreMailbox = from_address
            .parse()
            .map_err(|e| AppError::BadRequest(format!("Invalid from address: {}", e)))?;

        let mut builder = Message::builder().from(from);

        for to in &request.to {
            let to_mailbox: LettreMailbox = to
                .parse()
                .map_err(|e| AppError::BadRequest(format!("Invalid to address '{}': {}", to, e)))?;
            builder = builder.to(to_mailbox);
        }

        if let Some(cc_list) = &request.cc {
            for cc in cc_list {
                let cc_mailbox: LettreMailbox = cc.parse().map_err(|e| {
                    AppError::BadRequest(format!("Invalid cc address '{}': {}", cc, e))
                })?;
                builder = builder.cc(cc_mailbox);
            }
        }

        if let Some(bcc_list) = &request.bcc {
            for bcc in bcc_list {
                let bcc_mailbox: LettreMailbox = bcc.parse().map_err(|e| {
                    AppError::BadRequest(format!("Invalid bcc address '{}': {}", bcc, e))
                })?;
                builder = builder.bcc(bcc_mailbox);
            }
        }

        builder = builder.subject(&request.subject);

        let email = match (&request.text_body, &request.html_body) {
            (Some(text), Some(html)) => builder
                .multipart(
                    MultiPart::alternative()
                        .singlepart(
                            SinglePart::builder()
                                .header(ContentType::TEXT_PLAIN)
                                .body(text.clone()),
                        )
                        .singlepart(
                            SinglePart::builder()
                                .header(ContentType::TEXT_HTML)
                                .body(html.clone()),
                        ),
                )
                .map_err(|e| AppError::Smtp(format!("Failed to build email: {}", e)))?,
            (Some(text), None) => builder
                .header(ContentType::TEXT_PLAIN)
                .body(text.clone())
                .map_err(|e| AppError::Smtp(format!("Failed to build email: {}", e)))?,
            (None, Some(html)) => builder
                .header(ContentType::TEXT_HTML)
                .body(html.clone())
                .map_err(|e| AppError::Smtp(format!("Failed to build email: {}", e)))?,
            (None, None) => {
                return Err(AppError::BadRequest(
                    "Email must have a text or HTML body".to_string(),
                ));
            }
        };

        let creds = Credentials::new(from_address.to_string(), from_password.to_string());

        let transport = if self.config.tls {
            AsyncSmtpTransport::<Tokio1Executor>::starttls_relay(&self.config.host)
                .map_err(|e| AppError::Smtp(format!("SMTP transport error: {}", e)))?
                .port(self.config.port)
                .credentials(creds)
                .build()
        } else {
            AsyncSmtpTransport::<Tokio1Executor>::builder_dangerous(&self.config.host)
                .port(self.config.port)
                .credentials(creds)
                .build()
        };

        transport
            .send(email)
            .await
            .map_err(|e| AppError::Smtp(format!("Failed to send email: {}", e)))?;

        Ok(())
    }

    /// PURPOSE: Send a METHOD:REPLY iMIP message back to a meeting organizer.
    /// Builds a multipart/alternative payload carrying a text/plain summary and
    /// the canonical text/calendar; method=REPLY part, which is what RFC 6047
    /// requires for an interoperable RSVP back to Outlook / Apple / Google.
    /// CONSTRAINTS: `ics_payload` must already be a complete iCalendar string
    /// with `METHOD:REPLY` (built by `services::ics_generator::generate_imip_reply`).
    /// EXTERNAL: Connects to the configured SMTP host with the user's
    /// credentials, same auth path as `send()`.
    #[allow(clippy::too_many_arguments)]
    pub async fn send_imip_reply(
        &self,
        from_address: &str,
        from_password: &str,
        to_address: &str,
        subject: &str,
        text_body: &str,
        ics_payload: &str,
    ) -> Result<(), AppError> {
        let from: LettreMailbox = from_address
            .parse()
            .map_err(|e| AppError::BadRequest(format!("Invalid from address: {}", e)))?;
        let to: LettreMailbox = to_address
            .parse()
            .map_err(|e| AppError::BadRequest(format!("Invalid to address '{}': {}", to_address, e)))?;

        // NOTE: text/calendar parts in an iMIP REPLY carry the method as a parameter
        // on Content-Type. lettre needs the parameter spelled out via ContentType::parse.
        let calendar_ct =
            ContentType::parse("text/calendar; method=REPLY; charset=UTF-8")
                .map_err(|e| AppError::Smtp(format!("Invalid calendar content type: {}", e)))?;

        let email = Message::builder()
            .from(from)
            .to(to)
            .subject(subject)
            .multipart(
                MultiPart::alternative()
                    .singlepart(
                        SinglePart::builder()
                            .header(ContentType::TEXT_PLAIN)
                            .body(text_body.to_string()),
                    )
                    .singlepart(
                        SinglePart::builder()
                            .header(calendar_ct)
                            .body(ics_payload.to_string()),
                    ),
            )
            .map_err(|e| AppError::Smtp(format!("Failed to build iMIP reply: {}", e)))?;

        let creds = Credentials::new(from_address.to_string(), from_password.to_string());
        let transport = if self.config.tls {
            AsyncSmtpTransport::<Tokio1Executor>::starttls_relay(&self.config.host)
                .map_err(|e| AppError::Smtp(format!("SMTP transport error: {}", e)))?
                .port(self.config.port)
                .credentials(creds)
                .build()
        } else {
            AsyncSmtpTransport::<Tokio1Executor>::builder_dangerous(&self.config.host)
                .port(self.config.port)
                .credentials(creds)
                .build()
        };
        transport
            .send(email)
            .await
            .map_err(|e| AppError::Smtp(format!("Failed to send iMIP reply: {}", e)))?;
        Ok(())
    }

    /// Added: Send a system-originated notification (billing receipt, OTP, password reset, etc.)
    /// Uses the configured `notification_from` / `notification_username` / `notification_password`
    /// fields from SmtpConfig — defaults to noreply@techatscale.io.
    /// Falls back to anonymous SMTP (no auth) if no notification credentials are set, which works
    /// for in-house Postfix relays that trust loopback.
    pub async fn send_notification(&self, request: &SendRequest) -> Result<(), AppError> {
        let from_address = self
            .config
            .notification_from
            .clone()
            .unwrap_or_else(|| "noreply@techatscale.io".to_string());

        let from: LettreMailbox = from_address
            .parse()
            .map_err(|e| AppError::BadRequest(format!("Invalid notification from address: {}", e)))?;

        let mut builder = Message::builder().from(from);
        for to in &request.to {
            let to_mailbox: LettreMailbox = to
                .parse()
                .map_err(|e| AppError::BadRequest(format!("Invalid to address '{}': {}", to, e)))?;
            builder = builder.to(to_mailbox);
        }
        builder = builder.subject(&request.subject);

        let email = match (&request.text_body, &request.html_body) {
            (Some(text), Some(html)) => builder
                .multipart(
                    MultiPart::alternative()
                        .singlepart(SinglePart::builder().header(ContentType::TEXT_PLAIN).body(text.clone()))
                        .singlepart(SinglePart::builder().header(ContentType::TEXT_HTML).body(html.clone())),
                )
                .map_err(|e| AppError::Smtp(format!("Failed to build email: {}", e)))?,
            (Some(text), None) => builder
                .header(ContentType::TEXT_PLAIN)
                .body(text.clone())
                .map_err(|e| AppError::Smtp(format!("Failed to build email: {}", e)))?,
            (None, Some(html)) => builder
                .header(ContentType::TEXT_HTML)
                .body(html.clone())
                .map_err(|e| AppError::Smtp(format!("Failed to build email: {}", e)))?,
            (None, None) => {
                return Err(AppError::BadRequest(
                    "Notification email must have a text or HTML body".to_string(),
                ));
            }
        };

        // Build transport with optional notification credentials (or anonymous loopback).
        let transport = match (&self.config.notification_username, &self.config.notification_password) {
            (Some(user), Some(pass)) => {
                let creds = Credentials::new(user.clone(), pass.clone());
                if self.config.tls {
                    AsyncSmtpTransport::<Tokio1Executor>::starttls_relay(&self.config.host)
                        .map_err(|e| AppError::Smtp(format!("SMTP transport error: {}", e)))?
                        .port(self.config.port)
                        .credentials(creds)
                        .build()
                } else {
                    AsyncSmtpTransport::<Tokio1Executor>::builder_dangerous(&self.config.host)
                        .port(self.config.port)
                        .credentials(creds)
                        .build()
                }
            }
            _ => {
                if self.config.tls {
                    AsyncSmtpTransport::<Tokio1Executor>::starttls_relay(&self.config.host)
                        .map_err(|e| AppError::Smtp(format!("SMTP transport error: {}", e)))?
                        .port(self.config.port)
                        .build()
                } else {
                    AsyncSmtpTransport::<Tokio1Executor>::builder_dangerous(&self.config.host)
                        .port(self.config.port)
                        .build()
                }
            }
        };

        transport
            .send(email)
            .await
            .map_err(|e| AppError::Smtp(format!("Failed to send notification email: {}", e)))?;

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_send_request_deserialization_full() {
        let json = r#"{
            "to": ["alice@example.com", "bob@example.com"],
            "cc": ["charlie@example.com"],
            "bcc": ["dave@example.com"],
            "subject": "Test email",
            "text_body": "Hello plain",
            "html_body": "<p>Hello HTML</p>"
        }"#;
        let req: SendRequest = serde_json::from_str(json).unwrap();
        assert_eq!(req.to.len(), 2);
        assert_eq!(req.cc.as_ref().unwrap().len(), 1);
        assert_eq!(req.bcc.as_ref().unwrap().len(), 1);
        assert_eq!(req.subject, "Test email");
        assert_eq!(req.text_body.as_deref(), Some("Hello plain"));
        assert_eq!(req.html_body.as_deref(), Some("<p>Hello HTML</p>"));
    }

    #[test]
    fn test_send_request_deserialization_minimal() {
        let json = r#"{
            "to": ["user@example.com"],
            "subject": "Minimal"
        }"#;
        let req: SendRequest = serde_json::from_str(json).unwrap();
        assert_eq!(req.to, vec!["user@example.com"]);
        assert_eq!(req.subject, "Minimal");
        assert!(req.cc.is_none());
        assert!(req.bcc.is_none());
        assert!(req.text_body.is_none());
        assert!(req.html_body.is_none());
    }

    #[test]
    fn test_send_request_empty_to_fails() {
        let json = r#"{"subject": "No recipients"}"#;
        let result = serde_json::from_str::<SendRequest>(json);
        assert!(result.is_err());
    }

    #[test]
    fn test_send_request_multiple_recipients() {
        let json = r#"{
            "to": ["a@test.com", "b@test.com", "c@test.com"],
            "subject": "Group mail"
        }"#;
        let req: SendRequest = serde_json::from_str(json).unwrap();
        assert_eq!(req.to.len(), 3);
    }

    #[test]
    fn test_smtp_service_creation() {
        let config = SmtpConfig {
            host: "smtp.example.com".to_string(),
            port: 587,
            tls: true,
            // notification_* fields gained in commit 4 (TMAIL noreply notification path).
            notification_from: None,
            notification_username: None,
            notification_password: None,
        };
        let _service = SmtpService::new(config);
        // Service created without panic
    }
}
