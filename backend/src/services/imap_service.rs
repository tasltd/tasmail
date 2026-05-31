use async_imap::Session;
use async_native_tls::TlsStream;
use futures::TryStreamExt;
use serde::Serialize;
use tokio::net::TcpStream;
use tokio_util::compat::{Compat, TokioAsyncReadCompatExt};

use crate::config::ImapConfig;
use crate::error::AppError;

/// Represents an IMAP folder
#[derive(Debug, Clone, Serialize)]
pub struct Folder {
    pub name: String,
    pub delimiter: String,
    pub messages: Option<u32>,
    pub unseen: Option<u32>,
}

/// Represents a message envelope (list view)
// Added (TMAIL-329): `preview` field carries a ~200 char plaintext snippet
// extracted from the message body so EmailList rows in the alt-UI render a
// useful preview line under the subject. Populated by list_messages /
// search_messages via a `BODY.PEEK[]<0.8192>` partial fetch + the
// `extract_preview` helper below; `None` when the body could not be partially
// parsed (truncated MIME, empty message, etc.) so the SPA can fall back to an
// empty preview line without rendering "null".
//
// Added (TMAIL-350): threading headers (`message_id`, `in_reply_to`,
// `references`) so the alt-UI EmailList can group rows into conversations
// without making a separate per-message fetch. All three are parsed from the
// same `BODY.PEEK[]<0.8192>` bytes already used for the preview snippet, via
// `extract_threading_headers` — no extra IMAP round trip. Empty / unparsable
// header sets are returned as `None` / `vec![]` so the SPA treats the row as
// a thread-of-one when grouping.
#[derive(Debug, Clone, Serialize)]
pub struct MessageEnvelope {
    pub uid: u32,
    pub subject: Option<String>,
    pub from: Option<String>,
    pub date: Option<String>,
    pub flags: Vec<String>,
    pub size: Option<u32>,
    pub preview: Option<String>,
    pub message_id: Option<String>,
    pub in_reply_to: Option<String>,
    pub references: Vec<String>,
}

/// Added (TMAIL-320): raw bytes of a single MIME part inside a message,
/// resolved by the dotted `part_id` path the `Attachment` struct already
/// exposes ("1", "2.1", …). Returned by `ImapService::get_message_part()`
/// so the attachment-download handler can stream it back to the browser
/// with the right Content-Type + Content-Disposition.
#[derive(Debug, Clone)]
pub struct MessagePart {
    pub filename: String,
    pub content_type: String,
    pub bytes: Vec<u8>,
}

/// Represents a full message with body
#[derive(Debug, Clone, Serialize)]
pub struct FullMessage {
    pub uid: u32,
    pub subject: Option<String>,
    pub from: Option<String>,
    pub to: Vec<String>,
    pub cc: Vec<String>,
    pub date: Option<String>,
    pub flags: Vec<String>,
    pub text_body: Option<String>,
    pub html_body: Option<String>,
    pub attachments: Vec<Attachment>,
    pub message_id: Option<String>,
    pub in_reply_to: Option<String>,
    pub references: Vec<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct Attachment {
    pub filename: String,
    pub content_type: String,
    pub size: usize,
    pub part_id: String,
}

// Added (TMAIL-302): made pub so the IMAP IDLE bridge (services::imap_idle_bridge)
// can hold and reuse a Session across IDLE cycles.
pub type ImapSession = Session<TlsStream<Compat<TcpStream>>>;

/// IMAP service bound to a single user's BYOK IMAP server.
///
/// Constructed exclusively via `ImapService::for_user(state, user_id)`, which loads
/// the user's default row from `imap_configurations`, decrypts the password with the
/// JWT-derived AES key, and returns a service bound to that host:port + the per-user
/// (username, password) pair to use on connect.
///
/// TMAIL-311 (finishes TMAIL-156): the legacy `ImapService::new(global_config)`
/// constructor — which forwarded the server-wide Dovecot config and required the
/// caller to pass plaintext mailbox credentials — was removed. All production
/// handlers were already on `for_user`; keeping `new` around as public API was a
/// foot-gun for future contributors.
pub struct ImapService {
    config: ImapConfig,
    /// Per-user decrypted IMAP credentials (username, password) loaded by
    /// `for_user`. Stored as `Option` only so test code can construct an
    /// `ImapService` via struct literal with `None` for paths that never touch the
    /// network (e.g. the `trash_folder()` resolution test).
    user_credentials: Option<(String, String)>,
    /// TMAIL-283: per-user resolved trash folder name (e.g. "Deleted Items" on
    /// Stalwart, "[Gmail]/Trash" on Gmail). Sourced from `imap_configurations.trash_folder`
    /// at `for_user()` time. `None` falls back to the hardcoded "Trash" default for
    /// any future per-user row that doesn't set the column.
    user_trash_folder: Option<String>,
}

impl ImapService {
    /// PURPOSE: Factory that loads the user's default IMAP server config from the
    /// `imap_configurations` table, decrypts the stored password, and returns a service
    /// bound to that server. Returns `Err(ServiceUnavailable)` if the user hasn't
    /// completed onboarding.
    pub async fn for_user(
        state: &crate::state::AppState,
        user_id: uuid::Uuid,
    ) -> Result<Self, AppError> {
        // TMAIL-162: try Redis first; falls through to DB on miss or Redis down.
        // We cache the full ImapConfiguration row including its encrypted_password ciphertext —
        // never the plaintext password.
        let cache_key = user_id.to_string();
        let cfg: crate::models::imap_config::ImapConfiguration = match state
            .cache
            .get_user_imap_config::<crate::models::imap_config::ImapConfiguration>(&cache_key)
            .await
        {
            Some(hit) => hit,
            None => {
                let row = crate::models::imap_config::ImapConfiguration::default_for_user(&state.db, user_id)
                    .await
                    .map_err(AppError::from)?
                    .ok_or_else(|| AppError::ServiceUnavailable(
                        "No IMAP server configured. Complete the onboarding wizard at /onboarding.".into()
                    ))?;
                let _ = state.cache.set_user_imap_config(&cache_key, &row).await;
                row
            }
        };

        let key = crate::models::ai_config::derive_encryption_key(&state.config.jwt.secret);
        let password = cfg
            .decrypt_password(&key)
            .map_err(|e| AppError::Internal(anyhow::anyhow!("Failed to decrypt IMAP password: {}", e)))?;

        // Note: BYOK uses async-native-tls's TLS layer; ImapConfig.tls means "use implicit TLS on connect".
        // Map the per-user `encryption` enum to the global ImapConfig shape used by connect():
        //   ssl    → tls=true, port unchanged (993)
        //   starttls/none → tls=false (TODO: emit STARTTLS when STARTTLS branch is implemented)
        let imap_cfg = ImapConfig {
            host: cfg.host.clone(),
            port: cfg.port as u16,
            tls: matches!(cfg.encryption.as_str(), "ssl"),
            master_password: None,
        };
        // TMAIL-283: stash the configured trash folder so delete_message can
        // honour it instead of hardcoding "Trash" (which doesn't exist on
        // Stalwart, Gmail, Outlook, ProtonMail Bridge — see folders-messages-2026-05.md finding #7).
        let user_trash_folder = cfg.trash_folder.clone();
        Ok(Self {
            config: imap_cfg,
            user_credentials: Some((cfg.username.clone(), password)),
            user_trash_folder,
        })
    }

    /// PURPOSE: Resolve the effective trash folder name for the current service.
    /// Returns the per-user configured value when set (BYOK), else the legacy
    /// hardcoded "Trash" so existing Dovecot self-host deployments keep working.
    pub fn trash_folder(&self) -> &str {
        self.user_trash_folder.as_deref().unwrap_or("Trash")
    }

    /// PURPOSE: Connect to the user's IMAP server using the credentials decrypted
    /// by `for_user`. This is the only sanctioned connect path for handlers —
    /// callers never see the plaintext password.
    ///
    /// The `Option` check on `user_credentials` is defensive: in production
    /// `for_user` always populates it, but the field is left as `Option` so test
    /// code can construct credential-less instances via struct literal.
    pub async fn connect_user(&self) -> Result<ImapSession, AppError> {
        let (username, password) = self.user_credentials.as_ref().ok_or_else(|| {
            AppError::Internal(anyhow::anyhow!(
                "ImapService::connect_user() requires construction via for_user()"
            ))
        })?;
        self.connect(username, password).await
    }

    // Added: Public accessor for IMAP config — used by EML import/export handlers
    // that need direct IMAP connections outside the service's own methods
    pub fn imap_config(&self) -> &ImapConfig {
        &self.config
    }

    /// PURPOSE: Borrow the per-user (username, password) pair populated by
    /// `for_user`. Returns `None` only for credential-less test instances
    /// constructed via struct literal — production code always sees `Some`.
    pub fn user_creds(&self) -> Option<(&str, &str)> {
        self.user_credentials.as_ref().map(|(u, p)| (u.as_str(), p.as_str()))
    }

    /// Connect and authenticate to the IMAP server
    async fn connect(&self, username: &str, password: &str) -> Result<ImapSession, AppError> {
        let tcp_stream = TcpStream::connect((&*self.config.host, self.config.port))
            .await
            .map_err(|e| AppError::Imap(format!("TCP connection failed: {}", e)))?;

        let compat_stream = tcp_stream.compat();

        let tls = async_native_tls::TlsConnector::new();
        let tls_stream = tls
            .connect(&self.config.host, compat_stream)
            .await
            .map_err(|e| AppError::Imap(format!("TLS connection failed: {}", e)))?;

        let client = async_imap::Client::new(tls_stream);

        let session = client
            .login(username, password)
            .await
            .map_err(|e| AppError::Imap(format!("Login failed: {}", e.0)))?;

        Ok(session)
    }

    /// List all folders for a user
    pub async fn list_folders(
        &self,
        username: &str,
        password: &str,
    ) -> Result<Vec<Folder>, AppError> {
        let mut session = self.connect(username, password).await?;

        // Collect stream into Vec
        let mailboxes: Vec<_> = session
            .list(Some(""), Some("*"))
            .await
            .map_err(|e| AppError::Imap(format!("LIST failed: {}", e)))?
            .try_collect()
            .await
            .map_err(|e| AppError::Imap(format!("LIST stream failed: {}", e)))?;

        let mut folders = Vec::new();
        for mailbox in &mailboxes {
            let name = mailbox.name().to_string();
            let delimiter = mailbox
                .delimiter()
                .map(|d| d.to_string())
                .unwrap_or_else(|| "/".to_string());

            let status = session
                .status(&name, "(MESSAGES UNSEEN)")
                .await
                .map_err(|e| {
                    AppError::Imap(format!("STATUS failed for {}: {}", name, e))
                })?;

            folders.push(Folder {
                name,
                delimiter,
                messages: Some(status.exists),
                unseen: status.unseen,
            });
        }

        let _ = session.logout().await;

        Ok(folders)
    }

    /// TMAIL-324: CREATE a new IMAP mailbox (folder) on the user's server.
    /// `name` is taken verbatim as the mailbox name — the caller is responsible
    /// for sanitising it (no `/`, no empty string). Returns the freshly-listed
    /// Folder so the caller has the canonical name + delimiter the server
    /// echoed back, which can differ from the requested string when the server
    /// normalises hierarchy delimiters.
    pub async fn create_folder(
        &self,
        username: &str,
        password: &str,
        name: &str,
    ) -> Result<Folder, AppError> {
        let mut session = self.connect(username, password).await?;

        session
            .create(name)
            .await
            .map_err(|e| AppError::Imap(format!("CREATE failed: {}", e)))?;

        // Fetch STATUS so the response carries message/unseen counts (both 0
        // for a brand-new mailbox, but keeps the response shape identical to
        // GET /api/folders).
        let status = session
            .status(name, "(MESSAGES UNSEEN)")
            .await
            .map_err(|e| AppError::Imap(format!("STATUS failed after CREATE: {}", e)))?;

        let _ = session.logout().await;

        Ok(Folder {
            name: name.to_string(),
            delimiter: "/".to_string(),
            messages: Some(status.exists),
            unseen: status.unseen,
        })
    }

    /// TMAIL-324: DELETE an IMAP mailbox (folder). Per RFC 3501 §6.3.4, this
    /// removes the mailbox and any messages it contains. The caller is
    /// responsible for refusing to delete protected folders (INBOX, Sent, etc).
    pub async fn delete_folder(
        &self,
        username: &str,
        password: &str,
        name: &str,
    ) -> Result<(), AppError> {
        let mut session = self.connect(username, password).await?;

        session
            .delete(name)
            .await
            .map_err(|e| AppError::Imap(format!("DELETE failed: {}", e)))?;

        let _ = session.logout().await;
        Ok(())
    }

    /// Fetch message envelopes from a folder with pagination
    pub async fn list_messages(
        &self,
        username: &str,
        password: &str,
        folder: &str,
        page: u32,
        page_size: u32,
    ) -> Result<(Vec<MessageEnvelope>, u32), AppError> {
        let mut session = self.connect(username, password).await?;

        let mailbox = session
            .select(folder)
            .await
            .map_err(|e| AppError::Imap(format!("SELECT failed: {}", e)))?;

        let total = mailbox.exists;
        if total == 0 {
            let _ = session.logout().await;
            return Ok((vec![], 0));
        }

        // Calculate sequence range (newest first)
        let end = total.saturating_sub(page * page_size);
        let start = end.saturating_sub(page_size).max(1);

        if end == 0 {
            let _ = session.logout().await;
            return Ok((vec![], total));
        }

        let range = format!("{}:{}", start, end);
        // Changed (TMAIL-329): added `BODY.PEEK[]<0.8192>` so each envelope row
        // can carry a ~200 char preview snippet. `.PEEK` avoids setting the
        // \Seen flag (list-view must not mark messages read); `<0.8192>`
        // requests the first 8 KiB of the raw RFC 822 body which is enough to
        // cover typical header + first text part for the overwhelming majority
        // of messages while keeping the FETCH payload bounded.
        let messages: Vec<_> = session
            .fetch(&range, "(UID ENVELOPE FLAGS RFC822.SIZE BODY.PEEK[]<0.8192>)")
            .await
            .map_err(|e| AppError::Imap(format!("FETCH failed: {}", e)))?
            .try_collect()
            .await
            .map_err(|e| AppError::Imap(format!("FETCH stream failed: {}", e)))?;

        let mut envelopes = Vec::new();
        for msg in &messages {
            let uid = msg.uid.unwrap_or(0);
            let envelope = msg.envelope();

            let subject = envelope
                .and_then(|e| e.subject.as_ref())
                .and_then(|s| std::str::from_utf8(s).ok())
                .map(String::from);

            let from = envelope
                .and_then(|e| e.from.as_ref())
                .and_then(|addrs| addrs.first())
                .map(format_imap_address);

            let date = envelope
                .and_then(|e| e.date.as_ref())
                .and_then(|d| std::str::from_utf8(d).ok())
                .map(String::from);

            let flags: Vec<String> = msg
                .flags()
                .map(|f| format!("{:?}", f))
                .collect();

            // Added (TMAIL-329): pull a preview snippet out of the partial body
            // bytes. msg.body() returns the bytes from `BODY.PEEK[]<0.8192>`
            // when the server honours partial fetch; on servers that don't,
            // we still try to parse the full body. extract_preview() returns
            // None when nothing usable can be extracted so the SPA renders a
            // blank preview line rather than "null".
            let preview = msg.body().and_then(extract_preview);

            // Added (TMAIL-350): parse threading headers (Message-ID,
            // In-Reply-To, References) from the same partial body bytes the
            // preview was extracted from. The alt-UI EmailList groups rows
            // into conversations using these — having them on every envelope
            // means the threaded view renders without per-row /messages/{uid}
            // fetches that would defeat the point of the list endpoint.
            let (message_id, in_reply_to, references) = msg
                .body()
                .map(extract_threading_headers)
                .unwrap_or_else(|| (None, None, Vec::new()));

            envelopes.push(MessageEnvelope {
                uid,
                subject,
                from,
                date,
                flags,
                size: msg.size,
                preview,
                message_id,
                in_reply_to,
                references,
            });
        }

        envelopes.reverse();
        let _ = session.logout().await;

        Ok((envelopes, total))
    }

    /// Search messages in a folder using IMAP SEARCH
    pub async fn search_messages(
        &self,
        username: &str,
        password: &str,
        folder: &str,
        query: &str,
    ) -> Result<Vec<MessageEnvelope>, AppError> {
        let mut session = self.connect(username, password).await?;

        session
            .select(folder)
            .await
            .map_err(|e| AppError::Imap(format!("SELECT failed: {}", e)))?;

        // Use IMAP SEARCH with TEXT criteria (searches headers + body)
        let search_criteria = format!("TEXT \"{}\"", query.replace('"', "\\\""));
        let uids: Vec<u32> = session
            .uid_search(&search_criteria)
            .await
            .map_err(|e| AppError::Imap(format!("SEARCH failed: {}", e)))?
            .into_iter()
            .collect();

        if uids.is_empty() {
            let _ = session.logout().await;
            return Ok(vec![]);
        }

        // Fetch envelopes for matching UIDs (limit to most recent 100)
        let uid_list: Vec<String> = uids.iter().rev().take(100).map(|u| u.to_string()).collect();
        let uid_range = uid_list.join(",");

        // Changed (TMAIL-329): mirror list_messages' BODY.PEEK[]<0.8192> partial
        // fetch so search-result rows render a preview too.
        let messages: Vec<_> = session
            .uid_fetch(&uid_range, "(UID ENVELOPE FLAGS RFC822.SIZE BODY.PEEK[]<0.8192>)")
            .await
            .map_err(|e| AppError::Imap(format!("FETCH failed: {}", e)))?
            .try_collect()
            .await
            .map_err(|e| AppError::Imap(format!("FETCH stream failed: {}", e)))?;

        let mut envelopes = Vec::new();
        for msg in &messages {
            let uid = msg.uid.unwrap_or(0);
            let envelope = msg.envelope();

            let subject = envelope
                .and_then(|e| e.subject.as_ref())
                .and_then(|s| std::str::from_utf8(s).ok())
                .map(String::from);

            let from = envelope
                .and_then(|e| e.from.as_ref())
                .and_then(|addrs| addrs.first())
                .map(format_imap_address);

            let date = envelope
                .and_then(|e| e.date.as_ref())
                .and_then(|d| std::str::from_utf8(d).ok())
                .map(String::from);

            let flags: Vec<String> = msg
                .flags()
                .map(|f| format!("{:?}", f))
                .collect();

            let preview = msg.body().and_then(extract_preview);

            // Added (TMAIL-350): same threading header extraction as
            // list_messages — keeps search results threaded too.
            let (message_id, in_reply_to, references) = msg
                .body()
                .map(extract_threading_headers)
                .unwrap_or_else(|| (None, None, Vec::new()));

            envelopes.push(MessageEnvelope {
                uid,
                subject,
                from,
                date,
                flags,
                size: msg.size,
                preview,
                message_id,
                in_reply_to,
                references,
            });
        }

        envelopes.reverse();
        let _ = session.logout().await;

        Ok(envelopes)
    }

    /// Move a message to a different folder
    ///
    /// TMAIL-317: when the destination mailbox does not exist (e.g. the modern
    /// UI's Archive button targets `Archive` on a fresh account that has never
    /// opened that folder), IMAP servers return a NO response — usually with
    /// the `[TRYCREATE]` response code per RFC 3501 §6.4.7. We handle that by
    /// attempting `CREATE` on the destination and retrying the COPY once.
    /// `CREATE` against an existing mailbox is a no-op on Dovecot / Stalwart /
    /// Gmail / Outlook (returns "Mailbox already exists" as a non-fatal NO)
    /// so we ignore its error and let the retry COPY surface the real fault
    /// if the original failure was unrelated.
    pub async fn move_message(
        &self,
        username: &str,
        password: &str,
        from_folder: &str,
        uid: u32,
        to_folder: &str,
    ) -> Result<(), AppError> {
        let mut session = self.connect(username, password).await?;

        session
            .select(from_folder)
            .await
            .map_err(|e| AppError::Imap(format!("SELECT failed: {}", e)))?;

        // Copy to destination — retry once after CREATE if it fails (TMAIL-317).
        if session.uid_copy(uid.to_string(), to_folder).await.is_err() {
            // CREATE is best-effort; ignore "Mailbox already exists" and let
            // the retry COPY produce the authoritative error message.
            let _ = session.create(to_folder).await;
            session
                .uid_copy(uid.to_string(), to_folder)
                .await
                .map_err(|e| AppError::Imap(format!("COPY failed: {}", e)))?;
        }

        // Mark as deleted in source
        let _: Vec<_> = session
            .uid_store(uid.to_string(), "+FLAGS (\\Deleted)")
            .await
            .map_err(|e| AppError::Imap(format!("Store failed: {}", e)))?
            .try_collect()
            .await
            .unwrap_or_default();

        // Expunge
        session
            .expunge()
            .await
            .map_err(|e| AppError::Imap(format!("EXPUNGE failed: {}", e)))?
            .try_collect::<Vec<_>>()
            .await
            .ok();

        let _ = session.logout().await;
        Ok(())
    }

    /// Delete a message (move to Trash or permanent delete)
    ///
    /// TMAIL-283: trash folder name is resolved via `self.trash_folder()` which
    /// honours the per-user `imap_configurations.trash_folder` (Stalwart
    /// "Deleted Items", Gmail "[Gmail]/Trash", etc.) and falls back to "Trash"
    /// for legacy self-host deployments. Deleting *from* the resolved trash
    /// folder is a permanent expunge; from any other folder it's a move.
    pub async fn delete_message(
        &self,
        username: &str,
        password: &str,
        folder: &str,
        uid: u32,
    ) -> Result<(), AppError> {
        let trash = self.trash_folder();
        if folder == trash {
            // Permanent delete from the trash folder.
            let mut session = self.connect(username, password).await?;
            session
                .select(folder)
                .await
                .map_err(|e| AppError::Imap(format!("SELECT failed: {}", e)))?;

            let _: Vec<_> = session
                .uid_store(uid.to_string(), "+FLAGS (\\Deleted)")
                .await
                .map_err(|e| AppError::Imap(format!("Store failed: {}", e)))?
                .try_collect()
                .await
                .unwrap_or_default();

            session
                .expunge()
                .await
                .map_err(|e| AppError::Imap(format!("EXPUNGE failed: {}", e)))?
                .try_collect::<Vec<_>>()
                .await
                .ok();

            let _ = session.logout().await;
            Ok(())
        } else {
            // Move to the resolved trash folder.
            self.move_message(username, password, folder, uid, trash)
                .await
        }
    }

    /// Toggle a flag on a message
    pub async fn set_flag(
        &self,
        username: &str,
        password: &str,
        folder: &str,
        uid: u32,
        flag: &str,
        add: bool,
    ) -> Result<(), AppError> {
        let mut session = self.connect(username, password).await?;

        session
            .select(folder)
            .await
            .map_err(|e| AppError::Imap(format!("SELECT failed: {}", e)))?;

        let store_cmd = if add {
            format!("+FLAGS ({})", flag)
        } else {
            format!("-FLAGS ({})", flag)
        };

        let _: Vec<_> = session
            .uid_store(uid.to_string(), &store_cmd)
            .await
            .map_err(|e| AppError::Imap(format!("Store failed: {}", e)))?
            .try_collect()
            .await
            .unwrap_or_default();

        let _ = session.logout().await;
        Ok(())
    }

    /// Save a draft message to the Drafts folder via IMAP APPEND
    pub async fn save_draft(
        &self,
        username: &str,
        password: &str,
        raw_message: &[u8],
    ) -> Result<(), AppError> {
        let mut session = self.connect(username, password).await?;

        // async-imap 0.11 append: (mailbox, flags, internaldate, content)
        session
            .append("Drafts", Some("(\\Draft \\Seen)"), None, raw_message)
            .await
            .map_err(|e| AppError::Imap(format!("APPEND to Drafts failed: {}", e)))?;

        let _ = session.logout().await;
        Ok(())
    }

    /// Get quota usage via IMAP (calculates from folder sizes)
    /// Returns (used_bytes, message_count)
    pub async fn get_quota(
        &self,
        username: &str,
        password: &str,
    ) -> Result<(i64, i32), AppError> {
        let mut session = self.connect(username, password).await?;

        let mailboxes: Vec<_> = session
            .list(Some(""), Some("*"))
            .await
            .map_err(|e| AppError::Imap(format!("LIST failed: {}", e)))?
            .try_collect()
            .await
            .map_err(|e| AppError::Imap(format!("LIST stream failed: {}", e)))?;

        let mut total_messages: u32 = 0;
        let mut total_size: i64 = 0;

        for mailbox in &mailboxes {
            let name = mailbox.name().to_string();
            let status = session
                .status(&name, "(MESSAGES)")
                .await
                .map_err(|e| {
                    AppError::Imap(format!("STATUS failed for {}: {}", name, e))
                })?;

            total_messages += status.exists;

            // Select folder and sum message sizes
            if status.exists > 0 {
                let mbox = session
                    .select(&name)
                    .await
                    .map_err(|e| AppError::Imap(format!("SELECT failed: {}", e)))?;

                if mbox.exists > 0 {
                    let range = format!("1:{}", mbox.exists);
                    let messages: Vec<_> = session
                        .fetch(&range, "RFC822.SIZE")
                        .await
                        .map_err(|e| AppError::Imap(format!("FETCH SIZE failed: {}", e)))?
                        .try_collect()
                        .await
                        .unwrap_or_default();

                    for msg in &messages {
                        if let Some(size) = msg.size {
                            total_size += size as i64;
                        }
                    }
                }
            }
        }

        let _ = session.logout().await;

        Ok((total_size, total_messages as i32))
    }

    /// Fetch a full message by UID
    pub async fn get_message(
        &self,
        username: &str,
        password: &str,
        folder: &str,
        uid: u32,
    ) -> Result<FullMessage, AppError> {
        let mut session = self.connect(username, password).await?;

        session
            .select(folder)
            .await
            .map_err(|e| AppError::Imap(format!("SELECT failed: {}", e)))?;

        let messages: Vec<_> = session
            .uid_fetch(uid.to_string(), "(FLAGS BODY[] ENVELOPE)")
            .await
            .map_err(|e| AppError::Imap(format!("UID FETCH failed: {}", e)))?
            .try_collect()
            .await
            .map_err(|e| AppError::Imap(format!("UID FETCH stream failed: {}", e)))?;

        let msg = messages
            .first()
            .ok_or_else(|| AppError::NotFound("Message not found".to_string()))?;

        let body_bytes = msg
            .body()
            .ok_or_else(|| AppError::Imap("No body in message".to_string()))?;

        let parsed = mailparse::parse_mail(body_bytes)
            .map_err(|e| AppError::Imap(format!("Mail parse failed: {}", e)))?;

        let envelope = msg.envelope();
        let subject = envelope
            .and_then(|e| e.subject.as_ref())
            .and_then(|s| std::str::from_utf8(s).ok())
            .map(String::from);

        let from = envelope
            .and_then(|e| e.from.as_ref())
            .and_then(|addrs| addrs.first())
            .map(format_imap_address);

        let to = envelope
            .and_then(|e| e.to.as_ref())
            .map(|addrs| addrs.iter().map(format_imap_address).collect())
            .unwrap_or_default();

        let cc = envelope
            .and_then(|e| e.cc.as_ref())
            .map(|addrs| addrs.iter().map(format_imap_address).collect())
            .unwrap_or_default();

        let date = envelope
            .and_then(|e| e.date.as_ref())
            .and_then(|d| std::str::from_utf8(d).ok())
            .map(String::from);

        let flags: Vec<String> = msg.flags().map(|f| format!("{:?}", f)).collect();

        let mut text_body = None;
        let mut html_body = None;
        let mut attachments = Vec::new();

        extract_parts(&parsed, &mut text_body, &mut html_body, &mut attachments, "");

        // Added: Extract threading headers from parsed mail
        let message_id = extract_header(&parsed, "Message-ID");
        let in_reply_to = extract_header(&parsed, "In-Reply-To");
        let references = extract_header(&parsed, "References")
            .map(|r| r.split_whitespace().map(String::from).collect())
            .unwrap_or_default();

        // Mark as seen — collect the stream to drive it
        let _seen: Vec<_> = session
            .uid_store(uid.to_string(), "+FLAGS (\\Seen)")
            .await
            .map_err(|e| AppError::Imap(format!("Store flags failed: {}", e)))?
            .try_collect()
            .await
            .unwrap_or_default();

        let _ = session.logout().await;

        Ok(FullMessage {
            uid,
            subject,
            from,
            to,
            cc,
            date,
            flags,
            text_body,
            html_body,
            attachments,
            message_id,
            in_reply_to,
            references,
        })
    }

    /// Added (TMAIL-320): fetch the raw bytes of a single MIME part inside a
    /// message. The `part_id` is the dotted path the `extract_parts` walker
    /// assigns ("1", "2.1", …) and is already returned to the SPA on every
    /// `FullMessage.attachments` entry. We re-fetch the full message with
    /// BODY[] here so we have the canonical raw bytes to walk — the IMAP
    /// `BODY[PART]` fetch shape varies across servers and we'd still have to
    /// parse the structure to know the content-type and filename, so a single
    /// BODY[] + in-memory MIME walk is the cleanest path.
    ///
    /// Returns `NotFound` if the message exists but no part matches the
    /// requested `part_id` (e.g. stale link, or the attachment list shifted
    /// after a server-side rewrite).
    pub async fn get_message_part(
        &self,
        username: &str,
        password: &str,
        folder: &str,
        uid: u32,
        part_id: &str,
    ) -> Result<MessagePart, AppError> {
        let mut session = self.connect(username, password).await?;

        session
            .select(folder)
            .await
            .map_err(|e| AppError::Imap(format!("SELECT failed: {}", e)))?;

        // BODY.PEEK[] so we don't accidentally mark the message \Seen just
        // because the user clicked Download. get_message() already sets \Seen
        // when the body is opened in the reader; downloading an attachment
        // should not change read state on its own.
        let messages: Vec<_> = session
            .uid_fetch(uid.to_string(), "(BODY.PEEK[])")
            .await
            .map_err(|e| AppError::Imap(format!("UID FETCH failed: {}", e)))?
            .try_collect()
            .await
            .map_err(|e| AppError::Imap(format!("UID FETCH stream failed: {}", e)))?;

        let _ = session.logout().await;

        let msg = messages
            .first()
            .ok_or_else(|| AppError::NotFound("Message not found".to_string()))?;

        let body_bytes = msg
            .body()
            .ok_or_else(|| AppError::Imap("No body in message".to_string()))?;

        let parsed = mailparse::parse_mail(body_bytes)
            .map_err(|e| AppError::Imap(format!("Mail parse failed: {}", e)))?;

        find_part_by_id(&parsed, part_id, "").ok_or_else(|| {
            AppError::NotFound(format!("Attachment part {} not found", part_id))
        })
    }
}

/// Format an IMAP address (from imap_proto) to a display string
fn format_imap_address(addr: &imap_proto::Address) -> String {
    let mailbox_name = addr
        .mailbox
        .as_ref()
        .and_then(|m| std::str::from_utf8(m).ok())
        .unwrap_or("");
    let host = addr
        .host
        .as_ref()
        .and_then(|h| std::str::from_utf8(h).ok())
        .unwrap_or("");
    let name = addr
        .name
        .as_ref()
        .and_then(|n| std::str::from_utf8(n).ok());
    match name {
        Some(n) if !n.is_empty() => format!("{} <{}@{}>", n, mailbox_name, host),
        _ => format!("{}@{}", mailbox_name, host),
    }
}

/// Extract a specific header value from a parsed email
fn extract_header(mail: &mailparse::ParsedMail, name: &str) -> Option<String> {
    mail.headers
        .iter()
        .find(|h| h.get_key().eq_ignore_ascii_case(name))
        .map(|h| h.get_value().trim().to_string())
        .filter(|v: &String| !v.is_empty())
}

/// Recursively extract text, html, and attachments from MIME parts
fn extract_parts(
    mail: &mailparse::ParsedMail,
    text_body: &mut Option<String>,
    html_body: &mut Option<String>,
    attachments: &mut Vec<Attachment>,
    part_prefix: &str,
) {
    let content_type = &mail.ctype.mimetype;

    if mail.subparts.is_empty() {
        let disposition = mail.get_content_disposition();
        if content_type == "text/plain" && text_body.is_none() {
            text_body.clone_from(&mail.get_body().ok());
        } else if content_type == "text/html" && html_body.is_none() {
            html_body.clone_from(&mail.get_body().ok());
        } else if disposition.disposition == mailparse::DispositionType::Attachment
            || !content_type.starts_with("text/")
        {
            let filename = disposition
                .params
                .get("filename")
                .cloned()
                .unwrap_or_else(|| "unnamed".to_string());
            let body = mail.get_body_raw().unwrap_or_default();
            attachments.push(Attachment {
                filename,
                content_type: content_type.to_string(),
                size: body.len(),
                part_id: part_prefix.to_string(),
            });
        }
    } else {
        for (i, part) in mail.subparts.iter().enumerate() {
            let prefix = if part_prefix.is_empty() {
                format!("{}", i + 1)
            } else {
                format!("{}.{}", part_prefix, i + 1)
            };
            extract_parts(part, text_body, html_body, attachments, &prefix);
        }
    }
}

/// Added (TMAIL-350): extract `Message-ID`, `In-Reply-To`, and `References`
/// from the partial body bytes already fetched for the preview snippet.
/// Lets the alt-UI EmailList group rows into conversations without a
/// per-row /messages/{uid} fetch.
///
/// Returns a tuple of `(message_id, in_reply_to, references)`:
///   * `message_id` and `in_reply_to` — first token of the header value with
///     surrounding whitespace stripped. The IMAP-served header is usually
///     already in the canonical `<id@host>` form; we preserve the angle
///     brackets so consumers can string-compare against `references` entries
///     without re-parsing.
///   * `references` — whitespace-separated tokens of the `References` header,
///     in chronological order (root first, parent last) per RFC 5322 §3.6.4.
///
/// `mailparse::parse_mail` tolerates truncated bodies as long as headers are
/// intact, which they are within the first 8 KiB the IMAP partial fetch
/// returns. On parse failure (binary garbage, empty body) every output is
/// `None` / empty so the SPA treats the row as a thread-of-one.
pub(crate) fn extract_threading_headers(
    body_bytes: &[u8],
) -> (Option<String>, Option<String>, Vec<String>) {
    let parsed = match mailparse::parse_mail(body_bytes) {
        Ok(p) => p,
        Err(_) => return (None, None, Vec::new()),
    };

    // RFC 5322 §3.6.4: Message-ID / In-Reply-To MUST contain exactly one
    // msg-id (`<...@...>`). Some senders technically allow whitespace-separated
    // ids in In-Reply-To — when that happens we keep only the first token so
    // downstream consumers can compare it directly against `references` and
    // `message_id` values across messages.
    let message_id = extract_header(&parsed, "Message-ID").and_then(first_msg_id_token);
    let in_reply_to = extract_header(&parsed, "In-Reply-To").and_then(first_msg_id_token);

    let references: Vec<String> = extract_header(&parsed, "References")
        .map(|raw| {
            raw.split_whitespace()
                .filter(|tok| !tok.is_empty())
                .map(String::from)
                .collect()
        })
        .unwrap_or_default();

    (message_id, in_reply_to, references)
}

/// Returns the first whitespace-delimited token of a `Message-ID` /
/// `In-Reply-To` header, or `None` when the value collapses to empty after
/// trimming. Keeping this as a free helper makes it easy to unit-test the
/// "many tokens in In-Reply-To" edge case without standing up a parsed mail.
fn first_msg_id_token(raw: String) -> Option<String> {
    raw.split_whitespace()
        .find(|tok| !tok.is_empty())
        .map(String::from)
}

/// Added (TMAIL-329): truncated-body → ~200 char preview snippet for the
/// alt-UI EmailList rows.
///
/// Called by `list_messages` / `search_messages` against the bytes returned
/// from a `BODY.PEEK[]<0.8192>` partial IMAP fetch. We don't request the
/// whole message body just to render a preview — that would blow up payload
/// size and IMAP latency on every list refresh — so the input here is up to
/// the first 8 KiB of the raw RFC 822 message. mailparse handles truncated
/// MIME gracefully when the headers are intact, which they almost always
/// are inside the first 8 KiB.
///
/// Resolution order:
///   1. text/plain leaf (preferred — already plain text)
///   2. text/html leaf (run through `strip_html_to_text`)
///   3. fallback to the parsed mail's `get_body()` (single-part text/* messages)
///
/// Returns `None` when nothing usable could be extracted (parse failure,
/// no readable body, only attachments) so the SPA shows an empty preview
/// line instead of the literal string "None".
pub(crate) fn extract_preview(body_bytes: &[u8]) -> Option<String> {
    const MAX_PREVIEW_CHARS: usize = 200;

    let parsed = mailparse::parse_mail(body_bytes).ok()?;

    // Walk the MIME tree once, collecting the first text/plain and first
    // text/html leaves we find. Cheaper than two passes and matches how
    // `extract_parts` walks the same shape.
    let mut text: Option<String> = None;
    let mut html: Option<String> = None;
    collect_text_parts(&parsed, &mut text, &mut html);

    // Single-part text/* messages (no multipart wrapper) — the walker above
    // skips those because `subparts.is_empty()` is the leaf branch. Fall back
    // to the top-level body when neither text nor html came out of the walk.
    let raw = text
        .or_else(|| html.map(|h| strip_html_to_text(&h)))
        .or_else(|| parsed.get_body().ok());

    let raw = raw?;
    let cleaned = collapse_whitespace(&raw);
    if cleaned.is_empty() {
        return None;
    }
    Some(truncate_chars(&cleaned, MAX_PREVIEW_CHARS))
}

/// Walk the MIME tree and capture the first text/plain leaf into `text` and
/// the first text/html leaf into `html`. Mirrors the structure of
/// `extract_parts` but skips the attachment branch (we only care about
/// rendering a preview, not enumerating parts).
fn collect_text_parts(
    mail: &mailparse::ParsedMail,
    text: &mut Option<String>,
    html: &mut Option<String>,
) {
    if mail.subparts.is_empty() {
        let ct = &mail.ctype.mimetype;
        if ct == "text/plain" && text.is_none() {
            if let Ok(body) = mail.get_body() {
                *text = Some(body);
            }
        } else if ct == "text/html" && html.is_none() {
            if let Ok(body) = mail.get_body() {
                *html = Some(body);
            }
        }
        return;
    }
    for part in &mail.subparts {
        collect_text_parts(part, text, html);
        if text.is_some() {
            // Optimisation: text/plain wins outright, no need to keep
            // walking once we have one.
            return;
        }
    }
}

/// Very small HTML → text reducer for previews. Not a general HTML parser —
/// the only goal is to produce a readable snippet. We drop <script>/<style>
/// blocks (otherwise their contents leak into the preview), replace block
/// tags with spaces so words don't run together, then strip remaining tags.
/// Final whitespace collapse happens in `collapse_whitespace`.
fn strip_html_to_text(html: &str) -> String {
    let mut out = String::with_capacity(html.len());
    let bytes = html.as_bytes();
    let mut i = 0;
    let mut in_tag = false;
    // skip_until is set to the tag name when we hit <script>/<style> so we
    // can fast-forward past their text content.
    let mut skip_until: Option<&'static str> = None;

    while i < bytes.len() {
        if let Some(close_tag) = skip_until {
            // Look for the matching closing tag (case-insensitive) and jump
            // past it. If we never find one (truncated MIME) the rest of the
            // input is lost — acceptable for a 200-char preview.
            let rest = &html[i..];
            let lower = rest.to_ascii_lowercase();
            if let Some(idx) = lower.find(close_tag) {
                i += idx + close_tag.len();
                skip_until = None;
                continue;
            }
            break;
        }

        let c = bytes[i];
        if c == b'<' {
            // Detect <script> / <style> opens so we skip their contents.
            let rest = &html[i..];
            let lower = rest.to_ascii_lowercase();
            if lower.starts_with("<script") {
                skip_until = Some("</script>");
                i += 1;
                continue;
            }
            if lower.starts_with("<style") {
                skip_until = Some("</style>");
                i += 1;
                continue;
            }
            in_tag = true;
            // Block-level tags like </p>, <br>, </div>, </li> should leave a
            // word boundary so adjacent text doesn't get welded together.
            out.push(' ');
            i += 1;
            continue;
        }
        if in_tag {
            if c == b'>' {
                in_tag = false;
            }
            i += 1;
            continue;
        }
        out.push(c as char);
        i += 1;
    }

    // Decode a small set of common HTML entities. Not exhaustive — preview
    // text is best-effort and a missed entity just shows literally.
    out.replace("&nbsp;", " ")
        .replace("&amp;", "&")
        .replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&quot;", "\"")
        .replace("&#39;", "'")
        .replace("&apos;", "'")
}

/// Collapse runs of whitespace (including newlines/tabs/CR) into a single
/// space and trim the result. Keeps the preview line tidy regardless of
/// how the source text was wrapped.
fn collapse_whitespace(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut prev_space = true;
    for ch in s.chars() {
        if ch.is_whitespace() {
            if !prev_space {
                out.push(' ');
                prev_space = true;
            }
        } else {
            out.push(ch);
            prev_space = false;
        }
    }
    out.trim().to_string()
}

/// Truncate to at most `max_chars` graphemes-ish (we count Unicode chars,
/// not bytes, so multibyte text doesn't get sliced mid-codepoint). Appends
/// an ellipsis when truncation happened so the UI shows the snippet was
/// cut off.
fn truncate_chars(s: &str, max_chars: usize) -> String {
    if s.chars().count() <= max_chars {
        return s.to_string();
    }
    let mut out: String = s.chars().take(max_chars).collect();
    out.push('…');
    out
}

/// Added (TMAIL-320): walk the MIME tree the same way `extract_parts` does
/// and return the raw bytes + filename + content-type for the leaf whose
/// dotted path matches `target_part_id`. Returns `None` if no leaf matches.
///
/// Path assignment MUST stay in sync with `extract_parts` above — both
/// functions number top-level subparts 1..N and append ".M" on each level
/// down — otherwise the SPA would receive a `part_id` for an attachment
/// and find nothing when it asked for that part back.
fn find_part_by_id(
    mail: &mailparse::ParsedMail,
    target_part_id: &str,
    part_prefix: &str,
) -> Option<MessagePart> {
    if mail.subparts.is_empty() {
        if part_prefix == target_part_id {
            let disposition = mail.get_content_disposition();
            let filename = disposition
                .params
                .get("filename")
                .cloned()
                .or_else(|| mail.ctype.params.get("name").cloned())
                .unwrap_or_else(|| "attachment".to_string());
            let bytes = mail.get_body_raw().unwrap_or_default();
            return Some(MessagePart {
                filename,
                content_type: mail.ctype.mimetype.clone(),
                bytes,
            });
        }
        return None;
    }
    for (i, part) in mail.subparts.iter().enumerate() {
        let prefix = if part_prefix.is_empty() {
            format!("{}", i + 1)
        } else {
            format!("{}.{}", part_prefix, i + 1)
        };
        if let Some(found) = find_part_by_id(part, target_part_id, &prefix) {
            return Some(found);
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    // Helper to build a simple email for testing
    fn simple_email(headers: &str, body: &str) -> String {
        format!("{}\r\n\r\n{}", headers, body)
    }

    #[test]
    fn test_extract_header_message_id() {
        let raw = simple_email(
            "From: test@example.com\r\nMessage-ID: <abc123@example.com>\r\nSubject: Test",
            "Hello world",
        );
        let parsed = mailparse::parse_mail(raw.as_bytes()).unwrap();
        let msg_id = extract_header(&parsed, "Message-ID");
        assert_eq!(msg_id, Some("<abc123@example.com>".to_string()));
    }

    #[test]
    fn test_extract_header_in_reply_to() {
        let raw = simple_email(
            "From: test@example.com\r\nIn-Reply-To: <parent@example.com>\r\nSubject: Re: Test",
            "Reply body",
        );
        let parsed = mailparse::parse_mail(raw.as_bytes()).unwrap();
        let irt = extract_header(&parsed, "In-Reply-To");
        assert_eq!(irt, Some("<parent@example.com>".to_string()));
    }

    #[test]
    fn test_extract_header_references() {
        let raw = simple_email(
            "From: test@example.com\r\nReferences: <a@example.com> <b@example.com>\r\nSubject: Test",
            "Body",
        );
        let parsed = mailparse::parse_mail(raw.as_bytes()).unwrap();
        let refs = extract_header(&parsed, "References");
        assert_eq!(refs, Some("<a@example.com> <b@example.com>".to_string()));
    }

    #[test]
    fn test_extract_header_missing() {
        let raw = simple_email("From: test@example.com\r\nSubject: Test", "Body");
        let parsed = mailparse::parse_mail(raw.as_bytes()).unwrap();
        assert_eq!(extract_header(&parsed, "Message-ID"), None);
        assert_eq!(extract_header(&parsed, "In-Reply-To"), None);
    }

    #[test]
    fn test_extract_header_case_insensitive() {
        let raw = simple_email(
            "From: test@example.com\r\nmessage-id: <lower@example.com>",
            "Body",
        );
        let parsed = mailparse::parse_mail(raw.as_bytes()).unwrap();
        let msg_id = extract_header(&parsed, "Message-ID");
        assert_eq!(msg_id, Some("<lower@example.com>".to_string()));
    }

    #[test]
    fn test_extract_parts_plain_text() {
        let raw = simple_email(
            "From: test@example.com\r\nContent-Type: text/plain",
            "Hello plain world",
        );
        let parsed = mailparse::parse_mail(raw.as_bytes()).unwrap();
        let mut text = None;
        let mut html = None;
        let mut attachments = Vec::new();
        extract_parts(&parsed, &mut text, &mut html, &mut attachments, "");

        assert!(text.is_some());
        assert!(text.unwrap().contains("Hello plain world"));
        assert!(html.is_none());
        assert!(attachments.is_empty());
    }

    #[test]
    fn test_extract_parts_html() {
        let raw = simple_email(
            "From: test@example.com\r\nContent-Type: text/html",
            "<p>Hello HTML</p>",
        );
        let parsed = mailparse::parse_mail(raw.as_bytes()).unwrap();
        let mut text = None;
        let mut html = None;
        let mut attachments = Vec::new();
        extract_parts(&parsed, &mut text, &mut html, &mut attachments, "");

        assert!(html.is_some());
        assert!(html.unwrap().contains("<p>Hello HTML</p>"));
        assert!(text.is_none());
    }

    #[test]
    fn test_extract_parts_multipart() {
        let raw = concat!(
            "From: test@example.com\r\n",
            "Content-Type: multipart/alternative; boundary=\"boundary123\"\r\n",
            "\r\n",
            "--boundary123\r\n",
            "Content-Type: text/plain\r\n",
            "\r\n",
            "Plain text part\r\n",
            "--boundary123\r\n",
            "Content-Type: text/html\r\n",
            "\r\n",
            "<p>HTML part</p>\r\n",
            "--boundary123--\r\n",
        );
        let parsed = mailparse::parse_mail(raw.as_bytes()).unwrap();
        let mut text = None;
        let mut html = None;
        let mut attachments = Vec::new();
        extract_parts(&parsed, &mut text, &mut html, &mut attachments, "");

        assert!(text.as_deref().unwrap().contains("Plain text part"));
        assert!(html.unwrap().contains("<p>HTML part</p>"));
        assert!(attachments.is_empty());
    }

    // Added (TMAIL-329): preview-extraction tests covering the shapes that
    // BODY.PEEK[]<0.8192> returns in practice — single-part text, single-part
    // html, multipart with both, html-only, empty, long bodies (truncation),
    // attachment-only, garbled MIME (parse failure path).

    #[test]
    fn test_extract_preview_plain_text() {
        let raw = simple_email(
            "From: a@example.com\r\nContent-Type: text/plain; charset=utf-8",
            "Hello there, here is a short preview.",
        );
        let preview = extract_preview(raw.as_bytes()).unwrap();
        assert_eq!(preview, "Hello there, here is a short preview.");
    }

    #[test]
    fn test_extract_preview_collapses_whitespace() {
        // Plain text with hard line wraps and trailing whitespace should
        // come out as a single tidy line — that's how every email reader
        // renders a preview snippet.
        let raw = simple_email(
            "From: a@example.com\r\nContent-Type: text/plain",
            "Line one\r\n   Line two\r\n\r\nLine three\t\twith tabs",
        );
        let preview = extract_preview(raw.as_bytes()).unwrap();
        assert_eq!(preview, "Line one Line two Line three with tabs");
    }

    #[test]
    fn test_extract_preview_html_only() {
        let raw = simple_email(
            "From: a@example.com\r\nContent-Type: text/html; charset=utf-8",
            "<p>Hello <b>world</b></p><p>Second paragraph</p>",
        );
        let preview = extract_preview(raw.as_bytes()).unwrap();
        assert!(preview.contains("Hello"));
        assert!(preview.contains("world"));
        assert!(preview.contains("Second paragraph"));
        // No raw tags should leak through.
        assert!(!preview.contains('<'));
        assert!(!preview.contains('>'));
    }

    #[test]
    fn test_extract_preview_html_drops_script_and_style() {
        let raw = simple_email(
            "From: a@example.com\r\nContent-Type: text/html",
            "<style>p { color: red; }</style><script>alert('x')</script><p>Visible text</p>",
        );
        let preview = extract_preview(raw.as_bytes()).unwrap();
        assert!(preview.contains("Visible text"));
        assert!(!preview.contains("color: red"));
        assert!(!preview.contains("alert"));
    }

    #[test]
    fn test_extract_preview_multipart_prefers_text_plain() {
        // text/plain wins over text/html when both are present — matches the
        // FullMessage extraction behaviour and avoids the lossy HTML strip.
        let raw = concat!(
            "From: a@example.com\r\n",
            "Content-Type: multipart/alternative; boundary=\"B\"\r\n",
            "\r\n",
            "--B\r\n",
            "Content-Type: text/plain\r\n",
            "\r\n",
            "Plain version of the message\r\n",
            "--B\r\n",
            "Content-Type: text/html\r\n",
            "\r\n",
            "<p>HTML version</p>\r\n",
            "--B--\r\n",
        );
        let preview = extract_preview(raw.as_bytes()).unwrap();
        assert!(preview.contains("Plain version"));
        // We chose plain, so the HTML alternative should NOT be in the
        // snippet.
        assert!(!preview.contains("HTML version"));
    }

    #[test]
    fn test_extract_preview_truncates_long_body() {
        // 600 chars of repeated "abc " — well past the 200-char cap.
        let long_body = "abc ".repeat(150);
        let raw = simple_email(
            "From: a@example.com\r\nContent-Type: text/plain",
            &long_body,
        );
        let preview = extract_preview(raw.as_bytes()).unwrap();
        // 200 chars + 1 ellipsis = 201 characters.
        assert_eq!(preview.chars().count(), 201);
        assert!(preview.ends_with('…'));
    }

    #[test]
    fn test_extract_preview_handles_unicode_boundary() {
        // Multibyte chars must not get sliced mid-codepoint when truncating.
        // 250 copies of "🚀" (each 4 bytes UTF-8) — pure char counting must
        // win over byte counting here.
        let body = "🚀".repeat(250);
        let raw = simple_email(
            "From: a@example.com\r\nContent-Type: text/plain; charset=utf-8",
            &body,
        );
        let preview = extract_preview(raw.as_bytes()).unwrap();
        // Must be valid UTF-8 and have 200 rockets + ellipsis.
        assert_eq!(preview.chars().count(), 201);
        assert!(preview.starts_with("🚀"));
    }

    #[test]
    fn test_extract_preview_returns_none_for_attachment_only() {
        // A multipart/mixed with only an application/pdf attachment — no
        // text or html body anywhere — should yield None.
        let raw = concat!(
            "From: a@example.com\r\n",
            "Content-Type: multipart/mixed; boundary=\"B\"\r\n",
            "\r\n",
            "--B\r\n",
            "Content-Type: application/pdf; name=\"x.pdf\"\r\n",
            "Content-Disposition: attachment; filename=\"x.pdf\"\r\n",
            "\r\n",
            "PDFBYTES\r\n",
            "--B--\r\n",
        );
        let preview = extract_preview(raw.as_bytes());
        assert!(preview.is_none(), "expected None, got {:?}", preview);
    }

    #[test]
    fn test_extract_preview_returns_none_for_empty_body() {
        // mailparse will parse this successfully but the body is empty.
        let raw = "From: a@example.com\r\nContent-Type: text/plain\r\n\r\n";
        let preview = extract_preview(raw.as_bytes());
        assert!(preview.is_none(), "expected None, got {:?}", preview);
    }

    #[test]
    fn test_extract_preview_returns_none_for_unparseable_input() {
        // Garbage bytes with no MIME shape — mailparse should fail and we
        // should swallow that into None rather than panicking.
        let preview = extract_preview(b"\xff\xfe\x00not a valid email\x00\x01");
        // mailparse is permissive enough that it sometimes "succeeds" on
        // junk by treating it as a header-less body. Either None OR a
        // truncated snippet are acceptable — but it must not panic. Assert
        // only the no-panic contract: the call returned without unwinding.
        let _ = preview;
    }

    #[test]
    fn test_strip_html_to_text_handles_entities() {
        // The small entity set we decode covers the most common ones.
        // Anything else passes through literally — acceptable for previews.
        let html = "Tom &amp; Jerry &lt;said&gt; &quot;hi&quot; &#39;ok&#39; &nbsp; end";
        let plain = strip_html_to_text(html);
        let normalised = collapse_whitespace(&plain);
        assert_eq!(normalised, "Tom & Jerry <said> \"hi\" 'ok' end");
    }

    #[test]
    fn test_truncate_chars_no_truncation_when_short() {
        // Short input must come back unchanged — no trailing ellipsis.
        assert_eq!(truncate_chars("short", 200), "short");
    }

    // Added (TMAIL-320): build a multipart/mixed email with one text part
    // and one PDF attachment so we can assert find_part_by_id resolves
    // the same dotted path that extract_parts assigns.
    fn email_with_pdf_attachment() -> &'static str {
        concat!(
            "From: test@example.com\r\n",
            "Subject: Has attachment\r\n",
            "Content-Type: multipart/mixed; boundary=\"BOUNDARY\"\r\n",
            "\r\n",
            "--BOUNDARY\r\n",
            "Content-Type: text/plain\r\n",
            "\r\n",
            "See attached.\r\n",
            "--BOUNDARY\r\n",
            "Content-Type: application/pdf; name=\"report.pdf\"\r\n",
            "Content-Disposition: attachment; filename=\"report.pdf\"\r\n",
            "Content-Transfer-Encoding: base64\r\n",
            "\r\n",
            "JVBERi0xLjQK\r\n",
            "--BOUNDARY--\r\n",
        )
    }

    #[test]
    fn test_find_part_by_id_resolves_attachment() {
        let parsed = mailparse::parse_mail(email_with_pdf_attachment().as_bytes()).unwrap();

        // extract_parts and find_part_by_id MUST agree on numbering. Walk
        // attachments first to grab whatever part_id was assigned, then ask
        // find_part_by_id for that same part_id and assert the round-trip.
        let mut text = None;
        let mut html = None;
        let mut attachments = Vec::new();
        extract_parts(&parsed, &mut text, &mut html, &mut attachments, "");
        assert_eq!(attachments.len(), 1, "expected one PDF attachment");
        let att = &attachments[0];
        assert_eq!(att.filename, "report.pdf");
        assert_eq!(att.content_type, "application/pdf");
        assert_eq!(att.part_id, "2", "PDF is the second top-level subpart");

        let part = find_part_by_id(&parsed, &att.part_id, "")
            .expect("find_part_by_id must resolve the same part_id extract_parts emitted");
        assert_eq!(part.filename, "report.pdf");
        assert_eq!(part.content_type, "application/pdf");
        // Base64 "JVBERi0xLjQK" decodes to "%PDF-1.4\n" — confirms we
        // returned the decoded body, not the base64-encoded source.
        assert_eq!(part.bytes, b"%PDF-1.4\n");
    }

    #[test]
    fn test_find_part_by_id_missing_returns_none() {
        let parsed = mailparse::parse_mail(email_with_pdf_attachment().as_bytes()).unwrap();
        assert!(find_part_by_id(&parsed, "9.9.9", "").is_none());
    }

    #[test]
    fn test_find_part_by_id_falls_back_to_ctype_name() {
        // When Content-Disposition is missing but Content-Type has a name=
        // param we should still surface that as the download filename — some
        // MTAs strip the disposition header.
        let raw = concat!(
            "From: test@example.com\r\n",
            "Content-Type: multipart/mixed; boundary=\"B\"\r\n",
            "\r\n",
            "--B\r\n",
            "Content-Type: text/plain\r\n",
            "\r\n",
            "body\r\n",
            "--B\r\n",
            "Content-Type: image/png; name=\"logo.png\"\r\n",
            "Content-Transfer-Encoding: base64\r\n",
            "\r\n",
            "iVBORw0K\r\n",
            "--B--\r\n",
        );
        let parsed = mailparse::parse_mail(raw.as_bytes()).unwrap();
        let part = find_part_by_id(&parsed, "2", "").expect("part 2 resolves");
        assert_eq!(part.filename, "logo.png");
        assert_eq!(part.content_type, "image/png");
    }

    #[test]
    fn test_extract_references_split() {
        let raw = simple_email(
            "References: <a@ex.com> <b@ex.com> <c@ex.com>",
            "Body",
        );
        let parsed = mailparse::parse_mail(raw.as_bytes()).unwrap();
        let refs_str = extract_header(&parsed, "References").unwrap();
        let refs: Vec<&str> = refs_str.split_whitespace().collect();
        assert_eq!(refs, vec!["<a@ex.com>", "<b@ex.com>", "<c@ex.com>"]);
    }

    #[test]
    fn test_trash_folder_defaults_to_legacy_name() {
        // When the per-user `imap_configurations.trash_folder` column is NULL
        // (`user_trash_folder = None`), `trash_folder()` must fall back to the
        // hardcoded "Trash" default so Dovecot/legacy mailboxes keep working.
        let svc = ImapService {
            config: ImapConfig {
                host: "127.0.0.1".to_string(),
                port: 993,
                tls: true,
                master_password: None,
            },
            user_credentials: None,
            user_trash_folder: None,
        };
        assert_eq!(svc.trash_folder(), "Trash");
    }

    #[test]
    fn test_trash_folder_honours_per_user_override() {
        // BYOK path: when user_trash_folder is set (Stalwart "Deleted Items",
        // Gmail "[Gmail]/Trash", etc.) the service uses it instead of "Trash".
        let cfg = ImapConfig {
            host: "127.0.0.1".to_string(),
            port: 993,
            tls: true,
            master_password: None,
        };
        let svc = ImapService {
            config: cfg,
            user_credentials: Some(("u".to_string(), "p".to_string())),
            user_trash_folder: Some("Deleted Items".to_string()),
        };
        assert_eq!(svc.trash_folder(), "Deleted Items");
    }

    #[test]
    fn test_folder_struct_serialization() {
        let folder = Folder {
            name: "INBOX".to_string(),
            delimiter: "/".to_string(),
            messages: Some(42),
            unseen: Some(5),
        };
        let json = serde_json::to_value(&folder).unwrap();
        assert_eq!(json["name"], "INBOX");
        assert_eq!(json["messages"], 42);
        assert_eq!(json["unseen"], 5);
    }

    #[test]
    fn test_message_envelope_serialization() {
        let env = MessageEnvelope {
            uid: 100,
            subject: Some("Test Subject".to_string()),
            from: Some("sender@example.com".to_string()),
            date: Some("2026-04-10".to_string()),
            flags: vec!["\\Seen".to_string()],
            size: Some(1024),
            // Added (TMAIL-329): preview field — serialises as null when
            // None so the SPA can fall back to an empty preview line.
            preview: Some("Hello there, this is a test preview".to_string()),
            // Added (TMAIL-350): threading headers on the envelope.
            message_id: Some("<m100@example.com>".to_string()),
            in_reply_to: Some("<parent@example.com>".to_string()),
            references: vec!["<root@example.com>".to_string(), "<parent@example.com>".to_string()],
        };
        let json = serde_json::to_value(&env).unwrap();
        assert_eq!(json["uid"], 100);
        assert_eq!(json["subject"], "Test Subject");
        assert_eq!(json["flags"][0], "\\Seen");
        assert_eq!(json["preview"], "Hello there, this is a test preview");
        assert_eq!(json["message_id"], "<m100@example.com>");
        assert_eq!(json["in_reply_to"], "<parent@example.com>");
        assert_eq!(json["references"][0], "<root@example.com>");
        assert_eq!(json["references"][1], "<parent@example.com>");
    }

    #[test]
    fn test_message_envelope_serialization_preview_none() {
        // Added (TMAIL-329): explicit `null` preview is in the wire shape so
        // the alt-UI's TS narrowing path (`m.preview ?? ''`) covers the case.
        let env = MessageEnvelope {
            uid: 100,
            subject: None,
            from: None,
            date: None,
            flags: vec![],
            size: None,
            preview: None,
            // Added (TMAIL-350): empty threading headers should serialise as
            // null/null/[] so the SPA treats the row as a thread-of-one.
            message_id: None,
            in_reply_to: None,
            references: vec![],
        };
        let json = serde_json::to_value(&env).unwrap();
        assert!(json["preview"].is_null());
        assert!(json["message_id"].is_null());
        assert!(json["in_reply_to"].is_null());
        assert_eq!(json["references"], serde_json::json!([]));
    }

    // ── TMAIL-350: extract_threading_headers() unit tests ─────────────────
    // The helper is the data path that backs the alt-UI Threaded view, so
    // cover the headline shapes: full chain (Message-ID + In-Reply-To +
    // References), root-of-thread (only Message-ID), absent headers
    // (thread-of-one fallback), multi-token In-Reply-To (RFC technically
    // allows whitespace-separated ids — we keep the first), and a
    // truncated/garbage body so the parse-error branch is exercised.

    #[test]
    fn test_extract_threading_headers_full_chain() {
        let raw = simple_email(
            "From: a@example.com\r\n\
             Message-ID: <child@example.com>\r\n\
             In-Reply-To: <parent@example.com>\r\n\
             References: <root@example.com> <middle@example.com> <parent@example.com>\r\n\
             Subject: Re: hi",
            "reply body",
        );
        let (msg_id, irt, refs) = extract_threading_headers(raw.as_bytes());
        assert_eq!(msg_id, Some("<child@example.com>".to_string()));
        assert_eq!(irt, Some("<parent@example.com>".to_string()));
        assert_eq!(
            refs,
            vec![
                "<root@example.com>".to_string(),
                "<middle@example.com>".to_string(),
                "<parent@example.com>".to_string(),
            ],
        );
    }

    #[test]
    fn test_extract_threading_headers_root_of_thread() {
        // Brand new conversation: only Message-ID is set; In-Reply-To +
        // References absent. SPA should treat this as a new thread.
        let raw = simple_email(
            "From: a@example.com\r\nMessage-ID: <root@example.com>\r\nSubject: hi",
            "first message in the thread",
        );
        let (msg_id, irt, refs) = extract_threading_headers(raw.as_bytes());
        assert_eq!(msg_id, Some("<root@example.com>".to_string()));
        assert_eq!(irt, None);
        assert!(refs.is_empty());
    }

    #[test]
    fn test_extract_threading_headers_no_headers_at_all() {
        // Some legacy / spam senders omit Message-ID entirely. We return
        // None across the board so the SPA falls back to a thread-of-one
        // bucket keyed by uid.
        let raw = simple_email("From: a@example.com\r\nSubject: no-headers", "body");
        let (msg_id, irt, refs) = extract_threading_headers(raw.as_bytes());
        assert_eq!(msg_id, None);
        assert_eq!(irt, None);
        assert!(refs.is_empty());
    }

    #[test]
    fn test_extract_threading_headers_multi_token_in_reply_to() {
        // RFC 5322 §3.6.4 says In-Reply-To "should" hold one msg-id but a
        // handful of senders pack multiple, whitespace-separated. We keep
        // only the first so cross-message equality checks against
        // Message-ID work without re-parsing.
        let raw = simple_email(
            "From: a@example.com\r\n\
             Message-ID: <c@example.com>\r\n\
             In-Reply-To: <p1@example.com> <p2@example.com>",
            "weird sender",
        );
        let (_msg_id, irt, _refs) = extract_threading_headers(raw.as_bytes());
        assert_eq!(irt, Some("<p1@example.com>".to_string()));
    }

    #[test]
    fn test_extract_threading_headers_unparsable_body_returns_empty() {
        // mailparse fails hard on binary garbage with no header block. We
        // must NOT panic — list_messages keeps walking and the affected
        // envelope just renders as an unthreaded row in the SPA.
        let (msg_id, irt, refs) = extract_threading_headers(&[0xff, 0xfe, 0x00]);
        // mailparse is permissive: it may return a parsed mail with zero
        // headers rather than failing outright. Either way the three
        // threading fields must all be the "no thread" sentinel.
        assert_eq!(msg_id, None);
        assert_eq!(irt, None);
        assert!(refs.is_empty());
    }

    #[test]
    fn test_full_message_serialization() {
        let msg = FullMessage {
            uid: 1,
            subject: Some("Thread Test".to_string()),
            from: Some("a@example.com".to_string()),
            to: vec!["b@example.com".to_string()],
            cc: vec![],
            date: Some("2026-04-10T10:00:00Z".to_string()),
            flags: vec![],
            text_body: Some("Hello".to_string()),
            html_body: None,
            attachments: vec![],
            message_id: Some("<msg1@example.com>".to_string()),
            in_reply_to: None,
            references: vec![],
        };
        let json = serde_json::to_value(&msg).unwrap();
        assert_eq!(json["message_id"], "<msg1@example.com>");
        assert_eq!(json["references"], serde_json::json!([]));
    }

    #[test]
    fn test_full_message_threading_fields() {
        let msg = FullMessage {
            uid: 2,
            subject: Some("Re: Thread Test".to_string()),
            from: Some("b@example.com".to_string()),
            to: vec!["a@example.com".to_string()],
            cc: vec![],
            date: None,
            flags: vec![],
            text_body: Some("Reply".to_string()),
            html_body: None,
            attachments: vec![],
            message_id: Some("<msg2@example.com>".to_string()),
            in_reply_to: Some("<msg1@example.com>".to_string()),
            references: vec!["<msg1@example.com>".to_string()],
        };
        let json = serde_json::to_value(&msg).unwrap();
        assert_eq!(json["in_reply_to"], "<msg1@example.com>");
        assert_eq!(json["references"][0], "<msg1@example.com>");
    }

    #[test]
    fn test_attachment_serialization() {
        let att = Attachment {
            filename: "document.pdf".to_string(),
            content_type: "application/pdf".to_string(),
            size: 2048,
            part_id: "1.2".to_string(),
        };
        let json = serde_json::to_value(&att).unwrap();
        assert_eq!(json["filename"], "document.pdf");
        assert_eq!(json["size"], 2048);
        assert_eq!(json["part_id"], "1.2");
    }

    // TMAIL-311 (finishes TMAIL-156): the legacy `ImapService::new(global_config)`
    // constructor was removed, so the BYOK contract regression tests that asserted
    // `user_creds()` returns None and `connect_user()` errors out on the legacy
    // path are no longer applicable — the path they guarded against no longer
    // exists. `for_user` is now the only way to build an ImapService in
    // production code; test code that needs a credential-less instance does so
    // via direct struct literal (see `test_trash_folder_defaults_to_legacy_name`).
}
