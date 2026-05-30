use lettre::{
    message::{
        header::{ContentType, InReplyTo, References},
        Mailbox as LettreMailbox, MultiPart, SinglePart,
    },
    transport::smtp::authentication::Credentials,
    AsyncSmtpTransport, AsyncTransport, Message, Tokio1Executor,
};

use crate::config::SmtpConfig;
use crate::error::AppError;

/// Request to send an email
#[derive(Debug, Default, serde::Deserialize)]
pub struct SendRequest {
    pub to: Vec<String>,
    pub cc: Option<Vec<String>>,
    pub bcc: Option<Vec<String>>,
    pub subject: String,
    pub text_body: Option<String>,
    pub html_body: Option<String>,
    // TMAIL-319: optional reply / forward threading headers (RFC 5322 §3.6.4).
    // `in_reply_to` is the source message's `Message-Id`; `references` is the
    // existing chain plus the source message id appended (so the next reply in
    // the thread gets a complete history). Both are skipped on a fresh compose.
    #[serde(default)]
    pub in_reply_to: Option<String>,
    #[serde(default)]
    pub references: Option<Vec<String>>,
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
        let email = Self::build_outgoing_message(from_address, request)?;
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

    /// PURPOSE: Build the lettre `Message` that `send()` will dial out, with
    /// support for optional reply / forward threading headers. Extracted so
    /// the address parsing, header stamping, and multipart shape are unit
    /// testable without standing up an SMTP server (matches the pattern of
    /// `build_imip_request_message`).
    ///
    /// CONSTRAINTS: Caller must have populated `request.to` with at least one
    /// address; the rest of `request` is plain SendRequest semantics. Reply
    /// headers (`in_reply_to`, `references`) are no-ops when None / empty.
    pub fn build_outgoing_message(
        from_address: &str,
        request: &SendRequest,
    ) -> Result<Message, AppError> {
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

        // TMAIL-319: stamp reply/forward threading headers when present. The
        // composer fills these on Reply / Reply All / Forward so downstream
        // mail clients render the message inside the existing conversation
        // (RFC 5322 §3.6.4). Both are no-ops on a fresh compose.
        if let Some(in_reply_to) = request.in_reply_to.as_deref() {
            let trimmed = in_reply_to.trim();
            if !trimmed.is_empty() {
                builder = builder.header(InReplyTo::from(trimmed.to_string()));
            }
        }
        if let Some(references) = request.references.as_deref() {
            // RFC 5322 says References is a single header containing the
            // whitespace-separated chain of message-ids — join with spaces
            // rather than emitting multiple headers (some receiving MTAs
            // only honour the first).
            let joined = references
                .iter()
                .map(|s| s.trim())
                .filter(|s| !s.is_empty())
                .collect::<Vec<_>>()
                .join(" ");
            if !joined.is_empty() {
                builder = builder.header(References::from(joined));
            }
        }

        match (&request.text_body, &request.html_body) {
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
                .map_err(|e| AppError::Smtp(format!("Failed to build email: {}", e))),
            (Some(text), None) => builder
                .header(ContentType::TEXT_PLAIN)
                .body(text.clone())
                .map_err(|e| AppError::Smtp(format!("Failed to build email: {}", e))),
            (None, Some(html)) => builder
                .header(ContentType::TEXT_HTML)
                .body(html.clone())
                .map_err(|e| AppError::Smtp(format!("Failed to build email: {}", e))),
            (None, None) => Err(AppError::BadRequest(
                "Email must have a text or HTML body".to_string(),
            )),
        }
    }

    /// PURPOSE: Build the lettre `Message` for an outbound iMIP invitation
    /// (METHOD:REQUEST). Extracted from `send_imip_request` so tests can
    /// inspect the multipart shape without dialling an SMTP server.
    /// CONSTRAINTS: `ics_payload` must be a complete iCalendar string with
    /// `METHOD:REQUEST` (built by `services::ics_generator::generate_ics`).
    pub fn build_imip_request_message(
        from_address: &str,
        to_address: &str,
        subject: &str,
        text_body: &str,
        ics_payload: &str,
    ) -> Result<Message, AppError> {
        let from: LettreMailbox = from_address
            .parse()
            .map_err(|e| AppError::BadRequest(format!("Invalid from address: {}", e)))?;
        let to: LettreMailbox = to_address
            .parse()
            .map_err(|e| AppError::BadRequest(format!("Invalid to address '{}': {}", to_address, e)))?;

        // NOTE: RFC 6047 — the text/calendar part of an iMIP REQUEST carries
        // the iTIP method as a Content-Type parameter. lettre needs the
        // parameter spelled out via ContentType::parse.
        let calendar_ct = ContentType::parse("text/calendar; method=REQUEST; charset=UTF-8")
            .map_err(|e| AppError::Smtp(format!("Invalid calendar content type: {}", e)))?;

        Message::builder()
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
            .map_err(|e| AppError::Smtp(format!("Failed to build iMIP request: {}", e)))
    }

    /// PURPOSE: Send a METHOD:REQUEST iMIP invitation to one attendee. Used
    /// by the calendar create-event flow (TMAIL-127) so attendees receive
    /// a calendar-app-recognised invite in their inbox.
    /// CONSTRAINTS: `ics_payload` must already carry `METHOD:REQUEST`. Auth
    /// uses the same BYO-SMTP credentials as `send()` and `send_imip_reply`.
    pub async fn send_imip_request(
        &self,
        from_address: &str,
        from_password: &str,
        to_address: &str,
        subject: &str,
        text_body: &str,
        ics_payload: &str,
    ) -> Result<(), AppError> {
        let email = Self::build_imip_request_message(
            from_address,
            to_address,
            subject,
            text_body,
            ics_payload,
        )?;

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
            .map_err(|e| AppError::Smtp(format!("Failed to send iMIP request: {}", e)))?;
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

    // Added (TMAIL-127): verify the multipart shape of an outbound iMIP
    // invitation. We don't need an SMTP server — `Message::formatted()`
    // emits the full RFC 5322 byte stream that lettre would dial out.
    #[test]
    fn test_build_imip_request_message_multipart_shape() {
        let ics = "BEGIN:VCALENDAR\r\nVERSION:2.0\r\nMETHOD:REQUEST\r\n\
                   BEGIN:VEVENT\r\nUID:invite-1@example.com\r\nSUMMARY:Sync\r\n\
                   END:VEVENT\r\nEND:VCALENDAR\r\n";
        let msg = SmtpService::build_imip_request_message(
            "organizer@example.com",
            "attendee@example.com",
            "Invitation: Project Sync",
            "You have been invited to: Project Sync",
            ics,
        )
        .expect("message build should succeed");

        let raw = String::from_utf8(msg.formatted())
            .expect("lettre formats as UTF-8 for ASCII headers/bodies");

        // ---- Envelope headers -----------------------------------------
        assert!(raw.contains("From: organizer@example.com"), "from header missing");
        assert!(raw.contains("To: attendee@example.com"), "to header missing");
        assert!(raw.contains("Subject: Invitation: Project Sync"), "subject missing");

        // ---- Multipart shape (text/plain + text/calendar) -------------
        assert!(
            raw.to_lowercase().contains("content-type: multipart/alternative"),
            "expected multipart/alternative wrapper, got:\n{}",
            raw
        );
        assert!(raw.contains("text/plain"), "text/plain part missing");
        assert!(raw.contains("text/calendar"), "text/calendar part missing");
        assert!(
            raw.contains("method=REQUEST"),
            "calendar Content-Type missing method=REQUEST parameter"
        );

        // ---- Both bodies present in the assembled MIME ----------------
        assert!(
            raw.contains("You have been invited to: Project Sync"),
            "plain-text fallback body missing"
        );
        // The ICS body is base64-encoded by lettre for non-ASCII safety,
        // but the VCALENDAR markers survive ASCII so they appear verbatim
        // OR within the encoded payload. Check the iCal source-of-truth
        // marker shows up either way by also checking for the method line.
        assert!(
            raw.contains("BEGIN:VCALENDAR") || raw.contains("QkVHSU46VkNBTEVO"),
            "ICS payload not present in MIME body"
        );
    }

    #[test]
    fn test_build_imip_request_message_rejects_bad_from_address() {
        let res = SmtpService::build_imip_request_message(
            "not-an-email",
            "attendee@example.com",
            "Subject",
            "body",
            "BEGIN:VCALENDAR\r\nEND:VCALENDAR\r\n",
        );
        assert!(matches!(res, Err(AppError::BadRequest(_))));
    }

    #[test]
    fn test_build_imip_request_message_rejects_bad_to_address() {
        let res = SmtpService::build_imip_request_message(
            "organizer@example.com",
            "also-not-an-email",
            "Subject",
            "body",
            "BEGIN:VCALENDAR\r\nEND:VCALENDAR\r\n",
        );
        assert!(matches!(res, Err(AppError::BadRequest(_))));
    }

    // Added (TMAIL-319): wire-format guard — a Reply / Reply All / Forward
    // request from the modern UI MUST round-trip its threading headers. A
    // serde rename or accidental rename of the field would silently disable
    // threading without anyone noticing.
    #[test]
    fn send_request_deserialises_reply_headers() {
        let json = r#"{
            "to": ["alice@example.com"],
            "subject": "Re: Hi",
            "text_body": "Sure, sounds good.",
            "in_reply_to": "<orig-1@example.com>",
            "references": ["<thread-root@example.com>", "<orig-1@example.com>"]
        }"#;
        let req: SendRequest = serde_json::from_str(json).unwrap();
        assert_eq!(req.in_reply_to.as_deref(), Some("<orig-1@example.com>"));
        let refs = req.references.expect("references must round-trip");
        assert_eq!(refs.len(), 2);
        assert_eq!(refs[0], "<thread-root@example.com>");
        assert_eq!(refs[1], "<orig-1@example.com>");
    }

    // Added (TMAIL-319): the lettre `Message` produced by `build_outgoing_message`
    // MUST carry the `In-Reply-To` and `References` headers when the request
    // supplies them — otherwise sent replies show up as top-level threads in
    // downstream mail clients (the exact bug TMAIL-319 fixes).
    #[test]
    fn build_outgoing_message_stamps_reply_threading_headers() {
        let req = SendRequest {
            to: vec!["alice@example.com".into()],
            subject: "Re: Project update".into(),
            text_body: Some("Sure, sounds good.\n\nOn Mon ... wrote:\n> original".into()),
            in_reply_to: Some("<orig-1@example.com>".into()),
            references: Some(vec![
                "<thread-root@example.com>".into(),
                "<orig-1@example.com>".into(),
            ]),
            ..Default::default()
        };
        let msg = SmtpService::build_outgoing_message("me@example.com", &req)
            .expect("message build should succeed");
        let raw = String::from_utf8(msg.formatted())
            .expect("ASCII test fixture should format as UTF-8");
        assert!(
            raw.contains("In-Reply-To: <orig-1@example.com>"),
            "In-Reply-To header missing:\n{}",
            raw
        );
        assert!(
            raw.contains(
                "References: <thread-root@example.com> <orig-1@example.com>"
            ),
            "References header missing or wrongly delimited:\n{}",
            raw
        );
    }

    // Added (TMAIL-319): a brand-new compose (no reply context) must NOT emit
    // empty / phantom threading headers — that's worse than missing because
    // some MTAs reject empty header values outright.
    #[test]
    fn build_outgoing_message_omits_threading_headers_for_fresh_compose() {
        let req = SendRequest {
            to: vec!["alice@example.com".into()],
            subject: "Hello".into(),
            text_body: Some("Just saying hi.".into()),
            ..Default::default()
        };
        let msg = SmtpService::build_outgoing_message("me@example.com", &req)
            .expect("message build should succeed");
        let raw = String::from_utf8(msg.formatted())
            .expect("ASCII test fixture should format as UTF-8");
        assert!(
            !raw.contains("In-Reply-To:"),
            "fresh compose must not emit In-Reply-To:\n{}",
            raw
        );
        assert!(
            !raw.contains("References:"),
            "fresh compose must not emit References:\n{}",
            raw
        );
    }

    // Added (TMAIL-319): whitespace-only header values must be skipped — a
    // composer bug that emits `in_reply_to: ""` should not poison the outbound
    // message with an empty header.
    #[test]
    fn build_outgoing_message_skips_blank_reply_headers() {
        let req = SendRequest {
            to: vec!["alice@example.com".into()],
            subject: "Hello".into(),
            text_body: Some("Body".into()),
            in_reply_to: Some("   ".into()),
            references: Some(vec!["".into(), "   ".into()]),
            ..Default::default()
        };
        let msg = SmtpService::build_outgoing_message("me@example.com", &req)
            .expect("message build should succeed");
        let raw = String::from_utf8(msg.formatted())
            .expect("ASCII test fixture should format as UTF-8");
        assert!(!raw.contains("In-Reply-To:"), "blank In-Reply-To must be skipped");
        assert!(!raw.contains("References:"), "all-blank References must be skipped");
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
