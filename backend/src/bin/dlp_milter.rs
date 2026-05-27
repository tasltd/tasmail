// Added: DLP milter daemon for TMAIL-108
//
// PURPOSE: Postfix-side milter that intercepts outbound mail before queueing.
//   Postfix opens a TCP connection to this process for every message, streams
//   the envelope/headers/body, and waits for an Accept/Reject verdict.
//
// CONFIG: TASMAIL_MILTER_BIND  (default 127.0.0.1:8895)
//         TASMAIL_DLP_DATABASE_URL or DATABASE_URL  (PostgreSQL DSN)
//         TASMAIL_DLP_BLOCKED_EXTENSIONS  (csv, default = library default list)
//         TASMAIL_DLP_FAIL_OPEN  (true|false, default true — match Postfix's
//                                 milter_default_action = accept semantics)
//
// SCOPE: Only used by the optional self-host Postfix deployment. The BYOK
//   path (mail.techatscale.io) does not run Postfix, so this binary is
//   not started in that environment. See docs/SELF-HOST-MAIL-SERVERS.md.

use std::env;
use std::ffi::CString;
use std::sync::Arc;

use bytes::Bytes;
use indymilter::{Callbacks, Context, ContextActions, EomContext, Status};
use mailparse::MailHeaderMap;
use sqlx::PgPool;
use tokio::net::TcpListener;
use tokio::signal;
use tracing::{error, info, warn};

use tasmail::models::dlp_rule::{DlpAction, DlpRule, DlpScanMatch};
use tasmail::services::dlp_scanner::{
    scan_attachments, scan_content, DEFAULT_BLOCKED_EXTENSIONS,
};

/// PURPOSE: Per-connection state accumulated across milter callbacks
/// CONSTRAINTS: indymilter stores this as `Context.data: Option<T>` and resets
///   it across abort/reuse — we always go through `data_mut()` to lazy-init.
#[derive(Default)]
struct MessageState {
    sender: Option<String>,
    recipients: Vec<String>,
    subject: Option<String>,
    body: Vec<u8>,
    attachment_filenames: Vec<String>,
}

/// Shared milter config — DB pool + policy, cloned into each callback closure
struct MilterConfig {
    pool: PgPool,
    blocked_exts: Vec<String>,
    fail_open: bool,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info")),
        )
        .init();

    let bind = env::var("TASMAIL_MILTER_BIND").unwrap_or_else(|_| "127.0.0.1:8895".to_string());
    let db_url = env::var("TASMAIL_DLP_DATABASE_URL")
        .or_else(|_| env::var("DATABASE_URL"))
        .map_err(|_| "TASMAIL_DLP_DATABASE_URL or DATABASE_URL must be set")?;
    let fail_open = env::var("TASMAIL_DLP_FAIL_OPEN")
        .map(|v| v != "false" && v != "0")
        .unwrap_or(true);

    let blocked_exts: Vec<String> = match env::var("TASMAIL_DLP_BLOCKED_EXTENSIONS") {
        Ok(csv) => csv
            .split(',')
            .map(|s| s.trim().trim_start_matches('.').to_lowercase())
            .filter(|s| !s.is_empty())
            .collect(),
        Err(_) => DEFAULT_BLOCKED_EXTENSIONS.iter().map(|s| s.to_string()).collect(),
    };

    info!(bind = %bind, blocked_ext_count = blocked_exts.len(), fail_open, "starting DLP milter");

    let pool = PgPool::connect(&db_url).await?;
    let cfg = Arc::new(MilterConfig {
        pool,
        blocked_exts,
        fail_open,
    });

    let listener = TcpListener::bind(&bind).await?;
    info!(addr = %bind, "DLP milter listening");

    let callbacks: Callbacks<MessageState> = build_callbacks(cfg);

    indymilter::run(listener, callbacks, Default::default(), signal::ctrl_c())
        .await
        .map_err(|e| {
            error!(error = ?e, "milter run loop failed");
            e
        })?;
    Ok(())
}

fn build_callbacks(cfg: Arc<MilterConfig>) -> Callbacks<MessageState> {
    Callbacks::new()
        .on_mail(|ctx, args| Box::pin(on_mail_cb(ctx, args)))
        .on_rcpt(|ctx, args| Box::pin(on_rcpt_cb(ctx, args)))
        .on_header(|ctx, name, value| Box::pin(on_header_cb(ctx, name, value)))
        .on_body(|ctx, chunk| Box::pin(on_body_cb(ctx, chunk)))
        .on_eom({
            let cfg = cfg.clone();
            move |ctx| {
                let cfg = cfg.clone();
                Box::pin(on_eom_cb(ctx, cfg))
            }
        })
        .on_abort(|ctx| Box::pin(on_abort_cb(ctx)))
}

/// Lazy-init state on first touch — indymilter starts with `data = None`.
fn data_mut(ctx: &mut Context<MessageState>) -> &mut MessageState {
    ctx.data.get_or_insert_with(MessageState::default)
}

fn data_mut_eom(ctx: &mut EomContext<MessageState>) -> &mut MessageState {
    ctx.data.get_or_insert_with(MessageState::default)
}

async fn on_mail_cb(ctx: &mut Context<MessageState>, args: Vec<CString>) -> Status {
    if let Some(from) = args.first() {
        if let Ok(s) = from.to_str() {
            data_mut(ctx).sender = Some(strip_angle(s));
        }
    }
    Status::Continue
}

async fn on_rcpt_cb(ctx: &mut Context<MessageState>, args: Vec<CString>) -> Status {
    if let Some(rcpt) = args.first() {
        if let Ok(s) = rcpt.to_str() {
            data_mut(ctx).recipients.push(strip_angle(s));
        }
    }
    Status::Continue
}

async fn on_header_cb(ctx: &mut Context<MessageState>, name: CString, value: CString) -> Status {
    let name_str = name.to_str().unwrap_or("");
    let value_str = value.to_str().unwrap_or("").to_string();
    let state = data_mut(ctx);
    if name_str.eq_ignore_ascii_case("Subject") {
        state.subject = Some(value_str.clone());
    }
    // NOTE: Also preserve the raw header in the body buffer so mailparse can
    //   reconstruct MIME structure (Content-Type, Content-Disposition, etc.)
    //   when extracting attachment filenames at EOM.
    let header_line = format!("{name_str}: {value_str}\r\n");
    state.body.extend_from_slice(header_line.as_bytes());
    Status::Continue
}

async fn on_body_cb(ctx: &mut Context<MessageState>, chunk: Bytes) -> Status {
    let state = data_mut(ctx);
    // NOTE: Cap accumulated body at 25 MiB (matches Postfix message_size_limit).
    //       Larger bodies should already be rejected at the SMTP layer.
    if state.body.is_empty() || !state.body.ends_with(b"\r\n\r\n") {
        // Added: first body chunk needs the blank-line separator after headers
        state.body.extend_from_slice(b"\r\n");
    }
    if state.body.len() + chunk.len() <= 26_214_400 {
        state.body.extend_from_slice(&chunk);
    }
    Status::Continue
}

async fn on_abort_cb(ctx: &mut Context<MessageState>) -> Status {
    ctx.data = None;
    Status::Continue
}

async fn on_eom_cb(ctx: &mut EomContext<MessageState>, cfg: Arc<MilterConfig>) -> Status {
    extract_attachment_filenames(ctx);
    evaluate_and_decide(ctx, cfg).await
}

/// PURPOSE: Pull attachment filenames out of the accumulated body for DLP scan
/// CONSTRAINTS: Uses mailparse; if the body isn't valid MIME the function is
///   a no-op rather than failing the milter.
fn extract_attachment_filenames(ctx: &mut EomContext<MessageState>) {
    let state = data_mut_eom(ctx);
    let Ok(parsed) = mailparse::parse_mail(&state.body) else {
        return;
    };
    let mut names = Vec::new();
    walk_parts(&parsed, &mut names);
    state.attachment_filenames = names;
}

fn walk_parts(part: &mailparse::ParsedMail<'_>, out: &mut Vec<String>) {
    if let Some(name) = part.ctype.params.get("name") {
        out.push(name.clone());
    }
    if let Some(disp) = part.get_headers().get_first_value("Content-Disposition") {
        if let Some(fname) = filename_from_disposition(&disp) {
            out.push(fname);
        }
    }
    for child in &part.subparts {
        walk_parts(child, out);
    }
}

fn filename_from_disposition(disp: &str) -> Option<String> {
    // Added: Minimal parser for "attachment; filename=foo.exe" / filename="foo.exe"
    for token in disp.split(';') {
        let t = token.trim();
        if let Some(rest) = t.strip_prefix("filename=") {
            return Some(rest.trim().trim_matches('"').to_string());
        }
    }
    None
}

/// PURPOSE: Run the DLP rule set against the assembled message and return
///   the milter verdict. Records violations in dlp_violations when we have a
///   user id (best-effort lookup from sender email).
async fn evaluate_and_decide(
    ctx: &mut EomContext<MessageState>,
    cfg: Arc<MilterConfig>,
) -> Status {
    let rules = match load_active_rules(&cfg.pool).await {
        Ok(r) => r,
        Err(e) => {
            warn!(error = ?e, "could not load DLP rules — applying fail policy");
            return if cfg.fail_open { Status::Continue } else { Status::Tempfail };
        }
    };

    let state = data_mut_eom(ctx);
    let body_text = String::from_utf8_lossy(&state.body).to_string();
    let subject = state.subject.clone();
    let filenames = state.attachment_filenames.clone();
    let sender = state.sender.clone();
    let first_rcpt = state.recipients.first().cloned();

    let mut matches = scan_content(&rules, subject.as_deref(), Some(&body_text));
    let blocked_refs: Vec<&str> = cfg.blocked_exts.iter().map(|s| s.as_str()).collect();
    matches.extend(scan_attachments(&filenames, &blocked_refs));

    if matches.is_empty() {
        return Status::Continue;
    }

    let verdict = decide(&matches);
    let _ = persist_violations(&cfg.pool, sender.as_deref(), subject.as_deref(), first_rcpt.as_deref(), &matches).await;

    if matches!(verdict, Status::Continue) {
        // Added: For warn/log actions, tag the message so the recipient MTA
        //   (or downstream filter) can see the DLP outcome without bouncing.
        if let (Ok(name), Ok(value)) = (CString::new("X-Tasmail-DLP"), CString::new("warn")) {
            let _ = ctx.actions.add_header(name, value).await;
        }
    }
    info!(
        match_count = matches.len(),
        verdict = ?verdict,
        sender = ?sender,
        "DLP verdict"
    );
    verdict
}

/// PURPOSE: Translate the strictest matched action into a milter Status
/// CONSTRAINTS: Block > Quarantine > Warn > Log; defaults to Continue
fn decide(matches: &[DlpScanMatch]) -> Status {
    let mut worst: Option<&DlpAction> = None;
    for m in matches {
        let rank = action_rank(&m.action);
        if worst.map(action_rank).unwrap_or(0) < rank {
            worst = Some(&m.action);
        }
    }
    match worst {
        Some(DlpAction::Block) => Status::Reject,
        Some(DlpAction::Quarantine) => Status::Tempfail,
        Some(DlpAction::Warn) | Some(DlpAction::Log) | None => Status::Continue,
    }
}

fn action_rank(a: &DlpAction) -> u8 {
    match a {
        DlpAction::Log => 1,
        DlpAction::Warn => 2,
        DlpAction::Quarantine => 3,
        DlpAction::Block => 4,
    }
}

async fn load_active_rules(pool: &PgPool) -> Result<Vec<DlpRule>, sqlx::Error> {
    sqlx::query_as::<_, DlpRule>("SELECT * FROM dlp_rules WHERE active = true")
        .fetch_all(pool)
        .await
}

async fn persist_violations(
    pool: &PgPool,
    sender: Option<&str>,
    subject: Option<&str>,
    recipient: Option<&str>,
    matches: &[DlpScanMatch],
) -> Result<(), sqlx::Error> {
    // NOTE: Resolve user_id by sender email; if the lookup fails we skip
    //       persistence rather than failing the milter (best-effort logging).
    let Some(sender) = sender else {
        return Ok(());
    };
    let user_id: Option<uuid::Uuid> = sqlx::query_scalar(
        "SELECT id FROM mailboxes WHERE LOWER(email) = LOWER($1) LIMIT 1",
    )
    .bind(sender)
    .fetch_optional(pool)
    .await
    .ok()
    .flatten();
    let Some(uid) = user_id else {
        return Ok(());
    };

    for m in matches {
        if m.rule_id == uuid::Uuid::nil() {
            // Built-in / attachment-extension match — no FK target, skip persistence
            continue;
        }
        sqlx::query(
            "INSERT INTO dlp_violations \
             (rule_id, user_id, action_taken, matched_pattern, matched_text, message_subject, recipient) \
             VALUES ($1, $2, $3, $4, $5, $6, $7)",
        )
        .bind(m.rule_id)
        .bind(uid)
        .bind(&m.action)
        .bind(&m.matched_pattern)
        .bind(&m.matched_text)
        .bind(subject)
        .bind(recipient)
        .execute(pool)
        .await?;
    }
    Ok(())
}

fn strip_angle(addr: &str) -> String {
    addr.trim().trim_start_matches('<').trim_end_matches('>').to_string()
}

#[cfg(test)]
mod tests {
    use super::*;
    use tasmail::models::dlp_rule::DlpSeverity;

    fn mk_match(action: DlpAction) -> DlpScanMatch {
        DlpScanMatch {
            rule_id: uuid::Uuid::nil(),
            rule_name: "test".to_string(),
            action,
            severity: DlpSeverity::Medium,
            matched_pattern: "p".to_string(),
            matched_text: "t".to_string(),
        }
    }

    #[test]
    fn decide_block_beats_warn() {
        let matches = vec![mk_match(DlpAction::Warn), mk_match(DlpAction::Block)];
        assert!(matches!(decide(&matches), Status::Reject));
    }

    #[test]
    fn decide_quarantine_returns_tempfail() {
        let matches = vec![mk_match(DlpAction::Warn), mk_match(DlpAction::Quarantine)];
        assert!(matches!(decide(&matches), Status::Tempfail));
    }

    #[test]
    fn decide_only_warn_returns_continue() {
        let matches = vec![mk_match(DlpAction::Warn), mk_match(DlpAction::Log)];
        assert!(matches!(decide(&matches), Status::Continue));
    }

    #[test]
    fn decide_empty_returns_continue() {
        assert!(matches!(decide(&[]), Status::Continue));
    }

    #[test]
    fn strip_angle_handles_brackets() {
        assert_eq!(strip_angle("<a@b.com>"), "a@b.com");
        assert_eq!(strip_angle("a@b.com"), "a@b.com");
        assert_eq!(strip_angle("  <a@b.com>  "), "a@b.com");
    }

    #[test]
    fn filename_from_disposition_quoted() {
        assert_eq!(
            filename_from_disposition("attachment; filename=\"payload.exe\""),
            Some("payload.exe".to_string())
        );
    }

    #[test]
    fn filename_from_disposition_unquoted() {
        assert_eq!(
            filename_from_disposition("attachment; filename=report.pdf"),
            Some("report.pdf".to_string())
        );
    }

    #[test]
    fn filename_from_disposition_inline_no_filename() {
        assert_eq!(filename_from_disposition("inline"), None);
    }
}
