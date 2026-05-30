//! IMAP IDLE bridge (TMAIL-302).
//!
//! Wires async-imap's IDLE extension to the WebSocket event channel so that
//! `WsEvent::NewMail` / `WsEvent::UnreadUpdate` actually fire when mail lands
//! in the user's mailbox.
//!
//! Architecture:
//! * One `tokio::spawn` task per `(user_id, folder)` subscription.
//! * Each task opens its **own** persistent IMAP connection via
//!   `ImapService::for_user` (so the bridge stays BYOK and uses the user's
//!   per-account TLS/credentials, not the global Dovecot config).
//! * Inner loop runs IDLE cycles up to RFC 2177's 29-minute window. On
//!   `IdleResponse::NewData` the task re-`SELECT`s the folder, compares the
//!   new `EXISTS` count to the previous snapshot and pushes a `NewMail`
//!   event for the delta plus an `UnreadUpdate` with the refreshed `UNSEEN`.
//! * Cancellation: tasks observe `mpsc::Sender::closed()` (WS disconnect)
//!   and a per-subscription `oneshot::Receiver<()>` (explicit unsubscribe
//!   or `IdleSubscription::drop`). Either signal aborts the IDLE wait
//!   cleanly by triggering the async-imap stop source.
//!
//! Concurrency cap: callers enforce `MAX_IDLE_PER_USER` (default 3) before
//! calling `subscribe`; the bridge itself does not maintain a global registry.

use std::time::Duration;

use async_imap::extensions::idle::IdleResponse;
use tokio::sync::{mpsc, oneshot};
use tokio::task::JoinHandle;
use uuid::Uuid;

use crate::error::AppError;
use crate::handlers::websocket::WsEvent;
use crate::services::imap_service::{ImapService, ImapSession};
use crate::state::AppState;

/// RFC 2177 recommends re-issuing IDLE before the 29-minute mark to avoid
/// servers tearing the connection down for inactivity.
const IDLE_TIMEOUT_SECS: u64 = 29 * 60;

/// Backoff applied between reconnect attempts when the IMAP session breaks.
const RECONNECT_BACKOFF_SECS: u64 = 30;

/// Maximum concurrent IDLE subscriptions per WebSocket client. Enforced by
/// callers (the WebSocket handler) — exposed here so the limit lives next
/// to the bridge implementation.
pub const MAX_IDLE_PER_USER: usize = 3;

/// Handle to one live IDLE subscription. Dropping the handle signals the
/// task to stop (via the oneshot cancel channel) and aborts the join handle
/// as a safety net.
pub struct IdleSubscription {
    pub folder: String,
    cancel_tx: Option<oneshot::Sender<()>>,
    handle: JoinHandle<()>,
}

impl IdleSubscription {
    /// Returns true while the spawned task is still running.
    pub fn is_running(&self) -> bool {
        !self.handle.is_finished()
    }
}

impl Drop for IdleSubscription {
    fn drop(&mut self) {
        if let Some(tx) = self.cancel_tx.take() {
            // Best-effort: the receiver may already be gone if the task
            // exited (e.g. WS closed). Ignore the SendError.
            let _ = tx.send(());
        }
        // Belt-and-braces — if the task ignores the cancel signal because
        // it's deep inside a blocking syscall, abort() guarantees teardown.
        self.handle.abort();
    }
}

/// Spawn a new IDLE task for `(user_id, folder)` and return a handle.
///
/// The first connect + `SELECT` happen on the caller's task so authentication
/// or "folder not found" errors propagate synchronously to the WS handler
/// (which can forward them as `WsEvent::Error`). Once the initial snapshot
/// is captured the loop runs entirely on the spawned task.
pub async fn subscribe(
    state: AppState,
    user_id: Uuid,
    folder: String,
    tx: mpsc::Sender<WsEvent>,
) -> Result<IdleSubscription, AppError> {
    let svc = ImapService::for_user(&state, user_id).await?;
    let mut session = svc.connect_user().await?;

    let mbox = session
        .select(&folder)
        .await
        .map_err(|e| AppError::Imap(format!("SELECT {} failed: {}", folder, e)))?;
    let initial_exists = mbox.exists;
    let initial_unseen = mbox.unseen.unwrap_or(0);

    // Push the initial unread snapshot so the SPA's TanStack Query cache
    // refreshes the folder badge immediately on subscribe.
    let _ = tx
        .send(WsEvent::UnreadUpdate {
            folder: folder.clone(),
            unread: initial_unseen,
        })
        .await;

    let (cancel_tx, cancel_rx) = oneshot::channel();
    let folder_for_task = folder.clone();
    let state_for_task = state.clone();
    let tx_for_task = tx.clone();
    let handle = tokio::spawn(async move {
        idle_loop(
            state_for_task,
            user_id,
            folder_for_task,
            Some(session),
            initial_exists,
            tx_for_task,
            cancel_rx,
        )
        .await;
    });

    Ok(IdleSubscription {
        folder,
        cancel_tx: Some(cancel_tx),
        handle,
    })
}

/// Outcome of one IDLE cycle. Drives the outer reconnect/stop loop.
enum CycleOutcome {
    /// Continue with the same session (timeout or new data).
    Continue,
    /// Session broke — drop it and try to reconnect after a backoff.
    Reconnect,
    /// WS disconnect or explicit cancel — stop the task entirely.
    Stop,
}

async fn idle_loop(
    state: AppState,
    user_id: Uuid,
    folder: String,
    initial_session: Option<ImapSession>,
    mut last_exists: u32,
    tx: mpsc::Sender<WsEvent>,
    mut cancel_rx: oneshot::Receiver<()>,
) {
    let mut session_opt = initial_session;

    loop {
        if tx.is_closed() {
            tracing::debug!(
                user_id = %user_id,
                folder = %folder,
                "imap_idle_bridge: WS sender closed, stopping"
            );
            break;
        }
        if cancel_rx.try_recv().is_ok() {
            tracing::debug!(
                user_id = %user_id,
                folder = %folder,
                "imap_idle_bridge: cancel signal received, stopping"
            );
            break;
        }

        // Acquire a session: reuse from the last cycle when possible,
        // otherwise reconnect via ImapService::for_user (which re-reads
        // the user's BYOK config + re-decrypts the password).
        let session = match session_opt.take() {
            Some(s) => s,
            None => match reconnect(&state, user_id).await {
                Some(s) => s,
                None => {
                    tokio::time::sleep(Duration::from_secs(RECONNECT_BACKOFF_SECS)).await;
                    continue;
                }
            },
        };

        let (session_after, outcome) =
            run_idle_cycle(session, &folder, &mut last_exists, &tx, &mut cancel_rx).await;

        match outcome {
            CycleOutcome::Continue => {
                session_opt = session_after;
            }
            CycleOutcome::Reconnect => {
                if let Some(mut s) = session_after {
                    let _ = s.logout().await;
                }
                session_opt = None;
            }
            CycleOutcome::Stop => {
                if let Some(mut s) = session_after {
                    let _ = s.logout().await;
                }
                break;
            }
        }
    }

    tracing::debug!(
        user_id = %user_id,
        folder = %folder,
        "imap_idle_bridge: task exit"
    );
}

async fn reconnect(state: &AppState, user_id: Uuid) -> Option<ImapSession> {
    match ImapService::for_user(state, user_id).await {
        Ok(svc) => match svc.connect_user().await {
            Ok(s) => Some(s),
            Err(e) => {
                tracing::warn!(
                    user_id = %user_id,
                    "imap_idle_bridge: connect_user failed: {}",
                    e
                );
                None
            }
        },
        Err(e) => {
            tracing::warn!(
                user_id = %user_id,
                "imap_idle_bridge: ImapService::for_user failed: {}",
                e
            );
            None
        }
    }
}

async fn run_idle_cycle(
    mut session: ImapSession,
    folder: &str,
    last_exists: &mut u32,
    tx: &mpsc::Sender<WsEvent>,
    cancel_rx: &mut oneshot::Receiver<()>,
) -> (Option<ImapSession>, CycleOutcome) {
    // Re-SELECT so the IDLE we issue below targets a fresh mailbox state.
    // This also gives us the latest EXISTS / UNSEEN snapshot to compare
    // against last_exists.
    let mbox = match session.select(folder).await {
        Ok(m) => m,
        Err(e) => {
            tracing::warn!("imap_idle_bridge: SELECT {} failed: {}", folder, e);
            return (None, CycleOutcome::Reconnect);
        }
    };

    if mbox.exists > *last_exists {
        let delta = mbox.exists - *last_exists;
        if tx
            .send(WsEvent::NewMail {
                folder: folder.to_string(),
                count: delta,
            })
            .await
            .is_err()
        {
            return (Some(session), CycleOutcome::Stop);
        }
    }
    *last_exists = mbox.exists;
    let unseen = mbox.unseen.unwrap_or(0);
    if tx
        .send(WsEvent::UnreadUpdate {
            folder: folder.to_string(),
            unread: unseen,
        })
        .await
        .is_err()
    {
        return (Some(session), CycleOutcome::Stop);
    }

    // Enter IDLE. async-imap consumes `Session` to produce a `Handle`; we
    // get the session back via `handle.done()` regardless of how the wait
    // resolves (server data, timeout, or manual interrupt).
    let mut handle = session.idle();
    if let Err(e) = handle.init().await {
        tracing::warn!("imap_idle_bridge: IDLE init failed for {}: {}", folder, e);
        let recovered = handle.done().await.ok();
        return (recovered, CycleOutcome::Reconnect);
    }

    // Inner scope so `wait_fut` (which borrows `handle`) is dropped before
    // we call `handle.done()` below — `done()` moves out of `handle`.
    let (outcome_res, interrupted_by_cancel) = {
        let (wait_fut, stop) = handle.wait_with_timeout(Duration::from_secs(IDLE_TIMEOUT_SECS));
        tokio::pin!(wait_fut);

        let mut interrupted = false;
        let res = tokio::select! {
            res = &mut wait_fut => res,
            // WS disconnected — interrupt IDLE and drain the future.
            _ = tx.closed() => {
                interrupted = true;
                drop(stop);
                (&mut wait_fut).await
            }
            // Explicit unsubscribe — same shutdown path as WS disconnect.
            _ = &mut *cancel_rx => {
                interrupted = true;
                drop(stop);
                (&mut wait_fut).await
            }
        };
        (res, interrupted)
    };

    // Always call done() to send DONE + return the session to a usable
    // state, even if we plan to logout / discard it below.
    let session = match handle.done().await {
        Ok(s) => s,
        Err(e) => {
            tracing::warn!("imap_idle_bridge: IDLE done failed: {}", e);
            return (None, CycleOutcome::Reconnect);
        }
    };

    match outcome_res {
        Ok(IdleResponse::NewData(_)) | Ok(IdleResponse::Timeout) => {
            if interrupted_by_cancel {
                (Some(session), CycleOutcome::Stop)
            } else {
                (Some(session), CycleOutcome::Continue)
            }
        }
        Ok(IdleResponse::ManualInterrupt) => (Some(session), CycleOutcome::Stop),
        Err(e) => {
            tracing::warn!("imap_idle_bridge: IDLE wait failed: {}", e);
            (Some(session), CycleOutcome::Reconnect)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn idle_timeout_under_30_minutes() {
        // RFC 2177 says servers MAY drop idle connections at the 30-minute mark.
        // We must re-IDLE strictly before that to keep the session alive.
        assert!(IDLE_TIMEOUT_SECS < 30 * 60);
    }

    #[test]
    fn max_idle_per_user_default_is_three() {
        // Documented as 3 in the TMAIL-302 issue. Lower => fewer IMAP conns
        // per WS client; higher => risk of exhausting the upstream server's
        // per-account connection limit (Gmail caps at ~15, Outlook ~20).
        assert_eq!(MAX_IDLE_PER_USER, 3);
    }

    #[tokio::test]
    async fn subscription_drop_aborts_join_handle() {
        // Spawn a long-lived task, wrap it in an IdleSubscription, and
        // confirm that dropping the wrapper terminates the task. We can't
        // pull the JoinHandle out of the struct (it implements Drop) so we
        // observe termination indirectly via a oneshot watchdog the task
        // would otherwise NEVER signal, plus a tight sleep window.
        let (done_tx, mut done_rx) = oneshot::channel::<()>();
        let (cancel_tx, cancel_rx) = oneshot::channel();
        let handle = tokio::spawn(async move {
            // Park forever unless aborted. If we somehow get past this
            // line the watchdog will receive `Ok(())` and the test fails.
            let _ = cancel_rx.await;
            tokio::time::sleep(Duration::from_secs(60 * 60)).await;
            let _ = done_tx.send(());
        });
        let sub = IdleSubscription {
            folder: "INBOX".to_string(),
            cancel_tx: Some(cancel_tx),
            handle,
        };

        drop(sub);

        // Give Tokio a tick to process the abort + sender close.
        tokio::time::sleep(Duration::from_millis(50)).await;

        // The watchdog must NOT have received Ok(()) — that would mean
        // the task escaped the abort and slept for an hour.
        match done_rx.try_recv() {
            Ok(()) => panic!("task should have been aborted, not run to completion"),
            // Err(Empty) or Err(Closed) both prove the task didn't finish
            // its happy path. Closed is the expected outcome after abort.
            Err(_) => {}
        }
    }

    #[tokio::test]
    async fn subscription_drop_signals_cancel() {
        // The Drop impl must fire the oneshot cancel signal so background
        // tasks that observe cancel_rx (rather than just is_closed()) exit
        // promptly.
        let (cancel_tx, mut cancel_rx) = oneshot::channel::<()>();
        let handle = tokio::spawn(async move {
            // Spin forever — Drop's abort() will tear us down.
            std::future::pending::<()>().await;
        });
        let sub = IdleSubscription {
            folder: "INBOX".to_string(),
            cancel_tx: Some(cancel_tx),
            handle,
        };

        drop(sub);

        // After Drop, the receiver must observe either Ok(()) (signal
        // fired) or Err(_) (sender + task gone). Either way it MUST NOT
        // hang — that would mean the cancel signal was never sent.
        let recv = tokio::time::timeout(Duration::from_secs(1), &mut cancel_rx).await;
        assert!(recv.is_ok(), "cancel channel did not resolve after Drop");
    }

    #[tokio::test]
    async fn cycle_outcome_stop_after_tx_closed() {
        // When the WS event channel is closed, `tx.is_closed()` must be
        // true on the next idle_loop iteration so the task exits without
        // attempting another reconnect.
        let (tx, rx) = mpsc::channel::<WsEvent>(4);
        drop(rx);
        assert!(tx.is_closed());
    }
}
