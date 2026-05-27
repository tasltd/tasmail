// Added: Centralized input validation module for security hardening (TMAIL-37)
// PURPOSE: Validates user input lengths, email formats, and other constraints
// to prevent abuse and injection attacks across all handlers.

use crate::error::AppError;

/// Added: Maximum input length constants to prevent oversized payloads
pub const MAX_USERNAME_LEN: usize = 254; // RFC 5321 max email length
pub const MAX_PASSWORD_LEN: usize = 128; // Prevent bcrypt/argon2 DoS with huge passwords
pub const MIN_PASSWORD_LEN: usize = 8;
pub const MAX_DISPLAY_NAME_LEN: usize = 200;
pub const MAX_SUBJECT_LEN: usize = 998; // RFC 2822 max header line
pub const MAX_SEARCH_QUERY_LEN: usize = 500;
pub const MAX_FOLDER_NAME_LEN: usize = 200;
// Added: Maximum number of recipients per send to prevent abuse / spam relay (TMAIL-37)
pub const MAX_RECIPIENTS_PER_MESSAGE: usize = 100;
// Added: Maximum body size for sent messages — defense-in-depth against payload abuse
pub const MAX_MESSAGE_BODY_LEN: usize = 10 * 1024 * 1024; // 10 MiB

/// Added: Reject any string containing CR (\r), LF (\n) or NUL (\0).
/// PURPOSE: Defense-in-depth against header injection in SMTP/IMAP protocol payloads.
/// Used by recipient, subject, and any value that gets serialised into RFC 2822 headers.
fn contains_protocol_break(s: &str) -> bool {
    s.contains('\r') || s.contains('\n') || s.contains('\0')
}

/// Added: Validate an email address used as a recipient (To/Cc/Bcc) or From.
/// PURPOSE: Blocks email header injection (CRLF) and oversized values before we
/// hand the address to `lettre::Mailbox::parse` or interpolate it into a raw
/// RFC 2822 message (e.g. `save_draft`). See TMAIL-37.
pub fn validate_email_address(addr: &str) -> Result<(), AppError> {
    let trimmed = addr.trim();
    if trimmed.is_empty() {
        return Err(AppError::BadRequest("Email address cannot be empty".to_string()));
    }
    if trimmed.len() > MAX_USERNAME_LEN {
        return Err(AppError::BadRequest(format!(
            "Email address exceeds maximum length of {} characters",
            MAX_USERNAME_LEN
        )));
    }
    if contains_protocol_break(trimmed) {
        return Err(AppError::BadRequest(
            "Email address contains invalid characters".to_string(),
        ));
    }
    // NOTE: Minimal structural check; full RFC 5322 parsing is left to lettre.
    if !trimmed.contains('@') || trimmed.starts_with('@') || trimmed.ends_with('@') {
        return Err(AppError::BadRequest(
            "Email address is not a valid address".to_string(),
        ));
    }
    Ok(())
}

/// Added: Validate a list of recipient addresses (To/Cc/Bcc).
/// PURPOSE: Caps the number of recipients per outbound message and runs
/// `validate_email_address` on each entry to prevent header injection /
/// spam relay abuse. See TMAIL-37.
pub fn validate_recipient_list(label: &str, addrs: &[String]) -> Result<(), AppError> {
    if addrs.len() > MAX_RECIPIENTS_PER_MESSAGE {
        return Err(AppError::BadRequest(format!(
            "{} list exceeds maximum of {} recipients",
            label, MAX_RECIPIENTS_PER_MESSAGE
        )));
    }
    for addr in addrs {
        validate_email_address(addr)?;
    }
    Ok(())
}

/// Added: Validate email username format and length
pub fn validate_username(username: &str) -> Result<(), AppError> {
    if username.is_empty() {
        return Err(AppError::BadRequest("Username cannot be empty".to_string()));
    }
    if username.len() > MAX_USERNAME_LEN {
        return Err(AppError::BadRequest(format!(
            "Username exceeds maximum length of {} characters",
            MAX_USERNAME_LEN
        )));
    }
    // NOTE: Basic email format validation — must contain @ with text on both sides
    if !username.contains('@') || username.starts_with('@') || username.ends_with('@') {
        return Err(AppError::BadRequest(
            "Username must be a valid email address".to_string(),
        ));
    }
    Ok(())
}

/// Added: Validate password length to prevent DoS via huge password hashing
pub fn validate_password(password: &str) -> Result<(), AppError> {
    if password.len() < MIN_PASSWORD_LEN {
        return Err(AppError::BadRequest(format!(
            "Password must be at least {} characters",
            MIN_PASSWORD_LEN
        )));
    }
    if password.len() > MAX_PASSWORD_LEN {
        return Err(AppError::BadRequest(format!(
            "Password exceeds maximum length of {} characters",
            MAX_PASSWORD_LEN
        )));
    }
    Ok(())
}

/// Added: Validate display name length
pub fn validate_display_name(name: &str) -> Result<(), AppError> {
    if name.len() > MAX_DISPLAY_NAME_LEN {
        return Err(AppError::BadRequest(format!(
            "Display name exceeds maximum length of {} characters",
            MAX_DISPLAY_NAME_LEN
        )));
    }
    Ok(())
}

/// Added: Validate search query length to prevent IMAP search abuse
pub fn validate_search_query(query: &str) -> Result<(), AppError> {
    if query.is_empty() {
        return Err(AppError::BadRequest("Search query cannot be empty".to_string()));
    }
    if query.len() > MAX_SEARCH_QUERY_LEN {
        return Err(AppError::BadRequest(format!(
            "Search query exceeds maximum length of {} characters",
            MAX_SEARCH_QUERY_LEN
        )));
    }
    // Added: Block IMAP command injection characters
    if query.contains('\r') || query.contains('\n') || query.contains('\0') {
        return Err(AppError::BadRequest(
            "Search query contains invalid characters".to_string(),
        ));
    }
    Ok(())
}

/// Added: Validate IMAP folder name to prevent command injection
pub fn validate_folder_name(folder: &str) -> Result<(), AppError> {
    if folder.is_empty() {
        return Err(AppError::BadRequest("Folder name cannot be empty".to_string()));
    }
    if folder.len() > MAX_FOLDER_NAME_LEN {
        return Err(AppError::BadRequest(
            "Folder name exceeds maximum length".to_string(),
        ));
    }
    // Added: Block IMAP protocol injection via folder names
    if folder.contains('\r') || folder.contains('\n') || folder.contains('\0') {
        return Err(AppError::BadRequest(
            "Folder name contains invalid characters".to_string(),
        ));
    }
    Ok(())
}

/// Added: Validate subject line length and reject CR/LF/NUL.
/// Changed: Now also blocks header injection — without this, a subject like
/// "Hi\r\nBcc: attacker@evil.com" could splice a Bcc into the outbound
/// RFC 2822 message (especially in `save_draft` which builds the message
/// via raw `format!`). See TMAIL-37.
pub fn validate_subject(subject: &str) -> Result<(), AppError> {
    if subject.len() > MAX_SUBJECT_LEN {
        return Err(AppError::BadRequest(format!(
            "Subject exceeds maximum length of {} characters",
            MAX_SUBJECT_LEN
        )));
    }
    if contains_protocol_break(subject) {
        return Err(AppError::BadRequest(
            "Subject contains invalid characters".to_string(),
        ));
    }
    Ok(())
}

/// Added: Validate message body size to cap outbound payloads (defense-in-depth).
/// PURPOSE: Rejects bodies larger than `MAX_MESSAGE_BODY_LEN`. Attachments go
/// through a different path with its own cap (`StorageConfig::max_file_size`).
pub fn validate_body_size(body: &str) -> Result<(), AppError> {
    if body.len() > MAX_MESSAGE_BODY_LEN {
        return Err(AppError::BadRequest(format!(
            "Message body exceeds maximum size of {} bytes",
            MAX_MESSAGE_BODY_LEN
        )));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    // -- Username validation tests --

    #[test]
    fn test_validate_username_valid() {
        assert!(validate_username("user@example.com").is_ok());
        assert!(validate_username("a@b.co").is_ok());
        assert!(validate_username("long.name+tag@subdomain.example.org").is_ok());
    }

    #[test]
    fn test_validate_username_empty() {
        let err = validate_username("").unwrap_err();
        assert!(matches!(err, AppError::BadRequest(_)));
    }

    #[test]
    fn test_validate_username_no_at() {
        let err = validate_username("userexample.com").unwrap_err();
        assert!(matches!(err, AppError::BadRequest(_)));
    }

    #[test]
    fn test_validate_username_starts_with_at() {
        let err = validate_username("@example.com").unwrap_err();
        assert!(matches!(err, AppError::BadRequest(_)));
    }

    #[test]
    fn test_validate_username_ends_with_at() {
        let err = validate_username("user@").unwrap_err();
        assert!(matches!(err, AppError::BadRequest(_)));
    }

    #[test]
    fn test_validate_username_too_long() {
        let long_name = format!("{}@example.com", "a".repeat(300));
        let err = validate_username(&long_name).unwrap_err();
        assert!(matches!(err, AppError::BadRequest(_)));
    }

    // -- Password validation tests --

    #[test]
    fn test_validate_password_valid() {
        assert!(validate_password("strongP@ss1").is_ok());
        assert!(validate_password("12345678").is_ok());
    }

    #[test]
    fn test_validate_password_too_short() {
        let err = validate_password("short").unwrap_err();
        assert!(matches!(err, AppError::BadRequest(_)));
    }

    #[test]
    fn test_validate_password_too_long() {
        let long_pw = "a".repeat(200);
        let err = validate_password(&long_pw).unwrap_err();
        assert!(matches!(err, AppError::BadRequest(_)));
    }

    #[test]
    fn test_validate_password_min_boundary() {
        assert!(validate_password("12345678").is_ok()); // exactly 8
        assert!(validate_password("1234567").is_err()); // 7
    }

    #[test]
    fn test_validate_password_max_boundary() {
        assert!(validate_password(&"a".repeat(128)).is_ok()); // exactly 128
        assert!(validate_password(&"a".repeat(129)).is_err()); // 129
    }

    // -- Display name validation tests --

    #[test]
    fn test_validate_display_name_valid() {
        assert!(validate_display_name("John Doe").is_ok());
        assert!(validate_display_name("").is_ok()); // Empty is OK (optional field)
    }

    #[test]
    fn test_validate_display_name_too_long() {
        let long_name = "x".repeat(201);
        assert!(validate_display_name(&long_name).is_err());
    }

    // -- Search query validation tests --

    #[test]
    fn test_validate_search_query_valid() {
        assert!(validate_search_query("invoice 2024").is_ok());
    }

    #[test]
    fn test_validate_search_query_empty() {
        assert!(validate_search_query("").is_err());
    }

    #[test]
    fn test_validate_search_query_too_long() {
        let long_q = "a".repeat(501);
        assert!(validate_search_query(&long_q).is_err());
    }

    #[test]
    fn test_validate_search_query_injection_newline() {
        assert!(validate_search_query("test\r\nLOGOUT").is_err());
    }

    #[test]
    fn test_validate_search_query_injection_null() {
        assert!(validate_search_query("test\0").is_err());
    }

    // -- Folder name validation tests --

    #[test]
    fn test_validate_folder_name_valid() {
        assert!(validate_folder_name("INBOX").is_ok());
        assert!(validate_folder_name("Sent").is_ok());
        assert!(validate_folder_name("My Folder/Sub").is_ok());
    }

    #[test]
    fn test_validate_folder_name_empty() {
        assert!(validate_folder_name("").is_err());
    }

    #[test]
    fn test_validate_folder_name_injection() {
        assert!(validate_folder_name("INBOX\r\nLOGOUT").is_err());
    }

    // -- Subject validation tests --

    #[test]
    fn test_validate_subject_valid() {
        assert!(validate_subject("Hello World").is_ok());
        assert!(validate_subject("").is_ok()); // Empty subject is valid
    }

    #[test]
    fn test_validate_subject_too_long() {
        let long_subj = "a".repeat(999);
        assert!(validate_subject(&long_subj).is_err());
    }

    // Added: TMAIL-37 — subject must reject CRLF/NUL to prevent header injection
    #[test]
    fn test_validate_subject_rejects_crlf_injection() {
        assert!(validate_subject("Hi\r\nBcc: attacker@evil.com").is_err());
        assert!(validate_subject("Hi\nFrom: spoof@evil.com").is_err());
        assert!(validate_subject("Hi\rSomething").is_err());
        assert!(validate_subject("Hi\0Null").is_err());
    }

    // -- Email address validation tests (TMAIL-37) --

    #[test]
    fn test_validate_email_address_valid() {
        assert!(validate_email_address("user@example.com").is_ok());
        assert!(validate_email_address("a.b+tag@sub.example.co").is_ok());
    }

    #[test]
    fn test_validate_email_address_rejects_empty() {
        assert!(validate_email_address("").is_err());
        assert!(validate_email_address("   ").is_err());
    }

    #[test]
    fn test_validate_email_address_rejects_crlf_injection() {
        // The classic header-injection payload — must be rejected before reaching SMTP
        assert!(validate_email_address("user@example.com\r\nBcc: x@evil.com").is_err());
        assert!(validate_email_address("user@example.com\nFrom: spoof@evil.com").is_err());
        assert!(validate_email_address("user@example.com\0").is_err());
    }

    #[test]
    fn test_validate_email_address_rejects_malformed() {
        assert!(validate_email_address("not-an-email").is_err());
        assert!(validate_email_address("@example.com").is_err());
        assert!(validate_email_address("user@").is_err());
    }

    #[test]
    fn test_validate_email_address_rejects_oversized() {
        let huge = format!("{}@example.com", "a".repeat(MAX_USERNAME_LEN));
        assert!(validate_email_address(&huge).is_err());
    }

    // -- Recipient list validation tests (TMAIL-37) --

    #[test]
    fn test_validate_recipient_list_valid() {
        let list = vec!["a@x.com".to_string(), "b@y.com".to_string()];
        assert!(validate_recipient_list("To", &list).is_ok());
    }

    #[test]
    fn test_validate_recipient_list_empty_is_ok() {
        // NOTE: emptiness is enforced at the API level (To required), but the
        // helper itself should accept an empty list — Cc/Bcc are often empty.
        let list: Vec<String> = vec![];
        assert!(validate_recipient_list("Cc", &list).is_ok());
    }

    #[test]
    fn test_validate_recipient_list_rejects_too_many() {
        let list: Vec<String> = (0..MAX_RECIPIENTS_PER_MESSAGE + 1)
            .map(|i| format!("u{}@example.com", i))
            .collect();
        assert!(validate_recipient_list("To", &list).is_err());
    }

    #[test]
    fn test_validate_recipient_list_rejects_injected_entry() {
        let list = vec![
            "ok@example.com".to_string(),
            "bad@example.com\r\nBcc: evil@x.com".to_string(),
        ];
        assert!(validate_recipient_list("To", &list).is_err());
    }

    // -- Body size validation tests (TMAIL-37) --

    #[test]
    fn test_validate_body_size_small_ok() {
        assert!(validate_body_size("hello").is_ok());
        assert!(validate_body_size("").is_ok());
    }

    #[test]
    fn test_validate_body_size_at_limit_ok() {
        let body = "a".repeat(MAX_MESSAGE_BODY_LEN);
        assert!(validate_body_size(&body).is_ok());
    }

    #[test]
    fn test_validate_body_size_too_large_rejected() {
        let body = "a".repeat(MAX_MESSAGE_BODY_LEN + 1);
        assert!(validate_body_size(&body).is_err());
    }
}
