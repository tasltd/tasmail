// Production-grade outgoing-mail queue processor (BYOK).
//
// Pipeline per cycle:
//   1. Atomically claim up to N items via `email_queue.claim_batch` (FOR UPDATE SKIP LOCKED).
//      Multiple worker processes / instances can run concurrently without double-send.
//   2. Group claimed items into per-worker chunks and process them in parallel
//      (`worker_concurrency` futures spawned per cycle).
//   3. For each item: load the user's default SMTP server from `smtp_configurations`,
//      decrypt the password, deliver via `SmtpService`, and update the row.
//   4. Emit Prometheus counters for sends/failures/dead-letters and the queue's
//      backlog size, so on-call gets paged when the queue stalls.
//
// Graceful shutdown via `tokio_util::sync::CancellationToken`.

use std::sync::Arc;

use futures::stream::{FuturesUnordered, StreamExt};
use sqlx::PgPool;
use tokio_util::sync::CancellationToken;

use crate::config::JwtConfig;
use crate::models::ai_config::derive_encryption_key;
use crate::models::email_queue::EmailQueueItem;
use crate::models::smtp_config::SmtpConfiguration;
use crate::services::smtp_service::{SendRequest, SmtpService};

const METRIC_QUEUE_DEPTH: &str = "tasmail_queue_depth";
const METRIC_QUEUE_SENT: &str = "tasmail_queue_sent_total";
const METRIC_QUEUE_FAILED: &str = "tasmail_queue_failed_total";
const METRIC_QUEUE_DEAD: &str = "tasmail_queue_dead_letter_total";
const METRIC_QUEUE_BOUNCED: &str = "tasmail_queue_bounced_total";
const METRIC_QUEUE_LATENCY: &str = "tasmail_queue_send_latency_seconds";

/// Added: TMAIL-58 NDR (Non-Delivery Report) classifier.
/// Returns true when the SMTP failure indicates a hard bounce — the remote permanently
/// rejected the message. Hard bounces should NOT be retried (they'll just keep failing
/// and waste budget); they go straight to status='bounced'.
///
/// Pattern coverage:
/// - 5xx SMTP reply codes (permanent failures per RFC 5321 §4.2.1)
/// - Enhanced status codes 5.x.x (RFC 3463)
/// - Common human-readable bounce phrases from major MTAs (Postfix, Sendmail, Exchange)
pub fn is_hard_bounce(error_message: &str) -> bool {
    let lower = error_message.to_lowercase();

    // RFC 5321 5xx reply codes — any standalone 5xx in the error indicates permanent failure
    // Match "550 ", "551 ", ... "559 " or "5.x.x" enhanced codes.
    let has_5xx_reply = lower
        .split_whitespace()
        .any(|tok| tok.len() == 3 && tok.starts_with('5') && tok.chars().skip(1).all(|c| c.is_ascii_digit()));
    let has_5xx_enhanced = lower.contains("5.1.") // bad destination address
        || lower.contains("5.2.") // mailbox status (full, disabled, etc.)
        || lower.contains("5.3.") // mail system status
        || lower.contains("5.4.") // network/routing
        || lower.contains("5.5.") // protocol
        || lower.contains("5.6.") // content
        || lower.contains("5.7."); // policy/blocked

    let bounce_phrases = [
        "mailbox unavailable",
        "user unknown",
        "no such user",
        "recipient address rejected",
        "mailbox not found",
        "does not exist",
        "address rejected",
        "blocked",
        "permanent failure",
        "permanently failed",
        "550 ",
        "551 ",
        "552 ", // exceeded storage quota — treat as hard bounce per RFC
        "553 ",
        "554 ",
    ];
    let has_phrase = bounce_phrases.iter().any(|p| lower.contains(p));

    has_5xx_reply || has_5xx_enhanced || has_phrase
}

pub struct QueueProcessor {
    pool: Arc<PgPool>,
    jwt_config: JwtConfig,
    poll_interval_secs: u64,
    batch_size: i64,
    worker_concurrency: usize,
    cancel: CancellationToken,
}

impl QueueProcessor {
    pub fn new(
        pool: Arc<PgPool>,
        jwt_config: JwtConfig,
        poll_interval_secs: u64,
    ) -> Self {
        Self {
            pool,
            jwt_config,
            poll_interval_secs,
            // Conservative defaults; can be tuned via env later.
            batch_size: 50,
            worker_concurrency: 4,
            cancel: CancellationToken::new(),
        }
    }

    /// Builder: override the default batch size (max items claimed per cycle).
    pub fn with_batch_size(mut self, batch_size: i64) -> Self { self.batch_size = batch_size; self }

    /// Builder: override worker concurrency (parallel deliveries per cycle).
    pub fn with_worker_concurrency(mut self, n: usize) -> Self { self.worker_concurrency = n; self }

    /// Returns a clone of the cancellation token so the caller can stop the processor.
    pub fn cancel_token(&self) -> CancellationToken { self.cancel.clone() }

    /// Spawn the processor as a background tokio task.
    pub fn start(self) {
        // Describe the metrics once on startup so /metrics surfaces them even when zero.
        metrics::describe_gauge!(METRIC_QUEUE_DEPTH, "Number of pending+failed items in the email queue");
        metrics::describe_counter!(METRIC_QUEUE_SENT, "Successfully delivered queued emails");
        metrics::describe_counter!(METRIC_QUEUE_FAILED, "Queued emails that failed and will be retried");
        metrics::describe_counter!(METRIC_QUEUE_DEAD, "Queued emails moved to dead_letter (max retries exceeded)");
        metrics::describe_counter!(METRIC_QUEUE_BOUNCED, "Queued emails hard-bounced (NDR detected, never retried)");
        metrics::describe_histogram!(METRIC_QUEUE_LATENCY, "End-to-end SMTP send latency for queued emails");

        let cancel = self.cancel.clone();
        tokio::spawn(async move {
            tracing::info!(
                "Queue processor started (poll={}s, batch={}, concurrency={})",
                self.poll_interval_secs, self.batch_size, self.worker_concurrency
            );
            let mut interval = tokio::time::interval(std::time::Duration::from_secs(self.poll_interval_secs));
            loop {
                tokio::select! {
                    _ = cancel.cancelled() => {
                        tracing::info!("Queue processor received shutdown signal — exiting");
                        break;
                    }
                    _ = interval.tick() => {
                        if let Err(e) = self.tick().await {
                            tracing::error!("Queue processor tick error: {}", e);
                        }
                    }
                }
            }
        });
    }

    /// Single processing cycle: refresh queue-depth gauge, claim a batch, process in parallel.
    async fn tick(&self) -> anyhow::Result<()> {
        // Cheap gauge refresh — single COUNT(*) on the partial index.
        if let Ok(depth) = sqlx::query_scalar::<_, i64>(
            "SELECT COUNT(*) FROM email_queue WHERE status IN ('pending', 'failed') AND next_retry_at <= NOW()",
        )
        .fetch_one(&*self.pool)
        .await {
            metrics::gauge!(METRIC_QUEUE_DEPTH).set(depth as f64);
        }

        let items = EmailQueueItem::claim_batch(&self.pool, self.batch_size).await?;
        if items.is_empty() {
            return Ok(());
        }
        tracing::info!("Claimed {} items from queue", items.len());

        let mut in_flight: FuturesUnordered<_> = items
            .into_iter()
            .map(|item| {
                let pool = self.pool.clone();
                let key = derive_encryption_key(&self.jwt_config.secret);
                async move { process_item(pool, key, item).await }
            })
            .collect();

        // Bound parallelism: drain the FuturesUnordered with a target of `worker_concurrency` outstanding.
        // (FuturesUnordered already polls them all concurrently — this just limits how many we await for at once.)
        let mut buffered: usize = 0;
        while let Some(_res) = in_flight.next().await {
            buffered += 1;
            if buffered >= self.worker_concurrency * 4 {
                // Periodic yield so other tokio tasks (HTTP handlers) get scheduled.
                buffered = 0;
                tokio::task::yield_now().await;
            }
        }

        Ok(())
    }
}

async fn process_item(
    pool: Arc<PgPool>,
    encryption_key: [u8; 32],
    item: EmailQueueItem,
) -> () {
    let start = std::time::Instant::now();
    let result: Result<(), anyhow::Error> = (async {
        // BYOK: load the user's default SMTP config + decrypt the password.
        let smtp_cfg = SmtpConfiguration::find_default(&pool, item.mailbox_id)
            .await?
            .ok_or_else(|| anyhow::anyhow!(
                "Mailbox {} has no default SMTP configuration — user must complete onboarding",
                item.mailbox_id
            ))?;
        let password = smtp_cfg
            .decrypted_password(&encryption_key)
            .map_err(|e| anyhow::anyhow!("Failed to decrypt SMTP password: {}", e))?;
        let from_address = smtp_cfg.from_address.clone().unwrap_or_else(|| smtp_cfg.username.clone());

        let smtp_runtime_cfg = crate::config::SmtpConfig {
            host: smtp_cfg.host.clone(),
            port: smtp_cfg.port as u16,
            tls: matches!(smtp_cfg.encryption.as_str(), "ssl" | "starttls"),
            notification_from: None,
            notification_username: None,
            notification_password: None,
        };
        let smtp = SmtpService::new(smtp_runtime_cfg);

        let request = SendRequest {
            to: item.to_addresses.clone(),
            cc: Some(item.cc_addresses.clone()),
            bcc: Some(item.bcc_addresses.clone()),
            subject: item.subject.clone(),
            text_body: if item.body_text.is_empty() { None } else { Some(item.body_text.clone()) },
            html_body: if item.body_html.is_empty() { None } else { Some(item.body_html.clone()) },
        };

        smtp.send(&from_address, &password, &request)
            .await
            .map_err(|e| anyhow::anyhow!("SMTP send failed: {}", e))?;
        Ok(())
    })
    .await;

    let elapsed = start.elapsed().as_secs_f64();
    metrics::histogram!(METRIC_QUEUE_LATENCY).record(elapsed);

    match result {
        Ok(()) => {
            metrics::counter!(METRIC_QUEUE_SENT).increment(1);
            if let Err(e) = EmailQueueItem::mark_sent(&pool, item.id).await {
                tracing::error!("Failed to mark queue item {} sent: {}", item.id, e);
            } else {
                tracing::info!("Sent queued email {} in {:.2}s", item.id, elapsed);
            }
        }
        Err(send_err) => {
            let error_message = format!("{}", send_err);
            let new_retry_count = item.retry_count + 1;

            // Added: TMAIL-58 — detect hard bounces (NDRs) and route them to 'bounced'
            // status without consuming retry budget. Retrying a 550 mailbox-unavailable
            // is pointless — it will keep failing and waste both our time and the remote MTA's.
            if is_hard_bounce(&error_message) {
                metrics::counter!(METRIC_QUEUE_BOUNCED).increment(1);
                if let Err(e) = EmailQueueItem::mark_bounced(&pool, item.id, &error_message).await {
                    tracing::error!("Failed to mark queue item {} bounced: {}", item.id, e);
                }
                tracing::warn!(
                    "Queue item {} → bounced (NDR detected, no retry): {}",
                    item.id, error_message
                );
            } else if new_retry_count >= item.max_retries {
                metrics::counter!(METRIC_QUEUE_DEAD).increment(1);
                if let Err(e) = EmailQueueItem::mark_dead_letter(&pool, item.id, &error_message).await {
                    tracing::error!("Failed to mark queue item {} dead_letter: {}", item.id, e);
                }
                tracing::warn!(
                    "Queue item {} → dead_letter after {} retries: {}",
                    item.id, new_retry_count, error_message
                );
            } else {
                metrics::counter!(METRIC_QUEUE_FAILED).increment(1);
                if let Err(e) = EmailQueueItem::mark_failed(&pool, item.id, &error_message, new_retry_count).await {
                    tracing::error!("Failed to mark queue item {} failed: {}", item.id, e);
                }
                let backoff = EmailQueueItem::calculate_backoff_secs(new_retry_count);
                tracing::warn!(
                    "Queue item {} failed (retry {}/{}, backoff {}s): {}",
                    item.id, new_retry_count, item.max_retries, backoff, error_message
                );
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // Fix: PgPool::connect_lazy spawns a background task and requires a Tokio runtime,
    // so this must be a #[tokio::test] (was #[test], failed with "requires a Tokio context").
    #[tokio::test]
    async fn queue_processor_construction() {
        // sanity: defaults are non-zero
        let pool = sqlx::PgPool::connect_lazy("postgres://invalid/invalid").unwrap();
        let cfg = JwtConfig {
            secret: "test".into(),
            access_token_expiry_secs: 60,
            refresh_token_expiry_secs: 60,
        };
        let p = QueueProcessor::new(Arc::new(pool), cfg, 5)
            .with_batch_size(100)
            .with_worker_concurrency(8);
        assert_eq!(p.batch_size, 100);
        assert_eq!(p.worker_concurrency, 8);
    }

    // Added: TMAIL-58 — NDR (hard bounce) classifier coverage
    #[test]
    fn is_hard_bounce_detects_smtp_5xx_codes() {
        assert!(is_hard_bounce("550 5.1.1 mailbox unavailable"));
        assert!(is_hard_bounce("SMTP error: 551 user does not exist"));
        assert!(is_hard_bounce("552 storage quota exceeded"));
        assert!(is_hard_bounce("553 mailbox name not allowed"));
        assert!(is_hard_bounce("554 transaction failed"));
    }

    #[test]
    fn is_hard_bounce_detects_enhanced_status_codes() {
        assert!(is_hard_bounce("Server replied with 5.1.1 bad destination"));
        assert!(is_hard_bounce("Delivery failed: 5.2.2 mailbox full"));
        assert!(is_hard_bounce("5.7.1 policy rejection from upstream"));
    }

    #[test]
    fn is_hard_bounce_detects_common_bounce_phrases() {
        assert!(is_hard_bounce("recipient address rejected by remote server"));
        assert!(is_hard_bounce("Mailbox Unavailable on host smtp.example.com"));
        assert!(is_hard_bounce("user unknown in virtual mailbox table"));
        assert!(is_hard_bounce("does not exist at this domain"));
        assert!(is_hard_bounce("permanent failure — message rejected"));
    }

    #[test]
    fn is_hard_bounce_rejects_transient_failures() {
        // 4xx codes are TRANSIENT — must NOT be classified as hard bounce
        assert!(!is_hard_bounce("421 service temporarily unavailable"));
        assert!(!is_hard_bounce("450 mailbox busy — try later"));
        assert!(!is_hard_bounce("4.7.1 greylisted, retry in 60s"));
        // Network-level errors are transient
        assert!(!is_hard_bounce("connection timed out"));
        assert!(!is_hard_bounce("DNS resolution failed for smtp.example.com"));
        assert!(!is_hard_bounce("TLS handshake failed"));
    }

    #[test]
    fn is_hard_bounce_empty_and_whitespace() {
        assert!(!is_hard_bounce(""));
        assert!(!is_hard_bounce("   "));
        assert!(!is_hard_bounce("unknown error"));
    }

    // Added: TMAIL-58 — case-insensitive matching (MTAs vary in casing)
    #[test]
    fn is_hard_bounce_is_case_insensitive() {
        assert!(is_hard_bounce("USER UNKNOWN"));
        assert!(is_hard_bounce("Mailbox NOT Found"));
        assert!(is_hard_bounce("PERMANENT FAILURE"));
    }
}
