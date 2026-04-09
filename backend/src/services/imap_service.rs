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
#[derive(Debug, Clone, Serialize)]
pub struct MessageEnvelope {
    pub uid: u32,
    pub subject: Option<String>,
    pub from: Option<String>,
    pub date: Option<String>,
    pub flags: Vec<String>,
    pub size: Option<u32>,
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
}

#[derive(Debug, Clone, Serialize)]
pub struct Attachment {
    pub filename: String,
    pub content_type: String,
    pub size: usize,
    pub part_id: String,
}

type ImapSession = Session<TlsStream<Compat<TcpStream>>>;

/// IMAP service for connecting to Dovecot and performing mail operations
pub struct ImapService {
    config: ImapConfig,
}

impl ImapService {
    pub fn new(config: ImapConfig) -> Self {
        Self { config }
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
        let messages: Vec<_> = session
            .fetch(&range, "(UID ENVELOPE FLAGS RFC822.SIZE)")
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

            envelopes.push(MessageEnvelope {
                uid,
                subject,
                from,
                date,
                flags,
                size: msg.size,
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

        let messages: Vec<_> = session
            .uid_fetch(&uid_range, "(UID ENVELOPE FLAGS RFC822.SIZE)")
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

            envelopes.push(MessageEnvelope {
                uid,
                subject,
                from,
                date,
                flags,
                size: msg.size,
            });
        }

        envelopes.reverse();
        let _ = session.logout().await;

        Ok(envelopes)
    }

    /// Move a message to a different folder
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

        // Copy to destination
        session
            .uid_copy(uid.to_string(), to_folder)
            .await
            .map_err(|e| AppError::Imap(format!("COPY failed: {}", e)))?;

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
    pub async fn delete_message(
        &self,
        username: &str,
        password: &str,
        folder: &str,
        uid: u32,
    ) -> Result<(), AppError> {
        if folder == "Trash" {
            // Permanent delete from Trash
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
            // Move to Trash
            self.move_message(username, password, folder, uid, "Trash")
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
