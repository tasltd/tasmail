// Added (TMAIL-374): SQLx model for the `classic_signup_drafts` table.
//
// Server-side in-progress state for the `/classic/signup` wizard. One row per
// browser that started a signup; the row id is the secret half of the
// `tasmail_classic_signup_draft` cookie. See migration 082 for the schema
// rationale.
//
// Lifecycle:
//   * `create()` — Step 1 (GET /classic/signup) creates a draft when the user
//     first lands without a cookie. The freshly-generated CSRF token is bound
//     to both the row and the rendered form.
//   * `attach_mailbox()` — Step 1 (POST /classic/signup) sets `mailbox_id`
//     once the Mailbox row has been inserted, and advances `current_step` to
//     `"servers"`.
//   * `mark_done()` — Step 3 (POST /classic/signup/done) advances
//     `current_step` to `"done"` right before the handler issues the real
//     classic_sessions cookie and redirects to the inbox.
//   * `delete()` — Step 3 (success) deletes the row after the session is
//     established so the cookie can be cleared without leaving a dangling
//     row.
//   * `cleanup_expired()` — background sweep (same scheduler tick that prunes
//     classic_sessions + pending_2fa_tokens).

use chrono::{DateTime, Duration, Utc};
use serde::{Deserialize, Serialize};
use sqlx::FromRow;
use uuid::Uuid;

/// Draft lifetime. FIXED (NOT sliding) so a half-completed wizard can't be
/// resumed days later — credentials drift, providers rotate app passwords,
/// etc. A user who walks away just starts the wizard over.
pub const SIGNUP_DRAFT_TTL_SECS: i64 = 1800;

/// Wizard step. Stored as TEXT with a CHECK constraint in the DB (see
/// migration 082) so a future fourth step is one CHECK change away rather
/// than a Postgres ENUM migration.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum SignupDraftStep {
    Account,
    Servers,
    Done,
}

impl SignupDraftStep {
    #[allow(dead_code)] // Used by tests and serialisation; the handler doesn't write this column.
    pub fn as_str(&self) -> &'static str {
        match self {
            SignupDraftStep::Account => "account",
            SignupDraftStep::Servers => "servers",
            SignupDraftStep::Done => "done",
        }
    }

    pub fn from_db_str(s: &str) -> Option<Self> {
        match s {
            "account" => Some(SignupDraftStep::Account),
            "servers" => Some(SignupDraftStep::Servers),
            "done" => Some(SignupDraftStep::Done),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct ClassicSignupDraft {
    pub id: Uuid,
    pub mailbox_id: Option<Uuid>,
    pub current_step: String,
    pub csrf_token: String,
    pub created_at: DateTime<Utc>,
    pub expires_at: DateTime<Utc>,
    pub last_seen_at: DateTime<Utc>,
    pub last_seen_ip: Option<String>,
    pub last_seen_ua: Option<String>,
}

impl ClassicSignupDraft {
    /// Insert a fresh draft at Step 1 ("account"). The caller generates the
    /// CSRF token (same shape as ClassicSession::create) so the rendered form
    /// and the persisted row carry identical bytes.
    pub async fn create(
        pool: &sqlx::PgPool,
        csrf_token: &str,
        ip: Option<&str>,
        ua: Option<&str>,
    ) -> Result<ClassicSignupDraft, sqlx::Error> {
        let expires_at = Utc::now() + Duration::seconds(SIGNUP_DRAFT_TTL_SECS);
        sqlx::query_as::<_, ClassicSignupDraft>(
            "INSERT INTO classic_signup_drafts
                (id, mailbox_id, current_step, csrf_token, created_at, expires_at,
                 last_seen_at, last_seen_ip, last_seen_ua)
             VALUES (gen_random_uuid(), NULL, 'account', $1, NOW(), $2, NOW(), $3, $4)
             RETURNING *",
        )
        .bind(csrf_token)
        .bind(expires_at)
        .bind(ip)
        .bind(ua)
        .fetch_one(pool)
        .await
    }

    /// Look up a draft by id. Filters out expired rows; the row stays for the
    /// cleanup sweep to reap. Same shape as PendingTwoFactorToken::find_active.
    pub async fn find_active(
        pool: &sqlx::PgPool,
        id: Uuid,
    ) -> Result<Option<ClassicSignupDraft>, sqlx::Error> {
        sqlx::query_as::<_, ClassicSignupDraft>(
            "SELECT * FROM classic_signup_drafts WHERE id = $1 AND expires_at > NOW()",
        )
        .bind(id)
        .fetch_optional(pool)
        .await
    }

    /// Step 1 transition: attach the freshly-created mailbox + advance to Step 2.
    /// Bumps `last_seen_at` so the audit timestamp on the row tracks the user's
    /// real progress (not just creation).
    pub async fn attach_mailbox(
        pool: &sqlx::PgPool,
        id: Uuid,
        mailbox_id: Uuid,
    ) -> Result<Option<ClassicSignupDraft>, sqlx::Error> {
        sqlx::query_as::<_, ClassicSignupDraft>(
            "UPDATE classic_signup_drafts
                SET mailbox_id = $1, current_step = 'servers', last_seen_at = NOW()
              WHERE id = $2
              RETURNING *",
        )
        .bind(mailbox_id)
        .bind(id)
        .fetch_optional(pool)
        .await
    }

    /// Step 2 transition: advance to Step 3 once IMAP + SMTP rows are saved.
    pub async fn mark_servers_done(
        pool: &sqlx::PgPool,
        id: Uuid,
    ) -> Result<Option<ClassicSignupDraft>, sqlx::Error> {
        sqlx::query_as::<_, ClassicSignupDraft>(
            "UPDATE classic_signup_drafts
                SET current_step = 'done', last_seen_at = NOW()
              WHERE id = $1
              RETURNING *",
        )
        .bind(id)
        .fetch_optional(pool)
        .await
    }

    /// Bump audit fields on any GET that re-renders an existing draft. Best-
    /// effort — the handler ignores errors so a transient DB hiccup doesn't
    /// surface an error page when the underlying flow is fine.
    pub async fn touch(
        pool: &sqlx::PgPool,
        id: Uuid,
        ip: Option<&str>,
        ua: Option<&str>,
    ) -> Result<(), sqlx::Error> {
        sqlx::query(
            "UPDATE classic_signup_drafts
                SET last_seen_at = NOW(), last_seen_ip = $2, last_seen_ua = $3
              WHERE id = $1",
        )
        .bind(id)
        .bind(ip)
        .bind(ua)
        .execute(pool)
        .await?;
        Ok(())
    }

    /// Delete after the wizard graduates into a real session. Also called by
    /// the handler when it detects a stale draft and needs to reset.
    pub async fn delete(pool: &sqlx::PgPool, id: Uuid) -> Result<bool, sqlx::Error> {
        let result = sqlx::query("DELETE FROM classic_signup_drafts WHERE id = $1")
            .bind(id)
            .execute(pool)
            .await?;
        Ok(result.rows_affected() > 0)
    }

    /// Background sweep — call from the same scheduler tick that prunes
    /// `classic_sessions` and `pending_2fa_tokens`.
    #[allow(dead_code)]
    pub async fn cleanup_expired(pool: &sqlx::PgPool) -> Result<u64, sqlx::Error> {
        let result = sqlx::query("DELETE FROM classic_signup_drafts WHERE expires_at < NOW()")
            .execute(pool)
            .await?;
        Ok(result.rows_affected())
    }

    /// Convenience: the typed step. Falls back to `Account` on an unknown
    /// db string so a stray manual UPDATE can't crash the handler — the
    /// user just resumes at Step 1 with their draft data intact.
    pub fn step(&self) -> SignupDraftStep {
        SignupDraftStep::from_db_str(&self.current_step).unwrap_or(SignupDraftStep::Account)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ttl_is_thirty_minutes() {
        // 30 mins is generous enough to type credentials + go look up an
        // app password in another tab without expiring the draft, but short
        // enough that a stolen cookie doesn't unlock weeks-old drafts.
        assert_eq!(SIGNUP_DRAFT_TTL_SECS, 1800);
    }

    #[test]
    fn step_roundtrip_strings() {
        for s in [SignupDraftStep::Account, SignupDraftStep::Servers, SignupDraftStep::Done] {
            assert_eq!(SignupDraftStep::from_db_str(s.as_str()), Some(s));
        }
    }

    #[test]
    fn step_from_unknown_string_returns_none() {
        assert_eq!(SignupDraftStep::from_db_str("bogus"), None);
        assert_eq!(SignupDraftStep::from_db_str(""), None);
        assert_eq!(SignupDraftStep::from_db_str("ACCOUNT"), None);
    }
}
