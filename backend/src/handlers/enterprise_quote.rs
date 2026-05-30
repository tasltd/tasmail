// TMAIL-182: enterprise quote-request endpoint.
//
// Public endpoint — anyone can POST a request to talk to sales. Rate-limited
// per IP via the existing cache_service::check_rate_limit(client_ip) helper to
// keep the form from being abused. On success: writes a row to
// enterprise_quote_requests, fires a notification email through the noreply
// SmtpService::send_notification() path so sales gets paged immediately, and
// returns a tracking id the form shows back to the requester.

use axum::{
    extract::{ConnectInfo, State},
    http::{HeaderMap, StatusCode},
    Json,
};
use serde::{Deserialize, Serialize};
use std::net::SocketAddr;
use uuid::Uuid;

use crate::error::AppError;
use crate::services::smtp_service::{SendRequest, SmtpService};
use crate::state::AppState;

#[derive(Debug, Deserialize)]
pub struct QuoteRequest {
    pub contact_name: String,
    pub contact_email: String,
    #[serde(default)]
    pub company: Option<String>,
    #[serde(default)]
    pub estimated_users: Option<i32>,
    pub message: String,
}

#[derive(Debug, Serialize)]
pub struct QuoteResponse {
    pub id: Uuid,
    pub status: String,
}

const SALES_INBOX: &str = "hello@techatscale.io";
const MAX_FIELD_LEN: usize = 4_000;

/// POST /api/enterprise/quote-request
pub async fn submit_quote_request(
    State(state): State<AppState>,
    ConnectInfo(addr): ConnectInfo<SocketAddr>,
    headers: HeaderMap,
    Json(body): Json<QuoteRequest>,
) -> Result<(StatusCode, Json<QuoteResponse>), AppError> {
    // ---- validation ----
    let name = body.contact_name.trim();
    let email = body.contact_email.trim().to_lowercase();
    let message = body.message.trim();

    if name.is_empty() || name.len() > 200 {
        return Err(AppError::BadRequest("contact_name is required (1-200 chars)".into()));
    }
    if email.is_empty() || !email.contains('@') || email.len() > 320 {
        return Err(AppError::BadRequest("contact_email must be a valid email address".into()));
    }
    if message.is_empty() || message.len() > MAX_FIELD_LEN {
        return Err(AppError::BadRequest(format!("message is required (1-{} chars)", MAX_FIELD_LEN)));
    }
    if let Some(n) = body.estimated_users {
        if !(1..=1_000_000).contains(&n) {
            return Err(AppError::BadRequest("estimated_users must be 1..=1_000_000".into()));
        }
    }

    // ---- rate limit by IP ----
    // Honour X-Forwarded-For when behind the Apache reverse proxy so we don't
    // throttle the entire fleet through a single per-loopback bucket.
    let client_ip = headers
        .get("x-forwarded-for")
        .and_then(|v| v.to_str().ok())
        .and_then(|s| s.split(',').next())
        .map(|s| s.trim().to_string())
        .unwrap_or_else(|| addr.ip().to_string());

    if !state.cache.check_rate_limit(&format!("eqr:{}", client_ip)).await {
        return Err(AppError::BadRequest(
            "Too many quote requests from your IP. Please retry later or email hello@techatscale.io directly.".into(),
        ));
    }

    let user_agent = headers
        .get("user-agent")
        .and_then(|v| v.to_str().ok())
        .map(String::from);

    // ---- persist ----
    let row = sqlx::query_as::<_, (Uuid, String)>(
        "INSERT INTO enterprise_quote_requests
            (contact_name, contact_email, company, estimated_users, message, source_ip, user_agent)
         VALUES ($1, $2, $3, $4, $5, $6::inet, $7)
         RETURNING id, status",
    )
    .bind(name)
    .bind(&email)
    .bind(body.company.as_deref().map(str::trim).filter(|s| !s.is_empty()))
    .bind(body.estimated_users)
    .bind(message)
    .bind(&client_ip)
    .bind(user_agent)
    .fetch_one(&state.db)
    .await?;

    // ---- fire-and-forget sales notification ----
    // Done inline (not spawned) so we keep the request handler simple, but
    // SMTP failures don't fail the form submission — the quote still landed
    // in the table for admins to see in /admin/quote-requests.
    let smtp = SmtpService::new(state.config.smtp.clone());
    let summary = format!(
        "From: {name} <{email}>\nCompany: {company}\nEstimated users: {users}\nIP: {ip}\n\n---\n\n{message}\n\n---\nQuote tracking id: {id}\nReply to {email} or open the admin dashboard at https://mail.techatscale.io/admin/quote-requests",
        name = name,
        email = email,
        company = body.company.as_deref().unwrap_or("(none)"),
        users = body.estimated_users.map(|n| n.to_string()).unwrap_or_else(|| "(unspecified)".to_string()),
        ip = client_ip,
        message = message,
        id = row.0,
    );
    let _ = smtp
        .send_notification(&SendRequest {
            to: vec![SALES_INBOX.to_string()],
            cc: None,
            bcc: None,
            subject: format!("New TASMail Enterprise quote request — {name}"),
            text_body: Some(summary),
            html_body: None,
            // Notifications are not part of a thread.
            in_reply_to: None,
            references: None,
        })
        .await
        .map_err(|e| {
            tracing::warn!(
                "Quote request {} stored but sales notification email failed: {}",
                row.0, e
            );
            e
        });

    Ok((StatusCode::CREATED, Json(QuoteResponse { id: row.0, status: row.1 })))
}
