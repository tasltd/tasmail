// TMAIL-180: pure invoice-math helpers, isolated from sqlx so we can run them
// in pure Rust unit tests without a Postgres harness.
//
// Pricing model (TMAIL-176):
//   amount = max(monthly_min, ceil(storage_gb) * ghs_per_gb)
//
// We round storage *up* to the next whole GB so a 0.4 GB mailbox is billed for
// 1 GB, matching every cloud provider's "per-GB" convention. The monthly
// minimum kicks in when usage is so low the rate would otherwise undercharge.

const BYTES_PER_GB: f64 = 1_073_741_824.0; // 2^30 — binary GB, matches `quota_usage.used_bytes`

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct InvoiceAmount {
    /// Computed amount in GHS, rounded to two decimals.
    pub amount_ghs: f64,
    /// True when the monthly minimum was higher than the per-GB calc and clamped the result.
    pub minimum_applied: bool,
    /// Number of whole GB the invoice is built on (storage rounded up).
    pub billed_gb: u64,
}

/// PURPOSE: Invoice math. Called by both the rollup loop (writes the result
/// to billing_invoices) and the in-app /api/billing/usage endpoint (shows the
/// running projection to the user).
pub fn compute_invoice_ghs(avg_storage_bytes: i64, ghs_per_gb: f64, monthly_min: f64) -> InvoiceAmount {
    let bytes = avg_storage_bytes.max(0) as f64;
    let raw_gb = bytes / BYTES_PER_GB;
    let billed_gb = raw_gb.ceil() as u64;
    let raw_amount = (billed_gb as f64) * ghs_per_gb;
    let (amount, minimum_applied) = if raw_amount < monthly_min {
        (monthly_min, true)
    } else {
        (raw_amount, false)
    };
    InvoiceAmount {
        amount_ghs: round_to_cents(amount),
        minimum_applied,
        billed_gb,
    }
}

fn round_to_cents(x: f64) -> f64 { (x * 100.0).round() / 100.0 }

#[cfg(test)]
mod tests {
    use super::*;

    const GB: i64 = 1_073_741_824;
    const RATE: f64 = 1.00;
    const MIN: f64 = 5.00;

    #[test]
    fn empty_mailbox_pays_the_minimum() {
        let r = compute_invoice_ghs(0, RATE, MIN);
        assert_eq!(r.amount_ghs, 5.00);
        assert!(r.minimum_applied);
        assert_eq!(r.billed_gb, 0);
    }

    #[test]
    fn one_gb_exact_pays_one_then_min_applies() {
        // 1 GB @ GHS 1/GB = GHS 1.00, but monthly minimum GHS 5 wins.
        let r = compute_invoice_ghs(GB, RATE, MIN);
        assert_eq!(r.billed_gb, 1);
        assert_eq!(r.amount_ghs, 5.00);
        assert!(r.minimum_applied);
    }

    #[test]
    fn just_over_one_gb_rounds_up_to_two_gb() {
        let r = compute_invoice_ghs(GB + 1, RATE, MIN);
        assert_eq!(r.billed_gb, 2);
        // 2 * 1 = 2, still under min, so 5.00.
        assert_eq!(r.amount_ghs, 5.00);
    }

    #[test]
    fn nine_point_five_gb_charges_ten_ghs() {
        // 9.5 GB → ceil → 10 → 10 * 1 = GHS 10.00; minimum doesn't kick in.
        let avg = (9.5_f64 * GB as f64) as i64;
        let r = compute_invoice_ghs(avg, RATE, MIN);
        assert_eq!(r.billed_gb, 10);
        assert_eq!(r.amount_ghs, 10.00);
        assert!(!r.minimum_applied);
    }

    #[test]
    fn fifty_gb_charges_fifty_ghs() {
        let r = compute_invoice_ghs(50 * GB, RATE, MIN);
        assert_eq!(r.billed_gb, 50);
        assert_eq!(r.amount_ghs, 50.00);
        assert!(!r.minimum_applied);
    }

    #[test]
    fn negative_storage_treated_as_zero() {
        let r = compute_invoice_ghs(-100, RATE, MIN);
        assert_eq!(r.billed_gb, 0);
        assert_eq!(r.amount_ghs, MIN);
    }

    #[test]
    fn alternative_rate_scales_linearly() {
        // GHS 0.50 / GB, monthly min GHS 2.00, 100 GB stored → GHS 50.00
        let r = compute_invoice_ghs(100 * GB, 0.50, 2.00);
        assert_eq!(r.amount_ghs, 50.00);
        assert!(!r.minimum_applied);
    }

    #[test]
    fn rate_change_does_not_drop_below_minimum() {
        let r = compute_invoice_ghs(GB, 0.10, 0.50);
        assert_eq!(r.amount_ghs, 0.50);
        assert!(r.minimum_applied);
    }

    #[test]
    fn cents_are_rounded_correctly() {
        // Force a value that needs rounding: 3 GB * 0.333 = 0.999 → 1.00
        let r = compute_invoice_ghs(3 * GB, 0.333, 0.0);
        assert_eq!(r.amount_ghs, 1.00);
    }
}
