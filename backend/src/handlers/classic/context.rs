// Added (TMAIL-384): Shared per-request "context loader" for the no-JS
// `/classic` surface. Owns the `QuotaIndicator` view-model + the function
// every authenticated handler calls to hydrate the footer's "Using X of Y"
// line.
//
// WHY THIS LIVES HERE
// -------------------
// The base layout (`templates/classic/base.html`) renders a small quota
// indicator in the footer (and is themed through CSS-only `<meter>`
// classes for SR friendliness — see P1 #30 in `docs/gap-analysis/classic-ui.md`).
// Askama is compile-time, so a polymorphic "any-template-with-a-quota-field"
// trick is impossible — instead, every authenticated template struct
// (`FolderTemplate`, `MessageTemplate`, etc.) carries the SAME
// `quota_indicator: Option<QuotaIndicator>` field shape, and each child
// template overrides `{% block quota_indicator %}{% include
// "classic/_quota_indicator.html" %}{% endblock %}`. The include sees the
// outer template's field by name — that's why the field name is locked
// down in this module so every handler agrees on it.
//
// CACHING
// -------
// The loader checks `state.cache.get_quota::<QuotaStatus>(&user_id)` first
// (existing Redis namespace used by `GET /api/quota`). On a hit it skips
// the DB entirely; on a miss it fetches `Mailbox::find_by_id` +
// `QuotaUsage::find_by_mailbox`, builds the canonical `QuotaStatus`, writes
// it back to the cache (so the next /classic page view AND the next
// /api/quota call hit warm), and converts it to a `QuotaIndicator`.
// When Redis is down (`disabled` mode or transport error) the cache layer
// returns `None`/`false` gracefully and we fall through to the DB — the
// two Postgres queries are sub-ms with the existing indexes, so an
// in-process fallback cache would be premature optimisation. The shared
// quota_ttl_secs default is 60s; the issue spec suggests ~30s, but the
// existing convention (and the same TTL used by `/api/quota`) is fine —
// the cache invalidates on any sync via `POST /api/quota/sync`.
//
// FAILURE POLICY
// --------------
// On ANY database / cache error the loader returns `None` and logs at
// `warn!`. A missing footer indicator is strictly less harmful than a
// 500 on every page view — the rest of the page still renders. The DB
// path runs after `classic_session_middleware`, so the mailbox row is
// known to exist; a `None` from the loader therefore really only happens
// during a Postgres outage.

use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::models::mailbox::Mailbox;
use crate::models::quota::{QuotaStatus, QuotaUsage};
use crate::state::AppState;

/// 80% — yellow "warning" tier on the footer indicator. Below this the
/// indicator renders in the neutral muted colour. Pulled out as a named
/// constant so the unit tests can lock the threshold down.
pub const QUOTA_WARNING_THRESHOLD_PERCENT: f64 = 80.0;

/// 95% — red "danger" tier. At/above this the indicator renders in
/// `--tm-danger` and bumps the underlying `<meter>` to `optimum`-low
/// semantics so assistive tech surfaces the same urgency.
pub const QUOTA_DANGER_THRESHOLD_PERCENT: f64 = 95.0;

/// View-model handed to the `_quota_indicator.html` partial. Carries
/// pre-formatted display strings + the numeric percent so the template
/// can render a `<meter>` element without doing any maths of its own.
///
/// Every field is owned `String` / primitive — no borrows / Cow — because
/// the template lives across an `.await` boundary in the handler chain.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QuotaIndicator {
    /// Human-readable used bytes (e.g. `"1.4 GB"`).
    pub used_display: String,
    /// Human-readable quota ceiling (e.g. `"10 GB"`).
    pub quota_display: String,
    /// 0..=100 — what the `<meter value=...>` attribute renders to. We
    /// `clamp` the raw percent so an over-quota mailbox still renders a
    /// full bar instead of overflowing.
    pub percent: f64,
    /// 0..=100 rounded to a whole number for the visible label
    /// ("Using 1.4 GB of 10 GB · 14%") — keeps the footer line stable
    /// regardless of locale-specific float formatting.
    pub percent_label: u32,
    /// Severity tier — drives the CSS class on the wrapper. The template
    /// branches on `is_warning` / `is_danger` (no need to compare strings).
    pub is_warning: bool,
    /// `true` when `percent >= QUOTA_DANGER_THRESHOLD_PERCENT`. Implies
    /// `is_warning == true` (always set together when entering the
    /// danger tier).
    pub is_danger: bool,
    /// `true` when the underlying mailbox has no quota set (`quota_bytes
    /// == 0`). The template hides the percent + meter in this case and
    /// just shows "Using X" so a misconfigured / unlimited mailbox doesn't
    /// render a misleading 0%.
    pub unlimited: bool,
}

impl QuotaIndicator {
    /// Build a `QuotaIndicator` view-model from the canonical `QuotaStatus`
    /// the API layer + the loader function below produce.
    ///
    /// Centralising the conversion here means the API endpoint and the
    /// Classic UI footer render exactly the same "1.4 GB of 10 GB · 14%"
    /// text — and the unit tests can validate the mapping without going
    /// through Askama.
    pub fn from_status(status: &QuotaStatus) -> Self {
        let unlimited = status.quota_bytes == 0;
        let raw_percent = if unlimited {
            0.0
        } else {
            status.usage_percent
        };
        let clamped = raw_percent.clamp(0.0, 100.0);
        let is_warning = !unlimited && raw_percent >= QUOTA_WARNING_THRESHOLD_PERCENT;
        let is_danger = !unlimited && raw_percent >= QUOTA_DANGER_THRESHOLD_PERCENT;
        // Round-half-up to nearest whole percent for the label. Skip the
        // label entirely on unlimited so we don't emit a confusing "0%".
        let percent_label = if unlimited {
            0
        } else {
            raw_percent.round().clamp(0.0, 999.0) as u32
        };

        QuotaIndicator {
            used_display: format_bytes(status.used_bytes),
            quota_display: format_bytes(status.quota_bytes),
            percent: clamped,
            percent_label,
            is_warning,
            is_danger,
            unlimited,
        }
    }
}

/// Format a byte count as a short human-readable string. Uses binary
/// prefixes (1 KiB = 1024 B) consistent with what the SPA's quota bar +
/// the `/api/quota` JSON consumers display — sticking with one convention
/// across surfaces prevents "30 GB" on one screen and "32 GiB" on another.
///
/// Returns `0 B` for the empty case so the footer never renders the bare
/// word "0" (which would look like a bug at a glance).
pub fn format_bytes(bytes: i64) -> String {
    if bytes <= 0 {
        return "0 B".to_string();
    }
    let bytes_f = bytes as f64;
    const UNITS: &[&str] = &["B", "KB", "MB", "GB", "TB", "PB"];
    let mut value = bytes_f;
    let mut unit_idx = 0;
    while value >= 1024.0 && unit_idx + 1 < UNITS.len() {
        value /= 1024.0;
        unit_idx += 1;
    }
    if unit_idx == 0 {
        // Bytes never get a decimal — "42 B" reads cleanly; "42.0 B"
        // looks like spurious precision.
        format!("{} {}", bytes, UNITS[0])
    } else if value >= 100.0 {
        format!("{:.0} {}", value, UNITS[unit_idx])
    } else if value >= 10.0 {
        format!("{:.1} {}", value, UNITS[unit_idx])
    } else {
        format!("{:.2} {}", value, UNITS[unit_idx])
    }
}

/// Per-request loader for the footer quota indicator.
///
/// * Hits the Redis cache first (`state.cache.get_quota`) — exact same
///   namespace as `GET /api/quota`, so the two surfaces share a single
///   warm path.
/// * On a cache miss runs the same DB queries the API endpoint does
///   (`Mailbox::find_by_id` + `QuotaUsage::find_by_mailbox`), assembles
///   a `QuotaStatus`, writes it back to the cache, then converts to the
///   view-model.
/// * Returns `None` on ANY error so an outage on the cache or the DB
///   doesn't take down the whole footer. The handler chain will still
///   render the page — the footer just omits the indicator line for
///   that request.
pub async fn load_quota_indicator(
    state: &AppState,
    mailbox_id: Uuid,
) -> Option<QuotaIndicator> {
    let id_str = mailbox_id.to_string();

    // Cache hit — short-circuit. No DB, no IMAP — matches the wording in
    // the issue ("avoid a synchronous quota fetch on every request").
    if let Some(cached) = state.cache.get_quota::<QuotaStatus>(&id_str).await {
        return Some(QuotaIndicator::from_status(&cached));
    }

    // Cache miss — fall through to the DB. Both queries are sub-ms with
    // the existing PK / FK indexes on `mailboxes` and `quota_usage`.
    let mailbox = match Mailbox::find_by_id(&state.db, mailbox_id).await {
        Ok(Some(m)) => m,
        Ok(None) => {
            tracing::warn!(
                ?mailbox_id,
                "classic quota loader: mailbox not found — skipping footer indicator"
            );
            return None;
        }
        Err(e) => {
            tracing::warn!(
                ?mailbox_id,
                error = %e,
                "classic quota loader: mailbox lookup failed — skipping footer indicator"
            );
            return None;
        }
    };

    let usage = match QuotaUsage::find_by_mailbox(&state.db, mailbox_id).await {
        Ok(opt) => opt,
        Err(e) => {
            tracing::warn!(
                ?mailbox_id,
                error = %e,
                "classic quota loader: usage lookup failed — skipping footer indicator"
            );
            return None;
        }
    };

    let status = QuotaUsage::to_status(
        usage.as_ref(),
        mailbox.quota_bytes,
        mailbox.quota_warn_percent,
        mailbox_id,
    );

    // Warm the cache so the next page view (and the next /api/quota call)
    // hit Redis directly. Failure here is non-fatal — Redis-down just
    // means we'll re-fetch from the DB next request.
    state.cache.set_quota(&id_str, &status).await;

    Some(QuotaIndicator::from_status(&status))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn thresholds_match_spec() {
        // Locked in by a test because the issue spec calls these out
        // explicitly ("warning at 80%, danger at 95%"). A future drift
        // here without updating the spec should fail the build.
        assert_eq!(QUOTA_WARNING_THRESHOLD_PERCENT, 80.0);
        assert_eq!(QUOTA_DANGER_THRESHOLD_PERCENT, 95.0);
    }

    #[test]
    fn format_bytes_zero_renders_human_label() {
        // The footer should never render the bare digit "0" — pin the
        // unit label down so a future refactor can't quietly drop it.
        assert_eq!(format_bytes(0), "0 B");
        assert_eq!(format_bytes(-1), "0 B");
    }

    #[test]
    fn format_bytes_uses_binary_prefixes() {
        // Binary (1024) — same convention as the SPA quota bar.
        assert_eq!(format_bytes(512), "512 B");
        assert_eq!(format_bytes(1024), "1.00 KB");
        assert_eq!(format_bytes(1536), "1.50 KB");
        assert_eq!(format_bytes(1024 * 1024), "1.00 MB");
        assert_eq!(format_bytes(1024 * 1024 * 1024), "1.00 GB");
        // 10+ unit drops to one decimal; 100+ to none.
        assert_eq!(format_bytes(15 * 1024 * 1024), "15.0 MB");
        assert_eq!(format_bytes(250 * 1024 * 1024), "250 MB");
    }

    #[test]
    fn format_bytes_terabyte_scale() {
        // A 2 TB mailbox shouldn't blow past the unit table.
        let two_tb = 2_i64 * 1024 * 1024 * 1024 * 1024;
        assert_eq!(format_bytes(two_tb), "2.00 TB");
    }

    fn status(used: i64, quota: i64) -> QuotaStatus {
        let mid = Uuid::new_v4();
        let usage = if used == 0 {
            None
        } else {
            Some(QuotaUsage {
                id: Uuid::new_v4(),
                mailbox_id: mid,
                used_bytes: used,
                message_count: 0,
                last_synced_at: chrono::Utc::now(),
            })
        };
        QuotaUsage::to_status(usage.as_ref(), quota, 80, mid)
    }

    #[test]
    fn indicator_ok_tier_below_warning_threshold() {
        // 50% — well under both thresholds.
        let s = status(5 * 1024 * 1024 * 1024, 10 * 1024 * 1024 * 1024);
        let ind = QuotaIndicator::from_status(&s);
        assert!(!ind.is_warning, "50% must not trigger warning");
        assert!(!ind.is_danger, "50% must not trigger danger");
        assert!(!ind.unlimited);
        assert_eq!(ind.percent_label, 50);
        assert_eq!(ind.used_display, "5.00 GB");
        assert_eq!(ind.quota_display, "10.0 GB");
    }

    #[test]
    fn indicator_warning_tier_at_80_percent() {
        // Exactly 80% — locks the boundary. Inclusive is the spec
        // ("warning at 80%"), so 80.0 must trigger warning.
        let quota = 10_000_000_000;
        let used = 8_000_000_000; // 80%
        let s = status(used, quota);
        let ind = QuotaIndicator::from_status(&s);
        assert!(ind.is_warning, "80% must trigger warning (inclusive)");
        assert!(!ind.is_danger, "80% must not trigger danger yet");
    }

    #[test]
    fn indicator_warning_tier_just_below_80_percent_is_ok() {
        let quota = 10_000_000_000;
        let used = 7_999_000_000; // ~79.99%
        let s = status(used, quota);
        let ind = QuotaIndicator::from_status(&s);
        assert!(!ind.is_warning, "below 80% must NOT trigger warning");
        assert!(!ind.is_danger);
    }

    #[test]
    fn indicator_danger_tier_at_95_percent() {
        // 95% — danger boundary, also still warning.
        let quota = 10_000_000_000;
        let used = 9_500_000_000;
        let s = status(used, quota);
        let ind = QuotaIndicator::from_status(&s);
        assert!(ind.is_warning, "95% must also satisfy warning");
        assert!(ind.is_danger, "95% must trigger danger (inclusive)");
    }

    #[test]
    fn indicator_over_quota_clamps_meter_value() {
        // 120% used — the `<meter value=...>` must stay <= 100 so the bar
        // doesn't visually overflow, but the percent_label keeps the raw
        // value so the user sees how far over they are.
        let quota = 1_000_000_000;
        let used = 1_200_000_000;
        let s = status(used, quota);
        let ind = QuotaIndicator::from_status(&s);
        assert!(ind.is_warning);
        assert!(ind.is_danger);
        assert!(ind.percent <= 100.0, "meter value must clamp to <=100");
        assert!(ind.percent_label >= 120, "label preserves raw percent");
    }

    #[test]
    fn indicator_unlimited_mailbox_renders_no_percent() {
        // quota_bytes = 0 means unlimited in this codebase
        // (`to_status` returns usage_percent = 0 in that case).
        let s = status(5 * 1024 * 1024 * 1024, 0);
        let ind = QuotaIndicator::from_status(&s);
        assert!(ind.unlimited, "0-quota mailbox must flag unlimited");
        assert!(!ind.is_warning, "unlimited never warns");
        assert!(!ind.is_danger, "unlimited never danger");
        assert_eq!(ind.percent_label, 0);
        assert_eq!(ind.used_display, "5.00 GB");
        // quota_display still renders for completeness, but the template
        // hides it when unlimited == true.
        assert_eq!(ind.quota_display, "0 B");
    }

    #[test]
    fn indicator_empty_mailbox_renders_zero_used() {
        // Brand-new mailbox: usage row absent, quota set.
        let s = status(0, 1_000_000_000);
        let ind = QuotaIndicator::from_status(&s);
        assert!(!ind.unlimited);
        assert!(!ind.is_warning);
        assert!(!ind.is_danger);
        assert_eq!(ind.percent_label, 0);
        assert_eq!(ind.used_display, "0 B");
    }
}
