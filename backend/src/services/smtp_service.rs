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
}
