// Added (TMAIL-386): SQLx model for the `remote_image_allowlist` table.
//
// Per-user, per-sender opt-in that tells the Classic UI message render path
// (`handlers::classic::message::get_message`) to surface real remote `<img
// src="http(s)://...">` URLs instead of the privacy-aware placeholder the
// sanitiser (`services::html_sanitizer`) rewrites them to by default.
//
// Lifecycle:
//   * `is_allowed()` — read-side, per message render. Cheap point lookup on
//     the `(mailbox_id, sender_address)` UNIQUE constraint.
//   * `allow_sender()` — write-side, fired by the "Always show images from
//     this sender" form on the read view. Idempotent: re-clicking the button
//     is a safe no-op (ON CONFLICT DO NOTHING).
//   * `delete_sender()` — for the future settings page that lists every
//     allowed sender + a per-row "Remove" button (P2 follow-up).
//
// Schema lives in `migrations/084_remote_image_allowlist.sql`.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::{FromRow, PgPool};
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct RemoteImageAllowlistRow {
    pub id: Uuid,
    pub mailbox_id: Uuid,
    pub sender_address: String,
    pub created_at: DateTime<Utc>,
}

/// Normalise a sender address for storage + lookup. The migration's UNIQUE
/// constraint is case-sensitive, so every read AND every write MUST run the
/// address through this helper. Drops surrounding whitespace then ASCII-
/// lowercases (we don't try to do unicode-case-fold here — domain names are
/// ASCII-only after IDN normalisation, and the local part is RFC 5321 case-
/// sensitive in theory but every real-world provider treats it case-
/// insensitively).
pub fn normalise(address: &str) -> String {
    address.trim().to_ascii_lowercase()
}

/// PURPOSE: Read-side check used by the message render handler. Returns
/// `true` when the user has previously allowed this sender's remote images.
///
/// Hard-defended with an explicit `WHERE mailbox_id = $1` clause so the
/// lookup is correct whether or not RLS is primed on this connection (the
/// Classic UI handlers use the raw pool, which bypasses RLS — the policy
/// in migration 084 is the belt-and-braces case for the future admin
/// surface that runs under the auth_middleware).
///
/// Returns `Ok(false)` for an empty or malformed address — defensive, so
/// a parser miss can't accidentally allowlist every "" row in the table.
pub async fn is_allowed(
    pool: &PgPool,
    mailbox_id: Uuid,
    sender_address: &str,
) -> Result<bool, sqlx::Error> {
    let normalised = normalise(sender_address);
    if normalised.is_empty() || !normalised.contains('@') {
        return Ok(false);
    }
    let row: Option<(Uuid,)> = sqlx::query_as(
        "SELECT id FROM remote_image_allowlist \
         WHERE mailbox_id = $1 AND sender_address = $2 LIMIT 1",
    )
    .bind(mailbox_id)
    .bind(&normalised)
    .fetch_optional(pool)
    .await?;
    Ok(row.is_some())
}

/// PURPOSE: Write-side. Idempotently INSERTs the `(mailbox_id, address)`
/// row. Returns `Ok(true)` when a new row was created, `Ok(false)` when the
/// row already existed (re-click of the "Always show images" button).
///
/// Returns `Err(sqlx::Error::Protocol)` shaped via `sqlx::Error::RowNotFound`
/// → no — we use `ON CONFLICT DO NOTHING` and inspect `rows_affected()` so a
/// no-op insert doesn't error out. Malformed addresses (empty / no `@`) are
/// rejected with `sqlx::Error::Protocol` so the handler can surface a 400
/// rather than persist a garbage row that breaks the read-side lookup. The
/// migration's CHECK constraint is a backstop for this same condition.
pub async fn allow_sender(
    pool: &PgPool,
    mailbox_id: Uuid,
    sender_address: &str,
) -> Result<bool, sqlx::Error> {
    let normalised = normalise(sender_address);
    if normalised.is_empty() || !normalised.contains('@') {
        return Err(sqlx::Error::Protocol(
            "sender_address must be a non-empty local@domain string".into(),
        ));
    }
    let res = sqlx::query(
        "INSERT INTO remote_image_allowlist (mailbox_id, sender_address) \
         VALUES ($1, $2) ON CONFLICT (mailbox_id, sender_address) DO NOTHING",
    )
    .bind(mailbox_id)
    .bind(&normalised)
    .execute(pool)
    .await?;
    Ok(res.rows_affected() > 0)
}

/// PURPOSE: Remove a per-sender allowlist row. Used by the future settings
/// page's per-row "Remove" button (P2 follow-up). Returns `true` when a row
/// was actually deleted so the handler can render a "Removed" banner only
/// when the click did something.
#[allow(dead_code)] // surfaces in the settings page that lands in a follow-up task.
pub async fn delete_sender(
    pool: &PgPool,
    mailbox_id: Uuid,
    sender_address: &str,
) -> Result<bool, sqlx::Error> {
    let normalised = normalise(sender_address);
    if normalised.is_empty() {
        return Ok(false);
    }
    let res = sqlx::query(
        "DELETE FROM remote_image_allowlist \
         WHERE mailbox_id = $1 AND sender_address = $2",
    )
    .bind(mailbox_id)
    .bind(&normalised)
    .execute(pool)
    .await?;
    Ok(res.rows_affected() > 0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalise_trims_and_lowercases() {
        assert_eq!(normalise("  Alice@Example.COM  "), "alice@example.com");
        assert_eq!(normalise("BOB@EXAMPLE.com"), "bob@example.com");
        assert_eq!(normalise("plain@host.tld"), "plain@host.tld");
        assert_eq!(normalise(""), "");
        assert_eq!(normalise("   "), "");
    }

    #[test]
    fn normalise_does_not_strip_plus_addressing() {
        // `+foo` subaddressing is a meaningful local-part suffix on most
        // providers (Gmail/Outlook/Zoho all route it). Keep it verbatim so
        // an allowlist row for `alice+newsletter@example.com` doesn't bleed
        // into a different row for `alice@example.com`.
        assert_eq!(
            normalise("Alice+Newsletter@example.com"),
            "alice+newsletter@example.com"
        );
    }
}
