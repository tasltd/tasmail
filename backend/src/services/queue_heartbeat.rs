// Added (TMAIL-310): Shared queue-processor liveness heartbeat.
//
// PURPOSE: The outgoing-mail queue processor calls `record_tick()` at the
// start of every cycle. The readiness probe in `handlers::health` reads the
// timestamp to detect a stalled processor — e.g. its tokio task has panicked,
// its DB pool is exhausted, or it is waiting on a hung await indefinitely.
//
// CONSTRAINTS:
//   * No locks on the read path — the gauge is a plain `AtomicI64` storing
//     Unix seconds. Readers and writers never block each other.
//   * `Clone` is cheap (Arc clone) so the heartbeat can be stamped into both
//     `AppState` (for the HTTP probe) and `QueueProcessor` (for the writer).
//   * Sentinel value `0` means "never ticked" — `last_tick()` returns `None`
//     in that case so the probe can distinguish "starting up" from "ticked
//     a long time ago".

use std::sync::Arc;
use std::sync::atomic::{AtomicI64, Ordering};

/// PURPOSE: Lock-free liveness gauge for the background queue processor.
#[derive(Clone, Debug)]
pub struct QueueHeartbeat {
    last_tick_unix: Arc<AtomicI64>,
}

impl Default for QueueHeartbeat {
    fn default() -> Self {
        Self::new()
    }
}

impl QueueHeartbeat {
    /// PURPOSE: Construct a fresh heartbeat — `last_tick()` returns `None`
    /// until `record_tick()` is called.
    pub fn new() -> Self {
        Self { last_tick_unix: Arc::new(AtomicI64::new(0)) }
    }

    /// PURPOSE: Record a successful processor cycle. Called from
    /// `QueueProcessor::tick`. Cheap — single atomic store.
    pub fn record_tick(&self) {
        let now = chrono::Utc::now().timestamp();
        self.last_tick_unix.store(now, Ordering::Relaxed);
    }

    /// PURPOSE: Return the wall-clock instant of the most recent tick, or
    /// `None` if the processor has never ticked.
    pub fn last_tick(&self) -> Option<chrono::DateTime<chrono::Utc>> {
        let ts = self.last_tick_unix.load(Ordering::Relaxed);
        if ts == 0 {
            None
        } else {
            chrono::DateTime::<chrono::Utc>::from_timestamp(ts, 0)
        }
    }

    /// PURPOSE: Seconds since the most recent tick. `None` when the processor
    /// has never ticked — the readiness probe surfaces that as "not_started".
    pub fn seconds_since_tick(&self) -> Option<i64> {
        self.last_tick().map(|t| (chrono::Utc::now() - t).num_seconds().max(0))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn never_ticked_returns_none() {
        let hb = QueueHeartbeat::new();
        assert!(hb.last_tick().is_none());
        assert!(hb.seconds_since_tick().is_none());
    }

    #[test]
    fn record_tick_updates_timestamp() {
        let hb = QueueHeartbeat::new();
        hb.record_tick();
        let t1 = hb.last_tick().expect("should be set after record_tick");
        assert!(t1.timestamp() > 0);
        let secs = hb.seconds_since_tick().expect("should be Some after tick");
        // seconds_since_tick is computed with chrono::Utc::now() which advances
        // strictly forward, so it must be non-negative and a small number.
        assert!(secs >= 0 && secs < 5, "seconds_since_tick={}", secs);
    }

    #[test]
    fn clone_shares_same_state() {
        let hb = QueueHeartbeat::new();
        let hb2 = hb.clone();
        hb.record_tick();
        // The clone observes the same store because both wrap the same Arc.
        assert!(hb2.last_tick().is_some());
    }

    #[test]
    fn record_tick_is_monotonic_within_a_second() {
        let hb = QueueHeartbeat::new();
        hb.record_tick();
        let first = hb.last_tick().unwrap();
        hb.record_tick();
        let second = hb.last_tick().unwrap();
        assert!(second >= first);
    }
}
